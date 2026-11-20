# Swift Backup (org.swiftapps.swiftbackup) — criptografía y formato del contenedor

Documento de ingeniería **medido**, no inferido. Cada afirmación lleva la
clase/método (ofuscados, R8) o el symbol/export donde se obtuvo. Lo que no
se pudo determinar está marcado explícitamente como pendiente.

> Todo el análisis se hizo **offline**, sobre el APK descargado del mirror
> `d.apkpure.net`. El **túnel al móvil estaba caído** (530) y la API key
> del MCP estaba revocada para toda la flota: no hizo falta el móvil.

## 1. Artefactos analizados

| Artefacto | Tamaño | sha256 |
|---|---|---|
| `classes.dex` | 9 273 900 B | `1db45566d56d9b26da945b1ac04fea8a3075cd6adba4727dfce27796f272a7fd` |
| `classes2.dex` | 2 844 984 B | `6bb33ae5c983c7e7e1f2f11048892dd44b7a9c0368db7d74211a07a729036844` |
| `swift-5.1.1.apk` | 15 368 344 B | `a059e010489ba501a4c9c28462f928f7c7954fd474d75611c2b994a3a59a43f2` |
| `lib/arm64-v8a/libsba_archive.so` | 1 371 184 B | — |

* App: **Swift Backup 5.1.1**, `versionCode 635`, `org.swiftapps.swiftbackup`
  (la versión instalada en el móvil es la 5.1.1 / vC 615→635 según `dumpsys`).
* El sha256 del **APK del mirror** (`a059e010…`) **no** coincide con el del APK
  instalado en el móvil (`f73db4ca…`): el mirror reempaqueta/firma. **Los DEX
  sí son los originales**: sus tamaños coinciden byte a byte con los extraídos
  del móvil, y dos descargas independientes (mía y la del principal, mismo
  mirror) dan el **mismo sha256** ⇒ artefacto determinista y comparable.
* El APK trae **32 `.so`** (4 arquitecturas) y **2 DEX**; no hay assets con
  lógica de backup (todo el motor está en `libsba_archive.so` + DEX).

### 1.1 Exportaciones nativas relevantes (`libsba_archive.so`)

```
Java_com_swiftapps_sba_SbaNativeCrypto_deriveArgon2id
Java_com_swiftapps_sba_SbaLibaegisCryptoNative_encryptAegis256ChunkedEntryFdRaw
Java_com_swiftapps_sba_SbaLibaegisCryptoNative_decryptAegis256ChunkedEntryFdRaw
Java_com_swiftapps_sba_SbaLibaegisCryptoNative_decryptAegis128X2ChunkedEntryFdRaw
Java_com_swiftapps_sba_SbaLibaegisCryptoNative_isAvailable
Java_com_swiftapps_sba_SbaSwiftTarNative_{createTarToFdWithWriteStatsRaw,extractTarFromFdRaw,
    extractTarZstdFromFdRaw,listTarEntriesFromFdRaw,listTarEntriesZstdFromFdRaw,
    extractAegisArchiveEntry,extractAegisArchiveEntryFused}
Java_com_swiftapps_sba_SbaZstdNative_{compressZstdBytes,compressZstdFdToFdRaw,...}
```

## 2. P5 — la derivación de la clave (esto es lo que pedía W3)

### 2.1 El KDF es **Argon2id**, NO PBKDF2

| Elemento | Valor medido | Evidencia |
|---|---|---|
| Clase del KDF | `Ly87;` (ofuscada) | contiene los mensajes de validación y la llamada nativa |
| Método | `Ly87;.a` | valida + deriva; `ins=6` (contraseña, salt, iterations, memoryKiB, parallelism, outputBytes) |
| Llamada real | `Lcom/swiftapps/sba/SbaNativeCrypto;->deriveArgon2id` | JNI en `libsba_archive.so` |
| Codificación de la contraseña | **UTF-8** | `Charset.newEncoder` → `CharBuffer.wrap` → `encode` → `ByteBuffer.array` |
| Salt | **16 bytes** (fijo) | `const/16 16` + `"SBA Argon2id salt must be 16 bytes"` |
| Iterations | **1..100** | bytecode compara con `101` ⇒ límite exclusivo |
| memoryKiB | **16384..65536** | bytecode compara con `16384` y `65537` |
| Parallelism | **1..8** | bytecode compara con `9` |
| outputBytes | > 0 (nativo: 16..128) | `"SBA Argon2id output bytes must be positive: "` |
| Contraseña | obligatoria, no vacía | `"SBA encryption password must not be empty"` |
| Salida para AEGIS-256 | **32 bytes** | `const/16 32` en `Lib7;.H` |
| Higiene | los bytes de la contraseña se ponen a cero tras usarlos | `Arrays.fill` (×2) |

Los mismos rangos aparecen literalmente en el binario nativo:

```
Argon2id iterations must be in 1..100
Argon2id memoryKiB must be in 16384..65536
Argon2id outputBytes must be in 16..128
Argon2id parallelism must be in 1..8
```

### 2.2 Flujo completo de descifrado (`Lib7;.H`)

1. Si el contenedor no está cifrado ⇒ `"SBA archive is not encrypted"`.
2. Deriva la clave: `Ly87;.a` (Argon2id) → 32 B.
3. Calcula el **key-check**: `Ly87;.d` (SHA-256 + *ordinal* del método de
   cifrado) y lo compara con el valor almacenado usando
   `MessageDigest.isEqual` ⇒ **la contraseña se puede verificar sin descifrar**.
4. Si no coincide ⇒ `IOException("Invalid SBA archive key")`.
   Si el archivo está cifrado y no hay clave ⇒ `"Archive key required for encrypted SBA payload"`.

### 2.3 No es una clave: es un **árbol de claves**

`Ly87;.<clinit>` inicializa **ocho etiquetas** (info-strings) distintas:

```
SBA1-7zAES-key-check-v1
SBA1-AES256GCM-key-check-v1
SBA1-AES256GCMSIV-key-check-v1
SBA1-AEGIS256-key-check-v1
SBA1-AEGIS128X2-key-check-v1
SBA1-payload-hmac-key-v1
SBA2-index-mac-key-v1
SBA2-index-metadata-mac-v1
```

⇒ una clave por uso (HMAC del payload, MAC del índice, MAC de metadatos del
índice y un key-check por método de cifrado). `Ly87;.b` es el helper
`HmacSHA256` sobre `SecretKeySpec` y `Ly87;.c`/`Ly87;.e` son los helpers
SHA-256 (con `e` devolviendo 8 bytes a partir de un índice).

### 2.4 ⚠️ Falsos amigos (no los uses)

* `Argon2-Salt`, `Argon2-Memory`, `Argon2-Passes`, `Argon2-Parallelism`,
  `Key-Derivation` ⇒ **no** son la config de Swift Backup: pertenecen a
  `com.jcraft.jsch.bc.Argon2` (BouncyCastle que va dentro de **JSch**, para
  parsear cabeceras PGP v3 `$argon2id$…`). El bundle incluye JSch por el
  cliente SFTP.
* `saltEnc`, `saltAuth` ⇒ `Lci9;` es una clase de **WebAuthn/COSE**
  (`"HmacSecretExtension{coseKeyAgreement="`), no del formato de backup.
* `Lg6;` con 1,2,4,…,131072 ⇒ clase genérica de tamaños, no de crypto.
* `Lv3a;` (constantes P-256: b, Gx, Gy) ⇒ helper de **curva elíptica**, no
  el KAT de AEGIS.
* `Aegis256(password=<redacted>)` ⇒ aparece en `com.jcraft.jsch` (KEX de JSch),
  no en Swift Backup.

## 3. AEGIS: implementación nativa y AEAD **por trozos**

* El AEGIS-256 **no está en Java**: vive en `libsba_archive.so`
  (`SbaLibaegisCryptoNative`).
* El contenedor es un **AEAD fragmentado**: cada *chunk* tiene su propio
  nonce y su propio AAD:
  ```
  SBA1-AEGIS256-entry-chunk-nonce-v1
  SBA1-AEGIS256-entry-chunk-aad-v1
  SBA1-AEGIS128X2-entry-chunk-nonce-v1
  SBA1-AEGIS128X2-entry-chunk-aad-v1
  ```
  ⇒ **no** es "un AEGIS-256 sobre el archivo entero": un lector tiene que
  derivar nonce y AAD por chunk.
* Límites nativos: `"AEGIS-256 chunk size is out of range"`,
  `"AEGIS-256 entry order must be non-negative"`,
  `"AEGIS-256 sizes must be non-negative"`,
  `"Invalid SBA AEGIS archive crypto parameters"`.
* Compresión: **zstd** (`SbaZstdNative`, y `SbaSwiftTarNative` con rutas
  `…Zstd…`) ⇒ el tar interior va comprimido con zstd.

## 4. Formato del contenedor (medido)

### 4.1 Cabecera (`SbaArchiveHeaderInfo` = `Lk87;`, `SbaRootArchiveHeaderInfo` = `Lmb7;`)

Campos, en el orden del `toString`:

```
headerSize, flags, createdAtEpochMs, compression, compressionMethodId,
compressionLevel, encryption, encryptionMethodId, kdfMethodId, kdfIterations,
payloadChunkSize, kdf
```

⇒ **el método de KDF y sus iteraciones viajan en la cabecera** ⇒ un lector
Rust puede leer `t` (y el método) del propio archivo, sin conocer los
valores por defecto de la app.

### 4.2 Índice (`Lib7;.A` = parser del índice)

```
"Invalid SBA archive metadata length: "
"Invalid SBA index header size: "        (tamaño de cabecera de índice = 16)
"Invalid SBA entry count: "
"SBA entry count exceeds index bounds: count="
"minimum index size"                     (84)
máximo de índice                         (1048576 = 0x100000)
"Duplicate SBA entry name in index: "
"Trailing bytes after SBA index records: "
```

### 4.3 Interoperabilidad con lo que ya tiene la flota

* El interior es un **tar** (rutas `extractTarFromFdRaw`, `listTarEntries…`),
  comprimido con **zstd** ⇒ encaja con `backup_formats.rs` (TAR + zstd) y con
  el lector `metadata.json` ya escrito.
* El formato **no** es `.ab` (AOSP): es un contenedor propio con índice,
  cabecera y AEAD por chunks ⇒ `backup_ab.rs` **no** aplica aquí.

## 5. Lo que queda pendiente (necesita un `.dat`/`.med` real del móvil)

1. **Valores por defecto** de `t`/`m`/`p` y de `outputBytes` cuando la app
   crea un backup nuevo: no están hardcodeados en la clase del KDF (sólo los
   rangos); se leen del propio archivo (`kdfIterations`) ⇒ para **leer** no
   hacen falta.
2. **Origen y bytes exactos del salt**: no está entre los 12 campos de la
   cabecera ⇒ con un `.dat` real se localiza (índice o bloque de metadatos).
3. **Fórmula exacta del key-check** (`Ly87;.d`): SHA-256 sobre qué mezcla
   exacta (etiqueta + ordinal del método + qué más).
4. **Derivación de las sub-claves** a partir de la clave maestra de 32 B
   (HKDF? SHA-256 encadenado?): las 8 etiquetas están medidas, la función no.
5. `metadata.json` real de `appconfigs/data/`: el conjunto real de claves.

## 6. Receta para el lector Rust (lo que puede implementarse ya)

```
clave_maestra = Argon2id(
        contraseña  -> UTF-8,
        salt        -> 16 B,
        t           -> cabecera.kdfIterations,
        m           -> cabecera/config,
        p           -> cabecera/config,
        out         -> 32 B)
```

* Implementación **propia** de Argon2id (sin crates nuevos, como el codec OMC
  ya hecho en el repo) o con `argon2` si se puede tocar `Cargo.toml`
  (requiere acuerdo: `Cargo.toml` está bloqueado por acuerdo previo).
* AEGIS-256 propio (spec CFRG) para el chunk: clave 32 B, nonce y AAD por
  chunk según las etiquetas medidas.
* Regla de la flota: **sólo lectura** sobre los backups del usuario.

## 7. Herramientas (nuevas, en `tools/re/`)

* **`tools/re/dexdump2.py`** — vuelca el bytecode Dalvik de una clase
  (constantes, `const-string`, invocaciones). La **tabla de anchos de
  instrucción se genera desde `rust-core/src/dex_decompile.rs`** (fuente
  única) ⇒ no volver a escribirla a mano: `goto` (10t) ocupa 2 unidades y
  eso desalineaba todo.
* **`tools/re/dexrefs.py`** — para cada cadena que casa con un filtro dice la
  **clase + método** que la referencia. Recorre los `class_defs` y sus
  `code_item` ⇒ la atribución es correcta.
  ⚠️ `mp/dexorigins.py` (versión anterior, no commiteada) **atribuía mal**:
  escaneaba code items y luego atribuía la clase desde el `method_id` global
  ⇒ mezclaba clases. No usar sus resultados como evidencia.

## 8. Procedencia y límites

* Mirror: `d.apkpure.net` (`apkpure.com` y `apkmirror.com` devuelven 403 al
  egress del sandbox).
* Versión analizada: **5.1.1 (vC 635)**. Otras versiones pueden cambiar
  los identificadores ofuscados (clases/métodos) — los **formatos y rangos**
  son la parte estable; las etiquetas `SBA1-…`/`SBA2-…` son contrato de
  formato y deben permanecer iguales.
* Nada de código propietario se ha copiado: esto es **medición de
  interoperabilidad** (como el lector `.ab`), no una reimplementación de su
  motor criptográfico propietario.
