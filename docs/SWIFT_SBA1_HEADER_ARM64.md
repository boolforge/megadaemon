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

## Regla de honestidad (no se congela en tests)

Todo lo de arriba sale del **escritor** ⇒ es el *stream que se escribe* ⇒ ⇒ **un lector debe
contrastarlo con un export real antes de darlo por bueno**, y el orden de campos debe vivir
en una **tabla de offsets versionada** (no en constantes incrustadas) para poder corregirlo
sin reescribir el lector. Hasta ese momento, el lector debe responder
`status: "kdf_no_verificado"` / `"layout_no_verificado"` en vez de affirmar.
