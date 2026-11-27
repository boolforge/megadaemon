# RE DE PRODUCTO — CURSOR 3.32.4: CONTRATOS APROVECHABLES PARA MEGADAEMON

> **Licencia y redistribución.** Cursor es **proprietary** (Anysphere Inc.); aquí sólo hay
> **análisis de formato y rutas**, y **ningún material redistribuido**. Este documento cita
> fragmentos de 1-3 líneas **sólo cuando son la definición del formato** (cabecera de un
> fichero de configuración, un nombre de evento, un nombre de campo); todo lo demás se
> describe con palabras. Los extractos largos, los ficheros extraídos verbatim y los datos
> crudos están en la **Drive privada** del proyecto, no en este repositorio.
> La EULA de Cursor (§1.2-1.3) prohíbe copiar, adaptar, hacer obras derivadas y redistribuir
> el software; el análisis de formatos observables y la reimplementación independiente de un
> formato no son el software. Ver `docs/medidas/cursor/08_licencia_precios.md`.

> **Qué es esto.** Ingeniería inversa **de contratos** (formatos, esquemas, catálogos,
> protocolos) del paquete oficial de Cursor 3.32.4 para linux-x64. No es decompilación.
> Objetivo: **¿qué puede MegaDaemon copiar, y qué NO?**

**Paquete analizado.** `cursor_3.32.4_amd64.deb`, 230 561 828 B, de
`https://downloads.cursor.com/production/0925c4b8226c56947c43f0b555fb98e187a7e029/linux/x64/deb/amd64/deb/cursor_3.32.4_amd64.deb`
(`vendor/cursor.json` → `version:3.32.4`, `commitSha:0925c4b82…`). Build del agente:
`2026-11-24T06:34:58.000Z` (`build-info.json` del daemon).
**Reglas del encargo aplicadas:** solo lectura del paquete (ningún binario de Cursor ejecutado),
sin secretos en los documentos, sin `cargo`, sin `web_search`.

**Formato del paquete.** `usr/share/cursor/resources/app/` es un **directorio**, no un
`app.asar`; el único `.asar` es `node_modules.asar` (28 B, marcador vacío). **No hubo nada que
desempaquetar.** El paquete descomprimido son 12 971 ficheros, 980,6 MB
(`artifacts/cursor/INVENTARIO.tsv` — sólo rutas y tamaños, sin contenido).

**Datos medidos.** El desarrollo por contrato está en `docs/medidas/cursor/` (9 ficheros):
rutas y líneas exactas, tablas de campos, catálogos de nombres y mensajes de validación
literales, con los bloques de código largos recortados.

---

## 0. Resumen ejecutivo

**9 contratos medidos.** Copiables tal cual por MegaDaemon: **hooks**, **custom modes**,
**el daemon MCP stateless**, **allowlists de permisos**, **convenciones OTel GenAI**,
**presupuestos de reintento**, **validador de URL de red**. El resto es adaptable.

**Las 5 piezas que MegaDaemon debería copiar primero** (destinos en §12):

1. `hooks.json` — `{version:1}` + 22 eventos, con política **deny > ask > allow**.
2. `CustomSubagent` — el "custom mode" con 14 campos y allowlist de tools.
3. El daemon MCP stateless sobre JSON-RPC 2.0, con **deadline en el mensaje**, **tareas
   asíncronas** y **presupuestos de reintento**.
4. La lectura **multi-ecosistema** de skills (`.claude/`, `.codex/`, `.grok/`, `.agents/`,
   `.cursor/`).
5. `mcp.json` con `placement`, `symbolic_path_arguments` y allowlists.

**Hecho medido relevante:** Cursor 3.32.4 **no trae reglas ni skills por defecto** (0 ficheros
`.mdc` en todo el paquete). Las reglas las crea el usuario, un plugin o el runtime.

---

## 1. CONTRATO 1 — Manifiesto de extensiones y puntos de entrada

**Ruta base:** `usr/share/cursor/resources/app/extensions/`

**22 extensiones** de los publishers `anysphere`, `cursor` y `everysphere`. Las cinco que
definen la arquitectura del producto:

| Extensión | `main` | `extensionKind` | Rol (descripción literal del manifiesto) |
|---|---|---|---|
| `cursor-agent-host` | `./dist/main.js` | — | Orquestación del agente (56 MB) |
| `cursor-agent-exec` | `./dist/main` | — | Ejecución de tools con permisos y aprobaciones (33 MB) |
| `cursor-retrieval` | `./dist/main` | `["workspace"]` | Indexación y retrieval de código (45 MB) |
| `cursor-mcp` | `./dist/main` | `["workspace"]` | Gestión de MCP (1,5 MB) |
| `cursor-local-agent-runtime` | `./dist/main` | `["ui"]` | Inferencia privada fuera del host de extensiones (8 MB) |

Dos extensiones de Cursor son **MCP servers del propio producto**: `cursor-browser-automation`
(«MCP server for browser automation») y `cursor-computer-use` («Computer use in Cursor»),
ambas `publisher: "cursor"`, `extensionKind: ["ui"]`.

**No son extensiones, pero marcan la arquitectura:** el bundle del workbench
(`out/vs/workbench/workbench.desktop.main.js`, 32,4 MB, 20 548 líneas muy largas), el proceso
MCP **aislado** (`out/vs/code/electron-utility/mcpProcess/mcpProcessMain.js`, 858 KB),
`out/main.js` (3,9 MB), la app «molten» (`molten/dist/`, con **código legible**) y
`product.json` (56,4 KB).

**Binarios** (`usr/share/cursor/bin/`): `cursor` (6 105 B, script lanzador), `cursor-tunnel` y
`code-tunnel` (19 471 864 B, idénticos byte a byte). **No existe `cursor-agent`** (§9.5).

Detalle: `docs/medidas/cursor/01_manifiesto_extensiones.md`.

---

## 2. CONTRATO 2 — Agent rules, skills y custom modes

### 2.1 Dónde viven

`out/vs/workbench/workbench.desktop.main.js:748` (módulo `cursorRules.js`) declara nueve raíces
de descubrimiento. Cursor lee skills de **cuatro** ecosystems distintos además del propio:
`.claude/skills`, `.codex/skills`, `.grok/skills`, **`.agents/skills`** (el estándar neutral
inter-vendor) y `.cursor/skills`. También dos raíces de plugins (`.cursor/plugins`,
`.cursor/plugins/cache`) y dos de plugins de Claude (`.claude/plugins`, `.claude/plugins/cache`).

| Concepto | Ruta |
|---|---|
| Reglas (formato actual) | `<ws>/.cursor/rules/*.md` |
| Reglas (formato legacy) | `<ws>/.cursor/rules/*.mdc` |
| Skills | `<ws>/.cursor/skills/<nombre>/SKILL.md` · `~/.cursor/skills/<nombre>/SKILL.md` |
| Skills gestionadas por Cursor | `~/.cursor/skills-cursor/<id>/SKILL.md` |
| **Custom modes** | `<ws>/.cursor/agents/` y `~/.cursor/agents/` |
| Slash commands | `<ws>/.cursor/commands/*.md` |
| Sandbox / settings / entorno | `<ws>/.cursor/sandbox.json` · `.cursor/settings.json` · `.cursor/environment.json` |
| MCP | `<ws>/.cursor/mcp.json` · `~/.cursor/mcp.json` |
| Hooks | `<ws>/.cursor/hooks.json` · `~/.cursor/hooks.json` |

`workbench.desktop.main.js:832`: Cursor **crea** `~/.cursor/agents` y lo vigila con un watcher
(el mensaje de log avisa de que el watch puede no engancharse hasta que exista).

### 2.2 Tipos de regla

`workbench.desktop.main.js:18381` define cinco tipos discriminados: `global`, `fileGlobbed`,
`agentFetched`, `manuallyAttached` y `none`. El objeto regla tiene 15 campos comparados:
`description`, `globs`, `alwaysApply`, `filename`, `body`, `frontmatter`, `fullPath`,
`gitRemoteOrigin`, `parseError`, `warnings`, `environments`, `disabledEnvironments`, `plugin`,
`marketplace`, `pluginId`, `marketplaceId`.

### 2.3 Formato del fichero de regla

**Escritura** (`workbench.desktop.main.js:18387-18390`): frontmatter delimitado por `---` con las
líneas `description: …`, `globs: …` y `alwaysApply: true|false`, seguido del cuerpo Markdown.

Definición del formato (extracto de 3 líneas):
```markdown
---
description: …    globs: …    alwaysApply: true|false
---
```

**Lectura** (`workbench.desktop.main.js:18715`, parser `Usg`): claves aceptadas
`description`, `globs`, `alwaysApply`, `attachToBackgroundAgents`, `metadata.environments` y
`metadata.disabledEnvironments`. No es YAML completo: sólo `clave: valor` a nivel superior, con
listas por ítem indentado o por CSV.

### 2.4 `.md` frente a `.mdc`

`molten/dist/engine/engine.cjs:4729`: una regla `.md` **sin** frontmatter se resuelve como
`alwaysApply: true` con `globs` vacío; un `.mdc` lleva el frontmatter explícito. Existe el
comando `Cursor: Convert Legacy Agent Applied Rules`
(`workbench.desktop.main.js:18715`) que migra `.mdc` → `.md` (sólo las que no declaren `globs`
ni `alwaysApply`) y borra el `.mdc`.

### 2.5 `SKILL.md` — frontmatter y validaciones

`extensions/cursor-agent-exec/dist/main.js:213`. Campos: `name`, `description`, `alwaysApply`,
`disable-model-invocation`, `environments` (alias `metadata.environments`),
`disabled-environments` (alias `metadata.disabledEnvironments`), `metadata.scopedTo`,
`metadata.surfaces`, `paths` (alias `globs`), `model` y `prompt`.

Límites: **nombre ≤ 64 caracteres** y coincidente con el nombre de la carpeta;
**descripción ≤ 1024 caracteres** y obligatoria salvo `disable-model-invocation: true`;
**cuerpo ≈ 5 000 tokens** como límite recomendado. Los mensajes de validación literales están
transcritos en `docs/medidas/cursor/02_rules_skills_modes.md`.

Los campos de lista aceptan **array o CSV** en la misma clave (normalización literal en
`cursor-agent-exec/dist/main.js:213`).

El frontmatter se parsea con dos implementaciones intercambiables — la propia (`frontmatter`) o
`graymatter` (npm) — elegidas por el feature flag `agentReplaceGraymatter`
(`cursor-agent-exec/dist/main.js:8`).

**Diferencias con Claude Code** (relevante: MegaDaemon ya soporta skills de Claude):
Cursor usa kebab-case (`disable-model-invocation`), Claude camelCase (`disableModelInvocation`);
Cursor añade `alwaysApply`, `metadata.scopedTo`, `metadata.surfaces`, `environments` y
`paths`/`globs`; Claude aporta `allowedTools`, `context`, `files`, `aliases`, `whenToUse`,
`argumentHint`, `model`, `userInvocable` y `hooks`.

### 2.6 Custom modes = `CustomSubagent`

`extensions/cursor-mcp/dist/main.js:212` — proto `agent.v1.`, 14 campos: `full_path`, `name`,
`description`, `tools` (lista repetida = **allowlist de tools del modo**), `model`, `prompt`,
`permission_mode`, `is_background`, `plugin`, `marketplace`, `plugin_id`, `marketplace_id`,
`force_default_model` y `source`.

Enums medidos:
- `CustomSubagentPermissionMode` = `UNSPECIFIED` · `DEFAULT` · `READONLY` · `AGENT_ONLY`
- `TaskMode` = `UNSPECIFIED` · `AGENT` · `PLAN`
- `SubagentExecutionEnvironment` = `UNSPECIFIED` · `LOCAL` · `CLOUD`
- `SubagentBackgroundReason` = `UNSPECIFIED` · `AGENT_REQUEST` · `USER_REQUEST` · `QUEUED_FOLLOW_UP`

Modos internos del producto (literales, misma línea): `AGENT`, `ASK`, `EDIT`, `DEBUG`.
La UI separa built-in de custom por el campo `source` (`workbench.desktop.main.js:17651`).

### 2.7 Reglas por defecto

**No existen.** `find . -name "*.mdc"` → **0 resultados** en todo el paquete. Hecho medido;
no volver a buscarlo.

Detalle: `docs/medidas/cursor/02_rules_skills_modes.md`.

---

## 3. CONTRATO 3 — Hooks ⭐

### 3.1 Dónde vive el fichero

| Ámbito | Ruta | Evidencia |
|---|---|---|
| Usuario | `~/.cursor/hooks.json` | `workbench.desktop.main.js:18705` |
| Proyecto | `<ws>/.cursor/hooks.json` | `workbench.desktop.main.js:18714` |
| Empresa | `/etc/cursor/hooks.json` (Linux) · `/Library/Application Support/Cursor/hooks.json` (macOS) · `C:\ProgramData\Cursor\hooks.json` (Windows) | `workbench.desktop.main.js:18705` |
| Scripts | `~/.cursor/hooks/beforePromptSubmit.sh` | `workbench.desktop.main.js:18714` |
| Compat Claude Code | `~/.claude/settings.json`, `<ws>/.claude/settings.json`, `<ws>/.claude/settings.local.json` | `workbench.desktop.main.js:18705` |

### 3.2 Formato

`workbench.desktop.main.js:18714` — el propio producto genera el fichero con esta forma:

```json
{ "version": 1, "hooks": { "beforeSubmitPrompt": [ { "command": "./hooks/beforePromptSubmit.sh" } ] } }
```

### 3.3 Los 22 eventos

`workbench.desktop.main.js:634` (módulo `hook-step.ts`):
`beforeShellExecution`, `beforeMCPExecution`, `afterShellExecution`, `afterMCPExecution`,
`beforeReadFile`, `afterFileEdit`, `beforeTabFileRead`, `afterTabFileEdit`, `stop`,
`beforeSubmitPrompt`, `afterAgentResponse`, `afterAgentThought`, `sessionStart`, `sessionEnd`,
`preCompact`, `subagentStart`, `subagentStop`, `preToolUse`, `postToolUse`,
`postToolUseFailure`, `workspaceOpen`.

### 3.4 Forma de una entrada

`workbench.desktop.main.js:634` (`qmu`): `{type: "command"|"prompt", command, args?, prompt?,
timeout?, loop_limit?, failClosed?}`. `args` debe ser un array de strings; el campo `shell`
**no está soportado** (el producto avisa con un warning y usa el shell por defecto del sistema).

### 3.5 El matcher es una REGEX

`workbench.desktop.main.js:634` (`tgu`): matcher vacío, `"*"` o ausente ⇒ fires for all;
una regex que no compila ⇒ **fires for all** (fail-open).
El sujeto del matcher depende del evento (`egu`): `command` para shell, `tool_name` para MCP y
pre/post tool, `Read`, `Write`, `TabRead`, `TabWrite`, `UserPromptSubmit`, `Stop`,
`AgentResponse`, `AgentThought` y `subagent_type`.

### 3.6 Respuestas (JSON por stdout) y política de decisión

| Evento | Campos admitidos |
|---|---|
| `beforeSubmitPrompt` | `continue?` (bool), `user_message?`, `additional_context?` |
| `beforeShellExecution`, `beforeMCPExecution` | `permission?` (`allow`/`deny`/`ask`), `user_message?`, `agent_message?` |
| `beforeReadFile`, `beforeTabFileRead` | `permission?` (`allow`/`deny`), `user_message?` |
| `preToolUse`, `subagentStart` | `permission?` (`allow`/`deny`/`ask`), `user_message?` |
| `postToolUse`, `postToolUseFailure` | `additional_context?` |
| `stop`, `subagentStop` | `followup_message?` |
| `preCompact` | `user_message?` |
| `workspaceOpen` | `pluginPaths?` (array de strings) |

**La política** (`workbench.desktop.main.js:634`, `wgu`) es **deny-first**: si cualquiera de los
dos lados dice `deny` gana `deny`; si no, `ask` gana sobre `allow`.
→ Es la pieza más valuable del contrato y encaja directo en `tool_policy.rs`, que ya tiene
`PolicyDecision::{Allow, Prompt, Deny}` (`Prompt` ≈ `ask`).

### 3.7 Compatibilidad con Claude Code

`workbench.desktop.main.js:634` (`claude-code-types.ts`): mapa de eventos de Claude a los de
Cursor (`PreToolUse`→`preToolUse`, `UserPromptSubmit`→`beforeSubmitPrompt`, `Stop`→`stop`,
`SubagentStop`→`subagentStop`, `SessionStart`/`SessionEnd`/`PreCompact` homónimos);
`PermissionRequest` y `Notification` quedan **sin soporte** (warning). Mapa de tools de Claude
a Cursor: `Bash`→`Shell`, `Write`/`Edit`→`Write`, `Read`/`Grep`/`WebFetch`/`WebSearch`/`Task`
homónimas; `Glob` queda **sin soporte** (warning). El matcher de Claude se traduce: es una OR
de nombres de tool separados por `|`, y `mcp__servidor__tool` se renombra a
`MCP:servidor__tool`.

Detalle: `docs/medidas/cursor/03_hooks.md`.

---

## 4. CONTRATO 4 — MCP

### 4.1 Esquema de `mcp.json`

`extensions/cursor-mcp/dist/main.js:212` — esquema Zod plano. Raíz: un objeto `mcpServers`
cuyo valor es un mapa de nombre → servidor. Cada servidor es una **unión discriminada por
`type`**:

| Variante | Campos |
|---|---|
| `stdio` | `command` (obligatorio), `args?`, `env?`, `envFile?`, `cwd?`, `enabledTools?`, `startupTimeoutSec?` |
| `http` / `sse` | `url` (obligatorio), `headers?`, `auth?`, `tls?`, `placement?`, `enabledTools?` |

`auth` (OAuth) lleva `CLIENT_ID` (obligatorio), `CLIENT_SECRET?`, `scopes?`, `redirectUris?` —
**en SCREAMING_SNAKE_CASE, a diferencia del resto del formato**, que es camelCase.
`tls` es un objeto con `caBundle` (string), con longitud mínima 1 y máxima 131 072, y campos
extra permitidos.

### 4.2 `placement`: dónde corre el MCP ⭐

`"placement": "server"` ⇒ el servidor MCP corre **en los servidores de Cursor** (funciona en
todos los dispositivos y en los cloud agents); `"placement": "client"` ⇒ local.
Citado en `workbench.desktop.main.js:20266`: el servidor «funcionará en cada ordenador en el
que inicies sesión y en los cloud agents».

### 4.3 Ámbitos y conflictos

Ámbitos: `~/.cursor/mcp.json` (usuario), `<proyecto>/.cursor/mcp.json` (proyecto), la cuenta
(servidor), el equipo (servidor) y los plugins. Al detectar diferencias se comparan **la URL,
todas las cabeceras, `auth.CLIENT_ID`, `auth.CLIENT_SECRET` y `auth.scopes`**
(`workbench.desktop.main.js:20266`) y se ofrecen cuatro resoluciones:
`use-account-version`, `replace-account-version`, `keep-both`, `cancel`.
Si las cabeceras incluyen `authorization` (case-insensitive) el servidor **no necesita login
interactivo**: se queda con la cabecera y no hay nada que volver a autorizar.

### 4.4 Proto de política y sobre de llamada

`extensions/cursor-mcp/dist/main.js:212`:
- `McpServerTransport` = `UNSPECIFIED` · `STDIO` · `HTTP` (el fichero de config admite además
  `sse`, legacy del cliente).
- `McpPolicyBlockedReason` = `UNSPECIFIED` · `MCP_SERVERS_OFF` · `MCP_ALLOWLIST` ·
  `CLOUD_AGENT_ALLOWLIST` · `DENYLIST` · `APPLIES_TO` · `TOOL_ACCESS`.
- `McpRefreshPolicy` = `UNSPECIFIED` · `FORCE_IF_MISSING` · `FORCE`.
- `McpArgs` (11 campos): `name`, `args`, `tool_call_id`, `provider_identifier`, `tool_name`,
  `smart_mode_approval_only`, `skip_approval`, `server_identifier`, `symbolic_path_arguments`,
  `arguments_repaired`, `refuse_saved_secrets`.

→ **`symbolic_path_arguments`** es especialmente valioso: la ruta viaja simbólica y se resuelve
donde corre la tool, así el daemon no necesita conocer el sistema de ficheros del usuario.

### 4.5 Allowlists (JSON Schema literal, copiado)

`extensions/cursor-always-local/schemas/permissions.schema.json` (1 839 B) define un objeto con
`mcpAllowlist` (patrón `servidor:tool`, `servidor:*`, `*:tool`, `*:*`, con su regex de
validación), `terminalAllowlist`, `approvalMode` (`allowlist`/`unrestricted`/`manual`, donde
`manual` equivale a «Ask Every Time») y los bloques `autoRun` y `autoReview`, cada uno con
`allow_instructions` y `block_instructions`.

`extensions/cursor-always-local/schemas/environment.schema.json` (8 909 B) añade, para entornos
cloud: `disableAllMcpServers`, `mcpServerAllowlist` (entradas con `name`, y `serverUrl` **o**
`command`, más `toolAllowlist`), `egressAllowlist` y `egressMode`
(`allow_all`/`parent_plus_network_settings`/`default_with_network_settings`/`network_settings_only`).

Detalle: `docs/medidas/cursor/04_mcp.md`.

---

## 5. CONTRATO 5 — Catálogo de tools del agente

### 5.1 Los 73 nombres

`out/vs/workbench/workbench.desktop.main.js:719`, función `I6u(e,t)`: mapea el nombre del
caso proto al nombre de la tool en el cable. Catálogo completo (por familia):

- **Ficheros**: `read_file_v2`, `edit_file_v2`, `delete_file`, `list_dir_v2`, `read_lints`
- **Búsqueda**: `ripgrep_raw_search`, `glob_file_search`, `semantic_search_full`
- **Terminal**: `run_terminal_command_v2`, `write_shell_stdin`, `execute`
- **MCP**: `get_mcp_tools`, `mcp_auth`, `list_mcp_resources`, `fetch_mcp_resource`, y la
  dinámica `mcp-<providerIdentifier>-<toolName>`
- **Web**: `web_search`, `web_fetch`, `x_search`
- **Planificación**: `todo_write`, `todo_read`, `create_plan`
- **Subagentes**: `task_v2`, `create_agent`, `stop_agent`, `send_to_agent`, `get_agent_status`,
  `read_agent_transcript`
- **Goals**: `create_goal`, `update_goal`
- **Interacción**: `ask_question`, `switch_mode`
- **Multimodal / canvas**: `computer_use`, `generate_image`, `generate_video`, `record_screen`,
  `write_canvas`, `read_canvas`
- **SCM**: `connect_scm`, `blame_by_file_path`, `set_active_branch`, `pr_management`,
  `edit_pr_labels`
- **Bugbot / CI**: `report_bug`, `report_bugfix_results`, `record_ci_investigation_findings`
- **Multi-agente**: `SendMessage`, `send_to_user`, `send_final_summary`, `update_current_step`
- **Code tour**: `UpdatePrCodeTour`, `GetPrCodeTour`
- **Entorno cloud**: `setup_vm_environment`, `replace_env`, `fetch_cloud_agent_data`
- **Proyectos**: `create_project`, `start_project_updates`, `stop_project_updates`
- **Tareas**: `list_tracked_tasks`, `create_tracked_tasks`, `update_tracked_tasks`,
  `delete_tracked_tasks`, `update_tracked_task_statuses`, `update_tracked_task_tags`
- **Otros**: `fetch`, `reflect`, `await`, `truncated`, `apply_agent_diff`, `ai_attribution`,
  `manage_access`, `search_conversations`, `start_grind_execution`, `start_grind_planning`

### 5.2 Los esquemas de entrada: NO MEDIDO ⛔

Los JSON Schema de las tools nativas **no están en el cliente**: los genera el backend
(`api2.cursor.sh`) y llegan por RPC. Lo que sí está en el cliente es el `ToolSchema` MCP
estándar (ver §9).

### 5.3 Taxonomía de errores por tool (copiable)

`workbench.desktop.main.js:719`, para `delete_file`: `fileNotFound`, `notFile` (con
`actualType`), `permissionDenied`, `fileBusy`, `rejected` (con `reason`), `error` y `success`.
Los mensajes formateados son "File not found: {path}", "Not a file ({actualType}): {path}",
"Permission denied: {path}", "File busy: {path}", "Delete rejected: {reason}" y
"Unknown error".

Detalle: `docs/medidas/cursor/05_tools.md`.

---

## 6. CONTRATO 6 — Contexto: qué entra al prompt

### 6.1 El proto `agent.v1.RequestContext` (autoritativo)

`extensions/cursor-mcp/dist/main.js:212` — 54 campos. Los que responden a «qué entra al
prompt»: `rules` y `non_file_rules`; `file_contents` (mapa ruta→contenido);
`repository_info` y `repository_info_should_query_prod`; `git_repos` y
`git_status_info_complete`; `project_layouts`; `env` y `env_info_complete`; `tools`;
`mcp_instructions` y `mcp_info_complete`; `mcp_file_system_options`; `agent_skills` y
`agent_skills_info_complete`; `custom_subagents` y `custom_subagents_info_complete`;
`hooks_config` y `hooks_additional_context`; `skill_options`; `user_intent_summary`;
`user_permissions_auto_run`, `project_permissions_auto_run` y `admin_permissions_auto_run`;
`disabled_team_rules` y `admin_command_denylist`; `debug_mode_config`; y
**`system_prompt_override`**.

Que exista `system_prompt_override` demuestra que **el prompt del sistema tiene punto de
extensión** (y que su contenido está en el backend, no en el paquete).

### 6.2 Scopes

`workbench.desktop.main.js:694` — scopes de recursos: `project`, `user`, `workspaces`, `team`,
`sources`. Una segunda función de la misma línea decide qué tipos de recurso admiten scope
`team`: plugins, mcps, rules y commands sí; **skills, agents y hooks no** (el scope `team`
queda excluido para ellos).

### 6.3 `.cursorignore`

11 ocurrencias en `workbench.desktop.main.js` (líneas 746, 747, 914, 928, 17567, 17584, 17585,
19514, 20225, 20248, 20263) → existe y se respeta.
**NO MEDIDO**: los patrones exactos y su precedencia frente a `.gitignore`.

---

## 7. CONTRATO 7 — Puente con el exterior

### 7.1 Hosts

| Host | Para qué | Evidencia |
|---|---|---|
| `https://api2.cursor.sh` | **Backend del agente** | `agent-host-daemon/dist/bin/backend-endpoint.js:6` (constante `DEFAULT_BACKEND_ENDPOINT`) |
| `https://api.cursor.com` | Backend de la app «molten» | `molten/dist/renderer/assets/cursor-backend-ChqkVT9m.js:2` |
| `https://cursor.com` | Web, OAuth, docs | `cursor-mcp/dist/main.js:212`; `daemon.cjs:717` |
| `https://downloads.cursor.com` | Updates y **harness del agente** | `build-info.json` (campo `agentHarnessCdnUrl`) |
| `https://cursor.com/agents/mcp/oauth/callback` | Callback OAuth de MCP | `molten/dist/engine/engine.cjs:26` |
| `console.statsig.com` / `api.statsigcdn.com` | Feature flags | harvest de URLs |
| `http://169.254.169.254` / `169.254.170.2` | IMDS de nube | harvest de extensiones |

### 7.2 Autenticación

`agent-host-daemon/dist/bin/daemon.cjs`: `/auth/login_intent` (POST, línea 22) →
`/auth/poll` (POST, línea 27) → `/auth/exchange_user_api_key` (POST, línea 23, con cabecera
`Authorization: Bearer <apiKey>`; un **403** se interpreta como violación de la política de
sign-in).
**NO MEDIDO**: el cuerpo de `login_intent` y el refresco de tokens.

### 7.3 Telemetría: convenciones OTel GenAI ⭐

`out/vs/workbench/workbench.desktop.main.js:23` — nombres de span y atributos medidos:
`ai.response.toolCalls`, `ai.prompt.messages`, `ai.prompt.tools`, `ai.model.id`,
`ai.model.provider`, `ai.response.providerMetadata`, `ai.usage.cachedInputTokens`,
`ai.telemetry.functionId`, `ai.usage.completionTokens`, `ai.usage.promptTokens`,
`ai.toolCall.name`, `ai.toolCall.id`, `ai.toolCall.args`, `gen_ai.response.model`,
`gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens`.

Son las **semantic conventions de OpenTelemetry para GenAI**: adoptarlas da métricas
interoperables sin coste.

Sentry (650 ocurrencias, `workbench.desktop.main.js:4`) reporta errores; Statsig (242
ocurrencias) gestiona feature flags, con el flag de cliente `disable_statsig`
(`workbench.desktop.main.js:494`). **No hay cliente de telemetría de producto propio medido.**

### 7.4 Modelos

**NO MEDIDO el catálogo**: los ids de modelo no son constantes del cliente. El model picker
(`molten/dist/renderer/assets/glass-model-picker-DIQ_38mA.js:2`) consume la lista por RPC y
sólo referencia `https://cursor.com/docs/models`. Lo medido es el **mecanismo**: preferencias de
parámetros por modelo persistidas (`modelParameterPreferences`, con `modelId` y `parameters`),
diálogos de confirmación por modelo (`confirmationDialogue` con `presentation`, `key`, `title`,
`body`), billing **por uso** (la UI ofrece «Get More Usage» y la doc de Bugbot es
`usage-based-billing`), y modelo forzado por modo (`CustomSubagent.model` +
`force_default_model`).

### 7.5 Política de red MCP ⭐

`out/vs/workbench/workbench.desktop.main.js:792` (módulo `mcp-url-utils.ts`): allowlist de
**70 dominios** de servicios MCP de terceros, denylist, `mcpNetworkMode`,
`localAgentNetworkAllowlist`, `blockMessage` y longitud máxima de URL **2048**.
Validadores: hostname (etiqueta DNS, 1-63 chars), wildcard `*.dominio`, IPv4, IPv6, CIDR
`IP/máscara`, puerto `*` y host `*`.
Mensaje de bloqueo medido: `Blocked by MCP Network Admin Server Controls`.

Detalle: `docs/medidas/cursor/06_endpoints_contexto.md`.

---

## 8. CONTRATO 8 — Licencia, precios, condiciones

### 8.1 La EULA prohíbe explícitamente el RE

`usr/share/cursor/resources/app/LICENSE.txt` — «Cursor End User License Agreement», última
modificación 13 de junio de 2023, con Anysphere Inc. como parte.

- **§1.2**: licencia limitada, no exclusiva, **no transferible**, revocable, y **sólo para uso
  personal**.
- **§1.3**: prohíbe **copiar, modificar, adaptar, traducir, preparar obras derivadas, hacer
  ingeniería inversa, desensamblar o descompilar**, así como **redistribuir** a terceros.
- **§9.2/§9.3**: arbitraje individual obligatorio y renuncia a acción colectiva.

Consecuencia: *analizar* formatos y *reimplementar* un lector/escritor independiente es
distinto de *copiar código*. Este repositorio se queda en el primer plano.

### 8.2 Terceros

De `agent-host-daemon/dist/bin/daemon.cjs.LEGAL.txt` (11 025 B): `undici`/formdata-polyfill
(MIT), `ws` (MIT), `tough-cookie` (BSD-3-Clause), **`re2js` v1.2.2 (MIT — RE2 lineal)**,
`is-extendable` (MIT), `strip-bom-string` (MIT), `ieee754` (BSD-3-Clause), `image-q` (MIT).

⭐ **`re2js` es el motor del matcher de hooks de Cursor**: por eso su regex puede ser arbitraria
sin riesgo de backtracking. **Requisito para MegaDaemon: los matchers de hooks deben usar `re2`
o un motor lineal, nunca backtracking con `regex`.**

### 8.3 Precios: NO MEDIDO

No hay literales de precio ni de plan en los bundles: el producto pregunta al servidor. Lo
medido es que el billing es **por uso** y que existe una UI de *teams* que enlaza a
`cursor.com/privacy` y `cursor.com/terms-of-service`. **Las cifras, los planes y los límites de
cuota NO son recuperables del paquete** (requieren consultar la web con `curl`).

---

## 9. CONTRATO 9 — El «agent host daemon» ⭐

Detalle completo: `docs/medidas/cursor/09_daemon_protocolo.md` y
`docs/medidas/cursor/10_protocolo_mcp_stateless.md` (contrato de referencia del protocolo).

### 9.1 Arranque, flags y backend

`agent-host-daemon/dist/bin/daemon.js` es un entrypoint Node ESM que importa seis módulos
«at-start» (errores de stdio, fichero de entorno, endpoint de backend, guarda de build, sumidero
de datos y machine-id) y llama a `runDaemonProcess(process.argv.slice(2))`.

`backend-endpoint.js` define tres piezas: el flag `--backend-url`, la variable de entorno
`CURSOR_API_ENDPOINT` y el backend por defecto `https://api2.cursor.sh`. La resolución
devuelve explícitamente su **origen** (`flag` \| `environment` \| `default`) y su `endpoint`.
Rechazos literales: una credencial embebida, un esquema sin https, o un host que no sea de
Cursor («un daemon empaquetado sólo acepta otro backend por `--backend-url` en su línea de
comandos»).

### 9.2 `environment-file`: consume-and-destroy ⭐

`environment-file.d.ts` — el fichero de variables de entorno debe ser un objeto JSON de
nombres a valores, tener **modo 0600 y un solo enlace**, y pertenecer al usuario actual.
El daemon **lo borra tras consumirlo**, y lo borra **con el descriptor abierto**, comprobando
después que no queda ningún nombre. Cualquier rechazo deja el fichero donde estaba, para que
lo borre el lanzador. Hay además una lista de `STARTUP_VARIABLES` (las que sólo se leen durante
el arranque de un proceso: Node, libuv u OpenSSL) que alcanzan a los hijos pero no al daemon.

→ **Patrón de seguridad copiable tal cual** para repartir secretos a los workers de MegaDaemon.

### 9.3 El protocolo

JSON-RPC 2.0 sobre MCP **stateless** (HTTP). Sobre JSON-RPC y los mensajes: §10 del contrato de
referencia. En resumen: sobre de petición con `jsonrpc`, `id`, `method` y `params`;
notificaciones sin `id`; respuestas con `result` o con `error` {`code`, `message`, `data?`};
`RequestId` = string \| entero; error propio **`-32042 UrlElicitationRequired`**.

El handshake viaja en `params._meta` con tres claves: versión de protocolo, capacidades del
cliente e información del cliente.

### 9.4 Deadline-first y presupuestos de reintento ⭐

`daemon.cjs:13924` — la envoltura de una llamada lleva `requestID`, `name`, `arguments`,
`timeoutSeconds`, `startedAtEpochSeconds`, **`deadlineEpochSeconds`**, `mode` y `sessionId`.
→ **el deadline viaja en el mensaje**; el receptor no recalcula ni depende del reloj local.

`daemon.cjs:3647` — `AGENT_RETRY_CAPS` = {serverCap: 3, transportCap: 10};
`STALL_RETRY_BUDGET_MS` = 3 min; `RATE_LIMIT_RETRY_BUDGET_MS` = 5 min;
`MAX_RETRIES_CEILING` = 100; y `RETRY_BUDGET_BY_KIND`, que mapea cada tipo de fallo a un
responsable (server / transport / by_signature).
→ **copiable tal cual**.

### 9.5 Lo que NO se pudo extraer ⛔

- **El CLI `cursor-agent` no está en el `.deb`.** `usr/share/cursor/bin/` sólo trae `cursor`,
  `cursor-tunnel` y `code-tunnel`; `usr/bin/` no existe. Se distribuye aparte ⇒ **su protocolo
  JSON no es recuperable de este paquete**.
- Los **esquemas de entrada** de las tools nativas, el **catálogo de modelos**, los **precios**
  y el **prompt del sistema**: backend.
- La lógica de `apply_agent_diff`, `reflect`, `await` y `start_grind_*`: backend.
- El **runtime/harness del agente**: se descarga del CDN (`agentHarnessCdnUrl`), no está en el
  paquete ⇒ no es analizable sin red.

---

## 10. Tabla maestro: contrato → copiable por MegaDaemon → destino en `rust-core/src/`

Ficheros de destino ya existentes en el repo (tamaño actual medido):

| Fichero | Líneas | Bytes | Estado |
|---|---|---|---|
| `rust-core/src/protocol.rs` | 387 | 12 316 | existe |
| `rust-core/src/tool_policy.rs` | 400 | 13 705 | existe |
| `rust-core/src/skills.rs` | 455 | 16 960 | existe |
| `rust-core/src/mcp_ext.rs` | 3 904 | 216 088 | **existe** (no es nuevo) |
| `rust-core/src/mcp_server.rs` | 460 | 23 547 | **existe** (no es nuevo) |
| `rust-core/src/agent_core.rs` | 611 | 25 337 | existe |
| `rust-core/src/retry.rs` | — | — | **NO existe → nuevo** |

| Contrato | ¿Copiable? | Motivo | Destino exacto | Qué habría que escribir | Tamaño est. |
|---|---|---|---|---|---|
| **Hooks: 22 eventos** | **SÍ, tal cual** | Lista de nombres, sin lógica | `rust-core/src/protocol.rs` | `pub enum HookEvent` con las 22 variantes + `impl FromStr` | +120 L |
| **Hooks: formato `hooks.json`** | **SÍ, tal cual** | Trivial y compatible con Claude Code | `rust-core/src/protocol.rs` | `struct HookFile { version: u32, hooks: BTreeMap<HookEvent, Vec<HookSpec>> }`, `struct HookSpec { r#type, command, args, prompt, timeout, loop_limit, fail_closed }` + deserializador | +180 L |
| **Hooks: respuestas JSON** | **SÍ, tal cual** | Contrato estable | `rust-core/src/protocol.rs` | `enum HookResponse { BeforePrompt{continue_, user_message, additional_context}, BeforeCommand{permission, user_message, agent_message}, Read{permission, user_message}, PreToolUse{…}, PostToolUse{additional_context}, Stop{followup_message}, PreCompact{user_message}, WorkspaceOpen{plugin_paths}, Freeform }` con `serde(tag="type")` | +200 L |
| **Hooks: política deny>ask>allow** | **SÍ, tal cual** | La pieza más valuable | `rust-core/src/tool_policy.rs` | Nueva fn `resolver_hooks(perm_hook, perm_tool) -> PolicyDecision` que aplique deny-first; mapear `ask`↔`Prompt`. `PolicyDecision` ya existe con `Allow/Prompt/Deny` | +60 L |
| **Hooks: matcher regex** | **ADAPTABLE** | Reutilizable; **usar `re2`, no `regex`** | `rust-core/src/tool_policy.rs` | `fn matcher_coincide(matcher:&str, sujeto:&str)->bool` sobre `re2::Regex` (o `regex` con `RegexBuilder::size_limit` si no se puede añadir `re2`); **fail-open** si no compila; `*`/vacío ⇒ true | +50 L (+ dep `re2`) |
| **Hooks: compat Claude Code** | **ADAPTABLE** | Exige mantener equivalencias | `rust-core/src/protocol.rs` | `fn map_evento_claude(&str)->Option<HookEvent>` y `fn map_tool_claude(&str)->Option<&'static str>` | +80 L |
| **Hooks: ficheros y scopes** | **SÍ, tal cual** | Rutas medidas | `rust-core/src/protocol.rs` | Constantes `HOOKS_USUARIO`, `HOOKS_PROYECTO`, `HOOKS_EMPRESA_*`, `HOOKS_CLAUDE_*` y fn `rutas_hooks(workspace, home, os)` | +40 L |
| **MCP: esquema `mcp.json`** | **SÍ, tal cual** | Ya casi el estándar | `rust-core/src/mcp_ext.rs` (extiende) | `struct McpConfig { mcp_servers: BTreeMap<String, McpServer> }`, `enum McpServer { Stdio{command,args,env,env_file,cwd,enabled_tools,startup_timeout_sec}, Http{url,headers,auth,tls,placement,enabled_tools} }` + validación de la unión discriminada | +150 L |
| **MCP: `placement`** | **ADAPTABLE** | La idea sí; en MD el MCP corre en el worker | `rust-core/src/mcp_ext.rs` | `enum Placement { Server, Client }` + decisión de dónde arrancar el proceso | +40 L |
| **MCP: `symbolic_path_arguments`** | **ADAPTABLE (muy valioso)** | Evita filtrar rutas absolutas | `rust-core/src/protocol.rs` | `struct SymbolicPathArgument` + resolución en el worker | +60 L |
| **MCP: `McpPolicyBlockedReason`** | **SÍ, tal cual** | Taxonomía ya pensada | `rust-core/src/mcp_ext.rs` | `enum McpBlockedReason` con los 7 valores + el mensaje que ya se emite | +30 L |
| **MCP: `McpRefreshPolicy`** | **SÍ, tal cual** | Trivial y útil | `rust-core/src/mcp_ext.rs` | `enum McpRefreshPolicy` con los 3 valores | +20 L |
| **Allowlist MCP/terminal + autoRun** | **SÍ, tal cual** | Formato casi estándar | `rust-core/src/tool_policy.rs` | `struct Permissions { mcp_allowlist, terminal_allowlist, approval_mode, auto_run, auto_review }` + `fn mcp_permitido(&str,&str)->bool` con el patrón `servidor:tool` y sus comodines | +120 L |
| **Custom modes (`CustomSubagent`)** | **SÍ, tal cual** | 14 campos, diseño limpio y portable | `rust-core/src/protocol.rs` + **nuevo** `rust-core/src/agent_modes.rs` | `struct CustomMode { full_path, name, description, tools: Vec<String>, model, prompt, permission_mode, is_background, force_default_model, source }` y el cargador de `.cursor/agents/` | +180 L (nuevo ≈150 L) |
| **`permission_mode`** | **SÍ, tal cual** | 3 niveles muy usables | `rust-core/src/tool_policy.rs` | `enum PermissionMode { Default, Readonly, AgentOnly }` + su efecto en `evaluate_policy` | +50 L |
| **`TaskMode` AGENT/PLAN** | **SÍ, tal cual** | Ya existe el concepto | `rust-core/src/protocol.rs` | `enum TaskMode { Agent, Plan }` | +20 L |
| **Reglas: frontmatter `description`/`globs`/`alwaysApply`** | **SÍ, tal cual** | 3 campos triviales | `rust-core/src/skills.rs` | `struct ReglaCursor { description, globs: Vec<String>, always_apply: bool, tipo: RuleType }` + parser; `RuleType` con las 5 variantes | +140 L |
| **Regla `.md` = alwaysApply implícito** | **SÍ, tal cual** | Regla simple | `rust-core/src/skills.rs` | Al cargar un `.md` sin frontmatter: `always_apply = true`, `globs = []` | +10 L |
| **Skills: frontmatter de Cursor** | **ADAPTABLE** | Aceptar kebab-case **y** camelCase, y `allowed-tools`/`context`/`files` | `rust-core/src/skills.rs` | Ampliar `Skill` con `always_apply`, `disable_model_invocation`, `environments`, `disabled_environments`, `scoped_to`, `surfaces`, `paths`; alias de claves | +150 L |
| **Límites 64/1024/5000** | **SÍ, tal cual** | Ya parecidos | `rust-core/src/skills.rs` | Constantes `SKILL_NOMBRE_MAX=64`, `SKILL_DESCRIPCION_MAX=1024`, `SKILL_CUERPO_TOKENS_MAX=5000` + validaciones con los mensajes medidos | +60 L |
| **Lectura multi-ecosistema** | **SÍ, tal cual (estrategia)** | Lo más valuable: MD ya lee skills de Claude y Codex | `rust-core/src/skills.rs` | Ampliar `descubrir()` con las raíces `.agents/skills`, `.claude/skills`, `.codex/skills`, `.grok/skills` además de `.cursor/skills`, con deduplicación por nombre | +50 L |
| **73 nombres de tool** | **SÍ (nombres)** | Vocabulario de dominio | `rust-core/src/protocol.rs` | `pub const TOOLS_CURSOR: &[&str] = &[…73…]` + helper de normalización (`mcp-<prov>-<tool>` → `mcp__<prov>__<tool>`) | +100 L |
| **Esquemas de entrada de tools nativas** | **NO MEDIDO ⛔** | Backend | — | — | — |
| **Taxonomía de errores de tool** | **SÍ, tal cual** | Errores accionables | `rust-core/src/protocol.rs` | `enum ToolErrorKind { FileNotFound, NotFile{actual_type}, PermissionDenied, FileBusy, Rejected{reason}, Error{message} }` | +60 L |
| **`ToolSchema` MCP (+annotations, taskSupport)** | **SÍ, tal cual** | MCP estándar + 2 extensiones | `rust-core/src/mcp_server.rs` (extiende) | Añadir `output_schema`, `annotations` (5 hints) y `execution.task_support` al `tool_definitions()` que ya existe | +120 L |
| **JSON-RPC 2.0 + MCP stateless** | **SÍ, tal cual** | MCP ya es estándar | `rust-core/src/mcp_server.rs` (extiende) | `handle_rpc` ya existe: añadir paginación (`nextCursor`), `resources/read`, `prompts/get`, `logging/setLevel`, `notifications/progress`, transporte HTTP stateless (hoy sólo stdio) | +250 L |
| **Tareas asíncronas (`tasks/*`)** | **SÍ, tal cual** | `input_required` resuelve "necesito al usuario a mitad" | `rust-core/src/mcp_server.rs` + `rust-core/src/protocol.rs` | `struct TaskSchema { task_id, status, ttl, created_at, last_updated_at, poll_interval, status_message }`, `enum TaskStatus { Working, InputRequired, Completed, Failed, Cancelled }` y los 4 métodos `tasks/*` | +200 L |
| **Deadline en el mensaje** | **SÍ, tal cual** | Deadline-first, sin ambigüedad de reloj | `rust-core/src/protocol.rs` | `struct Deadline { started_at_epoch_s, deadline_epoch_s, timeout_s }` en el sobre de `tools/call` | +50 L |
| **Presupuestos de reintento** | **SÍ, tal cual** | Pensado; ahorramanship | **nuevo** `rust-core/src/retry.rs` | `struct RetryPolicy { server_cap, transport_cap, stall_budget_ms, rate_limit_budget_ms, ceiling }`, `enum FailureKind` (9 valores) y `fn budget_para(kind)->&'static str`, `fn decidir_reintento(...)` | **nuevo ≈ 150 L** |
| **`environment-file` consume-and-destroy** | **SÍ, tal cual (patrón)** | Patrón de seguridad, no de producto | `rust-core/src/api_key.rs` (o `secrets.rs` nuevo) | `fn take_environment_file(path) -> Result<Map,Error>`: validar nombre, modo 0600 y 1 enlace, abrir con `O_NOFOLLOW`, leer, **borrar con el fd abierto** y comprobar que no queda nombre | +120 L |
| **`backend-endpoint` (flag>env>default)** | **ADAPTABLE** | Patrón copiable; la política de hosts es de Cursor | `rust-core/src/api_key.rs` | `struct AppliedBackend { source: Source, endpoint: String }` con `Source::{Flag, Environment, Default}` y los rechazos por credencial/esquema/origen | +60 L |
| **`UrlElicitationRequired = -32042`** | **SÍ, tal cual** | Código JSON-RPC de la serie de implementación | `rust-core/src/protocol.rs` | `const ERR_URL_ELICITATION: i64 = -32042;` + emisor | +10 L |
| **Convenciones OTel GenAI** | **SÍ, tal cual** | Ya son estándar OTel | **nuevo** `rust-core/src/telemetry.rs` | Constantes con los 15 nombres de span/atributo `ai.*` y `gen_ai.*` + helper de etiquetado | **nuevo ≈ 120 L** |
| **Validador de URL MCP** | **SÍ, tal cual** | Regex defensivas | `rust-core/src/mcp_ext.rs` | `fn url_permitida(url, allowlist, denylist) -> bool` con las regex de hostname/wildcard/IPv4/IPv6/CIDR y el tope de 2048 | +120 L |
| **Política de red MCP** | **ADAPTABLE** | Mecanismo sí; la lista de 70 dominios es de producto | `rust-core/src/mcp_ext.rs` | `struct NetworkPolicy { mode, denylist, allowlist, block_message }` | +80 L |
| **Lista de 70 dominios MCP de Cursor** | **NO (producto)** | Puede servir de semilla, no publicarse | — | — | — |
| **`RequestContext` (54 campos)** | **ADAPTABLE (muy valioso)** | Copiar los campos; el protobuf no | `rust-core/src/agent_core.rs` (extiende) | `struct RequestContext { rules, file_contents: BTreeMap<PathBuf,String>, repository_info, git_repos, project_layouts, env, tools, mcp_instructions, agent_skills, custom_subagents, hooks_config, skill_options, user_intent_summary, permissions_auto_run, disabled_team_rules, admin_command_denylist, system_prompt_override }` | +150 L |
| **Scopes (`project/user/workspaces/team/sources`)** | **SÍ, tal cual** | Simple y útil | `rust-core/src/protocol.rs` | `enum ResourceScope` + `fn scope_admite(tipo, scope) -> bool` con la regla de que skills/agents/hooks no admiten `team` | +50 L |
| **Flujo de auth `login_intent`→`poll`→`exchange_user_api_key`** | **ADAPTABLE** | Patrón copy-paste-able | `rust-core/src/api_key.rs` | Los tres pasos + el `Bearer` + el manejo del 403 como violación de política | +100 L |
| **Catálogo de modelos y precios** | **NO MEDIDO ⛔** | Server-side | — | — | — |
| **`.cursorignore` vs `.gitignore`** | **NO MEDIDO ⛔** | Pendiente | — | — | — |
| **CLI `cursor-agent`** | **NO MEDIDO ⛔** | El binario no está en el `.deb` | — | — | — |
| **Prompt del sistema** | **NO MEDIDO ⛔** | Backend (sólo hay `system_prompt_override`) | — | — | — |
| **Manifiesto de extensión (formato VS Code)** | **NO** | MD tiene su propio registro | `plugins/` | — | — |
| **Proceso MCP aislado** | **SÍ, tal cual (patrón)** | Aislar código de terceros | `rust-core/src/mcp_server.rs` | Mantener el MCP en proceso aparte del principal | — |
| **Ingest NDJSON por HTTP** | **SÍ, tal cual** | Patrón útil para trazas | nuevo `rust-core/src/agent_traces.rs` | Endpoint HTTP de ingest NDJSON a un log de trazas | **nuevo ≈ 100 L** |

**Resumen de ficheros nuevos:** `rust-core/src/retry.rs` (~150 L),
`rust-core/src/agent_modes.rs` (~150 L), `rust-core/src/telemetry.rs` (~120 L) y, opcional,
`rust-core/src/agent_traces.rs` (~100 L). Todo lo demás **extiende** ficheros existentes.

---

## 11. Lo que NO se pudo extraer (resumen explícito)

| # | Qué | Por qué |
|---|---|---|
| 1 | **CLI `cursor-agent`** y su protocolo JSON de salida | El binario no está en el `.deb` (sólo `cursor`, `cursor-tunnel`, `code-tunnel`). Se distribuye aparte. |
| 2 | **Esquemas JSON de entrada de las tools nativas** | Los genera el backend y llegan por RPC; no hay constantes en el cliente. |
| 3 | **Catálogo de modelos** y **precios/plan/cuotas** | Server-side. |
| 4 | **Prompt del sistema del agente** | Backend. Sólo se ve el punto de extensión `system_prompt_override`. |
| 5 | **Lógica de `apply_agent_diff`, `reflect`, `await`, `start_grind_*`** | Backend. |
| 6 | **Runtime/harness del agente** | Se descarga del CDN (`agentHarnessCdnUrl`); no está en el paquete. |
| 7 | **Reglas/skills por defecto** | **No existen**: 0 ficheros `.mdc`. Hecho medido y cerrado. |
| 8 | **Detalle de `.cursorignore`** (patrones y precedencia) | Sólo se ha medido que se respeta (11 ocurrencias). |
| 9 | **MCP servers de terceros por defecto** | Sólo se ha medido la allowlist de dominios (70), no qué servidores concretos hay. |
| 10 | **Código de los bundles** | Minificado; y la EULA prohíbe copiarlo. No es el objetivo. |

---

## 12. Dónde está todo

| Qué | Ruta |
|---|---|
| Este informe | `docs/RE_CURSOR_CONTRATOS.md` |
| **Datos medidos por contrato** (rutas, líneas, tablas de campos, catálogos) | `docs/medidas/cursor/01…10_*.md` |
| Inventario del paquete (ruta, bytes; **sin contenido de ficheros**) | `artifacts/cursor/INVENTARIO.tsv` |
| Copia íntegra de los contratos, los ficheros verbatim y los datos crudos | **Drive privada** del proyecto (fuera del repo) |

---

## 13. Recomendaciones al root

1. **P0 — `hooks.json` + política deny>ask>allow**, con matcher sobre **`re2`** (nunca
   `regex` con backtracking). Encaja directo en `protocol.rs` + `tool_policy.rs`.
2. **P0 — `CustomSubagent` como modelo de custom mode** en un `agent_modes.rs` nuevo.
3. **P1 — deadline-first + presupuestos de reintento** (`retry.rs` nuevo): el patrón
   `deadlineEpochSeconds` y `RETRY_BUDGET_BY_KIND` son copiables tal cual.
4. **P1 — adoptar `.agents/skills`** como raíz neutral de skills, leyendo además
   `.claude/skills` y `.codex/skills` como ya hace Cursor.
5. **P2 — `RequestContext` como estructura explícita de contexto** en `agent_core.rs`, en
   lugar de armarlo de forma implícita.
6. **Pendiente de medición** (opcional): barrido de `.cursorignore` vs `.gitignore`, y
   `curl` a cursor.com/pricing para los precios.
