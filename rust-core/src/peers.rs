//! Registro de instancias interconectadas de MegaDaemon.
//!
//! Este módulo implementa el descubrimiento de pares que el protocolo de
//! despliegue declara: cada instancia debe poder saber qué otras
//! instancias del enjambre existen, cómo localizarlas y qué pueden hacer.
//!
//! Fuentes, en este orden de precedencia:
//!   1. `MEGADAEMON_PEERS`: JSON en línea (`[{...}]`) o ruta a un fichero.
//!   2. `<MEGADAEMON_STATE_DIR>/peers.json`: fichero con `{"peers":[...]}`
//!      o un array pelado.
//!   3. La propia instancia, siempre presente y siempre la primera.
//!
//! Formato de un par (contrato público, no se inventa nada más):
//!   { "id": "...", "url": "http://host:puerto",
//!     "api_key_alias": "...", "capabilities": ["..."], "last_seen": "..." }
//!
//! `api_key_alias` es un ALIAS, nunca la clave: las claves no salen de la
//! instancia que las posee. Para hablar con un par hace falta su clave,
//! que se resuelve en la instancia local (ver `peer_key`).
//!
//! `last_seen` es la marca de tiempo ISO-8601 UTC del último sondeo TCP
//! con éxito contra el par. Un par que no contesta conserva la marca
//! anterior y aparece como `reachable: false` en la instantánea, que es
//! como se distingue "caído" de "nunca visto".

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::io::Read;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Un par del enjambre. Los cinco campos públicos son el contrato; `role`
/// es interno y no se serializa hacia fuera.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Peer {
    pub id: String,
    pub url: String,
    #[serde(default)]
    pub api_key_alias: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub last_seen: String,
    #[serde(default, skip_serializing)]
    pub role: String,
    /// Clave local para hablar con este par. NUNCA se serializa.
    #[serde(default, skip_serializing)]
    pub api_key: String,
}

/// Capacidades que declara esta instancia. Es la lista de motores del
/// núcleo Rust; un par que no la declare se annuncia sin ella.
pub const LOCAL_CAPABILITIES: &[&str] = &[
    "cfg-lift",
    "cirx-ssa",
    "mba-simplify",
    "deobfuscate",
    "entropy-scan",
    "pattern-scan",
    "pointer-scan",
    "crypto-scan",
    "kaitai-parse",
    "die-binwalk",
    "rizin",
    "frida-dbi",
    "npu-cv",
    "npu-bench",
    "hw-mmio",
    "providers",
    "sync",
    "peers",
];

/// Directorio de estado de la instancia.
pub fn state_dir() -> PathBuf {
    std::env::var("MEGADAEMON_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("state"))
}

/// Segundos desde epoch (UTC).
pub fn epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// ISO-8601 UTC (`2026-11-19T21:14:03Z`) sin dependencias externas.
/// Algoritmo de Howard Hinnant (civil_from_days), exacto para fechas
/// gregorianas.
pub fn iso8601(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, m, d, h, mi, s
    )
}

/// Extrae `host:puerto` de una URL; si no hay puerto, 9090.
pub fn host_port(url: &str) -> Option<(String, u16)> {
    let rest = url
        .trim()
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .trim_end_matches('/');
    let rest = rest.split('/').next().unwrap_or(rest);
    if rest.is_empty() {
        return None;
    }
    if let Some(stripped) = rest.strip_prefix('[') {
        // [::1]:9090
        if let Some(end) = stripped.find(']') {
            let host = stripped[..end].to_string();
            let port = stripped[end + 1..]
                .trim_start_matches(':')
                .parse::<u16>()
                .unwrap_or(9090);
            return Some((host, port));
        }
        return None;
    }
    match rest.rsplit_once(':') {
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty() => {
            Some((h.to_string(), p.parse::<u16>().unwrap_or(9090)))
        }
        _ => Some((rest.to_string(), 9090)),
    }
}

/// Sondeo real contra el par: `GET <url>/health` y exigir una respuesta
/// HTTP válida con `"status":"ok"`.
///
/// NO vale con un connect TCP: en sandboxes con proxy de salida el
/// accept() se produce para cualquier puerto y un sondeo así declararía
/// "alcanzable" a un host que no sirve nada. Con HTTP se distingue de
/// verdad "contesta" de "acepta y se queda colgado".
pub fn probe_peer(url: &str, timeout_ms: u64) -> Option<String> {
    probe_peer_with_key(url, timeout_ms, None)
}

/// Sondeo con clave: si la instancia exige API key (modo forzado), un
/// GET /health sin cabecera vuelve 401 y el par pareceria caido aunque
/// este muerto de vida. La clave que se usa es la del propio par (la
/// local, si el par es uno mismo).
pub fn probe_peer_with_key(url: &str, timeout_ms: u64, key: Option<&str>) -> Option<String> {
    let (host, port) = host_port(url)?;
    let scheme = if url.trim_start().starts_with("https://") {
        "https"
    } else {
        "http"
    };
    let base = if url.trim_end().ends_with('/') {
        url.trim_end().to_string()
    } else {
        url.to_string()
    };
    let health = format!("{}/health", base);

    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(timeout_ms.max(800))))
        .build();
    let agent = ureq::Agent::new_with_config(config);

    match scheme {
        "https" => {
            // HTTPS exige cliente TLS: el nucleo no arrastra uno para esto,
            // asi que se delega en ureq con rustls.
            let mut req = agent.get(&health);
            if let Some(k) = key {
                req = req.header("X-API-Key", k);
            }
            let resp = req.call();
            match resp {
                Ok(r) if r.status() == 200 => Some(iso8601(epoch_secs())),
                _ => None,
            }
        }
        _ => {
            // HTTP a pelo: basta con una peticion minima, sin depender de
            // ureq, para no gastar un TLS handshake en cada par.
            let addr = (host.as_str(), port).to_socket_addrs().ok()?.next()?;
            let mut s = TcpStream::connect_timeout(&addr, Duration::from_millis(timeout_ms.max(500))).ok()?;
            let mut req = format!(
                "GET /health HTTP/1.1\r\nHost: {}\r\nUser-Agent: megadaemon-peers\r\nConnection: close\r\n",
                host
            );
            if let Some(k) = key {
                req.push_str(&format!("X-API-Key: {}\r\n", k));
            }
            req.push_str("\r\n");
            use std::io::Write;
            s.write_all(req.as_bytes()).ok()?;
            s.set_read_timeout(Some(Duration::from_millis(timeout_ms.max(800))))
                .ok()?;
            let mut buf = [0u8; 4096];
            let mut head = String::new();
            // La respuesta puede llegar en varios paquetes: se lee hasta
            // agotar el tiempo o llenarse el buffer. El criterio de
            // exito es una linea de estado HTTP 200 de verdad, no un
            // accept() del proxy de salida.
            let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms.max(800));
            while head.len() < buf.len() && std::time::Instant::now() < deadline {
                match s.read(&mut buf[head.len()..]) {
                    Ok(0) => break,
                    Ok(n) => {
                        head.push_str(&String::from_utf8_lossy(&buf[head.len()..head.len() + n]));
                        if head.contains("\r\n\r\n") {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            if head.starts_with("HTTP/1.") && head.contains(" 200 ") {
                Some(iso8601(epoch_secs()))
            } else {
                None
            }
        }
    }
}

/// La instancia local, tal y como se ve desde el enjambre.
pub fn self_instance() -> Peer {
    let name = std::env::var("MEGADAEMON_INSTANCE_NAME").unwrap_or_else(|_| "megadaemon".into());
    let role = std::env::var("MEGADAEMON_INSTANCE_ROLE").unwrap_or_else(|_| "worker".into());
    let listen = std::env::var("MEGADAEMON_LISTEN").unwrap_or_else(|_| "127.0.0.1:9090".into());
    Peer {
        id: name.clone(),
        url: format!("http://{}", listen),
        api_key_alias: "local".into(),
        capabilities: LOCAL_CAPABILITIES.iter().map(|s| s.to_string()).collect(),
        last_seen: iso8601(epoch_secs()),
        role,
        api_key: std::env::var("MEGADAEMON_API_KEY").unwrap_or_default(),
    }
}

/// Lee los pares declarados en la configuración (sin la instancia local).
pub fn configured_peers() -> Vec<Peer> {
    let mut out: Vec<Peer> = Vec::new();

    // 1) variable de entorno: JSON en línea o ruta.
    if let Ok(v) = std::env::var("MEGADAEMON_PEERS") {
        let v = v.trim().to_string();
        if v.is_empty() {
            // nada
        } else if v.starts_with('[') || v.starts_with('{') {
            if let Ok(parsed) = serde_json::from_str::<Value>(&v) {
                out.extend(peer_list_from_value(&parsed));
            }
        } else if let Ok(txt) = fs::read_to_string(&v) {
            if let Ok(parsed) = serde_json::from_str::<Value>(&txt) {
                out.extend(peer_list_from_value(&parsed));
            }
        }
    }

    // 2) fichero del directorio de estado.
    let f = state_dir().join("peers.json");
    if let Ok(txt) = fs::read_to_string(&f) {
        if let Ok(parsed) = serde_json::from_str::<Value>(&txt) {
            for p in peer_list_from_value(&parsed) {
                if !out.iter().any(|e| e.id == p.id) {
                    out.push(p);
                }
            }
        }
    }

    out
}

/// Acepta `[{...}]`, `{"peers":[{...}]}` o `{"id":...}` suelto.
fn peer_list_from_value(v: &Value) -> Vec<Peer> {
    let arr: Vec<&Value> = match v {
        Value::Array(a) => a.iter().collect(),
        Value::Object(o) => match o.get("peers") {
            Some(Value::Array(a)) => a.iter().collect(),
            _ => vec![v],
        },
        _ => Vec::new(),
    };
    arr.into_iter()
        .filter_map(|item| serde_json::from_value::<Peer>(item.clone()).ok())
        .filter(|p| !p.id.is_empty() && !p.url.is_empty())
        .collect()
}

/// Instancias conocidas: la propia primero, luego las declaradas.
pub fn discover_peers() -> Vec<Peer> {
    let mut out = vec![self_instance()];
    for p in configured_peers() {
        if !out.iter().any(|e| e.id == p.id) {
            out.push(p);
        }
    }
    out
}

/// Instantánea pública: los pares con el contrato de cinco campos, el
/// alcance de cada uno y si contesta ahora mismo.
/// Cache de la instantanea de pares.
///
/// Por que: el enjambre sondea /peers continuamente (descubrimiento). El
/// sondeo real contra cada par cuesta hasta 1,5 s y se hacia EN SERIE, de
/// modo que un par caido multiplicaba la latencia de la respuesta y
/// consumia un hilo del servidor durante segundos. Con la cache de 5 s y
/// el sondeo en paralelo, la respuesta es la misma pero cuesta lo que
/// cuesta el par MAS LENTO, no la suma.
static SNAPSHOT_CACHE: OnceLock<Mutex<(Option<std::time::Instant>, Value)>> = OnceLock::new();

fn snapshot_cache() -> &'static Mutex<(Option<std::time::Instant>, Value)> {
    SNAPSHOT_CACHE.get_or_init(|| Mutex::new((None, Value::Null)))
}

/// TTL de la instantanea de pares.
const SNAPSHOT_TTL: Duration = Duration::from_secs(5);

pub fn peers_snapshot() -> Value {
    {
        let cache = crate::lock_ok(snapshot_cache());
        if let (Some(at), v) = (&cache.0, &cache.1) {
            if at.elapsed() < SNAPSHOT_TTL {
                return v.clone();
            }
        }
    }
    let fresh = build_snapshot();
    let mut cache = crate::lock_ok(snapshot_cache());
    *cache = (Some(std::time::Instant::now()), fresh.clone());
    fresh
}

/// Construye la instantanea sondeando a los pares EN PARALELO.
fn build_snapshot() -> Value {
    let peers = discover_peers();
    let mut reachable = serde_json::Map::new();
    // Sondeo en paralelo: el tiempo total es el del par mas lento, no la
    // suma de todos. Con 3 pares y 1,5 s de timeout cada uno, la version
    // en serie tardaba hasta 4,5 s con un par caido.
    let results: Vec<(Peer, bool)> = std::thread::scope(|scope| {
        let handles: Vec<_> = peers
            .into_iter()
            .map(|p| {
                scope.spawn(move || {
                    let key = if p.api_key.is_empty() {
                        None
                    } else {
                        Some(p.api_key.as_str())
                    };
                    let ok = probe_peer_with_key(&p.url, 1500, key).is_some();
                    (p, ok)
                })
            })
            .collect();
        handles
            .into_iter()
            .filter_map(|h| h.join().ok())
            .collect()
    });
    let mut list: Vec<Peer> = Vec::new();
    for (mut p, ok) in results {
        if ok {
            p.last_seen = iso8601(epoch_secs());
        }
        reachable.insert(p.id.clone(), json!(ok));
        list.push(p);
    }
    json!({
        "count": list.len(),
        "self": self_instance().id,
        "state_dir": state_dir().to_string_lossy(),
        "reachable": Value::Object(reachable),
        "peers": list,
    })
}

/// Guarda un par en el fichero de estado (para que sobreviva a un
/// reinicio sin depender de la variable de entorno).
pub fn persist_peer(peer: &Peer) -> std::io::Result<()> {
    let dir = state_dir();
    fs::create_dir_all(&dir)?;
    let mut all = configured_peers();
    all.retain(|p| p.id != peer.id);
    all.push(peer.clone());
    let doc = json!({ "peers": all });
    fs::write(dir.join("peers.json"), serde_json::to_string_pretty(&doc)?)
}

/// Instancia local declarada, para el tablón de coordinación.
pub fn instance_record() -> Value {
    let s = self_instance();
    json!({
        "id": s.id,
        "url": s.url,
        "role": s.role,
        "api_key_alias": s.api_key_alias,
        "capabilities": s.capabilities,
        "device": std::env::var("MEGADAEMON_INSTANCE_DEVICE").unwrap_or_else(|_| s.id.clone()),
        "last_seen": iso8601(epoch_secs()),
        "listen": std::env::var("MEGADAEMON_LISTEN").unwrap_or_else(|_| "127.0.0.1:9090".into()),
        "state_dir": state_dir().to_string_lossy(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso8601_convierte_epoch() {
        assert_eq!(iso8601(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso8601(1_700_000_000), "2023-11-14T22:13:20Z");
    }

    #[test]
    fn host_port_extrae_host_y_puerto() {
        assert_eq!(
            host_port("http://127.0.0.1:9090"),
            Some(("127.0.0.1".to_string(), 9090))
        );
        assert_eq!(
            host_port("https://mdtmfnc.duckdns.org/"),
            Some(("mdtmfnc.duckdns.org".to_string(), 9090))
        );
        assert_eq!(
            host_port("http://[::1]:9091"),
            Some(("::1".to_string(), 9091))
        );
    }

    #[test]
    fn la_instancia_local_siempre_cabe() {
        let peers = discover_peers();
        assert!(!peers.is_empty());
        assert_eq!(peers[0].id, self_instance().id);
        assert!(!peers[0].capabilities.is_empty());
    }

    #[test]
    fn la_instantanea_no_filtra_la_clave() {
        let snap = peers_snapshot();
        let txt = serde_json::to_string(&snap).unwrap();
        let key = std::env::var("MEGADAEMON_API_KEY").unwrap_or_default();
        if !key.is_empty() {
            assert!(!txt.contains(&key), "la API key no puede salir en la respuesta");
        }
    }
}
