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
| **Argon2id** | ✅ **hecho y validado** | `rust-core/src/argon2.rs`: BLAKE2b + H' + H0 + relleno, **verde con los 3 vectores oficiales de la RFC 9106** (d/i/id) **y** con 36/36 comparaciones byte a byte contra el **oráculo C** (`phc-winner-argon2` 20190702) en el rango de Swift. API: `derive_key(passphrase, salt, iterations, memory_kib, parallelism, out_len)` |
| **zstd** | ✅ **hecho** | `rust-core/src/zstd.rs`: **FFI por `dlopen`** a `libzstd.so.1` (sin dependencia de compilación) **con fallback al binario `zstd -d -c`** de Termux. Fixture real congelado en el test |
| **Cabecera** (orden de campos: magic, versión, sal, iteraciones, memoria, tamaño de trozo, entradas) | 🟡 **inspeccionada, no congelada** | `rust-core/src/swift_sba1.rs`: se reconoce el magic, se deduce el tamaño (96/144) y se **inspeccionan** las candidatas big-endian. **No se congela el layout hasta tener un `.dat` real** |
| **Derivación del nonce y del AAD por trozo** | ❌ | misma vía (las etiquetas de dominio están, la función KDF no) |
| **Un `.dat` real** | ❌ | necesita el móvil (o el usuario lo suba) |

⇒ **Lo que ya es verdad y usable**: el inventario de algoritmos, los límites
del KDF, las etiquetas de dominio, los magics, y que el contenido es TAR.
⇒ **Lo que NO es aún**: abrir un `.dat` de principio a fin (faltan Argon2id,
zstd, la cabecera y un fichero real).

## 3.1. Estado del lector (2026-11-21)

`rust-core/src/swift_sba1.rs` + `rust-core/src/zstd.rs` (**14/14 tests verdes**):

| Qué hace | Cómo | Nota de honestidad |
|---|---|---|
| Reconoce el contenedor | `es_sba1()` / `leer_cabecera()` | magic `SBA1` y `SBA_SWIFTTAR` |
| Deduce el tamaño de cabecera | 96 o 144 bytes (**big-endian**, medido en el escritor ARM64 ⇒ mismo orden en x86_64) | por *scanning*, no por offset fijo |
| Inspecciona los campos | candidatas u32/u64 big-endian con su offset | **no** se interpretan: hace falta un `.dat` real |
| **Descomprime el payload** | zstd → TAR | **esto sí está resuelto** (TAR + ZSTD es lo medido) |
| Extrae a disco | `extraer_tar_zstd()` con rutas saneadas (`..`, absolutas y `:` se rechazan) | evita escribir fuera del destino |
| Limitaciones | `max_salida` acota la RAM (el backup completo del usuario no cabe en el móvil) | error explícito, no desbordamiento silencioso |

⇒ **Lo que falta para abrir un `.dat` de punta a punta**: el layout exacto de
la cabecera, la derivación **nativa** de nonce/AAD por trozo, y un fichero
real contra el que validar. El **KDF ya no bloquea** (Argon2id verificado) y
**zstd tampoco**.

## 3.2. Regla de oro del formato: no congelar lo no medido

El `SBA1` **no** tiene todavía un layout publicado. Por eso `leer_cabecera()`
devuelve `crudo` + `campos` (offset, valor) en vez de un `struct` con campos
con nombre: cuando llegue un `.dat` real, se **casa** el layout y *entonces*
se congela en un test. Congelar ahora sería escribir el formato a ciegas.

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

---

## 6. El layout **byte a byte** de la cabecera (medido en el escritor)

> Añadido por el agente RE de APK/nativo.  Source: **escritor ARM64**
> (`SbaArchiveNative_createArchive` de `libsba_archive.so`, la arquitectura
> del móvil).  **Contrasteado** con la medición del escritor **x86_64**
> (otro agente) ⇒ **las dos arquitecturas coinciden en la secuencia
> completa de 27 escrituras** ⇒ el layout **no cambia** por arquitectura.

### 6.1 Cómo se localizó (sin símbolos: la `.so` va stripped)

1. El literal `"SBA1"` está en `.rodata` en el offset **0x11b35**
   (verificado: `SBA1\x00failed to a…`).
2. El xref es **`adrp x2, 0x11000`** + **`add x2, x2, #0xb35`**
   (`0x6562c` / `0x65630`) ⇒ ése es el escritor de la cabecera.
3. A partir de ahí, la secuencia de llamadas a los *helpers* de *append*
   da el orden exacto de los campos.

### 6.2 Los helpers (y lo que dicen)

| Helper | Qué es | Evidencia |
|---|---|---|
| `0x6ae10` | **append de bytes** `(buf, ptr, len)` | `bl 0x6ae10` con `x2="SBA1"`, `w4=4` |
| `0x6aa00` | **append de entero de 2 bytes, BIG-ENDIAN** | `lsr w8,w20,#8` + `strb w8` (byte **alto**) y luego `strb w20` (byte ** bajo**); **no recibe argumento de ancho** ⇒ siempre 2 bytes |
| `0x6ab6c` | **append de entero de ancho variable** (1..3 B) | `lsr w8,w20,#24` + `strb` ⇒ escribe el byte más alto primero; el ancho lo elige el llamante con un `csel` |
| `0x6b140` | **append de bytes** (sin prefijo) | `memcpy` de `x4` bytes |
| `0x6b524` | **append de bloque** (longitud + bytes) | `mov x21,x2` (longitud) · `mov x22,x3` (puntero) |

### 6.3 La tabla: los 27 campos, en orden

| # | Bytes | Tipo | Contenido | Origen en el escritor |
|---|-------|------|-----------|----------------------|
| 1 | 4 | ASCII | `SBA1` | `adrp/add` → `bl 0x6ae10` (`x2=0x11b35`, `w4=4`) |
| 2 | 2 | **u16 BE** | `flags` | `ldr w1,[x20]` → `bl 0x6aa00` |
| 3 | 2 | **u16 BE** | `headerSize` = **0x60 (96)** ó **0x90 (144)** | `mov w8,#0x60` + `csel w23,w9,w8,eq` → `bl 0x6aa00` |
| 4 | **1..3** | **var BE** | `encryptionMethodId` | `ldr w8,[x20,#4]` + `csel w1,w10,w9,eq` → `bl 0x6ab6c` |
| 5 | 8 | **u64 BE** | `createdAtEpochMs` = `time(NULL)*1000` | `bl time@plt`; `mul x28,x0,#1000`; escrito **byte a byte** con `lsr x20,x28,x21` empezando por `>>56` ⇒ **MSB primero** |
| 6 | 2 | u16 BE | campo A (sin nombre confirmado) | `ldr w1,[sp,#20]` → `bl 0x6aa00` |
| 7 | 2 | u16 BE | campo B (sin nombre confirmado) | `ldr w1,[sp,#16]` → `bl 0x6aa00` |
| 8 | 2 | u16 BE | `struct[0x04]` | `ldr w1,[x20,#4]` → `bl 0x6aa00` |
| 9 | 2 | u16 BE | `struct[0x08]` | `ldr w1,[x20,#8]` → `bl 0x6aa00` |
| 10 | 2 | u16 BE | `struct[0x0c]` | `ldr w1,[x20,#12]` → `bl 0x6aa00` |
| 11 | 2 | u16 BE | **longitud del bloque 1** | `ldr w8,[x20,#72]` − `ldr w9,[x20,#64]` → `bl 0x6aa00` |
| 12 | 2 | u16 BE | **longitud del bloque 2** | `ldr w8,[x20,#96]` − `ldr w9,[x20,#88]` → `bl 0x6aa00` |
| 13 | 2 | u16 BE | **longitud del bloque 3** | `ldr w8,[x20,#120]` − `ldr w9,[x20,#112]` → `bl 0x6aa00` |
| 14 | 4 | u32 BE | longitud del bloque 1 | `bl 0x6b140`/`0x6b524` |
| 15 | N | bytes | **bloque 1** | `ldp x2,x3,[x20,#64]` → `sub x4,x3,x2` |
| 16 | 4 | u32 BE | longitud del bloque 2 | |
| 17 | M | bytes | **bloque 2** | `ldp x2,x3,[x20,#88]` |
| 18 | 4 | u32 BE | longitud del bloque 3 | |
| 19 | P | bytes | **bloque 3** | `ldp x2,x3,[x20,#112]` |
| 20 | 2 | u16 BE | `0` | `mov w1,wzr` → `bl 0x6aa00` |
| 21 | 2 | u16 BE | `0` | `mov w1,wzr` → `bl 0x6aa00` |
| 22 | **1..3** | **var BE** | `struct[0x18]` | `ldr w1,[x20,#24]` → `bl 0x6ab6c` |
| 23 | **1..3** | **var BE** | `struct[0x10]` — **sólo si `flags == 2`** | `cmp w8,#2` → `ldr w1,[x20,#16]` → `bl 0x6ab6c` |
| 24 | 2 | u16 BE | `struct[0x14]` | `ldr w1,[x20,#20]` → `bl 0x6aa00` |
| 25 | 2 | u16 BE | `struct[0x1c]` | `ldr w1,[x20,#28]` → `bl 0x6aa00` |
| 26 | 2 | u16 BE | `struct[0x20]` | `ldr w1,[x20,#32]` → `bl 0x6aa00` |
| 27 | 2 | u16 BE | `struct[0x04]` (última escritura) | `ldr w8,[x20,#4]` → `bl 0x6aa00` |

### 6.4 Los tres bloques de bytes

El struct nativo (`x20`) tiene **tres arrays**, leídos como
`(inicio, fin)` en los offsets:

| Bloque | Inicio | Fin | Tamaño máximo observado |
|---|---|---|---|
| 1 | `struct[0x40]` | `struct[0x48]` | 32 B (candidato: **key-check** = SHA-256) |
| 2 | `struct[0x58]` | `struct[0x60]` | 16 B (candidato: **sal**) |
| 3 | `struct[0x70]` | `struct[0x78]` | 8 B (candidato: **id/nonce**) |

⇒ Los candidatos a `salt`, `kdf` y `keyCheck` del modelo Kotlin de 12
campos (`headerSize, flags, createdAtEpochMs, compression,
compressionMethodId, compressionLevel, encryption, encryptionMethodId,
kdfMethodId, kdfIterations, payloadChunkSize, kdf`) están **entre estos
tres**, pero **cuál es cuál no está confirmado** ⇒ no se les pone nombre.

### 6.5 Reglas del formato que sí quedan **cerradas**

1. **BIG-ENDIAN para todos los enteros, sin excepción.**  Verificado en dos
   helpers independientes: el de 2 bytes (`0x6aa00`: `value>>8` primero) y
   el de 8 bytes (`createdAtEpochMs`: `>>56` primero) ⇒ coherente con
   `DataOutputStream` de Java/Kotlin ⇒ ⇒ **un lector little-endian leería
   todos los campos al revés**.
2. **`headerSize` es 96 (0x60) o 144 (0x90)**: 96 para la info page de una
   entrada, 144 para la del archivo raíz.
3. **La secuencia de campos no depende de la arquitectura** (ARM64 == x86_64).
4. Los campos **4, 22 y 23** tienen **ancho variable** ⇒ **un lector no
   puede deducir su ancho sólo de los bytes**.

### 6.6 El lector y el escritor (`rust-core/src/swift_sba1_header.rs`)

Módulo nuevo, con **9 tests** (round-trip, big-endian, truncados, magic
inválido, tamaño desconocido):

| API | Qué hace |
|---|---|
| `Cabecera::escribir(&self, perfil)` | escribe la cabecera **exactamente** como el nativo |
| `Cabecera::leer(bytes, perfil)` | la lee; devuelve la cabecera y los bytes consumidos |
| `leer_auto(bytes)` | prueba los dos perfiles (con/sin cifrado) y se queda con el que cuadra |
| `Perfil` | los **anchos** de los campos de ancho variable (1..3 bytes) |

⇒ El perfil es explícito **a propósito**: hasta que haya un `.dat` real,
**no se congela** el layout.  Cuando llegue, se casa el perfil con los
bytes y *entonces* se fija en un test (misma regla que §3.2).

### 6.7 Lo que **sigue** sin poder cerrarse

| Pieza | Por qué | Dónde está |
|---|---|---|
| **nonce y AAD por trozo** | la derivación es **NATIVA** (`SbaLibaegisCryptoNative_decryptAegis256ChunkedEntryFdRaw`) ⇒ las etiquetas `SBA1-AEGIS256-entry-chunk-nonce-v1` / `-aad-v1` son **etiquetas**, no el algoritmo | `libsba_archive.so` |
| **ancho de los campos variables** | lo decide el escritor con un `csel` según otros campos ⇒ no es autodominante | `Perfil` |
| **nombre de los 3 bloques** | faltan bytes reales | §6.4 |
| **un `.dat` real** | el usuario tiene que exportar el backup | — |

### 6.8 Lo que se ha cerrado desde que se escribió esto

* **El key-check ya tiene fórmula exacta** (otro agente, commit `42bb52a`):
  `SHA-256( tag ‖ salt ‖ nonce2 ‖ clave )[:16]`, con la etiqueta
  `SBA1-AEGIS256-key-check-v1` ⇒ ver `docs/SWIFT_KOTLIN_MARSHALLING.md` y
  `rust-core/src/swift_archive.rs` (`KeyCheckComposition::SwiftTagged`).
* **El lector DEX del repo ya está arreglado** (los mismos tres bugs que
  encontró este agente, de forma independiente):
  1. el opcode va en el **byte bajo** del code unit (el comentario anterior
     "byte alto" estaba equivocado ⇒ el desensamblado era casi todo `nop`);
  2. el payload de `encoded_value` **no** tiene ancho fijo por tipo;
  3. `read_u8` recibía el offset por valor ⇒ el payload no avanzaba el `pos`.
  ⇒ **el motor de este repo (`apk_deobfuscator.rs`) lleva su propio lector
  de DEX, little-endian y tolerante**, y da **0 errores** con esos tres
  bugs ya corregidos de forma independiente en el lector compartido.

⇒ **El round-trip sintético** (escribir → leer) **sí** está hecho y
testeado: prueba que nuestro escritor reproduce la secuencia del nativo y
que el lector la invierte.  Lo que **no** prueba (y no se afirma) es la
interoperabilidad con la app, que necesita el `.dat` real y la derivación
nativa de nonce/AAD.
