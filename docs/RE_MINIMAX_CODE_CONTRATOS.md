# RE — MiniMax Code 0.8.0 (`@minimax-ai/code`)

**Fecha de la medición:** 2026-11-27 · **Todo lo que hay aquí está medido**, con su
ruta y su hash. Lo que no se pudo medir va marcado **NO MEDIDO**.

## 1. Qué se descargó (con hashes)

| paquete | versión | bytes (tgz) | sha256 |
|---|---|---:|---|
| `@minimax-ai/code` | **0.8.0** (publicada el 2026-11-27T07:26Z) | 100.643.508 | `fbc48d5e138ce8f43f11627b7378a209772a522c1125da7b9dbf2157ae8e9365` |
| `@minimax-ai/code-acp` | **0.8.0** (publicada el 2026-11-27T07:38Z) | 33.977 | `cf86a861f20e26680d61db310323f25b8db9702cac17bef0317788f39d321523` |

74 versiones de `@minimax-ai/code` y 6 de `@minimax-ai/code-acp` en el registro.

## 2. Qué es: **JavaScript puro, no un binario nativo**

A diferencia de Claude Code (Bun) y Codex (Rust), MiniMax Code es un paquete JS:

| parte | medida |
|---|---|
| `chunks/` | **125 ficheros, 32 MB** (el grueso del agente) |
| `cli.js` | 4 KB (el arranque) |
| `browser-program-worker.js` | 56 KB |
| `quickjs-worker.js` | 28 KB (un intérprete **QuickJS** embebido) |
| `emscripten-module.wasm`, `photon_rs_bg.wasm` | 2 módulos **WASM** |
| `native/` | binarios sólo **darwin** y **win32** |
| `embedded/computer-use/0.34.1/` | **"MiniMax Computer Use"** para darwin y win32 |
| `assets/skills/` | **30 skills** |
| `assets/agents/` | agentes y prompts integrados |

⇒ Para el RE de MiniMax Code **no** aplica el motor sancionado de binarios: la
lógica está en los chunks JS. El motor sancionado (Ghidra) se aplica al
`embedded/computer-use` **nativo**, que es de darwin/win32 ⇒ **NO MEDIDO** en
linux.

## 3. El launcher ACP (`@minimax-ai/code-acp`): el dialecto propio

Es **código abierto y legible** (`acp.js`, sin dependencias). Esto es lo que el
registro ACP prueba como `minimax-code`.

### 3.1 Los 4 métodos de autenticación (`BOOTSTRAP_AUTH_METHODS`)

| type | id | nombre | detalle |
|---|---|---|---|
| `terminal` | `minimax-code-login` | Sign in to MiniMax Code (Mainland China) | `login --region cn` |
| `terminal` | `minimax-code-login-global` | Sign in to MiniMax Code (Global) | `login --region global` |
| `env_var` | `minimax-code-api-key` | Use a MiniMax API key (Mainland China) | `MCODE_PROVIDER_API_KEY` (secret) |
| `env_var` | `minimax-code-api-key-global` | Use a MiniMax API key (Global) | `MCODE_PROVIDER_API_KEY` (secret) |

Es el **mismo modelo `terminal`/`env_var`** que usa ACP v1, pero con **regiones**
(China/global) como dimensión de la autenticación. MegaDaemon no tiene el
concepto de región ⇒ es una extensión que OURDI.

### 3.2 Los 17 métodos de extensión + 3 notificaciones

`BASE_EXTENSION_METHODS` (13):

```
session/activate
mcode/session/activate
mcode/session/steer
mcode/session/queue/list
mcode/session/queue/enqueue
mcode/session/queue/update
mcode/session/queue/delete
mcode/session/queue/steer
mcode/session/delegation/get
mcode/session/delegation/stop
mcode/session/delegation/message
mcode/session/delegation/transcript
```

`GOAL_EXTENSION_METHODS` (4):

```
mcode/session/goal/get
mcode/session/goal/create
mcode/session/goal/patch
mcode/session/goal/clear
```

`BASE_EXTENSION_NOTIFICATIONS` (3):

```
mcode/session/current_session_update
mcode/session/queue_update
mcode/session/delegation_update
```

**Contraste medido con los demás dialectos**: Devin expone **37** capacidades
`cognition.ai/*`; MiniMax expone **17** con prefijo `mcode/session/`. Los tres
conceptos que MiniMax añade y que **ningún otro** tiene:

| concepto | métodos | qué es |
|---|---|---|
| **cola** | `queue/{list,enqueue,update,delete,steer}` | una cola de peticiones: el cliente **encola** trabajo y lo va soltando |
| **delegación** | `delegation/{get,stop,message,transcript}` | **delega** una sesión a otra y puede mandar mensajes y leer la transcripción |
| **objetivo** | `goal/{get,create,patch,clear}` | un **goal** explícito, parcheable y borrable |

Esto es material directo para el P2 de MegaDaemon (memoria + grafo): la
**delegación con transcripción** es el mismo concepto que nuestro *agent
registry*, y la **cola** es lo que nos falta para el tablón.

### 3.3 Other detalles medidos del launcher

* **Responde `initialize` en stdio mientras arranca** (`BOOTSTRAP_INIT_ID =
  '__mcode_acp_bootstrap_init__'`): si el `mcode` aún no está instalado, el
  launcher **responde él mismo** para que el cliente no pierda el timeout de
  `initialize` mientras descarga `@minimax-ai/code` del registro público.
  Truco que MegaDaemon debería copiar para el arranque en frío.
* **Caché con alcance de ABI**: `~/.minimax-code/acp-releases`, y la clave
  incluye `process.versions.modules` ⇒ una caché por versión de Node.
* **Locks**: `DEFAULT_LOCK_STALE_MS = 30 min`, `DEFAULT_LOCK_WAIT_TIMEOUT_MS =
  30 min`.
* **Escapado de `cmd.exe`**: implementa el escape de *cross-spawn* (`^` para
  metacaracteres, `windowsVerbatimArguments`).

## 4. Los 5 agentes integrados (`assets/agents/builtin-agents.json`)

```
mavis  ·  bunny  ·  explore  ·  worker  ·  verifier
```

* `mavis` es el agente por defecto (de ahí los identificadores `__mavis_*` del
  bundle: `__mavis_guardian_ready`, `__mavis_sensitive_root_probe__`,
  `__mavis_tuiPackageRoot`…). **No es código nuestro**: es el nombre del agente
  de MiniMax.
* `explore` y `worker` ⇒ patrón **multi-agente** (como Codex v2 y nuestro
  *agent registry*).
* `verifier` ⇒ un agente **verificador** separado: valida el trabajo de otro.
  MegaDaemon tiene el equivalente (`verifier`/`critic` en el tablero) ⇒
  convergencia medida.
* `bunny` tiene `PERSONA.md`: hay una noción de **persona** configurable.

## 5. La política de exclusión de rutas y secretos (medida)

En `chunks/chunk-JNRKF55Q.js`, dos listas que se combinan en `dct` y se pasan
como `--iglob=`:

**Directorios de control de versiones** (`nJr`):

```
!**/.git/**   !**/.svn/**   !**/.hg/**   !**/.bzr/**   !**/.jj/**   !**/.sl/**
```

(`jj` = Jujutsu, `sl` = Sapling: MiniMax conoce los VCS modernos.)

**Ficheros de credenciales** (`rJr`):

```
!.env   !.env.*   !*.key   !*.pem   !*.p12   !*.pfx
!**/.ssh/**   !**/.aws/**   !**/.kube/**
!.npmrc   !id_rsa   !id_dsa   !id_ecdsa   !id_ed25519
```

Y la función `sJr()` normaliza la ruta (`\`→`/`, quita la `/` final, divide en
segmentos) y compara contra esas reglas con el marcador
`__mavis_sensitive_root_probe__`.

**Convergencia medida con MegaDaemon**: `crates/redact` + `tool_policy.rs`
trabajan con el mismo problema por el otro lado (redactar en vez de excluir).
Los 14 patrones de arriba son, literalmente, **la lista que el motor de
MegaDaemon debería reconocer** como "no tocar" — se pueden portar tal cual.

## 6. Las 30 skills que trae (medidas)

`agent-manage` · `app-manage` · `bunny-task` · `code-review` · `cron-manage` ·
`deep-research` · `deploy-website` · `edit-deployed-website` · `im-routing` ·
`init` · `lark-tools` · `llm-call` · `mcode-tools-master` · `mcp-manage` ·
`memory-manage` · `minimax-code-product` · `panel-manage` · `resume-codex` ·
`session-dispatch` · `session-manage` (+ 10 más)

Highlights: **`cron-manage`** (el jefe pidió automatizaciones), **`deep-research`**,
**`code-review`**, **`memory-manage`**, **`mcp-manage`**, **`session-dispatch`**,
**`resume-codex`** (integrarse con Codex) y **`lark-tools`** (Lark/Feishu:
correo, IM, documentos).

⇒ **MiniMax Code trae un sistema de skills con `SKILL.md`**, el mismo formato que
usa el sistema de skills de este entorno. Es el mismo concepto que
`rust-core/src/skills.rs` de MegaDaemon.

## 7. Variables de entorno (medidas)

`MCODE_PROVIDER_API_KEY` (la credencial) · `MCODE_REGION` · `MCODE_CONFIG_DIR` ·
`MCODE_INSTALL_ROOT` · `MCODE_AUTH_PROVIDER` · `MCODE_AUTH_BROKER_ENDPOINT` ·
`MCODE_OAUTH_TOKEN_ENDPOINT` · `MCODE_OAUTH_REVOCATION_ENDPOINT` ·
`MCODE_OAUTH_DEVICE_AUTHORIZATION_ENDPOINT` · `MCODE_EXTRA_HEADERS` ·
`MCODE_PRIVATE_SID` · `MCODE_PRIVATE_PATH` · `MCODE_PRIVATE_KIND` ·
`MCODE_TUI_RESULT_PATH` · `MCODE_TUI_DEFAULT_AGENT` · `MCODE_EDITOR` ·
`MCODE_CHROME_PATH` · `MINIMAX_DATA_DIR` · `MINIMAX_PLUGIN_ROOT` ·
`MCODE_ACP_PACKAGE_NAME` · `MCODE_ACP_TARGET_VERSION` · `MCODE_ACP_EXACT_VERSION` ·
`MCODE_ACP_NODE_ABI` · `MCODE_ACP_CACHE_DIR`

(`MCODE_PRIVATE_SID`/`PATH`/`KIND` aparecen junto a los endpoints OAuth ⇒
probablemente identificadores de sesión privada; **NO MEDIDO** su semántica.)

## 8. Computer Use embebido (relevante para el P4)

`embedded/computer-use/0.34.1/<plataforma>/` trae la app **"MiniMax Computer
Use"** (`minimax-computer-use`), `cua_driver_sdk.dll` (Windows) y un tema de
cursor. Es decir: **MiniMax Code ya tiene un agente de computer-use embebido**
(0.34.1) con driver nativo por plataforma. Para el P4 de MegaDaemon (GUI /
computer use) esto es la referencia industrial más cercana que hay medida.
Plataformas disponibles: **darwin-arm64, darwin-x64, win32-arm64, win32-x64**
(sin linux) ⇒ el binario native **NO MEDIDO** en este sandbox.

## 9. Lo que NO se midió

* Los **32 MB de chunks JS** no se han analizado en detalle (sólo los ficheros
  citados: `chunk-JNRKF55Q.js` y el `acp.js`). El resto de la lógica del
  agente queda **NO MEDIDO**.
* Los módulos **WASM** (`emscripten-module.wasm`, `photon_rs_bg.wasm`) no se
  han descompilado.
* El **computer-use nativo** (darwin/win32) no se ha reversingueado: no hay
  binario de linux y aquí no aplica el motor sancionado.
* El comportamiento en ejecución (no sólo estático) de la cola y la delegación.
