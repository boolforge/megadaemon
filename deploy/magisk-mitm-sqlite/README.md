# Módulo Magisk: MITM de la DB de Antigravity

Este módulo es la **mitad de arranque** del MITM de SQLite: hace que el
bind-mount que sirve la DB "mentirosa" **sobreviva a un reinicio**.

## Qué sustituye

La DB de conversaciones de Antigravity.  Hay **dos** ubicaciones posibles
(medidas el 2026-11-22 en el S21 Ultra, y en conflicto entre módulos del
repo: `antigravity_sessions.rs` la veía en el nivel superior del directorio
de la app y `sqlite_mitm.rs` en `conversations/`).  El helper **detecta**
cuál existe:

```
/data/user/0/com.agy/files/.gemini/antigravity-app/conversation_summaries.db
/data/user/0/com.agy/files/.gemini/antigravity-app/conversations/conversation_summaries.db
```

## Cómo

`mount -o bind <db_servida> <db_original>`

* La app (com.agy) escribe en la **servida**.
* La **original** queda intacta en su partición (recuperable).
* El motor de réplica del daemon (`sqlite_mitm::replicar`, sólo lectura +
  `busy_timeout` 2000 ms) lee por la ruta montada ⇒ **captura** lo que la
  app escribe sin bloquearla.  La DB servida se prepara en **WAL**: en WAL
  el lector y el escritor conviven.

## Ciclo de vida

| Momento | Qué hace |
|---|---|
| `post-fs-data.sh` | Prepara `$STATE_DIR/mitm/` y `$STATE_DIR/mitm/backup/`. **No monta nada** (en esta fase `/data` puede no estar descifrado). |
| `service.sh` (late start) | Si `mitm/estado.json` dice que el MITM estaba **activo** y la DB servida existe, vuelve a aplicar el bind-mount (el montaje no sobrevive al reinicio). Idempotente. |
| `uninstall.sh` | Desmonta el bind-mount (aunque no haya estado: puede haber un montaje huérfano) y deja el móvil como estaba. |

## Desactivar en caliente

Desde el daemon (tool MCP `sqlite_mitm`, acción `desactivar`) o a mano:

```sh
sh /data/adb/modules/megadaemon_mitm_sqlite/mitm-sqlite.sh desactivar
```

El `umount` devuelve la ruta al **fichero real**; después el daemon
re-mergea las filas servidas en la original ⇒ Antigravity no pierde
conversaciones.

## Lo que NO se toca nunca

* El fichero original: no se borra, no se escribe encima (salvo el
  re-merge final, que es una escritura *normal* de SQLite sobre el
  fichero real ⇒ es la operación de escritura legítima de la app).
* Los ficheros `-wal`/`-shm` de la original: se **copian** al directorio de
  respaldo antes de apartarlos; nunca se destruyen.

## Estado de verificación

`NO VERIFICADO` en dispositivo: el S21 Ultra del usuario está caído
(2026-11-23) y ADB es inalcanzable desde el sandbox.  Todo lo anterior
está **derivado del código del repo** y de las mediciones del
2026-11-22; el comando exacto de verificación está en
`docs/MITM-SQLITE-Y-PROXY.md`.
