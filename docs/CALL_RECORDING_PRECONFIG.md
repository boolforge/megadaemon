# Grabación de llamadas y Preconfig · qué se puede hacer en este móvil

Todo lo de aquí está **medido en el S21 Ultra real** (SM-G998B · Android 14 ·
One UI 6 · Magisk Kitsune v27.2 · SELinux Enforcing), no copiado de manuals.

## 1. Grabar el audio de la llamada: las cuatro vías y cuál vale aquí

| Vía | Qué necesita | ¿Sirve en este móvil? |
|---|---|---|
| **A. UID de sistema (1000)** | root (`su 1000`) | **Sí, es la vía buena**: el UID `system` está firmado con la clave de plataforma ⇒ tiene `CAPTURE_AUDIO_OUTPUT` ⇒ puede abrir `VOICE_CALL` |
| B. Parche de política de audio | Magisk systemless + `audio_policy_configuration.xml` | **No aquí**: la voz del módem se mezcla en el HAL, no hay mixport de voz que capturar |
| C. Parche binario de `libaudioflinger.so` | Rizin + parche de bytes | Innecesario: A da lo mismo sin tocar binarios |
| D. Grabador nativo de Samsung | **CSC THL/INS** | Sí: es lo que hace el botón de grabar del sistema |

### A · El permiso y por qué el UID de sistema lo resuelve

`MediaRecorder.AudioSource.VOICE_CALL` / `VOICE_UPLINK` / `VOICE_DOWNLINK`
exigen `android.permission.CAPTURE_AUDIO_OUTPUT`, que es
`signature|privileged`: **ninguna app normal lo obtiene** (y `RECORD_AUDIO`
no sirve). Pero el UID 1000 (`android.uid.system`) está firmado con la clave
de plataforma, así que lo tiene todo.

**Medido en el móvil:**
```
$ su 1000 -c id
uid=1000(system) gid=1000(system) groups=1000(system) context=u:r:magisk:s0
```

⇒ El grabador nativo (Rust, enlazado a `libaudioclient` con el NDK) se
lanza así:
```bash
su -mm -c "su 1000 -c '/data/local/tmp/md-rec --source 4 --out /data/local/tmp/llamada.pcm'"
```
`--source 4` = `VOICE_CALL`, `2` = uplink, `3` = downlink, `1` = micrófono.

**Lo que NO se puede prometer:** que el HAL exponga la voz del módem como
entrada capturable. Si el HAL sólo tiene salida de voz, el PCM sale en
silencio y **eso no lo arregla ningún parche de AudioFlinger**: se comprueba
grabando y mirando si el fichero no es todo ceros. Por eso el plan en Rust
(`call_recorder::capture_launch_plan`) **incluye la comprobación de silencio
como paso explícito**.

### B · La política de audio de este móvil

`/vendor/etc/audio_policy_configuration.xml` (+ `audio_policy_configuration_base.xml`):
mixPorts `primary-out`, `deep-buffer`, `fast`, `primary-in`, `a2dp input`;
devicePort `Voice Call Mic` (`AUDIO_DEVICE_IN_VOICE_CALL`) **enrutado a
`primary-in`**; **no existe ningún mixport de captura de llamada** y no se
declaran fuentes `AUDIO_SOURCE_VOICE_*`.

⇒ Un parche de política **añadiría** un mixport de captura, no "desbloquearía"
uno. En AOSP puro (donde la voz sí pasa por el sistema) eso funciona; en
Samsung, con la voz generándose en el módem, es dudoso. Implementado como
análisis, no como promesa: `call_recorder::audio_policy_facts()` decide si
hace falta parche.

HAL de Samsung en el móvil: `/vendor/lib64/hw/audio.primary.default.so`,
`audio.sec_primary.default.so`, `audio.r_submix.default.so`, `audio.a2dpsink.default.so`,
`audio.bluetooth.default.so`, `audio.usb.default.so`.

### D · La vía nativa de Samsung y su flag

El flag es **`CscFeature_VoiceCall_ConfigRecording`**:
- **No está en ningún XML del móvil**: `grep -rl` sobre `/prism`, `/optics`,
  `/system/etc`, `/vendor/etc` ⇒ **cero hits**. Los hits son **APKs**:
  `SamsungInCallUI.apk`, `SamsungDialer.apk`, `TelephonyUI.apk`,
  `SamsungContacts.apk`, `EmergencySOS.apk`.
- En el **string table del DEX de SamsungInCallUI** la cadena aparece pegada a
  la familia completa: `ConfigOpStyleForImsFunction`, `ForIncomingCall`,
  `ForLiveTranslate`, `ForMvno`, `ForRTT`, `ForSpamCall`,
  `DisableGeoDescription`, `EnableDisplaySdnNameDuringCall`,
  `SupportAutoAnsweringMemo`, `SupportCrtCrbtText`, `SupportHoldUiForVolte`.
- **No hay "CallRecording" en ninguno de los 60+ jars de `/system/framework`**
  ⇒ el grabador no está en el framework: está en una app.
- `dumpsys media.audio_policy` muestra `AUDIO_STREAM_VOICE_CALL` (id 13) y
  `stv_call_connection : AUDIO_STREAM_VOICE_CALL`.

⇒ El CSC decide si el **sistema** ofrece el botón de grabar. Con `AMO`
(America) no lo ofrece; con `THL`/`INS` sí. El camino para cambiarlo está
implementado en `rom_migration::csc_swap_via_mps_code()` (ver §3).

## 2. Preconfig: qué es, cómo se instala y por qué aquí no hace falta

`com.sec.android.Preconfig` es la app de **preconfiguración de operador** de
Samsung. En firmwares antiguos venía como `/system/app/Preconfig/Preconfig.apk`
(junto a `AutoPreconfig`), y se lanzaba con:
```bash
am start -n com.sec.android.Preconfig/.Preconfig   # o marcar *#272*<IMEI>#*
```
Si está en la imagen de sistema pero desinstalado para el usuario:
```bash
cmd package install-existing com.sec.android.Preconfig
```

**En este firmware NO existe**: `cmd package list packages -u | grep -i preconfig`
→ vacío; no hay `/system/app/*Preconfig*`; y no está en
`/prism/etc/omc_apks_list.txt` (la lista de APKs OMC). One UI 6 lo eliminó
del firmware genérico.

**Plan de instalación implementado** (`rom_migration::preconfig_install_plan`):
1. Si está en la imagen → `cmd package install-existing`.
2. Si no → extraer `Preconfig.apk` de un firmware Samsung que lo traiga e
   instalarlo como app de sistema (pide permisos de plataforma).
3. **Si ninguna** → **reimplementar su función**, que es exactamente lo que
   ya hace MegaDaemon: leer el CSC con `AT+PRECONFG=1,0` y escribirlo con
   `AT+PRECONFG=2,<CSC>`, o editar `mps_code.dat` con root.

⇒ **Conclusión:** para cambiar el CSC **no hace falta Preconfig**. Preconfig
es la interfaz; el mecanismo real es el módem (`AT+PRECONFG`) o el fichero
persistente (`mps_code.dat`).

## 3. El CSC en este móvil (medido)

- CSC actual: **`AMO`** (América).
- **El CSC persiste en `/mnt/vendor/efs/mps_code.dat`**: exactamente 3 bytes,
  `AMO`. Es el mismo fichero que escribe el módem con `AT+PRECONFG=2,<CSC>`.
- `/prism/etc/carriers/single/AMO/` contiene `contents.db`, `sales_code.dat`,
  `sysconfig`, `permissions`, `default-permissions`, `omc-default-permissions`,
  `cid`, `AMO_keystrings.dat`, `appmanager.conf`, `default_application_order.xml`,
  `default_workspace.xml`.
- `/optics/configs/carriers/single/AMO/conf` (segunda copia).
- `/prism/etc/` tiene además `omc_data.json` (11 685 B) y
  `omc_apks_list.txt` (los APKs OMC: Tim, Vodafone, Orange, Globel, Ignite…).
- `/prism` está montado **ro** ⇒ un cambio de CSC ahí necesita remount.
- `flash.locked=1` + AVB `green` ⇒ **bootloader cerrado (FMM Lock)**:
  el CSC **no se puede flashear** de forma permanente; sólo cambiar y
  revertir (`csc_swap_via_mps_code` + `csc_revert_plan`).

**Rutas candidatas para leer los flags** (`csc_feature_candidates`):
`/prism/etc/carriers/single/<CSC>/contents.db`, `.../sysconfig`,
`.../permissions`, `/optics/configs/carriers/single/<CSC>/conf`,
`.../default-permissions`. **Pendiente**: confirmar cuál guarda los
`CscFeature_*` (preguntado al nodo RE externo).

## 4. Lo implementado en el repo

- `call_recorder.rs`: `CaptureSource` (Mic/VoiceCall/VoiceUplink/VoiceDownlink/
  RemoteSubmix) con el valor de `MediaRecorder.AudioSource` y el permiso que
  exige; `capture_launch_plan()` (UID de sistema con root, o micrófono sin
  root, con sus límites escritos); `audio_policy_facts()` (¿hace falta parche?);
  `parse_csc_features()` / `csc_feature()` / `voice_call_config_recording()`;
  `csc_feature_candidates()`.
- `rom_migration.rs`: `PRECONFIG_PACKAGE`, `preconfig_state()`,
  `preconfig_install_plan()`; y `flash_plan()` ahora **bloquea** si el
  bootloader está cerrado (antes el parámetro `device` no se usaba).
- Tests: `tests/call_audio_smoke.rs` (8 tests nuevos) + los tres arneses
  previos. Verde: `call_audio` 41/41 · `rom_migration` 29/29 ·
  `backup_full` 26/26 · `call_recorder` 18/18.

---

## 5. Respuestas del nodo RE externo (medidas, §9 de su Manual)

El nodo RE (`9cb64913`) respondió a las 7 preguntas con mediciones sobre el
móvil vivo. Incorporadas:

**P1 · Qué app puede grabar llamadas.** Barrido de **398 APKs** (5.580 MB)
buscando `CAPTURE_AUDIO_OUTPUT` en manifiesto y en DEX:
| APK | ¿manifiesto? | veredicto |
|---|---|---|
| `/system/priv-app/YourPhone_P1_5` (Microsoft Your Phone) | **sí** | declara el permiso |
| `/system/priv-app/LinkToWindowsService` (Microsoft) | **sí** | declara el permiso |
| `/system/priv-app/GalaxyApps_OPEN` (Galaxy Store) | no | sólo lo referencia (falso positivo) |

⇒ **Ninguna app de Samsung declara el permiso**, y el grabador no está en los
jars de `/system/framework` ⇒ **en este firmware (CSC `AMO`) la grabación
nativa no la implementa ninguna app**.

**P3 · `contents.db` es SQLite** (156.672 B): tabla
`contents(salesCode TEXT, _id INTEGER, filePath TEXT, fileSize INTEGER)` con
**2.006 filas** (22 de AMO, 131 apuntan a un `cscfeature.xml`). ⇒ es un
**inventario de ficheros** del CSC (ruta + tamaño esperado), **no** un almacén
de valores `CscFeature_*`. Sirve para auditar que un parche no rompa el tamaño.

**P4 · El formato OMC NO es gzip+XOR.**
`/optics/configs/carriers/single/AMO/conf/system/cscfeature.xml` = **1.109 B**,
cabecera **`2f 27 29 b7 da c1 ac 37`** ⇒ envoltorio OMC **binario propio**.
No hay APK decodificador OMC en `/prism/app` (11 APKs medidos).
Implementado: `call_recorder::omc_state()` distingue texto / gzip / OMC.

**P6 · El HAL no expone captura de voz.** `libaudiohal.so` (33.184 B) tiene
**cero** cadenas `voice|voip|csd|VOICE_CALL|capture|in_call` ⇒ el audio de la
llamada lo genera el **modem** ⇒ **ningún parche de AudioFlinger lo arregla**.

**P2 · `dex_decompile` tenía un bug real** (`encoded_value` no soportaba
`VALUE_NULL` 0x1e / `VALUE_BOOLEAN` 0x1f) ⇒ **ya corregido** por el agente DEX
+ CLI `megadaemon-core dex`; los DEX están sanos (SamsungInCallUI: 30.480
cadenas, 10.019 clases).

**P7 · Pendiente**: la **P5 de Swift Backup** (derivación AEGIS, `.splits`,
`Config`/`Blacklist`, cloud, Chrome).

### Veredicto final (implementado en `CallAudioFacts::conclusion()`)

1. **Cambiar el CSC** a `THL`/`INS` ⇒ el propio sistema graba, sin hacks
   (`csc_swap_via_mps_code`).
2. **UID de sistema (1000) + `VOICE_CALL`** ⇒ tiene el permiso, pero con el HAL
   medido hay que **comprobar que el PCM no sale en silencio**.
3. **Microsoft Your Phone / Link to Windows** ya declaran el permiso.
