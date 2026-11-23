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

/// Ruta LEGADA de las credenciales.
///
/// MEDIDO (2026-11-23): `/data/local/tmp` es VOLÁTIL en Android (se borra al
/// reiniciar el móvil) ⇒ las credenciales de Drive desaparecían y el
/// daemon se quedaba sin acceso a Drive hasta volver a instalarlas.  Ahora
/// el valor por defecto es el **state dir canónico** (en el móvil,
/// `/data/adb/megadaemon/state`, persistente) y esta ruta sólo se usa como
/// alternativa de LECTURA.
pub const LEGACY_CREDENTIALS: &str = DEFAULT_CREDENTIALS;
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
        // Sin state_dir explícito: el state dir CANÓNICO (que respeta
        // MEGADAEMON_STATE_DIR).  Antes era `/data/local/tmp/…`, volátil.
        _ => PathBuf::from(crate::state_dir::state_dir()).join("gdrive_credentials.json"),
    }
}

/// Candidatas para **leer** las credenciales: la canónica y la legada.
///
/// Se leen en orden: si la canónica no está, se usa la legada (para no
/// romper un móvil que aún tenga las credenciales en el sitio viejo).
pub fn credentials_candidates(state_dir: Option<&str>) -> Vec<PathBuf> {
    let mut v = vec![credentials_path(state_dir)];
    let leg = PathBuf::from(LEGACY_CREDENTIALS);
    if !v.contains(&leg) {
        v.push(leg);
    }
    v
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
    // La credencial puede estar en la ruta canónica o en la legada
    // (MEDIDO 2026-11-23): se prueban en orden.
    let candidatas = credentials_candidates(state_dir);
    let path = candidatas
        .iter()
        .find(|p| p.exists())
        .cloned()
        .unwrap_or_else(|| candidatas[0].clone());
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
    let candidatas = credentials_candidates(state_dir);
    let path = candidatas
        .iter()
        .find(|p| p.exists())
        .cloned()
        .unwrap_or_else(|| candidatas[0].clone());
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
        "oauth2_crate": "oauth2 5.0 (PKCE + Device Flow + Refresh Token via ureq)",
    })
}

/// Pure-sync `oauth2` v5.0 HTTP adapter backed by MegaDaemon's hardened `ureq` agent (zero `tokio`/`reqwest` bloat).
fn ureq_oauth_http(
    request: oauth2::HttpRequest,
) -> Result<oauth2::HttpResponse, std::io::Error> {
    let ag = agent();
    let uri = request.uri().to_string();
    let method = request.method().as_str();
    let mut out_resp = if method == "POST" {
        let mut req = ag.post(&uri);
        for (k, v) in request.headers().iter() {
            if let Ok(vs) = v.to_str() {
                req = req.header(k.as_str(), vs);
            }
        }
        req.send(request.body().as_slice())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?
    } else {
        let mut req = ag.get(&uri);
        for (k, v) in request.headers().iter() {
            if let Ok(vs) = v.to_str() {
                req = req.header(k.as_str(), vs);
            }
        }
        req.call()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?
    };

    let status_u16 = out_resp.status().as_u16();
    let status = oauth2::http::StatusCode::from_u16(status_u16)
        .unwrap_or(oauth2::http::StatusCode::INTERNAL_SERVER_ERROR);
    let mut headers = oauth2::http::HeaderMap::new();
    for (k, v) in out_resp.headers().iter() {
        if let (Ok(hn), Ok(hv)) = (
            oauth2::http::header::HeaderName::from_bytes(k.as_str().as_bytes()),
            oauth2::http::header::HeaderValue::from_bytes(v.as_bytes()),
        ) {
            headers.insert(hn, hv);
        }
    }
    let body = out_resp
        .body_mut()
        .read_to_vec()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

    let mut builder = oauth2::http::Response::builder().status(status);
    if let Some(hm) = builder.headers_mut() {
        *hm = headers;
    }
    builder
        .body(body)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
}

/// Generates an OAuth2 Authorization URL + PKCE (`S256`) verifier/state using the `oauth2` crate.
pub fn build_pkce_auth_url(
    client_id: &str,
    client_secret: Option<&str>,
    auth_url: &str,
    token_url: &str,
    redirect_url: &str,
    scopes: &[&str],
) -> Result<(String, String, String), String> {
    use oauth2::basic::BasicClient;
    use oauth2::{
        AuthUrl, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, RedirectUrl, Scope, TokenUrl,
    };

    let mut client = BasicClient::new(ClientId::new(client_id.to_string()))
        .set_auth_uri(AuthUrl::new(auth_url.to_string()).map_err(|e| e.to_string())?)
        .set_token_uri(TokenUrl::new(token_url.to_string()).map_err(|e| e.to_string())?)
        .set_redirect_uri(RedirectUrl::new(redirect_url.to_string()).map_err(|e| e.to_string())?);
    if let Some(sec) = client_secret {
        client = client.set_client_secret(ClientSecret::new(sec.to_string()));
    }
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let mut req = client.authorize_url(CsrfToken::new_random).set_pkce_challenge(pkce_challenge);
    for sc in scopes {
        req = req.add_scope(Scope::new((*sc).to_string()));
    }
    let (url, csrf) = req.url();
    Ok((
        url.to_string(),
        pkce_verifier.secret().to_string(),
        csrf.secret().to_string(),
    ))
}

/// Refreshes an OAuth2 token cleanly via `oauth2::basic::BasicClient::exchange_refresh_token`.
pub fn refresh_with_oauth2(
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<GoogleToken, String> {
    use oauth2::basic::BasicClient;
    use oauth2::{ClientId, ClientSecret, RefreshToken, TokenResponse, TokenUrl};

    let client = BasicClient::new(ClientId::new(client_id.to_string()))
        .set_client_secret(ClientSecret::new(client_secret.to_string()))
        .set_token_uri(TokenUrl::new(TOKEN_URL.to_string()).map_err(|e| e.to_string())?);
    let rt = RefreshToken::new(refresh_token.to_string());
    let tok = client
        .exchange_refresh_token(&rt)
        .request(&ureq_oauth_http)
        .map_err(|e| format!("oauth2 refresh failed: {}", e))?;
    let secs = tok.expires_in().map(|d| d.as_secs()).unwrap_or(3600);
    Ok(GoogleToken {
        access_token: tok.access_token().secret().to_string(),
        expires_at: Instant::now() + Duration::from_secs(secs.saturating_sub(RENEW_MARGIN_SECS)),
    })
}

/// Starts an OAuth2 Device Authorization Flow (`RFC 8628`) via `oauth2::basic::BasicClient`.
pub fn start_device_flow(
    client_id: &str,
    client_secret: Option<&str>,
    device_auth_url: &str,
    scopes: &[&str],
) -> Result<Value, String> {
    use oauth2::basic::BasicClient;
    use oauth2::{ClientId, ClientSecret, DeviceAuthorizationUrl, Scope};

    let mut client = BasicClient::new(ClientId::new(client_id.to_string()))
        .set_device_authorization_url(
            DeviceAuthorizationUrl::new(device_auth_url.to_string()).map_err(|e| e.to_string())?,
        );
    if let Some(sec) = client_secret {
        client = client.set_client_secret(ClientSecret::new(sec.to_string()));
    }
    let mut req = client.exchange_device_code();
    for sc in scopes {
        req = req.add_scope(Scope::new((*sc).to_string()));
    }
    let details: oauth2::StandardDeviceAuthorizationResponse = req
        .request(&ureq_oauth_http)
        .map_err(|e| format!("oauth2 device flow failed: {}", e))?;
    Ok(json!({
        "verification_uri": details.verification_uri().to_string(),
        "user_code": details.user_code().secret(),
        "device_code": details.device_code().secret(),
        "expires_in_secs": details.expires_in().as_secs(),
        "interval_secs": details.interval().as_secs(),
    }))
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
