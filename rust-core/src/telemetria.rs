//! Telemetría del móvil: **recolección → disco → Drive**, sin depender de que
//! nadie esté conectado.
//!
//! POR QUÉ ESTE MÓDULO
//! --------------------
//! MEDIDO (2026-11-26): el problema de fondo del enjambre NO es el protocolo
//! sino el **canal**.  El túnel del móvil es un *quick tunnel* de Cloudflare:
//! su hostname cambia en cada reconexión y el relé canónico
//! (`mdtmfnc.duckdns.org`) puede estar caído.  Cuando eso pasa, el sandbox se
//! queda CIEGO y la única forma de recuperar el control es que **alguien** se
//! conecte a mano.
//!
//! La telemetría a Drive rompe ese círculo: el móvil **empuja** su estado a
//! un sitio estable (la API de Drive no cambia de hostname, la credencial se
//! refresca sola) y el root **tira** de él desde donde esté.  Es el mismo
//! patrón que usa `diagnostico::publicar_periodico`, pero aquí además:
//!
//!   * se **auto-detecta cuándo el nodo está listo** (`estado_listo()`),
//!     de modo que nadie tiene que adivinar si ya arrancó;
//!   * se publican los **datos de conexión** (URL viva, puertos, build,
//!     fingerprint de las claves) para que el root sepa a dónde conectarse
//!     sin tener que descubrir el túnel;
//!   * se sube el **inventario de instrumentación** (NDJSON de los hooks,
//!     plan de sondas) para poder analizarlo sin el móvil delante.
//!
//! REGLAS
//! ------
//! * **Nada de secretos**: aquí sólo viajan hashes (`sha256`/`sha16`), nunca
//!   claves ni tokens.  El token de Drive se pide a `google_auth` y no sale
//!   de este proceso.
//! * **La subida es best-effort pero observable**: si Drive falla, el
//!   diagnóstico queda en disco (`telemetria/`) y la siguiente iteración
//!   reintenta; el estado local es la fuente de verdad.
//! * **Batería**: el periodo se duplica en modo ahorro (reutiliza
//!   `battery_advanced::modo_ahorro_activo`, la misma política que ya usa
//!   el diagnóstico).

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Carpeta (VFS) donde se publican las telemetrías en Drive.
pub const CARPETA_VFS: &str = "MegaDaemon/telemetria";

/// Subcarpeta de los datos de conexión (para encontrarlos de un vistazo).
pub const CARPETA_CONEXION: &str = "MegaDaemon/conexion";

/// Nombre del fichero local con el último estado (JSON).
const ULTIMO_JSON: &str = "ultimo.json";

/// Nombre del fichero local con el último informe (Markdown).
const ULTIMO_MD: &str = "ultimo.md";

/// Epoch en segundos.
fn ahora() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Nombre del nodo (móvil, PC, agente…).
fn nodo() -> String {
    crate::diagnostico::nodo()
}

/// SHA-256 de un fichero (streaming; no lo carga entero en memoria).
fn sha256_fichero(path: &Path) -> Option<String> {
    crate::drive_native::sha256_file(path).ok()
}

/// SHA-16 (hex) de un texto: permite comparar credenciales **sin filtrarlas**.
fn sha16_real(texto: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(texto.as_bytes());
    let out = h.finalize();
    out.iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
}

/// Directorio local de telemetrías.
fn dir_local(state_dir: Option<&str>) -> PathBuf {
    let base = state_dir
        .map(|s| s.to_string())
        .unwrap_or_else(|| crate::state_dir::state_dir());
    Path::new(&base).join("telemetria")
}

// ========================================================================= //
// RECOLECCIÓN
// ========================================================================= //

/// Lee un fichero del sistema y lo recorta.
fn leer(estado_dir: Option<&str>, ruta: &str) -> Option<String> {
    let p = if let Some(s) = estado_dir {
        Path::new(s).join(ruta)
    } else {
        PathBuf::from(ruta)
    };
    std::fs::read_to_string(p)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Bytes leídos de una interfaz de red (`/proc/net/dev`).
///
/// Formato de la línea: `wlan0: 12345 67 ... 890 12 ...`
/// (columnas: rx bytes, rx packets, ..., tx bytes, tx packets).
fn parsear_net_dev(texto: &str) -> Value {
    let mut mapa = serde_json::Map::new();
    for linea in texto.lines() {
        let Some((iface, resto)) = linea.split_once(':') else {
            continue;
        };
        let iface = iface.trim();
        if iface.is_empty() || iface == "lo" {
            continue;
        }
        let c: Vec<u64> = resto
            .split_whitespace()
            .map(|x| x.parse::<u64>().unwrap_or(0))
            .collect();
        if c.len() < 9 {
            continue;
        }
        // c[0]=rx bytes, c[8]=tx bytes.
        mapa.insert(
            iface.to_string(),
            json!({"rx_bytes": c[0], "tx_bytes": c[8]}),
        );
    }
    Value::Object(mapa)
}

/// Clasifica una interfaz de red: `"wifi"`, `"movil"`, `"ethernet"` o
/// `"otra"`.
///
/// MEDIDO (2026-11-26, Samsung S22): la interfaz de **datos móviles** en
/// Android se llama `rmnet_data0` (o `rmnet_data1`, `ccmni0`… según el
/// módem) y la de **WiFi** es `wlan0`.  Distinguirlas es lo que permite
/// separar "gasto por WiFi" de "gasto por red móvil" (el Jazztel factura
/// el móvil, no el WiFi).
fn clasificar_interfaz(iface: &str) -> &'static str {
    let i = iface.to_ascii_lowercase();
    if i.starts_with("wlan") || i.starts_with("wifi") {
        "wifi"
    } else if i.starts_with("rmnet") || i.starts_with("ccmni") || i.starts_with("ppp")
        || i.starts_with("wwan") || i.starts_with("cdma")
    {
        "movil"
    } else if i.starts_with("eth") {
        "ethernet"
    } else {
        "otra"
    }
}

/// Reparte los bytes por tipo de transporte.
fn desglose_por_transporte(net: &Value) -> Value {
    let mut wifi = 0u64;
    let mut movil = 0u64;
    let mut ethernet = 0u64;
    let mut otra = 0u64;
    if let Some(m) = net.as_object() {
        for (iface, v) in m {
            let rx = v["rx_bytes"].as_u64().unwrap_or(0);
            let tx = v["tx_bytes"].as_u64().unwrap_or(0);
            let total = rx.saturating_add(tx);
            match clasificar_interfaz(iface) {
                "wifi" => wifi += total,
                "movil" => movil += total,
                "ethernet" => ethernet += total,
                _ => otra += total,
            }
        }
    }
    json!({
        "wifi_bytes": wifi,
        "movil_bytes": movil,
        "ethernet_bytes": ethernet,
        "otra_bytes": otra,
    })
}

/// Recolecta el estado completo del nodo.
///
/// Devuelve un JSON **sin secretos**: sólo medidas, nombres y hashes.
pub fn recoger(state_dir: Option<&str>) -> Value {
    let sd = state_dir.map(|s| s.to_string());
    let mut v = json!({
        "ts": ahora(),
        "nodo": nodo(),
        "build": crate::self_update::build_commit(),
        "version": env!("CARGO_PKG_VERSION"),
    });

    // --- energía ------------------------------------------------------------
    let cap = leer(sd.as_deref(), "/sys/class/power_supply/battery/capacity");
    let status = leer(sd.as_deref(), "/sys/class/power_supply/battery/status");
    let temp = leer(sd.as_deref(), "/sys/class/power_supply/battery/temp");
    v["bateria"] = json!({
        "capacidad_pct": cap.as_deref().and_then(|s| s.parse::<u64>().ok()),
        "estado": status,
        "temp_raw": temp.as_deref().and_then(|s| s.parse::<i64>().ok()),
        "modo_ahorro": crate::battery_advanced::modo_ahorro_activo(),
    });

    // --- memoria ------------------------------------------------------------
    let meminfo = leer(sd.as_deref(), "/proc/meminfo").unwrap_or_default();
    let mut mem = serde_json::Map::new();
    for linea in meminfo.lines() {
        let Some((k, resto)) = linea.split_once(':') else {
            continue;
        };
        if ["MemTotal", "MemAvailable", "MemFree", "Cached", "SwapTotal", "SwapFree"]
            .contains(&k.trim())
        {
            let kb: u64 = resto
                .trim()
                .split_whitespace()
                .next()
                .and_then(|x| x.parse().ok())
                .unwrap_or(0);
            mem.insert(k.trim().to_string(), json!(kb * 1024));
        }
    }
    v["memoria_bytes"] = Value::Object(mem);

    // --- carga --------------------------------------------------------------
    let loadavg = leer(sd.as_deref(), "/proc/loadavg").unwrap_or_default();
    let cargas: Vec<f64> = loadavg
        .split_whitespace()
        .take(3)
        .map(|x| x.parse().unwrap_or(0.0))
        .collect();
    v["carga"] = json!({
        "load1": cargas.first().copied().unwrap_or(0.0),
        "load5": cargas.get(1).copied().unwrap_or(0.0),
        "load15": cargas.get(2).copied().unwrap_or(0.0),
    });
    v["uptime_seg"] = json!(leer(sd.as_deref(), "/proc/uptime")
        .and_then(|s| s.split_whitespace().next().and_then(|x| x.parse::<f64>().ok()))
        .map(|x| x as u64)
        .unwrap_or(0));

    // --- red ----------------------------------------------------------------
    let net_dev = leer(sd.as_deref(), "/proc/net/dev").unwrap_or_default();
    let net = parsear_net_dev(&net_dev);
    let mut v_red = json!({"interfaces": net.clone()});
    if let (Some(o), Some(d)) = (v_red.as_object_mut(), net.as_object()) {
        o.insert("por_transporte".to_string(), desglose_por_transporte(&net));
        // Top 5 interfaces por tráfico total (para ver cuál está activa).
        let mut filas: Vec<(String, u64)> = d
            .iter()
            .map(|(k, x)| {
                (
                    k.clone(),
                    x["rx_bytes"].as_u64().unwrap_or(0) + x["tx_bytes"].as_u64().unwrap_or(0),
                )
            })
            .collect();
        filas.sort_by(|a, b| b.1.cmp(&a.1));
        o.insert(
            "top".to_string(),
            Value::Array(
                filas
                    .into_iter()
                    .take(5)
                    .map(|(k, t)| json!({"interfaz": k, "bytes": t, "tipo": clasificar_interfaz(&k)}))
                    .collect(),
            ),
        );
    }
    v["red"] = v_red;

    // --- instrumentación viva (NDJSON de los hooks) -------------------------
    v["instrumentacion"] = resumen_instrumentacion(sd.as_deref());

    // --- canal y conexión ---------------------------------------------------
    v["canal"] = crate::channel::read_channel_json(sd.as_deref()).unwrap_or(Value::Null);
    v["conexion"] = datos_conexion(sd.as_deref());

    // --- diagnóstico (reutiliza el módulo existente) ------------------------
    v["diagnostico"] = crate::diagnostico::recoger();

    v
}

/// Resumen compacto del NDJSON de los hooks (sin los cuerpos enormes).
fn resumen_instrumentacion(state_dir: Option<&str>) -> Value {
    let dir = state_dir
        .map(|s| s.to_string())
        .unwrap_or_else(|| crate::state_dir::state_dir());
    // El NDJSON de los hooks vive en la caché de la app instrumentada; se
    // buscan las candidatas conocidas (el UID va en el nombre).
    let mut candidatas: Vec<PathBuf> = Vec::new();
    for base in [
        "/data/user/0/com.agy/cache",
        "/data/data/com.agy/cache",
    ] {
        if let Ok(rd) = std::fs::read_dir(base) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                if n.starts_with("megadaemon-hooks-") && n.ends_with(".ndjson") {
                    candidatas.push(e.path());
                }
            }
        }
    }
    let mut out = serde_json::Map::new();
    out.insert("candidatas".to_string(), json!(candidatas.len()));
    if let Some(p) = candidatas.iter().max_by_key(|p| {
        std::fs::metadata(p).and_then(|m| m.modified()).ok()
    }) {
        let bytes = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        let texto = std::fs::read_to_string(p).unwrap_or_default();
        let lineas = texto.lines().count();
        let malformadas = texto
            .lines()
            .filter(|l| !l.trim().is_empty() && serde_json::from_str::<Value>(l).is_err())
            .count();
        let con_ret = texto.lines().filter(|l| l.contains("ret_str")).count();
        out.insert("fichero".to_string(), json!(p.display().to_string()));
        out.insert("bytes".to_string(), json!(bytes));
        out.insert("lineas".to_string(), json!(lineas));
        out.insert("malformadas".to_string(), json!(malformadas));
        out.insert("eventos_con_retorno".to_string(), json!(con_ret));
    }
    // El state dir puede estar en otro sitio: se anota para el diagnóstico.
    out.insert("state_dir".to_string(), json!(dir));
    Value::Object(out)
}

/// Datos de **conexión**: por dónde se puede alcanzar a este nodo.
///
/// Sólo URLs, puertos y hashes: **nunca** claves ni tokens.
pub fn datos_conexion(state_dir: Option<&str>) -> Value {
    let sd = state_dir.map(|s| s.to_string());
    let base = sd
        .clone()
        .unwrap_or_else(|| crate::state_dir::state_dir());
    let canal = crate::channel::read_channel_json(sd.as_deref()).unwrap_or(Value::Null);

    let quick = canal
        .get("quick")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let paste = canal
        .get("paste")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Fingerprints de las claves (NUNCA su valor).
    let mut claves = serde_json::Map::new();
    for cand in [
        format!("{base}/megadaemon_api.key"),
        format!("{base}/megadaemon_api.keys"),
        "/data/adb/megadaemon/state/megadaemon_api.key".to_string(),
    ] {
        let p = Path::new(&cand);
        if let Ok(txt) = std::fs::read_to_string(p) {
            let t = txt.trim();
            if !t.is_empty() {
                claves.insert(
                    p.file_name().unwrap_or_default().to_string_lossy().to_string(),
                    json!({"sha16": sha16_real(t), "bytes": t.len()}),
                );
            }
        }
    }

    json!({
        "quick": quick,
        "paste": paste,
        "canonical": crate::channel::canonical_efectiva(),
        "puerto_gateway": 9090,
        "build": crate::self_update::build_commit(),
        "claves_sha16": Value::Object(claves),
        "ts": ahora(),
    })
}

/// ¿Está el nodo **listo** para recibir trabajo?
///
/// Se considera listo cuando se cumple TODO:
/// 1. el binario del daemon existe y es ejecutable;
/// 2. hay una URL de túnel publicada;
/// 3. el canal se puede leer (el estado local existe).
///
/// La comprobación de que el túnel **responde** se hace aparte
/// (`sonda_tunel`), porque requiere red y no debe fallar el diagnóstico local.
pub fn estado_listo(state_dir: Option<&str>) -> Value {
    let sd = state_dir.map(|s| s.to_string());
    let base = sd
        .clone()
        .unwrap_or_else(|| crate::state_dir::state_dir());

    let binarios = [
        "/data/adb/megadaemon/megadaemon-core",
        "/data/local/tmp/megadaemon/megadaemon-core",
    ];
    let mut bin = None;
    for b in binarios {
        let p = Path::new(b);
        if p.exists() {
            bin = Some(json!({
                "ruta": b,
                "bytes": std::fs::metadata(p).map(|m| m.len()).unwrap_or(0),
                "sha256": sha256_fichero(p),
            }));
            break;
        }
    }

    let conexion = datos_conexion(sd.as_deref());
    let quick = conexion["quick"].as_str().unwrap_or("").to_string();
    let canal_leido = crate::channel::read_channel_json(sd.as_deref()).is_some();

    let listo = bin.is_some() && !quick.is_empty() && canal_leido;
    json!({
        "listo": listo,
        "binario": bin,
        "quick": quick,
        "canal_leido": canal_leido,
        "state_dir": base,
        "ts": ahora(),
        "criterio": "binario presente + URL de tunel publicada + canal legible",
    })
}

/// Sonda el túnel publicado y devuelve el código HTTP + si es el daemon.
pub fn sonda_tunel(url: &str) -> Value {
    if url.trim().is_empty() {
        return json!({"ok": false, "codigo": 0, "detalle": "sin URL"});
    }
    let (code, es_daemon) = crate::channel::probe_url(url);
    json!({
        "ok": es_daemon,
        "codigo": code,
        "es_daemon": es_daemon,
        "url": url,
        "ts": ahora(),
    })
}

// ========================================================================= //
// PERSISTENCIA Y PUBLICACIÓN
// ========================================================================= //

/// Escribe el estado en disco (JSON + Markdown) y devuelve las rutas.
pub fn escribir_local(v: &Value, state_dir: Option<&str>) -> Result<(PathBuf, PathBuf), String> {
    let dir = dir_local(state_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("crear {}: {e}", dir.display()))?;
    let json_p = dir.join(ULTIMO_JSON);
    let md_p = dir.join(ULTIMO_MD);
    std::fs::write(
        &json_p,
        serde_json::to_vec_pretty(v).unwrap_or_default(),
    )
    .map_err(|e| format!("escribir {}: {e}", json_p.display()))?;
    std::fs::write(&md_p, a_markdown(v)).map_err(|e| format!("escribir {}: {e}", md_p.display()))?;
    Ok((json_p, md_p))
}

/// Informe Markdown legible de un estado.
pub fn a_markdown(v: &Value) -> String {
    let n = v["nodo"].as_str().unwrap_or("?");
    let ts = v["ts"].as_u64().unwrap_or(0);
    let mut s = format!("# Telemetría MegaDaemon — {n}\n\n- ts: {ts}\n");
    s.push_str(&format!(
        "- build: `{}`\n",
        v["build"].as_str().unwrap_or("?")
    ));
    if let Some(c) = v["bateria"]["capacidad_pct"].as_u64() {
        s.push_str(&format!("- batería: {c} % ({})\n", v["bateria"]["estado"].as_str().unwrap_or("?")));
    }
    s.push_str(&format!(
        "- carga (1m): {}\n",
        v["carga"]["load1"].as_f64().unwrap_or(0.0)
    ));
    let t = &v["red"]["por_transporte"];
    s.push_str(&format!(
        "- red: wifi {} B · móvil {} B · ethernet {} B\n",
        t["wifi_bytes"].as_u64().unwrap_or(0),
        t["movil_bytes"].as_u64().unwrap_or(0),
        t["ethernet_bytes"].as_u64().unwrap_or(0),
    ));
    let inst = &v["instrumentacion"];
    s.push_str(&format!(
        "- instrumentación: {} líneas, {} con retorno, {} malformadas\n",
        inst["lineas"].as_u64().unwrap_or(0),
        inst["eventos_con_retorno"].as_u64().unwrap_or(0),
        inst["malformadas"].as_u64().unwrap_or(0),
    ));
    s.push_str(&format!(
        "- conexión: `{}`\n",
        v["conexion"]["quick"].as_str().unwrap_or("(sin túnel)")
    ));
    s
}

/// Publica la telemetría en Drive (subida **nativa**, sin `curl`).
///
/// Si la subida falla, el estado **queda en disco** y la función devuelve el
/// error: el siguiente ciclo reintenta.  Nunca se pierde el dato.
pub fn publicar(state_dir: Option<&str>) -> Result<Value, String> {
    let sd = state_dir.map(|s| s.to_string());
    let v = recoger(sd.as_deref());
    let (json_p, md_p) = escribir_local(&v, sd.as_deref())?;

    let base = sd
        .clone()
        .unwrap_or_else(|| crate::state_dir::state_dir());
    let vfs = format!("{CARPETA_VFS}/{}", nodo());
    let stamp = v["ts"].as_u64().unwrap_or(0);

    let j = crate::drive_vfs::write_local_file(&json_p, &vfs, &format!("t-{stamp}.json"), sd.as_deref())
        .map_err(|e| format!("subir json a Drive: {e}"))?;
    let m = crate::drive_vfs::write_local_file(&md_p, &vfs, &format!("t-{stamp}.md"), sd.as_deref())
        .map_err(|e| format!("subir md a Drive: {e}"))?;

    // Índice local: el último estado, sin listar la carpeta.
    let dir = dir_local(sd.as_deref());
    let indice = dir.join("indice.json");
    let _ = std::fs::write(
        &indice,
        serde_json::to_vec_pretty(&json!({
            "ts": stamp,
            "nodo": nodo(),
            "json_file_id": j.get("file_id").cloned().unwrap_or(Value::Null),
            "md_file_id": m.get("file_id").cloned().unwrap_or(Value::Null),
            "vfs": vfs,
            "state_dir": base,
        }))
        .unwrap_or_default(),
    );

    Ok(json!({
        "ok": true,
        "vfs": vfs,
        "json": j,
        "md": m,
        "local": json_p.display().to_string(),
    }))
}

/// Publica los **datos de conexión** (dónde está este nodo).
///
/// Es lo que permite al root conectarse **sin descubrir el túnel**: lee este
/// fichero de Drive y ya sabe la URL, el puerto y el build.
pub fn publicar_conexion(state_dir: Option<&str>) -> Result<Value, String> {
    let sd = state_dir.map(|s| s.to_string());
    let conexion = datos_conexion(sd.as_deref());
    let listo = estado_listo(sd.as_deref());

    let dir = dir_local(sd.as_deref());
    std::fs::create_dir_all(&dir).map_err(|e| format!("crear {}: {e}", dir.display()))?;
    let p = dir.join("conexion.json");
    std::fs::write(
        &p,
        serde_json::to_vec_pretty(&json!({
            "conexion": conexion,
            "listo": listo,
        }))
        .unwrap_or_default(),
    )
    .map_err(|e| format!("escribir {}: {e}", p.display()))?;

    let vfs = format!("{CARPETA_CONEXION}/{}", nodo());
    let up = crate::drive_vfs::write_local_file(&p, &vfs, "conexion.json", sd.as_deref())
        .map_err(|e| format!("subir conexion a Drive: {e}"))?;

    Ok(json!({
        "ok": true,
        "vfs": vfs,
        "file_id": up.get("file_id").cloned().unwrap_or(Value::Null),
        "conexion": conexion,
        "listo": listo,
    }))
}

/// Sube un artefacto (binario, NDJSON, informe…) a Drive.
///
/// Es la vía para **subir instrumentación sin pasar por GitHub**: el repo es
/// público, así que un NDJSON con datos del dispositivo no debe acabar ahí.
pub fn subir_artefacto(
    ruta: &str,
    nombre: Option<&str>,
    carpeta: Option<&str>,
    state_dir: Option<&str>,
) -> Result<Value, String> {
    let p = Path::new(ruta);
    if !p.exists() {
        return Err(format!("no existe: {ruta}"));
    }
    let nombre = nombre
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            p.file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| "artefacto.bin".to_string())
        });
    let vfs = carpeta
        .map(|c| c.to_string())
        .unwrap_or_else(|| format!("{CARPETA_VFS}/{}", nodo()));
    let up = crate::drive_vfs::write_local_file(p, &vfs, &nombre, state_dir)
        .map_err(|e| format!("subir {nombre} a Drive: {e}"))?;
    Ok(json!({
        "ok": true,
        "vfs": vfs,
        "nombre": nombre,
        "file_id": up.get("file_id").cloned().unwrap_or(Value::Null),
        "bytes": up.get("bytes").cloned().unwrap_or(Value::Null),
        "sha256_local": sha256_fichero(p),
    }))
}

/// Hilo de publicación periódica (telemetría + conexión).
///
/// El periodo se **duplica** en modo ahorro: con la batería al 14 % y sin
/// cargador, el gasto de red es lo que más duele.
pub fn publicar_periodico(minutos: u64) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("telemetria-drive".to_string())
        .spawn(move || {
            let base = std::time::Duration::from_secs(minutos.max(1) * 60);
            loop {
                let ahorro = crate::battery_advanced::modo_ahorro_activo();
                match publicar(None) {
                    Ok(_) => eprintln!("telemetria: publicada en Drive"),
                    Err(e) => eprintln!("telemetria: no se pudo publicar: {e}"),
                }
                match publicar_conexion(None) {
                    Ok(_) => eprintln!("telemetria: conexion publicada"),
                    Err(e) => eprintln!("telemetria: conexion no publicada: {e}"),
                }
                std::thread::sleep(if ahorro { base * 2 } else { base });
            }
        })
        .expect("hilo de telemetria")
}

// ========================================================================= //
// TOOL MCP
// ========================================================================= //

/// Definición de la herramienta MCP.
pub fn tool_definitions() -> Value {
    json!([
        {
            "name": "telemetria",
            "description": "Telemetría del nodo: estado, red desglosada por transporte (wifi vs movil), batería, instrumentación viva y auto-detección de 'listo'. Acciones: estado | publicar | conexion | listo | sonda | artefacto.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["estado", "publicar", "conexion", "listo", "sonda", "artefacto"],
                        "default": "estado"
                    },
                    "url": { "type": "string", "description": "URL a sondear (acción sonda)." },
                    "ruta": { "type": "string", "description": "Ruta local del artefacto a subir (acción artefacto)." },
                    "nombre": { "type": "string", "description": "Nombre en Drive del artefacto (acción artefacto)." },
                    "carpeta": { "type": "string", "description": "Carpeta VFS de destino (acción artefacto)." }
                },
                "required": ["action"]
            }
        }
    ])
}

/// Despacha la herramienta MCP.
pub fn dispatch(args: &Value) -> Value {
    let s = |k: &str| {
        args.get(k)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    };
    match s("action").as_str() {
        "estado" | "" => recoger(None),
        "publicar" => match publicar(None) {
            Ok(v) => v,
            Err(e) => json!({"ok": false, "error": e}),
        },
        "conexion" => match publicar_conexion(None) {
            Ok(v) => v,
            Err(e) => json!({"ok": false, "error": e}),
        },
        "listo" => estado_listo(None),
        "sonda" => sonda_tunel(&s("url")),
        "artefacto" => {
            let r = s("ruta");
            let nombre = s("nombre");
            let carpeta = s("carpeta");
            if r.is_empty() {
                json!({"ok": false, "error": "falta la ruta del artefacto"})
            } else {
                let opt_n = if nombre.is_empty() { None } else { Some(nombre.as_str()) };
                let opt_c = if carpeta.is_empty() { None } else { Some(carpeta.as_str()) };
                match subir_artefacto(&r, opt_n, opt_c, None) {
                    Ok(v) => v,
                    Err(e) => json!({"ok": false, "error": e}),
                }
            }
        }
        otra => json!({"ok": false, "error": format!("acción desconocida: {otra}")}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clasifica_las_interfaces_bien() {
        assert_eq!(clasificar_interfaz("wlan0"), "wifi");
        assert_eq!(clasificar_interfaz("wlan1"), "wifi");
        assert_eq!(clasificar_interfaz("rmnet_data0"), "movil");
        assert_eq!(clasificar_interfaz("ccmni0"), "movil");
        assert_eq!(clasificar_interfaz("eth0"), "ethernet");
        assert_eq!(clasificar_interfaz("ap0"), "otra");
    }

    #[test]
    fn parsea_net_dev_separando_rx_y_tx() {
        let texto = "Inter-|   Receive                                                |  Transmit\n face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed\n wlan0: 1000 10 0 0 0 0 0 0 2000 20 0 0 0 0 0 0\n rmnet_data0: 300 3 0 0 0 0 0 0 400 4 0 0 0 0 0 0\n";
        let v = parsear_net_dev(texto);
        assert_eq!(v["wlan0"]["rx_bytes"], 1000);
        assert_eq!(v["wlan0"]["tx_bytes"], 2000);
        assert_eq!(v["rmnet_data0"]["rx_bytes"], 300);
        // `lo` se ignora.
        assert!(v.get("lo").is_none());
    }

    #[test]
    fn el_desglose_separa_wifi_de_movil() {
        let net = json!({
            "wlan0": {"rx_bytes": 1000, "tx_bytes": 2000},
            "rmnet_data0": {"rx_bytes": 300, "tx_bytes": 400},
            "eth0": {"rx_bytes": 5, "tx_bytes": 5},
        });
        let d = desglose_por_transporte(&net);
        assert_eq!(d["wifi_bytes"], 3000);
        assert_eq!(d["movil_bytes"], 700);
        assert_eq!(d["ethernet_bytes"], 10);
    }

    #[test]
    fn el_sha16_depende_del_contenido_y_no_lo_filtra() {
        let a = sha16_real("secreto");
        let b = sha16_real("secreto");
        let c = sha16_real("otro");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 16);
        assert!(!a.contains("secreto"));
    }

    #[test]
    fn el_markdown_menciona_lo_importante() {
        let v = json!({
            "ts": 1, "nodo": "android", "build": "abc1234",
            "bateria": {"capacidad_pct": 55, "estado": "Discharging"},
            "carga": {"load1": 1.5},
            "red": {"por_transporte": {"wifi_bytes": 10, "movil_bytes": 20, "ethernet_bytes": 0}},
            "instrumentacion": {"lineas": 5, "eventos_con_retorno": 2, "malformadas": 0},
            "conexion": {"quick": "https://x.trycloudflare.com"},
        });
        let md = a_markdown(&v);
        assert!(md.contains("android"));
        assert!(md.contains("batería"));
        // El Markdown usa "móvil" con tilde (el test miraba "movil" sin
        // ella y fallaba en el CI del 2026-11-26).
        assert!(md.contains("móvil"), "{}", md);
        assert!(md.contains("https://x.trycloudflare.com"));
    }

    #[test]
    fn recoger_devuelve_un_json_con_las_claves_principales() {
        let v = recoger(None);
        assert!(v.is_object());
        assert!(v.get("ts").is_some());
        assert!(v.get("nodo").is_some());
        assert!(v.get("red").is_some());
        assert!(v.get("instrumentacion").is_some());
        assert!(v.get("conexion").is_some());
    }

    #[test]
    fn los_datos_de_conexion_no_llevan_claves_en_claro() {
        let c = datos_conexion(None);
        let txt = serde_json::to_string(&c).unwrap_or_default();
        // Ni la API key ni el token de sesión pueden aparecer.
        assert!(!txt.contains("X-Seere-Auth-Token"));
        assert!(!txt.contains("X-API-Key"));
        // Los hashes sí, con su nombre.
        assert!(c.get("claves_sha16").is_some());
        assert!(c.get("canonical").is_some());
    }

    #[test]
    fn la_tool_tiene_esquema_valido() {
        let d = tool_definitions();
        let arr = d.as_array().expect("array");
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["name"], "telemetria");
        assert_eq!(arr[0]["inputSchema"]["type"], "object");
    }

    #[test]
    fn sonda_sin_url_no_falla_con_panic() {
        let v = sonda_tunel("");
        assert_eq!(v["ok"], false);
        assert_eq!(v["codigo"], 0);
    }
}
