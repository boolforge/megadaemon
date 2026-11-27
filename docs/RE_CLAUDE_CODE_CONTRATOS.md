# RE — Claude Code 2.1.332 (build nativo linux-x64)

**Fecha de la medición:** 2026-11-27 · **Todo lo que hay aquí está medido**, con su
ruta y su hash. Lo que no se pudo medir va marcado **NO MEDIDO**.

## 1. Qué se descargó (con hashes)

| pieza | bytes | sha256 | de dónde |
|---|---:|---|---|
| `@anthropic-ai/claude-code` 2.1.332 (wrapper npm) | 32.525 | `8d8ac8379d14f7de86bff9c3a687d70c3f3487f734e7cda66f563da1fd176d36` | `registry.npmjs.org` |
| `@anthropic-ai/claude-code-linux-x64` 2.1.332 | 186.117.258 | `1c89f2f1e0bee93742cff8de9181fc07c702ce0651f1492bb3b97487d3a84af1` | `registry.npmjs.org` |
| **`package/claude`** (binario nativo) | **402.325.736** | **`a2df1f41bb00250992926420d0063f89b127cb81abdd284dbefb80c50ff1d026`** | dentro del tarball de plataforma |

## 2. La arquitectura de distribución (medido en `install.cjs`)

El paquete npm **no** es el programa: es un **wrapper**. `install.cjs` (postinstall)
copia el binario nativo del paquete de plataforma correspondiente a
`bin/claude.exe`, y a partir de ahí `claude` **ejecuta el binario nativo
directamente**: no queda ningún proceso Node resident.

`package.json` declara **9 plataformas** + 2 builds de Android:

```
darwin-arm64  darwin-x64  linux-x64  linux-arm64
linux-x64-musl  linux-arm64-musl
linux-arm64-android  linux-x64-android
win32-x64  win32-arm64
```

Reglas medidas del instalador:

* `FAIL_ON_MISSING_NATIVE` se sustituye en build; sin paquete de plataforma deja
  el stub y **explica** qué hacer (menciona Linuxulator para FreeBSD).
* **Dos copias instaladas ⇒ exit 1**; un `package.json` ilegible (>64 KiB) ⇒
  exit 1.
* Un **FIFO** en lugar de `package.json` no debe bloquear la lectura: lo abre con
  `O_NONBLOCK` (`MANIFEST_OPEN_FLAGS`).

## 3. El runtime y dónde está la lógica

| dato | valor | cómo se midió |
|---|---|---|
| runtime | **Bun 1.4.6** | cadena `Bun/1.4.6` en el binario |
| bundle JS embebido | **65.990.284 B** en **5.877 líneas** | líneas >1.000 chars de `strings -n 8` |
| source map embebido | **NO hay** | 1 única coincidencia de `sourceMappingURL=data:application/json;base64,` y es **un ejemplo del propio runtime de Bun** (`…base64,Transpiler`), no un mapa |
| identifiers del bundle | minificados (`$$$$0$1$2$3$4$A$F$H$R$_$bindable$break$…`) | línea de 47.669 chars |

**Consecuencia para el RE (y para el motor sancionado):** el binario nativo es el
runtime de Bun; **la lógica del agente está en el bytecode JS embebido**, no en
código nativo. Descompilar el ELF con Ghidra daría el runtime, no el agente. La
vía correcta es la que se ha usado aquí: **strings sobre el binario + extracción
del bundle** y análisis del código minificado. El bundle se ha extraído a
`/workspace/agy-re/productos/claude-code/native/bundle.js` (**no** se versiona:
es material de terceros).

## 4. Las 44 tools (del SDK oficial del propio paquete)

El paquete incluye `sdk-tools.d.ts`, **generado por `json-schema-to-typescript`
a partir del JSON Schema real**: las firmas son las que el binario aplica. 44
tools de entrada y 44 de salida, en 181.004 B. Catálogo completo (con el tipo de
cada campo y su descripción) en `docs/medidas/claude_code_tools.json`.

| tool | campos entrada | tool | campos entrada |
|---|---:|---|---:|
| Agent | 10 | TaskUpdate | 9 |
| Bash | 5 | TaskList | 0 |
| ExitPlanMode | 3 | Workflow | 7 |
| FileEdit | 4 | CronCreate / CronDelete / CronList | 4 / 1 / 0 |
| FileRead | 5 | ScheduleWakeup | 5 |
| FileWrite | 2 | RemoteTrigger | 5 |
| Glob | 2 | ShowOnboardingRolePicker | 0 |
| Grep | 9 | OfferChromeSetup | 1 |
| TaskStop | 2 | ReadNotifications | 0 |
| ListMcpResources | 1 | Monitor | 5 |
| RefreshMcpTools | 1 | ProposeSkills | 61 |
| Mcp | 1 | ProposeGoal | 2 |
| NotebookEdit | 5 | Artifact | 20 |
| ReadMcpResourceDir | 2 | PushNotification | 2 |
| ReadMcpResource | 2 | EnterWorktree / ExitWorktree | 2 / 2 |
| ReportFindings | 10 | TodoWrite | 3 |
| WebFetch | 3 | TaskCreate / TaskGet | 4 / 1 |
| WebSearch | 3 | EnterPlanMode | 0 |
| AskUserQuestion | 304 ⚠ | Projects | 7 |
| SendFeedback | 6 | ClaudeDesign | 2 |

⚠ `AskUserQuestion` con 304 campos y `ProposeSkills` con 61: es un **fallo de mi
parser** (tipos en línea con `;` dentro), no del esquema. Los tipos de esas dos
tools están **NO MEDIDOS** con detalle; el resto sí.

Semántica medida en los esquemas (ejemplos literales del `.d.ts`):

* `Bash.timeout`: "max 600000 for a foreground command".
* `Bash.run_in_background`: el timeout limita el proceso en segundo plano
  (default **1.800.000 ms**, máx **7.200.000 ms**).
* `Bash.dangerouslyDisableSandbox`: "run commands without sandboxing" ⇒ **el
  sandbox se puede desactivar desde la propia tool**.
* `ExitPlanMode.allowedPrompts`: marcado como **"Deprecated: no longer used"**.

## 5. Los 11 eventos de hook (medidos en el binario)

`PreToolUse` · `PostToolUse` · `PostToolUseFailure` · `PermissionRequest` ·
`UserPromptSubmit` · `Notification` · `Stop` · `SubagentStop` · `SessionStart` ·
`SessionEnd` · `PreCompact`

(Ocurrencias en el binario: PreToolUse 425, Stop 435, PostToolUse 262,
PermissionRequest 212, SessionStart 137, UserPromptSubmit 114, SubagentStop 67,
PreCompact 48, SessionEnd 47, Notification 44, PostToolUseFailure 64.)

El hook se configura en `hooks.json` y se combina con `settings.json` /
`settings.local.json` / `managed-settings.json`.

## 6. El modelo de permisos (medido)

**5 modos** de permiso, por su clave literal en el bundle:

| modo | clave | significado medido |
|---|---|---|
| por defecto | `default` | pregunta |
| edición automática | `acceptEdits` | acepta ediciones de ficheros |
| plan | `plan` | sólo planifica |
| bypass | `bypassPermissions` | salta la comprobación (206 apariciones) |
| auto | `autoMode` | modo automático (17 apariciones) |

Listas `allow` / `ask` / `deny` presentes (1.067 / 1.296 / 1.608 apariciones).
Interruptores de política restrictiva medidos:

| política | efecto |
|---|---|
| `permissions.disableBypassPermissionsMode` | desactiva el modo bypass |
| `permissions.disableAutoMode` | desactiva el modo auto |
| `permissions.blockReadsOutsideWorkingDirectories` | bloquea lecturas fuera del directorio de trabajo |
| `permissions.additionalDirectories` | directorios adicionales permitidos (6 apariciones) |

Es el mismo modelo de tres niveles que usa MegaDaemon (`deny` > `ask` > `allow`),
más los modos y los interruptores.

## 7. Defensa contra inyección y *trojan source* (medido)

El bundle lleva el patrón literal

```js
/[\u200c-\u200f\u202a-\u202e\u206a-\u206f\ufeff]/
```

Es decir: **caracteres de ancho cero** (U+200C-U+200F) y **marcas de dirección
bidi** (U+202A-U+202E, U+206A-U+206F) y el **BOM** (U+FEFF). Claude Code los
detecta para no ejecutar código con elVisually-confundido. **Esto es
reproducible en MegaDaemon**: el validador de entradas (`crates/redact` /
`rust-core`) debería rechazar o marcar estos rangos antes de pasarlos a un
modelo o a una shell.

## 8. Variables de entorno: **1.098** (medidas)

* **1.033** `CLAUDE_*`
* **65** `ANTHROPIC_*`

Lista completa en `docs/medidas/claude_code_contrato.json`. Subconjunto medido
relevante:

| variable | para qué |
|---|---|
| `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_IDENTITY_TOKEN` | credenciales de la API |
| `ANTHROPIC_BASE_URL`, `ANTHROPIC_API_HOST`, `ANTHROPIC_ASSETS_HOST` | endpoints (proxy) |
| `ANTHROPIC_BEDROCK_*`, `ANTHROPIC_VERTEX_*`, `ANTHROPIC_FOUNDRY_*`, `ANTHROPIC_GOOGLE_CLOUD_*` | los cuatro proveedores de modelo |
| `ANTHROPIC_AWS_API_KEY`, `ANTHROPIC_AWS_BASE_URL`, `ANTHROPIC_AWS_WORKSPACE_ID` | Bedrock |
| `ANTHROPIC_MODEL`, `ANTHROPIC_SMALL_FAST_MODEL`, `ANTHROPIC_DEFAULT_*_MODEL` | selección de modelo |
| `ANTHROPIC_CUSTOM_HEADERS`, `ANTHROPIC_BETAS` | cabeceras y betas de la API |
| `ANTHROPIC_CONFIG_DIR` | dónde está la configuración |
| `ANTHROPIC_TELEMETRY_GROUP`, `ANTHROPIC_ENVIRONMENT_ID` | telemetría |

## 9. Ficheros de configuración (medido)

`settings.json` (465), `settings.local.json` (257), `.mcp.json` (208),
`hooks.json` (75), `managed-settings.json` (61), `managed-mcp.json` (42),
`auth.json` (21), `marketplace.json` (16), `state.json` (11),
`claude_desktop_config.json` (13), `plugin.json` (35).

**73 rutas bajo `.claude/`** medidas (`.claude/agents/`, `.claude/commands/`,
`.claude/checkpoints/`, `.claude/classifiers/`, `.claude/daemon.json`,
`.claude/approvals.yml`, `.claude/agent-registry.json`, `.claude/bash-log.txt`, …).
Lista completa en el JSON de medidas.

## 10. Lo que NO se midió

* **El código fuente original** del bundle: no hay source map embebido (medido),
  y el npm no publica el código. El bundle minificado sí se extrajo.
* **El comportamiento** (qué hace realmente cada hook): requiere ejecución, no
  sólo estática.
* **MiniMax Code y DeepSeek Code**: no se han descargado en esta sesión ⇒
  **NO MEDIDO**.
* **Codex 0.172.0**: no descargado en esta sesión ⇒ **NO MEDIDO** (el paquete
  `openai/codex` es Rust: ahí sí aplicaría el motor sancionado).
* Las **tools' tipos** de `AskUserQuestion` y `ProposeSkills` (fallo de parser).

## 11. Cómo reproducir

```bash
# 1) los paquetes
curl -sL -o cc.tgz https://registry.npmjs.org/@anthropic-ai/claude-code/-/claude-code-2.1.332.tgz
curl -sL -o cc-linux-x64.tgz https://registry.npmjs.org/@anthropic-ai/claude-code-linux-x64/-/claude-code-linux-x64-2.1.332.tgz

# 2) el bundle y las medidas
strings -n 8 package/claude > strings.txt
awk 'length($0)>1000' strings.txt > bundle.js

# 3) las medidas estructuradas
python3 tools/extraer-tools-claude-code.py          # -> docs/medidas/claude_code_tools.json
python3 tools/medir-contrato-claude-code.py         # -> docs/medidas/claude_code_contrato.json
python3 tools/extraer-sourcemaps.py package/claude <dir>   # -> 0 source maps (medido)
```
