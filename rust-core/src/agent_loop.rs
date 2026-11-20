//! F3 — LLM agéntico recursivo (meta-agente) para MegaDaemon.
//!
//! El objetivo no es "una llamada al LLM" sino un **bucle**: decidir → llamar
//! una tool → **evaluar** → **corregirse**.  Con tres cosas que la mayoría de
//! los bucles no tienen y que aquí son requisito:
//!
//! 1. **Memoria de fallos** (`failures.ndjson`): acción, error y qué funcionó.
//!    El mismo error **no se repite en el siguiente ciclo**: antes de ejecutar,
//!    el bucle consulta la memoria y marca la acción como "evitar"; además el
//!    prompt del modelo **incluye** el resumen de lo que falló.
//! 2. **Presupuesto por hora** (no por acción) + **coste real**: tokens y
//!    latencia medidos, y el precio **con su fuente**.  Si no hay fuente ⇒
//!    `NO VERIFICADO`, nunca inventado.
//! 3. **Degradación honesta**: si el modelo devuelve basura, el bucle **no**
//!    inventa una acción: aplica un **criterio fijo** (igual que el
//!    `RuleBrain` de la voz: reclamar > preguntar > decir > callar).
//! 4. **El prompt NUNCA lleva la API key ni el token de sesión**: el prompt se
//!    construye aquí, con sólo la tarea, la memoria de fallos y la lista de
//!    proveedores (sin endpoints con credenciales).
//! 5. **Meta-agente**: enruta la petición al nodo/servicio correcto (el
//!    `service_bus` de W6 + el tablero) — eso es lo que el usuario llama
//!    "meta agente" y lo que quiere para Antigravity.
//! 6. **Circuit breaker de 3 estados** (cerrado/abierto/semiabierto) con
//!    backoff exponencial y **ventana de recuperación medida**: si cae el
//!    proveedor barato cae al siguiente, y **SEERE local es el último
//!    recurso** ⇒ el enjambre nunca se queda sin voz.
//!
//! **Restricción del móvil (load 17,65 · 128 % de IRQ · 876 procesos)**: cada
//! llamada a tool crea 2+ procesos (`su -mm -c sh -c …`).  Un bucle cerrado que
//! llame tools sin tope **tumba el móvil**.  Por eso el bucle tiene un **tope
//! duro de llamadas por tick** y un **tope de procesos de shell por tick**, y
//! el ejecutor debe reutilizar **un único shell root persistente** (socket
//! unix) en vez de un `su` por llamada.
//!
//! **No toca `providers.rs`**: el backend de chat se inyecta (`ChatBackend`).

use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Precio por modelo.  `source` es **obligatorio** para que el coste sea
/// citable; sin fuente el coste es `NO VERIFICADO` (no se inventa).
#[derive(Clone, Debug, Default)]
pub struct PriceEntry {
    pub usd_per_1k_input: Option<f64>,
    pub usd_per_1k_output: Option<f64>,
    /// Fuente del precio (URL o documento).  Sin fuente => `NO VERIFICADO`.
    pub source: Option<String>,
}

/// Resultado de coste: importe + su fuente, o `NO VERIFICADO`.
#[derive(Clone, Debug)]
pub struct Cost {
    pub usd: Option<f64>,
    pub source: Option<String>,
    pub note: String,
}

impl Cost {
    /// Coste sin precio verificable: se marca, no se estima a ojo.
    pub fn unverified(reason: &str) -> Self {
        Cost {
            usd: None,
            source: None,
            note: format!("NO VERIFICADO: {}", reason),
        }
    }

    pub fn is_verified(&self) -> bool {
        self.usd.is_some()
    }
}

/// Libro de precios por modelo.
#[derive(Clone, Debug, Default)]
pub struct PriceBook {
    entries: HashMap<String, PriceEntry>,
}

impl PriceBook {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra un precio **con su fuente**.  Sin fuente no se registra.
    pub fn with_source(
        mut self,
        model: &str,
        usd_per_1k_input: f64,
        usd_per_1k_output: f64,
        source: &str,
    ) -> Self {
        self.entries.insert(
            model.to_string(),
            PriceEntry {
                usd_per_1k_input: Some(usd_per_1k_input),
                usd_per_1k_output: Some(usd_per_1k_output),
                source: Some(source.to_string()),
            },
        );
        self
    }

    /// Registra un precio **sin fuente**: queda como `NO VERIFICADO`.
    pub fn without_source(mut self, model: &str) -> Self {
        self.entries.insert(
            model.to_string(),
            PriceEntry {
                usd_per_1k_input: None,
                usd_per_1k_output: None,
                source: None,
            },
        );
        self
    }

    /// Coste de una llamada.  Si el modelo no está en el libro (o su entrada
    /// no tiene fuente) ⇒ `NO VERIFICADO`, **nunca** un número inventado.
    pub fn cost_of(&self, model: &str, tokens_in: u64, tokens_out: u64) -> Cost {
        match self.entries.get(model) {
            None => Cost::unverified(&format!("no hay precio registrado para {}", model)),
            Some(e) => {
                let src = match &e.source {
                    None => {
                        return Cost::unverified(&format!(
                            "el precio de {} no tiene fuente citable",
                            model
                        ))
                    }
                    Some(s) => s.clone(),
                };
                let (Some(i), Some(o)) = (e.usd_per_1k_input, e.usd_per_1k_output) else {
                    return Cost::unverified(&format!("precio incompleto para {}", model));
                };
                let usd = (tokens_in as f64 / 1000.0) * i + (tokens_out as f64 / 1000.0) * o;
                Cost {
                    usd: Some(usd),
                    source: Some(src),
                    note: String::new(),
                }
            }
        }
    }
}

/// Entrada de la memoria de fallos (una línea de `failures.ndjson`).
#[derive(Clone, Debug)]
pub struct FailureEntry {
    pub ts: u64,
    pub tick: u32,
    pub provider: String,
    pub action: String,
    pub error: String,
    pub ok: bool,
    pub detail: String,
}

impl FailureEntry {
    fn to_json(&self) -> Value {
        json!({
            "ts": self.ts,
            "tick": self.tick,
            "provider": self.provider,
            "action": self.action,
            "error": self.error,
            "ok": self.ok,
            "detail": self.detail,
        })
    }

    fn from_json(v: &Value) -> Option<Self> {
        Some(FailureEntry {
            ts: v.get("ts").and_then(|x| x.as_u64()).unwrap_or(0),
            tick: v.get("tick").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
            provider: v
                .get("provider")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            action: v
                .get("action")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            error: v
                .get("error")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            ok: v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false),
            detail: v
                .get("detail")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
        })
    }
}

/// Memoria de fallos: NDJSON append-only en el `state_dir`.
#[derive(Clone, Debug)]
pub struct FailureMemory {
    path: PathBuf,
    entries: Vec<FailureEntry>,
    max_entries: usize,
}

impl FailureMemory {
    pub fn new(path: PathBuf) -> Self {
        let mut m = FailureMemory {
            path,
            entries: Vec::new(),
            max_entries: 500,
        };
        m.reload();
        m
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Recarga desde disco (NDJSON: una línea por entrada; las líneas
    /// corruptas se ignoran, no tumban la carga).
    pub fn reload(&mut self) {
        self.entries.clear();
        let Ok(text) = fs::read_to_string(&self.path) else {
            return;
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<Value>(line) {
                if let Some(e) = FailureEntry::from_json(&v) {
                    self.entries.push(e);
                }
            }
        }
        if self.entries.len() > self.max_entries {
            let drop = self.entries.len() - self.max_entries;
            self.entries.drain(0..drop);
        }
    }

    /// Registra un intento (éxito o fallo) y lo persiste.
    pub fn record(&mut self, e: FailureEntry) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("memoria de fallos: {}", e))?;
        }
        let line = serde_json::to_string(&e.to_json()).map_err(|e| e.to_string())?;
        use std::io::Write;
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| format!("memoria de fallos: {}", e))?;
        writeln!(f, "{}", line).map_err(|e| format!("memoria de fallos: {}", e))?;
        self.entries.push(e);
        if self.entries.len() > self.max_entries {
            let drop = self.entries.len() - self.max_entries;
            self.entries.drain(0..drop);
        }
        Ok(())
    }

    /// Errores recientes de una acción concreta (últimos `window` ticks).
    pub fn recent_errors(&self, action: &str, current_tick: u32, window: u32) -> Vec<String> {
        self.entries
            .iter()
            .filter(|e| !e.ok && e.action == action)
            .filter(|e| current_tick.saturating_sub(e.tick) <= window)
            .map(|e| e.error.clone())
            .collect()
    }

    /// ¿Hay que evitar esta acción?  Si el MISMO error ya ocurrió en la
    /// ventana reciente ⇒ sí (el bucle no repite el error).
    pub fn should_avoid(&self, action: &str, current_tick: u32, window: u32) -> Option<String> {
        self.recent_errors(action, current_tick, window)
            .into_iter()
            .next()
    }

    /// Resumen para el prompt: "acción → error (n veces)".
    pub fn summary(&self, current_tick: u32, window: u32) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for e in self.entries.iter() {
            if e.ok || current_tick.saturating_sub(e.tick) > window {
                continue;
            }
            let line = format!("{} -> {} (tick {})", e.action, e.error, e.tick);
            if !out.contains(&line) {
                out.push(line);
            }
        }
        out.truncate(10);
        out
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Error de presupuesto.
#[derive(Clone, Debug)]
pub enum BudgetError {
    Tokens { used: u64, limit: u64 },
    Usd { used: f64, limit: f64 },
    Calls { used: u32, limit: u32 },
}

impl std::fmt::Display for BudgetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BudgetError::Tokens { used, limit } => {
                write!(f, "presupuesto de tokens de la hora agotado ({} / {})", used, limit)
            }
            BudgetError::Usd { used, limit } => {
                write!(f, "presupuesto de coste de la hora agotado ({:.6} / {:.6})", used, limit)
            }
            BudgetError::Calls { used, limit } => {
                write!(f, "presupuesto de llamadas de la hora agotado ({} / {})", used, limit)
            }
        }
    }
}

/// Presupuesto **por hora** (ventana rodante), no por acción.
#[derive(Clone, Debug)]
pub struct HourBudget {
    pub token_limit: u64,
    pub usd_limit: Option<f64>,
    pub calls_limit: u32,
    pub window_secs: u64,
    window_start: u64,
    tokens: u64,
    usd: f64,
    calls: u32,
}

impl HourBudget {
    pub fn new(token_limit: u64, usd_limit: Option<f64>, calls_limit: u32) -> Self {
        HourBudget {
            token_limit,
            usd_limit,
            calls_limit,
            window_secs: 3600,
            window_start: 0,
            tokens: 0,
            usd: 0.0,
            calls: 0,
        }
    }

    fn roll(&mut self, now: u64) {
        if self.window_start == 0 {
            self.window_start = now;
            return;
        }
        if now.saturating_sub(self.window_start) >= self.window_secs {
            self.window_start = now;
            self.tokens = 0;
            self.usd = 0.0;
            self.calls = 0;
        }
    }

    /// ¿Queda presupuesto?  (sin consumir)
    pub fn allows(&mut self, now: u64, tokens: u64, cost: &Cost) -> Result<(), BudgetError> {
        self.roll(now);
        if self.tokens.saturating_add(tokens) > self.token_limit {
            return Err(BudgetError::Tokens {
                used: self.tokens,
                limit: self.token_limit,
            });
        }
        if let (Some(limit), Some(usd)) = (self.usd_limit, cost.usd) {
            if self.usd + usd > limit {
                return Err(BudgetError::Usd {
                    used: self.usd,
                    limit,
                });
                }
        }
        if self.calls + 1 > self.calls_limit {
            return Err(BudgetError::Calls {
                used: self.calls,
                limit: self.calls_limit,
            });
        }
        Ok(())
    }

    /// Consume presupuesto (tras la llamada real).
    pub fn spend(&mut self, now: u64, tokens: u64, cost: &Cost) {
        self.roll(now);
        self.tokens = self.tokens.saturating_add(tokens);
        self.calls = self.calls.saturating_add(1);
        if let Some(usd) = cost.usd {
            self.usd += usd;
        }
    }

    pub fn tokens_used(&mut self, now: u64) -> u64 {
        self.roll(now);
        self.tokens
    }

    pub fn usd_used(&mut self, now: u64) -> f64 {
        self.roll(now);
        self.usd
    }

    pub fn calls_used(&mut self, now: u64) -> u32 {
        self.roll(now);
        self.calls
    }
}

/// Estados del circuit breaker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakerState {
    Closed,
    Open,
    HalfOpen,
}

impl BreakerState {
    pub fn as_str(&self) -> &'static str {
        match self {
            BreakerState::Closed => "cerrado",
            BreakerState::Open => "abierto",
            BreakerState::HalfOpen => "semiabierto",
        }
    }
}

/// Circuit breaker de 3 estados con backoff exponencial y **ventana de
/// recuperación medida** (cuánto tardó el proveedor en volver a responder).
#[derive(Clone, Debug)]
pub struct CircuitBreaker {
    pub name: String,
    pub failure_threshold: u32,
    pub base_backoff_secs: u64,
    pub max_backoff_secs: u64,
    pub half_open_successes_to_close: u32,
    state: BreakerState,
    failures: u32,
    opened_at: u64,
    backoff_secs: u64,
    half_open_successes: u32,
    last_recovery_secs: Option<u64>,
    recoveries: Vec<u64>,
}

impl CircuitBreaker {
    pub fn new(name: &str) -> Self {
        CircuitBreaker {
            name: name.to_string(),
            failure_threshold: 3,
            base_backoff_secs: 30,
            max_backoff_secs: 900,
            half_open_successes_to_close: 1,
            state: BreakerState::Closed,
            failures: 0,
            opened_at: 0,
            backoff_secs: 0,
            half_open_successes: 0,
            last_recovery_secs: None,
            recoveries: Vec::new(),
        }
    }

    pub fn state(&self) -> BreakerState {
        self.state
    }

    pub fn failures(&self) -> u32 {
        self.failures
    }

    /// ¿Se permite una llamada ahora?  En abierto sólo tras el backoff
    /// (entonces pasa a semiabierto).
    pub fn allows(&mut self, now: u64) -> bool {
        match self.state {
            BreakerState::Closed | BreakerState::HalfOpen => true,
            BreakerState::Open => {
                if now.saturating_sub(self.opened_at) >= self.backoff_secs {
                    self.state = BreakerState::HalfOpen;
                    self.half_open_successes = 0;
                    true
                } else {
                    false
                }
            }
        }
    }

    pub fn on_success(&mut self, now: u64) {
        match self.state {
            BreakerState::HalfOpen => {
                self.half_open_successes += 1;
                if self.half_open_successes >= self.half_open_successes_to_close {
                    // **Ventana de recuperación MEDIDA**: del open al close.
                    let secs = now.saturating_sub(self.opened_at);
                    self.last_recovery_secs = Some(secs);
                    self.recoveries.push(secs);
                    if self.recoveries.len() > 20 {
                        self.recoveries.remove(0);
                    }
                    self.state = BreakerState::Closed;
                    self.failures = 0;
                    self.backoff_secs = 0;
                }
            }
            _ => {
                self.state = BreakerState::Closed;
                self.failures = 0;
            }
        }
    }

    pub fn on_failure(&mut self, now: u64) {
        self.failures = self.failures.saturating_add(1);
        if self.state == BreakerState::HalfOpen || self.failures >= self.failure_threshold {
            if self.state != BreakerState::Open {
                self.opened_at = now;
                self.backoff_secs = self.base_backoff_secs;
            } else {
                self.backoff_secs = (self.backoff_secs * 2).min(self.max_backoff_secs);
            }
            self.state = BreakerState::Open;
            self.half_open_successes = 0;
        }
    }

    /// Último tiempo de recuperación medido (segundos), si lo hubo.
    pub fn measured_recovery_secs(&self) -> Option<u64> {
        self.last_recovery_secs
    }

    /// Media de las recuperaciones medidas (para dimensionar el backoff con
    /// datos, no con intuición).
    pub fn mean_recovery_secs(&self) -> Option<f64> {
        if self.recoveries.is_empty() {
            return None;
        }
        let sum: u64 = self.recoveries.iter().sum();
        Some(sum as f64 / self.recoveries.len() as f64)
    }
}

/// Acción que el bucle puede ejecutar.  NUNCA se inventa una acción: si el
/// modelo no devuelve algo parseable, se usa el criterio fijo (`Action::Idle`
/// con el motivo) ✓.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Llamar a una tool del registro local.
    Tool { name: String, args: String },
    /// Enrutar a otro nodo/proveedor (meta-agente).
    Delegate { provider: String, task_type: String },
    /// Decir algo (difusión por el bus).
    Speak { text: String },
    /// No hacer nada, con motivo.
    Idle { reason: String },
}

impl Action {
    pub fn key(&self) -> String {
        match self {
            Action::Tool { name, .. } => format!("tool:{}", name),
            Action::Delegate { provider, .. } => format!("delegate:{}", provider),
            Action::Speak { .. } => "speak".to_string(),
            Action::Idle { .. } => "idle".to_string(),
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Action::Tool { name, args } => format!("tool {} {}", name, args),
            Action::Delegate { provider, task_type } => {
                format!("delegate {} ({})", provider, task_type)
            }
            Action::Speak { text } => format!("speak: {}", text),
            Action::Idle { reason } => format!("idle: {}", reason),
        }
    }
}

/// Parsea la decisión del modelo.  Acepta JSON (`{"action":"tool",...}`) o una
/// forma simple (`TOOL nombre {"args":...}`).  Si no hay nada interpretable
/// ⇒ `None` ⇒ el bucle degrada al criterio fijo ✓.
pub fn parse_decision(reply: &str) -> Option<Action> {
    let trimmed = reply.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
        let action = v.get("action").and_then(|x| x.as_str()).unwrap_or("");
        match action {
            "tool" => {
                let name = v.get("name").or_else(|| v.get("tool")).and_then(|x| x.as_str())?;
                let args = match v.get("args") {
                    Some(Value::String(s)) => s.clone(),
                    Some(other) => other.to_string(),
                    None => "{}".to_string(),
                };
                return Some(Action::Tool {
                    name: name.to_string(),
                    args,
                });
            }
            "delegate" => {
                let provider = v.get("provider").and_then(|x| x.as_str())?;
                let task_type = v
                    .get("task_type")
                    .or_else(|| v.get("task"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("general");
                return Some(Action::Delegate {
                    provider: provider.to_string(),
                    task_type: task_type.to_string(),
                });
            }
            "speak" => {
                let text = v.get("text").and_then(|x| x.as_str())?;
                return Some(Action::Speak {
                    text: text.to_string(),
                });
            }
            "idle" => {
                let reason = v.get("reason").and_then(|x| x.as_str()).unwrap_or("sin motivo");
                return Some(Action::Idle {
                    reason: reason.to_string(),
                });
            }
            _ => {}
        }
    }
    // Forma simple: "TOOL nombre {...}" / "DELEGATE proveedor tarea"
    let upper = trimmed.to_uppercase();
    if upper.starts_with("TOOL ") {
        let rest = trimmed[5..].trim();
        let mut it = rest.splitn(2, char::is_whitespace);
        let name = it.next().unwrap_or("").trim().to_string();
        let args = it.next().unwrap_or("{}").trim().to_string();
        if !name.is_empty() {
            return Some(Action::Tool { name, args });
        }
    }
    if upper.starts_with("DELEGATE ") {
        let rest = trimmed[8..].trim();
        let mut it = rest.splitn(2, char::is_whitespace);
        let provider = it.next().unwrap_or("").trim().to_string();
        let task_type = it.next().unwrap_or("general").trim().to_string();
        if !provider.is_empty() {
            return Some(Action::Delegate {
                provider,
                task_type,
            });
        }
    }
    if upper.starts_with("SPEAK ") {
        return Some(Action::Speak {
            text: trimmed[6..].trim().to_string(),
        });
    }
    None
}

/// Proveedor del enjambre (el que decide el meta-agente a quién enrutar).
#[derive(Clone, Debug)]
pub struct ProviderRef {
    pub id: String,
    pub model: String,
    /// `true` para el proveedor local (SEERE): es el **último recurso**.
    pub local: bool,
}

impl ProviderRef {
    pub fn remote(id: &str, model: &str) -> Self {
        ProviderRef {
            id: id.to_string(),
            model: model.to_string(),
            local: false,
        }
    }

    pub fn local(id: &str, model: &str) -> Self {
        ProviderRef {
            id: id.to_string(),
            model: model.to_string(),
            local: true,
        }
    }
}

/// Respuesta del backend de chat (inyectado: no toco `providers.rs`).
#[derive(Clone, Debug)]
pub struct ChatOutcome {
    pub reply: String,
    pub tokens_used: u64,
    pub prompt_tokens: u64,
    pub latency_ms: f64,
    pub model_id: String,
}

/// Backend de chat inyectable (normalmente `providers::execute_chat`).
pub trait ChatBackend {
    fn chat(&mut self, provider: &ProviderRef, prompt: &str) -> Result<ChatOutcome, String>;
}

/// Ejecutor de acciones (tools).  Debe reutilizar **un shell root
/// persistente**; `spawns_per_call` sirve para que el bucle pueda **contar**
/// los procesos que el móvil va a sufrir.
pub trait ToolExecutor {
    fn call(&mut self, action: &Action, tick: u32) -> Result<String, String>;
    /// Procesos que crea cada llamada (2+ si abre un `su` nuevo).
    fn spawns_per_call(&self) -> u32 {
        0
    }
    /// ¿El ejecutor usa un shell persistente?  Si no, el bucle avisa.
    fn uses_persistent_shell(&self) -> bool {
        false
    }
}

/// Router del meta-agente: decide a qué nodo/servicio va la petición
/// (en producción: `service_bus::select` + el tablero).
pub trait Router {
    fn route(&self, provider: &str, task_type: &str) -> Option<String>;
    /// Tarea libre que encaja con el criterio fijo (reclamar > preguntar >
    /// decir > callar).  Si no hay ninguna ⇒ `None`.
    fn fixed_fallback_task(&self) -> Option<String> {
        None
    }
}

/// Configuración del bucle.  Los topes por tick son **duros**: son lo que
/// impide que el bucle tumbe el móvil.
#[derive(Clone, Debug)]
pub struct AgentLoopConfig {
    /// Tope duro de llamadas al LLM por tick.
    pub max_calls_per_tick: u32,
    /// Tope duro de *procesos de shell* por tick (2 por llamada si se abre un
    /// `su` nuevo ⇒ el tope impide la tormenta).
    pub max_shell_spawns_per_tick: u32,
    /// Ventana (en ticks) en la que un error se considera "repetido".
    pub failure_window_ticks: u32,
    /// Presupuesto por hora.
    pub budget: HourBudget,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        AgentLoopConfig {
            max_calls_per_tick: 3,
            max_shell_spawns_per_tick: 4,
            failure_window_ticks: 5,
            budget: HourBudget::new(120_000, Some(1.0), 120),
        }
    }
}

/// Informe de un tick del bucle.
#[derive(Clone, Debug)]
pub struct TickReport {
    pub tick: u32,
    pub calls: u32,
    pub tokens: u64,
    pub cost: Cost,
    pub actions: Vec<String>,
    pub final_action: Action,
    pub degraded: bool,
    pub breakers: Vec<(String, BreakerState)>,
    pub provider_used: Option<String>,
    pub stopped_reason: Option<String>,
}

/// El bucle agéntico.
pub struct AgentLoop {
    pub cfg: AgentLoopConfig,
    pub failures: FailureMemory,
    pub prices: PriceBook,
    pub breakers: HashMap<String, CircuitBreaker>,
    pub providers: Vec<ProviderRef>,
    tick: u32,
}

impl AgentLoop {
    pub fn new(
        cfg: AgentLoopConfig,
        failures: FailureMemory,
        prices: PriceBook,
        providers: Vec<ProviderRef>,
    ) -> Self {
        let mut breakers = HashMap::new();
        for p in &providers {
            breakers.insert(p.id.clone(), CircuitBreaker::new(&p.id));
        }
        AgentLoop {
            cfg,
            failures,
            prices,
            breakers,
            providers,
            tick: 0,
        }
    }

    /// Ruta canónica de la memoria de fallos: `<state_dir>/failures.ndjson`.
    pub fn default_failures_path(state_dir: &str) -> PathBuf {
        Path::new(state_dir).join("failures.ndjson")
    }

    /// Construye el prompt.  **Nunca** incluye API key, token ni secretos:
    /// sólo la tarea, la memoria de fallos y la lista de proveedores.
    pub fn build_prompt(&self, task: &str) -> String {
        let summary = self.failures.summary(self.tick, self.cfg.failure_window_ticks);
        let providers: Vec<String> = self
            .providers
            .iter()
            .map(|p| {
                if p.local {
                    format!("{} (local, ultimo recurso)", p.id)
                } else {
                    p.id.clone()
                }
            })
            .collect();
        let mut p = String::new();
        p.push_str("Eres el meta-agente de MegaDaemon. Responde SOLO con una accion.\n");
        p.push_str("Formatos admitidos:\n");
        p.push_str("  {\"action\":\"tool\",\"name\":\"<tool>\",\"args\":{...}}\n");
        p.push_str("  {\"action\":\"delegate\",\"provider\":\"<id>\",\"task_type\":\"<tipo>\"}\n");
        p.push_str("  {\"action\":\"speak\",\"text\":\"...\"}\n");
        p.push_str(&format!("Tarea: {}\n", task));
        p.push_str(&format!("Proveedores disponibles: {}\n", providers.join(", ")));
        if !summary.is_empty() {
            p.push_str("Lo que YA fallo (no lo repitas):\n");
            for s in summary {
                p.push_str(&format!("  - {}\n", s));
            }
        }
        p.push_str("Provider local (SEERE) es el ultimo recurso: solo si los demas fallan.\n");
        p
    }

    /// Elige proveedor: cadena con breaker; el local es el último recurso.
    fn pick_provider(&mut self, now: u64, tried: &[String]) -> Option<ProviderRef> {
        let order: Vec<ProviderRef> = self
            .providers
            .iter()
            .filter(|p| !p.local)
            .cloned()
            .chain(self.providers.iter().filter(|p| p.local).cloned())
            .collect();
        for p in order {
            // Un proveedor que YA ha fallado en este tick no se reintenta:
            // se pasa al siguiente de la cadena (el local es el ultimo).
            if tried.iter().any(|t| *t == p.id) {
                continue;
            }
            let br = self
                .breakers
                .entry(p.id.clone())
                .or_insert_with(|| CircuitBreaker::new(&p.id));
            if br.allows(now) {
                return Some(p);
            }
        }
        None
    }

    /// Ejecuta UN tick completo: decide → actúa → evalúa → corrige.
    pub fn tick(
        &mut self,
        now: u64,
        task: &str,
        backend: &mut dyn ChatBackend,
        executor: &mut dyn ToolExecutor,
        router: &mut dyn Router,
    ) -> TickReport {
        self.tick = self.tick.saturating_add(1);
        let tick = self.tick;
        let mut report = TickReport {
            tick,
            calls: 0,
            tokens: 0,
            cost: Cost::unverified("no se ha llamado a ningun proveedor"),
            actions: Vec::new(),
            final_action: Action::Idle {
                reason: "sin accion".to_string(),
            },
            degraded: false,
            breakers: self
                .breakers
                .iter()
                .map(|(k, v)| (k.clone(), v.state()))
                .collect(),
            provider_used: None,
            stopped_reason: None,
        };

        let mut shell_spawns: u32 = 0;
        let mut last_error: Option<String> = None;
        // Proveedores ya intentados en ESTE tick (no se repiten aqui).
        let mut tried: Vec<String> = Vec::new();

        while report.calls < self.cfg.max_calls_per_tick {
            // 1) ¿Queda presupuesto de la hora?
            let probe_cost = Cost::unverified("antes de llamar no se conoce el coste");
            if let Err(e) = self.cfg.budget.allows(now, 0, &probe_cost) {
                report.stopped_reason = Some(e.to_string());
                report.final_action = Action::Idle {
                    reason: e.to_string(),
                };
                break;
            }

            // 2) Decide (con memoria de fallos en el prompt).
            let prompt = self.build_prompt(task);
            let Some(provider) = self.pick_provider(now, &tried) else {
                report.stopped_reason = Some("todos los proveedores en abierto".to_string());
                report.final_action = Action::Idle {
                    reason: "circuit breaker: no hay proveedor disponible".to_string(),
                };
                break;
            };
            report.provider_used = Some(provider.id.clone());
            tried.push(provider.id.clone());
            let outcome = backend.chat(&provider, &prompt);
            report.calls += 1;

            let outcome = match outcome {
                Ok(o) => {
                    if let Some(br) = self.breakers.get_mut(&provider.id) {
                        br.on_success(now);
                    }
                    o
                }
                Err(e) => {
                    if let Some(br) = self.breakers.get_mut(&provider.id) {
                        br.on_failure(now);
                    }
                    last_error = Some(e.clone());
                    let _ = self.failures.record(FailureEntry {
                        ts: now,
                        tick,
                        provider: provider.id.clone(),
                        action: "decision".to_string(),
                        error: e.clone(),
                        ok: false,
                        detail: "el proveedor fallo antes de decidir".to_string(),
                    });
                    continue; // siguiente proveedor en la cadena
                }
            };

            // 3) Coste real (tokens + latencia medidos; precio con fuente).
            let cost = self
                .prices
                .cost_of(&outcome.model_id, outcome.prompt_tokens, outcome.tokens_used);
            self.cfg
                .budget
                .spend(now, outcome.tokens_used, &cost);
            report.tokens = report.tokens.saturating_add(outcome.tokens_used);
            report.cost = cost;

            // 4) ¿La decisión es interpretable?  Si no => criterio FIJO.
            let Some(action) = parse_decision(&outcome.reply) else {
                report.degraded = true;
                let fallback = Self::fixed_fallback(router);
                report.actions.push(format!(
                    "respuesta no interpretable -> criterio fijo: {}",
                    fallback.describe()
                ));
                let _ = self.failures.record(FailureEntry {
                    ts: now,
                    tick,
                    provider: provider.id.clone(),
                    action: "decision".to_string(),
                    error: "respuesta no interpretable".to_string(),
                    ok: false,
                    detail: outcome.reply.chars().take(120).collect::<String>(),
                });
                self.run_action(
                    &fallback,
                    tick,
                    now,
                    executor,
                    router,
                    &mut report,
                    &mut shell_spawns,
                    &provider,
                );
                report.final_action = fallback;
                break;
            };

            // 5) Memoria de fallos: NO repetir el mismo error.
            if let Some(prev) = self
                .failures
                .should_avoid(&action.key(), tick, self.cfg.failure_window_ticks)
            {
                report.actions.push(format!(
                    "evitada {} (ya fallo con: {})",
                    action.key(),
                    prev
                ));
                let _ = self.failures.record(FailureEntry {
                    ts: now,
                    tick,
                    provider: provider.id.clone(),
                    action: action.key(),
                    error: prev,
                    ok: false,
                    detail: "evitada por memoria de fallos".to_string(),
                });
                continue; // el modelo ve la memoria en el siguiente prompt
            }

            // 6) Actúa y evalúa.
            let ok = self.run_action(
                &action,
                tick,
                now,
                executor,
                router,
                &mut report,
                &mut shell_spawns,
                &provider,
            );
            let _ = self.failures.record(FailureEntry {
                ts: now,
                tick,
                provider: provider.id.clone(),
                action: action.key(),
                error: if ok { String::new() } else { "error de la tool".to_string() },
                ok,
                detail: action.describe(),
            });
            report.final_action = action;
            if !ok {
                last_error = Some("la tool fallo".to_string());
                continue; // siguiente ciclo: la memoria ya impide repetir
            }
            break; // acción correcta: fin del tick
        }

        if report.actions.is_empty() {
            report.actions.push(format!(
                "sin llamadas (tope por tick: {})",
                self.cfg.max_calls_per_tick
            ));
        }
        if let Some(e) = last_error {
            if report.stopped_reason.is_none() {
                report.stopped_reason = Some(e);
            }
        }
        report
    }

    /// Criterio FIJO de degradación (nunca una acción inventada):
    /// reclamar una tarea del tablero > delegar al proveedor local > callar.
    fn fixed_fallback(router: &mut dyn Router) -> Action {
        if let Some(task) = router.fixed_fallback_task() {
            return Action::Tool {
                name: "claim_task".to_string(),
                args: json!({ "task": task }).to_string(),
            };
        }
        Action::Idle {
            reason: "el modelo no devolvio una accion utilizable: criterio fijo = callar".to_string(),
        }
    }

    /// Ejecuta la acción y registra el resultado.  `ok` = la acción funcionó.
    #[allow(clippy::too_many_arguments)]
    fn run_action(
        &mut self,
        action: &Action,
        tick: u32,
        now: u64,
        executor: &mut dyn ToolExecutor,
        router: &mut dyn Router,
        report: &mut TickReport,
        shell_spawns: &mut u32,
        provider: &ProviderRef,
    ) -> bool {
        // Tope duro de procesos de shell por tick (el móvil es el recurso
        // escaso: cada `su` son 2+ procesos).
        let spawns = executor.spawns_per_call();
        if *shell_spawns + spawns > self.cfg.max_shell_spawns_per_tick {
            report.actions.push(format!(
                "tope de shell del tick alcanzado ({} + {} > {}): accion descartada",
                shell_spawns,
                spawns,
                self.cfg.max_shell_spawns_per_tick
            ));
            return false;
        }
        *shell_spawns += spawns;

        match action {
            Action::Delegate { provider, task_type } => {
                match router.route(provider, task_type) {
                    Some(target) => {
                        report.actions.push(format!("enrutado a {} ({})", target, task_type));
                        true
                    }
                    None => {
                        report.actions.push(format!(
                            "sin destino para {} ({}): delegacion rechazada",
                            provider, task_type
                        ));
                        false
                    }
                }
            }
            _ => match executor.call(action, tick) {
                Ok(out) => {
                    let short: String = out.chars().take(160).collect();
                    report.actions.push(format!("{} -> {}", action.describe(), short));
                    true
                }
                Err(e) => {
                    report
                        .actions
                        .push(format!("{} -> ERROR {}", action.describe(), e));
                    let _ = self.failures.record(FailureEntry {
                        ts: now,
                        tick,
                        provider: provider.id.clone(),
                        action: action.key(),
                        error: e.clone(),
                        ok: false,
                        detail: "error al ejecutar la tool".to_string(),
                    });
                    false
                }
            },
        }
    }
}

/// Ahora en segundos (epoch).  Envuelto para poder testear sin reloj real.
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("megadaemon-f3-{}-{}", name, std::process::id()));
        p
    }

    struct FakeBackend {
        replies: Vec<Result<String, String>>,
        prompts: Vec<String>,
        models: Vec<String>,
    }

    impl FakeBackend {
        fn new(replies: Vec<Result<String, String>>) -> Self {
            FakeBackend {
                replies,
                prompts: Vec::new(),
                models: Vec::new(),
            }
        }
    }

    impl ChatBackend for FakeBackend {
        fn chat(&mut self, provider: &ProviderRef, prompt: &str) -> Result<ChatOutcome, String> {
            let idx = self.prompts.len();
            self.prompts.push(prompt.to_string());
            self.models.push(provider.model.clone());
            let r = self
                .replies
                .get(idx)
                .cloned()
                .unwrap_or_else(|| Ok("{\"action\":\"idle\",\"reason\":\"sin respuestas\"}".into()));
            r.map(|reply| ChatOutcome {
                reply,
                tokens_used: 100,
                prompt_tokens: 50,
                latency_ms: 12.0,
                model_id: provider.model.clone(),
            })
        }
    }

    struct FakeExecutor {
        calls: Vec<String>,
        fail_on: Option<String>,
        spawns: u32,
    }

    impl FakeExecutor {
        fn new(fail_on: Option<&str>) -> Self {
            FakeExecutor {
                calls: Vec::new(),
                fail_on: fail_on.map(|s| s.to_string()),
                spawns: 2,
            }
        }
    }

    impl ToolExecutor for FakeExecutor {
        fn call(&mut self, action: &Action, _tick: u32) -> Result<String, String> {
            let k = action.key();
            self.calls.push(k.clone());
            if let Some(f) = &self.fail_on {
                if *f == k {
                    return Err("boom".to_string());
                }
            }
            Ok("hecho".to_string())
        }
        fn spawns_per_call(&self) -> u32 {
            self.spawns
        }
    }

    struct FakeRouter {
        routed: Vec<String>,
        fallback: Option<String>,
    }

    impl Router for FakeRouter {
        fn route(&self, provider: &str, task_type: &str) -> Option<String> {
            Some(format!("{}::{}", provider, task_type))
        }
        fn fixed_fallback_task(&self) -> Option<String> {
            self.fallback.clone()
        }
    }

    fn loop_with(path: PathBuf) -> AgentLoop {
        let prices = PriceBook::new()
            .with_source("m1", 0.0001, 0.0002, "precio de prueba (fuente: test)")
            .without_source("m-sin-fuente");
        AgentLoop::new(
            AgentLoopConfig::default(),
            FailureMemory::new(path),
            prices,
            vec![ProviderRef::remote("p1", "m1"), ProviderRef::local("seere", "m-local")],
        )
    }

    #[test]
    fn el_mismo_error_no_se_repite_en_el_siguiente_ciclo() {
        let path = tmp_path("memoria");
        let _ = fs::remove_file(&path);
        let mut ag = loop_with(path.clone());
        let mut backend = FakeBackend::new(vec![
            Ok("{\"action\":\"tool\",\"name\":\"shell\",\"args\":{}}".into()),
            Ok("{\"action\":\"tool\",\"name\":\"shell\",\"args\":{}}".into()),
            Ok("{\"action\":\"tool\",\"name\":\"otra\",\"args\":{}}".into()),
        ]);
        let mut ex = FakeExecutor::new(Some("tool:shell"));
        let mut router = FakeRouter {
            routed: Vec::new(),
            fallback: None,
        };

        // Tick 1: el modelo elige "shell" y la tool falla => se registra.
        let r1 = ag.tick(1000, "tarea", &mut backend, &mut ex, &mut router);
        assert!(!r1.actions.is_empty());
        assert!(ag
            .failures
            .recent_errors("tool:shell", 1, 5)
            .iter()
            .any(|e| e == "boom"));

        // El bucle se corrige EN EL MISMO tick: la segunda propuesta de la
        // misma tool se evita gracias a la memoria de fallos.
        assert!(
            r1.actions.iter().any(|a| a.contains("evitada")),
            "la segunda vuelta deberia evitar la tool que ya fallo: {:?}",
            r1.actions
        );
        // Y la tool que fallo NO se ejecuto dos veces.
        assert_eq!(
            ex.calls.iter().filter(|c| *c == "tool:shell").count(),
            1,
            "la tool que falla no debe ejecutarse otra vez: {:?}",
            ex.calls
        );
        // Tick 2: el prompt ya lleva el resumen de lo que fallo.
        let mut backend2 = FakeBackend::new(vec![Ok("{\"action\":\"tool\",\"name\":\"otra\",\"args\":{}}".into())]);
        let mut ex2 = FakeExecutor::new(None);
        let mut router2 = FakeRouter { routed: Vec::new(), fallback: None };
        let r2 = ag.tick(1001, "tarea", &mut backend2, &mut ex2, &mut router2);
        assert!(backend2.prompts[0].contains("YA fallo"));
        assert!(ex2.calls.contains(&"tool:otra".to_string()), "debe usar otra tool: {:?}", r2.actions);
    }

    #[test]
    fn presupuesto_por_hora_no_por_accion() {
        let mut b = HourBudget::new(1000, None, 10);
        let c = Cost {
            usd: None,
            source: None,
            note: String::new(),
        };
        assert!(b.allows(100, 500, &c).is_ok());
        b.spend(100, 500, &c);
        // Segunda llamada del mismo minuto: aún hay presupuesto (1000 tokens).
        assert!(b.allows(100, 500, &c).is_ok());
        b.spend(100, 500, &c);
        // Agotado.
        assert!(b.allows(100, 1, &c).is_err());
        // Nueva hora => se reinicia la ventana.
        assert!(b.allows(100 + 3601, 1, &c).is_ok());
    }

    #[test]
    fn coste_con_fuente_y_no_verificado() {
        let pb = PriceBook::new()
            .with_source("m1", 0.001, 0.002, "https://precio.example/2026-01")
            .without_source("m-sin-fuente");
        let c = pb.cost_of("m1", 1000, 1000);
        assert!(c.is_verified());
        assert_eq!(c.source.as_deref(), Some("https://precio.example/2026-01"));
        assert!((c.usd.unwrap() - 0.003).abs() < 1e-9);

        let c2 = pb.cost_of("m-sin-fuente", 1000, 1000);
        assert!(!c2.is_verified());
        assert!(c2.note.contains("NO VERIFICADO"));
        assert!(c2.usd.is_none());

        let c3 = pb.cost_of("modelo-desconocido", 10, 10);
        assert!(!c3.is_verified());
        assert!(c3.usd.is_none());
    }

    #[test]
    fn degradacion_honesta_cuando_el_modelo_devuelve_basura() {
        let path = tmp_path("basura");
        let _ = fs::remove_file(&path);
        let mut ag = loop_with(path);
        let mut backend = FakeBackend::new(vec![Ok("asdfghjkl qwerty no es json".into())]);
        let mut ex = FakeExecutor::new(None);
        let mut router = FakeRouter {
            routed: Vec::new(),
            fallback: Some("t-123".to_string()),
        };
        let r = ag.tick(2000, "tarea", &mut backend, &mut ex, &mut router);
        assert!(r.degraded, "debe marcar degradacion");
        match &r.final_action {
            Action::Tool { name, .. } => assert_eq!(name, "claim_task"),
            other => panic!("no debe inventar acciones, obtuve {:?}", other),
        }
        // El primer prompt no puede tener fallos previos; la memoria ya lo
        // registra para el siguiente ciclo.
        assert!(!backend.prompts[0].contains("YA fallo"));
        assert!(ag.failures.recent_errors("decision", 1, 5).len() > 0);

        // Tick 2: el prompt ya lleva el resumen de lo que fallo.
        let mut backend2 = FakeBackend::new(vec![Ok("{\"action\":\"idle\",\"reason\":\"x\"}".into())]);
        let mut ex2 = FakeExecutor::new(None);
        let mut router2 = FakeRouter { routed: Vec::new(), fallback: Some("t-123".into()) };
        ag.tick(2001, "tarea", &mut backend2, &mut ex2, &mut router2);
        assert!(
            backend2.prompts[0].contains("YA fallo"),
            "el prompt siguiente debe llevar la memoria de fallos: {}",
            backend2.prompts[0]
        );
    }

    #[test]
    fn el_prompt_no_lleva_la_key_ni_el_token() {
        let path = tmp_path("prompt");
        let _ = fs::remove_file(&path);
        let ag = loop_with(path);
        let p = ag.build_prompt("tarea");
        assert!(!p.contains("X-Seere-Auth-Token"));
        assert!(!p.contains("X-API-Key"));
        assert!(!p.contains("mdaemon_sk"));
        assert!(p.contains("tarea"));
    }

    #[test]
    fn circuit_breaker_tres_estados_con_backoff() {
        let mut br = CircuitBreaker::new("p1");
        br.base_backoff_secs = 10;
        assert_eq!(br.state(), BreakerState::Closed);
        br.on_failure(100);
        br.on_failure(101);
        assert_eq!(br.state(), BreakerState::Closed); // umbral = 3
        br.on_failure(102);
        assert_eq!(br.state(), BreakerState::Open);
        assert!(!br.allows(105), "sigue en backoff");
        assert!(br.allows(112), "pasado el backoff => semiabierto");
        assert_eq!(br.state(), BreakerState::HalfOpen);
        br.on_success(115);
        assert_eq!(br.state(), BreakerState::Closed);
        // Ventana de recuperación MEDIDA.
        assert_eq!(br.measured_recovery_secs(), Some(13));
    }

    #[test]
    fn cadena_de_proveedores_y_ultimo_recurso_local() {
        let path = tmp_path("cadena");
        let _ = fs::remove_file(&path);
        let mut ag = loop_with(path);
        // "p1" falla => el bucle debe pasar a "seere" (el local).
        let mut backend = FakeBackend::new(vec![Err("sin credenciales".into())]);
        let mut ex = FakeExecutor::new(None);
        let mut router = FakeRouter {
            routed: Vec::new(),
            fallback: None,
        };
        let r = ag.tick(3000, "tarea", &mut backend, &mut ex, &mut router);
        assert_eq!(r.provider_used.as_deref(), Some("seere"));
        // El breaker de "p1" cuenta el fallo (umbral=3) pero todavia no se
        // abre: en el mismo tick basta con pasar al siguiente de la cadena.
        assert_eq!(ag.breakers.get("p1").unwrap().failures(), 1);
        assert_eq!(ag.breakers.get("p1").unwrap().state(), BreakerState::Closed);
    }

    #[test]
    fn tope_duro_de_llamadas_por_tick() {
        let path = tmp_path("tope");
        let _ = fs::remove_file(&path);
        let mut ag = loop_with(path);
        ag.cfg.max_calls_per_tick = 2;
        let mut backend = FakeBackend::new(vec![
            Err("e1".into()),
            Err("e2".into()),
            Err("e3".into()),
            Err("e4".into()),
        ]);
        let mut ex = FakeExecutor::new(None);
        let mut router = FakeRouter {
            routed: Vec::new(),
            fallback: None,
        };
        let r = ag.tick(4000, "tarea", &mut backend, &mut ex, &mut router);
        assert!(r.calls <= 2, "no debe pasar del tope: {}", r.calls);
    }
}
