# Sesión 2026-11-22 — Móvil actualizado, Antigravity unificado, backlog desbloqueado

Todo lo que sigue está **medido en el Samsung S21 Ultra** (Exynos 2100, Magisk)
con el comando exacto o el fichero exacto. Nada es una suposición.

---

## 1. El móvil: de v13.1 a v13.4.0, con verificación en el dispositivo

| | Antes | Ahora |
|---|---|---|
| Binario | `/data/local/tmp/megadaemon/megadaemon-core` | el mismo path |
| Tamaño | 8.859.456 B | **11.967.640 B** |
| Versión | v13.1 | **v13.4.0** (`/api/health` → `version`) |
| Tools MCP | 26 | **62** |
| SHA-256 | `718c1c59ccf6b6bf…` | `5ebc2929345de4d7095fdbf6e48dfba921993c7abc0959b4e0defb50f1b49aa3` |

Procedimiento (todo reversible):

1. Release del CI `v63343226b03ad4397ae8fe53537837c357a8ca34` (publicada el
   2026-11-22T01:17:35Z). Asset `megadaemon-core-android-arm64`.
2. Descarga al sandbox + `sha256sum` **coincidente con el `.sha256` oficial**.
3. Subida al móvil por `POST /mcp` → tool `root_exec` (ver §5).
4. `sha256sum` **en el móvil** sobre el fichero ensamblado: idéntico.
5. `cp megadaemon-core megadaemon-core.v13.1.bak` (rollback) y `mv` del nuevo.
6. `pkill -f 'megadaemon-core cockpit'` ⇒ lo relanza el supervisor
   `/data/adb/service.d/99-megadaemon.sh` (backoff + binario `.prev`).
7. `/api/health` → `{"status":"ok","version":"13.4.0"}` y 62 tools en `/mcp`.

**MiniMax verificado con el binario nuevo**:

```
megadaemon-core chat minimax-m3.1-flash "Responde exactamente: MINIMAX_OK"
{"reply":"MINIMAX_OK","vendor":"MiniMax","latency_ms":1204.0,"tokens_used":373,…}
```

---

## 2. Unificación de runtimes (la "pelea" entre Antigravity y MegaDaemon)

**Medido antes**: Antigravity mantenía **su propio clon** del repo
(`/data/user/0/com.agy/files/megadaemon/`) y **su propio runtime**
(`/data/user/0/com.agy/files/runtime/bin/`, 860 entradas: stubs de 781-862 B
que envuelven el binario real). El daemon de producción vivía aparte, en
`/data/local/tmp/megadaemon/megadaemon-core`. Dos copias, dos binarios,
dos versiones.

**Medido ahora**:

```
/data/user/0/com.agy/files/runtime/bin/megadaemon-core
  11.967.640 B  sha256 5ebc2929345de4d7095fdbf6e48dfba921993c7abc0959b4e0defb50f1b49aa3
  uid 101573 (= u0_a1573, la app)   .prev = copia del anterior
```

Es **byte a byte el mismo binario** que corre el daemon ⇒ una sola versión,
cero peleas. El plugin MCP ya apuntaba al daemon local
(`mcp_config.json` → `http://127.0.0.1:9090/mcp` con `X-API-Key` +
`X-Seere-Auth-Token`), así que **Antigravity ya usa a MegaDaemon como motor**.

**Pendiente de automatizar** (implementado en `rust-core/src/antigravity_clone.rs`,
aún no compilado para el móvil): el daemon re-sincroniza solo el
`mcp_config.json` al arrancar, para que rotar la API key o el token de
sesión **no** rompa las 52 tools del plugin.

---

## 3. El backlog de Antigravity estaba muerto (y ya no)

**Medido**: el último ciclo del runner era del **2026-11-22 01:14** (14 h
antes) ⇒ el planificador llevaba 14 h parado. La tarea `task_553653`
(`minimax_bootstrap`) **nunca se había ejecutado** desde el 19-Nov.

**Causa raíz**: `/data/adb/service.d/97-antigravity-tasks.sh` **no existía**
⇒ el supervisor nunca se instaló ⇒ ningún reinicio relanzaba el backlog.

### Cinco bugs reales encontrados y corregidos

| # | Bug | Evidencia |
|---|---|---|
| 1 | `md_mcp.sh` pedía `auth_token`; v13.4.0 devuelve `token` **y exige dos cabeceras** (`X-Seere-Auth-Token` **+** `X-API-Key`) | `{"error":"unauthorized","hint":"falta X-API-Key o Authorization: Bearer"}` ⇒ **la flota perdía el acceso root al móvil justo al actualizarlo** |
| 2 | `antigravity-task-supervisor.sh --install` **nunca funcionó**: `SRC_DIR` y `STATE_DIR` son el mismo directorio ⇒ `cp` responde "are the same file" ⇒ `return 1` **antes** de crear el `service.d` | `ls /data/adb/service.d/97-antigravity-tasks.sh` → *No such file* |
| 3 | El catch-up ejecutaba el **nombre** de la tarea en vez del comando: el TSV tiene 11 columnas (`id, name, …, command`) y el supervisor leía `read -r id cmd` ⇒ se quedaba con la columna 2 | `sh: 1: minimax_bootstrap: not found` |
| 4 | Al arreglar el 3, el supervisor hacía `tail -n +2` (para saltar la cabecera) ⇒ **se comía la primera tarea pendiente** ⇒ catch-up vacío | el catch-up no lanzaba nada |
| 5 | `$'\t'` no existe en el `sh` de Android (mksh) ⇒ el parser portable usa `printf '\t'` | `sh -n` no lo detecta; sí en el móvil |

**Estado tras las correcciones** (medido en el móvil):

```
[15:45:41] catch-up: ejecuto 'task_553653' (timeout 300s) -> …/catchup-…-task_553653.log
[15:46:30] catch-up: 'task_553653' terminado con rc=1
```

El `rc=1` **ya no es del runner**: es del propio script del usuario
`bootstrap_core.sh`, que falla con
`ln: failed to create symbolic link '//agy_standalone/bin/libloader.so'`
(ruta con variable vacía). **No se ha tocado ese fichero**: es trabajo ajeno.

El supervisor queda instalado en `service.d` ⇒ el backlog se relanza en
**cada reinicio** y hace catch-up de lo pendiente.

---

## 4. Aviso de seguridad: credencial expuesta

`mcp_config.json` (de la app Antigravity, legible por ella) contiene la
**API key de MiniMax en claro**. Esa misma clave apareció en el transcript
de esta sesión. **Hay que rotarla** en la cuenta de MiniMax y actualizar
`providers.minimax.api_key`; el daemon la usa para el chat. No se ha
imprimido ni copiado a ningún fichero del repo.

La `X-Seere-Auth-Token` del `mcp_config.json` también estaba escrita a
mano ⇒ con la sincronización automática (§2) deja de ser un punto de rotura.

---

## 5. Cómo se sube un binario al móvil (mecanismo medido)

Restricciones reales del dispositivo:

* **no hay `curl` ni `wget`** (`toybox wget` → rc=127);
* el comando entero es **un argumento** de `su -mm -c` y Android limita
  cada argumento a **128 KB** (`MAX_ARG_STRLEN`); el total (`ARG_MAX`,
  2 MB) no es el límite real;
* la salida del `root_exec` viene truncada (~4 KB) ⇒ no se pueden listar
  147 ficheros desde fuera;
* las escrituras exigen `confirm_destructive: true`.

Solución (`tools/subir_binario_movil.sh` + `tools/completar_subida_movil.sh`):

1. trocear el binario (80 KiB);
2. `base64` de cada trozo repartido en argumentos de 90 KB de `printf %s`
   (cabecera del comando ≈ 108 KB ⇒ cabe);
3. `POST /mcp` (JSON-RPC) con `root_exec` ⇒ el cuerpo HTTP no tiene el
   límite del argv;
4. **verificar el SHA-256 de cada trozo dentro del móvil** (147 hashes
   esperados como argumentos: 9,7 KB) ⇒ se reenvía sólo el que esté mal;
5. ensamblar con **lista explícita** de nombres (con un glob entran
   ficheros de subidas anteriores) y comprobar el SHA-256 final.

Measuras: 147 trozos, ~3,5 min de subida, 1 reenvío (`parte_004`,
corrupto en tránsito) y verificación final idéntica al original.

---

## 6. Qué queda pendiente

* **Compilar el ARM64 con el código de esta sesión** (gateway
  `/v1/chat/completions`, auto-update permanente, `antigravity_clone`,
  sincronización del `mcp_config.json`, fix del CSC). El build estaba en
  marcha con `opt-level=1` (el `opt-level=3` muere de OOM: el sandbox
  tiene 3 GB).
* **Auto-update**: hasta que ese binario esté en el móvil, el daemon
  **no** se actualiza solo (el actual no tiene el subcomando `update`).
* **Rotar la API key de MiniMax** (§4).
* **`bootstrap_core.sh`** (fichero del usuario) con la ruta `//agy_standalone/…`
  rota ⇒ hay que corregirlo para que `task_553653` termine en verde.
