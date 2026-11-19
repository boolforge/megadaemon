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
    Done,
    Failed,
    Cancelled,
}

impl State {
    pub fn as_str(&self) -> &'static str {
        match self {
            State::Pending => "pending",
            State::Running => "running",
            State::Done => "done",
            State::Failed => "failed",
            State::Cancelled => "cancelled",
        }
    }
    fn from_str(s: &str) -> State {
        match s {
            "running" => State::Running,
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
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn queue() -> &'static Mutex<Vec<Task>> {
    static Q: OnceLock<Mutex<Vec<Task>>> = OnceLock::new();
    Q.get_or_init(|| Mutex::new(Vec::new()))
}

fn journal() -> &'static Mutex<Vec<Value>> {
    static J: OnceLock<Mutex<Vec<Value>>> = OnceLock::new();
    J.get_or_init(|| Mutex::new(Vec::new()))
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
    let mut n = c.lock().unwrap();
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
    };
    let id = t.id.clone();
    queue().lock().unwrap().push(t);
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
    queue().lock().unwrap().clone()
}

/// Una tarea por id.
pub fn get(id: &str) -> Option<Task> {
    queue().lock().unwrap().iter().find(|t| t.id == id).cloned()
}

/// Cancela una tarea pendiente.
pub fn cancel(id: &str) -> bool {
    let mut q = queue().lock().unwrap();
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
    let mut q = queue().lock().unwrap();
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
    let mut best: Option<&Task> = None;
    for t in q {
        if t.state != State::Pending {
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

        let id = {
            let q = queue().lock().unwrap();
            pick(&q, allow_heavy, max_cpu)
        };
        let Some(id) = id else { break };

        // Marca en curso.
        {
            let mut q = queue().lock().unwrap();
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
            let mut q = queue().lock().unwrap();
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
    let mut j = journal().lock().unwrap();
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
    let q = queue().lock().unwrap();
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
            })
        })
        .collect();
    drop(q);
    let _ = std::fs::create_dir_all(crate::cockpit_server::state_dir());
    let _ = std::fs::write(
        queue_path(),
        serde_json::to_string_pretty(&json!({ "tasks": items })).unwrap_or_default(),
    );
}

/// Restaura la cola desde disco.
pub fn restore() -> usize {
    let Ok(txt) = std::fs::read_to_string(queue_path()) else {
        return 0;
    };
    let Ok(v) = serde_json::from_str::<Value>(&txt) else {
        return 0;
    };
    let Some(items) = v["tasks"].as_array() else {
        return 0;
    };
    let mut q = queue().lock().unwrap();
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
        });
        n += 1;
    }
    n
}

/// Exporta las tareas pendientes (reparto entre nodos).
pub fn export_tasks(node: &str) -> Value {
    let q = queue().lock().unwrap();
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

/// Importa tareas desde otro nodo (fusión por id).
pub fn import_tasks(v: &Value) -> usize {
    let Some(items) = v["tasks"].as_array() else {
        return 0;
    };
    let mut q = queue().lock().unwrap();
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
        });
        n += 1;
    }
    drop(q);
    persist();
    n
}

/// Estadísticas de la cola.
pub fn stats() -> Value {
    let q = queue().lock().unwrap();
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
    let j = journal().lock().unwrap();
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
    let j = journal().lock().unwrap();
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
            let mut q2 = queue().lock().unwrap();
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
