//! `channel-publish` — publica el estado del canal **fuera del móvil**.
//!
//! El móvil **no tiene `curl` ni `wget`**: por eso la publicación la hace
//! este binario (que usa el cliente HTTP del propio crate, `ureq`) y no un
//! script de shell. Es la misma lógica que la tool MCP `channel_publish`, así
//! que el watchdog puede llamarla **sin el daemon** (ni que esté vivo el
//! túnel).
//!
//! ```text
//! channel-publish [--state-dir <dir>] [--quick <url>] [--duckdns <url>]
//!                  [--ip <ip>] [--pids "<pids>"] [--print-only]
//! ```
//!
//! Qué hace:
//!   1. escribe `tunnel.url` (quick) y `channel.url` (DuckDNS) si se pasan;
//!   2. fusiona el estado en `channel.json`
//!      (`duckdns`, `quick`, `ip_publica`, `cloudflared_pids`, `ts`);
//!   3. **publica ese JSON en un paste público** ⇒ el sandbox lo lee aunque
//!      el túnel esté caído; el paste incluye la URL del paste anterior ⇒
//!      la cadena se renueva sola;
//!   4. escribe la URL del paste dentro de `channel.json` y la imprime.
//!
//! **Publica una URL del dispositivo en un sitio público**: por eso el
//! watchdog sólo lo llama cuando la URL ha cambiado de verdad.

use megadaemon_core::channel;
use serde_json::json;
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

fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}

fn main() -> ExitCode {
    let state_dir = arg("--state-dir");
    let quick = arg("--quick").filter(|s| !s.trim().is_empty());
    let duckdns = arg("--duckdns").filter(|s| !s.trim().is_empty());
    let ip = arg("--ip").filter(|s| !s.trim().is_empty());
    let pids = arg("--pids").filter(|s| !s.trim().is_empty());

    // ── Modo sondeo: stdout = el código HTTP (para el watchdog), stderr = detalle.
    //    El móvil no tiene curl: el sondeo lo hace este binario.
    if let Some(url) = arg("--probe").filter(|s| !s.trim().is_empty()) {
        let (code, es_daemon) = channel::probe_url(&url);
        eprintln!(
            "channel-publish: {url} -> HTTP {code} ({}) cuerpo_daemon={es_daemon} muerto={}",
            if es_daemon { "respondio" } else { "sin cuerpo" },
            channel::es_canal_muerto(code)
        );
        println!("{code}");
        return ExitCode::from(0);
    }

    // Si no nos pasan nada, se usa lo que ya haya en el estado.
    let actual = channel::read_channel_json(state_dir.as_deref());
    let quick = quick.or_else(|| {
        actual
            .as_ref()
            .and_then(|v| v.get("quick"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    });
    let duckdns = duckdns.or_else(|| {
        actual
            .as_ref()
            .and_then(|v| v.get("duckdns"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    });

    if flag("--print-only") {
        let estado = actual.unwrap_or_else(|| json!({}));
        println!("{}", serde_json::to_string_pretty(&estado).unwrap_or_default());
        return ExitCode::from(0);
    }

    let out = channel::publish(
        state_dir.as_deref(),
        quick.as_deref(),
        duckdns.as_deref(),
        ip.as_deref(),
        pids.as_deref(),
    );
    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
    ExitCode::from(if out.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        0
    } else {
        1
    })
}
