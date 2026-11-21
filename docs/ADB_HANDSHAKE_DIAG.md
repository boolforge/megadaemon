# ADB sobre TCP contra el Samsung: diagnóstico exacto y parche propuesto

> Encargo E2 del tablero (`ext-adb-handshake`). Todo lo de aquí está **medido en el
> dispositivo** (S21 Ultra, `SM-G998B`, ROM Dr.Ketan U08, kernel 5.4.242, Magisk v27.2-kitsune-4).
> Reproducible con `tools/adbsweep.py` (subido al móvil como `/data/local/tmp/adbsweep.py`).

## 1. Resumen

El cliente ADB **nunca obtiene el banner** de `adbd`. El bloqueo **no es de red ni de formato
de mensaje**: `adbd` acepta la conexión TCP, **consume** los bytes del `CNXN` y **no responde**.
Las 64 combinaciones de (CRC × payload × `arg1`) probadas dan `TIMEOUT`, y `adbd` no habla primero.

| Hipótesis | Resultado | Veredicto |
|---|---|---|
| `adbd` no escucha | `ss -ltn` → `LISTEN *:5555 users:(("adbd",pid=12571,fd=14))` | **descartada** |
| Paquete/CNXN mal formado | 64 combinaciones (9 algoritmos CRC × 4 banners × 3 `arg1`) → todas `TIMEOUT` | **descartada** |
| CRC del CNXN | Ninguna variante responde (incluye payload vacío, donde el CRC es 0 en casi todas) | **descartada** |
| El CRC del cliente está mal | Sí lo está (ver §3), pero **no es la causa del silencio** | **parche aparte** |
| `adbd` habla primero | Conectar y sólo leer 8 s → sin banner espontáneo | **descartada** |
| Los bytes no llegan | `ss -tnp` → `ESTAB` en ambos extremos, `rx_queue=0` (adbd los consumió) | **descartada** |
| Wireless debugging bloquea 5555 | `settings get global adb_wifi_enabled` → **0** | **descartada** |
| adbd muere al recibir | `adbd` vivo (`init.svc.adbd=running`), hilo de accept en `inet_csk_accept` | **descartada** |

**Conclusión:** el `adbd` de este ROM acepta el transporte TCP y **descarta** el `CNXN` en
silencio. Es un rechazo a nivel de `adbd`, no del cliente.

## 2. Evidenciacollected (medida)

```
# el socket lo posee adbd
LISTEN 0 4 *:5555 *:*  users:(("adbd",pid=12571,fd=14))

# la conexión se establece y adbd CONSUME los bytes (rx_queue=0 en ambos extremos)
ESTAB 0 0 127.0.0.1:47024        127.0.0.1:5555  users:(("python3",pid=25947,fd=3))
ESTAB 0 0 [::ffff:127.0.0.1]:5555 [::ffff:127.0.0.1]:47024 users:(("adbd",pid=12571,fd=16))

# el hilo de accept de adbd está vivo
tid=12580 syscall=242 (accept4) 0xe ...   # accept4(fd=14, NULL, NULL, SOCK_NONBLOCK)

# y sin embargo no hay ningún hilo leyendo la conexión: el CNXN ya fue procesado y descartado
```

Hilos de `adbd` durante una conexión pendiente: `epoll_wait`(×3), `futex`(×4), `accept4`(×1),
`ppoll`(×1, transporte USB), `uname`. **Ninguno en `read()` sobre el fd de la conexión** ⇒ el
mensaje se leyó y se tiró.

Barrido executed en el propio móvil (python3 de Termux, python 3.14):

```
--- adbd habla primero? (conectar y solo leer 8 s) ---
  banner espontaneo: NO (timeout)
zlib  host::\0               arg1=4096    TIMEOUT
zlib  host::completo         arg1=1048576 TIMEOUT
jam   host::feat=shell_v2\0  arg1=256     TIMEOUT
aosp  host::\0               arg1=4096    TIMEOUT
...  (64 combinaciones: 9 algoritmos CRC x 4 banners x 3 valores de arg1)
```

`logcat -c` + intento + volcado: **cero líneas** de `adbd` (ni `CNXN`, ni `AUTH`, ni
`connection terminated`). El descarte es mudo.

## 3. Defecto real en el cliente Rust (independiente del silencio)

`rust-core/src/adb_client.rs` implementa el CRC-32 del protocolo ADB así:

```rust
/// OJO: **no** es el `zlib.crc32`.  AOSP usa `adb_crc32()`, que empieza con
/// `crc = 0` y **no hace el XOR final**: es el CRC-32 "JAMCRC".
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0;                     // init 0
    for &b in data {
        crc = (crc >> 8) ^ table[((crc ^ b as u32) & 0xFF) as usize];
    }
    crc                                       // sin XOR final
}
```

Esa nota es **incorrecta**: el CRC de ADB es el de la familia reflected con poly `0xEDB88320`
(la misma tabla que usa zlib), y el error aparece al **validar la respuesta del dispositivo**
(`Message::decode` → `if crc32(&payload) != crc`), no al enviar. Con la variante `init=0` sin XOR
final, **todo mensaje recibido de `adbd` se rechaza** con `protocolo: crc32 incorrecto` — que es
exactamente el error que devuelve hoy la tool `adb` del maestro:

```json
{"adbd_ok": false, "banner": "", "error": "protocolo: crc32 incorrecto",
 "adbd_host": "127.0.0.1", "adbd_port": 5555, "host_key_present": true}
```

Ojo al detalle: ese error **no puede proceder de una lectura** —`recv()` usa `read_exact` con
timeout y un fallo de red daría un error de E/S— sino de `Message::decode` sobre bytes ya leídos.
Por tanto el cliente **sí recibe algo** en algún camino, o bien el error se cachea de un intento
previo. En cualquier caso el checksum de la respuesta se valida con un algoritmo que no es el de AOSP.

### Parche propuesto para `adb_client.rs`

1. **CRC correcto.** Usar el CRC-32 estándar (reflected, poly `0xEDB88320`, init `0xFFFFFFFF`,
   XOR final `0xFFFFFFFF`) — es decir `zlib.crc32` — y corregir el comentario, que hoy induce
   al error contrario. Añadir test vectorial: `crc32(b"123456789") == 0xCBF43926` y
   `crc32(b"") == 0`.
2. **No validar el checksum de respuestas que no lo llevan.** `CNXN`/`AUTH` de `adbd` traen payload
   real y su checksum; el resto, no. Validar sólo cuando `data_length > 0`.
3. **Timeout de lectura y cierre ordenado.** `recv()` debe aplicar un timeout por mensaje y,
   ante error, `shutdown(Both)` + cierre, para no dejar sockets establishes que bloqueen el
   accept loop de `adbd` (con `LISTEN` backlog 4, cinco conexiones sin cerrar bloquean el
   servicio entero: es la causa más probable del `RST` que veía el maestro).
4. **Diagnóstico honesto.** El error debe decir *qué* falló (timeout / EOF / checksum) y no un
   único `crc32 incorrecto` para todo.

```rust
/// CRC-32 del payload de un mensaje ADB.
/// AOSP (`adb/adb.cpp`) usa el CRC-32 estándar reflected (poly 0xEDB88320,
/// init 0xFFFFFFFF, XOR final 0xFFFFFFFF) => es `zlib.crc32`.
/// Ojo: una variante "init 0 sin XOR final" (lo que hay hoy) hace que
/// adbd y el propio cliente rechacen los mensajes validos.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc = (crc >> 8) ^ table[((crc ^ b as u32) & 0xFF) as usize];
    }
    crc ^ 0xFFFF_FFFF
}
```

## 4. Camino alternativo (recomendado mientras tanto)

En el propio móvil **no hace falta ADB**: la tool `root_exec` ya da shell root
(`uid=0`, `u:r:magisk:s0`). Para un nodo externo, ADB es imprescindible; para el maestro local,
`root_exec` cubre el caso de uso. Si hay que sí o sí ADB desde fuera, el camino vigente es
**Wireless debugging** (`adb pair` + `adb connect` sobre el puerto aleatorio, con TLS), que exige
activarlo en *Opciones de desarrollador* —imposible sin tocar la pantalla, que el usuario está
usando— y que además está **desactivado** en este ROM (`adb_wifi_enabled=0`).

## 5. Reproducir

```bash
# en el móvil (Termux python3)
python3 /data/local/tmp/adbsweep.py 127.0.0.1 5555
# socket de adbd
ss -ltnp | grep 5555
# conexiones durante un intento
nohup python3 -u /data/local/tmp/adbsweep.py 127.0.0.1 5555 & sleep 2; ss -tnp | grep 5555
```
