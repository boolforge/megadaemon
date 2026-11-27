# RE — **ACP (Agent Client Protocol)**: contrato de referencia

> MegaDaemon · subagente de RE de productos · 2026‑11‑27
> ACP es **el estándar de facto** para conectar un editor con un agente: ya lo
> implementan tres productos medidos — **`mcode acp` de MiniMax Code**, **acpira** y
> **Devin Desktop / Devin CLI (chisel)**. Este documento fija el esquema literal.
>
> **Todo lo aquí'affirmado está medido** en el binario
> `Devin/resources/app/extensions/windsurf/devin/bin/devin` (262.445.096 B, crate
> `pkg.cognition.build-3983e96abc3e6656`) y en
> `Devin/resources/app/extensions/windsurf/dist/acp/AGENTS.md` (31.656 B).
> Las cadenas del binario son las **listas de campos serde** que el compilador dejó
> en `.rodata`; de ahí sale el esquema **campo por campo**, sin adivinar.

---

## 1. Por qué ACP y no otra cosa

Tres implementaciones independientes measured, con el mismo cable:

| Implementación | Rol | Dónde |
|---|---|---|
| **chisel** (Devin CLI/Desktop) | **agente** | `chisel-acp-client`, `chisel-acp-relay`, `chisel-agent/src/acp_server/` |
| **mcode acp** (MiniMax Code) | agente | fuera de este paquete |
| **acpira** | cliente | fuera de este paquete |

El cliente (`chisel-acp-relay`) habla con **agentes de terceros** y el agente
(chisel) habla con **clientes de terceros** — es simétrico. `AGENTS.md` lo dice:
*"The same events fire in the Devin CLI, Devin Desktop, and cloud sessions"* y el
relay es `chisel-acp-relay/src/sessions.rs`.

**Transporte**: JSON‑RPC 2.0 sobre **stdio** (proceso local) o **WebSocket**
(Devin Cloud). Del lado del host remoto, NDJSON sobre un enlace de host
(`wire.ts` → `Hello`, `Hostd`, `RemoteAgent`, `ConfigProblem`, `AttachAgent`,
`AttachTerminal`, `TermOut`).

---

## 2. Negociación: `initialize`

Medido en `chisel-agent/src/acp_shared/initialize.rs` y en las cadenas del binario.

### 2.1 Mensaje

```jsonc
// → agente
{
  "jsonrpc": "2.0", "id": 1, "method": "initialize",
  "params": {
    "protocolVersion": 2,          // 1 | 2  (el cliente ofrece 2; 1 en builds airgap)
    "clientCapabilities": { … },  // ver §2.3
    "clientInfo": { "name": "windsurf", "version": "1.141.2" },
    "_meta": { … }                 // extensiones cognition.ai/*
  }
}
```

```jsonc
// ← agente
{
  "jsonrpc": "2.0", "id": 1,
  "result": {
    "agentCapabilities": { … },   // ver §2.4
    "authMethods": [ … ],
    "agentInfo": { "name": "chisel", "version": "…" }
  }
}
```

`clientInfo.name` envía **`"windsurf" | "windsurf-next" | "windsurf-insiders"`**
(`AGENTS.md`) para que el agente atribuya la analítica al IDE anfitrión.

### 2.2 `Implementation`

`{ name: string, version: string }`

### 2.3 `ClientCapabilities` (v2)

```
ClientCapabilities {
  fs:            FileSystemCapabilities      // { readTextFile: bool, writeTextFile: bool }
  session:       SessionCapabilities          // ver §2.5
  elicitation:   ElicitationCapabilities      // { form: {...}, url: {...} }
}
```

`fs` es **declarativo y se hace cumplir**: `rejectUndeclaredFsRequest()` responde
`method_not_found` a `fs/read_text_file` / `fs/write_text_file` si el conector no
declaró la capacidad (`AGENTS.md`, sección `clientCapabilities.fs`).

### 2.4 `AgentCapabilities`

```
AgentCapabilities {
  loadSession:        bool
  promptCapabilities: PromptCapabilities   // { image: bool, audio: bool, embeddedContext: bool }
  mcpCapabilities:    McpCapabilities      // { http: bool, sse: bool }
  sessionCapabilities: SessionCapabilities
}
```

### 2.5 `SessionCapabilities` (capacidades **anunciadas por el agente**)

```
SessionCapabilities {
  delete:       SessionDeleteCapabilities
  systemPrompt: SessionSystemPromptCapabilities
  resume:       SessionResumeCapabilities
  close:        SessionCloseCapabilities
}
```
Sub‑capacidades: `SessionListCapabilities`,
`SessionAdditionalDirectoriesCapabilities`,
`SessionSystemPromptModeCapabilities`,
`SessionSystemPromptOverrideCapabilities`.

### 2.6 `AuthMethod`

Enum **internamente etiquetado**:

```
AuthMethod = { type: "terminal", … } | { type: "<otro>", … }
AgentAuthCapabilities { logout: bool }
```

---

## 3. Métodos ACP

### 3.1 Cliente → agente (11 del núcleo + 9 auxiliares)

| Método | Request | Response |
|---|---|---|
| `initialize` | `InitializeRequest` | `InitializeResponse` |
| `authenticate` | `AuthenticateRequest` | — |
| `session/new` | `NewSessionRequest` | `NewSessionResponse` |
| `session/load` | `LoadSessionRequest` | — |
| `session/resume` | `ResumeSessionRequest` | — |
| `session/delete` | `DeleteSessionRequest` | — |
| `session/list` | — | `ListSessionsResponse` |
| `session/set_mode` | `SetSessionModeRequest` | — |
| `session/set_config_option` | `SetSessionConfigOptionRequest` | `SetSessionConfigOptionResponse` |
| `session/prompt` | `PromptRequest` | `PromptResponse` |
| `session/cancel` | `CancelNotification` (notificación) | — |
| `session/close` | `CloseSessionRequest` | — |
| `fs/read_text_file` | — | `ReadTextFileResponse` |
| `fs/write_text_file` | — | — |
| `terminal/create` | — | `CreateTerminalResponse` |
| `terminal/output` | — | `TerminalOutputResponse` |
| `terminal/wait_for_exit` | — | — |
| `terminal/kill` | — | — |
| `terminal/release` | — | — |
| `elicitation/create` | `CreateElicitationRequest` | `CreateElicitationResponse` |

### 3.2 Agente → cliente (notificaciones)

| Método | Payload |
|---|---|
| `session/update` | `SessionUpdate` (13 variantes, §5) |
| `session/request_permission` | `RequestPermissionOutcome` |

### 3.3 Esquema literal de cada mensaje

**`NewSessionRequest`** — `{ cwd: string, mcpServers: McpServer[] }`
**`NewSessionResponse`** — `{ modes: SessionModeState }`
**`SessionModeState`** — `{ currentModeId: string, availableModes: SessionMode[] }`
**`SessionMode`** — `{ id: string, name: string, description: string }`
**`LoadSessionRequest`** — `{ sessionId: string, cwd: string, mcpServers: McpServer[] }`
**`ResumeSessionRequest`** — `{ sessionId: string }`
**`DeleteSessionRequest`** — `{ sessionId: string }`
**`CloseSessionRequest`** — `{ sessionId: string }`
**`ListSessionsResponse`** — `{ sessions: SessionInfo[], nextCursor: string | null }`
**`SessionInfo`** — `{ sessionId: string, cwd: string, title: string, updatedAt: string, currentModeId: string, availableModes: SessionMode[] }`
**`SetSessionModeRequest`** — `{ sessionId: string, modeId: string }`
**`PromptRequest`** — `{ prompt: ContentBlock[] }`
**`PromptResponse`** — `{ stopReason: StopReason }`
**`StopReason`** — `end_turn | max_tokens | max_turn_requests | refusal | cancelled`
**`Usage`** — `{ max_tokens, max_turn_requests, refusal, cancelled, totalTokens, inputTokens, outputTokens, thoughtTokens, cachedWriteTokens }`
**`CancelNotification`** — `{ sessionId: string }`
**`SystemPrompt`** — `{ content: string }`

**`McpServer`** — enum **internamente etiquetado** (`type`):

| Variante | Campos |
|---|---|
| `http` | `{ type: "http", url: string, headers: HttpHeader[] }` |
| `sse` | `{ type: "sse", url: string, headers: HttpHeader[] }` |
| `stdio` | `{ type: "stdio", command: string, env: EnvVariable[] }` |

**`HttpHeader`** — `{ name: string, value: string }`
**`EnvVariable`** — `{ name: string, value: string }`

**`SessionConfigOption`** — enum **internamente etiquetado**:

| Variante | Campos |
|---|---|
| `select` | `{ type: "select", value: SessionConfigSelectOption }` |
| `boolean` | `{ type: "boolean", value: bool }` |
| `value` | `{ type: "value", value: string }` |

**`SessionConfigSelectOption`** — `{ value: string, label?: string, description?: string }`
**`SessionConfigSelectGroup`** — `{ group: string, options: SessionConfigSelectOption[] }`
**`SessionConfigSelect`** — `{ currentValue: string, options: SessionConfigSelectOption[] }`
**`SessionConfigOptionCategory`** — `"select" | "boolean" | "value"`
**`SetSessionConfigOptionRequest`** — `{ sessionId: string, configId: string, value: … }`
**`SetSessionConfigOptionResponse`** — `{ configOptions: SessionConfigOption[] }`

**`RequestPermissionOutcome`** — enum:

| Variante | Campos |
|---|---|
| `cancelled` | `{ outcome: "cancelled" }` |
| `selected` | `{ outcome: "selected", optionId: string }` |

**`PermissionOption`** — `{ optionId: string, name: string, kind: … }`
**`ReadTextFileResponse`** — `{ content: string, limit?: number }`
**`CreateTerminalResponse`** — `{ command: string, env: EnvVariable[], cwd: string, outputByteLimit: number }`
**`TerminalOutputResponse`** — `{ output: string, truncated: bool, exitStatus: TerminalExitStatus }`

---

## 4. Contenido (bloques)

```
ContentBlock = TextContent | ResourceLink | ImageContent | AudioContent | EmbeddedResource

TextContent      { text: string, annotations?: Annotations }
ImageContent     { data: string, mimeType: string, uri?: string, annotations?: Annotations }
AudioContent     { data: string, mimeType: string, annotations?: Annotations }
EmbeddedResource { resource: Resource, annotations?: Annotations }
Resource         = TextResourceContents | BlobResourceContents
TextResourceContents { uri: string, text: string, mimeType?: string }
BlobResourceContents { uri: string, blob: string, mimeType?: string }
ResourceLink     { uri: string, name?: string, mimeType?: string, size?: number,
                   title?: string, description?: string, annotations?: Annotations }
Annotations      { audience?: …, priority?: …, lastModified?: … }
```

**Plan** (v2) — `{ entries: PlanEntry[] }`;
**`PlanEntry`** — `{ content: string, status: "pending" | "in_progress" | "completed",
priority: "high" | "medium" | "low" }`.

---

## 5. `session/update`: las 13 variantes

Notificación con tag `sessionUpdate`:

| Variante | Campos |
|---|---|
| `user_message_chunk` | `{ content: ContentBlock }` |
| `agent_message_chunk` | `{ content: ContentBlock }` |
| `agent_thought_chunk` | `{ content: ContentBlock }` |
| `tool_call` | `{ toolCallId, title, kind, status, content, locations, rawInput, rawOutput }` |
| `tool_call_update` | idem `tool_call` (actualización parcial) |
| `available_commands_update` | `{ availableCommands: … }` |
| `current_mode_update` | `{ currentModeId: string }` |
| `config_option_update` | `{ configOptions: SessionConfigOption[] }` |
| `session_info_update` | `{ title: string, updatedAt: string }` |
| `usage_update` | `{ usage: Usage }` |
| `notice` | `{ message: string }` |
| `compaction_update` | `{ compactionId: string, status: CompactionStatus }` |
| `compaction_summary_chunk` | `{ summary: string }` |

### 5.1 Llamadas a herramienta

```
ToolCall {
  toolCallId: string
  title: string
  kind:   ToolCallKind
  status: ToolCallStatus     // pending | in_progress | completed | failed
  content: ToolCallContent[] // content | diff | terminal
  locations: ToolCallLocation[]
  rawInput:  …   // JSON del tool call original
  rawOutput: …   // salida cruda
}
ToolCallKind   = read | edit | move | delete | search | execute | think
               | fetch | switch_mode | tool | other
ToolCallContent = { type: "content", content: ContentBlock }
               | { type: "diff",     diff: Diff }
               | { type: "terminal", terminal: Terminal }
Diff     { path: string, oldText: string, newText: string }
Terminal { terminalId: string, oldText: string, newText: string }
ToolCallLocation { path: string, line: number }
```

### 5.2 Elicitación (formularios)

```
CreateElicitationRequest { sessionId, message, mode: ElicitationMode, schema: ElicitationSchema }
ElicitationMode = "form" | "url"
ElicitationSchema = { type: "object", properties: { [name]: ElicitationPropertySchema }, required?: string[] }
ElicitationPropertySchema = StringPropertySchema | NumberPropertySchema | IntegerPropertySchema
                          | BooleanPropertySchema | MultiSelectPropertySchema | EnumOption
StringPropertySchema  { type: "string",  title?, description?, minLength?, maxLength?, format? }
NumberPropertySchema  { type: "number",  title?, description?, minimum?, maximum? }
IntegerPropertySchema { type: "integer", title?, description?, minimum?, maximum? }
BooleanPropertySchema { type: "boolean", title?, description?, default? }
MultiSelectPropertySchema { type: "multiSelect", items: MultiSelectItems, title?, description? }
MultiSelectItems = StringMultiSelectItems { type: "string" }
                 | TitledMultiSelectItems { type: "titled", options: { value, title }[] }
EnumOption { value: string, label: string, description?: string }
CreateElicitationResponse { action: ElicitationAcceptAction }
ElicitationAcceptAction { action: "accept", content: { [field]: value } }
CompleteElicitationNotification { elicitationId: string }
ElicitationCapabilities { form: ElicitationFormCapabilities, url: ElicitationUrlCapabilities }
ElicitationSessionScope { scope: "session" }
ElicitationRequestScope { scope: "request" }
```

Devin Desktop **gestiona en nativo** el `elicitation/create` con formulario y
responde `method_not_found` a los modos no declarados (`AGENTS.md`).

---

## 6. Las 37 capacidades `cognition.ai/*`

Todas van en `initialize.params._meta` (o en `clientCapabilities._meta`) y son
**opt‑in**: el agente sólo anuncia la capacidad correspondiente si el cliente la
pide. Fuente: `extensions/windsurf/dist/acp/AGENTS.md`, párrafo por párrafo.

### 6.1 Tabla — nombre, qué lleva, dónde aterriza en MegaDaemon

| # | Capacidad | Qué lleva / qué habilita | Módulo `rust-core/src/` |
|---|---|---|---|
| 1 | `cognition.ai/lazyRepoOptions` | Acepta opciones de config `repos` marcadas `cognition.ai/optionsDeferred`; el cliente carga los repos solo | `board.rs` |
| 2 | `cognition.ai/locale` | Locale BCP‑47 (`vscode.env.language`) para los textos del selector de modelos | `automation_runtime.rs` |
| 3 | `cognition.ai/statelessShellReplay` | Replay sin estado + terminales nativos; sólo ACP v2 | `agent_loop.rs` |
| 4 | `cognition.ai/shellChainReads` | Un `Read` representa un comando de una cadena (`cognition.ai/replacesChainMember`) | `agent_loop.rs` |
| 5 | `cognition.ai/queuedMessagesInfoUpdate` | La cola de envío se lee de `messages_in_queued` (`cognition.ai/queuedMessages` en `_meta`) | `service_bus.rs` |
| 6 | `cognition.ai/subagentSupport` | Habilita subagentes | `agent_core.rs` / `meta_agent.rs` |
| 7 | `cognition.ai/subagentControl` | Métodos `cognition.ai/subagent/{foreground,background,cancel}` (opt‑in mutuo) | `agent_core.rs` |
| 8 | `cognition.ai/multiRootWorkspace` | Workspaces con varias raíces | `board.rs` |
| 9 | `cognition.ai/partialContent` | Datos parciales de formulario en el `_meta` del decline | `protocol.rs` |
| 10 | `cognition.ai/messageGrouping` | Previews en streaming de tool calls + reagrupación por `clientMessageId` | `protocol.rs` |
| 11 | `cognition.ai/wholeUserMessages` | Prompt multi‑bloque como un único `user_message` v2; `cognition.ai/quotedArtifacts` en el `_meta` del bloque de texto | `protocol.rs` |
| 12 | `cognition.ai/refTagsRaw` | **No** convertir `<ref_file>`/`<ref_snippet>` a `resource_link` en el servidor | `protocol.rs` (ref tags) |
| 13 | `cognition.ai/clientProvidedModels` | El cliente enriquece las opciones de modelo con su catálogo nativo | `providers.rs` |
| 14 | `cognition.ai/groupedSessionConfigOptions` | Opciones de modelo agrupadas en `SessionConfigSelectGroup` | `providers.rs` |
| 15 | `cognition.ai/windsurfConfigBridge` | Opciones con categorías `_windsurf_*` (`_windsurf_ignore_files`, `_windsurf_max_turn_requests`) | `tool_policy.rs` |
| 16 | `cognition.ai/revert` | `cognition.ai/revert/{listSteps,preview,execute,forkFromStep}` (opt‑in mutuo) | `agent_loop.rs` (checkpointing) |
| 17 | `cognition.ai/loadStats` | Notificaciones `_loadStarting`, `_loadRead`, `_loadStats` y `_processMemory` | telemetría (nuevo) |
| 18 | `cognition.ai/externalHistorySkeleton` | Carga acotada: `cognition.ai/tailReplay`, `_meta.cognition.ai/replay{startRowId,endRowId}`, `replayFilterVersion`, `historySkeleton`, `_cognition.ai/session/loadPage` | `agent_loop.rs` (session store) |
| 19 | `cognition.ai/mcp` | `cognition.ai/mcp/{listServers,toggleServer,toggleTool}` (opt‑in mutuo) | `mcp_ext.rs` |
| 20 | `cognition.ai/plugins` | `cognition.ai/plugins/{list,assets}` (opt‑in mutuo) | `skills.rs` |
| 21 | `cognition.ai/editorContext` | El cliente inyecta estado de editor (`_cognition.ai/document/*`) | `protocol.rs` |
| 22 | `cognition.ai/requestDiagnostics` | Registro de diagnósticos en el agente; flag `CASCADE_AUTO_FIX_LINTS` | `tool_policy.rs` |
| 23 | `terminal_output` | Metadatos `terminal_info`, `terminal_output`, `terminal_exit` en `tool_call`/`tool_call_update` | `protocol.rs` |
| 24 | `cognition.ai/lazyEditorFiles` | Descarga perezosa de diffs vía `_cognition.ai/resource/load` | `protocol.rs` |
| 25 | `cognition.ai/toolCallQuestions` | Las preguntas llegan como `ToolCallStart`/`ToolCallProgress` | `protocol.rs` |
| 26 | `cognition.ai/questionAnswerRows` | La respuesta llega como su propio `user_message_chunk` (`cognition.ai/questionEventId`, `cognition.ai/answerEventId`) | `protocol.rs` |
| 27 | `cognition.ai/browserPreview` | Tool `browser_preview` + notificaciones `_cognition.ai/browserPreview/capture` | `automation_runtime.rs` |
| 28 | `cognition.ai/browserPreviewOpen` | El cliente abre la URL: comando `devin.desktopShell.openBrowserPreview(connectorId, sessionId, previewUrl)` | `automation_runtime.rs` |
| 29 | `cognition.ai/browserTab` | El cliente sirve `_cognition.ai/browserTab/*`; el webapp manda `cognition.ai/desktopLocalBrowser: true` | `automation_runtime.rs` |
| 30 | `cognition.ai/cloudHandoff` | El cliente dibuja el progreso de `/handoff`; notificaciones `_cognition.ai/handoff/status` (connecting, gathering context, creating, done, error) | `board.rs` |
| 31 | `cognition.ai/cloudSetupSecretValues` | Muestra la tarjeta de secretos; `_meta.cognition.ai/cloudSetupSecretValues` | `board.rs` |
| 32 | `cognition.ai/cloudSetupMcpKeyValues` | Muestra las claves MCP; `_meta.cognition.ai/cloudSetupMcpKeyValues` | `mcp_ext.rs` |
| 33 | `cognition.ai/mcpAuthorizationRelay` | **Sólo host remoto**: el navegador está en esta máquina, no en el agente → `_cognition.ai/mcp/authorization{Pending,RedirectPort,Required,Finished}` | `mcp_ext.rs` |
| 34 | `cognition.ai/sessionListFolders` | Carpetas en la barra lateral: `cognition.ai/folder` por sesión y `cognition.ai/sidebarFoldersByOrg` en la respuesta | `board.rs` |
| 35 | `cognition.ai/sessionListPermissionScopes` | `session/list` de cuenta con scopes derivados de permisos; filtro `cognition.ai/participant: [me]`; flag `cognition.ai/sessionListExtendedFilters` | `board.rs` / `service_bus.rs` |
| 36 | `cognition.ai/fastContext` | Tool inline `find_code_context`. Puerta: `!teamConfig.disableFastContext && unleash('cascade-find-code-context') && !userSettings.disableInstantContextAgent`; backend `GetDevstralStream` | `agent_loop.rs` |
| 37 | `cognition.ai/clientEnvironment` (en `_meta`, no en capacidades) | `{ acpBackend, apiKey, apiServerUrl }` — el backend ACP elige la ruta antes de la respuesta a `initialize` | `providers.rs` |

### 6.2 Además, capacidades ACP **estándar** (no `cognition.ai`)

| Capacidad | Dónde | Para qué |
|---|---|---|
| `elicitation` (`{ form: {} }`) | `clientCapabilities` | Formularios de elicitation |
| `fs` (`{ readTextFile, writeTextFile }`) | `clientCapabilities` | Acceso a ficheros del cliente; **se hace cumplir** |
| `clientInfo` | `initialize.params` | Identificación del cliente |

---

## 7. Del lado del cliente: transport y tipos

`chisel-acp-client` (4 ficheros) + `chisel-acp-relay` (5):

| Fichero | Rol |
|---|---|
| `chisel-acp-client/src/bootstrap.rs` | Arranque del conector |
| `chisel-acp-client/src/child.rs` | Proceso hijo (agente local) |
| `chisel-acp-client/src/connection.rs` | Transporte + ciclo de vida |
| `chisel-acp-client/src/handshake.rs` | `initialize` y negociación de versión |
| `chisel-acp-relay/src/connection.rs` | Relay Cloud ↔ cliente |
| `chisel-acp-relay/src/jsonrpc.rs` | Capa JSON‑RPC |
| `chisel-acp-relay/src/sessions.rs` | Estado de sesiones |
| `chisel-acp-relay/src/state.rs` | Estado del relay |

**Modelo de conexión** (`AGENTS.md`): un conector = (agente, ubicación).
`AcpConnectorLocation` = `local` | `cloud` | `RemoteHost`.
`connectorIdFor(agentId, location)` → el id desnudo para local/cloud,
`<agent>@<hostId>` para remoto. `hostId` = alias SSH o `wsl+<distro>`.
URI `devin-host://<hostId>/<path>`.

**Transparencia de versión**: el host guarda el `protocolVersion` del agente en
`initialize`, almacena las notificaciones `session/update` tal cual y las reproduce
en esa versión. Un agente v1 (chisel, OpenCode) sigue v1; uno v2 sigue v2.

---

## 8. Cómo encaja esto en `rust-core/src/protocol.rs`

`protocol.rs` (12.316 B) es nuestro dialecto. Lo que hay que escribir:

| Bloque | Tamaño est. | Origen de la verdad |
|---|---|---|
| Tipos ACP v1 (11 mensajes, ~30 structs) | 900‑1.200 L | §3.3 de este doc |
| `SessionUpdate` + 13 variantes | 250‑350 L | §5 |
| Content blocks + tools | 200‑300 L | §4, §5.1 |
| Elicitation | 250‑400 L | §5.2 |
| Capa `_meta` de extensiones | 400‑600 L | §6.1 (37 claves) |
| Handshake `initialize` (capabilities + negociación) | 200‑300 L | §2 |

**Total estimado: 2.200‑3.150 líneas** para un ACP v1+v2 completo con elicitación,
tools y el mapa de extensiones. Es el bloque más grande y el más reutilizable de
toda la tanda: las otras dos implementaciones (`mcode acp`, acpira) hablan el mismo
cable.

---

## 9. Lo NO medido

| Elemento | Por qué |
|---|---|
| Versiones exactas de `protocolVersion` aceptadas por cada agente | Requiere ejecutar los agentes |
| Los `SessionMode` concretos que devuelve chisel | Requiere sesión real |
| Los `enum` completos de `PermissionOption.kind` | Requiere sesión real |
| Los campos de `elicitation/*` en modo `url` | No aparece en el binario con detalle |
| El contenido de `_meta` que **envía** el agente en cada notificación | Sólo se han medido las claves |

---

## 10. Trazabilidad

| Afirmación | Evidencia |
|---|---|
| Tipos y campos ACP v1 | `bin/devin` strings 441.835 / 441.874 / 441.886 / 441.899 (listas de campos serde) |
| Tipos y campos ACP v2 | `bin/devin` región `SessionUpdate` (offset 182.534.049) y `ToolCallUpdate` (182.561.112) |
| Elicitación | `bin/devin` región `struct CreateElicitationRequest` |
| Contenido y Plan | `bin/devin` región `struct PlanEntry` |
| 37 capacidades `cognition.ai/*` | `extensions/windsurf/dist/acp/AGENTS.md` (párrafo por párrafo) |
| Transporte, conectores, hosts remotos | `extensions/windsurf/dist/acp/AGENTS.md` |
| Módulos del cliente | `chisel-acp-client/src/{bootstrap,child,connection,handshake}.rs`, `chisel-acp-relay/src/*` |
