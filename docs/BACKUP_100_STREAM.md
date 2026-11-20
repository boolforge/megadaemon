# Plan maestro: backup 100 % en stream a Google Drive

> Pedido del usuario (2026-11-20): *"prepárame ya la backup en stream a saco del 100 % de todas
> particiones etc"* + *"hay que hacer backup de esas cosas y contraseñas y cookies y de todo:
> bookmarks, páginas abiertas etc. Pero eso hay que implementarlo apropiadamente en MegaDaemon"*.
> Este documento es la especificación. Todo lo que aquí se afirma está **medido en el dispositivo**.

## 0. Decisión de diseño: **nativo Rust, no rclone**

`rust-core/src/backup_vault.rs` (2.011 líneas) **ya implementa el cliente de Drive nativo**:

| pieza | estado | dónde |
|---|---|---|
| subida reanudable (session URI, `uploadType=resumable`) | ✅ existe | `backup_vault.rs:987-1050` (`resumable`, `drive_resumable_command`) |
| umbral de reanudable (8 MiB) | ✅ existe | `backup_vault.rs:1825` (tests) |
| parsing de `Location:` y `upload_id` | ✅ existe | `backup_vault.rs:1833-1851` (tests) |
| `backup_full` (4 niveles: volátil, ROM, datos de usuario, credenciales cifradas) | ✅ existe | `backup_full.rs` (1.678 líneas) |
| vault de credenciales cifradas | ✅ existe | `backup_vault.rs` + `signing_vault.rs` (707 líneas, Rust puro sin JVM) |

**Por qué nativo y no rclone**:
1. **Cero dependencia de binario externo**: rclone son 32 MB + config propia + credenciales
   duplicadas + un segundo pathway de secretos. El daemon ya tiene `api_key.rs` (registro
   multi-clave revocable por agente) y `signing_vault.rs`.
2. **Un solo lugar para los secretos**: el token de Google debe vivir en el vault cifrado del
   daemon, no en un `rclone.conf` de 0600 en otra ruta.
3. **Streaming real sin staging**: el daemon puede hacer `dd | stream` sobre una partición y
   `read()` incremental sobre un fichero, sin escribir nunca una copia local (el móvil tiene
   **1,9 GB libres**; las particiones de sistema son 6,4 GB).
4. **Verificación**: `sha2` ya está en las dependencias (`Cargo.toml`) → manifiesto SHA-256 por
   fichero; Drive además da MD5 por fichero en `files.list` → **doble verificación**.

rclone queda como **opción externa opcional** (si el daemon detecta `rclone` en PATH puede
delegar), pero **no es el camino por defecto**.

## 1. Estado real del almacenamiento (medido)

```
Filesystem        Size  Used  Avail  Use%  Montado
/dev/block/sda34  225G  223G  1.9G   100%  /data          ← el problema
/dev/block/dm-0   7.8G  7.1G  641M   92%  /            (system)
/dev/block/dm-1   1.6G  1.5G   97M   95%  /vendor
/dev/block/dm-2   0.9G  977M   23M   98%  /product
/dev/block/sda29  1.3G  1.0G  325M   77%  /prism
/dev/block/sda30   29M  5.8M   23M   21%  /optics
/dev/block/sda31  575M  216K  574M    1%  /cache
/dev/block/sda1    16M  2.3M   13M   16%  /mnt/vendor/efs
/dev/block/sda2    16M  5.2M   10M   34%  /efs
/dev/block/sda21   50M   34M   16M   69%  /keyrefuge
```

Desglose de `/data` (en KiB, medido con `du -sk`):

| ruta | tamaño | contenido | política de backup |
|---|---|---|---|
| `/data/app` | **88,5 GB** | APKs instaladas (1.719 apps) | backup + **recreable desde Play** |
| `/data/media/0` | **85,9 GB** | datos del usuario | **backup íntegro (prioridad 1)** |
| `/data/user` + `/data/data` | **47,8 GB** | datos de apps | backup (prioridad 2) |
| `/data/adb` | 1,5 GB | Magisk: módulos, DB, keystore | **backup cifrado (prioridad 0)** |
| `/data/local` | 1,2 GB | tmp, scripts, megaldaemon | selectivo |
| `/data/per_boot` | 758 MB | estado por boot | volátil |
| `/data/system` | 589 MB | ajustes, paquetes | sí |
| `/data/apex` | 345 MB | APEX ya montados | recreable |

Detalle de `/storage/emulated/0` (KB): `Download` **44.361.142**, `Android` **19.588.355**,
`DCIM` **18.044.564**, `iLovePDF` **812.757**, `Pictures` **314.532**, `Movies` **49.314**,
`logcat.txt` **36.556**, `Music` **22.359**, `Documents` **18.164**, `Aliucord` **15.649**,
`RouterKeygen.dic` **64.292**.

## 2. Qué se respalda (100 %, sin excepciones)

### Nivel 0 — secretos y credenciales (**prioridad absoluta**)
- **Keyring del sistema**: `/data/misc/keystore`, `/data/misc/keystore/*`, `keystore2` DB.
- **Credenciales del daemon**: `state/megadaemon_api.keys`, `state/peers.json` (con clave del par),
  `state/cockpit_token`, `state/agents/*.json`.
- **Tokens/cookies de apps**: `harvest_sessions` (cookies en claro de WebView + tokens de
  DeepSeek, apps de IA), `app_intel` (perfiles de identidad), `signing_vault` (material criptográfico).
- **Bookmarks y passkeys**: `bookmarks` (inventory/export/passkeys/manager_export).
- **Pestañas abiertas**: estado de navegadores (Chrome/Brave/Firefox) — `tabs` + `sessions`.
- **Conversaciones**: `sync_conversations` / `sync_web_conversations` (historial en nube real).
- ⇒ Todo eso se cifra con la clave del vault (`backup_vault`) antes de subir. **Nunca en claro,
  nunca en el journal, nunca en un NDJSON de estado.**

### Nivel 1 — datos del usuario
`/data/media/0` completo (DCIM, Download, Android, Pictures, Movies, Music, Documents,
iLovePDF, Aliucord, RouterKeygen.dic, logcat.txt) + `/data/user/0` (preferencias, bases de datos
de apps).

### Nivel 2 — estado del sistema
`/data/app` (los APK instalados, para poder reinstalar exactamente lo mismo),
`/data/system`, `/data/adb` (módulos Magisk), `/data/local`, `/data/per_boot`.

### Nivel 3 — particiones (imágenes binarias)
Stream con `dd` **sin staging**:

| partición | contenido |
|---|---|
| `super` | system + vendor + product + odm (metadata dinámico) |
| `system`, `vendor`, `product`, `odm`, `prism`, `optics` | montadas en `/` |
| `boot`, `init_boot`, `vendor_boot` | arranque |
| `vbmeta`, `vbmeta_system`, `vbmeta_vendor` | AVB |
| `efs`, `keyrefuge` | **calibración, IMEI, claves** (sensibilísimo → cifrado) |
| `metadata` | estado de fabrica |

### Nivel 4 —EEPROM/calibración
`/dev/block/by-name/*` (listado completo del bloque de particiones), `nv_data`, `persist`.

## 3. Mecánica: stream, no copia

```
[movil]                                    [Drive]
 /data/media/0/DCIM  ─┐
 /data/media/0/Download ─┼─► stream ──► rclone/Drive (resumable, 8-16 MiB chunks)
 particiones (dd | )   ─┘              + md5 (Drive) + sha256 (manifiesto local→subido)
```

- **Nunca** se materializa una copia local: el móvil tiene 1,9 GB libres y el corpus son 223 GB.
- Reanudable: si se corta (la red del móvil es móvil), se retoma por `upload_id`.
- Un **manifiesto** `BACKUP_MANIFEST.json` + `SHA256SUMS.txt` sube **primero** (pequeño) y luego
  los datos; el manifiesto se actualiza al final con los hashes.
- Verificación en dos niveles: MD5 que devuelve Drive por fichero + SHA-256 calculado en origen.

## 4. Antes de subir: liberar espacio **sin borrar nada importante**

El usuario pide "dejar bastante espacio libre sin borrar cosas importantes". Estrategia:

1. **Subir primero, borrar después** (nunca al revés).
2. Cachés regenerables → se limpian **después** de verificar el backup:
   `/data/app/*/cache`, `/data/user/0/*/cache`, `~/.cache`, `logcat.txt` (36 MB), `/data/local/tmp`.
3. **Downloads (44 GB)**: es lo que más espacio consume y casi todo es re-descargable. Se sube
   primero; después el usuario puede vaciarlo si lo quiere (su decisión, no la nuestra).
4. **APKs de `/data/app` (88 GB)**: son instalables de nuevo; se sube el APK de cada app (lo que
   ya hace `apk_inventory`) y luego se puede limpiar la caché de instalador
   (`/data/app/.cache`).
5. **Nada de esto se borra automáticamente**: el daemon sólo **informa** de lo que recuperaría.

## 5. Lo que falta para ejecutarlo

| # | falta | cómo se resuelve |
|---|---|---|
| 1 | **refresh token de Google con `client_secret`** | el token guardado es de una *app web*; el `client_secret` está en la Cloud Console del usuario. Alternativa: `rclone config` (OAuth en el navegador) → `client_id`+`secret`+`refresh_token` para el vault |
| 2 | **DNS roto en el móvil** (ver `docs/DNS_DIAG_20261120.md`) | afecta a toda descarga desde el móvil; se arregla en la app VPN o con DNS por interfaz |
| 3 | cliente de Drive ampliable a "carpeta + 100 %" | extender `backup_vault.rs` con `folder_id`, `copy` recursivo y streaming de particiones |

## 6. Requisitos de credenciales en el móvil

El backup de credenciales/cookies/bookmarks/tabs **no se puede hacer desde el sandbox**: el
keyring, las cookies y las sesiones están en el sandbox de la app y en el TEE. ⇒ El agente que
ejecuta el backup **debe ser el del dispositivo** (`root_exec`/`fs_read` del móvil), que es
justo donde vive `backup_full`/`backup_vault`.

**Conclusión operativa**: el `backup_full` nativo corre en el móvil y sube directo a Drive; el
nodo externo (`cloud-sandbox`) orquesta, verifica (hashes) y deja el registro en el repo.
