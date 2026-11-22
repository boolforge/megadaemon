# Backlog de ideas (todo lo pedido, con estado) — 2026-11-20

> "Apunta todas las ideas en el repo YA, todo organizado. Por si acaso."
> Este documento es el índice maestro. Cada bloque apunta al documento de detalle.
> Leyenda: ✅ hecho · 🟡 en curso · ⏳ pendiente · ⛔ bloqueado (por qué)

## A. Backup 100 % a Google Drive

| # | idea | estado | detalle |
|---|---|---|---|
| A1 | Backup **en stream** del 100 % de `/data` (223 GB) sin staging local | ⏳ | `BACKUP_100_STREAM.md` — el móvil tiene 1,9 GB libres; imposible sin stream |
| A2 | Imágenes de **todas las particiones** (`dd \| stream`: super, system, vendor, product, odm, prism, optics, boot, init_boot, vendor_boot, vbmeta*, efs, keyrefuge, metadata, nv_data) | ⏳ | ídem |
| A3 | **Keyring / contraseñas / cookies / bookmarks / passkeys / pestañas abiertas** | ⏳ | backup cifrado con el vault; **nunca en claro** |
| A4 | **Conversaciones** (ChatGPT/Claude/Gemini/DeepSeek/Kimi…) e historial en nube | 🟡 | `sync_conversations` + `sync_web_conversations` ya existen |
| A5 | **Dejar espacio libre** sin borrar nada importante: subir → verificar → *proponer* limpieza (cachés, `Download` 44 GB, APK de `/data/app` 88 GB) | ⏳ | el daemon **informa**, no borra |
| A6 | Verificación doble: **MD5 de Drive + SHA-256 propio** + manifiesto `SHA256SUMS.txt` | 🟡 | `sha2` ya está en el crate |
| A7 | Reanudable ante cortes (red móvil) | 🟡 | el cliente nativo ya usa `uploadType=resumable` (8 MiB) |
| A8 | Cliente de Drive **nativo Rust** (no rclone) | ✅ | ya existe en `backup_vault.rs`; **rclone = opcional** |
| A9 | ⛔ **Token de Google** | 🟡 | `rclone authorize` **esperando el código** en mi sandbox (URL de Google lista); el móvil ya tiene rclone ARM64 y `tools/nh_backup_phone.sh` listo para las 5 fases |

## B. Kali NetHunter + kernel

| # | idea | estado | detalle |
|---|---|---|---|
| B1 | **Kali NetHunter rootfs minimal** en Termux (120 MB, medido) | ⛔ | instalador **apagado por orden del usuario**; **bloqueo de DNS resuelto**: `tools/http_by_ip.py` descarga por IP (rclone ARM64 ya instalado en el móvil por esa vía) |
| B2 | NetHunter **Lite** (Magisk, sin kernel custom) | ⏳ | el móvil ya tiene Magisk 27.2-kitsune-4 |
| B3 | NetHunter **Full** (kernel propio) | ⏳ | **no existe** para el 2100; exigiría portar con el kernel-builder + desbloquear bootloader (KVB) |
| B4 | "Kernel más moderno": **GKI 5.15/6.1** con `init_boot` parcheado (Magisk + `vbmeta` parcheado) ⇒ **sin desbloquear, KVB intacto** | ⏳ | cuello de botella: `vendor_boot` con módulos compatibles |
| B5 | **Mainline 6.x/7.3** para el 2100 | ⛔ | no hay soporte oficial ⇒ 7.3 es **destino de reimplementación**, no imagen flasheable |
| B6 | USB gadget **HID/CDC-ECM/CDROM** (feature NetHunter) | ⏳ | requiere GKI (configfs/f_hid no está en el USB de Samsung) |
| B7 | **WiFi monitor + inyección** | 🟡 | con **adaptador externo** RTL8812AU/RTL88x2BU como `.ko` contra las cabeceras 5.4 ⇒ **sin kernel custom** |
| B8 | NFC: el 2100 usa PN553 en TEE (`secure_element_hal_service`); **no hay HCE de usuario** | ✅ | documentado; para NFC real hace falta Proxmark |
| B9 | Tomar de **UN1CA**: parcheo de `init_boot`/`vbmeta`, tri-flash, cambio de CSC, EFS | 🟡 | `rom_migration.rs` ya cubre parte |
| B10 | **CSC**: actual AMO; el cambio de CSC no toca el kernel | ✅ | `docs/CALL_RECORDING_PRECONF.md` |

## C. Magisk / hook / repos

| # | idea | estado | detalle |
|---|---|---|---|
| C1 | Evaluar los **21 módulos** instalados (viabilidad, pros/contras) | ✅ | `KALI_NETHUNTER_KERNEL_PLAN.md` §6 |
| C2 | Pila recomendada: `playintegrityfix` + `tricky_store` + **LSPosed (Zygisk)** + `zygisk-assistant` + **`mm`** + `mmrl_wpd` + `flagsecurepatcher` + módulos propios | 🟡 | instalar LSPosed + quitar `cromite-webview`/`hosts`/`luckypatcher_*` |
| C3 | **LSPosed** (no instalado) como estándar de hooks | ⏳ | sustituye a los módulos `luckypatcher_zygisk` |
| C4 | **MRepo → Mock Modules (`mm`)**: ya instalado, mejor y más ligero | ✅ | decisión: quedarse con `mm` + `mmrl_wpd` |
| C5 | `tricky_store` permite **firmar con clave propia** ⇒ evita KVB y root real para apps que validan | ✅ | ya instalado (v1.4.1) |
| C6 | Audit de apps que **detectan root** (`com.mc.notify` 19 señales, knoxpatch 12, magiskdetector) | ✅ | `docs/P5_ROOT_DETECTION.md` |
| C7 | `root_hide` era **simulación** → ahora mide de verdad | ✅ | `root_detectors` + `root_consumers` (en `fix/robustness-20261120`, falta rebuild en el maestro) |

## D. MegaDaemon como "mega store manager"

| # | idea | estado | detalle |
|---|---|---|---|
| D1 | Diseño `StoreDescriptor` + índices firmados + instaladores (Magisk/APK/payload/tool) | ✅ | `MEGADAEMON_CLEANUP_PLAN.md` §3 |
| D2 | **Ligero**: sin SDK de Google, sin rclone, sin binario externo; índices en JSON cacheado | ✅ | diseño |
| D3 | API estable `/api/stores*` para que la GUI no toque el disco | ⏳ | prerequisite de la GUI |
| D4 | Registry **declarativa** de tools (53 cableados a mano hoy) | ⏳ | reduce el coste de "hacer más cosas" |
| D5 | Limpiar ramas | ✅ | `publish/*` borradas (fusionadas) y `fix/megadaemon-stability` rebaseada (ahead 2 / behind 0) ⇒ **4 ramas → 3** |
| D6 | Absorber los **12 duplicados exactos** en `util_text.rs`/`util_fs.rs` | ⏳ | medido: `walk_files`×3, `hex`×3, `now_iso`×2, `format_iso8601`×2, `parse_apk_paths`×2 |
| D7 | Trocear `signing.rs` (5.887 líneas) y `webui.rs` (4.767) | ⏳ | top-10 = 45 % del crate |
| D8 | Unificar el parser DEX Rust con `tools/dex_deobf.py` | ⏳ | arregla de una vez el bug `encoded_value 0x17` |

## E. RE / Exynos 2100 (continuo)

| # | idea | estado | detalle |
|---|---|---|---|
| E1 | P1 NPU/EDEN verificado en el dispositivo (9 ioctls VS4L, `/dev/vertex10` 0644, NPU en producción, LLM en NPU imposible) | ✅ | `docs/P1_NPU_EDEN.md` |
| E2 | P2 TEE / P3 boot chain / P4 vendor (134 ioctls) / P5 root | ✅ | `docs/P2..P5` |
| E3 | **420 binarios decompilados** a C + 3 oráculos (unicorn, Z3, angr) | ✅ | `output/mass_decomp/` (fuera del repo, regla legal) |
| E4 | **Matriz de portabilidad 5.4 → 7.3** por driver | ✅ | `output/driver_port_matrix.md` |
| E5 | Reimplementar los drivers con **ABI estable** (ioctls) sobre APIs modernas | ⏳ | repo `port/exynos2100` |
| E6 | ADB/TCP: el `adbd` del ROM **descarta el CNXN** (64 combinaciones) ⇒ hace falta **pairing+TLS** (wireless debugging) | 🟡 | `docs/ADB_HANDSHAKE_DIAG.md` |
| E7 | **DNS roto en el móvil** (el VPN ata el DNS por uid; root sí resuelve, las apps no) | 🟡 | `docs/DNS_DIAG_20261120.md` |

## F. things que dependen del usuario

| # | qué hace falta | para qué |
|---|---|---|
| F1 | `client_secret` de la app de Google **o** autorizar `rclone config` en el navegador (pegar el código) | A1-A9 (backup) |
| F2 | En la app VPN: **DNS a través de la VPN** o DNS manual (1.1.1.1) | B1, descargas, E6 |
| F3 | Decidir si el kernel pasa a **GKI 5.15/6.1** (implica testear vendor modules) | B4 |
| F4 | Decidir qué se limpia en el móvil **después** del backup | A5 |
| F5 | Confirmar si se instala **LSPosed** y se quitan `cromite-webview`/`hosts`/`luckypatcher_*` | C2 |

## G. Antigravity — backlog del cliente en el móvil (2026-11-22)

> Recuperado del S21 Ultra por root. Detalle y evidencia: `docs/ANTIGRAVITY_BACKLOG.md`.

| # | idea | estado | detalle |
|---|---|---|---|
| G1 | **Backlog real del micro-daemon de Antigravity** (2 tareas en `daemon/tasks.json`, no 1) | ✅ | `task_442838` "Snatcher-MegaDaemon-SuperLoop" (interval 300 s) + `task_553653` "minimax_bootstrap" (delay, **nunca ejecutada**) |
| G2 | **Reanudar la tarea recurrente**: el worker del ciclo #38 arrancó el 19-Nov 10:21:33 y el micro-daemon **murió sin dejar rastro** (0 errores en las 860 líneas del log, sin crash dump, PID reciclado a otra app) | ⛔ | el planificador vive **dentro** del proceso `com.agy`: sin supervisor ni autostart no vuelve solo. Quedó **2 d 14 h** sin ejecutar tras 4 reinicios |
| G3 | **Arranque con lock + timeout + log con fecha + estado legible** (para que el enjambre lea el ciclo) | ✅ | `deploy/antigravity-task-runner.sh`, `deploy/antigravity-task-supervisor.sh`, `deploy/antigravity-tasks.py` — probados en el móvil: lock OK, timeout `rc=124`, concurrencia OK, catch-up OK, `last_status.json` OK |
| G4 | **Recuperar lo perdido**: 14.494 funciones, 923 descompiladas, 36 builds OK / 1 KO, 37 ciclos | ✅ | `megadaemon_state.json` (el estado del motor está a salvo) |
| G5 | **Recuperar las 27 h 25 min** de ejecución perdida (18-Nov 06:51 → 19-Nov 10:16) | ⛔ | irrecuperable: no quedó registro. La causa del hueco **NO VERIFICADA** |
| G6 | **Salida de los ciclos completa** (el daemon recorta `last_output` a ~1 KB ⇒ el enjambre nunca leyó el resultado) | ✅ | el runner guarda el log íntegro con fecha |
| G7 | **Cerrar el `verified: false`** de las 6 plataformas (ps1 14.494 func, saturn 0, pce 0, msx2 256 paletas, pc88 7 ports, segacd 53.110) | ⏳ | el motor declara mucho y **no ha verificado nada** |
| G8 | **Rotar el bearer de Google en claro** de `agy_standalone/probe.log` | ⛔ | requisito previo al catch-up de `task_553653` (`bootstrap_core.sh` escribe ahí) |
| G9 | **Bajar la carga del móvil** antes de reactivar el bucle de 300 s | ⏳ | medido: `MemFree` 466 MB, swap 3.341/4.095 MB, load 22 ⇒ el ciclo es un trabajo pesado |
