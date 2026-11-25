//! MegaDaemon v13.4 — Planificador multitarea.
//!
//! Un monolito puede hacer muchas cosas a la vez **si** las cosas que hace
//! son tareas con estado, prioridad, dependencias y presupuesto de recursos.
//! Este módulo es esa cola:
//!
//! * `Task` describe una unidad de trabajo (qué, con qué datos, con qué
//!   dependencias, qué recursos consume).
//! * El planificador decide **qué se ejecuta ahora** teniendo en cuenta:
//!   prioridad, dependencias satisfechas, presupuesto de CPU/red y la
//!   **política de energía** del dispositivo (`crate::power_policy`): sin
//!   cargador y con batería baja no se lanzan tareas pesadas.
//! * El estado se **persiste** en disco, de modo que un reinicio no pierde el
//!   trabajo en curso.
//! * `export_tasks` / `import_tasks` permiten repartir el trabajo entre varios
//!   nodos: es la base del futuro sistema distribuido (móvil + PC Linux).
//! * Todo lo que se ejecuta queda en un **journal** en disco: se puede
//!   auditar qué se hizo y por qué.

use serde_json::{json, Value};
use std::io::Write;
use std::sync::{Mutex, OnceLock};

/// Estado de una tarea.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Pending,
    Running,
    /// Pausada: se espera a `resume_after` (cuota agotada, red caida...).
    Paused,
    Done,
    Failed,
    Cancelled,
}

impl State {
    pub fn as_str(&self) -> &'static str {
        match self {
            State::Pending => "pending",
            State::Running => "running",
            State::Paused => "paused",
            State::Done => "done",
            State::Failed => "failed",
            State::Cancelled => "cancelled",
        }
    }
    fn from_str(s: &str) -> State {
        match s {
            "running" => State::Running,
            "paused" => State::Paused,
            "done" => State::Done,
            "failed" => State::Failed,
            "cancelled" => State::Cancelled,
            _ => State::Pending,
        }
    }
}

/// Recursos consumidos por una tarea.
#[derive(Clone, Copy, Debug, Default)]
pub struct Resources {
    /// Coste de CPU relativo (0 = trivial, 3 = pesado).
    pub cpu: u8,
    /// Necesita red.
    pub net: bool,
    /// No debe lanzarse con batería baja (o sin cargador).
    pub heavy: bool,
}

/// Una unidad de trabajo.
#[derive(Clone, Debug)]
pub struct Task {
    pub id: String,
    pub kind: String,
    pub priority: u8,
    pub payload: Value,
    pub state: State,
    pub attempts: u32,
    pub max_attempts: u32,
    pub deps: Vec<String>,
    pub res: Resources,
    pub created_at: u64,
    pub started_at: Option<u64>,
    pub finished_at: Option<u64>,
    pub error: Option<String>,
    pub result: Option<Value>,
    /// Nodo al que se asignó (para el reparto distribuido).
    pub node: String,
    /// Epoch hasta el que la tarea está en pausa (cuota, red...).  `None`
    /// = se puede ejecutar ya.
    pub resume_after: Option<u64>,
    /// Epoch en que caduca el lease del nodo que la tiene.
    pub lease_expira: Option<u64>,
    /// Motivo de la última pausa (para que el agente lo sepa).
    pub motivo_pausa: Option<String>,
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// La cola GLOBAL, aislada por `state_dir`.
///
/// MEDIDO 2026-11-25 (CI rojo, run 0d74b89): la cola era un `static` único
/// del proceso ⇒ TODOS los tests la compartían y, como corren en PARALELO,
/// se pisaban: `un_lease_caducado_devuelve_la_tarea_a_la_cola` fallaba con
/// "assertion failed: t.node.is_empty()" (otro test le había reclamado la
/// tarea) y `migrar_tareas_mueve_el_trabajo_a_otro_nodo` con `n == 1`.
///
/// Ahora hay una cola por `state_dir` (en producción hay UNO solo ⇒ el
/// comportamiento es el de antes) y, como el `state_dir` de los tests
/// deriva del hilo (`state_dir::state_dir_de_test()`), cada test tiene su
/// cola.  `Box::leak` da la referencia `'static` que pide la firma.
fn queue() -> &'static Mutex<Vec<Task>> {
    /// Mapa `state_dir` -> cola (alias para que el parser no confunda los
    /// `>>>` anidados).
    type Mapa = std::collections::HashMap<String, &'static Mutex<Vec<Task>>>;
    static MAP: OnceLock<Mutex<Mapa>> = OnceLock::new();
    let clave = crate::cockpit_server::state_dir();
    let mapa = MAP.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    let mut g = mapa.lock().unwrap_or_else(|p| p.into_inner());
    g.entry(clave)
        .or_insert_with(|| Box::leak(Box::new(Mutex::new(Vec::new()))))
}

/// La bitácora, aislada por `state_dir` (mismo motivo que [`queue`]).
fn journal() -> &'static Mutex<Vec<Value>> {
    type Mapa = std::collections::HashMap<String, &'static Mutex<Vec<Value>>>;
    static MAP: OnceLock<Mutex<Mapa>> = OnceLock::new();
    let clave = crate::cockpit_server::state_dir();
    let mapa = MAP.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    let mut g = mapa.lock().unwrap_or_else(|p| p.into_inner());
    g.entry(clave)
        .or_insert_with(|| Box::leak(Box::new(Mutex::new(Vec::new()))))
}

fn queue_path() -> String {
    format!("{}/task_queue.json", crate::cockpit_server::state_dir())
}

fn journal_path() -> String {
    format!("{}/task_journal.ndjson", crate::cockpit_server::state_dir())
}

/// Genera un id de tarea estable y único.
fn new_id(kind: &str) -> String {
    static COUNTER: OnceLock<Mutex<u64>> = OnceLock::new();
    let c = COUNTER.get_or_init(|| Mutex::new(0));
    let mut n = c.lock().unwrap_or_else(|p| p.into_inner());
    *n += 1;
    format!("{}-{}-{:04}", kind, now(), *n)
}

/// Encola una tarea y devuelve su id.
pub fn submit(kind: &str, payload: Value, priority: u8, deps: Vec<String>, res: Resources) -> String {
    let t = Task {
        id: new_id(kind),
        kind: kind.to_string(),
        priority: priority.min(255),
        payload,
        state: State::Pending,
        attempts: 0,
        max_attempts: 2,
        deps,
        res,
        created_at: now(),
        started_at: None,
        finished_at: None,
        error: None,
        result: None,
        node: String::new(),
        resume_after: None,
        lease_expira: None,
        motivo_pausa: None,
    };
    let id = t.id.clone();
    queue().lock().unwrap_or_else(|p| p.into_inner()).push(t);
    persist();
    id
}

/// Encola y ejecuta en el acto (atajo del agente).
pub fn submit_now(kind: &str, payload: Value, priority: u8) -> Value {
    let id = submit(kind, payload, priority, vec![], Resources::default());
    run_due(1)
        .into_iter()
        .find(|r| r["id"] == json!(id))
        .unwrap_or(json!({ "ok": false, "error": "no_task_ran", "id": id }))
}

/// Todas las tareas.
pub fn list() -> Vec<Task> {
    queue().lock().unwrap_or_else(|p| p.into_inner()).clone()
}

/// Una tarea por id.
pub fn get(id: &str) -> Option<Task> {
    queue().lock().unwrap_or_else(|p| p.into_inner()).iter().find(|t| t.id == id).cloned()
}

/// Cancela una tarea pendiente.
pub fn cancel(id: &str) -> bool {
    let mut q = queue().lock().unwrap_or_else(|p| p.into_inner());
    if let Some(t) = q.iter_mut().find(|t| t.id == id) {
        if t.state == State::Pending || t.state == State::Running {
            t.state = State::Cancelled;
            t.finished_at = Some(now());
            record(&format!("cancel:{}", id), &json!({ "kind": t.kind }));
            drop(q);
            persist();
            return true;
        }
    }
    false
}

/// Reintenta una tarea fallida.
pub fn retry(id: &str) -> bool {
    let mut q = queue().lock().unwrap_or_else(|p| p.into_inner());
    if let Some(t) = q.iter_mut().find(|t| t.id == id) {
        if t.state == State::Failed {
            t.state = State::Pending;
            t.error = None;
            t.attempts = 0;
            drop(q);
            persist();
            return true;
        }
    }
    false
}

/// ¿Están satisfechas las dependencias de la tarea?
fn deps_ok(t: &Task, q: &[Task]) -> bool {
    t.deps.iter().all(|d| {
        q.iter()
            .find(|x| x.id == *d)
            .map(|x| x.state == State::Done)
            .unwrap_or(false)
    })
}

/// Selección de la siguiente tarea a ejecutar, según energía y recursos.
fn pick(q: &[Task], allow_heavy: bool, max_cpu: u8) -> Option<String> {
    let t0 = now();
    let mut best: Option<&Task> = None;
    for t in q {
        if t.state != State::Pending {
            continue;
        }
        // Una tarea con `resume_after` futuro NO se ejecuta (cuota o red).
        if t.resume_after.map(|r| r > t0).unwrap_or(false) {
            continue;
        }
        if !deps_ok(t, q) {
            continue;
        }
        if t.res.heavy && !allow_heavy {
            continue;
        }
        if t.res.cpu > max_cpu {
            continue;
        }
        best = match best {
            None => Some(t),
            Some(b) => {
                if t.priority > b.priority
                    || (t.priority == b.priority && t.created_at < b.created_at)
                {
                    Some(t)
                } else {
                    Some(b)
                }
            }
        };
    }
    best.map(|t| t.id.clone())
}

/// Ejecuta como máximo `max` tareas pendientes y devuelve sus resultados.
///
/// El ejecutor es **interpretativo**: cada `kind` de tarea tiene un handler
/// registrado en `crate::agent_core`.  Aquí sólo se gestiona el ciclo de
/// vida, la cola y la persistencia.
pub fn run_due(max: usize) -> Vec<Value> {
    let mut out = vec![];
    for _ in 0..max {
        // Política de energía real del dispositivo.
        let (pstate, pol) = crate::power_policy::current_policy();
        let allow_heavy = pol.allow_heavy || pstate.charging;
        let max_cpu: u8 = if allow_heavy { 3 } else { 1 };

        // Despausar lo que ya puede volver (cuota recuperada, reset pasado).
        {
            let mut q = queue().lock().unwrap_or_else(|p| p.into_inner());
            let t0 = now();
            for t in q.iter_mut() {
                if t.state == State::Paused {
                    if t.resume_after.map(|r| r <= t0).unwrap_or(true) {
                        t.state = State::Pending;
                        t.motivo_pausa = None;
                    }
                }
            }
        }
        let id = {
            let q = queue().lock().unwrap_or_else(|p| p.into_inner());
            pick(&q, allow_heavy, max_cpu)
        };
        let Some(id) = id else { break };

        // Marca en curso.
        {
            let mut q = queue().lock().unwrap_or_else(|p| p.into_inner());
            if let Some(t) = q.iter_mut().find(|t| t.id == id) {
                t.state = State::Running;
                t.started_at = Some(now());
                t.attempts += 1;
            }
        }
        persist();

        let payload = get(&id).map(|t| t.payload.clone()).unwrap_or(Value::Null);
        let kind = get(&id).map(|t| t.kind.clone()).unwrap_or_default();
        let outcome = crate::agent_core::execute(&kind, &payload);

        let ok = outcome.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
        let err = outcome
            .get("error")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        {
            let mut q = queue().lock().unwrap_or_else(|p| p.into_inner());
            if let Some(t) = q.iter_mut().find(|t| t.id == id) {
                t.finished_at = Some(now());
                t.result = Some(outcome.clone());
                if ok {
                    t.state = State::Done;
                } else if t.attempts < t.max_attempts {
                    t.state = State::Pending; // reintento con backoff lógico
                    t.error = err.clone();
                } else {
                    t.state = State::Failed;
                    t.error = err.clone();
                }
            }
        }
        record(&kind, &outcome);
        out.push(json!({ "id": id, "kind": kind, "ok": ok, "result": outcome }));
        persist();
    }
    out
}

/// Escribe una entrada en el journal.
fn record(kind: &str, outcome: &Value) {
    let entry = json!({
        "ts": now(),
        "kind": kind,
        "ok": outcome.get("ok").cloned().unwrap_or(Value::Null),
        "summary": outcome
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .chars()
            .take(200)
            .collect::<String>(),
    });
    let mut j = journal().lock().unwrap_or_else(|p| p.into_inner());
    j.push(entry.clone());
    if j.len() > 500 {
        let excess = j.len() - 500;
        j.drain(0..excess);
    }
    drop(j);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(journal_path())
    {
        let _ = writeln!(f, "{}", entry);
    }
}

/// Guarda la cola en disco.
pub fn persist() {
    let q = queue().lock().unwrap_or_else(|p| p.into_inner());
    let items: Vec<Value> = q
        .iter()
        .map(|t| {
            json!({
                "id": t.id, "kind": t.kind, "priority": t.priority,
                "payload": t.payload, "state": t.state.as_str(),
                "attempts": t.attempts, "max_attempts": t.max_attempts,
                "deps": t.deps, "created_at": t.created_at,
                "started_at": t.started_at, "finished_at": t.finished_at,
                "error": t.error, "node": t.node,
                "resume_after": t.resume_after, "lease_expira": t.lease_expira,
                "motivo_pausa": t.motivo_pausa,
            })
        })
        .collect();
    drop(q);
    let _ = std::fs::create_dir_all(crate::cockpit_server::state_dir());
    let ruta = queue_path();
    // MEDIDO (2026-11-23, auditoría de calidad): la cola se guardaba con
    // `fs::write`, que **trunca** el destino.  Si el proceso moría a mitad de
    // la escritura, `task_queue.json` quedaba corrupto y `restore()` devolvía
    // 0 tareas sin explicar por qué => el trabajo en curso se perdía en
    // silencio.  Ahora se escribe de forma ATÓMICA (tmp + rename): el
    // fichero que hay en disco es siempre el de la última cola COMPLETA.
    let datos = serde_json::to_string_pretty(&json!({ "tasks": items })).unwrap_or_default();
    if let Err(e) = crate::util_fs::write_atomic(
        std::path::Path::new(&ruta),
        datos.as_bytes(),
        0o600,
    ) {
        eprintln!("[task_scheduler] no se pudo guardar la cola en {ruta}: {e}");
    }
}

/// Restaura la cola desde disco.
///
/// MEDIDO (2026-11-23, auditoría de calidad): antes `restore()` devolvía 0
/// tanto si no había cola como si la cola estaba CORRUPTA (JSON inválido) ⇒
/// el llamante no podía distinguir "no hay nada que restaurar" de "el
/// trabajo se perdió" (la clase de bug nº4 de los 4 bugs reales del motor
/// de backup: el llamante necesita el MOTIVO, no un error genérico).
///
/// Ahora:
/// * si el fichero no existe ⇒ 0 (y no es un error);
/// * si el fichero existe pero no se puede leer ⇒ se avisa por stderr;
/// * si el fichero está corrupto ⇒ se avisa por stderr y se **conserva**
///   como `<cola>.corrupto` (el trabajo no se borra: queda para inspección
///   o rescate manual) y se devuelve 0.
pub fn restore() -> usize {
    let ruta = queue_path();
    let txt = match std::fs::read_to_string(&ruta) {
        Ok(t) => t,
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                eprintln!("[task_scheduler] no se pudo leer la cola {ruta}: {e}");
            }
            return 0;
        }
    };
    let v: Value = match serde_json::from_str(&txt) {
        Ok(v) => v,
        Err(e) => {
            let copia = format!("{ruta}.corrupto");
            let _ = std::fs::rename(&ruta, &copia);
            eprintln!(
                "[task_scheduler] la cola {ruta} está CORRUPTA ({e}); se conservó en {copia}"
            );
            return 0;
        }
    };
    let Some(items) = v["tasks"].as_array() else {
        eprintln!("[task_scheduler] la cola {ruta} no tiene la lista 'tasks'");
        return 0;
    };
    let mut q = queue().lock().unwrap_or_else(|p| p.into_inner());
    let mut n = 0;
    for it in items {
        let state = State::from_str(it["state"].as_str().unwrap_or("pending"));
        // Una tarea que quedó "running" al morir el proceso vuelve a pendiente.
        let state = if state == State::Running { State::Pending } else { state };
        q.push(Task {
            id: it["id"].as_str().unwrap_or("").to_string(),
            kind: it["kind"].as_str().unwrap_or("").to_string(),
            priority: it["priority"].as_u64().unwrap_or(0) as u8,
            payload: it["payload"].clone(),
            state,
            attempts: it["attempts"].as_u64().unwrap_or(0) as u32,
            max_attempts: it["max_attempts"].as_u64().unwrap_or(2) as u32,
            deps: it["deps"]
                .as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default(),
            res: Resources::default(),
            created_at: it["created_at"].as_u64().unwrap_or(0),
            started_at: it["started_at"].as_u64(),
            finished_at: it["finished_at"].as_u64(),
            error: it["error"].as_str().map(|s| s.to_string()),
            result: None,
            node: it["node"].as_str().unwrap_or("").to_string(),
            resume_after: it["resume_after"].as_u64(),
            lease_expira: it["lease_expira"].as_u64(),
            motivo_pausa: it["motivo_pausa"].as_str().map(|s| s.to_string()),
        });
        n += 1;
    }
    n
}

/// Exporta las tareas pendientes (reparto entre nodos).
pub fn export_tasks(node: &str) -> Value {
    let q = queue().lock().unwrap_or_else(|p| p.into_inner());
    let tasks: Vec<Value> = q
        .iter()
        .filter(|t| t.state == State::Pending)
        .map(|t| {
            json!({
                "id": t.id, "kind": t.kind, "priority": t.priority,
                "payload": t.payload, "deps": t.deps,
                "node": node,
            })
        })
        .collect();
    json!({ "ok": true, "node": node, "count": tasks.len(), "tasks": tasks })
}

/// Exporta **todas** las tareas con su estado (para sincronizar nodos).
///
/// `export_tasks` sólo saca las `pending`: para sincronizar hace falta el
/// estado completo (running, paused, done...).
pub fn export_all_tasks(node: &str) -> Value {
    let q = queue().lock().unwrap_or_else(|p| p.into_inner());
    let tasks: Vec<Value> = q
        .iter()
        .map(|t| {
            json!({
                "id": t.id, "kind": t.kind, "state": t.state.as_str(),
                "priority": t.priority, "payload": t.payload,
                "attempts": t.attempts, "max_attempts": t.max_attempts,
                "deps": t.deps, "node": t.node, "error": t.error,
                "resume_after": t.resume_after, "lease_expira": t.lease_expira,
                "motivo_pausa": t.motivo_pausa,
                "created_at": t.created_at, "started_at": t.started_at,
                "finished_at": t.finished_at,
            })
        })
        .collect();
    json!({ "ok": true, "node": node, "count": tasks.len(), "tasks": tasks })
}

/// Cuanto "avanzado" está un estado (para la fusión).
fn rango_estado(s: State) -> u8 {
    match s {
        State::Done => 5,
        State::Running => 4,
        State::Paused => 3,
        State::Pending => 2,
        State::Failed => 1,
        State::Cancelled => 0,
    }
}

/// Fusiona las tareas de otro nodo con las locales **por id**.
///
/// Regla: gana el estado más avanzado (done > running > paused > pending).
/// Así, un nodo que acaba de terminar una tarea no la re-ejecuta otro, y
/// una tarea que el otro nodo tiene en curso no se duplica aquí.
/// Devuelve (insertadas, actualizadas).
pub fn merge_tasks(v: &Value) -> (usize, usize) {
    let Some(items) = v["tasks"].as_array() else {
        return (0, 0);
    };
    let mut q = queue().lock().unwrap_or_else(|p| p.into_inner());
    let mut ins = 0;
    let mut upd = 0;
    for it in items {
        let id = it["id"].as_str().unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let remoto = State::from_str(it["state"].as_str().unwrap_or("pending"));
        match q.iter_mut().find(|t| t.id == id) {
            None => {
                q.push(Task {
                    id,
                    kind: it["kind"].as_str().unwrap_or("").to_string(),
                    priority: it["priority"].as_u64().unwrap_or(0) as u8,
                    payload: it["payload"].clone(),
                    state: remoto,
                    attempts: it["attempts"].as_u64().unwrap_or(0) as u32,
                    max_attempts: it["max_attempts"].as_u64().unwrap_or(2) as u32,
                    deps: it["deps"]
                        .as_array()
                        .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
                        .unwrap_or_default(),
                    res: Resources::default(),
                    created_at: it["created_at"].as_u64().unwrap_or_else(now),
                    started_at: it["started_at"].as_u64(),
                    finished_at: it["finished_at"].as_u64(),
                    error: it["error"].as_str().map(|s| s.to_string()),
                    result: None,
                    node: it["node"].as_str().unwrap_or("").to_string(),
                    resume_after: it["resume_after"].as_u64(),
                    lease_expira: it["lease_expira"].as_u64(),
                    motivo_pausa: it["motivo_pausa"].as_str().map(|s| s.to_string()),
                });
                ins += 1;
            }
            Some(local) => {
                if rango_estado(remoto) > rango_estado(local.state) {
                    local.state = remoto;
                    local.node = it["node"].as_str().unwrap_or("").to_string();
                    local.resume_after = it["resume_after"].as_u64();
                    local.lease_expira = it["lease_expira"].as_u64();
                    local.motivo_pausa = it["motivo_pausa"].as_str().map(|s| s.to_string());
                    local.error = it["error"].as_str().map(|s| s.to_string());
                    local.finished_at = it["finished_at"].as_u64();
                    upd += 1;
                }
            }
        }
    }
    drop(q);
    if ins + upd > 0 {
        persist();
    }
    (ins, upd)
}

/// Importa tareas desde otro nodo (fusión por id).
pub fn import_tasks(v: &Value) -> usize {
    let Some(items) = v["tasks"].as_array() else {
        return 0;
    };
    let mut q = queue().lock().unwrap_or_else(|p| p.into_inner());
    let mut n = 0;
    for it in items {
        let id = it["id"].as_str().unwrap_or("").to_string();
        if id.is_empty() || q.iter().any(|t| t.id == id) {
            continue;
        }
        q.push(Task {
            id,
            kind: it["kind"].as_str().unwrap_or("").to_string(),
            priority: it["priority"].as_u64().unwrap_or(0) as u8,
            payload: it["payload"].clone(),
            state: State::Pending,
            attempts: 0,
            max_attempts: 2,
            deps: it["deps"]
                .as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default(),
            res: Resources::default(),
            created_at: now(),
            started_at: None,
            finished_at: None,
            error: None,
            result: None,
            node: it["node"].as_str().unwrap_or("").to_string(),
            resume_after: it["resume_after"].as_u64(),
            lease_expira: it["lease_expira"].as_u64(),
            motivo_pausa: it["motivo_pausa"].as_str().map(|s| s.to_string()),
        });
        n += 1;
    }
    drop(q);
    persist();
    n
}

/// Estadísticas de la cola.
pub fn stats() -> Value {
    let q = queue().lock().unwrap_or_else(|p| p.into_inner());
    let mut by_state = serde_json::Map::new();
    let mut by_kind = serde_json::Map::new();
    for t in q.iter() {
        let e = by_state
            .entry(t.state.as_str().to_string())
            .or_insert(json!(0));
        *e = json!(e.as_u64().unwrap_or(0) + 1);
        let k = by_kind.entry(t.kind.clone()).or_insert(json!(0));
        *k = json!(k.as_u64().unwrap_or(0) + 1);
    }
    let j = journal().lock().unwrap_or_else(|p| p.into_inner());
    json!({
        "ok": true,
        "tasks": q.len(),
        "by_state": Value::Object(by_state),
        "by_kind": Value::Object(by_kind),
        "journal_entries": j.len(),
        "queue_path": queue_path(),
    })
}

/// Lista las tareas en JSON.
pub fn list_json() -> Value {
    json!({
        "ok": true,
        "tasks": list().iter().map(|t| json!({
            "id": t.id, "kind": t.kind, "priority": t.priority,
            "state": t.state.as_str(), "attempts": t.attempts,
            "deps": t.deps, "created_at": t.created_at,
            "started_at": t.started_at, "finished_at": t.finished_at,
            "error": t.error, "node": t.node,
        })).collect::<Vec<Value>>(),
    })
}

/// Journal completo (auditoría de lo ejecutado).
pub fn journal_json(limit: usize) -> Value {
    let j = journal().lock().unwrap_or_else(|p| p.into_inner());
    let start = j.len().saturating_sub(limit);
    json!({ "ok": true, "entries": j[start..].to_vec() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encolar_y_listar() {
        let id = submit("test.kind", json!({ "x": 1 }), 5, vec![], Resources::default());
        assert!(!id.is_empty());
        let t = get(&id).expect("la tarea debe existir");
        assert_eq!(t.kind, "test.kind");
        assert_eq!(t.priority, 5);
        assert_eq!(t.state, State::Pending);
        cancel(&id);
        assert_eq!(get(&id).unwrap().state, State::Cancelled);
    }

    #[test]
    fn las_dependencias_bloquean_hasta_que_se_resuelven() {
        let a = submit("test.dep_a", json!({}), 5, vec![], Resources::default());
        let b = submit("test.dep_b", json!({}), 5, vec![a.clone()], Resources::default());
        // "b" depende de "a": no es elegible hasta que "a" esté hecha.
        let q = list();
        let tb = q.iter().find(|t| t.id == b).unwrap();
        assert!(!deps_ok(tb, &q));
        // Se marca "a" como hecha y entonces "b" sí queda satisfecha.
        {
            let mut q2 = queue().lock().unwrap_or_else(|p| p.into_inner());
            if let Some(t) = q2.iter_mut().find(|t| t.id == a) {
                t.state = State::Done;
            }
        }
        let q3 = list();
        let tb3 = q3.iter().find(|t| t.id == b).unwrap();
        assert!(deps_ok(tb3, &q3));
    }

    #[test]
    fn la_prioridad_manda_en_la_seleccion() {
        let low = submit("test.low", json!({}), 1, vec![], Resources::default());
        let high = submit("test.high", json!({}), 9, vec![], Resources::default());
        let q = list();
        let picked = pick(&q, true, 3).expect("debe elegir una");
        assert_eq!(picked, high);
        cancel(&low);
        cancel(&high);
    }

    #[test]
    fn las_tareas_pesadas_se_omiten_sin_presupuesto() {
        let id = submit("test.heavy", json!({}), 9, vec![], Resources { cpu: 3, net: false, heavy: true });
        let q = list();
        assert!(pick(&q, false, 3).is_none() || pick(&q, false, 3).unwrap() != id);
        cancel(&id);
    }

    #[test]
    fn exportar_e_importar_reparte_tareas() {
        let id = submit("test.export", json!({ "k": 1 }), 3, vec![], Resources::default());
        let exp = export_tasks("nodo-b");
        assert!(exp["tasks"].as_array().unwrap().iter().any(|t| t["id"] == json!(id)));
        // El nodo receptor no duplica ids ya conocidos.
        let n = import_tasks(&exp);
        assert_eq!(n, 0, "no debe reimportar tareas ya presentes");
        cancel(&id);
    }

    #[test]
    fn el_estado_usa_nombres_estables() {
        assert_eq!(State::Pending.as_str(), "pending");
        assert_eq!(State::from_str("failed"), State::Failed);
    }
}

// ---------------------------------------------------------------------------
// MIGRACIÓN Y PAUSA (2026-11-22)
//
// MEDIDO: los agentes de la flota se quedaron SIN CUOTA a mitad de un
// bloque y perdieron el trabajo que no estaba commiteado.  Con estas tres
// funciones, la cola no se pierde y el trabajo se REPARTE:
//
// * `pausar_por_cuota`: el nodo agota la cuota -> sus tareas quedan en
//   `paused` con `resume_after = reset_at` (no se cortan a mitad).
// * `migrar_tareas`: el nodo died/se quedo sin cuota -> sus tareas pasan a
//   otro nodo, que las retoma desde el checkpoint.
// * `expirar_leases`: el nodo murio sin devolver la tarea -> el lease
//   caduca y la tarea vuelve a la cola para que otro la coja.
// ---------------------------------------------------------------------------

/// Pausa las tareas de un nodo porque se agotó la cuota.
///
/// Devuelve cuántas quedaron en pausa.  `reset_at` es el epoch del reset;
/// si es `None`, la pausa es de 15 minutos por defecto.
pub fn pausar_por_cuota(node: &str, reset_at: Option<u64>, motivo: &str) -> usize {
    let mut q = queue().lock().unwrap_or_else(|p| p.into_inner());
    let hasta = reset_at.unwrap_or(now() + 900);
    let mut n = 0;
    for t in q.iter_mut() {
        if t.node == node && (t.state == State::Running || t.state == State::Pending) {
            t.state = State::Paused;
            t.resume_after = Some(hasta);
            t.motivo_pausa = Some(motivo.to_string());
            n += 1;
        }
    }
    drop(q);
    persist();
    n
}

/// Reasigna las tareas de un nodo a otro (handoff).
///
/// Útil cuando un agente falla, se queda sin cuota o se apaga: el trabajo
/// pasa a otro nodo SIN perderse.
pub fn migrar_tareas(desde: &str, hacia: &str) -> usize {
    let mut q = queue().lock().unwrap_or_else(|p| p.into_inner());
    let mut n = 0;
    for t in q.iter_mut() {
        if t.node == desde && matches!(t.state, State::Running | State::Pending | State::Paused) {
            t.node = hacia.to_string();
            t.state = State::Pending;
            t.resume_after = None;
            t.lease_expira = None;
            t.motivo_pausa = None;
            n += 1;
        }
    }
    drop(q);
    persist();
    n
}

/// Devuelve a la cola las tareas cuyo lease ha caducado.
///
/// Un nodo que muere con una tarea en curso NO la pierde: el lease caduca
/// y otro nodo puede cogerla.
pub fn expirar_leases() -> usize {
    let t0 = now();
    let mut q = queue().lock().unwrap_or_else(|p| p.into_inner());
    let mut n = 0;
    for t in q.iter_mut() {
        // MEDIDO (2026-11-23, auditoría de calidad): `expirar_leases` sólo
        // miraba las tareas EN CURSO (`Running`).  Una tarea REPARTIDA
        // (`repartir_por_nodo` la deja en `Pending` con `node` y
        // `lease_expira` puestos) cuyo nodo moría ANTES de empezarla se
        // quedaba apuntando a ese nodo para siempre:
        //   * `expirar_leases` no la tocaba (no estaba `Running`);
        //   * `repartir_por_nodo` sólo coge tareas con `node` VACÍO;
        //   => el trabajo NUNCA volvía a repartirse: la tarea se quedaba
        //      colgada para siempre (trabajo perdido en silencio).
        // Ahora una lease caducada libera el nodo SIEMPRE, esté la tarea en
        // curso o pendiente.
        if !t.lease_expira.map(|e| e <= t0).unwrap_or(false) {
            continue;
        }
        match t.state {
            State::Running => {
                t.state = State::Pending;
                t.node = String::new();
                t.lease_expira = None;
                t.motivo_pausa = Some("lease caducado (el nodo no devolvio la tarea)".to_string());
                n += 1;
            }
            State::Pending => {
                t.node = String::new();
                t.lease_expira = None;
                n += 1;
            }
            _ => {}
        }
    }
    drop(q);
    if n > 0 {
        persist();
    }
    n
}

/// Asigna un lease a las tareas que este nodo está ejecutando.
///
/// `segundos` = duración del lease; el nodo lo refresca mientras trabaja.
pub fn renovar_lease(nodo: &str, segundos: u64) -> usize {
    let mut q = queue().lock().unwrap_or_else(|p| p.into_inner());
    let mut n = 0;
    for t in q.iter_mut() {
        if t.node == nodo && t.state == State::Running {
            t.lease_expira = Some(now() + segundos);
            n += 1;
        }
    }
    drop(q);
    n
}

/// Reparte el trabajo entre nodos: cada nodo coge hasta `max` tareas
/// pendientes.  Devuelve los ids que se ha asignado.
pub fn repartir_por_nodo(nodo: &str, max: usize, lease_segundos: u64) -> Vec<String> {
    let mut q = queue().lock().unwrap_or_else(|p| p.into_inner());
    let mut out = Vec::new();
    for t in q.iter_mut() {
        if out.len() >= max {
            break;
        }
        if t.state == State::Pending && t.node.is_empty() {
            t.node = nodo.to_string();
            t.lease_expira = Some(now() + lease_segundos);
            out.push(t.id.clone());
        }
    }
    drop(q);
    if !out.is_empty() {
        persist();
    }
    out
}

#[cfg(test)]
mod tests_migracion {
    use super::*;

    /// El objetivo: si un nodo se queda sin cuota o muere, la tarea NO se
    /// pierde y otro nodo puede cogerla.
    #[test]
    fn pausar_por_cuota_deja_las_tareas_para_el_reset() {
        let id = submit("test-cuota", json!({"x": 1}), 1, vec![], Resources::default());
        {
            let mut q = queue().lock().unwrap_or_else(|p| p.into_inner());
            if let Some(t) = q.iter_mut().find(|t| t.id == id) {
                t.node = "nodo-a".to_string();
                t.state = State::Running;
            }
        }
        let n = pausar_por_cuota("nodo-a", Some(now() + 3600), "cuota agotada");
        assert_eq!(n, 1);
        let t = get(&id).unwrap();
        assert_eq!(t.state, State::Paused);
        assert!(t.resume_after.unwrap() > now());
        assert_eq!(t.motivo_pausa.as_deref(), Some("cuota agotada"));
    }

    #[test]
    fn migrar_tareas_mueve_el_trabajo_a_otro_nodo() {
        let id = submit("test-migra", json!({}), 1, vec![], Resources::default());
        {
            let mut q = queue().lock().unwrap_or_else(|p| p.into_inner());
            if let Some(t) = q.iter_mut().find(|t| t.id == id) {
                t.node = "nodo-muerto".to_string();
                t.state = State::Running;
            }
        }
        let n = migrar_tareas("nodo-muerto", "nodo-vivo");
        assert_eq!(n, 1);
        let t = get(&id).unwrap();
        assert_eq!(t.node, "nodo-vivo");
        assert_eq!(t.state, State::Pending);
    }

    #[test]
    fn un_lease_caducado_devuelve_la_tarea_a_la_cola() {
        let id = submit("test-lease", json!({}), 1, vec![], Resources::default());
        {
            let mut q = queue().lock().unwrap_or_else(|p| p.into_inner());
            if let Some(t) = q.iter_mut().find(|t| t.id == id) {
                t.node = "nodo-muerto".to_string();
                t.state = State::Running;
                t.lease_expira = Some(now() - 10);
            }
        }
        assert_eq!(expirar_leases(), 1);
        let t = get(&id).unwrap();
        assert_eq!(t.state, State::Pending);
        assert!(t.node.is_empty());
    }

    #[test]
    fn una_tarea_pausada_no_se_ejecuta_hasta_el_reset() {
        let id = submit("test-pausa", json!({}), 1, vec![], Resources::default());
        {
            let mut q = queue().lock().unwrap_or_else(|p| p.into_inner());
            if let Some(t) = q.iter_mut().find(|t| t.id == id) {
                t.state = State::Paused;
                t.resume_after = Some(now() + 3600);
            }
        }
        let ids = repartir_por_nodo("nodo-x", 10, 60);
        assert!(!ids.contains(&id), "una tarea en pausa no se reparte");
    }
}
