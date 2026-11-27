# CONTRATO 4 — MCP (Model Context Protocol)

Fuente: `usr/share/cursor/resources/app/extensions/cursor-mcp/dist/main.js` (1,6 MB, minificado).
Cita literal de la línea **212** (el fichero tiene 213 líneas, minificado a una línea enorme).

---

## 4.1 Esquema de `mcp.json` (Zod, literal)

`cursor-mcp/dist/main.js:212`:

```js
const Ga=Aa({type:La(literal("stdio")),
  command:Pa(),
  args:La(Ra(Pa())),
  env:La(Ua(Pa(),Pa())),
  envFile:La(Pa()),
  cwd:La(Pa()),
  enabledTools:La(Ra(Pa())),
  startupTimeoutSec:La(number().check(...))});      // con .catch(default)

const Xa=Aa({type:La(Fa(["http","sse"])),
  url:Pa(),
  headers:La(Ua(Pa(),Pa())),
  auth:La(Va),
  tls:La(Ka),
  placement:La(Fa(["server","client"])),
  enabledTools:La(Ra(Pa()))});

const Qa=Ja([Ga,Xa]);            // union
Aa({mcpServers:Ua(Pa(),Qa)});     // Record<string, Server>
```

Traducido a JSON Schema:

```jsonc
{
  "mcpServers": {
    "<nombre>": {
      // ── variante stdio ──
      "type": "stdio",
      "command": "string",              // obligatorio
      "args": ["string"],               // opcional
      "env": { "<K>": "<V>" },          // opcional
      "envFile": "string",              // opcional
      "cwd": "string",                  // opcional
      "enabledTools": ["string"],       // opcional
      "startupTimeoutSec": 0            // opcional (número, con valor por defecto)

      // ── variante http/sse ──
      // "type": "http" | "sse",
      // "url": "string",                // obligatorio
      // "headers": { "<K>": "<V>" },    // opcional
      // "auth": { "CLIENT_ID": "string", "CLIENT_SECRET": "string?",
      //            "scopes": ["string"]?, "redirectUris": ["string"]? },
      // "tls": { "caBundle": "string" },
      // "placement": "server" | "client",
      // "enabledTools": ["string"]
    }
  }
}
```

### Tabla campo por campo

| Campo | Tipo | Variante | Notas |
|---|---|---|---|
| `type` | `"stdio"` \| `"http"` \| `"sse"` | ambas | **discriminante**; `stdio` exige `command`, `http`/`sse` exigen `url` |
| `command` | string | stdio | obligatorio |
| `args` | string[] | stdio | |
| `env` | Record<string,string> | stdio | |
| `envFile` | string | stdio | fichero con variables de entorno |
| `cwd` | string | stdio | |
| `enabledTools` | string[] | ambas | allowlist de tools del servidor |
| `startupTimeoutSec` | number | stdio | con `.catch()` → valor por defecto si falla la validación |
| `url` | string | http/sse | obligatorio |
| `headers` | Record<string,string> | http/sse | |
| `auth` | objeto OAuth | http/sse | ver 4.2 |
| `tls` | `{caBundle: string}` | http/sse | CA propia |
| `placement` | `"server"` \| `"client"` | http/sse | **dónde se ejecuta**: en los servidores de Cursor o en el cliente |

### OAuth (`auth`) — literal

`cursor-mcp/dist/main.js:212`:
```js
const Va=Aa({CLIENT_ID:Pa(),CLIENT_SECRET:La(Pa()),scopes:La(Ra(Pa())),redirectUris:La(Ra(Pa()))})
```
> Ojo: las claves OAuth van en **SCREAMING_SNAKE_CASE** (`CLIENT_ID`), mientras que el resto del formato MCP de Cursor usa camelCase. Es una inconsistencia real del producto, copiable pero conviene normalizarla.

### TLS (`tls`) — literal

```js
const Ka=(Ha={caBundle:Pa().check(Sa.WN(),Sa.m9(1),Sa.Eb(131072))}, new Na({type:"object",shape:Ha,catchall:$a(),...ba.A2(Ya)}))
```
→ `{caBundle: string}` con `catchall` (permite claves extra) y límites: `min(1)`, `max(131072)` (128 KiB).

---

## 4.2 Dónde vive el fichero y cómo se fusiona

| Ámbito | Ruta | Evidencia |
|---|---|---|
| Usuario | `~/.cursor/mcp.json` | `workbench.desktop.main.js:20266` — `"The copy of ${i} in ~/.cursor/mcp.json and the one on your ${Po} account"` |
| Proyecto | `<proyecto>/.cursor/mcp.json` | `workbench.desktop.main.js:20266` — `return e.projectPath!==void 0?\`${e.projectPath}/.cursor/mcp.json\`:void 0` |
| Cuenta (servidor) | API de cuenta | `workbench.desktop.main.js:20266` — `getMcpConfig({teamScope:!1,redactSecrets:!1})` |
| Equipo (servidor) | API de equipo | idem, con `teamScope` |
| Plugin | por plugin | `workbench.desktop.main.js:20266` — *"A plugin's entry for this URL decides where it runs."* |

### Modelo deplacement (dónde corre el servidor MCP)

Es el rasgo más interesante y **el más copiable**:

- `"placement": "client"` → corre en la máquina del usuario (local).
- `"placement": "server"` → corre en los servidores de Cursor (funciona en todos los dispositivos y en cloud agents).

Cita literal (`workbench.desktop.main.js:20266`):
```
"It will run on ${Po}'s servers, so it works on every computer you sign in on and in cloud agents."
"It is removed from ~/.cursor/mcp.json and keeps its headers, so there is nothing to sign in to again."
```

### Detección de conflicto usuario↔equipo↔plugin

`workbench.desktop.main.js:20266`, función `GJg` — campos que se comparan para detectar diferencias:
`url`, todas las claves de `headers`, `auth.CLIENT_ID`, `auth.CLIENT_SECRET`, `auth.scopes`.

Acciones ofrecidas al usuario (`workbench.desktop.main.js:20266`):
```js
var TZr="use-account-version", EZr="replace-account-version", IZr="keep-both", $Jg="cancel";
```
→ 4 resolutions: **usar la versión de la cuenta / reemplazar la de la cuenta / mantener ambas / cancelar**.

### Detección de cabecera de autorización

`workbench.desktop.main.js:20266`:
```js
function UJg(e){return Object.keys(e??{}).some(t=>t.toLowerCase()==="authorization")}
```
→ si `headers` tiene `authorization` (case-insensitive), el servidor **no necesita** login interactivo: *"there is nothing to sign in to again"*.

---

## 4.3 Transporte y política (proto `agent.v1.`)

`cursor-mcp/dist/main.js:212`:

```js
_w = jc(Mc.proto3, gw, "McpServerTransport",
  [[0,"UNSPECIFIED"],[1,"STDIO"],[2,"HTTP"]], 1)

vw = jc(Mc.proto3, gw, "McpPolicyBlockedReason",
  [[0,"UNSPECIFIED"],[1,"MCP_SERVERS_OFF"],[2,"MCP_ALLOWLIST"],
   [3,"CLOUD_AGENT_ALLOWLIST"],[4,"DENYLIST"],[5,"APPLIES_TO"],[6,"TOOL_ACCESS"]], 1)

ww = jc(Mc.proto3, gw, "McpRefreshPolicy",
  [[0,"UNSPECIFIED"],[1,"FORCE_IF_MISSING"],[2,"FORCE"]], 1)
```

> Nota: el proto tiene `STDIO` y `HTTP` (2 valores) mientras que el **fichero de config** tiene `stdio`/`http`/`sse` (3 valores). El `sse` es legacy solo del cliente.

### `McpArgs` (proto) — el sobre de una llamada a tool MCP

`cursor-mcp/dist/main.js:212`:
```js
class bw extends yw{constructor(e){super(),
  this.name="", this.args={}, this.toolCallId="", this.providerIdentifier="",
  this.toolName="", this.smartModeApprovalOnly=!1, this.skipApproval=!1,
  this.serverIdentifier="", this.symbolicPathArguments=[], this.argumentsRepaired=!1,
  this.refuseSavedSecrets=!1, ...}}
static $(){return["McpArgs|1 name 9|2 args 9,#0|3 tool_call_id 9|4 provider_identifier 9|
  5 tool_name 9|6 smart_mode_approval_only 8|7 skip_approval 8|8 server_identifier 9|
  9 symbolic_path_arguments 9*|10 arguments_repaired 8|11 refuse_saved_secrets 8"]}}
```

Campos notables:
- `symbolic_path_arguments` — argumentos de ruta **simbólicos** (el cliente las resuelve a rutas reales; permite que la ruta vivan en el servidor y no en el cliente → el daemon no necesita conocer el FS del usuario).
- `arguments_repaired` — marca que el cliente reparó argumentos inválidos.
- `refuse_saved_secrets` — no usar credenciales guardadas.
- `smart_mode_approval_only` — la llamada solo requiere aprobación en "smart mode".

### `McpTool` (proto)

`cursor-agent-host/.../daemon.cjs:3647`:
```js
function mcpToolDefinitionsToProto(mcpTools){
  return new McpTools({mcpTools:mcpTools.map(tool=>({
    name:tool.name, providerIdentifier:tool.providerIdentifier, toolName:tool.toolName,
    description:tool.description,
    inputSchema:tool.inputSchema?Value.fromJson(tool.inputSchema):void 0,
    outputSchemaJson:tool.outputSchema!==void 0?JSON.stringify(tool.outputSchema):void 0}))})}
```

---

## 4.4 Allowlist / permisos MCP (JSON Schema, copiable literal)

`extensions/cursor-always-local/schemas/permissions.schema.json` (1839 B) — **copiado tal cual** en
`contratos/ficheros/permissions.schema.json`:

```jsonc
{
  "mcpAllowlist": ["<server>:<tool>", "<server>:*", "*:<tool>", "*:*"],
  "terminalAllowlist": ["<patrón de comando>"],
  "approvalMode": "allowlist" | "unrestricted" | "manual",
  "autoRun": {
    "allow_instructions": ["..."],
    "block_instructions": ["..."]
  },
  "autoReview": {
    "allow_instructions": ["..."],
    "block_instructions": ["..."]
  }
}
```

Cita literal del patrón de allowlist:
> `"MCP tools that can run automatically. Entries use \`server:tool\`, \`server:*\`, \*:tool\`, or \`*:*\`."`

Patrón regex del ítem:
```json
"pattern": "^[^:*]+:[^:*]+$|^[^:*]+:\\*$|^\\*:[^:*]+$|^\\*:\\*$"
```

`approvalMode`:
> `"Controls how Cursor approves tool calls. \`manual\` maps to Ask Every Time."`

---

## 4.5 Allowlist de servidores MCP por entorno (JSON Schema)

`extensions/cursor-always-local/schemas/environment.schema.json` (8909 B) — copiado en
`contratos/ficheros/environment.schema.json`. Extracto relevante:

```jsonc
"disableAllMcpServers": { "type": "boolean",
  "description": "When true, block every user and team MCP server for agents using this environment. Cursor built-in MCP servers are never restricted. An empty mcpServerAllowlist alone does not disable MCP." },

"mcpServerAllowlist": [{
  "name": "string",
  "serverUrl": "string",     // HTTP MCP, por patrón de URL/dominio
  "command": "string",       // stdio MCP, por patrón de comando
  "toolAllowlist": ["string"]
}],
```
Con `anyOf: [{required:["serverUrl"]}, {required:["command"]}]` y `additionalProperties: false`.

El mismo schema trae la política de egreso de red (útil para MegaDaemon):
```jsonc
"egressAllowlist": ["registry.npmjs.org"],
"egressMode": "allow_all" | "parent_plus_network_settings" | "default_with_network_settings" | "network_settings_only"
```

---

## 4.6 Valoración para MegaDaemon

| Aspecto | Valoración | Motivo |
|---|---|---|
| Esquema `mcp.json` (stdio + http/sse) | **COPIABLE tal cual** | Es Zod plano, ya casi el MCP de facto de la industria. Añadir `placement` es la única extensión propia. |
| `envFile` / `env` / `cwd` / `startupTimeoutSec` | **COPIABLE tal cual** | Campos estándar y útiles. |
| `placement: server \| client` | **COPIABLE (con matiz)** | La idea (el MCP corre en el servidor y no en el cliente) es el avance clave; en MegaDaemon el equivalente es "el MCP corre en el nodo worker". |
| Detección de conflicto + 4 resoluciones | **ADAPTABLE** | Lógica de UI; hay que Translate el diálogo a un flujo de API/CLI. |
| `McpArgs.symbolic_path_arguments` | **ADAPTABLE (muy valioso)** | Evita que el cliente mande rutas absolutas: el path se resuelve donde corre la tool. Reduce superficie de datos. |
| `McpPolicyBlockedReason` (7 valores) | **COPIABLE tal cual** | Taxonomía de rechazo útil y ya pensada. |
| `McpRefreshPolicy` (3 valores) | **COPIABLE tal cual** | Trivial y útil. |
| `permissions.schema.json` (allowlist + autoRun) | **COPIABLE tal cual** | Formato ya casi estándar (Claude Code usa `permissions.allow`). |
| `environment.schema.json` (allowlist de MCP + egress) | **ADAPTABLE** | Es un schema grande de *cloud dev environments*; a MegaDaemon le interesa el bloque `mcpServerAllowlist` + `egressMode`. |
| Servidores MCP por defecto del producto | **NO MEDIDO** | No hay MCP servers de terceros embebidos; solo `cursor-browser-automation` y `cursor-computer-use`, que son **MCP servers de Cursor** (`extensions/cursor-browser-automation/package.json`: *"MCP server for browser automation in {productName}"*). El resto se configura por el usuario. |

**Destino en MegaDaemon**: `rust-core/src/mcp_ext.rs` (esquema + allowlist + placement),
`rust-core/src/mcp_server.rs` (transporte), `rust-core/src/protocol.rs` (sobre `McpArgs`).
