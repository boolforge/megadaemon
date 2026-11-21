# Trazabilidad: de hallazgo a código (qué está integrado y qué falta)

> La regla de la flota: un hallazgo que no acaba en código (o en una tool) es un hallazgo a medias.
> Esta matriz es el contrato: cada fila dice **dónde vive** y **quién lo cierra**.

| Hallazgo | Dónde vive | Estado | Commit | Nota |
|---|---|---|---|---|
| A-04 fail-closed (P0) | `rust-core/src/api_key.rs :: decide() + load_with()` | **INTEGRADO** | `4bf9f34` | 3 tests nuevos; fail-closed + autogenera key con aviso sin secreto |
| D-09 poison recovery (P1) | `rust-core/src/task_scheduler.rs (18) + alerts.rs (6)` | **INTEGRADO** | `4bf9f34` | lock().unwrap_or_else(|p| p.into_inner()) |
| VS4L: 7/9 vivos + tabla | `rust-core/src/dex_decompile.rs :: VS4L_IOCTLS (VS4L_LIVE_CODES=7)` | **INTEGRADO** | `bd734f7` | doc §1.2 con JSON machine-readable (live por codigo) |
| Sonda VS4L correcta (sin argumento) | `rust-core/src/npu_vs4l.rs` + CLI `npu-probe --vs4l` + tool MCP `npu_vs4l_probe` | **INTEGRADO** | `c2b45d8` | emite el ioctl SIN argumento => el driver devuelve el size que espera (9 codigos, 7 vivos); `EINVAL`=existe, `ENOTTY`=no existe; ioctl portable por arquitectura (`5dc4934`) |
| OMC: decode/encode del cscfeature.xml | `rust-core/src/rom_migration.rs (Rust) + tools/omc_codec.py (Python)` | **INTEGRADO** | `2105911 / 3f45329` | round-trip verificado en 126/126 CSC del movil |
| CSC feature: **19/126 de fabrica** (20 con el patch Magisk) + patch de AMO | `rust-core/src/rom_migration.rs :: csc_feature_get/set + omc_feature_patch_plan()` | **INTEGRADO** | `2105911` | valor a escribir = 'RecordingAllowed' (no true); tag exacto |
| Consumidor runtime del CSC | `-- solo doc: §10.4 (SemCscFeature, framework.jar::classes5.dex)` | **INTEGRADO (verificador)** | `c2b45d8` | no hay que tocar el framework: lo lee solo. El VERIFICADOR post-parche ya existe: `rust-core/src/csc_verify.rs` (CLI `csc-verify`, tool MCP `csc_verify`) => lee el CSC activo, descifra el OMC, comprueba el valor, lo contrasta con el `fileSize` de `contents.db`, busca copias en claro y da la matriz de los 126 CSC |
| Detectores Marina/Lucky Patcher (C2) | `rust-core/src/app_intel.rs (artefactos embebidos, no el paquete)` | **INTEGRADO** | `aad4e8e` | regla: detecta artefactos, no el paquete ofuscado |
| NPU.bin real (/vendor/firmware) | `env_introspect (ya busca en /vendor/firmware y /vendor/etc/init)` | **INTEGRADO** | `51144e7 / a1a3430` | env_introspect devuelve available:true |
| Buscador de strings->origen en DEX | `tools/re/dexdump.py + tools/re/dexorigins.py (nuevo)` | **INTEGRADO** | `3f45329` / `a887e16` | dexorigins: clase+metodo de cada cadena. **FIX**: el indice de `const-string` esta en la UNIDAD SIGUIENTE (no en la del opcode) => antes daba 0 aciertos en todo el DEX; ademas casa por nombre de metodo/clase y lista las cadenas de la tabla sin referencia |
| Parser DEX: opcode y `encoded_value` | `rust-core/src/dex_decompile.rs` | **INTEGRADO** | `c2b45d8` | **4 bugs reales**: (1) opcode leido del byte BAJO (va en el ALTO) => decodificaba basura; (2) `encoded_value` con tipos inventados 0x09..0x10 => ningun DEX real parseaba (los reales: 0x17 STRING, 0x1c ARRAY, 0x1e NULL, anchos fijos); (3) `goto` 10t como 1 unidad => desalineaba el desensamblado; (4) delta del 10t leido del byte del opcode. 24/24 verde |
| Manifiesto AXML sin aapt | `tools/re/axml.py` | **INTEGRADO** | `-` | permisos, meta-data xposed, authorities |
| Mapa de 126 CSC + 19 con grabacion | `docs/ANDROID_EXYNOS2100_FINDINGS.md §10.2` | **INTEGRADO** | `60e53a3` | tabla + sha16 por CSC |
| Particiones mundo seguro + EFS (7 sha256) | `-- solo doc: §3.2` | **PENDIENTE** | `-` | integrar en backup_vault (W3/W4): que el backup cubra FLL.bin + keystore2 + /DAK + /tee/srt |
| P5 Swift Backup: lector DER | `-- no existe` | **PENDIENTE (avanzado)** | `-` | **DEX analizado (agente DEX `659db26c`)**: KDF = **Argon2id** (NATIVO `SbaNativeCrypto.deriveArgon2id`: salt 16 B, iter 1..100, mem 16384..65536 KiB, par 1..8, salida 32 B, contrasena UTF-8 borrada tras usarla; key-check SHA-256 con `MessageDigest.isEqual`) ⇒ el contenedor es **propio** (no `.ab`): **tar+zstd** dentro y **AEGIS-256 por chunks** (nonce+AAD por chunk) con cabecera de 12 campos (`kdfMethodId`, `kdfIterations`, `payloadChunkSize`...) => las iteraciones se leen del propio archivo. **FALSO AMIGO descartado**: `libconceal.so` (Facebook Conceal: PBKDF2 + AES-256-GCM) esta en el APK pero **NO es el KDF del backup local**. Falta un `.dat`/`.med` real (canal caido) para el round-trip |
| Resiliencia del canal (tunel 530) | `tools/phone_url.py + state/tunnel.url` | **PARCIAL** | `-` | el MCP sigue hardcodeando URL en md-secrets/instance.env => leerla en RUNTIME y preferir la LAN http://<IP_DEL_MOVIL>:3000/mcp |

## Los 4 que quedan abiertos (con dueño y condición de cierre)

1. ~~**Sonda VS4L como tool**~~ — **CERRADO**: `rust-core/src/npu_vs4l.rs` (`npu-probe --vs4l` / `npu_vs4l_probe`) ya está en `main` ⇒ emite `ioctl(fd, code)` sin argumento y devuelve `{code, live, driver_size}`.
2. ~~**Verificador post-parche del CSC**~~ — **CERRADO**: `rust-core/src/csc_verify.rs` (`csc-verify` / `csc_verify`) ⇒ comprueba el valor **exacto** de la feature (`--expect` / `expect_value`, veredicto `MISMATCH`), lo contrasta con `contents.db`, detecta copias en claro y da la matriz de los 126 CSC ⇒ **queda pendiente sólo ejecutarlo en el móvil tras el parche** (canal caído).
3. **EFS/mundo-seguro en el backup** — las 7 particiones con sha256 y el inventario del EFS están medidos (§3.2) pero **no están en `backup_vault`** ⇒ *cerrar*: que el backup cubra `FLL.bin` (anti-rollback), `/DAK/` y `/tee/srt/`, y avise de que sin `keystore2` el backup es incompleto.
4. **P5 Swift Backup (lector DER)** — el DEX ya está analizado (`/workspace/swift/classes.dex` + `classes2.dex`) ⇒ la derivación de clave está en **código nativo** (`libconceal.so`: PBKDF2 + AES-256-GCM) ⇒ *queda*: extraer un `.dat` real del móvil (canal caído) y escribir el **lector de sólo lectura** con round-trip; el tool `dexorigins.py` (arreglado) sirve para el resto de clases.

## Qué NO hay que rehacer (ya está en el repo)

· `tools/omc_codec.py` — decodificador OMC (Python, sin deps) + su equivalente Rust en `rom_migration.rs`.
· `tools/re/dexorigins.py` — de qué clase/método sale cada cadena (para la derivación AEGIS).
· `tools/re/axml.py` / `tools/re/dexdump.py` — manifiesto y DEX sin `aapt`/Java.
· `docs/ANDROID_EXYNOS2100_FINDINGS.md` — Manual §0-§10.4 (VS4L, TEE, boot, CSC, OMC, consumidor runtime).
· `docs/MARINA_PATCHER_ANALYSIS.md` — C2 de Marina Patcher.
· `rust-core/src/app_intel.rs` — detectores del C2 (por artefactos, no por paquete).
