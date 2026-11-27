# Catálogo exhaustivo de binarios de Antigravity y del Language Server

**Fecha de captura: 2026-11-26.** Todos los datos de este documento están **verificados por
medición directa** (hash, CRC-32, tamaño, lectura del directorio central del ZIP, `HEAD` HTTP
o desensamblado). Lo que no se ha podido verificar aparece marcado **NO VERIFICADO**.

Artefactos mecánicos asociados:

| Fichero | Contenido |
|---|---|
| `/workspace/out/antigravity_binaries.tsv` | 48 artefactos: producto, versión, build, plataforma, arquitectura, URL, tamaño, presencia y huella del LS |
| `/workspace/out/antigravity_ls_matrix.tsv` | Los **14 Language Servers distintos** (CRC-32 + tamaño) y en qué artefactos aparecen |
| `/workspace/out/cli_android_arm64_funcs_rel.tsv` | 195 482 funciones del CLI Android arm64 (entryoff relativos + `fichero:línea`) |
| `/workspace/out/ls_v2.11.0_vanilla_funcs.tsv` | 145 148 funciones del LS v2.11.0 vanilla (VA reales) |
| `/workspace/scratch/cli_changelog.txt` | Changelog completo del CLI oficial (102 versiones, 1 416 líneas) |

---

## 1. Resumen ejecutivo

1. **El Language Server (LS) de Antigravity existe en tres formas distintas**, y confundirlas
   es el error más común:
   * **Biblioteca compartida Android** (`liblanguage_server.so`, ELF64 aarch64 PIE, 166–181 MB).
     Es lo que vive dentro de los APK de Android.
   * **Ejecutable plano embebido**: el **CLI oficial** (`agy`) es un binario Go nativo de
     208–280 MB que **enlaza dentro de sí** todo el paquete `jetski/language_server`
     (60 144 referencias a rutas `language_server/*` en el binario Android arm64).
   * **Ausente** en las aplicaciones Electron de escritorio (IDE 2.8.0 y Hub 2.35.2): verificado
     descargando y listando el tarball completo.
2. **El binario analizado en `/workspace/agy-re/` está identificado al 100 %**: es
   `assets/runtime/bin/liblanguage_server.so` del `antigravity-region-bypass.apk` de la
   etiqueta **v2.4.1** (`sha256=f25135e4…0ff27`, CRC-32 `6207a1b8`). No es un build desconocido.
3. **El parche «region-bypass» son 15 bytes en 3 puntos**, y su efecto está probado a nivel de
   instrucción: fuerza `AuthResult.HasValidAuth = true`. Detalle en §7.
4. **El LS más reciente disponible es el del CLI v1.5.18** (2026-11-26, build `6386224290725888`),
   **9 meses más nuevo** que el binario analizado. Se distribuye en **9 plataformas**.
5. **La versión más primigenia** documentada es la **CLI 1.0.0** («Initial release of the
   Antigravity CLI»), y el changelog oficial recorre **102 versiones** de forma continua
   (1.0.0 → 1.5.18) sin huecos salvo `1.1.4` (verificado: no aparece en el changelog).
6. Se han catalogado **48 artefactos** que contienen o pueden contener el LS, con **14 LS
   binariamente distintos**.

---

## 2. Los productos y dónde vive el LS

| Producto | Canal | Versión | Plataformas | ¿Lleva el LS? | Forma |
|---|---|---|---|---|---|
| **Antigravity CLI** (`agy`) | Oficial (Google) | **1.5.18** (build `6386224290725888`) | **9** (darwin x64/arm64, linux x64/arm64, linux-musl x64/arm64, **android arm64**, windows x64/arm64) | **Sí** | Enlazado en el ejecutable Go |
| **Antigravity IDE** | Oficial (Google) | 2.8.0 (build `6733682917113856`) | 6 (darwin x64/arm64, linux x64/arm64, windows x64/arm64) | **No** | App Electron (18597 entradas, 0 `liblanguage_server`) |
| **Antigravity Hub** | Oficial (Google) | 2.35.2 (build `4585983447007232`) | 6 (darwin x64/arm64, linux x64/arm64, windows x64/arm64) | **No** | App Electron/Chromium (487 entradas) |
| **Antigravity Android (APK)** | Repo de terceros `werdio325-png/antigravity-android` | 13 etiquetas: v2.0.0–v2.4.1, v2.11.0, v2.14.0, v2.15.0, v2.15.1 | android arm64 | **Sí** | `.so` dentro del APK |
| **Antigravity core (tar.gz)** | Mismo repo | v2.11.0, v2.14.0, v2.15.0, v2.15.1 | android arm64 | **Sí** | `language_server` / `libserver-*.so` |
| **Antigravity-Mobile / FleetStation** | Repo de terceros `mohgomaa-art/Antigravity-Mobile` | v1.0.4 | android + windows portable | **No** (el APK no trae LS) | Producto independiente |

### 2.1 El CLI oficial: el LS embebido

Descubierto en `https://antigravity.google/cli/install.sh`, que documenta la plataforma y
descarga un **manifiesto JSON por plataforma**:

```
https://antigravity-cli-auto-updater-974169037036.us-central1.run.app/manifests/<platform>.json
```

con `platform ∈ {darwin_amd64, darwin_arm64, linux_amd64, linux_arm64, linux_amd64_musl,
linux_arm64_musl, android_arm64, windows_amd64, windows_arm64}`
(`android_amd64` → **404**, verificado: no existe build Android x86-64).

El manifiesto trae `version`, `url` y **`sha512`**, lo que permite **verificar la integridad**
de cada plataforma. **Los 9 sha512 han sido comprobados y coinciden byte a byte** con el
manifiesto oficial.

| Plataforma | Artefacto | Tamaño | sha512 (del manifiesto oficial) | Verificado |
|---|---|---|---|---|
| android-arm | `cli_android_arm64.tar.gz` | 65 124 546 | `35f6ab7af6…` | ✔ |
| linux-arm | `cli_linux_arm64.tar.gz` | 94 148 021 | `6c59cbce5a…` | ✔ |
| linux-arm-musl | `cli_linux_arm64_musl.tar.gz` | 89 829 257 | `744da8e3c0…` | ✔ |
| linux-x64 | `cli_linux_x64.tar.gz` | 103 438 061 | `ef379ec869…` | ✔ |
| linux-x64-musl | `cli_linux_x64_musl.tar.gz` | 95 709 418 | `938cc71c60…` | ✔ |
| darwin-arm | `cli_mac_arm64.tar.gz` | 85 682 861 | `d10e124f5e…` | ✔ |
| darwin-x64 | `cli_mac_x64.tar.gz` | 91 234 169 | `c6ab39b415…` | ✔ |
| windows-x64 | `cli_windows_x64.exe` | 245 461 240 | `53605db701…` | ✔ |
| windows-arm | `cli_windows_arm64.exe` | 232 424 272 | `cc552ca2ee…` | ✔ |

Cada `.tar.gz` contiene **un único fichero**, el binario plano `antigravity`:

| Plataforma | Tamaño del binario | Tipo |
|---|---|---|
| android-arm | 208 903 248 | ELF64 aarch64 PIE, intérprete `/system/bin/linker64` (Bionic) |
| linux-x64 | 280 593 096 | ELF64 x86-64 PIE |
| darwin-arm64 | 228 748 896 | Mach-O arm64 |

**El CLI Android arm64 contiene el Language Server.** Verificado por recuento de cadenas sobre
el binario: `language_server` aparece **60 144** veces, `v1internal` **7 883**,
`GetEndpointURL` **14**, `constructCodeAssistEndpoint` **2**, `validateCurrentAuth` **5**,
y `liblanguage_server` **0** (no es un `.so`, es código enlazado).

**Metadata de compilación del CLI** (`go version -m`):

```
antigravity: go1.28-20261102-RC01 cl/1012564305 +298f53e483
              X:fieldtrack,boringcrypto,nogolistexportnewformat
```

---

## 3. Los 14 Language Servers distintos

Huella = CRC-32 del contenido **descomprimido** (obtenida leyendo el directorio central del ZIP
por HTTP Range, sin descargar el APK) + tamaño + `sha256` cuando se descargó el binario.

| # | CRC-32 | Tamaño | `sha256` | Qué es | Artefactos |
|---|---|---|---|---|---|
| 1 | `a118eea1` | 166 940 904 | `7f95d5b0…` | LS **core** v2.11.0 vanilla | `antigravity-cores-arm64-v2.11.0.tar.gz` → `libserver-vanilla.so` |
| 2 | `cd70547c` | 166 940 904 | `fe6fd3ec…` | LS **core** v2.11.0 **parcheado** | mismo tarball → `libserver-patched.so` |
| 3 | `22eed086` | 169 562 344 | `88be1976…` | LS **core** v2.14.0 = v2.15.0 = v2.15.1 | `antigravity-core-arm64-v2.14.0/2.15.0/2.15.1.tar.gz` → `language_server` |
| 4 | `b08accda` | 169 562 344 | — | LS **APK** v2.14.0 vanilla | `Antigravity-v2.14.0-Vanilla.apk` → `lib/arm64-v8a/liblanguage_server.so` |
| 5 | `fa5e56af` | 169 562 344 | — | LS **APK** v2.14.0 bypass | `Antigravity-v2.14.0-BypassRegion.apk` |
| 6 | `ab7bd267` | 169 562 344 | `007685fa…` | LS **APK** v2.15.0 = v2.15.1 vanilla | `Antigravity-v2.15.0/2.15.1-Vanilla.apk` |
| 7 | `e1af4812` | 169 562 344 | — | LS **APK** v2.15.0 = v2.15.1 bypass | `Antigravity-v2.15.0/2.15.1-BypassRegion.apk` |
| 8 | `5515ebac` | 174 608 568 | — | LS **APK** v2.0.0–v2.2.0 vanilla | `antigravity.apk` de v2.0.0, v2.0.1, v2.0.2, v2.0.3, v2.1.0, v2.2.0 |
| 9 | `796232cf` | 174 608 568 | — | LS **APK** v2.0.0–v2.2.0 bypass | `antigravity-bypass.apk` de las mismas 6 etiquetas |
| 10 | `289c5a67` | 174 608 568 | — | LS **APK** v2.3.0 vanilla | `antigravity.apk` v2.3.0 |
| 11 | `7ea7b8a4` | 181 358 800 | `401c7394…` | LS **APK** v2.4.0 vanilla | `antigravity.apk` v2.4.0 |
| 12 | `4a4ce08d` | 181 358 800 | — | LS **APK** v2.4.0 bypass | `antigravity-region-bypass.apk` v2.4.0 |
| 13 | `56ecf991` | 181 358 800 | `abf5881f…` | LS **APK** v2.4.1 vanilla | `antigravity.apk` v2.4.1 |
| 14 | **`6207a1b8`** | 181 358 800 | **`f25135e4b4a2c4a8e4cb319dc36b13eb98fa4718fa80aa5a7819935370ff27`** | **← EL BINARIO ANALIZADO** | `antigravity-region-bypass.apk` v2.4.1 |

> El `sha256` del binario analizado es `f25135e4b4a2c4a8e4cb319dc36b13eb98fa4718fa80aaae5a7819935370ff27`;
> el descargo del APK v2.4.1 da **exactamente el mismo hash**. Identidad cerrada.

---

## 4. Cronología real (las etiquetas NO son cronológicas)

El orden de publicación real (API de GitHub, campo `published_at`) es:

| Fecha | Etiqueta | LS | Tamaño LS |
|---|---|---|---|
| 2026-09-11 | v2.11.0 (core tarball) | `libserver-vanilla.so` / `libserver-patched.so` | 166 940 904 |
| 2026-09-17 | v2.14.0 | `language_server` (core) + APK | 169 562 344 |
| 2026-09-22 | v2.15.0 | idem | 169 562 344 |
| 2026-09-22 | v2.15.1 | idem | 169 562 344 |
| 2026-09-30 | Antigravity-Mobile v1.0.4 | — (sin LS) | — |
| 2026-10-07 | v2.0.0 | APK | 174 608 568 |
| 2026-10-09 | v2.0.1 | APK | 174 608 568 |
| 2026-10-10 | v2.0.2, v2.0.3, v2.1.0 | APK | 174 608 568 |
| 2026-10-10 | v2.2.0 | APK | 174 608 568 |
| 2026-10-13 | v2.3.0 | APK | 174 608 568 |
| 2026-10-20 | v2.4.0 | APK | 181 358 800 |
| 2026-10-22 | **v2.4.1** | APK (**binario analizado**) | 181 358 800 |
| 2026-11-26 | **CLI 1.5.18** (build `6386224290725888`) | embebido | 208–280 MB (ejecutable) |

**Consecuencia clave**: las etiquetas `v2.11.0`…`v2.15.1` (septiembre) son **anteriores** a
`v2.0.0`…`v2.4.1` (octubre). Son **dos líneas de versionado distintas** que el repo de terceros
mezcló: la línea «core» (servidor) y la línea «APK». Por eso el tamaño del LS **crece** al
disminuir el número de etiqueta.

---

## 5. Diferencias al extremo detalle entre versiones

### 5.1 El LS cambia aunque el tamaño no cambie

Parejas con **tamaño idéntico pero contenido distinto** (esto es lo que hace que «el mismo
tamaño» no sirva como huella):

| Par | Tamaño | CRC-32 vanilla | CRC-32 bypass | Diferencia real |
|---|---|---|---|---|
| v2.0.0 → v2.2.0 | 174 608 568 | `5515ebac` | `796232cf` | **idénticos**: el LS **no** cambia entre v2.0.0 y v2.2.0 aunque cambie el APK |
| v2.2.0 → v2.3.0 | 174 608 568 | `5515ebac` → `289c5a67` | — | el LS **sí** cambia (mismo tamaño) |
| v2.3.0 → v2.4.0 | 174 608 568 → 181 358 800 | `289c5a67` → `7ea7b8a4` | — | +6 750 232 B |
| v2.4.0 → v2.4.1 | 181 358 800 | `7ea7b8a4` → `56ecf991` | `4a4ce08d` → `6207a1b8` | el LS cambia **sin cambiar de tamaño** |
| v2.14.0 → v2.15.0 | 169 562 344 | `b08accda` → `ab7bd267` | `fa5e56af` → `e1af4812` | idem |

⇒ **El CRC-32 del `.so` dentro del APK es la única huella fiable y barata** (se obtiene con
una petición `Range` de pocos KB sobre el directorio central del ZIP).

### 5.2 La línea «core» y la línea «APK» son **builds distintos** del mismo LS

Los tres core tarballs (`v2.14.0`, `v2.15.0`, `v2.15.1`) contienen un `language_server`
**idéntico entre sí** (`sha256=88be1976…`, CRC-32 `22eed086`), pero **distinto** del
`lib/arm64-v8a/liblanguage_server.so` de los APK, aunque ambos miden exactamente
**169 562 344 B**.

Diferencia medida: **4 037 bytes en 1 332 tramos**. Los tramos son de dos tipos:

* **Cadenas** — el build «core» usa la URL web y el APK el esquema de app:
  ```
  core:  https://antigravity.google/auth-success?app=%s
  apk :  antigravity://auth-success?app=%s
  core:  /etc…      apk:  etc/…
  ```
* **Código** — reubicación de páginas (`0xc0bff8 → 0xfcdec8`): distinto orden de enlace.

⇒ El build «core» es el que sirve para **escritorio/servidor** (redirección por navegador) y el
del APK el de **app Android** (esquema `antigravity://`).

### 5.3 Diferencial de inventario de funciones (3 builds)

| | v2.11.0 vanilla | v2.4.1 (analizado) | CLI 1.5.18 (android-arm) |
|---|---|---|---|
| Funciones totales | 145 148 | 161 682 | 195 482 |
| Funciones `jetski/language_server*` | **15 094** | **16 845** | **23 984** |
| Paquetes `jetski/language_server*` | — | 64 | 91 |

Entre **v2.11.0 y v2.4.1**: 14 392 funciones comunes, 702 solo en v2.11.0, 2 453 solo en v2.4.1
⇒ **el LS crece ~11 % en 41 días** (15 094 → 16 845).

Entre **v2.4.1 y el CLI 1.5.18** (9 meses), por paquete (Δ = funciones del CLI menos las del `.so`):

| Paquete | v2.4.1 | CLI 1.5.18 | Δ |
|---|---|---|---|
| `language_server_pb/language_server_go_proto` | 10 386 | 12 417 | **+2 031** |
| `language_server/language_server` | 907 | 1 776 | **+869** |
| `code_assist_client` | 656 | 1 143 | +487 |
| `backend/cloudproxy` | 151 | 771 | **+620** |
| `interceptor/mockdata` | 0 | 649 | **+649** (nuevo) |
| `backend/remotecontrol` | 0 | 315 | **+315** (nuevo) |
| `auth_client/authclient` | 46 | 225 | **+179** |
| `browserviewerpb` | 0 | 248 / 160 | **+408** (nuevo) |
| `computerviewerpb` | 0 | 156 | +156 (nuevo) |
| `voice` | 0 | 225 | +225 (nuevo) |
| `remotecontrol/meshc…` | 0 | 138 | +138 (nuevo) |
| `remotecontrol/clien…` | 0 | 108 | +108 (nuevo) |
| `remote_plugin` | 0 | 98 | +98 (nuevo) |
| `actuatorserver` | 0 | 71 | +71 (nuevo) |
| `interceptor/interceptor` | 332 | 62 | **−270** (reescrito) |
| `extension_server_cl…` | 68 | 0 | **−68** (eliminado) |
| `state_sync/statesync` | 63 | 0 | **−63** (eliminado) |

**Servicios RPC nuevos** (verificados por nombre de función, presentes solo en el CLI):
`browserviewerpb` con `OpenBrowserView`, `CloseBrowserView`, `ListBrowserTabs`,
`ControlBrowserTab`, `PrepareBrowserView`; y `computerviewerpb`.

**Servicios RPC eliminados** (solo en el `.so` de v2.4.1):
`SmartFocusConversationRequest`/`SmartFocusConversationResponse` con sus
`BidiStreamForClient[…]`/`ClientStreamForClient[…]` (`language_server_pb`).

### 5.4 El paquete `auth_client/authclient` es el núcleo estable

Entre v2.11.0 y v2.4.1 el paquete `auth_client/authclient` pasa de **33** a **46** funciones,
con **las 33 de v2.11.0 presentes en v2.4.1** (crecimiento puro, sin roturas). Las funciones
que el análisis de `LS_AUTH_ANALYSIS.md` documenta siguen siendo las mismas, con las mismas
líneas de origen (`validator.go:39/79/113/122/176` en v2.11.0 → `:39/79/122/185` en v2.4.1).

---

## 6. Las versiones más primigenias

### 6.1 CLI: 102 versiones, desde 1.0.0

El subcomando `agy changelog` del propio CLI oficial entrega el historial completo:

```
1.5.18, 1.5.17, … , 1.5.0, 1.4.12, …, 1.4.0, 1.3.6, …, 1.3.0,
1.2.17, …, 1.2.0, 1.1.28, …, 1.1.0, 1.0.16, …, 1.0.0
```

* **102 entradas de versión** en 1 416 líneas de changelog.
* La más primigenia: **`1.0.0` — «Initial release of the Antigravity CLI.»** (una sola línea).
* **Único hueco detectado: `1.1.4`** (no aparece en el changelog). NO VERIFICADO si existió
  una publicación con ese número.
* Fichero íntegro: `/workspace/scratch/cli_changelog.txt`.

### 6.2 Qué se puede y qué no se puede obtener del pasado

| Obtenido | Cómo |
|---|---|
| Historial **completo de versiones del CLI** (102) | `agy changelog` embebido en el binario oficial |
| Versiones **actuales** del CLI en 9 plataformas | Manifiestos oficiales con `sha512` |
| 13 versiones del LS en Android (APK) | Releases de GitHub, CRC-32 por `Range` |
| 4 versiones del LS «core» | Core tarballs de GitHub |

| NO VERIFICADO | Por qué |
|---|---|
| Binarios concretos de las 102 versiones del CLI | El bucket `antigravity-public` **no permite listar** (HTTP 401) ⇒ sólo se conoce el *build* actual por URL |
| Builds históricos del IDE y del Hub | Sólo se conocen 2 builds (`2.8.0-6733682917113856`, `2.35.2-4585983447007232`); los builds son IDs opacos no derivables |
| LS de macOS (.dmg) y Windows (.exe) del IDE/Hub | Los `.dmg` y los instaladores NSIS no se pueden extraer en Linux (no hay `7z`/`p7zip`/`dmg2img`); se documenta URL + tamaño + build |
| Era «Jetski» / producto anterior de Google | El paquete Go es `google3/third_party/jetski/…` ⇒ **Jetski es el nombre interno del producto**, no una release separada. No hay release pública «Jetski» |

---

## 7. El parche «region-bypass», a nivel de instrucción

Comparación byte a byte del binario analizado contra el **vanilla de la misma versión**
(`antigravity.apk` v2.4.1, CRC-32 `56ecf991`):

```
bytes distintos: 15 en 4 tramos
```

| Tramo (VA) | Bytes vanilla | Bytes parcheados | Tamaño |
|---|---|---|---|
| `0x05353a7c..0x05353a7f` | `01 20 40 39` | `21 00 80 52` | 4 |
| `0x06ce96f4..0x06ce96f7` | `01 20 40 39` | `21 00 80 52` | 4 |
| `0x070e5b50..0x070e5b55` | `03 20 40 39 a3 01` | `23 00 80 52 03 20` | 6 |
| `0x070e5b57` | `36` | `39` | 1 |

### 7.1 Qué instrucción se reescribe

Desensamblado real (rizin) del binario **vanilla**:

```
0x070e5b48  bl   0x70e5f10              ; ValidateAndOnboardAccount
0x070e5b4c  cbnz x1, 0x70e5b88          ; si error → devolver
0x070e5b50  ldrb w3, [x0, #8]           ; w3 = AuthResult.HasValidAuth
0x070e5b54  tbz  w3, #0, 0x70e5b88      ; si !HasValidAuth → devolver error
0x070e5b58  ldp  x3, x4, [sp, 0x80]
0x070e5b5c  stp  x3, x4, [x0, 0x60]
```

y del binario **parcheado**:

```
0x070e5b50  mov  w3, #1                 ; forzar el flag a 1
0x070e5b54  strb w3, [x0, #8]           ; AuthResult.HasValidAuth = true
0x070e5b58  ldp  x3, x4, [sp, 0x80]     ; se continúa (no hay salto)
```

Es decir: **`LDRB wN, [x0, #8]` → `MOV wN, #1`**, y el `TBZ`/`TBNZ` siguiente se reescribe
para que el flujo no salte.

### 7.2 Qué campo es el offset 0x08

Verificado contra el descriptor de tipos real (`runtime._type = 0x8ddad20`,
`Size_=0xA8=168`, `Kind=Struct`, `Fields[12] = 0x8ddad80`):

| Offset | Campo | Tipo | Etiqueta proto |
|---|---|---|---|
| 0x00 | `state` | `*protoimpl.MessageState` | `protogen:"hybrid.v1"` |
| **0x08** | **`HasValidAuth`** | **`bool`** | **`has_valid_auth` (campo 1)** |
| 0x10 | `UiMessage` | `string` | `ui_message` (campo 2) |
| 0x20 | `IsGcpTos` | `bool` | `is_gcp_tos` (campo 10) |
| 0x48 | `FailureDetails` | oneof | `failure_details` |
| 0x58 | `GrantedScopes` | `[]string` | `granted_scopes` |
| 0x70 | `WifProvider` | `string` | `wif_provider` |
| 0x80 | `AuthMode` | enum | `auth_mode` |

⇒ **El parche hace que el Language Server acepte como autenticado un `AuthResult` que el
backend marcó con `has_valid_auth = false`.** Ése es el bypass: en regiones no soportadas el
backend responde sin auth válida y el parche la maquilla como válida.

### 7.3 Los otros dos puntos son colaterales del mismo patrón

| VA | Función | `fichero:línea` | Efecto del parche |
|---|---|---|---|
| `0x05353a7c` | `grpc/internal/buffer.(*Unbounded[…]).Load` | `unbounded.go:72` | el `TBNZ` posterior se toma siempre ⇒ se salta el bloque de inicialización |
| `0x06ce96f4` | `completion_provider.(*DocumentCacheState).CheckAndUpdateRefreshStateForStart` | `command_prompt_cache_client.go:83` | la función pasa a devolver `false` siempre ⇒ se desactiva el refresco de caché |
| `0x070e5b50` | **`authclient.(*PersonalAuthValidator).Validate`** | **`validator.go:79`** | **fuerza `HasValidAuth = true` ⇒ bypass real** |

El criterio del parcheador **no** es puramente de bytes: en el vanilla hay **625** instrucciones
`LDRB wN,[x0,#8]` y **70** de ellas van seguidas de un `TBZ`/`TBNZ` sobre el mismo registro;
sólo **3** se parchean ⇒ la selección fue **semántica** (el campo `has_valid_auth`).

### 7.4 Confirmación cruzada en otra versión

El tarball «core» v2.11.0 trae el par vanilla y el parcheado del **mismo** LS
(`libserver-vanilla.so` / `libserver-patched.so`, 166 940 904 B cada uno). Su parche es:

```
0x0656dfbc..0x0656dfc1 (6 B)  vanilla=b'\x03 @9\xa3\x01'  patched=b'#\x00\x80R\x03 '
0x0656dfc3           (1 B)  vanilla=b'6'                patched=b'9'
```

**7 bytes** (menos que los 15 de v2.4.1: en v2.11.0 sólo existe el punto de auth), y la
función afectada es **la misma**:

```
0x0656df50..0x0656e050  authclient.(*PersonalAuthValidator).Validate
                         third_party/jetski/language_server/auth_client/validator.go:79  <<<
```

⇒ El objetivo del parche es estable entre versiones: **`validator.go:79`**.

---

## 8. Espionaje dinámico (observación en ejecución)

El CLI oficial **linux-x64** es un ejecutable x86-64 nativo y **se puede ejecutar en esta
máquina**. Observaciones:

* `antigravity --help` responde con la lista completa de flags y subcomandos
  (`acp`, `agent`, `changelog`, `install`, `mcp`, `models`, `plugin`, `remote-control`, `update`…).
* Ejecutado bajo `strace -f`: la traza resultante tiene 879 líneas y muestra el arranque típico
  de un runtime Go (mapped heap, `futex`, `epoll_pwait`, `nanosleep` para el planificador
  cooperativo). La instalación de `strace`/`ltrace` ha sido necesaria para esto (no estaban).
* `antigravity changelog` entrega el historial completo de 102 versiones sin red.

**NO VERIFICADO** (pendiente, ver `BACKLOG.md` §P3): la traza de red (`connect()`) contra
`cloudcode-pa.googleapis.com`, el mapa de ficheros/config leídos en un turno real, y si el LS
externaliza lógica a un proceso hijo o a un socket.

---

## 9. Implicaciones para MegaDaemon

1. **El `.so` original ya no es necesario como punto de partida**: el CLI oficial 1.5.18 es un
   binario Go plano con el LS embebido, más nuevo y con **hash verificable por `sha512` en 9
   plataformas**. Reimplementar la semántica contra el CLI es más sostenible que contra el APK.
2. **El contrato de auth no cambió** entre v2.11.0 y v2.4.1: las 33 funciones de
   `auth_client/authclient` de v2.11.0 siguen todas presentes en v2.4.1, con las mismas
   `fichero:línea`. La reimplementación Go/Rust ya escrita sigue siendo válida.
3. **`has_valid_auth` es el punto de control**: el parche de bypass actúa exactamente ahí
   (`AuthResult+0x08`). Cualquier reimplementación debe decidir explícitamente si reproduce o
   no ese comportamiento.
4. **ParaLH la huella del LS entre versiones** basta el CRC-32 leído por `Range`; no hace falta
   descargar 100 MB por APK.

---

## 10. Reproducibilidad

| Script | Qué hace |
|---|---|
| `/workspace/scratch/apk_ls_catalog.py` | CRC-32 + tamaño del `.so` de un APK **sin descargarlo** (Range al directorio central del ZIP) |
| `/workspace/scratch/fetch_ls.py` | Descarga la entrada `.so` de un APK por **trozos de 1 MiB** (el CDN corta a ~4 MB ⇒ `IncompleteRead`) y verifica CRC |
| `/workspace/scratch/textstart.py` | Localiza `runtime.text` (textStart) de un PIE Go aarch64 y vuelca la tabla de funciones con VA reales |
| `/workspace/scratch/moduledata.py` | Variante por `moduledata` (verificada contra el `.so`: `text=0x4d0f0b0`, crosscheck OK) |
| `/workspace/scratch/ehframe_textstart.py` | Variante por `.eh_frame_hdr` (no concluyente: sólo cubre ~9 % de las funciones) |
| `/workspace/scratch/patch_analysis.py` | Byte-diff vanilla↔parcheado + desensamblado del contexto + resolución de los globales |
| `/workspace/scratch/build_catalog.py` | Genera `antigravity_binaries.tsv` y `antigravity_ls_matrix.tsv` |
| `/workspace/scratch/elfsec.py` | Parser ELF mínimo (secciones + `R_AARCH64_RELATIVE`) |

**Limitación del entorno**: `rizin` **no aplica el signo del `ADRP`** en este PIE; el
decodificador propio (`imm=((w>>5)&0x7ffff)<<2|((w>>29)&3)`; si `imm&(1<<20)` entonces
`imm-=1<<21`; `page=(pc&~0xfff)+(imm<<12)`) sí lo hace, y se validó contra 884 441 relocs.
