//! **Persistencia de sesiones ACP en SQLite** (`acp_sesion_store.rs`).
//!
//! ## Propósito
//!
//! `protocol_acp/sesion.rs` guarda las sesiones ACP **en memoria del
//! proceso** (`docs/DECISION_ACP.md` §3.2: «la persistencia de sesiones ...
//! es otro módulo»).  Este módulo es ese otro módulo: un **store SQLite**
//! que persiste el **mismo modelo** que existe hoy en memoria (`id`, `cwd`,
//! `modo`, `padre`, `estado`, historial, `turnos`) y lo restaura tal cual,
//! con la semántica de **épocas** del patrón 4 (compactar **no borra**).
//!
//! La regla del repo es que toda capacidad nueva vive en Rust dentro del
//! daemon; este módulo es sólo el **almacén**: no decide nada de ACP, no
//! habla con el cliente y no depende del protocolo.  Lo que decide (validar
//! una fila, calcular el siguiente `seq`, decir si un mensaje entra en la
//! vista viva) son **funciones puras** y testeables sin base de datos.
//!
//! ## Fuentes medidas
//!
//! * `docs/RE_DEVIN_CLI_RUST.md` §4.1 — DDL literal medido del producto de
//!   referencia: `sessions(id, working_directory, …, current_epoch)`,
//!   `messages(…, compaction_epoch, sequence_number,
//!   UNIQUE(session_id, compaction_epoch, sequence_number))` e índice
//!   `idx_messages_session_epoch`.
//! * `docs/PATRONES_CLI_RUST_DEFINICION.md` §Patrón 4 (la compactación marca
//!   una **época**, no borra) y §Patrón 10 (esquema SQLite de sesiones).
//! * `rust-core/src/compaction_epoch.rs` — la semántica de época ya
//!   implementada: `seq` **global** que no se reinicia, vista viva = época
//!   actual, historial completo consultable por época.
//! * `rust-core/src/protocol_acp/sesion.rs` — el modelo ACP que existe hoy
//!   en memoria (id, cwd, modo, padre, estado, mensajes, turnos) y los
//!   nombres de estado (`activa` / `cancelada` / `terminada`).
//! * `rust-core/src/protocol_acp/updates.rs` — los `session/update` que
//!   entran en el historial (un `Value` JSON heterogéneo).
//! * `rust-core/src/board.rs` — el patrón rusqlite del repo: fichero en el
//!   `state_dir`, `busy_timeout`, WAL con degradación a DELETE,
//!   `schema_version` en una tabla `*_meta`, esquema idempotente.
//!
//! ## Qué NO está medido
//!
//! * El DDL de 10 tablas del producto de referencia **no se copia** (es
//!   material propietario): aquí se define el **esquema propio** de este
//!   store, con las **mismas ideas** (épocas, `UNIQUE`, índices) y las
//!   columnas que el modelo ACP necesita y el medido no tiene.
//! * El comportamiento en el **móvil** (`su -mm -c`) no está medido: el repo
//!   sólo verifica tipos, tests y contratos.
//! * El **solapamiento** con `rust-core/src/board_schema.rs` (otro
//!   subagente, DDL medido de 10 tablas) **no está resuelto aquí**: este
//!   módulo **no depende** de él y usa **nombres de tabla propios** para no
//!   chocar.  Ver «Solapamiento» más abajo.
//!
//! ## Tablas y columnas (las que usa este store)
//!
//! Fichero de base de datos: **`acp_sesiones.sqlite`** dentro del
//! `state_dir` (`state_dir::state_dir()`), nombre propio para no tocar el
//! `board.sqlite` del tablón.
//!
//! | tabla | columnas |
//! |---|---|
//! | `acp_meta` | `key` (PK), `value` |
//! | `acp_sessions` | `id` (PK), `cwd`, `modo`, `padre`, `estado`, `current_epoch`, `turnos`, `created_at`, `updated_at` |
//! | `acp_messages` | `id` (PK autoincr), `session_id`, `compaction_epoch`, `sequence_number`, `rol`, `contenido`, `es_resumen`, `acp_json`, `created_at`, `UNIQUE(session_id, compaction_epoch, sequence_number)` |
//! | `acp_session_extensions` | `session_id`, `key` (PK compuesta), `value`, `updated_at` |
//! | `acp_sessions_borradas` | `session_id` (PK), `borrada_at` |
//!
//! Índices: `idx_acp_messages_session_epoch (session_id, compaction_epoch)`,
//! `idx_acp_messages_session_seq (session_id, sequence_number)`,
//! `idx_acp_sessions_activity (updated_at)`.
//!
//! ## Correspondencia con el DDL medido (para el root)
//!
//! | DDL medido (`docs/RE_DEVIN_CLI_RUST.md` §4.1) | aquí |
//! |---|---|
//! | `sessions.id` | `acp_sessions.id` |
//! | `sessions.working_directory` | `acp_sessions.cwd` |
//! | `sessions.current_epoch` | `acp_sessions.current_epoch` |
//! | `sessions.created_at` / `last_activity_at` | `created_at` / `updated_at` |
//! | `sessions.backend_type` / `model` / `permission_mode` | — (el modelo ACP no los tiene; `modo` es lo más cercano) |
//! | *(no existe)* | `acp_sessions.estado` (activa/cancelada/terminada) |
//! | *(no existe)* | `acp_sessions.padre` (el BOSQUE: la sesión de la que forked) |
//! | *(no existe)* | `acp_sessions.turnos` |
//! | `messages.id` | `acp_messages.id` |
//! | `messages.session_id` | `acp_messages.session_id` |
//! | `messages.sequence_number` | `acp_messages.sequence_number` (global por sesión) |
//! | `messages.compaction_epoch` | `acp_messages.compaction_epoch` |
//! | `messages.role` | `acp_messages.rol` |
//! | `messages.content` | `acp_messages.contenido` |
//! | *(no existe)* | `acp_messages.acp_json` (el `Value` ACP **exacto**: sin esto el round-trip pierde el `update`) |
//! | *(no existe)* | `acp_messages.es_resumen` |
//!
//! ## Solapamiento con `board_schema.rs` (pendiente de resolver por el root)
//!
//! * Este módulo **crea sus propias tablas** (`acp_*`) y **no depende** de
//!   `board_schema.rs` (que otro subagente está escribiendo).
//! * **MEDIDO (2026-11-28, el fichero ya existe)**: `board_schema.rs` crea
//!   `sessions`, `messages`, `message_nodes`, `subagent_heads`,
//!   `tool_call_state`, `prompt_history`, `rendered_commits`,
//!   `live_row_times`, `session_window_tools` y `app_state`, con los
//!   índices `idx_messages_session_epoch`, `idx_sessions_activity` y
//!   `idx_sessions_hidden`.  **Ninguno** de esos nombres choca con los
//!   míos (`acp_*`): los dos esquemas pueden vivir en la **misma** base sin
//!   conflicto.
//! * Los **nombres de columna** de los campos que sí coinciden con el DDL
//!   medido (`current_epoch`, `compaction_epoch`, `sequence_number`,
//!   `created_at`, `updated_at`) son los mismos, para que una futura
//!   migración sea una copia de columnas y no una traducción.
//! * Si el root decide **fusionar** los dos esquemas, lo que hay que hacer
//!   es: (a) renombrar `acp_sessions`→`sessions` y `acp_messages`→`messages`
//!   (su `session_id` ya es **TEXT**, como el mío, porque el id de sesión
//!   ACP es una cadena, no el `INTEGER` del DDL medido), (b) decidir si
//!   `rol`→`role` y `contenido`→`content` (y `cwd`→`working_directory`,
//!   `updated_at`→`last_activity_at`), (c) decidir qué hacer con `acp_json`
//!   (el DDL medido tiene `tool_calls_json` y `tool_call_id`, pero **no** tiene
//!   dónde guardar el `session/update` completo ⇒ sin `acp_json` se pierde
//!   información), (d) decidir si `acp_sessions_borradas` (la tumba) se
//!   queda o se sustituye por el `hidden` que el DDL medido indexa pero no
//!   define, y (e) decidir si `acp_sessions.estado` / `.padre` / `.turnos`
//!   (que el DDL medido no tiene dónde meter) se quedan en una tabla aparte
//!   o se aparcan en `sessions` con columnas nuevas.
//! * El fichero de base de datos **no** colisiona: `board_schema.rs` no
//!   define ninguno (es sólo DDL) y el mío es `acp_sesiones.sqlite`.
//!
//! ## Decisiones
//!
//! * **Borrar es EXPLÍCITO**: `borrar_sesion` es la **única** operación que
//!   elimina filas.  Guardar, compactar, revertir o cargar **no** borran
//!   nada (patrón 4: la compactación marca una época, no borra).
//! * **El borrado deja tumba** (`acp_sessions_borradas`) ⇒ un error
//!   tipado distingue «nunca existió» ([`ErrorStore::SesionNoExiste`]) de
//!   «existió y se borró» ([`ErrorStore::SesionYaBorrada`]).
//! * **`seq` es global por sesión** y **no se reinicia**: ni al compactar,
//!   ni al recargar, ni al reabrir la base.  Es el `sequence_number` del
//!   DDL medido.

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Fichero de la base de sesiones ACP dentro del `state_dir`.
///
/// Nombre **propio** a propósito: el tablón (`board.sqlite`) y las sesiones
/// ACP son cosas distintas y no deben compartir fichero sin querer.
pub const ACPS_DB_FILE: &str = "acp_sesiones.sqlite";

/// Versión del esquema de este store (tabla `acp_meta`).
pub const SCHEMA_VERSION: i64 = 1;

/// Segundos de espera a que SQLite libere un bloqueo (el mismo valor que
/// usa `board.rs`).
pub const BUSY_TIMEOUT_MS: u32 = 5_000;

// =============================================================================
// ERRORES TIPADOS
// =============================================================================

/// Errores del store: un **tipo**, no un `String`.
///
/// MEDIDO (patrón 5): el llamante decide si degrada, reintenta o corrige.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorStore {
    /// La sesión no existe (nunca existió en esta base).
    SesionNoExiste(String),
    /// La sesión existió, se borró explícitamente y se sigue pidiendo.
    ///
    /// Distinguirlo de [`ErrorStore::SesionNoExiste`] importa: borrar es
    /// una decisión explícita y **resucitar** una sesión borrada también
    /// tiene que serlo.
    SesionYaBorrada(String),
    /// La base no tiene el esquema que este código espera.
    ///
    /// MEDIDO: el DDL medido y el de este store son distintos; si alguien
    /// abre aquí una base con otra forma, el fallo tiene que ser
    /// explícito y no un error de SQL genérico.
    EsquemaIncompatible(String),
    /// Ya hay un mensaje con la misma clave
    /// (`session_id`, `compaction_epoch`, `sequence_number`) con **otro**
    /// contenido.
    MensajeDuplicado {
        session_id: String,
        compaction_epoch: u32,
        sequence_number: u64,
    },
    /// El mensaje con ese `id` no existe (no se puede actualizar).
    MensajeNoExiste(String),
    /// Los datos que se quieren guardar no son válidos.
    DatosInvalidos(String),
    /// Otro error de SQLite o de disco.
    Sqlite(String),
}

impl std::fmt::Display for ErrorStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorStore::SesionNoExiste(id) => write!(f, "no existe la sesión {}", id),
            ErrorStore::SesionYaBorrada(id) => {
                write!(f, "la sesión {} fue borrada (borrado explícito)", id)
            }
            ErrorStore::EsquemaIncompatible(m) => {
                write!(f, "esquema incompatible con este store: {}", m)
            }
            ErrorStore::MensajeDuplicado {
                session_id,
                compaction_epoch,
                sequence_number,
            } => write!(
                f,
                "mensaje duplicado en {} (epoch {}, seq {})",
                session_id, compaction_epoch, sequence_number
            ),
            ErrorStore::MensajeNoExiste(id) => write!(f, "no existe el mensaje {}", id),
            ErrorStore::DatosInvalidos(m) => write!(f, "datos inválidos: {}", m),
            ErrorStore::Sqlite(m) => write!(f, "error de SQLite: {}", m),
        }
    }
}

impl std::error::Error for ErrorStore {}

impl From<rusqlite::Error> for ErrorStore {
    fn from(e: rusqlite::Error) -> Self {
        match e {
            rusqlite::Error::SqliteFailure(ref inner, _) => match inner.code {
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked => {
                    ErrorStore::Sqlite(format!("la base está ocupada: {}", e))
                }
                rusqlite::ErrorCode::DiskFull => {
                    ErrorStore::Sqlite(format!("no queda espacio en disco: {}", e))
                }
                rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase => {
                    ErrorStore::EsquemaIncompatible(format!("la base no es usable: {}", e))
                }
                rusqlite::ErrorCode::ConstraintViolation => {
                    ErrorStore::Sqlite(format!("violación de restricción: {}", e))
                }
                _ => ErrorStore::Sqlite(e.to_string()),
            },
            other => ErrorStore::Sqlite(other.to_string()),
        }
    }
}

// =============================================================================
// MODELO
// =============================================================================

/// El estado de una sesión ACP.
///
/// MEDIDO (`protocol_acp/sesion.rs`): los nombres son los mismos que allí
/// (`activa` / `cancelada` / `terminada`) ⇒ el root puede mapear sin
/// diccionarios.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EstadoSesion {
    /// Activa: admite `session/prompt`.
    Activa,
    /// Cancelada: se puede `session/resume`.
    Cancelada,
    /// Terminada: el turno ended con un `stopReason`.
    Terminada,
}

impl EstadoSesion {
    /// El nombre estable (el que va al disco y al JSON).
    pub fn nombre(self) -> &'static str {
        match self {
            EstadoSesion::Activa => "activa",
            EstadoSesion::Cancelada => "cancelada",
            EstadoSesion::Terminada => "terminada",
        }
    }

    /// Parseo tolerante: un estado desconocido es [`EstadoSesion::Activa`]
    /// (igual que `board.rs` con los estados del tablón).
    pub fn desde_nombre(s: &str) -> EstadoSesion {
        match s.trim().to_ascii_lowercase().as_str() {
            "cancelada" | "canceled" | "cancelled" => EstadoSesion::Cancelada,
            "terminada" | "terminated" | "ended" => EstadoSesion::Terminada,
            _ => EstadoSesion::Activa,
        }
    }

    /// ¿Admite un `session/prompt`?
    pub fn admite_prompt(self) -> bool {
        matches!(self, EstadoSesion::Activa)
    }
}

/// Un mensaje del historial, tal como se guarda en disco.
#[derive(Clone, Debug, PartialEq)]
pub struct MensajeAcp {
    /// Id de fila (`0` = todavía no está en la base).
    pub id: i64,
    /// La sesión a la que pertenece.
    pub session_id: String,
    /// La época a la que pertenece (patrón 4).
    pub compaction_epoch: u32,
    /// El `seq` **global** de la sesión (crece siempre; no se reinicia).
    pub sequence_number: u64,
    /// El rol (`user`, `assistant`, `thought`, `resumen`, `update`…).
    pub rol: String,
    /// El texto del mensaje (lo que mide el presupuesto de contexto).
    pub contenido: String,
    /// Si este mensaje es el **resumen** que abrió su época.
    pub es_resumen: bool,
    /// El `Value` ACP **exacto** (sin esto el round-trip pierde el
    /// `session/update` y los `ContentBlock`).
    pub acp_json: Value,
    /// Instante de creación (segundos Unix).
    pub created_at: i64,
}

impl MensajeAcp {
    /// Un mensaje nuevo (aún sin id de fila).
    pub fn nuevo(
        session_id: &str,
        rol: &str,
        contenido: &str,
        acp_json: Value,
        epoch: u32,
        seq: u64,
    ) -> Self {
        MensajeAcp {
            id: 0,
            session_id: session_id.to_string(),
            compaction_epoch: epoch,
            sequence_number: seq,
            rol: rol.to_string(),
            contenido: contenido.to_string(),
            es_resumen: false,
            acp_json,
            created_at: ahora(),
        }
    }

    /// Caracteres del contenido (lo que mide el presupuesto de contexto).
    pub fn chars(&self) -> usize {
        self.contenido.chars().count()
    }
}

/// Una sesión ACP persistida.
///
/// Refleja el modelo de `protocol_acp/sesion.rs` (`Sesion`) más la época
/// actual, que en el modelo en memoria todavía no existe.
#[derive(Clone, Debug, PartialEq)]
pub struct SesionAcp {
    /// Identificador (opaco para el cliente).
    pub id: String,
    /// Directorio de trabajo de la sesión.
    pub cwd: String,
    /// Modo actual (ACP v1 tiene modos).
    pub modo: Option<String>,
    /// La sesión de la que forked (el BOSQUE).
    pub padre: Option<String>,
    /// Estado.
    pub estado: EstadoSesion,
    /// La época en la que se está trabajando ahora.
    pub current_epoch: u32,
    /// Turnos completados.
    pub turnos: u32,
    /// El historial **completo** en formato ACP (lo que devuelve
    /// `session/load`): mensajes y `session/update`, de todas las épocas.
    pub mensajes: Vec<Value>,
    /// Instante de creación (segundos Unix).
    pub created_at: i64,
    /// Instante de la última escritura (segundos Unix).
    pub updated_at: i64,
}

impl SesionAcp {
    /// Una sesión nueva (época 0, activa, sin historial).
    pub fn nueva(id: &str, cwd: &str) -> Self {
        SesionAcp {
            id: id.to_string(),
            cwd: cwd.to_string(),
            modo: None,
            padre: None,
            estado: EstadoSesion::Activa,
            current_epoch: 0,
            turnos: 0,
            mensajes: Vec::new(),
            created_at: ahora(),
            updated_at: ahora(),
        }
    }

    /// El número de mensajes del historial (todas las épocas).
    pub fn num_mensajes(&self) -> usize {
        self.mensajes.len()
    }
}

/// Una sesión con su historial detailado (los mensajes con su `seq` y su
/// época, no sólo el `Value` ACP).
#[derive(Clone, Debug, PartialEq)]
pub struct HistorialSesion {
    /// La sesión (metadatos).
    pub sesion: SesionAcp,
    /// Todos los mensajes, en orden de `seq` (épocas incluidas).
    pub mensajes: Vec<MensajeAcp>,
}

/// El resumen de una sesión (lo que ve el cliente en `session/list`).
#[derive(Clone, Debug, PartialEq)]
pub struct ResumenSesion {
    pub id: String,
    pub cwd: String,
    pub modo: Option<String>,
    pub padre: Option<String>,
    pub estado: EstadoSesion,
    pub current_epoch: u32,
    pub turnos: u32,
    /// Nº de mensajes del historial completo.
    pub mensajes: usize,
    pub created_at: i64,
    pub updated_at: i64,
}

impl ResumenSesion {
    /// El resumen en JSON (compatible con `sesion::Sesion::resumen()`: los
    /// mismos campos, más `current_epoch` y las marcas de tiempo).
    pub fn a_json(&self) -> Value {
        json!({
            "sessionId": self.id,
            "cwd": self.cwd,
            "estado": self.estado.nombre(),
            "modo": self.modo,
            "padre": self.padre,
            "turnos": self.turnos,
            "mensajes": self.mensajes,
            "current_epoch": self.current_epoch,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
        })
    }
}

/// Lo que devolvió una compactación (mismos campos que
/// `compaction_epoch::Compactacion`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Compactacion {
    /// La época nueva (la anterior queda sellada).
    pub epoch_nueva: u32,
    /// La época que se ha sellado.
    pub epoch_sellada: u32,
    /// Cuántos mensajes se han movido a la época anterior (**NO** se borran).
    pub mensajes_sellados: usize,
    /// Caracteres que el resumen ahorra en el prompt.
    pub chars_ahorrados: usize,
    /// El historial COMPLETO sigue siendo el mismo número de mensajes.
    pub mensajes_totales: usize,
}

/// Lo que borró un `borrar_sesion` explícito.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Borrado {
    /// La sesión borrada.
    pub session_id: String,
    /// Mensajes eliminados.
    pub mensajes: usize,
    /// Extensiones eliminadas.
    pub extensiones: usize,
}

// =============================================================================
// DECISIONES PURAS (testeables sin base de datos)
// =============================================================================

/// El siguiente `seq` global de una sesión.
///
/// MEDIDO (patrón 4): el `seq` es global y **no se reinicia**; el siguiente
/// es siempre el mayor + 1.  Con la lista vacía, el primero es el 1.
pub fn siguiente_seq(seqs: &[u64]) -> u64 {
    seqs.iter().copied().max().unwrap_or(0).saturating_add(1)
}

/// ¿Este mensaje entra en la **vista viva**?
///
/// La vista viva es lo que el agente ve ahora: **sólo** la época actual
/// (patrón 4, criterio 2).
pub fn es_mensaje_vivo(m: &MensajeAcp, current_epoch: u32) -> bool {
    m.compaction_epoch == current_epoch
}

/// ¿Este mensaje es de esa época concreta?
pub fn es_mensaje_en_epoch(m: &MensajeAcp, epoch: u32) -> bool {
    m.compaction_epoch == epoch
}

/// Valida una sesión antes de guardarla.
///
/// MEDIDO: se valida lo **estructural** (el id tiene que existir y ser
/// utilizable como clave); el `cwd` **no** se comprueba contra el disco
/// (una sesión puede apuntar a un directorio que todavía no existe) y los
/// campos opcionales admiten cualquier cosa.
pub fn validar_sesion(s: &SesionAcp) -> Result<(), ErrorStore> {
    if s.id.trim().is_empty() {
        return Err(ErrorStore::DatosInvalidos(
            "el id de sesión está vacío".into(),
        ));
    }
    if s.id.contains('\0') {
        return Err(ErrorStore::DatosInvalidos(
            "el id de sesión contiene un NUL".into(),
        ));
    }
    if s.id.len() > 256 {
        return Err(ErrorStore::DatosInvalidos(
            "el id de sesión es más largo de 256 bytes".into(),
        ));
    }
    Ok(())
}

/// Valida un mensaje antes de guardarlo.
pub fn validar_mensaje(m: &MensajeAcp) -> Result<(), ErrorStore> {
    if m.session_id.trim().is_empty() {
        return Err(ErrorStore::DatosInvalidos(
            "el mensaje no tiene sesión".into(),
        ));
    }
    if m.sequence_number == 0 {
        return Err(ErrorStore::DatosInvalidos(
            "el seq del mensaje tiene que ser >= 1".into(),
        ));
    }
    Ok(())
}

/// El rol de un `Value` ACP (puro).
///
/// Los mensajes del historial ACP son heterogéneos: un prompt
/// (`{"role":"user","content":[…]}`), una respuesta
/// (`{"role":"assistant","stopReason":…}`) o un `session/update`
/// (`{"sessionUpdate":"agent_message_chunk",…}`).  Aquí se normalize el
/// discriminante a un rol plano (lo que va a la columna `rol`).
pub fn rol_de_json(v: &Value) -> String {
    if let Some(r) = v.get("role").and_then(|x| x.as_str()) {
        return r.to_string();
    }
    match v
        .get("sessionUpdate")
        .and_then(|x| x.as_str())
        .unwrap_or("")
    {
        "user_message_chunk" => "user".to_string(),
        "agent_message_chunk" => "assistant".to_string(),
        "agent_thought_chunk" => "thought".to_string(),
        "tool_call" => "tool_call".to_string(),
        "tool_call_update" => "tool_call_update".to_string(),
        _ => "update".to_string(),
    }
}

/// El texto de un `Value` ACP (puro).
///
/// El texto es lo que mide el presupuesto de contexto; si el `Value` no
/// tiene texto legible se cae al JSON serializado (nunca se pierde el
/// contenido).
pub fn contenido_de_json(v: &Value) -> String {
    // 1. `{"role":…,"content":[…]}` (los `ContentBlock` del prompt).
    if let Some(c) = v.get("content") {
        let t = texto_de_bloque(c);
        if !t.is_empty() {
            return t;
        }
    }
    // 2. `{"sessionUpdate":"agent_message_chunk","content":{"type":"text",…}}`.
    if let Some(u) = v.get("update") {
        let t = texto_de_bloque(u.get("content").unwrap_or(&Value::Null));
        if !t.is_empty() {
            return t;
        }
    }
    // 3. `{"role":"assistant","stopReason":…}` ⇒ el motivo es lo que hay.
    if let Some(r) = v.get("stopReason").and_then(|x| x.as_str()) {
        return format!("stopReason: {}", r);
    }
    // 4. Sin texto legible: el JSON entero (no se pierde nada).
    if v.is_null() {
        String::new()
    } else {
        serializar(v)
    }
}

/// Texto de un bloque o lista de bloques (`ContentBlock`).
fn texto_de_bloque(c: &Value) -> String {
    match c {
        Value::Array(items) => {
            let partes: Vec<String> = items.iter().map(texto_de_bloque).collect();
            partes
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        }
        Value::Object(_) => {
            if let Some(t) = c.get("text").and_then(|x| x.as_str()) {
                t.to_string()
            } else {
                String::new()
            }
        }
        Value::String(s) => s.clone(),
        _ => String::new(),
    }
}

/// Qué mensajes hay que **anexar** al guardar una sesión.
///
/// Puro: los mensajes ya guardados y los nuevos se comparan por su JSON
/// exacto y se anexa **sólo el sufijo** que no estaba (el prefijo común se
/// considera ya guardado).  Es lo que hace que guardar dos veces la misma
/// sesión sea **idempotente**.
pub fn nuevos_para_anexar(existentes: &[String], nuevos: &[String]) -> Vec<String> {
    let mut i = 0usize;
    while i < existentes.len() && i < nuevos.len() && existentes[i] == nuevos[i] {
        i += 1;
    }
    let corte = i.min(nuevos.len());
    nuevos[corte..].to_vec()
}

/// Decide si hay que compactar (patrón 4) y cuánto se ahorra.
///
/// MEDIDO (`compaction_epoch.rs`): bajo presupuesto **no** se compacta
/// (crear una época costaría más de lo que ahorra) y el ahorro es **0** si
/// el resumen es peor que el historial (no se inventa un número).
///
/// `None` = no se compacta.  `Some(ahorrado)` = se compacta y se ahorran
/// esos caracteres.
pub fn plan_compactacion(
    chars_vivos: usize,
    presupuesto: usize,
    chars_sellados: usize,
    chars_resumen: usize,
) -> Option<usize> {
    if chars_vivos <= presupuesto {
        return None;
    }
    Some(chars_sellados.saturating_sub(chars_resumen))
}

/// Dos filas de mensaje son **la misma** (para la idempotencia).
pub fn fila_mensaje_igual(a: &MensajeAcp, b: &MensajeAcp) -> bool {
    a.rol == b.rol
        && a.contenido == b.contenido
        && a.es_resumen == b.es_resumen
        && serializar(&a.acp_json) == serializar(&b.acp_json)
}

/// Serializa un `Value` a texto (nunca falla: `Value` es serializable).
pub fn serializar(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "null".to_string())
}

/// Ahora, en segundos Unix.
fn ahora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// =============================================================================
// ESQUEMA
// =============================================================================

/// El esquema de este store (aplicado de forma idempotente).
///
/// MEDIDO: las ideas vienen del DDL medido (`docs/RE_DEVIN_CLI_RUST.md`
/// §4.1): época en la sesión, época en el mensaje, `UNIQUE` por
/// `(session_id, compaction_epoch, sequence_number)` e índice por
/// `(session_id, compaction_epoch)`.  Los **nombres de tabla** son propios
/// (`acp_*`) para no chocar con `board_schema.rs`.
pub const ESQUEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS acp_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS acp_sessions (
    id            TEXT PRIMARY KEY,
    cwd           TEXT NOT NULL DEFAULT '',
    modo          TEXT,
    padre         TEXT,
    estado        TEXT NOT NULL DEFAULT 'activa',
    current_epoch INTEGER NOT NULL DEFAULT 0,
    turnos        INTEGER NOT NULL DEFAULT 0,
    created_at    INTEGER NOT NULL DEFAULT 0,
    updated_at    INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS acp_messages (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id       TEXT NOT NULL REFERENCES acp_sessions(id) ON DELETE CASCADE,
    compaction_epoch INTEGER NOT NULL DEFAULT 0,
    sequence_number  INTEGER NOT NULL,
    rol              TEXT NOT NULL DEFAULT '',
    contenido        TEXT NOT NULL DEFAULT '',
    es_resumen       INTEGER NOT NULL DEFAULT 0,
    acp_json         TEXT NOT NULL DEFAULT 'null',
    created_at       INTEGER NOT NULL DEFAULT 0,
    UNIQUE(session_id, compaction_epoch, sequence_number)
);
CREATE TABLE IF NOT EXISTS acp_session_extensions (
    session_id TEXT NOT NULL REFERENCES acp_sessions(id) ON DELETE CASCADE,
    key        TEXT NOT NULL,
    value      TEXT NOT NULL DEFAULT 'null',
    updated_at INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (session_id, key)
);
CREATE TABLE IF NOT EXISTS acp_sessions_borradas (
    session_id TEXT PRIMARY KEY,
    borrada_at INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_acp_messages_session_epoch
    ON acp_messages(session_id, compaction_epoch);
CREATE INDEX IF NOT EXISTS idx_acp_messages_session_seq
    ON acp_messages(session_id, sequence_number);
CREATE INDEX IF NOT EXISTS idx_acp_sessions_activity
    ON acp_sessions(updated_at);
"#;

/// Las tablas que este store espera encontrar, con sus columnas.
///
/// Se usa para detectar un **esquema incompatible** (base con otra forma)
/// y no dejar que SQLite falle luego con un error genérico.
pub const TABLAS_ESPERADAS: &[(&str, &[&str])] = &[
    (
        "acp_sessions",
        &[
            "id",
            "cwd",
            "modo",
            "padre",
            "estado",
            "current_epoch",
            "turnos",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "acp_messages",
        &[
            "id",
            "session_id",
            "compaction_epoch",
            "sequence_number",
            "rol",
            "contenido",
            "es_resumen",
            "acp_json",
            "created_at",
        ],
    ),
    (
        "acp_session_extensions",
        &["session_id", "key", "value", "updated_at"],
    ),
    ("acp_sessions_borradas", &["session_id", "borrada_at"]),
    ("acp_meta", &["key", "value"]),
];

// =============================================================================
// OPERACIONES CON LA BASE DE DATOS (funciones libres: `&Connection`)
// =============================================================================

/// Crea el esquema (idempotente: se puede aplicar mil veces).
///
/// MEDIDO: `CREATE TABLE IF NOT EXISTS` + `CREATE INDEX IF NOT EXISTS` ⇒
/// aplicar el esquema dos veces no falla y no duplica nada.
pub fn aplicar_esquema(conn: &Connection) -> Result<(), ErrorStore> {
    conn.execute_batch(ESQUEMA_V1).map_err(map_error_esquema)?;
    conn.execute(
        "INSERT INTO acp_meta(key,value) VALUES('schema_version',?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![SCHEMA_VERSION.to_string()],
    )?;
    Ok(())
}

/// Un error de SQLite al aplicar el esquema: si la base tiene **otra
/// forma** (una columna que no existe), es un esquema incompatible y no un
/// error genérico.
fn map_error_esquema(e: rusqlite::Error) -> ErrorStore {
    match &e {
        rusqlite::Error::SqliteFailure(_, Some(msg)) if msg.contains("no such column") => {
            ErrorStore::EsquemaIncompatible(format!("la base tiene otra forma: {}", msg))
        }
        _ => ErrorStore::from(e),
    }
}

/// Comprueba que la base tiene el esquema que este código espera.
///
/// Errores: [`ErrorStore::EsquemaIncompatible`] si falta una tabla o una
/// columna, o si la base es de una versión **más nueva** que el código.
pub fn verificar_esquema(conn: &Connection) -> Result<(), ErrorStore> {
    // 1. La versión (si la tabla de meta existe; en una base nueva no, y
    //    entonces no hay nada que comprobar).
    if tabla_existe(conn, "acp_meta")? {
        let version: Option<String> = conn
            .query_row(
                "SELECT value FROM acp_meta WHERE key='schema_version'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(v) = version {
            if let Ok(n) = v.parse::<i64>() {
                if n > SCHEMA_VERSION {
                    return Err(ErrorStore::EsquemaIncompatible(format!(
                        "la base es de la versión {} y este código sólo entiende la {}",
                        n, SCHEMA_VERSION
                    )));
                }
            }
        }
    }
    // 2. Las tablas y sus columnas.
    for (tabla, columnas) in TABLAS_ESPERADAS {
        if !tabla_existe(conn, tabla)? {
            // Una tabla que no está todavía no es un conflicto: la crea
            // `aplicar_esquema`.  Sólo se fiscalizan las que EXISTEN.
            continue;
        }
        let presentes = columnas_de(conn, tabla)?;
        for c in *columnas {
            if !presentes.iter().any(|p| p == c) {
                return Err(ErrorStore::EsquemaIncompatible(format!(
                    "la tabla {} no tiene la columna {}",
                    tabla, c
                )));
            }
        }
    }
    Ok(())
}

/// ¿Existe esa tabla en la base?
fn tabla_existe(conn: &Connection, tabla: &str) -> Result<bool, ErrorStore> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
        params![tabla],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// Las columnas de una tabla (los nombres de tabla son de una lista fija:
/// no hay inyección posible).
fn columnas_de(conn: &Connection, tabla: &str) -> Result<Vec<String>, ErrorStore> {
    let sql = format!("PRAGMA table_info({})", tabla);
    let mut stmt = conn.prepare(&sql)?;
    let filas = stmt.query_map([], |r| r.get::<_, String>(1))?;
    let mut out = Vec::new();
    for f in filas {
        out.push(f?);
    }
    Ok(out)
}

/// Guarda (upsert) una sesión y **anexa** el historial que falte.
///
/// * La fila de la sesión se **actualiza** (cwd, modo, estado, turnos…), pero
///   `created_at` **no** se toca y `current_epoch` sólo **sube** (para bajar
///   hay [`revertir_a_epoch`], que es explícito).
/// * El historial se anexa **sólo** con lo que no estaba (prefijo común):
///   guardar dos veces la misma sesión **no** duplica mensajes.
/// * **No borra nada** (patrón 4).
pub fn guardar_sesion(conn: &Connection, s: &SesionAcp) -> Result<(), ErrorStore> {
    validar_sesion(s)?;
    if esta_borrada(conn, &s.id)? {
        return Err(ErrorStore::SesionYaBorrada(s.id.clone()));
    }
    conn.execute(
        "INSERT INTO acp_sessions
            (id, cwd, modo, padre, estado, current_epoch, turnos, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
         ON CONFLICT(id) DO UPDATE SET
            cwd           = excluded.cwd,
            modo          = excluded.modo,
            padre         = excluded.padre,
            estado        = excluded.estado,
            current_epoch = MAX(acp_sessions.current_epoch, excluded.current_epoch),
            turnos        = excluded.turnos,
            updated_at    = excluded.updated_at",
        params![
            &s.id,
            &s.cwd,
            &s.modo,
            &s.padre,
            s.estado.nombre(),
            s.current_epoch as i64,
            s.turnos as i64,
            s.created_at,
            s.updated_at,
        ],
    )?;

    // El historial: anexar sólo el sufijo que no estaba.
    let existentes: Vec<String> = cargar_mensajes(conn, &s.id)?
        .iter()
        .map(|m| serializar(&m.acp_json))
        .collect();
    let nuevos: Vec<String> = s.mensajes.iter().map(serializar).collect();
    let anexar = nuevos_para_anexar(&existentes, &nuevos);
    if anexar.is_empty() {
        return Ok(());
    }
    let mut seq = siguiente_seq_de_sesion(conn, &s.id)?;
    for texto in anexar {
        let v: Value = serde_json::from_str(&texto).unwrap_or(Value::Null);
        let m = MensajeAcp::nuevo(
            &s.id,
            &rol_de_json(&v),
            &contenido_de_json(&v),
            v,
            s.current_epoch,
            seq,
        );
        insertar_mensaje(conn, &m)?;
        seq += 1;
    }
    Ok(())
}

/// Carga una sesión con su historial **completo** (todas las épocas).
///
/// Errores: [`ErrorStore::SesionNoExiste`] o [`ErrorStore::SesionYaBorrada`].
pub fn cargar_sesion(conn: &Connection, id: &str) -> Result<SesionAcp, ErrorStore> {
    let fila = conn
        .query_row(
            "SELECT id, cwd, modo, padre, estado, current_epoch, turnos, created_at, updated_at
             FROM acp_sessions WHERE id=?1",
            params![id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, i64>(6)?,
                    r.get::<_, i64>(7)?,
                    r.get::<_, i64>(8)?,
                ))
            },
        )
        .optional()?;
    let (id, cwd, modo, padre, estado, epoch, turnos, created_at, updated_at) = match fila {
        Some(f) => f,
        None => return Err(error_sesion_inexistente(conn, id)),
    };
    let mensajes: Vec<Value> = cargar_mensajes(conn, &id)?
        .iter()
        .map(|m| m.acp_json.clone())
        .collect();
    Ok(SesionAcp {
        id,
        cwd,
        modo,
        padre,
        estado: EstadoSesion::desde_nombre(&estado),
        current_epoch: epoch.max(0) as u32,
        turnos: turnos.max(0) as u32,
        mensajes,
        created_at,
        updated_at,
    })
}

/// Carga una sesión con sus mensajes **detallados** (con `seq` y época).
pub fn cargar_historial(conn: &Connection, id: &str) -> Result<HistorialSesion, ErrorStore> {
    let sesion = cargar_sesion(conn, id)?;
    let mensajes = cargar_mensajes(conn, id)?;
    Ok(HistorialSesion { sesion, mensajes })
}

/// Todos los mensajes de una sesión, en orden de `seq` (épocas incluidas).
pub fn cargar_mensajes(conn: &Connection, id: &str) -> Result<Vec<MensajeAcp>, ErrorStore> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, compaction_epoch, sequence_number, rol, contenido,
                es_resumen, acp_json, created_at
         FROM acp_messages WHERE session_id=?1
         ORDER BY sequence_number ASC, id ASC",
    )?;
    let filas = stmt.query_map(params![id], fila_a_mensaje)?;
    let mut out = Vec::new();
    for f in filas {
        out.push(f?);
    }
    Ok(out)
}

/// La **vista viva**: los mensajes de la época actual (patrón 4).
pub fn cargar_vista_viva(conn: &Connection, id: &str) -> Result<Vec<MensajeAcp>, ErrorStore> {
    let s = cargar_sesion(conn, id)?;
    let todos = cargar_mensajes(conn, id)?;
    Ok(todos
        .into_iter()
        .filter(|m| es_mensaje_vivo(m, s.current_epoch))
        .collect())
}

/// El historial de una época concreta (sigue consultable tras compactar).
pub fn cargar_historial_por_epoch(
    conn: &Connection,
    id: &str,
    epoch: u32,
) -> Result<Vec<MensajeAcp>, ErrorStore> {
    let todos = cargar_mensajes(conn, id)?;
    Ok(todos
        .into_iter()
        .filter(|m| es_mensaje_en_epoch(m, epoch))
        .collect())
}

/// Las épocas que existen en una sesión (en orden).
pub fn cargar_epochs(conn: &Connection, id: &str) -> Result<Vec<u32>, ErrorStore> {
    let todos = cargar_mensajes(conn, id)?;
    let mut v: Vec<u32> = todos.iter().map(|m| m.compaction_epoch).collect();
    v.sort_unstable();
    v.dedup();
    Ok(v)
}

/// El siguiente `seq` global de una sesión (no se reinicia nunca).
pub fn siguiente_seq_de_sesion(conn: &Connection, id: &str) -> Result<u64, ErrorStore> {
    let max: Option<i64> = conn
        .query_row(
            "SELECT MAX(sequence_number) FROM acp_messages WHERE session_id=?1",
            params![id],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    Ok(siguiente_seq(&[max.unwrap_or(0).max(0) as u64]))
}

/// Guarda (upsert) mensajes concretos, con su `seq` y su época.
///
/// * Un mensaje con `id = 0` es **nuevo**: si ya hay otro con la misma
///   clave `(session_id, compaction_epoch, sequence_number)` y **otro**
///   contenido ⇒ [`ErrorStore::MensajeDuplicado`]; si el contenido es
///   **igual**, es idempotente y no hace nada.
/// * Un mensaje con `id > 0` **actualiza** la fila existente (si no existe,
///   [`ErrorStore::MensajeNoExiste`]).
/// * **No borra nada**.
pub fn guardar_mensajes(
    conn: &Connection,
    session_id: &str,
    msgs: &[MensajeAcp],
) -> Result<(), ErrorStore> {
    exigir_sesion(conn, session_id)?;
    for m in msgs {
        validar_mensaje(m)?;
        if m.id > 0 {
            let n = conn.execute(
                "UPDATE acp_messages
                 SET compaction_epoch=?2, sequence_number=?3, rol=?4, contenido=?5,
                     es_resumen=?6, acp_json=?7, created_at=?8
                 WHERE id=?1 AND session_id=?9",
                params![
                    m.id,
                    m.compaction_epoch as i64,
                    m.sequence_number as i64,
                    &m.rol,
                    &m.contenido,
                    m.es_resumen as i64,
                    serializar(&m.acp_json),
                    m.created_at,
                    session_id,
                ],
            )?;
            if n == 0 {
                return Err(ErrorStore::MensajeNoExiste(m.id.to_string()));
            }
            continue;
        }
        let existente = buscar_por_clave(conn, session_id, m.compaction_epoch, m.sequence_number)?;
        match existente {
            Some(e) => {
                if fila_mensaje_igual(&e, m) {
                    // Idempotente: la misma fila, el mismo contenido.
                    continue;
                }
                return Err(ErrorStore::MensajeDuplicado {
                    session_id: session_id.to_string(),
                    compaction_epoch: m.compaction_epoch,
                    sequence_number: m.sequence_number,
                });
            }
            None => {
                insertar_mensaje(conn, m)?;
            }
        }
    }
    Ok(())
}

/// Anexa un mensaje a una sesión: le asigna la época actual y el siguiente
/// `seq` **global** (que no se reinicia).
pub fn añadir_mensaje(
    conn: &Connection,
    session_id: &str,
    rol: &str,
    contenido: &str,
    acp_json: &Value,
) -> Result<MensajeAcp, ErrorStore> {
    let s = cargar_sesion(conn, session_id)?;
    let seq = siguiente_seq_de_sesion(conn, session_id)?;
    let m = MensajeAcp::nuevo(
        session_id,
        rol,
        contenido,
        acp_json.clone(),
        s.current_epoch,
        seq,
    );
    let id = insertar_mensaje(conn, &m)?;
    Ok(MensajeAcp { id, ..m })
}

/// `session/list`: el resumen de todas las sesiones.
pub fn listar(conn: &Connection) -> Result<Vec<ResumenSesion>, ErrorStore> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.cwd, s.modo, s.padre, s.estado, s.current_epoch, s.turnos,
                s.created_at, s.updated_at,
                (SELECT COUNT(*) FROM acp_messages m WHERE m.session_id = s.id) AS n_mensajes
         FROM acp_sessions s
         ORDER BY s.created_at ASC, s.id ASC",
    )?;
    let filas = stmt.query_map([], |r| {
        Ok(ResumenSesion {
            id: r.get(0)?,
            cwd: r.get(1)?,
            modo: r.get(2)?,
            padre: r.get(3)?,
            estado: EstadoSesion::desde_nombre(&r.get::<_, String>(4)?),
            current_epoch: r.get::<_, i64>(5)?.max(0) as u32,
            turnos: r.get::<_, i64>(6)?.max(0) as u32,
            mensajes: r.get::<_, i64>(9)?.max(0) as usize,
            created_at: r.get(7)?,
            updated_at: r.get(8)?,
        })
    })?;
    let mut out = Vec::new();
    for f in filas {
        out.push(f?);
    }
    Ok(out)
}

/// **Borra** una sesión y su historial.
///
/// Es la **única** operación que elimina filas (decisión explícita):
/// compactar, guardar, revertir y cargar **no** borran nada.
///
/// Errors: [`ErrorStore::SesionNoExiste`] (nunca existió) o
/// [`ErrorStore::SesionYaBorrada`] (ya estaba borrada).  El borrado deja
/// **tumba** en `acp_sessions_borradas` para poder distinguir los dos casos.
pub fn borrar_sesion(conn: &Connection, id: &str) -> Result<Borrado, ErrorStore> {
    if !existe_sesion(conn, id)? {
        return Err(error_sesion_inexistente(conn, id));
    }
    let tx = conn.unchecked_transaction()?;
    let mensajes = tx.execute("DELETE FROM acp_messages WHERE session_id=?1", params![id])?;
    let extensiones = tx.execute(
        "DELETE FROM acp_session_extensions WHERE session_id=?1",
        params![id],
    )?;
    tx.execute("DELETE FROM acp_sessions WHERE id=?1", params![id])?;
    tx.execute(
        "INSERT INTO acp_sessions_borradas(session_id, borrada_at) VALUES(?1,?2)
         ON CONFLICT(session_id) DO UPDATE SET borrada_at=excluded.borrada_at",
        params![id, ahora()],
    )?;
    tx.commit()?;
    Ok(Borrado {
        session_id: id.to_string(),
        mensajes,
        extensiones,
    })
}

/// **Compacta SIN BORRAR**: sella la época actual y abre una nueva con un
/// resumen (patrón 4).
///
/// * Los mensajes anteriores **no se borran**: siguen con su época.
/// * La época nueva empieza con el resumen.
/// * El `seq` es global y **no se reinicia**.
/// * Bajo presupuesto **no** se compacta (costaría más de lo que ahorra).
pub fn compactar(
    conn: &Connection,
    id: &str,
    resumen: &str,
    presupuesto_chars: usize,
) -> Result<Compactacion, ErrorStore> {
    let s = cargar_sesion(conn, id)?;
    let todos = cargar_mensajes(conn, id)?;
    let vista: Vec<&MensajeAcp> = todos
        .iter()
        .filter(|m| es_mensaje_vivo(m, s.current_epoch))
        .collect();
    let chars_vivos: usize = vista.iter().map(|m| m.chars()).sum();
    let sellados: Vec<&&MensajeAcp> = vista.iter().filter(|m| !m.es_resumen).collect();
    let chars_sellados: usize = sellados.iter().map(|m| m.chars()).sum();
    let chars_resumen = resumen.chars().count();

    let plan = plan_compactacion(
        chars_vivos,
        presupuesto_chars,
        chars_sellados,
        chars_resumen,
    );
    let ahorro = match plan {
        None => {
            return Ok(Compactacion {
                epoch_nueva: s.current_epoch,
                epoch_sellada: s.current_epoch,
                mensajes_sellados: 0,
                chars_ahorrados: 0,
                mensajes_totales: todos.len(),
            })
        }
        Some(a) => a,
    };

    let nueva = s.current_epoch + 1;
    let seq = siguiente_seq_de_sesion(conn, id)?;
    let m = MensajeAcp::nuevo(
        id,
        "resumen",
        resumen,
        json!({"role": "resumen", "content": resumen}),
        nueva,
        seq,
    );
    let mut m = m;
    m.es_resumen = true;
    let tx = conn.unchecked_transaction()?;
    insertar_mensaje(&tx, &m)?;
    tx.execute(
        "UPDATE acp_sessions SET current_epoch=?2, updated_at=?3 WHERE id=?1",
        params![id, nueva as i64, ahora()],
    )?;
    tx.commit()?;
    Ok(Compactacion {
        epoch_nueva: nueva,
        epoch_sellada: s.current_epoch,
        mensajes_sellados: sellados.len(),
        chars_ahorrados: ahorro,
        mensajes_totales: todos.len() + 1,
    })
}

/// **Revierte** a una época anterior: el historial sigue intacto, sólo
/// cambia la época en la que se está trabajando (patrón 4).
pub fn revertir_a_epoch(conn: &Connection, id: &str, epoch: u32) -> Result<(), ErrorStore> {
    let s = cargar_sesion(conn, id)?;
    let epochs = cargar_epochs(conn, id)?;
    if !epochs.contains(&epoch) {
        return Err(ErrorStore::DatosInvalidos(format!(
            "la sesión {} no tiene la época {}",
            id, epoch
        )));
    }
    conn.execute(
        "UPDATE acp_sessions SET current_epoch=?2, updated_at=?3 WHERE id=?1",
        params![id, epoch as i64, ahora()],
    )?;
    let _ = s; // la carga valida que la sesión existe
    Ok(())
}

/// Guarda una **extensión** de la sesión (clave → valor JSON).
pub fn guardar_extension(
    conn: &Connection,
    session_id: &str,
    key: &str,
    value: &Value,
) -> Result<(), ErrorStore> {
    exigir_sesion(conn, session_id)?;
    conn.execute(
        "INSERT INTO acp_session_extensions(session_id, key, value, updated_at)
         VALUES(?1,?2,?3,?4)
         ON CONFLICT(session_id, key) DO UPDATE SET
            value=excluded.value, updated_at=excluded.updated_at",
        params![session_id, key, serializar(value), ahora()],
    )?;
    Ok(())
}

/// Lee una extensión de la sesión (`None` si no está).
pub fn cargar_extension(
    conn: &Connection,
    session_id: &str,
    key: &str,
) -> Result<Option<Value>, ErrorStore> {
    let txt: Option<String> = conn
        .query_row(
            "SELECT value FROM acp_session_extensions WHERE session_id=?1 AND key=?2",
            params![session_id, key],
            |r| r.get(0),
        )
        .optional()?;
    Ok(txt.map(|t| serde_json::from_str::<Value>(&t).unwrap_or(Value::Null)))
}

/// Quita la **tumba** de una sesión borrada (resucitar es explícito).
pub fn limpiar_tumba(conn: &Connection, id: &str) -> Result<(), ErrorStore> {
    conn.execute(
        "DELETE FROM acp_sessions_borradas WHERE session_id=?1",
        params![id],
    )?;
    Ok(())
}

// ── Auxiliares internos ──────────────────────────────────────────────────

/// Convierte una fila de `acp_messages` en un [`MensajeAcp`].
fn fila_a_mensaje(r: &rusqlite::Row<'_>) -> rusqlite::Result<MensajeAcp> {
    let epoch: i64 = r.get("compaction_epoch")?;
    let seq: i64 = r.get("sequence_number")?;
    let es_resumen: i64 = r.get("es_resumen")?;
    let json_txt: String = r.get("acp_json")?;
    Ok(MensajeAcp {
        id: r.get("id")?,
        session_id: r.get("session_id")?,
        compaction_epoch: epoch.max(0) as u32,
        sequence_number: seq.max(0) as u64,
        rol: r.get("rol")?,
        contenido: r.get("contenido")?,
        es_resumen: es_resumen != 0,
        acp_json: serde_json::from_str::<Value>(&json_txt).unwrap_or(Value::Null),
        created_at: r.get("created_at")?,
    })
}

/// Inserta un mensaje (devuelve el id de fila nuevo).
fn insertar_mensaje(conn: &Connection, m: &MensajeAcp) -> Result<i64, ErrorStore> {
    match conn.execute(
        "INSERT INTO acp_messages
            (session_id, compaction_epoch, sequence_number, rol, contenido,
             es_resumen, acp_json, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            &m.session_id,
            m.compaction_epoch as i64,
            m.sequence_number as i64,
            &m.rol,
            &m.contenido,
            m.es_resumen as i64,
            serializar(&m.acp_json),
            m.created_at,
        ],
    ) {
        Ok(_) => Ok(conn.last_insert_rowid()),
        Err(e) => match e {
            rusqlite::Error::SqliteFailure(ref inner, _)
                if inner.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                Err(ErrorStore::MensajeDuplicado {
                    session_id: m.session_id.clone(),
                    compaction_epoch: m.compaction_epoch,
                    sequence_number: m.sequence_number,
                })
            }
            other => Err(ErrorStore::from(other)),
        },
    }
}

/// Busca un mensaje por su clave `(session, epoch, seq)`.
fn buscar_por_clave(
    conn: &Connection,
    session_id: &str,
    epoch: u32,
    seq: u64,
) -> Result<Option<MensajeAcp>, ErrorStore> {
    conn.query_row(
        "SELECT id, session_id, compaction_epoch, sequence_number, rol, contenido,
                es_resumen, acp_json, created_at
         FROM acp_messages
         WHERE session_id=?1 AND compaction_epoch=?2 AND sequence_number=?3",
        params![session_id, epoch as i64, seq as i64],
        fila_a_mensaje,
    )
    .optional()
    .map_err(ErrorStore::from)
}

/// ¿Existe la sesión?
fn existe_sesion(conn: &Connection, id: &str) -> Result<bool, ErrorStore> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM acp_sessions WHERE id=?1",
        params![id],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// ¿La sesión está en la tumba (se borró explícitamente)?
fn esta_borrada(conn: &Connection, id: &str) -> Result<bool, ErrorStore> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM acp_sessions_borradas WHERE session_id=?1",
        params![id],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// El error correcto según la sesión **no** exista: ¿nunca existió o fue
/// borrada?
fn error_sesion_inexistente(conn: &Connection, id: &str) -> ErrorStore {
    match esta_borrada(conn, id) {
        Ok(true) => ErrorStore::SesionYaBorrada(id.to_string()),
        _ => ErrorStore::SesionNoExiste(id.to_string()),
    }
}

/// Exige que la sesión exista (si no, el error tipado correcto).
fn exigir_sesion(conn: &Connection, id: &str) -> Result<(), ErrorStore> {
    if existe_sesion(conn, id)? {
        Ok(())
    } else {
        Err(error_sesion_inexistente(conn, id))
    }
}

// =============================================================================
// EL STORE (la conexión y el fichero)
// =============================================================================

/// El store de sesiones ACP sobre SQLite.
///
/// El **patrón de conexión** es el de `board.rs`: fichero en el `state_dir`,
/// `busy_timeout`, WAL con degradación a DELETE si el sistema de ficheros no
/// lo soporta (una `state_dir` en NFS), `synchronous=FULL` y
/// `foreign_keys=ON`.
#[derive(Debug)]
pub struct AcpSesionStore {
    conn: Connection,
    path: Option<PathBuf>,
}

impl AcpSesionStore {
    /// Abre (o crea) el almacén en `path`.
    ///
    /// Crea el directorio si hace falta, aplica el esquema (idempotente) y
    /// **verifica** que la base tiene la forma esperada.
    pub fn open(path: &Path) -> Result<AcpSesionStore, ErrorStore> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path)?;
        AcpSesionStore::configurar(&conn)?;
        let store = AcpSesionStore {
            conn,
            path: Some(path.to_path_buf()),
        };
        // Primero se **verifica** (una base que ya existe y tiene otra
        // forma ⇒ error tipado, no un fallo de SQLite al crear un índice),
        // después se aplica el esquema (idempotente) y se vuelve a
        // verificar.
        store.verificar_esquema()?;
        store.aplicar_esquema()?;
        store.verificar_esquema()?;
        Ok(store)
    }

    /// Abre el almacén en el `state_dir` de la instancia.
    ///
    /// La ruta es `state_dir()/acp_sesiones.sqlite` (`state_dir::state_dir()`
    /// es la fuente única de verdad de la ruta de estado).
    pub fn abrir_en_state_dir() -> Result<AcpSesionStore, ErrorStore> {
        let dir = PathBuf::from(crate::state_dir::state_dir());
        AcpSesionStore::open(&dir.join(ACPS_DB_FILE))
    }

    /// Un almacén **en memoria** (para tests y para el modo efímero).
    pub fn en_memoria() -> Result<AcpSesionStore, ErrorStore> {
        let conn = Connection::open_in_memory()?;
        AcpSesionStore::configurar(&conn)?;
        let store = AcpSesionStore { conn, path: None };
        store.aplicar_esquema()?;
        store.verificar_esquema()?;
        Ok(store)
    }

    /// Envuelve una **conexión ya abierta** (por si el root quiere vivir en
    /// la misma base que otra cosa).  Aplica el esquema y lo verifica.
    pub fn con_conexion(conn: Connection) -> Result<AcpSesionStore, ErrorStore> {
        AcpSesionStore::configurar(&conn)?;
        let store = AcpSesionStore { conn, path: None };
        store.aplicar_esquema()?;
        store.verificar_esquema()?;
        Ok(store)
    }

    /// Configura la conexión (igual que `board.rs`).
    fn configurar(conn: &Connection) -> Result<(), ErrorStore> {
        conn.busy_timeout(std::time::Duration::from_millis(BUSY_TIMEOUT_MS as u64))?;
        // WAL: lo mejor para muchos escritores concurrentes.  **No** todos
        // los sistemas de ficheros lo soportan (una `state_dir` en NFS): si
        // falla, se sigue en DELETE (más lento pero correcto).
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        let _ = conn.pragma_update(None, "synchronous", "FULL");
        let _ = conn.pragma_update(None, "foreign_keys", "ON");
        Ok(())
    }

    /// La conexión (por si el llamante necesita SQL propio).
    pub fn conexion(&self) -> &Connection {
        &self.conn
    }

    /// La ruta del fichero (si es persistente).
    pub fn ruta(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Aplica el esquema (idempotente).
    pub fn aplicar_esquema(&self) -> Result<(), ErrorStore> {
        aplicar_esquema(&self.conn)
    }

    /// Verifica que la base tiene la forma esperada.
    pub fn verificar_esquema(&self) -> Result<(), ErrorStore> {
        verificar_esquema(&self.conn)
    }

    /// Guarda (upsert) una sesión y anexa el historial que falte.
    pub fn guardar_sesion(&self, s: &SesionAcp) -> Result<(), ErrorStore> {
        guardar_sesion(&self.conn, s)
    }

    /// Carga una sesión con su historial completo.
    pub fn cargar_sesion(&self, id: &str) -> Result<SesionAcp, ErrorStore> {
        cargar_sesion(&self.conn, id)
    }

    /// Carga una sesión con sus mensajes detallados.
    pub fn cargar_historial(&self, id: &str) -> Result<HistorialSesion, ErrorStore> {
        cargar_historial(&self.conn, id)
    }

    /// Todos los mensajes (épocas incluidas).
    pub fn cargar_mensajes(&self, id: &str) -> Result<Vec<MensajeAcp>, ErrorStore> {
        cargar_mensajes(&self.conn, id)
    }

    /// La vista viva (la época actual).
    pub fn cargar_vista_viva(&self, id: &str) -> Result<Vec<MensajeAcp>, ErrorStore> {
        cargar_vista_viva(&self.conn, id)
    }

    /// El historial de una época concreta.
    pub fn cargar_historial_por_epoch(
        &self,
        id: &str,
        epoch: u32,
    ) -> Result<Vec<MensajeAcp>, ErrorStore> {
        cargar_historial_por_epoch(&self.conn, id, epoch)
    }

    /// Las épocas que existen.
    pub fn cargar_epochs(&self, id: &str) -> Result<Vec<u32>, ErrorStore> {
        cargar_epochs(&self.conn, id)
    }

    /// Guarda (upsert) mensajes concretos.
    pub fn guardar_mensajes(
        &self,
        session_id: &str,
        msgs: &[MensajeAcp],
    ) -> Result<(), ErrorStore> {
        guardar_mensajes(&self.conn, session_id, msgs)
    }

    /// Anexa un mensaje (le asigna época y `seq`).
    pub fn añadir_mensaje(
        &self,
        session_id: &str,
        rol: &str,
        contenido: &str,
        acp_json: &Value,
    ) -> Result<MensajeAcp, ErrorStore> {
        añadir_mensaje(&self.conn, session_id, rol, contenido, acp_json)
    }

    /// Compacta **sin borrar**.
    pub fn compactar(
        &self,
        id: &str,
        resumen: &str,
        presupuesto_chars: usize,
    ) -> Result<Compactacion, ErrorStore> {
        compactar(&self.conn, id, resumen, presupuesto_chars)
    }

    /// Revierte a una época anterior (sin tocar el historial).
    pub fn revertir_a_epoch(&self, id: &str, epoch: u32) -> Result<(), ErrorStore> {
        revertir_a_epoch(&self.conn, id, epoch)
    }

    /// El resumen de todas las sesiones.
    pub fn listar(&self) -> Result<Vec<ResumenSesion>, ErrorStore> {
        listar(&self.conn)
    }

    /// **Borra** una sesión y su historial (decisión explícita).
    pub fn borrar_sesion(&self, id: &str) -> Result<Borrado, ErrorStore> {
        borrar_sesion(&self.conn, id)
    }

    /// Guarda una extensión de la sesión.
    pub fn guardar_extension(
        &self,
        session_id: &str,
        key: &str,
        value: &Value,
    ) -> Result<(), ErrorStore> {
        guardar_extension(&self.conn, session_id, key, value)
    }

    /// Lee una extensión de la sesión.
    pub fn cargar_extension(
        &self,
        session_id: &str,
        key: &str,
    ) -> Result<Option<Value>, ErrorStore> {
        cargar_extension(&self.conn, session_id, key)
    }

    /// Quita la tumba de una sesión borrada.
    pub fn limpiar_tumba(&self, id: &str) -> Result<(), ErrorStore> {
        limpiar_tumba(&self.conn, id)
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Una ruta temporal **por test** (los tests corren en paralelo).
    fn ruta_temporal(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("acp-store-{}-{}", std::process::id(), nombre));
        let _ = std::fs::create_dir_all(&d);
        d.join(ACPS_DB_FILE)
    }

    fn store(nombre: &str) -> AcpSesionStore {
        AcpSesionStore::open(&ruta_temporal(nombre)).expect("abre el store")
    }

    fn prompt(texto: &str) -> Value {
        json!({"role": "user", "content": [{"type": "text", "text": texto}]})
    }

    fn respuesta(texto: &str) -> Value {
        json!({"role": "assistant", "content": [{"type": "text", "text": texto}]})
    }

    fn sesion_con_historial(id: &str) -> SesionAcp {
        let mut s = SesionAcp::nueva(id, "/tmp/proyecto");
        s.mensajes = vec![prompt("hola"), respuesta("buenas"), prompt("más cosas")];
        s
    }

    #[test]
    fn el_esquema_es_idempotente() {
        let s = store("esquema");
        // Aplicarlo dos veces no falla.
        s.aplicar_esquema().expect("1ª");
        s.aplicar_esquema().expect("2ª");
        s.aplicar_esquema().expect("3ª");
        // Y las tablas están.
        let n: i64 = s
            .conexion()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name LIKE 'acp_%'",
                [],
                |r| r.get(0),
            )
            .expect("cuenta");
        assert_eq!(n, 5, "las 5 tablas del store: {}", n);
        // Y los índices.
        let i: i64 = s
            .conexion()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name LIKE 'idx_acp_%'",
                [],
                |r| r.get(0),
            )
            .expect("cuenta");
        assert_eq!(i, 3, "los 3 índices del store: {}", i);
    }

    #[test]
    fn guardar_y_cargar_devuelve_lo_mismo() {
        let s = store("roundtrip");
        let original = sesion_con_historial("ses-1");
        s.guardar_sesion(&original).expect("guarda");
        let leida = s.cargar_sesion("ses-1").expect("carga");
        assert_eq!(leida.id, original.id);
        assert_eq!(leida.cwd, original.cwd);
        assert_eq!(leida.modo, original.modo);
        assert_eq!(leida.padre, original.padre);
        assert_eq!(leida.estado, original.estado);
        assert_eq!(leida.turnos, original.turnos);
        assert_eq!(leida.current_epoch, original.current_epoch);
        assert_eq!(
            leida.mensajes, original.mensajes,
            "el historial ACP vuelve igual"
        );
    }

    #[test]
    fn guardar_dos_veces_no_duplica_el_historial() {
        let s = store("idempotente");
        let original = sesion_con_historial("ses-2");
        s.guardar_sesion(&original).expect("1º");
        s.guardar_sesion(&original).expect("2º");
        s.guardar_sesion(&original).expect("3º");
        let h = s.cargar_historial("ses-2").expect("historial");
        assert_eq!(h.mensajes.len(), 3, "no se duplica: {}", h.mensajes.len());
        // Y los seq son 1, 2, 3.
        let seqs: Vec<u64> = h.mensajes.iter().map(|m| m.sequence_number).collect();
        assert_eq!(seqs, vec![1, 2, 3]);
    }

    #[test]
    fn el_seq_es_global_y_no_se_reinicia() {
        let ruta = ruta_temporal("seq");
        {
            let s = AcpSesionStore::open(&ruta).expect("abre");
            s.guardar_sesion(&SesionAcp::nueva("ses-3", "/tmp"))
                .expect("guarda");
            s.añadir_mensaje("ses-3", "user", "uno", &prompt("uno"))
                .expect("1");
            s.añadir_mensaje("ses-3", "assistant", "dos", &respuesta("dos"))
                .expect("2");
        }
        // Se **reabre** la base: el contador no se reinicia.
        {
            let s = AcpSesionStore::open(&ruta).expect("reabre");
            let m = s
                .añadir_mensaje("ses-3", "user", "tres", &prompt("tres"))
                .expect("3");
            assert_eq!(m.sequence_number, 3, "el seq sigue: {}", m.sequence_number);
        }
        // Y tras compactar tampoco se reinicia.
        let s = AcpSesionStore::open(&ruta).expect("reabre");
        s.compactar("ses-3", "resumen", 1).expect("compacta");
        let m = s
            .añadir_mensaje("ses-3", "user", "cuatro", &prompt("cuatro"))
            .expect("4");
        assert_eq!(
            m.sequence_number, 5,
            "tras compactar sigue: {}",
            m.sequence_number
        );
        let h = s.cargar_historial("ses-3").expect("historial");
        let seqs: Vec<u64> = h.mensajes.iter().map(|m| m.sequence_number).collect();
        assert_eq!(
            seqs,
            vec![1, 2, 3, 4, 5],
            "el seq es global y no se reinicia"
        );
        // El resumen abre la época 1 y NO borra los anteriores.
        assert_eq!(h.mensajes[3].compaction_epoch, 1);
        assert!(h.mensajes[3].es_resumen);
        assert_eq!(h.mensajes[4].compaction_epoch, 1);
    }

    #[test]
    fn la_vista_viva_es_la_epoch_actual_y_el_historial_completo_sigue_consultable() {
        let s = store("epochs");
        s.guardar_sesion(&SesionAcp::nueva("ses-4", "/tmp"))
            .expect("guarda");
        s.añadir_mensaje("ses-4", "user", "hola", &prompt("hola"))
            .expect("1");
        s.añadir_mensaje("ses-4", "assistant", "buenas", &respuesta("buenas"))
            .expect("2");
        // Compactar: sella la época 0 y abre la 1.  **No borra nada**.
        let c = s
            .compactar("ses-4", "resumen de la conversación", 1)
            .expect("compacta");
        assert_eq!(c.epoch_sellada, 0);
        assert_eq!(c.epoch_nueva, 1);
        assert_eq!(c.mensajes_sellados, 2);
        // La vista viva es **sólo** la época nueva.
        let viva = s.cargar_vista_viva("ses-4").expect("vista");
        assert_eq!(viva.len(), 1, "sólo el resumen");
        assert!(viva[0].es_resumen);
        // La época 0 sigue consultable.
        let e0 = s.cargar_historial_por_epoch("ses-4", 0).expect("epoch 0");
        assert_eq!(e0.len(), 2, "la epoch 0 no se borró");
        // Y el historial completo tiene los 3.
        let todos = s.cargar_mensajes("ses-4").expect("todos");
        assert_eq!(todos.len(), 3);
        assert_eq!(s.cargar_epochs("ses-4").expect("epochs"), vec![0, 1]);
    }

    #[test]
    fn listar_devuelve_el_resumen_correcto() {
        let s = store("listar");
        let mut a = SesionAcp::nueva("ses-a", "/tmp/a");
        a.turnos = 3;
        a.estado = EstadoSesion::Terminada;
        s.guardar_sesion(&a).expect("guarda a");
        s.añadir_mensaje("ses-a", "user", "x", &prompt("x"))
            .expect("1");
        s.añadir_mensaje("ses-a", "user", "y", &prompt("y"))
            .expect("2");
        let mut b = SesionAcp::nueva("ses-b", "/tmp/b");
        b.modo = Some("plan".into());
        b.padre = Some("ses-a".into());
        s.guardar_sesion(&b).expect("guarda b");

        let lista = s.listar().expect("lista");
        assert_eq!(lista.len(), 2);
        let ra = lista.iter().find(|r| r.id == "ses-a").expect("ses-a");
        assert_eq!(ra.cwd, "/tmp/a");
        assert_eq!(ra.estado, EstadoSesion::Terminada);
        assert_eq!(ra.turnos, 3);
        assert_eq!(ra.mensajes, 2, "el nº de mensajes");
        assert_eq!(ra.current_epoch, 0);
        let rb = lista.iter().find(|r| r.id == "ses-b").expect("ses-b");
        assert_eq!(rb.mensajes, 0);
        assert_eq!(rb.modo.as_deref(), Some("plan"));
        assert_eq!(rb.padre.as_deref(), Some("ses-a"));
        // Y el resumen JSON tiene los campos del modelo en memoria.
        let j = ra.a_json();
        assert_eq!(j["sessionId"], json!("ses-a"));
        assert_eq!(j["estado"], json!("terminada"));
        assert_eq!(j["mensajes"], json!(2));
    }

    #[test]
    fn borrar_quita_la_sesion_de_la_lista_y_su_historial() {
        let s = store("borrar");
        s.guardar_sesion(&sesion_con_historial("ses-5"))
            .expect("guarda");
        assert_eq!(s.listar().expect("lista").len(), 1);
        let b = s.borrar_sesion("ses-5").expect("borra");
        assert_eq!(b.mensajes, 3, "borra los 3 mensajes");
        assert_eq!(
            s.listar().expect("lista").len(),
            0,
            "ya no está en la lista"
        );
        assert!(s.cargar_mensajes("ses-5").expect("mensajes").is_empty());
        // Y el error es el TIPADO: fue borrada (no "nunca existió").
        match s.cargar_sesion("ses-5") {
            Err(ErrorStore::SesionYaBorrada(id)) => assert_eq!(id, "ses-5"),
            otro => panic!("esperaba SesionYaBorrada, {:?}", otro),
        }
    }

    #[test]
    fn ni_guardar_ni_compactar_borran_nada() {
        let s = store("noborrar");
        s.guardar_sesion(&sesion_con_historial("ses-6"))
            .expect("guarda");
        // Guardar otra vez (upsert) no borra el historial.
        let mut otra = s.cargar_sesion("ses-6").expect("carga");
        otra.cwd = "/tmp/otro".into();
        otra.turnos = 1;
        s.guardar_sesion(&otra).expect("guarda otra vez");
        assert_eq!(s.cargar_mensajes("ses-6").expect("mensajes").len(), 3);
        // Compactar no borra: sellar una época deja los mensajes donde están.
        s.compactar("ses-6", "resumen", 1).expect("compacta");
        assert_eq!(s.cargar_mensajes("ses-6").expect("mensajes").len(), 4);
        // Y la sesión sigue en la lista.
        assert_eq!(s.listar().expect("lista").len(), 1);
        // Sólo el borrado explícito quita filas.
        s.borrar_sesion("ses-6").expect("borra");
        assert_eq!(s.cargar_mensajes("ses-6").expect("mensajes").len(), 0);
    }

    #[test]
    fn error_tipado_de_sesion_inexistente() {
        let s = store("inexistente");
        match s.cargar_sesion("no-existe") {
            Err(ErrorStore::SesionNoExiste(id)) => assert_eq!(id, "no-existe"),
            otro => panic!("esperaba SesionNoExiste, {:?}", otro),
        }
        match s.cargar_vista_viva("no-existe") {
            Err(ErrorStore::SesionNoExiste(_)) => {}
            otro => panic!("esperaba SesionNoExiste, {:?}", otro),
        }
        match s.borrar_sesion("no-existe") {
            Err(ErrorStore::SesionNoExiste(_)) => {}
            otro => panic!("esperaba SesionNoExiste, {:?}", otro),
        }
        match s.añadir_mensaje("no-existe", "user", "x", &prompt("x")) {
            Err(ErrorStore::SesionNoExiste(_)) => {}
            otro => panic!("esperaba SesionNoExiste, {:?}", otro),
        }
    }

    #[test]
    fn error_tipado_de_mensaje_duplicado() {
        let s = store("duplicado");
        s.guardar_sesion(&SesionAcp::nueva("ses-7", "/tmp"))
            .expect("guarda");
        let m = s
            .añadir_mensaje("ses-7", "user", "hola", &prompt("hola"))
            .expect("1");
        // La misma clave con **otro** contenido ⇒ duplicado.
        let otro = MensajeAcp::nuevo("ses-7", "user", "otro", prompt("otro"), 0, 1);
        match s.guardar_mensajes("ses-7", &[otro]) {
            Err(ErrorStore::MensajeDuplicado {
                session_id,
                compaction_epoch,
                sequence_number,
            }) => {
                assert_eq!(session_id, "ses-7");
                assert_eq!(compaction_epoch, 0);
                assert_eq!(sequence_number, 1);
            }
            otro => panic!("esperaba MensajeDuplicado, {:?}", otro),
        }
        // La misma clave con el **mismo** contenido ⇒ idempotente.
        let igual = MensajeAcp::nuevo("ses-7", "user", "hola", prompt("hola"), 0, 1);
        s.guardar_mensajes("ses-7", &[igual]).expect("idempotente");
        assert_eq!(s.cargar_mensajes("ses-7").expect("mensajes").len(), 1);
        // Y el mensaje guardado se puede actualizar en su sitio.
        let mut cambio = m.clone();
        cambio.id = m.id;
        cambio.contenido = "hola editado".into();
        s.guardar_mensajes("ses-7", &[cambio]).expect("actualiza");
        let h = s.cargar_historial("ses-7").expect("historial");
        assert_eq!(h.mensajes.len(), 1);
        assert_eq!(h.mensajes[0].contenido, "hola editado");
    }

    #[test]
    fn una_sesion_con_cwd_inexistente_y_datos_raros_se_guarda_y_se_recupera() {
        let s = store("raros");
        let mut x = SesionAcp::nueva("ses-8", "/no/existe/este/directorio/nunca");
        x.modo = None;
        x.padre = None;
        x.mensajes = vec![
            json!({"role": "user", "content": [{"type": "text", "text": "acentúa ñ 🦀"}]}),
            json!({"role": "assistant", "stopReason": "EndTurn"}),
            json!({"sessionUpdate": "agent_message_chunk",
                   "content": {"type": "text", "text": "linea1\nlinea2"}}),
        ];
        s.guardar_sesion(&x).expect("guarda");
        let leida = s.cargar_sesion("ses-8").expect("carga");
        assert_eq!(leida.cwd, "/no/existe/este/directorio/nunca");
        assert_eq!(leida.modo, None);
        assert_eq!(leida.padre, None);
        assert_eq!(leida.mensajes, x.mensajes, "vuelve byte a byte");
        // Y el rol/contenido derivados son razonables.
        let h = s.cargar_historial("ses-8").expect("historial");
        assert_eq!(h.mensajes[0].rol, "user");
        assert!(h.mensajes[0].contenido.contains("acentúa"));
        assert_eq!(h.mensajes[1].rol, "assistant");
        assert_eq!(h.mensajes[2].rol, "assistant", "el update es del agente");
        assert!(h.mensajes[2].contenido.contains("linea2"));
    }

    #[test]
    fn un_esquema_incompatible_da_error_tipado() {
        let ruta = ruta_temporal("incompatible");
        {
            // Una base con la tabla correcta pero **otra forma**.
            let conn = Connection::open(&ruta).expect("abre");
            conn.execute_batch(
                "CREATE TABLE acp_sessions (id TEXT PRIMARY KEY, cwd TEXT NOT NULL);
                 CREATE TABLE acp_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO acp_meta(key,value) VALUES('schema_version','1');",
            )
            .expect("crea el esquema raro");
        }
        let e = match AcpSesionStore::open(&ruta) {
            Ok(_) => panic!("debe fallar: la base no tiene el esquema"),
            Err(e) => e,
        };
        match e {
            ErrorStore::EsquemaIncompatible(m) => {
                assert!(m.contains("acp_sessions"), "el error dice qué tabla: {}", m)
            }
            otro => panic!("esperaba EsquemaIncompatible, {:?}", otro),
        }
    }

    #[test]
    fn un_fichero_que_no_es_una_base_da_error_tipado() {
        let ruta = ruta_temporal("no-base");
        std::fs::write(&ruta, b"esto no es una base de datos").expect("escribe");
        let e = match AcpSesionStore::open(&ruta) {
            Ok(_) => panic!("debe fallar: el fichero no es una base"),
            Err(e) => e,
        };
        match e {
            ErrorStore::EsquemaIncompatible(_) => {}
            otro => panic!("esperaba EsquemaIncompatible, {:?}", otro),
        }
    }

    #[test]
    fn las_extensiones_de_sesion_se_guardan_y_se_recuperan() {
        let s = store("extensiones");
        s.guardar_sesion(&SesionAcp::nueva("ses-9", "/tmp"))
            .expect("guarda");
        s.guardar_extension("ses-9", "modelo", &json!({"id": "x"}))
            .expect("guarda");
        s.guardar_extension("ses-9", "modelo", &json!({"id": "y"}))
            .expect("sobrescribe");
        let v = s.cargar_extension("ses-9", "modelo").expect("lee");
        assert_eq!(v, Some(json!({"id": "y"})));
        assert_eq!(s.cargar_extension("ses-9", "nada").expect("lee"), None);
        // Y al borrar la sesión, las extensiones van con ella.
        s.borrar_sesion("ses-9").expect("borra");
        assert_eq!(s.cargar_extension("ses-9", "modelo").expect("lee"), None);
    }

    #[test]
    fn revertir_de_epoca_no_toca_el_historial() {
        let s = store("revertir");
        s.guardar_sesion(&SesionAcp::nueva("ses-10", "/tmp"))
            .expect("guarda");
        s.añadir_mensaje("ses-10", "user", "hola", &prompt("hola"))
            .expect("1");
        s.compactar("ses-10", "resumen", 1).expect("compacta");
        assert_eq!(s.cargar_vista_viva("ses-10").expect("vista").len(), 1);
        s.revertir_a_epoch("ses-10", 0).expect("revierte");
        let viva = s.cargar_vista_viva("ses-10").expect("vista");
        assert_eq!(viva.len(), 1, "vuelve a la epoch 0");
        assert_eq!(viva[0].rol, "user");
        assert_eq!(
            s.cargar_mensajes("ses-10").expect("mensajes").len(),
            2,
            "no se borró"
        );
        // Revertir a una época que no existe es un error.
        assert!(s.revertir_a_epoch("ses-10", 99).is_err());
    }

    #[test]
    fn las_funciones_puras_dicen_lo_que_deben() {
        // El `seq` global.
        assert_eq!(siguiente_seq(&[]), 1);
        assert_eq!(siguiente_seq(&[1, 2, 3]), 4);
        assert_eq!(siguiente_seq(&[9]), 10);
        // La vista viva.
        let m = MensajeAcp::nuevo("s", "user", "x", json!("null"), 1, 5);
        assert!(es_mensaje_vivo(&m, 1));
        assert!(!es_mensaje_vivo(&m, 0));
        assert!(es_mensaje_en_epoch(&m, 1));
        // La compactación (patrón 4): bajo presupuesto no se compacta.
        assert_eq!(plan_compactacion(10, 100, 10, 3), None);
        assert_eq!(plan_compactacion(200, 100, 200, 50), Some(150));
        // Y si el resumen es peor que el historial, el ahorro es 0.
        assert_eq!(plan_compactacion(200, 100, 200, 500), Some(0));
        // El prefijo común (idempotencia).
        let a = vec!["1".to_string(), "2".to_string()];
        let b = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        assert!(nuevos_para_anexar(&a, &a).is_empty());
        assert_eq!(nuevos_para_anexar(&a, &b), vec!["3".to_string()]);
        assert_eq!(
            nuevos_para_anexar(&[], &b).len(),
            3,
            "sin histórico, todo es nuevo"
        );
        // El rol y el contenido derivados de un `Value` ACP.
        assert_eq!(rol_de_json(&json!({"role": "user"})), "user");
        assert_eq!(
            rol_de_json(&json!({"sessionUpdate": "agent_thought_chunk"})),
            "thought"
        );
        assert_eq!(
            rol_de_json(&json!({"sessionUpdate": "lo_que_sea"})),
            "update"
        );
        assert_eq!(
            contenido_de_json(
                &json!({"role": "user", "content": [{"type": "text", "text": "hola"}]})
            ),
            "hola"
        );
        assert_eq!(
            contenido_de_json(&json!({"role": "assistant", "stopReason": "EndTurn"})),
            "stopReason: EndTurn"
        );
    }

    #[test]
    fn el_bosque_y_el_estado_de_la_sesion_se_persisten() {
        let s = store("estado");
        let mut x = SesionAcp::nueva("ses-11", "/tmp");
        x.estado = EstadoSesion::Cancelada;
        x.turnos = 7;
        s.guardar_sesion(&x).expect("guarda");
        let leida = s.cargar_sesion("ses-11").expect("carga");
        assert_eq!(leida.estado, EstadoSesion::Cancelada);
        assert!(!leida.estado.admite_prompt());
        assert_eq!(leida.turnos, 7);
        // Y un estado desconocido cae en "activa" (tolerante, como el tablón).
        assert_eq!(
            EstadoSesion::desde_nombre("inventado"),
            EstadoSesion::Activa
        );
    }
}
