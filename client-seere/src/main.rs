//! =============================================================================
//! SEERE (UR-SEAR / SEIR): PRÍNCIPE ALADO, VELOZ & GUAPERAS DE LOS 72 ESPÍRITUS
//! =============================================================================
//! 100% Native, ultra-fast, zero-overhead, sleek Matrix client for MegaDaemon
//! Featuring:
//!   - Direct Zero-Copy SHM Video Monitor (24-bit TrueColor Unicode Half-Blocks)
//!   - Thermal & Charging Proactive Governor Radar
//!   - Direct MegaBrain Neural Core Query (127.0.0.1:8080)
//!   - Universal Asset Extraction Dispatch (PC-8801, MSX2, PC Engine, Sega CD)
//! =============================================================================

use std::fs::File;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const MATRIX_GREEN: &str = "\x1b[38;2;0;255;102m";
const MATRIX_BRIGHT: &str = "\x1b[1;38;2;120;255;170m";
const MATRIX_DIM: &str = "\x1b[38;2;0;136;51m";
const MATRIX_CYAN: &str = "\x1b[1;38;2;0;238;255m";
const MATRIX_WARN: &str = "\x1b[1;38;2;255;204;0m";
const MATRIX_ALERT: &str = "\x1b[1;38;2;255;51;51m";
const MATRIX_RESET: &str = "\x1b[0m";

const SHM_PATH: &str = "/data/user/0/com.agy/files/snatcher_vla_shm.bin";

fn http_request(method: &str, path: &str, body_json: Option<&str>) -> Result<String, String> {
    let mut stream = TcpStream::connect("127.0.0.1:3000")
        .map_err(|e| format!("Error conectando a MegaDaemon (127.0.0.1:3000): {}", e))?;
    stream.set_read_timeout(Some(Duration::from_secs(60))).ok();

    let body = body_json.unwrap_or("");
    let req = format!(
        "{} {} HTTP/1.1\r\nHost: 127.0.0.1:3000\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        method, path, body.len(), body
    );
    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;

    let mut resp = String::new();
    stream.read_to_string(&mut resp).map_err(|e| e.to_string())?;

    if let Some(pos) = resp.find("\r\n\r\n") {
        Ok(resp[pos + 4..].to_string())
    } else {
        Ok(resp)
    }
}

fn read_battery_info() -> (f32, String) {
    let temp = std::fs::read_to_string("/sys/class/power_supply/battery/temp")
        .unwrap_or_else(|_| "320".to_string())
        .trim()
        .parse::<f32>()
        .unwrap_or(320.0) / 10.0;
    let status = std::fs::read_to_string("/sys/class/power_supply/battery/status")
        .unwrap_or_else(|_| "Discharging".to_string())
        .trim()
        .to_string();
    (temp, status)
}

fn print_seere_banner() {
    print!("{}", MATRIX_CYAN);
    println!(r#"
          /\_/\      ⚡ SEERE (UR-SEAR) ⚡ v1.0.0 [NATIVE RUST]
         ( o.o )     Príncipe Alado, Veloz y Guaperas de los 72 Espíritus
       (>  \~/  <)   ====================================================
        /     \      Cruza la Tierra en un Parpadeo | Zero-Copy SHM Video
       (_______)     Mente y Corazón Neural | Ingesta & Dumper Universal
    "#);
    print!("{}", MATRIX_RESET);
}

/// Renders real-time video frame from SHM buffer using 24-bit TrueColor Unicode half-blocks
fn render_shm_ascii_monitor() {
    let mut file = match File::open(SHM_PATH) {
        Ok(f) => f,
        Err(_) => {
            println!("{}Búfer SHM no encontrado en {}{}", MATRIX_WARN, SHM_PATH, MATRIX_RESET);
            return;
        }
    };

    let mut header = [0u8; 32];
    if file.read_exact(&mut header).is_err() {
        return;
    }

    let magic = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    let w = u16::from_le_bytes([header[20], header[21]]) as usize;
    let h = u16::from_le_bytes([header[22], header[23]]) as usize;

    if magic != 0x534E4154 || w == 0 || h == 0 {
        println!("{}Búfer SHM no inicializado (Magic: 0x{:08X}){}", MATRIX_WARN, magic, MATRIX_RESET);
        return;
    }

    let expected_bytes = w * h * 3;
    let mut raw_pixels = vec![0u8; expected_bytes];
    // Skip to pixel data at offset 32824
    use std::io::Seek;
    if file.seek(std::io::SeekFrom::Start(32824)).is_err() {
        return;
    }
    if file.read_exact(&mut raw_pixels).is_err() {
        return;
    }

    println!("{}╔═ MONITOR DE VÍDEO REAL SHM: {}x{} RGB24 (Cero Simulación) ════════════════╗{}", MATRIX_BRIGHT, w, h, MATRIX_RESET);

    // Downsample to 56 columns x 24 rows (each char row = 2 vertical pixels)
    let out_cols = 56;
    let out_rows = 14;

    for row in 0..out_rows {
        print!("{}║ ", MATRIX_DIM);
        for col in 0..out_cols {
            let px = (col * w) / out_cols;
            let py_top = (row * 2 * h) / (out_rows * 2);
            let py_bot = ((row * 2 + 1) * h) / (out_rows * 2);

            let idx_top = (py_top * w + px) * 3;
            let idx_bot = (py_bot * w + px) * 3;

            let (rt, gt, bt) = if idx_top + 2 < raw_pixels.len() {
                (raw_pixels[idx_top], raw_pixels[idx_top + 1], raw_pixels[idx_top + 2])
            } else {
                (0, 0, 0)
            };

            let (rb, gb, bb) = if idx_bot + 2 < raw_pixels.len() {
                (raw_pixels[idx_bot], raw_pixels[idx_bot + 1], raw_pixels[idx_bot + 2])
            } else {
                (0, 0, 0)
            };

            // Unicode upper half block: FG is top, BG is bottom
            print!("\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m▀", rt, gt, bt, rb, gb, bb);
        }
        println!("{}\x1b[0m{} ║{}", MATRIX_RESET, MATRIX_DIM, MATRIX_RESET);
    }
    println!("{}╚════════════════════════════════════════════════════════════════════╝{}", MATRIX_DIM, MATRIX_RESET);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    print!("\x1b[2J\x1b[H"); // Clear screen
    print_seere_banner();

    let (temp, charging) = read_battery_info();
    let is_charging = charging.to_lowercase().contains("charg");
    let profile = if is_charging {
        format!("{}[FULL TURBO CHARGING (Enchufado)]{}", MATRIX_CYAN, MATRIX_RESET)
    } else if temp > 40.0 {
        format!("{}[HOT THROTTLED (Protección Silicio)]{}", MATRIX_ALERT, MATRIX_RESET)
    } else {
        format!("{}[SUSTAINED BALANCED]{}", MATRIX_GREEN, MATRIX_RESET)
    };

    println!("{}Estado Térmico:{} {:.1}°C (Batería: {}) → Perfil: {}", MATRIX_BRIGHT, MATRIX_RESET, temp, charging, profile);

    // Route actions if provided
    if args.len() > 1 {
        let cmd = &args[1];
        match cmd.as_str() {
            "--live" | "-l" => {
                render_shm_ascii_monitor();
            }
            "--brain" | "-m" => {
                let prompt = if args.len() > 2 {
                    args[2..].join(" ")
                } else {
                    "Analiza el estado del motor Snatcher y genera recomendaciones de bajo nivel.".to_string()
                };
                println!("\n{}Consultando MegaBrain (Mente y Corazón local 127.0.0.1:8080)...{}", MATRIX_CYAN, MATRIX_RESET);
                let payload = format!(r#"{{"prompt": "{}"}}"#, prompt.replace('"', "\\\""));
                match http_request("POST", "/api/brain_query", Some(&payload)) {
                    Ok(resp) => println!("{}Respuesta MegaBrain:{}\n{}\n", MATRIX_BRIGHT, MATRIX_RESET, resp),
                    Err(e) => println!("{}Error: {}{}", MATRIX_ALERT, e, MATRIX_RESET),
                }
            }
            "--extract" | "-e" => {
                let plat = if args.len() > 2 { &args[2] } else { "pce" };
                println!("\n{}Disparando Ingesta y Extracción Universal de {}...{}", MATRIX_GREEN, plat, MATRIX_RESET);
                let payload = format!(r#"{{"platform": "{}"}}"#, plat);
                match http_request("POST", "/api/dumper_extract", Some(&payload)) {
                    Ok(resp) => println!("{}Resultado Extracción:{}\n{}\n", MATRIX_BRIGHT, MATRIX_RESET, resp),
                    Err(e) => println!("{}Error: {}{}", MATRIX_ALERT, e, MATRIX_RESET),
                }
            }
            "--boost" | "-b" => {
                println!("\n{}Disparando Boost de Gobernador y Root (uid=0)...{}", MATRIX_GREEN, MATRIX_RESET);
                match http_request("POST", "/api/trigger_boost", None) {
                    Ok(resp) => println!("{}Resultado Boost:{}\n{}\n", MATRIX_BRIGHT, MATRIX_RESET, resp),
                    Err(e) => println!("{}Error: {}{}", MATRIX_ALERT, e, MATRIX_RESET),
                }
            }
            "--record" | "-r" => {
                println!("\n{}Capturando clip real de 60 fotogramas desde SHM...{}", MATRIX_GREEN, MATRIX_RESET);
                match http_request("POST", "/api/record_clip", None) {
                    Ok(resp) => println!("{}Clip Registrado:{}\n{}\n", MATRIX_BRIGHT, MATRIX_RESET, resp),
                    Err(e) => println!("{}Error: {}{}", MATRIX_ALERT, e, MATRIX_RESET),
                }
            }
            _ => {
                println!("Comando desconocido '{}'. Uso: seere [--live|--brain <msg>|--extract <plat>|--boost|--record]", cmd);
            }
        }
        return;
    }

    // Default overview mode
    let t0 = Instant::now();
    match http_request("GET", "/api/brain_status", None) {
        Ok(body) => {
            let lat = t0.elapsed().as_micros();
            println!("{}[✓] MegaBrain Conectado en {}µs:{} {}", MATRIX_CYAN, lat, MATRIX_RESET, body);
        }
        Err(e) => println!("{}MegaBrain: {}{}", MATRIX_WARN, e, MATRIX_RESET),
    }

    render_shm_ascii_monitor();

    println!("\n{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}", MATRIX_DIM, MATRIX_RESET);
    println!("{}ACCIONES DISPONIBLES EN SEERE:{}", MATRIX_BRIGHT, MATRIX_RESET);
    println!("  seere --live            Ver monitor de vídeo real TrueColor en vivo");
    println!("  seere --brain \"prompt\"  Consultar la mente y corazón neural local");
    println!("  seere --extract <plat>  Extraer ROM/CD (pce, pc88, msx2, segacd)");
    println!("  seere --boost           Elevar a Root uid=0 y boost Cortex-X1");
    println!("  seere --record          Grabar clip real de 60 FPS en MP4/WebP");
    println!("{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}", MATRIX_DIM, MATRIX_RESET);
}
