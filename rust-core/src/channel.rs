//! **Estado y publicación del canal** del enjambre: el *quick tunnel* de
//! Cloudflare (hostname rotatorio) y el canal estable de DuckDNS.
//!
//! El problema de fondo que resuelve este módulo:
//!
//! * El *quick tunnel* **regenera el hostname en cada reinicio** ⇒ el sandbox
//!   se queda **ciego** al móvil si no descubre la URL nueva por sí mismo.
//! * El móvil **no tiene `curl` ni `wget`** ⇒ la publicación **no** puede
//!   hacerse con un script de shell ⇒ la hace **este código**, que ya tiene
//!   cliente HTTP (`ureq`) en todas las plataformas.
//!
//! Por eso hay **dos canales y dos ficheros**, nunca mezclados:
//!
//! | Canal | Fichero | Para qué |
//! |-------|---------|----------|
//! | quick (Cloudflare) | `tunnel.url` | el túnel efímero de este momento |
//! | estable (DuckDNS)  | `channel.url` | el dominio que no rota |
//!
//! Y un **tercer** fichero, `channel.json`, que es el **estado completo**
//! (`duckdns`, `quick`, `ip_publica`, `ts`, `cloudflared_pids`, `paste`) ⇒ lo
//! publica el móvil y lo lee el sandbox.
//!
//! ⚠️ **El `paste` es la pieza que resuelve el caso "el túnel está caído"**:
//! como el móvil tiene salida a Internet, puede publicar su URL vigente en
//! un paste **público** aunque nadie pueda hablar con él. El contenido del
//! paste incluye la URL del paste anterior ⇒ la cadena es **auto-renovable**:
//! el sandbox que conoce un paste aprende el siguiente.
//!
//! Reglas: nunca panica, nunca espera infinita (timeouts cortos), y **el
//! token de DuckDNS no aparece aquí** (eso vive en `duckdns.rs`).
//!
//! 📜 **Política de publicación de datos del móvil**: `docs/API_V2.md` §5.15 —
//! toda tool que publique fuera exige `confirm=true`, lo dice en su
//! descripción y responde con **QUÉ** se publica y **DÓNDE**.
//!
//! 📡 **El punto canónico NO es el paste**: `paste.rs` no tiene caducidad y
//! cada publicación crea un paste nuevo (el sandbox no puede descubrir el
//! siguiente). El canónico es el **endpoint `/channel.json` del daemon**
//! (público, payload mínimo) servido por el relay del PC ⇒ URL estable, sin
//! PAT en el móvil y sin caché de CDN. El repo (Contents API) y el paste
//! (TTL de 6 h impuesto por el lector) son **respaldos**.

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Servicio de paste público (el móvil no tiene curl: publica este código).
pub const PASTE_URL: &str = "https://paste.rs";

/// Respaldo: si paste.rs no responde, se prueba éste.
pub const PASTE_URL_ALT: &str = "https://paste.c-net.org/";

/// **URL canónica y estable** del estado del canal: el **endpoint del daemon**
/// (`GET /channel.json`, público y con payload mínimo) servido por el **relay
/// del PC**.
///
/// ⚠️ Por qué no el paste: cada publicación crea un paste **nuevo** ⇒ el
/// sandbox que conoce el paste P1 **no puede descubrir P2** (la cadena
/// `prev_paste` sólo se sigue hacia atrás). Y por qué no el `raw` del repo:
/// **cachea en CDN** ⇒ puede servir la versión vieja. El relay no cachea y
/// su URL no rota ⇒ es el único que sirve de verdad "lo último".
///
/// Se puede sobrescribir con `MEGADAEMON_CANONICAL_URL` (p. ej. si el
/// dominio del relay cambia).
pub const CANONICAL_URL: &str = "https://mdtmfnc-s21.duckdns.org/channel.json";

/// URL canónica efectiva: la del entorno si la hay, si no la de por defecto.
pub fn canonical_efectiva() -> String {
    match std::env::var("MEGADAEMON_CANONICAL_URL") {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => CANONICAL_URL.to_string(),
    }
}

/// Publica un fichero en el repo con la **Contents API** de GitHub ⇒ la URL
/// `raw` de ese fichero es estable y siempre sirve la última versión.
///
/// El token se lee fuera (fichero 0600 o entorno) y **nunca** se registra.
pub fn publish_to_repo(
    owner: &str,
    repo: &str,
    path: &str,
    token: &str,
    text: &str,
) -> Option<Value> {
    use base64ct::Encoding;
    let b64 = base64ct::Base64::encode_string(text.as_bytes());
    let url = format!("https://api.github.com/repos/{owner}/{repo}/contents/{path}");

    let cfg = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .timeout_connect(Some(Duration::from_secs(6)))
        .http_status_as_error(false)
        .build();
    let agent = ureq::Agent::new_with_config(cfg);

    // ⚠️ La Contents API **exige el `sha` del blob actual** para ACTUALIZAR un
    // fichero que ya existe (si no, responde 422 "Validation Failed"). Sin
    // este GET, el móvil podría publicar **una sola vez** y luego fallar
    // siempre ⇒ el estado se congelaría en la primera URL.
    let sha_actual = agent
        .get(&url)
        .header("Authorization", &format!("Bearer {token}"))
        .header("User-Agent", "MegaDaemon-channel")
        .header("Accept", "application/vnd.github+json")
        .call()
        .ok()
        .and_then(|r| r.into_body().read_to_string().ok())
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("sha").and_then(|s| s.as_str()).map(|s| s.to_string()));

    let mut body_obj = json!({
        "message": "canal: estado del canal (quick tunnel + canal estable)",
        "content": b64,
        "branch": "main",
    });
    if let (Some(o), Some(sha)) = (body_obj.as_object_mut(), sha_actual.as_ref()) {
        o.insert("sha".to_string(), json!(sha));
    }
    let body = body_obj.to_string();

    let resp = agent
        .put(&url)
        .header("Authorization", &format!("Bearer {token}"))
        .header("User-Agent", "MegaDaemon-channel")
        .header("Accept", "application/vnd.github+json")
        .send(body.as_bytes())
        .ok()?;
    let status = resp.status().as_u16();
    let texto = resp.into_body().read_to_string().unwrap_or_default();
    // 200 = creado, 201 = actualizado.
    if !(status == 200 || status == 201) {
        return Some(json!({
            "ok": false,
            "http_status": status,
            "detail": "la Contents API no acepto la publicacion",
        }));
    }
    let v: Value = serde_json::from_str(&texto).unwrap_or(Value::Null);
    Some(json!({
        "ok": true,
        "http_status": status,
        "canonical": canonical_url(owner, repo, path),
        "commit": v.get("content").and_then(|c| c.get("sha")).and_then(|s| s.as_str()),
    }))
}

/// URL `raw` canónica para un fichero del repo.
pub fn canonical_url(owner: &str, repo: &str, path: &str) -> String {
    format!("https://raw.githubusercontent.com/{owner}/{repo}/main/{path}")
}

/// Nombre del fichero de estado del canal.
pub const CHANNEL_JSON: &str = "channel.json";

/// Fichero con la URL del canal estable (DuckDNS).
pub const CHANNEL_URL: &str = "channel.url";

/// Fichero con la URL del quick tunnel (Cloudflare).
pub const TUNNEL_URL: &str = "tunnel.url";

/// Timeout del paste: corto, porque si el móvil va saturado no queremos
/// quedarnos esperando (y el watchdog debe poder reintentar luego).
const PASTE_TIMEOUT: u64 = 12;

/// Directorio del canal: `state_dir` explícito > el del daemon > el del móvil.
pub fn channel_dir(state_dir: Option<&str>) -> PathBuf {
    if let Some(d) = state_dir {
        if !d.trim().is_empty() {
            return PathBuf::from(d);
        }
    }
    // El `state_dir` del daemon es la fuente de verdad (ver `cockpit_server`).
    let sd = crate::cockpit_server::state_dir();
    if !sd.trim().is_empty() {
        return PathBuf::from(sd);
    }
    PathBuf::from("/data/local/tmp/megadaemon")
}

/// Ruta de un fichero del canal.
pub fn channel_path(state_dir: Option<&str>, name: &str) -> PathBuf {
    channel_dir(state_dir).join(name)
}

/// Lee `channel.json` (tolerante: si no existe o está corrupto, `None`).
pub fn read_channel_json(state_dir: Option<&str>) -> Option<Value> {
    let raw = fs::read_to_string(channel_path(state_dir, CHANNEL_JSON)).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Escribe `channel.json` (best effort; crea el directorio si hace falta).
pub fn write_channel_json(state_dir: Option<&str>, value: &Value) -> std::io::Result<()> {
    let path = channel_path(state_dir, CHANNEL_JSON);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).ok();
    }
    let mut texto = serde_json::to_string_pretty(value).unwrap_or_default();
    texto.push('\n');
    fs::write(&path, texto)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).ok();
    }
    Ok(())
}

/// Escribe una URL suelta (`tunnel.url` o `channel.url`).
pub fn write_url_file(state_dir: Option<&str>, name: &str, url: &str) -> std::io::Result<()> {
    let path = channel_path(state_dir, name);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).ok();
    }
    fs::write(&path, format!("{url}\n"))
}

/// Actualiza **un** campo del `channel.json` sin perder los demás.
/// Devuelve el JSON resultante.
pub fn merge_channel_field(
    state_dir: Option<&str>,
    key: &str,
    value: Value,
) -> Value {
    let mut actual = read_channel_json(state_dir).unwrap_or_else(|| json!({}));
    if !actual.is_object() {
        actual = json!({});
    }
    if let Some(o) = actual.as_object_mut() {
        o.insert(key.to_string(), value);
        o.insert("ts".to_string(), json!(now_iso()));
    }
    let _ = write_channel_json(state_dir, &actual);
    actual
}

/// Publica un texto en un paste público y devuelve **la URL del paste**.
/// `None` si ningún servicio responde (se avisa, no se cae nada).
pub fn publish_paste(text: &str) -> Option<String> {
    paste_en(PASTE_URL, text).or_else(|| paste_en(PASTE_URL_ALT, text))
}

/// Publica en un servicio concreto. ⚠️ `paste.rs` responde **201** (no 200)
/// con la URL del paste en el cuerpo ⇒ mirar sólo el 200 descartaba la
/// publicación y hacía creer que el paste estaba caído.
fn paste_en(url: &str, text: &str) -> Option<String> {
    let cfg = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(PASTE_TIMEOUT)))
        .timeout_connect(Some(Duration::from_secs(5)))
        .http_status_as_error(false)
        .build();
    let agent = ureq::Agent::new_with_config(cfg);
    let resp = agent
        .post(url)
        .header("User-Agent", "MegaDaemon-channel")
        .send(text)
        .ok()?;
    let status = resp.status().as_u16();
    if !(status == 200 || status == 201) {
        return None;
    }
    let cuerpo = resp.into_body().read_to_string().ok()?;
    let url_paste = cuerpo.trim();
    if url_paste.starts_with("http") {
        Some(url_paste.to_string())
    } else {
        None
    }
}

/// Sondea una URL del canal y devuelve `(código_http, ¿el cuerpo es del daemon?)`.
///
/// El móvil **no tiene `curl` ni `wget`** ⇒ el sondeo lo hace este código.
///
/// ⚠️ Interpretación (la que evitó que se matara el túnel por una key
/// caducada):
/// · `530/000/502/503/504` ⇒ **el túnel está muerto** (o no hay red).
/// · `401/403` ⇒ **el daemon respondió** ⇒ el túnel está **VIVO**: es la
///   credencial, **no** la red ⇒ **no se toca el túnel**.
pub fn probe_url(url: &str) -> (u16, bool) {
    let (code, es_daemon, _detalle) = probe_url_detalle(url);
    (code, es_daemon)
}

/// Como `probe_url`, pero además devuelve el **detalle** del 403 (`reason`,
/// `sent_key_fp`, `key_path`) que el daemon ya expone (`forbidden_body`).
///
/// ⇒ El watchdog puede distinguir "no hay ninguna key configurada" de "la
/// credencial no coincide" **sin abrir el móvil** y **sin parsear el hint**.
pub fn probe_url_detalle(url: &str) -> (u16, bool, Value) {
    let cfg = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(12)))
        .timeout_connect(Some(Duration::from_secs(5)))
        .http_status_as_error(false)
        .build();
    let agent = ureq::Agent::new_with_config(cfg);
    let objetivo = format!("{}/mcp", url.trim_end_matches('/'));
    match agent
        .post(&objetivo)
        .header("Content-Type", "application/json")
        .send("{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}")
    {
        Ok(resp) => {
            let code = resp.status().as_u16();
            let cuerpo = resp.into_body().read_to_string().unwrap_or_default();
            let es_daemon = cuerpo.contains("\"error\"") || cuerpo.contains("\"jsonrpc\"");
            let detalle: Value = serde_json::from_str(&cuerpo).unwrap_or(Value::Null);
            (code, es_daemon, detalle)
        }
        Err(e) => {
            // `ureq::Error::StatusCode(u16)`: la variante YA es el codigo.
            let code = match e {
                ureq::Error::StatusCode(c) => c,
                _ => 0,
            };
            (code, false, Value::Null)
        }
    }
}

/// Un código HTTP **no** es motivo para matar el túnel por sí solo.
pub fn es_canal_muerto(code: u16) -> bool {
    matches!(code, 0 | 502 | 503 | 504 | 530)
}

/// Payload **público** del canal: lo único que hace falta para **redescubrir
/// el canal** son las URLs.
///
/// ⚠️ `ip_publica` y `cloudflared_pids` **NO** se publican por defecto: son
/// dato de reconocimiento del dispositivo y no aportan al propósito (lo pidió
/// el agente de cockpit/web). Se quedan en el `channel.json` **local** del
/// móvil, que es privado.
pub fn public_payload(estado: &Value) -> Value {
    json!({
        "quick": estado.get("quick").cloned().unwrap_or(Value::Null),
        "duckdns": estado.get("duckdns").cloned().unwrap_or(Value::Null),
        "canonical": canonical_efectiva(),
        "ts": now_iso(),
    })
}

/// Estado completo del canal + publicación en paste (si se puede).
///
/// * `quick`: la URL del *quick tunnel* vigente.
/// * `duckdns`: la URL estable (dominio que no rota).
/// * `ip_publica`: la IP que ve el propio host.
/// * `pids`: pids de `cloudflared` (los deja el watchdog).
///
/// El paste publicado contiene el JSON **con la URL del paste anterior**
/// ⇒ el sandbox que conoce un paste aprende el siguiente (cadena
/// auto-renovable) ⇒ aunque el túnel esté muerto, el sandbox puede
/// redescubrir la URL cuando el túnel vuelva.
pub fn publish(
    state_dir: Option<&str>,
    quick: Option<&str>,
    duckdns: Option<&str>,
    ip_publica: Option<&str>,
    pids: Option<&str>,
) -> Value {
    // 1) fusionar los campos en channel.json
    if let Some(u) = quick {
        let _ = write_url_file(state_dir, TUNNEL_URL, u);
        let _ = merge_channel_field(state_dir, "quick", json!(u));
    }
    if let Some(u) = duckdns {
        let _ = write_url_file(state_dir, CHANNEL_URL, u);
        let _ = merge_channel_field(state_dir, "duckdns", json!(u));
    }
    if let Some(ip) = ip_publica {
        let _ = merge_channel_field(state_dir, "ip_publica", json!(ip));
    }
    if let Some(p) = pids {
        let _ = merge_channel_field(state_dir, "cloudflared_pids", json!(p));
    }

    let mut estado = read_channel_json(state_dir).unwrap_or_else(|| json!({}));
    let paste_previo = estado
        .get("paste")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // 2) el paste lleva la URL del paste anterior (cadena auto-renovable), la
    //    URL canónica y **sólo las URLs** (nada de IP ni de PIDs: eso queda en
    //    el channel.json local, que es privado).
    if let Some(o) = estado.as_object_mut() {
        o.insert("prev_paste".to_string(), json!(paste_previo));
        o.insert("canonical".to_string(), json!(canonical_efectiva()));
        o.insert("ts".to_string(), json!(now_iso()));
    }
    let mut publico = public_payload(&estado);
    if let Some(o) = publico.as_object_mut() {
        o.insert("prev_paste".to_string(), json!(paste_previo));
    }
    let texto = serde_json::to_string_pretty(&publico).unwrap_or_default();
    match publish_paste(&texto) {
        Some(url_paste) => {
            if let Some(o) = estado.as_object_mut() {
                o.insert("paste".to_string(), json!(url_paste));
            }
            let _ = write_channel_json(state_dir, &estado);
            let mut salida = json!({
                "ok": true,
                "published": true,
                "paste": url_paste,
            });
            if let (Some(o), Some(s)) = (salida.as_object_mut(), estado.as_object()) {
                for k in ["quick", "duckdns", "ip_publica", "cloudflared_pids", "ts"] {
                    if let Some(v) = s.get(k) {
                        o.insert(k.to_string(), v.clone());
                    }
                }
            }
            salida
        }
        None => {
            // El paste falló: el estado local **sí** queda actualizado.
            let _ = write_channel_json(state_dir, &estado);
            json!({
                "ok": true,
                "published": false,
                "detail": "el paste no respondio; el estado local si queda escrito",
                "channel": estado,
            })
        }
    }
}

/// Instante actual en ISO-8601 (UTC).
pub fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days as i64);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

/// Días desde epoch ⇒ fecha civil (algoritmo de Howard Hinnant).
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

    fn dir_tmp(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(nombre);
        let _ = fs::create_dir_all(&d);
        d
    }

    #[test]
    fn el_json_del_canal_conserva_los_campos_al_fusionar() {
        let d = dir_tmp("md_channel_test1");
        let s = d.to_string_lossy().to_string();
        merge_channel_field(Some(&s), "quick", json!("https://a.trycloudflare.com"));
        merge_channel_field(Some(&s), "ip_publica", json!("1.2.3.4"));
        let v = read_channel_json(Some(&s)).unwrap();
        assert_eq!(
            v.get("quick").and_then(|x| x.as_str()),
            Some("https://a.trycloudflare.com")
        );
        assert_eq!(v.get("ip_publica").and_then(|x| x.as_str()), Some("1.2.3.4"));
        assert!(v.get("ts").is_some());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn el_estado_ignora_un_json_corrupto() {
        let d = dir_tmp("md_channel_test2");
        let s = d.to_string_lossy().to_string();
        fs::write(channel_path(Some(&s), CHANNEL_JSON), "{esto no es json").unwrap();
        assert!(read_channel_json(Some(&s)).is_none());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn quick_y_duckdns_viven_en_ficheros_distintos() {
        let d = dir_tmp("md_channel_test3");
        let s = d.to_string_lossy().to_string();
        write_url_file(Some(&s), TUNNEL_URL, "https://q.trycloudflare.com").unwrap();
        write_url_file(Some(&s), CHANNEL_URL, "https://mdtmfnc.duckdns.org").unwrap();
        let q = fs::read_to_string(channel_path(Some(&s), TUNNEL_URL)).unwrap();
        let k = fs::read_to_string(channel_path(Some(&s), CHANNEL_URL)).unwrap();
        assert!(q.contains("trycloudflare"));
        assert!(k.contains("duckdns"));
        assert_ne!(q, k);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn la_fecha_utc_es_coherente() {
        let s = now_iso();
        assert_eq!(s.len(), 20, "{s}");
        assert!(s.ends_with('Z'));
    }
}
