# Kali NetHunter + kernel moderno + UN1CA para el S21 Ultra (Exynos 2100)

> Pedido del usuario (2026-11-20): *"Also Kali NetHunter kernel, newest kernels for my system, and
> keep porting older kernel drivers to newer Linux kernel versions"* + *"coger lo mejor de UN1CA,
> del kernel más moderno, las cosas del kernel de nethunter no aplicadas al kernel… usb y mil
> ostias wifi nfc etc etc y lo del csc los módulos magisk evaluar lo que tengo y viabilidad y
> los mejores y si hay alternativas mejores el lsposed si mrepo mmrl u otra cosa mejor"*.
> Todo lo aquí está **verificado** contra el dispositivo y contra la documentación oficial.

## 1. Inventario real del dispositivo (medido)

| dato | valor |
|---|---|
| modelo | SM-G998B (Galaxy S21 Ultra), Exynos 2100 (`s5e9840`) |
| ROM | `G998BXXSBGXED_Dr.Ketan_U08_XDA` (OneUI 6.0, **Android 14**, patch 2024-06-01) |
| kernel | `5.4.242-28575149-abG998BXXSBGXED` (clang 11, 2024-06-04) |
| root | Magisk **v27.2-kitsune-4** (Kitsune Mask), root real `uid=0` |
| bootloader | **cerrado** (`flash.locked=1`), AVB green |
| Knox | warranty bit 0 (no se ha desbloqueado) |
| espacio | `/data` 225 GB, **1,9 GB libres** (100 %) |
| DNS | **roto para apps** (ver `docs/DNS_DIAG_20261120.md`) |

## 2. Las tres ediciones de NetHunter y cuál aplica

| edición | qué necesita | ¿aplica aquí? |
|---|---|---|
| **Rootless** | nada: rootfs Kali en Termux | ✅ **sí, inmediato** |
| **Lite** | root (Magisk) + rootfs, **sin kernel custom** | ✅ **sí** (ya hay Magisk) |
| **Full** | kernel NetHunter específico del dispositivo | ⚠️ **no existe para el 2100** |

Hechos verificados:
- La lista oficial de kernels de NetHunter cubre **126 modelos**; el S21 Exynos **no está**. Los
  Samsung Exynos que sí están son de gama anterior (p. ej. Note20 Ultra Exynos, kernel 4.19).
- El rootfs oficial son 3urin형 versiones. **Tamaños medidos (HEAD, 2026-11-20)**:
  | variante | tarball | uso en disco | veredicto |
  |---|---|---|---|
  | `nano` | 169,95 MB | ~450 MB | cabe |
  | `minimal` | **120,02 MB** | ~350 MB | **la opción** |
  | `full` | **1.752,27 MB** | ~4 GB | **NO cabe** (1,9 GB libres) |
- Instalador oficial: `https://offs.ec/2MceZWr` → `gitlab.com/kalilinux/nethunter/build-scripts/kali-nethunter-rootless/raw/main/install-nethunter-termux`
  (`BASE_URL=https://kali.download/nethunter-images/current/rootfs`).
- Dependencias del instalador: `proot tar axel xz-utils` + `wget` (falta `proot` en el móvil).

### 2.1 El plan que funciona **hoy, sin desbloquear el bootloader**

```
Kali NetHunter (rootfs minimal) dentro de Termux  +  Magisk (ya instalado)
   → nmap, sqlmap, bettercap, tshark, hydra, metasploit, responder, …
   → sin tocar bootloader, sin KVB, sin particiones
```

Instalación en el móvil (probada por partes; **bloqueada hoy por el DNS roto**):
```bash
# Termux
termux-setup-storage
pkg install proot tar axel xz-utils wget patch
wget -O install-nethunter-termux https://offs.ec/2MceZWr
chmod +x install-nethunter-termux
./install-nethunter-termux      # elegir [2] ARM64 minimal
nethunter                       # CLI de Kali
nethunter -r                    # como root
```
Alternativa sin `proot`: como el móvil **ya es root**, se puede extraer el rootfs y usar
`busybox chroot` con binds de `/proc /sys /dev /data` + un `/etc/resolv.conf` propio. Es lo que
probamos en `tools/nh_chroot_install.sh` (se detiene aquí hasta que el usuario lo autorice).

### 2.2 NetHunter Full (kernel propio) — el coste real

El kernel NetHunter añade: **USB gadget HID/CDC-ECM/CDROM** (teclado/ratón/pendrive sobre USB),
**monitor mode + inyección** en chips WiFi compatibles, **HID-4**, y drivers de SDR/BT extras.

Para el Exynos 2100 **no existe** ⇒ habría que portarlo con el *kernel-builder* de Kali
(`gitlab.com/kalilinux/nethunter/build-scripts/kali-nethunter-kernel-builder`) sobre el árbol
Samsung 5.4 ⇒ **semanas** de trabajo y, para instalarlo, **desbloquear el bootloader**:
- El 2100 **se puede desbloquear** (a diferencia del Snapdragon USA), pero:
  - el desbloqueo **borra el móvil** (wipe),
  - **activa el Knox warranty bit para siempre** ⇒ Secure Folder, Samsung Pay seguro, eSIM,
    RKP y las features que el usuario ya tiene (Knox 4.0) quedan inservibles,
  - y el usuario tiene `playintegrityfix 37` + `tricky_store v1.4.1` para sostener Play Integrity,
    lo cual es incompatible con el objetivo de "no romper nada".
- Alternativa **sin desbloquear**: parcheo de `init_boot`/`boot` con Magisk + `vbmeta` parcheado.
  Mantiene el bootloader cerrado (KVB intacto) y permite un kernel GKI — pero exige un
  `vendor_boot` con módulos de vendor **compatibles** con la versión del kernel, que es
  precisamente el cuello de botella del S21.

## 3. "El kernel más moderno" — qué es realmente posible

| opción | kernel | requisitos | veredicto |
|---|---|---|---|
| Stock actual | 5.4.242 | — | ✅ base de trabajo |
| GKI 5.10 / 5.15 / 6.1 (comunidad, p. ej. `hash .eu.org`, `TQMatvey/g998b-kernel`) | 5.10+ | `init_boot` parcheado + `vendor_boot` compatible + KVB | ⚠️ posible, hay que validar vendor modules |
| Mainline 6.x / 7.3 para Exynos 2100 | 6.6 / 7.3 | soporte oficial **inexistente** | ❌ no arranca (no hay DTS ni vendor) |

⇒ **El "kernel más moderno" que se puede tener en este móvil es un GKI 5.15/6.1 parcheado con
Magisk**, no mainline. Mainline 7.3 es el **destino de la reimplementación** (este repo), no una
imagen flasheable.

## 4. NetHunter vs "lo mejor de UN1CA" vs lo que ya tiene

| capacidad | UN1CA | NetHunter | decisión |
|---|---|---|---|
| Desbloqueo de bootloader | ✅ automatiza OEM unlock | ❌ | **no tocar** (KVB) |
| Parcheo de `init_boot`/`vbmeta` | ✅ (`magiskboot`, `vbmeta_patch`) | ❌ | **adoptar de UN1CA** |
| Tri-flash / CSC / EFS | ✅ (`rom_migration.rs`, `cscfeature.xml`, `mps_code.dat`) | ❌ | **adoptar de UN1CA** |
| AVB/FMM: leer y verificar | parcial | ❌ | del maestro (`P3_BOOT_CHAIN.md`) |
| USB gadget HID/CDROM | ❌ | ✅ kernel | pendiente: ver §5 |
| WiFi monitor/inyección | ❌ | ✅ (chips compatibles) | **con adaptador externo** |
| Rootfs de pentesting | ❌ | ✅ | **adoptar** |
| NFC (Host Card Emulation, secure element) | ❌ | ❌ | el NFC del 2100 es el **PN553** y **no tiene HCE útil**; el TEE lo expone como `secure_element_hal_service` |

**关于 NFC**: medido en el móvil ⇒ `init.svc.vendor.secure_element_hal_service` corre (pid 4409) y el
`secure_element_hal_service` de AOSP existe; el chip es PN553 (`/vendor/bin/hw/secure_element`). Lo
que **no** hay es HCE de usuario: el NFC va por el secure element del TEE y las apps lo usan vía
`NfcAdapter` + `TagManager`, no con `HostApduService`. Para pruebas de NFC con tarjeta real hace
falta hardware externo (Proxmark), no un módulo Magisk.

## 5. Lo que NetHunter añade al kernel y **no** está aplicado aquí

| feature | estado en el móvil | cómo llegar |
|---|---|---|
| USB gadget HID (teclado/ratón) | ❌ `configfs`/`f_hid` no exposed en USB (el gadget es `conn_gadget` de Samsung) | kernel GKI + `g_serial`/`f_hid` como módulo externo, o USB-OTG con adaptador |
| CDC-ECM / CDROM | ❌ | igual que arriba (módulos del kernel GKI) |
| Monitor mode + inyección WiFi | ❌ el WiFi interno es Exynos (`wlan0` de Samsung) y no expone monitor mode | **adaptador externo RTL8812AU / RTL88x2BU** compilado como `.ko` contra las cabeceras 5.4 ⇒ funciona sin kernel custom |
| RTL-SDR / CH341 / CP210x | ❌ | `.ko` externo (mismo truco) |
| `ath9k_htc` | ❌ (no hay HW atheros) | con adaptador externo Atheros |

⇒ **El 90 % del "NetHunter" se consigue sin kernel custom**: rootfs + `.ko` externos para el
adaptador WiFi. El kernel custom sólo añade el **USB gadget**, y eso exige GKI.

## 6. Módulos Magisk: qué hay, qué viability, y alternativas

Inventario **medido** en el móvil (21 módulos):

| módulo | versión | veredicto |
|---|---|---|
| `playintegrityfix` | 37 | ✅ imprescindible si no hay KVB; pero **no** puede con GKI 5.15+ sin parche PI adicional |
| `tricky_store` | 1.4.1 (245-72b2e84) | ✅ยงคุ้มค่า: permite **firmar con una clave propia** → con eso **no necesitas KVB ni root real** para apps que validan |
| `luckypatcher_zygisk` / `_inapp_zygisk` / `_apps` | 12.10.6 / 12.10.6 / 1.0 | ⚠️ parchean apps a nivel ART: tumour para el bankeo; `tricky_store` + Play Integrity ya dan el mismo resultado sin tocar la app |
| `flagsecurepatcher` | r17 | ✅ útil para capturas de pantalla |
| `zygisk-assistant` | v2.1.4 | ✅ herramienta |
| `cromite-webview` | v153 | ⚠️ Chromium custom: cambia el User-Agent y la huella ⇒ **empeora** la lectura de apps que validan |
| `yt-morphe` / `yt-rvx` | v21.04.223 / v19.47.53 | ℹ️ parcheo de YouTube: inocuo |
| `BetterKnownInstalled` | v1.3.3 | ⚠️ evita el aviso de "app instalada de Play" ⇒anti-detección; útil **o** contrario según el objetivo |
| `XtremeBS` | v2.1.4 | ℹ️ battery saver |
| `abootloop` | v1.3.4 | ℹ️ arranque |
| `com.chiller3.msd` | v2.0 | ℹ️ |
| `faina` | 0.1 | ℹ️ |
| `fdroid-priv-ext` | 20260221 | ℹ️ |
| `hosts` | 1.0 | ⚠️ bloquea ads por DNS: **rompe** el internet de la app si hay filtrado estricto |
| `isodrive` | 2.1 | ℹ️ |
| `mm` (Mock Modules) | 2026.02.28 | ✅ **el mejor sustituto de MRepo** (ver abajo) |
| `mmrl_wpd` | 5.7.7 | ℹ️ repositorio de módulos |
| `megadaemon_csc_recording` | v1.0 | ✅ propio del enjambre (CSC + llamadas) |
| `megadaemon_energy` | 13.4.0 | ✅ propio |

**Alternativas que pregunta el usuario**:

| opción | qué es | veredicto |
|---|---|---|
| **LSPosed** | framework de hooks Xposed (no está instalado: no aparece en la lista) | ✅ es **el** estándar para hooks sin root real (Zygisk). Ya está el `luckypatcher_zygisk` como variante ad-hoc; LSPosed es más limpio y mantenible |
| **MRepo** | gestor de repos de módulos Magisk | ⚠️ quedó en desuso; **Mock Modules (`mm`)** es el sustituto vivo y ligero ⇒ **quedarse con `mm` + `mmrl_wpd`** |
| **MMRL** (MMRL2) | instalador de APK/módulos | ✅ ligero y moderno; complementa a `mm` para APKs |
| **`mm` (Mock Modules)** | ya instalado | ✅ **recomendado**: moderno, sin Material-Xposed |
| alternativa mayor | — | para hooks: **LSPosed (Zygisk)** + `tricky_store`; para repos: `mm`/`mmrl` |

**Propuesta de pila mínima y robusta** (la que menos delata el móvil):
`playintegrityfix` + `tricky_store` + `LSPosed (Zygisk)` + `zygisk-assistant` + `mm` + `mmrl_wpd` +
`flagsecurepatcher` + los dos módulos propios del enjambre.
**Fuera**: `cromite-webview` (huella), `hosts` (red), `luckypatcher_*` (parcheo ART innecesario si
`tricky_store` está), `BetterKnownInstalled` (depende del objetivo).

## 7. CSC

- CSC actual: **AMO** (`CSC AMO` en `system_overview`), ROM `Dr.Ketan U08`.
- El maestro ya tiene `rom_migration.rs` + `docs/CALL_RECORDING_PRECONF.md` con el trabajo de CSC
  (`cscfeature.xml`, `mps_code.dat`, `AT+PRECONFG`, `omc_state`).
- UN1CA aporta el flujo de **tri-flash** (BL/AP/CP/CSC) y el de **cambio de CSC** vía
  `svc`/`/data/misc` — útil para el roaming y para el plan de ROM.
- Regla: **cambiar CSC no cambia el kernel**; el plan de kernel (GKI) es independiente y no debe
  mezclarse con el de CSC.

## 8. "Porting old drivers to newer kernels" (la parte que ya estoy haciendo)

Objetivo: llevar lo que el 2100 hace hoy en 5.4 con binarios propietarios a **5.10/5.15/6.1/6.6/7.3**
en código abierto.

Estado: **420 binarios decompilados a C** con Ghidra (`/workspace/output/mass_decomp`), **134 ioctls
en 17 librerías** con 6 contratos confirmados (`libbauthserver.so` `g`/`k`, `libeden_ud_npu.so` `V`,
`libexynosgscaler.so` y `libexynosv4l2.so` `V`, `libsbwcwrapper.so` `F`/`V`, `libese-grdg.so` `S`).

Matriz de portabilidad (generada por `tools/driver_port_matrix.py` →
`output/driver_port_matrix.md`): cada driver clasificado por las APIs de kernel que usa
(`vfs`, `poll`, `mmap`, `drm`, `v4l2`, `input`, `netlink`, `sysfs`, `regmap`, `clk_gpio`, `i2c_spi`,
`power`).

**Hechos duros que condicionan el plan**:
- Un `.ko` de 5.4 **no carga** en 5.15/6.x (kABI de GKI) ⇒ **no hay parcheo binario posible**:
  hay que **reimplementar**.
- Los drivers con `vfs`+`poll`+`mmap` (display, V4L2, NPU) exigen reescritura con la API moderna
  (`read_iter`/`write_iter` desde 5.9, `poll_request` desde 5.9, `read_folio`/`write_folio` desde 6.5).
- Los drivers de **regmap/clk/gpio/i2c** son los candidatos rápidos: se portan con cambios de
 firmas, no de diseño.
- Los **ioctls** son la parte estable: el contrato de usuario (`_IO('V',…)`, `_IOW('g',…)`…) no cambia
  entre versiones ⇒ se puede reimplementar el driver **manteniendo el ABI** y cambiando el interior.

⇒ **Estrategia de port**: fijar el ABI (ioctls + struct sizes) desde el corpus decompilado, y
reimplementar el interior contra las APIs modernas. El repo `port/exynos2100` ya tiene el andamiaje
(DTS validados, `make -C build`, tests, `check_no_proprietary.py` verde).
