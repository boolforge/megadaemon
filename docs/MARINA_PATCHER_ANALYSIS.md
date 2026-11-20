# Marina Patcher (`ru.drjmwdme.cgwbfzijr`) — análisis estático del APK

> Analizado **offline** en el sandbox con el APK descomprimido facilitado por el usuario (el móvil
> estaba con el túnel caído, HTTP 530). Todo lo de aquí es del **fichero real**, con hashes.

## 0. Identidad de los artefactos

| Artefacto | sha256 | Bytes |
|---|---|---|
| `classes.dex` | `5b0d376ddc79d4ff409f3158b382a335da1b7157a53815211c1d77c4f1200a79` | 6.054.740 |
| `AndroidManifest.xml` (AXML binario) | `0c7089e58263f6ae534cc4e3928edbfb54bd5d8542d8956f73339e28030058a8` | 53.888 |
| `resources.arsc` | `70d225f05454290e36e9c5c90afb6ed3766179f44521e200efec27f630d0e2e0` | 5.405.908 |

El `classes.dex` **coincide byte a byte** con el de la app instalada en el móvil
(`ru.drjmwdme.cgwbfzijr`) ⇒ mismo binario, sólo renombrado/ofuscado el paquete.

## 1. Qué es realmente: **Lucky Patcher (chelpus) reempaquetado**

* **Paquete** `ru.drjmwdme.cgwbfzijr` (ofuscado); clases `App`, `MainActivity`…`MainActivity10`,
  `PatchService`, `BinderActivity`, `NotificationService`, `OnBootReceiver`, `OnAlarmReceiver`,
  `PackageChangeReceiver`, `provider`, `service`, `SetPrefs`.
* **Firmante**: `META-INF/CERT.RSA` = **clave de test de AOSP**
  (`C=US, ST=California, L=Mountain View, O=Android, OU=Android, CN=Android`) ⇒ APK re-firmado.
* **Origen**: `chelpus.com`, `luckypatchers.com`, `t.me/lucky_patcher_chelpus`,
  `youtube.com/c/LuckyPatcherVideo`, `app.transifex.com/chelpus/lucky-patcher-1`.
* **Módulo Xposed/LSPosed**: metadatos `xposedmodule`, `xposedscope`, `xposeddescription`,
  `xposedminversion` y `assets/xposed_init` = **`com.xposed.XSupport`**.

## 2. 🔴 C2 · URLs **hardcodeadas** (22 únicas)

| URL | Qué es |
|---|---|
| **`http://109.95.209.70:8008/`** | **IP cruda**, sin TLS |
| **`https://149.56.155.246/luckypatcher/`** | **segunda IP cruda** |
| `http://patch.chelpus.com` · `http://patch.chelpus.com/submit` | **servidor de parches**: el APK objetivo **se sube a un tercero** |
| `https://chelpus.com/luckypatcher/` | web del producto |
| `http://www.appnext.com/myid.html` | **id de afiliado** de AppNext |
| `ws.tapjoyads.com` · `pagead2.googlesyndication.com` | ads con tracking |
| `play.google.com/store/apps/details?id=eu.chainfire.supersu` | enlace a **SuperSU** |
| `play.google.com/store/apps/details?id=stericson.busybox&feature` | enlace a **BusyBox** |
| `t.me/lucky_patcher_chelpus` · `app.transifex.com/chelpus/lucky-patcher-1` | canal y traducción |
| `4pda.ru` · `forpda.ru` · `pcl.info` · `binmt.cc` · `p.info` · `ya.ru` | foros ru (contenido en cirílico) |

25 dominios en total (incluye `android.net`, `adobe.com`, `x509.info`).

## 3. 🔴 C2 · **Claves de firma de AOSP empaquetadas** + `apksigner` embebido

| Ruta | Qué es |
|---|---|
| `assets/testkey.pk8` / `.sbt` / `.x509.pem` / `.init` | **clave de test de AOSP** (pública) |
| `keys/media.pk8` / `.sbt` / `.x509.pem` | clave **media** de AOSP |
| `keys/platform.pk8` / `.sbt` / `.x509.pem` | clave **platform** de AOSP |
| `keys/shared.pk8` / `.sbt` / `.x509.pem` | clave **shared** de AOSP |
| `keys/testkey.pk8` / `.sbt` / `.x509.pem` | clave **testkey** de AOSP |
| `keys/keystore.ks` | keystore Java (1.374 B, **PKCS12**) |
| `com/android/apksigner/` | **la herramienta `apksigner` de AOSP embebida** |
| `com/android/vending/` + `com/services/ITestServiceInterface.aidl` | **servicio de licencias de Play embebido** |
| `org/apache/http/` | cliente HTTP embebido |

⇒ **Capacidad**: firmar APKs con testkey/media/platform/shared de AOSP (y bypassear licencias).
⇒ **Impacto real en este S21 Ultra**: **limitado** — bootloader **cerrado** (`flash.locked=1`),
particiones firmadas por Samsung y AVB `green` ⇒ un APK con testkey **no** pasa la verificación
de firma ⇒ no sirve para parchear apps de sistema; **sí** para re-firmar terceros.
⇒ **Riesgo real**: envío del APK a un tercero (`patch.chelpus.com/submit`), IPs sin TLS, root y Xposed.

## 4. Permisos declarados (20)

`ACCESS_SUPERUSER` · `REQUEST_INSTALL_PACKAGES` · `REQUEST_DELETE_PACKAGES` · `QUERY_ALL_PACKAGES` ·
`MANAGE_EXTERNAL_STORAGE` · `READ/WRITE_EXTERNAL_STORAGE` · `WRITE_MEDIA_STORAGE` ·
`SYSTEM_ALERT_WINDOW` · `KILL_BACKGROUND_PROCESSES` · `INTERNET` · `ACCESS_NETWORK_STATE` ·
`CHANGE_WIFI_STATE` · `READ_PHONE_STATE` · `RECEIVE_BOOT_COMPLETED` · `FOREGROUND_SERVICE` ·
`WAKE_LOCK` · `VIBRATE` · `REORDER_TASKS` · `Ad_ID`.

⇒ `ACCESS_SUPERUSER` + `REQUEST_INSTALL_PACKAGES` + `QUERY_ALL_PACKAGES` = **pide root y puede
instalar/desinstalar paquetes**.

## 5. Otros

* `assets/AdsBlockList.txt` (80 líneas): bloqueo de anuncios por patrón (`[HTTP]`, `/ads`, `.ads.`, `advert`, `.gstatic.com`…).
* 1.170 blobs base64 ≥40 chars (certs X.509 DER, p. ej. `MIIEvgIBADANBgkqhkiG9w0…`).
* **Sin `lib/*.so`**: un solo DEX, todo el SDK de firma en Java.

## 6. Cómo se comprueba (sin móvil)

```bash
sha256sum classes.dex AndroidManifest.xml
strings classes.dex | grep -Eo 'https?://[^" ]+' | sort -u
strings classes.dex | grep -Eo '\b([a-z0-9-]+\.)+(com|net|org|ru|cn|io)\b' | sort -u
openssl pkcs7 -inform DER -print_certs -in META-INF/CERT.RSA | grep subject
ls -la assets/ keys/
python3 tools/re/axml.py AndroidManifest.xml     # manifiesto AXML sin aapt
```

## 7. Recomendación para el usuario

Marina Patcher en el móvil = Lucky Patcher ofuscado, con **claves de firma de AOSP dentro**,
**módulo Xposed**, permisos de **root** y de **instalar/desinstalar paquetes**, y **envío del APK
objetivo a un servidor de terceros**. Con el bootloader cerrado no puede tocar apps de sistema,
pero el vector de hook de Xposed y el envío del APK son riesgos reales ⇒ **desinstalar, o como
mínimo no darle root ni usarlo**.
