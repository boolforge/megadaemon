# ANTIGRAVITY APROVECHABLE — la IDE agéntica de Google como motor de la flota

> **Workstream W5 (continuación instrumentada)** · Samsung S21 Ultra del usuario
> (`com.agy`, "AGY v2" 2.11.0) · measurements del **2026-11-22 13:5x–14:2x**
> (el móvil quedó sin daemon a las ~14:30; lo que sigue está medido, no supuesto).
>
> Reglas del repo aplicadas: **ningún secreto en el repo, en el log ni en el
> chat** (del token CSRF y del OAuth sólo se publica su `sha16`); comentarios en
> español, identificadores en inglés; todo lo que no se ha podido verificar va
> marcado como **NO VERIFICADO**.

---

## 0. Resumen en una página

| Pregunta | Respuesta medida |
|---|---|
| ¿Qué es? | La IDE agéntica de Google (`com.agy`) instalada en el móvil. Levanta un *language server* propio en `127.0.0.1:45157` (HTTPS con certificado autofirmado) que habla **Connect RPC**. |
| ¿Cómo se abre su API? | Con el **token CSRF** que el SPA publica en `window.__APP_CONFIG__.csrfToken`, en la cabecera **exacta** `x-codeium-csrf-token`. Sin ella: `401 {"code":"unauthenticated","message":"unauthenticated: missing CSRF token"}`. **No** pide OAuth de Google para la API local. |
| ¿Qué tiene? | **36 modelos** reales (Claude Opus/Sonnet 5.5, Gemini 3.x/4.x, GPT-OSS 120B), un historial de conversaciones (cascadas) buscable, un planificador de tareas y una cola MCP. |
| ¿Sirve como motor? | **Sí, y ya está instrumentado**: cliente Rust (`antigravity_bridge`), CLI (`md-antigravity`), tool MCP, tool de agente, API `/api/v2/antigravity*` y panel de cockpit. |
| ¿MiniMax en el selector? | **NO.** El catálogo es de Google/Anthropic/OpenAI y el binario **no contiene** la cadena `minimax`. Sólo puede entrar por BYOK (ver §5, con las 4 rutas medidas). |
| ¿Y el superloop? | Es un ciclo de 211 s (hasta 570 s) cada 300 s: **intervalo solapado** y con el móvil ya en load 19,85. §4 da la receta para no dejarlo en load 20. |

---

## 1. Qué es Antigravity (medido)

### 1.1 La app y su language server

`com.agy` es una **shell WebView** (`classes.dex` = 212 KB): el producto real
está en `assets/runtime/bin/liblanguage_server.so` (181 MB, binario **Go** de
Google). Al arrancarla, el runtime deja un proceso `libloader.so` escuchando:

```
libloader.so --library-path /data/user/0/com.agy/files/runtime/lib \
  --preload .../runtime/lib/liblse_compat.so \
  .../runtime/bin/liblanguage_server.so --standalone \
  --override_ide_name antigravity --subclient_type hub \
  --override_ide_version 2.11.0 --https_server_port 45157 \
  --csrf_token <uuid> --app_data_dir antigravity-app \
  --web_bundle_path /data/user/0/com.agy/files/web --enable_sidecars
```

* **Medido**: `libloader.so` (pid 25963) con **251 MB RSS**; `com.agy` con
  **156 MB RSS**; `127.0.0.1:45157` en LISTEN.
* **Medido**: `/etc` del proceso es `/system/etc` (`/proc/<pid>/root/etc` →
  symlink), `HOME=/data/user/0/com.agy/files`,
  `XDG_CONFIG_HOME=/data/user/0/com.agy/files/.config`.

### 1.2 La API real (no la SPA)

El SPA (`GET /`) devuelve 3,4 KB de HTML con
`window.__APP_CONFIG__ = {productName, csrfToken, appVersion:"2.11.0",
releaseChannel:"stable", devMode:false}`.
**`/mcp` y `/tools` NO son un MCP**: son el *catch-all* de la SPA (200 con HTML).

La API de verdad es **Connect RPC sobre protobuf**, método a método:

```
POST https://127.0.0.1:45157/exa.language_server_pb.LanguageServerService/<Método>
Content-Type: application/json
x-codeium-csrf-token: <csrfToken del __APP_CONFIG__>
```

El binario expone **332 métodos** del servicio (extraídos de las cadenas del
`.so`). Los que usa la flota: `GetAvailableModels`, `GetCascadeModelConfigData`,
`GetAllCascadeTrajectories`, `GetCascadeTrajectory`, `GetCascadeTrajectorySteps`,
`SearchConversations`, `StartCascade`, `SendUserCascadeMessage`,
`GetMcpServerStates`, `GetStatus`, `GetCapabilities`, `GetAuthStatus`,
`GetUserStatus`, `JetboxReadAppState`, `GetUserSettings`, `SetUserSettings`.

### 1.3 Autenticación (el hallazgo que cambia el enfoque)

| Petición | Resultado **medido** |
|---|---|
| sin cabecera | **401** `{"code":"unauthenticated","message":"unauthenticated: missing CSRF token"}` |
| `x-codeium-csrf-token: <csrf del SPA>` | **200** + JSON del método |
| `X-CSRF-Token`, `Authorization: Bearer <oauth>`, `Cookie`, `X-Goog-Api-Key` | **no** habilitan la llamada |

⇒ La sesión de Google es **otra cosa** y está en otro sitio:
`/data/user/0/com.agy/files/.gemini/jetski-standalone-oauth-token`
(1.740 B, `sha16 = 1028cbc50fe6b8db`), con `access_token`, `refresh_token`,
`id_token` y `auth_method`. `GetAuthStatus` → **200**
`{"authResult":{"hasValidAuth":true,"grantedScopes":[…,"https://www.googleapis.com/auth/aicode","…/cloud-platform",…]}}`.
El token **nunca** se imprime: el puente expone su `sha16`
(`antigravity_bridge::AntigravityBridge::csrf_sha16`).

### 1.4 Los ids de modelo están ofuscados (y se traducen)

El estado del móvil (`antigravity_state.pbtxt`) guarda el id **ofuscado**:

```
last_selected_agent_model: MODEL_PLACEHOLDER_M526
```

El catálogo real (`GetAvailableModels`) trae el id **real** como clave del mapa
`models` y, dentro, el ofuscado:

```json
{"response":{
  "models":{
    "gemini-4-hydrogen-medium":{"displayName":"Gemini 4 Hydrogen (Medium)",
      "model":"MODEL_PLACEHOLDER_M526","modelProvider":"MODEL_PROVIDER_GOOGLE",
      "recommended":true,"maxTokens":1048576,"maxOutputTokens":65536,
      "quotaInfo":{"remainingFraction":0.6177836,"resetTime":"2026-11-22T15:37:22Z"}},
    …},
  "defaultAgentModelId":"gemini-4-neon-high",
  "tieredModelIds":{"flashLite":["gemini-3.5-flash-lite"],"flash":[…],"pro":[…]}}}
```

⇒ **`MODEL_PLACEHOLDER_M526` ≡ `gemini-4-hydrogen-medium`** (verificado en el
catálogo vivo). El puente traduce en los dos sentidos
(`real_id_for_placeholder` / `placeholder_for_real_id`) porque `StartCascade`
habla el idioma ofuscado y la UI el real.

**Catálogo completo medido (36 modelos)** — Google (29): `gemini-2.5-flash`,
`gemini-2.5-flash-lite`, `gemini-2.5-flash-thinking`, `gemini-2.5-pro`,
`gemini-3-flash`, `gemini-3.1-flash-image`, `gemini-3.1-flash-lite`,
`gemini-3.5-flash-lite`, `gemini-3.8-flash-{high,low,medium,tiered}`,
`gemini-4-argon-{high,low,medium}`, `gemini-4-carbon-disabled`,
`gemini-4-hydrogen-{high,low,medium,sonar,tiered-high,tiered-xhigh,xhigh}`,
`gemini-4-neon-{high,low,medium,xhigh}`,
`tab_flash_lite_preview`, `tab_jump_flash_lite_preview`;
Anthropic (6): `claude-opus-5-5-{high,low,medium}`,
`claude-sonnet-5-5-{high,low,medium}` (con `vertexModelId`
`claude-opus-5-5@default`); OpenAI (1): `gpt-oss-120b-medium`
(`openai/gpt-oss-120b-maas`). Por defecto: **`gemini-4-neon-high`**.
**MiniMax no aparece** y el binario **no contiene** la cadena `minimax`.

---

## 2. Qué puede hacer la flota con él (y ya lo hace)

Todo esto está implementado en `rust-core/` y es **paridad total** entre las
tres superficies (CLI, MCP y agente), con tests propios:

| Superficie | Dónde | Qué expone |
|---|---|---|
| **Biblioteca** | `rust-core/src/antigravity_bridge.rs` | `AntigravityBridge`: `model_catalog()`, `models()`, `default_model()`, `cascade_model_configs()`, `auth_status()`, `conversations()`, `search()`, `conversation()`, `enqueue()`, `tasks()`, `agent_status()`, `tool_catalog()`. Todo el parseo es **puro** (funciones `*_from_*` sin red) ⇒ testeable sin móvil. |
| **CLI** | `rust-core/src/bin/md-antigravity.rs` | `status \| catalog \| conversations \| search \| conversation \| tasks \| enqueue` (+ `--json`, `--model`, `--workspace`, `--wait`). |
| **MCP** | `rust-core/src/mcp_ext.rs` (tool `antigravity`) | `action = status\|catalog\|models\|conversations\|search\|conversation\|tasks\|enqueue`. `enqueue` **exige `confirm=true`** (crea una conversación y consume cuota). |
| **Agente** | `rust-core/src/agent_coding.rs` (tool `antigravity`) | las mismas acciones, para que un agente pueda pilotar Antigravity desde su bucle ReAct. |
| **API del daemon** | `rust-core/src/webui.rs` | `GET /api/v2/antigravity`, `/antigravity/models`, `/antigravity/conversations`, `/antigravity/search?q=`, `/antigravity/conversation?id=`, `/antigravity/tasks`, `POST /api/v2/antigravity/enqueue` (también `confirm=true`). Configurable con `MD_ANTIGRAVITY_URL` y `MD_ANTIGRAVITY_TIMEOUT`. |
| **GUI** | `web/panels/antigravity_panel.html` + `.js` | Panel "ANTIG" del cockpit: estado, catálogo real con cuota, conversaciones, búsqueda, encolado y cola de tareas. **Va aparte** a propósito: la plantilla del cockpit se está rediseñando (`e81fade1`), así que el panel se entrega como pieza enchufable y no pisa ese trabajo. |

### 2.1 El superloop, qué es y qué NO es

`daemon/tasks.json` (dentro del sandbox de Antigravity) declara **2 tareas**:

| id | nombre | trigger | estado medido |
|---|---|---|---|
| `task_442838` | `Snatcher-MegaDaemon-SuperLoop` | `interval` 300 s | `enabled: true`, `last_run_status: success`, `last_run_duration: 211 s` |
| `task_553653` | `minimax_bootstrap` | `delay` (run_at 2026-11-19 12:09:13) | **nunca ejecutada** |

El comando del superloop es
`python3 /data/user/0/com.agy/files/scummvm/tools/re_pipeline/snatcher_megadaemon.py --once`.
**Medido**: `python3` **no está en el PATH del sistema** (rc=127); sólo existe
como wrapper en `/data/user/0/com.agy/files/runtime/bin/python3` (3.14.10).

Lo que ese ciclo produce (estado del repo `re_pipeline/megadaemon_state.json`):
**25 ciclos**, **14.494 funciones recuperadas**, **915 descompiladas**, 25
trampas de hardware, 24 passes de build de ScummVM; en el móvil iba por **37
ciclos / 923 descompiladas** (medido en `last_output`).

### 2.2 Other piezas useful de Antigravity

* **Cascadas** (`GetAllCascadeTrajectories`): el historial del agente, con
  `cascadeId`, `trajectoryId`, estado, nº de pasos y workspaces ⇒ es la
  memoria conversacional del móvil.
* **Cola MCP** (`GetMcpServerStates`): el móvil ya tiene registrado el servidor
  `megadaemon-minimax` (52 tools) ⇒ el puente bidireccional **ya existe**.
* **Extensión** (`extension_server`): `CreateTerminal`, `ReadTerminal`,
  `ShowTerminal`, `OpenTerminal`, `EditNotebook`, `ReadNotebook`, `SaveDocument`.
* **BYOK**: el IDE sabe registrar modelos propios (ver §5).

---

## 3. Cómo se arranca el superloop **de forma controlada**

### 3.1 Por qué no basta con la tarea del IDE

**Medido**: el micro-daemon de Antigravity vive **dentro** del proceso
`com.agy`. Cuando la app muere, el planificador muere con ella y las 2 tareas
se quedan `enabled=true` **sin ejecutar** (medido: 3 días con el backlog
parado, `daemon.pid` apuntando a otro proceso, `io.github.mekhontsev.magicdesk`).
Los reinicios tampoco lo salvan: el móvil reinició 4 veces (20–21-Nov) y el
backlog siguió muerto.

Por eso existen `deploy/antigravity-task-supervisor.sh` (supervisor + *catch-up*)
y `deploy/antigravity-task-runner.sh` (un ciclo), instalables como
`/data/adb/service.d/97-antigravity-tasks.sh`.

### 3.2 Las 5 garantías del runner (medibles desde fuera)

1. **Nunca se solapan dos ciclos** — lock atómico con `mkdir`
   (`$STATE_DIR/runner.lock.d`): `mkdir` es atómico en POSIX y este móvil no
   tiene `flock` (`Unknown option 'c'`). Si hay ciclo en marcha ⇒ rc **75**.
2. **Nunca se queda colgado** — `timeout` con `ANTIGRAVITY_MAX_RUNTIME`
   (900 s por defecto); rc **124** si se excede.
3. **Cada ciclo deja log con fecha** — `logs/cycle-AAAAMMDD-HHMMSS.log`.
4. **Cada ciclo deja estado legible** — `last_status.json`.
5. **Si no puede arrancar, lo dice** — rc **78** (preflight) con el motivo.

Y el supervisor **no pisa al daemon vivo**: si `daemon.log` se ha tocado hace
menos de `ANTIGRAVITY_DAEMON_ALIVE_WINDOW` (600 s) ⇒ el daemon de la app es el
dueño del ciclo y el supervisor se aparta.

### 3.3 La receta para NO dejar el móvil en load 20

**El dato que obliga a cambiar los valores por defecto**: el ciclo dura
**211 s** (el último medido) y hasta **570 s** en el historial, pero el
`interval` declarado es **300 s** ⇒ **el planificador puede solapar dos
workers**. Y el móvil, con el superloop + el IDE de Google vivo, estaba ya en
**load average 19,85** con 3,9 GB libres (medido 14:2x).

Arranque controlado, en este orden:

```sh
# 1) budgets: intervalo >= 2x el peor ciclo medido (570 s) y runtime acotado
export ANTIGRAVITY_INTERVAL=1200          # s (por defecto 300 ⇒ solapa)
export ANTIGRAVITY_MAX_RUNTIME=900         # s, timeout duro del ciclo
export ANTIGRAVITY_CATCHUP_MAX_RUNTIME=300 # s, catch-up acotado
export ANTIGRAVITY_KEEP_LOGS=50            # no crecer el disco (98 % lleno)

# 2) arranque controlado (no en primer plano)
deploy/antigravity-task-supervisor.sh --install   # copia + service.d + arranca
deploy/antigravity-task-supervisor.sh --status     # estado y PID

# 3) parar el superloop sin tocar la app
deploy/antigravity-task-supervisor.sh --stop
```

**Reglas de carga** (medidas, no opiniones):

* **Un ciclo cada ≥ 2× su duración** ⇒ con 570 s de peor caso, 1200 s. Con
  300 s se solapan (es exactamente el bug que el lock viene a tapar).
* **El IDE de Google ya se come 400 MB** (`libloader.so` 251 + `com.agy` 156):
  no añadir procesos pesados mientras corre el superloop.
* **Catch-up sólo con el daemon muerto**: el supervisor lo hace solo si
  `daemon.log` lleva > 600 s sin tocarse.
* **Los logs se podan** (`KEEP_LOGS=50`): el disco del móvil está al 98 %.
* **Supervisar la carga, no el proceso**: `load average` > 15 ⇒ parar el
  superloop (`--stop`); > 20 ⇒ además bajar `ANTIGRAVITY_INTERVAL`.

**Aviso de memoria (medido, y el coste de no hacerlo)**: el tope de RSS del
supervisor de MegaDaemon (`ENERGY_DAEMON_RSS_MAX_KB`, 200 MB por defecto)
**mataba al propio daemon** del móvil ⇒ túnel 502 con el móvil encendido. Ya
lo ha corregido el agente MITM (tope adaptativo) y el jefe lo ha mergeado
(`2b67c09`).

---

## 4. Lo que **no** se ha conseguido (con la evidencia)

### 4.1 MiniMax en el selector de modelos — NO

| Vía | Resultado medido |
|---|---|
| (a) `admin_settings.json` con `models.list` | **Sin efecto**: el catálogo salió idéntico (240.754 B, mismo sha16) tras escribirlo. Además la ruta que busca el binario es `/etc/antigravity/admin_settings.json` ⇒ en Android es `/system/etc/antigravity/…`, **no escribible** sin root en la app. |
| (b) `SetUserSettings` (y `GetUserSettings`) | **501** `unimplemented: unimplemented (error ID: …)`. No hay RPC para escribir el catálogo en este build. |
| (c) Formato del `models.list` | **Deducido del binario** (ver abajo), pero el Tier 1 no llega a mover el catálogo. |
| (d) BYOK del IDE | **Existe** y es la vía real (§4.2). Falta el endpoint compatible. |

**Formato real deducido de `liblanguage_server.so`** (no inventado):

* Ruta de settings: `/etc/antigravity/admin_settings.json` (Linux),
  `Antigravity\admin_settings.json` (Windows),
  `/Library/Application Support/Antigravity/admin_settings.json` (macOS).
* Tipo Go: `gateway.AdminSettings` → `gateway.ModelsConfig` →
  `[]gateway.ModelEntry`, con `ModelID string \`json:"modelId"\``,
  `DisplayName \`json:"displayName,omitempty"\``,
  `SupportsThinking *bool \`json:"supportsThinking,omitempty"\``,
  `SupportedMimeTypes map[string]bool \`json:"supportedMimeTypes,omitempty"\``,
  `MaxTokens int64 \`json:"maxTokens,omitempty"\``,
  `MaxOutputTokens int64 \`json:"maxOutputTokens,omitempty"\``,
  `ThinkingBudget int64 \`json:"thinkingBudget,omitempty"\``.
  ⇒ forma del fichero: `{"models":{"list":[{"modelId":"…","displayName":"…"}]}}`.
  El error de validación del propio binario lo delata:
  `models.list[%d] has an empty modelId`.
* **Los 4 tiers** de resolución del catálogo (cadenas literales del binario):
  * `[ModelConfig] Resolving models from Tier 1 (Inline admin_settings models.list)`
  * `[ModelConfig] Tier 1 models.list parse failed: %v. Falling back to next tier.`
  * `[ModelConfig] Tier 2 remote catalog fetch failed: %v. Falling back to Tier 3.`
  * `[ModelConfig] Resolving models from Tier 3 (Gateway discovery at %s, wireProtocol: %s)`
  * `[ModelConfig] Resolving models from Tier 4 (Built-in default catalog fallback)`
* **Las palancas del gateway** (env): `AGY_LLM_GATEWAY_URL`,
  `AGY_LLM_GATEWAY_API_KEY`, `AGY_LLM_GATEWAY_HEADERS`,
  `AGY_LLM_GATEWAY_CA_CERT`, `AGY_LLM_GATEWAY_MODELS`,
  `AGY_LLM_GATEWAY_PROXY_URL`, `AGY_LLM_GATEWAY_WIRE_PROTOCOL`.

⇒ **La vía que queda** (y es la que hay que hacer): levantar un *gateway*
compatible (OpenAI/Gemini) que sirva MiniMax en el móvil y apuntar el
language server a él con `AGY_LLM_GATEWAY_*`. Requiere **arrancar el LS con
esas variables**, y hoy el LS lo lanza la app ⇒ toca hacerlo desde el PC con
ADB o desde un `service.d` propio. **NO VERIFICADO**: que el Tier 3 llegue a
pintar el selector de la UI.

### 4.2 El formato del BYOK del propio IDE (medido en el bundle web)

`web/generated/main.js` (9,9 MB) construye los modelos propios así:

```js
modelInfo = {
  modelId: I.Model.GOOGLE_GEMINI_INTERNAL_BYOM,   // enum del proto
  apiProvider: I.APIProvider.API_PROVIDER_GOOGLE_EVERGREEN,
  chatModelName: <etiqueta>,                        // p.ej. "MiniMax M3.1 Flash"
  modelName: "gemini-api:/models/<model>",          // esquema gemini-api:
  maxTokens: <n>,
  modelFeatures: { zeroShotCapable, supportsImages, supportsToolCalls, supportsThinking }
}
// y el mapa: { "<etiqueta>": modelInfo }
```

El diálogo valida: *"Model must be available on the Gemini API and use the
gemini-api scheme"* (placeholder: `gemini-api:/models/gemini-v3-byom`).
Del lado del proto: `exa.codeium_common_pb.UserSettings.custom_models`
(mapa `string → ModelInfo`) y `jetbox_state_pb.CustomModelsConfig.custom_models`,
que el móvil **no tiene** (`jetski_state.pbtxt` sin `custom_models`).
El fichero BYOK que sí existe en el móvil
(`.gemini/antigravity/custom_models.json`, 0600) tiene **otra forma**
(`{models:[{provider,modelId,displayName,apiKey}]}`) ⇒ es nuestro, no lo lee
la app ⇒ **inerte para el selector**.

⇒ Para poner MiniMax en el selector harían falta **dos** cosas: (i) el
endpoint compatible con Gemini para MiniMax, y (ii) registrar el modelo
propio en `custom_models` del estado de la app (proto, no JSON nuestro).

### 4.3 Otros huecos (NO VERIFICADO / pendientes)

* El **panel GUI** está entregado como pieza enchufable, **no** integrado en la
  plantilla vigente (rediseño en curso): falta aplicarlo sobre la plantilla
  nueva (3 pasos documentados en `web/panels/antigravity_panel.html`).
* `enqueue` está implementado pero **no se ha ejecutado contra el móvil**
  (el móvil cayó antes): falta una prueba real de `StartCascade` +
  `SendUserCascadeMessage` con el modelo del estado.
* El **BYOK de MiniMax** necesita el gateway (§4.1): no implementado.

---

## 5. Cómo seguir (orden recomendado)

1. **Re-medir** cuando el móvil vuelva (el daemon está sin servicio): repetir
   `GET /` → `csrfToken` → `GetAvailableModels` y comprobar que siguen siendo
   **36 modelos** y que `MODEL_PLACEHOLDER_M526` ⇄ `gemini-4-hydrogen-medium`
   se resuelve en los dos sentidos. El sha16 del catálogo medido era
   `7179fda6f56a878a` (240.754 B).
2. **Probar `enqueue`** de verdad (1 prompt corto) y anotar el `cascadeId`.
3. **Gateway para MiniMax**: `AGY_LLM_GATEWAY_*` + un endpoint Gemini-compatible
   en el móvil ⇒ comprobar si el Tier 3 puebla el selector (es la única vía
   que queda, y el Tier 1 está bloqueado por `/system/etc`).
4. **Aplicar el panel** sobre la plantilla nueva del cockpit y **P0-1** (chat
   del agente contra `POST /v1/chat/completions`) cuando el jefe dé el SHA
   base ⇒ el backlog y sus puertas están en `docs/BACKLOG_AGENTE_EN_MEGADAEMON.md`.
5. **No tocar `/system/etc/antigravity/admin_settings.json`**: es un sistema
   de sólo lectura ⇒ el BYOK del IDE (o un módulo Magisk de overlay) es la
   vía limpia.

---

## 6. Trazabilidad

| afirmación | de dónde sale |
|---|---|
| cabecera `x-codeium-csrf-token` y 401 literal | medición en el móvil, 2026-11-22 (curl contra `127.0.0.1:45157`) |
| catálogo de 36 modelos + `defaultAgentModelId` | `GetAvailableModels` (sha16 `7179fda6f56a878a`) |
| `MODEL_PLACEHOLDER_M526` = `gemini-4-hydrogen-medium` | catálogo + `antigravity_state.pbtxt` |
| `GetUserSettings`/`SetUserSettings` 501 | medición directa |
| `gateway.ModelEntry` y sus tags JSON | strings del `.so` (`go.shape.struct { ModelID string "json:\"modelId\""; … }`) |
| los 4 tiers del catálogo | strings del `.so` (`[ModelConfig] Resolving models from Tier N …`) |
| `AGY_LLM_GATEWAY_*` | strings del `.so` |
| formato BYOK (`gemini-api:/models/…`, `MODEL_GOOGLE_GEMINI_INTERNAL_BYOM`) | `web/generated/main.js` (bundle de la app) |
| 332 métodos del servicio | cadenas `/exa.language_server_pb.LanguageServerService/<Método>` del `.so` |
| tareas `task_442838` / `task_553653` | `daemon/tasks.json` del móvil |
| ciclo de 211 s / 570 s, 37 ciclos, 923 descompiladas | `tasks.json` (`last_run_duration`, `last_output`) y `re_pipeline/megadaemon_state.json` |
| load 19,85 y RSS de `libloader.so`/`com.agy` | `/proc/loadavg`, `/proc/<pid>/status` del móvil |
| oauth token sha16 `1028cbc50fe6b8db` | `jetski-standalone-oauth-token` (sha16, **sin imprimir el valor**) |
| causa de la caída del daemon (tope de RSS / racha de sondas) | `deploy/99-megadaemon.sh:670-692` y `:850` |
