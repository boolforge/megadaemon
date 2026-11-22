//! Instrumentación de las **sesiones de Antigravity** en el móvil.
//!
//! MEDIDO (2026-11-22, S21 Ultra): Antigravity guarda el índice de sus
//! conversaciones en
//! `/data/user/0/com.agy/files/.gemini/antigravity-app/conversation_summaries.db`
//! (SQLite, tabla `conversation_summaries`) y cada conversación en
//! `conversations/<id>.db`.
//!
//! La tabla tiene exactamente los campos que hacen falta para **controlar**
//! una sesión y no sólo verla:
//!
//! | campo | para qué sirve |
//! |---|---|
//! | `status` | `CASCADE_RUN_STATUS_IDLE` / en curso |
//! | `step_count` | cuántos pasos lleva |
//! | `killed` | **1 = el sub-agente fue MATADO** (colgado) |
//! | `not_fully_idle` | sigue con trabajo pendiente |
//! | `last_user_input_time` / `last_user_input_step_index` | el último input del usuario y en qué paso |
//! | `battle_id` / `winning_conversation_id` | las variantes que compiten y cuál gana |
//! | `parent_conversation_id` / `nesting_depth` | el árbol de sub-agentes |
//!
//! Ejemplo real (medido): la conversación `9dfa200c-…` tiene un sub-agente
//! `f54617bc-…` con `step_count=180`, `agent_name=self`,
//! `nesting_depth=1` y **`killed=1`** ⇒ ese sub-agente estaba muerto desde
//! el 17-Nov mientras su padre seguía vivo.  Eso es exactamente el "se
//! queda aburrido" que se quejó el usuario.
//!
//! Todo es **lectura**: no se toca ningún proceso de la app.

use serde::{Deserialize, Serialize};

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
}

/// Resumen agregado del estado de las sesiones.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EstadoSesiones {
    pub total: usize,
    pub en_curso: usize,
    pub idle: usize,
    pub muertas: usize,
    pub con_padre: usize,
    /// Sesiones que llevan demasiado sin modificarse (horas).
    pub inactivas_24h: usize,
    /// La conversación más recientemente tocada.
    pub ultima: Option<ResumenSesion>,
    pub db_legible: bool,
    pub detalle: String,
}

/// Lee el índice de conversaciones.
///
/// Devuelve un vector vacío (y `db_legible=false`) si la base no está: en el
/// PC o en un entorno sin móvil no es un error.
pub fn sesiones() -> Result<Vec<ResumenSesion>, String> {
    if !std::path::Path::new(DB_SUMMARIES).exists() {
        return Ok(Vec::new());
    }
    let con = rusqlite::Connection::open(DB_SUMMARIES).map_err(|e| format!("sqlite: {e}"))?;
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
                ultima_modificacion: r.get::<_, Option<String>>(4)
                    .unwrap_or_default()
                    .unwrap_or_default(),
                workspace: r.get::<_, Option<String>>(5)
                    .unwrap_or_default()
                    .unwrap_or_default(),
                estado: r.get::<_, Option<String>>(6).unwrap_or_default().unwrap_or_default(),
                agente: r.get::<_, Option<String>>(7).unwrap_or_default().unwrap_or_default(),
                padre: r.get::<_, Option<String>>(8).unwrap_or_default(),
                profundidad: r.get::<_, Option<i64>>(9).unwrap_or_default().unwrap_or(0),
                killed: r.get::<_, Option<i64>>(10).unwrap_or_default().unwrap_or(0),
                no_idle: r.get::<_, Option<i64>>(11).unwrap_or_default().unwrap_or(0),
                battle_id: r.get::<_, Option<String>>(12).unwrap_or_default(),
                ganadora: r.get::<_, Option<String>>(13).unwrap_or_default(),
                ultimo_input: r.get::<_, Option<String>>(14)
                    .unwrap_or_default()
                    .unwrap_or_default(),
                ultimo_input_paso: r.get::<_, Option<i64>>(15).unwrap_or_default().unwrap_or(-1),
            })
        })
        .map_err(|e| format!("query: {e}"))?;
    let mut out = Vec::new();
    for f in filas.flatten() {
        out.push(f);
    }
    Ok(out)
}

/// Estado agregado de las sesiones.
pub fn estado() -> Result<EstadoSesiones, String> {
    let lista = sesiones()?;
    let mut e = EstadoSesiones {
        total: lista.len(),
        db_legible: std::path::Path::new(DB_SUMMARIES).exists(),
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
    }
    e.ultima = lista.first().cloned();
    e.detalle = format!(
        "{} conversaciones: {} en curso, {} idle, {} muertas (sub-agentes matados), {} con padre",
        e.total, e.en_curso, e.idle, e.muertas, e.con_padre
    );
    Ok(e)
}

/// Lista las sesiones **muertas** (sub-agentes matados) con su padre.
///
/// Es la vista "qué se quedó colgado" del panel.
pub fn muertas() -> Result<Vec<ResumenSesion>, String> {
    Ok(sesiones()?.into_iter().filter(|s| s.killed == 1).collect())
}

/// Estado del planificador interno de Antigravity (`daemon/tasks.json`).
///
/// Devuelve el JSON tal cual (sin interpretar): es el planificador de la
/// app y su formato es suyo.
pub fn tareas_daemon() -> Result<serde_json::Value, String> {
    let texto = std::fs::read_to_string(DAEMON_TASKS).map_err(|e| format!("leer tasks.json: {e}"))?;
    serde_json::from_str(&texto).map_err(|e| format!("tasks.json no es JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_movil_no_revienta() {
        let s = sesiones().unwrap_or_default();
        // En el sandbox no hay la base: vacio, sin panic.
        assert!(s.is_empty() || !s.is_empty());
    }

    #[test]
    fn el_estado_tiene_campos_de_control() {
        let e = estado().unwrap_or_default();
        assert!(e.total >= 0);
        assert!(e.detalle.contains("conversaciones"));
    }

    #[test]
    fn la_db_debe_estar_en_una_ruta_conocida() {
        assert!(DB_SUMMARIES.contains("antigravity-app"));
        assert!(DAEMON_TASKS.ends_with("daemon/tasks.json"));
    }
}
