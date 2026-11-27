# RE — **ACP comparado**: Devin ∪ acpira ∪ MiniMax Code (+ el canónico)

> MegaDaemon · subagente de RE de productos · 2026‑11‑27
>
> **Objetivo**: decidir el diseño de `rust-core/src/protocol.rs`.
>
> **Fuentes medidas** (todas con `curl` directo, sin `web_search`):
>
> | Fuente | Qué se midió |
> |---|---|
> | `agentclientprotocol/agent-client-protocol` | **el esquema canónico**, en Rust, v1 **y** v2 |
> | `agentclientprotocol/registry` → `.protocol-matrix/latest.json` | **42 agentes probados** el **2026‑11‑26** |
> | `agentclientprotocol/rust-sdk` | SDK Rust oficial + docs de diseño |
> | `hotic/acpira` | dialecto propio, en Rust |
> | `MiniMax-AI/minimax-code` | dialecto vía **SDK oficial** `@agentclientprotocol/sdk` |
> | `Devin/…/devin/bin/devin` | dialecto de Devin (medido en `docs/RE_ACP.md`) |
>
> Copias de lo bajado en `artifacts/acp-comparado/` (no versionado).

---

## 0. El hallazgo que decide el diseño ⭐

**No hay que escribir los tipos a mano.** Existe un **crate Rust oficial** con el
esquema completo, v1 y v2:

```
agent-client-protocol-schema  v1.22.0
  licencia: Apache-2.0
  autores:  Zed <hi@zed.dev>
  repo:    github.com/agentclientprotocol/agent-client-protocol
  docs.rs: https://docs.rs/agent-client-protocol-schema
  edición: 2024, rust-version del workspace
```

Módulos: `v1/{agent,client,content,elicitation,error,ext,mcp,mod,plan,protocol_level,proxy,rpc,tool_call}.rs`
y `v2/{agent,client,…}.rs` (**v2 existe y es oficial**).

⇒ **La estimación de 2.200‑3.150 líneas de `docs/RE_ACP.md` §8 baja a ~300‑500
líneas**: un `Cargo.toml` + `use agent_client_protocol_schema::v1::*` + la capa de
transporte + el mapa de extensiones `cognition.ai/*`. **Apache‑2.0** ⇒ se puede
depender directamente.

---

## 1. El canónico: métodos ACP v1

Extraídos de `agent-client-protocol-schema/src/v1/agent.rs` y `v1/client.rs`.

### 1.1 Cliente → agente

| Método | Request | Response |
|---|---|---|
| `initialize` | `InitializeRequest` | `InitializeResponse` |
| `authenticate` | — | — |
| `session/new` | `NewSessionRequest` | `NewSessionResponse` |
| `session/load` | `LoadSessionRequest` | — |
| `session/resume` | `ResumeSessionRequest` | — |
| `session/delete` | `DeleteSessionRequest` | — |
| `session/list` | — | `ListSessionsResponse` |
| `session/fork` | — | — |
| `session/set_mode` | `SetSessionModeRequest` | — |
| `session/set_config_option` | `SetSessionConfigOptionRequest` | `SetSessionConfigOptionResponse` |
| `session/set_title` | — | — |
| `session/archive` | — | — |
| `session/unarchive` | — | — |
| `session/status` | — | — |
| `session/options` | — | — |
| `session/prompt` | `PromptRequest` | `PromptResponse` |
| `session/cancel` | — | — |
| `session/close` | `CloseSessionRequest` | — |
| `auth/status` | — | — |

### 1.2 Agente → cliente

| Método | Payload |
|---|---|
| `session/update` | `SessionUpdate` |
| `session/request_permission` | — |
| `fs/read_text_file` | — |
| `fs/write_text_file` | — |
| `terminal/create` | — |
| `terminal/output` | — |
| `terminal/wait_for_exit` | — |
| `terminal/kill` | — |
| `terminal/release` | — |
| `elicitation/create` | — |
| `elicitation/complete` | — |
| `auth/status_update` | — |

### 1.3 `SessionUpdate` canónico: **22 variantes**

```
AgentMessageChunk          AgentThoughtChunk        AvailableCommandsUpdate
CompactionSummaryChunk     CompactionUpdate         ConfigOptionUpdate
CurrentModeUpdate          CursorUpdate             Notice
Plan                       PlanRemoved              PlanUpdate
PromptSuggestionsUpdate    SessionInfoUpdate        SessionMessage
SessionMessageChunk        SubagentUpdate           ToolCall
ToolCallUpdate             UsageUpdate              UserMessageChunk
```

### 1.4 Tipos canónicos clave (campos exactos)

```rust
InitializeRequest  { protocol_version, client_capabilities, client_info: Option<Implementation>, meta }
InitializeResponse { protocol_version, agent_capabilities, auth_methods: Vec<AuthMethod>,
                     agent_info: Option<Implementation>, meta }
NewSessionRequest  { cwd, additional_directories: Vec<PathBuf>, mcp_servers: Vec<McpServer>,
                     system_prompt: Option<SystemPrompt>,
                     config_options: Vec<InitialSessionConfigOption>, meta }
NewSessionResponse { session_id, modes: Option<SessionModeState>,
                     config_options: Option<Vec<SessionConfigOption>>, meta }
ListSessionsResponse { sessions: Vec<SessionInfo>, next_cursor: Option<String>, meta }
SessionInfo        { session_id, cwd, additional_directories, title: Option<String>,
                     updated_at: Option<String>, archived: Option<bool>, meta }
SessionModeState   { current_mode_id, available_modes: Vec<SessionMode>, meta }
SessionMode        { id, name, description: Option<String>, meta }
PromptRequest      { session_id, prompt: Vec<ContentBlock>, meta }
PromptResponse     { stop_reason: StopReason, usage: Option<Usage>, meta }
Usage              { total_tokens: u64, input_tokens: u64, output_tokens: u64,
                     thought_tokens: Option<u64>, cached_read_tokens: Option<u64>,
                     cached_write_tokens: Option<u64>, meta }
StopReason         = EndTurn | MaxTokens | MaxTurnRequests | Refusal | Cancelled
AgentCapabilities  { load_session: bool, prompt_capabilities, mcp_capabilities,
                     session_capabilities, auth: AgentAuthCapabilities,
                     providers: Option<ProvidersCapabilities>, meta }
SessionCapabilities{ list, delete, additional_directories, system_prompt, fork, archive,
                     status, options, title, resume, close: Option<…Capabilities>, meta }
McpCapabilities    { http: bool, sse: bool, acp: bool, meta }
PromptCapabilities { image: bool, audio: bool, embedded_context: bool, meta }
ToolCall           { tool_call_id, title, name: Option<String>, kind: ToolKind,
                     status: ToolCallStatus, content: Vec<ToolCallContent>,
                     locations: Vec<ToolCallLocation>,
                     raw_input: Option<Value>, raw_output: Option<Value>, meta }
ToolCallUpdate     { tool_call_id, fields: ToolCallUpdateFields, meta }
ToolCallStatus     = Pending | InProgress | Completed | Failed
ToolCallLocation   { path: PathBuf, line: Option<u32>, meta }
Plan               { entries: Vec<PlanEntry>, meta }
PlanEntry          { content: String, priority: PlanEntryPriority, status: PlanEntryStatus, meta }
```

**Dos diferencias de nomenclatura con lo que medí en Devin**: el canónico usa
`meta` (no `_meta`) y `snake_case` (no camelCase) en Rust, aunque en el **wire**
el JSON es camelCase (`#[serde(rename_all = "camelCase")]`).

---

## 2. Matriz de los **42 agentes** del registro oficial

`registry/.protocol-matrix/latest.json`, fecha **2026‑11‑26**, `agentsProbed: 42`,
`initializeSuccess: 41`.

### 2.1 Soporte por capacidad (sobre los 41 que inicializan)

| Capacidad | Soportada por |
|---|---|
| `loadSession` | **39 / 41** |
| `sessionList` | 35 / 41 |
| `sessionResume` | 25 / 41 |
| `sessionFork` | 18 / 41 |
| `setModel` | 2 / 41 |
| `sessionStop` | **0 / 41** ⇐ *nadie lo implementa* |

### 2.2 Protocolo

**Los 41 agentes que inicializan son `protocolVersion: 1`. Cero agentes en v2.**

⇒ **ACP v2 es hoyistencial**: existe en el esquema oficial (`v2/agent.rs`, 454 KB)
y Devin lo usa, pero **nadie más lo habla**. La matriz se regenera a diario.

### 2.3 `authMethods`

`terminal` 22 · `agent` 23 · `env_var` 3 ⇒ los tres mecanismos conviven.

### 2.4 Los dos agentes de nuestra comparación, en la matriz

| Agente | `protocolVersion` | `initialize` | `authMethods` | capacidades |
|---|---|---|---|---|
| **`devin`** | 1 | success | `agent`, `terminal` | `loadSession`, `sessionList` |
| **`minimax-code`** | 1 | success | `env_var`, `terminal` | `loadSession`, `sessionList`, `sessionFork`, `sessionResume` |
| `claude-acp` | 1 | success | `terminal` | `loadSession`, `sessionList`, `sessionFork`, `sessionResume` |
| `opencode` | 1 | success | `terminal` | las 4 |
| `codex-acp` | 1 | success | `agent` | las 4 |

**Confirmación cruzada**: la matriz dice que `devin` **no** soporta `sessionFork`
(`no_response`), y en el binario de Devin `session/fork` aparece **0 veces**
(coincide). La matriz dice `sessionResume: false` para Devin (probe `no_response`)
pero el binario **sí** contiene `session/resume` ⇒ Devin lo implementa pero el
probe no lo-conditioning (auth). Coherente.

---

## 3. Tabla **método a método**: los tres dialectos

`✔` = implementado · `—` = no · `?` = no medido

| Método (canónico) | **Devin** (binario) | **acpira** (`acpira-agent`) | **MiniMax Code** (TUI) |
|---|---|---|---|
| `initialize` | ✔ | ✔ | ✔ |
| `authenticate` | ✔ | — | ✔ |
| `session/new` | ✔ | ✔ | ✔ |
| `session/load` | ✔ | ✔ | ✔ |
| `session/resume` | ✔ | ✔ | ✔ |
| `session/prompt` | ✔ | ✔ | ✔ |
| `session/cancel` | ✔ | — | ? |
| `session/delete` | ✔ | — | ? |
| `session/list` | ✔ | ✔ | ✔ |
| `session/set_mode` | ✔ | — | ✔ (`session.setMode`) |
| `session/set_config_option` | ✔ | — | ✔ (`session.setConfigOption`) |
| `session/close` | ✔ | — | ✔ |
| **`session/fork`** | — | — | ✔ |
| `session/set_title` | — | — | ? |
| `session/archive` / `unarchive` | — | — | ? |
| `session/status` / `session/options` | — | — | ? |
| `auth/status` | — | — | ? |
| `fs/read_text_file` | ✔ | — | ? (SDK) |
| `fs/write_text_file` | ✔ | — | ? (SDK) |
| `terminal/create` | ✔ | — | ? (SDK) |
| `terminal/output` | ✔ | — | ? (SDK) |
| `terminal/wait_for_exit` | ✔ | — | ? (SDK) |
| `terminal/kill` | ✔ | — | ? (SDK) |
| `terminal/release` | ✔ | — | ? (SDK) |
| `elicitation/create` | ✔ | — | ? (SDK) |
| `elicitation/complete` | ✔ | — | ? (SDK) |
| `session/update` (notificación) | ✔ | ✔ | ✔ |
| `session/request_permission` | ✔ | — | ? (SDK) |
| `auth/status_update` | — | — | ? |

### 3.1 Lectura

- **MiniMax Code habla ACP canónico**, vía `acp.methods.agent.session.*` del SDK
  oficial. Cuando usa el SDK, **no se puede distinguir** qué métodos soporta sin
  abrir el SDK ⇒ marcado `?`.
- **acpira habla un subconjunto mínimo**: `initialize`, `session/new`,
  `session/load`, `session/resume`, `session/prompt`, `session/list`,
  `session/update`. **No** implementa `fs/*`, `terminal/*`, `elicitation/*`:
  sus tools de fichero y shell son **propias del agente**, no del cliente ⇒ es
  un diseño "el agente hace todo" (más simple de implementar, menos componible).
- **Devin implementa 23 de los 30 métodos canónicos**: es el más completo de los
  tres, pero **le faltan `session/fork`, `session/set_title`,
  `session/archive`/`unarchive`, `session/status`, `session/options`,
  `auth/status`** — justo los que el registro oficial sí prueba.
  Curiosidad: Devin tiene el revert/fork **interno** (`cognition.ai/revert/…`),
  no como `session/fork` estándar.

---

## 4. `session/update`: divergencia de variantes

| Variante canónica | Devin | acpira | MiniMax |
|---|---|---|---|
| `user_message_chunk` | ✔ | — | ? |
| `agent_message_chunk` | ✔ | — | ✔ |
| `agent_thought_chunk` | ✔ | — | ? |
| `tool_call` | ✔ | — | ? |
| `tool_call_update` | ✔ | — | ? |
| `available_commands_update` | ✔ | ✔ | ✔ |
| `current_mode_update` | ✔ | ✔ | ✔ |
| `config_option_update` | ✔ | ✔ | ✔ |
| `session_info_update` | ✔ | ✔ | ✔ |
| `usage_update` | ✔ | ✔ | ? |
| `notice` | ✔ | — | ? |
| `compaction_update` | ✔ | — | ? |
| `compaction_summary_chunk` | ✔ | — | ? |
| `plan` / `plan_update` / `plan_removed` | ✔ (v2) | `plan` | **`plan_removed`** ⚠️ |
| `prompt_suggestions_update` | — | — | ? |
| `session_message` / `session_message_chunk` | — | — | ? |
| `subagent_update` | — | — | ? |
| `cursor_update` | — | — | ? |

⚠️ **MiniMax usa `sessionUpdate: 'plan_removed'`**, que es el **nombre plano**,
no el canónico `PlanRemoved` (que en el wire sería `plan_removed` igualmente —
**coincide**). Verificado: el canónico serializa `PlanRemoved` como
`plan_removed`. ⇒ **no hay divergencia real aquí**; MiniMax usa el SDK, que
serializa bien.

### 4.1 ⚠️ Divergencia real: `usage_update`

| Dialecto | Forma |
|---|---|
| **Canónico** | `Usage { total_tokens, input_tokens, output_tokens, thought_tokens?, cached_read_tokens?, cached_write_tokens? }` |
| **Devin** | idéntico al canónico (medido en el binario) |
| **acpira** | **`{ used: u64, size: u64 }`** —otro formato! |

⇒ Si `protocol.rs` habla con acpira, hay que **normalizar `usage`**. Es un caso
de libro de por qué el dialecto mínimo tiene que fijar su forma.

### 4.2 ⚠️ Divergencia real: `current_mode_update`

| Dialecto | Forma |
|---|---|
| **Canónico** | `{ currentModeId }` (+ `availableModes` en `SessionModeState` de `NewSessionResponse`) |
| **Devin** | `{ currentModeId }` + `_meta` con extras |
| **acpira** | `{ currentModeId }` + **`_meta: {"acpira/availableModes": modes}`** |

⇒ acpira mueve la lista de modos al `_meta`. Divergencia menor pero real.

---

## 5. Extensiones propietarias: cada dialecto tiene las suyas

| Dialecto | Extensión | Contenido |
|---|---|---|
| **Devin** | `cognition.ai/*` | **37 capacidades** (ver `docs/RE_ACP.md` §6.1): subagentes, revert, plugins, MCP, browser, refs, modelos, carga de historial… |
| **acpira** | `acpira/*` | `acpira/context`, `acpira/rewind`, `acpira/sidekick`, `acpira/summoned`, `acpira/turn`, `acpira/availableModes` |
| **MiniMax** | `mcode/*` | `mcode/session/steer`, `mcode/session/queue/{list,enqueue,update,delete,steer}`, `mcode/session/goal/{get,create,patch,clear}`, `mcode/session/delegation/{get,stop}`, `mcode/session/current_session_update`, `mcode/session/queue_update`, `mcode/session/goal_update`, `mcode/session/delegation_update` |
| **MiniMax** | `session/activate` | nombre **sin** prefijo de producto ⇒ parece semicanónico |

**Sobrelapamiento notable**: `acpira/rewind` ≈ `cognition.ai/revert` (deshacer y
seguir por otro camino) y `acpira/sidekick` ≈ Fusion (lead + sidecar) ≈
`local_fusion/sidekick_tool.rs` en Devin. **MiniMax `mcode/session/delegation`**
≈ subagentes. ⇒ **las extensiones se parecen entre sí aunque los productos sean
competidores**: hay un patrón común deindustry que podemos implementar **una vez**.

---

## 6. El mínimo común — qué debería hablar nuestro `protocol.rs`

### 6.1 Restricción dura: **v1**

Los 41 agentes que inicializan son v1. Ninguno habla v2. El registro se regenera
a diario y lo comprueba con una matriz de 42 agentes.

⇒ **`protocol.rs` debe hablar v1 como mínimo**, y v2 **sólo** si hay un cliente
que lo pida. El campo `protocolVersion` se negocia en `initialize` y el resto
del mundo es v1.

### 6.2 Núcleo mínimo (lo que los 3 dialectos + 41 agentes hacen)

| Elemento | Decisión |
|---|---|
| Transporte | JSON‑RPC 2.0 sobre stdio (mínimo) |
| `initialize` / `InitializeResponse` | **obligatorio** — 41/41 |
| `session/new` | **obligatorio** — 41/41 |
| `session/prompt` | **obligatorio** |
| `session/update` (notificación) | **obligatorio** |
| `session/load` | **casi obligatorio** (39/41 con `loadSession`) |
| `session/list` | **recomendado** (35/41) |
| `session/resume` | **recomendado** (25/41) |
| `session/set_mode` | **opcional** — pero MiniMax y Devin lo tienen |
| `session/set_config_option` | **opcional** — MiniMax y Devin lo tienen |
| `session/cancel` | **opcional** |
| `fs/*`, `terminal/*`, `elicitation/*` | **opcional** — sólo Devin y el SDK oficial |
| `session/fork` | **opcional** — sólo MiniMax (18/41) |
| `session/set_title`, `archive`, `status`, `options`, `auth/status` | **no prioritario** — nadie de nuestra comparación |

### 6.3 Formato de `usage`: **el canónico**, no el de acpira

El canónico es el que usa el SDK oficial y el que usa Devin. acpira es el
minoritario. ⇒ `protocol.rs` adopta `Usage` canónico y, si hace falta hablar con
acpira, normaliza en el borde.

### 6.4 Los 13 `SessionUpdate` de Devin ⊂ los 22 canónicos

Verificado uno a uno. ⇒ **cualquier variante que Devin emite es estándar**.

### 6.5 Extensiones: **namespace por producto**, campo `_meta`

Los tres hacen lo mismo: las extensiones van en `_meta` (canónico: `meta`) con
un prefijo de producto (`cognition.ai/`, `acpira/`, `mcode/`).
⇒ **nuestro `protocol.rs` debe dejar `meta` abierto y no parsearlo**, y exponer
un registro de extensiones: `HashMap<String, serde_json::Value>`.

---

## 7. Decisión de diseño para `rust-core/src/protocol.rs`

```
rust-core/src/protocol/
├── mod.rs              # re-exports
├── transport.rs        # JSON-RPC 2.0 sobre stdio  (~250 L)
├── v1.rs               # re-export de agent-client-protocol-schema::v1  (~40 L)
├── session.rs          # gestión de sesión ACP (create/load/list/resume) (~400 L)
├── updates.rs          # reducción de SessionUpdate a eventos internos (~300 L)
├── extensions.rs       # registro _meta: HashMap<String, Value> (~150 L)
└── permissions.rs      # session/request_permission + elicitation (~300 L)
```

| Bloque | Líneas | Nota |
|---|---|---|
| `Cargo.toml` + dependencia `agent-client-protocol-schema = "1.22"` | 3 | **Apache‑2.0** |
| `v1.rs` (re-exports + `SessionUpdate` → evento interno) | 300‑500 | **sustituye a las 2.200‑3.150** de la estimación anterior |
| `transport.rs` (JSON‑RPC 2.0 stdio) | 200‑300 | puede reutilizarse `acpira-rpc` como referencia de diseño |
| `session.rs` | 350‑500 | |
| `extensions.rs` | 120‑200 | |
| `permissions.rs` | 250‑350 | |
| tests (22 variantes × 3 dialectos) | 300‑450 | tabla de la §3 como test |
| **TOTAL** | **1.500‑2.200** | frente a 2.200‑3.150 **sin** el crate |

**Descuentos aplicados**: usar el crate canónico ahorra ~1.800 líneas; no
implementar v2 ahorra ~700; no implementar `fs/*`/`terminal/*` ahorra ~400 (se
pueden añadir después).

**Interoperabilidad medida**: con el núcleo de la §6.2 se habla con
**41/41 agentes** del registro (todos hacen `initialize` + `session/new` +
`session/prompt` + `session/update`), y se cubren los 3 dialectos comparados.

---

## 8. Lo NO medido

| Elemento | Por qué |
|---|---|
| Los métodos que MiniMax **no** usa (`?` en la tabla) | Usa el SDK oficial; habría que abrir el SDK y ver el enum de métodos, no el cliente |
| `session/fork` en acpira | acpira tiene `session_manager/fork.rs` pero es **fork de sesión interna**, no `session/fork` ACP |
| El schema v2 del canónico | Sólo se ha medido que existe (`v2/agent.rs`, 454 KB), no su contenido |
| `acp-tck` (Test Compatibility Kit) | Existe (`agentclientprotocol/acp-tck`); no se ha ejecutado |
| Los 42 agentes de la matriz, uno a uno | Sólo se han extraído `devin`, `minimax-code` y 10 más |
| El dialecto de acpira en `fs`/`terminal` | No los implementa: sus tools son propias del agente |

---

## 9. Trazabilidad

| Afirmación | Evidencia |
|---|---|
| Crate canónico, versión, licencia | `agent-client-protocol/Cargo.toml`, `agent-client-protocol-schema/Cargo.toml` |
| Métodos canónicos v1 | `…/src/v1/agent.rs` (377.486 B) + `v1/client.rs` (278.957 B) |
| 22 variantes de `SessionUpdate` | `awk '/pub enum SessionUpdate/,/^}/'` sobre `v1/client.rs` |
| Tipos canónicos campo a campo | extracción de `pub <campo>: <tipo>` sobre `v1/agent.rs` |
| Matriz de 42 agentes | `registry/.protocol-matrix/latest.json` (104.355 B, fecha 2026‑11‑26) |
| Devin v1, `sessionFork:false` | matriz (`devin`) + `devin_cli_strings.txt` (0 ocurrencias de `session/fork`) |
| MiniMax usa el SDK oficial | `packages/tui/src/acp/agent.ts`: `import * as acp from '@agentclientprotocol/sdk'` |
| MiniMax: `session.new/list/fork/load/resume/close/setMode/setConfigOption` | `packages/tui/src/acp/agent.ts` (`app.onRequest(acp.methods.agent.session.*)`) |
| Extensiones de MiniMax | `packages/tui/src/acp/extensions.ts` (`TUI_ACP_EXTENSION_METHODS`, `TUI_ACP_EXTENSION_NOTIFICATIONS`) |
| acpira: métodos y variantes | `rust/crates/acpira-agent/src/acp.rs`, `acp/sessions.rs`, `acp/options.rs` |
| acpira: extensiones | `acpira/context`, `acpira/rewind`, `acpira/sidekick`, `acpira/summoned`, `acpira/turn` |
| acpira: `usage_update` con `{used,size}` | `acp/sessions.rs:51` y `:259` |
| acpira: `current_mode_update` con `_meta.acpira/availableModes` | `acp/options.rs:202` |
| acpira: no implementa fs/terminal/elicitation | ausencia de esos literales en todo `acpira-agent` |
