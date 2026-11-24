# Arquitectura agéntica de MegaDaemon — investigación y hoja de ruta (2026-11-24)

> Encargo: usar referencias públicas (sobre todo `openai/codex`) para
> **mejorar las capacidades agénticas de MegaDaemon**, no para citarlas.
> Este documento es la investigación **medida** (lo que se ha leído en esta
> sesión) + el hueco real del repo + la hoja de ruta con su puerta de
> aceptación. El primer corte de código va **debajo**, en §6: ya está en el
> repo, no es un plan.
>
> **Alcance honesto:** en esta sesión se han medido `openai/codex`
> (`codex-rs`, `openai/plugins`, `openai/skills`) **y Claude Code**
> (`AprilNEA/claude-code-source`, que es el código recuperado del mapa de
> fuentes de `@anthropic-ai/claude-code@2.1.88`; y
> `leeyeel/claude-code-sourcemap`, la versión 0.2.8 con los mapas
> extraídos). Las referencias de descompilación de Codex (`JimLiu/decode-codex`,
> `kohoj/codex-app-teardown`, `NeuZhou/awesome-ai-anatomy`,
> `LEON-gittech/Open-Codex`) **quedan por leer**: no se afirma nada de ellas
> aquí. `codeaashu/claude-code` se ha medido sólo de pasada: su rama por
> defecto es un proyecto derivado («codemaster»), no el código de Anthropic
> ⇒ no se le atribuye ninguna característica (y su rama `backup` con la
> fuente original no es accesible desde la API de GitHub).

---

## 1. Qué se ha medido (fuente)

| referencia | cómo se ha medido | qué sale |
|---|---|---|
| `openai/codex` → `codex-rs/` | listado del API de GitHub | **132 crates** en un solo workspace |
| `openai/codex` → `codex-rs/README.md` | lectura | el README es un enlace a la documentación; la arquitectura está en el árbol de crates |
| `openai/plugins` → `README.md` | lectura | manifiesto `.codex-plugin/plugin.json` + superficies `skills/`, `.app.json`, `.mcp.json`, `agents/`, `commands/`, `hooks.json`, `assets/`; *marketplace* en `.agents/plugins/marketplace.json` |
| `openai/skills` → `README.md` | lectura | **el repo está DEPRECADO** (remite a `openai/plugins`); las skills son carpetas de instrucciones + scripts + recursos, con niveles `.system`, `.curated`, `.experimental` y un estándar abierto (`agentskills.io`) |

Lo que **no** se ha medido: el contenido de cada crate (132 lecturas no caben
en la cuota). Las afirmaciones sobre *qué hace* cada crate salen de su nombre
y de su grupo, y se marcan como tales.

---

## 2. La arquitectura de Codex, en 7 grupos

Del árbol real de `codex-rs` (132 crates) sale una estructura reconocible:

1. **Núcleo del agente**: `core`, `core-api`, `agent-core` (roles, identidad),
   `agent-loop`, `agent-adapters`.
2. **Contrato para clientes** (lo importante para el GUI):
   `app-server`, `app-server-protocol`, `app-server-transport`,
   `app-server-client`, `app-server-remote-client`, `app-server-daemon`,
   `app-server-test-client` ⇒ **un servidor de aplicación con protocolo
   tipado, transporte y cliente**, de modo que un IDE, un TUI, un cliente
   remoto o un daemon talk to the same core.
3. **Ejecución no interactiva**: `exec`, `exec-server`, `exec-server-cli`,
   `exec-server-protocol` ⇒ el mismo core, pero con otro transporte para
   usarlo en scripts (el equivalente de `megadaemon-core chat`).
4. **Extensiones**: `plugin`, `plugin-host`, `core-plugins`, `core-plugin-common`,
   `ext`, `connectors`, `skills`, `hooks` ⇒ capacidades que se añaden
   **sin recompilar**: manifiesto + skills + hooks + servidores MCP declarados.
5. **Multiagente**: `message-board`, `message-board-mcp`,
   `agent-message-board-client/host`, `agent-graph-store`, `agent-identity`,
   `agent-roles`.
6. **Estado y trazabilidad**: `thread-store`, `thread-manager-sample`,
   `history`, `message-history`, `rollout`, `rollout-trace`, `state`,
   `session`/historial.
7. **Seguridad y política**: `sandboxing`, `linux-sandbox`,
   `windows-sandbox-rs`, `bwrap`, `mxc-sandbox`, `execpolicy`,
   `execpolicy-rules`, `process-hardening`, `secrets`, `keyring-store`,
   `user-verification`.

**La regla que se ve en todo el árbol**: *el núcleo del agente no sabe nada
del cliente*. El TUI, la CLI, el `exec` y el `app-server` son **transportes**
del mismo núcleo. Por eso Codex puede tener un TUI, un IDE, un daemon y un
modo script sin reimplementar el agente.

---

## 3. Qué tiene MegaDaemon hoy (medido en el repo)

| capacidad | dónde está | equivalente en Codex |
|---|---|---|
| API compatible con OpenAI | `rust-core/src/cockpit_server.rs:1378` → `llm_gateway::chat_completions` | `responses-api-proxy`, `codex-api` |
| Dialecto Gemini (`/v1internal:*`) | `cockpit_server.rs:1416` | `codex-api` |
| Chat por CLI | `megadaemon-core chat` | `exec`, `exec-server-cli` |
| Servidor MCP por stdio | `rust-core/src/mcp_server.rs`, `mcp_ext.rs` | `codex-mcp`, `rmcp-client` |
| Herramientas MCP | ~100 nombres en el catálogo (`root_exec`, `fs_read`, `fs_write`, `http_fetch`, `drive_backup`, `sqlite_mitm`…) | `tools`, `apply-patch`, `file-search` |
| Tablón de tareas | `board.rs` | `agent-graph-store` |
| Bus de mensajes | `service_bus.rs` (con `tool_allowlist`) | `message-board` |
| Delegación entre agentes | `delegate.rs`, `agent_coding.rs`, `agent_loop.rs` | `agent-roles`, `agent-adapters` |
| Identidad | `service_identity.rs` | `agent-identity` |
| Sesiones (del GUI de Antigravity) | `antigravity_sessions.rs` | `thread-store` |
| Automatización del dispositivo | `automation*.rs`, `cron.rs`, `energy_automation.rs`, `comms_automation.rs` | (Codex no tiene esto: es la ventaja del móvil) |
| Interfaz web | `webui.rs` (5.014 líneas) | (Codex: `tui`, más un `app-server` para los IDE) |

**Ventaja de MegaDaemon**: el daemon *es* el producto y vive en un móvil con
root: puede lanzar apps, leer la pantalla, medir el término o hablar con
Tasker. Codex es un cliente de escritorio de un modelo remoto. La
integración que pide el jefe («usarme con la API a través de MegaDaemon
conectado a un GUI») es aquí más fácil de lo que sería al revés.

---

## 4. Los seis huecos (con evidencia)

| # | hueco | evidencia en el repo | referencia Codex |
|---|---|---|---|
| H1 | **No hay contrato tipado para los clientes.** Los endpoints HTTP son JSON ad-hoc y no declaran versión ⇒ un GUI no puede apoyarse en nada estable | `cockpit_server.rs:1378`, `:1416`, `:2292` (tres formas distintas) | `app-server-protocol` + `-transport` + `-client` |
| H2 | **No hay plugins ni skills**: toda capacidad está compilada dentro; añadir una herramienta = recompilar + CI + release | no hay módulo de skills/plugin en `rust-core/src/` | `plugin`, `plugin-host`, `core-plugins`, `skills`, `hooks` |
| H3 | **No hay hilo/sesión propio del agente**: `antigravity_sessions.rs` son las sesiones del GUI ajeno; las del daemon no son un artefacto de primera clase | `antigravity_sessions.rs` | `thread-store`, `rollout`, `rollout-trace`, `history` |
| H4 | **No hay identificador ni tiempos por llamada de herramienta**: en la auditoría del LLM hubo que inventar campos de traza a mano (`provider_trace`) | `providers.rs` (`provider_trace`) | `tool-call-id`, `direct-tool-timing` |
| H5 | **No hay política de ejecución por comando**: hay una lista de herramientas permitidas en el bus, pero no reglas *permitido/preguntar/denegado* por comando | `service_bus.rs:307` (`tool_allowlist`) | `execpolicy`, `execpolicy-rules`, `sandboxing` |
| H6 | **No hay descubrimiento de habilidades en tiempo de ejecución**: el agente no puede cargar «instrucciones + scripts + recursos» de una carpeta | — | `skills` + `agentskills.io` |

---

## 5. Hoja de ruta con puertas de aceptación

### P0 · Protocolo tipado del app-server (empezado en §6)
Un `protocol.rs` con versión, sobre y tipos; el GUI y el cliente de
Antigravity hablan ese contrato, no JSON suelto.
**Puerta**: el cliente negotiate la versión en `initialize` y recibe una
respuesta de error si no coincide; el protocolo tiene tests de ida y vuelta
**sin red**; el `cockpit` expone `POST /app` con el mismo contrato.

### P1 · Plugins y skills (el motor de «capacidades agénticas»)
Un manifiesto por plugin (el patrón de `.codex-plugin/plugin.json`): `skills/`
(carpetas con instrucciones y scripts), `.mcp.json` (servidores MCP que se
arrancan), `commands/`, `hooks.json`, `assets/`. Un `marketplace.json` para
instalar de una lista.
**Puerta**: se instala un plugin desde un directorio local y sus skills
aparecen en el catálogo del daemon **sin recompilar**; una skill mal
formada da un error legible y no rompe el arranque.

### P2 · Hilos y trazas del agente (`rollout`)
Cada sesión del agente deja un artefacto consultable: qué pidió, qué
herramientas llamó, qué costó, qué cambió.
**Puerta**: una sesión se puede reabrir y seguir; la traza se puede leer
desde la UI y desde el MCP.

### P3 · Identificador y tiempos por herramienta
Un `tool_call_id` que viaje por el bus y el MCP, con tiempos por llamada.
**Puerta**: el mismo `tool_call_id` aparece en la traza, en el log del
daemon y en la respuesta al cliente.

### P4 · Política de ejecución
Reglas por comando con tres estados (permitido / preguntar / denegado), al
estilo de `execpolicy-rules`, evaluadas **antes** de ejecutar (no después).
**Puerta**: un comando denegado no se ejecuta y el motivo vuelve al cliente
por el mismo camino que cualquier otro error de herramienta.

---

## 6. Lo que YA está implementado en este commit

`rust-core/src/protocol.rs` (nuevo) + `pub mod protocol;` en `lib.rs`:

* `PROTOCOL_VERSION = "1.0"`, `PROTOCOL_NAME = "megadaemon.app-server"`.
* `Request` / `Response` / `Error` con las reglas de JSON-RPC 2.0 (el error
  nunca va junto al `result`).
* Códigos de error (`PARSE_ERROR`, `INVALID_REQUEST`, `METHOD_NOT_FOUND`,
  `INVALID_PARAMS`, `INTERNAL`).
* Tipos de `initialize` (con `comprobar_handshake`: la versión del cliente y
  la del daemon tienen que coincidir), `session/create`, `session/message`,
  `tool/call`.
* La lista `METHODS` como fuente única de verdad y `tiene_metodo`.
* **7 tests unitarios** sin red.

Es el **contrato**, no el transporte: de momento no se mete en el `cockpit`
(eso es el siguiente corte, P0-continuación). Con esto, el GUI puede empezar
a hablar con un esquema versionado en vez de adivinar JSON.

---

## 7. Riesgos

1. **H1 y el `cockpit`**: `cockpit_server.rs` es un fichero grande y tocado
   por varios dueños ⇒ el siguiente corte (exponer `POST /app`) debe ser un
   commit pequeño y propio.
2. **H2 (plugins)**: ejecutar código de un plugin es ejecutar código
   ajeno ⇒ el manifiesto debe declarar permisos y el daemon debe aplicar la
   política de P4 **antes** de que el plugin exista.
3. **Cuota**: el plan completo (P0..P4) es de varios días de agente; con la
   cuota semanal corta, el orden importa: P0 habilita el GUI (lo que pidió el
   jefe), P1 hace que las capacidades crezcan sin CI.

---

## 8. Trazabilidad

| afirmación | de dónde sale |
|---|---|
| 132 crates en `codex-rs` | listado del API de GitHub (`/contents/codex-rs`), 2026-11-24 |
| manifiesto de plugin y marketplace | `openai/plugins` README, 2026-11-24 |
| skills: estándar y niveles; repo deprecado | `openai/skills` README, 2026-11-24 |
| gateway, dialecto Gemini y `/api/chat` | `rust-core/src/cockpit_server.rs:1378`, `:1416`, `:2292` |
| lista de herramientas del bus | `rust-core/src/service_bus.rs:307` |
| módulos de agente, tablón, bus, delegación | `rust-core/src/{agent_core,agent_loop,agent_coding,agent_voice,agent_adapters,board,service_bus,delegate,meta_agent,service_identity}.rs` |
| protocolo nuevo | `rust-core/src/protocol.rs` |
| tarea principal del agente (auditoría del chat/LLM) | `docs/AUDIT_LLM_V16.md` |


---

## 9. Claude Code (medido) — la segunda referencia

Del árbol real de `src/` en el código recuperado de `@anthropic-ai/claude-code@2.1.88`:

**Módulos**: `QueryEngine.ts`, `Task.ts`, `Tool.ts`, `query/`, `tools/`,
`commands/`, `hooks/`, `skills/`, `plugins/`, `permissions.ts` (0.2.8),
`history.ts`, `cost-tracker.ts`, `costHook.ts`, `context.ts`, `memdir/`,
`state/`, `migrations/`, `tasks/`, `assistant/`, `coordinator/`, `server/`,
`remote/`, `bridge/`, `upstreamproxy/`, `services/`, `entrypoints/`,
`components/`, `screens/`, `ink/`, `keybindings/`, `vim/`, `voice/`,
`outputStyles/`, `native-ts/`, `vendor/`, `schemas/`, `types/`, `utils/`.

**Las 44 herramientas** (las que el agente puede invocar) son el corazón del
diseño: `AgentTool`, `AskUserQuestionTool`, `BashTool`, `PowerShellTool`,
`REPLTool`, `FileReadTool`, `FileEditTool`, `FileWriteTool`,
`NotebookEditTool`, `GlobTool`, `GrepTool`, `LSPTool`, `WebFetchTool`,
`WebSearchTool`, `MCPTool`, `McpAuthTool`, `ListMcpResourcesTool`,
`ReadMcpResourceTool`, `MCPTool`, `TodoWriteTool`, `TaskCreateTool`,
`TaskGetTool`, `TaskListTool`, `TaskOutputTool`, `TaskStopTool`,
`TaskUpdateTool`, `TeamCreateTool`, `TeamDeleteTool`, `SendMessageTool`,
`SkillTool`, `ToolSearchTool`, `ConfigTool`, `EnterPlanModeTool`,
`ExitPlanModeTool`, `EnterWorktreeTool`, `ExitWorktreeTool`,
`ScheduleCronTool`, `SleepTool`, `BriefTool`, `SyntheticOutputTool`.

### Lo que aporta y Codex no tenía

| # | capacidad de Claude Code | para qué | estado en MegaDaemon |
|---|---|---|---|
| H7 | `EnterPlanModeTool` / `ExitPlanModeTool` | el agente **propone un plan y espera** antes de tocar nada | ❌ no existe |
| H8 | `EnterWorktreeTool` / `ExitWorktreeTool` (+ el crate `worktree` de Codex) | el trabajo aislado es una **herramienta**, no una operación manual | ⚠️ hay worktrees en el entorno, no es una herramienta del daemon |
| H9 | `ToolSearchTool` | con ~100 herramientas, el agente **las busca bajo demanda** en vez de llevarlas todas en el contexto | ❌ el catálogo se sirve entero |
| H10 | `AskUserQuestionTool` (+ `user-verification` de Codex) | el agente **pregunta** a la persona a mitad de la tarea | ❌ |
| H11 | `McpAuthTool`, `ListMcpResourcesTool`, `ReadMcpResourceTool` | cliente MCP **con autenticación y recursos** (Codex: `rmcp-client`) | ❌ el daemon es servidor MCP, no cliente |
| H12 | `memdir/` | un directorio de memoria por proyecto, legible y editable por el agente | ⚠️ hay tablón y conversaciones, no una memoria por proyecto |

### Lo que las DOS referencias coinciden (esto es la señal fuerte)

Codex (132 crates) y Claude Code (fuente recuperada) **coinciden** en siete
puntos. Cuando dos implementaciones independientes hacen lo mismo, es el
patrón que hay que copiar:

1. **Núcleo del agente separado del cliente** (Codex: `core` + `app-server` +
   `exec`; Claude: `QueryEngine` + `query/` + `entrypoints/` + `server/`).
2. **Un contrato de protocolo para los clientes** (Codex:
   `app-server-protocol`; Claude: `server/` + `remote/` + `bridge/`).
3. **Extensiones sin recompilar** (Codex: `plugin`, `plugin-host`, `skills`;
   Claude: `plugins/`, `skills/`, `SkillTool`).
4. **Ganchos del ciclo de vida** (Codex: `hooks`; Claude: `hooks/`,
   `costHook.ts`).
5. **Política y permisos** (Codex: `execpolicy`, `execpolicy-rules`,
   `sandboxing`; Claude: `permissions.ts`).
6. **Multiagente con identidad y tablón** (Codex: `message-board`,
   `agent-identity`, `agent-roles`; Claude: `TeamCreateTool`,
   `SendMessageTool`, `AgentTool`, `Task*Tool`).
7. **Sesiones e historial persistentes** (Codex: `thread-store`, `rollout`;
   Claude: `history.ts`, `state/`, `migrations/`).

⇒ **El orden de trabajo que sale de las dos fuentes es el mismo:**
protocolo (P0) → skills/plugins (P1) → ganchos y política (P4) →
sesiones (P2) → instrumentación (P3). Y las novedades que sólo aporta
Claude Code (H7 plan mode, H9 búsqueda de herramientas, H10 preguntar a la
persona) encajan justo en el sitio donde el agente de MegaDaemon más falta:
trabaja sobre un móvil compartido, con root, sin la persona al lado.

### Prioridad revisada

| orden | pieza | por qué |
|---|---|---|
| 1º | **P0 protocolo** | sin contrato no hay GUI ni cliente remoto (las dos referencias lo hacen así) |
| 2º | **P1 skills + plugins** | es lo que hace que las capacidades **crezcan sin CI** (Codex y Claude Code) |
| 3º | **H7 modo plan** + **H10 preguntar** | el agente de un móvil con root no puede acting sin confirmar |
| 4º | **P4 política de ejecución** | con root y plugins, el «permitido / preguntar / denegado» es obligatorio |
| 5º | **P2 sesiones y trazas** | sin traza no hay confianza ni resuming |
| 6º | **H9 búsqueda de herramientas** | con 100 herramientas el contexto se desborda |
