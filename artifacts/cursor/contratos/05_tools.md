# CONTRATO 5 — CATÁLOGO DE HERRAMIENTAS DEL AGENTE

Fuente: `out/vs/workbench/workbench.desktop.main.js:719`, función `I6u(e,t)`.
El bundle está minificado pero **los literales de string sobreviven**, así que el catálogo es exacto.

---

## 5.1 El mapa completo (73 tools + 1 dinámico)

Cita literal (línea 719):

```js
function I6u(e,t){switch(t){
  case"readToolCall":return"read_file_v2";
  case"deleteToolCall":return"delete_file";
  ...
}}
```

### Tabla `caso proto → nombre de tool en el cable`

| Caso proto (tool call) | Nombre de tool (wire) | Familia |
|---|---|---|
| `readToolCall` | `read_file_v2` | ficheros |
| `editToolCall` | `edit_file_v2` | ficheros |
| `deleteToolCall` | `delete_file` | ficheros |
| `lsToolCall` | `list_dir_v2` | ficheros |
| `grepToolCall` | `ripgrep_raw_search` | búsqueda |
| `globToolCall` | `glob_file_search` | búsqueda |
| `semSearchToolCall` | `semantic_search_full` | búsqueda (indexación de código) |
| `readLintsToolCall` | `read_lints` | ficheros |
| `shellToolCall` | `run_terminal_command_v2` | terminal |
| `writeShellStdinToolCall` | `write_shell_stdin` | terminal |
| `executeToolCall` | `execute` | terminal |
| `getMcpToolsToolCall` | `get_mcp_tools` | MCP |
| `mcpToolCall` | `mcp-<providerIdentifier>-<toolName>` | MCP (dinámico) |
| `mcpAuthToolCall` | `mcp_auth` | MCP |
| `listMcpResourcesToolCall` | `list_mcp_resources` | MCP |
| `readMcpResourceToolCall` | `fetch_mcp_resource` | MCP |
| `webSearchToolCall` | `web_search` | web |
| `webFetchToolCall` | `web_fetch` | web |
| `xSearchToolCall` | `x_search` | web |
| `todo_write` (`updateTodosToolCall`) | `todo_write` | planificación |
| `readTodosToolCall` | `todo_read` | planificación |
| `createPlanToolCall` | `create_plan` | planificación |
| `taskToolCall` | `task_v2` | subagentes |
| `createAgentToolCall` | `create_agent` | subagentes |
| `stopAgentToolCall` | `stop_agent` | subagentes |
| `sendToAgentToolCall` | `send_to_agent` | subagentes |
| `getAgentStatusToolCall` | `get_agent_status` | subagentes |
| `readAgentTranscriptToolCall` | `read_agent_transcript` | subagentes |
| `createGoalToolCall` | `create_goal` | goals |
| `updateGoalToolCall` | `update_goal` | goals |
| `fetchToolCall` | `fetch` | genérica |
| `reflectToolCall` | `reflect` | cognitiva |
| `awaitToolCall` | `await` | control |
| `truncatedToolCall` | `truncated` | control |
| `askQuestionToolCall` | `ask_question` | interacción |
| `switchModeToolCall` | `switch_mode` | modos |
| `applyAgentDiffToolCall` | `apply_agent_diff` | diffs |
| `computerUseToolCall` | `computer_use` | computer-use |
| `generateImageToolCall` | `generate_image` | multimodal |
| `generateVideoToolCall` | `generate_video` | multimodal |
| `recordScreenToolCall` | `record_screen` | computer-use |
| `searchConversationsToolCall` | `search_conversations` | historial |
| `connectScmToolCall` | `connect_scm` | SCM |
| `blameByFilePathToolCall` | `blame_by_file_path` | SCM |
| `setActiveBranchToolCall` | `set_active_branch` | SCM |
| `prManagementToolCall` | `pr_management` | SCM |
| `editPrLabelsToolCall` | `edit_pr_labels` | SCM |
| `reportBugToolCall` | `report_bug` | bugbot |
| `reportBugfixResultsToolCall` | `report_bugfix_results` | bugbot |
| `recordCiInvestigationFindingsToolCall` | `record_ci_investigation_findings` | CI |
| `aiAttributionToolCall` | `ai_attribution` | atribución |
| `sendMessageToolCall` | `SendMessage` | multi-agente |
| `sendToUserToolCall` | `send_to_user` | multi-agente |
| `sendFinalSummaryToolCall` | `send_final_summary` | multi-agente |
| `communicateUpdateToolCall` | `update_current_step` | multi-agente |
| `updateOriginPrCodeTourToolCall` | `UpdatePrCodeTour` | code tour |
| `getOriginPrCodeTourToolCall` | `GetPrCodeTour` | code tour |
| `writeCanvasToolCall` | `write_canvas` | canvas |
| `readCanvasToolCall` | `read_canvas` | canvas |
| `setupVmEnvironmentToolCall` | `setup_vm_environment` | cloud env |
| `replaceEnvToolCall` | `replace_env` | cloud env |
| `fetchCloudAgentDataToolCall` | `fetch_cloud_agent_data` | cloud env |
| `createProjectToolCall` | `create_project` | proyectos |
| `startProjectUpdatesToolCall` | `start_project_updates` | proyectos |
| `stopProjectUpdatesToolCall` | `stop_project_updates` | proyectos |
| `listTrackedTasksToolCall` | `list_tracked_tasks` | tareas |
| `createTrackedTasksToolCall` | `create_tracked_tasks` | tareas |
| `updateTrackedTasksToolCall` | `update_tracked_tasks` | tareas |
| `deleteTrackedTasksToolCall` | `delete_tracked_tasks` | tareas |
| `updateTrackedTaskStatusesToolCall` | `update_tracked_task_statuses` | tareas |
| `updateTrackedTaskTagsToolCall` | `update_tracked_task_tags` | tareas |
| `manageAccessToolCall` | `manage_access` | permisos |
| `startGrindExecutionToolCall` | `start_grind_execution` | interno/"grind" |
| `startGrindPlanningToolCall` | `start_grind_planning` | interno/"grind" |

### Tool dinámica de MCP (literal)

`workbench.desktop.main.js:719`:
```js
case"mcpToolCall":{
  if(e.tool.case==="mcpToolCall"){
    const i=e.tool.value.args,
          n=i?.providerIdentifier||"",
          r=i?.toolName||i?.name||"";
    return`mcp-${n}-${r}`
  }
  return"mcp"}
```
→ El nombre de una tool MCP es **`mcp-<providerIdentifier>-<toolName>`**. Ej: `mcp-github-create_issue`.
→ Esto **encaja exactamente** con la convención `mcp__server__tool` de Claude Code, pero con guiones.
MegaDaemon debería normalizar a `mcp__<server>__<tool>` (con doble underscore) por compatibilidad.

---

## 5.2 Esquemas de entrada de las tools: DÓNDE ESTÁN

**NO MEDIDO como JSON Schema en el cliente.** El catálogo de tools con sus esquemas se
resuelve en el **backend** (`api2.cursor.sh`). Lo que sí está en el cliente es:

1. El **nombre** de cada tool (73 nombres, tabla anterior).
2. La **forma del resultado** (proto, con taxonomía de errores).
3. Los esquemas **MCP** de tools externas (`ToolSchema` Zod en el daemon, ver 09).

Razón: la petición `RequestContext` (`cursor-mcp/dist/main.js:212`) lleva los schemas como
`Value` de protobuf desde el servidor, no como constantes del cliente.

**Consecuencia práctica para MegaDaemon**: el catálogo de tools **no es copiable**; hay que
**reconstruirlo** a partir de (a) los 73 nombres, (b) el MCP `ToolSchema` estándar, y
(c) las tools propias de Cursor que sí están documentadas por el daemon.

### Tools MCP estándar (`ToolSchema`) — `agent-host-daemon/dist/bin/daemon.cjs:14918`

```js
ToolSchema=object({
  ...BaseMetadataSchema.shape,          // name, title
  ...IconsSchema.shape,
  description:string().optional(),
  inputSchema:object({
    type:literal("object"),
    properties:record(string(),AssertObjectSchema).optional(),
    required:array(string()).optional()
  }).catchall(unknown()),
  outputSchema:object({
    type:literal("object"),
    properties:record(string(),AssertObjectSchema).optional(),
    required:array(string()).optional()
  }).catchall(unknown()).optional(),
  annotations:ToolAnnotationsSchema.optional(),
  execution:ToolExecutionSchema.optional(),
  _meta:record(string(),unknown()).optional()
})

ToolAnnotationsSchema=object({
  title:string().optional(),
  readOnlyHint:boolean().optional(),
  destructiveHint:boolean().optional(),
  idempotentHint:boolean().optional(),
  openWorldHint:boolean().optional()
})

ToolExecutionSchema=object({
  taskSupport:_enum(["required","optional","forbidden"]).optional()
})
```

`CallToolResultSchema` (`daemon.cjs:14919`):
```js
CallToolResultSchema=ResultSchema.extend({
  content:array(ContentBlockSchema).default([]),
  structuredContent:record(string(),unknown()).optional(),
  isError:boolean().optional()
})
CallToolRequestParamsSchema=TaskAugmentedRequestParamsSchema.extend({
  name:string(), arguments:record(string(),unknown()).optional()
})
```

---

## 5.3 Taxonomía de resultados (errores) por tool

`workbench.desktop.main.js:719`, función que traduce el resultado proto a texto de error.

### `delete_file` (ejemplo canónico)

```js
case"fileNotFound":  return`File not found: ${t.result.value.path}`;
case"notFile":       {const i=t.result.value; return`Not a file (${i.actualType}): ${i.path}`}
case"permissionDenied": return`Permission denied: ${t.result.value.path}`;
case"fileBusy":      return`File busy: ${t.result.value.path}`;
case"rejected":      return`Delete rejected: ${t.result.value.reason}`;
case"error":         return t.result.value.error||"Unknown error";
case"success":       return null;
```

→ **Copia tal cual**: es una taxonomía de errores de fichero ideal para un daemon
(`fileNotFound` / `notFile` / `permissionDenied` / `fileBusy` / `rejected` / `error` / `success`).
Los `case` del proto son **discriminados en minúsculas** (`fileNotFound`, `permissionDenied`).

### `ask_question`

```js
case"error":   return t.result.value.errorMessage||"Unknown error";
case"success": case"async": case"rejected": return null;
```

### `read_file_v2` / `semantic_search_full` / `read_lints`

```js
case"error":   return t.result.value.errorMessage||"Unknown error";
case"success": return null;
```

---

## 5.4 Las tools propias de Cursor que SÍ son locales

| Extensión | Tamaño | Qué es |
|---|---|---|
| `extensions/cursor-browser-automation/` | 641 KB | **MCP server** de automatización de navegador. `package.json`: *"MCP server for browser automation in {productName}"*, `main: ./dist/extension.js`, `extensionKind: ["ui"]` |
| `extensions/cursor-computer-use/` | 2,1 MB | *"Computer use in Cursor"*, `main: ./dist/extension.js`, `extensionKind: ["ui"]` |
| `extensions/cursor-retrieval/` | 45 MB | indexación y retrieval de código (`semantic_search_full`) |
| `extensions/cursor-agent-exec/` | 34 MB | ejecución local de tools del agente |
| `extensions/cursor-agent-host/` | 57 MB | orquestación del agente + daemon |
| `extensions/cursor-local-agent-runtime/` | 8 MB | *"Hosts Cursor Private Inference outside workspace extension hosts"* |

---

## 5.5 Valoración para MegaDaemon

| Aspecto | Valoración | Motivo |
|---|---|---|
| Los **73 nombres** de tool | **COPIABLE tal cual** | Son un vocabulario de dominio, no código. Aporta interoperabilidad si alguien escribe un cliente de esas tools. |
| Convención `mcp-<provider>-<tool>` | **ADAPTABLE** | Normalizar a `mcp__<server>__<tool>` para no romper a Claude Code. |
| Taxonomía de errores (`fileNotFound`/`notFile`/`permissionDenied`/`fileBusy`/`rejected`) | **COPIABLE tal cual** | Excelente para un daemon: errores accionables y distinguibles. |
| `ToolSchema` MCP (input/output schema + annotations + taskSupport) | **COPIABLE tal cual** | Es el MCP estándar con las extensiones `annotations` y `execution`. |
| Esquemas de entrada de las tools de Cursor | **NO MEDIDO** | Viven en el backend. Hay que reconstruirlos. |
| Nombres con `Snake_Case` o `CamelCase` mezclados (`SendMessage`, `UpdatePrCodeTour` vs `todo_write`) | **NO COPIABLE tal cual (fuga)** | Es una inconsistencia real del producto. MegaDaemon debe normalizar a snake_case y mapear, no clonar el desorden. |
| `start_grind_*` | **NO COPIABLE** | Nomenclatura interna de Cursor, sin semántica pública. |

**Destino en MegaDaemon**: `rust-core/src/protocol.rs` (enum de tools),
`rust-core/src/tool_policy.rs` (permisos por tool), `rust-core/src/mcp_server.rs` (`ToolSchema`).
