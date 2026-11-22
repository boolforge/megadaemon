# Board v1 — un solo tablón de tareas

> 2026-11-21.  Decisión del principal y del usuario: **un tablón**, **un
> formato**, **una** máquina de estados, **un** modelo de bloqueo.

## 1. El problema, medido

El enjambre tenía **tres** tableros de tareas coexistiendo:

| Tablón | Fichero | Formato | Bloqueo | ¿Lo ve el usuario? |
|---|---|---|---|---|
| `coordination` | `<state_dir>/board.sqlite` | SQLite | `lease` + `owner` | no |
| `mesh` | `<state_dir>/mesh_board.json` | JSON | `lease` + `claim` | no |
| `task_scheduler` | `<state_dir>/task_queue.json` (+ ndjson) | JSON | `deps` + recursos | **sí** (`/api/tasks`) |

Consecuencias medidas:

1. El cockpit servía **sólo** `task_scheduler::list_json()` ⇒ el trabajo real
   del enjambre era invisible para el usuario.
2. **Tres espacios de ids** y **tres máquinas de estados**.
3. `task_scheduler` guarda la cola **en memoria** (`OnceLock<Mutex<Vec<Task>>>`)
   y persiste con un `fs::write` del fichero entero: dos procesos en el mismo
   dispositivo se pisan, y un corte a mitad deja el disco desfasado de la RAM.
4. `mesh_board.json` tiene el mismo problema (lectura-modificación-escritura).
5. Los leases usan el reloj **local** de cada nodo: un nodo con el reloj
   atrasado puede robar una tarea viva.

## 2. La decisión

* **Almacén**: SQLite (`rusqlite`, ya dependencia) ⇒ transacciones, sin
  pérdida de actualizaciones, sin ficheros que se pisen.
* **Fila**: un único tipo `Task` (columnas indexadas + `payload` JSON abierto).
* **Estados**: `queued → claimed → running → done|failed|cancelled`, con la
  tabla de transiciones legales en **un solo sitio**
  (`board::can_transition`): nadie escribe un estado sin pasar por ahí ⇒ no
  hay deriva silenciosa.
* **Bloqueo**: dos reglas y sólo dos: `lease` (dueño + caducidad) y `deps`
  (dependencias insatisfechas, tabla `task_deps`).

## 3. Arquitectura

```
        board::BoardStore  (trait: el backend es reemplazable)
        ├── board::SqliteStore  (producción)
        └── board::LocalQueue   (modo degradado: memoria + WAL ndjson)
```

Que el almacén sea un **trait** es lo que evita el lock-in: mañana puede ser
un KV embebido, un tablón remoto o un servidor **sin tocar a quien lo usa**.

## 4. Resiliencia (entornos inestables)

| Fallo | Qué hace el tablón |
|---|---|
| `SQLITE_BUSY` | `busy_timeout` 5 s + reintentos con backoff exponencial |
| `state_dir` en NFS (sin WAL) | se detecta y se sigue en `DELETE`; el modo queda en `stats` |
| Reloj atrás en un nodo | **reloj monótono del tablón** (`board_clock`): nadie acorta un lease ni roba una tarea |
| Caída a mitad de una tarea | el lease caduca ⇒ `reclaim` la devuelve a la cola con su evento |
| Base corrupta | `snapshot` (`VACUUM INTO`) + restauración al abrir |
| Disco lleno / base bloqueada | errores **tipados** (`NoSpace`, `Locked`, `Busy`): el llamante puede degradar |
| El tablón no abre | **modo degradado**: `LocalQueue` (memoria + WAL ndjson) + `flush_to()` para reconciliar |
| Escritores concurrentes | `BEGIN IMMEDIATE` ⇒ un solo ganador por tarea (test de 2 hilos) |

Además, **todo** cambio de estado deja un evento en `task_events`: auditoría,
reconstrucción de proyecciones y diagnóstico forense.

## 5. Migración de los tres tableros

`board::migrate_legacy_*` — **idempotente** (cada fila legacy lleva
`legacy_id` + `legacy_board`; se puede repetir sin duplicar) y con
`--dry-run` en el CLI:

1. `coordination` (SQLite viejo) → `Task` (`status`, `owner`, `priority`,
   `report`→`last_error`, `lease_expires_at`).
2. `mesh` (`mesh_board.json`) → `Task` (`task_id`→`id`, `published`→`queued`,
   `payload` se conserva).
3. `task_scheduler` (`task_queue.json`) → `Task` + `task_deps` (`deps`→deps
   reales; `resources`→`payload.resources`).

El **original no se borra** ⇒ la migración es reversible.

## 6. Qué cambia en cada módulo (NO es mi carril)

* `coordination.rs`: pasa a ser cliente del tablón unificado.
* `mesh.rs`: su JSON pasa a réplica (o queda deprecado).
* `task_scheduler.rs`: deja de ser tablón y pasa a ser el **ejecutor**
  (reclama, ejecuta, reporta).
* `cockpit_server.rs`: `/api/tasks` sirve `board::tasks_json()` ⇒ el usuario ve
  por fin **las tres fuentes en una lista**.

> `cockpit_server.rs` está fuera de mi carril (regla del enjambre) y
> `coordination.rs`/`mesh.rs`/`task_scheduler.rs` son de sus dueños.  Yo
> dejo el módulo `board` y la CLI, que son **aditivos**.

## 7. Preparado para el futuro

* **Esquema versionado**: `schema_version` + cadena de migraciones **lineal y
  transaccional**; añadir una columna es un `ALTER TABLE` idempotente.
* **Payload abierto**: lo no indexado viaja en JSON ⇒ hoy se meten `resources`
  o `deps` sin migrar el esquema; mañana se tipa una columna sin romper las
  filas viejas.
* **Sin proveedor, sin nube, sin formato propietario**: SQLite + JSON.
* **Sin red**: los tests son deterministas y offline.

## 8. Verificación

`rust-core/src/board.rs` lleva **12 tests** que fijan los invariantes:
transición ilegal rechazada (y los terminales no resucitan); ciclo de vida
con rastro en los eventos; dos agentes no reclaman lo mismo; el lease caduca y
la tarea vuelve a la cola; un reloj atrasado no roba el lease; las
dependencias bloquean hasta resolverse; la migración es idempotente; el modo
degradado no pierde tareas; snapshot + base corrupta; 2 escritores concurrentes
sobre 20 tareas = 20 reclamos; las estadísticas reflejan el estado real; la
vista JSON no inventa nada.

CLI: `md-board --help` (tasks, ready, stats, events, add, claim, start,
report, renew, reclaim, **migrate --dry-run**, snapshot, doctor).

## 9. Riesgos y reversión

| Riesgo | Mitigación |
|---|---|
| Un módulo legacy sigue escribiendo en su fichero | el tablón único es el que se **lee**; los legacy quedan deprecados y sus ficheros no se borran |
| La migración duplica tareas | `legacy_id` con índice único + `--dry-run` antes |
| El modo degradado tapa un fallo del tablón | `doctor` informa del modo (`memory`/`delete`/`wal`) y del tamaño de la cola local |
| Rollback | los ficheros legacy siguen ahí; `git revert` del commit de integración |
