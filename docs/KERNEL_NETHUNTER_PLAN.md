# Plan: kernel actualizado + NetHunter en el Galaxy S21 Ultra (Exynos 2100)

> **Estado: PREPARADO, NO EJECUTADO.** A la fecha de este documento (2026-11-22)
> **no se ha flasheado nada, no se ha reiniciado el móvil y no se ha tocado la
> partición de arranque.** Lo único que se ha hecho es **medir** y **respaldar**.
>
> Investigación completa con fuentes: [`KERNEL_NETHUNTER_RESEARCH.md`](./KERNEL_NETHUNTER_RESEARCH.md).
> Contexto previo del repo: [`KALI_NETHUNTER_KERNEL_PLAN.md`](./KALI_NETHUNTER_KERNEL_PLAN.md) (2026-11-20).

---

## 0. Resumen ejecutivo (lo primero que hay que leer)

| pregunta | respuesta |
|---|---|
| ¿Existe un kernel **NetHunter** para el Exynos 2100? | **NO.** No está en la lista oficial (275 kernels / 126 modelos), ni en `devices.yml`, ni hay `exynos2100*.dtb` en Kali. **NetHunter Full es imposible aquí.** |
| ¿Y un kernel "moderno" para el 2100? | Sí, pero **todo el ecosistema real se queda en Linux 5.4.3xx** (Chiclet v7.1 del 2026-11-16 = 5.4.305; LineageOS-exynos = 5.4.302). Un 5.10/6.x **no** puede arrancar este ROM (los módulos de vendor son 5.4 + `MODVERSIONS`). |
| ¿Cambiar el kernel da algo que no se pueda tener sin tocarlo? | Casi nada: el kernel de fábrica **ya tiene** `f_hid`, `g_serial`, `NCM`, `RNDIS` y `mass_storage` **built-in** y expone el UDC `dwc3` ⇒ el HID por USB (la estrella de NetHunter) se puede hacer con **configfs**. Lo único irreducible es la **inyección WiFi sobre el chip interno**, que se resuelve con **adaptador USB externo**. |
| ¿Y el riesgo? | Alto e innecesario: el bootloader está **cerrado** (Knox intacto); parchear `boot`/`vendor_boot` sin probarlo bien es exactamente el camino del bootloop. |
| **Recomendación** | **Fase A (sin tocar el kernel)**: NetHunter Rootless/Lite + gadget HID opcional + adaptador WiFi externo. **Fase B (opcional, con opt-in explícito)**: kernel 5.4.3xx parcheado con Magisk, sólo si A no da el WiFi que se quiere. |
| Punto de retorno | **Ya existe y está verificado**: volcado de `boot`, `vendor_boot`, `recovery`, `vbmeta`, `vbmeta_system` en Drive privado + SHA-256 + MD5 (fase 0, hecha el 2026-11-22). |

---

## 1. Estado real del kernel (medido el 2026-11-22 vía `root_exec`, timeout 11 s)

### 1.1 Identidad

```
uname -a        → Linux localhost 5.4.242-28575149-abG998BXXSBGXED #1 SMP PREEMPT
                  Tue Jun 4 16:06:37 KST 2024 aarch64 Toybox
/proc/version  → Linux version 5.4.242-28575149-abG998BXXSBGXED (dpi@SWDK6219)
                  (Android (6443078 based on r383902) clang version 11.0.1 ...) #1 SMP PREEMPT
getprop ro.product.model      → SM-G998B
getprop ro.soc.model          → Exynos 2100
ro.build.version.release/sdk  → 14 / 34
ro.build.display.id          → G998BXXSBGXED_Dr.Ketan_U08_XDA
ro.build.version.security_patch → 2024-06-01
magisk -v                     → v27.2-kitsune-4:MAGISK:R   (27002)
getenforce                    → Enforcing
```

**Bootloader**: `ro.boot.flash.locked=1`, `ro.boot.vbmeta.device_state=locked`,
`ro.boot.verifiedbootstate=green` (vía `getprop`).
⚠️ **Discrepancia medida**: `/proc/cmdline` trae
`androidboot.verifiedbootstate=orange` y `androidboot.warranty_bit=1`, mientras
que `getprop` dice `green`/`locked`. Se deja constancia: la fuente de verdad
para "puedo flashear sin desbloquear" es `flash.locked=1` + `vbmeta.device_state=locked`.
`/proc/cmdline` además trae `oops=panic` y `softdog.soft_panic=1` ⇒ **un
kernel que entre en panic reinicia el móvil**, no se queda colgado (bueno
para la reversión, malo para depurar).

### 1.2 Config y módulos

* **`/proc/config.gz` EXISTE** (45.008 B, **7.472** líneas) ⇒ la config del
  kernel en marcha es legible (imprescindible para construir un `.ko` externo).
* `CONFIG_IKCONFIG_PROC=y`, `CONFIG_MODULES=y`, `CONFIG_MODVERSIONS=y`,
  `CONFIG_ANDROID=y`, `CONFIG_BPF=y`, `CONFIG_NETFILTER=y`,
  `CONFIG_EXT4_FS=y`, `CONFIG_SECURITY_SELINUX=y`.
* **USB gadget (clave)**: `CONFIG_USB_GADGET=y`, `CONFIG_USB_CONFIGFS=y`,
  **`CONFIG_USB_F_HID=y`**, `CONFIG_USB_CONFIGFS_F_HID=y`,
  `CONFIG_USB_LIBCOMPOSITE=y`, `CONFIG_USB_U_SERIAL=y`,
  `CONFIG_USB_CONFIGFS_SERIAL=y`, `CONFIG_USB_F_ACM=y`,
  `CONFIG_USB_CONFIGFS_ACM=y`, `CONFIG_USB_CONFIGFS_NCM=y`,
  `CONFIG_USB_CONFIGFS_RNDIS=y`, `CONFIG_USB_CONFIGFS_MASS_STORAGE=y`,
  `CONFIG_USB_DWC3=y`, `CONFIG_USB_DWC3_GADGET=y`, `CONFIG_USB_DWC3_EXYNOS=y`,
  `CONFIG_USB_OTG=y`.
* **Módulos cargados: 275** (`/proc/modules`; 276 con `lsmod`) — todos de
  Samsung (`sec_*`, `snd_soc_*`, `broadcom_sensorhub`, `uwb`, `usb_f_*`…).
* `dmesg`: **19.754** líneas, 638 coincidencias de `error|fail|panic|oops`
  (ruido normal de un ROM de Samsung; no es señal de nada).
* `configfs` montado en **`/config`**; gadget de Samsung en
  `/config/usb_gadget/g1`; UDC reales en `/sys/class/udc/`:
  **`10e00000.usb`** (dwc3, el bueno) y `dummy_udc.0`.

### 1.3 Particiones y espacio

* `/dev/block/by-name/`: `blcmd boot bota cache cp_debug cpefs dqmdbg dtbo efs
  fmp_selftest harx keyrefuge keystorage metadata misc nad_refer omr optics
  param persistent prism radio recovery reserved sda sdb sdc sdd sde sec_efs
  spu steady super uh uhcfg up_param userdata vbmeta vbmeta_system vendor_boot`
  ⇒ **NO existe `init_boot`** (Samsung usa `recovery` en su lugar) ni
  `vendor_dlkm`.
* `/data`: **225 GB totales, 191 GB usados, ~33 GB libres** (f2fs).
* `/data/adb` = 1,4 GB; **25 módulos Magisk** (incluye `playintegrityfix`,
  `tricky_store`, `zygisk_lsposed`, `mm`, `mmrl_wpd`, `megadaemon_csc_recording`,
  `megadaemon_energy`, `flagsecurepatcher`, `hosts`, `cromite-webview`…).
* `/data/adb/service.d/`: `91-adb-tcp.sh`, `98-cloudflared.sh`,
  `99-megadaemon.sh` (44 KB), `lineage.sh`, `prop.sh`, `package.sh`, `hash.sh`,
  `.box_cleanup.sh`.

---

## 2. FASE 0 — Punto de retorno (HECHA el 2026-11-22)

> **Estado: HECHA Y VERIFICADA.** No requiere opt-in porque **no escribe en
> ninguna partición**: sólo lee bloques y copia a fichero.

### 2.1 Qué se respaldó (medido)

| partición | bytes | SHA-256 (origen == fichero) | MD5 (local == Drive) | `dd` |
|---|---|---|---|---|
| `boot` | 67.108.864 | `f96cd410e6f048739fe36737f047e2ede6d229d428e1d0c74de80017c8beb4fd` | `0abc5b7bfaeabdeb295f6ad697a45fba` | 955 ms |
| `vendor_boot` | 67.108.864 | `b55ecfd0744ae8ae5404635b3345b846764e55eca8349a655e71aebd52d0b2fb` | `512d689d506bc938ba8c9539c56cdc8b` | 433 ms |
| `recovery` | 67.108.864 | `777350644d9f5c345970fef5f92ea5efdb1fbc9d2b0005b4a8b62dcb90703f19` | `dad6c79365c83a6f6f50df24891d7e43` | 829 ms |
| `vbmeta` | 65.536 | `b1ef8b5108b1a333c13e6db5cf1e2c1446a4e017e81d0fe9b9392de70a1238c9` | `d891a13e17c7759d2e40d2130ff0dfd7` | 111 ms |
| `vbmeta_system` | 65.536 | `254732e930687a6d5a3c0c82965f84c9a4a36767f6aa39eeacabf53af6aba5c6` | `f78ac7aeb9be2bc9fa2efe262266348a` | 110 ms |

**Total: 201.457.664 B (192,06 MiB) en 2.438 ms de `dd`** ⇒ el punto de retorno
es barato: no hay excusa para tocar el kernel sin él.

### 2.2 Dónde está

* En el móvil: `/data/local/tmp/kernel-backup/{boot,vendor_boot,recovery,vbmeta,vbmeta_system}.img`
  + `manifest.json` (sha256 del manifiesto
  `fa846c54e74614d86903bdda81dd3670a3ca5c814fb3fa73557814ceb4030a46`).
* En Drive (carpeta **privada** `megadaemon-backup`, id
  `15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0`), verificado desde fuera con la API:

| fichero | id de Drive | tamaño | MD5 |
|---|---|---|---|
| `kernel-backup-20261122-boot.img` | `1wbLmSn0d4moWCyXROe8SRBTwm7zeyNwL` | 67.108.864 | `0abc5b7bfaeabdeb295f6ad697a45fba` |
| `kernel-backup-20261122-vendor_boot.img` | `15qO6kHWzOQWkvrRQer1dxOxLPY1ig40E` | 67.108.864 | `512d689d506bc938ba8c9539c56cdc8b` |
| `kernel-backup-20261122-recovery.img` | `1Vimbu3qxsa5ix7qFiihjUZiuOjLhPz2S` | 67.108.864 | `dad6c79365c83a6f6f50df24891d7e43` |
| `kernel-backup-20261122-vbmeta.img` | `1yn6XLj94vTqbhohZvmYUyBcqBoWJWq-0` | 65.536 | `d891a13e17c7759d2e40d2130ff0dfd7` |
| `kernel-backup-20261122-vbmeta_system.img` | `1JxnwKGa2J3S5-UGcO9la14oHin6h8857` | 65.536 | `f78ac7aeb9be2bc9fa2efe262266348a` |

  Subida medida: 201,5 MB en **69 s** (≈2,9 MB/s), token minteado **en el
  móvil** con su propio `gdrive_credentials.json` (0600) ⇒ la credencial no
  sale del dispositivo.
* Además se subirán **al estado del móvil** (`/data/adb`, `service.d`, CSC) en
  la fase de "backups en general" (backlog P0 del repo), **no** en este plan.

### 2.3 Cómo repetirlo (automatizado)

```bash
# en el móvil (root) — el CLI hace volcado + SHA-256 + manifiesto
md-kernel backup --dir /data/local/tmp/kernel-backup
#   -> boot, vendor_boot, recovery, init_boot (esta última: AUSENTE, se ignora)
md-kernel verify --manifest /data/local/tmp/kernel-backup/manifest.json --check-live
md-kernel upload-drive --manifest /data/local/tmp/kernel-backup/manifest.json \
    --folder 15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0 \
    --prefix kernel-backup-$(date -u +%Y%m%d)
```

Equivalente por MCP: tool `kernel_backup`, acciones `backup` · `verify` ·
`upload_drive` · `rollback_plan`.

---

## 3. FASE A — Todo NetHunter que se puede tener SIN tocar el kernel

> **Esta es la fase recomendada.** No toca particiones, no toca el bootloader,
> no reinicia el móvil (salvo el reinicio opcional de Magisk para instalar un
> módulo) y es **reversible borrando un directorio**.

### A.0 Comprobación previa (medida, hecha)

* Termux **instalado y con `sudo`** (`/data/data/com.termux/files/usr/bin/sudo`).
* En Termux hay: `tar xz wget curl python git sudo pip pip3`.
  **Falta `proot`** (lo instala el instalador rootless o `pkg install proot`).
* **No hay `rclone`** instalado y `$HOME/.config/rclone/` está **vacío** ⇒ el
  script `tools/nh_backup_phone.sh` (que sube con `rclone`) **no funciona tal
  cual**; la subida a Drive de este plan usa la API de Drive nativa
  (`ureq`), que sí funciona.
* No hay apps de NetHunter/KeX instaladas (`pm list packages` sólo muestra
  `com.termux*` y `catima`).
* Espacio: **~33 GB libres** ⇒ cabe el rootfs `full` (1,7 GiB) sin problema.

### A.1 NetHunter Rootless (sin root, sin kernel) — 20 min

```bash
# 1. Termux
pkg update -y && pkg upgrade -y
pkg install -y wget proot tar axel xz-utils python
termux-setup-storage

# 2. Instalador rootless OFICIAL (URL corta verificada el 2026-11-22)
wget -O install-nethunter-termux https://offs.ec/2MceZWr
sha256sum install-nethunter-termux      # se anota en el acta
chmod +x install-nethunter-termux
./install-nethunter-termux               # elegir [2] ARM64 minimal

# 3. Comprobar
nethunter --version
nethunter -c 'cat /etc/os-release; uname -m; which nmap sqlmap msfconsole'
```

* rootfs `minimal` = 120,0 MiB (SHA-256
  `96aeced81a857a987715a14417af05c2934e3e19052d6151c398a1c215448b60`).
* **Ojo**: el instalador rootless **no debe** correr con Termux en modo root
  (el propio script avisa). ⇒ se instala **sin** `su`.
* Reversión: `rm -rf $PREFIX/…/kali-arm64` + borrar el lanzador `nethunter`.

### A.2 NetHunter Lite (root real + base de datos de Metasploit) — opcional

Dos vías, **sin kernel**:

```bash
# Vía 1: instalador "generic ARM64" como MÓDULO MAGISK (doc oficial)
#   https://kali.download/nethunter-images/current/kali-nethunter-2026.3-generic-arm64-minimal.zip
#   (323.359.325 B; SHA-256 en la página de Kali)
#   -> se instala desde Magisk > Módulos > Instalar desde el almacenamiento
#   -> REINICIO (el único reinicio de esta fase) y reversible quitando el módulo

# Vía 2: chroot de rootfs con busybox, usando el root que YA hay
#   (sin proot, sin reinicio, sin Magisk)
#   tar -xJf kali-nethunter-rootfs-minimal-arm64.tar.xz -C /data/local/tmp/nh
#   mount --bind /proc /data/local/tmp/nh/proc
#   mount --bind /sys  /data/local/tmp/nh/sys
#   mount --bind /dev  /data/local/tmp/nh/dev
#   chroot /data/local/tmp/nh /bin/bash
```

### A.3 Gadget USB (HID/CDC/NCM) con el kernel DE FÁBRICA — fase aparte

**Por qué es posible (medido)**: `CONFIG_USB_F_HID=y`,
`CONFIG_USB_CONFIGFS_F_HID=y`, `CONFIG_USB_CONFIGFS_SERIAL=y`,
`CONFIG_USB_CONFIGFS_NCM=y`, `CONFIG_USB_CONFIGFS_MASS_STORAGE=y`,
`CONFIG_USB_DWC3_GADGET=y`; `configfs` en `/config`; UDC real `10e00000.usb`.

```bash
# 1) Desmontar el gadget de Samsung (SOLO con cable conectado y USB depurando)
mount -t configfs none /config          # ya está montado
echo g1 > /config/usb_gadget/g1/UDC      # desvincula el gadget de Samsung

# 2) Crear el gadget propio
mkdir -p /config/usb_gadget/nh
cd /config/usb_gadget/nh
echo 0x1209 > idVendor        # PID '1209' (el de NetHunter)
echo 0x0001 > idProduct
mkdir -p strings/0x409
echo "nethunter" > strings/0x409/serialnumber
mkdir -p functions/hid.0        # f_hid:built-in (CONFIG_USB_F_HID=y)
echo 0 > functions/hid.0/protocol   # 1 = teclado, 2 = ratón
mkdir -p functions/acm.0        # CDC-ACM
mkdir -p functions/ncm.0        # red por USB
mkdir -p configs/c.1
ln -s functions/hid.0 configs/c.1/
ln -s functions/acm.0 configs/c.1/
echo 10e00000.usb > UDC         # enlazar al dwc3 real
```

**Riesgos y salvaguardas (obligatorias)**:
* ⚠️ **Puede tumbar el túnel**: si el gadget queda mal y el móvil pierde el
  USB, se pierde el control remoto ⇒ **con el cable conectado** y con el
  usuario delante.
* ⚠️ **Restaurar**: `echo "" > UDC; rmdir functions/* configs/* …; echo g1 >
  /config/usb_gadget/g1/UDC` ⇒ devuelve el gadget de Samsung.
* ⚠️ **SELinux** puede bloquear la escritura en `/config` ⇒ si `mkdir` falla
  con `Permission denied`, parar y usar `magiskpolicy --live` (o `setenforce 0`
  temporalmente, **reversible**).
* **Sólo lectura hasta que el usuario lo pida**: la inspección ya está hecha
  (`ls`, `cat`), la escritura **no**.

### A.4 Adaptador WiFi externo (monitor mode + inyección) — sin kernel

* Hardware: **RTL8812AU / RTL8812BU / RTL88x2BU** (o Atheros AR9271 con
  `ath9k_htc`, que es el que mejor se documenta en la doc de NetHunter).
* El `.ko` se compila **fuera** contra las cabeceras 5.4 + `CONFIG_MODVERSIONS`
  ⇒ **no toca** el kernel del móvil (es un módulo externo, se carga con
  `insmod`).
* Se puede compilar en el propio móvil (Termux tiene `clang`/`make`) leyendo
  `/proc/config.gz` + las cabeceras del `vendor_boot` del propio móvil.
* Sin adaptador: el WiFi interno (Exynos, driver propietario) **no** da
  monitor mode ⇒ imposible sin kernel Y sin hardware.

### A.5 Verificación de la Fase A

```bash
# daemon vivo y respondiendo
curl -s http://127.0.0.1:9090/health
# Magisk y root
su -mm -c 'id; magisk -v'
# túnel (desde fuera)
curl -s -o /dev/null -w '%{http_code}\n' https://<TUNEL>/health
# el rootfs de Kali
nethunter -c 'id; which msfconsole; systemctl is-active kali' 2>/dev/null || true
```

**Qué se pide al usuario en A**: (1) confirmar que instala el rootfs;
(2) confirmar **presencia** cuando se cree el gadget USB (y tener el cable
conectado); (3) decidir si se instala el módulo Magisk "generic" (implica
**un reinicio**, reversible quitando el módulo).

---

## 4. FASE B — Kernel actualizado (OPCIONAL, con opt-in explícito)

> **Sólo si la Fase A no da el WiFi que se quiere** y el usuario acepta
> explícitamente el riesgo. **NO se ha hecho nada de esta fase.**

### B.0 Por qué NO se recomienda (medido + documentado)

1. **No existe kernel de NetHunter para el 2100** ⇒ el "kernel NetHunter" es
   una-fiction en este dispositivo.
2. **El bootloader está cerrado** ⇒ para un kernel propio hay que **parchear
   `boot`/`vendor_boot` con Magisk** (o desbloquear, lo que **destruye el
   KNV/Knox**, el Secure Folder, Samsung Pay y deja el móvil en la calle).
   El propio README de la LineageOS de la comunidad avisa: *"Your warranty is
   now void. Knox will trip; Samsung Pay/Wallet and Secure Folder stop working
   for good."*
3. **El 2100 real se queda en 5.4.3xx**: el `vendor_boot` de este ROM trae
   módulos de vendor compilados contra 5.4 con `CONFIG_MODVERSIONS=y` ⇒ un
   5.10/6.x **no arranca**. Toda la comunidad (Chiclet v7.1 del 2026-11-16,
   Proton+, UN1CA, LineageOS-exynos) **mantiene la rama 5.4**.
4. **El kernel de fábrica ya tiene `f_hid`/`g_serial`/NCM/mass_storage**
   built-in ⇒ la función estrella de NetHunter **no necesita** kernel propio.
5. El móvil tiene: rooting, CSC parcheado con valor legal
   (`RecordingAllowedByMenu`), backups en Drive y un daemon propio ⇒ **el
   coste de un bootloop es alto** y el beneficio, medido, es pequeño.

### B.1 Si aun así se hace: las 3 fases exactas

**B.1.1 Preparación (no destructiva)**
```bash
# 1) Re-respaldo (por si el móvil ha cambiado desde la Fase 0)
md-kernel backup --dir /data/local/tmp/kernel-backup-b --overwrite
md-kernel upload-drive --manifest /data/local/tmp/kernel-backup-b/manifest.json \
    --folder 15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0 --prefix kernel-backup-b-$(date -u +%Y%m%d)

# 2) Copia de seguridad del estado de Magisk (módulos, service.d, props)
tar -czf /data/local/tmp/adb-state-$(date -u +%Y%m%d).tar.gz -C /data adb

# 3) Identidad de la imagen destino (sha256) y verificación contra la de origen
sha256sum /data/local/tmp/kernel-backup/boot.img
```

**B.1.2 Flasheo (ÚNICO punto irreversible; con el usuario delante)**
```bash
# Sólo con: pantalla encendida, cable USB, Magisk instalado, usuario delante.
# Se parchea `boot` con Magisk (NO se flashea a pelo, NO se toca vbmeta):
#   -> Magisk > Instalar > "Parchear un archivo de boot"
#   -> seleccionar la imagen del kernel (AnyKernel3 de Chiclet v7.1 o la
#      salida de UN1CA con el tree 5.4 del propio móvil)
#   -> Magisk avisa: "Backup: /storage/emulated/0/Download/magisk_patched-*.img"
#   -> DESPUÉS: flashear ese magisk_patched-*.img en la partición `boot`
#      (Odin: AP; o `dd` con el móvil en download mode)
#
# Antes de flashear, IMPRIMIR el sha256 de magisk_patched-*.img y anotar el
# de boot.img original: si `magisk_patched` == `boot.img` original, NO se
# ha parcheado nada y no se flashea.
```

**B.1.3 Verificación del arranque (el "health check")**
```bash
# 1) ¿Arranca? (lo primero: pantalla + logo + lockscreen)
# 2) Magisk y root
su -mm -c 'id; magisk -v; getprop ro.boot.verifiedbootstate'
# 3) Fingerprint del kernel: DEBE ser distinto del original
md-kernel fingerprint --partition boot
#    (el original es f96cd410e6f048739fe36737f047e2ede6d229d428e1d0c74de80017c8beb4fd)
# 4) Daemon y túnel
curl -s http://127.0.0.1:9090/health
curl -s -o /dev/null -w '%{http_code}\n' https://<TUNEL>/health
# 5) Módulos Magisk y service.d intactos
ls /data/adb/modules | wc -l ; ls /data/adb/service.d
# 6) CSC con valor legal
su -mm -c 'getprop ro.build.version.oplusregion; ls /data/misc/ 2>/dev/null | head'
# 7) GPU/NPU/cámara (lo que suele romperse con un kernel mal portado)
su -mm -c 'ls /sys/class/graphics 2>/dev/null; ls /dev/video* 2>/dev/null | head'
```

### B.2 Rollback (el punto de retorno)

Si el móvil **no arranca** (pantalla en negro / bucle de logo / bootloop):

1. **No tocar nada más.** El `boot.img` original está en:
   * Drive (carpeta privada `megadaemon-backup`, nombres
     `kernel-backup-20261122-boot.img`, id `1wbLmSn0d4moWCyXROe8SRBTwm7zeyNwL`);
   * en el propio móvil en `/data/local/tmp/kernel-backup/boot.img` (si el
     filesystem `/data` aún monta; si no, tirar de Drive).
2. **Bajar el fichero de Drive** (desde un PC o desde el móvil con internet):
   ```bash
   # con el token de OAuth (el del móvil o el del PC con la misma cuenta)
   curl -L -o boot.img \
     "https://www.googleapis.com/rive/v3/files/1wbLmSn0d4moWCyXROe8SRBTwm7zeyNwL?alt=media"
   # OJO: la URL real es https://www.googleapis.com/drive/v3/files/<ID>?alt=media
   sha256sum boot.img
   # debe imprimir exactamente:
   # f96cd410e6f048739fe36737f047e2ede6d229d428e1d0c74de80017c8beb4fd
   ```
3. **Restaurar** (escribir en la partición ⇒ **con el móvil en recovery**):
   ```bash
   # recovery: ADB sideload / Odin, o con root si arranca
   dd if=boot.img of=/dev/block/by-name/boot bs=4M conv=fsync
   sha256sum /dev/block/by-name/boot
   # debe volver a imprimir f96cd410… (el hash del bloque, no del fichero)
   ```
4. **Plan de rollback completo** (generado por el módulo, **nunca** ejecutado
   solo):
   ```bash
   md-kernel rollback-plan --manifest /data/local/tmp/kernel-backup/manifest.json
   ```
   ⇒ imprime, por partición, el `sha256sum` previo, el `dd` exacto y el
   `sha256sum` posterior, para `boot`, `vendor_boot` y `recovery`.

### B.3 Qué se pide al usuario en cada punto de B

| punto | qué se pide |
|---|---|
| B.1.1 | "Voy a preparar el parche; esto **no** cambia nada todavía" → OK para generar `magisk_patched` |
| B.1.2 | "Ahora se **escribe** en `boot`. Si algo sale mal, se restaura desde Drive. ¿Confirmas?" ⇒ **confirmación explícita** antes del `dd` |
| B.1.3 | "Ha reiniciado. ¿Ves el lockscreen?" → si no, **rollback** |
| B.2 | "No arranca: vamos a restaurar el `boot` original desde Drive" → ejecutar el `dd` **a mano**, paso a paso |

---

## 5. Automatización entregada en este bloque

| pieza | ruta | qué hace |
|---|---|---|
| Módulo Rust | `rust-core/src/kernel_and_blocks.rs` | `info`/`dmesg`/`partitions`/`processes`/`fingerprint`/`backup`/`verify`/`upload_drive`/`rollback_plan`. **Nunca escribe en `/dev/block`**. Validación fail-closed de nombres y rutas; SHA-256 de origen **y** de fichero; subida a Drive con verificación doble (tamaño + SHA-256 en `appProperties`); plan de rollback **como texto** |
| Tool MCP | `kernel_backup` (registrada en `android.rs`) | las mismas acciones, con `folder_id`, `credentials` y `name_prefix` |
| CLI | `rust-core/src/bin/md-kernel.rs` | `md-kernel info\|backup\|verify\|upload-drive\|rollback-plan` (`--json` para máquina) |
| Script de subida | `tools/kernel_backup_drive.sh` | **ejecutado en el móvil**: mintea el token **en el móvil** (la credencial no sale), sube en trozos de 8 MiB con `Content-Range` y verifica con MD5 de Drive |
| Tests | `rust-core/tests/kernel_backup_smoke.rs` + tests unitarios del módulo | 20 tests verdes (13 unitarios + 7 de humo) en el arnés aislado |

> ⚠️ **El crate completo del repo NO compila hoy** por fallos **preexistentes**
> en módulos de otros agentes (`rom_migration`, `alerts`, `drive_native`,
> `git_ops`, `root_stealth`, `csc_verify`…). El módulo nuevo compila y sus
> tests pasan **aislado** (`#[path = "../src/kernel_and_blocks.rs"]`), que es
> justo para lo que se diseñó. No se ha tocado ningún módulo ajeno.

---

## 6. Decisión que se pide al usuario

1. **Fase A.1** (NetHunter Rootless, sin root, sin kernel) — **recomendada**:
   ¿la instalo?
2. **Fase A.3** (gadget HID/CDC por USB con el kernel de fábrica): requiere
   **cable** y presencia; ¿la intento?
3. **Fase B** (kernel propio 5.4.3xx parcheado con Magisk): **no recomendada**;
   sólo con un "sí" explícito y después de A.
4. ¿Se sube también el **estado de Magisk** (`/data/adb`) y el CSC a Drive
   (backlog P0 del repo)?

**Hasta que haya respuesta, no se toca ni el kernel ni el bootloader, y el
punto de retorno de la Fase 0 sigue intacto y verificado.**
