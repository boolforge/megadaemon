# Investigación: kernels para el Exynos 2100 (SM-G998B) y NetHunter en 2026

> **Fecha de la investigación: 2026-11-22.** Todas las fuentes de abajo se
> descargaron y comprobaron ese día. Lo que no se pudo abrir va marcado
> como **NO VERIFICADO**.
>
> **Dispositivo de referencia** (medido en el móvil, no supuesto):
> SM-G998B (Galaxy S21 Ultra), SoC **Exynos 2100**, Android 14 (SDK 34),
> ROM `G998BXXSBGXED_Dr.Ketan_U08_XDA`, kernel **5.4.242-28575149-abG998BXXSBGXED**,
> Magisk v27.2-kitsune-4 (Kitsune Mask), bootloader **cerrado**
> (`ro.boot.flash.locked=1`, `ro.boot.vbmeta.device_state=locked`).

---

## 1. Veredicto en una línea

> **NO existe un kernel de Kali NetHunter para el Exynos 2100.** No existe en
> la lista oficial de kernels (275 kernels / 126 modelos) ni en el
> `devices.yml` del repositorio oficial de kernels, y el árbol de device-trees
> de Kali no tiene **ningún** `exynos2100*.dtb`. El Exynos 2100 **no está
> soportado por NetHunter Full**; sí lo está (y de sobra) por **NetHunter
> Rootless** y **NetHunter Lite**, que **no necesitan kernel propio**.

---

## 2. NetHunter: qué hay y qué no hay (fuente oficial)

### 2.1 Lista oficial de kernels

| fuente | URL | fecha de comprobación | resultado |
|---|---|---|---|
| Tabla de kernels | <https://nethunter.kali.org/kernels> | 2026-11-22 (HTML descargado, 86.959 B) | "275 kernels … 126 device models". `grep -iE "2100\|G998\|SM-G998\|S21\|p3s\|o1s\|universal2100"` sobre el HTML ⇒ **0 coincidencias** |
| Tabla de device-modules | <https://nethunter.kali.org/device-kernels.html> | 2026-11-22 | "126 device modules · 191 kernels · 275 kernel versions". La lista de Samsung termina en **Note20 Ultra (Exynos) `c2s`** y **S20 FE 5G (Snapdragon) `r8q`** |
| `devices.yml` del repo oficial de kernels | <https://gitlab.com/kalilinux/nethunter/build-scripts/kali-nethunter-kernels> (`devices.yml`, 151.203 B) | 2026-11-22 | **126 modelos**, **272 ids de kernel**, niveles de Android de `kitkat` a `sixteen`. `grep -inE "2100\|G998\|S21\|p3s\|o1s\|dm1s"` ⇒ **0 coincidencias** |
| Índice de imágenes oficiales | <https://kali.download/nethunter-images/current/> | 2026-11-22 | **93 zips**. Los de Samsung: `…-c2s-oui-ksu-thirteen-full.zip` (Note20 Ultra Exynos) y `…-r8q-los-…` (S20 FE). **No hay ningún `p3s`/`G998`/S21** |
| Device-trees de Kali (ARM64) | <http://http.kali.org/dists/kali-dev/main/installer-arm64/current/images/device-tree/exynos/> | 2026-11-22 (índice fechado 2026-Jul-04) | `exynos2200-g0s`, `exynos5433-tm2`, `exynos7870-*`, `exynos9810-starlte`, `exynos990-*`… ⇒ **ningún `exynos2100*.dtb`** |

El último commit del repo oficial de kernels es de **2026-10-17** y el
proyecto `kali-nethunter-kernels` se movía el **2026-11-22** (actividad en
GitLab). Es decir: **la lista oficial está viva y el 2100 sigue sin estar**.

### 2.2 El Exynos más cercano que sí tiene NetHunter

`Samsung Galaxy Note20 Ultra (Exynos)`, kernel-id **`c2s-oui-ksu`**:
Android 13, **Linux 4.19**, "One UI 5.1", funciones `CDROM, HID-4, Injection,
ATH9K_HTC` (fuente: `nethunter.kali.org/kernels`, fila de `c2s-oui-ksu`).
⇒ Es el **único** NetHunter de un Exynos Samsung reciente, y es de otra
generación (Note20 Ultra, no S21).

### 2.3 Las tres ediciones de NetHunter y cuál aplica aquí

Fuente: <https://www.kali.org/docs/nethunter/> y
<https://www.kali.org/docs/nethunter/nethunter-rootless/> (verificados 2026-11-22).

| edición | qué necesita | ¿aplica al S21 Ultra Exynos? |
|---|---|---|
| **Rootless** | nada: Termux + rootfs Kali con `proot` | ✅ **sí, inmediato** |
| **Lite** | root (Magisk) + rootfs Kali, **sin kernel custom** | ✅ **sí** (ya hay Magisk) |
| **Full** | kernel NetHunter **específico del dispositivo** | ❌ **no existe para el 2100** |

Tabla de capacidades de la doc oficial (verificada 2026-11-22):

| capacidad | Rootless | Lite | Full |
|---|---|---|---|
| Kali CLI, todas las apps de Kali, KeX | ✅ | ✅ | ✅ |
| Metasploit con base de datos | ❌ | ✅ | ✅ |
| **Wi-Fi injection** | ❌ | ❌ | ✅ |
| **HID attacks** | ❌ | ❌ | ✅ |
| **BT Arsenal / EvilTwin** | ❌ | ❌ | ✅ |

⇒ El 100 % de "NetHunter" que se puede tener aquí es **Rootless o Lite**.
Lo único que se pierde son las capacidades que **dependen del kernel**
(inyección WiFi sobre el chip interno, HID sobre USB y BT Arsenal/EvilTwin).

### 2.4 Rutas de instalación verificadas

| recurso | URL | comprobado |
|---|---|---|
| Instalador rootless oficial (URL corta) | `https://offs.ec/2MceZWr` | 2026-11-22: **302** → `https://gitlab.com/kalilinux/nethunter/build-scripts/kali-nethunter-rootless/raw/main/install-nethunter-termux` (9.577 B) |
| Instalador rootless en GitLab | <https://gitlab.com/kalilinux/nethunter/build-scripts/kali-nethunter-rootless> | 2026-11-22: actividad 2026-06-30, 22 estrellas |
| Rootfs ARM64 (índice) | <https://kali.download/nethunter-images/current/rootfs/> | 2026-11-22 (ficheros fechados 2026-Oct-24) |
| `SHA256SUMS` del rootfs | <https://kali.download/nethunter-images/current/rootfs/SHA256SUMS> | 2026-11-22 (1,3 KiB, 2026-Oct-25) |
| NetHunter **generic ARM64** (sin kernel de dispositivo) | `https://kali.download/nethunter-images/current/kali-nethunter-2026.3-generic-arm64-{nano,minimal,full}.zip` | 2026-11-22: 375.731.555 / 323.359.325 / 2.035.365.099 B (`Last-Modified: 2026-10-24`) |
| Grupo de build-scripts de NetHunter | <https://gitlab.com/kalilinux/nethunter/build-scripts> | 2026-11-22, 11 proyectos: `kali-nethunter-rootfs`, `-rootless`, `-installer`, `-pro`, `-webinstall`, `-bootanimation`, `-kernel-builder`, `-kernels`, `-lrt`, `-utils`, `-project` |
| NetHunter Kernel Builder (para portar) | <https://gitlab.com/kalilinux/nethunter/build-scripts/kali-nethunter-kernel-builder> | 2026-11-22: actividad 2026-10-27, 49 estrellas |

**Tamaños del rootfs ARM64** (índice oficial, 2026-10-24):

| variante | descarga | instalado (estimado) |
|---|---|---|
| `nano` | 169,9 MiB | ~450 MB |
| `minimal` | **120,0 MiB** | ~350 MB |
| `full` | 1,7 GiB | ~4 GB |

SHA-256 publicados (2026-10-25):
`minimal-arm64` = `96aeced81a857a987715a14417af05c2934e3e19052d6151c398a1c215448b60`,
`nano-arm64` = `4fe2db857e2b3a7924e7e53059813c475fa461e0747de313f87272a049a08484`,
`full-arm64` = `cc677eac68d6699873b1d617dda2957506665e71f9910923584248c80e1ec007`.

> **NOTA sobre el espacio**: el doc previo de este repo decía "1,9 GB
> libres". **Es falso hoy**: medido el 2026-11-22, `/data` tiene **225 GB
> totales, 191 GB usados, ~33 GB libres** (`stat -f`: 8.771.112 bloques de
> 4.096 B libres en f2fs). ⇒ **cabe incluso el rootfs `full`**. La cifra
> antigua invalidaba la opción por espacio; la real la deja abierta.

---

## 3. Kernels que sí existen para el Exynos 2100 (2026)

Ninguno de estos es de NetHunter; todos son de la comunidad.

| proyecto | URL | estado (2026-11-22) | kernel |
|---|---|---|---|
| **Chiclet Kernel** (`xfwdrev/android_kernel_samsung_ex2100`) | <https://github.com/xfwdrev/android_kernel_samsung_ex2100> | release **v7.1 el 2026-11-16** (AnyKernel3, KernelSU-Next); v7 (2026-08-20), v6.6 (2026-06-05), v6.5.2 (2026-05-10) | **Linux 5.4.305** — la serie 5.4 está **EOL** y el propio proyecto dice que "el kernel se quedará en 5.4" |
| **LineageOS 23.2 (Android 16) para S21 Exynos** (`Macktherealonereally/Exynos-LineageOS-Mack-ROM`) | <https://github.com/Macktherealonereally/Exynos-LineageOS-Mack-ROM> | releases del **2026-10-06** al **2026-11-19**; último tag `lineage-23.2-20261119` | rama `lineage-23.2-20261106` de `android_kernel_samsung_universal2100` ⇒ **Linux 5.4.302** ("Kleptomaniac Octopus") |
| Fuentes del kernel de LineageOS-exynos | <https://github.com/Macktherealonereally/android_kernel_samsung_universal2100> | ramas `lineage-23.2*`, `G991B-ONLY`, `android12-5.4-exynos`… | `Makefile`: VERSION 5 / PATCHLEVEL 4 / SUBLEVEL **302** |
| **UN1CA** (fuente de kernel del 2100) | <https://github.com/UN1CA/kernel_samsung_exynos2100> | repo activo | 5.4 + parches |
| **Proton+** | <https://github.com/ProtonKernel/Proton> | repo activo | 5.4 GKI |
| **TQMatvey** (kernel OneUI para S21 Ultra) | <https://github.com/TQMatvey/g998b-kernel> | repo activo; **sin README en `main` (404)** y **sin releases** ⇒ NO VERIFICADO su contenido | árbol GKI (tiene `build.config.gki`) |
| **Exy2100-LOS** (manifest de la familia S21) | <https://github.com/Exy2100-LOS> | **parado en 2023** (`android_device_samsung_p3s`, `android_kernel_samsung_p3s` sin pushes desde 2023-08/10) | 5.4 |
| Árbol `exy2100` original | <https://github.com/exy2100> | último push 2026-07-21 | 5.4 |

### 3.1 Lo que "starlight" es (y lo que no es)

- `starlte` = **Galaxy S9**, `star2lte` = **Galaxy S9+** (los dos **S9**, no S21).
- No existe ningún proyecto LineageOS vivo llamado "starlight" para el
  Exynos 2100: `grep -i starlight` en la tabla oficial de NetHunter ⇒ 0, y el
  wiki de LineageOS no lista "starlight" como dispositivo.
- Lo que **sí** existe para el 2100 es la familia **`o1s` (S21 base, SM-G991B)**,
  **`p3s` (S21 Ultra, SM-G998B = este móvil)** y `dm1s` (S21+), con la
  LineageOS 23.2 de `Macktherealonereally`.
- ⚠️ **La LineageOS 23.2 viva publica únicamente `o1s`** (SM-G991B). Los
  assets de las releases son `…-UNOFFICIAL-o1s-{NG,MTG,AMTG}.zip`: **no hay
  build `p3s`** ⇒ el S21 **Ultra** (este móvil) **no** tiene LineageOS
  oficial de la comunidad hoy.
- Su README avisa (verificado): *"Your warranty is now void. Knox will trip;
  Samsung Pay/Wallet and Secure Folder stop working for good. Make a backup
  first."* ⇒ **desbloquear el bootloader no es opción** en este móvil.

### 3.2 Por qué un kernel 5.10/6.x para el 2100 no es realista

El `vendor_boot` de este ROM trae los **módulos de vendor compilados contra
el 5.4** con `CONFIG_MODVERSIONS=y` (medido). Un kernel 5.10+/6.x **no
cargaría** esos `.ko` (kABI incompatible) ⇒ sin `vendor_boot` que los
contenga, el móvil no llegaría a.boot (display, cámara, GPU, NPU, NFC…). Los
proyectos reales del 2100 (Chiclet, Proton+, LineageOS-exynos, UN1CA) **todos
se quedan en la rama 5.4.x**, y el propio Chiclet lo dice: *"Linux 5.4 is now
officially EOL, therefore the kernel version will permanently remain at
5.4.302"*.

⇒ **El "kernel más moderno" real para este 2100 es un 5.4.3xx parcheado**
(que es exactamente lo que ofrece Chiclet v7.1, del 2026-11-16), **no** un
6.x. Y su beneficio real (KernSU/AnyKernel3, tweaks, EROFS, KFENCE, DMA-BUF)
**no incluye** nada de NetHunter.

---

## 4. Hallazgo medido importante: el kernel de fábrica YA tiene el USB gadget de NetHunter

Del `/proc/config.gz` del móvil (**medido**, no de la doc):

```
CONFIG_USB_GADGET=y            CONFIG_USB_CONFIGFS=y
CONFIG_USB_F_HID=y             CONFIG_USB_CONFIGFS_F_HID=y     ← HID (teclado/ratón) BUILT-IN
CONFIG_USB_LIBCOMPOSITE=y      CONFIG_USB_U_SERIAL=y
CONFIG_USB_CONFIGFS_SERIAL=y   CONFIG_USB_F_ACM=y  CONFIG_USB_CONFIGFS_ACM=y
CONFIG_USB_CONFIGFS_NCM=y      CONFIG_USB_CONFIGFS_RNDIS=y
CONFIG_USB_CONFIGFS_MASS_STORAGE=y                           ← CDROM/pendrive
CONFIG_USB_DWC3=y              CONFIG_USB_DWC3_GADGET=y
CONFIG_USB_DWC3_EXYNOS=y       CONFIG_USB_OTG=y
```

Y en el sistema de ficheros:

* `configfs` está montado en **`/config`** (no en `/sys/kernel/config`).
* Ya existe un gadget de Samsung en **`/config/usb_gadget/g1`**, con la
  función `usb_f_conn_gadget` (módulo de Samsung) enlazada al UDC
  **`10e00000.usb`** (el controlador dwc3 real del SoC).
* `/sys/class/udc/` expone **dos** UDC: `10e00000.usb` (el real, dwc3) y
  `dummy_udc.0`.
* `f_hid`, `libcomposite`, `g_serial` **no** están como módulos cargados
  (los que hay son los de Samsung: `usb_f_conn_gadget`, `usb_f_dm`,
  `usb_f_dm1`, `usb_f_ptp`, `usb_f_mtp_samsung`, `usb_f_ss_mon_gadget`), pero
  la config dice `=y` ⇒ están **compilados dentro** del kernel.

⇒ **La función estrella de NetHunter Full (HID por USB: teclado/ratón
inyectado en el PC) se puede crear en este móvil con el kernel DE FÁBRICA**,
creando un gadget de configfs con `f_hid` y enlazándolo al UDC `10e00000.usb`
— **sin flashear un kernel**. Lo mismo con `g_serial` (CDC-ACM), `NCM`/`RNDIS`
(red por USB) y `mass_storage` (CDROM).

**Matices honestos (no verificados en la práctica):**
* hay que hacerlo **con el gadget de Samsung `g1` desmontado o aparte**, y
  un gadget mal configurado deja el móvil sin USB ⇒ el túnel (y el control
  remoto) se cae; ⇒ sólo con el cable conectado y **opt-in del usuario**;
* SELinux puede restrictingar la escritura en `/config` (no comprobado);
* `f_hid` requiere `hid` (feature) y no un `.ko` aparte: no hace falta
  compilar nada.

⇒ **Conclusión**: la única capacidad de NetHunter Full que de verdad **no**
se puede tener sin kernel propio es la **inyección/monitor mode del WiFi
interno** (chip Samsung/Exynos, driver propietario, sin monitor mode) — y eso
se resuelve con un **adaptador USB externo** (RTL8812AU/RTL88x2BU) y su `.ko`
compilado contra las cabeceras 5.4, **sin tocar el kernel del móvil**.

---

## 5. Qué se puede hacer SIN tocar el kernel (resumen)

| objetivo | cómo | ¿kernel? |
|---|---|---|
| Kali completo (nmap, sqlmap, metasploit, bettercap, tshark, hydra…) | **NetHunter Rootless**: Termux + `proot` + rootfs `minimal`/`nano` (120,0 / 169,9 MiB) | ❌ no |
| Kali con **base de datos** de Metasploit y **root real** dentro | **NetHunter Lite** (`generic-arm64` como módulo Magisk, o chroot con `busybox` desde root) | ❌ no |
| **HID** (teclado/ratón en el PC), CDC-ACM, red NCM/RNDIS, CDROM | gadget de **configfs** con `f_hid`/`g_serial`/`NCM`/`mass_storage` (ya `=y` en el kernel) | ❌ no (con cuidado) |
| **Monitor mode + inyección WiFi** | adaptador USB externo (RTL8812AU…) + `.ko` contra cabeceras 5.4 | ❌ no |
| BT Arsenal / EvilTwin (BT "HID" sobre Bluetooth) | parche de BT proprietary de NetHunter | ⚠️ sí (o hardware externo) |
| WiFi injection sobre el **chip interno** | imposible (driver propietario) | ⚠️ sí (y no hay kernel de NetHunter para el 2100) |
| Grabar sesiones, MITM de red, pivoting | rootfs Kali + `tcpdump`/`bettercap`/`responder` | ❌ no |

---

## 6. Fuentes consultadas (todas comprobadas el 2026-11-22 salvo lo marcado)

**Kali / NetHunter (oficial)**
1. <https://nethunter.kali.org/kernels> — 275 kernels / 126 modelos; sin 2100.
2. <https://nethunter.kali.org/device-kernels.html> — 126 device modules / 191 kernels / 275 versiones; sin S21.
3. <https://nethunter.kali.org/image-models.html> — 73 modelos con imagen; sin S21.
4. <https://gitlab.com/kalilinux/nethunter/build-scripts/kali-nethunter-kernels> (`devices.yml`) — 126 modelos / 272 ids; sin 2100.
5. <https://gitlab.com/kalilinux/nethunter/build-scripts/kali-nethunter-kernel-builder> — para portar (semanas).
6. <https://gitlab.com/kalilinux/nethunter/build-scripts/kali-nethunter-rootless> — instalador rootless.
7. <https://kali.download/nethunter-images/current/> — 93 zips; `generic-arm64-{nano,minimal,full}`.
8. <https://kali.download/nethunter-images/current/rootfs/> + `SHA256SUMS` — tamaños y hashes.
9. <https://www.kali.org/docs/nethunter/> — ediciones y tabla de capacidades.
10. <https://www.kali.org/docs/nethunter/nethunter-rootless/> — pasos rootless.
11. <https://www.kali.org/docs/nethunter/installing-nethunter/> — instalador "generic ARM64" (sin kernel de dispositivo).
12. <https://www.kali.org/docs/nethunter/building-nethunter/> — `--kernel` admitidos (no hay 2100).
13. <https://www.kali.org/docs/nethunter/wireless-cards/> — chipsets WiFi soportados (RTL8812AU…).
14. <http://http.kali.org/dists/kali-dev/main/installer-arm64/current/images/device-tree/exynos/> — sin dtb del 2100.
15. `https://offs.ec/2MceZWr` → 302 al instalador rootless (9.577 B).

**Exynos 2100 / S21**
16. <https://github.com/xfwdrev/android_kernel_samsung_ex2100> (+ releases) — Chiclet v7.1 (2026-11-16), 5.4.305.
17. <https://github.com/Macktherealonereally/Exynos-LineageOS-Mack-ROM> (+ releases) — LineageOS 23.2 sólo `o1s`.
18. <https://github.com/Macktherealonereally/android_kernel_samsung_universal2100> — ramas y `Makefile` (5.4.302).
19. <https://github.com/UN1CA/kernel_samsung_exynos2100> — fuente de kernel.
20. <https://github.com/ProtonKernel/Proton> — Proton+.
21. <https://github.com/TQMatvey/g998b-kernel> — árbol GKI para S21 Ultra; **README 404 y sin releases ⇒ contenido NO VERIFICADO**.
22. <https://github.com/Exy2100-LOS> — manifest de la familia S21 (`o1s`, `p3s`); **parado en 2023**.
23. <https://github.com/exy2100> — árbol original; último push 2026-07-21.
24. <https://wiki.lineageos.org/devices/> — sin "starlight" como S21.

**NO VERIFICADO**
25. Un proyecto "**Nethunter-Nexus**" rootless: **no encontrado** en las
    búsquedas del 2026-11-22. El rootless oficial es el de
    `kali-nethunter-rootless` (verificado, punto 6). Si alguien se refiere a
    ese nombre, no hay fuente oficial con ese nombre.
26. Que el zip `generic-arm64` instale **efectivamente** el rootfs como
    módulo Magisk en este móvil: la **documentación** lo indica
    ("install as a module in Magisk"), pero **no se ha ejecutado** aquí.
27. Que un gadget `f_hid` propio funcione en la práctica: la **config del
    kernel lo permite** (medido) pero **no se ha probado**; requiere cable
    y puede tumbar el túnel ⇒ fase opt-in.
