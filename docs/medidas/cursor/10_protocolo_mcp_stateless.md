# CONTRATO 10 — PROTOCOLO MCP STATELESS (JSON-RPC 2.0) DEL AGENT HOST DAEMON

> **Contrato de referencia para el `mcp-server` de MegaDaemon.**
> Todo lo que hay aquí son **nombres de método y nombres de campo**: es el *formato* del
> protocolo, no código. La implementación de Cursor es proprietary y **no se redistribuye**;
> lo que se documenta aquí es la forma del mensaje, que es un estándar JSON-RPC 2.0 + MCP.
>
> **Fuente:** `usr/share/cursor/resources/app/extensions/cursor-agent-host/dist/agent-host-daemon/dist/bin/daemon.cjs`
> (bundle de 23 MB; los números de línea son del bundle tal cual).
> **Build medido:** `{"cursorVersion":"3.32.4","commit":"0925c4b8226c56947c43f0b555fb98e187a7e029","buildDate":"2026-11-24T06:34:58.000Z"}`
> (`build-info.json`).

---

## 10.1 Transporte

**Stateless HTTP** (Streamable HTTP de MCP). El daemon acepta `POST` con `Content-Type:
application/json` y un cuerpo JSON-RPC; la respuesta es `200` con `content-type: application/json`.

`daemon.cjs:14993` — el lector del POST:
- exige `method === "POST"`;
- exige `typeof body === "string"`;
- exige que la URL apunte al endpoint correcto;
- parsea el cuerpo y exige `parsed["jsonrpc"] === "2.0"`;
- distingue `isRequest` (tiene `method` **e** `id`) de notificación.

`daemon.cjs:14997` — respuestas especiales:
- `notifications/initialized` → `202 No Content` (cuerpo vacío);
- `initialize` → antes de responder, el servidor emite un `server/discover` con el `id` del cliente;
- `server/discover` es un método **no estándar de MCP** propio de Cursor (ver 10.6).

`daemon.cjs:14995` — la respuesta JSON-RPC se construye con `jsonrpc:"2.0"` y
`content-type: application/json`.

---

## 10.2 Sobre JSON-RPC (campos literales)

`daemon.cjs:14906` y `:14907`.

| Mensaje | Campos (nombre del esquema → campos) |
|---|---|
| `RequestSchema` | `method` (string), `params` (opcional, objeto laxo) |
| `NotificationSchema` | `method` (string), `params` (opcional, objeto laxo) |
| `ResultSchema` | `_meta` (opcional) |
| `RequestIdSchema` | unión de `string` \| `number` entero |
| `JSONRPCRequestSchema` | `jsonrpc` (literal "2.0"), `id`, + campos de `RequestSchema` — **estricto** |
| `JSONRPCNotificationSchema` | `jsonrpc`, + campos de `NotificationSchema` — **estricto** (sin `id`) |
| `JSONRPCResultResponseSchema` | `jsonrpc`, `id`, `result` — **estricto** |
| `JSONRPCErrorResponseSchema` | `jsonrpc`, `id` (opcional), `error` = { `code` (int), `message` (string), `data` (opcional) } — **estricto** |

`daemon.cjs:14907` — códigos de error propios (además de los JSON-RPC estándar):
`-32601 MethodNotFound` · `-32602 InvalidParams` · `-32603 InternalError` ·
**`-32042 UrlElicitationRequired`**.

`UrlElicitationRequired` es el más interesante: es el código para "el agente necesita que el
usuario autorice algo (p. ej. una URL) antes de seguir". Valor copiable tal cual.

---

## 10.3 `params`, `_meta` y el handshake

`daemon.cjs:14905`:

| Esquema | Campos |
|---|---|
| `BaseRequestParamsSchema` | `_meta` (opcional) |
| `RequestMetaSchema` (laxo) | `progressToken` (opcional), `<RELATED_TASK_META_KEY>` (opcional) |
| `TaskMetadataSchema` | `ttl` (número, opcional) |
| `RelatedTaskMetadataSchema` | `taskId` (string) |
| `TaskAugmentedRequestParamsSchema` | `_meta` + `task` (opcional) |

`daemon.cjs:14995` — **handshake en `_meta`**. El cliente envía, dentro de `params._meta`:

| Clave | Contenido |
|---|---|
| `<META_PROTOCOL_VERSION>` | `STATELESS_MCP_PROTOCOL_VERSION` |
| `<META_CLIENT_CAPABILITIES>` | capacidades del cliente |
| `<META_CLIENT_INFO>` | identificación del cliente |

→ **Copiar tal cual**: es la forma de negociar versión y capacidades sin un método aparte.

---

## 10.4 Catálogo de métodos

### Métodos MCP estándar (medidos como literales)

| Método | Params | Resultado |
|---|---|---|
| `initialize` | (handshake en `_meta`) | `protocolVersion`, `capabilities`, `serverInfo`, `instructions` |
| `ping` | — | objeto vacío |
| `tools/list` | `params.cursor` (opcional, paginado) | `{ tools: ToolSchema[], nextCursor?, _meta? }` |
| `tools/call` | `name` (string), `arguments` (objeto, opcional) | `CallToolResult` |
| `resources/list` | paginado | `{ resources[], nextCursor? }` |
| `resources/read` | `uri` | `ResourceContents` |
| `resources/subscribe` | `uri` | — |
| `resources/unsubscribe` | `uri` | — |
| `prompts/list` | paginado | `{ prompts[], nextCursor? }` |
| `prompts/get` | `name`, `arguments?` | `{ description?, messages[] }` |
| `logging/setLevel` | `level` (enum) | — |
| `tasks/list` | paginado | `{ tasks[], nextCursor? }` |
| `tasks/get` | `taskId` | `TaskSchema` |
| `tasks/result` | `taskId` | payload de la tarea |
| `tasks/cancel` | `taskId` | estado de la tarea |
| `server/discover` | — | capacidades (extensión de Cursor) |

### Notificaciones

| Notificación | Contenido |
|---|---|
| `notifications/initialized` | — (el servidor responde `202`) |
| `notifications/cancelled` | identificador de la petición cancelada |
| `notifications/progress` | `progressToken` + avance |
| `notifications/message` | `level` (enum de logging), `logger?`, `data` |
| `notifications/tools/list_changed` | (params opcionales) |
| `notifications/prompts/list_changed` | (params opcionales) |
| `notifications/tasks/status` | `TaskSchema` como params |

### Enum de niveles de logging

`daemon.cjs:14920` — ocho valores, de menor a mayor severidad:
`debug` · `info` · `notice` · `warning` · `error` · `critical` · `alert` · `emergency`.

### ⚠️ Falsos positivos descartados

`tools/common`, `tools/shell`, `tools/grep`, `tools/read`, `tools/mcp` **NO son métodos**:
son *namespaces de logger* (`createLogger("tools/grep")`, `createLogger("tools/shell")`,
`createLogger("tools/common")`, `createLogger("tools/mcp")`, `createLogger("tools/read")`).
Verificado en `daemon.cjs:24158`, `:23205`, `:17976`, `:18786`, `:24262`.

---

## 10.5 Objetos del protocolo (campos literales)

`daemon.cjs:14908`, `:14911`, `:14912`, `:14919`.

### `ToolSchema`

| Campo | Tipo |
|---|---|
| `name` | string (de `BaseMetadataSchema`) |
| `title` | string, opcional |
| `icons` | array, opcional |
| `description` | string, opcional |
| `inputSchema` | objeto `{ type: "object", properties?, required? }` con campos extra permitidos |
| `outputSchema` | objeto igual que `inputSchema`, opcional |
| `annotations` | `ToolAnnotationsSchema`, opcional |
| `execution` | `ToolExecutionSchema`, opcional |
| `_meta` | objeto de valores desconocidos, opcional |

### `ToolAnnotationsSchema`

`title?` · `readOnlyHint?` (bool) · `destructiveHint?` (bool) · `idempotentHint?` (bool) ·
`openWorldHint?` (bool).

### `ToolExecutionSchema`

`taskSupport?` = `"required"` \| `"optional"` \| `"forbidden"`.
→ **Una tool declara si DEBE ejecutarse dentro de una task asíncrona.** Es la extensión que
permite "esta tool es larga, lánzala como task".

### `CallToolResult`

`content` (array, con valor por defecto **vacío**) · `structuredContent` (objeto, opcional) ·
`isError` (bool, opcional).
Variante de compatibilidad: `CompatibilityCallToolResultSchema` acepta además `toolResult`
(unknown).

### `CallToolRequestParams`

`name` (string, obligatorio) · `arguments` (objeto de valores desconocidos, opcional).
Sobre `TaskAugmentedRequestParamsSchema` → añade `task` (opcional).

### Paginación

`PaginatedRequestSchema` = `RequestSchema` + `params` (con `cursor`).
`PaginatedResultSchema` = `ResultSchema` + `nextCursor` (opcional).

### Tareas asíncronas

`daemon.cjs:14912`:

| Esquema | Campos |
|---|---|
| `TaskStatusSchema` (enum) | `working` · `input_required` · `completed` · `failed` · `cancelled` |
| `TaskSchema` | `taskId` (string), `status` (enum), `ttl` (número \| null), `createdAt` (string), `lastUpdatedAt` (string), `pollInterval?` (número), `statusMessage?` (string) |
| `CreateTaskResultSchema` | `ResultSchema` + `task` (`TaskSchema`) |

→ `input_required` es el estado que permite al agente **preguntar al usuario** desde una task
en curso. `pollInterval` dice al cliente cada cuánto preguntar. `ttl` acota la vida de la task.

### `ListChangedOptionsBaseSchema`

`autoRefresh` (bool, por defecto `true`) · `debounceMs` (entero ≥ 0, por defecto `300`).

---

## 10.6 Extensiones propias de Cursor (NO copiar literalmente)

| Extensión | Dónde | Qué es |
|---|---|---|
| `server/discover` | `daemon.cjs:14997` | método de negociación de capacidades, no estándar de MCP |
| `_meta` con `META_PROTOCOL_VERSION` / `META_CLIENT_CAPABILITIES` / `META_CLIENT_INFO` | `daemon.cjs:14995` | los nombres de clave son de Cursor; el *patrón* sí es copiable |
| `RELATED_TASK_META_KEY` en `_meta` | `daemon.cjs:14905` | une una petición a una task en curso |
| `execution.taskSupport` | `daemon.cjs:14919` | extensión de MCP, síTJ reasonable adoptarla |
| Envoltura de llamada con `deadlineEpochSeconds` / `startedAtEpochSeconds` / `timeoutSeconds` / `sessionId` / `requestID` / `mode` | `daemon.cjs:13924` | **deadline-first**: copiable el patrón |

---

## 10.7 Presupuesto de reintentos (medido, copiable)

`daemon.cjs:3647`:

| Constante | Valor | Significado |
|---|---|---|
| `AGENT_RETRY_CAPS.serverCap` | 3 | tope de reintentos atribuibles al servidor |
| `AGENT_RETRY_CAPS.transportCap` | 10 | tope de reintentos de transporte |
| `STALL_RETRY_BUDGET_MS` | 180 000 (3 min) | presupuesto total de un atasco |
| `RATE_LIMIT_RETRY_BUDGET_MS` | 300 000 (5 min) | presupuesto total de un 429 |
| `MAX_RETRIES_CEILING` | 100 | tope duro |

`RETRY_BUDGET_BY_KIND` mapea cada tipo de fallo a un responsable:
`alpn_failure` → server · `poll_seqno_gap` → server · `append_fatal` → by_signature ·
`http_464` → server · `http_404_unimplemented` → server · `socket_reset` → transport ·
`stream_closed_early` → server · `idle_deadline` → server · `stall` → transport · `other` → server.

→ **Copiar tal cual**: dos tapas (servidor / transporte), presupuestos temporales por tipo de
fallo y "de quién es la culpa" explícito. Es de lo mejor que tiene el daemon.

---

## 10.8 El modelo de permisos por modo (hallazgo lateral)

`daemon.cjs:17976` — literales medidos:
- `ASK_MODE_MODEL_ERROR` = `"You are in ask mode and cannot run non read-only tools. Ask the user to switch to agent mode if edits are required."`
- `READONLY_PERMISSIONS_MO…` (constante de permisos de solo lectura, truncada en el bundle).
- `SAME_STEP_FILE_WRITER_TOOLS` = conjunto de tools que **escriben ficheros** (empieza por
  `ApplyPatch`, `AwaitShell`, `CopyToBox`, `Edi…`), relevante para decidir cuándo disparar
  un hook `afterFileEdit`.

→ El **modo Ask** es un modo de solo lectura con un mensaje al modelo explícito. El contrato de
`custom modes` (`agent.v1.CustomSubagent`) + este mensaje cubren el caso "modo restrictivo".

---

## 10.9 Qué tiene ya MegaDaemon y qué falta

`rust-core/src/mcp_server.rs` (460 líneas) **ya es** un servidor MCP sobre stdio con JSON-RPC 2.0.
Hoy implementa: `initialize` (con `protocolVersion`, `capabilities`, `serverInfo`, `instructions`),
`ping`, `tools/list`, `tools/call`, `resources/list`, `prompts/list`, y las notificaciones
`notifications/initialized` / `notifications/cancelled`. Errores con `-32601` para método
desconocido.

| Pieza del contrato | ¿Está en MD? | Acción |
|---|---|---|
| Sobre JSON-RPC + `RequestId` string\|int | Sí | — |
| `initialize` con `capabilities`/`serverInfo` | Sí | — |
| `tools/list` paginado (`nextCursor`) | No | añadir |
| `tools/call` con `content` / `structuredContent` / `isError` | Parcial | añadir `structuredContent` + `isError` |
| `outputSchema` en `ToolSchema` | No | añadir |
| `annotations` (`readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint`) | No | añadir |
| `execution.taskSupport` | No | añadir |
| Tareas asíncronas (`tasks/*`, `TaskSchema`, `input_required`) | No | añadir |
| `notifications/progress` + `progressToken` | No | añadir |
| `logging/setLevel` + `notifications/message` | No | añadir |
| Transporte HTTP stateless (hoy solo stdio) | No | añadir (con `--backend-url`-style) |
| Deadline en el mensaje | No | añadir |
| Presupuestos de reintento | No | **fichero nuevo `retry.rs`** |
| `_meta` con versión/capacidades/cliente | No | añadir |

---

## 10.10 Valoración para MegaDaemon

| Aspecto | Valoración | Motivo |
|---|---|---|
| Sobre JSON-RPC 2.0 + MCP como transporte del daemon | **COPIABLE tal cual** | MCP ya es estándar |
| `TaskSchema` + `tasks/*` + `input_required` | **COPIABLE tal cual** | El estado `input_required` resuelve "el agente necesita al usuario en mitad de un trabajo largo" |
| `execution.taskSupport` | **COPIABLE tal cual** | Declara si la tool es larga |
| `ToolAnnotationsSchema` (5 hints) | **COPIABLE tal cual** | Es MCP estándar y permite políticas por anotación |
| `outputSchema` | **COPIABLE tal cual** | Permite validar la salida de la tool |
| `logging/setLevel` + `notifications/message` | **COPIABLE tal cual** | Útil para un daemon con varios niveles de log |
| `UrlElicitationRequired = -32042` | **COPIABLE tal cual** | Código de la serie de implementación, estándar de facto |
| Deadline-first en la envoltura | **COPIABLE tal cual** | El deadline viaja; no hay ambigüedad de reloj |
| Presupuestos de reintento | **COPIABLE tal cual** | Pensado; ahorramanship |
| `_meta` con protocolo/capacidades/cliente | **ADAPTABLE** | El patrón sí; los nombres de clave son de Cursor |
| `server/discover` | **NO COPIABLE** | Extensión privativa |
| `RELATED_TASK_META_KEY` | **ADAPTABLE** | El concepto sí; la clave es de Cursor |
| `SAME_STEP_FILE_WRITER_TOOLS` (nombre del set) | **NO COPIABLE** | El *concepto* (qué tools escriben) es copiable; la lista, no |

**Destino en MegaDaemon**: `rust-core/src/mcp_server.rs` (todo lo anterior),
`rust-core/src/protocol.rs` (envelopas de task y deadline),
**nuevo** `rust-core/src/retry.rs` (presupuestos de reintento).
