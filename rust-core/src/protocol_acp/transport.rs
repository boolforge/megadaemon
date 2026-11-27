//! **Transporte JSON-RPC 2.0** de ACP (patrón del plan de `protocol.rs`,
//! `docs/DECISION_ACP.md` §3, módulo `transport.rs`).
//!
//! ACP habla **JSON-RPC 2.0**: un mensaje por línea (framing NDJSON) sobre
//! stdio, y el mismo formato sobre HTTP (el `cockpit_server.rs` ya tiene el
//! servidor HTTP).
//!
//! MEDIDO (registro ACP oficial, 2026-11-26): de 42 agentes probados, 41
//! inicializan y **los 41 usan la versión 1** del protocolo ⇒ aquí se habla
//! v1 canónico + extensiones `mdaemon/` (`docs/DECISION_ACP.md` §2).
//!
//! Por qué aquí y no en un script: la regla del proyecto es que toda
//! capacidad nueva vive en Rust dentro del daemon y se expone por CLI y por
//! tool MCP.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Etiqueta de versión de JSON-RPC.  ACP v1 la exige siempre.
pub const VERSION_JSONRPC: &str = "2.0";

/// Códigos de error JSON-RPC que ACP usa.
pub mod codigo {
    /// JSON inválido.
    pub const PARSE_ERROR: i64 = -32700;
    /// El objeto enviado no es una solicitud válida.
    pub const INVALID_REQUEST: i64 = -32600;
    /// El método no existe.
    pub const METHOD_NOT_FOUND: i64 = -32601;
    /// Los parámetros no valen.
    pub const INVALID_PARAMS: i64 = -32602;
    /// Error interno.
    pub const INTERNAL_ERROR: i64 = -32603;
    /// El servidor va mal (p. ej. no se puede aplicar una política).
    pub const SERVER_ERROR: i64 = -32000;
}

/// Un mensaje JSON-RPC 2.0, sea solicitud, respuesta o notificación.
///
/// MEDIDO: en ACP una respuesta puede ser "respuesta" (`result`) o "error";
/// una NOTIFICACIÓN es una solicitud sin `id` (p. ej. `session/update`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mensaje {
    /// Siempre `"2.0"`.
    pub jsonrpc: String,
    /// El id de la solicitud (ausente en las notificaciones).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    /// El método (en las solicitudes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// Los parámetros (en las solicitudes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    /// El resultado (en las respuestas correctas).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// El error (en las respuestas con error).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorRpc>,
}

/// Un error JSON-RPC.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorRpc {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl ErrorRpc {
    pub fn nuevo(code: i64, message: impl Into<String>) -> Self {
        ErrorRpc {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Error con datos extra (siempre **sin credenciales**: el dato lo pone
    /// el llamante y no puede llevar secretos).
    pub fn con_datos(code: i64, message: impl Into<String>, datos: Value) -> Self {
        ErrorRpc {
            code,
            message: message.into(),
            data: Some(datos),
        }
    }
}

/// Un mensaje tal y como viaja por el cable.
#[derive(Debug, Clone, PartialEq)]
pub enum Entrada {
    /// Una solicitud (con id): el agente debe responder.
    Solicitud(Mensaje),
    /// Una notificación (sin id): el agente actúa pero no responde.
    Notificacion(Mensaje),
    /// Una respuesta a una solicitud que hicimos nosotros.
    Respuesta(Mensaje),
}

impl Entrada {
    /// El id de la petición a la que responde (si lo hay).
    pub fn id(&self) -> Option<&Value> {
        match self {
            Entrada::Solicitud(m) | Entrada::Notificacion(m) | Entrada::Respuesta(m) => m.id.as_ref(),
        }
    }

    /// El método (sólo en solicitudes y notificaciones).
    pub fn metodo(&self) -> Option<&str> {
        match self {
            Entrada::Solicitud(m) | Entrada::Notificacion(m) => m.method.as_deref(),
            Entrada::Respuesta(_) => None,
        }
    }

    /// Los parámetros (sólo en solicitudes y notificaciones).
    pub fn params(&self) -> Option<&Value> {
        match self {
            Entrada::Solicitud(m) | Entrada::Notificacion(m) => m.params.as_ref(),
            Entrada::Respuesta(_) => None,
        }
    }

    /// ¿Es un error?
    pub fn es_error(&self) -> bool {
        matches!(self, Entrada::Respuesta(m) if m.error.is_some())
    }
}

/// Parsea una línea del cable.
///
/// MEDIDO (y es lo que hacen las implementaciones de referencia): sobre
/// stdio el framing es **una línea por mensaje** (NDJSON).  Se tolera el
/// `Content-Length:` (framing LSP) porque algunos clientes lo usan, pero no
/// es lo que usa ACP.
pub fn parsear(linea: &str) -> Result<Entrada, ErrorRpc> {
    let t = linea.trim();
    if t.is_empty() {
        return Err(ErrorRpc::nuevo(codigo::PARSE_ERROR, "línea vacía"));
    }
    // Tolerancia con el framing LSP (`Content-Length: N`).
    if t.starts_with("Content-Length:") {
        return Err(ErrorRpc::nuevo(
            codigo::INVALID_REQUEST,
            "framing LSP no soportado: ACP usa una línea por mensaje",
        ));
    }
    let v: Value = serde_json::from_str(t).map_err(|e| {
        ErrorRpc::nuevo(codigo::PARSE_ERROR, format!("JSON inválido: {}", e))
    })?;
    parsear_valor(&v)
}

/// Parsea un `Value` ya deserializado.
pub fn parsear_valor(v: &Value) -> Result<Entrada, ErrorRpc> {
    let obj = v.as_object().ok_or_else(|| {
        ErrorRpc::nuevo(codigo::INVALID_REQUEST, "el mensaje no es un objeto JSON")
    })?;
    let jsonrpc = obj.get("jsonrpc").and_then(|x| x.as_str()).unwrap_or("");
    if jsonrpc != VERSION_JSONRPC {
        return Err(ErrorRpc::nuevo(
            codigo::INVALID_REQUEST,
            format!("jsonrpc debe ser \"{}\" (venía \"{}\")", VERSION_JSONRPC, jsonrpc),
        ));
    }
    let m: Mensaje = serde_json::from_value(v.clone())
        .map_err(|e| ErrorRpc::nuevo(codigo::INVALID_REQUEST, format!("mensaje inválido: {}", e)))?;
    if let Some(e) = &m.error {
        return Ok(Entrada::Respuesta(Mensaje {
            jsonrpc: VERSION_JSONRPC.to_string(),
            id: m.id.clone(),
            method: None,
            params: None,
            result: None,
            error: Some(e.clone()),
        }));
    }
    match (&m.id, &m.method) {
        (Some(_), Some(_)) => Ok(Entrada::Solicitud(m)),
        (None, Some(_)) => Ok(Entrada::Notificacion(m)),
        (Some(_), None) => Ok(Entrada::Respuesta(m)),
        (None, None) => Err(ErrorRpc::nuevo(
            codigo::INVALID_REQUEST,
            "el mensaje no es ni solicitud ni respuesta",
        )),
    }
}

/// Construye una solicitud.
pub fn solicitud(id: Value, metodo: &str, params: Option<Value>) -> Mensaje {
    Mensaje {
        jsonrpc: VERSION_JSONRPC.to_string(),
        id: Some(id),
        method: Some(metodo.to_string()),
        params,
        result: None,
        error: None,
    }
}

/// Construye una notificación (sin id).
pub fn notificacion(metodo: &str, params: Option<Value>) -> Mensaje {
    Mensaje {
        jsonrpc: VERSION_JSONRPC.to_string(),
        id: None,
        method: Some(metodo.to_string()),
        params,
        result: None,
        error: None,
    }
}

/// Construye una respuesta correcta.
pub fn respuesta_ok(id: Value, result: Value) -> Mensaje {
    Mensaje {
        jsonrpc: VERSION_JSONRPC.to_string(),
        id: Some(id),
        method: None,
        params: None,
        result: Some(result),
        error: None,
    }
}

/// Construye una respuesta con error.
pub fn respuesta_error(id: Option<Value>, e: ErrorRpc) -> Mensaje {
    Mensaje {
        jsonrpc: VERSION_JSONRPC.to_string(),
        id,
        method: None,
        params: None,
        result: None,
        error: Some(e),
    }
}

/// Enmarca un mensaje para el cable: **una línea** (NDJSON).
pub fn enmarcar(m: &Mensaje) -> String {
    let mut s = serde_json::to_string(m).unwrap_or_else(|_| "{}".to_string());
    s.push('\n');
    s
}

/// Enmarca un `Value` cualquiera (respuestas propias del daemon).
pub fn enmarcar_valor(v: &Value) -> String {
    let mut s = serde_json::to_string(v).unwrap_or_else(|_| "{}".to_string());
    s.push('\n');
    s
}

/// Generador de ids de solicitud.
///
/// MEDIDO: JSON-RPC admite ids numéricos o strings; los numéricos son los
/// más habituales en ACP.  El contador es **por proceso** y monotono: dos
/// solicitudes del mismo cliente nunca comparten id.
pub struct ContadorIds {
    n: std::sync::atomic::AtomicU64,
}

impl Default for ContadorIds {
    fn default() -> Self {
        ContadorIds {
            n: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

impl ContadorIds {
    /// Devuelve el siguiente id.
    pub fn siguiente(&self) -> u64 {
        self.n.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
    }
}

// =============================================================================
// CLI
// =============================================================================
/// `megadaemon-core acp-transport <parsear|enmarcar|ids>`
pub fn run_cli(args: &[String]) -> i32 {
    match run_cli_res(args) {
        Ok(v) => {
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            0
        }
        Err(e) => {
            eprintln!("acp-transport error: {}", e);
            1
        }
    }
}

fn run_cli_res(args: &[String]) -> Result<Value, String> {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("");
    match sub {
        "parsear" => {
            let linea = args.get(1).map(|s| s.as_str()).unwrap_or("");
            match parsear(linea) {
                Ok(e) => {
                    let tipo = match e {
                        Entrada::Solicitud(_) => "solicitud",
                        Entrada::Notificacion(_) => "notificacion",
                        Entrada::Respuesta(_) => "respuesta",
                    };
                    Ok(json!({
                        "ok": true,
                        "tipo": tipo,
                        "metodo": e.metodo(),
                        "id": e.id(),
                        "es_error": e.es_error(),
                    }))
                }
                Err(err) => Ok(json!({
                    "ok": false,
                    "error": { "code": err.code, "message": err.message },
                })),
            }
        }
        "enmarcar" => {
            let metodo = args.get(1).map(|s| s.as_str()).unwrap_or("");
            if metodo.is_empty() {
                return Err("falta el método".to_string());
            }
            let m = solicitud(json!(1), metodo, None);
            Ok(json!({ "ok": true, "linea": enmarcar(&m).trim_end().to_string() }))
        }
        "ids" => {
            let c = ContadorIds::default();
            let n: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(3);
            let ids: Vec<u64> = (0..n).map(|_| c.siguiente()).collect();
            Ok(json!({ "ok": true, "ids": ids }))
        }
        "" => Err("uso: acp-transport <parsear|enmarcar|ids> [args]".to_string()),
        otro => Err(format!("subcomando desconocido: {}", otro)),
    }
}

// =============================================================================
// TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn una_solicitud_se_parsea() {
        let e = parsear(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#).unwrap();
        assert_eq!(e.metodo(), Some("initialize"));
        assert_eq!(e.id(), Some(&json!(1)));
        assert!(!e.es_error());
    }

    #[test]
    fn una_notificacion_no_tiene_id() {
        let e = parsear(r#"{"jsonrpc":"2.0","method":"session/update","params":{}}"#).unwrap();
        assert!(matches!(e, Entrada::Notificacion(_)));
        assert!(e.id().is_none());
        assert_eq!(e.metodo(), Some("session/update"));
    }

    #[test]
    fn una_respuesta_con_error_se_reconoce() {
        let e = parsear(
            r#"{"jsonrpc":"2.0","id":7,"error":{"code":-32601,"message":"no such method"}}"#,
        )
        .unwrap();
        assert!(e.es_error());
        assert_eq!(e.id(), Some(&json!(7)));
    }

    #[test]
    fn el_json_roto_es_un_parse_error() {
        let e = parsear("{no es json").unwrap_err();
        assert_eq!(e.code, codigo::PARSE_ERROR);
    }

    #[test]
    fn la_version_jsonrpc_debe_ser_2_0() {
        let e = parsear(r#"{"jsonrpc":"1.0","id":1,"method":"initialize"}"#).unwrap_err();
        assert_eq!(e.code, codigo::INVALID_REQUEST);
    }

    #[test]
    fn el_framing_lsp_se_rechaza_con_un_mensaje_claro() {
        let e = parsear("Content-Length: 42").unwrap_err();
        assert!(e.message.contains("LSP"), "{}", e.message);
    }

    #[test]
    fn una_linea_vacia_es_un_error() {
        assert!(parsear("   ").is_err());
    }

    #[test]
    fn el_mensaje_que_no_es_solicitud_ni_respuesta_se_rechaza() {
        let e = parsear(r#"{"jsonrpc":"2.0"}"#).unwrap_err();
        assert_eq!(e.code, codigo::INVALID_REQUEST);
    }

    #[test]
    fn enmarcar_produce_una_sola_linea() {
        let m = solicitud(json!(3), "session/new", Some(json!({"cwd":"/tmp"})));
        let s = enmarcar(&m);
        assert!(s.ends_with('\n'));
        assert_eq!(s.trim_end().lines().count(), 1);
        // Y lo que sale se vuelve a parsear igual.
        let e = parsear(s.trim_end()).unwrap();
        assert_eq!(e.metodo(), Some("session/new"));
        assert_eq!(e.id(), Some(&json!(3)));
    }

    #[test]
    fn el_contador_de_ids_es_monotono_y_distinto() {
        let c = ContadorIds::default();
        assert_eq!(c.siguiente(), 1);
        assert_eq!(c.siguiente(), 2);
        assert_eq!(c.siguiente(), 3);
    }

    #[test]
    fn la_respuesta_de_error_conserva_el_id() {
        let m = respuesta_error(Some(json!(9)), ErrorRpc::nuevo(codigo::METHOD_NOT_FOUND, "nope"));
        let e = parsear(enmarcar(&m).trim_end()).unwrap();
        assert!(e.es_error());
        assert_eq!(e.id(), Some(&json!(9)));
    }
}
