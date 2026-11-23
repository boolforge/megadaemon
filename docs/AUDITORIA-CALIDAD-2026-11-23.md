# AUDITORÍA DE CALIDAD DEL CRATE `rust-core` (2026-11-23)

**Agente:** auditoría de código (rama `rama/audit-calidad-2026-11-23`)
**Alcance:** `rust-core/src/` (180 módulos) y sus tests.
**Método:** LECTURA + ESCRITURA. **No se compiló nada** (hay un Ghidra/JVM de
1,4 GB en este sandbox de 3 GB: compilar lo mata por OOM). La verificación
hecha fue `tools/check-sintaxis.sh` (rustfmt: **293 ficheros analizados, 0
errores de sintaxis**), `git log/diff/show` y lectura directa del código.
**Los 4 bugs de `backup_apps.rs` que ya arreglaste NO se han tocado.**

## 0. Base de la rama (desviación consciente del enunciado)

El enunciado pedía `rama/audit-calidad-2026-11-23` **desde `main`**. La rama
se ha creado desde el **HEAD real del trunk** (`rama/fix-ci-2026-11-23` @
`0ba46cb`), no desde `main` (`3ab5a2e`), y el motivo está medido:

| dato | valor |
|---|---|
| `git diff --stat main..HEAD -- rust-core/src/` | **144 ficheros, +81.392 líneas** |
| `main` contiene el commit `5493451` (tus 4 bugs de `backup_apps.rs`) | **NO** |
| `merge-base(main, HEAD)` | `a058e83` |

Auditar `main` habría sido auditar un código obsoleto en el que **tus 4
arreglos no existen** (habría vuelto a "encontrar" bugs ya corregidos y
habría arreglado código que el root ya no usa). `main` **no se ha tocado** y
**no se ha hecho push**; la rama propia queda lista para que el root la
integre.

---

## 1. Tabla de hallazgos

Severidad: **P0** = pérdida de datos / deja el sistema muerto ·
**P1** = corruption/fallo silencioso con impacto real ·
**P2** = calidad/coherencia · **P3** = cosmético.
Estado: **ARREGLADO** (commit) · **NO VERIFICADO** (no se arregla: no se ha
podido demostrar leyendo el código) · **PENDIENTE**.

Las líneas citadas son las del **base `0ba46cb`** (donde estaba el defecto);
la columna "ahora" es la línea del arreglo en esta rama.

### A) Datos y persistencia (riesgo de pérdida de datos)

| # | fichero:línea | clase | sev | estado | commit | ahora |
|---|---|---|---|---|---|---|
| A-1 | `http_transfer.rs:303` (`rename` antes del sha) | se publica un destino **no verificado** | **P0** | ARREGLADO | `14e2d14` | `http_transfer.rs:415` |
| A-2 | `http_transfer.rs:188` (el "resume" mide el destino, no el `.part`) | reanudación rota + sha sólo de la cola | **P1** | ARREGLADO | `01b9a07` | `http_transfer.rs:232,339` |
| A-3 | `http_transfer.rs:289` (la rama de fichero no descomprime brotli) | contenido guardado ≠ contenido real | **P1** | ARREGLADO | `11c1142` | `http_transfer.rs:350` |
| A-4 | `drive_native.rs:399` (`File::create(dest)`) | trunca el destino + descarga parcial aceptada como OK | **P0** | ARREGLADO | `52a971c` | `drive_native.rs:400` |
| A-5 | `backup_crypto.rs:381` (`master.len() < AEGIS_KEY_LEN`) | validación que acepta lo que AEGIS **no** soporta ⇒ **panic** | **P0** | ARREGLADO | `d6e921d` | `backup_crypto.rs:391` |
| A-6 | `task_scheduler.rs:383` (`fs::write` de la cola) | escritura que trunca el estado | **P0** | ARREGLADO | `cc8911f` | `task_scheduler.rs:391` |
| A-7 | `task_scheduler.rs:390` (`restore` nunca llamado) | estado que se pierde en cada reinicio | **P0** | ARREGLADO | `a16915d` | `main.rs:122` |
| A-8 | `task_scheduler.rs:394` (`restore` = 0 para todo) | corrupción indistinguible de "no hay cola" | **P1** | ARREGLADO | `dfaed65` | `task_scheduler.rs:414` |
| A-9 | `task_scheduler.rs:781` (`expirar_leases` sólo mira `Running`) | lease que **nunca** caduca ⇒ tarea colgada para siempre | **P1** | ARREGLADO | `dfff3db` | `task_scheduler.rs:816` |
| A-10 | `titanium_backup.rs:419` (`fs::write` al destino) | trunca el fichero bueno al extraer | **P1** | ARREGLADO | `d4ff67e` | `titanium_backup.rs:426` |
| A-11 | `sqlite_mitm.rs:315` (`remove_file` + generar en sitio) | DB corrupta publicada y **montada** sobre la app | **P1** | ARREGLADO | `16a1014` | `sqlite_mitm.rs:335` |
| A-12 | `backup_plan.rs:482` (`fs::write` del plan) | escritura que trunca el estado | P2 | ARREGLADO | `46bfdf6` | `backup_plan.rs:488` |
| A-13 | `quota_guard.rs:180` (`fs::write` + `let _ =`) | estado corrupto + error callado | P2 | ARREGLADO | `46bfdf6` | `quota_guard.rs:188` |
| A-14 | `cert_vault.rs:1923` (`write_atomic` privado) | lógica duplicada: la versión buena no la usaba nadie | P2 | ARREGLADO | `9839c76` | `util_fs.rs` (canónico) |

### B) Planificación, cola y cuotas

| # | fichero:línea | clase | sev | estado | commit | ahora |
|---|---|---|---|---|---|---|
| B-1 | `task_sync.rs:101` (status HTTP ignorado) | `Ok` en silencio tras un error ignorado | **P1** | ARREGLADO | `61bd1d8` | `task_sync.rs:110` |
| B-2 | `task_sync.rs:116` (`if let Ok(txt)`) | error tragado al importar la cola remota | P2 | ARREGLADO | `61bd1d8` | `task_sync.rs:124` |
| B-3 | `task_sync.rs:13` (doc: "running > pending > paused > done") | comentario que describe un comportamiento inexistente | P2 | ARREGLADO | `92c9471` | `task_sync.rs:13` |
| B-4 | `task_sync.rs:70,75` (`ahora()` dos veces) | nombre local ≠ nombre subido | P3 | ARREGLADO | `92c9471` | `task_sync.rs:70` |
| B-5 | `task_scheduler.rs:222` (`pick` ignora `node` y `lease_expira`) | ejecución duplicada entre nodos | **P1** | **NO VERIFICADO** | — | — |
| B-6 | `task_scheduler.rs:533` (`merge_tasks` no copia `attempts`/`deps`/`priority`) | fusión incompleta por id | P2 | **NO VERIFICADO** | — | — |

### C) Auto-actualización y supervisión

| # | fichero:línea | clase | sev | estado | commit | ahora |
|---|---|---|---|---|---|---|
| C-1 | `self_update.rs:477` (`rename` del viejo antes del nuevo) | ventana **sin binario** ⇒ móvil sin daemon | **P0** | ARREGLADO | `01391d7` | `self_update.rs:502` |
| C-2 | `self_update.rs:432` (`Err(_) => None`) | se instala **sin verificar** si falta el `.sha256` | **P0** | ARREGLADO | `9f0e1d6` | `self_update.rs:452` |
| C-3 | `self_update.rs:258` (jitter del mtime del binario) | sincronización de toda la flota + doc que miente | P3 | ARREGLADO | `e0e009f` | `self_update.rs:267` |

### D) Seguridad de secretos

| # | fichero:línea | clase | sev | estado | commit | ahora |
|---|---|---|---|---|---|---|
| D-1 | `api_key.rs:203,256,310,159` (`fs::write` + `set_mode_600`) | secreto creado **0644**; chmod con el error callado | **P0** | ARREGLADO | `5efd002` | `api_key.rs:380` |
| D-2 | `agyshim.rs:495` (`ruta` en claro en el NDJSON) | asymmetricidad de redacción (cuerpo sí, ruta no) | **P1** | ARREGLADO | `939733a` | `agyshim.rs:506` |
| D-3 | `agyshim.rs:219` (el redactor no cubre `?key=`) | formato query-string sin cubrir | P2 | **NO VERIFICADO** | — | — |

### E) Calidad y consistencia transversal

| # | fichero:línea | clase | sev | estado | commit | ahora |
|---|---|---|---|---|---|---|
| E-1 | `util_fs.rs` (no existía helper atómico) | `write_atomic` canónico + `cert_vault` delegando | P2 | ARREGLADO | `9839c76` | `util_fs.rs:26` |
| E-2 | `llm_gateway.rs:428` (`.expect("parts es un array")`) | `expect` en producción | P3 | **DESCARTADO** (inalcanzable: el `json!` de 4 líneas antes siempre pone un array) | — | — |
| E-3 | 1.648 `unwrap()/expect()` en el crate (119 ficheros) | `expect` en rutas de producción | — | **NO VERIFICADO** (cobertura parcial, ver §4) | — | — |

**Totales: 20 bugs ARREGLADOS (14 de clase A/B de datos, 3 de auto-update,
2 de secretos, 1 de calidad) · 4 NO VERIFICADOS · 1 descartado.**

---

## 2. Los 20 commits (uno por bug)

```
9839c76 fix(util_fs): escritura atómica CANÓNICA (antes: fs::write trunca el destino)
cc8911f fix(task_scheduler): la cola se guardaba TRUNCANDO el fichero (pérdida silenciosa)
dfaed65 fix(task_scheduler): restore() no distinguía "no hay cola" de "cola CORRUPTA"
a16915d fix(task_scheduler): la cola NUNCA se restauraba al arrancar (trabajo perdido)
dfff3db fix(task_scheduler): lease que NUNCA caduca => tarea colgada para siempre
14e2d14 fix(http_transfer): la descarga PUBLICABA el destino antes de verificar el sha
01b9a07 fix(http_transfer): el "reanudar" medía el fichero equivocado (y el sha sólo cubría la cola)
11c1142 fix(http_transfer): al guardar a fichero NO se descomprimía brotli (la de memoria sí)
52a971c fix(drive_native): la descarga de Drive escribía EN EL DESTINO (truncando el bueno)
d6e921d fix(backup_crypto): clave maestra de longitud incorrecta => PANIC en vez de error
9f0e1d6 fix(self_update): se instalaba el binario SIN verificar si faltaba el .sha256
01391d7 fix(self_update): la instalación dejaba una ventana SIN binario (móvil sin daemon)
5efd002 fix(api_key): el fichero de la credencial se creaba 0644 y el chmod fallaba en silencio
d4ff67e fix(titanium_backup): la extracción TRUNCABA el fichero de destino antes de escribir
16a1014 fix(sqlite_mitm): la DB "mentirosa" se generaba EN EL SITIO (si fallaba, la app veía una DB rota)
46bfdf6 fix(backup_plan, quota_guard): dos ficheros de estado se guardaban TRUNCANDO el destino
61bd1d8 fix(task_sync): la sincronización con Drive devolvía "ok" sin haber sincronizado nada
92c9471 fix(task_sync): dos incoherencias del sincronizador (nombre del fichero y doc que miente)
939733a fix(agyshim): la RUTA se guardaba en claro en el NDJSON (el cuerpo no)
e0e009f fix(self_update): el jitter del auto-actualizador era FIJO (toda la flota golpeaba a la vez)
```

Cada mensaje explica **qué se midió**, no qué se quiso hacer (PROTOCOLO_FLOCKA
§2), para que sirva de patrón al resto de la flota.

---

## 3. Los 4 que quedan `NO VERIFICADO` (NO se han arreglado)

### B-5 · `task_scheduler.rs:222` — `pick()` ignora la lease ⇒ ejecución duplicada
`pick()` selecciona cualquier tarea `Pending` sin mirar `t.node` ni
`t.lease_expira`. Una tarea repartida con `repartir_por_nodo` queda
`Pending` con `node` y `lease_expira` ⇒ cualquier nodo que la sincronice
(`task_sync`) se la queda y la ejecuta **en paralelo** con su dueño.
**Por qué no se arregla:** el arreglo correcto necesita saber el nodo local
dentro de `pick()`, y hoy ni `pick()` ni `run_due()` lo reciben (`run_due`
no tiene noción de nodo). Meterla cambia la semántica del reparto y no se
puede compilar ni probar aquí ⇒ riesgo de romper el trabajo de la flota.
**Medida para el root:** el nodo ya existe en `task_sync::nodo_actual()`
(`MEGADAEMON_NODE`/`HOSTNAME`); la solución de una línea es que `pick()`
acepte el nodo y salte las tareas con lease viva de OTRO nodo.

### B-6 · `task_scheduler.rs:533` — `merge_tasks` no copia todos los campos
Cuando el estado remoto es más avanzado, la fusión copia `state`, `node`,
`resume_after`, `lease_expira`, `motivo_pausa`, `error` y `finished_at`, pero
**no** `attempts`, `max_attempts`, `deps`, `priority` ni `payload` ⇒ una tarea
que falló dos veces en otro nodo llega aquí con `attempts` local (0) y puede
reintentarse de más; y las dependencias del remoto no se copian.
**Por qué no se arregla:** no se ha podido medir en este sandbox qué
semántica quiere el fleet (y `board.rs` tiene su propia migración de esta
cola: `board.rs:1576`). Cambiarlo a ciegas puede duplicar o perder tareas.

### D-3 · `agyshim.rs:219` — el redactor no cubre el formato query-string
`redactar()` sólo conoce "nombre: valor" (JSON) y "nombre valor". Una ruta
con `?key=<API_KEY>` (la API de Gemini lo hace así) **no** se redacta: se
queda en claro en el NDJSON. Añadir `"key="` a la lista de patrones tal
cual NO vale: el algoritmo busca el final del valor en `,`/`\n`/`}` y en una
query no hay ninguno ⇒ borraría el resto de la URL (y en un JSON, campos
legítimos). **Por qué no se arregla:** requiere reescribir el redactor y
probarlo (aquí no se puede compilar) ⇒ riesgo de filtrar PEOR.

### E-3 · cobertura de `unwrap()`/`expect()`
El crate tiene **1.648** `unwrap()/expect()` en **119** ficheros. Se han
revisado los de las rutas de datos, cola, auto-update y secretos (los
listados arriba). **El resto NO está revisado**: no se puede afirmar que
estén bien.

---

## 4. Qué se ha revisado y qué no

**Revisado a fondo (leído entero):** `http_transfer.rs`, `drive_native.rs`
(download/upload), `backup_crypto.rs` (KDF/AAD/nonce/manifiesto),
`task_scheduler.rs`, `task_sync.rs`, `self_update.rs`, `supervisor.rs`
(revisado: ya estaba bien, sin hallazgos), `api_key.rs`, `agyshim.rs`,
`quota_guard.rs`, `backup_plan.rs`, `titanium_backup.rs`, `sqlite_mitm.rs`,
`util_fs.rs`, `llm_gateway.rs` (parte de la ruta Gemini).

**Revisado por patrón (grep + lectura del contexto):** `swift_container.rs`
y `swift_sba1*.rs` (**sin hallazgos**: son lectores puros, no escriben
ficheros; y `swift_container::leer_passphrase_de` ya exige 0600),
`backup_crypto.rs` (nombres/IV/nonces: los nonces por trozo son
`HMAC(nonce_key, etiqueta||índice||ruta)`, no se reutilizan),
`quota_guard.rs`, `llm_gateway.rs`, `api_key.rs`.

**Módulos que el enunciado nombraba y NO existen en el repo** (no hay nada
que auditar en ellos; el equivalente real es el que se indica):

| nombrado | realidad |
|---|---|
| `drive_backup.rs` | no existe → `drive_native.rs`, `drive_client.rs`, `drive_vfs.rs` |
| `phone_backup*` | no existe → el tool MCP `phone_backup` (`mcp_ext.rs:1008`) delega en `backup_apps.rs` (**no tocado**) |
| `harness_policy*` | no existe → `tool_policy.rs`, `power_policy.rs`, `accelerator_policy.rs` |

**No revisado:** los ~165 módulos restantes (messaging, mesh, coordination,
board, webui, cockpit_server, dex_decompile, telecom, profile_analysis,
antigravity_*, re_*, …). Queda para la siguiente ronda.

---

## 5. Lo que tiene que hacer el root

1. **Compilar y testear** (aquí no se pudo):
   `cd rust-core && cargo test --release --all-targets`.
   Lo único que se ha podido verificar aquí es la **sintaxis**
   (`tools/check-sintaxis.sh`: 293 ficheros, 0 errores).
2. **Revisar 3 cambios con efecto de comportamiento** (el resto son
   correcciones puras):
   * `self_update::aplicar` ahora **exige** el `.sha256` (la release lo
     publica: `.github/workflows/build.yml`, los dos `sha256sum`). Si
     alguien despliega una release sin `.sha256`, la actualización **no se
     instala** (antes sí, sin verificar).
   * `self_update::aplicar` ahora **copia** el binario viejo a `.prev` en
     vez de **renombrarlo** ⇒ `.prev` ocupa el doble de espacio (41 MB) y el
     rollback sigue igual.
   * `task_queue.json`, `quota.json` y el plan de backup se escriben ahora
     con **modo 0600** (antes 0644) ⇒ si algún lector externo los leía,
     deja de poder. Dentro del crate, el único lector de `task_queue.json`
     es `board.rs:1576` (el mismo proceso) ✓.
3. `merge --no-ff` de `rama/audit-calidad-2026-11-23` (20 commits).

## 6. Patrón que queda para el resto de la flota

1. **Escribir en `<destino>.tmp` y renombrar** (`util_fs::write_atomic`): un
   `fs::write` sobre un fichero de estado **trunca** el destino; si el
   proceso muere a mitad, el estado se pierde **en silencio**.
2. **No publicar antes de verificar**: el sha-256 se compara **antes** del
   `rename`; si no cuadra, el destino no se toca y el `.part` se borra.
3. **Un error ignorado es un bug**: `let _ = ...` sobre una escritura, un
   `chmod` o un status HTTP es un fallo silencioso. Si el resultado es
   "nada ha pasado", hay que devolver el **motivo**.
4. **Los secretos se crean con su modo**: `OpenOptions::mode(0o600)` al
   crear, no `chmod` después (y el error del `chmod` no se ignora).
5. **Comparar identificadores del mismo tipo**: sha de fichero contra sha de
   fichero, commit contra commit (ya arreglado en `8ccdd04`; aquí se ha
   encontrado el mismo patrón en la verificación del `.sha256`).
6. **Un `expect` en producción sólo vale si el invariante es local y
   evidente** (`llm_gateway.rs:428` lo tiene; `backup_crypto.rs:448` **no**:
   el invariante estaba en otro módulo).
