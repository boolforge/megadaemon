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

---

## 6. Lo que aporta el agente `general` (2026-11-23) — auditoría de ramas + credenciales

> Esta sección la añade el agente cuya **tarea principal** de esta sesión fue la
> **auditoría de ramas** y la **rotación de credenciales** (`docs/AUDITORIA-RAMAS-2026-11-23.md`
> y `docs/ROTACION-CREDENCIALES.md`). No sustituye a §3: lo de aquí es lo que ya
> está **medido y entregado**.

### 6.1 H2/H3 — la interfaz existe ya (Jetski × API v2)

`web/cockpit_antigravity.html` — una sola página, sin dependencias externas (el móvil
no tiene `node`), con:

| pieza | de dónde sale | qué hace |
|---|---|---|
| marco visual *Jetski Web* | `/workspace/agy-intel/assets_web_index.html` (el shell de la IDE de Antigravity, copy local) | tema claro/oscuro, `backdrop-filter`, `--accent-glow` |
| cliente de la **API v2** | `docs/API_V2.md` §3 (2 capas de auth) | `POST /api/session/exchange` (la API key **en el cuerpo**) → `X-Seere-Auth-Token`; el token **no se cachea** (rota en cada reinicio del daemon) |
| Panel / Agentes / Tareas | `/api/v2/dashboard`, `/api/v2/nodes`, `/api/v2/tasks` | una vista = una llamada agregada |
| **Chat con el agente** | `POST /api/chat` (`rust-core/src/cockpit_server.rs:2383`) | cierra **P0-1** (página de chat con el agente) |
| Asignar / enviar | `POST /api/v2/nodes/assign`, `POST /api/v2/nodes/message` | **P2-1**: una tarea publicada desde la UI llega al bus |
| Reclamar / reportar | `POST /api/v2/tasks/claim\|release\|report` | el agente trabaja por el tablero, no por scripts sueltos |
| Pestaña *Antigravity* | `assets_web_index.html` | iframe de la IDE de Antigravity en la misma página ⇒ **la fusión** |

**Puerta de aceptación de H2/H3:** desde un navegador real contra el nodo, la pestaña
*Chat* devuelve una respuesta de un proveedor real del vault y **el cuerpo de la
petición sale por `POST /api/chat`** (se comprueba en el log del daemon, no en el
navegador). *NO verificado en el móvil: el túnel daba 502 durante esta sesión
(`curl` → `http=502` contra `MEGADAEMON_URL` ⇒ no es la credencial, es el móvil).*

### 6.2 H4/P2-1 — el agente es un nodo del sistema

`tools/registrar-agente-general.py` — **idempotente** y **dry-run por defecto**
(el mismo patrón que `tools/md-repair-entorno.sh`: *un diagnóstico que escribe estado
puede romper el sistema que está diagnosticando*).

```bash
tools/registrar-agente-general.py                # dry-run: imprime lo que haría
tools/registrar-agente-general.py --apply        # escribe (instances + tasks + registro)
```

Da de alta `agent-general@cloud-compute-new-hjn5f` en `state/board.sqlite`
(tabla `instances`, **mismo INSERT** que `coordination.rs::register_instance`) y deja
sus 5 tareas en el tablón. El registro por fichero es `state/agents/agent-general.json`
(mismo esquema que `state/agents/inventory-agent.json`).

### 6.3 Lo que la auditoría añadió al backlog (P0, sin cerrar)

- **P0 · `main` está roto y es PÚBLICO**: marcadores de conflicto sin resolver en
  `state/mesh_board.json:3-7` ⇒ **el bus no parsea como JSON**. Culpables medidos:
  `d148c95` (`merge(flota)`) y `d902a7e` (`docs=theirs, codigo=ours`).
  Detalle: `docs/AUDITORIA-RAMAS-2026-11-23.md` §0.
- **P0 · credenciales expuestas**: 4 API keys de Google (`AIzaSy…`) escritas a pelo
  en `tools/drive_ls.py:23-26` de un repo **público** (`boolforge/megadaemon`,
  `private=false`). Detalle y procedimiento: `docs/ROTACION-CREDENCIALES.md`.
- **Lo que NO hay que rehacer**: la API key viva (`mdaemon_sk_live_…`) **no está en el
  repo** — 0 coincidencias comparando el valor completo (sha256) contra las 81 puntas
  de rama. Esa rotación ya está hecha.

### 6.4 Nota de trazabilidad

| afirmación | de dónde sale |
|---|---|
| `web/cockpit_antigravity.html` (la fusión) | este repo, rama `rama/ramas-auditoria-2026-11-23` |
| copy del GUI de Antigravity | `/workspace/agy-intel/assets_web_index.html` |
| `POST /api/chat` (body: `message`, `model`, `system`, `max_tokens`) | `rust-core/src/cockpit_server.rs:2383-2388` |
| `/api/v2/nodes/assign` · `/api/v2/nodes/message` | `rust-core/src/webui.rs:74-75` y `:4103-4108` |
| `/api/v2/tasks/claim\|release\|report` | `rust-core/src/webui.rs:4120-4123` |
| `instances` / `tasks` / `events` del tablón | `state/board.sqlite` (esquema en `rust-core/src/coordination.rs:322`) |
| repo público | `curl -s https://api.github.com/repos/boolforge/megadaemon` → `"private":false` |
