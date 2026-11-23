//! Instrumentación de las **sesiones de Antigravity** en el móvil.
//!
//! MEDIDO (2026-11-22, S21 Ultra): Antigravity guarda el índice de sus
//! conversaciones en
//! `/data/user/0/com.agy/files/.gemini/antigravity-app/conversation_summaries.db`
//! (SQLite, tabla `conversation_summaries`), cada conversación en
//! `conversations/<id>.db`, los transcripts en
//! `brain/<id>/.system_generated/logs/transcript.jsonl` y el daemon interno en
//! `daemon/tasks.json` y `daemon/daemon.log`.
//!
//! Todo es **solo lectura** (SQLite `SQLITE_OPEN_READ_ONLY` + `busy_timeout`):
//! nunca bloquea ni muta el estado vivo de la app.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Directorio de datos de Antigravity en el móvil.
pub const APP_DIR: &str = "/data/user/0/com.agy/files/.gemini/antigravity-app";
/// Índice de conversaciones.
pub const DB_SUMMARIES: &str =
    "/data/user/0/com.agy/files/.gemini/antigravity-app/conversation_summaries.db";
/// Fichero de estado del daemon interno de Antigravity (el planificador).
pub const DAEMON_TASKS: &str =
    "/data/user/0/com.agy/files/.gemini/antigravity-app/daemon/tasks.json";
/// Log del daemon interno.
pub const DAEMON_LOG: &str =
    "/data/user/0/com.agy/files/.gemini/antigravity-app/daemon/daemon.log";

/// Directorio activo de Antigravity (`MEGADAEMON_ANTIGRAVITY_DIR` o `APP_DIR`).
pub fn default_app_dir() -> PathBuf {
    match std::env::var("MEGADAEMON_ANTIGRAVITY_DIR") {
        Ok(v) if !v.trim().is_empty() => PathBuf::from(v.trim()),
        _ => PathBuf::from(APP_DIR),
    }
}

/// Una conversación (o sub-agente) de Antigravity.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResumenSesion {
    pub conversation_id: String,
    pub titulo: String,
    pub preview: String,
    pub pasos: i64,
    pub ultima_modificacion: String,
    pub workspace: String,
    pub estado: String,
    pub agente: String,
    pub padre: Option<String>,
    pub profundidad: i64,
    /// 1 = el sub-agente fue matado.
    pub killed: i64,
    /// 1 = sigue con trabajo pendiente.
    pub no_idle: i64,
    pub battle_id: Option<String>,
    pub ganadora: Option<String>,
    pub ultimo_input: String,
    pub ultimo_input_paso: i64,
    /// Estado **normalizado** (EN_CURSO / IDLE / COLGADA / MUERTA).
    ///
    /// MEDIDO (2026-11-23): el estado crudo (`CASCADE_RUN_STATUS_*`) obliga
    /// a cada consumidor a reimplementar las reglas; aquí se decide una vez.
    #[serde(default)]
    pub estado_norm: String,
    /// Tokens de la conversación (0 si la DB no los trae o no se pudo leer).
    #[serde(default)]
    pub tokens: i64,
    /// Modelo en uso (cadena vacía si no se pudo determinar).
    #[serde(default)]
    pub modelo: String,
}

/// Resumen agregado del estado de las sesiones.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EstadoSesiones {
    pub total: usize,
    pub en_curso: usize,
    pub idle: usize,
    pub muertas: usize,
    pub con_padre: usize,
    /// Sesiones que llevan más de 24 h sin modificarse.
    pub inactivas_24h: usize,
    /// Sesiones colgadas (`killed == 1`, `no_idle == 1` en estado IDLE, o activas > 24 h).
    pub colgadas: usize,
    /// La conversación más recientemente tocada.
    pub ultima: Option<ResumenSesion>,
    pub db_legible: bool,
    pub detalle: String,
}

fn non_empty_opt(v: Option<String>) -> Option<String> {
    v.and_then(|s| {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    })
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Parsea `YYYY-MM-DD[T ]HH:MM:SS...` (UTC) a segundos Unix sin dependencias externas.
pub fn parse_iso_utc_secs(ts: &str) -> Option<u64> {
    let s = ts.trim();
    if s.len() < 19 {
        return None;
    }
    let b = s.as_bytes();
    if b[4] != b'-' || b[7] != b'-' || (b[10] != b'T' && b[10] != b' ') || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let year: i64 = s[0..4].parse().ok()?;
    let month: u32 = s[5..7].parse().ok()?;
    let day: u32 = s[8..10].parse().ok()?;
    let hour: u64 = s[11..13].parse().ok()?;
    let min: u64 = s[14..16].parse().ok()?;
    let sec: u64 = s[17..19].parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || min > 59 || sec > 59 {
        return None;
    }
    // Algoritmo de Howard Hinnant (días desde 1970-01-01).
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = month as i64;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    if days < 0 {
        return None;
    }
    Some((days as u64) * 86_400 + hour * 3_600 + min * 60 + sec)
}

fn es_inactiva_24h(s: &ResumenSesion, now_secs: u64) -> bool {
    parse_iso_utc_secs(&s.ultima_modificacion)
        .map(|ts| now_secs.saturating_sub(ts) > 86_400)
        .unwrap_or(false)
}

/// Estado normalizado de una sesión.
///
/// Las reglas (MEDIDAS sobre el estado crudo `CASCADE_RUN_STATUS_*`):
/// * `killed == 1`               ⇒ **MUERTA** (el sub-agente fue matado).
/// * `IDLE` + `not_fully_idle=1` ⇒ **COLGADA** (dice idle pero tiene trabajo).
/// * `IDLE`                      ⇒ **IDLE** (de verdad parado).
/// * activa y sin tocar en 24 h   ⇒ **COLGADA** (zombie).
/// * resto                       ⇒ **EN_CURSO**.
pub fn estado_normalizado(estado: &str, killed: i64, no_idle: i64, inactiva_24h: bool) -> &'static str {
    if killed == 1 {
        return "MUERTA";
    }
    let idle = estado.contains("IDLE");
    if idle {
        if no_idle == 1 {
            return "COLGADA";
        }
        return "IDLE";
    }
    if inactiva_24h {
        return "COLGADA";
    }
    "EN_CURSO"
}

fn es_colgada(s: &ResumenSesion, now_secs: u64) -> bool {
    let is_idle = s.estado.contains("IDLE");
    s.killed == 1 || (is_idle && s.no_idle == 1) || (!is_idle && es_inactiva_24h(s, now_secs))
}

fn open_readonly(db_path: &Path) -> Result<rusqlite::Connection, String> {
    let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
        | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let con = rusqlite::Connection::open_with_flags(db_path, flags)
        .map_err(|e| format!("sqlite({}): {e}", db_path.display()))?;
    let _ = con.busy_timeout(Duration::from_millis(2000));
    Ok(con)
}

/// Lee el índice de conversaciones desde `app_dir/conversation_summaries.db`.
pub fn sesiones_en(app_dir: &Path) -> Result<Vec<ResumenSesion>, String> {
    let db_path = app_dir.join("conversation_summaries.db");
    if !db_path.exists() {
        return Ok(Vec::new());
    }
    let con = open_readonly(&db_path)?;
    let mut st = con
        .prepare(
            "SELECT conversation_id, title, preview, step_count, last_modified_time, \
             workspace_uris, status, agent_name, parent_conversation_id, nesting_depth, \
             killed, not_fully_idle, battle_id, winning_conversation_id, \
             last_user_input_time, last_user_input_step_index \
             FROM conversation_summaries ORDER BY last_modified_time DESC",
        )
        .map_err(|e| format!("prepare: {e}"))?;
    let filas = st
        .query_map([], |r| {
            Ok(ResumenSesion {
                conversation_id: r.get::<_, String>(0).unwrap_or_default(),
                titulo: r.get::<_, Option<String>>(1).unwrap_or_default().unwrap_or_default(),
                preview: r.get::<_, Option<String>>(2).unwrap_or_default().unwrap_or_default(),
                pasos: r.get::<_, Option<i64>>(3).unwrap_or_default().unwrap_or(0),
                ultima_modificacion: r
                    .get::<_, Option<String>>(4)
                    .unwrap_or_default()
                    .unwrap_or_default(),
                workspace: r
                    .get::<_, Option<String>>(5)
                    .unwrap_or_default()
                    .unwrap_or_default(),
                estado: r.get::<_, Option<String>>(6).unwrap_or_default().unwrap_or_default(),
                agente: r.get::<_, Option<String>>(7).unwrap_or_default().unwrap_or_default(),
                padre: non_empty_opt(r.get::<_, Option<String>>(8).unwrap_or_default()),
                profundidad: r.get::<_, Option<i64>>(9).unwrap_or_default().unwrap_or(0),
                killed: r.get::<_, Option<i64>>(10).unwrap_or_default().unwrap_or(0),
                no_idle: r.get::<_, Option<i64>>(11).unwrap_or_default().unwrap_or(0),
                battle_id: non_empty_opt(r.get::<_, Option<String>>(12).unwrap_or_default()),
                ganadora: non_empty_opt(r.get::<_, Option<String>>(13).unwrap_or_default()),
                ultimo_input: r
                    .get::<_, Option<String>>(14)
                    .unwrap_or_default()
                    .unwrap_or_default(),
                ultimo_input_paso: r.get::<_, Option<i64>>(15).unwrap_or_default().unwrap_or(-1),
                estado_norm: String::new(),
                tokens: 0,
                modelo: String::new(),
            })
        })
        .map_err(|e| format!("query: {e}"))?;
    let mut out = Vec::new();
    let ahora = now_unix_secs();
    for f in filas.flatten() {
        let mut f = f;
        f.estado_norm = estado_normalizado(
            &f.estado,
            f.killed,
            f.no_idle,
            es_inactiva_24h(&f, ahora),
        )
        .to_string();
        out.push(f);
    }
    Ok(out)
}

/// Lee el índice de conversaciones del directorio por defecto.
pub fn sesiones() -> Result<Vec<ResumenSesion>, String> {
    sesiones_en(&default_app_dir())
}

/// Estado agregado de las sesiones en `app_dir`.
pub fn estado_en(app_dir: &Path) -> Result<EstadoSesiones, String> {
    let lista = sesiones_en(app_dir)?;
    let now = now_unix_secs();
    let mut e = EstadoSesiones {
        total: lista.len(),
        db_legible: app_dir.join("conversation_summaries.db").exists(),
        ..Default::default()
    };
    for s in &lista {
        if s.estado.contains("IDLE") {
            e.idle += 1;
        } else {
            e.en_curso += 1;
        }
        if s.killed == 1 {
            e.muertas += 1;
        }
        if s.padre.is_some() {
            e.con_padre += 1;
        }
        if es_inactiva_24h(s, now) {
            e.inactivas_24h += 1;
        }
        if es_colgada(s, now) {
            e.colgadas += 1;
        }
    }
    e.ultima = lista.first().cloned();
    e.detalle = format!(
        "{} conversaciones: {} en curso, {} idle, {} muertas, {} colgadas, {} con padre, {} inactivas >24h",
        e.total, e.en_curso, e.idle, e.muertas, e.colgadas, e.con_padre, e.inactivas_24h
    );
    Ok(e)
}

/// Estado agregado de las sesiones del directorio por defecto.
pub fn estado() -> Result<EstadoSesiones, String> {
    estado_en(&default_app_dir())
}

/// Lista las sesiones **muertas** (`killed == 1`) en `app_dir`.
pub fn muertas_en(app_dir: &Path) -> Result<Vec<ResumenSesion>, String> {
    Ok(sesiones_en(app_dir)?
        .into_iter()
        .filter(|s| s.killed == 1)
        .collect())
}

/// Lista las sesiones **muertas** (sub-agentes matados) con su padre.
pub fn muertas() -> Result<Vec<ResumenSesion>, String> {
    muertas_en(&default_app_dir())
}

/// Lista las sesiones **colgadas** (`killed == 1`, `no_idle == 1` en IDLE, o activas > 24 h).
pub fn colgadas_en(app_dir: &Path) -> Result<Vec<ResumenSesion>, String> {
    let now = now_unix_secs();
    Ok(sesiones_en(app_dir)?
        .into_iter()
        .filter(|s| es_colgada(s, now))
        .collect())
}

/// Lista las sesiones **colgadas** en el directorio por defecto.
pub fn colgadas() -> Result<Vec<ResumenSesion>, String> {
    colgadas_en(&default_app_dir())
}

fn build_tree_node(
    id: &str,
    by_id: &HashMap<&str, &ResumenSesion>,
    children_map: &HashMap<&str, Vec<&str>>,
    seen: &mut HashSet<String>,
) -> Value {
    if !seen.insert(id.to_string()) {
        return json!({ "conversation_id": id, "cycle": true });
    }
    let children: Vec<Value> = children_map
        .get(id)
        .map(|kids| {
            kids.iter()
                .map(|k| build_tree_node(k, by_id, children_map, seen))
                .collect()
        })
        .unwrap_or_default();
    match by_id.get(id) {
        Some(s) => json!({
            "conversation_id": s.conversation_id,
            "titulo": s.titulo,
            "estado": s.estado,
            "agente": s.agente,
            "pasos": s.pasos,
            "killed": s.killed,
            "no_idle": s.no_idle,
            "profundidad": s.profundidad,
            "ultima_modificacion": s.ultima_modificacion,
            "hijos": children,
        }),
        None => json!({ "conversation_id": id, "hijos": children }),
    }
}

/// Construye el árbol padre -> sub-agentes (todas las raíces o una conversación concreta).
pub fn arbol_en(app_dir: &Path, root_id: Option<&str>) -> Result<Value, String> {
    let lista = sesiones_en(app_dir)?;
    let by_id: HashMap<&str, &ResumenSesion> = lista
        .iter()
        .map(|s| (s.conversation_id.as_str(), s))
        .collect();
    let mut children_map: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut roots: Vec<&str> = Vec::new();
    for s in &lista {
        if let Some(ref p) = s.padre {
            if by_id.contains_key(p.as_str()) {
                children_map
                    .entry(p.as_str())
                    .or_default()
                    .push(s.conversation_id.as_str());
                continue;
            }
        }
        roots.push(s.conversation_id.as_str());
    }
    let mut seen = HashSet::new();
    if let Some(rid) = root_id.filter(|r| !r.trim().is_empty()) {
        if !by_id.contains_key(rid) {
            return Err(format!("conversación no encontrada: {rid}"));
        }
        return Ok(build_tree_node(rid, &by_id, &children_map, &mut seen));
    }
    let trees: Vec<Value> = roots
        .into_iter()
        .map(|r| build_tree_node(r, &by_id, &children_map, &mut seen))
        .collect();
    Ok(json!({ "total": lista.len(), "raices": trees }))
}

/// Construye el árbol de sesiones en el directorio por defecto.
pub fn arbol(root_id: Option<&str>) -> Result<Value, String> {
    arbol_en(&default_app_dir(), root_id)
}

fn read_transcript_tail(path: &Path, tail: usize) -> (usize, Vec<Value>) {
    let Ok(f) = File::open(path) else {
        return (0, Vec::new());
    };
    let reader = BufReader::new(f);
    let mut total = 0usize;
    let mut ring: std::collections::VecDeque<Value> = std::collections::VecDeque::with_capacity(tail.max(1));
    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        total += 1;
        if tail == 0 {
            continue;
        }
        let item = match serde_json::from_str::<Value>(trimmed) {
            Ok(v) => {
                let preview = v
                    .get("content")
                    .and_then(|c| c.as_str())
                    .map(|s| s.chars().take(160).collect::<String>())
                    .unwrap_or_default();
                json!({
                    "step_index": v.get("step_index").cloned().unwrap_or(Value::Null),
                    "type": v.get("type").cloned().unwrap_or(Value::Null),
                    "source": v.get("source").cloned().unwrap_or(Value::Null),
                    "status": v.get("status").cloned().unwrap_or(Value::Null),
                    "created_at": v.get("created_at").cloned().unwrap_or(Value::Null),
                    "content_preview": preview,
                })
            }
            Err(_) => json!({ "raw": trimmed.chars().take(160).collect::<String>() }),
        };
        if ring.len() == tail {
            ring.pop_front();
        }
        ring.push_back(item);
    }
    (total, ring.into_iter().collect())
}

/// Detalle completo de una sesión: resumen, sub-agentes hijos, `conversations/<id>.db` y cola del transcript.
pub fn detalle_sesion_en(app_dir: &Path, conversation_id: &str, tail: usize) -> Result<Value, String> {
    let id = conversation_id.trim();
    if id.is_empty() {
        return Err("conversation_id vacío".to_string());
    }
    let lista = sesiones_en(app_dir)?;
    let resumen = lista
        .iter()
        .find(|s| s.conversation_id == id)
        .cloned()
        .ok_or_else(|| format!("sesión no encontrada: {id}"))?;
    let hijos: Vec<ResumenSesion> = lista
        .into_iter()
        .filter(|s| s.padre.as_deref() == Some(id))
        .collect();

    let conv_db = app_dir.join("conversations").join(format!("{id}.db"));
    let conv_db_bytes = std::fs::metadata(&conv_db).map(|m| m.len()).ok();
    let transcript_path = app_dir
        .join("brain")
        .join(id)
        .join(".system_generated/logs/transcript.jsonl");
    let (transcript_lines, transcript_tail) = read_transcript_tail(&transcript_path, tail);

    Ok(json!({
        "resumen": resumen,
        "hijos": hijos,
        "conversation_db": {
            "exists": conv_db_bytes.is_some(),
            "path": conv_db.display().to_string(),
            "bytes": conv_db_bytes,
        },
        "transcript": {
            "exists": transcript_path.exists(),
            "path": transcript_path.display().to_string(),
            "total_steps": transcript_lines,
            "tail": transcript_tail,
        },
        // v16 (2026-11-23): tokens y modelo en uso.  El esquema de la DB
        // de la conversacion NO esta medido (movil caido) => el lector es
        // defensivo y lo dice.
        "tokens_y_modelo": tokens_y_modelo_en(app_dir, id),
        // La traza del shim que cae dentro de la vida de la conversion.
        "shim": correlacion_shim_en(app_dir, 200).unwrap_or(Value::Null),
    }))
}

/// Detalle completo de una sesión en el directorio por defecto.
pub fn detalle_sesion(conversation_id: &str, tail: usize) -> Result<Value, String> {
    detalle_sesion_en(&default_app_dir(), conversation_id, tail)
}

// =========================================================================
// TOKENS, MODELO EN USO Y CORRELACIÓN CON EL NDJSON DEL SHIM
// =========================================================================
//
// MEDIDO (2026-11-23): el detalle de sesión decía el estado, los pasos y
// el transcript, pero NO decía **cuántos tokens** lleva la conversación ni
// **qué modelo** está contestando, que es justo lo que hace falta para
// "leer el estado real de Antigravity sin bloquearla".
//
// El esquema de `conversations/<id>.db` **NO se ha podido medir** (el móvil
// está caído desde el 2026-11-23) ⇒ el lector es **defensivo**: lista lo que
// encuentra, suma lo que parece tokens y NO falla si el esquema es otro.
// Lo que no se pudo leer se marca y se dice el comando exacto de
// verificación en `docs/MITM-SQLITE-Y-PROXY.md`.

/// Ruta del NDJSON del shim (el registro público del tráfico).
pub fn ruta_ndjson_shim() -> PathBuf {
    if let Ok(v) = std::env::var("MEGADAEMON_AGYSHIM_NDJSON") {
        let t = v.trim();
        if !t.is_empty() {
            return PathBuf::from(t);
        }
    }
    PathBuf::from(crate::cockpit_server::state_dir()).join("agyshim.ndjson")
}

/// Tokens y modelo en uso de una conversación.
///
/// Lee `conversations/<id>.db` en **SOLO LECTURA** con `busy_timeout`:
/// nunca bloquea a Antigravity.
///
/// Devuelve además el **esquema** que ha encontrado (tablas y columnas):
/// es lo que permite descubrir la forma real de la DB en el dispositivo
/// sin tener que inventarla.
pub fn tokens_y_modelo_en(app_dir: &Path, conversation_id: &str) -> Value {
    let id = conversation_id.trim();
    if id.is_empty() {
        return json!({ "ok": false, "motivo": "conversation_id vacio" });
    }
    let db = app_dir.join("conversations").join(format!("{id}.db"));
    if !db.exists() {
        return json!({
            "ok": false,
            "motivo": format!("no existe {}", db.display()),
            "tokens": 0,
            "modelo": "",
        });
    }
    let con = match open_readonly(&db) {
        Ok(c) => c,
        Err(e) => {
            return json!({ "ok": false, "motivo": e, "tokens": 0, "modelo": "" });
        }
    };

    // 1) Tablas
    let tablas: Vec<String> = match con.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    ) {
        Ok(mut st) => match st.query_map([], |r| r.get::<_, String>(0)) {
            Ok(f) => f.flatten().collect(),
            Err(e) => return json!({ "ok": false, "motivo": format!("query tablas: {e}") }),
        },
        Err(e) => return json!({ "ok": false, "motivo": format!("prepare tablas: {e}") }),
    };

    // 2) Columnas de cada tabla (y qué parece token / qué parece modelo)
    let mut esquema = Vec::new();
    let mut tokens = 0i64;
    let mut modelo = String::new();
    for t in &tablas {
        let cols: Vec<String> = match con.prepare(&format!("PRAGMA table_info(\"{t}\")")) {
            Ok(mut st) => match st.query_map([], |r| r.get::<_, String>(1)) {
                Ok(f) => f.flatten().collect(),
                Err(_) => Vec::new(),
            },
            Err(_) => Vec::new(),
        };
        let col_token: Vec<&String> = cols
            .iter()
            .filter(|c| {
                let l = c.to_lowercase();
                l.contains("token") || l.contains("usage")
            })
            .collect();
        let col_modelo: Vec<&String> = cols
            .iter()
            .filter(|c| c.to_lowercase().contains("model"))
            .collect();
        esquema.push(json!({
            "tabla": t,
            "columnas": cols,
            "columnas_token": col_token,
            "columnas_modelo": col_modelo,
        }));

        // Tokens: suma de las columnas enteras que parecen tokens.
        for c in &col_token {
            let sql = format!("SELECT COALESCE(SUM(CAST(\"{c}\" AS INTEGER)),0) FROM \"{t}\"");
            if let Ok(v) = con.query_row(&sql, [], |r| r.get::<_, i64>(0)) {
                tokens += v;
            }
        }
        // Modelo: el último valor no vacío de las columnas que parecen modelo.
        for c in &col_modelo {
            let sql = format!(
                "SELECT \"{c}\" FROM \"{t}\" WHERE \"{c}\" IS NOT NULL AND \"{c}\" <> '' LIMIT 1"
            );
            if let Ok(v) = con.query_row(&sql, [], |r| r.get::<_, String>(0)) {
                if !v.trim().is_empty() {
                    modelo = v.trim().to_string();
                    break;
                }
            }
        }
    }

    json!({
        "ok": true,
        "conversation_id": id,
        "db": db.display().to_string(),
        "tokens": tokens,
        "modelo": modelo,
        "esquema": esquema,
        "modo": "SOLO LECTURA con busy_timeout 2000 ms",
        "aviso": "el esquema de conversations/<id>.db NO esta medido (movil caido): los tokens son la suma de las columnas que PARECEN de tokens y el modelo, el de las que PARECEN de modelo.",
    })
}

/// Tokens y modelo en uso en el directorio por defecto.
pub fn tokens_y_modelo(conversation_id: &str) -> Value {
    tokens_y_modelo_en(&default_app_dir(), conversation_id)
}

/// Correlación entre las sesiones de Antigravity y el NDJSON del shim.
///
/// El NDJSON del shim tiene `ts` (unix) y el modelo pedido/real; las
/// sesiones tienen `last_user_input_time` y `last_modified_time` (RFC3339).
/// Se cruzan por **ventana temporal**: los entries del shim que caen dentro
/// de la vida de la conversación.
///
/// No bloquea nada: los dos ficheros se leen del disco.
pub fn correlacion_shim_en(app_dir: &Path, tail: usize) -> Result<Value, String> {
    correlacion_shim_de(app_dir, &ruta_ndjson_shim(), tail)
}

/// Correlación con un NDJSON **concreto** (testeable sin el móvil).
pub fn correlacion_shim_de(
    app_dir: &Path,
    ndjson: &Path,
    tail: usize,
) -> Result<Value, String> {
    let Ok(txt) = std::fs::read_to_string(ndjson) else {
        return Ok(json!({
            "ok": true,
            "entradas": 0,
            "nota": "todavia no hay NDJSON del shim (o no se puede leer)",
            "ruta_ndjson": ndjson.display().to_string(),
        }));
    };
    let entradas: Vec<Value> = txt
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect();
    let lista = sesiones_en(app_dir)?;

    let mut por_sesion = Vec::new();
    for s in &lista {
        let fin = parse_iso_utc_secs(&s.ultima_modificacion).unwrap_or(0);
        let ini = parse_iso_utc_secs(&s.ultimo_input).unwrap_or(fin.saturating_sub(3600));
        let dentro: Vec<&Value> = entradas
            .iter()
            .filter(|e| {
                let ts = e.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);
                ts >= ini && ts <= fin.max(ini)
            })
            .collect();
        if dentro.is_empty() {
            continue;
        }
        let mut pedidos: Vec<String> = Vec::new();
        let mut reales: Vec<String> = Vec::new();
        let mut sustituciones = 0u64;
        for e in &dentro {
            if let Some(m) = e.get("requested_model").and_then(|v| v.as_str()) {
                if !pedidos.contains(&m.to_string()) {
                    pedidos.push(m.to_string());
                }
            }
            if let Some(m) = e.get("real_model").and_then(|v| v.as_str()) {
                if !reales.contains(&m.to_string()) {
                    reales.push(m.to_string());
                }
            }
            if e.get("substituted").and_then(|v| v.as_bool()).unwrap_or(false) {
                sustituciones += 1;
            }
        }
        por_sesion.push(json!({
            "conversation_id": s.conversation_id,
            "titulo": s.titulo,
            "estado": s.estado,
            "estado_norm": s.estado_norm,
            "ventana": { "desde": ini, "hasta": fin },
            "entradas_shim": dentro.len(),
            "modelos_pedidos": pedidos,
            "modelos_reales": reales,
            "sustituciones": sustituciones,
        }));
    }

    Ok(json!({
        "ok": true,
        "ruta_ndjson": ndjson.display().to_string(),
        "entradas_totales": entradas.len(),
        "tail": tail,
        "sesiones_con_traza": por_sesion.len(),
        "correlacion": por_sesion,
    }))
}

/// Correlación con el NDJSON del shim en el directorio por defecto.
pub fn correlacion_shim(tail: usize) -> Result<Value, String> {
    correlacion_shim_en(&default_app_dir(), tail)
}

/// Estado del planificador interno de Antigravity (`daemon/tasks.json`).
pub fn tareas_daemon_en(app_dir: &Path) -> Result<Value, String> {
    let p = app_dir.join("daemon/tasks.json");
    let texto = std::fs::read_to_string(&p).map_err(|e| format!("leer tasks.json: {e}"))?;
    serde_json::from_str(&texto).map_err(|e| format!("tasks.json no es JSON: {e}"))
}

/// Estado del planificador interno de Antigravity en el directorio por defecto.
pub fn tareas_daemon() -> Result<Value, String> {
    tareas_daemon_en(&default_app_dir())
}

/// Últimas `tail` líneas de `daemon/daemon.log`.
pub fn daemon_log_en(app_dir: &Path, tail: usize) -> Result<Vec<String>, String> {
    let p = app_dir.join("daemon/daemon.log");
    let f = File::open(&p).map_err(|e| format!("leer daemon.log: {e}"))?;
    let limit = tail.max(1);
    let mut ring: std::collections::VecDeque<String> = std::collections::VecDeque::with_capacity(limit);
    for line in BufReader::new(f).lines().map_while(Result::ok) {
        if ring.len() == limit {
            ring.pop_front();
        }
        ring.push_back(line);
    }
    Ok(ring.into_iter().collect())
}

/// Últimas `tail` líneas de `daemon/daemon.log` en el directorio por defecto.
pub fn daemon_log(tail: usize) -> Result<Vec<String>, String> {
    daemon_log_en(&default_app_dir(), tail)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_app_dir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "md_agy_sessions_{}_{}_{}",
            tag,
            std::process::id(),
            now_unix_secs()
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(p.join("daemon")).unwrap();
        p
    }

    fn seed_db(app_dir: &Path) {
        let db_path = app_dir.join("conversation_summaries.db");
        let con = rusqlite::Connection::open(&db_path).unwrap();
        con.execute_batch(
            "CREATE TABLE conversation_summaries (
                conversation_id text PRIMARY KEY,
                title text NOT NULL DEFAULT '',
                preview text NOT NULL DEFAULT '',
                step_count integer NOT NULL DEFAULT 0,
                last_modified_time datetime NOT NULL,
                workspace_uris text NOT NULL DEFAULT '[]',
                status text NOT NULL DEFAULT '',
                source text NOT NULL DEFAULT '',
                project_id text NOT NULL DEFAULT '',
                agent_name text NOT NULL DEFAULT '',
                parent_conversation_id text NOT NULL DEFAULT '',
                nesting_depth integer NOT NULL DEFAULT 0,
                battle_id text NOT NULL DEFAULT '',
                winning_conversation_id text NOT NULL DEFAULT '',
                not_fully_idle numeric NOT NULL DEFAULT false,
                killed numeric NOT NULL DEFAULT false,
                last_user_input_time datetime NOT NULL DEFAULT '',
                last_user_input_step_index integer NOT NULL DEFAULT -1
            );",
        )
        .unwrap();

        // Sesión raíz reciente en curso (con parent_conversation_id = '' como en la DB real).
        con.execute(
            "INSERT INTO conversation_summaries (conversation_id, title, step_count, last_modified_time, status, agent_name, parent_conversation_id, nesting_depth, not_fully_idle, killed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, '', 0, 1, 0)",
            rusqlite::params!["root-1", "Principal", 42, "2099-01-01 10:00:00+00:00", "CASCADE_RUN_STATUS_RUNNING", ""],
        )
        .unwrap();
        // Sub-agente muerto (killed=1) hijo de root-1.
        con.execute(
            "INSERT INTO conversation_summaries (conversation_id, title, step_count, last_modified_time, status, agent_name, parent_conversation_id, nesting_depth, not_fully_idle, killed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, 0, 1)",
            rusqlite::params!["child-dead", "Subagente colgado", 180, "2020-01-01 00:00:00+00:00", "CASCADE_RUN_STATUS_IDLE", "self", "root-1"],
        )
        .unwrap();
        // Sesión antigua >24h marcada como IDLE pero con not_fully_idle=1.
        con.execute(
            "INSERT INTO conversation_summaries (conversation_id, title, step_count, last_modified_time, status, agent_name, parent_conversation_id, nesting_depth, not_fully_idle, killed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, '', 0, 1, 0)",
            rusqlite::params!["root-stuck", "Idle con trabajo", 10, "2020-05-05 12:00:00+00:00", "CASCADE_RUN_STATUS_IDLE", "",],
        )
        .unwrap();
    }

    #[test]
    fn parsea_fechas_iso_utc() {
        assert_eq!(
            parse_iso_utc_secs("2026-11-22 22:30:21.831970793+00:00"),
            Some(1795386621)
        );
        assert_eq!(parse_iso_utc_secs("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_iso_utc_secs("invalida"), None);
    }

    #[test]
    fn sesiones_estado_arbol_detalle_y_logs_funcionan_con_sqlite_real() {
        let dir = temp_app_dir("real");
        seed_db(&dir);

        // Transcript y daemon files reales.
        let tdir = dir.join("brain/root-1/.system_generated/logs");
        std::fs::create_dir_all(&tdir).unwrap();
        std::fs::write(
            tdir.join("transcript.jsonl"),
            concat!(
                "{\"step_index\":1,\"type\":\"USER_INPUT\",\"source\":\"USER_EXPLICIT\",\"status\":\"DONE\",\"content\":\"hola\"}\n",
                "{\"step_index\":2,\"type\":\"PLANNER_RESPONSE\",\"source\":\"MODEL\",\"status\":\"DONE\",\"content\":\"listo\"}\n"
            ),
        )
        .unwrap();
        std::fs::create_dir_all(dir.join("conversations")).unwrap();
        std::fs::write(dir.join("conversations/root-1.db"), b"sqlite-stub").unwrap();
        std::fs::write(dir.join("daemon/tasks.json"), r#"{"tasks":[{"id":"t1"}]}"#).unwrap();
        std::fs::write(dir.join("daemon/daemon.log"), "l1\nl2\nl3\n").unwrap();

        let lista = sesiones_en(&dir).unwrap();
        assert_eq!(lista.len(), 3);
        // Empty parent_conversation_id in SQLite must normalize to None, not Some("").
        let root1 = lista.iter().find(|s| s.conversation_id == "root-1").unwrap();
        assert!(root1.padre.is_none());

        let est = estado_en(&dir).unwrap();
        assert!(est.db_legible);
        assert_eq!(est.total, 3);
        assert_eq!(est.en_curso, 1);
        assert_eq!(est.idle, 2);
        assert_eq!(est.muertas, 1);
        assert_eq!(est.con_padre, 1);
        assert_eq!(est.inactivas_24h, 2);
        assert_eq!(est.colgadas, 2);

        let dead = muertas_en(&dir).unwrap();
        assert_eq!(dead.len(), 1);
        assert_eq!(dead[0].conversation_id, "child-dead");
        assert_eq!(dead[0].padre.as_deref(), Some("root-1"));

        let stuck = colgadas_en(&dir).unwrap();
        assert_eq!(stuck.len(), 2);

        let tree = arbol_en(&dir, Some("root-1")).unwrap();
        assert_eq!(tree["hijos"].as_array().unwrap().len(), 1);
        assert_eq!(tree["hijos"][0]["conversation_id"], "child-dead");

        let det = detalle_sesion_en(&dir, "root-1", 1).unwrap();
        assert_eq!(det["hijos"].as_array().unwrap().len(), 1);
        assert_eq!(det["conversation_db"]["exists"], true);
        assert_eq!(det["transcript"]["total_steps"], 2);
        assert_eq!(det["transcript"]["tail"].as_array().unwrap().len(), 1);
        assert_eq!(det["transcript"]["tail"][0]["step_index"], 2);

        let tasks = tareas_daemon_en(&dir).unwrap();
        assert_eq!(tasks["tasks"][0]["id"], "t1");

        let logs = daemon_log_en(&dir, 2).unwrap();
        assert_eq!(logs, vec!["l2".to_string(), "l3".to_string()]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_estado_normalizado_cubre_los_casos() {
        assert_eq!(estado_normalizado("CASCADE_RUN_STATUS_IDLE", 1, 0, false), "MUERTA");
        assert_eq!(estado_normalizado("CASCADE_RUN_STATUS_IDLE", 0, 1, false), "COLGADA");
        assert_eq!(estado_normalizado("CASCADE_RUN_STATUS_IDLE", 0, 0, false), "IDLE");
        assert_eq!(estado_normalizado("CASCADE_RUN_STATUS_RUNNING", 0, 0, true), "COLGADA");
        assert_eq!(estado_normalizado("CASCADE_RUN_STATUS_RUNNING", 0, 0, false), "EN_CURSO");
    }

    #[test]
    fn los_tokens_y_el_modelo_se_leen_sin_bloquear() {
        let dir = temp_app_dir("tokens");
        seed_db(&dir);
        std::fs::create_dir_all(dir.join("conversations")).unwrap();
        {
            let con = rusqlite::Connection::open(dir.join("conversations/root-1.db")).unwrap();
            con.execute_batch(
                "CREATE TABLE usage (id INTEGER, prompt_tokens INTEGER, completion_tokens INTEGER, model TEXT);
                 INSERT INTO usage VALUES (1, 1200, 340, 'gemini-2.5-pro');
                 INSERT INTO usage VALUES (2, 800, 120, 'gemini-2.5-pro');",
            )
            .unwrap();
        }
        let v = tokens_y_modelo_en(&dir, "root-1");
        assert_eq!(v["ok"], true);
        // 1200+340+800+120 = 2460 (las dos columnas que parecen tokens).
        assert_eq!(v["tokens"], 2460);
        assert_eq!(v["modelo"], "gemini-2.5-pro");
        // Y devuelve el esquema que ha encontrado (para poder medirlo).
        assert!(v["esquema"].as_array().unwrap().len() >= 1);

        // Una conversacion sin DB: NO es un error, se dice el motivo.
        let v2 = tokens_y_modelo_en(&dir, "no-existe");
        assert_eq!(v2["ok"], false);
        assert_eq!(v2["tokens"], 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_correlacion_con_el_ndjson_del_shim_cruza_por_ventana() {
        let dir = temp_app_dir("corr");
        seed_db(&dir);
        // Una sesion con ventana CONOCIDA (2026-11-22 22:30:21 UTC = 1795386621).
        {
            let con = rusqlite::Connection::open(dir.join("conversation_summaries.db")).unwrap();
            con.execute(
                "INSERT INTO conversation_summaries (conversation_id, title, step_count, last_modified_time, last_user_input_time, status, agent_name, parent_conversation_id, nesting_depth, not_fully_idle, killed)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '', 0, 0, 0)",
                rusqlite::params!["root-2026", "De 2026", 7, "2026-11-22 22:35:21+00:00", "2026-11-22 22:30:21+00:00", "CASCADE_RUN_STATUS_RUNNING", ""],
            )
            .unwrap();
        }
        // El NDJSON del shim: una entrada dentro de la vida de root-2026 y
        // otra fuera de toda ventana.
        let nd = dir.join("agyshim.ndjson");
        std::fs::write(
            &nd,
            concat!(
                r#"{"ts":1795386621,"metodo":"POST","ruta":"/v1internal:generateContent","requested_model":"megadaemon","real_model":"minimax-m3.1-flash","substituted":true,"proxy":true}"#,
                "\n",
                r#"{"ts":1000000000,"metodo":"POST","ruta":"/v1internal:generateContent","requested_model":"megadaemon","real_model":"otro","substituted":true,"proxy":true}"#,
                "\n",
            ),
        )
        .unwrap();
        let v = correlacion_shim_de(&dir, &nd, 50).expect("correlacion");
        assert_eq!(v["entradas_totales"], 2);
        // Las sesiones de 2020 y la de 2099 quedan FUERA de la ventana.
        let corr = v["correlacion"].as_array().unwrap();
        assert_eq!(corr.len(), 1, "solo una sesion cae en la ventana");
        let r0 = &corr[0];
        assert_eq!(r0["conversation_id"], "root-2026");
        assert_eq!(r0["sustituciones"], 1);
        assert!(r0["entradas_shim"].as_u64().unwrap() >= 1);
        assert!(r0["modelos_reales"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m == "minimax-m3.1-flash"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sin_db_devuelve_vacio_sin_error() {
        let dir = temp_app_dir("empty");
        let s = sesiones_en(&dir).unwrap();
        assert!(s.is_empty());
        let e = estado_en(&dir).unwrap();
        assert!(!e.db_legible);
        assert_eq!(e.total, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
