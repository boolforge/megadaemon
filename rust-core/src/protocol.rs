//! MegaDaemon v16 — PROTOCOLO del «app-server» (JSON-RPC 2.0).
//!
//! ## Por qué este módulo
//!
//! MegaDaemon ya expone endpoints HTTP ad-hoc (`/v1/chat/completions`,
//! `/api/chat`, `/api/providers`, el dialecto Gemini `/v1internal:*`).  Pero
//! un cliente (el GUI web del cockpit, el cliente de Antigravity, un IDE, un
//! móvil) no tiene un CONTRATO estable: cada endpoint es un JSON distinto y
//! ninguno declara su versión.  En `openai/codex` ese problema está resuelto
//! con un `app-server` y su protocolo tipado
//! (`app-server-protocol`, `app-server-transport`, `app-server-client`,
//! `app-server-daemon`, `app-server-remote-client`).
//!
//! Este módulo es el contrato: **tipos + versión + nombres de método**, sin
//! lógica de negocio.  El transporte (HTTP o stdio) y el despachador van en
//! otro sitio; aquí sólo está lo que el cliente y el servidor tienen que
//! COMPARTIR para poder evolucionar sin romperse.
//!
//! ## Reglas del protocolo
//!
//! * Todo mensaje es un objeto JSON con `jsonrpc: "2.0"`.
//! * `id` es `null` en las notificaciones (sin respuesta) y un valor
//!   cualquiera en las peticiones.
//! * El error va en `error` (objeto con `code`, `message` y `data`
//!   opcional) y NUNCA junto a `result`.
//! * `protocol_version` se fija en el handshake (`initialize`) y las dos
//!   partes deben decir la misma; si no, el servidor responde con un error
//!   y cierra la sesión.
//!
//! ## Métodos (v1)
//!
//! | método | para qué |
//! |---|---|
//! | `initialize` | handshake: versión del protocolo, capacidades y lista de métodos |
//! | `session/create` | abre una sesión de chat (modelo, system prompt) |
//! | `session/list` | lista las sesiones abiertas |
//! | `session/message` | manda mensajes a una sesión y recibe la respuesta |
//! | `session/abort` | corta una sesión en curso |
//! | `tool/list` | herramientas MCP que el daemon expone |
//! | `tool/call` | ejecuta una herramienta MCP |
//! | `shutdown` | cierra la sesión de protocolo |

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Versión del protocolo.  Cambia sólo con un cambio incompatible.
pub const PROTOCOL_VERSION: &str = "1.0";

/// Nombre del protocolo (para el handshake).
pub const PROTOCOL_NAME: &str = "megadaemon.app-server";

/// Códigos de error del protocolo (JSON-RPC + los propios de MegaDaemon).
pub mod error_code {
    /// JSON inválido.
    pub const PARSE_ERROR: i64 = -32700;
    /// El sobre no es un objeto JSON-RPC válido.
    pub const INVALID_REQUEST: i64 = -32600;
    /// El método no existe.
    pub const METHOD_NOT_FOUND: i64 = -32601;
    /// Los parámetros no valen.
    pub const INVALID_PARAMS: i64 = -32602;
    /// Error interno del daemon.
    pub const INTERNAL: i64 = -32000;
}

/// Una petición (JSON-RPC 2.0).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// `jsonrpc` siempre vale "2.0".
    #[serde(default = "jsonrpc_version")]
    pub jsonrpc: String,
    /// Identificador de la petición; `null` = notificación (sin respuesta).
    #[serde(default)]
    pub id: Option<Value>,
    /// Nombre del método.
    pub method: String,
    /// Parámetros del método.
    #[serde(default)]
    pub params: Value,
}

fn jsonrpc_version() -> String {
    "2.0".to_string()
}

impl Request {
    /// Petición normal (con `id`).
    pub fn new(id: Value, method: &str, params: Value) -> Self {
        Request {
            jsonrpc: jsonrpc_version(),
            id: Some(id),
            method: method.to_string(),
            params,
        }
    }

    /// Notificación (sin `id` ⇒ sin respuesta).
    pub fn notification(method: &str, params: Value) -> Self {
        Request {
            jsonrpc: jsonrpc_version(),
            id: None,
            method: method.to_string(),
            params,
        }
    }

    /// ¿Es una notificación (no espera respuesta)?
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }
}

/// Una respuesta (JSON-RPC 2.0): `result` o `error`, nunca los dos.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    #[serde(default = "jsonrpc_version")]
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<Error>,
}

/// Un error de protocolo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl Error {
    pub fn new(code: i64, message: &str) -> Self {
        Error {
            code,
            message: message.to_string(),
            data: None,
        }
    }

    pub fn with_data(code: i64, message: &str, data: Value) -> Self {
        Error {
            code,
            message: message.to_string(),
            data: Some(data),
        }
    }
}

impl Response {
    /// Respuesta correcta.
    pub fn ok(id: Option<Value>, result: Value) -> Self {
        Response {
            jsonrpc: jsonrpc_version(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Respuesta de error.
    pub fn err(id: Option<Value>, code: i64, message: &str) -> Self {
        Response {
            jsonrpc: jsonrpc_version(),
            id,
            result: None,
            error: Some(Error::new(code, message)),
        }
    }
}

/// Parámetros del handshake `initialize`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InitializeParams {
    /// Versión que el cliente habla.
    #[serde(default)]
    pub protocol_version: String,
    /// Nombre del cliente (para el registro).
    #[serde(default)]
    pub client: String,
    /// Capacidades del cliente (libre: `Value`).
    #[serde(default)]
    pub capabilities: Value,
}

/// Respuesta del handshake `initialize`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    pub protocol_version: String,
    pub server: ServerInfo,
    /// Métodos que este daemon implementa.
    pub methods: Vec<String>,
}

/// Identidad del servidor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// Parámetros de `session/create`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionCreateParams {
    /// Modelo pedido (`None` = el que por defecto en la bóveda).
    #[serde(default)]
    pub model: Option<String>,
    /// Prompt de sistema para la sesión.
    #[serde(default)]
    pub system: Option<String>,
}

/// Una sesión de chat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Marca de tiempo de creación (epoch, segundos).
    pub created_at: i64,
}

/// Un mensaje de la sesión.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// Parámetros de `session/message`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionMessageParams {
    pub session_id: String,
    #[serde(default)]
    pub messages: Vec<Message>,
}

/// Parámetros de `tool/call`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolCallParams {
    /// Nombre de la herramienta MCP.
    pub name: String,
    /// Argumentos de la llamada.
    #[serde(default)]
    pub arguments: Value,
}

/// Resultado de una herramienta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    /// `true` si la herramienta terminó bien.
    pub ok: bool,
    /// Salida de la herramienta.
    #[serde(default)]
    pub content: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Los métodos del protocolo v1 (fuente única de verdad).
pub const METHODS: &[&str] = &[
    "initialize",
    "session/create",
    "session/list",
    "session/message",
    "session/abort",
    "tool/list",
    "tool/call",
    "shutdown",
];

/// ¿El daemon implementa este método?
pub fn tiene_metodo(m: &str) -> bool {
    METHODS.contains(&m)
}

/// Comprobación del handshake: el cliente y el servidor deben hablar la misma
/// versión.  Si no, devuelve el error que hay que devolverle.
pub fn comprobar_handshake(params: &InitializeParams) -> Result<(), Error> {
    let pedida = params.protocol_version.trim();
    if pedida.is_empty() {
        // El cliente no dice versión: se acepta (clientes simples), pero se
        // responde con la del daemon.
        return Ok(());
    }
    if pedida == PROTOCOL_VERSION {
        return Ok(());
    }
    Err(Error::with_data(
        error_code::INVALID_REQUEST,
        "versión de protocolo no soportada",
        json!({
            "soportada": PROTOCOL_VERSION,
            "pedida": pedida,
        }),
    ))
}

/// Respuesta del handshake con los datos del daemon.
pub fn initialize_result(version_daemon: &str) -> InitializeResult {
    InitializeResult {
        protocol_version: PROTOCOL_VERSION.to_string(),
        server: ServerInfo {
            name: PROTOCOL_NAME.to_string(),
            version: version_daemon.to_string(),
        },
        methods: METHODS.iter().map(|m| m.to_string()).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn una_peticion_va_y_vuelve_en_json() {
        let r = Request::new(json!(7), "tool/call", json!({"name": "fs_read"}));
        let texto = serde_json::to_string(&r).unwrap();
        assert!(texto.contains("\"jsonrpc\":\"2.0\""));
        assert!(texto.contains("\"method\":\"tool/call\""));
        let vuelta: Request = serde_json::from_str(&texto).unwrap();
        assert_eq!(vuelta.method, "tool/call");
        assert_eq!(vuelta.id, Some(json!(7)));
        assert!(!vuelta.is_notification());
    }

    #[test]
    fn una_notificacion_no_tiene_id() {
        let r = Request::notification("shutdown", Value::Null);
        assert!(r.is_notification());
        let v = serde_json::to_value(&r).unwrap();
        assert!(v.get("id").is_none());
    }

    #[test]
    fn una_respuesta_de_error_no_trae_result() {
        let r = Response::err(Some(json!(1)), error_code::METHOD_NOT_FOUND, "no existe");
        let v = serde_json::to_value(&r).unwrap();
        assert!(v.get("result").is_none(), "error y result no pueden ir juntos");
        assert_eq!(v["error"]["code"], error_code::METHOD_NOT_FOUND);
        assert_eq!(v["error"]["message"], "no existe");
    }

    #[test]
    fn los_metodos_conocidos_estan_declarados() {
        assert!(tiene_metodo("initialize"));
        assert!(tiene_metodo("tool/call"));
        assert!(!tiene_metodo("metodo_inventado"));
    }

    #[test]
    fn el_handshake_acepta_la_version_del_daemon() {
        let p = InitializeParams {
            protocol_version: PROTOCOL_VERSION.to_string(),
            client: "prueba".to_string(),
            capabilities: Value::Null,
        };
        assert!(comprobar_handshake(&p).is_ok());
    }

    #[test]
    fn el_handshake_rechaza_una_version_ajena() {
        let p = InitializeParams {
            protocol_version: "99.0".to_string(),
            client: "prueba".to_string(),
            capabilities: Value::Null,
        };
        let e = comprobar_handshake(&p).unwrap_err();
        assert_eq!(e.code, error_code::INVALID_REQUEST);
        assert_eq!(e.data.unwrap()["soportada"], PROTOCOL_VERSION);
    }

    #[test]
    fn la_lista_de_metodos_del_handshake_coincide() {
        let r = initialize_result("16.0");
        assert_eq!(r.protocol_version, PROTOCOL_VERSION);
        assert_eq!(r.methods.len(), METHODS.len());
        assert!(r.methods.iter().any(|m| m == "session/message"));
    }
}
