//! Gateway **OpenAI-compatible** de MegaDaemon: el "modelo" es SIEMPRE
//! MegaDaemon.
//!
//! IDEA (decidida por el usuario, 2026-11-22)
//! ----------------------------------------------------
//! * El cliente (Antigravity u otro) habla con **un** modelo: `megadaemon`.
//! * **MegaDaemon enruta**: elige proveedor, hace fallback real y devuelve
//!   la respuesta con la trazabilidad (qué proveedor atendió, si hubo
//!   fallback, latencia, tokens).
//! * **MegaDaemon potencia con herramientas**: las tools que llegue en el
//!   petición se pasan al proveedor cuando éste las soporta, y las que
//!   usa quedan en `tools_used`.
//! * **MegaDaemon se auto-actualiza** (ver `self_update`).
//!
//! POR QUÉ EXISTE
//! -------------
//! * `providers::execute_chat` YA hace la cascada (pedido → otros cloud
//!   habilitados → SEERE local) con **fallback real** y presupuesto.
//! * Lo que faltaba era la **envoltura HTTP** en formato OpenAI, para que
//!   cualquier cliente (la GUI de Antigravity entre otros) pueda apuntar a
//!   `http://127.0.0.1:9090/v1/chat/completions` y hablar con **toda** la
//!   flota (modelos + herramientas) en vez de con un proveedor suelto.
//!
//! MEDIDO 2026-11-22 (móvil S21 Ultra, build v13.1):
//! * `megadaemon-core chat minimax-m3.1-flash "..."` ⇒ `{"vendor":"MiniMax",
//!   "latency_ms":1232, "fallback_used":false, "reply":"MINIMAX_OK"}` ⇒ el
//!   camino a MiniMax funciona y **no** hay fallback.
//! * La GUI de Antigravity (language server en `127.0.0.1:45157`, gRPC) NO
//!   puede llamar a un proveedor externo directamente: su catálogo de
//!   modelos es cerrado y sus ids vienen ofuscados (`MODEL_PLACEHOLDER_*`).
//!   Un BYOK que apunte a este gateway es la vía limpia.
//!
//! v16 —endurecimiento (AUDIT_LLM_V16)
//! ------------------------------------
//! * La respuesta tiene **EXACTAMENTE** la forma OpenAI (`id`, `object`,
//!   `created`, `model`, `choices`, `usage`).  La trazabilidad de MegaDaemon
//!   va en `"megadaemon"` **sólo si el cliente la pide**
//!   (`"include_trace": true`), y el razonamiento **sólo** si pide
//!   `"include_reasoning": true` (antes el CoT del modelo sefiltraba al
//!   cliente en el mensaje).
//! * `messages` vacíos / inválidos ⇒ **400** (no 200 con cuerpo de error).
//! * `model: "megadaemon"` ⇒ routing COMPLETO: **no** se llama a un
//!   proveedor concreto, se usa el modelo por defecto de la boveda y su
//!   cascada.
//! * Todo el enrutado es **inyectable** (`chat_completions_with`) para que
//!   la suite sea determinista y **sin red**.

use serde_json::{json, Value};

use crate::providers::{ChatPolicy, ChatRequest, LlmTransport, LocalFallback, SeereLocal, UreqTransport};

/// Alias que significan "el modelo es MegaDaemon" (routing completo).
pub const MEGADAEMON_ALIASES: &[&str] = &["megadaemon", "auto", "default", "megadaemon/router"];

/// Error del gateway con su código HTTP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayError {
    pub status: u16,
    pub message: String,
    /// Etiqueta estable (`invalid_request`, `upstream`, …).
    pub kind: String,
}

impl GatewayError {
    /// Error 400 (petición inválida del cliente).
    pub fn bad(msg: impl Into<String>) -> Self {
        GatewayError {
            status: 400,
            message: msg.into(),
            kind: "invalid_request_error".to_string(),
        }
    }

    /// Error 502 (el proveedor falló).
    pub fn upstream(msg: impl Into<String>) -> Self {
        GatewayError {
            status: 502,
            message: msg.into(),
            kind: "upstream_error".to_string(),
        }
    }

    /// Cuerpo JSON con la forma de error de OpenAI.
    pub fn to_json(&self) -> Value {
        json!({
            "error": {
                "message": self.message,
                "type": self.kind,
                "code": self.status,
            }
        })
    }
}

impl std::fmt::Display for GatewayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({}): {}", self.status, self.kind, self.message)
    }
}

/// Conversación parseada de una petición OpenAI.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedConversation {
    /// Contenido del mensaje `system`/`developer` (si lo hay).
    pub system: Option<String>,
    /// Mensajes listos para enviarse al proveedor (roles intactos).
    pub messages: Vec<Value>,
    /// Todo el texto del usuario concatenado (para las APIs de prompt único).
    pub user_text: String,
}

/// Extrae la conversación de una petición OpenAI.
///
/// Acepta:
/// * `content` como **string**;
/// * `content` como **array** de partes (`{"type":"text","text":"…"}`);
/// * roles `system`, `developer`, `user` y `assistant`.
///
/// v16: antes el assistant se concatenaba con el user y el modelo recibía
/// su propia respuesta anterior como si la hubiera dicho el usuario.
pub fn parse_messages(messages: &Value) -> Result<ParsedConversation, GatewayError> {
    let arr = messages
        .as_array()
        .ok_or_else(|| GatewayError::bad("'messages' debe ser una lista"))?;
    if arr.is_empty() {
        return Err(GatewayError::bad("'messages' esta vacia"));
    }
    let mut system: Option<String> = None;
    let mut out: Vec<Value> = Vec::new();
    let mut user_parts: Vec<String> = Vec::new();
    for m in arr {
        let role = m
            .get("role")
            .and_then(|r| r.as_str())
            .unwrap_or("user")
            .trim()
            .to_lowercase();
        let content = content_to_text(m.get("content"));
        match role.as_str() {
            "system" | "developer" => {
                if !content.trim().is_empty() {
                    system = Some(content.clone());
                    out.push(json!({"role": "system", "content": content}));
                }
            }
            "user" | "assistant" | "tool" | "function" => {
                if content.trim().is_empty() {
                    continue;
                }
                if role == "user" {
                    user_parts.push(content.clone());
                }
                let r = if role == "tool" || role == "function" {
                    "user".to_string()
                } else {
                    role.clone()
                };
                out.push(json!({"role": r, "content": content}));
            }
            // Rol desconocido: se trata como usuario (como antes).
            _ => {
                if !content.trim().is_empty() {
                    user_parts.push(content.clone());
                    out.push(json!({"role": "user", "content": content}));
                }
            }
        }
    }
    if out.is_empty() || user_parts.is_empty() {
        return Err(GatewayError::bad(
            "no hay ningun mensaje de usuario en 'messages'",
        ));
    }
    Ok(ParsedConversation {
        system,
        messages: out,
        user_text: user_parts.join("\n\n"),
    })
}

/// Convierte el `content` de un mensaje a texto plano.
///
/// Acepta string y array de partes (multimodal).
pub fn content_to_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|p| {
                if let Some(t) = p.get("text").and_then(|t| t.as_str()) {
                    Some(t.to_string())
                } else if p.get("type").and_then(|t| t.as_str()) == Some("text") {
                    p.get("text").and_then(|t| t.as_str()).map(|t| t.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Traduce una petición OpenAI a la llamada de MegaDaemon y devuelve la
/// respuesta en **formato OpenAI**.
///
/// `model`:
///
/// * `"megadaemon"` (o alias) ⇒ **routing completo**: se pide el proveedor
///   por defecto del vault y, si falla, se cae al siguiente (el fallback ya
///   lo hace `providers::execute_chat`).  **No** se llama a un proveedor
///   concreto.
/// * un id concreto (`"minimax-m3.1-flash"`, `"groq-free"`, ...) ⇒ ese
///   proveedor primero, con la misma cascada de respaldo.
///
/// `tools`: se acepta y se registra; su ejecución la hace el proveedor si
/// lo soporta y, si no, la respuesta lo dice en `tools_used` (nunca se
/// finge que se usaron).
pub fn chat_completions(body: &Value) -> Result<Value, GatewayError> {
    chat_completions_with(
        body,
        &UreqTransport::default(),
        &SeereLocal,
        &ChatPolicy::default(),
    )
}

/// Igual que [`chat_completions`] pero con el transporte, el motor local y
/// la política **inyectados** (para la suite offline).
pub fn chat_completions_with(
    body: &Value,
    transport: &dyn LlmTransport,
    local: &dyn LocalFallback,
    policy: &ChatPolicy,
) -> Result<Value, GatewayError> {
    let vault = crate::providers::load_vault();
    chat_completions_with_vault(body, &vault, transport, local, policy)
}

/// Igual que [`chat_completions_with`] pero con la **boveda** inyectada
/// (para la suite: sin tocar el disco y con proveedores sintéticos).
pub fn chat_completions_with_vault(
    body: &Value,
    vault: &crate::providers::ProviderVault,
    transport: &dyn LlmTransport,
    local: &dyn LocalFallback,
    policy: &ChatPolicy,
) -> Result<Value, GatewayError> {
    let model = body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("megadaemon")
        .trim()
        .to_string();
    let conv = parse_messages(body.get("messages").unwrap_or(&Value::Null))?;
    let max_tokens = body
        .get("max_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let include_trace = body
        .get("include_trace")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let include_reasoning = body
        .get("include_reasoning")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // v16: `"megadaemon"` (y sus alias) ⇒ routing COMPLETO.  No se pide un
    // proveedor concreto: se deja que la cascada decida.
    let requested = if MEGADAEMON_ALIASES
        .iter()
        .any(|a| a.eq_ignore_ascii_case(&model))
    {
        None
    } else {
        Some(model.clone())
    };

    let req = ChatRequest {
        prompt: conv.user_text.clone(),
        requested_model: requested,
        attachment_path: None,
        system: conv.system.clone(),
        max_tokens,
        messages: conv.messages.clone(),
    };

    let res = crate::providers::execute_chat_in(vault, transport, local, policy, &req);
    let prompt_tokens = if res.prompt_tokens > 0 {
        res.prompt_tokens
    } else {
        // Estimación honesta (chars/4): el proveedor no siempre lo dice.
        ((conv.user_text.chars().count() as u64) / 4).max(1)
    };
    let completion_tokens = if res.completion_tokens > 0 {
        res.completion_tokens
    } else {
        res.tokens_used.saturating_sub(prompt_tokens)
    };
    let total_tokens = if res.tokens_used > 0 {
        res.tokens_used
    } else {
        prompt_tokens + completion_tokens
    };

    let mut message = json!({
        "role": "assistant",
        "content": res.reply,
    });
    if include_reasoning && !res.reasoning.trim().is_empty() {
        message["reasoning_content"] = Value::String(res.reasoning.clone());
    }
    if include_trace && !res.tools_used.is_empty() {
        message["megadaemon_tools_used"] = json!(res.tools_used);
    }

    let mut out = json!({
        "id": format!("chatcmpl-mgd-{}", now_millis()),
        "object": "chat.completion",
        "created": now_secs(),
        "model": model,
        "choices": [{
            "index": 0,
            "message": message,
            "finish_reason": "stop",
        }],
        "usage": {
            "prompt_tokens": prompt_tokens,
            "completion_tokens": completion_tokens,
            "total_tokens": total_tokens,
        },
    });

    if include_trace {
        // Trazabilidad propia de MegaDaemon (opt-in: por defecto la
        // respuesta es OpenAI *pura*).
        out["megadaemon"] = json!({
            "provider_id": res.model_id,
            "provider_name": res.model_name,
            "vendor": res.vendor,
            "latency_ms": res.latency_ms,
            "fallback_used": res.fallback_used,
            "tools_used": res.tools_used,
        });
    }

    Ok(out)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_de_megadaemon_son_los_esperados() {
        assert!(MEGADAEMON_ALIASES.iter().any(|a| *a == "megadaemon"));
        assert!(MEGADAEMON_ALIASES.iter().any(|a| *a == "auto"));
    }

    #[test]
    fn parse_messages_separa_el_system_del_usuario() {
        let v = json!([
            {"role": "system", "content": "eres un agente"},
            {"role": "user", "content": "hola"}
        ]);
        let c = parse_messages(&v).unwrap();
        assert_eq!(c.user_text, "hola");
        assert_eq!(c.system.unwrap(), "eres un agente");
    }

    #[test]
    fn parse_messages_rechaza_listas_vacias_o_invalidas() {
        assert!(parse_messages(&json!([])).is_err());
        assert!(parse_messages(&json!("nope")).is_err());
        assert!(parse_messages(&json!([{"role":"system","content":"x"}])).is_err());
    }

    #[test]
    fn parse_messages_acepta_contenido_multimodal() {
        let v = json!([{"role":"user","content":[
            {"type":"text","text":"parte 1"},
            {"type":"text","text":"parte 2"}
        ]}]);
        let c = parse_messages(&v).unwrap();
        assert!(c.user_text.contains("parte 1") && c.user_text.contains("parte 2"));
    }

    #[test]
    fn parse_messages_conserva_el_rol_assistant() {
        let v = json!([
            {"role": "user", "content": "hola"},
            {"role": "assistant", "content": "buenas"},
            {"role": "user", "content": "mas"}
        ]);
        let c = parse_messages(&v).unwrap();
        assert_eq!(c.messages.len(), 3);
        assert_eq!(c.messages[1]["role"], "assistant");
        assert_eq!(c.user_text, "hola\n\nmas");
    }
}
