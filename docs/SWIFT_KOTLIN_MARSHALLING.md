# Swift Backup · marshalling real de la cabecera `SBA1` (Kotlin → nativa)

> **Aviso primero (importante)**: el lector de DEX del repo (`rust-core/src/dex_decompile.rs`
> y `tools/re/dexorigins.py`) tenía **tres bugs reales** que lo hacían
> producir basura (desensamblados con casi todo `nop`). Todo lo de este
> documento se ha **re-medido después de arreglarlos** (ver §7).
> ⇒ **lo que se haya deducido antes con esas herramientas hay que volver a
> mirarlo**: este documento es la fuente buena.
>
> **Cuarto bug (2026-11-21, agente RE)**: `read_u16` del lector **leía los
> u16 big-endian**, pero el formato DEX es **little-endian** ⇒ los
> `class_idx`/`type_idx` de `field_ids`/`method_ids`, las `type_list` y los
> tamaños del `code_item` salían byte-swapeados ⇒ **cada método se atribuía a
> la clase equivocada**. Con el fix: **32.695 métodos, 0 mal atribuidos**
> (identico al oráculo independiente en Python).
> ⇒ ⇒ **TODO lo de este documento ha sido re-medido con el lector ya
> arreglado** (firma JNI, modelo, etiquetas, composición del key-check,
> parámetros del KDF, orden de los argumentos): **coincide** ⇒ lo publicado
> se sostiene.

## 1. De dónde sale

* APK de Swift Backup 5.1.1; sus **DEX son byte a byte los del móvil**:
  `classes.dex` `1db45566d56d9b26da945b1ac04fea8a3075cd6adba4727dfce27796f272a7fd` (9.273.900 B),
  `classes2.dex` `6bb33ae5c983c7e7e1f2f11048892dd44b7a9c0368db7d74211a07a729036844` (2.844.984 B).
* La app **no** está ofuscada en su propio paquete (`com.swiftapps.sba.*` conserva
  los nombres); lo ofuscado con R8 es la biblioteca.
* Herramientas: el lector DEX del repo (arreglado) y `tools/re/dexorigins.py`
  (también arreglado). Todo el RE es **en local**: el móvil no hace falta.

## 2. La llamada nativa: el marshalling

```java
// Lcom/swiftapps/sba/SbaArchiveNative; (método nativo JNI)
long createArchive(
    int,                        // 1
    java.io.FileDescriptor,      // 2
    byte[],                     // 3
    byte[][],                   // 4
    String,                     // 5  <- passphrase
    int[],                      // 6
    String[][],                  // 7
    String[][],                  // 8
    int, int, int, int, int, int, int, int, int, int,   // 9..18
    byte[], byte[], byte[], byte[],                      // 19..22
    SbaNativeProgressListener   // 23
);
```

Firma real medida (`method_idx 0x624E` = M[25150], re-verificada tras el fix del `read_u16`):

```
createArchive(ILjava/io/FileDescriptor;[B[[B[Ljava/lang/String;[I[[Ljava/lang/String;
              [[Ljava/lang/String;IIIIIIIIII[B[B[B[B
              Lcom/swiftapps/sba/internal/progress/SbaNativeProgressListener;)[J
```

**Call site**: `Ldy6;->f0(...)` (el "crear archivo" de la app) ⇒
`invoke-virtual/range {v2..v25}` ⇒ `v2` = `this` y `v3..v25` = los 23 argumentos.
Trazando los `move*` inmediatamente anteriores a la llamada:

| # | Tipo | De dónde sale (medido) |
|---|------|------------------------|
| 1 | `int` | `v6` (un valor del modelo) |
| 2 | `FileDescriptor` | `v50` (el fichero de salida) |
| 3 | `byte[]` | `v55` |
| 4 | `byte[][]` | `v25` (los datos de las entradas) |
| 5 | `String` | `v12` = **la passphrase** |
| 6 | `int[]` | `v8` (construido en el bucle de entradas) |
| 7 | `String[][]` | `v9` |
| 8 | `String[][]` | `v10` |
| 9..18 | `int` ×10 | `v11..v20` (constantes + campos del modelo) |
| 19 | `byte[]` | `v21` = **clave derivada** (`Lg87;.a`) |
| 20 | `byte[]` | `v22` = **key-check** (`Lg87;.b`) |
| 21 | `byte[]` | `v23` = **salt** (`Lg87;.c`) |
| 22 | `byte[]` | `v24` = **segundo bloque aleatorio de 16 B** (`Lg87;.d`) |
| 23 | listener | `v25` |

⇒ ⇒ **los 3 bloques de bytes de la cabecera son `salt`, `keyCheck` y el segundo
bloque aleatorio de 16 B** (la clave derivada **no** va en la cabecera: se pasa
aparte). Esto encaja con lo medido en el escritor ARM64 del `.so`, que lee
**tres** arrays de bytes del struct (`+0x40/+0x48`, `+0x58/+0x60`, `+0x70/+0x78`).

## 3. El KDF (Argon2id) — entrada exacta

`Ly87;.a([C[BIIII)[B` valida y luego llama a la nativa:

```java
// sal: EXACTAMENTE 16 bytes  (array-length == 16)
// iteraciones: 1..100
// memoria:     16384..65536 KiB
// paralelismo: 1..8
// salida:      > 0
byte[] pass = utf8(chars);
byte[] key  = SbaNativeCrypto.deriveArgon2id(pass, salt, iterations, memKiB, parallelism, outLen);
```

⇒ ⇒ **coincide exactamente** con la API canónica del repo
(`crate::argon2::derive_key(passphrase, salt, iterations, memory_kib, parallelism, out_len)`,
Argon2id RFC 9106, verificado contra el oráculo C): **secret vacío y AD vacío**.
⇒ ⇒ el lector Rust no necesita cambiar el KDF: sólo **validar los rangos**
(hay un `KdfParams::validate()` nuevo, con los límites que impone la app).

## 4. El key-check — composición exacta (¡ya no es una suposición!)

`Ly87;.d(sc7;[B[B[B)[B`:

```java
MessageDigest md = MessageDigest.getInstance("SHA-256");
md.update(etiqueta);   // según el método de cifrado
md.update(salt);
md.update(nonce2);      // el segundo bloque aleatorio de 16 B
md.update(clave);      // la clave derivada con Argon2id
return Arrays.copyOf(md.digest(), 16);   // TRUNCADO a 16 bytes
```

**Etiquetas de dominio** (`Ly87;.<clinit>`, `static final byte[]`):

| Enum `sc7` (método de cifrado) | Etiqueta |
|---|---|
| `SevenZipAes` | `SBA1-7zAES-key-check-v1` |
| `Aes256Gcm` | `SBA1-AES256GCM-key-check-v1` |
| `Aes256GcmSiv` | `SBA1-AES256GCMSIV-key-check-v1` |
| `Aegis256` | `SBA1-AEGIS256-key-check-v1` |
| `Aegis128X2` | `SBA1-AEGIS128X2-key-check-v1` |

(Otras etiquetas de la misma clase, para otros usos:
`SBA1-payload-hmac-key-v1`, `SBA2-index-mac-key-v1`,
`SBA2-index-metadata-mac-v1`.)

⇒ ⇒ **El lector ya replica el key-check exacto**
(`KeyCheckComposition::SwiftTagged` + `key_check_tag()`), con el truncado a 16 B.
⇒ ⇒ Con la **passphrase + salt + nonce2** de la cabecera se puede **comprobar
que la passphrase es correcta y que el fichero no está manipulado** ⇒ es
interop real, no un "casi".

## 4bis. El campo `e` del modelo = **clave HMAC del payload** (nuevo)

El constructor medido del modelo (`Lg87;.<init>([B[B[B[B)V`) es:

```java
a = arg0;  b = arg1;  c = arg2;  d = arg3;
e = Ly87;.c(a, c, d);        // <- el 5o campo, DERIVADO
```

y `Ly87;.c([B[B[B)[B` es:

```java
MessageDigest md = MessageDigest.getInstance("SHA-256");
md.update(tag);    // Ly87;.f = "SBA1-payload-hmac-key-v1"
md.update(arg1);   // el 2o argumento
md.update(arg2);   // el 3o argumento
md.update(arg0);   // el 1o argumento
return md.digest();             // SIN truncar (32 B)
```

⇒ ⇒ **La clave HMAC del payload tiene EXACTAMENTE la misma forma que el
key-check** (`SHA-256(etiqueta || salt || nonce2 || clave)`), pero con **otra
etiqueta** (`SBA1-payload-hmac-key-v1`) y **sin truncar**:

| Derivación | Etiqueta | Salida |
|---|---|---|
| key-check | `SBA1-<cifrado>-key-check-v1` | 16 B (truncado) |
| **clave HMAC del payload** | `SBA1-payload-hmac-key-v1` | 32 B (completo) |

⇒ ⇒ el campo `e` del modelo es la clave HMAC del payload ⇒ ⇒ **ya se puede
calcular en Rust** (`PAYLOAD_HMAC_TAG` en `rust-core/src/swift_archive.rs`) ⇒
sirve para **autenticar el contenido** cuando se replique el AEAD por trozos.

## 4ter. La **cadena completa**, con la evidencia del call site

`Ldy6;->f0` (M[27206], 64 regs), desensamblado con el lector ya arreglado:

```text
019d: const/16 v2, #16
019f: new-array  v13, v2, [B          ; 16 B
01a1: new-array  v15, v2, [B          ; 16 B
01a8: SecureRandom.nextBytes(v13)     ; v13 = SAL
01ab: SecureRandom.nextBytes(v15)     ; v15 = NONCE2 (2o bloque aleatorio)
01ae: const/16 v32, #1                ; iteraciones = 1
01b0: const/16 v33, #32               ; salida = 32 B
01b2: const/16 v30, #3                ; paralelismo = 3
01b4: const/16 v31, #16384            ; memoria = 16384 KiB
01ba: invoke-static/range {v28..v33}, Ly87;.a([C[BIIII)[B   ; deriveKey(chars, v13, 1, 16384, 3, 32)
01bd: move-result-object v2           ; v2 = CLAVE derivada
01c0: sget-object v3, Lsc7;.Aegis256
01c2: invoke-static {v3, v2, v13, v15}, Ly87;.d(Lsc7;[B[B[B)[B
                                            ; d(metodo, clave, salt, nonce2) = KEY-CHECK
01ca: invoke-direct {v8, v2, v3, v13, v15}, Lg87;.<init>([B[B[B[B)V
                                            ; modelo(clave, keyCheck, salt, nonce2)
```

⇒ ⇒ **el orden de los argumentos queda PROBADO**, no inferido:
`d(metodo, clave, salt, nonce2)` y dentro `update(tag) → update(salt) →
update(nonce2) → update(clave)` ⇒ **keyCheck = SHA-256(tag || salt ||
nonce2 || clave)[:16]** ⇒ ⇒ y `Lg87;` = (a=clave, b=keyCheck, c=salt,
d=nonce2, e=clave HMAC del payload).

## 5. Lo que sigue **NO** determinado (y por qué)

| Pieza | Estado | Por qué |
|---|---|---|
| **Offset y ancho** de cada campo de la cabecera | ✅ **MEDIDO** | el escritor nativo se desensambló instrucción a instrucción (`0x655b8..0x65920`) y los helpers de append están identificados: `0x6aa00`=u16 BE, `0x6ab6c`=u32 BE, `0x6b524`=relleno de ceros. Tabla completa en `docs/SWIFT_SBA1_HEADER_ARM64.md` |
| **Orden** de los 3 bloques de bytes dentro de la cabecera | 🟡 | los `byte[]` llegan por orden JNI y el escritor los vuelca en el orden del struct, pero el struct no dice cuál es cuál ⇒ el lector **prueba las 6 permutaciones** y la que cuadra **descubre el orden** |
| Los 10 `int` del marshalling ↔ campos del modelo | 🟡 | hay constantes y ramas; el lector no los necesita para verificar el key-check |
| **Nonce/AAD por trozo** | ❌ **no replicado** | es **nativo** (`SBA1-AEGIS256-entry-chunk-nonce-v1` es una *etiqueta*, no el algoritmo) |

⇒ ⇒ El lector **ya tiene la tabla versionada y medida** (`sba1_layout(formato)`
en `rust-core/src/swift_archive.rs`): 96 B (formato ≠ 2) o 144 B (formato = 2),
con u16/u32/u64 big-endian donde toca. Lo que ya es exacto (KDF + key-check +
offsets) está en el **código y en tests**, no en vectores con números inventados.

## 6. Cómo se cierra lo que falta

1. **Un `.dat` real** del usuario (export de Swift Backup) ⇒ se contrasta el
   key-check de verdad y **el orden de los 3 bloques se descubre solo**
   (el lector prueba las 6 permutaciones y dice cuál cuadra).
2. Los offsets **ya están medidos** (tercera pasada del desensamblado) ⇒ lo
   que queda por confirmar con un fichero real es el **nombre** de los campos
   `campo3..campo17` y el orden de los bloques ⇒ nada de eso impide
   **verificar la passphrase**.

## 7. Los tres bugs del lector DEX (arreglados el 2026-11-21)

1. **Opcode**: en un DEX los *code units* se guardan **little-endian** y el
   primer **byte** de la instrucción es el opcode ⇒ con el *unit* leído como
   `u16`, el opcode está en el byte **BAJO** (`op | AA<<8`), igual que ART
   (`inst & 0xff`). El comentario anterior ("byte alto") estaba **equivocado**
   ⇒ el desensamblado era casi todo `nop` (y los vectores de test, construidos
   con el mismo error, pasaban ⇒ el test no ata nada).
2. **Payload de `encoded_value`**: **no** tiene ancho fijo por tipo ⇒ vale
   `value_arg + 1` bytes (un `INT` se codifica `0x64` ⇒ 4 bytes; un índice de
   tipo `0x37` ⇒ 2 bytes). `NULL` (`0x1e`) y `BOOLEAN` (`0x1f`) no tienen
   payload.
3. **Posición**: `read_u8` recibe el offset **por valor** ⇒ el payload se leía
   **sin avanzar** el `pos` ⇒ el parseo se quedaba atascado en el mismo byte.

Con los tres arreglados: **0 tipos desconocidos** y el último blob de
`static_values` cae justo en el límite de la sección (antes: 66 y 11).

## 8. Postura legal

Swift Backup es **propietario**. Aquí **no se ha copiado ni una línea de su
código**: sólo se han medido **bytes, cadenas, firmas JNI y el desensamblado**
de su binario para poder **leer** los backups del propio usuario (interop).
Lo que se reimplementa en MegaDaemon es **nuestro**: Argon2id (RFC 9106),
AEGIS-256 (RFC 10032), TAR, zstd y el key-check (SHA-256 con etiqueta de
dominio, todo composición estándar).
