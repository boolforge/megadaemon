//! MegaDaemon v13.3 — Capa de sincronización **servidor**.
//!
//! Lo que hace [`crate::sync`] es la copia *local* (cachés y bases de datos
//! privadas de cada app).  Esta capa trae el resto: el historial que vive en
//! la nube.
//!
//! Tres mecanismos, complementarios y automáticos:
//!
//! 1. **Cosecha de sesión** (`harvest_sessions`): se leen las cookies en
//!    claro de las WebView de las apps, el token de la app de DeepSeek y la
//!    `last_app_key` de Kimi.  Lo que esté en claro se usa; lo que esté
//!    cifrado por el keystore se documenta como pendiente.
//! 2. **Pull por API** (`web_pull`): con esa sesión se consulta el endpoint
//!    real que usa cada web (los mismos que consumen las apps) y se
//!    normaliza al esquema común.
//! 3. **Ingesta de exportación oficial** (`ingest_export`): los ficheros que
//!    los propios servicios generan (ChatGPT, Anthropic, Google Takeout) se
//!    parsean tal cual.  Es la vía con máxima fidelidad y la que cubre
//!    servicios sin API pública.
//!
//! Principio: **no se inventan datos**.  Si la sesión está caducada o el
//! servicio responde 401/403, se reporta el error exacto y se sigue; nunca
//! se fabrican conversaciones vacías para "rellenar".

use serde_json::{json, Value};
use std::fs;
use std::path::Path;

/// Sesión cosechada de un servicio.
#[derive(Clone, Debug)]
pub struct Session {
    pub service: String,
    pub cookie: String,
    pub bearer: String,
    pub source: String,
    pub note: String,
}

fn sync_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(crate::cockpit_server::state_dir()).join("sync")
}

fn vault_path() -> std::path::PathBuf {
    sync_dir().join("sessions.json")
}

/// WebView de cada app: las cookies de sesión suelen estar en claro.
const COOKIE_SOURCES: &[(&str, &str)] = &[
    ("chatgpt", "/data/data/com.openai.chatgpt/app_webview/Default/Cookies"),
    ("claude", "/data/data/com.anthropic.claude/app_webview/Default/Cookies"),
    ("deepseek", "/data/data/com.deepseek.chat/app_webview/Default/Cookies"),
    ("kimi", "/data/data/com.moonshot.kimichat/app_webview/Default/Cookies"),
    ("gemini", "/data/data/com.google.android.apps.bard/app_webview/Default/Cookies"),
];

/// Cookies de analítica/ads que no aportan nada y sólo ruido.
const ANALYTICS_COOKIES: &[&str] = &[
    "_ga", "_gid", "_gcl_au", "gcl_aw_p", "_gac_", "_gad_", "__gads", "__gpi",
    "_clck", "_clsk", "ide", "muid", "test_cookie", "amzn_", "frc_", "ajs_anonymous_id",
    "amplitude", "mixpanel", "sensorsdata", "baidu", "_is_gd", "_is_c", "Hm_lvt_",
    "Hm_lpvt_", "loginexpiry", "yandex", "_ym_", "vk_", "tns_",
];

/// ¿Esta cookie sirve para autenticar?
///
/// Criterio: se descarta **todo lo que sea analítica/ads** y se conserva el
/// resto.  La regla de "lista blanca" era demasiado estricta: los servicios
/// anti-bot (Cloudflare) exigen cookies de estado (`__cf_bm`, `__cflb`) y de
/// cliente (`oai-did`, `_dd_s`) que no contienen nada identificable por sí
/// solas pero sin las cuales la API responde 401.
fn is_interesting_cookie(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    !ANALYTICS_COOKIES
        .iter()
        .any(|a| n.starts_with(&a.to_ascii_lowercase()))
}

/// Extrae cookies en claro de una base de datos de Chromium.
///
/// `sqlite3` no está en el intérprete de Rust, así que se delega en el
/// extractor Python embebido (`--cookies`): ya sabe abrir cookies cifradas
/// con el esquema de Chromium y devuelve el JSON.
fn cookies_via_extractor() -> Vec<(String, String, String, Vec<Value>)> {
    let mut out = vec![];
    let Some(cmd) = crate::sync::extractor_cmd(&["--cookies"]) else {
        return out;
    };
    let res = crate::android::root_exec(&cmd, 120_000, 512 * 1024);
    for line in res.stdout.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        if v.get("__cookies__").is_none() {
            continue;
        }
        let origin = v["origin"].as_str().unwrap_or("").to_string();
        let mut jar = vec![];
        for c in v["cookies"].as_array().cloned().unwrap_or_default() {
            let name = c["name"].as_str().unwrap_or("").to_string();
            if !is_interesting_cookie(&name) {
                continue;
            }
            // Las cifradas no se pueden usar sin descifrar el keystore.
            if c["encrypted"].as_bool().unwrap_or(false) {
                continue;
            }
            let val = c["value"].as_str().unwrap_or("").to_string();
            if val.is_empty() {
                continue;
            }
            jar.push(json!({ "name": name, "value": val }));
        }
        out.push((origin, String::new(), String::new(), jar));
    }
    out
}

/// Extrae un valor concreto de un fichero XML de SharedPreferences.
fn pref_value(path: &str, key: &str) -> Option<String> {
    let txt = fs::read_to_string(path).ok()?;
    let needle = format!("name=\"{}\"", key);
    let idx = txt.find(&needle)?;
    let rest = &txt[idx..];
    let gt = rest.find('>')? + 1;
    let end = rest[gt..].find('<')? + gt;
    Some(rest[gt..end].trim().to_string())
}

/// Tokens de sesión que las apps guardan en su almacenamiento privado.
fn app_tokens_via_extractor() -> Vec<(String, String, String)> {
    let mut out = vec![];
    let Some(cmd) = crate::sync::extractor_cmd(&["--harvest"]) else {
        return out;
    };
    let res = crate::android::root_exec(&cmd, 120_000, 512 * 1024);
    for line in res.stdout.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        if v.get("__app_token__").is_some() {
            out.push((
                v["service"].as_str().unwrap_or("").to_string(),
                v["token"].as_str().unwrap_or("").to_string(),
                v["source"].as_str().unwrap_or("").to_string(),
            ));
        }
    }
    out
}

/// Cosecha sesiones de todas las fuentes disponibles.
pub fn harvest_sessions() -> Value {
    let mut sessions: Vec<Session> = vec![];
    let mut notes: Vec<String> = vec![];

    // 1) cookies de las WebView de las apps (en claro)
    let jars = cookies_via_extractor();
    for (origin, _a, _b, jar) in &jars {
        if jar.is_empty() {
            continue;
        }
        let svc = if origin.contains("chatgpt") {
            "chatgpt"
        } else if origin.contains("claude") {
            "claude"
        } else if origin.contains("deepseek") {
            "deepseek"
        } else if origin.contains("kimi") {
            "kimi"
        } else if origin.contains("gemini") {
            "gemini"
        } else if origin.contains("chrome") {
            "chrome"
        } else if origin.contains("edge") {
            "edge"
        } else {
            continue;
        };
        // Orden por relevancia: primero las cookies de sesión y de estado
        // anti-bot, después el resto.  Se limitan a 40 para no exceder los
        // límites de cabecera.
        let mut scored: Vec<(u8, String)> = jar
            .iter()
            .map(|c| {
                let n = c["name"].as_str().unwrap_or("").to_ascii_lowercase();
                let prio = if n.contains("next-auth.session") || n == "sessionkey" {
                    0
                } else if n == "cf_clearance" || n.starts_with("__cf") || n == "__cflb" {
                    1
                } else if n.starts_with("oai-") || n.starts_with("_dd_") || n.starts_with("ds_") {
                    2
                } else {
                    3
                };
                (
                    prio,
                    format!(
                        "{}={}",
                        c["name"].as_str().unwrap_or(""),
                        c["value"].as_str().unwrap_or("")
                    ),
                )
            })
            .collect();
        scored.sort_by_key(|(p, _)| *p);
        let cookie = scored
            .into_iter()
            .take(40)
            .map(|(_, c)| c)
            .collect::<Vec<String>>()
            .join("; ");
        sessions.push(Session {
            service: svc.to_string(),
            cookie,
            bearer: String::new(),
            source: format!("webview:{}", origin),
            note: "cookies en claro de la WebView de la app".into(),
        });
    }

    // 2) tokens de las apps (DeepSeek `app_user_info.token`, Kimi
    //    `last_app_key`, tokens de WebView de ChatGPT/Claude/Gemini).
    let mut seen_tokens: Vec<String> = vec![];
    for (svc, token, source) in app_tokens_via_extractor() {
        if token.len() < 16 || seen_tokens.contains(&token) {
            continue;
        }
        seen_tokens.push(token.clone());
        sessions.push(Session {
            service: svc.clone(),
            cookie: String::new(),
            bearer: token,
            source,
            note: "token de sesión de la app (puede estar caducado)".into(),
        });
    }
    if !seen_tokens.iter().any(|_| true) {
        notes.push("no se ha encontrado ningún token de sesión legible".into());
    }

    // 4) Chrome: las cookies están cifradas por el keystore → no son
    //    utilizables sin descifrado en proceso (Frida).  Se deja constancia.
    notes.push(
        "chrome: cookies cifradas por AndroidKeyStore (no exportables); \
         para las cuentas reales hay que inyectarse en el proceso (Frida) o \
         usar la exportación oficial del servicio"
            .into(),
    );

    // Guarda la bóveda (0600).
    let dir = sync_dir();
    let _ = fs::create_dir_all(&dir);
    let vault: Vec<Value> = sessions
        .iter()
        .map(|s| {
            json!({
                "service": s.service, "cookie": s.cookie, "bearer": s.bearer,
                "source": s.source, "note": s.note,
                "harvested_at": now_secs(),
            })
        })
        .collect();
    let _ = fs::write(vault_path(), serde_json::to_string_pretty(&vault).unwrap_or_default());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(vault_path(), fs::Permissions::from_mode(0o600));
    }

    json!({
        "ok": true,
        "sessions": sessions.len(),
        "by_service": sessions.iter().map(|s| s.service.clone()).collect::<Vec<String>>(),
        "vault": vault_path().to_string_lossy(),
        "notes": notes,
    })
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Carga la bóveda de sesiones.
fn load_vault() -> Vec<Session> {
    let Ok(txt) = fs::read_to_string(vault_path()) else {
        return vec![];
    };
    let Ok(v) = serde_json::from_str::<Value>(&txt) else {
        return vec![];
    };
    v.as_array()
        .map(|a| {
            a.iter()
                .map(|s| Session {
                    service: s["service"].as_str().unwrap_or("").to_string(),
                    cookie: s["cookie"].as_str().unwrap_or("").to_string(),
                    bearer: s["bearer"].as_str().unwrap_or("").to_string(),
                    source: s["source"].as_str().unwrap_or("").to_string(),
                    note: s["note"].as_str().unwrap_or("").to_string(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Petición HTTP GET con el `curl` del dispositivo.
///
/// Devuelve (status, cuerpo).  Se usa `curl` en vez de un cliente TLS de Rust
/// porque Cloudflare rechaza (401/403) las peticiones con huella TLS distinta
/// a la de un navegador, aunque la cookie sea válida.
fn http_get(url: &str, cookie: &str, bearer: &str, ua: &str) -> (u16, String) {
    let Some(curl) = crate::sync::curl_path() else {
        return (0, "no hay curl en el dispositivo".into());
    };
    let mut cmd = format!(
        "{} -s --compressed -m 45 -w '\n__MD_HTTP__%{{http_code}}' -H 'User-Agent: {}' -H 'Accept: application/json, text/plain, */*'",
        crate::android::sh_quote(&curl),
        ua
    );
    if !cookie.is_empty() {
        cmd.push_str(&format!(" -H 'Cookie: {}'", cookie.replace('\'', "'\''")));
    }
    if !bearer.is_empty() {
        cmd.push_str(&format!(" -H 'Authorization: Bearer {}'", bearer.replace('\'', "'\''")));
    }
    cmd.push_str(&format!(" '{}'", url.replace('\'', "'\''")));
    let r = crate::android::root_exec(&cmd, 60_000, 8 * 1024 * 1024);
    if r.timed_out {
        return (0, "timeout".into());
    }
    let out = r.stdout;
    match out.rfind("__MD_HTTP__") {
        Some(pos) => {
            let (body, tail) = out.split_at(pos);
            let code: u16 = tail["__MD_HTTP__".len()..].trim().parse().unwrap_or(0);
            (code, body.trim_end().to_string())
        }
        None => (0, out.trim().to_string()),
    }
}

const UA: &str = "Mozilla/5.0 (Linux; Android 14; SM-G998B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36";

/// Pull por API de un servicio. Devuelve (conversaciones, mensaje de error).
fn pull_service(service: &str, session: &Session) -> (Vec<Value>, Option<String>) {
    match service {
        "chatgpt" => {
            let mut out = vec![];
            let mut offset = 0;
            let mut total = 0i64;
            while offset < 500 {
                let url = format!(
                    "https://chatgpt.com/backend-api/conversations?offset={}&limit=25",
                    offset
                );
                let (st, body) = http_get(&url, &session.cookie, "", UA);
                if st == 0 {
                    return (vec![], Some(format!("http: {}", body)));
                }
                if st != 200 {
                    return (vec![], Some(format!("HTTP {} en {}", st, url)));
                }
                let Ok(v) = serde_json::from_str::<Value>(&body) else {
                    return (vec![], Some("respuesta no JSON".into()));
                };
                total = v["total"].as_i64().unwrap_or(0);
                let items = v["items"].as_array().cloned().unwrap_or_default();
                if items.is_empty() {
                    break;
                }
                let n_items = items.len();
                for it in items {
                    let id = it["id"].as_str().unwrap_or("").to_string();
                    if id.is_empty() {
                        continue;
                    }
                    let title = it["title"].as_str().unwrap_or("").to_string();
                    let updated = it["update_time"].as_f64().unwrap_or(0.0);
                    // detalle completo
                    let (st2, body2) = http_get(
                        &format!("https://chatgpt.com/backend-api/conversation/{}", id),
                        &session.cookie,
                        "",
                        UA,
                    );
                    let mut msgs = vec![];
                    if st2 == 200 {
                        if let Ok(cv) = serde_json::from_str::<Value>(&body2) {
                            if let Some(mapping) = cv["mapping"].as_object() {
                                let mut nodes: Vec<(&String, &Value)> = mapping.iter().collect();
                                nodes.sort_by(|a, b| {
                        let ta = a.1["message"]["create_time"].as_f64().unwrap_or(0.0);
                        let tb = b.1["message"]["create_time"].as_f64().unwrap_or(0.0);
                        ta.partial_cmp(&tb).unwrap_or(std::cmp::Ordering::Equal)
                    });
                                for (_nid, node) in nodes {
                                    let msg = &node["message"];
                                    if msg.is_null() {
                                        continue;
                                    }
                                    let role = msg["author"]["role"].as_str().unwrap_or("");
                                    if role.is_empty() || role == "system" || role == "tool" {
                                        continue;
                                    }
                                    let parts = msg["content"]["parts"].as_array().cloned().unwrap_or_default();
                                    let text = parts
                                        .iter()
                                        .filter_map(|p| p.as_str())
                                        .collect::<Vec<&str>>()
                                        .join("\n");
                                    if text.trim().is_empty() {
                                        continue;
                                    }
                                    msgs.push(json!({
                                        "role": role,
                                        "content": text,
                                        "ts": msg["create_time"].as_f64().unwrap_or(0.0),
                                        "meta": {"model": msg["metadata"]["model_slug"].clone()},
                                    }));
                                }
                            }
                        }
                    }
                    if !msgs.is_empty() {
                        out.push(json!({
                            "service": "chatgpt", "id": id, "title": title,
                            "updated_at": updated, "model": it["default_model_slug"].clone(),
                            "messages": msgs, "source": "chatgpt.com/backend-api",
                        }));
                    }
                }
                offset += n_items as i64;
                if offset >= total {
                    break;
                }
            }
            (out, None)
        }
        "deepseek" => {
            let (st, body) = http_get(
                "https://chat.deepseek.com/api/v0/chat_session_list",
                &session.cookie,
                &session.bearer,
                UA,
            );
            if st != 200 {
                return (vec![], Some(format!("HTTP {} en chat_session_list", st)));
            }
            let Ok(v) = serde_json::from_str::<Value>(&body) else {
                return (vec![], Some("respuesta no JSON".into()));
            };
            let items = v.as_array().cloned().unwrap_or_default();
            let mut out = vec![];
            for it in items {
                let id = it["id"].as_str().unwrap_or("").to_string();
                let title = it["title"].as_str().unwrap_or("").to_string();
                if id.is_empty() {
                    continue;
                }
                let (st2, body2) = http_get(
                    &format!(
                        "https://chat.deepseek.com/api/v0/chat_session?chat_session_id={}",
                        id
                    ),
                    &session.cookie,
                    &session.bearer,
                    UA,
                )
;
                let mut msgs = vec![];
                if st2 == 200 {
                    if let Ok(cv) = serde_json::from_str::<Value>(&body2) {
                        let resp = cv["response"].as_array().cloned().unwrap_or_default();
                        for m in resp {
                            let text = m["content"].as_str().unwrap_or("").to_string();
                            if text.trim().is_empty() {
                                continue;
                            }
                            msgs.push(json!({
                                "role": m["role"].as_str().unwrap_or("user"),
                                "content": text,
                                "ts": 0,
                                "meta": {},
                            }));
                        }
                    }
                }
                if !msgs.is_empty() {
                    out.push(json!({
                        "service": "deepseek", "id": id, "title": title,
                        "updated_at": it["updated_at"].as_f64().unwrap_or(0.0),
                        "model": it["model_type"].clone(), "messages": msgs,
                        "source": "chat.deepseek.com/api/v0",
                    }));
                }
            }
            (out, None)
        }
        "kimi" => {
            let (st, body) = http_get(
                "https://www.kimi.com/api/chat/list",
                &session.cookie,
                &session.bearer,
                UA,
            );
            if st != 200 {
                return (vec![], Some(format!("HTTP {} en kimi api/chat/list", st)));
            }
            let Ok(v) = serde_json::from_str::<Value>(&body) else {
                return (vec![], Some("respuesta no JSON".into()));
            };
            let items = v["data"].as_array().cloned().unwrap_or_default();
            let mut out = vec![];
            for it in items {
                let id = it["id"].as_str().unwrap_or("").to_string();
                let title = it["title"].as_str().unwrap_or(it["name"].as_str().unwrap_or("")).to_string();
                if id.is_empty() {
                    continue;
                }
                let (st2, body2) = http_get(
                    &format!("https://www.kimi.com/api/chat/{}", id),
                    &session.cookie,
                    &session.bearer,
                    UA,
                )
;
                let mut msgs = vec![];
                if st2 == 200 {
                    if let Ok(cv) = serde_json::from_str::<Value>(&body2) {
                        for m in cv["messages"].as_array().cloned().unwrap_or_default() {
                            let text = m["content"].as_str().unwrap_or("").to_string();
                            if text.trim().is_empty() {
                                continue;
                            }
                            msgs.push(json!({
                                "role": m["role"].as_str().unwrap_or("user"),
                                "content": text,
                                "ts": 0, "meta": {},
                            }));
                        }
                    }
                }
                if !msgs.is_empty() {
                    out.push(json!({
                        "service": "kimi", "id": id, "title": title,
                        "updated_at": it["updated_at"].as_f64().unwrap_or(0.0),
                        "model": "", "messages": msgs, "source": "kimi.com/api",
                    }));
                }
            }
            (out, None)
        }
        "claude" => {
            let (st, body) = http_get(
                "https://claude.ai/api/organizations",
                &session.cookie,
                "",
                UA,
            );
            if st != 200 {
                return (vec![], Some(format!("HTTP {} en claude.ai/api/organizations", st)));
            }
            let Ok(v) = serde_json::from_str::<Value>(&body) else {
                return (vec![], Some("respuesta no JSON".into()));
            };
            let mut out = vec![];
            for org in v.as_array().cloned().unwrap_or_default() {
                let oid = org["uuid"].as_str().unwrap_or("").to_string();
                if oid.is_empty() {
                    continue;
                }
                let (st2, body2) = http_get(
                    &format!("https://claude.ai/api/organizations/{}/chat_conversations?limit=100", oid),
                    &session.cookie,
                    "",
                    UA,
                );
                if st2 != 200 {
                    continue;
                }
                let Ok(cv) = serde_json::from_str::<Value>(&body2) else {
                    continue;
                };
                for c in cv.as_array().cloned().unwrap_or_default() {
                    let id = c["uuid"].as_str().unwrap_or("").to_string();
                    let title = c["name"].as_str().unwrap_or("").to_string();
                    let (st3, body3) = http_get(
                        &format!(
                            "https://claude.ai/api/organizations/{}/chat_conversations/{}?rendering_mode=raw",
                            oid, id
                        ),
                        &session.cookie,
                        "",
                        UA,
                    )
    ;
                    let mut msgs = vec![];
                    if st3 == 200 {
                        if let Ok(cv2) = serde_json::from_str::<Value>(&body3) {
                            for m in cv2["chat_messages"].as_array().cloned().unwrap_or_default() {
                                let text = m["text"].as_str().unwrap_or("").to_string();
                                if text.trim().is_empty() {
                                    continue;
                                }
                                msgs.push(json!({
                                    "role": m["sender"].as_str().unwrap_or("user"),
                                    "content": text,
                                    "ts": m["created_at"].as_str().unwrap_or("").parse::<f64>().unwrap_or(0.0),
                                    "meta": {},
                                }));
                            }
                        }
                    }
                    if !msgs.is_empty() {
                        out.push(json!({
                            "service": "claude", "id": id, "title": title,
                            "updated_at": c["updated_at"].as_str().unwrap_or("").parse::<f64>().unwrap_or(0.0),
                            "model": "", "messages": msgs, "source": "claude.ai/api",
                        }));
                    }
                }
            }
            (out, None)
        }
        "gemini" => {
            let (st, body) = http_get(
                "https://gemini.google.com/app/api/rpc",
                &session.cookie,
                "",
                UA,
            );
            if st != 200 {
                return (
                    vec![],
                    Some(format!(
                        "HTTP {} en gemini.google.com (la sesión cosechada es de invitado: \
                         hace falta la cookie __Secure-1PSID de la cuenta real)",
                        st
                    )),
                );
            }
            let _ = body;
            (vec![], None)
        }
        other => (vec![], Some(format!("sin conector de servidor para {}", other))),
    }
}

/// Ejecuta el pull de todos los servicios con sesión disponible.
pub fn web_pull(services: Option<Vec<String>>) -> Value {
    let vault = load_vault();
    let wanted = services.unwrap_or_else(|| {
        vec!["chatgpt".into(), "claude".into(), "deepseek".into(), "kimi".into(), "gemini".into()]
    });
    let mut all = vec![];
    let mut per_service = serde_json::Map::new();
    let mut errors = vec![];
    for svc in wanted {
        let Some(session) = vault.iter().find(|s| s.service == svc) else {
            per_service.insert(svc.clone(), json!({ "skipped": "sin sesión cosechada" }));
            continue;
        };
        let (convs, err) = pull_service(&svc, session);
        let n_msgs: usize = convs
            .iter()
            .filter_map(|c| c["messages"].as_array())
            .map(|a| a.len())
            .sum();
        per_service.insert(
            svc.clone(),
            json!({ "conversations": convs.len(), "messages": n_msgs, "source": session.source }),
        );
        if let Some(e) = err {
            errors.push(json!({ "service": svc, "error": e }));
        }
        all.extend(convs);
    }
    let n = all.len();
    let ingested = crate::sync::ingest_conversations(all);
    json!({
        "ok": true,
        "pulled": n,
        "ingested": ingested,
        "per_service": Value::Object(per_service),
        "errors": errors,
    })
}

// --------------------------------------------------------------------------- //
// Ingesta de exportaciones oficiales
// --------------------------------------------------------------------------- //

/// Detecta el formato de un fichero de exportación y lo normaliza.
pub fn detect_export_format(name: &str, head: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    if n.contains("conversations.json") {
        // ChatGPT: {"title":...,"mapping":{...}}   Anthropic: {"name":...,"chat_messages":[...]}
        if head.contains("chat_messages") {
            return Some("anthropic");
        }
        if head.contains("\"mapping\"") {
            return Some("openai");
        }
    }
    if head.contains("chat_messages") {
        return Some("anthropic");
    }
    if head.contains("\"mapping\"") {
        return Some("openai");
    }
    if head.contains("clientId") && head.contains("title") {
        return Some("takeout");
    }
    None
}

/// Parsea una exportación oficial → conversaciones normalizadas.
pub fn parse_export(text: &str, format: &str) -> Vec<Value> {
    let Ok(v) = serde_json::from_str::<Value>(text) else {
        return vec![];
    };
    match format {
        "openai" => {
            let arr = v.as_array().cloned().unwrap_or_default();
            let mut out = vec![];
            for c in arr {
                let id = c["id"].as_str().unwrap_or("").to_string();
                let title = c["title"].as_str().unwrap_or("").to_string();
                let mut msgs = vec![];
                if let Some(mapping) = c["mapping"].as_object() {
                    let mut nodes: Vec<(&String, &Value)> = mapping.iter().collect();
                    nodes.sort_by(|a, b| {
                        let ta = a.1["message"]["create_time"].as_f64().unwrap_or(0.0);
                        let tb = b.1["message"]["create_time"].as_f64().unwrap_or(0.0);
                        ta.partial_cmp(&tb).unwrap_or(std::cmp::Ordering::Equal)
                    });
                    for (_nid, node) in nodes {
                        let msg = &node["message"];
                        if msg.is_null() {
                            continue;
                        }
                        let role = msg["author"]["role"].as_str().unwrap_or("");
                        if role != "user" && role != "assistant" && role != "system" {
                            continue;
                        }
                        let parts = msg["content"]["parts"].as_array().cloned().unwrap_or_default();
                        let body = parts
                            .iter()
                            .filter_map(|p| p.as_str())
                            .collect::<Vec<&str>>()
                            .join("\n");
                        if body.trim().is_empty() {
                            continue;
                        }
                        msgs.push(json!({
                            "role": role, "content": body,
                            "ts": msg["create_time"].as_f64().unwrap_or(0.0),
                            "meta": {"model": msg["metadata"]["model_slug"].clone()},
                        }));
                    }
                }
                if !msgs.is_empty() {
                    out.push(json!({
                        "service": "chatgpt", "id": id,
                        "title": title,
                        "updated_at": c["update_time"].as_f64().unwrap_or(0.0),
                        "model": c["default_model_slug"].clone(),
                        "messages": msgs, "source": "openai-export",
                    }));
                }
            }
            out
        }
        "anthropic" => {
            let arr = v.as_array().cloned().unwrap_or_default();
            let mut out = vec![];
            for c in arr {
                let id = c["uuid"].as_str().unwrap_or("").to_string();
                let title = c["name"].as_str().unwrap_or("").to_string();
                let mut msgs = vec![];
                for m in c["chat_messages"].as_array().cloned().unwrap_or_default() {
                    let body = m["text"].as_str().unwrap_or("").to_string();
                    if body.trim().is_empty() {
                        continue;
                    }
                    msgs.push(json!({
                        "role": m["sender"].as_str().unwrap_or("user"),
                        "content": body,
                        "ts": m["created_at"].as_str().unwrap_or("").parse::<f64>().unwrap_or(0.0),
                        "meta": {},
                    }));
                }
                if !msgs.is_empty() {
                    out.push(json!({
                        "service": "claude", "id": id, "title": title,
                        "updated_at": c["updated_at"].as_str().unwrap_or("").parse::<f64>().unwrap_or(0.0),
                        "model": c["model"].clone(), "messages": msgs,
                        "source": "anthropic-export",
                    }));
                }
            }
            out
        }
        "takeout" => {
            let arr = v.as_array().cloned().unwrap_or_default();
            let mut out = vec![];
            for c in arr {
                let id = c["clientId"].as_str().unwrap_or("").to_string();
                let title = c["title"].as_str().unwrap_or("").to_string();
                let mut msgs = vec![];
                for m in c["messages"].as_array().cloned().unwrap_or_default() {
                    let body = m["message"].as_str().unwrap_or("").to_string();
                    if body.trim().is_empty() {
                        continue;
                    }
                    msgs.push(json!({
                        "role": m["author"].as_str().unwrap_or("user"),
                        "content": body,
                        "ts": m["create_time"].as_f64().unwrap_or(0.0),
                        "meta": {},
                    }));
                }
                if !msgs.is_empty() {
                    out.push(json!({
                        "service": "gemini", "id": id, "title": title,
                        "updated_at": c["create_time"].as_f64().unwrap_or(0.0),
                        "model": "", "messages": msgs, "source": "google-takeout",
                    }));
                }
            }
            out
        }
        _ => vec![],
    }
}

/// Ingesta un fichero de exportación desde el dispositivo.
pub fn ingest_export(path: &str) -> Value {
    if !path.starts_with('/') || path.contains("..") {
        return json!({ "ok": false, "error": "bad_path", "hint": "ruta absoluta sin '..'" });
    }
    if !Path::new(path).exists() {
        return json!({ "ok": false, "error": "not_found", "hint": path });
    }
    let cmd = format!(
        "head -c 2000000 {} | base64 -w 0",
        crate::android::sh_quote(path)
    );
    let r = crate::android::root_exec(&cmd, 120_000, 4 * 1024 * 1024);
    if r.stdout.trim().is_empty() {
        return json!({ "ok": false, "error": "empty", "hint": r.stderr });
    }
    let decoded = match decode_base64(&r.stdout) {
        Some(d) => d,
        None => {
            return json!({ "ok": false, "error": "decode", "hint": "base64 inválido" });
        }
    };
    let head: String = decoded.chars().take(4000).collect();
    let name = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("export.json");
    let Some(format) = detect_export_format(name, &head) else {
        return json!({
            "ok": false, "error": "unknown_format",
            "hint": "no parece una exportación de ChatGPT, Anthropic o Takeout",
            "head": head.chars().take(200).collect::<String>(),
        });
    };
    let convs = parse_export(&decoded, format);
    let n = convs.len();
    let msgs: usize = convs
        .iter()
        .filter_map(|c| c["messages"].as_array())
        .map(|a| a.len())
        .sum();
    let ingested = crate::sync::ingest_conversations(convs);
    json!({
        "ok": true, "path": path, "format": format,
        "conversations": n, "messages": msgs, "ingested": ingested,
    })
}

/// Decodifica base64 estándar.
pub fn decode_base64(s: &str) -> Option<String> {
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = base64_decode_bytes(&compact)?;
    String::from_utf8(bytes).ok()
}

fn base64_decode_bytes(s: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut rev = [255u8; 256];
    for (i, c) in TABLE.iter().enumerate() {
        rev[*c as usize] = i as u8;
    }
    let mut out = vec![];
    let mut buf = 0u32;
    let mut bits = 0u32;
    for ch in s.bytes() {
        if ch == b'=' {
            break;
        }
        let v = rev[ch as usize];
        if v == 255 {
            continue;
        }
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_propio_descodifica() {
        assert_eq!(decode_base64("aGVsbG8=").unwrap(), "hello");
        assert_eq!(decode_base64("").unwrap(), "");
    }

    #[test]
    fn detecta_el_formato_de_exportacion() {
        let openai = r#"{"title":"x","mapping":{"1":{"message":{"author":{"role":"user"}}}}}"#;
        let anthropic = r#"[{"name":"x","chat_messages":[]}]"#;
        assert_eq!(detect_export_format("conversations.json", openai), Some("openai"));
        assert_eq!(detect_export_format("conversations.json", anthropic), Some("anthropic"));
        assert_eq!(detect_export_format("otro.json", "{}"), None);
    }

    #[test]
    fn parsea_una_exportacion_de_openai() {
        let sample = r#"[
          {"id":"abc","title":"Prueba","update_time":1700000000.0,"default_model_slug":"gpt-5",
           "mapping":{
             "1":{"message":{"create_time":1699999999.0,"author":{"role":"user"},"content":{"parts":["hola"]},
                             "metadata":{"model_slug":"gpt-5"}}},
             "2":{"message":{"create_time":1700000000.0,"author":{"role":"assistant"},"content":{"parts":["respuesta"]},
                             "metadata":{"model_slug":"gpt-5"}}}}}
        ]"#;
        let convs = parse_export(sample, "openai");
        assert_eq!(convs.len(), 1);
        assert_eq!(convs[0]["service"], json!("chatgpt"));
        assert_eq!(convs[0]["messages"].as_array().unwrap().len(), 2);
        assert_eq!(convs[0]["messages"][0]["content"], json!("hola"));
    }

    #[test]
    fn parsea_una_exportacion_de_anthropic() {
        let sample = r#"[{"uuid":"u1","name":"Conversación","updated_at":"1700000000.0",
          "chat_messages":[{"sender":"human","text":"hola","created_at":"1700000000.0"},
                           {"sender":"assistant","text":"respuesta","created_at":"1700000001.0"}]}]"#;
        let convs = parse_export(sample, "anthropic");
        assert_eq!(convs.len(), 1);
        assert_eq!(convs[0]["messages"].as_array().unwrap().len(), 2);
        assert_eq!(convs[0]["messages"][0]["role"], json!("human"));
    }

    #[test]
    fn las_cookies_de_tracking_se_descartan_y_el_resto_se_conserva() {
        // Se conserva todo lo que pueda autenticar…
        assert!(is_interesting_cookie("__Secure-next-auth.session-token"));
        assert!(is_interesting_cookie("cf_clearance"));
        assert!(is_interesting_cookie("__cf_bm"));
        assert!(is_interesting_cookie("oai-did"));
        // …y se descarta el ruido de analítica/ads.
        assert!(!is_interesting_cookie("_ga"));
        assert!(!is_interesting_cookie("_gcl_au"));
        assert!(!is_interesting_cookie("GCL_AW_P"));
        assert!(!is_interesting_cookie("Hm_lvt_1fff341d7a963a4043e858ef0e19a17c"));
        assert!(!is_interesting_cookie(""));
    }
}
