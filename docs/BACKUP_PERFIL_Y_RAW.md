# BACKUP PERFIL Y RAW — segunda pieza del respaldo masivo (S21 Ultra, root)

> **Qué es esto**: la segunda pieza del respaldo masivo del Samsung S21 Ultra
> (SM-G998B, Android 14, root Magisk). La primera pieza son los APK
> (`backup_apps`); esta es **todo lo que no es un APK**: configuración,
> certificados, navegadores y varios — más la **opción B (raw de
> particiones)** y la **subida a Drive en streaming**.
>
> **Dónde vive el código**
>
> | Fichero | Qué hace |
> |---|---|
> | `rust-core/src/backup_profile.rs` | El **plan** del perfil (qué entra y por qué) + captura local + stream a Drive |
> | `rust-core/src/backup_stream.rs` | La **fontanería** de la subida: N conexiones, SHA-256 por trozo, progreso, reanudación |
> | `rust-core/src/backup_raw.rs` | Particiones, FS por superbloque y volcado **bloque a bloque** (sólo lectura) |
> | `rust-core/src/bin/md-backup-perfil.rs` | El CLI (`plan`, `capture`, `upload`, `parts`, `raw-plan`, `raw-dump`, `raw-verify`) |
> | `rust-core/tests/backup_{profile,raw,stream}_smoke.rs` | Tests deterministas: **sin red, sin móvil** |

No hay motor paralelo: el contenedor local lo hace
[`backup_engine::capture`](../rust-core/src/backup_engine.rs) (MDBK1: TAR +
zstd + manifiesto + cifrado opcional) y la subida la hace
[`drive_native`](../rust-core/src/drive_native.rs) / `google_auth` (mismo
token, misma API, Rust puro, sin `curl`).

---

## 1. Los perfiles (P0 · P1 · P2 · P3)

| Perfil | Qué entra | Tamaño en este móvil |
|---|---|---|
| **P0** | apps + datos: `shared_prefs` y `files` de cada app | **928** apps tienen `shared_prefs` |
| **P1** | **perfil**: configuración del sistema, certificados, navegadores, varios | `/data/system` = **596.582 KiB** (583 MiB) sin `blobstore` |
| **P2** | `/sdcard` | **225 GB** (sólo multimedia) |
| **P3** | **raw** de particiones (forense) | 224,81 GiB (`userdata`) + 10,94 GiB (`super`) |

```bash
md-backup-perfil plan --root / --profile P1 --json
md-backup-perfil plan --root / --profile P1 --hash --json   # con SHA-256 real
```

### Qué entra en el perfil (P1), con lo **medido** en este móvil

| Cubo | Rutas | Medido |
|---|---|---|
| `shared_prefs` | `/data/user/0/<pkg>/shared_prefs/**` | **928** apps lo tienen |
| `app_files` | `/data/user/0/<pkg>/files/**` | — |
| `system` | `/data/system/**` | **596.582 KiB** |
| `certificates` | `/data/misc/keychain/**`, `/data/misc/keystore/**`, `*.p12`/`*.cer` del usuario | `keystore/persistent.sqlite` = 2.387.968 B; `keychain/pins` = 39.938 B |
| `browser` | Chrome/Thorium/Samsung Internet/Firefox: `Bookmarks`, `Login Data`, `Cookies`, `History`, `Preferences`, `Web Data`, `Extensions/` | Chrome: `Bookmarks` 1.008 B, `Cookies` 3.506.176 B, `History` 75.268.096 B |
| `misc` | KDE Connect, notificaciones, ajustes (`/data/system/users/0/settings_*.xml`) | `org.kde.kdeconnect_tp` instalado ✓ |

### Exclusiones (cachés regenerables) y patrón configurable

Por defecto: `/cache/`, `/caches/`, `/code_cache/`, `/no_backup/`,
`/nocache/`, `/GPUCache/`, `/DawnCache/`, `/Service Worker/CacheStorage/`,
`/blob_storage/`, `/thumbnails/`, `.crdownload`, `.part`, `.tmp`.

Propias de `/data/system` (con su motivo en el plan):

| Excluido | Motivo | Medido |
|---|---|---|
| `blobstore` | copia de los blobs de las apps para la nube; los datos vivos están en `/data/user/0` | 400.082 KiB |
| `package_cache` | APK ya instalados: se regeneran al reinstalar | 81.787 KiB |
| `procstats`, `uiderrors.txt` | estadísticas: regenerables | 3.224 KiB |

Se pueden quitar con `--include-system-blobs`, y añadir patrón propio con
`--exclude-pattern`.

---

## 2. La subida que **empieza antes** de que el backup esté hecho

### 2.1 Lo que se midió contra la API real de Drive (2026-11-22)

| Experimento | Resultado medido |
|---|---|
| Dos trozos **fuera de orden** en la misma sesión | **503** `the upload offset is 524288 byte(s), which exceeds already uploaded size of 0 byte(s)` |
| Último trozo con total `*` (desconocido) | **308** para siempre: la sesión **nunca** finaliza |
| Último trozo con total conocido (`bytes a-b/TOTAL`) | **200** + el recurso (`id`, `md5Checksum`, `size`) |
| Estado de la sesión (`PUT` con `bytes */TOTAL`, cuerpo vacío) | **308** + cabecera `Range: bytes=0-N` ⇒ **reanudable** |
| Descarga del fichero subido | **idéntico** byte a byte (`igual? True`) |

**Consecuencias de diseño** (no son teorías, son mediciones):

1. Drive **exige trozos secuenciales por sesión** ⇒ la paralelización **no**
   puede ser "N trozos de la misma sesión".
2. Drive **exige el total conocido** en el último trozo ⇒ el tamaño del
   contenido tiene que saberse **antes** de subir ⇒ el contenido se
   **calcula el tamaño exacto** por adelantado (ver §2.3).

### 2.2 El diseño: N conexiones = N sesiones = N objetos

```
  productor ──► Pipeline (cola acotada = contrapresión)
                 │  trozo i → tramo (i·N/T)
                 ▼
           N conexiones (una sesión de Drive por tramo)
                 │  Content-Range: bytes off-(off+len-1)/total_del_tramo
                 ▼
           Drive  ──►  MANIFEST.json (al final)
```

* Cada **tramo** es una sesión reanudable propia ⇒ **N subidas en paralelo**
  (2-4, `--workers`).
* Dentro de un tramo los trozos van **en orden** (obligatorio, medido).
* El **manifiesto final** (`<nombre>.manifest.json`) ata los tramos:
  SHA-256 de cada fichero, SHA-256 de cada trozo, SHA-256 global y los ids
  de Drive.
* La cola es **acotada** (4 trozos por conexión) ⇒ la memoria no crece y el
  productor **se bloquea** si Drive va lento: subir mientras se crea **sin**
  cargar el backup en RAM ni en disco.

### 2.3 Cómo se sabe el total antes de subir

* **Perfil**: el contenido es un **TAR**, y el tamaño de un TAR se sabe
  exactamente: `512` por entrada + contenido alineado a 512 + `1024` de fin
  (`backup_profile::tar_total_bytes`). El hash de cada fichero (que hace
  falta para el manifiesto) se calcula en la primera pasada.
* **Raw**: el tamaño es el del **dispositivo de bloque** (`stat`).

Por eso el perfil en streaming va **sin zstd** (el tamaño comprimido no se
puede saber de antemano). El camino "bonito" (contenedor local MDBK1, con
zstd y cifrado) lo hace `md-backup-perfil capture`.

### 2.4 Progreso (lo que el usuario quiere ver)

```
[md-backup] 37.20 / 210.00 MiB (17.7 %) · 3.41 MB/s · ETA 00:00:31 · 4 trozos
```

`bytes subidos · % · MB/s · ETA · trozos`, cada 2 s, en `stderr`.

### 2.5 Reanudación

`UploadState` (JSON en `--state-dir`) guarda, por conexión, la URI de la
sesión y los trozos ya subidos. Si el daemon muere:

1. Se carga el estado.
2. Se pregunta al servidor cuánto tiene (`PUT` con `Content-Range: bytes */TOTAL`
   → 308 + `Range`).
3. Se salta lo ya subido y se sigue.

### 2.6 Uso

```bash
# Streaming real a Drive (3 conexiones, 8 MiB por trozo)
md-backup-perfil upload --root / --profile P1 --name s21-perfil-P1 \
    --folder <id-de-carpeta> --workers 3 --state-dir /data/local/tmp/megadaemon/state

# Sin red: los tramos se escriben en local (para probar)
md-backup-perfil upload --root / --profile P1 --name prueba --simulate --out-dir /tmp/x
```

---

## 3. Opción B: RAW de particiones

### 3.1 Reglas del juego

* **Sólo lectura.** Todo el módulo abre con `File::open` (modo lectura).
  **Nunca** borra, formatea ni redimensiona: el volcado *sobrescribe el
  bloque en el destino*, no el origen. (El test
  `el_volcado_nunca_escribe_en_el_origen` comprueba que tamaño y mtime del
  origen no cambian.)
* **No cabe en el móvil.** `userdata` mide **241.390.583.808 B (224,81 GiB)**
  y `/data` tiene **10 GB** libres ⇒ el destino es **Drive, por trozos**,
  nunca un fichero local. El plan **avisa antes de empezar**.
* **Verificable**: SHA-256 por bloque y SHA-256 global; `raw-verify` relee el
  origen y compara bloque a bloque.

### 3.2 Lo que se lee del dispositivo

* `/proc/partitions` ⇒ dispositivos y tamaño (bloques de 1 KiB).
* `/sys/class/block/<dev>/size` ⇒ tamaño exacto (sectores de 512 B) y `ro`.
* `/dev/block/by-name/` ⇒ los nombres lógicos de Samsung.
* `/proc/mounts` ⇒ punto de montaje y FS.
* El **superbloque** de cada partición ⇒ el FS real, leído del bloque (sin
  `blkid`, que no existe en Android): `0xEF53`@1080 (ext4),
  `0xF2F52010`@1024 (f2fs), `EXFAT`@1024, `FAT`@510, `NTFS`@3, `XFSB`@0,
  `hsqs`@0, `0xE0F5E1E2`@1064 (erofs).

### 3.3 Medido en este móvil (S21 Ultra)

Particiones reales (`/proc/partitions`): `sda` (249.892.864 bloques) con
`sda1`…`sda34`, `sdb`, `sdc`, `sdd`(+`sdd1`), `sde` y los `dm-0`…`dm-31`
(mapper de las particiones lógicas del super).

Nombres lógicos (`/dev/block/by-name`) y tamaños **medidos con
`/sys/class/block/<dev>/size`**:

| Nombre | Dispositivo | Sectores (512 B) | Bytes | Tamaño |
|---|---|---|---|---|
| `userdata` | `sda34` | 471.465.984 | 241.390.583.808 | **224,81 GiB** |
| `super` | `sda28` | 22.937.600 | 11.744.051.200 | **10,94 GiB** |
| `keyrefuge` | `sda21` | 106.496 | 54.525.952 | 52 MiB |
| `metadata` | `sda25` | 65.536 | 33.554.432 | 32 MiB |
| `efs` | `sda1` | 40.960 | 20.971.520 | 20 MiB |
| `sec_efs` | `sda2` | 40.960 | 20.971.520 | 20 MiB |
| `persistent` | `sda19` | 1.024 | 524.288 | 512 KiB |
| `keystorage` | `sda6` | 1.024 | 524.288 | 512 KiB |

Montajes medidos: `/` = `dm-0` (ext4, **ro**), `/vendor` = `dm-1` (ro),
`/product` = `dm-2` (ro), `/odm` = `dm-3` (ro), `/metadata` = `metadata`
(ext4, rw), `/prism`, `/optics`, `/omr`… (rw).

### 3.4 Uso

```bash
md-backup-perfil parts --root / --json          # lista con FS y montaje
md-backup-perfil raw-plan --root / --partition userdata   # tamaño + avisos
md-backup-perfil raw-dump --device /dev/block/by-name/userdata \
    --name s21-raw-userdata --workers 3 --state-dir <dir>
md-backup-perfil raw-verify --device /dev/block/by-name/userdata \
    --report s21-raw-userdata.raw.json
```

---

## 4. Los tests

```bash
cd rust-core
CARGO_TARGET_DIR=/tmp/md-tgt-backup CARGO_PROFILE_DEV_DEBUG=0 \
RUSTFLAGS="-C debuginfo=0" cargo test --offline -j1 \
  --test backup_profile_smoke --test backup_raw_smoke --test backup_stream_smoke
```

* **Sin red**: el destino de las pruebas es `FileSink` (ficheros locales),
  que es exactamente lo que hace `--simulate`.
* **Sin móvil**: todo se monta en un directorio temporal que imita `/data`.
* **Deterministas**: los hashes son de contenido fijo; no hay relojes ni
  aleatoriedad en las aserciones.

| Test | Qué demuestra |
|---|---|
| `el_plan_entra_lo_que_debe_y_explica_el_por_que` | El plan incluye shared_prefs/certs/navegador/KDE y **explica** cada exclusión |
| `la_lista_blanca_de_paquetes_filtra` | La lista blanca y el motivo |
| `el_exclusion_por_patron_es_configurable` | El patrón propio del usuario |
| `el_stream_genera_un_tar_reconstruible_y_verificable` | El TAR se reconstruye y cada fichero cuadra con su SHA-256 |
| `la_verificacion_del_stream_detecta_una_corrupcion` | Un byte cambiado ⇒ verificación FALLA |
| `el_flujo_se_reparte_en_n_conexiones_y_sale_entero` | El reparto en N conexiones y el hash global |
| `el_progreso_informa_de_bytes_velocidad_y_eta` | El progreso tiene bytes, MB/s y ETA |
| `el_estado_de_reanudacion_se_guarda_y_se_recupera` | El estado se guarda y se recarga (reanudación) |
| `el_volcado_por_bloques_sale_exacto_y_se_verifica` | El volcado sale exacto y la verificación detecta el cambio |
| `el_volcado_nunca_escribe_en_el_origen` | El origen no se toca (tamaño y mtime) |
| `se_listan_las_particiones_de_un_proc_partitions_real` | El parseo de `/proc/partitions` |

---

## 5. Lo que **NO** está verificado

* (ver el informe del agente: la subida real a Drive desde el **móvil** no se
  pudo hacer porque el móvil **no tiene** la credencial de Drive
  (`/data/local/tmp/megadaemon/state/gdrive_credentials.json` no existe);
  la subida real se validó contra la API desde el sandbox, no desde el
  dispositivo).
* El **compilado ARM64** del binario para el móvil: el sandbox tiene 3 GB de
  RAM y la compilación completa no cabe (ver §6).
