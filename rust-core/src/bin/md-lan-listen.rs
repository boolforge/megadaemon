//! `md-lan-listen` — descubre el móvil por el canal LAN (sin internet).
//!
//! Escucha el beacon UDP que emite el daemon y, si lo encuentra, comprueba
//! que el cockpit responde por HTTP.  Pensado para el PC de casa y para el
//! sandbox, que están en la misma red que el móvil.
//!
//! Uso:
//!     md-lan-listen                  # escucha 15 s y muestra lo que encuentre
//!     md-lan-listen --timeout 60     # espera 60 s
//!     md-lan-listen --json           # salida JSON (para scripts)
//!     md-lan-listen --probe          # además comprueba /health por HTTP
//!
//! Códigos de salida: 0 = móvil encontrado (y vivo si --probe) · 1 = no
//! encontrado o no responde.

use std::time::Duration;

fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == name {
            return it.next().cloned();
        }
    }
    None
}

fn main() -> std::process::ExitCode {
    let timeout = arg("--timeout")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(15)
        .max(1);
    let json = flag("--json");
    let probe = flag("--probe");

    let beacon = match megadaemon_core::lan::listen_beacons(Duration::from_secs(timeout)) {
        Some(b) => b,
        None => {
            if json {
                println!(
                    "{{\"ok\":false,\"error\":\"no_beacon\",\"timeout_s\":{timeout}}}"
                );
            } else {
                println!("NO se ha encontrado el móvil por el canal LAN en {timeout} s");
                println!("  · ¿está el móvil en la misma subred? (la difusión UDP no cruza routers)");
                println!("  · ¿está el daemon vivo? (ps | grep megadaemon-core)");
            }
            return std::process::ExitCode::from(1);
        }
    };

    let url = beacon
        .url
        .clone()
        .unwrap_or_else(|| megadaemon_core::lan::lan_url(&beacon.ips[0], beacon.port));

    // Comprobación de vida por HTTP (opcional).
    let mut http_code = 0u16;
    let mut vivo = false;
    if probe {
        // HTTP mínimo con std (sin dependencias): GET /health.
        let host = url
            .trim_start_matches("http://")
            .trim_end_matches('/')
            .to_string();
        if let Some((h, p)) = host.rsplit_once(':') {
            let p: u16 = p.parse().unwrap_or(80);
            if let Ok(s) = std::net::TcpStream::connect((h, p)) {
                use std::io::{Read, Write};
                let mut s = s;
                let req = format!("GET /health HTTP/1.0\r\nHost: {h}\r\nConnection: close\r\n\r\n");
                if s.write_all(req.as_bytes()).is_ok() {
                    let mut buf = [0u8; 512];
                    if let Ok(n) = s.read(&mut buf) {
                        let txt = String::from_utf8_lossy(&buf[..n]);
                        if let Some(line) = txt.lines().next() {
                            let mut it = line.split_whitespace();
                            let _ = it.next();
                            if let Some(code) = it.next().and_then(|c| c.parse::<u16>().ok()) {
                                http_code = code;
                                vivo = (200..400).contains(&code);
                            }
                        }
                    }
                }
            }
        }
    }

    if json {
        println!(
            "{{\"ok\":true,\"id\":\"{}\",\"ip\":\"{}\",\"ips\":{:?},\"port\":{},\"url\":\"{}\",\"http\":{},\"vivo\":{}}}",
            beacon.id,
            beacon.ips.first().cloned().unwrap_or_default(),
            beacon.ips,
            beacon.port,
            url,
            http_code,
            vivo
        );
    } else {
        println!("móvil encontrado por LAN: {}", beacon.id);
        println!("  IPs: {:?}", beacon.ips);
        println!("  URL: {url}");
        if probe {
            if vivo {
                println!("  /health -> HTTP {http_code} (vivo)");
            } else {
                println!("  /health -> HTTP {http_code} (NO responde)");
            }
        }
    }

    if probe && !vivo {
        return std::process::ExitCode::from(1);
    }
    std::process::ExitCode::from(0)
}
