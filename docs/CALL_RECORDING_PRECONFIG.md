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

---

## 6. La vía que SÍ funciona: **capturar la voz del modem por tinyalsa** (sin parche, sin permisos)

**Medido en el S21 Ultra:**

- `libtinyalsa.so` está en `/system/lib64/` (45.024 B) y **no** exporta los
  wrappers `tinyalsa_*`: exporta la API baja (`pcm_open`, `pcm_read`,
  `pcm_start`, `pcm_close`, `pcm_is_ready`, `mixer_*`). El capturador lo carga
  con `dlopen` ⇒ binario sin cabeceras y sin enlazar.
- `/proc/asound/pcm` lista **`00-12: WDMA0` … `00-19: WDMA7` como `capture`**
  (card 0, devices 12-19). En el Exynos 2100 los WDMA son la **interfaz de voz
  AP↔CP (SIFM)**: lo confirma `/vendor/etc/mixer_paths.xml`, donde
  `ABOX SIFM0..7 = WDMA` y se fijan `ABOX WDMA0 Width/Channel/Rate`.
- **`pcm_open(card=0, dev=12..19, CAPTURE)` FUNCIONA con root** (puntero válido,
  `errno=0`, medido en los devices 0, 1, 4, 12, 13, 14 y 15). Los nodos
  `/dev/snd/*` son `system:audio 0660` ⇒ hace falta root o el grupo `audio`.
- `pcm_read` **se queda bloqueado** cuando no hay llamada: es lo esperado (no
  hay audio fluyendo). ⇒ **la confirmación final necesita una llamada en
  curso**.

⇒ **Por qué esto explica la P6 del nodo RE**: el HAL de usuario no tiene
ninguna cadena de voz porque **la voz no pasa por el HAL**; pasa por el WDMA.
Por eso `libaudiohal.so` no la menciona y por eso el `VOICE_CALL` estándar
puede devolver silencio.

**Capturador** (`md_pcm`, ARM64, NDK, `dlopen` de tinyalsa):
```bash
md_pcm --card 0 --dev 12 --sec 75 --out /data/local/tmp/voice/wdma12.raw
```
Devuelve 0 si el PCM **no** es silencio y 2 si lo es ⇒ la comprobación de
"¿hay voz?" es automática.

**Lo que falta**: (1) saber cuál de los 8 WDMA es el *uplink* y cuál el
*downlink* (⇒ hay que capturar durante una llamada); (2) el formato nativo de
cada PCM (ancho de muestra, canales, frecuencia).

## 7. Lo que encontró el agente DEX (y encaja con lo anterior)

En el DEX de SamsungInCallUI (`classes.dex`, 30.480 cadenas):

- El grabador **no** es un `MediaRecorder` estándar: es
  **`com.samsung.android.ims.SemCmcMediaRecorder`** (clase *package-private*)
  con la fuente de audio **`PT_AS_VOICE_CALL`** (uplink de la llamada).
- Hay **doble implementación** (`MediaRecorder` estándar **y** `SemCmc`) y el
  flag del CSC elige entre las dos: `Record - MediaRecorder is null.`,
  `Record - SemCmcMediaRecorder is null.`.
- La tubería de audio es el framework **"Phoebus"**:
  `AudioSessionImpl` + `UtteranceRecorderInput` (**con `LocalBinder`**) +
  `RecordingRepository`/`RecordingStorage` + `AutoRecordingIndicator`.
- `isAecRecording` ⇒ el sistema graba **con AEC** (audio ya procesado).
- Hay un **diálogo de aviso** antes de grabar
  (`RecordingDisclaimerDialogView`, `KEY_CALL_RECORDING_DISCLAIMER`) ⇒ el CSC
  gobierna también el consentimiento.

⇒ **Vía nativa = el propio Samsung**, no un hook: con el CSC correcto, el
framework Phoebus se encarga. Eso encaja con que **12 apps tengan
`CAPTURE_AUDIO_OUTPUT` concedido en ejecución** (entre ellas
`com.samsung.android.incallui` y `com.samsung.android.callassistant`), y con
que el permiso **esté concedido a `uid=1000`**.


## 8. El formato OMC: **descifrado** (rotación + XOR posicional)

El nodo RE lo cerró (referencia `soulr344/OMCDecoder`, `include.h`):

```
OMC = gzip(XML) con el envoltorio:  out[i] = rotl(b[i], SHIFTS[i%256]) XOR SALTS[i%256]
```
La **clave gira cada 256 bytes** (por eso el índice de coincidencia salía
aleatorio y no había periodicidad). **No es XOR simple ni AES.**

**Portado a Rust** en `rom_migration.rs` (sin crates nuevos: `flate2` ya
estaba en `Cargo.toml`):
- `omc_decode()` / `omc_encode()` — el envoltorio, byte a byte.
- `omc_decode_gzip()` / `omc_encode_gzip()` — OMC ↔ XML plano.
- `csc_feature_get()` / `csc_features()` / `csc_feature_set()` — leer y
  escribir banderas `<CscFeature_X>VALOR</CscFeature_X>`.
- `omc_feature_patch_plan()` — el plan completo de parche.

**Verificado contra el fichero real del móvil** (no contra un fixture): el
`cscfeature.xml` de AMO (1.109 B, sha16 `ef110140f7bb95e1`) decodifica a
`<SalesCode>AMO</SalesCode>`, `<Country>SPAIN</Country>`,
`<CountryISO>ES</CountryISO>`; y `encode(decode(x)) == x` en los **126/126**
CSC.

### Qué bandera hay que poner (dato duro)

`CscFeature_VoiceCall_ConfigRecording` **sólo existe en 19 de los 126 CSC**:

| Valor | CSC |
|---|---|
| `RecordingAllowed` (sin aviso) | **THL, ILO, XXV**, BKD, BTC, CAL, EGY, ILP, LYS, MYM, NPL, SEK, SLK, TUN |
| `RecordingAllowedByMenu,disclaimer_latin` | CHO, EON, GTO, TPA |
| `RecordingAllowedByMenu,disclaimer_india` | INU |
| **ausente** | **AMO** (y 106 más) ⇒ hoy no hay grabación nativa |

### El parche mínimo (y por qué NO cambiar el CSC entero)

AMO = **Orange España** (`<Country>SPAIN</Country>`, `contents.db` →
`config-com.orange.update.xml`, OOBE `com.aura.oobe.samsung`). Cambiar
AMO→THL/ILO/XXV **no es cambiar un flag**: cambia el perfil de operador
(OOBE, permisos por defecto, APN/VoLTE, apps del carrier) ⇒ te puedes quedar
sin la configuración de Orange.

⇒ **Lo correcto es parchear el `cscfeature.xml` de AMO** añadiéndole el
`<CscFeature_VoiceCall_ConfigRecording>RecordingAllowed</CscFeature_VoiceCall_ConfigRecording>`
que hoy no existe (así lo decide el XML de THL/ILO/XXV).

**Pasos exactos** (`omc_feature_patch_plan()`):
1. Copia de seguridad del fichero.
2. Decodificar → insertar el flag → re-codificar.
3. **`/optics` está en solo-lectura** ⇒ `mount -o remount,rw /optics` (root) o
   un **módulo Magisk systemless** que haga bind-mount del fichero parcheado.
4. Escribir el fichero parcheado.
5. **Actualizar `contents.db` a la vez**: guarda el `fileSize` de cada fichero
   del CSC ⇒ si el tamaño no cuadra, el inventario del CSC no cuadra.
6. Reiniciar y comprobar.
7. Revertir = restaurar fichero + `fileSize` original + reiniciar.

**Aviso**: el magic `2f2729b7` **no está en ninguno de los 2.476 ficheros del
firmware** ⇒ el decodificador **no está en el móvil** (está en las
herramientas de build de Samsung). El consumidor en runtime (el que aplica el
flag) hay que buscarlo entre los que **leen** la feature: `services.jar`
(29 hits), `vsimmanager.jar` (428), `telephony-common.jar` (19),
`imsmanager.jar` (3).

## 9. Cambio de CSC: datos reales y riesgo

- SIM: **JAZZTEL, MCC/MNC 21403** (España). CSC actual **AMO**.
- `mps_code.dat` = 3 bytes `AMO`, en `/mnt/vendor/efs` (**rw**) ⇒ escribible
  con root. `/prism` (sda29) y `/optics` (sda30) están **ro** ⇒ la vía de
  copiar el directorio necesita remount.
- CSC de grabación presentes: **THL (630 B), ILO (422 B), XXV**; `INS` **no**
  está en este firmware.
- El CSC **no** cambia la SIM ni el MCC/MNC: el módem sigue registrando en la
  red de la SIM. Lo que cambia es el juego de configuración (APN, IMS/VoLTE,
  MMS y las banderas `CscFeature_*`) ⇒ riesgo real de perder VoLTE o datos,
  **reversible** restaurando los 3 bytes y reiniciando.
- Después del cambio hay que comprobar: `ro.csc.sales_code`, coherencia del
  baseband y si el InCallUI habilita `CscFeature_VoiceCall_ConfigRecording`.

---

## 10. Estado final de la vía ALSA (y por qué no se sigue por ahí)

Secuencia medida con el capturador (`md_pcm`, `dlopen` de
`/system/lib64/libtinyalsa.so`):

| Paso | Resultado medido |
|---|---|
| `pcm_open(card=0, dev=12..19, CAPTURE)` | **OK** (puntero válido, `errno=0`) en los devices probados (12-15, 0, 1, 4) |
| `pcm_state(pcm)` | **SIGSEGV** dentro de la librería |
| `pcm_start(pcm)` (sin `pcm_state`) | **SIGSEGV** ⇒ el crash está en `pcm_start` |
| `pcm_read` | no llega a ejecutarse |

**El móvil sigue sano**: en `dmesg` no hay oops ni panic; sólo mensajes
normales del driver **`abox 18c50000.abox`** (`channels_put`, `width_put`,
`abox_disable`) ⇒ el driver ABOX (Audio Block) es la interfaz de voz CP↔AP y
se está cerrando con orden. El crash es **en espacio de usuario** (la librería
tinyalsa), no un oops del kernel.

**Decisión: no seguir pinchando el driver de voz.** Razones:
1. `pcm_start` requiere la ruta de voz **configurada** (controles de mixer
   `ABOX SIFM0..7` de `/vendor/etc/mixer_paths.xml`) y **una llamada en
   curso**; sin las dos cosas el ioctl revienta.
2. Un ioctl equivocado sobre el ABOX **sí** puede tumbar el kernel en otros
   firmwares ⇒ no vale la pena arriesgar el móvil del enjambre para una
   prueba que además necesita una llamada que nadie puede hacer desde aquí.
3. La vía **real y soportada** es la del framework (§7): con el CSC
   correcto, `Phoebus`/`SemCmcMediaRecorder` graban solos, con AEC, y el
   sistema pone su indicador. No hay que tocar drivers.

⇒ **El camino que queda, y es el bueno**: **cambiar el CSC** (`mps_code.dat`
⇒ `ILO`/`THL`/`XXV`) para que el propio Samsung habilite
`CscFeature_VoiceCall_ConfigRecording`, o **descifrar el OMC** para editar el
flag sin cambiar de CSC. La primera está implementada y es reversible; la
segunda es un ataque de criptografía con 126 muestras y una pista nueva
(el prefijo común de 10 bytes).

## 11. Resumen de las tres rutas y su estado

| Ruta | Estado | Qué falta |
|---|---|---|
| **A · Grabador nativo de Samsung** (CSC `THL`/`ILO`/`XXV`) | Implementada y reversible (`mps_code.dat`, 3 B) | Un reinicio (afecta a la flota) y confirmar que el CSC destino no rompe APN/VoLTE |
| **B · Editar el flag OMC directamente** | Algoritmo OMC pendiente (ni XOR ni bloque; cifrado de flujo) | El decodificador; dataset de 126 muestras ya entregado al nodo RE |
| **C · Capturar la voz del modem por ALSA** | `pcm_open` funciona; `pcm_start` revienta sin llamada | Ruta de voz configurada + llamada en curso ⇒ **no se sigue** (riesgo de kernel) |
| **D · Grabador propio con `CAPTURE_AUDIO_OUTPUT`** | Permiso concedido a `uid=1000` (medido) | El HAL **no expone voz** ⇒ devolvería silencio; el que funciona es el de Samsung |
| **E · Preconfig** | **No existe en este firmware**; su función ya está cubierta por `AT+PRECONFG` y `mps_code.dat` | — |
