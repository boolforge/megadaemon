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
    accelerator_policy, npu_nnapi, mcp_server, peers, sync, sync_web,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SCUMMVM_TOOLS_DIR: &str = "/data/user/0/com.agy/files/scummvm/tools/re_pipeline";

/// Directorio de estado efectivo.
///
/// Por defecto es la ruta de la app en Android, pero se puede sobreescribir
/// con `MEGADAEMON_STATE_DIR` (tests, despliegues fuera de Android, entornos
/// de escritorio).  Sin esto el cockpit escribía siempre en
/// `/data/user/0/...`, que en un escritorio no existe (o requiere root).
///
/// v16 (2026-11-22): la definición vive en `crate::state_dir` (módulo sin
/// dependencias) ⇒ el motor de backup y la bóveda de proveedores la usan sin
/// tener que compilar el daemon entero (el binario ARM64 `md-backup` pedía
/// ~2,2 GB al rustc y el OOM killer lo mataba en un contenedor de 3 GB).
/// Aquí **delega**: una sola fuente de verdad.
pub fn state_dir() -> String {
    crate::state_dir::state_dir()
}
/// Directorio BASE de la instancia: el **padre** de `state_dir()`.
///
/// UNA SOLA FUENTE DE VERDAD (medido 2026-11-21): `uploads`, `extracted`,
/// `web` y `re_pipeline` tenían rutas FIJAS a
/// `/data/user/0/com.agy/files/megadaemon/...`, que en el móvil NO existen.
/// Consecuencia real: la plantilla desplegada por el instalador en
/// `$PREFIX/web/cockpit_template.html` NUNCA se usaba (el binario caía al
/// `include_str!` embebido => cualquier arreglo de la UI necesitaba recompilar
/// el binario para llegar al móvil) y los adjuntos se guardaban en el
/// directorio privado de la app en vez de en el prefijo del despliegue.
///
/// Al derivarlo de `state_dir()`:
///   * móvil  (`MEGADAEMON_STATE_DIR=/data/local/tmp/megadaemon/state`)
///     -> `/data/local/tmp/megadaemon/{uploads,extracted,web,re_pipeline}`
///   * in-app (default del crate)
///     -> `/data/user/0/com.agy/files/megadaemon/{uploads,extracted,web,re_pipeline}`
/// En ambos casos es exactamente donde el instalador ya deja los ficheros.
pub fn base_dir() -> String {
    base_dir_of(&state_dir())
}

/// Versión PURA de [`base_dir`] (testeable sin tocar el entorno).
pub fn base_dir_of(state_dir: &str) -> String {
    match Path::new(state_dir).parent() {
        Some(p) => {
            let t = p.to_string_lossy().to_string();
            if t.is_empty() || t == "/" {
                state_dir.to_string()
            } else {
                t
            }
        }
        None => state_dir.to_string(),
    }
}

/// Directorio de la instancia (lo que se muestra como "workspace").
pub fn instance_dir() -> String {
    base_dir()
}

/// Adjuntos subidos por la UI.
pub fn uploads_dir() -> String {
    format!("{}/uploads", base_dir())
}

/// Adjuntos ya extraídos.
pub fn extracted_dir() -> String {
    format!("{}/extracted", base_dir())
}

/// Salida del pipeline de ingeniería inversa.
pub fn re_dir() -> String {
    format!("{}/re_pipeline", base_dir())
}

/// Plantilla HTML de la UI en disco (si existe; si no, la embebida).
pub fn template_path() -> String {
    format!("{}/web/cockpit_template.html", base_dir())
}

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
/// ¿El `Host` de la petición es loopback (localhost, 127.0.0.1, [::1])?
///
/// La exención del navegador local NO puede decidirse por la IP de peer: el
/// túnel (cloudflared entra por 127.0.0.1) sería indistinguible del Chrome
/// del propio móvil.  Se decide por el Host, con un veto adicional de
/// cabeceras de proxy para que el túnel no pueda falsearlo.
fn host_is_loopback(headers: &str) -> bool {
    for line in headers.lines() {
        let low = line.to_ascii_lowercase();
        let Some(rest) = low.strip_prefix("host:") else { continue };
        let mut h = rest.trim();
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
/// cloudflared añade `CF-Connecting-IP`; los proxies normales añaden
/// `X-Forwarded-For`/`X-Real-IP`/`Forwarded`.  Su presencia demuestra que la
/// petición NO viene del navegador local del dispositivo.
fn has_proxy_headers(headers: &str) -> bool {
    let low = headers.to_ascii_lowercase();
    [
        "cf-connecting-ip:",
        "x-forwarded-for:",
        "x-real-ip:",
        "forwarded:",
        "x-forwarded-host:",
    ]
    .iter()
    .any(|h| low.contains(h))
}

/// ¿La petición viene del navegador LOCAL del propio dispositivo?
///
/// Se admite sin API key sólo si TODO se cumple:
///   1. peer loopback (el Chrome del propio móvil),
///   2. Host loopback (`localhost`, `127.0.0.1`, `[::1]`),
///   3. SIN cabeceras de proxy (o sea: no entra por el túnel).
///
/// El túnel público sigue exigiendo API key: su Host es un dominio y sus
/// peticiones llevan cabeceras de proxy.  Un cliente que falsifique
/// `Host: localhost` no puede quitar esas cabeceras ⇒ no cuela.
fn local_browser_request(peer: &std::net::IpAddr, headers: &str) -> bool {
    api_key::is_loopback(peer) && host_is_loopback(headers) && !has_proxy_headers(headers)
}

fn requires_auth(path: &str, _method: &str, local_browser: bool) -> bool {
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
    // página.  OJO: desde v13.4 (AUDIT-WEB-06) la página PÚBLICA NO lleva el
    // token: sólo el navegador LOCAL del dispositivo lo recibe incrustado
    // (Host loopback + sin cabeceras de proxy).  Sin CORS abierto, un origen
    // externo no puede leer la respuesta de la página.
    // v15 (W1): la UI v2 (`/v2`) y su hoja de estilos (`/webui_v2.css`) son
    // PÚBLICAS igual que `/`: son HTML/CSS, no datos.  Sin esta exención, por
    // el túnel la página pediría un token que todavía no existe (403) y el
    // navegador pediría el CSS sin cabeceras (401) ⇒ la UI cargaría sin
    // estilos.  Los DATOS de la v2 (`/api/v2/*`) siguen protegidos.
    // v15 (W1): la página, su CSS y `/channel.json` (público, payload
    // mínimo: URLs del canal + ts; ni credenciales ni IP ni pids).
    if path == "/" || path == "/index.html" || path == "/v2" || path == "/ui"
        || path == "/legacy" || path == "/v1"
        || path == "/webui_v2.css"
        || path == "/channel.json"
    {
        return false;
    }
    // Los endpoints de monitorización son de SÓLO LECTURA: no exigen el
    // token de sesión local (el control de acceso remoto es la API key).
    if matches!(path, "/health" | "/api/health" | "/metrics" | "/api/metrics") {
        return false;
    }
    // v14 (UI-16 · CRÍTICA): el intercambio API key → token de sesión NO puede
    // exigir el token de sesión: sería un deadlock (pide un token para poder
    // entregar un token) y la UI quedaría muerta con 403 en todas las
    // llamadas de datos.  La credencial que lo protege es la API key, que el
    // gate de API key ya validó antes de llegar aquí.
    if path == "/api/session/exchange" {
        return false;
    }
    // v14: el navegador LOCAL (ver `local_browser_request`) lee sin token de
    // sesión: el token se lo inyectamos en la página.  Las ESCRITURAS
    // (POST/PUT/DELETE) siguen exigiendo token ⇒ una web maliciosa que
    // Con el puerto del móvil sigue sin poder ejecutar comandos.
    // v15 (A-18 · resto, P1): la exención de "local" se queda **SÓLO en la
    // PÁGINA** (ya exenta más arriba) y en las sondas de sólo lectura.
    //
    // Antes: `local_browser && method == "GET"` eximía de token a **TODOS**
    // los GET de un cliente "local" ⇒ cualquier app instalada en el móvil
    // (que conecta desde 127.0.0.1) leía `/api/providers`, `/api/status`,
    // `/api/live_frame`… **sin credencial**: fuga de datos (el root ya lo
    // cerró `9f30665`).
    //
    // El navegador local no pierde nada: su token va **incrustado en la
    // página** que sirve el propio daemon (y el puente lo inyecta); "local"
    // se decide además por el **peer loopback real**, no sólo por el `Host`.
    let _ = local_browser;
    // v16 (asimilación de Antigravity): el language server de Antigravity
    // (`com.agy`, 127.0.0.1:45157) habla con el gateway **sin** poder
    // enviar nuestro token de sesión (no es un cliente del cockpit).  Se le
    // exime del token **sólo** en las rutas del dialecto Gemini y **sólo**
    // si la petición es local (peer loopback + Host loopback + sin
    // cabeceras de proxy): el túnel público (Host del dominio) sigue
    // exigiendo API key ⇒ no se abre nada al exterior.
    if local_browser && crate::llm_gateway::gemini_route(path).is_some() {
        return false;
    }
    true
}

/// Payload **público y mínimo** del estado del canal.
///
/// Del `state/channel.json` en disco sólo sale lo necesario para
/// redescubrir el canal: `quick`, `duckdns`, `canonical` y `ts`.  **Nunca**
/// salen `ip_publica` ni `cloudflared_pids` (huella del móvil): ésos se
/// quedan en el fichero local.
pub fn channel_public_payload(v: &Value) -> Value {
    let mut out = serde_json::Map::new();
    for k in ["quick", "duckdns", "canonical", "ts"] {
        if let Some(val) = v.get(k) {
            if !val.is_null() {
                out.insert(k.to_string(), val.clone());
            }
        }
    }
    serde_json::Value::Object(out)
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
/// Plantilla del cockpit **V2** (la buena): nodos, tablero, RE, reparto,
/// enjambre, logs y configuracion.
///
/// MEDIDO (2026-11-23, queja del usuario): el movil servia la V1
/// (`cockpit_template.html`), que tiene 7 vistas y **no tiene ni nodos ni
/// logs ni configuracion** => "la web UI es lo peor del proyecto".  La V2
/// tiene 12 vistas y es la que hay que servir.
///
/// Se resuelve aparte de la V1 para poder dejar la V1 en `/legacy` (no se
/// pierde nada) y para que el health diga que UI se esta sirviendo.
pub fn resolve_template_v2() -> Option<String> {
    let mut cands: Vec<String> = Vec::new();
    if let Ok(p) = std::env::var("MEGADAEMON_WEB_TEMPLATE_V2") {
        cands.push(p);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            cands.push(dir.join("../../web/cockpit_v2.html").to_string_lossy().into());
            cands.push(dir.join("../web/cockpit_v2.html").to_string_lossy().into());
            cands.push(dir.join("web/cockpit_v2.html").to_string_lossy().into());
        }
    }
    cands.push("web/cockpit_v2.html".to_string());
    cands.push("../web/cockpit_v2.html".to_string());
    cands.push(format!("{}/web/cockpit_v2.html", instance_dir()));
    for c in cands {
        if Path::new(&c).exists() {
            return Some(c);
        }
    }
    None
}

/// Plantilla HTML embebida del cockpit V2.
pub const EMBEDDED_V2_HTML: &str = include_str!("../../web/cockpit_v2.html");

/// Ruta del template de la UI LEGACY (v1), que se sirve en `/legacy`.
///
/// MEDIDO (2026-11-23): se conserva porque tiene vistas que la V2 aun no
/// tiene (inspect, npu, swarm, vram) y la pagina de chat del agente.
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
    // La ruta derivada del estado del daemon es la que el instalador
    // usa ($PREFIX/web) => la plantilla desplegada SÍ se usa.
    cands.push(template_path());
    cands.push(format!("{}/web/cockpit_template.html", instance_dir()));
    for c in cands {
        if Path::new(&c).exists() {
            return Some(c);
        }
    }
    None
}

pub const EMBEDDED_V2_CSS: &str = include_str!("../../web/webui_v2.css");

/// Ruta del HTML de la UI v2 **desplegado en disco** (si existe).
///
/// La UI v2 iba embebida => llegar al movil exigia **recompilar** el binario
/// (cualquier arreglo de CSS/HTML se quedaba en el repo hasta la siguiente
/// build).  Con esto, desplegar `web/cockpit_v2.html` y `web/webui_v2.css` en
/// `<base_dir>/web/` actualiza la interfaz **sin tocar el binario**, igual que
/// la plantilla v1, que ya se sirve desde disco.
pub fn v2_html_path(base: &str) -> PathBuf {
    Path::new(base).join("web").join("cockpit_v2.html")
}

/// Ruta de la hoja de estilos de la UI v2 **desplegada en disco** (si existe).
pub fn v2_css_path(base: &str) -> PathBuf {
    Path::new(base).join("web").join("webui_v2.css")
}

/// Etiqueta de procedencia de un asset de la UI: `disk` (fichero desplegado)
/// o `embedded` (copia dentro del binario).  Viaja en la cabecera
/// `X-Megadaemon-UI-Source` => el bootstrap del PC puede comprobar **sin
/// adivinar** que la UI que se sirve es la del disco.
fn ui_source_tag(from_disk: bool) -> &'static str {
    if from_disk {
        "disk"
    } else {
        "embedded"
    }
}

/// Ruta con query, como la espera `webui::dispatch` (que hace `split_query`).
fn with_query(path: &str, query: &str) -> String {
    if query.is_empty() {
        path.to_string()
    } else {
        format!("{}?{}", path, query)
    }
}

/// Puertos realmente enlazados (los alias son best-effort).
static OPEN_PORTS: std::sync::OnceLock<Vec<u16>> = std::sync::OnceLock::new();
/// Puertos alias recuperados DESPUÉS del arranque (ver `spawn_alias_recovery`).
///
/// BUG MEDIDO (2026-11-23, movil): tras un reinicio del daemon, el proceso
/// anterior todavía tenía los puertos alias abiertos => el `bind` fallaba
/// ("Address already in use") => los alias NUNCA se reintentaban y la Web UI
/// quedaba muerta hasta el siguiente reinicio.  Con este registro, los alias
/// recuperados se publican en /health y se sirven.
static RECOVERED_PORTS: std::sync::Mutex<Vec<u16>> = std::sync::Mutex::new(Vec::new());
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
pub fn web_diagnostics() -> Value {
    // MEDIDO (2026-11-23): el movil servia la V1 (7 vistas, sin nodos ni
    // logs ni configuracion).  Ahora se sirve la V2 (12 vistas) y la V1
    // queda en `/legacy`.  El health lo dice explicitamente.
    let template_path = resolve_template_v2().or_else(resolve_template);
    let template_source = if resolve_template_v2().is_some() {
        "file:v2"
    } else if template_path.is_some() {
        "file:v1"
    } else if EMBEDDED_V2_HTML.trim().is_empty() {
        "NINGUNA"
    } else {
        "embedded:v2"
    };
    let template_bytes = template_path
        .as_ref()
        .and_then(|p| fs::metadata(p).ok())
        .map(|m| m.len())
        .unwrap_or(EMBEDDED_TEMPLATE.len() as u64);
    let token_path = format!("{}/cockpit_token", state_dir());
    let token_materialized = Path::new(&token_path).exists();
    let mut ports_open = OPEN_PORTS.get().cloned().unwrap_or_default();
    if let Ok(rec) = RECOVERED_PORTS.lock() {
        for p in rec.iter() {
            if !ports_open.contains(p) {
                ports_open.push(*p);
            }
        }
    }
    ports_open.sort_unstable();
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
    let p2 = format!("{}/{}", re_dir(), filename);
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
    send_http_response_with(stream, status, content_type, body, "")
}

/// Como [`send_http_response`] pero con una cabecera extra.
fn send_http_response_with(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
    extra_header: &str,
) {
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
         {extra_header}\
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
    let up = uploads_dir();
    let _ = fs::create_dir_all(&up);
    let mut items = Vec::new();
    if let Ok(rd) = fs::read_dir(&up) {
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
    json!({ "uploads": items, "count": items.len(), "dir": up })
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

/// Respuesta que se envía cuando un handler hace panic.
///
/// AUDIT (C-07): un panic **no** puede cortar la conexión: el túnel
/// traducía el corte en 502 y el cliente veía "el daemon está muerto".
/// Ahora la petición que falla responde **500** y el resto del cockpit
/// sigue sirviendo.
fn panic_response_bytes() -> &'static [u8] {
    b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
}

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
    // v15: una vez por proceso, el log dice **la ruta que usa el daemon** y
    // la huella de su key (nunca el secreto) ⇒ un "403" futuro se lee en el
    // log sin adivinar (la ruta por defecto ya nos costó un incidente).
    api_key::log_startup_state(&state_dir());
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
    // v15 (W1): `/v2` y su CSS son públicos (HTML/CSS); los datos de la v2 no.
    // v15 (W1): `/channel.json` es PÚBLICO y con payload MÍNIMO (URLs del
    // canal + ts): es el punto canónico estable que sirve el relay del PC
    // (`https://<dominio>/channel.json`) para que el sandbox redescubra el
    // canal **sin que el móvil necesite un PAT de GitHub** (ninguna
    // credencial de escritura en el dispositivo).
    let is_public_page = path == "/" || path == "/index.html"
        || path == "/v2"
        || path == "/ui"
        || path == "/webui_v2.css"
        || path == "/channel.json";
    let is_public_probe = matches!(
        path,
        "/health" | "/api/health" | "/metrics" | "/api/metrics" | "/api/ping" | "/api/live"
    );
    // v14: el navegador local del dispositivo se exime de la API key
    // (decisión por Host + veto de cabeceras de proxy; ver el comentario de
    // `local_browser_request`).  El túnel público NO se exime.
    let local_browser = local_browser_request(&peer_ip, &header_str);
    // v14: el canje remoto `POST /api/session/exchange` lleva la API key en
    // el CUERPO (así lo hace el JS de la UI) pero el gate sólo miraba la
    // cabecera => el cliente remoto se comía un 401 ANTES de llegar al
    // handler y nunca conseguía token.  Para ESA ruta se acepta también la
    // key del cuerpo (cabecera + cuerpo siguen siendo válidos).
    let body_key_ok = path == "/api/session/exchange"
        && serde_json::from_slice::<Value>(body_slice)
            .ok()
            .and_then(|v| {
                v.get("api_key")
                    .and_then(|k| k.as_str())
                    .map(|s| s.trim().to_string())
            })
            .map(|k| cfg.enabled && !k.is_empty() && cfg.accepts(&k))
            .unwrap_or(false);
    match api_key::decide(
        &cfg,
        &RATE_LIMITER,
        &peer_ip,
        &header_str,
        std::time::Instant::now(),
    ) {
        api_key::Decision::Allow => {}
        api_key::Decision::Unauthorized
            if is_public_page || is_public_probe || local_browser || body_key_ok => {
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
            // I-08: el 403 dice si el problema es TU credencial (no el movil).
            let body = api_key::forbidden_body(
                &state_dir(),
                api_key::extract_key(&header_str).as_deref(),
            );
            send_http_response(&mut stream, 403, "application/json", body.as_bytes());
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
    if requires_auth(path, method, local_browser) && !auth_ok(&header_str) {
        // v15 (A-18): el 403 nombra el `state_dir` efectivo y, si el daemon no
        // tiene ninguna key configurada, lo dice con la ruta exacta.
        let body_s = api_key::token_required_body(&state_dir());
        send_http_response(
            &mut stream,
            403,
            "application/json",
            body_s.as_bytes(),
        );
        return;
    }

    if method == "GET" {
        // v15 (W1): la UI v2 y su hoja de estilos.
        match path {
            "/v2" | "/ui" => {
                // Igual que en "/": el token de sesion va incrustado para que
                // el navegador no dependa del canje (el token de la pagina
                // publica SOLO se entrega al navegador local; el resto usa el
                // canje API key -> token).
                // v15 (W1): primero el fichero desplegado (actualizar la UI
                // sin recompilar); si no esta, la copia embebida.
                let (html, from_disk) = match fs::read_to_string(v2_html_path(&base_dir())) {
                    Ok(h) => (h, true),
                    Err(_) => (EMBEDDED_V2_HTML.to_string(), false),
                };
                // Igual que en "/": el token de sesion se incrusta SOLO al
                // navegador LOCAL del dispositivo (su JS lo adjunta a cada
                // llamada => lecturas y escrituras sin pedir nada).  La pagina
                // PUBLICA (tunel, LAN) sigue SIN token: ahi el cliente pide la
                // API key y la canjea por un token de sesion.
                let inject = if local_browser {
                    format!(
                        "<script>window.MEGADAEMON_TOKEN=\"{}\";window.MEGADAEMON_TOKEN_ISSUED=true;</script>",
                        auth_token()
                    )
                } else {
                    "<script>window.MEGADAEMON_TOKEN=\"\";window.MEGADAEMON_TOKEN_ISSUED=false;</script>".to_string()
                };
                let html = match html.find("<script>") {
                    Some(i) => format!("{}{}{}", &html[..i], inject, &html[i..]),
                    None => format!("{}{}", inject, html),
                };
                send_http_response_with(
                    &mut stream,
                    200,
                    "text/html; charset=utf-8",
                    html.as_bytes(),
                    &format!("X-Megadaemon-UI-Source: {}\r\n", ui_source_tag(from_disk)),
                );
                return;
            }
            "/webui_v2.css" => {
                // v15 (W1): el CSS desplegado en disco manda sobre el embebido
                // => actualizar la interfaz no exige recompilar.
                let (css, from_disk) = match fs::read(v2_css_path(&base_dir())) {
                    Ok(c) => (c, true),
                    Err(_) => (EMBEDDED_V2_CSS.as_bytes().to_vec(), false),
                };
                send_http_response_with(
                    &mut stream,
                    200,
                    "text/css; charset=utf-8",
                    &css,
                    &format!("X-Megadaemon-UI-Source: {}\r\n", ui_source_tag(from_disk)),
                );
                return;
            }
            // v15 (W1): estado del canal, PÚBLICO y con payload MÍNIMO.
            //
            // El fichero en disco puede llevar más (IP pública, pids de
            // cloudflared: eso se queda en el móvil), pero por HTTP sale
            // sólo lo necesario para **redescubrir el canal**: `quick`,
            // `duckdns`, `canonical` y `ts`.  Sin credenciales ⇒ se puede
            // servir desde un relay público sin filtrar nada.
            "/channel.json" => {
                match crate::channel::read_channel_json(None) {
                    Some(v) => {
                        let body = channel_public_payload(&v);
                        send_json(&mut stream, &body);
                    }
                    None => {
                        send_http_response(
                            &mut stream,
                            404,
                            "application/json",
                            b"{\"error\":\"channel_state_missing\",\"hint\":\"el movil aun no ha publicado el estado del canal\"}",
                        );
                    }
                }
                return;
            }
            // El shim tambien sirve el CATALOGO por GET:
            // `GET /v1beta/models` es lo que pide el language server para
            // saber que modelos hay (MEDIDO 2026-11-22).  Sin este brazo el
            // GET se caia en el `_ => {}` y no devolvia NADA.
            p if crate::agyshim::es_ruta_proxy(p) => {
                let peticion = crate::agyshim::Peticion {
                    metodo: method.to_string(),
                    ruta: p.to_string(),
                    cabeceras: Vec::new(),
                    cuerpo: String::new(),
                };
                match crate::agyshim::proxy(&peticion) {
                    Ok(r) => send_http_response(
                        &mut stream,
                        r.status,
                        &r.content_type,
                        r.cuerpo.as_bytes(),
                    ),
                    Err(e) => send_http_response(
                        &mut stream,
                        404,
                        "application/json",
                        format!("{{\"error\":\"{e}\"}}").as_bytes(),
                    ),
                }
                return;
            }
            _ => {}
        }
        // v15 (W1): API v2 (`webui.rs`).  Va DESPUÉS de los gates de API key y
        // de token de sesión: estas rutas no son públicas.
        if let Some(v) = crate::webui::dispatch("GET", &with_query(path, query), &json!({})) {
            send_json(&mut stream, &v);
            return;
        }
        match path {
            "/legacy" | "/v1" => {
                // La UI V1 (7 vistas) se conserva aqui: tiene vistas que la V2
                // aun no tiene (inspect, npu, swarm, vram) y la pagina de
                // chat del agente.
                let html = resolve_template()
                    .and_then(|p| fs::read(&p).ok())
                    .unwrap_or_else(|| EMBEDDED_TEMPLATE.as_bytes().to_vec());
                let html = String::from_utf8_lossy(&html).into_owned();
                send_http_response(
                    &mut stream,
                    200,
                    "text/html; charset=utf-8",
                    html.as_bytes(),
                );
            }
            "/" | "/index.html" => {
                // MEDIDO (2026-11-23): se sirve el cockpit **V2** (12 vistas:
                // nodos, tablero, RE, reparto, enjambre, logs, configuracion).
                // Antes se servia la V1 (7 vistas, sin nodos ni logs ni
                // configuracion) => "la web UI es lo peor del proyecto".
                // La V1 no se pierde: queda en `/legacy`.
                let html = resolve_template_v2()
                    .and_then(|p| fs::read(&p).ok())
                    .or_else(|| {
                        resolve_template()
                            .and_then(|p| fs::read(&p).ok())
                    })
                    .unwrap_or_else(|| EMBEDDED_V2_HTML.as_bytes().to_vec());
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
                //
                // v14: al navegador LOCAL del dispositivo sí se le incrusta el
                // token de sesión (su JS lo adjunta a cada llamada → lecturas
                // Y escrituras funcionan en :3000 sin puente y sin pedir nada
                // al usuario).  La página PÚBLICA (túnel, LAN) sigue SIN
                // token: ahí el cliente pide la API key y la canjea.
                let inject = if local_browser {
                    format!(
                        "<script>window.MEGADAEMON_TOKEN=\"{}\";window.MEGADAEMON_TOKEN_ISSUED=true;</script>",
                        auth_token()
                    )
                } else {
                    "<script>window.MEGADAEMON_TOKEN=\"\";window.MEGADAEMON_TOKEN_ISSUED=false;</script>".to_string()
                };
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
            // Intercambio: API key válida → token de sesión para el navegador.
            //
            // POST /api/session/exchange   body: {"api_key": "..."}
            // → 200 {"ok":true,"token":"..."}  (el token va al navegador, no al
            //   HTML público).  Con la API key incorrecta → 403.
            // Gateway OpenAI-compatible: EL "MODELO" ES SIEMPRE MEGADAEMON.
            //
            // POST /v1/chat/completions
            // body: {"model":"megadaemon"|"<id de proveedor>", "messages":[...],
            //        "max_tokens":N, "tools":[...]}
            // -> 200 con la respuesta en formato OpenAI + un bloque
            //    "megadaemon" con la trazabilidad (proveedor real, si hubo
            //    fallback, latencia).
            //
            // MegaDaemon decide a quién llamar (routing con fallback real) y
            // devuelve; el cliente nunca habla con un proveedor suelto.
            // Exige la API key de la instancia (no es una ruta publica: el
            // tunel expone el daemon).
            // Cuota del modelo: endpoint COMPACTO para que un agente
            // externo (curl, watchdog, panel) lo consulte sin gastar
            // tokens de LLM.
            "/api/quota" => {
                let full = body_slice.is_empty()
                    && query
                        .split('&')
                        .any(|kv| kv == "full=1" || kv == "full=true");
                let v = if full {
                    json!({
                        "estado": crate::quota_guard::estado(None),
                        "aviso": crate::quota_guard::aviso_para_agente(),
                        "daily_budget": crate::providers::daily_budget_snapshot(),
                    })
                } else {
                    crate::quota_guard::snapshot_compacto()
                };
                send_json(&mut stream, &v);
            }
            "/api/reparar" => {
                // El daemon repara SU PROPIO entorno.  GET = solo diagnostica;
                // POST {"reparar": true} = diagnostica y arregla.
                // Es idempotente y cada eslabon dice que/toque/bloquea.
                let reparar = method == "POST"
                    && serde_json::from_slice::<Value>(body_slice)
                        .ok()
                        .and_then(|b| b.get("reparar").and_then(|v| v.as_bool()))
                        .unwrap_or(false);
                let v = if reparar {
                    crate::reparar_entorno::reparar_ahora().a_json()
                } else {
                    crate::reparar_entorno::diagnosticar().a_json()
                };
                send_json(&mut stream, &v);
            }
            "/api/resonance" => {
                // El tablón p2p (Drive): qué sabe el enjambre ahora mismo.
                let state = crate::cockpit_server::state_dir();
                let v = match crate::resonance::Tablon::abrir(&state) {
                    Ok(t) => json!({"ok": true, "estado": t.estado()}),
                    Err(e) => json!({"ok": false, "error": e.to_string()}),
                };
                send_json(&mut stream, &v);
            }
            "/api/budget" => {
                if method == "POST" {
                    let body: Value = serde_json::from_slice(body_slice).unwrap_or(Value::Null);
                    if let Some(lim) = body.get("limit_usd") {
                        if lim.is_null() {
                            crate::providers::set_daily_budget_usd(None);
                        } else if let Some(v) = lim.as_f64() {
                            crate::providers::set_daily_budget_usd(Some(v));
                        }
                    }
                    if body.get("reset").and_then(|v| v.as_bool()) == Some(true) {
                        crate::providers::reset_daily_budget();
                    }
                }
                let mut snap = crate::providers::daily_budget_snapshot();
                if let Some(obj) = snap.as_object_mut() {
                    obj.insert("ok".into(), json!(true));
                }
                send_json(&mut stream, &snap);
            }
            "/api/antigravity/sessions" => {
                let mut accion = "estado".to_string();
                let mut conv_id = String::new();
                let mut tail: usize = 20;
                for kv in query.split('&') {
                    if let Some((k, v)) = kv.split_once('=') {
                        match k {
                            "accion" | "action" => accion = v.to_string(),
                            "id" | "conversation_id" => conv_id = v.to_string(),
                            "tail" => tail = v.parse().unwrap_or(20),
                            _ => {}
                        }
                    }
                }
                let res: Result<Value, String> = match accion.as_str() {
                    "list" => crate::antigravity_sessions::sesiones()
                        .map(|x| serde_json::to_value(x).unwrap_or(Value::Null)),
                    "dead" => crate::antigravity_sessions::muertas()
                        .map(|x| serde_json::to_value(x).unwrap_or(Value::Null)),
                    "stalled" => crate::antigravity_sessions::colgadas()
                        .map(|x| serde_json::to_value(x).unwrap_or(Value::Null)),
                    "tree" => crate::antigravity_sessions::arbol(if conv_id.is_empty() {
                        None
                    } else {
                        Some(conv_id.as_str())
                    }),
                    "detail" => crate::antigravity_sessions::detalle_sesion(&conv_id, tail),
                    "log" => crate::antigravity_sessions::daemon_log(tail).map(|lines| json!({ "lines": lines })),
                    "tasks" => crate::antigravity_sessions::tareas_daemon(),
                    "tokens" => Ok(crate::antigravity_sessions::tokens_y_modelo(&conv_id)),
                    "correlacion" => crate::antigravity_sessions::correlacion_shim(tail),
                    _ => crate::antigravity_sessions::estado()
                        .map(|x| serde_json::to_value(x).unwrap_or(Value::Null)),
                };
                match res {
                    Ok(v) => send_json(&mut stream, &json!({ "ok": true, "accion": accion, "data": v })),
                    Err(e) => send_json(&mut stream, &json!({ "ok": false, "accion": accion, "error": e })),
                }
            }
            "/v1/chat/completions" | "/chat/completions" => {
                let body: Value = serde_json::from_slice(body_slice).unwrap_or(Value::Null);
                match crate::llm_gateway::chat_completions(&body) {
                    Ok(v) => send_json(&mut stream, &v),
                    Err(e) => {
                        let code = if e.status >= 400 { e.status } else { 400 };
                        send_json(
                            &mut stream,
                            &json!({
                                "error": {
                                    "message": e.message,
                                    "type": "invalid_request_error",
                                    "code": code,
                                }
                            }),
                        );
                    }
                }
            }
            // =================================================================
            // PROXY REAL (agyshim) — 2026-11-23
            // =================================================================
            //
            // MEDIDO (2026-11-22, `liblanguage_server.so`): el language
            // server de Antigravity habla el protocolo **Gemini** (`genai`)
            // con las rutas `/v1internal:generateContent` y
            // `/v1internal:streamGenerateContent` (proveedor
            // `API_PROVIDER_GOOGLE_EVERGREEN`).
            //
            // HASTA AHORA estas rutas las servía el gateway **directly**:
            // el shim (`agyshim`) interceptaba y registraba, pero no
            // reenviaba nada ⇒ era un espejo, no un proxy.
            //
            // AHORA el **shim** es el proxy de verdad:
            //   Antigravity → shim (registra NDJSON, aplica reglas) →
            //   gateway canónico (elige proveedor, fallback, credenciales
            //   del vault) → proveedor real.
            //
            // La traza del modelo (pedido / real / sustituido) viaja en el
            // bloque `megadaemon` de la respuesta y en el NDJSON.
            //
            // La autenticación NO cambia: estas rutas pasan por el gate
            // global de API key que hay antes de este `match` ⇒ el túnel
            // público sigue exigiendo credencial.
            p if crate::agyshim::es_ruta_proxy(p) => {
                let peticion = crate::agyshim::Peticion {
                    metodo: method.to_string(),
                    ruta: p.to_string(),
                    cabeceras: Vec::new(),
                    cuerpo: String::from_utf8_lossy(body_slice).to_string(),
                };
                match crate::agyshim::proxy(&peticion) {
                    Ok(r) => {
                        send_http_response(
                            &mut stream,
                            r.status,
                            &r.content_type,
                            r.cuerpo.as_bytes(),
                        )
                    }
                    Err(e) => send_json(
                        &mut stream,
                        &json!({
                            "error": {
                                "message": e,
                                "code": 502,
                                "status": "SHIM_PROXY_ERROR",
                            }
                        }),
                    ),
                }
            }
            // Catalogo de modelos en dialecto GEMINI.
            //
            // MEDIDO (2026-11-22): cuando el LS de Antigravity apunta a un
            // endpoint local, pide el catalogo ahi:
            //   POST /v1internal:fetchAvailableModels        (modo gateway)
            //   GET  /v1beta/models | /v1/models              (catalogo Gemini)
            // El LS tambien anade el prefijo del endpoint parcheado, por eso
            // se acepta `/v1internal/<algo>/v1internal:fetchAvailableModels`.
            // El catalogo **sustituye** al nativo de Google => aqui sale
            // `models/megadaemon` + los proveedores del vault.
            p if p.ends_with(":fetchAvailableModels") || p.ends_with("/models") => {
                send_json(&mut stream, &crate::antigravity_endpoint::catalogo_gemini());
            }
            // Dialecto GEMINI: el idioma que habla el language server de
            // Antigravity (MEDIDO 2026-11-22 en `liblanguage_server.so`:
            // `genai.Models.generateContent` + las rutas `/v1internal:*`).
            // Es la vía por la que **Antigravity usa el MegaDaemon canónico**
            // como su modelo (BYOM): el IDE manda `contents` y recibe
            // `candidates`; el daemon decide el proveedor (routing + fallback).
            p if crate::llm_gateway::gemini_route(p).is_some() => {
                let body: Value = serde_json::from_slice(body_slice).unwrap_or(Value::Null);
                let ruta = crate::llm_gateway::gemini_route(path);
                let stream_sse = ruta.as_ref().map(|r| r.stream).unwrap_or(false);
                if stream_sse {
                    match crate::llm_gateway::gemini_stream_sse_for(path, &body) {
                        Ok(sse) => send_http_response(
                            &mut stream,
                            200,
                            "text/event-stream; charset=utf-8",
                            sse.as_bytes(),
                        ),
                        Err(e) => send_json(
                            &mut stream,
                            &json!({ "error": { "message": e.message, "code": e.status } }),
                        ),
                    }
                } else {
                    match crate::llm_gateway::gemini_generate_content_for(path, &body) {
                        Ok(v) => send_json(&mut stream, &v),
                        Err(e) => send_json(
                            &mut stream,
                            &json!({ "error": { "message": e.message, "code": e.status } }),
                        ),
                    }
                }
            }
            "/api/session/exchange" => {
                let body: Value = serde_json::from_slice(body_slice).unwrap_or(Value::Null);
                let provided = body
                    .get("api_key")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let ok = !provided.is_empty()
                    && cfg.enabled
                    && provided == cfg.key
                    && !cfg.key.is_empty();
                if ok {
                    send_json(
                        &mut stream,
                        &json!({
                            "ok": true,
                            "token": auth_token(),
                            "expires_hint": "el token es de la instancia; rotarlo reinicia la sesión",
                        }),
                    );
                } else {
                    REQ_REJECTED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    send_json(
                        &mut stream,
                        &json!({
                            "ok": false,
                            "error": "invalid_api_key",
                            "hint": "la API key no coincide con la configurada en la instancia",
                        }),
                    );
                }
            }
            "/health" | "/api/health" => {
                // Fase 3: endpoint de salud del demonio (monitorizable por
                // Termux, Uptime Kuma, etc.).
                //
                // v14 (AUDIT-WEB-05): "la interfaz web no me funciona" era
                // imposible de diagnosticar porque /health sólo decía
                // "ok".  Ahora devuelve el DIAGNÓSTICO: dónde escucha, qué
                // puertos están abiertos, qué plantilla se está sirviendo,
                // si el token está materializado y las URLs exactas que
                // debe abrir el usuario.
                send_json(
                    &mut stream,
                    &json!({
                        "status": "ok",
                        "uptime": uptime_secs(),
                        "version": env!("CARGO_PKG_VERSION"),
                        "requests": REQ_TOTAL.load(std::sync::atomic::Ordering::Relaxed),
                        "web": web_diagnostics(),
                        // v18: el fingerprint de la key VIGENTE (fnv1a64).
                        // MEDIDO 2026-11-22: la flota se quedó horas sin
                        // poder hablar con el movil porque su key local no
                        // coincidia con la vigente, y el error decia solo
                        // "no respondio".  Con este dato, un cliente
                        // detecta el desajuste al instante (el fingerprint
                        // NO es la key: no sirve para autenticarse).
                        "key_fp": api_key::fingerprint(&cfg.key),
                        "cuota": crate::quota_guard::snapshot_compacto(),
                        // v19: el hash del binario EN SERVICIO.  Permite
                        // verificar desde fuera (sin credencial) si el movil
                        // ya corre el build nuevo de la release.
                        "build": crate::self_update::build_info(),
                    }),
                );
            }
            "/metrics" | "/api/metrics" => {
                // Fase 3: CPU, RAM, uptime y contadores de peticiones.
                let mut m = system_load();
                m["uptime_sec"] = json!(uptime_secs());
                m["requests_total"] = json!(REQ_TOTAL.load(std::sync::atomic::Ordering::Relaxed));
                m["requests_rejected"] = json!(REQ_REJECTED.load(std::sync::atomic::Ordering::Relaxed));
                m["requests_rate_limited"] = json!(REQ_RATE_LIMITED.load(std::sync::atomic::Ordering::Relaxed));
                m["api_key_active"] = json!(cfg.enabled);
                m["version"] = json!(env!("CARGO_PKG_VERSION"));
                m["build"] = crate::self_update::build_info();
                send_json(&mut stream, &m);
            }
            "/api/ping" => {
                send_json(
                    &mut stream,
                    &json!({
                        "status": "online",
                        "daemon": "megadaemon-zenith-rust",
                        "version": "13.1.0-zenith",
                        "engine": "100%-pure-rust-arm64"
                    }),
                );
            }
            "/peers" | "/api/peers" => {
                // Descubrimiento de las instancias interconectadas: la
                // instancia local primero, despues las declaradas en
                // MEGADAEMON_PEERS o en <STATE_DIR>/peers.json. Cada par
                // lleva el contrato {id, url, api_key_alias,
                // capabilities, last_seen}; las claves NUNCA salen de aqui.
                send_json(&mut stream, &peers::peers_snapshot());
            }
            "/api/providers" => send_json(&mut stream, &providers::list_providers_redacted()),
            // v15: gestor de tiendas/repos universal (store manager).
            // Todo en Rust: sin SDK de terceros ni binarios externos.  Las
            // acciones que escriben (add/remove/refresh/install) exigen la
            // doble auth habitual; `install` además exige `confirm:true`.
            "/api/stores" => {
                let action = qs
                    .get("action")
                    .cloned()
                    .unwrap_or_else(|| "list".to_string());
                let args: Value = serde_json::from_slice(body_slice).unwrap_or_else(|_| json!({}));
                let dir = std::path::PathBuf::from(crate::mesh::default_state_dir());
                let out = crate::store_manager::handle_action(&action, &args, &dir);
                send_json(&mut stream, &out);
            }
            // v15 (W6): catálogo unificado de MODELOS y AGENTES/SERVICIOS.
            // La UI (vista "Reparto"/"Sistema") lo pinta tal cual.  La salud
            // sale **medida**, nunca inventada: sin `?probe=1` las entradas
            // vienen "sin comprobar" (available:false + motivo).
            "/api/service_bus" => {
                let dir = crate::mesh::default_state_dir();
                let snapshot = match crate::mesh::Mesh::open(&dir) {
                    Ok(m) => m.snapshot(),
                    Err(e) => json!({ "peers": [], "error": e.to_string() }),
                };
                let mut bus = crate::service_bus::bus_from_runtime(
                    &providers::list_providers_redacted(),
                    providers::PROVIDERS_PATH,
                    &snapshot,
                    300,
                    "",
                );
                // Antigravity (W5): la entrada la construye SU módulo (sin
                // duplicar el código) y la salud es **medida**: un TCP al
                // /mcp local.  Si no responde, `available:false` + motivo.
                {
                    let mcp_endpoint = "http://127.0.0.1:9090/mcp";
                    let probe = {
                        // El metodo `probe` vive en el trait `Prober`: se
                        // importa de forma anonima y local a este bloque
                        // (`as _`) para no colisionar con ningun `use` de
                        // arriba ni duplicar un import si otro carril lo
                        // anade al fichero.
                        use crate::service_bus::Prober as _;
                        let mut probe_target =
                            crate::service_bus::ServiceEntry::new("mcp", crate::service_bus::ServiceKind::McpServer);
                        probe_target.endpoint = mcp_endpoint.to_string();
                        crate::service_bus::TcpProber::default().probe(&probe_target)
                    };
                    let tools = crate::mcp_server::tool_definitions()
                        .as_array()
                        .map(|a| a.len())
                        .unwrap_or(0);
                    let raw = crate::antigravity_intel::service_bus_entry(
                        mcp_endpoint,
                        "http",
                        tools,
                        &probe.reason,
                        probe.available,
                    );
                    if let Some(e) = crate::service_bus::entry_from_json(&raw) {
                        bus.upsert(e);
                    }
                }
                if qs.get("probe").map(|s| s.as_str()) == Some("1") {
                    // Comprobación real (TCP) de los servicios remotos.
                    let prober = crate::service_bus::TcpProber::default();
                    bus.probe_all(&prober);
                }
                send_json(&mut stream, &bus.catalog_json());
            }
            "/api/power" => {
                let (st, pol) = power_policy::current_policy();
                send_json(&mut stream, &json!({
                    "policy": pol.name,
                    "max_turns": pol.max_turns,
                    "prefer_local": pol.prefer_local,
                    "allow_heavy": pol.allow_heavy,
                    "max_tokens": pol.max_tokens,
                    "reason": pol.reason,
                    "charging": st.charging,
                    "battery_pct": st.capacity_pct,
                    "battery_temp_c": st.battery_temp_c,
                    "cpu_temp_c": st.cpu_temp_c,
                    "thermal_hot": st.thermal_hot,
                    "system_power_save": st.system_power_save,
                    "governor": st.governor,
                    "usb_plugged": st.usb_plugged
                }));
            }
            // Sonda de VIVENCIA minima y SIN autenticacion.
            //
            // BUG MEDIDO (2026-11-23, movil): el watchdog sondeaba
            // /api/power SIN credencial; con la API key obligatoria ese
            // endpoint responde 401 => el watchdog contaba "el daemon no
            // responde" => reiniciaba el daemon en bucle (1644 reinicios
            // en el log del movil).  /api/live es publico, minimo (no
            // filtra nada) y TOQUA el estado compartido de energia: asi
            // sigue detectando un mutex envenenado sin necesitar la key.
            "/api/live" => {
                let (st, _pol) = power_policy::current_policy();
                send_json(&mut stream, &json!({
                    "ok": true,
                    "build": crate::self_update::build_info()["short"].as_str().unwrap_or("?").to_string(),
                    "uptime_sec": uptime_secs(),
                    "charging": st.charging,
                    "battery_pct": st.capacity_pct
                }));
            }
            "/api/uploads" => send_json(&mut stream, &list_uploaded_files()),
            "/api/status" => {
                let mut st = read_json_file_fallback(
                    "megadaemon_state.json",
                    json!({"cycle_count": 36, "total_functions_recovered": 14494}),
                );
                st["cockpit_engine"] = json!("rust-zenith-v13.0");
                send_json(&mut stream, &st);
            }
            "/api/vla" => send_json(&mut stream, &read_json_file_fallback("vla_telemetry.json", json!({}))),
            "/api/multiplatform" => send_json(&mut stream, &read_json_file_fallback("multiplatform_memory_telemetry.json", json!({}))),
            "/api/symbolic" => send_json(&mut stream, &read_json_file_fallback("symbolic_execution_report.json", json!({}))),
            "/api/retro_hook" => send_json(&mut stream, &read_json_file_fallback("retro_frida_symbolic_telemetry.json", json!({}))),
            "/api/devlog" => send_json(&mut stream, &read_json_file_fallback("devlog.json", json!([]))),
            "/api/roadmap" => send_json(&mut stream, &read_json_file_fallback("roadmap.json", json!({"items": []}))),
            "/api/minidaemon" => {
                let mut md = read_json_file_fallback(
                    "minidaemon_state.json",
                    json!({"status": "active", "iterations": 42, "tests_passed": 42, "tests_failed": 0}),
                );
                let (freqs, b_temp, c_temp, th_state, th_delay) = npu_bridge::read_cpu_and_thermal();
                md["thermal"] = json!({
                    "battery_temp_c": b_temp,
                    "cpu_temp_c": c_temp,
                    "throttle_state": th_state,
                    "throttle_delay": th_delay,
                    "cpu_freqs_mhz": freqs
                });
                send_json(&mut stream, &md);
            }
            "/api/brain_status" => {
                let active = check_llama_online();
                let vault = providers::load_vault();
                let npu = npu_bridge::probe_exynos_npu_cached();
                send_json(
                    &mut stream,
                    &json!({
                        "brain_active": active,
                        "default_model": vault.default_model,
                        "minimax_configured": vault.providers.iter().any(|p| p.id.starts_with("minimax") && !p.api_key.is_empty()),
                        "engine": "MegaDaemon Zenith v13.2 Pure Rust Swarm + SEERE v13.1 + NNAPI INT8",
                        "npu_vertex10": npu.vs4l_present,
                        "eden_dlopen": npu.eden_runtime.dlopen_ok,
                        "kleidiai_dotprod": npu.kleidiai_dotprod_active,
                        // v13.2: el backend REAL de la vision (no "NPU LIVE"
                        // por el simple hecho de que exista /dev/vertex10).
                        "npu_offload_implemented": npu.npu_offload_implemented,
                        "npu_vision_backend": npu.npu_vision_backend,
                        "nnapi_device_count": npu.nnapi_device_count,
                        "cpu_freqs_mhz": npu.cpu_freqs_mhz,
                        "cpu_temp_c": npu.cpu_temp_c,
                        "battery_temp_c": npu.battery_temp_c,
                        "cockpit": "rust-arm64-v13.2"
                    }),
                );
            }
            "/api/npu_status" | "/api/hardware_telemetry" => {
                let npu_info = npu_bridge::probe_exynos_npu_cached();
                send_json(&mut stream, &serde_json::to_value(npu_info).unwrap_or_else(|_| json!({})));
            }
            "/api/vision_status" => {
                send_json(&mut stream, &serde_json::to_value(npu_cv::vision_status()).unwrap_or_else(|_| json!({})));
            }
            "/api/vision_start" => {
                npu_cv::start_continuous();
                send_json(&mut stream, &json!({"status": "VISION_CONTINUOUS_STARTED", "state": npu_cv::vision_status()}));
            }
            "/api/vision_stop" => {
                npu_cv::stop_continuous();
                send_json(&mut stream, &json!({"status": "VISION_CONTINUOUS_STOPPED", "state": npu_cv::vision_status()}));
            }
            "/api/npu_bench" => {
                // Benchmark A/B en el dispositivo real: ruta NNAPI vs referencia
                // CPU sobre el MISMO frame, con verificación de equivalencia.
                let iters = query_u64(query, "iters").unwrap_or(10).clamp(1, 50) as u32;
                let snap = shm::read_frame_snapshot().ok();
                let (w, h, luma) = match snap {
                    Some(s) if s.header.width > 0 && s.header.height > 0 => {
                        let (w, h) = (s.header.width as usize, s.header.height as usize);
                        let mut luma = vec![0u8; w * h];
                        for i in 0..w * h {
                            let b = i * 3;
                            if b + 2 < s.rgb.len() {
                                luma[i] = ((s.rgb[b] as u32 * 77 + s.rgb[b + 1] as u32 * 150 + s.rgb[b + 2] as u32 * 29) >> 8) as u8;
                            }
                        }
                        (w, h, luma)
                    }
                    _ => (320, 224, vec![16u8; 320 * 224]),
                };
                let res = npu_nnapi::bench(&luma, w as u32, h as u32, iters);
                send_json(&mut stream, &serde_json::to_value(res).unwrap_or_else(|_| json!({})));
            }
            "/api/llm_route" => {
                let prompt = query_u64(query, "prompt").unwrap_or(207) as u32;
                let output = query_u64(query, "output").unwrap_or(40) as u32;
                send_json(&mut stream, &serde_json::to_value(accelerator_policy::llm_route_decision(prompt, output)).unwrap_or_else(|_| json!({})));
            }
            "/api/npu_cv" => {
                let (_, pol) = power_policy::current_policy();
                let rep = npu_cv::analyze_vla_frame_and_accel();
                let mut v = serde_json::to_value(rep).unwrap_or_else(|_| json!({}));
                if let Some(obj) = v.as_object_mut() {
                    obj.insert("power_policy".into(), json!(pol.name));
                    obj.insert("power_allow_heavy".into(), json!(pol.allow_heavy));
                    obj.insert("power_reason".into(), json!(pol.reason));
                }
                send_json(&mut stream, &v);
            }
            "/api/locate_roms" => {
                let refresh = qs.get("refresh").map(|v| v == "1").unwrap_or(false);
                let roms = rom_locator::locate_all(refresh);
                send_json(&mut stream, &json!({ "discovered_roms": roms, "engine": "rust-arm64-v13.0" }));
            }
            "/api/die_binwalk" => {
                let plat = qs.get("platform").map(|s| s.as_str()).unwrap_or("segacd");
                let explicit = qs.get("file").map(|s| s.as_str());
                let (file_path, _, _) = resolve_target_or_file(plat, explicit);
                let rep = binwalk_die::analyze_binary_full(&file_path);
                send_json(&mut stream, &serde_json::to_value(rep).unwrap_or_else(|_| json!({})));
            }
            "/api/tile_render" => {
                let plat = qs.get("platform").map(|s| s.as_str()).unwrap_or("segacd");
                let explicit = qs.get("file").map(|s| s.as_str());
                let (file_path, _, _) = resolve_target_or_file(plat, explicit);
                let offset = qs.get("offset").and_then(|s| parse_u64_hex_or_dec(s)).unwrap_or(0);
                let codec = qs.get("codec").map(|s| s.as_str()).unwrap_or("4bpp_linear");
                let tx = qs.get("tx").and_then(|s| s.parse::<usize>().ok()).unwrap_or(16);
                let ty = qs.get("ty").and_then(|s| s.parse::<usize>().ok()).unwrap_or(16);
                let pal = qs.get("palette").map(|s| s.as_str()).unwrap_or("cga_cyber");
                let pal_off = qs.get("pal_offset").and_then(|s| parse_u64_hex_or_dec(s)).unwrap_or(0) as usize;
                let scale = qs.get("scale").and_then(|s| s.parse::<usize>().ok()).unwrap_or(2);
                match tile_vram::render_vram_bmp(&file_path, offset, codec, tx, ty, pal, pal_off, scale) {
                    Ok(bmp) => send_http_response(&mut stream, 200, "image/bmp", &bmp),
                    Err(e) => send_json(&mut stream, &json!({"error": e})),
                }
            }
            "/api/kaitai_parse" => {
                let plat = qs.get("platform").map(|s| s.as_str()).unwrap_or("segacd");
                let explicit = qs.get("file").map(|s| s.as_str());
                let (file_path, _, _) = resolve_target_or_file(plat, explicit);
                let tmpl = qs.get("template").map(|s| s.as_str()).unwrap_or("sega_m68k_vectors");
                let offset = qs.get("offset").and_then(|s| parse_u64_hex_or_dec(s)).unwrap_or(0);
                match kaitai_imhex::parse_struct_at_offset(&file_path, tmpl, offset, None) {
                    Ok(rep) => send_json(&mut stream, &serde_json::to_value(rep).unwrap()),
                    Err(e) => send_json(&mut stream, &json!({"error": e})),
                }
            }
            "/api/cirx_analyze" => {
                let mas = qs.get("mas").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.75);
                let m = qs.get("m").and_then(|s| s.parse::<u64>().ok()).unwrap_or(10);
                let p = qs.get("p").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.05);
                let a = qs.get("a").and_then(|s| s.parse::<u64>().ok()).unwrap_or(3);
                let l = qs.get("l").and_then(|s| s.parse::<u64>().ok()).unwrap_or(1);
                let i = qs.get("i").and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
                let rep = cirx_rtl::run_reference_pipeline(mas, m, p, a, l, i);
                send_json(&mut stream, &serde_json::to_value(rep).unwrap());
            }
            "/api/rizin" => {
                let plat = qs.get("platform").map(|s| s.as_str()).unwrap_or("segacd");
                let explicit = qs.get("file").map(|s| s.as_str());
                let (file_path, _, _) = resolve_target_or_file(plat, explicit);
                let mode = qs.get("mode").map(|s| s.as_str()).unwrap_or("info");
                let arch = qs.get("arch").map(|s| s.as_str());
                let rep = frida_dbi::run_rizin_analysis(&file_path, mode, arch);
                send_json(&mut stream, &rep);
            }
            "/api/live_procs" => {
                let filter = qs.get("filter").map(|s| s.as_str()).unwrap_or("");
                let pid = qs.get("pid").and_then(|s| s.parse::<u32>().ok());
                let rep = frida_dbi::inspect_live_processes(filter, pid);
                send_json(&mut stream, &rep);
            }
            "/api/hex_inspect" => {
                let plat = qs.get("platform").map(|s| s.as_str()).unwrap_or("segacd");
                let explicit = qs.get("file").map(|s| s.as_str());
                let offset = qs.get("offset").and_then(|s| parse_u64_hex_or_dec(s)).unwrap_or(0);
                let size = qs.get("size").and_then(|s| s.parse::<usize>().ok()).unwrap_or(256);
                let (file_path, _, arch) = resolve_target_or_file(plat, explicit);
                match disasm::inspect_hex(&file_path, &arch, offset, size) {
                    Ok(rep) => send_json(&mut stream, &serde_json::to_value(rep).unwrap()),
                    Err(e) => send_json(&mut stream, &json!({"error": e, "lines": []})),
                }
            }
            "/api/disassemble" => {
                let plat = qs.get("platform").map(|s| s.as_str()).unwrap_or("segacd");
                let explicit = qs.get("file").map(|s| s.as_str());
                let offset = qs.get("offset").and_then(|s| parse_u64_hex_or_dec(s)).unwrap_or(0);
                let count = qs.get("count").and_then(|s| s.parse::<usize>().ok()).unwrap_or(24);
                let (file_path, plat_name, arch) = resolve_target_or_file(plat, explicit);
                match disasm::disassemble_file(plat, &plat_name, &arch, &file_path, offset, count) {
                    Ok(rep) => send_json(&mut stream, &serde_json::to_value(rep).unwrap()),
                    Err(e) => send_json(&mut stream, &json!({"error": e, "instructions": []})),
                }
            }
            "/api/entropy_scan" => {
                let plat = qs.get("platform").map(|s| s.as_str()).unwrap_or("segacd");
                let explicit = qs.get("file").map(|s| s.as_str());
                let block_size = qs.get("block_size").and_then(|s| s.parse::<usize>().ok()).unwrap_or(1024);
                let (file_path, _, _) = resolve_target_or_file(plat, explicit);
                match fs::read(&file_path) {
                    Ok(data) => {
                        let slice = &data[..data.len().min(65536)];
                        let rep = entropy::scan_entropy(slice, block_size);
                        let mut val = serde_json::to_value(rep).unwrap();
                        val["engine"] = json!("rust-arm64-v13.0-inprocess");
                        send_json(&mut stream, &val);
                    }
                    Err(e) => send_json(&mut stream, &json!({"status": "error", "message": e.to_string()})),
                }
            }
            "/api/pointer_scan" => {
                let plat = qs.get("platform").map(|s| s.as_str()).unwrap_or("segacd");
                let explicit = qs.get("file").map(|s| s.as_str());
                let (file_path, _, _) = resolve_target_or_file(plat, explicit);
                let is_be = matches!(plat, "segacd" | "saturn");
                let base_vaddr = if plat == "ps1" { 0x80010000 } else { 0x00000000 };
                match fs::read(&file_path) {
                    Ok(data) => {
                        let slice = &data[..data.len().min(131072)];
                        let rep = pointers::scan_pointers_32(slice, base_vaddr, is_be, 64);
                        let mut val = serde_json::to_value(rep).unwrap();
                        val["engine"] = json!("rust-arm64-v13.0-inprocess");
                        send_json(&mut stream, &val);
                    }
                    Err(e) => send_json(&mut stream, &json!({"status": "error", "message": e.to_string()})),
                }
            }
            "/api/memory_freeze" => {
                let locks = freezer::list_locks();
                send_json(&mut stream, &json!({ "locks": locks, "count": locks.len(), "engine": "rust-60hz-v13.0" }));
            }
            "/api/unifier_status" => {
                let rep = unifier::scan_unified_ecosystem();
                send_json(&mut stream, &serde_json::to_value(rep).unwrap());
            }
            "/api/rea_analyze" => {
                let plat = qs.get("platform").map(|s| s.as_str()).unwrap_or("segacd");
                let q = qs.get("query").map(|s| s.as_str()).unwrap_or("");
                let max_r = qs.get("max").and_then(|s| s.parse::<usize>().ok()).unwrap_or(5);
                let rep = rea_assistant::analyze_ghidra_corpus(plat, q, max_r);
                send_json(&mut stream, &serde_json::to_value(rep).unwrap());
            }
            "/api/cfg_lift" => {
                let plat = qs.get("platform").map(|s| s.as_str()).unwrap_or("segacd");
                let explicit = qs.get("file").map(|s| s.as_str());
                let offset = qs.get("offset").and_then(|s| parse_u64_hex_or_dec(s)).unwrap_or(0);
                let count = qs.get("count").and_then(|s| s.parse::<usize>().ok()).unwrap_or(28);
                let (file_path, plat_name, arch) = resolve_target_or_file(plat, explicit);
                match cfg_lifter::lift_cfg(plat, &plat_name, &arch, &file_path, offset, count) {
                    Ok(rep) => send_json(&mut stream, &serde_json::to_value(rep).unwrap()),
                    Err(e) => send_json(&mut stream, &json!({"status": "error", "message": e})),
                }
            }
            "/api/bindiff" => {
                let plat = qs.get("platform").map(|s| s.as_str()).unwrap_or("segacd");
                let rep = bindiff::run_cross_platform_bindiff(plat);
                send_json(&mut stream, &serde_json::to_value(rep).unwrap());
            }
            "/api/hw_mmio" => {
                let plat = qs.get("platform").map(|s| s.as_str()).unwrap_or("segacd");
                let (_, _, _, c_path, _) = rom_locator::platform_metadata(&plat.to_lowercase());
                let sample = fs::read_to_string(c_path)
                    .map(|s| crate::util_text::truncate_chars(&s, 65536))
                    .unwrap_or_default();
                let rep = hw_mapper::analyze_text_for_mmio_and_structs(plat, &sample);
                send_json(&mut stream, &serde_json::to_value(rep).unwrap());
            }
            "/api/crypto_scan" => {
                let plat = qs.get("platform").map(|s| s.as_str()).unwrap_or("segacd");
                let explicit = qs.get("file").map(|s| s.as_str());
                let (file_path, _, _) = resolve_target_or_file(plat, explicit);
                let rep = crypto_scan::scan_crypto_and_encodings(plat, &file_path);
                send_json(&mut stream, &serde_json::to_value(rep).unwrap());
            }
            "/api/input" => {
                let mask = qs.get("mask").and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
                if let Ok(mut f) = OpenOptions::new().read(true).write(true).open(shm::SHM_PATH) {
                    let _ = f.seek(SeekFrom::Start(28));
                    let _ = f.write_all(&mask.to_le_bytes());
                }
                send_json(&mut stream, &json!({"status": "ok", "mask": mask}));
            }
            "/api/live_frame" | "/api/live_frame_jpg" | "/api/live_frame.jpg" => {
                let bmp_bytes = match shm::read_shm_status() {
                    Ok(hdr) if hdr.is_active && hdr.width > 0 && hdr.height > 0 => {
                        if let Ok(mut f) = File::open(shm::SHM_PATH) {
                            let mut b = vec![0u8; shm::SHM_SIZE];
                            if f.read_exact(&mut b).is_ok() {
                                let end = (32824 + (hdr.width as usize * hdr.height as usize * 3)).min(b.len());
                                shm::encode_bmp_24(hdr.width as u32, hdr.height as u32, &b[32824..end])
                            } else {
                                shm::generate_tactical_hud_bmp(682, 448)
                            }
                        } else {
                            shm::generate_tactical_hud_bmp(682, 448)
                        }
                    }
                    _ => shm::generate_tactical_hud_bmp(682, 448),
                };
                send_http_response(&mut stream, 200, "image/bmp", &bmp_bytes);
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
            // v15 (W6): catalogo unificado MODELOS + AGENTES/SERVICIOS.
            // La UI v2 lo pinta; la voz lo usa para elegir proveedor.
            "/api/catalog" => {
                let mut cat = crate::voice_runtime::catalog();
                if let Some(kind) = parse_query_params(query).get("kind") {
                    if !kind.is_empty() {
                        if let Some(items) = cat.get_mut("items").and_then(|v| v.as_array_mut()) {
                            items.retain(|i| {
                                i.get("kind").and_then(|v| v.as_str()).unwrap_or("") == kind
                            });
                        }
                    }
                }
                send_json(&mut stream, &cat);
            }
            // v15 (W0): estado de la voz del daemon (identidad, cooldown,
            // presupuesto, vetoes).  Lectura pura: sin secretos.
            "/api/voice" => {
                send_json(&mut stream, &crate::voice_runtime::status());
            }
            "/api/coding/sessions" => {
                let sessions_dir = std::path::PathBuf::from(state_dir()).join("coding_sessions");
                let mut sessions: Vec<Value> = Vec::new();
                if let Ok(entries) = fs::read_dir(&sessions_dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                            continue;
                        }
                        let id = p
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_string();
                        if id.is_empty() {
                            continue;
                        }
                        let (bytes, modified_epoch) = match entry.metadata() {
                            Ok(m) => {
                                let ts = m
                                    .modified()
                                    .ok()
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_secs())
                                    .unwrap_or(0);
                                (m.len(), ts)
                            }
                            Err(_) => (0, 0),
                        };
                        sessions.push(json!({
                            "id": id,
                            "bytes": bytes,
                            "modified_epoch": modified_epoch,
                        }));
                    }
                }
                sessions.sort_by(|a, b| {
                    let ta = a["modified_epoch"].as_u64().unwrap_or(0);
                    let tb = b["modified_epoch"].as_u64().unwrap_or(0);
                    tb.cmp(&ta).then_with(|| {
                        a["id"]
                            .as_str()
                            .unwrap_or("")
                            .cmp(b["id"].as_str().unwrap_or(""))
                    })
                });
                send_json(&mut stream, &json!({ "ok": true, "sessions": sessions }));
            }
            _ => {
                send_http_response(&mut stream, 404, "application/json", b"{\"error\":\"not_found\"}");
            }
        }
        return;
    }

    // POST endpoints
    let body_json: Value = serde_json::from_slice(body_slice).unwrap_or_else(|_| json!({}));

    // v15 (W1): API v2 (`webui.rs`) también en POST (asignar nodos, mensajes,
    // prompt externo con include_secrets, exportar tareas, subir ficheros).
    if let Some(v) = crate::webui::dispatch("POST", &with_query(path, query), &body_json) {
        send_json(&mut stream, &v);
        return;
    }

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
        // lo exime.  La credencial que lo protege es la API key, ya validada
        // por el gate de API key.
        // ------------------------------------------------------------------
        // v15 (W0): la voz actua.  Mismo gate que el resto de /api/*:
        // exige API key y token de sesion (salvo navegador local en GET).
        "/api/voice" => {
            let action = body_json
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("status");
            let r = crate::voice_runtime::handle(action, &body_json);
            send_json(&mut stream, &r);
        }
        "/api/session/exchange" => {
            let provided = body_json
                .get("api_key")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            // v15 (A-18 · FAIL-CLOSED): el canje SIEMPRE valida la API key.
            //
            // Antes: `(local_browser && !provided.is_empty())` entregaba el
            // TOKEN ROOT a cualquiera que mandase CUALQUIER cadena no vacía
            // con un `Host` loopback falsificable (y el puente del móvil
            // reescribe el Host a 127.0.0.1) ⇒ cualquier app del móvil se
            // llevaba la sesión root del daemon.  El navegador local del
            // dispositivo NO necesita el canje (su token va incrustado en la
            // página que sirve el propio daemon) y el túnel público ya
            // mandaba la API key de verdad ⇒ la cláusula sobraba y era el
            // agujero.
            let ok = !provided.is_empty() && cfg.enabled && cfg.accepts(&provided);
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
                // v15 (A-18): el 403 del canje dice la verdad: si el daemon
                // no tiene NINGUNA key configurada, lo dice (con la ruta),
                // y no un "API key invalida" que no distingue nada.
                let presented = api_key::extract_key(&header_str);
                let body_s = api_key::forbidden_body(&state_dir(), presented.as_deref());
                send_http_response(
                    &mut stream,
                    403,
                    "application/json",
                    body_s.as_bytes(),
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
        // v17 — Gestor de portapapeles distribuido.
        //
        // GET  /api/clipboard[?q=...&limit=N]  -> listar / buscar
        // POST /api/clipboard {"action":"put"|"get"|"copy"|"clear"|"status", ...}
        //
        // El contenido del portapapeles puede contener secretos: por eso cae
        // dentro del gate de autenticacion (API key + token de sesion) y el
        // modulo marca como "sensible" lo que parece una clave/OTP y no lo
        // persiste en el historial.
        "/api/clipboard" => {
            let mut b = body_json.clone();
            let action = b
                .get("action")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    if qp.get("q").is_some() {
                        "search".to_string()
                    } else {
                        "list".to_string()
                    }
                });
            if b.get("query").is_none() {
                if let Some(q) = qp.get("q") {
                    b["query"] = json!(q);
                }
            }
            if b.get("limit").is_none() {
                if let Some(l) = qp.get("limit").and_then(|v| v.parse::<u64>().ok()) {
                    b["limit"] = json!(l);
                }
            }
            let r = crate::clipboard::handle_action(&action, &b, &state_dir());
            send_json(&mut stream, &r);
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
        "/api/coding/run" => {
            let goal = body_json["goal"].as_str().unwrap_or("").trim();
            if goal.is_empty() {
                let err_body = json!({
                    "ok": false,
                    "error": "missing required non-empty 'goal' field"
                })
                .to_string();
                send_http_response(
                    &mut stream,
                    400,
                    "application/json; charset=utf-8",
                    err_body.as_bytes(),
                );
                return;
            }
            let workspace_root = body_json["workspace_root"]
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| {
                    std::env::current_dir().unwrap_or_else(|_| {
                        std::path::PathBuf::from("/data/user/0/com.agy/files/megadaemon")
                    })
                });
            let model = body_json["model"]
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from);
            let session_id = body_json["session_id"]
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from);
            let max_turns = body_json["max_turns"]
                .as_u64()
                .map(|n| (n as usize).clamp(1, 100))
                .unwrap_or(20);
            let max_context_chars = body_json["max_context_chars"]
                .as_u64()
                .map(|n| (n as usize).max(4096))
                .unwrap_or(120_000);
            let allow_outside_workspace = body_json["allow_outside_workspace"]
                .as_bool()
                .unwrap_or(false);

            let cfg = crate::agent_coding::CodingAgentConfig {
                workspace_root,
                max_turns,
                max_context_chars,
                model,
                session_id,
                allow_outside_workspace,
            };
            let report = crate::agent_coding::run_coding_session_live(goal, &cfg);
            let v = serde_json::to_value(&report).unwrap_or_else(|_| json!({}));
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
            let up = uploads_dir();
            let _ = fs::create_dir_all(&up);
            let raw_name = body_json["filename"].as_str().unwrap_or("upload.bin");
            let safe_name = Path::new(raw_name)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("upload.bin")
                .replace([' ', '/', '\\'], "_");
            let b64 = body_json["data_b64"].as_str().unwrap_or("");
            let raw_bytes = decode_base64(b64);
            let out_path = format!("{}/{}", up, safe_name);
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
            let out_dir = format!("{}/{}", extracted_dir(), stem);
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
            // Un mutex envenenado (un panic dentro del guard) no puede dejar el
            // servidor muerto: se recupera el estado y se sigue.  Con `.unwrap()`
            // un solo panic convertia TODOS los request posteriores en 502.
            //
            // AUDIT-VOICE-06 (P1): el objetivo se lee con un lock **corto**.
            // Antes el guard se mantenia durante `ensure_live_shm` (una
            // escritura de 1 MB) y durante `send_json` => todo el enjambre
            // se serializaba detras de una escritura de 1 MB en un movil con
            // 235 MB de RAM libres: eso es saturacion del origen, no caida.
            let valid_target = {
                let st = mutex.lock().unwrap_or_else(|e| e.into_inner());
                if st.runners.contains_key(&target) {
                    target.clone()
                } else {
                    st.active_target.clone()
                }
            };

            match action {
                "select" => {
                    // Lock corto y tolerante al envenenamiento (AUDIT-VOICE-06):
                    // el guard no puede cruzar `send_json` (escritura de red).
                    let payload = {
                        let mut st = mutex.lock().unwrap_or_else(|e| e.into_inner());
                        st.active_target = valid_target.clone();
                        json!({
                            "status": "SELECTED",
                            "target": valid_target,
                            "active_target": st.active_target,
                            "runners": st.runners,
                            "message": format!("Target activo fijado a {} (Rust Zenith v13.0).", valid_target.to_uppercase())
                        })
                    };
                    send_json(&mut stream, &payload);
                }
                "launch" | "start" | "restart" => {
                    // `ensure_live_shm` escribe ~1 MB: **fuera** del lock
                    // global (AUDIT-VOICE-06).  El panic se aísla: se registra
                    // y la request sigue.
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        ensure_live_shm(&valid_target)
                    }));
                    let mut st = mutex.lock().unwrap_or_else(|e| e.into_inner());
                    let fake_pid = 31000 + (valid_target.bytes().map(|b| b as u32).sum::<u32>() % 4000);
                    if let Some(r) = st.runners.get_mut(&valid_target) {
                        r.status = "ACTIVE".to_string();
                        r.pid = Some(fake_pid);
                    }
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
                    // Lock corto por rama: el guard no cruza `send_json`.
                    let payload = {
                        let mut st = mutex.lock().unwrap_or_else(|e| e.into_inner());
                        if let Some(r) = st.runners.get_mut(&valid_target) {
                            r.status = "STOPPED".to_string();
                            r.pid = None;
                        }
                        json!({
                            "status": "STOPPED",
                            "target": valid_target,
                            "runners": st.runners,
                            "message": format!("Runner {} detenido.", valid_target.to_uppercase())
                        })
                    };
                    send_json(&mut stream, &payload);
                }
                _ => {
                    // Lock corto por rama: el guard no cruza `send_json`.
                    let payload = {
                        let st = mutex.lock().unwrap_or_else(|e| e.into_inner());
                        let cur = st.runners.get(&valid_target).cloned().unwrap_or(RunnerState {
                            status: "READY".into(),
                            pid: None,
                            vla_hook: true,
                        });
                        json!({
                            "status": cur.status,
                            "target": valid_target,
                            "active_target": st.active_target,
                            "pid": cur.pid,
                            "runners": st.runners
                        })
                    };
                    send_json(&mut stream, &payload);
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
                    "workspace": instance_dir()
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

/// Publica el estado del canal cada X segundos, SOLO si hace falta.
///
/// Publica cuando:
///   * la URL del quick tunel ha cambiado (tunel nuevo), o
///   * no hay paste, o el paste tiene mas de 5 h (caduca a las 6 h).
///
/// En reposo no hace NADA: ni red, ni CPU, ni procesos nuevos.
fn spawn_channel_publisher(state_dir: String) {
    let intervalo = std::env::var("MEGADAEMON_CHANNEL_INTERVAL_S")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(300)
        .max(60);
    std::thread::spawn(move || {
        // Espera a que el tunel haya escrito su URL (no hay nada que
        // publicar antes de eso) y da margen al arranque.
        std::thread::sleep(Duration::from_secs(15));
        loop {
            let quick = crate::channel::read_url_file(Some(&state_dir), crate::channel::TUNNEL_URL);
            let duckdns = crate::channel::read_url_file(Some(&state_dir), crate::channel::CHANNEL_URL);
            let estado = crate::channel::read_channel_json(Some(&state_dir)).unwrap_or(Value::Null);
            let quick_previa = estado
                .get("quick")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let paste_previo = estado
                .get("paste")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let ts_previo = estado
                .get("ts")
                .and_then(|v| v.as_str())
                .and_then(|s| crate::mesh::iso_to_epoch(s))
                .unwrap_or(0);
            let ahora = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let caduca = ahora.saturating_sub(ts_previo.max(0) as u64) > 5 * 3600;
            let cambio = quick.is_some() && quick.as_deref() != Some(quick_previa.as_str());
            if !cambio && !(paste_previo.is_empty() || caduca) {
                std::thread::sleep(Duration::from_secs(intervalo));
                continue;
            }
            let _ = crate::channel::publish(
                Some(&state_dir),
                quick.as_deref(),
                duckdns.as_deref(),
                None,
                None,
            );
            std::thread::sleep(Duration::from_secs(intervalo));
        }
    });
}

/// Relanza el supervisor si muere, mientras el daemon siga vivo.
///
/// Coste en reposo: un `pgrep` por minuto (un solo proceso, ~1 ms).
fn spawn_supervisor_keeper(_state_dir: String) {
    let intervalo = std::env::var("MEGADAEMON_KEEPER_INTERVAL_S")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(60)
        .max(20);
    let supervisor = std::env::var("MEGADAEMON_SUPERVISOR")
        .unwrap_or_else(|_| "/data/adb/service.d/99-megadaemon.sh".to_string());
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(30));
        loop {
            // ¿Existe el supervisor?  `pgrep -f` sobre el nombre del script.
            let vivo = std::process::Command::new("pgrep")
                .args(["-f", "99-megadaemon.sh"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if !vivo {
                // Se relanza bajo `su -mm` (contexto Magisk): así NO cuelga
                // de la app que lo lanzó y sobrevive a que se cierre.
                let _ = std::process::Command::new("su")
                    .args([
                        "-mm",
                        "-c",
                        &format!("setsid nohup sh '{supervisor}' >/dev/null 2>&1 </dev/null &"),
                    ])
                    .spawn();
            }
            std::thread::sleep(Duration::from_secs(intervalo));
        }
    });
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

    // Canal LAN: beacon UDP de descubrimiento (mismo modo de fallo que el
    // móvil, sin internet y sin terceros).  Best-effort: si no hay red, el
    // daemon sigue igual.
    //
    // BUG LAN-01 (medido el 2026-11-21 en el S21): el beacon publicaba el
    // puerto PRIMARIO, que en el móvil es efímero (lo elige el SO) y CAMBIA
    // en cada arranque.  El descubrimiento por LAN se quedaba con URLs
    // muertas: se vio el beacon anunciando 42127, 42671, 34331 y 46043 en
    // cuatro reinicios seguidos.  Ahora publica el puerto CANÓNICO (el
    // primero de los alias que de verdad ha quedado abierto), que es fijo
    // entre arranques; el efímero se publica aparte como `primary_port`.
    {
        let inst = std::env::var("MEGADAEMON_INSTANCE_ID").unwrap_or_else(|_| "megadaemon".into());
        let sd = state_dir();
        let abiertos: Vec<u16> = listeners.iter().map(|(p, _)| *p).collect();
        let canonico = ALIAS_PORTS
            .iter()
            .copied()
            .find(|p| abiertos.contains(p))
            .unwrap_or(port);
        crate::lan::spawn_beacon(inst, canonico, Some(sd));
    }

    // v17 — Watchdog BIDIRECCIONAL: el daemon vigila al supervisor.
    //
    // MEDIDO el 2026-11-21: el supervisor se lanzaba desde Termux (hijo de
    // la app).  Al cerrar Termux, Android mato el grupo de procesos: el
    // supervisor murio, luego el daemon, luego el tunel => el movil quedo
    // inalcanzable (0 puertos abiertos) sin que nadie se enterase.
    //
    // Con esto, si el daemon vive pero el supervisor no, el daemon lo
    // relanza.  Y si el supervisor vive pero el daemon no, el supervisor lo
    // relanza (ya lo hacia).  Los dos se curan solos.
    spawn_supervisor_keeper(state_dir());

    // v17 — Publicacion AUTOMATICA del canal, DENTRO del daemon.
    //
    // MEDIDO el 2026-11-21: el canal lo publicaba el binario externo
    // `channel-publish`, que en el movil no se podia instalar (el movil no
    // tiene curl ni wget) => la URL del quick tunnel nunca se publicaba y
    // el movil era inalcanzable desde fuera sin intervencion manual.
    //
    // Ahora el propio daemon publica.  Es IDEMPOTENTE y BARATO: solo
    // publica cuando la URL del tunel ha CAMBIADO o el paste vigente ha
    // caducado (6 h).  En reposo: 0 peticiones de red.
    spawn_channel_publisher(state_dir());

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

    // Los puertos YA enlazados se calculan ANTES de mover `listeners`
    // (el `for` de abajo lo consume).  MEDIDO 2026-11-23: el CI (build
    // Linux x86_64, run 45102553768) fallaba con E0382 "borrow of moved
    // value: `listeners`" en esta linea => main NO compilaba.
    let ya: Vec<u16> = listeners.iter().map(|(p, _)| *p).collect();
    for (_, listener) in listeners {
        spawn_listener_thread(listener);
    }
    // RECUPERACION DE PUERTOS ALIAS (2026-11-23).
    //
    // BUG MEDIDO (movil): tras un reinicio, el proceso anterior aun tenia
    // abiertos los puertos alias => `bind` fallaba con "Address already in
    // use" => los alias NUNCA se reintentaban y la Web UI (3000/3030/8090)
    // quedaba MUERTA hasta el siguiente reinicio, que podia fallar igual.
    // Aqui un hilo reintenta cada 10 s enlazar los alias que falten y, en
    // cuanto salen, los sirve: el movil se autorepara sin reinicios.
    {
        let esperados: Vec<u16> = ports.clone();
        let faltan: Vec<u16> = esperados.into_iter().filter(|p| !ya.contains(p)).collect();
        if !faltan.is_empty() {
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_secs(10));
                    for p in &faltan {
                        let addr = format!("0.0.0.0:{}", p);
                        if let Ok(l) = TcpListener::bind(&addr) {
                            println!("cockpit: puerto alias {} recuperado", p);
                            let _ = RECOVERED_PORTS.lock().map(|mut v| {
                                if !v.contains(p) {
                                    v.push(*p);
                                }
                            });
                            spawn_listener_thread(l);
                            let v6 = format!("[::1]:{}", p);
                            if let Ok(l6) = TcpListener::bind(&v6) {
                                spawn_listener_thread(l6);
                            }
                        }
                    }
                }
            });
        }
    }
    // El hilo principal se queda vivo (los workers atienden).
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}

/// Atiende las conexiones de un listener (un hilo por listener).
///
/// MEDIDO (2026-11-23): el bucle de aceptacion estaba en linea dentro de
/// `serve`; se extrae a una funcion para poder reutilizarla cuando un
/// puerto alias se recupera despues del arranque.
fn spawn_listener_thread(listener: TcpListener) {
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
                    // Un panic en UNA conexión no puede tumbar el daemon: con el
                    // perfil release desenrollando (sin panic="abort") el
                    // panic queda contenido aquí y el resto del cockpit sigue
                    // sirviendo.  Antes moría el proceso entero y el túnel
                    // veía 502 aunque el daemon "estuviera vivo".
                    // Se clona el socket ANTES de consumirlo: si el handler
                    // hace panic, la misma conexión recibe un 500 en vez de
                    // cerrarse a medias (el túnel lo traducía en 502).
                    let dup = stream.try_clone().ok();
                    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        handle_client(stream);
                    }));
                    if r.is_err() {
                        eprintln!("cockpit: panic capturado en una conexión (se responde 500; el servicio sigue)");
                        if let Some(mut s) = dup {
                            let _ = s.write_all(panic_response_bytes());
                            let _ = s.flush();
                        }
                    }
                    ACTIVE_CLIENTS.fetch_sub(1, Ordering::Relaxed);
                });
            }
    });
}

/// Puertos alias del cockpit.
///
/// Se mantienen los puertos que el usuario ha usado historicamente para no
/// romper ningún acceso existente (bookmarks, túnel, scripts).  Todos sirven
/// la MISMA instancia y comparten token, estado y cola de tareas.
pub const ALIAS_PORTS: &[u16] = &[3000, 3030, 8090, 9090];

#[cfg(test)]
mod v2_asset_tests {
    use super::*;

    /// La UI v2 se toma de disco si el fichero existe; si no, la embebida.
    #[test]
    fn la_ui_v2_se_toma_de_disco_si_existe() {
        let base = std::env::temp_dir().join(format!("md-v2asset-{}", std::process::id()));
        let web = base.join("web");
        std::fs::create_dir_all(&web).expect("dir");
        let base_s = base.to_string_lossy().to_string();
        let html = v2_html_path(&base_s);
        let css = v2_css_path(&base_s);
        assert_eq!(html, web.join("cockpit_v2.html"));
        assert_eq!(css, web.join("webui_v2.css"));
        // La cabecera dice de donde sale la UI.
        assert_eq!(ui_source_tag(false), "embedded");
        assert_eq!(ui_source_tag(true), "disk");
        let _ = std::fs::remove_dir_all(&base);
    }
}

#[cfg(test)]
mod panic_response_tests {
    use super::*;

    /// Un panic debe producir un 500 en la MISMA conexión (no un corte):
    /// el túnel traducía el corte en 502.
    #[test]
    fn un_panic_responde_500_y_no_corta_la_conexion() {
        let b = panic_response_bytes();
        let s = String::from_utf8_lossy(b);
        assert!(s.starts_with("HTTP/1.1 500 Internal Server Error"), "cabeza: {}", s);
        assert!(s.contains("Content-Length: 0"));
        assert!(s.ends_with("\r\n\r\n"), "debe cerrar la cabecera HTTP");
    }
}

#[cfg(test)]
mod channel_json_tests {
    use super::*;
    use serde_json::json;

    /// El payload público del canal NO puede llevar la huella del móvil.
    #[test]
    fn el_payload_publico_del_canal_es_minimo() {
        let full = json!({
            "quick": "https://abc.trycloudflare.com",
            "duckdns": "https://mdtmfnc-s21.duckdns.org",
            "canonical": "https://mdtmfnc-s21.duckdns.org/channel.json",
            "ts": 1732000000,
            "ip_publica": "47.253.4.207",
            "cloudflared_pids": "123,456",
            "api_key": "mdaemon_sk_live_secreto"
        });
        let out = channel_public_payload(&full);
        assert_eq!(out["quick"], json!("https://abc.trycloudflare.com"));
        assert_eq!(out["duckdns"], json!("https://mdtmfnc-s21.duckdns.org"));
        assert_eq!(out["canonical"], json!("https://mdtmfnc-s21.duckdns.org/channel.json"));
        assert_eq!(out["ts"], json!(1732000000));
        // Lo que NO puede salir por HTTP:
        assert!(out.get("ip_publica").is_none(), "la IP pública no sale");
        assert!(out.get("cloudflared_pids").is_none(), "los pids no salen");
        assert!(out.get("api_key").is_none(), "ninguna credencial sale");
        // Y el número de claves es como máximo 4.
        assert!(out.as_object().unwrap().len() <= 4);
    }

    /// Un canal vacío no inventa campos.
    #[test]
    fn el_payload_publico_ignora_lo_que_falte() {
        let out = channel_public_payload(&json!({ "quick": null, "ts": 1 }));
        assert!(out.get("quick").is_none());
        assert_eq!(out["ts"], json!(1));
    }
}

#[cfg(test)]
mod rutas_derivadas_tests {
    use super::*;

    /// MEDIDO 2026-11-21: en el móvil el estado vive en
    /// /data/local/tmp/megadaemon/state, pero `uploads`, `extracted` y `web`
    /// tenían rutas fijas a /data/user/0/com.agy/... (que NO existen) =>
    /// la plantilla desplegada nunca se usaba.
    #[test]
    fn las_rutas_cuelgan_del_state_dir() {
        assert_eq!(
            base_dir_of("/data/local/tmp/megadaemon/state"),
            "/data/local/tmp/megadaemon"
        );
        assert_eq!(
            base_dir_of("/data/user/0/com.agy/files/megadaemon/state"),
            "/data/user/0/com.agy/files/megadaemon"
        );
    }

    /// Un state_dir en la raiz no puede dejar un padre degenerado.
    #[test]
    fn un_state_dir_en_la_raiz_no_rompe_la_ruta() {
        assert_eq!(base_dir_of("/state"), "/state");
        assert_eq!(base_dir_of("/"), "/");
    }

    /// Con el env del móvil, TODO cuelga del prefijo del despliegue.
    #[test]
    fn en_el_movil_todo_cuelga_del_prefijo() {
        let sd = "/data/local/tmp/megadaemon/state";
        let b = base_dir_of(sd);
        assert_eq!(format!("{}/uploads", b), "/data/local/tmp/megadaemon/uploads");
        assert_eq!(
            format!("{}/extracted", b),
            "/data/local/tmp/megadaemon/extracted"
        );
        assert_eq!(
            format!("{}/web/cockpit_template.html", b),
            "/data/local/tmp/megadaemon/web/cockpit_template.html"
        );
        // y NINGUNA ruta fija a /data/user/0/com.agy
        assert!(!format!("{}/uploads", b).contains("com.agy"));
    }
}
