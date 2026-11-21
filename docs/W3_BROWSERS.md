# W3 · Navegadores: inventario real, copia y sincronización · RE de Swift Backup

> Medición sobre el S21 Ultra del usuario (SM-G998B, Android 14, Magisk/root).
> Todo lo que hay aquí sale del móvil, no de catálogos de internet: la lista
> de navegadores seSacó del *resolver table* de `dumpsys package` y de las
> firmas de perfil en `/data/user/0/<pkg>`.

---

## 1. Cuántos navegadores hay (y por qué "tropecientos" es correcto)

Hay **tres cifras distintas**, y la confusion viene de no separarlas:

| Cifra | Qué es | Cómo se mide |
|---|---|---|
| **13** | Navegadores **reales**: app con perfil de navegador propio | firma de perfil en `/data/user/0/<pkg>`: `app_chrome/Default/Bookmarks` (Chromium), `files/mozilla/` (Gecko), `app_sbrowser/` (Samsung) |
| **515** | Apps que **pueden abrir una web** (`VIEW` + `BROWSABLE` + http/https o `text/html`) | resolver table de `dumpsys package` |
| **~500** | Apps con perfil **WebView** (`app_webview/`) | escaneo de `/data/user/0/*` |

Los 515 no son navegadores: son apps de banca, compras, noticias y PDF que
abren enlaces con Custom Tabs. **Respalden los ficheros del usuario, no el
perfil WebView**: su masa es caché regenerable.

### Los 13 navegadores reales, con su motor y su peso

| Navegador | Paquete | Motor | Datos | Sincronización nativa |
|---|---|---|---|---|
| Chrome | `com.android.chrome` | Chromium | **1,2 G** | Google Sync |
| Chrome Dev | `com.chrome.dev` | Chromium | 160 M | Google Sync |
| **Thorium** | `org.chromium.thorium` | Chromium | 274 M | **ninguna** |
| Edge | `com.microsoft.emmx` | Chromium | 95 M | Microsoft Edge Sync |
| Firefox | `org.mozilla.fenix` | Gecko | **1,1 G** | **Firefox Sync** (E2E) |
| Iceraven | `io.github.forkmaintainers.iceraven` | Gecko | **2,3 G** | Firefox Sync (fork de Fenix) |
| Fennec | `us.spotco.fennec_dos` | Gecko | 24 M | Firefox Sync |
| Tor Browser | `org.torproject.torbrowser` | Gecko | 58 M | **ninguna** (a propósito) |
| Tor Browser Alpha | `org.torproject.torbrowser_alpha` | Gecko | 88 M | ninguna |
| Samsung Internet | `com.sec.android.app.sbrowser` | Samsung | 545 M | Samsung Pass |
| Samsung Internet Beta | `com.sec.android.app.sbrowser.beta` | Samsung | 214 M | Samsung Pass |
| FixupXer | `com.fixupxer` | WebView | 533 K | ninguna |
| MyAllVideoBrowser | `com.myAllVideoBrowser` | WebView | 144 M | ninguna |

Fuera de navegadores, el mayor bloque del móvil es `com.agy` (Antigravity) con
**14 GB** de datos WebView, y `com.rk.xededitor` con 1,5 GB.

⇒ **Los datos de navegador del móvil (~5,5 GB) y el resto de apps suman más
que los 5,9 GB libres.** Por eso el plan de backup es *streaming a Drive por
trozos con SHA-256*, nunca una copia local intermedia.

### Dónde vive cada cosa

| Motor | Rutas dentro de `/data/user/0/<pkg>` |
|---|---|
| Chromium | `app_chrome/Default/{Bookmarks,History,Cookies,Preferences,Web Data}`, `app_chrome/Local State` |
| Gecko | `files/mozilla/profiles.ini` → `files/mozilla/<hash>.default/{places.sqlite,cookies.sqlite,formhistory.sqlite,content-prefs.sqlite,cert9.db,key4.db,logins.json}` |
| Samsung | `app_sbrowser/Default/...`, `app_sbrowser/Local State` |
| WebView | `app_webview/Default/...` (perfil compartido) |

---

## 2. Estrategia de copia y sincronización

`backup_full::browser_sync_strategy()` la genera. La prioridad es siempre la
**sincronización nativa**, porque es la única que replica de verdad las
contraseñas:

1. **Sincronización nativa** (Firefox Sync, Google Sync, Edge Sync, Samsung
   Pass). Es la única vía que replica contraseñas: están cifradas con la
   clave del Android Keystore, que **no sale del dispositivo**.
2. **Copia root del perfil** con SHA-256 por fichero, sólo de lo portable
   (marcadores, historial, preferencias), **nunca la caché**.
3. **Exportación de marcadores** a HTML/JSON como red de seguridad: portable
   entre cualquier navegador.
4. **Verificación en destino**: SHA-256 de cada fichero comprobado antes de
   dar el backup por bueno.
5. **Lo atado al keystore se documenta, no se copia**: `Local State`
   (`os_crypt.encrypted_key`), `key4.db`, `logins.json`, `Cookies`.

Firefox Sync es **la mejor vía del lote**: cifra de extremo a extremo con la
contraseña de la cuenta (no la del keystore), así que marcadores,
contraseñas e historial **sí** son portables entre dispositivos.

Thorium y Tor **no tienen sincronización**: su réplica es la copia root del
perfil + exportación a HTML.

### Lo que NO entra en el plan

- `cache/`, `code_cache/`, `GPUCache/`, `DawnCache/`, `Service Worker/CacheStorage`,
  `blob_storage/` → regenerable.
- Los ~500 perfiles `app_webview/` → caché de apps que abren enlaces.
- Cookies y contraseñas cifradas con clave del keystore → zona privada.

---

## 3. RE de Swift Backup (v5.1.1, instalada en el móvil)

`org.swiftapps.swiftbackup` **está instalada** en este móvil (`pm path` →
`/data/app/~~pbwhtiYXFh4xDN_dToeH5A==/org.swiftapps.swiftbackup-rmFvjZ4wVP1zY_fpVoXM3A==/base.apk`,
15,4 MB, `versionName=5.1.1`, actualizado 2026-11-15).

Se descomprimió el DEX en el propio móvil (`classes.dex` 9,27 MB +
`classes2.dex` 2,84 MB) y se parseó la **tabla de strings del DEX** con un
script propio (`dexstr.py`) para sacar las constantes y las clases.

### Lo que se ha medido

| Hecho | Evidencia |
|---|---|
| Partes que respaldo por app: **APP, DATA, EXTDATA, EXPANSION, MEDIA** | journal `SMessage`: `Props=Restore(appParts=[APP, DATA, EXTDATA, EXPANSION, MEDIA], restorePermissionsMode=Granted, restoreSpecialPermissions=true, restoreSsaid=true, …)` |
| Cifrado: **AEGIS-256** (+ AEGIS-128X2) | strings `AEGIS-256`, `Aegis256`, `SBA1-AEGIS256-key-check-v1`, `SBA AEGIS encryption backend is not available` |
| Compresión: **Zstandard** | `ZSTD`, `Native zstd compress bytes failed`, `Invalid zstd compression level` |
| Esquema de ficheros | `.app` `.apk` `.dat` `.splits` `.extdat` `.med` `.extra` (apps) · `.fld` `.flm` `metadata.json` (carpetas) · `.cls` (llamadas) `.msg` (SMS) `.wal` (fondos) `.wfi` (Wi-Fi) |
| Layout en disco | `/storage/emulated/0/SwiftBackup/accounts/<accountHash>/backups/apps/local/<pkg>/<backupId>/` |
| Backups en nube **etiquetados por modelo** | `postConnectActions: Tags=[SM-G930F, SM-G998B]` |
| Reglas por app | `appconfigs/data/Config`, `Blacklist`, `app_cached_data` (1.737 apps con `isInstalled/isEnabled/isLaunchable`) |
| Chrome tiene **tratamiento especial** | strings `com.android.chrome`, `com.chrome.beta`, `com.chrome.dev`, `com.google.android.apps.chrome` |
| Tamaño de su propia base | 716 MB / 7,9 M mensajes (`SMessage`): es un journal de todo lo que hace |

**Corrección importante**: el módulo LSPosed que documenta Swift Backup dice
que cifra con *Facebook Conceal*; la versión instalada (5.1.1) usa **AEGIS-256
propio** (Conceal era de versiones anteriores). Se ha medido, no supuesto.

### El formato, al detalle (medido en la nativa)

El APK trae `libsba_archive.so`, que exporta **el algoritmo exacto**:
**AEGIS-256** (y AEGIS-128X2) por trozos, **Argon2id** para derivar la clave
(rangos: iteraciones 1..100, memoria 16–64 MiB, paralelismo 1..8, salida
16–128 B) y el contenido en **TAR comprimido con ZSTD** (magics `SBA1` /
`SBA_SWIFTTAR`, AAD/nonce por trozo con las etiquetas
`SBA1-AEGIS256-entry-chunk-aad-v1` / `-nonce-v1`). El detalle completo, con
el método (los **DEX del mirror son idénticos a los del móvil** ⇒ el RE se
hace entero en local) está en **`docs/SWIFT_BACKUP_FORMAT.md`**.

### Lo bueno que se reimplementa en MegaDaemon

1. **Partes explícitas por app** (APP / DATA / EXTDATA / EXPANSION / MEDIA):
   igual que en `backup_full::LevelSpec`.
2. **Etiquetas por dispositivo** en la nube: una instancia sólo restaura lo
   que es de su modelo. En MegaDaemon: `drive_upload_plan()` por instancia,
   con el `vault_id` por instancia.
3. **Cifrado propio del pack** con clave derivada de hardware:
   `backup_vault::derive_hardware_key()` (PBKDF2 sobre el secreto del
   dispositivo) + AES-256-GCM con AAD `rel_path || 0x00 || idx`.
4. **Diff incremental**: Swift Backup compara "APK size/version/data size"
   antes de descargar (`AppDataChangeChecker: No change in APKs size…`). En
   MegaDaemon es el `sha256` por fichero del manifiesto: si no cambia, no se
   vuelve a subir.
5. **Etiquetas de "protegido" y notas** por backup: útil para distinguir
   "copia verificada" de "copia parcial".
6. **Reintento de subida reanudable por rangos** (ya implementado:
   `drive_chunk_plan()` + `drive_chunk_command()`).

### Otras herramientas open source revisadas

| Herramienta | Lo que aporta |
|---|---|
| **Neo Backup** (fork de OAndBackupX, GPL-3) | backup profundo con root, sólo local; el modelo "tar de /data/data con root" |
| **DataBackup** (FOSS) | lo mismo, más simple |
| **tarb** (CLI) | backup de apps+datos y ajustes de sistema **desde la línea de órdenes**: es el estilo que encaja en MegaDaemon (`deploy/backup_phone.sh`) |
| **Seedvault** | usa las mismas APIs internas que `adb backup`; en Android moderno casi todo el mundo se ha retirado de ese marco (las apps lo excluyen), y por eso hace falta root |

---

## 4. Cómo se ejecuta

```sh
# Manifiesto con sha256 por fichero y por trozo (sin copiar nada)
deploy/backup_phone.sh --src /data/local/tmp/megadaemon/state --list

# Subida por streaming (chunks + sha256, sin copia local intermedia)
deploy/backup_phone.sh --src /data/local/tmp/megadaemon/state \
    --session "$SESSION_URI" --out MegaDaemon/estado --manifest state-manifest.json

# Verificación: relee de Drive y compara los sha256
deploy/backup_phone.sh --src ... --session ... --file-id "$FILE_ID" --verify
```

Desde Rust: `backup_full::build_pack()` (filtra la zona privada),
`backup_vault::seal_dir_streaming()` (cifra sin cargar nada entero),
`backup_vault::restore_verify()` (las 3 lecturas reales del restore),
`backup_vault::drive_chunk_plan()` (trozos para Drive).
