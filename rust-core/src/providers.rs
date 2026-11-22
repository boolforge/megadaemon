//! MegaDaemon v13.0 Zenith — Pure Rust Parasitic Multi-LLM Swarm & Provider Vault
//! Replaces legacy Python `parasite_llm_router.py` runtime shell-outs with native ARM64 Rust.
//! Supports MiniMax (M2.7 / M2.5), GitHub Models (via gh CLI), Pollinations (zero-key),
//! OpenRouter (:free), Groq, Cerebras, Google AI Studio, Mistral, Hybrid Tool+Cloud, and Local SEERE v12.

use crate::{binwalk_die, cirx_rtl, kaitai_imhex, npu_bridge, provider_catalog, seere_v11};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

/// Ruta HISTÓRICA del fichero de providers (instalaciones antiguas y
/// despliegue dentro de la app Antigravity).  Se conserva como LECTURA de
/// respaldo; la escritura va siempre a [`providers_path`].
pub const PROVIDERS_PATH: &str = "/data/user/0/com.agy/files/megadaemon/state/providers.json";

/// Ruta REAL del fichero de providers (donde viven las API keys BYOK).
///
/// Se deriva del `state_dir` del daemon (una sola fuente de verdad).
/// MEDIDO 2026-11-21: antes el fichero estaba SIEMPRE en el directorio
/// privado de la app (`/data/user/0/com.agy/files/megadaemon/state/`), que
/// en el móvil NO es la zona que se respalda => las claves BYOK del usuario
/// quedaban FUERA del backup.  En el móvil ahora es
/// `/data/local/tmp/megadaemon/state/providers.json` (dentro de la zona
/// respaldada); en la app, `/data/user/0/com.agy/files/megadaemon/state/`.
pub fn providers_path() -> String {
    format!("{}/providers.json", crate::cockpit_server::state_dir())
}

/// Rutas de lectura, en orden: la actual y la histórica.
fn providers_read_candidates() -> Vec<String> {
    let mut v = vec![providers_path()];
    let legacy = PROVIDERS_PATH.to_string();
    if !v.contains(&legacy) {
        v.push(legacy);
    }
    v
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderEntry {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub endpoint: String,
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_auth_type")]
    pub auth_type: String, // "bearer", "anthropic", "gh_cli", "none", "local", "hybrid"
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub free_signup_url: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub supports_reasoning: bool,
}

fn default_auth_type() -> String {
    "bearer".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderVault {
    pub default_model: String,
    pub providers: Vec<ProviderEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResult {
    pub reply: String,
    pub reasoning: String,
    pub model_id: String,
    pub model_name: String,
    pub vendor: String,
    pub latency_ms: f64,
    pub tokens_used: u64,
    pub tools_used: Vec<String>,
    pub fallback_used: bool,
    /// Estado de la cuota del proveedor (snapshot MUY compacto).
    ///
    /// Viaja DENTRO de la respuesta: el agente sabe cuanto queda sin
    /// pedirlo y sin gastar un token en una llamada extra.
    /// `None` si no hay datos de cuota.
    #[serde(default)]
    pub quota: Option<serde_json::Value>,
    /// Traza de la cascada: quién se intentó y a quién se saltó (y por qué).
    ///
    /// v16: `fallback_used` sólo decía "no fue el proveedor que pediste".
    /// Con la traza, el cliente (y el agente) ve QUIÉN respondió y a QUIÉN
    /// se saltó (modelo retirado, cuota en pausa, sin credencial).
    /// `Vec::new()` = no hubo cascada (o no se pudo ni empezar).
    #[serde(default)]
    pub provider_trace: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// v16 — Errores TIPADOS, redacción de secretos, parseo PURO y transporte
//       INYECTABLE (⇒ la cascada se testea sin red y sin secretos)
// ─────────────────────────────────────────────────────────────────────────────

/// Tope duro de `max_tokens` que se envía a un proveedor.
///
/// v16: un `max_tokens` absurdo (10.000.000) hace que el proveedor
/// responda 400 y el daemon lo trata como un fallo del PROVEEDOR ⇒ cae al
/// siguiente sin avisar de que el problema era el `max_tokens`.
pub const MAX_TOKENS_CAP: u32 = 32_768;

/// `max_tokens` por defecto cuando el llamante no dice nada.
pub const DEFAULT_MAX_TOKENS: u32 = 4_096;

/// Timeout por defecto de UNA llamada a proveedor (segundos).
pub const DEFAULT_PROVIDER_TIMEOUT_SECS: u64 = 45;

/// Nº máximo de reintentos por proveedor (sólo 429/5xx/transporte/timeout).
pub const MAX_RETRIES: u32 = 2;

/// Error TIPADO de una llamada a proveedor.
///
/// v16: antes todo era `String`.  Con `String` no se puede distinguir
/// "esto se reintenta" de "esto no" (429/5xx vs 401/404), ni "te falta la
/// key" de "el proveedor está retirado", ni redactar el secreto de forma
/// fiable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    /// Fallo de transporte (DNS, TLS, conexión rechazada, corte).
    Transport { provider: String, detail: String },
    /// Timeout: el proveedor no respondió a tiempo.
    Timeout { provider: String, detail: String },
    /// Status HTTP >= 400 (con el status REAL: 401/402/404/429/5xx).
    Http { provider: String, status: u16, detail: String },
    /// Status 2xx pero cuerpo no interpretable.
    Parse { provider: String, detail: String },
    /// Falta la credencial (o no se pudo resolver).
    MissingKey { provider: String, detail: String },
    /// Proveedor o modelo RETIRADO del catálogo.
    Retired { provider: String, model: String, note: String },
    /// El presupuesto no permite la llamada.
    Budget { provider: String, detail: String },
}

impl ProviderError {
    /// Id del proveedor que falló.
    pub fn provider(&self) -> &str {
        match self {
            ProviderError::Transport { provider, .. }
            | ProviderError::Timeout { provider, .. }
            | ProviderError::Http { provider, .. }
            | ProviderError::Parse { provider, .. }
            | ProviderError::MissingKey { provider, .. }
            | ProviderError::Budget { provider, .. }
            | ProviderError::Retired { provider, .. } => provider.as_str(),
        }
    }

    /// Status HTTP, si lo hubo.
    pub fn status(&self) -> Option<u16> {
        match self {
            ProviderError::Http { status, .. } => Some(*status),
            _ => None,
        }
    }

    /// Etiqueta estable (para logs, trazas y tests).
    pub fn kind(&self) -> &'static str {
        match self {
            ProviderError::Transport { .. } => "transport",
            ProviderError::Timeout { .. } => "timeout",
            ProviderError::Http { .. } => "http",
            ProviderError::Parse { .. } => "parse",
            ProviderError::MissingKey { .. } => "missing_key",
            ProviderError::Retired { .. } => "retired",
            ProviderError::Budget { .. } => "budget",
        }
    }

    /// ¿Tiene sentido reintentar?
    ///
    /// Regla v16: **nunca** un 4xx (salvo 429 = rate limit) ni un error de
    /// credenciales.  Un 401/404 no mejora reintentando: se pierde tiempo y
    /// se puede disparar el *rate limit* del propio proveedor.
    pub fn retryable(&self) -> bool {
        match self {
            ProviderError::Transport { .. } | ProviderError::Timeout { .. } => true,
            ProviderError::Http { status, .. } => {
                *status == 429 || (500..=599).contains(status)
            }
            _ => false,
        }
    }

    /// El detalle (sin el prefijo del proveedor).
    pub fn detail(&self) -> &str {
        match self {
            ProviderError::Transport { detail, .. }
            | ProviderError::Timeout { detail, .. }
            | ProviderError::Http { detail, .. }
            | ProviderError::Parse { detail, .. }
            | ProviderError::MissingKey { detail, .. }
            | ProviderError::Budget { detail, .. } => detail.as_str(),
            ProviderError::Retired { note, .. } => note.as_str(),
        }
    }

    /// Devuelve una copia con el secreto sustituido por `***`.
    ///
    /// v16: el cuerpo de error de un proveedor puede ecoar la API key
    /// ("Invalid API key: sk-abc…").  Ese texto acababa dentro de `reply`
    /// (el fallback offline concatena los errores) y por tanto **en el
    /// cliente**.
    pub fn redacted(&self, secret: &str) -> ProviderError {
        let s = secret.trim();
        if s.is_empty() {
            return self.clone();
        }
        let mut e = self.clone();
        match &mut e {
            ProviderError::Transport { detail, .. }
            | ProviderError::Timeout { detail, .. }
            | ProviderError::Http { detail, .. }
            | ProviderError::Parse { detail, .. }
            | ProviderError::MissingKey { detail, .. }
            | ProviderError::Budget { detail, .. } => {
                *detail = redact_secret(detail.as_str(), s);
            }
            ProviderError::Retired { .. } => {}
        }
        e
    }
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderError::Transport { provider, detail } => {
                write!(f, "HTTP transport error ({}): {}", provider, detail)
            }
            ProviderError::Timeout { provider, detail } => {
                write!(f, "timeout ({}): {}", provider, detail)
            }
            ProviderError::Http {
                provider,
                status,
                detail,
            } => write!(f, "HTTP {} ({}): {}", status, provider, detail),
            ProviderError::Parse { provider, detail } => {
                write!(f, "respuesta ilegible ({}): {}", provider, detail)
            }
            ProviderError::MissingKey { provider, detail } => {
                write!(f, "sin credencial ({}): {}", provider, detail)
            }
            ProviderError::Retired {
                provider,
                model,
                note,
            } => write!(f, "MODELO RETIRADO: {} ({}) -> {}", model, provider, note),
            ProviderError::Budget { provider, detail } => {
                write!(f, "fuera de presupuesto ({}): {}", provider, detail)
            }
        }
    }
}

impl std::error::Error for ProviderError {}

/// Sustituye cualquier aparición del secreto por `***`.
///
/// Primero tapa las variantes CON prefijo ("Bearer sk-x") y después la
/// clave sola, para que un secreto pegado con prefijo tampoco escape.
pub fn redact_secret(text: &str, secret: &str) -> String {
    let s = secret.trim();
    if s.is_empty() {
        return text.to_string();
    }
    let mut out = text.to_string();
    for pre in ["Bearer ", "bearer ", "Basic ", "basic "] {
        let with = format!("{}{}", pre, s);
        if with != s {
            out = out.replace(&with, &format!("{}***", pre));
        }
    }
    out.replace(s, "***")
}

/// Redacta varios secretos a la vez.
pub fn redact_all(text: &str, secrets: &[&str]) -> String {
    let mut out = text.to_string();
    for s in secrets {
        out = redact_secret(&out, s);
    }
    out
}

/// Recorta a `n` caracteres SIN partir un UTF-8 por la mitad.
///
/// v16: `&texto[..texto.len().min(200)]` (el código anterior) hace PANIC
/// si el corte cae en medio de un carácter multibyte (un emoji en el
/// mensaje de error del proveedor tumbaba la petición).
pub fn truncar(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Tool call nativa del proveedor (OpenAI `tool_calls`).
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// Respuesta del proveedor ya interpretada.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParsedReply {
    /// Texto final para el cliente (sin el `<think>`).
    pub content: String,
    /// Razonamiento interno (de `reasoning_content` o del `<think>`).
    pub reasoning: String,
    /// Tokens consumidos (`usage.total_tokens`).
    pub tokens_used: u64,
    /// Tokens de entrada (`usage.prompt_tokens`).
    pub prompt_tokens: u64,
    /// Tokens de salida (`usage.completion_tokens`).
    pub completion_tokens: u64,
    /// Tool calls nativas que pidió el proveedor.
    pub tool_calls: Vec<ParsedToolCall>,
    /// `finish_reason` del proveedor (`stop`, `length`, `tool_calls`…).
    pub finish_reason: String,
}

/// Parsea la respuesta de un proveedor.  Función **PURO** (sin red, sin
/// estado global ⇒ se testea con el banco de fixtures).
///
/// v16: antes el parseo estaba embebido en `call_openai_http` y sólo se
/// podía probar contra un servidor real.  Aquí se mueve a una función
/// pura que cubre los cuerpos raros que dan los proveedores de verdad:
/// `<think>`, `reasoning_content`, `tool_calls`, `content` en array, body
/// no-JSON y respuestas truncadas.
pub fn parse_provider_reply(provider: &str, body: &str) -> Result<ParsedReply, ProviderError> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err(ProviderError::Parse {
            provider: provider.to_string(),
            detail: "respuesta HTTP vacía".to_string(),
        });
    }
    let v: Value = match serde_json::from_str::<Value>(trimmed) {
        Ok(v) => v,
        Err(_) => {
            // Algunos endpoints zero-key devuelven texto plano.
            return Ok(ParsedReply {
                content: trimmed.to_string(),
                ..Default::default()
            });
        }
    };
    // Un 2xx con cuerpo de error: el proveedor falló de todos modos.
    if let Some(err) = v.get("error") {
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .or_else(|| err.as_str())
            .unwrap_or("API error");
        return Err(ProviderError::Parse {
            provider: provider.to_string(),
            detail: format!("la respuesta trae 'error': {}", msg),
        });
    }
    let choice = v
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .cloned()
        .unwrap_or(Value::Null);
    let msg = choice.get("message").cloned().unwrap_or(Value::Null);

    // Contenido: `string` o `array` de partes (multimodal).
    let mut raw_content = match msg.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    };
    // Formato legacy: `choices[0].text` sin `message`.
    if raw_content.trim().is_empty() {
        raw_content = choice
            .get("text")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
    }

    let extra_reason = msg
        .get("reasoning_content")
        .and_then(|m| m.as_str())
        .or_else(|| msg.get("reasoning").and_then(|m| m.as_str()))
        .unwrap_or("")
        .to_string();
    let (clean, think_tag) = split_think_tags(&raw_content);
    let reasoning = if !extra_reason.trim().is_empty() {
        extra_reason
    } else {
        think_tag
    };

    let usage = v.get("usage").cloned().unwrap_or(Value::Null);
    let prompt_tokens = usage.get("prompt_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
    let completion_tokens = usage.get("completion_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
    let tokens_used = usage
        .get("total_tokens")
        .and_then(|x| x.as_u64())
        .unwrap_or_else(|| prompt_tokens.saturating_add(completion_tokens));

    let tool_calls = parse_tool_calls(msg.get("tool_calls"));
    let finish_reason = choice
        .get("finish_reason")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();

    if clean.trim().is_empty() && reasoning.trim().is_empty() && tool_calls.is_empty() {
        return Err(ProviderError::Parse {
            provider: provider.to_string(),
            detail: format!("respuesta sin contenido: {}", truncar(trimmed, 200)),
        });
    }
    // Si el proveedor sólo devuelve razonamiento, el razonamiento ES la
    // respuesta (igual que antes) pero sin duplicarlo.
    let (content, reasoning) = if clean.trim().is_empty() {
        (reasoning.clone(), String::new())
    } else {
        (clean, reasoning)
    };
    Ok(ParsedReply {
        content,
        reasoning,
        tokens_used,
        prompt_tokens,
        completion_tokens,
        tool_calls,
        finish_reason,
    })
}

/// Extrae las `tool_calls` nativas de un mensaje OpenAI.
fn parse_tool_calls(node: Option<&Value>) -> Vec<ParsedToolCall> {
    let arr = match node.and_then(|n| n.as_array()) {
        Some(a) => a,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    for c in arr {
        let f = c.get("function").cloned().unwrap_or(Value::Null);
        let name = f
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            continue;
        }
        let args = match f.get("arguments") {
            Some(Value::String(s)) => {
                serde_json::from_str::<Value>(s).unwrap_or(Value::String(s.clone()))
            }
            Some(v) => v.clone(),
            None => Value::Object(serde_json::Map::new()),
        };
        out.push(ParsedToolCall {
            id: c.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string(),
            name,
            arguments: args,
        });
    }
    out
}

/// Saca el mensaje de error del cuerpo de una respuesta (JSON o texto).
pub fn extract_error_detail(body: &str) -> String {
    let t = body.trim();
    if let Ok(v) = serde_json::from_str::<Value>(t) {
        if let Some(e) = v.get("error") {
            let m = e
                .get("message")
                .and_then(|m| m.as_str())
                .or_else(|| e.as_str())
                .map(|s| s.to_string());
            if let Some(m) = m {
                return truncar(&m, 200);
            }
        }
        for campo in ["message", "detail"] {
            if let Some(m) = v.get(campo).and_then(|m| m.as_str()) {
                return truncar(m, 200);
            }
        }
    }
    truncar(t, 200)
}

/// Convierte un status HTTP + cuerpo en un error TIPADO (y redactable).
///
/// v16: el status se conserva (antes se perdía: `ureq` convertía los 4xx
/// en `Err` y el diagnóstico de 401/402/404/429 era código muerto).
pub fn parse_provider_error(provider: &str, status: u16, body: &str) -> ProviderError {
    let detail = extract_error_detail(body);
    let hint = match status {
        401 | 403 => " (API key invalida o sin permisos)",
        402 => " (sin saldo / plan agotado)",
        404 => " (modelo inexistente o RETIRADO - mira el catalogo)",
        429 => " (rate limit o cuota diaria agotada)",
        _ => "",
    };
    ProviderError::Http {
        provider: provider.to_string(),
        status,
        detail: format!("{}{}", detail, hint),
    }
}

/// Petición HTTP a un proveedor (todo lo que necesita el transporte).
#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub endpoint: String,
    pub model: String,
    pub api_key: String,
    pub auth_type: String,
    pub body: Value,
}

/// Respuesta HTTP cruda de un proveedor.
#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub status: u16,
    pub body: String,
}

/// Transporte HTTP **inyectable**.
///
/// v16: hasta ahora `call_openai_http` hacía el POST in situ con `ureq`, lo
/// que hacía IMPOSIBLE testear la cascada, el fallback y el parseo sin
/// red.  Con este trait, los tests inyectan un transporte de mentira y la
/// suite es determinista y offline.
pub trait LlmTransport: Send + Sync {
    /// Ejecuta el POST.  Devuelve el status y el cuerpo SIEMPRE que haya
    /// respuesta HTTP (incluido 4xx/5xx): el status lo interpreta la capa
    /// de arriba.
    fn post(&self, req: &LlmRequest, timeout: Duration) -> Result<LlmResponse, ProviderError>;

    /// Nombre del transporte (para logs).
    fn name(&self) -> &'static str {
        "transport"
    }
}

/// Transporte real (ureq + rustls).  Sin curl, sin fork/exec.
#[derive(Debug, Clone)]
pub struct UreqTransport {
    /// Timeout de conexión.
    pub connect_timeout: Duration,
    /// Timeout de escritura.
    pub write_timeout: Duration,
}

impl Default for UreqTransport {
    fn default() -> Self {
        UreqTransport {
            connect_timeout: Duration::from_secs(10),
            write_timeout: Duration::from_secs(10),
        }
    }
}

impl LlmTransport for UreqTransport {
    fn name(&self) -> &'static str {
        "ureq"
    }

    fn post(&self, req: &LlmRequest, timeout: Duration) -> Result<LlmResponse, ProviderError> {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .timeout_connect(Some(self.connect_timeout))
            .timeout_per_write(Some(self.write_timeout))
            // v16 (DEFECTO-01): por defecto `ureq` convierte 4xx/5xx en
            // `Err`, y entonces el código NUNCA llegaba al bloque que
            // distingue 401 (key mala) de 402 (sin saldo) de 404 (modelo
            // muerto) de 429 (rate limit): el diagnóstico era CÓDIGO MUERTO
            // y `quota_guard` nunca veía el status (no pausaba nunca).
            .http_status_as_error(false)
            .build();
        let agent = ureq::Agent::new_with_config(config);

        let mut r = agent.post(&req.endpoint);
        let key = req.api_key.trim();
        match req.auth_type.as_str() {
            "anthropic" => {
                if !key.is_empty() {
                    r = r.header("x-api-key", key);
                }
                r = r.header("anthropic-version", "2023-06-01");
            }
            "none" | "local" => {}
            _ => {
                if !key.is_empty() {
                    r = r.header("Authorization", &format!("Bearer {}", key));
                }
            }
        }
        r = r
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://github.com/boolforge/megadaemon")
            .header("X-Title", "MegaDaemon Zenith v16");

        let body_bytes = serde_json::to_vec(&req.body).map_err(|e| ProviderError::Transport {
            provider: req.model.clone(),
            detail: format!("cuerpo no serializable: {}", e),
        })?;

        let resp = match r.send(body_bytes.as_slice()) {
            Ok(resp) => resp,
            Err(e) => return Err(map_ureq_error(&req.model, e)),
        };
        let status = resp.status().as_u16();
        let text = resp.into_body().read_to_string().map_err(|e| {
            ProviderError::Transport {
                provider: req.model.clone(),
                detail: format!("HTTP body read error: {}", e),
            }
        })?;
        Ok(LlmResponse { status, body: text })
    }
}

/// Traduce un error de `ureq` al error tipado.
fn map_ureq_error(provider: &str, e: ureq::Error) -> ProviderError {
    match e {
        ureq::Error::StatusCode(code) => ProviderError::Http {
            provider: provider.to_string(),
            status: code,
            detail: String::new(),
        },
        ureq::Error::Timeout(_) => ProviderError::Timeout {
            provider: provider.to_string(),
            detail: "el proveedor no respondió a tiempo".to_string(),
        },
        ureq::Error::Io(io) => {
            if io.kind() == std::io::ErrorKind::TimedOut {
                ProviderError::Timeout {
                    provider: provider.to_string(),
                    detail: io.to_string(),
                }
            } else {
                ProviderError::Transport {
                    provider: provider.to_string(),
                    detail: io.to_string(),
                }
            }
        }
        other => ProviderError::Transport {
            provider: provider.to_string(),
            detail: other.to_string(),
        },
    }
}

/// Todo lo que la cascada necesita, **inyectado** (⇒ testeable sin red).
///
/// v16: `execute_chat` leía el vault de disco y hacía el POST con `ureq`
/// ⇒ no se podía testear ni un caso.  Ahora el núcleo (`execute_chat_with`)
/// recibe el vault y el transporte; `execute_chat` sigue siendo la façade
/// de producción con los mismos argumentos.
pub struct ChatDeps<'a> {
    /// El vault (proveedores + modelo por defecto).
    pub vault: &'a ProviderVault,
    /// Transporte HTTP.
    pub transport: &'a dyn LlmTransport,
    /// ¿Este proveedor está en pausa por cuota?  Por defecto `quota_guard`.
    pub quota_paused: fn(&str) -> bool,
    /// Predicado opcional de presupuesto por proveedor y `max_tokens`.
    /// Si es `None`, se aplica `daily_budget_permite` (presupuesto diario USD).
    pub budget_permite: Option<&'a dyn Fn(&ProviderEntry, u32) -> bool>,
    /// Timeout de UNA llamada a proveedor.
    pub timeout: Duration,
    /// Nº de reintentos por proveedor (429/5xx/transporte/timeout).
    pub retries: u32,
    /// Espera base entre reintentos (ms).  0 en tests (sin esperas).
    pub backoff_ms: u64,
}

impl<'a> ChatDeps<'a> {
    /// Dependencias de producción (timeout y reintentos por defecto).
    pub fn new(vault: &'a ProviderVault, transport: &'a dyn LlmTransport) -> Self {
        ChatDeps {
            vault,
            transport,
            quota_paused: crate::quota_guard::en_pausa,
            budget_permite: None,
            timeout: Duration::from_secs(DEFAULT_PROVIDER_TIMEOUT_SECS),
            retries: MAX_RETRIES,
            backoff_ms: 250,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct DailyBudgetState {
    day_epoch: u64,
    limit_usd: Option<f64>,
    spend_usd: f64,
    tokens_total: u64,
    calls_total: u64,
    unpriced_calls: u64,
}

impl DailyBudgetState {
    fn current_day() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() / 86_400)
            .unwrap_or(0)
    }

    fn rollover_if_needed(&mut self) {
        let today = Self::current_day();
        if self.day_epoch != today {
            self.day_epoch = today;
            self.spend_usd = 0.0;
            self.tokens_total = 0;
            self.calls_total = 0;
            self.unpriced_calls = 0;
        }
    }
}

fn daily_budget_cell() -> &'static std::sync::Mutex<DailyBudgetState> {
    static CELL: std::sync::OnceLock<std::sync::Mutex<DailyBudgetState>> =
        std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        let limit_usd = std::env::var("MEGADAEMON_DAILY_BUDGET_USD")
            .ok()
            .and_then(|s| s.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v >= 0.0);
        std::sync::Mutex::new(DailyBudgetState {
            day_epoch: DailyBudgetState::current_day(),
            limit_usd,
            ..Default::default()
        })
    })
}

/// Configura el límite de gasto diario en USD (`None` = sin límite).
pub fn set_daily_budget_usd(limit: Option<f64>) {
    let mut g = daily_budget_cell().lock().unwrap_or_else(|p| p.into_inner());
    g.rollover_if_needed();
    g.limit_usd = limit.filter(|v| v.is_finite() && *v >= 0.0);
}

/// Reinicia los contadores del presupuesto diario (útil en tests).
pub fn reset_daily_budget() {
    let mut g = daily_budget_cell().lock().unwrap_or_else(|p| p.into_inner());
    g.day_epoch = DailyBudgetState::current_day();
    g.spend_usd = 0.0;
    g.tokens_total = 0;
    g.calls_total = 0;
    g.unpriced_calls = 0;
}

/// Indica si el presupuesto diario permite llamar a `p`.
/// Los proveedores gratuitos/locales (`tier` `free`/`local` o `auth_type` `none`/`local`)
/// nunca se bloquean por el tope USD.
pub fn daily_budget_permite(p: &ProviderEntry, _max_tokens: u32) -> bool {
    if p.tier.eq_ignore_ascii_case("free")
        || p.tier.eq_ignore_ascii_case("local")
        || p.auth_type == "local"
        || p.auth_type == "none"
    {
        return true;
    }
    let mut g = daily_budget_cell().lock().unwrap_or_else(|p| p.into_inner());
    g.rollover_if_needed();
    match g.limit_usd {
        Some(cap) if cap <= 0.0 => false,
        Some(cap) => g.spend_usd < cap,
        None => true,
    }
}

/// Registra el uso de tokens y el coste calculado con `price_table::fleet_cost`.
pub fn record_daily_spend(
    _provider_id: &str,
    model: &str,
    tokens_in: u64,
    tokens_out: u64,
    total_tokens: u64,
) -> Option<f64> {
    let cost = crate::price_table::fleet_cost(model, tokens_in, tokens_out);
    let mut g = daily_budget_cell().lock().unwrap_or_else(|p| p.into_inner());
    g.rollover_if_needed();
    g.tokens_total = g.tokens_total.saturating_add(total_tokens);
    g.calls_total = g.calls_total.saturating_add(1);
    if let Some(usd) = cost.usd {
        g.spend_usd += usd;
    } else {
        g.unpriced_calls = g.unpriced_calls.saturating_add(1);
    }
    cost.usd
}

/// Snapshot JSON del presupuesto y gasto diario actual.
pub fn daily_budget_snapshot() -> Value {
    let mut g = daily_budget_cell().lock().unwrap_or_else(|p| p.into_inner());
    g.rollover_if_needed();
    let remaining = g.limit_usd.map(|cap| (cap - g.spend_usd).max(0.0));
    json!({
        "day_epoch": g.day_epoch,
        "limit_usd": g.limit_usd,
        "spend_usd": g.spend_usd,
        "remaining_usd": remaining,
        "tokens_total": g.tokens_total,
        "calls_total": g.calls_total,
        "unpriced_calls": g.unpriced_calls,
    })
}

fn default_vault() -> ProviderVault {
    ProviderVault {
        // El default es el LOCAL: gratis, offline y sin latencia de red.  La
        // nube entra como failover.  En v13.0 el default era "local-seere"
        // pero el catálogo sólo tenía 2 entradas, así que la UI mostraba
        // "minimax-m2.7" (hardcodeado en el HTML) y ese modelo estaba
        // RETIRADO desde el 2026-11-18.
        default_model: "local-seere".to_string(),
        providers: provider_catalog::catalog_v13_1(),
    }
}

pub fn load_vault() -> ProviderVault {
    for path in providers_read_candidates() {
        let raw = match fs::read_to_string(&path) {
            Ok(r) => r,
            Err(_) => continue,
        };
        match serde_json::from_str::<ProviderVault>(&raw) {
            Ok(v) => return v,
            Err(e) => {
                // BUG v13.0: un providers.json corrupto devolvía el vault por
                // defecto y el siguiente save_vault() SOBRESCRIBÍA el
                // fichero, perdiendo TODAS las API keys del usuario.
                // Ahora se hace una copia .corrupt-<ts> y se avisa.
                let bak = format!("{}.corrupt-{}", path, ts_now());
                let _ = fs::write(&bak, &raw);
                eprintln!(
                    "[providers] providers.json corrupto ({}); copia en {}; se usa el catálogo por defecto",
                    e, bak
                );
                // No se sigue con el siguiente candidato: el fichero existe
                // pero está roto => avisar, no seguir leyendo el respaldo.
                break;
            }
        }
    }
    default_vault()
}

fn ts_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn save_vault(vault: &ProviderVault) -> Result<(), String> {
    let path = providers_path();
    if let Some(parent) = Path::new(&path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    let text = serde_json::to_string_pretty(vault).map_err(|e| e.to_string())?;
    // v16 (DEFECTO-07): escritura ATÓMICA (tmp + rename).  Antes
    // `fs::write` truncaba el fichero y escribía encima: un corte de luz
    // (o un OOM kill) en mitad dejaba un `providers.json` truncado ⇒
    // `load_vault` lo declaraba corrupto y el daemon se quedaba SIN las
    // API keys del usuario (y el siguiente `save_vault` las pisaba).
    let tmp = format!("{}.tmp-{}", path, std::process::id());
    fs::write(&tmp, text.as_bytes()).map_err(|e| format!("no se pudo escribir {}: {}", tmp, e))?;
    if let Err(e) = fs::rename(&tmp, &path) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("no se pudo renombrar {} -> {}: {}", tmp, path, e));
    }
    Ok(())
}

/// Valida el endpoint de un proveedor.
///
/// v16 (DEFECTO-06): `update_provider` aceptaba CUALQUIER endpoint
/// (`file://…`, `http://169.254.169.254/…` = metadatos de la nube) y el
/// endpoint HTTP del cockpit **no tiene autenticación** ⇒ SSRF + robo de
/// credenciales.  Ahora sólo se admiten `http`/`https`, sin credenciales
/// embebidas en la URL y sin el endpoint de metadatos.
pub fn validar_endpoint(ep: &str) -> Result<String, String> {
    let s = ep.trim();
    if s.is_empty() {
        return Err("endpoint vacío".to_string());
    }
    let lower = s.to_ascii_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return Err(format!("endpoint no permitido (sólo http/https): {}", s));
    }
    // Credenciales embebidas en la URL (http://user:pass@host).
    let rest = match lower.strip_prefix("http://") {
        Some(r) => r,
        None => lower.strip_prefix("https://").unwrap_or(""),
    };
    if let Some(at) = rest.find('@') {
        if rest[..at].contains(':') {
            return Err("el endpoint no puede llevar credenciales embebidas".to_string());
        }
    }
    // Metadatos de la nube: no es ningún proveedor de LLM.
    if lower.contains("169.254.169.254") || lower.contains("metadata.google.internal") {
        return Err("endpoint de metadatos bloqueado (SSRF)".to_string());
    }
    Ok(s.to_string())
}

fn mask_key(k: &str) -> String {
    if k.is_empty() {
        return String::new();
    }
    if k == "AUTO_GH_CLI" {
        return "AUTO (gh auth token)".to_string();
    }
    // v13.2 (HW-12): antes se revelaban 6+4 = 10 caracteres (casi toda la
    // key).  Ahora como máximo 2+2.
    let chars: Vec<char> = k.chars().collect();
    if chars.len() < 12 {
        return "***".to_string();
    }
    let head: String = chars[..2].iter().collect();
    let tail: String = chars[chars.len() - 2..].iter().collect();
    format!("{}...{}", head, tail)
}

pub fn list_providers_redacted() -> Value {
    let vault = load_vault();
    let gh_ready = resolve_gh_token().is_some();
    let list: Vec<Value> = vault
        .providers
        .iter()
        .map(|p| {
            let ready = match p.auth_type.as_str() {
                "local" | "none" | "hybrid" => true,
                "gh_cli" => gh_ready,
                _ => !p.api_key.trim().is_empty(),
            };
            json!({
                "id": p.id,
                "name": p.name,
                "vendor": p.vendor,
                "endpoint": p.endpoint,
                "model": p.model,
                "auth_type": p.auth_type,
                "tier": p.tier,
                "free_signup_url": p.free_signup_url,
                "enabled": p.enabled,
                "ready": ready,
                "has_key": !p.api_key.trim().is_empty() || p.auth_type == "gh_cli",
                "key_masked": mask_key(&p.api_key),
                "supports_reasoning": p.supports_reasoning,
            })
        })
        .collect();
    let retired: Vec<Value> = provider_catalog::retired_models()
        .iter()
        .map(|(id, model, note)| json!({"id": id, "model": model, "note": note}))
        .collect();
    json!({
        "default_model": vault.default_model,
        "providers": list,
        "retired": retired,
        "catalog_size": vault.providers.len(),
        "free_providers": vault.providers.iter().filter(|p| provider_catalog::tier_free(&p.tier)).count(),
        "keyless_providers": vault.providers.iter().filter(|p| p.tier == "keyless").count(),
        "engine": "rust-swarm-v13.1"
    })
}

pub fn update_provider(
    id: &str,
    api_key: Option<&str>,
    model: Option<&str>,
    endpoint: Option<&str>,
    enabled: Option<bool>,
    set_default: bool,
) -> Result<Value, String> {
    // v16 (DEFECTO-06): se VALIDA antes de tocar NADA.  Antes el endpoint
    // se aceptaba tal cual y el modelo se comprobaba contra el RETIRADO
    // por ID (no por MODELO): `update_provider(id, model="MiniMax-M2.7")`
    // se aceptaba y el vault quedaba con un modelo muerto.
    let nuevo_endpoint: Option<String> = match endpoint {
        Some(ep) if !ep.trim().is_empty() => Some(validar_endpoint(ep)?),
        _ => None,
    };
    if let Some(m) = model {
        let mt = m.trim();
        if !mt.is_empty() {
            if let Some((rid, rmodel, rnote)) = provider_catalog::is_retired(mt) {
                return Err(format!("MODELO RETIRADO: {} ({}) -> {}", rmodel, rid, rnote));
            }
        }
    }
    let mut vault = load_vault();
    let mut found = false;
    for p in &mut vault.providers {
        if p.id == id {
            if let Some(k) = api_key {
                p.api_key = k.trim().to_string();
                if !p.api_key.is_empty() {
                    p.enabled = true;
                    p.tier = "configured".to_string();
                }
            }
            if let Some(m) = model {
                if !m.trim().is_empty() {
                    p.model = m.trim().to_string();
                }
            }
            if let Some(nuevo) = nuevo_endpoint.as_ref() {
                // v13.2 (HW-03): cambiar el endpoint de un proveedor que ya
                // tiene una key guardada enviaba esa key al host nuevo
                // (SSRF + robo de credenciales) sin pedir la key.
                // Ahora hay que re-introducir la key al cambiar de host.
                //
                // v13.2 (HW-03): la API key queda ATADA al endpoint
                // completo (esquema+host+ruta), no sólo al host.  El
                // endpoint HTTP del cockpit no tiene autenticación, así
                // que sin esta regla cualquiera podía re-apuntar un
                // proveedor con key guardada yeysar esa key (SSRF +
                // robo de credenciales).  Al cambiar el endpoint, la
                // key se vacía y hay que re-introducirla.
                if p.endpoint.trim() != nuevo.as_str() && !p.api_key.trim().is_empty() {
                    p.api_key = String::new();
                    p.auth_type = "none".to_string();
                }
                p.endpoint = nuevo.to_string();
            }
            if let Some(en) = enabled {
                p.enabled = en;
            }
            found = true;
            break;
        }
    }
    // Si el usuario pide un modelo RETIRADO, se avisa en lugar de fallar
    // en silencio contra la API (M2.7/M2.5 se retiró el 2026-11-18; GitHub Models
    // se retiró el 2026-07-30).
    if let Some((rid, rmodel, rnote)) = provider_catalog::is_retired(id) {
        return Err(format!(
            "MODELO RETIRADO: {} ({}) -> {}",
            rmodel, rid, rnote
        ));
    }
    if !found && !id.is_empty() {
        // Allow creating custom OpenAI-compatible provider dynamically
        vault.providers.push(ProviderEntry {
            id: id.to_string(),
            name: format!("Custom ({})", id),
            vendor: "Custom".to_string(),
            endpoint: nuevo_endpoint.clone().unwrap_or_else(|| {
                "https://openrouter.ai/api/v1/chat/completions".to_string()
            }),
            model: model.unwrap_or("openai/gpt-4o-mini").trim().to_string(),
            api_key: api_key.unwrap_or("").trim().to_string(),
            auth_type: "bearer".to_string(),
            tier: "custom".to_string(),
            free_signup_url: String::new(),
            enabled: enabled.unwrap_or(true),
            supports_reasoning: true,
        });
    }
    if set_default && !id.is_empty() {
        vault.default_model = id.to_string();
    }
    save_vault(&vault)?;
    Ok(list_providers_redacted())
}

use std::sync::Mutex;
use std::sync::OnceLock;

static GH_TOKEN_CACHE: OnceLock<Mutex<Option<(u64, String)>>> = OnceLock::new();

/// Token de `gh auth token`, cacheado 300 s.
///
/// BUG v13.0: se ejecutaba `gh auth token` en CADA llamada (y en cada
/// proveedor de la cascada): un fork+exec por petición.
fn resolve_gh_token() -> Option<String> {
    let cache = GH_TOKEN_CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some((ts, tok)) = guard.as_ref() {
            if ts_now().saturating_sub(*ts) < 300 {
                return Some(tok.clone());
            }
        }
    }
    let fresh = Command::new("gh")
        .args(["auth", "token"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty());
    if let Ok(mut guard) = cache.lock() {
        *guard = fresh.as_ref().map(|t| (ts_now(), t.clone()));
    }
    fresh
}

fn split_think_tags(raw: &str) -> (String, String) {
    if let Some(start) = raw.find("<think>") {
        if let Some(end) = raw.find("</think>") {
            if end > start + 7 {
                let thought = raw[start + 7..end].trim().to_string();
                let before = raw[..start].trim();
                let after = raw[end + 8..].trim();
                let clean = if before.is_empty() {
                    after.to_string()
                } else if after.is_empty() {
                    before.to_string()
                } else {
                    format!("{}\n{}", before, after)
                };
                return (clean, thought);
            }
        }
    }
    (raw.trim().to_string(), String::new())
}

/// Cliente HTTPS NATIVO (ureq + rustls).  Sin curl, sin fork/exec, sin
/// ficheros temporales con la API key en disco.
///
/// BUGS QUE ARREGLA (v13.0 -> v13.1):
///   * v13.0 hacía `Command::new("curl")` en CADA petición: un fork+exec por
///     llamada, y en Android `curl` NO existe en el toybox de AOSP (el
///     agente no podía hablar con la nube sin Termux).
///   * v13.0 escribía la API key en `/data/user/0/com.agy/files/tmp/md_curl_*.cfg`
///     (ruta HARDCODEADA) y la borraba al terminar.  Si el write fallaba
///     (directorio sin permisos), la petición salía SIN cabecera de
///     autenticación y el error que veía el usuario era "HTTP transport
///     error", no "401 Unauthorized".
///   * v13.0 no miraba el status HTTP: un 401/402/429 se parseaba como si
///     fuera una respuesta vacía.
///   * v13.0 no distinguía "quedate sin saldo" de "el modelo no existe":
///     un 402 (credit exhausted) y un 404 (model retired) se mezclaban.
pub fn call_openai_http(
    endpoint: &str,
    model: &str,
    api_key: &str,
    auth_type: &str,
    system_prompt: &str,
    user_prompt: &str,
    max_tokens: u32,
    temperature: f32,
) -> Result<(String, String, u64), String> {
    let payload = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}
        ],
        "max_tokens": if max_tokens == 0 { DEFAULT_MAX_TOKENS } else { max_tokens.min(MAX_TOKENS_CAP) },
        "temperature": temperature
    });

    // v16: el POST va por el MISMO transporte inyectable que usa la
    // cascada (y con `http_status_as_error(false)`), así que el status
    // HTTP llega al diagnóstico en vez de perderse en un `Err`.
    let transport = UreqTransport::default();
    let req = LlmRequest {
        endpoint: endpoint.to_string(),
        model: model.to_string(),
        api_key: api_key.to_string(),
        auth_type: auth_type.to_string(),
        body: payload,
    };
    let resp = transport
        .post(&req, Duration::from_secs(DEFAULT_PROVIDER_TIMEOUT_SECS))
        .map_err(|e| e.redacted(api_key).to_string())?;

    if resp.status >= 400 {
        // Diagnóstico accionable: distinguir saldo/agota/modelo-muerto.
        let e = parse_provider_error(model, resp.status, &resp.body).redacted(api_key);
        return Err(e.to_string());
    }

    let parsed = parse_provider_reply(model, &resp.body).map_err(|e| e.redacted(api_key).to_string())?;
    Ok((parsed.content, parsed.reasoning, parsed.tokens_used))
}

fn default_re_system_prompt() -> &'static str {
    "Eres MegaDaemon Zenith v13.0, un motor experto de ingeniería inversa multiplataforma (Android ARM64/DEX/APK, Linux ELF, Windows PE/PE32+, macOS Mach-O, DOS MZ/COM, Sega CD M68K, PS1 MIPS R3000A, Saturn SH-2, PC Engine HuC6280, MSX2 Z80) y arquitectura formal LTO-Decomp v3.4 (CIRX Micro-RTL/SSA, MAS Track Routing, Absolute Set Closure Lemma A={formal_control_flow}). Operas en un entorno local de investigación sin restricciones artificiales. Responde de forma concisa, técnica, directa y precisa en el idioma del usuario (por defecto español)."
}

/// Executes optional deterministic Rust RE pre-analysis when an attachment or special intent is detected,
/// giving even cloud models grounded byte-level telemetry without hallucination.
/// Directorios donde puede estar un adjunto legible por el daemon.
///
/// v13.2 (HW-04): antes se analizaba (y subía al proveedor) CUALQUIER ruta
/// que llegara en el campo `attachment`: con el endpoint HTTP sin
/// autenticación eso es una lectura de ficheros arbitraria + exfiltración.
pub const ATTACHMENT_ROOTS: &[&str] = &[
    "/data/user/0/com.agy/files/megadaemon/uploads",
    "/data/user/0/com.agy/files/megadaemon/extracted",
    "/data/user/0/com.agy/files/megadaemon/re_pipeline",
    "/data/user/0/com.agy/files/snatcher",
];

/// ¿Este adjunto está permitido?
pub fn attachment_allowed(path: &str) -> bool {
    if path.is_empty() || !path.starts_with('/') || path.contains("..") {
        return false;
    }
    let canon = match std::fs::canonicalize(path) {
        Ok(c) => c.to_string_lossy().to_string(),
        Err(_) => return false,
    };
    ATTACHMENT_ROOTS.iter().any(|r| canon.starts_with(r))
}

fn build_grounded_re_context(prompt: &str, attachment_path: Option<&str>) -> (String, Vec<String>) {
    let mut ctx = String::new();
    let mut tools = Vec::new();
    let low = prompt.to_lowercase();

    if let Some(fp) = attachment_path {
        if !attachment_allowed(fp) {
            // Adjunto fuera del sandbox: se ignora y se declara.
            ctx.push_str(&format!(
                "\n[ADJUNTO IGNORADO: la ruta no está dentro de las carpetas permitidas del daemon: {}]\n",
                fp
            ));
            tools.push(format!("AttachmentRejected({})", fp));
        } else if !fp.is_empty() && Path::new(fp).exists() {
            let die = binwalk_die::analyze_binary_full(fp);
            tools.push(format!("DiE+Binwalk({})", die.format_family));
            ctx.push_str(&format!(
                "\n[TELEMETRÍA NATIVA RUST DiE/BINWALK DEL ARCHIVO ADJUNTO]\n\
                 Archivo: {} ({} bytes)\n\
                 Formato/OS: {} | Arch: {} | Empaquetador/Compilador: {}\n\
                 Entropía Global: {:.3} bits/B | Firmas Embebidas: {} | Comtype hits: {}\n",
                die.filename,
                die.file_size,
                die.format_family,
                die.architecture,
                die.packer_or_compiler,
                die.global_entropy,
                die.embedded_signatures.len(),
                die.comtype_candidates.len()
            ));
            if !die.sections.is_empty() {
                let sec_names: Vec<String> = die
                    .sections
                    .iter()
                    .take(8)
                    .map(|s| format!("{}({}B,H={:.2})", s.name, s.size, s.entropy))
                    .collect();
                ctx.push_str(&format!("Secciones: {}\n", sec_names.join(", ")));
            }
            if !die.bulk_artifacts.is_empty() {
                let arts: Vec<String> = die
                    .bulk_artifacts
                    .iter()
                    .take(8)
                    .map(|a| format!("{}:{}", a.kind, a.value))
                    .collect();
                ctx.push_str(&format!("Bulk Extractor: {}\n", arts.join(" | ")));
            }
            // If text-like file (.md, .py, .rs, .c, .js, .json, .txt, .asm), include first 4KB verbatim!
            if let Ok(bytes) = fs::read(fp) {
                let sample_len = bytes.len().min(4096);
                let printable = bytes[..sample_len]
                    .iter()
                    .filter(|&&b| b == b'\n' || b == b'\r' || b == b'\t' || (32..=126).contains(&b))
                    .count();
                if sample_len > 0 && (printable * 100 / sample_len) >= 85 {
                    let txt = String::from_utf8_lossy(&bytes[..sample_len]);
                    ctx.push_str(&format!("\n[CONTENIDO TEXTUAL DEL ADJUNTO (primeros {} B)]\n{}\n", sample_len, txt));
                    tools.push("TextIngest".to_string());
                }
            }
        }
    }

    if low.contains("npu") || low.contains("exynos") || low.contains("temperatura") || low.contains("thermal") {
        let npu = npu_bridge::probe_exynos_npu();
        tools.push("NPUProbe".to_string());
        ctx.push_str(&format!(
            "\n[TELEMETRÍA EXYNOS 2100 EN VIVO]\nVS4L /dev/vertex10: {} | EDEN dlopen: {} | CPU Temp: {:.1}C | Batt: {:.1}C | Freqs MHz: {:?}\n",
            npu.vs4l_present, npu.eden_runtime.dlopen_ok, npu.cpu_temp_c, npu.battery_temp_c, npu.cpu_freqs_mhz
        ));
    }

    if low.contains("cirx") || low.contains("lto-decomp") || low.contains("ssa") {
        let rep = cirx_rtl::run_reference_pipeline(0.75, 10, 0.05, 3, 1, 0);
        tools.push("CIRX-SSA-v3.4".to_string());
        ctx.push_str(&format!(
            "\n[LTO-DECOMP v3.4 CIRX SSA VERIFICADO]\nTrack: {} | Risk R={} (S={}) | Blocks: {} | Phis: {} | Closure Lemma: {}\n",
            rep.track, rep.risk_r, rep.risk_s, rep.ssa_blocks, rep.phi_nodes, rep.closure_lemma_status
        ));
    }

    if low.contains("mba") || low.contains("desofusca") || low.contains("ollvm") {
        if let Some(expr_idx) = prompt.find('(') {
            let candidate = &prompt[expr_idx..];
            let simp = kaitai_imhex::simplify_mba_expression(candidate);
            if simp.rules_applied > 0 {
                tools.push("MBA-Deobf".to_string());
                ctx.push_str(&format!(
                    "\n[SIMPLIFICADOR MBA D810/GOOMBA]\nEntrada: {}\nSalida: {}\nReglas: {:?}\n",
                    simp.original, simp.simplified, simp.steps
                ));
            }
        }
    }

    (ctx, tools)
}

/// Master Pure Rust Chat Orchestrator — handles explicit model selection, Hybrid Rust-Tools+Cloud,
/// Local SEERE v12 offline execution, and automatic zero-downtime fallback!
pub fn execute_chat(
    prompt: &str,
    requested_model: Option<&str>,
    attachment_path: Option<&str>,
    system_override: Option<&str>,
    max_tokens: u32,
) -> ChatResult {
    // Fachada de producción: vault de disco + transporte real.
    let vault = load_vault();
    let transport = UreqTransport::default();
    let deps = ChatDeps::new(&vault, &transport);
    execute_chat_with(
        prompt,
        requested_model,
        attachment_path,
        system_override,
        max_tokens,
        &deps,
    )
}

/// Núcleo de la cascada con TODO inyectado (vault + transporte + cuota).
///
/// v16: `execute_chat` leía el vault de disco y hacía el POST con `ureq`
/// ⇒ ningún caso de la cascada era testeable.  Aquí todo es explícito y
/// la firma pública de `execute_chat` no cambia.
pub fn execute_chat_with(
    prompt: &str,
    requested_model: Option<&str>,
    attachment_path: Option<&str>,
    system_override: Option<&str>,
    max_tokens: u32,
    deps: &ChatDeps<'_>,
) -> ChatResult {
    let t0 = Instant::now();
    let vault = deps.vault;
    let target_id: &str = match requested_model.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        Some(s) => s,
        None => vault.default_model.as_str(),
    };

    // v13.2 (HW-13): pedir un modelo RETIRADO no puede terminar en otro
    // proveedor sin avisar.  Se responde con el aviso y el sustituto.
    if let Some((rid, rmodel, rnote)) = provider_catalog::is_retired(target_id) {
        return ChatResult {
            reply: format!(
                "MODELO RETIRADO: {} ({}) -> {}. El daemon NO lo sustituye por otro \
                 proveedor en silencio; elige un modelo vigente.",
                rmodel, rid, rnote
            ),
            reasoning: String::new(),
            model_id: rid.to_string(),
            model_name: rmodel.to_string(),
            vendor: "catalog".to_string(),
            latency_ms: (t0.elapsed().as_secs_f64() * 1000.0).round(),
            tokens_used: 0,
            tools_used: Vec::new(),
            fallback_used: false,
            quota: None,
            provider_trace: vec![format!("{} => SALTADO (modelo retirado)", rid)],
        };
    }

    let (re_ctx, mut tools_used) = build_grounded_re_context(prompt, attachment_path);
    let enriched_prompt = if re_ctx.is_empty() {
        prompt.to_string()
    } else {
        format!("{}\n\n[PREGUNTA DEL USUARIO]\n{}", re_ctx, prompt)
    };

    // Explicit Local SEERE v12 offline path
    if target_id == "local-seere" || target_id == "local" {
        // v13.2 (S09): el fallback local respeta la política de energía.
        let (_, pol) = crate::power_policy::current_policy();
        let res = seere_v11::run_seere_v11(&enriched_prompt, pol.max_turns.max(1), true, false, false, true, true);
        let ans = res["answer"].as_str().unwrap_or("Sin respuesta local").to_string();
        if let Some(arr) = res["tools"].as_array() {
            for t in arr {
                let tn = t["tool"].as_str().unwrap_or("TOOL");
                let ta = t["arg"].as_str().unwrap_or("");
                tools_used.push(format!("{}:{}", tn, ta));
            }
        }
        return ChatResult {
            reply: ans,
            reasoning: String::new(),
            model_id: "local-seere".to_string(),
            model_name: "Local SEERE v12 (Offline Rust GBNF)".to_string(),
            vendor: "Local-Exynos".to_string(),
            latency_ms: (t0.elapsed().as_secs_f64() * 1000.0).round(),
            tokens_used: res["tokens"].as_u64().unwrap_or(0),
            tools_used,
            fallback_used: false,
            quota: None,
            provider_trace: vec!["local-seere => OK (offline)".to_string()],
        };
    }

    // Hybrid path: run SEERE local shell/root tool discovery if shell/root intent + synthesize with MiniMax M2.7
    let actual_cloud_id = if target_id == "hybrid-seere" {
        let low = prompt.to_lowercase();
        if low.contains("pid")
            || low.contains("root")
            || low.contains("ls ")
            || low.contains("archivo")
            || low.contains("kernel")
            || low.contains("proceso")
            || low.contains("memoria")
        {
            let arsenal = seere_v11::Arsenal::new(false, true, true);
            let sys_snap = arsenal.exec_root("uname -r; id; free -m | head -n 2");
            tools_used.push("RootSnap".to_string());
            let _ = sys_snap;
        }
        "minimax-m3.1-flash"
    } else {
        target_id
    };

    let sys_prompt = system_override.unwrap_or_else(|| default_re_system_prompt());
    // v16: `max_tokens` acotado.  Un `max_tokens` absurdo hacía que el
    // proveedor respondiera 400 y el daemon lo tomaba como un fallo del
    // PROVEEDOR ⇒ caía al siguiente sin decir cuál era el problema.
    let capped_max_tokens = if max_tokens == 0 {
        DEFAULT_MAX_TOKENS
    } else {
        max_tokens.min(MAX_TOKENS_CAP)
    };

    // Build ordered candidate cascade: requested model first -> other enabled cloud models -> local SEERE v12
    let mut candidates: Vec<ProviderEntry> = Vec::new();
    if let Some(p) = vault.providers.iter().find(|p| p.id == actual_cloud_id) {
        candidates.push(p.clone());
    }
    for p in &vault.providers {
        if p.id != actual_cloud_id && p.enabled && p.auth_type != "local" && p.auth_type != "hybrid" {
            candidates.push(p.clone());
        }
    }

    let mut errors: Vec<String> = Vec::new();
    let mut trace: Vec<String> = Vec::new();
    for (idx, p) in candidates.iter().enumerate() {
        // v16 (DEFECTO-03): un proveedor RETIRADO no puede usarse "de
        // rebote" en la cascada.  El aviso de "modelo retirado" sólo
        // cubría el modelo PEDIDO: el resto de la cascada lo gastaba en
        // silencio (y con una key que el usuario ya no puede usar).
        if let Some((_, rmodel, rnote)) = provider_catalog::is_retired(&p.id)
            .or_else(|| provider_catalog::is_retired(&p.model))
        {
            trace.push(format!("{} => SALTADO (modelo retirado: {})", p.id, rmodel));
            errors.push(format!("{}: modelo retirado ({})", p.id, rnote));
            continue;
        }

        // v16 (DEFECTO-04): un proveedor en PAUSA por cuota no se llama:
        // la cascada salta al siguiente.  Antes se gastaba la llamada
        // (y el 429 no se registraba nunca, así que nadie se pausaba).
        if (deps.quota_paused)(&p.id) {
            trace.push(format!("{} => SALTADO (cuota en pausa)", p.id));
            errors.push(format!("{}: cuota en pausa", p.id));
            continue;
        }

        let permitido = match deps.budget_permite {
            Some(f) => f(p, capped_max_tokens),
            None => daily_budget_permite(p, capped_max_tokens),
        };
        if !permitido {
            trace.push(format!("{} => SALTADO (presupuesto)", p.id));
            errors.push(format!("{}: fuera de presupuesto", p.id));
            continue;
        }

        let key = match p.auth_type.as_str() {
            "gh_cli" => match resolve_gh_token() {
                Some(t) => t,
                None => {
                    trace.push(format!("{} => SALTADO (sin token de gh)", p.id));
                    continue;
                }
            },
            "none" | "local" => String::new(),
            _ => {
                if p.api_key.trim().is_empty() {
                    trace.push(format!("{} => SALTADO (sin credencial)", p.id));
                    continue;
                }
                p.api_key.clone()
            }
        };

        let payload = json!({
            "model": p.model,
            "messages": [
                {"role": "system", "content": sys_prompt},
                {"role": "user", "content": enriched_prompt}
            ],
            "max_tokens": capped_max_tokens,
            "temperature": 0.3
        });
        let req = LlmRequest {
            endpoint: p.endpoint.clone(),
            model: p.model.clone(),
            api_key: key.clone(),
            auth_type: p.auth_type.clone(),
            body: payload,
        };

        // v16 (DEFECTO-05): reintentos con backoff SÓLO para lo que se
        // puede reintentar (429, 5xx, transporte, timeout).  Un 401/404
        // NO se reintenta: no mejora y quema el rate limit del proveedor.
        let mut attempt: u32 = 0;
        let mut err: Option<ProviderError> = None;
        loop {
            match deps.transport.post(&req, deps.timeout) {
                Ok(resp) => {
                    if resp.status >= 400 {
                        let e = parse_provider_error(&p.id, resp.status, &resp.body).redacted(&key);
                        if e.retryable() && attempt < deps.retries {
                            attempt += 1;
                            dormir_backoff(deps.backoff_ms, attempt);
                            continue;
                        }
                        err = Some(e);
                        break;
                    }
                    match parse_provider_reply(&p.id, &resp.body) {
                        Ok(parsed) => {
                            // v16 (DEFECTO-08): `tools_used` refleja lo
                            // REALMENTE usado: las tool calls nativas que
                            // pidió el proveedor (antes sólo salían las
                            // herramientas locales de contexto RE).
                            for tc in &parsed.tool_calls {
                                tools_used.push(format!("provider_tool:{}", tc.name));
                            }
                            let tokens_in = parsed.prompt_tokens;
                            let tokens_out = parsed.completion_tokens;
                            let total_tokens = parsed
                                .tokens_used
                                .max(tokens_in.saturating_add(tokens_out));
                            crate::quota_guard::registrar_uso(&p.id, total_tokens, None);
                            let _cost_usd = record_daily_spend(
                                &p.id,
                                &p.model,
                                tokens_in,
                                tokens_out,
                                total_tokens,
                            );
                            trace.push(format!("{} => OK", p.id));
                            return ChatResult {
                                reply: parsed.content,
                                reasoning: parsed.reasoning,
                                model_id: p.id.clone(),
                                model_name: p.name.clone(),
                                vendor: p.vendor.clone(),
                                latency_ms: (t0.elapsed().as_secs_f64() * 1000.0).round(),
                                tokens_used: parsed.tokens_used,
                                tools_used,
                                fallback_used: idx > 0,
                                quota: Some(crate::quota_guard::snapshot_compacto_de(Some(&p.id))),
                                provider_trace: trace,
                            };
                        }
                        Err(e) => {
                            err = Some(e.redacted(&key));
                            break;
                        }
                    }
                }
                Err(e) => {
                    let e = e.redacted(&key);
                    if e.retryable() && attempt < deps.retries {
                        attempt += 1;
                        dormir_backoff(deps.backoff_ms, attempt);
                        continue;
                    }
                    err = Some(e);
                    break;
                }
            }
        }

        if let Some(e) = &err {
            // Cuota/rate-limit: se REGISTRA (con el status REAL) para que
            // el agente lo vea y para poder PAUSAR en vez de reintentar en
            // bucle.  v16: antes el status se perdía y esto nunca pasaba.
            if let Some(st) = e.status() {
                if st == 429 {
                    crate::quota_guard::registrar_error_cuota(
                        &p.id,
                        st,
                        &format!("{} rate limit", e.detail()),
                    );
                } else if st == 402 {
                    crate::quota_guard::registrar_error_cuota(
                        &p.id,
                        st,
                        &format!("{} insufficient credit", e.detail()),
                    );
                }
            }
            // v16 (DEFECTO-02): el error ya viene REDACTADO (la clave del
            // proveedor no puede acabar en el `reply` que ve el cliente).
            errors.push(format!("{}: {}", p.id, truncar(&e.to_string(), 200)));
            trace.push(format!("{} => {}", p.id, truncar(&e.to_string(), 120)));
        }
    }

    // Final guaranteed offline fallback to Local SEERE v12 Rust GBNF
    let (_, pol) = crate::power_policy::current_policy();
    let res = seere_v11::run_seere_v11(&enriched_prompt, pol.max_turns.max(1), true, false, false, true, true);
    let ans = res["answer"].as_str().unwrap_or("Sin respuesta").to_string();
    let note = if errors.is_empty() {
        ans
    } else {
        format!("{}\n\n[Fallback offline activo — errores cloud: {}]", ans, errors.join(" | "))
    };
    trace.push("local-seere => OK (offline, último recurso)".to_string());
    ChatResult {
        reply: note,
        reasoning: String::new(),
        model_id: "local-seere".to_string(),
        model_name: "Local SEERE v12 (Offline Fallback)".to_string(),
        vendor: "Local-Exynos".to_string(),
        latency_ms: (t0.elapsed().as_secs_f64() * 1000.0).round(),
        tokens_used: res["tokens"].as_u64().unwrap_or(0),
        tools_used,
        fallback_used: true,
        quota: Some(crate::quota_guard::snapshot_compacto()),
        provider_trace: trace,
    }
}

/// Espera el backoff entre reintentos (0 en tests ⇒ sin esperas).
fn dormir_backoff(base_ms: u64, attempt: u32) {
    if base_ms == 0 {
        return;
    }
    let mult = 1u64 << attempt.min(6);
    std::thread::sleep(Duration::from_millis(base_ms.saturating_mul(mult)));
}

pub fn test_provider_connection(id: &str) -> Value {
    let t0 = Instant::now();
    let res = execute_chat("Responde exactamente: OK", Some(id), None, Some("Reply OK only."), 24);
    json!({
        "status": if res.model_id == id || (!res.fallback_used && !res.reply.is_empty()) { "OK" } else { "FALLBACK" },
        "requested": id,
        "resolved_model": res.model_id,
        "reply": res.reply,
        "latency_ms": (t0.elapsed().as_secs_f64() * 1000.0).round(),
        "fallback_used": res.fallback_used
    })
}
