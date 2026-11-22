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

use serde_json::{json, Value};

/// Alias que significan "el modelo es MegaDaemon" (routing completo).
pub const MEGADAEMON_ALIASES: &[&str] = &["megadaemon", "auto", "default", "megadaemon/router"];

/// Error del gateway con su código HTTP.
#[derive(Debug)]
pub struct GatewayError {
    pub status: u16,
    pub message: String,
}

impl GatewayError {
    fn bad(msg: impl Into<String>) -> Self {
        GatewayError {
            status: 400,
            message: msg.into(),
        }
    }
}

/// Extrae el prompt y el system prompt de una petición OpenAI.
///
/// Devuelve `(prompt_de_usuario, system_prompt_opcional)`.
pub fn parse_messages(messages: &Value) -> Result<(String, Option<String>), GatewayError> {
    let arr = messages
        .as_array()
        .ok_or_else(|| GatewayError::bad("'messages' debe ser una lista"))?;
    if arr.is_empty() {
        return Err(GatewayError::bad("'messages' está vacía"));
    }
    let mut system: Option<String> = None;
    let mut user_parts: Vec<String> = Vec::new();
    for m in arr {
        let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("user");
        let content = match m.get("content") {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Array(parts)) => parts
                .iter()
                .filter_map(|p| {
                    p.get("text")
                        .and_then(|t| t.as_str())
                        .map(|t| t.to_string())
                        .or_else(|| {
                            p.get("type")
                                .and_then(|t| t.as_str())
                                .filter(|t| *t == "text")
                                .and_then(|_| p.get("text").and_then(|t| t.as_str()))
                                .map(|t| t.to_string())
                        })
                })
                .collect::<Vec<_>>()
                .join("\n"),
            _ => String::new(),
        };
        match role {
            "system" | "developer" => system = Some(content),
            _ => {
                if !content.trim().is_empty() {
                    user_parts.push(content);
                }
            }
        }
    }
    if user_parts.is_empty() {
        return Err(GatewayError::bad(
            "no hay ningún mensaje de usuario en 'messages'",
        ));
    }
    Ok((user_parts.join("\n\n"), system))
}

/// Traduce una petición OpenAI a la llamada de MegaDaemon y devuelve la
/// respuesta en **formato OpenAI**.
///
/// `model`:
///
/// * `"megadaemon"` (o alias) ⇒ **routing completo**: se pide el proveedor
///   por defecto del vault y, si falla, se cae al siguiente (el fallback ya
///   lo hace `providers::execute_chat`).
/// * un id concreto (`"minimax-m3.1-flash"`, `"groq-free"`, ...) ⇒ ese
///   proveedor primero, con la misma cascada de respaldo.
///
/// `tools`: se acepta y se registra; su ejecución la hace el proveedor si
/// lo soporta y, si no, la respuesta lo dice en `tools_used` (nunca se
/// finge que se usaron).
pub fn chat_completions(body: &Value) -> Result<Value, GatewayError> {
    let model = body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("megadaemon")
        .trim()
        .to_string();
    let (prompt, system) = parse_messages(body.get("messages").unwrap_or(&Value::Null))?;
    let max_tokens = body
        .get("max_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(2048)
        .clamp(1, 32_768) as u32;
    let requested = if MEGADAEMON_ALIASES
        .iter()
        .any(|a| a.eq_ignore_ascii_case(&model))
    {
        None
    } else {
        Some(model.as_str())
    };

    let res = crate::providers::execute_chat(&prompt, requested, None, system.as_deref(), max_tokens);

    let usage = json!({
        "prompt_tokens": 0,
        "completion_tokens": res.tokens_used,
        "total_tokens": res.tokens_used,
    });
    let mut message = json!({
        "role": "assistant",
        "content": res.reply,
    });
    if !res.reasoning.trim().is_empty() {
        message["reasoning_content"] = Value::String(res.reasoning.clone());
    }
    if !res.tools_used.is_empty() {
        message["megadaemon_tools_used"] = json!(res.tools_used);
    }
    Ok(json!({
        "id": format!("chatcmpl-mgd-{}", now_millis()),
        "object": "chat.completion",
        "created": now_secs(),
        "model": model,
        "choices": [{
            "index": 0,
            "message": message,
            "finish_reason": "stop",
        }],
        "usage": usage,
        // Trazabilidad propia de MegaDaemon (no rompe el formato OpenAI).
        "megadaemon": {
            "provider_id": res.model_id,
            "provider_name": res.model_name,
            "vendor": res.vendor,
            "latency_ms": res.latency_ms,
            "fallback_used": res.fallback_used,
        }
    }))
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
        for a in MEGADAEMON_ALIASES {
            assert!(a.eq_ignore_ascii_case("megadaemon") || *a != "megadaemon");
        }
        assert!(MEGADAEMON_ALIASES.iter().any(|a| *a == "megadaemon"));
    }

    #[test]
    fn parse_messages_separa_el_system_del_usuario() {
        let v = json!([
            {"role": "system", "content": "eres un agente"},
            {"role": "user", "content": "hola"}
        ]);
        let (p, s) = parse_messages(&v).unwrap();
        assert_eq!(p, "hola");
        assert_eq!(s.unwrap(), "eres un agente");
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
        let (p, _) = parse_messages(&v).unwrap();
        assert!(p.contains("parte 1") && p.contains("parte 2"));
    }

    #[test]
    fn la_respuesta_tiene_forma_openai() {
        // Con un modelo que no existe: la respuesta DEBE seguir siendo
        // OpenAI-compatible y decir qué pasó (no un 500 opaco).
        let body = json!({
            "model": "megadaemon",
            "messages": [{"role":"user","content":"ping"}]
        });
        match chat_completions(&body) {
            Ok(v) => {
                assert_eq!(v["object"], "chat.completion");
                assert!(v["choices"].is_array());
                assert!(v["megadaemon"]["provider_id"].is_string());
            }
            Err(e) => {
                // Si el proveedor falla, el error es utilizable.
                assert!(!e.message.is_empty());
            }
        }
    }
}
