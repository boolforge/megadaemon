# ANTIGRAVITY INTEL — el cliente `com.agy` (AGY v2 para Android) y su MCP

> **Workstream W5** del PLAN MAESTRO v15 (`docs/PLAN_S21_ULTRA_V15.md`, commit `9184140`).
> Parte de DEX/inventario: agente `659db26c`. Con el agente de apps/ocultación (`275a2232`).
>
> **Reglas aplicadas**: nada de credenciales en el repo, en el chat ni en el estado compartido
> (aquí sólo hay **nombres de campo** y **sha16**); RE **del binario que realmente corre** en el
> dispositivo, no del paquete de distribución (lección del nodo `cloud-sandbox` con
> `libeden_ud_npu.so`); el móvil **no compila** (3 GB de RAM, disco al 98 %) ⇒ builds en disco local.

---

## 0. Resumen ejecutivo (lo que hay que saber de un vistazo)

1. **Antigravity está instalado en el Samsung** como paquete `com.agy` ("AGY v2"), APK de
   **103.469.317 B**, `sha256 = c43b4adbc20050619dc441522385cd78500d3b8fd00a525b3d15f0ee258411b8`.
2. El APK **no** contiene la lógica del producto: `classes.dex` son **212.260 B** (un shell
   WebView). El producto real está en `assets/runtime/bin/liblanguage_server.so`
   (**181.358.800 B**, binario **Go** de Google) y en `assets/env/` + `assets/web/`.
3. **Antigravity YA tiene registrado a MegaDaemon como servidor MCP** (`megadaemon-minimax`)…
   …pero **está roto**: el comando apunta a una copia de nuestro binario que **no existe** en el
   dispositivo ⇒ el servidor MCP no arranca (los *schemas* de las tools sí quedaron cacheados, por
   eso "parece" que funciona).
4. El SDK oficial de Antigravity (`google-antigravity-sdk`) admite **stdio y Streamable HTTP con
   cabeceras** ⇒ el puente correcto es **apuntar al daemon vivo** (`http://127.0.0.1:9090/mcp` +
   `X-API-Key`), no lanzar una copia del binario.
5. El binario nativo implementa **MCP por sí mismo** (`mcp` ×6.182, `jsonrpc` ×300 en las cadenas)
   y habla con **OpenAI** (×13.133), **Gemini** (×3.009), **Anthropic** (×119) ⇒ ya es un cliente
   MCP de pleno derecho.

---

## 1. Inventario del cliente (DoD W5.1)

### 1.1 Binarios y artefactos (en el APK, sha256 del APK arriba)

| Artefacto | Tamaño | Qué es |
|---|---:|---|
| `classes.dex` | 212.260 B | Shell WebView de la app Android (permisos, servicios, receivers) |
| `assets/runtime/bin/liblanguage_server.so` | 181.358.800 B (52.898.787 B comprimido) | **El producto**: language server de Google en Go (MCP, proveedores, agentes) |
| `assets/runtime/bin/libloader.so` | 242.560 B | Cargador del runtime |
| `assets/runtime/lib/liblse_compat.so` | 70.736 B | Compat LSE (ARMv8.0-A + emulación) |
| `assets/env/overlay/lib/daemon/{executor,triggers,scheduler}.py` | 6,4/5,5/7,0 KB | Lógica del daemon (hooks y disparadores) |
| `assets/env/overlay/etc/antigravity/AGENTS.md` | 7.785 B | Reglas del agente cargadas en **cada** conversación |
| `assets/env/overlay/bin/agy-{daemon,notify,upgrade,fix-paths}` | 3,6–21,2 KB | CLI del daemon, notificaciones, upgrade, rutas |
| `assets/web/generated/main.js` | 9.902.890 B | Bundle web de la UI (307 ficheros en `assets/web/`) |
| `assets/env/manifest.json` | 413 B | `bootstrapSha256`, `overlaySha256`, `pkgSha256`, `bundleId` |

### 1.2 Layout en el dispositivo (`/data/user/0/com.agy/files`)

```
files/                        $HOME del usuario Linux embebido ($PREFIX = files/usr)
  .gemini/                    ← estado de IA (Gemini-compatible)
    config/
      config.json             ← configuración global + PROVEEDORES (credenciales)
      mcp_config.json         ← SERVIDORES MCP declarados  ★
      AGENTS.md, plugins/, skills/, projects/, sidecars/
    antigravity-app/
      mcp/<server-id>/*.json  ← schemas de las tools por servidor MCP
      daemon/tasks.json       ← HOOKS / tareas programadas
      daemon/daemon.log, daemon.pid
      brain/<conv-id>/…        ← conversaciones + artefactos
      crashes/…
      installation_id, jetski_state.pbtxt, antigravity_state.pbtxt
    events.jsonl              ← eventos del runtime (language_server:stderr…)
    jetski-standalone-oauth-token   ← OAuth del runtime "Jetski"  ★ credencial
  runtime/                    ← binario extraído del bootstrap (el que corre)
  megadaemon/                 ← clon git de nuestro repo dentro del sandbox
  uploads/                    ← ficheros subidos por el cockpit web
```

### 1.3 Plugins y skills instalados (`~/.gemini/config/`)

* **Plugins (7)**: `android-cli-plugin`, `chrome-devtools-plugin`, `data-agent-kit-plugin`,
  `gemini-api`, **`google-antigravity-sdk`**, `google-developer-knowledge`,
  `modern-web-guidance-plugin`.
* **Skills (6)**: `advanced-ui-ux`, **`minimax-megadaemon`**, `mobile-design`, `rust-pro`, `ui-ux-pro`.

La skill `minimax-megadaemon` documenta el flujo previsto: *"Via MCP Server
(megadaemon-minimax): call `minimax_chat(...)` or `swarm_chat(...)`"*, con un **fallback CLI**
("Zero-Latency Pure Rust") y la ruta de subidas por el cockpit web
(`http://127.0.0.1:3000` → `files/megadaemon/uploads/`).

### 1.4 Hooks / disparadores (`daemon/tasks.json`)

| id | nombre | trigger | comando |
|---|---|---|---|
| `task_442838` | `Snatcher-MegaDaemon-SuperLoop` | `interval` 300 s | `python3 …/scummvm/tools/re_pipeline/snatcher_megadaemon.py --once` |
| `task_553653` | `minimax_bootstrap` | (script de arranque) | `bash …/modules/runner/bootstrap_core.sh` |

`agy-daemon` es el planificador de fondo (<10 MB RAM, 0 % CPU en reposo, corre como *foreground
service* con wakelock parcial). Tipos de trigger soportados: `--delay`, `--interval`, `--cron`,
`--watch-file`, `--github-pr`, `--battery-below`, `--prompt` (tarea de IA) y `--command`.
Formato de evento del runtime (`events.jsonl`):
`{"event":"system.error","timestamp":<ms>,"source":"language_server:stderr","data":{…}}`.

### 1.5 Permisos y capacidades

* `rish` (Shizuku) da shell **uid 2000**: `am`, `logcat`, `dumpsys`, `getprop`, `settings`.
* Puertos reservados del core: **45157** (release) y **45158** (dev) — no usarlos para servidores.
* Puertos "seguros" para desarrollo: 3000, 5173, 8000, 8080.
* `/storage/emulated/0` va montado `noexec` ⇒ los scripts se invocan por intérprete.

---

## 2. ¿Qué protocolo habla? (DoD W5.2)

**MCP (Model Context Protocol)**, y con doble transporte:

```python
# del SDK oficial google-antigravity-sdk (plugin instalado en el propio dispositivo)
types.McpStdioServer(name="my_stdio_server", command="python3", args=["mcp_server.py"])
types.McpStreamableHttpServer(name="my_http_server", url="https://example.com/mcp",
                              headers={"Authorization": "Bearer your-token-here"})
```

* En el binario nativo: `mcp` ×6.182, `jsonrpc` ×300, `oauth` ×1.279, `api_key` ×162,
  `apikey` ×444, `/v1/` ×15.268, `openai` ×13.133, `gemini` ×3.009, `anthropic` ×119.
* El propio cliente trae **filtro de tools** por servidor: `enabled_tools` / `disabled_tools`
  (allow/denylist) ⇒ sirve para exponerle a Antigravity sólo un subconjunto y ahorrar tokens.

### 2.1 Lo que hay hoy en `mcp_config.json` (estructura, sin valores)

```json
{
  "mcpServers": {
    "megadaemon-minimax": {
      "command": "/data/user/0/com.agy/files/megadaemon/rust-core/target/release/megadaemon-core",
      "args": ["mcp-server"],
      "env": { "MEGADAEMON_MCP": "1" }
    }
  },
  "providers": { "minimax": { "api_key": "<valor; sha16 213c056052c2383d, 125 chars>" } }
}
```

* El servidor MCP declarado es **stdio** sobre **una copia de nuestro binario dentro del sandbox
  de Antigravity**.
* **Esa ruta NO existe** en el dispositivo (`rust-core/target/release/` sin binario: el clon git
  está sin compilar) ⇒ **el MCP no arranca**. El clon sí existe:
  `files/megadaemon/` (rama `main`).
* El binario **vivo** del daemon es otro: `sha256 d93fa2172a74ab179ea6db8116f54166…`
  (7.409.280 B) ⇒ *producto vs binario en ejecución*: exactamente la lección del nodo
  `cloud-sandbox`.

---

## 3. Estructura de credenciales (DoD W5.3) — **nunca los valores**

| Dónde | Qué guarda | Cómo lo tratamos |
|---|---|---|
| `~/.gemini/config/config.json` → `providers.<id>.api_key` | Clave del proveedor (p. ej. `minimax`) | Sólo **nombre del campo** + **sha16** del valor + longitud |
| `~/.gemini/config/mcp_config.json` → `mcpServers.<n>.env` / `.headers` | Credenciales del MCP | Sólo **nombres** de variables/cabeceras |
| `~/.gemini/jetski-standalone-oauth-token` | OAuth del runtime "Jetski" | Sólo la **ruta** (nunca abierto) |
| `~/.gemini/antigravity-app/{jetski_state,antigravity_state}.pbtxt` | Estado protobuf del runtime | Sólo **rutas** |

Lo que se entrega: `sha16 = 213c056052c2383d` (125 chars) para `providers.minimax.api_key`.
**El valor no aparece en este documento, ni en el repo, ni en el chat, ni en el estado compartido.**

---

## 4. El puente documentado (DoD W5.4)

### 4.1 La receta (Streamable HTTP contra el daemon **vivo**)

```json
{
  "mcpServers": {
    "megadaemon": {
      "url": "http://127.0.0.1:9090/mcp",
      "headers": { "X-API-Key": "<API key de la instancia>" }
    }
  }
}
```

* Por **LAN**: `http://192.168.1.124:9090/mcp` (el daemon ya sirve 3000/3030/8090/9090 dual-stack).
* Por **túnel**: `https://<url-efímera>/mcp` + la misma cabecera (la URL se resuelve en runtime
  desde `tunnel.url`; **nunca hardcodeada**).
* Filtro opcional de tools: `enabled_tools: ["minimax_chat","swarm_chat","dex_decompile"]`.

### 4.2 Por qué HTTP > stdio

| | stdio (lo que hay hoy) | Streamable HTTP (propuesta) |
|---|---|---|
| Binario | copia en el sandbox (inexistente ⇒ roto) | ninguno: el daemon vivo |
| Tools | las de la copia (desactualizadas) | **las 45, siempre al día** |
| Estado | instancia aparte | **el mismo `board.sqlite`** que la flota |
| Mantenimiento | recompilar dentro del sandbox | ninguno |

### 4.3 Estado del fallback CLI

La skill `minimax-megadaemon` propone como fallback
`…/rust-core/target/release/megadaemon-core chat minimax-m2.7` — **también roto** (mismo binario
inexistente). La ruta que **sí** funciona hoy es la web: cockpit en `http://127.0.0.1:3000`
(la página incrusta el token de sesión ⇒ lecturas y escrituras nativas).

---

## 5. Riesgos y hallazgos

1. **MCP configurado pero roto** (`megadaemon-minimax` → binario inexistente). Fix: §4.1.
2. **Hook cada 5 min** (`Snatcher-MegaDaemon-SuperLoop`) ejecutando `snatcher_megadaemon.py --once`
   ⇒ CPU/batería del móvil en background. Revisar con `agy-daemon task list`.
3. **`bootstrap_core.sh`** parece reinstalar el binario dentro del sandbox: puede volver a
   sobrescribir la config del MCP ⇒ **cualquier cambio en `mcp_config.json` debe ser idempotente
   y verificable** (o aplicar el cambio *después* del bootstrap).
4. **El binario nativo es Go de Google** ⇒ RE profunda por delante (símbolos, protobuf, OAuth).
   Sólo se ha hecho un barrido de cadenas; el trabajo de verdad (mapear el protocolo MCP interno,
   los proveedores y el OAuth) sigue pendiente y es RE de software de terceros: **sólo con licencia
   o permiso**.
5. **Credential storage doble**: `config.json` (proveedores) + `jetski-standalone-oauth-token`
   (OAuth del runtime) ⇒ hay que rotar **ambos** si se filtra uno.

---

## 6. Reproducibilidad (comandos exactos)

```bash
# APK y sha256 del cliente que corre
pm path com.agy
sha256sum /data/app/~~j0Ed8z8Ure_gK6t8fDORYg==/com.agy-g4RJUI1keObbU5LgIHA4pg==/base.apk

# estructura del estado (sólo nombres)
ls -la /data/user/0/com.agy/files/.gemini/config
ls -d  /data/user/0/com.agy/files/.gemini/antigravity-app/mcp/*/

# definición de los MCP + estructura de credenciales (VALORES: NUNCA)
base64 -w0 /data/user/0/com.agy/files/.gemini/config/mcp_config.json
base64 -w0 /data/user/0/com.agy/files/.gemini/config/config.json

# hooks
base64 -w0 /data/user/0/com.agy/files/.gemini/antigravity-app/daemon/tasks.json
```

Los ficheros con posibles credenciales se han copiado **fuera del repo** a
`md-secrets/agy-intel/` (0600) para analizarlos con redacción; al repo sólo llega este
documento y el módulo Rust, que **no sabe devolver valores de credenciales** (test incluido).

---

## 7. Módulo Rust entregado

* `rust-core/src/antigravity_intel.rs` — módulo del workstream (agente de apps/ocultación,
  `275a2232`): parsea los ficheros reales de configuración de Antigravity
  (`mcp_config.json`, `custom_models.json`, `config.json`, `daemon/tasks.json`), **detecta los
  servidores MCP rotos** (los que apuntan a un binario inexistente), **mapea los
  modelos/proveedores** de Antigravity a los proveedores que MegaDaemon ya tiene
  (`minimax` → `minimax-m3.1-flash`), **genera la entrada `mcpServers`** que convierte a
  Antigravity en cliente del `/mcp` de este móvil (y la equivalente con transporte HTTP), y
  **proyecta las tareas** del daemon de Antigravity al formato del tablón.
* `rust-core/tests/antigravity_intel_smoke.rs` — smoke test: monta un árbol `.gemini` sintético y
  comprueba, entre otras cosas, que **ningún valor de credencial sale del módulo** (sólo nombres
  de campo y sha16).
* Wiring en `lib.rs` + tool MCP `antigravity_intel` para que W6 (`service_bus`) pueda registrar a
  Antigravity como cliente/servicio más.

> Nota de reparto: este documento y **las medidas del dispositivo** (secciones 1–6) son del agente
> de DEX/inventario (`659db26c`); el módulo Rust y su test son del agente de apps/ocultación
> (`275a2232`). El mismo fichero `src/antigravity_intel.rs` fue escrito por los dos en paralelo y
> **ganó la versión del agente de apps/ocultación** (más completa: añade el mapeo de proveedores y
> la proyección al tablón). Para evitar pisarnos, el inventario de DEX/inventario se conserva como
> **medición + este documento + el hallazgo emitido por los 4 canales**, no como módulo aparte.

---

## 8. Apéndice — barrido de cadenas del binario nativo (medido en el móvil)

`assets/runtime/bin/liblanguage_server.so` (181.358.800 B; 52.898.787 B comprimido en el APK) es un
binario **Go de Google** (símbolos `google3/base/go`, `protobuf/v2`, `cockroachdb/errors`).
Barrido de cadenas por streaming (sin extraerlo a disco: el móvil está al 98 % de disco):

| marcador | ocurrencias | lectura |
|---|---:|---|
| `mcp` | 6.182 | **implementa MCP por sí mismo** |
| `jsonrpc` | 300 | transporte JSON-RPC 2.0 |
| `oauth` | 1.279 | flujo OAuth propio |
| `api_key` / `apikey` | 162 / 444 | cabeceras de credencial |
| `/v1/` | 15.268 | endpoints estilo OpenAI (`/v1/chat/completions`, `/v1/messages`) |
| `openai` | 13.133 | cliente OpenAI-compatible |
| `gemini` | 3.009 | cliente Gemini |
| `anthropic` | 119 | cliente Anthropic |

**Prueba definitiva (segundo barrido, focalizado)**: el binario embuelve el **SDK oficial de MCP
en Go de Google**:

```
google3/third_party/golang/github_com/modelcontextprotocol/go_sdk/v/v0/mcp/mcp
  · CallToolParams, CallToolParamsRaw, CompleteParams, ElicitParams, CancelledParams
  · CreateMessageParams, CreateMessageWithToolsParams, ListPromptsParams, GetPromptParams
  · ClientCapabilities, Implementation{clientInfo, protocolVersion}, PingParams
  · methodInfo, isResult(), isNil()  ·  json:"name" · json:"arguments" · json:"_meta"
```

⇒ el language server de Antigravity es **un cliente MCP completo** (no una imitación): llama
`tools/call`, `initialize`, `complete`, `elicitation`, etc. con el SDK oficial.

Complemento con la evidencia del SDK (plugin `google-antigravity-sdk`, instalado en el propio
dispositivo): `skills/google-antigravity-sdk/examples/getting_started/mcp_tools.md` documenta
`types.McpStdioServer(name, command, args)` **y** `types.McpStreamableHttpServer(name, url,
headers)`, más el filtrado `enabled_tools` / `disabled_tools` por servidor.

Nota operativa: el barrido se hizo **en el móvil** y en streaming; las primeras versiones se
lanzaron por duplicado (3 procesos a la vez) y se.cancelaron dos de ellos para no machacar la RAM
del dispositivo.

---

## 9. El puente MCP: APLICADO y VERIFICADO en el dispositivo (2026-11-20)

**Fichero real** (medido): `/data/user/0/com.agy/files/.gemini/config/mcp_config.json` (432 B),
claves raíz `mcpServers` **y** `providers` (con `providers.minimax`, el BYOK del usuario).

**Antes (ROTO)** — `command` apuntaba a una copia de nuestro binario que **no existe** en el
dispositivo ⇒ el servidor no arrancaba y sólo quedaban cacheados los schemas de 7 tools:

```json
"megadaemon-minimax": { "command": "/data/user/0/com.agy/files/megadaemon/rust-core/target/release/megadaemon-core",
                        "args": ["mcp-server"], "env": { "MEGADAEMON_MCP": "…" } }
```

**Ahora (operativo)** — se conserva el **nombre** y se cambia el **transporte**:

```json
"megadaemon-minimax": { "url": "http://127.0.0.1:9090/mcp",
                        "headers": { "X-API-Key": "<clave de la instancia>",
                                     "X-Seere-Auth-Token": "<token de sesión>" } }
```

### Tres reglas que NO son opcionales (medidas, no supuestas)

1. **El nombre del servidor se conserva: `megadaemon-minimax`.** La skill `minimax-megadaemon` y
   el `config.json` de Antigravity referencian el servidor **por nombre**
   (`mcp(megadaemon-minimax/minimax_chat)`, `mcp(megadaemon-minimax/megadaemon_npu_cv)`).
   Añadir un servidor nuevo con otro nombre deja esas allow-lists apuntando a la nada.
2. **El `/mcp` exige DOS cabeceras**, incluso desde loopback:
   `sin clave → 403`, `X-API-Key → 403 {"error":"unauthorized","hint":"falta la cabecera
   X-Seere-Auth-Token"}`, `X-API-Key + X-Seere-Auth-Token → 200` con **45 tools**.
3. **El escritor debe FUSIONAR**, no sobrescribir: el fichero tiene `providers` al mismo nivel ⇒
   sobrescribirlo deja a Antigravity sin modelo BYOK.

### El token no sale del dispositivo

Se obtiene **en el móvil** con `POST http://127.0.0.1:9090/api/session/exchange` y la API key **en
el cuerpo** → `{"token": <64 chars>, "ok": true}`. Así el secreto no viaja por chat, Drive ni
catálogo (sólo su `sha16`).

### Aplicación: idempotente, con backup y escritura atómica

`/data/local/tmp/megadaemon/apply_bridge.py` (3 KB): exchange → merge del JSON → verificación
funcional con las cabeceras exactas del config → backup `mcp_config.json.bak-<ts>` → escritura a
`.tmp` + `mv` atómico. Reejecutable; imprime **sólo** `sha16` y longitudes.

Verificación: `tools/list = 45` con las cabeceras del config · `providers.minimal` intacto ·
backup `mcp_config.json.bak-20261120-021556`.

**`bootstrap_core.sh` NO toca `mcp_config.json`** (63 líneas, leído en el dispositivo: no menciona
ni `mcp_config` ni `mcpServers`) ⇒ la config es estable frente al bootstrap.

**Pendiente**: reiniciar el language server de Antigravity para que relea la config (implica
tocar la app en uso ⇒ no se hace de forma automática).

---

## 7. Sección del carril de apps/ocultación (`275a2232`) — medidas del cliente vivo y puente

> Complemento medido **en el móvil del usuario** (no del paquete de distribución).
> Todo lo que sigue es reproducible con `root_exec` + `curl` contra `127.0.0.1:9090`.

### 7.1 La pregunta clave: ¿Antigravity habla MCP? — **SÍ, y por qué lo sabemos**

| Evidencia | Dónde | Qué demuestra |
|---|---|---|
| `mcpServers` con `command`/`args`/`env` y con `url`/`headers` | `~/.gemini/config/mcp_config.json` | El cliente acepta los **dos** transportes |
| `types.McpStdioServer(name, command, args)` y `types.McpStreamableHttpServer(name, url, headers)` | plugin `google-antigravity-sdk`, `skills/.../mcp_tools.md` | El **SDK oficial** documenta stdio **y** Streamable HTTP con cabeceras |
| `google3/third_party/golang/github.com/modelcontextprotocol/go_sdk/v/v0/mcp/mcp` (`CallToolParams`, `ClientCapabilities`, `Implementation{clientInfo, protocolVersion}`) | cadenas de `liblanguage_server.so` | El binario embuelve el **SDK oficial de MCP en Go**: es un cliente MCP de pleno derecho |
| `~/.gemini/antigravity-app/mcp/<server_id>/*.json` | caché del cliente | El cliente guarda los **schemas de las tools** por servidor ⇒ ya negoció `tools/list` con él |

Servidores MCP que el cliente tiene hoy:

| `server_id` | Origen | Tools en caché |
|---|---|---|
| `megadaemon-minimax` | el nuestro (ver §7.3) | 7 (esquemas viejos, del intento por stdio) |
| `data-agent-kit-plugin_dak-notebook` | plugin del propio Antigravity | 9 |
| `gemini-api_gemini-api-docs` | plugin del propio Antigravity | 2 (+ `instructions.md`) |
| `data-agent-kit-plugin_dak-viz` | plugin del propio Antigravity | — |

### 7.2 El puente, con la **corrección** de la receta

Medido contra el daemon vivo:

| Petición a `POST http://127.0.0.1:9090/mcp` | Resultado |
|---|---|
| sin cabeceras | **403** |
| sólo `X-API-Key` | **403** |
| sólo `X-Seere-Auth-Token` | **200** |
| las dos (como está en la config de Antigravity) | **200** |

⇒ **La API key NO abre `/mcp`**; lo abre el **token de sesión**. La receta que circulated con
`X-API-Key` sola daría 403. Además la respuesta es **JSON plano**
(`Content-Type: application/json`) y **sin** cabecera `Mcp-Session-Id` ⇒ el servidor MCP es
**sin estado** y no hay que mantener sesión en el puente.

La entrada correcta en `~/.gemini/config/mcp_config.json` (ya aplicada en el móvil):

```json
{ "mcpServers": { "megadaemon-minimax": {
    "url": "http://127.0.0.1:9090/mcp",
    "headers": { "X-Seere-Auth-Token": "<token de sesión, 0600>" } } } }
```

- **LAN**: `http://192.168.1.124:9090/mcp` (mismo daemon, otra interfaz).
- **Túnel**: `https://<tunnel-url>/mcp` (la URL efímera vive en `/data/local/tmp/megadaemon/tunnel.url`).
- El `providers.minimax.api_key` (BYOK del cliente) **se conserva**: el modelo del IDE y el MCP
  son cosas distintas.

### 7.3 El token rota ⇒ el puente se rompía solo (y su arreglo)

El token de sesión se materializa **al arrancar el daemon** ⇒ en cada reinicio el valor escrito en
la config de Antigravity quedaba caduco y el MCP moría **en silencio** (el cliente reintenta y no
dice nada). Arreglo en el supervisor (`deploy/99-megadaemon.sh`, función
`sync_antigravity_bridge`): tras cada arranque verificado del daemon se re-sincroniza **sólo** la
cabecera del servidor HTTP local del `mcp_config.json` (merge idempotente, conserva `providers` y
el resto), y el fichero queda `0600` del usuario de la app. **El secreto nunca sale del móvil**:
ni al repo, ni al chat, ni al tablón.

### 7.4 Inventario del cliente vivo (medido)

- **Paquete**: `com.agy` ("AGY v2"); APK en `/data/app/~~j0Ed8z8Ure_gK6t8fDORYg==/com.agy-*/base.apk`.
- **Sandbox POSIX**: `/data/user/0/com.agy/files/` (37 entradas: `usr/bin` con toolchain completa —
  `bash`, `python3`, `7z`, `aapt`, `clang`, `rish` (Shizuku, uid 2000) —, `usr/lib/python3.*/site-packages`,
  `megadaemon/` con un **clon git** del repo, `dumper-web/antigravity-android-repo/`, `agy_standalone/`).
- **Procesos**: la app `com.agy` (pid 2679) + el **language server** (Go) como hijo:
  `libloader.so … liblanguage_server.so --standalone --override_ide_name antigravity --subclient_type hub
  --override_ide_version 2.11.0 --https_server_port 45157 --csrf_token <uuid> --app_data_dir antigravity-app
  --web_bundle_path …/web --enable_sidecars`.
- **Storages**: `.gemini/antigravity-app/{brain,conversations,plugin_data,sidecar_data,mcp,builtin,bin,
  annotations,presence,crashes}` + `*.pbtxt` (`antigravity_state.pbtxt`, `jetski_state.pbtxt`) +
  `conversation_summaries.db` + `agyhub_summaries_proto.pb`.
- **Hooks del daemon**: `Snatcher-MegaDaemon-SuperLoop` (interval 300 s) y `minimax_bootstrap`
  (cron, deshabilitado) en `~/.gemini/antigravity-app/daemon/tasks.json`; el daemon acepta
  `interval | cron | github | file | battery | webhook` (webhook autenticado con `X-AGY-Token`
  en `127.0.0.1`, cuerpo ≤ 64 KB).
- **Permisos**: `config.json → userSettings.globalPermissionGrants.allow` (concesiones globales),
  `autoExecutionPolicy`, `browserJsExecutionPolicy`, `enableTerminalSandbox: false`.
- **Credenciales (sólo estructura)**: `providers.<id>.api_key` en `mcp_config.json`/`config.json`,
  `mcpServers.<n>.env`/`.headers`, `~/.gemini/jetski-standalone-oauth-token`,
  `antigravity/custom_models.json → models[].apiKey`.
- ⚠️ **Hallazgo de seguridad**: el **bearer OAuth de Google aparece en claro** en
  `agy_standalone/probe.log` (registro del proxy de pruebas). No se reproduce aquí; conviene
  rotarlo y dejar ese log en `0600` (o borrarlo).

### 7.5 Proveedores: mapeo, **sin duplicar** (`providers.rs` ya tiene 18 entradas)

| Antigravity (provider/model) | Proveedor de MegaDaemon | Modelo |
|---|---|---|
| `minimax` / `MiniMax-M3.1-Flash` | `minimax-m3.1-flash` | MiniMax-M3.1-Flash |
| `minimax` / `MiniMax-M3` | `minimax-m3` | MiniMax-M3 |
| `gemini` | `google-ai-studio` | gemini-3.8-flash |
| `glm` | `zai-glm-free` | glm-4.7-flash |
| `openrouter` / `groq` / `pollinations` | `openrouter-free` / `groq-free` / `pollinations-free` | — |
| `anthropic` / `deepseek` / `cerebras` / `mistral` | `anthropic` / `deepseek` / `cerebras` / `mistral-free` | — |
| `seere` / local | `local-seere` | local |

El cliente declara hoy un único modelo BYOK (`minimax` / `MiniMax-M3.1-Flash`) ⇒ **coincide 1:1**
con `minimax-m3.1-flash`. La tabla completa vive en `antigravity_intel::provider_map_table()`.

### 7.6 Del cliente al tablón y al bus de servicios

- `antigravity_intel::tasks_to_board_tasks(&hooks)` proyecta las tareas propias de Antigravity al
  formato de `state/mesh_board.json` (`task_id: "antigravity-<id>"`, `owner: "antigravity"`,
  `payload.trigger`, `interval_sec`, `last_run_status`…), **sin credenciales**.
- `antigravity_intel::service_bus_entry(...)` deja la entrada lista para W6
  (`id: "antigravity"`, `kind: "agent_client"`, `transport: "http"`, `auth_kind: "session_token_file"`,
  `health` = **el valor medido**, nunca inventado).

### 7.7 Cómo verificar el puente de punta a punta en 10 segundos

1. Abrir Antigravity y pedirle al agente cualquier tool del servidor `megadaemon-minimax`
   (p. ej. `system_overview`).
2. En el móvil, el daemon deja traza de la llamada del cliente:
   `grep '"/mcp"' /sdcard/megadaemon.log | tail` → debe aparecer una entrada con **UA del cliente**
   (no `curl/7.88.1`) y `reason` vacío ⇒ el cliente se conectó de verdad.
3. La lista de tools del daemon son **45**; el IDE debería/cachear los schemas en
   `~/.gemini/antigravity-app/mcp/megadaemon-minimax/`.

### 7.8 Módulo y tests

- `rust-core/src/antigravity_intel.rs` — escaneo con tipos, parseo de `mcp_config.json` /
  `config.json` / `custom_models.json` / `tasks.json`, **estructura de credenciales sin valores**
  (nombre de campo + longitud + sha16), detección de servidores MCP **rotos**, mapeo de
  proveedores, generación del `mcpServers` HTTP y del `merge_server_entry`, proyección al tablón y
  entrada de `service_bus`.
- `rust-core/tests/antigravity_intel_smoke.rs` — `#[path]` (no toca `lib.rs`): monta un árbol
  `.gemini` sintético y comprueba que **ningún valor de credencial sale del módulo**.
- `cargo test --test antigravity_intel_smoke` → **14 passed / 0 failed** (4 del arnés + 10 internos).
