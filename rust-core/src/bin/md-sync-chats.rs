//! `md-sync-chats` — CLI del motor de sincronización de CHATS.
//!
//! Sincroniza las conversaciones de los servicios que viven en el móvil
//! (WhatsApp, Telegram, Signal, Discord) y de los servicios SaaS agénticos
//! (ChatGPT, Claude, Gemini, DeepSeek, Kimi, Perplexity, Mistral, Copilot,
//! Poe y **Grok**) con un **modelo unificado**:
//!
//! ```text
//! conversación = { servicio, conversation_id, título, mensajes[{id,ts,autor,texto,tipo}],
//!                  origen (local_db|web_api|export|notificaciones),
//!                  nivel  (completo|metadatos|cifrado_no_descifrado) }
//! ```
//!
//! Uso:
//! ```text
//! md-sync-chats list [--json]
//! md-sync-chats pull --servicio <servicio> [--json]
//! md-sync-chats status [--json]
//! md-sync-chats ingest <fichero> [--json]
//! md-sync-chats migrate [--json]
//! md-sync-chats token --set <token-de-discord> | --show
//! ```
//!
//! El estado por defecto es `<state_dir>/sync_chats` (NDJSON por servicio +
//! `cursors.json`).  El token de Discord vive en
//! `<state_dir>/sync_chats/discord_token` con permisos `0600`: **nunca** en el
//! repositorio.
//!
//! Sin móvil (en el sandbox) los comandos responden con el estado real
//! ("sin acceso") en vez de inventar datos.

use megadaemon_core::sync_chats;
use serde_json::{json, Value};
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
    "uso: md-sync-chats <list|pull|status|ingest|migrate|token> [--servicio <s>] [--json]"
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("{}", usage());
        return ExitCode::from(2);
    }
    let cmd = args[1].clone();
    let json_out = has("--json");

    match cmd.as_str() {
        "list" => {
            let value = sync_chats::list_services();
            if json_out {
                println!("{}", serde_json::to_string_pretty(&value).unwrap_or_default());
            } else {
                print_list(&value);
            }
            ExitCode::SUCCESS
        }
        "pull" => {
            let Some(service) = arg("--servicio").or_else(|| arg("--service")) else {
                eprintln!("{}", usage());
                return ExitCode::from(2);
            };
            let value = sync_chats::pull(&service);
            if json_out {
                println!("{}", serde_json::to_string_pretty(&value).unwrap_or_default());
            } else {
                print_pull(&value);
            }
            // Un error real (no un "sin acceso") es código de salida 1.
            if value["ok"] == json!(false) && value["status"] == json!("error") {
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        "status" => {
            let value = sync_chats::status();
            if json_out {
                println!("{}", serde_json::to_string_pretty(&value).unwrap_or_default());
            } else {
                print_status(&value);
            }
            ExitCode::SUCCESS
        }
        "ingest" => {
            let Some(path) = args.get(2).cloned() else {
                eprintln!("{}", usage());
                return ExitCode::from(2);
            };
            let value = sync_chats::ingest_export(&path);
            if json_out {
                println!("{}", serde_json::to_string_pretty(&value).unwrap_or_default());
            } else {
                println!(
                    "ingesta: {} conversación(es), {} mensaje(s), {} nuevo(s)",
                    value["conversations"],
                    value["messages"],
                    value["new_messages"]
                );
            }
            if value["ok"] == json!(false) {
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        "migrate" => {
            // Trae al modelo unificado lo que ya se sincronizó con el motor
            // de IA (`crate::sync`), sin volver a descargarlo del móvil.
            let value = sync_chats::migrate_legacy();
            if json_out {
                println!("{}", serde_json::to_string_pretty(&value).unwrap_or_default());
            } else {
                println!(
                    "migradas {} conversación(es), {} mensaje(s) nuevo(s)",
                    value["migrated"], value["new_messages"]
                );
            }
            ExitCode::SUCCESS
        }
        "token" => {
            if let Some(tok) = arg("--set") {
                match sync_chats::write_discord_token(&tok) {
                    Ok(()) => {
                        println!("token de Discord guardado en {} (0600)", sync_chats::discord_token_path().display());
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("no se pudo guardar el token: {}", e);
                        ExitCode::from(1)
                    }
                }
            } else if has("--show") {
                match sync_chats::read_discord_token() {
                    Some(_) => println!("token de Discord: presente"),
                    None => println!("token de Discord: ausente"),
                }
                ExitCode::SUCCESS
            } else {
                eprintln!("{}", usage());
                ExitCode::from(2)
            }
        }
        _ => {
            eprintln!("{}", usage());
            ExitCode::from(2)
        }
    }
}

/// `list`: tabla de servicios con su origen y su nivel de acceso.
fn print_list(value: &Value) {
    let Some(items) = value["services"].as_array() else {
        println!("(sin servicios)");
        return;
    };
    println!(
        "{:<18} {:<20} {:<24} {}",
        "SERVICIO", "ORIGEN", "NIVEL", "QUÉ FALTA"
    );
    for it in items {
        println!(
            "{:<18} {:<20} {:<24} {}",
            it["service"].as_str().unwrap_or("?"),
            it["origin"].as_str().unwrap_or("?"),
            it["access"].as_str().unwrap_or("?"),
            it["missing"].as_str().unwrap_or("").chars().take(60).collect::<String>()
        );
    }
    println!("\n{} servicio(s).", items.len());
}

/// `pull`: resumen del pull de un servicio.
fn print_pull(value: &Value) {
    let svc = value["service"].as_str().unwrap_or("?");
    println!(
        "{}: estado={} origen={} nivel={}",
        svc,
        value["status"].as_str().unwrap_or("?"),
        value["origin"].as_str().unwrap_or("?"),
        value["access"].as_str().unwrap_or("?")
    );
    println!(
        "  conversaciones={} mensajes={} nuevas={} repetidas={} (añadidas={} actualizadas={})",
        value["conversations"],
        value["messages"],
        value["new_messages"],
        value["repeated_messages"],
        value["added"],
        value["updated"]
    );
    if let Some(cursor) = value.get("cursor") {
        println!(
            "  cursor: ts={} id={}",
            cursor["last_ts"], cursor["last_id"]
        );
    }
    if let Some(notes) = value["notes"].as_array() {
        for n in notes {
            println!("  nota: {}", n.as_str().unwrap_or(""));
        }
    }
    if let Some(errs) = value["errors"].as_array() {
        for e in errs {
            println!("  error: {}", e.as_str().unwrap_or(""));
        }
    }
}

/// `status`: almacén + cursores por servicio.
fn print_status(value: &Value) {
    println!("almacén: {}", value["store"].as_str().unwrap_or("?"));
    println!(
        "total: {} conversación(es), {} mensaje(s) verificado(s)",
        value["total_conversations"], value["total_messages"]
    );
    println!("token de Discord: {}", value["discord_token"].as_str().unwrap_or("?"));
    if let Some(cat) = value["catalog"].as_object() {
        println!("\n{:<18} {:<24} {:<10} {}", "SERVICIO", "NIVEL", "EN ALMACÉN", "CURSOR");
        for (svc, info) in cat {
            let cursor = info
                .get("cursor")
                .map(|c| {
                    let ts = c.get("last_ts").and_then(|v| v.as_i64()).unwrap_or(0);
                    if ts == 0 {
                        "-".to_string()
                    } else {
                        format!("ts={}", ts)
                    }
                })
                .unwrap_or_else(|| "-".to_string());
            println!(
                "{:<18} {:<24} {:<10} {}",
                svc,
                info["access"].as_str().unwrap_or("?"),
                info["in_store"],
                cursor
            );
        }
    }
}
