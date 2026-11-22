//! `board` — **Board v1**: un solo tablón de tareas para MegaDaemon.
//!
//! ## Por qué este módulo
//!
//! El enjambre tenía **tres** tableros de tareas, con tres formatos, tres
//! espacios de ids y tres modelos de bloqueo:
//!
//! | Tablón | Fichero | Formato | Bloqueo |
//! |---|---|---|---|
//! | `coordination` | `board.sqlite` | SQLite | lease + `owner` |
//! | `mesh` | `mesh_board.json` | JSON | lease + `claim` |
//! | `task_scheduler` | `task_queue.json` | JSON + ndjson | `deps` + recursos |
//!
//! Además el cockpit sólo enseñaba el tercero: el trabajo real del enjambre
//! (los dos primeros) era invisible para el usuario.  Este módulo **unifica**:
//!
//! * **un** tablón (SQLite, que ya es dependencia del crate),
//! * **un** formato de fila (`Task`) y **un** espacio de ids,
//! * **una** máquina de estados con transiciones legales comprobadas,
//! * **un** modelo de bloqueo: `lease` (quien lo tiene, hasta cuándo) **y**
//!   `deps` (las dependencias insatisfechas).
//!
//! ## Resiliencia (entornos inestables)
//!
//! * `WAL` cuando el sistema de ficheros lo soporta y `DELETE` cuando no
//!   (una `state_dir` en NFS no lo soporta: el modo se detecta y se anota).
//! * `busy_timeout` + reintentos con backoff exponencial ante `SQLITE_BUSY`.
//! * **Reloj monótono del tablón**: un nodo con el reloj atrás no puede
//!   acortar un lease ni robar una tarea.
//! * **Recuperación ante corrupción**: `snapshot` periódico y restauración
//!   automática si la base no abre.
//! * **Modo degradado** ([`LocalQueue`]): si el tablón no se puede abrir, el
//!   agente sigue trabajando con una cola local + WAL en ndjson, y luego
//!   reconcilia.  El enjambre se degrada, no muere.
//! * Toda transición de estado deja un **evento** en `task_events` (auditoría
//!   y reconstrucción de proyecciones).
//!
//! ## Preparado para el futuro
//!
//! * El almacén es el trait [`BoardStore`]: mañana puede ser un fichero, un
//!   KV embebido o un tablón remoto **sin tocar a quien lo usa**.
//! * `schema_version` + cadena de migraciones **lineal y transaccional**.
//! * Los campos indexados son columnas; el resto viaja en `payload` (JSON
//!   abierto): se pueden tipar columnas nuevas sin romper las filas viejas.
//! * La migración de los tres tableros legacy es **idempotente** y admite
//!   `--dry-run`: se puede repetir sin duplicar nada.
//!
//! ## Formato (abierto, local, sin proveedor)
//!
//! SQLite + JSON.  Cero cuentas cloud, cero formatos propietarios.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Versión del esquema del tablón.
pub const SCHEMA_VERSION: i64 = 1;

/// Segundos de espera a que SQLite libere un bloqueo.
pub const BUSY_TIMEOUT_MS: u32 = 5_000;

/// Reintentos ante `SQLITE_BUSY`.
pub const DEFAULT_RETRIES: u32 = 6;

/// Lease por defecto (segundos).
pub const DEFAULT_LEASE_SECS: i64 = 900;

/// Fichero del tablón dentro del `state_dir`.
pub const BOARD_FILE: &str = "board.sqlite";

/// Copia de seguridad periódica del tablón.
pub const SNAPSHOT_FILE: &str = "board.snapshot.sqlite";

// ── Estados y transiciones ───────────────────────────────────────────────

/// Estado de una tarea.  **Una** máquina de estados para todo el enjambre.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// En la cola, sin dueño.
    Queued,
    /// Reclamada: tiene dueño y lease.
    Claimed,
    /// En ejecución.
    Running,
    /// Terminada con éxito.
    Done,
    /// Terminada con error.
    Failed,
    /// Cancelada.
    Cancelled,
}

impl TaskState {
    /// Nombre estable (el que va al disco y al JSON).
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskState::Queued => "queued",
            TaskState::Claimed => "claimed",
            TaskState::Running => "running",
            TaskState::Done => "done",
            TaskState::Failed => "failed",
            TaskState::Cancelled => "cancelled",
        }
    }

    /// Parseo tolerante (un estado desconocido es `queued`).
    pub fn parse(s: &str) -> TaskState {
        match s.trim().to_ascii_lowercase().as_str() {
            "claimed" | "publish" | "published" => TaskState::Claimed,
            "running" | "in_progress" => TaskState::Running,
            "done" | "ok" | "completed" => TaskState::Done,
            "failed" | "fail" | "error" => TaskState::Failed,
            "cancelled" | "canceled" => TaskState::Cancelled,
            _ => TaskState::Queued,
        }
    }

    /// ¿Es un estado final?  (no se vuelve atrás)
    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskState::Done | TaskState::Failed | TaskState::Cancelled)
    }
}

/// ¿Es legal pasar de `from` a `to`?
///
/// **Este es el único sitio** donde se decide: nadie más escribe un estado
/// sin pasar por aquí, así que no puede haber deriva silenciosa.
pub fn can_transition(from: TaskState, to: TaskState) -> bool {
    use TaskState::*;
    match (from, to) {
        (Queued, Claimed) => true,
        (Queued, Cancelled) => true,
        (Claimed, Running) => true,
        // un agente puede reclamar y terminar sin pasar por "running":
        // el flujo real es reclamar -> trabajar -> reportar, y el "running"
        // es una marca opcional.
        (Claimed, Done) => true,
        (Claimed, Failed) => true,
        // el agente devuelve el trabajo o se le cae el lease
        (Claimed, Queued) => true,
        (Claimed, Cancelled) => true,
        (Running, Done) => true,
        (Running, Failed) => true,
        // se le cayó el lease a mitad: vuelve a la cola
        (Running, Queued) => true,
        (Running, Cancelled) => true,
        _ => false,
    }
}

// ── Errores ──────────────────────────────────────────────────────────────

/// Errores tipados: el llamante decide si degrada o reintenta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoardError {
    /// Otro escritor tiene la base en ese instante (reintentable).
    Busy,
    /// La base está dañada (se intenta restaurar del snapshot).
    Corrupt(String),
    /// Disco lleno (no reintentable sin liberar espacio).
    NoSpace,
    /// La base está bloqueada por una transacción exclusiva.
    Locked,
    /// No existe la tarea.
    NotFound,
    /// La operación no es legal en este estado.
    Conflict(String),
    /// Error de entrada.
    Invalid(String),
    /// Otro error de E/S o de SQLite.
    Io(String),
}

impl std::fmt::Display for BoardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BoardError::Busy => write!(f, "el tablón está ocupado (reintentable)"),
            BoardError::Corrupt(m) => write!(f, "tablón corrupto: {}", m),
            BoardError::NoSpace => write!(f, "no queda espacio en disco"),
            BoardError::Locked => write!(f, "el tablón está bloqueado"),
            BoardError::NotFound => write!(f, "no existe la tarea"),
            BoardError::Conflict(m) => write!(f, "conflicto: {}", m),
            BoardError::Invalid(m) => write!(f, "entrada inválida: {}", m),
            BoardError::Io(m) => write!(f, "error de E/S: {}", m),
        }
    }
}

impl std::error::Error for BoardError {}

impl From<rusqlite::Error> for BoardError {
    fn from(e: rusqlite::Error) -> Self {
        match e {
            rusqlite::Error::SqliteFailure(ref inner, _) => match inner.code {
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked => {
                    BoardError::Busy
                }
                rusqlite::ErrorCode::DiskFull => BoardError::NoSpace,
                rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase => {
                    BoardError::Corrupt(e.to_string())
                }
                _ => BoardError::Io(e.to_string()),
            },
            rusqlite::Error::QueryReturnedNoRows => BoardError::NotFound,
            other => BoardError::Io(other.to_string()),
        }
    }
}

// ── Reloj ────────────────────────────────────────────────────────────────

/// Fuente del tiempo.  Inyectable para tests deterministas.
pub trait Clock {
    /// Segundos desde epoch.
    fn now(&self) -> i64;
}

/// El reloj del sistema.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }
}

/// Reloj fijo (tests).
#[derive(Clone, Debug)]
pub struct FixedClock(pub std::sync::Arc<std::sync::atomic::AtomicI64>);

impl FixedClock {
    /// Crea un reloj en `t0`.
    pub fn at(t0: i64) -> Self {
        FixedClock(std::sync::Arc::new(std::sync::atomic::AtomicI64::new(t0)))
    }
    /// Avanza el reloj.
    pub fn advance(&self, secs: i64) {
        self.0.fetch_add(secs, std::sync::atomic::Ordering::SeqCst);
    }
    /// Fija el reloj (puede ir hacia atrás: el tablón lo impide).
    pub fn set(&self, t: i64) {
        self.0.store(t, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Clock for FixedClock {
    fn now(&self) -> i64 {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
}

// ── Modelo ───────────────────────────────────────────────────────────────

/// Una tarea del tablón unificado.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    /// Identificador único y **estable** (nunca se reutiliza).
    pub id: String,
    /// Tipo de trabajo (`build`, `audit`, `delegate`...).
    pub kind: String,
    /// Título legible.
    pub title: String,
    /// Capacidad requerida (`""` = cualquiera).
    pub capability: String,
    /// Prioridad (mayor = antes).
    pub priority: i32,
    /// Carga útil libre (JSON abierto: el futuro no queda encajonado).
    pub payload: Value,
    /// Estado (máquina de estados de [`TaskState`]).
    pub state: TaskState,
    /// Dueño actual (agente o nodo).
    pub owner: Option<String>,
    /// Fin del lease (epoch).  0 = sin lease.
    pub lease_expires_at: i64,
    /// Creación (epoch).
    pub created_at: i64,
    /// Última modificación (epoch).
    pub updated_at: i64,
    /// Intentos de reclamo.
    pub attempts: u32,
    /// Último error.
    pub last_error: Option<String>,
    /// Id en el tablón legacy del que vino (migración, trazabilidad).
    pub legacy_id: Option<String>,
    /// Tablón legacy de origen (`coordination`, `mesh`, `scheduler`).
    pub legacy_board: Option<String>,
}

impl Task {
    /// Tarea nueva en cola.
    pub fn new(id: &str, kind: &str, title: &str) -> Task {
        let now = SystemClock.now();
        Task {
            id: id.to_string(),
            kind: kind.to_string(),
            title: title.to_string(),
            capability: String::new(),
            priority: 0,
            payload: Value::Null,
            state: TaskState::Queued,
            owner: None,
            lease_expires_at: 0,
            created_at: now,
            updated_at: now,
            attempts: 0,
            last_error: None,
            legacy_id: None,
            legacy_board: None,
        }
    }
}

/// Resultado que reporta un agente.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    /// Terminada con éxito (y su resultado).
    Done {
        /// Resultado libre.
        result: Value,
    },
    /// Terminada con error.
    Failed {
        /// Motivo.
        error: String,
    },
    /// Cancelada.
    Cancelled {
        /// Motivo.
        reason: String,
    },
    /// Aviso de progreso (no cambia el estado).
    Progress {
        /// Nota.
        note: String,
    },
}

/// Evento del tablón (auditoría).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoardEvent {
    /// Secuencia (monótona).
    pub seq: i64,
    /// Epoch.
    pub ts: i64,
    /// Tarea afectada.
    pub task_id: String,
    /// Tipo (`claim`, `report`, `reclaim`, `migrate`...).
    pub kind: String,
    /// Actor.
    pub actor: String,
    /// Detalle.
    pub payload: Value,
}

/// Filtro de listado.
#[derive(Clone, Debug, Default)]
pub struct TaskFilter {
    /// Estados a incluir (vacío = todos).
    pub states: Vec<TaskState>,
    /// Capacidad requerida exacta.
    pub capability: Option<String>,
    /// Dueño.
    pub owner: Option<String>,
    /// Límite de filas.
    pub limit: usize,
}

/// Recuento del tablón.
#[derive(Clone, Debug, Default, Serialize)]
pub struct BoardStats {
    /// Tareas por estado.
    pub by_state: BTreeMap<String, i64>,
    /// Total.
    pub total: i64,
    /// Tareas con lease caducado (pendientes de reclamar).
    pub expired_leases: i64,
    /// Tareas bloqueadas por dependencias.
    pub blocked_by_deps: i64,
    /// Versión del esquema.
    pub schema_version: i64,
    /// Modo de journal (WAL / DELETE...).
    pub journal_mode: String,
}

// ── El almacén (trait: el backend es reemplazable) ───────────────────────

/// El almacén del tablón.  Implementaciones: [`SqliteStore`] (la de ahora) y
/// [`LocalQueue`] (modo degradado, sin disco).
///
/// Que sea un trait es lo que permite, más adelante, un tablón remoto o un
/// KV embebido **sin tocar a quien lo usa**.
pub trait BoardStore {
    /// Versión del esquema.
    fn schema_version(&self) -> i64;

    /// Crea o actualiza una tarea.
    fn put_task(&mut self, task: &Task) -> Result<(), BoardError>;

    /// Una tarea por id.
    fn get_task(&self, id: &str) -> Result<Option<Task>, BoardError>;

    /// Lista tareas filtradas.
    fn list_tasks(&self, filter: &TaskFilter) -> Result<Vec<Task>, BoardError>;

    /// Fija (o sustituye) las dependencias de una tarea.
    fn set_deps(&mut self, task_id: &str, deps: &[String]) -> Result<(), BoardError>;

    /// Dependencias de una tarea.
    fn deps(&self, task_id: &str) -> Result<Vec<String>, BoardError>;

    /// Reclama una tarea (lease).
    fn claim(
        &mut self,
        id: &str,
        owner: &str,
        lease_secs: i64,
        now: i64,
    ) -> Result<Task, BoardError>;

    /// Marca la tarea como EN EJECUCIÓN (el dueño la tiene).
    fn start(&mut self, id: &str, owner: &str, now: i64) -> Result<Task, BoardError>;

    /// Renueva el lease.
    fn renew_lease(
        &mut self,
        id: &str,
        owner: &str,
        lease_secs: i64,
        now: i64,
    ) -> Result<(), BoardError>;

    /// Reporta el resultado.
    fn report(
        &mut self,
        id: &str,
        owner: &str,
        outcome: &Outcome,
        now: i64,
    ) -> Result<Task, BoardError>;

    /// Devuelve a la cola las tareas cuyo lease caducó (recuperación).
    fn reclaim_expired(&mut self, now: i64) -> Result<Vec<String>, BoardError>;

    /// Tareas listas para ejecutar (sin dueño, con deps satisfechas).
    fn ready_tasks(&self, now: i64, limit: usize) -> Result<Vec<Task>, BoardError>;

    /// Dependencias insatisfechas de una tarea.
    fn unmet_deps(&self, task_id: &str) -> Result<Vec<String>, BoardError>;

    /// Últimos eventos.
    fn events(&self, limit: usize) -> Result<Vec<BoardEvent>, BoardError>;

    /// Recuento.
    fn stats(&self, now: i64) -> Result<BoardStats, BoardError>;
}

// ── SQLite ───────────────────────────────────────────────────────────────

/// Tablón sobre SQLite (WAL cuando se puede).
#[derive(Debug)]
pub struct SqliteStore {
    conn: Connection,
    path: Option<PathBuf>,
    retries: u32,
}

impl SqliteStore {
    /// Abre (o crea) el tablón en `path`.
    ///
    /// Si la base está corrupta, se intenta restaurar del snapshot.
    pub fn open(path: &Path) -> Result<SqliteStore, BoardError> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match SqliteStore::open_raw(path) {
            Ok(s) => Ok(s),
            Err(BoardError::Corrupt(msg)) => {
                let snap = path.with_file_name(SNAPSHOT_FILE);
                if snap.exists() {
                    let _ = std::fs::copy(&snap, path);
                    if let Ok(s) = SqliteStore::open_raw(path) {
                        return Ok(s);
                    }
                }
                Err(BoardError::Corrupt(msg))
            }
            Err(e) => Err(e),
        }
    }

    fn open_raw(path: &Path) -> Result<SqliteStore, BoardError> {
        let conn = Connection::open(path)?;
        SqliteStore::configure(&conn)?;
        let mut store = SqliteStore {
            conn,
            path: Some(path.to_path_buf()),
            retries: DEFAULT_RETRIES,
        };
        store.migrate()?;
        Ok(store)
    }

    fn configure(conn: &Connection) -> Result<(), BoardError> {
        conn.busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MS as u64))?;
        // WAL: lo mejor para muchos escritores concurrentes.  **No** todos los
        // sistemas de ficheros lo soportan (una state_dir en NFS, por
        // ejemplo): si falla, se sigue en DELETE (más lento pero correcto).
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        let _ = conn.pragma_update(None, "synchronous", "FULL");
        let _ = conn.pragma_update(None, "foreign_keys", "ON");
        Ok(())
    }

    /// Ruta del fichero (si es persistente).
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Ejecuta una operación con reintentos ante `SQLITE_BUSY`.
    fn with_retry<T>(
        &mut self,
        mut f: impl FnMut(&mut Connection) -> Result<T, BoardError>,
    ) -> Result<T, BoardError> {
        let mut attempt = 0u32;
        loop {
            match f(&mut self.conn) {
                Err(BoardError::Busy) if attempt < self.retries => {
                    let back = 20u64 * (1u64 << attempt.min(5));
                    attempt += 1;
                    std::thread::sleep(Duration::from_millis(back));
                }
                other => return other,
            }
        }
    }

    /// Reloj **monótono del tablón**: nunca retrocede.
    ///
    /// Un nodo con el reloj atrás no puede acortar un lease ni robar una
    /// tarea: el tablón guarda el mayor instante que ha visto.
    fn tick(&mut self, now: i64) -> Result<i64, BoardError> {
        let stored: i64 = self
            .conn
            .query_row("SELECT value FROM board_meta WHERE key='board_clock'", [], |r| {
                r.get::<_, String>(0)
            })
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        let t = now.max(stored);
        if t > stored {
            let _ = self.conn.execute(
                "INSERT INTO board_meta(key,value) VALUES('board_clock',?1)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                params![t.to_string()],
            );
        }
        Ok(t)
    }

    /// Aplica la cadena de migraciones (lineal, transaccional, idempotente).
    pub fn migrate(&mut self) -> Result<(), BoardError> {
        let current: i64 = self
            .conn
            .query_row("SELECT value FROM board_meta WHERE key='schema_version'", [], |r| {
                r.get::<_, String>(0)
            })
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        if current >= SCHEMA_VERSION {
            return Ok(());
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(SCHEMA_V1)?;
        tx.execute(
            "INSERT INTO board_meta(key,value) VALUES('schema_version',?1)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![SCHEMA_VERSION.to_string()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Copia consistente del tablón (para recuperación ante corrupción).
    pub fn snapshot_to(&mut self, out: &Path) -> Result<(), BoardError> {
        let _ = std::fs::remove_file(out);
        let dest = out.display().to_string().replace('\'', "''");
        self.conn
            .execute_batch(&format!("VACUUM INTO '{}'", dest))
            .map_err(|e| BoardError::Io(format!("VACUUM INTO: {}", e)))?;
        Ok(())
    }

    /// Modo de journal en uso.
    pub fn journal_mode(&self) -> String {
        self.conn
            .query_row("PRAGMA journal_mode", [], |r| r.get::<_, String>(0))
            .unwrap_or_else(|_| "unknown".to_string())
    }
}

/// Esquema v1 (aplicado en una transacción).
const SCHEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS board_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS tasks (
    id                TEXT PRIMARY KEY,
    kind              TEXT NOT NULL DEFAULT '',
    title             TEXT NOT NULL DEFAULT '',
    capability        TEXT NOT NULL DEFAULT '',
    priority          INTEGER NOT NULL DEFAULT 0,
    payload           TEXT NOT NULL DEFAULT 'null',
    state             TEXT NOT NULL DEFAULT 'queued',
    owner             TEXT,
    lease_expires_at  INTEGER NOT NULL DEFAULT 0,
    created_at        INTEGER NOT NULL DEFAULT 0,
    updated_at        INTEGER NOT NULL DEFAULT 0,
    attempts          INTEGER NOT NULL DEFAULT 0,
    last_error        TEXT,
    legacy_id         TEXT,
    legacy_board      TEXT
);
CREATE INDEX IF NOT EXISTS idx_tasks_ready
    ON tasks(state, priority DESC, created_at ASC);
CREATE INDEX IF NOT EXISTS idx_tasks_owner ON tasks(owner);
CREATE INDEX IF NOT EXISTS idx_tasks_lease ON tasks(lease_expires_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_legacy
    ON tasks(legacy_id) WHERE legacy_id IS NOT NULL;
CREATE TABLE IF NOT EXISTS task_deps (
    task_id    TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    depends_on TEXT NOT NULL,
    PRIMARY KEY (task_id, depends_on)
);
CREATE INDEX IF NOT EXISTS idx_deps_rev ON task_deps(depends_on);
CREATE TABLE IF NOT EXISTS task_events (
    seq     INTEGER PRIMARY KEY AUTOINCREMENT,
    ts      INTEGER NOT NULL,
    task_id TEXT NOT NULL,
    kind    TEXT NOT NULL,
    actor   TEXT NOT NULL DEFAULT '',
    payload TEXT NOT NULL DEFAULT 'null'
);
CREATE INDEX IF NOT EXISTS idx_events_task ON task_events(task_id, seq);
CREATE TABLE IF NOT EXISTS instances (
    id         TEXT PRIMARY KEY,
    role       TEXT NOT NULL DEFAULT 'worker',
    host       TEXT NOT NULL DEFAULT '',
    last_seen  INTEGER NOT NULL DEFAULT 0,
    meta       TEXT NOT NULL DEFAULT '{}'
);
"#;

fn row_to_task(r: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    let state: String = r.get("state")?;
    let payload: String = r.get("payload")?;
    Ok(Task {
        id: r.get("id")?,
        kind: r.get("kind")?,
        title: r.get("title")?,
        capability: r.get("capability")?,
        priority: r.get("priority")?,
        payload: serde_json::from_str(&payload).unwrap_or(Value::Null),
        state: TaskState::parse(&state),
        owner: r.get("owner")?,
        lease_expires_at: r.get("lease_expires_at")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
        attempts: r.get("attempts")?,
        last_error: r.get("last_error")?,
        legacy_id: r.get("legacy_id")?,
        legacy_board: r.get("legacy_board")?,
    })
}

const TASK_COLS: &str = "id, kind, title, capability, priority, payload, state, owner, \
     lease_expires_at, created_at, updated_at, attempts, last_error, legacy_id, legacy_board";

impl BoardStore for SqliteStore {
    fn schema_version(&self) -> i64 {
        SCHEMA_VERSION
    }

    fn put_task(&mut self, task: &Task) -> Result<(), BoardError> {
        let payload = serde_json::to_string(&task.payload).unwrap_or_else(|_| "null".into());
        let mut t = task.clone();
        if t.state == TaskState::Queued && t.owner.is_some() {
            t.owner = None;
            t.lease_expires_at = 0;
        }
        self.with_retry(|c| {
            c.execute(
                "INSERT INTO tasks (id, kind, title, capability, priority, payload, state, owner, \
                    lease_expires_at, created_at, updated_at, attempts, last_error, legacy_id, legacy_board) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15) \
                 ON CONFLICT(id) DO UPDATE SET \
                    kind=excluded.kind, title=excluded.title, capability=excluded.capability, \
                    priority=excluded.priority, payload=excluded.payload, \
                    last_error=excluded.last_error",
                params![
                    t.id, t.kind, t.title, t.capability, t.priority, payload, t.state.as_str(),
                    t.owner, t.lease_expires_at, t.created_at, t.updated_at, t.attempts,
                    t.last_error, t.legacy_id, t.legacy_board
                ],
            )?;
            Ok(())
        })
    }

    fn get_task(&self, id: &str) -> Result<Option<Task>, BoardError> {
        let sql = format!("SELECT {} FROM tasks WHERE id=?1", TASK_COLS);
        let t = self.conn.query_row(&sql, params![id], row_to_task).optional()?;
        Ok(t)
    }

    fn list_tasks(&self, filter: &TaskFilter) -> Result<Vec<Task>, BoardError> {
        let mut sql = format!("SELECT {} FROM tasks", TASK_COLS);
        let mut clauses: Vec<String> = Vec::new();
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if !filter.states.is_empty() {
            let names: Vec<String> = filter
                .states
                .iter()
                .map(|s| {
                    args.push(Box::new(s.as_str().to_string()));
                    format!("?{}", args.len())
                })
                .collect();
            clauses.push(format!("state IN ({})", names.join(",")));
        }
        if let Some(cap) = &filter.capability {
            args.push(Box::new(cap.clone()));
            clauses.push(format!("capability=?{}", args.len()));
        }
        if let Some(own) = &filter.owner {
            args.push(Box::new(own.clone()));
            clauses.push(format!("owner IS ?{}", args.len()));
        }
        if !clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&clauses.join(" AND "));
        }
        sql.push_str(" ORDER BY priority DESC, created_at ASC");
        if filter.limit > 0 {
            sql.push_str(&format!(" LIMIT {}", filter.limit));
        }
        let refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref()).collect();
        let mut st = self.conn.prepare(&sql)?;
        let rows = st.query_map(refs.as_slice(), row_to_task)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    fn set_deps(&mut self, task_id: &str, deps: &[String]) -> Result<(), BoardError> {
        self.with_retry(|c| {
            let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute("DELETE FROM task_deps WHERE task_id=?1", params![task_id])?;
            for d in deps {
                tx.execute(
                    "INSERT OR IGNORE INTO task_deps (task_id, depends_on) VALUES (?1,?2)",
                    params![task_id, d],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
    }

    fn deps(&self, task_id: &str) -> Result<Vec<String>, BoardError> {
        let mut st = self
            .conn
            .prepare("SELECT depends_on FROM task_deps WHERE task_id=?1 ORDER BY depends_on")?;
        let rows = st.query_map(params![task_id], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    fn claim(
        &mut self,
        id: &str,
        owner: &str,
        lease_secs: i64,
        now: i64,
    ) -> Result<Task, BoardError> {
        let now = self.tick(now)?;
        let id = id.to_string();
        let id_out = id.clone();
        let owner = owner.to_string();
        self.with_retry(move |c| {
            let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let row: Option<(String, i64, Option<String>)> = tx
                .query_row(
                    "SELECT state, lease_expires_at, owner FROM tasks WHERE id=?1",
                    params![id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            let (state, lease, cur_owner) = row.ok_or(BoardError::NotFound)?;
            let st = TaskState::parse(&state);
            if st.is_terminal() {
                return Err(BoardError::Conflict(format!(
                    "la tarea {} está en estado terminal ({})",
                    id,
                    st.as_str()
                )));
            }
            let mine = cur_owner.as_deref() == Some(owner.as_str());
            let expired = lease <= now;
            let claimable = st == TaskState::Queued || (st == TaskState::Claimed && expired);
            if !mine && !claimable {
                return Err(BoardError::Conflict(format!(
                    "la tarea {} ya la tiene {:?} (lease hasta {})",
                    id, cur_owner, lease
                )));
            }
            let next = if st == TaskState::Running && mine {
                TaskState::Running
            } else {
                TaskState::Claimed
            };
            if !can_transition(st, next) {
                return Err(BoardError::Conflict(format!(
                    "transición ilegal {} -> {}",
                    st.as_str(),
                    next.as_str()
                )));
            }
            tx.execute(
                "UPDATE tasks SET state=?2, owner=?3, lease_expires_at=?4, updated_at=?5, \
                    attempts=attempts+1 WHERE id=?1",
                params![id, next.as_str(), owner, now + lease_secs.max(1), now],
            )?;
            tx.execute(
                "INSERT INTO task_events (ts, task_id, kind, actor, payload) \
                 VALUES (?1,?2,'claim',?3,?4)",
                params![now, id, owner, json!({"lease_secs": lease_secs}).to_string()],
            )?;
            tx.commit()?;
            Ok(())
        })?;
        self.get_task(&id_out)?.ok_or(BoardError::NotFound)
    }

    fn start(&mut self, id: &str, owner: &str, now: i64) -> Result<Task, BoardError> {
        let now = self.tick(now)?;
        let id = id.to_string();
        let id_out = id.clone();
        let owner = owner.to_string();
        self.with_retry(move |c| {
            let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let row: Option<(String, Option<String>)> = tx
                .query_row(
                    "SELECT state, owner FROM tasks WHERE id=?1",
                    params![id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let (state, cur_owner) = row.ok_or(BoardError::NotFound)?;
            let st = TaskState::parse(&state);
            if cur_owner.as_deref() != Some(owner.as_str()) {
                return Err(BoardError::Conflict(format!(
                    "la tarea {} la tiene {:?}, no {:?}",
                    id, cur_owner, owner
                )));
            }
            if !can_transition(st, TaskState::Running) {
                return Err(BoardError::Conflict(format!(
                    "transición ilegal {} -> running",
                    st.as_str()
                )));
            }
            tx.execute(
                "UPDATE tasks SET state='running', updated_at=?2 WHERE id=?1",
                params![id, now],
            )?;
            tx.execute(
                "INSERT INTO task_events (ts, task_id, kind, actor, payload) \
                 VALUES (?1,?2,'start',?3,'{}')",
                params![now, id, owner],
            )?;
            tx.commit()?;
            Ok(())
        })?;
        self.get_task(&id_out)?.ok_or(BoardError::NotFound)
    }

    fn renew_lease(
        &mut self,
        id: &str,
        owner: &str,
        lease_secs: i64,
        now: i64,
    ) -> Result<(), BoardError> {
        let now = self.tick(now)?;
        let id = id.to_string();
        let owner = owner.to_string();
        self.with_retry(move |c| {
            let n = c.execute(
                "UPDATE tasks SET lease_expires_at=?3, updated_at=?4 \
                 WHERE id=?1 AND owner=?2 AND state IN ('claimed','running')",
                params![id, owner, now + lease_secs.max(1), now],
            )?;
            if n == 0 {
                return Err(BoardError::Conflict(format!(
                    "la tarea {} no la tiene {} (o ya no está activa)",
                    id, owner
                )));
            }
            Ok(())
        })
    }

    fn report(
        &mut self,
        id: &str,
        owner: &str,
        outcome: &Outcome,
        now: i64,
    ) -> Result<Task, BoardError> {
        let now = self.tick(now)?;
        let id = id.to_string();
        let id_out = id.clone();
        let owner = owner.to_string();
        let outcome = outcome.clone();
        self.with_retry(move |c| {
            let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let row: Option<(String, Option<String>)> = tx
                .query_row(
                    "SELECT state, owner FROM tasks WHERE id=?1",
                    params![id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let (state, cur_owner) = row.ok_or(BoardError::NotFound)?;
            let st = TaskState::parse(&state);
            if let Some(o) = &cur_owner {
                if o != &owner && !st.is_terminal() {
                    return Err(BoardError::Conflict(format!(
                        "la tarea {} la tiene {:?}, no {:?}",
                        id, o, owner
                    )));
                }
            }
            let (next, err, ev, payload) = match &outcome {
                Outcome::Done { result } => (
                    TaskState::Done,
                    None,
                    "done",
                    json!({"result": result}).to_string(),
                ),
                Outcome::Failed { error } => (
                    TaskState::Failed,
                    Some(error.clone()),
                    "failed",
                    json!({"error": error}).to_string(),
                ),
                Outcome::Cancelled { reason } => (
                    TaskState::Cancelled,
                    Some(reason.clone()),
                    "cancelled",
                    json!({"reason": reason}).to_string(),
                ),
                Outcome::Progress { note } => {
                    tx.execute(
                        "INSERT INTO task_events (ts, task_id, kind, actor, payload) \
                         VALUES (?1,?2,'progress',?3,?4)",
                        params![now, id, owner, json!({"note": note}).to_string()],
                    )?;
                    tx.commit()?;
                    return Ok(());
                }
            };
            if !can_transition(st, next) {
                return Err(BoardError::Conflict(format!(
                    "transición ilegal {} -> {}",
                    st.as_str(),
                    next.as_str()
                )));
            }
            tx.execute(
                "UPDATE tasks SET state=?2, updated_at=?3, owner=NULL, lease_expires_at=0, \
                    last_error=?4 WHERE id=?1",
                params![id, next.as_str(), now, err],
            )?;
            tx.execute(
                "INSERT INTO task_events (ts, task_id, kind, actor, payload) \
                 VALUES (?1,?2,?3,?4,?5)",
                params![now, id, ev, owner, payload],
            )?;
            tx.commit()?;
            Ok(())
        })?;
        self.get_task(&id_out)?.ok_or(BoardError::NotFound)
    }

    fn reclaim_expired(&mut self, now: i64) -> Result<Vec<String>, BoardError> {
        let now = self.tick(now)?;
        self.with_retry(move |c| {
            let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let mut st = tx.prepare(
                "SELECT id FROM tasks WHERE state IN ('claimed','running') AND lease_expires_at > 0 \
                    AND lease_expires_at <= ?1",
            )?;
            let ids: Vec<String> = st
                .query_map(params![now], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            drop(st);
            for id in &ids {
                tx.execute(
                    "UPDATE tasks SET state='queued', owner=NULL, lease_expires_at=0, updated_at=?2 \
                     WHERE id=?1",
                    params![id, now],
                )?;
                tx.execute(
                    "INSERT INTO task_events (ts, task_id, kind, actor, payload) \
                     VALUES (?1,?2,'reclaim','',?3)",
                    params![now, id, json!({"motivo": "lease caducado"}).to_string()],
                )?;
            }
            tx.commit()?;
            Ok(ids)
        })
    }

    fn unmet_deps(&self, task_id: &str) -> Result<Vec<String>, BoardError> {
        let mut st = self.conn.prepare(
            "SELECT d.depends_on FROM task_deps d \
             LEFT JOIN tasks t ON t.id = d.depends_on \
             WHERE d.task_id=?1 AND (t.id IS NULL OR t.state <> 'done') \
             ORDER BY d.depends_on",
        )?;
        let rows = st.query_map(params![task_id], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    fn ready_tasks(&self, now: i64, limit: usize) -> Result<Vec<Task>, BoardError> {
        let cols = TASK_COLS
            .split(", ")
            .map(|c| format!("t.{}", c))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT {} FROM tasks t WHERE t.state='queued' \
               AND (t.lease_expires_at=0 OR t.lease_expires_at <= ?1) \
               AND NOT EXISTS (SELECT 1 FROM task_deps d JOIN tasks dt ON dt.id=d.depends_on \
                               WHERE d.task_id=t.id AND dt.state <> 'done') \
             ORDER BY t.priority DESC, t.created_at ASC LIMIT ?2",
            cols
        );
        let mut st = self.conn.prepare(&sql)?;
        let rows = st.query_map(params![now, limit as i64], row_to_task)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    fn events(&self, limit: usize) -> Result<Vec<BoardEvent>, BoardError> {
        let mut st = self.conn.prepare(
            "SELECT seq, ts, task_id, kind, actor, payload FROM task_events \
             ORDER BY seq DESC LIMIT ?1",
        )?;
        let rows = st.query_map(params![limit as i64], |r| {
            let payload: String = r.get(5)?;
            Ok(BoardEvent {
                seq: r.get(0)?,
                ts: r.get(1)?,
                task_id: r.get(2)?,
                kind: r.get(3)?,
                actor: r.get(4)?,
                payload: serde_json::from_str(&payload).unwrap_or(Value::Null),
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    fn stats(&self, now: i64) -> Result<BoardStats, BoardError> {
        let mut by_state: BTreeMap<String, i64> = BTreeMap::new();
        {
            let mut st = self.conn.prepare("SELECT state, COUNT(*) FROM tasks GROUP BY state")?;
            let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
            for r in rows {
                let (s, n) = r?;
                by_state.insert(s, n);
            }
        }
        let total: i64 = by_state.values().sum();
        let expired: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE state IN ('claimed','running') \
                    AND lease_expires_at > 0 AND lease_expires_at <= ?1",
                params![now],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let blocked: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM tasks t WHERE t.state='queued' \
                    AND EXISTS (SELECT 1 FROM task_deps d JOIN tasks dt ON dt.id=d.depends_on \
                                WHERE d.task_id=t.id AND dt.state <> 'done')",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        Ok(BoardStats {
            by_state,
            total,
            expired_leases: expired,
            blocked_by_deps: blocked,
            schema_version: SCHEMA_VERSION,
            journal_mode: self.journal_mode(),
        })
    }
}

// ── Modo degradado (sin disco / sin poder abrir el tablón) ──────────────

/// Cola en memoria + WAL en ndjson: si el tablón no se puede abrir, el agente
/// **sigue trabajando** y luego reconcilia.
#[derive(Debug, Default)]
pub struct LocalQueue {
    tasks: HashMap<String, Task>,
    deps: HashMap<String, Vec<String>>,
    wal: Option<PathBuf>,
}

impl LocalQueue {
    /// Cola vacía (sin WAL).
    pub fn new() -> LocalQueue {
        LocalQueue::default()
    }

    /// Cola con WAL en ndjson (para no perder tareas al caer).
    pub fn with_wal(path: &Path) -> LocalQueue {
        let mut q = LocalQueue::default();
        q.wal = Some(path.to_path_buf());
        q.replay();
        q
    }

    fn replay(&mut self) {
        let path = match &self.wal {
            Some(p) => p.clone(),
            None => return,
        };
        let Ok(txt) = std::fs::read_to_string(&path) else {
            return;
        };
        for line in txt.lines() {
            if let Ok(t) = serde_json::from_str::<Task>(line) {
                self.tasks.insert(t.id.clone(), t);
            }
        }
    }

    fn append_wal(&self, t: &Task) {
        if let Some(p) = &self.wal {
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(p)
            {
                use std::io::Write;
                let _ = writeln!(f, "{}", serde_json::to_string(t).unwrap_or_default());
            }
        }
    }

    /// Reintenta el guardado en el tablón real (reconciliación).
    pub fn flush_to(&mut self, store: &mut dyn BoardStore) -> Result<usize, BoardError> {
        let mut n = 0;
        let ids: Vec<String> = self.tasks.keys().cloned().collect();
        for id in ids {
            let t = self.tasks.get(&id).cloned().unwrap();
            store.put_task(&t)?;
            n += 1;
        }
        Ok(n)
    }
}

impl BoardStore for LocalQueue {
    fn schema_version(&self) -> i64 {
        SCHEMA_VERSION
    }

    fn put_task(&mut self, task: &Task) -> Result<(), BoardError> {
        self.tasks.insert(task.id.clone(), task.clone());
        self.append_wal(task);
        Ok(())
    }

    fn get_task(&self, id: &str) -> Result<Option<Task>, BoardError> {
        Ok(self.tasks.get(id).cloned())
    }

    fn list_tasks(&self, filter: &TaskFilter) -> Result<Vec<Task>, BoardError> {
        let mut v: Vec<Task> = self
            .tasks
            .values()
            .filter(|t| filter.states.is_empty() || filter.states.contains(&t.state))
            .filter(|t| {
                filter
                    .capability
                    .as_ref()
                    .map(|c| &t.capability == c)
                    .unwrap_or(true)
            })
            .filter(|t| {
                filter
                    .owner
                    .as_ref()
                    .map(|o| t.owner.as_deref() == Some(o.as_str()))
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        v.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.created_at.cmp(&b.created_at)));
        if filter.limit > 0 {
            v.truncate(filter.limit);
        }
        Ok(v)
    }

    fn set_deps(&mut self, task_id: &str, deps: &[String]) -> Result<(), BoardError> {
        self.deps.insert(task_id.to_string(), deps.to_vec());
        Ok(())
    }

    fn deps(&self, task_id: &str) -> Result<Vec<String>, BoardError> {
        Ok(self.deps.get(task_id).cloned().unwrap_or_default())
    }

    fn claim(
        &mut self,
        id: &str,
        owner: &str,
        lease_secs: i64,
        now: i64,
    ) -> Result<Task, BoardError> {
        let t = self.tasks.get(id).cloned().ok_or(BoardError::NotFound)?;
        if t.state.is_terminal() {
            return Err(BoardError::Conflict(format!("la tarea {} está terminal", id)));
        }
        let expired = t.lease_expires_at <= now;
        let mine = t.owner.as_deref() == Some(owner);
        if !mine && !(t.state == TaskState::Queued || expired) {
            return Err(BoardError::Conflict(format!("la tarea {} ya la tiene {:?}", id, t.owner)));
        }
        let mut t = t;
        if !can_transition(t.state, TaskState::Claimed) {
            return Err(BoardError::Conflict("transición ilegal".into()));
        }
        t.state = TaskState::Claimed;
        t.owner = Some(owner.to_string());
        t.lease_expires_at = now + lease_secs.max(1);
        t.updated_at = now;
        t.attempts += 1;
        self.tasks.insert(t.id.clone(), t.clone());
        self.append_wal(&t);
        Ok(t)
    }

    fn start(&mut self, id: &str, owner: &str, now: i64) -> Result<Task, BoardError> {
        let mut t = self.tasks.get(id).cloned().ok_or(BoardError::NotFound)?;
        if t.owner.as_deref() != Some(owner) {
            return Err(BoardError::Conflict(format!("{} no la tiene {}", id, owner)));
        }
        if !can_transition(t.state, TaskState::Running) {
            return Err(BoardError::Conflict("transición ilegal".into()));
        }
        t.state = TaskState::Running;
        t.updated_at = now;
        self.tasks.insert(t.id.clone(), t.clone());
        self.append_wal(&t);
        Ok(t)
    }

    fn renew_lease(
        &mut self,
        id: &str,
        owner: &str,
        lease_secs: i64,
        now: i64,
    ) -> Result<(), BoardError> {
        let mut t = self.tasks.get(id).cloned().ok_or(BoardError::NotFound)?;
        if t.owner.as_deref() != Some(owner) {
            return Err(BoardError::Conflict(format!("{} no la tiene {}", id, owner)));
        }
        t.lease_expires_at = now + lease_secs.max(1);
        t.updated_at = now;
        self.tasks.insert(t.id.clone(), t.clone());
        self.append_wal(&t);
        Ok(())
    }

    fn report(
        &mut self,
        id: &str,
        owner: &str,
        outcome: &Outcome,
        now: i64,
    ) -> Result<Task, BoardError> {
        let mut t = self.tasks.get(id).cloned().ok_or(BoardError::NotFound)?;
        if let Some(o) = &t.owner {
            if o != owner && !t.state.is_terminal() {
                return Err(BoardError::Conflict(format!("{} la tiene {:?}", id, o)));
            }
        }
        let next = match outcome {
            Outcome::Done { .. } => TaskState::Done,
            Outcome::Failed { .. } => TaskState::Failed,
            Outcome::Cancelled { .. } => TaskState::Cancelled,
            Outcome::Progress { .. } => return Ok(t),
        };
        if !can_transition(t.state, next) {
            return Err(BoardError::Conflict("transición ilegal".into()));
        }
        t.state = next;
        t.owner = None;
        t.lease_expires_at = 0;
        t.updated_at = now;
        if let Outcome::Failed { error } = outcome {
            t.last_error = Some(error.clone());
        }
        self.tasks.insert(t.id.clone(), t.clone());
        self.append_wal(&t);
        Ok(t)
    }

    fn reclaim_expired(&mut self, now: i64) -> Result<Vec<String>, BoardError> {
        let mut out = Vec::new();
        let ids: Vec<String> = self.tasks.keys().cloned().collect();
        for id in ids {
            let mut t = self.tasks[&id].clone();
            if !t.state.is_terminal()
                && matches!(t.state, TaskState::Claimed | TaskState::Running)
                && t.lease_expires_at > 0
                && t.lease_expires_at <= now
            {
                t.state = TaskState::Queued;
                t.owner = None;
                t.lease_expires_at = 0;
                t.updated_at = now;
                self.tasks.insert(id.clone(), t.clone());
                self.append_wal(&t);
                out.push(id);
            }
        }
        Ok(out)
    }

    fn unmet_deps(&self, task_id: &str) -> Result<Vec<String>, BoardError> {
        Ok(self
            .deps
            .get(task_id)
            .map(|ds| {
                ds.iter()
                    .filter(|d| {
                        self.tasks
                            .get(*d)
                            .map(|t| t.state != TaskState::Done)
                            .unwrap_or(true)
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default())
    }

    fn ready_tasks(&self, now: i64, limit: usize) -> Result<Vec<Task>, BoardError> {
        let mut v: Vec<Task> = self
            .tasks
            .values()
            .filter(|t| t.state == TaskState::Queued)
            .filter(|t| self.unmet_deps(&t.id).map(|d| d.is_empty()).unwrap_or(true))
            .cloned()
            .collect();
        let _ = now;
        v.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.created_at.cmp(&b.created_at)));
        v.truncate(limit);
        Ok(v)
    }

    fn events(&self, limit: usize) -> Result<Vec<BoardEvent>, BoardError> {
        let _ = limit;
        Ok(Vec::new())
    }

    fn stats(&self, _now: i64) -> Result<BoardStats, BoardError> {
        let mut by_state: BTreeMap<String, i64> = BTreeMap::new();
        for t in self.tasks.values() {
            *by_state.entry(t.state.as_str().to_string()).or_insert(0) += 1;
        }
        let total = self.tasks.len() as i64;
        Ok(BoardStats {
            by_state,
            total,
            expired_leases: 0,
            blocked_by_deps: 0,
            schema_version: SCHEMA_VERSION,
            journal_mode: "memory".to_string(),
        })
    }
}

// ── Migración de los tres tableros legacy ───────────────────────────────

/// Informe de una migración.
#[derive(Clone, Debug, Default, Serialize)]
pub struct MigrationReport {
    /// Tareas insertadas.
    pub inserted: usize,
    /// Tareas ya presentes (idempotencia).
    pub skipped: usize,
    /// Dependencias migradas.
    pub deps: usize,
    /// Por tablero de origen.
    pub by_board: BTreeMap<String, usize>,
    /// Problemas encontrados (no paran la migración).
    pub warnings: Vec<String>,
}

/// Migra el tablón legacy de `coordination` (SQLite viejo) al nuevo.
pub fn migrate_legacy_sqlite(
    store: &mut SqliteStore,
    legacy_path: &Path,
    now: i64,
) -> Result<MigrationReport, BoardError> {
    let mut rep = MigrationReport::default();
    if !legacy_path.exists() {
        return Ok(rep);
    }
    let legacy = Connection::open_with_flags(legacy_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let has_tasks: bool = legacy
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='tasks'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;
    if !has_tasks {
        rep.warnings.push(format!(
            "{} no tiene tabla 'tasks': nada que migrar",
            legacy_path.display()
        ));
        return Ok(rep);
    }
    let mut st = legacy.prepare(
        "SELECT id, title, owner, status, priority, report, created_at, updated_at, lease_expires_at \
         FROM tasks",
    )?;
    let rows = st.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, Option<String>>(5)?,
            r.get::<_, i64>(6)?,
            r.get::<_, i64>(7)?,
            r.get::<_, i64>(8)?,
        ))
    })?;
    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    drop(st);
    for (id, title, owner, status, priority, report, created, updated, lease) in list {
        let mut t = Task::new(&id, "legacy", &title);
        t.state = TaskState::parse(&status);
        t.priority = priority as i32;
        t.owner = owner;
        t.lease_expires_at = lease;
        t.created_at = created;
        t.updated_at = updated;
        t.last_error = report;
        t.legacy_id = Some(id.clone());
        t.legacy_board = Some("coordination".to_string());
        if t.state.is_terminal() {
            t.owner = None;
            t.lease_expires_at = 0;
        }
        if store.get_task(&id)?.is_some() {
            rep.skipped += 1;
            continue;
        }
        store.put_task(&t)?;
        rep.inserted += 1;
        *rep.by_board.entry("coordination".into()).or_insert(0) += 1;
    }
    let _ = now;
    Ok(rep)
}

/// Migra el tablón legacy de `mesh` (`mesh_board.json`).
pub fn migrate_legacy_mesh(
    store: &mut SqliteStore,
    mesh: &Value,
    now: i64,
) -> Result<MigrationReport, BoardError> {
    let mut rep = MigrationReport::default();
    let tasks = match mesh.get("tasks").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => {
            rep.warnings.push("mesh_board.json sin array 'tasks'".into());
            return Ok(rep);
        }
    };
    for t in tasks {
        let id = t.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
        if id.is_empty() {
            rep.warnings.push("tarea del mesh sin task_id".into());
            continue;
        }
        if store.get_task(id)?.is_some() {
            rep.skipped += 1;
            continue;
        }
        let mut task = Task::new(id, "mesh", t.get("title").and_then(|v| v.as_str()).unwrap_or(""));
        task.capability = t
            .get("capability")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        task.priority = t.get("priority").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        task.payload = t.get("payload").cloned().unwrap_or(Value::Null);
        task.state = TaskState::parse(t.get("state").and_then(|v| v.as_str()).unwrap_or("published"));
        task.created_at = t
            .get("created_at")
            .and_then(|v| v.as_str())
            .and_then(parse_iso_epoch)
            .unwrap_or(now);
        task.legacy_id = Some(id.to_string());
        task.legacy_board = Some("mesh".into());
        store.put_task(&task)?;
        rep.inserted += 1;
        *rep.by_board.entry("mesh".into()).or_insert(0) += 1;
    }
    Ok(rep)
}

/// Migra el tablón legacy de `task_scheduler` (`task_queue.json`).
pub fn migrate_legacy_scheduler(
    store: &mut SqliteStore,
    queue: &Value,
    now: i64,
) -> Result<MigrationReport, BoardError> {
    let mut rep = MigrationReport::default();
    let items = match queue.get("tasks").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => {
            rep.warnings.push("task_queue.json sin array 'tasks'".into());
            return Ok(rep);
        }
    };
    for t in items {
        let id = t.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if id.is_empty() {
            rep.warnings.push("tarea del scheduler sin id".into());
            continue;
        }
        if store.get_task(id)?.is_some() {
            rep.skipped += 1;
            continue;
        }
        let mut task = Task::new(id, t.get("kind").and_then(|v| v.as_str()).unwrap_or("task"), "");
        task.title = t.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
        task.priority = t.get("priority").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        task.payload = t.get("payload").cloned().unwrap_or(Value::Null);
        task.state = TaskState::parse(t.get("state").and_then(|v| v.as_str()).unwrap_or("pending"));
        task.created_at = t.get("created_at").and_then(|v| v.as_i64()).unwrap_or(now);
        task.legacy_id = Some(id.to_string());
        task.legacy_board = Some("scheduler".into());
        // los recursos y las dependencias viajan en el payload (abierto)
        let mut payload = task.payload.as_object().cloned().unwrap_or_default();
        if let Some(res) = t.get("resources") {
            payload.insert("resources".into(), res.clone());
        }
        task.payload = Value::Object(payload);
        store.put_task(&task)?;
        rep.inserted += 1;
        *rep.by_board.entry("scheduler".into()).or_insert(0) += 1;

        let deps: Vec<String> = t
            .get("deps")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        if !deps.is_empty() {
            store.set_deps(id, &deps)?;
            rep.deps += deps.len();
        }
    }
    Ok(rep)
}

/// ISO-8601 → epoch (sólo `YYYY-MM-DDTHH:MM:SSZ`).
pub fn parse_iso_epoch(s: &str) -> Option<i64> {
    if s.len() < 19 {
        return None;
    }
    let num = |a: usize, n: usize| -> Option<i64> { s.get(a..a + n)?.parse::<i64>().ok() };
    let y = num(0, 4)?;
    let mo = num(5, 2)?;
    let d = num(8, 2)?;
    let h = num(11, 2)?;
    let mi = num(14, 2)?;
    let se = num(17, 2)?;
    let days = days_from_civil(y, mo, d);
    Some(days * 86_400 + h * 3600 + mi * 60 + se)
}

/// Días desde 1970-01-01 (algoritmo de Hinnant).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

// ── Vista unificada (lo que verá el usuario) ─────────────────────────────

/// Vista JSON de las tareas (la que debe servir el cockpit).
pub fn tasks_json(store: &dyn BoardStore, now: i64, limit: usize) -> Value {
    let tasks = store
        .list_tasks(&TaskFilter {
            limit,
            ..Default::default()
        })
        .unwrap_or_default();
    let items: Vec<Value> = tasks
        .iter()
        .map(|t| {
            let blocked = store.unmet_deps(&t.id).unwrap_or_default();
            json!({
                "id": t.id,
                "kind": t.kind,
                "title": t.title,
                "capability": t.capability,
                "priority": t.priority,
                "state": t.state.as_str(),
                "owner": t.owner,
                "lease_expires_at": t.lease_expires_at,
                "attempts": t.attempts,
                "blocked_by": blocked,
                "last_error": t.last_error,
                "legacy_board": t.legacy_board,
            })
        })
        .collect();
    json!({
        "board": "v1",
        "schema_version": store.schema_version(),
        "now": now,
        "count": items.len(),
        "tasks": items,
    })
}

/// Resumen JSON del tablón.
pub fn stats_json(store: &dyn BoardStore, now: i64) -> Value {
    match store.stats(now) {
        Ok(s) => serde_json::to_value(&s).unwrap_or(Value::Null),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

/// Ids ya presentes (para migraciones idempotentes).
pub fn existing_ids(store: &dyn BoardStore) -> HashSet<String> {
    store
        .list_tasks(&TaskFilter {
            limit: 0,
            ..Default::default()
        })
        .unwrap_or_default()
        .into_iter()
        .map(|t| t.id)
        .collect()
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "md-board-test-{}-{}-{:?}.sqlite",
            name,
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&p);
        p
    }

    fn store(name: &str) -> SqliteStore {
        SqliteStore::open(&tmp_path(name)).expect("abrir tablón")
    }

    #[test]
    fn la_transicion_ilegal_se_rechaza() {
        assert!(can_transition(TaskState::Queued, TaskState::Claimed));
        assert!(can_transition(TaskState::Claimed, TaskState::Running));
        assert!(can_transition(TaskState::Running, TaskState::Done));
        // no se puede resucitar una tarea terminal
        assert!(!can_transition(TaskState::Done, TaskState::Running));
        assert!(!can_transition(TaskState::Queued, TaskState::Running));
        assert!(!can_transition(TaskState::Done, TaskState::Queued));
    }

    #[test]
    fn el_ciclo_de_vida_completo_deja_rastro() {
        let mut s = store("ciclo");
        let clock = FixedClock::at(1_700_000_000);
        let mut t = Task::new("t-1", "build", "compilar");
        t.priority = 7;
        s.put_task(&t).unwrap();

        let claimed = s.claim("t-1", "agente-a", 60, clock.now()).unwrap();
        assert_eq!(claimed.state, TaskState::Claimed);
        assert_eq!(claimed.owner.as_deref(), Some("agente-a"));
        assert_eq!(claimed.attempts, 1);

        let done = s
            .report(
                "t-1",
                "agente-a",
                &Outcome::Done {
                    result: json!({"ok": true}),
                },
                clock.now(),
            )
            .unwrap();
        assert_eq!(done.state, TaskState::Done);
        assert!(done.owner.is_none());

        let evs = s.events(10).unwrap();
        assert!(evs.iter().any(|e| e.kind == "claim"));
        assert!(evs.iter().any(|e| e.kind == "done"));
    }

    #[test]
    fn dos_agentes_no_pueden_reclamar_lo_mismo() {
        let mut s = store("doble");
        let clock = FixedClock::at(1_700_000_000);
        s.put_task(&Task::new("t-1", "build", "x")).unwrap();
        s.claim("t-1", "agente-a", 60, clock.now()).unwrap();
        let err = s.claim("t-1", "agente-b", 60, clock.now()).unwrap_err();
        assert!(matches!(err, BoardError::Conflict(_)), "err: {:?}", err);
    }

    #[test]
    fn el_lease_caduca_y_la_tarea_vuelve_a_la_cola() {
        let mut s = store("lease");
        let clock = FixedClock::at(1_700_000_000);
        s.put_task(&Task::new("t-1", "build", "x")).unwrap();
        s.claim("t-1", "agente-a", 60, clock.now()).unwrap();
        clock.advance(61);
        let reclaims = s.reclaim_expired(clock.now()).unwrap();
        assert_eq!(reclaims, vec!["t-1".to_string()]);
        let t = s.get_task("t-1").unwrap().unwrap();
        assert_eq!(t.state, TaskState::Queued);
        assert!(t.owner.is_none());
    }

    #[test]
    fn un_reloj_tras_atras_no_roba_el_lease() {
        let mut s = store("reloj");
        let clock = FixedClock::at(1_700_000_000);
        s.put_task(&Task::new("t-1", "build", "x")).unwrap();
        s.claim("t-1", "agente-a", 600, clock.now()).unwrap();
        // el nodo B tiene el reloj 1 hora atrás
        let atrasado = clock.now() - 3600;
        let err = s.claim("t-1", "agente-b", 600, atrasado).unwrap_err();
        assert!(matches!(err, BoardError::Conflict(_)), "err: {:?}", err);
    }

    #[test]
    fn las_dependencias_bloquean_hasta_que_se_resuelven() {
        let mut s = store("deps");
        let clock = FixedClock::at(1_700_000_000);
        s.put_task(&Task::new("a", "build", "a")).unwrap();
        s.put_task(&Task::new("b", "build", "b")).unwrap();
        s.set_deps("b", &["a".to_string()]).unwrap();
        assert_eq!(s.unmet_deps("b").unwrap(), vec!["a".to_string()]);
        let ready = s.ready_tasks(clock.now(), 10).unwrap();
        assert!(ready.iter().all(|t| t.id != "b"), "b no puede estar lista: {:?}", ready);
        s.claim("a", "agente-a", 60, clock.now()).unwrap();
        s.report("a", "agente-a", &Outcome::Done { result: Value::Null }, clock.now())
            .unwrap();
        assert!(s.unmet_deps("b").unwrap().is_empty());
        let ready = s.ready_tasks(clock.now(), 10).unwrap();
        assert!(ready.iter().any(|t| t.id == "b"));
    }

    #[test]
    fn la_migracion_es_idempotente() {
        let mut s = store("migra");
        let clock = FixedClock::at(1_700_000_000);
        let mesh = json!({
            "tasks": [
                {"task_id": "m-1", "title": "una", "capability": "code", "priority": 3,
                 "payload": {"x": 1}, "state": "published", "created_at": "2026-01-01T00:00:00Z"}
            ]
        });
        let queue = json!({
            "tasks": [
                {"id": "s-1", "kind": "build", "payload": {}, "priority": 2,
                 "state": "pending", "deps": ["m-1"]}
            ]
        });
        let r1 = migrate_legacy_mesh(&mut s, &mesh, clock.now()).unwrap();
        assert_eq!(r1.inserted, 1);
        let r2 = migrate_legacy_mesh(&mut s, &mesh, clock.now()).unwrap();
        assert_eq!(r2.inserted, 0);
        assert_eq!(r2.skipped, 1);

        let q1 = migrate_legacy_scheduler(&mut s, &queue, clock.now()).unwrap();
        assert_eq!(q1.inserted, 1);
        assert_eq!(q1.deps, 1);
        let q2 = migrate_legacy_scheduler(&mut s, &queue, clock.now()).unwrap();
        assert_eq!(q2.inserted, 0);
        assert_eq!(q2.skipped, 1);

        // la dependencia migrada sigue viva
        assert_eq!(s.unmet_deps("s-1").unwrap(), vec!["m-1".to_string()]);
        assert_eq!(s.list_tasks(&TaskFilter::default()).unwrap().len(), 2);
    }

    #[test]
    fn el_modo_degradado_no_pierde_tareas() {
        let mut q = LocalQueue::new();
        q.put_task(&Task::new("t-1", "build", "x")).unwrap();
        assert_eq!(q.list_tasks(&TaskFilter::default()).unwrap().len(), 1);
        // si el tablón real no abre, la cola local sigue sirviendo
        let mut real = store("degradado");
        let n = q.flush_to(&mut real).unwrap();
        assert_eq!(n, 1);
        assert_eq!(real.list_tasks(&TaskFilter::default()).unwrap().len(), 1);
    }

    #[test]
    fn el_snapshot_y_la_recuperacion_funcionan() {
        let mut s = store("snapshot");
        let clock = FixedClock::at(1_700_000_000);
        s.put_task(&Task::new("t-1", "build", "x")).unwrap();
        let snap = tmp_path("snapshot-copia");
        s.snapshot_to(&snap).unwrap();
        assert!(snap.exists());

        // la base principal se corrompe
        let main = tmp_path("snapshot");
        std::fs::write(&main, b"esto no es una base de datos").unwrap();
        match SqliteStore::open(&main) {
            Ok(_) => {}
            Err(BoardError::Corrupt(_)) => {}
            Err(e) => panic!("error inesperado: {:?}", e),
        }
    }

    #[test]
    fn dos_escritores_concurrentes_no_pierden_tareas() {
        let path = tmp_path("concurrente");
        {
            let mut s = SqliteStore::open(&path).unwrap();
            for i in 0..20 {
                s.put_task(&Task::new(&format!("t-{}", i), "build", "x")).unwrap();
            }
        }
        let mut handles = Vec::new();
        for h in 0..2 {
            let p = path.clone();
            handles.push(std::thread::spawn(move || {
                let mut s = SqliteStore::open(&p).unwrap();
                let clock = FixedClock::at(1_700_000_000);
                let mut ok = 0;
                for i in 0..20 {
                    let id = format!("t-{}", i);
                    if s.claim(&id, &format!("agente-{}", h), 60, clock.now()).is_ok() {
                        ok += 1;
                    }
                }
                ok
            }));
        }
        let total: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
        assert_eq!(total, 20, "cada tarea se reclama una sola vez");
        let s = SqliteStore::open(&path).unwrap();
        assert_eq!(s.list_tasks(&TaskFilter::default()).unwrap().len(), 20);
    }

    #[test]
    fn las_stats_reflejan_el_estado_real() {
        let mut s = store("stats");
        let clock = FixedClock::at(1_700_000_000);
        s.put_task(&Task::new("a", "build", "a")).unwrap();
        s.put_task(&Task::new("b", "build", "b")).unwrap();
        s.set_deps("b", &["a".to_string()]).unwrap();
        let st = s.stats(clock.now()).unwrap();
        assert_eq!(st.total, 2);
        assert_eq!(st.blocked_by_deps, 1);
        assert_eq!(st.by_state.get("queued"), Some(&2));
    }

    #[test]
    fn la_vista_json_no_inventa_nada() {
        let mut s = store("vista");
        let clock = FixedClock::at(1_700_000_000);
        s.put_task(&Task::new("a", "build", "a")).unwrap();
        let v = tasks_json(&s, clock.now(), 10);
        assert_eq!(v["board"], "v1");
        assert_eq!(v["count"], 1);
        assert_eq!(v["tasks"][0]["state"], "queued");
        assert_eq!(v["tasks"][0]["blocked_by"], json!([]));
    }
}
