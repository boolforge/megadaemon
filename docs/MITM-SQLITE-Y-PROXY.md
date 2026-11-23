# MITM DE SQLITE + PROXY REAL DE ANTIGRAVITY

> Rama: `rama/mitm-proxy-2026-11-23` · 2026-11-23
>
> **Estado del dispositivo: CAÍDO.**  El S21 Ultra del usuario no es
> alcanzable desde el sandbox (ADB inalcanzable, túnel 502).  **TODO lo que
> toca el móvil está `NO VERIFICADO`** y lleva el comando exacto de
> verificación.  Lo que sí está verificado aquí: el código **parsea**
> (`rustfmt`, sin RAM), los **scripts pasan `sh -n`**, el helper **responde
> JSON** en sus tres acciones y la **lógica pura tiene tests** que el root
> compila.

---

## 0. Resumen

| # | Punto | Qué estaba | Qué hay ahora |
|---|---|---|---|
| 1 | MITM de la DB de Antigravity | Un **plan** de bind-mount que nadie ejecutaba | Bind-mount **real** (módulo Magisk + helper root), DB servida en WAL, **captura** por el motor de réplica existente, **re-merge** al desactivar |
| 2 | `agyshim` | Interceptaba y registraba (ni siquiera reenviaba: usaba un GET) | **Proxy real**: atiende `/v1internal/*`, `/v1beta/*` y `/proxy/*`, reenvía **por el gateway canónico**, y deja la traza del modelo (pedido / real / sustituido) en la respuesta y en el NDJSON |
| 3 | `antigravity_sessions` | Estado, lista, muertos/colgados, árbol, detalle | + **estado normalizado**, **tokens y modelo en uso**, **correlación con el NDJSON del shim** |

Ficheros nuevos:

```
deploy/magisk-mitm-sqlite/mitm-sqlite.sh   <- la implementación ÚNICA del montaje
deploy/magisk-mitm-sqlite/module.prop
deploy/magisk-mitm-sqlite/post-fs-data.sh
deploy/magisk-mitm-sqlite/service.sh
deploy/magisk-mitm-sqlite/uninstall.sh
deploy/magisk-mitm-sqlite/customize.sh
deploy/magisk-mitm-sqlite/README.md
tools/mitm-sqlite.sh                        <- envoltorio local del helper
rust-core/src/antigravity_fake_so.rs        <- el .so instrumentado con un FICHERO FALSO
rust-core/src/redact_forma.rs               <- la segunda pasada de redaccion (por forma)
docs/MITM-SQLITE-Y-PROXY.md                 <- este documento
```

---

## 1. MITM de SQLite con bind-mount REAL

### 1.1 Qué fichero se sustituye y cómo

**La DB**: el índice de conversaciones de Antigravity.  Hay **dos**
ubicaciones en circulación y el repo se contradecía:

| Módulo | Ruta que leía |
|---|---|
| `antigravity_sessions.rs` | `…/antigravity-app/conversation_summaries.db` |
| `sqlite_mitm.rs` (antes) | `…/antigravity-app/conversations/conversation_summaries.db` |

Con el MITM activo, **montar el fichero equivocado = no montar nada**.
Ahora `sqlite_mitm::candidatas_db_antigravity()` **detecta** cuál existe y
`ruta_db_antigravity()` devuelve la buena.

**Cómo**: `mount -o bind <db_servida> <db_original>`.

* La app (`com.agy`) **escribe** en la DB servida.
* La original **queda intacta** en su partición (recuperable).
* Se monta desde un **módulo Magisk** (`deploy/magisk-mitm-sqlite/`):

| Script | Cuándo | Qué hace |
|---|---|---|
| `post-fs-data.sh` | antes de zygote | Prepara `$STATE_DIR/mitm/` y `$STATE_DIR/mitm/backup/`. **No monta nada** (en esa fase `/data` puede no estar descifrado) |
| `service.sh` | late start | Si el estado dice que el MITM estaba activo y la DB servida existe, **vuelve a aplicar el bind-mount** (el montaje no sobrevive al reinicio). Idempotente |
| `uninstall.sh` | desinstalación | Desmonta el bind-mount (aunque no haya estado: puede haber un montaje huérfano) y deja el móvil como estaba |

### 1.2 Cómo se capturan las escrituras SIN bloquear la original

El **hook** es el propio bind-mount: tras el montaje, la ruta original
apunta a la DB **servida**, así que el motor de réplica **ya existente**
(`sqlite_mitm::replicar`, lectura **SOLO LECTURA** con
`busy_timeout` 2000 ms) lee por esa ruta y captura lo que la app escribe.

* La DB servida se prepara en **WAL** ⇒ lector y escritor **conviven**
  (en *rollback-journal* un lector SÍ bloquearía al escritor ⇒ la app se
  quedaría colgada, que es justo lo que hay que evitar).
* La réplica **nunca** escribe en la DB de la app.
* La DB canónica (`sesiones.db`) es del daemon: WAL, indexada, con tabla
  de réplicas y de huérfanos.

Acciones de la tool MCP `sqlite_mitm`:

| Acción | Efecto |
|---|---|
| `estado` | Qué está montado, qué hay en disco, cuánto se ha capturado, y el estado del último `activar`/`desactivar` |
| `activar` | Checkpoint del WAL original → genera la DB servida (WAL, filtrada) → monta el bind-mount → guarda el estado (0600) |
| `desactivar` | Desmonta → **re-mergea** las filas servidas en la original → borra la DB servida |
| `capturar` |Replica AHORA lo que la app ha escrito (sin montar nada) |
| `servida` | Genera la copia **sin** montarla (para inspeccionarla) |
| `montaje` / `plan` | Devuelve el comando exacto del montaje |

**Efectos colaterales del `activar`, declarados:**

1. Se hace `PRAGMA wal_checkpoint(TRUNCATE)` sobre la original (best
   effort, 2 s) para que la copia esté completa.  Si la app está
   escribiendo, **no se bloquea más de 2 s** y se avisa
   (`checkpoint_original: false` + `checkpoint_motivo`).
2. Los ficheros `-wal`/`-shm` de la original se **copian** al directorio de
   respaldo y se apartan: si la app los encontrara, intentaría
   reproducir transactions de la DB original sobre la DB servida ⇒
   *"database disk image is malformed"*.  **Nunca se borran**: quedan como
   copia forense.
3. Se hace `chown`/`chmod` de la DB servida con el uid/gid/modo de la
   original, para que la app pueda escribirla.

### 1.3 Cómo se restaura la DB original

`desactivar`:

1. `umount <original>` ⇒ la ruta vuelve a ser el **fichero real**.
2. **Re-merge**: las filas de la DB servida se insertan en la original con
   `INSERT OR IGNORE` ⇒
   * las conversaciones que la app creó durante el MITM **se conservan**;
   * las que la original ya tenía (incluidas las **ocultas** o con el
     estado reescrito por el MITM) **no se tocan** ⇒ la "mentira"
     sobrevive a la restauración.
3. Se borra la DB servida y sus `-wal`/`-shm`.
4. Los `-wal`/`-shm` apartados **no se restauran** (reproducir un WAL
   viejo sobre la DB ya re-mergeada la corrompería); las copias se
   conservan en `$STATE_DIR/mitm/backup/`.

### 1.4 Por qué un helper en shell y no todo en Rust

El montaje necesita el **namespace de montaje de Magisk** (`su -mm`), y ese
namespace es *por invocación*: si el daemon hiciera el `mount` en una
invocación de `su` y el `umount` en otra, no serían el mismo namespace.
Por eso **toda** la secuencia privilegiada (montar / desmontar / estado)
ocurre DENTRO de una sola invocación del helper.

`deploy/magisk-mitm-sqlite/mitm-sqlite.sh` es la **implementación única**;
`tools/mitm-sqlite.sh` es su envoltorio local y el módulo Magisk lleva su
propia copia (`customize.sh` la instala).

---

## 2. `agyshim` como PROXY REAL

### 2.1 El antes y el después

**Antes**: el shim leía la petición, la anotaba en el NDJSON y devolvía lo
que ya venía.  Peor: el "upstream" se reenviaba con `http_transfer::get_texto`
⇒ **un GET para una POST** ⇒ el proxy no funcionaba.

**Ahora**:

```
Antigravity (language server, 127.0.0.1:45157)
   │  POST /v1internal:generateContent      (dialecto Gemini, `genai` SDK)
   │  POST /v1internal:streamGenerateContent
   ▼
shim  (agyshim::proxy)
   │  registra el NDJSON (0600, redactado) · aplica las reglas · decide el dialecto
   ▼
GATEWAY CANÓNICO  (llm_gateway)
   │  routing real + fallback real + credenciales del vault (0600)
   ▼
proveedor real (Gemini / Claude / OpenAI-compatible)
```

* Rutas servidas: `/v1internal/*`, `/v1beta/*` y `/proxy/*`
  (`/proxy/v1internal:generateContent` es lo mismo que
  `/v1internal:generateContent`).
* Dialectos: **Gemini** (`candidates`), **OpenAI** (`choices`) y
  **catálogo** de modelos.
* El shim **nunca ve la credencial del proveedor**: la tiene el vault del
  daemon y la usa `providers::execute_chat`.
* La autenticación **no cambia**: estas rutas pasan por el gate global de
  API key que hay antes del `match` de rutas ⇒ el túnel público sigue
  exigiendo credencial.

### 2.2 La trazabilidad del modelo

La respuesta lleva, dentro del bloque `megadaemon`:

| Campo | Significado |
|---|---|
| `requested_model` | Lo que **pidió** el cliente (`megadaemon`, `gemini-2.5-pro`…) |
| `real_model` | El id del modelo que **contestó** (`minimax-m3.1-flash`…) |
| `substituted` | `true` si el que contestar **no** era el pedido |
| `provider_id`, `provider_name`, `vendor`, `provider_trace`, `quota` | Lo que ya había (quién atendido y a quién se saltó) |

Con el alias `megadaemon`, `substituted` es **siempre `true`**: el alias no
es un modelo, es el router.  Si el cliente pide un modelo concreto
(`/v1beta/models/gemini-2.5-pro:generateContent`), ese va **primero** en la
cascada y `substituted` dice si acabó respondiendo otro.

El bloque SSE (streaming) **también** lleva la trazabilidad (antes no).

### 2.3 Credenciales: nunca en el NDJSON

* El NDJSON se escribe con modo **0600**.
* Los cuerpos van redactados por `agyshim::redactar`, que ahora tiene
  **dos** pasadas:
  1. por **nombre** de campo (`api_key`, `Authorization`, `bearer `,
     `x-goog-api-key`…): tapa nombre **y** valor;
  2. por **forma** de token (`sk-`, `sk-cp-`, `AIza`, `gsk_`, `xoxb-`,
     `xapp-`, `ghp_`): una credencial que viaja **suelta** ya no se cuela.
* Un token de menos de 12 caracteres **no** se redacta (evita destrozar
  texto normal).

---

## 3. `antigravity_sessions` ampliada

| Qué | Dónde | Nota |
|---|---|---|
| **Estado normalizado** (`EN_CURSO`/`IDLE`/`COLGADA`/`MUERTA`) | campo `estado_norm` de cada sesión | las reglas se deciden **una vez** y no en cada consumidor |
| **Tokens y modelo en uso** | `tokens_y_modelo_en()` | lee `conversations/<id>.db` en **solo lectura** con `busy_timeout`; el esquema **no está medido**, así que el lector es **defensivo**: suma las columnas que *parecen* tokens y devuelve el **esquema que ha encontrado** (tablas y columnas) para poder medirlo en el dispositivo |
| **Correlación con el NDJSON del shim** | `correlacion_shim_en()` | cruza por **ventana temporal** (`last_user_input_time` … `last_modified_time`) y devuelve, por sesión, los **modelos pedidos**, los **modelos reales** y las **sustituciones** |

Acciones nuevas de la tool `antigravity_sessions`: `tokens` y
`correlacion` (también por HTTP: `/api/antigravity/sessions?accion=tokens`).

---

## 3-bis. El `.so` de Antigravity: un **fichero falso** que el daemon posee

*(petición del usuario, 2026-11-23: «hazlo también con el `.so`, que
seguro que hace falta un fichero falso y que poseas el original para
instrumentarlo»)*

### El problema

El language server (`liblanguage_server.so`, 181.358.800 B) lleva dentro,
como constante, la URL del proveedor:
`https://daily-cloudcode-pa.googleapis.com`.  Para que su tráfico pase por
el MegaDaemon hay que **sustituir esa constante** por una URL local de la
**misma longitud** (así el layout del ELF no cambia).  Eso ya lo hacía
`antigravity_endpoint::aplicar_endpoint` (parche **en el sitio**, con el
respaldo `.bak-megadaemon` **dentro del directorio de la app**).

### Lo que hace el módulo nuevo (`rust-core/src/antigravity_fake_so.rs`)

| Paso | Qué |
|---|---|
| 1 | El daemon **posee el ORIGINAL**: lo copia a su **propio** directorio (`<state>/agy/lib/liblanguage_server.so.original`) y anota su sha256 ⇒ **fuera del alcance de la app** (que antes podía tocar el `.bak`) |
| 2 | El daemon **fabrica el FALSO**: los bytes del **original** con la URL sustituida ⇒ `liblanguage_server.so.fake`, un artefacto aparte (se puede inspeccionar, empujar y comparar) |
| 3 | Lo **instala EN EL SITIO**: se escribe **encima** del fichero de la app **sin borrarlo antes** ⇒ se conservan el **inode**, la **etiqueta SELinux**, el dueño y los permisos (si se borrara y se recreara, el contexto SELinux se perdería y la app no podría cargarlo) |
| 4 | `desactivar` **restaura** el original **byte a byte** y verifica su sha256 |

El **original** es la fuente de verdad: el falso se fabrica **siempre** a
partir de él (nunca del fichero actual, que podría estar ya parcheado).

### Superficies

* Tool MCP `antigravity_fake_so`: `plan` (no escribe) · `activar`
  (`confirm=true`) · `desactivar` (`confirm=true`) · `estado`.
* CLI: `md-antigravity fake [plan|activar|desactivar|estado] [--confirm]`.

### Lo que NO está verificado

El parche en sí **ya estaba medido** (2026-11-22, el SHA del manifiesto
cambia y el LS sigue arrancando).  Lo **nuevo** —la propiedad del original
por el daemon, el falso como artefacto separado y la instalación en el
sitio conservando el inode— **NO se ha podido probar en el móvil**
(caído desde el 2026-11-23).

```bash
# Verificación (en el móvil):
md-antigravity fake plan          # ver los sha256 y los cambios
md-antigravity fake activar --confirm
sha256sum /data/user/0/com.agy/files/runtime/bin/liblanguage_server.so \
          /data/local/tmp/megadaemon/state/agy/lib/liblanguage_server.so.fake \
          /data/local/tmp/megadaemon/state/agy/lib/liblanguage_server.so.original
# el del .so de la app debe coincidir con el del .fake, y el .original con el de git
md-antigravity fake desactivar --confirm
```

---

## 4. Qué queda **NO VERIFICADO** (móvil caído)

| # | Afirmación | Por qué no está verificada | **Comando exacto** de verificación |
|---|---|---|---|
| V1 | El bind-mount monta y la app escribe en la DB servida | no hay dispositivo | `md-mitm sqlite_mitm estado` (tool MCP) y en el móvil: `mount \| grep conversation_summaries` |
| V2 | La DB servida queda en WAL y la app puede escribirla | no hay dispositivo | `sqlite3 <served> 'PRAGMA journal_mode;'` + `ls -l <served>` (uid de `com.agy`) |
| V3 | La captura replica lo que la app escribe | no hay dispositivo | `md-mitm sqlite_mitm capturar` y comparar con `conversation_summaries.db` |
| V4 | El re-merge no pierde conversaciones | no hay dispositivo | `md-mitm sqlite_mitm desactivar` y `sqlite3 <original> 'SELECT COUNT(*) FROM conversation_summaries;'` |
| V5 | El proxy `/v1internal:generateContent` devuelve `candidates` con la traza | no hay dispositivo | `md-mitm agyshim proxy '{"ruta":"/v1internal:generateContent","cuerpo":"{\"contents\":[{\"role\":\"user\",\"parts\":[{\"text\":\"ping\"}]}]}"}'` |
| V6 | El LS de Antigravity usa el gateway (el `megadaemon` de la GUI) | no hay dispositivo | `md-antigravity integrar --estado` + `POST https://127.0.0.1:45157/exa.language_server_pb.LanguageServerService/GetMcpServerStates {}` |
| V7 | El modelo `megadaemon` aparece **habilitado** en el selector | la app regenera el bundle web | `grep -c 'osName==="linux"' /data/user/0/com.agy/files/web/generated/main.js` (debe ser 0 tras `integrar`) |
| V8 | El esquema real de `conversations/<id>.db` | no se ha podido leer | `sqlite3 /data/user/0/com.agy/files/.gemini/antigravity-app/conversations/<id>.db '.schema'` |
| V9 | El **fichero falso** del `.so` arranca y el LS usa el gateway | el móvil está caído (el parche en sí ya estaba medido) | `md-antigravity fake activar --confirm` + `sha256sum` de los tres ficheros (ver §3-bis) |

### Lo que **sí** está verificado en este entorno

* Los **8 ficheros Rust** tocados **parsean** (`rustfmt --edition 2021
  --emit stdout`, sin RAM, sin compilar).
* Los **6 scripts de shell** pasan `sh -n`.
* El helper responde **JSON válido** en `estado` / `activar` (fallo
  explícito) / `desactivar` / acción desconocida, y **sale con código 1**
  cuando falla (fail-closed).
* La **lógica pura** (detección de rutas, dialectos, traza del modelo,
  redaction, re-merge, correlación) tiene **tests** que el root compila.

---

## 5. Cómo reproducirlo

```bash
# En el móvil (o en el sandbox con root):
md-mitm sqlite_mitm estado                 # ¿qué está montado?
md-mitm sqlite_mitm activar                # monta (escribe en la app)
md-mitm sqlite_mitm capturar               # replica lo escrito
md-mitm sqlite_mitm desactivar             # desmonta y re-mergea

# El proxy:
md-mitm agyshim estado                     # rutas, modelos pedidos/reales
md-mitm agyshim log                        # NDJSON (redactado)
md-mitm agyshim proxy                      # llamada de prueba

# Las sesiones:
md-mitm antigravity_sessions tokens conversation_id=<id>
md-mitm antigravity_sessions correlacion

# A mano (el helper):
sh /data/adb/modules/megadaemon_mitm_sqlite/mitm-sqlite.sh estado
```

---

## 6. Decisiones que conviene conocer

1. **Una sola implementación del montaje** (shell), llamada desde Rust y
   desde el módulo Magisk ⇒ no hay dos caminos que se desincronicen.
2. **El NDJSON es público** ⇒ 0600 + redacción en dos pasadas.  La
   credencial del proveedor nunca lo pisa (la tiene el vault).
3. **La DB servida en WAL**: sin WAL, el lector (la réplica) bloquearía
   al escritor (la app) en modo *rollback-journal*.
4. **El re-merge usa `INSERT OR IGNORE`**: las filas ocultas por el MITM
   no se "resucitan" al restaurar.
5. **El helper trabaja con `mount -o bind`** (no `--bind`): `toybox`,
   `busybox` y `util-linux` lo entienden; `--bind` sólo lo entienden
   `busybox`/`util-linux`.
6. **El gate de la GUI vive en el bundle web**: si la app lo regenera,
   el parche se pierde ⇒ `integracion_estado` lo comprueba y lo dice.
