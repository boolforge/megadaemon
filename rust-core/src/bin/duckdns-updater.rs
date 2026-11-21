//! `duckdns-updater` — mantiene el dominio **DuckDNS** apuntando a la IP
//! pública de la casa, para que el enjambre tenga un **canal estable** que no
//! rota en cada reinicio del móvil (a diferencia del *quick tunnel*).
//!
//! ```text
//! duckdns-updater --once                 # una actualización y salir
//! duckdns-updater --interval 300         # bucle cada 5 minutos
//! duckdns-updater --status                # sólo informa del estado
//! ```
//!
//! El token se lee de un **fichero 0600** (`--token-file`, por defecto
//! `/data/local/tmp/megadaemon/duckdns.token`). Nunca se imprime, ni se
//! guarda en el estado, ni va en la línea de órdenes.

use megadaemon_core::duckdns;
use serde_json::json;
use std::time::Duration;

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

fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}

/// Dominio: `--domain` > `instance.json` > constante por defecto.
fn resolve_domain(explicit: Option<&str>, instance_file: Option<&str>) -> String {
    if let Some(d) = explicit {
        if !d.trim().is_empty() {
            return d.trim().to_string();
        }
    }
    if let Some(f) = instance_file {
        if let Ok(txt) = std::fs::read_to_string(f) {
            if let Some(d) = duckdns::domain_from_instance(&txt) {
                return d;
            }
        }
    }
    duckdns::DEFAULT_DOMAIN.to_string()
}

fn main() -> std::process::ExitCode {
    let token_file = arg("--token-file");
    let state_file = arg("--state-file");
    let instance_file = arg("--instance");
    let domain = resolve_domain(arg("--domain").as_deref(), instance_file.as_deref());
    let ip_explicita = arg("--ip");

    if flag("--status") {
        let st = duckdns::status(state_file.as_deref(), token_file.as_deref());
        println!("{}", st);
        return std::process::ExitCode::from(if st["token_present"].as_bool() == Some(true) {
            0
        } else {
            1
        });
    }

    let intervalo = arg("--interval")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(300)
        .max(30);

    // El token: fichero 0600. Si no está, se avisa con el motivo (y se sale).
    let token = match duckdns::read_token(&std::path::PathBuf::from(
        token_file.clone().unwrap_or_else(|| duckdns::DEFAULT_TOKEN_FILE.to_string()),
    )) {
        Some(t) => t,
        None => {
            let msg = json!({
                "ok": false,
                "error": "token_missing_or_not_0600",
                "hint": "crea el fichero con el token de DuckDNS y chmod 600 (nunca en el repo)",
                "domain": domain,
            });
            println!("{msg}");
            return std::process::ExitCode::from(1);
        }
    };

    let una_vez = flag("--once");
    let state_dir = arg("--state-dir");
    loop {
        let ip = match ip_explicita.clone() {
            Some(i) => Some(i),
            None => duckdns::public_ip(),
        };
        let res = match ip {
            Some(ip) => {
                let r = duckdns::update_once(&domain, &token, &ip, state_file.as_deref());
                // URL viva del canal estable: se publica para que nadie la
                // hardcodee (la leen de `channel.url`).
                let ok = r.get("ok").and_then(|v| v.as_bool()) == Some(true);
                let saltado = r.get("skipped").is_some();
                if ok {
                    let url_estable = format!("https://{domain}");
                    let _ = duckdns::write_channel_url(state_dir.as_deref(), &url_estable);
                    // Estado completo del canal (lo lee el sandbox).
                    let _ = megadaemon_core::channel::merge_channel_field(
                        state_dir.as_deref(),
                        "duckdns",
                        serde_json::json!(url_estable),
                    );
                    let _ = megadaemon_core::channel::merge_channel_field(
                        state_dir.as_deref(),
                        "ip_publica",
                        serde_json::json!(ip),
                    );
                    // El paste se publica SOLO cuando la URL cambia de verdad.
                    if !saltado && flag("--publish") {
                        let _ = megadaemon_core::channel::publish(
                            state_dir.as_deref(),
                            None,
                            Some(&url_estable),
                            Some(&ip),
                            None,
                        );
                    }
                }
                r
            }
            None => json!({
                "ok": false,
                "error": "no_public_ip",
                "domain": domain,
                "detail": "no se pudo descubrir la IP pública",
            }),
        };
        println!("{res}");
        if una_vez {
            let ok = res["ok"].as_bool().unwrap_or(false);
            return std::process::ExitCode::from(if ok { 0 } else { 1 });
        }
        std::thread::sleep(Duration::from_secs(intervalo));
    }
}
