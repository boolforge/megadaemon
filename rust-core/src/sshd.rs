//! Servidor SSH nativo de MegaDaemon (v13.4).
//!
//! Objetivo: poder administrar el demonio desde cualquier sitio (el "harness"
//! remoto) sin depender de herramientas externas: **todo en Rust**, con `russh`
//! como motor SSH y los módulos ya existentes (`android`, `sync`, `apk_inventory`,
//! `repo_updates`, ...) como backend.
//!
//! ── Dos capas bien separadas ───────────────────────────────────────────────
//!
//! 1. **Intérprete de comandos** (`dispatch`) — Rust puro, siempre disponible,
//!    sin red y testeable.  Es la gramática que se expone al cliente SSH y
//!    también sirve como API interna para el resto del daemon.
//! 2. **Servidor de red** (`run`) — sólo con la feature `ssh`.  Acepta
//!    conexiones, autentica (publickey y/o password) y canaliza cada línea
//!    recibida hacia `dispatch`.
//!
//! El shell que se ejecuta NO es un shell del sistema: es este intérprete.
//! No se ejecuta nada que no esté en la gramática, salvo el comando `exec`,
//! que es explícito, auditable y pasa por la política destructiva de
//! `android::root_exec_tool`.
//!
//! ── Gramática de comandos (v1) ─────────────────────────────────────────────
//!
//! ```text
//! help                                 esta ayuda
//! status | health                      estado del demonio y del servidor SSH
//! metrics                              contadores + almacén de sincronización
//! sync [all|claude|deepseek|kimi|...]  sincroniza los extractores
//! search <texto>                       búsqueda full-text en el almacén
//! list [servicio]                      lista conversaciones
//! overview                             resumen del dispositivo (root)
//! exec [--confirm] <comando>           ejecuta una orden como root
//! apk [list|analyze <ruta>]            inventario de APK
//! repos [status|plan]                  estado / plan de actualización
//! tasks [list|stats]                   tareas programadas
//! alerts [list|status]                 alertas
//! quit                                 cierra la sesión
//! ```
//!
//! Ejemplos:
//!
//! ```text
//! sync claude
//! search "conversación con espacio"
//! exec ls -la /data
//! ```

use serde_json::{json, Value};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

// ========================================================================= //
// Constantes
// ========================================================================= //

/// Máximo de conexiones simultáneas al servidor SSH.
pub const MAX_SESSIONS: usize = 8;
/// Tiempo máximo de inactividad de una sesión (30 minutos).
pub const IDLE_TIMEOUT_SECS: u64 = 30 * 60;
/// Puerto por defecto del servidor SSH.
pub const DEFAULT_PORT: u16 = 2222;
/// Nombre del fichero de clave de host (Ed25519).
pub const HOST_KEY_FILE: &str = "ssh_host_ed25519";
/// Nombre del fichero de claves autorizadas.
pub const AUTHORIZED_KEYS_FILE: &str = "authorized_keys";
/// Nombre del fichero de auditoría (JSON Lines).
pub const AUDIT_FILE: &str = "ssh_audit.ndjson";
/// Variable de entorno con la contraseña de acceso (opcional).
pub const PASSWORD_ENV: &str = "MEGADAEMON_SSH_PASSWORD";
/// Cuántas conexiones recientes recuerda `status()`.
const LAST_CONNECTIONS: usize = 16;
/// Timeout por defecto de `exec` (ms).
const EXEC_TIMEOUT_MS: u64 = 30_000;
/// Tope de salida de `exec` (caracteres).
const EXEC_MAX_CHARS: usize = 64 * 1024;

// ========================================================================= //
// Estado global del servidor
// ========================================================================= //

/// Estado vivo del servidor SSH (contadores y conexiones recientes).
struct Runtime {
    /// Instante en el que arrancó el servidor (`None` si nunca arrancó).
    started_at: Mutex<Option<Instant>>,
    /// Puerto real de escucha.
    port: Mutex<u16>,
    /// ¿Estamos escuchando ahora mismo?
    listening: AtomicBool,
    /// Conexiones activas.
    sessions: AtomicU32,
    /// Conexiones aceptadas desde el arranque.
    connections_total: AtomicU64,
    /// Comandos ejecutados desde el arranque.
    commands_total: AtomicU64,
    /// Últimas conexiones (para `status()`).
    last_connections: Mutex<VecDeque<Value>>,
}

fn runtime() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| Runtime {
        started_at: Mutex::new(None),
        port: Mutex::new(DEFAULT_PORT),
        listening: AtomicBool::new(false),
        sessions: AtomicU32::new(0),
        connections_total: AtomicU64::new(0),
        commands_total: AtomicU64::new(0),
        last_connections: Mutex::new(VecDeque::new()),
    })
}

/// Segundos transcurridos desde UNIX epoch.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ========================================================================= //
// Rutas de estado
// ========================================================================= //

/// Directorio de estado de megadaemon.
fn state_dir() -> String {
    crate::cockpit_server::state_dir()
}

/// Ruta del fichero de clave de host.
pub fn host_key_path() -> PathBuf {
    PathBuf::from(state_dir()).join(HOST_KEY_FILE)
}

/// Ruta del fichero de claves autorizadas.
pub fn authorized_keys_path() -> PathBuf {
    PathBuf::from(state_dir()).join(AUTHORIZED_KEYS_FILE)
}

/// Ruta del fichero de auditoría.
pub fn audit_path() -> PathBuf {
    PathBuf::from(state_dir()).join(AUDIT_FILE)
}

// ========================================================================= //
// base64 + huellas (sin dependencias externas)
// ========================================================================= //

const B64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Codifica en base64 estándar (con relleno `=`).
fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64_ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(B64_ALPHABET[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(B64_ALPHABET[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(B64_ALPHABET[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Decodifica base64 estándar (ignora espacios y relleno).
fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    let mut out: Vec<u8> = Vec::with_capacity(s.len() * 3 / 4);
    for c in s.chars() {
        if c == '=' {
            break;
        }
        if c.is_whitespace() {
            continue;
        }
        let v: u32 = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            _ => return None,
        };
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xff) as u8);
            acc &= if bits == 0 { 0 } else { (1u32 << bits) - 1 };
        }
    }
    Some(out)
}

/// Huella SHA256 estilo OpenSSH (`SHA256:base64`) sobre el *blob* público.
///
/// OpenSSH calcula el SHA256 sobre el blob completo de la clave pública tal y
/// como aparece en el fichero `authorized_keys` (tipo + datos, en base64), sin
/// el prefijo de longitud que usa el protocolo en los mensajes.
fn fingerprint_blob(blob: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(blob);
    format!("SHA256:{}", b64_encode(&digest).trim_end_matches('='))
}

// ========================================================================= //
// Claves autorizadas
// ========================================================================= //

/// Una clave autorizada, ya analizada.
struct AuthorizedKey {
    /// Tipo de clave (`ssh-ed25519`, `ssh-rsa`, ...).
    key_type: String,
    /// Huella SHA256 estilo OpenSSH.
    fingerprint: String,
    /// Comentario de la línea (opcional).
    comment: String,
}

/// ¿El token parece el *tipo* de clave SSH?
fn is_key_type(token: &str) -> bool {
    token.starts_with("ssh-")
        || token.starts_with("ecdsa-")
        || token.starts_with("sk-")
        || token.starts_with("rsa-sha2-")
        || token.starts_with("webauthn-")
        || token.starts_with("x509v3-")
}

/// Divide una línea en tokens respetando comillas simples y dobles.
fn split_quoted(line: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut pending = false;
    for ch in line.chars() {
        match quote {
            Some(q) => {
                if ch == q {
                    quote = None;
                } else {
                    cur.push(ch);
                }
            }
            None => {
                if ch == '\'' || ch == '"' {
                    quote = Some(ch);
                    pending = true;
                } else if ch.is_whitespace() {
                    if pending {
                        tokens.push(std::mem::take(&mut cur));
                        pending = false;
                    }
                } else {
                    cur.push(ch);
                    pending = true;
                }
            }
        }
    }
    if pending {
        tokens.push(cur);
    }
    tokens
}

/// Analiza un fichero `authorized_keys` en formato OpenSSH.
fn parse_authorized_keys(text: &str) -> Vec<AuthorizedKey> {
    let mut out: Vec<AuthorizedKey> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Las directivas (@cert-authority, @revoked) no son claves de acceso.
        if line.starts_with('@') {
            continue;
        }
        let tokens = split_quoted(line);
        let Some(idx) = tokens.iter().position(|t| is_key_type(t)) else {
            continue;
        };
        let key_type = tokens[idx].clone();
        let Some(encoded) = tokens.get(idx + 1) else {
            continue;
        };
        let Some(blob) = b64_decode(encoded) else {
            continue;
        };
        if blob.is_empty() {
            continue;
        }
        let comment = tokens[idx + 2..].join(" ");
        out.push(AuthorizedKey {
            key_type,
            fingerprint: fingerprint_blob(&blob),
            comment,
        });
    }
    out
}

/// Lee y analiza las claves autorizadas del estado de megadaemon.
fn load_authorized_keys() -> Vec<AuthorizedKey> {
    let path = authorized_keys_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    parse_authorized_keys(&text)
}

// ========================================================================= //
// Auditoría
// ========================================================================= //

/// Registra un evento de auditoría en `<state_dir>/ssh_audit.ndjson`.
///
/// Cada línea lleva marca de tiempo, peer y (cuando aplica) el comando.
pub fn audit_event(event: &str, peer: &str, extra: Value) {
    let dir = state_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = audit_path();
    let mut line = json!({
        "ts": now_secs(),
        "event": event,
        "peer": peer,
    });
    if let (Some(obj), Some(details)) = (line.as_object_mut(), extra.as_object()) {
        for (k, v) in details {
            obj.insert(k.clone(), v.clone());
        }
    }
    let Ok(text) = serde_json::to_string(&line) else {
        return;
    };
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = f.write_all(format!("{}\n", text).as_bytes());
    }
}

/// Registra la ejecución de un comando (con su peer y su resultado).
pub fn audit_command(peer: &str, user: &str, command: &str, ok: bool) {
    runtime().commands_total.fetch_add(1, Ordering::Relaxed);
    audit_event(
        "command",
        peer,
        json!({ "user": user, "command": command, "ok": ok }),
    );
}

/// Recuerda una conexión en el historial de `status()`.
fn remember_connection(record: Value) {
    let rt = runtime();
    if let Ok(mut last) = rt.last_connections.lock() {
        last.push_back(record);
        while last.len() > LAST_CONNECTIONS {
            last.pop_front();
        }
    }
}

// ========================================================================= //
// API pública: estado y preparación del acceso
// ========================================================================= //

/// Métodos de autenticación habilitados en este momento.
pub fn enabled_methods() -> Vec<&'static str> {
    let mut methods = vec!["publickey"];
    if password_configured() {
        methods.push("password");
    }
    methods
}

/// ¿Hay contraseña configurada en el entorno?
fn password_configured() -> bool {
    match std::env::var(PASSWORD_ENV) {
        Ok(p) => !p.trim().is_empty(),
        Err(_) => false,
    }
}

/// Estado del servidor SSH y de la configuración de acceso.
///
/// Siempre disponible (con o sin feature `ssh`).
pub fn status() -> Value {
    let rt = runtime();
    let ak = authorized_keys_path();
    let hk = host_key_path();
    let started = rt.started_at.lock().ok().and_then(|g| *g);
    let uptime = started.map(|t| t.elapsed().as_secs()).unwrap_or(0);
    let port = rt.port.lock().map(|p| *p).unwrap_or(DEFAULT_PORT);
    let last = rt
        .last_connections
        .lock()
        .map(|g| g.iter().cloned().collect::<Vec<Value>>())
        .unwrap_or_default();
    json!({
        "ok": true,
        "feature_enabled": cfg!(feature = "ssh"),
        "port": port,
        "listening": rt.listening.load(Ordering::Relaxed),
        "authorized_keys_present": ak.exists(),
        "host_key_present": hk.exists(),
        "methods": enabled_methods(),
        "sessions": rt.sessions.load(Ordering::Relaxed),
        "uptime": uptime,
        "last_connections": last,
    })
}

/// Instala (o regenera) la clave de host y devuelve su huella pública.
///
/// La huella sirve para que el usuario la compare con la que muestra el
/// cliente SSH (`ssh-keygen -lf`), de forma que detecte un hombre en medio.
/// Nunca se imprime la clave privada.
pub fn install_host_key() -> Value {
    let path = host_key_path();
    let existed = path.exists();
    #[cfg(feature = "ssh")]
    {
        match ssh_impl::load_or_create_host_key() {
            Ok(key) => {
                let fingerprint = ssh_impl::host_key_fingerprint(&key);
                let public = ssh_impl::host_key_public_line(&key);
                json!({
                    "ok": true,
                    "feature_enabled": true,
                    "created": !existed,
                    "path": path.to_string_lossy(),
                    "public_key": public,
                    "fingerprint": fingerprint,
                    "hint": "verifica la huella con: ssh-keygen -lf <(ssh-keygen -y -f <host_key>)",
                })
            }
            Err(e) => json!({
                "ok": false,
                "feature_enabled": true,
                "path": path.to_string_lossy(),
                "error": e,
            }),
        }
    }
    #[cfg(not(feature = "ssh"))]
    {
        json!({
            "ok": false,
            "feature_enabled": false,
            "created": false,
            "path": path.to_string_lossy(),
            "host_key_present": existed,
            "error": "ssh feature no compilada: recompila con --features ssh para generar la clave de host",
        })
    }
}

/// Lista las claves autorizadas con su huella (para que el usuario verifique).
pub fn authorized_keys_status() -> Value {
    let path = authorized_keys_path();
    let present = path.exists();
    let keys = load_authorized_keys();
    let items: Vec<Value> = keys
        .iter()
        .map(|k| {
            json!({
                "type": k.key_type,
                "fingerprint": k.fingerprint,
                "comment": k.comment,
            })
        })
        .collect();
    json!({
        "ok": true,
        "path": path.to_string_lossy(),
        "present": present,
        "count": keys.len(),
        "keys": items,
        "methods": enabled_methods(),
        "hint": if keys.is_empty() {
            "copia tu clave pública (ssh-ed25519 AAAA... usuario@host) en este fichero"
        } else {
            "las huellas deben coincidir con las de tu cliente SSH"
        },
    })
}

// ========================================================================= //
// Intérprete de comandos (Rust puro, sin red)
// ========================================================================= //

/// Tokeniza una línea de comandos respetando comillas simples y dobles.
///
/// Ejemplo: `search "palabra con espacio"` → `["search", "palabra con espacio"]`.
pub fn tokenize(line: &str) -> Vec<String> {
    split_quoted(line)
}

/// ¿La línea pide cerrar la sesión?
pub fn is_quit(line: &str) -> bool {
    let tokens = tokenize(line);
    match tokens.first() {
        Some(t) => {
            let t = t.to_ascii_lowercase();
            t == "quit" || t == "exit"
        }
        None => false,
    }
}

/// Serializa un `Value` como JSON legible (con fallback a `to_string`).
fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

/// Ejecuta un comando de la gramática y devuelve la respuesta en texto plano.
///
/// Es la **única** puerta de entrada del shell SSH: cualquier cosa que no
/// esté en la gramática se responde con "desconocido" y no ejecuta nada.
pub fn dispatch(line: &str) -> String {
    let tokens = tokenize(line);
    if tokens.is_empty() {
        return String::new();
    }
    let command = tokens[0].to_ascii_lowercase();
    let args: Vec<String> = tokens[1..].to_vec();
    match command.as_str() {
        "help" | "?" | "h" => help_text(),
        "status" | "health" => pretty(&status()),
        "metrics" => pretty(&metrics()),
        "sync" => cmd_sync(&args),
        "search" => cmd_search(&args),
        "list" | "ls" => cmd_list(&args),
        "overview" => cmd_overview(),
        "exec" => cmd_exec(&args),
        "apk" => cmd_apk(&args),
        "repos" => cmd_repos(&args),
        "tasks" => cmd_tasks(&args),
        "alerts" => cmd_alerts(&args),
        "quit" | "exit" => "Hasta luego.\n".to_string(),
        other => format!(
            "error: comando desconocido: '{}'. Escribe 'help' para ver la lista.",
            other
        ),
    }
}

/// Texto de ayuda.
fn help_text() -> String {
    let services = crate::sync::SERVICES.join(", ");
    format!(
        "MegaDaemon SSH · intérprete de comandos v13.4\n\
         \n\
         Comandos:\n\
         \x20 help                            esta ayuda\n\
         \x20 status | health                 estado del demonio y del servidor SSH\n\
         \x20 metrics                         contadores y almacén de sincronización\n\
         \x20 sync [all|<servicio>...]        sincroniza los extractores\n\
         \x20 search <texto>                  busca en el almacén de conversaciones\n\
         \x20 list [servicio]                 lista conversaciones (opcional: un servicio)\n\
         \x20 overview                        resumen del dispositivo (root)\n\
         \x20 exec [--confirm] <comando>      ejecuta una orden como root (auditada)\n\
         \x20 apk [list|analyze <ruta>]       inventario de APK\n\
         \x20 repos [status|plan]             estado / plan de actualización de repos\n\
         \x20 tasks [list|stats]              tareas programadas\n\
         \x20 alerts [list|status]            alertas activas\n\
         \x20 quit                            cierra la sesión\n\
         \n\
         Servicios soportados por sync: {}\n\
         \n\
         Ejemplos:\n\
         \x20 sync claude\n\
         \x20 search \"conversación con espacio\"\n\
         \x20 exec ls -la /data\n",
        services
    )
}

/// Métricas agregadas del daemon.
fn metrics() -> Value {
    let rt = runtime();
    let started = rt.started_at.lock().ok().and_then(|g| *g);
    let uptime = started.map(|t| t.elapsed().as_secs()).unwrap_or(0);
    let sync = crate::sync::status();
    json!({
        "ok": true,
        "uptime": uptime,
        "sessions": rt.sessions.load(Ordering::Relaxed),
        "connections_total": rt.connections_total.load(Ordering::Relaxed),
        "commands_total": rt.commands_total.load(Ordering::Relaxed),
        "sync": {
            "total_conversations": sync.get("total_conversations").cloned().unwrap_or(Value::Null),
            "total_messages": sync.get("total_messages").cloned().unwrap_or(Value::Null),
            "total_chars": sync.get("total_chars").cloned().unwrap_or(Value::Null),
            "services": sync.get("services").cloned().unwrap_or(Value::Null),
        },
    })
}

/// `sync [all|<servicio>...]`
fn cmd_sync(args: &[String]) -> String {
    let wanted: Vec<String> = args
        .iter()
        .map(|a| a.to_ascii_lowercase())
        .filter(|a| !a.starts_with('-'))
        .collect();
    if wanted.is_empty() || wanted.iter().any(|w| w == "all") {
        return pretty(&crate::sync::run_sync(None, false));
    }
    let selected: Vec<String> = wanted
        .iter()
        .filter(|s| crate::sync::SERVICES.contains(&s.as_str()))
        .cloned()
        .collect();
    if selected.is_empty() {
        return pretty(&json!({
            "ok": false,
            "error": "servicio_desconocido",
            "hint": "usa 'sync all' o un servicio soportado",
            "supported": crate::sync::SERVICES,
        }));
    }
    pretty(&crate::sync::run_sync(Some(selected), false))
}

/// `search <texto>`
fn cmd_search(args: &[String]) -> String {
    let query = args.join(" ");
    if query.trim().is_empty() {
        return pretty(&json!({
            "ok": false,
            "error": "consulta_vacia",
            "hint": "uso: search <texto>",
        }));
    }
    pretty(&crate::sync::search(&query, None, 20))
}

/// `list [servicio]`
fn cmd_list(args: &[String]) -> String {
    let service = args.first().map(|s| s.to_ascii_lowercase());
    if let Some(svc) = service.as_ref() {
        if !crate::sync::SERVICES.contains(&svc.as_str()) {
            return pretty(&json!({
                "ok": false,
                "error": "servicio_desconocido",
                "hint": "usa 'list' o un servicio soportado",
                "supported": crate::sync::SERVICES,
            }));
        }
    }
    pretty(&crate::sync::list_conversations(service.as_deref(), 50))
}

/// `overview` — resumen del dispositivo.
fn cmd_overview() -> String {
    pretty(&crate::android::system_overview())
}

/// `exec [--confirm] <comando>` — ejecución root explícita y auditable.
fn cmd_exec(args: &[String]) -> String {
    let mut confirm = false;
    let mut rest: &[String] = args;
    if let Some(first) = args.first() {
        if first == "--confirm" || first == "-y" {
            confirm = true;
            rest = &args[1..];
        }
    }
    let command = rest.join(" ");
    if command.trim().is_empty() {
        return pretty(&json!({
            "ok": false,
            "error": "comando_vacio",
            "hint": "uso: exec [--confirm] <comando>",
        }));
    }
    let res = crate::android::root_exec_tool(&command, EXEC_TIMEOUT_MS, EXEC_MAX_CHARS, confirm);
    let exit = res
        .pointer("/result/exit_code")
        .cloned()
        .unwrap_or(Value::Null);
    pretty(&json!({
        "ok": res.get("ok").cloned().unwrap_or(Value::Bool(false)),
        "command": command,
        "exit": exit,
        "exit_code": exit,
        "error": res.get("error").cloned().unwrap_or(Value::Null),
        "hint": res.get("hint").cloned().unwrap_or(Value::Null),
        "result": res.get("result").cloned().unwrap_or(Value::Null),
    }))
}

/// `apk [list|analyze <ruta>]`
fn cmd_apk(args: &[String]) -> String {
    let sub = args
        .first()
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_else(|| "list".to_string());
    let path = args.get(1).cloned().unwrap_or_default();
    match sub.as_str() {
        "list" | "" => {
            let inventory = crate::apk_inventory::inventory(50);
            pretty(&json!({
                "ok": inventory.get("ok").cloned().unwrap_or(Value::Bool(false)),
                "subcommand": "list",
                "inventory": inventory,
            }))
        }
        "analyze" => {
            if path.trim().is_empty() {
                return pretty(&json!({
                    "ok": false,
                    "error": "ruta_vacia",
                    "hint": "uso: apk analyze <ruta>",
                }));
            }
            let analysis = crate::apk_inventory::analyze_apk(&path);
            pretty(&json!({
                "ok": analysis.get("ok").cloned().unwrap_or(Value::Bool(false)),
                "subcommand": "analyze",
                "path": path,
                "analysis": analysis,
            }))
        }
        other => pretty(&json!({
            "ok": false,
            "error": "subcomando_desconocido",
            "hint": "uso: apk [list|analyze <ruta>]",
            "subcommand": other,
        })),
    }
}

/// `repos [status|plan]`
fn cmd_repos(args: &[String]) -> String {
    let sub = args
        .first()
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_else(|| "status".to_string());
    match sub.as_str() {
        "status" => pretty(&crate::repo_updates::status()),
        "plan" => pretty(&crate::repo_updates::plan(None)),
        other => pretty(&json!({
            "ok": false,
            "error": "subcomando_desconocido",
            "hint": "uso: repos [status|plan]",
            "subcommand": other,
        })),
    }
}

/// `tasks [list|stats]`
fn cmd_tasks(args: &[String]) -> String {
    let sub = args
        .first()
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_else(|| "list".to_string());
    match sub.as_str() {
        "list" | "" => pretty(&crate::task_scheduler::list_json()),
        "stats" => pretty(&crate::task_scheduler::stats()),
        other => pretty(&json!({
            "ok": false,
            "error": "subcomando_desconocido",
            "hint": "uso: tasks [list|stats]",
            "subcommand": other,
        })),
    }
}

/// `alerts [list|status]`
fn cmd_alerts(args: &[String]) -> String {
    let sub = args
        .first()
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_else(|| "list".to_string());
    match sub.as_str() {
        "list" | "" => pretty(&crate::alerts::list(None, false, 50)),
        "status" => pretty(&crate::alerts::status()),
        other => pretty(&json!({
            "ok": false,
            "error": "subcomando_desconocido",
            "hint": "uso: alerts [list|status]",
            "subcommand": other,
        })),
    }
}

/// ¿La respuesta de un comando indica éxito?
#[cfg_attr(not(feature = "ssh"), allow(dead_code))]
fn response_ok(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return true;
    }
    if t.starts_with("error:") {
        return false;
    }
    if let Ok(v) = serde_json::from_str::<Value>(t) {
        if let Some(ok) = v.get("ok").and_then(|b| b.as_bool()) {
            return ok;
        }
    }
    true
}

// ========================================================================= //
// Servidor SSH de red (feature `ssh`)
// ========================================================================= //

#[cfg(feature = "ssh")]
mod ssh_impl {
    use super::*;
    use russh::keys::ssh_key::rand_core::OsRng;
    use russh::keys::ssh_key::{LineEnding, PublicKey};
    use russh::keys::{Algorithm, PrivateKey};
    use russh::server::{Auth, Config, Msg, Server as _, Session};
    use russh::{Channel, ChannelId, CryptoVec};
    use std::net::SocketAddr;
    use std::sync::Arc;
    use tokio::net::TcpListener;
    use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

    /// Banner del shell.
    const BANNER: &str =
        "MegaDaemon SSH v13.4 · intérprete de comandos (escribe 'help' para la lista, 'quit' para salir)\r\n";
    /// Prompt del shell.
    const PROMPT: &str = "megadaemon> ";

    // ───────────────────────────────────────────────────────────────────── //
    // Clave de host
    // ───────────────────────────────────────────────────────────────────── //

    /// Carga la clave de host; si no existe, la genera (Ed25519, 0600).
    pub fn load_or_create_host_key() -> Result<PrivateKey, String> {
        let path = host_key_path();
        if path.exists() {
            return russh::keys::load_secret_key(&path, None)
                .map_err(|e| format!("no se pudo leer la clave de host {:?}: {}", path, e));
        }
        let key = generate_host_key()?;
        save_host_key(&key)?;
        Ok(key)
    }

    /// Genera una clave de host Ed25519 nueva.
    fn generate_host_key() -> Result<PrivateKey, String> {
        let mut rng = OsRng;
        PrivateKey::random(&mut rng, Algorithm::Ed25519)
            .map_err(|e| format!("no se pudo generar la clave de host: {}", e))
    }

    /// Guarda la clave de host en formato OpenSSH con permisos 0600.
    fn save_host_key(key: &PrivateKey) -> Result<(), String> {
        let path = host_key_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {:?}: {}", parent, e))?;
        }
        let pem = key
            .to_openssh(LineEnding::LF)
            .map_err(|e| format!("no se pudo serializar la clave de host: {}", e))?;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts
            .open(&path)
            .map_err(|e| format!("no se pudo escribir {:?}: {}", path, e))?;
        use std::io::Write;
        f.write_all(pem.as_bytes())
            .map_err(|e| format!("no se pudo escribir {:?}: {}", path, e))?;
        // La clave pública se guarda aparte para que el usuario la verifique.
        let pub_line = host_key_public_line(key);
        let _ = std::fs::write(path.with_extension("pub"), pub_line);
        Ok(())
    }

    /// Línea `authorized_keys` de la clave de host.
    pub fn host_key_public_line(key: &PrivateKey) -> String {
        key.public_key()
            .to_openssh()
            .unwrap_or_else(|_| String::new())
    }

    /// Huella OpenSSH de la clave de host.
    ///
    /// Se calcula sobre el blob tal y como aparece en la línea
    /// `authorized_keys`, que es exactamente lo que muestra `ssh-keygen -lf`.
    pub fn host_key_fingerprint(key: &PrivateKey) -> String {
        fingerprint_of_line(&host_key_public_line(key))
    }

    /// Huella OpenSSH de una clave pública cualquiera.
    fn public_fingerprint(key: &PublicKey) -> String {
        match key.to_openssh() {
            Ok(line) => fingerprint_of_line(&line),
            Err(_) => String::new(),
        }
    }

    /// Extrae la huella de una línea `authorized_keys` (`tipo base64 comentario`).
    fn fingerprint_of_line(line: &str) -> String {
        match line.split_whitespace().nth(1) {
            Some(b64) => match b64_decode(b64) {
                Some(blob) => fingerprint_blob(&blob),
                None => String::new(),
            },
            None => String::new(),
        }
    }

    // ───────────────────────────────────────────────────────────────────── //
    // Configuración del servidor
    // ───────────────────────────────────────────────────────────────────── //

    /// Construye la configuración de russh con los métodos habilitados.
    fn build_config(host_key: PrivateKey) -> Config {
        let mut methods = russh::MethodSet::PUBLICKEY;
        if password_configured() {
            methods |= russh::MethodSet::PASSWORD;
        }
        Config {
            server_id: russh::SshId::Standard(format!(
                "SSH-2.0-MegaDaemon_{}",
                env!("CARGO_PKG_VERSION")
            )),
            methods,
            keys: vec![host_key],
            inactivity_timeout: Some(std::time::Duration::from_secs(IDLE_TIMEOUT_SECS)),
            auth_rejection_time: std::time::Duration::from_secs(3),
            auth_rejection_time_initial: Some(std::time::Duration::from_secs(0)),
            ..Default::default()
        }
    }

    // ───────────────────────────────────────────────────────────────────── //
    // Manejador de sesión
    // ───────────────────────────────────────────────────────────────────── //

    /// Estado de una conexión SSH en curso.
    pub struct SshHandler {
        /// Dirección del_peer (ip:puerto).
        peer: String,
        /// Usuario solicitado por el cliente.
        user: String,
        /// Método de autenticación usado.
        auth_method: String,
        /// Buffer de entrada del shell (bytes sin procesar).
        input: Vec<u8>,
        /// Canal de envío de líneas completas al shell.
        shell_tx: Option<UnboundedSender<String>>,
        /// Comandos ejecutados en esta sesión.
        commands: u32,
    }

    impl SshHandler {
        /// Crea un manejador para una conexión entrante.
        pub fn new(peer_addr: Option<SocketAddr>) -> Self {
            SshHandler {
                peer: peer_addr
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "desconocido".to_string()),
                user: String::new(),
                auth_method: String::new(),
                input: Vec::new(),
                shell_tx: None,
                commands: 0,
            }
        }

        /// ¿La clave pública está en las claves autorizadas?
        fn key_authorized(key: &PublicKey) -> bool {
            let fp = public_fingerprint(key);
            if fp.is_empty() {
                return false;
            }
            load_authorized_keys().iter().any(|k| k.fingerprint == fp)
        }
    }

    /// Comparación en tiempo (aproximadamente) constante de contraseñas.
    fn secret_eq(a: &str, b: &str) -> bool {
        let (a, b) = (a.as_bytes(), b.as_bytes());
        if a.len() != b.len() {
            return false;
        }
        let mut diff = 0u8;
        for (x, y) in a.iter().zip(b.iter()) {
            diff |= x ^ y;
        }
        diff == 0
    }

    /// implementación de `russh::server::Handler`.
    ///
    /// **Nota**: `russh` usa `#[async_trait]`, pero este proyecto no puede
    /// añadir `async-trait` a `Cargo.toml` (dependencias fijadas), así que las
    /// firmas se escriben ya "desazucaradas", tal y como las genera
    /// `async_trait` (`Pin<Box<dyn Future + Send + 'async_trait>>`).
    impl russh::server::Handler for SshHandler {
        type Error = russh::Error;

        fn auth_publickey<'life0, 'life1, 'life2, 'async_trait>(
            &'life0 mut self,
            user: &'life1 str,
            public_key: &'life2 PublicKey,
        ) -> ::core::pin::Pin<
            Box<
                dyn ::core::future::Future<Output = Result<Auth, Self::Error>>
                    + ::core::marker::Send
                    + 'async_trait,
            >,
        >
        where
            'life0: 'async_trait,
            'life1: 'async_trait,
            'life2: 'async_trait,
            Self: 'async_trait,
        {
            Box::pin(async move {
                if SshHandler::key_authorized(public_key) {
                    self.user = user.to_string();
                    self.auth_method = "publickey".to_string();
                    Ok(Auth::Accept)
                } else {
                    audit_event(
                        "auth_failed",
                        &self.peer,
                        json!({ "user": user, "method": "publickey" }),
                    );
                    Ok(Auth::Reject {
                        proceed_with_methods: None,
                    })
                }
            })
        }

        fn auth_password<'life0, 'life1, 'life2, 'async_trait>(
            &'life0 mut self,
            user: &'life1 str,
            password: &'life2 str,
        ) -> ::core::pin::Pin<
            Box<
                dyn ::core::future::Future<Output = Result<Auth, Self::Error>>
                    + ::core::marker::Send
                    + 'async_trait,
            >,
        >
        where
            'life0: 'async_trait,
            'life1: 'async_trait,
            'life2: 'async_trait,
            Self: 'async_trait,
        {
            Box::pin(async move {
                let expected = std::env::var(PASSWORD_ENV).unwrap_or_default();
                if !expected.is_empty() && secret_eq(&expected, password) {
                    self.user = user.to_string();
                    self.auth_method = "password".to_string();
                    Ok(Auth::Accept)
                } else {
                    audit_event(
                        "auth_failed",
                        &self.peer,
                        json!({ "user": user, "method": "password" }),
                    );
                    Ok(Auth::Reject {
                        proceed_with_methods: None,
                    })
                }
            })
        }

        fn auth_succeeded<'life0, 'life1, 'async_trait>(
            &'life0 mut self,
            _session: &'life1 mut Session,
        ) -> ::core::pin::Pin<
            Box<
                dyn ::core::future::Future<Output = Result<(), Self::Error>>
                    + ::core::marker::Send
                    + 'async_trait,
            >,
        >
        where
            'life0: 'async_trait,
            'life1: 'async_trait,
            Self: 'async_trait,
        {
            Box::pin(async move {
                audit_event(
                    "connect",
                    &self.peer,
                    json!({ "user": self.user, "method": self.auth_method }),
                );
                remember_connection(json!({
                    "ts": now_secs(),
                    "peer": self.peer,
                    "user": self.user,
                    "method": self.auth_method,
                }));
                Ok(())
            })
        }

        fn channel_open_session<'life0, 'life1, 'async_trait>(
            &'life0 mut self,
            _channel: Channel<Msg>,
            _session: &'life1 mut Session,
        ) -> ::core::pin::Pin<
            Box<
                dyn ::core::future::Future<Output = Result<bool, Self::Error>>
                    + ::core::marker::Send
                    + 'async_trait,
            >,
        >
        where
            'life0: 'async_trait,
            'life1: 'async_trait,
            Self: 'async_trait,
        {
            Box::pin(async move { Ok(true) })
        }

        fn shell_request<'life0, 'life1, 'async_trait>(
            &'life0 mut self,
            channel: ChannelId,
            session: &'life1 mut Session,
        ) -> ::core::pin::Pin<
            Box<
                dyn ::core::future::Future<Output = Result<(), Self::Error>>
                    + ::core::marker::Send
                    + 'async_trait,
            >,
        >
        where
            'life0: 'async_trait,
            'life1: 'async_trait,
            Self: 'async_trait,
        {
            Box::pin(async move {
                session.channel_success(channel)?;
                let handle = session.handle();
                let (tx, mut rx) = unbounded_channel::<String>();
                self.shell_tx = Some(tx);
                let peer = self.peer.clone();
                let user = self.user.clone();
                tokio::spawn(async move {
                    let _ = handle.data(channel, CryptoVec::from(BANNER)).await;
                    let _ = handle.data(channel, CryptoVec::from(PROMPT)).await;
                    while let Some(line) = rx.recv().await {
                        let quit = is_quit(&line);
                        let mut out = dispatch(&line);
                        if !out.is_empty() && !out.ends_with('\n') {
                            out.push('\n');
                        }
                        let ok = response_ok(&out);
                        if quit {
                            out.push_str("Hasta luego.\n");
                        } else {
                            out.push_str(PROMPT);
                        }
                        let _ = handle.data(channel, CryptoVec::from(out)).await;
                        audit_command(&peer, &user, &line, ok);
                        if quit {
                            let _ = handle.exit_status_request(channel, 0).await;
                            let _ = handle.eof(channel).await;
                            let _ = handle.close(channel).await;
                            break;
                        }
                    }
                });
                Ok(())
            })
        }

        fn exec_request<'life0, 'life1, 'life2, 'async_trait>(
            &'life0 mut self,
            channel: ChannelId,
            data: &'life1 [u8],
            session: &'life2 mut Session,
        ) -> ::core::pin::Pin<
            Box<
                dyn ::core::future::Future<Output = Result<(), Self::Error>>
                    + ::core::marker::Send
                    + 'async_trait,
            >,
        >
        where
            'life0: 'async_trait,
            'life1: 'async_trait,
            'life2: 'async_trait,
            Self: 'async_trait,
        {
            Box::pin(async move {
                let command = String::from_utf8_lossy(data).trim().to_string();
                let out = dispatch(&command);
                let ok = response_ok(&out);
                self.commands += 1;
                session.channel_success(channel)?;
                let handle = session.handle();
                let mut payload = out;
                if !payload.is_empty() && !payload.ends_with('\n') {
                    payload.push('\n');
                }
                let _ = handle.data(channel, CryptoVec::from(payload)).await;
                let _ = handle
                    .exit_status_request(channel, if ok { 0 } else { 1 })
                    .await;
                let _ = handle.eof(channel).await;
                let _ = handle.close(channel).await;
                audit_command(&self.peer, &self.user, &command, ok);
                Ok(())
            })
        }

        fn data<'life0, 'life1, 'life2, 'async_trait>(
            &'life0 mut self,
            _channel: ChannelId,
            data: &'life1 [u8],
            _session: &'life2 mut Session,
        ) -> ::core::pin::Pin<
            Box<
                dyn ::core::future::Future<Output = Result<(), Self::Error>>
                    + ::core::marker::Send
                    + 'async_trait,
            >,
        >
        where
            'life0: 'async_trait,
            'life1: 'async_trait,
            'life2: 'async_trait,
            Self: 'async_trait,
        {
            Box::pin(async move {
                if let Some(tx) = self.shell_tx.clone() {
                    self.input.extend_from_slice(data);
                    while let Some(pos) = self
                        .input
                        .iter()
                        .position(|b| *b == b'\n' || *b == b'\r')
                    {
                        let raw: Vec<u8> = self.input.drain(..=pos).collect();
                        let line = String::from_utf8_lossy(&raw)
                            .trim_end_matches(['\r', '\n'])
                            .to_string();
                        let _ = tx.send(line);
                    }
                }
                Ok(())
            })
        }
    }

    /// Servidor russh: crea un manejador por conexión.
    pub struct SshDaemon;

    impl russh::server::Server for SshDaemon {
        type Handler = SshHandler;

        fn new_client(&mut self, peer_addr: Option<SocketAddr>) -> Self::Handler {
            SshHandler::new(peer_addr)
        }
    }

    /// Arranca el servidor SSH.
    ///
    /// Escucha en `bind:port`, autentica con las claves autorizadas (y con la
    /// contraseña del entorno si existe) y sirve el intérprete de comandos.
    pub async fn run(port: u16, bind: &str) -> Result<(), String> {
        let host_key = load_or_create_host_key()?;
        let config = Arc::new(build_config(host_key));

        let listener = TcpListener::bind((bind, port))
            .await
            .map_err(|e| format!("no se pudo escuchar en {}:{}: {}", bind, port, e))?;
        let local = listener
            .local_addr()
            .map_err(|e| format!("no se pudo leer la dirección local: {}", e))?;

        let rt = runtime();
        if let Ok(mut p) = rt.port.lock() {
            *p = local.port();
        }
        if let Ok(mut started) = rt.started_at.lock() {
            *started = Some(Instant::now());
        }
        rt.listening.store(true, Ordering::Relaxed);
        audit_event(
            "listen",
            &local.to_string(),
            json!({ "port": local.port(), "methods": enabled_methods() }),
        );

        let mut daemon = SshDaemon;
        loop {
            let accepted = listener.accept().await;
            let (socket, peer) = match accepted {
                Ok(v) => v,
                Err(e) => {
                    audit_event(
                        "accept_error",
                        "",
                        json!({ "error": e.to_string() }),
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    continue;
                }
            };

            let active = rt.sessions.load(Ordering::Relaxed) as usize;
            if active >= MAX_SESSIONS {
                audit_event(
                    "rejected",
                    &peer.to_string(),
                    json!({ "reason": "max_sessions", "limit": MAX_SESSIONS }),
                );
                drop(socket);
                continue;
            }

            rt.sessions.fetch_add(1, Ordering::Relaxed);
            rt.connections_total.fetch_add(1, Ordering::Relaxed);
            let config = config.clone();
            let handler = daemon.new_client(Some(peer));
            tokio::spawn(async move {
                if let Ok(session) = russh::server::run_stream(config, socket, handler).await {
                    let _ = session.await;
                }
                rt.sessions.fetch_sub(1, Ordering::Relaxed);
            });
        }
    }
}

// ========================================================================= //
// API pública: arranque del servidor
// ========================================================================= //

/// Arranca el servidor SSH nativo (requiere la feature `ssh`).
#[cfg(feature = "ssh")]
pub async fn run(port: u16, bind: &str) -> Result<(), String> {
    ssh_impl::run(port, bind).await
}

/// Sin la feature `ssh` no hay servidor de red: se informa con claridad.
#[cfg(not(feature = "ssh"))]
pub async fn run(_port: u16, _bind: &str) -> Result<(), String> {
    Err("ssh feature no compilada: recompila con --features ssh para habilitar el servidor SSH".to_string())
}

/// Versión **síncrona** de [`run`], para el binario (`megadaemon-core sshd <puerto>`).
///
/// `main.rs` la invoca desde un `main()` síncrono, así que aquí se levanta un
/// runtime de tokio propio y se bloquea hasta que el servidor termina.
#[cfg(feature = "ssh")]
pub fn run_sync(port: u16, bind: &str) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("no se pudo crear el runtime de tokio: {}", e))?;
    rt.block_on(run(port, bind))
}

/// Sin la feature `ssh` no hay servidor de red: se informa con claridad.
#[cfg(not(feature = "ssh"))]
pub fn run_sync(_port: u16, _bind: &str) -> Result<(), String> {
    Err("ssh feature no compilada: recompila con --features ssh para habilitar el servidor SSH".to_string())
}

// ========================================================================= //
// Tests (sin red)
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_devuelve_la_ayuda() {
        let out = dispatch("help");
        assert!(out.contains("help"), "la ayuda debe mencionar 'help': {}", out);
        assert!(out.contains("exec"), "la ayuda debe mencionar 'exec': {}", out);
    }

    #[test]
    fn status_devuelve_json_valido_con_ok() {
        let out = dispatch("status");
        let v: Value = serde_json::from_str(&out).expect("status debe devolver JSON válido");
        assert_eq!(v.get("ok").and_then(|b| b.as_bool()), Some(true));
        assert!(v.get("feature_enabled").is_some());
        assert!(v.get("methods").is_some());
    }

    #[test]
    fn health_es_alias_de_status() {
        let v: Value = serde_json::from_str(&dispatch("health")).unwrap();
        assert_eq!(v.get("ok").and_then(|b| b.as_bool()), Some(true));
    }

    #[test]
    fn exec_devuelve_el_codigo_de_salida() {
        let out = dispatch("exec ls");
        assert!(out.contains("exit"), "exec debe informar del exit: {}", out);
        let v: Value = serde_json::from_str(&out).expect("exec debe devolver JSON válido");
        assert!(v.get("exit").is_some());
        assert!(v.get("command").is_some());
    }

    #[test]
    fn comando_desconocido_no_ejecuta_nada() {
        let out = dispatch("esto_no_existe");
        assert!(
            out.contains("desconocido"),
            "un comando desconocido debe avisar: {}",
            out
        );
        // No debe parecer un JSON de ejecución real.
        assert!(!out.contains("\"exit\""), "no debe ejecutar nada: {}", out);
    }

    #[test]
    fn comillas_simples_se_tokenizan_bien() {
        let toks = tokenize("search \"palabra con espacio\"");
        assert_eq!(toks.len(), 2);
        assert_eq!(toks[0], "search");
        assert_eq!(toks[1], "palabra con espacio");

        let toks2 = tokenize("search 'palabra con espacio'");
        assert_eq!(toks2, vec!["search", "palabra con espacio"]);
    }

    #[test]
    fn search_conserva_el_texto_con_espacios() {
        let out = dispatch("search \"palabra con espacio\"");
        assert!(
            out.contains("palabra con espacio"),
            "search debe devolver la consulta: {}",
            out
        );
        let v: Value = serde_json::from_str(&out).expect("search debe devolver JSON válido");
        assert_eq!(
            v.get("query").and_then(|q| q.as_str()),
            Some("palabra con espacio")
        );
    }

    #[test]
    fn lista_de_comandos_cubierta() {
        // Cada comando de la gramática debe responder algo (nada de "desconocido").
        // Nota: `overview` y `exec` no se prueban aquí porque lanzan procesos
        // reales como root (el requisito de "exec ls" se cubre aparte).
        for cmd in [
            "help", "status", "health", "metrics", "list", "apk list", "repos status", "tasks list",
            "alerts list", "quit",
        ] {
            let out = dispatch(cmd);
            assert!(
                !out.contains("desconocido"),
                "'{}' debe estar en la gramática: {}",
                cmd,
                out
            );
        }
    }

    #[test]
    fn sync_con_servicio_desconocido_avisa() {
        let out = dispatch("sync no-existe");
        assert!(out.contains("servicio_desconocido"), "{}", out);
    }

    #[test]
    fn exec_sin_comando_avisa() {
        let out = dispatch("exec");
        assert!(out.contains("comando_vacio"), "{}", out);
    }

    #[test]
    fn estado_publico_informa_de_la_feature() {
        let v = status();
        assert_eq!(v.get("ok").and_then(|b| b.as_bool()), Some(true));
        assert_eq!(
            v.get("feature_enabled").and_then(|b| b.as_bool()),
            Some(cfg!(feature = "ssh"))
        );
        assert!(v.get("sessions").is_some());
        assert!(v.get("uptime").is_some());
        assert!(v.get("last_connections").is_some());
    }

    #[test]
    fn huella_open_ssh_estable() {
        // El vector de prueba de OpenSSH: la huella debe ser "SHA256:" + base64.
        let fp = fingerprint_blob(&[1, 2, 3]);
        assert!(fp.starts_with("SHA256:"));
        assert!(!fp.contains('='));
    }

    #[test]
    fn base64_ida_y_vuelve() {
        let data = b"megadaemon";
        let enc = b64_encode(data);
        assert_eq!(b64_decode(&enc).unwrap(), data.to_vec());
        assert!(b64_decode("no-base64!!").is_none());
    }

    #[test]
    fn claves_autorizadas_se_parsean() {
        // Clave pública ed25519 de ejemplo (sólo se usa la forma, no la validity).
        let sample = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIB2xJ8uY1m0Q9kZ1nJ5t0Yk3nJ5t0Yk3nJ5t0Yk3n user@host";
        let keys = parse_authorized_keys(sample);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key_type, "ssh-ed25519");
        assert!(keys[0].fingerprint.starts_with("SHA256:"));
        assert_eq!(keys[0].comment, "user@host");
    }

    #[test]
    fn estado_de_claves_autorizadas_es_json() {
        let v = authorized_keys_status();
        assert_eq!(v.get("ok").and_then(|b| b.as_bool()), Some(true));
        assert!(v.get("keys").is_some());
        assert!(v.get("count").is_some());
    }

    #[test]
    fn install_host_key_informa_segun_la_feature() {
        let v = install_host_key();
        assert!(v.get("ok").is_some());
        assert_eq!(
            v.get("feature_enabled").and_then(|b| b.as_bool()),
            Some(cfg!(feature = "ssh"))
        );
    }

    #[test]
    fn quit_cierra_la_sesion() {
        assert!(is_quit("quit"));
        assert!(is_quit("exit"));
        assert!(!is_quit("status"));
        assert!(!is_quit(""));
    }
}
