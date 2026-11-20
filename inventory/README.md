# W2 · Inventario del Samsung S21 Ultra (Exynos 2100)

**Versión del formato:** `w2.1` · **Generado:** 2026-07-20 · **Fuente:** el móvil (root vía MCP `root_exec`)

## Qué es

`inventory.json` es el inventario **versionado** del dispositivo, con el SHA-256 de
cada fichero de referencia. Es la **entrada de W3 (backup)** y la **línea base de
W4 (migración de ROM)**: sin saber qué hay, no se sabe qué respaldar ni qué comparar.

## Contenido

| Sección | Qué lleva |
|---|---|
| `hardware` | SoC (Exynos 2100 / `s5e9840` / `universal2100_r`), CPU (núcleos, modelo), GPU (EGL, relojes), NPU (`/dev/vertex10`), RAM+swap, pantalla, sensores, cámaras, módem |
| `software` | ROM (versión, SDK, parche, **CSC `AMO`**, fingerprint), kernel, SELinux, Magisk, root, módulos, perfiles de usuario (work profile) |
| `boot_y_particiones` | Tabla de particiones, `/dev/block/by-name`, props de boot, **AVB** (`verifiedbootstate`, `device_state`, `flash.locked` ⇒ bootloader cerrado, hash alg), **TEE** y keymaster |
| `apps` | 1719 paquetes: ruta del APK, versionCode/Name, min/target SDK, **hash de firma** (truncado a 4 bytes por la plataforma, documentado tal cual), si es de sistema, si está habilitada, permisos de riesgo |
| `red` | Interfaces, rutas, DNS, proxy, VPN, redes guardadas y claves ADB autorizadas (**hasheadas**) |
| `cuentas` | **Sólo presencia y estado**: tipo de cuenta, número y hash del nombre. **Nunca tokens** (`tokens_included: false`) |
| `espacio` | Mapa de uso por directorio y por sistema de ficheros |
| `referencias` | Un item por fichero de referencia: `sha256`, `bytes`, `visibilidad` |

## Privacidad (el repo es público)

- Los **SSID/BSSID**, las **claves ADB autorizadas** y los **nombres de cuenta** van
  **hasheados (sha16)**: en el manifiesto público, la ruta absoluta de un fichero
  privado se sustituye por `privado:<sha16 de la ruta>`.
- La **contraseña WiFi no se extrae**: sólo se cuenta cuántas redes la tienen.
- **Cero tokens, cero claves privadas, cero material de firma.**
- La copia **privada** (con los ficheros de referencia en claro) va a **Drive**.

## Cómo se regenera

```bash
# desde el repo, con target en disco local (el workspace es NFS y va lento)
cargo run --release --bin md-device-inventory -- \
    --secrets /workspace/md-secrets/instance.env --out <salida>

# reensamblar desde una captura ya hecha, SIN tocar el móvil
cargo run --release --bin md-device-inventory -- --from-refs <salida> --out <salida>
```

El driver **trae cada fichero por trozos de 256 KB** (el móvil va al 98 % de disco) y
**aborta si el SHA-256 del móvil no cuadra con el local**. La verificación está en
`rust-core/src/device_inventory.rs` y los tests en `rust-core/tests/device_inventory_smoke.rs`.
