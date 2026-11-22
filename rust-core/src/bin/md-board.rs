//! `md-board` — CLI del tablón unificado de tareas (Board v1).
//!
//! Un solo tablón para todo el enjambre: un formato, un espacio de ids, una
//! máquina de estados y un modelo de bloqueo (lease + dependencias).  Sirve
//! **sin el enjambre**: el usuario puede listar, reclamar y reportar tareas
//! desde su máquina.
//!
//! ```text
//! md-board --help
//! md-board --state-dir <dir> tasks [--state queued|claimed|running|done|failed] [--limit N] [--json]
//! md-board --state-dir <dir> ready [--limit N]
//! md-board --state-dir <dir> stats
//! md-board --state-dir <dir> events [--limit N]
//! md-board --state-dir <dir> add "<titulo>" [--id ID] [--kind K] [--priority N] [--capability C] [--depends a,b]
//! md-board --state-dir <dir> claim <id> --owner <agente> [--lease 900]
//! md-board --state-dir <dir> start <id> --owner <agente>
//! md-board --state-dir <dir> report <id> --owner <agente> (--ok "<msg>" | --fail "<msg>" | --cancel "<msg>")
//! md-board --state-dir <dir> renew <id> --owner <agente> [--lease 900]
//! md-board --state-dir <dir> reclaim
//! md-board --state-dir <dir> migrate [--mesh <f>] [--scheduler <f>] [--legacy-sqlite <f>] [--dry-run]
//! md-board --state-dir <dir> snapshot [--out <f>]
//! md-board --state-dir <dir> doctor
//! ```
//!
//! El tablón vive en `<state-dir>/board.sqlite`.  Todo es local: SQLite + JSON,
//! sin cuentas cloud ni formatos propietarios.

use megadaemon_core::board::{
    migrate_legacy_mesh, migrate_legacy_scheduler, migrate_legacy_sqlite, stats_json, tasks_json,
    BoardStore, Outcome, SqliteStore, Task, TaskFilter, TaskState, BOARD_FILE, DEFAULT_LEASE_SECS,
    SNAPSHOT_FILE,
};
use serde_json::json;
use std::path::{Path, PathBuf};
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

/// Positional `n` (0 = primero).  Las opciones con valor se saltan.
fn positional(n: usize) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let mut seen = 0usize;
    let mut skip = false;
    for a in args.iter().skip(1) {
        if skip {
            skip = false;
            continue;
        }
        if a.starts_with("--") {
            if !matches!(
                a.as_str(),
                "--json" | "--dry-run" | "--help" | "-h" | "--ok" | "--fail" | "--cancel"
            ) {
                skip = true;
            }
            continue;
        }
        if seen == n {
            return Some(a.clone());
        }
        seen += 1;
    }
    None
}

fn usage() -> &'static str {
    "uso: md-board <tasks|ready|stats|events|add|claim|start|report|renew|reclaim|migrate|snapshot|doctor> [opciones]

opciones globales:
  --state-dir <dir>   directorio de estado (por defecto, el del daemon)
  --json              salida JSON (tasks/stats/events)
  --help              esta ayuda

opciones por accion:
  tasks    --state <queued|claimed|running|done|failed>  --limit N
  ready    --limit N
  report   <id> --owner <agente> (--ok \"<msg>\" | --fail \"<msg>\" | --cancel \"<msg>\")
  add      \"<titulo>\" [--id ID] [--kind K] [--priority N] [--capability C] [--depends a,b]
  claim    <id> --owner <agente> [--lease 900]
  migrate  [--mesh <f>] [--scheduler <f>] [--legacy-sqlite <f>] [--dry-run]

El tablon vive en <state-dir>/board.sqlite (SQLite + JSON, local y abierto)."
}

fn state_dir() -> PathBuf {
    match arg("--state-dir") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(megadaemon_core::cockpit_server::state_dir()),
    }
}

fn board_path(dir: &Path) -> PathBuf {
    dir.join(BOARD_FILE)
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn die(msg: &str) -> ExitCode {
    eprintln!("{}", msg);
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 || has("--help") || has("-h") {
        println!("{}", usage());
        return ExitCode::SUCCESS;
    }
    let cmd = args[1].clone();
    let dir = state_dir();
    let path = board_path(&dir);
    let mut store = match SqliteStore::open(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("no se pudo abrir el tablon {}: {}", path.display(), e);
            eprintln!("hint: el modo degradado permite seguir trabajando con una cola local.");
            return ExitCode::from(2);
        }
    };

    match cmd.as_str() {
        "tasks" => {
            let limit = arg("--limit").and_then(|v| v.parse().ok()).unwrap_or(100);
            let mut filter = TaskFilter {
                limit,
                ..Default::default()
            };
            if let Some(s) = arg("--state") {
                filter.states.push(TaskState::parse(&s));
            }
            if has("--json") {
                println!("{}", tasks_json(&store, now(), limit));
            } else {
                let tasks = store.list_tasks(&filter).unwrap_or_default();
                println!("{:<24} {:<10} {:<14} {:<6} {}", "id", "estado", "dueno", "prio", "titulo");
                for t in &tasks {
                    println!(
                        "{:<24} {:<10} {:<14} {:<6} {}",
                        t.id,
                        t.state.as_str(),
                        t.owner.as_deref().unwrap_or("-"),
                        t.priority,
                        t.title
                    );
                }
            }
        }
        "ready" => {
            let limit = arg("--limit").and_then(|v| v.parse().ok()).unwrap_or(20);
            let tasks = store.ready_tasks(now(), limit).unwrap_or_default();
            if has("--json") {
                println!(
                    "{}",
                    json!({ "ready": tasks.iter().map(|t| t.id.clone()).collect::<Vec<_>>() })
                );
            } else {
                for t in &tasks {
                    println!("{}  prio={}  {}", t.id, t.priority, t.title);
                }
            }
        }
        "stats" => println!("{}", stats_json(&store, now())),
        "events" => {
            let limit = arg("--limit").and_then(|v| v.parse().ok()).unwrap_or(20);
            let evs = store.events(limit).unwrap_or_default();
            if has("--json") {
                println!("{}", json!({ "events": evs }));
            } else {
                for e in evs.iter().rev() {
                    println!("#{} {} {} {} {}", e.seq, e.ts, e.task_id, e.kind, e.actor);
                }
            }
        }
        "add" => {
            let title = positional(0).unwrap_or_default();
            if title.trim().is_empty() {
                return die("falta el titulo");
            }
            let n = store
                .list_tasks(&TaskFilter {
                    limit: 0,
                    ..Default::default()
                })
                .unwrap_or_default()
                .len();
            let id = arg("--id").unwrap_or_else(|| format!("t-{}-{}", now(), n));
            let mut t = Task::new(&id, &arg("--kind").unwrap_or_else(|| "task".into()), &title);
            t.priority = arg("--priority").and_then(|v| v.parse().ok()).unwrap_or(0);
            t.capability = arg("--capability").unwrap_or_default();
            t.payload = json!({ "origin": "md-board" });
            match store.put_task(&t) {
                Ok(()) => {
                    if let Some(deps) = arg("--depends") {
                        let list: Vec<String> = deps
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        let _ = store.set_deps(&id, &list);
                    }
                    println!("{}", id);
                }
                Err(e) => return die(&format!("error: {}", e)),
            }
        }
        "claim" => {
            let id = positional(0).unwrap_or_default();
            let owner = arg("--owner").unwrap_or_else(|| "cli".into());
            let lease = arg("--lease")
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_LEASE_SECS);
            match store.claim(&id, &owner, lease, now()) {
                Ok(t) => println!("{} {}", t.id, t.state.as_str()),
                Err(e) => return die(&format!("error: {}", e)),
            }
        }
        "start" => {
            let id = positional(0).unwrap_or_default();
            let owner = arg("--owner").unwrap_or_else(|| "cli".into());
            match store.start(&id, &owner, now()) {
                Ok(t) => println!("{} {}", t.id, t.state.as_str()),
                Err(e) => return die(&format!("error: {}", e)),
            }
        }
        "report" => {
            let id = positional(0).unwrap_or_default();
            let owner = arg("--owner").unwrap_or_else(|| "cli".into());
            let outcome = if let Some(m) = arg("--ok") {
                Outcome::Done {
                    result: json!({ "message": m }),
                }
            } else if let Some(m) = arg("--fail") {
                Outcome::Failed { error: m }
            } else if let Some(m) = arg("--cancel") {
                Outcome::Cancelled { reason: m }
            } else {
                return die("falta --ok, --fail o --cancel");
            };
            match store.report(&id, &owner, &outcome, now()) {
                Ok(t) => println!("{} {}", t.id, t.state.as_str()),
                Err(e) => return die(&format!("error: {}", e)),
            }
        }
        "renew" => {
            let id = positional(0).unwrap_or_default();
            let owner = arg("--owner").unwrap_or_else(|| "cli".into());
            let lease = arg("--lease")
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_LEASE_SECS);
            match store.renew_lease(&id, &owner, lease, now()) {
                Ok(()) => println!("{} lease renovado", id),
                Err(e) => return die(&format!("error: {}", e)),
            }
        }
        "reclaim" => {
            let ids = store.reclaim_expired(now()).unwrap_or_default();
            println!("{}", json!({ "reclaimed": ids }));
        }
        "migrate" => {
            let dry = has("--dry-run");
            let mut inserted = 0u64;
            let mut skipped = 0u64;
            let mut deps = 0u64;
            let mut warns: Vec<String> = Vec::new();
            if let Some(m) = arg("--mesh") {
                let raw = std::fs::read_to_string(&m).unwrap_or_default();
                let v: serde_json::Value = serde_json::from_str(&raw).unwrap_or(json!({}));
                if dry {
                    warns.push(format!(
                        "mesh {}: {} tareas (no migrado: dry-run)",
                        m,
                        v["tasks"].as_array().map(|a| a.len()).unwrap_or(0)
                    ));
                } else {
                    let r = migrate_legacy_mesh(&mut store, &v, now()).unwrap_or_default();
                    inserted += r.inserted as u64;
                    skipped += r.skipped as u64;
                    deps += r.deps as u64;
                    warns.extend(r.warnings);
                }
            }
            if let Some(q) = arg("--scheduler") {
                let raw = std::fs::read_to_string(&q).unwrap_or_default();
                let v: serde_json::Value = serde_json::from_str(&raw).unwrap_or(json!({}));
                if dry {
                    warns.push(format!(
                        "scheduler {}: {} tareas (no migrado: dry-run)",
                        q,
                        v["tasks"].as_array().map(|a| a.len()).unwrap_or(0)
                    ));
                } else {
                    let r = migrate_legacy_scheduler(&mut store, &v, now()).unwrap_or_default();
                    inserted += r.inserted as u64;
                    skipped += r.skipped as u64;
                    deps += r.deps as u64;
                    warns.extend(r.warnings);
                }
            }
            if let Some(l) = arg("--legacy-sqlite") {
                if dry {
                    warns.push(format!("legacy-sqlite {} (no migrado: dry-run)", l));
                } else {
                    let r = migrate_legacy_sqlite(&mut store, Path::new(&l), now()).unwrap_or_default();
                    inserted += r.inserted as u64;
                    skipped += r.skipped as u64;
                    deps += r.deps as u64;
                    warns.extend(r.warnings);
                }
            }
            let mut out = json!({
                "inserted": inserted,
                "skipped": skipped,
                "deps": deps,
                "dry_run": dry
            });
            if !warns.is_empty() {
                out["avisos"] = json!(warns);
            }
            println!("{}", out);
        }
        "snapshot" => {
            let out = arg("--out").unwrap_or_else(|| {
                dir.join(SNAPSHOT_FILE)
                    .display()
                    .to_string()
            });
            match store.snapshot_to(Path::new(&out)) {
                Ok(()) => println!("{}", out),
                Err(e) => return die(&format!("error: {}", e)),
            }
        }
        "doctor" => {
            let st = store.stats(now()).unwrap_or_default();
            println!(
                "{}",
                json!({
                    "ruta": path.display().to_string(),
                    "existe": path.exists(),
                    "journal_mode": st.journal_mode,
                    "schema_version": st.schema_version,
                    "tareas": st.total,
                    "leases_caducados": st.expired_leases,
                    "bloqueadas_por_deps": st.blocked_by_deps,
                    "snapshot": dir.join(SNAPSHOT_FILE).display().to_string(),
                })
            );
        }
        other => {
            eprintln!("accion desconocida: {}", other);
            eprintln!("{}", usage());
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_uso_documenta_las_acciones() {
        let u = usage();
        for a in [
            "tasks", "ready", "stats", "add", "claim", "report", "migrate", "doctor",
        ] {
            assert!(u.contains(a), "falta {} en el uso", a);
        }
    }

    #[test]
    fn la_ruta_del_tablon_va_en_el_state_dir() {
        let p = board_path(Path::new("/tmp/x"));
        assert!(p.ends_with(BOARD_FILE));
        assert!(p.starts_with("/tmp/x"));
    }
}
