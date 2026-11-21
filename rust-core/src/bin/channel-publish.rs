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
        let (code, es_daemon, detalle) = channel::probe_url_detalle(&url);
        // `forbidden_body` del daemon: `reason` + `sent_key_fp` + `key_path`.
        // Con eso el watchdog sabe si es credencial sin abrir el móvil.
        let reason = detalle
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let fp = detalle
            .get("sent_key_fp")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let kpath = detalle
            .get("key_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        eprintln!(
            "channel-publish: {url} -> HTTP {code} cuerpo_daemon={es_daemon} muerto={} reason={reason} sent_key_fp={fp} key_path={kpath}",
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

    // ── Publicación en el REPO (URL canónica y estable) ──────────────────────
    // El paste rota en cada publicación ⇒ el sandbox no puede descubrir el
    // siguiente. El `raw` del repo **siempre** sirve la última versión ⇒ es el
    // punto canónico de descubrimiento. El token va de un fichero 0600 o del
    // entorno: nunca se imprime.
    if flag("--repo-publish") {
        let repo = arg("--repo").unwrap_or_else(|| "boolforge/megadaemon".to_string());
        let (owner, name) = repo.split_once('/').unwrap_or(("boolforge", "megadaemon"));
        let path = arg("--path").unwrap_or_else(|| "state/channel.json".to_string());
        let token = match arg("--token-file") {
            Some(p) => std::fs::read_to_string(p).ok().map(|s| s.trim().to_string()),
            None => std::env::var("MEGADAEMON_GITHUB_TOKEN").ok(),
        }
        .filter(|t| !t.is_empty());
        let Some(token) = token else {
            let msg = json!({
                "ok": false,
                "error": "no_github_token",
                "hint": "ponlo en un fichero 0600 (--token-file) o en MEGADAEMON_GITHUB_TOKEN",
            });
            println!("{}", serde_json::to_string_pretty(&msg).unwrap_or_default());
            return ExitCode::from(1);
        };
        let estado = channel::read_channel_json(state_dir.as_deref()).unwrap_or_else(|| json!({}));
        // Al repo va el payload PUBLICO (sólo URLs): el repo es público.
        let publico = channel::public_payload(&estado);
        let texto = serde_json::to_string_pretty(&publico).unwrap_or_default();
        // El `match` produce el código de salida (no puede ser una sentencia
        // whose arms devuelven ExitCode: eso no compila).
        let codigo = match channel::publish_to_repo(owner, name, &path, &token, &texto) {
            Some(out) => {
                println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
                if out.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                    ExitCode::from(0)
                } else {
                    ExitCode::from(1)
                }
            }
            None => {
                let msg = json!({ "ok": false, "error": "no_se_pudo_publicar_en_el_repo" });
                println!("{}", serde_json::to_string_pretty(&msg).unwrap_or_default());
                ExitCode::from(1)
            }
        };
        return codigo;
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
