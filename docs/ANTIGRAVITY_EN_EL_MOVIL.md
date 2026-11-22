# ANTIGRAVITY: lo que hay dentro (medido en el móvil, 2026-11-21/22)

Todo lo de aquí está **medido sobre el S21 Ultra con root** (vía `root_exec`
del MCP del móvil), no deducido.

## 1. Qué es y cómo corre

| Dato | Valor medido |
|---|---|
| Proceso | `libloader.so --preload …/liblse_compat.so …/runtime/bin/liblanguage_server.so --standalone --override_ide_name antigravity --subclient_type hub --override_ide_version 2.11.0 --https_server_port 45157 --csrf_token <uuid> --app_data_dir antigravity-app --web_bundle_path …/files/web --enable_sidecars` |
| RSS | **430 MB** (`440392 kB`) — el proceso más pesado del móvil |
| Puerto | `127.0.0.1:45157` **HTTPS** (cert autofirmado ⇒ `curl -k`) |
| Web | `/` sirve la SPA con `window.__APP_CONFIG__ = {productName:"antigravity-app", csrfToken:"…", appVersion:"2.11.0", releaseChannel:"stable", devMode:false}` |
| Ojo | `/mcp` y `/tools` devuelven 200 pero son el **catch-all de la SPA**: NO son un MCP |
| Directorio de app | `/data/user/0/com.agy/files/` (contiene `runtime/`, `web/`, `.gemini/`, `scummvm/`) |

## 2. El backlog (recuperado)

`/data/user/0/com.agy/files/.gemini/antigravity-app/`

| Ruta | Contenido |
|---|---|
| `daemon/tasks.json` | 2091 B. Tarea `task_442838` **"Snatcher-MegaDaemon-SuperLoop"**: `python3 …/scummvm/tools/re_pipeline/snatcher_megadaemon.py --once`, trigger **interval 300 s**, `enabled:true`, `last_run_status:success`, ciclo **#37**, log *"SYSTEM TUNING: ELEVATED HARDWARE OPTIMIZATION & ROOT GOVERNOR"* |
| `daemon/daemon.log`, `daemon/daemon.pid` | log y pid del scheduler |
| `antigravity_state.pbtxt` | onboarding completado (4 pasos), `last_selected_agent_model: MODEL_PLACEHOLDER_M319`, `installation_uuid: 5b21c18a-32a1-46bf-95f3-19a809c25df0`, migraciones 3/4/5 completadas |
| `jetski_state.pbtxt` | estado de "jetski" |
| `brain/` | 29 subdirectorios |
| `conversations/` | directorio grande (8192 B de entradas) |
| `annotations/`, `crashes/`, `bin/`, `builtin/` | anotaciones, volcados de crash, binarios y recursos |
| `mcp/` | los plugins MCP que usa la IDE (abajo) |

## 3. El puente ya existe (y es bidireccional)

`mcp/` contiene los plugins que Antigravity carga:

| Plugin | Tools |
|---|---|
| `megadaemon-minimax` | **52 definiciones de tool de MegaDaemon** (adb, agent, alerts, api_keys, apk_inventory, app_control, app_intel, battery_saver, bookmarks, capabilities, conversation_stats, dex_decompile, env_introspect, find_duplicates, fs_read, fs_write, get_conversation, git_ops, github, harvest_sessions, identity_and_web, ingest_export, kernel_and_blocks, list_conversations, magisk_module, megadaemon_catalog, megadaemon_cirx_analyze, megadaemon_die_binwalk, megadaemon_mba_simplify, megadaemon_npu_cv, megadaemon_rizin, megadaemon_voice, messaging, meta_route, minimax_chat, profile, props_and_settings, repo_updates, root_exec, root_hide, root_stealth, screen_and_ui, search_conversations, selinux_control, service_bus, signing_vault, ssh_remote, swarm_chat, sync_conversations, sync_web_conversations, system_overview, task_queue) + `instructions.md` |
| `data-agent-kit-plugin_dak-notebook` | create_notebook, delete_cell, get_cell_outputs, get_notebook_info |
| `data-agent-kit-plugin_dak-viz` | (a medir) |
| `gemini-api_gemini-api-docs` | gemini_get_doc, gemini_search_docs + instructions.md |

⇒ **Antigravity ya puede llamar a las tools de MegaDaemon** (52), y el
formato de definición es el mismo que usa nuestro MCP (`name`,
`description`, `parameters`).

## 4. El hallazgo que explica el "catálogo corto"

El daemon del móvil corre un binario **viejo** (8.859.456 B) frente a la
release actual (11.928.296 B) ⇒ su MCP anuncia **26 tools**, mientras que el
plugin de Antigravity define **52**. Al actualizar el binario (lo hace el
instalador) el móvil expone el catálogo completo.

## 5. Qué falta por medir

- La **API real** del servidor de la IDE (más allá de la SPA): la están
  descubriendo `md-antigravity` (en curso) y el agente de RE.
- El contenido de `brain/` y `conversations/` (volcado pendiente).
- Por qué "se petó" el flujo: pendiente de cruzar `daemon/daemon.log` y
  `crashes/`.
- Si `data-agent-kit-plugin_dak-viz` tiene tools (el listado salió vacío).
