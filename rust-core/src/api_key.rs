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

/// Límite de peticiones por IP y ventana, configurable por entorno.
///
/// v14 (AUDIT-OPS-01): el límite fijo de 60/min hace que TODOS los agentes
/// que entran por el túnel Cloudflare compartan el mismo cubo (el peer
///Seen es 127.0.0.1) ⇒ 429 en cascada ⇒ los agentes leen "la instancia
/// está caída" y se rinden.  Con esto el operador puede subirlo, y el
/// loopback (UI local y proxy) queda exento por defecto.
pub fn rate_max_requests() -> u32 {
    std::env::var("MEGADAEMON_RATE_MAX")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(RATE_MAX_REQUESTS)
}

/// ¿El límite de ritmo se aplica a esta IP?
///
/// Por defecto NO se aplica al loopback: la UI local y el proxy interno no
/// deben competir por el cubo de los agentes remotos.  Se puede forzar con
/// `MEGADAEMON_RATE_LOOPBACK=1`.
pub fn rate_applies_to(ip: &IpAddr) -> bool {
    let force = std::env::var("MEGADAEMON_RATE_LOOPBACK")
        .map(|v| v.trim() == "1")
        .unwrap_or(false);
    if force {
        return true;
    }
    !is_loopback(ip)
}

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
    /// Claves adicionales por agente (una por línea en `megadaemon_api.keys`).
    ///
    /// Antes sólo existía una clave: acuñar otra invalidaba a toda la flota.
    /// Con este registro cada agente (o un agente externo) tiene la suya y
    /// puede revocarse sin tocar a los demás.
    pub extra_keys: Vec<String>,
    /// Si está activa, se exige fuera de loopback.
    pub enabled: bool,
    /// Fuerza la key incluso para loopback.
    ///
    /// Necesario cuando el cockpit está **detrás de un proxy local** (túnel
    /// Cloudflare, nginx, caddy): el proxy conecta desde 127.0.0.1, así que
    /// sin esto la exemption de loopback dejaría el servicio abierto a
    /// cualquiera que alcance el proxy.  Se activa con
    /// `MEGADAEMON_REQUIRE_API_KEY=1`.
    pub forced: bool,
}

impl ApiKeyConfig {
    /// Carga la configuración: env primero, luego fichero.
    pub fn load(state_dir: &str) -> Self {
        let forced = matches!(
            std::env::var("MEGADAEMON_REQUIRE_API_KEY").unwrap_or_default().trim(),
            "1" | "true" | "yes" | "on"
        );
        Self::load_with(state_dir, forced)
    }

    /// Igual que [`load`], pero con el flag `forced` explícito (testeable
    /// sin tocar el entorno).
    pub fn load_with(state_dir: &str, forced: bool) -> Self {
        let extra_keys = load_extra_keys(state_dir);
        if let Ok(k) = std::env::var("MEGADAEMON_API_KEY") {
            let k = k.trim().to_string();
            // v14: el env manda sólo si contiene una key REAL.  Antes una
            // basura de 5 caracteres en `megadaemon.env` (p. ej. "none")
            // tapaba la key buena del fichero de estado => el daemon
            // rechazaba a todos con "API key invalida" y el puente inyectaba
            // la misma basura.  Mismo criterio de longitud que el fichero.
            if k.len() >= 16 {
                return Self { key: k, extra_keys, enabled: true, forced };
            }
        }
        let path = format!("{}/megadaemon_api.key", state_dir);
        if let Ok(existing) = fs::read_to_string(&path) {
            let k = existing.trim().to_string();
            if k.len() >= 16 {
                return Self { key: k, extra_keys, enabled: true, forced };
            }
        }
        if forced {
            // A-04 (P0): `forced` sin key NO puede significar "sirve sin
            // credencial".  Se acuña una key y se avisa (ruidoso, pero sin
            // imprimir el secreto: sólo ruta y huella).
            match Self::generate_and_store(state_dir) {
                Ok(k) => {
                    eprintln!(
                        "[api_key] ATENCION: MEGADAEMON_REQUIRE_API_KEY=1 pero no habia ninguna key \
                         configurada; se ha acuñado una nueva en {}/megadaemon_api.key (0600), \
                         huella fnv1a64={}.  Léela del fichero: no la pegues en el chat ni en logs.",
                        state_dir,
                        fingerprint(&k)
                    );
                    return Self { key: k, extra_keys, enabled: true, forced };
                }
                Err(e) => {
                    eprintln!(
                        "[api_key] CRITICO: MEGADAEMON_REQUIRE_API_KEY=1, no hay key y no se pudo acuñar \
                         una ({}).  El daemon responde 401 a todo (fail-closed).",
                        e
                    );
                    return Self { key: String::new(), extra_keys, enabled: false, forced };
                }
            }
        }
        Self { key: String::new(), extra_keys, enabled: false, forced }
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

    /// ¿Esta clave concreta está autorizada (principal o de un agente)?
    pub fn accepts(&self, k: &str) -> bool {
        if constant_time_eq(k, &self.key) {
            return true;
        }
        self.extra_keys.iter().any(|e| constant_time_eq(k, e))
    }
}

/// Ruta del registro de claves adicionales.
pub fn extra_keys_path(state_dir: &str) -> String {
    format!("{}/megadaemon_api.keys", state_dir)
}

/// Carga las claves adicionales (una por línea; se ignoran vacíos y `#`).
fn load_extra_keys(state_dir: &str) -> Vec<String> {
    let path = extra_keys_path(state_dir);
    let Ok(txt) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    txt.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect()
}

/// Acuña una clave **nueva** para un agente y la añade al registro de claves
/// adicionales, sin tocar la clave principal: los demás agentes siguen
/// funcionando y esta se revoca quitando su línea.
pub fn generate_extra_key(state_dir: &str, label: &str) -> Result<String, String> {
    let key = format!("mdaemon_sk_live_{}", random_hex(32));
    let path = extra_keys_path(state_dir);
    fs::create_dir_all(state_dir).map_err(|e| format!("mkdir {}: {}", state_dir, e))?;
    let mut existing = fs::read_to_string(&path).unwrap_or_default();
    if !existing.is_empty() && !existing.ends_with('\n') {
        existing.push('\n');
    }
    let comment = if label.is_empty() {
        String::new()
    } else {
        format!("# {}\n", label.replace(['\n', '\r'], " "))
    };
    let mut out = String::new();
    out.push_str(&existing);
    out.push_str(&comment);
    out.push_str(&key);
    out.push('\n');
    fs::write(&path, out).map_err(|e| format!("write {}: {}", path, e))?;
    set_mode_600(&path);
    Ok(key)
}

/// Revoca una clave del registro de claves adicionales **sin tocar la clave
/// principal**: los demás agentes siguen funcionando.
///
/// `target` admite la clave completa, un prefijo de 8 caracteres o más, o la
/// etiqueta que se puso al acuñarla.  Quita también el comentario de
/// etiqueta que la acompaña, para que el registro no guarde metadatos de
/// claves que ya no existen.
///
/// Devuelve `true` si se revocó algo.
pub fn revoke_extra_key(state_dir: &str, target: &str) -> Result<bool, String> {
    let target = target.trim();
    if target.len() < 8 {
        return Err("target demasiado corto: se necesitan al menos 8 caracteres".into());
    }
    let path = extra_keys_path(state_dir);
    let existing = fs::read_to_string(&path).unwrap_or_default();
    if existing.trim().is_empty() {
        return Ok(false);
    }
    let mut kept: Vec<String> = Vec::new();
    let mut revoked: Option<String> = None;
    let mut pending_label: Option<String> = None;
    for line in existing.lines() {
        let t = line.trim();
        if t.starts_with('#') {
            // Comentario de etiqueta: se conserva sólo si su clave sigue viva.
            pending_label = Some(t.to_string());
            continue;
        }
        if t.is_empty() {
            continue;
        }
        if t.starts_with(target) || t == target {
            revoked = Some(t.to_string());
            pending_label = None;
            continue;
        }
        if let Some(lbl) = pending_label.take() {
            kept.push(lbl);
        }
        kept.push(t.to_string());
    }
    if let Some(lbl) = pending_label.take() {
        kept.push(lbl);
    }
    let mut out = kept.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    fs::write(&path, out).map_err(|e| format!("write {}: {}", path, e))?;
    set_mode_600(&path);
    Ok(revoked.is_some())
}

/// Lista el registro de claves adicionales: `(etiqueta, clave)`, con la clave
/// enmascarada salvo que se pida `reveal` (queda en la auditoría).
pub fn list_extra_keys(state_dir: &str, reveal: bool) -> Vec<(String, String)> {
    let path = extra_keys_path(state_dir);
    let Ok(txt) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut out: Vec<(String, String)> = Vec::new();
    let mut label = String::new();
    for line in txt.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if let Some(rest) = t.strip_prefix('#') {
            label = rest.trim().to_string();
            continue;
        }
        let shown = if reveal {
            t.to_string()
        } else {
            mask_key(t)
        };
        out.push((label.clone(), shown));
        label.clear();
    }
    out
}

/// Enmascara una clave dejando sólo el prefijo y los 4 últimos caracteres.
fn mask_key(k: &str) -> String {
    if k.len() <= 16 {
        return "*".repeat(k.len());
    }
    format!("{}...{}", &k[..12], &k[k.len() - 4..])
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
        // Exención de loopback (ver `rate_applies_to`): la UI local y el proxy
        // interno no deben agotar el cubo de los agentes remotos.
        if !rate_applies_to(ip) {
            return true;
        }
        let mut map = match self.buckets.lock() {
            Ok(m) => m,
            Err(p) => p.into_inner(),
        };
        let entry = map.entry(*ip).or_insert((now, 0));
        if now.duration_since(entry.0).as_secs() >= RATE_WINDOW_SECS {
            *entry = (now, 1);
            return true;
        }
        if entry.1 >= rate_max_requests() {
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
        // A-04 (P0) fail-closed: con `forced` (= MEGADAEMON_REQUIRE_API_KEY=1)
        // la ausencia de key NO puede dejar el servicio abierto: el túnel
        // público serviría el daemon sin credencial (con root_exec detrás).
        if cfg.forced {
            warn_once_no_key();
            return Decision::Unauthorized;
        }
        return Decision::Allow;
    }
    // Loopback exento, EXCEPTO si se fuerza (cockpit detrás de un proxy
    // local: el proxy entra por 127.0.0.1 y la exemption abriría el servicio).
    if is_loopback(peer) && !cfg.forced {
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
            if cfg.accepts(&k) {
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

/// Huella no criptográfica de una key, para poder correlacionar en logs
/// **sin revelar el secreto** (no es sha16: es FNV-1a 64, sin dependencias).
pub fn fingerprint(key: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in key.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}

/// Cuerpo del **403** (I-08) que deja claro que el problema es la credencial
/// local y **no** el movil.
///
/// **Lo que NO hacemos** (y es lo que proponia el informe): publicar una
/// huella de la key *vigente*.  Un hash de 64 bits de un secreto de alta
/// entropia, expuesto a cualquiera que llegue al daemon, es un **oraculo de
/// verificacion**: permite probar candidatos offline contra la huella.
///
/// Lo que si es seguro y util:
///   - `sent_key_fp`: la huella de la key que el cliente **si** presento (su
///     propio secreto => no filtra nada) => el agente ve "el daemon recibio MI
///     key", no "la key del movil es X".
///   - `key_rotated_at`: mtime del fichero de la key vigente => si la copia
///     local del agente es mas antigua, ve al instante que su key caducó.
pub fn forbidden_body(state_dir: &str, presented: Option<&str>) -> String {
    let fp = presented.map(fingerprint).unwrap_or_else(|| "-".to_string());
    let rotated = std::fs::metadata(format!("{}/megadaemon_api.key", state_dir))
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!(
        "{{\"error\":\"forbidden\",\"hint\":\"API key invalida o caducada: esta credencial no coincide con la vigente\",\"sent_key_fp\":\"{}\",\"key_rotated_at\":{}}}",
        fp, rotated
    )
}

/// Aviso una sola vez por proceso: "key no configurada" con `forced`.
fn warn_once_no_key() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static WARNED: AtomicBool = AtomicBool::new(false);
    if !WARNED.swap(true, Ordering::Relaxed) {
        eprintln!(
            "[api_key] CRITICO: MEGADAEMON_REQUIRE_API_KEY=1 pero no hay key configurada => \
             peticion RECHAZADA (fail-closed).  Key no configurada: no se sirve nada."
        );
    }
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
        let cfg = ApiKeyConfig { key: "k".into(), extra_keys: Vec::new(), enabled: true, forced: false };
        let lim = RateLimiter::new();
        assert_eq!(decide(&cfg, &lim, &ip4(127, 0, 0, 1), "", Instant::now()), Decision::Allow);
        assert_eq!(
            decide(&cfg, &lim, &IpAddr::V6(Ipv6Addr::LOCALHOST), "", Instant::now()),
            Decision::Allow
        );
    }

    #[test]
    fn sin_key_401_invalida_403() {
        let cfg = ApiKeyConfig { key: "secreta".into(), extra_keys: Vec::new(), enabled: true, forced: false };
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
        let cfg = ApiKeyConfig { key: "secreta".into(), extra_keys: Vec::new(), enabled: true, forced: false };
        let lim = RateLimiter::new();
        let ext = ip4(198, 51, 100, 7);
        assert_eq!(
            decide(&cfg, &lim, &ext, "Authorization: Bearer secreta\r\n", Instant::now()),
            Decision::Allow
        );
    }

    #[test]
    fn rate_limit_60_por_minuto() {
        let cfg = ApiKeyConfig { key: "secreta".into(), extra_keys: Vec::new(), enabled: true, forced: false };
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
    fn forced_exige_key_aunque_venga_de_loopback() {
        // Caso del túnel: el proxy conecta desde 127.0.0.1, pero el servicio
        // está expuesto -> la key es obligatoria.
        let cfg = ApiKeyConfig { key: "secreta".into(), extra_keys: Vec::new(), enabled: true, forced: true };
        let lim = RateLimiter::new();
        assert_eq!(
            decide(&cfg, &lim, &ip4(127, 0, 0, 1), "", Instant::now()),
            Decision::Unauthorized
        );
        assert_eq!(
            decide(&cfg, &lim, &ip4(127, 0, 0, 1), "X-API-Key: secreta\r\n", Instant::now()),
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

    /// Directorio temporal propio de cada test (sin dependencias externas).
    fn tmp_dir(tag: &str) -> String {
        let d = format!(
            "/tmp/md_apikey_test_{}_{}_{}",
            tag,
            std::process::id(),
            random_hex(4)
        );
        let _ = fs::create_dir_all(&d);
        d
    }

    #[test]
    fn acuñar_y_revocar_una_clave_no_toca_la_principal() {
        let dir = tmp_dir("mint");
        fs::write(format!("{}/megadaemon_api.key", dir), "mdaemon_sk_live_principal").unwrap();
        let key = generate_extra_key(&dir, "cloud-sandbox").unwrap();
        assert!(key.starts_with("mdaemon_sk_live_"));
        assert_eq!(key.len(), "mdaemon_sk_live_".len() + 64);

        let cfg = ApiKeyConfig::load(&dir);
        assert!(cfg.accepts(&key), "la clave acuñada debe autorizarse");
        assert!(cfg.accepts("mdaemon_sk_live_principal"), "la principal sigue viva");

        // Revocar por prefijo (8+ caracteres): el target tiene que ser un
        // PREFIJO real de la key (la funcion hace `t.starts_with(target)`).
        // Antes se cortaba por la mitad ("ve_...") y el revoke no encajaba:
        // test rojo desde antes de A-04 (verificado contra HEAD sin mis cambios).
        let target = &key[..20];
        assert!(revoke_extra_key(&dir, target).unwrap());
        let cfg2 = ApiKeyConfig::load(&dir);
        assert!(!cfg2.accepts(&key), "la clave revocada ya no autoriza");
        assert!(
            cfg2.accepts("mdaemon_sk_live_principal"),
            "la principal no se toca al revocar"
        );

        // Revocar dos veces no revoca nada nuevo.
        assert!(!revoke_extra_key(&dir, target).unwrap());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_registro_se_enmascara_sin_reveal() {
        let dir = tmp_dir("mask");
        let key = generate_extra_key(&dir, "agente-externo").unwrap();
        let rows = list_extra_keys(&dir, false);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "agente-externo");
        assert!(rows[0].1.contains("..."), "la clave debe ir enmascarada");
        assert!(!rows[0].1.contains(&key), "la clave completa no debe aparecer");

        let rows = list_extra_keys(&dir, true);
        assert_eq!(rows[0].1, key);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn revocar_una_clave_inexistente_no_altera_el_registro() {
        let dir = tmp_dir("noop");
        let key = generate_extra_key(&dir, "a").unwrap();
        let before = fs::read_to_string(extra_keys_path(&dir)).unwrap();
        assert!(!revoke_extra_key(&dir, "mdaemon_sk_live_99999999").unwrap());
        let after = fs::read_to_string(extra_keys_path(&dir)).unwrap();
        assert_eq!(before, after);
        assert!(ApiKeyConfig::load(&dir).accepts(&key));
        let _ = fs::remove_dir_all(&dir);
    }

    /// A-04: con `forced` y sin key configurada, `decide()` NO puede devolver
    /// `Allow` (fail-closed).
    #[test]
    fn a04_forced_sin_key_rechaza_en_vez_de_autorizar() {
        let cfg = ApiKeyConfig {
            key: String::new(),
            extra_keys: Vec::new(),
            enabled: false,
            forced: true,
        };
        let lim = RateLimiter::new();
        let ext = IpAddr::V4(std::net::Ipv4Addr::new(203, 0, 113, 9));
        assert_eq!(
            decide(&cfg, &lim, &ext, "", std::time::Instant::now()),
            Decision::Unauthorized,
            "con forced=true y sin key no se sirve nada"
        );
        // Y tampoco por loopback.
        let loopback = IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);
        assert_eq!(
            decide(&cfg, &lim, &loopback, "", std::time::Instant::now()),
            Decision::Unauthorized
        );
    }

    /// A-04: `forced` sin key ⇒ `load_with` acuña una key, la persiste (0600)
    /// y deja el middleware activo.
    #[test]
    fn a04_forced_sin_key_autogenera_key() {
        let dir = std::env::temp_dir()
            .join(format!("md_a04_{}", random_hex(6)))
            .to_string_lossy()
            .to_string();
        let cfg = ApiKeyConfig::load_with(&dir, true);
        assert!(cfg.enabled, "con forced la key autogenerada debe activar el middleware");
        assert!(cfg.forced);
        assert!(cfg.key.len() >= 16);
        let stored = std::fs::read_to_string(format!("{}/megadaemon_api.key", &dir)).unwrap();
        assert_eq!(stored.trim(), cfg.key);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Sin `forced` y sin key, el comportamiento histórico se mantiene
    /// (middleware inactivo) — pero `decide` con `forced` nunca autoriza.
    #[test]
    fn a04_sin_forced_sin_key_sigue_inactivo() {
        let dir = std::env::temp_dir()
            .join(format!("md_a04b_{}", random_hex(6)))
            .to_string_lossy()
            .to_string();
        let cfg = ApiKeyConfig::load_with(&dir, false);
        assert!(!cfg.enabled);
        assert!(cfg.key.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod forbidden_body_tests {
    use super::forbidden_body;

    #[test]
    fn no_filtra_la_key_vigente_ni_la_presentada() {
        let b = forbidden_body("/no/existe", Some("mdaemon_sk_live_ABCDEF"));
        assert!(b.contains("\"error\":\"forbidden\""), "{}", b);
        assert!(b.contains("sent_key_fp"), "{}", b);
        assert!(!b.contains("mdaemon_sk_live_ABCDEF"), "el secreto no debe aparecer: {}", b);
        // sin key presented: no debe Romper el JSON
        let b2 = forbidden_body("/no/existe", None);
        assert!(b2.contains("sent_key_fp"), "{}", b2);
    }
}
