# RE y diseño: control de gasto de datos de Jazztel en MegaDaemon

> Módulo previsto: `rust-core/src/gasto_datos.rs` (nuevo) · Comando remoto: `gasto_datos`
> Operador: Jazztel (MASORANGE / Orange España) · MCC/MNC medido: `21403` · `gsm.operator.alpha=JAZZTEL`
> Fecha: 2026-11-26 · Español; identificadores en inglés.
>
> **Regla del documento**: lo que no se ha medido se dice `NO VERIFICADO`. No se
> afirma el contenido de un DEX, de un JSON de API ni de una factura que no se ha
> leído byte a byte. Cada afirmación lleva su ruta exacta o su URL.

---

## 0. Resumen ejecutivo

| Pregunta | Respuesta |
|---|---|
| ¿Se puede medir el gasto en el móvil? | **Sí**, de tres formas independientes: (a) `NetworkStatsManager`/`TrafficStats` (por UID), (b) el contador del kernel (`/proc/net/xt_qtaguid/stats` en legacy; mapas eBPF en P+), (c) la **caché local de la app Jazztel** (`/data/user/0/com.orange.bluu.jazztel/no_backup`, 20,4 MiB medidos). |
| ¿Es lo mismo que lo que factura Jazztel? | **No.** El sistema mide *bytes que movió el proceso*; Jazztel factura *bytes que contabilizó la red del operador*. Coinciden casi siempre y difieren en: tethering, roaming, APN de respaldo, Data Saver y **corte de ciclo de facturación**. |
| ¿Dónde está la verdad de facturación? | En la **API del operador** que consume la app ("Mi consumo") y en la **factura PDF**. El daemon debe leer esas dos, no el contador del sistema. |
| ¿Se puede reimplementar sin la app? | El **caché local** da el consumo sin red ni MITM (ruta de menor riesgo). La **API** lo refresca. El **contador del sistema** sirve de contraste y de desglose por app. |
| ¿Hay material de RE? | **Sí, en Drive**: APK base de 44 MB (`md5 de06e522…`), sus splits, sus datos y 9 facturas PDF. **No** hay copia del APK en el workspace ⇒ el análisis de DEX/permisos queda `NO VERIFICADO`. |
| ¿Qué falta? | El móvil. Todas las mediciones de §A.10 y §D.3 dependen de él. |

**Decisión que gobierna el documento**: `gasto_datos` **no** pretende ser el sistema
de datos de Android; es un **reconciliador**. Recoge tres fuentes (contador del
sistema, caché de la app, API/factura del operador), las compara y expone la
diferencia. El valor para el usuario no es "cuánto ha gastado el sistema" sino
**"cuánto ha gastado Jazztel y por qué no coincide con mi Android"**.

---

# PARTE A — Cómo se mide el gasto de datos en Android (base verificable)

## A.1 Las dos preguntas que se confunden

| Magnitud | Qué es | Quién la mide |
|---|---|---|
| **Tráfico del proceso** | Bytes que han salido/entrado por los sockets de un UID. Medida **local, del dispositivo**. | `netd` (`NetworkStatsService`), en el kernel |
| **Consumo facturable** | Bytes que el operador ha contabilizado en la red y que aparecen en la factura. | El operador (contadores de red, CDR/PCRF) |

No pueden diferir arbitrariamente (la facturación va por el APN y el operador, y
el sistema local ve lo que pasa por la interfaz), pero **difieren** en al menos
cinco casos (§A.6, §A.8).

**Regla de diseño derivada**: todo número que el daemon presente como "gasto"
lleva su **origen** y su **confianza**:

```jsonc
{ "bytes": 123456789, "origen": "jazztel_api", "confianza": "alta" }
```

## A.2 `TrafficStats` — el contador por UID

`android.net.TrafficStats` expone contadores por UID y por socket.

| Método | Devuelve |
|---|---|
| `getTotalBytes()` / `getTotalTxBytes()` | Bytes desde el arranque (`getTotalBytes()` = RX + TX) |
| `getUidBytes(int uid)` / `getUidTxBytes(int uid)` | Bytes de un UID (un app sin privilegios sólo ve los suyos) |
| `setThreadStatsUid(int)` / `getThreadStatsUid()` | **Reasigna** la facturación del tráfico a otro UID (servicio por cuenta de otro: descargas, MDM) |
| `setThreadStatsTag(int)` / `getThreadStatsTag()` | Etiqueta el socket con un **tag** de categoría (0..255): un UID puede tener varios tags |
| `getSocketStats()` | Tag/UID/fds de los sockets abiertos |

`getUidBytes(de_otro_uid)` devuelve `-1` sin `READ_NETWORK_STATE`. Para ver *todos*
los UIDs hace falta `NetworkStatsManager` (§A.3) con `PACKAGE_USAGE_STATS`.

## A.3 `NetworkStatsManager` — la API moderna, con buckets

`android.app.usage.NetworkStatsManager` (API 24+) es la vía correcta: devuelve
histórico en *buckets* (bins de tiempo) y separa por dimensiones.
Fuente: <https://developer.android.com/reference/android/app/usage/NetworkStatsManager>

| Método | Semántica |
|---|---|
| `querySummary(networkType, subscriberId, start, end)` | **Sólo los UID del usuario llamante.** Agrega en el tiempo; **no** agrega por estado/uid/metered/roaming ⇒ devuelve muchos buckets |
| `querySummaryForUser(...)` | Igual, todos los usuarios del perfil |
| `querySummaryForDevice(...)` | **Todo el dispositivo**, un único bucket (requiere perfil o device owner) |
| `queryDetails(...)` / `queryDetailsForUid(...)` | Detalle, no agregado |
| `queryDetailsForUidTag(..., uid, tag)` | Detalle por UID **y tag** |
| `queryDetailsForUidTagState(..., uid, tag, state)` | Detalle por UID + tag + **estado** |
| `registerUsageCallback(...)` | Aviso al superar un umbral de bytes ⇒ **el gancho natural para "aviso de MB restantes"** |

**Dimensiones de cada bucket** (`NetworkStats.Bucket`):

| Dimensión | Valores | Por qué importa aquí |
|---|---|---|
| `uid` | `UID_ALL` o un UID | Reparto por app |
| `tag` | `TAG_NONE`, `TAG_SYSTEM`, `TAG_SYSTEM_DHCP`, `TAG_SYSTEM_NTP`, `TAG_SYSTEM_RESOLVER`, `TAG_APP`, `TAG_RADIO`, `TAG_VPN`, `TAG_KERNEL`, `TAG_RUNTIME` | `TAG_RADIO` = tráfico del módem, no de una app |
| `state` | `STATE_ALL`, `STATE_CONNECTED`, `STATE_DISCONNECTED` | `DISCONNECTED` = tráfico en cola, no en vuelo |
| `defaultNetwork` | `DEFAULT_NETWORK_ALL/YES/NO` | Separa la red por defecto de la VPN o red propia |
| `metered` | booleano | **La clave de la facturación**: una red metered es la que puede costar dinero |
| `roaming` | booleano | Itinerancia: se factura distinto |
| `networkType` | `TYPE_MOBILE`, `TYPE_WIFI`, `TYPE_ETHERNET`, `TYPE_VPN` | ⇒ aquí está el **WiFi vs móvil** |

### La etiqueta `TRAFFIC_BACKGROUND`

`NetworkStats.Bucket.TRAFFIC_BACKGROUND` es un **tag**, no un estado. Marca tráfico
generado sin la app en primer plano:

* La app puede ponerlo ella misma con `TrafficStats.setThreadStatsTag(...)`.
* Android calcula "foreground vs background" en la pantalla de Uso de datos
  cruzando el tráfico con el estado del proceso en el periodo ⇒ **ese desglose
  NO sale de un único campo del contador**: es una heurística del sistema.

⇒ Para `gasto_datos` el tag `TRAFFIC_BACKGROUND` es una **pista**, no un veredicto.

### Permisos

| Permiso | Para qué |
|---|---|
| `PACKAGE_USAGE_STATS` | Histórico de `NetworkStatsManager`. **No** es permiso de runtime normal: se concede por `AppOpsManager` (`OPSTR_GET_USAGE_STATS`); con root, `cmd appops set <pkg> GET_USAGE_STATS allow` |
| `READ_NETWORK_STATE` | Estado de red y `NetworkCapabilities` |
| Rol de carrier / app de operador | Acceso de nivel operador: lo que Jazztel ve en su app |
| Device owner / profile owner | `querySummaryForDevice` |

## A.4 `ConnectivityManager` y `NetworkCapabilities` — clasificar el transporte

`ConnectivityManager.getNetworkCapabilities(network)`.
Fuente: <https://developer.android.com/reference/android/net/NetworkCapabilities.html>

| `TRANSPORT_*` | Para el gasto |
|---|---|
| `TRANSPORT_CELLULAR` | Radio del móvil. **La red que factura Jazztel** |
| `TRANSPORT_WIFI` | WiFi. Normalmente **no** factura, salvo WiFi de pago |
| `TRANSPORT_ETHERNET` | Cable. No factura (suele ser la fibra de casa) |
| `TRANSPORT_VPN` | **Trampa clásica**: VPN sobre WiFi conserva `TRANSPORT_WIFI`+`TRANSPORT_VPN`; sobre móvil conserva `TRANSPORT_CELLULAR`+`TRANSPORT_VPN`. Hay que mirar **los dos** |
| `TRANSPORT_BLUETOOTH/USB/LOWPAN/SATELLITE/THREAD` | Restos; no portan IP aquí |

**Regla que impone la propia documentación de Android** ("Reading network state",
<https://developer.android.com/develop/connectivity/network-ops/reading-network-state>):
*"to query whether a network costs money to the user, do not look at the transport,
but instead look at the absence or presence of `NET_CAPABILITY_NOT_METERED`"*.

⇒ **`NET_CAPABILITY_NOT_METERED` es la señal de facturación, no el transporte.** El
daemon debe guardar **ambos**: el transporte (para el desglose WiFi/móvil) y
`metered` (para lo que cuesta dinero). Además existen
`NET_CAPABILITY_TEMPORARILY_NOT_METERED` (Android 12+) y
`NET_CAPABILITY_LOCAL_NETWORK` (red de tethering, §A.8).

## A.5 Dónde vive el dato según la versión de Android

### Legacy (Android ≤ 8.1): `xt_qtaguid`

La contabilidad por app la hacía el kernel con el módulo netfilter `xt_qtaguid`.
Fuente: <https://source.android.google.cn/docs/core/data/kernel-overview>

* El framework marcaba los sockets (`system/core/libcutils/qtaguid.c`) escribiendo en
  `/proc/net/xt_qtaguid/ctrl`; el módulo contaba **por socket** y asignaba la
  propiedad al **UID** del socket (`sk_socket->file->f_cred->fsuid`).
* **Tag y UID facturable son cosas distintas**: la tabla guardaba `acct_tag`
  (etiqueta de la app) y `uid_tag` (UID al que se factura); se podían desviar los
  bytes de una app a otro UID (mecanismo de `setThreadStatsUid`).

**Rutas exactas** (AOSP, `core/jni/android_net_TrafficStats.cpp`,
<https://android.googlesource.com/platform/frameworks/base/+/981962e/core/jni/android_net_TrafficStats.cpp>):

| Ruta | Contenido | Lo parsea |
|---|---|---|
| `/proc/net/xt_qtaguid/stats` | `idx iface acct_tag_hex uid_tag_int cnt_set rx_bytes rx_packets tx_bytes tx_packets …` | `nativeGetUidStat` |
| `/proc/net/xt_qtaguid/iface_stat_fmt` | `iface rx_bytes rx_packets tx_bytes tx_packets …` | `nativeGetIfaceStat` |
| `/proc/net/xt_qtaguid/ctrl` | Control: `t <fd> <tag> <billed_uid>` | `qtaguid_tagSocket()` |
| `/proc/net/xt_qtaguid/iface_stat/<iface>/` | Estadísticas por interfaz | kernel |

⚠️ **Corrección de la premisa**: `/proc/net/xt_qtaguid_ctrl` (fichero plano con
guion bajo) es el formato **pre-Android-4**. Hoy es un **directorio**
`/proc/net/xt_qtaguid/` con `ctrl`, `stats` e `iface_stat/`. En el móvil de este
proyecto (Samsung **SM-S908B**, Android 13 — `docs/PHONE_INVENTORY.md:22`)
**ninguna** de estas existe: el dispositivo salió de fábrica con Android 12+, y
Android 9+ con kernel ≥ 4.9 **debe** usar eBPF (V12, V13).

### Android 9 (Pie): transición

<https://developer.android.com/about/versions/pie/android-9.0-changes-all>:
*"Beginning with Android 9, apps are not allowed to have direct read access to
files in the `/proc/net/xt_qtaguid` folder. … The public APIs that rely on these
files, `TrafficStats` and `NetworkStatsManager`, continue to work as intended."*
Además, en Android 9 el *socket tag* **sobrevive** al paso por binder
(`ParcelFileDescriptor`), lo que cambia `queryDetailsForUidTag()`.

### Android 11+ y dispositivos P+ con kernel ≥ 4.9: eBPF + cgroup 2

Fuente: <https://source.android.google.cn/docs/core/data/ebpf-traffic-monitor>

* Filtro **cgroup eBPF** en la capa de transporte (cuenta contra el UID correcto
  según el socket y el tag puesto desde userspace) + **`xt_bpf`** en las cadenas
  `bw_raw_PREROUTING` y `bw_mangle_POSTROUTING` (cuenta contra la interfaz).
* Contadores en **mapas eBPF**: `traffic_tag_stats_map`, `traffic_uid_stats_map`,
  `traffic_iface_stats_map`; en runtime, `traffic_cookie_tag_map` y
  `traffic_uid_counterSet_map`.
* `NetworkStatsService` **lee esos mapas** ⇒ misma dimensionalidad que
  `NetworkStatsManager` ("uid + tag + iface + state + metered + roaming").
* Diagnóstico: `dumpsys netd` (estado del `trafficController`) y
  `dumpsys netd trafficcontroller` (volcado de cada mapa).

⇒ En el SM-S908B (Android 13) aplica **eBPF** ⇒ `/proc/net/xt_qtaguid` no existe
⇒ la única vía es `NetworkStatsManager`/`TrafficStats` o el volcado de `netd`.

### El binder `netstats`

* Servicio: `netd` (`/system/bin/netd`); interfaz Binder
  `android.net.INetworkStatsService`; tie service `android.net.INetworkManagement`.
* **`TrafficStats` y `NetworkStatsManager` son clientes de ese binder.** En
  Android 9+ el acceso directo a los ficheros se movió *al servicio* ⇒ no hay vía
  alternativa para leer los contadores saltándose el binder.
* Trazas útiles: `dumpsys netstats`, `dumpsys netstats detail`,
  `dumpsys netd trafficcontroller`.

## A.6 Data Saver, datos de fondo y "lo que Android cuenta ≠ lo que factura"

| Mecanismo | Qué hace | Efecto en el número |
|---|---|---|
| **Data Saver** | Bloquea el tráfico de fondo de las apps no exentas | El tráfico **no ocurre** ⇒ contador y factura bajan a la vez |
| **Restricción de datos de fondo** (por app) | Corta la red de una app en segundo plano | Igual que arriba |
| **Excepciones de Data Saver** | Esas apps siguen gastando | ⇒ el desglose por app **no cuadra** con la factura |
| **Foreground vs fondo** | El sistema lo estima cruzando tráfico y estado del proceso | **Estimación**, no dato del contador |
| **Red no mediada** | La app puede saltarse la red móvil | El operador **no** lo factura; el sistema tampoco lo ve |

⇒ El sistema y el operador cuentan lo mismo salvo: (a) el sistema ve tráfico que
el operador no factura (WiFi, tethering local, redes no medradas); (b) el
operador ve tráfico que el sistema no ve (roaming ajeno, APN de respaldo); (c)
hay Data Saver o restricciones que cortan tráfico por el medio.

## A.7 El "Data Usage" de Samsung: ¿hay un provider propio?

**Respuesta: NO VERIFICADO que exista un provider Samsung propio de consumo de
datos; sí está verificado que la pantalla existe y que los datos los aporta el
sistema de AOSP.**

Verificado por fuentes públicas:

| Hecho | Fuente |
|---|---|
| Samsung tiene **Ajustes → Conexiones → Uso de datos → Uso de datos móviles**, con desglose por app, "Ciclo de facturación" y "Alerta de datos"; además "Data saver" y "Redes permitidas para apps" | <https://www.samsung.com/us/support/answer/ANS10002914/> · <https://www.samsung.com/uk/support/mobile-devices/how-do-i-check-data-usage-on-my-samsung-galaxy-device/> · <https://www.att.com/device-support/article/wireless/KM1426213/Samsung/SamsungSMG998U> (guía del SM-G998U: mismo modelo base) |
| **La propia Samsung dice que su contador es "una estimación"** del ciclo mensual | <https://www.samsung.com/uk/support/mobile-devices/how-do-i-check-data-usage-on-my-samsung-galaxy-device/> |
| `com.android.providers.telephony` es el **TelephonyProvider de AOSP** (SMS/MMS + APNs). **No** guarda estadísticas de tráfico | <https://android.googlesource.com/platform/packages/providers/TelephonyProvider/> · <https://sysapps.bayton.org/packages/com.android.providers.telephony> |
| `com.samsung.android.providers.context` es el "Context Service" de Samsung (contexto, no datos) | índice de apps de sistema (bayton.org, anterior) |

**NO VERIFICADO**: si Samsung instala un provider con tablas de consumo (hay
indicios en listados de terceros de un "Data usage (Policy & Stats)"). Verificar
en el móvil con los comandos C9 de §D.3.

**Diseño (independiente de la respuesta)**: `gasto_datos` **no** debe depender de
un provider propietario; se apoya en `NetworkStatsManager`, que es AOSP y está
garantizado en cualquier Android 10+.

## A.8 Tethering: tráfico que SÍ factura Jazztel y AOSP no ve como "app"

* Con el móvil de **hotspot WiFi**, el tráfico de los dispositivos conectados sale
  por la interfaz celular (`rmnet*`).
* En AOSP ese tráfico se contabiliza por **UID del sistema** (root/system o el
  proceso de tethering), **no** por la app que lo originó en el otro dispositivo.
* El operador lo ve como **datos del móvil**: se factura al SIM.
* `NetworkCapabilities` de la red de hotspot lleva `NET_CAPABILITY_LOCAL_NETWORK`
  (Android 12+) y, con upstream celular, `NET_CAPABILITY_DUN` (la pasarela de
  tethering del operador).
* `ConnectivityManager.getTetheredIfaces()` / `TetheringManager.getTetheredIfaces()`
  (API 31+) devuelve las interfaces que sirven tethering.

⇒ Si el móvil ha hecho de hotspot, el total por UID de `NetworkStatsManager`
**suma menos** que lo que factura Jazztel. El daemon debe detectarlo y reportarlo
como **"diferencia explicada: tethering"**, no inventar un reparto. Con offload
por hardware hay además otro trozo servido por el HAL (`getForwardedStats`):
<https://source.android.com/docs/core/data/tethering-offload>.

## A.9 Equivalencia: qué es "el gasto que factura Jazztel" y qué no

| Dato | ¿Equivale al facturable? | Matiz |
|---|---|---|
| `NetworkStatsManager` cellular, `metered=true`, `roaming=false` | **Sí** (desvía <1 %) | Mismo APN, mismo radio; desvía NAT de carrier, compresión, control |
| `NetworkStatsManager` cellular con `metered=false` | **No** | Red no facturable |
| `NetworkStatsManager` WiFi | **No** (salvo WiFi de pago) | El operador no lo ve |
| `NetworkStatsManager` con `TRANSPORT_VPN` | **Depende** | Hay que mirar el transporte subyacente |
| `TrafficStats.getUidBytes()` del propio proceso | **No** | Sólo ese proceso, y sólo desde que arrancó |
| `/proc/net/xt_qtaguid/stats` | **Sí** (legacy) | Sólo Android ≤ 8.1 o kernels con el módulo |
| Mapas eBPF (`dumpsys netd trafficcontroller`) | **Sí** | Misma dimensionalidad que `NetworkStatsManager` |
| Pantalla "Uso de datos" de Samsung | **No** | Samsung la llama "estimación"; el corte de ciclo es local, no el del operador |
| **Caché local de la app Jazztel** | **Sí** | Es el dato del operador tal cual lo ve el cliente |
| **API del operador** | **Sí** | Verdad del ciclo vigente |
| **Factura PDF** | **Sí, y es la verdad histórica** | Sólo cuadra con el ciclo cerrado |
| Total del sistema vs del operador | **No comparables** | Se explica con tethering, roaming, WiFi, Data Saver, delay de contabilidad y **corte de ciclo** |

## A.10 Cómo se mide esto desde este daemon

Vía `root_exec` (`rust-core/src/android.rs:1104`). ⚠️ **Tope de 1024 caracteres de
salida** ⇒ los volcados grandes se escriben a fichero y se leen por franjas.

```bash
# 1) Versión de Android y kernel (decide legacy vs eBPF)
su -mm -c 'getprop ro.build.version.release; getprop ro.build.version.sdk; uname -r'
# 2) ¿Existe xt_qtaguid? (vacío ⇒ eBPF)
su -mm -c 'ls -d /proc/net/xt_qtaguid 2>&1; ls -la /proc/net/xt_qtaguid 2>&1 | head -5'
# 3) eBPF: estado y volcado de mapas
su -mm -c 'dumpsys netd | head -40'
su -mm -c 'dumpsys netd trafficcontroller | head -120'
# 4) Agregado por UID
su -mm -c 'dumpsys netstats | head -150'
su -mm -c 'dumpsys netstats detail > /data/local/tmp/megadaemon/netstats_detail.txt'
su -mm -c 'wc -l /data/local/tmp/megadaemon/netstats_detail.txt'
# 5) Interfaces y redes
su -mm -c 'ip -br addr; cat /proc/net/dev | head -12'
su -mm -c 'dumpsys connectivity | grep -iE "NetworkAgentInfo|TRANSPORT" | head -30'
# 6) ¿Tethering activo? (explica diferencias)
su -mm -c 'ip -br link | grep -iE "ap|wlan|usb"'
su -mm -c 'dumpsys tethering | head -40'
# 7) UID de la app Jazztel (para filtrar el contador)
su -mm -c 'dumpsys package com.orange.bluu.jazztel | grep -m1 userId='
```

Para leer por franjas sin pasarse del tope:
`su -mm -c 'sed -n "100,160p" /data/local/tmp/megadaemon/netstats_detail.txt'`.

---

# PARTE B — La app Jazztel: qué desofuscar y qué reimplantar

## B.0 Qué material hay (VERIFICADO, metadatos de Drive)

Listado del 2026-11-26 con la credencial OAuth de
`/workspace/md-secrets/gdrive_credentials.json` (**sólo metadatos**, sin
descargar contenido). Los nombres siguen la convención del propio daemon
(`rust-core/src/backup_formats.rs:616`): `app` = APK, `splits` = APK de split,
`dat` = datos, `extdat` = datos externos, `extra` = metadatos.

| Fichero | Bytes | `md5Checksum` | Drive id | Fecha |
|---|---|---|---|---|
| `com.orange.bluu.jazztel.app (SM-G998B) (id-20261126-063715-WO)` | 44.050.916 | `de06e522fcae656d6f259a782ee7e710` | `1iqPps2LWutrcSLWeQVP2dAkX7nsZsoCB` | 2026-11-26 |
| `com.orange.bluu.jazztel.dat (SM-G998B) (…-20261126-063715-WO)` | 6.404.970 | `e97916f863dc839d01b3b82429d7662f` | `1w4wb8ZQMRmJ4ALfMh2Oi1-9SDr2dQNfa` | 2026-11-26 |
| `com.orange.bluu.jazztel.splits (SM-G998B) (…-20261126-063715-WO)` | 2.879.144 | `bd947ba4cf01f1997408a9fecbec553d` | `1CJgxYS-OAu06xM0vhEyGwwSPcDSjo2bk` | 2026-11-26 |
| `com.orange.bluu.jazztel.extdat (SM-G998B) (…-20261126-063715-WO)` | 2.041.146 | `a11270b9e54e5f3e6110b546ebece6d9` | `1qiJHoni_Na1dM0rGRTcIg1ShiBHjGaNw` | 2026-11-26 |
| `com.orange.bluu.jazztel.extra (SM-G998B) (…-20261126-063715-WO)` | 1.063 | `66bac31818222af1822289ff7966b69a` | `1pnoLTF0j4bK_vjHlXHJUyDxMw4ZiEk_z` | 2026-11-26 |
| `com.orange.bluu.jazztel.app (SM-G998B) (id-20261124-065332-XI)` | 44.050.916 | `de06e522fcae656d6f259a782ee7e710` | `1x7IYfJX_ZbOo0VoWWUpCiCdxGindFTEI` | 2026-11-24 |
| `com.orange.bluu.jazztel.app (SM-G998B) (id-20261122-064415-MR)` | 43.976.922 | `c16699fcd6fa96cc98d8bec335c66df0` | `1ECjAbVi4Y0msgi1d-0w-mokgpRit3vMn` | 2026-11-22 |

**Nota**: los APK de 11-22 (43.976.922 B) y de 11-24/26 (44.050.916 B) tienen
`md5` distinto ⇒ **versiones distintas** de la app. El más reciente es el de
2026-11-26.

⚠️ **Discrepancia de modelo (a verificar)**: los artefactos de Drive llevan la
etiqueta **"(SM-G998B)"**, pero el móvil del inventario actual es un
**SM-S908B** (`docs/PHONE_INVENTORY.md:22`, Android 13). ⇒ O bien el APK se
extrajo de otro dispositivo, o bien la etiqueta del nombre es histórica.
**No afecta al análisis del DEX** (el contenido es el que sea), pero sí a la
versión instalada: por eso C13 compara la `versionName` del móvil.

### Las facturas de Jazztel (VERIFICADO, metadatos)

En la carpeta privada `15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0` hay una carpeta
`Facturas jazztel` (id `12KIiM3NqD6j1RypMS6Fbgl9gwCln3z0R`) con **9 facturas PDF**
con el patrón `<NIF>_J<numero_de_factura>.pdf`:

| Nombre (NIF enmascarado) | Bytes | | Nombre | Bytes |
|---|---|---|---|---|
| `<NIF>_J1674984997.pdf` | 88.564 | | `<NIF>_J1729086766.pdf` | 59.215 |
| `<NIF>_J1688384590.pdf` | 76.248 | | `<NIF>_J1742584565.pdf` | 71.650 |
| `<NIF>_J1701929783.pdf` | 95.205 | | `<NIF>_J1756259961.pdf` | 69.157 |
| `<NIF>_J1715530770.pdf` | 64.677 | | `<NIF>_J1769845369.pdf` | 129.504 |
| | | | `<NIF>_J1783480167.pdf` | 98.702 |

⚠️ **PII en los nombres**: el NIF del titular está en el nombre del PDF. **Ese
nombre no debe aparecer en este repositorio, que es público.** El daemon
referencia las facturas por **hash del contenido** o por un alias
(`factura-2026-03`), nunca por el nombre de Drive. Misma regla que ya aplica al
repo con `state/*.key`, `md-secrets/`, `*.pem` (§C.4).

## B.1 Qué **no** hay: el APK no está en el workspace

```
find /workspace -maxdepth 6 -iname "*jazztel*" -o -iname "*bluu*" -o -iname "*orange*"  →  0 resultados
```

Los APK presentes en `/workspace/apks` y `/workspace/apk_work` son `EdXposed`,
`EdXposedManager`, `LSPatch_manager`, `MarinaPatcher`, `MonoPatch`, `TrustMe`,
`daemon.apk`, `manager.apk` y `swift-5.1.1.apk`. **Ninguno es de Jazztel.**

⇒ **El análisis del DEX de Jazztel (clases, permisos, receivers, providers,
strings de facturación) está `NO VERIFICADO`.** La §B.2 dice **qué hay que
mirar**, no lo que se ha visto. Motivo: la instrucción del encargo es **no
descargar nada nuevo**; se ha listado el Drive (metadatos) pero no se ha bajado
el APK.

### Procedimiento exacto para completar el análisis (cuando se autorice)

```bash
# 0) Traer el APK base (44 MB). Credenciales FUERA del repo.
python3 /workspace/md-root/tools/drive_fetch.py \
  --file-id 1iqPps2LWutrcSLWeQVP2dAkX7nsZsoCB \
  --out /workspace/apk_work/jazztel/jazztel.apk \
  --creds /workspace/md-secrets/gdrive_credentials.json
# El tool imprime sha256 + token_sha16: NO imprime el token.

# 1) Los splits (config.*.apk) hacen falta para el manifiesto final
python3 /workspace/md-root/tools/drive_fetch.py \
  --file-id 1CJgxYS-OAu06xM0vhEyGwwSPcDSjo2bk \
  --out /workspace/apk_work/jazztel/jazztel.splits

# 2) Análisis con las herramientas DEL REPO (no jadx, no apktool).
#    El repo ya tiene descompilador DEX propio: rust-core/src/dex_decompile.rs
python3 /workspace/md-root/tools/apk_scan.py /workspace/apk_work/jazztel/jazztel.apk
python3 /workspace/md-root/tools/decompile_apk.py /workspace/apk_work/jazztel/jazztel.apk > jazztel.java

# 3) Strings de facturación + endpoints (app_intel::extract_endpoints)
strings -a jazztel.apk | grep -iE 'consumo|datos|factur|bono|tarifa|GB|restante' | head -50
```

Qué extraer, **en este orden** (= fases de §B.2):

| # | Qué | Dónde | Herramienta del repo |
|---|---|---|---|
| 1 | `package`, `versionName`, `versionCode`, `minSdk`/`targetSdk` | Manifiesto (AXML) | `apk_inventory::analyze_apk` → `manifest` |
| 2 | Permisos y declarados, `meta-data` | Manifiesto | `apk_inventory` → `permissions_risk`, `manifest_axml` |
| 3 | Receivers/Services/Activities **exportados** | Manifiesto | `apk_inventory` → `exported_without_permission` |
| 4 | Providers y sus `authorities` | Manifiesto | `apk_inventory` → `manifest_axml` |
| 5 | Strings de facturación | DEX | `dex_decompile::dex_strings` + `grep` |
| 6 | **URLs de la API de consumo** | DEX + assets | `app_intel::extract_endpoints` |
| 7 | Clases que contienen esos strings | DEX | `dex_decompile::decompile_class` |
| 8 | Ofuscación / empaquetador / parcheo | DEX + ZIP | `apk_inventory` → `obfuscation`, `packers`, `patch_frameworks` |
| 9 | Fingerprint de la firma | META-INF | `apk_inventory` → `signature` |
| 10 | Libs nativas y ABI | `lib/` | `apk_inventory` → `native_libraries` |

> **Markers a buscar** (los que localizan la facturación): `consumo`,
> `Mi consumo`, `datos`, `megas`, `GB`, `factura(s)`, `tarifa`, `bono(s)`,
> `restante(s)`, `consumoTotal`, `dataConsumption`, `datosConsumidos`, `lines`,
> `lineas`, `renewal`, `renovacion`, `fechaFacturacion`, `ciclo`, `cuota`,
> `megapool`, `bundle`.

## B.2 Cadena de desofuscación (orden y herramientas)

Se sigue el orden de `docs/APK_OBFUSCATION_PLAYBOOK.md` (referencia operativa del
escáner) sobre `docs/APK_ANALYZER_SPEC.md`. **No se reescribe el método: se usa.**

### Fase 0 — Inventario bruto

`ZIP ─► AndroidManifest.xml (AXML) ─► classes*.dex ─► strings (MUTF-8) ─► lib/**/*.so ─► META-INF`

Herramienta: `apk_inventory::analyze_apk(path)` → `entries`, `dex_files`,
`native_libs`, `abis`, `assets`, `manifest`, `fingerprint`, `sha256`.
**Salida**: SHA-256 (cotejar con `md5 de06e522…`), nº de DEX, ABIs y presencia de
`assets/xposed_init` (⇒ alerta de supply-chain, §4.1 del playbook).

### Fase 1 — Identificadores (renombrado)

**Qué se busca**: clases de 1-3 caracteres (`^[A-Za-z]{1,2}[0-9]{0,2}$`), ratio
> 40 % de clases cortas, `SourceFile` ausente, y la prueba definitiva
`SourceFile = "r8-map-id-<sha256>"` (⇒ R8, confianza 99).
**Herramientas**: `dex_decompile::analyze_obfuscation(dex)` → `ObfuscationReport`
(`classes_short_ratio`, `encrypted_strings_ratio`, `control_flow_obfuscation`,
`source_file_ratio`); `dex_decompile::obfuscation_map(dex)`.
**Salida**: tabla `ofuscado → legible` **propuesta**, nunca impuesta: el playbook
§6.2 fija `RENAME_CONFIDENCE_THRESHOLD = 60`; por debajo se conserva el nombre
original y se marca `applied: false`.

### Fase 2 — Strings (cifrados)

**Qué se busca**: el patrón canónico "cadena + routine de descifrado" (§5.1 del
playbook), que exige **las tres**: (1) campo estático de tipo
`[I`/`[B`/`[C`/`[Ljava/lang/String;`, (2) un método con `xor-int`/`xor-long`,
(3) ese método construye un `String` (`new-instance` + `invoke-direct <init>`).
**Herramientas**: `analyze_obfuscation` → `encrypted_strings`; `obfuscation_findings(dex)`
→ `Finding[]` con `kind = "string-decryptor"`.
**Calibración ya medida en el repo** (playbook §6.4): `La32;->h` → `decodeBase64`
(88), `La05;` → `dbHelper` (83), `La72;->b` → `encrypt` (84). El motor **distingue
cifrar de descifrar** siguiendo la constante de modo a `Cipher.init` (1=ENCRYPT,
2=DECRYPT).
**Salida**: las clases que descifran strings con su **punto de entrada**, que es
donde hay que enganchar el hook (§B.3).

### Fase 3 — Flujo de control

**Qué se busca**: `control_flow_obfuscation` del informe: bloques opacos, "jump
spaghetti", predicados inalcanzables, aplanamiento de flujo.
**Herramientas**: `dex_decompile::disassemble_method(dex, class_idx, method_idx)`;
`decompile_class(dex, idx)`.
**Por qué importa**: si el flujo está aplanado, la clase que **decide** qué
petición de consumo hacer puede estar separada de la que la **formatea** ⇒ eso
determina dónde hookear.

### Fase 4 — Ofuscación de recursos

**Qué se busca**: `resources.arsc` con nombres renombrados (`res/abc.png`,
`res/AB.xml`), `assets/` con ficheros de nombre corto (cifrados), `strings.xml`
con claves sin sentido.
**Herramientas**: `apk_inventory` → `assets`, `entries`; `axml_parser` para XML;
`app_intel::extract_endpoints` sobre los assets de texto.
**Por qué importa**: los **endpoints de la API de consumo** pueden estar en un
asset (un `config.json` cifrado) y no en el DEX ⇒ el plano B (§B.3) empieza por
el **tráfico**, no por el código.

### Fase 5 — Puntos de carga dinámica

Si aparece `DexClassLoader`/`InMemoryDexClassLoader`/`PathClassLoader` con
constante de texto (§5.2 del playbook), el DEX visible **no es el DEX real**.
Relevante aquí: el APK base pesa 44 MB y las apps de Orange/MASORANGE suelen
cargar dinámicamente el módulo de consumo. **Herramienta**: `obfuscation_findings`
→ `kind = "dynamic-loader"` con `constant` = nombre del fichero cargado.

### Fase 6 — Reimplementación

Sólo cuando las fases 0-5 dicen **qué** hacer y **cómo** se llama. Renombrado por
**rol** (`ROLE_RULES` del playbook §6), nunca por adivinanza.

## B.3 Instrumentación viva: cómo capturar el JSON de consumo

### Qué hay disponible (VERIFICADO en el repo)

| Pieza | Ruta | Qué hace |
|---|---|---|
| Puente de hooks | `rust-core/src/hooks_bridge.rs` | Tool `inject_hooks`; acciones `estado\|instalar\|inyectar\|leer`; inyección ptrace (`crates/injector-payload`) |
| Payload | `crates/hooks-payload/src/hooks.rs` | `frida-gum` 0.17.2, `dlopen` en el proceso objetivo |
| Plan por dirección | `crates/hooks-payload/src/plan.rs` | Hooks por **dirección ELF** (binarios stripped); texto plano `RUTA<TAB>VA_HEX<TAB>ID<TAB>TAREA` |
| Sink | `crates/hooks-payload/src/sink.rs` | NDJSON (una línea JSON por evento) con `ts`, `pid`, `tid` |
| Redacción | `crates/hooks-payload/src/redact.rs` (`agyshim::redactar`) | **Las credenciales se redactan antes de escribir** ⇒ el NDJSON es seguro de subir |
| Confianza de CA | `rust-core/src/cert_android.rs:59-68` | Fuentes: `/data/misc/keychain`, `/data/misc/keychain/cacerts-added`, `/system/etc/security/cacerts`, `/data/misc/keystore/user_0` |

**Símbolos que el payload YA hookea** (`hooks.rs:87-172`): `open`, `openat`,
`unlink`, `connect`, `sendto`, `getaddrinfo`, `sqlite3_open_v2`,
`sqlite3_prepare_v2`, `sqlite3_step`, `sqlite3_finalize`.

### Tres rutas, ordenadas por riesgo

#### Ruta 1 — Caché local de la app (riesgo mínimo, la primera)

La app cachea el consumo en su directorio de datos. Medido:
`/data/user/0/com.orange.bluu.jazztel/no_backup` = **20.854 KB (20,4 MiB)**
(`docs/PHONE_INVENTORY.md:409`).

⚠️ **Corrección de unidad**: el enunciado decía "20.854 B"; lo medido son
**20.854 KB** (punto de millar español) = **20,4 MiB**. Tres órdenes de magnitud.

Sin MITM, sin root en la app, sin red:

```bash
su -mm -c 'ls -la /data/user/0/com.orange.bluu.jazztel/no_backup/' 2>&1 | head -30
su -mm -c 'ls -la /data/user/0/com.orange.bluu.jazztel/databases/' 2>&1
su -mm -c 'ls -la /data/user/0/com.orange.bluu.jazztel/shared_prefs/' 2>&1
su -mm -c 'ls -la /data/user/0/com.orange.bluu.jazztel/files/' 2>&1 | head -20
```

El payload **ya hookea `sqlite3_*`**: si la app abre su DB de consumo, el NDJSON
registra `sqlite3_prepare_v2` con el SQL ⇒ **el esquema real, sin adivinar**. Y
el hook de `open`/`openat` registra **qué ficheros toca** ⇒ se identifica el
fichero de caché JSON.
**Ventaja**: cero riesgo legal (leer el almacenamiento de la propia app con root
ya presente), cero MITM, cero detección de pinning.

#### Ruta 2 — TLS (Conscrypt) para ver el JSON en tránsito

Android usa **Conscrypt**: `/apex/com.android.conscrypt/lib64/libssl.so` y
`libcrypto.so`.

| Símbolo a sondear | Módulo | Qué se obtiene |
|---|---|---|
| `SSL_read` | `libssl.so` (Conscrypt) | **El cuerpo de la respuesta HTTP ya descifrado** ⇒ aquí está el JSON de consumo |
| `SSL_write` | `libssl.so` | El cuerpo de la petición (con el `Authorization`) ⇒ **redactado por `redact.rs`** |
| `SSL_get_peer_certificate`, `SSL_get_verify_result` | `libssl.so` | Detecta pinning (fallo de validación) |
| `recv`, `read`, `pread` | `libc.so` | Sólo si TLS no está en el proceso |
| `connect`, `getaddrinfo` | `libc.so` (**ya hookeados**) | **Destino y puerto de la API de consumo** |

⚠️ El payload actual **no** hookea `SSL_read`/`SSL_write` ⇒ hay que añadirlos
(`adjunta(..., "SSL_read", &["libssl.so"], …)`; `localiza()` ya busca en módulos y
después en globales).

**El dato barato que ya se tiene hoy**: con `connect` + `getaddrinfo`
(instrumentados) se obtiene el **host y el puerto** de todas las peticiones ⇒
**el host de la API de consumo, sin tocar TLS**:

```bash
# 1) inyectar el payload en la app Jazztel (tool inject_hooks, action=inyectar)
# 2) abrir la app y navegar a "Mi consumo"
# 3) leer el NDJSON
su -mm -c 'grep -a getaddrinfo /data/local/tmp/megadaemon/eventos.ndjson | tail -30'
su -mm -c 'grep -a "\"event\":\"connect\"" /data/local/tmp/megadaemon/eventos.ndjson | tail -30'
su -mm -c 'grep -a "\"hook\":\"ficheros\"" /data/local/tmp/megadaemon/eventos.ndjson | tail -40'
```

⇒ **Plan de captura: (a) `getaddrinfo` para el host, (b) `open`/`sqlite3_*` para
la caché, (c) `SSL_read` sólo si (a) y (b) no bastan.**

#### Ruta 3 — MITM con CA propia (la más potente, la más delicada)

El repo tiene `deploy/magisk-mitm-sqlite/` (bind-mount de la DB de Antigravity) y
`cert_android.rs` (import de CA al vault), pero **no** un MITM de red genérico.
Haría falta: proxy local (el repo ya tiene `web_bridge.py` y `sqlite_mitm`),
importar la CA del proxy en `/data/misc/keychain/cacerts-added` o
`/system/etc/security/cacerts` (Android exige el hash de nombre:
`openssl x509 -hash -subject_hash_old`), y **sortear el pinning** si lo hay
(detectable con `SSL_get_verify_result` y con
`apk_inventory` → `manifest_axml.application.network_security_config`).
**Riesgo**: interceptar tu propio tráfico para consultar tu propio consumo **no
es un problema penal**, pero sí puede activar **detección de root/MITM** en la
app ⇒ por eso las rutas 1 y 2 evitan esa categoría.

### Niveles de sondeo: lista cerrada

**Nivel Java** (requiere frida-java-bridge, que hoy **NO** está en el payload):
`okhttp3.OkHttpClient`, `okhttp3.internal.http.RealCall`, `retrofit2.Retrofit`,
`okhttp3.ResponseBody.string()/bytes()`, `com.google.gson.Gson.fromJson`,
`org.json.JSONObject.get*`, `android.database.sqlite.SQLiteDatabase.query`,
`SQLiteOpenHelper`, y la clase de sesión/token de `com.orange.bluu.*`
(**NO VERIFICADO**).

**Nivel nativo** (compatible hoy): `SSL_read`, `SSL_write`, `connect`,
`getaddrinfo`, `open`, `openat`, `read`, `recv`, `sqlite3_open_v2`,
`sqlite3_prepare_v2`, `sqlite3_step`.

⚠️ El payload es **frida-gum puro, sin java-bridge**
(`crates/hooks-payload/README.md`) ⇒ añadir java-bridge exige otro crate. La
**Ruta 2 (TLS en libc) es la compatible con la infraestructura existente**.

### Cómo se sabe qué clase toca el string de facturación

1. `dex_decompile::dex_strings(dex)` → lista de strings.
2. `grep` de los markers de §B.1.
3. Para cada hit, resolver el índice del string en el pool ⇒ obtener
   `clase.método` de la instrucción `const-string` que lo referencia.
4. `decompile_class(dex, idx)` ⇒ el Java de esa clase: se ve si formatea una URL,
   si llama a un `ViewModel`, etc.
5. Si el string está **cifrado** (fase 2), el hit aparece en el **descifrador**,
   no en la clase consumidora ⇒ se sigue el `sput-object`/la llamada al
   descifrador para llegar a ella.

## B.4 Diseño de las funciones del daemon

Todas en `rust-core/src/gasto_datos.rs`. **Diseño, no código.**

```rust
pub struct GastoSnapshot {
    pub ts: i64,                    // epoch ms
    pub origen: Origen,             // JazztelApi | JazztelCache | JazztelFactura | Sistema
    pub ciclo: Option<Ciclo>,       // inicio/fin del ciclo de facturación
    pub total_bytes: Option<u64>,           // None si no se sabe la cuota
    pub consumidos_bytes: Option<u64>,
    pub por_categoria: Vec<Bote>,
    pub confianza: Confianza,       // Alta | Media | Baja
}
```

### `gasto_datos::estado()` → MB por categoría

* **Salida**: `{ok, origen, confianza, ts, ciclo{etiqueta,inicio,fin},
  total_mb, total_bytes, cuota_mb, consumidos_mb, por_categoria[], notas[]}`.
* **Categorías**: `movil`, `wifi`, `ethernet`, `vpn`, `movil_facturado`, `bono`,
  `acumulado`, `compartido`.
* **Unidades**: siempre MB decimal (1.000.000 B) **y** el byte exacto, porque
  "MB" vs "MiB" es la fuente clásica de discusión con una factura.
* **Puras y testeables**: `clasificar_transporte(caps) -> Categoria`,
  `bytes_a_mb(bytes) -> f64` (con redondeo explícito), `desglose_por_app(filas)`.

### `gasto_datos::comparar_con_factura()` → operador vs sistema

```jsonc
{ "ok": true,
  "factura":  {"ciclo":"2026-03-01..2026-03-31","mb_facturados":1180.0,"fuente":"factura.pdf"},
  "sistema":   {"mb_moviles":1100.0,"fuente":"networkstats"},
  "delta_mb": 80.0, "delta_pct": 7.1, "delta_explicado": true,
  "explicaciones": [{"tipo":"tethering","mb":60.0,"confianza":"media"},
                    {"tipo":"roaming","mb":20.0,"confianza":"alta"}] }
```

* **Regla de honestidad**: si el delta no se puede explicar ⇒ `explicaciones: []`
  y `delta_explicado: false`. **No se reparte la diferencia a ojo.**
* **Puras y testeables**: `parse_factura(texto_pdf) -> FacturaMin` (en el PC con
  `pypdf`; en el móvil, con parser propio o dejando el parseo al PC),
  `explicar_delta(factura, sistema, contexto) -> Vec<Explicacion>`.

### `gasto_datos::avisos_factura()` → umbrales y fecha de renovación

```jsonc
{ "ok": true, "avisos": [
  {"nivel":"info","codigo":"umbral_80","titulo":"Has usado el 80 % de tus datos",
   "mb_restantes":236.0,"mb_totales":1180.0,"pct":80.0,"dedup_key":"umbral_80:2026-03"},
  {"nivel":"aviso","codigo":"renovacion_proxima","titulo":"Renueva en 3 días",
   "dias_para_renovar":3,"fecha_renovacion":"2026-03-28"} ] }
```

* **Integración**: el daemon ya tiene `rust-core/src/alerts.rs` con
  `raise(rule, severity, title, body, dedup_key)` y `list(min_severity, …)` ⇒
  `avisos_factura()` **no crea un sistema de avisos nuevo**: emite con
  `alerts::raise` y `dedup_key` estable por ciclo (`umbral_80:<ciclo>`).
* **Umbrales por defecto** (configurables): 50 %, 80 %, 95 %, 100 % y "quedan
  < 500 MB". El de 100 % es crítico porque Jazztel **activa un bono automático**
  al agotar la cuota (verificado: Bono 100, 2 € por 100 MB —
  <https://www.jazztel.com/cliente/lineas-adicionales>).
* **Puras y testeables**: `evaluar_umbrales(cuota, consumidos, umbrales) -> Vec<Aviso>`,
  `dias_hasta(fecha, hoy) -> i64`.

### `gasto_datos::desglose_wifi_vs_movil()`

```jsonc
{ "ok": true,
  "wifi":  {"mb":134.5,"interfaces":["wlan0"],"apps":[{"paquete":"com.x","mb":80.2}]},
  "movil": {"mb":1100.0,"interfaces":["rmnet_data0"],"apps":[{"paquete":"com.y","mb":900.1}],
            "incluye_tethering": false},
  "vpn":   {"mb":12.0,"bajo":"wifi"},
  "advertencia":"el tráfico VPN no se atribuye a un solo transporte: se reporta con el subyacente" }
```

**Reglas**: (1) usar `networkType` **y** `hasTransport()` **y**
`NET_CAPABILITY_NOT_METERED`; (2) una red con `TRANSPORT_VPN` se reporta
**anidada** bajo su transporte subyacente (`bajo: "wifi"`), nunca plana;
(3) `tethering_activo` se lee de `dumpsys tethering` / interfaces `ap*`; si está
activo, la categoría `movil` incluye tráfico de hotspot que **no es de ninguna
app** ⇒ se marca `incluye_tethering: true`.
**Puras y testeables**: `clasificar_red(transporte, metered, vpn) -> Categoria`,
`agregar_por_app(filas) -> Vec<AppBote>`.

---

# PARTE C — Interfaz con MegaDaemon

## C.1 Dónde se registra el comando

Hay **dos** despachos de herramientas y hay que tocar los dos:

| Punto | Fichero | Para qué |
|---|---|---|
| Tools remotas (HTTP) | `rust-core/src/mcp_ext.rs` → `dispatch(name, args) -> Option<Value>` | `POST /api/tool/gasto_datos` (`cockpit_server.rs:2632`) |
| Tools de dispositivo (MCP del móvil) | `rust-core/src/android.rs` → `dispatch(name, args) -> Value` + `tool_definitions()` | Tool `gasto_datos` del MCP del móvil |
| Tool MCP declarada | `rust-core/src/mcp_server.rs` (`tools/list`) | Que el MCP del móvil la anuncie |

**Estilo a imitar**: `sqlite_mitm` (`rust-core/src/sqlite_mitm.rs`), el módulo más
parecido en forma (estado propio + acciones + `estado.json` + SQLite con
`rusqlite`).

```rust
// mcp_ext.rs, dentro del match de dispatch()
"gasto_datos" => {
    let r = crate::gasto_datos::dispatch(&args);
    return Some(json!({
        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&r).unwrap_or_default() }],
        "isError": !r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false)
    }));
}
```

## C.2 La API del comando

**Identificador**: `gasto_datos`

| Acción | Qué hace | Muta estado | Irreversible |
|---|---|---|---|
| `estado` | Snapshot actual: MB por categoría + total + origen | No | No |
| `datos_moviles` | Sólo la categoría móvil, con desglose por app | No | No |
| `datos_wifi` | Sólo la categoría WiFi, con desglose por app | No | No |
| `comparar` | `comparar_con_factura()`: operador vs sistema, con diferencias explicadas | No | No |
| `avisos` | `avisos_factura()`: umbrales, MB restantes, fecha de renovación | **Sí** (emite alertas) | No |
| `refrescar` | Vuelve a leer la fuente (caché / API) y persiste | **Sí** | **Sí** (puede hacer login contra el portal ⇒ toca credenciales) |

**Parámetros**:

```jsonc
{ "action": "estado",              // enum: estado|datos_moviles|datos_wifi|comparar|avisos|refrescar
  "linea": "…",                    // opcional: alias de línea (enmascarado)
  "ciclo": "2026-03",              // opcional: ciclo de facturación a consultar
  "umbrales": [50, 80, 95, 100],   // opcional: sólo para "avisos"
  "confirm": false,                // obligatorio true para "refrescar"
  "max_chars": 1024 }              // opcional: recorte de salida
```

**Ejemplo de respuesta** (`POST /api/tool/gasto_datos` con `{"action":"estado"}`):

```jsonc
{ "ok": true, "origen": "jazztel_cache", "confianza": "media", "ts": 1793179200000,
  "ciclo": {"inicio":1790832000000,"fin":1793510400000,"etiqueta":"2026-03"},
  "total_mb": 1180.0, "total_bytes": 1180000000, "cuota_mb": null, "consumidos_mb": 1180.0,
  "por_categoria": [
    {"categoria":"movil","mb":1100.0,"origen":"sistema"},
    {"categoria":"wifi","mb":134.5,"origen":"sistema"},
    {"categoria":"bono","mb":0.0,"origen":"jazztel_cache"}],
  "notas": ["el sistema mide bytes de proceso; el operador factura bytes de red: la diferencia se explica en 'comparar'"] }
```

## C.3 Autenticación y `confirm`

### Doble auth

El gate ya existe: `rust-core/src/api_key.rs` → `decide(...)` acepta
`X-API-Key` **o** `Authorization: Bearer`, y exige **además**
`X-Seere-Auth-Token` (constantes en `rust-core/src/antigravity_intel.rs:77-79`:
`AUTH_HEADER = "X-Seere-Auth-Token"`, `API_KEY_HEADER = "X-API-Key"`).

* **No** se abre ruta nueva: `gasto_datos` entra por `POST /api/tool/…`, que
  **ya está detrás del gate** (`cockpit_server.rs:2632`); las rutas públicas
  están listadas explícitamente en `cockpit_server.rs:268` y `:1234`.
* `gasto_datos` **no** se añade a la lista de páginas públicas.
* En `mcp_server.rs` la tool se declara con el formato de las demás
  (`inputSchema` con `action` como **enum**; el test de `android.rs:1438` exige
  enum en todo parámetro llamado `action`).

### `confirm: true` para lo irreversible

`refrescar` es la única acción que (1) puede necesitar autenticarse contra el
portal de Jazztel, (2) puede escribir en el almacén local de credenciales y
(3) modifica el estado persistido ⇒ **exige `confirm: true`**. Precedentes del
repo: `android.rs:1104` (`root_exec_tool` → `needs_confirm` si
`is_destructive(cmd)`) y `cockpit_server.rs` (`/api/v2/admin/restart` exige
`confirm:true`).

```jsonc
{ "ok": false, "error": "needs_confirm",
  "hint": "la acción 'refrescar' toca el portal de Jazztel: repítela con confirm=true" }
```

## C.4 Persistencia y secretos

### Datos (no secretos)

State dir: `crate::state_dir::state_dir()` ⇒ `$MEGADAEMON_STATE_DIR` o
`/data/user/0/com.agy/files/megadaemon/state` (`rust-core/src/state_dir.rs:17`).

| Artefacto | Ruta | Formato |
|---|---|---|
| Base de datos | `<state>/gasto_datos.sqlite` | **SQLite con `rusqlite`** (dependencia ya presente: `rusqlite = "0.32"`, `bundled` — `rust-core/Cargo.toml:33`) |
| Estado | `<state>/gasto_datos/estado.json` | JSON (patrón de `sqlite_mitm::ruta_estado_mitm()`) |
| NDJSON capturado | `/data/local/tmp/megadaemon/eventos.ndjson` | Ya lo usa `hooks-payload` |

```sql
CREATE TABLE IF NOT EXISTS muestras (
  id INTEGER PRIMARY KEY, ts INTEGER NOT NULL,   -- epoch ms
  uid INTEGER, paquete TEXT,
  iface TEXT,                 -- rmnet_data0, wlan0…
  categoria TEXT NOT NULL,    -- movil|wifi|ethernet|vpn|tethering
  bytes_rx INTEGER NOT NULL DEFAULT 0, bytes_tx INTEGER NOT NULL DEFAULT 0,
  tag INTEGER, state INTEGER,
  metered INTEGER NOT NULL DEFAULT 0, roaming INTEGER NOT NULL DEFAULT 0,
  origen TEXT NOT NULL);      -- sistema|jazztel_cache|jazztel_api|factura
CREATE INDEX IF NOT EXISTS ix_muestras_ts  ON muestras(ts);
CREATE INDEX IF NOT EXISTS ix_muestras_pkg ON muestras(paquete, ts);

CREATE TABLE IF NOT EXISTS ciclos (
  etiqueta TEXT PRIMARY KEY,   -- "2026-03"
  operador TEXT NOT NULL,      -- JAZZTEL
  mcc_mnc TEXT,                -- 21403
  linea_alias TEXT,            -- alias, NUNCA el número
  inicio INTEGER NOT NULL, fin INTEGER NOT NULL,
  cuota_bytes INTEGER,         -- NULL si no se conoce
  consumidos_bytes INTEGER, fuente TEXT NOT NULL);

CREATE TABLE IF NOT EXISTS avisos (
  id TEXT PRIMARY KEY, regla TEXT NOT NULL, ciclo TEXT NOT NULL,
  nivel TEXT NOT NULL, titulo TEXT NOT NULL, cuerpo TEXT,
  creado INTEGER NOT NULL, visto INTEGER NOT NULL DEFAULT 0);
CREATE UNIQUE INDEX IF NOT EXISTS ix_avisos_dedup ON avisos(ciclo, regla);
```

### Credenciales de Jazztel — **nunca en el repo**

El repo es **público** (`boolforge/megadaemon`). Reglas:

| # | Regla |
|---|---|
| 1 | **El fichero de credenciales NO está en el repo.** Vive en el móvil, en el state dir, con **modo 0600**: `<state>/gasto_datos/jazztel_creds.json` |
| 2 | **El repo lo ignora.** `.gitignore` ya excluye `md-secrets/`, `state/*.key`, `state/*.keys`, `state/*.url`, `state/providers.json`; hay que **añadir** (cambio del root, fuera de este encargo): `state/gasto_datos.sqlite`, `state/gasto_datos.sqlite-*`, `state/gasto_datos/`. ⚠️ `state/` **no** está excluido en bloque ⇒ sin esas líneas, un `state/gasto_datos.sqlite` con la línea y el consumo **acaba en GitHub** |
| 3 | **Se escribe con `chmod 0600`** y se comprueba con `stat -c %a` antes de usarlo; si los permisos no son 0600 ⇒ no se lee y se devuelve `error: "credenciales_permisos_incorrectos"` |
| 4 | **Contenido**: `usuario` (DNI), `clave` y opcionalmente un token de sesión, **cifrado en reposo** con la clave del dispositivo. El crate ya tiene `argon2.rs` (vectores en `docs/argon2id_vectors.json`) y `backup_crypto.rs`; el vault (`signing_vault`) es el precedente. Reutilizar, no reimplementar |
| 5 | **Nunca se imprime el token.** Precedente: `tools/drive_fetch.py` imprime `token_sha16` (sha256 del token, 16 hex) y **nunca** el token |
| 6 | **El NDJSON de hooks va redactado** (`redact.rs`) ⇒ se puede subir a Drive sin filtrar el `Authorization` del portal |
| 7 | **El DNI y el número de línea no se escriben en logs ni en el JSON público.** El daemon expone `linea_alias` ("linea-1") y `dni_sha16` para correlacionar sin exponer |
| 8 | **Las facturas de Drive tienen el NIF en el nombre** (§B.0) ⇒ se referencian por **hash del contenido**, no por el nombre |

Cifrado en reposo (diseño): `clave_maestra = Argon2id(passphrase, salt_en_state_dir)`;
fichero = JSON `{"v":1,"alg":"xchacha20poly1305","nonce":b64,"ct":b64}`.

## C.5 Verificación

### Tests unitarios puros (sin móvil, sin red) — `#[cfg(test)]` en `gasto_datos.rs`

| Test | Qué fija |
|---|---|
| `mb_y_mib_no_se_confunden` | 1.000.000 B = 1 MB (decimal); 1.048.576 B = 1 MiB. **El fallo clásico** |
| `clasificar_red_mete_vpn_bajo_su_transporte` | `TRANSPORT_VPN`+`TRANSPORT_WIFI` ⇒ categoría `wifi`, `bajo: "wifi"` |
| `la_red_no_medrada_no_cuenta_como_facturable` | `NET_CAPABILITY_NOT_METERED` ⇒ `facturable=false` |
| `evaluar_umbrales_no_repite_el_aviso` | 80 % y 95 % ⇒ 2 avisos; reevaluar con los mismos números ⇒ 0 nuevos |
| `el_aviso_de_100_por_ciento_es_critico` | Al agotar la cuota (bono automático) ⇒ severidad crítica |
| `dias_hasta_la_renovacion` | Frontera de mes, año bisiesto, fecha ya pasada ⇒ 0 |
| `el_delta_sin_explicar_no_se_reparte` | 80 MB sin causa ⇒ `explicaciones: []`, `delta_explicado: false` |
| `el_tethering_explica_la_diferencia` | Con tethering activo ⇒ aparece la explicación `tethering` |
| `el_snapshot_distingue_sistema_de_operador` | `origen` y `confianza` viajan en cada fila |
| `parseo_de_factura_tolera_formatos` | Factura con "1.180,00 MB" (punto de millar español) ⇒ 1180.0 |
| `el_estado_se_aisla_por_test` | `state_dir_de_test()` (ya resuelto en `state_dir.rs`: un directorio por hilo) |
| `no_se_persisten_credenciales` | Tras `guardar_credenciales`, el fichero existe con modo 0600 y su contenido no aparece en ningún JSON de salida |

**CI**: los tests corren **en paralelo en el mismo proceso**
(`state_dir.rs:52-58`) ⇒ el estado de cada test va a un temporal por hilo (ya
resuelto con `state_dir_de_test()`).

### Mediciones en el móvil (`NO VERIFICADO` hasta que se hagan)

| ID | Medición | Cómo | Estado |
|---|---|---|---|
| M1 | `NetworkStatsManager` devuelve filas con `metered`/`roaming` correctos | `dumpsys netstats detail` + lectura del módulo | `NO VERIFICADO` |
| M2 | El desglose por app cuadra con Ajustes → Uso de datos | Comparar `datos_moviles` con la pantalla | `NO VERIFICADO` |
| M3 | La caché de la app Jazztel contiene el consumo en formato legible | `ls` de `no_backup/`, `databases/`, `shared_prefs/` | `NO VERIFICADO` |
| M4 | Los hooks `connect`/`getaddrinfo` dan el host de la API de consumo | `inject_hooks` → `inyectar` → "Mi consumo" → leer NDJSON | `NO VERIFICADO` |
| M5 | `SSL_read` se puede hookear en `libssl.so` (Conscrypt) | Añadir el hook y ver si el informe dice `instalado` | `NO VERIFICADO` |
| M6 | El total del sistema vs el de la factura: la diferencia se explica | `comparar` con una factura real | `NO VERIFICADO` |
| M7 | El sistema cuenta o no el tráfico de tethering | Hotspot on/off + comparar | `NO VERIFICADO` |
| M8 | El ciclo de renovación de la app == el de la factura | Comparar fechas | `NO VERIFICADO` |

**Regla del repo, aplicada**: una medición no hecha se escribe `NO VERIFICADO` con
el comando exacto que la produciría. **Nunca** "probablemente" ni "debería".

---

# PARTE D — Estado de los hechos

## D.1 (a) VERIFICADO — leído de ficheros del workspace o de fuentes abiertas

| # | Hecho | Evidencia exacta |
|---|---|---|
| V1 | Operador de la SIM **JAZZTEL**, MCC/MNC **21403** | `rust-core/src/call_recorder.rs:1609` (`SIM_OPERATOR_ALPHA`), `:1610` (`SIM_OPERATOR_NUMERIC`) |
| V2 | La app Jazztel en el móvil es **`com.orange.bluu.jazztel`** | `docs/PHONE_INVENTORY.md:409` |
| V3 | `no_backup` de la app mide **20.854 KB = 20,4 MiB** | `docs/PHONE_INVENTORY.md:409`; `du -sk` crudo en la bitácora de sesión (`20854 KB 20.4 MiB`) |
| V4 | El APK de Jazztel **existe en Drive**: 44.050.916 B, `md5 de06e522fcae656d6f259a782ee7e710`, id `1iqPps2LWutrcSLWeQVP2dAkX7nsZsoCB`, 2026-11-26 | Listado OAuth de Drive (2026-11-26), credencial `/workspace/md-secrets/gdrive_credentials.json` |
| V5 | Hay **3 versiones distintas** del APK en Drive (11-22 / 11-24 / 11-26), distinguibles por `md5` | idem (`c16699fc…`, `de06e522…`, `de06e522…`) |
| V6 | La app existe en Play con ese **package name**; desarrollador **MASORANGE**; última versión publicada **26.30.5** (28-10-2026), XAPK 22,47 MB, `sha256 6a684f9a…` | <https://play.google.com/store/apps/details?id=com.orange.bluu.jazztel> · <https://jazztel.en.uptodown.com/android> |
| V7 | La app tiene sección **"Mi consumo"** con megas consumidos y restantes | <https://www.lavanguardia.com/andro4all/operadoras/como-ver-el-consumo-en-jazztel-y-saber-cuantos-gigas-te-quedan> · <https://www.adslzone.net/operadores/jazztel/consultar-consumo-jazztel/> |
| V8 | Hay **3 vías** oficiales: área de clientes (DNI + contraseña), app y el **código USSD `*167#`** (SMS con megas/minutos/SMS restantes) | <https://ayudaleyprotecciondatos.es/datos-moviles/consultar/> · <https://roams.es/companias-telefonicas/jazztel/guias/clientes/consultar-consumo/> |
| V9 | Jazztel **activa un bono automático** al agotar la cuota: **Bono 100** (2 € por 100 MB), o la velocidad baja a 16 kbps si no está contratado | <https://www.jazztel.com/cliente/lineas-adicionales> · <https://roams.es/companias-telefonicas/jazztel/guias/clientes/consultar-consumo/> |
| V10 | Hay **9 facturas PDF** de Jazztel en Drive, patrón `<NIF>_J<factura>.pdf` (59–130 KB) | Carpeta `Facturas jazztel` (id `12KIiM3NqD6j1RypMS6Fbgl9gwCln3z0R`) |
| V11 | AOSP legacy: contador en `/proc/net/xt_qtaguid/stats` y `/proc/net/xt_qtaguid/iface_stat_fmt`; control en `/proc/net/xt_qtaguid/ctrl` | <https://android.googlesource.com/platform/frameworks/base/+/981962e/core/jni/android_net_TrafficStats.cpp> · <https://source.android.google.cn/docs/core/data/kernel-overview> |
| V12 | Desde **Android 9** las apps **no** pueden leer `/proc/net/xt_qtaguid`; las APIs públicas siguen funcionando | <https://developer.android.com/about/versions/pie/android-9.0-changes-all> |
| V13 | Android 9+/P+ usa **eBPF + cgroup 2** con los mapas `traffic_tag_stats_map`, `traffic_uid_stats_map`, `traffic_iface_stats_map`; diagnóstico con `dumpsys netd trafficcontroller` | <https://source.android.google.cn/docs/core/data/ebpf-traffic-monitor> |
| V14 | `NET_CAPABILITY_NOT_METERED` (y no el transporte) es lo que dice si la red **cuesta dinero** | <https://developer.android.com/reference/android/net/NetworkCapabilities.html> · <https://developer.android.com/develop/connectivity/network-ops/reading-network-state> |
| V15 | La red de hotspot lleva `NET_CAPABILITY_LOCAL_NETWORK` y, con upstream celular, `NET_CAPABILITY_DUN` | <https://developer.android.com/reference/android/net/NetworkCapabilities.html> |
| V16 | Samsung tiene pantalla **Conexiones → Uso de datos** con desglose por app, ciclo y aviso | <https://www.samsung.com/us/support/answer/ANS10002914/> · <https://www.att.com/device-support/article/wireless/KM1426213/Samsung/SamsungSMG998U> |
| V17 | **La propia Samsung** califica su contador como **"una estimación"** del ciclo mensual | <https://www.samsung.com/uk/support/mobile-devices/how-do-i-check-data-usage-on-my-samsung-galaxy-device/> |
| V18 | `com.android.providers.telephony` es el TelephonyProvider de AOSP (SMS/MMS + APNs); **no** guarda estadísticas de tráfico | <https://android.googlesource.com/platform/packages/providers/TelephonyProvider/> · <https://sysapps.bayton.org/packages/com.android.providers.telephony> |
| V19 | El payload hookea `open`, `openat`, `unlink`, `connect`, `sendto`, `getaddrinfo`, `sqlite3_open_v2`, `sqlite3_prepare_v2`, `sqlite3_step`, `sqlite3_finalize` | `crates/hooks-payload/src/hooks.rs:87-172` |
| V20 | El payload usa **frida-gum puro, sin java-bridge** | `crates/hooks-payload/README.md` |
| V21 | El payload escribe **NDJSON** con credenciales **redactadas** | `crates/hooks-payload/src/sink.rs` · `src/redact.rs` |
| V22 | El daemon ya tiene SQLite (`rusqlite 0.32`), alertas con `dedup_key`, gate de auth con `X-API-Key` + `X-Seere-Auth-Token`, y ruta genérica `POST /api/tool/<nombre>` | `rust-core/Cargo.toml:33` · `rust-core/src/alerts.rs:85` · `rust-core/src/api_key.rs:618` · `rust-core/src/antigravity_intel.rs:77-79` · `rust-core/src/cockpit_server.rs:2632` |
| V23 | `state/` **no** está excluido en bloque por `.gitignore` ⇒ un fichero nuevo de estado **acaba en GitHub** salvo que se añada explícitamente | `.gitignore:9-22` |
| V24 | **No hay** ningún APK/DEX de Jazztel en `/workspace` | Barrido `find /workspace -maxdepth 6 -iname "*jazztel*"\|"*bluu*"\|"*orange*"` → 0 resultados (2026-11-26) |
| V25 | `root_exec` recorta la salida; para volcados grandes hay que volcar a fichero y leer por franjas | `rust-core/src/android.rs:139` |

## D.2 (b) INFERIDO — deducción razonada, no medida

| # | Inferencia | En qué se apoya | Riesgo |
|---|---|---|---|
| I1 | La app **cachea el consumo** en `no_backup/` (20,4 MiB) en formato legible (SQLite o JSON) | Un `no_backup` de 20 MiB en una app de operador casi sólo contiene caché de sesión y de datos | **Alto** ⇒ por eso Ruta 1 es la primera a *probar*, no la primera afirmada |
| I2 | La app usa **HTTP(S) con OkHttp/Retrofit** y una API JSON propia | Patrón de las apps de Orange/MASORANGE; `no_backup` de 20 MiB | Medio |
| I3 | El package real en el móvil es `com.orange.bluu.jazztel` (no `…jazztel.app`): `.app` es el **sufijo de la copia de Drive** | Convención de nombres de `backup_formats.rs:616` | **Bajo** (V2) |
| I4 | La facturación de Jazztel va por el **APN de datos** (no el de MMS) ⇒ el tráfico de la app Jazztel **sí** cuenta en la cuota | Comportamiento estándar de APN + que la app muestra "Mi consumo" con los megas restantes | Medio |
| I5 | El corte de ciclo de Jazztel **no coincide** con el mes natural ⇒ el total del sistema y el de la factura se desvían al principio y al final de mes | "las facturas no corresponden a meses naturales" (<https://www.adslzone.net/operadores/jazztel/consultar-consumo-jazztel/>) | **Alto** ⇒ es la explicación #1 de las diferencias |
| I6 | El APK de 44 MB incluye **carga dinámica** del módulo de consumo (el DEX real no es `classes.dex`) | Tamaño muy superior a una app de UI + patrón "bluey"/Bluevia | Medio ⇒ pendiente de la fase 5 |
| I7 | Hay **pinning de TLS** en la app | App de operador con datos de facturación | Medio ⇒ por eso Ruta 1 es la primera |
| I8 | El móvil mide con **eBPF** (SM-S908B, Android 13) ⇒ no hay `/proc/net/xt_qtaguid` | V13 + `docs/PHONE_INVENTORY.md:22` | **Alto**, confirmable en 1 comando (C1). La versión exacta del kernel **NO** está medida en el repo |

## D.3 (c) NO VERIFICADO — y cómo verificarlo en el móvil

Todos son comandos de `root_exec` (tool existente). ⚠️ **Tope de 1024 caracteres de
salida** ⇒ se parte en lotes o se vuelca a fichero.

### C1 — ¿Contador legacy o eBPF?

```bash
su -mm -c 'ls -d /proc/net/xt_qtaguid 2>&1; uname -r; getprop ro.build.version.sdk'
```
*Esperado*: `No such file or directory` + kernel 5.x ⇒ **eBPF**.
⚠️ La versión exacta del kernel **no** está medida en el repo: este comando la
mide.
*Si es legacy*: el directorio con `ctrl`/`stats`/`iface_stat/`.

### C2 — Estado del `trafficController` (eBPF) — *que eBPF esté activo y el nombre de los mapas*

```bash
su -mm -c 'dumpsys netd | head -40'
su -mm -c 'dumpsys netd trafficcontroller | head -120'
```

### C3 — Agregado por UID — *bloques `uid=… iface=… tag=… state=… metered=… roaming=…`*

```bash
su -mm -c 'dumpsys netstats | head -150'
su -mm -c 'dumpsys netstats detail > /data/local/tmp/megadaemon/netstats_detail.txt'
su -mm -c 'wc -l /data/local/tmp/megadaemon/netstats_detail.txt'
```

### C4 — UID, ruta y permisos de la app Jazztel

```bash
su -mm -c 'dumpsys package com.orange.bluu.jazztel | grep -m1 userId='
su -mm -c 'pm path com.orange.bluu.jazztel'
su -mm -c 'dumpsys package com.orange.bluu.jazztel | grep -A5 "requested permissions"'
```
*Interesa*: el UID (para filtrar el contador) y la **lista real de permisos**.

### C5 — Caché local de la app (Ruta 1)

```bash
su -mm -c 'ls -la /data/user/0/com.orange.bluu.jazztel/no_backup/ 2>&1 | head -30'
su -mm -c 'ls -la /data/user/0/com.orange.bluu.jazztel/databases/ 2>&1'
su -mm -c 'ls -la /data/user/0/com.orange.bluu.jazztel/shared_prefs/ 2>&1'
su -mm -c 'ls -la /data/user/0/com.orange.bluu.jazztel/files/ 2>&1 | head -20'
```
*Si hay una `.db`*: `sqlite3 <ruta> '.tables'` y `.schema` (o `strings` del
fichero si `sqlite3` no está en el móvil).

### C6 — Interceptar el tráfico (Ruta 2)

```bash
# a) inyectar el payload en el proceso de la app
#    tool inject_hooks { "action":"inyectar", "package":"com.orange.bluu.jazztel" }
# b) abrir la app y navegar a "Mi consumo"
# c) leer destinos y ficheros tocados
su -mm -c 'grep -a getaddrinfo /data/local/tmp/megadaemon/eventos.ndjson | tail -30'
su -mm -c 'grep -a "\"event\":\"connect\"" /data/local/tmp/megadaemon/eventos.ndjson | tail -30'
su -mm -c 'grep -a "\"hook\":\"ficheros\"" /data/local/tmp/megadaemon/eventos.ndjson | tail -40'
```
*Interesa*: el **host** de la API de consumo y la **ruta del fichero de caché**.

### C7 — El esquema real, sin adivinar — *el SQL de las tablas de consumo*

```bash
su -mm -c 'grep -a "\"hook\":\"sqlite\"" /data/local/tmp/megadaemon/eventos.ndjson | tail -40'
```

### C8 — Tethering

```bash
su -mm -c 'ip -br link | grep -iE "ap|wlan|usb"'
su -mm -c 'dumpsys tethering | head -40'
```

### C9 — Samsung "Uso de datos": ¿provider propio?

```bash
su -mm -c 'dumpsys package providers | grep -iE "data|usage|traffic" | head -20'
su -mm -c 'ls -la /data/user/0/com.android.providers.telephony/databases/ 2>&1'
su -mm -c 'dumpsys package com.android.providers.telephony | grep -iE "versionName|codePath" | head -5'
```
*Interesa*: si aparece un authority Samsung con tablas de `data_usage`.

### C10 — Ciclo de facturación y Data Saver

```bash
su -mm -c 'settings get global data_saver_state 2>&1'
su -mm -c 'dumpsys netpolicy | head -40'
```
*Interesa*: el **ciclo configurado en el sistema** (AOSP/Samsung, **no** el de
Jazztel) ⇒ explica diferencias.

### C11 — Formato real de la factura

```bash
# En el PC (no en el móvil), con el nombre del fichero enmascarado:
python3 - <<'PY'
from pypdf import PdfReader
r = PdfReader("<factura.pdf>")
print("\n".join(p.extract_text() or "" for p in r.pages))
PY
```
*Interesa*: **cómo escribe Jazztel los megabytes** ("1.180,00 MB", "1,18 GB",
"1180 MB") ⇒ de ahí sale el parser de `comparar_con_factura()`.

### C12 — API del portal de Jazztel (fuera del móvil)

Desde el PC, con la sesión del área de clientes, capturar **una** petición a la
API de consumo con el navegador en modo red ⇒ URL, cabeceras y forma del JSON.
⚠️ **Es la vía que más credenciales toca**: con cuidado y sin subir las cabeceras
a ningún sitio. El daemon **no** debe llamar esa API hasta que se sepa el
contrato **y** haya almacenamiento cifrado de credenciales (§C.4).

### C13 — Versión de la app instalada

```bash
su -mm -c 'dumpsys package com.orange.bluu.jazztel | grep -E "versionName|versionCode" | head -4'
```
*Interesa*: contrastar con V6 (26.30.5 publicada) ⇒ si el móvil tiene otra
versión, el análisis del APK de Drive **no** aplica tal cual.

## D.4 Correcciones y avisos

1. **Unidad del `no_backup`**: el enunciado decía "20.854 B"; lo medido son
   **20.854 KB = 20,4 MiB** (punto de millar español). El daemon debe reportar
   bytes exactos y MB decimales, y **nunca** mezclar MB con MiB.
2. **`/proc/net/xt_qtaguid_ctrl`** no es la ruta actual: es el formato
   pre-Android-4. Hoy es un **directorio** `/proc/net/xt_qtaguid/` con `ctrl`,
   `stats` e `iface_stat/`; y en Android 9+ **no existe** (V12, V13).
3. **No hay provider Samsung de consumo verificado** (V18). El diseño no depende
   de él.
4. **PII en los nombres de las facturas** (§B.0): el NIF del titular está en el
   nombre del PDF de Drive. Este repositorio es público ⇒ **el NIF no debe
   aparecer aquí ni en ningún fichero del repo**.
5. **El APK no está en el workspace** (V24) ⇒ todo lo de §B.1 (permisos,
   receivers, providers, clases con strings de facturación) queda `NO VERIFICADO`
   hasta que se ejecute el procedimiento de §B.1.
6. **El `origen` de cada número es obligatorio** en la API: sin él el usuario no
   puede saber si mira el sistema o el operador, y esa diferencia es
   precisamente el valor del producto.

---

## Referencias del repo (ficheros concretos)

Cada ruta aparece ya citada en el punto donde se usa; el resumen es para
localizarla rápido.

* **RE / APK*: `rust-core/src/dex_decompile.rs` (descompilador DEX propio:
  `parse_dex`, `analyze_obfuscation`, `decompile_class`, `dex_strings`,
  `obfuscation_findings`), `apk_inventory.rs` (escáner: manifiesto, permisos,
  endpoints, firma, native libs), `apk_deobfuscator.rs` (ofuscación y
  empaquetadores), `app_intel.rs:750` (`extract_endpoints`), `axml_parser.rs`.
* **Hooks*: `rust-core/src/hooks_bridge.rs:179` (`dispatch` de `inject_hooks`),
  `crates/hooks-payload/src/{hooks.rs,plan.rs,sink.rs,redact.rs}`,
  `crates/injector-payload/src/lib.rs` (inyector ptrace).
* **Comando y auth*: `rust-core/src/mcp_ext.rs:954` (tools remotas),
  `android.rs:1303,1123` (`dispatch` + `tool_definitions`),
  `cockpit_server.rs:2632` (ruta `/api/tool/`), `api_key.rs:473,618` (gate),
  `antigravity_intel.rs:77-79` (cabeceras), `alerts.rs:85` (`raise` con
  `dedup_key`), `state_dir.rs:17` (state dir), `cert_android.rs:59-68` (fuentes
  de CA), `magisk_module.rs` (módulos Magisk), `sqlite_mitm.rs` (modelo de
  módulo con estado + SQLite + acciones), `deploy/magisk-mitm-sqlite/`.
* **Contexto medido*: `rust-core/src/call_recorder.rs:1609-1610` (operador y
  MCC/MNC), `docs/PHONE_INVENTORY.md:409` (`no_backup`), `.gitignore:9-22`.
* **Método (no reescribir)*: `docs/APK_OBFUSCATION_PLAYBOOK.md`,
  `docs/APK_ANALYZER_SPEC.md`.
