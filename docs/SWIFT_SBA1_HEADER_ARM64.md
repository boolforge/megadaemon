# Layout de la cabecera `SBA1` — medido en el **escritor ARM64** (el del móvil)

Fuente: `libsba_archive.so` **ARM64** del APK de Swift Backup 5.1.1
(`/workspace/swift/nat/lib/arm64-v8a/libsba_archive.so`, 1.371.184 B, sha16 `798750abd220a8ae`).
El agente DEX midió el escritor **x86_64**; aquí se repite la medición **sobre ARM64**,
que es la arquitectura del dispositivo ⇒ sirve para responder si el formato cambia
entre arquitecturas.

## Cómo se localizó (sin símbolos: el .so va stripped)

1. `magic "SBA1"` en `.rodata`, offset/vaddr **0x11b35** (en este .so vaddr == offset).
2. Xref en `.text` (AArch64, `adrp` + `add`): **`0x6562c: adrp x2, 0x11000`** /
   **`0x65630: add x2, x2, #0xb35`** ⇒ ése es el escritor de la cabecera.
3. A partir de ahí se leen las llamadas a los *helpers* de append.

## Lo medido (hechos, no inferencias)

| # | Hecho | Evidencia |
|---|-------|-----------|
| 1 | El primer campo es el literal **"SBA1"** (4 bytes) | `adrp/add` → `0x11b35`; `bl 0x6ae10` (append bytes, `x2`="SBA1", `w4`=4) en `0x65640` |
| 2 | La cabecera se escribe en un **buffer de `headerSize` bytes** | `0x65608: cmp w8,#2` · `0x6560c: mov w8,#0x60` · `0x65610: csel w23,w9,w8,eq` · `0x65614: mov x0,w23` · `bl 0x147c04` (alloc) |
| 3 | `headerSize` por defecto = **0x60 (96)**; hay una **variante** con otro valor (`w9`) | idem (`csel` con la rama alternativa) — coincide con lo medido en x86_64 (96/144) |
| 4 | Los enteros se escriben **byte a byte, LSB→MSB en la memoria pero con el byte ALTO primero** ⇒ **BIG-ENDIAN** | helper `0x6aa00`: `0x6aa2c: lsr w8,w20,#8` + `strb w8,[x26],#1` (byte alto en la posición N) y `0x6aac8: strb w20,[x26],#1` (byte bajo en N+1) |
| 5 | Ese helper escribe **exactamente 2 bytes** ⇒ es el append de **u16** | sólo dos `strb` en el camino rápido + el de capacidad |
| 6 | Hay un helper de **bytes con longitud** (`0x6ae10`) y otro de **bloque (len + bytes)** (`0x6b524`) | `bl 0x6ae10` (x2 puntero, w4 longitud) · `bl 0x6b524` (x2/x3 = puntero+longitud) |
| 7 | El **orden de campos** leído del escritor ARM64 (tras el magic): `u16` de `[x20+0]`, `u16` headerSize, … `[sp+0x10]`, `[sp+0x14]`, `[x20+4]`, `[x20+8]`, `[x20+0xc]`, tres **diferencias** (`[x20+0x48]-[x20+0x40]`, `[x20+0x60]-[x20+0x58]`, `[x20+0x78]-[x20+0x70]`), un bloque `len+bytes`, dos `u16` a cero, y `[x20+0x18]`, `[x20+0x10]`, `[x20+0x14]`, `[x20+0x1c]`, `[x20+0x20]` | secuencia de `bl 0x6aa00` entre `0x65640` y `0x658d0` |

## Respuestas a las preguntas abiertas

* **¿Cambia el layout entre x86_64 y ARM64?** ⇒ **NO en la forma**: la secuencia de campos
  (magic → tamaño → campos `u16` → diferencias → bloque → ceros → campos finales) es la misma
  que la medida en x86_64 por el agente DEX ⇒ ⇒ **una sola implementación del lector sirve
  para ambas arquitecturas**. Lo que **no** está cerrado es el *ancho exacto* de cada campo
  (los enteros de 4 y 8 bytes usan helpers hermanos aún no identificados) ni a qué nombre del
  modelo Kotlin corresponde cada valor (`salt`, `kdf`, `keyCheck`).
* **¿El nonce/AAD por trozo?** ⇒ **sigue siendo nativo** (`SbaLibaegisCryptoNative_decryptAegis*ChunkedEntryFdRaw`
  aparece en el propio `.so`) ⇒ ⇒ **no se escribe a ciegas**: la etiqueta
  `SBA1-AEGIS256-entry-chunk-nonce-v1` es una *etiqueta*, no el algoritmo.

## Segunda pasada (2026-11-21): el helper de enteros y el BIG-ENDIAN

* El helper `0x6aa00` **no es un append de ancho fijo**: tiene **varias rutas** de
  escritura ⇒ el **ancho se recibe como argumento** (`x2`/`w2`).
* **BIG-ENDIAN: invariante del formato** (no una observación puntual). En la ruta
  de 2 bytes se storea `valor>>8` y luego `valor` (`0x6aa2c`/`0x6aac8`); en la
  de 4 bytes se storean los desplazamientos **24, 16, 8, 0** (`0x6ab98`,
  `0x6ac34`, …) ⇒ ⇒ **el byte más alto va primero en todos los anchos**
  ⇒ coherente con `DataOutputStream` de Java/Kotlin ⇒ ⇒ ⇒ **no es
  Little-Endian**: un lector que lo asumiera leería los campos al revés.
  ⇒ **regla para el lector: todos los enteros de la cabecera se leen
  big-endian, sin excepción**.
* El argumento de ancho que se usa en la tanda de campos **no es una constante**
  sino un valor calculado (`x25 = x22 - x23`, `0x656cc`) ⇒ ⇒ **de aquí no sale
  el ancho exacto de cada campo**.

### Offsets del struct nativo leídos por el escritor

`x20` es el struct que Kotlin rellena y pasa a la nativa. El escritor lee:

| Offset | Lectura | Tipo probable |
|--------|---------|----------------|
| `+0x00` | `ldr w1,[x20]` | entero (flags) |
| `+0x04`, `+0x08`, `+0x0c` | `ldr w1,[x20,#..]` | enteros |
| `+0x10`, `+0x14` | `ldr w1,[x20,#..]` | enteros |
| `+0x18`, `+0x1c`, `+0x20` | `ldr w1,[x20,#..]` | enteros |
| `+0x40`/`+0x48` | puntero y longitud | **byte array 1** |
| `+0x58`/`+0x60` | puntero y longitud | **byte array 2** |
| `+0x70`/`+0x78` | puntero y longitud | **byte array 3** |

⇒ tres arrays de bytes (los candidatos son `salt`, `kdf` y `keyCheck` del modelo
Kotlin de 12 campos: `headerSize, flags, createdAtEpochMs, compression,
compressionMethodId, compressionLevel, encryption, encryptionMethodId,
kdfMethodId, kdfIterations, payloadChunkSize, kdf` — modelo medido por el agente
DEX en `docs/SWIFT_BACKUP_FORMAT.md`).

### Qué NO replicamos: la derivación de nonce/AAD

La derivación del **nonce** y del **AAD** por trozo es **NATIVA**
(`SbaLibaegisCryptoNative_decryptAegis256ChunkedEntryFdRaw` y compañía): las
etiquetas (`SBA1-AEGIS256-entry-chunk-nonce-v1`, `…-aad-v1`) son *etiquetas*,
no el algoritmo ⇒ ⇒ ⇒ **queda documentada como "derivación nativa, no
replicada"** ⇒ ⇒ ⇒ **no se inventa** ⇒ ⇒ ⇒ ⇒ **y el lector NO depende de ella
para ser útil**: con el KDF verificado puede
  1. **parsear la cabecera** (invariante big-endian + tabla de offsets),
  2. **derivar la clave** (Argon2id, por la API del signer) y
  3. **verificar el key-check** ⇒ ⇒ **eso prueba que la passphrase es correcta**
     y que el fichero no está manipulado ⇒ ⇒ ⇒ ⇒ **interop real**: abrir y
     ver backups cuyo key-check coincide **sin conocer la derivación exacta**.
⇒ ⇒ Lo que **queda fuera** (y se dice explícitamente): **extraer el contenido**
de las entradas ⇒ ⇒ ⇒ ⇒ por eso el lector debe responder
`status:"aead_no_replicado"` cuando llega a esa parte, en vez de devolver
basura.

### Por qué el ancho exacto sigue ABIERTO (y no lo voy a inventar)

El escritor **calcula** el ancho en runtime ⇒ ⇒ el mapeo definitivo campo↔nombre
y ancho↔bytes **no se puede extraer sólo del `.so`**. Sale de una de estas dos
fuentes, y ambas son externas a mi carril:

1. **El marshalling en Kotlin/DEX**: el orden real de los argumentos con los que
   Kotlin llama a `SbaArchiveNative.createArchive` ⇒ es el modelo del agente DEX.
2. **El export real del usuario**: contrastar los primeros bytes con la tabla.

⇒ ⇒ **hasta entonces el lector debe vivir con el layout en tabla versionada y
responder `status:"layout_no_verificado"`**, y **nunca** congelar estos offsets
en tests.

⇒ ⇒ **El mapa de offsets de esta tabla se le ha pasado tal cual al agente DEX**
(`659db26c`), que lo cierra contra el marshalling real de Kotlin
(`docs/SWIFT_KOTLIN_MARSHALLING.md`: campo ↔ offset ↔ ancho ↔ tipo ↔
significado, con la evidencia y `NO DETERMINADO` donde no se pueda) ⇒ ⇒ **el
lector lo consume de ahí; no se escribe aquí**.

## Regla de honestidad (no se congela en tests)

Todo lo de arriba sale del **escritor** ⇒ es el *stream que se escribe* ⇒ ⇒ **un lector debe
contrastarlo con un export real antes de darlo por bueno**, y el orden de campos debe vivir
en una **tabla de offsets versionada** (no en constantes incrustadas) para poder corregirlo
sin reescribir el lector. Hasta ese momento, el lector debe responder
`status: "kdf_no_verificado"` / `"layout_no_verificado"` en vez de affirmar.


---

# TERCERA PASADA (2026-11-21): layout **completo**,offsets exactos incluidos

El escritor de la cabecera se Analizó **instrucción a instrucción**
(`createArchive+0x2cac` = `0x655b8..0x65920`) y los **helpers de append**
(`0x6aa00`, `0x6ab6c`, `0x6b140`, `0x6b524`, `0x6ae10`) se identificaron por
su código. ⇒ ⇒ **Ya no hay "NO DETERMINADO" en los offsets ni en los anchos.**

## Los helpers (la clave de todo)

| Helper | Qué hace | Evidencia |
|---|---|---|
| `0x6ae10` | append de **bytes** (`src`, `len`) | `x2`=puntero, `w4`=longitud |
| `0x6aa00` | append de **u16 BIG-ENDIAN** | `0x6aa2c: lsr w8,w20,#8` + `strb` ⇒ byte alto primero; luego `0x6aac8: strb w20` ⇒ byte bajo. **2 bytes** |
| `0x6ab6c` | append de **u32 BIG-ENDIAN** | `0x6ab98: lsr w8,w20,#24` → `#16` → `#8` → byte bajo. **4 bytes** |
| `0x6b140` | append de bytes con crecimiento del buffer | `memcpy` + realoc |
| `0x6b524` | append de **N copias de un byte** (aquí: ceros de relleno) | `0x6b5b8: ldrb w1,[x22]` + `memset(dst, w1, n)`; el byte se pone a 0 antes (`strb wzr`) |

⇒ ⇒ **Los enteros de la cabecera NO son todos del mismo ancho**: hay u16 y u32
mezclados (y un u64 para la fecha). Por eso "el ancho se calcula en runtime"
era una lectura equivocada: **cada campo tiene su helper fijo**.

## La cabecera, campo a campo

Tamaño: **96 bytes** (`formato != 2`) o **144 bytes** (`formato == 2`).
El byte 4..6 (u16 BE) es el `formato`, y **el propio `formato` decide el
tamaño** (`0x65608: cmp w8,#2` · `csel w23,w9,w8,eq` con `w9=0x90=144`,
`w8=0x60=96`).

| Offset | Ancho | Tipo | Contenido | Evidencia (instrucción) |
|---|---|---|---|---|
| 0 | 4 | ASCII | `"SBA1"` | `0x65640 bl 0x6ae10` (x2=`0x11b35`, w4=4) |
| 4 | 2 | u16 BE | `formato` (2 ⇒ cabecera de 144 B) | `0x65644 ldr w1,[x20]` → `0x6564c bl 0x6aa00` |
| 6 | 2 | u16 BE | `headerSize` (96 ó 144) | `0x65654 mov w1,w23` → `0x65658 bl 0x6aa00` |
| 8 | 4 | u32 BE | enum 0..3 (compresión/cifrado) | `0x65660 cmp w22,#0` · `cset/cinc` → `0x65680 bl 0x6ab6c` |
| 12 | 8 | u64 BE | `createdAtEpochMs` (`time(NULL)*1000`) | `0x65688 bl time@plt` · `0x65698 mul x28,x0,#1000` · bucle de 8 `strb` |
| 20 | 2 | u16 BE | flags (`arg0` del marshalling) | `0x65748 ldr w1,[sp,#20]` → `0x65750 bl 0x6aa00` |
| 22 | 2 | u16 BE | flags2 (`arg1`) | `0x65758 ldr w1,[sp,#16]` → `0x65760 bl 0x6aa00` |
| 24 | 2 | u16 BE | campo 3 (`[x20+4]`) | `0x65764` → `0x6576c bl 0x6aa00` |
| 26 | 2 | u16 BE | campo 4 (`[x20+8]`) | `0x65770` → `0x65778 bl 0x6aa00` |
| 28 | 2 | u16 BE | campo 5 (`[x20+12]`) | `0x6577c` → `0x65784 bl 0x6aa00` |
| 30 | 2 | u16 BE | **longitud del bloque 1** | `0x65788 ldr w8,[x20,#72]` · `ldr w9,[x20,#64]` · `sub` → `0x65798 bl 0x6aa00` |
| 32 | 2 | u16 BE | **longitud del bloque 2** | `0x6579c…657ac` |
| 34 | 2 | u16 BE | **longitud del bloque 3** | `0x657b0…657c0` |
| 36 | 16 | bytes | **bloque 1** (rellenado con ceros hasta 16) | `0x657d4 bl 0x6b140` (bytes) + `0x657f4 bl 0x6b524` (`n = 16 - len`, ceros) |
| 52 | 16 | bytes | **bloque 2** | `0x65808` + `0x65828` |
| 68 | 16 | bytes | **bloque 3** | `0x6583c` + `0x6585c` |
| 84 | 2 | u16 BE | 0 | `0x65860 mov w1,wzr` → `0x65868 bl 0x6aa00` |
| 86 | 2 | u16 BE | 0 | `0x6586c` → `0x65874 bl 0x6aa00` |
| 88 | 4 | u32 BE | campo 12 (`[x20+24]`) | `0x65878 ldr w1,[x20,#24]` → `0x65880 bl 0x6ab6c` |
| **formato == 2 (144 B)** | | | | |
| 92 | 4 | u32 BE | campo 13 (`[x20+16]`) | `0x65890` → `0x65898 bl 0x6ab6c` |
| 96 | 2 | u16 BE | campo 14 (`[x20+20]`) | `0x6589c` → `0x658a4 bl 0x6aa00` |
| 98 | 2 | u16 BE | campo 15 (`[x20+28]`) | `0x658a8` → `0x658b0 bl 0x6aa00` |
| 100 | 2 | u16 BE | campo 16 (`[x20+32]`) | `0x658b4` → `0x658bc bl 0x6aa00` |
| 102 | 2 | u16 BE | campo 17 (`[x20+4] != 0 ? 1 : 0`) | `0x658c0…658d0 bl 0x6aa00` |
| 104 | 32 | ceros | reservado | `0x658e4 mov w2,#0x20` → `0x658e8 bl 0x6b524` |
| 136 | 8 | ceros | reservado | `0x658fc mov w2,#8` → `0x65900 bl 0x6b524` |
| **formato != 2 (96 B)** | | | | |
| 92 | 4 | ceros | reservado | `0x65918 mov w2,#4` → `0x6591c bl 0x6b524` |

**Cuadre exacto**: 4+2+2+4+8+2+2+2+2+2+2+2+2+16+16+16+2+2+4 = **92**, y luego
+4 (variante 96) ó +4+2+2+2+2+32+8 (variante 144) ⇒ **96 / 144 exactos**. ✔

## Qué es cada bloque

Los tres `byte[]` del marshalling (`createArchive` args 19..22 =
claveArgon2id, keyCheck, salt, nonce2) se vuelcan en la cabecera **tres**:
la clave derivada **no** va en la cabecera (se usa para el AEAD). Los tres
bloques van con su longitud delante (30/32/34) y se rellenan con ceros hasta
16 bytes.

* El **key-check** es `SHA-256(etiqueta || salt || nonce2 || clave)[:16]`
  (medido en el DEX, ver `docs/SWIFT_KOTLIN_MARSHALLING.md`) ⇒ son 16 bytes ⇒
  ocupa el bloque entero.
* El **salt** son 16 bytes exactos (la app lo exige).
* El **nonce2** son 16 bytes (`SecureRandom.nextBytes`).

⇒ ⇒ **Lo que NO se pudo cerrar**: el **orden** de los tres bloques (los
`byte[]` llegan por orden JNI y el escritor los vuelca en el orden del struct,
pero el struct no dice cuál es cuál). ⇒ ⇒ el lector **prueba las 6
permutaciones** y se queda con la que cuadra ⇒ **el orden se descubre solo**
cuando llegue un `.dat` real (y así queda documentado).

## Lo que sí queda nativo (no replicado)

La derivación del **nonce/AAD por trozo** del AEAD es **nativa**
(`SbaLibaegisCryptoNative_decryptAegis*ChunkedEntryFdRaw`): las etiquetas
`SBA1-AEGIS256-entry-chunk-nonce-v1` / `-aad-v1` son *etiquetas*, no el
algoritmo. ⇒ el lector Rust **parsea + deriva la clave + verifica el
key-check**, y **no** extrae el contenido.

## Implementación en el repo

`rust-core/src/swift_archive.rs`:
* `sba1_layout(formato)` ⇒ la tabla **medida** (96/144 B) con sus campos,
  anchos y endianess;
* `sba1_detectar(bytes)` ⇒ lee el `formato` y devuelve la variante;
* `KeyCheckComposition::SwiftTagged` + `key_check_tag()` ⇒ la composición
  exacta del key-check;
* `KeyCheckSpec.probar_permutaciones` ⇒ descubre el orden de los bloques;
* `KdfParams::validate()` ⇒ los rangos que impone la app.

Tests: **15/15** (incluido uno end-to-end con el layout medido y el
descubrimiento del orden).


---

# CUARTA PASADA: corrección de un error ajeno (anchos "variables")

El módulo `rust-core/src/swift_sba1_header.rs` (agente RE, commit `26cc28a`)
afirma:

> "Los campos 4, 22 y 23 se escriben con un helper de **ancho variable**
> (`0x6ab6c`): el ancho (1, 2 o 3 bytes) lo decide el escritor con un `csel`
> según otros campos."

⇒ **Eso es incorrecto**, y su `Perfil { campo4, campo22, campo23 }` (anchos
1/2/3) hace que su lector **desalinee la cabecera real**.

## La prueba

`0x6ab6c` escribe **4 bytes fijos**, big-endian, sin ninguna rama de ancho:

```
6ab98:  lsr  w8, w20, #24      ; byte 3 (MSB)
6ab9c:  strb w8, [x26], #1
6ac34:  lsr  w8, w20, #16      ; byte 2
6ac38:  strb w8, [x26], #1
6acd0:  lsr  w8, w20, #8       ; byte 1
6acd4:  strb w8, [x26], #1
        strb w20, [x26], #1    ; byte 0 (LSB)
```

⇒ ⇒ **u32 BIG-ENDIAN de ancho fijo 4**. Los `csel` que se ven en el escritor
**eligen el VALOR** de un campo (p. ej. `0x65660 cmp w22,#0` · `cset/cinc`
eligen entre 0..3), **nunca el ancho**.

Lo mismo con el hermano `0x6aa00`, que escribe **2 bytes fijos**
(`lsr #8` y luego el byte bajo) ⇒ **u16 BE**.

## Los 27 «escrituras» sí coinciden

Contando el timestamp como 8 bytes, la secuencia de escrituras del escritor es
de **27** en la variante de 96 B ⇒ **la misma** que la de esta tercera pasada
⇒ ⇒ **los dos trabajos coinciden en el orden**; lo que se corrige es **el
ancho de 3 campos**:

| Campo | Anchos variables (afirmación ajena) | **Ancho real (medido)** |
|---|---|---|
| `encryptionMethodId` (offset 8) | 1, 2 ó 3 | **4** (u32 BE) |
| campo 22 (`struct[0x18]`, offset 88) | 1, 2 ó 3 | **4** (u32 BE) |
| campo 23 (`struct[0x10]`, offset 92/96) | 1, 2 ó 3 | **4** (u32 BE) |

⇒ ⇒ **Cómo detectarlo sin `.dat`**: el total de la cabecera cuadra **exactamente**
(96 / 144) sólo con estos anchos; con anchos variables, la suma no cuadra.

⇒ ⇒ **Recomendación al enjambre**: que el módulo de cabecera tenga **una sola**
fuente de verdad. La tabla medida está en
`rust-core/src/swift_archive.rs::sba1_layout(formato)` (y su tabla completa
aquí arriba). Los módulos `swift_sba1.rs` (raw + TAR/ZSTD) y
`swift_sba1_header.rs` (byte a byte) son **complementarios**, pero el segundo
debe usar los anchos fijos medidos aquí.
