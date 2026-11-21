# Catálogo de formatos de backup — qué reimplementar en MegaDaemon

> Objetivo: **reimplementar ideas y formatos**, nunca código con licencia
> incompatible. Este catálogo enumera los proyectos de backup que nos interesan
> (libres **y** no libres), su formato, su licencia y el **riesgo legal**
> concreto de tocar cada uno.
>
> Fecha: 2026-11-20 · Nodo: `dex` (General 659db26c) · Medición en el
> Samsung S21 Ultra real (Swift Backup 16.0.2 instalada).

## 0. Regla de oro

| Licencia | Qué se puede hacer dentro de MegaDaemon |
|---|---|
| MIT / BSD-2 / BSD-3 / Apache-2.0 / ISC | **Copiar código y formato** (con atribución en el `LICENSE`/NOTICE) |
| GPL-2.0 / GPL-3.0 | **Sólo ideas y formatos**. El código sólo si el proyecto entero pasa a GPL-compatible (no es el caso) |
| AGPL-3.0 (Neo Backup) | **Sólo ideas y formatos**, igual que GPL pero con la obligación de red ⇒ nunca embeber su código en un daemon |
| Propietario / freemium (Swift Backup, Titanium) | **Sólo lectura e interoperabilidad**: nunca copiar código ni *assets*; implementar el lector desde el **formato documentado por observación** (cabeceras, bytes) es interop lawful |

---

## 1. Catálogo

### 1.1 Swift Backup — `org.swiftapps.swiftbackup` (el que nos han encargado)

| Dato | Valor (medido) |
|---|---|
| Licencia | **Propietario / freemium** (SwiftApps). Licencia verificada con el Play Store y el catálogo de alternativas |
| Versión medida | 16.0.2 (153.7.0esr) · APK sha256 `f73db4caacdb273f0b68341969cf0b64d192acb7ee01133f58e28d11715d9099` |
| Cifrado | **AEGIS-256** (+ AEGIS-128X2) · constantes: `AEGIS-256`, `Aegis256`, `SBA1-AEGIS256-key-check-v1`, `SBA AEGIS encryption backend is not available` |
| Compresión | **Zstandard** (`ZSTD`, `Native zstd compress bytes failed`, `Invalid zstd compression level`) |
| Formatos | `.app` `.apk` `.dat` `.splits` `.extdat` `.med` `.extra` (apps) · `.fld` `.flm` `metadata.json` (carpetas) · `.cls` (llamadas) `.msg` (SMS) `.wal` (fondos) `.wfi` (Wi-Fi) |
| Layout | `/storage/emulated/0/SwiftBackup/accounts/<accountHash>/backups/apps/local/<pkg>/<backupId>/` |
| Partes por app | `APP, DATA, EXTDATA, EXPANSION, MEDIA` (journal `SMessage`) |
| Reglas por app | `appconfigs/data/Config`, `Blacklist`, `app_cached_data` |
| Nube | Google Drive, OneDrive, Dropbox, Box, Yandex, Mega, Filen, Nextcloud, WebDAV, SMB, SFTP, FTP (clases `cloud/protocols`, `cloud/connect`) |
| Caso especial | Chrome: `com.android.chrome`, `com.chrome.beta`, `com.chrome.dev`, `com.google.android.apps.chrome` |
| **Riesgo legal** | **Alto si se copia código**. Medio si sólo se implementa un **lector** del formato: es interop de facto. Bajo si sólo se documentan cabeceras y magnitudes |

**Qué reimplementar**: (a) un **lector** de `.dat`/`.med`/`.splits` (cabecera + AEGIS-256 + zstd) para *auditar* backups del propio usuario; (b) el **derivador de clave** verificado con el vector de comprobación `SBA1-AEGIS256-key-check-v1`; (c) un **catálogo** de archivos por app (`.fld`/`.flm`/`metadata.json`). **Nunca** portar sus clases.

### 1.2 Seedvault — el estándar de AOSP

| Dato | Valor |
|---|---|
| Licencia | **Apache-2.0** ([seedvault-app/seedvault](https://github.com/seedvault-app/seedvault)) |
| Formato | Backups v0/v1/v2 sobre flash USB; cifrado con **mnemónico BIP39** (⇒ frase de 12/24 palabras) |
| API interna | Usa las mismas APIs internas que `adb backup` (`android.permission.BACKUP`) |
| Herramientas | **Seednaut** (descifrar e inspeccionar v2), **seedvault-extractor** (v1), **seedvault-parser** (v0, re-cifra) |
| **Riesgo legal** | **Bajo**: Apache-2.0. Copiar código con atribución |

**Qué reimplementar**: **el lector Seednaut es el mejor modelo** de lo que hay que hacer (verificar cabecera, descifrar con la frase, listar). our `docs/` puede documentar el layout v2 (el que usa el propio AOSP).

### 1.3 Neo Backup / OAndBackupX (y `oandbackup`)

| Dato | Valor |
|---|---|
| Licencia | **AGPL-3.0** (Neo Backup; fork de OAndBackup) · `oandbackup` (F-Droid) **GPL** |
| Formato | Un `.tar` por app + `ab`-style: `<pkg>.tar` con las partes `app`, `dat`, `ext`, `media` |
| **Riesgo legal** | **Medio-alto**: AGPL ⇒ obligación de ofrecer el código completo de cualquier servicio que lo use por red. ⇒ **sólo ideas y formatos**, jamás linkear ni embeber |

**Qué reimplementar**: el **modelo de partes por app** (`app/dat/ext/media`) y el **formato tar por paquete**, que es el más interoperable de todos.

### 1.4 Titanium Backup (`com titanium` / `ti.android.backup`)

| Dato | Valor |
|---|---|
| Licencia | **Propietario** (freemium), **discontinuado** |
| Formato | `.tib` = tar + cifrado (AES) + índice; incluye `.apk` + datos |
| **Riesgo legal** | **Alto** si se toca; **bajo** como referencia de diseño histórico |

**Qué reimplementar**: nada. **Documentar** el concepto de "un tar por backup con índice" (que es lo que ya hace Neo Backup).

### 1.5 `adb backup` / Android BackupManager (AOSP)

| Dato | Valor |
|---|---|
| Licencia | **Apache-2.0** (AOSP) |
| Formato | `.ab` = **tar** con `manifest` + `fs`/`data`; cifrado opcional con `BackupPassword`: **`none`** o **AES-256-CBC** (PBKDF2-HMAC-SHA1, 10 000 iteraciones, IV + HMAC-SHA1) |
| Estado | **Obsoleto** (`adb backup` deprecado); Android 12+ lo restringe a `adb backup` local |
| **Riesgo legal** | **Bajo** (Apache-2.0) |

**Qué reimplementar**: **el lector de `.ab`** (ya existe en AOSP: `BackupManagerService`) ⇒ es el formato **más fácil de soportar** y da interoperabilidad con cualquier herramienta legacy. Es el candidato nº1 para el módulo `backup_full.rs` del fleet.

### 1.6 restic

| Dato | Valor |
|---|---|
| Licencia | **BSD-2-Clause** ([restic/restic](https://github.com/restic/restic)) |
| Formato | Repositorio de **chunks** con content-addressing; cifrado **AES-256-CTR + Poly1305-MAC**, clave maestra derivada con **scrypt** (N=32768, r=8, p=1) de la contraseña; **solicitud de key-derivation** (`restic key add`) |
| **Riesgo legal** | **Bajo** |

**Qué reimplementar**: el **esquema de derivación por "solicitud de clave"** (nunca guardar la contraseña) y el **formato de repositorio** si queremos que MegaDaemon sea compatible con restic.

### 1.7 Borg

| Dato | Valor |
|---|---|
| Licencia | **BSD-3-Clause** ([borgbackup/borg](https://github.com/borgbackup/borg)) |
| Formato | Segmentos con **cifrado autenticado** (BorgCrypto: AES-CTR + MAC) y **AEAD** en versiones nuevas; metadatos en msgpack + zlib |
| **Riesgo legal** | **Bajo** |

**Qué reimplementar**: el concepto de **segmentos con metadatos separados** (permite listar sin descifrar todo).

### 1.8 Kopia

| Dato | Valor |
|---|---|
| Licencia | **Apache-2.0** ([kopia/kopia](https://github.com/kopia/kopia)) |
| Formato | Repositorio **content-addressable** con **políticas** (retención, periodicidad) y **cifrado en cliente** (AES-256-GCM + scrypt para la contraseña) |
| **Riesgo legal** | **Bajo** |

**Qué reimplementar**: el modelo de **política de retención** (cuántas versiones, hourly/daily/monthly) ⇒ es lo que le falta a un backup de móvil.

### 1.9 Duplicati

| Dato | Valor |
|---|---|
| Licencia | **MIT** (LICENSE de `master` en [duplicati/duplicati](https://github.com/duplicati/duplicati)) |
| Formato | Cifrado local **AES-256**; índice `.dlist` y datos `.dblock`; manifiesto con hashes |
| **Riesgo legal** | **Bajo** |

**Qué reimplementar**: el **manifiesto separado del cuerpo** (permite verificar la integridad sin leer todo).

### 1.10 rustic (el "restic en Rust")

| Dato | Valor |
|---|---|
| Licencia | **Apache-2.0** ([rustic-rust/rustic](https://github.com/rustic-rust/rustic)) |
| Formato | **Compatible con restic** |
| **Riesgo legal** | **Bajo** y **el más relevante**: es Rust ⇒ mismo lenguaje que MegaDaemon, y su formato es el de restic |

**Qué reimplementar**: **éste es el que nos interesa de verdad** ⇒ un cliente restic-compatible (o enlazar su lógica) permite que MegaDaemon **verifique** backups de cualquier máquina sin inventar otro formato. Gracias por Apache-2.0.

### 1.11 Plakar / Backrest / Duplicacy (mención)

| Proyecto | Licencia | Nota |
|---|---|---|
| Plakar | **ISC** | Formato tipo restic en Go |
| Backrest | **GPL-3.0** | UI web sobre restic |
| Duplicacy | Propietario (binario) | Formato propio con locking |

⇒ Útiles como **referencia de formato**, no como código.

---

## 2. Qué se implementa dentro de MegaDaemon (plan derivado del catálogo)

| Prioridad | Entregable | Basado en | Licencia del resultado |
|---|---|---|---|
| P0 | **Lector de `.ab`** (tar + manifest + AES-256-CBC/PBKDF2-SHA1) | AOSP `BackupManagerService` | Apache-2.0 (el nuestro) |
| P0 | **Lector de `.tar` por app** (partes app/dat/ext/media) | OAndBackup/Neo Backup | Apache-2.0 (implementación propia del *formato*) |
| P1 | **Lector Swift Backup** (`.dat`/`.med`/`.splits`, AEGIS-256 + zstd) con **KAT** `SBA1-AEGIS256-key-check-v1` | Formato medido | Apache-2.0 (nuestro) |
| P1 | **Cliente restic-compatible** (verificar/inventariar) | rustic / restic | Apache-2.0 |
| P2 | **Política de retención** y manifiesto con hashes | Kopia / Duplicati | Apache-2.0 |

**Nunca**: portar código de Swift Backup, Titanium, Neo Backup (AGPL) ni O&O.

### 2.1. Estado real (2026-11-21) — lo que ya está en el repo

| Pieza | Módulo | Estado |
|---|---|---|
| Lectores **TAR** (ustar + GNU longname + pax) y **ZIP** (stored + deflate) | `rust-core/src/backup_formats.rs` | ✅ verde |
| **AEGIS-256** (RFC 10032, vectores oficiales) | `rust-core/src/aegis.rs` | ✅ verde |
| **Argon2id** (RFC 9106 d/i/id) + API `derive_key` | `rust-core/src/argon2.rs` | ✅ verde (3 vectores de la RFC + 36/36 contra el oráculo C) |
| **ZSTD** (FFI `dlopen` a `libzstd` + fallback al binario `zstd`) | `rust-core/src/zstd.rs` | ✅ verde (fixture real congelado) |
| Contenedor **SBA1** (magic, cabecera **inspeccionada** —no congelada—, payload TAR+ZSTD) | `rust-core/src/swift_sba1.rs` | ✅ verde (14/14) |
| **Cifrado por trozos** de Swift (AEGIS-256, nonce/AAD nativos) | — | ❌ falta: exige `.dat` real |
| **Layout exacto** de la cabecera `SBA1` | — | ❌ falta: se inspecciona, no se congela (a propósito) |

⇒ **formatos externos que el repo ya sabe leer**: TAR, ZIP y el payload
TAR+ZSTD del contenedor de Swift. Lo que **no** se puede todavía es abrir un
`.dat` cifrado de punta a punta (falta la derivación nativa de nonce/AAD y el
layout de la cabecera) ⇒ **sigue haciendo falta un `.dat` real**.

## 3. Fuentes

- Seedvault (Apache-2.0): https://github.com/seedvault-app/seedvault
- Swift Backup (Play, propietario): https://play.google.com/store/apps/details?id=org.swiftapps.swiftbackup
- Catálogo de licencias de apps de backup: https://alternativeto.net/software/swift-backup/ · https://alternativeto.net/software/titanium-backup/
- Neo Backup (AGPL-3.0) / oandbackup: https://alternativeto.net/software/oandbackupx/
- restic (BSD-2): https://github.com/restic/restic
- Borg (BSD-3): https://github.com/borgbackup/borg
- Kopia (Apache-2.0): https://github.com/kopia/kopia
- Duplicati (MIT): https://github.com/duplicati/duplicati · LICENSE: https://raw.githubusercontent.com/duplicati/duplicati/master/LICENSE
- rustic (Apache-2.0): https://github.com/rustic-rust/rustic
- Tabla de licencias de herramientas de backup: https://awesome-alternatives/awesome-alternatives/blob/main/catalog/backup.md
- Módulo Xposed que documenta Swift Backup (MIT): https://github.com/Xposed-Modules-Repo/io.github.s1ddhants1.swiftbackupprem/blob/main/README.md
