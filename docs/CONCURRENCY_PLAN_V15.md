# MegaDaemon v15 — CONTRATO DE CONCURRENCIA

> **Este documento es un contrato de diseño, no un informe de estado.** Su
> función es que dos agentes distintos puedan implementar la misma cosa sin
> pisarse, y que un tercero pueda verificar lo implementado sin preguntar.
>
> **Cómo se lee**: cada sección define *qué* se implementa, *quién* lo
> implementa, *cómo se comprueba* y *qué NO se puede hacer*. Los identificadores
> (`code`, `route_id`, `agent_id`, `zone`, `class`) están en inglés; la prosa en
> español. Cuando algo no está medido, se dice `pendiente` y no se estima.
>
> **Alcance**: API v2, colaboración entre agentes, automejora local, sistema
> distribuido, compartición de ficheros, **reparto de trabajo entre nodos con
> certificados de nodo y reglas locales** (§9) y **unificación con la
> especificación ya escrita y con la crítica adversaria** (§10).
>
> **Documentos que este contrato NO sustituye ni contradice**:
> `docs/MASTER_PLAN.md` (plan estratégico), `docs/COORDINATION.md` (tablón
> append-only), `docs/AUDIT_ENJAMBRE_V14.md` (hechos medidos),
> `docs/CONNECT_EXTERNAL_AGENT_ANDROID_RE.md` (protocolo del agente externo),
> `docs/API_V2.md` y `docs/UI_V2_SPEC.md` (especificaciones de
> implementación, **capa 2** de §10.2) y `docs/AUDIT_ADVERSARIAL_V15.md`
> (capa 3: objeciones, **no decide**).
>
> **Alcance de cambios de este documento**: nuevo fichero, 2026-11-19.
> No se edita ningún otro fichero del repo.

---

## 0. RESUMEN EJECUTIVO (13 líneas)

1. MegaDaemon no es un "triste chatbot" por falta de capacidades: `rust-core/`
   tiene ~50 módulos de RE real (`dex_decompile`, `signing`, `apk_inventory`,
   `root_hide`, `delegate`…). Es un chatbot por **arquitectura**: el trabajo
   vive en el chat, no en el daemon; la API es un `match` de rutas legacy; y
   no hay estado compartido fiable entre agentes.
2. El enjambre no avanza por **E/S**, no por falta de trabajo: `/workspace` es
   NFS v3 `nolock` (medido: 0 `.rlib` producidos en 15 `target/`).
3. El contrato que falta es el **API v2** (superficie única, envelope único,
   paginación, idempotencia, `X-Request-Id`) + el **bus** + el **libro de
   fallos** + el **contrato de ficheros**.
4. El reparto de trabajo se hace por el **tablero SQLite** (`<state_dir>/board.sqlite`),
   con `add → claim (lease) → report`, y es la única fuente de verdad.
5. La **colaboración entre agentes** se hace por HTTP + NDJSON contra el
   primary. No MQTT: el móvil tiene ~0,3 GB libres de RAM.
6. El **sistema agéntico local** se define como un ciclo cerrado
   `observación → diagnóstico → acción → medición → aprendizaje → ajuste`,
   con un *libro de fallos* estructurado en NDJSON.
7. El **sistema distribuido** es *primary-único + merge idempotente
   convergente*. Nada de Raft/consenso: no compensa en un teléfono.
8. La **automejora** es real pero acotada: todo lo reversible se automatiza;
   lo irreversible (`confirm: true`) exige humano.
9. **Cada nodo aporta y cada nodo consume**: el reparto de trabajo entre
   nodos se decide con **certificados de nodo** (`node cert`) publicados por
   cada instancia y con **reglas locales** que indican qué se comparte, qué
   se reserva para el usuario y qué se delega (§9). El móvil no es sólo
   coordinador: cuando está libre **acepta** trabajo; cuando el usuario lo
   necesita, lo **delega**.
10. Cada ruta, tabla, fichero y métrica de este documento tiene **un dueño**.
    Un `owner_id` que aparece en dos filas es un conflicto: se reporta y se
    resuelve en el tablón antes de codificar.
11. Este documento **no escribe código Rust**: define contratos (JSON, SQL,
    HTTP) que se implementan tal cual.
12. Las decisiones que dependen del usuario están todas en §7 (incluidas las
    cuatro nuevas de §7.7 sobre el reparto de carga entre nodos).
13. **Estado real de la adopción (§10)**: el contrato **todavía no lo usan
    todos los agentes**. Hay tres capas — este contrato (decide),
    `docs/API_V2.md`/`docs/UI_V2_SPEC.md` (implementan) y la crítica
    adversaria (objeta) — y ahora están **unificadas** por §10.2-§10.4: el
    envelope y el vocabulario de rutas ya implementados **prevalecen**, y las
    81 hallazgos de la crítica (áreas A…J) están registrados y pendientes de
    veredicto. Para que el contrato sea vinculante faltan dos acciones
    mecánicas: **commitearlo** y **una línea en `docs/COORDINATION.md`**
    (§10.6).

---

## 1. DIAGNÓSTICO COMÚN

### 1.1 Hechos medidos (no hay especulación en esta tabla)

Cada fila cita el documento y el apartado donde el hecho está medido. Si un
hecho no está medido, **no aparece aquí**: está en §1.4 (pendientes).

| # | Hecho medido | Valor | Fuente |
|---|---|---|---|
| D-1 | Tipo de `/workspace` | `nfs` (vers=3, `nolock`, `hard`, TCP) → NAS Alibaba | `AUDIT_ENJAMBRE_V14.md` §1.1 |
| D-2 | Disco local del contenedor de agentes | overlay/ext4, 30 GB, **26 GB libres** | `AUDIT_ENJAMBRE_V14.md` §1.1 |
| D-3 | `CARGO_HOME` | `/workspace/.home/.cargo` — **también en el NAS** | `AUDIT_ENJAMBRE_V14.md` §1.1 |
| D-4 | `.rlib` producidos en todo el workspace | **0**, tras revisar **15** `target/` | `AUDIT_ENJAMBRE_V14.md` §1.1 |
| D-5 | Procesos `cargo`/`rustc` simultáneos | **65 → 128** | `AUDIT_ENJAMBRE_V14.md` §1.1 |
| D-6 | Procesos `cargo` simultáneos (medición alternativa) | **52** | `tools/build-queue.sh` cabecera |
| D-7 | Load average en 2 CPUs | **28-39** | `AUDIT_ENJAMBRE_V14.md` §1.1 |
| D-8 | `cp -a` del registro de crates (NAS → local) | **3,7 MB en 4 minutos** | `AUDIT_ENJAMBRE_V14.md` §1.1 |
| D-9 | RAM del **contenedor de agentes** | 3 GB totales, ~700 MB libres | `AUDIT_ENJAMBRE_V14.md` §1.1 |
| D-10 | RAM del **móvil** (S21 Ultra) | 10,5 GB totales, **10,2 GB usados** (~0,3 GB libres) | `CONNECT_EXTERNAL_AGENT_ANDROID_RE.md` §0.8 |
| D-11 | Ficheros de código sin commitear | **23** (~60.000 líneas) | `AUDIT_ENJAMBRE_V14.md` §0.3 |
| D-12 | `main` frente a `origin/main` | divergido: **behind 10 / ahead 4** | `AUDIT_ENJAMBRE_V14.md` §0.3 |
| D-13 | Tablero `board.sqlite` | **no existe** ni en el repo ni en el PC | `AUDIT_ENJAMBRE_V14.md` §0.4 |
| D-14 | Bloque `impl TaskBoard` **duplicado byte a byte** (239 líneas) en `coordination.rs` | el crate no compilaba | `COORDINATION.md` cremallero 21:5x (agente APK) |
| D-15 | `delegate.rs:561` (`E0425`) | rompió el build de todo el swarm | `COORDINATION.md` cremallero 22:0x |
| D-16 | Ficheros **declarados en `lib.rs` pero ausentes en disco/HEAD** (`apk_inventory.rs`, `cockpit_doctor.rs`, `adb_client.rs`) | nadie podía compilar | `COORDINATION.md` cremallero 21:5x / 22:1x |
| D-17 | Ficheros **borrados** del árbol compartido (`adb_client.rs`, `coordination.rs` a las 20:36; 5 `docs/` a las 20:37) | recuperados del object store de git | `COORDINATION.md` cremallero 22:1x |
| D-18 | `git checkout`/`stash`/`clean` sobre el árbol compartido | **pisó a 3 agentes** | `COORDINATION.md` cremallero 22:0x |
| D-19 | Rate limit | **60 req/min por IP**; en el móvil **todo** el tráfico sale por `127.0.0.1` (navegador + túnel Cloudflare + agentes) ⇒ **un único cubo** | `api_key.rs` (`RATE_MAX_REQUESTS = 60`) + `COORDINATION.md` 22:1x |
| D-20 | Cascada 429 | una recarga de la UI (20+ peticiones) agotaba el cubo ⇒ el túnel devolvía **429**; los agentes lo leían como "la instancia está caída" y **abandonaban**; en el diagnóstico **una llamada MCP propia murió con HTTP 429 y otra con 502** | `COORDINATION.md` cremallero 22:1x |
| D-21 | `web_bridge.py` | murió **3× en 30 min**; además era **IPv4-only** y Chrome en Android resuelve `localhost` a `::1` primero | `COORDINATION.md` cremallero 22:0x |
| D-22 | La cola de trabajo era **el chat** | 20+ mensajes del usuario sin leer, sin estado, sin auditoría | `MASTER_PLAN.md` §0 |
| D-23 | El daemon **no estaba corriendo** en el Samsung | `ERR_CONNECTION_REFUSED` en 3000/3030/8090/9090 | `MASTER_PLAN.md` §0 |
| D-24 | Deuda de estado | `lib.rs` cableaba 9 módulos v14 (~22.000 líneas) que dormían **sin compilar** | `MASTER_PLAN.md` §4 Ola 0 |

### 1.2 Por qué "megadaemon es un triste chatbot"

Cuatro causas, cada una con su hecho:

| # | Causa | Hecho que la sostiene | Consecuencia para el usuario |
|---|---|---|---|
| C-1 | **El trabajo no vive en el daemon.** El daemon tiene las capacidades; la *coordinación* vive en el chat. | D-22: la cola era el chat, 20+ mensajes sin leer ni auditar. | El sistema no recuerda, no continúa, no demuestra nada. Un daemon que sólo responde a un prompt *es* un chatbot, por muy grande que sea el `lib.rs`. |
| C-2 | **La API es un `match` monolítico**, sin contrato: cada ruta devuelve una forma distinta, no hay envelope, no hay paginación, no hay idempotencia, no hay `X-Request-Id`. | `cockpit_server.rs` ~70 rutas `/api/*` legacy con cuerpos ad-hoc (`send_json` directo), y `/mcp` como segunda superficie con lógica propia. | Ni la UI ni los agentes pueden construir una capa de datos fiable. Cada consumidor reimplementa paginación, errores y reintentos. |
| C-3 | **No hay estado compartido con autoridad.** Sin `board.sqlite` (D-13), sin leases entre nodos, sin bus de mensajes. | D-13; `mesh.rs` es un bus *peer-to-peer* de malla y `coordination.rs` un tablero *por instancia*: hoy nadie coordina a nadie. | Los agentes se pisan, repiten trabajo y se rinden (D-20) sin que nadie lo sepa. |
| C-4 | **No hay memoria de los fallos.** No existe un registro estructurado de "qué falló, por qué y qué se hizo". | `mesh.rs::learning_notes()` sólo agrega `AgentTask` en memoria; `delegate.rs` lleva EWMA de calidad por cuenta, pero nada cruza ambos mundos. | El enjambre repite el mismo error mañana. Un sistema que no aprende de sus fallos es un chatbot con más botones. |

### 1.3 Por qué el enjambre no avanza

| # | Causa | Hecho | Contramedida ya decidida | Effector en v15 |
|---|---|---|---|---|
| E-1 | **E/S bloqueada, no CPU.** `/workspace` en NFS `nolock`. | D-1, D-3, D-4, D-8 | Regla 5-bis de `MASTER_PLAN.md`: `CARGO_TARGET_DIR` local + `tools/build-queue.sh` con `flock` | §6 A-4 (build queue como servicio, medido) |
| E-2 | **Builds concurrentes no serializados.** | D-5, D-6, D-7; errores falsos `failed to copy bitcode` / `rust-lld: cannot open ...rcgu.o` / `database is locked` (D-1: el índice de crates es SQLite sobre NFS con `nolock`) | `tools/build-queue.sh` con `flock`, 2 slots, `CARGO_INCREMENTAL=0`, `-j 2` | §6 A-4 |
| E-3 | **Pisado de ficheros.** `git add -A`, `checkout`, `stash`, `clean`, merges que se llevan la versión vieja de un fichero. | D-11, D-12, D-16, D-17, D-18 | Reglas 2, 3 y 7 de `MASTER_PLAN.md`; "nada de `git checkout/stash/clean`" | §3.4 (reglas de no pisado, ahora **con dueño de fichero por ruta exacta**) |
| E-4 | **429 en cascada que se lee como "caída".** | D-19, D-20 | El puente sale con IP de origen `127.0.0.2`; `MEGADAEMON_RATE_MAX` y `MEGADAEMON_RATE_LOOPBACK` configurables | §2.4 (rate limit **por agente**, no por IP) |
| E-5 | **El tablero no existe.** "El plan vive en el demonio" era una intención. | D-13 | `md-coord` ya está escrito (`coordination.rs`, `src/bin/md-coord.rs`) pero no desplegado | §3.2, §6 A-1: **desplegar y publicar** |
| E-6 | **Un módulo roto rompe a todos.** | D-14, D-15 | Arnés `#[path]` por módulo antes de tocar `lib.rs` | §6 A-4 (gate de CI local: arnés verde antes de cablear) |
| E-7 | **El trabajo pesado se pide al móvil.** | D-10 (~0,3 GB libres) | — (nunca se aplicó) | §5.4 (reparto explícito móvil/PC + presupuestos) |

### 1.4 Lo que **no** está medido (se prohíbe tratarlo como hecho)

| # | Pendiente de medición | Cómo se mide | Por qué importa |
|---|---|---|---|
| P-1 | RSS real del daemon en el Samsung | `/proc/<pid>/status` vía `root_exec` | Fija el presupuesto de RAM de §5.4 |
| P-2 | Latencia real del bus entre móvil y PC | `POST /api/v2/nodes/message` + subscribe, p95 sobre 100 mensajes | Fija el SLA de §3.5 |
| P-3 | Coste real (µs y bytes) de `sha256` por MB en el Exynos 2100 | benchmark con el crate `sha2` | §8 verificación de manifiestos |
| P-4 | Throughput real de `POST /api/v2/share/put` por la red del móvil | pushing de un artefacto de 50 MB | §8 idempotencia y coste |
| P-5 | Deriva de calidad por proveedor (EWMA de `delegate.rs`) con ventana de 24 h | `delegate::adjustments()` + ledger de precios | §4.6 |

> Regla: un número que no viene de una medición entra en el documento
> marcado `pendiente` y con su método de medición. **Nunca** con "≈" ni con
> "estimado".

---

## 2. CONTRATO DE API v2

### 2.0 Principios (normativos)

| # | Regla |
|---|---|
| AP-1 | **Una superficie HTTP única**: todo lo nuevo bajo `/api/v2/`. `/mcp` (JSON-RPC) se mantiene por compatibilidad y **no** es una segunda API: es un *mismo* dispatch (§2.5). |
| AP-2 | **Un único envelope de error** (§2.2) para todas las rutas v2. |
| AP-3 | **Toda respuesta lleva `X-Request-Id`**. Si el cliente no lo manda, el servidor lo genera. |
| AP-4 | **Toda lista se pagina**: `?limit=` + `?cursor=`, orden estable, `next_cursor` en la respuesta. |
| AP-5 | **Todo POST que cree estado es idempotente** mediante `Idempotency-Key`. |
| AP-6 | **Una ruta, un dueño.** La columna `owner` de §2.1 es la asignación; el que no es dueño **no la implementa**. |
| AP-7 | **Ruta no implementada = `501 not_implemented`**, nunca `404`. `404` significa "recurso inexistente"; `501` significa "esta ruta existe y está asignada, pero su dueño no la ha entregado". Esto hace visible el retraso en vez de hiding it. |
| AP-8 | **v2 no habla con la UI por dentro**: la UI (`web/cockpit_v2.html`) consume **sólo** `/api/v2/*`. Las rutas v1 (`/api/*`) quedan congeladas: se sirven, no se amplían. |
| AP-9 | **No hay puerto nuevo**: v2 se sirve en el mismo listener del cockpit (9090 real + alias 3000/3030/8090/8080/80). El móvil no abre sockets nuevos. |

### 2.1 Tabla de rutas

Columnas: **Auth** = `public` | `api-key` | `api-key+token`.
`api-key` = cabecera `X-API-Key` (o `Authorization: Bearer`); `api-key+token` =
además `X-Seere-Auth-Token` (escritura). Definición exacta del gate:
`api_key::decide()` en `rust-core/src/api_key.rs`.

| # | Método | Path | Auth | Params | Respuesta 2xx | `route_id` | Estado | Owner (fichero) |
|---|---|---|---|---|---|---|---|---|
| R-01 | GET | `/api/v2/health` | public | — | `{status,version,uptime_s,protocol,schema,degraded,routes_pending}` | `v2.health` | **nueva** | `webui.rs` (api) |
| R-02 | GET | `/api/v2/version` | public | — | `{version,protocol,build}` | `v2.version` | **nueva** | `webui.rs` (api) |
| R-03 | GET | `/api/v2/capabilities` | api-key | — | `{items:[Capability],digest}` | `v2.capabilities` | **nueva** | `webui.rs` → `capability.rs` |
| R-04 | GET | `/api/v2/tasks` | api-key | `scope(local\|board),status,owner,limit,cursor` | `{items:[Task],next_cursor,stats,journal}` | `v2.tasks.list` | **existente** (agregado) | `webui.rs` (api) |
| R-05 | POST | `/api/v2/tasks` | api-key+token | body `{title,class,priority,owner_hint,payload,lease_secs}` | `{id,status:"queued"}` | `v2.tasks.add` | **nueva** (extiende R-04) | `coordination.rs` (coord) |
| R-06 | GET | `/api/v2/tasks/{id}` | api-key | — | `{task:Task}` | `v2.tasks.get` | **nueva** | `coordination.rs` (coord) |
| R-07 | POST | `/api/v2/tasks/{id}/claim` | api-key+token | body `{owner,lease_secs}` | `{won,lease_expires_at,owner}` | `v2.tasks.claim` | **nueva** | `coordination.rs` (coord) |
| R-08 | POST | `/api/v2/tasks/{id}/report` | api-key+token | body `{ok,report}` | `{status,updated_at}` | `v2.tasks.report` | **nueva** | `coordination.rs` (coord) |
| R-09 | POST | `/api/v2/tasks/{id}/heartbeat` | api-key+token | body `{owner}` | `{lease_expires_at}` | `v2.tasks.heartbeat` | **nueva** | `coordination.rs` (coord) |
| R-10 | GET | `/api/v2/events` | api-key | `kind,actor,since,limit,cursor` | `{items:[Event],next_cursor}` | `v2.events.list` | **nueva** | `coordination.rs` (coord) |
| R-11 | GET | `/api/v2/nodes` | api-key | `role,capability,limit,cursor` | `{items:[Node],next_cursor,instances,claims,events}` | `v2.nodes.list` | **existente** | `webui.rs` (api) |
| R-12 | GET | `/api/v2/schema` | api-key | — | `{schema,protocol,min_supported_protocol,tables}` | `v2.schema.get` | **nueva** | `coordination.rs` (coord) |
| R-13 | POST | `/api/v2/nodes/sync` | api-key+token | body `{snapshot}` → `{snapshot}` | merge idempotente | `v2.nodes.sync` | **nueva** (junto a `nodes/migrate`) | `coordination.rs` (coord) |
| R-14 | GET | `/api/v2/agents` | api-key | `role,capability,limit,cursor` | `{items:[Agent],next_cursor}` | `v2.agents.list` | **nueva** | `autonomy.rs` (agent) |
| R-15 | GET | `/api/v2/agents/{id}` | api-key | — | `{agent:Agent}` | `v2.agents.get` | **nueva** | `autonomy.rs` (agent) |
| R-16 | POST | `/api/v2/agents/register` | api-key+token | body `{agent_id,role,capabilities,instance_id}` | `{agent_id,registered:true}` | `v2.agents.register` | **nueva** | `autonomy.rs` (agent) |
| R-17 | GET | `/api/v2/goals` | api-key | `status,limit,cursor` | `{items:[Goal],next_cursor}` | `v2.goals.list` | **nueva** | `agent_core.rs` (agent) |
| R-18 | POST | `/api/v2/goals` | api-key+token | body `{goal,prefer_llm,max_tasks}` | `{goal_id,plan:[PlannedTask]}` | `v2.goals.create` | **nueva** | `agent_core.rs` (agent) |
| R-19 | GET | `/api/v2/goals/{id}` | api-key | — | `{goal_id,status,plan,results}` | `v2.goals.get` | **nueva** | `agent_core.rs` (agent) |
| R-20 | GET | `/api/v2/failures` | api-key | `kind,tag,severity,status,since,limit,cursor` | `{items:[Failure],next_cursor}` | `v2.failures.list` | **nueva** | `autonomy.rs` (agent) |
| R-21 | POST | `/api/v2/failures` | api-key+token | body = nota estructurada (§4.2) | `{id,stored:true}` | `v2.failures.create` | **nueva** | `autonomy.rs` (agent) |
| R-22 | GET | `/api/v2/metrics` | api-key | `window_s,section` | `{failure_rate_by_kind,mttr_s,verified_fix_ratio,provider_drift,fleet_ratio}` | `v2.metrics.get` | **nueva** | `autonomy.rs` (agent) |
| R-23 | GET | `/api/v2/alerts` | api-key | `min_severity,include_ack,limit,cursor` | `{items:[Alert],next_cursor}` | `v2.alerts.list` | **existente** | `webui.rs` → `alerts.rs` |
| R-24 | POST | `/api/v2/alerts/{id}/ack` | api-key+token | — | `{id,acknowledged:true}` | `v2.alerts.ack` | **nueva** | `webui.rs` → `alerts.rs` |
| R-25 | POST | `/api/v2/nodes/message` | api-key+token | body `{node,message,data_b64}` | `{delivered:true}` | `v2.bus.send` | **existente** | `webui.rs` (api) |
| R-26 | POST | `/api/v2/nodes/broadcast` | api-key+token | body `{topic,kind,body}` | `{message_id,recipients}` | `v2.bus.broadcast` | **nueva** | `mesh.rs` (mesh) |
| R-27 | POST | `/api/v2/nodes/reply` | api-key+token | body `{in_reply_to,body}` | `{message_id}` | `v2.bus.reply` | **nueva** | `mesh.rs` (mesh) |
| R-28 | GET | `/api/v2/nodes/messages` | api-key | `topic,node,since,limit,cursor` | `{items:[Message],next_cursor}` | `v2.bus.list` | **nueva** | `mesh.rs` (mesh) |
| R-29 | GET | `/api/v2/nodes/subscribe` | api-key | `topic,since,cursor,wait_s(≤30)` | NDJSON o `{items,next_cursor}` | `v2.bus.subscribe` | **nueva** | `mesh.rs` (mesh) |
| R-30 | POST | `/api/v2/share/manifest` | api-key+token | body = §8.2 | `{accepted,rejected,manifest_sha256}` | `v2.share.manifest.publish` | **nueva** | `file_share.rs` (files) |
| R-31 | GET | `/api/v2/share/manifests` | api-key | `zone,agent,since,limit,cursor` | `{items:[Artifact],next_cursor}` | `v2.share.manifest.list` | **nueva** | `file_share.rs` (files) |
| R-32 | POST | `/api/v2/share/put` | api-key+token | body `{path,data_b64,sha256}` | `{sha256,size,duplicate}` | `v2.share.put` | **existente** | `webui.rs` (files) |
| R-33 | GET | `/api/v2/share/verify` | api-key | `sha256` | `{ok,mismatches[]}` | `v2.share.verify` | **nueva** | `file_share.rs` (files) |
| R-34 | GET | `/api/v2/share/get` | api-key | `path,offset,len` | base64 + `sha256` | `v2.share.get` | **existente** | `webui.rs` (files) |
| R-35 | GET | `/api/v2/routes` | api-key | — | `{items:[{method,path,route_id,owner,tool}],auth}` | `v2.routes.get` | **existente** (ampliar con `route_id`+`tool`) | `webui.rs` (api) |
| R-36 | POST | `/mcp` | api-key+token | JSON-RPC 2.0 | JSON-RPC 2.0 | `v2.mcp.rpc` | **existente** (JSON-RPC; **no** se crea `/api/v2/mcp`) | `mcp_server.rs` (mcp) |
| R-37 | POST | `/api/v2/certs/publish` | api-key+token | body = `node cert` (§9.2) | `{cert_hash,seq,accepted:true}` | `v2.certs.publish` | **nueva** | `node_cert.rs` (dist) |
| R-38 | GET | `/api/v2/certs` | api-key | `node,class,stale,limit,cursor` | `{items:[Cert],next_cursor}` | `v2.certs.list` | **nueva** | `node_cert.rs` (dist) |
| R-39 | GET | `/api/v2/certs/{node}` | api-key | — | `{cert:Cert,rules_hash,contribution}` | `v2.certs.get` | **nueva** | `node_cert.rs` (dist) |
| R-40 | GET | `/api/v2/rules` | api-key | `node` | `{items:[{node,rules_hash,effective,evaluated_at}]}` | `v2.rules.list` | **nueva** | `node_cert.rs` (dist) |
| R-41 | POST | `/api/v2/offers` | api-key+token | body = oferta (§9.6) | `{offer_id,accepted:true}` | `v2.offers.create` | **nueva** | `node_cert.rs` (dist) |
| R-42 | GET | `/api/v2/offers` | api-key | `class,node,limit,cursor` | `{items:[Offer],next_cursor}` | `v2.offers.list` | **nueva** | `node_cert.rs` (dist) |
| R-43 | GET | `/api/v2/contributions` | api-key | `window_s,node,limit,cursor` | `{items:[Contribution],fleet_ratio}` | `v2.contributions.list` | **nueva** | `node_cert.rs` (dist) |
| R-44 | POST | `/api/v2/placement` | api-key+token | body `{class,kind,cost,priority}` | `{node,reason,alternatives[]}` | `v2.placement.decide` | **nueva** | `node_cert.rs` (dist) |

#### 2.1.1 Renombre aplicado: `v15.0` → `v15.1` (decidido el 2026-11-19 22:36)

> **Decisión del usuario**: *"si el renombre provee ventajas a corto o largo
> plazo, se hace"*. Analizado y aplicado **sobre el contrato**, no sobre el
> código de otro agente.

**El análisis (por qué NO se renombra lo implementado):**

| Criterio | Renombrar las 20 rutas implementadas | Extender el contrato a la vocabulary existente |
|---|---|---|
| Coste | Tocar `webui.rs` (94 KB), `cockpit_v2.html` (97 KB), `webui_v2.css` (30 KB) y los 16 tests que comprueban que la UI llama exactamente a las rutas registradas | 0 ficheros de otros; sólo rutas **nuevas** (las que no existen) |
| Riesgo de pisado | **Alto**: son ficheros de otro agente (NP-3/NP-6) ⇒ exactamente el fallo que este contrato existe para evitar | Nulo: se **extiende** una familia existente, no se crea otra |
| Ganancia funcional | **Cero**: `/api/v2/dashboard` y `/api/v2/tasks` ya devuelven el agregado que la UI necesita (1 fetch en vez de 20: el motivo real de `API_V2.md` §1) | Alta: los agentes obtienen **recurso + filtros + paginación** sin duplicar vistas |
| Coste a largo plazo | Mantener **dos** vocabularios (`/api/v2/dashboard` + `/api/v2/tasks`) para lo mismo ⇒ el cliente tiene que saber cuál usar | **Un** vocabulario: `tasks`, `nodes`, `share`, `agents`, `certs`… |
| Veredicto | ❌ No compensa | ✅ **Aplicado** |

**Lo que se renombra (todo es camino del contrato, nada de código ajeno):**

| Antes (v15.0) | Ahora (v15.1) | Motivo |
|---|---|---|
| `/api/v2/board/tasks` (R-04..R-06) | `/api/v2/tasks`, `/api/v2/tasks/{id}` | La familia `tasks` **ya existe** (`GET /api/v2/tasks`); el board se distingue con `?scope=board` |
| `/api/v2/board/tasks/{id}/claim|report|heartbeat` (R-07..R-09) | `/api/v2/tasks/{id}/claim|report|heartbeat` | Sub-recursos del mismo recurso |
| `/api/v2/board/events` (R-10) | `/api/v2/events` | Extensión natural |
| `/api/v2/board/instances` (R-11) | `/api/v2/nodes` | **Ya existe** con el mismo contenido |
| `/api/v2/board/schema` (R-12) | `/api/v2/schema` | Recurso global, no del board |
| `/api/v2/board/sync` (R-13) | `/api/v2/nodes/sync` | Junto a `/api/v2/nodes/migrate` (ya existe) |
| `/api/v2/bus/send|broadcast|reply|messages|subscribe` (R-25..R-29) | `/api/v2/nodes/message|broadcast|reply|messages|subscribe` | La familia `nodes` **ya existe** y ya es el bus |
| `/api/v2/files/manifest|verify` y `/api/v2/files/{zone}/{name}` (R-30, R-33, R-34) | `/api/v2/share/manifest`, `/api/v2/share/manifests`, `/api/v2/share/verify`; `GET` de fichero = `/api/v2/share/get` (ya existe) | La familia `share` **ya existe** con verificación por sha256 |
| `/api/v2/metrics/autonomy` (R-22) | `/api/v2/metrics` | Un único sitio de métricas (`section` filtra) |
| `/api/v2/mcp/tools` (R-35) | **fusionado** en `/api/v2/routes` (ya existe): cada ruta publica su `route_id` y su tool MCP | No crear una segunda tabla de rutas |
| `/api/v2/mcp` (R-36) | **eliminado**: el endpoint MCP sigue siendo `POST /mcp` (JSON-RPC). Crear `/api/v2/mcp` sería duplicar el endpoint | Compatibilidad con `tools/mdmcp.py` y con el prompt del agente externo ya publicado |
| `/api/v2/agents` (R-14..R-16) | **sin cambios** | No colisiona con `nodes` (instancias ≠ agentes de software) |

**Lo que NO cambia:** el `route_id` es el **identificador estable** y no se
renombra (§2.1.1 regla EQ-3): `v2.tasks.list` sigue siendo `v2.tasks.list`
aunque el path haya cambiado. Un cliente que use `route_id` sobrevive al
próximo renombre; uno que use el path, no.

**Consecuencia práctica (recuento exacto de las 44 rutas):**

| Situación | Nº | Rutas |
|---|---|---|
| **Ya implementadas** (mismo path, mismo contenido) | **8** | R-04 `tasks`, R-11 `nodes`, R-23 `alerts`, R-25 `nodes/message`, R-32 `share/put`, R-34 `share/get`, R-35 `routes`, R-36 `mcp` |
| **Nuevas** que amplían una familia existente (hay que añadirlas al `match` de `webui.rs`) | **28** | R-01, R-02, R-03, R-05..R-10, R-12..R-24, R-26..R-31, R-33 |
| **Nuevas** de la capa de distribución (§9), que no existe en ninguna forma | **8** | R-37..R-44 |
| De las 28 nuevas, **9 se pueden resolver reutilizando la v1** en vez de escribir lógica nueva | 9 | R-07/R-08/R-09 (`POST /api/tool/task_queue`), R-17/R-18/R-19 (`POST /api/tool/agent`), R-24 (`POST /api/tool/alerts`), R-05 (`POST /api/v2/nodes/assign`) |

**Regla para el futuro (EQ-6):** una ruta nueva **extiende** una familia
existente si la hay; si no, se crea una familia **nueva** y se documenta aquí.
Nunca se crea una familia paralela para lo mismo (fue exactamente el error de
v15.0).

**Ley de propiedad**: `route_id` es la clave de la tabla. Dos agentes no pueden
implementar el mismo `route_id`. El `owner` indicado es el responsable de
**escribir el handler**; `webui.rs` es responsable del **cableado** (tabla de
rutas, auth, envelope, paginación, `X-Request-Id`) y **no** de la lógica de
dominio. Si un owner no entrega, la ruta responde `501` (AP-7) y el retraso es
visible en `GET /api/v2/health` (campo `routes_pending`).

**Lectura de la columna `owner`:** `fichero.rs` = ese módulo **escribe** el
handler. `webui.rs → otro.rs` = `webui.rs` **cablea** la ruta y delega la
lógica en `otro.rs`; sólo uno de los dos escribe lógica de negocio. Ninguna
ruta tiene dos escritores de lógica.

**Rutas heredadas** (v1, congeladas, sin owner nuevo): todo `/api/*` y
`/api/tool/*` de `cockpit_server.rs` siguen vivas tal cual. v2 **no** las
duplica: las llama por función, nunca por HTTP.

### 2.2 Envelope de error único (normativo)

> **v2.0 — ENVELOPE ADOPTADO (unificado con `webui.rs`, ya implementado)**
>
> El envelope **no se inventa**: se adopta el que ya está implementado y
> testado en `rust-core/src/webui.rs` (21:27, 16 tests) y documentado en
> `docs/API_V2.md` §5.1. Cambiarlo ahora rompería la UI v2
> (`web/cockpit_v2.html`, 97 KB, ya escrita) y los 16 tests. La regla del
> repo es "reutiliza o mufla" (`MASTER_PLAN.md` §1, causa 3): **no se
> reescribe lo que funciona**.

**Forma canónica de una respuesta v2 (v2.0):**

```json
{
  "ok": false,
  "error": "hash_mismatch",
  "expected": "9f2c…",
  "actual": "41ed…",
  "detail": "el contenido no coincide con el sha256 declarado: no se escribe nada",
  "request_id": "req-20261119T220000Z-000042-9f3a",
  "generated_at": "2026-11-19T22:14:03Z"
}
```

| Campo | Regla |
|---|---|
| `ok` | `true` = la operación se ejecutó; `false` = error. **Siempre presente.** |
| `error` | Código `snake_case` de la lista cerrada de abajo. **Es el único campo que el cliente debe ramificar.** |
| `detail` / `hint` / `reason` | Texto legible en español, ≤ 200 chars. **No se parsea.** |
| `expected` / `actual` | Sólo en errores de verificación (hash, tamaño). |
| `request_id` | **Aditivo** (no existía en v2.0): idéntico a la cabecera `X-Request-Id`. Debe llegar en toda respuesta. |
| `generated_at` | ISO-8601 UTC. Ya implementado en `webui.rs`. |

> **Regla de oro heredada de `docs/API_V2.md` §1 y vigente aquí**: *si un dato
> no existe, la respuesta lo dice*. No hay campos inventados, ni mocks, ni
> `0` para tapar un `null`: cada ausencia viene con su `reason` legible.
> Esto es incompatible con el `unwrap_or_default()` detectado por la crítica
> adversaria (`C-02`) y obliga a corregirlo.

**Divergencia conocida (queda registrada, no se maquilla):** la implementación
responde **HTTP 200 con `ok:false`** ante un error (p. ej. `unknown_route`),
mientras que este contrato pide el **status HTTP correcto** (401/403/404/409/
422/429). La lista de códigos con su HTTP es la **especificación objetivo**;
el estado actual es **200 + `ok:false`**. Corregirlo es trabajo del dueño de
`cockpit_server.rs` (no de este documento) y está registrado asímismo como
**`A-08`** en el registro de objeciones (§10.4). Hasta que se corrija, los
clientes **deben ramificar por `error`**, nunca por el status HTTP.

Códigos cerrados y su HTTP **objetivo**:

| `code` | HTTP | Cuándo |
|---|---|---|
| `unauthorized` | 401 | Falta la credencial (no hay `X-API-Key`; key no registrada). |
| `forbidden` | 403 | Credencial presente pero insuficiente (key inválida, rol sin permiso, falta `X-Seere-Auth-Token` en escritura, `zone=restricted`). |
| `not_found` | 404 | El recurso (tarea, artefacto, agente) no existe. |
| `method_not_allowed` | 405 | La ruta existe, el método no. Header `Allow` obligatorio. |
| `conflict` | 409 | Conflicto de estado (lease vencido, clave de idempotencia reutilizada con otro body, `schema` incompatible). |
| `unprocessable` | 422 | Body bien formado pero inválido (campo obligatorio ausente, enum inválido). Body con detalle por campo en `details.fields`. |
| `rate_limited` | 429 | Rate limit excedido. **Obligatorio**: header `Retry-After` en segundos + `details.retry_after_ms`. |
| `internal` | 500 | Fallo interno. `details` **no** filtra rutas de ficheros ni secrets; sólo `request_id`. |
| `not_implemented` | 501 | Ruta asignada sin handler (AP-7). |
| `timeout` | 504 | Timeout del handler (p. ej. `wait_s` del bus). |
| `stale_seq` | 409 | Certificado de nodo con `seq` ≤ el último conocido (§9). |
| `bad_signature` | 403 | La firma HMAC del `node cert` no valida (§9). |
| `cert_expired` | 409 | Se intenta asignar a un nodo cuyo cert ha caducado (§9). |
| `quarantined` | 409 | El nodo está en cuarentena por baja contribución (§9.7). |
| `no_candidate` | 409 | Ningún nodo puede tomar la clase pedida (§9). |
| `class_not_allowed` | 403 | La tarea pide una clase que el nodo no tiene (filtro duro, §9). |

> **Divergencia heredada (documentada a propósito)**: `cockpit_server.rs`
> responde `403 {"error":"unauthorized"}` cuando falta el token de escritura.
> v2 normaliza a `401`/`403` según la tabla de arriba. v1 **no cambia**
> (compatibilidad con la UI actual); la divergencia está anotada aquí para que
> nadie la "arregle" sin acordarlo.

### 2.3 Paginación, orden y `limit`

| Regla | Valor |
|---|---|
| `?limit=` | Por defecto **50**. Máximo **500**. `limit=0` ⇒ `422 unprocessable`. `limit>500` ⇒ **clampeo a 500** + `details.limit_clamped=true` (no es error: no romper clientes). |
| `?cursor=` | Opaco, `base64url` sin padding de `{"k":<última clave de orden>,"t":<ts>}` + HMAC corto. El cliente **no** lo interpreta. |
| Orden | **Estable y total**: `ORDER BY <clave_primaria> DESC, id DESC` (para eventos y tareas: `ts DESC, id DESC`). Sin orden estable el cursor no sirve. |
| Respuesta | `{"items":[...], "next_cursor": "<opaque>|null"}`. `next_cursor=null` ⇒ fin de la página. |
| Máximo de páginas | El cliente no debería pasar de 50 páginas por recurso; el servidor no lo impide (no es un error). |

### 2.4 Auth, 401 vs 403 vs 429, y el fin del 429 en cascada

**Orden de evaluación (obligatorio, el mismo en v1 y v2):**

```
1. Rate limit  → 429   (se evalúa PRIMERO: es la defensa más barata)
2. API key     → 401 (falta) / 403 (inválida)
3. Token       → 403 (falta, sólo en escritura)
4. Ruta        → 404 / 405 / 501
5. Body        → 422
```

> Nota de coherencia con `api_key.rs`: hoy `decide()` devuelve `Unauthorized`
> (falta key) y `Forbidden` (key incorrecta). La tabla de arriba **coincide**
> con esa distinción; lo que cambia en v2 es que la **falta de token** pasa
> de `403` a `403` con `code:"forbidden"` (antes `403` con
> `{"error":"unauthorized"}`). v2 no reinterpreta `401`/`403` de la key.

**Regla anti-cascada (nueva, obligatoria):**

| Regla | Detalle |
|---|---|
| AL-1 | **El cubo de rate limit se indexa por agente autenticado, no por IP**, cuando hay `X-API-Key` válida. Clave de cubo = `api_key_id` (hash del prefijo de la key, no la key). Sin key ⇒ clave de cubo = IP peer. |
| AL-2 | Motivo (medido, D-19/D-20): en el móvil **todo** el tráfico sale por `127.0.0.1`; navegador, túnel Cloudflare y agentes comparten IP ⇒ un cubo ⇒ 429 en cascada ⇒ los agentes abandonan. Con AL-1 el túnel, la UI y cada agente tienen cubo propio. |
| AL-3 | `Retry-After` **obligatorio** en el 429 y valor ≥ 1 s. Un 429 sin `Retry-After` está prohibido: fue lo que hizo que los agentes lo leyeran como caída. |
| AL-4 | Rutas exentas de rate limit: `/api/v2/health`, `/api/v2/version`, `/health`, `/metrics`. |
| AL-5 | Límite por defecto: `MEGADAEMON_RATE_MAX` (env), **600 req/min por agente** y **60 req/min por IP no autenticada**. El valor por defecto de 60 se conserva **sólo** para el tráfico no autenticado. |
| AL-6 | `MEGADAEMON_RATE_LOOPBACK=1` fuerza el límite también en loopback (comportamiento actual). Por defecto el loopback autenticado **no** se limita. |

**Doble auth en escritura** (ya vigente, no se cambia): `X-API-Key` +
`X-Seere-Auth-Token`. `X-Seere-Auth-Token` es obligatorio en todo `POST/PUT/DELETE`
de v2 salvo en la lista explícita de "lectura con efecto" (ninguna por ahora).

### 2.5 `X-Request-Id` e idempotencia

**`X-Request-Id`:**

| Regla | Detalle |
|---|---|
| XI-1 | Si el cliente envía `X-Request-Id`, el servidor lo **respeta** (trazabilidad extremo a extremo) y lo valida: 8-64 chars de `[A-Za-z0-9._:-]`. Si no valida ⇒ `422 unprocessable`. |
| XI-2 | Si falta, el servidor genera: `req-<YYYYMMDD>T<HHMMSS>Z-<contador 6>-<rand base32 4>`. |
| XI-3 | Aparece en: la cabecera de **toda** respuesta (2xx y error), el campo `error.request_id` y **una línea de log por petición** (`path`, `status`, `peer_ip`, `agent_id`, `duration_ms`, `request_id`). |
| XI-4 | Los logs del daemon **no** llevan el body de la petición (puede contener secretos). |

**Idempotencia en POST:**

| Regla | Detalle |
|---|---|
| ID-1 | Todo POST de v2 que cree estado requiere `Idempotency-Key` (8-128 chars). Ausente ⇒ `422 unprocessable` con `details.fields.idempotency_key`. |
| ID-2 | El servidor guarda `(key, method, path, body_sha256) → (status, body, headers)` en la tabla `idempotency` (§3.2), TTL **24 h**. |
| ID-3 | Misma key + mismo body ⇒ **replay** de la respuesta guardada, con header `Idempotent-Replay: true`. No se ejecuta dos veces. |
| ID-4 | Misma key + body **distinto** ⇒ `409 conflict`, `code:"idempotency_key_reuse"`. |
| ID-5 | La key es **por agente**: el namespace es `(agent_id, key)`. Dos agentes distintos pueden usar la misma key sin colisión. |
| ID-6 | `POST /api/v2/tasks/{id}/claim` es además **idempotente por naturaleza**: `won:false` si ya está reclamada. Reclamar dos veces ⇒ la segunda devuelve `409 conflict` con `details.owner` si el lease es de otro. |

### 2.6 MCP (JSON-RPC) ↔ REST: una sola fuente de verdad

**Relación normativa:**

```
        HTTP REST (/api/v2/...)            JSON-RPC 2.0 (POST /mcp)
                  │                                     │
                  │            webui::dispatch()        │
                  └──────────────►  ◄──────────────────┘
                                  │
                    ┌─────────────┴─────────────┐
                    │  tabla de rutas (route_id) │
                    │  + handlers de dominio     │
                    └───────────────────────────┘
```

| Regla | Detalle |
|---|---|
| MC-1 | `webui::dispatch(req) -> ApiResponse` es **la única** función que produce respuestas v2. El router REST la llama; el servidor MCP la llama. |
| MC-2 | `mcp_server.rs` **no contiene lógica de dominio**: traduce JSON-RPC ⇄ `ApiRequest`/`ApiResponse`. Si hace falta una tool nueva, se añade **una fila** a la tabla de rutas + **una fila** a la tabla de tools MCP. |
| MC-3 | Nombre de tool MCP = `v2_<dominio>_<acción>` (snake_case). Ejemplos: `v2_board_tasks_list` → R-04; `v2_bus_send` → R-25. |
| MC-4 | `GET /api/v2/routes` (R-35) devuelve el catálogo **con el `route_id` de cada tool**. Test de aceptación: para toda tool, `route_id` resuelve a una ruta existente. |
| MC-5 | Las tools legacy (las 26 verificadas en v13.3.0: `system_overview`, `root_exec`, `fs_read`, `fs_write`, `megadaemon_die_binwalk`, …) **se mantienen** tal cual, pero se registran en la tabla como `legacy` para que sean descubribles. No se reescriben en v15. |
| MC-6 | Un error de dominio devuelto en REST se traduce a error JSON-RPC: `{"jsonrpc":"2.0","id":N,"error":{"code":-32000,"message":"<error.message>","data":{"error_code":"<error.code>","request_id":"<id>"}}}` con el **mismo** `error.code` de §2.2. El `error_code` es lo que el agente ramifica; el HTTP status **no** viaja en JSON-RPC. |
| MC-7 | `/mcp` **no** se versiona (compatibilidad con `tools/mdmcp.py` y con el prompt del agente externo ya publicado). **No se crea** `/api/v2/mcp`: sería duplicar el endpoint MCP (v15.1). Un único `POST /mcp` que hace `dispatch()`. |
| MC-8 | Autenticación de `/mcp`: **idéntica** a REST (misma `api_key::decide()` + token). Sin excepciones. |

**Tabla de tools MCP v2 (declarativa, se implementa como dato, no como código):**

| Tool | `route_id` | Notas |
|---|---|---|
| `v2_board_tasks_list` | R-04 | |
| `v2_board_tasks_add` | R-05 | `Idempotency-Key` obligatorio |
| `v2_board_tasks_claim` | R-07 | devuelve `won` |
| `v2_board_tasks_report` | R-08 | |
| `v2_agents_list` | R-14 | |
| `v2_agents_register` | R-16 | |
| `v2_goals_create` | R-18 | delega en `agent_core::plan` |
| `v2_failures_list` | R-20 | libro de fallos |
| `v2_failures_create` | R-21 | nota estructurada |
| `v2_bus_send` | R-25 | |
| `v2_bus_broadcast` | R-26 | |
| `v2_bus_subscribe` | R-29 | |
| `v2_files_manifest` | R-30 | |
| `v2_files_push` | R-32 | |
| `v2_files_verify` | R-33 | |
| `v2_capabilities` | R-03 | |
| `v2_certs_publish` | R-37 | publica el `node cert` del emisor |
| `v2_certs_list` | R-38 | |
| `v2_offers_create` | R-41 | |
| `v2_offers_list` | R-42 | |
| `v2_contributions_list` | R-43 | ledger de contribución |
| `v2_placement_decide` | R-44 | "¿dónde va esta tarea?" |

### 2.7 Formato canónico de intercambio entre nodos (NDJSON + MANIFEST)

**Por qué NDJSON** (no JSON, no tar, no SQLite por red):

| Razón | Detalle |
|---|---|
| Streaming | El emisor **no** necesita el conjunto completo en RAM para escribirlo; el receptor lo lee línea a línea. Con 10,2/10,5 GB usados en el móvil (D-10), un `Vec<Value>` de un manifiesto grande es un riesgo directo de OOM. |
| Append-only / tolerante a cortes | Si el proceso muere a mitad, el fichero tiene líneas completas hasta el corte y **una** línea truncada al final, que se descarta. Un JSON array truncado **no** se puede leer. |
| Merge idempotente línea a línea | El merge por `sha256`/`id` (§8) se hace sobre líneas, sin parsear el documento entero. |
| Diffable y greppable | `grep '"zone":"public"' MANIFEST.ndjson` funciona. Con JSONSerializer no. |
| Reutiliza lo ya decidido | `tools/findings_from_manifest.py` **ya** consume `MANIFEST.ndjson` con una línea JSON por artefacto, y `findings.ndjson` con el mismo formato. Este contrato **no lo cambia**: lo formaliza. |
| Compatible con la restricción de 8 MB del transporte HTTP | Cada línea va en su propia petición/chunk. |

**Reglas del formato (normativas):**

| Regla | Detalle |
|---|---|
| ND-1 | UTF-8, saltos `\n` (LF), sin BOM, sin espacios finales. Una línea = un objeto JSON completo. |
| ND-2 | Sin `NaN`/`Infinity` (JSON no los tiene). Los enteros grandes van como **cadena** (`"1638297600000"`), para no perder precisión en JSON de doble. |
| ND-3 | Todo objeto lleva `ts` (ISO-8601 UTC, `YYYY-MM-DDTHH:MM:SSZ`) y `schema`. |
| ND-4 | Orden de claves: `schema`, `type`, `ts`, y luego el resto en orden alfabético. Determinista ⇒ el mismo contenido produce el mismo sha256. |
| ND-5 | Tope: **1 MB por línea**, **10.000 líneas por fichero**. Superado ⇒ `422 unprocessable`. |
| ND-6 | Las líneas de un `MANIFEST.ndjson` **no se editan**: si un artefacto cambia, se **añade** una línea nueva con otro `sha256` (§8.4). |

**Encabezado de fichero (primera línea, obligatoria en todo flujo de compartición):**

```json
{"schema":"mdshare/1","type":"header","ts":"2026-11-19T22:00:00Z","stream":"agent-firma-01","from":"samsung-s21-ultra@home","zone":"public","count":3}
```

**Línea de artefacto (una por artefacto):**

```json
{"schema":"mdshare/1","type":"artifact","ts":"2026-11-19T22:00:00Z","id":"art-0001","name":"app.apk","sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","size":12582912,"mime":"application/vnd.android.package-archive","zone":"public","path":"share/public/e3/e3b0c442.../app.apk","agent":"agent-re-01","note":"APK crudo de ChatGPT 2.47.10"}
```

**Línea de cierre:**

```json
{"schema":"mdshare/1","type":"end","ts":"2026-11-19T22:05:12Z","count":3,"total_bytes":12582912,"manifest_sha256":"<sha256 de las líneas artifact concatenadas>"}
```

**Reglas de cierre obligatorias:**

| Regla | Detalle |
|---|---|
| ND-7 | `manifest_sha256` = SHA-256 (hex) de la concatenación **exacta** de las líneas `type:"artifact"` (con su `\n`), en orden de emisión. Es lo que se verifica en destino (§8.3). |
| ND-8 | Un manifiesto **sin** línea `end` está **incompleto**: el receptor lo acepta como *parcial* (procesa lo que haya, marca los artefactos presentes) y **no** lo considera válido para "publicar". |
| ND-9 | Un artefacto cuyo `sha256` no cuadre **rechaza el artefacto entero**, no el manifiesto (§8.3). |

---

## 3. CONTRATO DE COLABORACIÓN ENTRE AGENTES

### 3.1 Roles

| Rol | Dónde vive | Puede | No puede |
|---|---|---|---|
| `primary` | `<state_dir>/instance.json` (`"role":"primary"`) — el **Samsung** | Decidir, repartir, arbitrar, aceptar/rechazar artefactos, promover/despromover instancias, migrar esquema | — |
| `worker` | `<state_dir>/instance.json` (`"role":"worker"`) — el **PC Linux** y los agentes externos | Reclamar tareas del tablero, ejecutar, reportar, publicar artefactos en `zone:public` | Cambiar el reparto, tocar el esquema del board, tocar ficheros de otro agente |
| `observer` | `<state_dir>/instance.json` (`"role":"observer"`) | Leer tablero, bus, métricas, libro de fallos | Escribir en el tablero, reclamar, publicar artefactos |

> Fuente: `coordination.rs::InstanceRole` (ya acepta `primary|maestra|master|
> coordinator`, `worker|esclavo|nodo|satellite`, `observer|observador`).
> Este contrato **no cambia** el enum; fija el comportamiento por rol.

**Reglas de rol (normativas):**

| # | Regla |
|---|---|
| CR-1 | Sólo el `primary` puede ejecutar `POST /api/v2/tasks` con `owner_hint` nulo, `promote_primary`, migraciones de esquema y `POST /api/v2/share/manifest` con `zone:public` de otro agente. |
| CR-2 | Un `observer` que hace POST recibe `403 forbidden` con `details.role="observer"`. |
| CR-3 | Un `worker` que reclama una tarea que ya tiene `owner` fijo para otro agente recibe `won:false` (no `403`): el tablero es el que arbitra. |
| CR-4 | El `primary` es **el único** que hace `git push` a `main`. Un worker nunca commitea al repo compartido (ver §3.4). |

### 3.2 El tablero

**Qué es:** una base SQLite **por instancia**, en `<state_dir>/board.sqlite`
(`rusqlite`, WAL). Es el estado compartido del enjambre: **el plan vive
aquí**, no en los chats.

**Dónde vive:**

| Nodo | Ruta | Rol |
|---|---|---|
| Samsung (primary) | `/data/local/tmp/megadaemon/board.sqlite` | **dueño** (única fuente de verdad) |
| PC Linux (worker) | `<state_dir>/board.sqlite` local, **sólo espejo de lectura** | espejo + outbound queue |
| Agente externo | **ninguno** | habla por HTTP contra el primary |

> **Prohibición explícita**: el `board.sqlite` **nunca** se comparte por NFS.
> Motivo medido (D-1): NFS v3 `nolock` + SQLite ⇒ `database is locked` con
> builds que ni se pisan. Un fichero SQLite sobre NFS **no** es un mecanismo
> de coordinación; el merge es por HTTP (§5.3).

**Esquema v2 del tablero** (`schema_version = 2` en la tabla `meta`):

```sql
-- Tablas heredadas de coordination.rs (v1) — NO se modifican
CREATE TABLE IF NOT EXISTS instances (
    id TEXT PRIMARY KEY, role TEXT NOT NULL, device TEXT NOT NULL DEFAULT '',
    platform TEXT NOT NULL DEFAULT '', version TEXT NOT NULL DEFAULT '',
    protocol INTEGER NOT NULL DEFAULT 1, capabilities TEXT NOT NULL DEFAULT '[]',
    state_dir TEXT NOT NULL DEFAULT '', last_seen INTEGER NOT NULL DEFAULT 0,
    registered_at INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY, title TEXT NOT NULL, owner TEXT, status TEXT NOT NULL DEFAULT 'queued',
    priority INTEGER NOT NULL DEFAULT 5, owner_hint TEXT, report TEXT,
    created_at INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL DEFAULT 0,
    lease_expires_at INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT, ts INTEGER NOT NULL, kind TEXT NOT NULL,
    actor TEXT NOT NULL, payload TEXT NOT NULL DEFAULT ''
);

-- Tablas nuevas de v2
CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY, value TEXT NOT NULL
);
-- meta: schema_version="2", protocol="1", created_at="<epoch>", primary_id="<id>"

-- Columnas nuevas sobre las tablas v1: SOLO ALTER TABLE ADD COLUMN (SC-4).
-- Necesarias para que R-05 (add_task con payload/lease/capability) y LS-6
-- (contador de intentos) tengan dónde escribir.
ALTER TABLE tasks ADD COLUMN payload TEXT NOT NULL DEFAULT '';
ALTER TABLE tasks ADD COLUMN lease_secs INTEGER NOT NULL DEFAULT 900;
ALTER TABLE tasks ADD COLUMN capability TEXT NOT NULL DEFAULT '';
ALTER TABLE tasks ADD COLUMN agent_id TEXT NOT NULL DEFAULT '';
ALTER TABLE tasks ADD COLUMN attempts INTEGER NOT NULL DEFAULT 0;
ALTER TABLE instances ADD COLUMN capacity INTEGER NOT NULL DEFAULT 2;

CREATE TABLE IF NOT EXISTS idempotency (
    agent_id TEXT NOT NULL DEFAULT '',
    key TEXT NOT NULL,
    method TEXT NOT NULL,
    path TEXT NOT NULL,
    body_sha256 TEXT NOT NULL,
    status INTEGER NOT NULL,
    response TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    PRIMARY KEY (agent_id, key, method, path)
);

CREATE TABLE IF NOT EXISTS failures (
    id TEXT PRIMARY KEY, ts INTEGER NOT NULL, kind TEXT NOT NULL,
    severity TEXT NOT NULL DEFAULT 'med', root_cause_hash TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'open', note TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS files (
    sha256 TEXT PRIMARY KEY, zone TEXT NOT NULL, name TEXT NOT NULL,
    size INTEGER NOT NULL, source_agent TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL, state TEXT NOT NULL DEFAULT 'published'
);

CREATE TABLE IF NOT EXISTS bus_messages (
    id TEXT PRIMARY KEY, topic TEXT NOT NULL, from_agent TEXT NOT NULL,
    to_agent TEXT NOT NULL DEFAULT '', kind TEXT NOT NULL, ts INTEGER NOT NULL,
    body TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_bus_topic ON bus_messages(topic, ts);
CREATE INDEX IF NOT EXISTS idx_failures_kind ON failures(kind, ts);
```

**Índice del reparto entre nodos (`<state_dir>/dist.sqlite`)**

La capa de distribución (§9) **no** toca `board.sqlite`: `coordination.rs` es
del dueño `coord` y añadirle tablas exigiría acuerdo. La distribución tiene
**su propio** fichero índice, con las mismas reglas (canónico NDJSON, índice
SQLite derivado, merge idempotente):

```sql
PRAGMA journal_mode=WAL;

-- Canonico: <state_dir>/certs.ndjson  (una linea por cert publicado)
CREATE TABLE IF NOT EXISTS certs (
    node        TEXT NOT NULL,
    seq         INTEGER NOT NULL,
    cert_hash   TEXT NOT NULL,
    rules_hash  TEXT NOT NULL DEFAULT '',
    classes     TEXT NOT NULL DEFAULT '[]',
    headroom    TEXT NOT NULL DEFAULT '{}',
    published_at INTEGER NOT NULL DEFAULT 0,
    expires_at  INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (node, seq)
);
CREATE INDEX IF NOT EXISTS idx_certs_hash ON certs(cert_hash);

-- Canonico: <state_dir>/contributions.ndjson  (una linea por aportacion)
CREATE TABLE IF NOT EXISTS contributions (
    id         TEXT PRIMARY KEY,
    node       TEXT NOT NULL,
    class      TEXT NOT NULL,
    units      REAL NOT NULL DEFAULT 0,
    kind       TEXT NOT NULL DEFAULT '',
    ts         INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_contrib_node ON contributions(node, ts);

-- Ofertas de capacidad libre (caducan)
CREATE TABLE IF NOT EXISTS offers (
    offer_id   TEXT PRIMARY KEY,
    node       TEXT NOT NULL,
    class      TEXT NOT NULL,
    units      REAL NOT NULL DEFAULT 0,
    expires_at INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_offers_class ON offers(class, expires_at);
```

> Regla de propiedad: `dist.sqlite` es un **índice derivado**. La fuente de
> verdad de los certificados es el flujo NDJSON `certs.ndjson` y el de las
> contribuciones es `contributions.ndjson` (append-only, estilo §4.2). Si el
> índice se corrompe, se reconstruye con `md-dist reindex` sin perder
> historia. Esto evita tocar el esquema de `coordination.rs` y mantiene el
> patrón "NDJSON canónico + índice reconstruible" que ya usa el libro de
> fallos.

> **Idempotencia de la migración**: `ALTER TABLE ... ADD COLUMN` **no** es
> idempotente en SQLite (falla si la columna ya existe). El migrador **debe**
> consultar `PRAGMA table_info(<tabla>)` y **sólo** ejecutar el `ALTER` de las
> columnas que falten. Prohibido ejecutar el bloque a ciegas (SC-4: migración
> aditiva, nunca destructiva).
>
> `CREATE TABLE IF NOT EXISTS` **sí** es idempotente y se puede repetir.

**Ciclo `add → claim → report`:**

| Paso | Llamada | Semántica | Garantía |
|---|---|---|---|
| 1. add | `POST /api/v2/tasks` (R-05) o `md-coord add "<título>" --priority N --owner <dominio>` | Crea la tarea `status='queued'`, `lease_expires_at=0`, `updated_at=now` | Idempotente por `Idempotency-Key` (ID-3) |
| 2. claim | `POST /api/v2/tasks/{id}/claim` (R-07) o `md-coord claim <id> --owner <agente> --lease 900` | `UPDATE tasks SET owner=?, status='claimed', lease_expires_at=now+lease WHERE id=? AND status='queued'` | **Atómico**: sólo un agente gana (`won:true`). Los demás reciben `won:false` |
| 3. report | `POST /api/v2/tasks/{id}/report` (R-08) o `md-coord report <id> --owner <agente> --ok "3 líneas"` | Fija `status='done'|'failed'`, guarda `report`, escribe evento | Idempotente: reportar dos veces la misma tarea con el mismo `owner` no cambia nada |
| 4. (opcional) heartbeat | `POST /api/v2/tasks/{id}/heartbeat` (R-09) | Renueva `lease_expires_at = now + lease` | Sólo el `owner` del lease |

**El lease (normativo):**

| Regla | Detalle |
|---|---|
| LS-1 | Duración por defecto del lease en el board: **900 s** (`coordination.rs::DEFAULT_LEASE_SECS = 900`). `lease_secs` configurable por tarea (mín. 60, máx. 86 400). |
| LS-2 | Un `worker` renueva su lease con heartbeat **cada `lease_secs/3`** (300 s por defecto). Un worker que no renueva **pierde** la tarea aunque su proceso siga vivo. |
| LS-3 | `mesh.rs` usa `DEFAULT_LEASE_SECS = 300` para **sus** tareas de malla. Las dos constants **no** se mezclan: **el board es la autoridad** sobre las tareas del board; las tareas internas de malla no se.|
| LS-4 | `reclaim_expired()` (ya implementado en `coordination.rs`) pone en `queued` toda tarea `claimed` con `lease_expires_at < now`, y **libera** `owner`. Se ejecuta en cada `heartbeat` del board y periódicamente (cada 60 s) en el primary. |
| LS-5 | Si el `owner` que expiró es un nodo **vivo**, el nodo recibe un evento `lease_revoked` y **abandona** la tarea en su siguiente ciclo. |

**Qué pasa si un worker muere:**

| Momento | Efecto | Detalle |
|---|---|---|
| t+0 | El proceso muere | La tarea queda `claimed` con `owner` muerto. Nadie se entera todavía. |
| t+lease | El lease caduca | `reclaim_expired()` la devuelve a `queued` (`owner=NULL`, `lease_expires_at=0`). Se escribe un evento `lease_expired`. **La tarea vuelve a la cola.** |
| t+lease | El nodo vuelve (heartbeat) | `register_instance`/`heartbeat` marca `last_seen`; si el worker tiene otra copia del board, su `merge_remote` (§5.3) **no** degrada un estado terminal, pero sí respeta el `queued` si `updated_at` es mayor. |
| t+2×lease | Nodo realmente muerto | La instancia queda `stale` (`last_seen` más viejo que el TTL) y `GET /api/v2/nodes` la marca `stale:true`. El primary puede reasignar sus tareas. |

**Reasignación y escalado:**

| Regla | Detalle |
|---|---|
| LS-6 | Una tarea que expira **más de 3 veces** (contador `attempts` en `tasks.payload` o columna futura) se marca `needs_primary`: el primary la revisa, la reescribe o la falla. Motivo: si falla siempre, el problema no es el worker. |
| LS-7 | El contador de intentos vive en la tarea (`meta`), no en el agente. Un agente que muere no pierde su historial de fallos. |

### 3.3 Identidad de agente y registro

**Formato de `agent_id`: `agent-<dominio>-<n>`**

| Regla | Detalle |
|---|---|
| ID-1 | `<dominio>` ∈ {`api`, `board`, `bus`, `files`, `firma`, `dex`, `delegate`, `re`, `npu`, `web`, `ops`, `signing`, `backup`, `intel`, `mobile`} — el **dominio de capacidad**, no el nombre de la sesión. |
| ID-2 | `<n>` es un entero ≥ 1, **único dentro del dominio** (`agent-firma-01`, `agent-firma-02`). La asignación la hace el primary (registro), no el agente. |
| ID-3 | El `agent_id` es **estable** entre sesiones y **no** contiene secretos ni identificadores de cuenta. |
| ID-4 | Un agente externo (fuera de la flota) usa el mismo formato con el prefijo `ext-`: `ext-androidre-01`. Esto lo distingue en el bus y en el board. |

**Registro: `<state_dir>/agents/<agent_id>.json`**

```json
{
  "schema": "mdagent/1",
  "agent_id": "agent-firma-01",
  "instance_id": "samsung-s21-ultra@home",
  "role": "worker",
  "capabilities": ["signing.cms", "signing.pdf", "pkcs11.dnie"],
  "owner_files": ["rust-core/src/signing.rs", "rust-core/tests/signing.rs"],
  "state_dir": "/data/local/tmp/megadaemon",
  "pid": 1234,
  "version": "13.4.0",
  "api_key_id": "k-7f3a",
  "last_seen": 1763592000,
  "contact": { "kind": "http", "url": "http://192.168.1.50:9090", "topics": ["board", "files"] }
}
```

| Regla | Detalle |
|---|---|
| RG-1 | El registro lo escribe el **primary** al dar de alta al agente (`POST /api/v2/agents/register`, R-16). El fichero es *append-mostly*: los campos `last_seen` los actualiza el propio agente. |
| RG-2 | `owner_files` es la **fuente de verdad del reparto de ficheros**: sustituye a la tabla en markdown de `COORDINATION.md`, que queda como registro histórico. |
| RG-3 | `api_key_id` es el **prefijo** del hash de la key (nunca la key). Permite mapear un 429/403 a un agente sin exponer la credencial. |
| RG-4 | Un agente cuyo `last_seen` sea mayor que `STALE_PEER_SECS * k` se considera `stale`. `mesh.rs` ya usa `STALE_PEER_SECS = 90`. |
| RG-5 | `contact` permite que un agente externo se suscriba al bus **sin** ser un proceso en la máquina del primary. |

### 3.4 Reglas de no pisado (normativas, aplicables a todo el enjambre)

| # | Regla | Motivo (medido) |
|---|---|---|
| NP-1 | **`git add <ruta exacta>`. Nunca `-A`, nunca `.`, nunca `-A` con `--exclude` inventado.** | D-11: 23 ficheros (~60.000 líneas) sin commitear. |
| NP-2 | **Un commit = un ámbito = un `owner`.** El mensaje empieza por `feat(v15):` / `fix(v15):` / `docs(v15):` y termina con `Co-Authored-By: Mavis <mavis@agy.internal>`. | Regla 3 de `MASTER_PLAN.md`. |
| NP-3 | **Nadie toca `rust-core/src/lib.rs`, `rust-core/src/main.rs`, `rust-core/Cargo.toml` salvo el primary o por acuerdo explícito registrado en el board** (tarea con `owner` = el primary y `payload.agreement = "<agent_id>"`). | Regla 2 de `MASTER_PLAN.md`; el registro en `lib.rs` es el punto de conflicto nº1. |
| NP-4 | **Prohibido `git checkout`, `git stash`, `git clean`, `git restore`, `git reset --hard` sobre el árbol compartido.** Si hay que descartar trabajo, **se borra el fichero a mano** (`rm`) y se commitea el borrado. | D-18: `checkout`/`stash`/`clean` pisó a 3 agentes. |
| NP-5 | **Prohibido `--force` en push.** Si el push no es fast-forward ⇒ avisar al primary, rebasing manual, sin machacar. | Regla 3 de `COORDINATION.md`; D-12 (divergencia main↔origin/main). |
| NP-6 | **El scope de un agente es su lista `owner_files`** (§3.3). Antes de editar: `git status --short` y comprobar que sólo aparecen rutas de tu lista. | D-16, D-17: ficheros declarados en `lib.rs` que no existían. |
| NP-7 | **Ninguna edición de `docs/COORDINATION.md` ni `docs/MASTER_PLAN.md` salvo el primary.** Los agentes anexan **una línea** al final del tablón (cremallero) y nada más. | Regla 7 de `MASTER_PLAN.md`. |
| NP-8 | **Este documento (`docs/CONCURRENCY_PLAN_V15.md`) sólo lo edita el primary o quien él designe.** Un agente que necesite cambiarlo abre una tarea en el board con la diff propuesta. | Este contrato es la referencia común; si cambia por debajo, dos agentes implementan cosas distintas. |
| NP-9 | **Un módulo entra en `lib.rs` sólo con su arnés `#[path]` verde** (`rust-core/tests/<modulo>.rs`). El fallo se ve en el módulo, no en el swarm. | D-15, D-14. |
| NP-10 | **Builds siempre por `tools/build-queue.sh`, con `CARGO_TARGET_DIR` en disco local.** | D-1, D-4, D-8; regla 5-bis. |

**Protocolo de conflicto (qué hacer si dos agentes quieren el mismo fichero):**

1. El segundo que lo detecta **para** y **no edita**.
2. Abre `POST /api/v2/tasks` con `owner_hint` = el nombre del fichero y `payload.dispute`.
3. Escribe **una línea** en `docs/COORDINATION.md` (cremallero) diciendo qué quiere y por qué.
4. El primary decide en el siguiente ciclo y lo deja en `owner_files` del registro de agentes (§3.3).

### 3.5 El bus de mensajes entre agentes (aún sin implementar)

**Primitivas exigidas (mínimo viable):**

| Primitiva | Firma | Semántica | Ruta |
|---|---|---|---|
| `send` | `send(to, topic, kind, body) -> message_id` | Point-to-point. El receptor es un `agent_id` o un `instance_id`. | R-25 |
| `broadcast` | `broadcast(topic, kind, body) -> {message_id, recipients}` | A todos los suscritos del topic. | R-26 |
| `reply` | `reply(in_reply_to, body) -> message_id` | Encadena: el mensaje original guarda `in_reply_to`. | R-27 |
| `subscribe` | `subscribe(topic, since, cursor, wait_s)` | Lee mensajes desde un cursor. `wait_s=0` ⇒ paginado JSON; `wait_s>0` ⇒ **long-poll** que devuelve NDJSON y acaba como mucho en `wait_s`. | R-28/R-29 |
| `ack` | (implícito) | **At-least-once**: el receptor deduplica por `message_id` (ver NP de idempotencia). No hay ack explícito en v1 del bus: el cursor del suscriptor hace de ack. | — |

**Requisitos no funcionales:**

| # | Requisito | Objetivo | Cómo se mide |
|---|---|---|---|
| BU-1 | Latencia de entrega `send → visible en el suscriptor` | **p95 ≤ 250 ms** en LAN; **p99 ≤ 1 s**. | P-2 (pendiente de medición). Objetivo de diseño, no hecho medido. |
| BU-2 | El emisor **nunca** bloquea más de **5 s** | El `send` es *fire-and-forget* sobre el board del primary. | Test: `send` con primary saturado devuelve en < 5 s con `202`-equivalente (`accepted:true`). |
| BU-3 | Entrega **at-least-once** + deduplicación por `message_id` | Duplicados tolerados. | Test: reenviar el mismo `message_id` no duplica efectos. |
| BU-4 | Retención de mensajes | **72 h** en `bus_messages`; luego `prune`. | `md-bus prune` idempotente. |
| BU-5 | Tamaño máximo de mensaje | **256 KB** (base64 de un artefacto **no** viaja por el bus: va por `POST /api/v2/share/put`). | `413` con `code:"unprocessable"` (el `413` no existe en la lista cerrada: se usa `422 unprocessable` con `details.max_bytes`). |
| BU-6 | Backpressure | Un suscriptor lento **no** bloquea al emisor: el bus guarda en `bus_messages` y el cursor decide. | Test de carga. |

**Por qué HTTP + NDJSON basta (y por qué no MQTT):**

| Criterio | HTTP + NDJSON | MQTT (broker) |
|---|---|---|
| RAM en el móvil | 0 MB extra (el daemon ya habla HTTP) | +5-15 MB de broker, y el móvil tiene **~0,3 GB libres** (D-10) |
| Procesos extra | 0 | 1 proceso más = 1 forma más de fallo (ya hubo: el puente murió 3× en 30 min, D-21) |
| Puertos | 0 nuevos (AP-9) | 1+ nuevo (1883) |
| Semántica de entrega | at-least-once + dedup → **suficiente** para este sistema | QoS 0/1/2 aporta poco aquí: no hay broker de servicio crítico |
| Reusa lo existente | `ureq` ya en el árbol; NDJSON ya usado por `sync/` y `mesh_journal.ndjson` | Dependencia nueva, camino de artefactos nuevo (§1: "reutiliza o mufla") |
| Latencia | p95 250 ms alcanzable en LAN con long-poll | Mejor, pero no es el cuello de botella real |
| Operación | Un solo binario `megadaemon-core` | Un segundo binario que hay que instalar, supervisar y actualizar en el móvil |

> **Conclusión operativa**: el bus es **una tabla en el board del primary**
> (`bus_messages`) + 4 rutas HTTP. Si el día que haga falta un broker real,
> la migración es cambiar `mesh.rs` por detrás de las mismas 4 rutas: el
> contrato no cambia. **MQTT se descarta por ahora; es una decisión de §7.4.**

---

## 4. SISTEMA AGÉNTICO LOCAL DE AUTOMEJORA

### 4.1 Qué deja de ser "un triste chatbot"

Un chatbot responde a un prompt y forgets. Un **sistema agéntico** tiene:
**estado propio** (tablero, libro de fallos, métricas), **objetivo propio**
(goals que se degradan en tareas), **memoria operativa** (qué falló y qué se
hizo), y **políticas que cambian** con la evidencia (pesos de prioridad,
enrutado de cuentas, prioridades de tarea).

**El ciclo (normativo):**

```
  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐
  │  OBSERVACIÓN │──►│  DIAGNÓSTICO │──►│    ACCIÓN    │
  │  (hechos)    │   │  (causa)     │   │  (ejecutar)  │
  └──────▲───────┘   └──────────────┘   └──────▲───────┘
         │                                      │
  ┌──────┴───────┐   ┌──────────────┐   ┌──────┴───────┐
  │  APRENDIZAJE │◄──│  MEDICIÓN    │◄──┤  AJUSTE      │
  │  (patrones)  │   │  (efecto)    │   │  (políticas) │
  └──────────────┘   └──────────────┘   └──────────────┘
```

| Paso | Qué hace | Módulo (fichero) | Salida |
|---|---|---|---|
| 1. Observación | Recoge lo que pasó: alertas, fallos de tarea, journal del mesh, errores de tool, estado del sistema | `alerts.rs` (reglas), `task_scheduler.rs` (`journal_json`), `mesh.rs` (`mesh_journal.ndjson`), `api_key.rs` (contadores) | Eventos brutos |
| 2. Diagnóstico | Clasifica el fallo: tipo (`kind`), causa raíz, entorno, severidad | **nuevo** `autonomy.rs::classify()` | Nota estructurada (§4.2) |
| 3. Acción | Ejecuta: reencola, cambia el enrutado, regenera un manifiesto, ajusta prioridades | `agent_core.rs::plan()/execute()`, `task_scheduler.rs::run_due()` | Tareas ejecutadas |
| 4. Medición | Mide el efecto de la acción: ¿el fallo se repitió? ¿el tiempo bajó? | `autonomy.rs::metrics()` | Métricas (§4.6) |
| 5. Aprendizaje | Extrae **patrones** de las notas: ≥2 ocurrencias con la misma causa raíz ⇒ candidato | `autonomy.rs::mine_patterns()` | Candidatos de ajuste |
| 6. Ajuste | Aplica el patrón al sistema: pesos, prioridades, enrutado | `delegate.rs::adjustments()/apply_adjustments()`, `mesh.rs::learning_notes()`, `agent_core.rs` (tabla de prioridades) | Estado del sistema cambiado |

> **El paso 5→6 es el que convierte esto en "auto"mejora**: sin él, el sistema
> mide y no cambia. Con él, el cambio es *automático* pero **acotado** (§4.5).

### 4.2 El libro de fallos (contrato de datos)

**Qué es:** un registro **estructurado** (no prosa) de cada fallo. Una nota
por línea, NDJSON, append-only.

**Dónde se guarda:**

| Artefacto | Ruta | Rol |
|---|---|---|
| **Canónico** | `<state_dir>/failures.ndjson` | Una nota por línea. Append-only. **Fuente de verdad.** |
| Índice | tabla `failures` de `board.sqlite` | Índice derivado para filtrar rápido. Se puede reconstruir: `md-fail reindex`. |
| Export | `<state_dir>/failures/<yyyy-mm>.ndjson` | Segmentado por mes, para compartir sin mandar un fichero gigante. |

**Formato de una nota (campos cerrados):**

```json
{
  "schema": "mdfail/1",
  "id": "fail-20261119-000123",
  "ts": "2026-11-19T22:00:00Z",
  "kind": "build.nfs_artifact",
  "symptom": "rust-lld: cannot open ...rcgu.o (failed to copy bitcode to object file)",
  "env": {
    "node": "samsung-s21-ultra@home",
    "instance_id": "samsung-s21-ultra@home",
    "device": "samsung-s21-ultra",
    "platform": "android-arm64",
    "version": "13.4.0",
    "protocol": 1,
    "agent_id": "agent-ops-01",
    "task_id": "t-018",
    "tool": "build-queue.sh",
    "route_id": null
  },
  "root_cause": "CARGO_TARGET_DIR estaba dentro de /workspace (NFS v3 nolock): los .rlib no se escriben",
  "root_cause_hash": "rc-9f3a1c",
  "fix": {
    "kind": "config",
    "description": "CARGO_TARGET_DIR=/run/agent-runtime/md/target (ext4 local) + tools/build-queue.sh",
    "artifact": "tools/build-queue.sh",
    "commit": "6df3938"
  },
  "verification": {
    "method": "measurement",
    "evidence": "0 errores de link; 1 .rlib producido en 42 s",
    "result": "pass"
  },
  "severity": "high",
  "tags": ["nfs", "build", "toolchain"],
  "recurrence": 3,
  "status": "verified"
}
```

**Campos y reglas:**

| Campo | Regla |
|---|---|
| `schema` | `mdfail/1`. Un cambio incompatible ⇒ `mdfail/2` y el lector acepta ambos. |
| `id` | `fail-<yyyymmdd>-<contador 6>`; único. |
| `kind` | Taxonomía cerrada (abajo). Un fallo sin `kind` conocido ⇒ `kind:"unknown"`. |
| `symptom` | **El mensaje/error observado**, literal. ≤ 400 chars. Sin interpretación. |
| `env` | Contexto. `node` = `instance_id`. `route_id` si el fallo vino de una ruta v2. |
| `root_cause` | **Causa**, no síntoma. ≤ 400 chars. |
| `root_cause_hash` | Hash corto y **estable** de `root_cause` normalizado (minúsculas, sin puntuación, palabras vacías fuera). Es la clave de agrupación del patrón. |
| `fix.kind` | `config` \| `code` \| `policy` \| `routing` \| `manual` \| `none` (sin corrección aún). |
| `fix.commit` | SHA del commit que lo arregla, si aplica. |
| `verification.method` | `test` \| `measurement` \| `manual` \| `none`. |
| `verification.result` | `pass` \| `fail` \| `pending`. **`pending` no cuenta como verificado** (§4.6). |
| `severity` | `low` \| `med` \| `high` \| `critical`. |
| `recurrence` | Nº de veces seen el mismo `root_cause_hash`. Lo incrementa `autonomy.rs` al agrupar. |
| `status` | `open` \| `fixed` \| `verified` \| `regressed`. `verified` exige `verification.result == "pass"`. |

**Taxonomía cerrada de `kind` (v1):**

| `kind` | Significado |
|---|---|
| `build.nfs_artifact` | Artefactos que no se escriben por NFS |
| `build.compile` | Error de compilación (E0xxx, E0425, etc.) |
| `build.link` | Error de enlazado |
| `build.dep` | Dependencia faltante / índice de crates |
| `runtime.crash` | El proceso murió |
| `runtime.port_conflict` | EADDRINUSE |
| `runtime.oom` | Proceso terminado por falta de memoria |
| `net.rate_limited` | 429 |
| `net.unreachable` | Connection refused / timeouts |
| `net.proxy_5xx` | 502/503/504 del túnel |
| `auth.rejected` | 401/403 inesperados |
| `git.conflict` | Merge conflict / fichero pisado |
| `git.diverged` | Push no fast-forward |
| `board.lease_expired` | Tarea perdida por lease |
| `files.hash_mismatch` | Artefacto con sha256 incorrecto |
| `files.duplicate_conflict` | Conflicto de versión (§8.4) |
| `delegate.account_burned` | Cuenta de provider agotada/quemada |
| `provider.quality_drift` | Degradación de calidad de un provider |
| `device.dozed` | Doze/batería/thermal |
| `unknown` | Sin clasificar |

**Cómo se consulta:**

| Vía | Comando | Uso |
|---|---|---|
| HTTP | `GET /api/v2/failures?kind=net.rate_limited&since=<epoch>&limit=100` (R-20) | UI y agentes |
| CLI | `md-fail list --kind <kind> --since <ts>` | operador |
| Fichero | `jq -c 'select(.kind=="net.rate_limited")' <state_dir>/failures.ndjson` | análisis ad-hoc |
| Índice | `SELECT kind, COUNT(*) FROM failures WHERE status!='verified' GROUP BY kind;` | métrica de §4.6 |

> `jq` en el móvil no está garantizado: el comando de fichero es para el PC.
> En el móvil, la vía es siempre HTTP.

### 4.3 Del libro de fallos a los ajustes (el bucle)

| Patrón detectado | Ajuste | Destino | Automático |
|---|---|---|---|
| ≥2 notas con el mismo `root_cause_hash` en 24 h | Bajar la prioridad de las tareas de ese `kind` en `task_scheduler` | `task_scheduler` (pesos por `kind`) | Sí |
| ≥3 notas `delegate.account_burned` del mismo provider en 1 h | Deshabilitar la cuenta y recalcular el presupuesto | `delegate.rs::adjustments()` (acción `disable`) | Sí |
| ≥2 notas `provider.quality_drift` del mismo modelo | Bajar el peso del modelo en el enrutado | `delegate.rs` (EWMA de calidad por modelo) | Sí |
| `files.hash_mismatch` ×2 con la misma fuente | Marcar la fuente como no fiable y exigir reenvío | `file_share.rs` (estado `quarantined`) | Sí |
| `net.rate_limited` ×3 en 10 min | Subir el límite del cubo del agente que satura (hasta el máximo configurado) y avisar | `api_key.rs` (bucket del agente) | Sí, **acotado** |
| `git.conflict` sobre un fichero | **Ninguno**: requiere decisión humana de propiedad | — | **No** |
| `runtime.oom` en el móvil | Mover el trabajo pesado al PC | `board` (nueva tarea para el worker) | Sí |

**Cómo se concretan los ajustes hoy (mecanismos existentes, no nuevos):**

| Mecanismo | API existente | Qué aporta |
|---|---|---|
| `delegate::adjustments()` | `rust-core/src/delegate.rs:1378` (lista) y `:1492` (`apply_adjustments`, que **valida** antes de aplicar) | Ajustes accionables por cuenta, con `reason` legible |
| `mesh::learning_notes()` | `rust-core/src/mesh.rs:1063` | Notas legibles: fallo dominante por tipo de tarea, rendimiento por agente |
| `agent_core` | `rust-core/src/agent_core.rs` (`plan`, `execute`, `is_known_kind`) | Lista blanca de `kind`s y prioridades por defecto |

**Regla de acoplamiento (importante):** el libro de fallos **no** modifica
`delegate.rs` ni `mesh.rs` directamente. Produce una lista de
`delegate::Adjustment` y la aplica vía `apply_adjustments()` (que **valida**:
`apply_adjustments_rechaza_invalidos_sin_tocar_nada` ya es un test verde en
`delegate.rs`). Así, un patrón erróneo **no puede** corromper el enrutado: la
aplicación valida y rechaza.

### 4.4 Automatización: qué sí y qué no

| Regla | Detalle |
|---|---|
| AU-1 | **Todo lo reversible y acotado se automatiza.** Reversible = deshacible con el estado anterior disponible y sin coste externo irreversible. |
| AU-2 | **Todo lo irreversible exige humano.** Irreversible = borra datos de terceros, gasta dinero, expone credenciales, cambia la identidad de una instancia, o toca el exterior (firma, envío, compra, publicación). |
| AU-3 | **El flag `confirm: true` es el frontera.** Todo handler que lo acepte queda **fuera** de la autonomía: la acción se *prepara*, se registra en el board como `needs_confirm`, y **espera**. |
| AU-4 | La autonomía **nunca** invoca `root_exec` con `confirm`, `fs_write` sobre `/system`, ni `git push`. |
| AU-5 | La autonomía **nunca** altera `owner_files` de otro agente ni el estado `primary` de una instancia. |
| AU-6 | **Cadencia máxima:** la autonomía se ejecuta como máximo **1 vez cada 60 s** por instancia y **como máximo 5 acciones por ciclo**. Si hay más candidatos, se ordenan por `(severidad × recurrence)` y se deja el resto para el ciclo siguiente (con `status:"pending"`). Motivo: el móvil va justo (D-10) y un bucle agresivo lo tumba. |
| AU-7 | **Trazabilidad total:** cada acción automática escribe (a) una nota en `failures.ndjson` con `fix.kind:"policy"` y `verification.method:"none"` hasta que se verifique, (b) un evento en la tabla `events` del board, y (c) una línea en el cremallero de `docs/COORDINATION.md`. |

**Lista explícita de lo que NO se automatiza (decisiones irreversibles):**

| Acción | Por qué no |
|---|---|
| `root_exec` con `confirm: true` | Ejecuta como root real en el dispositivo |
| Instalar/desinstalar apps, Magisk, paquetes | Afecta al dispositivo del usuario |
| Firmar con clave privada (`.p12`, DNIe) | Irreversible criptográficamente y con valor legal |
| Rotar/revocar keys (`megadaemon_api.keys`, token de GitHub) | Deja fuera a la flota; ya pasó (token expuesto en el chat) |
| Enviar algo al exterior (email, push, webhook) | Coste y efecto en terceros |
| Borrar artefactos o tareas | Destruye evidencia |
| `git push`, `git commit`, cambios de `owner_files` | La regla NP-3/NP-4 lo prohíbe a propósito |
| Cambiar el `primary` | Afecta a todo el enjambre |
| Migrar el esquema del board | Afecta a todas las instancias |

### 4.5 Verificación de las correcciones

| Regla | Detalle |
|---|---|
| VF-1 | Una nota con `fix.kind != "none"` **nunca** pasa a `status:"verified"` sin `verification.result == "pass"`. |
| VF-2 | `verification.method` ∈ `test` \| `measurement` \| `manual`. `manual` cuenta como verificado sólo si `evidence` referencia un artefacto concreto (commit, log, captura). |
| VF-3 | `status:"regressed"` = el mismo `root_cause_hash` volvió a aparecer **después** de estar `verified`. Se incrementa `recurrence` y se reabre. |
| VF-4 | El **ratio de correcciones verificadas** (§4.6) es la métrica que decide si la autonomía se activa o se degrada a "sólo propone". |

### 4.6 Métricas del propio sistema

Se exponen en `GET /api/v2/metrics` (R-22) y se calculan sobre
`<state_dir>/failures.ndjson` + `board.sqlite`.

| Métrica | Definición exacta | Fuente | Objetivo (diseño) |
|---|---|---|---|
| M-1 `failure_rate_by_kind` | `count(notas de kind K en ventana W) / W_horas` | `failures.ndjson` | — (se compara contra la línea base) |
| M-2 `failure_rate_ratio` | `count(K) / count(todas)` | ídem | — |
| M-3 `mttr_s` (tiempo de resolución) | `median(nota.verification.ts - nota.ts)` de las notas con `status:"verified"`. La ventana por defecto: 7 días | `failures.ndjson` | — |
| M-4 `verified_fix_ratio` | `count(status="verified") / count(fix.kind != "none")` | ídem | **≥ 0,7** |
| M-5 `open_failures` | `count(status IN ("open","fixed"))` | ídem | — |
| M-6 `provider_drift` | Por proveedor/modelo: `EWMA(calidad)` y su pendiente en 24 h | `delegate.rs` (EWMA de calidad) + `pricing_ledger` | — |
| M-7 `task_retry_rate` | `count(tareas re-encoladas por expiración de lease) / count(tareas cerradas)` | `board.sqlite` (`events` con `kind="lease_expired"`) | — |
| M-8 `autonomy_actions` | Acciones automáticas aplicadas por ciclo y por día | `events` del board | — |
| M-9 `error_rate_by_route` | Errores por `route_id` y por `agent_id` | log con `X-Request-Id` (XI-3) | — |

**Reglas sobre las métricas:**

| # | Regla |
|---|---|
| MT-1 | Ninguna métrica se publica sin su **denominador**. `{failure_rate_by_kind: {"build.nfs_artifact": 2.1}}` sin `window_hours` es inválido. |
| MT-2 | Ninguna métrica se estima: si no hay datos, el valor es `null` y no `0`. |
| MT-3 | Las métricas se calculan **en el PC** cuando el dataset lo permita (el móvil no debe recomputar histogramas grandes). El primary expone el *endpoint*, el worker puede recalcular y reportar. |

---

## 5. DISEÑO DEL SISTEMA DISTRIBUIDO ("comunismo algorítmico")

### 5.1 Topología

```
                        ┌─────────────────────────────────────────┐
                        │        PRIMARY — Samsung S21 Ultra      │
                        │  (root, Magisk, Android 14, ~0,3 GB lib.)│
                        │                                         │
                        │  ┌───────────────┐  ┌────────────────┐  │
                        │  │ board.sqlite  │  │ bus_messages   │  │
                        │  │ tasks/inst/   │  │ + failures     │  │
                        │  │ events/files  │  │ + idempotency  │  │
                        │  └───────┬───────┘  └────────┬───────┘  │
                        │          │                   │          │
                        │  ┌───────▼───────────────────▼───────┐  │
                        │  │   HTTP :9090 (+ alias)  /api/v2    │  │
                        │  │   POST /mcp (JSON-RPC)             │  │
                        │  │   POST /api/v2/certs/publish        │  │
                        │  └───────┬──────────────────┬─────────┘  │
                        └──────────┼──────────────────┼────────────┘
                                   │                  │
                    ┌──────────────▼──┐        ┌──────▼───────────────────┐
                    │ WORKER — PC     │        │ ext-androidre-01         │
                    │ Linux           │        │ (agente externo,         │
                    │                 │        │  fuera de la LAN)        │
                    │ - lee el board  │        │ - sólo HTTP + manifests  │
                    │ - ejecuta lo    │        │ - propone, no commitea   │
                    │   pesado        │        └──────────────────────────┘
                    │ - build ARM64   │
                    │ - DEX/firma RE  │
                    │                 │
                    │ publica su      │
                    │ node cert ──────┼──► (primary lo usa para repartir)
                    └─────────────────┘

   Flujo de "cada nodo aporta" (§9):

   cada nodo ──node cert──► primary ──tarea+sufijo──► nodo que la puede hacer
        ▲                      │                              │
        │                      ▼                              ▼
        └────resultado verificado── bus/files ──────► contribution ledger
```

**El ciclo de contribución (§9):** cada nodo **publica un `node cert`**
(capacidades + holgura real + reglas locales aplicadas) → el primary **coloca**
cada tarea en el nodo que la puede hacer y que la **quiere** hacer ahora →
el nodo ejecuta y reporta → el resultado se verifica → el nodo **acumula
contribución** en el ledger. Un nodo que sólo consume y nunca aporta pierde
prioridad (§9.7).

**Reglas de topología:**

| # | Regla |
|---|---|
| TP-1 | **Un `primary` y sólo uno.** Se detecta por `meta.primary_id` en el board y por `"role":"primary"` en `instance.json`. Si dos se declaran primary, el que tenga el `last_seen` más antiguo gana y el otro se degrada a `worker` con un evento `primary_conflict`. |
| TP-2 | Los workers **no** tienen copia de escritura del board. |
| TP-3 | Los agentes externos **no** tienen fichero de estado en la máquina del primary: su estado es el bus + los manifiestos. |
| TP-4 | Todo el tráfico es **HTTP/1.1 sobre el mismo listener**; no hay puertos nuevos (AP-9). |
| TP-5 | La topología es de **2 niveles**, no una malla completa: primary ↔ workers ↔ externos. Una malla N² no cabe en un teléfono. |

### 5.2 Reparto: qué tarea va a qué nodo

**Algoritmo (determinista, ejecutable) — versión v15 con clases y créditos:**

```
asignar(tarea):
  1. FILTRAR candidatos:
       nodos con instance.role ∈ {worker}
       AND instance.last_seen > now - STALE_PEER_SECS * 2              (vivo)
       AND (tarea.capability ∈ nodos.capabilities OR tarea.capability == "")
       AND nodo.carga_actual < nodo.capacidad
       AND cert(nodo).classes contiene tarea.class                     (§9.2)
       AND cert(nodo).accepts contiene tarea.class                     (§9.4)
       AND cert(nodo) no está expirado (ttl) ni en cuarentena
  2. ORDENAR por la tupla:
       (a) prioridad de la tarea                (mayor = antes)
       (b) crédito de contribución del nodo     (mayor = primero: quien más
                                                 ha aportado, aporta primero)
       (c) holgura del nodo para esa clase      (mayor = mejor)
       (d) carga actual normalizada             (menor = mejor)
       (e) menos tareas abiertas del nodo        (menor = mejor)
       (f) agent_id (desempate lexicográfico)    (determinista)
  3. Si no hay candidatos: la tarea queda 'queued' y se emite el evento
     'no_candidate' (con los motivos de descarte: 'stale', 'no_capability',
     'no_class', 'not_accepted', 'overloaded', 'quarantined').
  4. Si el nodo elegido no reclama en 60 s: la tarea vuelve a 'queued' y se
     recalcula con el mismo orden (credito incluido).
```

**Implementación:** el reparto lo hace **el primary**; los workers **no**
compiten por el mismo trabajo de forma implícita. El `claim` sigue siendo
atómico (§3.2), de modo que un worker puede **adelantarse** al reparto y
reclamar por su cuenta (`won:true`) sin romper nada.

**Las clases y el crédito son lo que convierte esto en "cada nodo aporta"**
(§9). El desempate (b) es intencionado: un nodo que ha aportado mucho recibe
antes la próxima tarea, y uno que sólo consume se va al final de la lista. No
es un bloqueo: es un orden. El bloqueo real (cuarentena) es §9.7 y exige
desahogo medido, no una regla blanda.

**Carga y capacidad de un nodo:**

| Métrica | Definición | Origen |
|---|---|---|
| `load` | `run_queue` del `task_scheduler` (tareas `running`) + nº de leases activos | `task_scheduler.rs::stats()` |
| `capacity` | `max_concurrency` declarado por la instancia en `instance.json` (por defecto 2 en el móvil, 4 en el PC) | `instance.json` |
| `capabilities` | Lista de `kind`s que el nodo sabe ejecutar (`capability.rs::capabilities_for_module`) | `instance.json` |
| `classes` | Clases de trabajo que el nodo **puede** hacer (filtro duro) | `node cert` (§9.3) |
| `accepts` | Clases que el nodo **quiere** hacer ahora, según sus reglas locales (filtro duro) | `node cert` (§9.3) |
| `headroom` | Holgura real por clase (RAM libre, CPU libre, temperatura, red) | `node cert` (§9.3) |
| `credit` | Unidades de contribución acumuladas en la ventana | `contributions.ndjson` (§9.7) |

**Si un nodo no responde:**

| Situación | Detección | Acción |
|---|---|---|
| Processo muerto, lease activo | `lease_expires_at < now` | `reclaim_expired()` → `queued`; otro worker lo toma (§3.2) |
| Nodo caído (sin heartbeat) | `last_seen` más viejo que `STALE_PEER_SECS*2` (180 s) | Se marca `stale:true` en `GET /api/v2/nodes`; se le retiran las tareas abiertas tras la expiración de su lease |
| Nodo lento (responde tarde) | `duration_ms` de la petición > umbral | El reparto lo penaliza (`load` sube); no se le asignan más tareas |
| Nodo corrupto (schema incompatible) | `protocol < MIN_SUPPORTED_PROTOCOL` o `schema > schema_local` | Se marca `incompatible` y **no** se le da trabajo (§5.4) |

### 5.3 Sincronización y convergencia

**Mecanismo:** `POST /api/v2/nodes/sync` (R-13) con la instantánea completa
(ya implementado en `coordination.rs::export_for_sync()` /
`merge_remote()` / `sync_with_peer()`).

**Reglas de merge (heredadas, se mantienen):**

| # | Regla |
|---|---|
| MG-1 | **Tareas** por `id`: gana el `updated_at` mayor. |
| MG-2 | Un estado **terminal** (`done`/`failed`) **nunca** degrada a `queued`. |
| MG-3 | **Instancias** por `last_seen` máximo. |
| MG-4 | **Eventos** deduplicados por `(ts, kind, actor, payload)`. |
| MG-5 | Coste: un `SELECT`/`INSERT` por fila, **cero recomputo**. |

**Reglas nuevas de v15:**

| # | Regla |
|---|---|
| MG-6 | El merge de **`files`** es por `sha256` (clave primaria): insertar un `sha256` ya presente es un **no-op**. Nunca sobrescribe. |
| MG-7 | El merge de **`bus_messages`** es por `id`: insertar un `id` conocido es no-op. |
| MG-8 | El merge de **`failures`** es por `id`; si el `id` existe con `status` menos avanzado, gana el estado más avanzado (`open` < `fixed` < `verified`), y `regressed` gana a todos. |
| MG-9 | El merge de **`idempotency`** es por `(agent_id, key, method, path)`: **nunca** se reescribe una entrada ya presente (la primera gana). |
| MG-10 | El merge es **idempotente y conmutativo en el mismo sentido**: aplicar el mismo snapshot dos veces no cambia el estado; aplicar A y luego B da el mismo resultado que B y luego A (para el mismo `updated_at`). |

**Push/pull:**

| Regla | Detalle |
|---|---|
| MG-11 | Los workers hacen **push** al primary en su latido (cada 30 s) y **pull** del snapshot del primary. El primary **nunca** initiates la conexión a un worker salvo para un `GET /api/v2/health` de sonda. |
| MG-12 | Si el primary está caído, los workers **siguen trabajando** con su copia local en modo lectura, acumulan su cola de salida y la vacían al volver. Nada se pierde. |

### 5.4 Migración de instancias y versionado de esquema

**Versionado:**

| Elemento | Campo | Valor actual | Regla |
|---|---|---|---|
| Esquema del board | `meta.schema_version` | `2` (nuevo en v15) | Se incrementa sólo con un cambio incompatible de tablas |
| Protocolo de coordinación | `instances.protocol` | `1` (`coordination.rs::PROTOCOL_VERSION`) | Cambia si cambia la semántica del lease |
| Versión del daemon | `instance.version` | `13.4.0` | Se compara en el plan de migración |
| Esquema del bus de malla | `mesh_board.json` → `schema` | `2` (`mesh.rs::BOARD_SCHEMA`) | **No** se mezcla con el del board |

> Nota: `coordination.rs` (SQLite) y `mesh.rs` (`mesh_board.json`) siguen
> siendo **dos artefactos distintos** con esquemas distintos. La decisión de
> §7.2 (primary único) asume que **el board SQLite es la autoridad** y que
> `mesh.rs` queda como transporte (peers, tunnels, aprendizaje). Esta
> separación se mantiene en v15 y **no se reimplementa**: `mesh.rs` sigue
> teniendo su propio `mesh_board.json` y su propio `schema`.

**Convivencia de una instancia vieja con una nueva (regla principal):**

| Regla | Detalle |
|---|---|
| SC-1 | Una instancia cuyo `protocol < MIN_SUPPORTED_PROTOCOL` se registra con `role` y `capabilities` pero **no puede reclamar tareas** (`won:false`, `details.reason="protocol_too_old"`). |
| SC-2 | Una instancia cuyo `protocol > PROTOCOL_VERSION` local **no puede escribir**: el merge rechaza su snapshot con `409 conflict`, `details.code="schema_too_new"`. **El estado no se corrompe**: los campos desconocidos se ignoran, los conocidos se mergean. |
| SC-3 | **Regla de campo desconocido:** al mergear, una columna o clave que el receptor no conoce se **conserva en `meta.raw_extra`** (JSON) y se ignora. Nunca se descarta información y nunca se interpreta mal. |
| SC-4 | El esquema se migra con `PRAGMA user_version` + `meta.schema_version`. La migración es **aditiva** (sólo `CREATE TABLE IF NOT EXISTS` / `ALTER TABLE ADD COLUMN`). **Prohibido** reescribir o borrar filas en una migración. |
| SC-5 | Orden de migración (heredado de `coordination.rs::migration_plan()`): **observers → workers → primary**. El coordinator se actualiza el último para que nunca quede caído. |
| SC-6 | Cada paso de migración exige acuse de recibo (`wait_for_ack`) por instancia. Una instancia que no acusa en 300 s se marca `stale` y el plan la salta. |
| SC-7 | **Rollback**: `md-coord snapshot > backup-<ts>.json` antes de migrar. Si el binario nuevo no arranca con el schema nuevo, se restaura el fichero anterior. El estado del board **no se pierde** porque es un fichero. |

**Procedimiento de migración v14 → v15 (concreto):**

```
1. md-coord snapshot > /data/local/tmp/megadaemon/board-<ts>.json     (backup)
2. Desplegar binario v15 en el worker (el primary sigue en v14)
3. md-coord migrate --target 15.0.0   -> imprime el plan ordenado
4. Cada instancia corre: md-coord migrate --target 15.0.0 --ack
   - crea las tablas nuevas (aditivo), rellena meta.schema_version=2
   - NO toca las tablas heredadas
5. Primary (último): md-coord migrate --target 15.0.0
6. Verificación: md-coord snapshot  ->  "schema_version": 2
   y el recuento de tasks/instances/events es >= el anterior
```

### 5.5 El coste: qué corre en el móvil y qué en el PC

**Presupuesto del móvil (S21 Ultra, 10,5 GB totales, ~0,3 GB libres — D-10):**

| Recurso | Presupuesto reservado | Nota |
|---|---|---|
| RAM del daemon (`megadaemon-core`) | **≤ 320 MB RSS** | Tope duro. Medición real: P-1 (pendiente). Si se supera ⇒ el daemon se auto-limita (deja de aceptar tareas `heavy`). |
| RAM del binario `md-coord` | **≤ 40 MB** | Proceso efímero, sólo cuando se usa. |
| Disco del `state_dir` | **≤ 1,5 GB** | `board.sqlite` (WAL) + `failures.ndjson` + `share/public` + journals. |
| `board.sqlite` (tamaño) | **≤ 64 MB** | Con `VACUUM` semanal y `events` podados a 30 días. |
| Conexiones HTTP simultáneas | **≤ 16** | El servidor actual es de un hilo por conexión: por encima de 16 se apila en el accept backlog. |
| Tamaño máximo de cuerpo HTTP | **8 MB** (ya vigente) | Los artefactos grandes van por chunking (§8.3). |

**Reparto de cargas:**

| Carga | Móvil (primary) | PC (worker) | Razón |
|---|---|---|---|
| Servir `/api/v2` + `/mcp` | **Sí** | Opcional (espejo) | Es el punto de entrada único |
| Tablero (leases, eventos) | **Sí** (autoridad) | Espejo de lectura | Un único punto de verdad |
| Bus de mensajes | **Sí** | Sí | Latencia |
| `sync.*` (conversaciones de apps) | Sí, con `heavy` | **Preferente en el PC** | Toca el keystore del proceso: sólo funciona en el dispositivo |
| `apk.inventory` | Sí | **Preferente en el PC** | El APK vive en el móvil: se vuelca y se analiza en el PC |
| `dex_decompile` | Sí (limitado) | **Preferente en el PC** | CPU y RAM |
| `signing` (CMS/PDF/PKCS#11) | **Sí, sólo aquí** | Imposible | La clave privada y el DNIe están en el móvil |
| `root_hide` / `app_intel` | **Sí, sólo aquí** | Imposible | Requiere root en el dispositivo |
| Builds ARM64 | **No** | **Sí** | El móvil no tiene NDK ni toolchain en producción; y no hay RAM para `rustc` |
| Análisis de repos (DEX, rizin, binwalk) | **No** | **Sí** | Pesado |
| Migraciones de esquema | Ordenadas, el primary el último | Sí | SC-5 |
| `failures.ndjson` (escritura) | Sí | Sí (cada agente escribe su copia) | Merge por `id` (MG-8) |

**Regla de gasto (para el móvil):**

| # | Regla |
|---|---|
| CG-1 | **Nada de bucles de polling agresivo.** Ningún temporizador por debajo de 5 s en el móvil. El heartbeat del board es de 30 s; el barrido de leases, de 60 s. |
| CG-2 | **El móvil no recalcula métricas grandes.** Las métricas se calculan donde hay dataset; el móvil sólo expone y agrega. |
| CG-3 | **Cada tarea declara `Resources {cpu, net, heavy}`** (`task_scheduler.rs`), y el planificador del móvil rechaza (`skip`) las `heavy` cuando `mem_available < 1,2 GB`. |
| CG-4 | **El daemon se auto-degrada**: si `mem_available < 800 MB`, deja de aceptar tareas nuevas, lo anuncia en `GET /api/v2/health` (`degraded:true`) y reduce el `limit` por defecto a 20. |
| CG-5 | **El bus usa long-poll** (`wait_s ≤ 30`) en vez de mantener conexiones abiertas para cada suscriptor: con 20+ agentes, 20 conexiones vivas en un teléfono es un problema. |
| CG-6 | **El `state_dir` del móvil está en almacenamiento local** (`/data/local/tmp/megadaemon`), nunca en NFS ni en la SD. |

### 5.6 Por qué **no** un sistema distribuido "de libro" (consenso, Raft)

| Criterio | Raft/Paxos en este sistema | Alternativa elegida | Veredicto |
|---|---|---|---|
| Coste en RAM | Log durable, membership, temporizadores por nodo | 0 (SQLite + HTTP) | Raft gana en RAM lo que pierde el resto |
| Coste en disco | `fsync` por entrada; el móvil tiene almacenamiento compartido y **NFS `nolock`** (D-1) | WAL local + merge idempotente | Raft exige fsync fiable; aquí el FS es LittleFS local en el móvil, pero el *board compartido* no puede ser NFS |
| Coste de implementación | Alto y frágil | Ya implementado (`merge_remote`, `sync_with_peer`) | El enjambre necesita **avanzar**, no un teorema de consenso |
| Tolerancia a fallos que realmente tenemos | 1 primary + N workers que pueden morir | Primary con failover manual (`promote_primary()` ya existe en `coordination.rs`) | El peer debe morir es un evento **raro y detectable** (heartbeat) |
| Consistencia requerida | Linearizability entre 20 agentes | Convergencia final + *lease* exclusivo por tarea | Lo que importa es que **una tarea la ejecute un agente**, no que todos vean el mismo instante |
| Observableidad | Estado opaco (log interno) | `GET /api/v2/events` y `GET /api/v2/events` | En un enjambre de 20+ agentes, ver el estado es más valioso que la consistencia fuerte |

**Lo que se implementa en su lugar:**

| # | Mecanismo | Garantía que da |
|---|---|---|
| CS-1 | **Un único punto de verdad**: el `board.sqlite` del primary | No hay split-brain *de estado*: las decisiones se toman en un sitio |
| CS-2 | **Convergencia por merge idempotente** (LWW por `updated_at`, terminal nunca degrada, dedup por clave) | Todos los nodos acaban con el mismo estado sin necesidad de consenso |
| CS-3 | **Lease exclusivo con caducidad** para la propiedad del trabajo | Exactamente una ejecución por tarea mientras el lease vive |
| CS-4 | **Detección de fallos por heartbeat** (`last_seen`) | Un nodo muerto se detecta en ≤ 180 s y su trabajo vuelve a la cola |
| CS-5 | **Failover manual y auditado** (`promote_primary()`) | Cuando el primary muere, otro nodo es promovido de forma explícita y queda en `events` |
| CS-6 | **Cuasi-captura de decisiones**: los eventos del board se replican como cola, y un nodo que se une después reconstruye su vista | Historial reproducible sin log de consenso |

**Límite explícito de la garantía (honestidad):**

> Con este diseño se obtiene **consistencia eventual con convergencia
> determinista**, **no** linearizability. En la ventana de convergencia
> (≤ `sync_interval` = 30 s, más la latencia del push) dos nodos pueden
> ver estados distintos del board. **Esta ventana es aceptable** porque:
> (a) la propiedad del trabajo la garantiza el *lease*, no la visibilidad;
> (b) el merge es determinista, así que el resultado final no depende del
> orden de llegada; (c) toda lectura que necesite verdad fuerte (p. ej.
> "quién es el primary") se hace **contra el primary**, no contra la copia.

---

## 6. PLAN DE TRABAJO

Criterio de orden: **por lo que desbloquea más**, no por lo que es más
interesante. Dependencias explícitas. Cada entregable tiene un criterio
**medible** (test o medición), nunca "funciona".

### Ola 0 — "Que el contrato exista de verdad" (antes que todo, 15 min)

| # | Entregable | Módulo Rust | Fichero(s) | Dueño | Criterio de aceptación MEDIBLE | Depende de |
|---|---|---|---|---|---|---|
| 0-1 | **Commit del contrato** (y de `docs/API_V2.md`, `docs/UI_V2_SPEC.md`, `docs/EXTERNAL_AGENT_PROMPT_FINAL.md`, que están sin commitear) | — | `docs/CONCURRENCY_PLAN_V15.md` + los otros | primary | `git log --oneline -1` contiene los 5 ficheros; `git status --short docs/` **vacío** | — |
| 0-2 | **Una línea en el tablón** que haga vinculante el contrato | — | `docs/COORDINATION.md` (al final, 1 línea) | primary | `grep -c "CONCURRENCY_PLAN_V15" docs/COORDINATION.md` ≥ 1; la línea nombra la regla: "toda ruta nueva se declara en la tabla del contrato **y** en `GET /api/v2/routes`" | 0-1 |
| 0-3 | **Registro de los 81 hallazgos** como tareas del tablero, priorizados y con dueño | `coordination` | `board.sqlite` (vía `md-coord add`) | primary | Un `md-coord tasks` por cada hallazgo de §10.4 (los 8 P0 primero) con `payload.adv = "<id>"` y `owner` = dueño del módulo DEFECTUOSO (no el del contrato) | 0-2, A-1 |
| 0-4 | **Mapa de rutas en `GET /api/v2/routes`** | `webui` | `rust-core/src/webui.rs` | api | `GET /api/v2/routes` devuelve, por ruta, su `route_id` del contrato y su `owner`; el test que compara la UI con la tabla sigue verde | A-3 |

> **Ola 0 no es opcional.** Sin 0-1 y 0-2, el contrato es un documento que
> sólo existe en este árbol de trabajo y los agentes siguen trabajando con
> cuatro verdades distintas (§10.1, H-7).

### Ola A — "Que el enjambre pueda trabajar" (día 0, bloqueante)

| # | Entregable | Módulo Rust | Fichero(s) | Dueño | Criterio de aceptación MEDIBLE | Depende de |
|---|---|---|---|---|---|---|
| A-1 | **Tablero desplegado en el primary** | `coordination` | `rust-core/src/bin/md-coord.rs`, `deploy/99-megadaemon.sh` | coord | `ls -l /data/local/tmp/megadaemon/board.sqlite` existe; `md-coord snapshot` devuelve JSON con `schema_version`; `md-coord tasks` lista ≥1 tarea | — |
| A-2 | **Esquema v2 del board** (tablas de §3.2) | `coordination` | `rust-core/src/coordination.rs` | coord | Test en `rust-core/tests/coordination_v2.rs`: tras `migrate()`, `meta.schema_version == "2"` y existen `idempotency`, `failures`, `files`, `bus_messages`; **migración sobre un board v1 existente no pierde filas** (comparar `count(*)` de `tasks` antes/después); **ejecutar `migrate()` dos veces no falla** (idempotencia del `ALTER`) | A-1 |
| A-3 | **Esqueleto de la API v2** | nuevo `webui` | `rust-core/src/webui.rs`, `rust-core/tests/api_v2_contract.rs` | api | Test file con ≥14 tests: `401` sin key, `403` key inválida, `403` sin token en POST, `429` + `Retry-After` presente, `404` vs `501` (AP-7), `422` body inválido, `limit` clamp a 500, `next_cursor` estable, `X-Request-Id` eco/generado, idempotency replay + `409` en key reusada, envelope de error con los 4 campos, `GET /api/v2/health` = 200 público | — |
| A-4 | **Idempotencia persistente** | `webui` + `coordination` | `webui.rs`, `coordination.rs` | api + coord | Test: 2 POST iguales con la misma `Idempotency-Key` ⇒ **1 sola** fila en `tasks` y la 2ª respuesta lleva `Idempotent-Replay: true`; key reusada con body distinto ⇒ `409` | A-2, A-3 |
| A-5 | **Rate limit por agente** (AL-1) | `api_key` | `rust-core/src/api_key.rs` | api | Test: 2 agentes con keys distintas sobre la misma IP ⇒ cada uno tiene su cubo (el 2º no recibe 429 hasta su propio límite); `Retry-After` ≥ 1 en todo 429 | A-3 |
| A-6 | **Build con disco local + cola** (regla 5-bis) | — | `tools/build-queue.sh` | todos | `MD_BUILD_TARGET=/run/agent-runtime/md/target tools/build-queue.sh build --release` ⇒ produce ≥1 `.rlib` en el target local (hoy: 0, D-4) y el segundo build es **incremental** (< 30 s) | — |
| A-7 | **Registro de agentes** | nuevo `autonomy` | `rust-core/src/autonomy.rs`, `<state_dir>/agents/` | agent | Test: `POST /api/v2/agents/register` crea `<state_dir>/agents/agent-<dominio>-<n>.json` con los 11 campos de §3.3 y es **idempotente** (2 registros del mismo id ⇒ 1 fichero) | A-3 |
| A-8 | **Reglas de no pisado aplicadas al repo** | — | `docs/COORDINATION.md` (1 línea), `.gitignore` | primary | `git status --short` **vacío** salvo las rutas del propio agente en los 20+ repos; `grep -rn "git add -A" docs/ tools/ deploy/` ⇒ 0 coincidencias en ficheros de instrucción | — |

### Ola B — "Que los agentes se hablen" (días 1-2)

| # | Entregable | Módulo Rust | Fichero(s) | Dueño | Criterio de aceptación MEDIBLE | Depende de |
|---|---|---|---|---|---|---|
| B-1 | **Bus de mensajes** (send/broadcast/reply/subscribe) | `mesh` | `rust-core/src/mesh.rs` | mesh | Test: `send` → `subscribe` devuelve el mensaje con el mismo `message_id`; `broadcast` llega a 2 suscriptores; `reply` encadena con `in_reply_to`; reenviar el mismo `message_id` **no** duplica (dedup por PK en `bus_messages`) | A-2 |
| B-2 | **NDJSON canónico** (mdshare/1) | nuevo `file_share` | `rust-core/src/file_share.rs` | files | Test: escribir y releer un manifiesto de 3 artefactos ⇒ `manifest_sha256` recomputado coincide; una línea truncada al final ⇒ el lector la descarta y las anteriores **no se pierden** | B-1 (formato de bus) |
| B-3 | **Mapeo MCP ↔ REST 1:1** | `mcp_server` + `webui` | `rust-core/src/mcp_server.rs`, `rust-core/src/webui.rs` | mcp + api | Test: para toda tool listada en `GET /api/v2/routes`, su `route_id` resuelve a una ruta registrada (≥15 tools mapeadas); las **26 tools legacy** siguen respondiendo igual que antes (comparar `tools/list` antes/después) | A-3 |
| B-4 | **Bus por HTTP contra el primary** (long-poll) | `mesh` + `webui` | `mesh.rs`, `webui.rs` | mesh | Medición: p95 de `send → visible` ≤ **250 ms** en LAN con 2 nodos; el emisor nunca tarda > 5 s | B-1 |
| B-5 | **Identidad y no-pisado en el repo** | — | `docs/COORDINATION.md`, `docs/AGENTS_OWNERSHIP.md` (nuevo) | primary | El fichero nuevo lista **cada** `owner_files` de `rust-core/src/*.rs` con su `agent_id`; 0 rutas sin dueño o con 2 dueños (verificable con un script de 20 líneas) | A-7 |

### Ola C — "Que aprenda de sus fallos" (días 3-5)

| # | Entregable | Módulo Rust | Fichero(s) | Dueño | Criterio de aceptación MEDIBLE | Depende de |
|---|---|---|---|---|---|---|
| C-1 | **Libro de fallos** | `autonomy` | `rust-core/src/autonomy.rs` | agent | Test: `POST /api/v2/failures` con una nota válida ⇒ 1 línea en `<state_dir>/failures.ndjson` + 1 fila en `failures`; nota con `kind` fuera de la taxonomía ⇒ `kind:"unknown"` + `notes`; `verification.result:"pending"` **no** cuenta como verificado | A-2, A-3 |
| C-2 | **Captura automática de fallos** | `autonomy` + `alerts` | `autonomy.rs`, `alerts.rs` | agent | Test: un 429 y un 502 registrados por el servidor ⇒ 2 notas nuevas con `kind:"net.rate_limited"` y `kind:"net.proxy_5xx"` en ≤ 5 s | C-1, A-5 |
| C-3 | **Maneo de patrones → ajustes** | `autonomy` + `delegate` | `autonomy.rs`, `delegate.rs` | agent + delegate | Test: 3 notas con el mismo `root_cause_hash` ⇒ se genera ≥1 `delegate::Adjustment` **válido**; `apply_adjustments()` lo aplica sin tocar cuentas no afectadas (ya hay test `apply_adjustments_rechaza_invalidos_sin_tocar_nada`) | C-1 |
| C-4 | **Métricas de autonomía** | `autonomy` | `autonomy.rs` | agent | Test: `GET /api/v2/metrics` devuelve `failure_rate_by_kind` **con denominador** (`window_hours`), `mttr_s`, `verified_fix_ratio`; con dataset vacío devuelve `null` y **no** `0` | C-1 |
| C-5 | **Autonomía acotada** | `autonomy` | `autonomy.rs` | agent | Test: el ciclo automático **no** ejecuta ninguna acción con `confirm:true`; máximo **5** acciones por ciclo y **1 ciclo/min**; toda acción deja nota en `failures.ndjson` + evento en `events` | C-3, C-4 |
| C-6 | **`mesh::learning_notes` alimentado por el libro** | `mesh` + `autonomy` | `mesh.rs`, `autonomy.rs` | mesh | Test: `learning_notes()` menciona el `kind` dominante leyendo de `failures.ndjson` (no sólo de `AgentTask` en memoria) | C-1 |

### Ola D — "Compartir ficheros sin pisarse" (días 5-7)

| # | Entregable | Módulo Rust | Fichero(s) | Dueño | Criterio de aceptación MEDIBLE | Depende de |
|---|---|---|---|---|---|---|
| D-1 | **Zonas y manifiesto** | `file_share` | `rust-core/src/file_share.rs` | files | Test: `POST /api/v2/share/manifest` con `zone:"restricted"` ⇒ `403 forbidden` con `details.zone`; con `zone:"public"` ⇒ aceptado y `files` tiene la fila | B-2 |
| D-2 | **Push/pull con verificación sha256** | `file_share` | `file_share.rs` | files | Test: artefacto con sha256 incorrecto ⇒ **rechazado** (`files.hash_mismatch`) y **no** aparece en `files`; el mismo sha256 reenviado ⇒ `duplicate:true`, **0 bytes** escritos de nuevo | D-1 |
| D-3 | **Versionado por hash (nunca sobrescribir)** | `file_share` | `file_share.rs` | files | Test: 2 artefactos con el **mismo nombre** y sha256 distinto ⇒ **conviven** en rutas distintas; el manifiesto de cada uno es **append-only** (el nº de líneas crece, no se reescribe) | D-2 |
| D-4 | **Push de artefactos desde el agente externo** | `file_share` + `mcp_server` | `file_share.rs`, `mcp_server.rs` | files + mcp | Test end-to-end: el layout `incoming/<agent_id>/MANIFEST.ndjson` que ya usa `tools/findings_from_manifest.py` se acepta tal cual (compatibilidad) y `tools/findings_from_manifest.py --check` sigue pasando sobre el manifiesto generado | D-2 |

### Ola E — "La web que es una vista, no un adorno" (días 5-10, en paralelo con D)

| # | Entregable | Módulo Rust | Fichero(s) | Dueño | Criterio de aceptación MEDIBLE | Depende de |
|---|---|---|---|---|---|---|
| E-1 | **UI v2 sobre `/api/v2`** | `webui` | `web/cockpit_v2.html`, `rust-core/src/webui.rs` | web + api | Test: la UI **no** pide ninguna ruta `/api/*` v1 (verificable con `grep` sobre el HTML: 0 coincidencias de `/api/` que no sea `/api/v2/`); todas sus peticiones llevan `X-Request-Id` | A-3 |
| E-2 | **Vista de enjambre** (board + agentes + bus) | `webui` | `web/cockpit_v2.html` | web | Test: la vista muestra tareas con su `lease_expires_at` en vivo (poll de R-04 cada 10 s) y un agente `stale` aparece distinto | A-3, A-7 |
| E-3 | **Vista de fallos y métricas** | `webui` | `web/cockpit_v2.html` | web | Test: la vista muestra `verified_fix_ratio` y el último fallo, leyendo de R-20/R-22 | C-4 |
| E-4 | **Degradación visible** | `webui` | `web/cockpit_v2.html` | web | Test: con `mem_available < 800 MB` el `GET /api/v2/health` trae `degraded:true` y la UI muestra el aviso (no un error silencioso) | CG-4 |

### Ola F — "Cierre y convergencia" (día 10+)

| # | Entregable | Módulo Rust | Fichero(s) | Dueño | Criterio de aceptación MEDIBLE | Depende de |
|---|---|---|---|---|---|---|
| F-1 | **Migración v14→v15 en los 3 nodos** | `coordination` | `coordination.rs`, `deploy/99-megadaemon.sh` | coord | Medición: `meta.schema_version == "2"` en los 3 nodos; `count(tasks)` y `count(events)` **no disminuyen** respecto al backup; el primary migró **el último** | A-2, B-5 |
| F-2 | **Failover del primary** | `coordination` | `coordination.rs` | coord | Test: `promote_primary()` cambia `meta.primary_id` y deja evento; el nodo viejo se degrada a `worker` y su merge sigue funcionando | F-1 |
| F-3 | **Presupuesto del móvil verificado** | `ops` | `deploy/99-megadaemon.sh`, `autonomy.rs` | ops | Medición real (P-1): RSS del daemon ≤ **320 MB** con 20 tareas en cola y 2 agentes suscritos; `board.sqlite` ≤ 64 MB | — |
| F-4 | **Informe de auditoría del sistema** | `autonomy` | `docs/AUDIT_V15.md` | primary | Documento con las métricas de §4.6 medidas **antes y después** de v15 (comparativa), con la fuente de cada número | todo lo anterior |

### Ola G — "Que cada nodo aporte" (días 4-9, transversal a C/D/E)

| # | Entregable | Módulo Rust | Fichero(s) | Dueño | Criterio de aceptación MEDIBLE | Depende de |
|---|---|---|---|---|---|---|
| G-1 | **Certificado de nodo** (`mdcert/1`): publicación, verificación HMAC, `seq` monótono, TTL | nuevo `node_cert` | `rust-core/src/node_cert.rs`, `rust-core/tests/node_cert.rs` | dist | Test: cert con `seq` anterior ⇒ `409 stale_seq`; firma inválida ⇒ `403 bad_signature`; cert con `node` ajeno a la API key ⇒ `403`; `mdcert` revalidado tras 180 s ⇒ `cert_expired` | A-3 |
| G-2 | **Reglas locales** (`mdrules/1`) + evaluador | `node_cert` | `node_cert.rs`, `<state_dir>/rules/share_rules.json` | dist | Test: con `mem_available_mb=800` ⇒ `accepts=[]`, `reason="mem_bajo…"`; con `mem=3000, cpu=10` ⇒ `accepts=["review","inference"]`; `reserve_for_user` **vacía** `accepts` aunque la regla diga lo contrario | G-1 |
| G-3 | **Índice de distribución** (`dist.sqlite`) + streams NDJSON | `node_cert` | `node_cert.rs` | dist | Test: `certs.ndjson` y `contributions.ndjson` son append-only; `md-dist reindex` reconstruye `dist.sqlite` **sin pérdida** (mismo `count(*)`) | G-1 |
| G-4 | **Clases de trabajo** en las tareas + filtro duro | `coordination` + `node_cert` | `coordination.rs`, `node_cert.rs` | coord + dist | Test: `class:"build"` **nunca** se asigna al móvil; `class:"device"` **sólo** al móvil; sin candidatos ⇒ `409 no_candidate` con el motivo | G-1, A-2 |
| G-5 | **Ledger de contribución** + crédito en el reparto | `node_cert` + `coordination` | `node_cert.rs`, `coordination.rs` | dist + coord | Test: aportación sin `evidence` ⇒ `units=0`; nodo con `ratio<0,2` ⇒ `task_reassigned` con `reason="low_contribution"`; el primary **nunca** entra en cuarentena | G-3, A-2 |
| G-6 | **Ofertas de capacidad libre** | `node_cert` | `node_cert.rs` | dist | Test: una oferta usada suma contribución; una oferta no usada **no** suma; la oferta caduca a los `ttl_s` | G-3 |
| G-7 | **Modo "free" del móvil** (reglas por defecto del Samsung) | `node_cert` | `deploy/99-megadaemon.sh` (plantilla de `share_rules.json`) | ops | Medición: con el móvil ocioso (`mem>2500 MB`) el cert declara `accepts=["review","inference"]`; con el usuario usando el teléfono (`mem<1200` o `screen_on`) declara `accepts=[]` y `offloads=["compute","build"]` | G-2, G-7 |
| G-8 | **Vista de reparto en la web** | `webui` | `web/cockpit_v2.html` | web | Test: la vista muestra por nodo su cert (clases, holgura, `reason` de la regla) y su crédito; muestra qué tareas se delegaron y por qué | G-1, G-5 |
| G-9 | **Presupuesto del móvil con reparto activo** | `ops` | `deploy/99-megadaemon.sh` | ops | Medición real (P-1 revisado): con 1 tarea externa aceptada, el RSS del daemon sube **≤ 150 MB** y `mem_available` baja de 2500 MB a **≥ 1500 MB** (la reserva del usuario) | G-7, F-3 |

**Reglas de la tabla:**

| # | Regla |
|---|---|
| W-1 | Cada fila = **un** `agent_id` (§3.4). Si dos filas comparten dueño, es un agente solo haciendo dos cosas (aceptable) o un conflicto (no). |
| W-2 | Cada criterio de aceptación es un **test** o una **medición**, ejecutable sin preguntar a nadie. "Funciona" no es un criterio. |
| W-3 | Ninguna fila depende de un fichero que otro agente va a reescribir. Si dos filas tocan el mismo fichero, la segunda depende de la primera **y** su `owner_files` está actualizado (B-5). |
| W-4 | El **Definition of Done** por tarea es el de `MASTER_PLAN.md` §7, con una condición añadida: **todo módulo nuevo entra en `lib.rs` con su arnés `#[path]` verde** (NP-9). |
| W-5 | Onda A antes que B antes que C antes que D. E puede ir en paralelo con D. **G es transversal**: arranca en C (G-1, G-2) y cierra antes que F. F cierra. |
| W-6 | La capa de distribución **no** toca `board.sqlite`: usa `dist.sqlite` como índice derivado (§3.2). Así `coord` y `dist` no se pisan. |
| W-7 | Un nodo **nunca** empieza a aceptar trabajo ajeno sin un `node cert` válido publicado. Sin cert ⇒ `no_candidate`. |
| W-8 | **Ola 0 va primero de todo**: commit + línea en el tablón. Sin eso, el resto del plan se ejecuta contra un documento que nadie ve (§10.6). |
| W-9 | Un hallazgo de la crítica (`<área>-<n>`) **no** es una tarea de documentación: es una tarea del **módulo defectuoso**, con su test como criterio de aceptación (UN-6). |

---

## 7. DECISIONES PENDIENTES (necesitan al usuario)

Cada decisión lleva: **recomendación** (la que este documento asume por
defecto, para que el trabajo no se bloquee) y **coste de la alternativa**.

### 7.1 ¿El vault de firma replica material privado?

| Opción | Descripción | Coste |
|---|---|---|
| **R-1 (RECOMENDADA)** | El vault **nunca** replica material privado. Se replica sólo el **manifiesto** (metadatos, sha256, algoritmo, huella de la clave). La firma se pide al nodo que tiene la clave: el worker envía el *preimage* + algoritmo y recibe la **firma** por la API autenticada. | Coste: la firma no es paralelizable (un único nodo firma) ⇒ ~1-3 s extra por documento firmado. Beneficio: la clave privada (DNIe, `.p12`, RSA del host ADB) nunca sale del dispositivo que la guarda. |
| A-1 | Replicar el vault cifrado (con passphrase) a todos los nodos | Coste: la passphrase acaba en todos los nodos ⇒ un worker comprometido da la clave de firma; el `.p12` del DNIe tiene validez legal y es **irreversible** si se filtra. |
| A-2 | Replicar el vault en claro | Coste: **inaceptable**. RCE⇒firma con la identidad del usuario. No se implementa en ninguna variante. |

**Estado actual:** `backup_vault.rs` ya implementa `seal`/`open`/`verify`/
`drive_upload_plan` con PBKDF2-HMAC-SHA256 (`backup_vault.rs:245`). La
decisión es de **política de replicación**, no de código.

### 7.2 ¿El tablero se replica a los workers o sólo lo posee el primary?

| Opción | Descripción | Coste |
|---|---|---|
| **R-2 (RECOMENDADA)** | El **primary posee** el `board.sqlite` (fuente de verdad). Los workers tienen un **espejo de sólo lectura** + cola de salida; hacen push/pull por HTTP (R-13). | Coste: ~1 día de trabajo (push/pull ya está en `sync_with_peer()`); los workers deben tolerar hasta 30 s de desfase (aceptable, §5.6). |
| A-1 | El `board.sqlite` **compartido por NFS** | Coste: **prohibido por medición** (D-1: NFS v3 `nolock` ⇒ `database is locked`). Además, dos writers sobre SQLite por red es la receta del corruption. No se implementa. |
| A-2 | Cada worker tiene su board y se fusiona todo a todo (malla) | Coste: N² tráfico, conflictos de ownership y **sin autoridad** para arbitrar. Contradice CR-1. No se implementa. |

### 7.3 ¿Los agentes externos pueden escribir en el repo?

| Opción | Descripción | Coste |
|---|---|---|
| **R-3 (RECOMENDADA)** | **No.** Publican artefactos y propuestas (manifiesto + `findings.ndjson` + nota en el board). El **primary** es el único que commitea y hace push. | Coste: un paso de ingesta extra en el primary (ya hay `tools/findings_from_manifest.py`; falta el wrapper que lo automate: ~medio día). Beneficio: cero riesgo de que un agente externo pise el árbol compartido (D-17, D-18). |
| A-1 | Que escriban en su **propio directorio del workspace** (`worktrees/<agent>/`) y el primary fusiona | Coste: hay que gestionar worktrees concurrentes sobre NFS (D-1) y el merge es manual: 2-3 días y riesgo de conflicto. |
| A-2 | Que commiteen directamente en `main` | Coste: **prohibido** (NP-2/NP-3/NP-5). Permitirlo reintroduce exactamente los fallos de D-11, D-12, D-17. No se implementa. |

### 7.4 ¿El rate limit sube por defecto?

| Opción | Descripción | Coste |
|---|---|---|
| **R-4 (RECOMENDADA)** | **Sí, pero sólo para tráfico autenticado**: 600 req/min **por agente** (AL-1); 60 req/min por IP **no autenticada**; `MEGADAEMON_RATE_MAX` lo sobrescribe. | Coste: ~1 día (indexar el cubo por `api_key_id` en vez de por IP + test). Beneficio: elimina la cascada 429 medida (D-19, D-20) sin abrir el servicio. |
| A-1 | Subir el límite global a 600 sin más | Coste: ~0 (es una constante) pero **no arregla nada**: todos los agentes y el túnel siguen compartiendo cubo ⇒ la cascada sigue (sólo se retrasa). |
| A-2 | Dejar 60 tal cual | Coste: 0 código, pero el enjambre sigue muriendo por 429 y los agentes "abandonan" creyendo que la instancia está caída (D-20). **Es la causa medida de "nadie me hace caso".** |

### 7.5 ¿El bus de mensajes es HTTP+NDJSON o MQTT?

| Opción | Descripción | Coste |
|---|---|---|
| **R-5 (RECOMENDADA)** | **HTTP + NDJSON** sobre el listener existente (tabla `bus_messages` + 4 rutas). Sin proceso extra. | Coste: ~2 días (B-1, B-4). Beneficio: 0 MB de RAM extra en el móvil, 0 puertos nuevos, 0 procesos que supervise (ya hubo un proceso que moría 3× en 30 min, D-21). |
| A-1 | MQTT con broker embebido | Coste: +1 binario, +1 puerto, +5-15 MB RAM, +1 forma de fallo, y un broker en el móvil con ~0,3 GB libres (D-10). El bus no necesita QoS 2 ni retención durable: at-least-once + dedup basta. |
| A-2 | WebSocket | Coste: mantener conexiones vivas en el móvil (CG-5) ⇒ 20+ conexiones permanentes con 0,3 GB libres. No recomendado **ahora**; se deja como opción de futuro **detrás de las mismas 4 rutas** si el long-poll resulta insuficiente. |

### 7.6 Otras decisiones abiertas (menor coste, mismo patrón)

| # | Decisión | Recomendación | Coste de la alternativa |
|---|---|---|---|
| D-1 | ¿`/api/v2` reemplaza a v1 o convive? | **Convivir**: v1 congelada (sirve, no se amplía), v2 canónica. La UI nueva sólo usa v2. | Sustituir v1 de golpe: rompe la UI actual (que hoy funciona tras D-21) y el prompt del agente externo ya publicado (`CONNECT_EXTERNAL_AGENT_ANDROID_RE.md` §3) ⇒ días de retrabajo. |
| D-2 | ¿Duración del lease: 900 s (board) o 300 s (mesh)? | **900 s para el board** (`coordination.rs::DEFAULT_LEASE_SECS`), **300 s** se queda para las tareas internas de malla. Un valor único. | Unificar a 300 s: las tareas largas de firma/DEX se re-encolan mientras el worker sigue trabajando ⇒ duplicación de trabajo. Unificar a 900: las tareas internas de malla tardan 15 min en liberarse si el worker muere ⇒ elongation de la cola. |
| D-3 | ¿El libro de fallos se commitea al repo? | **No**: vive en `<state_dir>/failures.ndjson` y se **exporta** por mes a `docs/` si el primary lo decide. | Commitearlo: diffs de cientos de líneas por ciclo, ruido en el tablón y riesgo de pisado (NP-1). |
| D-4 | ¿Los agentes externos pueden reclamar tareas del board? | **No de momento**: publican y esperan. Reclamar exige `owner_files` y un `agent_id` registrado. | Permitirlo: un externo sin registro usa ficheros ajenos (NP-6). |
| D-5 | ¿El `primary` puede delegar el reparto? | **No**: el reparto lo hace el primary con el algoritmo de §5.2. | Delegar: más complejo y sin autoridad clara; el `claim` atómico ya evita el conflicto. |
| D-6 | ¿`GET /api/v2/health` sigue siendo público? | **Sí**, pero con **menos** campos que hoy (`AUDIT_V14_UI_HTTP.md` UI-15: reconocimiento de superficie). Se quita todo lo que sea inventario del dispositivo; queda `{status,version,uptime_s,protocol,schema,degraded,routes_pending}`. | Quitarlo: el diagnóstico de "no responde" (D-23) vuelve a ser invisible. |
| D-7 | ¿La web v2 se sirve en el mismo puerto? | **Sí** (AP-9): mismo listener, alias 3000/3030/8090/8080/80, sin puertos nuevos. | Puerto nuevo: hay que tocar el supervisor del móvil y el `web_bridge.py` (que ya muere, D-21) ⇒ más superficie de fallo. |

### 7.7 Decisiones sobre el reparto de trabajo entre nodos (§9)

| # | Decisión | Recomendación | Coste de la alternativa |
|---|---|---|---|
| D-8 | **¿El móvil acepta trabajo ajeno?** (el usuario paga el dispositivo: batería, calor, datos) | **Sí, pero sólo `review` e `inference` remota**, y sólo con `mem_available ≥ 2500 MB` y `screen_on == false` (DF-2, DF-3, DF-5). El resto se **delega**. | No aceptar nada: el móvil sólo coordina y ejecuta `device`; se desaprovecha la NPU/GPU ociosa y el "cada nodo aporta" se queda en theory. Aceptar de más: el usuario nota el móvil caliente y lento — y el móvil es justo el nodo con la RAM justa (D-10). |
| D-9 | **¿El reparto es "fair-share" estricto (cuota fija) o "crédito" (orden por aportación)?** | **Crédito**: la aportación pasa a **desempate con prioridad** en §5.2; la **cuarentena** (DL-5) sólo tras 48 h de ratio bajo y es reversible. | Cuota fija: dos nodos con capacidades distintas reciben lo mismo ⇒ se desperdicia el más capaz y se satura el menos capaz. Coste de la cuota: más estado y más casos raro. |
| D-10 | **¿Las ofertas de capacidad son explícitas o implícitas?** | **Explícitas** (`POST /api/v2/offers`), con TTL ≤ 600 s. | Implícitas (el primary infiere la oferta del cert): menos tráfico, pero el primary adivina mal (D-10 original: la Cascada 429 nació de adivinar el estado de los demás). Coste de la alternativa: 0 código, menos corrección. |
| D-11 | **¿Un nodo que no aporta puede ser expulsado del enjambre?** | **No se expulsa**: se le **degrada a `observer`** (deja de recibir tareas) y se levanta solo al volver a aportar (DL-5). La expulsión sería una decisión sobre el hardware del usuario: **irreversible y fuera del perímetro** (§4.4 AU-2). | Expulsar: castiga al nodo y deja al enjambre sin capacidad; además requiere intervención humana para revertirlo. |
| D-12 | **¿El `node cert` es firmado (HMAC) o va en claro?** | **Firmado** con clave por nodo (`<state_dir>/node_cert.key`, 0600), verificada por el primary (DV-1). TOFU en el alta. | En claro: cualquiera con una API key puede publicar un cert falso diciendo "estoy libre" y acaparar tareas. Coste de la alternativa: 0 código, riesgo directo de DoS del reparto. |
| D-13 | **¿Quién decide la clase de una tarea?** | **El que la crea**, con la lista cerrada de §9.1; sin clase ⇒ `review`. El primary **no** reasigna clases (sólo el *lugar*). | El primary infiere la clase desde el `kind`: frágil (`agent_core` tiene 20 `kind`s y no una taxonomía de coste) y mezcla decisiones de ejecución con decisiones de colocación. |

### 7.8 Decisiones YA tomadas (ya no están pendientes)

> Registro de lo que el usuario decidió el 2026-11-19 22:36, para que ningún
> agente lo vuelva a abrir. Los cambios que producen están en §10.5.

| # | Decisión | Veredicto | Consecuencia práctica |
|---|---|---|---|
| T-1 | ¿Se commitea el contrato y se apunta en el tablón? | **Sí, a todo** | Hecho: commit propio del contrato + commit de rescate de los docs sin commitear de otros agentes + **una línea** en `docs/COORDINATION.md` (Ola 0-1, 0-2). |
| T-2 | ¿Se renombran las rutas ya implementadas? | **No se renombra el código ajeno; sí se renombra el contrato** (v15.0 → v15.1) | El contrato adopta el vocabulario existente (`tasks`, `nodes`, `share`, `events`); se elimina la familia paralela `board`/`bus`/`files`. 0 ficheros de otros agentes tocados. |
| T-3 | ¿Se crea `/api/v2/mcp`? | **No** | El endpoint MCP sigue siendo `POST /mcp` (JSON-RPC): compatibilidad con `tools/mdmcp.py` y con el prompt del agente externo ya publicado. |
| T-4 | ¿Se unifica con la crítica adversaria? | **Sí, estructuralmente** | Tres capas con precedencia (§10.2) + formato `mdobj/1` + registro de las 15 objeciones (§10.4). El crítico **no edita** el contrato. |
| T-5 | ¿Quién resuelve las objeciones? | El **primary** (o a quien designe), una por una, con veredicto escrito | Cada `accepted` produce un cambio en el contrato con entrada en §10.5. |

---

## 8. CONTRATO DE COLABORACIÓN DE ARCHIVOS

### 8.1 Zonas

| Zona | Ruta | ¿Se replica? | Contenido | Quién puede escribir |
|---|---|---|---|---|
| `public` | `<state_dir>/share/public/<sha[0:2]>/<sha256>/<name>` | **Sí**, a todos los nodos | Artefactos derivados, informes, `findings.ndjson`, código de proyectos, APKs/DEX volcados, logs sanitizados | Cualquier agente registrado; el primary lo publica |
| `instance` | `<state_dir>/share/instance/<agent_id>/…` | **No** (sólo su nodo) | Estado local, caché, journals, artefactos intermedios en curso, `board.sqlite` local | Sólo el dueño de la instancia |
| `restricted` | `<state_dir>/vault/…` (0700) | **Nunca** | Claves privadas, tokens, cookies, material de firma, keystore exports | Sólo el proceso local; **jamás** por HTTP de réplica |

**Reglas de zona (normativas):**

| # | Regla |
|---|---|
| Z-1 | `POST /api/v2/share/put` con `zone:"restricted"` ⇒ **`403 forbidden`**, `details.zone="restricted"`, `details.reason="never_replicated"`. **Prohibido** meter material privado en el pipeline de réplica, aunque el propio agente sea el dueño. |
| Z-2 | El **hash del artefacto** es la clave de la zona: el mismo sha256 en dos zonas ⇒ dos entradas (no se deduplican entre zonas). |
| Z-3 | La zona se declara en la **línea de cabecera** del flujo y en cada línea `artifact`. Una cabecera `zone:"public"` con una línea `artifact` de otra zona ⇒ esa línea se rechaza (`422 unprocessable`, `details.mismatch`). |
| Z-4 | `instance` nunca sale del nodo; ni siquiera para "revisar". Un artefacto `instance` que un nodo quiere compartir pasa a `public` **de forma explícita** (copia + manifiesto nuevo), nunca por defecto. |
| Z-5 | `restricted` no se replica, no se versiona en el bus, no aparece en `GET /api/v2/share/manifest` para otros agentes, y no se incluye en ningún export de board. |

### 8.2 El manifiesto

**Formato:** NDJSON `mdshare/1` (§2.7). Fichero canónico:
`<state_dir>/share/public/<agent_id>/MANIFEST.ndjson` (append-only).
**Compatibilidad:** el layout ya en uso por el agente externo
(`incoming/<agent_id>/MANIFEST.ndjson`) **se acepta tal cual** — es la misma
estructura. `tools/findings_from_manifest.py` sigue funcionando sin cambios.

**Línea de artefacto (los 12 campos exactos):**

```json
{"schema":"mdshare/1","type":"artifact","ts":"2026-11-19T22:00:00Z","id":"art-0001","name":"app.apk","sha256":"e3b0c442…","size":12582912,"mime":"application/vnd.android.package-archive","zone":"public","path":"share/public/e3/e3b0c442…/app.apk","agent":"ext-androidre-01","note":"APK crudo"}
```

**Campos obligatorios:** `schema`, `type`, `ts`, `id`, `name`, `sha256`, `size`,
`zone`. **Opcionales:** `mime`, `path`, `agent`, `note`.

**Reglas:**

| # | Regla |
|---|---|
| FM-1 | `sha256` = SHA-256 **hex minúscula** del fichero, calculado con el crate `sha2` **en trozos** (nunca cargando el fichero entero en RAM: el móvil tiene ~0,3 GB libres, D-10). |
| FM-2 | `size` = tamaño en bytes. |
| FM-3 | `id` = identificador **estable y único** dentro del manifiesto del agente (`art-0001`, `art-0002`, …). |
| FM-4 | Un manifiesto **append-only**: una corrección es **una línea nueva**, nunca una edición. |
| FM-5 | `manifest_sha256` (línea `end`) = SHA-256 de la concatenación exacta de las líneas `artifact` en orden de emisión. |

### 8.3 Envío, verificación en destino, rechazo

**Flujo de push (R-32):**

```
1. El emisor calcula el sha256 en trozos (sha2) y lo anuncia:
   POST /api/v2/share/put  {name, sha256, size, zone:"public"}
   → 200 {sha256, size, duplicate: bool}
        duplicate=true  ⇒ el destino ya lo tiene: NO se transfiere nada (idempotencia)
        duplicate=false ⇒ continuar
2. Transferencia en trozos (por el límite de 8 MB del cuerpo HTTP):
   POST /api/v2/share/put  {name, sha256, chunk_index, chunk_total, data_b64}
   → 200 {received_chunk: n}
   El destino escribe en <tmp>/<sha256>.part
3. Cierre: POST /api/v2/share/put {name, sha256, chunk_total, final:true}
   → el destino RECALCULA el sha256 del fichero ensamblado y COMPARA
        coincide   ⇒ mueve a share/public/<sha[0:2]>/<sha256>/<name>, inserta en `files`, responde 200
        NO coincide ⇒ borra el .part, responde 422 unprocessable, details.expected/actual,
                       registra una nota kind="files.hash_mismatch" en el libro de fallos
```

| # | Regla |
|---|---|
| PV-1 | **La verificación se hace SIEMPRE en destino**, recalculando el hash del fichero ensamblado. Confiar en el hash del emisor no es verificar. |
| PV-2 | **Rechazo**: si no cuadra ⇒ el artefacto **no** entra en `files`, no se publica y se genera la nota de fallo `files.hash_mismatch` con el `agent` origen. |
| PV-3 | **Idempotencia**: `files.sha256` es la clave primaria ⇒ reenviar el mismo artefacto ⇒ `duplicate:true`, **0 bytes** escritos. Repetir el push nunca duplica. |
| PV-4 | **Nunca sobrescribir**: dos artefactos con el mismo `name` y sha256 distinto **conviven** en rutas distintas (`<sha>/<name>`). El que llega después **no** borra al anterior. |
| PV-5 | **Supersesión explícita**: si un artefacto reemplaza a otro, el manifiesto lo declara con `"supersedes":"<sha256>"`. La versión antigua **se conserva** (no se borra) hasta que el primary lo decida. |
| PV-6 | **Cuarentena**: 2+ `files.hash_mismatch` del mismo `agent` ⇒ ese agente queda `quarantined` en `files` (no publica más) hasta que el primary lo reactive. Motivo: el `agent` externo ya warned "no sobrescribas un artefacto ya volcado por otro sin registrar la versión". |
| PV-7 | **Límite de tamaño**: un artefacto > **512 MB** ⇒ `422 unprocessable` con `details.max_bytes`. En el móvil, encima, `share/public` no puede pasar de **1 GB** (presupuesto de §5.5) ⇒ si se supera, `503`-equivalente → `422 unprocessable` con `details.quota="share_public"`. |
| PV-8 | **Verificación bajo demanda**: `GET /api/v2/share/verify?sha256=…` recalcula y devuelve `{ok, mismatches[]}`. Es la operación que usa el primary para auditar un workspace externo. |

### 8.4 Versionado por hash ("nunca sobrescribir")

| Concepto | Definición |
|---|---|
| Identidad de un artefacto | su `sha256` (contenido) — **no** su nombre |
| Nombre | Etiqueta legible; **no** identifica |
| Colisión de nombre | Dos artefactos con el mismo `name` y distinto hash ⇒ **dos entradas**, rutas distintas |
| Ruta final | `share/public/<sha[0:2]>/<sha256>/<name>` |
| Historial | El manifiesto es append-only; la "versión actual" de un nombre es la **última** línea con ese `name` que no esté `superseded` |

**Por qué no "último gana"**: dos agentes pueden producir artefactos del mismo
nombre en paralelo (p. ej. `report.md`). Con "último gana" se pierde uno sin
que nadie lo note. Con versionado por hash, ambos existen y el manifiesto
dice cuál es cuál.

### 8.5 Material privado: qué nunca sale del dispositivo

| Material | Dónde vive | Por qué no sale |
|---|---|---|
| Clave RSA del host ADB | `/data/local/tmp/megadaemon/adb_host_key.pem` (0600) | Es la credencial de `root` frente al `adbd` del propio móvil. El que la tiene, entra como root. Ya está en la lista de "NO darle" de `CONNECT_EXTERNAL_AGENT_ANDROID_RE.md`. |
| Claves de firma (`signing.rs`): RSA/ECDSA, `.p12`, DNIe | `state_dir` + AndroidKeyStore / TEE | Valor legal. El DNIe además **no sale ni por root** (clave en el TEE). |
| Token de escritura (`X-Seere-Auth-Token`) | `MEGADAEMON_COCKPIT_TOKEN` fuera del repo | Da escritura en la instancia. |
| API keys (`megadaemon_api.key`, `megadaemon_api.keys`) | `state_dir` (0600) | Acceso remoto. |
| Token de GitHub | Fuera del repo | Push a `main`. |
| Cookies de Chrome / almacén de WhatsApp | TEE / keystore del proceso | Cifrado con clave del TEE: **ni root las saca**. No son "privadas" por política, son **técnicamente** inalcanzables. |
| `MEGADAEMON_API_KEY` en ficheros del repo | — | Nunca. `.gitignore` ya filtra `*.key`/`*.env`; y `git add -f` está prohibido. |

**El patrón único (decidido):** para todo material que no puede salir, la
compartición no es "copiar el fichero" sino **"pedir la operación"**:

| Material | Quién lo usa | Cómo se comparte la capacidad (no el secreto) |
|---|---|---|
| Firma | El agente que pide una firma | Envía `preimage_digest` + algoritmo + plantilla; el nodo con la clave **firma** y devuelve la **firma** + el certificado público. La clave no se mueve. |
| ADB | El PC que quiere hablar con el `adbd` | El daemon del móvil hace de **proxy** (`adb_tool`: `status\|shell\|devices\|push\|pull`). El PC pide, el móvil ejecuta. |
| Root exec | PC/agente externo | `root_exec` vía `/mcp`, con `confirm:true` en las acciones destructivas (auditadas). |
| Sesión autenticada del cockpit | Navegador | `POST /api/session/exchange` canjea la API key por un token de sesión; la key **nunca** va en la URL ni en la query string. |

> **Línea roja del contrato**: **ningún endpoint de v2 devuelve material
> `restricted`**, ni siquiera al propio `primary`. La única vía de retorno es
> la **firma** (dato público, verificable con la clave pública) y la
> **ejecución delegada** (efecto, no secreto).

### 8.6 Resumen operativo del contrato de ficheros

| Regla | Una línea |
|---|---|
| FF-1 | Todo artefacto lleva `sha256`; el destino **recalcula** y rechaza si no cuadra. |
| FF-2 | La clave de identidad es el **hash**, no el nombre: nunca se sobrescribe nada. |
| FF-3 | Las tres zonas se declaran explícitamente y `restricted` **jamás** se replica. |
| FF-4 | El manifiesto es **append-only** NDJSON; `manifest_sha256` cierra el fichero. |
| FF-5 | Repetir un push es **gratis** (idempotente por hash). |
| FF-6 | Lo privado no se comparte: se comparte la **capacidad** de usarlo. |

---

## 9. CONTRATO DE DISTRIBUCIÓN DE TRABAJO ENTRE NODOS

> **Interpretación de la petición**: "automatizado por *certs* y *reglas
> locales*". Se implementa como dos piezas: el **certificado de nodo**
> (`node cert`, un documento firmado que declara qué puede y qué quiere hacer
> un nodo) y las **reglas locales** (un fichero por nodo que decide, con
> señales medibles, **qué se comparte, qué se reserva y qué se delega**).
> Si la intención era otra, es un bloque de §9.3 y §9.4 y se corrige ahí.

### 9.0 Principio: cada nodo es consumidor **y** proveedor

| # | Principio |
|---|---|
| DN-1 | **Todo nodo del enjambre consume y aporta.** No hay nodos "de segunda". El Samsung coordina **y** ejecuta device, **y** ofrece capacidad libre cuando el usuario no lo necesita. |
| DN-2 | **El reparto es una decisión local, ejecutada localmente.** El nodo decide qué comparte según **sus** señales (memoria libre, temperatura, batería, carga, red). El primary **no** adivina: consume el `node cert` que el nodo publica. |
| DN-3 | **La colocación es global.** El primary decide **dónde** va cada tarea (prioridad → clase → holgura → crédito), pero **sólo entre nodos que han dicho que sí** (filtro duro `accepts`). |
| DN-4 | **Aportar es medible y con consecuencias.** El ledger de contribución (§9.7) es la fuente de verdad. Un nodo que sólo consume pierde prioridad; si el desahogo se prolonga, deja de recibir trabajo (reversible, automático). |
| DN-5 | **Nada de jobs traducidos a "carga" ciega.** El reparto se hace por **clase de trabajo** (§9.1), que es lo que un humano entiende: "este DEX va al PC", "esta firma va al móvil", "esta inferencia va a la cuenta gratuita". |

### 9.1 Las clases de trabajo

Cada tarea lleva una **clase**. La clase determina quién puede hacerla, cuánto
cuesta y cómo se contabiliza la contribución.

| Clase | Qué es | Quién puede | Coste | `mode` | Peso de contribución |
|---|---|---|---|---|---|
| `inference` | Inferencia LLM: planes de `agent_core`, redacción, resúmenes, reintentos | PC (cuentas del `delegate`), móvil (NPU/GPU local **sólo** con `allow_heavy`), cuentas gratuitas | tokens / RAM | `local` \| `remote` | 2 |
| `build` | Compilación Rust (`cargo`), empaquetado de artefactos | **Sólo el PC** con toolchain. El móvil **nunca** (D-10, sin NDK en producción) | CPU + IO altos | `local` | 4 |
| `compute` | RE pesada: DEX, rizin, binwalk, desofuscación, entropía, diff de binarios, análisis de PDFs | PC | CPU + RAM altos | `local` | 4 |
| `network` | Red: egress, scraping, descargas, verificación de URLs, proxies | PC con IP estable; workers externos | ancho de banda + riesgo de bloqueo de IP | `remote` | 3 |
| `storage` | Verificación por hash, réplicas, backups, catalogación de artefactos | Cualquiera. En el móvil **sólo** hash + manifiesto (el disco es escaso) | disco | `local` | 1 |
| `device` | Root, ADB, Magisk, batería/Doze, backup de apps, grabador, **firma (PKCS#11/DNIe)** | **Sólo el móvil** (raíz real, keystore, TEE) | sensible: toca el dispositivo del usuario | `local` | 2 |
| `review` | Revisión, verificación, auditoría, redacción de informes | Cualquiera (persona o IA) | baja | `remote` | 1 |
| `coord` | Decidir, arbitrar, colocar tareas, mantener el board y el ledger | **Sólo el primary** | baja | `local` | 1 |

**Reglas de las clases:**

| # | Regla |
|---|---|
| DC-1 | La clase se declara en la tarea (`tasks.capability` con prefijo `class:` o campo `payload.class`). Sin clase ⇒ `review` (la más barata y segura). |
| DC-2 | Las clases `device` y `coord` **nunca** se asignan a un nodo que no sea el que las puede hacer: el filtro es duro (`classes` del cert), no una penalización. |
| DC-3 | `build` **jamás** va al móvil. Si el único nodo capaz es el móvil, la tarea **queda en cola** con el motivo `no_candidate:build`. No se degrada a "se hace en el móvil". |
| DC-4 | `inference` tiene dos modos: `local` (NPU/GPU del propio nodo: cara en RAM, sólo con `allow_heavy`) y `remote` (cuenta gratuita del `delegate`: casi gratis para el nodo). **El `mode` lo elige el nodo al publicar su cert**, según su holgura. |
| DC-5 | La clase `network` sólo se asigna a nodos **fuera** de la LAN del usuario (PC o externos), para no exponer la IP doméstica. |

### 9.2 El certificado de nodo (`node cert`)

**Qué es:** un documento JSON **firmado** que un nodo publica para decir
"esto es lo que soy y esto es lo que puedo hacer **ahora**". No es un
documento estático: cambia con el estado del dispositivo.

**Formato (canónico):**

```json
{
  "schema": "mdcert/1",
  "node": "samsung-s21-ultra@home",
  "instance_id": "samsung-s21-ultra@home",
  "role": "primary",
  "seq": 42,
  "published_at": "2026-11-19T22:30:00Z",
  "expires_at": "2026-11-19T22:33:00Z",
  "rules_hash": "sha256:8f2c1a…",
  "classes": [
    { "id": "inference", "modes": ["remote"], "cost": 1 },
    { "id": "storage",    "modes": ["local"],  "cost": 1 },
    { "id": "device",     "modes": ["local"],  "cost": 1 },
    { "id": "review",     "modes": ["remote"], "cost": 1 },
    { "id": "coord",      "modes": ["local"],  "cost": 1 }
  ],
  "accepts": ["review"],
  "offloads": ["compute", "build"],
  "headroom": {
    "mem_available_mb": 640,
    "cpu_load_pct": 22,
    "disk_free_mb": 11200,
    "battery_pct": 71,
    "charging": false,
    "thermal_hot": false,
    "allow_heavy": false,
    "net_up": true,
    "active_tasks": 1
  },
  "capacity": { "inference": 1, "storage": 1, "device": 1, "review": 2, "coord": 1 },
  "reserved_for_user": { "mem_mb": 1200, "cpu_pct": 30, "min_battery_pct": 20 },
  "dedicated": [],
  "signature": "hmac-sha256:6b1f…"
}
```

**Campos y reglas:**

| Campo | Regla |
|---|---|
| `node` | Identificador estable del nodo (`<device>@<host>`). Debe coincidir con `instance_id` registrado. |
| `seq` | Contador **monótono** por nodo. El primary rechaza un cert con `seq` ≤ el último recibido (`409 conflict`, `details.reason="stale_seq"`). |
| `expires_at` | **TTL 180 s**. Un cert vencido ⇒ el nodo **no** recibe trabajo (se trata como `stale`). |
| `rules_hash` | `sha256` del fichero de reglas locales (§9.3). Permite saber si dos nodos ejecutan la misma política y detectar drift. |
| `classes` | Lo que el nodo **puede** hacer (filtro duro). Cada entrada con `modes` (DC-4) y `cost` (1-5, relativo). |
| `accepts` | Lo que el nodo **quiere** hacer ahora (filtro duro, derivado de sus reglas locales). |
| `offloads` | Lo que el nodo **delega** en otros: es la lista de clases que el nodo publicará en el board en vez de ejecutarlas localmente. |
| `headroom` | Estado real medido (§9.3, tabla de señales). |
| `capacity` | Concurrencia máxima **por clase** que el nodo admite ahora. |
| `reserved_for_user` | Lo que el nodo **se guarda** para el usuario: nunca se delega ni se usa para trabajo ajeno. |
| `dedicated` | Clases reservadas en exclusiva (nodo "dedicado", §9.8). |
| `signature` | HMAC-SHA256 del cuerpo con la **clave del nodo** (`<state_dir>/node_cert.key`, 0600, 32 bytes aleatorios). El primary la verifica con la clave que el nodo registró en su alta (TOFU sobre canal autenticado). |

**Cadencia de publicación:**

| Regla | Detalle |
|---|---|
| DC-6 | **Cada 60 s**, y **de inmediato** al cambiar una señal de disparo (`mem_available_mb` cruza un umbral, `thermal_hot` pasa a `true`, `allow_heavy` pasa a `false`, `charging` cambia). |
| DC-7 | En el móvil la publicación cuesta **3 lecturas de ficheros** (`/proc/meminfo`, `/proc/loadavg`, `power_policy`) + 1 HMAC: **sin proceso extra, sin polling agresivo** (cumple CG-1). |
| DC-8 | El cert se publica por `POST /api/v2/certs/publish` (R-37) con `Idempotency-Key` = `cert:<node>:<seq>`: reintentar no duplica. |
| DC-9 | **El cert es una declaración, no una prueba.** El primary lo contrasta con `capability::verify()` (§9.5) y con el resultado medido de las tareas (§9.7). |

### 9.3 Las reglas locales

**Dónde:** `<state_dir>/rules/share_rules.json` — un fichero por nodo, escrito a
mano o por el operador, **evaluado localmente**. Formato **JSON** (el árbol de
crates no tiene parser de TOML ni YAML: `serde_json` es lo que hay; no se
añade una dependencia nueva por esto).

```json
{
  "schema": "mdrules/1",
  "node": "samsung-s21-ultra@home",
  "version": 3,
  "reserve_for_user": { "mem_mb": 1200, "cpu_pct": 30, "min_battery_pct": 20 },
  "rules": [
    {
      "id": "r-busy-offload-heavy",
      "priority": 100,
      "when": { "mem_available_mb": { "lt": 1200 } },
      "then": { "accepts": [], "offloads": ["compute", "build", "network"],
                "reason": "mem_bajo: el dispositivo se reserva al usuario" }
    },
    {
      "id": "r-thermal-offload-all",
      "priority": 95,
      "when": { "thermal_hot": { "eq": true } },
      "then": { "accepts": [], "offloads": ["compute", "build", "inference"],
                "reason": "thermal: nada de trabajo pesado" }
    },
    {
      "id": "r-battery-low-offload",
      "priority": 90,
      "when": { "battery_pct": { "lt": 20 }, "charging": { "eq": false } },
      "then": { "accepts": ["review"], "offloads": ["compute", "build"],
                "reason": "bateria baja sin cargador" }
    },
    {
      "id": "r-idle-accept-review-and-inference",
      "priority": 50,
      "when": { "mem_available_mb": { "gte": 2500 }, "cpu_load_pct": { "lt": 40 } },
      "then": { "accepts": ["review", "inference"], "offloads": [],
                "reason": "holgado: el movil aporta capacidad" }
    },
    {
      "id": "r-default",
      "priority": 0,
      "when": {},
      "then": { "accepts": ["review"], "offloads": ["compute", "build"],
                "reason": "por defecto: solo revision barata" }
    }
  ]
}
```

**Semántica de la evaluación:**

| Regla | Detalle |
|---|---|
| DR-1 | Las reglas se evalúan **por `priority` descendente**. **Gana la primera que casa** (no se acumulan): una decisión, una razón, auditable. Si ninguna casa ⇒ `defaults` (o la regla de `priority` 0). |
| DR-2 | `when` es una conjunción de condiciones; `{}` casa siempre. |
| DR-3 | Operadores por condición: `lt`, `lte`, `gt`, `gte`, `eq`, `ne`, `in`, `not_in`. |
| DR-4 | `then` produce el estado efectivo del nodo: `accepts` (clases que acepta), `offloads` (clases que delega), `reason` (texto que se publica en el cert y en la UI). |
| DR-5 | `reserve_for_user` es un **piso**: ningún nodo baja de esa reserva para hacer trabajo ajeno. Si `headroom` < reserva ⇒ `accepts` se vacía automáticamente, aunque la regla diga otra cosa. |
| DR-6 | El `rules_hash` (sha256 del fichero) va en el cert. Si el primary ve dos certs del mismo nodo con `rules_hash` distintos ⇒ deriva de política: evento `rules_drift` (aviso, no error). |

**Señales disponibles (todas ya existen en el árbol, ninguna nueva):**

| Señal | Origen | Nota |
|---|---|---|
| `mem_available_mb` | `/proc/meminfo` | Ya se lee en `cockpit_server.rs` |
| `cpu_load_pct` | `/proc/loadavg` | Ya se lee en `cockpit_server.rs` |
| `disk_free_mb` | `statvfs` del `state_dir` | |
| `battery_pct`, `charging`, `battery_temp_c` | `power_policy::read_power_state()` | |
| `thermal_hot`, `allow_heavy` | `power_policy::policy_for()` | `allow_heavy=false` ⇒ sin trabajo pesado (ya implementado) |
| `active_tasks` | `task_scheduler::stats()` | |
| `net_up` | comprobación de la ruta al primary | |
| `screen_on`, `dozing` | `power_policy` / `android` (best effort) | |

### 9.4 El ciclo: de las reglas al reparto

```
cada 60 s (o al cruzar un umbral de disparo):
   1. leer señales           (3 ficheros, sin forks)
   2. evaluar reglas locales  -> {accepts, offloads, reason}
   3. calcular clases posibles: capabilities ∩ reglas ∩ energía
        (device/build/npu se desactivan si !allow_heavy)
   4. firmar (HMAC) y publicar node cert  -> POST /api/v2/certs/publish
   5. el primary lo guarda (dist.sqlite + certs.ndjson) y lo usa al colocar

cuando el nodo tiene trabajo local que no puede o no quiere hacer:
   6. publica la tarea en el board con class=..., priority=...
      -> la coloca otro nodo; el resultado vuelve por bus + files (§8)

cuando el nodo termina trabajo (propio o asignado):
   7. report + verificación
   8. acumular contribución  -> contributions.ndjson
```

| Regla | Detalle |
|---|---|
| DR-7 | **El nodo puede delegar trabajo que ya tiene**: si una tarea local es de clase `compute` y el nodo está `busy`, el nodo la **publica** en el board con `owner_hint` vacío y la ejecuta quien pueda. El nodo no se bloquea esperando. |
| DR-8 | **Delegar no es perder**: la tarea delegada vuelve al nodo como informe verificado y cuenta como **contribución** del nodo que delegó (peso de la clase `coord`). |
| DR-9 | El `reason` de la regla que ganó se propaga al `event` del board (`kind="task_offloaded"`, `payload.reason`) ⇒ se puede auditar por qué una tarea fue a otro nodo. |

### 9.5 Verificación y confianza en el cert

| Regla | Detalle |
|---|---|
| DV-1 | El primary **verifica la firma** HMAC. Un cert sin firma válida ⇒ `403 forbidden` (`details.reason="bad_signature"`). |
| DV-2 | El primary **no se fía** del `classes` del cert: al asignar una tarea de la clase X a un nodo, comprueba que el nodo **declara** esa capacidad en `capability.rs` (`capability::verify()` ya verifica binarios, rutas y envs en el dispositivo) y que la **tarea se completa bien**. Un nodo que falla repetidamente una clase pierde esa clase (§9.7). |
| DV-3 | La **firma protege la integridad**, no la veracidad: un nodo puede declarar que está libre y no estarlo. Por eso `headroom` es una *declaración* y el sistema se apoya en el **resultado medido** (tiempos, fallos) para ajustar el crédito. |
| DV-4 | **Rotación de la clave del cert**: republicar con `seq+1` y `"rotated": true`; el primary acepta la nueva clave **sólo** por el canal autenticado (`X-Seere-Auth-Token`) y registra el evento `cert_key_rotated`. |
| DV-5 | Un nodo **no puede** publicar un cert con `node` de otro: el `node` del cert debe coincidir con el `agent_id`/`instance_id` de la API key usada. |

### 9.6 Ofertas y peticiones

**Oferta (`offer`):** capacidad libre que un nodo anuncia. Es el mecanismo de
"el móvil aporta cuando está libre" y el de "el PC ofrece red cuando no está
compilando".

```json
{
  "schema": "mdoffer/1",
  "offer_id": "off-20261119-0007",
  "node": "samsung-s21-ultra@home",
  "class": "inference",
  "mode": "remote",
  "units": 1,
  "ttl_s": 300,
  "created_at": "2026-11-19T22:30:00Z",
  "note": "NPU no disponible (allow_heavy=false); ofrezco inferencia remota con cuenta gratuita"
}
```

| Regla | Detalle |
|---|---|
| DO-1 | Una oferta **caduca**: `expires_at = created_at + ttl_s` (máx. 600 s). El primary la borra sola. |
| DO-2 | El primary **empareja** ofertas con peticiones (tareas sin nodo elegible) por clase, `mode` y `units`. |
| DO-3 | Una oferta **usada** cuenta como contribución del nodo que la hizo (§9.7). Una oferta **no usada** no cuenta (si no, "ofrecer" sería gratis y el sistema mentiría sobre la carga real). |
| DO-4 | El nodo que ofrece **no** ejecuta la tarea que ofrece: la ejecuta el nodo que la pide. La oferta es capacidad del oferente, no una auto-tarea. |

### 9.7 El ledger de contribución y la regla "cada nodo aporta"

**Unidad de contribución:** una línea por aportación verificable.

```json
{"schema":"mdcontrib/1","ts":"2026-11-19T22:41:00Z","id":"cont-000123","node":"pc-linux@lab","class":"compute","units":4,"kind":"task.completed","task_id":"t-018","evidence":"sha256:9ab3…","by":"agent-dex-01"}
```

| `kind` | Cuenta como | Peso |
|---|---|---|
| `task.completed` | Tarea completada y verificada | peso de la clase (§9.1) |
| `artifact.published` | Artefacto publicado con sha256 verificado | 1 |
| `review.done` | Revisión entregada | 1 |
| `offer.used` | Capacidad ofrecida y consumida por otro nodo | 1 |
| `coord.decision` | Arbitraje, colocación de tareas, promoción | 1 |
| `task.delegated` | Tarea local delegada a un nodo capaz | 1 (peso `coord`) |
| `task.failed` | **No cuenta** (0). Se registra con `units:0` para la métrica de fallo | 0 |

**Cálculo del crédito (por nodo, ventana de 24 h por defecto):**

```
credit(nodo)    = Σ unidades aportadas
consumo(nodo)   = Σ coste de las tareas que el nodo ejecutó
ratio(nodo)     = credit / (credit + consumo)      # 0 si no hay nada
```

**Reglas del ledger:**

| # | Regla |
|---|---|
| DL-1 | El ledger es **append-only** (`<state_dir>/contributions.ndjson`) y su índice es `dist.sqlite` (reconstruible). |
| DL-2 | Una aportación con `evidence` ausente **no cuenta** (units = 0). "Evidencia o nada" (regla 9 de `MASTER_PLAN.md`). |
| DL-3 | El credit se usa como **desempate con prioridad** en §5.2 (posición b). No es un bloqueo. |
| DL-4 | **Desahogo**: un nodo con `ratio < 0,2` que además tiene peticiones de trabajo pendientes ⇒ el primary le **reasigna** esas tareas a otros nodos y escribe el evento `task_reassigned` con `payload.reason="low_contribution"`. |
| DL-5 | **Cuarentena**: un nodo con `ratio < 0,2` durante **48 h** consecutivas ⇒ deja de recibir tareas (`cert.accepts` se ignora para él) y se marca `quarantined` en `GET /api/v2/certs`. **Reversible**: en cuanto vuelve a aportar, se levanta sola. |
| DL-6 | **El primary está exento** de DL-4 y DL-5: su contribución es arbitrar, colocar, servir y mantener el estado. Si no, el coordinador se aislaría a sí mismo. |
| DL-7 | Un nodo en `observer` **no consume** nada ⇒ ratio 1.0 por definición ⇒ nunca se pone en cuarentena. |
| DL-8 | La cuarentena **nunca** borra datos ni revoca credenciales: es un filtro de asignación. |

### 9.8 Nodos "dedicated" y nodos "free"

**Nodo `dedicated`:** un nodo cuya capacidad principal está reservada.

| Regla | Detalle |
|---|---|
| DD-1 | `cert.dedicated = ["compute"]` significa: **sólo** se le asigna `compute`. Cualquier otra clase queda fuera aunque su `accepts` la incluya. |
| DD-2 | La contribución de un nodo `dedicated` se cuenta con su clase reservada; no necesita buscar trabajo fuera de ella. |
| DD-3 | Un nodo `dedicated` **no se penaliza** si sólo sirve su clase: el ratio se calcula sobre lo que efectivamente puede aportar. |
| DD-4 | "Dedicated" lo declara el nodo, no el primary. El primary lo respeta (filtro duro) y lo muestra en la UI. |

**Nodo `free` (el modo del móvil cuando el usuario lo necesita):**

> *"if the device requires free processing for other stuff"* ⇒ el dispositivo
> necesita CPU/RAM libres para otra cosa ⇒ **no** se usa su capacidad para
> trabajo ajeno, **se delega** lo que pueda y **se acepta** sólo lo barato.

| Situación del móvil | `accepts` | `offloads` | Por qué |
|---|---|---|---|
| `mem_available < 1200 MB` (usuario usando el teléfono) | `[]` | `compute`, `build`, `network` | El móvil se reserva al usuario (DR-5) |
| `thermal_hot` o `!allow_heavy` | `[]` | todo lo pesado | `power_policy` ya lo prohíbe |
| `battery < 20 %` sin cargador | `["review"]` | `compute`, `build` | Ni una sola llamada de red |
| `mem ≥ 2500 MB`, `cpu < 40 %`, `battery ≥ 40 %` | `["review", "inference"]` | `[]` | **El móvil aporta**: revisión e inferencia remota (barata para él) |
| `charging` y `mem ≥ 3500 MB` | `["review", "inference"]` + `inference.local` | `[]` | Cargando y con holgura: puede usar la NPU (`allow_heavy`) |

**Reglas del nodo `free`:**

| # | Regla |
|---|---|
| DF-1 | El móvil **nunca** ejecuta `build` (DC-3) ni `compute` en modo local, aunque pueda: no hay RAM, no hay toolchain y el usuario lo nota. |
| DF-2 | El móvil **sí** ejecuta `device` (es su clase natural) y `review`/`inference.remote` (coste casi nulo para él: es una llamada a una cuenta gratuita). |
| DF-3 | El móvil acepta **como máximo 1** tarea externa concurrente y **sólo** de las clases `review` e `inference` (por privacidad, DF-4). |
| DF-4 | **Privacidad**: el móvil sólo acepta trabajo cuyo `payload.shareable == true` y `zone == "public"`. Nunca envía al resto de la flota datos del usuario: ni conversaciones, ni APKs del usuario, ni capturas. Lo que el móvil produce para el enjambre es **resultado**, no **material privado**. |
| DF-5 | El móvil publica su `accepts` **reducido** cuando el usuario está mirando: la regla `r-busy-offload-heavy` cubre el caso; si además `screen_on == true`, el `accepts` se vacía (regla opcional `r-screen-on`). |

### 9.9 Qué NO se reparte

| Material / acción | Por qué no se reparte |
|---|---|
| Claves privadas, tokens, cookies, keystore | §8.5. El material **no** sale; se comparte la **capacidad** de usarlo. |
| Datos del usuario (conversaciones, APKs instaladas, capturas) | DF-4. El móvil no es un nodo de datos. |
| `device` en cualquier nodo que no sea el móvil | Requiere root real y keystore |
| `build` en el móvil | DC-3 |
| Acciones irreversibles (`confirm: true`) | §4.4 (AU-2) |
| `network` desde un nodo de la LAN del usuario | DC-5 (no exponer la IP doméstica) |

### 9.10 Presupuesto del móvil (amplía §5.5)

| Recurso | Tope | Motivo |
|---|---|---|
| Publicación de cert | 1 cada 60 s, 3 lecturas de ficheros | DC-6, DC-7 |
| Tareas externas concurrentes | **1** | DF-3 |
| RAM para una tarea externa aceptada | **≤ 150 MB** | Con `mem_available ≥ 2500 MB` hay margen de sobra |
| Trabajo local pesado | **0** | DC-3, DF-1 |
| Coste de la inferencia remota para el móvil | ~0 RAM (es una llamada HTTP a una cuenta gratuita) | `delegate` enruta a cuentas gratuitas |

> **Conclusión**: para el móvil, "aportar" es barato si se hace por la vía
> correcta: **revisar, delegar y arbitrar**, no **calcular**. La clase `build`
> y `compute` son, por diseño, trabajo del PC.

### 9.11 Contrato HTTP y MCP de la capa de distribución

| Ruta | Método | Auth | Función |
|---|---|---|---|
| R-37 `/api/v2/certs/publish` | POST | api-key+token | Publicar el cert propio |
| R-38 `/api/v2/certs` | GET | api-key | Listar certs (filtro `class`, `stale`) |
| R-39 `/api/v2/certs/{node}` | GET | api-key | Cert + `rules_hash` + crédito |
| R-40 `/api/v2/rules` | GET | api-key | Reglas efectivas por nodo |
| R-41 `/api/v2/offers` | POST | api-key+token | Publicar oferta de capacidad libre |
| R-42 `/api/v2/offers` | GET | api-key | Listar ofertas vivas |
| R-43 `/api/v2/contributions` | GET | api-key | Ledger y `fleet_ratio` |
| R-44 `/api/v2/placement` | POST | api-key+token | "¿dónde va esta tarea?" → nodo + razón + alternativas |

**MCP (proyección 1:1, §2.6):** `v2_certs_publish` (R-37),
`v2_certs_list` (R-38), `v2_offers_create` (R-41), `v2_offers_list` (R-42),
`v2_contributions_list` (R-43), `v2_placement_decide` (R-44).

**Errores específicos de esta capa:**

| `code` | HTTP | Cuándo |
|---|---|---|
| `stale_seq` | 409 | El cert publicado tiene `seq` ≤ el último conocido |
| `bad_signature` | 403 | La firma HMAC no valida |
| `cert_expired` | 409 | Se intenta asignar a un nodo con cert vencido |
| `quarantined` | 409 | El nodo está en cuarentena por DL-5 |
| `no_candidate` | 409 (en `task.completed`) | Ningún nodo puede tomar la clase pedida |
| `class_not_allowed` | 403 | La tarea pide una clase que el nodo no tiene (filtro duro) |

### 9.12 Criterios de aceptación de la capa de distribución

| # | Criterio (test o medición) |
|---|---|
| DC-10 | Publicar un cert con `seq` anterior ⇒ `409 stale_seq`; con firma inválida ⇒ `403 bad_signature`. |
| DC-11 | Las reglas se evalúan **por prioridad** y **la primera que casa gana**: con `mem_available_mb=800` y `thermal_hot=false`, el `accepts` efectivo es `[]` y el `reason` es el de `r-busy-offload-heavy`. |
| DC-12 | Con `mem_available_mb=3000`, `cpu_load_pct=10`, `battery_pct=80` ⇒ `accepts=["review","inference"]` y `offloads=[]` (el móvil **ofrece** capacidad). |
| DC-13 | Una tarea de clase `build` **nunca** se asigna al móvil: `POST /api/v2/placement` con `class:"build"` devuelve un nodo del PC o `409 no_candidate`. |
| DC-14 | Una tarea de clase `device` **sólo** se asigna al móvil. |
| DC-15 | El ledger suma unidades con `evidence` y **descarta** las que no lo tienen (`units` efectivo = 0). |
| DL-9 | Un nodo con `ratio < 0,2` y trabajo pendiente ⇒ el primary reasigna y escribe `task_reassigned` con `reason="low_contribution"`. |
| DL-10 | Tras 48 h de ratio bajo ⇒ el nodo aparece `quarantined` en `GET /api/v2/certs` y **no** recibe tareas; al volver a aportar, se levanta **sin intervención humana**. |
| DL-11 | El primary **no** entra nunca en cuarentena (DL-6). |
| DF-6 | El móvil nunca acepta una tarea cuyo `payload.shareable != true` (DF-4). |
| DF-7 | Con `screen_on == true` y `mem_available_mb=900`, `accepts` es `[]` (el usuario está mirando el teléfono). |

## 10. UNIFICACIÓN: CONTRATO ↔ ESPECIFICACIÓN ↔ CRÍTICA ADVERSARIA

> Esta sección responde a una pregunta práctica: **"¿esto lo usan todos los
> agentes?" y "¿se puede unificar con la crítica adversaria que está haciendo
> otro agente?"**. La respuesta corta: *no se usa aún* (hechos en §10.1) y
> *sí se puede unificar*, pero la unificación tiene que ser **estructural**
> (tres capas con precedencia), no una simple fusión de texto (§10.2-§10.4).

### 10.1 Estado real: qué se está usando y qué no (medido 2026-11-19 22:31)

| Hecho | Evidencia | Consecuencia |
|---|---|---|
| H-1 | `docs/CONCURRENCY_PLAN_V15.md` está **sin commitear** (`??` en `git status`) | Un agente que arranque con `git pull` **no lo ve**. Un fichero sin commitear es trabajo en riesgo (D-11: 23 ficheros perdidos por eso). |
| H-2 | `docs/API_V2.md` (21:30), `rust-core/src/webui.rs` (21:27), `web/cockpit_v2.html` (21:27), `web/webui_v2.css` (21:22) los escribió **otro agente** con **otro vocabulario de rutas** y **otro envelope** | El contrato §2 (44 rutas, envelope `error{code,…}`) y la implementación (20 rutas, envelope `ok/error`) **eran incompatibles** hasta la unificación de §2.1.1 y §2.2. |
| H-3 | `pub mod webui;` **ya está** en `lib.rs` (21:28) | La capa v2 **compila**; la UI v2 ya habla con ella. |
| H-4 | La crítica adversaria tiene `docs/AUDIT_ADVERSARIAL_V15.md` (73.846 B, 21:37) con **81 hallazgos** en 10 áreas (A…J) y `rust-core/tests/audit_adversarial_v15.rs` con **15 tests** que reproducen los P0/P1 | La crítica **sí** está en documento, pero **sin veredicto**: los 81 hallazgos no están mapeados a ninguna sección del contrato ni priorizados como tareas. Eso es lo que hace §10.4. |
| H-5 | El único tablón vivo es `docs/COORDINATION.md` (42 KB, actualizado 21:24 y 21:32) | El protocolo **de facto** del enjambre sigue siendo: tablón + `git add <ruta>` + commits. **Este contrato no es aún vinculante.** |
| H-6 | `board.sqlite` sigue sin existir en el repo ni en el móvil (D-13) | "El plan vive en el demonio" (regla 1 de `MASTER_PLAN.md`) **todavía no es cierto**: el plan vive en el markdown y en los commits. |
| H-7 | Existen **4 documentos de diseño** simultáneos y sin referencias cruzadas: `CONCURRENCY_PLAN_V15.md`, `API_V2.md`, `UI_V2_SPEC.md` (21:32), `AUDIT_ADVERSARIAL_V15.md` (pendiente) | Es el riesgo exacto de NP-1/NP-6 aplicado a los documentos: **cuatro verdades** en vez de una. |

**Veredicto honesto:** de las 8 secciones del contrato, la 1, la 3, la 4 y la
8 ya se **practican** (los módulos existen y funcionan), pero la 2 **no se
estaba usando** (cada agente diseñó su propia superficie HTTP) y la 5 y la 6
son sólo intención. El contrato **no es un contrato todavía**: es una
especificación que nadie está obligado a cumplir.

### 10.2 Las tres capas y su precedencia (normativo)

| Capa | Documento | Quién escribe | Qué decide | Puede contradecir a otra capa |
|---|---|---|---|---|
| **1. CONTRATO** | `docs/CONCURRENCY_PLAN_V15.md` (este) | el primary (o quien él designe, NP-8) | **Qué es correcto**: qué rutas existen, qué envelope, qué reglas de no pisado, qué clase de trabajo, qué puede automatizarse. | **Sí**: es la capa que decide. |
| **2. ESPECIFICACIÓN** | `docs/API_V2.md` (capa v2), `docs/UI_V2_SPEC.md` (UI) | dueño de `webui.rs`, dueño de la UI | **Cómo se implementa** la capa 1: firmas, JSON exacto, tests, CSS. | No sobre la capa 1: si contradice, se abre una objeción (§10.3). |
| **3. OBJECIONES** | `docs/AUDIT_ADVERSARIAL_V15.md` (pendiente) | el agente de crítica adversaria | **Que el contrato es incorrecto o incompleto**: un defecto reproducido con un test. | No: **no edita** el contrato; propone. |

| # | Regla de unificación |
|---|---|
| UN-1 | **Cada capa tiene un dueño y una función**. Ninguna copia el contenido de otra: la especificación describe la implementación; la crítica señala secciones por id; el contrato decide. |
| UN-2 | **Precedencia**: ante conflicto, gana la capa 1 (contrato) **salvo** que la capa 3 demuestre con un test que la capa 1 es incorrecta ⇒ entonces gana la objeción hasta que el primary resuelva. |
| UN-3 | **La capa 2 nunca renombra lo implementado** (EQ-1): si la capa 1 pide un nombre y la capa 2 ya tiene otro funcionando, la capa 1 **se corrige** (mapa de equivalencia §2.1.1), no al revés. |
| UN-4 | **La capa 3 no edita** `CONCURRENCY_PLAN_V15.md` ni `API_V2.md`: escribe **objeciones** con id `<área>-<n>` (la convención que ya usa). Esto evita el pisado de documentos entre el crítico y el primary. |
| UN-5 | Toda objeción **abierta** aparece en la tabla de §10.4. Una objeción **aceptada** produce un cambio en el contrato **con entrada en el registro de cambios** (§10.5). Una objeción **rechazada** conserva su motivo. |
| UN-6 | **Un test de la crítica que falla es una tarea de trabajo**, no un documento: entra en el tablero (`md-coord add`) con `owner` = el dueño del módulo DEFECTUOSO y `payload.adv = "<área>-<n>"`. El contrato **no** reescribe el código. |
| UN-7 | La crítica **no** critica lo que no está medido: una objeción sin test reproducible se marca `pendiente` y no bloquea (regla 9 de `MASTER_PLAN.md`: evidencia o nada). |

### 10.3 Formato de una objeción (el que la crítica YA usa)

> La crítica adversaria ya ha establecido su propia convención de ids en
> `docs/AUDIT_ADVERSARIAL_V15.md`: **`<área>-<n>`** (`A-01` … `J-07`), con
> áreas fijas y severidad `P0`–`P3`. **Se adopta esa convención tal cual**
> (mismo criterio que con el envelope: no se reescribe lo que ya funciona y
> está escrito en 74 KB de documento).

| Campo | Regla |
|---|---|
| `id` | `<área>-<n>`. Las áreas ya definidas por la crítica: **A** seguridad · **B** credenciales en claro · **C** corrección · **D** fiabilidad · **E** rendimiento y recursos · **F** arquitectura · **G** cobertura de tests · **H** agente externo y conversor · **I** UX de la UI v1 · **J** límites honestos (no son defectos: son restricciones reales). |
| `severity` | `P0` (bloquea/seguridad) · `P1` (alto) · `P2` (medio) · `P3` (bajo). |
| `targets` | **Ids de sección del contrato** (`2.2`, `9.7`, `8.3`…). La crítica debe añadir esta columna al escribir nuevas objeciones; las 81 existentes se mapean en §10.4. |
| `evidence` | `test` (ruta + nombre del test + resultado) · `measurement` (comando + salida) · `manual`. Sin evidencia ⇒ `pendiente`. |
| `proposed_change` | El cambio **concreto** que se pide (al contrato, al código, o a un script de despliegue). |
| `owner` | El dueño del **módulo defectuoso**, no el del contrato. |
| `status` | `open` · `accepted` · `rejected` · `deferred` · `fixed`. |
| `verdict` | Lo emite **sólo** el dueño de la capa 1 (primary). Se escribe en el registro de §10.4, nunca en el fichero de la crítica. |

**Nota sobre la convención:** este contrato usaba `ADV-<área>-<n>` en su
primera redacción. Se **abandona** ese prefijo en favor del de la crítica
(`A-01`): una sola convención de ids para todo el enjambre. Los ids de la
crítica **no se renumeran** (son 81 referencias ya escritas en un documento de
74 KB y en 15 tests).


### 10.4 Registro de objeciones (tabla viva, en este contrato)

Estado a las 22:40 del 2026-11-19: la crítica adversaria ha entregado
`docs/AUDIT_ADVERSARIAL_V15.md` (**73.846 B**) con **81 hallazgos**
(74 con severidad, más 7 en el área J = límites honestos, no defectos) y
`rust-core/tests/audit_adversarial_v15.rs` con **15 tests** que reproducen
los P0/P1 principales. **Ninguno tiene veredicto todavía**: los resuelve el
primary, uno a uno.

#### 10.4.1 Los 8 P0 (bloqueantes)

| `id` | Hallazgo | Sección del contrato | Veredicto | Acción resultante |
|---|---|---|---|---|
| `A-01` | `fs_write` mete el contenido crudo del usuario dentro de un heredoc `<<'__MD_EOF__'` ⇒ **las líneas siguientes se ejecutan como shell root** | §4.4 (AU-4), §8.5 | `pendiente` | Regla nueva: el contenido **nunca** se interpola en un argv de shell; escribir con `std::fs::write` y el `chmod` aparte |
| `A-02` | `find_duplicates` ejecuta el **nombre del fichero** dentro de un AWK ⇒ RCE root en 2 pasos | §4.4 (AU-4), §8.5 | `pendiente` | Igual que A-01: argv sin shell |
| `A-04` | `MEGADAEMON_REQUIRE_API_KEY=1` no fuerza nada si no hay key ⇒ **fail-open del túnel público** | §2.4 (orden de evaluación) | `pendiente` | §2.4 exige fail-closed: si `forced`, exigir key y autogenerarla |
| `A-07` | El intercambio de token de sesión es inalcanzable (está en la rama `GET`, y `requires_auth` lo exige) ⇒ **la web del móvil devuelve 403 en todo** | §2.4 (doble auth) | `pendiente` | Mover a `POST` y eximirlo de `requires_auth` |
| `A-15` | El puente web inyecta API key **y** token de sesión a cualquier cliente de la LAN y escucha en `0.0.0.0:80` | §2.4, §8.5, §10.6 | `pendiente` | El puente no inyecta el token; `MD_WEB_BIND` = `127.0.0.1` por defecto |
| `B-01` | API key real (`mdaemon_sk_live_<prefijo redactado>`) y token root publicados en el árbol de trabajo | §8.5, §10.6 (AD-1) | `pendiente` | **Rotar ya** (es la decisión 4 de `MASTER_PLAN.md` §8, aún abierta) |
| `B-02` | URL y token de un túnel Cloudflare en claro en `tools/upload_bin.py` (**versionado**) | §8.5 | `pendiente` | Borrar el token del fichero; moverlo a `md-secrets/` |
| `I-01` | La UI v1 sólo sabe usar `window.MEGADAEMON_TOKEN` y la v14 lo inyecta vacío ⇒ **la web no tiene ninguna vía de autenticación** | §2.1 (AP-8), §2.4 | `pendiente` | La v2 (`cockpit_v2.html`) ya lo resuelve; la v1 queda congelada |

#### 10.4.2 Resto de hallazgos por área (P1/P2/P3), con su destino en el contrato

| Área | Ids | Qué agrupa | Sección(es) del contrato | Veredicto |
|---|---|---|---|---|
| **A. Seguridad** | `A-03`, `A-05`, `A-06`, `A-08`, `A-09`, `A-10`, `A-11`, `A-12`, `A-13`, `A-14`, `A-16` | política destructiva weak, rate limit por IP en vez de por agente, `confirm` incompleto, catch-all 200, sondeo a `:8080`, puertos alias sin 8080, SEERE ejecuta root sin `is_destructive`, lectura arbitraria de ficheros, comparación no constante, log en `/sdcard`, **64×8 MB = 512 MB de buffers** | §2.4 (AL-1…AL-6, orden de evaluación), §4.4 (AU-3/AU-4: `confirm`), §5.5 (**≤16 conexiones**, ≤320 MB RSS), §2.1 (AP-7: 404 vs 501), §8.5 (logs fuera de `/sdcard`) | `pendiente` (11) |
| **B. Credenciales en claro** | `B-03`, `B-04`, `B-05` | el repo publica IP de LAN y URL del túnel; la key se busca en rutas legibles; el registro de claves se escribe troncando | §8.5, §10.6 (AD-1: commitear sin secretos) | `pendiente` (3) |
| **C. Corrección** | `C-01`…`C-10` | tokens estimados con `chars/4`; `unwrap_or_default()` convierte "no se pudo leer" en "vacío"; el daemon reporta **tres versiones** distintas; el mapa de estados HTTP no conoce 429 ni 503; `Mutex::lock().unwrap()` envenena el mutex; 19 × `to_value().unwrap()`; `root_exec` no espera a los hilos lectores; bóveda corrupta que se re-copia en cada llamada; el comentario dice 100 MB y el tope real son 8 MB | §2.2 (**"si un dato no existe, la respuesta lo dice"**), §4.6 (M-6: deriva de calidad), §5.5 (presupuesto), §2.3 (`limit` real vs comentado) | `pendiente` (10) |
| **D. Fiabilidad** | `D-01`…`D-08` | el supervisor hace `exit 0` tras 3 fallos de arranque; el despliegue nunca hace rollback (`verify || true`); "vivo pero muto"; `pgrep -f` en bucle de 5 s; log en FUSE; el tablero abre SQLite **sin `busy_timeout`**; el conversor trunca `findings.ndjson`; `run_server` nunca devuelve | §5.5, §3.2 (lease, `reclaim_expired`), §6 (Ola F: rollback y acuse de recibo), §8.2 (append-only del manifiesto) | `pendiente` (8) |
| **E. Rendimiento** | `E-01`…`E-08` | se releen key y bóveda de proveedores **en cada request**; 512 MB + 128 MB de pilas; un hilo por conexión ×2 listeners; sondeo ciego cada 12 s y 20 s; `audit.ndjson` sin rotar; 3 runners probados por `exec_root`; cada refresco de UI relee la bóveda | §5.5 (**presupuesto del móvil**), §9.10, §2.3, §3.5 (BU-2: el emisor no bloquea) | `pendiente` (8) |
| **F. Arquitectura** | `F-01`…`F-09` | **dos tableros** (`mesh.rs` JSON vs `coordination.rs` SQLite) — el solapamiento que `MASTER_PLAN.md` §8 dejó abierto; el catálogo MCP expone 27 tools y no las 17 v14; `cockpit_doctor` no está cableado en `main.rs`; `md-coord` no tiene `sync`; **120 KB de código sin compilar** (`webui.rs`, `signing_vault.rs`); tres directorios de estado distintos; rutas absolutas de Android en el núcleo; el router son dos `match` gigantes con rutas duplicadas | §3.2 (**el board SQLite es la autoridad**, `mesh.rs` = transporte), §2.6 (catálogo MCP 1:1), §6 (NP-9: todo módulo entra en `lib.rs` con arnés verde), §5.4 (migración), §10.1 | `pendiente` (9) |
| **G. Cobertura de tests** | `G-01`…`G-06` | el fichero de test que la auditoría v14 cita **no existe**; ficheros `zz_*_tmp` sin borrar; **ningún test cubre la matriz de autenticación**; con `panic=abort` no hay `catch_unwind`; coste de la suite | §2.1 (AP-7), §2.4 (**la matriz de auth es el test que faltaba**), §6 (W-2: todo criterio es un test), §6 (W-4) | `pendiente` (6) |
| **H. Agente externo** | `H-01`…`H-05` | el conversor no resuelve la colisión DEX/app_intel como se.documenta; **el doc dice que el token sólo hace falta en POST/PUT, pero toda llamada MCP es un POST**; el contrato "26 tools" no cuadra con el repo; cada llamada MCP hace 2 viajes con timeout de 900 s; docstring que no coincide con la firma | §2.4 (doble auth: **el MCP es POST ⇒ token siempre**), §2.6, §8.2 (compatibilidad del layout) | `pendiente` (5) |
| **I. UX v1** | `I-02`…`I-07` | no hay forma de meter la API key ni de ver 401/403; la subida falla a partir de ~6 MB con un 413 sin explicar; no se ve la versión ni el consumo; sondeo sin parar con la pestaña oculta; nada de la v2 visible; los errores no distinguen "daemon caído" de "403" | §2.1 (AP-8: v1 congelada, v2 canónica), §2.4 (errores legibles), §6 (Ola E: la UI v2) | `pendiente` (6) |
| **J. Límites honestos** | `J-01`…`J-07` | No son defectos: son cosas que **no se pueden hacer** (cookies del TEE, Play Integrity, almacén de WhatsApp, sin API de Play Store…). Ya están en `MASTER_PLAN.md` §3 (P3) | §1.4 (pendientes), §8.9, §9.9 | `informativo` (7) |

**Recuento:** 74 defectos con severidad (8 P0, 21 P1, 38 P2, 7 P3) + 7 límites
informativos = **81**. **0 con veredicto.**

**Los tres hallazgos que más pesan sobre este contrato** (y que obligan a
cambios reales, no cosméticos):

| Hallazgo | Qué obliga a cambiar en el contrato |
|---|---|
| `A-16` + `E-03` | §5.5 decía "≤16 conexiones y ≤320 MB RSS" **como si fuera una decisión de diseño**. La auditoría demuestra que hoy el código reserva **512 MB de buffers + 128 MB de pilas** en un móvil con ~300 MB libres: el presupuesto de §5.5 **no está aplicado en el código**. Hay que bajarlo en `cockpit_server.rs` (64 → 32 clientes, tope de cuerpo acumulado). |
| `A-05` | §2.4 (AL-1…AL-6) asumía que el rate limit por IP era el problema. La auditoría prueba que hoy **el loopback no tiene ningún cubo** ⇒ el límite está *desactivado*, no *mal dimensionado*. AL-1 (cubo por agente) deja de ser una mejora: es un **fix**. |
| `F-01` | §3.2 y §5.4 ya decidían que el board SQLite es la autoridad y `mesh.rs` es transporte, pero **los dos siguen vivos y duplicados** en el repo. La decisión de `MASTER_PLAN.md` §8 (pendiente desde hace horas) **no está ejecutada**: hay que **vaciar** una de las dos. |


### 10.5 Registro de cambios del contrato (consecuencia de las objeciones)

| Fecha | Origen | Cambio | Sección |
|---|---|---|---|
| 2026-11-19 | Unificación con `webui.rs` | Envelope v2.0 **adoptado** (`ok`/`error`/`detail`/`generated_at`) + `request_id` aditivo | §2.2 |
| 2026-11-19 | Unificación con `webui.rs` | Recuento de las 44 rutas frente a las 20 implementadas | §2.1.1 |
| 2026-11-19 | Decisión del usuario (*"sí a todo; renombra si compensa"*) | **Renombre v15.0 → v15.1**: el contrato adopta el vocabulario existente (`tasks`, `nodes`, `share`, `events`) en vez de crear familias paralelas (`board`, `bus`, `files`). **El código ajeno no se toca.** `route_id` estables. | §2.1, §2.1.1 |
| 2026-11-19 | Decisión del usuario | `POST /api/v2/mcp` **eliminado**: el endpoint MCP sigue siendo `POST /mcp` | §2.1, §2.6 |
| 2026-11-19 | Decisión del usuario | El contrato **se commitea** y **se apunta en el tablón** (`docs/COORDINATION.md`), una línea | §10.6 (Ola 0) |
| 2026-11-19 | Crítica adversaria | **81 hallazgos** (74 defectos + 7 límites) registrados por área, **pendientes de veredicto**; se adopta su convención de ids `<área>-<n>` (se abandona el prefijo `ADV-`) | §10.3, §10.4 |

### 10.6 Cómo pasa este contrato de "especificación" a "contrato"

Un documento sólo es vinculante si hay **tres cosas**: se ve, se cita y se
verifica.

| # | Mecanismo | Estado | Quién |
|---|---|---|---|
| AD-1 | **Commitear el documento** (ya existe `docs/EXTERNAL_AGENT_PROMPT_FINAL.md` y `API_V2.md` sin commitear: mismo riesgo) | ❌ pendiente | primary (`git add docs/CONCURRENCY_PLAN_V15.md` + commit) |
| AD-2 | **Una línea en `docs/COORDINATION.md`** (el tablón que *todos* leen al empezar, según su propia cabecera) que diga: "el contrato vigente es `docs/CONCURRENCY_PLAN_V15.md`; toda ruta nueva se declara en su tabla de rutas y en `GET /api/v2/routes`" | ❌ pendiente | primary (NP-7: una línea, al final) |
| AD-3 | **`GET /api/v2/routes` como tabla viva**: la implementación ya la publica; el contrato exige que toda ruta nueva aparezca ahí (EQ-4) | ⚠️ parcial (existe, no incluye las rutas del contrato) | dueño de `webui.rs` |
| AD-4 | **Gate de tablero**: toda tarea que toque la API v2 se registra en `board.sqlite` con `payload.route_id`; sin `route_id` no se ejecuta | ❌ pendiente (el board no existe, D-13) | coord |
| AD-5 | **Gate de test**: el criterio de aceptación de cada ruta es un test en `rust-core/tests/` (la crítica ya lo hace con `audit_adversarial_v15.rs`: 15 tests = 15 criterios) | ✅ en marcha | todos |
| AD-6 | **Gate de ownership**: `docs/AGENTS_OWNERSHIP.md` (B-5) lista cada `owner_files` ⇒ cada agente sabe qué puede tocar | ❌ pendiente | primary |

**Sin AD-1 y AD-2, este documento no es un contrato: es un documento más.**
Por eso son las dos primeras tareas, y por eso la primera decisión de §7.1
del plan maestro ("el plan vive en el demonio") no está cumplida.

### 10.7 Lo que la crítica adversaria aporta y este contrato NO

Honestidad sobre los límites del contrato:

| Lo que la crítica hace mejor | Por qué |
|---|---|
| **Prueba** en vez de opinar | Cada defecto tiene un test que **falla** con el código actual. Un contrato puede tener razón y aun así estar equivocado: el test lo demuestra. |
| Encuentra los **huecos** | `A-01`/`A-02` (inyección de comandos), `A-11` (SEERE ejecuta root sin `is_destructive`), `A-12` (lectura arbitraria de ficheros) y `C-02` (`unwrap_or_default`) **no** estaban cubiertos por este contrato. La crítica ha encontrado huecos reales, no sólo desacuerdos. |
| Da **criterios de aceptación** reutilizables | Los 15 tests son 15 criterios de aceptación que valen para el `Definition of Done` (§6 W-4). |
| Mide lo que el contrato daba por supuesto | `E-01`/`E-02`/`E-03` desmontan la suposición de §5.5 ("el móvil va justo de RAM") con números: el techo real son 512 MB + 128 MB de pilas. |

**Regla para el primary:** una objeción `accepted` **mejora el contrato**;
una objeción `rejected` **mejora el argumento** del contrato (y se escribe el
motivo, no se borra la objeción). En ninguno de los dos casos se borra la
prueba: el test que falla sigue en el repo hasta que el defecto se corrija.

---

## ANEXO A — Glosario de identificadores

| Identificador | Dónde vive | Quién lo crea | Semántica |
|---|---|---|---|
| `route_id` | tabla de rutas §2.1 | primary | Clave de una ruta v2. Estable entre versiones. |
| `agent_id` | `<state_dir>/agents/<agent_id>.json` | primary | `agent-<dominio>-<n>` (o `ext-<dominio>-<n>`). |
| `instance_id` | `<state_dir>/instance.json` | la instancia | `host:pid` o id estable del nodo. |
| `task_id` | `board.sqlite.tasks.id` | primary | Identificador de tarea del tablero. |
| `message_id` | `board.sqlite.bus_messages.id` | emisor | Identificador de mensaje del bus (dedup). |
| `failure_id` | `board.sqlite.failures.id` | agente | `fail-<yyyymmdd>-<contador>`. |
| `manifest_sha256` | línea `end` del manifiesto | emisor | SHA-256 de las líneas `artifact` concatenadas. |
| `request_id` | cabecera `X-Request-Id` | servidor (o cliente) | Traza extremo a extremo. |
| `idempotency_key` | `board.sqlite.idempotency` | cliente | Clave de idempotencia por POST. |
| `api_key_id` | `instance.json` del agente | primary | Prefijo del hash de la key. **Nunca** la key. |
| `class` | `tasks.payload.class` | quien crea la tarea | Clase de trabajo (§9.1): `inference`, `build`, `compute`, `network`, `storage`, `device`, `review`, `coord`. |
| `cert_hash` | `dist.sqlite.certs` | nodo | sha256 del cuerpo del `node cert` firmado. |
| `rules_hash` | `node cert` | nodo | sha256 de `share_rules.json`; detecta drift de política entre nodos. |
| `offer_id` | `dist.sqlite.offers` | nodo | Oferta de capacidad libre (§9.6). Caduca. |
| `contribution_id` | `dist.sqlite.contributions` | nodo | Aportación al enjambre, con `evidence` obligatoria para contar. |
| `credit` | derivado del ledger | primary | Unidades aportadas por un nodo; desempate con prioridad en §5.2. |
| `<área>-<n>` | `docs/AUDIT_ADVERSARIAL_V15.md` | agente de crítica | Id de hallazgo/objeción (§10.3). No edita el contrato: propone. |
| `route_id` (capa 2) | `GET /api/v2/routes` (`webui::routes_doc()`) | dueño de `webui.rs` | Tabla de rutas legible por máquina, en tiempo de ejecución (EQ-4). |

---

## ANEXO B — Trazabilidad: de qué documento viene cada sección

| Sección de este doc | Documento previo que la origina o la acota |
|---|---|
| §1 Diagnóstico | `docs/AUDIT_ENJAMBRE_V14.md` (hechos medidos), `docs/MASTER_PLAN.md` §0-§1, `docs/COORDINATION.md` (cremallero) |
| §2 API v2 | `rust-core/src/cockpit_server.rs` (rutas v1), `rust-core/src/api_key.rs` (auth/rate limit), `rust-core/src/mcp_server.rs` (JSON-RPC), `rust-core/src/mcp_ext.rs` (tools) |
| §3 Colaboración | `rust-core/src/coordination.rs` (tablero, leases, merge), `rust-core/src/mesh.rs` (bus, learning), `docs/COORDINATION.md` (reglas), `docs/MASTER_PLAN.md` §5 |
| §4 Autonomía | `rust-core/src/agent_core.rs` (plan/ejecute), `rust-core/src/delegate.rs` (ajustes), `rust-core/src/alerts.rs` (alertas), `rust-core/src/task_scheduler.rs` (cola y journal) |
| §5 Distribuido | `rust-core/src/coordination.rs::migration_plan()`, `rust-core/src/mesh.rs` (BOARD_SCHEMA), `docs/MASTER_PLAN.md` §5.3-§5.4 |
| §6 Plan | `docs/MASTER_PLAN.md` §3-§4 (backlog y olas), `docs/ROADMAP_V14.md` |
| §7 Decisiones | `docs/MASTER_PLAN.md` §8 (decisiones de la reunión) |
| §8 Ficheros | `docs/CONNECT_EXTERNAL_AGENT_ANDROID_RE.md` §1/§4 (convención `incoming/<id>/`), `tools/findings_from_manifest.py` (MANIFEST.ndjson), `rust-core/src/backup_vault.rs` (vault) |
| §9 Reparto entre nodos | `rust-core/src/power_policy.rs` (señales: `allow_heavy`, `thermal_hot`, `charging`, `battery_temp_c`), `rust-core/src/capability.rs` (registro de capacidades y `verify()`), `rust-core/src/accelerator_policy.rs` (`llm_route_decision`: CPU vs GPU del S21), `rust-core/src/task_scheduler.rs` (`Resources{cpu,net,heavy}`), `rust-core/src/delegate.rs` (enrutado a cuentas gratuitas = inferencia remota) |
| §10 Unificación | `docs/API_V2.md` (capa 2: envelope `ok/error` y 20 rutas implementadas), `rust-core/src/webui.rs` (implementación, 16 tests), `web/cockpit_v2.html` + `web/webui_v2.css` (UI v2), `docs/UI_V2_SPEC.md` (spec de UI), `rust-core/tests/audit_adversarial_v15.rs` (15 tests que reproducen los P0/P1) |

---

*Contrato de concurrencia v15. Fichero nuevo: `docs/CONCURRENCY_PLAN_V15.md`.
No modifica ningún otro documento. Las cifras marcadas `pendiente` son
mediciones pendientes (P-1..P-5) y no deben citarse como hechos.*
