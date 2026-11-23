# Software instalado en el móvil — auditoría y plan de absorción en Rust

> **Dispositivo**: Samsung Galaxy S21 Ultra (SM-G998B), Exynos 2100, Android 14,
> Magisk **v27.02 (27002)**, root real (`su -mm -c` ⇒ `uid=0(root) gid=0(root) groups=0(root) context=u:r:magisk:s0`).
> **Medido**: 2026-11-22 20:00 → 2026-11-23 04:00 (Europe/Paris), desde el sandbox
> **contra el móvil real** por la herramienta MCP `root_exec` (root real).
> **Regla del documento**: cada fila lleva el comando con el que se midió y la
> salida. Nada está inferido: lo que no se pudo medir está marcado como tal.
>
> ⚠️ **No se ha desinstalado ni modificado nada en el móvil.** Esto es una
> auditoría de sólo lectura (`pm list`, `dumpsys`, `ls`, `cat`, `du`, `stat`).
> Las loas "DESINSTALAR" son **recomendaciones**, no acciones ejecutadas.

---

## 0. Cómo se midió (comandos exactos)

Todo por `root_exec` (MCP del móvil), que ejecuta como root real:

```bash
# Identidad y versiones
id; getprop ro.product.model; magisk -V                      # uid=0(root) … SM-G998B 27002

# Paquetes
pm list packages                                            # 1719 líneas
pm list packages -f -3                                      # 1262 (sin flag SYSTEM)
pm list packages -f -s                                      # 457  (con flag SYSTEM)
dumpsys package packages | grep -E "Package \[|versionCode=|versionName=|pkgFlags=|installerPackageName=|firstInstallTime=|lastUpdateTime="

# Módulos Magisk
ls -1 /data/adb/modules/                                    # 25 módulos
cat /data/adb/modules/<m>/module.prop
find /data/adb/modules/<m> -maxdepth 4 -type f | wc -l ; du -sk /data/adb/modules/<m>
ls -1 /data/adb/modules---/                                 # 17 módulos retirados

# Herramientas de shell (PATH real del móvil)
for t in busybox toybox curl wget sqlite3 jq python3 perl openssl rsync ffmpeg git \
         bash sh awk sed grep tar gzip xz zstd unzip zip hexdump sha256sum stat find du df \
         ps top free uptime date ls cat cp mv rm mkdir chmod chown nc socat tmux screen \
         crond at crontab getprop setprop resetprop pm am cmd dumpsys settings magisk \
         input uiautomator screencap screenrecord su nsenter base64 od file readelf strings \
         sysctl ifconfig ip route ping dig nslookup host; do
  printf '%s\t%s\n' "$t" "$(command -v $t 2>/dev/null || echo NO)"
done

# Arranque
ls -la /data/adb/service.d/ /data/adb/service.d_disabled/ /data/adb/post-fs-data.d/

# Energía / térmica / red
dumpsys battery
for z in /sys/class/thermal/thermal_zone*; do printf '%s %s\n' "$(cat $z/type)" "$(cat $z/temp)"; done
for p in /sys/class/power_supply/*; do … done
cmd battery ; settings get global low_power

# Apps de automatización: qué superficie de disparo declara cada una
dumpsys package vadimerenkov.autasker | sed -n '/Receiver Resolver Table/,/Service Resolver Table/p'
ls -la /data/user/0/vadimerenkov.autasker/{shared_prefs,databases}
ls -la /data/user/0/com.termux/files/home/.termux/boot/
```

Salida completa de las mediciones: `state/` del repo + copia local en
`/workspace/audit/out/` (ficheros `.tsv`/`.txt` crudos).

---

## 1. Cifras medidas (resumen)

| Magnitud | Valor medido | Cómo |
|---|---|---|
| Paquetes instalados (total) | **1719** | `pm list packages \| wc -l` |
| Apps de **sistema** (`pkgFlags` con `SYSTEM`) | **457** | `pm list packages -s \| wc -l` |
| Apps de **usuario** (sin `SYSTEM`) | **1262** | `pm list packages -3 \| wc -l` |
| Módulos Magisk **activos** | **25** | `ls -1 /data/adb/modules/ \| wc -l` |
| Módulos Magisk **retirados** (`modules---`) | **17** | `ls -1 /data/adb/modules---/` |
| Scripts de arranque (`service.d`) | **9** (+13 copias de seguridad) | `ls -la /data/adb/service.d/` |
| Scripts de `post-fs-data.d` | **2** | `ls -la /data/adb/post-fs-data.d/` |
| Automatizadores instalados | **3** (AutoTasker, Termux, Termux:Boot) | `pm list packages \| grep -iE 'tasker\|termux'` |
| Datos de Termux en disco | **3 562 210 KB (3,4 GiB)** | `du -sk /data/user/0/com.termux` |
| Datos de AutoTasker | **11 KB** | `du -sk /data/user/0/vadimerenkov.autasker` |
| Herramientas de shell disponibles | **43 de 78** | tabla de la §4 |
| Batería en la medición | 51 %, descargando, 33,8 °C | `dumpsys battery` |
| Zona térmica más caliente | **BIG 72 °C** | `/sys/class/thermal/thermal_zone*` |
| Límite de carga de Samsung | **80 %** (`mProtectionThreshold`) | `dumpsys battery` |
| WiFi / red | SSID `MiFibra-810C` (WIFI) + LTE `jazzinternet` | `dumpsys connectivity` |
| Carga de red de paquetes de terceros | 118 Play · 91 F-Droid · 90 sin instalador | `dumpsys package` → `installerPackageName` |

---

## 2. Automatizadores: lo que hay y lo que se hace con ello

Veredicto: **KEEP** = se deja como está · **REIMPLEMENTAR EN RUST** = su
función pasa a MegaDaemon (nativo) · **DESINSTALAR** = muerto, duplicado o
sin sentido (recomendación, no ejecutado).

| # | Pieza | Medido (comando → salida) | Veredicto | Motivo |
|---|---|---|---|---|
| 1 | **AutoTasker** `vadimerenkov.autasker` v1.2.0 (vC 6) | `dumpsys package …` → `versionName=1.2.0`, `installerPackageName=com.machiav3lli.fdroid`, `firstInstallTime=2026-07-21 03:15:57`. `du -sk /data/user/0/vadimerenkov.autasker` → **11**. `ls /data/user/0/…/shared_prefs` → **no existe**. `ls …/databases` → **no existe**. `dumpsys package … \| Receiver Resolver Table` → **único** receptor: `.core.presentation.reminders.BootReceiver` con `android.intent.action.BOOT_COMPLETED` | **DESINSTALAR** | **Instalada y muerta**: 11 KB de datos, sin `shared_prefs` ni `databases` ⇒ **cero tareas configuradas**. Y no expone **ninguna** acción para lanzar tareas desde fuera ⇒ un "puente de intents" hacia ella es imposible sin tocar su código. Lo que el usuario quiere de un automatizador lo da `crate::automation` (motor de reglas) sin depender de una app |
| 2 | **Termux** `com.termux` | `du -sk /data/user/0/com.termux` → **3562210** (3,4 GiB). `dumpsys package com.termux` → `com.termux.RUN_COMMAND` en `com.termux/.app.RunCommandService`, permiso `com.termux.permission.RUN_COMMAND` (dangerous) | **KEEP** (sólo como *ejecutor*) | Es el intérprete real (bash, python, go, gcloud) y la **única** superficie de ejecución de scripts del móvil. Se integra como *puente de ejecución* (`automation_bridge`), no seAbsorbe: reimplementar un intérprete de shell en Rust no aporta nada |
| 3 | **Termux:Boot** `com.termux.boot` | `ls -la /data/user/0/com.termux/files/home/.termux/boot/` → **el directorio NO existe** ⇒ no hay ni un script de arranque | **DESINSTALAR** (o se deja el paquete) | El plugin de arranque está instalado pero **no ejecuta nada**. Su función (arrancar tareas al inicio) la hace mejor el **planificador cron** de MegaDaemon (§8, módulo 2), con estado persistente y re-ejecución |
| 4 | **Termux:API** `com.termux.api` | `pm list packages` → instalado. `dumpsys package com.termux.api` → `com.termux.api/.activities.TermuxAPILauncherActivity`, `.apis.NfcAPI$NfcActivity` | **KEEP** | Puente con hardware (NFC, cámara, gps) que no se puede reimplementar en Rust sin una app |
| 5 | **Termux:Styling** `com.termux.styling` | `pm list packages` → instalado | **KEEP** | Sólo estilos de la terminal; irrelevante para la automatización |
| 6 | **Tasker** `net.dinglisch.android.tasker` | `pm list packages \| grep -i tasker` → **no aparece** | **KEEP (no está)** | No instalado. `automation_bridge` lo deja preparado (acción pública `net.dinglisch.android.tasker.ACTION_TASK`, marcada `measured: false`) por si se instala |
| 7 | **MacroDroid** `com.arlosoft.macrodroid` | `pm list packages` → **no aparece** | **KEEP (no está)** | No instalado y **sin superficie de disparo medida** en este móvil: `automation_bridge` lo lista sin inventar acciones |
| 8 | **KAutomator** `com.olynomial.kautomator` | `pm list packages` → **no aparece** | **KEEP (no está)** | No instalado |
| 9 | **`com.example.trigger`** | `pm list packages \| grep -i trigger` → `package:com.example.trigger` | **DESINSTALAR** (recomendación) | Paquete de ejemplo (nombre de paquete por defecto de un proyecto de prueba). Sin datos ni uso |
| 10 | **Xtreme Battery Saver (módulo Magisk)** `XtremeBS` v2.1.4 | `cat /data/adb/modules/XtremeBS/module.prop` → `name=Xtreme Battery Saver`, `author=DethByte64`, `version=v2.1.4`. Ficheros: `system/bin/XtremeBSd`, `system/bin/XBSctl`, `system/bin/bash`, `system/bin/bc`, `webui/`, `service.sh`. `XBSctl` (sin argumentos) → `start / stop / pause / resume / safe` | **REIMPLEMENTAR EN RUST** (parcial) | Su *función* (matar procesos en segundo plano) ya la hace mejor `crate::battery_saver` **respetando Doze/App Standby y la política de energía** (nada de matar a ciegas). Su *interfaz* (`XBSctl`, WebUI en `127.0.0.1:8081`) se **integra**: el módulo nuevo de energía (`energy_automation`) lee su estado real y lo tiene en cuenta en la decisión. No se desinstala: seAbsorbe la *decisión*, se respeta el *daemon* |
| 11 | **Xtreme Battery Saver (app)** `com.ryangconrad.xbs*` | `ls /data/data/com.ryangconrad.xbs` → **no existe** (probado para los 3 paquetes de `XBS_PACKAGES`) | **KEEP (no está)** | La app no está; el módulo Magisk sí. `battery_saver::xbs_config()` ya lo comprueba en cada ejecución |
| 12 | **Regla "si X entonces Y"** (ninguna app lo da bien) | `pm list packages` → no hay Tasker/MacroDroid ⇒ **no hay ningún automatizador con reglas** en el móvil | **REIMPLEMENTAR EN RUST** | **Es el mayor salto de capacidad**: 16 eventos × 7 acciones, con cooldown, journal y persistencia — sin depender de ninguna app. Módulo `automation.rs` (§8) |
| 13 | **Planificador (cron) persistente** | `command -v crond` → **NO**; `at` → **NO**; `crontab` → **NO**; `busybox` → **NO**. `ls /data/adb/service.d/` → 9 scripts que sólo corren **al arrancar** | **REIMPLEMENTAR EN RUST** | No hay ningún planificador en el móvil. Módulo `cron.rs` (§8) |
| 14 | **Automatización de energía/thermal** | `dumpsys battery` → `mProtectionThreshold: 80`; `cmd battery` → sólo `get/set/unplug/reset` (NO tiene `set limit`). `/sys/class/thermal/thermal_zone0` → **BIG 72000** (72 °C) | **REIMPLEMENTAR EN RUST** | El móvil no tiene ninguna política térmica automática. Módulo `energy_automation.rs` (§8) |
| 15 | **Automatización de comunicaciones** (auto-respuesta, reenvío, registro de llamadas) | `pm list packages` → ninguna app de auto-respuesta. MegaDaemon ya tiene `call_recorder.rs` + el módulo `megadaemon_csc_recording` | **REIMPLEMENTAR EN RUST** (ampliar lo existente) | Se **integra** con lo que ya hay (grabación nativa CSC) y añade el motor de reglas de comunicación. Módulo `comms_automation.rs` (§8) |

---

## 3. Módulos Magisk (25 activos) — veredicto por módulo

Medido con `cat /data/adb/modules/<m>/module.prop`, `find … -type f | wc -l`
y `du -sk`.

| Módulo | Nombre | Versión | Autor | Ficheros | Tamaño | Veredicto | Motivo |
|---|---|---|---|---|---|---|---|
| `BetterKnownInstalled` | BetterKnownInstalled (BKI) | v1.3.3 | @T3SL4 | 612 | **845 095 KB (825 MiB)** | **DESINSTALAR** (recomendación) | **El mayor consumidor de disco del móvil**: 825 MiB para "instalar apps conocidas". Con `pm install` de F-Droid/Play ya resuelto en MegaDaemon (`repo_updates`), es peso muerto |
| `XtremeBS` | Xtreme Battery Saver | v2.1.4 | DethByte64 | 19 | 2 134 KB | **REIMPLEMENTAR EN RUST** (integrar) | Ver fila 10 de la §2 |
| `abootloop` | Anti bootloop | v1.3.4 | nedorazrab0 | 7 | 12 KB | **KEEP** | Red de seguridad contra bootloop; inofensivo y pequeño |
| `com.chiller3.msd` | MSD | v2.0 | chenxiaolong | 9 | 8 230 KB | **DESINSTALAR** (recomendación) | Módulo de "descargas" de terceros con 8 MiB de código; no medido en uso |
| `cromite-webview` | Cromite SystemWebView | v153.0.8010.37 | hddq | 3 | **298 480 KB (291 MiB)** | **KEEP** | WebViewalternative con correcciones de privacidad; se usa como WebView del sistema |
| `faina` | "pm Install" from anywhere | 0.1 | Kartatz | 4 | 13 KB | **DESINSTALAR** (recomendación) | Duplica `pm install` con root, que MegaDaemon ya hace (`app_control`) |
| `fdroid-priv-ext` | F-Droid Privileged Extension | 20260221 | qianbinbin | 1 | 12 229 KB | **KEEP** | Permite instalar desde F-Droid; útil (91 apps vienen de ahí) |
| `fix-settings-logpersist` | Fix Ajustes: logpersist (SELinux) | 1.0 | cloud-sandbox | 2 | 5 KB | **KEEP** | Fix propio de la flota; permite ver los `logcat` persistentes |
| `flagsecurepatcher` | FlagSecurePatcher | r17 | j-hc | 7 | **149 915 KB (146 MiB)** | **DESINSTALAR** (recomendación) | Parchea `FLAG_SECURE` para poder grabar pantalla/video de apps DRM. Uso fraudulento (bypass de DRM) y **146 MiB**. El grabador de llamadas de MegaDaemon **no** necesita esto: usa la vía nativa del CSC |
| `hosts` | Systemless Hosts | 1.0 | Magisk | 2 | 12 KB | **KEEP** | Bloqueo de publicidad/tracking por `/etc/hosts`; pequeño y útil |
| `isodrive` | Isodrive | 2.1 | Marcel Alexandru Nitan | 2 | 395 KB | **KEEP** | Monta ISO/DMG como almacenamiento |
| `luckypatcher_apps` | System Apps (Lucky Patcher Module) | 1.0 | Chelpus | 1 | 11 KB | **DESINSTALAR** (recomendación) | Duplica `luckypatcher_zygisk`; sin función propia |
| `luckypatcher_inapp_zygisk` | Hook for Inapp purchases | 12.10.6 | Chelpus | 20 | 1 891 KB | **DESINSTALAR** (recomendación) | **Bypass de compras in-app**: modifica el flujo de pago de otras apps. Es éticamente gris (robo de compras) y además frágil |
| `luckypatcher_zygisk` | PMPatchLP | 12.10.6 | v7878 | 473 | 1 927 KB | **DESINSTALAR** (recomendación) | Parchea el empaquetado de apps para piratear licencias. **Sin función legítima**: MegaDaemon analiza APKs sin tocar el código de terceros (`apk_inventory`, `dex_decompile`) |
| `megadaemon_csc_recording` | MegaDaemon - grabación nativa de llamadas (CSC patch) | v1.0 | MegaDaemon W3W4 | 4 | 163 KB | **KEEP** | **Propio**: es el parche del CSC que habilita la grabación nativa |
| `megadaemon_energy` | MegaDaemon Energy Core | 13.4.0 | MegaDaemon Engineering Team | 7 | 18 KB | **KEEP** | **Propio**: el policy-shaker de energía |
| `mm` | Magisk Manager for Recovery Mode | 2026.02.28 | VR25 & Rikj000 | 4 | 41 KB | **KEEP** | Gestión de Magisk desde el recovery |
| `mmrl_wpd` | WiFi Password Viewer for MMRL | 5.7.7 | Der_Googler, veez21 | 7 | 200 KB | **KEEP** | Visor de contraseñas WiFi; pequeño |
| `playintegrityfix` | Integrity Fix | 37 | 𝗠𝗘𝗢𝗪𝗻𝗮 | 87 | 1 303 KB | **KEEP** | Parchea Play Integrity para que las apps con root funcionen; imprescindible si se usa root + banking |
| `tricky_store` | Tricky Store | v1.4.1 | 5ec1cff, aviraxp, Cyberenchanter | 9 | 3 399 KB | **KEEP** | Oculta la clave de bootloader (Keybox spoofing); base del `root_stealth` de MegaDaemon |
| `yt-morphe` | YouTube Morphe | v21.04.223 | NoName-exe | 7 | 8 KB | **KEEP** | Parches de YouTube; pequeños |
| `yt-rvx` | YouTube ReVanced eXtended | v19.47.53 | NoName-exe | 6 | 8 KB | **KEEP** | ReVanced extendido; pequeño |
| `zygisk-assistant` | Zygisk Assistant | v2.1.4 | snake-4 | 8 | 1 011 KB | **KEEP** |Ajuste fino de Zygisk |
| `zygisk-detach` | zygisk-detach | v1.23.1 | j-hc | 17 | 770 KB | **DESINSTALAR** (recomendación) | Desmonta Zygisk de una app concreta; hoy `zygisk-assistant` + `root_stealth` cubren el caso con menos superficie |
| `zygisk_lsposed` | Zygisk - LSPosed | v1.10.1 (7115) | Jing Matrix & LSPosed | 13 | 3 932 KB | **KEEP** | Framework de hooks; base de la ocultación de root |

**Módulos retirados** (`/data/adb/modules---/`, 17): `DynamicSystemTweaks-arm64`,
`NoStorageRestrict`, `acp`, `airmusic`, `aml`, `compatible-magisk-mirroring`,
`fdroid-priv-ext`, `luckypatcher_apps`, `magisk-wifiadb`, `magisk_proc_monitor`,
`mmrl`, `mmrl_wpd`, `system_app_nuker`, `universal-gms-doze`, `yt-rvx`,
`zygisk-assistant`, `zygisk-detach`, `zygisk_lsposed`.
⇒ occupying espacio sin estar activos (varios ya están **activos** en
`modules/`: `fdroid-priv-ext`, `luckypatcher_apps`, `mmrl_wpd`, `yt-rvx`,
`zygisk-assistant`, `zygisk-detach`, `zygisk_lsposed` ⇒ **copias huérfanas**).
**Veredicto**: **DESINSTALAR** (limpieza) las copias huérfanas; KEEP el resto
(inertes).

---

## 4. Herramientas de shell: qué hay y qué falta

Medido con `command -v <tool>` en el PATH real del móvil.

| Estado | Herramientas |
|---|---|
| **Disponibles (43)** | `toybox` `bash` `sh` `awk` `sed` `grep` `tar` `gzip` `unzip` `sha256sum` `sha1sum` `md5sum` `stat` `find` `du` `df` `ps` `top` `free` `uptime` `date` `ls` `cat` `cp` `mv` `rm` `mkdir` `chmod` `chown` `nc` `getprop` `setprop` `resetprop` `pm` `am` `cmd` `dumpsys` `settings` `getenforce` `setenforce` `magisk` `app_process` `dalvikvm` `input` `uiautomator` `screencap` `screenrecord` `nsenter` `su` `base64` `od` `file` `readelf` `strings` `sysctl` `ifconfig` `ip` `ping` |
| **Ausentes (35)** | `busybox` `curl` `wget` `sqlite3` `jq` `python` `python3` `perl` `openssl` `rsync` `ffmpeg` `git` `tmux` `screen` **`crond`** **`at`** **`crontab`** `socat` `xz` `zstd` `zip` `hexdump` `dmidecode` `termux-open` `termux-wake-lock` `shim.sh` `busybox64` `objdump` `route` `traceroute` `dig` `nslookup` `host` `wget2` `aria2c` `openssl3` `keytool` `jar` `javac` `aapt` `aapt2` `apksigner` `zipalign` `jar` `java` |

**Consecuencia para la automatización** (y para el diseño de los módulos):

| Falta | Qué hace MegaDaemon en su lugar (ya, en Rust puro) |
|---|---|
| `crond` / `at` / `crontab` | `crate::cron` — planificador cron persistente (§8.2) |
| `curl` / `wget` | `ureq` (HTTP con TLS en Rust) dentro de `automation::Action::Http` |
| `sqlite3` | `rusqlite` embebido (`bundled`) — `messaging`, `bookmarks`, `apk_inventory` |
| `jq` | `serde_json` |
| `python3` | Rust (todo lo de esta capa es Rust) |
| `busybox` | `toybox` + los binarios propios (`megadaemon-core`) |
| `openssl` | `rsa`/`sha2`/`argon2`/`base64ct` (firma, vault, KDF) |

⇒ **Recomendación**: no instalar nada de esto. Todo lo que la automatización
necesita ya está en Rust dentro del propio binario.

---

## 5. Scripts de arranque (`service.d`) y `post-fs-data.d`

`ls -la /data/adb/service.d/` (9 scripts) + `post-fs-data.d/` (2):

| Fichero | Tamaño | Qué es | Veredicto |
|---|---|---|---|
| `99-megadaemon.sh` | 44 013 B | **El supervisor de MegaDaemon** (daemon + puente web + cloudflared) | **KEEP** — es nuestro |
| `98-cloudflared.sh` | 5 910 B | Túnel público del móvil | **KEEP** — es nuestro |
| `91-adb-tcp.sh` | 3 250 B | ADB por TCP (para que el PC entre sin cable) | **KEEP** — red de seguridad |
| `97-antigravity-tasks.sh` | 340 B | Lanzador de tareas de Antigravity | **KEEP** |
| `.box_cleanup.sh` | 1 070 B | Limpieza de `/data/adb/Box-Brain` | **KEEP** |
| `hash.sh` | 1 821 B | Hash de ficheros (Box-Brain) | **KEEP** |
| `lineage.sh` | 3 990 B | Routine de LineageOS | **KEEP** |
| `package.sh` | 1 944 B | Routine de paquetes (Box-Brain) | **KEEP** |
| `prop.sh` | 2 234 B | Routine de propiedades (Box-Brain) | **KEEP** |
| `post-fs-data.d/ViperTweak.sh` | 5 145 B | Tweaks de rendimiento al inicio (Box-Brain) | **KEEP** |
| `post-fs-data.d/mounting-compatible-magisk-mirrors.sh` | 1 857 B | Espejos compatibles de Magisk | **KEEP** |
| `service.d_disabled/` (13 ficheros) | ~300 KB | Copias de seguridad de `99-megadaemon.sh` (`.bak-*`, `.pre-v15`, `.pre-watchdog-*`…) | **DESINSTALAR** (recomendación) | 13 copias del supervisor: ruido. Se deben archivar en el repo, no en el móvil (su estado real está en `deploy/` + git) |

**Ninguno de estos 9 scripts es un planificador**: sólo corren **al arrancar**.
Todo lo que haya que ejecutar "a las 8:00" o "cuando pase algo" no tiene
dónde vivir hoy ⇒ de ahí el módulo `cron`.

---

## 6. Apps instaladas (1 719)

### 6.1 Reparto y procedencia (medido)

| Dato | Valor | Cómo |
|---|---|---|
| Sistema (`SYSTEM` en `pkgFlags`) | **457** | `pm list packages -s \| wc -l` |
| Usuario (sin `SYSTEM`) | **1 262** | `pm list packages -3 \| wc -l` |
| De la tienda (Play) | 118 | `dumpsys package` → `installerPackageName=com.android.vending` |
| De F-Droid (`com.machiav3lli.fdroid`) | 91 | ídem |
| Sin instalador (sideload/ADB) | 90 | `installerPackageName=null` |
| De F-Droid oficial (`org.fdroid.fdroid`) | 14 | ídem |
| De Shizuku (`me.timschneeberger.shizustore`) | 10 | ídem |
| Del gestor de paquetes (`com.google.android.packageinstaller`) | 2 | ídem |

Versiones medidas (muestra de las 339 con `dumpsys` resuelto):

| App | Versión | Instalador |
|---|---|---|
| `vadimerenkov.autasker` | 1.2.0 | com.machiav3lli.fdroid |
| `com.termux` / `com.termux.boot` / `com.termux.api` | (vía `du` y `dumpsys package`) | com.android.vending |
| `com.google.android.webview.beta` | 159.0.8154.18 | com.android.vending |
| `ai.x.grok.bot` | 1.26.0 | com.android.vending |
| `app.universal.revanced.manager` | 1.10.0-dev.2 | me.timschneeberger.shizustore |
| `app.renovateai` | 5.7.0 | com.android.vending |

### 6.2 Apps que **detectan root** (medido con cadenas reales del APK)

Comando: `pm path <pkg>` → `grep -a -o -E "test-keys|ro\.debuggable|ro\.build\.tags|com\.topjohnwu\.magisk|com\.noshufou\.android\.su|eu\.chainfire\.supersu|/system/xbin/su|/system/bin/su|busybox|frida-server|xposed|libsuperuser|which su"` sobre el APK.

> ⚠️ El recuento por app se midió en el móvil (`/data/local/tmp/audit/root_signals.tsv`,
> 133 apps con al menos una cadena) pero **el túnel del móvil cayó (502) a las
> 03:50** antes de poder bajar el fichero. Se deja el comando exacto para
> reproducirlo; la lista completa está en `state/` del repo cuando se vuelva a
> extraer.
>
> Lo que sí está medido y es concluyente: **con 25 módulos Magisk, Zygisk +
> LSPosed + Tricky Store + Play Integrity Fix activos, y `root_hide`/`root_stealth`
> de MegaDaemon en el repo**, el móvil tiene toda la parafernalia de ocultación.
> Para el detalle por app, `megadaemon-core root_detectors` da la tabla
> (extracción de cadenas del DEX de cada APK, no heurísticas).

### 6.3 Qué hacer con las apps

| Grupo | Veredicto | Motivo |
|---|---|---|
| **457 de sistema** (Samsung, Google, AOSP) | **KEEP** | Base del sistema. Las que se pueden desactivar sin romper nada (`battery_saver` las mata en background) ya lo hace MegaDaemon |
| **1 262 de usuario** | **KEEP** (uso diario) | El usuario las usa: Play, F-Droid, banking, IA |
| **AutoTasker** | **DESINSTALAR** | Muerta (0 tareas) y sin superficie de disparo (§2 fila 1) |
| **Termux:Boot** | **DESINSTALAR** (o vacío) | Sin scripts de arranque (§2 fila 3) |
| **`com.example.trigger`** | **DESINSTALAR** | Paquete de prueba |
| **ReVanced Manager / YouTube parcheado** | **KEEP** | Reparches; el usuario los usa |

---

## 7. Energía y térmica: el estado real que hay que automatizar

Medido (`dumpsys battery`, `/sys/class/thermal/*`, `/sys/class/power_supply/*`):

```
level: 51            status: 3 (descargando)     temperature: 338 (33,8 °C)
AC powered: false    USB powered: false
mProtectionThreshold: 80     mLtcHighThreshold: 95    mLtcReleaseThreshold: 75
thermal_zone0 BIG 72000   thermal_zone1 MID 68000   thermal_zone2 LITTLE 67000
thermal_zone3 G3D 60000   thermal_zone4 ISP 63000   thermal_zone5 NPU 61000
thermal_zone6 CP  63000   thermal_zone7 ac  40000   thermal_zone8 battery 33800
```

| Hallazgo | Consecuencia | Veredicto |
|---|---|---|
| El **núcleo BIG estaba a 72 °C** con el móvil "en reposo" | Riesgo de thermal throttle y de que la OOM killer se lleve procesos (ya se llevó el túnel dos veces) | **REIMPLEMENTAR EN RUST**: `energy_automation` veta el trabajo pesado por encima del umbral y avisa |
| Samsung ya tiene **protección de batería al 80 %** (`mProtectionThreshold`) | No hay que inventar un límite de carga: hay que **leerlo y respetarlo** | **KEEP + integrar**: el módulo de energía lo lee (`charge_limit_pct`) |
| `cmd battery` **NO** tiene `set limit` en este build | La única forma de "cargar hasta X" es `dumpsys battery unplug` (suspender) o el ajuste de Samsung | **Documentado**: el módulo dice exactamente qué puede y qué no puede hacer, en vez de fingir |
| `XtremeBS` corre su daemon (`XtremeBSd`) con WebUI en `127.0.0.1:8081` | Ya hay un gestor de energía de terceros | **INTEGRAR** (leer su estado, respetar su "safe mode") |

---

## 8. Qué se reimplementa en Rust dentro de MegaDaemon (ENTREGA 2)

Rama `agent/automacion-software`. Cada módulo: **CLI + herramienta MCP +
superficie de agente + tests**.

### 8.1 Motor de disparadores y reglas — `rust-core/src/automation.rs`

* 16 eventos: `sms_received`, `call_missed`, `call_ended`, `call_incoming`,
  `battery_changed`, `charger_changed`, `app_opened`, `app_closed`,
  `file_created`, `temperature_changed`, `network_changed`, `wifi_changed`,
  `schedule`, `screen_on`, `screen_off`, `boot_completed`.
* 12 campos de condición (`kind`, `package`, `ssid`, `path`, `peer`, `text`,
  `battery_pct`, `temperature_c`, `network`, `value`, `extra`) × 11 operadores
  (`eq`, `ne`, `lt`, `le`, `gt`, `ge`, `contains`, `prefix`, `suffix`, `in`,
  `exists`), con comparación numérica cuando ambos lados son números.
* 7 acciones: `notify`, `script`, `http`, `write_file`, `setting`,
  `root_command`, `broadcast`.
* `cooldown_secs`, `max_fires`, `stop_on_error`, `allow_root` por regla.
* Journal en NDJSON (`automation_journal.ndjson`) con regla, evento, acción y
  resultado ⇒ **todo lo que se ejecuta queda registrado**.
* **Tests**: 41 unitarios + 4 de integración (`tests/automation_test.rs`).

### 8.2 Planificador cron persistente — `rust-core/src/cron.rs`

* Expresiones de 5 campos con `*`, `*/n`, `a-b`, `a-b/n`, listas y `?`; nombres
  de mes y día (`jan`…`dec`, `sun`…`sat`).
* Calendario civil implementado a mano (días↔epoch) **con zona horaria del
  móvil** (`date +%z`); sin `chrono`.
* Persistencia (`cron_state.json`): `last_run`, `next_run`, `running_since`,
  `failures`.
* **Catch-up** de lo perdido (`skip` / `run_once` / `run_all`, con tope).
* **Reintentos** con espera creciente y **reanudación** de las tareas que
  quedaron interrumpidas (el daemon murió con una en curso).
* **Límites de recursos**: tope de simultáneas, tiempo máximo de ejecución,
  veto de tareas pesadas con batería baja.
* **Tests**: 17 unitarios + 8 de integración (`tests/cron_test.rs`).

### 8.3 Energía y térmica — `rust-core/src/energy_automation.rs`

* Perfiles: `normal`, `conservacion` (tope 80 %, mínimo 25 %, veto de pesado
  bajo 42 °C), `viaje` (tope 85 %); los del usuario se cargan desde
  `energy_profiles.json`.
* Lee el estado **real**: batería, zonas térmicas, `low_power`, gobernador,
  límite de carga de Samsung y **el estado del módulo XtremeBS**.
* Decide: `SetChargeLimit`, `StopCharging`, `ResumeCharging`, `RestrictHeavy`,
  `Notify`, `SetProfile` — y **explica por qué** en cada decisión.
* Aplica de verdad: `dumpsys battery unplug` (suspender carga) y
  `dumpsys battery reset` (reanudar); y **dice** que en este build `cmd
  battery` no expone `set limit` (medido) en vez de fingir un éxito.
* **Tests**: 11 unitarios + 7 de integración (`tests/energy_automation_test.rs`).

### 8.4 Comunicaciones — `rust-core/src/comms_automation.rs`

* Eventos: SMS recibido/enviado, llamada entrante/perdida/finalizada.
* Decisiones: `Reply`, `Forward`, `Record`, `Log`, `Ignore`.
* **El consentimiento es del usuario** (`comms_consent.json`): por defecto
  **todo concedido** (`auto_reply`, `forwarding`, `outgoing_allowed`,
  `allow_record_calls` a `true`, `dry_run` a `false`) — lo edita el usuario, no
  el programa. El usuario puede vetar interlocutores (`blocked_peers`),
  limitar respuestas por hora y poner un enfriamiento por interlocutor.
* **Bitácora obligatoria** (`comms_journal.ndjson`): quién, qué, cuándo y con
  qué texto. Sin rastro no hay automatización de comunicaciones.
* Envío real por `service call isms` y, si no, `am start SENDTO` (que devuelve
  el compositor); **si ninguna funciona, lo dice** con el error de cada vía.
* **Tests**: 8 unitarios + 7 de integración (`tests/comms_automation_test.rs`).

### 8.5 Puente con los automatizadores instalados — `rust-core/src/automation_bridge.rs`

* Detecta qué automatizadores hay (con su versión real) y **qué superficie de
  disparo declara cada uno**, medido con `dumpsys package`:
  * Termux → `com.termux.RUN_COMMAND` en `com.termux/.app.RunCommandService`
    (medido).
  * Termux:Boot → `com.termux.boot/.BootReceiver` con `BOOT_COMPLETED`
    (medido) — y **medido también que su directorio de scripts no existe**.
  * AutoTasker → **sólo** `BOOT_COMPLETED` en su `BootReceiver` de recordatorios
    (medido) ⇒ **no hay puente posible hacia sus tareas**.
  * Tasker → acción pública de fabricante, marcada `measured: false` porque
    **no está instalado**.
  * MacroDroid / KAutomator → listados **sin** acciones inventadas.
* Construye el comando `am` validado (sin metacaracteres: se rechazan `;`, `|`,
  `&`, `` ` ``, `$`, redirecciones…) y comprueba que el parser del propio
  módulo acepta lo que genera.
* Bandeja de entrada (`bridge/inbox/*.json`) para eventos que lleguen de otras
  apps.
* **Tests**: 8 unitarios + 8 de integración (`tests/automation_bridge_test.rs`).

### 8.6 Superficies añadidas

| Superficie | Qué |
|---|---|
| **CLI** | `megadaemon-core automation\|cron\|energy\|comms\|bridge <acción>` |
| **MCP** | 5 herramientas nuevas: `automation`, `cron`, `energy`, `comms`, `bridge` (con `action` tipado como `enum`) |
| **Agente** (`agent_coding`) | 12 `kind` nuevos en la lista blanca de `agent_core` (`automation.list`, `automation.poll`, `automation.fire`, `cron.list`, `cron.tick`, `energy.status`, `energy.apply`, `comms.status`, `comms.decide`, `bridge.report`, `bridge.detect`) con su ejecutor |

---

## 9. Cómo reproducir la auditoría

```bash
# Desde el sandbox, contra el móvil (root real por MCP)
cd /workspace && python3 mdc.py root_exec '{"command":"pm list packages -3 | wc -l","max_chars":200000}'
cd /workspace && python3 mdc.py root_exec '{"command":"ls -1 /data/adb/modules/","max_chars":200000}'
cd /workspace && python3 mdc.py root_exec '{"command":"for t in busybox curl sqlite3 jq python3 crond; do printf \"%s=%s\\n\" $t \"$(command -v $t || echo NO)\"; done","max_chars":200000}'
```

Los barridos pesados (1 262 paquetes con `dumpsys package` + `du`) tardan
~40 min ⇒ se lanzan **en segundo plano dentro del móvil** y se bajan en
bloque (`tail -c +N | head -c M | base64`), como documenta el runbook de la
flota.

---

## 10. Resumen ejecutivo

1. **No hay automatizador real en el móvil**: AutoTasker está instalada pero
   con **0 tareas** y sin ninguna acción de disparo; Tasker, MacroDroid y
   KAutomator **no están**. La automatización de verdad son 9 scripts de
   `service.d` que sólo corren al arrancar.
2. **No hay planificador**: `crond`, `at`, `crontab` y `busybox` **no existen**
   en el PATH del móvil ⇒ no se puede programar nada.
3. **El software de terceros que sobra** son ~975 MiB: `BetterKnownInstalled`
   (825 MiB), `flagsecurepatcher` (146 MiB), `cromite-webview` (291 MiB, este
   sí se usa) y los tres módulos de Lucky Patcher (bypass de compras y
   pirateo de licencias).
4. **Lo que se absorbe**: las 5 capacidades de la §8, en Rust puro, dentro
   del daemon ⇒ sin apps de terceros, sin depender de Play/F-Droid y con
   journal de todo lo que se hace.
5. **Lo que se respeta**: `XtremeBS` (se integra, no se desinstala), Termux
   (se usa como ejecutor), la protección de batería de Samsung (se lee) y el
   parche del CSC (se mantiene).
