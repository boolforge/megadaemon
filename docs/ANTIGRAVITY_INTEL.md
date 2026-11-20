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

Complemento con la evidencia del SDK (plugin `google-antigravity-sdk`, instalado en el propio
dispositivo): `skills/google-antigravity-sdk/examples/getting_started/mcp_tools.md` documenta
`types.McpStdioServer(name, command, args)` **y** `types.McpStreamableHttpServer(name, url,
headers)`, más el filtrado `enabled_tools` / `disabled_tools` por servidor.

Nota operativa: el barrido se hizo **en el móvil** y en streaming; las primeras versiones se
lanzaron por duplicado (3 procesos a la vez) y se.cancelaron dos de ellos para no machacar la RAM
del dispositivo.
