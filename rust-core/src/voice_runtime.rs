//! Cableado de la VOZ (W0) y del catálogo unificado (W6).
//!
//! `agent_voice.rs` (núcleo puro del agente de voz) decide QUÉ hacer;
//! este módulo lo enchufa con el MUNDO real:
//!
//! * **Contexto**: tareas del tablón de coordinación (`coordination.rs`) +
//!   peers del bus (`mesh.rs`).
//! * **Ejecutor**: las mismas tools que sirve `/api/tool/<n>`
//!   (`mcp_ext::dispatch`), con `mesh_claim` → `TaskBoard::claim_task`,
//!   `mesh_ask` → el buzón del bus (`webui::node_message`) y
//!   `swarm_chat` → `providers::execute_chat`.
//! * **Difusión**: los 4 canales — journal del bus, `sync/re_*.ndjson`,
//!   tablón y `swarm_chat`.
//! * **Catálogo unificado (W6)**: MODELOS + AGENTES/SERVICIOS con los campos
//!   `id, kind, endpoint, auth_kind, health, latency, cost_estimate,
//!   capabilities, enabled`.
//! * **Bucle autónomo opt-in** (`MEGADAEMON_VOICE=1`).
//!
//! Reglas duras (el móvil va con 3 GB de RAM y el usuario puede estar
//! usándolo):
//!
//! * **1 acción por tick**, con cooldown y presupuesto por hora.
//! * **Nunca** reinicios, despliegues ni procesos nuevos: el ejecutor veta
//!   las tools destructivas y jamás toca el supervisor.
//! * El prompt del LLM **no lleva** ni la API key ni el token; lo que se
//!   difunde son textos/ids/hashes (`redact()` limpia cualquier secreto que
//!   un modelo pueda haber colado en su respuesta).
//! * Nada inventado: si no se puede medir (latencia, coste, health) se
//!   devuelve `null` + el motivo, nunca un número de mentira.

use crate::agent_voice::{
    act, now_iso, plan_next, speak_to_peer, voice_status, whoami, Action, Brain, Executor,
    LlmBrain, PeerView, RuleBrain, TaskView, VoiceContext, VoiceOutcome, VoiceProfile,
};
use crate::{coordination, mesh, providers, webui};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

// ------------------------------------------------------------------------ //
// Configuración (todo por entorno, con valores prudentes por defecto)
// ------------------------------------------------------------------------ //

fn env_i64(name: &str, default: i64) -> i64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or(default)
}

/// ¿El bucle autónomo está activado? Opt-in explícito.
pub fn voice_enabled() -> bool {
    matches!(
        std::env::var("MEGADAEMON_VOICE").unwrap_or_default().trim(),
        "1" | "true" | "yes" | "on"
    )
}

/// Segundos entre ticks del bucle autónomo.
pub fn voice_interval_secs() -> i64 {
    env_i64("MEGADAEMON_VOICE_INTERVAL", 180).max(30)
}

/// Segundos mínimos entre dos acciones.
pub fn voice_cooldown_secs() -> i64 {
    env_i64("MEGADAEMON_VOICE_COOLDOWN", 60).max(0)
}

/// Máx. acciones por hora (presupuesto duro: el móvil no es una granja).
pub fn voice_budget_per_hour() -> i64 {
    env_i64("MEGADAEMON_VOICE_MAX_ACTIONS_PER_HOUR", 6).max(0)
}

/// Segundos de vida de un peer para considerarlo "vivo".
pub fn voice_peer_ttl_secs() -> i64 {
    env_i64("MEGADAEMON_VOICE_PEER_TTL", 900).max(30)
}

/// Lease con el que la voz reclama una tarea.
pub fn voice_lease_secs() -> i64 {
    env_i64("MEGADAEMON_VOICE_LEASE", 900).max(60)
}

// ------------------------------------------------------------------------ //
// Rutas
// ------------------------------------------------------------------------ //

/// Directorio de estado de la instancia (el mismo que usa el cockpit).
pub fn state_dir() -> String {
    crate::cockpit_server::state_dir()
}

/// Tablón de coordinación (el mismo fichero que `md-coord`).
pub fn board_path() -> String {
    format!("{}/board.sqlite", state_dir())
}

/// Estado persistente de la voz (última acción, presupuesto, tick).
pub fn voice_state_path() -> String {
    format!("{}/voice_state.json", state_dir())
}

/// Journal del bus (canal 1 de la difusión).
pub fn journal_path() -> String {
    mesh::journal_path(&state_dir()).to_string_lossy().to_string()
}

/// Directorio del canal `sync` (canal 2).
pub fn sync_dir() -> String {
    format!("{}/sync", state_dir())
}

// ------------------------------------------------------------------------ //
// Estado persistente
// ------------------------------------------------------------------------ //

/// Estado de la voz (JSON). Nunca contiene secretos.
pub fn load_voice_state() -> Value {
    fs::read_to_string(voice_state_path())
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .unwrap_or_else(|| json!({}))
}

/// Guarda el estado de la voz (escritura atómica: un corte no lo deja a medias).
pub fn save_voice_state(v: &Value) -> Result<(), String> {
    let path = voice_state_path();
    let tmp = format!("{}.tmp", path);
    if let Some(parent) = Path::new(&path).parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir: {}", e))?;
    }
    fs::write(&tmp, serde_json::to_string_pretty(v).unwrap_or_default())
        .map_err(|e| format!("write {}: {}", tmp, e))?;
    fs::rename(&tmp, &path).map_err(|e| format!("rename: {}", e))
}

/// Marca una acción ejecutada (para el presupuesto y el anti-bucle).
fn record_action(action_key: &str, ok: bool) -> Result<(), String> {
    let mut st = load_voice_state();
    let now = now_epoch();
    let mut actions: Vec<Value> = st
        .get("actions")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    actions.retain(|a| {
        a.get("ts")
            .and_then(|v| v.as_i64())
            .map(|ts| now - ts < 3600)
            .unwrap_or(false)
    });
    actions.push(json!({"ts": now, "action": action_key, "ok": ok}));
    st["actions"] = Value::Array(actions);
    st["last_action"] = json!(action_key);
    st["last_tick_ts"] = json!(now);
    st["updated_at"] = json!(now_iso());
    save_voice_state(&st)
}

/// ¿Queda presupuesto en esta hora?
pub fn budget_available() -> bool {
    let now = now_epoch();
    let n = load_voice_state()
        .get("actions")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter(|x| {
                    x.get("ts")
                        .and_then(|v| v.as_i64())
                        .map(|ts| now - ts < 3600)
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0);
    (n as i64) < voice_budget_per_hour()
}

/// Ahora en epoch segundos (sin depender de chrono).
pub fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ------------------------------------------------------------------------ //
// Identidad
// ------------------------------------------------------------------------ //

/// Identidad del daemon como agente del enjambre.
pub fn profile() -> VoiceProfile {
    let inst = coordination::Instance::load(&state_dir());
    let node_id = inst
        .as_ref()
        .map(|i| i.id.clone())
        .unwrap_or_else(mesh::instance_id);
    let instance = inst
        .as_ref()
        .map(|i| i.device.clone())
        .unwrap_or_else(mesh::hostname);
    let is_master = inst
        .as_ref()
        .map(|i| i.role.as_str() == "primary")
        .unwrap_or_else(mesh::is_master);
    let st = load_voice_state();
    let endpoint = st
        .get("endpoint")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let mut p = VoiceProfile::new(&node_id, &instance, &endpoint, is_master);
    if let Some(caps) = st.get("capabilities").and_then(|v| v.as_array()) {
        let mine: Vec<String> = caps
            .iter()
            .filter_map(|c| c.as_str().map(|s| s.to_string()))
            .collect();
        if !mine.is_empty() {
            p.capabilities = mine;
        }
    }
    p
}

/// Id con el que la voz reclama tareas (el mismo que aparece en el tablón).
pub fn owner_id() -> String {
    profile().node_id
}

// ------------------------------------------------------------------------ //
// Contexto
// ------------------------------------------------------------------------ //

/// Construye el contexto de la voz desde el estado REAL (tablón + bus).
pub fn build_context() -> VoiceContext {
    let mut ctx = VoiceContext::default();
    ctx.profile = profile();
    ctx.cooldown_secs = voice_cooldown_secs();
    ctx.max_actions_per_tick = 1;

    if let Ok(board) = coordination::TaskBoard::open(&board_path()) {
        if let Ok(tasks) = board.list_tasks(None) {
            ctx.tasks = tasks
                .iter()
                .map(|t| TaskView {
                    task_id: t.id.clone(),
                    title: t.title.clone(),
                    capability: t.owner_hint.clone().unwrap_or_default(),
                    priority: t.priority,
                    state: t.status.as_str().to_string(),
                    claimed_by: t.owner.clone().unwrap_or_default(),
                })
                .collect();
        }
        if let Ok(peers) = board.live_instances(voice_peer_ttl_secs()) {
            ctx.peers = peers
                .iter()
                .map(|p| PeerView {
                    node_id: p.id.clone(),
                    instance: p.device.clone(),
                    capabilities: p.capabilities.clone(),
                    last_seen: p.last_seen,
                })
                .collect();
        }
    }

    let st = load_voice_state();
    ctx.secs_since_last_tick = now_epoch()
        - st
            .get("last_tick_ts")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
    ctx.last_action = st
        .get("last_action")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    ctx
}

// ------------------------------------------------------------------------ //
// Cerebro LLM (adaptador) con degradación segura
// ------------------------------------------------------------------------ //

/// Cerebro que pregunta a un modelo y **cae al criterio fijo** si el modelo
/// devuelve basura, no responde o no hay presupuesto.
pub struct LlmVoiceBrain {
    inner: LlmBrain,
    provider_id: String,
}

impl LlmVoiceBrain {
    pub fn new(provider_id: &str) -> Self {
        LlmVoiceBrain {
            inner: LlmBrain::new(provider_id),
            provider_id: provider_id.to_string(),
        }
    }
}

impl Brain for LlmVoiceBrain {
    fn decide(&self, ctx: &VoiceContext) -> Action {
        if !ctx.may_act() {
            return RuleBrain.decide(ctx);
        }
        let prompt = self.inner.build_prompt(ctx);
        let res = providers::execute_chat(&prompt, Some(&self.provider_id), None, None, 512);
        match self.inner.parse_action(&res.reply) {
            Some(a) => a,
            None => {
                // El modelo no habló el contrato: al criterio fijo.
                RuleBrain.decide(ctx)
            }
        }
    }
}

/// Cerebro que se usa por defecto: LLM si hay proveedor y presupuesto,
/// criterio fijo si no.
pub fn default_brain() -> Box<dyn Brain> {
    let provider = std::env::var("MEGADAEMON_VOICE_PROVIDER")
        .unwrap_or_else(|_| "local-seere".to_string())
        .trim()
        .to_string();
    if provider.is_empty() {
        return Box::new(RuleBrain);
    }
    Box::new(LlmVoiceBrain::new(&provider))
}

// ------------------------------------------------------------------------ //
// Ejecutor
// ------------------------------------------------------------------------ //

/// Tools que la voz NO puede ejecutar (ni el bucle autónomo).
///
/// La voz es un agente, no unydployer: no reinicia el supervisor, no
/// despliega binarios, no borra ficheros ni mata procesos.  Estas tools
/// requieren una decisión humana explícita.
const VETOED: &[&str] = &[
    "restart",
    "restart_daemon",
    "deploy",
    "install",
    "reboot",
    "kill",
    "pkill",
    "rm",
    "mv",
    "cp",
    "push",
    "git_push",
    "shell",
    "root_exec",
    "adb_shell",
];

/// ¿La tool está vetada para la voz?
pub fn is_vetoed(tool: &str) -> bool {
    let t = tool.trim().to_ascii_lowercase();
    VETOED.iter().any(|v| t == *v || t.contains(v))
}

/// Quita cualquier cosa que parezca un secreto de un texto de modelo.
pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for token in text.split_whitespace() {
        let low = token.to_ascii_lowercase();
        let looks_secret = low.contains("mdaemon_sk_live")
            || low.contains("seere_auth_token")
            || (token.len() >= 32 && token.chars().all(|c| c.is_ascii_hexdigit()))
            || low.starts_with("bearer ");
        if looks_secret {
            out.push_str("[redactado]");
            out.push(' ');
        } else {
            out.push_str(token);
            out.push(' ');
        }
    }
    out.trim().to_string()
}

/// Ejecutor real: la registry del daemon (`/api/tool/<n>`).
pub struct DaemonExecutor;

impl Executor for DaemonExecutor {
    fn call(&self, tool: &str, args: &Value) -> Result<Value, String> {
        match tool {
            "mesh_claim" => {
                let task_id = args
                    .get("task_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if task_id.is_empty() {
                    return Err("mesh_claim sin task_id".into());
                }
                let board = coordination::TaskBoard::open(&board_path()).map_err(|e| e)?;
                let owner = owner_id();
                let won = board
                    .claim_task(&task_id, &owner, voice_lease_secs())
                    .map_err(|e| e)?;
                if !won {
                    return Err(format!("la tarea {} ya está reclamada por otro", task_id));
                }
                let _ = board.publish_event(
                    "voice_claim",
                    &owner,
                    &json!({"task_id": task_id, "node": owner}),
                );
                Ok(json!({
                    "ok": true,
                    "summary": format!("tarea {} reclamada (lease {}s)", task_id, voice_lease_secs()),
                    "task_id": task_id,
                }))
            }
            "mesh_ask" => {
                let peer = args
                    .get("peer")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let question = args
                    .get("question")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if peer.is_empty() || question.is_empty() {
                    return Err("mesh_ask necesita peer y question".into());
                }
                let ctx = build_context();
                let msg = speak_to_peer(&ctx, &peer, &question);
                let r = webui::node_message(&peer, &msg, "");
                Ok(json!({
                    "ok": r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
                    "summary": format!("pregunta depositada en el buzón de {}", peer),
                    "peer": peer,
                    "task_id": r.get("task_id").cloned().unwrap_or(Value::Null),
                }))
            }
            "swarm_chat" => {
                let text = args
                    .get("text")
                    .and_then(|v| v.as_str())
                    .or_else(|| args.get("prompt").and_then(|v| v.as_str()))
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if text.is_empty() {
                    return Err("swarm_chat sin texto".into());
                }
                let provider = args
                    .get("provider_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("local-seere");
                let res = providers::execute_chat(&text, Some(provider), None, None, 512);
                if res.reply.trim().is_empty() {
                    return Err(format!("el proveedor {} no respondió", provider));
                }
                Ok(json!({
                    "ok": true,
                    "summary": redact(&res.reply),
                    "model": res.model_name,
                    "latency_ms": res.latency_ms,
                }))
            }
            other => {
                if is_vetoed(other) {
                    return Err(format!("la tool {} está vetada para la voz", other));
                }
                match crate::mcp_ext::dispatch(other, args) {
                    Some(v) => Ok(v),
                    None => Err(format!("tool desconocida: {}", other)),
                }
            }
        }
    }
}

// ------------------------------------------------------------------------ //
// Difusión (4 canales)
// ------------------------------------------------------------------------ //

/// Difunde líneas NDJSON por los 4 canales: journal, sync, tablón y swarm_chat.
///
/// Devuelve qué se hizo y qué se saltó **con el motivo** (nada inventado).
pub fn broadcast(lines: &[String]) -> Value {
    let mut journal_ok = 0usize;
    let mut sync_ok = 0usize;
    let mut board_ok = 0usize;
    let mut swarm: Option<Value> = None;
    let mut errors: Vec<String> = Vec::new();

    let journal = journal_path();
    let sync_file = format!("{}/re_{}.ndjson", sync_dir(), sanitize(&owner_id()));

    for line in lines {
        if let Some(parent) = Path::new(&journal).parent() {
            let _ = fs::create_dir_all(parent);
        }
        match fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&journal)
            .and_then(|mut f| {
                use std::io::Write;
                f.write_all(line.as_bytes())?;
                f.write_all(b"\n")?;
                Ok(())
            }) {
            Ok(_) => journal_ok += 1,
            Err(e) => errors.push(format!("journal: {}", e)),
        }

        if let Some(parent) = Path::new(&sync_file).parent() {
            let _ = fs::create_dir_all(parent);
        }
        match fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&sync_file)
            .and_then(|mut f| {
                use std::io::Write;
                f.write_all(line.as_bytes())?;
                f.write_all(b"\n")?;
                Ok(())
            }) {
            Ok(_) => sync_ok += 1,
            Err(e) => errors.push(format!("sync: {}", e)),
        }

        if let Ok(v) = serde_json::from_str::<Value>(line) {
            if let Ok(board) = coordination::TaskBoard::open(&board_path()) {
                if board
                    .publish_event("voice_action", &owner_id(), &v)
                    .is_ok()
                {
                    board_ok += 1;
                }
            }
        }
    }

    // Canal 4: el enjambre (swarm_chat). Sólo si hay presupuesto: cada
    // llamada es un modelo (dinero/tiempo) y el móvil va justo.
    let spoken = lines
        .iter()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|v| v.get("describe").and_then(|d| d.as_str()).map(|s| s.to_string()))
        .collect::<Vec<_>>()
        .join("; ");
    if spoken.is_empty() {
        swarm = Some(json!({"ok": false, "skipped": "sin texto que difundir"}));
    } else if !budget_available() {
        swarm = Some(json!({"ok": false, "skipped": "presupuesto de acciones agotado"}));
    } else {
        let provider = std::env::var("MEGADAEMON_VOICE_PROVIDER")
            .unwrap_or_else(|_| "local-seere".to_string());
        let res = providers::execute_chat(&spoken, Some(&provider), None, None, 256);
        swarm = Some(json!({
            "ok": !res.reply.trim().is_empty(),
            "provider": provider,
            "model": res.model_name,
            "latency_ms": res.latency_ms,
            "reply": redact(&res.reply),
        }));
    }

    json!({
        "journal_lines": journal_ok,
        "sync_lines": sync_ok,
        "board_events": board_ok,
        "swarm": swarm,
        "errors": errors,
    })
}

/// Nombre de fichero seguro para un id de nodo.
fn sanitize(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

// ------------------------------------------------------------------------ //
// Catálogo unificado (W6)
// ------------------------------------------------------------------------ //

/// Catálogo unificado: MODELOS + AGENTES/SERVICIOS.
///
/// **Delega en `service_bus` (W6)**: el módulo del bus de servicios es la
/// fuente canónica del catálogo; aquí sólo se serializa al esquema que la UI
/// v2 pinta y que la voz consume para elegir proveedor.
///
/// Campos por entrada: `id, kind, endpoint, auth_kind, health, latency,
/// cost_estimate, capabilities, enabled`.  Lo que no se puede medir va
/// `null` + motivo: **nada inventado**.
pub fn catalog_entries() -> Vec<crate::service_bus::ServiceEntry> {
    let mut entries: Vec<crate::service_bus::ServiceEntry> = Vec::new();

    // MODELOS: del catálogo redacted de providers (con `ready` real).
    entries.extend(crate::service_bus::catalog_from_providers_json(
        &providers::list_providers_redacted(),
        "md-secrets/instance.env",
    ));

    // AGENTES: los peers vivos del bus (snapshot del mesh).
    if let Some(snap) = mesh_snapshot() {
        entries.extend(crate::service_bus::catalog_from_mesh_snapshot(
            &snap,
            voice_peer_ttl_secs(),
        ));
    }

    // La propia voz es un agente del enjambre (no viene del snapshot del
    // mesh: es esta misma instancia).
    let prof = profile();
    let mut voice = crate::service_bus::ServiceEntry::new(
        &prof.node_id,
        crate::service_bus::ServiceKind::Agent,
    );
    voice.label = format!("voz de {}", prof.instance);
    voice.endpoint = prof.endpoint.clone();
    voice.auth_kind = "api_key".to_string();
    voice.capabilities = prof.capabilities.clone();
    voice.notes = "identidad publica de la voz (sin secretos)".to_string();
    voice.enabled = true;
    entries.push(voice);

    // SERVICIOS: este daemon (MCP), el bus y la propia voz.
    entries.push(crate::service_bus::seed_mcp_server(
        "megadaemon-mcp",
        if prof.endpoint.is_empty() {
            "local://cockpit"
        } else {
            &prof.endpoint
        },
        "api_key+session_token",
        "md-secrets/instance.env",
        &["tools", "voice", "catalog"],
    ));
    entries.push(crate::service_bus::seed_local_tool(
        "megadaemon-voice",
        "local://voice_runtime",
        &["voice", "plan", "act", "whoami"],
    ));
    entries
}

/// Snapshot del mesh (para el catálogo de agentes).
pub fn mesh_snapshot() -> Option<Value> {
    let dir = state_dir();
    crate::mesh::Mesh::open(&dir).ok().map(|m| m.snapshot())
}

/// Entrada del bus serializada al esquema unificado.
fn entry_json(e: &crate::service_bus::ServiceEntry) -> Value {
    json!({
        "id": e.id,
        "kind": e.kind.as_str(),
        "label": e.label,
        "endpoint": e.endpoint,
        "auth_kind": e.auth_kind,
        // `auth_ref` es la RUTA del fichero de secretos; nunca la credencial.
        "auth_ref": e.auth_ref,
        "health": {
            "available": e.health.available,
            "reason": e.health.reason,
            "detail": e.health.detail,
            "checked_at": e.health.checked_at,
        },
        "latency": e.latency_ms,
        "latency_measured": e.latency_ms.is_some(),
        "cost_estimate": {
            "usd_per_1k_input": e.cost_estimate.usd_per_1k_input,
            "usd_per_1k_output": e.cost_estimate.usd_per_1k_output,
            "usd_per_call": e.cost_estimate.usd_per_call,
            "free": e.cost_estimate.free,
        },
        "capabilities": e.capabilities,
        "enabled": e.enabled,
        "model": e.model,
        "tier": e.tier,
        "notes": e.notes,
    })
}

/// Catálogo unificado (MODELOS + AGENTES/SERVICIOS).
pub fn catalog() -> Value {
    let entries = catalog_entries();
    let items: Vec<Value> = entries.iter().map(entry_json).collect();
    let count = |k: &str| items.iter().filter(|i| i["kind"] == k).count();
    json!({
        "schema": crate::service_bus::SERVICE_BUS_SCHEMA,
        "items": items,
        "counts": {
            "model": count("model"),
            "agent": count("agent"),
            "agent_client": count("agent_client"),
            "mcp_server": count("mcp_server"),
            "local_tool": count("local_tool"),
        },
        "generated_at": now_iso(),
        "note": "latency sin medir => null; health real o 'available:false' con motivo; sin credenciales",
    })
}

/// Elige proveedor por tarea (lo usa la voz): el `service_bus` decide con
/// su política (y fallback), no inventamos reglas duplicadas.
pub fn pick_provider(task_hint: &str) -> Option<String> {
    let hint = task_hint.to_ascii_lowercase();
    let task = if hint.contains("build") || hint.contains("compil") {
        crate::service_bus::TaskType::Build
    } else if hint.contains("anal") || hint.contains("razon") || hint.contains("reason") {
        crate::service_bus::TaskType::Analysis
    } else if hint.contains("redac") || hint.contains("escrib") || hint.contains("writing") {
        crate::service_bus::TaskType::Writing
    } else {
        crate::service_bus::TaskType::Inference
    };
    let bus = crate::service_bus::ServiceBus::new(catalog_entries());
    bus.select(task).chosen
}

// ------------------------------------------------------------------------ //
// Acciones de alto nivel
// ------------------------------------------------------------------------ //

/// Un tick completo: plan → act → difusión.
pub fn tick(brain: &dyn Brain) -> Value {
    let ctx = build_context();
    if !ctx.may_act() {
        return json!({
            "ok": true,
            "acted": false,
            "reason": format!("cooldown: hace {}s (min {}s)", ctx.secs_since_last_tick, ctx.cooldown_secs),
            "status": voice_status(&ctx),
        });
    }
    if !budget_available() {
        return json!({
            "ok": true,
            "acted": false,
            "reason": format!("presupuesto agotado ({} acciones/hora)", voice_budget_per_hour()),
            "status": voice_status(&ctx),
        });
    }
    let action = plan_next(&ctx, brain);
    let outcome = act(&ctx, &action, &DaemonExecutor);
    let spread = broadcast(&outcome.broadcast);
    let _ = record_action(&outcome.action.key(), outcome.ok);
    json!({
        "ok": true,
        "acted": !matches!(outcome.action, Action::Idle { .. }),
        "action": action_json(&outcome.action),
        "detail": outcome.detail,
        "broadcast": spread,
        "status": voice_status(&ctx),
    })
}

/// Acción a JSON (para la tool y el journal).
pub fn action_json(a: &Action) -> Value {
    match a {
        Action::Claim { task_id, capability } => json!({
            "action": "claim", "task_id": task_id, "capability": capability,
        }),
        Action::Ask { peer, question } => json!({
            "action": "ask", "peer": peer, "question": question,
        }),
        Action::Speak { text } => json!({"action": "speak", "text": text}),
        Action::Do { tool, args } => json!({"action": "do", "tool": tool, "args": args}),
        Action::Idle { reason } => json!({"action": "idle", "reason": reason}),
    }
}

/// Ejecuta una acción concreta (sin decidirla).
pub fn run_action(action: &Action, spread: bool) -> Value {
    let ctx = build_context();
    let outcome = act(&ctx, action, &DaemonExecutor);
    let broadcast = if spread { broadcast(&outcome.broadcast) } else { json!({"skipped": true}) };
    let _ = record_action(&outcome.action.key(), outcome.ok);
    json!({
        "ok": outcome.ok,
        "action": action_json(&outcome.action),
        "detail": outcome.detail,
        "broadcast": broadcast,
    })
}

/// Estado de la voz (para `status`).
pub fn status() -> Value {
    let ctx = build_context();
    let st = load_voice_state();
    json!({
        "voice": voice_status(&ctx),
        "enabled": voice_enabled(),
        "interval_secs": voice_interval_secs(),
        "budget_per_hour": voice_budget_per_hour(),
        "budget_available": budget_available(),
        "vetoed_tools": VETOED,
        "last_tick_ts": st.get("last_tick_ts").cloned().unwrap_or(Value::Null),
        "last_tick_at": st.get("updated_at").cloned().unwrap_or(Value::Null),
        "board": board_path(),
        "journal": journal_path(),
    })
}

/// Identidad pública de la voz.
pub fn whoami_json() -> Value {
    let mut v = whoami(&profile());
    if let Some(obj) = v.as_object_mut() {
        obj.insert("board".into(), json!(board_path()));
        obj.insert("voice_enabled".into(), json!(voice_enabled()));
        obj.insert("vetoed_tools".into(), json!(VETOED));
    }
    v
}

/// Habla con un nodo por el bus (deja la pregunta en su buzón).
pub fn speak(peer: &str, text: &str) -> Value {
    let ctx = build_context();
    let msg = speak_to_peer(&ctx, peer, text);
    let r = webui::node_message(peer, &msg, "");
    let mut out = r.clone();
    if let Some(obj) = out.as_object_mut() {
        obj.insert("message".into(), json!(redact(&msg)));
    }
    out
}

/// Punto de entrada de la tool MCP `megadaemon_voice`.
pub fn handle(action: &str, args: &Value) -> Value {
    match action {
        "plan" => {
            let ctx = build_context();
            let brain: Box<dyn Brain> = if args
                .get("llm")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                default_brain()
            } else {
                Box::new(RuleBrain)
            };
            let a = plan_next(&ctx, brain.as_ref());
            json!({
                "ok": true,
                "action": action_json(&a),
                "describe": a.describe(),
                "status": voice_status(&ctx),
            })
        }
        "act" => {
            // Acción explícita o, si no viene, la que decide el cerebro.
            let action = match args.get("action").and_then(|v| v.as_str()) {
                Some("claim") => Action::Claim {
                    task_id: args.get("task_id").and_then(|v| v.as_str()).unwrap_or("").into(),
                    capability: args.get("capability").and_then(|v| v.as_str()).unwrap_or("").into(),
                },
                Some("ask") => Action::Ask {
                    peer: args.get("peer").and_then(|v| v.as_str()).unwrap_or("").into(),
                    question: args.get("question").and_then(|v| v.as_str()).unwrap_or("").into(),
                },
                Some("speak") => Action::Speak {
                    text: args.get("text").and_then(|v| v.as_str()).unwrap_or("").into(),
                },
                Some("do") => Action::Do {
                    tool: args.get("tool").and_then(|v| v.as_str()).unwrap_or("").into(),
                    args: args.get("args").cloned().unwrap_or(json!({})),
                },
                Some("idle") => Action::Idle {
                    reason: args.get("reason").and_then(|v| v.as_str()).unwrap_or("").into(),
                },
                _ => {
                    let brain = default_brain();
                    let ctx = build_context();
                    plan_next(&ctx, brain.as_ref())
                }
            };
            let spread = args
                .get("broadcast")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            run_action(&action, spread)
        }
        "speak" => {
            let peer = args.get("peer").and_then(|v| v.as_str()).unwrap_or("");
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
            if peer.is_empty() || text.is_empty() {
                json!({"ok": false, "error": "speak necesita peer y text"})
            } else {
                speak(peer, text)
            }
        }
        "whoami" => whoami_json(),
        "status" => status(),
        _ => json!({
            "ok": false,
            "error": "accion desconocida",
            "hint": "acciones: plan | act | speak | whoami | status",
        }),
    }
}

// ------------------------------------------------------------------------ //
// Bucle autónomo (opt-in)
// ------------------------------------------------------------------------ //

/// Arranca el bucle autónomo si `MEGADAEMON_VOICE=1`.
///
/// Un hilo, un tick cada N s, 1 acción por tick, cooldown y presupuesto.
/// **Nunca** lanza procesos ni reinicia nada.
pub fn spawn_autonomous_loop() {
    if !voice_enabled() {
        return;
    }
    let interval = voice_interval_secs().max(30) as u64;
    let _ = std::thread::Builder::new()
        .name("voice".into())
        .spawn(move || {
            // Espera un poco: el arranque del cockpit debe stabilisation.
            std::thread::sleep(std::time::Duration::from_secs(20));
            loop {
                let brain = default_brain();
                let out = tick(brain.as_ref());
                let line = json!({
                    "ts": now_iso(),
                    "node": owner_id(),
                    "event": "voice_tick",
                    "kind": "voice",
                    "acted": out.get("acted").cloned().unwrap_or(Value::Bool(false)),
                    "action": out.get("action").cloned().unwrap_or(Value::Null),
                    "reason": out.get("reason").cloned().unwrap_or(Value::Null),
                });
                let journal = journal_path();
                if let Some(parent) = Path::new(&journal).parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if let Ok(mut f) = fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&journal)
                {
                    use std::io::Write;
                    let _ = f.write_all(line.to_string().as_bytes());
                    let _ = f.write_all(b"\n");
                }
                std::thread::sleep(std::time::Duration::from_secs(interval));
            }
        });
}

// ------------------------------------------------------------------------ //
// Tests
// ------------------------------------------------------------------------ //

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_destructivas_estan_vetadas() {
        assert!(is_vetoed("restart"));
        assert!(is_vetoed("pkill"));
        assert!(is_vetoed("root_exec"));
        assert!(is_vetoed("deploy"));
        assert!(!is_vetoed("apk_inventory"));
        assert!(!is_vetoed("dex_decompile"));
    }

    #[test]
    fn redacta_secretos() {
        let t = "mdaemon_sk_live_0123456789abcdef0123456789abcdef";
        let r = redact(t);
        assert!(!r.contains("mdaemon_sk_live"));
        assert!(r.contains("[redactado]"));
    }

    #[test]
    fn redacta_hex_largo() {
        let r = redact("hash 0123456789abcdef0123456789abcdef0123456789abcdef ok");
        assert!(r.contains("[redactado]"));
    }

    #[test]
    fn accion_a_json_cubre_las_cinco() {
        let acts = vec![
            Action::Claim { task_id: "t1".into(), capability: "reverse".into() },
            Action::Ask { peer: "p".into(), question: "q".into() },
            Action::Speak { text: "hola".into() },
            Action::Do { tool: "apk_inventory".into(), args: json!({}) },
            Action::Idle { reason: "nada".into() },
        ];
        for a in acts {
            let j = action_json(&a);
            assert!(j.get("action").and_then(|v| v.as_str()).is_some());
        }
    }

    #[test]
    fn el_contexto_tiene_los_campos_basicos() {
        let ctx = build_context();
        assert!(!ctx.profile.node_id.is_empty());
        assert_eq!(ctx.max_actions_per_tick, 1);
        assert!(ctx.cooldown_secs >= 0);
    }

    #[test]
    fn el_catalogo_tiene_el_esquema_unificado() {
        let c = catalog();
        let items = c.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        // Siempre hay entradas propias (la voz y el MCP del daemon).
        assert!(items.iter().any(|i| i["kind"] == "agent"));
        assert!(items.iter().any(|i| i["kind"] == "mcp_server"));
        // Los modelos dependen del vault configurado: si no hay proveedores,
        // no hay modelos (y NO se inventa ninguno).
        for i in items.iter() {
            for f in [
                "id", "kind", "endpoint", "auth_kind", "health", "latency",
                "cost_estimate", "capabilities", "enabled",
            ] {
                assert!(i.get(f).is_some(), "falta el campo {} en {:?}", f, i["id"]);
            }
            // Nada inventado: la salud trae siempre el motivo.
            assert!(i["health"].get("available").is_some());
            assert!(i["health"].get("reason").is_some());
            // Y ninguna credencial en el catálogo.
            let txt = i.to_string();
            assert!(!txt.contains("mdaemon_sk_live"));
        }
    }

    #[test]
    fn el_estado_de_la_voz_no_trae_secretos() {
        let st = load_voice_state();
        let txt = st.to_string();
        assert!(!txt.contains("mdaemon_sk_live"));
    }
}
