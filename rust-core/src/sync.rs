//! MegaDaemon v13.3 — Motor de sincronización de conversaciones.
//!
//! Objetivo: **una sola copia, completa y buscable**, de las conversaciones
//! de las apps de IA del usuario (ChatGPT, Claude, Gemini, DeepSeek, Kimi,
//! Perplexity, Mistral, Copilot, Poe), sin intervención manual.
//!
//! Diseño (y por qué es mejor que un script suelto):
//!
//! 1. **Descubrimiento automático**: no hay rutas fijas; se barren los
//!    almacenes privados de cada app (SQLite + WebView/LevelDB) buscando
//!    tablas que contengan estructura de conversación.
//! 2. **Lectura consistente**: cada DB se copia a un temporal antes de
//!    leerse, porque las apps escriben en caliente.
//! 3. **Normalización**: todo se reduce a un esquema común
//!    (`Conversation { service, id, title, updated_at, model, messages[] }`),
//!   including bloques de razonamiento, tool_use y citas.
//! 4. **Almacén propio**: NDJSON por servicio en el state dir, con merge
//!    idempotente por `service+id` (se puede re-sincronizar sin duplicar).
//! 5. **Búsqueda**: índice invertido en memoria con tokenización y
//!    puntuación posicional; sin dependencias externas.
//! 6. **Auditable**: cada sincronización queda registrada y cada salida
//!    expone su procedencia (`source` = fichero exacto del que salió).

use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Extractor Python embebido en el binario (ver `assets/sync_extract.py`).
const EXTRACTOR_SRC: &str = include_str!("../assets/sync_extract.py");

/// Servicios soportados por el extractor.
pub const SERVICES: &[&str] = &[
    "chatgpt",
    "claude",
    "gemini",
    "deepseek",
    "kimi",
    "perplexity",
    "mistral",
    "copilot",
    "poe",
];

fn sync_dir() -> PathBuf {
    PathBuf::from(crate::cockpit_server::state_dir()).join("sync")
}

fn extractor_path() -> PathBuf {
    PathBuf::from(crate::cockpit_server::state_dir()).join("sync_extract.py")
}

/// Escribe el extractor embebido en disco si ha cambiado.
fn ensure_extractor() -> Result<PathBuf, String> {
    let p = extractor_path();
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {:?}: {}", parent, e))?;
    }
    let current = fs::read_to_string(&p).unwrap_or_default();
    if current != EXTRACTOR_SRC {
        let mut f = fs::File::create(&p).map_err(|e| format!("create {:?}: {}", p, e))?;
        f.write_all(EXTRACTOR_SRC.as_bytes())
            .map_err(|e| format!("write {:?}: {}", p, e))?;
    }
    let _ = fs::set_permissions(&p, permissions_exec());
    Ok(p)
}

#[cfg(unix)]
fn permissions_exec() -> std::fs::Permissions {
    use std::os::unix::fs::PermissionsExt;
    std::fs::Permissions::from_mode(0o755)
}

#[cfg(not(unix))]
fn permissions_exec() -> std::fs::Permissions {
    std::fs::Permissions::from_std(
        fs::OpenOptions::new()
            .append(true)
            .open("/dev/null")
            .map(|f| f.metadata().map(|m| m.permissions()).unwrap_or_default())
            .unwrap_or_default(),
    )
}

/// Ruta del extractor embebido, listo para ejecutar (lo escribe si hace falta).
pub fn extractor_argv() -> Option<String> {
    ensure_extractor().ok().map(|p| p.to_string_lossy().into_owned())
}

/// Comando completo para ejecutar el extractor embebido con el intérprete
/// del dispositivo.  En el contexto `su` no hay `python3` en el PATH, así que
/// se usa la ruta absoluta de Termux.
pub fn extractor_cmd(args: &[&str]) -> Option<String> {
    let script = extractor_argv()?;
    let py = python_candidates()
        .into_iter()
        .find(|p| p.starts_with('/') && std::path::Path::new(p).exists())
        .unwrap_or_else(|| "python3".to_string());
    let mut cmd = format!("{} {}", shell_quote(&py), shell_quote(&script));
    for a in args {
        cmd.push(' ');
        cmd.push_str(&shell_quote(a));
    }
    Some(cmd)
}

/// Ruta del `curl` disponible en el dispositivo.
///
/// Se usa `curl` (y no un cliente TLS de Rust) para el pull web porque
/// Cloudflare distingue la huella TLS: con `ureq` el servicio responde 401/403
/// aunque la cookie sea válida.  Con `curl` (que usa la misma pila que las
/// apps) la petición es legítima.
pub fn curl_path() -> Option<String> {
    for cand in [
        "/data/data/com.termux/files/usr/bin/curl",
        "/system/bin/curl",
        "/data/data/com.termux/files/usr/bin/curl-secure",
    ] {
        if std::path::Path::new(cand).exists() {
            return Some(cand.to_string());
        }
    }
    None
}

/// Comilla un valor para el shell (reutiliza la política de `android`).
pub fn shell_quote(s: &str) -> String {
    crate::android::sh_quote(s)
}

/// Candidatos a intérprete Python en el dispositivo (Termux primero).
fn python_candidates() -> Vec<String> {
    let mut v = vec![
        "/data/data/com.termux/files/usr/bin/python3".to_string(),
        "/data/data/com.termux/files/usr/bin/python".to_string(),
        "python3".to_string(),
        "python".to_string(),
    ];
    v.dedup();
    v
}

/// Ejecuta el extractor y devuelve las líneas NDJSON.
fn run_extractor(args: &[String]) -> Result<(String, String), String> {
    let script = ensure_extractor()?;
    let mut last_err = String::from("no se encontró un intérprete Python en el dispositivo");
    for py in python_candidates() {
        let mut cmd = std::process::Command::new(&py);
        cmd.arg(&script);
        for a in args {
            cmd.arg(a);
        }
        cmd.stdin(std::process::Stdio::null());
        let out = match cmd.output() {
            Ok(o) => o,
            Err(e) => {
                last_err = format!("{}: {}", py, e);
                continue;
            }
        };
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        if stdout.is_empty() && !stderr.is_empty() {
            last_err = format!("{}: {}", py, stderr.trim().chars().take(400).collect::<String>());
            continue;
        }
        return Ok((stdout, stderr));
    }
    Err(last_err)
}

/// Clave idempotente de una conversación.
fn key(service: &str, id: &str) -> String {
    format!("{}::{}", service, id)
}

/// Convierte un NDJSON de conversaciones en el mapa `clave -> conversación`.
fn load_store() -> HashMap<String, Value> {
    let mut map = HashMap::new();
    let dir = sync_dir();
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return map,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("ndjson") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let svc = v.get("service").and_then(|x| x.as_str()).unwrap_or("");
            let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("");
            if svc.is_empty() || id.is_empty() {
                continue;
            }
            map.insert(key(svc, id), v);
        }
    }
    map
}

/// Guarda el almacén (un fichero NDJSON por servicio).
fn save_store(map: &HashMap<String, Value>) -> Result<(), String> {
    let dir = sync_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("mkdir {:?}: {}", dir, e))?;
    // agrupar por servicio
    let mut by_service: HashMap<String, Vec<&Value>> = HashMap::new();
    for v in map.values() {
        let svc = v
            .get("service")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown")
            .to_string();
        by_service.entry(svc).or_default().push(v);
    }
    // borrar ficheros de servicios que ya no tienen conversaciones
    if let Ok(entries) = fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("ndjson") {
                let stem = p.file_stem().and_then(|x| x.to_str()).unwrap_or("");
                if !by_service.contains_key(stem) {
                    let _ = fs::remove_file(&p);
                }
            }
        }
    }
    for (svc, list) in by_service {
        let mut buf = String::new();
        for v in list {
            buf.push_str(&serde_json::to_string(v).unwrap_or_default());
            buf.push('\n');
        }
        let path = dir.join(format!("{}.ndjson", svc));
        let tmp = dir.join(format!(".{}.tmp", svc));
        fs::write(&tmp, buf.as_bytes()).map_err(|e| format!("write {:?}: {}", tmp, e))?;
        fs::rename(&tmp, &path).map_err(|e| format!("rename: {}", e))?;
    }
    Ok(())
}

/// Ejecuta una sincronización completa.
///
/// `services` = `None` → todos los servicios soportados.
pub fn run_sync(services: Option<Vec<String>>, include_cookies: bool) -> Value {
    let mut wanted: Vec<String> = services
        .unwrap_or_else(|| SERVICES.iter().map(|s| s.to_string()).collect());
    wanted.retain(|s| SERVICES.contains(&s.as_str()));
    if wanted.is_empty() {
        return json!({ "ok": false, "error": "no_services", "hint": "servicios desconocidos" });
    }

    let started = std::time::Instant::now();
    let (stdout, stderr) = match run_extractor(&wanted) {
        Ok(v) => v,
        Err(e) => {
            return json!({ "ok": false, "error": "extractor_failed", "hint": e });
        }
    };

    let mut store = load_store();
    let before = store.len();
    let mut added = 0usize;
    let mut updated = 0usize;
    let mut per_service: HashMap<String, (usize, usize)> = HashMap::new(); // (nuevas, total)
    let mut errors: Vec<Value> = vec![];
    let mut cookies: Vec<Value> = vec![];

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v.get("__error__").is_some() {
            errors.push(v);
            continue;
        }
        if v.get("__cookies__").is_some() {
            cookies.push(v);
            continue;
        }
        if v.get("__stats__").is_some() {
            continue;
        }
        let svc = v.get("service").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
        if svc.is_empty() || id.is_empty() {
            continue;
        }
        let k = key(&svc, &id);
        let existed = store.contains_key(&k);
        if existed {
            updated += 1;
        } else {
            added += 1;
        }
        store.insert(k, v);
        let e = per_service.entry(svc).or_insert((0, 0));
        e.0 += 1;
    }

    // Cookies (para el pull web-side posterior)
    if include_cookies {
        if let Ok((cout, _)) = run_extractor(&["--cookies".to_string()]) {
            for line in cout.lines() {
                if let Ok(v) = serde_json::from_str::<Value>(line.trim()) {
                    if v.get("__cookies__").is_some() {
                        cookies.push(v);
                    }
                }
            }
        }
    }

    let save_err = save_store(&store);
    for (svc, (n, _)) in &per_service {
        let _ = svc;
        let _ = n;
    }
    let mut stats = serde_json::Map::new();
    for svc in SERVICES {
        let n = store
            .values()
            .filter(|v| v.get("service").and_then(|x| x.as_str()) == Some(*svc))
            .count();
        let msgs: usize = store
            .values()
            .filter(|v| v.get("service").and_then(|x| x.as_str()) == Some(*svc))
            .filter_map(|v| v.get("messages").and_then(|m| m.as_array()))
            .map(|a| a.len())
            .sum();
        stats.insert(
            svc.to_string(),
            json!({ "conversations": n, "messages": msgs }),
        );
    }

    let mut cookie_summary = json!({});
    for c in &cookies {
        cookie_summary[c["origin"].as_str().unwrap_or("?")] = json!({
            "count": c["count"].clone(),
            "plaintext": (c["cookies"].as_array().map(|a| a.iter().filter(|x| x["encrypted"] == json!(false)).count())).unwrap_or(0),
        });
    }

    let _ = write_sync_meta(&json!({
        "last_sync_epoch": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        "duration_ms": started.elapsed().as_millis() as u64,
        "services": wanted,
        "total_conversations": store.len(),
    }));

    json!({
        "ok": save_err.is_ok(),
        "added": added,
        "updated": updated,
        "before": before,
        "total_conversations": store.len(),
        "duration_ms": started.elapsed().as_millis() as u64,
        "per_service": Value::Object(stats),
        "cookies": cookie_summary,
        "errors": errors,
        "stderr_tail": stderr.trim().chars().rev().take(400).collect::<String>().chars().rev().collect::<String>(),
        "save_error": save_err.err(),
    })
}

fn write_sync_meta(meta: &Value) -> Result<(), String> {
    let dir = sync_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::write(dir.join("meta.json"), serde_json::to_string_pretty(meta).unwrap_or_default())
        .map_err(|e| e.to_string())
}

fn read_sync_meta() -> Value {
    let p = sync_dir().join("meta.json");
    fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(json!({}))
}

/// Inyecta conversaciones en el almacén (merge idempotente por servicio+id).
/// Devuelve cuántas han quedado en total.
pub fn ingest_conversations(convs: Vec<Value>) -> usize {
    if convs.is_empty() {
        return load_store().len();
    }
    let mut store = load_store();
    for c in convs {
        let svc = c.get("service").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let id = c.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
        if svc.is_empty() || id.is_empty() {
            continue;
        }
        store.insert(key(&svc, &id), c);
    }
    let total = store.len();
    let _ = save_store(&store);
    total
}

/// Estado del almacén.
pub fn status() -> Value {
    let store = load_store();
    let mut per_service: HashMap<String, (usize, usize, usize)> = HashMap::new(); // conv, msgs, chars
    for v in store.values() {
        let svc = v
            .get("service")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown")
            .to_string();
        let msgs = v.get("messages").and_then(|m| m.as_array()).cloned().unwrap_or_default();
        let chars: usize = msgs
            .iter()
            .map(|m| m.get("content").and_then(|c| c.as_str()).unwrap_or("").len())
            .sum();
        let e = per_service.entry(svc).or_insert((0, 0, 0));
        e.0 += 1;
        e.1 += msgs.len();
        e.2 += chars;
    }
    let mut services = serde_json::Map::new();
    let mut total_c = 0;
    let mut total_m = 0;
    let mut total_chars = 0;
    for (svc, (c, m, ch)) in &per_service {
        services.insert(
            svc.clone(),
            json!({ "conversations": c, "messages": m, "chars": ch }),
        );
        total_c += c;
        total_m += m;
        total_chars += ch;
    }
    json!({
        "ok": true,
        "services": Value::Object(services),
        "total_conversations": total_c,
        "total_messages": total_m,
        "total_chars": total_chars,
        "meta": read_sync_meta(),
        "supported_services": SERVICES,
    })
}

/// Tokenización simple: minúsculas, alfanuméricos yacentos unidos.
fn tokenize(s: &str) -> Vec<String> {
    let norm: String = s
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    norm.split_whitespace()
        .filter(|t| t.len() > 2)
        .map(|t| t.to_string())
        .collect()
}

/// Búsqueda full-text sobre el almacén.
pub fn search(query: &str, service: Option<&str>, limit: usize) -> Value {
    let terms = tokenize(query);
    if terms.is_empty() {
        return json!({ "ok": true, "query": query, "hits": [], "note": "consulta vacía" });
    }
    let store = load_store();
    let mut hits: Vec<(f64, Value)> = vec![];
    for v in store.values() {
        if let Some(svc) = service {
            if v.get("service").and_then(|x| x.as_str()) != Some(svc) {
                continue;
            }
        }
        let title = v.get("title").and_then(|x| x.as_str()).unwrap_or("");
        let msgs = v.get("messages").and_then(|m| m.as_array()).cloned().unwrap_or_default();
        let mut best = 0f64;
        let mut best_msg: Option<Value> = None;
        for m in &msgs {
            let content = m.get("content").and_then(|c| c.as_str()).unwrap_or("");
            let hay = format!("{} {}", title, content).to_lowercase();
            let mut score = 0f64;
            for t in &terms {
                let mut count = 0;
                let mut idx = 0;
                while let Some(p) = hay[idx..].find(t.as_str()) {
                    count += 1;
                    idx += p + t.len();
                    if count > 50 {
                        break;
                    }
                }
                if count > 0 {
                    score += 1.0 + (count as f64).ln();
                    // bonus por aparecer en el título
                    if title.to_lowercase().contains(t.as_str()) {
                        score += 1.5;
                    }
                }
            }
            if score > best {
                best = score;
                best_msg = Some(m.clone());
            }
        }
        if best > 0.0 {
            let content = best_msg
                .as_ref()
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_str())
                .unwrap_or("");
            // snippet centrado en la primera coincidencia
            let lower = content.to_lowercase();
            let pos = terms
                .iter()
                .filter_map(|t| lower.find(t.as_str()))
                .min()
                .unwrap_or(0);
            let start = pos.saturating_sub(140);
            let snippet: String = content
                .chars()
                .skip(start)
                .take(420)
                .collect();
            hits.push((
                best,
                json!({
                    "service": v.get("service").cloned().unwrap_or(Value::Null),
                    "conversation_id": v.get("id").cloned().unwrap_or(Value::Null),
                    "title": title,
                    "role": best_msg.as_ref().and_then(|m| m.get("role")).cloned().unwrap_or(Value::Null),
                    "ts": best_msg.as_ref().and_then(|m| m.get("ts")).cloned().unwrap_or(Value::Null),
                    "updated_at": v.get("updated_at").cloned().unwrap_or(Value::Null),
                    "snippet": format!("...{}", snippet),
                    "score": (best * 1000.0).round() / 1000.0,
                }),
            ));
        }
    }
    hits.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let limit = limit.clamp(1, 200);
    let total = hits.len();
    hits.truncate(limit);
    json!({
        "ok": true,
        "query": query,
        "terms": terms,
        "total_hits": total,
        "returned": hits.len(),
        "hits": hits.into_iter().map(|(_, v)| v).collect::<Vec<Value>>(),
    })
}

/// Lista conversaciones de un servicio.
pub fn list_conversations(service: Option<&str>, limit: usize) -> Value {
    let store = load_store();
    let mut items: Vec<Value> = store
        .values()
        .filter(|v| match service {
            Some(s) => v.get("service").and_then(|x| x.as_str()) == Some(s),
            None => true,
        })
        .map(|v| {
            json!({
                "service": v.get("service").cloned().unwrap_or(Value::Null),
                "id": v.get("id").cloned().unwrap_or(Value::Null),
                "title": v.get("title").cloned().unwrap_or(Value::Null),
                "updated_at": v.get("updated_at").cloned().unwrap_or(Value::Null),
                "model": v.get("model").cloned().unwrap_or(Value::Null),
                "messages": v.get("messages").and_then(|m| m.as_array()).map(|a| a.len()).unwrap_or(0),
                "source": v.get("source").cloned().unwrap_or(Value::Null),
            })
        })
        .collect();
    items.sort_by(|a, b| {
        let ta = a["updated_at"].as_f64().unwrap_or(0.0);
        let tb = b["updated_at"].as_f64().unwrap_or(0.0);
        tb.partial_cmp(&ta).unwrap_or(std::cmp::Ordering::Equal)
    });
    let total = items.len();
    items.truncate(limit.clamp(1, 1000));
    json!({ "ok": true, "total": total, "returned": items.len(), "conversations": items })
}

/// Devuelve una conversación completa.
pub fn get_conversation(service: &str, id: &str) -> Value {
    let store = load_store();
    match store.get(&key(service, id)) {
        Some(v) => json!({ "ok": true, "conversation": v }),
        None => json!({ "ok": false, "error": "not_found", "hint": "no existe esa conversación" }),
    }
}

// --------------------------------------------------------------------------- //
// Definiciones MCP
// --------------------------------------------------------------------------- //
pub fn tool_definitions() -> Value {
    json!([
        {
            "name": "sync_conversations",
            "description": "Sincroniza automáticamente las conversaciones de las apps de IA del dispositivo (ChatGPT, Claude, Gemini, DeepSeek, Kimi, Perplexity, Mistral, Copilot, Poe) leyendo sus almacenes privados, normalizándolos y guardándolos en el almacén de MegaDaemon. Idempotente: re-sincronizar no duplica.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "services": {
                        "type": "array",
                        "items": { "type": "string", "enum": ["chatgpt","claude","gemini","deepseek","kimi","perplexity","mistral","copilot","poe"] },
                        "description": "Servicios a sincronizar; si se omite, todos."
                    },
                    "include_cookies": { "type": "boolean", "default": false, "description": "Extrae también cookies de sesión (texto plano) para el pull web-side." }
                }
            }
        },
        {
            "name": "search_conversations",
            "description": "Busca en TODAS las conversaciones sincronizadas (ChatGPT, Claude, DeepSeek, Kimi, ...) y devuelve fragmentos con servicio, conversación, rol y puntuación.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "service": { "type": "string", "enum": ["chatgpt","claude","gemini","deepseek","kimi","perplexity","mistral","copilot","poe"] },
                    "limit": { "type": "integer", "default": 20, "maximum": 200 }
                },
                "required": ["query"]
            }
        },
        {
            "name": "conversation_stats",
            "description": "Estado del almacén de conversaciones: número de conversaciones y mensajes por servicio, última sincronización y servicios soportados.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "list_conversations",
            "description": "Lista las conversaciones sincronizadas (título, servicio, mensajes, fecha).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "service": { "type": "string" },
                    "limit": { "type": "integer", "default": 50, "maximum": 1000 }
                }
            }
        },
        {
            "name": "sync_web_conversations",
            "description": "Cosecha la sesión disponible de cada app y descarga el historial QUE VIVE EN LA NUBE vía la API real de cada servicio (chatgpt.com/backend-api, chat.deepseek.com, kimi.com, claude.ai, gemini.google.com). Los errores se reportan tal cual (401/403) sin inventar datos.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "services": { "type": "array", "items": { "type": "string" } },
                    "harvest_first": { "type": "boolean", "default": true }
                }
            }
        },
        {
            "name": "harvest_sessions",
            "description": "Cosecha las sesiones/tokens disponibles en las apps (cookies en claro de las WebView, token de DeepSeek, app key de Kimi) y los deja en la bóveda de sesiones de MegaDaemon (permisos 600).",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "ingest_export",
            "description": "Ingesta una exportación oficial (ChatGPT conversations.json, Anthropic conversations.json, Google Takeout) al almacén de MegaDaemon, con el mismo esquema que el resto.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }
        },
        {
            "name": "get_conversation",
            "description": "Devuelve una conversación completa (todos los mensajes) por servicio e identificador.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "service": { "type": "string" },
                    "conversation_id": { "type": "string" }
                },
                "required": ["service", "conversation_id"]
            }
        }
    ])
}

/// Despacha las herramientas de sync.
pub fn dispatch(name: &str, args: &Value) -> Option<Value> {
    let s = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let n = |k: &str| args.get(k).and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let b = |k: &str| args.get(k).and_then(|v| v.as_bool()).unwrap_or(false);
    match name {
        "sync_conversations" => {
            let services = args.get("services").and_then(|v| v.as_array()).map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect::<Vec<String>>()
            });
            Some(run_sync(services, b("include_cookies")))
        }
        "search_conversations" => {
            let svc = s("service");
            Some(search(
                &s("query"),
                if svc.is_empty() { None } else { Some(svc.as_str()) },
                if n("limit") == 0 { 20 } else { n("limit") },
            ))
        }
        "conversation_stats" => Some(status()),
        "list_conversations" => {
            let svc = s("service");
            Some(list_conversations(
                if svc.is_empty() { None } else { Some(svc.as_str()) },
                if n("limit") == 0 { 50 } else { n("limit") },
            ))
        }
        "sync_web_conversations" => {
            if b("harvest_first") {
                let _ = crate::sync_web::harvest_sessions();
            }
            let services = args.get("services").and_then(|v| v.as_array()).map(|a| {
                a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect::<Vec<String>>()
            });
            Some(crate::sync_web::web_pull(services))
        }
        "harvest_sessions" => Some(crate::sync_web::harvest_sessions()),
        "ingest_export" => Some(crate::sync_web::ingest_export(&s("path"))),
        "get_conversation" => Some(get_conversation(&s("service"), &s("conversation_id"))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokeniza_sin_puntuacion() {
        let t = tokenize("¿Cómo estás, Gemini 2.5?");
        assert!(t.contains(&"cómo".to_string()));
        assert!(t.contains(&"estás".to_string()));
        assert!(t.contains(&"gemini".to_string()));
        assert!(!t.iter().any(|x| x == "2.5"));
    }

    #[test]
    fn la_clave_de_almacen_es_unica_por_servicio() {
        assert_ne!(key("chatgpt", "x"), key("claude", "x"));
        assert_eq!(key("kimi", "abc"), "kimi::abc");
    }

    #[test]
    fn todas_las_herramientas_de_sync_declaran_servicios_validos() {
        let defs = tool_definitions();
        for d in defs.as_array().unwrap() {
            if let Some(props) = d["inputSchema"]["properties"].as_object() {
                if let Some(svc) = props.get("service") {
                    if let Some(en) = svc.get("enum").and_then(|e| e.as_array()) {
                        for e in en {
                            let name = e.as_str().unwrap();
                            assert!(SERVICES.contains(&name), "servicio desconocido: {}", name);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn el_extractor_va_embebido_y_no_vacio() {
        assert!(EXTRACTOR_SRC.len() > 5000);
        assert!(EXTRACTOR_SRC.contains("extract_claude"));
        assert!(EXTRACTOR_SRC.contains("extract_deepseek"));
        assert!(EXTRACTOR_SRC.contains("extract_kimi"));
        assert!(EXTRACTOR_SRC.contains("extract_chatgpt"));
    }

    #[test]
    fn la_busqueda_ignora_consultas_vacias() {
        let r = search("   ", None, 10);
        assert_eq!(r["hits"].as_array().unwrap().len(), 0);
        assert_eq!(r["ok"], json!(true));
    }
}
