# Backlog: el agente (yo) DENTRO de MegaDaemon — API + interfaz (2026-11-23)

> Lo pidió el jefe el 2026-11-23, literalmente:
>
> 1. «no usas el sistema de MegaDaemon» ⇒ el trabajo del agente debe pasar por
>    el daemon, no por atajos sueltos.
> 2. «no te integras conscientemente dentro de mi software para poder usarte
>    con la API a través de MegaDaemon» ⇒ el daemon tiene que exponer una API
>    con la que se pueda hablar con el agente.
> 3. «conectado a un GUI o interfaz web (que es fatal, ya podrías fusionarte con
>    una copia del GUI de Antigravity)» ⇒ falta la INTERFAZ y falta el cliente.
> 4. «una vez termines todo eso que estabas haciendo, agrega a tu backlog lo que
>    te he dicho» ⇒ este documento.
> 5. «no olvides nunca jamás tu tarea principal» ⇒ **esta lista NO sustituye a la
>    tarea que el jefe asigna en cada momento**; es una lista más. La tarea
>    principal del 2026-11-23 era la auditoría de la parte de chat/LLM
>    (`docs/AUDIT_LLM_V16.md`), y sigue viva.

Todo lo de aquí está **medido** sobre el repo en `main` (SHA `78501f4`), con
`fichero:línea` para que se pueda verificar. Nada de lo que sigue está
inventado: lo que no se ha medido, está marcado como **NO MEDIDO**.

---

> **Arquitectura objetivo e investigación (Codex + Claude Code, medidos el
> 2026-11-24): `docs/AGENTIC_ARCHITECTURE.md`** — los seis huecos reales del
> daemon, la hoja de ruta P0..P4 con su puerta de aceptación y el primer
> corte ya implementado (`rust-core/src/protocol.rs`).

## 1. Lo que YA existe (medido) — la API del daemon

| Qué | Dónde | Estado |
|---|---|---|
| Gateway **compatible con OpenAI** (`POST /v1/chat/completions`, alias `/chat/completions`) | `rust-core/src/cockpit_server.rs:1378` → `crate::llm_gateway::chat_completions` | **EXISTE y está cableado** |
| Dialecto **Gemini** (`/v1internal:*`, `generateContent`, `streamGenerateContent`) | `rust-core/src/cockpit_server.rs:1416` → `crate::llm_gateway::gemini_route` | **EXISTE** (es el dialecto que habla el cliente de Antigravity) |
| Endpoint de chat propio del cockpit (`POST /api/chat`) | `rust-core/src/cockpit_server.rs:2292` | **EXISTE** |
| Listado de proveedores (con secretos redactados) | `rust-core/src/cockpit_server.rs:1541` y `:2380` | **EXISTE** |
| Interfaz web embebida en el daemon | `rust-core/src/webui.rs` (5.014 líneas) | **EXISTE**, pero **no tiene página de chat con el agente** |
| Herramientas por MCP/HTTP (p. ej. `drive_backup`) | `rust-core/src/mcp_ext.rs` | **EXISTE** |
| CLI de chat | `megadaemon-core chat <modelo> <prompt>` | **EXISTE** |

⇒ **El punto 2 del jefe (la API) ya está resuelto de facto**: cualquier cliente
OpenAI-compatible puede hablar con el daemon si apunta a
`http://<nodo>:3000/v1`. Lo que falta es (a) que haya una **interfaz** y
(b) que el **cliente de Antigravity** (que está en Python) hable con él.

---

## 2. Lo que falta (medido)

### 2.1 No hay interfaz de chat con el agente
`webui.rs` sirve la interfaz del cockpit, pero no hay ninguna página que
permita **hablar con el agente** por el gateway. El usuario tiene que usar
`curl` contra `/v1/chat/completions`.

### 2.2 El cliente de Antigravity es Python y el móvil NO tiene Python
`core/` (16 ficheros: `megadaemon.py`, `minidaemon.py`, `brain.py`, `router.py`,
`process_supervisor.py`, `rust_bridge.py`, …) es **Python 2/3**.

MEDIDO en `docs/SOFTWARE_INSTALLED.md` (2026-11-23, S21 Ultra por root):
`python3` **NO existe** en el móvil (`which python3` → rc=127), tampoco
`node`, `sqlite3`, `jq`, `crond`, `at`, `curl` ni `wget`.

⇒ «fusionarse con una copia del GUI de Antigravity» **no** puede significar
ejecutar ese Python en el móvil: hay que **portar el cliente a Rust** (o a
WASM) y conectarlo al `/v1/chat/completions` y al `/v2` del daemon.

### 2.3 El agente (yo) no trabaja por el daemon
El trabajo del agente se hace hoy con scripts sueltos (`tools/*.sh`) y con la
tool MCP del harness, no contra la API del daemon. Mientras sea así, la
integración es **nominal**: el daemon expone la API, pero nadie (ni el agente)
la usa como cliente.

---

## 3. Backlog con puerta de aceptación

### P0-1 · Página de chat con el agente en la UI web del cockpit
Una página nueva servida por el daemon (mismo origen, sin dependencias
externas: el móvil no tiene node) con: modelo (del vault), prompt, respuesta,
y botón de enviar.
**Puerta**: desde un navegador real contra el nodo, una petición a la página
devuelve la respuesta de un proveedor real del vault, y el cuerpo de la
petición sale por `POST /v1/chat/completions` (se comprueba en el log del
daemon, no en el navegador).

### P0-2 · El daemon puede hablar consigo mismo
El gateway debe poder enrutar una petición al **proveedor local** (SEERE /
motor local) y no sólo a proveedores remotos, para que el agente pueda
usar el daemon como cliente de sí mismo sin salir a Internet.
**Puerta**: `POST /v1/chat/completions` con el modelo local devuelve respuesta
**sin** salir a la red (verificable en el log de peticiones).

### P1-1 · Puerto a Rust del cliente de Antigravity (el «GUI»)
Empezar por `core/brain.py` + `core/router.py` (los que hablan con el modelo)
y `core/rust_bridge.py` (que ya habla con Rust ⇒ el camino existe).
**Puerta**: el mismo diálogo que hoy hace `brain.py` se reproduce con el
binario Rust, apuntando al `/v1/chat/completions` del daemon, **en el móvil**.

### P1-2 · El cliente usa el `/v2` del daemon para el estado
El dashboard del agente (tareas, bus, cola) sale del `/v2`, no de ficheros
sueltos: así hay una sola fuente de verdad.
**Puerta**: el cliente Rust lee estado, tareas y salud **sólo** por HTTP del
daemon (verificable: sin acceso directo a `state/`).

### P2-1 · El agente trabaja por el daemon (tools propias en MCP)
`board` (tasks), `bus` (publicación), `backup` y `automation` han de ser
herramientas MCP propias del daemon, para que el trabajo del agente en
cualquier nodo pase por la misma API que ve el usuario.
**Puerta**: una tarea del tablón creada por el agente aparece en el cockpit
**y** se puede ejecutar desde la UI nueva.

### P2-2 · La cuota se respeta desde el daemon
El veto de presupuesto (`daily_budget_permite`) ya está en el camino del
chat; falta que la **interfaz** lo muestre (por qué una respuesta no salió:
proveedor de pago, cuota, retired) en vez de fallar en silencio.
**Puerta**: la respuesta de la UI explica el motivo con el campo `reason` del
`provider_trace`.

---

## 4. Riesgos medidos que condicionan el plan

1. **Sin node ni python3 en el móvil** ⇒ nada de Electron ni de Python: el
   cliente tiene que ser Rust (o WASM servido por el propio daemon).
2. **Memoria**: el sandbox de desarrollo tiene 3 GB y el móvil es el objetivo;
   el cliente Rust debe ser pequeño (el daemon ya usa `--release` + LTO).
3. **La API ya está ⇒ el riesgo NO es el backend, es el cliente**: el 90 % del
   esfuerzo de P1-1 está en portar `core/`, no en tocar el gateway.
4. El gateway tiene el dialecto **Gemini** (`/v1internal:*`) implemented; el
   cliente de Antigravity ya lo usa ⇒ al portarlo, ese dialecto es el que hay
   que verificar primero (`docs/ANTIGRAVITY_INTEL.md`).

---

## 5. Trazabilidad

| afirmación | de dónde sale |
|---|---|
| `/v1/chat/completions` cableado | `rust-core/src/cockpit_server.rs:1378` |
| dialecto Gemini `/v1internal:*` | `rust-core/src/cockpit_server.rs:1416` |
| `/api/chat` del cockpit | `rust-core/src/cockpit_server.rs:2292` |
| UI web embebida (5.014 líneas) | `rust-core/src/webui.rs` |
| el cliente Antigravity es Python | `core/*.py` (16 ficheros) |
| el móvil no tiene python3/node/sqlite3/jq/curl/wget | `docs/SOFTWARE_INSTALLED.md` (medido por root, 2026-11-23) |
| la auditoría que es la tarea principal de esta sesión | `docs/AUDIT_LLM_V16.md` |
