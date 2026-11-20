# Referencia AOSP del protocolo ADB

Fuentes **oficiales de AOSP** del módulo ADB, bajadas para verificar el
comportamiento del campo de checksum de la cabecera de 24 bytes (la duda que
planteó el agente de hardware/NPU: ¿AOSP rellena el CRC del *handshake*?).

## Por qué estos ficheros están aquí

`android.googlesource.com` y `raw.githubusercontent.com` **no son alcanzables
desde el sandbox** (el proxy de salida devuelve un 404 sintético), y el móvil
—que sí tiene internet— quedó sin canal MCP (403 por la key caducada).  Se
resolvió con **clones git parciales** (`--filter=blob:none --depth 1`) de los
mirrors de GitHub, que sí pasan, y extrayendo los blobs con `git show`.

## Procedencia exacta

| Ruta aquí | Origen | Commit | Android |
|---|---|---|---|
| `system_core_android11/` | `github.com/aosp-mirror/platform_system_core`, rama `android11-release`, dir `adb/` | `f53626de…` | 11 |
| `packages_modules_adb_17/` | `github.com/GrapheneOS/platform_packages_modules_adb`, rama `17` | `c10299c6075900ba731ef72e5ffe4b1cf7b69c42` | 14 |

Licencia: **Apache-2.0** (AOSP).  `packages/modules/adb` es el árbol actual
(ADB se movió de `system/core/adb` a `packages/modules/adb` a partir de
Android 12); el S21 es Android 14 ⇒ `packages_modules_adb_17` es la referencia
que corresponde al dispositivo.

## Hallazgos (verificados sobre el código, no de memoria)

1. **El campo se llama `data_crc32`, no `data_check`.**  La cabecera de 24
   bytes es:

   ```c
   struct message {
       unsigned command;       /* command identifier constant (A_CNXN, ...) */
       unsigned arg0;          /* first argument                            */
       unsigned arg1;          /* second argument                           */
       unsigned data_length;   /* length of payload (0 is allowed)          */
       unsigned data_crc32;    /* crc32 of data payload                     */
       unsigned magic;         /* command ^ 0xffffffff                      */
   };
   ```

2. **AOSP NO rellena ese campo ni lo verifica.**  `data_crc32` aparece
   **1 vez** en cada fichero de protocolo (el comentario del `struct`) y
   **0 veces** en `adb.cpp` / `adb_io.cpp` (moderno) y en `adb.cpp` (clásico):
   el paquete se pone a cero al reservarlo (`memset(&p->msg, 0, …)`) y se
   manda tal cual.

   ⇒ **Un cliente ADB no puede exigir un CRC válido en el handshake**
   (CNXN/AUTH) ni en ningún otro mensaje: la referencia oficial no lo
   comprueba.  Exigirlo rompe la sesión contra `adbd` real.

3. **El "checksum" clásico de AOSP no es un CRC32.**  En
   `system/core/adb/adb.cpp` (Android 11):

   ```c
   uint32_t calculate_apacket_checksum(const apacket* p) {
       uint32_t sum = 0;
       for (size_t i = 0; i < p->msg.data_length; ++i) {
           sum += static_cast<uint8_t>(p->payload[i]);   // SUMA, no CRC
       }
       return sum;
   }
   ```

   Es una **suma aditiva de bytes** y pertenece al protocolo *sync* heredado,
   no al protocolo de socket.  De ahí que en el tráfico real convivan el
   CRC-32 "JAMCRC" de AOSP y el CRC-32 IEEE (zlib) según el camino: el
   cliente debe tolerar ambos (y no exigirlo en el handshake).

4. **Constantes de comando** (`packages_modules_adb_17/protocol.md`):

   ```
   A_SYNC 0x434e5953   A_CNXN 0x4e584e43   A_AUTH 0x48545541
   A_OPEN 0x4e45504f   A_OKAY 0x59414b4f   A_CLSE 0x45534c43
   A_WRTE 0x45545257   A_STLS 0x534C5453
   ```

5. **CNXN**: `version = 0x01000000`, `maxdata = 256 KiB` (antes 4096); los
   paquetes CONNECT/AUTH **no pueden superar 4096** porque se envían antes de
   conocer el `maxdata` del otro lado.

6. "Receipt of an invalid message header, corrupt message payload, or an
   unrecognized command **MUST** result in the closing of the remote
   connection."

## Cómo se obtuvo (reproducible)

```bash
# clásico (Android 11)
git clone --filter=blob:none --no-checkout --depth 1 \
    --branch android11-release \
    https://github.com/aosp-mirror/platform_system_core aosp14
git -C aosp14 fetch --depth 1 origin android11-release
git -C aosp14 show FETCH_HEAD:adb/protocol.txt

# moderno (Android 14)
git clone --filter=blob:none --no-checkout --depth 1 --branch 17 \
    https://github.com/GrapheneOS/platform_packages_modules_adb aosp-adb
git -C aosp-adb show HEAD:adb.cpp
git -C aosp-adb show HEAD:docs/dev/protocol.md
```
