# CONTRATO 9 — EL "AGENT HOST DAEMON": PROTOCOLO LOCAL (JSON-RPC / MCP stateless)

Este es **el contrato más copiable de todo el paquete**: un daemon Node que
implementa un **servidor MCP stateless sobre JSON-RPC 2.0** y habla con el backend de Cursor.

Ruta: `extensions/cursor-agent-host/dist/agent-host-daemon/dist/`
Binario: `dist/bin/daemon.cjs` (23 MB, bundle; **los ficheros de arranque son fuente legible**).

Ficheros **fuente legible** (con doc comments — atypicalo en un bundle):
`dist/bin/daemon.js`, `dist/bin/backend-endpoint.js` + `.d.ts`,
`dist/bin/environment-file.js` + `.d.ts`, `dist/bin/orbit-build-guard.js`,
`dist/bin/data-sink-at-start.js`, `dist/bin/client-machine-id-at-start.js`,
`dist/bin/stdio-errors-at-start.js`.
Copiados en `contratos/ficheros/`.

---

## 9.1 Cómo se arranca

`dist/bin/daemon.js`:
```js
#!/usr/bin/env node
import "./stdio-errors-at-start.js";
import "./environment-file-at-start.js";
import "./backend-endpoint-at-start.js";
import "./orbit-build-guard.js";
import "./data-sink-at-start.js";
import "./client-machine-id-at-start.js";
import { runDaemonProcess } from "../process/run-process.js";
...
delete process.env.NODE_COMPILE_CACHE;
process.env.NODE_DISABLE_COMPILE_CACHE = "1";
process.stderr.write(`agent-host-daemon entered at ${enteredAtMs.toFixed(1)}ms\n`);
void runDaemonProcess(process.argv.slice(2)).then((exitCode)=>{ if(exitCode!==0) process.exit(exitCode); });
```

### Flags y variables

| Flag / variable | Fichero | Significado |
|---|---|---|
| `--backend-url <URL>` | `backend-endpoint.js` | backend alternativo; **se rechaza si se da dos veces** |
| `CURSOR_API_ENDPOINT` | `backend-endpoint.js` | variable de entorno con el backend |
| `--environment-file <path>` | `environment-file.js` | fichero JSON de variables de entorno para los hijos |

`backend-endpoint.js` (fuente legible, literal):
```js
export const BACKEND_URL_FLAG = "--backend-url";
/** The variable every part of the daemon reads its backend from. */
export const BACKEND_ENDPOINT_VARIABLE = "CURSOR_API_ENDPOINT";
/** The backend a daemon with none named talks to. */
const DEFAULT_BACKEND_ENDPOINT = "https://api2.cursor.sh";
```

Y la política de seguridad (**patrón muy copiable**), literal:
```js
case"embedded_credentials": return new BackendEndpointRefusedError(`${named} carries a user name or a password`);
case"insecure_scheme":     return new BackendEndpointRefusedError(`${named} names the Cursor API host ${hostOf(url)} without https`);
case"not_cursor_origin":   return new BackendEndpointRefusedError(`${named} names ${hostOf(url)}, which is not one of Cursor's API hosts; a packaged daemon takes any other backend only from ${BACKEND_URL_FLAG} on its command line`);
```
Reglas: (1) el flag, una sola vez, con URL; (2) en build empaquetado la variable de entorno
solo se acepta si es un origen permitido por la regla de credenciales compartida; (3) nunca
credenciales embebidas; (4) nunca http://; (5) nunca un host ajeno.

`backend-endpoint.d.ts` — contrato de resolución:
```ts
export interface AppliedBackendEndpoint {
    /** Where the backend came from: the launcher's flag, the environment's
     *  variable, or neither, which leaves the daemon on its default. */
    readonly source: "flag" | "environment" | "default";
    /** The backend the daemon talks to. */
    readonly endpoint: string;
}
```

### `environment-file.d.ts` — contrato de variables de entorno (seguro)

```ts
/** The only file names the daemon reads. The name is checked before the file is
 *  opened, so a mistyped path to an SSH key or a credentials file is refused
 *  without being read or deleted. */
export declare const ENVIRONMENT_FILE_NAME: RegExp;
/** Names outside this are skipped: Bash's exported functions (`BASH_FUNC_x%%`) and the like. */
export declare const PORTABLE_VARIABLE_NAME: RegExp;
/** Read by Node, libuv or OpenSSL, wholly or in part, only while a process starts.
 *  From the file they reach the daemon's children but not the daemon. */
export declare const STARTUP_VARIABLES: ReadonlySet<string>;

export declare function takeEnvironmentFile(path: string, options?: TakeEnvironmentFileOptions): EnvironmentFileContents;
export declare function applyEnvironmentFile(argv: readonly string[], env: Record<string,string|undefined>, options?: TakeEnvironmentFileOptions): AppliedEnvironmentFile | undefined;
export declare function openVerifiedEnvironmentFile(path: string, linkStats: Stats, uid: number): number;
export declare function deleteOpenFile(path: string, fd: number): void;
```

Invariantes (del doc comment):
> *"The file is a JSON object of variable names to string values. It must be named like
> ENVIRONMENT_FILE_NAME, and be a regular file owned by this user with mode 0600 and one link.
> Any refusal leaves the file where it is, for the launcher to delete: the daemon deletes only
> a file it has parsed."*

> *"Deletes the file while its descriptor is still open, then checks that no name is left:
> a copy renamed away or linked after the checks would survive."*

→ **Patrón de "consume-and-destroy" de secretos**: parsear, aplicar a los hijos, borrar
sin dejar rastro, con defenses contra symlink/FIFO/TOCTOU. **Altamente copiable** para
MegaDaemon (que también reparte claves a workers).

---

## 9.2 El protocolo: JSON-RPC 2.0 + MCP

### Transporte

`daemon.cjs:14993` — el daemon implementa un endpoint **stateless** (Streamable HTTP MCP):
```js
function readJsonRpcPost(input2,init,endpoint){
  if((init?.method??"GET").toUpperCase()!=="POST"||typeof init?.body!=="string"||!targetsEndpoint(input2,endpoint))return void 0;
  let parsed;try{parsed=JSON.parse(init.body)}catch{return void 0}
  if(!isObject4(parsed)||parsed["jsonrpc"]!=="2.0")return void 0;
  const method=typeof parsed["method"]==="string"?parsed["method"]:void 0;
  return{message:parsed,method,isRequest:method!==void 0&&"id"in parsed}}
```

### Mensajes

`daemon.cjs:14906` — esquemas Zod exactos:
```js
RequestIdSchema = union([string(), number().int()])
JSONRPCRequestSchema = object({jsonrpc:literal(JSONRPC_VERSION), id:RequestIdSchema, ...RequestSchema.shape}).strict()
JSONRPCNotificationSchema = object({jsonrpc:literal(JSONRPC_VERSION), ...NotificationSchema.shape}).strict()
JSONRPCResultResponseSchema = object({jsonrpc:literal(JSONRPC_VERSION), id:RequestIdSchema, result:ResultSchema}).strict()
JSONRPCErrorResponseSchema = object({jsonrpc:literal(JSONRPC_VERSION), id:RequestIdSchema.optional(),
  error:object({code:number().int(), message:string(), data:unknown().optional()})}).strict()
```

### Códigos de error JSON-RPC propios

`daemon.cjs:14907`:
```js
ErrorCode2[ErrorCode2["UrlElicitationRequired"]=-32042]="UrlElicitationRequired"
```
→ **`-32042 UrlElicitationRequired`**: error de la serie -32000 (implementación-defined) para
"el agente necesita que el usuario autorice una URL". Muy útil: un daemon puede pedir
autorización sin inventar un código.

### Métodos MCP implementados

Extraídos del bundle (`daemon.cjs`):

```
initialize            notifications/initialized    notifications/cancelled
ping                  tools/list                   tools/call
tools/common          tools/grep                   tools/mcp
tools/read            tools/shell
resources/list        resources/read               resources/subscribe
resources/unsubscribe prompts/list                 prompts/get
server/discover       notifications/progress        notifications/message
tasks/list            tasks/get                    tasks/result
tasks/cancel
```

Citados:
- `daemon.cjs:14997` — `if(method==="notifications/initialized"){return new Response(null,{status:202})}`,
  `if(post.isRequest&&method==="initialize"){const discover={jsonrpc:"2.0",id:message["id"],method:"server/discover",params:{}};...}`
  → **`server/discover`**: un método **no estándar de MCP** que Cursor usa para negociar capacidades.
- `daemon.cjs:14919` — `ListToolsRequestSchema=PaginatedRequestSchema.extend({method:literal("tools/list")})`
- `daemon.cjs:14919` — `CallToolRequestSchema=RequestSchema.extend({method:literal("tools/call"),params:CallToolRequestParamsSchema})`
- `daemon.cjs:14919` — `ToolListChangedNotificationSchema=NotificationSchema.extend({method:literal("notifications/tools/list_changed"),...})`
- `daemon.cjs:14919` — `PromptListChangedNotificationSchema=NotificationSchema.extend({method:literal("notifications/prompts/list_changed"),...})`
- `daemon.cjs:14918` — `LoggingLevelSchema=_enum(["debug","info","notice","warning", ...])`

### Extensión de tareas (`tasks/*`) — no estándar

El daemon soporta un modelo de **tareas asíncronas** encima de JSON-RPC:
- `TaskAugmentedRequestParamsSchema` — añade `task` a los params.
- `notifications/progress` con `progressToken`.
- `tasks/list`, `tasks/get`, `tasks/result`, `tasks/cancel`.
- `ToolExecutionSchema.taskSupport: "required" | "optional" | "forbidden"` — **una tool declara si DEBE ir en una task**.

→ **Muy copiable**: es la forma de sacar workloads largos del request/response síncrono.
MegaDaemon ya tiene `protocol.rs`; esto encaja como un `TaskEnvelope`.

### `_meta` y protocolo versionado

`daemon.cjs:14995`:
```js
bodyMessage.params = {...params, _meta:{...meta3,
  [META_PROTOCOL_VERSION]: STATELESS_MCP_PROTOCOL_VERSION,
  [META_CLIENT_CAPABILITIES]: hello.capabilities,
  [META_CLIENT_INFO]: hello.clientInfo}}
```
→ Los clientes/daemons se identifican en `_meta` con `protocolVersion`, `capabilities`, `clientInfo`.
**Es el handshake de MCP**, pero con un campo `_meta` propio de Cursor.

---

## 9.3 El transporte de salida (`tools/call`)

`daemon.cjs:13924` — el daemon habla con un RPC de computer-use (macOS) por el mismo formato:
```js
const response = await this.sendWithRelaunchRetry(ctx,()=>{
  const startedAtEpochSeconds = Date.now()/1e3;
  const sessionId = this.activeSessionId;
  return{
    id: requestID,
    method:"tools/call",
    requestID: randomUUID(),
    name: name17,
    arguments: args,
    timeoutSeconds: timeoutMs/1e3,
    startedAtEpochSeconds,
    deadlineEpochSeconds: startedAtEpochSeconds + timeoutMs/1e3,
    mode: this.mode,
    ...(sessionId!==void 0&&{sessionId})
  }}, name17);
```

→ **Overhead de sesión con deadlines explícitos** (`timeoutSeconds`, `startedAtEpochSeconds`,
`deadlineEpochSeconds`, `sessionId`, `requestID`). Es el patrón de "deadline-first" que
MegaDaemon debería adoptar: el deadline viaja en el mensaje, no se recalcula en el receptor.

---

## 9.4 Retry / presupuesto de reintentos

`daemon.cjs:3647` — constantes literales:
```js
AGENT_RETRY_CAPS = {serverCap:3, transportCap:10};
AGENT_RETRY_TRANSPORT_CAP_IN_TEST = 2;
STALL_RETRY_BUDGET_MS = 3*60*1e3;      // 3 min
RATE_LIMIT_RETRY_BUDGET_MS = 5*60*1e3; // 5 min
RETRY_BUDGET_BY_KIND = {
  alpn_failure:"server", poll_seqno_gap:"server", append_fatal:"by_signature",
  http_464:"server", http_404_unimplemented:"server", socket_reset:"transport",
  stream_closed_early:"server", idle_deadline:"server", stall:"transport", other:"server"}
MAX_RETRIES_CEILING = 100
```

→ **Copiar tal cual**: dos tapas (server / transport), presupuestos temporales por tipo de fallo,
y mapeo explícito "qué fallo es culpa de quién". Es exactamente lo que falta en muchos daemons.

---

## 9.5 El backend: `api2.cursor.sh` y el proto `agent.v1.`

### Backend

`backend-endpoint.js`: `DEFAULT_BACKEND_ENDPOINT = "https://api2.cursor.sh"`.
Los comandos del daemon llegan al backend por **ConnectRPC** (`@connectrpc/connect@1.6.1`,
`daemon.cjs:1010-1015`), no por REST plano.

### Proto `agent.v1.` — el contrato de "contexto del agente"

`cursor-mcp/dist/main.js:212`, `RequestContext`:
```
RequestContext|2 rules #0*|4 env #1|6 repository_info #2*|7 tools #3*|
8 conversation_notes_listing 9?|9 shared_notes_listing 9?|11 git_repos #4*|
13 project_layouts #5*|14 mcp_instructions #6*|15 debug_mode_config 7?|
16 cloud_rule 9?|17 web_search_enabled 8?|18 skill_options 8?|
19 repository_info_should_query_prod 8?|20 file_contents 9,9|21 user_intent_summary 9?|
22 custom_subagents #9*|23 mcp_file_system_options 10?|24 web_fetch_enabled 8?|
25 hooks_additional_context 9?|26 commit_attribution_message 9?|27 pr_attribution_message 9?|
28 hooks_config #11?|29 agent_skills #12*|30 precomputed_human_changes #13*|
31 recently_added_plugin #14?|32 supports_mcp_auth 8?|33 git_repo_info_complete 8?|
34 mcp_meta_tool_options 15?|35 read_lints_enabled 8?|36 mcp_info_complete 8?|
37 non_file_rules #0*|38 matched_installed_plugin #16?|39 rules_info_complete 8?|
40 env_info_complete 8?|41 repository_info_complete 8?|42 custom_subagents_info_complete 8?|
43 agent_skills_info_complete 8?|44 mcp_file_system_info_complete 8?|45 git_status_info_complete 8?|
46 user_permissions_auto_run #17?|47 project_permissions_auto_run #17?|48 admin_permissions_auto_run #17?|
49 disabled_team_rules 9*|50 search_conversations_enabled 8?|51 send_message_enabled 8?|
52 admin_command_denylist 9*|53 system_prompt_override #18?
```

**Esto ES el contrato de "qué entra al prompt"** (punto 6 del encargo). C.campos clave:
`rules`, `env`, `repository_info`, `tools`, `git_repos`, `project_layouts`, `mcp_instructions`,
`file_contents` (map), `user_intent_summary`, `custom_subagents`, `agent_skills`,
`hooks_config`, `hooks_additional_context`, `non_file_rules`, `skill_options`,
`user/project/admin_permissions_auto_run`, `disabled_team_rules`, `admin_command_denylist`,
`system_prompt_override`.

Los `#N` son índices de tipos anidados (1-rules, 2-env, …). Los `8?`/`9?` son opcionales.

### `system_prompt_override` — el override de prompt del sistema

`RequestContext` incluye `system_prompt_override` (campo 53). Eso significa que **el cliente
puede sustituir el system prompt del backend**. Interesante: el prompt del sistema **no** es
un contrato fijo; hay un punto de extensión. **El contenido concreto del prompt del servidor
NO MEDIDO** (no está en el paquete).

---

## 9.6 Lo que el daemon NO tiene (NO MEDIDO)

- No hay binario `cursor-agent` en el `.deb`: `usr/share/cursor/bin/` solo contiene
  `cursor`, `cursor-tunnel`, `code-tunnel`. El CLI `cursor-agent` se distribuye aparte
  (no viene en el paquete de escritorio). **Por tanto el "protocolo JSON del CLI cursor-agent"
  NO se puede extraer de aquí.**
- El prompt del sistema del agente: vive en el backend.
- Los esquemas de entrada de las tools nativas: viven en el backend.
- La lógica de `apply_agent_diff`, `reflect`, `await`, `start_grind_*`: en el backend.

---

## 9.7 Valoración para MegaDaemon

| Aspecto | Valoración | Motivo |
|---|---|---|
| JSON-RPC 2.0 sobre stdio/HTTP + MCP como transporte del daemon | **COPIABLE tal cual** | MCP ya es estándar; el daemon es un servidor MCP. |
| `ToolSchema` / `CallToolResultSchema` / `ListToolsResultSchema` | **COPIABLE tal cual** | MCP estándar + 2 extensiones (`annotations`, `execution.taskSupport`). |
| Tareas asíncronas (`tasks/*`, `taskSupport`, `progressToken`) | **COPIABLE tal cual** | Encaja con el `protocol.rs` de MegaDaemon. |
| `_meta` con `protocolVersion`/`capabilities`/`clientInfo` | **COPIABLE tal cual** | Es el handshake MCP normalizado. |
| Deadline en el mensaje (`deadlineEpochSeconds`, `timeoutSeconds`) | **COPIABLE tal cual** | Deadline-first, sin ambigüedad de reloj. |
| Retry budgets (`serverCap`/`transportCap`/`RETRY_BUDGET_BY_KIND`/`MAX_RETRIES_CEILING`) | **COPIABLE tal cual** | Pensado; ahorramanship. |
| `environment-file` (consume-and-destroy, 0600, 1 link, anti-TOCTOU) | **COPIABLE tal cual** | Es un patrón de seguridad, no de producto. |
| `backend-endpoint` (flag único, origen permitido, sin credenciales, https) | **ADAPTABLE** | La política de orígenes es de Cursor; el *patrón* (resolución con `source: flag|environment|default`) es copiable. |
| `server/discover` (método no estándar) | **NO COPIABLE** | Extensión privativa. |
| `UrlElicitationRequired = -32042` | **COPIABLE tal cual** | Código JSON-RPC de la serie de implementación; estándar de facto para "el usuario debe autorizar". |
| El proto `agent.v1.RequestContext` | **ADAPTABLE (muy valioso)** | Es la lista de lo que entra al prompt. Copiar los **campos**; el wire format protobuf no. |
| `system_prompt_override` | **ADAPTABLE** | El punto de extensión es valuable; el contenido no. |
| `cursor-agent` CLI y su salida JSON | **NO MEDIDO** | El binario no está en el paquete. |

**Destino en MegaDaemon**: `rust-core/src/protocol.rs` (JSON-RPC + envelopes de task),
`rust-core/src/mcp_server.rs` (servidor MCP stateless), `rust-core/src/agent_core.rs`
(`RequestContext` como contexto de prompt), nuevo `rust-core/src/retry.rs` (presupuestos).
