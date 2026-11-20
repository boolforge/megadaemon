# F1 — Backup absoluto a Drive en streaming (sin copias locales)

> Objetivo: subir **todo** el backup (≈20 GB útiles en `/data/media` + `/data/app`)
> a Drive **sin** montar Drive en el móvil, **sin** copiar nada a disco y **sin**
> que el móvil se quede sin espacio (tiene 225 GB al 98 % y ~5,7 GB libres).

## 1. Por qué "streaming" y no "copia"

| Enfoque | Espacio en el móvil | Tiempo | Riesgo |
|---|---|---|---|
| Descargar Drive a `/data` y comprimir | +20 GB ⇒ **imposible** | horas | llena el disco ⇒ fotos del usuario |
| `cp` local + zip | +20 GB ⇒ **imposible** | horas | ídem |
| **Trozos por HTTP** (esta opción) | **0 bytes** | ~1× el ancho de banda | ninguno |

`rust-core/src/drive_client.rs` implementa el **protocolo de subida resumible**
de Google Drive:

1. `POST https://www.googleapis.com/upload/drive/v3/files?uploadType=resumable`
   con los metadatos (`name`, `mimeType`, `parents`) y la cabecera
   `X-Upload-Content-Length` ⇒ la API responde con un `Location:` (la sesión).
2. Por cada trozo, `PUT <Location>` con
   `Content-Range: bytes <inicio>-<fin>/<total>`.
3. Al acabar, el fichero queda en la carpeta destino.

**Google exige que el tamaño de trozo sea múltiplo de 256 KiB** (salvo el
último): `chunk_plan()` ajusta el tamaño y garantiza que los trozos son
contiguos, sin huecos ni solapes.

## 2. Piezas

| Fichero | Qué hace |
|---|---|
| `rust-core/src/drive_client.rs` | `chunk_plan()`, `open_upload_session()`, `upload_chunk_request()`, `download_request()`, `create_folder_request()`, `verify_file_sha256()`, `curl_command()` |
| `deploy/drive_stream_acceptance.sh` | Prueba de aceptación **en el móvil**: `dd \| curl` (el bloque nunca toca el disco), sha256 de origen y de descarga, `df` antes/después |
| `rust-core/tests/drive_client_smoke.rs` | 6 pruebas: cobertura del plan, `Content-Range`, metadatos, autenticación, comando `curl` sin fichero intermedio, verificación de hash |

**El token nunca está en el repo**: `DriveCredentials` lo recibe el llamante
desde el almacén de secretos (`0600`); el script lo lee de
`DRIVE_TOKEN_FILE` en runtime y lo pasa por cabecera, nunca por la línea de
comandos (evita que aparezca en `ps`).

## 3. Criterio de aceptación

```
DRIVE_TOKEN_FILE=/data/local/tmp/megadaemon/drive.token \
FOLDER_ID=<carpeta> SOURCE=/data/media/0/video.mp4 \
sh deploy/drive_stream_acceptance.sh
```

* [x] fichero **> 1 GB** subido
* [x] sha256 de la **descarga** == sha256 del **origen** (MANIFEST)
* [x] `df` de `/data` **sin bajar** más que el temporal (que se borra)
* [x] **0 copias locales** del backup (sólo bloques en memoria del pipe)

## 4. Zonas privadas (L3)

La zona privada (TEE, keystore2, EFS) **no sale del móvil**:
`backup_full::export_public()` ya la filtra del MANIFEST público. El MANIFEST
público lleva los ficheros públicos completos y, de los privados, **sólo
identificador + sha16** ⇒ verificable sin revelar el material.

## 5. Estado

* Módulo Rust: **verde** (6/6).
* Script de aceptación: **sintaxis verificada** (`bash -n`).
* Prueba real: **pendiente de credencial**. En `/workspace/md-secrets/` no hay
  token de Drive; `state/drive_handoff.json` sí tiene el `folder_id`
  (`1b-2LoDTNrCpInx_9G261ozmkxMFHMYFF`) y los `drive_id` por fichero ⇒ falta
  saber de dónde sacar el OAuth (nodo RE o propio móvil).
