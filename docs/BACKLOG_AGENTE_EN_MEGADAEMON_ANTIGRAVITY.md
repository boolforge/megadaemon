# ADENDUM al backlog del agente DENTRO de MegaDaemon — Antigravity (motor del móvil)

> Addendum de `docs/BACKLOG_AGENTE_EN_MEGADAEMON.md` (commit `6d81467`, que
> **no** está en la rama de este workstream ⇒ va aparte para no chocar con
> quien loFusione). Todo lo que hay aquí está **medido** en el S21 Ultra
> (2026-11-22 13:5x–14:2x) o **marcado NO VERIFICADO**.
>
> Referencia completa de lo ya medido: `docs/ANTIGRAVITY_APROVECHABLE.md`.

---

## 0. Reglas de operación (las pidió el jefe; de aquí en adelante, obligatorias)

1. **Un solo mergers**: los sub-agentes **no** hacen `push` a `main`; publican
   su SHA en `state/mesh_journal.ndjson` y avisan al jefe, que compila,
   mergea y testea (su CI tiene RAM; el sandbox no).
2. **NFS**: `git status`/`git log` tardan *minutos*; un `grep -r` del repo
   entero pasa de 180 s ⇒ **`rg` o rutas exactas, siempre**. Nunca `find .`
   sobre el repo.
3. **RAM**: **nunca dos builds `cargo` a la vez**. Un `kill` a mitad de build
   deja artefactos corruptos (pasó con `zstd-sys`: el enlazado falla con
   `undefined symbol: ZSTD_buildFSETable` ⇒ `cargo clean -p zstd-sys` y
   repetir).
4. **Worktrees en `/workspace`**, nunca en `/root` (es efímero: un worktree
   ahí se pierde al reciclar el sandbox).
5. **Commits por bloque** con `git add <ruta exacta>` (nunca `-A`) y
   `git commit` **temprano**: en un árbol compartido lo *untracked* es efímero
   (otro agente hizo `reset --hard` y borró ficheros nuevos de otro).
6. **WIP ajeno = no se toca**: si el rojo local viene de un fichero **no
   versionado** de otro workstream, se reporta al jefe y se arregla **en el
   árbol de trabajo**, no en su commit.

---

## 1. Hecho (3 commits en `agent/antigravity-instrumentado`)

| SHA | Entrega |
|---|---|
| `b6e3895` | `antigravity_bridge.rs` contra la **API real** + rescate de `ghidra.rs` (mergeado por el jefe como `041b9c3`) |
| `a997273` | paridad **CLI + MCP + agente** + API `/api/v2/antigravity*` + panel GUI enchufable + informe |
| `3fccbc7` | fix real del parser de conversaciones (cazado por test propio) |

**Hallazgo que desbloquea el motor** (medido): la API local de Antigravity
(`127.0.0.1:45157`) **no** pide OAuth de Google: pide el **CSRF del SPA**
(`window.__APP_CONFIG__.csrfToken`) en la cabecera exacta
`x-codeium-csrf-token`; sin ella ⇒ `401 unauthenticated: missing CSRF token`.
Catálogo real: **36 modelos**; `MODEL_PLACEHOLDER_M526` ⇄
`gemini-4-hydrogen-medium` (el estado guarda el ofuscado, la UI el real).

---

## 2. Pendientes con puerta de aceptación

### P0-3 · Aplicar el panel ANTIG al cockpit (pieza ya escrita)

* **Qué**: `web/panels/antigravity_panel.html` + `.js` están **listos** (vista
  ANTIG: estado, catálogo real con cuota, conversaciones, búsqueda, encolado,
  cola de tareas). **No** se han metido en `cockpit_template.html` porque la
  plantilla se está rediseñando (`e81fade1`).
* **Puerta**: en el cockpit nuevo, la vista ANTIG carga con **1 fetch**
  (`GET /api/v2/antigravity`), muestra los **36** modelos con su id real y su
  cuota, y `ENCOLAR` devuelve un `cascade_id` real.
* **Quién**: el sub-agente de Antigravity, sobre el SHA que dé el jefe.

### P0-4 · Página de chat del agente contra `/v1/chat/completions`

* **Qué**: la página de chat que el jefe pidió (P0-1 de su backlog): modelo
  del vault, prompt, respuesta y botón de enviar, **sin dependencias
  externas** (el móvil no tiene node).
* **Puerta**: (a) el cuerpo de la petición sale por `POST /v1/chat/completions`
  —verificable en el log del daemon, no en el navegador—; (b) cuando el
  proveedor rechaza, la UI **explica el motivo** con el `reason` del
  `provider_trace` (P2-2 del backlog del jefe).
* **Depende de**: P0-3 (misma plantilla) ⇒ **un solo commit** para los dos.

### P1-3 · MiniMax en el selector de Antigravity (vía gateway)

* **Qué**: el catálogo de Antigravity es de Google/Anthropic/OpenAI y el
  binario **no contiene** la cadena `minimax` ⇒ sólo entra por BYOK. Las 4
  rutas medidas y por qué 3 están cerradas:
  * `admin_settings.json` con `models.list` → el binario busca
    `/etc/antigravity/admin_settings.json` ⇒ en Android es `/system/etc`
    (**no escribible** sin root en la app);
  * `SetUserSettings`/`GetUserSettings` → **501 unimplemented** en este build;
  * el Tier 1 (admin file) **no mueve** el catálogo (medido: respuesta
    idéntica byte a byte);
  * **queda viva**: el **Tier 3 = gateway discovery**, con las variables
    `AGY_LLM_GATEWAY_{URL,API_KEY,HEADERS,MODELS,PROXY_URL,CA_CERT,WIRE_PROTOCOL}`.
* **Qué hay que hacer**: (1) un endpoint **compatible con Gemini** para
  MiniMax en el móvil (el `modelName` del BYOK del IDE es
  `gemini-api:/models/<model>`); (2) arrancar el language server **con**
  `AGY_LLM_GATEWAY_*` (lo lanza la app ⇒ hoy hay que hacerlo desde el PC con
  ADB o desde un `service.d` propio).
* **Puerta**: `GetAvailableModels` devuelve un modelo cuyo proveedor no es
  Google/Anthropic/OpenAI **y** el selector de la UI lo muestra.

### P1-4 · BYOK nativo del IDE (alternativa al gateway)

* **Qué**: el IDE sabe registrar modelos propios: `UserSettings.custom_models`
  (mapa `string → ModelInfo`, con `modelId = MODEL_GOOGLE_GEMINI_INTERNAL_BYOM`,
  `apiProvider`, `chatModelName`, `modelName`, `maxTokens`, `modelFeatures`) y
  `jetbox_state_pb.CustomModelsConfig.custom_models` (estado proto de la app).
  El móvil **no** tiene `custom_models` en su estado ⇒ hoy no hay ningún
  modelo propio registrado. El fichero que sí existe
  (`.gemini/antigravity/custom_models.json`) tiene **otra forma** y es
  **inerte** para el selector.
* **Puerta**: el modelo propio aparece en el selector **sin** tocar
  `/system/etc`.

### P2-3 · Probar `enqueue` de verdad y medir su coste

* **Qué**: `StartCascade` + `SendUserCascadeMessage` están implementados
  (con el modelo **ofuscado** del estado) pero **no se han ejecutado contra
  el móvil** (el móvil cayó antes).
* **Puerta**: un `enqueue` real ⇒ `cascade_id` + la cascada aparece en
  `GetAllCascadeTrajectories` + nº de pasos y cuota consumida anotados.

### P2-4 · Vigilancia del catálogo cuando el móvil vuelva

* **Qué**: el catálogo puede cambiar (Google lo empuja desde el servidor).
  Un health-check que compare `nº de modelos` y `sha16` con lo medido
  (**36** / `7179fda6f56a878a`) avisa cuando el motor cambia de catálogo.
* **Puerta**: una comprobación automática + aviso al bus cuando el sha del
  catálogo **cambie** (o el móvil vuelva con otro número de modelos).

---

## 3. Restricciones medidas que condicionan estos P

1. **El móvil ya estaba en load 19,85** con el IDE de Google vivo
   (`libloader.so` 251 MB + `com.agy` 156 MB) ⇒ **no** añadir procesos
   pesados mientras corre el superloop (`ANTIGRAVITY_INTERVAL` ≥ 2× el ciclo:
   1200 s, no 300 s).
2. **El disco del móvil está al 98 %** ⇒ logs podados (`ANTIGRAVITY_KEEP_LOGS=50`).
3. **El tope de RSS del supervisor mataba al propio daemon** ⇒ túnel 502 con
   el móvil encendido (ya corregido y mergeado por el jefe: `2b67c09`).
4. **El móvil no tiene python3/node/sqlite3/jq/curl de fábrica** ⇒ todo el
   cliente nuevo es **Rust** (éste es el motivo de que exista
   `antigravity_bridge.rs`).
5. **El binario de Antigravity sólo se deja arrancar desde la app o desde un
   `service.d` propio** ⇒ cualquier palanca de entorno (`AGY_LLM_GATEWAY_*`)
   exige arrancar el LS a mano ⇒ **de ahí el coste alto de P1-3**.

---

## 4. Trazabilidad

| afirmación | de dónde sale |
|---|---|
| cabecera `x-codeium-csrf-token`, 401 literal | medición en el móvil (curl a `127.0.0.1:45157`) |
| 36 modelos, `defaultAgentModelId=gemini-4-neon-high` | `GetAvailableModels` (sha16 `7179fda6f56a878a`) |
| `MODEL_PLACEHOLDER_M526` ⇄ `gemini-4-hydrogen-medium` | catálogo + `antigravity_state.pbtxt` |
| `GetUserSettings`/`SetUserSettings` = 501 | medición directa |
| `gateway.ModelEntry` + ruta `models.list[i].modelId` | strings de `liblanguage_server.so` |
| los 4 tiers (`[ModelConfig] Resolving models from Tier N …`) | strings de `liblanguage_server.so` |
| `AGY_LLM_GATEWAY_*` | strings de `liblanguage_server.so` |
| BYOK: `gemini-api:/models/…`, `MODEL_GOOGLE_GEMINI_INTERNAL_BYOM` | `web/generated/main.js` (bundle de la app) |
| tasks `task_442838` / `task_553653`, ciclo de 211 s | `daemon/tasks.json` del móvil |
| load 19,85, RSS de LS y app | `/proc/loadavg`, `/proc/<pid>/status` |
| reglas de NFS/RAM/worktrees | historial de esta sesión (git lento, artefactos `zstd-sys` corruptos, worktree perdido en `/root`) |
