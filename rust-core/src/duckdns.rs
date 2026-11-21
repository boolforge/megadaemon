//! Canal estable del enjambre: actualizador de **DuckDNS** (nativo, sin
//! dependencias nuevas) y lectura del **estado del túnel**.
//!
//! Por qué DuckDNS y no el *quick tunnel* de Cloudflare: un *quick tunnel*
//! **rota el hostname en cada reinicio** y además no resuelve, así que cada
//! reinicio del móvil deja a todos los agentes sin canal (530/403). Un
//! dominio propio (p. ej. `mdtmfnc.duckdns.org`) es **estable** y permite
//! además obtener un certificado Let's Encrypt por **DNS-01**.
//!
//! Reglas de seguridad (no negociables):
//!
//! * El **token de DuckDNS NUNCA** está en el repo, ni en el entorno del
//!   proceso, ni en el journal: se lee de un **fichero 0600**.
//! * El token **nunca** se escribe en la salida ni en el estado: el estado
//!   sólo guarda el **último resultado** y la marca de tiempo.
//! * El módulo es **tolerante**: si el token no está, si la red falla o el
//!   fichero de estado está corrupto, devuelve `ok:false` con el motivo y
//!   **no rompe nada**.
//!
//! Uso típico en el móvil (instancia maestra):
//!
//! ```text
//! duckdns-updater --once                 # una actualización y salir
//! duckdns-updater --interval 300         # bucle cada 5 min
//! ```

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Endpoint de DuckDNS. Es estable por diseño (a diferencia del quick tunnel).
pub const DUCKDNS_UPDATE_URL: &str = "https://www.duckdns.org/update";

/// Fichero por defecto del token (0600). NUNCA en el repo.
pub const DEFAULT_TOKEN_FILE: &str = "/data/local/tmp/megadaemon/duckdns.token";

/// Fichero por defecto del estado local.
pub const DEFAULT_STATE_FILE: &str = "/data/local/tmp/megadaemon/duckdns.state.json";

/// Dominio por defecto si `instance.json` no lo trae.
pub const DEFAULT_DOMAIN: &str = "mdtmfnc.duckdns.org";

/// Segundos sin timeout: DuckDNS es un Form submit, responde en <2 s.
const HTTP_TIMEOUT: u64 = 8;

/// Ruta del fichero de token según el entorno.
fn token_path(explicit: Option<&str>) -> PathBuf {
    match explicit {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => PathBuf::from(DEFAULT_TOKEN_FILE),
    }
}

/// Ruta del fichero de estado.
fn state_path(explicit: Option<&str>) -> PathBuf {
    match explicit {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => PathBuf::from(DEFAULT_STATE_FILE),
    }
}

/// Lee el token de un fichero 0600. **Devuelve `None` si no existe, si está
/// vacío o si los permisos no son restrictivos** (600 o 400).
pub fn read_token(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode() & 0o777;
        // 0600 o 0400: nada más wide. 0640/0644 => no lo leemos (el token
        // sería legible por otros usuarios del dispositivo).
        if mode & 0o077 != 0 {
            return None;
        }
    }
    let raw = fs::read_to_string(path).ok()?;
    let t = raw.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Extrae el dominio DuckDNS de un `instance.json` (campo `duckdns_domain`).
pub fn domain_from_instance(instance_json: &str) -> Option<String> {
    let v: Value = serde_json::from_str(instance_json).ok()?;
    v.get("duckdns_domain")
        .and_then(|d| d.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// ¿Este dominio es el **CANÓNICO**?
///
/// MEDIDO (2026-11-21): el canónico `mdtmfnc.duckdns.org` apuntaba a
/// `47.253.152.120` = la IP pública del **sandbox** (NAT del datacenter), no
/// la del PC de casa => el relay, que vive en el PC, era inalcanzable.  La
/// causa: `duckdns.org/update` se había llamado desde el sandbox, y ese
/// endpoint pone el A record a la IP de **quien llama**.
///
///
/// Y un **`OK` de la API no prueba que el dominio exista**: DuckDNS sólo
/// crea los subdominios **registrados en la cuenta** => hay que **verificar
/// el DNS** despues (DoH: `Status 0` + Answer = existe; `Status 3` = no
/// existe, y el mensaje de "OK" sale igual).
/// De ahí la regla: **un SOLO escritor** del A record canónico = el relay
/// del PC (`deploy/relay-pc.sh`, que se ejecuta en casa).  El móvil (o el
/// sandbox) no lo tocan salvo que se pida explícitamente.
pub fn es_canonico(domain: &str) -> bool {
    domain.trim().eq_ignore_ascii_case(DEFAULT_DOMAIN)
}

/// ¿Se puede actualizar el A record de este dominio?
///
/// Todo salvo el canónico.  El canónico requiere `permitir_canonico`
/// explícito (`--allow-canonical` o `MEGADAEMON_ALLOW_CANONICAL_DNS=1`).
pub fn puede_actualizar(domain: &str, permitir_canonico: bool) -> bool {
    !es_canonico(domain) || permitir_canonico
}

/// Construye la URL de actualización. Es una función **pura** (testeable sin
/// red): el token va en la query porque así lo define la API de DuckDNS.
pub fn update_url(domain: &str, token: &str, ip: &str) -> String {
    format!(
        "{DUCKDNS_UPDATE_URL}?domains={domain}&token={token}&ip={ip}"
    )
}

/// Descubre la IP pública. Usa un servicio de eco; si falla, devuelve `None`.
pub fn public_ip() -> Option<String> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(HTTP_TIMEOUT)))
        .timeout_connect(Some(Duration::from_secs(4)))
        .http_status_as_error(false)
        .build();
    let agent = ureq::Agent::new_with_config(agent);
    let r = agent
        .get("https://api.ipify.org")
        .header("User-Agent", "MegaDaemon-duckdns")
        .call()
        .ok()?;
    if r.status().as_u16() != 200 {
        return None;
    }
    let body = r.into_body().read_to_string().ok()?;
    let ip = body.trim().to_string();
    if ip.is_empty() {
        None
    } else {
        Some(ip)
    }
}

/// Una actualización de DuckDNS. Devuelve un JSON con el resultado; nunca
/// panica ni deja el proceso colgado.
///
/// **Si la IP no ha cambiado NO se llama a la API**: DuckDNS tolera las
/// actualizaciones repetidas, pero son ruido. Así el estado refleja el
/// último update *real*.
pub fn update_once(
    domain: &str,
    token: &str,
    ip: &str,
    state_file: Option<&str>,
) -> Value {
    // ¿La IP ya es la misma que la del último update correcto?
    if let Some(prev) = read_state(state_file) {
        let misma_ip = prev.get("ip").and_then(|v| v.as_str()) == Some(ip);
        let ok_previo = prev.get("ok").and_then(|v| v.as_bool()) == Some(true);
        if misma_ip && ok_previo {
            let mut estado = prev.clone();
            if let Some(o) = estado.as_object_mut() {
                o.insert("skipped".to_string(), json!("ip_unchanged"));
                o.insert("checked_at".to_string(), json!(now_iso()));
            }
            let _ = write_state(state_file, &estado);
            return estado;
        }
    }

    let url = update_url(domain, token, ip);
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(HTTP_TIMEOUT)))
        .timeout_connect(Some(Duration::from_secs(4)))
        .http_status_as_error(false)
        .build();
    let agent = ureq::Agent::new_with_config(agent);
    let now = now_iso();

    let (ok, detail, http_status) = match agent.get(&url).call() {
        Ok(r) => {
            let st = r.status().as_u16();
            let body = r.into_body().read_to_string().unwrap_or_default();
            let body = body.trim().to_string();
            // DuckDNS responde "OK" (o "ip updated") cuando acepta el update.
            let good = st == 200 && (body.eq_ignore_ascii_case("ok") || body.contains("OK"));
            (good, body, st)
        }
        Err(e) => (false, format!("{e}"), 0),
    };

    // El estado **jamás** contiene el token.
    let estado = json!({
        "ok": ok,
        "domain": domain,
        "ip": ip,
        "http_status": http_status,
        "detail": detail.chars().take(200).collect::<String>(),
        "updated_at": now,
    });
    let _ = write_state(state_file, &estado);
    estado
}

/// Guarda el estado (best effort) en el fichero indicado.
pub fn write_state(state_file: Option<&str>, state: &Value) -> std::io::Result<()> {
    let path = state_path(state_file);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).ok();
    }
    let mut texto = serde_json::to_string_pretty(state).unwrap_or_default();
    texto.push('\n');
    fs::write(&path, texto)?;
    // 0600: el fichero no lleva secretos, pero no hace falta que lo lea nadie más.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).ok();
    }
    Ok(())
}

/// Lee el estado previo (tolerante: si no existe o está corrupto, `None`).
pub fn read_state(state_file: Option<&str>) -> Option<Value> {
    let path = state_path(state_file);
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Escribe la **URL viva del canal estable** en `<state_dir>/channel.url`
/// (y en `duckdns.url` como alias), para que **ningún agente hardcodee una
/// URL**: la leen del fichero.
///
/// ⚠️ **No** se escribe en `tunnel.url`: ese fichero es del *quick tunnel* de
/// Cloudflare y lo lee `deploy/tunnel-watchdog.sh`; mezclar los dos canales
/// haría que el watchdog sondeara la URL equivocada.
pub fn write_channel_url(state_dir: Option<&str>, url: &str) -> std::io::Result<()> {
    let dir = match state_dir {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => PathBuf::from("/data/local/tmp/megadaemon"),
    };
    fs::create_dir_all(&dir).ok();
    for nombre in ["channel.url", "duckdns.url"] {
        let p = dir.join(nombre);
        fs::write(&p, format!("{url}\n"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&p, fs::Permissions::from_mode(0o644)).ok();
        }
    }
    Ok(())
}

/// Estado completo de DuckDNS para el dashboard: `{ok, domain, token_present,
/// last_update, ip}`. **No incluye el token.**
pub fn status(state_file: Option<&str>, token_file: Option<&str>) -> Value {
    let token = read_token(&token_path(token_file));
    let last = read_state(state_file);
    json!({
        "ok": token.is_some(),
        "domain": DEFAULT_DOMAIN,
        "token_present": token.is_some(),
        "token_file": token_path(token_file).to_string_lossy(),
        "last_update": last,
    })
}

/// Instante actual en ISO-8601 (UTC), sin dependencias externas.
pub fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Formato YYYY-MM-DDTHH:MM:SSZ (cálculo manual, sin chrono).
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days as i64);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

/// Convierte días desde epoch en fecha civil (algoritmo de Howard Hinnant).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_ausente_o_permisos_amplios_no_se_leen() {
        let dir = std::env::temp_dir().join("md_duckdns_test");
        let _ = fs::create_dir_all(&dir);
        let p = dir.join("token");
        fs::write(&p, "abc123\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // 0644 => rechazado (el token sería legible por otros).
            fs::set_permissions(&p, fs::Permissions::from_mode(0o644)).unwrap();
            assert!(read_token(&p).is_none());
            // 0600 => aceptado.
            fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
            assert_eq!(read_token(&p).as_deref(), Some("abc123"));
        }
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn el_dominio_sale_de_instance_json() {
        let js = r#"{"duckdns_domain":"mdtmfnc.duckdns.org","role":"primary"}"#;
        assert_eq!(
            domain_from_instance(js).as_deref(),
            Some("mdtmfnc.duckdns.org")
        );
        assert!(domain_from_instance("no es json").is_none());
        assert!(domain_from_instance("{}").is_none());
    }

    /// El canónico lo escribe el relay del PC: un solo escritor.
    #[test]
    fn el_canonico_necesita_permiso_explicito() {
        assert!(es_canonico("mdtmfnc.duckdns.org"));
        assert!(es_canonico("  MDTMFNC.DUCKDNS.ORG  "));
        assert!(!es_canonico("otro.duckdns.org"));
        // sin permiso => NO se puede actualizar el canónico
        assert!(!puede_actualizar("mdtmfnc.duckdns.org", false));
        // con permiso explícito => sí
        assert!(puede_actualizar("mdtmfnc.duckdns.org", true));
        // un dominio NO canónico se puede siempre
        assert!(puede_actualizar("otro.duckdns.org", false));
    }

    #[test]
    fn la_url_de_update_lleva_dominio_token_e_ip() {
        let u = update_url("mdtmfnc.duckdns.org", "TOK", "1.2.3.4");
        assert!(u.starts_with(DUCKDNS_UPDATE_URL));
        assert!(u.contains("domains=mdtmfnc.duckdns.org"));
        assert!(u.contains("token=TOK"));
        assert!(u.contains("ip=1.2.3.4"));
    }

    #[test]
    fn el_estado_nunca_contiene_el_token() {
        let dir = std::env::temp_dir().join("md_duckdns_state");
        let _ = fs::create_dir_all(&dir);
        let f = dir.join("state.json");
        let st = json!({"ok": true, "domain": "d", "ip": "1.2.3.4"});
        write_state(Some(f.to_str().unwrap()), &st).unwrap();
        let raw = fs::read_to_string(&f).unwrap();
        assert!(!raw.contains("TOK"));
        let leido = read_state(Some(f.to_str().unwrap())).unwrap();
        assert_eq!(leido.get("ok").and_then(|v| v.as_bool()), Some(true));
        let _ = fs::remove_file(&f);
    }

    #[test]
    fn la_fecha_utc_es_coherente() {
        let s = now_iso();
        assert_eq!(s.len(), 20, "{s}");
        assert!(s.ends_with('Z'));
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[10..11], "T");
    }
}
