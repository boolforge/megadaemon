# Detectores de *toolkits de parcheo* reempaquetados (familia Lucky Patcher)

> Módulo: `rust-core/src/app_intel.rs` (`app_intel::detect_patcher_toolkit*`).
> Caso de origen: **C2** — `docs/MARINA_PATCHER_ANALYSIS.md` (commit `c6d6a07`),
> análisis offline del APK que pasó el usuario (`MarinaPatcher`, paquete
> ofuscado `ru.drjmwdme.cgwbfzijr`).

## Qué se midió (y por qué importa al patrón, no al nombre)

El `classes.dex` de la app instalada en el S21 coincide **byte a byte** con el
del APK reempaquetado:

| Artefacto | sha256 | Bytes |
|---|---|---|
| `classes.dex` | `5b0d376ddc79d4ff409f3158b382a335da1b7157a53815211c1d77c4f1200a79` | 6.054.740 |
| `AndroidManifest.xml` | `0c7089e58263f6ae534cc4e3928edbfb54bd5d8542d8956f73339e28030058a8` | 53.888 |
| `resources.arsc` | `70d225f05454290e36e9c5c90afb6ed3766179f44521e200efec27f630d0e2e0` | 5.405.908 |

⇒ **el ocultamiento está en el paquete y en la etiqueta** ("Marina Patcher" con
el icono de Lucky Patcher), **no en los identificadores del DEX** (no hay
ProGuard/R8: `App`, `PatchService`, `BinderActivity`, `MainActivity1..10`…).
Un detector por *nombres de clase* no lo ve; uno por **artefactos embebidos** sí.

## Lo que delata el APK (y ahora detecta `app_intel`)

| Id | Clase | Severidad | Marcador |
|---|---|---|---|
| `aosp-testkey-material` | signing | critical | `testkey.pk8`, `testkey.x509.pem`, `media.pk8`, `platform.pk8`, `shared.pk8` |
| `keystore-p12-embedded` | signing | critical | `keystore.ks` |
| `apksigner-embedded` | signing | high | `com/android/apksigner`, `com.android.apksigner` |
| `play-license-bypass` | license-bypass | critical | `ITestServiceInterface` |
| `play-vending-embedded` | license-bypass | high | `com/android/vending`, `com.android.vending` |
| `xposed-module` | hook | critical | `com.xposed.XSupport`, `com/xposed/XSupport` |
| `xposed-metadata` | hook | high | `xposedminversion` |
| `xposed-scope-metadata` | hook | medium | `xposedscope` |
| `uploads-target-apk` | exfiltration | critical | `patch.chelpus.com/submit` |
| `plaintext-patch-endpoint` | network | high | `http://patch.chelpus.com` |
| `raw-ip-endpoint` | network | high | `109.95.209.70` |
| `raw-ip-endpoint-tls` | network | high | `149.56.155.246` |
| `affiliate-tracking-id` | tracking | medium | `appnext.com/myid.html` |
| `ad-sdk-tapjoy` / `ad-sdk-adsense` | tracking | low | `tapjoyads.com`, `googlesyndication.com` |
| `promo-supersu` / `promo-busybox` | promo | low | `eu.chainfire.supersu`, `stericson.busybox` |

Además: 20 permisos (entre ellos `ACCESS_SUPERUSER`, `REQUEST_INSTALL_PACKAGES`,
`REQUEST_DELETE_PACKAGES`, `QUERY_ALL_PACKAGES`, `MANAGE_EXTERNAL_STORAGE`,
`SYSTEM_ALERT_WINDOW`, `READ_PHONE_STATE`) y firmante **AOSP test**
(`CN=Android, O=Android, L=Mountain View, ST=California, C=US`) ⇒ **re-firmado**
desde otro APK.

## API

```rust
// ids únicos que casan en un texto (DEX, AXML o smali)
let ids = app_intel::detect_patcher_toolkit(&dex_text);
// id, clase, severidad y el marcador exacto que casó
let det = app_intel::detect_patcher_toolkit_detail(&dex_text);
// notas de impacto real en un móvil con bootloader cerrado
let notas = app_intel::patcher_toolkit_impact_notes();
// perfil del app (endpoints + features + el sha256 del dex medido)
let p = app_intel::profile_by_id("lucky-patcher").unwrap();
```

## Impacto real (sin dramatizar)

Con **bootloader cerrado** (`flash.locked=1`) y AVB `green`, un APK firmado con
claves de test de AOSP **no pasa** la verificación ⇒ **no puede parchear apps de
sistema**. Los riesgos reales son:

1. pide **root** (`ACCESS_SUPERUSER`);
2. es **módulo Xposed/LSPosed** (hook de cualquier app si hay LSPosed);
3. **sube el APK que parchea** a un tercero (`patch.chelpus.com/submit`);
4. habla por **HTTP sin cifrar** y con **IPs crudas** ⇒ interceptación trivial;
5. tracking de afiliado (`appnext`) y SDKs de anuncios.

⇒ Recomendación al usuario: **desinstalarlo** (o, como mínimo, no darle root).

## Regla general que sale de C2

**No detectes el *paquete* ofuscado: detecta los artefactos.** Un mismo binario
puede cambiar de nombre y de icono cada semana (aquí `chelpus` → `ru.drjmwdme.*`);
los hashes del contenido y los marcadores embebidos no. Por eso el escáner
debe trabajar sobre **el APK**, no sobre `pm list packages`.
