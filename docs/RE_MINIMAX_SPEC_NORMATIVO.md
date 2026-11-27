# Especificación normativa — COLA, DELEGACIÓN y GOAL (de MiniMax Code a MegaDaemon)

**Fuente:** `MiniMax-AI/minimax-code` (MIT, 2.297★, push 2026-11-27), leído del
**código fuente** (no del bundle). Todo lo de aquí está medido sobre:
`packages/tui/src/acp/extensions.ts`, `packages/local-runtime/src/thread-goal/`,
`packages/local-runtime/src/team/api.ts`, `packages/local-runtime/src/runtime/`.

**Para quién:** quien implemente esto en `rust-core/`. La regla del proyecto es
que la capacidad va **en Rust**, no en scripts.

---

## 0. El dialecto ACP de MiniMax, completo

**17 métodos** (`TUI_ACP_EXTENSION_METHODS`) y **4 notificaciones**
(`TUI_ACP_EXTENSION_NOTIFICATIONS`). Nota: el *launcher* (`@minimax-ai/code-acp`)
declara sólo **3** notificaciones; el agente real emite **4** (añade
`mcode/session/goal_update`). El contrato del agente es el que manda.

### 0.1 Los 17 métodos con su firma exacta

| # | método ACP | params | result |
|---|---|---|---|
| 1 | `session/activate` | `{sessionId}` | `{sessionId}` + notifica `current_session_update` |
| 2 | `mcode/session/activate` | `{sessionId}` | `{sessionId}` + notifica `current_session_update` |
| 3 | `mcode/session/steer` | `{sessionId, text, clientRequestId?}` | `{turnId, mode}` |
| 4 | `mcode/session/queue/list` | `{sessionId}` | `{items}` |
| 5 | `mcode/session/queue/enqueue` | `{sessionId, text}` | `enqueueMessage(...)` |
| 6 | `mcode/session/queue/update` | `{sessionId, itemId, text}` | `{item \| null}` |
| 7 | `mcode/session/queue/delete` | `{sessionId, itemId}` | `{item \| null}` |
| 8 | `mcode/session/queue/steer` | `{sessionId, itemId}` | `steerQueuedMessage(...)` |
| 9 | `mcode/session/goal/get` | `{sessionId}` | `{goal \| null}` |
| 10 | `mcode/session/goal/create` | `{sessionId, objective, tokenBudget?}` | `{goal}` |
| 11 | `mcode/session/goal/patch` | `{sessionId, status?, objective?, tokenBudget?}` | `{goal}` |
| 12 | `mcode/session/goal/clear` | `{sessionId}` | `{cleared: bool}` |
| 13 | `mcode/session/delegation/get` | `{sessionId}` | `{snapshot}` |
| 14 | `mcode/session/delegation/stop` | `{sessionId}` | `{receipt}` |
| 15-17 | `mcode/session/delegation/{message,transcript}` + `mcode/session/queue/steer` | ver §3 | ver §3 |

> Nota honesta: `extensions.ts` (el registro que el TUI anuncia) lista 17
> métodos, pero **sólo 14 están implementados ahí**; `delegation/message` y
> `delegation/transcript` aparecen en el launcher (`acp.js`) y **no** en el
> registro del TUI ⇒ su implementación está en otro sitio (runtime). Medido como
> *declarados pero no implementados en el TUI*.

### 0.2 Las 4 notificaciones

`mcode/session/current_session_update` · `mcode/session/queue_update` ·
`mcode/session/goal_update` · `mcode/session/delegation_update`

### 0.3 Tres reglas transversales medidas

1. **Capability del cliente**: las notificaciones sólo se emiten si el cliente
   anuncia `_meta["minimax-code/extensions"]` = `true` **o**
   `{version: 1, notifications: true}` (`TUI_ACP_EXTENSION_VERSION = 1`).
2. **Feature flag**: si el goal está deshabilitado (`runtime.isGoalEnabled()`),
   los 4 métodos de goal **no se anuncian**, `goal_update` **no se emite**, y
   llamar a cualquiera de ellos da
   `invalidParams("Goal extensions are disabled by the Runtime feature configuration.")`.
3. **Errores ACP** que usa: `acp.RequestError.resourceNotFound(sessionId)`
   (sesión inexistente), `acp.RequestError.invalidParams(...)` (parámetros),
   `acp.RequestError.internalError(...)` (ciclo en la cadena de padres).

---

## 1. COLA (`session/queue`)

### 1.1 Qué es

Una **cola de mensajes de prompt por sesión**: el cliente encola texto y el
agente lo va vaciando. No es una cola de trabajos.

### 1.2 Semántica medida

* `enqueue` mete un mensaje; devuelve el resultado del runtime (id del item).
* `list` devuelve `{items}` (los mensajes encolados de esa sesión).
* `update` **cambia el texto** de un item ⇒ devuelve `{item}` o `{item: null}`
  si ya no existe.
* `delete` borra un item ⇒ `{item}` (el borrado) o `{item: null}` (no existía).
* `queue/steer` **convierte un item encolado en el mensaje activo**: es el
  atajo de "en vez de esperar, dime esto ahora".
* `session/steer` (a nivel de sesión) requiere que haya un **turno ACP
  admitido y activo**; si no, `invalidParams("Steering requires an admitted,
  active ACP prompt Turn for this Session.")`.
  Además hay un **control de concurrencia** (`preDelivery.accept`): si el turno
  activo cambia entre la admisión y la entrega ⇒ `invalidParams("The active ACP
  prompt Turn changed before steering was admitted.")`; y si el `steer` intenta
  *activar* un turno nuevo en vez de-directionalarlo ⇒
  `invalidParams("Steering cannot activate a new Turn for an ACP prompt.")`.
  ⇒ **El patrón es: comparar-y-rechazar (compare-and-swap) sobre el turno
  activo.** Es exactamente lo que hay que hacer en Rust.

### 1.3 Mapeo a MegaDaemon (nombres nuestros)

| ACP | módulo Rust | tipo / función |
|---|---|---|
| `queue/list` | **nuevo** `rust-core/src/session_queue.rs` | `pub struct QueueItem { id, session_id, text, created_ms, state }` · `pub fn list(session_id: &str) -> Vec<QueueItem>` |
| `queue/enqueue` | ídem | `pub fn enqueue(session_id: &str, text: &str) -> QueueItem` |
| `queue/update` | ídem | `pub fn update(session_id: &str, item_id: &str, text: &str) -> Option<QueueItem>` |
| `queue/delete` | ídem | `pub fn delete(session_id: &str, item_id: &str) -> Option<QueueItem>` |
| `queue/steer` | ídem | `pub fn steer(session_id: &str, item_id: &str) -> Result<SteerOutcome, QueueError>` |
| `session/steer` | ídem + `agent_loop.rs` | `pub fn steer_turn(session_id: &str, text: &str, expected_turn: &str) -> Result<SteerOutcome, QueueError>` |

**Ojo con `job_queue.rs`**: ahí ya hay `JobStatus`/`JobRecord`/`JobQueue`, pero
es una cola de **trabajos** (kind + payload + prioridad), no de mensajes de
prompt. **No reutilizar ese tipo**: son dos cosas distintas con el mismo nombre
en inglés. La cola de prompt es un tipo nuevo.

### 1.4 Estado en disco

Propuesta (a confirmar por quien implemente): `state/queues/<session_id>.jsonl`
— un item por línea, con `id`, `text`, `created_ms`, `state`.
Justificación: `state_dir.rs` ya centraliza el estado; JSONL permite **apénd**
sin reescribir (que es lo que hace una cola) y `task_scheduler.rs` ya usa un
patrón de `persist()`/`restore()` equivalente.

### 1.5 Contrato de la CLI

```
megadaemon-core queue listar   <session_id>
megadaemon-core queue encolar  <session_id> --texto <txt>
megadaemon-core queue editar   <session_id> <item_id> --texto <txt>
megadaemon-core queue borrar   <session_id> <item_id>
megadaemon-core queueLolita    <session_id> <item_id>   # steer
```

### 1.6 Contrato del tool MCP

Una tool, con parámetros, en vez de 5:

```
mdaemon/queue          { "accion": "list|enqueue|update|delete|steer",
                          "session_id": "...", "item_id": "...", "texto": "..." }
```

Motivo: en MCP, una tool con un discriminante es más usable por un modelo que
cinco tools parecidas; y el resultado es el mismo objeto `{item|null}`.

---

## 2. DELEGACIÓN (`session/delegation`)

### 2.1 Qué es (y por qué NO es nuestro `delegate.rs`)

La delegación de MiniMax es **delegar una sesión a otra sesión** (un
subagente), con **transcripción** y posibilidad de **mandarle mensajes** y
**pararla**.

> ⚠ **Medido y MUY importante**: nuestro `rust-core/src/delegate.rs` (3.731
> líneas: `DelegationTask`, `Attempt`, `Adjustment`, `ProviderAccount`,
> `MAX_DAILY_BUDGET_USD`, `PRICING_TABLE`) delega **trabajo a cuentas de
> proveedor LLM** (rotación de cuentas, presupuesto, calidad). **No es lo
> mismo**. Son dos conceptos que comparten la palabra "delegar".
> El nuestro = *a quién se lo pido*; el de MiniMax = *a quién se lo paso*.

### 2.2 Semántica medida

* La **raíz** de una delegación se resuelve recorriendo `parentSessionId` hasta
  el final, con **detección de ciclos** (`visited`): si el grafo tiene un ciclo
  ⇒ `internalError("Session parent chain contains a cycle.")`.
* `delegation/get` devuelve un **snapshot** de la delegación **de la raíz** (no
  de la sesión consultada) ⇒ es una vista de árbol.
* `delegation/stop` devuelve un **recibo** (`receipt`) ⇒ parada auditable.
* La tabla de tareas de un "team plan" (`packages/local-runtime/src/team/api.ts`)
  es `LocalTeamPlanTask { id, title?, description?, prompt?, assignedTo?,
  modelConfigId?, dependsOn: string[], raw }` ⇒ **las tareas forman un DAG con
  dependencias** (`dependsOn`).
* El dispatch devuelve `{ sessionId, queueItemId?, agentName? }` y la
  cancelación `{ sessionId, queueCancelled, turnAborted }` ⇒ **cancelar una tarea
  puede cancelar un item de cola y abortar un turno**, y **son dos hechos
  distintos** (`queueCancelled` ≠ `turnAborted`).

### 2.3 Mapeo a MegaDaemon

| ACP | módulo Rust | tipo / función |
|---|---|---|
| `delegation/get` | `rust-core/src/delegation_tree.rs` (nuevo) | `pub struct DelegationSnapshot { root_session_id, children: Vec<DelegationNode> }` · `pub fn snapshot(session_id: &str) -> Result<DelegationSnapshot, DelegationError>` |
| `delegation/stop` | ídem | `pub fn stop(session_id: &str) -> Result<StopReceipt, DelegationError>` |
| `delegation/message` | ídem | `pub fn message(session_id: &str, text: &str) -> Result<(), DelegationError>` |
| `delegation/transcript` | ídem | `pub fn transcript(session_id: &str) -> Result<Vec<TranscriptEntry>, DelegationError>` |
| (equivalente de `team/plan`) | `rust-core/src/task_scheduler.rs` (existente) | `submit(kind, payload, priority, deps, res)` **ya tiene `deps`** ⇒ el DAG de tareas ya existe en nuestro núcleo |

**Conclusión medida**: el DAG de tareas (`dependsOn`) **ya lo tenemos** en
`task_scheduler.rs` con `deps: Vec<String>` y persistencia (`persist()`/
`restore()`). Lo que **no** tenemos es el **árbol de sesiones delegadas** y su
**transcripción**.

### 2.4 Estado en disco

`state/delegation/<root_session_id>.json` con el árbol (padre → hijos) y
`state/transcripts/<session_id>.jsonl` con la transcripción (una línea por
mensaje: rol, texto, timestamp, `turn_id`).

### 2.5 Contrato de la CLI

```
megadaemon-core delegacion ver      <session_id>
megadaemon-core delegacion parar    <session_id>
megadaemon-core delegacion mensaje  <session_id> --texto <txt>
megadaemon-core delegacion leer     <session_id>
```

### 2.6 Contrato del tool MCP

```
mdaemon/delegacion   { "accion": "get|stop|message|transcript", "session_id": "...", "texto": "..." }
```

---

## 3. GOAL (`session/goal`)

### 3.1 Qué es

Un **objetivo explícito por sesión**, con estado, presupuesto de tokens y
adjuntos. Es lo que MiniMax llama *thread goal*.

### 3.2 Semántica medida (`thread-goal/contract.ts` + `extensions.ts`)

* **Estados** (6 en el store interno, 5 expuestos por ACP):
  `active` · `paused` · `blocked` · `complete` · `budget_limited` ·
  `usage_limited`.
  El ACP expone `['active','paused','blocked','complete','budget_limited']`
  (sin `usage_limited`).
* **Transición validada**: **no se puede re-entrar a `active` desde `complete`
  o `budget_limited`** ⇒ HTTP 409 con código `GOAL_STATUS_CONFLICT` /
  `GOAL_BUDGET_LIMITED`. La validación de enum la hace la capa de contrato; el
  guard real, en el store.
* **`token_budget` es tri-estado en el PATCH** (trampa medida y deliberada):
  * campo **omitido** ⇒ dejar el valor como está;
  * `0` ⇒ **quitar el tope** (→ `null`);
  * entero **positivo** ⇒ poner el tope.
  El store aplica `positiveOrNull` (cualquier valor no positivo = "sin tope").
  El mismo truco que el cron `keep_sessions=0 = keep-all` ⇒ no hace falta un
  flag `clear_token_budget`.
  **Matiz medido**: en la **capa ACP** el valor debe ser `null` o un número
  **estrictamente positivo** (`optionalBudget`); el `0` se acepta más abajo, en
  el store.
* **PATCH vacío ⇒ error**: `invalidParams("Goal patch must change at least one
  field.")`.
* **`objective`**: texto no vacío (con `trim()`); la longitud **no** está
  limitada por defecto (hay un `objectiveMaxChars` opcional en la política).
* **Adjuntos**: `asset_data_url` / `asset_source` (file o remote) / nombre, con
  códigos de error `invalid_asset_data_url`, `asset_source_required`,
  `asset_source_not_file`, `asset_remote_url_invalid`,
  `asset_remote_unreadable`, y los de sistema (`ENOENT`, `EACCES`, `EPERM`,
  `ENOTDIR`, `EISDIR`).
* **`clear`** devuelve `{cleared: bool}` (no `null`): se distingue "no había
  goal" de "había y se borró".
* La capa de contrato **traduce el IDL plano** (`matrix/api_common/goal.thrift`,
  direccionado por `session_id`) al modelo interno (`@mavis/goal`), y **no** lleva
  acoplamiento HTTP (ni `Response` de Hono) ⇒ reutilizable por los tests.

### 3.3 Mapeo a MegaDaemon

| ACP | módulo Rust | tipo / función |
|---|---|---|
| `goal/get` | `rust-core/src/goal.rs` (nuevo) | `pub struct Goal { goal_id, session_id, objective, status: GoalStatus, token_budget: Option<u64>, attachments: Vec<Attachment> }` |
| | | `pub enum GoalStatus { Active, Paused, Blocked, Complete, BudgetLimited, UsageLimited }` |
| `goal/create` | ídem | `pub fn create(session_id: &str, objective: &str, token_budget: Option<u64>) -> Result<Goal, GoalError>` |
| `goal/patch` | ídem | `pub fn patch(session_id: &str, patch: GoalPatch) -> Result<Goal, GoalError>` con `GoalPatch { status: Option<GoalStatus>, objective: Option<String>, token_budget: Option<Option<u64>> }` (doble `Option` = tri-estado) |
| `goal/clear` | ídem | `pub fn clear(session_id: &str) -> bool` |
| | | `pub enum GoalError { StatusConflict, BudgetLimited, InvalidParams(&'static str), NotFound }` |

**El tri-estado en Rust**: `Option<Option<u64>>` es la forma idiomática de
"omitido / null / valor" ⇒ el `None` exterior = no tocar, `Some(None)` = quitar
el tope, `Some(Some(n))` = ponerlo. Es el mismo patrón que
`serde_json::Value` con `skip_serializing_if`, pero explícito y sin `Value`.

### 3.4 Estado en disco

`state/goals/<session_id>.json` (un goal por sesión, 1:1) + los adjuntos como
ficheros en `state/goals/<session_id>.attachments/`. 1:1 porque el goal se
direcciona por `session_id` y `clear` lo borra entero.

### 3.5 Contrato de la CLI

```
megadaemon-core goal ver       <session_id>
megadaemon-core goal crear     <session_id> --objetivo <txt> [--tokens <n>]
megadaemon-core goal parche    <session_id> [--estado <s>] [--objetivo <txt>] [--tokens <n>|--sin-tope]
megadaemon-core goal limpiar   <session_id>
```

`--sin-tope` es el equivalente de `--tokens 0` (quitar el tope).

### 3.6 Contrato del tool MCP

```
mdaemon/goal  { "accion": "get|create|patch|clear", "session_id": "...",
                "objetivo": "...", "estado": "active|paused|blocked|complete|budget_limited",
                "token_budget": 1234 | null }
```

---

## 4. Cómo encaja esto con nuestro ACP

Nuestro `rust-core/src/protocol_acp.rs` ya define nuestro espacio de
extensiones (`mdaemon/…`). Los 17 métodos de MiniMax se mapean **1:1** a
extensiones `mdaemon/` con el mismo nombre de concepto:

| MiniMax | nosotros (propuesta) |
|---|---|
| `mcode/session/queue/*` (5) | `mdaemon/queue/{list,enqueue,update,delete,steer}` |
| `mcode/session/goal/*` (4) | `mdaemon/goal/{get,create,patch,clear}` |
| `mcode/session/delegation/*` (4) | `mdaemon/delegation/{get,stop,message,transcript}` |
| `mcode/session/steer` | `mdaemon/session/steer` |
| `session/activate`, `mcode/session/activate` | `mdaemon/session/activate` (1 solo; MiniMax tiene el duplicado por compatibilidad) |
| notificaciones (4) | `mdaemon/{session_current_update,queue_update,goal_update,delegation_update}` |

**Regla de nombres**: el dialecto de MiniMax usa `mcode/session/<recurso>/<verbo>`;
el de Devin usa `cognition.ai/<verbo>`; el nuestro ya usa `mdaemon/<recurso>/<verbo>`.
⇒ **el nuestro es el que mejor casa** con el de MiniMax. Es un argumento medido
a favor de mantener `mdaemon/<recurso>/<verbo>` y **no** copiar `cognition.ai/*`.

## 5. Lo que NO está medido

* La **implementación** de `delegation/message` y `delegation/transcript` (no
  están en `extensions.ts`; están en el runtime).
* El **formato en disco** de MiniMax (no es nuestro: es un *spec* de lo que
  debemos hacer, no una copia).
* Si el `goal` de MiniMax sobrevive a un reinicio: medido que el store persiste,
  pero **no** el mecanismo exacto.
* El **número de goals por sesión**: el código direcciona por `session_id` ⇒
  parece 1:1, pero **no medido** de forma explícita.
