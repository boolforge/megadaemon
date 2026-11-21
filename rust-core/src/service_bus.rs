//! MegaDaemon v15 — **Bus de servicios**: catálogo unificado de MODELOS y
//! AGENTES/SERVICIOS, con selección automática por tipo de tarea, *fallback*
//! real y presupuesto por tick/día.
//!
//! # Por qué existe
//!
//! El daemon tenía las piezas sueltas: `providers.rs` con 13 proveedores de
//! LLM, `mesh.rs` con el tablón del enjambre, `agent_voice.rs` con la voz del
//! daemon, `repo_updates.rs`/`git_ops.rs` para GitHub.  Pero **nadie las
//! unía**: la voz elegía modelo a mano (`LlmBrain::provider_id`), no había
//! forma de saber si un servicio estaba vivo sin inventarse la respuesta, y no
//! había presupuesto (el enjambre podía gastarse la cuota de un día entero
//! por una decisión tonta).
//!
//! Este módulo es el **catálogo + política de selección**:
//!
//! * **MODELOS**: lo que hay en `providers.rs` (MiniMax, Gemini, Groq,
//!   OpenRouter, GLM, Pollinations, SEERE local…).
//! * **AGENTES/SERVICIOS**: los otros nodos MegaDaemon (peers del mesh),
//!   Antigravity como cliente, servidores MCP de terceros y herramientas
//!   locales.
//!
//! Cada entrada tiene exactamente los campos que pide el plan maestro:
//! `id, kind, endpoint, auth_kind, health, latency, cost_estimate,
//! capabilities, enabled`.
//!
//! # Reglas duras
//!
//! * **La salud nunca se inventa.** Una entrada recién construida tiene
//!   `health.available = false` y `reason = "sin comprobar"`.  Sólo una
//!   comprobación real (un `connect()` TCP, un `last_seen` del mesh) puede
//!   poner `available = true`, y el motivo del fallo se guarda siempre.
//! * **Ningún secreto viaja en el catálogo.** De una credencial sólo se
//!   publica el *cómo* (`auth_kind`) y *dónde* (`auth_ref` = ruta del fichero
//!   de secretos).  Ni la key, ni su prefijo, ni su hash.
//! * **El presupuesto manda.** Si la llamada costaría más que lo que queda
//!   del tick o del día, el servicio no se elige (ni aunque sea el único).
//! * **El fallback es real**: si el proveedor elegido falla, se prueba el
//!   siguiente de la cadena y se anota el intento.
//!
//! Este módulo **no habla con nadie**: decide.  Quien ejecuta es el
//! llamante (la voz, la tool MCP, la UI).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Versión del esquema del catálogo (la UI la usa para detectar deriva).
pub const SERVICE_BUS_SCHEMA: &str = "service-bus/v1";

/// Tipos de tarea que el bus sabe enrutar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    /// Decidir / preguntar / responder (el prompt de la voz).
    Inference,
    /// Escribir o corregir código.
    Build,
    /// Analizar datos, binarios o logs.
    Analysis,
    /// Redactar texto para humanos.
    Writing,
}

impl TaskType {
    /// Nombre estable (`inference`, `build`, `analysis`, `writing`).
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskType::Inference => "inference",
            TaskType::Build => "build",
            TaskType::Analysis => "analysis",
            TaskType::Writing => "writing",
        }
    }

    /// Parsea el nombre (admite español: `inferencia`, `análisis`,
    /// `redacción`).
    pub fn parse(s: &str) -> Option<TaskType> {
        let k = s.trim().to_lowercase();
        let k = k
            .replace('ó', "o")
            .replace('í', "i")
            .replace('á', "a")
            .replace('é', "e")
            .replace('ú', "u");
        Some(match k.as_str() {
            "inference" | "inferencia" | "infer" | "decide" | "decision" => TaskType::Inference,
            "build" | "code" | "codigo" | "programacion" => TaskType::Build,
            "analysis" | "analisis" | "analyze" | "analyse" | "reasoning" => TaskType::Analysis,
            "writing" | "redaccion" | "write" | "draft" | "text" => TaskType::Writing,
            _ => return None,
        })
    }

    /// Capacidades que un servicio debe tener para este tipo de tarea.
    pub fn required_capabilities(&self) -> &'static [&'static str] {
        match self {
            TaskType::Inference => &["inference"],
            TaskType::Build => &["code"],
            TaskType::Analysis => &["analysis"],
            TaskType::Writing => &["writing"],
        }
    }

    /// Todos los tipos de tarea (para la UI).
    pub fn all() -> &'static [TaskType] {
        &[
            TaskType::Inference,
            TaskType::Build,
            TaskType::Analysis,
            TaskType::Writing,
        ]
    }
}

/// Qué clase de servicio es una entrada.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceKind {
    /// Un modelo de lenguaje (los de `providers.rs`).
    Model,
    /// Otro nodo del enjambre (otro MegaDaemon).
    Agent,
    /// Un **cliente** MCP de otro IDE/agente (Antigravity `com.agy`): no
    /// recibe trabajo, pero consume nuestras tools y habla con nuestros
    /// proveedores.
    AgentClient,
    /// Un servidor MCP de terceros.
    McpServer,
    /// Una herramienta local del propio daemon.
    LocalTool,
}

impl ServiceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ServiceKind::Model => "model",
            ServiceKind::Agent => "agent",
            ServiceKind::AgentClient => "agent_client",
            ServiceKind::McpServer => "mcp_server",
            ServiceKind::LocalTool => "local_tool",
        }
    }

    pub fn parse(s: &str) -> Option<ServiceKind> {
        Some(match s.trim().to_lowercase().as_str() {
            "model" | "modelo" | "llm" => ServiceKind::Model,
            "agent" | "agente" | "peer" | "node" => ServiceKind::Agent,
            "agent_client" | "client" | "cliente" | "mcp_client" => ServiceKind::AgentClient,
            "mcp_server" | "mcp" | "server" => ServiceKind::McpServer,
            "local_tool" | "tool" | "local" => ServiceKind::LocalTool,
            _ => return None,
        })
    }
}

/// Estado de salud **real** de un servicio.
///
/// `available` sólo es `true` si alguien lo comprobó de verdad.  Si no se ha
/// comprobado nunca, `available = false` y `reason = "sin comprobar"`: la
/// ausencia de datos **no** es un sí.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Health {
    /// ¿Se comprobó y respondió?
    pub available: bool,
    /// Cuándo se comprobó (ISO-8601); `None` = nunca comprobado.
    pub checked_at: Option<String>,
    /// Motivo por el que no está disponible (o por el que sí, en una línea).
    pub reason: String,
    /// Detalle de la comprobación (error, código HTTP, latencia...).
    pub detail: String,
}

impl Default for Health {
    fn default() -> Self {
        Health::unknown()
    }
}

impl Health {
    /// Salud **desconocida** (nunca comprobada): no disponible, con motivo.
    pub fn unknown() -> Health {
        Health {
            available: false,
            checked_at: None,
            reason: "sin comprobar".to_string(),
            detail: String::new(),
        }
    }

    /// Comprobación correcta.
    pub fn ok(latency_ms: u64) -> Health {
        Health {
            available: true,
            checked_at: Some(now_iso()),
            reason: "ok".to_string(),
            detail: format!("latencia {} ms", latency_ms),
        }
    }

    /// Comprobación fallida (con el motivo real).
    pub fn down(reason: &str, detail: &str) -> Health {
        Health {
            available: false,
            checked_at: Some(now_iso()),
            reason: reason.to_string(),
            detail: detail.to_string(),
        }
    }
}

/// Estimación de coste de un servicio (USD).  Los servicios gratuitos
/// (cuentas ya pagadas, modelos locales, keyless) tienen `free = true` y
/// coste 0: ese es el ahorro del enjambre.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CostEstimate {
    /// USD por 1k tokens de entrada.
    pub usd_per_1k_input: f64,
    /// USD por 1k tokens de salida.
    pub usd_per_1k_output: f64,
    /// USD fijos por llamada (si los hay).
    pub usd_per_call: f64,
    /// El servicio no cuesta dinero (cuota ya pagada / local).
    pub free: bool,
}

impl Default for CostEstimate {
    fn default() -> Self {
        CostEstimate::free()
    }
}

impl CostEstimate {
    /// Servicio gratuito.
    pub fn free() -> CostEstimate {
        CostEstimate {
            usd_per_1k_input: 0.0,
            usd_per_1k_output: 0.0,
            usd_per_call: 0.0,
            free: true,
        }
    }

    /// Servicio de pago con tarifa por token.
    pub fn per_token(usd_per_1k_input: f64, usd_per_1k_output: f64) -> CostEstimate {
        CostEstimate {
            usd_per_1k_input,
            usd_per_1k_output,
            usd_per_call: 0.0,
            free: false,
        }
    }

    /// Coste estimado de una llamada.
    pub fn estimate(&self, input_tokens: u32, output_tokens: u32) -> f64 {
        if self.free {
            return 0.0;
        }
        (input_tokens as f64 / 1000.0) * self.usd_per_1k_input
            + (output_tokens as f64 / 1000.0) * self.usd_per_1k_output
            + self.usd_per_call
    }

    /// Coste estimado de una llamada con una salida de `output_tokens`.
    pub fn estimate_call(&self, output_tokens: u32) -> f64 {
        self.estimate(0, output_tokens)
    }
}

/// Una entrada del catálogo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServiceEntry {
    /// Identificador estable (`minimax-m3.1-flash`, `peer-cloud-sandbox`).
    pub id: String,
    /// Clase de servicio.
    pub kind: ServiceKind,
    /// Nombre legible.
    pub label: String,
    /// Endpoint (URL, host:port o ruta local).  **Vacío** si no aplica.
    pub endpoint: String,
    /// *Cómo* se autentica: `none`, `bearer`, `anthropic`, `gh_cli`,
    /// `local`, `session_token`, `api_key_file`.
    pub auth_kind: String,
    /// *Dónde* está la credencial: ruta del fichero de secretos.  **Nunca la
    /// credencial.**
    pub auth_ref: String,
    /// ¿Está la credencial presente en ese fichero?  `None` = no aplica
    /// (servicio local o keyless) o no se ha comprobado.
    ///
    /// Ojo: esto **no** es la salud.  Un proveedor free sin key puede estar
    /// perfectamente disponible (keyless); lo que dice la salud es si el
    /// servicio responde.
    pub auth_ready: Option<bool>,
    /// Salud real.
    pub health: Health,
    /// Latencia medida en ms (`None` si nunca se midió).
    pub latency_ms: Option<u64>,
    /// Estimación de coste.
    pub cost_estimate: CostEstimate,
    /// Capacidades que ofrece.
    pub capabilities: Vec<String>,
    /// Subconjunto de tools que se expone a este servicio (vacío = todas).
    ///
    /// Antigravity (y cualquier cliente MCP) admite `enabled_tools` /
    /// `disabled_tools` por servidor: publicar menos tools = menos tokens
    /// para el cliente y menos superficie.
    pub tool_allowlist: Vec<String>,
    /// Si el servicio está habilitado.
    pub enabled: bool,
    /// Modelo concreto (para los `kind = model`).
    pub model: String,
    /// Tier del proveedor (`free`, `paid`, `local`...).
    pub tier: String,
    /// Notas libres (dónde está, cómo se usa...).
    pub notes: String,
}

impl ServiceEntry {
    /// Entrada mínima: salud **desconocida** (nunca inventada).
    pub fn new(id: &str, kind: ServiceKind) -> ServiceEntry {
        ServiceEntry {
            id: id.to_string(),
            kind,
            label: id.to_string(),
            endpoint: String::new(),
            auth_kind: "none".to_string(),
            auth_ref: String::new(),
            auth_ready: None,
            health: Health::unknown(),
            latency_ms: None,
            cost_estimate: CostEstimate::free(),
            capabilities: Vec::new(),
            tool_allowlist: Vec::new(),
            enabled: true,
            model: String::new(),
            tier: String::new(),
            notes: String::new(),
        }
    }

    /// ¿Tiene la capacidad pedida?  Una entrada **sin** capacidades se
    /// considera genérica y sirve para todo.
    pub fn supports(&self, capability: &str) -> bool {
        if self.capabilities.is_empty() {
            return true;
        }
        let cap = capability.trim().to_lowercase();
        self.capabilities
            .iter()
            .any(|c| c.trim().to_lowercase() == cap)
    }

    /// ¿Está en la red (o sea: depende de que haya internet)?
    pub fn is_remote(&self) -> bool {
        let e = self.endpoint.trim().to_lowercase();
        e.starts_with("http://") || e.starts_with("https://") || e.starts_with("ws://")
    }

    /// ¿Es local (offline, loopback, ruta local)?
    ///
    /// Un servicio en `127.0.0.1` **es** local aunque use `http://`: el
    /// SEERE del propio móvil no depende de internet y, por eso, es el
    /// último recurso (no el primero) cuando hay red.
    pub fn is_local(&self) -> bool {
        let e = self.endpoint.trim().to_lowercase();
        if e.starts_with("http://") || e.starts_with("https://") || e.starts_with("ws://") {
            return e.contains("127.0.0.1")
                || e.contains("localhost")
                || e.contains("[::1]")
                || e.contains("0.0.0.0");
        }
        true
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Presupuesto
// ─────────────────────────────────────────────────────────────────────────────

/// Presupuesto de gasto del bus (USD) por tick y por día.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Budget {
    /// Tope por tick.
    pub per_tick_usd: f64,
    /// Tope por día.
    pub per_day_usd: f64,
    /// Gastado en el tick en curso.
    pub spent_tick_usd: f64,
    /// Gastado en el día en curso.
    pub spent_day_usd: f64,
    /// Día (epoch / 86400) en el que se acumula el gasto.
    pub day_epoch: u64,
}

impl Default for Budget {
    fn default() -> Self {
        Budget::new(0.05, 1.0)
    }
}

impl Budget {
    /// Presupuesto nuevo (gasto a cero).
    pub fn new(per_tick_usd: f64, per_day_usd: f64) -> Budget {
        Budget {
            per_tick_usd,
            per_day_usd,
            spent_tick_usd: 0.0,
            spent_day_usd: 0.0,
            day_epoch: today_epoch(),
        }
    }

    /// Lo que queda del tick.
    pub fn remaining_tick(&self) -> f64 {
        (self.per_tick_usd - self.spent_tick_usd).max(0.0)
    }

    /// Lo que queda del día.
    pub fn remaining_day(&self) -> f64 {
        (self.per_day_usd - self.spent_day_usd).max(0.0)
    }

    /// ¿Permite este gasto?
    pub fn allows(&self, cost_usd: f64) -> bool {
        cost_usd <= self.remaining_tick() + 1e-12 && cost_usd <= self.remaining_day() + 1e-12
    }

    /// Anota un gasto (rota el día si ha cambiado).
    pub fn spend(&mut self, cost_usd: f64) {
        let today = today_epoch();
        if today != self.day_epoch {
            self.day_epoch = today;
            self.spent_day_usd = 0.0;
        }
        self.spent_tick_usd += cost_usd;
        self.spent_day_usd += cost_usd;
    }

    /// Reinicia el gasto del tick (el día sigue).
    pub fn reset_tick(&mut self) {
        self.spent_tick_usd = 0.0;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Política de selección
// ─────────────────────────────────────────────────────────────────────────────

/// Cómo se ordena la cadena de candidatos.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionPolicy {
    /// **Parásita**: primero lo gratis (cuota ya pagada, local, keyless),
    /// luego lo más barato.  Es la política por defecto del enjambre.
    Cheapest,
    /// **Capaz**: primero el servicio con más capacidades y mejor latencia;
    /// el local al final (es el último recurso, no el primero).
    Best,
}

/// Resultado de seleccionar un servicio para un tipo de tarea.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Selection {
    /// Tipo de tarea pedido.
    pub task: TaskType,
    /// Servicio elegido (si hay).
    pub chosen: Option<String>,
    /// Cadena de fallback, en orden.
    pub chain: Vec<String>,
    /// Servicios descartados y por qué (`id`, motivo).
    pub rejected: Vec<(String, String)>,
    /// Explicación en una línea.
    pub reason: String,
    /// Coste estimado de la llamada elegida.
    pub estimated_cost_usd: f64,
    /// Política usada.
    pub policy: SelectionPolicy,
}

/// Resultado de ejecutar con fallback.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RouteOutcome {
    /// ¿Hubo respuesta?
    pub ok: bool,
    /// La respuesta del servicio que respondió.
    pub reply: String,
    /// Id del servicio que respondió.
    pub used: Option<String>,
    /// Intentos: `(id, error)` de cada fallo.
    pub attempts: Vec<(String, String)>,
    /// Gasto real estimado.
    pub spent_usd: f64,
    /// Error final si no hubo respuesta.
    pub error: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Sondas de salud (comprobaciones REALES)
// ─────────────────────────────────────────────────────────────────────────────

/// Una sonda comprueba si un servicio está vivo.  **Nunca inventa**: si no
/// puede comprobar, devuelve `Health::unknown()`.
pub trait Prober {
    fn probe(&self, entry: &ServiceEntry) -> Health;
}

/// Sonda TCP: abre una conexión real al host:puerto del endpoint y mide la
/// latencia.  Si el host no resuelve o no acepta, el servicio está caído.
pub struct TcpProber {
    /// Tiempo máximo de espera de la conexión.
    pub timeout_ms: u64,
}

impl Default for TcpProber {
    fn default() -> Self {
        TcpProber { timeout_ms: 1500 }
    }
}

impl TcpProber {
    /// Extrae `host:puerto` de un endpoint (soporta `http://`, `https://` y
    /// `host:puerto` pelado).
    pub fn host_port(endpoint: &str) -> Option<(String, u16)> {
        let e = endpoint.trim();
        if e.is_empty() {
            return None;
        }
        let rest = if let Some(r) = e.strip_prefix("https://") {
            r
        } else if let Some(r) = e.strip_prefix("http://") {
            r
        } else if let Some(r) = e.strip_prefix("ws://") {
            r
        } else {
            e
        };
        let hostport = rest.split('/').next().unwrap_or(rest);
        let (host, port) = match hostport.rsplit_once(':') {
            Some((h, p)) => (h.to_string(), p.parse::<u16>().ok()?),
            None => (
                hostport.to_string(),
                if e.starts_with("https://") { 443 } else { 80 },
            ),
        };
        if host.is_empty() {
            return None;
        }
        Some((host, port))
    }

    fn connect_once(host: &str, port: u16, timeout_ms: u64) -> std::io::Result<u64> {
        use std::net::{TcpStream, ToSocketAddrs};
        use std::time::{Duration, Instant};
        let t0 = Instant::now();
        let mut last_err = std::io::Error::new(std::io::ErrorKind::Other, "sin dirección");
        let addrs = (host, port).to_socket_addrs()?;
        for addr in addrs {
            match TcpStream::connect_timeout(&addr, Duration::from_millis(timeout_ms)) {
                Ok(_) => return Ok(t0.elapsed().as_millis() as u64),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }
}

impl Prober for TcpProber {
    fn probe(&self, entry: &ServiceEntry) -> Health {
        if entry.endpoint.trim().is_empty() {
            return Health::down("sin endpoint", "no hay dirección que comprobar");
        }
        let (host, port) = match TcpProber::host_port(&entry.endpoint) {
            Some(hp) => hp,
            None => {
                return Health::down(
                    "endpoint no comprobable",
                    &format!("no parece host:puerto: {}", entry.endpoint),
                )
            }
        };
        match TcpProber::connect_once(&host, port, self.timeout_ms) {
            Ok(ms) => Health::ok(ms),
            Err(e) => Health::down("conexión fallida", &format!("{}:{} · {}", host, port, e)),
        }
    }
}

/// Sonda que nunca afirma nada: devuelve siempre "sin comprobar".
///
/// Sirve para el caso "no sé, y no voy a inventarlo" y para los tests.
pub struct UnknownProber;

impl Prober for UnknownProber {
    fn probe(&self, _entry: &ServiceEntry) -> Health {
        Health::unknown()
    }
}

/// Sonda de servicios locales: comprueba que la ruta exista en disco.
pub struct PathProber;

impl Prober for PathProber {
    fn probe(&self, entry: &ServiceEntry) -> Health {
        let p = entry.endpoint.trim();
        if p.is_empty() {
            return Health::down("sin ruta", "no hay ruta local que comprobar");
        }
        if std::path::Path::new(p).exists() {
            Health::ok(0)
        } else {
            Health::down("ruta inexistente", p)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// El bus
// ─────────────────────────────────────────────────────────────────────────────

/// Catálogo + política de selección.
#[derive(Clone, Debug, Default)]
pub struct ServiceBus {
    entries: Vec<ServiceEntry>,
    budget: Budget,
}

impl ServiceBus {
    /// Crea el bus con un catálogo (los ids duplicados se colapsan: gana el
    /// primero).
    pub fn new(entries: Vec<ServiceEntry>) -> ServiceBus {
        let mut out: Vec<ServiceEntry> = Vec::new();
        for e in entries {
            if e.id.trim().is_empty() {
                continue;
            }
            if out.iter().any(|x| x.id == e.id) {
                continue;
            }
            out.push(e);
        }
        ServiceBus {
            entries: out,
            budget: Budget::default(),
        }
    }

    /// Todas las entradas.
    pub fn entries(&self) -> &[ServiceEntry] {
        &self.entries
    }

    /// Una entrada por id.
    pub fn entry(&self, id: &str) -> Option<&ServiceEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// El presupuesto.
    pub fn budget(&self) -> &Budget {
        &self.budget
    }

    /// Fija el presupuesto.
    pub fn set_budget(&mut self, b: Budget) {
        self.budget = b;
    }

    /// Añade o sustituye una entrada.
    pub fn upsert(&mut self, e: ServiceEntry) {
        match self.entries.iter_mut().find(|x| x.id == e.id) {
            Some(slot) => *slot = e,
            None => self.entries.push(e),
        }
    }

    /// Quita una entrada; devuelve si estaba.
    pub fn remove(&mut self, id: &str) -> bool {
        let n = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() != n
    }

    /// Habilita/deshabilita una entrada.
    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<(), String> {
        match self.entries.iter_mut().find(|e| e.id == id) {
            Some(e) => {
                e.enabled = enabled;
                Ok(())
            }
            None => Err(format!("servicio desconocido: {}", id)),
        }
    }

    /// Fija la salud de una entrada (para cuando otro módulo ya la comprobó
    /// de verdad: el mesh, por ejemplo).
    pub fn set_health(&mut self, id: &str, health: Health) -> Result<(), String> {
        match self.entries.iter_mut().find(|e| e.id == id) {
            Some(e) => {
                if let Some(ms) = health.detail.strip_prefix("latencia ") {
                    if let Some(v) = ms.split(' ').next().and_then(|x| x.parse::<u64>().ok()) {
                        e.latency_ms = Some(v);
                    }
                }
                e.health = health;
                Ok(())
            }
            None => Err(format!("servicio desconocido: {}", id)),
        }
    }

    /// Comprueba la salud de todas las entradas con la sonda dada (y guarda
    /// el resultado real).
    pub fn probe_all(&mut self, prober: &dyn Prober) -> Vec<(String, Health)> {
        let mut out = Vec::new();
        for e in self.entries.iter_mut() {
            let h = prober.probe(e);
            if let Some(ms) = h.detail.strip_prefix("latencia ") {
                if let Some(v) = ms.split(' ').next().and_then(|x| x.parse::<u64>().ok()) {
                    e.latency_ms = Some(v);
                }
            }
            out.push((e.id.clone(), h.clone()));
            e.health = h;
        }
        out
    }

    /// Comprueba la salud de una entrada.
    pub fn probe_one(&mut self, id: &str, prober: &dyn Prober) -> Result<Health, String> {
        let e = self
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or_else(|| format!("servicio desconocido: {}", id))?;
        let h = prober.probe(e);
        if let Some(ms) = h.detail.strip_prefix("latencia ") {
            if let Some(v) = ms.split(' ').next().and_then(|x| x.parse::<u64>().ok()) {
                e.latency_ms = Some(v);
            }
        }
        e.health = h.clone();
        Ok(h)
    }

    /// Candidatos válidos para un tipo de tarea, **en orden de preferencia**.
    /// Quedan fuera (y se anota por qué): los deshabilitados, los que no
    /// tienen la capacidad, los que se sabe caídos (comprobados y con
    /// `available = false`) y los que no caben en el presupuesto.
    pub fn candidates(&self, task: TaskType) -> (Vec<&ServiceEntry>, Vec<(String, String)>) {
        let mut ok: Vec<&ServiceEntry> = Vec::new();
        let mut rejected: Vec<(String, String)> = Vec::new();
        for e in &self.entries {
            if !e.enabled {
                rejected.push((e.id.clone(), "deshabilitado".into()));
                continue;
            }
            let missing: Vec<&str> = task
                .required_capabilities()
                .iter()
                .copied()
                .filter(|c| !e.supports(c))
                .collect();
            if !missing.is_empty() {
                rejected.push((
                    e.id.clone(),
                    format!("no tiene la capacidad {}", missing.join(",")),
                ));
                continue;
            }
            // Caído *de verdad* (comprobado) → fuera.  Lo no comprobado se
            // intenta: el intento es la comprobación.
            if e.health.checked_at.is_some() && !e.health.available {
                rejected.push((e.id.clone(), format!("caído: {}", e.health.reason)));
                continue;
            }
            let cost = e.cost_estimate.estimate_call(DEFAULT_OUTPUT_TOKENS);
            if !self.budget.allows(cost) {
                rejected.push((
                    e.id.clone(),
                    format!(
                        "fuera de presupuesto (${:.6} > ${:.6} del tick)",
                        cost,
                        self.budget.remaining_tick()
                    ),
                ));
                continue;
            }
            ok.push(e);
        }
        (ok, rejected)
    }

    /// Selecciona un servicio para un tipo de tarea (sin ejecutar nada).
    pub fn select(&self, task: TaskType) -> Selection {
        self.select_with(SelectionPolicy::Cheapest, task)
    }

    /// Selecciona con política explícita.
    pub fn select_with(&self, policy: SelectionPolicy, task: TaskType) -> Selection {
        let (mut cands, rejected) = self.candidates(task);
        match policy {
            SelectionPolicy::Cheapest => cands.sort_by(|a, b| {
                let ca = a.cost_estimate.estimate_call(DEFAULT_OUTPUT_TOKENS);
                let cb = b.cost_estimate.estimate_call(DEFAULT_OUTPUT_TOKENS);
                ca.partial_cmp(&cb)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.latency_ms.cmp(&b.latency_ms))
                    .then(a.id.cmp(&b.id))
            }),
            SelectionPolicy::Best => cands.sort_by(|a, b| {
                // El local al final: es el último recurso, no el primero.
                let la = if a.is_local() { 1 } else { 0 };
                let lb = if b.is_local() { 1 } else { 0 };
                la.cmp(&lb)
                    .then(
                        b.capabilities
                            .len()
                            .cmp(&a.capabilities.len()),
                    )
                    .then(a.latency_ms.cmp(&b.latency_ms))
                    .then(a.id.cmp(&b.id))
            }),
        }
        let chain: Vec<String> = cands.iter().map(|e| e.id.clone()).collect();
        let chosen = chain.first().cloned();
        let cost = chosen
            .as_ref()
            .and_then(|id| self.entry(id))
            .map(|e| e.cost_estimate.estimate_call(DEFAULT_OUTPUT_TOKENS))
            .unwrap_or(0.0);
        let reason = match &chosen {
            Some(id) => {
                let e = self.entry(id).expect("existe");
                let verif = if e.health.available {
                    format!("verificado ({} ms)", e.latency_ms.unwrap_or(0))
                } else if e.health.checked_at.is_some() {
                    format!("caído: {}", e.health.reason)
                } else {
                    "sin verificar (se intentará)".to_string()
                };
                format!(
                    "{} para {} · {} · coste ${:.6} · {}",
                    e.id,
                    task.as_str(),
                    e.kind.as_str(),
                    cost,
                    verif
                )
            }
            None => format!(
                "ningún servicio sirve para {} ({} descartados)",
                task.as_str(),
                rejected.len()
            ),
        };
        Selection {
            task,
            chosen,
            chain,
            rejected,
            reason,
            estimated_cost_usd: cost,
            policy,
        }
    }

    /// Ejecuta una llamada con **fallback real**: prueba la cadena en orden y
    /// se queda con la primera que responda.  El gasto se anota en el
    /// presupuesto, y los servicios que fallan quedan marcados como caídos
    /// (evidencia real, no suposición).
    pub fn route(
        &mut self,
        task: TaskType,
        call: &mut dyn FnMut(&ServiceEntry) -> Result<String, String>,
    ) -> RouteOutcome {
        self.route_with(SelectionPolicy::Cheapest, task, call)
    }

    /// Igual que [`ServiceBus::route`] pero con política explícita.
    pub fn route_with(
        &mut self,
        policy: SelectionPolicy,
        task: TaskType,
        call: &mut dyn FnMut(&ServiceEntry) -> Result<String, String>,
    ) -> RouteOutcome {
        let sel = self.select_with(policy, task);
        let mut attempts: Vec<(String, String)> = Vec::new();
        let mut spent = 0.0f64;
        for id in &sel.chain {
            let (entry, cost) = {
                let e = match self.entry(id) {
                    Some(e) => e,
                    None => continue,
                };
                let c = e.cost_estimate.estimate_call(DEFAULT_OUTPUT_TOKENS);
                (e.clone(), c)
            };
            if !self.budget.allows(cost) {
                attempts.push((id.clone(), "sin presupuesto".to_string()));
                continue;
            }
            match call(&entry) {
                Ok(reply) => {
                    self.budget.spend(cost);
                    spent += cost;
                    if let Some(e) = self.entries.iter_mut().find(|x| x.id == *id) {
                        e.health = Health::ok(e.latency_ms.unwrap_or(0));
                    }
                    return RouteOutcome {
                        ok: true,
                        reply,
                        used: Some(id.clone()),
                        attempts,
                        spent_usd: spent,
                        error: None,
                    };
                }
                Err(err) => {
                    attempts.push((id.clone(), err.clone()));
                    // Evidencia real: este servicio falló ahora mismo.
                    if let Some(e) = self.entries.iter_mut().find(|x| x.id == *id) {
                        e.health = Health::down("fallo en la llamada", &err);
                    }
                }
            }
        }
        RouteOutcome {
            ok: false,
            reply: String::new(),
            used: None,
            attempts,
            spent_usd: spent,
            error: Some(
                sel.rejected
                    .iter()
                    .map(|(id, why)| format!("{}: {}", id, why))
                    .collect::<Vec<_>>()
                    .join("; "),
            ),
        }
    }

    /// El modelo que debe usar **la voz** para su prompt de decisión:
    /// MiniMax (o el mejor cloud disponible) si está vivo; el SEERE local si
    /// no hay red.  Devuelve también la cadena de fallback.
    pub fn voice_plan(&self) -> Selection {
        self.select_with(SelectionPolicy::Best, TaskType::Inference)
    }

    /// Atajo: id del modelo para la voz (`None` si no hay ninguno).
    pub fn voice_model(&self) -> Option<String> {
        self.voice_plan().chosen
    }

    /// Catálogo completo en JSON (para la UI y para los agentes).
    ///
    /// **No incluye secretos**: sólo `auth_kind` y `auth_ref` (la ruta del
    /// fichero).  Las claves nunca se copian aquí.
    pub fn catalog_json(&self) -> Value {
        let entries: Vec<Value> = self
            .entries
            .iter()
            .map(|e| {
                json!({
                    "id": e.id,
                    "kind": e.kind.as_str(),
                    "label": e.label,
                    "endpoint": e.endpoint,
                    "auth_kind": e.auth_kind,
                    "auth_ref": e.auth_ref,
                    "auth_ready": e.auth_ready,
                    "health": {
                        "available": e.health.available,
                        "checked_at": e.health.checked_at,
                        "reason": e.health.reason,
                        "detail": e.health.detail,
                    },
                    "latency_ms": e.latency_ms,
                    "cost_estimate": {
                        "free": e.cost_estimate.free,
                        "usd_per_1k_input": e.cost_estimate.usd_per_1k_input,
                        "usd_per_1k_output": e.cost_estimate.usd_per_1k_output,
                        "usd_per_call": e.cost_estimate.usd_per_call,
                    },
                    "capabilities": e.capabilities,
                    "tool_allowlist": e.tool_allowlist,
                    "enabled": e.enabled,
                    "model": e.model,
                    "tier": e.tier,
                    "notes": e.notes,
                })
            })
            .collect();

        let mut counts: HashMap<&str, u64> = HashMap::new();
        for e in &self.entries {
            *counts.entry(e.kind.as_str()).or_insert(0) += 1;
            if e.health.available {
                *counts.entry("available").or_insert(0) += 1;
            }
            if e.enabled {
                *counts.entry("enabled").or_insert(0) += 1;
            }
            if e.cost_estimate.free {
                *counts.entry("free").or_insert(0) += 1;
            }
        }

        let mut selections = serde_json::Map::new();
        for t in TaskType::all() {
            let s = self.select(*t);
            selections.insert(
                t.as_str().to_string(),
                json!({
                    "chosen": s.chosen,
                    "chain": s.chain,
                    "reason": s.reason,
                    "estimated_cost_usd": s.estimated_cost_usd,
                }),
            );
        }

        json!({
            "schema": SERVICE_BUS_SCHEMA,
            "generated_at": now_iso(),
            "counts": {
                "total": self.entries.len(),
                "models": counts.get("model").copied().unwrap_or(0),
                "agents": counts.get("agent").copied().unwrap_or(0),
                "mcp_servers": counts.get("mcp_server").copied().unwrap_or(0),
                "local_tools": counts.get("local_tool").copied().unwrap_or(0),
                "available": counts.get("available").copied().unwrap_or(0),
                "enabled": counts.get("enabled").copied().unwrap_or(0),
                "free": counts.get("free").copied().unwrap_or(0),
            },
            "budget": {
                "per_tick_usd": self.budget.per_tick_usd,
                "per_day_usd": self.budget.per_day_usd,
                "spent_tick_usd": self.budget.spent_tick_usd,
                "spent_day_usd": self.budget.spent_day_usd,
                "remaining_tick_usd": self.budget.remaining_tick(),
                "remaining_day_usd": self.budget.remaining_day(),
            },
            "task_types": TaskType::all().iter().map(|t| t.as_str()).collect::<Vec<_>>(),
            "entries": entries,
            "selection": Value::Object(selections),
        })
    }
}

/// Tokens de salida que se asumen al estimar el coste de una llamada de
/// decisión (los prompts de la voz son cortos).
pub const DEFAULT_OUTPUT_TOKENS: u32 = 256;

// ─────────────────────────────────────────────────────────────────────────────
// Constructores de catálogo
// ─────────────────────────────────────────────────────────────────────────────

/// Convierte el listado **redactado** de `providers::list_providers_redacted()`
/// en entradas del catálogo.
///
/// Se reutiliza el listado que ya existe (así no se duplica el catálogo de
/// proveedores ni se tocan las keys): de cada proveedor sale un `kind =
/// model` con su endpoint, su `auth_kind` y la **referencia** al fichero de
/// secretos (`secrets_ref`), nunca la credencial.
pub fn catalog_from_providers_json(redacted: &Value, secrets_ref: &str) -> Vec<ServiceEntry> {
    let mut out = Vec::new();
    let list = match redacted.get("providers").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return out,
    };
    for p in list {
        let id = p.get("id").and_then(|v| v.as_str()).unwrap_or("").trim();
        if id.is_empty() {
            continue;
        }
        let tier = p.get("tier").and_then(|v| v.as_str()).unwrap_or("");
        let model = p.get("model").and_then(|v| v.as_str()).unwrap_or("");
        let endpoint = p.get("endpoint").and_then(|v| v.as_str()).unwrap_or("");
        let auth_type = p.get("auth_type").and_then(|v| v.as_str()).unwrap_or("bearer");
        let ready = p.get("ready").and_then(|v| v.as_bool()).unwrap_or(false);

        let mut e = ServiceEntry::new(id, ServiceKind::Model);
        e.label = p
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(id)
            .to_string();
        e.endpoint = endpoint.to_string();
        e.auth_kind = auth_kind_for(auth_type).to_string();
        e.auth_ref = secrets_ref.to_string();
        e.auth_ready = Some(ready);
        e.model = model.to_string();
        e.tier = tier.to_string();
        e.enabled = p.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
        e.capabilities = model_capabilities(id, tier, model);
        e.cost_estimate = cost_for_model(model, tier);
        e.notes = if ready {
            "credencial presente en el fichero de secretos".to_string()
        } else {
            "sin credencial: el servicio sólo sirve si es local o keyless".to_string()
        };
        out.push(e);
    }
    out
}

/// `auth_kind` normalizado (los valores de `providers.rs` traducidos al
/// vocabulario del bus).
pub fn auth_kind_for(auth_type: &str) -> &'static str {
    match auth_type.trim().to_lowercase().as_str() {
        "none" | "keyless" => "none",
        "local" | "hybrid" => "local",
        "gh_cli" => "gh_cli",
        "anthropic" => "anthropic",
        "bearer" => "bearer",
        _ => "bearer",
    }
}

/// ¿Este tier no cuesta dinero (cuota ya pagada, keyless, local)?
pub fn tier_is_free(tier: &str) -> bool {
    matches!(
        tier.trim().to_lowercase().as_str(),
        "free" | "trial" | "keyless" | "offline" | "local" | "family" | "subscription"
    )
}

/// Capacidades que se deducen del proveedor (heurística estable).
pub fn model_capabilities(id: &str, tier: &str, model: &str) -> Vec<String> {
    let hay = format!("{} {} {}", id, tier, model).to_lowercase();
    let mut caps: Vec<String> = vec!["inference".into()];
    if hay.contains("local") || hay.contains("offline") || hay.contains("seere") {
        caps.push("offline".into());
        caps.push("analysis".into());
        caps.push("writing".into());
    }
    if hay.contains("code") || hay.contains("cod") {
        caps.push("code".into());
    }
    if hay.contains("gemini") {
        caps.push("vision".into());
        caps.push("analysis".into());
        caps.push("long_context".into());
    }
    if hay.contains("minimax") {
        caps.push("analysis".into());
        caps.push("long_context".into());
        caps.push("writing".into());
    }
    if hay.contains("gpt-oss") || hay.contains("groq") {
        caps.push("fast".into());
    }
    if hay.contains("openrouter") || hay.contains("ollama") {
        caps.push("writing".into());
    }
    caps.sort();
    caps.dedup();
    caps
}

/// Coste estimado de un modelo a partir de la tabla de precios del daemon
/// (`crate::delegate`): los modelos gratuitos cuestan 0.
pub fn cost_for_model(model: &str, tier: &str) -> CostEstimate {
    if crate::delegate::is_free_model(model) {
        return CostEstimate::free();
    }
    match crate::delegate::model_price(model) {
        Some((inp, out)) => CostEstimate::per_token(inp / 1000.0, out / 1000.0),
        None => {
            // Modelo sin tarifa publicada: se asume **de pago** y se marca
            // como estimación (la UI lo muestra como "estimado").
            if tier_is_free(tier) {
                CostEstimate::free()
            } else {
                CostEstimate::per_token(0.00125, 0.01)
            }
        }
    }
}

/// Convierte una fecha ISO-8601 UTC (`2026-11-19T12:00:00Z`) a epoch.
///
/// El tablón del mesh guarda `last_seen` como **ISO-8601** (no epoch), así
/// que sin esto la salud de los peers sería "sin contacto" siempre.
pub fn parse_iso_epoch(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() < 19 {
        return None;
    }
    let num = |a: usize, z: usize| -> Option<i64> { s.get(a..z)?.parse::<i64>().ok() };
    let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, mi, se) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    // Inverso de `days_from_civil` (Hinnant).
    let y_adj = if mo <= 2 { y - 1 } else { y };
    let era = if y_adj >= 0 { y_adj } else { y_adj - 399 } / 400;
    let yoe = y_adj - era * 400;
    let mp = if mo > 2 { mo - 3 } else { mo + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + mi * 60 + se)
}

/// Extrae el `last_seen` de un peer (epoch o ISO-8601).
fn last_seen_epoch(p: &Value) -> Option<i64> {
    let v = p.get("last_seen")?;
    if let Some(n) = v.as_i64() {
        return Some(n);
    }
    v.as_str().and_then(parse_iso_epoch)
}

/// Convierte el *snapshot* del mesh (tablón) en entradas `kind = agent`.
///
/// La salud de un peer sale de su `last_seen` real: si está más viejo que el
/// TTL, el servicio está **caído** (con el motivo), no "disponible".
pub fn catalog_from_mesh_snapshot(snapshot: &Value, stale_secs: i64) -> Vec<ServiceEntry> {
    let mut out = Vec::new();
    let now = now_epoch() as i64;
    let peers = match snapshot.get("peers").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return out,
    };
    for p in peers {
        let id = p.get("id").and_then(|v| v.as_str()).unwrap_or("").trim();
        if id.is_empty() {
            continue;
        }
        let mut e = ServiceEntry::new(id, ServiceKind::Agent);
        e.label = p
            .get("hostname")
            .and_then(|v| v.as_str())
            .unwrap_or(id)
            .to_string();
        e.endpoint = p
            .get("endpoint")
            .or_else(|| p.get("addr"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        e.auth_kind = "session_token".to_string();
        e.auth_ref = "md-secrets/instance.env".to_string();
        e.capabilities = p
            .get("capabilities")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        e.cost_estimate = CostEstimate::free();
        let last_seen = last_seen_epoch(p).unwrap_or(0);
        let age = now - last_seen;
        if last_seen == 0 {
            e.health = Health::down("sin contacto", "el peer nunca ha hecho heartbeat");
        } else if age > stale_secs {
            e.health = Health::down(
                "sin contacto",
                &format!("último heartbeat hace {} s (TTL {} s)", age, stale_secs),
            );
        } else {
            e.health = Health::ok((age.max(0) * 1000) as u64);
            e.latency_ms = Some((age.max(0) * 1000) as u64);
        }
        out.push(e);
    }
    out
}

/// Entrada para un servidor MCP de terceros.
pub fn seed_mcp_server(
    id: &str,
    endpoint: &str,
    auth_kind: &str,
    auth_ref: &str,
    capabilities: &[&str],
) -> ServiceEntry {
    let mut e = ServiceEntry::new(id, ServiceKind::McpServer);
    e.label = id.to_string();
    e.endpoint = endpoint.to_string();
    e.auth_kind = auth_kind.to_string();
    e.auth_ref = auth_ref.to_string();
    e.capabilities = capabilities.iter().map(|c| c.to_string()).collect();
    e.cost_estimate = CostEstimate::free();
    e.notes = "servidor MCP de terceros (se comprueba al usarlo)".to_string();
    e
}

/// Entrada para una herramienta local del daemon.
pub fn seed_local_tool(id: &str, path: &str, capabilities: &[&str]) -> ServiceEntry {
    let mut e = ServiceEntry::new(id, ServiceKind::LocalTool);
    e.label = id.to_string();
    e.endpoint = path.to_string();
    e.auth_kind = "none".to_string();
    e.capabilities = capabilities.iter().map(|c| c.to_string()).collect();
    e.cost_estimate = CostEstimate::free();
    e.notes = "herramienta local del daemon".to_string();
    e
}

/// Construye el bus con el catálogo **real** del daemon:
///
/// * **modelos**: del listado redactado de `providers.rs` (13 proveedores);
/// * **agentes**: de los peers del tablón (con su `last_seen` real);
/// * **cliente MCP**: Antigravity (`com.agy`), si se conoce su endpoint.
///
/// `providers_secrets_ref` es la ruta del fichero de secretos: al catálogo
/// sólo llega esa ruta, nunca la credencial.
pub fn bus_from_runtime(
    providers_redacted: &Value,
    providers_secrets_ref: &str,
    mesh_snapshot: &Value,
    stale_secs: i64,
    mcp_endpoint: &str,
) -> ServiceBus {
    let mut entries = catalog_from_providers_json(providers_redacted, providers_secrets_ref);
    entries.extend(catalog_from_mesh_snapshot(mesh_snapshot, stale_secs));
    if !mcp_endpoint.trim().is_empty() {
        entries.push(seed_antigravity_client(mcp_endpoint));
    }
    ServiceBus::new(entries)
}

/// Entrada de Antigravity como **cliente** del MCP del móvil.
///
/// No es un servidor: es un cliente que habla con nosotros.  Se cataloga
/// para que la UI vea el otro lado de la conexión, pero su "endpoint" es el
/// MCP del propio daemon y su salud se comprueba contra el propio daemon.
///
/// Datos del nodo W5 (`docs/ANTIGRAVITY_INTEL.md`): paquete `com.agy`
/// ("AGY v2" para Android), runtime interno "Jetski", plugin oficial
/// `google-antigravity-sdk`, skill `minimax-megadaemon` y puertos reservados
/// 45157 (release) / 45158 (dev) — que **no** se usan para servidores.
pub fn seed_antigravity_client(mcp_endpoint: &str) -> ServiceEntry {
    let mut e = ServiceEntry::new("antigravity", ServiceKind::AgentClient);
    e.label = "Antigravity (cliente MCP, com.agy)".to_string();
    e.endpoint = mcp_endpoint.to_string();
    e.auth_kind = "api_key_and_session_token".to_string();
    e.auth_ref = "md-secrets/instance.env".to_string();
    e.capabilities = vec![
        "mcp_client".into(),
        "mcp_stdio_server".into(),
        "task_daemon".into(),
        "byok_models".into(),
        "ide".into(),
    ];
    e.cost_estimate = CostEstimate::free();
    e.notes = "cliente MCP (SDK oficial de Google): se conecta a nuestro /mcp; \
               su config `megadaemon-minimax` (stdio) está ROTO porque lanza \
               una copia del binario que no existe en el dispositivo: usar el \
               puente HTTP (X-API-Key + X-Seere-Auth-Token; el token sale de \
               POST /api/session/exchange en el propio móvil)"
        .to_string();
    e
}

/// Nombre del servidor MCP en la config de Antigravity.
///
/// **No se cambia**: la skill `minimax-megadaemon` y el `config.json` de
/// Antigravity lo referencian por nombre (`mcp(megadaemon-minimax/minimax_chat)`).
/// Cambiarlo dejaría las allow-lists apuntando a un servidor inexistente.
pub const ANTIGRAVITY_SERVER_NAME: &str = "megadaemon-minimax";

/// Puente MCP que se le configura a Antigravity (`mcp_config.json`).
///
/// **La clave no se incrusta**: `api_key_ref` es la ruta del fichero de
/// secretos y el llamante inyecta los valores con [`bridge_with_key`] y
/// [`bridge_with_token`] en el momento de escribir la config.
///
/// El `/mcp` del daemon exige **dos** cabeceras (medido en el móvil):
/// `X-API-Key` **y** `X-Seere-Auth-Token` (el token de sesión se obtiene en
/// el propio dispositivo con `POST /api/session/exchange`, API key en el
/// cuerpo ⇒ el secreto nunca sale del móvil).
pub fn antigravity_bridge_config(
    mcp_url: &str,
    api_key_ref: &str,
    enabled_tools: &[&str],
) -> Value {
    let mut server = json!({
        "url": mcp_url,
        "headers": {
            "X-API-Key": format!("${{{}}}", api_key_ref),
            "X-Seere-Auth-Token": "${session_token}",
        },
    });
    if !enabled_tools.is_empty() {
        server["enabled_tools"] = json!(enabled_tools);
    }
    json!({
        "mcpServers": {
            ANTIGRAVITY_SERVER_NAME: server,
        }
    })
}

/// Inyecta la API key real en el puente (en el momento de escribir la
/// config, nunca antes: el valor no pasa por el catálogo ni por el tablón).
pub fn bridge_with_key(mut cfg: Value, api_key: &str) -> Value {
    if let Some(server) = cfg
        .get_mut("mcpServers")
        .and_then(|m| m.get_mut(ANTIGRAVITY_SERVER_NAME))
    {
        if let Some(h) = server.get_mut("headers").and_then(|h| h.as_object_mut()) {
            h.insert("X-API-Key".to_string(), json!(api_key));
        }
    }
    cfg
}

/// Inyecta el token de sesión (`POST /api/session/exchange` en el móvil).
pub fn bridge_with_token(mut cfg: Value, token: &str) -> Value {
    if let Some(server) = cfg
        .get_mut("mcpServers")
        .and_then(|m| m.get_mut(ANTIGRAVITY_SERVER_NAME))
    {
        if let Some(h) = server.get_mut("headers").and_then(|h| h.as_object_mut()) {
            h.insert("X-Seere-Auth-Token".to_string(), json!(token));
        }
    }
    cfg
}

/// **Fusiona** el puente en la config existente **sin perder nada**.
///
/// `mcp_config.json` de Antigravity tiene dos claves raíz: `mcpServers` **y**
/// `providers` (el BYOK `providers.minimax`).  Sobrescribir el fichero
/// entero dejaría a Antigravity sin modelo ⇒ aquí sólo se sustituye
/// `mcpServers["megadaemon-minimax"]` y el resto se conserva byte a byte
/// (semánticamente: se conserva cada clave y cada valor).
pub fn merge_bridge_config(existing_raw: &str, bridge: &Value) -> Result<String, String> {
    let mut root: Value = if existing_raw.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(existing_raw).map_err(|e| format!("config inválida: {}", e))?
    };
    if !root.is_object() {
        return Err("la config existente no es un objeto JSON".into());
    }
    let incoming = bridge
        .get("mcpServers")
        .and_then(|m| m.get(ANTIGRAVITY_SERVER_NAME))
        .cloned()
        .ok_or_else(|| "el puente no trae el servidor esperado".to_string())?;

    let obj = root.as_object_mut().expect("objeto");
    let servers = obj
        .entry("mcpServers".to_string())
        .or_insert_with(|| json!({}));
    if !servers.is_object() {
        *servers = json!({});
    }
    let servers = servers.as_object_mut().expect("objeto");
    let prev = servers
        .get(ANTIGRAVITY_SERVER_NAME)
        .cloned()
        .unwrap_or(Value::Null);
    servers.insert(ANTIGRAVITY_SERVER_NAME.to_string(), merge_server(prev, incoming));
    serde_json::to_string_pretty(&root).map_err(|e| e.to_string())
}

/// Fusiona una entrada de servidor con la que ya había.
///
/// Regla: **las cabeceras reales que ya están se conservan** (no se pisan con
/// placeholders); el resto lo manda el puente (url, allowlist).
fn merge_server(prev: Value, incoming: Value) -> Value {
    let mut out = incoming.clone();
    let prev_headers = prev
        .get("headers")
        .and_then(|h| h.as_object())
        .cloned()
        .unwrap_or_default();
    if prev_headers.is_empty() {
        return out;
    }
    if let Some(h) = out.get_mut("headers").and_then(|h| h.as_object_mut()) {
        for (k, v) in prev_headers {
            let es_real = v
                .as_str()
                .map(|s| !s.is_empty() && !s.starts_with("${"))
                .unwrap_or(false);
            if es_real {
                h.insert(k, v);
            }
        }
    }
    out
}

/// Escribe el puente en la config real de Antigravity: **merge + backup +
/// rename atómico**, e idempotente (se puede reejecutar sin miedo).
///
/// Devuelve el resumen (rutas y tamaños), **jamás** el contenido de las
/// cabeceras.
pub fn apply_bridge_file(path: &str, bridge: &Value) -> Result<Value, String> {
    let p = std::path::Path::new(path);
    let existing = std::fs::read_to_string(p).unwrap_or_default();
    let merged = merge_bridge_config(&existing, bridge)?;
    // Backup antes de tocar nada.
    let mut backup = String::new();
    if p.exists() {
        backup = format!("{}.bak-{}", path, now_epoch());
        std::fs::copy(p, &backup).map_err(|e| format!("no se pudo hacer backup: {}", e))?;
    }
    // Escritura atómica: fichero temporal + rename (nunca a medias).
    let tmp = format!("{}.tmp-{}", path, now_epoch());
    std::fs::write(&tmp, merged.as_bytes()).map_err(|e| format!("no se pudo escribir: {}", e))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("no se pudo renombrar: {}", e))?;
    // Permisos restrictivos: el fichero lleva credenciales.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(json!({
        "ok": true,
        "path": path,
        "backup": backup,
        "server": ANTIGRAVITY_SERVER_NAME,
        "bytes": merged.len(),
        "preserved_root_keys": serde_json::from_str::<Value>(&merged)
            .ok()
            .and_then(|v| v.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>()))
            .unwrap_or_default(),
    }))
}

/// Adaptador genérico: convierte una entrada JSON hecha por otro módulo
/// (p.ej. `antigravity_intel::service_bus_entry`) en una [`ServiceEntry`].
///
/// **No inventa salud**: si el JSON no trae `healthy`, la entrada nace
/// "sin comprobar"; si lo trae, se respeta tal cual (con su motivo).
pub fn entry_from_json(v: &Value) -> Option<ServiceEntry> {
    let id = v.get("id").and_then(|x| x.as_str())?.trim();
    if id.is_empty() {
        return None;
    }
    let kind = v
        .get("kind")
        .and_then(|x| x.as_str())
        .and_then(ServiceKind::parse)
        .unwrap_or(ServiceKind::Agent);
    let mut e = ServiceEntry::new(id, kind);
    e.label = v
        .get("label")
        .and_then(|x| x.as_str())
        .unwrap_or(id)
        .to_string();
    e.endpoint = v
        .get("endpoint")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    e.auth_kind = v
        .get("auth_kind")
        .and_then(|x| x.as_str())
        .unwrap_or("none")
        .to_string();
    e.auth_ref = v
        .get("auth_ref")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    e.capabilities = v
        .get("capabilities")
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|c| c.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    e.tool_allowlist = v
        .get("tool_allowlist")
        .or_else(|| v.get("enabled_tools"))
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|c| c.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    e.enabled = v.get("enabled").and_then(|x| x.as_bool()).unwrap_or(true);
    e.auth_ready = v.get("auth_ready").and_then(|x| x.as_bool());
    e.model = v
        .get("model")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    e.tier = v.get("tier").and_then(|x| x.as_str()).unwrap_or("").to_string();
    e.notes = v
        .get("notes")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    e.cost_estimate = match v.get("cost_estimate") {
        Some(Value::String(s)) if s == "included" || s == "free" => CostEstimate::free(),
        Some(Value::Object(o)) => {
            let g = |k: &str| o.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0);
            CostEstimate {
                usd_per_1k_input: g("usd_per_1k_input"),
                usd_per_1k_output: g("usd_per_1k_output"),
                usd_per_call: g("usd_per_call"),
                free: o.get("free").and_then(|x| x.as_bool()).unwrap_or(false),
            }
        }
        _ => CostEstimate::free(),
    };
    // Salud: sólo si el JSON la trae medida.
    match v.get("healthy").and_then(|x| x.as_bool()) {
        Some(true) => {
            e.health = Health::ok(v.get("latency_ms").and_then(|x| x.as_u64()).unwrap_or(0));
            if let Some(reason) = v.get("health").and_then(|x| x.as_str()) {
                if !reason.is_empty() {
                    e.health.reason = reason.to_string();
                }
            }
        }
        Some(false) => {
            let reason = v
                .get("health")
                .and_then(|x| x.as_str())
                .unwrap_or("caído")
                .to_string();
            e.health = Health::down(&reason, "");
        }
        None => {
            e.health = Health::unknown();
        }
    }
    Some(e)
}

// ─────────────────────────────────────────────────────────────────────────────
// Utilidades
// ─────────────────────────────────────────────────────────────────────────────

/// Epoch de hoy (día UTC) para el presupuesto.
pub fn today_epoch() -> u64 {
    now_epoch() / 86_400
}

/// Segundos desde epoch.
pub fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Hora actual en ISO-8601 UTC (algoritmo de Hinnant: días → fecha civil).
pub fn now_iso() -> String {
    let secs = now_epoch() as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn model(id: &str, tier: &str, caps: &[&str], free: bool) -> ServiceEntry {
        let mut e = ServiceEntry::new(id, ServiceKind::Model);
        e.tier = tier.to_string();
        e.capabilities = caps.iter().map(|c| c.to_string()).collect();
        e.cost_estimate = if free {
            CostEstimate::free()
        } else {
            CostEstimate::per_token(0.001, 0.002)
        };
        e
    }

    fn verified(e: &mut ServiceEntry, up: bool) {
        e.health = if up {
            Health::ok(10)
        } else {
            Health::down("caído", "prueba")
        };
    }

    #[test]
    fn el_catalogo_tiene_los_campos_del_plan_maestro() {
        let mut e = model("minimax", "paid", &["inference"], false);
        e.endpoint = "https://api.minimax.io/v1/chat/completions".into();
        e.auth_kind = "bearer".into();
        e.auth_ref = "/state/providers.json".into();
        let v = serde_json::to_value(&e).unwrap();
        for f in [
            "id",
            "kind",
            "endpoint",
            "auth_kind",
            "health",
            "latency_ms",
            "cost_estimate",
            "capabilities",
            "enabled",
        ] {
            assert!(v.get(f).is_some(), "falta el campo {}", f);
        }
    }

    #[test]
    fn el_snapshot_del_mesh_trae_el_last_seen_en_iso() {
        // El tablón guarda ISO-8601: si no se parsea, todos los peers salen
        // "sin contacto" aunque estén vivos.
        let ahora = now_iso();
        let snap = json!({
            "peers": [
                {"id":"vivo","hostname":"cloud","capabilities":["gpu"],"last_seen": ahora},
                {"id":"muerto","hostname":"x","capabilities":[],"last_seen": "2020-01-01T00:00:00Z"}
            ]
        });
        let cat = catalog_from_mesh_snapshot(&snap, 300);
        let vivo = cat.iter().find(|c| c.id == "vivo").unwrap();
        assert!(vivo.health.available, "un peer recién visto está vivo");
        let muerto = cat.iter().find(|c| c.id == "muerto").unwrap();
        assert!(!muerto.health.available);
        assert!(muerto.health.reason.contains("contacto"));
    }

    #[test]
    fn el_parseo_de_iso_es_consistente() {
        let ahora = now_iso();
        let e = parse_iso_epoch(&ahora).expect("la fecha de ahora se parsea");
        let ahora_epoch = now_epoch() as i64;
        assert!((e - ahora_epoch).abs() <= 2, "epoch: {} vs {}", e, ahora_epoch);
        assert_eq!(parse_iso_epoch("no es una fecha"), None);
    }

    #[test]
    fn la_salud_nunca_se_inventa() {
        let e = ServiceEntry::new("x", ServiceKind::Model);
        assert!(!e.health.available);
        assert_eq!(e.health.reason, "sin comprobar");
        assert!(e.health.checked_at.is_none());
        // Un peer sin heartbeat tampoco es "disponible".
        let snap = json!({"peers":[{"id":"p1","last_seen":0,"capabilities":["reverse"]}]});
        let cat = catalog_from_mesh_snapshot(&snap, 300);
        assert_eq!(cat.len(), 1);
        assert!(!cat[0].health.available);
        assert!(cat[0].health.reason.contains("contacto"));
    }

    #[test]
    fn la_seleccion_prefiere_el_gratis_y_luego_el_mas_barato() {
        let mut cheap = model("barato", "paid", &["inference"], false);
        cheap.cost_estimate = CostEstimate::per_token(0.0001, 0.0002);
        let mut dear = model("caro", "paid", &["inference"], false);
        dear.cost_estimate = CostEstimate::per_token(0.01, 0.02);
        let mut free = model("gratis", "free", &["inference"], true);
        free.health = Health::unknown();
        let bus = ServiceBus::new(vec![dear.clone(), cheap.clone(), free.clone()]);
        let sel = bus.select(TaskType::Inference);
        assert_eq!(sel.chosen.as_deref(), Some("gratis"));
        assert_eq!(sel.chain.len(), 3);
        // Con el gratis caído (comprobado) entra el barato.
        let mut bus2 = ServiceBus::new(vec![dear, cheap, free]);
        bus2.set_health("gratis", Health::down("caído", "prueba")).unwrap();
        let sel2 = bus2.select(TaskType::Inference);
        assert_eq!(sel2.chosen.as_deref(), Some("barato"));
    }

    #[test]
    fn el_fallback_pasa_al_siguiente_cuando_el_primero_falla() {
        let mut a = model("a", "paid", &["inference"], false);
        a.cost_estimate = CostEstimate::per_token(0.001, 0.002);
        let mut b = model("b", "paid", &["inference"], false);
        b.cost_estimate = CostEstimate::per_token(0.002, 0.004);
        let mut bus = ServiceBus::new(vec![a, b]);
        let mut calls: Vec<String> = Vec::new();
        let out = bus.route(TaskType::Inference, &mut |e| {
            calls.push(e.id.clone());
            if e.id == "a" {
                Err("500".into())
            } else {
                Ok("respuesta".into())
            }
        });
        assert!(out.ok);
        assert_eq!(out.used.as_deref(), Some("b"));
        assert_eq!(calls, vec!["a", "b"]);
        assert_eq!(out.attempts.len(), 1);
        assert_eq!(out.attempts[0].0, "a");
        // El que falló queda marcado como caído (evidencia real).
        assert!(!bus.entry("a").unwrap().health.available);
    }

    #[test]
    fn el_presupuesto_corta_la_cadena() {
        let mut e = model("caro", "paid", &["inference"], false);
        e.cost_estimate = CostEstimate::per_token(1.0, 2.0);
        let mut bus = ServiceBus::new(vec![e]);
        bus.set_budget(Budget::new(0.001, 10.0));
        let sel = bus.select(TaskType::Inference);
        assert!(sel.chosen.is_none());
        assert!(sel.rejected[0].1.contains("presupuesto"));
        // Con presupuesto suficiente, entra.
        bus.set_budget(Budget::new(10.0, 10.0));
        assert!(bus.select(TaskType::Inference).chosen.is_some());
    }

    #[test]
    fn el_presupuesto_se_gasta_y_se_rota() {
        let mut b = Budget::new(1.0, 2.0);
        assert!(b.allows(0.5));
        b.spend(0.5);
        assert!((b.remaining_tick() - 0.5).abs() < 1e-9);
        b.reset_tick();
        assert!((b.remaining_tick() - 1.0).abs() < 1e-9);
        assert!((b.remaining_day() - 1.5).abs() < 1e-9);
        assert!(!b.allows(2.0));
    }

    #[test]
    fn la_voz_prefiere_el_cloud_y_cae_al_local() {
        let mut cloud = model("minimax", "paid", &["inference"], false);
        cloud.endpoint = "https://api.minimax.io/v1/chat/completions".into();
        verified(&mut cloud, true);
        let mut local = model("local-seere", "local", &["inference"], true);
        local.endpoint = "http://127.0.0.1:8080/v1/chat/completions".into();
        verified(&mut local, true);
        let bus = ServiceBus::new(vec![local.clone(), cloud.clone()]);
        let plan = bus.voice_plan();
        assert_eq!(plan.chosen.as_deref(), Some("minimax"));
        // Sin cloud vivo: el local.
        let mut bus2 = ServiceBus::new(vec![local, cloud]);
        bus2.set_health("minimax", Health::down("caído", "prueba")).unwrap();
        assert_eq!(bus2.voice_model().as_deref(), Some("local-seere"));
    }

    #[test]
    fn el_catalogo_no_lleva_secretos() {
        let mut e = model("minimax", "paid", &["inference"], false);
        e.auth_kind = "bearer".into();
        e.auth_ref = "/state/providers.json".into();
        e.notes = "credencial en el fichero".into();
        let bus = ServiceBus::new(vec![e]);
        let j = bus.catalog_json().to_string();
        for bad in ["sk-", "ghp_", "Bearer ", "api_key\":", "mdaemon_sk_"] {
            assert!(!j.contains(bad), "el catálogo no debe contener {:?}", bad);
        }
        // Pero sí dice cómo y dónde.
        assert!(j.contains("\"auth_kind\":\"bearer\""));
        assert!(j.contains("/state/providers.json"));
    }

    #[test]
    fn el_catalogo_json_refleja_el_estado_real() {
        let mut a = model("a", "free", &["inference"], true);
        verified(&mut a, true);
        let b = model("b", "paid", &["inference"], false);
        let bus = ServiceBus::new(vec![a, b]);
        let j = bus.catalog_json();
        assert_eq!(j["counts"]["total"], json!(2));
        assert_eq!(j["counts"]["available"], json!(1));
        assert_eq!(j["counts"]["free"], json!(1));
        assert_eq!(j["schema"], json!(SERVICE_BUS_SCHEMA));
        assert!(j["selection"]["inference"]["chosen"].is_string());
        assert_eq!(j["entries"][0]["health"]["available"], json!(true));
        assert_eq!(j["entries"][1]["health"]["available"], json!(false));
        assert_eq!(j["entries"][1]["health"]["reason"], json!("sin comprobar"));
    }

    #[test]
    fn el_tipo_de_tarea_filtra_por_capacidades() {
        let mut code = model("code", "free", &["code"], true);
        verified(&mut code, true);
        let mut wri = model("writer", "free", &["writing"], true);
        verified(&mut wri, true);
        let bus = ServiceBus::new(vec![code, wri]);
        assert_eq!(bus.select(TaskType::Build).chosen.as_deref(), Some("code"));
        assert_eq!(bus.select(TaskType::Writing).chosen.as_deref(), Some("writer"));
        assert!(bus.select(TaskType::Analysis).chosen.is_none());
    }

    #[test]
    fn los_tipos_de_tarea_se_parsean_en_espanol() {
        assert_eq!(TaskType::parse("inferencia"), Some(TaskType::Inference));
        assert_eq!(TaskType::parse("análisis"), Some(TaskType::Analysis));
        assert_eq!(TaskType::parse("redacción"), Some(TaskType::Writing));
        assert_eq!(TaskType::parse("BUILD"), Some(TaskType::Build));
        assert_eq!(TaskType::parse("cocina"), None);
    }

    #[test]
    fn el_catalogo_de_modelos_sale_del_listado_redactado() {
        let redacted = json!({
            "providers": [
                {"id":"minimax-m3.1-flash","name":"MiniMax","vendor":"MiniMax",
                 "endpoint":"https://api.minimax.io/v1/chat/completions","model":"MiniMax-M3.1-Flash",
                 "auth_type":"bearer","tier":"paid","enabled":true,"ready":true},
                {"id":"local-seere","name":"SEERE","vendor":"Local",
                 "endpoint":"http://127.0.0.1:8080/v1/chat/completions","model":"local",
                 "auth_type":"local","tier":"offline","enabled":true,"ready":true}
            ]
        });
        let cat = catalog_from_providers_json(&redacted, "/state/providers.json");
        assert_eq!(cat.len(), 2);
        assert_eq!(cat[0].kind, ServiceKind::Model);
        assert_eq!(cat[0].auth_kind, "bearer");
        assert_eq!(cat[0].auth_ref, "/state/providers.json");
        assert_eq!(cat[1].auth_kind, "local");
        // El local es gratis; el cloud con tarifa, de pago.
        assert!(cat[1].cost_estimate.free);
        assert!(!cat[0].cost_estimate.free);
        // La salud nace sin comprobar (la credencial no es salud).
        assert!(!cat[0].health.available);
        // Pero sí sabemos si la credencial está puesta.
        assert_eq!(cat[0].auth_ready, Some(true));
        assert_eq!(cat[1].auth_ready, Some(true));
    }

    #[test]
    fn la_sonda_tcp_no_inventa_salud() {
        let p = TcpProber { timeout_ms: 300 };
        let mut e = ServiceEntry::new("x", ServiceKind::Model);
        // Endpoint vacío → no comprobable.
        e.endpoint = String::new();
        assert!(!p.probe(&e).available);
        // Puerto cerrado en localhost → caído de verdad.
        e.endpoint = "http://127.0.0.1:1/x".into();
        let h = p.probe(&e);
        assert!(!h.available);
        assert!(h.checked_at.is_some());
        assert!(h.reason.contains("conexión") || h.reason.contains("endpoint"));
    }

    #[test]
    fn la_sonda_desconocida_no_afirma_nada() {
        let e = ServiceEntry::new("x", ServiceKind::Model);
        let h = UnknownProber.probe(&e);
        assert!(!h.available);
        assert_eq!(h.reason, "sin comprobar");
        assert!(h.checked_at.is_none());
    }

    #[test]
    fn probe_all_guarda_el_resultado_real() {
        let mut bus = ServiceBus::new(vec![model("a", "free", &["inference"], true)]);
        let res = bus.probe_all(&UnknownProber);
        assert_eq!(res.len(), 1);
        assert!(!bus.entry("a").unwrap().health.available);
    }

    #[test]
    fn el_adaptador_ingiere_la_entrada_de_antigravity() {
        // Misma forma que produce `antigravity_intel::service_bus_entry`.
        let v = json!({
            "id": "antigravity",
            "kind": "agent_client",
            "endpoint": "http://127.0.0.1:9090/mcp",
            "auth_kind": "session_token_file",
            "health": "stdio roto, http vivo",
            "healthy": true,
            "tools_count": 7,
            "capabilities": ["mcp_client", "task_daemon"],
            "cost_estimate": "included",
            "latency_ms": null,
            "enabled": true
        });
        let e = entry_from_json(&v).expect("entrada válida");
        assert_eq!(e.id, "antigravity");
        assert_eq!(e.kind, ServiceKind::AgentClient);
        assert_eq!(e.auth_kind, "session_token_file");
        assert!(e.health.available, "la salud medida se respeta");
        assert_eq!(e.health.reason, "stdio roto, http vivo");
        assert!(e.cost_estimate.free, "incluido en el plan = coste 0");
        assert!(e.capabilities.contains(&"mcp_client".to_string()));
    }

    #[test]
    fn el_adaptador_no_inventa_salud() {
        // Sin "healthy": nace sin comprobar.
        let e = entry_from_json(&json!({"id":"x","kind":"agent"})).unwrap();
        assert!(!e.health.available);
        assert_eq!(e.health.reason, "sin comprobar");
        assert!(e.health.checked_at.is_none());
        // Con healthy=false: caído, con su motivo.
        let e2 = entry_from_json(&json!({"id":"x","kind":"agent","healthy":false,"health":"sin contacto"}))
            .unwrap();
        assert!(!e2.health.available);
        assert_eq!(e2.health.reason, "sin contacto");
    }

    #[test]
    fn el_puente_mcp_para_antigravity_no_lleva_la_clave() {
        let cfg = antigravity_bridge_config(
            "http://127.0.0.1:9090/mcp",
            "md-secrets/instance.env",
            &["minimax_chat", "swarm_chat"],
        );
        let j = cfg.to_string();
        assert!(j.contains("127.0.0.1:9090/mcp"));
        // El nombre del servidor NO se cambia: las allow-lists de Antigravity
        // lo referencian (`mcp(megadaemon-minimax/minimax_chat)`).
        assert!(j.contains(ANTIGRAVITY_SERVER_NAME));
        // El /mcp exige LAS DOS cabeceras.
        assert!(j.contains("X-API-Key"));
        assert!(j.contains("X-Seere-Auth-Token"));
        assert!(j.contains("minimax_chat"));
        // La clave NO está: sólo la referencia.
        assert!(!j.contains("mdaemon_sk_"));
        // Y al inyectarla, se pone (en el momento de escribir la config).
        let con = bridge_with_key(cfg.clone(), "mdaemon_sk_REAL");
        assert!(con.to_string().contains("mdaemon_sk_REAL"));
        assert!(!j.contains("mdaemon_sk_REAL"));
        let con2 = bridge_with_token(con, "tok_sesion_64chars");
        assert!(con2.to_string().contains("tok_sesion_64chars"));
    }

    #[test]
    fn el_merge_conserva_las_claves_raiz_del_fichero() {
        // La config real de Antigravity trae `mcpServers` Y `providers`
        // (el BYOK minimax).  Si se sobrescribe el fichero, se pierde el
        // modelo: el merge sólo toca nuestro servidor.
        let existing = r#"{
            "mcpServers": {
                "otro": {"url": "http://x/mcp"}
            },
            "providers": {"minimax": {"api_key": "sk-cp-secreto"}}
        }"#;
        let bridge = antigravity_bridge_config(
            "http://127.0.0.1:9090/mcp",
            "md-secrets/instance.env",
            &[],
        );
        let merged = merge_bridge_config(existing, &bridge).unwrap();
        let v: Value = serde_json::from_str(&merged).unwrap();
        // `providers` intacto.
        assert_eq!(v["providers"]["minimax"]["api_key"], json!("sk-cp-secreto"));
        // Nuestro servidor presente.
        assert!(v["mcpServers"][ANTIGRAVITY_SERVER_NAME].is_object());
        // Y el otro servidor sigue ahí.
        assert_eq!(v["mcpServers"]["otro"]["url"], json!("http://x/mcp"));
    }

    #[test]
    fn el_merge_es_idempotente() {
        let bridge = antigravity_bridge_config(
            "http://127.0.0.1:9090/mcp",
            "md-secrets/instance.env",
            &[],
        );
        let once = merge_bridge_config("{}", &bridge).unwrap();
        let twice = merge_bridge_config(&once, &bridge).unwrap();
        let a: Value = serde_json::from_str(&once).unwrap();
        let b: Value = serde_json::from_str(&twice).unwrap();
        assert_eq!(a, b, "aplicar dos veces debe dar lo mismo");
    }

    #[test]
    fn el_merge_no_pisa_las_cabeceras_reales() {
        // Caso real: el móvil ya tiene el puente aplicado con las cabeceras
        // buenas.  Re-aplicarlo NO debe sustituirlas por placeholders.
        let existing = r#"{
            "mcpServers": {
                "megadaemon-minimax": {
                    "url": "http://127.0.0.1:9090/mcp",
                    "headers": {"X-API-Key": "REAL_KEY", "X-Seere-Auth-Token": "REAL_TOKEN"}
                }
            },
            "providers": {"minimax": {"api_key": "sk-cp-secreto"}}
        }"#;
        let bridge = antigravity_bridge_config(
            "http://127.0.0.1:9090/mcp",
            "md-secrets/instance.env",
            &[],
        );
        let merged = merge_bridge_config(existing, &bridge).unwrap();
        let v: Value = serde_json::from_str(&merged).unwrap();
        let h = &v["mcpServers"]["megadaemon-minimax"]["headers"];
        assert_eq!(h["X-API-Key"], json!("REAL_KEY"));
        assert_eq!(h["X-Seere-Auth-Token"], json!("REAL_TOKEN"));
        assert!(!merged.contains("${"), "no deben quedar placeholders");
        assert_eq!(
            v["providers"]["minimax"]["api_key"],
            json!("sk-cp-secreto"),
            "el BYOK debe seguir intacto"
        );
    }

    #[test]
    fn la_allowlist_de_tools_se_publica_en_el_catalogo() {
        let mut e = seed_antigravity_client("http://127.0.0.1:9090/mcp");
        e.tool_allowlist = vec!["minimax_chat".into(), "swarm_chat".into()];
        let bus = ServiceBus::new(vec![e]);
        let j = bus.catalog_json();
        assert_eq!(j["entries"][0]["kind"], json!("agent_client"));
        assert_eq!(j["entries"][0]["tool_allowlist"].as_array().unwrap().len(), 2);
    }
}
