//! MegaDaemon — **Patrón 7**: el daemon (socket Unix + lock de instancia +
//! handshake fail-closed + RPC de cola).
//!
//! # QUÉ COPIA ESTE MÓDULO
//!
//! El patrón "socket Unix + lock + handshake + RPC de cola" del daemon de
//! referencia (`chisel-server`), documentado en
//! `docs/PATRONES_CLI_RUST_DEFINICION.md` §"Patrón 7" y con la fuente
//! medida en `docs/RE_DEVIN_CLI_RUST.md` §5.
//!
//! La idea: **un daemon por directorio de estado**, que habla por un socket
//! Unix y sólo obedece a quien **se presenta** con la credencial del estado.
//! El handshake va **antes** de obedecer nada: un cliente sin credencial se
//! queda sin API (criterio de aceptación 2), y dos daemons sobre el mismo
//! `state_dir` no pueden convivir (criterio 1).
//!
//! # FUENTE MEDIDA (lo que sí está comprobado en la referencia)
//!
//! * `daemon.lock` y `daemon.sock` en el directorio de sesiones
//!   (`docs/RE_DEVIN_CLI_RUST.md` §5, y su §11 "Trazabilidad": las cadenas
//!   `daemon.lock`, `daemon.sock`, `list claimed sessions` y
//!   `list queued sessions` están **literalmente** en el binario medido).
//! * El daemon **verifica que quien se conecta es su propio protocolo** antes
//!   de obedecer: la cadena medida es
//!   `"the process at the daemon socket is not a devin daemon"` ⇒ handshake
//!   antes de tocar nada.
//! * Los tres nombres de RPC medidos: `list claimed sessions`,
//!   `list queued sessions`, `list outposts` (y además `create outpost` y
//!   `SSH forward supervisor`, que **NO** se implementan aquí: ver abajo).
//! * La **cola** que se consulta es la real del repo, `task_scheduler.rs`
//!   (MEDIDO): estados `Pending`, `Running`, `Paused`, `Done`, `Failed`,
//!   `Cancelled`; y por tarea `node` (a quién se asignó), `lease_expira`
//!   (lease), `resume_after` (pausa), `attempts`/`max_attempts`.
//! * El **patrón de autenticación fail-closed** ya existe en el repo
//!   (`cockpit_server.rs`): token 0600 en `<state_dir>/cockpit_token` ⇒ aquí
//!   se replica tal cual, con la MISMA fuente de credencial.
//!
//! # QUÉ **NO** ESTÁ MEDIDO (y por tanto NO se finge)
//!
//! * **El protocolo de la referencia, byte a byte.** La referencia medida no
//!   publica el formato de sus tramas (ni su framing, ni el JSON, ni el
//!   saludo). Lo que hay aquí es un protocolo **propio, documentado y
//!   consistente** con la idea medida (handshake primero, luego RPC), no una
//!   reimplementación byte a byte de `chisel-server`.
//! * **Qué es exactamente un "outpost".** §5 enumera el RPC `list outposts`
//!   y `create outpost` pero **NO define** el concepto. Por eso
//!   `list outposts` va marcado aquí como **NO MEDIDO** y se implementa con
//!   una interpretación **conservadora y razonada** (los "puestos de
//!   avanzada": los nodos que **tienen trabajo en vuelo**), declarada
//!   explícitamente en la propia respuesta (`result.definition`) para que el
//!   consumidor sepa qué está leyendo. `create outpost` **no** se implementa:
//!   no hay nada medido que crear.
//! * **El "SSH forward supervisor"** de la referencia: son túneles SSH, otra
//!   historia; aquí no hay ni servidor SSH ni supervisor de túneles.
//! * **El marco de "sesiones"** de la referencia: allí una "session" es una
//!   sesión del producto; aquí la sesión es **una tarea de la cola** (el
//!   equivalente real que tenemos medido). La equivalencia es una decisión
//!   de este repo, no una medición.
//! * **La persistencia del lock** entre reinicios: el fichero `daemon.lock`
//!   de la referencia no se ha podido inspeccionar ⇒ aquí se documenta el
//!   comportamiento exacto (ver [`InstanceLock::acquire`]).
//!
//! # PROTOCOLO (documentado, estable)
//!
//! Transporte: **socket Unix** (`std::os::unix::net`), tramas **JSON
//! línea a línea** (NDJSON: un objeto JSON por línea, `\n` como separador,
//! UTF-8). Se eligió NDJSON porque `serde_json` ya es dependencia del crate y
//! porque una línea = una trama hace el protocolo auditable con `nc -U`.
//!
//! ```text
//! C -> D   {"auth":"<credencial>"}\n
//! D -> C   {"ok":true,"event":"handshake","protocol":"megadaemon-daemon-socket/1"}\n
//! C -> D   {"command":"list queued sessions"}\n
//! D -> C   {"ok":true,"command":"list queued sessions","result":{...}}\n
//! ```
//!
//! * El **handshake es obligatorio y va primero**: sin él (o con credencial
//!   incorrecta) el daemon responde un error **genérico** y **cierra**; no
//!   se sirve ninguna RPC.
//! * Petición desconocida ⇒ error **tipado** (`error.code =
//!   "unknown_command"`), nunca un pánico.
//! * Credenciales: se comparan en **tiempo constante** y **no** se registran
//!   en ningún log.
//!
//! # SEPARACIÓN E/S ↔ DECISIÓN (para poder testear sin red)
//!
//! * **E/S** (abstraído): [`DaemonSocket::bind`], [`DaemonSocket::serve`],
//!   [`DaemonSocket::serve_once`], [`DaemonSocket::handle`],
//!   [`DaemonClient`], [`InstanceLock`].
//! * **Decisión pura** (testeable sin abrir nada): [`Comando::parse`],
//!   [`token_valido`], [`claimed_sessions`], [`queued_sessions`],
//!   [`outposts`], [`construir_respuesta`].
//!
//! Los tests usan un socket Unix en un temporal de `std::env::temp_dir()`
//! (nunca puertos de red) y `std::env::temp_dir()` con nombre único por
//! test (no se usa `tempfile`, que no es dependencia del crate).
//!
//! # CAVILLO MEDIDO EN LOS TESTS (importante para quien los lea)
//!
//! `crate::state_dir::state_dir_de_test()` aísla el estado **por hilo**
//! (MEDIDO 2026-11-25: los tests en paralelo se pisaban la cola). Por eso
//! el hilo que atiende el socket vería **otra cola, vacía**. La fuente de
//! la cola es por tanto **inyectable** ([`DaemonSocket::bind_con_cola`]) y el
//! proveedor **por defecto** es la cola real del proceso
//! ([`fuente_cola_real`] ⇒ [`crate::task_scheduler::list`]). En producción
//! no hay inyección: el daemon sirve SIEMPRE la cola real.

use crate::task_scheduler::{State, Task};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Fichero del lock de instancia (medido en la referencia: `daemon.lock`).
pub const LOCK_FILE: &str = "daemon.lock";

/// Fichero del socket Unix (medido en la referencia: `daemon.sock`).
pub const SOCKET_FILE: &str = "daemon.sock";

/// Token del cockpit: la credencial del estado si ya existe (0600).
pub const COCKPIT_TOKEN_FILE: &str = "cockpit_token";

/// Credencial propia del socket, si no hay token del cockpit (0600).
pub const DAEMON_TOKEN_FILE: &str = "daemon_token";

/// RPC medido: sesiones ya reclamadas por un nodo.
pub const CMD_CLAIMED: &str = "list claimed sessions";

/// RPC medido: sesiones en cola de espera.
pub const CMD_QUEUED: &str = "list queued sessions";

/// RPC medido: "outposts" (**concepto NO MEDIDO**, ver la cabecera).
pub const CMD_OUTPOSTS: &str = "list outposts";

/// Etiqueta del protocolo (va en la respuesta del handshake).
pub const PROTOCOLO: &str = "megadaemon-daemon-socket/1";

/// Tamaño máximo de una trama (protección contra memoria infinita).
pub const MAX_LINEA: usize = 64 * 1024;

/// Timeout de lectura de una conexión atendida.
pub const TIMEOUT_LECTURA: Duration = Duration::from_secs(30);

/// Nodo sintético para el trabajo que se ejecuta en LOCAL (sin `node`).
pub const NODO_LOCAL: &str = "local";

/// Fuente de la cola: cierre `() -> Vec<Task>` compartible entre hilos.
pub type FuenteCola = Arc<dyn Fn() -> Vec<Task> + Send + Sync>;

/// La cola REAL del proceso (`task_scheduler`).
pub fn fuente_cola_real() -> FuenteCola {
    Arc::new(crate::task_scheduler::list)
}

// ---------------------------------------------------------------------------
// ERRORES
// ---------------------------------------------------------------------------

/// Errores del daemon (tipados: el llamante puede decidir con ellos).
#[derive(Debug)]
pub enum DaemonError {
    /// El lock de instancia ya está tomado ⇒ hay otro daemon en marcha.
    LockTaken {
        /// Ruta del fichero de lock.
        path: PathBuf,
        /// Pid que lo tomó (si se pudo leer).
        pid: Option<u32>,
    },
    /// Fallo de E/S.
    Io {
        /// Ruta afectada (si la hay).
        path: Option<PathBuf>,
        /// Error original.
        source: std::io::Error,
    },
    /// Credencial inválida (o ausente) en el handshake.
    Unauthorized,
    /// Trama malformada (JSON roto, línea enorme, campo obligatorio vacío).
    Malformed(String),
    /// RPC que no existe.
    UnknownCommand(String),
    /// El peer cerró la conexión.
    Closed,
}

impl std::fmt::Display for DaemonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DaemonError::LockTaken { path, pid } => match pid {
                Some(p) => write!(
                    f,
                    "ya hay otro daemon en marcha sobre este estado: el lock {} lo tiene el pid {p}; sólo un daemon por state_dir",
                    path.display()
                ),
                None => write!(
                    f,
                    "ya hay otro daemon en marcha sobre este estado: el lock {} existe pero no se puede leer su pid; bórralo si el daemon ya murió",
                    path.display()
                ),
            },
            DaemonError::Io { path, source } => match path {
                Some(p) => write!(f, "error de E/S en {}: {source}", p.display()),
                None => write!(f, "error de E/S: {source}"),
            },
            DaemonError::Unauthorized => {
                write!(f, "handshake rechazado: credencial inválida")
            }
            DaemonError::Malformed(m) => write!(f, "petición malformada: {m}"),
            DaemonError::UnknownCommand(c) => write!(f, "comando desconocido: {c}"),
            DaemonError::Closed => write!(f, "conexión cerrada"),
        }
    }
}

impl std::error::Error for DaemonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DaemonError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl DaemonError {
    /// Error de E/S con ruta.
    fn io(path: Option<&Path>, source: std::io::Error) -> Self {
        DaemonError::Io {
            path: path.map(|p| p.to_path_buf()),
            source,
        }
    }
}

// ---------------------------------------------------------------------------
// COMANDOS (puro)
// ---------------------------------------------------------------------------

/// Los tres RPC de cola del patrón 7 (los nombres son los MEDIDOS).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comando {
    /// `list claimed sessions`: tareas ya asignadas a un nodo (`node`).
    ListClaimedSessions,
    /// `list queued sessions`: tareas pendientes de ejecutar, sin dueño.
    ListQueuedSessions,
    /// `list outposts`: puestos con trabajo en vuelo (**concepto NO MEDIDO**).
    ListOutposts,
}

impl Comando {
    /// El nombre exacto del RPC.
    pub fn as_str(&self) -> &'static str {
        match self {
            Comando::ListClaimedSessions => CMD_CLAIMED,
            Comando::ListQueuedSessions => CMD_QUEUED,
            Comando::ListOutposts => CMD_OUTPOSTS,
        }
    }

    /// Clasifica una petición (Puro). Error **tipado** si no existe.
    ///
    /// Se normalizan los espacios (se colapsan los seguidos) y se ignora el
    /// caso: `"list  claimed   sessions"` es la misma RPC que
    /// `"list claimed sessions"`.
    pub fn parse(s: &str) -> Result<Comando, DaemonError> {
        let norm = normalizar(s);
        match norm.as_str() {
            "list claimed sessions" => Ok(Comando::ListClaimedSessions),
            "list queued sessions" => Ok(Comando::ListQueuedSessions),
            "list outposts" => Ok(Comando::ListOutposts),
            _ => Err(DaemonError::UnknownCommand(s.trim().to_string())),
        }
    }
}

/// Colapsa espacios y pasa a minúsculas (para clasificar peticiones).
fn normalizar(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut espacio = false;
    for c in s.trim().chars() {
        if c.is_whitespace() {
            espacio = true;
            continue;
        }
        if espacio && !out.is_empty() {
            out.push(' ');
        }
        espacio = false;
        for l in c.to_lowercase() {
            out.push(l);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// CREDENCIAL (puro el comparador; E/S el fichero)
// ---------------------------------------------------------------------------

/// ¿La credencial presentada es la del estado? (comparación en tiempo constante)
///
/// Fail-closed: una credencial **vacía** nunca es válida (ni la esperada ni
/// la presentada) ⇒ un cliente sin credencial no puede colarse por un
/// descuido (un estado sin token materializado tampoco valida nada).
pub fn token_valido(esperado: &str, presentado: &str) -> bool {
    if esperado.is_empty() || presentado.is_empty() {
        return false;
    }
    let a = esperado.as_bytes();
    let b = presentado.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut dif = 0u8;
    for i in 0..a.len() {
        dif |= a[i] ^ b[i];
    }
    dif == 0
}

/// La credencial del estado para el socket.
///
/// Orden (MEDIDO en el repo, replicado del cockpit):
/// 1. `<state_dir>/cockpit_token` si ya existe ⇒ **esa** es la credencial
///    (mismo fichero 0600 que usa el cockpit HTTP).
/// 2. `<state_dir>/daemon_token` si ya existe ⇒ credencial propia ya
///    materializada.
/// 3. Si no hay ninguna ⇒ se **genera** una propia y se guarda **0600**.
///
/// AVISO (decisión de este repo, no medición): reutilizar el token del
/// cockpit amplía el círculo de confianza del socket (el token del cockpit
/// también lo lleva la UI embebido). Es lo que pide el patrón 7 tal y como
/// está definido en el repo; quien quiera separar los dos dominios puede
/// borrar `cockpit_token` y dejar que el socket use `daemon_token`.
pub fn credencial_del_estado(state_dir: &Path) -> Result<String, DaemonError> {
    for nombre in [COCKPIT_TOKEN_FILE, DAEMON_TOKEN_FILE] {
        let ruta = state_dir.join(nombre);
        if let Some(t) = leer_token(&ruta) {
            return Ok(t);
        }
    }
    let token = hex_aleatorio(32);
    let ruta = state_dir.join(DAEMON_TOKEN_FILE);
    fs::create_dir_all(state_dir).map_err(|e| DaemonError::io(Some(state_dir), e))?;
    fs::write(&ruta, token.as_bytes()).map_err(|e| DaemonError::io(Some(&ruta), e))?;
    set_modo_600(&ruta);
    Ok(token)
}

/// Lee un token de un fichero (mínimo 16 caracteres ⇒ se acepta).
fn leer_token(ruta: &Path) -> Option<String> {
    let txt = fs::read_to_string(ruta).ok()?;
    let t = txt.trim().to_string();
    if t.len() >= 16 {
        Some(t)
    } else {
        None
    }
}

/// Pone un fichero a 0600 (best effort: si el FS no lo soporta, se avisa).
fn set_modo_600(ruta: &Path) {
    let r = fs::set_permissions(ruta, fs::Permissions::from_mode(0o600));
    if let Err(e) = r {
        eprintln!(
            "[daemon_socket] no se pudo dejar {} en 0600: {e}",
            ruta.display()
        );
    }
}

/// Hex aleatorio de `nbytes` bytes ( `/dev/urandom`, con respaldo).
fn hex_aleatorio(nbytes: usize) -> String {
    let mut buf = vec![0u8; nbytes];
    if let Ok(mut f) = fs::File::open("/dev/urandom") {
        use std::io::Read;
        if f.read_exact(&mut buf).is_err() {
            buf.iter_mut().for_each(|b| *b = 0);
        }
    }
    // Respaldo: reloj + pid (no criptográfico; sólo si no hay urandom).
    if buf.iter().all(|b| *b == 0) {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let p = std::process::id() as u128;
        let v = t ^ (p << 64);
        buf[..16].copy_from_slice(&v.to_le_bytes());
    }
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// Epoch en segundos.
fn ahora() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// LOCK DE INSTANCIA
// ---------------------------------------------------------------------------

/// El lock de instancia: **un solo daemon por `state_dir`**.
///
/// Se implementa con un fichero (`daemon.lock`, el nombre MEDIDO) creado en
/// exclusiva (`create_new(true)`), sin dependencias externas. El fichero
/// guarda el pid del dueño para poder distinguir "daemon vivo" de "daemon
/// muerto":
///
/// * si el lock existe y su pid **sigue vivo** ⇒ `LockTaken` (el segundo
///   daemon **no arranca**, criterio de aceptación 1);
/// * si el lock existe pero su pid está **muerto** ⇒ el lock está **caducado**
///   (el daemon anterior murió) ⇒ se toma el relevo;
/// * si el lock existe pero **no se puede leer** su pid ⇒ **fail-closed**:
///   se rechaza el arranque y hay que borrar el fichero a mano.
///
/// Al soltar el guard ([`Drop`]) el fichero **se borra**: el estado queda
/// coherente (no quedan locks huérfanos por la vía normal).
#[derive(Debug)]
pub struct InstanceLock {
    path: PathBuf,
    pid: u32,
}

impl InstanceLock {
    /// Toma el lock de instancia de un `state_dir`.
    pub fn acquire(state_dir: &Path) -> Result<InstanceLock, DaemonError> {
        fs::create_dir_all(state_dir).map_err(|e| DaemonError::io(Some(state_dir), e))?;
        let path = state_dir.join(LOCK_FILE);
        match Self::crear(&path) {
            Ok(l) => Ok(l),
            Err(e) => {
                if e.kind() != std::io::ErrorKind::AlreadyExists {
                    return Err(DaemonError::io(Some(&path), e));
                }
                let pid = leer_pid(&path);
                match pid {
                    // Lock CADUCADO: el dueño está muerto => se toma el relevo.
                    Some(p) if p != std::process::id() && !proceso_vivo(p) => {
                        let _ = fs::remove_file(&path);
                        Self::crear(&path).map_err(|e2| {
                            if e2.kind() == std::io::ErrorKind::AlreadyExists {
                                DaemonError::LockTaken {
                                    path: path.clone(),
                                    pid: leer_pid(&path),
                                }
                            } else {
                                DaemonError::io(Some(&path), e2)
                            }
                        })
                    }
                    // Lock VIVO (o ilegible) => fail-closed.
                    _ => Err(DaemonError::LockTaken { path, pid }),
                }
            }
        }
    }

    /// Crea el fichero de lock en exclusiva y escribe el pid.
    fn crear(path: &Path) -> Result<InstanceLock, std::io::Error> {
        let mut f = OpenOptions::new().write(true).create_new(true).open(path)?;
        let pid = std::process::id();
        let contenido = json!({
            "pid": pid,
            "started_at": ahora(),
            "socket": SOCKET_FILE,
        })
        .to_string();
        if let Err(e) = f.write_all(contenido.as_bytes()).and_then(|_| f.sync_all()) {
            // No dejamos un lock a medias: sin pid no se puede ni recuperar.
            drop(f);
            let _ = fs::remove_file(path);
            return Err(e);
        }
        let _ = f.set_permissions(fs::Permissions::from_mode(0o600));
        Ok(InstanceLock {
            path: path.to_path_buf(),
            pid,
        })
    }

    /// Ruta del fichero de lock.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Pid del dueño del lock.
    pub fn pid(&self) -> u32 {
        self.pid
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        // El fichero se borra al soltar el guard: el estado queda coherente.
        let _ = fs::remove_file(&self.path);
    }
}

/// Lee el pid del fichero de lock (`None` si no se puede leer ⇒ fail-closed).
fn leer_pid(path: &Path) -> Option<u32> {
    let txt = fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(txt.trim()).ok()?;
    let pid = v.get("pid")?.as_u64()?;
    u32::try_from(pid).ok()
}

/// ¿El proceso sigue vivo? (`kill(pid, 0)`).
fn proceso_vivo(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    // `EPERM` = existe pero es de otro usuario ⇒ también está vivo.
    let rc = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if rc == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

// ---------------------------------------------------------------------------
// LA COLA REAL -> RESPUESTAS (puro)
// ---------------------------------------------------------------------------

/// Tarea en JSON (la cola REAL, sin inventar campos).
fn tarea_json(t: &Task) -> Value {
    json!({
        "id": t.id,
        "kind": t.kind,
        "state": t.state.as_str(),
        "priority": t.priority,
        "node": t.node,
        "attempts": t.attempts,
        "max_attempts": t.max_attempts,
        "created_at": t.created_at,
        "started_at": t.started_at,
        "finished_at": t.finished_at,
        "lease_expira": t.lease_expira,
        "resume_after": t.resume_after,
        "motivo_pausa": t.motivo_pausa,
        "error": t.error,
        "deps": t.deps,
        "payload": t.payload,
    })
}

/// Definición textual de `list claimed sessions` (va en la respuesta).
const DEF_CLAIMED: &str = "Tareas de la cola real con 'node' no vacio: ya estan asignadas a un nodo. Es la columna 'node' tal cual (MEDIDO en task_scheduler), sin filtrar por estado: una tarea terminada conserva su nodo, asi que tambien aparece aqui. Las que estan 'pending' sin nodo estan en 'list queued sessions'.";

/// Definición textual de `list queued sessions` (va en la respuesta).
const DEF_QUEUED: &str = "Tareas de la cola real en estado 'pending' y sin nodo asignado: la cola de espera, lo que todavia no ha separado nadie (nadie las ha reclamado). Las tareas 'paused' NO estan aqui (esperan a 'resume_after').";

/// Definición textual de `list outposts` (va en la respuesta).
const DEF_OUTPOSTS: &str = "NO MEDIDO en la referencia: §5 enumera el RPC 'list outposts' pero no define que es un outpost. Interpretacion conservadora de este repo: los puestos con trabajo EN VUELO, es decir los nodos que tienen alguna tarea 'running' o con un lease vivo ('lease_expira' en el futuro). Las tareas sin 'node' se agrupan bajo el nodo sintetico 'local' (se ejecutan en esta maquina).";

/// `list claimed sessions`: lo ya asignado a un nodo (columna `node`).
pub fn claimed_sessions(cola: &[Task]) -> Vec<Value> {
    cola.iter()
        .filter(|t| !t.node.trim().is_empty())
        .map(tarea_json)
        .collect()
}

/// `list queued sessions`: lo pendiente de ejecutar y sin dueño.
pub fn queued_sessions(cola: &[Task]) -> Vec<Value> {
    cola.iter()
        .filter(|t| t.state == State::Pending && t.node.trim().is_empty())
        .map(tarea_json)
        .collect()
}

/// `list outposts`: los puestos con trabajo en vuelo (**concepto NO MEDIDO**).
///
/// Interpretación conservadora (ver la cabecera del módulo): un "outpost" es
/// un nodo que **tiene trabajo en vuelo**:
/// * tareas en estado `running`, o
/// * tareas con un **lease vivo** (`lease_expira` en el futuro).
///
/// Las tareas sin `node` se agrupan bajo el nodo sintético [`NODO_LOCAL`].
pub fn outposts(cola: &[Task]) -> Vec<Value> {
    let t0 = ahora();
    let mut grupos: BTreeMap<String, Vec<&Task>> = BTreeMap::new();
    for t in cola {
        let en_vuelo = t.state == State::Running;
        let lease_vivo = t.lease_expira.map(|e| e > t0).unwrap_or(false);
        if !en_vuelo && !lease_vivo {
            continue;
        }
        let nodo = if t.node.trim().is_empty() {
            NODO_LOCAL
        } else {
            t.node.as_str()
        };
        grupos.entry(nodo.to_string()).or_default().push(t);
    }
    grupos
        .into_iter()
        .map(|(nodo, tareas)| {
            let lease = tareas.iter().filter_map(|t| t.lease_expira).max();
            json!({
                "node": nodo,
                "local": nodo == NODO_LOCAL,
                "count": tareas.len(),
                "lease_expira": lease,
                "sessions": tareas
                    .iter()
                    .map(|t| json!({
                        "id": t.id,
                        "kind": t.kind,
                        "state": t.state.as_str(),
                        "lease_expira": t.lease_expira,
                    }))
                    .collect::<Vec<Value>>(),
            })
        })
        .collect()
}

/// Construye la respuesta completa de una RPC (Puro).
pub fn construir_respuesta(comando: Comando, cola: &[Task]) -> Value {
    match comando {
        Comando::ListClaimedSessions => {
            let s = claimed_sessions(cola);
            json!({
                "ok": true,
                "command": CMD_CLAIMED,
                "result": {
                    "count": s.len(),
                    "definition": DEF_CLAIMED,
                    "sessions": s,
                }
            })
        }
        Comando::ListQueuedSessions => {
            let s = queued_sessions(cola);
            json!({
                "ok": true,
                "command": CMD_QUEUED,
                "result": {
                    "count": s.len(),
                    "definition": DEF_QUEUED,
                    "sessions": s,
                }
            })
        }
        Comando::ListOutposts => {
            let o = outposts(cola);
            json!({
                "ok": true,
                "command": CMD_OUTPOSTS,
                "result": {
                    "count": o.len(),
                    "definition": DEF_OUTPOSTS,
                    "outposts": o,
                }
            })
        }
    }
}

// ---------------------------------------------------------------------------
// TRANSPORTES: línea a línea (NDJSON)
// ---------------------------------------------------------------------------

/// Lee una línea (sin el `\n`), con tope de tamaño. `None` = EOF.
fn leer_linea(r: &mut impl BufRead, max: usize) -> Result<Option<String>, DaemonError> {
    let mut buf: Vec<u8> = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = r.read(&mut byte).map_err(|e| DaemonError::io(None, e))?;
        if n == 0 {
            if buf.is_empty() {
                return Ok(None);
            }
            break;
        }
        if byte[0] == b'\n' {
            break;
        }
        if buf.len() >= max {
            return Err(DaemonError::Malformed(format!(
                "linea de mas de {max} bytes"
            )));
        }
        buf.push(byte[0]);
    }
    Ok(Some(
        String::from_utf8_lossy(&buf)
            .trim_end_matches('\r')
            .trim()
            .to_string(),
    ))
}

/// Parsea una trama: debe ser un objeto JSON.
fn parsear_frame(linea: &str) -> Result<Value, String> {
    let v: Value = serde_json::from_str(linea).map_err(|e| format!("json invalido: {e}"))?;
    if !v.is_object() {
        return Err("la trama no es un objeto json".to_string());
    }
    Ok(v)
}

/// Escribe una trama (una línea JSON) y la vacía.
fn escribir_json(w: &mut impl Write, v: &Value) -> Result<(), DaemonError> {
    writeln!(w, "{v}").map_err(|e| DaemonError::io(None, e))?;
    w.flush().map_err(|e| DaemonError::io(None, e))
}

/// Error genérico (sin pistas sobre la credencial).
fn error_json(code: &str, msg: &str) -> Value {
    json!({ "ok": false, "error": { "code": code, "message": msg } })
}

/// Error de una RPC (incluye el comando pedido).
fn error_comando(comando: &str, code: &str, msg: &str) -> Value {
    json!({
        "ok": false,
        "command": comando.trim(),
        "error": { "code": code, "message": msg },
    })
}

// ---------------------------------------------------------------------------
// EL DAEMON
// ---------------------------------------------------------------------------

/// El daemon: lock de instancia + socket Unix + credencial del estado.
pub struct DaemonSocket {
    state_dir: PathBuf,
    socket_path: PathBuf,
    lock: InstanceLock,
    listener: UnixListener,
    credencial: String,
    cola: FuenteCola,
}

impl DaemonSocket {
    /// Levanta el daemon sobre un `state_dir` (cola real del proceso).
    ///
    /// Orden importante: **primero el lock** (sólo un daemon por estado) y
    /// **después** el socket. Si el socket ya existe es de un daemon que
    /// murió (el lock garantiza que no hay nadie vivo) ⇒ se reemplaza.
    pub fn bind(state_dir: &Path) -> Result<DaemonSocket, DaemonError> {
        Self::bind_con_fuente(state_dir, fuente_cola_real())
    }

    /// Igual que [`DaemonSocket::bind`] pero con la **fuente de la cola**
    /// inyectada (para tests: el estado de test se aísla por hilo).
    pub fn bind_con_cola<F>(state_dir: &Path, fuente: F) -> Result<DaemonSocket, DaemonError>
    where
        F: Fn() -> Vec<Task> + Send + Sync + 'static,
    {
        Self::bind_con_fuente(state_dir, Arc::new(fuente))
    }

    /// Implementación común (la cola ya viene como `FuenteCola`).
    fn bind_con_fuente(state_dir: &Path, cola: FuenteCola) -> Result<DaemonSocket, DaemonError> {
        fs::create_dir_all(state_dir).map_err(|e| DaemonError::io(Some(state_dir), e))?;
        // 1) Lock de instancia (si falla, no se abre el socket).
        let lock = InstanceLock::acquire(state_dir)?;
        // 2) Credencial del estado (si falla, el lock se suelta solo).
        let credencial = credencial_del_estado(state_dir)?;
        // 3) Socket Unix.
        let socket_path = state_dir.join(SOCKET_FILE);
        if socket_path.exists() {
            // Resto de un daemon muerto (el lock dice que no hay nadie vivo).
            let _ = fs::remove_file(&socket_path);
        }
        let listener =
            UnixListener::bind(&socket_path).map_err(|e| DaemonError::io(Some(&socket_path), e))?;
        let _ = fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600));
        Ok(DaemonSocket {
            state_dir: state_dir.to_path_buf(),
            socket_path,
            lock,
            listener,
            credencial,
            cola,
        })
    }

    /// Ruta del socket.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Ruta del lock.
    pub fn lock_path(&self) -> &Path {
        self.lock.path()
    }

    /// `state_dir` del daemon.
    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    /// La credencial que hay que presentar en el handshake.
    ///
    /// No se registra en ningún log: se entrega al cliente por la vía que
    /// el llamante elija (mismo `state_dir`, permisos 0600).
    pub fn credencial(&self) -> &str {
        &self.credencial
    }

    /// El listener (para tests y para integrar con otros bucles).
    pub fn listener(&self) -> &UnixListener {
        &self.listener
    }

    /// Bucle de servicio: acepta y atiende cada conexión en su hilo.
    ///
    /// No termina nunca (salvo error del listener).
    pub fn serve(self: &Arc<Self>) -> Result<(), DaemonError> {
        loop {
            let (stream, _addr) = self
                .listener
                .accept()
                .map_err(|e| DaemonError::io(Some(&self.socket_path), e))?;
            let d = Arc::clone(self);
            let _ = std::thread::spawn(move || {
                let _ = d.handle(stream);
            });
        }
    }

    /// Atiende **una** conexión y vuelve (para tests sin bucle infinito).
    pub fn serve_once(self: &Arc<Self>) -> Result<(), DaemonError> {
        let (stream, _addr) = self
            .listener
            .accept()
            .map_err(|e| DaemonError::io(Some(&self.socket_path), e))?;
        self.handle(stream)
    }

    /// Atiende una conexión: handshake y luego RPC en bucle.
    pub fn handle(&self, stream: UnixStream) -> Result<(), DaemonError> {
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(TIMEOUT_LECTURA));
        let _ = stream.set_write_timeout(Some(TIMEOUT_LECTURA));
        let mut lector = BufReader::new(stream.try_clone().map_err(|e| DaemonError::io(None, e))?);
        let mut escritor = stream;

        // --- 1) HANDSHAKE (fail-closed) -------------------------------------
        let Some(linea) = leer_linea(&mut lector, MAX_LINEA)? else {
            // EOF antes del handshake: ni una respuesta (fail-closed).
            return Err(DaemonError::Closed);
        };
        let frame = match parsear_frame(&linea) {
            Ok(f) => f,
            Err(msg) => {
                let _ = escribir_json(&mut escritor, &error_json("malformed_request", &msg));
                return Err(DaemonError::Malformed(msg));
            }
        };
        let presentada = frame
            .get("auth")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if !token_valido(&self.credencial, presentada) {
            // Error GENÉRICO y cierre: no se dice qué faltaba ni se sirve nada.
            let _ = escribir_json(
                &mut escritor,
                &error_json("unauthorized", "handshake rechazado"),
            );
            return Err(DaemonError::Unauthorized);
        }
        escribir_json(
            &mut escritor,
            &json!({ "ok": true, "event": "handshake", "protocol": PROTOCOLO }),
        )?;
        // Tolerancia: si el handshake traía ya un comando, se responde.
        if let Some(cmd) = frame.get("command").and_then(|v| v.as_str()) {
            let resp = self.respuesta_para(cmd);
            escribir_json(&mut escritor, &resp)?;
        }

        // --- 2) RPC en bucle -------------------------------------------------
        loop {
            let Some(linea) = leer_linea(&mut lector, MAX_LINEA)? else {
                break; // EOF: el cliente se fue.
            };
            if linea.is_empty() {
                continue;
            }
            let resp = match parsear_frame(&linea) {
                Ok(frame) => match frame.get("command").and_then(|v| v.as_str()) {
                    Some(cmd) => self.respuesta_para(cmd),
                    None => error_comando("", "malformed_request", "falta el campo 'command'"),
                },
                Err(msg) => error_comando("", "malformed_request", &msg),
            };
            escribir_json(&mut escritor, &resp)?;
        }
        Ok(())
    }

    /// Responde a una RPC (consulta la cola real y serializa).
    pub fn respuesta_para(&self, comando: &str) -> Value {
        match Comando::parse(comando) {
            Ok(c) => {
                let cola = (self.cola)();
                construir_respuesta(c, &cola)
            }
            Err(DaemonError::UnknownCommand(_)) => error_comando(
                comando,
                "unknown_command",
                &format!("comando desconocido: {}", comando.trim()),
            ),
            Err(_) => error_comando(
                comando,
                "malformed_request",
                "no se pudo clasificar la peticion",
            ),
        }
    }
}

impl Drop for DaemonSocket {
    fn drop(&mut self) {
        // El socket se borra al soltar el daemon (y el lock, en su propio Drop).
        let _ = fs::remove_file(&self.socket_path);
    }
}

/// `Debug` **manual**: la credencial es un secreto y **no** se imprime
/// (un `derive(Debug)` la volcaría en cualquier log de un `{:?}`).
impl std::fmt::Debug for DaemonSocket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DaemonSocket")
            .field("state_dir", &self.state_dir)
            .field("socket_path", &self.socket_path)
            .field("lock_path", &self.lock.path())
            .field("credencial", &"<oculta>")
            .finish()
    }
}

// ---------------------------------------------------------------------------
// EL CLIENTE
// ---------------------------------------------------------------------------

/// Cliente del socket: hace el handshake y lanza RPC.
pub struct DaemonClient {
    lector: BufReader<UnixStream>,
    escritor: UnixStream,
}

impl DaemonClient {
    /// Conecta al socket (aún sin autenticar).
    pub fn connect(socket_path: &Path) -> Result<DaemonClient, DaemonError> {
        let s =
            UnixStream::connect(socket_path).map_err(|e| DaemonError::io(Some(socket_path), e))?;
        let r = s
            .try_clone()
            .map_err(|e| DaemonError::io(Some(socket_path), e))?;
        Ok(DaemonClient {
            lector: BufReader::new(r),
            escritor: s,
        })
    }

    /// Conecta **y** hace el handshake (fail-closed: si no vale, error).
    pub fn connect_handshake(
        socket_path: &Path,
        credencial: &str,
    ) -> Result<DaemonClient, DaemonError> {
        let mut c = DaemonClient::connect(socket_path)?;
        c.handshake(credencial)?;
        Ok(c)
    }

    /// Presenta la credencial. Si el daemon no la acepta ⇒ `Unauthorized`.
    pub fn handshake(&mut self, credencial: &str) -> Result<(), DaemonError> {
        self.escribir(&json!({ "auth": credencial }))?;
        let resp = self.leer()?;
        if resp.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            return Err(DaemonError::Unauthorized);
        }
        Ok(())
    }

    /// Lanza una RPC y devuelve la respuesta cruda (JSON).
    pub fn call(&mut self, comando: &str) -> Result<Value, DaemonError> {
        self.escribir(&json!({ "command": comando }))?;
        self.leer()
    }

    /// Escribe una trama.
    fn escribir(&mut self, v: &Value) -> Result<(), DaemonError> {
        writeln!(self.escritor, "{v}").map_err(|e| DaemonError::io(None, e))?;
        self.escritor.flush().map_err(|e| DaemonError::io(None, e))
    }

    /// Lee una trama.
    fn leer(&mut self) -> Result<Value, DaemonError> {
        let mut linea = String::new();
        let n = self
            .lector
            .read_line(&mut linea)
            .map_err(|e| DaemonError::io(None, e))?;
        if n == 0 {
            return Err(DaemonError::Closed);
        }
        serde_json::from_str(linea.trim())
            .map_err(|e| DaemonError::Malformed(format!("respuesta no json: {e}")))
    }
}

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_scheduler::Resources;
    use std::sync::atomic::{AtomicU64, Ordering};

    static CONTADOR: AtomicU64 = AtomicU64::new(0);

    /// Directorio temporal ÚNICO por test (sin `tempfile`).
    fn dir_de_test(nombre: &str) -> PathBuf {
        let n = CONTADOR.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "megadaemon-daemon-sock-{}-{}-{}",
            std::process::id(),
            n,
            nombre
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("crear el directorio del test");
        dir
    }

    /// Borra el temporal al terminar el test.
    struct Limpiador(PathBuf);

    impl Drop for Limpiador {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Tarea de prueba con el tipo REAL (`task_scheduler::Task`).
    fn tarea(id: &str, estado: State, node: &str, lease: Option<u64>) -> Task {
        Task {
            id: id.to_string(),
            kind: "test.daemon".to_string(),
            priority: 1,
            payload: json!({ "x": 1 }),
            state: estado,
            attempts: 0,
            max_attempts: 2,
            deps: vec![],
            res: Resources::default(),
            created_at: 1,
            started_at: None,
            finished_at: None,
            error: None,
            result: None,
            node: node.to_string(),
            resume_after: None,
            lease_expira: lease,
            motivo_pausa: None,
        }
    }

    /// Levanta un daemon con la cola REAL capturada en este hilo.
    ///
    /// `state_dir::state_dir_de_test()` aísla la cola **por hilo** ⇒ el hilo
    /// que atiende el socket vería una cola vacía. Por eso aquí se inyecta
    /// la cola real (misma API `task_scheduler`) como proveedor.
    fn daemon_con_cola_real(dir: &Path) -> Arc<DaemonSocket> {
        let cola = crate::task_scheduler::list();
        Arc::new(DaemonSocket::bind_con_cola(dir, move || cola.clone()).expect("bind del daemon"))
    }

    // --- Criterio 1: dos daemons sobre el mismo state_dir ------------------

    #[test]
    fn el_segundo_daemon_no_arranca_sobre_el_mismo_state_dir() {
        let dir = dir_de_test("lock");
        let _limpieza = Limpiador(dir.clone());

        let _l1 = InstanceLock::acquire(&dir).expect("el primer daemon toma el lock");
        let e = InstanceLock::acquire(&dir).expect_err("el segundo daemon NO debe arrancar");
        match &e {
            DaemonError::LockTaken { .. } => {}
            otro => panic!("se esperaba LockTaken, se obtuvo {otro:?}"),
        }
        let msg = e.to_string();
        assert!(
            msg.contains("otro daemon"),
            "el error debe ser claro: {msg}"
        );
    }

    #[test]
    fn el_segundo_daemon_no_puede_abrir_el_socket() {
        let dir = dir_de_test("lock-socket");
        let _limpieza = Limpiador(dir.clone());

        let _d1 = DaemonSocket::bind(&dir).expect("el primer daemon arranca");
        let e = DaemonSocket::bind(&dir).expect_err("el segundo daemon NO debe arrancar");
        assert!(matches!(e, DaemonError::LockTaken { .. }), "{e:?}");
    }

    #[test]
    fn el_lock_se_libera_al_soltar_el_guard() {
        let dir = dir_de_test("lock-drop");
        let _limpieza = Limpiador(dir.clone());

        let l = InstanceLock::acquire(&dir).expect("lock");
        assert!(l.path().exists(), "el fichero de lock debe existir");
        drop(l);
        assert!(
            !dir.join(LOCK_FILE).exists(),
            "al soltar el guard el lock debe desaparecer"
        );
        // Y otro daemon puede tomar el relevo.
        let _l2 = InstanceLock::acquire(&dir).expect("el lock queda libre");
    }

    #[test]
    fn un_lock_caducado_se_puede_tomar_el_relevo() {
        let dir = dir_de_test("lock-stale");
        let _limpieza = Limpiador(dir.clone());

        // Lock de un daemon MUERTO (pid que no existe).
        fs::write(
            dir.join(LOCK_FILE),
            json!({ "pid": 4294967u32, "started_at": 0 }).to_string(),
        )
        .expect("escribir lock caducado");
        let l = InstanceLock::acquire(&dir).expect("un lock caducado no debe bloquear");
        assert_eq!(l.pid(), std::process::id());
    }

    // --- El socket se limpia ---------------------------------------------

    #[test]
    fn el_socket_se_limpia_al_soltar_el_daemon() {
        let dir = dir_de_test("socket-drop");
        let _limpieza = Limpiador(dir.clone());

        let d = DaemonSocket::bind(&dir).expect("bind");
        assert!(d.socket_path().exists(), "el socket debe existir");
        let socket = d.socket_path().to_path_buf();
        drop(d);
        assert!(
            !socket.exists(),
            "al soltar el daemon el socket debe desaparecer"
        );
        assert!(
            !dir.join(LOCK_FILE).exists(),
            "el lock también debe soltarse"
        );
    }

    #[test]
    fn un_socket_heredado_de_un_daemon_muerto_se_reemplaza() {
        let dir = dir_de_test("socket-stale");
        let _limpieza = Limpiador(dir.clone());

        fs::write(dir.join(SOCKET_FILE), b"resto de un daemon muerto").expect("fichero sucio");
        let d = DaemonSocket::bind(&dir).expect("bind sobre un socket heredado");
        assert!(d.socket_path().exists());
    }

    // --- Criterio 2: sin credencial no hay API ----------------------------

    #[test]
    fn un_cliente_sin_credencial_es_rechazado() {
        let dir = dir_de_test("sin-credencial");
        let _limpieza = Limpiador(dir.clone());

        let daemon = Arc::new(DaemonSocket::bind(&dir).expect("bind"));
        let h = {
            let d = Arc::clone(&daemon);
            std::thread::spawn(move || {
                let _ = d.serve_once();
            })
        };

        let mut c = DaemonClient::connect(daemon.socket_path()).expect("conectar");
        let e = c
            .handshake("")
            .expect_err("sin credencial no debe pasar el handshake");
        assert!(matches!(e, DaemonError::Unauthorized), "{e:?}");

        // Y además no puede usar la API: la conexión está cerrada.
        let r = c.call(CMD_QUEUED).expect_err("no debe poder hacer RPC");
        assert!(
            matches!(r, DaemonError::Closed | DaemonError::Io { .. }),
            "{r:?}"
        );
        // Se cierra el cliente ANTES de unir el hilo del servidor: si no, el
        // servidor se queda leyendo hasta su timeout de 30 s.
        drop(c);
        let _ = h.join();
    }

    #[test]
    fn un_cliente_con_una_credencial_equivocada_es_rechazado() {
        let dir = dir_de_test("credencial-mala");
        let _limpieza = Limpiador(dir.clone());

        let daemon = Arc::new(DaemonSocket::bind(&dir).expect("bind"));
        let h = {
            let d = Arc::clone(&daemon);
            std::thread::spawn(move || {
                let _ = d.serve_once();
            })
        };

        let mut c = DaemonClient::connect(daemon.socket_path()).expect("conectar");
        let e = c
            .handshake("esta-no-es-la-credencial-del-estado")
            .expect_err("una credencial falsa no debe pasar");
        assert!(matches!(e, DaemonError::Unauthorized), "{e:?}");
        // Se cierra el cliente ANTES de unir el hilo del servidor: si no, el
        // servidor se queda leyendo hasta su timeout de 30 s.
        drop(c);
        let _ = h.join();
    }

    #[test]
    fn un_cliente_con_credencial_hace_handshake_y_puede_hacer_rpc() {
        let dir = dir_de_test("con-credencial");
        let _limpieza = Limpiador(dir.clone());

        let daemon = Arc::new(DaemonSocket::bind(&dir).expect("bind"));
        let h = {
            let d = Arc::clone(&daemon);
            std::thread::spawn(move || {
                let _ = d.serve_once();
            })
        };

        let mut c = DaemonClient::connect_handshake(daemon.socket_path(), daemon.credencial())
            .expect("handshake");
        let resp = c.call(CMD_QUEUED).expect("la RPC debe responder");
        assert_eq!(resp["ok"], json!(true));
        assert_eq!(resp["command"], json!(CMD_QUEUED));
        assert!(resp["result"]["sessions"].is_array());
        // Se cierra el cliente ANTES de unir el hilo del servidor: si no, el
        // servidor se queda leyendo hasta su timeout de 30 s.
        drop(c);
        let _ = h.join();
    }

    // --- Criterio 3: las tres RPC devuelven la cola REAL -------------------

    #[test]
    fn list_claimed_sessions_devuelve_la_cola_real() {
        let dir = dir_de_test("claimed");
        let _limpieza = Limpiador(dir.clone());

        // Cola real: se encola y se reparte (nodo asignado => "claimed").
        let id = crate::task_scheduler::submit(
            "test.daemon.claimed",
            json!({ "n": 1 }),
            5,
            vec![],
            Resources::default(),
        );
        let repartidas = crate::task_scheduler::repartir_por_nodo("nodo-a", 10, 600);
        assert!(repartidas.contains(&id), "la tarea debe quedar repartida");

        let cola = crate::task_scheduler::list();
        let resp = construir_respuesta(Comando::ListClaimedSessions, &cola);
        assert_eq!(resp["ok"], json!(true));
        assert_eq!(resp["result"]["count"], json!(1));
        assert_eq!(resp["result"]["sessions"][0]["id"], json!(id));
        assert_eq!(resp["result"]["sessions"][0]["node"], json!("nodo-a"));
        assert_eq!(resp["result"]["sessions"][0]["state"], json!("pending"));
    }

    #[test]
    fn list_queued_sessions_devuelve_la_cola_real() {
        let dir = dir_de_test("queued");
        let _limpieza = Limpiador(dir.clone());

        let id = crate::task_scheduler::submit(
            "test.daemon.queued",
            json!({ "n": 2 }),
            3,
            vec![],
            Resources::default(),
        );
        let cola = crate::task_scheduler::list();
        let resp = construir_respuesta(Comando::ListQueuedSessions, &cola);
        assert_eq!(resp["ok"], json!(true));
        assert_eq!(resp["result"]["count"], json!(1));
        assert_eq!(resp["result"]["sessions"][0]["id"], json!(id));
        assert_eq!(resp["result"]["sessions"][0]["state"], json!("pending"));
        assert_eq!(resp["result"]["sessions"][0]["node"], json!(""));
    }

    #[test]
    fn list_outposts_devuelve_las_tareas_en_vuelo() {
        let dir = dir_de_test("outposts");
        let _limpieza = Limpiador(dir.clone());

        let id = crate::task_scheduler::submit(
            "test.daemon.outpost",
            json!({ "n": 3 }),
            3,
            vec![],
            Resources::default(),
        );
        crate::task_scheduler::repartir_por_nodo("nodo-b", 10, 600);

        let cola = crate::task_scheduler::list();
        let resp = construir_respuesta(Comando::ListOutposts, &cola);
        assert_eq!(resp["ok"], json!(true));
        assert_eq!(resp["result"]["count"], json!(1));
        let outpost = &resp["result"]["outposts"][0];
        assert_eq!(outpost["node"], json!("nodo-b"));
        assert_eq!(outpost["count"], json!(1));
        assert_eq!(outpost["sessions"][0]["id"], json!(id));
        // La definición viaja en la respuesta: el consumidor sabe qué lee.
        assert!(resp["result"]["definition"]
            .as_str()
            .unwrap()
            .contains("NO MEDIDO"));
    }

    #[test]
    fn por_el_socket_llega_la_cola_real() {
        let dir = dir_de_test("e2e");
        let _limpieza = Limpiador(dir.clone());

        let id = crate::task_scheduler::submit(
            "test.daemon.e2e",
            json!({ "n": 7 }),
            4,
            vec![],
            Resources::default(),
        );
        crate::task_scheduler::repartir_por_nodo("nodo-e2e", 10, 600);

        let daemon = daemon_con_cola_real(&dir);
        let h = {
            let d = Arc::clone(&daemon);
            std::thread::spawn(move || {
                let _ = d.serve_once();
            })
        };

        let mut c = DaemonClient::connect_handshake(daemon.socket_path(), daemon.credencial())
            .expect("handshake");
        let resp = c.call(CMD_CLAIMED).expect("RPC por el socket");
        assert_eq!(resp["ok"], json!(true));
        assert_eq!(resp["command"], json!(CMD_CLAIMED));
        assert_eq!(resp["result"]["count"], json!(1));
        assert_eq!(resp["result"]["sessions"][0]["id"], json!(id));
        assert_eq!(resp["result"]["sessions"][0]["node"], json!("nodo-e2e"));
        // Se cierra el cliente ANTES de unir el hilo del servidor: si no, el
        // servidor se queda leyendo hasta su timeout de 30 s.
        drop(c);
        let _ = h.join();
    }

    #[test]
    fn las_tres_consultas_particionan_la_cola() {
        let cola = vec![
            tarea("t-running", State::Running, "nodo-a", Some(ahora() + 600)),
            tarea(
                "t-claimed-pending",
                State::Pending,
                "nodo-b",
                Some(ahora() + 600),
            ),
            tarea("t-queued", State::Pending, "", None),
            tarea("t-done", State::Done, "nodo-a", None),
        ];

        let claimed = claimed_sessions(&cola);
        let queued = queued_sessions(&cola);
        let out = outposts(&cola);

        assert_eq!(claimed.len(), 3, "claimed = todo lo que tiene nodo");
        assert!(
            claimed.iter().any(|s| s["id"] == json!("t-done")),
            "una tarea terminada conserva su nodo"
        );
        assert_eq!(queued.len(), 1, "queued = pending sin nodo");
        assert_eq!(queued[0]["id"], json!("t-queued"));
        // outposts = los nodos con trabajo EN VUELO: 'running' o lease vivo.
        // 'nodo-a' tiene la que esta en curso; 'nodo-b' tiene la repartida
        // con lease vivo (todavia no ha arrancado, pero su nodo la sostiene).
        assert_eq!(out.len(), 2, "outposts = los nodos con trabajo en vuelo");
        let a = out
            .iter()
            .find(|o| o["node"] == json!("nodo-a"))
            .expect("el puesto de nodo-a");
        assert_eq!(a["count"], json!(1));
        assert_eq!(a["sessions"][0]["id"], json!("t-running"));
        let b = out
            .iter()
            .find(|o| o["node"] == json!("nodo-b"))
            .expect("el puesto de nodo-b");
        assert_eq!(b["count"], json!(1));
        assert_eq!(b["sessions"][0]["id"], json!("t-claimed-pending"));
    }

    #[test]
    fn un_trabajo_local_sin_nodo_cuenta_como_outpost_local() {
        let cola = vec![tarea("t-local", State::Running, "", None)];
        let out = outposts(&cola);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["node"], json!(NODO_LOCAL));
        assert_eq!(out[0]["local"], json!(true));
    }

    // --- Petición desconocida: error tipado, sin pánico -------------------

    #[test]
    fn una_peticion_desconocida_da_error_tipado() {
        let e = Comando::parse("list banana sessions").expect_err("debe fallar");
        assert!(matches!(e, DaemonError::UnknownCommand(_)), "{e:?}");
        assert!(e.to_string().contains("banana"), "{e}");
    }

    #[test]
    fn una_peticion_desconocida_por_el_socket_da_error_tipado() {
        let dir = dir_de_test("desconocido");
        let _limpieza = Limpiador(dir.clone());

        let daemon = Arc::new(DaemonSocket::bind(&dir).expect("bind"));
        let h = {
            let d = Arc::clone(&daemon);
            std::thread::spawn(move || {
                let _ = d.serve_once();
            })
        };

        let mut c = DaemonClient::connect_handshake(daemon.socket_path(), daemon.credencial())
            .expect("handshake");
        let resp = c.call("create outpost").expect("respuesta");
        assert_eq!(resp["ok"], json!(false));
        assert_eq!(resp["error"]["code"], json!("unknown_command"));
        assert_eq!(resp["command"], json!("create outpost"));
        // El daemon sigue vivo: no entra en pánico.
        let resp2 = c.call(CMD_OUTPOSTS).expect("respuesta");
        assert_eq!(resp2["ok"], json!(true));
        // Se cierra el cliente ANTES de unir el hilo del servidor: si no, el
        // servidor se queda leyendo hasta su timeout de 30 s.
        drop(c);
        let _ = h.join();
    }

    // --- La credencial del estado -----------------------------------------

    #[test]
    fn la_credencial_reutiliza_el_token_del_cockpit() {
        let dir = dir_de_test("credencial-cockpit");
        let _limpieza = Limpiador(dir.clone());

        let token = "cockpit-token-del-estado-0001";
        fs::write(dir.join(COCKPIT_TOKEN_FILE), token).expect("escribir token");
        let c = credencial_del_estado(&dir).expect("credencial");
        assert_eq!(c, token);
    }

    #[test]
    fn la_credencial_propia_se_genera_con_permisos_0600() {
        let dir = dir_de_test("credencial-propia");
        let _limpieza = Limpiador(dir.clone());

        let c = credencial_del_estado(&dir).expect("credencial");
        assert_eq!(c.len(), 64, "32 bytes en hex");
        let ruta = dir.join(DAEMON_TOKEN_FILE);
        assert!(ruta.exists(), "la credencial propia debe guardarse");
        let modo = fs::metadata(&ruta).expect("metadatos").permissions().mode() & 0o777;
        assert_eq!(modo, 0o600, "la credencial propia debe ser 0600");
        // Y es estable entre llamadas.
        assert_eq!(credencial_del_estado(&dir).expect("credencial"), c);
    }

    #[test]
    fn la_comparacion_de_credenciales_es_exacta() {
        assert!(token_valido("abcdef", "abcdef"));
        assert!(!token_valido("abcdef", "abcdeg"));
        assert!(!token_valido("abcdef", "abcde"));
        assert!(!token_valido("abcdef", ""));
        assert!(!token_valido("", ""));
    }
}
