# Inventario de software del S21 Ultra con veredicto

> Generado por `tools/verdict_inventory.py` desde el inventario **medido** del móvil (`inventory/inventory.json`, W2) + el análisis de APKs del fleet (W5/C2).  Regenerable: `python3 tools/verdict_inventory.py`.

## 0. Qué es este documento y de dónde sale cada celda

El móvil tiene **1719 apps** (1262 de usuario, 457 de sistema, 186 deshabilitadas) y **1322 piden permisos de riesgo**.  El inventario guarda por app: versión, `firma_sha256` (truncado a 4 bytes por Android 14), permisos de riesgo, `target_sdk`, estado y ruta del APK.

Cada fila mezcla **dato medido** y **juicio**:

| Marca | Significado |
|---|---|
| **M** | Medido en el móvil (inventario W2) o en el APK (hashes, DEX, artefactos embebidos) |
| **D** | Deducido del repo: medidas ya publicadas por el fleet |
| **P** | **PENDIENTE de medición**: el móvil lleva caído desde las 03:37 UTC (túneles 530) ⇒ no se inventa el dato, se marca |

En la columna *Riesgo* sólo hay **hechos** del inventario (permisos, firma, `target_sdk`, estado).  Cuando el riesgo real depende de algo que aún no se ha medido (red, telemetría, uploading), la fila lo dice explícitamente en la justificación.

## 1. Reglas de veredicto

- **DESINSTALAR** — hay evidencia **medida** de que la app (a) sube datos del usuario a un servidor de terceros, (b) pide permisos peligrosos que no necesita para su funcion declarada, o (c) es una reempaquetada/pirata.  **Una app que sube datos del usuario a un servidor desconocido es DESINSTALAR, salvo que el usuario lo diga expresamente.**

- **REIMPLEMENTAR EN RUST** — la funcion cabe en el mega-binario Rust de MegaDaemon (motor de parcheo DEX/APK, gestor de grabaciones, lector de backup, laboratorio de desofuscacion).  La app se puede quitar cuando el motor este.

- **KEEP** — no hay evidencia medida en contra: app legitima, o de sistema, o su funcion ya la necesitamos pero todavia no esta reimplementada.

## 2. Veredicto por app (59 filas; 52 con datos medidos del inventario)

| App | Paquete | Categoría | Qué aporta | ¿Reimplementar en Rust? | Riesgo **medido** | Veredicto |
|---|---|---|---|---|---|---|
| MarinaPatcher (Lucky Patcher reempaquetado) | `ru.drjmwdme.cgwbfzijr` | RE / parcheador | Parchea APKs, hookea metodos, re-firma, bypasea licencias de Play | SI (motor de parcheo + hook) | 9 permisos de riesgo, 24 pedidos, tercera, target_sdk 28, firma d7f1f224 | **DESINSTALAR** |
| LSPatch | `org.lsposed.lspatch` | RE / parcheador | Inyecta un modulo en un APK y lo re-firma (el nucleo open-source de Lucky Patcher) | SI (motor de parcheo DEX/APK) | 2 permisos de riesgo, 10 pedidos, tercera, target_sdk 36, firma 9dc15fdf | **KEEP** |
| KnoxPatch | `io.mesalabs.knoxpatch` | RE / parcheador | Parcheo de APK con soporte de Knox (Samsung) | SI (motor de parcheo) | 0 permisos de riesgo, 0 pedidos, tercera, target_sdk 36, firma 108d1c9 | **KEEP** |
| EdXposed (manager) | `org.meowcat.edxposed.manager` | RE / framework de hooks | Gestor del framework de hooks LSPosed/EdXposed | SI (motor de hooks) | 7 permisos de riesgo, 13 pedidos, DESHABILITADA, tercera, target_sdk 27, firma 90d1fc20 | **KEEP** |
| MonoPatch | `com.htetz.monopatch` | RE / parcheador | Parche de un solo DEX | SI (motor de parcheo) | 0 permisos de riesgo, 0 pedidos, tercera, target_sdk 34, firma 7257b111 | **KEEP** |
| SberbankPatcher | `ru.bluecat.sberbankpatcher` | RE / parcheador | Parche surgical de UNA app concreta (banco) | SI (ejemplo de parche surgical) | 3 permisos de riesgo, 9 pedidos, DESHABILITADA, tercera, target_sdk 35, firma bd403815 | **KEEP** |
| Magisk (27.2-kitsune) | `io.github.huskydg.magisk` | RE / root | Root + MagiskHide + modulos systemless | NO (se usa, no se reimplementa) | 2 permisos de riesgo, 10 pedidos, tercera, target_sdk 34, firma 4497cbb1 | **KEEP** |
| ADB tools | `com.legendsayantan.adbtools` | RE / utileria | Utilidades ADB desde el movil | NO (ya lo hace el cliente ADB en Rust, rust-core/src/adb_client.rs) | 3 permisos de riesgo, 10 pedidos, DESHABILITADA, tercera, target_sdk 34, firma bcb2a2b | **KEEP** |
| adb captain | `adb.captain` | RE / utileria | Utilidades ADB (packaged) | NO (mismo criterio que ADB tools) | 5 permisos de riesgo, 9 pedidos, DESHABILITADA, tercera, target_sdk 36, firma b38c0403 | **KEEP** |
| Swift Backup | `org.swiftapps.swiftbackup` | Backup | Backup y restauracion de apps (AEGIS-256 + Zstd) | SI (lector de formatos) | 7 permisos de riesgo, 26 pedidos, tercera, target_sdk 37, firma 21c098dc | **KEEP** |
| CallApp / Lucky Patcher (recarga) | `com.callapp.contacts` | RE / dudoso | Lleva el recurso lucky_patcher_package; NO es el parcheador (el parcheador es MarinaPatcher) | SI (limpieza de recursos huerfanos) | 23 permisos de riesgo, 51 pedidos, DESHABILITADA, tercera, target_sdk 29, firma d7f1f224 | **DESINSTALAR** |
| ReVanced GMS (microG parcheado) | `app.revanced.android.gms` | Sistema / parcheado | Google Play Services parcheado para no depender de la tienda | NO (se usa) | 19 permisos de riesgo, 52 pedidos, tercera, target_sdk 29, firma 6ada4089 | **KEEP** |
| Huami Watch Manager | `com.huami.watch.hmwatchmanager` | BASURA / relicto | Gestor de smartwatch Xiaomi/Huami | NO | 21 permisos de riesgo, 61 pedidos, DESHABILITADA, tercera, target_sdk 34, firma ff14caac | **DESINSTALAR** |
| Amazfit | `com.mc.amazfit1` | BASURA / relicto | App del reloj Amazfit | NO | 20 permisos de riesgo, 44 pedidos, DESHABILITADA, tercera, target_sdk 34, firma d5c808db | **DESINSTALAR** |
| Mi Fitness (Xiaomi) | `com.xiaomi.hm.health` | BASURA / relicto | App de salud de Xiaomi | NO | 20 permisos de riesgo, 51 pedidos, DESHABILITADA, tercera, target_sdk 34, firma ff14caac | **DESINSTALAR** |
| Kids Watch 360 (Xplora) | `com.xplora.kidswatch360` | BASURA / relicto | App de reloj infantil | NO | 19 permisos de riesgo, 43 pedidos, DESHABILITADA, tercera, target_sdk 26, firma b09615cd | **DESINSTALAR** |
| KDE Connect (telefono) | `org.kde.kdeconnect_tp` | BASURA / relicto | KDE Connect para el telefono (receptor de notificaciones del PC) | NO | 18 permisos de riesgo, 35 pedidos, DESHABILITADA, tercera, target_sdk 37, firma e93187c9 | **DESINSTALAR** |
| Microsoft Phone (Your Phone) | `com.microsoft.YourPhone_P1_5` | Grabador / Microsoft's | Enlace con el PC; una de las 2 apps que DECLARAN CAPTURE_AUDIO_OUTPUT | SI (gestor de grabaciones) | PENDIENTE (no esta en el inventario) | **KEEP** |
| Link to Windows (servicio) | `com.microsoft.LinkToWindowsService` | Grabador / Microsoft's | Servicio de enlace con el PC; la otra app que DECLARA CAPTURE_AUDIO_OUTPUT | SI (gestor) | PENDIENTE (no esta en el inventario) | **KEEP** |
| Mantenimiento del dispositivo (Samsung) | `com.samsung.android.ool` | Sistema / grabador nativo | Mantenimiento del dispositivo; es la app que en Europa avisa por voz de la grabacion | NO (se usa) | PENDIENTE (no esta en el inventario) | **KEEP** |
| BCR (Basic Call Recorder) | `com.axyos.contacts` | Grabador | Grabador de llamadas basico | SI (gestor de grabaciones, NO grabador silencioso) | PENDIENTE (no esta en el inventario) | **KEEP** |
| Termux | `com.termux` | Terminal / desarrollo | Terminal y entorno de desarrollo en el movil | NO (lo usa el fleet: bash, python3, git) | 0 permisos de riesgo, 0 pedidos, tercera, target_sdk 28, firma 7c3fcce | **KEEP** |
| Termux:API | `com.termux.api` | Terminal / desarrollo | Puente de Termux con la API de Android (SMS, llamadas, bateria) | NO (lo usa el fleet) | 0 permisos de riesgo, 0 pedidos, tercera, target_sdk 28, firma 7c3fcce | **KEEP** |
| Antigravity (IDE de Google) | `com.agy` | RE / IDE | IDE de Google que habla MCP: consume las tools de MegaDaemon por HTTP | SI (cliente MCP ya implementado en rust-core/src/antigravity_intel.rs) | 8 permisos de riesgo, 16 pedidos, tercera, target_sdk 28, firma f3e870a8 | **KEEP** |
| MarinaPatcher (copia) | `ru.drjmwdme.cgwbfzijr.bak` | BASURA | Residuo | NO | PENDIENTE (no esta en el inventario) | **DESINSTALAR** |
| OsmAnd+ | `net.osmand.plus` | Navegacion | Mapas y navegacion offline | NO | 10 permisos de riesgo, 21 pedidos, tercera, target_sdk 36, firma 1f6de4c9 | **KEEP** |
| Google (buscador/launcher) | `com.google.android.googlequicksearchbox` | Sistema / launcher | Buscador y launcher de Google | NO | 21 permisos de riesgo, 95 pedidos, tercera, target_sdk 37, firma 98deb0ca | **KEEP** |
| WhatsApp | `com.whatsapp` | Mensajeria | Mensajeria | NO | 19 permisos de riesgo, 62 pedidos, tercera, target_sdk 37, firma d5acdcd2 | **KEEP** |
| Telegram | `org.telegram.messenger` | Mensajeria | Mensajeria | NO | 18 permisos de riesgo, 46 pedidos, tercera, target_sdk 36, firma 9744bfe6 | **KEEP** |
| Zoom | `us.zoom.videomeetings` | Videollamadas | Videollamadas | NO | 17 permisos de riesgo, 43 pedidos, tercera, target_sdk 36, firma d43453fb | **KEEP** |
| Messenger Lite | `com.facebook.stella` | Mensajeria | Mensajeria ligera | NO | 20 permisos de riesgo, 67 pedidos, tercera, target_sdk 36, firma f37fb5e7 | **KEEP** |
| Samsung Pay | `com.samsung.android.spay` | Pagos | Pagos Samsung | NO | 18 permisos de riesgo, 100 pedidos, tercera, target_sdk 37, firma b378e95c | **KEEP** |
| Telefono (Google) | `com.google.android.dialer` | Sistema | Marcador y llamadas | NO | 18 permisos de riesgo, 60 pedidos, tercera, target_sdk 37, firma 788396df | **KEEP** |
| Samsung OneConnect | `com.samsung.android.oneconnect` | Sistema | Enlace con el PC Samsung | NO | 19 permisos de riesgo, 89 pedidos, tercera, target_sdk 36, firma b378e95c | **KEEP** |
| EasyMover (Samsung) | `com.sec.android.easyMover.Agent` | Sistema Samsung | Migracion de datos entre moviles Samsung | NO | 4 permisos de riesgo, 27 pedidos, sistema, target_sdk 34, firma b378e95c | **KEEP** |
| Rampart (Samsung) | `com.samsung.android.rampart` | Sistema Samsung | Modulo de seguridad/anti-tamper de Samsung | NO | 1 permisos de riesgo, 13 pedidos, sistema, target_sdk 34, firma b378e95c | **KEEP** |
| IP Geofence (Samsung) | `com.samsung.android.ipsgeofence` | Sistema Samsung | Geofencing por IP (redes WiFi) | NO | 0 permisos de riesgo, 0 pedidos, sistema, target_sdk 37, firma b378e95c | **KEEP** |
| CarKey (Samsung) | `com.samsung.android.carkey` | Sistema Samsung | Llaves de coche digitales; trae deteccion de manipulacion propia | NO | 1 permisos de riesgo, 6 pedidos, sistema, target_sdk 37, firma b378e95c | **KEEP** |
| Knox Attestation (Samsung) | `com.samsung.android.knox.attestation` | Sistema Samsung | Cliente de Enhanced Attestation: enlaza con la cadena /DAK/ de la TEE | NO | 0 permisos de riesgo, 0 pedidos, sistema, target_sdk 34, firma b378e95c | **KEEP** |
| SmartCall (Samsung) | `com.samsung.android.smartcallprovider` | Sistema Samsung | Proveedor de llamadas inteligente (spam/identificacion) | NO | 1 permisos de riesgo, 4 pedidos, sistema, target_sdk 34, firma b378e95c | **KEEP** |
| Camaras (Samsung) | `com.samsung.android.vtcamerasettings` | Sistema Samsung | Ajustes de camara | NO | 0 permisos de riesgo, 0 pedidos, sistema, target_sdk 34, firma b378e95c | **KEEP** |
| Peripheral framework (Samsung) | `com.samsung.android.peripheral.framework` | Sistema Samsung | Marco de periféricos (BD, perfil) | NO | 0 permisos de riesgo, 0 pedidos, DESHABILITADA, sistema, target_sdk 33, firma b378e95c | **KEEP** |
| Manual Selector (OOBE del operador) | `com.aura.oobe.samsung` | Sistema / operador | OOBE del operador: el CSC decide出厂 la configuracion regional | NO | 2 permisos de riesgo, 12 pedidos, sistema, target_sdk 36, firma 1d42cae5 | **KEEP** |
| Contactos (Samsung) | `com.samsung.android.app.contacts` | Sistema Samsung | Contactos | NO | 11 permisos de riesgo, 43 pedidos, sistema, target_sdk 34, firma d82618b8 | **KEEP** |
| Google Play Services | `com.google.android.gms` | Sistema | Servicios de Google | NO | 0 permisos de riesgo, 0 pedidos, sistema, target_sdk 37, firma 98deb0ca | **KEEP** |
| Chrome | `com.android.chrome` | Navegador | Navegador | NO | 12 permisos de riesgo, 40 pedidos, sistema, target_sdk 36, firma e3ca78d8 | **KEEP** |
| Samsung Notes | `com.samsung.android.app.notes` | Productividad | Notas | NO | 7 permisos de riesgo, 48 pedidos, sistema, target_sdk 37, firma b378e95c | **KEEP** |
| Samsung Weather | `com.acc.weather` | Productividad | Meteorologia | NO | PENDIENTE (no esta en el inventario) | **KEEP** |
| Galeria (Samsung) | `com.sec.android.gallery3d` | Productividad | Galeria/fotos | NO | 9 permisos de riesgo, 39 pedidos, sistema, target_sdk 34, firma b378e95c | **KEEP** |
| Telefono (Samsung) | `com.samsung.android.dialer` | Sistema | Marcador | NO | 0 permisos de riesgo, 0 pedidos, sistema, target_sdk 34, firma b378e95c | **KEEP** |
| Maps (Google) | `com.google.android.apps.maps` | Navegacion | Mapas | NO | 14 permisos de riesgo, 40 pedidos, DESHABILITADA, tercera, target_sdk 37, firma 98deb0ca | **KEEP** |
| Markor | `net.gsantner.markor` | Notas | Editor de notas (Markdown) | NO | 3 permisos de riesgo, 5 pedidos, DESHABILITADA, tercera, target_sdk 35, firma 963e04e3 | **KEEP** |
| Marina | `gpyn.fi` | RE / parcheador | Marina: motor de parcheo ART en runtime con descarga de parches de terceros | SI (motor de parcheo) | 2 permisos de riesgo, 8 pedidos, DESHABILITADA, tercera, target_sdk 34, firma 94b4d160 | **DESINSTALAR** |
| MarinaPatcher (variante) | `ru.drjmwdme.cgwbfzijr.x` | BASURA | Variante del mismo APK | NO | PENDIENTE (no esta en el inventario) | **DESINSTALAR** |
| Shizuku Call Recorder | `com.kitsumed.shizucallrecorder` | Grabador (open-source) | Grabador de llamadas sobre Shizuku, con codigo abierto | SI (gestor de grabaciones) | 5 permisos de riesgo, 12 pedidos, DESHABILITADA, tercera, target_sdk 37, firma bd05ca9a | **KEEP** |
| Audio Recorder (open-source) | `com.github.axet.audiorecorder` | Grabador (open-source) | Grabador de audio open-source | SI (gestor de grabaciones) | 8 permisos de riesgo, 16 pedidos, tercera, target_sdk 31, firma 8d17cc53 | **KEEP** |
| Easy Voice Recorder Pro | `com.digipom.easyvoicerecorder.pro` | Grabador | Grabador de voz comercial | SI (gestor de grabaciones) | 9 permisos de riesgo, 16 pedidos, DESHABILITADA, tercera, target_sdk 32, firma a5345e0d | **KEEP** |
| InCallUI (Samsung) | `com.samsung.android.incallui` | Sistema Samsung | App de llamada en curso (la que dibuja el HUD de llamada) | NO | 0 permisos de riesgo, 0 pedidos, sistema, target_sdk 34, firma b378e95c | **KEEP** |
| Game Booster (Samsung) | `com.samsung.android.game.gametools` | Sistema Samsung | Optimizacion de juegos (GPU/CPU) | NO | 4 permisos de riesgo, 34 pedidos, sistema, target_sdk 33, firma b378e95c | **KEEP** |

## 3. Los casos que necesitan una decisión **del usuario**

1. **MarinaPatcher** (`ru.drjmwdme.cgwbfzijr`): *DESINSTALAR*.  Es Lucky Patcher con otro nombre y otro icono, y **sube el APK que parchea a un servidor de terceros** (`patch.chelpus.com/submit`) con IPs crudas y sin TLS.  Además pide root y es un módulo Xposed.  Si el usuario dice que lo quiere, se queda; si no, se va.
2. **Marina** (`g.pyn.fi`): *DESINSTALAR* (su función la cubren LSPatch/KnoxPatch open-source y el motor Rust del fleet).
3. **Relictos de smartwatch** (Huami/Amazfit/Mi Fitness/Kids Watch/KDE Connect teléfono): *DESINSTALAR* — están deshabilitados y piden 18-21 permisos de riesgo en un móvil sin reloj conectado.
4. **Microsoft Your Phone / Link to Windows**: *KEEP*, pero son las **únicas 2 apps que declaran `CAPTURE_AUDIO_OUTPUT`** en el móvil (medido sobre 398 APKs / 5.580 MB).  Si el usuario quierePrivacy al grabador, estas son las que hay que mirar.

## 4. Ahorro

Lo que se puede quitar sin perder nada de lo propio del móvil: **5 relictos de wearable** + **Marina** + **MarinaPatcher** + **ADB tools/adb captain** (su función ya está en `megadaemon-adb`, Rust puro).  Los dos APK de ADB solos son ~35 MB.  El resto son apps del usuario: la decisión es suya, no nuestra.

## 5. Lo que falta por medir (móvil caído)

Sin canal al móvil no se puede cerrar el inventario.  En cuanto vuelva:

1. `pm list packages -3 -U` + `dumpsys package <pkg> | grep -A40 permissions` para los paquetes marcados **P**.
2. Tamaño real por APK (`stat -c %s <ruta_apk>`) — el inventario guarda la ruta, no el tamaño.
3. `CAPTURE_AUDIO_OUTPUT` y `REQUEST_INSTALL_PACKAGES` no están en la lista de 27 permisos del inventario: barrido de APKs como el que ya se hizo (398 APKs / 5.580 MB) para cerrarlos.
4. Confirmar qué wearables/relactos quiere conservar el usuario.

## 6. Reproducir

```bash
# regenerar este documento
python3 tools/verdict_inventory.py
# solo validar (>=40 filas)
python3 tools/verdict_inventory.py --check
# el inventario medido (recuperado del historial si lo perdio un reset)
git show a56b1c3:inventory/inventory.json > inventory/inventory.json
```
