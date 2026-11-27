# RE DE PRODUCTO — CURSOR 3.32.4: CONTRATOS APROVECHABLES PARA MEGADAEMON

> **Qué es esto.** Ingeniería inversa **de contratos** (formatos, esquemas, catálogos,
> protocolos) del paquete oficial de Cursor 3.32.4 para linux-x64. No es decompilación:
> todo lo que hay aquí son **formatos, nombres, esquemas y protocolos** con su ruta exacta
> dentro del paquete. El objetivo es responder una pregunta concreta:
> **¿qué puede MegaDaemon copiar, y qué NO?**
>
> **Paquete analizado.** `cursor_3.32.4_amd64.deb`, 230 561 828 B.
> Descarga oficial: `https://downloads.cursor.com/production/0925c4b8226c56947c43f0b555fb98e187a7e029/linux/x64/deb/amd64/deb/cursor_3.32.4_amd64.deb`
> (`vendor/cursor.json` → `version:3.32.4`, `commitSha:0925c4b82…`)
> Construido el `2026-11-24T06:34:58.000Z` (`build-info.json` del daemon).
>
> **Reglas del encargo aplicadas:** solo lectura del paquete (no se ha ejecutado ningún
> binario de Cursor), sin secretos en el documento, sin `cargo`, sin `web_search`.
>
> **Aviso legal importante → ver §10.** La EULA de Cursor (§1.3) prohíbe explícitamente
> copiar, adaptar y redistribuir. Este informe es **análisis**; los ficheros extraídos
> verificados están en `artifacts/cursor/contratos/ficheros/` (**zona de artefactos, no el
> repo público**) y las citas literales son *hechos de interfaz*, no código.

---

## 0. Resumen ejecutivo: 9 contratos medidos, 6 copiables tal cual

| # | Contrato | Estado | Copiable tal cual |
|---|---|---|---|
| 1 | Manifiesto de extensiones y puntos de entrada | MEDIDO | Parcial |
| 2 | Agent rules / skills / custom modes | MEDIDO | **Sí** (formato) |
| 3 | **Hooks** (22 eventos + compat Claude Code) | MEDIDO | **Sí** |
| 4 | **MCP** (`mcp.json`, `placement`, allowlists) | MEDIDO | **Sí** |
| 5 | Catálogo de tools del agente (73 nombres) | MEDIDO (nombres) / NO MEDIDO (esquemas) | Nombres sí |
| 6 | Contexto (`RequestContext`, scopes, `.cursorignore`) | MEDIDO | Campos sí |
| 7 | Puente con el exterior (endpoints, auth, telemetría, red) | MEDIDO / parcial | **Sí** (mecanismos) |
| 8 | Licencia, precios, condiciones | MEDIDO (licencia) / NO MEDIDO (precios) | — |
| 9 | **El "agent host daemon": JSON-RPC 2.0 + MCP stateless** | MEDIDO | **Sí** |
| — | El CLI `cursor-agent` y su salida JSON | **NO MEDIDO** (no está en el paquete) | — |

**Las 4 cosas que MegaDaemon debería copiar ya:**

1. **`hooks.json` con 22 eventos** (`03_hooks.md`) — formato ya compatible con Claude Code,
   con una política de decisión **deny > ask > allow** que es exactamente lo que necesita
   `tool_policy.rs`.
2. **El daemon como servidor MCP stateless sobre JSON-RPC 2.0** (`09_daemon_protocolo.md`) —
   con deadlines en el mensaje, presupuestos de reintento por tipo de fallo y soporte de
   tareas asíncronas.
3. **Los custom modes como `CustomSubagent`** (`02_rules_skills_modes.md`) — 14 campos,
   allowlist de tools, permiso `DEFAULT|READONLY|AGENT_ONLY`.
4. **La lectura multi-ecosistema de skills** (`.claude/`, `.codex/`, `.grok/`, `.agents/`) —
   Cursor ya es multi-vendor; `.agents/skills` es el estándar neutral.

---

## 1. CONTRATO 1 — Manifiesto de extensiones y puntos de entrada

**Ruta base:** `usr/share/cursor/resources/app/extensions/`

### 1.1 Lo primero: NO hay `app.asar`

`usr/share/cursor/resources/app/` es un **directorio**, no un asar. El único `.asar` es
`node_modules.asar` (**28 B**, un marcador vacío). → **No hay nada que desempaquetar.**

### 1.2 Las 22 extensiones de Anysphere/Cursor (medido)

| Extensión | publisher | `main` | `extensionKind` | `contributes` | Peso |
|---|---|---|---|---|---|
| `cursor-agent-exec` | anysphere | `./dist/main` | — | — | 32,9 MB |
| `cursor-agent-host` | anysphere | `./dist/main.js` | — | commands, configuration | **56,0 MB** |
| `cursor-agent-worker` | anysphere | `./dist/main` | — | — | 2,0 MB |
| `cursor-always-local` | anysphere | `./dist/main` | `["ui"]` | commands, keybindings, menus, jsonValidation, configuration | 4,6 MB |
| `cursor-browser-automation` | **cursor** | `./dist/extension.js` | `["ui"]` | — | 0,6 MB |
| `cursor-checkout` | anysphere | `./dist/main` | — | — | 0,03 MB |
| `cursor-commits` | anysphere | `./dist/main` | `["workspace"]` | commands, keybindings, menus | 0,5 MB |
| `cursor-computer-use` | **cursor** | `./dist/extension.js` | `["ui"]` | — | 2,1 MB |
| `cursor-deeplink` | anysphere | `./dist/main` | `["ui"]` | commands | 0,3 MB |
| `cursor-explorer` | anysphere | `./dist/main` | `["workspace"]` | — | 0,03 MB |
| `cursor-file-service` | anysphere | — | — | — | 0,01 MB |
| `cursor-local-agent-runtime` | anysphere | `./dist/main` | `["ui"]` | — | 7,8 MB |
| `cursor-mcp` | anysphere | `./dist/main` | `["workspace"]` | commands, keybindings, menus, configuration | 1,5 MB |
| `cursor-ndjson-ingest` | anysphere | `./dist/main` | `["workspace"]` | commands, configuration | 0,02 MB |
| `cursor-polyfills-remote` | anysphere | `./dist/main` | `["workspace"]` | — | 0,1 MB |
| `cursor-resolver` | anysphere | `./dist/main` | `["ui"]` | resourceLabelFormatters | 0,4 MB |
| `cursor-resolver-helper` | anysphere | `./dist/main` | `["ui"]` | — | 0,9 MB |
| `cursor-retrieval` | anysphere | `./dist/main` | `["workspace"]` | commands, keybindings, menus, configuration, languages | **44,5 MB** |
| `cursor-shadow-workspace` | anysphere | `./dist/extension` | `["workspace"]` | commands, configuration | 0,4 MB |
| `cursor-socket` | anysphere | `./dist/main` | `["ui"]` | — | 0,03 MB |
| `cursor-worktree-textmate` | everysphere | — | `["ui"]` | languages, grammars, configurationDefaults | 0,1 MB |
| `protobuf` | anysphere | — | `["ui"]` | languages, grammars | 0,1 MB |

Detalle por extensión: `artifacts/cursor/contratos/01_manifiesto_extensiones.md`.

### 1.3 Descripciones literales (las que importan)

`extensions/cursor-browser-automation/package.json`:
> `"description": "MCP server for browser automation in {productName}"`, `"publisher": "cursor"`,
> `"extensionKind": ["ui"]`

`extensions/cursor-computer-use/package.json`:
> `"description": "Computer use in Cursor"`, `"publisher": "cursor"`

`extensions/cursor-ndjson-ingest/package.json`:
> `"description": "HTTP server for ingesting NDJSON logs to workspace/.cursor/debug.log"`

`extensions/cursor-agent-exec/package.json`:
> `"description": "Provides agent execution capabilities for {productName}, enabling agents to run commands, interact with files, and use tools with user permissions and approvals"`

`extensions/cursor-agent-host/package.json`:
> `"description": "Hosts {productName} agent orchestration in the AgentExec extension host"`

`extensions/cursor-local-agent-runtime/package.json`:
> `"description": "Hosts Cursor Private Inference outside workspace extension hosts"`

### 1.4 El runtime del producto (no son extensiones)

| Fichero | Ruta | Tamaño |
|---|---|---|
| Bundle workbench | `out/vs/workbench/workbench.desktop.main.js` | 32,4 MB (20 548 líneas) |
| Bundle "glass" | `out/vs/workbench/workbench.glass.main.js` | 40,0 MB |
| Bundle anysphere-ui | `out/vs/workbench/workbench.anysphere-ui.js` | 9,1 MB |
| Main Electron | `out/main.js` | 3,9 MB |
| **Proceso MCP** | `out/vs/code/electron-utility/mcpProcess/mcpProcessMain.js` | 858 KB |
| App "molten" (Canvas/Grok) | `molten/dist/` | bundle propio, **código legible** |
| Identidad | `product.json` | 56,4 KB |

**Binarios** (`usr/share/cursor/bin/`): `cursor` (6 105 B, script), `cursor-tunnel` y
`code-tunnel` (19 471 864 B, **idénticos byte a byte**).
**No existe `cursor-agent`** → ver §9.

---

## 2. CONTRATO 2 — Agent rules, skills y custom modes

### 2.1 El árbol de directorios `.cursor/` (medido)

`out/vs/workbench/workbench.desktop.main.js:748` (módulo `cursorRules.js`):

```js
vRt = UJ(".claude/skills")
_Rt = UJ(".codex/skills")
SRt = UJ(".grok/skills")
jwe = UJ(".claude/plugins")
bGn = UJ(".cursor/skills")
CQe = UJ(".agents/skills")
CGn = UJ(".cursor/agents")
b$a = UJ(".cursor/plugins")
C$a = UJ(".cursor/plugins/cache")
k$a = UJ(".claude/plugins/cache")
YG  = UJ(".cursor/skills-cursor")
```

**⭐ Hallazgo**: Cursor lee skills de **cuatro** ecosystems (`.claude/`, `.codex/`, `.grok/`,
`.agents/`) además del suyo. `.agents/skills` es el estándar inter-vendor neutral.

| Concepto | Ruta |
|---|---|
| Reglas (nuevo) | `<ws>/.cursor/rules/*.md` |
| Reglas (legacy) | `<ws>/.cursor/rules/*.mdc` |
| Skills | `<ws>/.cursor/skills/<nombre>/SKILL.md` |
| Skills de usuario | `~/.cursor/skills/<nombre>/SKILL.md` |
| Skills gestionadas | `~/.cursor/skills-cursor/<id>/SKILL.md` |
| **Custom modes** | `<ws>/.cursor/agents/` y `~/.cursor/agents/` |
| Slash commands | `<ws>/.cursor/commands/*.md` |
| Sandbox | `<ws>/.cursor/sandbox.json` |
| Settings | `<ws>/.cursor/settings.json` |
| Entorno cloud | `<ws>/.cursor/environment.json` |
| MCP (proyecto) | `<ws>/.cursor/mcp.json` |
| MCP (usuario) | `~/.cursor/mcp.json` |
| Hooks (usuario) | `~/.cursor/hooks.json` |
| Hooks (proyecto) | `<ws>/.cursor/hooks.json` |

`workbench.desktop.main.js:832` — Cursor **crea** `~/.cursor/agents` y lo vigila:
> `"Failed to ensure ~/.cursor/agents before watch; watch may not attach until it exists"`

### 2.2 Tipos de regla (discriminados)

`workbench.desktop.main.js:18381`:
```js
function s7c(e){return e?"global"in e&&e.global?"global"
  :"fileGlobbed"in e&&e.fileGlobbed?"fileGlobbed"
  :"agentFetched"in e&&e.agentFetched?"agentFetched"
  :"manuallyAttached"in e&&e.manuallyAttached?"manuallyAttached"
  :"none":"none"}
```
Tipos: `global` · `fileGlobbed` · `agentFetched` · `manuallyAttached` · `none`.

Campos del objeto regla (los 15 que compara `kxm`, misma línea):
`description`, `globs`, `alwaysApply`, `filename`, `body`, `frontmatter`, `fullPath`,
`gitRemoteOrigin`, `parseError`, `warnings`, `environments`, `disabledEnvironments`,
`plugin`, `marketplace`, `pluginId`, `marketplaceId`.

### 2.3 Formato exacto del fichero de regla

**Escritura** (`workbench.desktop.main.js:18387-18390`):
```js
const o=`---
${s.join(`
`)}
---
${t.trimEnd()}`
```
con `s = ["description: …", "globs: …", "alwaysApply: true|false"]`.

**Ejemplo real copiado** (lo que Cursor escribe):
```markdown
---
description: Usar tabs de 2 espacios en este repo
globs: **/*.ts
alwaysApply: false
---
Cuando edites TypeScript, indenta con 2 espacios, no con tabs.
```

**Lectura** (`workbench.desktop.main.js:18715`, `Usg`):
```js
const t={description:"", globs:"", alwaysApply:!1, attachToBackgroundAgents:!1,
         "metadata.environments":[], "metadata.disabledEnvironments":[]};
```
Claves aceptadas: `description`, `globs`, `alwaysApply`, `attachToBackgroundAgents`,
`metadata.environments`, `metadata.disabledEnvironments`.
El parser es ad-hoc (no YAML completo): `clave: valor` a nivel superior, listas por
`- item` indentado **o** por CSV.

### 2.4 `.md` vs `.mdc` (regla legacy → regla nueva)

`molten/dist/engine/engine.cjs:4729`:
```js
if(!e.endsWith(".mdc"))return{fullPath:e,content:t,description:"",alwaysApply:!0,globs:[]};
```
→ **Una regla `.md` sin frontmatter es SIEMPRE `alwaysApply:true` con `globs:[]`.**
Y existe el comando `Cursor: Convert Legacy Agent Applied Rules`
(`workbench.desktop.main.js:18715`) que migra `.mdc` → `.md` (solo las que no tengan
`globs` ni `alwaysApply`) y **borra** el `.mdc`.

### 2.5 `SKILL.md` — frontmatter y validaciones

`extensions/cursor-agent-exec/dist/main.js:213`:

| Campo | Alias | Tipo | Regla |
|---|---|---|---|
| `name` | — | string | ≤ **64** chars; minúsculas+números con guiones simples; **debe coincidir con la carpeta** |
| `description` | — | string | **obligatoria** salvo `disable-model-invocation: true`; ≤ **1024** chars |
| `alwaysApply` | — | bool | convierte la skill en regla `global` |
| `disable-model-invocation` | — | bool | la skill no se ofrece al modelo |
| `environments` | `metadata.environments` | array \| CSV | entornos donde aplica |
| `disabled-environments` | `metadata.disabledEnvironments` | array \| CSV | |
| `metadata.scopedTo` | — | array \| CSV | ámbito |
| `metadata.surfaces` | — | array \| CSV | superficies |
| `paths` | `globs` | array | rutas a las que aplica |
| `model` | — | string | modelo a forzar |
| `prompt` | — | string | prompt del subagente |

Mensajes de validación **literales** (copiables tal cual):
```
Name is {n} characters; the limit is 64. Rename the skill's folder.
Name should be lowercase letters and numbers joined by single hyphens, like "pdf-helper". Rename the skill's folder.
Frontmatter name {"x"} doesn't match the folder name {"y"}, which is the name the skill goes by. Make them the same.
Description is {n} characters; the limit is 1,024.
Body is about {n} tokens; the recommended limit is 5,000. Move details into reference files that SKILL.md links to.
Description is required
Invalid skill frontmatter: {e}
```

Normalización array-o-CSV (literal, `cursor-agent-exec/dist/main.js:213`):
```js
if(Array.isArray(e)){const t=e.filter(t=>"string"==typeof t&&t.length>0); return 0===t.length?void 0:t}
if("string"==typeof e){const t=e.split(",").map(t=>t.trim()).filter(t=>t.length>0); return 0===t.length?void 0:t}
```

### 2.6 Custom modes = `CustomSubagent` (proto `agent.v1.`)

`extensions/cursor-mcp/dist/main.js:212`:
```js
class qv extends Av{constructor(e){super(),
  this.fullPath="", this.name="", this.description="", this.tools=[],
  this.model="", this.prompt="", this.permissionMode=Mv.UNSPECIFIED,
  this.isBackground=!1, this.forceDefaultModel=!1, ...}}
static $(){return["CustomSubagent|1 full_path 9|2 name 9|3 description 9|4 tools 9*|
  5 model 9|6 prompt 9|7 permission_mode #0|8 is_background 8|9 plugin 9?|10 marketplace 9?|
  11 plugin_id 9?|12 marketplace_id 9?|13 force_default_model 8|14 source 9?",Mv]}}
```

```js
Mv = jc(Mc.proto3,Nv,"CustomSubagentPermissionMode",[[0,"UNSPECIFIED"],[1,"DEFAULT"],[2,"READONLY"],[3,"AGENT_ONLY"]],1)
Uv = jc(Mc.proto3,Nv,"TaskMode",[[0,"UNSPECIFIED"],[1,"AGENT"],[2,"PLAN"]],1)
Jv = jc(Mc.proto3,Nv,"SubagentExecutionEnvironment",[[0,"UNSPECIFIED"],[1,"LOCAL"],[2,"CLOUD"]],1)
Dv = jc(Mc.proto3,Nv,"SubagentBackgroundReason",[[0,"UNSPECIFIED"],[1,"AGENT_REQUEST"],[2,"USER_REQUEST"],[3,"QUEUED_FOLLOW_UP"]],1)
```

Modos internos del producto (literales, misma línea): `"AGENT"`, `"ASK"`, `"EDIT"`, `"DEBUG"`.
La UI separa built-in de custom por `source` (`workbench.desktop.main.js:17651`):
`F=(s??[]).filter(dt=>dt.source!=="custom")` / `B=(s??[]).filter(dt=>dt.source==="custom")`.

### 2.7 Reglas por defecto del producto

**NO EXISTEN.** `find . -name "*.mdc"` → **0 resultados** en todo el paquete.
Cursor no trae reglas ni skills por defecto; las trae el usuario, un plugin o
`~/.cursor/skills-cursor/<id>/SKILL.md` (generadas en runtime, no presentes en el `.deb`).

---

## 3. CONTRATO 3 — Hooks ⭐ (el más copiable)

Detalle completo: `artifacts/cursor/contratos/03_hooks.md`.

### 3.1 Dónde vive el fichero

| Ámbito | Ruta | Evidencia |
|---|---|---|
| Usuario | `~/.cursor/hooks.json` | `workbench.desktop.main.js:18705` |
| Proyecto | `<ws>/.cursor/hooks.json` | `workbench.desktop.main.js:18714` — `"<workspace>/.cursor/hooks.json"` |
| Empresa | `/etc/cursor/hooks.json` (Linux) · `/Library/Application Support/Cursor/hooks.json` (macOS) · `C:\ProgramData\Cursor\hooks.json` (Win) | `workbench.desktop.main.js:18705` |
| Scripts | `~/.cursor/hooks/beforePromptSubmit.sh` | `workbench.desktop.main.js:18714` |
| Compat Claude | `~/.claude/settings.json`, `<ws>/.claude/settings.json`, `<ws>/.claude/settings.local.json` | `workbench.desktop.main.js:18705` |

### 3.2 Formato (generado por el propio producto)

`workbench.desktop.main.js:18714`:
```js
const b=JSON.stringify({version:1,hooks:{beforeSubmitPrompt:[{command:"./hooks/beforePromptSubmit.sh"}]}},null,2)
```
**Ejemplo real copiado:**
```json
{
  "version": 1,
  "hooks": {
    "beforeSubmitPrompt": [
      { "command": "./hooks/beforePromptSubmit.sh" }
    ]
  }
}
```

### 3.3 Los 22 eventos

`workbench.desktop.main.js:634` (`hook-step.ts`):
```
beforeShellExecution  beforeMCPExecution  afterShellExecution  afterMCPExecution
beforeReadFile  afterFileEdit  beforeTabFileRead  afterTabFileEdit  stop
beforeSubmitPrompt  afterAgentResponse  afterAgentThought
sessionStart  sessionEnd  preCompact  subagentStart  subagentStop
preToolUse  postToolUse  postToolUseFailure  workspaceOpen
```

### 3.4 Forma de una entrada

`workbench.desktop.main.js:634` (`qmu`): `{type:"command"|"prompt", command, args?, prompt?, timeout?, loop_limit?, failClosed?}`.
`args` **debe** ser array de strings; `shell` **no está soportado** (warning literal:
*"Hook \"{i}\" sets shell \"{e.shell}\", which Cursor does not support; the platform default shell will be used"*).

### 3.5 Matcher: es una REGEX

`workbench.desktop.main.js:634` (`tgu`):
```js
function tgu(e,t){if(!e.matcher||e.matcher===""||e.matcher==="*"||t===void 0)return!0;
  try{return new RegExp(e.matcher).test(t)}catch{return!0}}
```
→ matcher vacío/`"*"`/ausente = fires for all; regex inválida = **fires for all** (fail-open).

Sujeto del matcher (`egu`, misma línea): `command` (shell), `tool_name` (MCP), `Read`,
`Write`, `TabRead`, `TabWrite`, `UserPromptSubmit`, `Stop`, `AgentResponse`,
`AgentThought`, `subagent_type`, `tool_name`.

### 3.6 Respuestas (JSON por stdout) y política

| Evento | Campos |
|---|---|
| `beforeSubmitPrompt` | `continue?: bool`, `user_message?: string`, `additional_context?: string` |
| `beforeShellExecution` / `beforeMCPExecution` | `permission?: "allow"\|"deny"\|"ask"`, `user_message?`, `agent_message?` |
| `beforeReadFile` / `beforeTabFileRead` | `permission?: "allow"\|"deny"`, `user_message?` |
| `preToolUse` / `subagentStart` | `permission?: "allow"\|"deny"\|"ask"`, `user_message?` |
| `postToolUse` / `postToolUseFailure` | `additional_context?: string` |
| `stop` / `subagentStop` | `followup_message?: string` |
| `preCompact` | `user_message?: string` |
| `workspaceOpen` | `pluginPaths?: string[]` |

**Política de decisión** (`workbench.desktop.main.js:634`, `wgu`) — **deny-first**:
```js
function wgu(e,t){if(e==="deny"||t==="deny")return"deny";if(e==="ask"||t==="ask")return"ask";if(...)
```
→ **deny > ask > allow**. Es la pieza más valuable de este contrato.

### 3.7 Compatibilidad Claude Code

`workbench.desktop.main.js:634` (`claude-code-types.ts`):
```js
VTt={PreToolUse:yl.preToolUse, PermissionRequest:null, PostToolUse:yl.postToolUse,
     UserPromptSubmit:yl.beforeSubmitPrompt, Stop:yl.stop, SubagentStop:yl.subagentStop,
     SessionStart:yl.sessionStart, SessionEnd:yl.sessionEnd, PreCompact:yl.preCompact,
     Notification:null}
FCa={Bash:"Shell",Read:"Read",Write:"Write",Edit:"Write",Glob:null,Grep:"Grep",
     WebFetch:"WebFetch",WebSearch:"WebSearch",Task:"Task"}
BCa=["Glob"]
UCa=["Notification","PermissionRequest"]
```
`PermissionRequest` y `Notification` → no soportados (warning).
`Glob` → no soportado: *"Tool \"Glob\" is not supported in Cursor and will be ignored"*.
Traducción de matcher Claude→Cursor (`Gmu`): `mcp__server__tool` → `MCP:server__tool`;
`Bash`→`Shell`, `Write`/`Edit`→`Write`.

---

## 4. CONTRATO 4 — MCP

Detalle completo: `artifacts/cursor/contratos/04_mcp.md`.

### 4.1 El esquema de `mcp.json` (Zod literal)

`extensions/cursor-mcp/dist/main.js:212`:
```js
const Ga=Aa({type:La(literal("stdio")), command:Pa(), args:La(Ra(Pa())), env:La(Ua(Pa(),Pa())),
  envFile:La(Pa()), cwd:La(Pa()), enabledTools:La(Ra(Pa())), startupTimeoutSec:…});
const Xa=Aa({type:La(Fa(["http","sse"])), url:Pa(), headers:La(Ua(Pa(),Pa())), auth:La(Va),
  tls:La(Ka), placement:La(Fa(["server","client"])), enabledTools:La(Ra(Pa()))});
const Qa=Ja([Ga,Xa]);
Aa({mcpServers:Ua(Pa(),Qa)});
```
OAuth: `Va=Aa({CLIENT_ID:Pa(),CLIENT_SECRET:La(Pa()),scopes:La(Ra(Pa())),redirectUris:La(Ra(Pa()))})`
TLS: `{caBundle: string}` con `min(1)`, `max(131072)`.

**Ejemplo real copiado:**
```json
{
  "mcpServers": {
    "github": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": { "GITHUB_TOKEN": "${GITHUB_TOKEN}" },
      "enabledTools": ["create_issue", "search_issues"]
    },
    "docs": {
      "type": "http",
      "url": "https://mcp.example.com/mcp",
      "headers": { "Authorization": "Bearer ${DOCS_TOKEN}" },
      "auth": { "CLIENT_ID": "…", "scopes": ["read"] },
      "placement": "server"
    }
  }
}
```

### 4.2 `placement`: dónde corre el MCP ⭐

`"placement": "server"` → corre en los servidores de Cursor (funciona en todos los dispositivos y
en cloud agents). `"placement": "client"` → local.
Cita (`workbench.desktop.main.js:20266`):
> *"It will run on Cursor's servers, so it works on every computer you sign in on and in cloud agents."*

### 4.3 Fusión y conflictos

Ámbitos: `~/.cursor/mcp.json` (usuario) · `<proyecto>/.cursor/mcp.json` (proyecto) ·
cuenta (servidor) · equipo (servidor) · plugin.
Campos comparados para detectar diferencias (`GJg`, `workbench.desktop.main.js:20266`):
`url`, todas las cabeceras, `auth.CLIENT_ID`, `auth.CLIENT_SECRET`, `auth.scopes`.
Resoluciones ofrecidas: `"use-account-version"` · `"replace-account-version"` · `"keep-both"` · `"cancel"`.

Detección de cabecera de auth (`UJg`, misma línea): `Object.keys(e??{}).some(t=>t.toLowerCase()==="authorization")`.

### 4.4 Proto de política

`extensions/cursor-mcp/dist/main.js:212`:
```js
McpServerTransport:    UNSPECIFIED | STDIO | HTTP
McpPolicyBlockedReason: UNSPECIFIED | MCP_SERVERS_OFF | MCP_ALLOWLIST | CLOUD_AGENT_ALLOWLIST | DENYLIST | APPLIES_TO | TOOL_ACCESS
McpRefreshPolicy:       UNSPECIFIED | FORCE_IF_MISSING | FORCE
```
`McpArgs` (11 campos): `name`, `args`, `tool_call_id`, `provider_identifier`, `tool_name`,
`smart_mode_approval_only`, `skip_approval`, `server_identifier`, `symbolic_path_arguments`,
`arguments_repaired`, `refuse_saved_secrets`.
→ **`symbolic_path_arguments`** es muy valuable: las rutas se resuelven donde corre la tool.

### 4.5 Allowlists (JSON Schema, copiado literal)

`extensions/cursor-always-local/schemas/permissions.schema.json` (1839 B) →
`artifacts/cursor/contratos/ficheros/permissions.schema.json`:
```jsonc
"mcpAllowlist": ["server:tool", "server:*", "*:tool", "*:*"],
"terminalAllowlist": ["…"],
"approvalMode": "allowlist" | "unrestricted" | "manual",
"autoRun":    { "allow_instructions": [], "block_instructions": [] },
"autoReview": { "allow_instructions": [], "block_instructions": [] }
```
Patrón regex: `^[^:*]+:[^:*]+$|^[^:*]+:\*$|^\*:[^:*]+$|^\*:$`.

`extensions/cursor-always-local/schemas/environment.schema.json` (8909 B) → copiado literal.
Añade `disableAllMcpServers`, `mcpServerAllowlist[{name,serverUrl|command,toolAllowlist}]`,
`egressAllowlist`, `egressMode` (`allow_all`|`parent_plus_network_settings`|`default_with_network_settings`|`network_settings_only`).

---

## 5. CONTRATO 5 — Catálogo de tools del agente

Detalle completo: `artifacts/cursor/contratos/05_tools.md`.

### 5.1 Los 73 nombres (medidos, literales)

`out/vs/workbench/workbench.desktop.main.js:719`, función `I6u(e,t)`.

| Familia | Nombres de tool |
|---|---|
| Ficheros | `read_file_v2` `edit_file_v2` `delete_file` `list_dir_v2` `read_lints` |
| Búsqueda | `ripgrep_raw_search` `glob_file_search` `semantic_search_full` |
| Terminal | `run_terminal_command_v2` `write_shell_stdin` `execute` |
| MCP | `get_mcp_tools` `mcp-<provider>-<tool>` `mcp_auth` `list_mcp_resources` `fetch_mcp_resource` |
| Web | `web_search` `web_fetch` `x_search` |
| Planificación | `todo_write` `todo_read` `create_plan` |
| Subagentes | `task_v2` `create_agent` `stop_agent` `send_to_agent` `get_agent_status` `read_agent_transcript` |
| Goals | `create_goal` `update_goal` |
| Interacción | `ask_question` `switch_mode` |
| Multimodal | `computer_use` `generate_image` `generate_video` `record_screen` `write_canvas` `read_canvas` |
| SCM | `connect_scm` `blame_by_file_path` `set_active_branch` `pr_management` `edit_pr_labels` |
| Bugbot/CI | `report_bug` `report_bugfix_results` `record_ci_investigation_findings` |
| Multi-agente | `SendMessage` `send_to_user` `send_final_summary` `update_current_step` |
| Code tour | `UpdatePrCodeTour` `GetPrCodeTour` |
| Cloud env | `setup_vm_environment` `replace_env` `fetch_cloud_agent_data` |
| Proyectos | `create_project` `start_project_updates` `stop_project_updates` |
| Tareas | `list_tracked_tasks` `create_tracked_tasks` `update_tracked_tasks` `delete_tracked_tasks` `update_tracked_task_statuses` `update_tracked_task_tags` |
| Otros | `fetch` `reflect` `await` `truncated` `apply_agent_diff` `ai_attribution` `manage_access` `search_conversations` `start_grind_execution` `start_grind_planning` |

**Tool dinámica MCP** (literal):
```js
case"mcpToolCall":{ … return`mcp-${n}-${r}` }   // n=providerIdentifier, r=toolName
```

### 5.2 Los esquemas de entrada: NO MEDIDO ⛔

Los JSON Schema de las tools nativas **viven en el backend** (`api2.cursor.sh`), no en el cliente.
Lo que sí está en el cliente es el `ToolSchema` **MCP estándar** (ver §9.3).

### 5.3 Taxonomía de errores por tool (copiable)

`workbench.desktop.main.js:719`, para `delete_file`:
```
fileNotFound     → "File not found: {path}"
notFile          → "Not a file ({actualType}): {path}"
permissionDenied → "Permission denied: {path}"
fileBusy         → "File busy: {path}"
rejected         → "Delete rejected: {reason}"
error            → "{error}" | "Unknown error"
success          → null
```
→ Taxonomía ideal para un daemon: errores accionables y distinguibles.

---

## 6. CONTRATO 6 — Contexto: qué entra al prompt

### 6.1 El proto `agent.v1.RequestContext` (autoritativo)

`extensions/cursor-mcp/dist/main.js:212` — campos (medido, literal):
```
rules · env · repository_info · tools · conversation_notes_listing · shared_notes_listing
git_repos · project_layouts · mcp_instructions · debug_mode_config · cloud_rule
web_search_enabled · skill_options · repository_info_should_query_prod · file_contents
user_intent_summary · custom_subagents · mcp_file_system_options · web_fetch_enabled
hooks_additional_context · commit_attribution_message · pr_attribution_message · hooks_config
agent_skills · precomputed_human_changes · recently_added_plugin · supports_mcp_auth
git_repo_info_complete · mcp_meta_tool_options · read_lints_enabled · mcp_info_complete
non_file_rules · matched_installed_plugin · rules_info_complete · env_info_complete
repository_info_complete · custom_subagents_info_complete · agent_skills_info_complete
mcp_file_system_info_complete · git_status_info_complete · user_permissions_auto_run
project_permissions_auto_run · admin_permissions_auto_run · disabled_team_rules
search_conversations_enabled · send_message_enabled · admin_command_denylist
system_prompt_override
```
(54 campos; los `#N` son índices de tipos anidados, los `8?`/`9?` opcionales.)

→ Esto **es** la respuesta a "cómo se decide qué entra al prompt".
`system_prompt_override` (campo 53) demuestra que **el system prompt tiene punto de extensión**.

### 6.2 Scopes de los recursos

`workbench.desktop.main.js:694`:
```js
function _vi(e,t){switch(e){
  case"plugins": case"mcps": return!0;
  case"rules": case"commands": return!0;
  case"skills": case"agents": case"hooks": return t!=="team";
  default: return!1}}
```
→ Scopes: `project` · `user` · `workspaces` · `team` · `sources`.
**Los hooks, skills y agents NO admiten scope "team".**

### 6.3 `.cursorignore`

11 hits en `workbench.desktop.main.js` (líneas 746, 747, 914, 928, 17567, 17584, 17585, 19514,
20225, 20248, 20263) → existe y se respeta.
**NO MEDIDO**: la lista exacta de patrones y su precedencia frente a `.gitignore`.

---

## 7. CONTRATO 7 — Puente con el exterior

Detalle completo: `artifacts/cursor/contratos/06_endpoints_contexto.md`.

### 7.1 Hosts

| Host | Para qué | Evidencia |
|---|---|---|
| `https://api2.cursor.sh` | **Backend del agente** | `agent-host-daemon/dist/bin/backend-endpoint.js:6` — `const DEFAULT_BACKEND_ENDPOINT = "https://api2.cursor.sh";` |
| `https://api.cursor.com` | Backend de la app "molten" | `molten/dist/renderer/assets/cursor-backend-ChqkVT9m.js:2` |
| `https://cursor.com` | Web, OAuth, docs | `cursor-mcp/dist/main.js:212`; `daemon.cjs:717` |
| `https://downloads.cursor.com` | Updates + **harness del agente** | `build-info.json` — `"agentHarnessCdnUrl":"https://downloads.cursor.com/agent-host-harness/"` |
| `https://cursor.com/agents/mcp/oauth/callback` | Callback OAuth de MCP | `molten/dist/engine/engine.cjs:26` |
| `console.statsig.com` / `api.statsigcdn.com` | Feature flags | harvest |
| `http://169.254.169.254` / `169.254.170.2` | IMDS de nube | harvest |

### 7.2 Auth (medido)

`extensions/cursor-agent-host/dist/agent-host-daemon/dist/bin/daemon.cjs`:
`/auth/login_intent` (POST, línea 22) → `/auth/poll` (POST, línea 27) →
`/auth/exchange_user_api_key` (POST, línea 23, `Authorization: Bearer ${apiKey}`; 403 →
`isSignInPolicyViolation`).
**NO MEDIDO**: el body de `login_intent` y el refresh de tokens.

### 7.3 Telemetría: convenciones OTel GenAI ⭐ (copiables tal cual)

`out/vs/workbench/workbench.desktop.main.js:23`:
```
ai.response.toolCalls   ai.prompt.messages   ai.prompt.tools   ai.model.id
ai.model.provider       ai.response.providerMetadata
ai.usage.cachedInputTokens  ai.telemetry.functionId
ai.usage.completionTokens   ai.usage.promptTokens
ai.toolCall.name  ai.toolCall.id  ai.toolCall.args
gen_ai.response.model  gen_ai.usage.input_tokens  gen_ai.usage.output_tokens
```
→ Son las **semantic conventions de OpenTelemetry para GenAI**. Adoptables tal cual.

Sentry (650 hits, `workbench.desktop.main.js:4`) = reporte de errores.
Statsig (242 hits) = feature flags, con flag `disable_statsig` (línea 494).
**NO hay cliente de telemetría de producto propio medido.**

### 7.4 Modelos

**NO MEDIDO el catálogo**: los ids no son constantes del cliente. Evidencia: el model picker
consume la lista por RPC (`molten/dist/renderer/assets/glass-model-picker-DIQ_38mA.js:2`),
la única URL es `https://cursor.com/docs/models`, y el harvest de ids solo devuelve nombres
internos/de test (`auto`, `composer-ensemble`, `gpt-4`, `claude-3.5-sonnet`…).

Mecanismo sí medido: preferencias de parámetros por modelo persistidas
(`modelParameterPreferences`, con `modelId`/`parameters`), diálogos de confirmación por modelo
(`confirmationDialogue`), billing **por uso** ("Get More Usage" +
`cursor.com/docs/bugbot#usage-based-billing`), y modelo forzado por modo
(`CustomSubagent.model` + `force_default_model`).

### 7.5 Política de red MCP ⭐ (copiable)

`out/vs/workbench/workbench.desktop.main.js:792` (`mcp-url-utils.ts`):
allowlist de **70 dominios** (`Jja=new Set([...])`), denylist, `mcpNetworkMode`,
`localAgentNetworkAllowlist`, `blockMessage`, longitud máx de URL **2048**.
Validadores:
```js
XRt="(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)?"              // etiqueta
Zja=new RegExp(`^(?:${XRt}\\.)*${XRt}$`)                       // hostname
eYa=new RegExp(`^\\*\\.(?:${XRt}\\.)*${XRt}$`)                  // wildcard
QRt="(?:25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)"                   // octeto
tYa=new RegExp(`^${QRt}(?:\\.${QRt}){3}$`)                     // IPv4
iYa=/^[0-9a-f:]+$/                                            // IPv6
nYa=new RegExp(`^(?:${QRt}(?:\\.${QRt}){3}|[0-9a-f:]+)\\/\\d{1,3}$`)  // CIDR
JRt=2048
H$n=/^([^/?#]+\/\/[^/?#]*):\*(?=[/?#]|$)/                      // puerto wildcard
rYa=/^[^/?#]+\/\/[^/?#]*\*$/                                   // host wildcard
```
Mensaje de bloqueo: `U$n="Blocked by MCP Network Admin Server Controls"`.

---

## 8. CONTRATO 8 — Licencia, precios, condiciones

Detalle completo: `artifacts/cursor/contratos/08_licencia_precios.md`.

### 8.1 ⚠️ La EULA prohíbe el RE explícitamente

`usr/share/cursor/resources/app/LICENSE.txt` — *"Cursor End User License Agreement,
Last Modified: June 13, 2023"*, con Anysphere Inc. como parte.

**§1.3 Use Restrictions** (cita literal):
> *"you may not: **(a) copy, modify, adapt, translate, prepare derivative works from, reverse
> engineer, disassemble, or decompile the Software** …; **(b) license, sell, resell, assign,
> transfer, sublicense, lease, lend, rent or otherwise distribute the Software to any third
> party**; …"*

**§1.2**: licencia *"limited, non-exclusive, non-transferable, and freely revocable … solely
for your personal use only"*.

**Consecuencia práctica** (distinción fina, y la que decide el destino de cada fichero):

| Actividad | Veredicto |
|---|---|
| Analizar formatos de fichero para reimplementarlos | OK en principio (son hechos/interfaz observable, no expresión) |
| Escribir un lector/escritor **independiente** de `hooks.json` / `mcp.json` / frontmatter | OK en principio |
| Copiar **código** de los bundles al repo de MegaDaemon | **NO** (§1.3 a) |
| Redistribuir los ficheros extraídos | **NO** (§1.3 b) |
| Publicar en un repo público las citas literales de este informe | Zona gris → **recomendación: mantener el informe con citas en la Drive privada** |

### 8.2 Terceros (licencias, copiables)

De `agent-host-daemon/dist/bin/daemon.cjs.LEGAL.txt` (11 025 B):
`undici`/formdata-polyfill (MIT), `ws` (MIT), `tough-cookie` (BSD-3-Clause, Salesforce),
**`re2js` v1.2.2 (MIT — RE2 lineal)**, `is-extendable` (MIT), `strip-bom-string` (MIT),
`ieee754` (BSD-3-Clause), `image-q` (MIT).

⭐ **`re2js` (RE2) es lo que usa Cursor para el matcher de hooks** — por eso puede ser regex sin
riesgo de ReDoS. **Recomendación directa: usar `re2` (crate de Rust) para los matchers de hooks
de MegaDaemon.**

### 8.3 Precios: NO MEDIDO

No hay literales de precio/plan en los bundles; el producto pregunta al servidor.
Lo único medido: el billing es **por uso** (`cursor.com/docs/bugbot#usage-based-billing`,
"Get More Usage" en el model picker) y existe una UI de **teams** con enlaces a
`cursor.com/privacy` y `cursor.com/terms-of-service`.
**Las cifras, los límites de cuota y los planes NO son recuperables del paquete** → requieren
`curl` a cursor.com. **Pendiente para el root.**

---

## 9. CONTRATO 9 — El "agent host daemon" ⭐ (lo más copiable)

Detalle completo: `artifacts/cursor/contratos/09_daemon_protocolo.md`.

Ruta: `extensions/cursor-agent-host/dist/agent-host-daemon/dist/`
(`daemon.cjs` 23 MB bundle, pero **los ficheros de arranque son fuente legible** con doc comments).

### 9.1 Arranque y flags

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
```

`dist/bin/backend-endpoint.js`:
```js
export const BACKEND_URL_FLAG = "--backend-url";
/** The variable every part of the daemon reads its backend from. */
export const BACKEND_ENDPOINT_VARIABLE = "CURSOR_API_ENDPOINT";
/** The backend a daemon with none named talks to. */
const DEFAULT_BACKEND_ENDPOINT = "https://api2.cursor.sh";
```
Resolución con procedencia explícita (`backend-endpoint.d.ts`):
```ts
readonly source: "flag" | "environment" | "default";
readonly endpoint: string;
```
Rechazos literales: *"carries a user name or a password"*, *"without https"*,
*"which is not one of Cursor's API hosts"*.

### 9.2 `environment-file`: consume-and-destroy ⭐ (patrón de seguridad copiable)

`dist/bin/environment-file.d.ts`:
> *"The file is a JSON object of variable names to string values. It must be named like
> ENVIRONMENT_FILE_NAME, and be a regular file owned by this user with **mode 0600 and one
> link**. Any refusal leaves the file where it is, for the launcher to delete: the daemon
> deletes only a file it has parsed."*
> *"Deletes the file while its descriptor is still open, then checks that no name is left:
> a copy renamed away or linked after the checks would survive."*

API: `takeEnvironmentFile`, `applyEnvironmentFile`, `openVerifiedEnvironmentFile`,
`deleteOpenFile`.

### 9.3 El protocolo: JSON-RPC 2.0 + MCP stateless

`daemon.cjs:14906`:
```js
RequestIdSchema = union([string(), number().int()])
JSONRPCRequestSchema = object({jsonrpc:literal(JSONRPC_VERSION), id:RequestIdSchema, ...RequestSchema.shape}).strict()
JSONRPCNotificationSchema = object({jsonrpc:literal(JSONRPC_VERSION), ...NotificationSchema.shape}).strict()
JSONRPCResultResponseSchema = object({jsonrpc:literal(JSONRPC_VERSION), id:RequestIdSchema, result:ResultSchema}).strict()
JSONRPCErrorResponseSchema = object({jsonrpc:literal(JSONRPC_VERSION), id:RequestIdSchema.optional(),
  error:object({code:number().int(), message:string(), data:unknown().optional()})}).strict()
```

**Métodos MCP implementados** (medidos):
```
initialize  notifications/initialized  notifications/cancelled  ping
tools/list  tools/call  tools/common  tools/grep  tools/mcp  tools/read  tools/shell
resources/list  resources/read  resources/subscribe  resources/unsubscribe
prompts/list  prompts/get
server/discover  notifications/progress  notifications/message
tasks/list  tasks/get  tasks/result  tasks/cancel
```
`server/discover` es **extensión privativa** de Cursor (línea 14997).

**Código de error propio** (`daemon.cjs:14907`):
```js
ErrorCode2[ErrorCode2["UrlElicitationRequired"]=-32042]="UrlElicitationRequired"
```
→ `-32042` = "el agente necesita autorización del usuario para esa URL".

**`ToolSchema` MCP** (`daemon.cjs:14918`):
```js
ToolSchema=object({ ...BaseMetadataSchema.shape, ...IconsSchema.shape,
  description:string().optional(),
  inputSchema:object({type:literal("object"), properties:record(string(),AssertObjectSchema).optional(),
                      required:array(string()).optional()}).catchall(unknown()),
  outputSchema:object({...}).catchall(unknown()).optional(),
  annotations:ToolAnnotationsSchema.optional(),
  execution:ToolExecutionSchema.optional(),
  _meta:record(string(),unknown()).optional()})
ToolAnnotationsSchema=object({title,readOnlyHint,destructiveHint,idempotentHint,openWorldHint})
ToolExecutionSchema=object({taskSupport:_enum(["required","optional","forbidden"]).optional()})
CallToolResultSchema=ResultSchema.extend({content:array(ContentBlockSchema).default([]),
  structuredContent:record(string(),unknown()).optional(), isError:boolean().optional()})
```

**Tareas asíncronas** (extensión de Cursor): `TaskAugmentedRequestParamsSchema`,
`notifications/progress` con `progressToken`, `tasks/*`, y `execution.taskSupport`
(una tool declara si DEBE ir en una task).

**Handshake** (`daemon.cjs:14995`): `_meta` con `protocolVersion`, `capabilities`, `clientInfo`.

### 9.4 Deadline-first y presupuestos de reintento ⭐

`daemon.cjs:13924` (el sobre de una llamada):
```js
{id, method:"tools/call", requestID:randomUUID(), name, arguments,
 timeoutSeconds: timeoutMs/1e3,
 startedAtEpochSeconds,
 deadlineEpochSeconds: startedAtEpochSeconds + timeoutMs/1e3,
 mode, ...(sessionId!==void 0&&{sessionId})}
```
→ **el deadline viaja en el mensaje**, no se recalcula en el receptor.

`daemon.cjs:3647`:
```js
AGENT_RETRY_CAPS = {serverCap:3, transportCap:10};
STALL_RETRY_BUDGET_MS = 3*60*1e3;
RATE_LIMIT_RETRY_BUDGET_MS = 5*60*1e3;
RETRY_BUDGET_BY_KIND = {alpn_failure:"server", poll_seqno_gap:"server", append_fatal:"by_signature",
  http_464:"server", http_404_unimplemented:"server", socket_reset:"transport",
  stream_closed_early:"server", idle_deadline:"server", stall:"transport", other:"server"}
MAX_RETRIES_CEILING = 100
```
→ **copiable tal cual**: dos tapas (servidor / transporte), presupuestos temporales por tipo de
fallo, y "de quién es la culpa" explícito.

### 9.5 Lo que NO se pudo extraer ⛔

- **El CLI `cursor-agent` no está en el paquete.** `usr/share/cursor/bin/` solo tiene `cursor`,
  `cursor-tunnel`, `code-tunnel`; `usr/bin/` no existe. Por tanto **el "protocolo JSON del
  CLI `cursor-agent`" NO es recuperable de aquí** (se distribuye aparte).
- El prompt del sistema del agente: vive en el backend (solo se ve el punto de extensión
  `system_prompt_override`).
- Los esquemas de entrada de las tools nativas: viven en el backend.
- La lógica de `apply_agent_diff`, `reflect`, `await`, `start_grind_*`: en el backend.
- El **runtime/harness del agente** se descarga del CDN
  (`agentHarnessCdnUrl`), no está en el `.deb` → no es analizable offline.

---

## 10. Tabla maestro: contrato → ¿copiable por MegaDaemon? → fichero destino

| Contrato | Medido en | ¿Copiable? | Motivo | Destino en `rust-core/src/` |
|---|---|---|---|---|
| **Hooks: 22 eventos** | `workbench.desktop.main.js:634` | **SÍ, tal cual** | Lista de nombres, sin lógica | `protocol.rs` (`HookEvent`) |
| **Hooks: formato `hooks.json`** (`version:1`+`hooks{}`) | `workbench.desktop.main.js:18714` | **SÍ, tal cual** | Trivial y ya compatible con Claude Code | `protocol.rs` (`HookSpec`) |
| **Hooks: respuestas + deny>ask>allow** | `workbench.desktop.main.js:634` | **SÍ, tal cual** | La política más valuable del análisis | `tool_policy.rs` |
| **Hooks: matcher regex** | `workbench.desktop.main.js:634` | **ADAPTABLE** | Reutilizable; usar `re2` en vez de `regex` (Cursor usa `re2js`) | `protocol.rs` |
| **Hooks: compat Claude Code** (`VTt`/`FCa`) | `workbench.desktop.main.js:634` | **ADAPTABLE** | Exige mantener equivalencias | `protocol.rs` |
| **MCP: esquema `mcp.json`** | `cursor-mcp/dist/main.js:212` | **SÍ, tal cual** | Zod plano, casi estándar | `mcp_ext.rs` |
| **MCP: `placement: server\|client`** | `cursor-mcp/dist/main.js:212` | **ADAPTABLE** | La idea sí; en MD el MCP corre en el worker | `mcp_ext.rs` |
| **MCP: `McpArgs.symbolic_path_arguments`** | `cursor-mcp/dist/main.js:212` | **ADAPTABLE (muy valioso)** | Evita mandar rutas absolutas | `protocol.rs` |
| **MCP: `McpPolicyBlockedReason`** (7 valores) | `cursor-mcp/dist/main.js:212` | **SÍ, tal cual** | Taxonomía ya pensada | `mcp_ext.rs` |
| **MCP: `McpRefreshPolicy`** (3 valores) | `cursor-mcp/dist/main.js:212` | **SÍ, tal cual** | Trivial y útil | `mcp_ext.rs` |
| **Allowlist MCP/terminal + autoRun** (`permissions.schema.json`) | `cursor-always-local/schemas/` | **SÍ, tal cual** | Formato casi estándar | `tool_policy.rs` |
| **Allowlist de servidores por entorno + egress** | `cursor-always-local/schemas/environment.schema.json` | **ADAPTABLE** | Schema grande de cloud env; sirve el bloque MCP | `mcp_ext.rs` |
| **Custom modes (`CustomSubagent`)** | `cursor-mcp/dist/main.js:212` | **SÍ, tal cual** | 14 campos, diseño limpio y portable | `protocol.rs` + nuevo `agent_modes.rs` |
| **`permission_mode` DEFAULT/READONLY/AGENT_ONLY** | `cursor-mcp/dist/main.js:212` | **SÍ, tal cual** | 3 niveles muy usables | `tool_policy.rs` |
| **`TaskMode` AGENT/PLAN** | `cursor-mcp/dist/main.js:212` | **SÍ, tal cual** | Ya existe el concepto en MD | `protocol.rs` |
| **Reglas: frontmatter `description`/`globs`/`alwaysApply`** | `workbench.desktop.main.js:18715`, `:18387` | **SÍ, tal cual** | 3 campos YAML triviales | `skills.rs` |
| **Regla `.md` = alwaysApply implícito** | `molten/dist/engine/engine.cjs:4729` | **SÍ, tal cual** | Regla simple | `skills.rs` |
| **Skills: frontmatter de Cursor** | `cursor-agent-exec/dist/main.js:213` | **ADAPTABLE** | Copiar nombres, aceptar también camelCase y `allowed-tools` | `skills.rs` |
| **Límites 64/1024/5000** | `cursor-agent-exec/dist/main.js:213` | **SÍ, tal cual** | Ya parecidos en `skills.rs` | `skills.rs` |
| **Lectura multi-ecosistema `.claude/.codex/.grok/.agents`** | `workbench.desktop.main.js:748` | **SÍ, tal cual (estrategia)** | Lo más valuable: MD debería adoptar la misma lista | `skills.rs` |
| **73 nombres de tool** | `workbench.desktop.main.js:719` | **SÍ (nombres)** | Vocabulario de dominio | `protocol.rs` |
| **Esquemas de entrada de tools nativas** | — | **NO MEDIDO ⛔** | Viven en el backend | — |
| **Taxonomía de errores de tool** | `workbench.desktop.main.js:719` | **SÍ, tal cual** | Errores accionables | `protocol.rs` |
| **`ToolSchema` MCP (+annotations, taskSupport)** | `daemon.cjs:14918` | **SÍ, tal cual** | MCP estándar + 2 extensiones | `mcp_server.rs` |
| **JSON-RPC 2.0 + MCP stateless como transporte** | `daemon.cjs:14906`, `:14993` | **SÍ, tal cual** | MCP ya es estándar | `protocol.rs` + `mcp_server.rs` |
| **Tareas asíncronas (`tasks/*`)** | `daemon.cjs:14919` | **SÍ, tal cual** | Encaja con el `protocol.rs` de MD | `protocol.rs` |
| **Deadline en el mensaje** | `daemon.cjs:13924` | **SÍ, tal cual** | Deadline-first sin ambigüedad de reloj | `protocol.rs` |
| **Presupuestos de reintento** | `daemon.cjs:3647` | **SÍ, tal cual** | Pensado; ahorramanship | nuevo `retry.rs` |
| **`environment-file` consume-and-destroy** | `environment-file.d.ts` | **SÍ, tal cual (patrón)** | Patrón de seguridad, no de producto | nuevo `secrets.rs` / `api_key.rs` |
| **`backend-endpoint` (flag>env>default)** | `backend-endpoint.d.ts` | **ADAPTABLE** | Patrón copiable; política de hosts es de Cursor | `api_key.rs` |
| **`UrlElicitationRequired = -32042`** | `daemon.cjs:14907` | **SÍ, tal cual** | Código JSON-RPC de la serie de implementación | `protocol.rs` |
| **Convenciones OTel GenAI (`ai.*`, `gen_ai.*`)** | `workbench.desktop.main.js:23` | **SÍ, tal cual** | Ya son estándar OTel | nuevo `telemetry.rs` |
| **Validador de URL MCP (host/IP/CIDR/wildcard/2048)** | `workbench.desktop.main.js:792` | **SÍ, tal cual** | Regex defensivas | `mcp_ext.rs` |
| **Política de red MCP (allowlist+denylist+modo)** | `workbench.desktop.main.js:792` | **ADAPTABLE** | Mecanismo sí; la lista de 70 dominios es de producto | `mcp_ext.rs` |
| **Lista de 70 dominios MCP de Cursor** | `workbench.desktop.main.js:792` | **NO (producto)** | Puede servir de semilla, no publicarse como "de Cursor" | — |
| **`RequestContext` (54 campos)** | `cursor-mcp/dist/main.js:212` | **ADAPTABLE (muy valioso)** | Copiar los campos; el wire protobuf no | `agent_core.rs` |
| **Scopes (`project/user/workspaces/team/sources`)** | `workbench.desktop.main.js:694` | **SÍ, tal cual** | Simple y útil | `protocol.rs` |
| **Hooks/skills/agents sin scope "team"** | `workbench.desktop.main.js:694` | **SÍ, tal cual** | Dato concreto | `protocol.rs` |
| **Flujo de auth `login_intent`→`poll`→`exchange_user_api_key`** | `daemon.cjs:22,23,27` | **ADAPTABLE** | Patrón copy-paste-able | `api_key.rs` |
| **Catálogo de modelos y precios** | — | **NO MEDIDO ⛔** | Server-side | — |
| **Detalle de `.cursorignore` vs `.gitignore`** | `workbench.desktop.main.js` (11 hits) | **NO MEDIDO ⛔** | Pendiente de barrido | — |
| **CLI `cursor-agent` y su salida JSON** | — | **NO MEDIDO ⛔** | El binario no está en el `.deb` | — |
| **Prompt del sistema del agente** | — | **NO MEDIDO ⛔** | Backend (solo hay `system_prompt_override`) | — |
| **Manifiesto de extensión (formato VS Code)** | `extensions/*/package.json` | **NO** | MD tiene su propio registro (`plugins/`) | `plugins/` |
| **Proceso MCP separado** | `out/vs/code/electron-utility/mcpProcess/` | **SÍ, tal cual (patrón)** | Aislar código de terceros | `mcp_server.rs` |
| **`cursor-ndjson-ingest` (ingest NDJSON por HTTP)** | `extensions/cursor-ndjson-ingest/package.json` | **SÍ, tal cual** | Patrón útil para trazas | nuevo `agent_traces.rs` |

---

## 11. Lo que NO se pudo extraer (resumen explícito)

| # | Qué | Por qué |
|---|---|---|
| 1 | **CLI `cursor-agent`** y su protocolo JSON de salida | El binario no está en el `.deb` (solo `cursor`, `cursor-tunnel`, `code-tunnel`). Se distribuye aparte. |
| 2 | **Esquemas JSON de entrada de las tools nativas** | Los genera el backend y llegan por RPC; no hay constantes en el cliente. |
| 3 | **Catálogo de modelos** y **precios/plan/cuotas** | Server-side. El cliente solo tiene la UI del picker y preferencias por `modelId`. |
| 4 | **Prompt del sistema del agente** | Backend. Solo se ve el punto de extensión `system_prompt_override`. |
| 5 | **Lógica de `apply_agent_diff`, `reflect`, `await`, `start_grind_*`** | Backend. |
| 6 | **Runtime/harness del agente** | Se descarga del CDN (`agentHarnessCdnUrl`), no está en el paquete. |
| 7 | **Reglas/skills por defecto del producto** | **No existen**: `find . -name "*.mdc"` → 0 resultados. |
| 8 | **Detalle de `.cursorignore`** (patrones y precedencia vs `.gitignore`) | Pendiente de un barrido específico; solo se ha medido que se respeta (11 hits). |
| 9 | **Listado de MCP servers de terceros por defecto** | Solo se ha medido la *allowlist de dominios* (70), no qué servidores concretos hay. |
| 10 | **Código de los bundles** (workbench, daemon, molten) | Minificado; y la EULA prohíbe copiarlo (§1.3). No es el objetivo. |

---

## 12. Dónde está todo

| Qué | Ruta |
|---|---|
| Informe completo por contrato | `artifacts/cursor/contratos/01…09_*.md` |
| Ficheros extraídos **verbatim** | `artifacts/cursor/contratos/ficheros/` |
| Inventario del paquete (ruta, bytes) | `artifacts/cursor/INVENTARIO.tsv` (12 971 ficheros, 980,6 MB) |
| Datos crudos del harvest (URLs, paths) | `artifacts/cursor/contratos/_raw/` |
| Script de harvest (reproducible) | `artifacts/cursor/_tools/harvest.py` |
| Paquete original | `artifacts/vendor/cursor.deb` + `artifacts/vendor/cursor.json` |

---

## 13. Recomendaciones al root

1. **Prioridad P0 — `hooks.json` + política deny>ask>allow.**
   Es el contrato con mejor relación valor/riesgo: formato estándar, 22 eventos, y encaja
   directamente en `rust-core/src/protocol.rs` + `tool_policy.rs`.
2. **Prioridad P0 — `CustomSubagent` como modelo de "custom mode".**
   14 campos, portable, y ya definido con un proto limpio. Mucho más simple que lo que
   MegaDaemon tenga hoy.
3. **Prioridad P1 — deadline-first + presupuestos de reintento.**
   El patrón `deadlineEpochSeconds` en el mensaje y `RETRY_BUDGET_BY_KIND` son copiables tal
   cual y arreglan dos problemas típicos del enjambre.
4. **Prioridad P1 — adoptar `.agents/skills` como raíz neutral**, leyendo también
   `.claude/skills` y `.codex/skills` como hace Cursor.
5. **Decisión pendiente (root):** ¿este documento con citas literales vive en el repo público?
   La EULA §1.3(b) prohíbe redistribuir el software; las *citas* no son el software, pero el
   informe incluye literales de código extensos. **Recomendación: subir
   `artifacts/cursor/` (contratos + ficheros verbatim) a la Drive privada
   `15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0` con `tools/drive_push.py`, y dejar en el repo público
   solo este documento con las citas reducidas a hechos de formato.**
6. **Pendiente de medición** (si el root lo quiere): barrido de `.cursorignore` vs `.gitignore`,
   y `curl` a cursor.com/pricing para los precios (no hay `web_search`, pero sí `curl`).
