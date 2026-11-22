//! Puente de la flota contra la **IDE agéntica Antigravity** (`com.agy`) del móvil.
//!
//! QUÉ ES ESTE MÓDULO
//! ------------------
//! Antigravity (el IDE/agente de Google instalado en el móvil del usuario)
//! levanta su *language server* en `127.0.0.1:45157` (**HTTPS con
//! certificado autofirmado**).  Ese language server habla **Connect RPC**
//! (protobuf sobre JSON) con un único servicio:
//!
//! ```text
//! POST https://127.0.0.1:45157/exa.language_server_pb.LanguageServerService/<Metodo>
//! Content-Type: application/json
//! Connect-Protocol-Version: 1
//! x-codeium-csrf-token: <csrfToken del __APP_CONFIG__ del SPA>
//! ```
//!
//! El token sale de `GET /` (la SPA sirve `window.__APP_CONFIG__ = {...}`) y
//! **nunca** se escribe en el repo, en el log ni en el chat: de él sólo se
//! publica su `sha16` (identificador, no reversible).
//!
//! MEDIDAS REALES (Samsung S21 Ultra del usuario, 2026-11-21/22)
//! -----------------------------------------------------------
//! * `GetStatus` → **200** `{}`; `GetCapabilities` → **200**
//!   `{"supportsHookResultProtoBytes":true}`.
//! * `SearchConversations {"query":"README","limit":3}` → **200** con
//!   `{"results":[{"cascadeId":…,"title":…,"workspaceName":…,"snippet":…}]}`.
//! * `GetAllCascadeTrajectories {"exclude_subtrajectories":true}` → **200**
//!   con `{"trajectorySummaries":{"<cascadeId>":{…}}}`.
//! * `GetCascadeTrajectory {"cascadeId":…}` → **200** con la trayectoria y
//!   sus `steps`.
//! * `GetMcpServerStates {}` → **200** con `{"states":[{"spec":{…},"status":…}]}`.
//! * `StartCascade {"source":"CORTEX_TRAJECTORY_SOURCE_CASCADE_CLIENT"}` →
//!   **200** `{"cascadeId":"…"}`.
//! * `SendUserCascadeMessage {"cascadeId":…,"items":[{"text":…}]}` → **200** `{}`.
//! * `GetConversationMetadata {}` → **400**
//!   `invalid_argument: conversation_id is required` (error Connect real).
//!
//! REGLAS
//! ------
//! * **Ningún secreto sale del módulo**: el CSRF sólo se expone como `sha16`.
//! * Todo el parseo es **puro y testeable**: las funciones `*_from_*` reciben
//!   el texto y no tocan la red; la red vive sólo en [`AntigravityBridge`].
//! * Tiempos acotados siempre (nada de esperas infinitas) y errores que
//!   dicen **exactamente** qué pasó.

use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;
use std::time::Duration;

/// Puerto por defecto del language server de Antigravity (MEDIDO 2026-11-21).
pub const DEFAULT_PORT: u16 = 45157;
/// Host por defecto: el language server sólo escucha en loopback.
pub const DEFAULT_HOST: &str = "127.0.0.1";
/// Servicio Connect RPC con **331 métodos** (descubierto en `main.js`).
pub const SERVICE: &str = "exa.language_server_pb.LanguageServerService";
/// Cabecera del token CSRF (la acepta el language server de Antigravity).
pub const CSRF_HEADER: &str = "x-codeium-csrf-token";
/// Versión del protocolo Connect.
pub const CONNECT_VERSION: &str = "1";
/// Ruta del SPA que lleva el `__APP_CONFIG__` con el CSRF.
pub const SPA_PATH: &str = "/";
/// `source` de `StartCascade` para una cascada launched por el cliente.
pub const SOURCE_CASCADE_CLIENT: &str = "CORTEX_TRAJECTORY_SOURCE_CASCADE_CLIENT";
/// `trajectory_type` de `StartCascade` para una conversación de usuario.
pub const TRAJECTORY_USER_MAINLINE: &str = "CORTEX_TRAJECTORY_TYPE_USER_MAINLINE";
/// Modelo por defecto si el dispositivo no dice nada (BYOM de Antigravity).
pub const DEFAULT_MODEL: &str = "MODEL_PLACEHOLDER_M319";
/// Estado de Antigravity (de ahí se saca el modelo seleccionado).
pub const AGY_STATE: &str =
    "/data/user/0/com.agy/files/.gemini/antigravity-app/antigravity_state.pbtxt";
/// Directorio con las definiciones de tool MCP que Antigravity expone.
pub const AGY_MCP_DIR: &str = "/data/user/0/com.agy/files/.gemini/antigravity-app/mcp";
/// Cola de tareas del daemon de Antigravity.
pub const AGY_TASKS: &str =
    "/data/user/0/com.agy/files/.gemini/antigravity-app/daemon/tasks.json";
/// Timeout por defecto de una llamada RPC (segundos).
pub const DEFAULT_TIMEOUT_SECS: u64 = 25;

// ---------------------------------------------------------------------------
// Errores
// ---------------------------------------------------------------------------

/// Error del puente: siempre dice **qué** pasó y **dónde**.
#[derive(Debug, Clone, PartialEq)]
pub enum BridgeError {
    /// El servidor respondió con un código HTTP != 200.
    Http {
        status: u16,
        code: String,
        message: String,
    },
    /// Fallo de transporte (timeout, TLS, conexión rechazada).
    Transport(String),
    /// El SPA no trajo `__APP_CONFIG__.csrfToken`.
    NoCsrfToken,
    /// El cuerpo no es JSON válido.
    BadJson(String),
    /// El método existe pero la respuesta no trae lo que se esperaba.
    Unexpected(String),
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BridgeError::Http {
                status,
                code,
                message,
            } => write!(f, "HTTP {status} [{code}]: {message}"),
            BridgeError::Transport(e) => write!(f, "transporte: {e}"),
            BridgeError::NoCsrfToken => write!(
                f,
                "el SPA no trae __APP_CONFIG__.csrfToken (¿el language server no está arrancado?)"
            ),
            BridgeError::BadJson(e) => write!(f, "JSON inválido: {e}"),
            BridgeError::Unexpected(e) => write!(f, "respuesta inesperada: {e}"),
        }
    }
}

impl std::error::Error for BridgeError {}

/// Resultado del puente.
pub type BridgeResult<T> = Result<T, BridgeError>;

// ---------------------------------------------------------------------------
// Configuración
// ---------------------------------------------------------------------------

/// Configuración del puente.
#[derive(Debug, Clone, PartialEq)]
pub struct BridgeConfig {
    pub host: String,
    pub port: u16,
    /// Timeout global de cada llamada.
    pub timeout_secs: u64,
    /// Modelo con el que se lanza el agente (`None` ⇒ [`DEFAULT_MODEL`]).
    pub model: Option<String>,
    /// Prefijo del workspace que se pasa al agente al encolar (opcional).
    pub workspace_uris: Vec<String>,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            host: DEFAULT_HOST.to_string(),
            port: DEFAULT_PORT,
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            model: None,
            workspace_uris: Vec::new(),
        }
    }
}

impl BridgeConfig {
    /// Base HTTPS del language server.
    pub fn base(&self) -> String {
        format!("https://{}:{}", self.host, self.port)
    }
}

// ---------------------------------------------------------------------------
// Parseo puro (testeable sin red)
// ---------------------------------------------------------------------------

/// Saca el objeto `__APP_CONFIG__` del HTML del SPA.
///
/// Devuelve el JSON del objeto, no el texto entero: así el llamante puede
/// pedir lo que quiera (p. ej. `csrfToken`) sin que el token se cuele en
/// logs por accidente.
pub fn app_config_from_html(html: &str) -> Option<Value> {
    let start = html.find("__APP_CONFIG__")?;
    let obj_start = html[start..].find('{')? + start;
    // Escaneo de llaves respetando comillas y escapes.
    let bytes: Vec<char> = html[obj_start..].chars().collect();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    let mut end = None;
    for (i, c) in bytes.iter().enumerate() {
        if in_str {
            if esc {
                esc = false;
            } else if *c == '\\' {
                esc = true;
            } else if *c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(obj_start + i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end?;
    serde_json::from_str(&html[obj_start..end]).ok()
}

/// El token CSRF del SPA (`__APP_CONFIG__.csrfToken`).
pub fn csrf_from_html(html: &str) -> Option<String> {
    app_config_from_html(html)?
        .get("csrfToken")?
        .as_str()
        .map(|s| s.to_string())
}

/// Interpreta un error Connect (`{"code":…,"message":…}`).
pub fn parse_connect_error(body: &str) -> Option<(String, String)> {
    let v: Value = serde_json::from_str(body).ok()?;
    let code = v.get("code")?.as_str()?.to_string();
    let msg = v
        .get("message")
        .and_then(|m| m.as_str())
        .unwrap_or("")
        .to_string();
    Some((code, msg))
}

/// Saca `last_selected_agent_model` del `antigravity_state.pbtxt`.
///
/// El estado es un proto-text; la clave que interesa es una línea suelta.
pub fn model_from_state(text: &str) -> Option<String> {
    for line in text.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("last_selected_agent_model:") {
            let v = rest.trim().trim_matches('"').to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

/// Una tool MCP que Antigravity tiene expuesta.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct McpToolInfo {
    /// Nombre del servidor MCP al que pertenece (carpeta).
    pub server: String,
    /// Nombre de la tool.
    pub name: String,
    /// Descripción corta (la primera línea).
    pub description: String,
    /// Nº de parámetros declarados.
    pub params: usize,
}

/// Parsea una definición de tool MCP (`{"name","description","parameters"}`).
pub fn parse_tool_definition(server: &str, body: &str) -> Option<McpToolInfo> {
    let v: Value = serde_json::from_str(body).ok()?;
    let name = v.get("name")?.as_str()?.to_string();
    let description = v
        .get("description")
        .and_then(|d| d.as_str())
        .unwrap_or("")
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    let params = v
        .get("parameters")
        .and_then(|p| p.get("properties"))
        .and_then(|p| p.as_object())
        .map(|m| m.len())
        .unwrap_or(0);
    Some(McpToolInfo {
        server: server.to_string(),
        name,
        description,
        params,
    })
}

/// Lee el catálogo de tools MCP que Antigravity expone desde el disco.
///
/// Cada subdirectorio es un servidor MCP y cada `*.json` una tool.
pub fn tool_catalog_from_dir(dir: &Path) -> Vec<McpToolInfo> {
    let mut out = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for srv in entries.flatten() {
        if !srv.path().is_dir() {
            continue;
        }
        let server = srv.file_name().to_string_lossy().to_string();
        let files = match fs::read_dir(srv.path()) {
            Ok(f) => f,
            Err(_) => continue,
        };
        for f in files.flatten() {
            let p = f.path();
            if p.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Ok(body) = fs::read_to_string(&p) {
                if let Some(mut t) = parse_tool_definition(&server, &body) {
                    if t.name.is_empty() {
                        t.name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
                    }
                    out.push(t);
                }
            }
        }
    }
    out.sort_by(|a, b| (&a.server, &a.name).cmp(&(&b.server, &b.name)));
    out
}

/// Resumen de una conversación (una entrada de `GetAllCascadeTrajectories`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ConversationSummary {
    pub cascade_id: String,
    pub title: String,
    pub trajectory_id: String,
    pub status: String,
    pub step_count: u64,
    pub last_modified: String,
    pub workspaces: Vec<String>,
}

/// Convierte la respuesta de `GetAllCascadeTrajectories` en resúmenes.
///
/// La respuesta real es un **mapa** `cascadeId → resumen`; se acepta
/// también una lista por si el servidor cambiara de forma.
pub fn conversation_summaries_from(v: &Value) -> Vec<ConversationSummary> {
    let mut out = Vec::new();
    let src = v
        .get("trajectorySummaries")
        .unwrap_or(v)
        .get("trajectorySummaries")
        .unwrap_or(v);
    match src {
        Value::Object(map) => {
            for (cid, s) in map {
                out.push(summary_from(cid, s));
            }
        }
        Value::Array(arr) => {
            for s in arr {
                let cid = s
                    .get("cascadeId")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                out.push(summary_from(&cid, s));
            }
        }
        _ => {}
    }
    out.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
    out
}

fn summary_from(cid: &str, s: &Value) -> ConversationSummary {
    let workspaces = s
        .get("workspaces")
        .and_then(|w| w.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|w| {
                    w.get("workspaceFolderAbsoluteUri")
                        .and_then(|u| u.as_str())
                        .map(|u| u.to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    ConversationSummary {
        cascade_id: cid.to_string(),
        title: s
            .get("summary")
            .or_else(|| s.get("title"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        trajectory_id: s
            .get("trajectoryId")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        status: s
            .get("status")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        step_count: s.get("stepCount").and_then(|x| x.as_u64()).unwrap_or(0),
        last_modified: s
            .get("lastModifiedTime")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        workspaces,
    }
}

/// Un paso (step) de una trayectoria.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StepSummary {
    pub kind: String,
    pub status: String,
    /// Texto del paso (el que se pueda extraer sin inventar nada).
    pub text: String,
    /// Mensaje de error, si el paso falló.
    pub error: String,
}

/// Resume los pasos de `GetCascadeTrajectory`.
pub fn step_summaries_from(v: &Value) -> Vec<StepSummary> {
    let mut out = Vec::new();
    let steps = v
        .get("trajectory")
        .and_then(|t| t.get("steps"))
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();
    for s in steps {
        let kind = s.get("type").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let status = s
            .get("metadata")
            .and_then(|m| m.get("status"))
            .or_else(|| s.get("status"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let mut text = String::new();
        for key in [
            "plannerResponse",
            "agentMessage",
            "userInput",
            "ephemeralMessage",
            "viewFile",
            "runCommand",
            "modelResponse",
        ] {
            if let Some(v) = s.get(key) {
                if let Some(t) = extract_text(v) {
                    if !t.is_empty() {
                        text = t;
                        break;
                    }
                }
            }
        }
        let error = s
            .get("errorMessage")
            .and_then(|e| e.get("error"))
            .and_then(|e| e.get("shortError"))
            .or_else(|| s.get("errorMessage").and_then(|e| e.get("shortError")))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        out.push(StepSummary {
            kind,
            status,
            text,
            error,
        });
    }
    out
}

/// Extrae un texto legible de un campo del paso (sin inventar nada).
fn extract_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Object(_) => {
            for key in ["text", "response", "message", "userResponse", "content"] {
                if let Some(Value::String(s)) = v.get(key) {
                    return Some(s.clone());
                }
            }
            // Un item de `TextOrScopeItem` trae `{"text": "..."}`.
            if let Some(Value::String(s)) = v.get("text") {
                return Some(s.clone());
            }
            None
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Cliente
// ---------------------------------------------------------------------------

/// Cliente del language server de Antigravity.
///
/// El certificado es **autofirmado** ⇒ la verificación TLS se desactiva
/// (el canal es loopback: `127.0.0.1`, el mismo proceso del usuario).
#[derive(Debug)]
pub struct AntigravityBridge {
    base: String,
    csrf: String,
    model: String,
    agent: ureq::Agent,
}

impl AntigravityBridge {
    /// Crea el cliente **descubriendo** el token CSRF en el SPA.
    pub fn new(cfg: &BridgeConfig) -> BridgeResult<Self> {
        let agent = build_agent(cfg.timeout_secs);
        let html = fetch_text(&agent, &format!("{}{}", cfg.base(), SPA_PATH))?;
        let csrf = csrf_from_html(&html).ok_or(BridgeError::NoCsrfToken)?;
        let model = cfg
            .model
            .clone()
            .or_else(|| model_from_state(&std::fs::read_to_string(AGY_STATE).unwrap_or_default()))
            .unwrap_or_else(|| DEFAULT_MODEL.to_string());
        Ok(Self {
            base: cfg.base(),
            csrf,
            model,
            agent,
        })
    }

    /// Crea el cliente con un token CSRF ya conocido.
    pub fn with_csrf(cfg: &BridgeConfig, csrf: &str) -> BridgeResult<Self> {
        Ok(Self {
            base: cfg.base(),
            csrf: csrf.to_string(),
            model: cfg
                .model
                .clone()
                .unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            agent: build_agent(cfg.timeout_secs),
        })
    }

    /// `sha16` del token CSRF (identificador; **nunca** el token).
    pub fn csrf_sha16(&self) -> String {
        crate::antigravity_intel::sha16(self.csrf.as_bytes())
    }

    /// Modelo con el que se lanza el agente.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// URL completa de un método Connect.
    pub fn method_url(&self, method: &str) -> String {
        format!("{}/{}/{}", self.base, SERVICE, method)
    }

    /// Llama a un método Connect RPC y devuelve el JSON de la respuesta.
    ///
    /// Un 4xx/5xx con cuerpo Connect (`{"code":…,"message":…}`) se
    /// convierte en [`BridgeError::Http`] con el **código y el mensaje
    /// literales** del servidor.
    pub fn rpc(&self, method: &str, body: &Value) -> BridgeResult<Value> {
        let url = self.method_url(method);
        let payload = serde_json::to_string(body).map_err(|e| BridgeError::BadJson(e.to_string()))?;
        let resp = self
            .agent
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Connect-Protocol-Version", CONNECT_VERSION)
            .header(CSRF_HEADER, &self.csrf)
            .header("User-Agent", "MegaDaemon-antigravity-bridge")
            .send(payload.as_str())
            .map_err(|e| BridgeError::Transport(describe_ureq(&e)))?;
        let status = resp.status().as_u16();
        let text = resp
            .into_body()
            .read_to_string()
            .map_err(|e| BridgeError::Transport(format!("lectura del cuerpo: {e}")))?;
        if status != 200 {
            let (code, message) = parse_connect_error(&text)
                .unwrap_or_else(|| ("http_error".to_string(), text.chars().take(400).collect()));
            return Err(BridgeError::Http {
                status,
                code,
                message,
            });
        }
        if text.trim().is_empty() {
            return Ok(Value::Object(Map::new()));
        }
        serde_json::from_str(&text).map_err(|e| BridgeError::BadJson(format!("{e}: {}", text.chars().take(200).collect::<String>())))
    }

    /// `GetStatus`: el language server está vivo.
    pub fn status(&self) -> BridgeResult<Value> {
        self.rpc("GetStatus", &json!({}))
    }

    /// `GetCapabilities`.
    pub fn capabilities(&self) -> BridgeResult<Value> {
        self.rpc("GetCapabilities", &json!({}))
    }

    /// `GetMcpServerStates`: los servidores MCP que Antigravity tiene
    /// cargados, con su estado vivo y su error (si lo tienen).
    pub fn catalog(&self) -> BridgeResult<Value> {
        self.rpc("GetMcpServerStates", &json!({}))
    }

    /// `GetAllCascadeTrajectories`: todas las conversaciones/agentes.
    pub fn conversations(&self) -> BridgeResult<Vec<ConversationSummary>> {
        let v = self.rpc(
            "GetAllCascadeTrajectories",
            &json!({ "exclude_subtrajectories": true }),
        )?;
        Ok(conversation_summaries_from(&v))
    }

    /// `SearchConversations`: busca en el historial por texto.
    pub fn search(&self, query: &str, limit: u32) -> BridgeResult<Value> {
        self.rpc(
            "SearchConversations",
            &json!({ "query": query, "limit": limit }),
        )
    }

    /// `GetCascadeTrajectory`: lee una conversación entera.
    pub fn conversation(&self, cascade_id: &str) -> BridgeResult<Value> {
        self.rpc("GetCascadeTrajectory", &json!({ "cascadeId": cascade_id }))
    }

    /// `GetUserTrajectory`: lee una trayectoria por su `trajectoryId`.
    pub fn user_trajectory(&self, trajectory_id: &str) -> BridgeResult<Value> {
        self.rpc(
            "GetUserTrajectory",
            &json!({ "trajectoryId": trajectory_id }),
        )
    }

    /// Encola trabajo en el agente de Antigravity.
    ///
    /// Hace **dos** llamadas (medidas):
    ///   1. `StartCascade` ⇒ devuelve el `cascadeId` de la conversación;
    ///   2. `SendUserCascadeMessage` ⇒ mete el texto como paso de usuario.
    ///
    /// El `cascadeConfig` lleva el modelo: sin él el agente responde
    /// `failed to construct executor: plan model not specified`.
    pub fn enqueue(&self, prompt: &str, workspace_uris: &[String]) -> BridgeResult<String> {
        let mut start = json!({
            "source": SOURCE_CASCADE_CLIENT,
            "trajectoryType": TRAJECTORY_USER_MAINLINE,
        });
        if !workspace_uris.is_empty() {
            start
                .as_object_mut()
                .expect("json! siempre es objeto")
                .insert(
                    "workspaceUris".to_string(),
                    Value::Array(
                        workspace_uris
                            .iter()
                            .map(|u| Value::String(u.clone()))
                            .collect(),
                    ),
                );
        }
        let started = self.rpc("StartCascade", &start)?;
        let cascade_id = started
            .get("cascadeId")
            .and_then(|x| x.as_str())
            .ok_or_else(|| {
                BridgeError::Unexpected(format!(
                    "StartCascade sin cascadeId: {}",
                    started.to_string().chars().take(200).collect::<String>()
                ))
            })?
            .to_string();
        self.send_message(&cascade_id, prompt)?;
        Ok(cascade_id)
    }

    /// Manda un mensaje de usuario a una cascada ya abierta.
    pub fn send_message(&self, cascade_id: &str, prompt: &str) -> BridgeResult<Value> {
        self.rpc(
            "SendUserCascadeMessage",
            &json!({
                "cascadeId": cascade_id,
                "items": [ { "text": prompt } ],
                "cascadeConfig": {
                    "plannerConfig": {
                        "planModel": self.model,
                        "requestedModel": { "model": self.model },
                    }
                }
            }),
        )
    }

    /// Estado del agente: `GetStatus` + `GetUserStatus` + nº de
    /// conversaciones (todo medido, nada inventado).
    pub fn agent_status(&self) -> BridgeResult<Value> {
        let status = self.status()?;
        let user = self.rpc("GetUserStatus", &json!({})).unwrap_or(Value::Null);
        let convs = self.conversations().map(|c| c.len()).unwrap_or(0);
        Ok(json!({
            "language_server": status,
            "user": user,
            "conversations": convs,
            "model": self.model,
            "csrf_sha16": self.csrf_sha16(),
            "base": self.base,
        }))
    }

    /// Catálogo de tools MCP: el estado vivo (por API) + las definiciones
    /// que hay en disco.
    pub fn tool_catalog(&self) -> BridgeResult<Value> {
        let states = self.catalog().unwrap_or(Value::Null);
        let servers: Vec<Value> = states
            .get("states")
            .and_then(|s| s.as_array())
            .map(|a| {
                a.iter()
                    .map(|s| {
                        json!({
                            "server": s.pointer("/spec/serverName").and_then(|x| x.as_str()).unwrap_or(""),
                            "status": s.get("status").and_then(|x| x.as_str()).unwrap_or(""),
                            "error": s.get("error").and_then(|x| x.as_str()).unwrap_or(""),
                            "transport": if s.pointer("/spec/url").is_some() { "http" } else { "stdio" },
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let tools: Vec<Value> = tool_catalog_from_dir(Path::new(AGY_MCP_DIR))
            .into_iter()
            .map(|t| {
                json!({
                    "server": t.server,
                    "name": t.name,
                    "params": t.params,
                    "description": t.description,
                })
            })
            .collect();
        Ok(json!({
            "servers": servers,
            "tools": tools,
            "tool_count": tools.len(),
            "server_count": servers.len(),
        }))
    }
}

// ---------------------------------------------------------------------------
// Utilidades internas
// ---------------------------------------------------------------------------

/// Agente HTTP con TLS sin verificación (cert autofirmado) y timeout.
fn build_agent(timeout_secs: u64) -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::Rustls)
        .disable_verification(true)
        .build();
    let cfg = ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(Duration::from_secs(timeout_secs)))
        .timeout_connect(Some(Duration::from_secs(5)))
        .http_status_as_error(false)
        .build();
    ureq::Agent::new_with_config(cfg)
}

/// GET simple (usado para el SPA).
fn fetch_text(agent: &ureq::Agent, url: &str) -> BridgeResult<String> {
    let resp = agent
        .get(url)
        .header("User-Agent", "MegaDaemon-antigravity-bridge")
        .call()
        .map_err(|e| BridgeError::Transport(describe_ureq(&e)))?;
    let status = resp.status().as_u16();
    let text = resp
        .into_body()
        .read_to_string()
        .map_err(|e| BridgeError::Transport(format!("lectura del cuerpo: {e}")))?;
    if status != 200 {
        return Err(BridgeError::Http {
            status,
            code: "http_error".to_string(),
            message: text.chars().take(200).collect(),
        });
    }
    Ok(text)
}

/// Traduce el error de `ureq` a un mensaje entendible.
fn describe_ureq(e: &ureq::Error) -> String {
    match e {
        ureq::Error::StatusCode(c) => format!("código HTTP {c}"),
        ureq::Error::Transport(t) => format!("transporte: {t}"),
    }
}
