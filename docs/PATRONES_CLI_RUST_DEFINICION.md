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

**Estado**: **HECHO** (2026-11-28) ⇒ `rust-core/src/efectos.rs` (14 tests)
y el **cableado** en `rust-core/src/agent_loop.rs`.

* El núcleo puro: `Evento` (`UserInput`, `Tick`, `InferenciaTerminada`,
  `ToolResult`, `TaskDone`, `RutaResuelta`, `CriterioFijo`, `TareaCerrada`,
  `Inerte`), `Efecto` (`Inferencia`, `Herramientas`, `Tareas`) y
  `reducir(estado, evento) -> (estado, efectos)`: el reductor **NO ejecuta
  nada** (criterio 2).
* Cada efecto lo despacha **un manejador** (`manejar_inferencia`,
  `manejar_herramientas`, `manejar_tareas`), que devuelve el evento
  siguiente; el bucle es **iterativo** (pila explícita, con tope duro de
  eventos), no recursivo.
* `Modo{Interactivo, Programatico, Subagente}` + `Perfil`: el mismo bucle
  sirve para los tres (criterio 3), con test de los tres modos.
* **Cableado**: `agent_loop.rs` ya **no** llama al modelo ni a las tools
  (criterio 1): `AgentLoop::tick` construye el estado, llama a
  `efectos::bucle` y vuelca el estado.  `pick_provider`, `run_action` y
  `fixed_fallback` se fueron del bucle; sus equivalentes puros están en
  `efectos.rs`.  La llamada al modelo vive sólo en `manejar_inferencia` y la
  de las tools sólo en `manejar_herramientas`.
* Los **8 tests preexistentes** de `agent_loop.rs` siguen pasando sin
  tocarlos (la memoria de fallos, la degradación, la cadena de proveedores,
  el tope de llamadas y el prompt sin secretos).
* Pendiente: `TaskDone` no lo emite nadie todavía (no hay trabajo
  asíncrono real); la API y el manejador están listos para engancharse al
  `task_scheduler`/`service_bus`.

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

**Estado**: **HECHO** (2026-11-28) ⇒ `rust-core/src/message_tree.rs`
(18 tests), sobre el esquema del patrón 10.

* El **bosque**: nodos con `parent_node_id`, varias raíces.
* `fork(nodo)`: hijo con el historial **copiado**; el padre no cambia
  (criterio 1).
* `revert(nodo)`: **no borra**; el historial completo sigue consultable
  (criterio 2).
* **Revert por pasos** (lo que faltaba): `pasos_revert` lista los puntos
  de retorno y `aplicar_paso` ejecuta el **elegido** (la referencia expone
  `cognition.ai/revert/{listSteps,preview,execute,forkFromStep}`).
* Persistencia en SQLite con las tablas del patrón 10.
* El fork abre una época nueva: no es inventado, lo fuerza el `UNIQUE`
  medido de `messages` (la copia tiene los mismos `seq` que el padre).

### CABLEADO (2026-11-28): el bosque se persiste en `message_nodes` y se restaura

La persistencia del bosque **existía** en `message_tree.rs` pero **nadie la
llamaba** en runtime: el bosque vivía en memoria del proceso y cada llamada
lo reconstruía desde los mensajes que le pasaban
(`rust-core/src/capacidades.rs::revert`) ⇒ al reiniciar el daemon se perdían
las ramas.

* `rust-core/src/message_tree.rs`: `guardar_bosque()` escribe **sólo** la
  topología (la tabla medida `message_nodes` y la cabeza viva en `app_state`)
  ⇒ **no** le inventa columnas al esquema medido y **no** pisa `sessions` ni
  `messages` (esos son del store de sesiones).
* `rust-core/src/acp_sesion_store.rs`: `guardar_bosque()` / `cargar_bosque()`
  / `listar_bosques()` sobre el almacén; el borrado explícito de una sesión
  se lleva su bosque.
* `rust-core/src/protocol_acp/sesion.rs`: el registro ACP guarda **un bosque
  por sesión** (un nodo por mensaje, padre = la cabeza viva), lo **persiste**
  al mutar y lo **restaura** al arrancar (`Registro::restaurar`); si una
  sesión se guardó sin bosque, se **reconstruye** de su historial persistido
  (no se pierde nada).  El `session/fork` copia el bosque (el padre no
  cambia) y el **revert por pasos** lo bifurca (nada se borra).
* MEDIDO: si el almacén no abre o no escribe, el ACP **sigue en memoria** y
  avisa por stderr (la persistencia es un efecto secundario, nunca un
  bloqueo).

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
5. El dialecto que dice el catálogo es **el que se habla**: un proveedor
   cuya API tiene dialecto propio recibe **ese** cuerpo (tools con
   `input_schema`, `system` arriba, `tool_choice` como objeto) y su
   respuesta se lee con **ese** parser.
6. El bucle de reintentos **espera lo que dice el proveedor**, y eso es
   **comprobable** (la espera es inyectable: un test ve qué se esperó).

**Estado**: **HECHO** ⇒ los puntos (1), (2), (3), (4), (5) y (6) están hechos.
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
* **HECHO (2026-11-28, cierre)**: los **tres** huecos que quedaban, que
  además **ningún test detectaba** (un cambio que los deshaciera habría
  dejado la suite en verde):
  * **El bucle ESPERA lo que dice el proveedor, y se puede probar** (criterio
    6).  Antes la espera era un `thread::sleep` clavado (`dormir_espera`) y
    los tests usaban `backoff_ms = 0` ⇒ `espera_reintento` devolvía CERO y el
    `retry_after` quedaba **enmascarado**: volver al backoff fijo no lo
    detectaba ningún test.  Ahora `ChatDeps` trae `dormir: fn(Duration)`
    (por defecto, la espera real) ⇒ el test **ve** qué se esperó
    (`rust-core/tests/llm_routing.rs`: `el_bucle_espera_lo_que_dice_el_proveedor`,
    `un_401_no_espera_nada`).
  * **El `retry_after` ESTRUCTURADO llega a la pausa por cuota** (criterio 2
    y 3).  Antes la cascada pasaba al `quota_guard` sólo el **texto** del
    error ⇒ el `Retry-After` de la **cabecera** se perdía y la pausa se caía
    a un cooldown a ciegas en vez de esperar lo que pidió el proveedor.  Ahora
    `quota_guard::registrar_error_cuota_con_retry_after` lo recibe
    (`la_cascada_pasa_el_retry_after_de_la_cabecera_a_la_pausa`).
  * **El dialecto de Anthropic se HABLA de verdad** (criterio 5).  El
    catálogo declaraba `ToolFormatter::AnthropicTools` para
    `api.anthropic.com` y Anthropic es proveedor de primera clase
    (`provider_catalog.rs`, endpoint `https://api.anthropic.com/v1/messages`),
    pero el cuerpo que se le mandaba era el de **OpenAI** (`system` como
    mensaje, `tools: [{type:"function",…}]`, `tool_choice: "auto"`) ⇒ la API
    lo rechaza con HTTP 400: el formateador decía una cosa y el código otra.
    Ahora `providers.rs` traduce el cuerpo (`cuerpo_anthropic`) y lee la
    respuesta (`parse_anthropic_reply`, con `content` en bloques: `text`,
    `tool_use`, `thinking`).
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

**Estado**: **HECHO** (2026-11-28) ⇒
`rust-core/src/contexto.rs` (15 tests) **+ cableado en el bucle**.

* `construir_contexto(estado, entrada) -> Contexto` es **puro**: no habla
  con el modelo, no ejecuta herramientas y no hace E/S.
* Devuelve los **bloques** del prompt (sistema, reglas, memoria, ficheros,
  época, historial, entrada) y los **metadatos** (chars por bloque y total,
  presupuesto, qué se recortó).
* El recorte se **registra**: qué bloque, cuántas unidades y cuántos
  caracteres ahorrados (medidos).  `presupuesto_chars = None` significa
  "sin límite", no un número inventado.
* El criterio 2 está comprobado **por test**: el código (sin comentarios) no
  menciona `providers::` ni ninguna tool.
* **CABLEADO (2026-11-28)**: `efectos::construir_prompt` (y por tanto
  `AgentLoop::build_prompt` y el prompt que el bucle manda al modelo) ya
  **no** se escribe a mano: se le pide el contexto a
  `contexto::construir_contexto` y se ensambla con `Contexto::to_prompt()`.
  El módulo `contexto` es la **única** fuente de los bloques del prompt.

  El prompt del meta-agente pasa de un texto plano a los bloques canónicos
  (MEDIDO, el contenido se conserva **todo**; lo que cambia son las
  etiquetas y los saltos de línea):

  ```
  [SISTEMA]
  Eres el meta-agente de MegaDaemon. Responde SOLO con una accion.

  [REGLAS]
  Formatos admitidos:
    {"action":"tool","name":"<tool>","args":{...}}
    {"action":"delegate","provider":"<id>","task_type":"<tipo>"}
    {"action":"speak","text":"..."}
  Provider local (SEERE) es el ultimo recurso: solo si los demas fallan.
  Proveedores disponibles: seere (local, ultimo recurso)

  [MEMORIA]
  Lo que YA fallo (no lo repitas):
    - <lo que ya falló>

  [ENTRADA]
  Tarea: <la tarea>
  ```

  * El bloque `[EPOCA]` **desaparece** si no hay historial: no hay bloques de
    adorno (el bucle de efectos no compacta).  Es un cambio de contrato
    del módulo `contexto` con su test (`un_estado_vacio_solo_tiene_la_entrada`).
  * El bloque `[MEMORIA]` sólo aparece **si hay fallos** (igual que antes).
  * El prompt es **determinista**: mismo estado ⇒ mismo prompt, byte a byte
    (`el_prompt_es_determinista_byte_a_byte`).
  * Validado en crate AISLADO (`agent_loop` + `efectos` + `contexto` +
    `compaction_epoch`): **55 tests verdes** (6 nuevos del cableado).

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

**Estado**: **HECHO** (2026-11-28) ⇒ `rust-core/src/daemon_socket.rs`
(20 tests).

* **Lock de instancia**: `<state_dir>/daemon.lock` con `create_new(true)`;
  dos daemons sobre el mismo `state_dir` ⇒ el segundo **no arranca**
  (criterio 1).  Se distingue lock vivo de lock caducado con
  `libc::kill(pid, 0)`; el fichero se borra al soltar el guard.
* **Socket Unix** en `<state_dir>/daemon.sock` (0600), protocolo NDJSON
  (un objeto JSON por línea).
* **Handshake fail-closed**: el que conecta presenta la credencial del
  estado (comparación en tiempo constante; credencial vacía nunca vale).  Un
  cliente sin credencial **no puede usar la API** (criterio 2).
* Las **tres consultas de cola** con esos nombres exactos — `list claimed
  sessions`, `list queued sessions`, `list outposts` — sobre la cola REAL de
  `task_scheduler.rs` (criterio 3).
* La E/S (bind/aceptar) está separada de la **decisión** (parseo, validación
  de la credencial, construcción de la respuesta), que es pura y testeable.
* **NO MEDIDO**: qué es un "outpost" (la referencia no lo define): se
  implementa una interpretación conservadora y la definición viaja en cada
  respuesta, para no fingir una medición que no hay.

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

**Estado**: **HECHO** (2026-11-28) ⇒ `rust-core/src/board_schema.rs`
(13 tests) **+ cableado en `board.rs` y en el store de sesiones**.

* El DDL **medido** de las 10 tablas, transcrito de
  `docs/RE_DEVIN_CLI_RUST.md` §4.1, con sus columnas, índices y `UNIQUE`
  (incluido `UNIQUE(session_id, compaction_epoch, sequence_number)`).
* Migración **idempotente**: sirve en un estado nuevo y en uno existente
  (aplicarla dos veces no falla ni duplica nada).
* Helpers de **introspección** (tablas, columnas, índices) y
  `verificar_esquema`, que comparan lo creado con lo medido tabla por
  tabla: es lo que permite un test que fije el esquema sin depender de la
  referencia en runtime.
* `sessions.hidden` NO está en el DDL medido pero su índice medido lo exige
  (SQLite: "no such column: hidden") ⇒ se declara como **extensión**
  propia, marcada como tal y con test que lo prueba.

### CABLEADO (2026-11-28): el esquema medido es el esquema REAL

Antes el esquema medido era un módulo **sin usar**: `board.rs` tenía su
propio esquema de tareas y el store de sesiones ACP (`acp_sesion_store.rs`)
el suyo (`acp_sessions`, `acp_messages`, `acp_meta`).  Ahora:

* `board.rs::migrate()` aplica **además** el esquema medido
  (`board_schema::aplicar`), así que la base de la instancia tiene las 10
  tablas medidas (test: `el_tablon_aplica_el_esquema_medido`).
* El store de sesiones ACP **usa las tablas medidas**: `acp_sessions` →
  `sessions`, `acp_messages` → `messages`, `acp_meta` → `app_state`.  Las
  columnas que el DDL medido no tiene se guardan donde toca:
  * `estado`, `padre` (el fork) y `turnos` ⇒ `app_state` con la clave
    `acp:sesion:<id>` (la tabla medida de clave/valor).  El esquema medido
    queda **intacto**: no se le inventan columnas.
  * `es_resumen` ⇒ se **deduce** del `role` (`"resumen"`).
  * el `Value` ACP exacto ⇒ `messages.tool_calls_json` (la columna medida
    donde va el JSON del mensaje: sin esto el round-trip pierde el
    `session/update`).
  * La **tumba** (`acp_sessions_borradas`) y las extensiones de sesión
    (`acp_session_extensions`) son **extensiones propias**: el esquema
    medido no las tiene.
* La instancia tiene **una** base SQLite: el tablón y las sesiones
  comparten `board.sqlite` (el nombre sale de `board_schema::DB_FILE`, una
  sola fuente de verdad).  Test: `el_tablon_y_las_sesiones_comparten_la_misma_base`.
* **MEDIDO**: los dos tests están en `board.rs` y el store tiene 18 tests.

Validado en crates AISLADOS: **56 tests** (tablón + esquema medido + store)
y **53 tests** (store + ACP) verdes.

---

## Resumen

| # | patrón | estado | módulo |
|---|---|---|---|
| 1 | bucle como manejador de efectos | **HECHO** | `efectos.rs` (+ cableado en `agent_loop.rs`) |
| 2 | keepalive del caché de prompt | **HECHO** | `cache_keepalive.rs` |
| 3 | la conversación es un bosque | **HECHO** | `message_tree.rs` |
| 4 | compactación por época | **HECHO** | `compaction_epoch.rs` |
| 5 | errores de proveedor tipados | **HECHO** | `providers.rs` + `provider_retry.rs` + `model_catalog.rs` + `quota_guard.rs` (errores tipados, `retry_after` estructurado **y usado**, umbral y formateador **POR MODELO**, dialecto de Anthropic hablado) |
| 6 | construir contexto ≠ conducir el agente | **HECHO** | `contexto.rs` (+ cableado en `efectos::construir_prompt` / `build_prompt`) |
| 7 | daemon: socket + handshake + cola | **HECHO** | `daemon_socket.rs` |
| 8 | sandbox compilado del permiso | **HECHO** | `sandbox_linux.rs` |
| 9 | `PLUGIN_DATA` por identidad | **HECHO** | `plugin_data.rs` |
| 10 | esquema SQLite de sesiones | **HECHO** | `board_schema.rs` (+ cableado en `board.rs` y en el store de sesiones) |

**10 de 10 completos** (2, 4, 9 y 8 ya estaban; este 2026-11-28 cierran
1, 3, 6, 7 y 10).  Los dos cabos de cableado que quedaban están
cerrados: `build_prompt` usa `contexto::construir_contexto` y el esquema
medido llega al `board.rs` (es el esquema real de la base de la
instancia).

## Capacidades propias nuevas (AST → Rust, 2026-11-28)

Siguen el procedimiento de `docs/EXTENSIONES_PROPIETARIAS.md` §4: la
capacidad de la referencia se **documenta** (con ruta y línea), la
implementación es **nuestra** (mismos nombres, misma estructura, **nuestro
texto**) y los tests fijan el **comportamiento observable**, no la
implementación.  El gate `tools/check-propietario.py` está en verde.

| capacidad propia | módulo | tests | qué cubre (medido) |
|---|---|---|---|
| cola de mensajes de la sesión | `cola_mensajes.rs` | 16 | `cognition.ai/queuedMessagesInfoUpdate` ~ `mcode/session/queue/*`: encolar, listar, actualizar, borrar y **redirigir** ("steer") |
| subagentes del lead | `subagentes.rs` | 16 | `cognition.ai/subagentSupport` + `subagentControl` ~ `mcode/session/delegation`: foreground, background, cancel, **opt-in mutuo** |
| control de servidores y tools MCP | `mcp_control.rs` | 16 | `cognition.ai/mcp/{listServers,toggleServer,toggleTool}`: opt-in mutuo, apagado idempotente |
| opciones de modelo | `model_options.rs` | 12 | `cognition.ai/clientProvidedModels` + `groupedSessionConfigOptions`: catálogo del daemon + el del cliente |
| persistencia de sesiones ACP | `acp_sesion_store.rs` | 17 | el almacén que `docs/DECISION_ACP.md` §3.2 dejaba pendiente (épocas, `seq` global, sin borrados colaterales) |

Los **métodos de extensión** que proponen (namespace `mdaemon`, en inglés
como los de `protocol_acp.rs`):

* cola: `queue/list`, `queue/enqueue`, `queue/update`, `queue/delete`,
  `queue/steer`;
* subagentes: `subagent/list`, `subagent/get`, `subagent/foreground`,
  `subagent/background`, `subagent/cancel`;
* MCP: `mcp/servers/list`, `mcp/servers/toggle`, `mcp/tools/toggle`;
* modelos: `models/options/list`, `models/options/enrich`,
  `models/options/filter`, `models/options/default`.

### Integradas (2026-11-28): usables de verdad

* **La fachada** `capacidades.rs` + la tool MCP `capacidades` (cola,
  subagentes, modelos, contexto y revert por pasos en un solo sitio; la cola
  y los subagentes **se persisten**).
* **Las extensiones ACP** están registradas en `protocol_acp.rs`
  (`extensiones_propias`, 25 extensiones nuevas).
* **El bus**: `service_bus::seed_capacidades_propias()` mete las
  capacidades propias en el catálogo (antes el bus no sabía que el daemon
  las tenía) y `seed_servidores_mcp()` mete los servidores MCP reales.
* **El MCP real**: `mcp_control.rs` ya no es un módulo sin usar:
  * `FileRegistry` implementa el trait `McpRegistry` sobre el
    `mcp_config.json` **real** del móvil (el que usa Antigravity):
    `estado_desde_mcp_config()` lo lee y `aplicar_estado_a_mcp_config()`
    lo escribe **sin perder** las cabeceras, el `env` ni lo demás (sólo se
    toca el estado del servidor).
  * La tool `mcp_control` está en el **catálogo MCP** (`mcp_ext::
    tool_definitions()` la concatena ⇒ sale en `tools/list`) y su dispatch
    llama al registro real (`mcp_ext::dispatch` ⇒ `mcp_control::
    dispatch_mcp`).
  * MEDIDO: el opt-in mutuo, en la vía ACP, sigue siendo estricto; en la
    superficie MCP **la llamada a la tool ES la declaración** (el cliente la
    pide explícitamente).
* El `/api/service_bus` del cockpit cataloga los servidores MCP reales (y
  si el `mcp_config.json` no se puede leer, lo dice en vez de inventar).

**Lo que NO está medido**: el comportamiento de estos patrones en el móvil
requiere el dispositivo (`su -mm -c`); el repo sólo puede verificar tipos,
tests y contratos.
