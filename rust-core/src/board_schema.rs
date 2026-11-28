//! # Patrón 10 — El esquema SQLite de sesiones (10 tablas), medido literal
//!
//! Implementa el **patrón 10** de `docs/PATRONES_CLI_RUST_DEFINICION.md:331-347`:
//! el esquema del almacén de sesiones de la CLI de referencia, con una
//! migración **idempotente** y con helpers de **introspección** que permiten
//! comprobar, tabla por tabla, que lo creado es lo medido.
//!
//! ## Fuente MEDIDA
//!
//! Todo el DDL de este módulo sale de `docs/RE_DEVIN_CLI_RUST.md:217-286`
//! (§4.1, "El esquema SQLite (medido del binario)"), transcrito **literalmente**
//! en [`DDL_MEDIDO`] (10 tablas + 4 índices).  Los comentarios del bloque
//! original (`-- ← EL BOSQUE`, etc.) se conservan porque son parte de la
//! lectura de la referencia.
//!
//! | # | tabla | clave / UNIQUE medido | índices medidos |
//! |---|---|---|---|
//! | 1 | `sessions` | `id` PK, `current_epoch` | `idx_sessions_activity`, `idx_sessions_hidden` |
//! | 2 | `messages` | `UNIQUE(session_id, compaction_epoch, sequence_number)` | `idx_messages_session_epoch` |
//! | 3 | `message_nodes` | `UNIQUE(session_id, node_id)` (**el bosque**) | `idx_message_nodes_session` |
//! | 4 | `subagent_heads` | `PRIMARY KEY (session_id, agent_id)` | — |
//! | 5 | `tool_call_state` | `PRIMARY KEY (session_id, tool_call_id)` | — |
//! | 6 | `prompt_history` | `id` PK autoincremental | — |
//! | 7 | `rendered_commits` | `UNIQUE(session_id, sequence_number)` | — |
//! | 8 | `live_row_times` | sin clave | — |
//! | 9 | `session_window_tools` | `session_id` PK | — |
//! | 10 | `app_state` | `key` PK | — |
//!
//! ## Lo que este módulo NO hace (y por qué)
//!
//! * **NO** migra datos de los tablones legacy (`board.sqlite`, `mesh_board.json`,
//!   `task_queue.json`): eso es de `board.rs` y su migración va en otro sitio.
//!   Este módulo sólo crea el **esquema medido** y lo **comprueba**.
//! * **NO** decide el backend de sesiones (SQLite o ficheros): la referencia
//!   tiene los dos y una importación entre ellos en curso
//!   (`docs/RE_DEVIN_CLI_RUST.md:299-312`); aquí sólo vive el SQLite.
//!
//! ## Decisiones sobre lo NO MEDIDO (y por qué)
//!
//! 1. **`sessions.hidden` — NO MEDIDO (columna), MEDIDO (el índice que la exige).**
//!    El `CREATE TABLE sessions` medido **no** declara `hidden`, pero el índice
//!    medido `idx_sessions_hidden ON sessions(hidden)` **sí** lo usa: SQLite
//!    responde `no such column: hidden` (medido aquí con SQLite 3.40).  Las dos
//!    cosas no pueden ser verdad a la vez ⇒ el bloque medido del `CREATE TABLE`
//!    está incompleto (lo normal es que la columna viniera de un
//!    `ALTER TABLE ... ADD COLUMN` de una migración posterior que no se
//!    capturó).  **Decisión conservadora**: se declara
//!    `hidden INTEGER NOT NULL DEFAULT 0` como **extensión propia** (marcada
//!    [`OrigenColumna::Extension`]) para que el índice medido exista tal y como
//!    está medido.  La alternativa (no crear ese índice) rompería el conjunto
//!    de índices medidos.
//! 2. **`IF NOT EXISTS` en cada `CREATE`**: es lo único que se añade al DDL
//!    medido para poder repetir la migración.  No cambia el esquema resultante.
//! 3. **La versión del esquema** se guarda en `app_state` (tabla **medida**,
//!    `key`/`value`), con la clave `board_schema.version`.  El nombre de la
//!    clave es **nuestro**, no de la referencia: NO MEDIDO.
//! 4. **El formato del contenido de `chat_message`** (lo usa
//!    `message_tree.rs`, patrón 3) es **nuestro** (JSON): NO MEDIDO.  La
//!    referencia sólo medimos que la columna es `TEXT NOT NULL`.
//!
//! Lo que **NO** está medido y por tanto **NO** se inventa: los tipos exactos
//! de los parámetros de las tablas que no tienen clave, el contenido de
//! `live_row_times`, el formato de `tool_calls_json` y el comportamiento del
//! producto cuando una migración encuentra un esquema *distinto* del medido
//! (aquí se reporta como desviación, no se corrige a ciegas).

use rusqlite::{params, Connection};

/// Versión del esquema de sesiones que aplica este módulo.
pub const VERSION_ESQUEMA: i64 = 1;

/// El fichero SQLite de la instancia.
///
/// MEDIDO (2026-11-28, patrón 10): la instancia tiene **una** base SQLite:
/// el tablón (`board.rs`) y las sesiones (`acp_sesion_store.rs`) comparten
/// fichero, así que el esquema medido es el esquema real y no un módulo
/// muerto.  El nombre sale de aquí (una sola fuente de verdad).
pub const DB_FILE: &str = "board.sqlite";

/// Clave de `app_state` (tabla **medida**) donde se guarda la versión aplicada.
pub const CLAVE_VERSION: &str = "board_schema.version";

/// Nombres de las 10 tablas **medidas** (en el orden del DDL medido).
pub const TABLAS_MEDIDAS: [&str; 10] = [
    "sessions",
    "messages",
    "message_nodes",
    "subagent_heads",
    "tool_call_state",
    "prompt_history",
    "rendered_commits",
    "live_row_times",
    "session_window_tools",
    "app_state",
];

/// Nombres de los 4 índices **medidos**.
pub const INDICES_MEDIDOS: [&str; 4] = [
    "idx_message_nodes_session",
    "idx_messages_session_epoch",
    "idx_sessions_activity",
    "idx_sessions_hidden",
];

/// El DDL **medido literal** (transcripción de `docs/RE_DEVIN_CLI_RUST.md:217-286`).
///
/// Se conserva **sin tocar** (ni `IF NOT EXISTS`, ni la extensión de
/// `sessions.hidden`) para que la comparación con el esquema aplicado sea
/// transparente: [`ESQUEMA_ESPERADO`] dice qué se aplica y por qué.
pub const DDL_MEDIDO: &str = r#"
CREATE TABLE sessions (
  id TEXT PRIMARY KEY,
  working_directory TEXT NOT NULL,
  backend_type TEXT NOT NULL,
  model TEXT NOT NULL,
  permission_mode TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  last_activity_at INTEGER NOT NULL,
  total_tokens INTEGER NOT NULL,
  current_epoch INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE messages (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  sequence_number INTEGER NOT NULL,
  role TEXT NOT NULL,
  content TEXT NOT NULL,
  tool_call_id TEXT,
  tool_calls_json TEXT,
  timestamp INTEGER NOT NULL,
  compaction_epoch INTEGER NOT NULL DEFAULT 0,
  UNIQUE(session_id, compaction_epoch, sequence_number)
);

CREATE TABLE message_nodes (          -- ← EL BOSQUE
  row_id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  node_id INTEGER NOT NULL,           -- node_id dentro del bosque de la sesión
  parent_node_id INTEGER,             -- NULL para la raíz
  chat_message TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE(session_id, node_id)
);

CREATE TABLE subagent_heads (
  session_id TEXT NOT NULL, agent_id TEXT NOT NULL,
  chain_node_id INTEGER NOT NULL, updated_at INTEGER NOT NULL,
  PRIMARY KEY (session_id, agent_id)
);

CREATE TABLE tool_call_state (
  session_id TEXT NOT NULL, tool_call_id TEXT NOT NULL,
  tool_call_json TEXT,               -- ToolCall inicial
  tool_call_update_json TEXT,        -- actualización final
  PRIMARY KEY (session_id, tool_call_id)
);

CREATE TABLE prompt_history (
  id INTEGER PRIMARY KEY AUTOINCREMENT, content TEXT NOT NULL,
  timestamp INTEGER NOT NULL, session_id TEXT NOT NULL
);

CREATE TABLE rendered_commits (      -- HTML renderizado, versionado
  id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL,
  sequence_number INTEGER NOT NULL, rendered_html TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE(session_id, sequence_number)
);

CREATE TABLE live_row_times (session_id TEXT, row_key TEXT, started_at TEXT);
CREATE TABLE session_window_tools (session_id TEXT PRIMARY KEY, window_tools_json TEXT);
CREATE TABLE app_state (key TEXT PRIMARY KEY, value TEXT);

CREATE INDEX idx_message_nodes_session  ON message_nodes(session_id);
CREATE INDEX idx_messages_session_epoch ON messages(session_id, compaction_epoch);
CREATE INDEX idx_sessions_activity      ON sessions(last_activity_at);
CREATE INDEX idx_sessions_hidden        ON sessions(hidden);
"#;

// ── El DDL que se APLICA (medido + `IF NOT EXISTS` + la extensión marcada) ──

const DDL_SESSIONS: &str = r#"
CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY,
  working_directory TEXT NOT NULL,
  backend_type TEXT NOT NULL,
  model TEXT NOT NULL,
  permission_mode TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  last_activity_at INTEGER NOT NULL,
  total_tokens INTEGER NOT NULL,
  current_epoch INTEGER NOT NULL DEFAULT 0,
  hidden INTEGER NOT NULL DEFAULT 0
);
"#;

const DDL_MESSAGES: &str = r#"
CREATE TABLE IF NOT EXISTS messages (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  sequence_number INTEGER NOT NULL,
  role TEXT NOT NULL,
  content TEXT NOT NULL,
  tool_call_id TEXT,
  tool_calls_json TEXT,
  timestamp INTEGER NOT NULL,
  compaction_epoch INTEGER NOT NULL DEFAULT 0,
  UNIQUE(session_id, compaction_epoch, sequence_number)
);
"#;

const DDL_MESSAGE_NODES: &str = r#"
CREATE TABLE IF NOT EXISTS message_nodes (          -- ← EL BOSQUE
  row_id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  node_id INTEGER NOT NULL,           -- node_id dentro del bosque de la sesión
  parent_node_id INTEGER,             -- NULL para la raíz
  chat_message TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE(session_id, node_id)
);
"#;

const DDL_SUBAGENT_HEADS: &str = r#"
CREATE TABLE IF NOT EXISTS subagent_heads (
  session_id TEXT NOT NULL, agent_id TEXT NOT NULL,
  chain_node_id INTEGER NOT NULL, updated_at INTEGER NOT NULL,
  PRIMARY KEY (session_id, agent_id)
);
"#;

const DDL_TOOL_CALL_STATE: &str = r#"
CREATE TABLE IF NOT EXISTS tool_call_state (
  session_id TEXT NOT NULL, tool_call_id TEXT NOT NULL,
  tool_call_json TEXT,               -- ToolCall inicial
  tool_call_update_json TEXT,        -- actualización final
  PRIMARY KEY (session_id, tool_call_id)
);
"#;

const DDL_PROMPT_HISTORY: &str = r#"
CREATE TABLE IF NOT EXISTS prompt_history (
  id INTEGER PRIMARY KEY AUTOINCREMENT, content TEXT NOT NULL,
  timestamp INTEGER NOT NULL, session_id TEXT NOT NULL
);
"#;

const DDL_RENDERED_COMMITS: &str = r#"
CREATE TABLE IF NOT EXISTS rendered_commits (      -- HTML renderizado, versionado
  id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL,
  sequence_number INTEGER NOT NULL, rendered_html TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE(session_id, sequence_number)
);
"#;

const DDL_LIVE_ROW_TIMES: &str = r#"
CREATE TABLE IF NOT EXISTS live_row_times (session_id TEXT, row_key TEXT, started_at TEXT);
"#;

const DDL_SESSION_WINDOW_TOOLS: &str = r#"
CREATE TABLE IF NOT EXISTS session_window_tools (session_id TEXT PRIMARY KEY, window_tools_json TEXT);
"#;

const DDL_APP_STATE: &str = r#"
CREATE TABLE IF NOT EXISTS app_state (key TEXT PRIMARY KEY, value TEXT);
"#;

const DDL_IDX_MESSAGE_NODES_SESSION: &str =
    "CREATE INDEX IF NOT EXISTS idx_message_nodes_session ON message_nodes(session_id);";
const DDL_IDX_MESSAGES_SESSION_EPOCH: &str =
    "CREATE INDEX IF NOT EXISTS idx_messages_session_epoch ON messages(session_id, compaction_epoch);";
const DDL_IDX_SESSIONS_ACTIVITY: &str =
    "CREATE INDEX IF NOT EXISTS idx_sessions_activity ON sessions(last_activity_at);";
const DDL_IDX_SESSIONS_HIDDEN: &str =
    "CREATE INDEX IF NOT EXISTS idx_sessions_hidden ON sessions(hidden);";

// ── Lo que se espera encontrar (para comparar) ─────────────────────────────

/// De dónde viene una columna del esquema esperado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrigenColumna {
    /// Columna transcrita **literalmente** del DDL medido.
    Medido,
    /// Columna **nuestra** (no está en el DDL medido).  Existe por un motivo
    /// escrito en el doc-comment del módulo.
    Extension,
}

/// Una columna del esquema esperado (tal y como la ve `PRAGMA table_info`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColumnaEsperada {
    pub nombre: &'static str,
    /// Tipo declarado (`TEXT`, `INTEGER`).
    pub tipo: &'static str,
    /// `NOT NULL` explícito en el DDL.
    pub not_null: bool,
    /// Literal del `DEFAULT` (tal y como lo devuelve `PRAGMA table_info`).
    pub default: Option<&'static str>,
    /// Posición dentro de la clave primaria (1..n); `0` si no forma parte.
    pub pk: i64,
    pub origen: OrigenColumna,
}

/// Un índice esperado (explícito, `CREATE INDEX`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndiceEsperado {
    pub nombre: &'static str,
    /// Columnas, en orden.
    pub columnas: &'static [&'static str],
    pub unico: bool,
}

/// Una tabla del esquema esperado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TablaEsperada {
    pub nombre: &'static str,
    /// El `CREATE TABLE` que se aplica (medido + `IF NOT EXISTS`).
    pub ddl: &'static str,
    pub columnas: &'static [ColumnaEsperada],
    /// Índices explícitos que viven en esta tabla.
    pub indices: &'static [IndiceEsperado],
    /// Restricciones `UNIQUE` declaradas **dentro** del `CREATE TABLE`
    /// (SQLite las materializa como índices automáticos de origen `u`).
    pub unicos: &'static [&'static [&'static str]],
    /// Columnas de la clave primaria, en orden.
    pub pk: &'static [&'static str],
}

/// El esquema medido completo, tabla por tabla.
///
/// Es la **fuente de verdad** de la comprobación: [`verificar_esquema`] compara
/// lo que hay en la base con esta lista, así que un test puede validar el
/// esquema creado **sin** la referencia en runtime.
pub const ESQUEMA_ESPERADO: &[TablaEsperada] = &[
    TablaEsperada {
        nombre: "sessions",
        ddl: DDL_SESSIONS,
        columnas: &[
            ColumnaEsperada {
                nombre: "id",
                tipo: "TEXT",
                not_null: false,
                default: None,
                pk: 1,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "working_directory",
                tipo: "TEXT",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "backend_type",
                tipo: "TEXT",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "model",
                tipo: "TEXT",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "permission_mode",
                tipo: "TEXT",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "created_at",
                tipo: "INTEGER",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "last_activity_at",
                tipo: "INTEGER",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "total_tokens",
                tipo: "INTEGER",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "current_epoch",
                tipo: "INTEGER",
                not_null: true,
                default: Some("0"),
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "hidden",
                tipo: "INTEGER",
                not_null: true,
                default: Some("0"),
                pk: 0,
                // NO MEDIDO: el `CREATE TABLE` medido no la declara, pero el
                // índice medido `idx_sessions_hidden` la exige.
                origen: OrigenColumna::Extension,
            },
        ],
        indices: &[
            IndiceEsperado {
                nombre: "idx_sessions_activity",
                columnas: &["last_activity_at"],
                unico: false,
            },
            IndiceEsperado {
                nombre: "idx_sessions_hidden",
                columnas: &["hidden"],
                unico: false,
            },
        ],
        unicos: &[],
        pk: &["id"],
    },
    TablaEsperada {
        nombre: "messages",
        ddl: DDL_MESSAGES,
        columnas: &[
            ColumnaEsperada {
                nombre: "id",
                tipo: "INTEGER",
                not_null: false,
                default: None,
                pk: 1,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "session_id",
                tipo: "TEXT",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "sequence_number",
                tipo: "INTEGER",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "role",
                tipo: "TEXT",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "content",
                tipo: "TEXT",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "tool_call_id",
                tipo: "TEXT",
                not_null: false,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "tool_calls_json",
                tipo: "TEXT",
                not_null: false,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "timestamp",
                tipo: "INTEGER",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "compaction_epoch",
                tipo: "INTEGER",
                not_null: true,
                default: Some("0"),
                pk: 0,
                origen: OrigenColumna::Medido,
            },
        ],
        indices: &[IndiceEsperado {
            nombre: "idx_messages_session_epoch",
            columnas: &["session_id", "compaction_epoch"],
            unico: false,
        }],
        unicos: &[&["session_id", "compaction_epoch", "sequence_number"]],
        pk: &["id"],
    },
    TablaEsperada {
        nombre: "message_nodes",
        ddl: DDL_MESSAGE_NODES,
        columnas: &[
            ColumnaEsperada {
                nombre: "row_id",
                tipo: "INTEGER",
                not_null: false,
                default: None,
                pk: 1,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "session_id",
                tipo: "TEXT",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "node_id",
                tipo: "INTEGER",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "parent_node_id",
                tipo: "INTEGER",
                not_null: false,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "chat_message",
                tipo: "TEXT",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "created_at",
                tipo: "INTEGER",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
        ],
        indices: &[IndiceEsperado {
            nombre: "idx_message_nodes_session",
            columnas: &["session_id"],
            unico: false,
        }],
        unicos: &[&["session_id", "node_id"]],
        pk: &["row_id"],
    },
    TablaEsperada {
        nombre: "subagent_heads",
        ddl: DDL_SUBAGENT_HEADS,
        columnas: &[
            ColumnaEsperada {
                nombre: "session_id",
                tipo: "TEXT",
                not_null: true,
                default: None,
                pk: 1,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "agent_id",
                tipo: "TEXT",
                not_null: true,
                default: None,
                pk: 2,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "chain_node_id",
                tipo: "INTEGER",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "updated_at",
                tipo: "INTEGER",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
        ],
        indices: &[],
        unicos: &[],
        pk: &["session_id", "agent_id"],
    },
    TablaEsperada {
        nombre: "tool_call_state",
        ddl: DDL_TOOL_CALL_STATE,
        columnas: &[
            ColumnaEsperada {
                nombre: "session_id",
                tipo: "TEXT",
                not_null: true,
                default: None,
                pk: 1,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "tool_call_id",
                tipo: "TEXT",
                not_null: true,
                default: None,
                pk: 2,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "tool_call_json",
                tipo: "TEXT",
                not_null: false,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "tool_call_update_json",
                tipo: "TEXT",
                not_null: false,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
        ],
        indices: &[],
        unicos: &[],
        pk: &["session_id", "tool_call_id"],
    },
    TablaEsperada {
        nombre: "prompt_history",
        ddl: DDL_PROMPT_HISTORY,
        columnas: &[
            ColumnaEsperada {
                nombre: "id",
                tipo: "INTEGER",
                not_null: false,
                default: None,
                pk: 1,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "content",
                tipo: "TEXT",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "timestamp",
                tipo: "INTEGER",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "session_id",
                tipo: "TEXT",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
        ],
        indices: &[],
        unicos: &[],
        pk: &["id"],
    },
    TablaEsperada {
        nombre: "rendered_commits",
        ddl: DDL_RENDERED_COMMITS,
        columnas: &[
            ColumnaEsperada {
                nombre: "id",
                tipo: "INTEGER",
                not_null: false,
                default: None,
                pk: 1,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "session_id",
                tipo: "TEXT",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "sequence_number",
                tipo: "INTEGER",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "rendered_html",
                tipo: "TEXT",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "created_at",
                tipo: "INTEGER",
                not_null: true,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
        ],
        indices: &[],
        unicos: &[&["session_id", "sequence_number"]],
        pk: &["id"],
    },
    TablaEsperada {
        nombre: "live_row_times",
        ddl: DDL_LIVE_ROW_TIMES,
        columnas: &[
            ColumnaEsperada {
                nombre: "session_id",
                tipo: "TEXT",
                not_null: false,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "row_key",
                tipo: "TEXT",
                not_null: false,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "started_at",
                tipo: "TEXT",
                not_null: false,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
        ],
        indices: &[],
        unicos: &[],
        pk: &[],
    },
    TablaEsperada {
        nombre: "session_window_tools",
        ddl: DDL_SESSION_WINDOW_TOOLS,
        columnas: &[
            ColumnaEsperada {
                nombre: "session_id",
                tipo: "TEXT",
                not_null: false,
                default: None,
                pk: 1,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "window_tools_json",
                tipo: "TEXT",
                not_null: false,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
        ],
        indices: &[],
        unicos: &[],
        pk: &["session_id"],
    },
    TablaEsperada {
        nombre: "app_state",
        ddl: DDL_APP_STATE,
        columnas: &[
            ColumnaEsperada {
                nombre: "key",
                tipo: "TEXT",
                not_null: false,
                default: None,
                pk: 1,
                origen: OrigenColumna::Medido,
            },
            ColumnaEsperada {
                nombre: "value",
                tipo: "TEXT",
                not_null: false,
                default: None,
                pk: 0,
                origen: OrigenColumna::Medido,
            },
        ],
        indices: &[],
        unicos: &[],
        pk: &["key"],
    },
];

/// Un índice **explícito** que se crea con la migración.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndiceAplicado {
    pub nombre: &'static str,
    pub tabla: &'static str,
    pub ddl: &'static str,
}

/// Los 4 índices medidos, con su `CREATE INDEX IF NOT EXISTS`.
pub const INDICES_APLICADOS: &[IndiceAplicado] = &[
    IndiceAplicado {
        nombre: "idx_message_nodes_session",
        tabla: "message_nodes",
        ddl: DDL_IDX_MESSAGE_NODES_SESSION,
    },
    IndiceAplicado {
        nombre: "idx_messages_session_epoch",
        tabla: "messages",
        ddl: DDL_IDX_MESSAGES_SESSION_EPOCH,
    },
    IndiceAplicado {
        nombre: "idx_sessions_activity",
        tabla: "sessions",
        ddl: DDL_IDX_SESSIONS_ACTIVITY,
    },
    IndiceAplicado {
        nombre: "idx_sessions_hidden",
        tabla: "sessions",
        ddl: DDL_IDX_SESSIONS_HIDDEN,
    },
];

// ── Introspección ─────────────────────────────────────────────────────────

/// Una columna, tal y como la devuelve `PRAGMA table_info`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Columna {
    pub nombre: String,
    pub tipo: String,
    pub not_null: bool,
    pub default: Option<String>,
    /// Posición dentro de la clave primaria (0 = no forma parte).
    pub pk: i64,
}

/// Un índice, tal y como lo devuelven `PRAGMA index_list` + `PRAGMA index_info`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Indice {
    pub nombre: String,
    pub tabla: String,
    pub unico: bool,
    /// `c` = `CREATE INDEX`, `u` = `UNIQUE` del `CREATE TABLE`, `pk` = clave primaria.
    pub origen: String,
    /// Columnas del índice, en orden.
    pub columnas: Vec<String>,
}

/// Todas las tablas de la base (incluye las internas de SQLite, p. ej.
/// `sqlite_sequence`, que aparece por el `AUTOINCREMENT` medido).
pub fn listar_tablas(conn: &Connection) -> Result<Vec<String>, rusqlite::Error> {
    let mut stmt =
        conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
    let filas = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut out = Vec::new();
    for f in filas {
        out.push(f?);
    }
    Ok(out)
}

/// Las columnas de una tabla, en el orden del `CREATE TABLE`.
///
/// Si la tabla no existe, la lista sale vacía (no es un error).
pub fn listar_columnas(conn: &Connection, tabla: &str) -> Result<Vec<Columna>, rusqlite::Error> {
    let sql = format!("PRAGMA table_info({})", quoted(tabla)?);
    let mut stmt = conn.prepare(&sql)?;
    let filas = stmt.query_map([], |row| {
        Ok(Columna {
            nombre: row.get(1)?,
            tipo: row.get(2)?,
            not_null: row.get::<_, i64>(3)? != 0,
            default: row.get::<_, Option<String>>(4)?,
            pk: row.get::<_, i64>(5)?,
        })
    })?;
    let mut out = Vec::new();
    for f in filas {
        out.push(f?);
    }
    Ok(out)
}

/// Los índices de una tabla (los explícitos, los `UNIQUE` del `CREATE TABLE`
/// y el de la clave primaria).
pub fn listar_indices(conn: &Connection, tabla: &str) -> Result<Vec<Indice>, rusqlite::Error> {
    let sql = format!("PRAGMA index_list({})", quoted(tabla)?);
    let mut stmt = conn.prepare(&sql)?;
    let mut crudos: Vec<(String, bool, String)> = Vec::new();
    {
        let filas = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? != 0,
                row.get::<_, String>(3)?,
            ))
        })?;
        for f in filas {
            crudos.push(f?);
        }
    }
    let mut out = Vec::new();
    for (nombre, unico, origen) in crudos {
        let columnas = columnas_de_indice(conn, &nombre)?;
        out.push(Indice {
            nombre,
            tabla: tabla.to_string(),
            unico,
            origen,
            columnas,
        });
    }
    Ok(out)
}

/// ¿Existe la tabla?
pub fn tabla_existe(conn: &Connection, tabla: &str) -> Result<bool, rusqlite::Error> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type IN ('table','view') AND name=?1",
        params![tabla],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// ¿Existe el índice (por nombre, sea explícito o automático)?
pub fn indice_existe(conn: &Connection, indice: &str) -> Result<bool, rusqlite::Error> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1",
        params![indice],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// Las columnas de un índice, en orden.
fn columnas_de_indice(conn: &Connection, indice: &str) -> Result<Vec<String>, rusqlite::Error> {
    let sql = format!("PRAGMA index_info({})", quoted(indice)?);
    let mut stmt = conn.prepare(&sql)?;
    let filas = stmt.query_map([], |row| row.get::<_, Option<String>>(2))?;
    let mut out = Vec::new();
    for f in filas {
        if let Some(c) = f? {
            out.push(c);
        }
    }
    Ok(out)
}

/// Un identificador SQLite entrecomillado, **validado**: sólo se admiten
/// letras, dígitos y `_` (nada de comillas ni de `;`).
fn quoted(nombre: &str) -> Result<String, rusqlite::Error> {
    if nombre.is_empty()
        || !nombre
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(rusqlite::Error::InvalidParameterName(nombre.to_string()));
    }
    Ok(format!("\"{}\"", nombre))
}

// ── La migración ──────────────────────────────────────────────────────────

/// Lo que hizo la migración.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResumenMigracion {
    /// Tablas que **no** existían y se han creado.
    pub tablas_creadas: Vec<String>,
    /// Tablas que ya existían (no se han tocado).
    pub tablas_existentes: Vec<String>,
    /// Índices que no existían y se han creado.
    pub indices_creados: Vec<String>,
    /// Índices que ya existían.
    pub indices_existentes: Vec<String>,
    /// Columnas añadidas con `ALTER TABLE` (sólo las que se pueden añadir sin
    /// inventar un valor: las que tienen `DEFAULT`).
    pub columnas_creadas: Vec<String>,
    /// Lo que **no** se ha podido hacer (con el motivo).
    pub desviaciones: Vec<String>,
    /// Versión del esquema aplicada.
    pub version_esquema: i64,
}

impl ResumenMigracion {
    fn nuevo() -> Self {
        ResumenMigracion {
            version_esquema: VERSION_ESQUEMA,
            ..Default::default()
        }
    }

    /// Cuántas tablas se han creado en esta ejecución.
    pub fn nuevas_tablas(&self) -> usize {
        self.tablas_creadas.len()
    }

    /// Cuántas tablas ya existían.
    pub fn tablas_previas(&self) -> usize {
        self.tablas_existentes.len()
    }

    /// Cuántos índices se han creado en esta ejecución.
    pub fn nuevos_indices(&self) -> usize {
        self.indices_creados.len()
    }

    /// Cuántos índices ya existían.
    pub fn indices_previos(&self) -> usize {
        self.indices_existentes.len()
    }

    /// ¿Ha quedado todo en su sitio (sin desviaciones)?
    pub fn limpio(&self) -> bool {
        self.desviaciones.is_empty()
    }
}

/// Aplica el esquema medido **de forma idempotente**.
///
/// Sirve **tanto para un estado nuevo como para uno existente**: se puede
/// llamar las veces que haga falta; la segunda vez no falla, no duplica nada
/// y no toca lo que ya estaba.
///
/// * Las tablas se crean con `CREATE TABLE IF NOT EXISTS`.
/// * Las columnas que falten se añaden con `ALTER TABLE ... ADD COLUMN`
///   **sólo** si su DDL medido trae `DEFAULT` (añadir una `NOT NULL` sin
///   `DEFAULT` obligaría a inventar un valor: eso se reporta como desviación,
///   no se hace).
/// * Los índices se crean con `CREATE INDEX IF NOT EXISTS`; si un índice no se
///   puede crear (p. ej. porque su tabla tiene otra forma), se reporta como
///   desviación y la migración **no** se cae.
/// * La versión aplicada se guarda en `app_state` (tabla medida).
///
/// La migración va en una transacción **IMMEDIATE** cuando se puede; si la
/// conexión ya tiene una transacción abierta, se sigue **sin** ella
/// (idempotencia por sentencia, no por atomicidad).
///
/// MEDIDO (2026-11-28): IMMEDIATE y no DEFERRED porque la migración
/// **escribe** (`CREATE TABLE`): con DEFERRED, una transacción que empieza
/// leyendo y luego sube a escritura se encuentra con "database is locked"
/// **sin esperar** (el busy handler no ayuda en esa subida).  Con IMMEDIATE
/// el bloqueo se pide al principio y el `busy_timeout` sí espera.
pub fn aplicar(conn: &Connection) -> Result<ResumenMigracion, rusqlite::Error> {
    let mut resumen = ResumenMigracion::nuevo();

    // MEDIDO (2026-11-28): `BEGIN IMMEDIATE` **a mano** (rusqlite 0.32 no
    // tiene `unchecked_transaction_with_behavior`): la migración escribe, así
    // que el bloqueo se pide AL PRINCIPIO.  Con DEFERRED, una lectura
    // seguida de una escritura choca con "database is locked" **sin
    // esperar** (el busy handler no ayuda en esa subida) ⇒ con dos
    // escritores concurrentes el segundo fallaba.  Si la conexión ya tiene
    // una transacción abierta, el `BEGIN` falla y se sigue **dentro** de
    // ella (idempotencia por sentencia, no por atomicidad).
    let propia = conn.execute_batch("BEGIN IMMEDIATE").is_ok();
    let resultado = aplicar_dentro(conn, &mut resumen);
    if propia {
        let _ = match resultado {
            Ok(_) => conn.execute_batch("COMMIT"),
            Err(_) => conn.execute_batch("ROLLBACK"),
        };
    }
    resultado.map(|_| resumen)
}

/// El cuerpo de la migración (con la transacción ya abierta o no).
fn aplicar_dentro(
    c: &Connection,
    resumen: &mut ResumenMigracion,
) -> Result<(), rusqlite::Error> {
    for tabla in ESQUEMA_ESPERADO {
        if tabla_existe(c, tabla.nombre)? {
            resumen.tablas_existentes.push(tabla.nombre.to_string());
            let actuales = listar_columnas(c, tabla.nombre)?;
            for col in tabla.columnas {
                if actuales.iter().any(|x| x.nombre == col.nombre) {
                    continue;
                }
                match anadir_columna(c, tabla.nombre, *col) {
                    Ok(()) => resumen
                        .columnas_creadas
                        .push(format!("{}.{}", tabla.nombre, col.nombre)),
                    Err(e) => resumen.desviaciones.push(format!(
                        "no se pudo añadir la columna {}.{}: {}",
                        tabla.nombre, col.nombre, e
                    )),
                }
            }
        } else {
            c.execute_batch(tabla.ddl)?;
            resumen.tablas_creadas.push(tabla.nombre.to_string());
        }
    }

    for indice in INDICES_APLICADOS {
        if indice_existe(c, indice.nombre)? {
            resumen.indices_existentes.push(indice.nombre.to_string());
        } else {
            match c.execute_batch(indice.ddl) {
                Ok(()) => resumen.indices_creados.push(indice.nombre.to_string()),
                Err(e) => resumen.desviaciones.push(format!(
                    "no se pudo crear el índice {}: {}",
                    indice.nombre, e
                )),
            }
        }
    }

    // La versión, en `app_state` (tabla MEDIDA).
    c.execute(
        "INSERT INTO app_state(key,value) VALUES(?1,?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![CLAVE_VERSION, VERSION_ESQUEMA.to_string()],
    )?;

    Ok(())
}

/// Añade una columna que falta, **sólo** si se puede sin inventar un valor.
fn anadir_columna(
    conn: &Connection,
    tabla: &str,
    col: ColumnaEsperada,
) -> Result<(), rusqlite::Error> {
    let Some(def) = col.default else {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "{}.{} no tiene DEFAULT: añadirla exigiría inventar un valor",
            tabla, col.nombre
        )));
    };
    let sql = format!(
        "ALTER TABLE {} ADD COLUMN {} {} NOT NULL DEFAULT {}",
        quoted(tabla)?,
        quoted(col.nombre)?,
        col.tipo,
        def
    );
    conn.execute_batch(&sql)
}

// ── La comprobación ───────────────────────────────────────────────────────

/// Compara el esquema de la base con el **medido**, tabla por tabla.
///
/// `Ok(())` ⇒ lo que hay en la base es exactamente lo esperado.
/// `Err(desviaciones)` ⇒ la lista de diferencias (tabla que falta, columna que
/// no está, tipo que no cuadra, índice que falta, `UNIQUE` que no existe...).
///
/// Es la comprobación que permite un test que valide el esquema creado contra
/// el DDL transcrito **sin** la referencia en runtime.
pub fn verificar_esquema(conn: &Connection) -> Result<(), Vec<String>> {
    match desviaciones(conn) {
        Ok(d) if d.is_empty() => Ok(()),
        Ok(d) => Err(d),
        Err(e) => Err(vec![format!("no se pudo inspeccionar el esquema: {}", e)]),
    }
}

/// Las desviaciones respecto de lo esperado (mismo contenido que
/// [`verificar_esquema`], pero sin envolver en `Result`).
pub fn desviaciones(conn: &Connection) -> Result<Vec<String>, rusqlite::Error> {
    let mut d: Vec<String> = Vec::new();

    for tabla in ESQUEMA_ESPERADO {
        let columnas = match listar_columnas(conn, tabla.nombre) {
            Ok(c) => c,
            Err(e) => {
                d.push(format!("no se pudo leer la tabla {}: {}", tabla.nombre, e));
                continue;
            }
        };
        if columnas.is_empty() {
            d.push(format!("falta la tabla {}", tabla.nombre));
            continue;
        }

        for esperada in tabla.columnas {
            match columnas.iter().find(|c| c.nombre == esperada.nombre) {
                None => d.push(format!(
                    "{}: falta la columna {}",
                    tabla.nombre, esperada.nombre
                )),
                Some(real) => {
                    if !real.tipo.eq_ignore_ascii_case(esperada.tipo) {
                        d.push(format!(
                            "{}.{}: tipo {} (esperado {})",
                            tabla.nombre, esperada.nombre, real.tipo, esperada.tipo
                        ));
                    }
                    if real.not_null != esperada.not_null {
                        d.push(format!(
                            "{}.{}: NOT NULL {} (esperado {})",
                            tabla.nombre, esperada.nombre, real.not_null, esperada.not_null
                        ));
                    }
                    if real.pk != esperada.pk {
                        d.push(format!(
                            "{}.{}: posición en la clave primaria {} (esperada {})",
                            tabla.nombre, esperada.nombre, real.pk, esperada.pk
                        ));
                    }
                    if !mismo_default(real.default.as_deref(), esperada.default) {
                        d.push(format!(
                            "{}.{}: default {:?} (esperado {:?})",
                            tabla.nombre, esperada.nombre, real.default, esperada.default
                        ));
                    }
                }
            }
        }

        let indices = listar_indices(conn, tabla.nombre)?;
        for esperado in tabla.indices {
            match indices.iter().find(|i| i.nombre == esperado.nombre) {
                None => d.push(format!(
                    "{}: falta el índice {}",
                    tabla.nombre, esperado.nombre
                )),
                Some(real) => {
                    let cols: Vec<&str> = real.columnas.iter().map(|s| s.as_str()).collect();
                    if cols != esperado.columnas {
                        d.push(format!(
                            "{}: el índice {} tiene {:?} (esperado {:?})",
                            tabla.nombre, esperado.nombre, cols, esperado.columnas
                        ));
                    }
                    if real.unico != esperado.unico {
                        d.push(format!(
                            "{}: el índice {} es único={} (esperado {})",
                            tabla.nombre, esperado.nombre, real.unico, esperado.unico
                        ));
                    }
                }
            }
        }

        for unico in tabla.unicos {
            let tiene = indices
                .iter()
                .any(|i| i.origen == "u" && columnas_coinciden(&i.columnas, unico));
            if !tiene {
                d.push(format!(
                    "{}: falta el UNIQUE({})",
                    tabla.nombre,
                    unico.join(", ")
                ));
            }
        }
    }

    Ok(d)
}

/// Dos listas de columnas iguales.
fn columnas_coinciden(real: &[String], esperado: &[&str]) -> bool {
    real.len() == esperado.len() && real.iter().zip(esperado.iter()).all(|(a, b)| a == b)
}

/// Dos `DEFAULT` equivalentes (el texto puede venir con espacios).
fn mismo_default(real: Option<&str>, esperado: Option<&str>) -> bool {
    match (real, esperado) {
        (None, None) => true,
        (Some(a), Some(b)) => a.trim() == b.trim(),
        _ => false,
    }
}

// ── TESTS ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn bd() -> Connection {
        let conn = Connection::open_in_memory().expect("bd en memoria");
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        conn
    }

    #[test]
    fn el_ddl_medido_tiene_las_diez_tablas_y_los_cuatro_indices() {
        for t in TABLAS_MEDIDAS.iter() {
            assert!(
                DDL_MEDIDO.contains(&format!("CREATE TABLE {}", t)),
                "falta la tabla medida {}",
                t
            );
        }
        for i in INDICES_MEDIDOS.iter() {
            assert!(
                DDL_MEDIDO.contains(&format!("CREATE INDEX {}", i)),
                "falta el índice medido {}",
                i
            );
        }
        assert_eq!(TABLAS_MEDIDAS.len(), 10);
        assert_eq!(INDICES_MEDIDOS.len(), 4);
    }

    #[test]
    fn aplicar_crea_las_diez_tablas_y_los_cuatro_indices() {
        let conn = bd();
        let r = aplicar(&conn).expect("la migración");
        assert_eq!(r.nuevas_tablas(), 10);
        assert_eq!(r.nuevos_indices(), 4);
        assert_eq!(r.tablas_previas(), 0);
        assert_eq!(r.indices_previos(), 0);
        assert!(r.limpio(), "desviaciones: {:?}", r.desviaciones);
        for t in TABLAS_MEDIDAS.iter() {
            assert!(tabla_existe(&conn, t).unwrap(), "falta {}", t);
        }
        for i in INDICES_MEDIDOS.iter() {
            assert!(indice_existe(&conn, i).unwrap(), "falta el índice {}", i);
        }
    }

    #[test]
    fn aplicar_dos_veces_no_falla_ni_duplica() {
        let conn = bd();
        let primera = aplicar(&conn).expect("primera migración");
        assert_eq!(primera.nuevas_tablas(), 10);

        let segunda = aplicar(&conn).expect("segunda migración");
        assert_eq!(segunda.nuevas_tablas(), 0, "la segunda no crea nada");
        assert_eq!(segunda.nuevos_indices(), 0);
        assert_eq!(segunda.tablas_previas(), 10);
        assert_eq!(segunda.indices_previos(), 4);
        assert!(segunda.limpio(), "desviaciones: {:?}", segunda.desviaciones);

        // Ni una tabla de más.
        let tablas = listar_tablas(&conn).unwrap();
        for t in TABLAS_MEDIDAS.iter() {
            assert_eq!(
                tablas.iter().filter(|x| *x == t).count(),
                1,
                "la tabla {} está duplicada",
                t
            );
        }
        // Y el esquema sigue siendo el esperado.
        assert!(verificar_esquema(&conn).is_ok());
    }

    #[test]
    fn el_esquema_verificado_coincide_con_el_esperado_tabla_por_tabla() {
        let conn = bd();
        aplicar(&conn).expect("migración");
        match verificar_esquema(&conn) {
            Ok(()) => {}
            Err(d) => panic!("desviaciones: {:?}", d),
        }
        for tabla in ESQUEMA_ESPERADO.iter() {
            let reales = listar_columnas(&conn, tabla.nombre).unwrap();
            assert_eq!(
                reales.len(),
                tabla.columnas.len(),
                "la tabla {} tiene {} columnas (esperadas {})",
                tabla.nombre,
                reales.len(),
                tabla.columnas.len()
            );
            for esperada in tabla.columnas.iter() {
                let real = reales
                    .iter()
                    .find(|c| c.nombre == esperada.nombre)
                    .unwrap_or_else(|| panic!("falta {}.{}", tabla.nombre, esperada.nombre));
                assert_eq!(real.tipo.to_ascii_uppercase(), esperada.tipo);
                assert_eq!(real.not_null, esperada.not_null);
                assert_eq!(real.pk, esperada.pk);
                assert_eq!(real.default.as_deref(), esperada.default);
            }
        }
    }

    #[test]
    fn los_indices_medidos_existen_con_sus_columnas() {
        let conn = bd();
        aplicar(&conn).expect("migración");
        for indice in INDICES_APLICADOS.iter() {
            let lista = listar_indices(&conn, indice.tabla).unwrap();
            let real = lista
                .iter()
                .find(|i| i.nombre == indice.nombre)
                .unwrap_or_else(|| panic!("falta el índice {}", indice.nombre));
            assert!(
                !real.columnas.is_empty(),
                "el índice {} no tiene columnas",
                indice.nombre
            );
        }
        // Comprobación explícita de los cuatro medidos.
        let por_nombre = |t: &str, n: &str| -> Vec<String> {
            listar_indices(&conn, t)
                .unwrap()
                .into_iter()
                .find(|i| i.nombre == n)
                .map(|i| i.columnas)
                .unwrap_or_default()
        };
        assert_eq!(
            por_nombre("message_nodes", "idx_message_nodes_session"),
            vec!["session_id"]
        );
        assert_eq!(
            por_nombre("messages", "idx_messages_session_epoch"),
            vec!["session_id", "compaction_epoch"]
        );
        assert_eq!(
            por_nombre("sessions", "idx_sessions_activity"),
            vec!["last_activity_at"]
        );
        assert_eq!(
            por_nombre("sessions", "idx_sessions_hidden"),
            vec!["hidden"]
        );
    }

    #[test]
    fn el_unique_de_messages_impide_dos_mensajes_iguales() {
        let conn = bd();
        aplicar(&conn).expect("migración");
        conn.execute(
            "INSERT INTO messages(session_id,sequence_number,role,content,timestamp)
             VALUES('s1',1,'user','hola',100)",
            [],
        )
        .expect("primer mensaje");
        let segundo = conn.execute(
            "INSERT INTO messages(session_id,sequence_number,role,content,timestamp)
             VALUES('s1',1,'user','otro',100)",
            [],
        );
        assert!(segundo.is_err(), "el UNIQUE medido no se está aplicando");
        // Con otra época sí entra (el UNIQUE incluye la época).
        let otra = conn.execute(
            "INSERT INTO messages(session_id,sequence_number,role,content,timestamp,compaction_epoch)
             VALUES('s1',1,'user','otro',100,1)",
            [],
        );
        assert!(otra.is_ok(), "el UNIQUE debe incluir compaction_epoch");
    }

    #[test]
    fn las_epochs_medidas_valen_cero_por_defecto() {
        let conn = bd();
        aplicar(&conn).expect("migración");
        let cols = listar_columnas(&conn, "sessions").unwrap();
        let epoch = cols.iter().find(|c| c.nombre == "current_epoch").unwrap();
        assert_eq!(epoch.default.as_deref(), Some("0"));
        assert_eq!(epoch.not_null, true);

        let cols = listar_columnas(&conn, "messages").unwrap();
        let epoch = cols
            .iter()
            .find(|c| c.nombre == "compaction_epoch")
            .unwrap();
        assert_eq!(epoch.default.as_deref(), Some("0"));
        assert_eq!(epoch.not_null, true);
    }

    #[test]
    fn hidden_es_una_extension_declarada_y_no_esta_en_el_ddl_medido() {
        // La columna NO está en el bloque medido...
        assert!(
            !DDL_MEDIDO.contains("hidden INTEGER"),
            "el DDL medido no declara la columna hidden"
        );
        // ...pero el índice medido sí la usa.
        assert!(DDL_MEDIDO.contains("idx_sessions_hidden"));
        assert!(DDL_MEDIDO.contains("sessions(hidden)"));
        // Y en el esquema esperado está marcada como extensión.
        let sesiones = ESQUEMA_ESPERADO
            .iter()
            .find(|t| t.nombre == "sessions")
            .expect("sessions");
        let hidden = sesiones
            .columnas
            .iter()
            .find(|c| c.nombre == "hidden")
            .expect("la columna hidden");
        assert_eq!(hidden.origen, OrigenColumna::Extension);
        // Es la **única** extensión del esquema.
        let extensiones: Vec<&str> = ESQUEMA_ESPERADO
            .iter()
            .flat_map(|t| t.columnas.iter())
            .filter(|c| c.origen == OrigenColumna::Extension)
            .map(|c| c.nombre)
            .collect();
        assert_eq!(extensiones, vec!["hidden"]);
    }

    #[test]
    fn un_estado_existente_no_se_toca_ni_se_duplica() {
        let conn = bd();
        // Una tabla del esquema medido ya está (creada a mano, igual).
        conn.execute_batch(
            "CREATE TABLE app_state (key TEXT PRIMARY KEY, value TEXT);
             INSERT INTO app_state(key,value) VALUES('mio','1');",
        )
        .expect("estado previo");

        let r = aplicar(&conn).expect("migración sobre estado existente");
        assert!(r.tablas_existentes.contains(&"app_state".to_string()));
        assert!(!r.tablas_creadas.contains(&"app_state".to_string()));
        assert_eq!(r.nuevas_tablas(), 9);
        // El dato previo sigue ahí.
        let v: String = conn
            .query_row("SELECT value FROM app_state WHERE key='mio'", [], |r| {
                r.get(0)
            })
            .expect("el dato previo");
        assert_eq!(v, "1");
        assert!(verificar_esquema(&conn).is_ok());
    }

    #[test]
    fn un_esquema_distinto_se_reporta_como_desviacion() {
        let conn = bd();
        // `sessions` con otra forma (sin `working_directory`).
        conn.execute_batch("CREATE TABLE sessions (id TEXT PRIMARY KEY);")
            .expect("tabla con otra forma");
        let r = aplicar(&conn).expect("la migración no debe caerse");
        // La tabla existe: no se recrea ni se duplica.
        assert!(r.tablas_existentes.contains(&"sessions".to_string()));
        // Las columnas que faltan y no tienen DEFAULT se reportan.
        assert!(
            r.desviaciones
                .iter()
                .any(|d| d.contains("working_directory")),
            "desviaciones: {:?}",
            r.desviaciones
        );
        // Y la comprobación final lo dice.
        let d = desviaciones(&conn).unwrap();
        assert!(d.iter().any(|x| x.contains("working_directory")));
        // Las columnas con DEFAULT sí se añaden.
        assert!(r
            .columnas_creadas
            .iter()
            .any(|c| c == "sessions.current_epoch"));
    }

    #[test]
    fn la_version_aplicada_se_guarda_en_app_state() {
        let conn = bd();
        aplicar(&conn).expect("migración");
        let v: String = conn
            .query_row(
                "SELECT value FROM app_state WHERE key=?1",
                params![CLAVE_VERSION],
                |r| r.get(0),
            )
            .expect("la versión");
        assert_eq!(v, VERSION_ESQUEMA.to_string());
        // Y al repetir la migración no se duplica la fila.
        aplicar(&conn).expect("segunda migración");
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM app_state WHERE key=?1",
                params![CLAVE_VERSION],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn la_introspeccion_rechaza_nombres_de_tabla_invalidos() {
        let conn = bd();
        let malo = listar_columnas(&conn, "x; DROP TABLE sessions").unwrap_err();
        assert!(matches!(malo, rusqlite::Error::InvalidParameterName(_)));
        let malo2 = listar_indices(&conn, "messages\"; --").unwrap_err();
        assert!(matches!(malo2, rusqlite::Error::InvalidParameterName(_)));
    }

    #[test]
    fn la_introspeccion_ve_las_tablas_y_las_columnas() {
        let conn = bd();
        aplicar(&conn).expect("migración");
        let tablas = listar_tablas(&conn).unwrap();
        assert!(tablas.contains(&"message_nodes".to_string()));
        assert!(tablas.contains(&"sessions".to_string()));
        assert!(!tabla_existe(&conn, "no_existe").unwrap());
        let cols = listar_columnas(&conn, "message_nodes").unwrap();
        let nombres: Vec<&str> = cols.iter().map(|c| c.nombre.as_str()).collect();
        assert_eq!(
            nombres,
            vec![
                "row_id",
                "session_id",
                "node_id",
                "parent_node_id",
                "chat_message",
                "created_at"
            ]
        );
        // El UNIQUE (session_id, node_id) es un índice automático de origen 'u'.
        let indices = listar_indices(&conn, "message_nodes").unwrap();
        let unico = indices
            .iter()
            .find(|i| i.origen == "u")
            .expect("el UNIQUE medido");
        assert_eq!(unico.columnas, vec!["session_id", "node_id"]);
    }
}
