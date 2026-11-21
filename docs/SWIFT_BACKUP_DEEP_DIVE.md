# Swift Backup: inspección profunda (medido el 2026-11-21)

App: **`org.swiftapps.swiftbackup`** (Swift Backup, en F-Droid y Play).
Todo lo de este documento está **medido en el S21 Ultra** con root, salvo lo
marcado como `NO VERIFICADO` o `PENDIENTE`.

---

## 1. Lo que hay en el móvil (medido)

### 1.1 Árbol de datos

```
/data/data/org.swiftapps.swiftbackup/
├── app_org.swiftapps.swiftbackup/    (huella de la app)
├── app_sslcache/
├── cache/
│   └── messages_backup/mms_data/      (MMS: .jpeg/.gif de los mensajes)
├── code_cache/
├── databases/
│   ├── swiftbackup-db                 718.618.624 B   <-- el grande
│   ├── swiftbackup-db-shm                32.768 B
│   ├── swiftbackup-db-wal               524.288 B
│   └── com.google.android.datatransport.events
├── files/
│   ├── datastore/                     (protobuf de Firebase/AppAuth)
│   ├── diagnostics/
│   ├── favorites/                      (vacío)
│   ├── rList                           (ArrayList serializado de Java)
│   ├── generatefid.lock
│   └── profileinstaller_profileWrittenFor_lastUpdateTime.dat
├── no_backup/                         (PersistedInstallation de Firebase)
└── shared_prefs/
    ├── org.swiftapps.swiftbackup_preferences.xml   <-- el mapa de ajustes
    ├── com.google.android.gms.signin.xml
    ├── com.google.firebase.auth.api.*.xml
    └── ...
```

### 1.2 Las copias de seguridad (en sdcard)

```
/sdcard/SwiftBackup/
├── .nomedia
├── .trash/atomic-delete/              (vacío: donde van las copias borradas)
├── accounts/<id-de-cuenta>/           (id = fcf724b2ae9147c8, medido)
│   ├── backups/
│   │   ├── apps/
│   │   │   ├── local/                 (vacío en este móvil)
│   │   │   └── cloud/com.samsung.android.mtp/20261119-063712-BY/
│   │   │       ├── com.samsung.android.mtp.xml     374 B
│   │   │       └── com.samsung.android.mtp.extra   788 B
│   │   ├── sms/local/v3.1723181144556.615.3040.SM-G998B.msg    2.555.691 B
│   │   ├── calls/local/v3.1723181173927.2000.SM-G998B.cls    1.233.620 B
│   │   ├── wifi/local/wifi_networks.wfi
│   │   ├── walls/applied/home_wall.wal   7.308.588 B   (PNG)
│   │   ├── walls/applied/lock_wall.wal   6.280.337 B   (PNG)
│   │   └── folders/
│   └── cache/
└── icon_cache/                        68 MB
```

**Punto clave**: el backups del móvil es una **copia de MTP** (el respaldo
del sistema Android), no una copia "nativa" de Swift Backup. Swift Backup
guarda por un lado lo que él sabe indexar y por otro copia las piezas del
respaldo del sistema.

### 1.3 Formato de los ficheros (medido por sus cabeceras)

| Fichero | Bytes iniciales | Qué es |
|---|---|---|
| `*.msq` / `*.xml` (MTP) | `76 31 3a 3a 3a` = **`v1:::`** | **contenedor propio de Swift Backup** (§2) |
| `*.extra` (MTP) | `76 31 3a 3a 3a` = **`v1:::`** | ídem |
| `*.msg` (SMS) | `37 7a 1c bd 5f 27 1c 00 04` | **archivo 7-Zip** (LZMA2) |
| `*.cls` (llamadas) | `37 7a 1c bd 5f 27 1c 00 04` | **archivo 7-Zip** (LZMA2) |
| `*.wfi` (wifi) | `01 02 f9 6c 6a 55 09` | binario propio |
| `*.wal` (fondos) | `89 50 4e 47` | **PNG** |

⇒ **Swift Backup guarda SMS y llamadas en 7-Zip**, no en un contenedor
propio. El motor Rust tiene que hablar 7-Zip (o convertir), no inventar un
formato.

### 1.4 El esquema de `swiftbackup-db` (718 MB)

Extraído de la cabecera SQLite (páginas 1..n) con un lector propio:

```sql
CREATE TABLE SMessage (time INTEGER NOT NULL, messageType INTEGER NOT NULL,
                       title TEXT NOT NULL, msg TEXT NOT NULL, color TEXT,
                       id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL);
CREATE TABLE app_cached_data (pkgName TEXT NOT NULL, name TEXT NOT NULL,
                       isInstalled INTEGER, isEnabled INTEGER,
                       isLaunchable INTEGER, locale TEXT,
                       PRIMARY KEY(pkgName));
CREATE TABLE android_metadata (locale TEXT);
CREATE TABLE room_master_table (id INTEGER PRIMARY KEY, identity_hash TEXT);
CREATE TABLE sqlite_sequence(name,seq);
```

`SMessage` = los mensajes que la app muestra en su bandeja (notificaciones
internas de la app). `app_cached_data` = el inventario de apps instaladas.

⇒ El DB **no** contiene las copias de seguridad: es el índice de la app.

### 1.5 La nube

De `org.swiftapps.swiftbackup_preferences.xml`:

* `google_drive_cloud_main_folder_id` = **`1L-XKLJKy68Jmjf0K8uIRXOklL4gUIdO1`**
* `google_drive_cloud_email_address` presente
* `google_drive_cloud_backup_tag` presente

⇒ **las copias están en Google Drive** (cuenta `jfcarrb@gmail.com`), en esa
carpeta. El móvil es el que sincroniza; el sandbox **no tiene token OAuth de
Drive** (`PENDIENTE DE CREDENCIAL`).

### 1.6 Otros hallazgos de las preferencias

* `checked_location_1DEVICE` … `checked_location_5DEVICE`: cinco ubicaciones
  de copia (local, nube, etc.).
* `schedule_data`, `KEY_SCHEDULE_ENABLED`: **copias programadas** (la app
  puede copiar sola, sin que nadie la abra).
* `compression_level_app_data`, `s3_chunk_size`, `nextcloud_chunk_size`,
  `multithreads_downloads`: **S3 y Nextcloud** además de Google Drive.
* `messages_backup_mms`, `KEY_DEF_SMS_PACKAGE`: copia de SMS/MMS.
* `apps_multiple_backups_strategy`:策略 de varias copias por app.
* `battery_opt`, `schedule_last_run_msg_…`: optimizations de batería.

⇒ Swift Backup **no** es sólo un copiador de ficheros: es un gestor de
copias programadas multi-nube con política de retención. Ahí está la mayor
parte de su valor real.

---

## 2. El contenedor `v1:::` (nuevo, medido hoy)

```text
v1:::<base64 bloque 1>:::<base64 bloque 2>
```

Medido en `com.samsung.android.mtp.xml` (374 B) y `.extra` (788 B):

* Los bloques van en **base64 estándar** con saltos de línea de 76
  caracteres (como `Base64.encodeToString` de Java).
* **Los dos bloques empiezan por los mismos dos bytes: `01 02`.**
* A partir de ahí: **entropía alta** (cifrado o compresión; con 58 bytes el
  techo es log₂58 = 5,86 bits/byte y la muestra da 5,60 ⇒ distribución
  uniforme).
* **No** empieza por `SBA1`: es un contenedor **distinto** del formato de
  exportación (`.dat`), que ya está medido en `swift_archive.rs`.

Implementado en `rust-core/src/swift_v1.rs` (10 tests), que **parsea,
separa, mide y reporta** — **no descifra nada**, porque no se sabe todavía
qué significa `01 02` ni qué cifrado (si lo hay) se usa.

### Lo que falta para cerrarlo

| Dato | Cómo se consigue |
|---|---|
| El fichero completo (no 300 B de cabecera) | `fs_read` del móvil (ya hay script) |
| El significado de `01 02` | el APK: clases que escriben este prefijo |
| Si hay cifrado y con qué clave | el APK + la `passphrase` del usuario |
| La relación `v1:::` ↔ `SBA1` | el APK |

---

## 3. Lo que ya sabíamos del formato de exportación (`SBA1`)

Documentado en `docs/SWIFT_BACKUP_FORMAT.md`,
`docs/SWIFT_SBA1_HEADER_ARM64.md` y `docs/SWIFT_KOTLIN_MARSHALLING.md`:

* Magic `SBA1` al principio; cabecera de **96 B** (144 si `formato == 2`);
* **big-endian**;
* AEAD **por trozos** (≤ 64 KiB) con etiquetas de dominio
  `SBA1-AEGIS256-entry-chunk-nonce-v1` / `...-aad-v1`, clave de 32 B;
* **key-check**: `SHA-256(tag ‖ salt ‖ nonce2 ‖ clave)[:16]`;
* **HMAC del payload**: `SHA-256("SBA1-payload-hmac-key-v1" ‖ salt ‖ nonce2
  ‖ clave)` (32 B);
* KDF: **Argon2id** (el canónico del repo, `crate::argon2::derive_key`);
* llamada JNI real: `createArchive` con **23 argumentos** (`Ldy6;->f0`),
  modelo `Lg87;`, `method_idx` real `0x624E` = M[25150].

⇒ El AEAD del export está medido salvo el **round-trip con un fichero real**
(que exige un `.dat` de verdad).

---

## 4. Plan: el motor Rust, mejor y más completo

Objetivo: un motor en Rust puro que (a) lee y escribe **todo** lo que
Swift Backup hace, y (b) hace más cosas.

### 4.1 Lo que hay que implementar

| Pieza | Qué es | Prioridad |
|---|---|---|
| `swift_v1` | contenedor `v1:::` (parseo + escritura) | **P0** (hoy) |
| `swift_sba1` | export `.dat` (SBA1) con AEAD por trozos | P0 (hecho, falta round-trip) |
| `swift_7z` | lectura/escritura **7-Zip** (`.msg`, `.cls`) | **P0** (nuevo) |
| `swift_mtp` | formato de copia del sistema (`.xml` + `.extra` del MTP) | P1 |
| `swift_backup_manifest` | manifiesto de la copia (apps, sms, calls, wifi, walls) | P1 |
| `swift_schedule` | copias programadas (lo que hace `schedule_data`) | P1 |
| `swift_cloud` | Google Drive / S3 / Nextcloud | P2 |

### 4.2 Lo que se hará MEJOR (más completo que Swift Backup)

1. **Un solo motor, sin app**: el daemon ya está en el móvil, con root, con
   acceso a los datos de todas las apps. Swift Backup necesita que su app
   viva y que el usuario conceda permisos.
2. **Copias programadas reales**, con ventana horaria, retención y
   presupuesto de red.
3. **Cifrado por pieza**: cada app con su clave derivada; el manifiesto
   cifrado aparte. Swift Backup cifra el export completo.
4. **Sin límite de apps**: el `APK` puede ser de 50 MB o de 5 GB; el motor
   trabaja por trozos y no carga nada en RAM.
5. **Verificable**: SHA-256 por piece + manifiesto; se puede restaurar en
   otra máquina y comprobar pieza a pieza.
6. **Sincronización entre nodos** por la malla (lo que Swift Backup no
   tiene): el móvil copia, el PC verifica, otro nodo restaura.
7. **Formatos abiertos**: exportar a `.tar.zst`/`.zip`/`.7z` **y** al
   `.dat` de Swift Backup (interoperabilidad).

### 4.3 Cómo se cerrará el AEAD por trozos

1. Traer un `.dat` real (el usuario lo exporta desde la app: un toque).
2. `swift-verify` sobre él: cabecera, key-check, HMAC del payload.
3. Descifrar por trozos y comparar con el hash de cada entrada.
4. Congelar vectores de test.

---

## 5. Cómo traer los ficheros (PENDIENTE: el móvil está caído)

El móvil se quedó sin daemon el 2026-11-21 (ver `docs/RESILIENCE.md` y el
commit `19b12a6`). Cuando vuelva:

```sh
# 1. El APK (para el RE)
python3 tools/swift_extract.sh apk

# 2. Las piezas de la copia de seguridad
python3 tools/swift_extract.sh backups

# 3. El DB de la app (índices y preferencias)
python3 tools/swift_extract.sh db
```

El script usa el MCP del móvil (`root_exec` + `fs_read`), que ya funciona por
el túnel o por la LAN.

### Alternativa: Drive

La carpeta `1L-XKLJKy68Jmjf0K8uIRXOklL4gUIdO1` de `jfcarrb@gmail.com`
contiene las copias de la nube. Con un token OAuth de Drive (el que falta)
se puede bajar por la API sin tocar el móvil.

---

## 6. Lo que NO se sabe (y no se inventa)

| Punto | Por qué |
|---|---|
| Significado de `01 02` | falta el APK y el fichero completo |
| Cifrado del contenedor `v1:::` | ídem |
| Contenido exacto de `.wfi` | sólo se midieron 10 bytes |
| Si el `.extra` es el MTP "real" o una reescritura | falta el original de Android |
| Token de Google Drive | ausente en el sandbox |
| Round-trip del AEAD por trozos | no hay `.dat` real |

Nada de esto está simulado: el parser **reporta** lo que mide y marca lo
que no sabe.
