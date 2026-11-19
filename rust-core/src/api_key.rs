//! Middleware de API key para el cockpit (Fase 4 del prompt maestro).
//!
//! Objetivo: que el cockpit pueda exponerse fuera de `localhost` (por un
//! túnel, un reverse proxy o directamente en la LAN) **exigiendo un
//! identificador**, sin por ello romper el uso local ni la UI.
//!
//! Reglas:
//! * la key se toma de `MEGADAEMON_API_KEY` o de `<STATE_DIR>/megadaemon_api.key`
//!   (permisos 600).  Si no hay ninguna, el middleware queda **inactivo**
//!   (el cockpit se comporta como hasta ahora: fail-closed del token local);
//! * cabecera aceptada: `X-API-Key: <key>` o `Authorization: Bearer <key>`;
//! * **loopback exento**: 127.0.0.1 / ::1 pasan sin key (uso local, la UI y
//!   el reverse proxy en la misma máquina);
//! * petición externa **sin** key → `401`;
//! * petición externa **con key incorrecta** → `403` + registro de auditoría
//!   (IP, timestamp, user-agent, path);
//! * rate limit: 60 peticiones/minuto por IP externa; al superarlo → `429`.
//!
//! La comparación de keys es de **tiempo constante**: un `==` normal filtra
//! información por temporización y la key es el único control de acceso
//! cuando el servicio está expuesto a internet.

use std::collections::HashMap;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Ventana del rate limit (1 minuto) y máximo de peticiones por IP.
const RATE_WINDOW_SECS: u64 = 60;
const RATE_MAX_REQUESTS: u32 = 60;

/// Resultado de la comprobación de acceso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Pasa.
    Allow,
    /// Sin cabecera de key y la IP no es de loopback.
    Unauthorized,
    /// Key presente pero incorrecta.
    Forbidden,
    /// Se ha superado el rate limit de la IP.
    RateLimited,
}

/// Configuración del middleware.
#[derive(Debug, Clone, Default)]
pub struct ApiKeyConfig {
    /// Key activa (vacía = middleware inactivo).
    pub key: String,
    /// Si está activa, se exige fuera de loopback.
    pub enabled: bool,
}

impl ApiKeyConfig {
    /// Carga la configuración: env primero, luego fichero.
    pub fn load(state_dir: &str) -> Self {
        if let Ok(k) = std::env::var("MEGADAEMON_API_KEY") {
            let k = k.trim().to_string();
            if !k.is_empty() {
                return Self { key: k, enabled: true };
            }
        }
        let path = format!("{}/megadaemon_api.key", state_dir);
        if let Ok(existing) = fs::read_to_string(&path) {
            let k = existing.trim().to_string();
            if k.len() >= 16 {
                return Self { key: k, enabled: true };
            }
        }
        Self::default()
    }

    /// Crea una key nueva con el prefijo de la casa y la deja en el disco
    /// con permisos 600.
    pub fn generate_and_store(state_dir: &str) -> Result<String, String> {
        let key = format!("mdaemon_sk_live_{}", random_hex(32));
        let path = format!("{}/megadaemon_api.key", state_dir);
        fs::create_dir_all(state_dir).map_err(|e| format!("mkdir {}: {}", state_dir, e))?;
        fs::write(&path, format!("{}\n", key)).map_err(|e| format!("write {}: {}", path, e))?;
        set_mode_600(&path);
        Ok(key)
    }
}

/// Octetos aleatorios de /dev/urandom (fallback: reloj + pid).
fn random_hex(nbytes: usize) -> String {
    let mut buf = vec![0u8; nbytes];
    if let Ok(mut f) = fs::File::open("/dev/urandom") {
        use std::io::Read;
        let _ = f.read_exact(&mut buf);
    } else {
        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        buf[..8].copy_from_slice(&t.to_le_bytes());
    }
    buf.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(unix)]
fn set_mode_600(path: &str) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = fs::metadata(path) {
        let mut perm = meta.permissions();
        perm.set_mode(0o600);
        let _ = fs::set_permissions(path, perm);
    }
}

#[cfg(not(unix))]
fn set_mode_600(_path: &str) {}

/// ¿La IP es de loopback (exenta de key)?
pub fn is_loopback(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => *v4 == Ipv4Addr::LOCALHOST || v4.octets()[0] == 127,
        IpAddr::V6(v6) => *v6 == Ipv6Addr::LOCALHOST || v6.is_loopback(),
    }
}

/// Extrae la key de las cabeceras (`X-API-Key` o `Authorization: Bearer`).
pub fn extract_key(headers: &str) -> Option<String> {
    let mut found: Option<String> = None;
    for line in headers.lines() {
        let low = line.to_ascii_lowercase();
        if let Some(rest) = low.strip_prefix("x-api-key:") {
            let _ = rest;
            // worked on `low`, but el valor se toma de la línea original
            if let Some(pos) = line.find(':') {
                found = Some(line[pos + 1..].trim().to_string());
            }
        } else if low.starts_with("authorization:") {
            if let Some(pos) = line.find(':') {
                let v = line[pos + 1..].trim();
                let v = v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer ")).unwrap_or(v);
                found = Some(v.trim().to_string());
            }
        }
    }
    found.filter(|s| !s.is_empty())
}

/// Comparación en tiempo constante (evita fugas por temporización).
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

/// Estado del rate limit por IP.
#[derive(Default)]
pub struct RateLimiter {
    buckets: Mutex<HashMap<IpAddr, (Instant, u32)>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Cuenta una petición de `ip` y devuelve `true` si está dentro del límite.
    pub fn allow(&self, ip: &IpAddr, now: Instant) -> bool {
        let mut map = match self.buckets.lock() {
            Ok(m) => m,
            Err(p) => p.into_inner(),
        };
        let entry = map.entry(*ip).or_insert((now, 0));
        if now.duration_since(entry.0).as_secs() >= RATE_WINDOW_SECS {
            *entry = (now, 1);
            return true;
        }
        if entry.1 >= RATE_MAX_REQUESTS {
            return false;
        }
        entry.1 += 1;
        true
    }

    /// Peticiones admitidas en la ventana actual para `ip`.
    pub fn count(&self, ip: &IpAddr) -> u32 {
        let map = match self.buckets.lock() {
            Ok(m) => m,
            Err(p) => p.into_inner(),
        };
        map.get(ip).map(|(_, c)| *c).unwrap_or(0)
    }
}

/// Decide si una petición pasa, aplicando key + rate limit.
pub fn decide(
    cfg: &ApiKeyConfig,
    limiter: &RateLimiter,
    peer: &IpAddr,
    headers: &str,
    now: Instant,
) -> Decision {
    if !cfg.enabled {
        return Decision::Allow;
    }
    // Loopback siempre exento: uso local (UI, reverse proxy en la misma caja).
    if is_loopback(peer) {
        return Decision::Allow;
    }
    match extract_key(headers) {
        None => {
            // Sin key: se cuenta igual para que el rate limit también
            //protege el endpoint de fuerza bruta.
            let _ = limiter.allow(peer, now);
            Decision::Unauthorized
        }
        Some(k) => {
            if !limiter.allow(peer, now) {
                return Decision::RateLimited;
            }
            if constant_time_eq(&k, &cfg.key) {
                Decision::Allow
            } else {
                Decision::Forbidden
            }
        }
    }
}

/// Línea de auditoría para una petición rechazada.
pub fn audit_line(
    peer: &IpAddr,
    path: &str,
    user_agent: &str,
    reason: &str,
) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!(
        "{{\"ts\":{},\"ip\":\"{}\",\"path\":\"{}\",\"ua\":\"{}\",\"reason\":\"{}\"}}",
        ts, peer, path, user_agent, reason
    )
}

/// Extrae el `User-Agent` de las cabeceras.
pub fn user_agent(headers: &str) -> String {
    for line in headers.lines() {
        let low = line.to_ascii_lowercase();
        if low.starts_with("user-agent:") {
            if let Some(pos) = line.find(':') {
                return line[pos + 1..].trim().to_string();
            }
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip4(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, b, c, d))
    }

    #[test]
    fn loopback_exento() {
        let cfg = ApiKeyConfig { key: "k".into(), enabled: true };
        let lim = RateLimiter::new();
        assert_eq!(decide(&cfg, &lim, &ip4(127, 0, 0, 1), "", Instant::now()), Decision::Allow);
        assert_eq!(
            decide(&cfg, &lim, &IpAddr::V6(Ipv6Addr::LOCALHOST), "", Instant::now()),
            Decision::Allow
        );
    }

    #[test]
    fn sin_key_401_invalida_403() {
        let cfg = ApiKeyConfig { key: "secreta".into(), enabled: true };
        let lim = RateLimiter::new();
        let ext = ip4(203, 0, 113, 9);
        assert_eq!(decide(&cfg, &lim, &ext, "", Instant::now()), Decision::Unauthorized);
        assert_eq!(
            decide(&cfg, &lim, &ext, "X-API-Key: otra\r\n", Instant::now()),
            Decision::Forbidden
        );
        assert_eq!(
            decide(&cfg, &lim, &ext, "X-API-Key: secreta\r\n", Instant::now()),
            Decision::Allow
        );
    }

    #[test]
    fn bearer_tambien_sirve() {
        let cfg = ApiKeyConfig { key: "secreta".into(), enabled: true };
        let lim = RateLimiter::new();
        let ext = ip4(198, 51, 100, 7);
        assert_eq!(
            decide(&cfg, &lim, &ext, "Authorization: Bearer secreta\r\n", Instant::now()),
            Decision::Allow
        );
    }

    #[test]
    fn rate_limit_60_por_minuto() {
        let cfg = ApiKeyConfig { key: "secreta".into(), enabled: true };
        let lim = RateLimiter::new();
        let ext = ip4(192, 0, 2, 55);
        let now = Instant::now();
        for i in 0..60 {
            assert_eq!(
                decide(&cfg, &lim, &ext, "X-API-Key: secreta\r\n", now),
                Decision::Allow,
                "petición {} debería pasar",
                i
            );
        }
        assert_eq!(
            decide(&cfg, &lim, &ext, "X-API-Key: secreta\r\n", now),
            Decision::RateLimited
        );
        // Tras pasar la ventana, vuelve a admitirse.
        let later = now + std::time::Duration::from_secs(RATE_WINDOW_SECS + 1);
        assert_eq!(
            decide(&cfg, &lim, &ext, "X-API-Key: secreta\r\n", later),
            Decision::Allow
        );
    }

    #[test]
    fn middleware_inactivo_sin_key() {
        let cfg = ApiKeyConfig::default();
        let lim = RateLimiter::new();
        assert_eq!(
            decide(&cfg, &lim, &ip4(203, 0, 113, 9), "", Instant::now()),
            Decision::Allow
        );
    }

    #[test]
    fn comparacion_en_tiempo_constante() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "abcd"));
        assert!(constant_time_eq("", ""));
    }

    #[test]
    fn extraccion_de_cabeceras() {
        assert_eq!(extract_key("X-API-Key: abc\r\n").as_deref(), Some("abc"));
        assert_eq!(extract_key("authorization: Bearer xyz\r\n").as_deref(), Some("xyz"));
        assert_eq!(extract_key("x-api-key:  \r\n"), None);
        assert_eq!(extract_key("Host: x\r\n"), None);
    }
}
