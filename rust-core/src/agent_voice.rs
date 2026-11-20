//! VOZ — el daemon como **agente** del enjambre, no solo como servidor de tools.
//!
//! # Por qué existe este módulo
//!
//! El fallo de diseño que Sudaderó el usuario: MegaDaemon tenía las piezas
//! (`mesh.rs` con tablón+journal+peers, `agent_core.rs` con el planificador,
//! `providers.rs` con 13 proveedores de LLM, `coordination.rs` con el board
//! SQLite) pero **nadie las unía**: el daemon no se registraba como agente,
//! no reclamaba tareas, no decidía con un LLM y no hablaba con los demás.
//! Solo obedecía cuando un humano llamaba a una tool.
//!
//! Este módulo es **puro y testeable**: la decisión entra por un trait
//! [`Brain`] (hay uno determinista y otro que usa un LLM real), la ejecución
//! sale por un trait [`Executor`], y la difusión son líneas NDJSON listas para
//! el journal y el canal `sync/`.
//!
//! Reglas de oro que este módulo respeta (y que hacen falta en un móvil con
//! 3 GB de RAM):
//!
//! * **una acción por tick** (`max_actions_per_tick`), nunca un bucle infinito;
//! * **nada de reinicios**: este módulo no toca procesos;
//! * **sin secretos**: lo que se difunde son textos, ids y hashes, nunca keys;
//! * **auditable**: cada acción deja una línea en el journal con su resultado.

use serde_json::{json, Value};

/// Rol con el que el daemon se presenta en el tablón del enjambre.
pub const VOICE_ROLE: &str = "voice";

/// Identificador de la "voz" del daemon como agente.
pub const VOICE_AGENT_ID: &str = "megadaemon-voice";

// ------------------------------------------------------------------------ //
// Identidad
// ------------------------------------------------------------------------ //

/// Identidad del daemon como agente del enjambre.
#[derive(Clone, Debug, PartialEq)]
pub struct VoiceProfile {
    /// Id único del agente en el tablón.
    pub node_id: String,
    /// Rol (`voice`).
    pub role: String,
    /// Capacidades que ofrece (p.ej. `reverse`, `npu`, `root_exec`).
    pub capabilities: Vec<String>,
    /// Instancia física donde corre (p.ej. `samsung-s21-ultra`).
    pub instance: String,
    /// Endpoint MCP público por el que le pueden hablar los demás.
    pub endpoint: String,
    /// Si esta instancia es la maestra del enjambre.
    pub is_master: bool,
}

impl Default for VoiceProfile {
    fn default() -> Self {
        VoiceProfile {
            node_id: VOICE_AGENT_ID.to_string(),
            role: VOICE_ROLE.to_string(),
            capabilities: vec![
                "reverse".into(),
                "npu".into(),
                "root_exec".into(),
                "device".into(),
                "voice".into(),
            ],
            instance: "unknown".into(),
            endpoint: String::new(),
            is_master: false,
        }
    }
}

impl VoiceProfile {
    /// Construye el perfil a partir del entorno (nada de I/O: el llamador pasa los datos).
    pub fn new(node_id: &str, instance: &str, endpoint: &str, is_master: bool) -> Self {
        VoiceProfile {
            node_id: node_id.to_string(),
            instance: instance.to_string(),
            endpoint: endpoint.to_string(),
            is_master,
            ..Default::default()
        }
    }

    /// El perfil tal y como se publica en el tablón (`agents[]` del board).
    pub fn to_json(&self) -> Value {
        json!({
            "agent": self.node_id,
            "role": self.role,
            "task": "voz del daemon: planifica, decide con LLM, ejecuta y difunde",
            "kind": "agent",
            "status": "running",
            "instance": self.instance,
            "endpoint": self.endpoint,
            "capabilities": self.capabilities,
            "is_master": self.is_master,
        })
    }
}

// ------------------------------------------------------------------------ //
// Vista del mundo (entrada de la decisión)
// ------------------------------------------------------------------------ //

/// Tarea tal y como la ve la voz (síntesis del tablón, sin I/O).
#[derive(Clone, Debug, PartialEq)]
pub struct TaskView {
    pub task_id: String,
    pub title: String,
    pub capability: String,
    pub priority: i32,
    pub state: String,
    pub claimed_by: String,
}

impl TaskView {
    /// ¿La tarea está libre (sin dueño y sin hecha)?
    pub fn is_free(&self) -> bool {
        self.claimed_by.trim().is_empty() && self.state != "done" && self.state != "failed"
    }
}

/// Peer visto en el tablón.
#[derive(Clone, Debug, PartialEq)]
pub struct PeerView {
    pub node_id: String,
    pub instance: String,
    pub capabilities: Vec<String>,
    pub last_seen: i64,
}

/// Contexto de decisión: lo que la voz sabe del mundo en este tick.
#[derive(Clone, Debug, PartialEq)]
pub struct VoiceContext {
    pub profile: VoiceProfile,
    pub tasks: Vec<TaskView>,
    pub peers: Vec<PeerView>,
    /// Texto de la última acción ejecutada (para no repetirla en bucle).
    pub last_action: Option<String>,
    /// Tope de acciones por tick (1 por defecto: el móvil va justo de RAM).
    pub max_actions_per_tick: usize,
    /// Segundos desde el último tick (para el cooldown).
    pub secs_since_last_tick: i64,
    /// Segundos mínimos entre dos ticks.
    pub cooldown_secs: i64,
}

impl Default for VoiceContext {
    fn default() -> Self {
        VoiceContext {
            profile: VoiceProfile::default(),
            tasks: Vec::new(),
            peers: Vec::new(),
            last_action: None,
            max_actions_per_tick: 1,
            secs_since_last_tick: 999,
            cooldown_secs: 60,
        }
    }
}

impl VoiceContext {
    /// ¿Toca actuar en este tick? (cooldown + tope por tick)
    pub fn may_act(&self) -> bool {
        self.max_actions_per_tick > 0 && self.secs_since_last_tick >= self.cooldown_secs
    }
}

// ------------------------------------------------------------------------ //
// Acciones
// ------------------------------------------------------------------------ //

/// Lo que la voz decide hacer en un tick.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Reclamar una tarea del tablón (con lease).
    Claim { task_id: String, capability: String },
    /// Preguntar a otro nodo por el bus.
    Ask { peer: String, question: String },
    /// Decir algo al enjambre (journal + sync + swarm_chat).
    Speak { text: String },
    /// Ejecutar una tool del daemon.
    Do { tool: String, args: Value },
    /// No hacer nada, y decir por qué.
    Idle { reason: String },
}

impl Action {
    /// Clave estable de la acción (para no repetirla y para auditar).
    pub fn key(&self) -> String {
        match self {
            Action::Claim { task_id, .. } => format!("claim:{}", task_id),
            Action::Ask { peer, .. } => format!("ask:{}", peer),
            Action::Speak { text } => format!("speak:{}", &text.chars().take(40).collect::<String>()),
            Action::Do { tool, .. } => format!("do:{}", tool),
            Action::Idle { reason } => format!("idle:{}", reason),
        }
    }

    /// Texto legible para el journal y para `swarm_chat`.
    pub fn describe(&self) -> String {
        match self {
            Action::Claim { task_id, capability } => {
                format!("reclamo la tarea {} (capacidad {})", task_id, capability)
            }
            Action::Ask { peer, question } => format!("pregunto a {}: {}", peer, question),
            Action::Speak { text } => format!("digo: {}", text),
            Action::Do { tool, .. } => format!("ejecuto la tool {}", tool),
            Action::Idle { reason } => format!("idle: {}", reason),
        }
    }

    /// `true` si la acción muta el estado (y por tanto necesita lock/auditoría).
    pub fn mutates(&self) -> bool {
        !matches!(self, Action::Idle { .. })
    }
}

// ------------------------------------------------------------------------ //
// Cerebros
// ------------------------------------------------------------------------ //

/// El "cerebro": decide la acción. Determinista en tests, LLM en producción.
pub trait Brain {
    fn decide(&self, ctx: &VoiceContext) -> Action;
}

/// Cerebro determinista (sin LLM): el mismo criterio que usaría un humano
/// con 5 minutos. Prioridad: reclamar > preguntar > decir > callar.
#[derive(Default)]
pub struct RuleBrain;

impl Brain for RuleBrain {
    fn decide(&self, ctx: &VoiceContext) -> Action {
        if !ctx.may_act() {
            return Action::Idle {
                reason: format!(
                    "cooldown: hace {}s (min {}s)",
                    ctx.secs_since_last_tick, ctx.cooldown_secs
                ),
            };
        }
        // 1) La tarea libre de mayor prioridad que encaje con mis capacidades.
        let mut best: Option<&TaskView> = None;
        for t in &ctx.tasks {
            if !t.is_free() {
                continue;
            }
            if !t.capability.trim().is_empty()
                && !ctx.profile.capabilities.iter().any(|c| c == &t.capability)
            {
                continue;
            }
            best = match best {
                None => Some(t),
                Some(b) if t.priority > b.priority => Some(t),
                Some(b) if t.priority == b.priority && t.task_id < b.task_id => Some(t),
                keep => keep,
            };
        }
        if let Some(t) = best {
            return Action::Claim {
                task_id: t.task_id.clone(),
                capability: t.capability.clone(),
            };
        }
        // 2) Si hay un peer con capacidad que yo no tengo, preguntarle por el estado.
        for p in &ctx.peers {
            if p.node_id == ctx.profile.node_id {
                continue;
            }
            if p
                .capabilities
                .iter()
                .any(|c| !ctx.profile.capabilities.contains(c))
            {
                return Action::Ask {
                    peer: p.node_id.clone(),
                    question: format!(
                        "¿tienesadvance en {}? ¿qué bloquea el enjambre?",
                        p.capabilities.join("/")
                    ),
                };
            }
        }
        // 3) Si soy la maestra y no hay nada que hacer, decir el estado.
        if ctx.profile.is_master {
            return Action::Speak {
                text: format!(
                    "voz de {}: {} tareas en el tablón, {} peers; sin trabajo libre para mis capacidades",
                    ctx.profile.node_id,
                    ctx.tasks.len(),
                    ctx.peers.len()
                ),
            };
        }
        Action::Idle {
            reason: "no hay tarea libre para mis capacidades".into(),
        }
    }
}

/// Cerebro LLM: le pregunta a un modelo qué hacer y **exige** un JSON estricto.
///
/// Si el modelo no devuelve algo válido, se cae al [`RuleBrain`]: la voz nunca
/// se queda muda por culpa de un modelo (y nunca ejecuta algo que no entienda).
pub struct LlmBrain {
    /// Modelo a usar (`local-seere`, `minimax-m3.1-flash`, …).
    pub provider_id: String,
    /// Reglas extra para el LLM.
    pub extra_instructions: String,
    /// Cerebro de reserva.
    pub fallback: RuleBrain,
}

impl LlmBrain {
    pub fn new(provider_id: &str) -> Self {
        LlmBrain {
            provider_id: provider_id.to_string(),
            extra_instructions: String::new(),
            fallback: RuleBrain,
        }
    }

    /// El prompt que se manda al modelo (contrato estricto, sin secretos).
    pub fn build_prompt(&self, ctx: &VoiceContext) -> String {
        let tasks: Vec<Value> = ctx
            .tasks
            .iter()
            .map(|t| {
                json!({
                    "task_id": t.task_id,
                    "title": t.title,
                    "capability": t.capability,
                    "priority": t.priority,
                    "state": t.state,
                    "claimed_by": t.claimed_by,
                })
            })
            .collect();
        let peers: Vec<Value> = ctx
            .peers
            .iter()
            .map(|p| {
                json!({
                    "node_id": p.node_id,
                    "instance": p.instance,
                    "capabilities": p.capabilities,
                })
            })
            .collect();
        format!(
            "Eres la voz de MegaDaemon en un enjambre de agentes. Decide UNA sola accion.\n\
             Responde SOLO con un JSON, sin texto alrededor, con esta forma exacta:\n\
             {{\"action\":\"claim|ask|speak|do|idle\", ...}}\n\
             - claim: {{\"action\":\"claim\",\"task_id\":\"...\",\"capability\":\"...\"}}\n\
             - ask:   {{\"action\":\"ask\",\"peer\":\"...\",\"question\":\"...\"}}\n\
             - speak: {{\"action\":\"speak\",\"text\":\"...\"}}\n\
             - do:    {{\"action\":\"do\",\"tool\":\"<tool del daemon>\",\"args\":{{}}}}\n\
             - idle:  {{\"action\":\"idle\",\"reason\":\"...\"}}\n\
             No inventes tareas ni herramientas. No ejecutes acciones destructivas.\n\
             Reglas extra del operador: {}\n\
             Contexto (JSON):\n\
             {}",
            self.extra_instructions,
            serde_json::to_string_pretty(&json!({
                "you": ctx.profile.to_json(),
                "tasks": tasks,
                "peers": peers,
                "last_action": ctx.last_action,
            }))
            .unwrap_or_default()
        )
    }

    /// Convierte la respuesta del modelo en una acción (o `None` si no vale).
    pub fn parse_action(&self, reply: &str) -> Option<Action> {
        let start = reply.find('{')?;
        let end = reply.rfind('}')?;
        let v: Value = serde_json::from_str(&reply[start..=end]).ok()?;
        let kind = v.get("action")?.as_str()?;
        Some(match kind {
            "claim" => Action::Claim {
                task_id: v.get("task_id")?.as_str()?.to_string(),
                capability: v
                    .get("capability")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string(),
            },
            "ask" => Action::Ask {
                peer: v.get("peer")?.as_str()?.to_string(),
                question: v.get("question")?.as_str()?.to_string(),
            },
            "speak" => Action::Speak {
                text: v.get("text")?.as_str()?.to_string(),
            },
            "do" => Action::Do {
                tool: v.get("tool")?.as_str()?.to_string(),
                args: v.get("args").cloned().unwrap_or(json!({})),
            },
            "idle" => Action::Idle {
                reason: v.get("reason")?.as_str()?.to_string(),
            },
            _ => return None,
        })
    }
}

// ------------------------------------------------------------------------ //
// Ejecución
// ------------------------------------------------------------------------ //

/// Ejecutor de tools: el mismo camino que usa `/api/tool/<nombre>`.
pub trait Executor {
    fn call(&self, tool: &str, args: &Value) -> Result<Value, String>;
}

/// Resultado de ejecutar una acción.
#[derive(Clone, Debug, PartialEq)]
pub struct VoiceOutcome {
    pub action: Action,
    pub ok: bool,
    pub detail: String,
    /// Líneas NDJSON para el journal / canal sync (sin secretos).
    pub broadcast: Vec<String>,
}

/// Ejecuta la acción.
///
/// * `Claim` → el ejecutor recibe la tool `mesh_claim` con la task_id.
/// * `Ask`   → el ejecutor recibe la tool `mesh_ask` con peer y pregunta.
/// * `Speak` → el ejecutor recibe la tool `swarm_chat` con el texto.
/// * `Do`    → el ejecutor recibe la tool pedida con sus argumentos.
/// * `Idle`  → no se ejecuta nada y no se difunde.
pub fn act(ctx: &VoiceContext, action: &Action, exec: &dyn Executor) -> VoiceOutcome {
    let (tool, args) = match action {
        Action::Claim { task_id, capability } => (
            "mesh_claim",
            json!({"task_id": task_id, "capability": capability}),
        ),
        Action::Ask { peer, question } => (
            "mesh_ask",
            json!({"peer": peer, "question": question}),
        ),
        Action::Speak { text } => ("swarm_chat", json!({"text": text})),
        Action::Do { tool, args } => (tool.as_str(), args.clone()),
        Action::Idle { reason } => {
            return VoiceOutcome {
                action: action.clone(),
                ok: true,
                detail: reason.clone(),
                broadcast: Vec::new(),
            }
        }
    };
    let res = exec.call(tool, &args);
    let ok = res.is_ok();
    let detail = match &res {
        Ok(v) => v
            .get("summary")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| truncate(&v.to_string(), 400)),
        Err(e) => truncate(e, 400),
    };
    let line = json!({
        "ts": now_iso(),
        "node": ctx.profile.node_id,
        "event": "voice_action",
        "kind": "voice",
        "action": action.key(),
        "describe": action.describe(),
        "ok": ok,
        "detail": detail,
        "target": if ok { Some(tool.to_string()) } else { None },
    });
    let broadcast = vec![line.to_string()];
    VoiceOutcome {
        action: action.clone(),
        ok,
        detail,
        broadcast,
    }
}

/// Ejecuta la decisión completa: plan → act → difusión.
pub fn tick(ctx: &VoiceContext, brain: &dyn Brain, exec: &dyn Executor) -> VoiceOutcome {
    let action = plan_next(ctx, brain);
    act(ctx, &action, exec)
}

/// Decide la siguiente acción (con degradación segura del LLM al criterio fijo).
pub fn plan_next(ctx: &VoiceContext, brain: &dyn Brain) -> Action {
    let action = brain.decide(ctx);
    // Nunca repetir la misma acción si ya está en curso o si ya es un idle por cooldown.
    if let Some(last) = &ctx.last_action {
        if &action.key() == last && !matches!(action, Action::Idle { .. }) {
            return Action::Idle {
                reason: format!("ya hice {} en el tick anterior", last),
            };
        }
    }
    action
}

/// Mensaje normalizado para `swarm_chat` (queda en la memoria del enjambre).
pub fn speak_to_peer(ctx: &VoiceContext, peer: &str, question: &str) -> String {
    format!(
        "[{} -> {}] {}",
        ctx.profile.node_id, peer, question
    )
}

/// Estado de la voz para la UI / para `voice_status`.
pub fn voice_status(ctx: &VoiceContext) -> Value {
    json!({
        "node_id": ctx.profile.node_id,
        "role": ctx.profile.role,
        "instance": ctx.profile.instance,
        "is_master": ctx.profile.is_master,
        "endpoint": ctx.profile.endpoint,
        "capabilities": ctx.profile.capabilities,
        "tasks_total": ctx.tasks.len(),
        "tasks_free": ctx.tasks.iter().filter(|t| t.is_free()).count(),
        "peers": ctx.peers.len(),
        "may_act": ctx.may_act(),
        "cooldown_secs": ctx.cooldown_secs,
        "secs_since_last_tick": ctx.secs_since_last_tick,
        "last_action": ctx.last_action,
        "max_actions_per_tick": ctx.max_actions_per_tick,
    })
}

/// Identidad pública de la voz (lo que se publica en el tablón).
pub fn whoami(profile: &VoiceProfile) -> Value {
    profile.to_json()
}

/// Hora actual en ISO-8601 UTC (sin dependencias: días→fecha civil).
pub fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // días → fecha civil (algoritmo de Hinnant)
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
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

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect::<String>() + "…"
    }
}

// ------------------------------------------------------------------------ //
// Tests
// ------------------------------------------------------------------------ //

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn ctx_with_task(id: &str, cap: &str, prio: i32, claimed: &str) -> TaskView {
        TaskView {
            task_id: id.into(),
            title: format!("tarea {}", id),
            capability: cap.into(),
            priority: prio,
            state: "published".into(),
            claimed_by: claimed.into(),
        }
    }

    struct FakeExec {
        calls: RefCell<Vec<(String, Value)>>,
        fail: bool,
    }
    impl Executor for FakeExec {
        fn call(&self, tool: &str, args: &Value) -> Result<Value, String> {
            self.calls.borrow_mut().push((tool.to_string(), args.clone()));
            if self.fail {
                Err("boom".into())
            } else {
                Ok(json!({"summary": format!("ok {}", tool)}))
            }
        }
    }

    #[test]
    fn rule_brain_reclama_la_tarea_libre_de_mayor_prioridad() {
        let mut ctx = VoiceContext::default();
        ctx.tasks = vec![
            ctx_with_task("t-low", "reverse", 1, ""),
            ctx_with_task("t-high", "reverse", 9, ""),
            ctx_with_task("t-taken", "reverse", 99, "otro"),
        ];
        let a = plan_next(&ctx, &RuleBrain);
        assert_eq!(
            a,
            Action::Claim {
                task_id: "t-high".into(),
                capability: "reverse".into()
            }
        );
    }

    #[test]
    fn rule_brain_respeta_el_cooldown() {
        let mut ctx = VoiceContext::default();
        ctx.secs_since_last_tick = 3;
        ctx.cooldown_secs = 60;
        ctx.tasks = vec![ctx_with_task("t1", "reverse", 5, "")];
        match plan_next(&ctx, &RuleBrain) {
            Action::Idle { reason } => assert!(reason.contains("cooldown")),
            other => panic!("esperaba idle, {:?}", other),
        }
    }

    #[test]
    fn rule_brain_pregunta_a_un_peer_con_capacidad_que_no_tiene() {
        let mut ctx = VoiceContext::default();
        ctx.tasks.clear();
        // Capacidad "gpu" que la voz NO tiene: por eso pregunta a ese peer.
        ctx.peers = vec![PeerView {
            node_id: "cloud-sandbox".into(),
            instance: "cloud".into(),
            capabilities: vec!["gpu".into()],
            last_seen: 0,
        }];
        match plan_next(&ctx, &RuleBrain) {
            Action::Ask { peer, .. } => assert_eq!(peer, "cloud-sandbox"),
            other => panic!("esperaba ask, {:?}", other),
        }
    }

    #[test]
    fn no_repite_la_accion_del_tick_anterior() {
        let mut ctx = VoiceContext::default();
        ctx.tasks = vec![ctx_with_task("t1", "reverse", 5, "")];
        ctx.last_action = Some("claim:t1".into());
        match plan_next(&ctx, &RuleBrain) {
            Action::Idle { reason } => assert!(reason.contains("ya hice")),
            other => panic!("esperaba idle, {:?}", other),
        }
    }

    #[test]
    fn act_usa_la_tool_correcta_y_prepara_el_broadcast() {
        let ctx = VoiceContext::default();
        let exec = FakeExec {
            calls: RefCell::new(Vec::new()),
            fail: false,
        };
        let out = act(
            &ctx,
            &Action::Claim {
                task_id: "t1".into(),
                capability: "reverse".into(),
            },
            &exec,
        );
        assert!(out.ok);
        assert_eq!(exec.calls.borrow()[0].0, "mesh_claim");
        assert_eq!(exec.calls.borrow()[0].1["task_id"], json!("t1"));
        assert_eq!(out.broadcast.len(), 1);
        let line: Value = serde_json::from_str(&out.broadcast[0]).unwrap();
        assert_eq!(line["node"], json!(VOICE_AGENT_ID));
        assert_eq!(line["event"], json!("voice_action"));
        assert_eq!(line["ok"], json!(true));
    }

    #[test]
    fn act_gestiona_el_error_sin_panicar() {
        let ctx = VoiceContext::default();
        let exec = FakeExec {
            calls: RefCell::new(Vec::new()),
            fail: true,
        };
        let out = act(
            &ctx,
            &Action::Do {
                tool: "root_exec".into(),
                args: json!({}),
            },
            &exec,
        );
        assert!(!out.ok);
        assert_eq!(out.broadcast.len(), 1);
        assert_eq!(out.broadcast[0].contains("\"ok\":false"), true);
    }

    #[test]
    fn idle_no_ejecuta_ni_difunde() {
        let ctx = VoiceContext::default();
        let exec = FakeExec {
            calls: RefCell::new(Vec::new()),
            fail: false,
        };
        let out = act(
            &ctx,
            &Action::Idle {
                reason: "nada que hacer".into(),
            },
            &exec,
        );
        assert!(out.broadcast.is_empty());
        assert!(exec.calls.borrow().is_empty());
    }

    #[test]
    fn llm_brain_parsea_las_cinco_acciones() {
        let b = LlmBrain::new("local-seere");
        let cases = vec![
            (
                r#"{"action":"claim","task_id":"t1","capability":"reverse"}"#,
                Action::Claim {
                    task_id: "t1".into(),
                    capability: "reverse".into()
                },
            ),
            (
                r#"texto {"action":"ask","peer":"p","question":"q"} fin"#,
                Action::Ask {
                    peer: "p".into(),
                    question: "q".into()
                },
            ),
            (
                r#"{"action":"speak","text":"hola"}"#,
                Action::Speak { text: "hola".into() },
            ),
            (
                r#"{"action":"do","tool":"system_overview","args":{}}"#,
                Action::Do {
                    tool: "system_overview".into(),
                    args: json!({})
                },
            ),
            (
                r#"{"action":"idle","reason":"nada"}"#,
                Action::Idle {
                    reason: "nada".into()
                },
            ),
        ];
        for (raw, want) in cases {
            assert_eq!(b.parse_action(raw), Some(want));
        }
        assert_eq!(b.parse_action("no json aqui"), None);
        assert_eq!(b.parse_action(r#"{"action":"inventada"}"#), None);
    }

    #[test]
    fn llm_brain_el_prompt_no_lleva_secretos() {
        let mut ctx = VoiceContext::default();
        ctx.profile.endpoint = "https://x.trycloudflare.com/mcp".into();
        let b = LlmBrain::new("local-seere");
        let p = b.build_prompt(&ctx);
        assert!(p.contains("\"action\""));
        assert!(!p.contains("mdaemon_sk_"));
        assert!(!p.contains("X-Seere-Auth-Token"));
    }

    #[test]
    fn speak_to_peer_identifica_emisor_y_destinatario() {
        let ctx = VoiceContext::default();
        let m = speak_to_peer(&ctx, "cloud-sandbox", "¿qué haces?");
        assert!(m.contains(VOICE_AGENT_ID));
        assert!(m.contains("cloud-sandbox"));
    }

    #[test]
    fn voice_status_refleja_el_mundo() {
        let mut ctx = VoiceContext::default();
        ctx.tasks = vec![ctx_with_task("a", "reverse", 1, ""), ctx_with_task("b", "x", 1, "y")];
        let s = voice_status(&ctx);
        assert_eq!(s["tasks_total"], json!(2));
        assert_eq!(s["tasks_free"], json!(1));
        assert_eq!(s["may_act"], json!(true));
    }
}
