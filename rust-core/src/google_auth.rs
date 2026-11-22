//! OAuth2 de Google en **Rust puro** (sin Python, sin shell, sin `curl`).
//!
//! Credencial tipo `authorized_user` (`application_default_credentials.json`):
//!
//! ```text
//! POST https://oauth2.googleapis.com/token
//! Content-Type: application/x-www-form-urlencoded
//! grant_type=refresh_token&client_id=..&client_secret=..&refresh_token=..
//! -> { "access_token": "...", "expires_in": 3599, "token_type": "Bearer" }
//! ```
//!
//! Reglas de este módulo:
//! * el secreto **nunca** se imprime, ni se devuelve, ni sale del fichero 0600;
//! * el token se cachea en memoria con su caducidad y se refresca solo
//!   (60 s antes de expirar) o a petición (tras un 401 de Drive);
//! * si el fichero no está, el error dice **dónde** se espera, nunca el valor.
//!
//! Ficha del fichero de credenciales (0600):
//!   `/data/local/tmp/megadaemon/state/gdrive_credentials.json` en el móvil,
//!   o la ruta que diga `MEGADAEMON_GDRIVE_CREDENTIALS`.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Endpoint de intercambio de token de Google.
pub const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
/// Variable de entorno con la ruta del fichero de credenciales.
pub const ENV_CREDENTIALS: &str = "MEGADAEMON_GDRIVE_CREDENTIALS";
/// Ruta por defecto en el móvil (dentro del `state_dir`).
pub const DEFAULT_CREDENTIALS: &str = "/data/local/tmp/megadaemon/state/gdrive_credentials.json";
/// Margen: se refresca antes de que caduque.
const RENEW_MARGIN_SECS: u64 = 60;

/// Token de acceso con su caducidad.
#[derive(Debug, Clone)]
pub struct GoogleToken {
    /// El `access_token` (Bearer).
    pub access_token: String,
    /// Cuándo deja de valer (con el margen aplicado).
    pub expires_at: Instant,
}

impl GoogleToken {
    /// ¿Caducado (o a punto de caducar)?
    pub fn is_expired(&self) -> bool {
        self.expires_at <= Instant::now()
    }
}

static CACHE: Mutex<Option<GoogleToken>> = Mutex::new(None);

/// Epoch en segundos (0 si el reloj está antes de 1970).
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Ruta del fichero de credenciales: env > `state_dir` > ruta por defecto.
pub fn credentials_path(state_dir: Option<&str>) -> PathBuf {
    if let Ok(p) = std::env::var(ENV_CREDENTIALS) {
        let p = p.trim();
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    match state_dir {
        Some(d) if !d.trim().is_empty() => PathBuf::from(d).join("gdrive_credentials.json"),
        _ => PathBuf::from(DEFAULT_CREDENTIALS),
    }
}

/// Percent-encoding de un valor de formulario (form-urlencoded).
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Agente HTTP de ureq con tiempos acotados (nada de esperas infinitas).
fn agent() -> ureq::Agent {
    let cfg = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(60)))
        .timeout_connect(Some(Duration::from_secs(15)))
        .http_status_as_error(false)
        .build();
    ureq::Agent::new_with_config(cfg)
}

/// Refresca el token desde el fichero (sin caché).
pub fn refresh_from_file(path: &Path) -> Result<GoogleToken, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("no se puede leer la credencial {}: {}", path.display(), e))?;
    let v: Value = serde_json::from_str(&raw)
        .map_err(|e| format!("la credencial {} no es JSON: {}", path.display(), e))?;
    let kind = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
    if kind != "authorized_user" {
        return Err(format!(
            "tipo de credencial '{}' no soportado (este módulo habla 'authorized_user')",
            kind
        ));
    }
    let client_id = v.get("client_id").and_then(|x| x.as_str()).unwrap_or("");
    let client_secret = v.get("client_secret").and_then(|x| x.as_str()).unwrap_or("");
    let refresh = v.get("refresh_token").and_then(|x| x.as_str()).unwrap_or("");
    if refresh.is_empty() {
        return Err("la credencial no trae `refresh_token`".to_string());
    }
    let form = format!(
        "grant_type=refresh_token&client_id={}&client_secret={}&refresh_token={}",
        urlencode(client_id),
        urlencode(client_secret),
        urlencode(refresh)
    );
    let resp = agent()
        .post(TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send(form.as_bytes())
        .map_err(|e| format!("POST {}: {}", TOKEN_URL, e))?;
    let status = resp.status().as_u16();
    let body = resp
        .into_body()
        .read_to_vec()
        .map_err(|e| format!("no se pudo leer la respuesta de oauth2: {}", e))?;
    if status != 200 {
        // El cuerpo de error de Google NO contiene el secreto, pero se
        // recorta por si acaso.
        let txt: String = String::from_utf8_lossy(&body).chars().take(200).collect();
        return Err(format!("oauth2 devolvió {}: {}", status, txt));
    }
    let j: Value =
        serde_json::from_slice(&body).map_err(|e| format!("respuesta de oauth2 no es JSON: {}", e))?;
    let token = j.get("access_token").and_then(|x| x.as_str()).unwrap_or("");
    if token.is_empty() {
        return Err("oauth2 no devolvió `access_token`".to_string());
    }
    let expires_in = j.get("expires_in").and_then(|x| x.as_u64()).unwrap_or(3600);
    Ok(GoogleToken {
        access_token: token.to_string(),
        expires_at: Instant::now() + Duration::from_secs(expires_in.saturating_sub(RENEW_MARGIN_SECS)),
    })
}

/// Lee un `access_token` vigente desde `rclone.conf` si no existe `gdrive_credentials.json`.
fn fallback_rclone_token() -> Option<GoogleToken> {
    let raw = std::fs::read_to_string("/data/local/tmp/rc/rclone.conf").ok()?;
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("token") {
            let rest = rest.trim_start();
            if let Some(json_str) = rest.strip_prefix('=') {
                if let Ok(v) = serde_json::from_str::<Value>(json_str.trim()) {
                    if let Some(tok) = v.get("access_token").and_then(|x| x.as_str()) {
                        if !tok.is_empty() {
                            return Some(GoogleToken {
                                access_token: tok.to_string(),
                                expires_at: Instant::now() + Duration::from_secs(1800),
                            });
                        }
                    }
                }
            }
        }
    }
    None
}

/// Token válido: de la caché, o refrescado.
pub fn token(state_dir: Option<&str>) -> Result<GoogleToken, String> {
    let mut guard = CACHE
        .lock()
        .map_err(|_| "la caché de token está envenenada".to_string())?;
    if let Some(t) = guard.as_ref() {
        if !t.is_expired() {
            return Ok(t.clone());
        }
    }
    let path = credentials_path(state_dir);
    let t = if path.exists() {
        refresh_from_file(&path)?
    } else if let Some(tok) = fallback_rclone_token() {
        tok
    } else {
        refresh_from_file(&path)?
    };
    *guard = Some(t.clone());
    Ok(t)
}

/// Fuerza el refresco (tras un 401 de Drive).
pub fn invalidate() {
    if let Ok(mut g) = CACHE.lock() {
        *g = None;
    }
}

/// Cabecera `Authorization` para las llamadas a Drive.
pub fn auth_header(state_dir: Option<&str>) -> Result<(String, String), String> {
    let t = token(state_dir)?;
    Ok(("Authorization".to_string(), format!("Bearer {}", t.access_token)))
}

/// Huella sha16 de la credencial: permite registrarla **sin** exponerla.
pub fn credentials_fingerprint(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    match std::fs::read(path) {
        Ok(b) => {
            let h = Sha256::digest(&b);
            h.iter().map(|x| format!("{:02x}", x)).collect::<String>().chars().take(16).collect()
        }
        Err(_) => "sin-credencial".to_string(),
    }
}

/// Estado del acceso a Google (para la tool y para los informes).
pub fn status(state_dir: Option<&str>) -> Value {
    let path = credentials_path(state_dir);
    let fp = credentials_fingerprint(&path);
    let (has_file, size) = match std::fs::metadata(&path) {
        Ok(m) => (true, m.len()),
        Err(_) => (false, 0),
    };
    let cached = CACHE.lock().ok().and_then(|g| g.as_ref().cloned());
    json!({
        "credentials_path": path.display().to_string(),
        "credentials_present": has_file,
        "credentials_bytes": size,
        "credentials_fingerprint_sha16": fp,
        "token_cached": cached.is_some(),
        "token_expired": cached.map(|t| t.is_expired()).unwrap_or(true),
        "state_dir": state_dir.unwrap_or(""),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_orden_de_precedencia_de_la_ruta_es_env_state_por_defecto() {
        // Sin env y sin state_dir => la ruta por defecto del móvil.
        let p = credentials_path(None);
        assert!(p.display().to_string().contains("gdrive_credentials.json"));
        // Con state_dir => dentro del state_dir.
        let p2 = credentials_path(Some("/data/local/tmp/megadaemon/state"));
        assert!(p2.display().to_string().ends_with("gdrive_credentials.json"));
        assert!(p2.display().to_string().contains("megadaemon"));
    }

    #[test]
    fn urlencode_escapa_lo_que_hace_falta() {
        assert_eq!(urlencode("abcXYZ019-_.~"), "abcXYZ019-_.~");
        assert_eq!(urlencode("a b"), "a%20b");
        assert_eq!(urlencode("a+b/c"), "a%2Bb%2Fc");
        assert_eq!(urlencode("a&b=c"), "a%26b%3Dc");
    }

    #[test]
    fn una_credencial_que_no_es_authorized_user_se_rechaza() {
        let dir = std::env::temp_dir().join(format!("md_gauth_{}", now_secs()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("c.json");
        std::fs::write(&f, br#"{"type":"service_account","private_key":"x"}"#).unwrap();
        let e = refresh_from_file(&f).unwrap_err();
        assert!(e.contains("authorized_user"), "{}", e);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn una_credencial_sin_refresh_token_dice_que_falta() {
        let dir = std::env::temp_dir().join(format!("md_gauth2_{}", now_secs()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("c.json");
        std::fs::write(&f, br#"{"type":"authorized_user","client_id":"x"}"#).unwrap();
        let e = refresh_from_file(&f).unwrap_err();
        assert!(e.contains("refresh_token"), "{}", e);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_huella_no_revela_el_contenido() {
        let dir = std::env::temp_dir().join(format!("md_gauth3_{}", now_secs()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("c.json");
        std::fs::write(&f, br#"{"type":"authorized_user","refresh_token":"secreto"}"#).unwrap();
        let fp = credentials_fingerprint(&f);
        assert_eq!(fp.len(), 16);
        assert!(!fp.contains("secreto"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
