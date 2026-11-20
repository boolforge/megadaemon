# Trazabilidad: de hallazgo a código (qué está integrado y qué falta)

> La regla de la flota: un hallazgo que no acaba en código (o en una tool) es un hallazgo a medias.
> Esta matriz es el contrato: cada fila dice **dónde vive** y **quién lo cierra**.

| Hallazgo | Dónde vive | Estado | Commit | Nota |
|---|---|---|---|---|
| A-04 fail-closed (P0) | `rust-core/src/api_key.rs :: decide() + load_with()` | **INTEGRADO** | `4bf9f34` | 3 tests nuevos; fail-closed + autogenera key con aviso sin secreto |
| D-09 poison recovery (P1) | `rust-core/src/task_scheduler.rs (18) + alerts.rs (6)` | **INTEGRADO** | `4bf9f34` | lock().unwrap_or_else(|p| p.into_inner()) |
| VS4L: 7/9 vivos + tabla | `rust-core/src/dex_decompile.rs :: VS4L_IOCTLS (VS4L_LIVE_CODES=7)` | **INTEGRADO** | `bd734f7` | doc §1.2 con JSON machine-readable (live por codigo) |
| Sonda VS4L correcta (sin argumento) | `-- solo doc: §1.2 (python de 6 lineas)` | **PENDIENTE** | `-` | falta una tool (npu-probe/vs4l_probe) que use ioctl SIN argumento => devuelve el tamano que espera el driver |
| OMC: decode/encode del cscfeature.xml | `rust-core/src/rom_migration.rs (Rust) + tools/omc_codec.py (Python)` | **INTEGRADO** | `2105911 / 3f45329` | round-trip verificado en 126/126 CSC del movil |
| CSC feature: 19 CSC con grabacion + patch de AMO | `rust-core/src/rom_migration.rs :: csc_feature_get/set + omc_feature_patch_plan()` | **INTEGRADO** | `2105911` | valor a escribir = 'RecordingAllowed' (no true); tag exacto |
| Consumidor runtime del CSC | `-- solo doc: §10.4 (SemCscFeature, framework.jar::classes5.dex)` | **PENDIENTE** | `-` | no hay que tocarlo (el framework lo lee), pero falta un VERIFICADOR post-parche: que el flag este activo tras reiniciar |
| Detectores Marina/Lucky Patcher (C2) | `rust-core/src/app_intel.rs (artefactos embebidos, no el paquete)` | **INTEGRADO** | `aad4e8e` | regla: detecta artefactos, no el paquete ofuscado |
| NPU.bin real (/vendor/firmware) | `env_introspect (ya busca en /vendor/firmware y /vendor/etc/init)` | **INTEGRADO** | `51144e7 / a1a3430` | env_introspect devuelve available:true |
| Buscador de strings->origen en DEX | `tools/re/dexdump.py + tools/re/dexorigins.py (nuevo)` | **INTEGRADO** | `3f45329` | dexorigins: clase+metodo de cada cadena; OJO opcode en byte ALTO |
| Manifiesto AXML sin aapt | `tools/re/axml.py` | **INTEGRADO** | `-` | permisos, meta-data xposed, authorities |
| Mapa de 126 CSC + 19 con grabacion | `docs/ANDROID_EXYNOS2100_FINDINGS.md §10.2` | **INTEGRADO** | `60e53a3` | tabla + sha16 por CSC |
| Particiones mundo seguro + EFS (7 sha256) | `-- solo doc: §3.2` | **PENDIENTE** | `-` | integrar en backup_vault (W3/W4): que el backup cubra FLL.bin + keystore2 + /DAK + /tee/srt |
| P5 Swift Backup: lector DER | `-- no existe` | **PENDIENTE** | `-` | falta (a) el DEX (lo pone el coordinador en /workspace/swift/) y (b) un .dat real para el round-trip; (-> tools/re/dexorigins.py para la derivacion AEGIS) |
| Resiliencia del canal (tunel 530) | `tools/phone_url.py + state/tunnel.url` | **PARCIAL** | `-` | el MCP sigue hardcodeando URL en md-secrets/instance.env => leerla en RUNTIME y preferir la LAN http://192.168.1.124:3000/mcp |

## Los 4 que quedan abiertos (con dueño y condición de cierre)

1. **Sonda VS4L como tool** — la tabla ya está en `dex_decompile`, pero **nadie puede sondear el driver** con el método correcto desde la flota: el comando está en el doc (§1.2) ⇒ *cerrar*: un `npu-probe --vs4l` que emita `ioctl(fd, code)` sin argumento y devuelva `{code, live, driver_size}`.
2. **Verificador post-parche del CSC** — el parche lo aplica el agente RE (W4) pero **nadie puede comprobar** que el framework cogió el flag ⇒ *cerrar*: tool que arranque el móvil, lea `SemCscFeature.getBoolean(...)` por logs/`dumpsys` y devuelva `grabacion=ON/OFF`.
3. **EFS/mundo-seguro en el backup** — las 7 particiones con sha256 y el inventario del EFS están medidos (§3.2) pero **no están en `backup_vault`** ⇒ *cerrar*: que el backup cubra `FLL.bin` (anti-rollback), `/DAK/` y `/tee/srt/`, y avise de que sin `keystore2` el backup es incompleto.
4. **P5 Swift Backup (lector DER)** — falta el DEX (el coordinador lo sube a `/workspace/swift/`) y un `.dat` real ⇒ *cerrar*: `tools/re/dexorigins.py` sobre el DEX para la derivación AEGIS-256, luego lector de sólo lectura con round-trip contra el `.dat`.

## Qué NO hay que rehacer (ya está en el repo)

· `tools/omc_codec.py` — decodificador OMC (Python, sin deps) + su equivalente Rust en `rom_migration.rs`.
· `tools/re/dexorigins.py` — de qué clase/método sale cada cadena (para la derivación AEGIS).
· `tools/re/axml.py` / `tools/re/dexdump.py` — manifiesto y DEX sin `aapt`/Java.
· `docs/ANDROID_EXYNOS2100_FINDINGS.md` — Manual §0-§10.4 (VS4L, TEE, boot, CSC, OMC, consumidor runtime).
· `docs/MARINA_PATCHER_ANALYSIS.md` — C2 de Marina Patcher.
· `rust-core/src/app_intel.rs` — detectores del C2 (por artefactos, no por paquete).
