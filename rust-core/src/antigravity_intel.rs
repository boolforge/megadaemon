//! W5 — Intel del cliente **Antigravity** (`com.agy`) y puente a MegaDaemon.
//!
//! QUÉ ES ESTE MÓDULO
//! -----------------
//! Antigravity (el IDE/agente de Google que el usuario tiene instalado en su
//! propio móvil, paquete `com.agy`) es un **cliente MCP de pleno derecho**:
//! su binario embuelve el SDK oficial de MCP en Go de Google
//! (`google3/third_party/golang/github.com/modelcontextprotocol/go_sdk`).
//! Su configuración (`~/.gemini/config/mcp_config.json`) tiene un mapa
//! `mcpServers` con los **dos** transportes que documenta su propio SDK:
//! `types.McpStdioServer(name, command, args)` y
//! `types.McpStreamableHttpServer(name, url, headers)`.
//!
//! Este módulo:
//!
//!   1. **Escanea** la instalación real (`.gemini/`) y devuelve un inventario
//!      con tipos: servidores MCP, plugins, skills, conversaciones, hooks del
//!      daemon, credenciales (**sólo estructura**) y hashes.
//!   2. **Detecta los servidores MCP rotos** (apuntan a un binario que no
//!      existe) — el caso real de `megadaemon-minimax`.
//!   3. **Mapea** los modelos/proveedores de Antigravity a los proveedores que
//!      MegaDaemon **ya tiene** (`minimax` → `minimax-m3.1-flash`, …) para no
//!      duplicar nada.
//!   4. **Genera** la entrada `mcpServers` que convierte a Antigravity en
//!      cliente del `/mcp` de este móvil (Streamable HTTP directo, sin binario
//!      duplicado) y la **receta** de cableado en texto.
//!   5. **Proyecta** las tareas propias de Antigravity al formato del tablón
//!      (`state/mesh_board.json`) y al catálogo `service_bus` de W6.
//!
//! REGLAS
//! ------
//! * **Ningún secreto sale del módulo**: de una credencial se guarda el
//!   **nombre del campo**, su **longitud** y el **sha16**; nunca el valor.
//! * Todo es **puro y testeable**: `scan_gemini_root` recibe la raíz, no la
//!   busca; nadie toca la red.
//!
//! MEDIDAS REALES (móvil Samsung del usuario, 2026-11-20)
//! -------------------------------------------------------
//! * `POST http://127.0.0.1:9090/mcp` → **403** sin cabecera, **403** con
//!   sólo `X-API-Key`, **200** con sólo `X-Seere-Auth-Token` ⇒ el token de
//!   sesión es lo que abre el MCP; la API key no basta (queda documentado en
//!   `bridge_recommendation`).
//! * La respuesta es **JSON plano** (`Content-Type: application/json`), **sin**
//!   cabecera `Mcp-Session-Id` ⇒ el servidor es **sin estado**.
//! * `custom_models.json` → `{provider:"minimax", modelId:"MiniMax-M3.1-Flash"}`
//!   ⇒ mapeo directo al proveedor `minimax-m3.1-flash` de `providers.rs`.
//! * `mcp_config.json` → `megadaemon-minimax` apunta a
//!   `.../megadaemon/rust-core/target/release/megadaemon-core`, que **no
//!   existe** ⇒ ese servidor MCP no arranca.

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

/// Raíz del sandbox POSIX de Antigravity en Android (`com.agy`).
pub const AGY_SANDBOX: &str = "/data/user/0/com.agy/files";
/// Raíz de la configuración `.gemini` de Antigravity.
pub const AGY_GEMINI: &str = "/data/user/0/com.agy/files/.gemini";
/// Configuración de servidores MCP (la que lee el cliente).
pub const AGY_MCP_CONFIG: &str = "/data/user/0/com.agy/files/.gemini/config/mcp_config.json";
/// Modelos BYOK del cliente.
pub const AGY_CUSTOM_MODELS: &str =
    "/data/user/0/com.agy/files/.gemini/antigravity/custom_models.json";
/// Plugins + permisos del cliente.
pub const AGY_CONFIG: &str = "/data/user/0/com.agy/files/.gemini/config/config.json";
/// Tareas del daemon de Antigravity.
pub const AGY_TASKS: &str =
    "/data/user/0/com.agy/files/.gemini/antigravity-app/daemon/tasks.json";

/// Transporte stdio (`command` + `args` + `env`).
pub const TRANSPORT_STDIO: &str = "stdio";
/// Transporte Streamable HTTP (`url` + `headers`).
pub const TRANSPORT_HTTP: &str = "http";

/// Cabecera de token de sesión que el daemon acepta en `/mcp`.
pub const AUTH_HEADER: &str = "X-Seere-Auth-Token";
/// Cabecera de API key (acepta GETs y rutas de API, **no** abre `/mcp`).
pub const API_KEY_HEADER: &str = "X-API-Key";

// ---------------------------------------------------------------------------
// Tipos
// ---------------------------------------------------------------------------

/// Un servidor MCP declarado en `mcp_config.json`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct McpServerInfo {
    pub name: String,
    /// `stdio` | `http` | `desconocido`
    pub transport: String,
    pub command: String,
    pub args: Vec<String>,
    /// **Nombres** de las variables de entorno (nunca los valores).
    pub env_names: Vec<String>,
    /// **Nombres** de las cabeceras (nunca los valores).
    pub header_names: Vec<String>,
    pub url: Option<String>,
    /// ¿Existe el binario del `command` en disco?  `false` ⇒ servidor roto.
    pub command_exists: bool,
}

/// Credencial de proveedor: **estructura**, nunca valor.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProviderCredential {
    pub provider: String,
    /// Nombres de todos los campos de la entrada.
    pub fields: Vec<String>,
    /// Longitud total del valor de credencial (no su contenido).
    pub value_len: usize,
    /// `(campo, sha16)` de cada campo sensible.
    pub value_sha16: Vec<(String, String)>,
    /// ¿Hay alguna credencial en esta entrada?
    pub has_credential: bool,
}

/// Un hook/tarea del daemon de Antigravity.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HookInfo {
    pub id: String,
    pub name: String,
    /// `interval` | `cron` | `github` | `file` | `battery` | `webhook` | `manual`
    pub trigger_type: String,
    pub interval_sec: Option<u64>,
    pub enabled: bool,
    /// Comando (previsualizado, recortado) — sin credenciales.
    pub command_preview: String,
    pub command_len: usize,
    pub project: String,
    pub conversation_id: String,
    pub wake_app: bool,
    pub last_run_status: Option<String>,
    pub last_run_duration: Option<u64>,
}

/// Un modelo BYOK configurado en Antigravity.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AgyProvider {
    pub provider: String,
    pub model_id: String,
    pub display_name: String,
    /// ¿El cliente tiene credencial para este proveedor? (nunca el valor)
    pub has_api_key: bool,
}

/// Inventario completo de la instalación de Antigravity (DoD 1).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Inventory {
    pub root: String,
    pub plugins: Vec<String>,
    pub skills: Vec<String>,
    pub conversations: usize,
    pub mcp_config_sha256: Option<String>,
    pub config_sha256: Option<String>,
    pub tasks_sha256: Option<String>,
    pub servers: Vec<McpServerInfo>,
    /// ¿Hay algún servidor stdio cuyo binario exista?  (si no, el cliente
    /// intenta arrancar un MCP que no existe).
    pub stdio_binary_present: bool,
    pub providers: Vec<ProviderCredential>,
    pub hooks: Vec<HookInfo>,
    /// Rutas de ficheros de credenciales (**rutas**, nunca contenido).
    pub credential_paths: Vec<String>,
    /// Modelos BYOK (del `custom_models.json`).
    pub custom_models: Vec<AgyProvider>,
}

// ---------------------------------------------------------------------------
// Utilidades
// ---------------------------------------------------------------------------

/// Redacta un secreto dejando sólo prefijo y sufijo.
pub fn redact_secret(s: &str) -> String {
    let n = s.chars().count();
    if n == 0 {
        return String::new();
    }
    if n <= 12 {
        return format!("<oculto:{}>", n);
    }
    let head: String = s.chars().take(6).collect();
    let tail: String = s.chars().skip(n - 4).collect();
    format!("{}…{} (len={})", head, tail, n)
}

/// sha256 en hexadecimal.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex(&h.finalize())
}

/// Los 16 primeros caracteres de un sha256 (identificador, no reversible).
pub fn sha16(data: &[u8]) -> String {
    sha256_hex(data).chars().take(16).collect()
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn as_str(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn as_bool(v: &Value) -> bool {
    v.as_bool().unwrap_or(false)
}

fn as_u64(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_f64().map(|f| f as u64))
}

/// ¿El nombre del campo parece una credencial?
fn is_secret_field(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("api_key")
        || n.contains("apikey")
        || n.contains("token")
        || n.contains("secret")
        || n.contains("password")
        || n.contains("passwd")
        || n.contains("credential")
        || n == "key"
}

fn file_sha256(path: &Path) -> Option<String> {
    fs::read(path).ok().map(|b| sha256_hex(&b))
}

// ---------------------------------------------------------------------------
// Parseo (puro: sólo `&str` -> estructura)
// ---------------------------------------------------------------------------

/// Parsea `mcp_config.json` → servidores MCP.  JSON inválido ⇒ vector vacío.
pub fn parse_mcp_config(raw: &str) -> Vec<McpServerInfo> {
    let v: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    let servers = match v.get("mcpServers").and_then(|s| s.as_object()) {
        Some(s) => s,
        None => return out,
    };
    for (name, spec) in servers {
        let command = spec.get("command").map(as_str).unwrap_or_default();
        let url = spec.get("url").map(as_str).unwrap_or_default();
        let transport = if !url.is_empty() {
            TRANSPORT_HTTP.to_string()
        } else if !command.is_empty() {
            TRANSPORT_STDIO.to_string()
        } else {
            "desconocido".to_string()
        };
        let args = spec
            .get("args")
            .and_then(|a| a.as_array())
            .map(|a| a.iter().map(as_str).collect())
            .unwrap_or_default();
        let env_names = spec
            .get("env")
            .and_then(|e| e.as_object())
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        let header_names = spec
            .get("headers")
            .and_then(|h| h.as_object())
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        out.push(McpServerInfo {
            name: name.clone(),
            transport,
            command_exists: false, // lo rellena scan_gemini_root (necesita el disco)
            command,
            args,
            env_names,
            header_names,
            url: if url.is_empty() { None } else { Some(url) },
        });
    }
    out
}

/// Parsea credenciales de proveedor de un JSON (`providers` de
/// `mcp_config.json` o de `config.json`).  JSON inválido ⇒ vector vacío.
pub fn parse_provider_credentials(raw: &str) -> Vec<ProviderCredential> {
    let v: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let providers = match v.get("providers").and_then(|p| p.as_object()) {
        Some(p) => p,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    for (id, spec) in providers {
        let obj = match spec.as_object() {
            Some(o) => o,
            None => continue,
        };
        let fields: Vec<String> = obj.keys().cloned().collect();
        let mut value_len = 0usize;
        let mut value_sha16 = Vec::new();
        for (k, val) in obj {
            if !is_secret_field(k) {
                continue;
            }
            let s = as_str(val);
            if s.is_empty() {
                continue;
            }
            value_len = value_len.max(s.chars().count());
            value_sha16.push((k.clone(), sha16(s.as_bytes())));
        }
        let has_credential = !value_sha16.is_empty();
        out.push(ProviderCredential {
            provider: id.clone(),
            fields,
            value_len,
            value_sha16,
            has_credential,
        });
    }
    out
}

/// Parsea `daemon/tasks.json` → hooks/tareas.  JSON inválido ⇒ vector vacío.
pub fn parse_daemon_tasks(raw: &str) -> Vec<HookInfo> {
    let v: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let arr = match v {
        Value::Array(a) => a,
        Value::Object(mut o) => o
            .remove("tasks")
            .and_then(|t| t.as_array().cloned())
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    let mut out = Vec::new();
    for t in arr {
        let trig = t.get("trigger").cloned().unwrap_or(Value::Null);
        let command = t.get("command").map(as_str).unwrap_or_default();
        out.push(HookInfo {
            id: t.get("id").map(as_str).unwrap_or_default(),
            name: t.get("name").map(as_str).unwrap_or_default(),
            trigger_type: trig
                .get("type")
                .map(as_str)
                .unwrap_or_else(|| "manual".to_string()),
            interval_sec: trig.get("interval_sec").and_then(as_u64),
            enabled: t.get("enabled").map(as_bool).unwrap_or(true),
            command_preview: command.chars().take(120).collect::<String>(),
            command_len: command.chars().count(),
            project: t.get("project").map(as_str).unwrap_or_default(),
            conversation_id: t.get("conversation_id").map(as_str).unwrap_or_default(),
            wake_app: t.get("wake_app").map(as_bool).unwrap_or(false),
            last_run_status: t.get("last_run_status").map(as_str),
            last_run_duration: t.get("last_run_duration").and_then(as_u64),
        });
    }
    out
}

/// Parsea `custom_models.json` → modelos BYOK del cliente.
pub fn parse_custom_models(raw: &str) -> Vec<AgyProvider> {
    let v: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let models = match v.get("models") {
        Some(Value::Array(a)) => a.clone(),
        Some(Value::Object(m)) => {
            let mut a = Vec::new();
            for (k, val) in m {
                let mut obj = val.as_object().cloned().unwrap_or_default();
                obj.entry("provider".to_string())
                    .or_insert(Value::String(k.clone()));
                a.push(Value::Object(obj));
            }
            a
        }
        _ => Vec::new(),
    };
    let mut out = Vec::new();
    for m in models {
        let key = m
            .get("apiKey")
            .or_else(|| m.get("api_key"))
            .map(as_str)
            .unwrap_or_default();
        out.push(AgyProvider {
            provider: m.get("provider").map(as_str).unwrap_or_default(),
            model_id: m
                .get("modelId")
                .or_else(|| m.get("model"))
                .map(as_str)
                .unwrap_or_default(),
            display_name: m.get("displayName").map(as_str).unwrap_or_default(),
            has_api_key: !key.is_empty(),
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Escaneo del disco
// ---------------------------------------------------------------------------

fn dir_names(path: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(path) {
        for e in rd.flatten() {
            if e.path().is_dir() {
                out.push(e.file_name().to_string_lossy().to_string());
            }
        }
    }
    out.sort();
    out
}

/// Rutas (relativas) de ficheros que **parecen** credenciales.
fn credential_paths(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(dir: &Path, root: &Path, depth: usize, out: &mut Vec<String>) {
        if depth > 3 {
            return;
        }
        let rd = match fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(_) => return,
        };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if p.is_dir() {
                walk(&p, root, depth + 1, out);
            } else {
                let n = name.to_ascii_lowercase();
                if n.contains("token")
                    || n.contains("oauth")
                    || n.contains("credential")
                    || n.contains("secret")
                    || n.contains("apikey")
                    || n.contains("api_key")
                {
                    out.push(
                        p.strip_prefix(root)
                            .unwrap_or(&p)
                            .to_string_lossy()
                            .to_string(),
                    );
                }
            }
        }
    }
    walk(root, root, 0, &mut out);
    out.sort();
    out
}

/// Escanea una instalación de Antigravity (raíz `.gemini`).
pub fn scan_gemini_root(root: &Path) -> Result<Inventory, String> {
    if !root.is_dir() {
        return Err(format!("no existe la raíz de Antigravity: {}", root.display()));
    }
    let mcp_path = root.join("config").join("mcp_config.json");
    let cfg_path = root.join("config").join("config.json");
    let tasks_path = root
        .join("antigravity-app")
        .join("daemon")
        .join("tasks.json");
    let models_path = root.join("antigravity").join("custom_models.json");

    let mcp_raw = fs::read_to_string(&mcp_path).unwrap_or_default();
    let cfg_raw = fs::read_to_string(&cfg_path).unwrap_or_default();
    let tasks_raw = fs::read_to_string(&tasks_path).unwrap_or_default();
    let models_raw = fs::read_to_string(&models_path).unwrap_or_default();

    let mut servers = parse_mcp_config(&mcp_raw);
    // ¿existe el binario de cada servidor stdio?
    for s in servers.iter_mut() {
        if s.transport == TRANSPORT_STDIO && !s.command.is_empty() {
            s.command_exists = Path::new(&s.command).is_file();
        }
    }
    let stdio_binary_present = servers
        .iter()
        .any(|s| s.transport == TRANSPORT_STDIO && s.command_exists);

    // Proveedores: unión de `mcp_config.json` y `config.json`.
    let mut providers = parse_provider_credentials(&mcp_raw);
    for p in parse_provider_credentials(&cfg_raw) {
        if !providers.iter().any(|e| e.provider == p.provider) {
            providers.push(p);
        }
    }

    // Plugins: los directorios de `config/plugins`; si no hay, las claves de
    // `config.json → plugins` (lo que el cliente declara).
    let mut plugins = dir_names(&root.join("config").join("plugins"));
    if plugins.is_empty() {
        if let Ok(v) = serde_json::from_str::<Value>(&cfg_raw) {
            if let Some(obj) = v.get("plugins").and_then(|p| p.as_object()) {
                plugins = obj.keys().cloned().collect();
                plugins.sort();
            }
        }
    }
    let skills = dir_names(&root.join("config").join("skills"));
    let conversations = dir_names(&root.join("antigravity-app").join("brain")).len();

    Ok(Inventory {
        root: root.to_string_lossy().to_string(),
        plugins,
        skills,
        conversations,
        mcp_config_sha256: file_sha256(&mcp_path),
        config_sha256: file_sha256(&cfg_path),
        tasks_sha256: file_sha256(&tasks_path),
        servers,
        stdio_binary_present,
        providers,
        hooks: parse_daemon_tasks(&tasks_raw),
        credential_paths: credential_paths(root),
        custom_models: parse_custom_models(&models_raw),
    })
}

// ---------------------------------------------------------------------------
// Salida
// ---------------------------------------------------------------------------

/// Serializa el inventario.  **Nunca** contiene valores de credenciales.
pub fn inventory_to_json(inv: &Inventory) -> String {
    let servers: Vec<Value> = inv
        .servers
        .iter()
        .map(|s| {
            json!({
                "name": s.name,
                "transport": s.transport,
                "command": s.command,
                "command_exists": s.command_exists,
                "args": s.args,
                "env_names": s.env_names,
                "header_names": s.header_names,
                "url": s.url,
            })
        })
        .collect();
    let providers: Vec<Value> = inv
        .providers
        .iter()
        .map(|p| {
            json!({
                "provider": p.provider,
                "fields": p.fields,
                "value_len": p.value_len,
                "value_sha16": p.value_sha16
                    .iter()
                    .map(|(f, h)| json!({"field": f, "sha16": h}))
                    .collect::<Vec<Value>>(),
            })
        })
        .collect();
    let hooks: Vec<Value> = inv
        .hooks
        .iter()
        .map(|h| {
            json!({
                "id": h.id,
                "name": h.name,
                "trigger_type": h.trigger_type,
                "interval_sec": h.interval_sec,
                "enabled": h.enabled,
                "command_preview": h.command_preview,
                "command_len": h.command_len,
                "project": h.project,
                "conversation_id": h.conversation_id,
                "wake_app": h.wake_app,
                "last_run_status": h.last_run_status,
                "last_run_duration": h.last_run_duration,
            })
        })
        .collect();
    let models: Vec<Value> = inv
        .custom_models
        .iter()
        .map(|m| {
            json!({
                "provider": m.provider,
                "model_id": m.model_id,
                "display_name": m.display_name,
                "has_api_key": m.has_api_key,
                "megadaemon_provider": map_provider(&m.provider, &m.model_id),
            })
        })
        .collect();
    let v = json!({
        "schema": 1,
        "root": inv.root,
        "plugins": inv.plugins,
        "skills": inv.skills,
        "conversations": inv.conversations,
        "mcp_config_sha256": inv.mcp_config_sha256,
        "config_sha256": inv.config_sha256,
        "tasks_sha256": inv.tasks_sha256,
        "stdio_binary_present": inv.stdio_binary_present,
        "servers": servers,
        "providers": providers,
        "hooks": hooks,
        "credential_paths": inv.credential_paths,
        "custom_models": models,
    });
    serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".to_string())
}

/// Receta de cableado en texto plano.  **Nunca** incluye valores de
/// credenciales: sólo nombres de campo, cabeceras y rutas.
pub fn bridge_recommendation(inv: &Inventory, endpoint: &str) -> String {
    let mut s = String::new();
    s.push_str("== Puente Antigravity -> MegaDaemon ==\n");
    s.push_str(&format!("endpoint MCP del movil: {}\n", endpoint));
    s.push_str("\n-- Config que se escribe en ~/.gemini/config/mcp_config.json --\n");
    s.push_str(&format!(
        "  mcpServers.megadaemon.url = {}\n",
        endpoint
    ));
    s.push_str(&format!(
        "  mcpServers.megadaemon.headers.{} = <token de sesion, fichero 0600>\n",
        AUTH_HEADER
    ));
    s.push_str(&format!(
        "  (la {} sirve para las rutas /api/*; medido: /mcp NO la acepta sola)\n",
        API_KEY_HEADER
    ));
    s.push_str("  transporte: Streamable HTTP (McpStreamableHttpServer: url + headers)\n");

    let broken: Vec<&McpServerInfo> = inv
        .servers
        .iter()
        .filter(|x| x.transport == TRANSPORT_STDIO && !x.command_exists)
        .collect();
    if broken.is_empty() {
        s.push_str("\n-- servidores stdio: todos tienen binario --\n");
    } else {
        s.push_str("\n-- servidores stdio ROTOS (el cliente no puede arrancarlos) --\n");
        for b in broken {
            s.push_str(&format!(
                "  {}: el comando NO existe -> {}\n",
                b.name, b.command
            ));
        }
        s.push_str("  => no lanzar una copia local del binario: apuntar al daemon vivo.\n");
    }
    if !inv.stdio_binary_present {
        s.push_str("\n-- no hay NINGUN servidor stdio operativo: el MCP de MegaDaemon\n");
        s.push_str("   nunca ha podido arrancar desde el cliente.\n");
    }
    s.push_str("\n-- credenciales: solo rutas, nunca valores --\n");
    for p in &inv.credential_paths {
        s.push_str(&format!("  {}\n", p));
    }
    s
}

// ---------------------------------------------------------------------------
// Mapeo de proveedores (DoD 3: NO duplicar lo que ya hay)
// ---------------------------------------------------------------------------

/// Traduce (proveedor, modelo) de Antigravity al **id de proveedor que ya
/// existe en `providers.rs`**.  `None` = no hay equivalente (no inventar).
pub fn map_provider(provider: &str, model_id: &str) -> Option<&'static str> {
    let p = provider.to_ascii_lowercase();
    let m = model_id.to_ascii_lowercase();
    match p.as_str() {
        "minimax" => {
            if m.contains("m3.1-flash") || m.contains("m31flash") {
                Some("minimax-m3.1-flash")
            } else if m.contains("m3") {
                Some("minimax-m3")
            } else {
                Some("minimax-m3.1-flash")
            }
        }
        "gemini" | "google" | "google-ai-studio" => Some("google-ai-studio"),
        "glm" | "zai" | "z.ai" => Some("zai-glm-free"),
        "openrouter" => Some("openrouter-free"),
        "groq" => Some("groq-free"),
        "pollinations" => Some("pollinations-free"),
        "ovhcloud" | "ovh" => Some("ovhcloud-free"),
        "cerebras" => Some("cerebras"),
        "mistral" => Some("mistral-free"),
        "cohere" => Some("cohere-trial"),
        "nvidia" | "nim" => Some("nvidia-nim"),
        "sambanova" => Some("sambanova-free"),
        "deepseek" => Some("deepseek"),
        "anthropic" | "claude" => Some("anthropic"),
        "openai" | "gpt-oss" | "gpt" => Some("groq-free"),
        "cloudflare" => Some("cloudflare-workers-ai"),
        "seere" | "local" | "local-seere" | "hybrid-seere" => Some("local-seere"),
        _ => None,
    }
}

/// Modelo por defecto del proveedor de MegaDaemon.
pub fn provider_model(id: &str) -> Option<&'static str> {
    Some(match id {
        "minimax-m3.1-flash" => "MiniMax-M3.1-Flash",
        "minimax-m3" => "MiniMax-M3",
        "google-ai-studio" => "gemini-3.8-flash",
        "zai-glm-free" => "glm-4.7-flash",
        "openrouter-free" => "openrouter/free",
        "groq-free" => "openai/gpt-oss-120b",
        "pollinations-free" => "openai",
        "local-seere" => "local",
        "anthropic" => "claude-sonnet-5",
        "deepseek" => "deepseek-chat",
        "cerebras" => "llama-3.3-70b",
        "mistral-free" => "mistral-small-latest",
        "cohere-trial" => "command-a-03-2025",
        "nvidia-nim" => "nvidia/llama-3.3-nemotron-super-49b-v1",
        "sambanova-free" => "Llama-4-Scout-17B-16E-Instruct",
        "cloudflare-workers-ai" => "gpt-oss-120b",
        "ovhcloud-free" => "gpt-4o-mini",
        _ => return None,
    })
}

/// Tabla completa de equivalencias (para la doc y el `service_bus`).
pub fn provider_map_table() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("minimax", "MiniMax-M3.1-Flash", "minimax-m3.1-flash"),
        ("minimax", "MiniMax-M3", "minimax-m3"),
        ("gemini", "gemini-3.8-flash", "google-ai-studio"),
        ("glm", "glm-4.7-flash", "zai-glm-free"),
        ("openrouter", "openrouter/free", "openrouter-free"),
        ("groq", "openai/gpt-oss-120b", "groq-free"),
        ("pollinations", "openai", "pollinations-free"),
        ("ovhcloud", "gpt-4o-mini", "ovhcloud-free"),
        ("cerebras", "llama-3.3-70b", "cerebras"),
        ("mistral", "mistral-small-latest", "mistral-free"),
        ("cohere", "command-a-03-2025", "cohere-trial"),
        ("nvidia", "nvidia/llama-3.3-nemotron-super-49b-v1", "nvidia-nim"),
        ("sambanova", "Llama-4-Scout-17B-16E-Instruct", "sambanova-free"),
        ("deepseek", "deepseek-chat", "deepseek"),
        ("anthropic", "claude-sonnet-5", "anthropic"),
        ("openai", "gpt-oss-120b", "groq-free"),
        ("cloudflare", "gpt-oss-120b", "cloudflare-workers-ai"),
        ("seere", "local", "local-seere"),
    ]
}

// ---------------------------------------------------------------------------
// Puente: la entrada que se escribe en el cliente
// ---------------------------------------------------------------------------

/// Entrada `mcpServers` de **Streamable HTTP** (el transporte que el propio
/// SDK de Antigravity documenta: `McpStreamableHttpServer(name, url, headers)`).
///
/// El valor de la cabecera lo inyecta quien la escribe, leyendo el token del
/// fichero 0600: este módulo **no** conoce ningún secreto.
pub fn build_http_mcp_entry(id: &str, endpoint: &str, header_value: &str) -> Value {
    json!({
        "mcpServers": {
            id: {
                "url": endpoint,
                "headers": { AUTH_HEADER: header_value }
            }
        }
    })
}

/// Fusiona una entrada de servidor en un `mcp_config.json` existente **sin
/// tocar el resto** (conserva `providers` y los otros servidores).
pub fn merge_server_entry(existing_raw: &str, entry: &Value) -> Result<String, String> {
    let mut root: Value = serde_json::from_str(existing_raw)
        .map_err(|e| format!("mcp_config.json no es JSON: {}", e))?;
    let obj = root
        .as_object_mut()
        .ok_or("mcp_config.json no es un objeto")?;
    let servers = obj
        .entry("mcpServers".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let servers = servers
        .as_object_mut()
        .ok_or("mcpServers no es un objeto")?;
    if let Some(new) = entry.get("mcpServers").and_then(|m| m.as_object()) {
        for (k, v) in new {
            servers.insert(k.clone(), v.clone());
        }
    }
    serde_json::to_string_pretty(&root).map_err(|e| format!("serialización: {}", e))
}

// ---------------------------------------------------------------------------
// DoD 4: las tareas de Antigravity en el tablón y en el service_bus
// ---------------------------------------------------------------------------

/// Proyecta los hooks/tareas de Antigravity al formato de
/// `state/mesh_board.json` (el tablón del enjambre).
pub fn tasks_to_board_tasks(hooks: &[HookInfo]) -> Vec<Value> {
    hooks
        .iter()
        .map(|h| {
            json!({
                "task_id": format!("antigravity-{}", h.id),
                "title": h.name,
                "capability": "antigravity",
                "priority": 5,
                "owner": "antigravity",
                "payload": {
                    "source": "antigravity-daemon",
                    "trigger": h.trigger_type,
                    "interval_sec": h.interval_sec,
                    "enabled": h.enabled,
                    "project": h.project,
                    "conversation_id": h.conversation_id,
                    "wake_app": h.wake_app,
                    "command_preview": h.command_preview,
                    "command_len": h.command_len,
                    "last_run_status": h.last_run_status,
                    "last_run_duration": h.last_run_duration,
                },
            })
        })
        .collect()
}

/// Entrada de `service_bus` (W6) para Antigravity como **cliente MCP**.
///
/// `health` y `healthy` los pasa quien lo llama con el **valor medido**: esta
/// función no inventa estados.
pub fn service_bus_entry(
    endpoint: &str,
    transport: &str,
    tools: usize,
    health: &str,
    healthy: bool,
) -> Value {
    json!({
        "id": "antigravity",
        "kind": "agent_client",
        "vendor": "Google",
        "endpoint": endpoint,
        "transport": transport,
        "auth_kind": "session_token_file",
        "health": health,
        "healthy": healthy,
        "tools_count": tools,
        "capabilities": ["mcp_client", "mcp_http_server", "task_daemon", "byok_models"],
        "cost_estimate": "included",
        "latency_ms": Value::Null,
        "enabled": true,
    })
}

/// Ruta de la raíz `.gemini` en Android (para callers que no la conocen).
pub fn default_gemini_root() -> PathBuf {
    PathBuf::from(AGY_GEMINI)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redaccion_no_deja_pasar_el_secreto() {
        let r = redact_secret("sk-cp-abcdefghijklmnop");
        assert!(r.starts_with("sk-cp-"));
        assert!(!r.contains("efghij"));
        assert_eq!(redact_secret("corto"), "<oculto:5>");
    }

    #[test]
    fn sha16_es_estable_y_corto() {
        assert_eq!(sha16(b"hola").len(), 16);
        assert_eq!(sha16(b"hola"), sha16(b"hola"));
        assert_ne!(sha16(b"hola"), sha16(b"holb"));
    }

    #[test]
    fn parsea_mcp_config_con_dos_transportes() {
        let raw = r#"{"mcpServers":{
            "a":{"command":"/no/existe/megadaemon-core","args":["mcp-server"],"env":{"MEGADAEMON_MCP":"1"}},
            "b":{"url":"http://127.0.0.1:9090/mcp","headers":{"X-Seere-Auth-Token":"t"}}}}"#;
        let s = parse_mcp_config(raw);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].transport, TRANSPORT_STDIO);
        assert_eq!(s[0].env_names, vec!["MEGADAEMON_MCP".to_string()]);
        assert_eq!(s[1].transport, TRANSPORT_HTTP);
        assert_eq!(s[1].url.as_deref(), Some("http://127.0.0.1:9090/mcp"));
        assert!(s[1].header_names.contains(&"X-Seere-Auth-Token".to_string()));
    }

    #[test]
    fn parsea_credenciales_solo_estructura() {
        let raw = r#"{"providers":{"minimax":{"api_key":"mdaemon_sk_live_VALOR","base_url":"https://x"}}}"#;
        let p = parse_provider_credentials(raw);
        assert_eq!(p.len(), 1);
        assert!(p[0].fields.contains(&"api_key".to_string()));
        assert!(p[0].fields.contains(&"base_url".to_string()));
        assert_eq!(p[0].value_len, "mdaemon_sk_live_VALOR".len());
        assert_eq!(p[0].value_sha16[0].0, "api_key");
        assert_eq!(p[0].value_sha16[0].1.len(), 16);
    }

    #[test]
    fn parsea_tareas_con_triggers() {
        let raw = r#"[{"id":"t1","name":"L","command":"python3 x.py","enabled":true,
                   "trigger":{"type":"interval","interval_sec":300}}]"#;
        let t = parse_daemon_tasks(raw);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].trigger_type, "interval");
        assert_eq!(t[0].interval_sec, Some(300));
        assert!(t[0].enabled);
        assert_eq!(t[0].command_len, "python3 x.py".len());
    }

    #[test]
    fn mapea_proveedores_sin_duplicar() {
        assert_eq!(
            map_provider("minimax", "MiniMax-M3.1-Flash"),
            Some("minimax-m3.1-flash")
        );
        assert_eq!(map_provider("minimax", "MiniMax-M3"), Some("minimax-m3"));
        assert_eq!(
            map_provider("gemini", "gemini-3.8-flash"),
            Some("google-ai-studio")
        );
        assert_eq!(map_provider("glm", "glm-4.7-flash"), Some("zai-glm-free"));
        assert_eq!(map_provider("inventado", "x"), None);
        for (_, _, id) in provider_map_table() {
            assert!(provider_model(id).is_some(), "proveedor sin modelo: {}", id);
        }
    }

    #[test]
    fn genera_entrada_http_sin_secretos_hardcodeados() {
        let e = build_http_mcp_entry("megadaemon", "http://127.0.0.1:9090/mcp", "TOKEN");
        let s = serde_json::to_string(&e).unwrap();
        assert!(s.contains("127.0.0.1:9090/mcp"));
        assert!(s.contains(AUTH_HEADER));
        assert_eq!(e["mcpServers"]["megadaemon"]["headers"][AUTH_HEADER], "TOKEN");
    }

    #[test]
    fn merge_conserva_providers_y_otros_servidores() {
        let raw = r#"{"mcpServers":{"viejo":{"command":"x"}},"providers":{"minimax":{"api_key":"k"}}}"#;
        let entry = build_http_mcp_entry("megadaemon", "http://127.0.0.1:9090/mcp", "T");
        let out = merge_server_entry(raw, &entry).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert!(v["mcpServers"]["viejo"].is_object());
        assert!(v["mcpServers"]["megadaemon"].is_object());
        assert!(v["providers"]["minimax"].is_object());
    }

    #[test]
    fn proyecta_tareas_al_tablon() {
        let raw = r#"[{"id":"task_9","name":"Loop","command":"python3 /x.py","enabled":true,
                   "trigger":{"type":"webhook"},"project":"/p","conversation_id":"c1"}]"#;
        let hooks = parse_daemon_tasks(raw);
        let board = tasks_to_board_tasks(&hooks);
        assert_eq!(board.len(), 1);
        assert_eq!(board[0]["task_id"], "antigravity-task_9");
        assert_eq!(board[0]["owner"], "antigravity");
        assert_eq!(board[0]["payload"]["trigger"], "webhook");
    }

    #[test]
    fn entrada_service_bus_con_health_measure() {
        let e = service_bus_entry("http://127.0.0.1:9090/mcp", TRANSPORT_HTTP, 45, "ok", true);
        assert_eq!(e["kind"], "agent_client");
        assert_eq!(e["health"], "ok");
        assert_eq!(e["tools_count"], 45);
        assert_eq!(e["auth_kind"], "session_token_file");
    }
}
