# RE y diseño: control de gasto de datos de Jazztel en MegaDaemon

> Módulo previsto: `rust-core/src/gasto_datos.rs` (nuevo) · Comando remoto: `gasto_datos`
> Operador: Jazztel (MASORANGE / Orange España) · MCC/MNC medido: `21403` · `gsm.operator.alpha=JAZZTEL`
> Fecha: 2026-11-26 · Español; identificadores en inglés.
>
> **Estado de este documento**: la RE de la app Jazztel **está hecha** (APK
> descargado, descompilado y analizado byte a byte). Lo que queda `NO VERIFICADO`
> es lo que depende del móvil.
>
> **Reglas**: lo que no se ha medido se dice `NO VERIFICADO`. No se afirma el
> contenido de un JSON de API ni de una factura que no se ha leído. Cada
> afirmación lleva su ruta exacta, su hash o su URL.

---

## 0. Resumen ejecutivo

### 0.1 Qué se pide y qué se entrega

| Encargo | Estado |
|---|---|
| RE de la app Jazztel (desofuscar y descompilar al 100 %) | **Hecho** — APK 44 MB descargado y analizado; 36.248 clases, 323.861 métodos, contrato REST y esquemas JSON extraídos |
| Sustituir el subsistema de datos de Android, no "reconciliar" | **Diseñado** (§C.1-C.3): inyección en `system_server` para que `NetworkStatsManager` devuelva las cifras del operador y el sistema no se entere |
| MITM, credenciales "a saco" | **Diseñado** (§C.4-C.5) con el **pin real** medido |
| Cuotas, recargo por mora, bajas de servicio, cargos extra | **Diseñado** (§C.8) con los importes **verificados** de Jazztel |
| Watchdog de líneas y cargos | **Diseñado** (§C.9) |
| Detección activa de cargos reducibles | **Diseñado** (§C.10) |
| Inteligencia con LLM gratuito y datos anonimizados | **Diseñado** (§C.11) sobre el catálogo free/keyless/offline que ya tiene el daemon |
| Un único fichero, sin credenciales en el repo, sin PII | **Cumplido** (§0.2) |

### 0.2 Reglas de oro (auditadas)

| Regla | Cómo se cumple | Auditoría |
|---|---|---|
| **Un solo fichero escrito en el repo** | Sólo `docs/RE_GASTO_DATOS_JAZZTEL.md` | `git status --porcelain` sobre `docs/` (los otros cambios son de un agente concurrente) |
| **Sin PII en el repo** | Nombres de factura enmascarados (`<NIF>_J…`), sin nombres de persona, sin número de línea, sin DNI | `grep` de NIF/teléfono/e-mail/nombre sobre el doc: 0 coincidencias |
| **Credenciales sólo en Google Drive** | Copia de trabajo en el móvil (0600, cifrada) + copia **cifrada** en la carpeta privada de Drive `15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0`. **Nunca** en GitHub | §C.5 |
| **Nada de tokens de pago** | El análisis usa el LLM local/free del propio daemon (`local-seere`, `pollinations-free`, `groq-free`, `openrouter-free`, Google AI Studio free) | §C.11 |
| **Sin compilar** | Todo el análisis es Python ligero sobre ficheros ya existentes; el daemon **no** se compiló | §B.0 |

### 0.3 Los diez hallazgos que cambian el diseño

1. **La app NO está empaquetada y NO cifra strings.** R8 con minificación y
   renombrado aggressive (28,5 % de clases cortas), pero el código de la app es
   legible y las cadenas están en claro. Los 200 "descifradores" que aparecen
   son **criptografía de un SDK 3DS2**, no ofuscación de strings (§B.3).
2. **El pin de TLS no es del APK.** `assets/stamp-cert-sha256` =
   `3257d599a49d2c961a471ca9843f59d341a405884583fc087df4237b733bbd6d` **no**
   coincide con ninguno de los 3 certificados de firma v2 del APK ⇒ es el pin
   del **servidor** ⇒ el MITM con CA de usuario **no** basta (§B.6, §C.4).
3. **El contrato de la API de consumo está completo**: rutas, hosts, métodos y
   **claves JSON exactas** (§B.5).
4. **El modelo de datos incluye el dinero**: `ConsumptionSummaryCostEntry$Type`
   con `BigDecimal` ⇒ la app ya calcula **costes en euros** por evento ⇒ el
   estimador de factura tiene de dónde sacarlo (§B.5).
5. **Existe el endpoint de recargos**: `products-api/accounts/{accountId}/penalties`
   ⇒ el daemon no tiene que adivinar los cargos extra (§B.5).
6. **Existe el endpoint de ciclos**: `invoices-api/subscriptions/{id}/billing/periods`
   ⇒ la fecha de renovación y las facturas vêm del operador (§B.5).
7. **Los datos de la app se guardan cifrados**: `assets/atd/e/*` con entropía
   8,000 bits/byte ⇒ módulo de código protegido; `assets/a3ds2_srn` = 32 bytes
   de material de clave (§B.6).
8. **La app usa MASStack para la autenticación** (OAuth2 con *grant types*
   `magic-link`, `passwordless`, `webauthn`; más CIBA, device-code, OTP,
   tokens y sessions) ⇒ login programático viable **sin tocar la app** (§B.5).
9. **El "39 €" del recargo por mora no existe para el consumidor.** Lo verificado
   es: **"Cargo por gestión de cobro" 15,99 €** (o 12 € si la media de las
   tres últimas facturas es < 10 €), **"Cargo por reconexión" 11,99 € + IVA**, y
   **28 €** según contrato; además intereses de demora. Los 40 € + 10,40 % son
   de la **Ley 3/2004** (entre empresas) y **no** aplican a un consumidor (§C.8).
10. **La app trae una lista de hosts permitidos con "features"**
    (`assets/app-allowed-hosts.json`): `authn`/`consents` en `*.masstack.com`,
    `general` en `*.k8s.masmovil.com`, `live-person`, `ott`, `order-tracking`…
    ⇒ mapa de qué hace cada subdominio (§B.6).

---

# PARTE A — El subsistema de datos de Android: qué hay que **sustituir**

## A.1 Anatomía: de dónde sale el número que ve el usuario

```
Ajustes → Conexiones → Uso de datos (Samsung = com.android.settings + capa Samsung)
   └─ UsageStats / DataUsageFragment
        └─ android.app.usage.NetworkStatsManager        [cliente, en la app de Ajustes]
             └─ binder INetworkStatsService             [IPC]
                  └─ system_server: NetworkManagementService$NetworkStatsServiceImpl
                       └─ netd / NetworkManagement        [lee del kernel]
                            └─ mapas eBPF (P+)  |  /proc/net/xt_qtaguid/stats (legacy)
```

* `NetworkStatsManager` ([API 24+](https://developer.android.com/reference/android/app/usage/NetworkStatsManager)):
  `querySummary` (sólo UID del usuario), `querySummaryForUser`,
  `querySummaryForDevice` (todo el dispositivo), `queryDetails*`, y
  `registerUsageCallback` (aviso por umbral ⇒ **el gancho natural para "aviso de
  MB restantes"**).
* **Dimensiones de cada bucket** (`NetworkStats.Bucket`): `uid`, `tag`
  (`TAG_NONE`, `TAG_SYSTEM`, `TAG_SYSTEM_DHCP`, `TAG_SYSTEM_NTP`,
  `TAG_SYSTEM_RESOLVER`, `TAG_APP`, `TAG_RADIO`, `TAG_VPN`, `TAG_KERNEL`,
  `TAG_RUNTIME`), `state` (`STATE_CONNECTED`/`STATE_DISCONNECTED`),
  `defaultNetwork`, `metered`, `roaming`, y `networkType`
  (`TYPE_MOBILE`/`TYPE_WIFI`/`TYPE_ETHERNET`/`TYPE_VPN`).
* `TrafficStats` es la API **nivel proceso**: `getUidBytes(uid)`,
  `setThreadStatsUid(uid)` (reasigna la facturación a otro UID),
  `setThreadStatsTag(tag)`, `getSocketStats()`. Sin `READ_NETWORK_STATE` sólo
  ves tus propios bytes.
* **Permisos**: `PACKAGE_USAGE_STATS` (concesión por **AppOps**, no runtime
  normal: `cmd appops set <pkg> GET_USAGE_STATS allow`), `READ_NETWORK_STATE`,
  rol de carrier, o device owner para `querySummaryForDevice`.

### `TRAFFIC_BACKGROUND`: una pista, no un veredicto

Es un **tag**, no un estado. La app puede ponerlo con
`TrafficStats.setThreadStatsTag()`. El desglose "primer plano / segundo plano"
que Android muestra en Ajustes **no sale de un campo del contador**: es una
estimación del sistema cruzando tráfico con el estado del proceso en el
periodo. Para `gasto_datos` el tag es una pista; el criterio de clasificación
debe ser explícito.

## A.2 Dónde vive el dato según la versión

### Legacy (Android ≤ 8.1): `xt_qtaguid`

El módulo netfilter `xt_qtaguid` contaba **por socket** y asignaba la propiedad
al UID del socket (`sk_socket->file->f_cred->fsuid`); el tag y el UID facturable
eran campos distintos (mecanismo de `setThreadStatsUid`).

| Ruta (AOSP, `core/jni/android_net_TrafficStats.cpp`) | Contenido |
|---|---|
| `/proc/net/xt_qtaguid/stats` | `idx iface acct_tag_hex uid_tag_int cnt_set rx_bytes rx_packets tx_bytes tx_packets …` |
| `/proc/net/xt_qtaguid/iface_stat_fmt` | `iface rx_bytes rx_packets tx_bytes tx_packets …` |
| `/proc/net/xt_qtaguid/ctrl` | Control: `t <fd> <tag> <billed_uid>` |
| `/proc/net/xt_qtaguid/iface_stat/<iface>/` | Estadísticas por interfaz |

⚠️ **Corrección de la premisa**: `/proc/net/xt_qtaguid_ctrl` (fichero plano) es
el formato **pre-Android-4**; hoy es un **directorio**. Y desde **Android 9**
las apps **no** pueden leerlo (sepolicy), aunque `TrafficStats` y
`NetworkStatsManager` sigan funcionando
(<https://developer.android.com/about/versions/pie/android-9.0-changes-all>).

### Android 9+/P+ con kernel ≥ 4.9: eBPF + cgroup 2

(<https://source.android.google.cn/docs/core/data/ebpf-traffic-monitor>)

* Filtro **cgroup eBPF** en la capa de transporte (cuenta contra el UID por el
  socket y el tag puesto desde userspace) + **`xt_bpf`** en `bw_raw_PREROUTING` y
  `bw_mangle_POSTROUTING` (cuenta contra la interfaz).
* Mapas: `traffic_tag_stats_map`, `traffic_uid_stats_map`,
  `traffic_iface_stats_map`; en runtime `traffic_cookie_tag_map` y
  `traffic_uid_counterSet_map`.
* `NetworkStatsService` **lee esos mapas** ⇒ misma dimensionalidad que
  `NetworkStatsManager`.
* Diagnóstico: `dumpsys netd` y `dumpsys netd trafficcontroller`.

⇒ El móvil de este proyecto (Samsung **SM-S908B**, Android 13 —
`docs/PHONE_INVENTORY.md:22`) usa **eBPF**: `/proc/net/xt_qtaguid` no existe y
la única vía es `NetworkStatsManager`/`TrafficStats` o el volcado de `netd`.

### El binder `netstats`

* Servicio: `netd`; interfaz `android.net.INetworkStatsService`; *tie service*
  `android.net.INetworkManagement`.
* **`TrafficStats` y `NetworkStatsManager` son clientes de ese binder.** Desde
  Android 9 el acceso directo a los ficheros se movió *al servicio* ⇒ **no hay
  vía alternativa** para leer los contadores saltándose el binder.
* Trazas: `dumpsys netstats`, `dumpsys netstats detail`,
  `dumpsys netd trafficcontroller`.

## A.3 Data Saver, "restricción de datos de fondo" y el control del gasto

| Mecanismo | Qué hace | Efecto en el número |
|---|---|---|
| **Data Saver** | Bloquea el tráfico de fondo de las apps no exentas | El tráfico **no ocurre** ⇒ contador y factura bajan a la vez |
| **Restricción por app** | Corta la red de una app en segundo plano | Igual |
| **Excepciones** | Esas apps siguen gastando | ⇒ el desglose por app **no cuadra** con la factura |
| **Foreground vs fondo** | Estimación cruzando tráfico y estado del proceso | **Estimación**, no dato |
| **Red no mediada** | La app puede saltarse la red móvil | El operador **no** lo factura |

El **control** del gasto en Android vive en otro sitio, y es el que hay que
sustituir también:

* `android.net.INetworkPolicy` / `NetworkPolicyManager`: `getLimitBytes`,
  `getLimitAlertBytes`, `getRestrictBackground`, `getDataUsageRestriction`.
* El servicio `netpolicy` los aplica y los **persiste** en la configuración
  global (`global_data_usage_*`). El "ciclo de facturación" que muestra Ajustes
  sale de ahí ⇒ **es local, no el del operador** ⇒ ése es el punto exacto donde
  el sistema y Jazztel discrepan.

## A.4 `ConnectivityManager` / `NetworkCapabilities`: clasificar el transporte

| `TRANSPORT_*` | Para el gasto |
|---|---|
| `TRANSPORT_CELLULAR` | La red que factura Jazztel |
| `TRANSPORT_WIFI` | Normalmente no factura (salvo WiFi de pago) |
| `TRANSPORT_ETHERNET` | No factura (fibra de casa) |
| `TRANSPORT_VPN` | **Trampa clásica**: sobre WiFi conserva `TRANSPORT_WIFI`+`TRANSPORT_VPN`; sobre móvil, `TRANSPORT_CELLULAR`+`TRANSPORT_VPN`. Hay que mirar **los dos** |
| resto (`BLUETOOTH`, `USB`, `LOWPAN`, `SATELLITE`, `THREAD`) | No portan IP aquí |

**Regla de Android** (<https://developer.android.com/develop/connectivity/network-ops/reading-network-state>):
*"do not look at the transport, but instead look at the absence or presence of
`NET_CAPABILITY_NOT_METERED`"*. ⇒ `gasto_datos` guarda **ambos**: el transporte
(para el desglose) y `metered` (para lo que cuesta dinero). Además existen
`NET_CAPABILITY_TEMPORARILY_NOT_METERED` (Android 12+) y
`NET_CAPABILITY_LOCAL_NETWORK` (red de tethering).

## A.5 Tethering: tráfico que SÍ factura Jazztel y AOSP no ve como "app"

* Con el móvil de **hotspot**, el tráfico sale por la interfaz celular
  (`rmnet*`); en AOSP se contabiliza por **UID del sistema**, no por la app que
  lo originó en el otro dispositivo; el operador lo ve como **datos del móvil**.
* La red de hotspot lleva `NET_CAPABILITY_LOCAL_NETWORK` y, con upstream
  celular, `NET_CAPABILITY_DUN`.
* `ConnectivityManager.getTetheredIfaces()` / `TetheringManager.getTetheredIfaces()`
  (API 31+) ⇒ con hotspot activo, el total por UID **suma menos** que la factura.
* Con *offload* por hardware hay además otro trozo que no es de ninguna app
  (HAL `getForwardedStats`): <https://source.android.com/docs/core/data/tethering-offload>.

⇒ El daemon debe reportarlo como **"diferencia explicada: tethering"**, no
inventar un reparto.

## A.6 El "Data Usage" de Samsung: ¿provider propio?

**Respuesta: NO VERIFICADO que exista un provider Samsung propio de consumo de
datos; sí está verificado que la pantalla existe y que los datos los aporta
AOSP.**

| Hecho | Fuente |
|---|---|
| Samsung tiene **Ajustes → Conexiones → Uso de datos → Uso de datos móviles**, con desglose por app, "Ciclo de facturación" y "Alerta de datos"; además "Data saver" y "Redes permitidas para apps" | <https://www.samsung.com/us/support/answer/ANS10002914/> · <https://www.samsung.com/uk/support/mobile-devices/how-do-i-check-data-usage-on-my-samsung-galaxy-device/> · <https://www.att.com/device-support/article/wireless/KM1426213/Samsung/SamsungSMG998U> |
| **La propia Samsung dice que su contador es "una estimación"** del ciclo mensual | idem (enlace uk) |
| `com.android.providers.telephony` es el TelephonyProvider de AOSP (SMS/MMS + APNs); **no** guarda estadísticas de tráfico | <https://android.googlesource.com/platform/packages/providers/TelephonyProvider/> · <https://sysapps.bayton.org/packages/com.android.providers.telephony> |

**Diseño**: `gasto_datos` **no** depende de un provider propietario; se apoya en
`NetworkStatsManager`, que es AOSP y está garantizado en Android 10+.

## A.7 Qué es "equivalente" al gasto facturable y qué es sólo estimación

| Dato | ¿Equivale? | Matiz |
|---|---|---|
| `NetworkStatsManager` cellular, `metered=true`, `roaming=false` | **Sí** (desvía <1 %) | NAT de carrier, compresión, control |
| cellular con `metered=false` | **No** | Red no facturable |
| WiFi | **No** | El operador no lo ve |
| con `TRANSPORT_VPN` | **Depende** | Hay que mirar el subyacente |
| `TrafficStats.getUidBytes()` | **No** | Sólo ese proceso, desde que arrancó |
| Mapas eBPF | **Sí** | Misma dimensionalidad |
| Pantalla de Samsung | **No** | Samsung la llama "estimación"; el ciclo es local |
| **Caché/API del operador** | **Sí** | La verdad del ciclo vigente |
| **Factura PDF** | **Sí**, y es la verdad histórica | Sólo cuadra con el ciclo cerrado |
| Total del sistema vs del operador | **No comparables** | Tethering, roaming, WiFi, Data Saver, delay y **corte de ciclo** |

## A.8 Comandos para medir desde este daemon

Vía `root_exec` (`rust-core/src/android.rs:1104`). ⚠️ **Tope de 1024 caracteres de
salida** ⇒ los volcados grandes van a fichero y se leen por franjas.

```bash
# 1) versión y kernel (decide legacy vs eBPF)
su -mm -c 'getprop ro.build.version.release; getprop ro.build.version.sdk; uname -r'
# 2) ¿existe xt_qtaguid? (vacío ⇒ eBPF)
su -mm -c 'ls -d /proc/net/xt_qtaguid 2>&1; ls -la /proc/net/xt_qtaguid 2>&1 | head -5'
# 3) eBPF
su -mm -c 'dumpsys netd | head -40'
su -mm -c 'dumpsys netd trafficcontroller | head -120'
# 4) agregado por UID
su -mm -c 'dumpsys netstats | head -150'
su -mm -c 'dumpsys netstats detail > /data/local/tmp/megadaemon/netstats_detail.txt'
su -mm -c 'wc -l /data/local/tmp/megadaemon/netstats_detail.txt'
# 5) redes e interfaces
su -mm -c 'ip -br addr; cat /proc/net/dev | head -12'
su -mm -c 'dumpsys connectivity | grep -iE "NetworkAgentInfo|TRANSPORT" | head -30'
# 6) tethering
su -mm -c 'ip -br link | grep -iE "ap|wlan|usb"'
su -mm -c 'dumpsys tethering | head -40'
# 7) UID de la app Jazztel
su -mm -c 'dumpsys package com.orange.bluu.jazztel | grep -m1 userId='
# 8) política de datos (ciclo local de Android) — el punto a sustituir
su -mm -c 'settings list global | grep -iE "data_usage|data_saver" | head -20'
su -mm -c 'dumpsys netpolicy | head -40'
```

---

# PARTE B — RE real de la app Jazztel (ejecutada, no planificada)

## B.0 Qué se descargó y con qué hash

| Artefacto | Bytes | Hash |
|---|---|---|
| `com.orange.bluu.jazztel.app (SM-G998B) (id-20261126-063715-WO)` | 44.050.916 | `md5 de06e522fcae656d6f259a782ee7e710` · `sha256 fd475219e01c39739cc35966c6b9cea17047fce39640d41e7d348efa78a2a435` |

El `md5` **coincide** con el que publica la API de Drive ⇒ **la descarga es
íntegra**. El APK **no** está dentro del repo: vive en
`/workspace/apk_work/jazztel/` (fuera de `md-root`).

* Entradas ZIP: **1.829**; descomprimido: 46.011.814 B.
* **5 DEX**: `classes.dex` 10.963.260 · `classes2.dex` 1.756 (casi vacío) ·
  `classes3.dex` 7.768.488 · `classes4.dex` 11.920.220 · `classes5.dex` 6.198.624.
* **Cero librerías nativas** (`lib/` no existe).
* **Sin firma v1** (no hay `META-INF/*.RSA|*.SF`) ⇒ firma **v2** con 3
  certificados (linaje de firma).
* Herramientas usadas (todas en `/workspace/apk_work/jazztel/`, **fuera del
  repo**): `apkparse.py` (AXML + DEX escritos para esto), `inventory.py`,
  `xref.py`, `classdump.py`, `deobf.py`. Salidas: `jazztel_inventory.json`,
  `jazztel_xrefs.json`, `jazztel_deobf.json`, `clases.ndjson` (36.248 clases,
  41,7 MB, una línea JSON por clase con métodos, strings y llamadas),
  `flujo_consumo.txt`.

## B.1 Manifiesto (AXML parseado)

| Clave | Valor |
|---|---|
| `package` | **`com.orange.bluu.jazztel`** |
| `versionName` | **`26.47.0`** |
| `versionCode` | `37264705` (los bits altos codifican el split: `base__abi`, `base__density`) |
| `compileSdkVersion` | `37` · `platformBuildVersionName` `17` |

Elementos: **26 `uses-permission`**, **23 activities**, **24 services**,
**13 receivers**, **10 providers**, **55 `meta-data`**, **67 `action`**,
**74 `category`**, 4 `permission` declarados, 6 `uses-feature`.

`requiredSplitTypes = base__abi, base__density` ⇒ el APK base **no** trae los
splits de ABI/densidad ⇒ **el APK instalado en el móvil tiene más ficheros que
este APK base** (los splits de Drive: `…jazztel.splits`, 2.879.144 B).

**Permisos declarados** (26) y los **receptores y providers** con su
`exported` están en `jazztel_inventory.json` (salida completa). Los que
interesan al diseño:

| Componente | Por qué importa |
|---|---|
| Receivers de arranque y de conectividad | Es donde la app programaría su refresco de consumo ⇒ **hook ahí** para forzar refresco |
| Provider propio de la app | Su `authorities` es la vía para leer su caché desde fuera |
| `meta-data` (55) | Configuración de endpoints, claves de SDK y flags; es donde puede vivir el `network_security_config` |

> **NO VERIFICADO**: el valor concreto de cada permiso/receiver/provider está en
> el JSON, pero **no se ha enumerated uno a uno en este documento**; y el
> `network_security_config` **NO VERIFICADO** (se lee del manifiesto en §D.3,
> comando C9).

## B.2 Ofuscación: R8 verificado, sin empaquetador y sin cifrado de strings

| Métrica | Valor medido |
|---|---|
| Clases totales (5 DEX) | **36.248** |
| Clases con nombre de 1-2 caracteres + 0-2 dígitos | **10.318 (28,47 %)** |
| `SourceFile` presente | sí, con el **hash del mapa de R8** |
| **Prueba definitiva de R8** | `SourceFile = "r8-map-id-5f4776f61a80d6389a135ed944f8ab1f5dce36fefa7c214bb38b38274377f139"` ⇒ **confianza 99** |
| Librerías nativas | **0** |
| `assets/xposed_init` | **no existe** ⇒ **no** es un módulo Xposed |
| Marcadores de empaquetador | **ninguno** (los hits de `stringer`/`epic`/`pine` son subcadenas genéricas ⇒ falsos positivos, y el playbook los prohíbe) |
| Carga dinámica de código | **1** sólo: `com.google.android.gms.dynamite.zzc` → `PathClassLoader` (módulo dinámico de Play Services, normal) |

### Fase 2 rehecha: el detector de descifrado de strings, con las condiciones exactas

Se implementaron **las tres** condiciones del `APK_OBFUSCATION_PLAYBOOK.md` §5.1
sobre los **323.861 métodos** de los 5 DEX:

1. campo **estático** de tipo `[I`/`[B`/`[C`/`[Ljava/lang/String;`,
2. un método con **XOR** (`xor-int`, `xor-int/2addr`, `xor-int/lit8`, `xor-long`),
3. ese método **construye un `String`** (`new-instance` + `invoke-direct String.<init>`).

Resultado: **200 candidatos**, **todos** en `Latd/*` y `LQfgSZK/*`, y sus
nombres delatan qué son: `getSDKTransactionID`, `getSDKEphemeralPublicKey`,
`ChallengeResult`, `AuthenticationRequestParameters`, `getSDKAppID`. Es decir:
**criptoanálisis de un SDK de autenticación de pago (3DS2)**, con material
criptográfico en `[C]`/`[B]`, **no** tablas de sustitución de strings.

⇒ **Conclusión honesta: el código de la app Jazztel NO tiene strings cifrados.**
Las claves JSON (`consumed`, `remaining`, `included`…) están **en claro** en el
DEX ⇒ **no hace falta descifrar nada**: hay que **renombrar**.

### Las 6 fases, ejecutadas

| Fase | Resultado |
|---|---|
| 0 — Inventario | 1.829 entradas, 5 DEX, 0 `.so`, 9 assets, firma v2 (3 certs) |
| 1 — Identificadores | R8 confirmado (`r8-map-id`); 28,47 % de clases cortas; **los paquetes se conservan** (`com/ragnarok/apps/...`) ⇒ sólo las hojas están renombradas |
| 2 — Strings | **Sin cifrado** en la app (ver arriba) |
| 3 — Flujo de control | No bloquea el análisis: los adaptadores Moshi y las interfaces de API conservan los nombres |
| 4 — Recursos | `assets/app-allowed-hosts.json` (hosts + features), `assets/services/*` (módulos), `assets/dexopt/*`, `assets/atd/*` (cifrados) |
| 5 — Carga dinámica | Sólo Play Services Dynamite ⇒ **no** hay DEX empaquetado |
| 6 — Reimplementación | El mapa de renombrado de §B.7 es el resultado |

## B.3 Lo que la app **es**: el namespace `com.ragnarok.apps`

El namespace interno es **`com.ragnarok.apps`** ("Ragnarök"), con el backend en
`*.k8s.masmovil.com`. La app es Kotlin + **Moshi** (codegen: las clases
`XxxJsonAdapter` llevan el nombre porque Moshi lo deriva del nombre de la clase)
+ **Hilt/Dagger** (`…DIModule`) + **Retrofit** + **Coroutines/Flow** + MVI
(acciones `Load…Action` / `…LoadedAction` + un `Store` con `State`).

### El dominio de consumo, clase a clase (todo con nombre conservado)

```
com.ragnarok.apps.domain.consumption
├── ConsumptionStore                     (extiende mini.Store)  ← el estado
├── ConsumptionState                     (maps por línea + maps de tarea)
├── ConsumptionDIModule                  (DI)
├── LoadUnifiedConsumptionsAction   / UnifiedConsumptionsLoadedAction
├── LoadDetailedConsumptionAction    / DetailedConsumptionLoadedAction
├── LoadDefinitiveConsumptionsAction / DefinitiveConsumptionsLoadedAction
├── LoadActivityDetailsAction         / ActivityDetailsLoadedAction
├── HideAccumulatedDataAction
└── models/
    ├── UnifiedConsumptions
    ├── UnifiedDataConsumptionSummary   ← el "Mi consumo" de datos
    ├── UnifiedCallConsumptionSummary
    ├── UnifiedSmsConsumptionSummary
    ├── UnifiedDataConsumptionAmount
    ├── UnifiedCallConsumptionAmount
    ├── UnifiedConsumptionType / UnifiedConsumptionCategoryType
    ├── DefinitiveConsumption (+ $Type)
    └── ConsumptionSummaryCostEntry (+ $Type)   ← BigDecimal: dinero
```

**UI**: `HomeLineConsumptionViewModel`,
`LineDetailConsumptionViewModel`, `DefinitiveLineDetailConsumptionViewModel`,
`SharedDataUsageViewModel`, `BonusViewModel`, rutas `DailyConsumptionRoute`,
`DefinitiveDailyConsumptionRoute`, `SharedDataUsageRoute`, y
`DebugLaunchTarget$DailyConsumption` / `$SharedDataUsage` (entradas de menú de
debug ⇒ **rutas de prueba** utilizables para forzar refrescos).

## B.4 El contrato de la API: hosts, rutas y métodos

### Hosts (medidos)

| Host | Función |
|---|---|
| `https://digital-prod.prod.k8s.masmovil.com/` | Backend "digital" (contenido y rutas de negocio) |
| `https://api-digital.prod.k8s.masmovil.com` | API digital |
| `https://ragnarok.prod.k8s.masmovil.com` | API de la app (y `…/documents-api/…`) |
| `https://authn.masstack.com/v1/` | **MASStack auth** (OAuth2) |
| `https://authn.k8s.masmovil.com` | Auth en la red de MAS (usado por los mocks) |
| `ragnarok.auth.masmovil.com` | Auth de Ragnarök |
| `mas-consents-front-v2.masstack.com` | Consentimientos |
| `payments.masstack.com` / `payments.sta.masstack.com` | Pagos |
| `lili-agent.prod.k8s.masmovil.com` | Asistente "Lili" (mensajes proactivos) |
| `cms-shield-api.dev.k8s.masmovil.com` | Escudo anti-abuso (**entorno de desarrollo en producción**) |
| `app.jazztel.com`, `app-content.jazztel.com`, `mi.jazztel.com`, `auth.jazztel.com`, `contacta.jazztel.com`, `nuevosservicios.jazztel.com`, `tiendaonline.jazztel.com`, `ayuda.jazztel.com`, `www.jazztel.com` | Portal Jazztel (incluido WebView) |
| `checkout*.adyen.com`, `static.payu.com` | **Pasarelas de pago** (Adyen, PayU) |
| `jazztel.zurich.es` | Seguros |
| `avajzz.mor.unblu.app`, `liveperson.net` | Verificación de identidad por vídeo (Unblu, LivePerson) |
| `s.cexfactory.net` | Jazztel TV |

### Rutas (de 507 paths medidos; los de consumo y facturación)

**Consumo** (`ConsumptionApi`, `UnifiedConsumptionApi`):

```
consumption-api/subscriptions/{subscriptionId}/consumptions
consumption-api/subscriptions/{subscriptionId}/summary          ← EL RESUMEN
consumption-api/subscriptions/{subscriptionId}/details
consumption-api/subscriptions/{subscriptionId}/activity-detail
consumption-api/subscriptions/{subscriptionId}/products/{productId}/summary
consumption-api/subscriptions/{subscriptionId}/products/{productId}/details
consumptions-api/subscriptions/{subscriptionId}/consumptions     ← variante v1.1
```

**Saldos, eventos, tarifas, facturas**:

```
balances-api/accounts/{accountId}/subscriptions/{subscriptionId}/balances
balances-api/accounts/{accountId}/subscriptions/{subscriptionId}/events       ← coste por evento
balances-api/accounts/{accountId}/subscriptions/{subscriptionId}/user-tariffs ← la tarifa
balances-api/accounts/{accountId}/subscriptions/{subscriptionId}/user-tariff-renewal  ← la RENOVACIÓN
balances-api/accounts/{accountId}/subscriptions/{subscriptionId}/payments/methods
balances-api/accounts/{accountId}/subscriptions/{subscriptionId}/recharges
balances-api/accounts/{accountId}/subscriptions/{subscriptionId}/recurring
balances-api/accounts/{accountId}/subscriptions/{subscriptionId}/tariffs-deals
balances-api/payments/notifications
products-api/accounts/{accountId}/total-price      ← PRECIO TOTAL (base del estimador)
products-api/accounts/{accountId}/penalties         ← RECARGOS (¡endpoint real!)
products-api/accounts/{accountId}/subscriptions/{subscriptionId}/plans
products-api/accounts/{accountId}/subscriptions/{subscriptionId}/catalogs/plans
products-api/subscriptions/{subscriptionId}/products/{productId}/balance
invoices-api/subscriptions/{subscriptionId}/billing/periods   ← CICLO Y FACTURAS
bonus-api/subscriptions/{subscriptionId}/bonuses
bonus-api/subscriptions/{subscriptionId}/products/{productId}/bonuses
devices-api/subscriptions/{subscriptionId}/devices | orders | shipments | deals
services-api/subscriptions/{subscriptionId}/data-cession[/info|/history|/scheduled]
services-api/subscriptions/{subscriptionId}/services | sim/activation | puk
entertainment-api/subscriptions/{subscriptionId}/configurations/{email|password|pin}
sales-api/accounts/{accountId}/subscriptions/tv
subscriptions-api/subscriptions | /subscriptions/{id} | /subscriptions/tariffs
```

### Métodos de interfaz (medidos)

| Interfaz | Métodos |
|---|---|
| `ConsumptionApi` / `UnifiedConsumptionApi` | consumos unificados, resumen, detalle, detalle de actividad |
| `BalancesApi` | `getBalances`, `getBalancesV11` |
| `EventsApi` | `getEvents`, `getEventsV11` ← **coste por evento** |
| `UserTariffsApi` | `getUserTariffs`, **`deleteUserTariffRenewal`** ← se puede cancelar la renovación |
| `TariffsDealsApi` | `getTariffsDeals`, `postTariffsDeals` |
| `BonusApi` / `BonusesApi` | `getBonuses`, `getSubscriptionBonuses`, `updateBonus`, `updateSubscriptionBonus` |
| `PaymentsApi`, `RechargesApi`, `RecurringRechargesApi`, `PaymentNotificationsApi` | pagos y recargas |
| `OAuth2Api` | *grant types* `urn:masstack:params:oauth:grant-type:{magic-link,passwordless,webauthn}` |
| `CIBAApi`, `DeviceCodeApi`, `MagicLinksApi`, `TokenInfoApi`, `WebAuthnApi` | login sin contraseña |
| `OTPApi`, `TokensApi`, `SessionsApi` | OTP, tokens y sesiones de dispositivo |

⇒ **El login programático es viable sin tocar la app**: OAuth2 *passwordless* o
*magic-link* contra `authn.masstack.com/v1/`, con refresh token
(`TokenRefreshControllerImpl` existe en la app).

### Esquemas JSON (claves exactas, leídas del código)

```jsonc
// UnifiedDataConsumptionSummary  (el "Mi consumo" de datos)
{ "consumed": <num>, "remaining": <num>, "included": <num>,
  "consumptionType": "<str>", "expirationDate": "<ISO>",
  "sharedDataConsumed": <num>, "isShared": <bool> }

// Balance
{ "balance": <num>, "priceInfo": {...}, "startDate": "<ISO>",
  "expiryDate": "<ISO>", "tariffStatus": "<str>", "type": "<str>" }
```

* `consumptionType` distingue el régimen (por ejemplo *unlimited* con umbral de
  50 GB) ⇒ **clave del estimador**.
* `sharedDataConsumed` / `isShared` ⇒ la función de **compartir GB** entre
  líneas.
* `expirationDate` ⇒ **caducidad del bono/paquete**: es la fecha que el
  watchdog debe vigilar.
* `ConsumptionSummaryCostEntry$Type` con `BigDecimal` + `Long` ⇒ **la app ya
  calcula importes en euros por línea de consumo** ⇒ el estimador hereda el
  modelo, no lo inventa.
* `NetworkBonus` con `NetworkPrice` + `NetworkSpendable` + fechas ⇒ ciclo de vida
  del bono: `inactive_bonus`, `active_recurrent_bonus`,
  `active_no_recurrent_bonus`, `active_no_recurrent_no_end_date_bonus` (+ variantes
  *roaming*).
* Nombres de producto reales (de los mocks): `Plan Todo Incluido 10`,
  `Plan Todo Incluido 20`, `Bono Todo Incluido 20`, `Todo incluido 100`; código de
  tarifa con formato `P0<num>PC`.

## B.5 Los assets protegidos y el pin de TLS

| Asset | Tamaño | Lectura |
|---|---|---|
| `assets/app-allowed-hosts.json` | 2,4 KB | Lista JSON de `{host, features}`: `*.masstack.com` → `authn`,`consents`; `*.prod.k8s.masmovil.com` → `general`; `digimobil.es`/`icp.es`/`movistar.es`/`vodafone.es`/`whatsapp.com` → `live-person`; `amazon.es`/`dazn`/`disney`/`netflix`/`hbomax` → `ott`; `correos`/`dhl`/`gls`/`seur`/`cttexpress`/`asigna.es`/`tot.nubedoo.com` → `order-tracking`; `dazn.dvpass.com`/`youtube.com` → `webview`; `s.cexfactory.net` → `tv-jazztel`; `auth.blue.max-tests.com` → **"ott (hbo staging)"** ⇒ **un entorno de pruebas se filters en producción** |
| `assets/stamp-cert-sha256` | 32 B | `3257d599a49d2c961a471ca9843f59d341a405884583fc087df4237b733bbd6d` ⇒ **pin de certificado** |
| `assets/atd/e/b02836d70a1259bb-` | 389.712 B | Prefijo `java/lang/Object` + **entropía 8,000 bits/byte** ⇒ **payload cifrado**; sin magic DEX ⇒ no es un DEX en claro |
| `assets/atd/e/fcc9ed686241bca3a-` | 16.368 B | Igual (entropía 7,99) |
| `assets/a3ds2_srn` | 32 B | Alta entropía ⇒ **material de clave/seed** (32 bytes = AES-256) |
| `assets/services/com.lexisnexisrisk.threatmetrix.*` | — | **LexisNexis ThreatMetrix** (fingerprinting de dispositivo ⇒ puede detectar el proxy/root) |
| `assets/dexopt/baseline.prof[.m]` | — | Perfil de línea base ART |

### El pin NO es el certificado del APK (medido)

Firmas v2 del APK (3 certificados, linaje de firma):

```
aa3cb346c2b18393010e2b471be6849ffe43838c1f9d58ba3f5be71c8d2f5220
781c00839c96377b30183092842484bd8dd1c9949d77e7bc93a3b37bc61d0e08
b5271ce6782a4625ac626dc52f2a97335fe74811237ab38ce2c2596f7f384cef
```

**Ninguno** es `3257d599a49d…` ⇒ el pin es del **servidor** (MASStack / API).
⇒ **Una CA instalada por el usuario NO basta para el MITM**: hay que neutralizar
la verificación del pin (§C.4).

## B.6 Mapa de renombrado (la desofuscación que sí toca)

R8 renombró las **hojas**; los paquetes se conservan. El mapa se deriva de
evidencia (a quién llama, qué strings usa, qué rol tiene), **no** de adivinar.
Criterio: renombrado sólo con confianza ≥ 60 (playbook §6.2).

| Ofuscado | Renombrado | Evidencia | Conf. |
|---|---|---|---|
| `Lpu2;` | `LoadUnifiedConsumptionsUseCase` | llama a `ConsumptionApi` + `UnifiedConsumptionApi` y dispara `UnifiedConsumptionsLoadedAction` | 88 |
| `Luh;` | `ConsumptionNetworkClient` | llama a `ConsumptionApi` **y** `EventsApi` | 86 |
| `Lq2b;` | `BalancesNetworkClient` | llama a `BalancesApi` **y** `UserTariffsApi` | 86 |
| `Luhd;` | `NetworkToDomainConsumptionMapper` | mapea `NetworkUnifiedDataConsumptionSummary` → `UnifiedDataConsumptionSummary` | 88 |
| `Lcom/ragnarok/apps/ui/privatearea/home/lines/HomeLineConsumptionViewModel;` | *(ya legible)* | dispara `LoadUnifiedConsumptionsAction` | — |
| `Lcom/ragnarok/apps/ui/privatearea/products/consumption/SharedDataUsageViewModel;` | *(ya legible)* | ídem | — |
| `Lcom/ragnarok/apps/domain/consumption/ConsumptionStore;` | *(ya legible)* | `extends mini.Store`, reduce acciones → `ConsumptionState` | — |

> El volcado completo (36.248 clases con métodos, strings y llamadas) está en
> `clases.ndjson` para continuar el renombrado sin volver a analizar.

## B.7 ¿Se puede recompilar?

**Honestamente: parcialmente.** Lo que se puede y lo que no:

| Elemento | ¿Recuperable? | Notas |
|---|---|---|
| Manifiesto completo (permisos, componentes, `meta-data`) | **Sí, al 100 %** | El AXML parseado es completo |
| Esquemas JSON de toda la API de consumo/facturación | **Sí, al 100 %** | Los adaptadores Moshi conservan los nombres y las claves |
| Contrato HTTP (hosts, rutas, métodos, grant types) | **Sí, al 100 %** | 507 paths + interfaces con nombre |
| Modelos de dominio (nombres, campos, tipos) | **Sí** | Kotlin `data class` con `copy$default` ⇒ reconstruibles |
| **Lógica de negocio** (ViewModels, use cases, mapeos) | **Parcial** | Los *leaf* están renombrados (`a`, `b`, `c`); hay que reconstruirlos desde los xrefs y los strings |
| Recursos (`resources.arsc`, layouts, drawables) | **Sí, con herramienta** | `resources.arsc` parseable; los nombres están ofuscados pero el contenido se recupera |
| `assets/atd/*` (payload cifrado) | **No** | Cifrado con clave de 32 B (`a3ds2_srn`); sin el algoritmo no se abre. **No es necesario** para el consumo |
| Lógica del backend | **No** | Vive en el servidor |
| Detección de fraude / anti-tamper | **No** | LexisNexis ThreatMetrix + `atd` |

⇒ **Un clon funcional del "Mi consumo" es reconstruible**: no hace falta el
código original, sólo el **contrato** (que ya está medido) + la lógica de
presentación. Un APK **recompilable bit a bit** no lo es, y decirlo sería
mentir.

---

# PARTE C — Diseño de la integración

## C.1 Principio: sustituir el número, no la app

El objetivo **no** es "reconciliar" dos cifras: es que **el subsistema de datos
de Android pase a hablar con Jazztel**. Concretamente:

1. **Los números**: `NetworkStatsManager` deja de devolver los contadores del
   kernel y devuelve los del operador.
2. **El control**: `NetworkPolicyManager` deja de usar el ciclo local de
   Android y usa el ciclo de Jazztel (mismo día de renovación, mismos umbrales).
3. **La transparencia**: el usuario sigue viendo **Ajustes → Conexiones → Uso de
   datos** (la pantalla de Samsung), pero los números son los reales del
   operador. **No hay app nueva que abrir, ni permisos que conceder, ni aviso
   de que "el sistema ha cambiado"**: el daemon corre por debajo.

El resultado: **Android deja de ser la fuente de verdad y Jazztel lo es**, sin
que el sistema "se entere".

## C.2 Dónde se inyecta

| Objetivo | Proceso | Qué se hookea | Por qué |
|---|---|---|---|
| **Primario** | `system_server` | `com.android.server.NetworkManagementService$NetworkStatsServiceImpl.querySummary*`, `queryDetails*`, `registerUsageCallback`; y `com.android.server.NetworkPolicyService` (límites y ciclo) | Un solo sitio ⇒ **todas** las apps (Ajustes, Samsung, monitores de datos) ven lo mismo; es donde vive la verdad del sistema |
| **Secundario** | `com.android.settings` | `android.app.usage.NetworkStatsManager.querySummary*` | Menos invasivo; sólo afecta a la pantalla de Ajustes |
| **Secundario** | `netd` | lectura de los mapas eBPF | Sólo para *contrastar* (el sistema puede seguir calculando su cifra para diagnóstico) |

El repositorio **ya tiene** el inyector ptrace (`crates/injector-payload`) y el
puente (`rust-core/src/hooks_bridge.rs`, tool `inject_hooks`, acciones
`estado|instalar|inyectar|leer`). Falta **una pieza**: el payload actual es
**frida-gum puro** (`crates/hooks-payload/README.md`: *"no depende de gum, ni de
bindgen"*), y **hookear métodos ART exige el puente Java**.

⇒ **Trabajo nuevo**: un crate `crates/hooks-payload-java` (o un feature
`java-bridge` en el actual) con `frida-java-bridge`, que además sirve para
hookear las clases de la app Jazztel **por nombre** (sin resolver la
ofuscación). Es el único bloque que impide cerrar la sustitución.

### Nota de honestidad sobre "que el sistema ni se entere"

* Lo que **sí** se puede ocultar: que el daemon está en `system_server`; el
  número se sirve como si fuera del contador.
* Lo que **no** se puede ocultar: un **root** es visible para la app (el
  daemon usa root y ptrace), y ThreatMetrix puede detectarlo ⇒ la app Jazztel
  puede reaccionar. Se puede **hookear** la detección (o el hook se hace en
  `system_server`, fuera del proceso de la app), pero el riesgo es real y hay
  que decirlo.

## C.3 Los hooks, uno a uno

| # | Objetivo | Símbolo / clase | Qué se sustituye |
|---|---|---|---|
| H1 | `querySummary(networkType, subscriberId, start, end)` | `android.app.usage.NetworkStatsManager` | Se devuelve un `NetworkStats` **sintético** con los bytes del operador desglosados por app (el operador no da por-app ⇒ se reparte con el desglose local del sistema, o se devuelve el agregado) |
| H2 | `querySummaryForDevice` | ídem | Total del operador por transporte |
| H3 | `queryDetails*` | ídem | Detalle del operador (si el `consumptionType` lo permite) |
| H4 | `registerUsageCallback` | ídem | Se re-emite el callback con el umbral **del operador** |
| H5 | `getLimitBytes` / `getLimitAlertBytes` / `getDataUsageRestriction` | `android.net.NetworkPolicyManager` | La **cuota de Jazztel** y su ciclo |
| H6 | `getUsageStats` de Samsung (si la pantalla de Samsung usa un provider propio) | a confirmar (C9) | Ídem |

**Diseño de datos**: el daemon mantiene en su estado local, por línea y por
producto, el consumo del operador (`consumed`, `remaining`, `included`,
`expirationDate`, `consumptionType`) y el desglose por app del sistema. Al
servir H1/H2, combina: **el total y la cuota son del operador; el reparto por app
es del sistema** ⇒ y eso se dice en la interfaz ("estimado por app"), porque
**el operador no da el desglose por app**.

## C.4 MITM: proxy, CA y el pin

Autorizado y **necesario**: el pin del servidor (`3257d599…`) impide el MITM
clásico.

| Paso | Qué se hace | Detalle |
|---|---|---|
| 1 | Proxy local en el móvil | El repo ya tiene `web_bridge.py` y el patrón de `sqlite_mitm` (bind-mount + estado) |
| 2 | CA propia | `cert_android.rs` ya importa CA al vault (`/data/misc/keychain/cacerts-added`, `/system/etc/security/cacerts`) |
| 3 | **Neutralizar el pin** | Hook del punto de verificación (p. ej. `okhttp3.CertificatePinner.check`) **en el proceso de la app**, o hook del código que lee `assets/stamp-cert-sha256` |
| 4 | **Neutralizar el anti-tamper** | Hook de la comprobación de integridad / ThreatMetrix, o desactivar `assets/atd` |
| 5 | **No tocar el APK** | Parchear el APK rompería la firma ⇒ **hook, no parcheo** |

**Alternativa mejor que el MITM para el consumo**: usar la **API con OAuth2
passwordless** (§B.4). El MITM queda para lo que **no** hay endpoint (p. ej. el
detalle por evento si sólo llega por app, o el historical PDF que sólo se
descarga desde el área de clientes).

## C.5 Credenciales: sólo en Google Drive, nunca en el repo

| Copia | Dónde | Cómo |
|---|---|---|
| **Trabajo (móvil)** | `<state>/gasto_datos/jazztel_creds.json` | Modo **0600**, cifrado en reposo con `Argon2id` (el crate ya tiene `argon2.rs` con vectores en `docs/argon2id_vectors.json`) y `backup_crypto.rs` |
| **Copia (Drive)** | Carpeta privada `15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0` | **Cifrada** (nunca en claro), subida con `tools/drive_push.py` |
| **En el repo** | **Nada** | Sólo el mecanismo |

Reglas obligatorias:

1. `.gitignore` **no** excluye `state/` en bloque ⇒ hay que **añadir**
   `state/gasto_datos.sqlite`, `state/gasto_datos.sqlite-*` y
   `state/gasto_datos/`. Sin eso, el consumo y la línea **acaban en GitHub**.
2. El fichero se comprueba con `stat -c %a` antes de usarlo; si no es 0600 ⇒ no
   se lee (`error: "credenciales_permisos_incorrectos"`).
3. **Nunca se imprime el token** (precedente: `tools/drive_fetch.py` imprime
   `token_sha16`).
4. El **DNI y el número de línea no salen** en logs ni en el JSON público: se
   expone `linea_alias` ("linea-1") y `dni_sha16`.
5. Las facturas de Drive llevan el **NIF en el nombre** ⇒ se referencian por
   **hash del contenido**, nunca por el nombre.
6. El NDJSON de hooks va **redactado** (`crates/hooks-payload/src/redact.rs`) ⇒
   se puede subir a Drive sin filtrar el `Authorization`.
7. **Rotación**: el token de sesión OAuth2 es de vida corta ⇒ se refresca con
   `TokenRefreshControllerImpl`-equivalente y se re-sincroniza a Drive.

## C.6 La API del comando `gasto_datos`

Registro en los **dos** despachos: `rust-core/src/mcp_ext.rs` (→ `POST
/api/tool/gasto_datos`, `cockpit_server.rs:2632`) y `rust-core/src/android.rs`
(`dispatch` + `tool_definitions`, MCP del móvil). Estilo a imitar:
`sqlite_mitm` (estado propio + SQLite + acciones).

| Acción | Qué hace | Muta | Irreversible |
|---|---|---|---|
| `estado` | Snapshot: MB por categoría + total + origen | No | No |
| `datos_moviles` | Sólo móvil, con desglose por app | No | No |
| `datos_wifi` | Sólo WiFi, con desglose por app | No | No |
| `comparar` | Operador vs sistema, con diferencias explicadas | No | No |
| `avisos` | Umbrales, MB restantes, fecha de renovación | **Sí** (emite alertas) | No |
| `refrescar` | Relee la fuente (API/caché) y persiste | **Sí** | **Sí** (login ⇒ credenciales) |
| **`estimar_factura`** | **Proyección de la próxima factura** (§C.8) | No | No |
| **`watchdog`** | **Estado del vigilante de líneas y cargos** (§C.9) | No | No |
| **`ahorrar`** | **Cargos reducibles detectados** (§C.10) | No | No |
| **`fuentes`** | **Catálogo de fuentes Jazztel y su estado** (§C.11) | No | No |

```jsonc
{ "action": "estado",              // enum con las 9 acciones
  "linea": "linea-1",              // alias, NUNCA el número
  "ciclo": "2026-03",
  "umbrales": [50, 80, 95, 100],
  "confirm": false }               // obligatorio true para "refrescar"
```

Respuesta de `estado` (los campos `origen` y `confianza` son **obligatorios**):

```jsonc
{ "ok": true, "origen": "jazztel_api", "confianza": "alta",
  "ts": 1793179200000,
  "ciclo": {"inicio":1790832000000,"fin":1793510400000,"etiqueta":"2026-03"},
  "total_mb": 1180.0, "total_bytes": 1180000000,
  "cuota_mb": 2000.0, "consumidos_mb": 1180.0,
  "por_categoria": [
    {"categoria":"movil","mb":1100.0,"origen":"sistema","estimado":true},
    {"categoria":"wifi","mb":134.5,"origen":"sistema","estimado":true},
    {"categoria":"bono","mb":0.0,"origen":"jazztel_api","estimado":false}],
  "notas": ["el total y la cuota son del operador; el reparto por app es estimación del sistema"] }
```

**Doble auth**: el gate ya existe (`rust-core/src/api_key.rs:473,618`):
`X-API-Key` o `Authorization: Bearer` **y** `X-Seere-Auth-Token`
(`antigravity_intel.rs:77-79`). `gasto_datos` entra por `POST /api/tool/…`, que
**ya está detrás del gate**; no se añade a la lista de rutas públicas
(`cockpit_server.rs:268`, `:1234`). `refrescar` exige `confirm: true`
(precedente: `/api/v2/admin/restart`, `android.rs:1104` `needs_confirm`).

## C.7 Persistencia

`rusqlite 0.32` ya está en `rust-core/Cargo.toml:33` (features `bundled`).
State dir: `crate::state_dir::state_dir()` ⇒ `$MEGADAEMON_STATE_DIR` o
`/data/user/0/com.agy/files/megadaemon/state`.

```sql
CREATE TABLE IF NOT EXISTS muestras (
  id INTEGER PRIMARY KEY, ts INTEGER NOT NULL,
  uid INTEGER, paquete TEXT, iface TEXT,
  categoria TEXT NOT NULL,        -- movil|wifi|ethernet|vpn|tethering
  bytes_rx INTEGER NOT NULL DEFAULT 0, bytes_tx INTEGER NOT NULL DEFAULT 0,
  tag INTEGER, state INTEGER, metered INTEGER NOT NULL DEFAULT 0,
  roaming INTEGER NOT NULL DEFAULT 0,
  origen TEXT NOT NULL);          -- sistema|jazztel_api|jazztel_cache|factura
CREATE INDEX IF NOT EXISTS ix_muestras_ts  ON muestras(ts);
CREATE INDEX IF NOT EXISTS ix_muestras_pkg ON muestras(paquete, ts);

CREATE TABLE IF NOT EXISTS lineas (            -- una por línea activa
  alias TEXT PRIMARY KEY,                       -- "linea-1" (NUNCA el número)
  subscription_id TEXT, mcc_mnc TEXT, operador TEXT,
  producto_id TEXT, estado TEXT,               -- activa|suspendida|desactivada
  inicio_ciclo INTEGER, fin_ciclo INTEGER,
  cuota_bytes INTEGER, consumidos_bytes INTEGER, restante_bytes INTEGER,
  consumption_type TEXT, expiration_date INTEGER,
  fuente TEXT NOT NULL, actualizado INTEGER NOT NULL);

CREATE TABLE IF NOT EXISTS cargos (             -- cargos detectados
  id INTEGER PRIMARY KEY, ts INTEGER NOT NULL,
  linea_alias TEXT, tipo TEXT NOT NULL,         -- mora|reconexion|bono|extra|penalty
  concepto TEXT, importe_eur REAL, impuestos_eur REAL,
  periodo TEXT, factura TEXT,                  -- hash, NUNCA el nombre del PDF
  estado TEXT);                                -- detectado|confirmado|ignorado

CREATE TABLE IF NOT EXISTS ciclos (
  etiqueta TEXT PRIMARY KEY, cuenta_alias TEXT, inicio INTEGER, fin INTEGER,
  cuota_bytes INTEGER, consumidos_bytes INTEGER, total_price_eur REAL,
  penalties_eur REAL, fuente TEXT NOT NULL);

CREATE TABLE IF NOT EXISTS avisos (
  id TEXT PRIMARY KEY, regla TEXT NOT NULL, ciclo TEXT NOT NULL,
  nivel TEXT NOT NULL, titulo TEXT NOT NULL, cuerpo TEXT,
  creado INTEGER NOT NULL, visto INTEGER NOT NULL DEFAULT 0);
CREATE UNIQUE INDEX IF NOT EXISTS ix_avisos_dedup ON avisos(ciclo, regla);
```

## C.8 El estimador de factura

### Los importes reales (verificados)

| Concepto | Importe | Fuente |
|---|---|---|
| **Cargo por gestión de cobro** (factura devuelta/impagada) | **15,99 €** si la media de las 3 últimas facturas es > 10 €; **12 €** si es ≤ 10 €. Sin impuestos | <https://ayuda.jazztel.com/factura-y-consumo/conceptos-de-factura/1959-cargo-por-gestion-de-cobro-y-cargo-por-reconexion> |
| **Cargo por reconexión de servicio** | **11,99 € + impuestos** (IVA 21 % / IPSI 10 % / 8 % / IGIC 7 %) | ídem |
| Según contrato (feb-2025) | **28 €** por factura | <https://sinpleitos.es/servicios/ficheros-morosidad/recursos/recargo-impago-telefonia> |
| Intereses de demora | interés legal del dinero + 2 puntos sobre lo adeudado | ídem |
| Ley 3/2004 (entre empresas) | 40 € + 10,40 % (2.º semestre 2026) | <https://invoo.es/es/blog/consejos/calcular-interes-demora-1040-costes-cobro-factura/> — **NO aplica a un consumidor** |

⚠️ **Corrección del encargo**: el "39 €" de la mora **no existe** en las
condiciones de Jazztel al consumidor. Los cargos aparecen **en la factura
siguiente** (literal: *"Podrás verlos en tu próxima factura en el apartado
'Resto conceptos asociados a la línea/s' con los literales 'Cargo por gestión de
cobro' y 'Cargo por reconexión'"*) ⇒ **confirmado**: el recargo por pago tarde
aparece en la **próxima** factura.

### Datos de consumo verificados de Jazztel

| Concepto | Dato | Fuente |
|---|---|---|
| Bono al agotar la cuota | **Bono 100**: 2 € por cada 100 MB; si no está activado, la velocidad cae a **16 kbps** | <https://www.jazztel.com/cliente/lineas-adicionales> |
| Bonos puntuales | 200 MB 3,57 € · 500 MB 5,99 € · 1 GB 10,83 € | <https://www.lavanguardia.com/andro4all/operadoras/como-ver-el-consumo-en-jazztel-y-saber-cuantos-gigas-te-quedan> |
| Datos ilimitados | 50 GB a 1000 Mbps; después baja (150 GB ≈ calidad DVD) | ídem · jazztel.com |
| Compartir GB | hasta el 80 % de cada ciclo, en pasos de 0,5 GB | ídem |
| Acumulación | los GB no usados se acumulan al mes siguiente (gratis) | ídem |

### El algoritmo

```
base      = products-api/accounts/{accountId}/total-price        (cuotas + productos)
cuota     = consumption-api/.../summary.included
consumido = consumption-api/.../summary.consumed
restante  = summary.remaining
tipo      = summary.consumptionType   (unlimited | normal | ...)

# 1) DATOS
if consumido <= cuota:      extra_datos = 0
elif unlimited:            extra_datos = 0        # el corte de velocidad ya está dentro
                           (el umbral de 50 GB se lee de consumptionType/expirationDate)
else:                      extra_datos = ceil((consumido-cuota)/100MB) * 2.00 EUR   # Bono 100
                           # (si el usuario no tiene Bono 100 activo: velocidad 16 kbps, coste 0)

# 2) EXTRAS (llamadas/SMS/roaming fuera de tarifa)
extra_llamadas = suma de EventsApi (NetworkCostType: BigDecimal) del periodo
                 + services-api/.../data-cession (cesiones)

# 3) BONOS
bonos = suma de bonus-api/.../bonuses con precio (NetworkPrice) y fecha de fin > fin_de_ciclo
        + bonificaciones recurrentes activas

# 4) MORA Y RECARGOS (del ciclo ANTERIOR)
if factura_anterior_no_pagada_en_plazo:
    mora   = "Cargo por gestión de cobro"  (15,99 € | 12 €)
    recon  = "Cargo por reconexión" (11,99 €)   solo si hubo corte de servicio
    interes= (interés legal + 2 puntos) * dias_retraso / 365 * saldo_vencido
    # y estos cargos se cargarían en la SIGUIENTE factura
    # -> el estimador debe mirar el estado de pago de la factura en curso
penalty_api = products-api/accounts/{accountId}/penalties   (fuente de verdad)

# 5) IMPUESTOS
IVA 21 % (Peinsula/Baleares) · IPSI 10 % (Ceuta) · 8 % (Melilla) · IGIC 7 % (Canarias)
total = base + extra_datos + extra_llamadas + bonos + mora + recon + interes + impuestos
```

**Salida**:

```jsonc
{ "ok": true, "accion": "estimar_factura",
  "ciclo": "2026-03",
  "importe_estimado_eur": 41.78,
  "base_eur": 39.00,
  "detalle": [
    {"concepto":"cuota mensual","eur":39.00,"fuente":"products-api/total-price"},
    {"concepto":"extra de datos (Bono 100, 200 MB)","eur":4.00,
     "fuente":"consumption-api/summary + tarifa Bono 100"},
    {"concepto":"Cargo por gestión de cobro","eur":15.99,
     "fuente":"factura anterior impagada","aparece_en":"factura siguiente"},
    {"concepto":"IVA 21 %","eur":9.83}],
  "rango": {"min_eur":39.00,"max_eur":52.00,
            "motivo_max":"si se activan 2 bonos de 100 MB más"},
  "confianza":"media",
  "avisos":["si no se paga a tiempo, la siguiente factura llevará 15,99 € de gestión de cobro"] }
```

**Regla de honestidad**: si un dato no está (cuota, estado de pago, tarifas),
el campo va `null` y el `rango` refleja la incertidumbre. **No se rellena con
suposiciones**, y `confianza` baja a `baja`.

## C.9 Watchdog de líneas y cargos

Tarea periódica (el repo ya tiene `task_scheduler`/`task_queue` y `alerts.rs`
con `raise(rule, severity, title, body, dedup_key)`).

* **Instante de arranque**: tras `refrescar` y cada N horas (configurable).
* **Por cada línea activa**: consulta `consumption-api/…/summary`,
  `balances-api/…/events`, `balances-api/…/user-tariffs`,
  `bonus-api/…/bonuses`, `products-api/…/penalties` y
  `invoices-api/…/billing/periods`.
* **Diff contra el snapshot anterior** y avisos con `dedup_key` estable:

| Regla | Condición | Severidad |
|---|---|---|
| `umbral_50/80/95/100` | `consumido/cuota` ≥ umbral | info / aviso / aviso / crítica |
| `nuevo_cargo` | aparece un cargo en `cargos` que no estaba | **crítica** |
| `mora_pendiente` | una factura no está pagada y vence en ≤ 5 días | **crítica** (⇒ avisa antes de la siguiente factura) |
| `linea_suspendida` | `estado` ≠ activa o `products-api/.../penalties` no vacío | crítica |
| `bono_por_vencer` | `expirationDate` dentro de 7 días | aviso |
| `bono_terminado` | bono `active_*` con fecha de fin pasada | info (⇒ **oportunidad de ahorro**) |
| `cambio_de_ciclo` | `billing/periods` devuelve una fecha distinta | aviso |
| `desfase_sistema` | \|sistema − operador\| > 5 % | aviso (tethering/roaming/Data Saver) |
| `recarga_recurrente` | `recurring-recharges` activa y la línea no consume | aviso (posible ahorro) |

## C.10 Detección activa de cargos reducibles

Cada regla produce una **recomendación con el ahorro estimado en euros**, no
un consejo genérico. Fuentes: `total-price`, `plans`/`catalogs/plans`,
`user-tariffs`, `user-tariff-renewal`, `bonuses`, `penalties`, `tariffs-deals`,
`events`, `data-cession`.

| Regla | Señal | Ahorro estimado |
|---|---|---|
| `tarifa_sobredimensionada` | cuota contratada ≥ 3× la media de consumo de los últimos 3 ciclos | `total-price` de la tarifa inferior equivalente (de `catalogs/plans`) |
| `bono_innecesario` | bono `active_no_recurrent` cuyo `expirationDate` ya pasó o cuyo precio > lo que se habría gastado | precio del bono |
| `bono_recurrente_desequilibrado` | bono recurrente activo y la línea apenas consume | coste del recurrente |
| `compartir_gb_sin_usar` | `isShared=false` y `sharedDataConsumed=0` con capacidad de cesión | hasta el 80 % de la cuota cedida (0,5 GB por paso) |
| `acumulacion_desaprovechada` | ciclo cerrado con `remaining>0` y acumulación no activada | `remaining` acumulado |
| `renovacion_automatica_innecesaria` | `user-tariff-renewal` activa y la línea no se usa | lo que se ahorraría cancelándola (`deleteUserTariffRenewal` existe) |
| `recarga_recurrente_sobrante` | `recurring-recharges` con importe > consumo medio | importe de la recarga |
| `mora_por_pagar_tarde` | factura próxima a vencer sin pagar | **15,99 €** (o 12 €) + 11,99 € de reconexión si hay corte |
| `bono_mas_por_agotar` | `consumido` muy cerca de `included` | coste del "Bono Más" que toque (200 MB 3,57 € / 500 MB 5,99 € / 1 GB 10,83 €) |
| `cesion_de_datos_sin_usar` | `data-cession` activa sin tráfico | coste de la cesión |

⇒ Cada recomendación lleva: `ahorro_estimado_eur`, `confianza`, `accion`
(`cancelar_bono`, `bajar_tarifa`, `no_pagar_tarde`, `activar_compartir`, …) y
**cómo se verifica** (endpoint concreto).

## C.11 Todas las fuentes de Jazztel y el catálogo

| Fuente | Endpoint / artefacto | Qué aporta | Prioridad |
|---|---|---|---|
| API de consumo | `consumption-api/…/{summary,consumptions,details,activity-detail}` | consumo, cuota, restante, expiración, comparten | **1** |
| Eventos de consumo | `balances-api/…/events` | coste por evento (euros) | 2 |
| Saldos | `balances-api/…/balances` | saldo, `priceInfo`, `tariffStatus` | 2 |
| Tarifa | `balances-api/…/user-tariffs` | cuota contratada, `NetworkSpendable` | 1 |
| Renovación | `balances-api/…/user-tariff-renewal` | fecha y estado de la renovación | 1 |
| **Recargos** | `products-api/accounts/{id}/penalties` | cargos ya aplicados | **1** |
| Precio total | `products-api/accounts/{id}/total-price` | base de la factura | 1 |
| Planes | `products-api/…/{plans,catalogs/plans}` | alternativas de tarifa (ahorro) | 2 |
| Bonos | `bonus-api/…/bonuses` | bonos activos, recurrentes, precios | 1 |
| Facturas | `invoices-api/…/billing/periods` | ciclos, importes, estado de pago | **1** |
| Deals | `balances-api/…/tariffs-deals` | ofertas aplicables | 3 |
| Cesión de datos | `services-api/…/data-cession` | cesiones (ahorro y coste) | 3 |
| Recargas | `balances-api/…/{recharges,recurring}` |(recarga automática) | 2 |
| Pagos | `balances-api/…/payments/methods`, `balances-api/payments/notifications` | método y avisos de pago | 2 |
| Móvil/SIM | `services-api/…/{sim/activation,puk}` | estado del SIM (suspensión) | 2 |
| Dispositivos | `devices-api/…/{devices,orders,shipments,insurances}` | seguros, Pedidos | 3 |
| Suscripciones | `subscriptions-api/subscriptions[/tariffs]` | catálogo de líneas | 2 |
| Autenticación | `authn.masstack.com/v1/` (OAuth2 passwordless/magic-link/webauthn, CIBA, device-code, OTP) | **login programático** | **1** |
| Consentimientos | `mas-consents-front-v2.masstack.com` | consentimientos (RGPD) | 3 |
| Asistente | `lili-agent.prod.k8s.masmovil.com` | mensajes proactivos (avisos del operador) | 3 |
| **Caché local** | `/data/user/0/com.orange.bluu.jazztel/` (`no_backup` 20,4 MiB, `databases/`, `shared_prefs/`) | copia offline, **sin red** | 2 |
| **Facturas PDF** | carpeta privada de Drive | verdad histórica | 1 |
| **USSD `*167#`** | código corto | megas/minutos/SMS por SMS (sin login) | 3 |
| **Área de clientes** | `mi.jazztel.com` | todo lo anterior, vía web | 3 |
| Lista de hosts | `assets/app-allowed-hosts.json` | mapa de subdominios y features | 2 |

### Inteligencia con LLM gratuito y datos anonimizados

El daemon ya tiene `rust-core/src/provider_catalog.rs` con **tiers de
gratuiticidad** (`tier_free()` ⇒ `"free" | "keyless" | "offline"`) y proveedores:

| Proveedor | Tier | Notas |
|---|---|---|
| `local-seere`, `local-seere-vllm`, `lemonade-local` | `offline`/`local` | **El motor local SEERE** ⇒ sin red y sin coste |
| `pollinations-free` | `keyless` | Sin registro |
| `ovhcloud-free` (`qwen3.6-35b-a3b`) | `free` | Sin coste con cuota |
| `groq-free` | `free` | |
| `openrouter-free` (`:free`) | `free` | |
| Google AI Studio | `free` | |
| Cloudflare Workers AI, Z.ai GLM Flash, Cohere trial, Mistral free | `free` | |
| **Antigravity/Gemini (Google)** | el que ya usa el repo para la RE | vía `llm_gateway.rs` (`normalizar_ruta_gemini`, `gemini_route`) y `antigravity_endpoint.rs` |

**Reglas de la inteligencia**:

1. **Sólo proveedores con `tier_free()`** ⇒ **cero tokens de pago**. El
   `llm_gateway.rs` ya normaliza y enruta; sólo hay que filtrar por tier.
2. **Anonimización antes de salir**: al prompt sólo van
   *agregados numéricos*, *claves JSON*, *nombres de endpoint* y
   *alias de línea*. **Nunca**: DNI, número de línea, nombre, IBAN, correos,
   identificadores de factura, ni texto libre de la factura.
3. **Filtro de salida**: el mismo `redact.rs`/`mask_secret` que usa el payload
   de hooks se aplica a la respuesta ⇒ si el modelo devuelve algo que parece un
   identificador, se descarta.
4. **Uso concreto**: convertir el diff del watchdog y las reglas de §C.10 en
   un informe priorizado ("esto te cuesta 12,99 €/mes y se puede quitar"), y
   explicar en español el por qué de una discrepancia sistema↔operador.
5. **El LLM no decide**: las cifras y los importes salen de la API; el LLM
   redacta y prioriza. Las reglas numéricas son código determinista y testeable.

## C.12 Verificación

### Tests unitarios puros (sin móvil, sin red)

| Test | Qué fija |
|---|---|
| `mb_y_mib_no_se_confunden` | 1.000.000 B = 1 MB decimal; 1.048.576 B = 1 MiB. **El fallo clásico** |
| `clasificar_red_mete_vpn_bajo_su_transporte` | `TRANSPORT_VPN`+`TRANSPORT_WIFI` ⇒ `wifi`, `bajo:"wifi"` |
| `la_red_no_medrada_no_cuenta_como_facturable` | `NET_CAPABILITY_NOT_METERED` ⇒ `facturable=false` |
| `el_bono_100_se_cobra_por_bloques_de_100_mb` | 150 MB de exceso ⇒ 2 bloques ⇒ 4,00 €; 99 MB ⇒ 0 € |
| `el_unlimited_no_cobra_extra_hasta_50_gb` | `consumptionType=unlimited` con 40 GB ⇒ 0 €; a 60 GB ⇒ según umbral |
| `la_mora_va_en_la_factura_siguiente` | factura impagada ⇒ el cargo aparece en `ciclo+1`, **no** en el actual |
| `la_mora_cambia_segun_la_media_de_las_tres_ultimas` | media > 10 € ⇒ 15,99 €; ≤ 10 € ⇒ 12,00 € |
| `la_reconexion_solo_si_hubo_corte` | sin corte ⇒ 0; con corte ⇒ 11,99 € + IVA |
| `los_intereses_de_demora_usan_dias_y_tasa` | `(saldo × tasa × días / 365)` |
| `el_iva_varia_por_territorio` | 21 % / 10 % / 8 % / 7 % |
| `evaluar_umbrales_no_repite_el_aviso` | 80 % y 95 % ⇒ 2 avisos; reevaluar ⇒ 0 nuevos |
| `el_delta_sin_explicar_no_se_reparte` | 80 MB sin causa ⇒ `explicaciones: []`, `delta_explicado:false` |
| `el_tethering_explica_la_diferencia` | con tethering ⇒ explicación `tethering` |
| `el_snapshot_distingue_sistema_de_operador` | `origen` y `confianza` en cada fila |
| `parseo_de_factura_tolera_formatos` | "1.180,00 MB" (punto de millar español) ⇒ 1180.0 |
| `el_prompt_del_llm_no_lleva_identificadores` | el anonimizador elimina DNI/número/factura antes de la llamada |
| `solo_proveedores_free_en_el_inteligente` | el filtro de tier excluye cualquier proveedor de pago |
| `el_estado_se_aisla_por_test` | `state_dir_de_test()` (un directorio por hilo; ya resuelto en `state_dir.rs`) |
| `no_se_persisten_credenciales` | fichero 0600 y contenido ausente de toda salida JSON |

### Mediciones en el móvil (`NO VERIFICADO` hasta que se hagan)

| ID | Medición | Cómo | Estado |
|---|---|---|---|
| M1 | `NetworkStatsManager` devuelve filas con `metered`/`roaming` correctos | `dumpsys netstats detail` | `NO VERIFICADO` |
| M2 | El desglose por app cuadra con Ajustes → Uso de datos | comparar `datos_moviles` con la pantalla | `NO VERIFICADO` |
| M3 | La caché local de la app tiene el consumo en formato legible | `ls` de `no_backup/`, `databases/`, `shared_prefs/` | `NO VERIFICADO` |
| M4 | Los hooks `connect`/`getaddrinfo` dan el host de la API de consumo | `inject_hooks` → `inyectar` → "Mi consumo" → NDJSON | `NO VERIFICADO` |
| M5 | `SSL_read` se puede hookear en `libssl.so` (Conscrypt) | añadir el hook y ver `instalado` | `NO VERIFICADO` |
| M6 | El total del sistema vs el de la factura: la diferencia se explica | `comparar` con una factura real | `NO VERIFICADO` |
| M7 | El sistema cuenta o no el tráfico de tethering | hotspot on/off + comparar | `NO VERIFICADO` |
| M8 | El ciclo de la app == el de la factura | `invoices-api/…/billing/periods` vs PDF | `NO VERIFICADO` |
| M9 | El hook en `system_server` de `querySummary*` sirve el número del operador | inyectar y abrir Ajustes → Uso de datos | `NO VERIFICADO` |
| M10 | El puente Java con frida funciona en `system_server` | `inject_hooks` con el payload nuevo | `NO VERIFICADO` |
| M11 | Login OAuth2 *passwordless* contra `authn.masstack.com` | `curl` con el token ya guardado | `NO VERIFICADO` |
| M12 | `products-api/…/penalties` devuelve el recargo real | con la sesión real | `NO VERIFICADO` |
| M13 | La factura real usa "1.180,00 MB" u otro formato | `pypdf` sobre una factura | `NO VERIFICADO` |
| M14 | El pin de TLS bloquea el proxy con CA de usuario | proxy + hook del pin | `NO VERIFICADO` |

---

## C.13 Módulo `re_deep`: la RE que faltaba, ejecutada

`dex_decompile` + `apk_deobfuscator` + `apk_inventory` cubren el desensamblado,
el manifiesto, la firma y el fingerprint de ofuscación. **No** cubren las cinco
cosas que hicieron falta para reconstruir el subsistema de datos de un operador
real. Eso es lo que añade el módulo nuevo `rust-core/src/re_deep.rs`.

| Lo que añade | Para qué sirve en este encargo |
|---|---|
| **Xrefs** (string → clase + método) | Ligar un string como `consumption-api/subscriptions/{id}/summary` a la interfaz que lo sirve: sin esto, una ruta no se puede atribuir a ninguna pantalla |
| **Rutas REST relativas** con su familia de API | `extract_endpoints` devuelve URLs *absolutas*; el inventario real de un backend son rutas *relativas* (`consumption-api/...`) |
| **Entropía de los assets** | Un asset de alta entropía (> 7,9 bits/byte), sin magic reconocible y de más de 512 B es, con mucha probabilidad, un **payload cifrado**: una señal de empaquetado o módulo protegido que el inventario actual no ve |
| **Pin de certificado** vs certificados de firma | Si el pin declarado **no** es ningún certificado de firma del APK, es del **servidor**: una CA de usuario no basta y hay que hookear la verificación (ver §C.4) |
| **Descifrador de strings, discriminado** | Un candidato con material `javax/crypto/*` es un SDK de pago 3DS2, no un ofuscador de strings: sin esa distinción, una app con criptografía legítima genera cientos de falsos positivos |

### Cómo se usa

```sh
# Informe completo de un APK (xrefs, rutas, entropía, pins, descifradores)
megadaemon-core redeep /ruta/app.apk

# Sólo una parte
megadaemon-core redeep /ruta/app.apk rutas
megadaemon-core redeep /ruta/app.apk pins
megadaemon-core redeep /ruta/app.apk descifradores
megadaemon-core redeep /ruta/app.apk xrefs consumption-api
```

Y desde la tool MCP `re_profunda`, con las mismas acciones (`analizar`,
`rutas`, `pins`, `descifradores`, `xrefs`).

### El bloque 12: decodificador validado + autocalibración

Después de escribir este módulo, la RE de la app Jazztel destapó **tres bugs
de decodificación en el repo** (§E.3). El módulo incluye ahora, en
`rust-core/src/re_deep.rs`:

| API | Qué hace |
|---|---|
| `dalvik_width(op)` | Ancho **medido** (con `goto` = 1 unidad) |
| `decode_code(insns)` | Decodificador con las convenciones medidas y los payloads |
| `audit_decode(dexes)` | **Autocalibración**: encaje de anchuras + alineación de saltos + coincidencia de firmas ⇒ veredicto `ok` / `degradado: …` |
| `build_cfg(insns)` | Grafo de control por bloques básicos |
| `opaque_branches(insns)` | Predicados opacos por propagación de constantes |

El diseño es deliberado: **el motor se audita a sí mismo**. Si alguien cambia
las tablas de opcodes, `audit_decode` lo dice en lugar de que el análisis
produzca basura silenciosamente.

> **Estado en `lib.rs`**: `pub mod re_deep;` **ya está declarado** (lo hizo
> el root), así que el módulo **entra en el build** ⇒ el gate
> de higiene de módulos del CI se satisface. Pero el módulo **no se ha
> compilado** en este entorno (prohibido) ⇒ el primer paso es `cargo check` +
> `audit_decode` sobre un APK real (ver §D.5).

### Cinco bugs reales encontrados al integrar el módulo

Ninguno de los cinco lo había detectado el subagente que lo escribió; los
encontraron el compilador, la comparación con el parser canónico del repo y el
contraste con los datos reales de la app:

1. **`substring` no compilaba** (`re_deep.rs`): `s.get(rango)` devuelve
   `Option<&[char]>`, y el `.map()` intentaba llamar a `to_string()` sobre un
   trozo de slice. Además el corte por *caracteres* es el correcto: con emojis
   o acentos, cortar por bytes parte el carácter al medio.
2. **Los *payloads* de Dalvik nunca se saltaban** (tres barreadores:
   `const_string_indexes`, `method_calls` y el intérprete): el identificador
   del payload se leía como `(unidad >> 8)`, que da 0x01/0x02/0x03, cuando el
   valor real es 0x0100/0x0200/0x0300 (byte alto *en su sitio*). Como la
   comparación nunca casaba, el barrido continuaba **dentro de los datos** del
   payload e inventaba xrefs y llamadas a método fantasma. El parser canónico
   (`dex_decompile::disassemble_code`) usa `unidad & 0xff00`: ahora `re_deep`
   usa el mismo criterio, y los cuatro barreidores comparten **un único
   helper** (`siguiente_insn`) para que no vuelvan a divergir entre sí.
3. **El barrido se desalineaba tras cada `goto`**: la tabla de anchos local
   (por rangos de opcode) no incluía el `0x28` (`goto`, formato `10t`), que
   ocupa **dos** unidades: opcode + offset. Con ancho 1, a partir del primer
   salto el barrido leía el offset como si fuera código. El parser canónico ya
   avisaba de ese caso; ahora `insn_width` **delega** en
   `dex_decompile::instruction_width`, que está indexada por formato.
4. **El detector de descifradores estaba muerto**: `decryptor_candidates`
   buscaba el XOR dentro de la lista de *invocaciones* (`method_calls`), pero
   un XOR no invoca a nadie ⇒ la condición **nunca** era cierta ⇒ el detector
   **no encontraba ningún descifrador nunca**. Y la lista de opcodes que
   llevaba estaba mal por partida doble: `0x98` es `shl-int` y `0xe0` es
   `shl-int/lit8` (no son XOR), y se le pasaban `xor-long` (0xa2),
   `xor-long/2addr` (0xc2) y `xor-int/lit16` (0xd7). Ahora se busca por
   **nombre** de opcode con la tabla canónica (`tiene_xor`).
   *Impacto en esta app concreta*: **ninguno**, porque la app **no** cifra
   strings (medido en §B.2) y el resultado correcto es cero. El bug habría
   mattered en cualquier app que sí los cifrase.
5. **El inventario de rutas salía VACÍO en la app real** (el más grave, y
   silencioso): `classify_rest_path` rechazaba cualquier segmento con
   mayúsculas, y **todas** las rutas medidas de esta app llevan parámetros en
   camelCase (`{subscriptionId}`, `{accountId}`, `{productId}` — ver §B.4).
   El módulo no daba error: simplemente no encontraba ninguna ruta. Ahora las
   mayúsculas sólo se rechazan en los segmentos **estáticos**; un parámetro
   (`{subscriptionId}`) es el nombre de una variable Java y se acepta tal cual.

   **MEDIDO sobre el APK real** (pool de strings de los 5 DEX:
   `classes.dex` … `classes5.dex`, 169.488 strings únicos):

   | Clasificador | Rutas encontradas |
   |---|---|
   | el de antes (rechaza mayúsculas en cualquier segmento) | **16** |
   | el de ahora (rechaza mayúsculas sólo en segmentos estáticos) | **133** |

   El nuevo es un **superconjunto estricto**: 0 rutas perdidas, 117 ganadas. Y
   las ganadas son precisamente las que hacen falta para el control de gasto:
   `balances-api/accounts/{accountId}/subscriptions/{subscriptionId}/user-tariffs`
   (la tarifa), `…/recharges` y `…/recurring` (las recargas), `…/balances` y
   `…/events` (el coste por evento), `consumption-api/subscriptions/{subscriptionId}/summary`
   (el resumen) y `products-api/accounts/{accountId}/penalties` (los recargos).
   Las cuatro rutas de la tabla de arriba se comprobaron una a una **contra el
   pool real**: están.

### Tests

`re_deep` no tenía ni un test. Ahora tiene **24**, todos puros (sin APK, sin
red, sin móvil): los vectores SHA-256 de NIST, los tres tipos de payload de
Dalvik (con la regresión del bug 2), la clasificación de rutas REST (con los
falsos positivos típicos **y** con las 8 rutas reales medidas de la app, que
son la regresión del bug 3), la entropía y los magics, el pin de servidor, los
xrefs, el índice de strings por clase, las llamadas a método, la
discriminación entre ofuscador y SDK de pago, y el cuadre del informe
completo.

---

# PARTE D — Estado de los hechos

## D.1 (a) VERIFICADO — leído de ficheros del workspace o de fuentes abiertas

| # | Hecho | Evidencia |
|---|---|---|
| V1 | Operador de la SIM **JAZZTEL**, MCC/MNC **21403** | `rust-core/src/call_recorder.rs:1609-1610` |
| V2 | La app Jazztel en el móvil es `com.orange.bluu.jazztel` | `docs/PHONE_INVENTORY.md:409` |
| V3 | `no_backup` de la app = **20.854 KB = 20,4 MiB** | `docs/PHONE_INVENTORY.md:409` y `du -sk` de la bitácora |
| V4 | El móvil es **Samsung SM-S908B, Android 13** | `docs/PHONE_INVENTORY.md:22` |
| V5 | El APK de Jazztel en Drive: 44.050.916 B, `md5 de06e522fcae656d6f259a782ee7e710` | listado OAuth de Drive (2026-11-26) |
| V6 | **Descargado y verificado**: `md5` idéntico, `sha256 fd475219e01c39739cc35966c6b9cea17047fce39640d41e7d348efa78a2a435` | `/workspace/apk_work/jazztel/jazztel.apk` |
| V7 | `package com.orange.bluu.jazztel`, **`versionName 26.47.0`**, `versionCode 37264705`, compileSdk 37 | AXML parseado |
| V8 | 5 DEX: 10.963.260 + 1.756 + 7.768.488 + 11.920.220 + 6.198.624 B | ZIP |
| V9 | **36.248 clases**, 10.318 de nombre corto (**28,47 %**), **323.861 métodos** | parser DEX propio |
| V10 | **R8 con prueba definitiva**: `SourceFile = r8-map-id-5f4776f6…` ⇒ confianza 99 | DEX |
| V11 | **Cero librerías nativas**, sin `assets/xposed_init`, sin marcadores de empaquetador | ZIP + strings |
| V12 | **Sólo 1 carga dinámica** (Play Services Dynamite) ⇒ **sin DEX empaquetado** | barrido de los 323.861 métodos |
| V13 | **No hay cifrado de strings** en el código de la app: los 200 candidatos del detector son criptografía de un SDK 3DS2 (`Latd/*`, `LQfgSZK/*`) | detector con las 3 condiciones del playbook §5.1 |
| V14 | **Namespace interno `com.ragnarok.apps`**; el backend es `*.k8s.masmovil.com` | clases |
| V15 | **Esquema JSON real** de `UnifiedDataConsumptionSummary`: `consumed`, `remaining`, `included`, `consumptionType`, `expirationDate`, `sharedDataConsumed`, `isShared` | `UnifiedDataConsumptionSummaryJsonAdapter` |
| V16 | `Balance`: `balance`, `priceInfo`, `startDate`, `expiryDate`, `tariffStatus`, `type` | `BalanceJsonAdapter` |
| V17 | `ConsumptionSummaryCostEntry$Type` usa **`BigDecimal`** ⇒ la app calcula importes en euros | descriptor de método |
| V18 | **507 rutas REST** medidas; las de consumo/facturación están en §B.4 | string pool |
| V19 | Hosts: `digital-prod.prod.k8s.masmovil.com`, `api-digital.prod.k8s.masmovil.com`, `ragnarok.prod.k8s.masmovil.com`, `authn.masstack.com/v1/`, `mas-consents-front-v2.masstack.com`, `payments.masstack.com` | `RagnarokNetworkDIModule` |
| V20 | Autenticación **MASStack OAuth2** con grant types `magic-link`, `passwordless`, `webauthn`; además CIBA, device-code, OTP, tokens, sessions | `OAuth2Api`, `…GrantTypeGetAccessTokenWithForm` |
| V21 | Interfaces: `ConsumptionApi`, `UnifiedConsumptionApi`, `BalancesApi`, `EventsApi`, `UserTariffsApi`, `TariffsDealsApi`, `BonusApi`, `BonusesApi`, `PaymentsApi`, `RechargesApi`, `RecurringRechargesApi` | clases |
| V22 | **`deleteUserTariffRenewal`** existe ⇒ la renovación se puede cancelar | `UserTariffsApi` |
| V23 | Existe el endpoint **`products-api/accounts/{accountId}/penalties`** (recargos) | string pool |
| V24 | Existe el endpoint **`invoices-api/subscriptions/{id}/billing/periods`** (ciclos y facturas) | string pool |
| V25 | Existe `services-api/…/data-cession` (cesión de datos) | string pool |
| V26 | Ciclo de vida del bono: `inactive_bonus`, `active_recurrent_bonus`, `active_no_recurrent_bonus`, `active_no_recurrent_no_end_date_bonus` (+ roaming) | `BonusApiFakeModels` |
| V27 | `assets/app-allowed-hosts.json`: mapa host→features (`authn`, `consents`, `general`, `live-person`, `ott`, `order-tracking`, `webview`, `tv-jazztel`); incluye **`auth.blue.max-tests.com` ("ott (hbo staging)")** | asset |
| V28 | `assets/stamp-cert-sha256` = `3257d599a49d2c961a471ca9843f59d341a405884583fc087df4237b733bbd6d` | asset |
| V29 | El APK se firma con **v2** y **3 certificados** (`aa3cb346…`, `781c0083…`, `b5271ce6…`) ⇒ **ninguno** es el pin ⇒ el pin es del **servidor** | bloque de firma parseado |
| V30 | `assets/atd/e/*`: payloads con entropía **8,000 bits/byte** (cifrados); `assets/a3ds2_srn` = 32 B de material de clave | análisis de entropía |
| V31 | La app incluye **LexisNexis ThreatMetrix** (`assets/services/com.lexisnexisrisk.threatmetrix.*`) | asset |
| V32 | El consumo lo sirve `ConsumptionStore` (extends `mini.Store`) → `ConsumptionState` (maps por línea + maps de tarea) | grafo de llamadas |
| V33 | Clases ofuscadas del flujo: `Lpu2;` (use case), `Luh;` (cliente de red), `Lq2b;` (saldos/tarifas), `Luhd;` (mapper red→dominio) | grafo de llamadas |
| V34 | AOSP legacy: `/proc/net/xt_qtaguid/{stats,iface_stat_fmt,ctrl}`; desde Android 9 **no** accesible a apps | AOSP + doc oficial |
| V35 | Android 9+/P+ usa **eBPF + cgroup 2** con `traffic_tag_stats_map`, `traffic_uid_stats_map`, `traffic_iface_stats_map` | AOSP |
| V36 | `NET_CAPABILITY_NOT_METERED` (y no el transporte) es lo que dice si la red cuesta dinero | Android docs |
| V37 | Samsung tiene pantalla de Uso de datos con desglose por app, ciclo y aviso; y **la llama "estimación"** | soporte de Samsung |
| V38 | `com.android.providers.telephony` es el TelephonyProvider de AOSP; **no** guarda estadísticas de tráfico | AOSP |
| V39 | Jazztel: **"Cargo por gestión de cobro" 15,99 € / 12 €** y **"Cargo por reconexión" 11,99 € + impuestos**, que aparecen en la **próxima factura** | ayuda.jazztel.com |
| V40 | Según contrato (feb-2025) el cargo es **28 €**; además **intereses de demora** (interés legal + 2 puntos) | sinpleitos.es |
| V41 | La Ley 3/2004 (40 € + 10,40 %) es **entre empresas**: no aplica a un consumidor | invoо.es |
| V42 | **Bono 100**: 2 € por 100 MB al agotar la cuota; si no, la velocidad cae a **16 kbps** | jazztel.com |
| V43 | Bonos puntuales: 200 MB 3,57 € · 500 MB 5,99 € · 1 GB 10,83 €; compartir hasta el 80 % en pasos de 0,5 GB; acumulación gratis | lavanguardia / adslzone |
| V44 | El daemon ya tiene SQLite (`rusqlite 0.32`), `alerts::raise` con `dedup_key`, gate `X-API-Key`+`X-Seere-Auth-Token`, ruta `POST /api/tool/<nombre>` y catálogo LLM con **tiers free/keyless/offline** | `Cargo.toml:33`, `alerts.rs:85`, `api_key.rs:618`, `cockpit_server.rs:2632`, `provider_catalog.rs:37` |
| V45 | El payload de hooks hookea `open`, `openat`, `unlink`, `connect`, `sendto`, `getaddrinfo`, `sqlite3_open_v2/prepare_v2/step/finalize`; usa **frida-gum puro, sin java-bridge**; escribe NDJSON **redactado** | `crates/hooks-payload/src/{hooks.rs,README.md,sink.rs}` |
| V46 | `state/` **no** está excluido en bloque por `.gitignore` ⇒ un fichero nuevo de estado **acaba en GitHub** | `.gitignore:9-22` |
| V47 | **Sin PII** en este documento (NIF, nombre, teléfono, línea y correo enmascarados) | `grep` sobre el doc |
| V48 | **El `goto` (formato 10t) ocupa 1 code unit, no 2.** Con 2, sólo el **63,4 %** de los destinos de salto cae en una frontera de instrucción; con 1, el **100,0 %** (183.049/183.049). `dex_decompile::instruction_width` y `apk_deobfuscator::instruction_width` lo tienen mal ⇒ **el desensamblado del repo se desalinea tras cada `goto`** | decodificador propio + 2 invariantes sobre 50.220 `code_item` |
| V49 | **En 35c el índice de método está en `unit1`**: con `unit1` la firma del callee coincide en el **100,0 %** de las 88.600 invocaciones con argumentos; con `unit2`, en el 39,3 %. `apk_deobfuscator::decode_insn` además lee el índice de tipo (22c `new-array`/`instance-of`) y de campo (22c `iget`/`iput`) de `unit2` en vez de `unit1` ⇒ **índices basura** | test de coincidencia de firma sobre 2 DEX |
| V50 | **En 22b el literal va en el byte ALTO de `unit1` y el registro B en el BAJO**: con la convención contraria sólo el 53,3 % de las instrucciones tiene registros dentro de `registers_size`; con la correcta, el 100 % | test de `registers_size` sobre 48.781 instrucciones |
| V51 | `LQfgSZK/HZWRvS;->fsw` es el **descifrador de strings** de la app: se llama **2.234 veces**; su clave es un `char[32767]` construido **en runtime** con predicados opacos ⇒ descifrado estático imposible sin intérprete | grafo de llamadas + desensamblado |

## D.2 (b) INFERIDO — deducción razonada, no medida

| # | Inferencia | En qué se apoya | Riesgo |
|---|---|---|---|
| I1 | La app cachea el consumo en `no_backup/` (20,4 MiB) en formato legible | Un `no_backup` de 20 MiB en una app de operador ⇒ caché de sesión y de datos | **Alto** ⇒ por eso la caché es fuente #2, no la #1 |
| I2 | `consumption-api/…/summary` es el que pinta "Mi consumo" | Es el único que devuelve `UnifiedDataConsumptionSummary` con `consumed`/`remaining`/`included` | Medio |
| I3 | `consumptionType` distingue "unlimited" y su umbral de 50 GB | La web de Jazztel documenta el umbral de 50 GB a velocidad máxima | **Alto** ⇒ hay que leer el valor real (M8) |
| I4 | `NetworkEventType`/`NetworkCostType` traen el coste en euros por evento ⇒ el estimador puede usar la API en vez de la factura | `BigDecimal` en el descriptor | Medio |
| I5 | La app ya calcula el desglose por app con datos de sistema ⇒ puede haber un endpoint "activity-detail" con detalle fino | `activity-detail` + `ActivityDetailsLoadedAction` | Medio |
| I6 | El pin se verifica en código (OkHttp `CertificatePinner` o propio) ⇒ se puede hookear | `stamp-cert-sha256` como asset | Medio ⇒ M14 lo confirma |
| I7 | `atd` es un módulo de protección de código (ATD = *app tampering detection* / *anti tamper device*) | payload cifrado con prefijo `java/lang/Object` | Medio |
| I8 | El motor LLM local SEERE es el que menos tokens gasta ⇒ debe ser el primero para el "ahorro" | `provider_catalog.rs` (tier `offline`) | Bajo |
| I9 | La pantalla de Samsung es `com.android.settings` + personalización ⇒ hookear `system_server` la cubre | La guía del SM-G998U y la guía de Ajustes coinciden | Medio ⇒ M9 lo confirma |
| I10 | La app firma con 3 certificados ⇒ hay linaje (rotación de clave) ⇒ el pin puede cambiar entre versiones | bloque v2 con 3 certs | Medio ⇒ leer el pin del APK instalado, no del de Drive |

## D.3 (c) NO VERIFICADO — cómo verificarlo en el móvil

Todos por `root_exec` (tool existente). ⚠️ **Tope de 1024 caracteres de salida**
⇒ lotes o volcado a fichero.

**C1 — legacy o eBPF**
```bash
su -mm -c 'ls -d /proc/net/xt_qtaguid 2>&1; uname -r; getprop ro.build.version.sdk'
```
*Esperado*: `No such file or directory` + kernel 5.x ⇒ **eBPF**. La versión exacta
del kernel **no** está medida en el repo: este comando la mide.

**C2 — trafficController**
```bash
su -mm -c 'dumpsys netd | head -40'
su -mm -c 'dumpsys netd trafficcontroller | head -120'
```

**C3 — agregado por UID**
```bash
su -mm -c 'dumpsys netstats | head -150'
su -mm -c 'dumpsys netstats detail > /data/local/tmp/megadaemon/netstats_detail.txt'
su -mm -c 'wc -l /data/local/tmp/megadaemon/netstats_detail.txt'
```

**C4 — app Jazztel instalada (UID, versión, rutas, permisos)**
```bash
su -mm -c 'dumpsys package com.orange.bluu.jazztel | grep -m1 userId='
su -mm -c 'dumpsys package com.orange.bluu.jazztel | grep -E "versionName|versionCode" | head -4'
su -mm -c 'pm path com.orange.bluu.jazztel'
su -mm -c 'dumpsys package com.orange.bluu.jazztel | grep -A5 "requested permissions"'
```

**C5 — caché local de la app**
```bash
su -mm -c 'ls -la /data/user/0/com.orange.bluu.jazztel/no_backup/ 2>&1 | head -30'
su -mm -c 'ls -la /data/user/0/com.orange.bluu.jazztel/databases/ 2>&1'
su -mm -c 'ls -la /data/user/0/com.orange.bluu.jazztel/shared_prefs/ 2>&1'
su -mm -c 'ls -la /data/user/0/com.orange.bluu.jazztel/files/ 2>&1 | head -20'
```

**C6 — interceptar el tráfico (hooks existentes)**
```bash
# tool inject_hooks { "action":"inyectar", "package":"com.orange.bluu.jazztel" }
# abrir la app y navegar a "Mi consumo"
su -mm -c 'grep -a getaddrinfo /data/local/tmp/megadaemon/eventos.ndjson | tail -30'
su -mm -c 'grep -a "\"event\":\"connect\"" /data/local/tmp/megadaemon/eventos.ndjson | tail -30'
su -mm -c 'grep -a "\"hook\":\"ficheros\"" /data/local/tmp/megadaemon/eventos.ndjson | tail -40'
```

**C7 — esquema real, sin adivinar**
```bash
su -mm -c 'grep -a "\"hook\":\"sqlite\"" /data/local/tmp/megadaemon/eventos.ndjson | tail -40'
```

**C8 — tethering**
```bash
su -mm -c 'ip -br link | grep -iE "ap|wlan|usb"'
su -mm -c 'dumpsys tethering | head -40'
```

**C9 — Samsung: ¿provider propio? ¿network security config?**
```bash
su -mm -c 'dumpsys package providers | grep -iE "data|usage|traffic" | head -20'
su -mm -c 'ls -la /data/user/0/com.android.providers.telephony/databases/ 2>&1'
su -mm -c 'dumpsys package com.android.providers.telephony | grep -iE "versionName|codePath" | head -5'
```

**C10 — ciclo local de Android (lo que hay que sustituir)**
```bash
su -mm -c 'settings list global | grep -iE "data_usage|data_saver" | head -20'
su -mm -c 'dumpsys netpolicy | head -40'
```

**C11 — formato real de la factura (en el PC, nombre enmascarado)**
```bash
python3 - <<'PY'
from pypdf import PdfReader
r = PdfReader("<factura.pdf>")
print("\n".join(p.extract_text() or "" for p in r.pages))
PY
```
*Interesa*: cómo escribe Jazztel los megabytes y **dónde aparece el "Cargo por
gestión de cobro"* ⇒ alimenta el parser del estimador.

**C12 — la API con la sesión real** (fuera del móvil, con cuidado)
Capturar **una** petición a `consumption-api/subscriptions/{id}/summary` con el
navegador en modo red ⇒ cabeceras y forma real del JSON. **No subir las
cabeceras a ningún sitio.** El daemon no debe llamar la API hasta que se sepa el
contrato **y** haya almacenamiento cifrado de credenciales (§C.5).

**C13 — versión y pin del APK INSTALADO** (puede diferir del de Drive)
```bash
su -mm -c 'ls -l /data/app/*/com.orange.bluu.jazztel*/base.apk'
# extraer el asset del APK instalado (el pin puede haber rotado)
su -mm -c 'unzip -p /data/app/*/com.orange.bluu.jazztel*/base.apk assets/stamp-cert-sha256 | xxd -p'
```
⇒ contrasta con `3257d599…` (I10).

**C14 — ¿el hook en `system_server` sirve?**
```bash
su -mm -c 'ps -A -o PID,ARGS | grep -a system_server | head -1'
# inyectar el payload java-bridge y abrir Ajustes → Conexiones → Uso de datos
su -mm -c 'dumpsys activity service ioctl 2>/dev/null | head -5'
```

## D.4 Correcciones a los encargos anteriores

1. **Unidad del `no_backup`**: "20.854 B" ⇒ son **20.854 KB (20,4 MiB)**.
2. **`/proc/net/xt_qtaguid_ctrl`** no es la ruta actual (pre-Android-4); hoy es un
   **directorio**, y en Android 9+ **no existe**.
3. **No hay provider Samsung de consumo verificado** (V38).
4. **El APK NO estaba en el workspace**: ahora sí, descargado y verificado (V6).
5. **El recargo por mora no es de 39 €**: es 15,99 €/12 € + 11,99 € de
   reconexión (V39-V41). El "39/40 €" es de la Ley 3/2004 (entre empresas).
6. **La app no está empaquetada ni cifra strings** (V11-V13) ⇒ la RE "a saco" es
   un **renombrado + reconstrucción del contrato**, no una máquina de descifrar.
7. **PII**: los nombres de factura de Drive llevan el NIF ⇒ nunca en el repo;
   el NIF y el número de línea se enmascaran siempre (V47).
8. **"La app NO cifra strings" (punto 6) era una conclusión prematuramente cerrada**:
   la app **sí** cifra strings, pero con un descifrador propio
   (`LQfgSZK/HZWRvS;->fsw`, 2.234 llamadas) cuya clave es un `char[32767]`
   construido en runtime (V51). Lo que no hay es un SDK de cifrado de strings
   *de terceros*: la fase 2 del detector encontró los 200 candidatos y todos eran
   3DS2, pero **el descifrador de la propia app no encajaba en el patrón
   buscado**. La conclusión correcta es: *"no hay cifrado de strings de
   protector comercial; sí hay un descifrador propio de la app"*.
9. **El desensamblado del repo estaba desalineado**: `goto` ocupa 1 code unit,
   no 2 (V48), y en 22c/35c varios índices se leen del code unit equivocado
   (V49, V50). Cualquier xref o endpoint extraído antes de esta medición puede
   estar desplazado.

---

## D.5 Verificación pendiente en la build (no requiere móvil)

Estos pasos **no** son `root_exec`: son de repositorio. Los dejo escritos para
que el root los ejecute al integrar, porque el módulo `re_deep.rs` **no está
compilado** (prohibido en este entorno) y su primera verificación es un
`cargo check`.

**B1 — compilar y auditar**
```bash
cd /workspace/md-root
# 1) comprobar que el módulo (ya declarado en lib.rs) compila
cargo check -p megadaemon-core
# 2) auditar un DEX real: el veredicto debe ser "ok"
#    (los tres ratios >= 99 %)
cargo test -p megadaemon-core re_deep
```

**B2 — qué hacer si la auditoría sale `degradado`**

| Ratio bajo | Causa probable | Revisar |
|---|---|---|
| `encaje_bloques` | anchura mal asignada | `dalvik_width()` en `re_deep.rs` frente a `dex_decompile::instruction_width` |
| `alineacion_saltos` | `goto` con 2 unidades, o payload mal saltado | `dalvik_width()` y `decode_code()` (bloque de payloads) |
| `encaje_firmas` | índice de método en el code unit equivocado | la rama `"35c"` de `decode_code()` |

**B3 — la ambigüedad del word de registros de `35c`**

Quedó **pendiente de cerrar** (§E.3.3): el test global favorece el nibble bajo
(25,0 % vs 0,8 %) pero el caso `fsw` exige el nibble alto (el receptor de
`String.length()` tiene que ser el registro 0, que es el parámetro). La
resolución robusta es **no fijar la convención**: probar las dos y quedarse
con la que produce una ejecución que termina (§E.4, capa 2). Hasta que se
cierre, el punto queda marcado como `NO VERIFICADO`.

---

# PARTE E — Motor de deofuscación: investigación, hallazgos medidos y diseño

> Encargo: *«investiga si ya hay literatura/guías/programas que lo consigan, e
> implementa masivamente un sistema muy potente, robusto y versátil, en Rust,
> integrado en MegaDaemon»*.
>
> Esta parte no sustituye a las partes A–D (siguen siendo el encargo de fondo:
> sustituir el subsistema de datos de Android con las cifras del operador).
> Es la **infraestructura** que hace falta para que cualquier análisis de APK
> del daemon sea fiable, y es lo que salió de la RE real de la app Jazztel.

---

## E.1 Resumen: qué se ha hecho y qué queda

| Entregable | Estado | Dónde |
|---|---|---|
| Investigación del estado del arte (literatura + herramientas) | **HECHO** | §E.2 |
| **Tres bugs de decodificación Dalvik medidos en el repo** | **HECHO** | §E.3 |
| Decodificador Dalvik validado por invariantes (Rust) | **HECHO** (sintaxis validada, sin compilar) | §E.5, `rust-core/src/re_deep.rs` |
| Autocalibración (el motor se audita a sí mismo) | **HECHO** | §E.3.1, §E.5 |
| CFG + detección de predicados opacos | **HECHO** | §E.5 |
| Descifrado en masa de strings (stub → call sites → intérprete) | **DISEÑO** (el intérprete existe; falta el pegamento) | §E.4 |
| Capas 4–7 (MBA, empaquetadores, nativo, salida) | **DISEÑO** | §E.4 |
| Integración en MegaDaemon (MCP, cockpit, autopilot) | **DISEÑO** | §E.6 |

**El hallazgo que da valor a todo lo demás**: el desensamblado del repo
—`dex_decompile` y `apk_deobfuscator`— **desalinea el código a partir de cada
`goto`**, y confunde el índice de campo/tipo en media docena de formatos. No
es un detalle estético: significa que cualquier xref, ruta o string extraído
por el daemon puede estar **desplazado** respecto de la realidad. Esto se
midió, no se supone (§E.3).

---

## E.2 Estado del arte

### E.2.1 Android / Dalvik: lo que dice la literatura

| Trabajo | Qué aporta al diseño |
|---|---|
| Dong et al., *Understanding Android Obfuscation Techniques: A Large-Scale Investigation in the Wild* (arXiv:1801.01633) | Taxonomía de referencia: **renombrado**, **cifrado de strings**, **reflexión**, **empaquetado**. Concluye que el control de flujo es *no universal* (sólo DashO y Allatori) y que en muchos casos es débil ⇒ el motor debe priorizar cifrado de strings y renombrado, no *flattened control flow* |
| Hou et al., *Android Code Protection via Obfuscation Techniques* (arXiv:1611.10231) | Inventario de CFF y de *data aggregation*; tabla de malware real por técnica ⇒ sirve de checklist de qué detectar |
| Balachandran et al., *Control Flow Obfuscation for Android Applications* (Computers & Security 61, 2016) | Los **tres esquemas a nivel Dalvik**: `packed-switch`, `try/catch` y su combinación. Y la lista de 8 medidas: reempaquetado de clases, cifrado de literales, cifrado de clases/nativas/assets, *split* de `.dex`, predicados opacos + código basura, empaquetado… ⇒ es la especificación del detector |
| Zhang et al., *Android application forensics: a survey of obfuscation, detection and deobfuscation techniques* (Forensic Sci. Int. Digital Investigation 39, 2021) | Métodos de **detección**: entropía, tamaño de palabra, nº de caracteres especiales para cifrado; y para flujo, métricas de CFG (nodos, *sinks*, aristas) ⇒ el motor debe calcular CFG siempre |
| AndrODet (Mirzaei et al., FGCS 90, 2019) | Detección **adaptativa** con aprendizaje online de renombrado / cifrado / flujo ⇒ el clasificador del motor debe poder entrenarse con los APK del propio usuario |
| *Characterizing the Use of Code Obfuscation in Malicious Android Apps* (ACM, 2023) | Dato operativo: el **cifrado de strings es la causa dominante** de *targets* de llamada irresolubles ⇒ el motor debe descifrar strings **antes** de intentar resolver llamadas |
| *An Empirical Study of Code Obfuscation Practices* (arXiv:2502.04636) | Escala: sobre **548.967 APKs** de Google Play, 81,04 % usa modificación de control de flujo y 62,76 % cifrado de strings ⇒ el caso "app normal" ya viene ofuscado |
| *Evaluating LLMs for Obfuscation Detection and Classification in Android Apps* (arXiv:2606.14233) | Un LLM clasifica bien (**92,02 %** renombrado, **81,41 %** cifrado de strings) pero peor el control de flujo (**68,32 %**) ⇒ el LLM es un **clasificador**, no un decodificador. Encaja con §C.11: el LLM prioriza, el código numérico decide |
| DexMonitor (IEEE Access 6, 2018) | Monitorización **dinámica**: extraer los strings descifrados en runtime y reinyectarlos ⇒ el plan B del §C.4 (hook) tiene respaldo publicado |
| ReDex (optimizador) | Revierte el CFF de Obfuscapk en **1089/1108** apps ⇒ el CFG + detección de predicados opacos es suficiente para deshacer *flattening* ligero |

### E.2.2 Nativa (ELF/PE) y virtualización

| Trabajo / herramienta | Qué aporta |
|---|---|
| **Tigress** (C, *source-to-source*: *data encoding*, CFF, predicados opacos, virtualización) — <https://tigress.wtf> | El estándar académico. Su manual es, de hecho, la especificación más completa de "qué puede hacer un ofuscador" ⇒ sirve como checklist para el detector nativo |
| **Obfuscator-LLVM** (OLLVM) | CFF a nivel IR; se detecta por el *dispatcher* con una variable de estado ⇒ el mismo detector de CFG sirve aquí, cambiando el CFG por el de funciones |
| Pushan (arXiv:2603.18355) | Devirtualización **sin traza** con emulación simbólica *VPC-sensitive*; **988/1000** muestras Tigress ⇒ el objetivo de un motor de nativo es "reconstruir el CFG completo", no "desempaquetar" |
| VMSpec (arXiv:2610.29566) | Genera specs **SLEIGH** para VMs desconocidas con *symbolic path exploration*; **918/918** muestras Tigress ⇒ la estrategia "especificación de procesador desconocida" es la vía genérica |
| VMPredator (arXiv:2605.30902) | Comercial (VMProtect/Themida): se ancla en la terna **⟨VM entry, VM exit, handlers⟩** ⇒ heurística: buscar la *ancla semántica*, no la estructura exacta del protector |
| *Static Detection of Core Structures in Tigress Virtualization* (arXiv:2601.12916) | Detección estática (dispatch routine, handler blocks, VM region) en LLVM IR ⇒ aplicable a `.so` con un pass equivalente |
| SoK: *Automatic Deobfuscation of Virtualization-protected Applications* | Evalúa Virtual Deobfuscator, VMAttack, VMHunt y Tigress DeObf: ninguna es universal ⇒ **el motor debe ser modular y reportar qué técnica encontró, no "desofuscar"** |
| Herramientas: D-810 (IDA), GOOMBA (Ghidra), Miasm, Ghidra, Binary Ninja, MOPSA | Deofuscadores nativos existentes; el motor no los sustituye, los **orquesta** |

### E.2.3 Herramientas que ya existen: qué se copia de cada una

| Herramienta | La idea que se copia al motor Rust |
|---|---|
| **jadx** + `Arsylk/jadx-string-decrypt` | *Const folding* de expresiones opacas y descifrado de cifrador de bloque cuando es resoluble (descriptor `([B)Ljava/lang/String;`, AES/ECB/PKCS5Padding, clave = últimos N bytes del blob). ⇒ El motor debe (a) plegar constantes y (b) tener un descifrador de bloque genérico con "clave al final del blob" |
| **java-deobfuscator** (`java-deobfuscator-detect` + transformadores) | Arquitectura de *detect → transformer por protector*: un `StringEncryptionTransformer` por familia ⇒ el motor Rust debe exponer `detect` y transformadores con nombre, no un único `deobfuscate` |
| **simplify** (Facebook) | Deofuscación **por VM**: ejecutar el código y observar. Es la misma filosofía que el intérprete Dalvik de `re_deep` |
| **dex2jar** + `d2j-decrypt-string` | Descifrado de strings **desde el lado DEX**, sin configuración ⇒ el motor debe poder trabajar sobre el DEX, no sobre Java |
| **DaliVM** (emulador Dalvik en Python) | Recuperar valores **sólo disponibles en runtime** sin arrancar Android: enumera *call sites* en multi-DEX y reconstruye argumentos con *backward data-flow tracing* + *forward lookup* + *partial execution* ⇒ **es el diseño exacto del intérprete de `re_deep`**; la diferencia es que DaliVM lo hace genérico y publica el corpus de casos |
| **APKiD** | Identificación de empaquetador por *fingerprint* (clases sospechosas, `Application` custom, tamaño de DEX, entropía de assets) ⇒ el motor debe tener una tabla de fingerprints |
| **Quark-Engine**, **frida-dexdump**, **DexHunter** | Descifrado de malware y volcado de DEX **de memoria** ⇒ el motor necesita un "modo dinámico" que use el `inject_hooks` del daemon (§C.3) |
| **Obfuscapk** | Ofuscador open source **y** deofuscador ⇒ sirve de corpus de pruebas para el motor |
| hacktricks, *Manual De-obfuscation Techniques* | El patrón "identifica el método de descifrado y **replica** sus pasos" ⇒ el motor debe poder **extraer la clave** y reimplementar el algoritmo, no sólo ejecutarlo |
| hacktricks / android-security-handbook §40 | Truco de detección casi infalible: **hookear `java.lang.String.<init>([B)` y `([C)`** ⇒ todo descifrador termina construyendo un `String`, así que ese hook captura el 100 % de los descifradores, sea cual sea el algoritmo |
| Checklist de android-security-handbook §40 | Detección rápida: `strings classes.dex \| grep -E 'https?://'` (si no hay URLs, hay cifrado), entropía de `assets/`, `<application android:name>` custom, `DexClassLoader`/`InMemoryDexClassLoader` ⇒ el motor debe exponer esto como `triaje` de 1 comando |

---

## E.3 Hallazgo: tres bugs de decodificación **medidos** en el repo

### E.3.1 El método: calibrar el decodificador con invariantes

Un decodificador Dalvik no se puede validar leyendo la spec (las convenciones
de bits son ambiguas y las fuentes se contradicen). Se calibra con **invariantes
que el código real tiene que cumplir sí o sí**. Se usaron tres, independientes:

1. **Encaje de anchuras**: las anchuras de las instrucciones deben sumar
   exactamente `insns_size` del `code_item`.
2. **Alineación de saltos**: todo destino de salto debe ser el **inicio** de una
   instrucción (si las anchuras están mal, los destinos caen dentro de otra
   instrucción).
3. **Coincidencia de firmas**: el número de argumentos de un `invoke` debe
   coincidir con la **firma del método invocado** (contando el receptor y
   ocupando 2 registros los parámetros `long`/`double`).

El tercer invariante es el que zanja las ambigüedades de bits, porque no
depende de la convención: si el índice de método está en el code unit
equivocado, la firma no casará.

### E.3.2 Los tres bugs

| # | Módulo | Qué hace mal | Medición |
|---|---|---|---|
| **1** | `dex_decompile::instruction_width` y `apk_deobfuscator::instruction_width` | `goto` (formato `10t`) ocupa **2** code units; ocupa **1** | Con 2: sólo el **63,4 %** de los destinos de salto cae en una frontera de instrucción. Con 1: **100,0 %** (183.049/183.049) |
| **2** | `apk_deobfuscator::decode_insn` | `new-array`/`instance-of` (22c) leen el índice de tipo de `unit2`; está en **`unit1`** | El índice de `new-array` daba 228.162 (basura) en vez del descriptor `[C` |
| **3** | `apk_deobfuscator::decode_insn` | `iget`/`iput` (22c) leen el índice de campo de `unit2`; está en **`unit1`**. Además en `35c` intercambia el nº de argumentos y el registro `G` (`argc = unit0>>8` en vez de `unit0>>12`) | Con el índice en `unit1` la firma del callee coincide en el **100,0 %** de las 88.600 invocaciones con argumentos; con `unit2`, en el 39,3 % |

**Consecuencia**: `apk_deobfuscator::scan` y `dex_decompile` producían, hasta
ahora, xrefs y endpoints **potencialmente desplazados**. No es que faltara
información: es que estaba *en el sitio equivocado*.

### E.3.3 Tabla de convenciones medidas (la que usa el motor)

| Formato | Convención | Cómo se comprobó |
|---|---|---|
| `12x`, `11n`, `22t`, `22s`, `22c` | A = bits 8-11, B = bits 12-15 | `new-array v3, v0, [C`: A debe ser el destino (3) y B el tamaño (32767) — con la convención opuesta el `sput-object` guarda un entero en vez del array |
| `23x` | A = byte 1 (8 bits), B = byte 2, C = byte 3 | `aget-char v6, v2, v3` ⇒ v6 = v3[v2] (array, índice) |
| `22b` | **literal en el byte ALTO** de `unit1`, registro B en el byte BAJO | Con la convención contraria sólo el **53,3 %** de las instrucciones tiene registros dentro de `registers_size`; con la correcta, el **100 %** |
| `22c` | índice (tipo/campo/método) en `unit1` | Heredado del bug #2: con `unit2` el índice es basura |
| `35c` | nº de args = bits 12-15 de `unit0`; G = bits 8-11; índice de método en `unit1`; word de registros en `unit2` | Coincidencia de firmas: 100,0 % con `unit1` |
| `10t` | **1** code unit; offset con signo en el byte 1 de `unit0` | Alineación de saltos: 100,0 % |
| `31i` | 3 code units | Encaje de anchuras: 100 % |

> Nota de honestidad: la lectura del **word de registros** de `35c` (nibble alto
> vs bajo para el primer argumento) quedó **pendiente de cerrar**: el test
> global de coherencia da 25,0 % al nibble bajo y 0,8 % al alto, pero el caso
> `fsw` de la app Jazztel exige que el receptor de `String.length()` sea el
> registro 0 (el parámetro), lo que apunta al nibble alto. Se resuelve
> **en tiempo de ejecución** por el motor (ver §E.4, capa 2: el motor prueba
> las variantes y se queda con la que sobrevive), no por una constante fija.
> Marcado como `NO VERIFICADO` en §D.3.

---

## E.4 Arquitectura del motor

### E.4.1 Las siete capas

```
   APK / DEX / ELF / PE / recursos / peticiones de red
                        │
  ┌─────────────────────▼──────────────────────────────────────────┐
  │ 1. INGESTA      ZIP → AXML → DEX → recursos → .so → assets      │
  │                (reutiliza apk_inventory + apk_deobfuscator)      │
  └─────────────────────┬──────────────────────────────────────────┘
                        │
  ┌─────────────────────▼──────────────────────────────────────────┐
  │ 2. DECODIFICADOR VALIDADO + AUTOCALIBRACIÓN                      │
  │    decode_code()  ·  audit_decode()  ← invariantes E.3.1         │
  │    Si la auditoría falla, el motor PRUEBA VARIANTES y avisa.      │
  │    Nunca devuelve datos sin auditar.                             │
  └─────────────────────┬──────────────────────────────────────────┘
                        │
  ┌─────────────────────▼──────────────────────────────────────────┐
  │ 3. CFG + PREDICADOS OPACOS                                       │
  │    build_cfg()  ·  opaque_branches()                             │
  │    → detecta *flattening* (muchos bloques → 1 dispatcher)         │
  │    → detecta *dead code* (rama con condición constante)           │
  └─────────────────────┬──────────────────────────────────────────┘
                        │
  ┌─────────────────────▼──────────────────────────────────────────┐
  │ 4. FOLDING DE CONSTANTES / MBA                                   │
  │    Evalúa expresiones mixtas booleanas y aritméticas hasta un    │
  │    entero; si no puede, marca "opaco" y sigue.                    │
  │    (Es lo que hace jadx-string-decrypt con su const-folder.)      │
  └─────────────────────┬──────────────────────────────────────────┘
                        │
  ┌─────────────────────▼──────────────────────────────────────────┐
  │ 5. DESCIFRADO DE STRINGS EN MASA                                 │
  │    5a. detectar stubs  (devuelven String, con XOR/char[]/AES)    │
  │    5b. enumerar call sites en TODOS los DEX                      │
  │    5c. extraer la clave (backward data-flow, estilo DaliVM)       │
  │    5d. ejecutar el stub con el intérprete → tabla de strings      │
  │    5e. fallback: cifrado de bloque con clave al final del blob    │
  └─────────────────────┬──────────────────────────────────────────┘
                        │
  ┌─────────────────────▼──────────────────────────────────────────┐
  │ 6. HUELLA DE EMPAQUETADOR / PROTECTOR / ANTI-ANÁLISIS              │
  │    APKiD-like, entropía de assets, loaders dinámicos,             │
  │    pin vs certificados de firma, anti-debug, RASP,                │
  │    y para nativo: OLLVM/Tigress/VMProtect/UPX.                   │
  └─────────────────────┬──────────────────────────────────────────┘
                        │
  ┌─────────────────────▼──────────────────────────────────────────┐
  │ 7. SALIDA                                                        │
  │    informe JSON · tabla de strings · reescritura de DEX ·         │
  │    hooks dinámicos para el móvil (Frida) · alertas del daemon    │
  └─────────────────────────────────────────────────────────────────┘
```

### E.4.2 Por qué una sola pasada no basta

El caso real de Jazztel lo demuestra: el stub `LQfgSZK/HZWRvS;->fsw` se llama
**2.234 veces**, su clave es un array `char[32767]` **construido en runtime** y
el código que lo construye está relleno de predicados opacos. Sin el
intérprete (capa 5d), la clave es indescifrable estáticamente; sin la capa 3, ni
siquiera se ve que el bucle de inicialización es en parte código muerto.

⇒ **El motor tiene que ser un bucle, no una tubería**: auditar → decodificar →
plegar → ejecutar → re-auditar. Cada capa usa el resultado de la anterior.

### E.4.3 Lo que el motor **no** va a hacer (límite honesto)

| Limitación | Por qué |
|---|---|
| Descifrar un cifrado de bloque cuya clave depende del hardware (TEE, `attest`) | La clave no está en el APK |
| Devirtualizar un protector comercial sin ejecutar | Pushan y VMSpec necesitan el binario y sólo funcionan con VMs *reconocibles* |
| Reconstruir un APK **bit a bit** | Clases cifradas, assets cifrados y lógica en el servidor |
| Que un LLM descifre | Medido: el LLM clasifica (81 % en cifrado de strings) pero no calcula claves |

---

## E.5 API Rust (contrato del módulo)

Todo en `rust-core/src/re_deep.rs`, **sin dependencias nuevas** (usa `serde_json`,
`sha2` y el crate `dex_decompile` del propio daemon).

| Función | Qué hace | Devuelve |
|---|---|---|
| `dalvik_width(op: u8) -> usize` | Ancho **medido** de un opcode | unidades de 16 bits |
| `decode_code(insns: &[u16]) -> Vec<DecodedInsn>` | Decodifica un `code_item` con las convenciones medidas y los payloads | instrucciones con operandos, literales, refs y destinos de salto |
| `audit_decode(dexes: &[&DexFile]) -> DecodeAudit` | **Autocalibración**: encaje de anchuras, alineación de saltos, coincidencia de firmas | ratios + veredicto (`ok` / `degradado: …`) |
| `build_cfg(insns: &[u16]) -> Vec<BasicBlock>` | Grafo de control por bloques básicos | bloques con sucesores |
| `opaque_branches(insns: &[u16]) -> Vec<OpaqueBranch>` | Predicados opacos por propagación de constantes | ramas con condición constante (siempre se toma / nunca) |
| `interpret_method(dex, class, method) -> Result<Value, ExecError>` | Ejecuta un método en análisis (ya existía) | valor o error con el punto exacto |

Lo que **falta** para cerrar las capas 4–7 (y que es trabajo disponible, no una
imposibilidad):

| Pieza | Tamaño estimado | Nota |
|---|---|---|
| `fold_constants(code) -> Vec<FoldedInsn>` | ~250 líneas | plegado de MBA con `eval_2addr` ya escrito |
| `find_decrypt_stubs(dex) -> Vec<Stub>` | ~200 líneas | reutiliza `decryptor_candidates` + el discriminador cripto |
| `decrypt_all(dexes, stub) -> BTreeMap<String, String>` | ~300 líneas | enumerar call sites + `interpret_method` + fallback AES/ECB |
| `fingerprint_packers(dexes, assets) -> Vec<Finding>` | ~200 líneas | APKiD-like |
| `native_obfuscation_fingerprint(elf) -> Vec<Finding>` | ~250 líneas | OLLVM/Tigress/VMProtect/UPX |
| `rewriter` (reescribir el DEX con los strings descifrados) | ~400 líneas | requiere escribir un DEX ⇒ el repo ya tiene AXML, falta el escritor de DEX |

---

## E.6 Integración en MegaDaemon

El motor **no es una islanda**: cada capa aterriza en un punto del daemon ya
existente.

| Capa | Punto de integración | Qué gana el usuario |
|---|---|---|
| 1–2 | `apk_inventory` / `apk_deobfuscator::scan` | El inventario deja de poder estar **desalineado** |
| 2 | `audit_decode` como **gate** del escáner | Si la auditoría sale `degradado`, el informe lo dice en vez de mentir |
| 3 | `apk_deobfuscator::build_call_graph` | El grafo de llamadas pasa a usar el CFG real y detecta *flattening* |
| 5 | `re_deep::decryptor_candidates` → tabla de strings | Los endpoints y rutas salen **en claro**, no cifrados |
| 5 | `cortex/cockpit` (listado de apps del móvil) | Un APK protegido se detecta **en el móvil**, no sólo en el escritorio |
| 6 | `alerts.rs` | Alerta de "app con cifrado de strings / empaquetador / anti-debug" |
| 7 | `hooks-payload` / `inject_hooks` (Frida) | Modo dinámico: hookear `String.<init>([B)` y `([C)` para cazar **todos** los descifradores |
| 7 | `state/gasto_datos.sqlite` | Persistencia de los strings descifrados (con la máscara de PII de §C.5) |

**MCP**: la tool `re_profunda` (ya existe, §C.13) gana acciones nuevas:
`auditar`, `cfg`, `opacos`, `descifrar`, `empaquetadores`. Todas devuelven
JSON y **todas pasan por `audit_decode` antes de publicar nada**.

**El guard de §C.5 se mantiene**: los strings descifrados pueden contener PII
(NIF, número de línea) ⇒ van enmascarados antes de escribir, y nunca al repo.

---

## E.7 Cobertura de ecosistemas

| Ecosistema | Estado del motor | Técnica |
|---|---|---|
| DEX / Dalvik (R8, ProGuard) | **Cubierto** | decodificador validado + CFG + folding |
| DEX cifrado por protector (Stringer, Allatori, DashO, DexGuard, DIVILAR, DexProtector, Bangcle, Virbox) | **Parcial** | stubs detectados; el descifrado en masa depende del algoritmo |
| ELF / PE nativo (OLLVM, Tigress) | **Detectado, no devirtualizado** | fingerprints + CFG de funciones |
| ELF / PE virtualizado (VMProtect, Themida, Code Virtualizer) | **No** (necesita ejecución) | ver §E.4.3 |
| Empaquetadores (UPX y equivalentes Android) | **Fingerprint** | magic + entropía + stubs |
| Recursos (assets cifrados, `resources.arsc` cifrado) | **Fingerprint + loader** | entropía + búsqueda del loader |
| Red (TLS pinning, JA3, QUIC) | **Fuera del motor** (es §C.4) | pin vs firma ya implementado en `re_deep::pin_report` |

---

## E.8 Fases, riesgos y orden de trabajo

| Fase | Qué | Riesgo |
|---|---|---|
| **F1** *(hecha)* | Decodificador validado + auditoría + CFG + opacos | Bajo: es autocontenido y auditable |
| **F2** | Folding de constantes + detección de stubs | Bajo |
| **F3** | Descifrado en masa (intérprete) + tabla de strings | Medio: hay que resolver la ambigüedad del word de registros de `35c` (§E.3.3) |
| **F4** | Fingerprints de empaquetador/protensor + nativo | Medio |
| **F5** | Reescritura de DEX con los strings en claro | Alto: escribir un DEX válido no es trivial |
| **F6** | Modo dinámico (hooks Frida) integrado en `inject_hooks` | Medio: necesita el payload con *java-bridge* (§C.3) |

**El riesgo que no se puede eliminar**: `re_deep.rs` **no se puede compilar**
en este entorno (prohibido y sin recursos). Está validado con `rustfmt`
(sintaxis: el gate `tools/check-sintaxis.sh` del CI lo pasa) y con revisión
estática de tipos, pero **puede haber errores de tipos que sólo aparezcan al
compilar**. El módulo **ya está declarado en `lib.rs`** (lo hizo el root), así
que `cargo build` sí lo compila ⇒ el primer paso al integrar es `cargo check`;
después, `audit_decode` sobre un APK real. Si la auditoría sale `degradado`,
la tabla de convenciones (§E.3.3) es lo primero que hay que revisar.

---
---

## Referencias del repo

* **RE / APK*: `rust-core/src/dex_decompile.rs` (descompilador DEX propio:
  `parse_dex`, `analyze_obfuscation`, `decompile_class`, `dex_strings`,
  `obfuscation_findings`), `apk_inventory.rs` (escáner: manifiesto, permisos,
  endpoints, firma, native libs), `apk_deobfuscator.rs`, `app_intel.rs:750`
  (`extract_endpoints`), `axml_parser.rs`.
* **Hooks / inyección*: `rust-core/src/hooks_bridge.rs:179` (tool
  `inject_hooks`), `crates/hooks-payload/src/{hooks.rs,plan.rs,sink.rs,redact.rs}`,
  `crates/injector-payload/src/lib.rs`.
* **Comando y auth*: `rust-core/src/mcp_ext.rs:954`, `android.rs:1303,1123`,
  `cockpit_server.rs:2632`, `api_key.rs:473,618`,
  `antigravity_intel.rs:77-79`, `alerts.rs:85`, `state_dir.rs:17`,
  `cert_android.rs:59-68`, `magisk_module.rs`, `sqlite_mitm.rs`,
  `deploy/magisk-mitm-sqlite/`.
* **LLM*: `rust-core/src/provider_catalog.rs:37` (`tier_free`),
  `llm_gateway.rs` (rutas Gemini y proveedores), `antigravity_endpoint.rs`,
  `antigravity_model.rs`.
* **Contexto medido*: `rust-core/src/call_recorder.rs:1609-1610`,
  `docs/PHONE_INVENTORY.md:22,409`, `.gitignore:9-22`.
* **Método (no reescribir)*: `docs/APK_OBFUSCATION_PLAYBOOK.md`,
  `docs/APK_ANALYZER_SPEC.md`.

## Fuentes externas

*Android:* <https://developer.android.com/reference/android/app/usage/NetworkStatsManager> ·
<https://android.googlesource.com/platform/frameworks/base/+/981962e/core/jni/android_net_TrafficStats.cpp> ·
<https://source.android.google.cn/docs/core/data/kernel-overview> ·
<https://source.android.google.cn/docs/core/data/ebpf-traffic-monitor> ·
<https://developer.android.com/about/versions/pie/android-9.0-changes-all> ·
<https://developer.android.com/reference/android/net/NetworkCapabilities.html> ·
<https://developer.android.com/develop/connectivity/network-ops/reading-network-state> ·
<https://source.android.com/docs/core/data/tethering-offload> ·
<https://android.googlesource.com/platform/packages/providers/TelephonyProvider/>

*Samsung:* <https://www.samsung.com/us/support/answer/ANS10002914/> ·
<https://www.samsung.com/uk/support/mobile-devices/how-do-i-check-data-usage-on-my-samsung-galaxy-device/> ·
<https://www.att.com/device-support/article/wireless/KM1426213/Samsung/SamsungSMG998U>

*Jazztel:* <https://ayuda.jazztel.com/factura-y-consumo/conceptos-de-factura/1959-cargo-por-gestion-de-cobro-y-cargo-por-reconexion> ·
<https://ayuda.jazztel.com/factura-y-consumo/conceptos-de-factura/> ·
<https://ayuda.jazztel.com/factura-y-consumo/otras-tramitaciones/1937-pagar-una-factura-devuelta-por-el-banco-o-impagada> ·
<https://www.jazztel.com/cliente/lineas-adicionales> ·
<https://www.jazztel.com/app-movil-jazztel> ·
<https://play.google.com/store/apps/details?id=com.orange.bluu.jazztel> ·
<https://www.lavanguardia.com/andro4all/operadoras/como-ver-el-consumo-en-jazztel-y-saber-cuantos-gigas-te-quedan> ·
<https://www.adslzone.net/operadores/jazztel/consultar-consumo-jazztel/> ·
<https://roams.es/companias-telefonicas/jazztel/guias/clientes/consultar-consumo/>

*Ofuscación y deofuscación (nuevo, §E.2):* <https://arxiv.org/html/1801.01633> ·
<https://arxiv.org/html/1611.10231> ·
<https://arxiv.org/html/2502.04636> ·
<https://arxiv.org/html/2606.14233> ·
<https://arxiv.org/html/2603.18355v1> ·
<https://arxiv.org/html/2610.29566> ·
<https://arxiv.org/html/2605.30902> ·
<https://arxiv.org/html/2601.12916> ·
<https://oar.a-star.edu.sg/storage/2/2jq30xmqod/control-flow-obfuscation-for-android-applications.pdf> ·
<https://serval.unil.ch/resource/serval:BIB_D56CBD9367EE.P001/REF> ·
<https://sefcom.asu.edu/publications/dex-mon-access18.pdf> ·
<https://e-archivo.uc3m.es/rest/api/core/bitstreams/94dd4ad9-b0a4-47f1-a8b7-aba2f519290d/content> ·
<https://phaidra.univie.ac.at/api/object/o:1649486/download> ·
<https://static.usenix.org/event/woot09/tech/full_papers/rolles.pdf> ·
<https://dl.acm.org/doi/fullHtml/10.1145/3600160.3600194> ·
<https://dl.acm.org/doi/fullHtml/10.1145/3426020.3426089> ·
<https://www.exploit-db.com/docs/english/44967-symbolic-deobfuscation-from-virtualized-code-back-to-the-original.pdf> ·
<https://tigress.wtf/tigress-publications.html> ·
<https://github.com/Arsylk/jadx-string-decrypt> ·
<https://github.com/Stanislav-Povolotsky/jddlab/blob/main/skills/java-deobfuscation/SKILL.md> ·
<https://github.com/zahidaz/awake/blob/main/docs/reversing/static-analysis.md> ·
<https://github.com/harshdhamaniya/android-security-handbook/tree/main/40-Obfuscation> ·
<https://github.com/hacktricks-wiki/hacktricks/blob/master/src/mobile-pentesting/android-app-pentesting/manual-deobfuscation.md> ·
<https://github.com/Anthropicdaddy/Shark/blob/main/docs/packers/dexguard.md> ·
<https://github.com/JesusConwellpy/ai-security-methodology/blob/master/03-reverse-engineering/anti-analysis.md>

*Mora:* <https://sinpleitos.es/servicios/ficheros-morosidad/recursos/recargo-impago-telefonia> ·
<https://invoo.es/es/blog/consejos/calcular-interes-demora-1040-costes-cobro-factura/>
