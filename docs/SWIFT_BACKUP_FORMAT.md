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

### 6.3 La tabla: los campos, con su offset y su ancho

> **Tabla canónica**: `rust-core/src/swift_archive.rs::sba1_layout(formato)`
> (fuente de verdad de la flota) y `docs/SWIFT_SBA1_HEADER_ARM64.md`.
> Este módulo **no la duplica**: la pide a `swift_archive` ⇒ el lector y el
> escritor no pueden divergir de ella.

⚠️ **Corrección** (detectada por el principal y verificada por mí):
los `csel` del escritor **eligen el VALOR** de un campo, **no su ancho**.
El helper `0x6ab6c` escribe **4 bytes fijos** big-endian
(`lsr w20,#24` → `#16` → `#8` → byte bajo, **sin ninguna rama de ancho**) ⇒
**u32 fijo**, no "1..3 bytes".  Los 27 *append* coinciden con la medición del
escritor x86_64 ⇒ ⇒ **una sola tabla sirve para las dos arquitecturas**.

#### Variante corta (formato ≠ 2) — `headerSize` = 96

| Offset | Bytes | Tipo | Campo |
|--------|-------|------|-------|
| 0 | 4 | ASCII | `SBA1` (magic) |
| 4 | 2 | u16 BE | `formato` |
| 6 | 2 | u16 BE | `headerSize` (96) |
| 8 | **4** | **u32 BE** | `compression` (= id del método de cifrado: 0, 2 ó 3) |
| 12 | 8 | u64 BE | `createdAtEpochMs` |
| 20 | 2 | u16 BE | `flags` |
| 22 | 2 | u16 BE | `flags2` |
| 24 | 2 | u16 BE | `campo3` |
| 26 | 2 | u16 BE | `campo4` |
| 28 | 2 | u16 BE | `campo5` |
| 30 | 2 | u16 BE | `lenBloque1` |
| 32 | 2 | u16 BE | `lenBloque2` |
| 34 | 2 | u16 BE | `lenBloque3` |
| 36 | 16 | bytes | `bloque1` (fijo, rellenado con ceros) |
| 52 | 16 | bytes | `bloque2` |
| 68 | 16 | bytes | `bloque3` |
| 84 | 2 | u16 BE | `campo10` |
| 86 | 2 | u16 BE | `campo11` |
| 88 | **4** | **u32 BE** | `campo12` |
| 92 | 4 | bytes | `reservado4` |

⇒ **96 exactos.**

#### Variante extendida (formato == 2) — `headerSize` = 144

Todo lo anterior igual hasta el offset 88, y a partir de ahí:

| Offset | Bytes | Tipo | Campo |
|--------|-------|------|-------|
| 92 | **4** | **u32 BE** | `campo13` |
| 96 | 2 | u16 BE | `campo14` |
| 98 | 2 | u16 BE | `campo15` |
| 100 | 2 | u16 BE | `campo16` |
| 102 | 2 | u16 BE | `campo17` |
| 104 | 32 | bytes | `reservado32` |
| 136 | 8 | bytes | `reservado8` |

⇒ **144 exactos.**

### 6.4 Los tres bloques de bytes

Cada bloque son **16 bytes fijos** (rellenados con ceros por la derecha),
precedidos de su longitud en `u16 BE`:

| Bloque | Longitud en | Datos en | Tamaño fijo |
|--------|-------------|----------|-------------|
| `bloque1` | offset 30 (u16 BE) | offset 36 | 16 B |
| `bloque2` | offset 32 (u16 BE) | offset 52 | 16 B |
| `bloque3` | offset 34 (u16 BE) | offset 68 | 16 B |

⇒ Los tres candidatos del modelo Kotlin de 12 campos (`salt`, `kdf`,
`keyCheck`) están **entre** estos tres, pero **cuál es cuál no está
confirmado** ⇒ no se les pone nombre.  El lector de la flota
(`KeyCheckSpec::probar_permutaciones`) **descubre el orden probando las 6
permutaciones**, así que con un `.dat` real se cierra solo.

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

Módulo propio, con **16 tests** (round-trip contra la tabla canónica,
big-endian, bloques en su offset, truncados, magic inválido, extracción
completa y saneado de rutas):

| API | Qué hace |
|---|---|
| `Cabecera::escribir()` | escribe la cabecera **exactamente** como la tabla medida (deriva de `sba1_layout`, no la duplica) |
| `Cabecera::leer(bytes)` | la lee con `sba1_detectar` + `parse_header` (la tabla canónica) |
| `leer_tar(bytes)` | índice del TAR interior (POSIX/GNU, nombres largos) |
| `sanear_ruta(nombre)` | rechaza rutas absolutas, `..` y `:` ⇒ **nada se escribe fuera del destino** |
| `informar_contenedor` / `extraer_contenedor` | cabecera + payload + zstd + TAR ⇒ **el extractor de un backup real** |

⇒ **No hay `Perfil`**: los anchos son fijos y salen de la tabla canónica;
así el lector **no puede desalinear** la cabecera real.

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


### 6.9 ¿Qué se puede verificar **sin** replicar el AEAD? (medido)

Cadena medida de los callers de `Ly87;.b` (el **único** primitivo HMAC de
Swift Backup) y de la tabla de imports de la nativa:

| Hecho | Cómo se midió |
|---|---|
| `Ly87;.b([B[BJJI[B)[B` = `HmacSHA256` | `Mac.getInstance("HmacSHA256")` + `SecretKeySpec` + `Mac.update` + `Mac.doFinal`; **deriva su clave** con `Ly87;.e` leyendo el campo estático `Ly87;.g` (la etiqueta) |
| `Ly87;.b` tiene **dos** callers, **ambos del índice** | escritor `Lg87;->f` (`CRC32.update` + `Ly87;.b` + `Os.pwrite`, strings *"SBA index CRC mismatch while writing MAC"*, *"Invalid SBA v2 index location while writing MAC: offset= size="*) · verificador `Lib7;->V` (`Ly87;.b` + `MessageDigest.isEqual`, string *"SBA encrypted index MAC mismatch"*) |
| La **nativa no tiene HMAC** | `strings libsba_archive.so` ⇒ **0** apariciones de `HmacSHA256`/HMAC; sus únicas etiquetas son `entry-chunk-aad` y `entry-chunk-nonce` |
| Las 8 etiquetas viven en el **DEX**, no en la nativa | `Ly87;-><clinit>` inicializa las 8; la nativa sólo trae las 2 de los chunks |
| Los 3 bloques de 16 B + `reservado32` (32 B) + `reservado8` (8 B) + `reservado4` (4 B) | el escritor ARM64 acaba con `bl 0x6b524` con `w2 = 0x20`, `w2 = 0x8` y `w2 = 0x4` ⇒ **32 + 8 + 4 bytes** ⇒ `reservado32` = la clave HMAC del payload (32 B, sin truncar) |

#### Consecuencia práctica

* ✅ **Verificable sin AEAD**: el **key-check** (`SHA-256(tag‖salt‖nonce2‖clave)`)
  y el **MAC del índice v2** (HMAC sobre el **índice cifrado** + CRC32).
  ⇒ **tamper-evident real** sobre la cabecera y el índice, sin tocar el AEAD.
* ❌ **NO verificable sin AEAD**: el **HMAC del payload**.  La clave
  (`reservado32`) sí se deriva y se guarda, pero **en el DEX no hay ningún
  consumidor** de ella (el único `Mac.doFinal` de Swift Backup es el del
  índice) **y la nativa no implementa HMAC** ⇒ ⇒ o el HMAC del payload se
  calcula dentro de la nativa (⇒ `aead_no_replicado`), o no se verifica
  nunca.  En ambos casos **no aporta una verificación adicional desde
  Rust**, y decirlo es más honesto que prometer un helper que devolvería
  basura.
* ⇒ El alcance honesto del lector queda: **parse + derivar clave + verificar
  key-check + verificar el MAC del índice** ⇒ y el **AEAD por trozos**
  (`aead_no_replicado`), que es donde vive la confidencialidad.


---

## 7. El AEAD por trozos: lo que se ha medido en la nativa

> Medición sobre `libsba_archive.so` (x86_64, `lib/x86_64/`), el mismo
> binario que la app usa en Android (ARM64) — las dos arquitecturas
> coinciden en la secuencia.
> Código: `rust-core/src/swift_archive.rs` (funciones `derive_chunk_nonce`,
> `derive_chunk_aad`, `aead_cifrar`, `aead_descifrar`).

### 7.1 El AESEA es **SHA-256** (no hay AES detrás)

Los valores iniciales que la nativa carga en `.rodata` son **exactamente**
los de SHA-256:

| Offset en el `.so` (x86_64) | Valor | Es |
|---|---|---|
| `0x150c0` | `6a09e667 bb67ae85 3c6ef372 a54ff53a` | H0..H3 de SHA-256 |
| `0x14fa0` | `510e527f 9b05688c 1f83d9ab 5be0cd19` | H4..H7 de SHA-256 |

Y el relleno final usa la longitud **en bits big-endian** (`bswap` de un
`u64` en `0x77328`) ⇒ es **SHA-256 estándar**, no una variante.

### 7.2 Nonce y AAD por trozo

Dos funciones internas distintas, una por etiqueta:

| | Función | Entrada | Salida |
|---|---|---|---|
| **nonce** | `0x77190` | `SHA-256( etiqueta_nonce ‖ D )` | 32 B |
| **AAD** | `0x77770` | `SHA-256( etiqueta_aad ‖ D )` | 32 B |

Etiquetas (literales en `.rodata`):

| Método | Etiqueta del nonce | Etiqueta del AAD |
|---|---|---|
| AEGIS-256 | `SBA1-AEGIS256-entry-chunk-nonce-v1` (`0x1536d`) | `SBA1-AEGIS256-entry-chunk-aad-v1` (`0x18802`) |
| AEGIS-128X2 | `SBA1-AEGIS128X2-entry-chunk-nonce-v1` (`0x1528d`) | `SBA1-AEGIS128X2-entry-chunk-aad-v1` (`0x13588`) |

⇒ **nonce y AAD se derivan del MISMO bloque `D` con etiquetas distintas**
⇒ es **separación de dominio** por hash, no dos claves distintas.

### 7.3 Qué es `D`: **el bloque de 16 bytes de la cabecera**

El constructor del contexto (`0x7070c`) copia:

```
ctx+0x00 = "AEGIS-256"                       (el nombre del algoritmo)
ctx+0x08 = "SBA1-AEGIS256-entry-chunk-nonce-v1"
ctx+0x10 = "SBA1-AEGIS256-entry-chunk-aad-v1"
ctx+0x18 = <callback del AEAD>              (0x88450)
ctx+0x20 = 32                                (longitud de la clave)
ctx+0x28 = <int>                             (el "entry order")
ctx+0x30 = <32 B>  ← la clave
ctx+0x48 = <16 B>  ← **D**    (`mov $0x10,%ecx` + `memcpy`)
ctx+0x60 = <N B>             (bloque variable)
```

⇒ **D son 16 bytes** ⇒ por la forma del key-check
(`SHA-256(etiqueta ‖ salt ‖ nonce2 ‖ clave)`) y por los tres bloques de
16 B de la cabecera, **D = el `salt`** (`bloque1`).

### 7.4 El AEAD es un **flujo continuo**, no un AEAD por trozo

El contexto se construye **una vez** por entrada y el bucle de trozos
(`ctx+0x88/0x90` = par *offset/length*, `ctx+0xb8` = cursor) reutiliza el
**mismo estado** ⇒ ⇒ el relleno con ceros del último bloque parcial va
**al final del mensaje**, no por trozo ⇒ ⇒ **equivale a cifrar el mensaje
entero de una vez** ⇒ ⇒ el `aegis.rs` del repo (que es one-shot) sirve
tal cual.

### 7.5 Los otros dos números medidos

| Parámetro | Valor | Evidencia |
|---|---|---|
| **Tamaño de trozo** | `1..=0x10000` (**64 KiB** máximo) | la validación compara con `0x10001`; error *"AEGIS-256 chunk size is out of range"* |
| **Longitud de la etiqueta AEAD** | **16 B** | `mov $0x10,%edx` justo antes de la llamada al callback del AEAD |
| **Firma JNI** | `b(int, long, int, byte[], byte[], byte[], long, long, int, listener): long[]` | `SbaLibaegisCryptoNative.b` en el DEX |

### 7.6 Lo que queda por cerrar (con un `.dat` real)

| Pieza | Estado |
|---|---|
| Nonce, AAD, clave, etiqueta y flujo | ✅ **medidos y replicados** (`swift_archive.rs`) |
| **Encuadre del contenedor** | 🟡 **se descubre**: el lector prueba las geometrías plausibles y **valida con el MAC del índice** (HMAC-SHA256 ⇒ 32 B ⇒ sin falsos positivos). Falta un `.dat` real para confirmar que alguna cuadra |
| **AEGIS-128X2** | ❌ no replicado (es la alternativa; AEGIS-256 es el principal, según `isSbaAegis256Available`) |
| **Nonce/AAD por *entrada*** | 🟡 si el contenedor tuviese **una sub-cabecera por entrada**, el `D` sería el de esa entrada ⇒ el lector lo trata como un parámetro, no como una constante |

⇒ Con un `.dat` real, el lector **descubre el encuadre solo** (validando
el MAC del índice) y devuelve el contenido descifrado con la etiqueta
AEAD verificada.



### 7.7 El encuadre del contenedor (modelo medido + autodeteción)

La estructura que la nativa deja (lectura de los mensajes de error y del
escritor):

```text
+--------------------------------+ 0
|  cabecera SBA1 (96 ó 144 B)    |
+--------------------------------+ header_size
|  stream AEAD:                 |
|    payload cifrado             |
|    indice cifrado             |  index_off
+--------------------------------+ tag_off
|  etiqueta AEAD (16 B)         |
+--------------------------------+ mac_off
|  MAC del indice (en claro)    |
+--------------------------------+ fin
```

* El **índice va cifrado** (dentro del stream): el verificador falla con
  *"SBA encrypted index MAC mismatch"* ⇒ ⇒ el MAC se calcula **sobre los
  bytes cifrados del índice**.
* El **MAC va en claro y al final** (se escribe con `pwrite` en *"SBA v2
  index location"*; error *"SBA archive is too small for v2 index MAC"*).
* El MAC del índice es `HMAC-SHA256(indice_cifrado, clave_mac)` con
  `clave_mac = SHA-256("SBA2-index-mac-key-v1" ‖ salt ‖ nonce2 ‖ clave)`
  (misma forma que el key-check, con **otra etiqueta**).  Existe además
  `SBA2-index-metadata-mac-v1` para los metadatos.

⇒ **El lector se autodetecta**: prueba las longitudes de índice plausibles
(declaradas en la cabecera + una lista) contra las del MAC y **sólo acepta
la geometría cuyo MAC cuadra** ⇒ ⇒ 32 bytes de HMAC ⇒ **sin falsos
positivos**.

### 7.8 Estado de la replicación (honesto)

| Pieza | Estado |
|---|---|
| KDF Argon2id | ✅ `crate::argon2::derive_key` (verificado, RFC 9106 + oráculo C) |
| Key-check | ✅ `SHA-256(etiqueta ‖ salt ‖ nonce2 ‖ clave)[:16]` |
| Clave HMAC del payload | ✅ `SHA-256("SBA1-payload-hmac-key-v1" ‖ salt ‖ nonce2 ‖ clave)` (32 B) |
| **Nonce por trozo** | ✅ `SHA-256("SBA1-AEGIS256-entry-chunk-nonce-v1" ‖ salt)` (32 B) |
| **AAD por trozo** | ✅ `SHA-256("SBA1-AEGIS256-entry-chunk-aad-v1" ‖ salt)` (32 B) |
| **Flujo AEAD** | ✅ AEGIS-256 continuo, **etiqueta de 16 B** (usa `crate::aegis`, RFC 10032) |
| **Encuadre del contenedor** | 🟡 **autodetección por el MAC del índice** (funciona; falta confirmarlo con un `.dat` real) |
| AEGIS-128X2 | ❌ alternativa de la app; no replicada (AEGIS-256 es el principal) |
| Descifrado **por entrada** (sub-cabecera propia) | 🟡 si el contenedor tuviese una sub-cabecera por entrada, el `D` (salt) sería el de esa entrada ⇒ el código lo toma como **parámetro**, no como constante |

⇒ El estado `aead_no_replicado` **ya no se usa para el AEAD** (que sí está
replicado y medido); queda reservado para AEGIS-128X2 y para cuando el
encuadre no se puede validar.

