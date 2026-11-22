//! `md-antigravity` — puente de la flota contra la IDE agéntica **Antigravity**
//! (`com.agy`) que vive en el móvil del usuario.
//!
//! La IDE expone un *language server* en `127.0.0.1:45157` (HTTPS con
//! certificado autofirmado) que habla **Connect RPC**:
//!
//! ```text
//! POST https://127.0.0.1:45157/exa.language_server_pb.LanguageServerService/<Metodo>
//! ```
//!
//! Este binario es el cliente de la flota.  **No** imprime el token CSRF:
//! sólo su `sha16` (medido: `0bde8f17043f1927` en el Samsung del usuario).
//!
//! USO
//! ---
//! ```text
//! md-antigravity status                     # ¿vive el language server?
//! md-antigravity catalog                   # servidores MCP + tools que expone
//! md-antigravity conversations              # lista de conversaciones/agentes
//! md-antigravity search <texto>            # busca en el historial
//! md-antigravity conversation <cascade_id>  # lee una conversación
//! md-antigravity tasks                     # cola de tareas del daemon de Antigravity
//! md-antigravity enqueue <texto>           # encola trabajo en el agente
//! ```
//!
//! OPCIONES
//! --------
//! ```text
//! --json            salida JSON (por defecto es texto legible)
//! --host <h>        host del language server (por defecto 127.0.0.1)
//! --port <n>        puerto (por defecto 45157)
//! --timeout <s>     timeout por llamada (por defecto 25)
//! --model <enum>    modelo del agente (por defecto el del estado del móvil)
//! --workspace <uri> workspace del agente (repetible)
//! --wait <s>        tras encolar, esperar a que el agente termine
//! ```
//!
//! NOTA DE RED
//! -----------
//! El language server **sólo** escucha en el loopback del móvil ⇒ este
//! binario corre **en el móvil** (o tras un `adb forward`).

use std::process::ExitCode;

use megadaemon_core::antigravity_bridge::{
    self, AntigravityBridge, BridgeConfig, BridgeError, ConversationSummary, StepSummary,
};
use serde_json::{json, Value};

/// Lee el valor de una opción.
fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == name {
            return it.next().cloned();
        }
        if let Some(v) = a.strip_prefix(&format!("{name}=")) {
            return Some(v.to_string());
        }
    }
    None
}

/// ¿La opción está presente?
fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}

/// Todos los valores de una opción repetible.
fn args_all(name: &str) -> Vec<String> {
    let argv: Vec<String> = std::env::args().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < argv.len() {
        if argv[i] == name {
            if i + 1 < argv.len() {
                out.push(argv[i + 1].clone());
            }
            i += 2;
            continue;
        }
        if let Some(v) = argv[i].strip_prefix(&format!("{name}=")) {
            out.push(v.to_string());
        }
        i += 1;
    }
    out
}

/// Configuración a partir de las opciones.
fn config() -> BridgeConfig {
    let mut cfg = BridgeConfig::default();
    if let Some(h) = arg("--host") {
        cfg.host = h;
    }
    if let Some(p) = arg("--port").and_then(|v| v.parse::<u16>().ok()) {
        cfg.port = p;
    }
    if let Some(t) = arg("--timeout").and_then(|v| v.parse::<u64>().ok()) {
        cfg.timeout_secs = t;
    }
    if let Some(m) = arg("--model") {
        cfg.model = Some(m);
    }
    cfg.workspace_uris = args_all("--workspace");
    cfg
}

/// Conecta con el language server (con errores claros).
fn connect(cfg: &BridgeConfig) -> Result<AntigravityBridge, BridgeError> {
    AntigravityBridge::new(cfg)
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let sub = argv.get(1).cloned().unwrap_or_default();
    let json_out = flag("--json");
    let cfg = config();

    let res: Result<(), String> = (|| {
        match sub.as_str() {
            "status" => cmd_status(&cfg, json_out),
            "catalog" => cmd_catalog(&cfg, json_out),
            "conversations" => cmd_conversations(&cfg, json_out),
            "search" => {
                let q = argv.get(2).cloned().unwrap_or_default();
                if q.is_empty() {
                    return Err("uso: md-antigravity search <texto>".into());
                }
                cmd_search(&cfg, &q, json_out)
            }
            "conversation" => {
                let id = argv.get(2).cloned().unwrap_or_default();
                if id.is_empty() {
                    return Err("uso: md-antigravity conversation <cascade_id>".into());
                }
                cmd_conversation(&cfg, &id, json_out)
            }
            "tasks" => cmd_tasks(json_out),
            "enqueue" => {
                let text = argv[2..].join(" ");
                if text.trim().is_empty() {
                    return Err("uso: md-antigravity enqueue <texto>".into());
                }
                cmd_enqueue(&cfg, &text, json_out)
            }
            "help" | "--help" | "-h" | "" => {
                usage();
                Ok(())
            }
            other => Err(format!(
                "comando desconocido: {other} (usa `md-antigravity help`)"
            )),
        }
    })();

    match res {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("md-antigravity: {e}");
            ExitCode::from(2)
        }
    }
}

/// Mensaje de uso.
fn usage() {
    eprintln!(
        "uso: md-antigravity <status|catalog|conversations|search|conversation|tasks|enqueue> [args]\n\
         \n\
         opciones: --json --host <h> --port <n> --timeout <s> --model <enum> --workspace <uri> --wait <s>"
    );
}

/// Imprime un valor: JSON o texto legible.
fn emit(json_out: bool, value: &Value, legible: impl FnOnce(&Value)) {
    if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
        );
    } else {
        legible(value);
    }
}

// ---------------------------------------------------------------------------
// Subcomandos
// ---------------------------------------------------------------------------

fn cmd_status(cfg: &BridgeConfig, json_out: bool) -> Result<(), String> {
    let b = connect(cfg).map_err(|e| e.to_string())?;
    let st = b.agent_status().map_err(|e| e.to_string())?;
    emit(json_out, &st, |v| {
        println!(
            "language server: {} (modelo {})",
            v.pointer("/language_server")
                .map(|x| x.to_string())
                .unwrap_or_else(|| "?".into()),
            v.get("model").and_then(|x| x.as_str()).unwrap_or("?")
        );
        println!(
            "conversaciones: {}   csrf_sha16: {}",
            v.get("conversations").and_then(|x| x.as_u64()).unwrap_or(0),
            v.get("csrf_sha16").and_then(|x| x.as_str()).unwrap_or("?")
        );
    });
    Ok(())
}

fn cmd_catalog(cfg: &BridgeConfig, json_out: bool) -> Result<(), String> {
    let b = connect(cfg).map_err(|e| e.to_string())?;
    let cat = b.tool_catalog().map_err(|e| e.to_string())?;
    emit(json_out, &cat, |v| {
        let servers = v.get("servers").and_then(|s| s.as_array());
        println!(
            "servidores MCP: {}   tools: {}",
            v.get("server_count").and_then(|x| x.as_u64()).unwrap_or(0),
            v.get("tool_count").and_then(|x| x.as_u64()).unwrap_or(0)
        );
        if let Some(arr) = servers {
            for s in arr {
                println!(
                    "  - {} [{}] {}",
                    s.get("server").and_then(|x| x.as_str()).unwrap_or("?"),
                    s.get("status").and_then(|x| x.as_str()).unwrap_or("?"),
                    s.get("error").and_then(|x| x.as_str()).unwrap_or("")
                );
            }
        }
        if let Some(tools) = v.get("tools").and_then(|t| t.as_array()) {
            println!("tools:");
            for t in tools {
                println!(
                    "  {}/{} ({} params)",
                    t.get("server").and_then(|x| x.as_str()).unwrap_or("?"),
                    t.get("name").and_then(|x| x.as_str()).unwrap_or("?"),
                    t.get("params").and_then(|x| x.as_u64()).unwrap_or(0)
                );
            }
        }
    });
    Ok(())
}

fn cmd_conversations(cfg: &BridgeConfig, json_out: bool) -> Result<(), String> {
    let b = connect(cfg).map_err(|e| e.to_string())?;
    let convs = b.conversations().map_err(|e| e.to_string())?;
    let v = Value::Array(convs.iter().map(conversation_json).collect());
    emit(json_out, &v, |v| {
        if let Some(arr) = v.as_array() {
            for c in arr {
                println!(
                    "{}  {:<28}  {:>6} pasos  {}",
                    c.get("cascade_id").and_then(|x| x.as_str()).unwrap_or("?"),
                    c.get("status").and_then(|x| x.as_str()).unwrap_or("?"),
                    c.get("step_count").and_then(|x| x.as_u64()).unwrap_or(0),
                    c.get("title").and_then(|x| x.as_str()).unwrap_or("")
                );
            }
        }
    });
    Ok(())
}

fn conversation_json(c: &ConversationSummary) -> Value {
    json!({
        "cascade_id": c.cascade_id,
        "title": c.title,
        "trajectory_id": c.trajectory_id,
        "status": c.status,
        "step_count": c.step_count,
        "last_modified": c.last_modified,
        "workspaces": c.workspaces,
    })
}

fn cmd_search(cfg: &BridgeConfig, q: &str, json_out: bool) -> Result<(), String> {
    let b = connect(cfg).map_err(|e| e.to_string())?;
    let res = b.search(q, 20).map_err(|e| e.to_string())?;
    emit(json_out, &res, |v| {
        if let Some(arr) = v.get("results").and_then(|r| r.as_array()) {
            if arr.is_empty() {
                println!("(sin resultados para «{q}»)");
            }
            for r in arr {
                println!(
                    "{}  {}",
                    r.get("cascadeId").and_then(|x| x.as_str()).unwrap_or("?"),
                    r.get("title").and_then(|x| x.as_str()).unwrap_or("")
                );
                println!(
                    "    {}",
                    r.get("snippet").and_then(|x| x.as_str()).unwrap_or("").replace('\n', " ")
                );
            }
        }
    });
    Ok(())
}

fn cmd_conversation(cfg: &BridgeConfig, id: &str, json_out: bool) -> Result<(), String> {
    let b = connect(cfg).map_err(|e| e.to_string())?;
    let traj = b.conversation(id).map_err(|e| e.to_string())?;
    let steps: Vec<StepSummary> = antigravity_bridge::step_summaries_from(&traj);
    let v = json!({
        "cascade_id": traj.pointer("/trajectory/cascadeId").and_then(|x| x.as_str()).unwrap_or(id),
        "trajectory_id": traj.pointer("/trajectory/trajectoryId").and_then(|x| x.as_str()).unwrap_or(""),
        "status": traj.get("status").and_then(|x| x.as_str()).unwrap_or(""),
        "num_total_steps": traj.get("numTotalSteps").and_then(|x| x.as_u64()).unwrap_or(0),
        "steps": steps.iter().map(|s| json!({
            "kind": s.kind, "status": s.status, "text": s.text, "error": s.error
        })).collect::<Vec<Value>>(),
    });
    emit(json_out, &v, |v| {
        println!(
            "{}  status={}  pasos={}",
            v.get("cascade_id").and_then(|x| x.as_str()).unwrap_or("?"),
            v.get("status").and_then(|x| x.as_str()).unwrap_or("?"),
            v.get("num_total_steps").and_then(|x| x.as_u64()).unwrap_or(0)
        );
        if let Some(arr) = v.get("steps").and_then(|s| s.as_array()) {
            for s in arr {
                let err = s.get("error").and_then(|x| x.as_str()).unwrap_or("");
                let txt = s.get("text").and_then(|x| x.as_str()).unwrap_or("");
                println!(
                    "  - {} [{}] {}{}",
                    s.get("kind").and_then(|x| x.as_str()).unwrap_or("?"),
                    s.get("status").and_then(|x| x.as_str()).unwrap_or("?"),
                    txt.replace('\n', " "),
                    if err.is_empty() {
                        String::new()
                    } else {
                        format!("  ERROR: {err}")
                    }
                );
            }
        }
    });
    Ok(())
}

fn cmd_tasks(json_out: bool) -> Result<(), String> {
    let raw = std::fs::read_to_string(antigravity_bridge::AGY_TASKS)
        .map_err(|e| format!("no se pudo leer {}: {e}", antigravity_bridge::AGY_TASKS))?;
    let hooks = megadaemon_core::antigravity_intel::parse_daemon_tasks(&raw);
    let v = Value::Array(
        hooks
            .iter()
            .map(|h| {
                json!({
                    "id": h.id,
                    "name": h.name,
                    "trigger": h.trigger_type,
                    "interval_sec": h.interval_sec,
                    "enabled": h.enabled,
                    "command_preview": h.command_preview,
                    "last_run_status": h.last_run_status,
                    "last_run_duration": h.last_run_duration,
                })
            })
            .collect(),
    );
    emit(json_out, &v, |v| {
        if let Some(arr) = v.as_array() {
            for t in arr {
                println!(
                    "{}  {}  trigger={} cada={}s  enabled={}  ultimo={:?}",
                    t.get("id").and_then(|x| x.as_str()).unwrap_or("?"),
                    t.get("name").and_then(|x| x.as_str()).unwrap_or(""),
                    t.get("trigger").and_then(|x| x.as_str()).unwrap_or("?"),
                    t.get("interval_sec").and_then(|x| x.as_u64()).unwrap_or(0),
                    t.get("enabled").and_then(|x| x.as_bool()).unwrap_or(false),
                    t.get("last_run_status").and_then(|x| x.as_str())
                );
            }
        }
    });
    Ok(())
}

fn cmd_enqueue(cfg: &BridgeConfig, text: &str, json_out: bool) -> Result<(), String> {
    let b = connect(cfg).map_err(|e| e.to_string())?;
    let cid = b
        .enqueue(text, &cfg.workspace_uris)
        .map_err(|e| e.to_string())?;
    let wait = arg("--wait").and_then(|v| v.parse::<u64>().ok());

    let mut out = json!({
        "cascade_id": cid,
        "model": b.model(),
        "queued": true,
    });
    if let Some(secs) = wait {
        // Espera activa: el agente va dejando pasos en la trayectoria.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
        let mut last: Option<Value> = None;
        while std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_secs(3));
            if let Ok(t) = b.conversation(&cid) {
                let steps = antigravity_bridge::step_summaries_from(&t);
                let running = steps.iter().any(|s| {
                    let s = s.status.to_uppercase();
                    s.contains("PENDING") || s.contains("RUNNING") || s.contains("IN_PROGRESS")
                });
                last = Some(t);
                if !running && steps.len() > 1 {
                    break;
                }
            }
        }
        if let Some(t) = last {
            let steps = antigravity_bridge::step_summaries_from(&t);
            out["steps"] = Value::Array(
                steps
                    .iter()
                    .map(|s| {
                        json!({"kind": s.kind, "status": s.status, "text": s.text, "error": s.error})
                    })
                    .collect(),
            );
            out["status"] = json!(t.get("status").and_then(|x| x.as_str()).unwrap_or(""));
        }
    }
    emit(json_out, &out, |v| {
        println!(
            "encolado en Antigravity: cascade_id={}",
            v.get("cascade_id").and_then(|x| x.as_str()).unwrap_or("?")
        );
        if let Some(arr) = v.get("steps").and_then(|s| s.as_array()) {
            for s in arr {
                let err = s.get("error").and_then(|x| x.as_str()).unwrap_or("");
                let txt = s.get("text").and_then(|x| x.as_str()).unwrap_or("");
                println!(
                    "  - {} [{}] {}{}",
                    s.get("kind").and_then(|x| x.as_str()).unwrap_or("?"),
                    s.get("status").and_then(|x| x.as_str()).unwrap_or("?"),
                    txt.replace('\n', " "),
                    if err.is_empty() {
                        String::new()
                    } else {
                        format!("  ERROR: {err}")
                    }
                );
            }
        }
    });
    Ok(())
}
