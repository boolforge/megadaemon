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
    /// Tokens de **entrada** (los reporta el proveedor; 0 si no los dice).
    #[serde(default)]
    pub prompt_tokens: u64,
    /// Tokens de **salida** (los reporta el proveedor; 0 si no los dice).
    #[serde(default)]
    pub completion_tokens: u64,
    pub tools_used: Vec<String>,
    pub fallback_used: bool,
}

impl ChatResult {
    /// JSON **sin** el razonamiento interno.
    ///
    /// v16 (AUDIT-LLM-16-E): el razonamiento es cadena-de-pensamiento del
    /// modelo.  Antes `execute_chat` lo devolvía tal cual y lo imprimían
    /// el CLI (`serde_json::to_string_pretty(&res)`) y la tool MCP
    /// `minimax_chat` (`<reasoning>...</reasoning>`): el cliente recibía el
    /// CoT crudo.  Ahora el razonamiento sólo sale si se pide explícitamente.
    pub fn public_json(&self, include_reasoning: bool) -> Value {
        let mut v = serde_json::to_value(self).unwrap_or(Value::Null);
        if !include_reasoning {
            if let Some(o) = v.as_object_mut() {
                o.remove("reasoning");
            }
        }
        v
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// v16 — Errores tipados, transporte inyectable y parseo robusto
// ─────────────────────────────────────────────────────────────────────────────

/// Tope duro de `max_tokens` que se envía a un proveedor.
///
/// v16: un `max_tokens` absurdo (10_000_000) hace que el proveedor
/// responda 400 y el daemon lo trata como un fallo del PROVEEDOR => cae al
/// siguiente sin avisar de que el problema era el `max_tokens`.  El límite
/// se aplica ANTES de salir a la red.
pub const MAX_TOKENS_CAP: u32 = 32_768;

/// `max_tokens` por defecto cuando el llamante no dice nada.
pub const DEFAULT_MAX_TOKENS: u32 = 4_096;

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
    /// Status HTTP >= 400.
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
            | ProviderError::Budget { provider, .. } => provider,
            ProviderError::Retired { provider, .. } => provider,
        }
    }

    /// Status HTTP, si lo hubo.
    pub fn status(&self) -> Option<u16> {
        match self {
            ProviderError::Http { status, .. } => Some(*status),
            _ => None,
        }
    }

    /// Etiqueta estable (para logs y tests).
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
            ProviderError::Http { status, .. } => *status == 429 || (500..=599).contains(status),
            _ => false,
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
        let fix = |d: &mut String| *d = redact_secret(d, s);
        match &mut e {
            ProviderError::Transport { detail, .. }
            | ProviderError::Timeout { detail, .. }
            | ProviderError::Parse { detail, .. }
            | ProviderError::MissingKey { detail, .. }
            | ProviderError::Budget { detail, .. } => fix(detail),
            ProviderError::Http { detail, .. } => fix(detail),
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
/// Também tapa los prefijos típicos (`Bearer `, `sk-`, `key=`) para que un
/// secreto pegado con prefijo tampoco escape.
pub fn redact_secret(text: &str, secret: &str) -> String {
    let s = secret.trim();
    if s.is_empty() {
        return text.to_string();
    }
    let mut out = text.replace(s, "***");
    // El proveedor puede devolver la key con el prefijo pegado ("Bearer sk-x").
    for pre in ["Bearer ", "bearer ", "sk-", "sk_"] {
        let with = format!("{}{}", pre, s);
        if with != s {
            out = out.replace(&with, &format!("{}***", pre));
        }
    }
    out
}

/// Redacta varios secretos a la vez.
pub fn redact_all(text: &str, secrets: &[&str]) -> String {
    let mut out = text.to_string();
    for s in secrets {
        out = redact_secret(&out, s);
    }
    out
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
    /// Razonamiento interno (viene en `reasoning_content` o en `<think>`).
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
            // v16: por defecto ureq convierte 4xx/5xx en `Err`, y entonces el
            // código NUNCA llegaba al bloque que distingue 401 (key mala) de
            // 402 (sin saldo) de 404 (modelo muerto) de 429 (rate limit):
            // el diagnóstico-rich was dead code.
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

        let body = serde_json::to_vec(&req.body).map_err(|e| ProviderError::Transport {
            provider: req.model.clone(),
            detail: format!("cuerpo no serializable: {}", e),
        })?;

        let resp = match r.send(body.as_slice()) {
            Ok(resp) => resp,
            Err(e) => return Err(map_ureq_error(&req.model, e)),
        };
        let status = resp.status().as_u16();
        let text = resp
            .into_body()
            .read_to_string()
            .map_err(|e| ProviderError::Transport {
                provider: req.model.clone(),
                detail: format!("HTTP body read error: {}", e),
            })?;
        Ok(LlmResponse { status, body: text })
    }
}

/// Traduce un error de `ureq` al error tipado.
fn map_ureq_error(provider: &str, e: ureq::Error) -> ProviderError {
    match e {
        ureq::Error::Timeout(_) => ProviderError::Timeout {
            provider: provider.to_string(),
            detail: "el proveedor no respondió a tiempo".to_string(),
        },
        ureq::Error::Io(io) => {
            let kind = io.kind();
            if kind == std::io::ErrorKind::TimedOut {
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
        // v16: `http_status_as_error(false)` evita este caso, pero si algún
        // transporte lo devuelve, el status es información útil.
        ureq::Error::StatusCode(code) => ProviderError::Http {
            provider: provider.to_string(),
            status: code,
            detail: String::new(),
        },
        other => ProviderError::Transport {
            provider: provider.to_string(),
            detail: other.to_string(),
        },
    }
}

/// Motor local (SEERE) inyectable.
///
/// v16: `execute_chat` llamaba a `seere_v11::run_seere_v11` in situ, que
/// intenta hablar con `127.0.0.1:8080`.  Para que la suite sea offline y
/// determinista, el motor local también se inyecta.
pub trait LocalFallback: Send + Sync {
    /// Ejecuta el motor local y devuelve `{"answer","tools","tokens"}`.
    fn run(&self, prompt: &str, max_turns: usize) -> Value;

    /// Nombre legible.
    fn name(&self) -> &'static str {
        "local"
    }
}

/// Motor local real: SEERE v13 (GBNF, offline).
#[derive(Debug, Clone, Copy, Default)]
pub struct SeereLocal;

impl LocalFallback for SeereLocal {
    fn name(&self) -> &'static str {
        "seere"
    }

    fn run(&self, prompt: &str, max_turns: usize) -> Value {
        let (_, pol) = crate::power_policy::current_policy();
        seere_v11::run_seere_v11(
            prompt,
            max_turns.max(1),
            true,
            false,
            false,
            true,
            true,
        )
    }
}

/// Política de la cascada de proveedores.
#[derive(Debug, Clone)]
pub struct ChatPolicy {
    /// ¿Se puede caer a un proveedor **de pago** durante el fallback?
    ///
    /// v16 (AUDIT-LLM-16-C): antes, sí, y sin avisar.  Pedir un proveedor
    /// free que fallaba acababa gastando dinero en MiniMax/DeepSeek/Anthropic
    /// sin que nadie lo pidiera ni se dijera.  Por defecto **NO**: el
    /// fallback sólo usa proveedores free/keyless/local.  El proveedor
    /// **pedido explícitamente** siempre se puede usar (es lo que el usuario
    /// pidió), sea del tier que sea.
    pub allow_paid_fallback: bool,
    /// Tope de `max_tokens` (0 = `MAX_TOKENS_CAP`).
    pub max_tokens_cap: u32,
    /// Reintentos para 429/5xx y fallos de transporte.
    pub max_retries: u32,
    /// Espera base del backoff en ms (0 = sin espera; lo usan los tests).
    pub backoff_base_ms: u64,
    /// Timeout por defecto de un proveedor (segundos).
    pub timeout_secs: u32,
    /// Presupuesto propio.
    ///
    /// `None` = el presupuesto **global** del proceso
    /// (`service_bus::global_budget`), que es el que se comparte entre
    /// llamadas.  La suite inyecta uno propio para ser determinista.
    pub budget: Option<std::sync::Arc<std::sync::Mutex<crate::service_bus::Budget>>>,
}

impl Default for ChatPolicy {
    fn default() -> Self {
        ChatPolicy {
            allow_paid_fallback: false,
            max_tokens_cap: MAX_TOKENS_CAP,
            max_retries: 2,
            backoff_base_ms: 250,
            timeout_secs: 45,
            budget: None,
        }
    }
}

impl ChatPolicy {
    /// Política de pruebas: sin esperas de backoff y con presupuesto propio.
    pub fn for_tests(budget: crate::service_bus::Budget) -> ChatPolicy {
        ChatPolicy {
            backoff_base_ms: 0,
            max_retries: 2,
            budget: Some(std::sync::Arc::new(std::sync::Mutex::new(budget))),
            ..ChatPolicy::default()
        }
    }

    /// Reserva coste en el presupuesto de la política.
    pub(crate) fn reserve_budget(
        &self,
        cost: f64,
    ) -> Option<crate::service_bus::BudgetReservation> {
        match &self.budget {
            Some(m) => {
                let mut g = match m.lock() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                g.try_reserve(cost)
            }
            None => crate::service_bus::with_global_budget(|b| b.try_reserve(cost)),
        }
    }

    /// Devuelve el coste reservado al presupuesto de la política.
    pub(crate) fn release_budget(&self, r: crate::service_bus::BudgetReservation) {
        match &self.budget {
            Some(m) => {
                let mut g = match m.lock() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                r.release(&mut g);
            }
            None => crate::service_bus::with_global_budget(|b| r.release(b)),
        }
    }

    fn cap(&self) -> u32 {
        if self.max_tokens_cap == 0 {
            MAX_TOKENS_CAP
        } else {
            self.max_tokens_cap.min(MAX_TOKENS_CAP)
        }
    }

    /// Normaliza el `max_tokens` pedido: 0 => por defecto, y nunca por
    /// encima del tope.
    pub fn clamp_tokens(&self, max_tokens: u32) -> u32 {
        if max_tokens == 0 {
            return DEFAULT_MAX_TOKENS.min(self.cap());
        }
        max_tokens.min(self.cap())
    }
}

/// Petición de chat normalizada.
#[derive(Debug, Clone, Default)]
pub struct ChatRequest {
    pub prompt: String,
    pub requested_model: Option<String>,
    pub attachment_path: Option<String>,
    pub system: Option<String>,
    pub max_tokens: u32,
    /// Mensajes ya construidos en formato OpenAI.
    ///
    /// Si no está vacío, se envían TAL CUAL al proveedor (conversación
    /// multi-turno con roles) y se ignoran `prompt` y `system`.  Antes el
    /// gateway concatenaba los mensajes de usuario y assistant en un único
    /// prompt: el modelo recibía su propia respuesta anterior como si fuera
    /// una frase del usuario.
    pub messages: Vec<Value>,
}

impl ChatRequest {
    /// Petición mínima con el prompt.
    pub fn new(prompt: &str) -> ChatRequest {
        ChatRequest {
            prompt: prompt.to_string(),
            ..Default::default()
        }
    }

    /// Fija el modelo pedido.
    pub fn with_model(mut self, model: &str) -> ChatRequest {
        self.requested_model = Some(model.to_string());
        self
    }

    /// Fija el `max_tokens`.
    pub fn with_max_tokens(mut self, n: u32) -> ChatRequest {
        self.max_tokens = n;
        self
    }
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

/// Guarda la boveda de proveedores de forma **atómica**.
///
/// v16 (AUDIT-LLM-16-D): antes se hacía `fs::write(path, text)`, que abre el
/// fichero con O_TRUNC y escribe.  Si el proceso muere (o el móvil se
/// apaga) a mitad de la escritura, el `providers.json` queda **truncado**:
/// `load_vault` lo marca como corrupto, hace una copia `.corrupt-<ts>` y
/// **vuelve al catálogo por defecto** ⇒ el usuario se queda **sin ninguna de
/// sus API keys**.
///
/// Ahora: se escribe a un temporal y se `rename()` encima (atómico en el
/// mismo sistema de ficheros) y se guarda una copia `.bak` de la versión
/// anterior.  Un corte de luz deja el `.bak` intacto.
pub fn save_vault(vault: &ProviderVault) -> Result<(), String> {
    let path = providers_path();
    if let Some(parent) = Path::new(&path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    let text = serde_json::to_string_pretty(vault).map_err(|e| e.to_string())?;
    let tmp = format!("{}.tmp-{}", path, ts_now());
    fs::write(&tmp, text.as_bytes()).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("no se pudo escribir el temporal {}: {}", tmp, e)
    })?;
    // Copia de seguridad de la versión anterior (si la había).
    if let Ok(prev) = fs::read(&path) {
        let _ = fs::write(format!("{}.bak", path), prev);
    }
    match fs::rename(&tmp, &path) {
        Ok(_) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(format!("no se pudo renombrar {} -> {}: {}", tmp, path, e))
        }
    }
}

/// Valida un endpoint de proveedor.
///
/// v16 (AUDIT-LLM-16-D): `update_provider` aceptaba **cualquier** cadena
/// como endpoint.  Con la API key ya guardada, re-apuntar el endpoint a
/// `http://169.254.169.254/latest/meta-data/` o a `http://127.0.0.1:9090/…`
/// hacía que el daemon **mandase la key del usuario a ese host** (SSRF +
/// exfiltración de credenciales).  Ahora se exige un `http(s)://` con host
/// y se rechazan los hosts internos/metadatos.
pub fn validate_endpoint(endpoint: &str) -> Result<String, String> {
    let e = endpoint.trim();
    if e.is_empty() {
        return Err("endpoint vacio".to_string());
    }
    if e.len() > 2048 {
        return Err("endpoint demasiado largo".to_string());
    }
    let lower = e.to_lowercase();
    let rest = if let Some(r) = lower.strip_prefix("https://") {
        r
    } else if let Some(r) = lower.strip_prefix("http://") {
        r
    } else {
        return Err(format!(
            "endpoint invalido: '{}' (debe empezar por http:// o https://)",
            e
        ));
    };
    let hostport = rest.split('/').next().unwrap_or("");
    let host = hostport
        .split('@')
        .next_back()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .trim_matches(|c| c == '[' || c == ']');
    if host.is_empty() {
        return Err("endpoint invalido: sin host".to_string());
    }
    // Hosts que no deben recibir la key del usuario.
    let blocked = [
        "169.254.169.254",
        "metadata.google.internal",
        "metadata.goog",
        "169.254.170.2",
        "100.100.100.200",
    ];
    for b in blocked {
        if host.eq_ignore_ascii_case(b) {
            return Err(format!(
                "endpoint rechazado: '{}' es un host de metadatos (SSRF)",
                host
            ));
        }
    }
    if host.eq_ignore_ascii_case("localhost") || host == "::1" {
        // Loopback: sólo se permite para motores LOCALES sin credencial.
        return Err(
            "endpoint rechazado: 'localhost' no puede recibir credenciales de proveedor".to_string(),
        );
    }
    Ok(e.to_string())
}

/// Valida un id de proveedor (usado al crear uno nuevo).
pub fn validate_provider_id(id: &str) -> Result<String, String> {
    let i = id.trim();
    if i.is_empty() {
        return Err("id de proveedor vacio".to_string());
    }
    if i.len() > 64 {
        return Err("id de proveedor demasiado largo".to_string());
    }
    if !i
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(format!(
            "id de proveedor invalido: '{}' (solo [A-Za-z0-9._-])",
            i
        ));
    }
    Ok(i.to_string())
}

/// Valida un modelo de proveedor.
pub fn validate_model(model: &str) -> Result<String, String> {
    let m = model.trim();
    if m.is_empty() {
        return Err("modelo vacio".to_string());
    }
    if m.len() > 200 {
        return Err("modelo demasiado largo".to_string());
    }
    if m.chars().any(|c| c.is_control()) {
        return Err("modelo invalido: contiene caracteres de control".to_string());
    }
    Ok(m.to_string())
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
    let id = id.trim();
    if id.is_empty() {
        return Err("id de proveedor vacio".to_string());
    }
    // v16 (AUDIT-LLM-16-D): la comprobación de "retirado" se hacía DESPUÉS
    // de mutar la boveda en memoria.  Ahora se hace ANTES de tocar nada:
    // un id retirado no se puede ni crear ni re-enable.
    if let Some((rid, rmodel, rnote)) = provider_catalog::is_retired(id) {
        return Err(format!(
            "MODELO RETIRADO: {} ({}) -> {}",
            rmodel, rid, rnote
        ));
    }
    // v16 (AUDIT-LLM-16-D): valida el endpoint ANTES de guardarlo (SSRF).
    let nuevo_endpoint = match endpoint {
        Some(ep) if !ep.trim().is_empty() => Some(validate_endpoint(ep)?),
        _ => None,
    };
    // v16 (AUDIT-LLM-16-D): valida el modelo y rechaza los RETIRADOS por
    // nombre.  Antes sólo se miraba el *id*: cambiar el modelo de un
    // proveedor vivo a uno retirado (minimax-m2.7) se aceptaba y el daemon
    // se comía un 404 en cada llamada.
    let nuevo_modelo = match model {
        Some(m) if !m.trim().is_empty() => {
            let m = validate_model(m)?;
            if let Some((_, rmodel, rnote)) = provider_catalog::is_retired(&m) {
                return Err(format!(
                    "MODELO RETIRADO: {} -> {}",
                    rmodel, rnote
                ));
            }
            Some(m)
        }
        _ => None,
    };

    let mut vault = load_vault();
    let mut found = false;
    for p in &mut vault.providers {
        if p.id == id {
            if let Some(k) = api_key {
                let k = k.trim();
                if !k.is_empty() && k.chars().any(|c| c.is_control()) {
                    return Err("API key invalida: contiene caracteres de control".to_string());
                }
                p.api_key = k.to_string();
                if !p.api_key.is_empty() {
                    p.enabled = true;
                    // v16 (AUDIT-LLM-16-D): el tier **NO** se sobreescribe con
                    // "configured".  Eso destruía la clasificación de coste
                    // del proveedor: `tier_free()` dejaba de devolver true
                    // para un Groq/Google gratis, y el bus lo pasaba a la
                    // lista de "de pago" (presupuesto y política "Cheapest"
                    // equivocados).  El tier original se conserva; que haya
                    // key ya se ve en `ready`/`has_key`.
                }
            }
            if let Some(m) = nuevo_modelo.as_ref() {
                p.model = m.clone();
            }
            if let Some(ep) = nuevo_endpoint.as_ref() {
                // v13.2 (HW-03): cambiar el endpoint de un proveedor que ya
                // tiene una key guardada enviaba esa key al host nuevo
                // (SSRF + robo de credenciales) sin pedir la key.
                // Ahora hay que re-introducir la key al cambiar de host.
                if p.endpoint.trim() != ep && !p.api_key.trim().is_empty() {
                    // v13.2 (HW-03): la API key queda ATADA al endpoint
                    // completo (esquema+host+ruta), no sólo al host.  El
                    // endpoint HTTP del cockpit no tiene autenticación, así
                    // que sin esta regla cualquiera podía re-apuntar un
                    // proveedor con key guardada yeysar esa key (SSRF +
                    // robo de credenciales).  Al cambiar el endpoint, la
                    // key se vacía y hay que re-introducirla.
                    p.api_key = String::new();
                    p.auth_type = "none".to_string();
                }
                p.endpoint = ep.clone();
            }
            if let Some(en) = enabled {
                p.enabled = en;
            }
            found = true;
            break;
        }
    }
    if !found {
        // Allow creating custom OpenAI-compatible provider dynamically
        let id = validate_provider_id(id)?;
        let ep = match nuevo_endpoint.as_ref() {
            Some(e) => e.clone(),
            None => "https://openrouter.ai/api/v1/chat/completions".to_string(),
        };
        let m = match nuevo_modelo.as_ref() {
            Some(m) => m.clone(),
            None => "openai/gpt-4o-mini".to_string(),
        };
        vault.providers.push(ProviderEntry {
            id: id.clone(),
            name: format!("Custom ({})", id),
            vendor: "Custom".to_string(),
            endpoint: ep,
            model: m,
            api_key: api_key.unwrap_or("").trim().to_string(),
            auth_type: "bearer".to_string(),
            tier: "custom".to_string(),
            free_signup_url: String::new(),
            enabled: enabled.unwrap_or(true),
            supports_reasoning: true,
        });
    }
    if set_default {
        // v16: el modelo por defecto tampoco puede ser uno RETIRADO ni
        // uno que no exista en la boveda (si no, el daemon arrancaba ya con
        // un default muerto).
        if provider_catalog::is_retired(&id).is_some() {
            return Err(format!("el modelo por defecto '{}' esta RETIRADO", id));
        }
        if !vault.providers.iter().any(|p| p.id == id) {
            return Err(format!("el modelo por defecto '{}' no existe", id));
        }
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

/// Separa el razonamiento `<think>…</think>` del texto final.
///
/// v16 (AUDIT-LLM-16-F): el código viejo buscaba el **primer** `</think>` en
/// todo el texto.  Si el modelo emitía un `</think>` huérfano ANTES del
/// `<think>` (típico de los razonadores chinos: cierran un bloque vacío al
/// principio), la comparación `end > start + 7` fallaba y el texto entero
/// (con las etiquetas y el razonamiento) se devolvía como respuesta.
///
/// Ahora se busca el `</think>` **después** del `<think>`, y si no hay
/// cierre se descarta el `<think>` parcial (respuesta truncada) en vez de
/// devolverlo crudo.
pub fn split_think_tags(raw: &str) -> (String, String) {
    if let Some(start) = raw.find("<think>") {
        let after = &raw[start + 7..];
        if let Some(rel_end) = after.find("</think>") {
            let end = start + 7 + rel_end;
            let thought = after[..rel_end].trim().to_string();
            let before = raw[..start].trim();
            let after_text = after[rel_end + 8..].trim();
            let clean = if before.is_empty() {
                after_text.to_string()
            } else if after_text.is_empty() {
                before.to_string()
            } else {
                format!("{}\n{}", before, after_text)
            };
            return (clean, thought);
        }
        // `<think>` sin cerrar: respuesta truncada.  Se devuelve el texto
        // visible (nada) y el razonamiento parcial aparte.
        let before = raw[..start].trim().to_string();
        let partial = after.trim().to_string();
        return (before, partial);
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
    let policy = ChatPolicy::default();
    let req = ChatRequest::new(user_prompt)
        .with_max_tokens(policy.clamp_tokens(max_tokens));
    let parsed = call_provider_typed(
        &UreqTransport::default(),
        &policy,
        endpoint,
        model,
        api_key,
        auth_type,
        Some(system_prompt),
        &req,
    )
    .map_err(|e| e.to_string())?;
    Ok((parsed.content, parsed.reasoning, parsed.tokens_used))
}

/// Truncado **seguro** de un texto UTF-8 por BYTES.
///
/// v16 (AUDIT-LLM-16-F): el código antigo hacía
/// `&trimmed[..trimmed.len().min(200)]`, que corta en mitad de un carácter
/// multibyte => **panic** ("byte index N is not a char boundary").  Con un
/// cuerpo con acentos o emoji (todos los proveedores chinos los devuelven)
/// el daemon.enteraba en panic al parsear un error.
pub fn truncate_utf8(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

/// Detalle de un error HTTP tal y como lo devuelve un proveedor.
fn http_error_detail(body: &str) -> String {
    let trimmed = body.trim();
    serde_json::from_str::<Value>(trimmed)
        .ok()
        .and_then(|v| {
            v.get("error")
                .and_then(|e| {
                    e.get("message")
                        .and_then(|m| m.as_str())
                        .or_else(|| e.as_str())
                        .map(|s| s.to_string())
                })
                .or_else(|| v.get("message").and_then(|m| m.as_str()).map(|s| s.to_string()))
        })
        .unwrap_or_else(|| truncate_utf8(trimmed, 200))
}

/// Pista accionable según el status HTTP.
pub fn http_status_hint(status: u16) -> &'static str {
    match status {
        401 | 403 => " (API key invalida o sin permisos)",
        402 => " (sin saldo / plan agotado)",
        404 => " (modelo inexistente o RETIRADO - mira el catalogo)",
        408 => " (timeout del servidor)",
        409 | 413 => " (payload demasiado grande o conflicto)",
        422 => " (peticion invalida: revisa model/max_tokens)",
        429 => " (rate limit o cuota diaria agotada)",
        500..=599 => " (error del servidor: reintentable)",
        _ => "",
    }
}

/// Interpreta el cuerpo de un proveedor.
///
/// **Puro**: no toca la red, no muta nada y **nunca hace panic**.  Devuelve
/// o una respuesta interpretada o un error TIPADO y útil.
///
/// v16 (AUDIT-LLM-16-F): el parseo viejo mezclaba cuatro casos y sólo
/// miraba uno:
///   * `content` vacío + `reasoning` presente => devolvía el razonamiento
///     como si fuera la respuesta final (el usuario veía el CoT como
///     respuesta).
///   * cuerpo **truncado** (JSON a medias) => `serde_json` falla y el
///     código lo trataba como "texto plano" => la respuesta del cliente era
///     un fragmento de JSON crudo.
///   * cuerpo **no JSON** => se aceptaba como texto plano sin distinguirlo
///     de un error HTML de un proxy.
///   * `content` como ARRAY (respuestas multimodales) => se leía como ""
///     y se perdía el texto.
pub fn parse_provider_body(
    provider_id: &str,
    status: u16,
    body: &str,
) -> Result<ParsedReply, ProviderError> {
    let trimmed = body.trim();

    if status >= 400 {
        let mut detail = http_error_detail(trimmed);
        detail.push_str(http_status_hint(status));
        return Err(ProviderError::Http {
            provider: provider_id.to_string(),
            status,
            detail,
        });
    }

    if trimmed.is_empty() {
        return Err(ProviderError::Parse {
            provider: provider_id.to_string(),
            detail: "respuesta HTTP vacia".to_string(),
        });
    }

    let looks_json = trimmed.starts_with('{') || trimmed.starts_with('[');

    let v: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(e) => {
            if looks_json {
                // JSON roto: casi siempre una respuesta TRUNCADA (el
                // proveedor cortó el stream) o un proxy que devolvió HTML.
                return Err(ProviderError::Parse {
                    provider: provider_id.to_string(),
                    detail: format!(
                        "JSON invalido (truncado o no-JSON): {} | primeros bytes: {}",
                        e,
                        truncate_utf8(trimmed, 160)
                    ),
                });
            }
            // Texto plano (endpoints zero-key): es una respuesta válida.
            return Ok(ParsedReply {
                content: trimmed.to_string(),
                reasoning: String::new(),
                tokens_used: 0,
                prompt_tokens: 0,
                completion_tokens: 0,
                tool_calls: Vec::new(),
                finish_reason: "stop".to_string(),
            });
        }
    };

    // Un 200 con cuerpo de error: algunos proveedores devuelven 200 con
    // {"error": {...}}.
    if let Some(err_obj) = v.get("error") {
        let msg = err_obj["message"]
            .as_str()
            .or_else(|| err_obj.as_str())
            .unwrap_or("API error");
        return Err(ProviderError::Parse {
            provider: provider_id.to_string(),
            detail: format!("API error: {}", msg),
        });
    }

    let choices = v.get("choices").and_then(|c| c.as_array());
    let first = match choices.and_then(|c| c.first()) {
        Some(c) => c,
        None => {
            return Err(ProviderError::Parse {
                provider: provider_id.to_string(),
                detail: format!(
                    "respuesta sin 'choices' (claves: {})",
                    v.as_object()
                        .map(|o| o.keys().cloned().collect::<Vec<_>>().join(","))
                        .unwrap_or_else(|| "?".to_string())
                ),
            })
        }
    };

    let msg_node = first.get("message").cloned().unwrap_or(Value::Null);
    let content = extract_content_text(&msg_node);
    let extra_reason = msg_node["reasoning_content"]
        .as_str()
        .or_else(|| msg_node["reasoning"].as_str())
        .unwrap_or("");
    let prompt_tokens = v["usage"]["prompt_tokens"].as_u64().unwrap_or(0);
    let completion_tokens = v["usage"]["completion_tokens"].as_u64().unwrap_or(0);
    let tokens = v["usage"]["total_tokens"]
        .as_u64()
        .unwrap_or(prompt_tokens.saturating_add(completion_tokens));
    let finish_reason = first
        .get("finish_reason")
        .and_then(|f| f.as_str())
        .unwrap_or("")
        .to_string();

    let (clean, think_tag) = split_think_tags(&content);
    let reasoning = if !extra_reason.is_empty() {
        extra_reason.to_string()
    } else {
        think_tag
    };

    let tool_calls = parse_native_tool_calls(&msg_node);

    if clean.is_empty() && reasoning.is_empty() && tool_calls.is_empty() {
        return Err(ProviderError::Parse {
            provider: provider_id.to_string(),
            detail: format!(
                "respuesta vacia (finish_reason={}) | primeros bytes: {}",
                if finish_reason.is_empty() {
                    "-"
                } else {
                    &finish_reason
                },
                truncate_utf8(trimmed, 160)
            ),
        });
    }

    Ok(ParsedReply {
        content: clean,
        reasoning,
        tokens_used: tokens,
        prompt_tokens,
        completion_tokens,
        tool_calls,
        finish_reason,
    })
}

/// Extrae el texto de `message.content` (string o array de partes).
pub fn extract_content_text(msg_node: &Value) -> String {
    match msg_node.get("content") {
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

/// Parsea las `tool_calls` nativas de OpenAI.
pub fn parse_native_tool_calls(msg_node: &Value) -> Vec<ParsedToolCall> {
    let mut out = Vec::new();
    let Some(arr) = msg_node.get("tool_calls").and_then(|c| c.as_array()) else {
        return out;
    };
    for (idx, item) in arr.iter().enumerate() {
        let id = item
            .get("id")
            .and_then(|i| i.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from)
            .unwrap_or_else(|| format!("call_native_{}", idx + 1));
        let func = item.get("function").unwrap_or(item);
        let Some(name) = func.get("name").and_then(|n| n.as_str()) else {
            continue;
        };
        let arguments = match func.get("arguments") {
            Some(Value::String(s)) => serde_json::from_str::<Value>(s).unwrap_or_else(|_| json!({})),
            Some(Value::Object(_)) => func.get("arguments").cloned().unwrap_or_else(|| json!({})),
            _ => json!({}),
        };
        out.push(ParsedToolCall {
            id,
            name: name.to_string(),
            arguments,
        });
    }
    out
}

/// Llama a un proveedor con **reintentos**, timeout por proveedor y
/// errores tipados.
///
/// v16: el reintento sólo se hace para lo que tiene sentido (429, 5xx,
/// transporte y timeout).  Un 401/404 **no** se reintenta: la key sigue
/// siendo mala y el modelo sigue sin existir; reintentar sólo gasta
/// tiempo y puede comerse el rate limit del proveedor.
pub fn call_provider_typed(
    transport: &dyn LlmTransport,
    policy: &ChatPolicy,
    endpoint: &str,
    model: &str,
    api_key: &str,
    auth_type: &str,
    system_prompt: Option<&str>,
    req: &ChatRequest,
) -> Result<ParsedReply, ProviderError> {
    let mut messages: Vec<Value> = Vec::new();
    if !req.messages.is_empty() {
        // Conversación multi-turno: se respeta tal cual.
        messages = req.messages.clone();
    } else {
        if let Some(s) = system_prompt {
            if !s.trim().is_empty() {
                messages.push(json!({"role": "system", "content": s}));
            }
        }
        messages.push(json!({"role": "user", "content": req.prompt}));
    }

    let body = json!({
        "model": model,
        "messages": messages,
        "max_tokens": policy.clamp_tokens(req.max_tokens),
        "temperature": 0.3,
    });

    let llm_req = LlmRequest {
        endpoint: endpoint.to_string(),
        model: model.to_string(),
        api_key: api_key.to_string(),
        auth_type: auth_type.to_string(),
        body,
    };

    execute_openai_call(transport, policy, &llm_req)
}

/// Ejecuta una petición ya construida con **reintentos**, timeout y
/// errores tipados.
///
/// v16: el reintento sólo se hace para lo que tiene sentido (429, 5xx,
/// transporte y timeout).  Un 401/404 **no** se reintenta: la key sigue
/// siendo mala y el modelo sigue sin existir; reintentar sólo gasta
/// tiempo y puede comerse el rate limit del proveedor.
pub fn execute_openai_call(
    transport: &dyn LlmTransport,
    policy: &ChatPolicy,
    llm_req: &LlmRequest,
) -> Result<ParsedReply, ProviderError> {
    let timeout = Duration::from_secs(policy.timeout_secs.max(1) as u64);
    let mut attempt = 0u32;
    loop {
        let err = match transport.post(llm_req, timeout) {
            Ok(resp) => match parse_provider_body(&llm_req.model, resp.status, &resp.body) {
                Ok(p) => return Ok(p),
                Err(e) => e,
            },
            Err(e) => e,
        };
        let err = err.redacted(&llm_req.api_key);
        let can_retry = err.retryable() && attempt < policy.max_retries;
        if !can_retry {
            return Err(err);
        }
        // Backoff exponencial: 250 ms, 500 ms, 1 s...
        let delay = policy.backoff_base_ms.saturating_mul(1u64 << attempt);
        if delay > 0 {
            std::thread::sleep(Duration::from_millis(delay));
        }
        attempt += 1;
    }
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

/// Coste estimado (USD) de una llamada a este proveedor.
///
/// Usa la misma tabla de precios que el bus de servicios
/// (`service_bus::cost_for_model`), para que el presupuesto del bus y el
/// de la cascada no se contradigan.
pub fn estimate_call_cost(p: &ProviderEntry, max_tokens: u32) -> f64 {
    let est = crate::service_bus::cost_for_model(&p.model, &p.tier);
    est.estimate(0, max_tokens.max(1))
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
    let vault = load_vault();
    let req = ChatRequest {
        prompt: prompt.to_string(),
        requested_model: requested_model.map(|s| s.to_string()),
        attachment_path: attachment_path.map(|s| s.to_string()),
        system: system_override.map(|s| s.to_string()),
        max_tokens,
        messages: Vec::new(),
    };
    execute_chat_in(
        &vault,
        &UreqTransport::default(),
        &SeereLocal,
        &ChatPolicy::default(),
        &req,
    )
}

/// Orquestador de chat **inyectable** (vault + transporte + motor local +
/// política).
///
/// v16: `execute_chat` leía el vault del disco y hacía el POST con `ureq`
/// in situ: era IMPOSIBLE testear la cascada, el fallback, el presupuesto y
/// el parseo sin red.  Esta función es la misma lógica con las cuatro
/// dependencias explícitas, de modo que la suite puede inyectar un vault
/// sintético y un transporte de mentira.
///
/// Reglas de la cascada (v16):
///
/// 1. El proveedor **pedido** es el primero y se usa aunque sea de pago
///    (es lo que el usuario pidió).
/// 2. Un proveedor **retirado** no se usa nunca, ni pedido ni de reserva.
/// 3. Un proveedor **deshabilitado** no se usa nunca, ni pedido ni de
///    reserva, y se avisa.
/// 4. El **fallback** sólo entra en proveedores gratuititos
///    (`free`/`keyless`/`offline`) salvo que la política permita pagar.
/// 5. El **SEERE local** es el último recurso: si la nube entera falla,
///    responde el motor local y el `fallback_used` es `true`.
/// 6. `fallback_used` es `true` **siempre** que el proveedor que respondió
///    no sea el pedido (v16: antes un modelo desconocido caía al primero de
///    la lista con `fallback_used = false`, mintiendo).
pub fn execute_chat_in(
    vault: &ProviderVault,
    transport: &dyn LlmTransport,
    local: &dyn LocalFallback,
    policy: &ChatPolicy,
    req: &ChatRequest,
) -> ChatResult {
    let t0 = Instant::now();
    let target_id = req
        .requested_model
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(vault.default_model.trim());

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
            prompt_tokens: 0,
            completion_tokens: 0,
            tools_used: Vec::new(),
            fallback_used: false,
        };
    }

    let (re_ctx, mut tools_used) =
        build_grounded_re_context(&req.prompt, req.attachment_path.as_deref());
    let mut enriched_prompt = if re_ctx.is_empty() {
        req.prompt.clone()
    } else {
        format!("{}\n\n[PREGUNTA DEL USUARIO]\n{}", re_ctx, req.prompt)
    };

    // Explicit Local SEERE v12 offline path
    if target_id == "local-seere" || target_id == "local" {
        let (_, pol) = crate::power_policy::current_policy();
        let res = local.run(&enriched_prompt, pol.max_turns.max(1));
        let ans = res["answer"]
            .as_str()
            .unwrap_or("Sin respuesta local")
            .to_string();
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
            prompt_tokens: 0,
            completion_tokens: 0,
            tools_used,
            fallback_used: false,
        };
    }

    // Hybrid path: run SEERE local shell/root tool discovery if shell/root intent + synthesize with MiniMax
    let actual_cloud_id = if target_id == "hybrid-seere" {
        let low = req.prompt.to_lowercase();
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
            // v16 (AUDIT-LLM-16-B): la telemetría se ejecutaba y se
            // DESCARTABA (`let _ = sys_snap;`): `tools_used` decía "RootSnap"
            // pero el modelo NUNCA veía la salida.  Ahora la salida entra en
            // el prompt => lo que se declara es lo que se usó.
            if !sys_snap.trim().is_empty() && !sys_snap.starts_with("ERR") {
                enriched_prompt = format!(
                    "{}\n\n[TELEMETRÍA LOCAL (RootSnap)]\n{}\n\n[PREGUNTA DEL USUARIO]\n{}",
                    re_ctx, sys_snap.trim(), req.prompt
                );
            }
        }
        "minimax-m3.1-flash"
    } else {
        target_id
    };

    let sys_prompt = req
        .system
        .as_deref()
        .unwrap_or_else(|| default_re_system_prompt());

    let (candidates, mut notes) = build_candidates(vault, actual_cloud_id, policy);

    let mut errors: Vec<String> = Vec::new();
    for (idx, p) in candidates.iter().enumerate() {
        // v16 (AUDIT-LLM-16-D): el presupuesto global manda sobre el gasto
        // **automático**.  Un proveedor de PAGO sólo entra si el presupuesto
        // lo permite; el proveedor **pedido explícitamente** no se bloquea
        // (el usuario lo pidió por su nombre: bloquearlo en silencio sería
        // peor que gastar).  El coste se RESERVA antes de llamar y se
        // devuelve si la llamada falla.
        let is_free = provider_catalog::tier_free(&p.tier);
        let mut reservation = None;
        if !is_free && p.id != actual_cloud_id {
            let cost = estimate_call_cost(p, policy.clamp_tokens(req.max_tokens));
            match policy.reserve_budget(cost) {
                Some(r) => reservation = Some(r),
                None => {
                    notes.push(format!(
                        "{} (tier {}) no entra en el fallback: fuera de presupuesto (${:.6})",
                        p.id, p.tier, cost
                    ));
                    continue;
                }
            }
        }
        let key = match p.auth_type.as_str() {
            "gh_cli" => match resolve_gh_token() {
                Some(t) => t,
                None => {
                    errors.push(format!(
                        "{}: sin token de 'gh auth token' (no se pudo autenticar)",
                        p.id
                    ));
                    continue;
                }
            },
            "none" => String::new(),
            "local" | "hybrid" => {
                // v16: un proveedor con `auth_type = local` NO se puede llamar
                // por HTTP OpenAI.  Antes caía en la rama `_` y, como no
                // tiene key, se saltaba **en silencio**: el proveedor
                // pedido desaparecía y la cascada continuaba sin avisar.
                errors.push(format!(
                    "{}: es un motor local ({}), no se llama por HTTP",
                    p.id, p.auth_type
                ));
                continue;
            }
            _ => {
                if p.api_key.trim().is_empty() {
                    errors.push(format!("{}: sin API key", p.id));
                    continue;
                }
                p.api_key.clone()
            }
        };

        let chat_req = ChatRequest {
            prompt: enriched_prompt.clone(),
            requested_model: Some(p.id.clone()),
            attachment_path: None,
            system: None,
            max_tokens: req.max_tokens,
            messages: req.messages.clone(),
        };

        match call_provider_typed(
            transport,
            policy,
            &p.endpoint,
            &p.model,
            &key,
            &p.auth_type,
            Some(sys_prompt),
            &chat_req,
        ) {
            Ok(parsed) => {
                let used_requested = p.id == actual_cloud_id;
                let mut reply = parsed.content.clone();
                if !notes.is_empty() {
                    reply = format!("{}\n\n[Nota del enrutador: {}]", reply, notes.join(" | "));
                }
                return ChatResult {
                    reply,
                    reasoning: parsed.reasoning,
                    model_id: p.id.clone(),
                    model_name: p.name.clone(),
                    vendor: p.vendor.clone(),
                    latency_ms: (t0.elapsed().as_secs_f64() * 1000.0).round(),
                    tokens_used: parsed.tokens_used,
                    prompt_tokens: parsed.prompt_tokens,
                    completion_tokens: parsed.completion_tokens,
                    tools_used,
                    fallback_used: !used_requested || idx > 0,
                };
            }
            Err(e) => {
                let safe = e.redacted(&key).to_string();
                errors.push(format!("{}: {}", p.id, safe));
                // La llamada falló: el dinero reservado vuelve al presupuesto.
                if let Some(r) = reservation {
                    policy.release_budget(r);
                }
            }
        }
    }

    // Final guaranteed offline fallback to Local SEERE v12 Rust GBNF
    let (_, pol) = crate::power_policy::current_policy();
    let res = local.run(&enriched_prompt, pol.max_turns.max(1));
    let ans = res["answer"].as_str().unwrap_or("Sin respuesta").to_string();
    if let Some(arr) = res["tools"].as_array() {
        for t in arr {
            let tn = t["tool"].as_str().unwrap_or("TOOL");
            let ta = t["arg"].as_str().unwrap_or("");
            tools_used.push(format!("{}:{}", tn, ta));
        }
    }
    let mut note = if errors.is_empty() {
        ans
    } else {
        format!(
            "{}\n\n[Fallback offline activo — errores cloud: {}]",
            ans,
            errors.join(" | ")
        )
    };
    if !notes.is_empty() {
        note = format!("{}\n\n[Nota del enrutador: {}]", note, notes.join(" | "));
    }
    ChatResult {
        reply: note,
        reasoning: String::new(),
        model_id: "local-seere".to_string(),
        model_name: "Local SEERE v12 (Offline Fallback)".to_string(),
        vendor: "Local-Exynos".to_string(),
        latency_ms: (t0.elapsed().as_secs_f64() * 1000.0).round(),
        tokens_used: res["tokens"].as_u64().unwrap_or(0),
        prompt_tokens: 0,
        completion_tokens: 0,
        tools_used,
        fallback_used: true,
    }
}

/// Construye la cascada de candidatos y las notas para el usuario.
///
/// Devuelve `(candidatos, notas)`.  Las notas son **visibles**: el usuario
/// tiene que saber si su proveedor fue descartado por estar deshabilitado,
/// por estar retirado o por ser de pago.
pub fn build_candidates(
    vault: &ProviderVault,
    target_id: &str,
    policy: &ChatPolicy,
) -> (Vec<ProviderEntry>, Vec<String>) {
    let mut notes: Vec<String> = Vec::new();
    let mut candidates: Vec<ProviderEntry> = Vec::new();

    let requested = vault.providers.iter().find(|p| p.id == target_id);
    match requested {
        Some(p) => {
            if let Some((_, rmodel, rnote)) = provider_catalog::is_retired(&p.id) {
                notes.push(format!(
                    "{} esta RETIRADO ({}): no se usa",
                    p.id, rnote
                ));
            } else if provider_catalog::is_retired(&p.model).is_some() {
                let (_, rmodel, rnote) = provider_catalog::is_retired(&p.model).unwrap();
                notes.push(format!(
                    "{} apunta al modelo RETIRADO {} ({}): no se usa",
                    p.id, rmodel, rnote
                ));
            } else if !p.enabled {
                // v16 (AUDIT-LLM-16-C): un proveedor deshabilitado se usaba
                // igual si era el pedido.  Ahora no, y se avisa.
                notes.push(format!("{} esta DESHABILITADO: no se usa", p.id));
            } else {
                candidates.push(p.clone());
            }
        }
        None => {
            notes.push(format!(
                "el proveedor pedido '{}' no esta en la boveda: se usa la cascada",
                target_id
            ));
        }
    }

    for p in &vault.providers {
        if p.id == target_id {
            continue;
        }
        if !p.enabled {
            continue;
        }
        if p.auth_type == "local" || p.auth_type == "hybrid" {
            continue;
        }
        // v16 (AUDIT-LLM-16-C): un proveedor RETIRADO no puede entrar en la
        // cascada de reserva.  Antes entraba: la comprobación de "retirado"
        // sólo se hacía sobre el modelo PEDIDO, no sobre los de reserva.
        if provider_catalog::is_retired(&p.id).is_some()
            || provider_catalog::is_retired(&p.model).is_some()
        {
            continue;
        }
        if !policy.allow_paid_fallback && !provider_catalog::tier_free(&p.tier) {
            // v16 (AUDIT-LLM-16-C): el fallback NO gasta dinero sin que se
            // pida.  Se anota para que el usuario lo sepa.
            notes.push(format!(
                "{} (tier {}) no entra en el fallback: es de pago",
                p.id, p.tier
            ));
            continue;
        }
        candidates.push(p.clone());
    }

    (candidates, notes)
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
