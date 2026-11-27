# RE — Tres patrones de la CLI Rust de Devin, concretados

> MegaDaemon · subagente de RE de productos · 2026‑11‑27
>
> **Fuente**: `Devin/resources/app/extensions/windsurf/devin/bin/devin` (262.445.096 B).
> Todo lo marcado **[M]** está **medido** (cadenas del binario, rutas de compilación,
> esquema SQL). Lo marcado **[P]** es **propuesta** de diseño: el binario no expone
> las firmas Rust exactas, así que el esqueleto es un *sketch* construido sobre los
> **nombres de módulo y de tipo medidos**, no una transcripción.
>
> **Nada de esto se ha copiado**: son tipos y firmas **propuestos** por nosotros a
> partir de la arquitectura observada.

---

# (a) El bucle como **manejador de efectos**

## a.1 Lo que hay medido [M]

Crate `affogato` (21 ficheros). El árbol, literal del binario:

```
affogato/src/agent/control_loop.rs              ← el bucle
affogato/src/agent/runner.rs                    ← ejecuta el bucle
affogato/src/agent/manager.rs                   ← ciclo de vida de agentes
affogato/src/agent/programmatic.rs              ← agente sin UI
affogato/src/agent/subagent.rs                  ← subagentes
affogato/src/agent/cog.rs                       ← "cog": el engranaje
affogato/src/agent/event_types.rs
affogato/src/agent/effects/mod.rs
affogato/src/agent/effects/handler.rs           ← despachador
affogato/src/agent/effects/inference.rs         ← efecto: llamar al modelo
affogato/src/agent/effects/tasks.rs             ← efecto: lanzar trabajo
affogato/src/agent/effects/tools.rs             ← efecto: ejecutar herramienta
affogato/src/agent/tool_pipeline/scheduler.rs   ← planificador de tool calls
affogato/src/stall_watch.rs                     ← detección de atasco
affogato/src/client.rs
affogato/src/image.rs
affogato/src/cache_keepalive*.rs                ← (ver §b)
```

**Los símbolos Rust que el compilador dejó en `.rodata`**: `affogato::agent::control_loop`,
`affogato::agent::effects::handler`, `affogato::agent::effects::tasks`,
`affogato::agent::effects::tools`, `affogato::agent::manager`,
`affogato::agent::programmatic`, `affogato::cache_keepalive`, `affogato::client`,
`affogato::cog`, `affogato::event_types`, `affogato::image`, `affogato::stall_watch`,
`affogato::subagent`.

**Los tipos del estado** [M]:

| Tipo | Campos | Medido en |
|---|---|---|
| `Cog` | **14** | `set_system_prefix`, `append_system_messages`, `footer_messages`, `user_display`, `permissions`, `tool_availability`, `permission_mask`, `strict_permissions`, `soft_deny` (+5 no medidos) |
| `DynamicChatMessage` | **5** | — |
| `PermissionMask` | **1** | — |
| `MessageNodeMetadata` | **4** | — |
| `EffectMarker` | — | existe en el **esquema ACP oficial** (`agent-client-protocol-schema`) ⇒ **no es de Cognition**: es del estándar |

`EffectMarker` en el esquema canónico [M] ⇒ Devin marca sus efectos con el tipo
del estándar ⇒ el bucle es **compatible con ACP** por construcción.

## a.2 La firma [P]

```rust
// ─── estado ────────────────────────────────────────────────────────────
pub struct AgentState {
    pub cog:        Cog,                     // 14 campos: prompt, permisos, tools
    pub messages:   Vec<DynamicChatMessage>, // 5 campos
    pub mask:       PermissionMask,          // 1 campo
    pub turn:       u32,
    pub pending:    Option<PendingEffect>,
}

// ─── efectos: lo que el bucle PIDE, no lo que hace ───────────────────
pub enum Effect {
    /// Pedir el siguiente turno al modelo.
    Inference(InferenceRequest),
    /// Ejecutar una tool call y devolver su resultado.
    Tool(ToolCallRequest),
    /// Lanzar trabajo asíncrono (tarea, subagente, proceso en background).
    Task(TaskRequest),
    /// Persistir / emitir.
    Emit(Event),
}

pub enum EffectOutcome {
    Inference(ModelTurn),
    Tool(ToolOutcome),
    Task(TaskOutcome),
    Emit(()),
}

// ─── el bucle: un REDUCTOR puro ───────────────────────────────────────
pub fn reduce(state: &AgentState, event: Event) -> (AgentState, Vec<Effect>);

// ─── los manejadores: uno por familia de efecto ───────────────────────
pub trait EffectHandler {
    type Request;
    type Outcome;
    async fn handle(&self, req: Self::Request, ctx: &Ctx) -> Result<Self::Outcome, Error>;
}

pub struct InferenceHandler<P: Provider> { ... }   // → providers.rs
pub struct ToolHandler { ... }                        // → tool_policy.rs
pub struct TaskHandler { ... }                       // → automation_runtime.rs
pub struct HandlerRegistry { ... }                   // → effects/handler.rs

// ─── el driver: despacha los efectos y devuelve eventos ───────────────
pub async fn run(state: AgentState, handlers: &HandlerRegistry) -> AgentState;
```

**El punto clave** [P, pero con la forma dictated por los nombres medidos]: `reduce`
es **puro** (estado + evento → estado + efectos) y **no** toca la red, el disco ni
el reloj. Todo el I/O vive en los manejadores. Eso es lo que permite que el mismo
bucle sirva para:

| Contexto | Medido en |
|---|---|
| el agente interactivo | `agent/runner.rs` |
| el agente programático (batch) | `agent/programmatic.rs` |
| un subagente | `agent/subagent.rs` |
| varios agentes a la vez | `agent/manager.rs` |

…y lo que hace que el test sea **determinista**: se prueba `reduce` sin red.

## a.3 Traducción a nuestros ficheros

| Nuestro fichero (tamaño medido) | Qué pasa | Tamaño est. |
|---|---|---|
| `rust-core/src/agent_core.rs` (25.337 B) | **pasa a ser `AgentState` + `Cog`**: el estado del agente y el constructor de contexto. Hoy mezcla estado y construcción de prompt. | +400‑600 L, −200 L |
| `rust-core/src/agent_loop.rs` (50.848 B) | **se reduce a `reduce()` + `run()`**: hoy hace la llamada al modelo y la tool inline. Pasa de "orquestador con I/O" a "reductor + despachador". | −1.500‑2.500 L netas, +800‑1.200 L de esqueleto |
| `rust-core/src/tool_policy.rs` (13.705 B) | **se convierte en `ToolHandler`**: aplica permisos y ejecuta la tool. | +200‑300 L |
| `rust-core/src/providers.rs` (85.325 B) | **se convierte en `InferenceHandler`**: una llamada al modelo con reintentos y límites. | +150‑250 L |
| `rust-core/src/automation_runtime.rs` (41.885 B) | **se convierte en `TaskHandler`**: trabajo asíncrono, cola, daemon. | +150‑250 L |
| **nuevo** `rust-core/src/agent/effects.rs` | `Effect`, `EffectOutcome`, `EffectHandler`, `HandlerRegistry` | 250‑400 L |
| **nuevo** `rust-core/src/agent/state.rs` | `AgentState`, `Cog`, `DynamicChatMessage`, `PermissionMask` | 300‑450 L |

**Qué habría que tocar, en orden**:
1. Extraer el estado actual de `agent_loop.rs` a `AgentState` (sin cambiar comportamiento).
2. Envolver la llamada al modelo actual como `InferenceHandler`.
3. Envolver la ejecución de tools actual como `ToolHandler`.
4. Convertir el bucle en `reduce()` + `run()`.
5. **Sólo entonces** el `programmatic.rs` y el subagente salen gratis.

## a.4 Tabla de equivalencia

| Devin (affogato) | Nuestro | Nota |
|---|---|---|
| `agent/control_loop.rs` | `agent_loop.rs` | el reductor |
| `agent/effects/handler.rs` | `agent/effects.rs` | registro de manejadores |
| `agent/effects/inference.rs` | `providers.rs` | |
| `agent/effects/tools.rs` | `tool_policy.rs` | |
| `agent/effects/tasks.rs` | `automation_runtime.rs` | |
| `agent/tool_pipeline/scheduler.rs` | `tool_policy.rs` + `service_bus.rs` | planificador de tool calls |
| `agent/programmatic.rs` | `automation_runtime.rs` | agente batch |
| `agent/subagent.rs` | `agent_core.rs` | |
| `agent/manager.rs` | `board.rs` | ciclo de vida de agentes |
| `agent/cog.rs` | `agent_core.rs` | estado + prompt |
| `stall_watch.rs` | `automation_runtime.rs` | watchdog |
| `cog` (14 campos) | `AgentState` | |

## a.5 Coste

| Fase | Esfuerzo |
|---|---|
| Extraer `AgentState` sin cambiar nada | 1‑2 días |
| `Effect` + `EffectOutcome` + `HandlerRegistry` | 1 día |
| `InferenceHandler` / `ToolHandler` / `TaskHandler` | 2‑3 días |
| Convertir el bucle en reductor + tests deterministas | 3‑5 días |
| Gains (programmatic, subagente, paralelismo de tools) | 2‑3 días |
| **Total** | **9‑14 días** |

---

# (b) `cache_keepalive` — la política que es dinero

## b.1 Lo que hay medido [M]

**Los 5 ficheros** (nombres exactos del binario):

```
affogato/src/cache_keepalive.rs            ← el driver
affogato/src/cache_keepalive_bucket.rs     ← agrupación de entradas
affogato/src/cache_keepalive_head.rs       ← cabeza de la cadena de prefijos
affogato/src/cache_keepalive_inputs.rs     ← qué entradas forman el prefijo cacheable
affogato/src/cache_keepalive_policy.rs     ← la política (decide y cuándo)
```

⇒ **5 ficheros = 5 responsabilidades**: *driver*, *bucket*, *head*, *inputs*,
*policy*. La separación de `inputs` (qué es cacheable) y `policy` (cuándo
pinguear) es justamente la que hace testeable la política sin coste de red.

**Los eventos de log** [M], textuales del binario:

| Evento | Mensaje | Significado |
|---|---|---|
| `no_action_request` | *"[CACHE_KEEPALIVE] No action request on PostActionGeneration; skipping"* | la generación terminó pero **no** había ninguna acción que pedir ⇒ no hay nada que mantener vivo |
| `action_answered` | *"[CACHE_KEEPALIVE] Starting keepalive pinging"* | la acción se respondió ⇒ **empieza el ping** de keepalive |
| — | *"[CACHE_KEEPALIVE] No serving route on the action response; skipping"* | la respuesta no trae ruta de servicio ⇒ se salta |
| `model_never_pinged` | *"model never pinged"* | ese modelo nunca se ha pingueado ⇒ no hay caché que mantener |
| `compaction` | — | hubo compactación ⇒ el prefijo cambió, la caché anterior ya no sirve |
| `agent_stopping` | — | el agente se para ⇒ no tiene sentido pinguear |
| `lead_idle` | *"Lead idle but subagent mid-run; leaving keepalive alone"* | el modelo principal está ocioso pero **hay un subagente corriendo** ⇒ **no** pinguear (el subagente va a pedir y ya lo refresca) |

**El nombre del evento que dispara el ciclo** [M]: `PostActionGeneration` ⇒ el
keepalive cuelga del **final de una generación**, no del reloj.

**Flags de configuración** [M] (en `OpenAiInferenceConfig`):

| Flag | Efecto |
|---|---|
| `extended_prompt_cache_retention` | retención extendida del prompt cache |
| `disable_prompt_cache_writes` | **no** escribir cachés nuevas |
| `disable_explicit_prompt_cache_breakpoints` | **no** poner breakpoints explícitos |
| `system_prompt_as_input_items` | el system prompt va como items, no como system ⇒ **cambia el prefijo** |
| `service_tier` | nivel de servicio (afecta a la retención) |
| `reasoning_context` | |

**Métricas** [M] — `PerformanceMetrics` con **12** campos:
`ttft_ms`, `total_time_ms`, `input_tokens`, `output_tokens`, `cache_read_tokens`,
`cache_creation_tokens`, `cache_creation_1h_tokens`, `cache_breakpoints`,
`context_input_tokens`, `tpot_ms`, `tokens_per_sec`.

`cache_read_tokens` + `cache_creation_tokens` + `cache_creation_1h_tokens` ⇒
**el coste del caché es una métrica de primera clase**. Y `CacheBreakpoint` con
**6** campos ⇒ hay **breakpoints explícitos** en el prompt (no sólo prefijo).

**Otros**: `cognition.ai/ephemeral/subscribe`, `cognition.ai/ephemeralState` ⇒ hay
un **canal efímero** para el estado de la sesión; `CompactionThresholds`
(`spawn_tokens`, `hard_tokens`, `apply_t…`) ⇒ el umbral de compactación es **por
modelo**.

## b.2 La política, reconstruida [P sobre [M]]

> Tras cada generación (`PostActionGeneration`), si el agente **va a quedarse
> quieto**, se manda una **petición mínima** que reenvía el prefijo cacheable
> para que el TTL del prompt cache del proveedor no expire. Si el prefijo cambió
> (compactación) o el agente se para, no se hace nada. Si hay un subagente
> corriendo, el subagente ya refresca: no se gasta un ping.

El **coste**: los prompt caches de los proveedores tienen TTL corto (5 min en
Anthropic). Sin esto, un turno de usuario tras una pausa **paga el input
entero**. Con esto, paga la lectura de caché.

## b.3 La API [P]

```rust
// ─── qué es cacheable ────────────────────────────────────────────────
pub struct KeepaliveInputs {
    /// Prefijo estable: system prompt + rules + skills (en ese orden, sin reordenar).
    pub prefix:      Vec<DynamicChatMessage>,
    /// Breakpoints explícitos dentro del prompt.
    pub breakpoints: Vec<CacheBreakpoint>,     // 6 campos, medidos
    /// Tokens del prefijo, para decidir si merece la pena.
    pub prefix_tokens: u32,
}

// ─── cabeza: qué entrada es la "actual" ─────────────────────────────
pub struct KeepaliveHead {
    pub model_id:    String,
    pub bucket_id:   String,
    pub last_pinged: Option<Instant>,
    pub ever_pinged: bool,                     // evento `model_never_pinged`
    pub generation:  u64,
}

// ─── política: decide ────────────────────────────────────────────────
pub struct KeepalivePolicy {
    pub ttl:            Duration,               // TTL conocido del proveedor
    pub ping_before_expiry: Duration,          // margen de seguridad
    pub min_prefix_tokens: u32,                // por debajo, no compensa
    pub disabled:        bool,                  // disable_prompt_cache_writes
    pub explicit_breakpoints: bool,            // disable_explicit_prompt_cache_breakpoints
    pub extended_retention: bool,              // extended_prompt_cache_retention
}

pub enum KeepaliveDecision {
    Ping { head: KeepaliveHead, inputs: KeepaliveInputs },
    Skip(SkipReason),
}

pub enum SkipReason {
    NoActionRequest,        // `no_action_request`
    ModelNeverPinged,       // `model_never_pinged`
    NoServingRoute,         // "No serving route on the action response"
    Compaction,             // `compaction` — el prefijo cambió
    AgentStopping,          // `agent_stopping`
    LeadIdleSubagentBusy,   // `lead_idle` — el subagente ya refresca
    PrefixTooSmall,        // min_prefix_tokens
    Disabled,               // disable_prompt_cache_writes
}

impl KeepalivePolicy {
    pub fn decide(&self, ev: &KeepaliveEvent, head: &KeepaliveHead,
                  inputs: &KeepaliveInputs) -> KeepaliveDecision { /* … */ }
}

// ─── driver ──────────────────────────────────────────────────────────
pub struct Keepalive { policy: KeepalivePolicy, heads: HashMap<String, KeepaliveHead> }

impl Keepalive {
    /// Se llama al final de cada generación.
    pub fn on_post_action_generation(&mut self, ev: &KeepaliveEvent) -> Option<Ping>;
    /// El ping: una petición mínima que reenvía el prefijo.
    pub fn ping(&self, head: &KeepaliveHead, inputs: &KeepaliveInputs) -> PingRequest;
}

// ─── el evento, con las ramas medidas ────────────────────────────────
pub enum KeepaliveEvent {
    PostActionGeneration { action_answered: bool, serving_route: Option<String> },
    Compaction,
    AgentStopping,
    SubagentStarted { agent_id: String },
    SubagentFinished { agent_id: String },
}
```

## b.4 Tabla de equivalencia y coste

| Devin | Nuestro | Coste |
|---|---|---|
| `cache_keepalive_inputs.rs` | nuevo `providers.rs::keepalive::KeepaliveInputs` | 0,5‑1 día |
| `cache_keepalive_policy.rs` | nuevo `providers.rs::keepalive::KeepalivePolicy` (puro, testeable) | 1‑2 días |
| `cache_keepalive_head.rs` | `KeepaliveHead` en `providers.rs` | 0,5 día |
| `cache_keepalive_bucket.rs` | agrupación por bucket (proveedor/modelo) | 0,5‑1 día |
| `cache_keepalive.rs` (driver) | `Keepalive` en `providers.rs` | 1‑2 días |
| `PerformanceMetrics` (12 campos) | `providers.rs` — métricas de proveedor | 0,5‑1 día |
| `CompactionThresholds` por modelo | `providers.rs` | 0,5 día |
| **Total** | | **4‑8 días** |

**Retorno**: si nuestro agente tiene pausas largas entre turnos (tarea en
background, usuario pensando, subagente trabajando), esto es **directamente
dinero ahorrado**. Es el patrón con mejor relación coste/beneficio de los diez.

---

# (c) La sesión como **bosque**, y el revert como **fork**

## c.1 Lo que hay medido [M]

### Tablas (SQLite, texto literal del binario)

```sql
CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY,
  working_directory TEXT NOT NULL,
  backend_type TEXT NOT NULL,
  model TEXT NOT NULL,
  permission_mode TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  last_activity_at INTEGER NOT NULL,
  total_tokens INTEGER NOT NULL,
  current_epoch INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS message_nodes (          -- ← EL BOSQUE
  row_id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  node_id INTEGER NOT NULL,           -- node_id within this session's forest
  parent_node_id INTEGER,             -- NULL for root nodes
  chat_message TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  FOREIGN KEY (session_id) REFERENCES sessions(id),
  UNIQUE(session_id, node_id)
);

CREATE TABLE IF NOT EXISTS messages (                -- la lista lineal (vista)
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  sequence_number INTEGER NOT NULL,
  role TEXT NOT NULL,
  content TEXT NOT NULL,
  tool_call_id TEXT,
  tool_calls_json TEXT,
  timestamp INTEGER NOT NULL,
  compaction_epoch INTEGER NOT NULL DEFAULT 0,
  UNIQUE(session_id, compaction_epoch, sequence_number)
);

CREATE TABLE IF NOT EXISTS subagent_heads (
  session_id TEXT NOT NULL,
  agent_id TEXT NOT NULL,
  chain_node_id INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (session_id, agent_id)
);

CREATE TABLE IF NOT EXISTS tool_call_state (
  session_id TEXT NOT NULL,
  tool_call_id TEXT NOT NULL,
  tool_call_json TEXT,                -- ToolCall inicial
  tool_call_update_json TEXT,         -- actualización final
  PRIMARY KEY (session_id, tool_call_id)
);

CREATE TABLE IF NOT EXISTS rendered_commits (        -- HTML renderizado, versionado
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  sequence_number INTEGER NOT NULL,
  rendered_html TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE(session_id, sequence_number)
);

CREATE TABLE IF NOT EXISTS live_row_times (session_id TEXT, row_key TEXT, started_at TEXT);
CREATE TABLE IF NOT EXISTS session_window_tools (session_id TEXT PRIMARY KEY, window_tools_json TEXT);
CREATE TABLE IF NOT EXISTS prompt_history (id INTEGER PK AUTOINCREMENT, content TEXT NOT NULL,
                                          timestamp INTEGER NOT NULL, session_id TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS app_state (key TEXT PRIMARY KEY, value TEXT NOT NULL);

CREATE INDEX IF NOT EXISTS idx_message_nodes_session  ON message_nodes(session_id);
CREATE INDEX IF NOT EXISTS idx_messages_session_epoch ON messages(session_id, compaction_epoch);
CREATE INDEX IF NOT EXISTS idx_sessions_activity      ON sessions(last_activity_at);
CREATE INDEX IF NOT EXISTS idx_sessions_hidden        ON sessions(hidden);
```

### Módulos del revert [M]

`chisel-agent/src/revert/engine.rs`, `chisel-agent/src/revert/steps.rs`,
`chisel-agent/src/revert/target.rs`.

### La capacidad ACP [M]

`cognition.ai/revert: true` — opt‑in mutuo para los métodos de extensión
**`cognition.ai/revert/{listSteps,preview,execute,forkFromStep}`**.

### Otros tipos [M]

`MessageNodeMetadata` (**4** campos), `Cog` (**14**), `DynamicChatMessage` (**5**),
`PermissionMask` (**1**).

## c.2 Por qué dos tablas (`messages` y `message_nodes`)

[M] Es lo que dice el esquema:

- **`message_nodes`** = el **bosque**: la verdad de la conversación. `parent_node_id`
  permite ramificar.
- **`messages`** = la **vista lineal**: `sequence_number` + `compaction_epoch`.
  Es la proyección "el camino que se está siguiendo".

⇒ **No hay dos fuentes de verdad**: el bosque es la verdad; `messages` es la
proyección lineal del camino activo. `compaction_epoch` marca **dónde** se
compactó, sin borrar nada del bosque.

⇒ `subagent_heads` dice **en qué nodo del bosque** murió cada subagente: al
reanudar, se sabe exactamente desde dónde continuar.

⇒ `rendered_commits` guarda el **HTML renderizado** por número de secuencia: se
puede reconstruir **exactamente** lo que el usuario vio, incluso después de un
cambio de modelo o de una compactación.

## c.3 El algoritmo del revert [P sobre [M]]

Los 4 métodos ACP miden las 4 operaciones:

```
listSteps   → lista los pasos (nodos del bosque) que se pueden deshacer
preview     → dice qué cambiaría (qué ficheros, qué diff) SIN aplicarlo
execute     → aplica el paso: crea un HIJO del nodo actual
forkFromStep→ reapunta la cabeza a un nodo anterior y sigue desde ahí
```

Algoritmo:

```rust
/// 1. listSteps: los pasos son los nodos del bosque con etiqueta.
pub fn list_steps(&self, session: &SessionId) -> Vec<Step> {
    // 1) recorrer message_nodes de la sesión
    // 2) un paso = un nodo marcado (p.ej. los que cambiaron ficheros)
    // 3) devolverlos con su node_id, su parent, y la etiqueta legible
}

/// 2. preview: simular SIN escribir.
pub fn preview(&self, session: &SessionId, step: &StepId) -> Preview {
    // 1) nodo destino = step.node_id
    // 2) camino = ruta de la raíz hasta nodo destino  (parent_node_id)
    // 3) diff entre el estado del workspace en el nodo actual y el del destino
    // 4) NO escribir nada
}

/// 3. execute: el revert NO borra, CREA.
pub fn execute(&self, session: &SessionId, step: &StepId) -> NodeId {
    // 1) nuevo_nodo = insert(message_nodes { parent_node_id: nodo_actual, … })
    // 2) el nuevo nodo lleva el contenido del paso revertido
    // 3) actualizar la cabecera (messages) para que apunte al nuevo nodo
    // 4) el bosque conserva los dos caminos
}

/// 4. forkFromStep: seguir por otro camino.
pub fn fork_from_step(&self, session: &SessionId, step: &StepId) -> NodeId {
    // 1) crear un nodo hijo del paso elegido
    // 2) la cabecera pasa a ese hijo
    // 3) el camino anterior quedaHUÉFANO pero VIVO
}
```

**La invariante**: *revertir es añadir un nodo, nunca borrar*. Por eso el
historial es completo y por eso se puede deshacer un revert.

## c.4 Los estados que acompañan al bosque

| Estado | Dónde | Por qué |
|---|---|---|
| **Compactación** | `compaction_epoch` en `messages`, `current_epoch` en `sessions` | compactar **no borra**: marca una época. El historial viejo sigue consultable (y el revert puede apuntar a él) |
| **Subagentes** | `subagent_heads` (session_id, agent_id) → chain_node_id | dónde murió cada subagente ⇒ reanudable |
| **Tool calls** | `tool_call_state` (ToolCall inicial + ToolCallUpdate final) | se guarda lo pedido **y** lo resuelto; el DDL dice que el final puede faltar si la sesión se interrumpió ⇒ **reconstrucción** |
| **Render** | `rendered_commits` | el HTML que vio el usuario, versionado |
| **Latencia** | `live_row_times` | tiempo de vida de cada operación viva ⇒ métricas por operación |

## c.5 Tabla de equivalencia

| Devin | Nuestro | Nota |
|---|---|---|
| `message_nodes` | **nuevo** `rust-core/src/session_store.rs` (o `board.rs`) | el bosque |
| `messages` | `board.rs` / `service_bus.rs` | la vista lineal |
| `sessions` | `board.rs` | metadatos de sesión |
| `subagent_heads` | `board.rs` | reanudar subagentes |
| `tool_call_state` | `tool_policy.rs` / `agent_loop.rs` | tool calls pendientes |
| `rendered_commits` | `webui` / cockpit | lo que ve el usuario |
| `revert/{engine,steps,target}.rs` | **nuevo** `rust-core/src/session_store.rs::revert` | 4 operaciones |
| `cognition.ai/revert/*` | `protocol.rs` (extensión) | el puente ACP |
| `session_store{sqlite,files}` + `store_io.rs` | `session_store.rs` | 2 backends + migración |

## c.6 Coste

| Fase | Esfuerzo |
|---|---|
| Esquema SQLite con `message_nodes` (2 tablas + índices) | 1‑2 días |
| Migración `messages` → `message_nodes` (con `parent_node_id`) | 2‑3 días |
| `listSteps` / `preview` | 2‑3 días |
| `execute` / `forkFromStep` (con escritura del workspace) | 3‑5 días |
| `subagent_heads` + reanudación | 2 días |
| `compaction_epoch` (no borra) | 2‑3 días |
| Segundo backend (`files`) + migración reanudable e idempotente | 3‑4 días |
| Puente ACP `cognition.ai/revert/*` | 1 día |
| **Total** | **16‑23 días** |

**Es el bloque más caro de los tres**, pero es el que da **el modelo de historial
que nos falta**: hoy el historial de MegaDaemon es una lista, y eso hace
imposible "deshacer y seguir por otro camino".

---

## d. Resumen: coste total de los tres

| Patrón | Bloque | Esfuerzo | Retorno |
|---|---|---|---|
| (b) `cache_keepalive` | `providers.rs` | 4‑8 días | **dinero directo** |
| (a) bucle como reductor | `agent_loop.rs` + `agent_core.rs` + 3 más | 9‑14 días | testabilidad, paralelismo, subagentes gratis |
| (c) sesión como bosque | `session_store.rs` + `board.rs` | 16‑23 días | historial con revert/fork |

**Orden recomendado**: (b) → (a) → (c).
(b) es el más barato y el de mejor retorno inmediato; (a) es la refactorización
que desbloquea todo lo demás; (c) es la que justifica (a).

## e. Lo NO medido

| Elemento | Por qué |
|---|---|
| Las **firmas Rust exactas** de `affogato` | El binario no las conserva: sólo nombres de módulo y de tipo. Los sketches son propuestas. |
| Los **5 campos restantes** de `Cog` (de 14) | La cadena del binario los trunca. |
| Los **4 campos** de `MessageNodeMetadata` | Ídem. |
| Los **6 campos** de `CacheBreakpoint` | Ídem. |
| El **TTL** real que usa Devin | Depende del proveedor; el binario no lo fija. |
| Los **6 campos** de `DynamicChatMessage` (de 5 medidos, 5 declarados) | Ídem. |
| El **formato del bundle** de plugins y el **esquema** de `revert` en ACP (sólo los nombres de método) | Requiere sesión real. |
| Si el ping de keepalive es una petición **real** al modelo o una ruta de servicio aparte | El log dice *"No serving route on the action response"* ⇒ sugiere una **ruta de servicio** distinta, pero no está medida. |
