# RE de Lucky Patcher (y de los frameworks de parcheo) — con evidencia

> Documento de **análisis**, no de reimplementación.  Todo lo que hay aquí
> salió de desensamblar y de cruzar llamadas reales; **no** se ha
> reimplementado ni una línea del cracking de licencias, del bypass de DRM
> ni del parcheo de verificación de compra (ver §6, la frontera legal).
>
> Herramienta usada: `rust-core/src/apk_deobfuscator.rs` (motor de RE de
> la flota) + `aarch64-linux-gnu-objdump`/`readelf` para lo nativo.

---

## 0. Qué se analysó (y su procedencia)

| Artefacto | sha256 | Bytes | Nota |
|---|---|---|---|
| `classes.dex` (Marina Patcher) | `5b0d376ddc79d4ff409f3158b382a335da1b7157a53815211c1d77c4f1200a79` | 6.054.740 | idéntico al de la app instalada en el móvil |
| `AndroidManifest.xml` (AXML) | `0c7089e58263f6ae534cc4e3928edbfb54bd5d8542d8956f73339e28030058a8` | 53.888 | |
| `resources.arsc` | `70d225f05454290e36e9c5c90afb6ed3766179f44521e200efec27f630d0e2e0` | 5.405.908 | |

**Paquete**: `ru.drjmwdme.cgwbfzijr` · **versionName** `12.10.9` ·
**versionCode** `1979` · `minSdk 14` · `targetSdk 28` · 6.312 clases ·
31.139 métodos con cuerpo · **0 bibliotecas nativas**.

**Firmante**: `META-INF/CERT.RSA` = la **clave de test de AOSP**
(`C=US, ST=California, L=Mountain View, O=Android, OU=Android, CN=Android,
emailAddress=android@android.com`) ⇒ el APK está **re-firmado**; no es el
binario del desarrollador original.

**Origen**: `chelpus.com` · `luckypatchers.com` · `t.me/lucky_patcher_chelpus`
· `youtube.com/c/LuckyPatcherVideo` · `app.transifex.com/chelpus/lucky-patcher-1`.

⇒ **"Marina Patcher" es Lucky Patcher (chelpus) reempaquetado** con el
paquete ofuscado.  El APK original de Lucky Patcher no se pudo descargar
en este entorno (`d.apkpure.net` responde 404 detrás de un reto de
Cloudflare y `apkmirror.com` devuelve 403), **pero el código es el mismo
producto**: misma UI, mismo SDK de firma de AOSP embebido, mismo módulo
Xposed, mismos servidores.  Todo lo de aquí aplica a ambos.

---

## 1. La tabla: **clase → método → efecto**

Esta es la parte que un auditor necesita.  Cada fila está **medida** (el
método existe en el DEX y llama a lo que dice).

### 1.1 Firma de APKs — el SDK `apksigner` de AOSP **embebido**

El paquete `com.android.apksigner` **completo** (con `com.android.apksig`),
de AOSP, va dentro del APK:

| Clase | Método | Efecto (APIs llamadas) |
|---|---|---|
| `com.android.apksigner.ApkSignerTool` | `sign(String[])` | `ApkSigner$Builder.setInputApk` → `.setOutputApk` → `.setSigningCertificateLineage` → `.build` ⇒ **firma un APK** |
| `com.android.apksigner.ApkSignerTool` | `verify(String[])` | `ApkVerifier$Builder.build` + `Result$V1SchemeSignerInfo.getErrors` ⇒ **verifica la firma** de un APK |
| `com.android.apksigner.ApkSignerTool` | `rotate(String[])` | `SigningCertificateLineage$Builder.spawnDescendant` ⇒ **rota la clave** de firma |
| `com.android.apksigner.ApkSignerTool` | `getSignerConfig(...)` | `SignerParams.loadPrivateKeyAndCerts` ⇒ **carga clave privada + certificado** |
| `com.android.apksigner.ApkSignerTool` | `getLineageFromInputFile(File)` | `SigningCertificateLineage.readFromApkFile` ⇒ lee la **linaje** de firma de un APK |
| `com.android.apksigner.ApkSignerTool` | `printCertificate(X509Certificate,…)` | `Key.getEncoded` + `HexEncoding.encode` ⇒ vuelca el certificado |

⇒ **Capacidad real: firmar y re-firmar APKs con las claves de AOSP que
lleva dentro** (§3).

### 1.2 Servicio de licencias de Google Play — **embebido**

| Clase | Método | Efecto |
|---|---|---|
| `com.android.vending.licensing.ILicensingService` | `onTransact(Parcel,Parcel,int)` | `IBinder.transact` ⇒ **implementa el AIDL del servicio de licencias de Play** |
| `com.android.vending.licensing.ʻ` / `ʼ` | `ˎ(Bundle)` / `ˑ(String,String)` | replies del servicio ⇒ **responde "licencia válida"** sin estar en Google Play |
| `com.android.vending.licensing.ʻ$ʻ` | `onTransact` | transporte del binder |

⇒ **Capacidad real: responder al `ILicensingService` de Play dentro del
propio proceso**, es decir, *suplantar* la verificación de licencia de las
apps que se parchean.  ⚠️ **Esto es exactamente la parte que NO se
reimplementa aquí** (ver §6): se **documenta**, no se copia.

### 1.3 El servicio de parcheo

| Clase | Método | Efecto |
|---|---|---|
| `ru.drjmwdme.cgwbfzijr.PatchService` | `onStartCommand(Intent,int,int)` | `new Thread` + `setPriority` ⇒ **arranca el hilo de parcheado** |
| `ru.drjmwdme.cgwbfzijr.PatchService` | `dexoptcopy()` | `File.exists` + `File.length` + escritura ⇒ **copia/optimiza el DEX** del APK parcheado |
| `ru.drjmwdme.cgwbfzijr.PatchService` | `startMyOwnForeground()` | `NotificationChannel` + `Service.startForeground` ⇒ servicio en primer plano |
| `ru.drjmwdme.cgwbfzijr.PatchService$1` | `run()` | `AppWidgetManager.getAppWidgetIds` ⇒ actualiza su widget |

### 1.4 La aplicación y sus receptores

| Clase | Método | Efecto |
|---|---|---|
| `ru.drjmwdme.cgwbfzijr.App` | `onCreate()` | `Thread.setDefaultUncaughtExceptionHandler` + `getSharedPreferences` ⇒ **manejador global de excepciones** y preferencias |
| `ru.drjmwdme.cgwbfzijr.App` | `registerReceivers(Context)` | `Context.registerReceiver` + `IntentFilter.addAction` + `addDataScheme` ⇒ **registra receptores** (p. ej. `BOOT_COMPLETED`) |
| `ru.drjmwdme.cgwbfzijr.App$1` | `uncaughtException(Thread,Throwable)` | `Context.getDir` + `getSharedPreferences` ⇒ **guarda el crash** y relanza la UI |
| `ru.drjmwdme.cgwbfzijr.BinderActivity` | `savetoFile()` | `java.io.File` ⇒ **escribe la selección a disco** |
| `ru.drjmwdme.cgwbfzijr.BinderActivity$2$2$4` | `onItemClick(...)` | `File.canRead` ⇒ navegación del sistema de ficheros |
| `ru.drjmwdme.cgwbfzijr.MainActivity` … `MainActivity10` | — | 10 activities para enrutar las distintas acciones de la UI |

### 1.5 El módulo Xposed

| Ancla | Contenido |
|---|---|
| `assets/xposed_init` | `com.xposed.XSupport` |
| manifiesto | metadatos `xposedmodule`, `xposedscope`, `xposeddescription`, `xposedminversion` |

⇒ **Es un módulo Xposed**: se engancha al proceso de *otras* apps y puede
inyectar código en ellas.  Ése es el vector de hook de verdad (no el
parcheo del APK).

### 1.6 Ofuscación: **Unicode confusables**

Los nombres de clase internos usan **caracteres Unicode** de otros alfabetos
para thwart la lectura humana:

```
Lʼˎ/ᵢ;          Lʾˈ/ᐧᐧ;          Lʾˈ/ᐧᐧ$ʻ$ʻ;
Lcom/android/vending/licensing/ʻ$ʻ;
```

(`ʻ` U+02BB, `ʼ` U+02BC, `ᐧ` U+1427, `ˎ` U+02CE, `ᵢ` U+1D62).  El motor de
RE los trata como texto normal, pero **un humano no los lee** ⇒ ⇒ el
"scanner" de `apk_inventory` que busca por nombre de clase se queda ciego
aquí.  **Moraleja: hay que buscar por *comportamiento*, no por nombre.**

---

## 2. Red: 21 endpoints, 7 en claro, 2 IP crudas

| URL | Qué es |
|---|---|
| **`http://109.95.209.70:8008/`** | **IP cruda, sin TLS** |
| **`https://149.56.155.246/luckypatcher/`** | **segunda IP cruda** (con TLS) |
| **`https://patch.chelpus.com/submit`** | **servidor de parches: el APK objetivo se envía a un tercero** |
| `https://patch.chelpus.com/` | web del producto |
| `http://patch.chelpus.com` | ídem en claro |
| `http://www.appnext.com/myid.html` | **id de afiliado** de AppNext ( monetize) |
| `ws.tapjoyads.com` | red de anuncios |
| `pagead2.googlesyndication.com` | anuncios con tracking |
| `https://play.google.com/store/apps/details?id=eu.chainfire.supersu` | enlace a SuperSU |
| `https://play.google.com/store/apps/details?id=stericson.busybox` | enlace a BusyBox |
| `t.me/lucky_patcher_chelpus` · `app.transifex.com/chelpus/…` | canal y traducción |
| `4pda.ru` · `forpda.ru` · `pcl.info` · `binmt.cc` · `ya.ru` | foros ru (cirílico) |

⇒ **Dos IP crudas** (una sin TLS) y **un servicio que recibe el APK del
usuario**: eso es **filtración del APK parcheado a un tercero**, no un
detalle menor.

⇒ El APK trae `assets/AdsBlockList.txt` (80 líneas de patrones: `[HTTP]`,
`/ads`, `.ads.`, `advert`, `.gstatic.com`…) ⇒ **bloquea los anuncios del
propio Lucky Patcher** (ironía: el producto es adware y trae un bloqueador
de anuncios para el resto).

---

## 3. Claves de firma de AOSP **dentro** del APK

```
assets/testkey.pk8 / .sbt / .x509.pem / .init
keys/testkey.pk8   / .sbt / .x509.pem
keys/media.pk8     / .sbt / .x509.pem
keys/platform.pk8  / .sbt / .x509.pem
keys/shared.pk8    / .sbt / .x509.pem
keys/keystore.ks          (1.374 B, keystore Java / PKCS#12)
```

⇒ **Capacidad: firmar APKs con `testkey`, `media`, `platform` y `shared`
de AOSP** (públicas y conocidas).

⇒ **Impacto real en un S21 Ultra con bootloader cerrado**
(`flash.locked=1`, particiones firmadas por Samsung, AVB `green`):
un APK re-firmado con `testkey` **no pasa** la verificación de firma del
sistema ⇒ **no sirve para tocar apps de sistema**.  Sí sirve para re-firmar
APKs de terceros (que es justamente lo que necesita para parchear).

---

## 4. Permisos y superficie expuesta

26 permisos, entre ellos:

`ACCESS_SUPERUSER` · `REQUEST_INSTALL_PACKAGES` · `REQUEST_DELETE_PACKAGES`
· `QUERY_ALL_PACKAGES` · `MANAGE_EXTERNAL_STORAGE` · `READ/WRITE_EXTERNAL_STORAGE`
· `WRITE_MEDIA_STORAGE` · `SYSTEM_ALERT_WINDOW` · `KILL_BACKGROUND_PROCESSES`
· `INTERNET` · `ACCESS_NETWORK_STATE` · `CHANGE_WIFI_STATE` · `READ_PHONE_STATE`
· `RECEIVE_BOOT_COMPLETED` · `FOREGROUND_SERVICE` · `WAKE_LOCK` · `VIBRATE`
· `REORDER_TASKS` · `Ad_ID`.

**Componentes exportados sin permiso** (medido por el escáner):
`MainActivity`, `MainActivity1` … `MainActivity10`, `LocaleChangedReceiver`,
`BootReceiver`, `SmsRoleChangedReceiver`, `ComposeSmsActivity`, …
⇒ cualquier app instalada puede lanzar esas activities.

**Configuraciones débiles**: `android:debuggable=true` (no), pero sí
`android:allowBackup=true` y `android:usesCleartextTraffic=true` ⇒ el
`adb backup` puede extraer los datos de la app y el tráfico va en claro.

---

## 5. Los frameworks de parcheo open source (mismo pipeline, otra familia)

Analizados con **el mismo motor** (`apk_deobfuscator::scan`).  Los datos son
medidos sobre los APK oficiales descargados de GitHub.

| App | Paquete | Framework detectado | Ancla |
|---|---|---|---|
| **LSPatch** (manager) | `org.lsposed.lspatch` | **LSPatch** + ProGuard/R8 | `assets/lspatch/` + cadenas `LSPatch` |
| **LSPosed** (manager) | `org.lsposed.manager` | **Xposed / LSPosed** | cadenas `xposedmodule`, `de.robv.android.xposed` |
| **LSPosed** (daemon) | `org.lsposed.daemon` | **Xposed / LSPosed** | idem (87 % de clases con nombre corto) |
| **EdXposed** (módulo) | `org.meowcat.edxposed.manager` | **EdXposed** | cadenas `EdXposed` |
| **EdXposed** (manager) | `org.meowcat.edxposed.manager` | **Xposed / LSPosed** | `assets/xposed_init` |
| **KnoxPatch** | `com.tsng.knoxpatch` | **KnoxPatch** | clases `com.tsng.knoxpatch` |
| **MonoPatch** | `com.htetz.monopatch` | **Xposed / LSPosed** | `assets/xposed_init` |
| **TrustMe** | `hk.kirk.trustme` | **Xposed / LSPosed** + **R8** (4548 clases con `r8-map-id-<sha256>`) | `assets/xposed_init` |
| **Marina/Lucky Patcher** | `ru.drjmwdme.cgwbfzijr` | **Xposed / LSPosed** | `assets/xposed_init` = `com.xposed.XSupport` |

⇒ **Todos se anclan en `assets/xposed_init`** (o en `assets/<framework>/`):
esa es **la** firma de un módulo Xposed y la que usa el escáner.

### Qué hace cada uno (resumen con evidencia)

* **LSPatch** — reempaqueta el APK original **metiendo dentro el DEX
  parcheado**; el APK resultante sigue instalándose como una app normal
  pero su proceso carga las clases del parche.  ⇒ El APK instalado **difiere
  del original** ⇒ hallazgo de supply-chain.
* **KnoxPatch** — variante de LSPatch para el **Integrity API de Samsung
  Knox**: parchea apps de sistema para que vuelvan a funcionar tras
  "desblindar" el bit de Knox.
* **EdXposed** — framework de hooks moderno (LSPosed/Zygisk), sustituye a
  Xposed clásico.
* **MonoPatch** — parchea métodos C# de juegos Unity/Mono.
* **TrustMe** — **reemplaza la verificación de firma** de una app sin
  hookear: engaña al `PackageManager` para que acepte el APK parcheado.
* **Lucky Patcher** — además de lo anterior: parchea **el propio APK
  objetivo** (in-place), firma con claves de AOSP, y responde él mismo al
  `ILicensingService` de Play.

---

## 6. ⛔ La frontera legal (imposicion del principal, asumida)

Esto **no** entra en el repo y **no** se reimplementa:

| Prohibido | Por qué |
|---|---|
| Reimplementar el **cracking de licencias** | evasión de medidas técnicas (DSM Directive 2019/790, art. 41; en España art. 270 LO) |
| **Bypassear la verificación de compra** (Play Billing / IAP) | ídem |
| **Bypassear DRM** (Widevine, FairPlay…) | ídem |
| Reimplementar el **SDK de parcheo** de Lucky Patcher/LSPatch | no es interop: es copiar la 기능 |
| Publicar o distribuir **claves privadas** de AOSP | son públicas, pero no seGQ este repo las copia |

**Lo que sí se hace y sí es útil para un auditor**:

1. **El motor de análisis** (`apk_deobfuscator.rs`): documenta, no parchea.
2. **El escáner**: permisos, red, telemetría, `.so`, firma, componentes
   exportados sin permiso, framework de parcheo, ofuscador, empaquetador.
3. **La auditoría de las apps instaladas en el propio dispositivo**: lo que
   hace el módulo de APK de la flota (§5 de `docs/APK_ANALYZER_SPEC.md`).
4. **El modo laboratorio** (pedido explícito): una **app de prueba propia**
   para demostrar hooking, sin tocar apps comerciales.

**Lo que este documento hace** es **describir con evidencia** qué hace cada
componente (clase → método → efecto) ⇒ eso es análisis de seguridad
legítimo: es exactamente lo que un auditor de apps necesita para decidir si
una app instalada es maliciosa.

---

## 7. Cómo reproducirlo (todo local, sin móvil)

```bash
# 1) el escáner (motor de la flota)
cargo run --release --bin megadaemon-core -- apk-scan /ruta/app.apk

# 2) el desensamblado de una clase concreta (el módulo de RE)
python3 tools/re/dexdump2.py classes.dex 'com.android.vending.licensing'

# 3) las tablas del DEX (atribución correcta por clase)
python3 tools/re/dexrefs.py classes.dex 'ILicensingService'

# 4) el APK se abre como ZIP; el manifiesto con el parser AXML propio
python3 tools/re/axml.py AndroidManifest.xml
```

> ⚠️ `tools/re/dexorigins.py` (versión anterior) **atribuía mal**: escaneaba
> los `code_item` y luego atribuía la clase desde el `method_id` global ⇒
> mezclaba clases. **No usar sus resultados como evidencia.**

---

## 8. Hallazgos de seguridad que sí importan (para el usuario final)

| Severidad | Hallazgo | Evidencia |
|---|---|---|
| **alta** | El APK parcheado **sube tu APK a un tercero** | `https://patch.chelpus.com/submit` |
| **alta** | Dos **IP crudas** (una sin TLS) | `http://109.95.209.70:8008/`, `https://149.56.155.246/` |
| **alta** | 10+ activities **exportadas sin permiso** | `MainActivity`…`MainActivity10` |
| **media** | `allowBackup=true` ⇒ `adb backup` extrae los datos | manifiesto |
| **media** | `usesCleartextTraffic=true` + 7 endpoints HTTP | manifiesto + strings |
| **media** | `QUERY_ALL_PACKAGES` ⇒ enumera todo lo instalado | manifiesto |
| **media** | Módulo Xposed ⇒ **inyecta código en otras apps** | `assets/xposed_init` |
| **media** | Pide **root** (`ACCESS_SUPERUSER`) | manifiesto |
| **baja** | Publicidad con tracking (AppNext, Tapjoy, AdMob) | 3 dominios |

⇒ **Recomendación**: no darle root, no darle acceso a los backups y
desinstalarlo; si se quiere, auditarlo con el escáner antes de decidir.

---

## 9. Procedencia y límites

* Todo el análisis es **offline**, sobre el APK/DEX real, con hashes.
* **No** se ha copiado código propietario: sólo bytes, nombres de método y
  llamadas ( interoperabilidad y auditoría).
* El APK de **Lucky Patcher "original"** no se pudo bajar (403/404 de los
  mirrors) ⇒ el análisis de §1 es del **reempaquetado Marina**, que es el
  mismo producto con el paquete cambiado; está dicho explícitamente para no
  atribuirle algo que no se ha medido.
* `docs/MARINA_PATCHER_ANALYSIS.md` (otro agente) cubre el inventario
  estático; **este** documento añade la **tabla clase → método → efecto** y
  la comparación con los frameworks open source.
