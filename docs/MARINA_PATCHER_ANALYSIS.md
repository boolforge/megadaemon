# Marina Patcher (= Lucky Patcher) — análisis de ingeniería inversa

> **Corrección de identidad (importante).** En la primera pasada
> (`docs/SAMSUNG_APK_DEOBFUSCATION.md`) atribuí Marina Patcher al paquete
> `com.callapp.contacts` porque su `resources.arsc` contenía el recurso
> `lucky_patcher_package`. **Era falso**: ese paquete es otra app
> (trae el SDK/ads de Lucky Patcher, no el parcheador).
> La app real es **`ru.drjmwdme.cgwbfzijr`**, y lo demuestra la huella del
> `classes.dex` instalada en el móvil (sha256 idéntico al del artefacto
> analizado aquí).

---

## 1. Identidad y procedencia

| Dato | Valor |
|---|---|
| Paquete | `ru.drjmwdme.cgwbfzijr` (ofuscado al azar) |
| Label | `MarinaPatcher` (Lucky Patcher renombrado por el usuario) |
| Versión | `12.10.9` (versionCode 1979) |
| APK instalado (`base.apk`) | 15 029 615 B · sha256 `7496b221946ff1c5fcaf0de91e57f22d37c730c33e8bfe13a894d24319433232` |
| `classes.dex` | 6 054 740 B · sha256 `5b0d376ddc79d4ff409f3158b382a335da1b7157a53815211c1d77c4f1200a79` |
| Ruta instalada | `/data/app/~~O3-8UI8r8CTIDZu4vpEnmg==/ru.drjmwdme.cgwbfzijr-5El5pJepVE5_zB6Es587dg==/base.apk` |
| Tipo | **módulo Xposed** (`assets/xposed_init` → `com.xposed.XSupport`) + aplicación con UI + servicio |
| Código nativo | **ninguno** (`lib/` no existe) ⇒ toda la lógica está en el DEX |

El artefacto recibido (el zip con el APK descomprimido) produce un
`classes.dex` con **sha256 idéntico** al de la app instalada ⇒ se analiza
el binario que realmente corre.

## 2. Manifiesto (parseado del AXML binario)

- `application`: `debuggable=false`, `largeHeap=true`, `usesCleartextTraffic=true`,
  `networkSecurityConfig`, `requestLegacyExternalStorage=true`, `allowBackup=true`.
- **Permisos (27)**, los relevantes: `ACCESS_SUPERUSER`, `REQUEST_INSTALL_PACKAGES`,
  `REQUEST_DELETE_PACKAGES`, `MANAGE_EXTERNAL_STORAGE`, `WRITE_MEDIA_STORAGE`,
  `QUERY_ALL_PACKAGES`, `SYSTEM_ALERT_WINDOW`, `REORDER_TASKS`,
  `KILL_BACKGROUND_PROCESSES`, `READ_PHONE_STATE`, `CHANGE_WIFI_STATE`,
  `RECEIVE_BOOT_COMPLETED`, `com.android.vending.BILLING` + `CHECK_LICENSE`,
  `com.huawei.permission.external_app_settings.USE_COMPONENT`,
  `oppo.permission.OPPO_COMPONENT_SAFE`.
- **Componentes** (30): `MainActivity` (+ 9 `activity-alias` `MainActivity1..10`),
  `SetPrefs`, `BinderActivity`, `PatchService`, `NotificationService`,
  `PackageChangeReceiver`, `OnAlarmReceiver`, `BinderBootReceiver`,
  `OnBootReceiver`, `com.services.MyService` / `TestService`
  (AIDL `com.ui.ITestServiceInterface`), `com.widgets.Widget1..5` +
  `WidgetConfigureActivity1/2` (ads), `androidx.core.content.FileProvider`
  (`authority=ru.drjmwdme.cgwbfzijr.provider`).
- **meta-data de Xposed**: `xposedmodule=true`, `xposedminversion=82`,
  `xposedscope`, `xposeddescription=MarinaPatcher`.

## 3. Anatomía del DEX

| Métrica | Valor |
|---|---|
| Clases | 6 312 |
| Métodos | 37 506 |
| Strings | 38 816 |
| Tipos | 7 462 |

**Ofuscación del identificador**: los nombres de clase/método usan
**homoglifos Unicode** (`ʻ ʼ ʽ ʾ ʿ ˆ ˈ ˉ …`) en lugar de ASCII — firma
distintiva del ofuscador de Lucky Patcher (p. ej.
`com/xposed/XSupport$ʾ`, `com/google/common/cache/ʻ`). Las clases *de la
app* (`ru.drjmwdme.cgwbfzijr.*`, 164 clases) conservan nombres legibles.

### 3.1 Los tres motores que trae dentro

| Motor | Clases | Qué es |
|---|---|---|
| **CorePatch** | `cc.binmt.signature.*` (`PmsHookApplication`, `PmsDexHookApplication`, `CustomPatchApplier`, `UnpackerApplication`, `LocalFileObserver`) | Hooks del `PackageManagerService` para **suplantar la firma** de la app parcheada |
| **PMPatch (vova7878)** | `com.v7878.oat.*` (`Oat`, `Reflection*Mirror`, `AndroidUnsafe`, `MapParser`), `com.v7878.hooks.pmpatch.Main` | **Parcheado de ART/OAT en runtime** (parchea el código ya compilado en memoria) |
| **Módulo Xposed propio** | `com.xposed.XSupport` (11 métodos) + 28 subclases anónimas | Punto de entrada del módulo (`assets/xposed_init`) |

Además embebe **apksigner de AOSP** (`com/android/apksigner/*` con sus
`help*.txt`), el **LVL** (License Verification Library) y el **Play
In-App Billing** (`com.android.vending.billing.IabHelper`,
`InAppBillingService`, `LicensingService`).

## 4. Qué hace realmente (evidencia ⇒ capacidad)

1. **Parchea el DEX de la app objetivo y lo re-inyecta**
   (`/Modified/classes.dex.apk`, `.apk@classes.dex`, `classes2..6.dex`,
   `unzip classes.dex fault!`) y borra las cachés ART para forzar la
   recompilación (`rm /data/dalvik-cache/*.art|.dex|.oat`, con barrido de
   `arm`, `arm64`, `x86`, `x86_64`).
2. **Re-firma** con las **claves AOSP públicas** que trae en `keys/` y
   `assets/` (testkey/platform/media/shared, 2008) y usa `apksigner`.
3. **Suplanta la firma** en runtime vía hooks de CorePatch (`PmsHookApplication`).
4. **Emula licencias y compras** (`BillingHack`, `checkLicense`,
   `%%%LICENSE_DATA%%%`, `use license verification V2`) y **proxy de Google Play**.
5. **Bloquea anuncios** (asset `AdsBlockList.txt` con ~30 dominios de ads;
   `option_remove_ads`).
6. **Modo root**: detecta Magisk/SuperSU/KernelSU/busybox
   (`/data/adb/magisk/busybox`, `/data/adb/ksu/bin/busybox`, `/product/bin/su`,
   `/magisk/.core/bin/su`, `/data/adb/magisk.db`, `busybox mount`, `remount`,
   `setenforce`, `option_SELinux_permissive`, `option_force_turn_off_magisk`).
7. **Módulo Magisk/Zygisk** (`PMPatchLP.zip`, `PMPatchInappLP.zip`,
   `option_magisk_module`, `installer_package=`).
8. **Caché de parches** (`PATCH_1..PATCH_3=true`, `[NEXT_PATCH]`,
   `PATCH_1=`, `PATCH_2=`, `PATCH_3=`) y contenedores `.lpzip`
   (`application/vnd.luckypatcher.archive`).

### 4.1 Superficie de red (lo más relevante para la flota)

| Destino | Uso |
|---|---|
| `https://patch.chelpus.com/` | **repositorio de parches** (código ejecutable) |
| `https://149.56.155.246/luckypatcher/` | servidor de parches/actualizaciones (**IP desnuda, TLS con IP**) |
| `http://109.95.209.70:8008/` | servidor de parches/actualizaciones (**HTTP en claro**) |
| `http://chelpus.com/luckypatcher/`, `https://chelpus.com/luckypatcher/`, `https://www.luckypatchers.com/chelpus/` | apoyo/ayuda |
| `https://app.transifex.com/chelpus/lucky-patcher-1` | traducciones |
| `www.appnext.com`, `ws.tapjoyads.com`, `pagead2.googlesyndication.com` | ads/analítica (`option_analytic`) |

⇒ Un parche descargado de esos hosts **es código que se ejecuta dentro de
otra app**. Es el riesgo de verdad de esta herramienta, no la ofuscación.

## 5. Estado real en este móvil

- `lp/j256h0` → `{"patch1":true,"patch2":true,"patch3":false,"patch4":false,"hide":false,"module_on":true}`
  ⇒ parches 1 y 2 activos, 3 y 4 inactivos, ocultación de root **desactivada**, **módulo Magisk activo**.
- `lp/82a8351d` → registro de busybox detectado en
  `/data/adb/magisk/busybox` ⇒ **root por Magisk confirmado**.
- `databases/`: `Packages` (**15 MB**), `BillingListPurchases`,
  `BillingRestoreTransactions` ⇒ emulación de compras en marcha.
- Esquema de estado por app (del DEX):
  `Packages(pkgName, pkgLabel, stored, storepref, hidden, statusi, boot_ads,
  boot_lvl, boot_custom, boot_manual, custom, lvl, ads, modified, system,
  odex, icon BLOB, updatetime, billing, install_dir)`.
- `shared_prefs/`: `ru.drjmwdme.cgwbfzijr_preferences.xml`, `config.xml`
  (flags por paquete), `unityads-installinfo.xml`.
- Switches seen: `switch_auto_backup_apk(_only_gp)`, `switch_auto_integrate_update`,
  `switch_auto_move_to_internal`, `option_remove_ads`, `option_magisk_module`,
  `option_force_turn_off_magisk`, `option_SELinux_permissive`, `option_analytic`,
  `option_detailed_logs`, `option_test_patch`, `option_autoupdate_custom_patches`.

## 6. Qué se integra en MegaDaemon (y qué no)

**Se integra (legal, útil y verificable):**

1. **Detector de firma AOSP** (`rust-core/src/dex_decompile.rs`):
   `aosp_signing_key(sha256_cert)` reconoce las 4 huellas AOSP que el
   parcheador usa ⇒ cualquier APK instalada firmada con una de ellas queda
   marcada como *firma suplantada / parcheada*. Es el indicador barato y
   definitivo de "esta app pasó por Marina Patcher".
2. **Detector de módulo** (`lp_module_findings`): marca cualquier APK que
   embeba el módulo (`com/xposed/XSupport`, `vnd.luckypatcher.archive`,
   `cc/binmt/signature/`, `com/v7878/hooks/pmpatch`, `ru/drjmwdme/cgwbfzijr`).
3. **Parser de AXML** (`tools/re/axml.py`): manifiesto legible sin `aapt`
   ⇒ requisito para automatizar el censo de APKs del móvil.

**No se reimplementa:** el motor de parcheo (PMPatch/CorePatch). Es
propiedad ajena, descarga parches de terceros y parchea ART en runtime;
reimplementarlo no aporta nada legal ni seguro.

## 7. Reproducir

```bash
python3 tools/re/axml.py AndroidManifest.xml     # manifiesto legible
python3 tools/re/dexdump.py classes.dex --strings > dex.json
# o con la herramienta propia del repo:
megadaemon-core dex <apk|dex> obfuscation|strings|decompile
```
