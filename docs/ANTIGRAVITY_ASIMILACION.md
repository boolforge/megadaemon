# ASIMILACIÓN DE ANTIGRAVITY POR EL MEGADAEMON

> **Objetivo (decidido por el usuario, 2026-11-22)**: que Antigravity sea,
> cada vez más, **sólo una GUI del MegaDaemon canónico** — sin duplicidades,
> sin doble motor, sin ineficiencias.
>
> Todo lo de aquí está **medido** en el S21 Ultra del usuario (root, vía el
> MCP del propio daemon) o **derivado del código del repo** con
> `fichero:línea`.  Lo que no se ha podido verificar se marca
> **NO VERIFICADO** y se dice cuál es el camino más cercano.

---

## 0. Resumen ejecutivo

| | Antes (medido 2026-11-22) | Después (esta rama) |
|---|---|---|
| Catálogo de tools que ve Antigravity | **26** (copia congelada en disco, `instructions.md` decía "v13.3") | **62**, el **catálogo vivo** del daemon (`http://127.0.0.1:9090/mcp`) |
| Motor de *pull* del código | **2** (el del daemon + el propio de `antigravity_clone.rs`) | **1** (el canónico: `self_update` + `deploy/instalador-megadaemon.sh`) |
| Cliente LLM | **2** rutas (la clave del proveedor dentro de Antigravity + el gateway) | **1**: el gateway canónico |
| Dialecto que habla el daemon | 1 (OpenAI) | **2**: OpenAI (los demás clientes) + **Gemini** (el idioma de Antigravity) |
| Entorno de Antigravity | invisible (nadie lo catalogaba) | **catalogado y utilizable** (`antigravity_env.rs`) |

---

## 1. Inventario de duplicidad (con evidencia)

### 1.1 Tabla

| # | Capacidad | Dónde está **duplicado** (dentro de Antigravity) | Dónde está el **canónico** | Evidencia (fichero:línea / medición) |
|---|---|---|---|---|
| **D1** | **Catálogo de tools** | `…/.gemini/antigravity-app/mcp/megadaemon-minimax/*.json` — **26 ficheros JSON** (197–870 B, escritos 2026-11-22 13:18) + `instructions.md` ("MegaDaemon v13.3") | `rust-core/src/mcp_server.rs:20` (`tool_definitions()`) agrega `android.rs:896` + `mcp_ext.rs:15` + los módulos nuevos ⇒ **62 tools** servidas por `POST /mcp` | MEDIDO: `POST 127.0.0.1:9090/mcp tools/list` ⇒ **62**; `ls mcp/megadaemon-minimax/` ⇒ **26** ficheros. El LS **no** usaba el plugin: `GetMcpServerStates` ⇒ `spec.serverUrl=http://127.0.0.1:9090/mcp`, `tools=26` (**caché vieja** del daemon v13.3) |
| **D2** | **Cliente LLM / credenciales** | `…/.gemini/config/mcp_config.json` → `providers.minimax.api_key` (**la misma clave** MiniMax) y `…/.gemini/antigravity/custom_models.json` → `apiKey` (**la misma clave**, otra vez) | `rust-core/src/llm_gateway.rs` (gateway OpenAI) + `rust-core/src/providers.rs:621` (`execute_chat`: routing + *fallback* real) | MEDIDO: la clave `sk-cp-2ZP8…` aparece **literalmente igual** en los dos ficheros del móvil |
| **D3** | **Modelo de la IDE** | `…/.gemini/antigravity/custom_models.json` con un formato **inventado** (`models[]` con `provider`/`modelId`/`displayName`/`apiKey`) | `appState.customModelsConfig.customModels` (BYOM) en `antigravity_state.pbtxt` | MEDIDO: `main.js` lee `b.appState?.customModelsConfig?.customModels` (offset 9783901 del bundle de 9.902.890 B). **`custom_models.json` no lo lee nadie del lado de la GUI** ⇒ por eso el modelo **no aparecía** en pantalla |
| **D4** | **Motor de *pull* del código** | `rust-core/src/antigravity_clone.rs` → `sincronizar_codigo()` bajaba el tarball de `main` de GitHub y lo aplicaba sobre el clon (con su propio gestor de respaldos) | `rust-core/src/self_update.rs` (auto-update del binario) + `deploy/instalador-megadaemon.sh --instalar` | `antigravity_clone.rs:275-419` (MEDIDO: **la función no tenía ni un llamante** en todo el repo ⇒ motor muerto) |
| **D5** | **Workspace / clon del repo** | `…/com.agy/files/megadaemon/` (copia del repo, con su propio `.git`) y `rust-core/src/unifier.rs:57` (escaneo con **9 rutas fijas** bajo `/data/user/0/com.agy/files/…`) | El repo canónico (`boolforge/megadaemon` @ `main`) + el **binario** canónico `/data/local/tmp/megadaemon/megadaemon-core` | `unifier.rs:58-122` (rutas hardcodeadas) · `antigravity_clone.rs:27` (`CLON`) |
| **D6** | **Binario del daemon** | `…/runtime/bin/megadaemon-core` **y** `…/runtime/bin/megadaemon-core.prev` (la copia anterior) | `/data/local/tmp/megadaemon/megadaemon-core` | `antigravity_clone.rs:205` (`unificar_runtime()`) — MEDIDO en el móvil: `runtime/bin/megadaemon-core` (11.928.296 B) + `.prev` |
| **D7** | **Entorno y utilidades** | `…/runtime/bin/` (**862 lanzadores** ELF de 781–814 B) + `…/usr/bin/` (**849 binarios** reales, 45 > 500 KB) | **No existía**: ahora `rust-core/src/antigravity_env.rs` | MEDIDO: 862 entradas / 849 sha256 distintos / 197 MB en `runtime/bin`; `usr/bin` con `curl 8.22.0`, **`git 2.56.0`**, `python3 3.14.10`, `openjdk 17.0.21.1`, `aapt` 1.360.584 B, `zipalign`, `llvm-ar`, `node` 51.010.000 B, `cmake` |
| **D8** | **Sesiones de la IDE** | `rust-core/src/antigravity_sessions.rs` (rescatado de la rama `agent/rescate-antigravity-22nov`, commit `a995194`, **sin revisar**) | `rust-core/src/antigravity_bridge.rs` (Connect RPC real: `GetAllCascadeTrajectories`, `SearchConversations`, `GetCascadeTrajectory`) | `antigravity_bridge.rs:616-643` (medidas reales: 200 con trajectories, 200 con search) |
| **D9** | **Credenciales del proveedor** | `mcp_config.json` (`providers.minimax.api_key`) | El **vault** del daemon (`api_key.rs` + `providers.rs`) | MEDIDO: la clave está **en claro** en el fichero de la app; en el daemon vive en `state/megadaemon_api.key` (0600) |

> **D8 (nota)**: `antigravity_sessions.rs` **no** está en `main` (vive sólo en la
> rama de rescate).  No se ha integrado: `antigravity_bridge.rs` ya cubre lo
> mismo por RPC (fuente medida).  Se deja **marcado** en vez de traido, para
> no meter un segundo camino de sesiones.

### 1.2 El caso especial: el catálogo de tools "congelado"

Éste es el hallazgo más importante y **no era visible en el repo**: el
language server de Antigravity **no** leía el plugin de disco, leía el
servidor MCP por HTTP… **pero con la lista de tools cacheada de cuando se
conectó**.

MEDIDO (2026-11-22, en el móvil):

```
GetMcpServerStates →
  megadaemon-minimax | READY | tools= 26 | url= http://127.0.0.1:9090/mcp
POST http://127.0.0.1:9090/mcp tools/list → 62 tools     (el daemon vivo, v13.4.0)
```

⇒ La GUI veía **26** tools (las del daemon v13.3) mientras el daemon ya
servía **62**.  La RPC `RefreshMcpServers` (descubierta en
`liblanguage_server.so`: `_LanguageServerService_RefreshMcpServers_Handler`)
fuerza el reconnect:

```
RefreshMcpServers → {}
GetMcpServerStates → megadaemon-minimax | READY | tools= 62
```

Y tras un **reinicio limpio de la GUI** (`am force-stop com.agy` +
`am start`, verificado: el language server vuelve, `SPA → HTTP 200`), ya
sale **62 sin refrescar a mano** ⇒ la duplicidad desaparece de raíz.

### 1.3 El caso especial: por qué el modelo "no aparecía en la GUI"

El usuario escribió `custom_models.json` con este formato:

```json
{"models":[{"provider":"minimax","modelId":"MiniMax-M3.1-Flash",
             "displayName":"MiniMax M3.1 Flash","apiKey":"sk-cp-…"}]}
```

Ese formato **no lo lee nadie** de la GUI: el proveedor real de la GUI es

```js
// main.js (MEDIDO)
this.customModelsProvider = zi(this.rootProvider,
  b => ({customModels: b.appState?.customModelsConfig?.customModels ?? {}}));
```

es decir, el estado **persistido en la IDE** (`antigravity_state.pbtxt`), no
un fichero suelto.  El formato correcto es:

```protobuf
custom_models_config: {
  custom_models: {
    key: "megadaemon"
    value: {
      api_provider: API_PROVIDER_GOOGLE_EVERGREEN
      chat_model_name: "megadaemon"
      model_name: "http://127.0.0.1:9090"
      max_tokens: 32768
      model_features: { zero_shot_capable: true supports_tool_calls: true }
    }
  }
}
```

**MEDIDO**: ese bloque está **escrito y persistido** en el móvil (ver §3).

---

## 2. Plan de jerarquía: qué módulo es el canónico de cada capacidad

| Capacidad | **CANÓNICO** (una sola fuente) | Qué se degrada / elimina | Por qué |
|---|---|---|---|
| Catálogo de tools | `rust-core/src/mcp_server.rs:20` → `POST /mcp` (62 tools) | El plugin estático `mcp/megadaemon-minimax/*.json` (26) | El daemon es el dueño del catálogo; la IDE lo consume por HTTP (ya configurado en `mcp_config.json`) |
| Cliente LLM y routing | `rust-core/src/llm_gateway.rs` + `providers.rs:621` | La clave del proveedor **dentro** de Antigravity (`providers.minimax`) | Un solo camino al modelo ⇒ el daemon decide el proveedor y el *fallback* |
| Dialecto del gateway | `llm_gateway.rs`: OpenAI (`/v1/chat/completions`) **y** Gemini (`/v1internal:generateContent`) | — | El gateway tiene que hablar el idioma de cada cliente; el de Antigravity es **Gemini**, no OpenAI |
| Modelo de la IDE | `appState.customModelsConfig.customModels` (BYOM) | `custom_models.json` (formato no soportado) | Es lo que la GUI lee de verdad |
| Sesiones de la IDE | `antigravity_bridge.rs` (Connect RPC medido) | `antigravity_sessions.rs` (rescatado, sin revisar, sin llamantes) | Un solo camino de verdad (la API real) |
| Código del repo | `boolforge/megadaemon@main` + `deploy/instalador-megadaemon.sh` | El clon de `/data/user/0/com.agy/files/megadaemon/` y su motor de pull | El clon es una **copia** que la GUI no necesita: sólo necesita el binario |
| Binario | `/data/local/tmp/megadaemon/megadaemon-core` → copiado a `runtime/bin/` por `unificar_runtime()` | `runtime/bin/megadaemon-core.prev` (copia vieja) | Un binario, una versión, un auto-update |
| Entorno y utilidades | `rust-core/src/antigravity_env.rs` (catálogo) + `android::root_exec` (uso) | — | Se **absorbe** el entorno en vez de adivinarlo |
| Credenciales | `state/megadaemon_api.key` (0600) en el daemon | Claves en claro en ficheros de la app | Una sola bóveda |

---

## 3. Antigravity usa SIEMPRE `megadaemon` — lo que está HECHO y MEDIDO

### 3.1 El modelo `megadaemon` está registrado en la IDE  ✅ MEDIDO

```
JetboxWriteState {appState:{customModelsConfig:{customModels:{"megadaemon":{…}}}}}
→ respuesta {} (OK)

antigravity_state.pbtxt (MEDIDO en el móvil, con el diff contra el respaldo):
+custom_models_config: {
+  custom_models: {
+    key: "megadaemon"
+    value: {
+      max_tokens: 32768
+      model_features: { zero_shot_capable: true supports_tool_calls: true }
+      api_provider: API_PROVIDER_GOOGLE_EVERGREEN
+      model_name: "http://127.0.0.1:9090"
+      chat_model_name: "megadaemon"
+    }
+  }
+}
```

El `model_id` no sale porque `GOOGLE_GEMINI_INTERNAL_BYOM` (=326) es el valor
**por defecto** del enum (proto3 omite los valores por defecto) ⇒ coincide
con lo que pone la GUI.

Método RPC usado: `POST https://127.0.0.1:45157/exa.language_server_pb.LanguageServerService/JetboxWriteState`
(el mismo servicio Connect que el resto; el nombre del método está en
`liblanguage_server.so`: `_LanguageServerService_JetboxWriteState_Handler`).

### 3.2 El catálogo vivo ya es el que ve la GUI  ✅ MEDIDO

```
(después de reiniciar la GUI)
GetMcpServerStates → megadaemon-minimax | READY | tools= 62 | url=http://127.0.0.1:9090/mcp
```

### 3.3 El modelo en el **selector** de la GUI  ❌ NO VERIFICADO — BLOQUEADO POR EL PRODUCTO

Captura de la GUI (`docs/media/` o en el informe del agente): el panel "Model"
de Antigravity **no** muestra "megadaemon"; muestra los modelos de Google
(Gemini 4 Neon/Argon/Hydrogen, Claude Opus 5.5, GPT-OSS 120B…).

**Por qué** (MEDIDO en `main.js`, no es un error de configuración):

```js
// Ycc() — el proveedor de modelos de la GUI
var r = k.osName === "linux" && k.isGoogleInternal;   // ← "supportsCustomModels"
k = k.isGoogleInternal;
w = useMemo(() => { var B = cKa(l); for (let C of B) C.disabled = !r; return B }, [l, r]);
//   ⇒ en Android (osName != "linux") TODOS los modelos personalizados salen disabled
```

⇒ **El BYOM (modelo personalizado) está restringido por la propia GUI a
Linux + builds "Google internal"**.  En Android no se ofrece, ni aunque el
modelo esté registrado perfectamente (lo está: §3.1).

**Camino más cercano (el que sí funciona)**: que la cascada se lance con el
modelo personalizado **explícitamente**, que es lo que hace la GUI cuando
eligues uno (`plannerConfig.planModel = GOOGLE_GEMINI_INTERNAL_BYOM` +
`plannerConfig.customModelInfoOverride = <ModelInfo>`):

* `antigravity_bridge.rs:653` (`enqueue`) ya manda `cascadeConfig.plannerConfig`;
* `antigravity_model.rs` construye exactamente el `ModelInfo` que la GUI
  pondría (`model_info()`), con los mismos nombres de campo;
* el language server habla **Gemini** con ese modelo ⇒ el endpoint
  `POST /v1internal:generateContent` del gateway canónico.

**NO VERIFICADO de punta a punta**: que una cascada con
`customModelInfoOverride` llegue a llamar al gateway.  Requiere el binario
ARM64 con el dialecto Gemini desplegado en el móvil (esta rama lo añade;
el despliegue es el paso siguiente).

### 3.4 Lo que el language server habla (MEDIDO en el binario)

De `liblanguage_server.so` (181.358.800 B), strings reales:

| String | Qué implica |
|---|---|
| `google3/third_party/jetski/models/model_config/model_config.WithAllowBYOM` | El BYOM es el camino de los modelos propios |
| `google3/third_party/golang/google_golang_org/genai/v/v0/google/genai/genai.Models.generateContent` | El protocolo es **Gemini** (`genai` SDK de Google), **no** OpenAI |
| `/v1internal:generateContent` y `/v1internal:streamGenerateContent` | Las rutas concretas del proveedor `API_PROVIDER_GOOGLE_EVERGREEN` |

⇒ Por eso el gateway **`llm_gateway.rs`** ahora habla los dos dialectos:
OpenAI para el resto de clientes, Gemini para Antigravity.

---

## 4. Qué absorbe MegaDaemon de Antigravity (implementación real)

### 4.1 `rust-core/src/antigravity_env.rs` (NUEVO) — el entorno

Cataloga el userland de Termux que Antigravity trae en su sandbox y lo
convierte en una **capacidad del daemon** (no en un misterio):

* `resolve(nombre)` ⇒ lanzador (`runtime/bin/x`) + binario real (`usr/bin/x`).
* `catalog(limit)` ⇒ inventario del runtime.
* `key_tools()` ⇒ las 14 herramientas que de verdad importan.
* `version_of(nombre)` ⇒ la versión **real**, ejecutando el lanzador con
  timeout (nada de bucles colgados: el móvil va justo de RAM).
* `environment_report()` ⇒ informe JSON (CLI, MCP y doc).

**Corrección de una afirmación falsa que estaba en el repo**:
`antigravity_clone.rs` decía *"el móvil **no** tiene `git`"*.  Es **FALSO**
(MEDIDO: `runtime/bin/git` → `git version 2.56.0`).  Ninguna función del
daemon debe volver a suponer que no hay `git`.

### 4.2 `rust-core/src/antigravity_model.rs` (NUEVO) — el modelo

* `model_info(spec)` ⇒ el `ModelInfo` **idéntico** al que construye la GUI.
* `app_state_payload()` / `custom_models_from_app_state()` ⇒ el estado.
* `register()` / `remove()` / `current_models()` ⇒ vía `JetboxWriteState`.
* `patch_provider_key()` ⇒ la credencial en `mcp_config.json`, **sin**
  romper lo que ya había y **sin** que la clave entre en el repo.

### 4.3 `rust-core/src/llm_gateway.rs` (AMPLIADO) — el dialecto Gemini

`gemini_route()`, `gemini_prompt()`, `gemini_generate_content()`,
`gemini_stream_sse()`: traducción **pura** de `contents`/`systemInstruction`/
`generationConfig` ⇒ `providers::execute_chat` ⇒ respuesta con forma
`candidates`/`usageMetadata` + el bloque de trazabilidad `megadaemon`.

Rutas servidas por el cockpit (`cockpit_server.rs`):
`/v1internal:generateContent`, `/v1internal:streamGenerateContent`,
`/v1beta/models/{model}:generateContent|streamGenerateContent`.

> **Seguridad**: esas rutas exigen la API key **salvo** que la petición sea
> *local* (peer loopback + `Host` loopback + sin cabeceras de proxy): el
> language server no puede mandar nuestro token de sesión.  El túnel público
> (Host del dominio) **sigue exigiendo API key** ⇒ no se abre nada al
> exterior.

### 4.4 `rust-core/src/antigravity_clone.rs` — degradado a **puente fino**

* **Se queda**: la medición (`estado`, `estado_clon`, `estado_runtime`) y la
  unificación (`unificar_runtime`).
* **Se va**: el **segundo motor de pull** (`sincronizar_codigo` bajaba el
  tarball de `main` y lo aplicaba).  Ahora **delega** en el mecanismo
  canónico (`deploy/instalador-megadaemon.sh --instalar`) y, si no está,
  **lo dice** (con `mecanismo_canonico: None` y el motivo en `detalle`).
  **Sustituto equivalente**: binario → `unificar_runtime()`; instalación
  completa → el instalador canónico.  No se borra nada sin equivalente.

---

## 5. Lo que se elimina en el móvil (con su sustituto)

| Qué | Sustituto | Estado |
|---|---|---|
| Las **26** definiciones congeladas de `mcp/megadaemon-minimax/*.json` | El **catálogo vivo** del daemon (62 tools) por `http://127.0.0.1:9090/mcp`, ya configurado en `mcp_config.json` | **Equivalente probado** (`GetMcpServerStates` → `tools=62`) |
| La **clave del proveedor** en `mcp_config.json` (`providers.minimax.api_key`) y en `custom_models.json` | El vault del daemon (`state/megadaemon_api.key`, 0600) | Pendiente de limpiar en el móvil (el `mcp_config.json` necesita `X-API-Key` para el `/mcp`: **esa** sí se queda; la de MiniMax, no) |
| `custom_models.json` (formato no soportado) | `appState.customModelsConfig.customModels` | Formato inventado por un agente anterior; **no lo lee nadie** |

> `mcp_config.json` **no se toca**: su `X-API-Key`/`X-Seere-Auth-Token` son
> la credencial del **puente MCP** (distinta cosa que la clave del
> proveedor de modelos).

---

## 6. Paridad

| Superficie | Dónde | Estado |
|---|---|---|
| CLI | `rust-core/src/bin/md-antigravity.rs`: `md-antigravity model [set\|remove]`, `md-antigravity env [--tool X]` | ✅ |
| MCP | `antigravity_model` y `antigravity_env` en el catálogo + dispatch (`mcp_server.rs`) | ✅ |
| `agent_coding` | hereda el catálogo MCP (`tool_policy.rs` enruta por el mismo contrato) | ✅ (mismas tools, mismo contrato) |
| Tests | `rust-core/tests/antigravity_asimilacion.rs` (13 tests) + tests del módulo | ✅ |

---

## 7. Pendientes honestos

| # | Pendiente | Por qué |
|---|---|---|
| P1 | **Desplegar el binario ARM64** con el dialecto Gemini y comprobar que una cascada con `customModelInfoOverride` llama a `/v1internal:generateContent` | Requiere build ARM64 + push + reinicio **sólo** del daemon (no del LS) |
| P2 | Que "megadaemon" **se vea** en el selector de la GUI | Bloqueado por el producto: `supportsCustomModels = osName==="linux" && isGoogleInternal` |
| P3 | Limpiar del móvil la clave del proveedor en `mcp_config.json` y `custom_models.json` | Requiere el paso P1 para no dejar a Antigravity sin modelo |
| P4 | Retirar el clon `…/com.agy/files/megadaemon/` | Sólo cuando el usuario confirme que no se usa como workspace de trabajo |
| P5 | `antigravity_sessions.rs` (rescatado) | **Marcado**: `antigravity_bridge.rs` ya cubre las sesiones por la API real |

---

## 8. Cómo reproducir las mediciones

```bash
# 1. Catálogo vivo vs. catálogo congelado
tools/md_mcp.sh list                      # 62 tools (lo que ve Antigravity)
curl -sk https://127.0.0.1:45157/ ...    # (en el móvil) GetMcpServerStates

# 2. Refrescar el catálogo de la IDE (si el daemon se ha actualizado)
POST https://127.0.0.1:45157/exa.language_server_pb.LanguageServerService/RefreshMcpServers  {}

# 3. Registrar el modelo
POST …/LanguageServerService/JetboxWriteState
{"appState":{"customModelsConfig":{"customModels":{"megadaemon":{…}}}}}

# 4. Ver el estado persistido
grep -A12 custom_models_config …/antigravity_state.pbtxt
```

Desde el repo:

```bash
md-antigravity model          # lista los modelos personalizados de la IDE
md-antigravity model set      # registra "megadaemon" -> http://127.0.0.1:9090
md-antigravity env            # catálogo del entorno de Antigravity
```
