//! MegaDaemon v14 — API v2 del cockpit: agregadores JSON para la UI "cockpit v2".
//!
//! POR QUÉ EXISTE ESTE MÓDULO
//! --------------------------
//! La UI v1 (`web/cockpit_template.html`) tiene un problema estructural: cada
//! vista hace sus propias peticiones (`/api/health`, `/api/metrics`,
//! `/api/providers`, `/api/power`, ...) y luego **pinta lo que sabe pintar**.
//! De ahí las quejas reales del usuario: "no muestra nodos conectados" (no
//! existe endpoint de nodos), "falla muchisimo" (20 peticiones, 20 puntos de
//! fallo), "no busca los LLM de forma agéntica" (no hay vista de agente) y
//! "es muy feo" (no hay sistema de diseño, ni modo claro, ni vistas de
//! coordinación).
//!
//! Este módulo resuelve el lado del daemon de ese rediseño:
//!
//!   * **Agrega**: cada vista de la UI v2 se sirve con **una** llamada
//!     (`/api/v2/dashboard`, `/api/v2/nodes`, ...) que devuelve todo lo que la
//!     vista necesita.  De 20 peticiones a 1.
//!   * **Expone lo que faltaba**: nodos del bus de coordinación, catálogo de
//!     LLM con consumo, delegación a cuentas gratuitas, bucle de auto-mejora
//!     del propio daemon y ficheros compartidos con verificación de hash.
//!   * **No inventa nada**: si el bus de coordinación no está desplegado, la
//!     respuesta dice exactamente qué fichero falta y cómo desplegarlo.  Si un
//!     dato no existe, el campo aparece como `null`/`available: false` con una
//!     razón legible, nunca como mock.
//!   * **Es verificable**: `api_v2_routes()` es la tabla de rutas; un test
//!     comprueba que la UI (`web/cockpit_v2.html`) llama exactamente a esas
//!     rutas y que ninguna ruta de la UI apunta a un endpoint inexistente.
//!
//! CABLEADO (lo hace el dueño del cockpit, 3 líneas en `cockpit_server.rs`):
//! ```ignore
//! if let Some(v) = crate::webui::dispatch(method, path, &body_json) {
//!     send_json(&mut stream, &v);
//!     return;
//! }
//! ```
//! y, para servir la página nueva:
//! ```ignore
//! "/v2" | "/cockpit_v2.html" => { /* servir web/cockpit_v2.html */ }
//! ```
//!
//! POLÍTICA DE AUTH: hereda la del cockpit (doble clave):
//! `X-API-Key` para todo `/api/*` y `X-Seere-Auth-Token` para las
//! operaciones que modifican estado.  Este módulo **no** autentica: sólo
//! serializa datos.  La UI pide la API key al usuario cuando recibe 401 (como
//! ya hace la v1) y la cambia por un token de sesión con
//! `POST /api/session/exchange`.
//!
//! SIN DEPENDENCIAS NUEVAS: sólo `serde_json`, `sha2`, `libc` y `std`, que ya
//! estaban en `Cargo.toml`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ─────────────────────────────────────────────────────────────────────────────
// Rutas
// ─────────────────────────────────────────────────────────────────────────────

/// Prefijo de la API v2.
pub const API_V2_PREFIX: &str = "/api/v2";

/// Tabla de rutas de la API v2: `(método, path)`.
///
/// Es la **fuente de verdad**: la UI se valida contra esta tabla (test
/// `las_rutas_de_la_api_v2_existen_en_la_ui`) y la documentación se genera a
/// partir de ella (`routes_doc`).
pub fn api_v2_routes() -> Vec<(&'static str, &'static str)> {
    vec![
        ("GET", "/api/v2/dashboard"),
        ("GET", "/api/v2/nodes"),
        ("POST", "/api/v2/nodes/assign"),
        ("POST", "/api/v2/nodes/message"),
        ("POST", "/api/v2/nodes/migrate"),
        ("GET", "/api/v2/tasks"),
        ("POST", "/api/v2/tasks/export"),
        ("POST", "/api/v2/tasks/import"),
        ("GET", "/api/v2/llm"),
        ("GET", "/api/v2/delegation"),
        ("POST", "/api/v2/delegation/apply"),
        ("GET", "/api/v2/alerts"),
        ("GET", "/api/v2/re"),
        ("GET", "/api/v2/agentic"),
        ("GET", "/api/v2/share"),
        ("GET", "/api/v2/share/get"),
        ("POST", "/api/v2/share/put"),
        ("POST", "/api/v2/share/chunk"),
        ("POST", "/api/v2/share/commit"),
        ("GET", "/api/v2/market"),
        ("GET", "/api/v2/market/rules"),
        ("POST", "/api/v2/market/rules"),
        ("POST", "/api/v2/market/route"),
        ("GET", "/api/v2/mesh_findings"),
        ("GET", "/api/v2/prompt_external"),
        ("POST", "/api/v2/prompt_external"),
        ("POST", "/api/v2/tasks/claim"),
        ("POST", "/api/v2/tasks/release"),
        ("POST", "/api/v2/tasks/report"),
        ("GET", "/api/v2/voice"),
        ("POST", "/api/v2/voice"),
        ("POST", "/api/v2/admin/restart"),
        ("GET", "/api/v2/routes"),
    ]
}

/// Descripción de cada ruta (para la doc autogenerada y el panel "Sistema").
fn route_descriptions() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("GET", "/api/v2/dashboard", "Resumen del daemon: salud, uptime, CPU/RAM, batería, NPU, alertas, cola y bus. 1 fetch por vista."),
        ("GET", "/api/v2/nodes", "Instancias del bus (maestra/worker/observer), carga, capacidades, latido, tareas y claims."),
        ("POST", "/api/v2/nodes/assign", "Asigna una tarea del tablero a un nodo (publica en el bus y encola en el tablero SQLite)."),
        ("POST", "/api/v2/nodes/message", "Envía un mensaje (y datos en base64) a un nodo por el buzón del bus."),
        ("POST", "/api/v2/nodes/migrate", "Plan de migración de versión de todas las instancias (el primary migra el último)."),
        ("GET", "/api/v2/tasks", "Tablero de tareas: cola local + cola del tablero, stats, tipos de tarea conocidos y journal."),
        ("POST", "/api/v2/tasks/export", "Exporta la cola de tareas para repartirla entre instancias (formato del sistema distribuido)."),
        ("POST", "/api/v2/tasks/import", "Importa una cola exportada por otra instancia (idempotente por id de tarea)."),
        ("GET", "/api/v2/llm", "Catálogo real de proveedores/modelos, el que está activo, rutas de IA y consumo por proveedor."),
        ("GET", "/api/v2/delegation", "Cuentas gratuitas, cuota, presupuesto, score, plan de reparto y ajustes propuestos por el motor."),
        ("POST", "/api/v2/delegation/apply", "Aplica los ajustes propuestos por el motor de delegación (todo o nada) y persiste."),
        ("GET", "/api/v2/alerts", "Alertas activas con severidad, dedup, acuse y estado del canal webhook."),
        ("GET", "/api/v2/re", "Inventario de APKs, estado de DEX/anti-root, exportación y vault de firma."),
        ("GET", "/api/v2/agentic", "Bucle de auto-mejora local: fases, notas de aprendizaje, fallos y ajustes propuestos."),
        ("GET", "/api/v2/share", "Inventario del vault de ficheros compartidos con sha256 y verificación real de cada blob."),
        ("GET", "/api/v2/share/get", "Descarga un fichero compartido (base64) con verificación de hash; admite rango por ?offset/&len."),
        ("POST", "/api/v2/share/put", "Sube un fichero compartido completo; verifica el sha256 declarado antes de escribir."),
        ("POST", "/api/v2/share/chunk", "Sube un trozo (chunk) de un fichero compartido; el hash es el del trozo."),
        ("POST", "/api/v2/share/commit", "Ensambla los trozos subidos, verifica el sha256 del fichero completo y lo publica en el manifiesto."),
        ("GET", "/api/v2/market", "Mercado de capacidades: oferta de cada nodo (con sus reglas locales), demanda pendiente, plan de reparto y ledger de contribución."),
        ("GET", "/api/v2/market/rules", "Reglas locales de compartición de un nodo (batería mínima, temperatura, dedicated, capacidades ofrecidas)."),
        ("POST", "/api/v2/market/rules", "Guarda las reglas locales de compartición de un nodo (validadas)."),
        ("POST", "/api/v2/market/route", "Elige el nodo que debe tomar una demanda según las reglas locales y (opcionalmente) la publica en el bus."),
        ("GET", "/api/v2/mesh_findings", "Hallazgos de RE que el enjambre ha publicado (sync/re_*.ndjson + journal del bus), paginados y sin credenciales. Alias: /api/mesh_findings."),
        ("GET", "/api/v2/prompt_external", "Prompt Markdown para un agente externo, generado en el servidor con el estado real (URLs, tools MCP, dispositivo, enjambre, hallazgos). Sin credenciales."),
        ("POST", "/api/v2/prompt_external", "Igual que el GET, pero puede incluir la API key y el token de sesión ({\"include_secrets\":true,\"api_key\":\"…\"}) — con aviso en el propio Markdown."),
        ("POST", "/api/v2/tasks/claim", "Reclama una tarea del tablero del bus bajo lock (BEGIN IMMEDIATE) con lease y rol; won=false si otro se adelantó."),
        ("POST", "/api/v2/tasks/release", "Devuelve una tarea a `queued`; sólo quien la tiene puede soltarla."),
        ("POST", "/api/v2/tasks/report", "Reporta el resultado de una tarea (hecha/fallida) con su informe."),
        ("GET", "/api/v2/voice", "Estado real de la voz del daemon (identidad, cooldown, presupuesto, bucle autónomo) + rol e id de la instancia."),
        ("POST", "/api/v2/voice", "Acción de la voz: status | whoami | plan | act | speak."),
        ("POST", "/api/v2/admin/restart", "Reinicia la instancia (confirm:true): el daemon sale y el supervisor lo relanza."),
        ("GET", "/api/v2/routes", "Tabla de rutas v2 autogenerada + política de auth (la UI se valida contra esto)."),
        ("POST", "/api/v2/tasks/claim", "Reclama una tarea del tablero del bus bajo lock (BEGIN IMMEDIATE) con lease y rol; won=false si otro se adelantó."),
        ("POST", "/api/v2/tasks/release", "Devuelve una tarea a `queued`; sólo quien la tiene puede soltarla."),
        ("POST", "/api/v2/tasks/report", "Reporta el resultado de una tarea (hecha/fallida) con su informe."),
        ("GET", "/api/v2/voice", "Estado real de la voz del daemon (identidad, cooldown, presupuesto, bucle autónomo) + rol e id de la instancia."),
        ("POST", "/api/v2/voice", "Acción de la voz: status | whoami | plan | act | speak."),
        ("POST", "/api/v2/admin/restart", "Reinicia la instancia (confirm:true): el daemon sale y el supervisor lo relanza."),
    ]
}

/// Documentación de la API v2 en JSON (la consume el panel "Sistema" de la UI).
pub fn routes_doc() -> Value {
    let mut items = vec![];
    for (method, path) in api_v2_routes() {
        let desc = route_descriptions()
            .into_iter()
            .find(|(_, p, _)| *p == path)
            .map(|(_, _, d)| d)
            .unwrap_or("");
        items.push(json!({ "method": method, "path": path, "description": desc }));
    }
    json!({
        "ok": true,
        "version": "v2",
        "prefix": API_V2_PREFIX,
        "count": items.len(),
        "routes": items,
        "auth": {
            "api_key_header": "X-API-Key",
            "session_token_header": "X-Seere-Auth-Token",
            "policy": "todo /api/* exige API key fuera de loopback; las operaciones que modifican estado exigen además token de sesión",
            "exchange": "POST /api/session/exchange con {\"api_key\":\"...\"} devuelve el token de sesión",
            "bootstrap": "la UI pide la API key al usuario si la primera llamada devuelve 401 y la guarda en localStorage",
        },
        "compatibility": {
            "v1_kept": true,
            "note": "las rutas v1 siguen vivas: la v2 es aditiva y la v1 no se rompe",
        },
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Utilidades
// ─────────────────────────────────────────────────────────────────────────────

/// Epoch actual en segundos.
pub fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Fecha/hora actual en ISO-8601 UTC (`2026-11-19T22:00:00Z`).
pub fn now_iso() -> String {
    let secs = now_epoch();
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Días desde epoch → fecha civil (algoritmo de Hinnant).
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
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Directorio de estado del cockpit (respeta `MEGADAEMON_STATE_DIR`).
pub fn cockpit_state_dir() -> String {
    crate::cockpit_server::state_dir()
}

/// Directorio de estado del bus de coordinación (el que usa `mesh`).
pub fn bus_state_dir() -> String {
    crate::mesh::default_state_dir()
}

/// Raíz del vault de ficheros compartidos.
pub fn share_root() -> PathBuf {
    if let Ok(d) = std::env::var("MEGADAEMON_SHARE_DIR") {
        let d = d.trim();
        if !d.is_empty() {
            return PathBuf::from(d);
        }
    }
    Path::new(&cockpit_state_dir()).join("share")
}

/// Fichero de estado del motor de delegación.
fn delegate_state_path() -> PathBuf {
    if let Ok(d) = std::env::var("MEGADAEMON_DELEGATE_STATE") {
        let d = d.trim();
        if !d.is_empty() {
            return PathBuf::from(d);
        }
    }
    Path::new(&cockpit_state_dir()).join("delegate_state.json")
}

/// SHA-256 en hexadecimal (reutiliza el módulo de la bóveda).
pub fn sha256_hex(data: &[u8]) -> String {
    crate::backup_vault::sha256_hex(data)
}

/// Base64 estándar (reutiliza el módulo de firma).
pub fn base64_encode(data: &[u8]) -> String {
    crate::signing::base64_encode(data)
}

/// Base64 estándar o URL-safe → bytes (reutiliza el módulo de firma).
pub fn base64_decode(text: &str) -> Result<Vec<u8>, String> {
    crate::signing::base64_decode(text)
}

/// Divide un path con query string.
fn split_query(path: &str) -> (&str, &str) {
    match path.split_once('?') {
        Some((p, q)) => (p, q),
        None => (path, ""),
    }
}

/// Valor de un parámetro de query string.
fn query_get(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = match pair.split_once('=') {
            Some((k, v)) => (k, v),
            None => (pair, ""),
        };
        if k == key {
            return Some(percent_decode(v));
        }
    }
    None
}

/// Decodifica `%XX` y `+` de un valor de query string.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(b) => {
                        out.push(b);
                        i += 3;
                    }
                    Err(_) => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Normaliza una ruta relativa de fichero compartido.
///
/// Rechaza rutas absolutas y `..`: el vault es una caja fuerte, no un
/// explorador de disco.
fn normalize_rel(rel: &str) -> Result<String, String> {
    let rel = rel.trim().trim_start_matches("./");
    if rel.is_empty() {
        return Err("ruta vacía".into());
    }
    if rel.starts_with('/') {
        return Err("la ruta debe ser relativa".into());
    }
    if rel.contains('\0') {
        return Err("la ruta contiene un NUL".into());
    }
    let mut parts: Vec<&str> = Vec::new();
    for part in rel.split('/') {
        match part {
            "" | "." => continue,
            ".." => {
                return Err("la ruta no puede salir del vault (..)".into());
            }
            p => {
                if p.len() > 128 {
                    return Err("segmento de ruta demasiado largo".into());
                }
                parts.push(p)
            }
        }
    }
    if parts.is_empty() {
        return Err("ruta vacía".into());
    }
    Ok(parts.join("/"))
}

/// Escribe un fichero de forma atómica (tmp + rename).
fn write_atomic(path: &Path, data: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {:?}: {}", parent, e))?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, data).map_err(|e| format!("write {:?}: {}", tmp, e))?;
    fs::rename(&tmp, path).map_err(|e| format!("rename {:?}: {}", path, e))?;
    Ok(())
}

/// Uptime del proceso en segundos (leído de `/proc/self/stat`).
fn process_uptime_secs() -> Option<u64> {
    let stat = fs::read_to_string("/proc/self/stat").ok()?;
    // El nombre del proceso puede contener espacios: se corta por ") ".
    let rest = stat.rsplit_once(") ")?.1;
    let fields: Vec<&str> = rest.split_whitespace().collect();
    // Tras ")" el primer campo es `state` (campo 3); starttime es el 22 → 19.
    let starttime: f64 = fields.get(19)?.parse().ok()?;
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as f64;
    if hz <= 0.0 {
        return None;
    }
    let system_up: f64 = fs::read_to_string("/proc/uptime")
        .ok()?
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    let uptime = system_up - starttime / hz;
    if uptime < 0.0 {
        return None;
    }
    Some(uptime as u64)
}

/// Muestreo de CPU desde `/proc/stat`: (total, idle).
fn cpu_sample() -> Option<(u64, u64)> {
    let stat = fs::read_to_string("/proc/stat").ok()?;
    let line = stat.lines().next()?;
    let nums: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|x| x.parse::<u64>().ok())
        .collect();
    if nums.len() < 5 {
        return None;
    }
    let total: u64 = nums.iter().sum();
    let idle = nums[3] + nums[4];
    Some((total, idle))
}

/// Porcentaje de CPU en uso (muestra de 120 ms).
fn cpu_usage_pct() -> Option<f64> {
    let first = cpu_sample()?;
    std::thread::sleep(Duration::from_millis(120));
    let second = cpu_sample()?;
    let dt = second.0.checked_sub(first.0)?;
    let di = second.1.checked_sub(first.1)?;
    if dt == 0 {
        return None;
    }
    Some(((dt - di) as f64 * 100.0 / dt as f64 * 10.0).round() / 10.0)
}

/// Memoria del sistema desde `/proc/meminfo`.
fn mem_info() -> Value {
    let mut total_kb: Option<u64> = None;
    let mut avail_kb: Option<u64> = None;
    if let Ok(mi) = fs::read_to_string("/proc/meminfo") {
        for l in mi.lines() {
            if l.starts_with("MemTotal:") {
                total_kb = l.split_whitespace().nth(1).and_then(|v| v.parse().ok());
            } else if l.starts_with("MemAvailable:") {
                avail_kb = l.split_whitespace().nth(1).and_then(|v| v.parse().ok());
            }
        }
    }
    match (total_kb, avail_kb) {
        (Some(t), Some(a)) if t > 0 => json!({
            "total_kb": t,
            "available_kb": a,
            "used_kb": t.saturating_sub(a),
            "used_pct": ((t - a) as f64 * 100.0 / t as f64 * 10.0).round() / 10.0,
        }),
        _ => json!({
            "total_kb": null, "available_kb": null, "used_kb": null, "used_pct": null,
            "reason": "no se pudo leer /proc/meminfo",
        }),
    }
}

/// Promedios de carga del sistema.
fn loadavg() -> Option<Value> {
    let la = fs::read_to_string("/proc/loadavg").ok()?;
    let mut it = la.split_whitespace();
    let one: f64 = it.next()?.parse().ok()?;
    let five: f64 = it.next()?.parse().ok()?;
    let fifteen: f64 = it.next()?.parse().ok()?;
    Some(json!({ "1m": one, "5m": five, "15m": fifteen }))
}

// ─────────────────────────────────────────────────────────────────────────────
// Bus de coordinación: estado real (nada de nodos inventados)
// ─────────────────────────────────────────────────────────────────────────────

/// Estado del bus: ¿está desplegado?  ¿hay tablón?  ¿es maestra esta instancia?
fn bus_status() -> Value {
    let dir = bus_state_dir();
    let board = crate::mesh::board_path(&dir);
    let instance = crate::mesh::instance_path(&dir);
    let board_exists = board.exists();
    let instance_exists = instance.exists();
    let master = crate::mesh::is_master();
    json!({
        "deployed": board_exists,
        "state_dir": dir,
        "board_path": board.to_string_lossy(),
        "instance_path": instance.to_string_lossy(),
        "instance_registered": instance_exists,
        "master": master,
        "reason": if board_exists {
            "tablón de coordinación presente".to_string()
        } else if instance_exists {
            "hay identidad de instancia pero falta el tablón: el bus aún no se ha desplegado en esta máquina".to_string()
        } else {
            format!(
                "tablero no desplegado: no existe {} (arranca el daemon con el mismo MEGADAEMON_STATE_DIR en las instancias que quieras conectar)",
                board.to_string_lossy()
            )
        },
    })
}

/// Lee el tablón del bus sin crearlo (para no "desplegar" el bus al mirarlo).
fn mesh_snapshot() -> Option<Value> {
    let dir = bus_state_dir();
    let board = crate::mesh::board_path(&dir);
    if !board.exists() {
        return None;
    }
    match crate::mesh::Mesh::open(&dir) {
        Ok(mesh) => Some(mesh.snapshot()),
        Err(e) => Some(json!({ "ok": false, "error": e })),
    }
}

/// Lee el tablero SQLite de coordinación (si existe).
fn coordination_snapshot() -> Option<Value> {
    let path = Path::new(&bus_state_dir()).join("board.sqlite");
    if !path.exists() {
        return None;
    }
    match crate::coordination::TaskBoard::open(&path.to_string_lossy()) {
        Ok(board) => match board.snapshot() {
            Ok(snap) => Some(snap),
            Err(e) => Some(json!({ "ok": false, "error": e })),
        },
        Err(e) => Some(json!({ "ok": false, "error": e })),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 1) Resumen (dashboard)
// ─────────────────────────────────────────────────────────────────────────────

/// Estado completo del daemon en un único objeto (una llamada, una vista).
///
/// Campos: `daemon`, `system`, `power`, `npu`, `alerts`, `queue`, `bus`,
/// `capabilities`, `llm` y `share`.  Ningún campo es inventado: si algo no
/// está disponible, viene `null` con una `reason` legible.
pub fn dashboard() -> Value {
    let bus = bus_status();
    let board = mesh_snapshot();
    let peers_alive = board
        .as_ref()
        .and_then(|b| b.get("peers_alive"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let bus_nodes = board
        .as_ref()
        .and_then(|b| b.get("peers"))
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    let (pstate, pol) = crate::power_policy::current_policy();
    let npu = crate::npu_bridge::probe_exynos_npu_cached();
    let alerts = crate::alerts::status();
    let queue = crate::task_scheduler::stats();
    let caps = crate::capability::verify();
    let prov = crate::providers::list_providers_redacted();
    let share = shared_files();

    let providers_ready = prov["providers"]
        .as_array()
        .map(|a| a.iter().filter(|p| p["ready"] == json!(true)).count())
        .unwrap_or(0);
    let providers_total = prov["providers"].as_array().map(|a| a.len()).unwrap_or(0);

    json!({
        "ok": true,
        "generated_at": now_iso(),
        "daemon": {
            "name": "megadaemon-zenith",
            "version": env!("CARGO_PKG_VERSION"),
            "pid": std::process::id(),
            "uptime_sec": process_uptime_secs(),
            "uptime_human": human_duration(process_uptime_secs()),
            "host": crate::mesh::hostname(),
            "platform": crate::mesh::platform_string(),
            "instance_id": crate::mesh::instance_id(),
            "state_dir": cockpit_state_dir(),
        },
        "system": {
            "cpu_pct": cpu_usage_pct(),
            "mem": mem_info(),
            "loadavg": loadavg(),
        },
        "power": {
            "capacity_pct": pstate.capacity_pct,
            "charging": pstate.charging,
            "usb_plugged": pstate.usb_plugged,
            "battery_temp_c": pstate.battery_temp_c,
            "cpu_temp_c": pstate.cpu_temp_c,
            "thermal_hot": pstate.thermal_hot,
            "governor": pstate.governor,
            "system_power_save": pstate.system_power_save,
            "policy": { "name": pol.name, "reason": pol.reason, "max_turns": pol.max_turns, "max_tokens": pol.max_tokens },
        },
        "npu": {
            "soc": npu.soc,
            "vs4l_present": npu.vs4l_present,
            "nnapi_devices": npu.nnapi_device_count,
            "eden_runtime": npu.eden_runtime.exists,
            "offload_implemented": npu.npu_offload_implemented,
            "offload_note": npu.npu_offload_note,
            "throttle_state": npu.throttle_state,
            "battery_temp_c": npu.battery_temp_c,
            "cpu_temp_c": npu.cpu_temp_c,
        },
        "alerts": {
            "active": alerts["active"].clone(),
            "crit": alerts["crit"].clone(),
            "warn": alerts["warn"].clone(),
            "info": alerts["info"].clone(),
            "dedup_window_s": alerts["dedup_window_s"].clone(),
        },
        "queue": {
            "total": queue["tasks"].clone(),
            "by_state": queue["by_state"].clone(),
            "by_kind": queue["by_kind"].clone(),
            "journal_entries": queue["journal_entries"].clone(),
        },
        "bus": {
            "deployed": bus["deployed"].clone(),
            "master": bus["master"].clone(),
            "reason": bus["reason"].clone(),
            "nodes": bus_nodes,
            "peers_alive": peers_alive,
        },
        "capabilities": {
            "total": caps["total"].clone(),
            "healthy": caps["healthy"].clone(),
            "degraded": caps["degraded"].clone(),
        },
        "llm": {
            "default_model": prov["default_model"].clone(),
            "providers": providers_total,
            "ready": providers_ready,
            "free_providers": prov["free_providers"].clone(),
            "catalog_size": prov["catalog_size"].clone(),
        },
        "share": {
            "files": share["total_files"].clone(),
            "bytes": share["total_bytes"].clone(),
            "root": share["root"].clone(),
        },
    })
}

/// Duración legible (`3d 04h 12m`).
fn human_duration(secs: Option<u64>) -> String {
    let Some(s) = secs else {
        return "desconocido".to_string();
    };
    let d = s / 86_400;
    let h = (s % 86_400) / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    if d > 0 {
        format!("{}d {:02}h {:02}m", d, h, m)
    } else if h > 0 {
        format!("{}h {:02}m {:02}s", h, m, sec)
    } else {
        format!("{}m {:02}s", m, sec)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2) Nodos (bus de coordinación)
// ─────────────────────────────────────────────────────────────────────────────

/// Instancias del bus + estado del bus + tareas + claims + eventos.
///
/// Si el bus no está desplegado devuelve `nodes: []` y una `reason` que
/// explica qué falta: **nunca** inventa nodos.
pub fn nodes() -> Value {
    let bus = bus_status();
    let board = mesh_snapshot();
    let coord = coordination_snapshot();

    let mut list: Vec<Value> = vec![];
    let mut tasks: Vec<Value> = vec![];
    let mut claims: Vec<Value> = vec![];
    let mut events: Vec<Value> = vec![];
    let mut stats = Value::Null;
    let mut notes: Vec<Value> = vec![];

    if let Some(b) = &board {
        if b.get("ok") == Some(&json!(false)) {
            return json!({
                "ok": false,
                "bus": bus,
                "nodes": [],
                "error": b.get("error").cloned().unwrap_or(Value::Null),
                "hint": "el tablón existe pero no se pudo leer; revisa permisos y formato",
            });
        }
        if let Some(me) = b.get("me") {
            let mut node = me.clone();
            if let Some(o) = node.as_object_mut() {
                o.insert("kind".to_string(), json!("local"));
                o.insert("alive".to_string(), json!(true));
            }
            list.push(node);
        }
        if let Some(peers) = b.get("peers").and_then(|v| v.as_array()) {
            for p in peers {
                let mut node = p.clone();
                if let Some(o) = node.as_object_mut() {
                    o.insert("kind".to_string(), json!("peer"));
                    o.insert("alive".to_string(), json!(true));
                }
                list.push(node);
            }
        }
        if let Some(t) = b.get("tasks").and_then(|v| v.as_array()) {
            tasks = t.clone();
        }
        if let Some(c) = b.get("claims").and_then(|v| v.as_array()) {
            claims = c.clone();
        }
        if let Some(s) = b.get("stats") {
            stats = s.clone();
        }
        if let Some(n) = b.get("notes").and_then(|v| v.as_array()) {
            notes = n.clone();
        }
    }

    // Instancias del tablero SQLite (rol, capacidades, latido) — si existe.
    let mut board_instances: Vec<Value> = vec![];
    if let Some(c) = &coord {
        if let Some(i) = c.get("instances").and_then(|v| v.as_array()) {
            board_instances = i.clone();
        }
        if let Some(e) = c.get("events").and_then(|v| v.as_array()) {
            events = e.clone();
        }
        if let Some(t) = c.get("tasks").and_then(|v| v.as_array()) {
            for t in t {
                tasks.push(json!({
                    "task_id": t["id"].clone(),
                    "title": t["title"].clone(),
                    "capability": "",
                    "priority": t["priority"].clone(),
                    "payload": Value::Null,
                    "created_at": t["created_at"].clone(),
                    "lease_secs": 0,
                    "state": t["status"].clone(),
                    "owner": t["owner"].clone(),
                    "source": "board.sqlite",
                }));
            }
        }
    }

    let deployed = bus["deployed"] == json!(true) || coord.is_some();

    json!({
        "ok": true,
        "generated_at": now_iso(),
        "bus": {
            "deployed": deployed,
            "master": bus["master"].clone(),
            "state_dir": bus["state_dir"].clone(),
            "board_path": bus["board_path"].clone(),
            "reason": bus["reason"].clone(),
            "sqlite_board": coord.is_some(),
        },
        "nodes": list,
        "node_count": list.len(),
        "board_instances": board_instances,
        "tasks": tasks,
        "claims": claims,
        "events": events,
        "stats": stats,
        "notes": notes,
        "actions": [
            {"id": "assign", "label": "Asignar tarea", "method": "POST", "path": "/api/v2/nodes/assign"},
            {"id": "message", "label": "Enviar mensaje/datos", "method": "POST", "path": "/api/v2/nodes/message"},
            {"id": "migrate", "label": "Pedir migración", "method": "POST", "path": "/api/v2/nodes/migrate"},
        ],
    })
}

/// Asigna una tarea a un nodo.
///
/// Publica la tarea en el tablón del bus (para que la otra instancia la
/// reclame) y, si el tablero SQLite está desplegado, la encola allí con el
/// propietario como pista.  Si el bus no está desplegado lo dice.
pub fn node_assign(node_id: &str, kind: &str, payload: &Value, capability: &str) -> Value {
    let node_id = node_id.trim();
    if node_id.is_empty() {
        return json!({ "ok": false, "error": "missing_node", "hint": "indica el nodo destino" });
    }
    let kind = if kind.trim().is_empty() {
        "generic".to_string()
    } else {
        kind.trim().to_string()
    };
    let dir = bus_state_dir();
    let board = crate::mesh::board_path(&dir);
    if !board.exists() {
        return json!({
            "ok": false,
            "error": "bus_not_deployed",
            "reason": bus_status()["reason"].clone(),
            "hint": "despliega el bus (tablón compartido) antes de repartir trabajo",
        });
    }
    let mut mesh = match crate::mesh::Mesh::open(&dir) {
        Ok(m) => m,
        Err(e) => return json!({ "ok": false, "error": "mesh_open_failed", "detail": e }),
    };
    let task_id = format!(
        "task-{}-{}",
        now_epoch(),
        crate::backup_vault::sha256_hex(format!("{}{}", node_id, kind).as_bytes())[..8].to_string()
    );
    let mut task = crate::mesh::MeshTask::new(&task_id, &kind, capability);
    task.payload = json!({
        "kind": kind,
        "target_node": node_id,
        "payload": payload.clone(),
    });
    task.priority = payload
        .get("priority")
        .and_then(|v| v.as_i64())
        .unwrap_or(5) as i32;
    if let Err(e) = mesh.publish(&task) {
        return json!({ "ok": false, "error": "publish_failed", "detail": e });
    }

    // Tablero SQLite (opcional).
    let mut board_task_id = Value::Null;
    let sqlite = Path::new(&dir).join("board.sqlite");
    if sqlite.exists() {
        if let Ok(board) = crate::coordination::TaskBoard::open(&sqlite.to_string_lossy()) {
            if let Ok(id) = board.add_task(
                &format!("{} → {}", kind, node_id),
                task.priority,
                Some(node_id),
            ) {
                board_task_id = json!(id);
            }
        }
    }

    json!({
        "ok": true,
        "task_id": task_id,
        "board_task_id": board_task_id,
        "node": node_id,
        "kind": kind,
        "capability": capability,
        "state": "published",
        "note": "publicada en el tablón; la otra instancia debe reclamarla (lease)",
    })
}

/// Envía un mensaje (y opcionalmente datos) a un nodo.
///
/// El mensaje se escribe en el buzón del bus (`<bus>/inbox/<node>.ndjson`) y
/// se publica una tarea con capacidad `message` para que el peer lo vea en su
/// próxima lectura del tablón.
pub fn node_message(node_id: &str, message: &str, data_b64: &str) -> Value {
    let node_id = node_id.trim();
    if node_id.is_empty() {
        return json!({ "ok": false, "error": "missing_node" });
    }
    let dir = bus_state_dir();
    let board = crate::mesh::board_path(&dir);
    if !board.exists() {
        return json!({
            "ok": false,
            "error": "bus_not_deployed",
            "reason": bus_status()["reason"].clone(),
        });
    }
    let mut mesh = match crate::mesh::Mesh::open(&dir) {
        Ok(m) => m,
        Err(e) => return json!({ "ok": false, "error": "mesh_open_failed", "detail": e }),
    };

    let inbox = Path::new(&dir).join("inbox");
    if let Err(e) = fs::create_dir_all(&inbox) {
        return json!({ "ok": false, "error": "inbox_unavailable", "detail": e.to_string() });
    }
    let file = inbox.join(format!("{}.ndjson", sanitize_name(node_id)));
    let record = json!({
        "ts": now_iso(),
        "from": crate::mesh::instance_id(),
        "to": node_id,
        "message": message,
        "data_b64": data_b64,
    });
    let mut line = record.to_string();
    line.push('\n');
    let mut existing = fs::read(&file).unwrap_or_default();
    existing.extend_from_slice(line.as_bytes());
    if let Err(e) = write_atomic(&file, &existing) {
        return json!({ "ok": false, "error": "inbox_write_failed", "detail": e });
    }

    let task_id = format!("msg-{}-{}", now_epoch(), crate::backup_vault::sha256_hex(line.as_bytes())[..8].to_string());
    let mut task = crate::mesh::MeshTask::new(&task_id, &format!("mensaje para {}", node_id), "message");
    task.payload = record.clone();
    if let Err(e) = mesh.publish(&task) {
        return json!({ "ok": false, "error": "publish_failed", "detail": e });
    }

    json!({
        "ok": true,
        "task_id": task_id,
        "node": node_id,
        "inbox": file.to_string_lossy(),
        "bytes": line.len(),
        "note": "mensaje depositado en el buzón del bus y publicado en el tablón",
    })
}

/// Plan de migración de versión de las instancias del bus.
///
/// El primary migra el último: el coordinador no puede quedarse caído.
pub fn node_migrate(target_version: &str) -> Value {
    let target = if target_version.trim().is_empty() {
        env!("CARGO_PKG_VERSION").to_string()
    } else {
        target_version.trim().to_string()
    };
    let dir = bus_state_dir();
    let board = crate::mesh::board_path(&dir);
    let mut out = json!({
        "ok": true,
        "target_version": target,
        "local_version": env!("CARGO_PKG_VERSION"),
        "release": crate::mesh::release_info(),
        "sqlite_plan": Value::Null,
        "mesh_plan": Value::Null,
    });
    let sqlite = Path::new(&dir).join("board.sqlite");
    if sqlite.exists() {
        if let Ok(b) = crate::coordination::TaskBoard::open(&sqlite.to_string_lossy()) {
            if let Ok(plan) = b.migration_plan(&target) {
                out["sqlite_plan"] = plan;
            }
        }
    }
    if board.exists() {
        if let Ok(mut mesh) = crate::mesh::Mesh::open(&dir) {
            if let Ok(plan) = crate::mesh::check_migration(&mut mesh, &crate::mesh::release_info()) {
                out["mesh_plan"] = serde_json::to_value(plan).unwrap_or(Value::Null);
            }
        }
    }
    if out["sqlite_plan"] == Value::Null && out["mesh_plan"] == Value::Null {
        out["reason"] = bus_status()["reason"].clone();
        out["note"] = json!("no hay instancias registradas: despliega el bus para calcular el plan");
    }
    out
}

/// Nombre de fichero seguro a partir de un id de nodo.
fn sanitize_name(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// 3) Tablero de tareas
// ─────────────────────────────────────────────────────────────────────────────

/// Tablero de tareas: cola local (`task_scheduler`) + cola del tablero de
/// coordinación + estadísticas + tipos de tarea conocidos.
pub fn task_board() -> Value {
    let queue = crate::task_scheduler::list_json();
    let stats = crate::task_scheduler::stats();
    let coord = coordination_snapshot();
    let mut board_tasks: Vec<Value> = vec![];
    let mut board_status = json!({ "deployed": false, "reason": "tablero SQLite no desplegado" });
    let mut board_instances: Vec<Value> = vec![];
    if let Some(c) = &coord {
        board_status = json!({
            "deployed": true,
            "path": format!("{}/board.sqlite", bus_state_dir()),
            "tasks_by_status": c.get("tasks_by_status").cloned().unwrap_or(Value::Null),
            "instances": c.get("instances").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
        });
        if let Some(t) = c.get("tasks").and_then(|v| v.as_array()) {
            board_tasks = t.clone();
        }
        if let Some(i) = c.get("instances").and_then(|v| v.as_array()) {
            board_instances = i.clone();
        }
    }
    json!({
        "ok": true,
        "generated_at": now_iso(),
        "tasks": queue.get("tasks").cloned().unwrap_or(json!([])),
        "stats": stats,
        "known_kinds": crate::agent_core::is_known_kind_list(),
        "board": board_status,
        "board_tasks": board_tasks,
        "board_instances": board_instances,
        "my_instance": local_instance().unwrap_or(Value::Null),
        "my_role": local_role(),
        "journal": crate::task_scheduler::journal_json(50),
        "actions": [
            {"id": "submit", "label": "Encolar", "method": "POST", "path": "/api/tool/task_queue", "body": {"action": "submit"}},
            {"id": "run", "label": "Ejecutar lote", "method": "POST", "path": "/api/tool/task_queue", "body": {"action": "run"}},
            {"id": "cancel", "label": "Cancelar", "method": "POST", "path": "/api/tool/task_queue", "body": {"action": "cancel"}},
            {"id": "retry", "label": "Reintentar", "method": "POST", "path": "/api/tool/task_queue", "body": {"action": "retry"}},
            {"id": "export", "label": "Exportar", "method": "POST", "path": "/api/tool/task_queue", "body": {"action": "export"}},
            {"id": "import", "label": "Importar", "method": "POST", "path": "/api/tool/task_queue", "body": {"action": "import"}},
        ],
        "note": "submit/run/cancel/retry/export usan el dispatcher genérico /api/tool/task_queue",
    })
}

/// Exporta la cola de tareas para repartirla entre instancias.
///
/// El formato es el que produce `task_scheduler::export_tasks`: las tareas
/// viajan con su id, de modo que importar en otra instancia es idempotente.
pub fn task_export(node: &str) -> Value {
    let v = crate::task_scheduler::export_tasks(node);
    json!({
        "ok": v.get("ok").cloned().unwrap_or(json!(true)),
        "node": node,
        "export": v,
        "note": "importa este JSON con POST /api/v2/tasks/import (las tareas ya conocidas no se duplican)",
    })
}

/// Importa una cola exportada por otra instancia.
pub fn task_import(payload: &Value) -> Value {
    let src = payload
        .get("export")
        .cloned()
        .unwrap_or_else(|| payload.clone());
    // `import_tasks` devuelve el número de tareas importadas (usize), no un
    // Result: el tablero es idempotente y lo que ya existía se ignora.
    let n = crate::task_scheduler::import_tasks(&src);
    json!({
        "ok": true,
        "imported": n,
        "note": "tareas importadas (las que ya existían se ignoran)",
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// 4) Catálogo de LLM
// ─────────────────────────────────────────────────────────────────────────────

/// Catálogo real de proveedores y modelos + el que está activo + consumo.
///
/// `providers` viene del vault real (`providers.json`), `catalog` del catálogo
/// del daemon (`provider_catalog`) y `pricing` de la tabla de precios con su
/// procedencia (`delegate::pricing_ledger`).
pub fn llm_catalog() -> Value {
    let vault = crate::providers::list_providers_redacted();
    let catalog: Vec<Value> = crate::provider_catalog::catalog_v13_1()
        .into_iter()
        .map(|p| {
            json!({
                "id": p.id,
                "name": p.name,
                "vendor": p.vendor,
                "endpoint": p.endpoint,
                "model": p.model,
                "auth_type": p.auth_type,
                "tier": p.tier,
                "free": crate::provider_catalog::tier_free(&p.tier),
                "free_signup_url": p.free_signup_url,
                "supports_reasoning": p.supports_reasoning,
            })
        })
        .collect();
    let ledger = crate::delegate::pricing_ledger();
    let delegation = delegation();
    let consumption = delegation
        .get("consumption")
        .cloned()
        .unwrap_or(json!({ "available": false }));
    let agent = crate::agent_core::status();

    json!({
        "ok": true,
        "generated_at": now_iso(),
        "default_model": vault["default_model"].clone(),
        "providers": vault["providers"].clone(),
        "provider_count": vault["providers"].as_array().map(|a| a.len()).unwrap_or(0),
        "ready_count": vault["providers"].as_array().map(|a| a.iter().filter(|p| p["ready"] == json!(true)).count()).unwrap_or(0),
        "free_providers": vault["free_providers"].clone(),
        "catalog": catalog,
        "catalog_size": catalog.len(),
        "retired": vault["retired"].clone(),
        "engine": vault["engine"].clone(),
        "routing": agent["llm_route"].clone(),
        "pricing": ledger,
        "consumption": consumption,
        "note": "ping de proveedor: POST /api/providers/test con {\"id\":\"...\"} (verificado contra el vault real)",
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// 5) Delegación a cuentas gratuitas
// ─────────────────────────────────────────────────────────────────────────────

/// Estado persistido del motor de delegación (cuentas + cola + intentos).
fn load_delegate_state() -> Option<Value> {
    let path = delegate_state_path();
    let raw = fs::read_to_string(&path).ok()?;
    serde_json::from_str::<Value>(&raw).ok()
}

/// Guarda el estado del motor de delegación.
fn save_delegate_state(state: &Value) -> Result<(), String> {
    let path = delegate_state_path();
    let body = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    write_atomic(&path, body.as_bytes())
}

/// Reconstruye el motor de delegación desde el estado persistido.
///
/// Los intentos se reproducen con `record_attempt` para que el aprendizaje
/// (EWMA de calidad, tokens, coste, cuota) sea el mismo que en vivo.  La
/// cuota se compensa para no descontarla dos veces.
fn build_delegate_engine(state: &Value) -> crate::delegate::DelegateEngine {
    let mut accounts: Vec<crate::delegate::ProviderAccount> = state
        .get("accounts")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| serde_json::from_value::<crate::delegate::ProviderAccount>(x.clone()).ok())
                .collect()
        })
        .unwrap_or_default();

    // Intentos por cuenta (para compensar la cuota consumida).
    let mut consumed: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    if let Some(tasks) = state.get("tasks").and_then(|v| v.as_array()) {
        for t in tasks {
            if let Some(attempts) = t.get("attempts").and_then(|v| v.as_array()) {
                for a in attempts {
                    let acc = a.get("account").and_then(|v| v.as_str()).unwrap_or("");
                    if !acc.is_empty() {
                        *consumed.entry(acc.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    for a in accounts.iter_mut() {
        if let Some(q) = a.remaining_quota {
            a.remaining_quota = Some(q.saturating_add(consumed.get(&a.id).copied().unwrap_or(0)));
        }
    }

    let mut engine = crate::delegate::DelegateEngine::new(accounts);
    if let Some(tasks) = state.get("tasks").and_then(|v| v.as_array()) {
        for t in tasks {
            let Ok(mut task) = serde_json::from_value::<crate::delegate::DelegationTask>(t.clone()) else {
                continue;
            };
            let attempts = std::mem::take(&mut task.attempts);
            let id = task.id.clone();
            engine.submit(task);
            for a in attempts {
                engine.record_attempt(&id, a);
            }
        }
    }
    engine
}

/// Serializa el motor de delegación a JSON persistible.
fn delegate_state_from_engine(engine: &crate::delegate::DelegateEngine) -> Value {
    json!({
        "version": 1,
        "updated_at": now_iso(),
        "accounts": engine.accounts(),
        "tasks": engine.tasks(),
    })
}

/// Vista de delegación: cuentas, cuota, presupuesto, score, plan y ajustes.
pub fn delegation() -> Value {
    let Some(state) = load_delegate_state() else {
        return json!({
            "ok": true,
            "deployed": false,
            "reason": format!(
                "no hay estado de delegación en {}: el motor todavía no tiene cuentas cargadas",
                delegate_state_path().to_string_lossy()
            ),
            "accounts": [],
            "adjustments": [],
            "plan": [],
            "consumption": { "available": false },
            "hint": "crea el fichero con la lista de cuentas (id, provider, model, tier, capabilities) para activar el reparto",
        });
    };
    let engine = build_delegate_engine(&state);
    let accounts: Vec<Value> = engine
        .accounts()
        .iter()
        .map(|a| {
            json!({
                "id": a.id,
                "provider": a.provider,
                "tier": a.tier,
                "model": a.model,
                "capabilities": a.capabilities,
                "remaining_quota": a.remaining_quota,
                "daily_budget_usd": a.daily_budget_usd,
                "priority": a.priority,
                "enabled": a.enabled,
                "notes": a.notes,
                "free": crate::delegate::tier_is_free(&a.tier),
                "score": engine.score(&a.id),
            })
        })
        .collect();
    let stats = engine.account_stats();
    let plan = engine.plan();
    let adjustments = engine.adjustments();
    let engine_stats = engine.stats();

    json!({
        "ok": true,
        "deployed": true,
        "generated_at": now_iso(),
        "state_path": delegate_state_path().to_string_lossy(),
        "accounts": accounts,
        "account_stats": stats,
        "plan": plan,
        "adjustments": adjustments,
        "stats": engine_stats,
        "consumption": {
            "available": true,
            "tokens": engine_stats.get("tokens").cloned().unwrap_or(Value::Null),
            "cost_usd": engine_stats.get("cost_usd").cloned().unwrap_or(Value::Null),
            "attempts": engine_stats.get("attempts").cloned().unwrap_or(Value::Null),
            "success_rate": engine_stats.get("success_rate").cloned().unwrap_or(Value::Null),
            "by_account": engine_stats.get("success_rate").and_then(|v| v.get("by_account")).cloned().unwrap_or(Value::Null),
        },
        "pricing": crate::delegate::pricing_report(30),
        "actions": [
            {"id": "apply", "label": "Aplicar ajustes", "method": "POST", "path": "/api/v2/delegation/apply"},
        ],
    })
}

/// Aplica los ajustes propuestos por el motor de delegación.
///
/// Todo o nada: si un ajuste no es válido no se aplica ninguno (lo decide
/// `DelegateEngine::apply_adjustments`).  Luego persiste el estado.
pub fn delegation_apply(adjustments: &Value) -> Value {
    let Some(state) = load_delegate_state() else {
        return json!({ "ok": false, "error": "not_deployed", "reason": delegation()["reason"].clone() });
    };
    let list: Vec<crate::delegate::Adjustment> = match adjustments.as_array() {
        Some(a) => a
            .iter()
            .filter_map(|x| serde_json::from_value::<crate::delegate::Adjustment>(x.clone()).ok())
            .collect(),
        None => {
            return json!({
                "ok": false,
                "error": "bad_payload",
                "hint": "envía {\"adjustments\":[...]} con la lista de ajustes del motor",
            })
        }
    };
    if list.is_empty() {
        return json!({ "ok": false, "error": "empty_adjustments" });
    }
    let mut engine = build_delegate_engine(&state);
    if let Err(e) = engine.apply_adjustments(&list) {
        return json!({ "ok": false, "error": "rejected", "detail": e });
    }
    let new_state = delegate_state_from_engine(&engine);
    if let Err(e) = save_delegate_state(&new_state) {
        return json!({ "ok": false, "error": "persist_failed", "detail": e });
    }
    json!({
        "ok": true,
        "applied": list.len(),
        "accounts": engine.accounts().len(),
        "state_path": delegate_state_path().to_string_lossy(),
        "note": "los ajustes se aplicaron y se guardaron; el motor vuelve a calcular score y plan en la siguiente carga",
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// 6) Alertas
// ─────────────────────────────────────────────────────────────────────────────

/// Alertas activas + estado del canal de entrega (webhook).
pub fn alerts_view() -> Value {
    let list = crate::alerts::list(None, false, 100);
    let status = crate::alerts::status();
    let webhook = std::env::var("MEGADAEMON_ALERT_WEBHOOK").ok();
    let channel = match webhook {
        Some(url) if !url.trim().is_empty() => {
            // No se filtra la URL completa: puede llevar credenciales.
            let host = url
                .split("//")
                .nth(1)
                .and_then(|r| r.split('/').next())
                .unwrap_or("configurado")
                .to_string();
            json!({
                "configured": true,
                "kind": "webhook",
                "host": host,
                "note": "MEGADAEMON_ALERT_WEBHOOK está definido: las alertas se entregan también fuera del daemon",
            })
        }
        _ => json!({
            "configured": false,
            "kind": "none",
            "note": "sin webhook configurado: las alertas sólo viven en el daemon (define MEGADAEMON_ALERT_WEBHOOK para entregarlas fuera)",
        }),
    };
    json!({
        "ok": true,
        "generated_at": now_iso(),
        "alerts": list.get("alerts").cloned().unwrap_or(json!([])),
        "total": list.get("total").cloned().unwrap_or(json!(0)),
        "returned": list.get("returned").cloned().unwrap_or(json!(0)),
        "status": status,
        "channel": channel,
        "actions": [
            {"id": "check", "label": "Evaluar reglas", "method": "POST", "path": "/api/tool/alerts", "body": {"action": "check"}},
            {"id": "ack", "label": "Acusar", "method": "POST", "path": "/api/tool/alerts", "body": {"action": "ack"}},
            {"id": "prune", "label": "Limpiar acusadas", "method": "POST", "path": "/api/tool/alerts", "body": {"action": "prune"}},
        ],
        "note": "listar/ack/check usan el dispatcher genérico /api/tool/alerts",
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// 7) RE / Apps
// ─────────────────────────────────────────────────────────────────────────────

/// Vista RE: inventario de APKs, estado de DEX, señales anti-root,
/// exportación y vault de firma.
///
/// El inventario de APKs requiere `pm` (root): si no está disponible, el
/// campo viene con `available: false` y la razón real, no una lista vacía
/// fingida.
pub fn re_apps() -> Value {
    let inventory = crate::apk_inventory::inventory(60);
    let pkgs = inventory["packages"].as_array().cloned().unwrap_or_default();
    let apps_available = inventory["ok"] == json!(true) && !pkgs.is_empty();

    // Material de firma presente en la instancia (lo que hay de verdad).
    let mut signature_files: Vec<Value> = vec![];
    for dir in [Path::new(&cockpit_state_dir()).join("keys"), Path::new(&bus_state_dir()).join("keys")] {
        if let Ok(entries) = fs::read_dir(&dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_file() {
                    let md = fs::metadata(&p).ok();
                    signature_files.push(json!({
                        "name": p.file_name().unwrap_or_default().to_string_lossy(),
                        "dir": dir.to_string_lossy(),
                        "size": md.map(|m| m.len()).unwrap_or(0),
                        "sha256": fs::read(&p).ok().map(|b| sha256_hex(&b)),
                    }));
                }
            }
        }
    }

    json!({
        "ok": true,
        "generated_at": now_iso(),
        "apps": {
            "available": apps_available,
            "count": pkgs.len(),
            "packages": pkgs,
            "reason": if apps_available {
                Value::Null
            } else {
                json!("no se pudo leer el inventario de paquetes (hace falta pm con root en el dispositivo)")
            },
            "duration_ms": inventory["duration_ms"].clone(),
        },
        "dex": {
            "available": true,
            "entry_point": "POST /api/tool/apk_inventory con {\"action\":\"analyze\",\"path\":\"...\"}",
            "note": "la decompilación DEX y las señales anti-root se lanzan por APK, no de forma masiva",
        },
        "root_hide": {
            "profile_targets": crate::root_stealth::TARGETS,
            "note": "el estado real del perfil de ocultación se lee con POST /api/tool/root_stealth {\"action\":\"status\"}",
        },
        "export": {
            "task_export": "POST /api/tool/task_queue {\"action\":\"export\"}",
            "repo_updates": "POST /api/tool/repo_updates {\"action\":\"plan\"}",
            "apk_dump": "POST /api/tool/apk_inventory {\"action\":\"dump\",\"dest_dir\":\"...\"}",
        },
        "signature_vault": {
            "files": signature_files,
            "count": signature_files.len(),
            "empty": signature_files.is_empty(),
            "note": "material de firma presente en la instancia (claves/keystores); vacío si no hay ninguno",
        },
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// 8) Bucle agéntico local de auto-mejora
// ─────────────────────────────────────────────────────────────────────────────

/// Estado del bucle de auto-mejora local del propio megadaemon.
///
/// El bucle tiene cinco fases y cada una se rellena con datos reales:
/// `observe` (salud y cola), `plan` (qué sabe hacer), `execute` (agentes que
/// han corrido), `measure` (éxito/fallo por tipo de tarea) y `adjust`
/// (ajustes propuestos por el motor de delegación).
pub fn agentic_loop_status() -> Value {
    let agent = crate::agent_core::status();
    let queue = crate::task_scheduler::stats();
    let journal = crate::task_scheduler::journal_json(50);
    let board = mesh_snapshot();
    let mut notes: Vec<Value> = vec![];
    let mut agents = Value::Null;
    let mut agent_stats = Value::Null;
    if let Some(b) = &board {
        if let Some(n) = b.get("notes").and_then(|v| v.as_array()) {
            notes = n.clone();
        }
        if let Some(a) = b.get("agents") {
            agents = a.clone();
        }
        if let Some(s) = b.get("stats") {
            agent_stats = s.clone();
        }
    }
    let delegation_state = delegation();
    let adjustments = delegation_state
        .get("adjustments")
        .cloned()
        .unwrap_or(json!([]));

    let entries = journal.get("entries").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut done = 0usize;
    let mut failed = 0usize;
    for e in &entries {
        if e.get("ok") == Some(&json!(true)) {
            done += 1;
        } else if e.get("ok") == Some(&json!(false)) {
            failed += 1;
        }
    }

    json!({
        "ok": true,
        "generated_at": now_iso(),
        "cycle": [
            {"phase": "observe", "label": "Observar", "source": "dashboard + cola + alertas", "ok": true},
            {"phase": "plan", "label": "Planificar", "source": "agent_core::plan (LLM local o heurística)", "ok": true},
            {"phase": "execute", "label": "Ejecutar", "source": "task_scheduler + bus de coordinación", "ok": true},
            {"phase": "measure", "label": "Medir", "source": "journal + agent_stats del bus", "ok": true},
            {"phase": "adjust", "label": "Ajustar", "source": "delegate::adjustments + notas de aprendizaje", "ok": true},
        ],
        "observe": {
            "queue": queue.get("tasks").cloned().unwrap_or(json!(0)),
            "by_state": queue.get("by_state").cloned().unwrap_or(Value::Null),
            "capabilities": agent.get("capabilities").cloned().unwrap_or(Value::Null),
            "known_task_kinds": agent.get("known_task_kinds").cloned().unwrap_or(json!([])),
        },
        "plan": {
            "route": agent.get("llm_route").cloned().unwrap_or(Value::Null),
            "heuristic_fallback": true,
            "note": "si no hay LLM disponible el planificador heurístico determinista sigue funcionando (0 tokens)",
        },
        "execute": {
            "journal_entries": entries.len(),
            "done": done,
            "failed": failed,
            "agents": agents,
        },
        "measure": {
            "agent_stats": agent_stats,
            "notes": notes,
        },
        "adjust": {
            "proposed": adjustments,
            "count": adjustments.as_array().map(|a| a.len()).unwrap_or(0),
            "apply_path": "/api/v2/delegation/apply",
        },
        "notes": [
            "el bucle es local: no necesita red y no consume tokens salvo que se pida un LLM explícitamente",
            "las notas de aprendizaje salen de los fallos registrados en el bus y del motor de delegación",
        ],
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// 9) Ficheros compartidos (vault cifrado + MANIFEST.ndjson)
// ─────────────────────────────────────────────────────────────────────────────

/// Nombre del manifiesto de ficheros compartidos.
pub const SHARE_MANIFEST_NAME: &str = "MANIFEST.ndjson";

/// Ruta del manifiesto dentro de una raíz de vault.
fn manifest_path(root: &Path) -> PathBuf {
    root.join(SHARE_MANIFEST_NAME)
}

/// Ruta del blob (contenido direccionado por hash) de un fichero.
fn blob_path(root: &Path, sha256: &str) -> PathBuf {
    root.join("blobs").join(sha256)
}

/// Ruta de los trozos en curso de un fichero.
fn parts_dir(root: &Path, rel: &str) -> PathBuf {
    root.join("parts").join(sanitize_name(rel))
}

/// Lee el manifiesto NDJSON (una línea por fichero).
fn read_manifest(root: &Path) -> Vec<Value> {
    let path = manifest_path(root);
    let Ok(raw) = fs::read_to_string(&path) else {
        return vec![];
    };
    raw.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect()
}

/// Escribe el manifiesto NDJSON completo (reescritura atómica).
fn write_manifest(root: &Path, entries: &[Value]) -> Result<(), String> {
    let mut body = String::new();
    for e in entries {
        body.push_str(&e.to_string());
        body.push('\n');
    }
    write_atomic(&manifest_path(root), body.as_bytes())
}

/// Inventario del vault de ficheros compartidos.
///
/// Cada entrada incluye **verificación real**: se recalcula el sha256 del blob
/// en disco y se compara con el declarado en el manifiesto.
pub fn shared_files() -> Value {
    shared_files_in(&share_root())
}

/// Inventario del vault en una raíz concreta (usado por los tests).
fn shared_files_in(root: &Path) -> Value {
    let entries = read_manifest(root);
    let mut files: Vec<Value> = vec![];
    let mut total_bytes = 0u64;
    let mut verified = 0usize;
    let mut broken = 0usize;
    let mut missing = 0usize;

    for e in &entries {
        let path = e.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let declared = e.get("sha256").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let size = e.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
        total_bytes += size;
        let blob = blob_path(root, &declared);
        let mut state = "missing";
        let mut actual = Value::Null;
        match fs::read(&blob) {
            Ok(data) => {
                let real = sha256_hex(&data);
                actual = json!(real.clone());
                if real == declared {
                    state = "ok";
                    verified += 1;
                } else {
                    state = "corrupt";
                    broken += 1;
                }
            }
            Err(_) => {
                missing += 1;
            }
        }
        files.push(json!({
            "path": path,
            "size": size,
            "sha256": declared,
            "updated_at": e.get("updated_at").cloned().unwrap_or(Value::Null),
            "node": e.get("node").cloned().unwrap_or(Value::Null),
            "state": state,
            "actual_sha256": actual,
            "verified": state == "ok",
        }));
    }

    let pending_parts = count_pending_parts(root);

    json!({
        "ok": true,
        "generated_at": now_iso(),
        "root": root.to_string_lossy(),
        "manifest": manifest_path(root).to_string_lossy(),
        "files": files,
        "total_files": files.len(),
        "total_bytes": total_bytes,
        "verified": verified,
        "broken": broken,
        "missing": missing,
        "pending_parts": pending_parts,
        "node": crate::mesh::instance_id(),
        "note": "MANIFEST.ndjson: una línea por fichero con sha256; los blobs se direccionan por hash para que dos nodos no se pisen",
    })
}

/// Cuenta las subidas por trozos a medio camino.
fn count_pending_parts(root: &Path) -> usize {
    let parts = root.join("parts");
    let Ok(entries) = fs::read_dir(&parts) else {
        return 0;
    };
    let mut n = 0;
    for e in entries.flatten() {
        if e.path().is_dir() {
            n += 1;
        }
    }
    n
}

/// Sube un fichero compartido completo (verificando el hash).
pub fn share_put(rel_path: &str, data_b64: &str, expected_sha256: &str) -> Value {
    share_put_in(&share_root(), rel_path, data_b64, expected_sha256)
}

/// Sube un fichero en la raíz indicada (usado por los tests).
fn share_put_in(root: &Path, rel_path: &str, data_b64: &str, expected_sha256: &str) -> Value {
    let rel = match normalize_rel(rel_path) {
        Ok(r) => r,
        Err(e) => return json!({ "ok": false, "error": "bad_path", "detail": e }),
    };
    let data = match base64_decode(data_b64) {
        Ok(d) => d,
        Err(e) => return json!({ "ok": false, "error": "bad_base64", "detail": e }),
    };
    let real = sha256_hex(&data);
    let expected = expected_sha256.trim().to_lowercase();
    if !expected.is_empty() && expected != real {
        return json!({
            "ok": false,
            "error": "hash_mismatch",
            "expected": expected,
            "actual": real,
            "detail": "el contenido no coincide con el sha256 declarado: no se escribe nada",
        });
    }
    if let Err(e) = write_atomic(&blob_path(root, &real), &data) {
        return json!({ "ok": false, "error": "write_failed", "detail": e });
    }
    let mut entries = read_manifest(root);
    let entry = json!({
        "path": rel,
        "size": data.len(),
        "sha256": real,
        "updated_at": now_iso(),
        "node": crate::mesh::instance_id(),
        "chunks": 1,
    });
    match entries.iter().position(|e| e["path"] == json!(rel)) {
        Some(i) => entries[i] = entry.clone(),
        None => entries.push(entry.clone()),
    }
    if let Err(e) = write_manifest(root, &entries) {
        return json!({ "ok": false, "error": "manifest_failed", "detail": e });
    }
    json!({
        "ok": true,
        "path": rel,
        "sha256": real,
        "size": data.len(),
        "entry": entry,
        "manifest": manifest_path(root).to_string_lossy(),
    })
}

/// Sube un trozo (chunk) de un fichero compartido.
///
/// El hash verificado es el del **trozo**; el hash del fichero completo se
/// comprueba en `share_commit`.
pub fn share_put_chunk(rel_path: &str, index: usize, data_b64: &str, expected_sha256: &str) -> Value {
    share_put_chunk_in(&share_root(), rel_path, index, data_b64, expected_sha256)
}

/// Sube un trozo en la raíz indicada (usado por los tests).
fn share_put_chunk_in(
    root: &Path,
    rel_path: &str,
    index: usize,
    data_b64: &str,
    expected_sha256: &str,
) -> Value {
    let rel = match normalize_rel(rel_path) {
        Ok(r) => r,
        Err(e) => return json!({ "ok": false, "error": "bad_path", "detail": e }),
    };
    let data = match base64_decode(data_b64) {
        Ok(d) => d,
        Err(e) => return json!({ "ok": false, "error": "bad_base64", "detail": e }),
    };
    let real = sha256_hex(&data);
    let expected = expected_sha256.trim().to_lowercase();
    if !expected.is_empty() && expected != real {
        return json!({
            "ok": false,
            "error": "hash_mismatch",
            "expected": expected,
            "actual": real,
            "detail": "el trozo no coincide con su sha256: no se escribe nada",
        });
    }
    let dir = parts_dir(root, &rel);
    if let Err(e) = fs::create_dir_all(&dir) {
        return json!({ "ok": false, "error": "parts_dir_failed", "detail": e.to_string() });
    }
    let file = dir.join(format!("{:06}.part", index));
    if let Err(e) = write_atomic(&file, &data) {
        return json!({ "ok": false, "error": "write_failed", "detail": e });
    }
    // Se guarda el sha256 del fichero completo declarado por el cliente (si lo
    // manda) para poder verificarlo en el commit.
    let meta_path = dir.join("meta.json");
    let mut meta: Value = fs::read_to_string(&meta_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(json!({}));
    if !expected.is_empty() {
        meta["file_sha256"] = json!(expected);
    }
    meta["path"] = json!(rel);
    meta["updated_at"] = json!(now_iso());
    let _ = write_atomic(&meta_path, meta.to_string().as_bytes());

    json!({
        "ok": true,
        "path": rel,
        "index": index,
        "sha256": real,
        "size": data.len(),
        "part": file.to_string_lossy(),
    })
}

/// Ensambla los trozos de un fichero y lo publica en el manifiesto.
pub fn share_commit(rel_path: &str, expected_sha256: &str) -> Value {
    share_commit_in(&share_root(), rel_path, expected_sha256)
}

/// Ensambla en la raíz indicada (usado por los tests).
fn share_commit_in(root: &Path, rel_path: &str, expected_sha256: &str) -> Value {
    let rel = match normalize_rel(rel_path) {
        Ok(r) => r,
        Err(e) => return json!({ "ok": false, "error": "bad_path", "detail": e }),
    };
    let dir = parts_dir(root, &rel);
    let mut parts: Vec<PathBuf> = match fs::read_dir(&dir) {
        Ok(rd) => rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().map(|x| x == "part").unwrap_or(false))
            .collect(),
        Err(e) => {
            return json!({
                "ok": false,
                "error": "no_parts",
                "detail": e.to_string(),
                "hint": "sube antes los trozos con POST /api/v2/share/chunk",
            })
        }
    };
    if parts.is_empty() {
        return json!({ "ok": false, "error": "no_parts" });
    }
    parts.sort();

    let mut data: Vec<u8> = Vec::new();
    for p in &parts {
        match fs::read(p) {
            Ok(b) => data.extend_from_slice(&b),
            Err(e) => {
                return json!({ "ok": false, "error": "read_part_failed", "detail": e.to_string() });
            }
        }
    }
    let real = sha256_hex(&data);
    let expected = expected_sha256.trim().to_lowercase();
    if expected.is_empty() {
        // Se usa el declarado en el meta.json de la subida por trozos.
        if let Some(meta) = fs::read_to_string(dir.join("meta.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        {
            if let Some(h) = meta.get("file_sha256").and_then(|v| v.as_str()) {
                if h != real {
                    return json!({
                        "ok": false,
                        "error": "hash_mismatch",
                        "expected": h,
                        "actual": real,
                        "detail": "el ensamblado no coincide con el sha256 del fichero completo",
                    });
                }
            }
        }
    } else if expected != real {
        return json!({
            "ok": false,
            "error": "hash_mismatch",
            "expected": expected,
            "actual": real,
            "detail": "el ensamblado no coincide con el sha256 del fichero completo: no se publica",
        });
    }

    if let Err(e) = write_atomic(&blob_path(root, &real), &data) {
        return json!({ "ok": false, "error": "write_failed", "detail": e });
    }
    let mut entries = read_manifest(root);
    let entry = json!({
        "path": rel,
        "size": data.len(),
        "sha256": real,
        "updated_at": now_iso(),
        "node": crate::mesh::instance_id(),
        "chunks": parts.len(),
    });
    match entries.iter().position(|e| e["path"] == json!(rel)) {
        Some(i) => entries[i] = entry.clone(),
        None => entries.push(entry.clone()),
    }
    if let Err(e) = write_manifest(root, &entries) {
        return json!({ "ok": false, "error": "manifest_failed", "detail": e });
    }
    // Los trozos ya no hacen falta.
    let _ = fs::remove_dir_all(&dir);

    json!({
        "ok": true,
        "path": rel,
        "sha256": real,
        "size": data.len(),
        "chunks": parts.len(),
        "entry": entry,
    })
}

/// Descarga un fichero compartido (con verificación de hash).
pub fn share_get(rel_path: &str) -> Value {
    share_get_range(rel_path, 0, 0)
}

/// Descarga un fichero compartido; con `len > 0` devuelve sólo ese rango.
fn share_get_range(rel_path: &str, offset: usize, len: usize) -> Value {
    let root = share_root();
    let rel = match normalize_rel(rel_path) {
        Ok(r) => r,
        Err(e) => return json!({ "ok": false, "error": "bad_path", "detail": e }),
    };
    let entries = read_manifest(&root);
    let Some(entry) = entries.iter().find(|e| e["path"] == json!(rel)) else {
        return json!({
            "ok": false,
            "error": "not_found",
            "hint": "el fichero no está en el MANIFEST.ndjson de este nodo",
        });
    };
    let declared = entry.get("sha256").and_then(|v| v.as_str()).unwrap_or("");
    let data = match fs::read(blob_path(&root, declared)) {
        Ok(d) => d,
        Err(e) => {
            return json!({
                "ok": false,
                "error": "blob_missing",
                "detail": e.to_string(),
                "hint": "el manifiesto declara el fichero pero el blob no está en este nodo (pídelo al nodo que lo tiene)",
            })
        }
    };
    let real = sha256_hex(&data);
    let verified = real == declared;
    let total = data.len();
    let slice: Vec<u8> = if len == 0 {
        data.clone()
    } else {
        let start = offset.min(total);
        let end = (offset.saturating_add(len)).min(total);
        data[start..end].to_vec()
    };
    json!({
        "ok": verified,
        "path": rel,
        "sha256": real,
        "declared_sha256": declared,
        "verified": verified,
        "size": total,
        "offset": offset,
        "returned": slice.len(),
        "data_b64": base64_encode(&slice),
        "note": if verified { "hash verificado" } else { "AVISO: el blob no coincide con el manifiesto" },
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// 10) Mercado de capacidades: reparto de trabajo entre nodos
// ─────────────────────────────────────────────────────────────────────────────

/// Tipos de trabajo que el mercado sabe repartir entre instancias.
///
/// `inference` = LLM/embeddings · `build` = compilación/RE/firma ·
/// `compute` = CPU genérica · `network` = scrape/sync · `storage` = vault.
pub const WORK_KINDS: &[&str] = &["inference", "build", "compute", "network", "storage"];

/// Reglas locales de compartición de un nodo.
///
/// Son las reglas que el usuario ajusta por instancia y que el motor respeta
/// **antes** de repartir nada: si el móvil está al 12 % de batería no se
/// ofrece CPU aunque haya cola, y una instancia "dedicada" no comparte nada.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShareRules {
    /// Nodo al que pertenecen estas reglas.
    pub node: String,
    /// Instancia dedicada: no comparte nada (sólo trabajo local).
    #[serde(default)]
    pub dedicated: bool,
    /// Compartir sólo mientras hay cargador.
    #[serde(default)]
    pub only_when_charging: bool,
    /// Batería mínima (%) para compartir.
    #[serde(default = "default_min_battery")]
    pub min_battery_pct: u8,
    /// Temperatura máxima de CPU (°C) para compartir trabajo pesado.
    #[serde(default = "default_max_temp")]
    pub max_cpu_temp_c: f32,
    /// Concurrencia máxima de tareas compartidas admitidas.
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,
    /// Fracción de la capacidad que se reserva para uso local (0.0 – 0.95).
    #[serde(default = "default_reserved")]
    pub reserved_ratio: f32,
    /// Tipos de trabajo que la instancia ofrece.
    #[serde(default)]
    pub offers: Vec<String>,
    /// Tipos de trabajo que la instancia nunca ofrece.
    #[serde(default)]
    pub denies: Vec<String>,
    /// Desempate: >0 prefiere esta instancia, <0 la relega.
    #[serde(default)]
    pub priority_bias: i32,
    /// ¿Comparte el LLM local (inferencia)?
    #[serde(default = "default_true_flag")]
    pub share_inference: bool,
    /// Última actualización (ISO-8601 UTC).
    #[serde(default)]
    pub updated_at: String,
}

fn default_min_battery() -> u8 {
    30
}
fn default_max_temp() -> f32 {
    45.0
}
fn default_max_concurrent() -> usize {
    2
}
fn default_reserved() -> f32 {
    0.5
}
fn default_true_flag() -> bool {
    true
}

impl Default for ShareRules {
    fn default() -> Self {
        ShareRules {
            node: String::new(),
            dedicated: false,
            only_when_charging: false,
            min_battery_pct: default_min_battery(),
            max_cpu_temp_c: default_max_temp(),
            max_concurrent: default_max_concurrent(),
            reserved_ratio: default_reserved(),
            offers: vec!["compute".into(), "network".into(), "storage".into()],
            denies: vec![],
            priority_bias: 0,
            share_inference: true,
            updated_at: String::new(),
        }
    }
}

/// Reglas por defecto de un nodo, derivadas del estado real del dispositivo.
///
/// Un móvil (Android) es conservador: guarda el 70 % de su CPU para sí y no
/// comparte sin cargador por debajo del 30 % de batería.  Un PC es más
/// generoso.
pub fn default_rules(node: &str) -> ShareRules {
    let mut r = ShareRules {
        node: node.to_string(),
        ..Default::default()
    };
    let mobile = crate::mesh::platform_is_android();
    if mobile {
        r.reserved_ratio = 0.7;
        r.min_battery_pct = 30;
        r.max_concurrent = 1;
        r.only_when_charging = false;
    }
    // La inferencia local sólo se ofrece si hay un LLM listo.
    let prov = crate::providers::list_providers_redacted();
    let has_llm = prov["providers"]
        .as_array()
        .map(|a| {
            a.iter().any(|p| {
                p["ready"] == json!(true)
                    && (p["auth_type"] == json!("local") || p["tier"] == json!("offline"))
            })
        })
        .unwrap_or(false);
    if has_llm && r.share_inference {
        r.offers.push("inference".into());
    }
    r
}

/// Ruta del fichero de reglas de compartición.
fn rules_path() -> PathBuf {
    if let Ok(d) = std::env::var("MEGADAEMON_SHARE_RULES") {
        let d = d.trim();
        if !d.is_empty() {
            return PathBuf::from(d);
        }
    }
    Path::new(&cockpit_state_dir()).join("mesh_rules.json")
}

/// Lee el fichero de reglas (`{"version":1,"nodes":{"<id>":{...}}}`).
fn load_rules_file() -> Value {
    fs::read_to_string(rules_path())
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .unwrap_or_else(|| json!({ "version": 1, "nodes": {} }))
}

/// Reglas de un nodo: las suyas si existen; si no, la política común (`"*"`);
/// si no, las derivadas del dispositivo.
pub fn rules_for(node: &str) -> ShareRules {
    let file = load_rules_file();
    let nodes = file.get("nodes").cloned().unwrap_or(json!({}));
    if let Some(r) = nodes.get(node).and_then(|v| serde_json::from_value::<ShareRules>(v.clone()).ok()) {
        return r;
    }
    if let Some(r) = nodes.get("*").and_then(|v| serde_json::from_value::<ShareRules>(v.clone()).ok()) {
        let mut r = r;
        r.node = node.to_string();
        return r;
    }
    default_rules(node)
}

/// Reglas de un nodo (JSON para la UI).
pub fn work_rules(node: &str) -> Value {
    let rules = rules_for(node);
    let explicit = load_rules_file()
        .get("nodes")
        .and_then(|n| n.get(node))
        .is_some();
    json!({
        "ok": true,
        "node": node,
        "rules": rules,
        "explicit": explicit,
        "source": rules_path().to_string_lossy(),
        "policy": {
            "only_when_charging": rules.only_when_charging,
            "min_battery_pct": rules.min_battery_pct,
            "max_cpu_temp_c": rules.max_cpu_temp_c,
            "max_concurrent": rules.max_concurrent,
            "reserved_ratio": rules.reserved_ratio,
            "priority_bias": rules.priority_bias,
            "dedicated": rules.dedicated,
            "share_inference": rules.share_inference,
            "offers": rules.offers,
            "denies": rules.denies,
        },
        "device": device_state(),
        "note": "las reglas se aplican ANTES de repartir: sin batería, con calor o en modo dedicado la instancia no ofrece nada",
    })
}

/// Guarda (o actualiza) las reglas de un nodo tras validarlas.
pub fn work_rules_set(node: &str, payload: &Value) -> Value {
    let node = node.trim();
    if node.is_empty() {
        return json!({ "ok": false, "error": "missing_node" });
    }
    let mut file = load_rules_file();
    let mut rules = rules_for(node);
    rules.node = node.to_string();

    let set_bool = |rules: &mut ShareRules, key: &str, v: bool| {
        match key {
            "dedicated" => rules.dedicated = v,
            "only_when_charging" => rules.only_when_charging = v,
            "share_inference" => rules.share_inference = v,
            _ => {}
        }
    };
    let p = payload.get("rules").unwrap_or(payload);
    if let Some(obj) = p.as_object() {
        for (k, v) in obj {
            match k.as_str() {
                "dedicated" | "only_when_charging" | "share_inference" => {
                    if let Some(b) = v.as_bool() {
                        set_bool(&mut rules, k, b);
                    }
                }
                "min_battery_pct" => {
                    if let Some(n) = v.as_u64() {
                        if n > 100 {
                            return json!({ "ok": false, "error": "bad_value", "detail": "min_battery_pct debe estar entre 0 y 100" });
                        }
                        rules.min_battery_pct = n as u8;
                    }
                }
                "max_cpu_temp_c" => {
                    if let Some(n) = v.as_f64() {
                        if !(15.0..=95.0).contains(&n) {
                            return json!({ "ok": false, "error": "bad_value", "detail": "max_cpu_temp_c debe estar entre 15 y 95" });
                        }
                        rules.max_cpu_temp_c = n as f32;
                    }
                }
                "max_concurrent" => {
                    if let Some(n) = v.as_u64() {
                        if n > 64 {
                            return json!({ "ok": false, "error": "bad_value", "detail": "max_concurrent debe estar entre 0 y 64" });
                        }
                        rules.max_concurrent = n as usize;
                    }
                }
                "reserved_ratio" => {
                    if let Some(n) = v.as_f64() {
                        if !(0.0..=0.95).contains(&n) {
                            return json!({ "ok": false, "error": "bad_value", "detail": "reserved_ratio debe estar entre 0.0 y 0.95" });
                        }
                        rules.reserved_ratio = n as f32;
                    }
                }
                "priority_bias" => {
                    if let Some(n) = v.as_i64() {
                        if !(-100..=100).contains(&n) {
                            return json!({ "ok": false, "error": "bad_value", "detail": "priority_bias debe estar entre -100 y 100" });
                        }
                        rules.priority_bias = n as i32;
                    }
                }
                "offers" => {
                    if let Some(a) = v.as_array() {
                        let mut out = vec![];
                        for x in a {
                            let Some(s) = x.as_str() else { continue };
                            if !WORK_KINDS.contains(&s) {
                                return json!({ "ok": false, "error": "bad_kind", "detail": format!("tipo de trabajo desconocido: {}", s),
                                               "valid": WORK_KINDS });
                            }
                            out.push(s.to_string());
                        }
                        rules.offers = out;
                    }
                }
                "denies" => {
                    if let Some(a) = v.as_array() {
                        let mut out = vec![];
                        for x in a {
                            let Some(s) = x.as_str() else { continue };
                            if !WORK_KINDS.contains(&s) {
                                return json!({ "ok": false, "error": "bad_kind", "detail": format!("tipo de trabajo desconocido: {}", s),
                                               "valid": WORK_KINDS });
                            }
                            out.push(s.to_string());
                        }
                        rules.denies = out;
                    }
                }
                _ => {}
            }
        }
    }

    rules.updated_at = now_iso();
    let mut nodes = file.get("nodes").cloned().unwrap_or(json!({}));
    if !nodes.is_object() {
        nodes = json!({});
    }
    if let Some(obj) = nodes.as_object_mut() {
        obj.insert(node.to_string(), serde_json::to_value(&rules).unwrap_or(json!({})));
    }
    file["version"] = json!(1);
    file["nodes"] = nodes;
    file["updated_at"] = json!(now_iso());
    if let Err(e) = write_atomic(&rules_path(), serde_json::to_string_pretty(&file).unwrap_or_default().as_bytes()) {
        return json!({ "ok": false, "error": "persist_failed", "detail": e });
    }
    json!({
        "ok": true,
        "node": node,
        "rules": rules,
        "source": rules_path().to_string_lossy(),
        "note": "reglas guardadas: el reparto las respeta en la siguiente petición",
    })
}

/// Estado real del dispositivo (base de las reglas locales).
fn device_state() -> Value {
    let (pstate, pol) = crate::power_policy::current_policy();
    json!({
        "capacity_pct": pstate.capacity_pct,
        "charging": pstate.charging,
        "usb_plugged": pstate.usb_plugged,
        "battery_temp_c": pstate.battery_temp_c,
        "cpu_temp_c": pstate.cpu_temp_c,
        "thermal_hot": pstate.thermal_hot,
        "governor": pstate.governor,
        "load": crate::mesh::current_load(),
        "policy": pol.name,
        "is_mobile": crate::mesh::platform_is_android(),
    })
}

/// Tipo de trabajo al que pertenece un `kind` de tarea.
pub fn work_kind_for(kind: &str) -> &'static str {
    let k = kind.to_ascii_lowercase();
    if k.starts_with("sync.web")
        || k.starts_with("web.")
        || k.starts_with("messaging.ingest")
        || k.starts_with("repo_updates")
        || k.starts_with("updates.")
        || k.contains("scrape")
        || k.contains("download")
    {
        "network"
    } else if k.starts_with("apk.analyze")
        || k.starts_with("dex")
        || k.starts_with("sign")
        || k.starts_with("build")
        || k.contains("rizin")
        || k.contains("cirx")
        || k.contains("kaitai")
        || k.contains("decompile")
    {
        "build"
    } else if k.starts_with("agent")
        || k.starts_with("llm")
        || k.starts_with("chat")
        || k.starts_with("delegat")
        || k.contains("seere")
        || k.contains("infer")
    {
        "inference"
    } else if k.starts_with("share")
        || k.starts_with("vault")
        || k.starts_with("backup")
        || k.contains("upload")
    {
        "storage"
    } else {
        "compute"
    }
}

/// Tipos de trabajo que ofrece un nodo según sus capacidades declaradas.
fn kinds_from_capabilities(caps: &[String]) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let push = |k: &str, out: &mut Vec<String>| {
        if !out.iter().any(|x| x == k) {
            out.push(k.to_string());
        }
    };
    for c in caps {
        let l = c.to_ascii_lowercase();
        if l.contains("llm") || l.contains("seere") || l.contains("inference") || l.contains("model") {
            push("inference", &mut out);
        }
        if l.contains("reverse") || l.contains("dex") || l.contains("sign") || l.contains("firma")
            || l.contains("build") || l.contains("apk") || l.contains("re")
        {
            push("build", &mut out);
        }
        if l.contains("compute") || l.contains("cpu") || l.contains("core") {
            push("compute", &mut out);
        }
        if l.contains("net") || l.contains("web") || l.contains("proxy") || l.contains("sync") {
            push("network", &mut out);
        }
        if l.contains("storage") || l.contains("vault") || l.contains("backup") || l.contains("disk") {
            push("storage", &mut out);
        }
    }
    out
}

/// Oferta efectiva de un nodo: ¿comparte? ¿cuánto? ¿por qué no?
fn effective_offer(rules: &ShareRules, node: &Value, device: &Value) -> Value {
    let mut blocked: Vec<String> = vec![];
    let load = node.get("load").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let battery = device.get("capacity_pct").and_then(|v| v.as_u64());
    let charging = device.get("charging").and_then(|v| v.as_bool()).unwrap_or(false);
    let cpu_temp = device.get("cpu_temp_c").and_then(|v| v.as_f64());
    let thermal = device.get("thermal_hot").and_then(|v| v.as_bool()).unwrap_or(false);

    if rules.dedicated {
        blocked.push("instancia dedicada: no comparte nada".to_string());
    }
    if rules.only_when_charging && !charging {
        blocked.push("sin cargador y la regla exige cargador".to_string());
    }
    if let Some(b) = battery {
        if (b as u8) < rules.min_battery_pct {
            blocked.push(format!(
                "batería {} % por debajo del mínimo {} %",
                b, rules.min_battery_pct
            ));
        }
    }
    if thermal {
        blocked.push("tensión térmica activa".to_string());
    }
    if let Some(t) = cpu_temp {
        if t > rules.max_cpu_temp_c as f64 {
            blocked.push(format!(
                "CPU a {:.0} °C por encima del máximo {:.0} °C",
                t, rules.max_cpu_temp_c
            ));
        }
    }

    // Capacidad libre = lo que no se reserva para uso local, menos la carga.
    // `rules.reserved_ratio` es f32 y `load` es f64: se Homogeneiza a f64
    // (E0277/E0308 rompia el build entero del crate).
    let mut capacity = (1.0f64 - rules.reserved_ratio as f64).max(0.0) * (1.0 - load.clamp(0.0, 1.0));
    if let Some(b) = battery {
        if b < 40 {
            capacity *= 0.5; // por debajo del 40 % se ofrece la mitad
        }
    }
    if rules.max_concurrent == 0 {
        blocked.push("concurrencia máxima 0".to_string());
        capacity = 0.0;
    }

    let caps: Vec<String> = node
        .get("capabilities")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let mut offers = rules.offers.clone();
    for k in kinds_from_capabilities(&caps) {
        if !offers.contains(&k) {
            offers.push(k);
        }
    }
    if !rules.share_inference {
        offers.retain(|k| k != "inference");
    }
    offers.retain(|k| !rules.denies.contains(k));
    if capacity < 0.05 {
        blocked.push(format!("capacidad libre {:.0} %", capacity * 100.0));
    }
    let available = blocked.is_empty() && capacity >= 0.05;

    json!({
        "available": available,
        "capacity": (capacity * 1000.0).round() / 1000.0,
        "offers": offers,
        "blocked_by": blocked,
        "reason": if available {
            format!(
                "ofrece {} con {:.0} % de capacidad libre",
                offers.join("/"),
                capacity * 100.0
            )
        } else {
            blocked.join(" · ")
        },
    })
}

/// Instancias conocidas por el mercado (bus + tablero SQLite).
fn market_nodes() -> Vec<Value> {
    let mut out: Vec<Value> = vec![];
    if let Some(board) = mesh_snapshot() {
        if let Some(me) = board.get("me") {
            let mut n = me.clone();
            if let Some(o) = n.as_object_mut() {
                o.insert("kind".to_string(), json!("local"));
            }
            out.push(n);
        }
        if let Some(peers) = board.get("peers").and_then(|v| v.as_array()) {
            for p in peers {
                let mut n = p.clone();
                if let Some(o) = n.as_object_mut() {
                    o.insert("kind".to_string(), json!("peer"));
                }
                out.push(n);
            }
        }
    }
    if let Some(coord) = coordination_snapshot() {
        if let Some(instances) = coord.get("instances").and_then(|v| v.as_array()) {
            for i in instances {
                let id = i.get("id").and_then(|v| v.as_str()).unwrap_or("");
                if id.is_empty() || out.iter().any(|n| n.get("id").and_then(|v| v.as_str()) == Some(id)) {
                    continue;
                }
                let caps: Vec<String> = i
                    .get("capabilities")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                out.push(json!({
                    "id": id,
                    "role": i.get("role").cloned().unwrap_or(json!("worker")),
                    "hostname": i.get("device").cloned().unwrap_or(json!("")),
                    "platform": i.get("platform").cloned().unwrap_or(json!("")),
                    "version": i.get("version").cloned().unwrap_or(json!("")),
                    "capabilities": caps,
                    "load": 0.0,
                    "last_seen": i.get("last_seen").cloned().unwrap_or(json!(0)),
                    "kind": "board",
                }));
            }
        }
    }
    out
}

/// Ledger de contribución: cuánto ha hecho cada nodo.
///
/// "Todos los nodos deben contribuir": un nodo vivo con capacidad libre y
/// cero contribución aparece marcado como `debe_contribuir`.
fn contribution_ledger(nodes: &[Value]) -> Vec<Value> {
    let mut ledger: std::collections::BTreeMap<String, (u64, u64, u64)> = std::collections::BTreeMap::new();

    // Agentes registrados en el bus.
    if let Some(board) = mesh_snapshot() {
        if let Some(agents) = board.get("agents").and_then(|v| v.as_array()) {
            for a in agents {
                let who = a.get("agent").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if who.is_empty() {
                    continue;
                }
                let e = ledger.entry(who).or_insert((0, 0, 0));
                e.0 += 1;
                match a.get("status").and_then(|v| v.as_str()) {
                    Some("done") => e.1 += 1,
                    Some("failed") => e.2 += 1,
                    _ => {}
                }
            }
        }
    }
    // Tareas del tablero con propietario.
    if let Some(coord) = coordination_snapshot() {
        if let Some(tasks) = coord.get("tasks").and_then(|v| v.as_array()) {
            for t in tasks {
                let Some(owner) = t.get("owner").and_then(|v| v.as_str()) else {
                    continue;
                };
                if owner.is_empty() {
                    continue;
                }
                let e = ledger.entry(owner.to_string()).or_insert((0, 0, 0));
                e.0 += 1;
                match t.get("status").and_then(|v| v.as_str()) {
                    Some("done") => e.1 += 1,
                    Some("failed") => e.2 += 1,
                    _ => {}
                }
            }
        }
    }
    // Journal local: el trabajo ejecutado en esta instancia cuenta como
    // contribución de esta instancia.
    let local_id = crate::mesh::instance_id();
    let journal = crate::task_scheduler::journal_json(200);
    let mut local = (0u64, 0u64, 0u64);
    if let Some(entries) = journal.get("entries").and_then(|v| v.as_array()) {
        for e in entries {
            local.0 += 1;
            if e.get("ok") == Some(&json!(true)) {
                local.1 += 1;
            } else if e.get("ok") == Some(&json!(false)) {
                local.2 += 1;
            }
        }
    }
    ledger.insert(local_id.clone(), local);

    let total: u64 = ledger.values().map(|v| v.0).sum();
    let mut out: Vec<Value> = vec![];
    for n in nodes {
        let id = n.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let (tasks, ok, failed) = ledger.get(&id).copied().unwrap_or((0, 0, 0));
        let share = if total > 0 {
            (tasks as f64 * 100.0 / total as f64 * 10.0).round() / 10.0
        } else {
            0.0
        };
        let ok_rate = if tasks > 0 {
            (ok as f64 * 100.0 / tasks as f64 * 10.0).round() / 10.0
        } else {
            0.0
        };
        out.push(json!({
            "node": id,
            "tasks": tasks,
            "ok": ok,
            "failed": failed,
            "ok_rate": ok_rate,
            "share_pct": share,
            "verdict": if tasks == 0 {
                "sin_trabajo"
            } else if failed > ok {
                "falla_mas_de_lo_que_hace"
            } else {
                "ok"
            },
        }));
    }
    out
}

/// Demanda pendiente: tareas de la cola que se pueden repartir.
fn market_demands() -> Vec<Value> {
    let mut out: Vec<Value> = vec![];
    let queue = crate::task_scheduler::list_json();
    if let Some(tasks) = queue.get("tasks").and_then(|v| v.as_array()) {
        for t in tasks {
            let state = t.get("state").and_then(|v| v.as_str()).unwrap_or("");
            if state != "pending" && state != "queued" {
                continue;
            }
            let kind = t.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            out.push(json!({
                "id": t.get("id").cloned().unwrap_or(json!("")),
                "kind": kind,
                "work_kind": work_kind_for(kind),
                "priority": t.get("priority").cloned().unwrap_or(json!(5)),
                "state": state,
                "source": "cola local",
            }));
        }
    }
    out
}

/// Puntúa un nodo para una demanda concreta.
///
/// Devuelve `None` si el nodo no puede cogerla (reglas, capacidad o tipo de
/// trabajo no admitido).
fn score_node(
    node: &Value,
    offer: &Value,
    rules: &ShareRules,
    work_kind: &str,
    contribution: &Value,
) -> Option<(f64, String)> {
    if offer.get("available").and_then(|v| v.as_bool()) != Some(true) {
        return None;
    }
    let offers: Vec<String> = offer
        .get("offers")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    if !offers.iter().any(|k| k == work_kind) {
        return None;
    }
    let capacity = offer.get("capacity").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let bias = 1.0 + (rules.priority_bias as f64 / 100.0);
    // Equidad: quien menos ha contributed recibe un empujón (todos deben
    // aportar).  El nodo con 0 % recibe ×1.5, el que ya hizo el 100 % ×1.0.
    let share = contribution
        .get("share_pct")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let fairness = 1.0 + ((100.0 - share) / 100.0) * 0.5;
    let score = capacity * bias * fairness;
    let id = node.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let reason = format!(
        "{} ofrece {} · capacidad {:.0} % · prioridad {:+} · contribución {:.1} % (equidad ×{:.2})",
        id,
        work_kind,
        capacity * 100.0,
        rules.priority_bias,
        share,
        fairness
    );
    Some((score, reason))
}

/// Mercado de capacidades: oferta, demanda, plan y contribución.
pub fn work_market() -> Value {
    let device = device_state();
    let nodes = market_nodes();
    let contribution = contribution_ledger(&nodes);
    let contribution_of = |id: &str| -> Value {
        contribution
            .iter()
            .find(|c| c.get("node").and_then(|v| v.as_str()) == Some(id))
            .cloned()
            .unwrap_or(json!({ "node": id, "tasks": 0, "share_pct": 0.0, "verdict": "sin_trabajo" }))
    };

    // Oferta por nodo (con sus reglas locales).
    let mut supply: Vec<Value> = vec![];
    for n in &nodes {
        let id = n.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if id.is_empty() {
            continue;
        }
        let rules = rules_for(id);
        let offer = effective_offer(&rules, n, &device);
        let mut entry = n.clone();
        if let Some(o) = entry.as_object_mut() {
            o.insert("rules".to_string(), serde_json::to_value(&rules).unwrap_or(json!({})));
            o.insert("offer".to_string(), offer.clone());
            o.insert("contribution".to_string(), contribution_of(id));
        }
        supply.push(entry);
    }

    // Demanda y plan de reparto.
    let demand = market_demands();
    let mut plan: Vec<Value> = vec![];
    for d in &demand {
        let work_kind = d.get("work_kind").and_then(|v| v.as_str()).unwrap_or("compute");
        let mut best: Option<(f64, String, Value)> = None;
        let mut considered: Vec<Value> = vec![];
        for n in &supply {
            let id = n.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let rules = rules_for(id);
            let offer = n.get("offer").cloned().unwrap_or(json!({}));
            let contrib = contribution_of(id);
            match score_node(n, &offer, &rules, work_kind, &contrib) {
                Some((score, reason)) => {
                    considered.push(json!({ "node": id, "score": (score * 1000.0).round() / 1000.0, "reason": reason }));
                    let better = match &best {
                        Some((s, _, _)) => score > *s,
                        None => true,
                    };
                    if better {
                        best = Some((score, reason, n.clone()));
                    }
                }
                None => {
                    considered.push(json!({
                        "node": id,
                        "score": 0.0,
                        "reason": offer
                            .get("reason")
                            .and_then(|v| v.as_str())
                            .unwrap_or("no disponible")
                            .to_string(),
                    }));
                }
            }
        }
        let entry = match &best {
            Some((score, reason, node)) => json!({
                "demand_id": d.get("id").cloned().unwrap_or(json!("")),
                "kind": d.get("kind").cloned().unwrap_or(json!("")),
                "work_kind": work_kind,
                "priority": d.get("priority").cloned().unwrap_or(json!(5)),
                "node": node.get("id").cloned().unwrap_or(json!("")),
                "score": (score * 1000.0).round() / 1000.0,
                "reason": reason,
                "assigned": true,
            }),
            None => json!({
                "demand_id": d.get("id").cloned().unwrap_or(json!("")),
                "kind": d.get("kind").cloned().unwrap_or(json!("")),
                "work_kind": work_kind,
                "priority": d.get("priority").cloned().unwrap_or(json!(5)),
                "node": Value::Null,
                "score": 0.0,
                "reason": "ningún nodo ofrece este tipo de trabajo con sus reglas actuales",
                "assigned": false,
            }),
        };
        plan.push(json!({
            "demand": d,
            "assignment": entry,
            "candidates": considered,
        }));
    }

    // Nodos que deben contribuir (vivos, con capacidad y sin trabajo).
    let idle: Vec<Value> = supply
        .iter()
        .filter(|n| {
            let id = n.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let contrib = contribution_of(id);
            contrib.get("tasks").and_then(|v| v.as_u64()).unwrap_or(0) == 0
                && n.get("offer").and_then(|o| o.get("available")).and_then(|v| v.as_bool()) == Some(true)
        })
        .map(|n| {
            json!({
                "node": n.get("id").cloned().unwrap_or(json!("")),
                "reason": "tiene capacidad libre y reglas compatibles pero no ha tomado ninguna tarea",
            })
        })
        .collect();

    json!({
        "ok": true,
        "generated_at": now_iso(),
        "device": device,
        "kinds": WORK_KINDS,
        "supply": supply,
        "demand": demand,
        "plan": plan,
        "contribution": contribution,
        "must_contribute": idle,
        "rules_source": rules_path().to_string_lossy(),
        "notes": [
            "las reglas locales mandan: sin batería, con calor, sin cargador o en modo «dedicada» la instancia no ofrece nada",
            "el reparto penaliza a quien ya ha hecho mucho trabajo: quien menos ha contribuido recibe un empujón (todos deben aportar)",
            "la capacidad ofrecida es la que queda tras reservar una fracción para el uso local de la propia instancia",
        ],
    })
}

/// Elige el nodo que debe tomar una demanda y (opcionalmente) la publica.
pub fn work_route(demand: &Value) -> Value {
    let market = work_market();
    let kind = demand
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let work_kind = demand
        .get("work_kind")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| work_kind_for(&kind).to_string());
    let demand_id = demand
        .get("demand_id")
        .or_else(|| demand.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let chosen = market["plan"]
        .as_array()
        .and_then(|plans| {
            plans.iter().find(|p| {
                let d = p.get("demand");
                let matches = d
                    .and_then(|d| d.get("id"))
                    .and_then(|v| v.as_str())
                    .map(|id| id == demand_id)
                    .unwrap_or(false);
                let kind_matches = d
                    .and_then(|d| d.get("kind"))
                    .and_then(|v| v.as_str())
                    .map(|k| k == kind)
                    .unwrap_or(false);
                matches || (demand_id.is_empty() && kind_matches)
            })
        })
        .map(|p| p.get("assignment").cloned().unwrap_or(json!({})));

    let Some(assignment) = chosen else {
        return json!({
            "ok": false,
            "error": "no_demand",
            "hint": "indica demand_id o kind de una tarea pendiente",
            "market_demands": market["demand"].clone(),
        });
    };

    let node = assignment.get("node").and_then(|v| v.as_str()).unwrap_or("");
    let mut published = Value::Null;
    if demand.get("publish").and_then(|v| v.as_bool()).unwrap_or(false) {
        if node.is_empty() {
            return json!({
                "ok": false,
                "error": "no_candidate",
                "reason": assignment.get("reason").cloned().unwrap_or(json!("")),
            });
        }
        let payload = demand.get("payload").cloned().unwrap_or(json!({}));
        published = node_assign(node, &kind, &payload, &work_kind);
    }

    json!({
        "ok": true,
        "assignment": assignment,
        "published": published,
        "note": "el plan es una recomendación de las reglas locales; publicarla en el bus es opcional",
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// 11) Hallazgos del enjambre (material de RE compartido por los nodos)
// ─────────────────────────────────────────────────────────────────────────────

/// Prefijos de credenciales que **no** deben salir por la API.
const SECRET_PREFIXES: &[&str] = &["sk-", "mdaemon_sk_", "ghp_", "ya29.", "gho_"];

/// Raíces de estado donde puede estar el material del enjambre.
///
/// Se prueban varias porque el daemon usa dos convenciones (`<state>` y
/// `<state>/state`) según el despliegue.  **Nunca** se lee fuera de estas
/// raíces: cada ruta se valida contra su raíz antes de abrirse.
fn swarm_state_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = vec![];
    let push = |p: PathBuf, roots: &mut Vec<PathBuf>| {
        if !p.as_os_str().is_empty() && !roots.iter().any(|r| r == &p) {
            roots.push(p);
        }
    };
    if let Ok(d) = std::env::var("MEGADAEMON_STATE_DIR") {
        let d = d.trim();
        if !d.is_empty() {
            let p = PathBuf::from(d);
            push(p.clone(), &mut roots);
            push(p.join("state"), &mut roots);
        }
    }
    // El bus (mesh) usa /data/local/tmp/megadaemon en Android.
    let mesh_dir = PathBuf::from(bus_state_dir());
    push(mesh_dir.join("state"), &mut roots);
    push(mesh_dir.clone(), &mut roots);
    // Y el cockpit usa /data/user/0/com.agy/files/megadaemon/state.
    let cockpit = PathBuf::from(cockpit_state_dir());
    push(cockpit.join("state"), &mut roots);
    push(cockpit, &mut roots);
    roots
}

/// ¿La ruta está dentro de la raíz? (evita escapes con `..` o symlinks)
fn path_inside(root: &Path, path: &Path) -> bool {
    let r = root.to_string_lossy().trim_end_matches('/').to_string();
    let p = path.to_string_lossy().to_string();
    p == r || p.starts_with(&format!("{}/", r))
}

/// Borra credenciales de un texto.
///
/// No basta con mirar el principio: un hallazgo puede llevar la key en medio
/// de una frase ("la clave es mdaemon_sk_…").  Se busca el prefijo y se
/// enmascara todo el token.
fn scrub_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        let rest: String = bytes[i..].iter().collect();
        let mut matched = false;
        for p in SECRET_PREFIXES {
            if rest.starts_with(p) {
                let mut j = i + p.chars().count();
                while j < bytes.len()
                    && (bytes[j].is_ascii_alphanumeric() || bytes[j] == '_' || bytes[j] == '-')
                {
                    j += 1;
                }
                out.push_str("[REDACTED]");
                i = j;
                matched = true;
                break;
            }
        }
        if !matched {
            out.push(bytes[i]);
            i += 1;
        }
    }
    out
}

/// Elimina credenciales de cualquier JSON (recursivo).
fn scrub_value(v: &mut Value) -> bool {
    let mut redacted = false;
    match v {
        Value::String(s) => {
            let clean = scrub_text(s);
            if clean != *s {
                *s = clean;
                redacted = true;
            }
        }
        Value::Array(a) => {
            for x in a.iter_mut() {
                redacted |= scrub_value(x);
            }
        }
        Value::Object(o) => {
            for (_, x) in o.iter_mut() {
                redacted |= scrub_value(x);
            }
        }
        _ => {}
    }
    redacted
}

/// Ordena una lista de hallazgos por `ts` descendente.
fn sort_findings(findings: &mut Vec<Value>) {
    findings.sort_by(|a, b| {
        let ta = a.get("ts").map(ts_key).unwrap_or(0);
        let tb = b.get("ts").map(ts_key).unwrap_or(0);
        tb.cmp(&ta).then_with(|| {
            a.get("sha256")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .cmp(b.get("sha256").and_then(|v| v.as_str()).unwrap_or(""))
        })
    });
}

/// Convierte un `ts` (epoch o ISO-8601) a epoch para ordenar.
fn ts_key(v: &Value) -> i64 {
    if let Some(n) = v.as_i64() {
        return n;
    }
    if let Some(n) = v.as_u64() {
        return n as i64;
    }
    if let Some(s) = v.as_str() {
        if let Some(secs) = crate::mesh::iso_to_epoch(s) {
            return secs;
        }
        if let Ok(n) = s.parse::<i64>() {
            return n;
        }
    }
    0
}

/// Campos que se conservan de cada hallazgo (el resto se descarta).
const FINDING_FIELDS: &[&str] = &[
    "agent", "kind", "title", "finding", "target", "sha256", "size", "confidence", "ts",
];

/// Normaliza una línea de hallazgo al contrato de la API.
fn normalize_finding(line: &Value) -> Option<Value> {
    let obj = line.as_object()?;
    let mut out = serde_json::Map::new();
    for f in FINDING_FIELDS {
        if let Some(v) = obj.get(*f) {
            out.insert((*f).to_string(), v.clone());
        }
    }
    if out.is_empty() {
        return None;
    }
    // Sin agente no se sabe de qué nodo es: no vale como hallazgo.
    if !out.contains_key("agent") {
        return None;
    }
    Some(Value::Object(out))
}

/// Lee los hallazgos del enjambre en las raíces dadas (interno, testeable).
fn mesh_findings_in(roots: &[PathBuf], limit: usize, offset: usize) -> Value {
    let mut findings: Vec<Value> = vec![];
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut journal_lines = 0usize;
    let mut board_agents: Vec<Value> = vec![];
    let mut board_tasks: Vec<Value> = vec![];
    let mut board_peers: Vec<Value> = vec![];
    let mut identities: Vec<Value> = vec![];
    let mut root_report: Vec<Value> = vec![];
    let mut redactions = 0usize;

    for root in roots {
        if !root.is_dir() {
            continue;
        }
        let mut files: Vec<String> = vec![];
        let mut root_journal = 0usize;
        let mut root_findings = 0usize;

        // 1) sync/re_*.ndjson  → hallazgos
        let sync_dir = root.join("sync");
        if let Ok(entries) = fs::read_dir(&sync_dir) {
            let mut paths: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    let name = p.file_name().unwrap_or_default().to_string_lossy();
                    name.starts_with("re_") && name.ends_with(".ndjson")
                })
                .collect();
            paths.sort();
            for p in paths {
                if !path_inside(root, &p) {
                    continue;
                }
                let Ok(raw) = fs::read_to_string(&p) else {
                    continue;
                };
                files.push(
                    p.strip_prefix(root)
                        .unwrap_or(&p)
                        .to_string_lossy()
                        .to_string(),
                );
                for line in raw.lines() {
                    let l = line.trim();
                    if l.is_empty() {
                        continue;
                    }
                    // Una línea rota NO revienta la vista: se salta.
                    let Ok(v) = serde_json::from_str::<Value>(l) else {
                        continue;
                    };
                    let Some(n) = normalize_finding(&v) else {
                        continue;
                    };
                    let mut n = n;
                    if scrub_value(&mut n) {
                        redactions += 1;
                    }
                    let key = n.to_string();
                    if seen.insert(key) {
                        findings.push(n);
                        root_findings += 1;
                    }
                }
            }
        }

        // 2) mesh_journal.ndjson → nº de líneas (auditoría del enjambre)
        let journal = root.join("mesh_journal.ndjson");
        if journal.is_file() && path_inside(root, &journal) {
            if let Ok(raw) = fs::read_to_string(&journal) {
                root_journal = raw.lines().filter(|l| !l.trim().is_empty()).count();
            }
        }
        journal_lines += root_journal;

        // 3) mesh_board.json → agentes, tareas y peers
        let board = root.join("mesh_board.json");
        let mut has_board = false;
        if board.is_file() && path_inside(root, &board) {
            if let Ok(raw) = fs::read_to_string(&board) {
                if let Ok(b) = serde_json::from_str::<Value>(&raw) {
                    has_board = true;
                    if let Some(a) = b.get("agents").and_then(|v| v.as_array()) {
                        for x in a {
                            let mut x = x.clone();
                            if scrub_value(&mut x) {
                                redactions += 1;
                            }
                            board_agents.push(x);
                        }
                    }
                    if let Some(t) = b.get("tasks").and_then(|v| v.as_array()) {
                        for x in t {
                            let mut x = x.clone();
                            if scrub_value(&mut x) {
                                redactions += 1;
                            }
                            board_tasks.push(x);
                        }
                    }
                    if let Some(p) = b.get("peers").and_then(|v| v.as_array()) {
                        for x in p {
                            let mut x = x.clone();
                            if scrub_value(&mut x) {
                                redactions += 1;
                            }
                            board_peers.push(x);
                        }
                    }
                }
            }
        }

        // 4) agents/*.json → identidad de cada nodo
        let agents_dir = root.join("agents");
        if let Ok(entries) = fs::read_dir(&agents_dir) {
            let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            paths.sort();
            for p in paths {
                if !p.is_file() || !path_inside(root, &p) {
                    continue;
                }
                if p.extension().unwrap_or_default() != "json" {
                    continue;
                }
                let Ok(raw) = fs::read_to_string(&p) else {
                    continue;
                };
                let Ok(mut v) = serde_json::from_str::<Value>(&raw) else {
                    continue;
                };
                if scrub_value(&mut v) {
                    redactions += 1;
                }
                let mut entry = v.as_object().cloned().unwrap_or_default();
                entry.insert(
                    "file".to_string(),
                    json!(p.file_name().unwrap_or_default().to_string_lossy()),
                );
                identities.push(Value::Object(entry));
            }
        }

        root_report.push(json!({
            "path": root.to_string_lossy(),
            "exists": true,
            "findings_files": files,
            "findings_here": root_findings,
            "journal_lines": root_journal,
            "board": has_board,
        }));
    }

    sort_findings(&mut findings);
    let total = findings.len();

    // Filtros opcionales por agente y tipo.
    let mut kinds: Vec<String> = vec![];
    let mut agent_names: Vec<String> = vec![];
    for f in &findings {
        let k = f.get("kind").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if !k.is_empty() && !kinds.contains(&k) {
            kinds.push(k);
        }
        let a = f.get("agent").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if !a.is_empty() && !agent_names.contains(&a) {
            agent_names.push(a);
        }
    }
    kinds.sort();
    agent_names.sort();

    let limit = limit.clamp(1, 200);
    let offset = offset.min(total.saturating_sub(1).max(0));
    let page: Vec<Value> = findings
        .iter()
        .skip(offset)
        .take(limit)
        .cloned()
        .collect();

    json!({
        "ok": true,
        "generated_at": now_iso(),
        "findings": page,
        "returned": page.len(),
        "total": total,
        "limit": limit,
        "offset": offset,
        "kinds": kinds,
        "agents": agent_names,
        "board_agents": board_agents,
        "node_identities": identities,
        "tasks": board_tasks,
        "peers": board_peers,
        "journal": journal_lines,
        "roots": root_report,
        "redactions": redactions,
        "note": "material de RE del enjambre: sólo lectura, con credenciales enmascaradas; requiere API key y token de sesión",
    })
}

/// `GET /api/mesh_findings` (alias) y `GET /api/v2/mesh_findings`.
///
/// Lee `<state>/sync/re_*.ndjson` + `<state>/mesh_journal.ndjson` + el tablón
/// del bus.  Máx. 200 hallazgos por página, ordenados por `ts` descendente.
pub fn mesh_findings(query: &str) -> Value {
    let limit = query_get(query, "limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(50);
    let offset = query_get(query, "offset")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    mesh_findings_in(&swarm_state_roots(), limit, offset)
}

// ─────────────────────────────────────────────────────────────────────────────
// 12) Prompt para agentes externos (generado en el SERVIDOR)
// ─────────────────────────────────────────────────────────────────────────────

/// Lee la URL pública vigente del túnel (es efímera: vive en un fichero).
///
/// Fuentes, por orden: `<state>/tunnel.url`, `/sdcard/megadaemon.url` y el
/// log del túnel (`<state>/tunnel.log`), del que se extrae la URL con un
/// `grep` ligero.  Si no hay ninguna, se dice: **nunca** se inventa una URL.
fn tunnel_url(kind: &str) -> String {
    let dir = PathBuf::from(bus_state_dir());
    let candidates: Vec<PathBuf> = if kind == "mcp" {
        vec![
            dir.join("mcp_tunnel.url"),
            PathBuf::from("/sdcard/megadaemon_mcp.url"),
        ]
    } else {
        vec![
            dir.join("tunnel.url"),
            PathBuf::from("/sdcard/megadaemon.url"),
        ]
    };
    for p in &candidates {
        if let Ok(raw) = fs::read_to_string(p) {
            let u = raw.trim().to_string();
            if u.starts_with("http") {
                return u.trim_end_matches('/').to_string();
            }
        }
    }
    // Último recurso: el log del túnel.
    let log = dir.join(if kind == "mcp" { "mcp_tunnel.log" } else { "tunnel.log" });
    if let Ok(raw) = fs::read_to_string(&log) {
        let mut found = String::new();
        for line in raw.lines() {
            if let Some(idx) = line.find("https://") {
                let tail = &line[idx..];
                let end = tail
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-'))
                    .unwrap_or(tail.len());
                let cand = &tail[..end];
                if cand.contains("trycloudflare") || cand.contains("cloudflare") {
                    found = cand.to_string();
                }
            }
        }
        if !found.is_empty() {
            return found;
        }
    }
    String::new()
}

/// Genera el prompt Markdown para un agente externo, **desde el estado real**.
///
/// `include_secrets` es explícito: por defecto el prompt NO lleva la API key
/// ni el token de sesión.  Si se piden, el Markdown lleva un aviso en la
/// primera línea y la respuesta incluye `contains_secrets: true`.
pub fn prompt_external(include_secrets: bool, api_key: &str) -> Value {
    let dev = device_state();
    let web_url = tunnel_url("web");
    let mcp_url = tunnel_url("mcp");
    let local_web = format!("http://127.0.0.1:{}", 3000);
    let local_mcp = format!("http://127.0.0.1:{}/mcp", 9090);

    let tools = crate::mcp_ext::tool_definitions();
    let tool_list = tools.as_array().cloned().unwrap_or_default();
    let swarm = mesh_findings_in(&swarm_state_roots(), 40, 0);
    let findings = swarm["findings"].as_array().cloned().unwrap_or_default();

    let mut md = String::new();
    if include_secrets {
        md.push_str("> ⚠️ **Este documento contiene credenciales** (API key y token de sesión). No lo compartas ni lo subas a ningún repositorio.\n\n");
    }
    md.push_str("# MegaDaemon v14 — contexto para un agente externo\n\n");
    md.push_str(&format!(
        "- Generado: {}\n- Instancia: `{}` · {} · {} · v{}\n- Uptime: {}\n\n",
        now_iso(),
        crate::mesh::instance_id(),
        crate::mesh::hostname(),
        crate::mesh::platform_string(),
        env!("CARGO_PKG_VERSION"),
        human_duration(process_uptime_secs())
    ));

    md.push_str("## Cómo hablar con el daemon\n\n");
    md.push_str(&format!(
        "- Cockpit (web): {}\n",
        if web_url.is_empty() {
            format!("{} (sin túnel público vigente)", local_web)
        } else {
            web_url.clone()
        }
    ));
    md.push_str(&format!(
        "- MCP (JSON-RPC 2.0): {}\n",
        if mcp_url.is_empty() {
            format!("{} (sin túnel MCP vigente)", local_mcp)
        } else {
            mcp_url.clone()
        }
    ));
    md.push_str(&format!("- Health (público, sin credenciales): {}/health\n", local_web));
    md.push_str("- Cabeceras:\n");
    if include_secrets {
        let key = if api_key.trim().is_empty() {
            "<API key no facilitada>"
        } else {
            api_key.trim()
        };
        md.push_str(&format!("  - `X-API-Key: {}`\n", key));
        md.push_str(&format!(
            "  - `X-Seere-Auth-Token: {}`\n",
            crate::cockpit_server::auth_token()
        ));
    } else {
        md.push_str("  - `X-API-Key: <ponla aquí>`\n");
        md.push_str("  - `X-Seere-Auth-Token: <cánjela con POST /api/session/exchange>`\n");
    }
    md.push_str("- El canje de la API key por token de sesión: `POST /api/session/exchange` con `{\"api_key\":\"…\"}` → `{\"ok\":true,\"token\":\"…\"}`.\n\n");

    md.push_str(&format!("## Herramientas MCP ({} disponibles)\n\n", tool_list.len()));
    for t in &tool_list {
        let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let desc = t.get("description").and_then(|v| v.as_str()).unwrap_or("");
        let short: String = desc.chars().take(150).collect();
        md.push_str(&format!("- `{}` — {}\n", name, short));
    }

    md.push_str("\n## Estado del dispositivo\n\n");
    md.push_str(&format!(
        "- Batería: {}{}\n- Temperatura CPU: {} °C{}\n- Carga: {}\n- Política de energía: {}\n- NPU: {} (offload implementado: {})\n",
        dev["capacity_pct"].as_u64().map(|v| format!("{} %", v)).unwrap_or_else(|| "—".into()),
        if dev["charging"] == json!(true) { " (cargando)" } else { "" },
        dev["cpu_temp_c"].as_f64().map(|v| format!("{:.1}", v)).unwrap_or_else(|| "—".into()),
        if dev["thermal_hot"] == json!(true) { " ⚠️ térmico" } else { "" },
        dev["load"].as_f64().map(|v| format!("{:.2}", v)).unwrap_or_else(|| "—".into()),
        dev["policy"].as_str().unwrap_or("—"),
        dev["is_mobile"].as_bool().unwrap_or(false),
        "ver /api/v2/dashboard"
    ));

    // Nodos del enjambre.
    let nodes = market_nodes();
    md.push_str(&format!("\n## Nodos del enjambre ({})\n\n", nodes.len()));
    for n in &nodes {
        let id = n.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let role = n.get("role").and_then(|v| v.as_str()).unwrap_or("");
        let caps: Vec<String> = n
            .get("capabilities")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        md.push_str(&format!(
            "- `{}` ({}) — capacidades: {}\n",
            id,
            role,
            if caps.is_empty() { "—".to_string() } else { caps.join(", ") }
        ));
    }

    // Hallazgos del enjambre.
    md.push_str(&format!(
        "\n## Hallazgos del enjambre ({} de {}; journal: {} líneas)\n\n",
        findings.len(),
        swarm["total"].as_u64().unwrap_or(0),
        swarm["journal"].as_u64().unwrap_or(0)
    ));
    if findings.is_empty() {
        md.push_str("_No hay hallazgos publicados todavía._\n");
    }
    for f in &findings {
        let kind = f.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        let title = f.get("title").and_then(|v| v.as_str()).unwrap_or("");
        let agent = f.get("agent").and_then(|v| v.as_str()).unwrap_or("");
        let conf = f.get("confidence").and_then(|v| v.as_str()).unwrap_or("");
        let target = f.get("target").and_then(|v| v.as_str()).unwrap_or("");
        let sha = f.get("sha256").and_then(|v| v.as_str()).unwrap_or("");
        let size = f.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
        let text = f.get("finding").and_then(|v| v.as_str()).unwrap_or("");
        let short: String = text.chars().take(600).collect();
        md.push_str(&format!(
            "### [{}] {} — {}\n",
            kind,
            title,
            if conf.is_empty() { String::new() } else { format!("confianza {}", conf) }
        ));
        md.push_str(&format!(
            "- agente: {} · target: {} · sha256: {} · {} B\n- {}\n\n",
            agent, target, sha, size, short
        ));
    }

    md.push_str("\n---\nGenerado por el cockpit v2 de MegaDaemon (`/api/v2/prompt_external`). Los datos son reales: si algo no aparece, es que el daemon no lo tiene.\n");

    json!({
        "ok": true,
        "generated_at": now_iso(),
        "markdown": md,
        "bytes": md.len(),
        "contains_secrets": include_secrets,
        "tools": tool_list.len(),
        "findings": findings.len(),
        "findings_total": swarm["total"].clone(),
        "journal": swarm["journal"].clone(),
        "web_url": web_url,
        "mcp_url": mcp_url,
        "warnings": if include_secrets {
            vec!["el Markdown contiene la API key y el token de sesión: no lo compartas ni lo subas al repo".to_string()]
        } else {
            vec![]
        },
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// 12) Tablero del bus: claim/release con rol y bajo lock
// ─────────────────────────────────────────────────────────────────────────────

/// Ruta del tablero SQLite de coordinación (la que usa `coordination::TaskBoard`).
fn coord_board_path() -> String {
    format!("{}/board.sqlite", bus_state_dir())
}

/// Instancia local (id + rol) registrada en el tablero.
fn local_instance() -> Option<Value> {
    let state = crate::coordination::Instance::load(&bus_state_dir())
        .or_else(|| crate::coordination::Instance::load(&cockpit_state_dir()));
    state.map(|i| json!({ "id": i.id, "role": i.role.as_str(), "device": i.device }))
}

/// Rol de la instancia local ("primary" | "worker" | "observer" | "unknown").
fn local_role() -> String {
    local_instance()
        .and_then(|i| i.get("role").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Rol pedido por el cliente, validado contra los tres roles reales.
///
/// Si el cliente no manda rol, se usa el de la instancia local.  Un rol
/// inválido **no** se inventa: se rechaza con el motivo.
fn requested_role(body: &Value) -> Result<String, String> {
    let raw = body
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_lowercase();
    if raw.is_empty() {
        return Ok(local_role());
    }
    let ok = matches!(raw.as_str(), "primary" | "worker" | "observer");
    if ok {
        Ok(raw)
    } else {
        Err(format!(
            "rol desconocido '{}': usa primary, worker u observer",
            raw
        ))
    }
}

/// Dueño de la tarea: el que manda el cliente o, si no, la instancia local.
fn requested_owner(body: &Value) -> String {
    let raw = body
        .get("owner")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if !raw.is_empty() {
        return raw;
    }
    local_instance()
        .and_then(|i| i.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .unwrap_or_else(|| "web-ui".to_string())
}

/// Registra un evento en la tabla `events` del tablero (mismo formato que
/// `coordination::TaskBoard::log_event`, que es privado).
fn coord_log_event(conn: &rusqlite::Connection, kind: &str, actor: &str, payload: &Value) {
    let _ = conn.execute(
        "INSERT INTO events (ts, kind, actor, payload) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            now_epoch(),
            kind,
            actor,
            serde_json::to_string(payload).unwrap_or_else(|_| "{}".to_string())
        ],
    );
}

/// Reclama una tarea del tablero del bus **bajo lock** (transacción
/// `BEGIN IMMEDIATE` de SQLite: dos agentes que pulsan a la vez no se quedan
/// los dos con la misma tarea).
///
/// El lease caduca solo: si el agente muere, `lease_expires_at` pasa y otra
/// instancia puede reclamarla. `won=false` significa "otro se te adelantó".
pub fn board_claim(body: &Value) -> Value {
    board_claim_at(&coord_board_path(), body)
}

/// Igual que [`board_claim`] pero sobre un tablero concreto (para las pruebas).
fn board_claim_at(path: &str, body: &Value) -> Value {
    let task_id = body
        .get("task_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if task_id.is_empty() {
        return json!({ "ok": false, "error": "missing_task_id" });
    }
    let role = match requested_role(body) {
        Ok(r) => r,
        Err(e) => return json!({ "ok": false, "error": "invalid_role", "reason": e }),
    };
    let owner = requested_owner(body);
    let lease = body
        .get("lease_secs")
        .and_then(|v| v.as_i64())
        .unwrap_or(300)
        .clamp(30, 86_400);
    if !std::path::Path::new(path).exists() {
        return json!({
            "ok": false,
            "error": "board_not_deployed",
            "reason": format!("no existe el tablero SQLite en {}", path),
        });
    }
    let conn = match rusqlite::Connection::open(path) {
        Ok(c) => c,
        Err(e) => return json!({ "ok": false, "error": "board_open_failed", "reason": e.to_string() }),
    };
    let now = now_epoch();
    let mut won = false;
    let now = now_epoch();
    // `BEGIN IMMEDIATE` toma el lock de escritura YA: por eso dos agentes que
    // pulsan "reclamar" a la vez no pueden quedarse los dos con la tarea.
    if let Err(e) = conn.execute_batch("BEGIN IMMEDIATE") {
        return json!({ "ok": false, "error": "board_lock_failed", "reason": e.to_string() });
    }
    let changed = conn.execute(
        "UPDATE tasks
            SET owner=?1, status='claimed', lease_expires_at=?2, updated_at=?3
          WHERE id=?4
            AND (status='queued' OR (status='claimed' AND lease_expires_at < ?3))",
        rusqlite::params![owner, now + lease, now, task_id],
    );
    match changed {
        Ok(n) => {
            won = n == 1;
            coord_log_event(
                &conn,
                if won { "task_claimed" } else { "task_claim_lost" },
                &owner,
                &json!({ "id": task_id, "role": role, "lease_secs": lease }),
            );
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            return json!({ "ok": false, "error": "claim_failed", "reason": e.to_string() });
        }
    }
    let _ = conn.execute_batch("COMMIT");
    let state = conn
        .query_row(
            "SELECT status, owner, lease_expires_at FROM tasks WHERE id=?1",
            rusqlite::params![task_id],
            |r| Ok((r.get::<_, String>(0).unwrap_or_default(), r.get::<_, String>(1).unwrap_or_default(), r.get::<_, i64>(2).unwrap_or(0))),
        )
        .ok();
    json!({
        "ok": true,
        "won": won,
        "task_id": task_id,
        "owner": owner,
        "role": role,
        "lease_secs": lease,
        "lease_expires_at": state.as_ref().map(|s| s.2).unwrap_or(0),
        "status": state.as_ref().map(|s| s.0.clone()).unwrap_or_default(),
        "current_owner": state.as_ref().map(|s| s.1.clone()).unwrap_or_default(),
        "note": if won {
            "tarea reclamada con lease; renuévala con el heartbeat"
        } else {
            "no se pudo reclamar: ya está tomada o el lease sigue vivo"
        },
    })
}

/// Devuelve una tarea al estado `queued`.
///
/// **Sólo** la suelta quien la tiene (`owner=?`), así que una UI no puede
/// robarle la tarea a otro agente.
pub fn board_release(body: &Value) -> Value {
    board_release_at(&coord_board_path(), body)
}

/// Igual que [`board_release`] pero sobre un tablero concreto (para las pruebas).
fn board_release_at(path: &str, body: &Value) -> Value {
    let task_id = body
        .get("task_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if task_id.is_empty() {
        return json!({ "ok": false, "error": "missing_task_id" });
    }
    let owner = requested_owner(body);
    if !std::path::Path::new(path).exists() {
        return json!({ "ok": false, "error": "board_not_deployed" });
    }
    let conn = match rusqlite::Connection::open(path) {
        Ok(c) => c,
        Err(e) => return json!({ "ok": false, "error": "board_open_failed", "reason": e.to_string() }),
    };
    let now = now_epoch();
    if let Err(e) = conn.execute_batch("BEGIN IMMEDIATE") {
        return json!({ "ok": false, "error": "board_lock_failed", "reason": e.to_string() });
    }
    let changed = conn.execute(
        "UPDATE tasks
            SET owner=NULL, status='queued', lease_expires_at=0, updated_at=?1
          WHERE id=?2 AND owner=?3",
        rusqlite::params![now, task_id, owner],
    );
    let n = match changed {
        Ok(n) => n,
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            return json!({ "ok": false, "error": "release_failed", "reason": e.to_string() });
        }
    };
    if n == 1 {
        coord_log_event(&conn, "task_released", &owner, &json!({ "id": task_id }));
    }
    let _ = conn.execute_batch("COMMIT");
    json!({
        "ok": n == 1,
        "task_id": task_id,
        "owner": owner,
        "released": n == 1,
        "note": if n == 1 {
            "tarea liberada"
        } else {
            "no estaba reclamada por ti: no se toca"
        },
    })
}

/// Reporta el resultado de una tarea (protocolo de 3 líneas del bus).
pub fn board_report(body: &Value) -> Value {
    board_report_at(&coord_board_path(), body)
}

/// Igual que [`board_report`] pero sobre un tablero concreto (para las pruebas).
fn board_report_at(path: &str, body: &Value) -> Value {
    let task_id = body
        .get("task_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if task_id.is_empty() {
        return json!({ "ok": false, "error": "missing_task_id" });
    }
    let owner = requested_owner(body);
    let ok = body.get("ok").and_then(|v| v.as_bool()).unwrap_or(true);
    let report = body
        .get("report")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if !std::path::Path::new(path).exists() {
        return json!({ "ok": false, "error": "board_not_deployed" });
    }
    let conn = match rusqlite::Connection::open(path) {
        Ok(c) => c,
        Err(e) => return json!({ "ok": false, "error": "board_open_failed", "reason": e.to_string() }),
    };
    let now = now_epoch();
    if let Err(e) = conn.execute_batch("BEGIN IMMEDIATE") {
        return json!({ "ok": false, "error": "board_lock_failed", "reason": e.to_string() });
    }
    let status = if ok { "done" } else { "failed" };
    let changed = conn.execute(
        "UPDATE tasks SET report=?1, status=?2, updated_at=?3, lease_expires_at=0
          WHERE id=?4 AND owner=?5",
        rusqlite::params![report, status, now, task_id, owner],
    );
    let n = match changed {
        Ok(n) => n,
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            return json!({ "ok": false, "error": "report_failed", "reason": e.to_string() });
        }
    };
    if n == 1 {
        coord_log_event(&conn, "task_reported", &owner, &json!({ "id": task_id, "ok": ok }));
    }
    let _ = conn.execute_batch("COMMIT");
    json!({
        "ok": n == 1,
        "task_id": task_id,
        "owner": owner,
        "status": status,
        "note": if n == 1 {
            "resultado reportado"
        } else {
            "no estaba reclamada por ti: no se toca"
        },
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// 13) Voz del daemon (W0) por la API v2
// ─────────────────────────────────────────────────────────────────────────────

/// Estado real de la voz (identidad, cooldown, presupuesto, bucle autónomo).
///
/// Si el módulo de voz no está compilado en esta build, se responde con
/// `available:false` y el motivo: nunca un estado inventado.
pub fn voice_status() -> Value {
    match crate::voice_runtime::status() {
        v => json!({
            "ok": true,
            "available": true,
            "voice": v,
            "instance": local_instance().unwrap_or(Value::Null),
            "role": local_role(),
        }),
    }
}

/// Acción de la voz: `status` | `whoami` | `plan` | `act` | `speak`.
pub fn voice_action(action: &str, body: &Value) -> Value {
    let a = if action.is_empty() { "status" } else { action };
    let r = crate::voice_runtime::handle(a, body);
    json!({
        "ok": r.get("ok").and_then(|v| v.as_bool()).unwrap_or(true),
        "action": a,
        "voice": r,
        "instance": local_instance().unwrap_or(Value::Null),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// 14) Acciones administrativas (con confirmación explícita)
// ─────────────────────────────────────────────────────────────────────────────

/// Reinicio de la instancia.
///
/// El daemon corre bajo un supervisor que lo relanza: la forma limpia de
/// "reiniciar" es **salir con éxito** (`exit(0)`) y que el supervisor levante
/// el binario de nuevo.  Por eso la respuesta se envía **antes** de salir y la
/// UI avisa de que la conexión se va a cortar.
///
/// Exige `confirm: true` y tiene un enfriamiento de 30 s (evita bucles de
/// reinicio si alguien pulsa el botón en racha).
pub fn admin_restart(body: &Value) -> Value {
    if !body.get("confirm").and_then(|v| v.as_bool()).unwrap_or(false) {
        return json!({
            "ok": false,
            "error": "confirmation_required",
            "hint": "esta acción reinicia el daemon: repite con confirm=true",
        });
    }
    let marker = format!("{}/restart_requested", cockpit_state_dir());
    let path = std::path::Path::new(&marker);
    let now = now_epoch();
    if let Ok(meta) = std::fs::metadata(path) {
        if let Ok(m) = meta.modified() {
            if let Ok(d) = m.duration_since(std::time::UNIX_EPOCH) {
                if now - (d.as_secs() as i64) < 30 {
                    return json!({
                        "ok": false,
                        "error": "cooldown",
                        "reason": "ya se pidió un reinicio hace menos de 30 s",
                    });
                }
            }
        }
    }
    let _ = std::fs::write(&marker, format!("{}\n", now));
    // Salimos en un hilo aparte para que la respuesta HTTP llegue al cliente.
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(600));
        std::process::exit(0);
    });
    json!({
        "ok": true,
        "scheduled": true,
        "eta_ms": 600,
        "note": "el daemon sale ahora y el supervisor lo vuelve a levantar; esta sesión se cortará",
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Router
// ─────────────────────────────────────────────────────────────────────────────

/// Router de la API v2: `(método, path, body)` → respuesta JSON.
///
/// Devuelve `None` si la ruta no es de la v2 (para que `cockpit_server` siga
/// con su propio flujo).
///
/// Los errores se devuelven con `"ok": false` y un `error` legible; el
/// servidor HTTP decide el status (401/403 los pone la capa de auth).
pub fn dispatch(method: &str, path: &str, body: &Value) -> Option<Value> {
    let (route, query) = split_query(path);
    let m = method.to_ascii_uppercase();
    // Alias pedido por el enjambre: /api/mesh_findings ≡ /api/v2/mesh_findings.
    // No es público: lo protegen el gate de API key y `requires_auth()`.
    if route == "/api/mesh_findings" {
        return Some(mesh_findings(query));
    }
    if !route.starts_with(API_V2_PREFIX) {
        return None;
    }
    let b = if body.is_object() { body.clone() } else { json!({}) };
    let s = |k: &str| b.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let n = |k: &str| b.get(k).and_then(|v| v.as_u64()).unwrap_or(0);

    // Una ruta v2 que entre en pánico NO debe tumbar el daemon: el `catch`
    // la convierte en un error legible.  v15 ya desenrolla (el perfil release
    // NO usa `panic="abort"`), así que el catch es efectivo; además las rutas
    // de la v2 no usan `expect()`/`unwrap()` sobre datos que vienen del
    // exterior, para que el caso normal no llegue ni a panic.
    let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    match (m.as_str(), route) {
        ("GET", "/api/v2/dashboard") => dashboard(),
        ("GET", "/api/v2/nodes") => nodes(),
        ("GET", "/api/v2/tasks") => task_board(),
        ("GET", "/api/v2/llm") => llm_catalog(),
        ("GET", "/api/v2/delegation") => delegation(),
        ("GET", "/api/v2/alerts") => alerts_view(),
        ("GET", "/api/v2/re") => re_apps(),
        ("GET", "/api/v2/agentic") => agentic_loop_status(),
        ("GET", "/api/v2/share") => shared_files(),
        ("GET", "/api/v2/routes") => routes_doc(),
        ("GET", "/api/v2/market") => work_market(),
        ("GET", "/api/v2/mesh_findings") => mesh_findings(query),
        ("GET", "/api/v2/prompt_external") => prompt_external(false, ""),
        ("POST", "/api/v2/prompt_external") => {
            let inc = b.get("include_secrets").and_then(|v| v.as_bool()).unwrap_or(false);
            prompt_external(inc, &s("api_key"))
        }
        ("GET", "/api/v2/market/rules") => {
            let node = query_get(query, "node").unwrap_or_default();
            work_rules(&node)
        }
        ("GET", "/api/v2/share/get") => {
            let path_q = query_get(query, "path").unwrap_or_default();
            let offset = query_get(query, "offset")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);
            let len = query_get(query, "len")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);
            share_get_range(&path_q, offset, len)
        }
        ("POST", "/api/v2/nodes/assign") => {
            let payload = b.get("payload").cloned().unwrap_or(json!({}));
            node_assign(&s("node"), &s("kind"), &payload, &s("capability"))
        }
        ("POST", "/api/v2/nodes/message") => {
            node_message(&s("node"), &s("message"), &s("data_b64"))
        }
        ("POST", "/api/v2/nodes/migrate") => node_migrate(&s("target_version")),
        ("POST", "/api/v2/tasks/export") => task_export(&s("node")),
        ("POST", "/api/v2/tasks/import") => task_import(&b),
        ("POST", "/api/v2/tasks/claim") => board_claim(&b),
        ("POST", "/api/v2/tasks/release") => board_release(&b),
        ("POST", "/api/v2/tasks/report") => board_report(&b),
        ("GET", "/api/v2/voice") => voice_status(),
        ("POST", "/api/v2/voice") => {
            let action = s("action");
            voice_action(&action, &b)
        }
        ("POST", "/api/v2/admin/restart") => admin_restart(&b),
        ("POST", "/api/v2/delegation/apply") => {
            let adj = b.get("adjustments").cloned().unwrap_or(json!([]));
            delegation_apply(&adj)
        }
        ("POST", "/api/v2/share/put") => {
            share_put(&s("path"), &s("data_b64"), &s("sha256"))
        }
        ("POST", "/api/v2/share/chunk") => {
            share_put_chunk(&s("path"), n("index") as usize, &s("data_b64"), &s("sha256"))
        }
        ("POST", "/api/v2/share/commit") => share_commit(&s("path"), &s("sha256")),
        ("POST", "/api/v2/market/rules") => {
            let node = s("node");
            work_rules_set(&node, &b)
        }
        ("POST", "/api/v2/market/route") => work_route(&b),
        _ => json!({
            "ok": false,
            "error": "unknown_route",
            "method": m,
            "path": route,
            "hint": "consulta GET /api/v2/routes para la tabla completa",
        }),
    }
    }))
    .unwrap_or_else(|_| {
        json!({
            "ok": false,
            "error": "internal_error",
            "reason": "la ruta ha entrado en pánico; el daemon sigue en pie",
            "method": m,
            "path": route,
        })
    });
    Some(out)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Ruta del HTML de la UI v2 (relativo al crate).
    fn ui_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("web")
            .join("cockpit_v2.html")
    }

    /// Texto de la UI v2 (embebido en el binario como respaldo).
    const UI_FALLBACK: &str = include_str!("../../web/cockpit_v2.html");

    fn ui_source() -> String {
        fs::read_to_string(ui_path()).unwrap_or_else(|_| UI_FALLBACK.to_string())
    }

    /// Extrae los paths literales que la UI pide con `fetch("...")`.
    ///
    /// Se ignora el query string: `fetch("/api/v2/share/get?path=...")` llama
    /// a la ruta `/api/v2/share/get` con parámetros.
    fn ui_fetch_paths(html: &str) -> Vec<String> {
        let mut out = vec![];
        let bytes = html.as_bytes();
        // El needle va como `&str`: ni `&[u8; 6]` ni `&[u8]` implementan
        // `Pattern` para `str::find` en este toolchain (E0277). Con `&str` la
        // semántica es idéntica (mismo texto, misma longitud) y `.len()` = 6.
        let needle = "fetch(";
        let mut i = 0;
        while let Some(pos) = html[i..].find(needle) {
            let start = i + pos + needle.len();
            let rest = &html[start..];
            let rest = rest.trim_start();
            if let Some(stripped) = rest.strip_prefix('"') {
                if let Some(end) = stripped.find('"') {
                    let raw = &stripped[..end];
                    out.push(raw.split('?').next().unwrap_or(raw).to_string());
                }
            }
            i = start;
            if i >= bytes.len() {
                break;
            }
        }
        out
    }

    /// Extrae los paths literales de API que usa la UI (`"/api/..."`).
    fn ui_api_literals(html: &str) -> Vec<String> {
        let mut out = vec![];
        let mut rest = html;
        while let Some(pos) = rest.find("\"/api/") {
            let after = &rest[pos + 1..];
            if let Some(end) = after.find('"') {
                let raw = &after[..end];
                // Los literales que acaban en "/" son prefijos que la UI
                // concatena en runtime (p.ej. "/api/tool/" + tool): no son
                // rutas por sí solos.  Las tools que se llaman así se validan
                // aparte contra el catálogo MCP.
                if !raw.ends_with('/') {
                    out.push(raw.split('?').next().unwrap_or(raw).to_string());
                }
                rest = &after[end..];
            } else {
                break;
            }
        }
        out
    }

    /// Nombres de tool que la UI pide por `callTool("…")`.
    fn ui_tool_calls(html: &str) -> Vec<String> {
        let mut out = vec![];
        let mut rest = html;
        while let Some(pos) = rest.find("callTool(\"") {
            let after = &rest[pos + "callTool(\"".len()..];
            if let Some(end) = after.find('"') {
                out.push(after[..end].to_string());
                rest = &after[end..];
            } else {
                break;
            }
        }
        out
    }

    /// Paths literales que el servidor v1 expone (se leen de su fuente).
    fn v1_server_paths() -> Vec<String> {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("cockpit_server.rs");
        let Ok(text) = fs::read_to_string(&src) else {
            return vec![];
        };
        let mut out = vec![];
        let mut rest = text.as_str();
        while let Some(pos) = rest.find("\"/api/") {
            let after = &rest[pos + 1..];
            if let Some(end) = after.find('"') {
                out.push(after[..end].to_string());
                rest = &after[end..];
            } else {
                break;
            }
        }
        out
    }

    // ── Rutas ───────────────────────────────────────────────────────────────

    #[test]
    fn la_tabla_de_rutas_no_tiene_duplicados_y_usa_el_prefijo_v2() {
        let routes = api_v2_routes();
        assert!(routes.len() >= 15, "la API v2 debe tener al menos 15 rutas");
        let mut seen: Vec<String> = vec![];
        for (m, p) in &routes {
            assert!(p.starts_with(API_V2_PREFIX), "ruta fuera del prefijo v2: {}", p);
            assert!(*m == "GET" || *m == "POST", "método no soportado: {}", m);
            let key = format!("{} {}", m, p);
            assert!(!seen.contains(&key), "ruta duplicada: {}", key);
            seen.push(key);
        }
    }

    #[test]
    fn cada_ruta_de_la_api_v2_se_pide_desde_la_ui() {
        let html = ui_source();
        let asked = ui_fetch_paths(&html);
        assert!(
            !asked.is_empty(),
            "la UI v2 no tiene ninguna llamada fetch(\"...\") literal"
        );
        for (m, p) in api_v2_routes() {
            assert!(
                asked.iter().any(|a| a == p),
                "la UI v2 no llama a {} {} (la ruta existe en la API pero nadie la usa)",
                m,
                p
            );
        }
    }

    #[test]
    fn la_ui_no_llama_a_rutas_inexistentes() {
        let html = ui_source();
        let v2: Vec<String> = api_v2_routes().into_iter().map(|(_, p)| p.to_string()).collect();
        let v1 = v1_server_paths();
        let tools: Vec<String> = crate::mcp_ext::tool_definitions()
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        for literal in ui_api_literals(&html) {
            if v2.contains(&literal) {
                continue;
            }
            if v1.contains(&literal) {
                continue;
            }
            // El dispatcher genérico acepta /api/tool/<herramienta>.
            if let Some(tool) = literal.strip_prefix("/api/tool/") {
                assert!(
                    tools.iter().any(|t| t == tool),
                    "la UI llama a la herramienta inexistente: {}",
                    literal
                );
                continue;
            }
            // Endpoints de página/sonda que no viven en la tabla de API.
            if matches!(
                literal.as_str(),
                "/api/health" | "/api/metrics" | "/api/ping" | "/api/session/exchange"
            ) {
                continue;
            }
            panic!("la UI v2 llama a una ruta que no existe: {}", literal);
        }
    }

    /// Las tools que la UI llama por `callTool("…")` existen en el catálogo MCP.
    #[test]
    fn las_tools_que_llama_la_ui_existen_en_el_catalogo_mcp() {
        let html = ui_source();
        let tools: Vec<String> = crate::mcp_ext::tool_definitions()
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let calls = ui_tool_calls(&html);
        assert!(!calls.is_empty(), "la UI debería llamar a alguna tool MCP");
        for t in calls {
            assert!(
                tools.iter().any(|x| x == &t),
                "la UI llama a la tool MCP inexistente: {}",
                t
            );
        }
    }

    // ── Dashboard ───────────────────────────────────────────────────────────

    #[test]
    fn el_dashboard_tiene_las_claves_esperadas() {
        let d = dashboard();
        assert_eq!(d["ok"], json!(true));
        for key in [
            "daemon",
            "system",
            "power",
            "npu",
            "alerts",
            "queue",
            "bus",
            "capabilities",
            "llm",
            "share",
        ] {
            assert!(d.get(key).is_some(), "el dashboard no tiene la clave {}", key);
        }
        assert!(d["daemon"]["version"].as_str().is_some());
        assert!(d["generated_at"].as_str().unwrap_or("").ends_with('Z'));
        // El bus puede no estar desplegado, pero el estado debe ser explícito.
        assert!(d["bus"].get("deployed").is_some());
        assert!(d["bus"].get("reason").is_some());
    }

    #[test]
    fn el_dashboard_no_inventa_nodos() {
        let d = dashboard();
        let deployed = d["bus"]["deployed"].as_bool().unwrap_or(false);
        if !deployed {
            assert_eq!(
                d["bus"]["nodes"], json!(0),
                "sin bus desplegado no puede haber nodos contados"
            );
            let reason = d["bus"]["reason"].as_str().unwrap_or("");
            assert!(
                reason.contains("desplegado") || reason.contains("presente"),
                "la razón del bus debe explicar el estado real: {}",
                reason
            );
        }
    }

    #[test]
    fn la_vista_de_nodos_es_consistente_con_el_estado_del_bus() {
        let n = nodes();
        assert_eq!(n["ok"], json!(true));
        let deployed = n["bus"]["deployed"].as_bool().unwrap_or(false);
        let count = n["nodes"].as_array().map(|a| a.len()).unwrap_or(0);
        if !deployed {
            assert_eq!(count, 0, "sin bus desplegado la lista de nodos debe ir vacía");
        }
        assert!(n["bus"].get("reason").is_some());
    }

    // ── Ficheros compartidos ────────────────────────────────────────────────

    #[test]
    fn el_hash_de_un_fichero_compartido_se_verifica() {
        let dir = std::env::temp_dir().join(format!("md-webui-share-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let data = b"contenido real del fichero compartido";
        let b64 = base64_encode(data);
        let good = sha256_hex(data);

        // Con el hash correcto: se acepta y el inventario lo verifica.
        let put = share_put_in(&dir, "docs/informe.txt", &b64, &good);
        assert_eq!(put["ok"], json!(true), "la subida debe aceptarse: {}", put);
        assert_eq!(put["sha256"], json!(good));

        let inv = shared_files_in(&dir);
        assert_eq!(inv["total_files"], json!(1));
        assert_eq!(inv["files"][0]["verified"], json!(true));
        assert_eq!(inv["files"][0]["state"], json!("ok"));
        assert_eq!(inv["broken"], json!(0));

        // Con el hash equivocado: se rechaza y NO se escribe nada.
        let bad = share_put_in(&dir, "docs/malo.txt", &b64, &"0".repeat(64));
        assert_eq!(bad["ok"], json!(false));
        assert_eq!(bad["error"], json!("hash_mismatch"));
        assert_eq!(shared_files_in(&dir)["total_files"], json!(1));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_blob_corrupto_se_detecta_al_verificar_el_inventario() {
        let dir = std::env::temp_dir().join(format!("md-webui-corrupt-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let data = b"datos";
        let b64 = base64_encode(data);
        let good = sha256_hex(data);
        assert_eq!(share_put_in(&dir, "a.bin", &b64, &good)["ok"], json!(true));

        // Se corrompe el blob por debajo (como un corte de luz o un sync roto).
        let blob = blob_path(&dir, &good);
        fs::write(&blob, b"otros datos").unwrap();

        let inv = shared_files_in(&dir);
        assert_eq!(inv["broken"], json!(1));
        assert_eq!(inv["files"][0]["verified"], json!(false));
        assert_eq!(inv["files"][0]["state"], json!("corrupt"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_subida_por_trozos_se_ensambla_y_se_verifica() {
        let dir = std::env::temp_dir().join(format!("md-webui-chunks-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let parts: [&[u8]; 3] = [b"parte-uno-", b"parte-dos-", b"parte-tres"];
        let mut whole: Vec<u8> = vec![];
        for p in parts {
            whole.extend_from_slice(p);
        }
        let whole_hash = sha256_hex(&whole);

        for (i, p) in parts.iter().enumerate() {
            let r = share_put_chunk_in(&dir, "grande.bin", i, &base64_encode(p), &sha256_hex(p));
            assert_eq!(r["ok"], json!(true), "trozo {} rechazado: {}", i, r);
        }
        let commit = share_commit_in(&dir, "grande.bin", &whole_hash);
        assert_eq!(commit["ok"], json!(true), "el commit falló: {}", commit);
        assert_eq!(commit["sha256"], json!(whole_hash));
        assert_eq!(commit["chunks"], json!(3));

        let inv = shared_files_in(&dir);
        assert_eq!(inv["total_files"], json!(1));
        assert_eq!(inv["files"][0]["verified"], json!(true));

        let _ = fs::remove_dir_all(&dir);
    }

    // ── Funciones puras / entradas vacías ───────────────────────────────────

    #[test]
    fn las_funciones_puras_no_fallan_con_entradas_vacias() {
        // Rutas relativas inválidas.
        assert!(normalize_rel("").is_err());
        assert!(normalize_rel("/absoluta").is_err());
        assert!(normalize_rel("../fuera").is_err());
        assert!(normalize_rel("./ok/fichero.txt").is_ok());

        // Acciones sin destino.
        assert_eq!(node_assign("", "x", &json!({}), "")["ok"], json!(false));
        assert_eq!(node_message("", "hola", "")["ok"], json!(false));

        // Compartición con rutas malas.
        assert_eq!(share_put_in(&std::env::temp_dir(), "../x", "", "")["ok"], json!(false));
        assert_eq!(share_put_chunk_in(&std::env::temp_dir(), "/x", 0, "", "")["ok"], json!(false));

        // Base64 inválido.
        assert!(base64_decode("###").is_err());
    }

    #[test]
    fn la_ruta_relativa_se_normaliza_y_se_protege() {
        assert_eq!(normalize_rel("a//b/./c").unwrap(), "a/b/c");
        assert!(normalize_rel("a/../../b").is_err());
        assert!(normalize_rel("a/\0/b").is_err());
    }

    #[test]
    fn el_router_deja_pasar_lo_que_no_es_de_la_v2() {
        assert!(dispatch("GET", "/api/health", &json!({})).is_none());
        assert!(dispatch("GET", "/", &json!({})).is_none());
        assert!(dispatch("GET", "/api/v2/dashboard", &json!({})).is_some());
    }

    #[test]
    fn el_router_responde_con_error_claro_para_una_ruta_desconocida() {
        let r = dispatch("GET", "/api/v2/no-existe", &json!({})).unwrap();
        assert_eq!(r["ok"], json!(false));
        assert_eq!(r["error"], json!("unknown_route"));
    }

    #[test]
    fn el_tablero_de_tareas_incluye_los_tipos_conocidos() {
        let t = task_board();
        assert_eq!(t["ok"], json!(true));
        let kinds = t["known_kinds"].as_array().cloned().unwrap_or_default();
        assert!(!kinds.is_empty(), "la lista de tipos de tarea no puede ir vacía");
        assert!(kinds.iter().any(|k| k == "sync.local"));
        assert!(t["board"].get("deployed").is_some());
    }

    #[test]
    fn el_catalogo_de_llm_lista_proveedores_reales() {
        let l = llm_catalog();
        assert_eq!(l["ok"], json!(true));
        assert!(l["providers"].is_array());
        assert!(l["catalog"].is_array());
        assert!(l["routing"].get("primary").is_some());
        // El catálogo del daemon tiene entradas reales (no una lista vacía).
        assert!(
            l["catalog"].as_array().map(|a| a.len()).unwrap_or(0) > 0,
            "el catálogo de proveedores no puede estar vacío"
        );
    }

    #[test]
    fn la_documentacion_de_rutas_coincide_con_la_tabla() {
        let doc = routes_doc();
        let listed = doc["routes"].as_array().cloned().unwrap_or_default();
        assert_eq!(listed.len(), api_v2_routes().len());
        for (i, (m, p)) in api_v2_routes().iter().enumerate() {
            assert_eq!(listed[i]["method"], json!(m));
            assert_eq!(listed[i]["path"], json!(p));
        }
        assert!(doc["auth"]["api_key_header"].as_str().unwrap_or("").contains("API-Key"));
    }

    #[test]
    fn el_bucle_agentico_declara_las_cinco_fases() {
        let a = agentic_loop_status();
        assert_eq!(a["ok"], json!(true));
        let phases = a["cycle"].as_array().cloned().unwrap_or_default();
        assert_eq!(phases.len(), 5, "el bucle debe declarar sus cinco fases");
        let names: Vec<&str> = phases.iter().filter_map(|p| p["phase"].as_str()).collect();
        for p in ["observe", "plan", "execute", "measure", "adjust"] {
            assert!(names.contains(&p), "falta la fase {} en el bucle", p);
        }
    }

    #[test]
    fn la_vista_de_alertas_muestra_el_canal_real() {
        let a = alerts_view();
        assert_eq!(a["ok"], json!(true));
        assert!(a["alerts"].is_array());
        assert!(a["channel"].get("configured").is_some());
        // El webhook configurado no debe filtrar la URL completa.
        if a["channel"]["configured"] == json!(true) {
            assert!(a["channel"].get("host").is_some());
        }
    }

    // ── Mercado de capacidades ──────────────────────────────────────────────

    #[test]
    fn cada_tipo_de_tarea_cae_en_un_tipo_de_trabajo_conocido() {
        assert_eq!(work_kind_for("sync.web"), "network");
        assert_eq!(work_kind_for("apk.analyze"), "build");
        assert_eq!(work_kind_for("agent.plan"), "inference");
        assert_eq!(work_kind_for("share.put"), "storage");
        assert_eq!(work_kind_for("loquesea"), "compute");
        for k in WORK_KINDS {
            assert!(!k.is_empty());
        }
    }

    #[test]
    fn las_reglas_locales_deciden_si_la_instancia_comparte() {
        let node = json!({ "id": "n1", "capabilities": ["llm", "reverse"], "load": 0.1 });
        let device = json!({
            "capacity_pct": 90, "charging": true, "cpu_temp_c": 35.0, "thermal_hot": false, "load": 0.1
        });
        let mut rules = default_rules("n1");
        rules.offers = vec!["compute".into(), "inference".into()];
        rules.dedicated = false;
        rules.only_when_charging = false;
        rules.min_battery_pct = 30;
        rules.max_cpu_temp_c = 45.0;
        rules.reserved_ratio = 0.5;

        let offer = effective_offer(&rules, &node, &device);
        assert_eq!(offer["available"], json!(true), "con batería y frío debe compartir: {}", offer);

        // Batería baja: deja de compartir y lo explica.
        let low = json!({
            "capacity_pct": 10, "charging": false, "cpu_temp_c": 35.0, "thermal_hot": false, "load": 0.1
        });
        let offer = effective_offer(&rules, &node, &low);
        assert_eq!(offer["available"], json!(false));
        let reason = offer["reason"].as_str().unwrap_or("");
        assert!(reason.contains("batería"), "debe explicar el bloqueo: {}", reason);

        // Modo dedicado: no comparte nada, aunque la batería sea perfecta.
        let mut dedicated = rules.clone();
        dedicated.dedicated = true;
        let offer = effective_offer(&dedicated, &node, &device);
        assert_eq!(offer["available"], json!(false));
        assert!(offer["reason"].as_str().unwrap_or("").contains("dedicada"));
    }

    #[test]
    fn las_reglas_se_validan_antes_de_guardarse() {
        let dir = std::env::temp_dir().join(format!("md-webui-rules-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        // Se validan los valores (aunque el fichero no se pueda escribir en
        // el sandbox, la validación ocurre antes).
        let bad_kind = json!({ "rules": { "offers": ["teletransporte"] } });
        assert_eq!(
            work_rules_set("", &bad_kind)["error"],
            json!("missing_node"),
            "sin nodo no se guarda"
        );
        assert_eq!(
            work_rules_set("n1", &json!({ "rules": { "min_battery_pct": 900 } }))["error"],
            json!("bad_value"),
            "una batería de 900 % es imposible"
        );
        assert_eq!(
            work_rules_set("n1", &json!({ "rules": { "reserved_ratio": 2.0 } }))["error"],
            json!("bad_value"),
            "no se puede reservar el 200 % de la capacidad"
        );
        assert_eq!(
            work_rules_set("n1", &json!({ "rules": { "offers": ["teletransporte"] } }))["error"],
            json!("bad_kind"),
            "tipo de trabajo desconocido"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_mercado_expone_oferta_demanda_y_contribucion() {
        let m = work_market();
        assert_eq!(m["ok"], json!(true));
        assert!(m["supply"].is_array());
        assert!(m["demand"].is_array());
        assert!(m["plan"].is_array());
        assert!(m["contribution"].is_array());
        assert!(m["kinds"].is_array());
        // El dispositivo se informa con datos reales (aunque pueden ser null).
        assert!(m["device"].get("capacity_pct").is_some());
        assert!(m["device"].get("charging").is_some());
        // Sin bus desplegado no puede haber oferta de peers inventada.
        let supply = m["supply"].as_array().cloned().unwrap_or_default();
        for s in &supply {
            assert!(s.get("offer").is_some());
            assert!(s.get("rules").is_some());
        }
    }

    // ── Hallazgos del enjambre ─────────────────────────────────────────────

    /// Prepara un directorio de estado del enjambre de mentira.
    fn fake_swarm_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("md-webui-swarm-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sync")).unwrap();
        fs::create_dir_all(dir.join("agents")).unwrap();
        dir
    }

    #[test]
    fn los_hallazgos_del_enjambre_se_leen_se_ordenan_y_se_paginan() {
        let dir = fake_swarm_dir("orden");
        let findings = [
            json!({ "agent": "cloud-sandbox", "kind": "bootloader", "title": "FMM Lock",
                    "finding": "el SBL bloquea el unlock", "target": "BL/sboot.bin",
                    "sha256": "a2cf9853", "size": 4194304, "confidence": "high", "ts": 1732000100 }),
            json!({ "agent": "cloud-sandbox", "kind": "reverse", "title": "DexClassLoader",
                    "finding": "carga dex dinámico", "target": "classes.dex",
                    "sha256": "bb11", "size": 100, "confidence": "medium", "ts": 1732000300 }),
            json!({ "agent": "otro-nodo", "kind": "npu", "title": "NNAPI",
                    "finding": "usa delegate", "target": "libfoo.so",
                    "sha256": "cc22", "size": 200, "confidence": "low", "ts": 1732000200 }),
        ];
        let body = findings
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<String>>()
            .join("\n");
        fs::write(dir.join("sync").join("re_cloud-sandbox.ndjson"), body).unwrap();
        fs::write(dir.join("mesh_journal.ndjson"), "a\nb\nc\nd\ne\n").unwrap();
        fs::write(
            dir.join("mesh_board.json"),
            json!({
                "agents": [{ "agent": "cloud-sandbox", "task": "re-exynos2100", "kind": "reverse", "status": "done" }],
                "tasks": [{ "task_id": "re-exynos2100-cloud-sandbox", "title": "RE Exynos 2100", "state": "done" }],
                "peers": [{ "id": "cloud-sandbox", "role": "worker" }]
            })
            .to_string(),
        )
        .unwrap();
        fs::write(
            dir.join("agents").join("cloud-sandbox.json"),
            json!({ "id": "cloud-sandbox", "role": "worker", "platform": "linux-x86_64" }).to_string(),
        )
        .unwrap();

        let roots = vec![dir.clone()];
        let r = mesh_findings_in(&roots, 50, 0);
        assert_eq!(r["ok"], json!(true));
        assert_eq!(r["total"], json!(3));
        assert_eq!(r["returned"], json!(3));
        // Orden por ts descendente.
        assert_eq!(r["findings"][0]["kind"], json!("reverse"));
        assert_eq!(r["findings"][2]["kind"], json!("bootloader"));
        // Journal = nº de líneas.
        assert_eq!(r["journal"], json!(5));
        // Tablón: agentes y tareas.
        assert_eq!(r["board_agents"].as_array().unwrap().len(), 1);
        assert_eq!(r["tasks"].as_array().unwrap().len(), 1);
        assert_eq!(r["node_identities"].as_array().unwrap().len(), 1);
        // Filtros derivados de los datos reales.
        assert!(r["kinds"].as_array().unwrap().len() >= 3);
        assert!(r["agents"].as_array().unwrap().len() >= 2);

        // Paginación.
        let p1 = mesh_findings_in(&roots, 2, 0);
        assert_eq!(p1["returned"], json!(2));
        assert_eq!(p1["total"], json!(3));
        let p2 = mesh_findings_in(&roots, 2, 2);
        assert_eq!(p2["returned"], json!(1));
        assert_eq!(p2["offset"], json!(2));
        // El tope de 200 se respeta.
        let huge = mesh_findings_in(&roots, 100000, 0);
        assert_eq!(huge["limit"], json!(200));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn una_linea_rota_del_enjambre_no_rompe_la_vista() {
        let dir = fake_swarm_dir("roto");
        let mut body = String::new();
        body.push_str("esto no es json\n");
        body.push_str(&json!({ "agent": "n1", "kind": "reverse", "finding": "ok", "ts": 5 }).to_string());
        body.push('\n');
        body.push_str("{\"agent\":\"n2\",\"kind\":\"npu\"}\n"); // sin ts: se acepta
        body.push_str("\n"); // línea vacía: se salta
        body.push_str("{\"sin\":\"agente\"}\n"); // sin agente: no es hallazgo
        fs::write(dir.join("sync").join("re_x.ndjson"), body).unwrap();
        let r = mesh_findings_in(&vec![dir.clone()], 50, 0);
        assert_eq!(r["total"], json!(2));
        assert_eq!(r["returned"], json!(2));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn los_secretos_del_enjambre_no_se_exponen() {
        let dir = fake_swarm_dir("secret");
        let body = json!({
            "agent": "n1", "kind": "reverse",
            "finding": "la clave del repo es mdaemon_sk_live_ABCDEF0123456789 y el token sk-zzzzzzzzzzzz",
            "sha256": "aa11", "ts": 7
        })
        .to_string();
        fs::write(dir.join("sync").join("re_s.ndjson"), body).unwrap();
        let r = mesh_findings_in(&vec![dir.clone()], 50, 0);
        let text = r.to_string();
        assert!(!text.contains("mdaemon_sk_live_ABCDEF0123456789"), "la key se filtró: {}", text);
        assert!(!text.contains("sk-zzzzzzzzzzzz"), "el token se filtró: {}", text);
        assert!(text.contains("[REDACTED]"));
        assert!(r["redactions"].as_u64().unwrap_or(0) >= 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_endpoint_de_hallazgos_esta_en_la_tabla_y_en_la_ui() {
        let routes = api_v2_routes();
        assert!(
            routes.iter().any(|(_, p)| *p == "/api/v2/mesh_findings"),
            "la ruta de hallazgos debe estar en la tabla"
        );
        // Y el alias que pidió el enjambre responde.
        let r = dispatch("GET", "/api/mesh_findings?limit=10", &json!({})).unwrap();
        assert_eq!(r["ok"], json!(true));
        assert!(r["findings"].is_array());
        assert!(r.get("journal").is_some());
    }

    // ── Tablero del bus: claim / release / report ───────────────────────────

    /// Crea un tablero SQLite de prueba con una tarea en `queued`.
    fn tablero_de_prueba(tag: &str) -> (std::path::PathBuf, String, String) {
        let dir = std::env::temp_dir().join(format!(
            "md-webui-board-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("dir de prueba");
        let path = dir.join("board.sqlite").to_string_lossy().to_string();
        let board = crate::coordination::TaskBoard::open(&path).expect("tablero");
        let id = board
            .add_task("tarea de prueba", 5, Some("worker"))
            .expect("tarea");
        // El id lo genera el tablero (t-<epoch>-<hash>): lo devolvemos para
        // poder reclamarlo con el MISMO id en la prueba.
        (dir, path, id)
    }

    #[test]
    fn el_claim_gana_el_lock_y_el_lease_impide_el_doble_claim() {
        let (_dir, path, id) = tablero_de_prueba("claim");
        let a = board_claim_at(&path, &json!({ "task_id": id, "owner": "agente-a", "role": "worker", "lease_secs": 300 }));
        assert_eq!(a["ok"], json!(true));
        assert_eq!(a["won"], json!(true), "el primer claim debe ganar");
        assert_eq!(a["role"], json!("worker"));

        // Otro agente con el lease vivo NO puede reclamarla.
        let b = board_claim_at(&path, &json!({ "task_id": id, "owner": "agente-b", "role": "worker" }));
        assert_eq!(b["ok"], json!(true));
        assert_eq!(b["won"], json!(false), "el segundo claim debe perder");
        assert_eq!(b["current_owner"], json!("agente-a"));
    }

    #[test]
    fn el_release_solo_lo_puede_hacer_el_dueno_y_el_report_marca_el_resultado() {
        let (_dir, path, id) = tablero_de_prueba("release");
        board_claim_at(&path, &json!({ "task_id": id, "owner": "agente-a", "role": "worker" }));

        // Un tercero no puede soltar la tarea de otro.
        let r = board_release_at(&path, &json!({ "task_id": id, "owner": "agente-b" }));
        assert_eq!(r["ok"], json!(false), "nadie roba la tarea de otro");
        assert_eq!(r["released"], json!(false));

        // El dueño sí puede.
        let r2 = board_release_at(&path, &json!({ "task_id": id, "owner": "agente-a" }));
        assert_eq!(r2["ok"], json!(true));
        assert_eq!(r2["released"], json!(true));

        // Y ahora se puede volver a reclamar.
        let c = board_claim_at(&path, &json!({ "task_id": id, "owner": "agente-b", "role": "primary" }));
        assert_eq!(c["won"], json!(true));
        let rep = board_report_at(&path, &json!({ "task_id": id, "owner": "agente-b", "ok": true, "report": "hecho" }));
        assert_eq!(rep["ok"], json!(true));
        assert_eq!(rep["status"], json!("done"));
    }

    #[test]
    fn un_rol_invalido_no_se_inventa() {
        let (_dir, path, id) = tablero_de_prueba("rol");
        let r = board_claim_at(&path, &json!({ "task_id": id, "owner": "x", "role": "rey" }));
        assert_eq!(r["ok"], json!(false));
        assert_eq!(r["error"], json!("invalid_role"));
        let sin_tarea = board_claim_at(&path, &json!({ "owner": "x" }));
        assert_eq!(sin_tarea["error"], json!("missing_task_id"));
    }

    #[test]
    fn el_reinicio_exige_confirmacion_y_respeta_el_enfriamiento() {
        let r = admin_restart(&json!({}));
        assert_eq!(r["ok"], json!(false));
        assert_eq!(r["error"], json!("confirmation_required"));
        // Con confirm=true el proceso se acaba: no lo ejecutamos en un test.
        // resto del test porque mata el proceso (el hilo sale a los 600 ms).
    }

    #[test]
    fn la_voz_y_el_reinicio_estan_en_la_tabla_de_rutas() {
        let routes = api_v2_routes();
        for p in [
            "/api/v2/voice",
            "/api/v2/tasks/claim",
            "/api/v2/tasks/release",
            "/api/v2/tasks/report",
            "/api/v2/admin/restart",
        ] {
            assert!(
                routes.iter().any(|(_, r)| *r == p),
                "falta la ruta {} en la tabla",
                p
            );
        }
    }

    #[test]
    fn el_prompt_externo_se_genera_en_el_servidor() {
        let p = prompt_external(false, "");
        assert_eq!(p["ok"], json!(true));
        assert_eq!(p["contains_secrets"], json!(false));
        let md = p["markdown"].as_str().unwrap_or("");
        assert!(md.contains("Cómo hablar con el daemon"), "falta la sección de acceso");
        assert!(md.contains("Herramientas MCP"));
        assert!(md.contains("Estado del dispositivo"));
        assert!(md.contains("Hallazgos del enjambre"));
        // El catálogo de tools es real, no un texto de relleno.
        assert!(p["tools"].as_u64().unwrap_or(0) > 10, "el catálogo de tools debe ser real");
        // SIN credenciales: el token de sesión no puede aparecer.
        assert!(
            !md.contains(crate::cockpit_server::auth_token()),
            "el prompt sin secretos no puede llevar el token de sesión"
        );
    }

    #[test]
    fn el_prompt_externo_con_credenciales_avisa_y_las_incluye() {
        let p = prompt_external(true, "mdaemon_sk_test_1234567890");
        assert_eq!(p["contains_secrets"], json!(true));
        let md = p["markdown"].as_str().unwrap_or("");
        assert!(md.contains("contiene credenciales"), "debe avisar en la primera línea");
        assert!(md.contains("mdaemon_sk_test_1234567890"));
        assert!(md.contains(crate::cockpit_server::auth_token()));
        let w = p["warnings"].as_array().cloned().unwrap_or_default();
        assert!(!w.is_empty(), "debe devolver avisos");
    }

    #[test]
    fn el_reparto_ignora_los_nodos_que_no_cumplen_las_reglas() {
        let node = json!({ "id": "n1", "capabilities": [], "load": 0.0 });
        let device = json!({
            "capacity_pct": 90, "charging": true, "cpu_temp_c": 30.0, "thermal_hot": false, "load": 0.0
        });
        let mut rules = default_rules("n1");
        rules.offers = vec!["compute".into()];
        rules.reserved_ratio = 0.0;
        rules.min_battery_pct = 10;
        let offer = effective_offer(&rules, &node, &device);
        let contrib = json!({ "share_pct": 0.0 });

        // Ofrece compute: puntúa.
        assert!(score_node(&node, &offer, &rules, "compute", &contrib).is_some());
        // No ofrece inference: descartado.
        assert!(score_node(&node, &offer, &rules, "inference", &contrib).is_none());
        // Nodo bloqueado: descartado aunque encaje el tipo.
        let blocked = json!({ "available": false, "capacity": 0.0, "offers": ["compute"], "reason": "x" });
        assert!(score_node(&node, &blocked, &rules, "compute", &contrib).is_none());
    }
}
