# Swift Backup · formato real, medido (y qué falta para leerlo)

> **Cómo se midió sin el móvil**: el APK se bajó de un mirror
> (`d.apkpure.net`) y sus **DEX son byte a byte los del móvil**:
> `classes.dex` `1db45566d56d9b26da945b1ac04fea8a3075cd6adba4727dfce27796f272a7fd`
> y `classes2.dex` `6bb33ae5c983c7e7e1f2f11048892dd44b7a9c0368db7d74211a07a729036844`
> (el APK **sí** difiere: `a059e010…` del mirror vs `f73db4ca…` del instalado
> ⇒ el mirror reempaqueta/firma, pero el código es idéntico ⇒ **el RE se
> puede hacer entero en local**).

## 1. Inventario de algoritmos (la verdad está en `libsba_archive.so`)

La app trae dos nativas ARM64: `libconceal.so` (258 816 B; el
"Conceal" de criptografía: AES-CBC-HMAC, PBKDF2, HMAC) y
**`libsba_archive.so` (1 371 184 B)** — aquí está **todo** el backup.

Exportaciones JNI de `libsba_archive.so` (sólo 20; el resto está stripped):

| JNI | Qué dice del formato |
|---|---|
| `SbaLibaegisCryptoNative_encryptAegis256ChunkedEntryFdRaw` | **AEGIS-256** por trozos de entrada |
| `SbaLibaegisCryptoNative_decryptAegis256ChunkedEntryFdRaw` | ídem (lectura) |
| `SbaLibaegisCryptoNative_decryptAegis128X2ChunkedEntryFdRaw` | alternativa **AEGIS-128X2** |
| `SbaLibaegisCryptoNative_isAvailable` | el backend AEGIS es opcional |
| `SbaNativeCrypto_deriveArgon2id` | **la clave se deriva con Argon2id** (no PBKDF2) |
| `SbaSwiftTarNative_extractTarZstdFromFdRaw` | **el contenido es TAR comprimido con ZSTD** |
| `SbaSwiftTarNative_listTarEntriesZstdFromFdRaw` | listar entradas (sin extraer) |
| `SbaSwiftTarNative_extractAegisArchiveEntry` / `...Fused` | extraer una entrada de un archivo AEGIS |
| `SbaSwiftTarNative_createTarToFdWithWriteStatsRaw` | escritura (crear) |
| `SbaZstdNative_{compress,decompress}Zstd{Bytes,FdToFdRaw,FileToFdRaw}` | zstd |
| `SbaArchiveNative_createArchive` | crear el archivo contenedor |
| `SbaRuntimeNative_version` | versión del runtime |

⇒ **Contenedor = TAR + ZSTD + AEGIS**, no un formato propietario opaco.
⇒ Mi lector **TAR** (`backup_formats.rs`) aplica directamente al payload.

## 2. La criptografía, con sus límites exactos (medidos en la nativa)

| Parámetro | Valor / rango (de los mensajes de error de la nativa) |
|---|---|
| KDF | **Argon2id** (`deriveArgon2id`), con **passphrase** (hay un log `Aegis256(password=<redacted>)`) |
| `iterations` | **1..100** ("Argon2id iterations must be in 1..100") |
| `memoryKiB` | **16384..65536** (16–64 MiB) |
| `parallelism` | **1..8** |
| `outputBytes` | **16..128** |
| salt | hay comprobaciones de longitud ("Salt is too long/short", "Salt pointer is NULL, but salt length is not 0") ⇒ **la sal va en la cabecera** |
| AEAD | **AEGIS-256** (principal) · **AEGIS-128X2** (alternativa) |
| AAD por trozo | etiqueta de dominio **`SBA1-AEGIS256-entry-chunk-aad-v1`** (y `-nonce-v1` para el nonce; las equivalentes de 128X2) |
| Autocomprobación | **`SBA1-AEGIS256-key-check-v1`** (KAT interno de la app) |
| Magics | **`SBA1`** (archivo), `SBA_SWIFTTAR` (formato tar propio) |

⇒ **AEGIS-256 ya está implementado en Rust puro** en `rust-core/src/aegis.rs`
(RFC 10032, validado con los vectores oficiales de la RFC: `AESRound`,
`Update`, TV1, TV2, TV3). Falta **AEGIS-128X2** sólo si la app lo usa por
defecto (el chequeo del DEX es `isSbaAegis256Available` ⇒ **AEGIS-256 es el
principal**).

## 3. Qué falta para leer un `.dat` de verdad

| Pieza | Estado | Cómo se cierra |
|---|---|---|
| **AEGIS-256** | ✅ hecho y validado (RFC 10032) | — |
| **TAR** | ✅ hecho (`read_tar`, nombres largos GNU, pax) | — |
| **AEGIS-128X2** | opcional | vectores A.4/A.5 de la RFC si hace falta |
| **Argon2id** | ❌ | implementarlo **en Rust puro** (BLAKE2b + núcleo Argon2) y validarlo con los vectores de la **RFC 9106** (no hay crate disponible: crates.io no es accesible desde el build) |
| **zstd** | ❌ | sin crate ⇒ dos vías: **FFI a `libzstd`** del sistema o el binario `zstd` de Termux; el payload es zstd ⇒ **es el cuello de botella** |
| **Cabecera** (orden de campos: magic, versión, sal, iteraciones, memoria, tamaño de trozo, entradas) | ❌ |ingeniería inversa del *writer* en la nativa (xref al magic `SBA1`) |
| **Derivación del nonce y del AAD por trozo** | ❌ | misma vía (las etiquetas de dominio están, la función KDF no) |
| **Un `.dat` real** | ❌ | necesita el móvil (o el usuario lo suba) |

⇒ **Lo que ya es verdad y usable**: el inventario de algoritmos, los límites
del KDF, las etiquetas de dominio, los magics, y que el contenido es TAR.
⇒ **Lo que NO es aún**: abrir un `.dat` de principio a fin (faltan Argon2id,
zstd, la cabecera y un fichero real).

## 4. Postura legal

Swift Backup es **propietario** (SwiftApps). Aquí **no se ha copiado ni una
línea de su código**: sólo se han medido **bytes, cadenas y exportaciones**
de su binario para poder **leer** backups del propio usuario (interop).
Lo que se reimplementa en MegaDaemon es **nuestro** (AEGIS-256 con la RFC,
TAR con el formato público, Argon2id con la RFC 9106). Ver
`docs/BACKUP_FORMATS_CATALOG.md` para la regla de licencias por proyecto.

## 5. Reglas aprendidas (para el que siga)

1. **El mirror sirve si los DEX coinciden**: compara **sha256 de `classes.dex`**
   (no del APK) con el del dispositivo ⇒ si cuadra, el RE se hace entero en
   local ⇒ **el móvil deja de ser un cuello de botella**.
2. **La nativa exporta el algoritmo, no el código**: los `JNI` +
   los mensajes de error **delatan los parámetros exactos** (rango de
   iteraciones, de memoria, de paralelismo) sin desensamblar nada.
3. **El dominio de las etiquetas (`-aad-v1`, `-nonce-v1`) dice cómo se
   derivan** el AAD y el nonce ⇒ es la pista para la última pieza del KDF.
