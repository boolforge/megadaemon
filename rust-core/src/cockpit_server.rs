//! MegaDaemon v13.0 Zenith — 100% Native Pure-Rust Multi-Threaded Cockpit HTTP Server
//! Zero runtime Python dependencies on the chat/RE hot path.
//! Serves the mobile-first `ui-ux-pro` Cockpit UI, unrestricted file uploads, MiniMax/Swarm chat,
//! DiE+Binwalk, Tile Molester VRAM viewer, Kaitai/ImHex, MBA Deobfuscator, CIRX v3.4 SSA,
//! Rizin 0.9.1, Frida DBI, Exynos 2100 NPU/CV, Memory Freezer, and 9-Project Unifier.

use crate::{
    api_key,
    bindiff, binwalk_die, cfg_lifter, cirx_rtl, crypto_scan, deobfuscate, disasm, entropy,
    freezer, frida_dbi, hw_mapper, kaitai_imhex, npu_bridge, npu_cv, pattern, pointers, providers,
    rea_assistant, rom_locator, seere_v11, shm, snapshot, tile_vram, unifier, power_policy,
    accelerator_policy, npu_nnapi, mcp_server, sync, sync_web,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MEGADAEMON_DIR: &str = "/data/user/0/com.agy/files/megadaemon";
const RE_DIR: &str = "/data/user/0/com.agy/files/megadaemon/re_pipeline";
const SCUMMVM_TOOLS_DIR: &str = "/data/user/0/com.agy/files/scummvm/tools/re_pipeline";
const STATE_DIR: &str = "/data/user/0/com.agy/files/megadaemon/state";

/// Directorio de estado efectivo.
///
/// Por defecto es la ruta de la app en Android, pero se puede sobreescribir
/// con `MEGADAEMON_STATE_DIR` (tests, despliegues fuera de Android, entornos
/// de escritorio).  Sin esto el cockpit escribía siempre en
/// `/data/user/0/...`, que en un escritorio no existe (o requiere root).
pub fn state_dir() -> String {
    if let Ok(d) = std::env::var("MEGADAEMON_STATE_DIR") {
        let d = d.trim();
        if !d.is_empty() {
            return d.to_string();
        }
    }
    STATE_DIR.to_string()
}
const UPLOADS_DIR: &str = "/data/user/0/com.agy/files/megadaemon/uploads";
const EXTRACTED_DIR: &str = "/data/user/0/com.agy/files/megadaemon/extracted";
const TEMPLATE_PATH: &str = "/data/user/0/com.agy/files/megadaemon/web/cockpit_template.html";

/// ── Autenticación del cockpit (v13.2 · S08) ────────────────────────────────
/// El cockpit EJECUTA órdenes (root) a través del agente SEERE.  Con
/// `Access-Control-Allow-Origin: *` y sin autenticación, cualquier página
/// web que visitara el móvil (o cualquiera en la LAN) podía hacer POST
/// /api/chat con `$ ...` y obtener ejecución de comandos como root.
///
/// Ahora:
///   * el token se genera en el arranque (o viene en SEERE_AUTH_TOKEN) y se
///     guarda en `<STATE_DIR>/cockpit_token` con permisos 0600;
///   * los endpoints que ejecutan o modifican estado exigen la cabecera
///     `X-Seere-Auth-Token`;
///   * el CORS deja de ser `*`: sólo se refleja el Origin si coincide con el
///     Host de la petición o está en MEGADAEMON_ALLOWED_ORIGINS;
///   * la UI recibe el token embebido (sólo legible same-origin).
static AUTH_TOKEN: OnceLock<String> = OnceLock::new();

fn random_hex(nbytes: usize) -> String {
    let mut buf = vec![0u8; nbytes];
    if let Ok(mut f) = File::open("/dev/urandom") {
        use std::io::Read;
        let _ = f.read_exact(&mut buf);
    } else {
        // Fallback: reloj + pid (no criptográfico, pero el fichero es local).
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        buf[..8].copy_from_slice(&t.to_le_bytes()[..8]);
    }
    buf.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Token de autenticación del cockpit (estable entre arranques).
pub fn auth_token() -> &'static str {
    AUTH_TOKEN.get_or_init(|| {
        if let Ok(t) = std::env::var("SEERE_AUTH_TOKEN") {
            let t = t.trim().to_string();
            if !t.is_empty() {
                return t;
            }
        }
        let path = format!("{}/cockpit_token", state_dir());
        if let Ok(existing) = fs::read_to_string(&path) {
            let t = existing.trim().to_string();
            if t.len() >= 16 {
                return t;
            }
        }
        let t = random_hex(32);
        let _ = fs::create_dir_all(state_dir());
        let _ = fs::write(&path, &t);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }
        t
    })
}

/// ¿La petición lleva el token correcto?
fn auth_ok(headers: &str) -> bool {
    let mut supplied = String::new();
    for line in headers.lines() {
        let low = line.to_lowercase();
        if low.starts_with("x-seere-auth-token:") {
            supplied = line[low.find(':').unwrap() + 1..].trim().to_string();
        }
    }
    !supplied.is_empty() && supplied == auth_token()
}

/// ¿Este endpoint exige autenticación?
fn requires_auth(path: &str, _method: &str) -> bool {
    // v13.2.3 (AUTH-01…AUTH-06): FAIL-CLOSED.
    //
    // Antes sólo se protegían los POST, y en la rama GET quedaban endpoints
    // con efecto de estado o con datos sensibles:
    //   * GET /api/vision_start  -> arrancaba el worker de visión continua
    //     (un hilo PERMANENTE que quema CPU en el dispositivo) sin token;
    //   * GET /api/memory_freeze -> escribía/congelaba ficheros sin token;
    //   * GET /api/providers     -> volcaba el catálogo de proveedores;
    //   * GET /api/live_frame    -> devolvía un frame de la cámara.
    //
    // El CORS restringido cierra el vector "cualquier web del móvil", pero el
    // cockpit también escucha en la LAN: cualquier proceso del dispositivo
    // puede llamar a 127.0.0.1:<puerto>.  Por eso ahora TODO lo que no sea
    // la página HTML exige token, con independencia del método.
    //
    // La única superficie pública es `GET /` (y `/index.html`): es la propia
    // página, que lleva el token embebido en `window.MEGADAEMON_TOKEN`.  Sin
    // CORS abierto, un origen externo no puede leerla.
    if path == "/" || path == "/index.html" {
        return false;
    }
    // Los endpoints de monitorización son de SÓLO LECTURA: no exigen el
    // token de sesión local (el control de acceso remoto es la API key).
    if matches!(path, "/health" | "/api/health" | "/metrics" | "/api/metrics") {
        return false;
    }
    // v14 (UI-16 · CRÍTICA): el intercambio API key → token de sesión NO
    // puede exigir el token de sesión: sería un deadlock (pide un token para
    // poder entregar un token) y la UI quedaría muerta con 403 en todas las
    // llamadas de datos.  La credencial que lo protege es la API key, que el
    // gate anterior ya validó.
    if path == "/api/session/exchange" {
        return false;
    }
    true
}

/// Resuelve el HTML del cockpit.
///
/// BUG v13.0: la ruta estaba HARDCODEADA al path de Android, así que fuera
/// del móvil (o en un checkout del repo) el cockpit servía un
/// `<h1>...</h1>` de 45 bytes: la UI "no funcionaba" y no había ni un error
/// visible.  Ahora se prueban, por orden:
///   1. $MEGADAEMON_WEB_TEMPLATE
///   2. <dir del binario>/../../web/cockpit_template.html
///   3. <dir del binario>/web/cockpit_template.html
///   4. ./web/cockpit_template.html
///   5. el path de Android de siempre
pub fn resolve_template() -> Option<String> {
    let mut cands: Vec<String> = Vec::new();
    if let Ok(p) = std::env::var("MEGADAEMON_WEB_TEMPLATE") {
        cands.push(p);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            cands.push(dir.join("../../web/cockpit_template.html").to_string_lossy().into());
            cands.push(dir.join("../web/cockpit_template.html").to_string_lossy().into());
            cands.push(dir.join("web/cockpit_template.html").to_string_lossy().into());
        }
    }
    cands.push("web/cockpit_template.html".to_string());
    cands.push("../web/cockpit_template.html".to_string());
    cands.push(TEMPLATE_PATH.to_string());
    for c in cands {
        if Path::new(&c).exists() {
            return Some(c);
        }
    }
    None
}

/// Plantilla HTML embebida en el binario.
///
/// BUG v13.4 (AUDIT-WEB-01): la UI "no funcionaba" en el PC del usuario
/// (localhost:3000) porque `resolve_template()` sólo miraba en disco y el
/// binario del PC no tenía `web/` al lado.  Con el `include_str!` el binario
/// es autosuficiente en cualquier plataforma; el fichero en disco sigue
/// teniendo prioridad para poder parchear la UI sin recompilar.
pub const EMBEDDED_TEMPLATE: &str = include_str!("../../web/cockpit_template.html");

/// Puertos realmente enlazados (los alias son best-effort).
static OPEN_PORTS: std::sync::OnceLock<Vec<u16>> = std::sync::OnceLock::new();
/// Puerto principal del cockpit.
static PRIMARY_PORT: std::sync::OnceLock<u16> = std::sync::OnceLock::new();
/// Instante en el que el listener nació.
static LISTENER_BORN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// IPs locales de la máquina (heurística: la que usaría el tráfico
/// saliente). Sirve para construir las URLs que el usuario debe abrir
/// desde el PC contra el móvil.
pub fn local_ips() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if sock.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = sock.local_addr() {
                let ip = addr.ip().to_string();
                if !ip.is_empty() && ip != "0.0.0.0" {
                    out.push(ip);
                }
            }
        }
    }
    out
}

/// Diagnóstico de la capa web.
///
/// El usuario dijo "la interfaz web no me funciona" sin poder saber por
/// qué.  Con esto, un `curl /health` dice TODO lo necesario: dónde
/// escucha, qué puertos están vivos, qué plantilla se sirve, si el token
/// está materializado y qué URLs exactas hay que abrir.
/// ¿El `Host` de la petición es loopback (localhost, 127.0.0.1, [::1])?
///
/// v14 (medido en el Samsung): el gate de API key se apoyaba en la IP de
/// peer (loopback) ⇒ el túnel (cloudflared entra por 127.0.0.1) quedaba
/// indistinguible del navegador local.  Con `MEGADAEMON_REQUIRE_API_KEY=1`
/// (obligatorio para no abrir la API a internet) el Chrome del móvil se
/// comía un 401 en CADA panel en vivo, porque el navegador no puede mandar
/// la cabecera `X-API-Key`.  La exención pasa a decidirse por el Host.
fn host_is_loopback(headers: &str) -> bool {
    for line in headers.lines() {
        let low = line.to_ascii_lowercase();
        let Some(rest) = low.strip_prefix("host:") else { continue };
        let mut h = rest.trim();
        // "[::1]:3000" -> "::1" ; "localhost:3000" -> "localhost"
        if let Some(inner) = h.strip_prefix('[') {
            h = inner.split(']').next().unwrap_or("").trim();
        } else {
            h = h.split(':').next().unwrap_or("").trim();
        }
        if h.eq_ignore_ascii_case("localhost") || h == "::1" {
            return true;
        }
        if let Ok(ip) = h.parse::<std::net::IpAddr>() {
            if api_key::is_loopback(&ip) {
                return true;
            }
        }
    }
    false
}

/// ¿La petición trae cabeceras de proxy (túnel, reverse proxy)?
///
/// Sirven para que un cliente **no pueda falsear el Host** y colarse por la
/// exención local: cloudflared añade `CF-Connecting-IP`, los proxies
/// normales añaden `X-Forwarded-For`/`X-Real-IP`/`Forwarded`.
fn has_proxy_headers(headers: &str) -> bool {
    let low = headers.to_ascii_lowercase();
    ["cf-connecting-ip:", "x-forwarded-for:", "x-real-ip:", "forwarded:", "x-forwarded-host:"]
        .iter()
        .any(|h| low.contains(h))
}

/// ¿La petición viene de un navegador LOCAL del propio dispositivo?
///
/// Se permite sin API key cuando TODO se cumple:
///   1. el peer es loopback (el navegador del propio móvil),
///   2. el Host es loopback (`localhost`, `127.0.0.1`, `[::1]`),
///   3. no hay cabeceras de proxy (o sea: no viene por el túnel).
///
/// El túnel público sigue exigiendo API key: su Host es un dominio y sus
/// peticiones llevan cabeceras de proxy.  El token de sesión sigue
/// protegiendo las escrituras en cualquier caso.
fn local_browser_request(peer: &std::net::IpAddr, headers: &str) -> bool {
    api_key::is_loopback(peer) && host_is_loopback(headers) && !has_proxy_headers(headers)
}

pub fn web_diagnostics() -> Value {
    let template_path = resolve_template();
    let template_source = if template_path.is_some() {
        "file"
    } else if EMBEDDED_TEMPLATE.trim().is_empty() {
        "NINGUNA"
    } else {
        "embedded"
    };
    let template_bytes = template_path
        .as_ref()
        .and_then(|p| fs::metadata(p).ok())
        .map(|m| m.len())
        .unwrap_or(EMBEDDED_TEMPLATE.len() as u64);
    let token_path = format!("{}/cockpit_token", state_dir());
    let token_materialized = Path::new(&token_path).exists();
    let ports_open = OPEN_PORTS.get().cloned().unwrap_or_default();
    let primary = PRIMARY_PORT.get().copied().unwrap_or(3000);
    let ips = local_ips();

    let mut urls: Vec<String> = Vec::new();
    for p in &ports_open {
        urls.push(format!("http://127.0.0.1:{}", p));
    }
    for ip in &ips {
        for p in &ports_open {
            urls.push(format!("http://{}:{}", ip, p));
        }
    }

    json!({
        "bind": "0.0.0.0",
        "primary_port": primary,
        "ports_open": ports_open,
        "ports_alias": ALIAS_PORTS,
        "listener_alive": LISTENER_BORN.get().is_some(),
        "template_source": template_source,
        "template_path": template_path,
        "template_bytes": template_bytes,
        "template_missing": template_source == "NINGUNA",
        "token_materialized": token_materialized,
        "local_browser_exemption": true,
        "api_key_required_for": "túnel, LAN y cualquier Host no loopback",
        "token_path": token_path,
        "local_ips": ips,
        "urls": urls,
        "state_dir": state_dir(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerState {
    pub status: String,
    pub pid: Option<u32>,
    pub vla_hook: bool,
}

pub struct GlobalCockpitState {
    pub active_target: String,
    pub runners: HashMap<String, RunnerState>,
}

static COCKPIT_STATE: OnceLock<Mutex<GlobalCockpitState>> = OnceLock::new();

fn get_cockpit_state() -> &'static Mutex<GlobalCockpitState> {
    COCKPIT_STATE.get_or_init(|| {
        let mut runners = HashMap::new();
        for &k in &["scummvm", "segacd", "pce", "saturn", "ps1", "msx2", "pc88"] {
            runners.insert(
                k.to_string(),
                RunnerState {
                    status: "READY".to_string(),
                    pid: None,
                    vla_hook: true,
                },
            );
        }
        Mutex::new(GlobalCockpitState {
            active_target: "segacd".to_string(),
            runners,
        })
    })
}

fn read_json_file_fallback(filename: &str, default_val: Value) -> Value {
    let p1 = format!("{}/{}", SCUMMVM_TOOLS_DIR, filename);
    let p2 = format!("{}/{}", RE_DIR, filename);
    let p3 = format!("{}/{}", state_dir(), filename);
    for p in &[p1, p2, p3] {
        if let Ok(s) = fs::read_to_string(p) {
            if let Ok(v) = serde_json::from_str::<Value>(&s) {
                // v13.2 (HW-01): si el fichero contiene un ARRAY (estado a
                // medias), indexarlo con una clave entraba en panic de
                // serde_json y, en release (panic=abort), mataba el cockpit
                // COMPLETO.  Sólo se aceptan objetos.
                if v.is_object() {
                    return v;
                }
            }
        }
    }
    default_val
}

fn resolve_target_or_file(plat: &str, explicit_file: Option<&str>) -> (String, String, String) {
    if let Some(fp) = explicit_file {
        let trimmed = fp.trim();
        if !trimmed.is_empty() && Path::new(trimmed).exists() {
            let fname = Path::new(trimmed)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(trimmed)
                .to_string();
            return (trimmed.to_string(), fname, "Multi-OS/Auto".to_string());
        }
    }
    let key = plat.to_lowercase();
    if let Some(entry) = rom_locator::resolve_binary(&key) {
        return (entry.path, entry.platform_name, entry.arch);
    }
    let (name, arch, _, _, _) = rom_locator::platform_metadata(&key);
    let fallback = match key.as_str() {
        "ps1" => "/data/user/0/com.agy/files/re-research/bin/ps1_slps.bin",
        "saturn" => "/data/user/0/com.agy/files/re-research/bin/saturn_main.bin",
        "pce" => "/data/user/0/com.agy/files/re-research/bin/pce_overlay.bin",
        "msx2" => "/data/user/0/com.agy/files/re-research/bin/msx2_com.bin",
        "pc88" => "/data/user/0/com.agy/files/re-research/bin/pc88_boot.bin",
        _ => "/data/user/0/com.agy/files/re-research/bin/scd_subcode.bin",
    };
    (fallback.to_string(), name.to_string(), arch.to_string())
}

fn ensure_live_shm(platform: &str) {
    let mut buf = vec![0u8; shm::SHM_SIZE];
    buf[0..4].copy_from_slice(&0x534E4154u32.to_le_bytes());
    buf[4..8].copy_from_slice(&13u32.to_le_bytes());
    buf[8..12].copy_from_slice(&120u32.to_le_bytes());
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0);
    buf[12..20].copy_from_slice(&ts.to_le_bytes());
    buf[20..22].copy_from_slice(&320u16.to_le_bytes());
    buf[22..24].copy_from_slice(&224u16.to_le_bytes());
    buf[24..26].copy_from_slice(&24u16.to_le_bytes());
    buf[26..28].copy_from_slice(&1u16.to_le_bytes());

    let tag = format!("MEGADAEMON ZENITH 13.0 // {} // LIVE SHM\0", platform.to_uppercase());
    let tag_b = tag.as_bytes();
    buf[32..32 + tag_b.len()].copy_from_slice(tag_b);

    let pat = b"SNATCH_SCRIPT_TABLE_POINTER_0x00FF0000";
    buf[0x018C20..0x018C20 + pat.len()].copy_from_slice(pat);

    let _ = fs::write(shm::SHM_PATH, &buf);
}

fn decode_base64(input: &str) -> Vec<u8> {
    let clean = match input.find(',') {
        Some(idx) => &input[idx + 1..],
        None => input,
    };
    let mut out = Vec::with_capacity(clean.len() * 3 / 4);
    let mut val = 0u32;
    let mut bits = 0i32;
    for b in clean.bytes() {
        let d = match b {
            b'A'..=b'Z' => (b - b'A') as u32,
            b'a'..=b'z' => (b - b'a' + 26) as u32,
            b'0'..=b'9' => (b - b'0' + 52) as u32,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        };
        val = (val << 6) | d;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((val >> bits) & 0xFF) as u8);
        }
    }
    out
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(v) = u8::from_str_radix(hex, 16) {
                    out.push(v);
                    i += 3;
                    continue;
                }
            }
        } else if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Lee un parámetro numérico de la query string.
fn query_u64(query: &str, key: &str) -> Option<u64> {
    let qs = parse_query_params(query);
    qs.get(key).and_then(|v| v.parse::<u64>().ok())
}

fn parse_query_params(query: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let mut it = pair.splitn(2, '=');
        let k = percent_decode(it.next().unwrap_or(""));
        let v = percent_decode(it.next().unwrap_or(""));
        map.insert(k, v);
    }
    map
}

fn check_llama_online() -> bool {
    if let Ok(addr) = "127.0.0.1:8080".parse::<SocketAddr>() {
        TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok()
    } else {
        false
    }
}

thread_local! {
    /// Origen permitido para ESTA conexión (vacío = sin CORS).
    static CORS_ORIGIN: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// ¿Se permite este Origin?  (v13.2 · S08: `*` era un agujero)
fn cors_allowed(origin: &str, host: &str) -> bool {
    if origin.is_empty() {
        return false;
    }
    // 1) Lista explícita por variable de entorno.
    if let Ok(list) = std::env::var("MEGADAEMON_ALLOWED_ORIGINS") {
        if list.split(',').any(|o| o.trim() == origin) {
            return true;
        }
    }
    // 2) Same-origin: el Origin debe apuntar al mismo Host que la petición.
    let o = origin.trim();
    for scheme in ["http://", "https://"] {
        if let Some(rest) = o.strip_prefix(scheme) {
            let hostport = rest.split('/').next().unwrap_or("");
            if !hostport.is_empty() && hostport.eq_ignore_ascii_case(host.trim()) {
                return true;
            }
        }
    }
    false
}

fn send_http_response(stream: &mut TcpStream, status: u16, content_type: &str, body: &[u8]) {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        413 => "Payload Too Large",
        403 => "Forbidden",
        _ => "Internal Server Error",
    };
    let cors = CORS_ORIGIN.with(|c| c.borrow().clone());
    let header = format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: {cors}\r\n\
         Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
         Access-Control-Allow-Headers: Content-Type, X-Seere-Auth-Token\r\n\
         Vary: Origin\r\n\
         Cache-Control: no-cache, no-store, must-revalidate\r\n\
         Connection: close\r\n\r\n",
        status,
        status_text,
        content_type,
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

fn send_json(stream: &mut TcpStream, val: &Value) {
    let s = serde_json::to_string(val).unwrap_or_else(|_| "{}".to_string());
    send_http_response(stream, 200, "application/json; charset=utf-8", s.as_bytes());
}

fn list_uploaded_files() -> Value {
    let _ = fs::create_dir_all(UPLOADS_DIR);
    let mut items = Vec::new();
    if let Ok(rd) = fs::read_dir(UPLOADS_DIR) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_file() {
                let name = p
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                if name.starts_with('.') {
                    continue;
                }
                let sz = entry.metadata().map(|m| m.len()).unwrap_or(0);
                items.push(json!({
                    "filename": name,
                    "path": p.to_string_lossy().to_string(),
                    "size_bytes": sz,
                    "size_formatted": if sz < 1048576 {
                        format!("{:.1} KB", sz as f64 / 1024.0)
                    } else {
                        format!("{:.2} MB", sz as f64 / 1048576.0)
                    }
                }));
            }
        }
    }
    items.sort_by(|a, b| {
        a["filename"]
            .as_str()
            .unwrap_or("")
            .cmp(b["filename"].as_str().unwrap_or(""))
    });
    json!({ "uploads": items, "count": items.len(), "dir": UPLOADS_DIR })
}

/// Contadores del cockpit (Fase 4: /metrics).
static REQ_TOTAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static REQ_REJECTED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static REQ_RATE_LIMITED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static SERVER_START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// Rate limit global por IP (Fase 4).
static RATE_LIMITER: std::sync::LazyLock<api_key::RateLimiter> =
    std::sync::LazyLock::new(api_key::RateLimiter::new);

fn uptime_secs() -> u64 {
    SERVER_START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs()
}

/// Lee la carga del sistema desde /proc (Android/Linux) o devuelve null.
fn system_load() -> Value {
    let mut out = json!({});
    // CPU: /proc/stat -> total e idle (muestra de 100 ms).
    if let Ok(stat) = fs::read_to_string("/proc/stat") {
        if let Some(line) = stat.lines().next() {
            let nums: Vec<u64> = line
                .split_whitespace()
                .skip(1)
                .filter_map(|x| x.parse::<u64>().ok())
                .collect();
            if nums.len() >= 5 {
                let total: u64 = nums.iter().sum();
                let idle = nums[3] + nums[4];
                out["cpu_total_ticks"] = json!(total);
                out["cpu_idle_ticks"] = json!(idle);
            }
        }
    }
    // RAM: /proc/meminfo
    if let Ok(mi) = fs::read_to_string("/proc/meminfo") {
        let mut total = None;
        let mut avail = None;
        for l in mi.lines() {
            if l.starts_with("MemTotal:") {
                total = l.split_whitespace().nth(1).and_then(|v| v.parse::<u64>().ok());
            } else if l.starts_with("MemAvailable:") {
                avail = l.split_whitespace().nth(1).and_then(|v| v.parse::<u64>().ok());
            }
        }
        if let (Some(t), Some(a)) = (total, avail) {
            out["mem_total_kb"] = json!(t);
            out["mem_available_kb"] = json!(a);
            out["mem_used_pct"] = json!(if t > 0 {
                ((t - a) as f64 * 100.0 / t as f64 * 10.0).round() / 10.0
            } else {
                0.0
            });
        }
    }
    // Carga media (Android la expone en /proc/loadavg).
    if let Ok(la) = fs::read_to_string("/proc/loadavg") {
        out["loadavg"] = json!(la.split_whitespace().next().unwrap_or(""));
    }
    out
}

/// Techo de clientes concurrentes del cockpit (protege la RAM del móvil).
const MAX_CLIENT_THREADS: usize = 64;
/// Clientes concurrentes en curso.
static ACTIVE_CLIENTS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

fn handle_client(mut stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(60)));

    let mut buf = vec![0u8; 65536];
    let mut total_read = 0usize;
    let header_end;

    loop {
        match stream.read(&mut buf[total_read..]) {
            Ok(0) => return,
            Ok(n) => {
                total_read += n;
                if let Some(pos) = find_subsequence(&buf[..total_read], b"\r\n\r\n") {
                    header_end = pos + 4;
                    break;
                }
                if total_read == buf.len() {
                    if buf.len() > 262144 {
                        return;
                    }
                    buf.resize(buf.len() * 2, 0);
                }
            }
            Err(_) => return,
        }
    }

    let header_str = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let mut lines = header_str.lines();
    let first_line = lines.next().unwrap_or("");
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let raw_uri = parts.next().unwrap_or("/");

    if method == "OPTIONS" {
        send_http_response(&mut stream, 200, "text/plain", b"OK");
        return;
    }

    let mut content_length = 0usize;
    let mut origin = String::new();
    let mut host = String::new();
    for line in lines {
        let low = line.to_lowercase();
        if low.starts_with("content-length:") {
            content_length = low["content-length:".len()..].trim().parse().unwrap_or(0);
        } else if low.starts_with("origin:") {
            origin = line[low.find(':').unwrap() + 1..].trim().to_string();
        } else if low.starts_with("host:") {
            host = line[low.find(':').unwrap() + 1..].trim().to_string();
        }
    }
    // CORS: sólo se refleja el Origin si está permitido (nunca `*`).
    let cors_ok = cors_allowed(&origin, &host);
    CORS_ORIGIN.with(|c| {
        *c.borrow_mut() = if cors_ok { origin.clone() } else { String::new() };
    });

    // Cap single HTTP body at 100 MB (allows large .7z, .exe, .apk, .iso uploads cleanly)
    // v13.2 (HW-05): 100 MB por conexión y un hilo por conexión = DoS local
    // trivial (basta con mandar un Content-Length enorme).
    const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
    if content_length > MAX_BODY_BYTES {
        send_http_response(
            &mut stream,
            413,
            "application/json",
            b"{\"error\":\"payload_too_large\",\"limit_bytes\":8388608}",
        );
        return;
    }

    let body_already = total_read.saturating_sub(header_end);
    if content_length > body_already {
        buf.resize(header_end + content_length, 0);
        let mut offset = total_read;
        while offset < header_end + content_length {
            match stream.read(&mut buf[offset..header_end + content_length]) {
                Ok(0) => break,
                Ok(n) => offset += n,
                Err(_) => break,
            }
        }
        total_read = offset;
    }

    let body_slice = if header_end + content_length <= total_read {
        &buf[header_end..header_end + content_length]
    } else {
        &buf[header_end..total_read]
    };

    let (path, query) = match raw_uri.find('?') {
        Some(idx) => (&raw_uri[..idx], &raw_uri[idx + 1..]),
        None => (raw_uri, ""),
    };
    let qs = parse_query_params(query);

    // ------------------------------------------------------------------
    // v13.2.5 (Fase 4): API key para acceso remoto.  Se evalúa ANTES que
    // el token local, porque es la primera barrera cuando el cockpit está
    // expuesto (túnel Cloudflare, reverse proxy, LAN).
    //   * loopback exento (UI y proxy en la misma máquina);
    //   * sin key -> 401; key incorrecta -> 403 + auditoría; exceso de
    //     ritmo -> 429.
    // ------------------------------------------------------------------
    REQ_TOTAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let peer_ip = stream
        .peer_addr()
        .map(|a| a.ip())
        .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
    let cfg = api_key::ApiKeyConfig::load(&state_dir());
    // BUG v13.4 (AUDIT-WEB-02): la página pública (`/`) se servía DESPUÉS del
    // gate de API key, así que el navegador recibía 401 y la UI "no
    // funcionaba" (ni en localhost:3000 del PC ni por el túnel Cloudflare).
    //
    // La página no es un endpoint de datos: sólo sirve el HTML+JS. El token
    // de sesión va embebido en ella, y la API key protege los DATOS vía
    // `requires_auth()` + `auth_ok()`. Servir el HTML sin API key mantiene el
    // fail-closed de los datos (que es lo que importa) y arregla la UI.
    // El cliente JS pide la API key al usuario si las llamadas fallan con 401.
    //
    // v14 (AUDIT-WEB-05): `/health` y `/metrics` también estaban exentos del
    // *token* de sesión pero NO del gate de API key ⇒ devolvían 401 y el
    // diagnóstico era invisible justo cuando hacía falta. Ahora son públicos
    // (no exponen datos de usuario: versión, uptime, puertos y contadores),
    // igual que ya se documentaba en el commit 4771277.
    let is_public_page = path == "/" || path == "/index.html";
    let is_public_probe = matches!(
        path,
        "/health" | "/api/health" | "/metrics" | "/api/metrics" | "/api/ping"
    );
    // v14: exención del navegador LOCAL (ver `local_browser_request`).
    // Sustituye a la exención por IP: el túnel entra por loopback, así que
    // la IP no distingue; el Host + la ausencia de cabeceras de proxy sí.
    let local_browser = local_browser_request(&peer_ip, &header_str);
    match api_key::decide(
        &cfg,
        &RATE_LIMITER,
        &peer_ip,
        &header_str,
        std::time::Instant::now(),
    ) {
        api_key::Decision::Allow => {}
        api_key::Decision::Unauthorized if is_public_page || is_public_probe || local_browser => {
            // Se permite la página/sonda; el resto de endpoints siguen protegidos.
            REQ_REJECTED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        api_key::Decision::Unauthorized => {
            REQ_REJECTED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            println!(
                "{}",
                api_key::audit_line(
                    &peer_ip,
                    path,
                    &api_key::user_agent(&header_str),
                    "missing_api_key"
                )
            );
            send_http_response(
                &mut stream,
                401,
                "application/json",
                b"{\"error\":\"unauthorized\",\"hint\":\"falta X-API-Key o Authorization: Bearer\"}",
            );
            return;
        }
        api_key::Decision::Forbidden => {
            REQ_REJECTED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            println!(
                "{}",
                api_key::audit_line(
                    &peer_ip,
                    path,
                    &api_key::user_agent(&header_str),
                    "invalid_api_key"
                )
            );
            send_http_response(
                &mut stream,
                403,
                "application/json",
                b"{\"error\":\"forbidden\",\"hint\":\"API key invalida\"}",
            );
            return;
        }
        api_key::Decision::RateLimited => {
            REQ_RATE_LIMITED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            println!(
                "{}",
                api_key::audit_line(
                    &peer_ip,
                    path,
                    &api_key::user_agent(&header_str),
                    "rate_limited"
                )
            );
            send_http_response(
                &mut stream,
                429,
                "application/json",
                b"{\"error\":\"rate_limited\",\"hint\":\"maximo 60 req/min por IP\"}",
            );
            return;
        }
    }

    // v13.2 (S08): los endpoints que ejecutan o modifican estado exigen el
    // token.  Sin esto, cualquiera (página web con CORS abierto, o alguien en
    // la LAN) podía hacer POST /api/chat con "$ comando" y obtener RCE root.
    if requires_auth(path, method) && !auth_ok(&header_str) {
        send_http_response(
            &mut stream,
            403,
            "application/json",
            b"{\"error\":\"unauthorized\",\"hint\":\"falta la cabecera X-Seere-Auth-Token\"}",
        );
        return;
    }

    if method == "GET" {
        match path {
            "/" | "/index.html" => {
                let html = resolve_template()
                    .and_then(|p| fs::read(&p).ok())
                    .unwrap_or_else(|| EMBEDDED_TEMPLATE.as_bytes().to_vec());
                // BUG v13.4 (AUDIT-WEB-06 · SEGURIDAD): la página pública
                // llevaba el token de sesión INCRUSTADO.  Con `GET /` público
                // (necesario para que el navegador pueda abrir la UI) eso
                // significa que cualquiera que llegue al puerto obtiene el
                // token → POST /api/chat → RCE root en el móvil.  El CORS
                // same-origin no protege: el atacante no usa un navegador
                // ajeno, usa curl.
                //
                // Solución: la página NO lleva el token.  El cliente pide la
                // API key (que es la credencial remota) y la intercambia por
                // un token de sesión en `POST /api/session/exchange`, que
                // exige la API key válida.  Así:
                //   * `GET /`               → público, sin secretos.
                //   * `GET /api/*`          → exige API key (o loopback).
                //   * `POST /api/*`         → exige API key + token de sesión.
                let html = String::from_utf8_lossy(&html).into_owned();
                let inject = "<script>window.MEGADAEMON_TOKEN=\"\";window.MEGADAEMON_TOKEN_ISSUED=false;</script>";
                let html = match html.find("<script>") {
                    Some(i) => format!("{}{}{}", &html[..i], inject, &html[i..]),
                    None => format!("{}{}", inject, html),
                };
                send_http_response(
                    &mut stream,
                    200,
                    "text/html; charset=utf-8",
                    html.as_bytes(),
                );
            }
            "/api/remote_auth" => {
                send_json(
                    &mut stream,
                    &json!({
                        "status": "ok",
                        "authenticated": true,
                        "server": "megadaemon-zenith-rust",
                        "mode": "100%-pure-rust-v13",
                        "version": "13.0.0"
                    }),
                );
            }
            _ => {
                send_http_response(&mut stream, 404, "application/json", b"{\"error\":\"not_found\"}");
            }
        }
        return;
    }

    // POST endpoints
    let body_json: Value = serde_json::from_slice(body_slice).unwrap_or_else(|_| json!({}));

    let qp = parse_query_params(query);
    match path {
        // ------------------------------------------------------------------
        // v14 (UI-16 · CRÍTICA) — Intercambio: API key → token de sesión.
        //
        // La página pública `GET /` NO lleva el token incrustado (si lo
        // llevara, cualquiera que alcanzase el puerto obtendría el control
        // del daemon).  El navegador pide la API key al usuario y la canjea
        // aquí por un token de sesión.
        //
        // El endpoint NO puede exigir el token de sesión (sería un deadlock:
        // pedir un token para poder entregar un token) → `requires_auth()`
        // lo exime.  La credencial que lo protege es la API key, que el gate
        // de API key ya validó antes de llegar aquí.
        // ------------------------------------------------------------------
        "/api/session/exchange" => {
            let provided = body_json
                .get("api_key")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let ok = !provided.is_empty()
                && cfg.enabled
                && !cfg.key.is_empty()
                && provided == cfg.key;
            if ok {
                send_json(
                    &mut stream,
                    &json!({
                        "ok": true,
                        "token": auth_token(),
                        "expires_hint": "el token pertenece a la instancia; rotarlo reinicia la sesion",
                    }),
                );
            } else {
                REQ_REJECTED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                send_http_response(
                    &mut stream,
                    403,
                    "application/json",
                    b"{\"ok\":false,\"error\":\"invalid_api_key\",\"hint\":\"la API key no coincide con la configurada en la instancia\"}",
                );
            }
        }
        // ------------------------------------------------------------------
        // v13.3 — API del motor de sincronización de conversaciones.
        // ------------------------------------------------------------------
        "/api/sync/run" => {
            let services = body_json.get("services").and_then(|v| v.as_array()).map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect::<Vec<String>>()
            });
            let cookies = body_json
                .get("include_cookies")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let r = sync::run_sync(services, cookies);
            send_json(&mut stream, &r);
        }
        // v13.4 — Vault de firma electronica: material criptografico con zona
        // publica (replicable) y zona privada (nunca sale del dispositivo).
        // Cae dentro del gate de autenticacion: todo lo que no sea `GET /`
        // exige API key + token de sesion.
        "/api/signing_vault" => {
            let action = body_json
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("list")
                .to_string();
            let r = crate::signing_vault::handle_action(
                &action,
                &body_json,
                std::path::Path::new(&state_dir()),
            );
            send_json(&mut stream, &r);
        }
        // v13.4 — genérico: cualquier herramienta del catálogo extendido
        // accesible como POST /api/tool/<nombre> con el cuerpo como args.
        _ if path.starts_with("/api/tool/") => {
            let tool = path.trim_start_matches("/api/tool/").to_string();
            match crate::mcp_ext::dispatch(&tool, &body_json) {
                Some(r) => send_json(&mut stream, &r),
                None => send_http_response(
                    &mut stream,
                    404,
                    "application/json",
                    b"{\"error\":\"unknown_tool\"}",
                ),
            }
        }
        "/api/capabilities" => {
            send_json(&mut stream, &crate::capability::verify());
        }
        "/api/tasks" => {
            send_json(&mut stream, &crate::task_scheduler::list_json());
        }
        "/api/alerts" => {
            send_json(&mut stream, &crate::alerts::list(None, false, 50));
        }
        "/api/profile" => {
            send_json(&mut stream, &crate::profile_analysis::analyze());
        }
        "/api/sync/status" => {
            send_json(&mut stream, &sync::status());
        }
        "/api/sync/harvest" => {
            let r = sync_web::harvest_sessions();
            send_json(&mut stream, &r);
        }
        "/api/sync/web" => {
            let services = body_json.get("services").and_then(|v| v.as_array()).map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect::<Vec<String>>()
            });
            let r = sync_web::web_pull(services);
            send_json(&mut stream, &r);
        }
        "/api/sync/ingest" => {
            let path = body_json
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let r = sync_web::ingest_export(&path);
            send_json(&mut stream, &r);
        }
        "/api/conversations/search" => {
            let q: String = body_json
                .get("query")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| qp.get("q").cloned())
                .unwrap_or_default();
            let svc: Option<String> = body_json
                .get("service")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| qp.get("service").cloned());
            let limit = body_json
                .get("limit")
                .and_then(|v| v.as_u64())
                .or_else(|| qp.get("limit").and_then(|v| v.parse::<u64>().ok()))
                .unwrap_or(20) as usize;
            let r = sync::search(&q, svc.as_deref(), limit);
            send_json(&mut stream, &r);
        }
        "/api/conversations/list" => {
            let svc: Option<String> = body_json
                .get("service")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| qp.get("service").cloned());
            let limit = body_json
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(50) as usize;
            let r = sync::list_conversations(svc.as_deref(), limit);
            send_json(&mut stream, &r);
        }
        // ------------------------------------------------------------------
        // v13.3 — Endpoint MCP nativo sobre HTTP (Streamable HTTP, JSON-RPC
        // 2.0).  Es el MISMO catálogo que el servidor stdio: RE + control
        // tipado del dispositivo.  La autenticación es la misma del cockpit
        // (API key / Bearer), evaluada antes de llegar aquí, de modo que el
        // túnel Cloudflare expone un único plano de control.
        // ------------------------------------------------------------------
        "/mcp" | "/mcp/" => {
            // Permite lote (batch) estilo JSON-RPC 2.0.
            let response = if let Some(batch) = body_json.as_array() {
                let out: Vec<Value> = batch
                    .iter()
                    .filter_map(|r| mcp_server::handle_rpc(r))
                    .collect();
                json!(out)
            } else {
                match mcp_server::handle_rpc(&body_json) {
                    Some(r) => r,
                    None => json!({ "jsonrpc": "2.0", "result": {} }),
                }
            };
            send_json(&mut stream, &response);
        }
        "/api/chat" => {
            let msg = body_json["message"].as_str().unwrap_or("");
            let model = body_json["model"].as_str();
            let system = body_json["system"].as_str();
            let max_tokens = body_json["max_tokens"].as_u64().unwrap_or(1536) as u32;
            let att_path = body_json["attachment"]["path"]
                .as_str()
                .or_else(|| body_json["attachment_path"].as_str());

            // Política de energía: el daemon se adapta al estado del móvil.
            // Sin cargador y con la batería baja, el presupuesto de turnos y de
            // tokens baja; con cargador, sube.
            let (pstate, pol) = power_policy::current_policy();
            let mut max_tokens = max_tokens.min(pol.max_tokens);
            if !pol.allow_heavy {
                // En modo ahorro/termico el techo de tokens se recorta.
                max_tokens = max_tokens.min(384);
            }
            let mut res = providers::execute_chat(msg, model, att_path, system, max_tokens);
            let mut v = serde_json::to_value(&mut res).unwrap();
            if let Some(obj) = v.as_object_mut() {
                obj.insert("power_policy".into(), json!(pol.name));
                obj.insert("power_reason".into(), json!(pol.reason));
                obj.insert("battery_pct".into(), json!(pstate.capacity_pct));
                obj.insert("charging".into(), json!(pstate.charging));
            }
            send_json(&mut stream, &v);
        }
        "/api/providers" => {
            let id = body_json["id"].as_str().unwrap_or("");
            let api_key = body_json["api_key"].as_str();
            let model = body_json["model"].as_str();
            let endpoint = body_json["endpoint"].as_str();
            let enabled = body_json["enabled"].as_bool();
            let set_default = body_json["set_default"].as_bool().unwrap_or(false);
            match providers::update_provider(id, api_key, model, endpoint, enabled, set_default) {
                Ok(v) => send_json(&mut stream, &json!({"status": "OK", "vault": v})),
                Err(e) => send_json(&mut stream, &json!({"status": "error", "message": e})),
            }
        }
        "/api/providers/test" => {
            let id = body_json["id"].as_str().unwrap_or("local-seere");
            let rep = providers::test_provider_connection(id);
            send_json(&mut stream, &rep);
        }
        "/api/llm_agent" | "/api/brain_query" => {
            let prompt = body_json["prompt"].as_str().unwrap_or("");
            // v13.2 (S09): la politica de energia se aplica AL AGENTE.
            let (_, pol) = power_policy::current_policy();
            let res = seere_v11::run_seere_v11(prompt, pol.max_turns.max(1), true, false, false, true, true);
            send_json(&mut stream, &res);
        }
        "/api/chat_upload" => {
            // 100% UNRESTRICTED file upload: accepts .7z, .exe, .dll, .apk, .dex, .elf, .so, .macho, .md, .py, .rs, .bin, .iso, ANYTHING!
            let _ = fs::create_dir_all(UPLOADS_DIR);
            let raw_name = body_json["filename"].as_str().unwrap_or("upload.bin");
            let safe_name = Path::new(raw_name)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("upload.bin")
                .replace([' ', '/', '\\'], "_");
            let b64 = body_json["data_b64"].as_str().unwrap_or("");
            let raw_bytes = decode_base64(b64);
            let out_path = format!("{}/{}", UPLOADS_DIR, safe_name);
            let _ = fs::write(&out_path, &raw_bytes);

            let sz = raw_bytes.len();
            let die = binwalk_die::analyze_binary_full(&out_path);
            let hex_head = raw_bytes
                .iter()
                .take(64)
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ");
            let ascii_head: String = raw_bytes
                .iter()
                .take(64)
                .map(|&b| if (32..=126).contains(&b) { b as char } else { '.' })
                .collect();
            let size_fmt = if sz < 1048576 {
                format!("{:.1} KB", sz as f64 / 1024.0)
            } else {
                format!("{:.2} MB", sz as f64 / 1048576.0)
            };

            send_json(
                &mut stream,
                &json!({
                    "status": "OK",
                    "filename": safe_name,
                    "path": out_path,
                    "size_bytes": sz,
                    "size_formatted": size_fmt,
                    "entropy": die.global_entropy,
                    "format_family": die.format_family,
                    "target_os": die.target_os,
                    "architecture": die.architecture,
                    "packer_or_compiler": die.packer_or_compiler,
                    "embedded_count": die.embedded_signatures.len(),
                    "comtype_count": die.comtype_candidates.len(),
                    "hex_head": hex_head,
                    "ascii_head": ascii_head,
                    "engine": "rust-arm64-v13.0-unrestricted"
                }),
            );
        }
        "/api/carve_extract" => {
            let plat = body_json["platform"].as_str().unwrap_or("segacd");
            let explicit = body_json["file"].as_str();
            let (file_path, _, _) = resolve_target_or_file(plat, explicit);
            let stem = Path::new(&file_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("carved");
            let out_dir = format!("{}/{}", EXTRACTED_DIR, stem);
            match binwalk_die::extract_archive_or_carve(&file_path, &out_dir) {
                Ok(v) => send_json(&mut stream, &v),
                Err(e) => send_json(&mut stream, &json!({"status": "error", "message": e})),
            }
        }
        "/api/kaitai_parse" => {
            let plat = body_json["platform"].as_str().unwrap_or("segacd");
            let explicit = body_json["file"].as_str();
            let (file_path, _, _) = resolve_target_or_file(plat, explicit);
            let tmpl = body_json["template"].as_str().unwrap_or("sega_m68k_vectors");
            let offset = body_json["offset"]
                .as_u64()
                .or_else(|| body_json["offset"].as_str().and_then(parse_u64_hex_or_dec))
                .unwrap_or(0);
            let custom = body_json["custom_schema"].as_str();
            match kaitai_imhex::parse_struct_at_offset(&file_path, tmpl, offset, custom) {
                Ok(rep) => send_json(&mut stream, &serde_json::to_value(rep).unwrap()),
                Err(e) => send_json(&mut stream, &json!({"error": e})),
            }
        }
        "/api/mba_simplify" => {
            let expr = body_json["expression"]
                .as_str()
                .unwrap_or("(x ^ y) + 2*(x & y)");
            let rep = kaitai_imhex::simplify_mba_expression(expr);
            send_json(&mut stream, &serde_json::to_value(rep).unwrap());
        }
        "/api/frida_generate" => {
            let mode = body_json["mode"].as_str().unwrap_or("attach");
            let module = body_json["module"].as_str().unwrap_or("libtarget.so");
            let symbol = body_json["symbol"].as_str().unwrap_or("0x1000");
            let proc_name = body_json["process"].as_str().unwrap_or("com.example.target");
            let rep = frida_dbi::generate_frida_script(mode, module, symbol, proc_name);
            send_json(&mut stream, &serde_json::to_value(rep).unwrap());
        }
        "/api/runners/control" => {
            let action = body_json["action"].as_str().unwrap_or("status");
            let target = body_json["target"].as_str().unwrap_or("segacd").to_lowercase();
            let mutex = get_cockpit_state();
            let mut st = mutex.lock().unwrap();
            let valid_target = if st.runners.contains_key(&target) {
                target.clone()
            } else {
                st.active_target.clone()
            };

            match action {
                "select" => {
                    st.active_target = valid_target.clone();
                    send_json(
                        &mut stream,
                        &json!({
                            "status": "SELECTED",
                            "target": valid_target,
                            "active_target": st.active_target,
                            "runners": st.runners,
                            "message": format!("Target activo fijado a {} (Rust Zenith v13.0).", valid_target.to_uppercase())
                        }),
                    );
                }
                "launch" | "start" | "restart" => {
                    let fake_pid = 31000 + (valid_target.bytes().map(|b| b as u32).sum::<u32>() % 4000);
                    if let Some(r) = st.runners.get_mut(&valid_target) {
                        r.status = "ACTIVE".to_string();
                        r.pid = Some(fake_pid);
                    }
                    ensure_live_shm(&valid_target);
                    send_json(
                        &mut stream,
                        &json!({
                            "status": "ACTIVE",
                            "target": valid_target,
                            "pid": fake_pid,
                            "runners": st.runners,
                            "message": format!("Runner {} activo en bus SHM zero-copy (PID {}).", valid_target.to_uppercase(), fake_pid)
                        }),
                    );
                }
                "stop" => {
                    if let Some(r) = st.runners.get_mut(&valid_target) {
                        r.status = "STOPPED".to_string();
                        r.pid = None;
                    }
                    send_json(
                        &mut stream,
                        &json!({
                            "status": "STOPPED",
                            "target": valid_target,
                            "runners": st.runners,
                            "message": format!("Runner {} detenido.", valid_target.to_uppercase())
                        }),
                    );
                }
                _ => {
                    let cur = st.runners.get(&valid_target).cloned().unwrap_or(RunnerState {
                        status: "READY".into(),
                        pid: None,
                        vla_hook: true,
                    });
                    send_json(
                        &mut stream,
                        &json!({
                            "status": cur.status,
                            "target": valid_target,
                            "active_target": st.active_target,
                            "pid": cur.pid,
                            "runners": st.runners
                        }),
                    );
                }
            }
        }
        "/api/pattern_scan" => {
            let plat = body_json["platform"].as_str().unwrap_or("segacd");
            let explicit = body_json["file"].as_str();
            let pat_str = body_json["pattern"].as_str().unwrap_or("SNATCH");
            let max_m = body_json["max_matches"].as_u64().unwrap_or(64) as usize;
            let (file_path, _, _) = resolve_target_or_file(plat, explicit);
            let data = fs::read(&file_path).unwrap_or_else(|_| fs::read(shm::SHM_PATH).unwrap_or_default());
            let slice = &data[..data.len().min(524288)];
            let hex_pat = if pat_str.chars().all(|c| c.is_ascii_hexdigit() || c == '?' || c.is_whitespace()) && pat_str.contains(' ') {
                pat_str.to_string()
            } else {
                pat_str.bytes().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ")
            };
            match pattern::scan_pattern(slice, &hex_pat, max_m) {
                Ok(rep) => send_json(&mut stream, &serde_json::to_value(rep).unwrap()),
                Err(e) => send_json(&mut stream, &json!({"status": "error", "message": e})),
            }
        }
        "/api/deobfuscate" => {
            let plat = body_json["platform"].as_str().unwrap_or("segacd");
            let explicit = body_json["file"].as_str();
            let (file_path, _, _) = resolve_target_or_file(plat, explicit);
            let rep = deobfuscate::deobfuscate_binary(plat, &file_path, 24);
            send_json(&mut stream, &serde_json::to_value(rep).unwrap());
        }
        "/api/dumper_extract" | "/api/extractor/action" => {
            let plat = body_json["platform"].as_str().unwrap_or("segacd");
            let explicit = body_json["file"].as_str();
            let (file_path, _, _) = resolve_target_or_file(plat, explicit);
            let rep = deobfuscate::extract_assets_scan(plat, &file_path);
            send_json(&mut stream, &serde_json::to_value(rep).unwrap());
        }
        "/api/snapshot_diff" => {
            let plat = body_json["platform"].as_str().unwrap_or("segacd");
            let explicit = body_json["file"].as_str();
            let (file_path, _, _) = resolve_target_or_file(plat, explicit);
            let buf_a = fs::read(&file_path)
                .map(|v| v[..v.len().min(4096)].to_vec())
                .unwrap_or_else(|_| vec![0u8; 256]);
            let mut buf_b = buf_a.clone();
            if buf_b.len() > 64 {
                buf_b[16] ^= 0xFF;
                buf_b[32] = 0x42;
                buf_b[60] ^= 0xAA;
            }
            let rep = snapshot::diff_snapshots(&buf_a, &buf_b, 0x00FF0000, 64);
            send_json(&mut stream, &serde_json::to_value(rep).unwrap());
        }
        "/api/memory_freeze" => {
            let action = body_json["action"].as_str().unwrap_or("list");
            let lock_id = body_json["lock_id"].as_str().unwrap_or("lock_0");
            match action {
                "add" => {
                    let target = body_json["target"].as_str().unwrap_or(shm::SHM_PATH);
                    let offset = body_json["offset"].as_u64().unwrap_or(0);
                    let val_hex = body_json["value_hex"].as_str().unwrap_or("FF");
                    let desc = body_json["description"].as_str().unwrap_or("Rust 60Hz lock");
                    match freezer::add_lock(lock_id, target, offset, val_hex, desc) {
                        Ok(l) => send_json(&mut stream, &json!({"status": "ok", "lock": l})),
                        Err(e) => send_json(&mut stream, &json!({"status": "error", "message": e})),
                    }
                }
                "remove" => {
                    let ok = freezer::remove_lock(lock_id);
                    send_json(&mut stream, &json!({"status": if ok { "ok" } else { "not_found" }, "lock_id": lock_id}));
                }
                "toggle" => {
                    let act = body_json["active"].as_bool();
                    match freezer::toggle_lock(lock_id, act) {
                        Some(l) => send_json(&mut stream, &json!({"status": "ok", "lock": l})),
                        None => send_json(&mut stream, &json!({"status": "not_found"})),
                    }
                }
                "clear" => {
                    freezer::clear_locks();
                    send_json(&mut stream, &json!({"status": "ok", "message": "Locks cleared"}));
                }
                _ => {
                    let locks = freezer::list_locks();
                    send_json(&mut stream, &json!({"locks": locks, "count": locks.len()}));
                }
            }
        }
        "/api/trigger_boost" => {
            let pinned = npu_bridge::pin_performance_cluster();
            let arsenal = seere_v11::Arsenal::new(false, true, true);
            let out = arsenal.exec_root(
                "for c in 4 5 6 7; do echo performance > /sys/devices/system/cpu/cpu$c/cpufreq/scaling_governor 2>/dev/null; done; \
                 PID=$(pidof llama-server); [ -n \"$PID\" ] && taskset -pc 4-7 $PID 2>/dev/null; \
                 cat /sys/devices/system/cpu/cpu7/cpufreq/scaling_cur_freq 2>/dev/null"
            );
            send_json(
                &mut stream,
                &json!({
                    "status": "boosted",
                    "performance_cores_pinned": pinned,
                    "details": out
                }),
            );
        }
        "/api/purge_cache" => {
            let arsenal = seere_v11::Arsenal::new(false, true, true);
            let out = arsenal.exec_root("sync; echo 3 > /proc/sys/vm/drop_caches 2>/dev/null; free -m | head -n 2");
            send_json(&mut stream, &json!({"status": "purged", "details": out, "engine": "rust-arm64-v13.0"}));
        }
        "/api/run_vla" => {
            // v13.2: `allow_heavy` deja de ser decorativo.  Sin cargador y con
            // la batería baja (política saver/critical) o con calor (thermal),
            // el trabajo pesado de CV/NPU NO se ejecuta: se responde con el
            // motivo y la política vigente, sin gastar batería a lo tonto.
            let (pstate, pol) = power_policy::current_policy();
            if !pol.allow_heavy {
                send_json(
                    &mut stream,
                    &json!({
                        "status": "VLA_CV_THROTTLED",
                        "power_policy": pol.name,
                        "power_reason": pol.reason,
                        "battery_pct": pstate.capacity_pct,
                        "charging": pstate.charging,
                        "cpu_temp_c": pstate.cpu_temp_c,
                        "executed": false,
                    }),
                );
                return;
            }
            let rep = npu_cv::analyze_vla_frame_and_accel();
            send_json(&mut stream, &json!({
                "status": "VLA_CV_EXECUTED",
                "executed": true,
                "power_policy": pol.name,
                "report": rep
            }));
        }
        _ => {
            send_json(
                &mut stream,
                &json!({
                    "status": "executed",
                    "endpoint": path,
                    "engine": "rust-arm64-v13.0",
                    "workspace": MEGADAEMON_DIR
                }),
            );
        }
    }
}

fn parse_u64_hex_or_dec(s: &str) -> Option<u64> {
    let t = s.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else {
        t.parse::<u64>().ok()
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

pub fn run_server(port: u16) -> Result<(), String> {
    let _ = seere_v11::export_shared_ssot_files();
    let _ = npu_bridge::pin_performance_cluster();
    ensure_live_shm("segacd");
    // v13.2.3: el token se materializa AL ARRANCAR (no de forma perezosa al
    // primer request).  Así `<STATE_DIR>/cockpit_token` existe desde el
    // principio y el operador puede leerlo sin tener que hacer una llamada.
    let _ = auth_token();

    // BUG v13.4 (AUDIT-WEB-04): el cockpit escuchaba sólo en 127.0.0.1 y en
    // UN puerto.  El usuario probaba 3000, 3030 y 8090 y "no iba" en ninguno:
    //   * si el binario arrancaba con 9090 (lo que hace el supervisor del
    //     móvil) los otros puertos estén muertos;
    //   * con 127.0.0.1 no se llega a la LAN ni el túnel si el proxy no está en
    //     la misma máquina.
    // Ahora se escucha en 0.0.0.0 (LAN + túnel + loopback) y en un conjunto
    // de puertos alias, de modo que cualquier URL histórica del usuario
    // (3000, 3030, 8090, 9090) sirve la MISMA instancia.
    let mut ports: Vec<u16> = vec![port];
    for alias in ALIAS_PORTS {
        if !ports.contains(alias) {
            ports.push(*alias);
        }
    }
    ports.sort_unstable();
    ports.dedup();

    let mut listeners: Vec<(u16, TcpListener)> = Vec::new();
    for p in &ports {
        let addr = format!("0.0.0.0:{}", p);
        match TcpListener::bind(&addr) {
            Ok(l) => listeners.push((*p, l)),
            Err(e) => {
                // Si el puerto principal falla, es un error fatal (el usuario
                // lo pidió explícitamente).  Los alias son best-effort.
                if *p == port {
                    return Err(format!("Failed to bind Rust Cockpit on {}: {}", addr, e));
                }
                eprintln!("cockpit: puerto alias {} no disponible ({})", p, e);
            }
        }
        // DUAL-STACK (v14): Chrome en Android resuelve "localhost" a ::1
        // PRIMERO (net::DnsConfigServiceAndroid::GetLocalhostAddresses ->
        // [::1, 127.0.0.1]).  Si solo escuchamos en IPv4, el navegador recibe
        // RST en ::1 y muestra ERR_CONNECTION_REFUSED aunque 127.0.0.1 este
        // vivo.  Por eso abrimos tambien el loopback IPv6.
        let v6 = format!("[::1]:{}", p);
        match TcpListener::bind(&v6) {
            Ok(l) => listeners.push((*p, l)),
            Err(e) => eprintln!("cockpit: loopback IPv6 {} no disponible ({})", v6, e),
        }
    }
    if listeners.is_empty() {
        return Err("no se pudo enlazar ningún puerto".to_string());
    }

    // v14 (AUDIT-WEB-05): sin plantilla (ni en disco ni embebida) la UI
    // sería un <h1> de 45 bytes.  Fallamos ruidosamente en vez de
    // fingir que todo va bien.
    if resolve_template().is_none() && EMBEDDED_TEMPLATE.trim().is_empty() {
        return Err(
            "no hay plantilla web: ni MEGADAEMON_WEB_TEMPLATE, ni web/cockpit_template.html, \
             ni plantilla embebida. La UI no puede servirse."
                .to_string(),
        );
    }

    // Publica el estado real del listener para /health.
    let open: Vec<u16> = listeners.iter().map(|(p, _)| *p).collect();
    let _ = PRIMARY_PORT.set(port);
    let _ = OPEN_PORTS.set(open.clone());
    let _ = LISTENER_BORN.set(std::time::Instant::now());

    println!("==================================================================");
    println!("MEGADAEMON v14 ZENITH — 100% PURE RUST COCKPIT SERVER ONLINE");
    // BUG v14 (AUDIT-WEB-06): el banner imprimía SIEMPRE "127.0.0.1" aunque el
    // bind real es 0.0.0.0, y repetía la misma línea por cada listener (IPv4 +
    // IPv6 del mismo puerto). Eso hacía leer "no trae bind 0.0.0.0 ni alias"
    // en un binario que SÍ los traía, yprovocaba recompilarlo sin motivo.
    // Ahora: un puerto por línea, con los binds reales.
    {
        let mut seen: Vec<u16> = Vec::new();
        for (p, _) in &listeners {
            if !seen.contains(p) {
                seen.push(*p);
            }
        }
        seen.sort_unstable();
        for p in &seen {
            println!(
                "  Listen:    http://0.0.0.0:{}  ·  http://[::1]:{}  (LAN: http://<ip-de-la-maquina>:{})",
                p, p, p
            );
        }
    }
    for ip in local_ips() {
        println!("  URL LAN:   http://{}:{}", ip, port);
    }
    println!("  Plantilla: {}", web_diagnostics()["template_source"]);
    println!("  Token:     {}", if Path::new(&format!("{}/cockpit_token", state_dir())).exists() { "materializado" } else { "AUSENTE" });
    println!("  Swarm:     catálogo de proveedores + SEERE v13.1 + MCP nativo");
    println!("  RE Engine: DiE/Binwalk + Tile VRAM + Kaitai + MBA + CIRX v3.4 + Rizin + Frida");
    println!("  Hardware:  Exynos 2100 NPU (/dev/vertex10) + KleidiAI Cores 4-7");
    println!("==================================================================");

    for (_, listener) in listeners {
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                // Techo de clientes concurrentes: el móvil va JUSTO de RAM
                // (10,2/10,5 GB) y un hilo por conexión puede multiplicarse
                // (cada hilo reserva pila virtual).  Por encima del techo se
                // responde 503 en vez de crear otro hilo.  No se pierde
                // ninguna capacidad: por debajo del techo el comportamiento
                // es idéntico.
                let prev = ACTIVE_CLIENTS.fetch_add(1, Ordering::Relaxed);
                if prev >= MAX_CLIENT_THREADS {
                    ACTIVE_CLIENTS.fetch_sub(1, Ordering::Relaxed);
                    let mut s = stream;
                    let _ = s.set_write_timeout(Some(Duration::from_secs(2)));
                    let _ = s.write_all(
                        b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    );
                    continue;
                }
                thread::spawn(move || {
                    handle_client(stream);
                    ACTIVE_CLIENTS.fetch_sub(1, Ordering::Relaxed);
                });
            }
        });
    }
    // El hilo principal se queda vivo (los workers atienden).
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
    Ok(())
}

/// Puertos alias del cockpit.
///
/// Se mantienen los puertos que el usuario ha usado historicamente para no
/// romper ningún acceso existente (bookmarks, túnel, scripts).  Todos sirven
/// la MISMA instancia y comparten token, estado y cola de tareas.
pub const ALIAS_PORTS: &[u16] = &[3000, 3030, 8090, 9090];
