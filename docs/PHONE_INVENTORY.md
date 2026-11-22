# INVENTARIO MASIVO DEL S21 ULTRA (root) — CIFRAS MEDIDAS

> **Todo lo que sigue está MEDIDO en el móvil**, vía `tools/md_mcp.sh root_exec`
> (root real, `su -mm -c`, uid=0) contra el Samsung de la flota, el **2026-11-22 00:00–01:15 UTC**.
> Ninguna cifra es estimada ni inventada. Lo que no se pudo medir aparece como
> **NO VERIFICADO** con el motivo exacto.
> Lectura de unidades: **KB = KiB (1.024 bytes)**, tal y como los devuelve `du -sk`.

## 0. Ficha de la medición

| Campo | Valor medido |
|---|---|
| Paquetes instalados | **1.719** (`pm list packages \| wc -l`) |
| Del sistema (`pm list packages -s`) | **457** |
| De usuario (`pm list packages -3`) | **1.262** |
| Deshabilitados (`pm list packages -d`) | **117** |
| Root | Magisk presente (`/data/adb/magisk`, `/data/adb/modules`, 1.331.199 KB de módulos) |
| RAM total | `MemTotal 10.829.480 kB` (10,32 GiB) |
| RAM disponible | `MemAvailable 3.953.012 kB` (3,77 GiB) · `MemFree 243.960 kB` · `SwapFree 589.984 kB` |
| Carga | `/proc/loadavg 24.35 21.62 20.08` (8 núcleos) |
| Disco `/data` | `df -k` → **235.730.924 KB** totales · **224.998.772 KB** usados · **10.732.152 KB** libres (96 %) |
| Android | Samsung SM-S908B / Android 13 (perfil de usuario 0 + perfil `de` secundario) |

**Cómo se midió (para que se pueda repetir).** `root_exec` devuelve como mucho 1.024 caracteres
por llamada si no se le pasa `max_chars`; por eso los barridos grandes se hicieron **en segundo
plano dentro del móvil** (`setsid … &` escribiendo TSV en `/data/local/tmp/inv/`) y luego se
trajeron con `max_chars=520000`. Ningún `du` se Lanzó en primer plano sobre un árbol grande.
Cero escrituras, cero borrados, cero `pm uninstall`, cero `kill` de procesos del usuario.

---

## 1. APPS

### 1.1 Recuento y clasificación

| Métrica | Valor medido |
|---|---|
| Paquetes totales | **1.719** |
| Del sistema (preinstaladas) | **457** |
| De usuario | **1.262** |
| Con directorio de datos en `/data/user/0` | **1.718** = 456 del sistema + 1.262 de usuario |
| Del sistema **sin** directorio de datos | **1** → `com.google.android.sdksandbox` |
| Directorios huérfanos en `/data/user/0` (sin entrada en `pm`) | **3** → `com.google.android.trichromelibrary_599308031`, `de.benibela.videlibri`, `ru.mike.diiatweaks` |
| Deshabilitados | **117** |

### 1.2 Dónde vive cada APK (`pm list packages -f`)

| Ubicación del APK | Nº de paquetes |
|---|---|
| `/data/app` | 1342 |
| `/system/priv-app` | 170 |
| `/system/app` | 114 |
| `/product/overlay` | 38 |
| `/system/system_ext` | 10 |
| `/product/app` | 7 |
| `/product/priv-app` | 6 |
| `/vendor/overlay` | 5 |
| `/apex/com.android.wifi` | 3 |
| `/apex/com.android.mediaprovider` | 3 |
| `/apex/com.android.tethering` | 2 |
| `/apex/com.android.permission` | 2 |
| resto de `/apex/*` (12 más: cellbroadcast, adservices, healthfitness, ondevicepersonalization, tethering, apex.cts.shim, btservices, extservices, rkpd, appsearch, uwb, virt) | 17 |
| **Total** | **1.719** |

**1.342 APK viven en `/data/app`** (instalados o *actualizados*) y sólo **377** en particiones de
sólo-lectura (`/system`, `/product`, `/vendor`, `/apex`), que vuelven con el firmware de fábrica.

### 1.3 Las 25 apps más pesadas por DATOS (`du -sk /data/user/0/<pkg>` individual)

| # | Paquete | Tipo | Datos | KB |
|---|---|---|---|---|
| 1 | `com.agy` | usuario | 16.123.3 MiB | 16.510.287 |
| 2 | `com.termux` | usuario | 3.473.5 MiB | 3.556.895 |
| 3 | `io.github.forkmaintainers.iceraven` | usuario | 2.366.3 MiB | 2.423.085 |
| 4 | `com.rk.xededitor` | usuario | 1.614.4 MiB | 1.653.106 |
| 5 | `org.mozilla.fenix` | usuario | 1.194.7 MiB | 1.223.404 |
| 6 | `com.google.android.apps.docs.editors.docs` | usuario | 1.141.8 MiB | 1.169.254 |
| 7 | `com.android.chrome` | sistema | 1.075.0 MiB | 1.100.766 |
| 8 | `com.google.android.apps.docs` | usuario | 813.6 MiB | 833.125 |
| 9 | `org.csploit.strix` | usuario | 799.2 MiB | 818.388 |
| 10 | `com.tda.unseen` | usuario | 764.0 MiB | 782.373 |
| 11 | `org.chromium.thorium` | usuario | 708.6 MiB | 725.637 |
| 12 | `com.adobe.reader` | usuario | 706.5 MiB | 723.448 |
| 13 | `org.swiftapps.swiftbackup` | usuario | 686.7 MiB | 703.203 |
| 14 | `com.google.android.googlequicksearchbox` | usuario | 667.0 MiB | 682.982 |
| 15 | `com.xingin.xhs` | usuario | 642.8 MiB | 658.237 |
| 16 | `com.google.android.providers.media.module` | usuario | 590.4 MiB | 604.556 |
| 17 | `com.sec.android.app.sbrowser` | usuario | 543.5 MiB | 556.519 |
| 18 | `com.google.android.apps.photos` | usuario | 542.3 MiB | 555.360 |
| 19 | `com.whatsapp` | usuario | 538.7 MiB | 551.598 |
| 20 | `me.timschneeberger.shizustore` | usuario | 502.4 MiB | 514.442 |
| 21 | `com.elcorteingles.app` | usuario | 432.0 MiB | 442.391 |
| 22 | `com.google.android.gms` | sistema | 410.7 MiB | 420.529 |
| 23 | `org.fdroid.basic` | usuario | 388.3 MiB | 397.575 |
| 24 | `com.google.android.apps.maps` | usuario | 314.2 MiB | 321.750 |
| 25 | `com.google.android.apps.docs.editors.sheets` | usuario | 305.8 MiB | 313.131 |

**Suma de las 25 = 38.242.041 KB (36.47 GiB)** = 75,0 % de todo `/data/user/0` (48.62 GiB).

Concentración medida en `/data/user/0`:

| Corte | KB | GiB | % de `/data/user/0` |
|---|---|---|---|
| top 10 | 30.070.683 | 28.68 | 59.0 % |
| top 20 | 36.346.665 | 34.66 | 71.3 % |
| top 25 | 38.242.041 | 36.47 | 75.0 % |
| top 50 | 43.814.918 | 41.79 | 85.9 % |
| top 100 | 47.349.263 | 45.16 | 92.9 % |

### 1.4 Apps del sistema (preinstaladas): ruta y tamaño reales

Las **457** apps del sistema suman **6,27 GiB de APK**. De ellas, **80 tienen el APK en `/data/app`**
(apps del sistema *actualizadas*: el factory reset las devuelve a la versión de fábrica, no a la nueva)
y **377 están en particiones de sólo-lectura** (se recuperan con el firmware).

| # | Paquete | APK | Ubicación real del APK |
|---|---|---|---|
| 1 | `com.android.webview` | 291.2 MiB | `/system/app` |
| 2 | `com.google.android.webview` | 252.0 MiB | `/data/app` |
| 3 | `com.android.chrome` | 240.0 MiB | `/data/app` |
| 4 | `com.google.android.gms` | 224.3 MiB | `/data/app` |
| 5 | `com.samsung.android.app.dofviewer` | 218.1 MiB | `/system/priv-app` |
| 6 | `com.samsung.android.aremojieditor` | 204.7 MiB | `/system/priv-app` |
| 7 | `com.samsung.android.app.notes` | 198.6 MiB | `/data/app` |
| 8 | `com.samsung.android.wallpaper.res` | 169.8 MiB | `/system/priv-app` |
| 9 | `com.samsung.android.scs` | 133.8 MiB | `/data/app` |
| 10 | `com.android.settings` | 104.8 MiB | `/system/priv-app` |
| 11 | `com.samsung.android.sdk.ocr` | 99.6 MiB | `/system/app` |
| 12 | `com.samsung.android.sume.nn.service` | 98.4 MiB | `/system/priv-app` |
| 13 | `com.microsoft.appmanager` | 95.8 MiB | `/data/app` |
| 14 | `com.samsung.android.honeyboard` | 91.3 MiB | `/data/app` |
| 15 | `com.samsung.android.intellivoiceservice` | 81.0 MiB | `/data/app` |
| 16 | `com.samsung.android.photoremasterservice` | 76.1 MiB | `/system/priv-app` |
| 17 | `com.samsung.android.bixby.agent` | 75.3 MiB | `/data/app` |
| 18 | `com.samsung.SMT.lang_en_us_l03` | 73.7 MiB | `/system/app` |
| 19 | `com.samsung.android.spayfw` | 73.2 MiB | `/data/app` |
| 20 | `com.android.vending` | 71.5 MiB | `/data/app` |
| 21 | `com.samsung.android.aremoji` | 70.5 MiB | `/data/app` |
| 22 | `com.sec.android.app.samsungapps` | 67.3 MiB | `/data/app` |
| 23 | `com.android.systemui` | 66.5 MiB | `/system/system_ext` |
| 24 | `com.samsung.android.accessibility.talkback` | 64.3 MiB | `/system/priv-app` |
| 25 | `com.samsung.android.smartsuggestions` | 62.4 MiB | `/data/app` |

> La tabla completa de las 457 (paquete · bytes de APK · ruta) está en el TSV medido en el móvil:
> `/data/local/tmp/inv/sys_pkgs.tsv` (457 líneas).

### 1.5 Apps del sistema con más DATOS

| Paquete | Datos | KB |
|---|---|---|
| `SYS` | 1.075.0 MiB | 1.100.766 |
| `SYS` | 590.4 MiB | 604.556 |
| `SYS` | 410.7 MiB | 420.529 |
| `SYS` | 305.1 MiB | 312.374 |
| `SYS` | 239.1 MiB | 244.793 |
| `SYS` | 176.6 MiB | 180.831 |
| `SYS` | 111.5 MiB | 114.221 |
| `SYS` | 86.1 MiB | 88.162 |
| `SYS` | 83.7 MiB | 85.671 |
| `SYS` | 82.7 MiB | 84.682 |
| `SYS` | 62.6 MiB | 64.061 |
| `SYS` | 58.4 MiB | 59.763 |
| `SYS` | 52.1 MiB | 53.349 |
| `SYS` | 42.1 MiB | 43.078 |
| `SYS` | 37.0 MiB | 37.863 |
| `SYS` | 31.7 MiB | 32.490 |
| `SYS` | 28.0 MiB | 28.689 |
| `SYS` | 27.1 MiB | 27.769 |
| `SYS` | 26.3 MiB | 26.955 |
| `SYS` | 25.9 MiB | 26.529 |

**Total datos de apps de sistema = 4.149.418 KB (3,96 GiB)**, repartidos en 456 paquetes.

### 1.6 Apps de usuario con más DATOS

| # | Paquete | Datos | KB |
|---|---|---|---|
| 1 | `USER` | 16.123.3 MiB | 16.510.287 |
| 2 | `USER` | 3.473.5 MiB | 3.556.895 |
| 3 | `USER` | 2.366.3 MiB | 2.423.085 |
| 4 | `USER` | 1.614.4 MiB | 1.653.106 |
| 5 | `USER` | 1.194.7 MiB | 1.223.404 |
| 6 | `USER` | 1.141.8 MiB | 1.169.254 |
| 7 | `USER` | 813.6 MiB | 833.125 |
| 8 | `USER` | 799.2 MiB | 818.388 |
| 9 | `USER` | 764.0 MiB | 782.373 |
| 10 | `USER` | 708.6 MiB | 725.637 |
| 11 | `USER` | 706.5 MiB | 723.448 |
| 12 | `USER` | 686.7 MiB | 703.203 |
| 13 | `USER` | 667.0 MiB | 682.982 |
| 14 | `USER` | 642.8 MiB | 658.237 |
| 15 | `USER` | 543.5 MiB | 556.519 |
| 16 | `USER` | 542.3 MiB | 555.360 |
| 17 | `USER` | 538.7 MiB | 551.598 |
| 18 | `USER` | 502.4 MiB | 514.442 |
| 19 | `USER` | 432.0 MiB | 442.391 |
| 20 | `USER` | 388.3 MiB | 397.575 |
| 21 | `USER` | 314.2 MiB | 321.750 |
| 22 | `USER` | 305.8 MiB | 313.131 |
| 23 | `USER` | 292.2 MiB | 299.237 |
| 24 | `USER` | 292.1 MiB | 299.103 |
| 25 | `USER` | 275.2 MiB | 281.763 |

**Total datos de apps de usuario = 46.826.641 KB (44,66 GiB)**, repartidos en **1.265 directorios**
(1.262 de paquete + los 3 huérfanos huérfanos listados en §1.1).

---

## 2. DATOS — peso real

### 2.1 `/data` a primer nivel (`du -sk /data/*`)

| Ruta | KB | GiB |
|---|---|---|
| `/data/app` | 88.149.106 | 84.07 |
| `/data/media` | 75.185.668 | 71.70 |
| `/data/user` | 50.985.318 | 48.62 |
| `/data/data` | 50.981.763 | 48.62 |
| `/data/adb` | 1.502.572 | 1.43 |
| `/data/user_de` | 946.112 | 0.90 |
| `/data/system` | 596.576 | 0.57 |
| `/data/per_boot` | 567.860 | 0.54 |
| `/data/local` | 496.074 | 0.47 |
| `/data/apex` | 344.513 | 0.33 |
| `/data/vendor` | 319.202 | 0.30 |
| `/data/dalvik-cache` | 272.047 | 0.26 |
| `/data/misc` | 136.875 | 0.13 |
| `/data/log` | 117.725 | 0.11 |
| `/data/system_ce` | 46.284 | 0.04 |
| `/data/sec` | 31.182 | 0.03 |
| `/data/tombstones` | 9.256 | 0.01 |
| `/data/overlays` | 8.336 | 0.01 |

`/data/data` es el mismo almacenamiento que `/data/user/0` (48,62 GiB) y **no se cuenta dos veces**.
Suma sin `/data/data` = **219.737.356 KB (209,56 GiB)**; `df` dice 224.998.772 KB usados.
**Diferencia = 5.261.416 KB (5,02 GiB) de metadatos del sistema de ficheros / reserved — NO VERIFICADO**
(no se itemizó bloque a bloque: `du` de `/data` cuenta bloques asignados, `df` cuenta el total del volumen).

### 2.2 `/data/user/0` desglosado

| Tramo | Nº paquetes | KB | GiB |
|---|---|---|---|
| Total `/data/user/0` | 1.718 | 50.978.411 | 48,62 |
| …del sistema | 456 | 4.149.418 | 3,96 |
| …de usuario | 1.265 | 46.826.641 | 44,66 |

### 2.3 `/data/app` desglosado (los APK)

| Componente | KB | GiB | Nº | ¿Respaldable? |
|---|---|---|---|---|
| Ficheros `.apk` (`base.apk` + `split*.apk`) | 66.967.724 | 63,87 | 2.776 ficheros | Sí — es el instalable |
| `lib/` (librerías nativas por ABI) | 13.110.470 | 12,50 | 1.066 carpetas | Sí — va dentro del paquete |
| `oat/` (código AOT de ART) | 7.636.287 | 7,28 | 1.001 carpetas | **NO — regenerable** |
| `*.dm` (metadatos de dex) | 422.044 | 0,41 | 625 ficheros | **NO — regenerable** |
| `.digests` / `.metadata` / `.prof` | ~12.647 | 0,01 | 1.220 ficheros | **NO — regenerable** |
| **Total `/data/app`** | **88.149.106** | **84,07** | | |

### 2.4 `/data/media/0` (= `/sdcard`) desglosado

| Ruta | KB | GiB |
|---|---|---|
| `/sdcard/Download` | 36.184.304 | 34.51 |
| `/sdcard/Android` | 19.268.785 | 18.38 |
| `/sdcard/DCIM` | 18.207.664 | 17.36 |
| `/sdcard/iLovePDF` | 812.757 | 0.78 |
| `/sdcard/Pictures` | 316.237 | 0.30 |
| `/sdcard/SwiftBackup` | 86.615 | 0.08 |
| `/sdcard/RouterKeygen.dic` | 64.292 | 0.06 |
| `/sdcard/Movies` | 49.379 | 0.05 |
| `/sdcard/logcat.txt` | 36.556 | 0.03 |
| `/sdcard/Music` | 22.360 | 0.02 |
| `/sdcard/Documents` | 18.164 | 0.02 |
| `/sdcard/Aliucord` | 15.649 | 0.01 |
| `/sdcard/ExtractedApks` | 14.703 | 0.01 |
| `/sdcard/Books` | 12.884 | 0.01 |
| `/sdcard/NeoStore` | 9.535 | 0.01 |
| `/sdcard/Imprimirpeinado` | 3.908 | 0.00 |

**Total `/data/media/0` = 75.185.640 KB (71,71 GiB)** (`du -sk /data/media/0`). La suma de sus hijos
directos da 75.136.491 KB: la diferencia de 49.149 KB (48 MiB) es el reparto de bloques de
directorio entre los 40 hijos. `du -sk /data/media` da 75.185.668 KB.

**`/sdcard/Download` (34,51 GiB) — desglose:**

| Carpeta | KB | GiB |
|---|---|---|
| `/sdcard/Download/Ordenado` | 18.973.745 | 18.09 |
| `/sdcard/Download/Snatcher-Collection` | 11.315.656 | 10.79 |
| `/sdcard/Download/Carpetas` | 4.325.727 | 4.13 |
| `/sdcard/Download/1DMP` | 739.459 | 0.71 |
| `/sdcard/Download/luupatch` | 200.229 | 0.19 |
| `/sdcard/Download/The C++ Programming Language, 4th Edition -- Bjarne Stroustrup -- 4th, 2013 -- Addison-Wesley Professional -- isbn13 978013.epub` | 163.540 | 0.16 |
| `/sdcard/Download/ms_dos_6_0_source_code` | 68.232 | 0.07 |
| `/sdcard/Download/scummvm_artifact.zip` | 63.036 | 0.06 |
| `/sdcard/Download/scummvm-snatcher-android-debug-apk (1).zip` | 63.024 | 0.06 |
| `/sdcard/Download/scummvm-snatcher-android-debug-apk (2).zip` | 63.020 | 0.06 |

**`/sdcard/DCIM` (17,36 GiB) — desglose:**

| Carpeta | KB | GiB |
|---|---|---|
| `/sdcard/DCIM/Camera` | 15.538.492 | 14.82 |
| `/sdcard/DCIM/Screenshots` | 1.997.176 | 1.90 |
| `/sdcard/DCIM/Screen recordings` | 622.248 | 0.59 |
| `/sdcard/DCIM/PlantNet` | 43.328 | 0.04 |
| `/sdcard/DCIM/PhotosEditor` | 3.604 | 0.00 |
| `/sdcard/DCIM/Google Photos` | 1.196 | 0.00 |
| `/sdcard/DCIM/ADBCaptain` | 636 | 0.00 |
| `/sdcard/DCIM/Restored` | 556 | 0.00 |
| `/sdcard/DCIM/KimiChat` | 428 | 0.00 |

**`/sdcard/Android/media` (6,71 GiB) — desglose:**

| Carpeta | KB | GiB |
|---|---|---|
| `/data/media/0/Android/media/com.whatsapp` | 7.029.626 | 6.70 |
| `/data/media/0/Android/media/org.telegram.messenger` | 651 | 0.00 |
| `/data/media/0/Android/media/fork.risin42.nagramx` | 14 | 0.00 |
| `/data/media/0/Android/media/org.swiftapps.swiftbackup` | 4 | 0.00 |
| `/data/media/0/Android/media/de.yalwa.loc` | 4 | 0.00 |
| `/data/media/0/Android/media/com.xphotokit.chatgptassist` | 4 | 0.00 |
| `/data/media/0/Android/media/com.termux` | 4 | 0.00 |
| `/data/media/0/Android/media/com.superking.parchisi.star` | 4 | 0.00 |

**`/sdcard/Android/data` (11,67 GiB) — 12 primeras:**

| Carpeta | KB | GiB |
|---|---|---|
| `/data/media/0/Android/data/zed.rainxch.githubstore` | 7.777.947 | 7.42 |
| `/data/media/0/Android/data/com.revanced.net.revancedmanager` | 1.335.059 | 1.27 |
| `/data/media/0/Android/data/com.github.axet.audiorecorder` | 515.809 | 0.49 |
| `/data/media/0/Android/data/ru.drjmwdme.cgwbfzijr` | 444.743 | 0.42 |
| `/data/media/0/Android/data/com.deniscerri.ytdl` | 370.524 | 0.35 |
| `/data/media/0/Android/data/org.woheller69.whisper` | 361.547 | 0.34 |
| `/data/media/0/Android/data/com.moodle.moodlemobile` | 357.960 | 0.34 |
| `/data/media/0/Android/data/com.seleuco.mame4d2024` | 222.109 | 0.21 |
| `/data/media/0/Android/data/com.machiav3lli.fdroid` | 211.850 | 0.20 |
| `/data/media/0/Android/data/de.marmaro.krt.ffupdater` | 139.514 | 0.13 |
| `/data/media/0/Android/data/fork.risin42.nagramx` | 120.004 | 0.11 |
| `/data/media/0/Android/data/com.intsig.camscanner` | 74.651 | 0.07 |

**`/sdcard/Android/obb` = 25 KB.** Los OBB se movieron a `Android/media` (Android 11+).

### 2.5 Apps de mensajería: dónde están DE verdad

Comprobado con `test -e` (no por suposición):

| Ruta | Existe |
|---|---|
| `/sdcard/WhatsApp` | **NO** |
| `/sdcard/Telegram` | **NO** |
| `/sdcard/Signal` | **NO** |
| `/sdcard/com.whatsapp` | **NO** |
| `/sdcard/TelegramMessenger` | **NO** |
| `/sdcard/org.thoughtcrime.securesms` | **NO** |
| `/data/media/0/Android/media/com.whatsapp` | **SÍ → 7.029.622 KB (6,71 GiB)** |
| `/data/media/0/Android/media/org.telegram.messenger` | **SÍ → 651 KB** (prácticamente vacío) |
| `/data/media/0/Android/media/fork.risin42.nagramx` | **SÍ → 14 KB** |

⇒ El multimedia de WhatsApp (6,71 GiB) vive en `Android/media/com.whatsapp/WhatsApp`.
Telegram y Signal **no tienen multimedia en el móvil** (sólo 665 KB en total): o están vacíos o
residen en la nube. **NO VERIFICADO**: no se comprobó si la sesión de Telegram/Signal tiene
historial en la nube (requiere la app, no el filesystem).

### 2.6 Resto de `/data` con contenido propio

| Ruta | KB | GiB | Contenido |
|---|---|---|---|
| `/data/adb` | 1.502.572 | 1,43 | Módulos Magisk (`modules` 1.331.199 KB), `rvhc` 119.420 KB, `modules---` 40.490 KB (deshabilitados), `magisk` 5.646 KB |
| `/data/user_de/0` | 946.259 | 0,90 | **Perfil secundario**: `com.google.android.tts` 518.809 KB, `com.google.android.gms` 307.310 KB, `io.github.huskydg.magisk` 20.176 KB, +10 más |
| `/data/system` | 596.576 | 0,57 | |
| `/data/per_boot` | 567.860 | 0,54 | |
| `/data/local` | 496.074 | 0,47 | `tmp` 497.726 KB (mis propios ficheros de medición + logs) |
| `/data/apex` | 344.513 | 0,33 | |
| `/data/vendor` | 319.202 | 0,30 | |
| `/data/dalvik-cache` | 272.047 | 0,26 | **NO — regenerable** (`arm64` 271.980 KB) |
| `/data/misc` | 136.875 | 0,13 | |
| `/data/log` | 117.725 | 0,11 | |

---

## 3. QUÉ ES RESPALDABLE Y QUÉ ES BASURA (medido, en números)

### 3.1 Cachés regenerables dentro de `/data/user/0`

Barrido con `find /data/user/0 -maxdepth 6 -type d \( -name cache -o -name Cache -o -name code_cache
-o -name 'Code Cache' -o -name noBackup -o -name no_backup -o -name thumbnails -o -name GPUCache
-o -name GPUPersistentCache -o -name 'Service Worker' -o -name ScriptCache -o -name 'Compiled Code'
-o -name shader_cache -o -name ShaderCache -o -name Crashpad \) -prune -exec du -sk {} +`
(`-prune` ⇒ **sin doble conteo**: un `cache` dentro de otro `cache` no se cuenta dos veces).

**Total = 2.734.501 KB (2.61 GiB) en 4701 directorios.**

| Directorio | KB | MiB |
|---|---|---|
| `/data/user/0/com.sec.android.app.sbrowser/app_sbrowser/Default/Service Worker` | 479.437 | 468.2 |
| `/data/user/0/com.deniscerri.ytdl/no_backup` | 232.129 | 226.7 |
| `/data/user/0/app.morphe.android.youtube/cache` | 164.037 | 160.2 |
| `/data/user/0/com.sec.android.app.sbrowser.beta/app_sbrowser/Default/Service Worker` | 157.552 | 153.9 |
| `/data/user/0/com.myAllVideoBrowser/no_backup` | 131.305 | 128.2 |
| `/data/user/0/com.rk.xededitor/local/sandbox/var/cache` | 120.234 | 117.4 |
| `/data/user/0/org.mozilla.fenix/no_backup` | 94.570 | 92.4 |
| `/data/user/0/akhil.alltrans/no_backup` | 89.975 | 87.9 |
| `/data/user/0/com.termux/files/home/.cargo/registry/cache` | 71.619 | 69.9 |
| `/data/user/0/com.vitorpamplona.amethyst/files/arti/cache` | 60.343 | 58.9 |
| `/data/user/0/com.android.chrome/cache` | 44.257 | 43.2 |
| `/data/user/0/InfinityLoop1309.NewPipeEnhanced/no_backup` | 40.615 | 39.7 |
| `/data/user/0/org.torproject.torbrowser_alpha/no_backup` | 34.024 | 33.2 |
| `/data/user/0/com.orange.bluu.jazztel/no_backup` | 20.854 | 20.4 |
| `/data/user/0/com.minimax.ai/cache` | 20.641 | 20.2 |

### 3.2 Regenerables fuera de `/data/user/0`

| Zona | KB | GiB | Por qué es basura |
|---|---|---|---|
| `/data/app/*/oat/` | 7.636.287 | 7.28 | Código Ahead-Of-Time de ART: se regenera solo al reinstalar/arrancar |
| `/data/app/*/*.dm` | 422.044 | 0.40 | Metadatos de dex (ART): se regeneran |
| `/data/dalvik-cache/arm64` | 272.047 | 0.26 | Caché dalvik: se regenera |
| **Subtotal regenerable puro** | **8.330.378** | **7.94** | |
| `/data/media/0/iLovePDF/processed` | 808.742 | 0.77 | PDF **convertidos**: derivados, no regenerables desde el móvil (los originales están borrados) pero sin valor propio |

### 3.3 Veredicto de basura

* **Regenerable puro (excluir del respaldo): 8.330.378 KB = 7.94 GiB.**
* **Derivados sin valor (excluir): 808.742 KB = 0.77 MiB.**
* **TOTAL EXCLUIBLE = 9.139.120 KB = 8.72 GiB**
  (más 272.047 KB de `/data/dalvik-cache` ya contados, y lo que cuelgue de `oat`/`dm`).

### 3.4 Descargas «repetidas» — lo que YA está resuelto

En `/sdcard/Download/Ordenado` (18,09 GiB) ya hay una pasada de deduplicación hecha por el usuario:

| Fichero | Líneas | Qué es |
|---|---|---|
| `archivos-borrados-por-redundantes.txt` | 266 | lista `keep` / `REDUNDANT` con el tamaño que se habría ahorrado |
| `dedupe-log-1794889930.tsv` | 876 | pares de ficheros idénticos detectados |
| `undo-1794889306.tsv` | 3.511 | registro de lo borrado (permite deshacer) |
| `undo-dirs-1794889435.tsv` | 137 | carpetas borradas (deshacer) |

**Medido por SHA-256, no por suposición:** los ficheros que *parecen* duplicados NO lo son.
`luupatch.7z.t0.001/.002/.003/.004` tienen **4 SHA-256 distintos** (son trozos de un archivo partido),
y `scummvm-snatcher-android-debug-apk (1).zip` vs `(2).zip` también **difieren**
(`a3037c07…` vs `eb7a4a68…`). **No se puede llamar «basura» a eso.**

---

## 4. EL «SUPERLOOP» DE ANTIGRAVITY

### 4.1 Qué es (leído del propio dispositivo)

Fichero de tareas del daemon de Antigravity:
`/data/user/0/com.agy/files/.gemini/antigravity-app/daemon/tasks.json`

| Campo | Valor medido |
|---|---|
| `id` | `task_442838` |
| `name` | **Snatcher-MegaDaemon-SuperLoop** |
| `command` | `python3 /data/user/0/com.agy/files/scummvm/tools/re_pipeline/snatcher_megadaemon.py --once` |
| `trigger.type` | `interval` |
| **`trigger.interval_sec`** | **300** |
| `enabled` | `true` |
| `last_run_status` | `success` |
| **`last_run_duration`** | **211 s** |
| `created_at` | 2026-11-18T00:24:02Z |
| `last_run_ts` (campo) | 2026-11-19T09:20:08Z — **campo obsoleto** (ver 4.3) |
| `trigger.last_run` (campo) | 2026-11-19T09:21:33Z — **campo obsoleto** |

### 4.2 Coste histórico medido (72 ejecuciones registradas en `daemon.log`)

De `grep -oE 'Finished in [0-9]+s' daemon.log`:

| Estadístico | Valor medido |
|---|---|
| Ejecuciones registradas | **72** |
| `Worker started` registrados | **74** |
| Duración mínima | **211 s** |
| Duración máxima | **575 s** |
| Duración media | **365,3 s** |
| Duración mediana | **365 s** |
| **Suma de tiempo de ejecución** | **26.304 s = 7,30 h** |
| Ventana con actividad | 2026-11-18T00:24 → 2026-11-19T10:21 (≈34 h) |

**Ciclo de trabajo vs. intervalo:** la duración media (365,3 s) **supera el intervalo de 300 s**.
Es decir, cuando el planificador está vivo, la tarea **se relanza en cuanto termina**
(factor de ocupación 365,3/300 = **121,8 %** del intervalo: el bucle es prácticamente continuo).

### 4.3 ¿Se está ejecutando AHORA? — medición en vivo

Muestreo de **66 muestras cada 5 s (5,5 min)** con `ps` + `top` (`/data/local/tmp/inv/sl_samples.txt`):

| Observación | Resultado medido |
|---|---|
| Proceso `python3 …/snatcher_megadaemon.py` capturado en `ps` | **0 veces** |
| Lanzador `sh /data/user/0/com.agy/files/runtime/bin/printf %s python3 …/snatcher_megadaemon.py --once` | **1 vez** (PID 18670, RSS 3.052 KB, ~2026-11-22 00:09) |
| Ficheros modificados en `…/re_pipeline/` en los 90 min previos | **0** (`megadaemon_state.json` sigue en 2026-11-19 10:20) |
| Líneas nuevas en `daemon.log` | **0** (mtime congelado en 2026-11-19T10:21:33Z) |
| `tasks.json` mtime | 2026-11-19T12:09:13Z |
| Último ciclo realmente registrado | **#37**, 2026-11-19T10:21:33Z |

**Corroboración independiente** (otro agente, mismo móvil, commit `b9f1072` de esta misma rama):
el runner volvió a ejecutar el comando real de `tasks.json` y el **ciclo #38 completo** tardó
**206 s** (2026-11-22 01:11:10 → 01:14:36, `rc 0`, BUILD: PASS). Confirma la escala real de un
ciclo: **~206–211 s** frente a los 300 s de intervalo ⇒ cuando el planificador está vivo el bucle
**se relanza sin parar** (factor de ocupación medido sobre los 72 ciclos históricos:
365,3 s de media / 300 s = **121,8 %**).

⇒ **El planificador de tareas de Antigravity NO está ejecutando el superloop.** El `tasks.json`
conserva la tarea con `interval_sec=300` y `enabled=true`, pero el worker no la dispara: el
lanzador se crea y muere sin dejar rastro. **NO VERIFICADO**: el coste de CPU *de un ciclo real*
— no se puede medir sin ejecutarlo, y ejecutarlo muta el sistema (el propio ciclo se anuncia como
«SYSTEM TUNING: ELEVATED HARDWARE OPTIMIZATION & ROOT GOVERNOR»). **No se ejecutó.**

### 4.4 Consumo medido de CPU y RAM

`ps -o pid,rss,etime,time,args` (muestreado 3 veces con `top -b -n 3 -d 2`):

| PID | Proceso | RSS | CPU acumulado | Uptime | % de 1 núcleo (acumulado/uptime) |
|---|---|---|---|---|---|
| 3091 | `com.agy` | 94–107 MB | `57:34` | 05:40:14 | **16,9 %** |
| 3492 | `libloader.so … liblanguage_server.so --standalone --override_ide_name antigravity` | 413–427 MB | `19:23` | 05:40:12 | **5,7 %** |
| 18952/18956 | `rish` (termux, lanzar del offload) | 2,7 / 52 MB | `0:00` / `0:02` | 03:41 | ~0 % |
| 19042 | `sh -c` (wrapper del offload) | 2,7 MB | `0:00` | 03:41 | ~0 % |
| 19350 | `rclone copy` (offload a Drive) | 31–41 MB | `03:53` | 03:41:13 | 0,5 % |

**RSS total de los 6 procesos `com.agy` = 533.664 KB (521 MiB).**
Carga del sistema durante el muestreo: `800%cpu` con **159–719 % idle** (el resto del sistema,
no el superloop, es lo que consume el núcleo; `loadavg` llegó a **24,35**).

### 4.5 Consumo medido en disco de Antigravity

| Ruta | KB | GiB / MiB |
|---|---|---|
| `/data/user/0/com.agy` (total) | 16.510.287 | **15,75 GiB** |
| ↳ `files/` | 16.505.774 | 15,74 GiB |
| ↳ `files/usr/` | 5.072.152 | 4,84 GiB |
| ↳ `files/gpui-mobile/` | 3.224.613 | 3,07 GiB |
| ↳ `files/megadaemon/` | 1.780.777 | 1,70 GiB |
| ↳ `files/tools/` | 1.771.667 | 1,69 GiB |
| ↳ `files/scummvm/` | 1.040.322 | 993 MiB |
| ↳ `files/dumper-web/` | 765.558 | 748 MiB |
| ↳ `files/pepasjc/` | 651.434 | 636 MiB |
| ↳ `files/models/` | 583.452 | 570 MiB |
| ↳ **`files/.gemini/antigravity-app/`** | **629.831** | **615 MiB** |
| ↳ ↳ `conversations/` | 316.892 | 309 MiB |
| ↳ ↳ `brain/` | 296.766 | 290 MiB |
| ↳ ↳ `bin/` | 15.512 | 15 MiB |
| ↳ **`files/runtime/`** | **183.842** | **180 MiB** |
| ↳ ↳ `bin/` | 178.652 | 174 MiB |
| ↳ ↳ `lib/` | 4.972 | 4,9 MiB |
| ↳ **`files/web/`** | **12.988** | **12,7 MiB** |
| ↳ ↳ `generated/` | 9.732 | 9,5 MiB |
| ↳ ↳ `workers/` | 1.377 | 1,3 MiB |
| ↳ ↳ `symbols-icons/` | 1.230 | 1,2 MiB |
| `cache/` | 85 | 85 KB |
| `code_cache/` | 4 | 4 KB |
| `no_backup/` | 4.107 | 4,0 MiB |

**Los tres directorios que pide el encargo — `antigravity-app` + `runtime/` + `web/` — suman
826.661 KB (807 MiB).** El resto de los 15,75 GiB de `com.agy` NO es del superloop: es el
tooling de la flota (`usr/`, `gpui-mobile/`, `megadaemon/`, `tools/`, `scummvm/`, `dumper-web/`),
que **ya está en el repo** ⇒ regenerable desde Git, no desde la tienda.

**Governor de CPU medido ahora mismo: `powersave` en los 8 núcleos.** El tuning de «ROOT GOVERNOR»
del superloop **no está activo** (o se revierte al terminar el ciclo).

---

## 5. DESTINO: GOOGLE DRIVE (medido)

| Dato | Valor medido |
|---|---|
| Remote | `gdrive:` (config `/data/local/tmp/rc/rclone.conf`) |
| Carpeta de destino ya en uso | `gdrive:Backup_Android/` (subcarpetas `Download`, `grandes`) |
| **Contenido actual de `Backup_Android`** | **11.396 objetos · 62,863 GiB** (`rclone size`) |
| Cuota total de la cuenta | **5 TiB** |
| Usado | **1,125 TiB** |
| **Libre** | **3,751 TiB** |
| **Velocidad de SUBIDA medida** | **304.742 B/s (297,6 KiB/s ≈ 0,291 MB/s)** |
| Cómo se midió | `rclone copy` PID 19350: `wchar` de `/proc/19350/io` = 4.047.431.409 B en `ELAPSED 03:43:13` = 13.282 s (promedio de toda la invocación, incluye listado y `check`) |
| **Velocidad de BAJADA medida** | **5.668.249 B/s (5,41 MiB/s)** |
| Cómo se midió | `rclone cat gdrive:Backup_Android/ | head -c 209715200` → 209.715.200 bytes en 37 s |
| Aviso de rclone | *«This remote uses rclone's shared Google Drive client_id, which is being retired and will stop working during 2026»* |

### 5.1 El pipeline de offload que YA existe (leído del proceso vivo)

Proceso `rish` (PID 18952) → `sh -c` (19042) → `rclone copy` (19350). Texto literal del comando:

```
for d in "YTDLnis" "Carpetas" "1DMP" "Snatcher-Collection" "Ordenado"; do
  SRC="/data/media/0/Download/$d"
  DST="gdrive:Backup_Android/Download/$d"
  [ -d "$SRC" ] || { echo "SKIP missing $SRC" >> "$LOG"; continue; }
  $R copy "$SRC" "$DST" $C --transfers 4 --checkers 8 --retries 3 --stats 30s --stats-one-line
  if copy rc == 0; then
    $R check "$SRC" "$DST" $C --one-way
    if check rc == 0; then
      rm -rf "$SRC"          # <-- BORRA el origen tras verificar
      echo "    VERIFIED & FREED $d"
```

Log real (`/sdcard/megadaemon-offload.log`):

```
--- COPY YTDLnis Sat Nov 21 21:28:14 CET 2026 ---
    copy rc=0 Sat Nov 21 21:28:35 CET 2026
--- CHECK YTDLnis Sat Nov 21 21:28:35 CET 2026 ---
2026/11/21 20:28:40 NOTICE: Google drive root 'Backup_Android/Download/YTDLnis': 0 differences found
2026/11/21 20:28:40 NOTICE: Google drive root 'Backup_Android/Download/YTDLnis': 34 matching files
    check rc=0 Sat Nov 21 21:28:40 CET 2026
    VERIFIED & FREED YTDLnis Sat Nov 21 21:28:41 CET 2026
--- COPY Carpetas Sat Nov 21 21:28:41 CET 2026 ---   <-- en curso al medir
```

**Este pipeline ya funciona y ya ha verificado una carpeta de extremo a extremo**
(`0 differences found`, `34 matching files`). Pero **es destructivo**: borra el origen tras
verificar. Para un plan de respaldo eso significa *copia única* salvo que Drive se considere
el respaldo definitivo. Y su `client_id` compartido se retira en 2026.

---

## 6. LO QUE NO SE PUDO MEDIR (NO VERIFICADO)

| # | Qué | Motivo |
|---|---|---|
| 1 | Coste de CPU/RAM **de un ciclo real** del superloop | Habría que ejecutarlo; el propio ciclo muta el governor del sistema. No se ejecutó. |
| 2 | Tasa de subida **aislada** (sólo transferencia) | La única medición es el promedio de una invocación `rclone` real en curso (incluye listado y `check`). |
| 3 | Reparto exacto de 5,02 GiB entre «usado» y metadatos de FS | `du` de `/data` cuenta bloques asignados; `df` cuenta el volumen. No se itemizó a nivel de bloque. |
| 4 | Contenido de los 3 directorios huérfanos de `/data/user/0` y de `com.google.android.sdksandbox` | No se inspeccionaron (no son de paquete registrado). |
| 5 | Si los 687 MiB de `org.swiftapps.swiftbackup/databases` son respaldos válidos | No se abrió la base de datos de Swift Backup. |
| 6 | Historial en la nube de Telegram y Signal | No hay multimedia local (665 KB); el estado en la nube exige la app, no el filesystem. |
| 7 | Si un APK parcheado (Lucky Patcher / Marina Patcher / ReVanced) se reinstala bien | No se probó ninguna reinstalación. |
| 8 | Tiempo real de un `rclone check` completo sobre el conjunto | Sólo hay una medición de `check` real: el de `YTDLnis` (5 s, 34 ficheros). |
| 9 | Duración exacta de la copia de `Carpetas` | Estaba en curso (4,05 GB de 4,43 GB subidos) al terminar la medición. |
| 10 | Contenido de `/data/media/0/iLovePDF/imported` frente a `processed` | Sólo el peso (1.436 KB vs 808.742 KB); no se comprobó la correspondencia archivo a archivo. |


---

## 7. Artefactos de la medición (reproducibilidad)

Todo lo anterior sale de ficheros TSV guardados **en el móvil** durante la medición
(`/data/local/tmp/inv/`, ~1,1 MB). Nada se borró del móvil: sólo se escribieron esos ficheros.

| Fichero | Contenido | Líneas |
|---|---|---|
| `u0_sizes.tsv` | `du -sk /data/user/0/*` (un `du` por paquete) | 1.721 |
| `u0_classed.tsv` | idem + clase (`SYS`/`USER`) | 1.721 |
| `sys_pkgs.tsv` | 457 apps del sistema: paquete, bytes de APK, ruta real | 457 |
| `top_join.tsv` | 40 apps mayores: datos + bytes de APK + ruta | 40 |
| `apk_per_pkg.tsv` | bytes de APK por paquete (los 1.719) | 1.719 |
| `app_sizes.tsv` | `du -sk /data/app/*` | 1.342 |
| `app_comp.txt` | composición de `/data/app` (apk/lib/oat/dm) | 5 |
| `media0.tsv`, `sdcard_sizes.tsv` | `/sdcard` y sus hijos | 40 / 40 |
| `sd_download.tsv`, `sd_dcim.tsv`, `sd_android.tsv` | subárboles de `/sdcard` | 30 / 9 / 3 |
| `amed_data.tsv`, `amed_media.tsv` | `Android/data` (553 dirs) y `Android/media` | 553 / 20 |
| `cache_flat.tsv`, `regen_all.tsv` | cachés de apps (no solapados) | 4.600 / 4.701 |
| `data_top.tsv` | `du -sk /data/*` | 71 |
| `agy_u0/files/gemini/app/runtime/web.tsv` | desglose de `com.agy` | 57 / 11 / 5 / 5 |
| `sl_samples.txt` | muestreo del superloop (66 × 5 s) | 160 |
| `sl_durations.txt` | 72 duraciones de ciclo del superloop | 72 |
| `thr2.txt`, `thr3.txt` | pruebas de velocidad contra Drive | 4 / 3 |

Para re-generarlos: `du -sk` con `-prune` para no solapar cachés, `pm list packages -f` para las
rutas, `stat -c %s` para los bytes exactos de APK, y `setsid … &` para los barridos que tardan
más de 30 s (el límite de `root_exec`).
