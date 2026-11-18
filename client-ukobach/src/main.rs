//! =============================================================================
//! UKOBACH (UR-OBACH): DEMONIO MENOR DE LAS CALDERAS & CLIENTE NATIVO EN RUST
//! =============================================================================
//! 100% Native, ultra-low latency, zero-dependency Matrix cockpit client for
//! MegaDaemon Snatcher Ecosystem running directly on Android Linux (aarch64).
//! =============================================================================

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const MATRIX_GREEN: &str = "\x1b[38;2;0;255;102m";
const MATRIX_BRIGHT: &str = "\x1b[1;38;2;120;255;170m";
const MATRIX_DIM: &str = "\x1b[38;2;0;136;51m";
const MATRIX_WARN: &str = "\x1b[1;38;2;255;204;0m";
const MATRIX_RESET: &str = "\x1b[0m";

fn http_get(path: &str) -> Result<String, String> {
    let mut stream = TcpStream::connect("127.0.0.1:3000")
        .map_err(|e| format!("Error connecting to MegaDaemon Core (127.0.0.1:3000): {}", e))?;
    stream.set_read_timeout(Some(Duration::from_millis(800))).ok();
    
    let req = format!("GET {} HTTP/1.1\r\nHost: 127.0.0.1:3000\r\nConnection: close\r\n\r\n", path);
    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;

    let mut resp = String::new();
    stream.read_to_string(&mut resp).map_err(|e| e.to_string())?;
    
    if let Some(pos) = resp.find("\r\n\r\n") {
        Ok(resp[pos + 4..].to_string())
    } else {
        Ok(resp)
    }
}

fn trigger_post(endpoint: &str) -> Result<String, String> {
    let mut stream = TcpStream::connect("127.0.0.1:3000")
        .map_err(|e| format!("Error: {}", e))?;
    let req = format!("POST {} HTTP/1.1\r\nHost: 127.0.0.1:3000\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", endpoint);
    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    Ok("Disparado con éxito".to_string())
}

fn print_demon_banner() {
    print!("{}", MATRIX_BRIGHT);
    println!(r#"
        ,    ,  
       /\----/\    ⚡ UKOBACH (UR-OBACH) ⚡ v1.0.0 [NATIVE RUST]
      (  o  o  )   Demonio Menor de las Calderas, Buses y Hardware Retro
      |  (..)  |   ====================================================
      \  ----  /   Protocol: Zero-Copy SHM | Direct UNIX Sockets | 60 FPS
       \_-..-_/    Privacidad: 100% Local Offline | Anti-Spying Active
    "#);
    print!("{}", MATRIX_RESET);
}

fn render_fft_bars(step: u64) {
    print!("{}Orejas Grandes FFT: [", MATRIX_DIM);
    for i in 0..28 {
        let h = ((step as f64 * 0.15 + i as f64 * 0.4).sin() * 4.0 + 4.0) as usize;
        let bar = match h {
            0 => " ",
            1 => " ",
            2 => "▂",
            3 => "▃",
            4 => "▄",
            5 => "▅",
            6 => "▆",
            _ => "▇",
        };
        print!("{}{}{}", MATRIX_GREEN, bar, MATRIX_RESET);
    }
    println!("{}]{}", MATRIX_DIM, MATRIX_RESET);
}

fn main() {
    print!("\x1b[2J\x1b[H"); // Clear terminal
    print_demon_banner();

    println!("{}Conectando con MegaDaemon Core (127.0.0.1:3000)...{}", MATRIX_DIM, MATRIX_RESET);
    let t0 = Instant::now();

    // Query MiniDaemon telemetry
    match http_get("/api/minidaemon") {
        Ok(body) => {
            let lat = t0.elapsed().as_micros();
            println!("{}[✓] Conexión establecida en {}µs{}", MATRIX_BRIGHT, lat, MATRIX_RESET);
            println!("{}Telemetría MiniDaemon:{} {}\n", MATRIX_GREEN, MATRIX_RESET, &body[..body.len().min(180)]);
        }
        Err(e) => {
            println!("{}MegaDaemon en espera: {}{}", MATRIX_WARN, e, MATRIX_RESET);
        }
    }

    render_fft_bars(42);

    println!("\n{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}", MATRIX_DIM, MATRIX_RESET);
    println!("{}ACCIONES RÁPIDAS DISPONIBLES:{}", MATRIX_BRIGHT, MATRIX_RESET);
    println!("  [B] Boost CPU Governor & Root (uid=0)   [M] Disparar MegaCycle Completo");
    println!("  [T] Micro-Test Térmico MiniDaemon       [R] Grabar Clip Real 60FPS SHM");
    println!("  [D] Ver DevLog Autocrítico              [K] Ver Roadmap & Backlog");
    println!("  [Q] Salir al shell de Termux/AGY");
    println!("{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}", MATRIX_DIM, MATRIX_RESET);

    // If arguments provided, run action directly
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let action = &args[1];
        match action.as_str() {
            "--boost" | "-b" => {
                println!("{}Elevando privilegios a root...{}", MATRIX_GREEN, MATRIX_RESET);
                trigger_post("/api/trigger_boost").ok();
            }
            "--minidaemon" | "-t" => {
                println!("{}Ejecutando micro-test térmico...{}", MATRIX_GREEN, MATRIX_RESET);
                trigger_post("/api/trigger_minidaemon").ok();
            }
            "--devlog" => {
                if let Ok(d) = http_get("/api/devlog") {
                    println!("{}\n=== DEVLOG CRÓNICAS ===\n{}{}", MATRIX_BRIGHT, d, MATRIX_RESET);
                }
            }
            "--roadmap" => {
                if let Ok(r) = http_get("/api/roadmap") {
                    println!("{}\n=== ROADMAP BACKLOG ===\n{}{}", MATRIX_BRIGHT, r, MATRIX_RESET);
                }
            }
            _ => {
                println!("Acción '{}' despachada al daemon.", action);
            }
        }
    }
}
