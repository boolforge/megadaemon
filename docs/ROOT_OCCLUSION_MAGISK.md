# Ocultación de root + módulos Magisk (Shamiko) + banco de pruebas

> **Todo lo de este documento está MEDIDO en el móvil** (Galaxy S21 Ultra
> SM-G998B, Exynos 2100, Android 14, **Kitsune Mask 27.2**, Zygisk activo).
> Fecha de las mediciones: **2026-11-22**. Donde algo no se pudo medir, está
> marcado como `PENDIENTE` con el motivo; no se rellena de suposición.
>
> Binario del banco de pruebas: `megadaemon-core rootcheck` (CLI) y la tool MCP
> `root_occlusion_audit`. Código: `rust-core/src/root_occlusion.rs`,
> `rust-core/src/magisk_module.rs` (gestión real), `rust-core/src/android.rs`
> (tool MCP), `rust-core/src/agent_core.rs` (ruta del agente).

---

## 1. Estado real de la ocultación HOY (medido)

### 1.1 Lo que sí está oculto

| Señal | Valor medido | Veredicto |
|---|---|---|
| `ro.debuggable` | `0` | **oculta** |
| `ro.build.tags` | `release-keys` | **oculta** |
| `ro.build.type` | `user` | **oculta** |
| `ro.secure` | `1` | **oculta** |

Comando (root, en el móvil):

```
getprop ro.debuggable; getprop ro.build.tags; getprop ro.build.type; getprop ro.secure
→ 0 / release-keys / user / 1
```

### 1.2 Lo que NO está oculto (esto es lo que detectan las apps)

Medido **desde dentro del proceso de la app** (`/proc/<pid>/mounts` y
`/proc/<pid>/root/...` del propio proceso, con su UID y su contexto SELinux
`u:r:untrusted_app:s0`):

| Señal | Medido en el móvil | ¿La ocultamos hoy? |
|---|---|---|
| `/system/bin/su` | **visible** (symlink a `./magisk`) | **NO** |
| `/system/bin/magisk` | **visible** (441 088 B) | **NO** |
| `/system/bin/magiskpolicy` | **visible** (331 520 B) | **NO** |
| `/data/adb` | **visible** en la raíz de la app | **NO** |
| `/debug_ramdisk` | **visible** | **NO** |
| `/proc/<pid>/mounts` con "magisk" | **16 entradas** (20 en `io.github.vvb2060.magiskdetector`) | **NO** |

Comandos exactos:

```
ls -la /system/bin/su            → lrwxrwxrwx ... /system/bin/su -> ./magisk
grep -ci magisk /proc/<PID>/mounts → 16
```

**Conclusión**: hoy la ocultación es de *props*, no de sistema de ficheros.
Por eso un detector que mira `su`, los binarios de Magisk o los montajes
**falla**. La causa es que el DenyList de Magisk está **sin enforcement**
(`denylist=0`) y el MagiskHide de Kitsune no unmounta el espejo de Magisk.

### 1.3 Estado real de Magisk (medido)

```
$ magisk -v
v27.2-kitsune-4:MAGISK:R
$ magisk -V
27002
$ magisk --sqlite "SELECT key,value FROM settings;"
key=bootloop|value=0
key=sulist|value=1
key=safemode|value=0
key=zygisk|value=1
key=denylist|value=0
key=magiskhide|value=1
```

| Ajuste | Valor | Lectura |
|---|---|---|
| `zygisk` | `1` | Zygisk **activo** (necesario para Shamiko/LSPosed) |
| `denylist` | `0` | DenyList **sin enforcement** ⇒ los montajes de Magisk siguen visibles |
| `magiskhide` | `1` | MagiskHide de Kitsune activo, pero **no unmounta** el espejo (medido: `su` sigue visible) |
| `sulist` | `1` | SuList de Kitsune activa (quién puede pedir `su`) |

Módulos instalados en `/data/adb/modules/`: **25, todos ENABLED** (entre ellos
`megadaemon_csc_recording`, `playintegrityfix`, `tricky_store`,
`zygisk_lsposed`, `luckypatcher_zygisk`, `luckypatcher_inapp_zygisk`,
`zygisk-detach`). **No hay ningún módulo Shamiko.**

---

## 2. La tabla app → señal → veredicto

Cada fila se compone de tres mediciones independientes:

* **(a) estático** — tabla de cadenas de cada `classes*.dex` (no un `grep`) y
  bytes crudos de las `.so` nativas ⇒ *qué busca la app*;
* **(b) runtime** — qué ve la app de verdad desde su namespace
  (`/proc/<pid>/mounts`, `/proc/<pid>/root/...`) ⇒ *qué está a la vista*;
* **(c) ejecución** — la app arranca con el logcat limpio y se captura su log
  filtrado ⇒ *qué dice el detector*.

> **PENDIENTE (a la fecha de este párrafo)**: la tabla final la genera el banco
> de pruebas (`megadaemon-core rootcheck ...`) y se pega aquí con su salida
> real. El veredicto por app se calcula cruzando (a) con (b): `PASS` si todas
> las señales que la app mira están ocultas, `FAIL` si queda alguna a la vista,
> `PARCIAL` si hay señales sin medir.

---

## 3. El banco de pruebas (repetible)

### 3.1 Qué hay y cómo se llama

| Vía | Comando | Qué hace |
|---|---|---|
| CLI | `megadaemon-core rootcheck <paquete>[,<paquete>...]` | Tabla completa: estático + runtime + veredicto |
| CLI | `megadaemon-core rootcheck --state` | Qué señales quedan a la vista ahora mismo |
| CLI | `megadaemon-core rootcheck --static <paquete>` | Sólo el escaneo estático del APK |
| CLI | `megadaemon-core rootcheck --exec <paquete>` | Arranca la app y captura su logcat |
| CLI | `megadaemon-core rootcheck --csc` | Valor real de la feature de grabación del CSC |
| MCP | tool `root_occlusion_audit` | Lo mismo, con `mode=audit|state|exec|static|csc` |
| Agente | task `root.occlusion.audit` | `{"kind":"root.occlusion.audit","payload":{"packages":[...],"mode":"audit"}}` |
| Agente | task `magisk.modules` | `{"kind":"magisk.modules","payload":{"action":"list\|install\|shamiko",...}}` |

Envoltorios con el nombre pedido:

```bash
tools/md-rootcheck <paquete>      # = megadaemon-core rootcheck <paquete>
tools/md-magisk list|enable|disable|install
```

### 3.2 (a) Prueba estática

Se lee el APK **en el dispositivo** (el binario corre allí, así que usa E/S
local y no baja el APK entero): se abre el ZIP, se extrae **la tabla de
cadenas** de cada DEX (cabecera `string_ids` → `uleb128` + MUTF-8) y se
buscan las cadenas de las `.so` en crudo.

Catálogo de señales vigiladas (`SIGNALS` en `root_occlusion.rs`):

`su_binary`, `su_exec`, `magisk_binary`, `magisk_mention`, `superuser_app`,
`build_props`, `proc_mounts`, `getprop`, `root_library`, `attestation`,
`hook_framework`, `busybox`, `root_api`.

### 3.3 (b) Prueba de runtime (la que de verdad importa)

Un `su` ejecutado desde root **no** prueba nada: se ejecuta en el namespace de
init y con contexto `u:r:magisk:s0`. La prueba real:

1. se lanza la app (`am start -W -a MAIN -c LAUNCHER <pkg>`);
2. se saca su PID;
3. se lee **`/proc/<pid>/mounts`** (namespace de montajes **de la app**) y
   **`/proc/<pid>/root/...`** (su raíz);
4. se leen las props tal y como las vería (`/proc/<pid>/root/system/build.prop`).

UID y contexto reales de los procesos medidos: `u:r:untrusted_app:s0`,
`u:r:untrusted_app_25:s0`, `u:r:untrusted_app_30:s0`, `u:r:untrusted_app_32:s0`.

### 3.4 (c) Prueba de ejecución

`logcat -c` → arranque de la app → 5 s → `logcat -d --pid=<pid>` filtrado por
palabras clave (`root`, `magisk`, `supersu`, `busybox`, `xposed`, `frida`,
`test-keys`, `tamper`, `integrity`, `safetynet`, ...).

> Si el detector **no escribe en logcat** (muchos no lo hacen), la prueba de
> ejecución no puede judgearse por sí sola: el veredicto se apoya en la sonda
> de runtime. El banco lo dice así en vez de inventar un PASS.

---

## 4. Gestión real de módulos Magisk

La gestión vive en `rust-core/src/magisk_module.rs` (instalar/validar) y en
`android.rs` (tool MCP). Reglas implementadas:

1. **Nada se activa a ciegas**: `install` tiene `dry_run` (por defecto) y, al
   instalar de verdad, **deja el módulo deshabilitado** (`disable`) para que la
   activación sea una decisión de quien reinicia.
2. **Un ZIP no es un módulo**: se valida `module.prop` con `id=`, y se anota si
   trae `zygisk/`, `service.sh`, `post-fs-data.sh`, `customize.sh`.
3. **Nada se borra**: se aparta, no se borra.

```bash
# Listar / habilitar / deshabilitar (lo de siempre, ahora con verificación)
megadaemon-core magisk list
megadaemon-core magisk enable  <id>
megadaemon-core magisk disable <id>

# Validar un ZIP local (no toca el móvil)
megadaemon-core magisk validate /data/local/tmp/megadaemon/modules/shamiko.zip

# Instalar (simulación por defecto)
megadaemon-core magisk install /data/local/tmp/megadaemon/modules/shamiko.zip --dry-run
megadaemon-core magisk install /data/local/tmp/megadaemon/modules/shamiko.zip --real --enable
```

### 4.1 Shamiko: descarga, verificación e instalación (medido)

**Descarga** (el PC es quien tiene red; el móvil no tiene `wget`/`curl` ni
salida a Internet):

```bash
curl -sSL https://api.github.com/repos/LSPosed/LSPosed.github.io/releases/latest
# → tag shamiko-414 (Shamiko 1.2.5, 2025-06-18)
curl -sSL -o Shamiko-v1.2.5-414-release.zip \
  https://github.com/LSPosed/LSPosed.github.io/releases/download/shamiko-414/Shamiko-v1.2.5-414-release.zip
```

**Verificación de la release** — y aquí aparece el bloqueo real:

| Release | Método de compresión | ¿Instalable en este móvil? |
|---|---|---|
| Shamiko 1.2.5 (`shamiko-414`) | **XZ (método 95) en todos los ficheros** | **NO**: ni `unzip` ni el instalador de Magisk 27002 lo leen |
| Shamiko 1.2.1 / 1.2 / 1.1.1 | **XZ** en `module.prop`, `customize.sh` y las `.so` | **NO** (mismo motivo) |
| **Shamiko 1.1 (`shamiko-344`)** | **deflate (8) puro** | ZIP OK, pero **el instalador aborta** (ver abajo) |

Comprobación en el PC:

```
$ unzip -l shamiko-v1.2.5-414-release.zip
  skipping: module.prop           unsupported compression method 95
  skipping: zygisk/arm64-v8a.so   unsupported compression method 95
```

**Instalación real medida** (Kitsune 27.2 = `magisk -V` **27002**), con el ZIP
v1.1 (deflate) subido al móvil y verificado byte a byte:

```
$ magisk --install-module /data/local/tmp/megadaemon/modules/shamiko.zip
 - Shamiko version v1.1 (344)
 - Verified META-INF/com/google/android/update-binary
 - Verified customize.sh
 - Verified verify.sh
 - Installing from Magisk / KernelSU app
- Magisk version: 27002
*********************************************************
! Please install Magisk Canary (>27003)
*********************************************************
exit=1
```

Después del intento: `/data/adb/modules_update/` **vacío** y
`/data/adb/modules/zygisk_shamiko` **no existe** ⇒ el instalador abortó
limpio, sin efectos.

**El bloqueo, en una frase**: el único Shamiko que este móvil puede
descomprimir (v1.1, deflate) **exige Magisk ≥ 27003** y se **auto-deshabilita**
en el arranque si `magisk -V < 27003` (su `post-fs-data.sh` hace
`touch $MODDIR/disable`); los Shamiko más nuevos (v1.1.1+) usan XZ y exigen
Magisk Canary ≥ 27005. Este móvil es **Kitsune 27.2 (27002)**.

Además, el propio Shamiko avisa de su requisito de configuración:

> *"Shamiko read the denylist from Magisk for simplicity but it requires
> denylist enforcement to be disabled first. ... **DO NOT** turn on denylist
> enforcement"*

En este móvil `denylist=0` ⇒ ese requisito **ya se cumple**.

**Estado real de Shamiko en el móvil**: `installed = false`, `zygisk = 1`,
`denylist = 0`, `magiskhide = 1`.

### 4.2 Qué queda pendiente del reinicio (y qué NO se ha tocado)

| # | Paso | Estado |
|---|---|---|
| 1 | Reiniciar el móvil | **PENDIENTE del usuario** (no se reinicia desde el agente) |
| 2 | Activar Shamiko | **BLOQUEADO**: requiere Magisk/Kitsune ≥ 27003, o una release cuyo ZIP sea deflate-compatible con 27002 |
| 3 | Alternativa que **sí** funciona en Kitsune 27.2: **DenyList con enforcement** (`denylist=1`) + añadir las apps objetivo a la lista | **PREPARADO, sin aplicar**: cambiar `denylist` exige reinicio para que Magisk haga el unmount |
| 4 | Re-medir con `rootcheck` | **PENDIENTE** (necesita el reinicio del punto 1) |

El paso 3 es el que **realmente** escondería `su`, `magisk` y los 16 montajes:
es el unmount por namespace de Magisk, no un hook. Con Shamiko (no
instalable aquí) el mecanismo sería el hook Zygisk.

Comandos exactos del paso 3 (NO ejecutados):

```bash
# 1) activar la aplicación del DenyList (aplica en el próximo arranque)
magisk --sqlite "INSERT OR REPLACE INTO policies (uid,policy,until,logging,notification) VALUES (10123,0,0,1,0);"
# 2) rebind del CSC (por si acaso) y reinicio manual
```

> `policy=0` ⇒ *denylist* (Magisk unmounta sus montajes para esa UID).
> `policy=2` ⇒ *su* (lo que ya tienen las filas que hay ahora: son la SuList
> de Kitsune, **no** la DenyList).

---

## 5. El CSC (grabación nativa de llamadas)

### 5.1 Estado verificado (medido)

| Qué | Valor medido |
|---|---|
| Módulo `megadaemon_csc_recording` | **ENABLED** (`/data/adb/modules/megadaemon_csc_recording/`) |
| ¿El `post-fs-data.sh` hizo el bind? | **Sí**: el fichero vivo y el del módulo tienen el **mismo sha256** |
| Fichero vivo | `/optics/configs/carriers/single/AMO/conf/system/cscfeature.xml` |
| sha256 del fichero vivo (OMC) | `d009908fa0b10aa387f9790f07eb1a593de9f47997b54853b1faa83a54bdaebb` (1143 B) |
| sha256 del **original de fábrica** | `ef110140f7bb95e109e623a11b644335843c1253bceaecdcf751fc43316e4d16` (1109 B) |
| Nº de features del fichero vivo | **30** |
| Nº de features del original de fábrica | **29** (y **sin** la feature de grabación) |
| **Valor real de `CscFeature_VoiceCall_ConfigRecording`** | **`RecordingAllowedByMenu,disclaimer_latin`** |

Cómo se midió el original de fábrica (el bind lo tapa): se desmonta **solo** el
bind-mount, se copia el fichero de debajo y se vuelve a montar:

```bash
T=/optics/configs/carriers/single/AMO/conf/system/cscfeature.xml
M=/data/adb/modules/megadaemon_csc_recording/cscfeature.xml
umount $T
sha256sum $T            # ef110140…  (1109 B, mtime 2008-12-31 = original)
cp -f $T /data/local/tmp/megadaemon/csc_patch/cscfeature_factory.xml
mount --bind $M $T       # se restaura el estado
sha256sum $T            # d009908f…  (1143 B) => bind restaurado
```

Decodificación OMC (rotación + XOR con tablas de 256, luego gzip), la misma
implementación que `rom_migration.rs` (`omc_decode_gzip` / `omc_encode_gzip`):

```bash
python3 tools/omc_codec.py decode <fichero.omc> -o <salida.xml>
python3 tools/omc_codec.py feature <fichero.omc> CscFeature_VoiceCall_ConfigRecording
→ RecordingAllowedByMenu,disclaimer_latin
```

> **Corrección a una premisa previa**: el módulo **no** es un *no-op* a nivel de
> fichero. El CSC de fábrica (AMO, `Version ED00008`, `Country SPAIN`) tiene 29
> features y **no trae** `CscFeature_VoiceCall_ConfigRecording`; el módulo añade
> esa bandera con el valor **legal** (`RecordingAllowedByMenu,disclaimer_latin`),
> es decir, grabación **desde el menú con el aviso al usuario**. Lo que el
> módulo **no** hace es activar la grabación sin aviso.

### 5.2 Cómo aplicar un valor nuevo (y comprobar que ha cambiado)

El comando aplica, **verifica** y **no cambia nada por defecto**:

```bash
# SIMULACIÓN (por defecto): no escribe en el móvil
megadaemon-core csc-patch --value RecordingAllowedByMenu,disclaimer_latin

# APLICAR de verdad (escribe en /data/adb/modules/megadaemon_csc_recording
# y actualiza el contents.db); requiere reiniciar para que lo lea el framework
megadaemon-core csc-patch --value <valor> --apply
```

Salida (verificable):

```json
{
  "modo": "simulacion",
  "csc": "AMO",
  "feature": "CscFeature_VoiceCall_ConfigRecording",
  "valor_anterior": "RecordingAllowedByMenu,disclaimer_latin",
  "valor_pedido": "RecordingAllowedByMenu,disclaimer_latin",
  "valor_leido_del_fichero_re_codificado": "RecordingAllowedByMenu,disclaimer_latin",
  "valor_aplicado": true,
  "sha256_antes": "d009908fa0b10aa387f9790f07eb1a593de9f47997b54853b1faa83a54bdaebb",
  "sha256_despues": "…",
  "sha256_cambia": true,
  "aviso": "SIMULACION: no se ha escrito nada en el modulo"
}
```

El flujo es: XML plano → `csc_feature_set` (sustituye o inserta dentro de
`<FeatureSet>`) → `omc_encode_gzip` (gzip + envoltorio OMC) → se **vuelve a
leer el fichero escrito, se calcula su sha256 y se descodifica** para
comprobar que el valor ha quedado aplicado. El `contents.db` se actualiza con
el tamaño nuevo porque el framework **descarta** el CSC parcheado si el tamaño
no cuadra.

### 5.3 Marco legal (no se cambia por defecto)

* **Art. 197 del Código Penal**: interceptar por parte de terceros las
  comunicaciones de otra persona sin autorización. La grabación de llamadas
  ajenas requiere consentimiento. El valor de fábrica de Samsung
  (`RecordingAllowedByMenu` + `disclaimer_latin`) **es el que avisa al resto de
  la conversación**: el propio sistema pide al usuario y muestra el aviso.
* **RGPD (UE 2016/679)**: las llamadas grabadas son **datos personales**; su
  tratamiento necesita base jurídica (consentimiento), información al
  interesado, minimización, plazo de conservación y seguridad. El disclaimer
  del CSC es la puerta de entrada del aviso, no su sustituto.
* Por eso **el valor por defecto de este trabajo es el de fábrica**
  (`RecordingAllowedByMenu,disclaimer_latin`) y cualquier cambio a
  `RecordingAllowed` (sin aviso) es una **decisión explícita del usuario**,
  que este banco deja medida pero no aplicada por defecto.

---

## 6. Qué se ha tocado en el móvil (y qué no)

**Tocado (reversible):**

* Se subió el ZIP de Shamiko v1.1 a `/data/local/tmp/megadaemon/modules/shamiko.zip`
  (sha256 `52e760863b6b301d34cffa1f353eadc1892b2543ded790246514e6e6434afb49`
  == el del PC) y se descomprimió en `/data/local/tmp/sh_test` para probarlo.
* Seintentó `magisk --install-module` ⇒ **abortó limpio**, sin instalar nada.
* Se desmontó **momentáneamente** el bind-mount del CSC para leer el original
  de fábrica y **se restauró** (sha256 verificado).
* Se arrancaron varias apps para la prueba de ejecución (el benchmark de
  root las deja abiertas en primer plano).

**No tocado:**

* **No se ha reiniciado el móvil.**
* No se ha instalado ningún módulo de Magisk (ni Shamiko ni ningún otro).
* No se ha cambiado `denylist`, `zygisk`, `magiskhide` ni `sulist`.
* No se ha cambiado el valor de la feature del CSC.
* No se ha instalado ni desinstalado ninguna app.
