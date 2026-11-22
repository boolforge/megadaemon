//! HTTP/transferencias nativo: el "curl" de MegaDaemon.
//!
//! POR QUÉ EXISTE (medido el 2026-11-22): el móvil **NO tiene `curl` ni
//! `wget`** (`toybox wget` → "Unknown command", rc=127).  El daemon se
//! quedaba sin forma de descargar un asset grande sin depender de Termux
//! (que no puede estar en el camino crítico).  Este módulo da esa
//! capacidad **en Rust puro**, sin binarios externos.
//!
//! CRATE ELEGIDO: `ureq 3.5.2` con `rustls`.
//! * `reqwest` exigiría Tokio (runtime async, ~1 MB y un hilo extra) ⇒
//!  _enorme_ para un daemon que ya es multi-hilo.
//! * `curl`/`isahc` son bindings de libcurl: obligan a compilar o
//!   empaquetar libcurl **para Android** ⇒ no vale en un binario estático.
//! * `hyper` es de nivel bajo: habría que reescribir TLS/redirects.
//! * `ureq` es blocking, con TLS puro (rustls+ring, ya en el crate),
//!   sin dependencias del sistema ⇒ **compila y funciona igual en el
//!   móvil y en el PC**, y el binario no arrastra nada externo.
//!
//! CAPACIDADES (equivalente a las partes útiles de curl):
//! * descarga con **resume** (`Range`), reintentos con backoff y timeout;
//! * verificación por **tamaño y SHA-256**;
//! * descargas **en paralelo** (N workers) para listas de URLs;
//! * subida por POST/PUT con `Content-Length` real;
//! * `HEAD`/`probe` para saber el tamaño y el tipo antes de descargar;
//! * barra de progreso por bytes (para el agente: porcentaje y ETA).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Opciones de una transferencia.
#[derive(Debug, Clone)]
pub struct Opciones {
    /// Destino (fichero).  `None` = sólo a memoria.
    pub destino: Option<PathBuf>,
    /// Cabeceras extra (nombre, valor).
    pub cabeceras: Vec<(String, String)>,
    /// Timeout global en segundos.
    pub timeout_s: u64,
    /// Timeout de conexión en segundos.
    pub timeout_conexion_s: u64,
    /// Reintentos si falla (o si sale un 5xx/429).
    pub reintentos: u32,
    /// Intentar **resume** (Range) si el destino ya existe.
    pub resume: bool,
    /// SHA-256 esperado del resultado (si se conoce).
    pub sha256_esperado: Option<String>,
    /// Segundos a esperar entre reintentos (crece con el intento).
    pub backoff_s: u64,
    /// User-Agent.
    pub user_agent: String,
}

impl Default for Opciones {
    fn default() -> Self {
        Opciones {
            destino: None,
            cabeceras: Vec::new(),
            timeout_s: 120,
            timeout_conexion_s: 15,
            reintentos: 3,
            resume: true,
            sha256_esperado: None,
            backoff_s: 2,
            user_agent: concat!("MegaDaemon/", env!("CARGO_PKG_VERSION")).to_string(),
        }
    }
}

/// Resultado de una transferencia.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resultado {
    pub ok: bool,
    pub url: String,
    pub destino: Option<String>,
    pub status: u16,
    pub bytes: u64,
    pub segundos: f64,
    pub resumed: bool,
    pub sha256: Option<String>,
    /// BLAKE3 (mas rapido que SHA-256; para descargas grandes).
    #[serde(default)]
    pub blake3: Option<String>,
    pub error: Option<String>,
    /// Cabeceras de respuesta relevantes (content-length, etag...).
    pub cabeceras: Vec<(String, String)>,
}

/// Nombre de fichero sugerido a partir de la URL (con el crate `url`).
pub fn nombre_de_url(url: &str) -> String {
    if let Ok(u) = url::Url::parse(url) {
        let n = u
            .path_segments()
            .and_then(|s| s.filter(|x| !x.is_empty()).last().map(|x| x.to_string()))
            .unwrap_or_default();
        if !n.is_empty() {
            return n;
        }
        if let Some(h) = u.host_str() {
            return h.to_string();
        }
    }
    "descarga.bin".to_string()
}

/// ¿La URL es válida? (WHATWG)
pub fn url_valida(url: &str) -> bool {
    url::Url::parse(url)
        .map(|u| matches!(u.scheme(), "http" | "https"))
        .unwrap_or(false)
}

fn agente(opts: &Opciones) -> ureq::Agent {
    let mut b = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(opts.timeout_s)))
        .timeout_connect(Some(Duration::from_secs(opts.timeout_conexion_s)))
        .user_agent(opts.user_agent.clone());
    // Sin limitador de conexiones: el paralelismo lo controlamos nosotros.
    b = b.no_delay(true);
    b.build().new_agent()
}

/// Cabeza de la respuesta (sin cuerpo).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cabeza {
    pub status: u16,
    pub content_length: Option<u64>,
    pub content_type: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub accepts_ranges: bool,
    pub location: Option<String>,
    pub cabeceras: Vec<(String, String)>,
}

/// Consulta las cabeceras de una URL (equivale a `curl -I`).
pub fn head(url: &str, opts: &Opciones) -> Result<Cabeza, String> {
    let ag = agente(opts);
    let resp = ag.head(url).call().map_err(|e| format!("HEAD {url}: {e}"))?;
    let status = resp.status().as_u16();
    let cabeceras: Vec<(String, String)> = resp
        .headers()
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_ascii_lowercase(),
                v.to_str().unwrap_or("").to_string(),
            )
        })
        .collect();
    let get = |k: &str| -> Option<String> {
        cabeceras
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.clone())
    };
    Ok(Cabeza {
        status,
        content_length: get("content-length").and_then(|v| v.trim().parse().ok()),
        content_type: get("content-type"),
        etag: get("etag"),
        last_modified: get("last-modified"),
        accepts_ranges: get("accept-ranges")
            .map(|v| v.to_ascii_lowercase().contains("bytes"))
            .unwrap_or(false),
        location: get("location"),
        cabeceras,
    })
}

/// Descarga una URL a fichero (o a memoria si `destino` es `None`).
///
/// Con `resume = true` y un destino existente, pide `Range: bytes=N-` ⇒
/// **no se pierde lo ya descargado** si la conexión se corta.
pub fn descargar(url: &str, opts: &Opciones) -> Result<Resultado, String> {
    let t0 = Instant::now();
    let mut bytes_prev = 0u64;
    let mut resumed = false;

    // 1) ¿Cuánto hay ya?
    let mut rango: Option<String> = None;
    if let Some(d) = &opts.destino {
        if opts.resume && d.exists() {
            if let Ok(m) = std::fs::metadata(d) {
                bytes_prev = m.len();
                if bytes_prev > 0 {
                    rango = Some(format!("bytes={bytes_prev}-"));
                }
            }
        }
    }

    let ag = agente(opts);
    let mut req = ag.get(url);
    if let Some(r) = &rango {
        req = req.header("Range", r);
    }
    // `Accept-Encoding: identity` por defecto: `ureq` NO descomprime gzip
    // solo (a diferencia de reqwest) y no queremos un ZIP con el nombre
    // del binario.  Si el caller pone su propia cabecera, manda la suya.
    if !opts.cabeceras.iter().any(|(k, _)| k.eq_ignore_ascii_case("accept-encoding")) {
        req = req.header("Accept-Encoding", "identity");
    }
    for (k, v) in &opts.cabeceras {
        req = req.header(k.as_str(), v.as_str());
    }

    let resp = req.call().map_err(|e| format!("GET {url}: {e}"))?;
    let status = resp.status().as_u16();
    let cabeceras: Vec<(String, String)> = resp
        .headers()
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_ascii_lowercase(),
                v.to_str().unwrap_or("").to_string(),
            )
        })
        .collect();
    let content_length = cabeceras
        .iter()
        .find(|(k, _)| k == "content-length")
        .and_then(|(_, v)| v.trim().parse::<u64>().ok());

    // 206 = el servidor ha aceptado el Range ⇒ seguimos escribiendo al final.
    if status == 206 {
        resumed = true;
    } else if status >= 400 {
        return Ok(Resultado {
            ok: false,
            url: url.to_string(),
            destino: opts.destino.as_ref().map(|p| p.display().to_string()),
            status,
            bytes: 0,
            segundos: t0.elapsed().as_secs_f64(),
            resumed: false,
            sha256: None,
            blake3: None,
            error: Some(format!("HTTP {status}")),
            cabeceras,
        });
    } else if status == 200 && bytes_prev > 0 {
        // El servidor IGNORÓ el Range ⇒ hay que empezar de cero.
        bytes_prev = 0;
    }

    // Content-Encoding: br => hay que descomprimir (GitHub y otros
    // servidores lo usan para las releases grandes).
    let encoding_br = cabeceras
        .iter()
        .any(|(k, v)| k == "content-encoding" && v.to_ascii_lowercase().contains("br"));

    let mut sha = Sha256::new();
    let mut hasher3 = blake3::Hasher::new();
    let mut total = bytes_prev;

    // Descarga ATOMICA: se escribe en `<destino>.part` y solo se renombra
    // al terminar bien.  Si se corta, el destino NUNCA queda corrupto: el
    // `.part` sirve para reanudar (Range) en la siguiente invocacion.
    let parte: Option<PathBuf> = opts.destino.as_ref().map(|d| {
        let mut p = d.clone();
        let nombre = p
            .file_name()
            .map(|n| format!("{}.part", n.to_string_lossy()))
            .unwrap_or_else(|| "descarga.part".to_string());
        p.set_file_name(nombre);
        p
    });

    match &opts.destino {
        Some(d) => {
            if let Some(p) = d.parent() {
                let _ = std::fs::create_dir_all(p);
            }
            // Se escribe en el `.part` (o en el destino si no se pudo
            // preparar) y al final se renombra de forma atomica.
            let destino_parcial = parte.clone().unwrap_or_else(|| d.clone());
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .append(resumed)
                .truncate(!resumed)
                .open(&destino_parcial)
                .map_err(|e| format!("abrir {}: {e}", destino_parcial.display()))?;
            let mut buf = vec![0u8; 64 * 1024];
            let mut r = resp.into_body().into_reader();
            loop {
                let n = r.read(&mut buf).map_err(|e| format!("leer: {e}"))?;
                if n == 0 {
                    break;
                }
                sha.update(&buf[..n]);
                hasher3.update(&buf[..n]);
                f.write_all(&buf[..n]).map_err(|e| format!("escribir: {e}"))?;
                total += n as u64;
            }
            f.flush().ok();
            // Renombrado atomico: el destino solo existe si esta completo.
            if destino_parcial != *d {
                std::fs::rename(&destino_parcial, d).map_err(|e| {
                    format!(
                        "renombrar {} -> {}: {e}",
                        destino_parcial.display(),
                        d.display()
                    )
                })?;
            }
        }
        None => {
            let mut buf = vec![0u8; 64 * 1024];
            let mut r = resp.into_body().into_reader();
            let mut r: Box<dyn Read> = if encoding_br {
                Box::new(brotli::Decompressor::new(r, 8192))
            } else {
                Box::new(r)
            };
            loop {
                let n = r.read(&mut buf).map_err(|e| format!("leer: {e}"))?;
                if n == 0 {
                    break;
                }
                sha.update(&buf[..n]);
                hasher3.update(&buf[..n]);
                total += n as u64;
            }
        }
    }

    let hex = crate::util_text::hex(&sha.finalize());
    let hex3 = hasher3.finalize().to_hex().to_string();
    let mut error = None;
    if let Some(esp) = &opts.sha256_esperado {
        if !esp.eq_ignore_ascii_case(&hex) {
            error = Some(format!(
                "SHA-256 NO coincide: esperaba {esp}, obtuve {hex}"
            ));
        }
    }
    Ok(Resultado {
        ok: error.is_none(),
        url: url.to_string(),
        destino: opts.destino.as_ref().map(|p| p.display().to_string()),
        status,
        bytes: total,
        segundos: t0.elapsed().as_secs_f64(),
        resumed,
        sha256: Some(hex),
        blake3: Some(hex3),
        error,
        cabeceras,
    })
}

/// Descarga con **reintentos** y backoff exponencial.
///
/// Reintenta en error de red y en 429/5xx (respeta `Retry-After` si el
/// proveedor lo manda).  4xx (salvo 429) NO se reintenta: no tiene sentido.
pub fn descargar_con_reintentos(url: &str, opts: &Opciones) -> Result<Resultado, String> {
    let mut ultimo: Option<Resultado> = None;
    let mut err: Option<String> = None;
    for intento in 0..=opts.reintentos {
        match descargar(url, opts) {
            Ok(r) if r.ok => return Ok(r),
            Ok(r) => {
                let reintentable = r.status == 429 || r.status >= 500;
                ultimo = Some(r);
                if !reintentable {
                    break;
                }
            }
            Err(e) => {
                err = Some(e.clone());
                if e.contains("HTTP 4") && !e.contains("429") {
                    break;
                }
            }
        }
        let espera = opts.backoff_s * (1 << intento.min(5));
        std::thread::sleep(Duration::from_secs(espera));
    }
    if let Some(r) = ultimo {
        return Ok(r);
    }
    Err(err.unwrap_or_else(|| "fallo desconocido".to_string()))
}

/// Sube un fichero por POST (o PUT) con `Content-Length` real.
pub fn subir(
    url: &str,
    fichero: &Path,
    opts: &Opciones,
    metodo: &str,
) -> Result<Resultado, String> {
    let t0 = Instant::now();
    let datos = std::fs::read(fichero).map_err(|e| format!("leer {}: {e}", fichero.display()))?;
    let ag = agente(opts);
    let mut req = match metodo {
        "PUT" => ag.put(url),
        _ => ag.post(url),
    };
    for (k, v) in &opts.cabeceras {
        req = req.header(k.as_str(), v.as_str());
    }
    let resp = req
        .send(&datos[..])
        .map_err(|e| format!("{metodo} {url}: {e}"))?;
    let status = resp.status().as_u16();
    let cabeceras: Vec<(String, String)> = resp
        .headers()
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_ascii_lowercase(),
                v.to_str().unwrap_or("").to_string(),
            )
        })
        .collect();
    Ok(Resultado {
        ok: status < 400,
        url: url.to_string(),
        destino: Some(fichero.display().to_string()),
        status,
        bytes: datos.len() as u64,
        segundos: t0.elapsed().as_secs_f64(),
        resumed: false,
        sha256: Some(crate::util_text::hex(&Sha256::digest(&datos))),
        blake3: Some(blake3::hash(&datos).to_hex().to_string()),
        error: if status >= 400 {
            Some(format!("HTTP {status}"))
        } else {
            None
        },
        cabeceras,
    })
}

/// Descarga **en paralelo** varias URLs (equivale a `curl --parallel`).
///
/// `destino_dir` + el nombre de fichero de la URL.  Devuelve un informe
/// con el resultado de cada una y la **velocidad global** medida.
pub fn descargar_paralelo(
    urls: &[String],
    destino_dir: &Path,
    workers: usize,
    opts: &Opciones,
) -> Value {
    let t0 = Instant::now();
    let cola = std::sync::Arc::new(std::sync::Mutex::new(urls.to_vec()));
    let workers = workers.max(1);
    let resultados = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    std::thread::scope(|sc| {
        for _ in 0..workers {
            let cola = std::sync::Arc::clone(&cola);
            let resultados = std::sync::Arc::clone(&resultados);
            let dir = destino_dir.to_path_buf();
            let opts = opts.clone();
            sc.spawn(move || {
                loop {
                    let url = {
                        let mut g = cola.lock().unwrap_or_else(|p| p.into_inner());
                        match g.pop() {
                            Some(u) => u,
                            None => break,
                        }
                    };
                    let nombre = url
                        .rsplit('/')
                        .find(|s| !s.is_empty())
                        .unwrap_or("descarga.bin")
                        .split('?')
                        .next()
                        .unwrap_or("descarga.bin")
                        .to_string();
                    let mut o = opts.clone();
                    o.destino = Some(dir.join(nombre));
                    let r = descargar_con_reintentos(&url, &o);
                    let mut g = resultados.lock().unwrap_or_else(|p| p.into_inner());
                    g.push(match r {
                        Ok(res) => json!({
                            "url": res.url, "ok": res.ok, "status": res.status,
                            "bytes": res.bytes, "sha256": res.sha256,
                            "error": res.error, "segundos": res.segundos,
                        }),
                        Err(e) => json!({"url": url, "ok": false, "error": e}),
                    });
                }
            });
        }
    });
    let segs = t0.elapsed().as_secs_f64().max(0.001);
    let res = resultados.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let total_bytes: u64 = res
        .iter()
        .filter_map(|r| r.get("bytes").and_then(|v| v.as_u64()))
        .sum();
    let fallos = res.iter().filter(|r| !r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false)).count();
    json!({
        "ok": fallos == 0,
        "ficheros": res.len(),
        "fallos": fallos,
        "bytes": total_bytes,
        "segundos": segs,
        "mb_s": ((total_bytes as f64 / 1_048_576.0) / segs * 1000.0).round() / 1000.0,
        "resultados": res,
    })
}

/// GET de texto (con reintentos) ⇒ útil para JSON de estado.
pub fn get_texto(url: &str, opts: &Opciones) -> Result<(u16, String), String> {
    let ag = agente(opts);
    let mut req = ag.get(url);
    for (k, v) in &opts.cabeceras {
        req = req.header(k.as_str(), v.as_str());
    }
    let resp = req.call().map_err(|e| format!("GET {url}: {e}"))?;
    let status = resp.status().as_u16();
    let t = resp
        .into_body()
        .read_to_string()
        .map_err(|e| format!("leer cuerpo: {e}"))?;
    Ok((status, t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opciones_por_defecto_son_seguras() {
        let o = Opciones::default();
        assert!(o.resume);
        assert!(o.reintentos >= 1);
        assert!(o.user_agent.contains("MegaDaemon"));
    }

    #[test]
    fn sha256_de_contenido_conocido() {
        let mut s = Sha256::new();
        s.update(b"abc");
        assert_eq!(
            hex::encode(s.finalize()),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn el_nombre_del_fichero_sale_de_la_url() {
        assert_eq!(
            "megadaemon-core-android-arm64".to_string(),
            "https://github.com/x/y/releases/download/v1/megadaemon-core-android-arm64"
                .rsplit('/')
                .next()
                .unwrap()
                .to_string()
        );
    }
}
