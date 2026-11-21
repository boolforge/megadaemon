//! `kde-discover` — CLI del canal KDE Connect.
//!
//! Escucha el descubrimiento UDP (puertos 1716 oficial y 1776 del enjambre),
//! lista los nodos vecinos con sus capacidades y, opcionalmente, hace un
//! `ping` por el canal cifrado (TCP + TLS) contra uno de ellos.
//!
//! Uso:
//! ```text
//!   kde-discover                          # escucha 10 s y lista lo que ve
//!   kde-discover --timeout 30 --json
//!   kde-discover --broadcast --name "PC-casa"
//!   kde-discover --ping 192.168.1.50
//!   kde-discover --ping 192.168.1.50 --request /api/status
//! ```
//!
//! Códigos de salida: 0 = ok · 1 = error de uso o de red · 2 = se escuchó
//! pero no se vio ningún nodo.

use megadaemon_core::kdeconnect::{
    self, DeviceIdentity, Framing, Identity, PeerStore, Session, SessionConfig, TlsMaterial,
};
use serde_json::json;
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

fn uso() -> ! {
    eprintln!(
        "kde-discover — descubrimiento y ping del canal KDE Connect\n\
         \n\
         Opciones:\n\
         \x20 --timeout <seg>   segundos de escucha (por defecto 10)\n\
         \x20 --port <p>        puerto UDP a escuchar (por defecto: 1716 y 1776)\n\
         \x20 --broadcast       anuncia nuestra identidad mientras escucha\n\
         \x20 --name <nombre>   nombre del nodo local (por defecto: hostname)\n\
         \x20 --state-dir <dir> directorio de estado (por defecto el del daemon)\n\
         \x20 --json            salida en JSON\n\
         \x20 --ping <ip[:p]>   conecta por TCP+TLS y hace ping a ese nodo\n\
         \x20 --tcp-port <p>    puerto TCP del ping (por defecto, el anunciado)\n\
         \x20 --framing <f>     newline (oficial) o length (2 bytes BE)\n\
         \x20 --request <ruta>  con --ping: pide esa ruta al daemon del peer"
    );
    std::process::exit(1);
}

fn hostname_por_defecto() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_string())
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "nodo".to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut timeout = 10.0f64;
    let mut ports: Vec<u16> = kdeconnect::UDP_PORTS.to_vec();
    let mut broadcast = false;
    let mut name = hostname_por_defecto();
    let mut state_dir = megadaemon_core::cockpit_server::state_dir();
    let mut json_out = false;
    let mut ping_target: Option<String> = None;
    let mut tcp_port: Option<u16> = None;
    let mut framing = Framing::Newline;
    let mut request_path: Option<String> = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--timeout" => {
                i += 1;
                timeout = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(10.0);
            }
            "--port" => {
                i += 1;
                let p: u16 = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(0);
                if p == 0 {
                    uso();
                }
                ports = vec![p];
            }
            "--broadcast" => broadcast = true,
            "--name" => {
                i += 1;
                name = args.get(i).cloned().unwrap_or(name);
            }
            "--state-dir" => {
                i += 1;
                state_dir = args.get(i).cloned().unwrap_or(state_dir);
            }
            "--json" => json_out = true,
            "--ping" => {
                i += 1;
                ping_target = args.get(i).cloned();
            }
            "--tcp-port" => {
                i += 1;
                tcp_port = args.get(i).and_then(|s| s.parse().ok());
            }
            "--framing" => {
                i += 1;
                framing = match args.get(i).map(|s| s.as_str()) {
                    Some("newline") => Framing::Newline,
                    Some("length") => Framing::LengthPrefixed,
                    _ => uso(),
                };
            }
            "--request" => {
                i += 1;
                request_path = args.get(i).cloned();
            }
            "--help" | "-h" => uso(),
            _ => uso(),
        }
        i += 1;
    }

    // Identidad y material TLS (persistidos en el state_dir).
    let dev = match DeviceIdentity::load_or_create(&state_dir, &name) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("kde-discover: no se pudo preparar la identidad: {e}");
            std::process::exit(1);
        }
    };
    let identity: Identity = dev.to_identity("desktop", kdeconnect::TCP_PORT_DEFAULT);
    let material = match TlsMaterial::load_or_create(&state_dir, &dev.device_id) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("kde-discover: no se pudo preparar el certificado TLS: {e}");
            std::process::exit(1);
        }
    };
    let mut peers = PeerStore::open(&state_dir);
    let _ = peers.load();

    // 1) Descubrimiento.
    let sock = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("kde-discover: no se pudo abrir el socket UDP: {e}");
            std::process::exit(1);
        }
    };
    let _ = sock.set_broadcast(true);

    let mut vistos = Vec::new();
    if ping_target.is_none() || broadcast {
        // Escucha en los puertos indicados.
        let mut escuchas: Vec<(u16, UdpSocket)> = Vec::new();
        for p in &ports {
            match UdpSocket::bind(("0.0.0.0", *p)) {
                Ok(s) => {
                    let _ = s.set_read_timeout(Some(Duration::from_millis(250)));
                    let _ = s.set_broadcast(true);
                    escuchas.push((*p, s));
                }
                Err(e) => {
                    eprintln!("kde-discover: aviso: no se pudo escuchar en el puerto {p}: {e}");
                }
            }
        }
        if broadcast {
            let _ = kdeconnect::broadcast_discovery(&sock, &identity, identity.tcp_port, &ports);
        }
        let deadline = std::time::Instant::now() + Duration::from_secs_f64(timeout);
        let mut buf = [0u8; 8192];
        while std::time::Instant::now() < deadline {
            for (port, s) in escuchas.iter() {
                if let Ok((n, from)) = s.recv_from(&mut buf) {
                    let Some(pkt) = kdeconnect::Packet::from_slice(&buf[..n]) else {
                        continue;
                    };
                    let Some(id) = Identity::from_packet(&pkt) else {
                        continue;
                    };
                    if id.device_id == identity.device_id {
                        continue;
                    }
                    if vistos.iter().any(|v: &kdeconnect::DiscoveredPeer| {
                        v.identity.device_id == id.device_id
                    }) {
                        continue;
                    }
                    vistos.push(kdeconnect::DiscoveredPeer {
                        identity: id,
                        addr: from,
                        udp_port: *port,
                        seen_at: kdeconnect::now_epoch(),
                    });
                }
            }
            if broadcast {
                // Se renueva el anuncio cada 15 s (como el beacon LAN).
                let _ = kdeconnect::broadcast_discovery(&sock, &identity, identity.tcp_port, &ports);
            }
        }
    }

    // 2) Ping opcional por el canal cifrado.
    let mut ping_json = serde_json::Value::Null;
    if let Some(target) = ping_target {
        let (host, puerto) = match target.rsplit_once(':') {
            Some((h, p)) => (h.to_string(), p.parse::<u16>().unwrap_or(0)),
            None => (target.clone(), 0),
        };
        // El id del peer: del descubrimiento, si lo vimos; si no, vacío (el
        // peer puede aceptarlo vacío en el primer paquete).
        let device_id = vistos
            .iter()
            .find(|v| v.addr.ip().to_string() == host)
            .map(|v| v.identity.device_id.clone())
            .unwrap_or_default();
        let puerto = match tcp_port.or(Some(puerto)) {
            Some(p) if p > 0 => p,
            _ => vistos
                .iter()
                .find(|v| v.addr.ip().to_string() == host)
                .map(|v| v.identity.tcp_port)
                .unwrap_or(kdeconnect::TCP_PORT_DEFAULT),
        };
        let addr: SocketAddr = format!("{host}:{puerto}")
            .parse()
            .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], puerto)));

        let cfg = SessionConfig {
            framing: Some(framing),
            timeout: Duration::from_secs(8),
        };
        let resultado = Session::connect(addr, &device_id, &identity, &material, &peers, &cfg).and_then(
            |mut s| {
                let emparejado = s.trusted;
                let mut out = json!({
                    "peer": s.peer.device_name,
                    "device_id": s.peer.device_id,
                    "paired": emparejado,
                    "framing": s.framing.name(),
                });
                if emparejado {
                    let rtt = s.ping()?;
                    out["ping_ms"] = json!(rtt.as_secs_f64() * 1000.0);
                    if let Some(path) = &request_path {
                        let resp = s.request("GET", path, None)?;
                        out["request"] = resp;
                    }
                } else {
                    out["note"] = json!("el peer NO está emparejado: no responde al ping");
                }
                Ok(out)
            },
        );
        ping_json = match resultado {
            Ok(v) => v,
            Err(e) => json!({ "error": e }),
        };
    }

    // 3) Salida.
    let vecinos: Vec<serde_json::Value> = vistos
        .iter()
        .map(|v| {
            json!({
                "device_id": v.identity.device_id,
                "name": v.identity.device_name,
                "type": v.identity.device_type,
                "protocol": v.identity.protocol_version,
                "app": v.identity.app_name,
                "addr": v.addr.ip().to_string(),
                "udp_port": v.udp_port,
                "tcp_port": v.identity.tcp_port,
                "incoming_capabilities": v.identity.incoming_capabilities,
                "outgoing_capabilities": v.identity.outgoing_capabilities,
                "paired": peers.is_trusted(&v.identity.device_id),
            })
        })
        .collect();

    if json_out {
        let mut out = json!({
            "ok": !vecinos.is_empty() || !ping_json.is_null(),
            "self": {
                "device_id": identity.device_id,
                "name": identity.device_name,
                "cert_fingerprint": material.fingerprint,
            },
            "listening_ports": ports,
            "neighbors": vecinos,
        });
        if !ping_json.is_null() {
            out["ping"] = ping_json.clone();
        }
        println!("{}", out);
    } else {
        println!("nodo local: {} ({})", identity.device_name, identity.device_id);
        println!("certificado: {}", material.fingerprint);
        if vecinos.is_empty() {
            println!("vecinos: ninguno en {} s", timeout);
        } else {
            println!("vecinos: {}", vecinos.len());
            for v in &vecinos {
                println!(
                    "  · {} [{}] {} · tcp {} · pareado={}",
                    v["name"].as_str().unwrap_or("?"),
                    v["device_id"].as_str().unwrap_or("?"),
                    v["addr"].as_str().unwrap_or("?"),
                    v["tcp_port"],
                    v["paired"]
                );
                let caps = v["incoming_capabilities"].as_array().cloned().unwrap_or_default();
                let caps: Vec<&str> = caps
                    .iter()
                    .filter_map(|c| c.as_str())
                    .collect();
                if !caps.is_empty() {
                    println!("      capacidades: {}", caps.join(", "));
                }
            }
        }
        if !ping_json.is_null() {
            if let Some(err) = ping_json.get("error").and_then(|e| e.as_str()) {
                println!("ping: ERROR {err}");
            } else {
                println!(
                    "ping: {} ms (pareado={})",
                    ping_json
                        .get("ping_ms")
                        .and_then(|v| v.as_f64())
                        .map(|v| format!("{v:.1}"))
                        .unwrap_or_else(|| "-".into()),
                    ping_json.get("paired").and_then(|v| v.as_bool()).unwrap_or(false)
                );
                if let Some(req) = ping_json.get("request") {
                    println!("md.request: {}", req);
                }
            }
        }
    }

    if vecinos.is_empty() && ping_json.is_null() {
        std::process::exit(2);
    }
}
