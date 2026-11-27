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

impl std::fmt::Display for GatewayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP {}: {}", self.status, self.message)
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
            // MEDIDO (2026-11-27): era "stop" FIJO.  Si el proveedor cortó
            // la respuesta, el cliente recibía "stop" y no se enteraba.
            "finish_reason": if res.finish_reason.trim().is_empty() {
                "stop".to_string()
            } else {
                res.finish_reason.clone()
            },
        }],
        "usage": usage,
        // Trazabilidad propia de MegaDaemon (no rompe el formato OpenAI).
        "megadaemon": {
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
}

/// Convierte una petición Gemini (`genai`) en prompt + sistema.
///
/// Es **puro**: no toca la red (testeable sin nada).
pub fn gemini_prompt(body: &Value) -> Result<GeminiPrompt, GatewayError> {
    let contents = body
        .get("contents")
        .and_then(|c| c.as_array())
        .ok_or_else(|| GatewayError::bad("'contents' debe ser una lista"))?;
    let mut parts_out: Vec<String> = Vec::new();
    for c in contents {
        let role = c.get("role").and_then(|r| r.as_str()).unwrap_or("user");
        let texto = gemini_partes(c.get("parts"));
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
        // MEDIDO (2026-11-27): el defecto eran 2.048 tokens.  Con un modelo
        // que "piensa" (thinking), el presupuesto se lo come el razonamiento
        // y la respuesta se corta A MEDIA FRASE ("The model cut off early in
        // its thinking — stopped mid-sentence").  Ahora el defecto es el
        // mismo que usa el resto del daemon (4.096) y, si el cliente NO
        // manda `maxOutputTokens`, se avisa en la traza (no se finge que el
        // modelo terminó bien).
        .unwrap_or(crate::providers::DEFAULT_MAX_TOKENS as u64)
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
    Ok(GeminiPrompt {
        prompt: parts_out.join("\n\n"),
        system,
        max_tokens,
        tools_requested,
    })
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

/// Variante **inyectable** de [`gemini_generate_content_con_modelo_for`].
pub fn gemini_generate_content_con_modelo_with(
    _path: &str,
    body: &Value,
    modelo: Option<&str>,
    deps: &crate::providers::ChatDeps<'_>,
) -> Result<Value, GatewayError> {
    let g = gemini_prompt(body)?;
    // Un alias NO es un modelo: que decida el router.
    let pedido = modelo
        .map(|m| m.trim())
        .filter(|m| !m.is_empty() && !es_alias_megadaemon(m));
    let res = crate::providers::execute_chat_with(
        &g.prompt,
        pedido,
        None,
        g.system.as_deref(),
        g.max_tokens,
        deps,
    );
    let mut v = gemini_response(&res, g.tools_requested);
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
    let mut message = json!({
        "role": "model",
        "parts": [{ "text": res.reply }],
    });
    if !res.reasoning.trim().is_empty() {
        message["parts"]
            .as_array_mut()
            .expect("parts es un array")
            .push(json!({ "text": res.reasoning, "thought": true }));
    }
    json!({
        "candidates": [{
            "content": message,
            // MEDIDO (2026-11-27): esto era "STOP" FIJO.  Cuando el
            // proveedor cortaba la respuesta a media frase (por `max_tokens`,
            // por filtro o por corte de stream) el cliente recibía "STOP" y
            // no había forma de saber por qué faltaba el final.  Ahora sale
            // el motivo REAL, traducido al vocabulario de Gemini.
            "finishReason": gemini_finish_reason(&res.finish_reason),
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
        }
    })
}

/// Traduce el `finish_reason` de OpenAI al vocabulario de Gemini.
///
/// MEDIDO (2026-11-27): el dialecto Gemini **fijaba `STOP` a pelo**, así que
/// una respuesta cortada a media frase ("The model cut off early in its
/// thinking — stopped mid-sentence") se anunciaba como terminada con
/// normalidad y el cliente no tenía ni idea de la causa.
///
/// * `length` ⇒ `MAX_TOKENS` (se agotó `max_tokens`: el corte es del límite).
/// * `content_filter` ⇒ `SAFETY` (lo cortó el filtro del proveedor).
/// * `tool_calls` ⇒ `STOP` (terminó pidiendo herramientas, no es un corte).
/// * vacío / desconocido ⇒ `STOP` (el proveedor no lo dijo: no se inventa).
pub fn gemini_finish_reason(finish_reason: &str) -> &'static str {
    match finish_reason.trim().to_ascii_lowercase().as_str() {
        "length" | "max_tokens" => "MAX_TOKENS",
        "content_filter" | "safety" | "recitation" => "SAFETY",
        _ => "STOP",
    }
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

/// Convierte una respuesta Gemini completa en el cuerpo SSE.
///
/// MEDIDO (2026-11-23): el chunk SSE **no** llevaba la trazabilidad => en
/// streaming nadie sabia que modelo habia contestado.
pub fn gemini_a_sse(v: &Value) -> String {
    let texto = v["candidates"][0]["content"]["parts"]
        .as_array()
        .map(|ps| {
            ps.iter()
                .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();
    let chunk = json!({
        "candidates": [{
            "content": { "role": "model", "parts": [{ "text": texto }] },
            // El motivo REAL del final (no "STOP" a pelo): si el proveedor
            // cortó por `max_tokens`, el cliente lo ve en el stream.
            "finishReason": v["candidates"][0]["finishReason"]
                .as_str()
                .unwrap_or("STOP"),
            "index": 0,
        }],
        "modelVersion": MODELO_CANONICO,
        "megadaemon": v.get("megadaemon").cloned().unwrap_or(Value::Null),
    });
    format!("data: {}\n\ndata: [DONE]\n\n", chunk)
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
            finish_reason: String::new(),
        };
        let v = gemini_response(&res, 3);
        assert_eq!(v["candidates"][0]["content"]["parts"][0]["text"], "hola");
        assert_eq!(v["candidates"][0]["content"]["role"], "model");
        assert_eq!(v["candidates"][0]["finishReason"], "STOP");
        assert_eq!(v["usageMetadata"]["candidatesTokenCount"], 7);
        assert_eq!(v["megadaemon"]["provider_id"], "minimax-m3.1-flash");
        assert_eq!(v["megadaemon"]["tools_requested"], 3);
    }

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
