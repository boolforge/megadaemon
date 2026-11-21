# Grabaciones — índice unificado y backup de seguridad

> Módulo: `rust-core/src/recordings.rs` · CLI: `rust-core/src/bin/md-recordings.rs`
>
> **Alcance**: este módulo **no graba**. Indexa, unifica y respalda las
> grabaciones **que ya existen**. La grabación nativa de Samsung y su aviso
> legal (`RecordingAllowedByMenu,disclaimer_latin` ⇒ **con aviso**) son cosa
> de `call_recorder` / CSC: aquí no se toca ni CSC ni el grabador.
>
> **Estado**: implementado y testeado (10 tests deterministas, sin red y sin
> dormir).  **Pendiente de cableado**: `pub mod recordings;` en
> `rust-core/src/lib.rs` (1 línea, lo pone el dueño del fichero) y los
> endpoints de la GUI (§7).

---

## 1. El problema

Hay varios grabadores en el móvil y **cada uno usa su propio árbol de
directorios y su propio esquema de nombres**:

| Grabador | Dónde deja el audio | Cómo lo nombra |
|---|---|---|
| Nativa de Samsung | `/sdcard/SoundRecordings/…` | `<numero>_<timestamp>.amr` |
| Samsung Voice Recorder | `/sdcard/Recordings/` | `VoiceRecording_….m4a` |
| Telegram | caché de la app | nombre opaco del cliente |
| WhatsApp | `/sdcard/WhatsApp/Media/…` | `<numero>_<timestamp>.opus` |
| BCR | su propio árbol | `<AAAAMMDD-HHMMSS>.<ext>` |

Y el nombre **cambia**: con la versión nueva de la app, con el firmware, y
porque el sistema mueve los ficheros de una carpeta a otra. Por eso el índice
**no puede** apoyarse en el nombre: el **SHA-256 es la identidad del
contenido** (ver §3).

Lo que hace este módulo, en tres pasos:

1. **Indexa** la raíz de audio (recursivo, sólo extensiones de audio).
2. **Unifica**: reconcilia lo que *declara* cada grabador con lo que *existe*
   en disco y clasifica cada grabación (§2).
3. **Respalda**: copia de seguridad **incremental** con MANIFEST y
   verificación posterior (§5).

---

## 2. Modelo de datos

### 2.1 Grabación (`Recording`)

Los nombres JSON son el contrato de la API (§7); los identificadores en Rust
están en inglés.

| JSON | Tipo | Significado |
|---|---|---|
| `id` | string | `<app>:<sha256[..16]>` — estable |
| `ruta` | string | ruta del fichero en el móvil (o la declarada si no existe) |
| `bytes` | number | tamaño |
| `sha256` | string | hash **de referencia** (el declarado; si no hay, el de disco) |
| `duracion_s` | number | duración en segundos (0 si no se sabe) |
| `mtime` | number | fecha de modificación (epoch, segundos) |
| `fuente` | string | `system` · `whatsapp` · `telegram` · `signal` · `discord` · `unknown:<pkg>` |
| `app` | string | app que declara la grabación (paquete Android o nombre corto) |
| `tipo` | string | formato (`wav`, `m4a`, `amr`…) |
| `estado` | string | ver §2.2 |
| `sha256_disco` | string? | hash **calculado** en disco |
| `sha256_declarado` | string? | hash **declarado** por la app |
| `emparejado_por` | string? | `ruta` · `hash` · `nombre` · `tamano_prefijo` · `tamano` |
| `clase` | string? | `voice` / `video` si la app lo publica |

### 2.2 Estados

| Token | Español | Qué significa | Qué hacer |
|---|---|---|---|
| `ok` | Ok | Emparejada y verificada | — |
| `missing` | Ausente | El fichero existe pero **no tiene audio** (vacío o ilegible) | revisar permisos / fichero truncado |
| `corrupt` | Corrupto | El hash **en disco no cuadra** con el declarado | el fichero cambió: re-bajar o re-grabar |
| `orphan` | Huérfano | Fichero en disco que **ninguna** app declara | indexar a mano o moverlo |
| `no_audio` | SinAudio | La app la declara pero **no hay fichero** que corresponda | la lista miente o el audio se perdió |
| `duplicate` | Duplicado | El mismo SHA-256 está en **dos sitios** | se puede borrar la copia (ahorra `wasted_bytes`) |

Prioridad cuando una grabación cumple varias cosas: `corrupt` > `missing` >
`duplicate` > `orphan`. `no_audio` es un estado *declarado*, no de disco.

### 2.3 Informe (`Informe`)

| JSON | Significado |
|---|---|
| `total` | entradas del índice unificado (declaradas + huérfanas) |
| `ok` | verificadas y sin duplicar |
| `ausentes` | ficheros sin audio utilizable |
| `corruptos` | hash en disco ≠ hash declarado |
| `huerfanos` | ficheros que ninguna app declara |
| `duplicados` | número de **ficheros** cuyo contenido está en más de un sitio |
| `sin_audio` | declaradas sin fichero en disco |
| `bytes_totales` | bytes ocupados por las grabaciones indexadas |
| `wasted_bytes` | bytes que ocupan las **copias sobrantes** (se conserva una) |

`wasted_bytes` es un dato accionable: es el espacio que se recupera borrando
duplicados.

### 2.4 Grupos de duplicados

`grupos_duplicados()` devuelve un `GrupoDuplicado` por hash repetido:

```json
{
  "sha256": "9f2c…",
  "ficheros": ["/sdcard/A/x.amr", "/sdcard/B/x.amr"],
  "bytes": 48213,
  "wasted_bytes": 48213
}
```

La **clave de agrupación es el hash**: dos ficheros con el mismo contenido
están en el mismo grupo aunque se llamen distinto y vivan en carpetas
distintas.

---

## 3. Reconciliación: cómo se empareja cada grabador

### 3.1 Las cuatro estrategias (de la más fiable a la menos)

| # | Estrategia | Cuándo se usa | Fiabilidad |
|---|---|---|---|
| 1 | **Ruta exacta** | la entrada trae `path` y ese fichero está | alta (pero frágil: el nombre puede haber cambiado) |
| 2 | **SHA-256** | la entrada publica `sha256` | **máxima**: identidad del contenido |
| 3 | **Nombre normalizado** | sólo hay `name` | media: depende de que el grabador normalice igual |
| 4 | **Tamaño + primeros N bytes** | la app publica `head_hex` + `bytes` | baja: sólo distingue, no identifica |
| 4' | **Tamaño único** | último recurso | **débil**: se marca `emparejado_por: "tamano"` |

### 3.2 Por qué el hash es la clave fiable

* El **nombre es volátil**: depende del firmware, de la versión de la app y
  de dónde el sistema mueva el fichero. Un `.amr` puede llamarse
  `20240101_103000.amr` hoy y `Recording_2024-01-01_10-30-00.amr` mañana.
* El **nombre no es único**: dos grabadores pueden usar la misma plantilla
  (`<numero>_<timestamp>`) y el mismo número de teléfono produce colisiones.
* El **hash es el contenido**: si el hash coincide, los ficheros son el mismo
  audio, estén donde estén y se llamen como se llamen.
* Por eso el hash va **primero**, y por eso el backup **deduplica por hash**:
  una copia repetida en otra carpeta **no** se vuelve a copiar (es lo que
  hace el backup incremental).

El nombre normalizado (estrategia 3) quita: la extensión, las fechas
(`2024-01-01`, `20240101`), las horas (`10-30-00`), el número de teléfono y
todos los separadores; deja minúsculas y letras/dígitos cortos. Ejemplo:

```
Call_2024-01-01_10-30-00_+34600123456.wav  →  "call"
grabacion_20240101.amr                     →  "grabacion"
```

### 3.3 Formatos de lista que se aceptan

`unificar()` recibe `&[Value]`: una entrada por grabador. Cada una puede ser:

```jsonc
{"app": "com.whatsapp", "recordings": [ { "path": …, "sha256": …, "bytes": … } ]}
```

y también, sin transformar:

* `[{ "path": …, "sha256": … }]` (entradas sueltas),
* `{"recordings": [ … ]}` / `{"items": […]}` / `{"files": […]}` / `{"entries": […]}`,
* `{"apps": [ … ]}`.

Los nombres de campo admitidos por entrada: `path|file|file_path|filepath|
audio_path|local_path|location|uri`, `name|file_name|filename|display_name`,
`bytes|size|size_bytes|length|filesize`, `sha256|sha_256|hash|sha|checksum|
digest`, `duration_s|duration|duration_sec|duration_seconds|length_s|seconds`,
`mtime|modified_unix|modified|timestamp|time|date`, `type|kind|format|ext|
extension|mime`, `app|package|source|recorder|app_id`.

El formato que produce `call_recorder::list_recordings` (`path`, `name`,
`ext`, `bytes`, `sha256`, `modified_unix`) entra **tal cual**, sin tocar.

### 3.4 App ↔ fuente

`fuente` se deduce de la app con el mismo mapa de paquetes que usa
`call_recorder::detect_source`, más la grabadora nativa de Samsung (que
graba llamadas del **sistema**):

| App contiene | `fuente` |
|---|---|
| `whatsapp` | `whatsapp` |
| `telegram`, `ngram` | `telegram` |
| `signal`, `securesms`, `molly` | `signal` |
| `discord` | `discord` |
| `samsung`, `voicerecorder`, `soundrecord`, `recorder`, `telecom`, `android`, `system` | `system` |
| resto | `unknown:<paquete>` |

---

## 4. Rutas en el móvil

| Qué | Dónde |
|---|---|
| Raíz de audio a indexar | `/sdcard/MegaDaemon/recordings` (o la que se pase con `--root`) |
| Listas de los grabadores | las publica cada app; el daemon las recoge y las pasa a `unificar()` |
| Área de backup (por defecto) | `<state_dir>/backups/recordings/` |
| Carga del backup | `<state_dir>/backups/recordings/<AAAA-MM-DD>/<app>/<fichero>` |
| MANIFEST | `<state_dir>/backups/recordings/MANIFEST.ndjson` |
| Resumen | `<state_dir>/backups/recordings/manifest.json` |

---

## 5. Backup de seguridad

### 5.1 Procedimiento

```text
1. Indexar la raíz de audio (todo lo que sea audio).
2. Leer el MANIFEST existente  →  conjunto de SHA-256 ya respaldados.
3. Para cada grabación:
     a. calcular el SHA-256 REAL del fichero en disco;
     b. si ese hash ya está en el MANIFEST → saltar (ya está respaldado,
        aunque haya cambiado de nombre o de carpeta);
     c. si no → copiar a <dest>/<día>/<app>/<fichero>;
     d. releer la copia y comparar el hash  →  si no cuadra, NO entra en el
        MANIFEST y se reporta como fallo;
     e. añadir una línea NDJSON al MANIFEST (append-only).
4. Reescribir manifest.json con el total y el hash del MANIFEST.
```

* **Incremental**: la comparación es por **SHA-256**, no por ruta ⇒ mover o
  renombrar un fichero **no** provoca una copia nueva.
* **Idempotente**: ejecutar dos veces seguidas ⇒ la segunda no copia nada y el
  MANIFEST queda **byte a byte idéntico**. Por eso las líneas **no** llevan
  marca de tiempo: sólo la fecha del original (`mtime`), que es estable.
* **Verificado**: nunca se da por buena una copia sin releerla.

### 5.2 Formato del MANIFEST

`MANIFEST.ndjson`: una línea JSON por fichero respaldado, **append-only**:

```json
{"path":"2026-11-20/com.whatsapp/llamada.wav","sha256":"9f2c…","bytes":48213,"app":"com.whatsapp","source":"/sdcard/WhatsApp/Media/llamada.wav","mtime":1763625600,"sha256_declarado":"9f2c…","estado":"ok"}
```

| Campo | Significado |
|---|---|
| `path` | ruta **relativa** dentro del destino |
| `sha256` | hash **del contenido respaldado** (el de disco) |
| `bytes` | tamaño |
| `app` | app de origen |
| `source` | ruta original en el móvil |
| `mtime` | fecha del original (epoch, segundos) |
| `sha256_declarado` | hash que declaraba la app, si **no** cuadra |
| `estado` | estado en el momento del backup (`ok`, `corrupt`, `orphan`…) |

`manifest.json`: el resumen del MANIFEST.

```json
{
  "version": 1,
  "manifest_file": "MANIFEST.ndjson",
  "total_files": 128,
  "total_bytes": 41200344,
  "manifest_sha256": "…"
}
```

`manifest_sha256` es el hash **del NDJSON**: si cambia una sola coma, cambia
el hash ⇒ sirve de sello de integridad del propio MANIFEST.

### 5.3 Verificación

`verificar(destino)` relee **todo** lo que dice el MANIFEST y recalcula el
hash de cada fichero:

| Resultado | Significado |
|---|---|
| `ok` | presente y con el hash correcto |
| `faltan` | el fichero no está en el destino |
| `corruptos` | el hash no cuadra (o no se puede leer) |

### 5.4 Destino en Drive (gancho preparado)

El destino puede ser local o Drive. `drive_client` **no habla HTTP**: produce
la petición exacta y el comando `curl` que la ejecuta. Por eso el módulo
prepara el **plan** y quien tiene el token lo ejecuta:

```rust
let plan = recordings::plan_drive(&grabaciones, "FOLDER_ID", 8 * 1024 * 1024);
```

Cada item trae los trozos con su `Content-Range` (`bytes 0-8388607/…`),
calculados con `drive_client::chunk_plan` (múltiplos de 256 KiB, como exige
Google). La ejecución es:

1. `drive_client::open_upload_session(creds, nombre, tamaño, target)` ⇒ `POST`
   y se guarda el `Location:` de la sesión.
2. Por cada trozo: `drive_client::upload_chunk_request(creds, uri, rango, total)`
   ⇒ `PUT` con `Content-Range`.
3. `drive_client::verify_file_sha256` sobre lo descargado.

El token **nunca** está en el repo: lo recibe el llamante del almacén de
secretos.

---

## 6. CLI (`md-recordings`)

```text
md-recordings scan   --root <dir> [--app <json|dir>]... [--json]
md-recordings backup --root <dir> [--dest <dir>] [--app <json|dir>]... [--json]
md-recordings verify --dest <dir> [--json]
```

| Opción | Qué hace |
|---|---|
| `--root <dir>` | raíz de audio a indexar |
| `--app <json\|dir>` | lista de un grabador: fichero JSON o directorio (se inventaría como `call_recorder::list_recordings`). Repetible |
| `--dest <dir>` | raíz del área de backup (por defecto `<state_dir>/backups/recordings`) |
| `--state-dir <dir>` | directorio de estado (por defecto `$MEGADAEMON_STATE_DIR` o `./state`) |
| `--json` | salida en JSON |

Códigos de salida: **0** sin problemas · **1** con ausentes / sin audio /
corruptos (o si falló el backup o la verificación) · **2** uso incorrecto.

> Los huérfanos y los duplicados **no** son error: son datos. Salen en el
> informe y en el JSON, pero no cambian el código de salida.

Ejemplo (sandbox, sin móvil):

```bash
md-recordings scan   --root /tmp/audio --app /tmp/lista_whatsapp.json --json
md-recordings backup --root /tmp/audio --dest /tmp/backup
md-recordings verify --dest /tmp/backup
```

---

## 7. Enganche a la GUI (diseño, no implementado)

La idea es que la GUI sea **trivial**: un endpoint que devuelve el informe y
otro que lanza el backup. Nada de lógica en el cliente.

### 7.1 `GET /api/recordings`

Devuelve **exactamente** lo que produce `recordings::reporte_json()`:

```jsonc
{
  "raiz": "/sdcard/MegaDaemon/recordings",
  "informe": { "total": 128, "ok": 120, "ausentes": 1, "corruptos": 2,
               "huerfanos": 3, "duplicados": 2, "sin_audio": 0,
               "bytes_totales": 41200344, "wasted_bytes": 48213 },
  "grabaciones": [ { "id": "…", "ruta": "…", "estado": "corrupt", … } ],
  "duplicados": [ { "sha256": "9f2c…", "ficheros": [ … ], "bytes": 48213,
                    "wasted_bytes": 48213 } ]
}
```

Lo que necesita la vista:

* una fila por grabación con `estado` (pintado por color: rojo `corrupt`,
  ámbar `no_audio`/`missing`, gris `orphan`, azul `duplicate`);
* el contador de `wasted_bytes` con un botón **"limpiar duplicados"** (la
  acción sería borrar la copia sobrante; **no** la implementa este módulo);
* el detalle de un corrupto: `sha256` declarado vs `sha256_disco` (ya están
  en el JSON).

### 7.2 `POST /api/recordings/backup`

```jsonc
// petición
{ "dest": null, "solo_ok": false }
// respuesta
{ "destino": "/data/…/backups/recordings", "copiados": 12, "saltados": 116,
  "verificados": 12, "duplicados_omitidos": 2,
  "bytes_copiados": 1048576, "total_manifest": 128,
  "manifest_sha256": "…", "fallos": [] }
```

Es `respaldar()` tal cual: incremental, idempotente y verificado. La GUI
puede llamarlo cada noche sin miedo: la segunda vez no copia nada.

### 7.3 `GET /api/recordings/verify`

Devuelve `verificar(destino)`: `ok` / `faltan` / `corruptos` + el detalle.

### 7.4 Dónde tocar en `cockpit_server.rs`

Tres rutas más un `pub mod recordings;` en `lib.rs`. El resto (leer la lista
de cada app en el móvil) es del dueño del daemon: este módulo sólo necesita
que le pasen `&[Value]`.

---

## 8. Verificación y límites

**Verificado** (tests deterministas, sin red, sin dormir, en
`rust-core/src/recordings.rs`):

| Test | Qué comprueba |
|---|---|
| `entrada_declarada_sin_fichero_es_sin_audio` | lista con 1 entrada sin fichero ⇒ `no_audio` |
| `fichero_sin_entrada_es_huerfano` | fichero en disco sin entrada ⇒ `orphan` |
| `contenido_distinto_del_hash_declarado_es_corrupto` | hash registrado ≠ hash en disco ⇒ `corrupt` |
| `dos_ficheros_iguales_son_duplicados` | mismo contenido ⇒ `duplicate` y `wasted_bytes` correcto |
| `backup_es_idempotente` | 2 ejecuciones ⇒ la 2ª no copia y el MANIFEST es idéntico |
| `backup_verificado_cuadra_con_el_destino` | el MANIFEST cuadra con el destino (y detecta un fichero estropeado) |
| `emparejamiento_por_hash_ignora_el_nombre` | el hash empareja aunque el nombre sea completamente distinto |
| `normalizar_nombre_quita_fecha_hora_y_telefono` | la normalización quita fecha/hora/teléfono |
| `el_hash_manda_sobre_el_nombre` | con el mismo nombre y distinto contenido, manda el hash |
| `plan_drive_usa_trozos_con_content_range` | los trozos de Drive llevan `Content-Range` |

**Límites conocidos**:

* La subida real a Drive **no** está implementada: queda el plan (§5.4). Es
  coherente con el diseño de `drive_client` (que tampoco habla HTTP).
* El emparejamiento por **nombre normalizado** depende de que el grabador no
  cambie el esquema de nombres; por eso el hash va antes.
* La estrategia `tamano` (último recurso) **puede** emparejar dos grabaciones
  distintas del mismo tamaño; por eso el informe marca `emparejado_por` y la
  GUI puede scolorear esas filas.
* La lista de apps la tiene que **recoger** el daemon (este módulo no habla
  con las apps).

**Reutilización de `call_recorder.rs`**: `sha256_hex`, `sha256_file`,
`now_iso8601`, `civil_from_epoch` y la lista de extensiones están **copiados
y comentados** en `recordings.rs`, porque `call_recorder` todavía **no está
registrado en `lib.rs`** (importarlo haría que este módulo no compilase
hasta que alguien lo cablee). Cuando `pub mod call_recorder;` entre en
`lib.rs`, esas cuatro funciones se pueden borrar y usar las del módulo
original: mismo nombre, misma firma.
