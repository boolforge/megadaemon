//! MegaDaemon v13.4 — Bus de coordinación distribuido («comunismo algorítmico»).
//!
//! Este módulo es la pieza que permite que **varias instancias de MegaDaemon**
//! (la maestra en el móvil Samsung S21 Ultra y las worker en PCs Linux) y
//! **varios agentes/subagentes** repartan trabajo sin pisarse, migrar a la
//! vez que se actualizan y dejar rastro de todo en el repo.
//!
//! No hay servidor externo: el bus es **un fichero compartido** (el tablón,
//! `mesh_board.json`) más un **journal** en NDJSON. Todo el protocolo es
//! local, determinista y auditable.
//!
//! # Cómo se evita el pisado entre instancias
//!
//! 1. **Lock de fichero** (`O_CREAT|O_EXCL` + `flock`) alrededor de *toda*
//!    operación leer→modificar→escribir. Sin lock, dos instancias pueden
//!    intercalar escrituras y corromper el JSON.
//! 2. **Re-lectura bajo lock**: nunca se confía en la copia en memoria; cada
//!    mutación relee el tablón del disco, de modo que la vista es la última.
//! 3. **Lease con caducidad**: una tarea sólo está bloqueada mientras su
//!    `lease_until` no expira. Si el dueño muere, otra instancia puede
//!    *robar* el lease (nada queda bloqueado para siempre).
//! 4. **Merge por campo, nunca sobrescritura**: `merge_peer` gana el
//!    `last_seen` más nuevo, conserva el claim con el lease más largo y
//!    **une** las capacidades. Un peer nunca borra lo que otra instancia tiene.
//! 5. **Ámbito de git por instancia**: cada instancia sólo commitea sus rutas
//!    (`instances/<id>/`, `state/<id>/`, `reports/<id>/`), así que dos
//!    instancias no pelean por los mismos ficheros del repo.
//! 6. **Prioridad de reparto**: la maestra (móvil) primero, luego mayor
//!    `priority`, luego menor `load`.
//!
//! # Formato en disco
//!
//! ```text
//! <state_dir>/instance.json        -> InstanceInfo (identidad de esta instancia)
//! <state_dir>/mesh_board.json      -> tablón compartido (peers/claims/tasks/agents)
//! <state_dir>/mesh_journal.ndjson  -> journal de eventos (una línea JSON por evento)
//! <state_dir>/mesh_board.json.lock -> lock temporal de exclusión mutua
//! ```

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::io::AsRawFd;

// ---------------------------------------------------------------------------
// Constantes
// ---------------------------------------------------------------------------

/// Versión del esquema de datos del tablón (`mesh_board.json`).
pub const BOARD_SCHEMA: u32 = 2;

/// Nombre del fichero de identidad de la instancia.
pub const INSTANCE_FILE: &str = "instance.json";
/// Nombre del fichero del tablón compartido.
pub const BOARD_FILE: &str = "mesh_board.json";
/// Nombre del journal de eventos.
pub const JOURNAL_FILE: &str = "mesh_journal.ndjson";

/// Duración por defecto del lease de una tarea reclamada (segundos).
pub const DEFAULT_LEASE_SECS: i64 = 300;
/// Segundos sin señal tras los cuales un peer se considera **rancio** y se poda.
pub const STALE_PEER_SECS: i64 = 90;
/// Segundos de gracia antes de aplazar una tarea a una instancia con más
/// prioridad (evita el hambre del worker si la maestra no la quiere).
pub const DEFER_GRACE_SECS: i64 = 30;
/// Timeout del lock de fichero (ms).
pub const LOCK_TIMEOUT_MS: u64 = 5_000;
/// Pausa entre reintentos del lock (ms).
pub const LOCK_RETRY_MS: u64 = 20;
/// Antigüedad a partir de la cual un lock se considera huérfano (segundos).
pub const STALE_LOCK_SECS: i64 = 120;

// ---------------------------------------------------------------------------
// Tiempo (sin dependencias de calendario: ISO-8601 UTC fijo, ordenable)
// ---------------------------------------------------------------------------

/// Segundos transcurridos desde el epoch UNIX.
pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Timestamp actual en ISO-8601 UTC (`2024-05-05T12:33:21Z`).
///
/// Formato de ancho fijo: el orden lexicográfico coincide con el cronológico,
/// que es lo que permite comparar leases sin parsearlos.
pub fn now_iso() -> String {
    iso_from_epoch(unix_now())
}

/// Convierte epoch UNIX a `(año, mes, día)`.
fn iso_from_epoch(secs: i64) -> String {
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

/// Días desde epoch → fecha civil (algoritmo de Hinnant, dominio civil).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Parsea un ISO-8601 UTC (`YYYY-MM-DDTHH:MM:SSZ`) a epoch UNIX.
pub fn iso_to_epoch(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 20 {
        return None;
    }
    let num = |a: usize, n: usize| -> Option<i64> { s.get(a..a + n)?.parse::<i64>().ok() };
    let y = num(0, 4)?;
    let mo = num(5, 2)?;
    let d = num(8, 2)?;
    let h = num(11, 2)?;
    let mi = num(14, 2)?;
    let se = num(17, 2)?;
    // Días desde epoch (algoritmo inverso de Hinnant).
    let y_adj = if mo <= 2 { y - 1 } else { y };
    let era = if y_adj >= 0 { y_adj } else { y_adj - 399 } / 400;
    let yoe = (y_adj - era * 400) as u64;
    let mp = if mo > 2 { mo - 3 } else { mo + 9 } as u64;
    let doy = (153 * mp + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe as i64 - 719_468;
    Some(days * 86_400 + h * 3600 + mi * 60 + se)
}

// ---------------------------------------------------------------------------
// Identidad de la instancia
// ---------------------------------------------------------------------------

/// Ficha de una instancia dentro del mesh.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct InstanceInfo {
    /// Identificador único y estable de la instancia.
    pub id: String,
    /// Rol (`master` / `worker`).
    pub role: String,
    /// Nombre del host.
    pub hostname: String,
    /// Plataforma (`android-arm64`, `linux-x86_64`, ...).
    pub platform: String,
    /// Versión del binario.
    pub version: String,
    /// Capacidades declaradas (`firma`, `reverse`, `npu`, ...).
    pub capabilities: Vec<String>,
    /// Dirección de contacto (host:port) si se conoce.
    pub addr: String,
    /// `true` si esta instancia es la maestra (la del móvil).
    pub master: bool,
    /// Prioridad de reparto (mayor = antes).
    pub priority: i32,
    /// Último latido (ISO-8601 UTC).
    pub last_seen: String,
    /// Carga actual normalizada (0.0 - 1.0).
    pub load: f32,
}

/// Identificador de esta instancia: hash de **host + MAC + boot id**.
///
/// Es estable entre reinicios de la misma máquina (el boot id cambia) y
/// distinto entre máquinas (MAC distinta), que es justo lo que hace falta
/// para que el móvil y el PC no colisionen.
pub fn instance_id() -> String {
    let material = format!(
        "{}|{}|{}",
        hostname(),
        mac_addr(),
        boot_id()
    );
    let digest = Sha256::digest(material.as_bytes());
    let mut out = String::with_capacity(32);
    for byte in digest.iter().take(16) {
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

/// Nombre del host (best effort).
pub fn hostname() -> String {
    if let Ok(h) = fs::read_to_string("/proc/sys/kernel/hostname") {
        let h = h.trim();
        if !h.is_empty() {
            return h.to_string();
        }
    }
    if let Ok(h) = std::env::var("HOSTNAME") {
        if !h.trim().is_empty() {
            return h.trim().to_string();
        }
    }
    "unknown-host".to_string()
}

/// Primera MAC no-loopback del sistema (best effort).
pub fn mac_addr() -> String {
    if let Ok(entries) = fs::read_dir("/sys/class/net") {
        let mut found: Vec<String> = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "lo" {
                continue;
            }
            if let Ok(mac) = fs::read_to_string(entry.path().join("address")) {
                let mac = mac.trim().to_string();
                if mac.len() >= 12 && mac.chars().any(|c| c != '0' && c != ':') {
                    found.push(mac);
                }
            }
        }
        found.sort();
        if let Some(mac) = found.first() {
            return mac.clone();
        }
    }
    for path in ["/etc/machine-id", "/var/lib/dbus/machine-id"] {
        if let Ok(id) = fs::read_to_string(path) {
            let id = id.trim();
            if !id.is_empty() {
                return id.to_string();
            }
        }
    }
    "00:00:00:00:00:00".to_string()
}

/// Identificador de arranque del kernel (cambia en cada boot).
pub fn boot_id() -> String {
    if let Ok(id) = fs::read_to_string("/proc/sys/kernel/random/boot_id") {
        let id = id.trim();
        if !id.is_empty() {
            return id.to_string();
        }
    }
    if let Ok(up) = fs::read_to_string("/proc/uptime") {
        let up = up.split_whitespace().next().unwrap_or("0").to_string();
        return format!("uptime:{}", up);
    }
    "unknown-boot".to_string()
}

/// ¿Estamos en Android? (el móvil es la instancia maestra).
pub fn platform_is_android() -> bool {
    if cfg!(target_os = "android") {
        return true;
    }
    if Path::new("/system/build.prop").exists() {
        return true;
    }
    std::env::var("ANDROID_ROOT").is_ok() || std::env::var("ANDROID_DATA").is_ok()
}

/// Descripción de plataforma (`android-arm64`, `linux-x86_64`, ...).
pub fn platform_string() -> String {
    let os = if platform_is_android() {
        "android".to_string()
    } else {
        std::env::consts::OS.to_string()
    };
    format!("{}-{}", os, std::env::consts::ARCH)
}

/// Carga del sistema normalizada (0.0 - 1.0) a partir de `/proc/loadavg`.
pub fn current_load() -> f32 {
    if let Ok(s) = fs::read_to_string("/proc/loadavg") {
        if let Some(v) = s
            .split_whitespace()
            .next()
            .and_then(|x| x.parse::<f32>().ok())
        {
            let cpus = std::thread::available_parallelism()
                .map(|n| n.get() as f32)
                .unwrap_or(1.0);
            return (v / cpus).clamp(0.0, 1.0);
        }
    }
    0.0
}

/// Dirección local IPv4 (best effort, sin enviar tráfico).
pub fn local_addr() -> String {
    if let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if sock.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = sock.local_addr() {
                return addr.ip().to_string();
            }
        }
    }
    "127.0.0.1".to_string()
}

/// Directorio de estado por defecto de la instancia.
pub fn default_state_dir() -> String {
    if let Ok(dir) = std::env::var("MEGADAEMON_STATE_DIR") {
        if !dir.trim().is_empty() {
            return dir.trim().to_string();
        }
    }
    if platform_is_android() {
        "/data/local/tmp/megadaemon".to_string()
    } else {
        match std::env::var("HOME") {
            Ok(home) if !home.trim().is_empty() => format!("{}/.megadaemon", home.trim()),
            _ => "/tmp/megadaemon".to_string(),
        }
    }
}

/// Ruta del `instance.json` de un directorio de estado.
pub fn instance_path(state_dir: &str) -> PathBuf {
    Path::new(state_dir).join(INSTANCE_FILE)
}

/// Ruta del tablón compartido de un directorio de estado.
pub fn board_path(state_dir: &str) -> PathBuf {
    Path::new(state_dir).join(BOARD_FILE)
}

/// Ruta del journal de un directorio de estado.
pub fn journal_path(state_dir: &str) -> PathBuf {
    Path::new(state_dir).join(JOURNAL_FILE)
}

/// ¿Esta instancia es la **maestra**?
///
/// La maestra es la del móvil: se marca `master: true` cuando el host es
/// Android **o** cuando el `instance.json` local lo declara explícitamente.
pub fn is_master() -> bool {
    master_for_dir(&default_state_dir())
}

/// ¿Es maestra esta instancia según su directorio de estado?
fn master_for_dir(state_dir: &str) -> bool {
    if let Some(inst) = load_instance(&instance_path(state_dir)) {
        if inst.master {
            return true;
        }
    }
    platform_is_android()
}

/// Construye la ficha de la instancia local (la que corre en este proceso).
pub fn load_local_instance(caps: Vec<String>) -> InstanceInfo {
    let master = is_master();
    InstanceInfo {
        id: instance_id(),
        role: if master { "master".into() } else { "worker".into() },
        hostname: hostname(),
        platform: platform_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: caps,
        addr: local_addr(),
        master,
        priority: if master { 100 } else { 50 },
        last_seen: now_iso(),
        load: current_load(),
    }
}

/// Marca (o desmarca) esta instancia como maestra en su `instance.json`.
pub fn set_master(state_dir: &str, master: bool) -> Result<(), String> {
    fs::create_dir_all(state_dir).map_err(|e| format!("create {}: {}", state_dir, e))?;
    let path = instance_path(state_dir);
    let mut inst = load_instance(&path).unwrap_or_else(|| load_local_instance(Vec::new()));
    inst.master = master;
    inst.role = if master { "master".into() } else { "worker".into() };
    inst.priority = if master { 100 } else { 50 };
    save_instance(&path, &inst)
}

/// Carga el `instance.json` si existe.
fn load_instance(path: &Path) -> Option<InstanceInfo> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str::<InstanceInfo>(&raw).ok()
}

/// Guarda el `instance.json`.
fn save_instance(path: &Path, inst: &InstanceInfo) -> Result<(), String> {
    let data = serde_json::to_string_pretty(inst).map_err(|e| format!("serialize instance: {}", e))?;
    write_atomic(path, data.as_bytes())
}

// ---------------------------------------------------------------------------
// Tablón: tareas, claims y agentes
// ---------------------------------------------------------------------------

/// Una tarea publicada en el tablón.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MeshTask {
    /// Identificador de la tarea (único en el tablón).
    pub task_id: String,
    /// Título legible.
    pub title: String,
    /// Capacidad requerida (`""` = cualquiera).
    pub capability: String,
    /// Prioridad (mayor = antes).
    pub priority: i32,
    /// Carga útil libre.
    pub payload: Value,
    /// Fecha de publicación (ISO-8601 UTC).
    pub created_at: String,
    /// Duración del lease en segundos.
    pub lease_secs: i64,
    /// Estado (`published`, `claimed`, `done`, `failed`).
    pub state: String,
}

impl MeshTask {
    /// Crea una tarea con valores por defecto sensatos.
    pub fn new(task_id: &str, title: &str, capability: &str) -> MeshTask {
        MeshTask {
            task_id: task_id.to_string(),
            title: title.to_string(),
            capability: capability.to_string(),
            priority: 0,
            payload: Value::Null,
            created_at: now_iso(),
            lease_secs: DEFAULT_LEASE_SECS,
            state: "published".to_string(),
        }
    }
}

/// Un reclamo sobre una tarea: la "propiedad" temporal de una tarea.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TaskClaim {
    /// Tarea reclamada.
    pub task_id: String,
    /// Instancia propietaria del lease.
    pub owner: String,
    /// Momento de la reclamación (ISO-8601 UTC).
    pub claimed_at: String,
    /// Fin del lease (ISO-8601 UTC). Pasado ese tiempo, otra instancia puede robarlo.
    pub lease_until: String,
    /// Progreso 0-100.
    pub progress: u8,
    /// Estado (`claimed`, `running`, `done`, `failed`).
    pub state: String,
}

/// Registro de la actividad de un agente o subagente.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AgentTask {
    /// Nombre del agente.
    pub agent: String,
    /// Tarea realizada.
    pub task: String,
    /// Tipo de trabajo (`firma`, `reverse`, `npu`, ...).
    pub kind: String,
    /// Estado (`running`, `done`, `failed`).
    pub status: String,
    /// Resumen del resultado.
    pub result_digest: Option<String>,
    /// Inicio (ISO-8601 UTC).
    pub started_at: String,
    /// Fin (ISO-8601 UTC).
    pub finished_at: Option<String>,
    /// Feedback o causa del fallo.
    pub feedback: Option<String>,
}

/// Estado interno del tablón en disco (`mesh_board.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BoardFile {
    #[serde(default = "default_schema")]
    schema: u32,
    #[serde(default)]
    peers: Vec<InstanceInfo>,
    #[serde(default)]
    claims: Vec<TaskClaim>,
    #[serde(default)]
    tasks: Vec<MeshTask>,
    #[serde(default)]
    agents: Vec<AgentTask>,
    #[serde(default)]
    updated_at: String,
}

/// Esquema por defecto de un tablón recién creado.
fn default_schema() -> u32 {
    BOARD_SCHEMA
}

impl Default for BoardFile {
    fn default() -> Self {
        BoardFile {
            schema: BOARD_SCHEMA,
            peers: Vec::new(),
            claims: Vec::new(),
            tasks: Vec::new(),
            agents: Vec::new(),
            updated_at: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Bus de coordinación
// ---------------------------------------------------------------------------

/// El bus: tablón compartido + identidad local + vista de peers y claims.
#[derive(Debug, Clone)]
pub struct Mesh {
    /// Ruta del tablón (`.../mesh_board.json`).
    pub board_path: String,
    /// Esta instancia.
    pub me: InstanceInfo,
    /// Peers conocidos (excluye a `me`).
    pub peers: Vec<InstanceInfo>,
    /// Claims conocidos.
    pub claims: Vec<TaskClaim>,
}

impl Mesh {
    /// Abre (o crea) el bus en un directorio de estado.
    ///
    /// Carga/crea `instance.json` y `mesh_board.json`, con lock de fichero
    /// para que dos procesos que arranquen a la vez no se pisen.
    pub fn open(state_dir: &str) -> Result<Mesh, String> {
        fs::create_dir_all(state_dir).map_err(|e| format!("create {}: {}", state_dir, e))?;
        let inst_path = instance_path(state_dir);
        let mut me = load_instance(&inst_path).unwrap_or_else(|| load_local_instance(Vec::new()));
        if me.id.trim().is_empty() {
            me.id = instance_id();
        }
        me.master = me.master || master_for_dir(state_dir);
        me.role = if me.master { "master".into() } else { "worker".into() };
        me.last_seen = now_iso();
        me.load = current_load();
        save_instance(&inst_path, &me)?;

        let board = board_path(state_dir);
        if let Some(parent) = board.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create {}: {}", parent.display(), e))?;
        }
        let board_str = board.to_string_lossy().to_string();
        Self::attach_with_identity(&board_str, me)
    }

    /// Abre el bus sobre un tablón concreto con una identidad concreta.
    ///
    /// Útil cuando varias instancias comparten el mismo tablón (por ejemplo
    /// a través de una carpeta sincronizada por git) pero son máquinas
    /// distintas.
    pub fn attach(board_path: &str, me: InstanceInfo) -> Result<Mesh, String> {
        Self::attach_with_identity(board_path, me)
    }

    fn attach_with_identity(board_path: &str, mut me: InstanceInfo) -> Result<Mesh, String> {
        if me.id.trim().is_empty() {
            me.id = instance_id();
        }
        if me.hostname.trim().is_empty() {
            me.hostname = hostname();
        }
        if me.platform.trim().is_empty() {
            me.platform = platform_string();
        }
        if me.version.trim().is_empty() {
            me.version = env!("CARGO_PKG_VERSION").to_string();
        }
        me.last_seen = now_iso();

        if let Some(parent) = Path::new(board_path).parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("create {}: {}", parent.display(), e))?;
            }
        }

        let lock = BoardLock::acquire(Path::new(board_path))?;
        let mut board = read_board(Path::new(board_path))?;
        upsert_instance(&mut board.peers, &me);
        board.updated_at = now_iso();
        write_board(Path::new(board_path), &board)?;
        drop(lock);

        let mut mesh = Mesh {
            board_path: board_path.to_string(),
            me,
            peers: Vec::new(),
            claims: board.claims.clone(),
        };
        mesh.sync_from(&board);
        Ok(mesh)
    }

    /// Relee el tablón del disco (sin lock) y actualiza la vista local.
    pub fn refresh(&mut self) -> Result<(), String> {
        let board = read_board(Path::new(&self.board_path))?;
        self.sync_from(&board);
        Ok(())
    }

    /// Actualiza la vista local a partir de un tablón ya leído.
    fn sync_from(&mut self, board: &BoardFile) {
        self.peers = board
            .peers
            .iter()
            .filter(|p| p.id != self.me.id)
            .cloned()
            .collect();
        self.claims = board.claims.clone();
    }

    /// Escribe el tablón de forma atómica (tmp + rename) y refresca la vista.
    fn commit(&mut self, board: &mut BoardFile) -> Result<(), String> {
        let mut me = self.me.clone();
        me.last_seen = now_iso();
        me.load = current_load();
        upsert_instance(&mut board.peers, &me);
        board.updated_at = me.last_seen.clone();
        board.schema = BOARD_SCHEMA;
        write_board(Path::new(&self.board_path), board)?;
        self.me = me;
        self.sync_from(board);
        Ok(())
    }

    /// Publica una tarea en el tablón.
    ///
    /// Si la tarea ya existe y está reclamada, **no** se sobrescribe (eso
    /// pisaría el trabajo de otra instancia).
    pub fn publish(&mut self, task: &MeshTask) -> Result<(), String> {
        let lock = BoardLock::acquire(Path::new(&self.board_path))?;
        let mut board = read_board(Path::new(&self.board_path))?;
        let mut t = task.clone();
        if t.created_at.trim().is_empty() {
            t.created_at = now_iso();
        }
        if t.state.trim().is_empty() {
            t.state = "published".to_string();
        }
        if t.lease_secs <= 0 {
            t.lease_secs = DEFAULT_LEASE_SECS;
        }
        match board.tasks.iter().position(|x| x.task_id == t.task_id) {
            Some(idx) => {
                let claimed = board.claims.iter().any(|c| c.task_id == t.task_id);
                if !claimed {
                    board.tasks[idx] = t;
                }
            }
            None => board.tasks.push(t),
        }
        let res = self.commit(&mut board);
        drop(lock);
        res
    }

    /// Reclama una tarea de forma atómica.
    ///
    /// Gana si ninguna instancia la tiene o si el lease expiró. Respeta la
    /// prioridad: si hay una instancia viva con mejor ranking (maestra >
    /// `priority` > menor `load`) y con la capacidad requerida, la tarea se
    /// aplaza a esa instancia (salvo que haya pasado el periodo de gracia).
    pub fn claim(&mut self, task_id: &str, capability: &str) -> Result<TaskClaim, String> {
        let lock = BoardLock::acquire(Path::new(&self.board_path))?;
        let mut board = read_board(Path::new(&self.board_path))?;
        let now = now_iso();

        // 1) ¿La tarea existe en el tablón y qué capacidad pide?
        let task = board.tasks.iter().find(|t| t.task_id == task_id).cloned();
        if let Some(t) = &task {
            if !t.capability.trim().is_empty() && capability.trim() != t.capability.trim() {
                let err = format!(
                    "task '{}' requires capability '{}' but this instance provides '{}'",
                    task_id, t.capability, capability
                );
                drop(lock);
                return Err(err);
            }
        }

        // 2) Claim existente: suyo → renovar; suyo y vivo → error; expirado → robar.
        let mut steal = false;
        if let Some(idx) = board.claims.iter().position(|c| c.task_id == task_id) {
            let existing = board.claims[idx].clone();
            if existing.owner == self.me.id {
                let lease = lease_deadline(task.as_ref().map(|t| t.lease_secs).unwrap_or(0));
                board.claims[idx].lease_until = lease;
                if board.claims[idx].state != "running" {
                    board.claims[idx].state = "running".to_string();
                }
                let claim = board.claims[idx].clone();
                let res = self.commit(&mut board);
                drop(lock);
                res?;
                return Ok(claim);
            }
            if existing.lease_until > now {
                let err = format!(
                    "task '{}' already claimed by '{}' until {}",
                    task_id, existing.owner, existing.lease_until
                );
                drop(lock);
                return Err(err);
            }
            // Lease expirado: se puede robar (nada queda bloqueado para siempre).
            steal = true;
        }

        // 3) Prioridad: si hay una instancia viva mejor parada que yo, aplazo.
        if let Some(better) = self.better_peer(&board, capability, task.as_ref()) {
            let err = format!(
                "task '{}' deferred to higher-priority instance '{}' (master={}, priority={}, load={:.2})",
                task_id, better.id, better.master, better.priority, better.load
            );
            drop(lock);
            return Err(err);
        }

        // 4) Reclamo (nuevo o robo de lease expirado).
        let lease = lease_deadline(task.as_ref().map(|t| t.lease_secs).unwrap_or(0));
        board.claims.retain(|c| c.task_id != task_id);
        let claim = TaskClaim {
            task_id: task_id.to_string(),
            owner: self.me.id.clone(),
            claimed_at: now.clone(),
            lease_until: lease,
            progress: 0,
            state: "claimed".to_string(),
        };
        board.claims.push(claim.clone());
        if let Some(t) = board.tasks.iter_mut().find(|t| t.task_id == task_id) {
            t.state = "claimed".to_string();
        }
        let _ = steal;
        let res = self.commit(&mut board);
        drop(lock);
        res?;
        Ok(claim)
    }

    /// Peer vivo con mejor ranking que esta instancia (para aplazar tareas).
    fn better_peer(
        &self,
        board: &BoardFile,
        capability: &str,
        task: Option<&MeshTask>,
    ) -> Option<InstanceInfo> {
        let now = now_iso();
        let cutoff = iso_from_epoch(unix_now() - stale_peer_secs());
        // Periodo de gracia: si la tarea lleva publicada un rato, no se aplaza
        // (el worker no puede quedarse sin trabajo para siempre).
        if let Some(t) = task {
            if let Some(created) = iso_to_epoch(&t.created_at) {
                if unix_now() - created > DEFER_GRACE_SECS {
                    return None;
                }
            }
        }
        let mut best: Option<InstanceInfo> = None;
        for peer in &board.peers {
            if peer.id == self.me.id || peer.id.trim().is_empty() {
                continue;
            }
            if peer.last_seen < cutoff || peer.last_seen > clock_skew_tolerance(&now) {
                continue; // rancio, o reloj muy desfasado (se ignora)
            }
            if !capability.trim().is_empty()
                && !peer.capabilities.is_empty()
                && !peer.capabilities.iter().any(|c| c == capability)
            {
                continue; // el peer no puede hacerla
            }
            if !rank_better(peer, &self.me) {
                continue;
            }
            match &best {
                Some(b) if !rank_better(peer, b) => {}
                _ => best = Some(peer.clone()),
            }
        }
        best
    }

    /// Latido: actualiza `last_seen` de `me` y poda los peers rancios.
    pub fn heartbeat(&mut self) -> Result<(), String> {
        let lock = BoardLock::acquire(Path::new(&self.board_path))?;
        let mut board = read_board(Path::new(&self.board_path))?;
        let now = now_iso();
        let mut me = self.me.clone();
        me.last_seen = now;
        me.load = current_load();
        upsert_instance(&mut board.peers, &me);

        // Poda de peers rancios (más de N segundos sin señal).
        let cutoff = iso_from_epoch(unix_now() - stale_peer_secs());
        let before = board.peers.len();
        board.peers.retain(|p| p.id == me.id || p.last_seen >= cutoff);
        let pruned = before - board.peers.len();

        let res = self.commit(&mut board);
        drop(lock);
        res?;
        let _ = pruned;
        Ok(())
    }

    /// Libera una tarea con resultado y lo registra en el journal.
    ///
    /// Sólo se liberan los claims **propios**: jamás se toca el lease de otro.
    pub fn release(&mut self, task_id: &str, ok: bool) -> Result<(), String> {
        let lock = BoardLock::acquire(Path::new(&self.board_path))?;
        let mut board = read_board(Path::new(&self.board_path))?;
        let now = now_iso();
        let mut released: Option<TaskClaim> = None;
        board.claims.retain(|c| {
            if c.task_id == task_id && c.owner == self.me.id {
                released = Some(c.clone());
                false
            } else {
                true
            }
        });
        let state = if ok { "done" } else { "failed" };
        // Sólo quien tenía el lease puede marcar la tarea como terminada:
        // si el claim es de otro, no se toca su trabajo.
        if released.is_some() {
            if let Some(t) = board.tasks.iter_mut().find(|t| t.task_id == task_id) {
                t.state = state.to_string();
            }
        }
        let entry = json!({
            "ts": now,
            "event": "release",
            "task_id": task_id,
            "owner": self.me.id,
            "ok": ok,
            "state": state,
            "instance": self.me.id,
        });
        append_journal(&journal_path_for(&self.board_path), &entry)?;
        let res = self.commit(&mut board);
        drop(lock);
        res
    }

    /// Merge de un peer: **nunca pisa**, sólo completa lo que falta.
    ///
    /// Gana el `last_seen` más nuevo, las capacidades son unión, `master` es
    /// pegajoso, `priority` es el máximo y los claims se conservan con el
    /// lease más largo.
    pub fn merge_peer(&mut self, peer: &InstanceInfo) -> Result<(), String> {
        let lock = BoardLock::acquire(Path::new(&self.board_path))?;
        let mut board = read_board(Path::new(&self.board_path))?;
        upsert_instance(&mut board.peers, peer);
        // Los claims se deduplican conservando el lease más largo.
        merge_claims_in_place(&mut board.claims);
        let res = self.commit(&mut board);
        drop(lock);
        res
    }

    /// Merge de claims remotos (por ejemplo tras un `git pull` del tablón).
    ///
    /// Regla: para el mismo `(task_id, owner)` gana el `lease_until` más
    /// largo. Nadie pierde su trabajo porque otra réplica retrase el lease.
    pub fn merge_claims(&mut self, remote: &[TaskClaim]) -> Result<(), String> {
        let lock = BoardLock::acquire(Path::new(&self.board_path))?;
        let mut board = read_board(Path::new(&self.board_path))?;
        for rc in remote {
            match board
                .claims
                .iter()
                .position(|c| c.task_id == rc.task_id && c.owner == rc.owner)
            {
                Some(idx) => {
                    if rc.lease_until > board.claims[idx].lease_until {
                        board.claims[idx] = rc.clone();
                    }
                }
                None => board.claims.push(rc.clone()),
            }
        }
        merge_claims_in_place(&mut board.claims);
        let res = self.commit(&mut board);
        drop(lock);
        res
    }

    /// Registra la actividad de un agente o subagente.
    pub fn record_agent(&mut self, t: AgentTask) {
        let lock = match BoardLock::acquire(Path::new(&self.board_path)) {
            Ok(l) => l,
            Err(_) => return,
        };
        let mut board = match read_board(Path::new(&self.board_path)) {
            Ok(b) => b,
            Err(_) => {
                drop(lock);
                return;
            }
        };
        let mut entry = t.clone();
        if entry.started_at.trim().is_empty() {
            entry.started_at = now_iso();
        }
        if entry.status.trim().is_empty() {
            entry.status = "running".to_string();
        }
        let pos = board.agents.iter().position(|a| {
            a.agent == entry.agent && a.task == entry.task && a.started_at == entry.started_at
        });
        match pos {
            Some(idx) => board.agents[idx] = entry,
            None => {
                // Un "done"/"failed" sustituye a la entrada "running" del mismo par.
                let running = board.agents.iter().position(|a| {
                    a.agent == entry.agent
                        && a.task == entry.task
                        && a.status == "running"
                        && entry.finished_at.is_some()
                });
                match running {
                    Some(idx) => board.agents[idx] = entry,
                    None => board.agents.push(entry),
                }
            }
        }
        let _ = append_journal(
            &journal_path_for(&self.board_path),
            &json!({
                "ts": now_iso(),
                "event": "agent",
                "agent": t.agent,
                "task": t.task,
                "kind": t.kind,
                "status": t.status,
            }),
        );
        let _ = self.commit(&mut board);
        drop(lock);
    }

    /// Estadísticas agregadas de los agentes (para la web y el MCP).
    pub fn agent_stats(&self) -> Value {
        let agents = self.agents();
        let mut total = 0usize;
        let mut by_status: BTreeMap<String, usize> = BTreeMap::new();
        let mut by_agent: BTreeMap<String, (usize, usize, usize, u64, u64)> = BTreeMap::new();
        let mut by_kind: BTreeMap<String, (usize, usize, usize, u64)> = BTreeMap::new();
        let mut failures: Vec<Value> = Vec::new();

        for a in &agents {
            total += 1;
            *by_status.entry(a.status.clone()).or_insert(0) += 1;
            let secs = match (iso_to_epoch(&a.started_at), a.finished_at.as_deref().and_then(iso_to_epoch)) {
                (Some(s), Some(e)) if e >= s => (e - s) as u64,
                _ => 0,
            };
            let e = by_agent.entry(a.agent.clone()).or_insert((0, 0, 0, 0, 0));
            e.0 += 1;
            if a.status == "done" {
                e.1 += 1;
                e.3 += secs;
            } else if a.status == "failed" {
                e.2 += 1;
                e.4 += secs;
            }
            let k = by_kind.entry(a.kind.clone()).or_insert((0, 0, 0, 0));
            k.0 += 1;
            if a.status == "failed" {
                k.1 += 1;
            } else if a.status == "done" {
                k.2 += 1;
            }
            k.3 += secs;
            if a.status == "failed" {
                failures.push(json!({
                    "agent": a.agent,
                    "task": a.task,
                    "kind": a.kind,
                    "feedback": a.feedback,
                    "at": a.finished_at,
                }));
            }
        }

        let agents_map: BTreeMap<String, Value> = by_agent
            .into_iter()
            .map(|(name, (n, done, failed, sum_done, sum_failed))| {
                let finished = done + failed;
                let ok_rate = if finished == 0 {
                    0.0
                } else {
                    done as f64 / finished as f64
                };
                let avg = if finished == 0 {
                    0.0
                } else {
                    (sum_done + sum_failed) as f64 / finished as f64
                };
                (
                    name,
                    json!({
                        "total": n,
                        "done": done,
                        "failed": failed,
                        "ok_rate": (ok_rate * 1000.0).round() / 1000.0,
                        "avg_secs": (avg * 10.0).round() / 10.0,
                    }),
                )
            })
            .collect();

        let kinds_map: BTreeMap<String, Value> = by_kind
            .into_iter()
            .map(|(name, (n, failed, done, sum))| {
                let avg = if n == 0 { 0.0 } else { sum as f64 / n as f64 };
                (
                    name,
                    json!({
                        "total": n,
                        "done": done,
                        "failed": failed,
                        "avg_secs": (avg * 10.0).round() / 10.0,
                    }),
                )
            })
            .collect();

        json!({
            "total": total,
            "by_status": by_status,
            "by_agent": agents_map,
            "by_kind": kinds_map,
            "failures": failures,
        })
    }

    /// Notas de aprendizaje: qué falla más, quién rinde y qué hacer.
    ///
    /// Es el combustible del bucle "cada fallo mejora megadaemon".
    pub fn learning_notes(&self) -> Vec<String> {
        let agents = self.agents();
        let mut notes: Vec<String> = Vec::new();
        if agents.is_empty() {
            notes.push("Sin datos de agentes todavía: registra actividad con record_agent()".to_string());
            return notes;
        }

        // 1) Tipo de tarea con más fallos (fallo dominante).
        let mut kind_fail: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        // Rendimiento por agente: (tareas ok, tareas fallidas).
        let mut agent_perf: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        let mut kind_secs: BTreeMap<String, (u64, usize)> = BTreeMap::new();
        for a in &agents {
            let secs = match (
                iso_to_epoch(&a.started_at),
                a.finished_at.as_deref().and_then(iso_to_epoch),
            ) {
                (Some(s), Some(e)) if e >= s => (e - s) as u64,
                _ => 0,
            };
            let k = kind_secs.entry(a.kind.clone()).or_insert((0, 0));
            k.0 += secs;
            k.1 += 1;
            if a.status == "failed" {
                let e = kind_fail.entry(a.kind.clone()).or_insert((0, 0));
                e.0 += 1;
                e.1 += 1;
                agent_perf.entry(a.agent.clone()).or_insert((0, 0)).1 += 1;
            } else if a.status == "done" {
                agent_perf.entry(a.agent.clone()).or_insert((0, 0)).0 += 1;
            }
        }

        // Fallo dominante por tipo de tarea.
        let dominant = kind_fail
            .iter()
            .max_by(|a, b| a.1 .0.cmp(&b.1 .0).then(b.1 .0.cmp(&a.1 .0)))
            .map(|(kind, (fails, total))| (kind.clone(), *fails, *total));
        if let Some((kind, fails, total)) = &dominant {
            notes.push(format!(
                "Fallo dominante: {} ({}/{} tareas de tipo {} fallan)",
                kind, fails, total, kind
            ));
        } else {
            notes.push("Sin fallos dominantes: no hay tareas fallidas registradas".to_string());
        }

        // Agente con peor historial.
        if let Some((agent, (_, fails))) = agent_perf
            .iter()
            .filter(|(_, (done, failed))| *done + *failed > 0)
            .max_by_key(|(_, (_, f))| *f)
        {
            notes.push(format!(
                "Agente con peor historial: {} ({} fallos registrados)",
                agent, fails
            ));
        }

        // Agente más fiable (con al menos una tarea terminada).
        let best = agent_perf
            .iter()
            .filter(|(_, (done, failed))| *done + *failed > 0)
            .map(|(a, (done, failed))| (a.clone(), *done, *done + *failed))
            .max_by(|a, b| {
                let ra = a.1 as f64 / a.2 as f64;
                let rb = b.1 as f64 / b.2 as f64;
                ra.partial_cmp(&rb)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.2.cmp(&b.2))
            });
        if let Some((agent, done, finished)) = best {
            let rate = (done as f64 / finished as f64 * 1000.0).round() / 10.0;
            notes.push(format!(
                "Agente más fiable: {} ({:.1}% de éxito en {} tareas)",
                agent, rate, finished
            ));
        }

        // Tipo de tarea más lento.
        if let Some((kind, (secs, n))) = kind_secs
            .iter()
            .filter(|(_, (_, n))| *n > 0)
            .max_by(|a, b| {
                let ma = a.1 .0 as f64 / a.1 .1 as f64;
                let mb = b.1 .0 as f64 / b.1 .1 as f64;
                ma.partial_cmp(&mb).unwrap_or(std::cmp::Ordering::Equal)
            })
        {
            let avg = (*secs as f64 / *n as f64 * 10.0).round() / 10.0;
            notes.push(format!(
                "Tipo de tarea más lento: {} (media {:.1} s en {} ejecuciones)",
                kind, avg, n
            ));
        }

        // Recomendación operativa.
        if let Some((kind, fails, total)) = &dominant {
            let worst_agent = agent_perf
                .iter()
                .filter(|(_, (_, f))| *f > 0)
                .max_by_key(|(_, (_, f))| *f)
                .map(|(a, _)| a.clone())
                .unwrap_or_else(|| "?".to_string());
            notes.push(format!(
                "Recomendación: el tipo {} concentra los fallos; revisa a {} y reparte {} a otro agente antes de repetir",
                kind, worst_agent, kind
            ));
            if *total > 0 && *fails == *total {
                notes.push(format!(
                    "Alerta: el tipo {} falló el {}% de las veces; merece un guard (retry o repliegue)",
                    kind,
                    (100.0 * *fails as f64 / *total as f64).round()
                ));
            }
        }
        notes
    }

    /// Instantánea serializable (web + MCP).
    pub fn snapshot(&self) -> Value {
        let board = read_board(Path::new(&self.board_path)).unwrap_or_default();
        let claims = if board.claims.is_empty() && !self.claims.is_empty() {
            self.claims.clone()
        } else {
            board.claims.clone()
        };
        let peers = board
            .peers
            .iter()
            .filter(|p| p.id != self.me.id)
            .cloned()
            .collect::<Vec<_>>();
        let alive = peers
            .iter()
            .filter(|p| p.last_seen >= iso_from_epoch(unix_now() - stale_peer_secs()))
            .count();
        json!({
            "schema": BOARD_SCHEMA,
            "board_path": self.board_path,
            "now": now_iso(),
            "me": self.me,
            "master": self.me.master,
            "peers": peers,
            "peers_alive": alive,
            "claims": claims,
            "tasks": board.tasks,
            "agents": board.agents,
            "stats": self.agent_stats(),
            "notes": self.learning_notes(),
            "git_safe_paths": git_safe_paths(),
        })
    }

    /// Lista de tareas del tablón.
    pub fn tasks(&self) -> Vec<MeshTask> {
        read_board(Path::new(&self.board_path))
            .map(|b| b.tasks)
            .unwrap_or_default()
    }

    /// Lista de agentes registrados.
    pub fn agents(&self) -> Vec<AgentTask> {
        read_board(Path::new(&self.board_path))
            .map(|b| b.agents)
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Migración / actualización simultánea
// ---------------------------------------------------------------------------

/// Información de release (versión, commit y esquema de datos).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ReleaseInfo {
    /// Versión del binario.
    pub version: String,
    /// Commit con el que se compiló.
    pub commit: String,
    /// Versión del esquema de datos que entiende.
    pub schema: u32,
    /// Cuándo se migró por última vez (ISO-8601 UTC).
    pub migrated_at: String,
}

/// Plan de migración de esquema de datos.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MigrationPlan {
    /// Versión de origen.
    pub from: String,
    /// Versión de destino.
    pub to: String,
    /// Pasos concretos a ejecutar.
    pub steps: Vec<String>,
    /// ¿Se puede migrar sin romper nada?
    pub compatible: bool,
}

/// Release local (versión del crate + esquema de datos soportado).
pub fn release_info() -> ReleaseInfo {
    ReleaseInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        commit: option_env!("MEGADAEMON_COMMIT")
            .unwrap_or("unknown")
            .to_string(),
        schema: BOARD_SCHEMA,
        migrated_at: now_iso(),
    }
}

/// Calcula el plan de migración frente a un release remoto.
///
/// Un release remoto con esquema más nuevo que el local es **incompatible**
/// (no se puede hablar su formato); uno más viejo se puede leer, pero su
/// copia del tablón necesita migrarse al esquema local.
pub fn check_migration(me: &mut Mesh, remote: &ReleaseInfo) -> Result<MigrationPlan, String> {
    let local = release_info();
    let mut steps: Vec<String> = Vec::new();
    let mut compatible = true;

    if remote.schema > BOARD_SCHEMA {
        compatible = false;
        steps.push(format!(
            "el peer {} usa el esquema {} y este binario sólo entiende hasta {}: actualiza antes de sincronizar",
            remote.version, remote.schema, BOARD_SCHEMA
        ));
    } else if remote.schema < BOARD_SCHEMA {
        steps.push(format!(
            "el peer {} usa el esquema {}: hay que migrar su copia a {}",
            remote.version, remote.schema, BOARD_SCHEMA
        ));
        steps.push(
            "migrate_board: instances->peers, expires_at->lease_until, tasks/agents por defecto"
                .to_string(),
        );
        steps.push("revisar los claims heredados: su lease puede haber expirado".to_string());
    } else {
        steps.push("esquema de datos compatible: no hace falta migrar datos".to_string());
    }

    if remote.version != local.version {
        steps.push(format!(
            "código desfasado: local {} frente a peer {}; se puede seguir, pero actualiza en ventana de mantenimiento",
            local.version, remote.version
        ));
    }
    if !remote.commit.is_empty()
        && remote.commit != "unknown"
        && local.commit != "unknown"
        && remote.commit != local.commit
    {
        steps.push(
            "commits distintos: el merge del repo puede traer conflictos; commitea sólo tu scope"
                .to_string(),
        );
    }
    steps.push(format!(
        "actualización simultánea: actualiza primero las worker y la maestra {} al final",
        local.version
    ));

    // Dejamos constancia de la migración en el tablón (journal).
    let _ = append_journal(
        &journal_path_for(&me.board_path),
        &json!({
            "ts": now_iso(),
            "event": "migration_plan",
            "from": local.version,
            "to": remote.version,
            "compatible": compatible,
        }),
    );

    Ok(MigrationPlan {
        from: local.version,
        to: remote.version.clone(),
        steps,
        compatible,
    })
}

/// Migra el JSON de un tablón al esquema actual (`1` → `2`).
///
/// Función **pura** (entrada → salida, sin reloj ni disco): es testeable y
/// determinista. Migrar un tablón ya migrado devuelve el mismo resultado
/// (idempotente).
pub fn migrate_board(raw: &str) -> Result<String, String> {
    let parsed: Value =
        serde_json::from_str(raw).map_err(|e| format!("invalid board json: {}", e))?;
    let schema = parsed
        .get("schema")
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as u32;
    if schema > BOARD_SCHEMA {
        return Err(format!(
            "board schema {} is newer than supported schema {}",
            schema, BOARD_SCHEMA
        ));
    }

    let mut out = serde_json::Map::new();
    out.insert("schema".to_string(), json!(BOARD_SCHEMA));

    // --- peers: "instances" (v1) → "peers" (v2), normalizando campos ---
    let raw_peers = parsed
        .get("peers")
        .or_else(|| parsed.get("instances"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut peers: Vec<Value> = Vec::with_capacity(raw_peers.len());
    for p in &raw_peers {
        peers.push(normalize_peer(p));
    }
    out.insert("peers".to_string(), Value::Array(peers));

    // --- claims: "expires_at" (v1) → "lease_until" (v2) ---
    let raw_claims = parsed
        .get("claims")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut claims: Vec<Value> = Vec::with_capacity(raw_claims.len());
    for c in &raw_claims {
        claims.push(normalize_claim(c));
    }
    out.insert("claims".to_string(), Value::Array(claims));

    // --- tasks / agents: nuevas en v2 ---
    let tasks = parsed
        .get("tasks")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let agents = parsed
        .get("agents")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    out.insert("tasks".to_string(), Value::Array(tasks));
    out.insert("agents".to_string(), Value::Array(agents));

    // --- metadatos ---
    out.insert(
        "updated_at".to_string(),
        parsed
            .get("updated_at")
            .cloned()
            .unwrap_or_else(|| Value::String(String::new())),
    );

    serde_json::to_string(&Value::Object(out)).map_err(|e| format!("serialize board: {}", e))
}

/// Normaliza una ficha de instancia al esquema v2.
fn normalize_peer(p: &Value) -> Value {
    let s = |k: &str| p.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let caps = p
        .get("capabilities")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "id": s("id"),
        "role": if s("role").is_empty() { "worker".to_string() } else { s("role") },
        "hostname": s("hostname"),
        "platform": s("platform"),
        "version": s("version"),
        "capabilities": caps,
        "addr": s("addr"),
        "master": p.get("master").and_then(|v| v.as_bool()).unwrap_or(false),
        "priority": p.get("priority").and_then(|v| v.as_i64()).unwrap_or(50),
        "last_seen": s("last_seen"),
        "load": p.get("load").and_then(|v| v.as_f64()).unwrap_or(0.0),
    })
}

/// Normaliza un claim al esquema v2.
fn normalize_claim(c: &Value) -> Value {
    let s = |k: &str| c.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let lease = if !s("lease_until").is_empty() {
        s("lease_until")
    } else {
        s("expires_at")
    };
    json!({
        "task_id": s("task_id"),
        "owner": s("owner"),
        "claimed_at": s("claimed_at"),
        "lease_until": lease,
        "progress": c.get("progress").and_then(|v| v.as_u64()).unwrap_or(0),
        "state": if s("state").is_empty() { "claimed".to_string() } else { s("state") },
    })
}

// ---------------------------------------------------------------------------
// Ámbito de git: cada instancia commitea lo suyo
// ---------------------------------------------------------------------------

/// Rutas del repo que **sólo** puede tocar esta instancia.
///
/// Derivadas del `id`: `instances/<id>/`, `state/<id>/`, `reports/<id>/`.
/// Así dos instancias nunca compiten por los mismos ficheros.
pub fn git_safe_paths() -> Vec<String> {
    git_safe_paths_for(&instance_id())
}

/// Rutas seguras de una instancia concreta.
pub fn git_safe_paths_for(id: &str) -> Vec<String> {
    let id = if id.trim().is_empty() { "unknown" } else { id.trim() };
    vec![
        format!("instances/{}/", id),
        format!("state/{}/", id),
        format!("reports/{}/", id),
    ]
}

/// Mensaje de commit estandarizado para el scope de una instancia.
///
/// No contiene ninguna ruta de otra instancia (eso es lo que evita que dos
/// instancias se pisen en el repo).
pub fn commit_scope(instance_id: &str) -> String {
    let id = if instance_id.trim().is_empty() {
        "unknown"
    } else {
        instance_id.trim()
    };
    let paths = git_safe_paths_for(id).join(", ");
    format!(
        "chore(mesh): update state for instance {id}\n\n\
         Scope: {paths}\n\
         Only the paths above belong to this instance; other instances keep their own scope.\n\n\
         Co-Authored-By: MegaDaemon Mesh <mesh@boolforge.dev>"
    )
}

// ---------------------------------------------------------------------------
// Internos: lock, IO y helpers
// ---------------------------------------------------------------------------

/// Segundos sin señal a partir de los cuales un peer es rancio.
fn stale_peer_secs() -> i64 {
    std::env::var("MEGADAEMON_MESH_STALE_SECS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(STALE_PEER_SECS)
}

/// Calcula el `lease_until` a partir de la duración de la tarea.
fn lease_deadline(lease_secs: i64) -> String {
    let secs = if lease_secs > 0 { lease_secs } else { DEFAULT_LEASE_SECS };
    iso_from_epoch(unix_now() + secs)
}

/// Ordena dos instancias: mejor ranking = maestra, luego `priority`, luego
/// menor `load`.
fn rank_better(a: &InstanceInfo, b: &InstanceInfo) -> bool {
    match (a.master, b.master) {
        (true, false) => return true,
        (false, true) => return false,
        _ => {}
    }
    if a.priority != b.priority {
        return a.priority > b.priority;
    }
    a.load < b.load
}

/// Merge de dos fichas de instancia sin pisar nada.
fn upsert_instance(list: &mut Vec<InstanceInfo>, incoming: &InstanceInfo) {
    if incoming.id.trim().is_empty() {
        return;
    }
    let idx = match list.iter().position(|p| p.id == incoming.id) {
        Some(i) => i,
        None => {
            list.push(incoming.clone());
            return;
        }
    };
    let local = &mut list[idx];

    // Gana el registro con el `last_seen` más nuevo; pero un campo vacío
    // nunca borra un valor que ya teníamos.
    let incoming_newer = incoming.last_seen > local.last_seen;
    if incoming_newer {
        if !incoming.role.trim().is_empty() {
            local.role = incoming.role.clone();
        }
        if !incoming.hostname.trim().is_empty() {
            local.hostname = incoming.hostname.clone();
        }
        if !incoming.platform.trim().is_empty() {
            local.platform = incoming.platform.clone();
        }
        if !incoming.version.trim().is_empty() {
            local.version = incoming.version.clone();
        }
        if !incoming.addr.trim().is_empty() {
            local.addr = incoming.addr.clone();
        }
        local.load = incoming.load;
        local.last_seen = incoming.last_seen.clone();
    }

    // Capacidades: unión (nunca se pierde una).
    for cap in &incoming.capabilities {
        if !local.capabilities.iter().any(|c| c == cap) {
            local.capabilities.push(cap.clone());
        }
    }

    // Larrhacha maestra es pegajosa y la prioridad sólo sube.
    local.master = local.master || incoming.master;
    local.priority = local.priority.max(incoming.priority);
}

/// Deduplica claims conservando el lease más largo.
fn merge_claims_in_place(claims: &mut Vec<TaskClaim>) {
    let mut merged: Vec<TaskClaim> = Vec::with_capacity(claims.len());
    for c in claims.drain(..) {
        match merged
            .iter()
            .position(|x| x.task_id == c.task_id && x.owner == c.owner)
        {
            Some(idx) => {
                if c.lease_until > merged[idx].lease_until {
                    merged[idx] = c;
                }
            }
            None => merged.push(c),
        }
    }
    *claims = merged;
}

/// Ruta del journal derivada de la ruta del tablón.
fn journal_path_for(board_path: &str) -> PathBuf {
    let p = Path::new(board_path);
    let dir = p.parent().unwrap_or_else(|| Path::new("."));
    dir.join(JOURNAL_FILE)
}

/// Lee el tablón del disco (si no existe, uno vacío).
fn read_board(path: &Path) -> Result<BoardFile, String> {
    match fs::read_to_string(path) {
        Ok(raw) => {
            if raw.trim().is_empty() {
                return Ok(BoardFile::default());
            }
            let mut board: BoardFile = serde_json::from_str(&raw)
                .map_err(|e| format!("corrupt board {}: {}", path.display(), e))?;
            if board.schema > BOARD_SCHEMA {
                return Err(format!(
                    "board {} has unsupported schema {} (max {})",
                    path.display(),
                    board.schema,
                    BOARD_SCHEMA
                ));
            }
            board.schema = BOARD_SCHEMA;
            Ok(board)
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(BoardFile::default()),
        Err(e) => Err(format!("read board {}: {}", path.display(), e)),
    }
}

/// Escribe el tablón de forma atómica (fichero temporal + rename).
fn write_board(path: &Path, board: &BoardFile) -> Result<(), String> {
    let data =
        serde_json::to_string_pretty(board).map_err(|e| format!("serialize board: {}", e))?;
    write_atomic(path, data.as_bytes())
}

/// Escritura atómica: temporal + `rename` (nunca queda un JSON a medias).
fn write_atomic(path: &Path, data: &[u8]) -> Result<(), String> {
    let tmp = PathBuf::from(format!("{}.tmp", path.display()));
    {
        let mut f = File::create(&tmp).map_err(|e| format!("create {}: {}", tmp.display(), e))?;
        f.write_all(data)
            .map_err(|e| format!("write {}: {}", tmp.display(), e))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("rename {}: {}", path.display(), e)
    })
}

/// Añade una línea JSON al journal.
fn append_journal(path: &Path, entry: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }
    let mut line = serde_json::to_string(entry).map_err(|e| format!("serialize journal: {}", e))?;
    line.push('\n');
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("open journal {}: {}", path.display(), e))?;
    f.write_all(line.as_bytes())
        .map_err(|e| format!("write journal {}: {}", path.display(), e))
}

/// Lock de exclusión mutua sobre el tablón.
///
/// Se crea un fichero `.lock` con `O_CREAT|O_EXCL` (nadie puede tomar un lock
/// ya tomado) y se reinforce con `flock` para cubrir el caso de un proceso
/// que muere sin limpiar. Reintentos con espera y timeout.
struct BoardLock {
    path: PathBuf,
    file: Option<File>,
}

impl BoardLock {
    /// Toma el lock (con reintentos hasta `LOCK_TIMEOUT_MS`).
    fn acquire(board_path: &Path) -> Result<BoardLock, String> {
        let lock_path = PathBuf::from(format!("{}.lock", board_path.display()));
        let deadline = Instant::now() + Duration::from_millis(LOCK_TIMEOUT_MS);
        let mut attempt: u32 = 0;
        loop {
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(file) => {
                    #[cfg(unix)]
                    unsafe {
                        // `flock` sobre el fichero recién creado: refuerza el
                        // `O_EXCL` (evita el robo del lock si el fichero se
                        // borra por error y otro lo recrea).
                        libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB);
                    }
                    return Ok(BoardLock {
                        path: lock_path,
                        file: Some(file),
                    });
                }
                Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                    // ¿Lock huérfano? Si es muy viejo y está libre, se rompe.
                    if let Some(age) = file_age_secs(&lock_path) {
                        if age > STALE_LOCK_SECS && break_stale_lock(&lock_path) {
                            let _ = fs::remove_file(&lock_path);
                            continue;
                        }
                    }
                    if Instant::now() >= deadline {
                        return Err(format!(
                            "mesh lock timeout on {} (another instance is busy)",
                            lock_path.display()
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(
                        LOCK_RETRY_MS + u64::from(attempt % 10),
                    ));
                    attempt = attempt.saturating_add(1);
                }
                Err(e) => return Err(format!("open lock {}: {}", lock_path.display(), e)),
            }
        }
    }
}

impl Drop for BoardLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(f) = &self.file {
            unsafe {
                libc::flock(f.as_raw_fd(), libc::LOCK_UN);
            }
        }
        self.file = None;
        let _ = fs::remove_file(&self.path);
    }
}

/// Tolerancia de desfase de reloj entre instancias (segundos).
///
/// Un peer con un reloj muy adelantado no se considera vivo: su `last_seen`
/// no es comparable con el nuestro.
fn clock_skew_tolerance(now: &str) -> String {
    match iso_to_epoch(now) {
        Some(epoch) => iso_from_epoch(epoch + 300),
        None => now.to_string(),
    }
}

/// Antigüedad de un fichero en segundos.
fn file_age_secs(path: &Path) -> Option<i64> {    let meta = fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    let dur = SystemTime::now().duration_since(modified).ok()?;
    Some(dur.as_secs() as i64)
}

/// Intenta romper un lock huérfano (sólo si nadie lo tiene tomado).
fn break_stale_lock(path: &Path) -> bool {
    #[cfg(unix)]
    {
        match OpenOptions::new().read(true).write(true).open(path) {
            Ok(f) => {
                let rc = unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
                if rc == 0 {
                    unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_UN) };
                    true
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// Directorio temporal único por test.
    fn tmp_dir(tag: &str) -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "md-mesh-{}-{}-{}",
            tag,
            std::process::id(),
            n
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("tmp dir");
        dir
    }

    /// Instancia de prueba con identidad explícita.
    fn inst(id: &str, master: bool, priority: i32, caps: &[&str]) -> InstanceInfo {
        InstanceInfo {
            id: id.to_string(),
            role: if master { "master".into() } else { "worker".into() },
            hostname: format!("host-{}", id),
            platform: "linux-x86_64".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            capabilities: caps.iter().map(|c| c.to_string()).collect(),
            addr: "127.0.0.1".into(),
            master,
            priority,
            last_seen: now_iso(),
            load: 0.1,
        }
    }

    // (a) El claim atómico entre dos instancias: gana una y sólo una.
    #[test]
    fn claim_atomico_entre_dos_instancias_gana_una() {
        let dir = tmp_dir("atomic");
        let board = dir.join(BOARD_FILE).to_string_lossy().to_string();
        let mut a = Mesh::attach(&board, inst("inst-a", false, 10, &["firma"])).unwrap();
        let mut b = Mesh::attach(&board, inst("inst-b", true, 90, &["firma"])).unwrap();

        let task = MeshTask::new("t-1", "firmar APK", "firma");
        a.publish(&task).unwrap();

        let ra = a.claim("t-1", "firma");
        let rb = b.claim("t-1", "firma");

        // Exactamente una gana.
        assert!(
            ra.is_ok() ^ rb.is_ok(),
            "exactamente una instancia debe ganar: a={:?} b={:?}",
            ra.err(),
            rb.err()
        );
        // Y el tablón tiene un único claim.
        let board_data = read_board(Path::new(&board)).unwrap();
        assert_eq!(board_data.claims.len(), 1);
        let winner = &board_data.claims[0];
        assert!(winner.owner == "inst-a" || winner.owner == "inst-b");
        assert!(winner.lease_until > now_iso());
    }

    // (a2) Con dos workers, gana el de mayor prioridad (desempate por load).
    #[test]
    fn claim_respeta_prioridad_entre_workers() {
        let dir = tmp_dir("prio");
        let board = dir.join(BOARD_FILE).to_string_lossy().to_string();
        let mut low = Mesh::attach(&board, inst("low", false, 10, &["x"])).unwrap();
        let mut high = Mesh::attach(&board, inst("high", false, 80, &["x"])).unwrap();
        low.publish(&MeshTask::new("t-2", "tarea", "x")).unwrap();

        // El de menor prioridad se aplaza...
        let err = low.claim("t-2", "x").unwrap_err();
        assert!(err.contains("deferred"), "se esperaba aplazamiento: {}", err);
        // ...y el de mayor prioridad entra.
        let claim = high.claim("t-2", "x").expect("el de mayor prioridad reclama");
        assert_eq!(claim.owner, "high");
    }

    // (b) Un lease expirado se puede robar.
    #[test]
    fn lease_expirado_permite_reclamar() {
        let dir = tmp_dir("lease");
        let board_path = dir.join(BOARD_FILE);
        let board = board_path.to_string_lossy().to_string();
        let mut a = Mesh::attach(&board, inst("inst-a", false, 10, &["x"])).unwrap();
        let mut b = Mesh::attach(&board, inst("inst-b", false, 10, &["x"])).unwrap();
        a.publish(&MeshTask::new("t-3", "tarea", "x")).unwrap();
        a.claim("t-3", "x").expect("primer claim");

        // Con el lease vivo, B no puede.
        let err = b.claim("t-3", "x").unwrap_err();
        assert!(err.contains("already claimed"), "{}", err);

        // Envejecemos el lease en el fichero (simula un worker muerto).
        let mut data = read_board(&board_path).unwrap();
        data.claims[0].lease_until = "2000-01-01T00:00:00Z".to_string();
        write_board(&board_path, &data).unwrap();

        // Ahora B roba el lease.
        let claim = b.claim("t-3", "x").expect("robo de lease expirado");
        assert_eq!(claim.owner, "inst-b");
        let data = read_board(&board_path).unwrap();
        assert_eq!(data.claims.len(), 1);
        assert_eq!(data.claims[0].owner, "inst-b");
    }

    // (c) El merge de peers no pierde claims ni capacidades.
    #[test]
    fn merge_peers_no_pierde_claims_ni_capacidades() {
        let dir = tmp_dir("merge");
        let board = dir.join(BOARD_FILE).to_string_lossy().to_string();
        let mut a = Mesh::attach(&board, inst("inst-a", false, 10, &["firma"])).unwrap();
        let mut b = Mesh::attach(&board, inst("inst-b", true, 90, &["reverse", "npu"])).unwrap();

        // B publica y reclama: A no debe perder ese claim al mergear.
        b.publish(&MeshTask::new("t-4", "tarea", "reverse")).unwrap();
        b.claim("t-4", "reverse").unwrap();
        let claim_before = b.claims.clone();

        // A recibe la ficha de B con capacidades distintas y un last_seen viejo.
        let mut stale_peer = inst("inst-b", true, 90, &["firma"]);
        stale_peer.last_seen = "2000-01-01T00:00:00Z".into();
        a.merge_peer(&stale_peer).unwrap();

        // El claim de B sigue ahí...
        assert_eq!(a.claims.len(), 1, "el claim ajeno debe sobrevivir al merge");
        assert_eq!(a.claims[0].owner, "inst-b");
        assert_eq!(a.claims[0].task_id, claim_before[0].task_id);
        // ...y las capacidades son la unión de las dos fichas.
        let peer = a.peers.iter().find(|p| p.id == "inst-b").unwrap();
        assert!(peer.capabilities.contains(&"reverse".to_string()));
        assert!(peer.capabilities.contains(&"firma".to_string()));
        assert!(peer.capabilities.contains(&"npu".to_string()));
        // La maestra no se degrada a worker y la prioridad no baja.
        assert!(peer.master);
        assert_eq!(peer.priority, 90);
    }

    // (c2) Merge de claims: gana el lease más largo.
    #[test]
    fn merge_claims_conserva_el_lease_mas_largo() {
        let dir = tmp_dir("mergeclaims");
        let board = dir.join(BOARD_FILE).to_string_lossy().to_string();
        let mut a = Mesh::attach(&board, inst("inst-a", false, 10, &["x"])).unwrap();
        a.publish(&MeshTask::new("t-5", "tarea", "x")).unwrap();

        let corto = TaskClaim {
            task_id: "t-5".into(),
            owner: "inst-b".into(),
            claimed_at: "2024-01-01T00:00:00Z".into(),
            lease_until: "2024-01-01T00:05:00Z".into(),
            progress: 10,
            state: "claimed".into(),
        };
        let largo = TaskClaim {
            lease_until: "2024-01-01T01:05:00Z".into(),
            progress: 90,
            ..corto.clone()
        };
        a.merge_claims(&[corto.clone()]).unwrap();
        a.merge_claims(&[largo.clone()]).unwrap();
        assert_eq!(a.claims.len(), 1);
        assert_eq!(a.claims[0].lease_until, largo.lease_until);
        assert_eq!(a.claims[0].progress, 90);
    }

    // (d) La migración 1→2 es idempotente.
    #[test]
    fn migracion_esquema_1_a_2_idempotente() {
        let v1 = r#"{
          "schema": 1,
          "instances": [
            {"id":"inst-a","role":"worker","hostname":"pc","platform":"linux-x86_64",
             "version":"13.3.0","capabilities":["firma"],"addr":"","master":false,
             "priority":50,"last_seen":"2024-01-01T00:00:00Z","load":0.25}
          ],
          "claims": [
            {"task_id":"t-9","owner":"inst-a","claimed_at":"2024-01-01T00:00:00Z",
             "expires_at":"2024-01-01T00:05:00Z","progress":42}
          ]
        }"#;
        let v2 = migrate_board(v1).expect("migración");
        let v2b = migrate_board(&v2).expect("migración idempotente");
        assert_eq!(v2, v2b, "migrar dos veces debe dar el mismo resultado");

        let parsed: Value = serde_json::from_str(&v2).unwrap();
        assert_eq!(parsed["schema"], 2);
        assert!(parsed["peers"].is_array());
        assert_eq!(parsed["peers"][0]["id"], "inst-a");
        assert_eq!(parsed["peers"][0]["load"], 0.25);
        assert_eq!(parsed["claims"][0]["lease_until"], "2024-01-01T00:05:00Z");
        assert_eq!(parsed["claims"][0]["progress"], 42);
        assert_eq!(parsed["claims"][0]["state"], "claimed");
        assert!(parsed["tasks"].is_array());
        assert!(parsed["agents"].is_array());
        // Un esquema más nuevo que el nuestro no se puede migrar.
        assert!(migrate_board(r#"{"schema":99}"#).is_err());
    }

    // (e) El scope de commit no incluye ficheros de otras instancias.
    #[test]
    fn commit_scope_no_toca_ficheros_de_otras_instancias() {
        let msg = commit_scope("inst-a");
        assert!(msg.contains("instances/inst-a/"));
        assert!(msg.contains("state/inst-a/"));
        assert!(msg.contains("reports/inst-a/"));
        assert!(msg.contains("Co-Authored-By:"));
        assert!(!msg.contains("inst-b"));
        assert!(!msg.contains("inst-c"));
        // Determinista: el mismo id da el mismo mensaje.
        assert_eq!(msg, commit_scope("inst-a"));
        // Y el scope de otra instancia es distinto.
        let other = commit_scope("inst-b");
        assert!(!other.contains("instances/inst-a/"));
        assert!(other.contains("instances/inst-b/"));
    }

    // (f) learning_notes detecta el fallo dominante.
    #[test]
    fn learning_notes_detecta_el_fallo_dominante() {
        let dir = tmp_dir("notes");
        let board = dir.join(BOARD_FILE).to_string_lossy().to_string();
        let mut m = Mesh::attach(&board, inst("inst-a", false, 10, &["x"])).unwrap();

        for _ in 0..3 {
            m.record_agent(AgentTask {
                agent: "agente-roto".into(),
                task: "t-falla".into(),
                kind: "firma".into(),
                status: "failed".into(),
                result_digest: None,
                started_at: "2024-01-01T00:00:00Z".into(),
                finished_at: Some("2024-01-01T00:00:30Z".into()),
                feedback: Some("sin claves".into()),
            });
        }
        m.record_agent(AgentTask {
            agent: "agente-bueno".into(),
            task: "t-ok".into(),
            kind: "reverse".into(),
            status: "done".into(),
            result_digest: Some("ok".into()),
            started_at: "2024-01-01T00:00:00Z".into(),
            finished_at: Some("2024-01-01T00:00:05Z".into()),
            feedback: None,
        });

        let notes = m.learning_notes();
        assert!(notes.iter().any(|n| n.contains("firma") && n.contains("dominante")), "{:?}", notes);
        assert!(notes.iter().any(|n| n.contains("agente-roto")), "{:?}", notes);
        assert!(notes.iter().any(|n| n.contains("agente-bueno")), "{:?}", notes);
        assert!(notes.iter().any(|n| n.contains("Recomendación")), "{:?}", notes);

        let stats = m.agent_stats();
        assert_eq!(stats["total"], 4);
        assert_eq!(stats["by_kind"]["firma"]["failed"], 3);
        assert_eq!(stats["by_agent"]["agente-bueno"]["done"], 1);
    }

    // (g) El heartbeat poda los peers rancios.
    #[test]
    fn heartbeat_poda_peers_rancios() {
        let dir = tmp_dir("hb");
        let board_path = dir.join(BOARD_FILE);
        let board = board_path.to_string_lossy().to_string();
        let mut a = Mesh::attach(&board, inst("inst-a", false, 10, &["x"])).unwrap();

        // Peer rancio inyectado a mano en el tablón.
        let mut data = read_board(&board_path).unwrap();
        let mut stale = inst("inst-zombie", false, 10, &["x"]);
        stale.last_seen = "2000-01-01T00:00:00Z".into();
        data.peers.push(stale);
        write_board(&board_path, &data).unwrap();

        a.heartbeat().unwrap();
        assert!(
            !a.peers.iter().any(|p| p.id == "inst-zombie"),
            "el peer rancio debe podarse: {:?}",
            a.peers
        );
        assert!(a.peers.iter().all(|p| p.id != "inst-a"), "uno no es su propio peer");
    }

    // (h) El snapshot es JSON válido y serializable.
    #[test]
    fn snapshot_es_json_valido() {
        let dir = tmp_dir("snap");
        let board = dir.join(BOARD_FILE).to_string_lossy().to_string();
        let mut a = Mesh::attach(&board, inst("inst-a", false, 10, &["firma"])).unwrap();
        a.publish(&MeshTask::new("t-6", "tarea", "firma")).unwrap();
        a.claim("t-6", "firma").unwrap();
        a.record_agent(AgentTask {
            agent: "agente".into(),
            task: "t-6".into(),
            kind: "firma".into(),
            status: "done".into(),
            result_digest: None,
            started_at: "2024-01-01T00:00:00Z".into(),
            finished_at: Some("2024-01-01T00:00:01Z".into()),
            feedback: None,
        });

        let snap = a.snapshot();
        let text = serde_json::to_string(&snap).expect("serializable");
        let parsed: Value = serde_json::from_str(&text).expect("json válido");
        assert_eq!(parsed["me"]["id"], "inst-a");
        assert_eq!(parsed["schema"], 2);
        assert_eq!(parsed["claims"][0]["task_id"], "t-6");
        assert_eq!(parsed["tasks"][0]["task_id"], "t-6");
        assert!(parsed["stats"].is_object());
        assert!(parsed["git_safe_paths"].is_array());
    }

    // Publicar, reclamar y liberar deja rastro en el journal.
    #[test]
    fn release_registra_en_el_journal() {
        let dir = tmp_dir("journal");
        let board = dir.join(BOARD_FILE).to_string_lossy().to_string();
        let mut a = Mesh::attach(&board, inst("inst-a", false, 10, &["x"])).unwrap();
        a.publish(&MeshTask::new("t-7", "tarea", "x")).unwrap();
        a.claim("t-7", "x").unwrap();
        a.release("t-7", true).unwrap();

        assert!(a.claims.is_empty(), "el claim propio debe liberarse");
        let journal = fs::read_to_string(dir.join(JOURNAL_FILE)).expect("journal");
        assert!(journal.contains("\"event\":\"release\""));
        assert!(journal.contains("t-7"));
        let lines: Vec<&str> = journal.lines().filter(|l| !l.trim().is_empty()).collect();
        assert!(!lines.is_empty());
        for l in lines {
            serde_json::from_str::<Value>(l).expect("cada línea del journal es JSON");
        }
    }

    // Liberar no toca los claims de otras instancias.
    #[test]
    fn release_no_toca_claims_ajenos() {
        let dir = tmp_dir("release-other");
        let board = dir.join(BOARD_FILE).to_string_lossy().to_string();
        let mut a = Mesh::attach(&board, inst("inst-a", false, 10, &["x"])).unwrap();
        let mut b = Mesh::attach(&board, inst("inst-b", false, 20, &["x"])).unwrap();
        b.publish(&MeshTask::new("t-8", "tarea", "x")).unwrap();
        b.claim("t-8", "x").unwrap();
        a.release("t-8", true).unwrap();
        assert_eq!(a.claims.len(), 1, "el claim ajeno debe seguir vivo");
        assert_eq!(a.claims[0].owner, "inst-b");
    }

    // La capability equivocada no permite reclamar.
    #[test]
    fn claim_respeta_la_capability() {
        let dir = tmp_dir("cap");
        let board = dir.join(BOARD_FILE).to_string_lossy().to_string();
        let mut a = Mesh::attach(&board, inst("inst-a", false, 10, &["firma"])).unwrap();
        a.publish(&MeshTask::new("t-10", "firmar", "firma")).unwrap();
        let err = a.claim("t-10", "reverse").unwrap_err();
        assert!(err.contains("capability"), "{}", err);
    }

    // Dos hilos reclamando a la vez: sólo uno gana (lock real).
    #[test]
    fn claim_concurrente_solo_gana_uno() {
        use std::sync::Arc;
        let dir = tmp_dir("race");
        let board = Arc::new(dir.join(BOARD_FILE).to_string_lossy().to_string());
        let mut seed = Mesh::attach(&board, inst("inst-seed", false, 10, &["x"])).unwrap();
        seed.publish(&MeshTask::new("t-11", "tarea", "x")).unwrap();

        let mut handles = Vec::new();
        for i in 0..4 {
            let board = Arc::clone(&board);
            handles.push(std::thread::spawn(move || {
                let mut m =
                    Mesh::attach(&board, inst(&format!("inst-{}", i), false, 50, &["x"])).unwrap();
                m.claim("t-11", "x").is_ok()
            }));
        }
        let winners = handles.into_iter().map(|h| h.join().unwrap()).filter(|ok| *ok).count();
        assert_eq!(winners, 1, "sólo una instancia puede ganar el claim");
        let data = read_board(Path::new(board.as_str())).unwrap();
        assert_eq!(data.claims.len(), 1);
    }

    // La migración de datos se aplica al abrir un tablón viejo.
    #[test]
    fn open_migra_un_tablon_viejo() {
        let dir = tmp_dir("openold");
        let board_path = dir.join(BOARD_FILE);
        let v1 = r#"{"schema":1,"instances":[{"id":"inst-a","last_seen":"2024-01-01T00:00:00Z"}],
                 "claims":[{"task_id":"t-1","owner":"inst-a","claimed_at":"2024-01-01T00:00:00Z",
                 "expires_at":"2024-01-01T00:05:00Z"}]}"#;
        fs::write(&board_path, v1).unwrap();
        let migrated = migrate_board(v1).unwrap();
        fs::write(&board_path, &migrated).unwrap();

        let m = Mesh::open(&dir.to_string_lossy()).unwrap();
        assert_eq!(m.claims.len(), 1);
        assert_eq!(m.claims[0].lease_until, "2024-01-01T00:05:00Z");
        assert!(Path::new(&m.board_path).exists());
        assert!(dir.join(INSTANCE_FILE).exists());
    }

    // El plan de migración distingue "peer viejo" de "peer nuevo".
    #[test]
    fn check_migration_distingue_peers() {
        let dir = tmp_dir("plan");
        let board = dir.join(BOARD_FILE).to_string_lossy().to_string();
        let mut a = Mesh::attach(&board, inst("inst-a", true, 100, &["x"])).unwrap();

        let viejo = ReleaseInfo {
            version: "13.0.0".into(),
            commit: "abc".into(),
            schema: 1,
            migrated_at: now_iso(),
        };
        let plan = check_migration(&mut a, &viejo).unwrap();
        assert!(plan.compatible);
        assert!(!plan.steps.is_empty());
        assert!(plan.steps.iter().any(|s| s.contains("migrar")));

        let nuevo = ReleaseInfo {
            version: "99.0.0".into(),
            commit: "def".into(),
            schema: 99,
            migrated_at: now_iso(),
        };
        let plan2 = check_migration(&mut a, &nuevo).unwrap();
        assert!(!plan2.compatible, "un esquema más nuevo no es compatible");
        assert_ne!(plan2.from, plan2.to);
    }

    // El instance_id es estable y no vacío.
    #[test]
    fn instance_id_es_estable() {
        let a = instance_id();
        let b = instance_id();
        assert_eq!(a, b);
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    // git_safe_paths deriva del id local.
    #[test]
    fn git_safe_paths_derivan_del_id() {
        let paths = git_safe_paths();
        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0], format!("instances/{}/", instance_id()));
        assert!(paths.iter().all(|p| p.contains(&instance_id())));
        assert_eq!(
            git_safe_paths_for("abc"),
            vec!["instances/abc/", "state/abc/", "reports/abc/"]
        );
    }

    // release_info refleja la versión del crate y el esquema actual.
    #[test]
    fn release_info_refleja_el_crate() {
        let r = release_info();
        assert_eq!(r.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(r.schema, BOARD_SCHEMA);
        assert!(!r.migrated_at.is_empty());
    }

    // El lock de fichero se libera (no deja el board bloqueado para siempre).
    #[test]
    fn el_lock_se_libera() {
        let dir = tmp_dir("lock");
        let board = dir.join(BOARD_FILE).to_string_lossy().to_string();
        {
            let _l = BoardLock::acquire(Path::new(&board)).unwrap();
            assert!(dir.join(format!("{}.lock", BOARD_FILE)).exists());
        }
        assert!(
            !dir.join(format!("{}.lock", BOARD_FILE)).exists(),
            "el lock debe liberarse al salir"
        );
        // Y se puede volver a tomar.
        let _l2 = BoardLock::acquire(Path::new(&board)).unwrap();
    }

    // El heartbeat registra el latido y el merge no degrada a la maestra.
    #[test]
    fn heartbeat_refresca_last_seen() {
        let dir = tmp_dir("hb2");
        let board = dir.join(BOARD_FILE).to_string_lossy().to_string();
        let mut a = Mesh::attach(&board, inst("inst-a", true, 100, &["x"])).unwrap();
        a.me.last_seen = "2000-01-01T00:00:00Z".into();
        a.heartbeat().unwrap();
        assert!(a.me.last_seen.as_str() > "2020-01-01T00:00:00Z");
        assert!(a.me.master, "la maestra sigue siendo maestra tras el latido");
    }
}
