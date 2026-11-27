# CONTRATO 3 — HOOKS (Cursor 3.32.4)

Fuente: bundle del workbench, **minificado**. Citas literales verificadas.
Todas las rutas son relativas a `usr/share/cursor/resources/app/`.

---

## 3.1 Dónde vive el fichero de hooks

| Ámbito | Ruta | Evidencia |
|---|---|---|
| Usuario | `~/.cursor/hooks.json` | `workbench.desktop.main.js:18705` — `this.configUri=this.uriIdentityService.extUri.joinPath(e,".cursor","hooks.json")` |
| Proyecto (workspace) | `<workspace>/.cursor/hooks.json` | `workbench.desktop.main.js:18705` — `refreshWorkspaceConfigUris()`; `workbench.desktop.main.js:18714` — `"<workspace>/.cursor/hooks.json"` |
| Empresa (enterprise) | Linux `/etc/cursor/hooks.json` · macOS `/Library/Application Support/Cursor/hooks.json` · Windows `C:\ProgramData\Cursor\hooks.json` | `workbench.desktop.main.js:18705` — `n="/Library/Application Support/Cursor"` / `n="C:\\ProgramData\\Cursor"` / `n="/etc/cursor"` |
| Scripts de hook | `~/.cursor/hooks/beforePromptSubmit.sh` | `workbench.desktop.main.js:18714` — `h=r.extUri.joinPath(u,"hooks")`, `m=r.extUri.joinPath(h,"beforePromptSubmit.sh")` |
| Compat Claude Code (lectura) | `~/.claude/settings.json`, `<ws>/.claude/settings.json`, `<ws>/.claude/settings.local.json` | `workbench.desktop.main.js:18705` — `this.claudeUserConfigUri=…joinPath(e,".claude","settings.json")`; `workbench.desktop.main.js:18705` — `this.claudeProjectConfigUris`, `this.claudeProjectLocalConfigUris` |

---

## 3.2 Formato del fichero (contrato nativo Cursor)

Generado por el propio producto — `workbench.desktop.main.js:18714`:

```json
> [recortado: 8 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```

Cita literal (`workbench.desktop.main.js:18714`):
```
const b=JSON.stringify({version:1,hooks:{beforeSubmitPrompt:[{command:"./hooks/beforePromptSubmit.sh"}]}},null,2)
```

### Forma de una entrada de hook

`workbench.desktop.main.js:634`, función `qmu`:

```js
function qmu(e,t,i=cYe){
> [recortado: 9 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```

Campos:

| Campo | Tipo | Obligatorio | Notas |
|---|---|---|---|
| `type` | `"command"` \| `"prompt"` | implícito | si `type==="prompt"` se usa `prompt`; si no, se usa `command` |
| `command` | string | sí (salvo `type:"prompt"`) | se ejecuta como argv |
| `args` | string[] | no | se concatena al comando con quoting tipo shell |
| `prompt` | string | sí si `type==="prompt"` | hook resuelto por el modelo, no por shell |
| `timeout` | number (ms) | no | presente si se define |
| `loop_limit` | number \| null | no | por defecto `null` |
| `failClosed` | boolean | no | por defecto `false` |
| `shell` | — | **no soportado** | `workbench.desktop.main.js:634` `$mu`: *"Hook \"{i}\" sets shell \"{e.shell}\", which Cursor does not support; the platform default shell will be used"* |

Citations:
- `workbench.desktop.main.js:634` `$mu` — `const i=e.command;if(!i)return null;const n=e.args;return n==null?(e.shell!==void 0&&t.warn(\`Hook "${i}" sets shell "${e.shell}", which Cursor does not support; the platform default shell will be used\`),i):Vmu(n)?Hmu(i,n):(t.warn(\`Hook "${i}" has invalid "args" (expected an array of strings), skipping\`),null)`
  → `args` **debe** ser array de strings; si no, el hook se descarta.

---

## 3.3 Los 22 eventos (enum `hook-step.ts`)

`workbench.desktop.main.js:634`:

```js
yl={beforeShellExecution:"beforeShellExecution",beforeMCPExecution:"beforeMCPExecution",
> [recortado: 10 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```

Lista plana:
`beforeShellExecution`, `beforeMCPExecution`, `afterShellExecution`, `afterMCPExecution`,
`beforeReadFile`, `afterFileEdit`, `beforeTabFileRead`, `afterTabFileEdit`, `stop`,
`beforeSubmitPrompt`, `afterAgentResponse`, `afterAgentThought`, `sessionStart`,
`sessionEnd`, `preCompact`, `subagentStart`, `subagentStop`, `preToolUse`,
`postToolUse`, `postToolUseFailure`, `workspaceOpen`.

### Semántica del `matcher` (sujeto al que aplica)

`workbench.desktop.main.js:634`, función `egu`:

```js
> [recortado: 14 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```

Y el matching es **regex** (`workbench.desktop.main.js:634`, `tgu`):
```js
function tgu(e,t){if(!e.matcher||e.matcher===""||e.matcher==="*"||t===void 0)return!0;
  try{return new RegExp(e.matcher).test(t)}catch{return!0}}
```
→ `matcher` vacío / `"*"` / ausente = fires for all. Si el matcher no compila como regex, **fires for all** (fail-open).

---

## 3.4 Respuestas de los hooks (JSON por stdout)

Cada hook escribe un JSON por stdout. Esquemas (`workbench.desktop.main.js:634`):

| Evento | Campos admitidos | Validación literal |
|---|---|---|
| `beforeSubmitPrompt` | `continue?: boolean`, `user_message?: string`, `additional_context?: string` | `"continue must be a boolean if provided"`, `"user_message must be a string if provided"`, `"additional_context must be a string if provided"` |
| `beforeShellExecution` / `beforeMCPExecution` (before-command-execution) | `permission?: "allow"\|"deny"\|"ask"`, `user_message?: string`, `agent_message?: string` | `"Invalid permission value. Expected one of: allow, deny, ask, or undefined"` |
| `beforeReadFile`, `beforeTabFileRead` | `permission?: "allow"\|"deny"`, `user_message?: string` | `"Invalid permission value. Expected one of: allow, deny, or undefined"` |
| `preToolUse` | `permission?: "allow"\|"deny"\|"ask"`, `user_message?: string` | idem 3 valores |
| `subagentStart` | `permission?: "allow"\|"deny"\|"ask"`, `user_message?: string` | idem 3 valores |
| `postToolUse`, `postToolUseFailure` | `additional_context?: string` | `"additional_context must be a string if provided"` |
| `stop`, `subagentStop` | `followup_message?: string` | `"followup_message must be a string if provided"` |
| `preCompact` | `user_message?: string` (o string plano) | `"user_message must be a string"` |
| `workspaceOpen` | `pluginPaths?: string[]` | `"pluginPaths must be an array of strings if provided"` |
| `afterAgentResponse`, `afterAgentThought`, `afterFileEdit`, `afterMCPExecution`, `afterShellExecution`, `afterTabFileEdit` | objeto libre (debe ser objeto) | `"Expected an object"` |

### Resolución de decisión allow/deny/ask

`workbench.desktop.main.js:634` (`wgu`): **`deny` gana sobre `ask`, `ask` gana sobre `allow`**
```js
function wgu(e,t){if(e==="deny"||t==="deny")return"deny";if(e==="ask"||t==="ask")return"ask";if(...)
```
→ Es una política **deny-first**. Muy copiable.

---

## 3.5 Compatibilidad con Claude Code (claude-code-types.ts)

`workbench.desktop.main.js:634`:

```js
VTt={PreToolUse:yl.preToolUse, PermissionRequest:null, PostToolUse:yl.postToolUse,
> [recortado: 9 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```

- `PermissionRequest` y `Notification` → **evento no soportado**, se ignora con warning.
- `Glob` (de Claude) → **no soportado**, warning *"Tool \"Glob\" is not supported in Cursor and will be ignored"*.
- `Bash`→`Shell`, `Write`/`Edit`→`Write`.

### Traducción de `matcher` de Claude a Cursor

`workbench.desktop.main.js:634`, función `Gmu`:

```js
function Gmu(e,t=cYe){
> [recortado: 15 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```

→ El matcher de Claude es una **OR de nombres de tool** separados por `|`; `mcp__server__tool` se mapea a `MCP:server__tool`; los nombres de tool de Claude se traducen al catálogo de Cursor.

---

## 3.6 Interop con `~/.claude/settings.json`

`workbench.desktop.main.js:634` (`Kmu` / `Ymu`): si el `settings.json` de Claude tiene `hooks` con la forma `[{matcher, hooks:[…]}]`, Cursor lo **convierte** a su formato nativo y lo fusiona, con warnings por evento desconocido o no soportado.

---

## 3.7 Valoración para MegaDaemon

| Aspecto | Valoración | Motivo |
|---|---|---|
| Enum de 22 eventos | **COPIABLE tal cual** | Es una lista de nombres, sin lógica. naming consistente y agnóstico del vendor. |
| Formato `hooks.json` (`version:1` + `hooks{}`) | **COPIABLE tal cual** | Formato trivial, ya usado por Claude Code → interop gratis. |
| `matcher` como regex | **ADAPTABLE** | Es reutilizable, pero conviene limitar a tool-name exacto o `mcp__server__tool` para no abrir ReDoS con regex de usuario. |
| Semántica de respuesta (allow/deny/ask + deny-first) | **COPIABLE tal cual** | Es la política más valuable: deny > ask > allow. |
| Compat Claude Code (`VTt`/`FCa`) | **ADAPTABLE** | Requiere mantener un mapa de equivalencias; útil si MegaDaemon quiere Releer `~/.claude/settings.json`. |
| Warnings al usuario | **ADAPTABLE** | El formato de warning es de UI; en un daemon hay que convertirlo a log estructurado. |
| `type:"prompt"` (hook resuelto por LLM) | **NO MEDIDO (comportamiento server-side)** | El *prompt* se manda al backend; la semántica no está en el cliente. |

**Destino en MegaDaemon**: `rust-core/src/protocol.rs` (tipos de evento + `HookSpec` + `HookResponse`) y
`rust-core/src/tool_policy.rs` (orden de resolución deny>ask>allow).
