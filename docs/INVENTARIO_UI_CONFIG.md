# Inventario exhaustivo: qué es configurable y qué es observable en MegaDaemon

> **Fecha de la auditoría:** 2026-11-23 · **Commit base:** `326896a`
> **Método:** extracción automática del código (no lectura a ojo). Cada cifra de
> este documento sale de un `grep`/`python` sobre el repo en ese commit y es
> reproducible.

---

## 1. Resumen ejecutivo

| Magnitud | Cifra real | Fuente |
|---|---|---|
| Módulos Rust | **194** ficheros en `rust-core/src/` | `ls rust-core/src/*.rs` |
| Tools MCP expuestas | **62** (+10 en `mcp_server`) | `mcp_ext.rs` |
| Endpoints HTTP v1 | **91** (`/api/*`) | `cockpit_server.rs` |
| Endpoints HTTP v2 | **54** (`/api/v2/*`) | `webui.rs` (`api_v2_routes()`) |
| Variables de entorno | **91** distintas | `env::var(...)` en `rust-core/src` |
| Ficheros de estado/nombreados en el código | **315** nombres distintos | barrido de literales |
| Endpoints v1 **sin ninguna UI** | **85 de 91 (93 %)** | cruce endpoint ↔ `fetch()` en la UI |
| Endpoints v2 con UI | **54 de 54** (verificado por `api_v2_routes()` y por `/api/v2/inventario` en vivo) | ídem |
| Vistas de la UI que se servía (V1) | **7** | `web/cockpit_template.html` |
| Vistas de la UI V2 | **19** | `web/cockpit_v2.html` |

**La frase corta:** el daemon tiene **muchísimo** más de lo que la Web UI
muestra. La UI no es "pobre": es **desconectada**. Hay 85 endpoints
funcionando que ninguna pantalla llama, 62 tools que nadie puede explorar y
91 parámetros que nadie puede ver ni cambiar.

---

## 2. Diagnóstico: por qué la UI "es lo peor del proyecto"

Tres causas medidas, no opiniones:

### 2.1 Se servía la UI equivocada
El daemon servía `web/cockpit_template.html` (**V1**, 122.774 B, 7 vistas:
antigravity, chat, inspect, npu, swarm, system, vram). La **V2**
(`web/cockpit_v2.html`, 177.808 B, 12 vistas) **nunca se servía**: no había
ni una ruta que la entregara.

*Corregido:* `/` y `/index.html` sirven la V2; la V1 queda en `/legacy` y
`/v1`. El supervisor materializa `cockpit_v2.html` y exporta
`MEGADAEMON_WEB_TEMPLATE_V2`.

### 2.2 La V2 habla con la API v2, y la API v2 es el 30 % del daemon
La V2 usa **54** endpoints `/api/v2/*` (medido el 2026-11-23 sobre
`api_v2_routes()`; antes eran 39 y quedaban 15 sin UI).  El daemon tiene
además **91 endpoints
`/api/*` v1** donde vive el grueso de las capacidades (motor RE, NPU,
telemetría de hardware, conversaciones, brain, devlog, métricas, backup,
voz, VLA, visión, firma, sincronización…). **Ninguno** está en la UI.

La causa es estructural: la API se dividió en dos generaciones (v1 = todo lo
viejo; v2 = lo nuevo) y **sólo se cableó la segunda con la UI**.

### 2.3 No hay ni configuración ni logs observables
- **Configuración:** 91 variables de entorno leídas desde 40 módulos
  distintos. Ninguna es editable, ninguna se muestra, y algunas se leen en
  el arranque ⇒ cambiarlas exige reiniciar.
- **Logs:** el daemon escribe en 14 sitios distintos (`/sdcard/megadaemon.log`,
  `state/auto_update.log`, `prefix/tunnel.log`, `mcp_tunnel.log`,
  `bridge.log`, `mitm_sqlite.log`, y 7 journals NDJSON). **Ninguno** era
  legible desde la UI: para saber por qué fallaba el móvil había que hacer
  `root_exec` a mano.

---

## 3. Inventario por subsistema

La tabla es el mapa completo. Cada fila dice: qué módulos lo implementan, qué
tools expone, qué endpoints tiene, qué ficheros de estado escribe, qué se
puede configurar y **qué muestra hoy la UI**.

| # | Subsistema | Módulos (clave) | Tools | Endpoints | Estado en disco | Configurable | **UI hoy** |
|---|---|---|---|---|---|---|---|
| 1 | **Núcleo / cockpit** | `cockpit_server`, `cockpit_doctor`, `state_dir`, `util_fs` | — | `/health`, `/metrics`, `/api/status`, `/api/live*`, `/api/capabilities`, `/api/ping` | `megadaemon_state.json`, `minidaemon_state.json`, `cockpit_token` | instancia, puertos, rate limit, orígenes | parcial (`resumen`, `sistema`) |
| 2 | **API key / auth** | `api_key`, `service_identity` | `api_keys` | `/api/remote_auth` | `megadaemon_api.key` (0600) | exigir key, rate máx, loopback, orígenes | **nada** |
| 3 | **Instrumentación AGY** | `antigravity_endpoint`, `antigravity_fake_so`, `antigravity_model`, `antigravity_bridge`, `antigravity_clone`, `antigravity_sessions`, `antigravity_env`, `antigravity_intel` | `antigravity`, `antigravity_sessions`, `antigravity_env` | `/api/v2/antigravity*` (7), `/api/antigravity/sessions` | `custom_models.json`, `mcp_config.json`, `estado.json`, `conversations/` | **nada** (modo .so, modelo, puente, MITM) | panel ANTIG (parcial) |
| 4 | **MITM SQLite** | `sqlite_mitm`, `sqlite_probe` | `sqlite_mitm` | — (vía tool) | `antigravity_servida.db`, `estado.json` | lista de ocultaciones, reescrituras | **nada** |
| 5 | **Shim de red AGY** | `agyshim`, `agyshim_rules` | `agyshim` | — | `agyshim.ndjson`, `agyshim_rules.json` | reglas de sustitución | **nada** |
| 6 | **Motor RE** | `re_engine`, `re_fusion`, `re_disasm`, `re_strings`, `re_ir`, `re_bdd`, `re_cff`, `re_mba`, `re_opaque`, `re_crypto`, `go_retool`, `ghidra`, `bindiff` | `re_analyze`, `re_fusion`, `re_disasm`, `go_retool`, `daad_harvester` | `/api/v2/re`, `/api/bindiff`, `/api/deobfuscate`, `/api/disassemble`, `/api/hex_inspect`, `/api/entropy_scan`, `/api/pattern_scan`, `/api/pointer_scan` (12 más) | `go.funcs.tsv`, `motor_analisis.json`, `ghidra_*.json` | timeout, workers, Java | vista RE (parcial) |
| 7 | **Decompiladores / APK** | `dex_decompile`, `apk_inventory`, `apk_deobfuscator`, `retro_apk_tools`, `axml_parser` | `dex_decompile`, `apk_inventory`, `retro_apk_tools` | `/api/carve_extract`, `/api/cirx_analyze`, `/api/dumper_extract`, `/api/kaitai_parse`, `/api/mba_simplify`, `/api/snapshot_diff`, `/api/cfg_lift` | — | — | **nada** |
| 8 | **NPU / VLA / visión** | `npu_bridge`, `npu_nnapi`, `npu_vs4l`, `npu_cv`, `shm`, `tile_vram` | `npu_vs4l_probe` | `/api/npu_*` (3), `/api/vla`, `/api/run_vla`, `/api/vision_*` (3), `/api/tile_render`, `/api/live_frame*` | `vla_telemetry.json`, `multiplatform_memory_telemetry.json` | — | vista NPU (en V1) |
| 9 | **Hardware / telemetría** | `env_introspect`, `hw_mapper`, `battery_advanced`, `power_policy`, `device_inventory` | `env_introspect`, `capabilities` | `/api/hardware_telemetry`, `/api/hw_mmio`, `/api/power`, `/api/profile` | `device.json`, `inventory.json` | umbrales térmicos, suelo RAM/disco, límite de carga | **nada** |
| 10 | **Energía / térmica** | `energy_automation`, `battery_saver`, `freezer` | `energy`, `battery_saver` | `/api/memory_freeze`, `/api/purge_cache` | `energy_profiles.json`, `energy_active.json` | perfiles, veto trabajo pesado | **nada** |
| 11 | **Cuotas / proveedores** | `quota_guard`, `providers`, `provider_catalog`, `price_table`, `accelerator_policy` | `quota_status`, `service_bus` | `/api/quota`, `/api/budget`, `/api/providers`, `/api/providers/test`, `/api/catalog`, `/api/llm_route` | `providers.json` | presupuesto USD, umbrales aviso/ahorro | **nada** |
| 12 | **Cola de tareas** | `task_scheduler`, `task_sync`, `job_queue`, `board`, `coordination` | `task_queue` | `/api/tasks`, `/api/v2/tasks*` (8), `/api/runners/control` | `tasks.json`, `tasks-1.json`, `board.sqlite`, `jobs.db` | lease, reintentos, sync Drive | vista reparto (parcial) |
| 13 | **Enjambre / nodos** | `mesh`, `peers`, `resonance`, `duckdns`, `channel`, `lan` | `resonance` | `/api/v2/nodes*` (4), `/api/v2/delegation*`, `/api/resonance`, `/api/peers`, `/api/minidaemon` | `mesh_board.json`, `mesh_journal.ndjson`, `peers.json`, `instance.json` | intervalo DuckDNS, publicación URL, peers | vista nodos (parcial) |
| 14 | **Tablón p2p** | `resonance` | `resonance` | `/api/resonance`, `/api/v2/market*` | `board.snapshot.sqlite`, journal | — | vista tablero (parcial) |
| 15 | **Drive / backups** | `drive_client`, `drive_native`, `drive_vfs`, `backup_*` (11), `swift_*` (5), `titanium_backup` | `drive_backup`, `drive_vfs`, `phone_backup`, `swift_container`, `titanium_backup` | `/api/signing_vault`, `/api/stores` | `container.json`, `manifest.json`, `gdrive_credentials.json` | raíz de Drive, reglas de compartición | **nada** |
| 16 | **Sincronización de chats** | `sync`, `sync_chats`, `sync_web`, `dict_sync`, `profile_analysis` | `sync_*` (8) | `/api/sync/*` (6), `/api/conversations/*` (2), `/api/brain_*` (2) | `conversations.json`, `sessions.json`, `export.json` | — | **nada** |
| 17 | **Agente / LLM** | `agent_core`, `agent_loop`, `agent_coding`, `agent_voice`, `meta_agent`, `llm_gateway`, `seere_v11` | `agent`, `coding_agent`, `agent_voice` | `/api/chat`, `/api/llm_agent`, `/api/v2/agentic`, `/api/v2/llm`, `/api/v2/prompt_external` | `failures.ndjson`, `megadaemon_audit.db` | modelo, voz, intervalos | **nada** |
| 18 | **Automatizaciones** | `automation`, `automation_runtime`, `automation_bridge`, `cron`, `comms_automation` | `automation`, `cron`, `comms`, `bridge` | `/api/trigger_boost`, `/api/input` | `automation_rules.json`, `automation_journal.ndjson`, `cron_state.json`, `comms_*.json` | reglas, cooldowns, consentimientos | **nada** |
| 19 | **Messaging / llamadas** | `messaging`, `telecom`, `call_recorder`, `recordings`, `recording_vault` | `messaging` | `/api/clipboard`, `/api/live_procs` | `clipboard.ndjson`, `journal.ndjson` | consentimiento, privacy | **nada** |
| 20 | **Root / sigilo** | `root_hide`, `root_stealth`, `root_intel`, `root_occlusion`, `inject` | `root_hide`, `root_detectors`, `root_consumers`, `root_stealth` | — | `inventory.json` | perfil de ocultación | **nada** |
| 21 | **Tiendas / paquetes** | `store_manager`, `pkgmgr`, `repo_updates` | `store_manager`, `repo_updates` | `/api/stores`, `/api/v2/admin/install_apk` | `stores.json`, `repos.json`, `index-v1.json` | canal, repo, tag | **nada** |
| 22 | **Auto-actualización** | `self_update`, `channel`, `supervisor` | `supervisor` | `/api/v2/admin/restart` | `auto_update.log`, `channel.json` | intervalo, activar, reiniciar | **nada** |
| 23 | **Autorreparación** | `reparar_entorno` | `reparar_entorno` | `/api/reparar` | `watchdog.sh` | — | **nada** |
| 24 | **SSH / red** | `sshd`, `http_transfer`, `identity_and_web` | `ssh_remote`, `http_fetch` | `/api/lan`, `/api/remote_auth` | `ssh_audit.ndjson` | — | **nada** |
| 25 | **Seguridad / atajos** | `signing`, `signing_vault`, `cert_vault`, `selfsigned_cert`, `argon2`, `backup_crypto` | — | `/api/signing_vault` | `priv.key`, `audit.log` | — | **nada** |
| 26 | **Frida / hooks** | `frida_hooks`, `frida_dbi`, `hooks_payload`, `hooks_bridge`, `inject` | `frida_hooks`, `hooks_payload` | `/api/frida_generate`, `/api/retro_hook` | `hooks.ndjson`, `retro_frida_symbolic_telemetry.json` | — | **nada** |
| 27 | **Alertas / tablón** | `alerts`, `antispam` | `alerts` | `/api/alerts`, `/api/v2/alerts` | — | webhook, reglas | vista alertas (parcial) |
| 28 | **MCPP / DAAD / retro** | `daad*`, `retro_gfx`, `rom_locator`, `rom_migration` | `daad_harvester`, `retro_apk_tools` | `/api/locate_roms`, `/api/die_binwalk` | `daad/` | — | **nada** |

---

## 4. La superficie configurable: de 91 variables dispersas a 12 secciones

Las 91 variables de entorno, agrupadas por la sección canónica que las
absorbe (`config_efectiva.rs`):

| Sección | Variables que absorbe | Claves canónicas |
|---|---|---|
| `general` | `MEGADAEMON_INSTANCE_NAME`, `_ROLE`, `_DEVICE`, `_STATE_DIR`, `_PREFIX` | `nombre_instancia`, `rol`, `puerto_cockpit`, `puerto_gateway`, `puerto_mcp` |
| `seguridad` | `MEGADAEMON_REQUIRE_API_KEY`, `MEGADAEMON_RATE_MAX`, `_RATE_LOOPBACK`, `MEGADAEMON_ALLOWED_ORIGINS`, `SEERE_AUTH_TOKEN`, `MEGADAEMON_API_KEY` | `exigir_api_key`, `rate_max_peticiones`, `rate_loopback`, `origenes_permitidos` |
| `instrumentacion_agy` | `MEGADAEMON_AGY_DIR`, `_ANTIGRAVITY_DIR`, `_AGY_TABLE`, `MEGADAEMON_MITM_HELPER`, `AGYSHIM_*` | `modo_so`, `modelo`, `puente_mcp`, `mitm_db`, `ocultar`, `reescribir` |
| `auto_update` | `MD_AUTO_UPDATE_INTERVAL_MIN`, `MD_NO_AUTO_UPDATE`, `MD_NO_AUTO_UPDATE_RESTART`, `MEGADAEMON_CANONICAL_URL` | `activo`, `intervalo_min`, `reiniciar_despues` |
| `energia` | `MEGADAEMON_CHARGE_LIMIT_PCT`, `MEGADAEMON_THERMAL_*_C`, `MEGADAEMON_RAM_FLOOR_KB`, `MEGADAEMON_DISK_FLOOR_KB` | `perfil`, `limite_carga_pct`, `termico_aviso_c/hard_c/critico_c`, `veto_trabajo_pesado`, `suelo_ram_kb`, `suelo_disco_kb` |
| `cuota` | `MEGADAEMON_DAILY_BUDGET_USD`, `SEERE_LLM`, `SEERE_SLOT` | `presupuesto_diario_usd`, `umbral_aviso_pct`, `umbral_ahorro_pct`, `pausar_en_ahorro` |
| `agente` | `MEGADAEMON_VOICE*`, `MEGADAEMON_AGYSHIM_NDJSON` | `modelo`, `voz`, `voz_intervalo_s`, `voz_cooldown_s`, `voz_max_acciones_hora`, `voz_lease_s` |
| `cola` | `MEGADAEMON_MESH_STALE_SECS`, `MEGADAEMON_NODE`, `MEGADAEMON_PEERS`, `HOSTNAME` | `lease_s`, `reintentos`, `sincronizar_drive` |
| `drive` | `MEGADAEMON_GDRIVE_CREDENTIALS`, `MEGADAEMON_GITHUB_TOKEN` | `raiz`, `sincronizar`, `tablón` |
| `re` | `MD_GHIDRA_DIR`, `MD_JAVA_HOME`, `JAVA_HOME`, `MD_ANTIGRAVITY_TIMEOUT` | `ghidra_java`, `timeout_ms`, `workers` |
| `tunel` | `MEGADAEMON_CHANNEL_INTERVAL_S`, `MEGADAEMON_ALLOW_CANONICAL_DNS`, `MEGADAEMON_LISTEN` | `activado`, `publicar_url`, `duckdns_intervalo_s` |
| `ui` | `MEGADAEMON_WEB_TEMPLATE`, `MEGADAEMON_WEB_TEMPLATE_V2`, `MEGADAEMON_KEEPER_INTERVAL_S` | `tema`, `auto_refresco_ms`, `mostrar_logs`, `log_lineas` |

**Variables que NO se tocan desde la config** (y por qué): las que son
*secretos* (`MEGADAEMON_API_KEY`, `SEERE_AUTH_TOKEN`, `GITHUB_TOKEN`,
`GDRIVE_CREDENTIALS`) — van en ficheros 0600, nunca en `config.json`; y las
que son *rutas del despliegue* (`MEGADAEMON_PREFIX`, `MEGADAEMON_STATE_DIR`),
que las fija el supervisor.

---

## 5. La superficie observable: qué debe verse y dónde

El principio: **todo lo que el daemon sabe de sí mismo debe ser visible desde
la UI**. Se agrupa en 14 familias:

| Familia | Qué muestra | Origen |
|---|---|---|
| **Identidad** | nombre, rol, build, SHA, versión, uptime, hostname | `self_update`, `mesh` |
| **Nodos del enjambre** | cada nodo: nombre, rol, URL, build, RAM, temperatura, tareas, último latido, si es canónico | `mesh`, `peers`, `resonance` |
| **Sistema** | RAM (usada/total/suelo), CPU/carga, disco libre, procesos, hilos | `/proc`, `env_introspect` |
| **Energía** | batería (%, salud, ciclos), temperatura, perfil activo, vetoes | `battery_advanced`, `energy_automation` |
| **Cuotas** | por modelo/proveedor: % restante, cuándo resetea, presupuesto USD | `quota_guard` |
| **Tareas** | cola completa: estado, lease, reintentos, nodo que la tiene, progreso | `task_scheduler` |
| **Tablón p2p** | cartas firmadas, anuncios, convergencia, instantáneas | `resonance` |
| **Instrumentación AGY** | cada pieza: aplicada/no, desde cuándo, por qué falló | `instrumentacion_agy` |
| **RE** | pipeline, motores, progreso de Ghidra, artefactos | `re_fusion`, `ghidra` |
| **Backups / Drive** | inventario, último backup, tamaño, destino, jobs | `backup_*`, `drive_native` |
| **Logs** | las 14 fuentes, tail en vivo, filtros por texto y nivel | `logs.rs` (nuevo) |
| **Configuración** | los 12 apartados, editable, con validación | `config_efectiva` (nuevo) |
| **Alertas** | activas,enbargadas, reglas evaluadas | `alerts` |
| **Tools** | las 62 tools con su descripción y su esquema | `mcp_ext` |

---

## 6. Los 85 endpoints huérfanos: cómo se exponen

No se duplica lógica: la API v2 **delega** en los mismos módulos que la v1.
Se agrupan por familia y se exponen bajo `/api/v2/<familia>/…`, de modo que la
UI tiene un contrato único:

| Familia v2 | Endpoints v1 que absorbe |
|---|---|
| `/api/v2/re/…` | `bindiff`, `deobfuscate`, `disassemble`, `hex_inspect`, `entropy_scan`, `pattern_scan`, `pointer_scan`, `carve_extract`, `cirx_analyze`, `dumper_extract`, `kaitai_parse`, `mba_simplify`, `cfg_lift`, `snapshot_diff`, `locate_roms`, `die_binwalk` (16) |
| `/api/v2/npu/…` | `npu_status`, `npu_cv`, `npu_bench`, `vla`, `run_vla`, `vision_start/stop/status`, `tile_render`, `live_frame`, `live_frame_jpg` (11) |
| `/api/v2/system/…` | `hardware_telemetry`, `hw_mmio`, `power`, `profile`, `memory_freeze`, `purge_cache`, `live_procs`, `minidaemon`, `peers`, `capabilities`, `metrics`, `health` (12) |
| `/api/v2/quota` | `quota`, `budget`, `providers`, `providers/test`, `catalog`, `llm_route` (6) |
| `/api/v2/sync/…` | `sync/run`, `sync/status`, `sync/harvest`, `sync/ingest`, `sync/web`, `conversations/list`, `conversations/search`, `brain_query`, `brain_status` (9) |
| `/api/v2/agent/…` | `chat`, `llm_agent`, `coding/run`, `coding/sessions`, `voice`, `minidaemon` (6) |
| `/api/v2/backup/…` | `signing_vault`, `stores`, `uploads` (3) |
| `/api/v2/automation/…` | `trigger_boost`, `input`, `runners/control` (3) |
| `/api/v2/privacy/…` | `clipboard`, `remote_auth` (2) |
| `/api/v2/antigravity/sessions` | `antigravity/sessions` (1) |
| `/api/v2/alerts` (existe) + `/api/v2/devlog` | `alerts`, `devlog` (2) |
| `/api/v2/tools` | **nuevo**: catálogo de las 62 tools con esquema |
| `/api/v2/logs` | **nuevo**: las 14 fuentes de log |
| `/api/v2/config` | **nuevo**: las 12 secciones de configuración |

Total v1 absorbido: **71**. Los 20 restantes (`/api/live*`,
`/api/ping`, `/api/status`, `/api/health`, `/api/session/exchange`,
`/api/tool/*`, `/api/devlog`, `/api/reparar`, `/api/minidaemon`,
`/api/providers/test`, `/api/sync/*`, `/api/v2/*`) son **de servicio**
(sonda, auth, proxy de tools): se quedan en v1 y la UI los sigue usando tal
cual.

---

## 7. Lo que se implementa (y en este orden)

| # | Pieza | Estado | Commit |
|---|---|---|---|
| 1 | **Motor de configuración** (`config_efectiva.rs`): 12 secciones, valores de fábrica, fusión con el entorno, validación y recarga en caliente | **hecho** | `326896a` |
| 2 | **Lector de logs** (`logs.rs`): 14 fuentes, tail por bloques, filtro por texto y nivel, NDJSON estructurado | **hecho** | `326896a` |
| 3 | **Orquestador de instrumentación AGY** (`instrumentacion_agy.rs`): las 4 piezas, informe por pieza, estado real | **hecho** | `326896a` |
| 4 | **Servir la V2** + `/legacy` para la V1 | **hecho** | `326896a`, `f85d269` |
| 5 | **API v2 ampliada** (10 rutas nuevas: logs, config, tools, nodes/detalle, system, quota, instrumentacion, backups) | **hecho** | `dae933d` |
| 6 | **UI V2**: vistas `logs`, `config`, `tools` + detalle de nodos, telemetría, cuotas, instrumentación y backups en las vistas existentes | **hecho** | `dae933d` |
| 7 | **Instrumentación automática**: ciclo de 60 s que re-instrumenta lo que falte; estado barato (metadatos, no hashes de 181 MB) | **hecho** | `64bd6ac` |
| 8 | **Portar de la V1** `chat`+`swarm`, `inspect`, `npu`, `vram` a la V2 (de forma nativa; la V1 sigue en `/legacy`) | **hecho** | `64bd6ac` |
| 9 | **Despliegue**: la V2 y su CSS viajan al móvil (instalador + auto-actualizador); una plantilla obsoleta se retira para que mande la embebida | **hecho** | `f85d269` |

### Cableado verificado

El test `webui::tests::cada_ruta_de_la_api_v2_se_pide_desde_la_ui` exige que
**toda** ruta de la API v2 sea llamada desde la UI. Comprobado sobre el
código tras los cambios:

```
rutas v2: 52 | sin llamada: 0
```

## 9. Bugs reales encontrados al conectar la UI con el daemon (2026-11-23)

Todos estos se **midieron en el móvil** (no son hipótesis):

| # | Síntoma en la UI | Causa real | Arreglo |
|---|---|---|---|
| 1 | «Bus de coordinación no desplegado» en todas las instancias | `Mesh::attach` **no lo llamaba nadie**: el fichero `mesh_board.json` no se creaba nunca | `mesh::desplegar_bus()` al arrancar + latido cada 45 s (`30d5cf6`) |
| 2 | «Sin instancias conectadas» | el tablero del bus es un fichero LOCAL (sólo se comparte si comparten state dir); la vía real es el tablón de Drive | la vista de nodos lista también los **anunciados en Drive** |
| 3 | «Sin cuentas / No hay cuentas de proveedor cargadas» | la vista leía `delegate_state.json`, un fichero que nadie crea; las cuentas reales están en el **vault de proveedores** | la vista muestra el vault (14 cuentas reales) |
| 4 | El gasto no se analizaba por proveedor (MiniMax, Google…) | el gasto vivía **sólo en memoria** (`DailyBudgetState`): se perdía al reiniciar y no había desglose | `spend_ledger.json` persistente + informe por proveedor (`33c2fae`) |
| 5 | La instrumentación decía «fallo» cada minuto | se leía la clave equivocada del estado (`registrado` en vez de `modelo_registrado.ok`) y la app cerrada se contaba como fallo | `estado_modelo()` ⇒ (registrado, puente_caido): con la app cerrada es **aplazado**, no fallo (`1dadeb8`) |
| 6 | `activar` fallaba con «sin_cambios» | el `.so` **ya** llevaba la URL local y se re-hacía el trabajo | `ya_tiene_url()` + `activar` idempotente (`79a9ac9`) |
| 7 | «leer .../runtime/bin/liblanguage_server.so: No such file or directory» | **Antigravity se actualizó y movió su runtime** (`standalone_runtime/bin/language_server`; el symlink de `agy_standalone` quedó roto) | `ruta_ls()` descubre el binario en vez de hardcodearlo (`8597320`, `30d5cf6`) |
| 8 | La app «restauraba» su runtime y se perdía la instrumentación | el manifiesto declaraba el sha del `.so` original ⇒ la app lo reponía | el manifiesto se actualiza con el sha del falso, en **todas** las claves que hablen de «language_server» |
| 9 | El móvil no se anunciaba en el tablón (nadie lo encontraba) | el supervisor escribe la URL en `$PREFIX/tunnel.url` y el daemon la buscaba en `$STATE_DIR/tunnel.url` | se miran los dos sitios (`36301fd`) |
| 10 | `/health` decía `file:v1` mientras servía la V2 embebida | el diagnóstico miraba «qué fichero existe», no «qué se sirve» | el diagnóstico reproduce el orden real de resolución (`b6972eb`) |

**Estado medido en el móvil tras los arreglos** (build `30d5cf6`):

```
/health              → 200, ui_servida=v2, template_source=embedded:v2
nodos/detalle        → 1 nodo (el local, master, vivo)
instrumentacion      → agy_vivo=true, so aplicado, puente_mcp al día
language_server      → contiene 127.0.0.1:9090 (el tráfico va a MegaDaemon)
original + falso     → en el state dir del daemon (181.358.800 B cada uno)
ciclo automático     → 315 B de log en 90 s (NO hay bucle)
```

### Lo que queda (aún NO hecho, con honestidad)

| Hueco | Por qué sigue abierto |
|---|---|
| Los **71 endpoints v1 agrupados** en familias v2 (`/api/v2/re/…`, `/api/v2/npu/…`, `/api/v2/system/…`, `/api/v2/sync/…`, …) | La V2 llama directamente a los endpoints v1 que necesita (inspect, npu, chat, vision) y a los nuevos de la v2. Agrupar los 71 es una reorganización grande de URL; el **contenido** ya es visible en la UI, lo que falta es la URL canónica. **Ahora se puede MEDIR**: `/api/v2/inventario` lista las rutas v1 y v2 y marca cuáles las consume la UI (`usada_por_ui`), contra el HTML que el daemon está sirviendo de verdad. La UI lo pinta en *Sistema*. |
| ~~`/api/v2/devlog`~~ **HECHO (2026-11-23)** | Existe `/api/v2/devlog` y la UI lo pinta en la vista *Sistema*.  De paso se arregló que `/api/devlog` devolvía **siempre** `[]` (el lector sólo aceptaba objetos y el fichero es un array; además nada lo escribía): ahora hay un módulo propio (`rust-core/src/devlog.rs`) con escritor y lector, y el daemon escribe eventos reales (auto-actualización e instrumentación). |
| Editar la configuración **del supervisor** desde la UI | Deliberado: si el supervisor se rompe, el móvil no vuelve. Se edita a mano y se verifica. |
| Ghidra (el motor RE) en el móvil | Necesita JDK 21 y GB de RAM: el móvil tiene ~4 GB. La UI muestra su progreso; la ejecución es del PC. |
| Las **230 funciones** que Ghidra no convierte (medido 2026-11-23) | Son las de ensamblador de Go (`.s`: `cmpbody`, `crosscall2_trampoline`…) y unas pocas de la stdlib. Ghidra las procesa pero no produce pseudocódigo: se registran como «sin pseudocódigo» en `NO-DECOMPIADAS.tsv`. |
| El **MITM de la DB** de Antigravity | Desactivado por defecto en la configuración (`mitm_db: false`): toca el fichero de la app. Se activa desde la UI o desde `config.json` cuando se quiere. |

---

## 8. Lo que NO se hace (y por qué)

| Descartado | Motivo |
|---|---|
| Mover el daemon a un framework web nuevo | 194 módulos funcionan; reescribir es riesgo puro. La v2 es el marco nuevo, no otro marco. |
| Exponer los 91 endpoints v1 uno a uno sin agrupar | 91 rutas más que mantener y una UI imposible de navegar. Se agrupan por familia. |
| Meter los secretos en `config.json` | Principio de la casa: secretos en ficheros 0600, nunca en un JSON de configuración legible. |
| Editar la config del supervisor desde la UI | El supervisor es de arranque: si se rompe, el móvil no vuelve. Se edita, se reinicia y se verifica. |
| Ghidra en el móvil | 11.2+ necesita JDK 21 y GB de RAM: el móvil tiene ~4 GB. Ghidra es del PC; la UI sólo muestra su progreso. |
| Pintar los ficheros binarios en la UI | La UI opera con los datos; los binarios se descargan desde Drive. |
