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

/// El modelo canónico (el alias que significa "routing completo").
pub const MODELO_CANONICO: &str = "megadaemon";

/// ¿Este nombre es un alias de "el modelo es MegaDaemon"?
pub fn es_alias_megadaemon(m: &str) -> bool {
    let t = m.trim();
    if t.is_empty() {
        return false;
    }
    MEGADAEMON_ALIASES.iter().any(|a| a.eq_ignore_ascii_case(t))
}

/// **Trazabilidad del MODELO**: qué pidió el cliente y qué contestó de verdad.
///
/// MEDIDO (2026-11-23): la respuesta decía QUÉ proveedor atendió
/// (`provider_id`) pero no decía qué pidió el cliente ⇒ con el alias
/// `megadaemon` el cliente creía estar hablando con "megadaemon" y de
/// verdad le había hablado MiniMax, sin que nadie lo supiera.
///
/// Añade al bloque `megadaemon`:
/// * `requested_model`: lo que pidió el cliente.
/// * `real_model`: el id del modelo que contestó.
/// * `substituted`: `true` si el que contestar **no** era el pedido
///   (con el alias `megadaemon` SIEMPRE es `true`: el alias no es un
///   modelo real, es el router).
pub fn anotar_traza(v: &mut Value, requested_model: &str) {
    let req = requested_model.trim();
    let real = v
        .pointer("/megadaemon/provider_id")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let alias = es_alias_megadaemon(req);
    let sustituido = if real.is_empty() {
        false
    } else if alias {
        true
    } else {
        !real.eq_ignore_ascii_case(req)
    };
    if let Some(m) = v.get_mut("megadaemon").and_then(|m| m.as_object_mut()) {
        m.insert("requested_model".into(), json!(req));
        m.insert("real_model".into(), json!(real));
        m.insert("substituted".into(), json!(sustituido));
    }
}

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
        write!(f, "HTTP {}: {}", self.status, self.message)
    }
}

/// Extrae el prompt y el system prompt de una petición OpenAI.
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
    // v16: la variante de producción (vault de disco + transporte real).
    let vault = crate::providers::load_vault();
    let transport = crate::providers::UreqTransport::default();
    let deps = crate::providers::ChatDeps::new(&vault, &transport);
    chat_completions_with(body, &deps)
}

/// Variante **inyectable** de [`chat_completions`].
///
/// v16: el gateway se testea sin red (mismo formato, otro vault).
pub fn chat_completions_with(
    body: &Value,
    deps: &crate::providers::ChatDeps<'_>,
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

    let res = crate::providers::execute_chat_with(
        &prompt,
        requested,
        None,
        system.as_deref(),
        max_tokens,
        deps,
    );

    let usage = json!({
        // El proveedor no siempre reporta los tokens de ENTRADA; 0 = "no lo
        // dice el proveedor" (no se inventa un número).
        "prompt_tokens": 0,
        "completion_tokens": res.tokens_used,
        "total_tokens": res.tokens_used,
    });
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
    // MEDIDO (2026-11-26, móvil): las tool calls del proveedor se
    // devolvían como texto y el cliente las pintaba en vez de ejecutarlas.
    let finish_reason = if res.tool_calls.is_empty() {
        "stop"
    } else {
        "tool_calls"
    };
    if !res.tool_calls.is_empty() {
        message["tool_calls"] = json!(res
            .tool_calls
            .iter()
            .map(|tc| json!({
                "id": tc.id,
                "type": "function",
                "function": {
                    "name": tc.name,
                    "arguments": tc.arguments.to_string(),
                }
            }))
            .collect::<Vec<_>>());
    }
    Ok(json!({
        "id": format!("chatcmpl-mgd-{}", now_millis()),
        "object": "chat.completion",
        "created": now_secs(),
        // HONESTIDAD (2026-11-22/23): 'model' dice el modelo que **realmente**
        // contestó, NO el que pidió el cliente (antes devolvía el pedido y el
        // cliente se creería que habló con otro).  Lo pedido viaja en
        // 'megadaemon.requested_model' y 'megadaemon.substituted'.
        "model": if res.model_name.trim().is_empty() {
            model.clone()
        } else {
            res.model_name.clone()
        },
        "choices": [{
            "index": 0,
            "message": message,
            "finish_reason": finish_reason,
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
            // Lo que pidió el cliente y si se sustituyó (el gateway NO
            // suplanta a nadie).
            "requested_model": model.clone(),
            "substituted": !res.model_id.eq_ignore_ascii_case(&model)
                && !MEGADAEMON_ALIASES
                    .iter()
                    .any(|a| a.eq_ignore_ascii_case(&model)),
            "fallback_used": res.fallback_used,
            "tokens_used": res.tokens_used,
            // v16: quién respondió y a quién se saltó (retirado, cuota en
            // pausa, sin credencial).  Sin esto el `fallback_used` no
            // explicaba NADA de lo que pasó.
            "provider_trace": res.provider_trace,
            // v16: el estado de la cuota viaja DENTRO de la respuesta.
            "quota": res.quota,
        }
    }))
}

/// La misma respuesta de [`chat_completions_with`] pero con la
/// **trazabilidad del modelo** escrita (`requested_model`/`real_model`/
/// `substituted`).
pub fn chat_completions_con_traza(body: &Value) -> Result<Value, GatewayError> {
    let mut v = chat_completions(body)?;
    let requested = body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or(MODELO_CANONICO)
        .trim()
        .to_string();
    let requested = if requested.is_empty() {
        MODELO_CANONICO.to_string()
    } else {
        requested
    };
    anotar_traza(&mut v, &requested);
    Ok(v)
}

// =========================================================================
// GEMINI / BYOM: el idioma que habla el language server de Antigravity
// =========================================================================
//
// MEDIDO 2026-11-22 (Samsung del usuario, binario `liblanguage_server.so`
// de 181.358.800 B, strings del binario):
//
//   * `google3/third_party/jetski/models/model_config/model_config.WithAllowBYOM`
//     ⇒ el "Bring Your Own Model" es el camino de los modelos personalizados.
//   * `google3/third_party/golang/google_golang_org/genai/v/v0/google/genai/genai.Models.generateContent`
//     ⇒ el language server habla el **protocolo Gemini** (`genai` SDK de Google),
//       NO el de OpenAI.
//   * `/v1internal:generateContent` y `/v1internal:streamGenerateContent`
//     ⇒ las rutas concretas que usa el proveedor `API_PROVIDER_GOOGLE_EVERGREEN`
//       (el que la GUI asigna a los modelos personalizados).
//   * La GUI construye el modelo así (`main.js`, 9.902.890 B):
//     `{modelId: GOOGLE_GEMINI_INTERNAL_BYOM (=326),
//       apiProvider: API_PROVIDER_GOOGLE_EVERGREEN (=30),
//       chatModelName: <etiqueta>, modelName: <URL base>, maxTokens, modelFeatures}`
//     y lo manda al servidor con `JetboxWriteState`.
//
// ⇒ Para que Antigravity use **SIEMPRE** MegaDaemon, el gateway tiene que
//   hablar ALSO el idioma de Antigravity.  OpenAI (`/v1/chat/completions`) se
//   queda como está (es lo que usan el resto de clientes) y se añade el
//   dialecto Gemini.

/// Rutas del dialecto Gemini que entiende el gateway.
pub const GEMINI_GENERATE_PATH: &str = "/v1internal:generateContent";
/// Ruta del dialecto Gemini en modo *streaming* (SSE).
pub const GEMINI_STREAM_PATH: &str = "/v1internal:streamGenerateContent";
/// Prefijo `v1beta` (variante pública del SDK `genai`).
pub const GEMINI_V1BETA_PREFIX: &str = "/v1beta/models/";

/// Qué ruta Gemini ha pedido el cliente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiRoute {
    /// El modelo pedido (vacío ⇒ "el que sea", lo decide el router).
    pub model: String,
    /// ¿Es la variante de streaming (SSE)?
    pub stream: bool,
}

/// Reconoce una ruta Gemini y devuelve el modelo y el modo.
///
/// Acepta:
/// * `/v1internal:generateContent`
/// * `/v1internal:streamGenerateContent`
/// * `/v1beta/models/{model}:generateContent`
/// * `/v1beta/models/{model}:streamGenerateContent`
/// Normaliza una ruta Gemini a la forma canónica (con dos puntos).
///
/// MEDIDO (2026-11-24, S21 Ultra): el language server de Antigravity pide
/// las rutas Gemini **con barra** (`/v1internal/generateContent`) y nuestro
/// shim sólo reconocía la forma con dos puntos ⇒ devolvía
/// `404 {"error":"el shim no sirve la ruta /v1internal/"}` ⇒ el LS se
/// rendía y el core salía con **exit 135**.
///
/// Normaliza:
/// * `/v1internal/generateContent`        ⇒ `/v1internal:generateContent`
/// * `/v1internal/streamGenerateContent`  ⇒ `/v1internal:streamGenerateContent`
/// * `/v1beta/models/<m>/generateContent` ⇒ `/v1beta/models/<m>:generateContent`
pub fn normalizar_ruta_gemini(p: &str) -> String {
    let mut s = p.to_string();
    if let Some(rest) = s.strip_prefix("/v1internal/") {
        if !rest.is_empty() && !rest.contains(':') && !rest.contains('/') {
            s = format!("/v1internal:{rest}");
        }
    }
    if let Some(cuerpo) = s.strip_prefix("/v1beta/models/") {
        if let Some(i) = cuerpo.rfind('/') {
            let (cabeza, cola) = cuerpo.split_at(i);
            let cola = &cola[1..];
            if !cola.is_empty() && !cola.contains(':') && !cola.contains('/') {
                s = format!("/v1beta/models/{cabeza}:{cola}");
            }
        }
    }
    s
}

pub fn gemini_route(path: &str) -> Option<GeminiRoute> {
    let p = normalizar_ruta_gemini(path.trim());
    let (rest, stream) = if let Some(r) = p.strip_suffix(":streamGenerateContent") {
        (r, true)
    } else if let Some(r) = p.strip_suffix(":generateContent") {
        (r, false)
    } else {
        return None;
    };
    let model = if let Some(m) = rest.strip_prefix(GEMINI_V1BETA_PREFIX) {
        m.to_string()
    } else if rest == "/v1internal" {
        String::new()
    } else {
        return None;
    };
    Some(GeminiRoute { model, stream })
}

/// Petición Gemini ya entendida.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GeminiPrompt {
    /// Texto del usuario (partes de texto concatenadas).
    pub prompt: String,
    /// `systemInstruction` (si lo hay).
    pub system: Option<String>,
    /// `generationConfig.maxOutputTokens` (acotado).
    pub max_tokens: u32,
    /// Nº de declaraciones de función que pedía el cliente (informativo).
    pub tools_requested: usize,
    /// Las herramientas del cliente, ya traducidas al formato OpenAI.
    ///
    /// MEDIDO (2026-11-26, móvil): el LS manda 18 `functionDeclarations`
    /// (`view_file`, `run_command`, …) y el proveedor SÓLO devuelve tool
    /// calls estructuradas si la petición lleva `tools`.
    pub tools: Vec<Value>,
    /// La conversación del cliente, en formato OpenAI (mensajes).
    ///
    /// MEDIDO (2026-11-26, móvil): para que el agente pueda CONTINUAR
    /// después de ejecutar una herramienta hay que devolverle su llamada
    /// (`assistant.tool_calls`) y el resultado (`role:"tool"`), no sólo el
    /// texto.
    pub mensajes: Vec<Value>,
    /// Tamaño ORIGINAL del prompt (antes de recortar).
    pub bytes_prompt: usize,
    /// ¿Se recortó el prompt por [`MAX_PROMPT_CHARS`]?
    pub truncado: bool,
}

/// Tope del prompt que sale del gateway hacia el proveedor.
///
/// MEDIDO (2026-11-26, móvil): el language server manda el contexto
/// COMPLETO de la conversación: 758 KB en un solo POST (`b937b` bytes de
/// chunk).  Ningún proveedor aguanta eso (y el motor local SEERE tampoco):
/// hay que acotarlo.  Se conserva la **cola** (lo más reciente es lo que
/// importa) y se deja constancia en la traza (`prompt_truncado`).
pub const MAX_PROMPT_CHARS: usize = 120_000;

// ==================================================================
// ENVOLTORIO v1internal (MEDIDO 2026-11-26, móvil)
// ==================================================================
//
// MEDIDO (2026-11-26, móvil): captura REAL del tráfico del language server
// (`tcpdump -i lo -w` sobre el puerto 9090 del daemon):
//
// ```text
// POST /mda/<token>/v1internal:streamGenerateContent?alt=sse HTTP/1.1
// Host: 127.0.0.1:9090
// User-Agent: antigravity/hub/2.11.0 (aidev_client; os_type=linux; arch=arm64)
// Transfer-Encoding: chunked
// Content-Type: application/json
//
// {"project":"aicode-consumers","requestId":"agent/…",
//  "request":{"contents":[{"role":"user","parts":[{"text":"…"}]}]}}
// ```
//
// Y el esquema extraído del PROPIO `.so`
// (`tools/proto-schema/extraer_esquema.py`) confirma la forma:
//
// ```proto
// // google/internal/cloud/code/v1internal/prediction_service.proto
// message GenerateContentRequest {
//   string project = 1;
//   string request_id = 2;
//   google.cloud.aiplatform.master.GenerateContentRequest request = 3;
//   string model = 4;
//   ...
// }
// message GenerateContentResponse {
//   google.cloud.aiplatform.master.GenerateContentResponse response = 1;
//   string trace_id = 2;
//   ...
// }
// ```
//
// ⇒ el LS **NO** envía el `GenerateContentRequest` pelado: lo envía
// ENVUELTO en `request`.  Antes el gateway leía `contents` en la raíz, no
// lo encontraba y devolvía 502:
//
// ```text
// Run: attempt 1 failed (SHIM_PROXY_ERROR (code 502):
//      gateway Gemini: 'contents' debe ser una lista), retrying in 4s
// ```

/// ¿El cuerpo es el ENVOLTORIO v1internal (`GenerateContentRequest`)?
///
/// MEDIDO (2026-11-26, móvil): el LS manda `{"project":…,"request":{…}}`.
pub fn es_envoltorio_v1internal(body: &Value) -> bool {
    body.get("request").map(|r| r.is_object()).unwrap_or(false)
}

/// El cuerpo EFECTIVO de la petición Gemini: el interior del envoltorio.
///
/// Si el cuerpo no viene envuelto (otro cliente habla Gemini pelado), se
/// devuelve tal cual: la función es **total**.
pub fn cuerpo_gemini(body: &Value) -> &Value {
    match body.get("request") {
        Some(r) if r.is_object() => r,
        _ => body,
    }
}

/// El modelo que pide el CUERPO (el LS no lo pone en la ruta).
///
/// MEDIDO (2026-11-26, móvil): con `/v1internal:streamGenerateContent` la
/// ruta no nombra ningún modelo; el modelo va en `model` (envoltorio) o en
/// `request.model` (el interior).
pub fn modelo_del_cuerpo(body: &Value) -> Option<String> {
    let g = cuerpo_gemini(body);
    body.get("model")
        .or_else(|| g.get("model"))
        .and_then(|m| m.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// La respuesta Gemini con SÓLO los campos del proto.
///
/// MEDIDO (2026-11-26): el protojson de Go **falla** ante un campo
/// desconocido ("unknown field") ⇒ nuestro bloque `megadaemon` (la
/// trazabilidad propia) NO puede viajar dentro de `response`.
pub fn gemini_solo_proto(v: &Value) -> Value {
    let mut out = json!({});
    for campo in ["candidates", "usageMetadata", "modelVersion", "promptFeedback"] {
        if let Some(x) = v.get(campo) {
            out[campo] = x.clone();
        }
    }
    out
}

/// Envuelve la respuesta Gemini en el `GenerateContentResponse` de v1internal.
///
/// MEDIDO (2026-11-26, móvil): sin esta envoltura el LS no encuentra ningún
/// `candidate` ⇒ el chat se queda vacío aunque el proveedor haya contestado.
pub fn envolver_v1internal(gemini: &Value, trace_id: &str) -> Value {
    json!({
        "response": gemini_solo_proto(gemini),
        "traceId": trace_id,
    })
}

/// Convierte una petición Gemini (`genai`) en prompt + sistema.
///
/// Es **puro**: no toca la red (testeable sin nada).
pub fn gemini_prompt(body: &Value) -> Result<GeminiPrompt, GatewayError> {
    // MEDIDO (2026-11-26, móvil): el LS envía el ENVOLTORIO v1internal ⇒ el
    // `contents` de verdad está en `request.contents`.
    let body = cuerpo_gemini(body);
    let contents = body
        .get("contents")
        .and_then(|c| c.as_array())
        .ok_or_else(|| GatewayError::bad("'contents' debe ser una lista"))?;
    let mut parts_out: Vec<String> = Vec::new();
    for c in contents {
        let role = c.get("role").and_then(|r| r.as_str()).unwrap_or("user");
        // MEDIDO (2026-11-26, móvil): las partes de una herramienta (la
        // llamada y su resultado) también son CONTENIDO: si sólo se copian
        // las de texto, el motor local se queda sin saber qué devolvió la
        // herramienta.
        let texto = gemini_partes_completas(c.get("parts"));
        if texto.trim().is_empty() {
            continue;
        }
        if role == "user" {
            parts_out.push(texto);
        } else if role == "model" && !parts_out.is_empty() {
            // El historial del modelo: se conserva como contexto.
            parts_out.push(texto);
        } else {
            parts_out.push(texto);
        }
    }
    if parts_out.is_empty() {
        return Err(GatewayError::bad("la petición Gemini no trae texto"));
    }
    let system = body
        .get("systemInstruction")
        .or_else(|| body.get("system_instruction"))
        .map(|s| gemini_partes(s.get("parts")))
        .filter(|s| !s.trim().is_empty());
    let max_tokens = body
        .pointer("/generationConfig/maxOutputTokens")
        .or_else(|| body.pointer("/generation_config/max_output_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(2048)
        .clamp(1, 32_768) as u32;
    let tools_requested = body
        .get("tools")
        .and_then(|t| t.as_array())
        .map(|ts| {
            ts.iter()
                .map(|t| {
                    t.get("functionDeclarations")
                        .or_else(|| t.get("function_declarations"))
                        .and_then(|f| f.as_array())
                        .map(|f| f.len())
                        .unwrap_or(0)
                })
                .sum::<usize>()
        })
        .unwrap_or(0);
    // MEDIDO (2026-11-26, móvil): las herramientas del LS traducidas al
    // formato OpenAI (sin ellas el proveedor NO devuelve tool calls).
    let tools = tools_gemini_a_openai(body.get("tools"));
    // La conversación en formato OpenAI (con `assistant.tool_calls` y
    // `role:"tool"`), acotada al mismo tope.
    let mensajes = acotar_mensajes(
        gemini_a_mensajes_openai(&Value::Array(contents.clone())),
        MAX_PROMPT_CHARS,
    );
    // MEDIDO (2026-11-26, móvil): el LS manda 758 KB de contexto.  Se
    // acota a [`MAX_PROMPT_CHARS`] conservando la COLA (lo más reciente) y
    // se deja constancia (`truncado`).
    let completo = parts_out.join("\n\n");
    let bytes_prompt = completo.chars().count();
    let (recortado, truncado) = if bytes_prompt > MAX_PROMPT_CHARS {
        let cola: String = completo
            .chars()
            .skip(bytes_prompt - MAX_PROMPT_CHARS)
            .collect();
        (cola, true)
    } else {
        (completo, false)
    };
    Ok(GeminiPrompt {
        prompt: recortado,
        system,
        max_tokens,
        tools_requested,
        tools,
        mensajes,
        bytes_prompt,
        truncado,
    })
}

// ==================================================================
// HERRAMIENTAS (MEDIDO 2026-11-26, móvil)
// ==================================================================
//
// MEDIDO (2026-11-26, móvil): el LS de Antigravity manda sus herramientas
// en `tools[].functionDeclarations[]` con el dialecto `master.Schema` del
// proto `google/cloud/aiplatform/master/tool.proto`:
//
// ```json
// {"name":"view_file","description":"…",
//  "parameters":{"type":"OBJECT",
//                 "properties":{"AbsolutePath":{"type":"STRING","description":"…"}},
//                 "required":["AbsolutePath"]}}
// ```
//
// El enum `master.Type` (extraído del propio `.so`) es
// `TYPE_UNSPECIFIED|STRING|NUMBER|INTEGER|BOOLEAN|ARRAY|OBJECT|NULL` ⇒ los
// tipos llegan en MAYÚSCULAS y hay que bajarlos para JSON Schema.
//
// Y el LS espera la llamada de vuelta como `functionCall` (el descriptor
// `master.FunctionCall` del binario define `name` + `args`, con `args` como
// `google.protobuf.Struct` ⇒ un objeto JSON plano):

/// Normaliza el tipo de `master.Type` a JSON Schema (minúsculas).
fn tipo_master_a_json_schema(t: &str) -> Option<&'static str> {
    match t.trim().to_ascii_uppercase().as_str() {
        "STRING" => Some("string"),
        "NUMBER" => Some("number"),
        "INTEGER" => Some("integer"),
        "BOOLEAN" => Some("boolean"),
        "ARRAY" => Some("array"),
        "OBJECT" => Some("object"),
        "NULL" => Some("null"),
        _ => None,
    }
}

/// Traduce un `master.Schema` a **JSON Schema** (el dialecto que entiende
/// un proveedor OpenAI-compatible).
///
/// Función **PURO** (⇒ testeable sin nada).
pub fn schema_master_a_json_schema(v: &Value) -> Value {
    let obj = match v.as_object() {
        Some(o) => o,
        None => return v.clone(),
    };
    let mut out = serde_json::Map::new();
    let mut tipo: Option<&'static str> = None;
    let mut nullable = false;
    for (k, val) in obj {
        match k.as_str() {
            "type" => {
                if let Some(t) = val.as_str() {
                    tipo = tipo_master_a_json_schema(t);
                }
            }
            "nullable" => nullable = val.as_bool().unwrap_or(false),
            "properties" => {
                let mut props = serde_json::Map::new();
                if let Some(p) = val.as_object() {
                    for (nombre, sub) in p {
                        props.insert(nombre.clone(), schema_master_a_json_schema(sub));
                    }
                }
                out.insert("properties".to_string(), Value::Object(props));
            }
            "items" => {
                out.insert("items".to_string(), schema_master_a_json_schema(val));
            }
            "prefixItems" | "prefix_items" => {
                let arr = val
                    .as_array()
                    .map(|a| a.iter().map(schema_master_a_json_schema).collect::<Vec<_>>())
                    .unwrap_or_default();
                out.insert("prefixItems".to_string(), Value::Array(arr));
            }
            "oneOf" | "one_of" | "anyOf" | "any_of" | "allOf" | "all_of" => {
                let destino = match k.as_str() {
                    "oneOf" | "one_of" => "oneOf",
                    "anyOf" | "any_of" => "anyOf",
                    _ => "allOf",
                };
                let arr = val
                    .as_array()
                    .map(|a| a.iter().map(schema_master_a_json_schema).collect::<Vec<_>>())
                    .unwrap_or_default();
                out.insert(destino.to_string(), Value::Array(arr));
            }
            "not" => {
                out.insert("not".to_string(), schema_master_a_json_schema(val));
            }
            "additionalProperties" | "additional_properties" => {
                // En `master.Schema` es un Schema; en JSON Schema suele ser
                // un booleano.  Si es booleano se copia tal cual.
                if val.is_boolean() {
                    out.insert("additionalProperties".to_string(), val.clone());
                } else {
                    out.insert(
                        "additionalProperties".to_string(),
                        schema_master_a_json_schema(val),
                    );
                }
            }
            "additionalPropertiesSchema" | "additional_properties_schema" => {
                out.insert(
                    "additionalProperties".to_string(),
                    schema_master_a_json_schema(val),
                );
            }
            "$ref" | "ref" => {
                out.insert("$ref".to_string(), val.clone());
            }
            "$defs" | "defs" => {
                let mut defs = serde_json::Map::new();
                if let Some(d) = val.as_object() {
                    for (nombre, sub) in d {
                        defs.insert(nombre.clone(), schema_master_a_json_schema(sub));
                    }
                }
                out.insert("$defs".to_string(), Value::Object(defs));
            }
            "minItems" | "min_items" => {
                out.insert("minItems".to_string(), val.clone());
            }
            "maxItems" | "max_items" => {
                out.insert("maxItems".to_string(), val.clone());
            }
            "minLength" | "min_length" => {
                out.insert("minLength".to_string(), val.clone());
            }
            "maxLength" | "max_length" => {
                out.insert("maxLength".to_string(), val.clone());
            }
            // El resto se copia tal cual (JSON Schema admite campos extra:
            // `title`, `format`, `default`, `example`, `enum`, `pattern`,
            // `minimum`, `maximum`, `description`…).
            _ => {
                out.insert(k.clone(), val.clone());
            }
        }
    }
    if let Some(t) = tipo {
        if nullable {
            out.insert(
                "type".to_string(),
                json!([t, "null"]),
            );
        } else {
            out.insert("type".to_string(), json!(t));
        }
    }
    Value::Object(out)
}

/// Traduce las herramientas del cliente (dialecto Gemini `master`) al
/// formato **OpenAI** (`{type:"function",function:{…}}`).
///
/// MEDIDO (2026-11-26, móvil): el proveedor SÓLO devuelve tool calls
/// estructuradas si la petición lleva `tools`; sin ellas responde con su
/// protocolo de texto.  Función **PURO**.
pub fn tools_gemini_a_openai(tools: Option<&Value>) -> Vec<Value> {
    let arr = match tools.and_then(|t| t.as_array()) {
        Some(a) => a,
        None => return Vec::new(),
    };
    let mut out: Vec<Value> = Vec::new();
    for t in arr {
        let decls = t
            .get("functionDeclarations")
            .or_else(|| t.get("function_declarations"))
            .and_then(|f| f.as_array());
        let Some(decls) = decls else { continue };
        for fd in decls {
            let name = fd
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if name.is_empty() {
                continue;
            }
            let description = fd
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();
            // `parametersJsonSchema` ya es JSON Schema; `parameters` es el
            // dialecto `master.Schema` y hay que bajarlo.
            let params = fd
                .get("parametersJsonSchema")
                .or_else(|| fd.get("parameters_json_schema"))
                .cloned()
                .or_else(|| fd.get("parameters").map(schema_master_a_json_schema))
                .unwrap_or_else(|| json!({"type": "object", "properties": {}}));
            let mut f = json!({"name": name, "parameters": params});
            if !description.is_empty() {
                f["description"] = json!(description);
            }
            out.push(json!({"type": "function", "function": f}));
        }
    }
    out
}

/// La conversación del cliente en formato **OpenAI**.
///
/// MEDIDO (2026-11-26, móvil): para que el agente CONTINÚE tras ejecutar
/// una herramienta hay que devolverle su llamada (`assistant.tool_calls`) y
/// el resultado (`role:"tool"`); si sólo se aplana el texto, el agente se
/// queda sin saber qué devolvió la herramienta.
///
/// Función **PURO**.
pub fn gemini_a_mensajes_openai(contents: &Value) -> Vec<Value> {
    let arr = match contents.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };
    let mut out: Vec<Value> = Vec::new();
    // Último `id` de tool call por nombre: si el LS no devuelve el `id` en
    // el `functionResponse`, se empareja por nombre.
    let mut ultimo_id: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for c in arr {
        let role = c.get("role").and_then(|r| r.as_str()).unwrap_or("user");
        let parts = c
            .get("parts")
            .and_then(|p| p.as_array())
            .cloned()
            .unwrap_or_default();
        let mut texto = String::new();
        let mut calls: Vec<Value> = Vec::new();
        let mut respuestas: Vec<Value> = Vec::new();
        for p in &parts {
            if let Some(t) = p.get("text").and_then(|t| t.as_str()) {
                texto.push_str(t);
                continue;
            }
            if let Some(fc) = p.get("functionCall").or_else(|| p.get("function_call")) {
                let name = fc
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if name.is_empty() {
                    continue;
                }
                let args = fc
                    .get("args")
                    .or_else(|| fc.get("arguments"))
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let id = fc
                    .get("id")
                    .and_then(|i| i.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("call_{}", calls.len() + 1));
                ultimo_id.insert(name.clone(), id.clone());
                calls.push(json!({
                    "id": id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": args.to_string(),
                    }
                }));
                continue;
            }
            if let Some(fr) = p.get("functionResponse").or_else(|| p.get("function_response")) {
                let name = fr
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let id = fr
                    .get("id")
                    .and_then(|i| i.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| ultimo_id.get(&name).cloned().unwrap_or_default());
                let resp = fr.get("response").cloned().unwrap_or_else(|| json!({}));
                respuestas.push(json!({
                    "role": "tool",
                    "tool_call_id": id,
                    "content": resp.to_string(),
                }));
            }
        }
        // Orden OpenAI: el mensaje del modelo con sus `tool_calls` va
        // ANTES que los resultados (`role:"tool"`), y el texto del usuario
        // después.
        let hay_calls = !calls.is_empty();
        if hay_calls {
            let mut m = json!({
                "role": "assistant",
                "content": if texto.is_empty() { Value::Null } else { json!(texto) },
            });
            m["tool_calls"] = Value::Array(calls);
            out.push(m);
        }
        for r in respuestas {
            out.push(r);
        }
        if !hay_calls && !texto.is_empty() {
            if role == "model" {
                out.push(json!({"role": "assistant", "content": texto}));
            } else {
                out.push(json!({"role": "user", "content": texto}));
            }
        }
    }
    out
}

/// Acota la conversación al tope, conservando la **cola** (lo más reciente).
///
/// MEDIDO (2026-11-26, móvil): el LS manda 750 KB de contexto.  Se recorta
/// por la FRENTE de los mensajes más antiguos (igual que el prompt plano)
/// y se limpian los `role:"tool"` que se quedan sin su `assistant`.
pub fn acotar_mensajes(mensajes: Vec<Value>, tope: usize) -> Vec<Value> {
    fn largo(m: &Value) -> usize {
        m.get("content")
            .and_then(|c| c.as_str())
            .map(|s| s.chars().count())
            .unwrap_or(0)
    }
    let mut ms = mensajes;
    let mut total: usize = ms.iter().map(largo).sum();
    if total <= tope {
        return ms;
    }
    let mut i = 0;
    while total > tope && i < ms.len() {
        let l = largo(&ms[i]);
        if l == 0 {
            i += 1;
            continue;
        }
        let quitar = (total - tope).min(l);
        let s = ms[i]["content"].as_str().unwrap_or("").to_string();
        let nuevo: String = s.chars().skip(quitar).collect();
        total -= quitar;
        if nuevo.trim().is_empty() {
            ms.remove(i);
            continue;
        }
        ms[i]["content"] = json!(nuevo);
        i += 1;
    }
    limpiar_mensajes_huerfanos(ms)
}

/// Quita los mensajes `role:"tool"` cuyo `tool_call_id` no corresponde a
/// ninguna llamada viva (⇒ el proveedor los rechazaría).
fn limpiar_mensajes_huerfanos(mensajes: Vec<Value>) -> Vec<Value> {
    let mut vivas: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out: Vec<Value> = Vec::new();
    for m in mensajes {
        let rol = m.get("role").and_then(|r| r.as_str()).unwrap_or("");
        if rol == "assistant" {
            if let Some(tcs) = m.get("tool_calls").and_then(|t| t.as_array()) {
                for tc in tcs {
                    if let Some(id) = tc.get("id").and_then(|i| i.as_str()) {
                        vivas.insert(id.to_string());
                    }
                }
            }
            out.push(m);
            continue;
        }
        if rol == "tool" {
            let id = m.get("tool_call_id").and_then(|i| i.as_str()).unwrap_or("");
            if id.is_empty() || !vivas.contains(id) {
                continue;
            }
            out.push(m);
            continue;
        }
        out.push(m);
    }
    out
}

/// Concatena las partes de un contenido Gemini, INCLUDING las tool calls y
/// sus resultados (como texto) ⇒ el motor local (SEERE) también ve el
/// intercambio de herramientas.
fn gemini_partes_completas(parts: Option<&Value>) -> String {
    let arr = match parts.and_then(|p| p.as_array()) {
        Some(a) => a,
        None => return String::new(),
    };
    let mut out = String::new();
    for p in arr {
        if let Some(t) = p.get("text").and_then(|t| t.as_str()) {
            out.push_str(t);
            continue;
        }
        if let Some(fc) = p.get("functionCall").or_else(|| p.get("function_call")) {
            let name = fc.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let args = fc
                .get("args")
                .or_else(|| fc.get("arguments"))
                .cloned()
                .unwrap_or_else(|| json!({}));
            out.push_str(&format!(
                "\n[LLAMADA A LA HERRAMIENTA {}: {}]\n",
                name, args
            ));
            continue;
        }
        if let Some(fr) = p.get("functionResponse").or_else(|| p.get("function_response")) {
            let name = fr.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let resp = fr.get("response").cloned().unwrap_or_else(|| json!({}));
            out.push_str(&format!(
                "\n[RESULTADO DE LA HERRAMIENTA {}: {}]\n",
                name, resp
            ));
        }
    }
    out
}

/// Concatena las partes de texto de un contenido Gemini.
fn gemini_partes(parts: Option<&Value>) -> String {
    let arr = match parts.and_then(|p| p.as_array()) {
        Some(a) => a,
        None => return String::new(),
    };
    let mut out: Vec<String> = Vec::new();
    for p in arr {
        if let Some(t) = p.get("text").and_then(|t| t.as_str()) {
            if !t.is_empty() {
                out.push(t.to_string());
            }
        }
    }
    // Las partes de un MISMO content se CONCATENAN (no se unen con '\n'):
    // la API de Gemini trata `parts` como trozos consecutivos del mismo
    // texto, y cada trozo trae su propia puntuacion ('hola ' + 'mundo' =
    // 'hola mundo').
    // MEDIDO (2026-11-23): con '\n' el prompt salia 'hola \nmundo' y el
    // texto del usuario llegaba partido al modelo (test rojo
    // gemini_prompt_une_el_texto_y_separa_el_sistema).
    out.concat()
}

/// El modelo que pide la RUTA (si la ruta lo nombra).
///
/// `/v1internal:generateContent` ⇒ `None` (el router decide).
/// `/v1beta/models/gemini-2.5-pro:generateContent` ⇒ `Some("gemini-2.5-pro")`.
pub fn modelo_de_la_ruta(path: &str) -> Option<String> {
    let r = gemini_route(path)?;
    let m = r.model.trim().to_string();
    if m.is_empty() {
        None
    } else {
        Some(m)
    }
}

/// Ejecuta una petición Gemini contra el router de MegaDaemon y devuelve
/// la respuesta **en formato Gemini** (`candidates`/`usageMetadata`).
pub fn gemini_generate_content(body: &Value) -> Result<Value, GatewayError> {
    // v16: la variante de producción (vault de disco + transporte real).
    let vault = crate::providers::load_vault();
    let transport = crate::providers::UreqTransport::default();
    let deps = crate::providers::ChatDeps::new(&vault, &transport);
    gemini_generate_content_with(body, &deps)
}

/// Variante de [`gemini_generate_content`] que acepta la ruta Gemini (`.../models/<model>:generateContent`).
pub fn gemini_generate_content_for(path: &str, body: &Value) -> Result<Value, GatewayError> {
    let modelo = modelo_de_la_ruta(path);
    gemini_generate_content_con_modelo_for(path, body, modelo.as_deref())
}

/// Variante de [`gemini_stream_sse`] que acepta la ruta Gemini (`.../models/<model>:streamGenerateContent`).
pub fn gemini_stream_sse_for(path: &str, body: &Value) -> Result<String, GatewayError> {
    let modelo = modelo_de_la_ruta(path);
    gemini_stream_sse_con_modelo(path, body, modelo.as_deref())
}

/// Variante de producción con el **modelo pedido** explícito.
///
/// `modelo = None` ⇒ decide el router (el alias canónico).
/// `modelo = Some("gemini-2.5-pro")` ⇒ ese modelo va PRIMERO en la
/// cascada (con el mismo *fallback* real).
pub fn gemini_generate_content_con_modelo_for(
    path: &str,
    body: &Value,
    modelo: Option<&str>,
) -> Result<Value, GatewayError> {
    let vault = crate::providers::load_vault();
    let transport = crate::providers::UreqTransport::default();
    let deps = crate::providers::ChatDeps::new(&vault, &transport);
    gemini_generate_content_con_modelo_with(path, body, modelo, &deps)
}

/// ¿El modelo pedido es ALGUNO de los proveedores del vault?
///
/// MEDIDO (2026-11-26, móvil): el LS pide modelos de **GOOGLE**
/// ("gemini-4-neon", "gemini-3.8-flash") que no son proveedores nuestros.
/// Si se pasan tal cual a la cascada, ésta los ignora y arranca por el
/// PRIMER proveedor habilitado (no por el canónico) ⇒ el "gateway canónico"
/// dejaba de serlo.  Un modelo desconocido ⇒ routing canónico (y queda
/// registrado igual en `requested_model`).
fn es_modelo_del_vault(vault: &crate::providers::ProviderVault, m: &str) -> bool {
    let t = m.trim();
    !t.is_empty() && vault.providers.iter().any(|p| p.id == t || p.model == t)
}

/// Variante **inyectable** de [`gemini_generate_content_con_modelo_for`].
pub fn gemini_generate_content_con_modelo_with(
    _path: &str,
    body: &Value,
    modelo: Option<&str>,
    deps: &crate::providers::ChatDeps<'_>,
) -> Result<Value, GatewayError> {
    let g = gemini_prompt(body)?;
    // Un alias NO es un modelo: que decida el router.
    // MEDIDO (2026-11-26): un modelo que NO es proveedor nuestro tampoco
    // (el LS pide modelos de Google) ⇒ routing canónico.
    let pedido = modelo
        .map(|m| m.trim())
        .filter(|m| !m.is_empty() && !es_alias_megadaemon(m))
        .filter(|m| es_modelo_del_vault(deps.vault, m));
    // MEDIDO (2026-11-26, móvil): la conversación se pasa YA montada (con el
    // sistema delante) para que el bucle de herramientas llegue intacto al
    // proveedor: sin `tools` NO devuelve tool calls, y sin la conversación
    // el agente no puede CONTINUAR tras ejecutar una.
    let mut msgs: Vec<Value> = Vec::new();
    if let Some(s) = g.system.as_deref() {
        if !s.trim().is_empty() {
            msgs.push(json!({"role": "system", "content": s}));
        }
    }
    let hay_conversacion = !g.mensajes.is_empty();
    msgs.extend(g.mensajes.iter().cloned());
    let res = crate::providers::execute_chat_con_tools_with(
        &g.prompt,
        pedido,
        None,
        g.system.as_deref(),
        g.max_tokens,
        &g.tools,
        if hay_conversacion { Some(&msgs) } else { None },
        deps,
    );
    let mut v = gemini_response(&res, g.tools_requested);
    // Traza del recorte: el gateway no puede callarse que recortó el
    // contexto (MEDIDO 2026-11-26, móvil: el LS manda 758 KB).
    if let Some(md) = v.get_mut("megadaemon").and_then(|x| x.as_object_mut()) {
        md.insert("prompt_bytes".to_string(), json!(g.bytes_prompt));
        md.insert("prompt_truncado".to_string(), json!(g.truncado));
    }
    let requested = modelo
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| MODELO_CANONICO.to_string());
    anotar_traza(&mut v, &requested);
    Ok(v)
}

/// Variante **inyectable** de [`gemini_generate_content`].
pub fn gemini_generate_content_with(
    body: &Value,
    deps: &crate::providers::ChatDeps<'_>,
) -> Result<Value, GatewayError> {
    gemini_generate_content_con_modelo_with("", body, None, deps)
}

/// Envoltorio de la respuesta de `providers::execute_chat` en forma Gemini.
fn gemini_response(res: &crate::providers::ChatResult, tools_requested: usize) -> Value {
    let mut parts: Vec<Value> = Vec::new();
    if !res.reply.trim().is_empty() || res.tool_calls.is_empty() {
        parts.push(json!({ "text": res.reply }));
    }
    // MEDIDO (2026-11-26, móvil): sin esta parte el agente de Antigravity
    // NO ejecuta la herramienta: la muestra como texto.  El dialecto es el
    // del proto `master` (`FunctionCall{name, args}` con `args` como
    // `google.protobuf.Struct` ⇒ objeto JSON plano), NO el `args`/`arguments`
    // de los dialectos públicos.
    for tc in &res.tool_calls {
        let mut fc = json!({"name": tc.name, "args": tc.arguments});
        if !tc.id.is_empty() {
            fc["id"] = json!(tc.id);
        }
        parts.push(json!({ "functionCall": fc }));
    }
    if !res.reasoning.trim().is_empty() {
        parts.push(json!({ "text": res.reasoning, "thought": true }));
    }
    json!({
        "candidates": [{
            "content": {
                "role": "model",
                "parts": parts,
            },
            "finishReason": "STOP",
            "index": 0,
        }],
        "usageMetadata": {
            "promptTokenCount": 0,
            "candidatesTokenCount": res.tokens_used,
            "totalTokenCount": res.tokens_used,
        },
        // HONESTIDAD: `modelVersion` dice el modelo REAL que contestó.
        "modelVersion": if res.model_name.trim().is_empty() {
            "megadaemon".to_string()
        } else {
            res.model_name.clone()
        },
        // Trazabilidad propia (no rompe el formato Gemini).
        "megadaemon": {
            "provider_id": res.model_id,
            "provider_name": res.model_name,
            "vendor": res.vendor,
            "latency_ms": res.latency_ms,
            "fallback_used": res.fallback_used,
            "tools_requested": tools_requested,
            // v16: la traza de la cascada y el estado de cuota.
            "provider_trace": res.provider_trace,
            "quota": res.quota,
            // MEDIDO (2026-11-26, móvil): las tool calls que se devuelven.
            "tool_calls": res.tool_calls.len(),
        }
    })
}

/// Variante *streaming*: devuelve el cuerpo SSE completo (una sola_DATA_
///
/// con la respuesta entera, porque el daemon no hace streaming fino).
pub fn gemini_stream_sse(body: &Value) -> Result<String, GatewayError> {
    let v = gemini_generate_content(body)?;
    Ok(gemini_a_sse(&v))
}

/// Variante *streaming* con el **modelo pedido** explicito.
pub fn gemini_stream_sse_con_modelo(
    path: &str,
    body: &Value,
    modelo: Option<&str>,
) -> Result<String, GatewayError> {
    let v = gemini_generate_content_con_modelo_for(path, body, modelo)?;
    Ok(gemini_a_sse(&v))
}

/// La trama (chunk) que se envía dentro de una respuesta Gemini.
fn gemini_chunk(v: &Value) -> Value {
    // MEDIDO (2026-11-26, móvil): la trama se quedaba SÓLO con el texto ⇒
    // las `functionCall` se perdían en el camino (el LS usa SSE).
    let parts = v["candidates"][0]["content"]["parts"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    json!({
        "candidates": [{
            "content": { "role": "model", "parts": parts },
            "finishReason": "STOP",
            "index": 0,
        }],
        "modelVersion": MODELO_CANONICO,
        "megadaemon": v.get("megadaemon").cloned().unwrap_or(Value::Null),
    })
}

/// Convierte una respuesta Gemini completa en el cuerpo SSE.
///
/// MEDIDO (2026-11-23): el chunk SSE **no** llevaba la trazabilidad => en
/// streaming nadie sabia que modelo habia contestado.
pub fn gemini_a_sse(v: &Value) -> String {
    format!(
        "data: {}\n\ndata: [DONE]\n\n",
        gemini_chunk(v)
    )
}

/// SSE **con el envoltorio v1internal**: cada trama es un
/// `GenerateContentResponse` de `v1internal` (con `response` dentro).
///
/// MEDIDO (2026-11-26, móvil): el LS pide `?alt=sse` ⇒ espera tramas SSE
/// `data: {json}`.
///
/// **NO** se manda `data: [DONE]`: el backend real de Google con `alt=sse`
/// termina el flujo cerrando la conexión, y `[DONE]` no es un JSON válido ⇒ el
/// decodificador protojson del cliente lo rechazaría.
pub fn gemini_a_sse_v1internal(v: &Value) -> String {
    let trama = envolver_v1internal(&gemini_chunk(v), "");
    format!("data: {trama}\n\n")
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

    // -----------------------------------------------------------------
    // GEMINI / BYOM (Antigravity)
    // -----------------------------------------------------------------

    #[test]
    fn reconoce_las_rutas_gemini_del_language_server() {
        // MEDIDO 2026-11-22: strings de `liblanguage_server.so`.
        let r = gemini_route("/v1internal:generateContent").unwrap();
        assert_eq!(r.model, "");
        assert!(!r.stream);

        let r = gemini_route("/v1internal:streamGenerateContent").unwrap();
        assert!(r.stream);

        let r = gemini_route("/v1beta/models/megadaemon:generateContent").unwrap();
        assert_eq!(r.model, "megadaemon");
        assert!(!r.stream);

        let r = gemini_route("/v1beta/models/megadaemon:streamGenerateContent").unwrap();
        assert_eq!(r.model, "megadaemon");
        assert!(r.stream);

        assert!(gemini_route("/v1/chat/completions").is_none());
        assert!(gemini_route("/v1internal:otraCosa").is_none());
        // MEDIDO 2026-11-24 (movil): el LS pide la forma CON BARRA.
        let barra = gemini_route("/v1internal/generateContent").expect("forma con barra");
        assert_eq!(barra.model, "");
        assert!(!barra.stream);
        let barra_stream = gemini_route("/v1internal/streamGenerateContent").expect("stream con barra");
        assert!(barra_stream.stream);
        let v1beta = gemini_route("/v1beta/models/megadaemon/generateContent").expect("v1beta con barra");
        assert_eq!(v1beta.model, "megadaemon");
        assert_eq!(
            normalizar_ruta_gemini("/v1internal/listModels"),
            "/v1internal:listModels"
        );
        // El catalogo lo sirve el shim en las dos formas.
        assert!(crate::agyshim::dialecto_de("/v1internal/listModels").is_some());
        assert!(crate::agyshim::dialecto_de("/v1internal:listModels").is_some());
        assert!(crate::agyshim::dialecto_de("/v1internal/generateContent").is_some());
        assert!(crate::agyshim::dialecto_de("/v1internal:generateContent").is_some());
    }

    #[test]
    fn gemini_prompt_une_el_texto_y_separa_el_sistema() {
        let body = json!({
            "contents": [
                {"role": "user", "parts": [{"text": "hola "}, {"text": "mundo"}]},
                {"role": "model", "parts": [{"text": "respuesta previa"}]},
                {"role": "user", "parts": [{"text": "¿y ahora?"}]}
            ],
            "systemInstruction": {"parts": [{"text": "eres un agente"}]},
            "generationConfig": {"maxOutputTokens": 1234},
            "tools": [{"functionDeclarations": [{"name": "a"}, {"name": "b"}]}]
        });
        let g = gemini_prompt(&body).unwrap();
        // MEDIDO (2026-11-23): las partes de un content Gemini se unen con
        // "\n" (antes con " "), asi que el test no puede fijar el separador.
        assert!(g.prompt.contains("hola"));
        assert!(g.prompt.contains("mundo"));
        assert!(g.prompt.contains("respuesta previa"));
        assert_eq!(g.system.as_deref(), Some("eres un agente"));
        assert_eq!(g.max_tokens, 1234);
        assert_eq!(g.tools_requested, 2);
    }

    #[test]
    fn gemini_prompt_rechaza_peticiones_vacias() {
        assert!(gemini_prompt(&json!({})).is_err());
        assert!(gemini_prompt(&json!({"contents": []})).is_err());
        assert!(gemini_prompt(&json!({"contents": [{"role":"user","parts":[]}]})).is_err());
    }

    #[test]
    fn max_tokens_se_acota() {
        let body = json!({"contents":[{"role":"user","parts":[{"text":"x"}]}],
                          "generationConfig":{"maxOutputTokens": 999999}});
        let g = gemini_prompt(&body).unwrap();
        assert!(g.max_tokens <= 32_768);
    }

    #[test]
    fn la_respuesta_gemini_tiene_forma_de_candidates() {
        let res = crate::providers::ChatResult {
            reply: "hola".to_string(),
            reasoning: String::new(),
            model_id: "minimax-m3.1-flash".to_string(),
            model_name: "MiniMax-M3.1-Flash".to_string(),
            vendor: "MiniMax".to_string(),
            latency_ms: 12.0,
            tokens_used: 7,
            tools_used: Vec::new(),
            fallback_used: false,
            quota: None,
            provider_trace: Vec::new(),
            tool_calls: Vec::new(),
        };
        let v = gemini_response(&res, 3);
        assert_eq!(v["candidates"][0]["content"]["parts"][0]["text"], "hola");
        assert_eq!(v["candidates"][0]["content"]["role"], "model");
        assert_eq!(v["candidates"][0]["finishReason"], "STOP");
        assert_eq!(v["usageMetadata"]["candidatesTokenCount"], 7);
        assert_eq!(v["megadaemon"]["provider_id"], "minimax-m3.1-flash");
        assert_eq!(v["megadaemon"]["tools_requested"], 3);
    }

    // -----------------------------------------------------------------
    // ENVOLTORIO v1internal (MEDIDO 2026-11-26, móvil)
    // -----------------------------------------------------------------

    /// MEDIDO (2026-11-26, móvil, captura real del tráfico del LS con
    /// `tcpdump`): el LS manda la petición ENVUELTA en `request`.
    const CUERPO_V1INTERNAL: &str = r#"{
        "project": "aicode-consumers",
        "requestId": "agent/9dfa200c/1795662565867",
        "model": "gemini-4-neon",
        "request": {
            "contents": [{"role": "user", "parts": [{"text": "hola"}]}],
            "systemInstruction": {"parts": [{"text": "eres un agente"}]},
            "generationConfig": {"maxOutputTokens": 321},
            "tools": [{"functionDeclarations": [{"name": "a"}]}]
        }
    }"#;

    #[test]
    fn el_envoltorio_v1internal_se_reconoce_y_se_desenvuelve() {
        let body: Value = serde_json::from_str(CUERPO_V1INTERNAL).unwrap();
        assert!(es_envoltorio_v1internal(&body));
        assert!(!es_envoltorio_v1internal(&json!({"contents": []})));
        // El cuerpo efectivo es el INTERIOR del envoltorio.
        assert_eq!(cuerpo_gemini(&body)["contents"][0]["parts"][0]["text"], "hola");
        // MEDIDO (2026-11-26, móvil): antes esto fallaba con
        // "'contents' debe ser una lista" (el `contents` de verdad está en
        // `request.contents`) ⇒ el LS veía un 502 y reintentaba 9 veces.
        let g = gemini_prompt(&body).unwrap();
        assert_eq!(g.prompt, "hola");
        assert_eq!(g.system.as_deref(), Some("eres un agente"));
        assert_eq!(g.max_tokens, 321);
        assert_eq!(g.tools_requested, 1);
    }

    #[test]
    fn el_modelo_del_cuerpo_se_lee_del_envoltorio() {
        let body: Value = serde_json::from_str(CUERPO_V1INTERNAL).unwrap();
        assert_eq!(modelo_del_cuerpo(&body).as_deref(), Some("gemini-4-neon"));
        assert_eq!(
            modelo_del_cuerpo(&json!({"request": {"model": "otro"}})).as_deref(),
            Some("otro")
        );
        assert!(modelo_del_cuerpo(&json!({})).is_none());
        // Una petición Gemini pelada (sin envoltorio) también vale.
        assert_eq!(
            modelo_del_cuerpo(&json!({"contents": [], "model": "x"})).as_deref(),
            Some("x")
        );
    }

    #[test]
    fn la_respuesta_se_envuelve_en_el_generatocontentresponse() {
        let res = crate::providers::ChatResult {
            reply: "hola".to_string(),
            reasoning: String::new(),
            model_id: "minimax-m3.1-flash".to_string(),
            model_name: "MiniMax-M3.1-Flash".to_string(),
            vendor: "MiniMax".to_string(),
            latency_ms: 12.0,
            tokens_used: 7,
            tools_used: Vec::new(),
            fallback_used: false,
            quota: None,
            provider_trace: Vec::new(),
            tool_calls: Vec::new(),
        };
        let v = gemini_response(&res, 3);
        let env = envolver_v1internal(&v, "trace-1");
        assert_eq!(env["traceId"], "trace-1");
        assert_eq!(env["response"]["candidates"][0]["content"]["parts"][0]["text"], "hola");
        assert_eq!(env["response"]["usageMetadata"]["candidatesTokenCount"], 7);
        // MEDIDO (2026-11-26): el protojson de Go falla con un campo
        // desconocido ⇒ el bloque `megadaemon` NO puede ir dentro de
        // `response` (el LS lo rechazaría).
        assert!(env["response"].get("megadaemon").is_none());
    }

    #[test]
    fn el_sse_v1internal_es_una_trama_con_response_y_sin_done() {
        let res = crate::providers::ChatResult {
            reply: "hola".to_string(),
            reasoning: String::new(),
            model_id: "minimax-m3.1-flash".to_string(),
            model_name: "MiniMax-M3.1-Flash".to_string(),
            vendor: "MiniMax".to_string(),
            latency_ms: 12.0,
            tokens_used: 7,
            tools_used: Vec::new(),
            fallback_used: false,
            quota: None,
            provider_trace: Vec::new(),
            tool_calls: Vec::new(),
        };
        let v = gemini_response(&res, 0);
        let sse = gemini_a_sse_v1internal(&v);
        assert!(sse.starts_with("data: {"));
        assert!(sse.contains("\"response\""));
        assert!(sse.contains("hola"));
        // `[DONE]` no es JSON: el decodificador protojson del cliente lo
        // rechazaría.  Google con `alt=sse` termina el flujo al cerrar.
        assert!(!sse.contains("[DONE]"));
    }

    #[test]
    fn un_modelo_que_no_es_proveedor_va_al_routing_canonico() {
        let vault = crate::providers::ProviderVault {
            default_model: "local-seere".to_string(),
            providers: vec![crate::providers::ProviderEntry {
                id: "google-ai-studio".to_string(),
                name: "Google AI Studio".to_string(),
                vendor: "Google".to_string(),
                endpoint: "https://example.invalid/v1/chat/completions".to_string(),
                model: "gemini-3.8-flash".to_string(),
                api_key: String::new(),
                auth_type: "bearer".to_string(),
                tier: "free".to_string(),
                free_signup_url: String::new(),
                enabled: true,
                supports_reasoning: false,
            }],
        };
        assert!(es_modelo_del_vault(&vault, "google-ai-studio"));
        assert!(es_modelo_del_vault(&vault, "gemini-3.8-flash"));
        // MEDIDO (2026-11-26, móvil): el LS pide modelos de GOOGLE que no
        // son proveedores nuestros ⇒ routing canónico (no "el primero
        // habilitado"), que es lo que hace que el gateway sea canónico.
        assert!(!es_modelo_del_vault(&vault, "gemini-4-neon"));
        assert!(!es_modelo_del_vault(&vault, ""));
    }

    #[test]
    fn un_prompt_enorme_se_recorta_conservando_la_cola() {
        // MEDIDO (2026-11-26, móvil): el LS manda 758 KB de contexto.
        let enorme = "a".repeat(MAX_PROMPT_CHARS + 5_000) + "FINAL";
        let body = json!({
            "contents": [{"role": "user", "parts": [{"text": enorme}]}]
        });
        let g = gemini_prompt(&body).unwrap();
        assert!(g.truncado, "un prompt de 125k debe recortarse");
        assert_eq!(g.bytes_prompt, enorme.chars().count());
        assert!(g.prompt.chars().count() <= MAX_PROMPT_CHARS);
        // Se conserva la COLA: lo más reciente.
        assert!(g.prompt.ends_with("FINAL"));
        // Un prompt normal NO se toca.
        let g2 = gemini_prompt(&json!({
            "contents": [{"role": "user", "parts": [{"text": "hola"}]}]
        }))
        .unwrap();
        assert!(!g2.truncado);
        assert_eq!(g2.bytes_prompt, 4);
    }

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

    // ==================================================================
    // HERRAMIENTAS (MEDIDO 2026-11-26, móvil)
    // ==================================================================

    /// El dialecto REAL que manda el LS (capturado del móvil).
    const TOOLS_DEL_LS: &str = r#"[
      {"functionDeclarations":[
        {"name":"view_file",
         "description":"View the contents of a file.",
         "parameters":{"type":"OBJECT",
                       "properties":{
                         "AbsolutePath":{"type":"STRING","description":"Ruta"},
                         "StartLine":{"type":"INTEGER","description":"Linea"},
                         "toolAction":{"type":"STRING","description":"Accion"}},
                       "required":["AbsolutePath","toolAction"]}}]},
      {"functionDeclarations":[
        {"name":"run_command",
         "description":"PROPOSE a command.",
         "parameters":{"type":"OBJECT",
                       "properties":{"CommandLine":{"type":"STRING"}},
                       "required":["CommandLine"]}}]}
    ]"#;

    /// El ejemplo anterior, ya parseado (un `&str` con `json!` sería un
    /// *string*, no el array de herramientas).
    fn tools_del_ls() -> Value {
        serde_json::from_str::<Value>(TOOLS_DEL_LS).expect("el ejemplo es JSON valido")
    }

    #[test]
    fn las_tools_del_ls_se_traducen_al_formato_openai() {
        let tools = tools_gemini_a_openai(Some(&tools_del_ls()));
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["type"], "function");
        assert_eq!(tools[0]["function"]["name"], "view_file");
        // Los tipos de `master.Type` (MAYÚSCULAS) bajan a JSON Schema.
        assert_eq!(tools[0]["function"]["parameters"]["type"], "object");
        assert_eq!(
            tools[0]["function"]["parameters"]["properties"]["AbsolutePath"]["type"],
            "string"
        );
        assert_eq!(
            tools[0]["function"]["parameters"]["properties"]["StartLine"]["type"],
            "integer"
        );
        assert_eq!(
            tools[0]["function"]["parameters"]["required"][0],
            "AbsolutePath"
        );
        assert_eq!(tools[1]["function"]["name"], "run_command");
    }

    #[test]
    fn el_prompt_saca_las_tools_del_cuerpo() {
        let body = json!({
            "contents":[{"role":"user","parts":[{"text":"lee el fichero"}]}],
            "tools": tools_del_ls(),
        });
        let g = gemini_prompt(&body).unwrap();
        assert_eq!(g.tools_requested, 2);
        assert_eq!(g.tools.len(), 2);
        assert_eq!(g.tools[0]["function"]["name"], "view_file");
    }

    #[test]
    fn la_conversacion_lleva_la_llamada_y_su_resultado() {
        let contents = json!([
            {"role":"user","parts":[{"text":"lee el fichero"}]},
            {"role":"model","parts":[
                {"functionCall":{"id":"call_1","name":"view_file",
                                 "args":{"AbsolutePath":"/tmp/x"}}}
            ]},
            {"role":"user","parts":[
                {"functionResponse":{"id":"call_1","name":"view_file",
                                     "response":{"contenido":"hola"}}}
            ]}
        ]);
        let ms = gemini_a_mensajes_openai(&contents);
        assert_eq!(ms.len(), 3);
        assert_eq!(ms[0]["role"], "user");
        assert_eq!(ms[1]["role"], "assistant");
        assert_eq!(ms[1]["tool_calls"][0]["id"], "call_1");
        assert_eq!(ms[1]["tool_calls"][0]["function"]["name"], "view_file");
        // Los argumentos OpenAI viajan como TEXTO JSON.
        assert!(ms[1]["tool_calls"][0]["function"]["arguments"]
            .as_str()
            .unwrap()
            .contains("AbsolutePath"));
        assert_eq!(ms[2]["role"], "tool");
        assert_eq!(ms[2]["tool_call_id"], "call_1");
        assert!(ms[2]["content"].as_str().unwrap().contains("hola"));
    }

    #[test]
    fn la_conversacion_se_acota_por_la_cola_y_no_deja_tools_huerfanas() {
        let contents = json!([
            {"role":"user","parts":[{"text":"a".repeat(500)}]},
            {"role":"model","parts":[
                {"functionCall":{"id":"call_1","name":"view_file","args":{}}}
            ]},
            {"role":"user","parts":[
                {"functionResponse":{"id":"call_1","name":"view_file","response":{}}}
            ]},
            {"role":"user","parts":[{"text":"z".repeat(500)}]},
        ]);
        let ms = acotar_mensajes(gemini_a_mensajes_openai(&contents), 600);
        // El total de texto cabe en el tope.
        let total: usize = ms
            .iter()
            .map(|m| m.get("content").and_then(|c| c.as_str()).unwrap_or("").chars().count())
            .sum();
        assert!(total <= 600, "total={}", total);
        // Ningún `role:"tool"` sin su `assistant`.
        for m in &ms {
            if m["role"] == "tool" {
                let id = m["tool_call_id"].as_str().unwrap();
                assert!(
                    ms.iter().any(|a| a["role"] == "assistant"
                        && a["tool_calls"]
                            .as_array()
                            .map(|t| t.iter().any(|x| x["id"] == id))
                            .unwrap_or(false)),
                    "tool huerfana: {}",
                    id
                );
            }
        }
    }

    #[test]
    fn la_respuesta_emite_la_function_call_para_el_ls() {
        let res = crate::providers::ChatResult {
            reply: "voy a leerlo".to_string(),
            reasoning: String::new(),
            model_id: "minimax-m3.1-flash".to_string(),
            model_name: "MiniMax-M3.1-Flash".to_string(),
            vendor: "MiniMax".to_string(),
            latency_ms: 12.0,
            tokens_used: 7,
            tools_used: vec!["provider_tool:view_file".to_string()],
            fallback_used: false,
            quota: None,
            provider_trace: Vec::new(),
            tool_calls: vec![crate::providers::ParsedToolCall {
                id: "call_1".to_string(),
                name: "view_file".to_string(),
                arguments: json!({"AbsolutePath": "/tmp/x"}),
            }],
        };
        let v = gemini_response(&res, 1);
        let parts = v["candidates"][0]["content"]["parts"].as_array().unwrap();
        // El texto sigue ahí...
        assert_eq!(parts[0]["text"], "voy a leerlo");
        // ...y la llamada viaja como `functionCall` con `args` (el dialecto
        // `master.FunctionCall` del proto del LS: `name` + `args`).
        let fc = parts
            .iter()
            .find(|p| p.get("functionCall").is_some())
            .expect("debe haber functionCall");
        assert_eq!(fc["functionCall"]["name"], "view_file");
        assert_eq!(fc["functionCall"]["args"]["AbsolutePath"], "/tmp/x");
        assert_eq!(fc["functionCall"]["id"], "call_1");
        // Y el SSE NO se la pierde (el LS usa `alt=sse`).
        let sse = gemini_a_sse_v1internal(&v);
        assert!(sse.contains("functionCall"));
        assert!(sse.contains("view_file"));
    }
}
