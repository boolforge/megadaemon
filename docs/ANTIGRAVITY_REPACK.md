# Antigravity: reempaquetar la app para que el motor sea el nuestro

Fecha: 2026-11-24 · Rama: `agent/antigravity-instrumentado` · Nada de esto toca `main`.

## 1. Resumen

El APK que hay instalado en el móvil **no es una app de Google**: es un **repack** del
proyecto abierto [`werdio325-png/antigravity-android`](https://github.com/werdio325-png/antigravity-android)
(commit `b315e3c`), clonado en `/workspace/agy-upstream`. Eso cambia el objetivo:

* **No hay anti-tampering que saltar** (medido: 0 referencias a `playintegrity`,
  `integritytoken`, `safetynet`, `play/core`, `apk_signature` en la capa nativa del `.so`
  **y** en el único dex del APK).
* **El build es reproducible sin Gradle y sin Android Studio**: `build.sh` →
  `modules/builder/build_apk.sh`, con `android.jar` + `javac` + `r8` + `aapt` +
  `apksigner`.
* **Todo el material está en el clon** (407 MB), incluido el core y las herramientas con
  su sha256 fijada en `config/paths.env`.
* ⇒ Podemos publicar **nuestro** APK, con nuestro daemon dentro, firmado por nosotros.

## 2. Lo que hay en el clon (medido)

| Ruta | Tamaño | Qué es |
|---|---|---|
| `modules/core/pristine/language_server.tar.gz` | 52,6 MB | el core (el `.so` de 181 MB comprimido) |
| `tools/android.jar` | 27,2 MB | compilación Java (sha256 fijado) |
| `tools/r8.jar` | 15,3 MB | dex (sha256 fijado) |
| `tools/debug.keystore` | 2,6 KB | firma de depuración |
| `env/overlay/` | — | **el overlay que se instala en el móvil** |
| `app/src/main/java/com/agy/**` | 125 `.java` | las 91 clases `com.agy.*` del APK |

Huellas medidas dentro del APK instalado (coinciden con el clon):

* `assets/env/overlay/lib/daemon/{executor,scheduler,triggers}.py` — el daemon in-app.
* `assets/env/manifest.json` — `bundleId`, `overlaySha256`, `bootstrapSha256`, `pkgSha256`,
  `bootstrapSymlinkCount=427`.
* URL del SPA `https://app.antigravity.local/`, puerto **45157**.

## 3. El pipeline de build

`modules/builder/`: `01_gen_config.py` → `02_r_java.sh` → `03_compile_java.sh` →
`04_dex.sh` → `05_stage_assets.py` → `06_package.sh` → `08_sign.sh` → `09_verify.sh`.

`build_apk.sh` exige en el PATH: **`python3 aapt javac java apksigner`**.
En el sandbox hay `python3`/`java`; **faltan `aapt` y `apksigner`** (Android build-tools) ⇒
primer paso material: traer los build-tools (`aapt2` + `apksigner` + `zipalign`, ~60 MB) o
un firmador equivalente (`uber-apk-signer`).

El manifest (`env/scripts/make_manifest.py` + `pack_bootstrap.py`) se **regenera solo** cuando
cambia el overlay ⇒ our overlay entra sin tocar el manifiesto a mano.

## 4. Los tres puntos de inyección de nuestro código

### 4.1 `env/overlay/bin/agy-daemon` — el daemon in-app pasa a ser el nuestro

El overlay trae `bin/{agy-daemon,agy-fix-paths,agy-notify,agy-upgrade,cs,rish}`.
`agy-daemon` es el lanzador del daemon Python (`lib/daemon/executor.py`, `scheduler.py`,
`triggers.py`).

**Aquí entra nuestro binario Rust**: el `megadaemon-core` con el puente al language server,
el agente y el panel. El objetivo del usuario ("MegaDaemon en medio") se cumple de la forma
más limpia: **el daemon de la app es nuestro**, y el `.so` real sigue siendo el de Google
(lo interceptamos, no lo sustituimos).

Requisito: el binario debe hablar el RPC que la app espera. Referencia medida en
`/workspace/up-Antigravity-Mobile` (bridge Python del LS): `Login`, `GetUserStatus`,
`RetrieveUserQuotaSummary`, `StartCascade`, `SendUserCascadeMessage` ⇒ **portable a Rust**.

### 4.2 `modules/patches/` — los parches del engine

Grupos: `gates`, `syscalls`, `failfast`, `pty_shell`, `resolv`, `sigill`; spec en
`modules/patches/spec/patch_manifest.json`, orquestación en `pipeline.py`.
La política declarada es `fail_closed`, `idempotent`, **`no_fixed_offsets`** ⇒ se parchea
**por símbolo**, no por offset ⇒ encaja con nuestra regla (la verdad de las direcciones es el
**pclntab**, nunca el índice de Ghidra ni offsets fijos).

Ahí va, cuando llegue, el patch de la URL con el secreto local (`/mda/<15 hex>`, 41 bytes =
longitud del original) y el del shim.

### 4.3 `app/src/main/java/com/agy/**` — la UI nativa (125 ficheros)

Sólo si hay que tocar la UI de la app (hoy no: la UI de MegaDaemon es la web del cockpit).

## 5. La firma y lo que cuesta

`08_sign.sh` firma con `apksigner` + `debug.keystore`. **Nuestra firma ≠ firma instalada**
⇒ hay que **desinstalar la app** ⇒ se borra `/data/user/0/com.agy` ⇒ **se pierde la sesión de
Google** y hay que volver a hacer login.

⇒ **Orden obligatorio**: primero el fix del 404 (`4dd1992`, ya integrado) ⇒ la app arranca y
el login aguanta ⇒ luego el build limpio ⇒ luego nuestro daemon ⇒ luego la reinstalación.
Si se hace al revés, cada reinstalación devuelve al bucle de login y no se puede medir nada.

## 6. Plan por pasos

| # | Paso | Puerta |
|---|---|---|
| 0 | Fix del 404 desplegado (shim) | `/v1internal/…` responde 200, no 404 |
| 1 | Traer build-tools (`aapt`, `apksigner`) | `build_apk.sh` pasa el chequeo de herramientas |
| 2 | Build **sin cambios** del upstream | APK instalable; comparar assets con el instalado |
| 3 | Nuestro daemon en `env/overlay/bin/agy-daemon` | el daemon que corre en el móvil es el nuestro |
| 4 | Bridge del LS portado a Rust (`Login`…`SendUserCascadeMessage`) | la app ve la sesión y las cascadas |
| 5 | Los hooks dentro del APK (el plan viaja dentro) | NDJSON de hooks sin escribir nada a mano |
| 6 | Firma propia + reinstalación | login limpio y arranque estable |

## 7. Riesgos medidos

* **La app re-extrae `files/runtime/` en cada arranque** ⇒ lo que metamos en el APK se
  re-extrae siempre (bien para nosotros) y **el parche del `.so` en disco se pierde** (por eso
  los hooks van por plan + payload, no por parche en disco).
* **El secreto local no puede ir en el APK**: lo genera el daemon en el móvil (el fichero
  `agy_proxy.secret` es la fuente de verdad) ⇒ nunca en un asset ni en el repo.
* **Nada de nuestro binario en `runtime/bin/`**: fue la causa del exit 135 original (el
  desempaquetador de la app no puede borrar un fichero root). El binario va en
  `files/megadaemon/bin/`.
* El build del upstream parchea el engine; nuestros parches deben ser **idempotentes** (la
  política del spec ya lo exige).

## 8. Lo que ya está medido de este camino

* Autenticación real de la API local: **CSRF del SPA** (`x-codeium-csrf-token`), no OAuth.
* Catálogo real: **36 modelos**, `defaultAgentModelId=gemini-4-neon-high`,
  `MODEL_PLACEHOLDER_M526` ⇄ `gemini-4-hydrogen-medium`; **MiniMax no está** en el selector.
* Los grants de permisos: formatos válidos `command(...)` y `n(...)` ⇒ el daemon los sanea.
* El `.so` desofuscado al 99 % (`docs/ANTIGRAVITY_DESOBFUSCACION.md`) y los puntos de hook
  por dirección del pclntab (`docs/ANTIGRAVITY_HOOKS_PLAN.txt`).
