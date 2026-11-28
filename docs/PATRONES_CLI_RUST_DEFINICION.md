# Definición de los 10 patrones de la CLI de agente en Rust

> Este documento **DEFINE** los diez patrones medidos en el CLI de Rust de
> Devin Desktop (`docs/RE_DEVIN_CLI_RUST.md`) y dice, para cada uno: su
> **contrato**, su **algoritmo**, su **criterio de aceptación** y **dónde vive
> en MegaDaemon** y qué falta.
>
> El índice con el coste y el módulo destino está en
> `docs/DISENO_PATRONES_CLI_RUST.md` §1.  Aquí está el detalle.
>
> MEDIDO 2026-11-28.  Lo que no está medido va marcado **NO MEDIDO**.

---

## Patrón 1 — El bucle como **manejador de efectos**

**Origen (medido)**: `affogato/agent/effects/{handler,inference,tasks,tools}.rs`
(`docs/RE_DEVIN_CLI_RUST.md` §2.1).  El bucle **no** llama al modelo ni a las
herramientas: define **efectos** y cada familia tiene su manejador.

| efecto | manejador | qué hace |
|---|---|---|
| inferencia | `effects/inference.rs` | pide el siguiente turno al modelo |
| herramientas | `effects/tools.rs` | ejecuta la tool call y devuelve el resultado |
| tareas | `effects/tasks.rs` | lanza trabajo asíncrono |

**Contrato**
* Entrada: un **evento** (`UserInput`, `ToolResult`, `TaskDone`, `Tick`).
* Salida: `(estado', efectos[])` — el reductor NO ejecuta nada.
* Cada efecto lo despacha **un manejador**, que devuelve el evento siguiente.

**Algoritmo**
```
bucle(state, evento):
    (state, efectos) = reducir(state, evento)
    for e in efectos:
        evento' = manejar(e)          # inferencia | tools | tareas
        state = bucle(state, evento')  # recursión/cola
    return state
```

**Criterio de aceptación**
1. `agent_loop.rs` no llama a `providers::execute_chat*` ni a las tools.
2. Un test puede dar una **lista de efectos** sin ejecutar nada (bucle puro).
3. El mismo bucle sirve para el agente interactivo, el programático y el
   subagente.

**Estado**: **TODO**.  Hoy `agent_loop.rs` (50.848 B) y `agent_core.rs`
(25.337 B) hacen la inferencia y las tools **inline** (medido en el RE).
Es un refactor de riesgo: `agent_loop.rs` es el módulo más Grande del crate
y el CI acaba de recuperar la capacidad de compilar ⇒ **no se toca hasta
que `main` esté verde y con la suite en verde**.

---

## Patrón 2 — Keepalive del **caché de prompt** del proveedor

**Origen (medido)**: `affogato/cache_keepalive*.rs` (5 ficheros), §2.3 del RE.
Pieza clave: `cache_keepalive_inputs.rs` = **qué entradas forman el prefijo
cacheable**.

**Contrato**
* Entrada: las entradas del prompt de este turno + el estado del turno
  anterior (firma del prefijo + reloj).
* Salida: `Reutilizar` | `Refrescar` | `Invalidado` + el motivo + el ahorro
  estimado en caracteres.

**Algoritmo** (implementado)
```
prefijo = racha_inicial(entradas)      # se corta en la 1ª volátil
firma  = hash(prefijo)
si prefijo < minimo_chars            -> Invalidado (no compensa)
si prefijo > presupuesto_chars        -> Invalidado (el caché se pierde)
si no hay estado previo               -> Invalidado (se cachea al enviar)
si firma != firma_previa              -> Invalidado (cambió un byte)
si ahora - ultimo_uso >= ttl_s        -> Refrescar (hay que calentarlo)
si no                                 -> Reutilizar (el caché sigue vivo)
```

**Criterio de aceptación**
1. Una herramienta o un contenido dinámico **cortan** el prefijo.
2. Cambiar un byte del prefijo o **reordenar** sus entradas invalida el
   caché (la firma es del prefijo *ordenado*).
3. Pasado el TTL sin cambiar el prefijo ⇒ `Refrescar`, no `Invalidado`.
4. El ahorro se mide en **caracteres** (el precio por carácter lo decide el
   proveedor; no se inventa dinero).

**Estado**: **HECHO** ⇒ `rust-core/src/cache_keepalive.rs` (12 tests),
expuesto por CLI (`cache-keepalive`) y tool MCP (`cache_keepalive`).

---

## Patrón 3 — La conversación es un **BOSQUE**

**Origen (medido)**: `message_nodes.parent_node_id`; el revert es un **fork**
del bosque.  Esquema SQLite medido (`docs/RE_DEVIN_CLI_RUST.md` §4.1):
`sessions.current_epoch`, `messages.compaction_epoch`, `UNIQUE(session_id,
compaction_epoch, sequence_number)`.

**Contrato**
* Nodo: `{id, padre, secuencia, epoch}`.
* `fork(nodo)` ⇒ hijo con `padre = nodo` y **historial copiado**.
* `revert(nodo)` ⇒ se vuelve a un nodo **sin borrar** nada.

**Algoritmo**
```
fork(nodo, cwd):
    hijo = {id: nuevo, padre: nodo.id, mensajes: copia(nodo.mensajes)}
    insertar(hijo)
    return hijo
```

**Criterio de aceptación**
1. Tras un fork, el padre **no** cambia (ni un mensaje).
2. El revert **no borra**: el historial completo sigue consultable.
3. El árbol es un **bosque** (varias raíces), no una lista.

**Estado**: **PARCIAL**.
* El mecanismo de bosque y fork **existe**: `session/fork` de ACP
  (`rust-core/src/protocol_acp/sesion.rs`, 10 tests) y la compactación por
  época (`rust-core/src/compaction_epoch.rs`, 11 tests).
* **FALTA**: las **tablas SQLite** `message_nodes` / `parent_node_id` en
  `board.rs`/`service_bus.rs`, y el **revert por pasos** (el revert de
  Devin lista los pasos y ejecuta el elegido; aquí sólo hay fork).

---

## Patrón 4 — La compactación **no borra**: marca una **época**

**Origen (medido)**: `messages.compaction_epoch` + `sessions.current_epoch`.

**Contrato**
* `compactar(resumen, presupuesto)` ⇒ sella la época actual y abre una nueva.
* **Ningún mensaje se borra**; los anteriores siguen consultables con su
  `epoch`.
* `seq` es global (no se reinicia).
* `revertir_a(epoch)` ⇒ se vuelve a una época anterior sin tocar el historial.

**Algoritmo** (implementado)
```
compactar(resumen, presupuesto):
    si vista_viva <= presupuesto: no hacer nada   # compactar costaría más
    epoch_sellada = current_epoch
    current_epoch += 1
    insertar(resumen, epoch=current_epoch)        # el resumen abre la época
    # los mensajes anteriores SE QUEDAN donde estaban
```

**Criterio de aceptación**
1. Tras compactar, el número de mensajes **no baja** (sube en 1: el resumen).
2. La vista viva es **sólo** la época actual.
3. El historial completo sigue consultable y la epoch anterior devuelve sus
   mensajes.
4. Bajo presupuesto **no** se compacta (crear una época costaría más de lo
   que ahorra).
5. El ahorro se mide en caracteres y **es 0** si el resumen es peor que el
   historial (no se inventa un número).

**Estado**: **HECHO** ⇒ `rust-core/src/compaction_epoch.rs` (11 tests), CLI
(`compaction-epoch`) y tool MCP (`compaction_epoch`).  El DDL medido de la
referencia está **documentado** (`DDL_MEDIDO`), no aplicado.

---

## Patrón 5 — `inference/`: **17 errores de proveedor** con `retry_after`

**Origen (medido)**: `inference/` (5 ficheros), §3 del RE.  Errores del
proveedor **tipados** (17), `retry_after` **estructurado**,
`compaction_thresholds` **por modelo** y `tool_formatter_type` por modelo.

**Contrato**
* Un error de proveedor es un **tipo**, no un `String`:
  `Http{status, detalle}`, `Timeout`, `Cuota{reset_at}`, `Transporte`,
  `Parse`.
* `retry_after` es un **dato** (`Option<u64>`), no texto dentro del mensaje.
* Los umbrales de compactación y el formateador de tools dependen del
  **modelo**, no son globales.

**Algoritmo**
```
al fallar un proveedor:
    e = clasificar(error)                  # -> ProviderError
    if e.retryable() and intentos < N:
        esperar(e.retry_after o backoff)
        reintentar
    else:
        siguiente_proveedor(de la cascada)
```

**Criterio de aceptación**
1. `ProviderError` **ya es** un enum con **7** variantes (`Transport`,
   `Timeout`, `Http{status}`, `Parse`, `MissingKey`, `Retired`, `Budget`),
   con `provider`, `detail` y `retryable()` (medido en
   `rust-core/src/providers.rs`).
2. `quota_guard` **usa** el `reset_at` que guarda (el defecto que causes el
   error 2 de Antigravity: un 429 pausaba al proveedor **para siempre**).
3. `retry_after` del proveedor se **parsea** y se usa, no se ignora.
4. Los umbrales por modelo se leen del catálogo, no están hardcodeados.

**Estado**: **HECHO** ⇒ los puntos (1), (2), (3), (b) y (c) están hechos.
* La tipificación de errores **existe** (`ProviderError` con 7 variantes) y
  el `reset_at` de cuota **existe** (`quota_guard::evaluar_pausa`).
* **HECHO (2026-11-28)**: el `retry_after` **viaja en el tipo de error**.
  * `ProviderError::Http` lleva `retry_after: Option<u64>` y lo lee
    `ProviderError::retry_after()` (`rust-core/src/providers.rs`).
  * `LlmResponse` lleva `retry_after: Option<u64>` y el transporte **lee la
    cabecera** `Retry-After` / `retry-after-ms` (antes se tiraban las
    cabeceras: `ureq` con `http_status_as_error(false)` las deja
    accesibles).
  * El parseo es **fuente única** en `rust-core/src/provider_retry.rs`
    (14 tests): cabecera, texto del cuerpo y espera real.
    `quota_guard` lo usa en vez de su copia.
  * El bucle de reintentos **espera lo que dice el proveedor**
    (`espera_reintento`), con tope de 120 s; antes usaba un backoff fijo de
    250 ms y reintentaba **antes de tiempo** (⇒ otro 429).
  * La traza dice cuánto pidió esperar: `p1 => HTTP 429 (...) (retry_after 42s)`.
* **HECHO (2026-11-28)**: (b) `compaction_thresholds` por modelo y (c)
  `tool_formatter_type` por modelo.
  * **nuevo `rust-core/src/model_catalog.rs`** (7 tests): el perfil de un
    modelo son **datos**, no constantes en el llamador:
    `tool_formatter(modelo, endpoint)` (la API que habla manda; los
    motores offline no hablan tool-calling ⇒ sus tools van como TEXTO) y
    `umbral_compactacion(modelo)` (por familia de modelo; el defecto es
    el presupuesto que ya cabía en el proveedor más estricto).
  * `providers.rs` usa el formateador del catálogo para decidir la forma
    de las tools, en vez de dejarlo hardcodeado por endpoint.
  * `mcp_ext.rs` (tool `compaction_epoch`) usa el umbral **del modelo**
    cuando el llamador no pasa presupuesto, y lo dice en la respuesta
    (`presupuesto_chars`, `umbral_del_modelo`).
* **NOTA**: la taxonomía tiene **7** variantes, no las 17 del original:
  el resto se distinguen por `detail`/`retry_after` dentro de la variante
  (`Http{status, detail, retry_after}`).

---

## Patrón 6 — Separar **construir contexto** de **conducir el agente**

**Origen (medido)**: `fx/context.rs` + `fx/driver.rs`, §2.5 del RE.
`context.rs` arma el contexto (reglas, skills, memoria, ficheros);
`driver.rs` conduce la ejecución.

**Contrato**
* `construir_contexto(estado, entrada) -> Contexto` — **puro**: no llama al
  modelo ni a las tools; devuelve los bloques del prompt y los metadatos.
* `conducir(contexto, estado) -> Evento` — el que habla con el modelo y
  ejecuta tools.

**Criterio de aceptación**
1. Un test puede construir el contexto **sin red** y comparar el prompt
   exacto.
2. `construir_contexto` no referencia `providers::` ni ejecuta herramientas
   (se comprueba con `grep`).

**Estado**: **PARCIAL** ⇒ la idea ya está repartida en varios módulos
(`agent_core.rs` = despacho, `board.rs` = estado, `llm_gateway.rs` =
inferencia), pero **no** hay un módulo `contexto` con la construcción
pura del prompt.  **FALTA** ese módulo, y sacarlo de `agent_loop.rs`.

---

## Patrón 7 — El daemon: socket Unix + lock + **handshake** + cola

**Origen (medido)**: `chisel-server` (9 ficheros), §5 del RE.  Socket Unix +
lock + **handshake que verifica que quien conecta es su daemon** + RPC de
cola (`list claimed sessions`, `list queued sessions`, `list outposts`).

**Contrato**
* Escucha en un socket Unix con lock de instancia (un solo daemon por
  estado).
* **Handshake**: el que conecta tiene que presentar una credencial del
  estado; si no, se rechaza (fail-closed).
* RPC de cola: `list claimed sessions`, `list queued sessions`,
  `list outposts`.

**Criterio de aceptación**
1. Dos daemons sobre el mismo `state_dir` ⇒ el segundo **no** arranca (lock).
2. Un cliente sin credencial **no** puede usar la API.
3. Las tres consultas de cola devuelven la cola real (`task_scheduler.rs`).

**Estado**: **PARCIAL**.
* La **cola** existe (`task_scheduler.rs`, MEDIDO): estados `Pending`,
  `Running`, `Paused`, `Done`, `Failed`, `Cancelled`; y por tarea
  `node` (a quién se asignó), `lease_expira` (lease), `resume_after` (pausa),
  `attempts`/`max_attempts`.
* El **cockpit HTTP** tiene autenticación fail-closed
  (`X-Seere-Auth-Token`, token 0600 en `<state_dir>/cockpit_token`).
* **FALTA**: el **socket Unix con handshake** (hoy el transporte es HTTP en
  el cockpit) y las tres consultas de cola con esos nombres.

---

## Patrón 8 — Sandbox: el permiso lógico **compilado a aislamiento**

**Origen (medido)**: §6 del RE (texto literal del producto).  En Linux exige
`bwrap` **y** `socat`; el permiso lógico se compila a una regla de sandbox.

**Criterio de aceptación**
1. Sin `bwrap` (ni `sandbox-exec`) ⇒ **fallar cerrado** (no se ejecuta nada).
2. La política deny > allow se compila a montajes y flags concretos.
3. `--unshare-net`, `--cap-drop ALL` y `--seccomp` presentes.

**Estado**: **HECHO** ⇒ `rust-core/src/sandbox_linux.rs` (bwrap + seatbelt,
10 tests), y `tool_policy.rs` decide (allow/ask/deny) mientras
`sandbox_linux.rs` compila esa decisión a la regla.

---

## Patrón 9 — `PLUGIN_DATA` por **identidad**, globs sin `..`/`#`/`@`

**Origen (medido)**: §7 del RE.

**Contrato**
* Los datos de un plugin viven en `<estado>/plugins/<identidad>`: la ruta
  **no lleva la versión** ⇒ sobreviven a las actualizaciones y se borran al
  desinstalarlo.
* Identidad: ASCII en minúsculas, dígitos, `.`, `_`, `-`; sin separadores,
  sin `..`, sin empezar por `.`.
* Glob: sin `..`, sin `#`, sin `@`, nunca absoluto.

**Criterio de aceptación**
1. La ruta **no** contiene la versión.
2. Una identidad con `..` o separador se rechaza.
3. Un glob con `..`, `#` o `@` se rechaza.
4. Purgar un plugin **no** toca los datos de otro.

**Estado**: **HECHO** ⇒ `rust-core/src/plugin_data.rs` (10 tests), CLI
(`plugin-data`) y tool MCP (`plugin_data`).

---

## Patrón 10 — El esquema SQLite de sesiones (10 tablas), medido literal

**Origen (medido)**: §4.1 del RE (el DDL literal del producto).

**Contrato**: las 10 tablas con sus columnas, índices y `UNIQUE`, medidas.

**Criterio de aceptación**
1. Una migración crea el esquema medido **en un estado nuevo** y en uno
   existente (idempotente).
2. `board.rs` (rusqlite) usa esas tablas.

**Estado**: **PARCIAL**.
* `board.rs` tiene su esquema (rusqlite) y `task_scheduler.rs` su cola.
* **FALTA**: la migración al esquema **medido** (`sessions` con
  `current_epoch`, `messages` con `compaction_epoch` y su `UNIQUE`) y las
  10 tablas exactas.

---

## Resumen

| # | patrón | estado | módulo |
|---|---|---|---|
| 1 | bucle como manejador de efectos | TODO | `agent_loop.rs` / `agent_core.rs` |
| 2 | keepalive del caché de prompt | **HECHO** | `cache_keepalive.rs` |
| 3 | la conversación es un bosque | PARCIAL | `protocol_acp/sesion.rs` (falta SQLite + revert por pasos) |
| 4 | compactación por época | **HECHO** | `compaction_epoch.rs` |
| 5 | errores de proveedor tipados | PARCIAL | `providers.rs` + `provider_retry.rs` (hecho el `retry_after`; falta lo por modelo) |
| 6 | construir contexto ≠ conducir el agente | PARCIAL | falta el módulo `contexto` |
| 7 | daemon: socket + handshake + cola | PARCIAL | falta el socket Unix con handshake |
| 8 | sandbox compilado del permiso | **HECHO** | `sandbox_linux.rs` |
| 9 | `PLUGIN_DATA` por identidad | **HECHO** | `plugin_data.rs` |
| 10 | esquema SQLite de sesiones | PARCIAL | falta la migración al esquema medido |

**3 de 10 completos** (los tres de coste **S**: 2, 4 y 9; el 8 ya estaba).
Los otros 7 tienen la parte estructural hecha y el detalle medido pendiente;
los patrones **1** y **10** son los que más tocan el núcleo y deben ir
**después** de que `main` esté verde y con la suite en verde.

**Lo que NO está medido**: el comportamiento de estos patrones en el móvil
requiere el dispositivo (`su -mm -c`); el repo sólo puede verificar tipos,
tests y contratos.
