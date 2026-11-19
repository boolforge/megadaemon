//! MegaDaemon v13.4 — Tablero de coordinación de instancias y agentes.
//!
//! Este módulo es la pieza que convierte el "buzz de los chats" en un
//! protocolo verificable. La idea es simple y deliberada:
//!
//! * **Una instancia = un agente con un rol.** El móvil Samsung es la
//!   instancia **primary** (la maestra); el PC y cualquier otro nodo son
//!   **worker**. El rol vive en `<state_dir>/instance.json` y se puede
//!   promover sin reescribir nada.
//! * **El tablero es la cola real**, no los mensajes de chat. Cada tarea
//!   se reclama (lease con caducidad), se reporta con el formato de 3
//!   líneas y queda auditada en la tabla `events`.
//! * **Las migraciones se coordinan**: `migration_plan()` dice quién está
//!   desfasado y en qué orden hay que actualizar (primero los workers,
//!   el primary al final para que nunca se quede el coordinador caído).
//!
//! Todo es local (SQLite en el `state_dir`), sin red y sin dependencias
//! nuevas: `rusqlite` + `serde_json` ya están en el árbol.
//!
//! Uso típico desde el binario:
//!
//! ```no_run
//! use megadaemon_core::coordination::{TaskBoard, Instance, InstanceRole};
//!
//! let board = TaskBoard::open("/data/local/tmp/megadaemon/board.sqlite")?;
//! let inst = Instance::load_or_register("/data/local/tmp/megadaemon", "samsung-s21-ultra", "android-arm64");
//! board.register_instance(&inst)?;
//! board.add_task("firma rust sin java", 1, Some("firma"))?;
//! ```

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Versión del protocolo de coordinación (cambia si cambia el formato de
/// las tablas o la semántica del lease).
pub const PROTOCOL_VERSION: u32 = 1;

/// Versión mínima de protocolo que un primary todavía sabe hablar.
/// Si una instancia registra una versión menor, el tablero la marca como
/// incompatible en vez de ignorarla en silencio.
pub const MIN_SUPPORTED_PROTOCOL: u32 = 1;

/// Segundos por defecto del lease de una tarea reclamada.
pub const DEFAULT_LEASE_SECS: i64 = 900;

// ---------------------------------------------------------------------------
// Instancias y roles
// ---------------------------------------------------------------------------

/// Rol de una instancia dentro del sistema distribuido.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceRole {
    /// Instancia maestra: coordina, reparte y arbitra. El móvil Samsung
    /// (Exynos 2100) es el primary por decisión del usuario.
    Primary,
    /// Instancia de trabajo: ejecuta tareas asignadas.
    Worker,
    /// Instancia observadora: sólo lee el tablero (métricas, auditorías).
    Observer,
}

impl InstanceRole {
    /// Convierte un texto ("primary", "worker", "maestra", ...) en rol.
    pub fn parse(s: &str) -> Option<InstanceRole> {
        match s.trim().to_ascii_lowercase().as_str() {
            "primary" | "maestra" | "master" | "coordinator" | "coordinador" => {
                Some(InstanceRole::Primary)
            }
            "worker" | "worker_1" | "esclavo" | "nodo" | "satellite" => Some(InstanceRole::Worker),
            "observer" | "observador" | "read_only" | "readonly" => Some(InstanceRole::Observer),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            InstanceRole::Primary => "primary",
            InstanceRole::Worker => "worker",
            InstanceRole::Observer => "observer",
        }
    }
}

/// Registro de una instancia (agente) del sistema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    /// Identificador estable (normalmente `host:pid` o el id del agente).
    pub id: String,
    pub role: InstanceRole,
    /// Modelo del dispositivo ("samsung-s21-ultra", "pc-linux").
    pub device: String,
    /// Platforma ("android-arm64", "linux-x86_64").
    pub platform: String,
    /// Versión del binario desplegada en la instancia.
    pub version: String,
    /// Versión del protocolo de coordinación que habla.
    pub protocol: u32,
    /// Capacidades declaradas (ficheros que puede tocar, dominios, etc.).
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Directorio de estado (`state_dir`) de la instancia.
    #[serde(default)]
    pub state_dir: String,
    /// Epoch de la última señal de vida.
    pub last_seen: i64,
}

impl Instance {
    /// Ruta del fichero de identidad de la instancia.
    pub fn path(state_dir: &str) -> PathBuf {
        Path::new(state_dir).join("instance.json")
    }

    /// Carga la identidad de la instancia desde `<state_dir>/instance.json`.
    pub fn load(state_dir: &str) -> Option<Instance> {
        let raw = fs::read_to_string(Instance::path(state_dir)).ok()?;
        serde_json::from_str::<Instance>(&raw).ok()
    }

    /// Guarda la identidad de la instancia (escritura atómica).
    pub fn save(&self) -> Result<(), String> {
        let path = Instance::path(&self.state_dir);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("mkdir {:?}: {}", parent, e))?;
        }
        let tmp = path.with_extension("json.tmp");
        let body = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&tmp, body).map_err(|e| format!("write {:?}: {}", tmp, e))?;
        fs::rename(&tmp, &path).map_err(|e| format!("rename {:?}: {}", path, e))?;
        Ok(())
    }

    /// Carga la identidad o la crea con valores por defecto (rol worker).
    pub fn load_or_register(state_dir: &str, device: &str, platform: &str) -> Instance {
        if let Some(mut i) = Instance::load(state_dir) {
            i.last_seen = now_secs();
            return i;
        }
        let id = format!("{}@{}", device, hostname());
        Instance {
            id,
            role: InstanceRole::Worker,
            device: device.to_string(),
            platform: platform.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol: PROTOCOL_VERSION,
            capabilities: Vec::new(),
            state_dir: state_dir.to_string(),
            last_seen: now_secs(),
        }
    }

    /// Cambia el rol de la instancia y persiste el cambio.
    pub fn set_role(&mut self, role: InstanceRole) -> Result<(), String> {
        self.role = role;
        self.save()
    }

    /// Marca esta instancia como maestra (primary) y persiste el rol.
    pub fn promote_primary(&mut self) -> Result<(), String> {
        self.set_role(InstanceRole::Primary)
    }
}

// ---------------------------------------------------------------------------
// Tareas del tablero
// ---------------------------------------------------------------------------

/// Estado de una tarea en el tablero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// En la cola, esperando agente.
    Queued,
    /// Reclamada por un agente (lease activo).
    Claimed,
    /// Terminada con éxito.
    Done,
    /// Terminada con error.
    Failed,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Queued => "queued",
            TaskStatus::Claimed => "claimed",
            TaskStatus::Done => "done",
            TaskStatus::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> TaskStatus {
        match s {
            "claimed" => TaskStatus::Claimed,
            "done" => TaskStatus::Done,
            "failed" => TaskStatus::Failed,
            _ => TaskStatus::Queued,
        }
    }
}

/// Fila del tablero tal y como se ve desde fuera.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardTask {
    pub id: String,
    pub title: String,
    pub owner: Option<String>,
    pub status: TaskStatus,
    pub priority: i32,
    pub owner_hint: Option<String>,
    pub report: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub lease_expires_at: i64,
}

/// Tablero de tareas compartido (SQLite). Un único fichero por instancia
/// maestra; los workers montan el mismo fichero por red/binde o lo consultan
/// a través de la API del primary.
pub struct TaskBoard {
    conn: Connection,
    path: Option<String>,
}

impl TaskBoard {
    /// Abre (o crea) el tablero en `path`.
    pub fn open(path: &str) -> Result<TaskBoard, String> {
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent).map_err(|e| format!("mkdir {:?}: {}", parent, e))?;
        }
        let conn = Connection::open(path).map_err(|e| format!("abrir {}: {}", path, e))?;
        let board = TaskBoard {
            conn,
            path: Some(path.to_string()),
        };
        board.migrate()?;
        Ok(board)
    }

    /// Tablero en memoria (tests).
    pub fn open_in_memory() -> Result<TaskBoard, String> {
        let board = TaskBoard {
            conn: Connection::open_in_memory().map_err(|e| e.to_string())?,
            path: None,
        };
        board.migrate()?;
        Ok(board)
    }

    /// Ruta del fichero del tablero (si es persistente).
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// Crea el esquema (idempotente).
    fn migrate(&self) -> Result<(), String> {
        self.conn
            .execute_batch(
                r#"
                -- v14: el tablero es el SUBSTRATO de coordinación: si sus filas
                -- viven en un -wal que se pierde (reinicio del daemon, limpieza
                -- del state_dir, fallo de E/S), el plan desaparece.  Por eso:
                -- journal_mode=DELETE (rollback journal: cada commit queda en el
                -- fichero principal, sin ficheros -wal/-shm que perder) y
                -- synchronous=FULL (durable frente a corte de luz).
                PRAGMA journal_mode=DELETE;
                PRAGMA synchronous=FULL;
                PRAGMA foreign_keys=ON;

                CREATE TABLE IF NOT EXISTS instances (
                    id            TEXT PRIMARY KEY,
                    role          TEXT NOT NULL,
                    device        TEXT NOT NULL DEFAULT '',
                    platform      TEXT NOT NULL DEFAULT '',
                    version       TEXT NOT NULL DEFAULT '',
                    protocol      INTEGER NOT NULL DEFAULT 1,
                    capabilities  TEXT NOT NULL DEFAULT '[]',
                    state_dir     TEXT NOT NULL DEFAULT '',
                    last_seen     INTEGER NOT NULL DEFAULT 0,
                    registered_at INTEGER NOT NULL DEFAULT 0
                );

                CREATE TABLE IF NOT EXISTS tasks (
                    id              TEXT PRIMARY KEY,
                    title           TEXT NOT NULL,
                    owner           TEXT,
                    status          TEXT NOT NULL DEFAULT 'queued',
                    priority        INTEGER NOT NULL DEFAULT 5,
                    owner_hint      TEXT,
                    report          TEXT,
                    created_at      INTEGER NOT NULL DEFAULT 0,
                    updated_at      INTEGER NOT NULL DEFAULT 0,
                    lease_expires_at INTEGER NOT NULL DEFAULT 0
                );

                CREATE TABLE IF NOT EXISTS events (
                    id      INTEGER PRIMARY KEY AUTOINCREMENT,
                    ts      INTEGER NOT NULL,
                    kind    TEXT NOT NULL,
                    actor   TEXT NOT NULL,
                    payload TEXT NOT NULL DEFAULT ''
                );

                CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status, priority);
                CREATE INDEX IF NOT EXISTS idx_events_ts ON events(ts);
                "#,
            )
            .map_err(|e| format!("migrate: {}", e))?;
        Ok(())
    }

    /// Registra o actualiza una instancia en el tablero.
    pub fn register_instance(&self, inst: &Instance) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO instances (id, role, device, platform, version, protocol, capabilities, state_dir, last_seen, registered_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                     role=excluded.role, device=excluded.device, platform=excluded.platform,
                     version=excluded.version, protocol=excluded.protocol,
                     capabilities=excluded.capabilities, state_dir=excluded.state_dir,
                     last_seen=excluded.last_seen",
                params![
                    inst.id,
                    inst.role.as_str(),
                    inst.device,
                    inst.platform,
                    inst.version,
                    inst.protocol as i64,
                    serde_json::to_string(&inst.capabilities).unwrap_or_else(|_| "[]".into()),
                    inst.state_dir,
                    now_secs(),
                ],
            )
            .map_err(|e| format!("register_instance: {}", e))?;
        self.log_event("instance_registered", &inst.id, &json!({"role": inst.role.as_str()}))
    }

    /// Señal de vida de una instancia; renueva también el lease de las
    /// tareas que tiene reclamadas.
    pub fn heartbeat(&self, instance_id: &str, lease_secs: i64) -> Result<(), String> {
        let now = now_secs();
        self.conn
            .execute(
                "UPDATE instances SET last_seen=?1 WHERE id=?2",
                params![now, instance_id],
            )
            .map_err(|e| format!("heartbeat: {}", e))?;
        self.conn
            .execute(
                "UPDATE tasks SET lease_expires_at=?1, updated_at=?2 WHERE owner=?3 AND status='claimed'",
                params![now + lease_secs, now, instance_id],
            )
            .map_err(|e| format!("heartbeat lease: {}", e))?;
        self.log_event("heartbeat", instance_id, &json!({"lease": lease_secs}))
    }

    /// Encola una tarea. Devuelve su id.
    pub fn add_task(
        &self,
        title: &str,
        priority: i32,
        owner_hint: Option<&str>,
    ) -> Result<String, String> {
        if title.trim().is_empty() {
            return Err("título vacío".into());
        }
        let now = now_secs();
        let id = format!("t-{}-{}", now, short_hash(title, now));
        self.conn
            .execute(
                "INSERT INTO tasks (id, title, owner, status, priority, owner_hint, created_at, updated_at)
                 VALUES (?1, ?2, NULL, 'queued', ?3, ?4, ?5, ?5)",
                params![id, title.trim(), priority, owner_hint, now],
            )
            .map_err(|e| format!("add_task: {}", e))?;
        self.log_event("task_added", "board", &json!({"id": id, "title": title}))?;
        Ok(id)
    }

    /// Reclama una tarea de forma atómica: sólo un agente puede ganar.
    /// Devuelve `true` si este agente se quedó con ella.
    pub fn claim_task(
        &self,
        task_id: &str,
        owner: &str,
        lease_secs: i64,
    ) -> Result<bool, String> {
        let now = now_secs();
        let changed = self
            .conn
            .execute(
                "UPDATE tasks
                    SET owner=?1, status='claimed', lease_expires_at=?2, updated_at=?3
                  WHERE id=?4
                    AND (status='queued' OR (status='claimed' AND lease_expires_at < ?3))",
                params![owner, now + lease_secs, now, task_id],
            )
            .map_err(|e| format!("claim_task: {}", e))?;
        let won = changed == 1;
        self.log_event(
            if won { "task_claimed" } else { "task_claim_lost" },
            owner,
            &json!({"id": task_id}),
        )?;
        Ok(won)
    }

    /// Devuelve al estado `queued` las tareas cuyo lease ha caducado.
    pub fn reclaim_expired(&self) -> Result<usize, String> {
        let now = now_secs();
        let n = self
            .conn
            .execute(
                "UPDATE tasks SET owner=NULL, status='queued', lease_expires_at=0, updated_at=?1
                  WHERE status='claimed' AND lease_expires_at < ?1",
                params![now],
            )
            .map_err(|e| format!("reclaim_expired: {}", e))?;
        if n > 0 {
            self.log_event("tasks_reclaimed", "board", &json!({"count": n}))?;
        }
        Ok(n)
    }

    /// Reporta el resultado de una tarea (formato de 3 líneas del protocolo).
    pub fn report_task(
        &self,
        task_id: &str,
        owner: &str,
        ok: bool,
        report: &str,
    ) -> Result<(), String> {
        let now = now_secs();
        let status = if ok { "done" } else { "failed" };
        let changed = self
            .conn
            .execute(
                "UPDATE tasks SET report=?1, status=?2, updated_at=?3, lease_expires_at=0
                  WHERE id=?4 AND owner=?5",
                params![report, status, now, task_id, owner],
            )
            .map_err(|e| format!("report_task: {}", e))?;
        if changed == 0 {
            return Err(format!(
                "la tarea {} no está reclamada por {}",
                task_id, owner
            ));
        }
        self.log_event("task_reported", owner, &json!({"id": task_id, "ok": ok}))
    }

    /// Lista las tareas del tablero (opcionalmente por estado).
    pub fn list_tasks(&self, status: Option<TaskStatus>) -> Result<Vec<BoardTask>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, title, owner, status, priority, owner_hint, report, created_at, updated_at, lease_expires_at
                   FROM tasks
                  WHERE (?1 IS NULL OR status = ?1)
                  ORDER BY priority ASC, created_at ASC",
            )
            .map_err(|e| e.to_string())?;
        let filter: Option<String> = status.map(|s| s.as_str().to_string());
        let rows = stmt
            .query_map(params![filter], |row| {
                Ok(BoardTask {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    owner: row.get(2)?,
                    status: TaskStatus::parse(&row.get::<_, String>(3)?),
                    priority: row.get(4)?,
                    owner_hint: row.get(5)?,
                    report: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    lease_expires_at: row.get(9)?,
                })
            })
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    /// Instancias con señal de vida reciente.
    pub fn live_instances(&self, ttl_secs: i64) -> Result<Vec<Instance>, String> {
        let now = now_secs();
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, role, device, platform, version, protocol, capabilities, state_dir, last_seen
                   FROM instances WHERE last_seen > ?1 ORDER BY role ASC, id ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![now - ttl_secs], |row| {
                let caps: String = row.get(6)?;
                Ok(Instance {
                    id: row.get(0)?,
                    role: InstanceRole::parse(&row.get::<_, String>(1)?).unwrap_or(InstanceRole::Worker),
                    device: row.get(2)?,
                    platform: row.get(3)?,
                    version: row.get(4)?,
                    protocol: row.get::<_, i64>(5)? as u32,
                    capabilities: serde_json::from_str(&caps).unwrap_or_default(),
                    state_dir: row.get(7)?,
                    last_seen: row.get(8)?,
                })
            })
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    /// Instancias registradas (con o sin señal de vida reciente).
    pub fn all_instances(&self) -> Result<Vec<Instance>, String> {
        self.live_instances(i64::MAX)
    }

    /// Marca el primary de este tablero (el móvil Samsung es la maestra).
    /// Devuelve el id de la instancia promovida.
    pub fn set_primary(&self, instance_id: &str) -> Result<String, String> {
        let n = self
            .conn
            .execute(
                "UPDATE instances SET role='primary' WHERE id=?1",
                params![instance_id],
            )
            .map_err(|e| format!("set_primary: {}", e))?;
        if n == 0 {
            return Err(format!("instancia desconocida: {}", instance_id));
        }
        self.log_event("primary_set", instance_id, &json!({}))?;
        Ok(instance_id.to_string())
    }

    /// Plan de migración a `target_version`.
    ///
    /// Reglas:
    /// * El **primary se actualiza el último**: así el coordinador sigue
    ///   vivo mientras los workers migran.
    /// * Los observers se actualizan los primeros (noCoordinatean trabajo).
    /// * Cada instancia con protocolo incompatible se marca: hay que
    ///   migrar de una en una, nunca en paralelo.
    pub fn migration_plan(&self, target_version: &str) -> Result<Value, String> {
        let instances = self.all_instances()?;
        let mut steps = Vec::new();
        let mut incompatible = Vec::new();
        let mut order: Vec<(u8, &str, &Instance)> = Vec::new();
        for inst in &instances {
            if inst.protocol < MIN_SUPPORTED_PROTOCOL {
                incompatible.push(inst.id.clone());
            }
            let rank = match inst.role {
                InstanceRole::Observer => 0,
                InstanceRole::Worker => 1,
                InstanceRole::Primary => 2,
            };
            order.push((rank, inst.id.as_str(), inst));
        }
        order.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(b.1)));
        let mut step_no = 1;
        for (_, _, inst) in order {
            if inst.version == target_version {
                continue;
            }
            steps.push(json!({
                "step": step_no,
                "instance": inst.id,
                "role": inst.role.as_str(),
                "from_version": inst.version,
                "to_version": target_version,
                "action": "update_and_reconnect",
                "wait_for_ack": true
            }));
            step_no += 1;
        }
        Ok(json!({
            "target_version": target_version,
            "protocol": PROTOCOL_VERSION,
            "min_supported_protocol": MIN_SUPPORTED_PROTOCOL,
            "instances": instances.len(),
            "steps": steps,
            "incompatible_protocol": incompatible,
            "notes": [
                "el primary se migra el último (el coordinador no se queda caído)",
                "migración secuencial: una instancia cada vez, con acuse de recibo"
            ]
        }))
    }

    /// Instantánea completa del tablero en JSON (lo que ve el cockpit).
    pub fn snapshot(&self) -> Result<Value, String> {
        let instances = self.all_instances()?;
        let tasks = self.list_tasks(None)?;
        let now = now_secs();
        let mut by_status = serde_json::Map::new();
        for s in ["queued", "claimed", "done", "failed"] {
            let n = tasks.iter().filter(|t| t.status.as_str() == s).count();
            by_status.insert(s.to_string(), json!(n));
        }
        Ok(json!({
            "protocol": PROTOCOL_VERSION,
            "now": now,
            "instances": instances,
            "tasks": tasks,
            "tasks_by_status": Value::Object(by_status),
            "events": self.recent_events(20)?
        }))
    }

    /// Últimos eventos (auditoría del protocolo).
    pub fn recent_events(&self, limit: usize) -> Result<Vec<Value>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT ts, kind, actor, payload FROM events ORDER BY id DESC LIMIT ?1")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![limit as i64], |row| {
                let payload: String = row.get(3)?;
                Ok(json!({
                    "ts": row.get::<_, i64>(0)?,
                    "kind": row.get::<_, String>(1)?,
                    "actor": row.get::<_, String>(2)?,
                    "payload": serde_json::from_str::<Value>(&payload).unwrap_or(Value::Null)
                }))
            })
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    /// Escribe un evento en la auditoría.
    fn log_event(&self, kind: &str, actor: &str, payload: &Value) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO events (ts, kind, actor, payload) VALUES (?1, ?2, ?3, ?4)",
                params![
                    now_secs(),
                    kind,
                    actor,
                    serde_json::to_string(payload).unwrap_or_else(|_| "{}".into())
                ],
            )
            .map_err(|e| format!("log_event: {}", e))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Utilidades
// ---------------------------------------------------------------------------

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn short_hash(input: &str, salt: i64) -> u64 {
    // FNV-1a: suficiente para ids de tarea, no es criptográfico.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in input.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h ^= salt as u64;
    h
}

fn hostname() -> String {
    fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

// ---------------------------------------------------------------------------
// Sincronización entre instancias (protocolo de baja latencia)
// ---------------------------------------------------------------------------

/// Resultado de fusionar la instantánea de otra instancia.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct MergeOutcome {
    /// Tareas nuevas incorporadas.
    pub tasks_new: usize,
    /// Tareas actualizadas (la versión remota era más nueva).
    pub tasks_updated: usize,
    /// Instancias nuevas o refrescadas.
    pub instances_upserted: usize,
    /// Eventos nuevos (auditoría) incorporados.
    pub events_new: usize,
    /// Conflictos resueltos conservando el estado local (p. ej. una tarea
    /// local ya terminada que en el peer seguía en curso).
    pub conflicts: usize,
}

impl MergeOutcome {
    pub fn total(&self) -> usize {
        self.tasks_new + self.tasks_updated + self.instances_upserted + self.events_new
    }
}

impl TaskBoard {
    /// Instantánea para enviar a un peer (idéntica a `snapshot`, pero
    /// con nombre explícito para el protocolo de sync).
    pub fn export_for_sync(&self) -> Result<Value, String> {
        self.snapshot()
    }

    /// Fusiona la instantánea de otra instancia en el tablero local.
    ///
    /// Es **idempotente** (se puede repetir en cualquier orden) y
    /// **convergente** (dos instancias que se exchangean snapshots acaban
    /// con el mismo estado):
    ///
    /// * tareas: gana la más reciente por `updated_at`, pero un estado
    ///   terminal (`done`/`failed`) nunca se degrada a `queued`/`claimed`;
    /// * instancias: gana el `last_seen` mayor;
    /// * eventos: se deduplican por (ts, kind, actor, payload).
    pub fn merge_remote(&self, remote: &Value) -> Result<MergeOutcome, String> {
        let mut out = MergeOutcome::default();

        // ── instancias ────────────────────────────────────────────────────
        if let Some(list) = remote.get("instances").and_then(|v| v.as_array()) {
            for inst in list {
                let id = inst.get("id").and_then(|v| v.as_str()).unwrap_or("");
                if id.is_empty() {
                    continue;
                }
                let role = inst
                    .get("role")
                    .and_then(|v| v.as_str())
                    .and_then(InstanceRole::parse)
                    .unwrap_or(InstanceRole::Worker);
                let device = inst.get("device").and_then(|v| v.as_str()).unwrap_or("");
                let platform = inst.get("platform").and_then(|v| v.as_str()).unwrap_or("");
                let version = inst.get("version").and_then(|v| v.as_str()).unwrap_or("");
                let protocol = inst.get("protocol").and_then(|v| v.as_u64()).unwrap_or(1) as i64;
                let state_dir = inst.get("state_dir").and_then(|v| v.as_str()).unwrap_or("");
                let caps = inst
                    .get("capabilities")
                    .cloned()
                    .unwrap_or_else(|| Value::Array(vec![]));
                let caps_s = serde_json::to_string(&caps).unwrap_or_else(|_| "[]".into());
                let remote_seen = inst.get("last_seen").and_then(|v| v.as_i64()).unwrap_or(0);

                // El `last_seen` local se conserva si es mayor (la instancia
                // quevolatile ya contactó no puede "envejecer" a un peer).
                self.conn
                    .execute(
                        "INSERT INTO instances (id, role, device, platform, version, protocol, capabilities, state_dir, last_seen, registered_at)
                         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?9)
                         ON CONFLICT(id) DO UPDATE SET
                             last_seen = MAX(instances.last_seen, excluded.last_seen),
                             version  = excluded.version,
                             role     = excluded.role,
                             device   = excluded.device,
                             platform = excluded.platform,
                             protocol = excluded.protocol,
                             capabilities = excluded.capabilities",
                        params![
                            id,
                            role.as_str(),
                            device,
                            platform,
                            version,
                            protocol,
                            caps_s,
                            state_dir,
                            remote_seen
                        ],
                    )
                    .map_err(|e| format!("merge instancias: {}", e))?;
                out.instances_upserted += 1;
            }
        }

        // ── tareas ───────────────────────────────────────────────────────
        if let Some(list) = remote.get("tasks").and_then(|v| v.as_array()) {
            for t in list {
                let id = match t.get("id").and_then(|v| v.as_str()) {
                    Some(i) if !i.is_empty() => i.to_string(),
                    _ => continue,
                };
                let title = t.get("title").and_then(|v| v.as_str()).unwrap_or("(sin título)");
                let owner = t.get("owner").and_then(|v| v.as_str()).map(|s| s.to_string());
                let status = t
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("queued")
                    .to_string();
                let priority = t.get("priority").and_then(|v| v.as_i64()).unwrap_or(5) as i32;
                let owner_hint = t.get("owner_hint").and_then(|v| v.as_str()).map(|s| s.to_string());
                let report = t.get("report").and_then(|v| v.as_str()).map(|s| s.to_string());
                let created_at = t.get("created_at").and_then(|v| v.as_i64()).unwrap_or(now_secs());
                let updated_at = t.get("updated_at").and_then(|v| v.as_i64()).unwrap_or(created_at);
                let lease = t.get("lease_expires_at").and_then(|v| v.as_i64()).unwrap_or(0);

                let local: Option<(String, i64)> = self
                    .conn
                    .query_row(
                        "SELECT status, updated_at FROM tasks WHERE id=?1",
                        params![id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .ok();

                match local {
                    None => {
                        self.conn
                            .execute(
                                "INSERT INTO tasks (id, title, owner, status, priority, owner_hint, report, created_at, updated_at, lease_expires_at)
                                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                                params![
                                    id,
                                    title,
                                    owner,
                                    status,
                                    priority,
                                    owner_hint,
                                    report,
                                    created_at,
                                    updated_at,
                                    lease
                                ],
                            )
                            .map_err(|e| format!("merge tarea nueva: {}", e))?;
                        out.tasks_new += 1;
                    }
                    Some((lstatus, lupdated)) => {
                        let remote_terminal = status == "done" || status == "failed";
                        let local_terminal = lstatus == "done" || lstatus == "failed";
                        let take_remote = if remote_terminal && !local_terminal {
                            true
                        } else if local_terminal && !remote_terminal {
                            false
                        } else {
                            updated_at > lupdated
                        };
                        if take_remote {
                            self.conn
                                .execute(
                                    "UPDATE tasks SET title=?1, owner=?2, status=?3, priority=?4,
                                            owner_hint=?5, report=?6, updated_at=?7, lease_expires_at=?8
                                      WHERE id=?9",
                                    params![
                                        title,
                                        owner,
                                        status,
                                        priority,
                                        owner_hint,
                                        report,
                                        updated_at,
                                        lease,
                                        id
                                    ],
                                )
                                .map_err(|e| format!("merge tarea: {}", e))?;
                            out.tasks_updated += 1;
                        } else {
                            out.conflicts += 1;
                        }
                    }
                }
            }
        }

        // ── eventos (auditoría) ──────────────────────────────────────────
        if let Some(list) = remote.get("events").and_then(|v| v.as_array()) {
            for ev in list {
                let ts = ev.get("ts").and_then(|v| v.as_i64()).unwrap_or(0);
                let kind = ev.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                let actor = ev.get("actor").and_then(|v| v.as_str()).unwrap_or("");
                let payload = ev.get("payload").cloned().unwrap_or(Value::Null);
                let payload_s = serde_json::to_string(&payload).unwrap_or_else(|_| "null".into());
                let exists: i64 = self
                    .conn
                    .query_row(
                        "SELECT COUNT(*) FROM events WHERE ts=?1 AND kind=?2 AND actor=?3 AND payload=?4",
                        params![ts, kind, actor, payload_s],
                        |row| row.get(0),
                    )
                    .unwrap_or(0);
                if exists == 0 {
                    self.conn
                        .execute(
                            "INSERT INTO events (ts, kind, actor, payload) VALUES (?1,?2,?3,?4)",
                            params![ts, kind, actor, payload_s],
                        )
                        .map_err(|e| format!("merge evento: {}", e))?;
                    out.events_new += 1;
                }
            }
        }

        Ok(out)
    }

    /// Sincroniza con otro peer en una sola ida y vuelta: se envía la
    /// instantánea local, el peer fusiona y devuelve la suya, y se fusiona
    /// de vuelta. El resultado es convergente en ambos sentidos.
    ///
    /// `peer_base` es la base del servidor de coordenadas del peer, p. ej.
    /// `http://127.0.0.1:9099` (o `http://<ip-del-movil>:9099`).
    pub fn sync_with_peer(
        &self,
        peer_base: &str,
        token: Option<&str>,
    ) -> Result<MergeOutcome, String> {
        let base = peer_base.trim_end_matches('/');
        if base.is_empty() {
            return Err("peer_base vacío".into());
        }
        let url = format!("{}/sync", base);
        let payload = self.export_for_sync()?;

        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(5)))
            .timeout_connect(Some(Duration::from_secs(2)))
            .user_agent("megadaemon-coord/1")
            .build();
        let agent = ureq::Agent::new_with_config(config);

        let mut req = agent.post(&url);
        if let Some(t) = token {
            req = req.header("X-Seere-Auth-Token", t);
        }
        let resp = req
            .send_json(&payload)
            .map_err(|e| format!("sync {}: {}", url, e))?;
        let bytes = resp
            .into_body()
            .into_with_config()
            .limit(8 * 1024 * 1024)
            .read_to_vec()
            .map_err(|e| format!("cuerpo de sync ilegible: {}", e))?;
        let remote: Value =
            serde_json::from_slice(&bytes).map_err(|e| format!("sync no es JSON: {}", e))?;
        self.merge_remote(&remote)
    }
}


// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn inst(id: &str, role: InstanceRole) -> Instance {
        Instance {
            id: id.to_string(),
            role,
            device: "samsung-s21-ultra".into(),
            platform: "android-arm64".into(),
            version: "13.4.0".into(),
            protocol: PROTOCOL_VERSION,
            capabilities: vec!["firmar".into()],
            state_dir: "/tmp/md-test".into(),
            last_seen: now_secs(),
        }
    }

    #[test]
    fn rol_parse_acepta_alias_en_castellano() {
        assert_eq!(InstanceRole::parse("primary"), Some(InstanceRole::Primary));
        assert_eq!(InstanceRole::parse("MAESTRA"), Some(InstanceRole::Primary));
        assert_eq!(InstanceRole::parse("worker"), Some(InstanceRole::Worker));
        assert_eq!(InstanceRole::parse("nonsense"), None);
    }

    #[test]
    fn instancia_maestra_se_persiste_en_instance_json() {
        let dir = std::env::temp_dir().join(format!("md-inst-{}", std::process::id()));
        let sd = dir.to_string_lossy().to_string();
        let _ = fs::create_dir_all(&dir);
        let mut i = Instance::load_or_register(&sd, "samsung-s21-ultra", "android-arm64");
        assert_eq!(i.role, InstanceRole::Worker, "por defecto es worker");
        i.promote_primary().expect("guardar");
        let reloaded = Instance::load(&sd).expect("leer instance.json");
        assert_eq!(reloaded.role, InstanceRole::Primary);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn una_tarea_solo_se_reclama_una_vez() {
        let b = TaskBoard::open_in_memory().unwrap();
        let id = b.add_task("firma rust sin java", 1, None).unwrap();
        assert!(b.claim_task(&id, "agente-a", 60).unwrap());
        assert!(
            !b.claim_task(&id, "agente-b", 60).unwrap(),
            "el segundo agente no puede robar la tarea"
        );
    }

    #[test]
    fn el_lease_caducado_libera_la_tarea() {
        let b = TaskBoard::open_in_memory().unwrap();
        let id = b.add_task("tarea", 5, None).unwrap();
        assert!(b.claim_task(&id, "agente-a", -1).unwrap());
        let reclaimed = b.reclaim_expired().unwrap();
        assert_eq!(reclaimed, 1);
        assert!(b.claim_task(&id, "agente-b", 60).unwrap());
    }

    #[test]
    fn reportar_tarea_cambia_estado_y_guarda_informe() {
        let b = TaskBoard::open_in_memory().unwrap();
        let id = b.add_task("tarea", 5, None).unwrap();
        b.claim_task(&id, "agente-a", 60).unwrap();
        b.report_task(&id, "agente-a", true, "hecho / nada / nada")
            .unwrap();
        let tasks = b.list_tasks(None).unwrap();
        assert_eq!(tasks[0].status, TaskStatus::Done);
        assert_eq!(tasks[0].report.as_deref(), Some("hecho / nada / nada"));
        // Otro agente no puede reportar una tarea ajena.
        assert!(b.report_task(&id, "agente-b", true, "x").is_err());
    }

    #[test]
    fn el_heartbeat_renueva_el_lease_de_las_tareas() {
        let b = TaskBoard::open_in_memory().unwrap();
        b.register_instance(&inst("movil", InstanceRole::Primary)).unwrap();
        let id = b.add_task("tarea", 5, None).unwrap();
        b.claim_task(&id, "movil", 1).unwrap();
        b.heartbeat("movil", 3600).unwrap();
        let tasks = b.list_tasks(None).unwrap();
        assert!(tasks[0].lease_expires_at > now_secs() + 100);
    }

    #[test]
    fn el_plan_de_migracion_migra_el_primary_el_ultimo() {
        let b = TaskBoard::open_in_memory().unwrap();
        b.register_instance(&inst("movil", InstanceRole::Primary)).unwrap();
        b.register_instance(&inst("pc", InstanceRole::Worker)).unwrap();
        b.register_instance(&inst("visor", InstanceRole::Observer)).unwrap();
        let plan = b.migration_plan("13.5.0").unwrap();
        let steps = plan["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0]["instance"], "visor");
        assert_eq!(steps[2]["instance"], "movil", "el primary va el último");
        assert_eq!(steps[1]["instance"], "pc");
    }

    #[test]
    fn el_snapshot_refleja_el_estado_del_tablero() {
        let b = TaskBoard::open_in_memory().unwrap();
        b.register_instance(&inst("movil", InstanceRole::Primary)).unwrap();
        let id = b.add_task("tarea", 1, None).unwrap();
        b.claim_task(&id, "movil", 60).unwrap();
        let snap = b.snapshot().unwrap();
        assert_eq!(snap["tasks_by_status"]["claimed"], json!(1));
        assert_eq!(snap["instances"].as_array().unwrap().len(), 1);
        assert!(snap["events"].as_array().unwrap().len() >= 3);
    }

    #[test]
    fn fusionar_es_idempotente_y_convergente() {
        let a = TaskBoard::open_in_memory().unwrap();
        let b = TaskBoard::open_in_memory().unwrap();
        a.register_instance(&inst("pc", InstanceRole::Worker)).unwrap();
        b.register_instance(&inst("movil", InstanceRole::Primary)).unwrap();
        let id = a.add_task("tarea compartida", 1, None).unwrap();

        let snap_a = a.export_for_sync().unwrap();
        let out1 = b.merge_remote(&snap_a).unwrap();
        assert_eq!(out1.tasks_new, 1);
        assert_eq!(out1.instances_upserted, 1);

        // Segunda fusión: no duplica nada.
        let out2 = b.merge_remote(&snap_a).unwrap();
        assert_eq!(out2.tasks_new, 0);
        assert_eq!(b.list_tasks(None).unwrap().len(), 1);

        // Y la vuelta: `a` ve la tarea de `b`.
        let snap_b = b.export_for_sync().unwrap();
        a.merge_remote(&snap_b).unwrap();
        assert_eq!(a.list_tasks(None).unwrap().len(), 1);
    }

    #[test]
    fn un_estado_terminal_no_se_degrada() {
        let a = TaskBoard::open_in_memory().unwrap();
        let b = TaskBoard::open_in_memory().unwrap();
        let id = a.add_task("tarea", 1, None).unwrap();
        a.claim_task(&id, "pc", 60).unwrap();
        a.report_task(&id, "pc", true, "hecho / nada / nada").unwrap();

        // `b` todavía la tiene en curso: al fusionar, gana el terminal.
        b.add_task(&format!("tarea"), 1, None).ok();
        let mut remote = b.export_for_sync().unwrap();
        // Forzamos el caso: en `b` la tarea está en curso y con fecha vieja.
        remote["tasks"] = json!([{
            "id": id, "title": "tarea", "owner": "pc", "status": "claimed",
            "priority": 1, "created_at": 1, "updated_at": 1, "lease_expires_at": 0
        }]);
        let out = a.merge_remote(&remote).unwrap();
        assert_eq!(out.conflicts, 1);
        assert_eq!(a.list_tasks(None).unwrap()[0].status, TaskStatus::Done);
    }

    #[test]
    fn registrar_instancia_es_idempotente() {
        let b = TaskBoard::open_in_memory().unwrap();
        b.register_instance(&inst("pc", InstanceRole::Worker)).unwrap();
        let mut i = inst("pc", InstanceRole::Worker);
        i.version = "13.4.1".into();
        b.register_instance(&i).unwrap();
        let all = b.all_instances().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].version, "13.4.1");
    }
}
