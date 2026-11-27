# CONTRATO 7 — PUENTE CON EL EXTERIOR: ENDPOINTS, AUTH, TELEMETRÍA, MODELOS, RED

Todos los hosts y rutas de este documento están **medidos** en el paquete (ruta:línea exactas).
Lo que **no** está en el paquete se marca explícitamente como NO MEDIDO.

---

## 7.1 Hosts (medidos)

| Host | Para qué | Evidencia |
|---|---|---|
| `https://api2.cursor.sh` | **Backend del agente** (chat, completion, tools, MCP) | `extensions/cursor-agent-host/dist/agent-host-daemon/dist/bin/backend-endpoint.js:6` — `const DEFAULT_BACKEND_ENDPOINT = "https://api2.cursor.sh";` |
| `https://api.cursor.com` | Backend de la app "molten" (Canvas/Grok/agents) | `molten/dist/renderer/assets/cursor-backend-ChqkVT9m.js:2` |
| `https://cursor.com` | Web, OAuth client metadata, docs | `cursor-agent-host/dist/main.js:213`; `cursor-mcp/dist/main.js:212`; `agent-host-daemon/.../daemon.cjs:717` |
| `https://www.cursor.com` | Web (assets,Privacy/TOS) | `molten/dist/renderer/assets/teams-DPBRQVc3.js:2` → `https://cursor.com/privacy`, `https://cursor.com/terms-of-service` |
| `https://downloads.cursor.com` | **Actualizaciones + harness del agente** | `agent-host-daemon/.../build-info.json` → `"agentHarnessCdnUrl":"https://downloads.cursor.com/agent-host-harness/"`; `daemon.cjs:4` → `https://downloads.cursor.com/agent-host-harness/` |
| `https://console.statsig.com`, `https://api.statsigcdn.com` | **Feature flags (Statsig)** | `workbench.desktop.main.js` (harvest de URLs) |
| `https://cursor.com/agents/mcp/oauth/callback` | **Callback OAuth de MCP** | `molten/dist/engine/engine.cjs:26`; `molten/dist/renderer/assets/boot-adA7W7PA.js:3` |
| `https://cursor.com/docs/skills#frontmatter-fields` | Docs de frontmatter de skills | `molten/dist/engine/engine.cjs:1722` |
| `https://cursor.com/docs/models` | Docs de modelos | `molten/dist/renderer/assets/glass-model-picker-DIQ_38mA.js:2` — `var Ne=\`https://cursor.com/docs/models\`` |
| `https://cursor.com/docs/approval-agents#approval-policy-files` | Docs de hooks/policies de aprobación | `molten/dist/renderer/assets/lazy-entry-BmMO3zBp.js:3` |
| `https://cursor.com/docs/bugbot#usage-based-billing` | Docs de billing por uso | `molten/dist/renderer/assets/page-lznzVes8.js:2` |
| `http://127.0.0.1`, `http://localhost` | Servicios locales (MCP local, bridges) | harvest |
| `http://169.254.169.254`, `http://169.254.170.2` | **IMDS de nube** (metadatos de instancia) | harvest de extensiones |

---

## 7.2 Endpoints de autenticación (medidos)

`extensions/cursor-agent-host/dist/agent-host-daemon/dist/bin/daemon.cjs`:

| Endpoint | Método | Línea | Nota |
|---|---|---|---|
| `${baseUrl}/auth/login_intent` | POST | 22 | cuerpo JSON; cabeceras `createLocalCliModeHeaders({"Content-Type":"application/json"})` |
| `${baseUrl}/auth/exchange_user_api_key` | POST | 23 | `Authorization: Bearer ${apiKey}`; si devuelve **403** → `isSignInPolicyViolation(body)` |
| `${baseUrl}/auth/poll` | POST | 27 | polling del login (device-code style) |
| `${baseUrl}/auth/login` | — | 660 | |
| `${baseUrl}/auth/credential` | — | 5 | |
| `${baseUrl}/auth/daemon` | — | 27346 | |

Cita literal (línea 23):
```js
const response = await authFetch(`${baseUrl}/auth/exchange_user_api_key`,
  {method:"POST",headers,body:JSON.stringify({})});
if(response.status===403){
  const body = await response.json().catch(()=>void 0);
  if(isSignInPolicyViolation(body)){throw new SignInPolicyViolation...}
}
```
→ El flujo es: `login_intent` → (poll) → `exchange_user_api_key` con `Bearer <apiKey>`.
**NO MEDIDO**: el contenido del body de `login_intent`, el nombre exacto del campo de la api key
en la respuesta, y el refresh de tokens.

### Mecanismo general

- El backend se puede **redirigir** por `--backend-url` o `CURSOR_API_ENDPOINT`
  (ver `09_daemon_protocolo.md` §9.1) — útil para un entorno air-gapped con proxy propio.
- La regla de origen (`backend-endpoint.js`) es estricta: sin credenciales embebidas, solo https,
  solo hosts de Cursor (salvo con el flag explícito).

---

## 7.3 Telemetría: convenciones OTel GenAI (COPIABLE TAL CUAL)

`out/vs/workbench/workbench.desktop.main.js:23` — nombres de span/atributos literales:

```
ai.response.toolCalls
ai.prompt.messages
ai.prompt.tools
ai.model.id
ai.model.provider
ai.response.providerMetadata
ai.usage.cachedInputTokens
ai.telemetry.functionId
ai.usage.completionTokens
ai.usage.promptTokens
ai.toolCall.name
ai.toolCall.id
ai.toolCall.args
gen_ai.response.model
gen_ai.usage.input_tokens
gen_ai.usage.output_tokens
```

→ Son las **semantic conventions de OpenTelemetry para GenAI** (`ai.*` + `gen_ai.*`).
**Copiable tal cual** y directamente utilizable por MegaDaemon para instrumentar su agente.

### Otros servicios de telemetría

| Servicio | Medido en | Nota |
|---|---|---|
| **Sentry** | `workbench.desktop.main.js:4` (`_sentryDebugIds`), 650 hits | reporte de errores; el bundle lleva el SDK de Sentry con debug ids por build |
| **Statsig** | 242 hits; flags `client_statsig_poll_interval_ms`, `disable_statsig` (`workbench.desktop.main.js:494`) | feature flags |
| **Posthog** | solo aparece como **dominio en la allowlist de conectores** (línea 792) → no es el cliente de telemetría | |
| **Amplitude** | 4 hits, todos del parser SVG (falso positivo) | **NO** es el cliente de telemetría |

→ **NO hay cliente de telemetría de producto propio medido**: Cursor usa Sentry (errores) +
Statsig (flags) y presumably el backend para analítica de producto. El envío de eventos de
producto ("qué se usa") no está en el cliente.

### Redirección de telemetría

`workbench.desktop.main.js:494` — flag de cliente `disable_statsig` (bool).
→ El cliente puede desactivar Statsig. Útil para entornos sin salida a internet.

---

## 7.4 Modelos: cómo se elige el modelo

**NO MEDIDO el catálogo**: los ids de modelo **no son constantes del cliente**.

Evidencia:
- `molten/dist/renderer/assets/glass-model-picker-DIQ_38mA.js:2` — el model picker consume una
  lista que llega por RPC (`modelId`, `params`, `parameters`, `confirmationDialogue`).
- La única URL es `https://cursor.com/docs/models` (enlace externo, no catálogo).
- El harvest de literales con forma de id de modelo en los bundles solo devuelve
  `auto`, `composer-agent-workspace-paths-none`, `composer-ensemble`,
  `composer-thinking-hover-tooltip`, `gpt-3`, `gpt-4`, `o2`, `claude-3.5-sonnet`,
  `claude-code`, `claude-plugin` — es decir, **nombres internos o de test**, no el catálogo real.

### Mecanismo de selección (medido)

- El modelo es un **parámetro de la petición** (`RequestContext` / `McpArgs`), no una config local.
- Hay **preferencias de parámetros por modelo** persistidas:
  `glass-model-picker-DIQ_38mA.js:2` — `f(\`modelParameterPreferences\`)`, con `modelId` y `parameters`.
- Hay **diálogos de confirmación** por modelo (`confirmationDialogue` con `presentation`, `key`,
  `title`, `body`) → algunos modelos exigen un aviso extra.
- La UI ofrece **"Get More Usage"** → el billing es **por uso** (coherente con
  `cursor.com/docs/bugbot#usage-based-billing`).
- El modelo se puede **forzar por modo**: `CustomSubagent.model` + `force_default_model`
  (proto `agent.v1.`, ver `02_rules_skills_modes.md`).
- Los modos internos son `AGENT`, `ASK`, `EDIT`, `DEBUG` (literales en `cursor-mcp/dist/main.js:212`).

---

## 7.5 Política de red para MCP (COPIABLE TAL CUAL) ⭐

Fuente: `out/vs/workbench/workbench.desktop.main.js:792`, módulo `mcp-url-utils.ts`.

### Allowlist de destinos (70 dominios)

```js
Jja=new Set(["airtable.com","anysphere.co","api.aws","apify.com","asana.com","ashbyhq.com",
"atlassian.com","atlassian.net","azuredatabricks.net","box.com","buildkite.com","calendly.com",
"canva.com","clickhouse.cloud","clickup.com","cloudflare.com","composio.dev","context7.com",
"cursor.com","cursor.sh","cursorapi.com","databricks.com","datadoghq.com","datadoghq.eu",
"ddog-gov.com","dropbox.com","elevenlabs.io","exa.ai","figma.com","firecrawl.dev","frontapp.com",
"github.com","githubcopilot.com","gitlab.com","google.com","googleapis.com","granola.ai","hex.tech",
"hubspot.com","huggingface.co","intercom.com","linear.app","monday.com","neon.tech","notion.com",
"plain.com","posthog.com","prisma.io","railway.com","ramp.com","render.com","salesforce.com",
"sentry.dev","sentry.io","slack.com","snowflakecomputing.com","spacelift.io","statsig.com",
"stripe.com","supabase.com","todoist.net","vercel.com","workos.com","x.ai","x.com","zapier.com"])
```

→ Es la lista de **servicios MCP de terceros que Cursor permite de fábrica** (Airtable, Notion,
Sentry, Stripe, Vercel, Zapier, HuggingFace, Composio, Firecrawl, Exa…). **Lista de lectura de
producto, no un formato.** Copiar la lista exacta en un repo público es discutible; copiar el
*mecanismo* (allowlist + denylist + admin) es claramente copiable.

### Validador de URL (regex literales)

```js
XRt="(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)?"                     // etiqueta de hostname
Zja=new RegExp(`^(?:${XRt}\\.)*${XRt}$`)                              // hostname
eYa=new RegExp(`^\\*\\.(?:${XRt}\\.)*${XRt}$`)                         // wildcard *.dominio
QRt="(?:25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)"                          // octeto
tYa=new RegExp(`^${QRt}(?:\\.${QRt}){3}$`)                            // IPv4
iYa=/^[0-9a-f:]+$/                                                   // IPv6
nYa=new RegExp(`^(?:${QRt}(?:\\.${QRt}){3}|[0-9a-f:]+)\\/\\d{1,3}$`)   // CIDR
JRt=2048                                                            // longitud máx de URL
H$n=/^([^/?#]+\/\/[^/?#]*):\*(?=[/?#]|$)/                             // wildcard de puerto
rYa=/^[^/?#]+\/\/[^/?#]*\*$/                                          // wildcard de host
```

### Campos de la política

Medidos en `workbench.desktop.main.js:792`:
`mcpNetworkMode`, `mcpNetworkDenylist`, `localAgentNetworkAllowlist`, `blockMessage`,
`win32UserExtensionUnsandbox`, y el mensaje de bloqueo:

```js
U$n="Blocked by MCP Network Admin Server Controls"
```

→ **La política de red la define el ADMIN del equipo (servidor) y se aplica al cliente.**
Copiable tal cual: allowlist + denylist + modo + mensaje + longitud máxima + validación estricta
de host/IP/CIDR/wildcards.

---

## 7.6 Contexto: qué entra al prompt (contrato `RequestContext`)

El proto `agent.v1.RequestContext` (íntegro en `02_rules_skills_modes.md` y en
`09_daemon_protocolo.md` §9.5) es el contrato autoritativo de qué se manda al backend.

Campos que respondan al punto 6 del encargo:

| Campo | Qué controla |
|---|---|
| `rules` / `non_file_rules` | reglas del agente (con y sin fichero asociado) |
| `file_contents` | mapa de ruta → contenido (ficheros abiertos) |
| `repository_info` / `repository_info_should_query_prod` | indexación del repo |
| `git_repos` / `git_status_info_complete` | contexto de git |
| `project_layouts` | estructura del proyecto |
| `env` / `env_info_complete` | entorno de ejecución |
| `tools` | catálogo de tools habilitadas |
| `mcp_instructions` / `mcp_info_complete` | instrucciones de los MCP |
| `mcp_file_system_options` | filesystem vía MCP |
| `agent_skills` / `agent_skills_info_complete` | skills |
| `custom_subagents` / `custom_subagents_info_complete` | custom modes |
| `hooks_config` / `hooks_additional_context` | hooks |
| `skill_options` | opciones de skills |
| `user_intent_summary` | resumen de la intención del usuario |
| `user/project/admin_permissions_auto_run` | permisos de auto-run por ámbito |
| `disabled_team_rules` / `admin_command_denylist` | política del admin |
| `system_prompt_override` | **override del prompt del sistema** |
| `debug_mode_config` | modo debug |

### `.cursorignore`

`workbench.desktop.main.js` tiene 11 hits de `.cursorignore` (líneas 746, 747, 914, 928, 17567,
17584, 17585, 19514, 20225, 20248, 20263) → existe y se respeta.
Detalle fino **NO MEDIDO**: la lista exacta de patrones que excluye y su precedencia frente a
`.gitignore`. **Pendiente** si el root lo necesita.

### Other scopes (contexto por superficie)

`workbench.desktop.main.js:694` — función que decide el ámbito de un recurso:
`case"project": … case"sources": return Ere(e)?"user":void 0; case"workspaces": case"team": return; default:return`
→ Los recursos MCP/rules/skills/etc. tienen scope: `project` | `user` | `workspaces` | `team` | `sources`.
Y `workbench.desktop.main.js:694` (`_vi`) marca qué scopes existen por tipo:
```js
function _vi(e,t){switch(e){
  case"plugins": case"mcps": return!0;
  case"rules": case"commands": return!0;
  case"skills": case"agents": case"hooks": return t!=="team";
  default: return!1}}
```
→ **los hooks, skills y agents NO tienen scope "team"** en esta versión: el scope `team` queda
excluido para ellos (`return t!=="team"`). Dato útil y **copiable tal cual**.

---

## 7.7 Valoración para MegaDaemon

| Aspecto | Valoración | Motivo |
|---|---|---|
| Convenciones OTel GenAI (`ai.*`, `gen_ai.*`) | **COPIABLE tal cual** | Son estándar OTel; adoptarlas da interoperabilidad de métricas gratis. |
| Validador de URL (host/IP/CIDR/wildcard/2048) | **COPIABLE tal cual** | Regex limpias, defensivas. |
| Política de red MCP (allowlist+denylist+modo+mensaje) | **ADAPTABLE** | El mecanismo es copiable; la lista de 70 dominios es de producto. |
| Mensaje `Blocked by MCP Network Admin Server Controls` | **ADAPTABLE** | Cambiar el texto; la idea de "admin del servidor manda" es buena para un modo multi-tenant. |
| `backend-endpoint` (`--backend-url` / `CURSOR_API_ENDPOINT`) | **ADAPTABLE** | El patrón (flag > env > default) es copiable; la política de hosts es de Cursor. |
| Endpoints `/auth/login_intent` → `/auth/poll` → `/auth/exchange_user_api_key` | **ADAPTABLE** | El flujo (intención → poll → intercambio por api key con `Bearer`) es un patrón copy-paste-able. |
| Nombres de span `ai.*` | **COPIABLE tal cual** | Ya son estándar. |
| Lista de 70 dominios MCP | **NO COPIABLE tal cual** (producto) | Puede usarse como semilla de una allowlist de MegaDaemon, pero no publicarla como "de Cursor". |
| Catálogo de modelos y precios | **NO MEDIDO** | Server-side. |
| Detalle de `.cursorignore` vs `.gitignore` | **NO MEDIDO** | Pendiente de un barrido específico. |

**Destino en MegaDaemon**: `rust-core/src/protocol.rs` (RequestContext),
`rust-core/src/mcp_ext.rs` (política de red), `rust-core/src/telemetry.rs` (nombres OTel),
`rust-core/src/api_key.rs` (flujo de auth).
