# CONTRATO 4 — MCP (Model Context Protocol)

Fuente: `usr/share/cursor/resources/app/extensions/cursor-mcp/dist/main.js` (1,6 MB, minificado).
Cita literal de la línea **212** (el fichero tiene 213 líneas, minificado a una línea enorme).

---

## 4.1 Esquema de `mcp.json` (Zod, literal)

`cursor-mcp/dist/main.js:212`:

```js
const Ga=Aa({type:La(literal("stdio")),
> [recortado: 19 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```

Traducido a JSON Schema:

```jsonc
> [recortado: 25 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
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
> [recortado: 9 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```

> Nota: el proto tiene `STDIO` y `HTTP` (2 valores) mientras que el **fichero de config** tiene `stdio`/`http`/`sse` (3 valores). El `sse` es legacy solo del cliente.

### `McpArgs` (proto) — el sobre de una llamada a tool MCP

`cursor-mcp/dist/main.js:212`:
```js
> [recortado: 8 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```

Campos notables:
- `symbolic_path_arguments` — argumentos de ruta **simbólicos** (el cliente las resuelve a rutas reales; permite que la ruta vivan en el servidor y no en el cliente → el daemon no necesita conocer el FS del usuario).
- `arguments_repaired` — marca que el cliente reparó argumentos inválidos.
- `refuse_saved_secrets` — no usar credenciales guardadas.
- `smart_mode_approval_only` — la llamada solo requiere aprobación en "smart mode".

### `McpTool` (proto)

`cursor-agent-host/.../daemon.cjs:3647`:
```js
> [recortado: 6 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```

---

## 4.4 Allowlist / permisos MCP (JSON Schema, copiable literal)

`extensions/cursor-always-local/schemas/permissions.schema.json` (1839 B) — **copiado tal cual** en
`contratos/ficheros/permissions.schema.json`:

```jsonc
> [recortado: 13 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
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
> [recortado: 9 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
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
