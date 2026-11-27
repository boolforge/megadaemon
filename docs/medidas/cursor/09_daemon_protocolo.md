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
> [recortado: 13 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
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
> [recortado: 5 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
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
> [recortado: 7 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```

### `environment-file.d.ts` — contrato de variables de entorno (seguro)

```ts
> [recortado: 14 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
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
> [recortado: 6 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```

### Mensajes

`daemon.cjs:14906` — esquemas Zod exactos:
```js
RequestIdSchema = union([string(), number().int()])
> [recortado: 6 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
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
> [recortado: 9 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
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
> [recortado: 4 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```
→ Los clientes/daemons se identifican en `_meta` con `protocolVersion`, `capabilities`, `clientInfo`.
**Es el handshake de MCP**, pero con un campo `_meta` propio de Cursor.

---

## 9.3 El transporte de salida (`tools/call`)

`daemon.cjs:13924` — el daemon habla con un RPC de computer-use (macOS) por el mismo formato:
```js
const response = await this.sendWithRelaunchRetry(ctx,()=>{
> [recortado: 15 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```

→ **Overhead de sesión con deadlines explícitos** (`timeoutSeconds`, `startedAtEpochSeconds`,
`deadlineEpochSeconds`, `sessionId`, `requestID`). Es el patrón de "deadline-first" que
MegaDaemon debería adoptar: el deadline viaja en el mensaje, no se recalcula en el receptor.

---

## 9.4 Retry / presupuesto de reintentos

`daemon.cjs:3647` — constantes literales:
```js
AGENT_RETRY_CAPS = {serverCap:3, transportCap:10};
> [recortado: 9 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
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
> [recortado: 16 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
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
