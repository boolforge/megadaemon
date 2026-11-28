# Patrones de una CLI de agente en Rust (Devin Desktop 3.15.14) → refactor de MegaDaemon

> Origen: el paquete de Devin Desktop trae su **propia CLI en Rust** (20 crates
> internos, ~297 ficheros). Es material **propietario**: **no se copia código**,
> se copia la **arquitectura**. Todo lo de aquí es análisis con rutas; lo
> medido está medido, lo demás va marcado **NO MEDIDO**.
> Worker: sesión de RE de Devin (commit `1f3c788`), documentos
> `docs/RE_DEVIN_CLI_RUST.md` y `docs/RE_DEVIN_DESKTOP_CONTRATOS.md`.

## 0. Los crates (medido)

| crate | ficheros | qué es |
|---|---:|---|
| `affogato` | 21 | **el agente**: bucle, inferencia, tareas, herramientas |
| `chisel-agent` | 146 | ACP: sesiones, permisos, revert, plugins |
| `chisel-core` / `chisel-api` / `chisel-commands` | 15 / 14 / 18 | núcleo, cliente de API, comandos |
| `chisel-ui` | 18 | TUI |
| `chisel-cloud-bridge` | 17 | puente con la nube |
| `chisel-acp-*` | 9 | ACP de bajo nivel |
| `chisel-otel`, `chisel-mcp`, `inference`, `fx`, `plugin-host` | 7 / 2 / 5 / 3 / 8 | métricas, MCP, proveedores, efectos, plugins |

Además **315 crates externos** y tres que son **una GUI en Rust**
(`dioxus-core` + `blitz-dom` + `stylo` + `taffy` ⇒ renderizan HTML/CSS con un
motor de navegador **en Rust** dentro de la terminal).

⇒ **Ojo**: `dioxus` + `blitz` + `stylo` + `taffy` es la vía para que nuestro
`webui.rs` (5.014 líneas de HTML servido) pueda tener **una GUI nativa de
escritorio** sin Electron ni navegador. Es el mismo camino que llevó a
`vercel-labs/agent-desktop` (medido en el benchmark).

## 1. Los diez patrones y a dónde van en MegaDaemon

| # | patrón (medido) | nuestro módulo | qué hay que escribir | coste |
|---|---|---|---|---|
| 1 | **el bucle como manejador de efectos**: `affogato/agent/effects/{handler,inference,tasks,tools}.rs` | **nuevo `efectos.rs`** + `agent_loop.rs` | **hecho**: reductor puro `(estado, evento) → (estado, efectos)`, tres manejadores, bucle iterativo y tres modos (interactivo, programático, subagente); `agent_loop.rs` cableado (14 tests) | **hecho** |
| 2 | **`cache_keepalive*` (5 ficheros)**: política de keepalive del **caché de prompt del proveedor** | **nuevo `cache_keepalive.rs`** | ya hecho: prefijo cacheable + firma + decisión (reutilizar/refrescar/invalidado) | **hecho** |
| 3 | **la conversación es un BOSQUE**: `message_nodes.parent_node_id`; el revert = **fork** del bosque | **nuevo `message_tree.rs`** | **hecho**: bosque con `parent_node_id`, `fork` que copia el historial, `revert` que no borra y **revert por pasos** (lista + ejecución del paso elegido) | **hecho** |
| 4 | **la compactación no borra**: `messages.compaction_epoch` + `sessions.current_epoch` | **nuevo `compaction_epoch.rs`** | ya hecho: epochs en vez de truncar; el historial se conserva | **hecho** |
| 5 | **`inference/`**: **17 errores de proveedor** con `retry_after` estructurado, `compaction_thresholds` **por modelo** y `tool_formatter_type` por modelo | `providers.rs` / `llm_gateway.rs` / **nuevo `model_catalog.rs`** | ya hecho: errores tipados + `retry_after` estructurado **y usado** (la espera es inyectable ⇒ se puede comprobar) + **catálogo por modelo** (umbral de compactación y formateador de tools) + **el dialecto que dice el catálogo es el que se habla** (Anthropic) | **hecho** |
| 6 | **`fx/context.rs` + `fx/driver.rs`**: separar **construcción de contexto** y **conducción del agente** | **nuevo `contexto.rs`** | **hecho**: `construir_contexto` puro (bloques + metadatos + recorte registrado), 15 tests; **cableado** en `efectos::construir_prompt` / `AgentLoop::build_prompt` (el prompt lo arma el módulo de contexto) | **hecho** |
| 7 | **daemon**: socket Unix + lock + **handshake que verifica que quien conecta es su daemon** + RPC de cola (`list claimed sessions`, `list queued sessions`, `list outposts`) | **nuevo `daemon_socket.rs`** | **hecho**: lock de instancia, socket Unix NDJSON, handshake fail-closed y las tres RPC sobre la cola real (20 tests) | **hecho** |
| 8 | **sandbox medido**: *"macOS seatbelt / Linux bwrap+seccomp"*; en Linux exige `bwrap` **y** `socat`; el permiso lógico **se compila a regla de sandbox** | `tool_policy.rs` + **nuevo `sandbox_linux.rs`** | ya hecho: `rust-core/src/sandbox_linux.rs` (bwrap + seatbelt, deny>allow, 10 tests) | **hecho** |
| 9 | **plugins**: `PLUGIN_DATA` persistente **por identidad, no por versión**; globs prohibidos **sin `..`, `#` ni `@`** | **nuevo `plugin_data.rs`** | ya hecho: persistencia por identidad + validación de globs (anti path-traversal) | **hecho** |
| 10 | **esquema SQLite de sesiones (10 tablas)** medido literal | **nuevo `board_schema.rs`** | **hecho**: DDL medido transcrito, migración idempotente e introspección para comparar tabla por tabla (13 tests); **cableado**: `board.rs` lo aplica y el store de sesiones ACP **usa** esas tablas (`sessions`, `messages`, `app_state`) | **hecho** |

### 2.1 Los tres que se concretan primero

* **(2) keepalive del caché de prompt** — dinero. Un `cache_keepalive` que
  re-envía la prefija cacheada al proveedor antes de que expire.
* **(4) compactación por epochs** — el historial no se pierde; se puede
  "descompactar" y auditar.
* **(9) `PLUGIN_DATA` por identidad** — sobrevive a las actualizaciones y se
  borra al desinstalar; con la validación de globs (sin `..`, `#`, `@`).

## 3. Lo que YA está hecho en el repo por este camino

| entregable | commit | qué |
|---|---|---|
| `rust-core/src/sandbox_linux.rs` | `2b393f5` (patrón 8) | **aislamiento real**: `bwrap` (Linux) y `sandbox-exec` (macOS), montajes ro/rw/tmpfs, `--unshare-net`, `--cap-drop ALL`, `--seccomp`, política **deny > allow**, `which()` propio, y **fallar cerrado** si no hay aislamiento |
| `rust-core/src/re_decompilers.rs` | `bfb6b80` | registro de motores de decompilación con contrato común |
| `rust-core/src/cache_keepalive.rs` | este commit (patrón 2) | **keepalive del caché de prompt**: el **prefijo cacheable** es la racha inicial de entradas *estables* (una herramienta o un contenido dinámico lo cortan); su **firma** (hash del prefijo) decide si el caché sigue vivo: `Reutilizar` (el prefijo es idéntico y no ha caducado) / `Refrescar` (idéntico pero pasado el TTL) / `Invalidado` (cambió un byte, o no había). Ahorra dinero: un prefijo estable se cobra a precio de caché |
| `rust-core/src/compaction_epoch.rs` | este commit (patrón 4) | **compactación por época, sin borrar**: `compactar()` sella la época actual y abre una nueva con un resumen; **ningún mensaje se borra** (los anteriores siguen consultables con su `epoch`), el `seq` es global y `revertir_a(epoch)` vuelve a una época anterior sin tocar el historial. Incluye el **DDL medido** en la referencia (`messages.compaction_epoch`, `sessions.current_epoch`, `UNIQUE(session_id, compaction_epoch, sequence_number)`) |
| `rust-core/src/plugin_data.rs` | este commit (patrón 9) | **`PLUGIN_DATA` por IDENTIDAD**: los datos viven en `<estado>/plugins/<identidad>` — la ruta **no lleva la versión**, así que sobreviven a las actualizaciones del plugin y se borran al desinstalarlo. Validación anti path-traversal: identidad ASCII en minúsculas (sin `/`, `\`, `..`, ni empezar por `.`), nombre de fichero sin separadores, y globs **sin `..`, `#` ni `@`** y nunca absolutos |

### 3.1 Cómo se expone (patrón 9)

La regla del repo es que **toda capacidad nueva vive en Rust dentro del daemon** y
se expone por **CLI** y por **tool MCP** (nada de Python):

* CLI: `megadaemon-core plugin-data <validar-glob|validar-identidad|listar|plugins|escribir|leer|purgar>`
  (`rust-core/src/main.rs`, subcomando `plugin-data`).
* Tool MCP: `plugin_data` (`rust-core/src/mcp_ext.rs`), con las mismas acciones.

**MEDIDO 2026-11-28**: el módulo se tipó y sus **10 tests pasaron** en un crate
aislado (sólo `serde_json`) con un stub de `cockpit_server::state_dir`; el
crate completo se valida en el CI. `cockpit_server::state_dir()` devuelve
`String` (no `PathBuf`) ⇒ hay que convertirla antes de encadenar el `join`.

El módulo de sandbox se apoya en el modelo de permisos que ya existe:
`tool_policy.rs` decide (allow/ask/deny) y `sandbox_linux.rs` **compila esa
decisión a una regla de aislamiento** — que es exactamente la idea del agente de
Devin: *el permiso lógico y el aislamiento son el mismo objeto*.

## 4. Reglas que se siguen (del propio repo)

* Nada de `cargo` en local (2 CPU, 3 GB sin swap): el CI compila y testea.
* Todo en castellano; lo no medido, **NO MEDIDO**.
* Copy sólo de MIT/Apache-2.0. Esto es **arquitectura**, no código.
