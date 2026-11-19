//! `md-coord` — CLI del tablero de coordinación de MegaDaemon.
//!
//! Se usa desde el móvil y desde el PC para que la coordinación de los
//! agentes pase por el demonio (y no por los chats):
//!
//! ```text
//! md-coord register --state <dir> --device samsung-s21-ultra --platform android-arm64 [--role primary]
//! md-coord heartbeat --state <dir>
//! md-coord add "<titulo>" [--priority N] [--owner <dominio>]
//! md-coord claim <task_id> --owner <agente> [--lease 900]
//! md-coord report <task_id> --owner <agente> --ok|--fail "<3 líneas>"
//! md-coord tasks [--status queued|claimed|done|failed]
//! md-coord instances
//! md-coord migrate --target 13.5.0
//! md-coord snapshot
//! ```
//!
//! El tablero vive en `<state_dir>/board.sqlite` por defecto.

use megadaemon_core::coordination::{Instance, InstanceRole, TaskBoard, TaskStatus};
use std::process::ExitCode;

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 0;
    while i < args.len() {
        if args[i] == name && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
        if let Some(v) = args[i].strip_prefix(&format!("{}=", name)) {
            return Some(v.to_string());
        }
        i += 1;
    }
    None
}

fn has(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}

fn usage() -> &'static str {
    "uso: md-coord <register|heartbeat|add|claim|report|tasks|instances|migrate|snapshot> [opciones]"
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("{}", usage());
        return ExitCode::from(2);
    }
    let cmd = args[1].clone();
    // v14: el state_dir por defecto debe ser el MISMO que usa el daemon
    // (`MEGADAEMON_STATE_DIR` en megadaemon.env = <PREFIX>/state).  Antes el
    // CLI caía en <PREFIX> y abría OTRO tablero (board.sqlite vacío) mientras
    // el daemon escribía en <PREFIX>/state/board.sqlite ⇒ el CLI veía 0
    // tareas y 0 instancias.  Ahora: env > <PREFIX>/state > ~/.megadaemon.
    let state = arg("--state").unwrap_or_else(|| {
        std::env::var("MEGADAEMON_STATE_DIR")
            .ok()
            .filter(|d| !d.trim().is_empty())
            .unwrap_or_else(|| {
                if megadaemon_core::mesh::platform_is_android() {
                    "/data/local/tmp/megadaemon/state".into()
                } else {
                    match std::env::var("HOME") {
                        Ok(h) if !h.trim().is_empty() => format!("{}/.megadaemon", h.trim()),
                        _ => "/tmp/megadaemon".into(),
                    }
                }
            })
    });
    let board_path = format!("{}/board.sqlite", state);

    let result: Result<(), String> = (|| {
        match cmd.as_str() {
            "register" => {
                let device = arg("--device").unwrap_or_else(|| "desconocido".into());
                let platform = arg("--platform").unwrap_or_else(|| "linux-x86_64".into());
                let mut inst = Instance::load_or_register(&state, &device, &platform);
                if let Some(r) = arg("--role") {
                    let role = InstanceRole::parse(&r)
                        .ok_or_else(|| format!("rol desconocido: {}", r))?;
                    inst.role = role;
                }
                if has("--primary") {
                    inst.role = InstanceRole::Primary;
                }
                inst.save()?;
                let board = TaskBoard::open(&board_path)?;
                board.register_instance(&inst)?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "id": inst.id,
                        "role": inst.role.as_str(),
                        "device": inst.device,
                        "platform": inst.platform,
                        "version": inst.version,
                        "protocol": inst.protocol,
                        "state_dir": inst.state_dir,
                    }))
                    .unwrap_or_default()
                );
                Ok(())
            }
            "heartbeat" => {
                let board = TaskBoard::open(&board_path)?;
                let inst = Instance::load(&state).ok_or("no hay instance.json en el state_dir")?;
                let lease = arg("--lease")
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(900);
                board.heartbeat(&inst.id, lease)?;
                let reclaimed = board.reclaim_expired()?;
                println!(
                    "{}",
                    serde_json::json!({"heartbeat": inst.id, "reclaimed": reclaimed})
                );
                Ok(())
            }
            "add" => {
                let title = args.get(2).cloned().unwrap_or_default();
                let priority = arg("--priority")
                    .and_then(|v| v.parse::<i32>().ok())
                    .unwrap_or(5);
                let board = TaskBoard::open(&board_path)?;
                let id = board.add_task(&title, priority, arg("--owner").as_deref())?;
                println!("{}", serde_json::json!({"id": id}));
                Ok(())
            }
            "claim" => {
                let task_id = args.get(2).cloned().unwrap_or_default();
                let owner = arg("--owner").ok_or("falta --owner")?;
                let lease = arg("--lease")
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(900);
                let board = TaskBoard::open(&board_path)?;
                let won = board.claim_task(&task_id, &owner, lease)?;
                println!("{}", serde_json::json!({"task": task_id, "won": won}));
                Ok(())
            }
            "report" => {
                let task_id = args.get(2).cloned().unwrap_or_default();
                let owner = arg("--owner").ok_or("falta --owner")?;
                let ok = has("--ok");
                // El informe de 3 líneas puede venir de varias formas:
                //   --text "..." | --report "..." | --ok "..." | --ok (bandera)
                //   o posicional: report <id> "..." --owner x --ok
                // OJO: `args.get(3)` NO vale (con banderas detrás, la
                // posición 3 es "--owner" y el informe se perdía).
                let report = arg("--text")
                    .or_else(|| arg("--report"))
                    .or_else(|| arg("--ok-text"))
                    .or_else(|| {
                        // `--ok` puede actuar como bandera o端口ar el texto.
                        arg("--ok").filter(|v| !v.starts_with("--"))
                    })
                    .or_else(|| {
                        args.get(3)
                            .cloned()
                            .filter(|v| !v.starts_with("--"))
                    })
                    .unwrap_or_else(|| "sin informe".into());
                let board = TaskBoard::open(&board_path)?;
                board.report_task(&task_id, &owner, ok, &report)?;
                println!("{}", serde_json::json!({"task": task_id, "ok": ok}));
                Ok(())
            }
            "tasks" => {
                let board = TaskBoard::open(&board_path)?;
                let status = arg("--status").and_then(|s| {
                    match s.as_str() {
                        "queued" => Some(TaskStatus::Queued),
                        "claimed" => Some(TaskStatus::Claimed),
                        "done" => Some(TaskStatus::Done),
                        "failed" => Some(TaskStatus::Failed),
                        _ => None,
                    }
                });
                let tasks = board.list_tasks(status)?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&tasks).unwrap_or_default()
                );
                Ok(())
            }
            "instances" => {
                let board = TaskBoard::open(&board_path)?;
                let instances = board.all_instances()?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&instances).unwrap_or_default()
                );
                Ok(())
            }
            "migrate" => {
                let target = arg("--target").unwrap_or_else(|| env!("CARGO_PKG_VERSION").into());
                let board = TaskBoard::open(&board_path)?;
                let plan = board.migration_plan(&target)?;
                println!("{}", serde_json::to_string_pretty(&plan).unwrap_or_default());
                Ok(())
            }
            "snapshot" => {
                let board = TaskBoard::open(&board_path)?;
                let snap = board.snapshot()?;
                println!("{}", serde_json::to_string_pretty(&snap).unwrap_or_default());
                Ok(())
            }
            other => Err(format!("comando desconocido: {} ({})", other, usage())),
        }
    })();

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}
