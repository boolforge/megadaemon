# Gestión integrada de paquetes — el estándar único

> **Estado**: `rust-core/src/pkgmgr.rs` (nuevo) + `rust-core/src/bin/md-pkg.rs` (nuevo).
> **Cabling pendiente**: falta `pub mod pkgmgr;` en `rust-core/src/lib.rs`
> (lo añade el dueño del repo; este módulo **no** toca `lib.rs`).

Este documento es la **norma**: el formato único de manifiesto, el flujo único
de actualización y el plan de migración desde lo que hay hoy.

---

## 0. TL;DR

* **Un solo formato** (`Manifest`) para **todo** lo que se actualiza: APKs,
  binarios, repos git, fuentes y diccionarios. Da igual de dónde venga
  (release de GitHub, canal del daemon, repo git, URL directa).
* **Un solo flujo**: detectar → decidir → descargar (reanudable) → verificar
  → instalar (atómico) → rollback.
* **La decisión es por SHA-256**, no por versión ⇒ si el contenido es
  idéntico, **no se descarga ni un byte** (aunque el número de versión sea
  distinto).
* **El cambio es atómico** (`rename`, nunca `cp`) ⇒ un binario en marcha se
  sustituye sin `ETXTBSY` ("Text file busy") y nunca queda a medias.
* **Un cursor por proveedor** (ETag / Last-Modified) ⇒ no se relee el
  manifiesto entero cuando el proveedor dice "sin cambios".
* **Una cuarentena de fallos** ⇒ no se reintenta lo que ya se sabe que falla.

Interfaz de línea de comandos:

```bash
md-pkg index                     # tabla unificada (sin red)
md-pkg check                     # refresca fuentes y dice qué hay que actualizar
md-pkg update --kind binary --name megadaemon-core [--dry-run]
md-pkg manifest validate dist/state/pkg-manifest.json
```

---

## 1. El problema: hoy hay cuatro formatos y tres caminos

| Dónde vive | Formato | Qué es | SHA-256 | ¿Hablan entre sí? |
|---|---|---|---|---|
| `.github/workflows/build.yml` → `dist/state/latest.json` | `{"version","short","ref","released_at","assets":{plataforma→fichero},"assets_versioned":{…}}` | estado de la release | **NO** (en ficheros `.sha256` sueltos) | no |
| `rust-core/src/channel.rs` → `state/channel.json` | `{"quick","duckdns","canonical","ts"}` | estado del canal | no | no |
| `rust-core/src/repo_updates.rs` | índice F-Droid / Obtainium (`package`, `versionCode`, `sha256`, `apk_url`) | catálogo de apps | sí (sólo F-Droid) | no |
| `deploy/99-megadaemon.sh` + `deploy/install-phone.sh` | shell | actualización del binario | sí (`sha256sum`) | no |

Y los caminos de actualización son tres, con **standards distintos**:

1. **CI → release → `latest.json`**: el `latest.json` no dice qué hash tiene
   cada binario; el lector tiene que ir a por los `.sha256` aparte, y el
   `latest.json` heredado **no se puede validar** (no lleva tamaño ni hash).
2. **`repo_updates::download()` + `apply()`**: descarga a `.part`, verifica el
   SHA-256 y renombra (bien), pero **no es reanudable** (no pide
   `Range: bytes=N-`) y `deploy_binary()` hace `cp -f` **sobre el binario en
   marcha** ⇒ `ETXTBSY` y la posibilidad de dejar el daemon truncado.
3. **`deploy/99-megadaemon.sh` (supervisor Magisk)**: `git pull` + `auto-update.sh`
   + un auto-rollback manual con `.restore` (el `auto-update.sh` **no existe
   en el repo**: el supervisor lo busca en `$PREFIX/repo/deploy/`, así que hoy
   el auto-update **no ocurre**).

Resultado: "package management / repo / store" son cuatro sistemas que no se
hablan, y las auto-actualizaciones no son eficientes (se re-descarga lo que
ya está, no se reanuda lo que se quedó a medias, y se sustituye el binario
en marcha de forma insegura).

---

## 2. El estándar único: el manifiesto

### 2.1 El JSON, campo a campo

```json
{
  "schema_version": 1,
  "version": "13.4.0",
  "released_at": "2026-11-21T10:00:00Z",
  "assets": [
    {
      "name": "megadaemon-core",
      "kind": "binary",
      "os": "android",
      "arch": "arm64",
      "url": "https://github.com/boolforge/megadaemon/releases/download/v13.4.0/megadaemon-core-android-arm64",
      "sha256": "9f2c…(64 hex)…",
      "size": 24117248,
      "signature": "v13.4.0+build.7"
    }
  ]
}
```

| Campo | Tipo | Obligatorio | Regla |
|---|---|---|---|
| `schema_version` | entero | sí | **exactamente** `1`. Es la versión del *formato*, no del contenido. Cambiar el formato ⇒ subirlo (y el lector rechaza lo que no entiende). |
| `version` | texto | sí | **semver-ish**: `MAJOR.MINOR[.PATCH][-pre][+build]`. Se admite un `v` inicial. Ejemplos válidos: `13.4.0`, `1.2`, `v13.4.0-rc1`, `13.4.0+build.7`. |
| `released_at` | texto | sí | ISO-8601 UTC: `AAAA-MM-DDThh:mm:ssZ`. |
| `assets` | lista | sí | **al menos un** asset (un manifiesto vacío suele ser un fichero truncado ⇒ se rechaza). |
| `assets[].name` | texto | sí | Nombre estable del artefacto (para un APK, el *package id*). **Sin separadores de ruta** (`/`, `\`), **sin `..`**, sin caracteres de control, ≤ 128 caracteres. |
| `assets[].kind` | enum | sí | `apk` \| `binary` \| `repo` \| `font` \| `dict`. |
| `assets[].os` | texto | sí | `android` \| `linux` \| `macos` \| `windows` \| `any`. Sólo `[a-z0-9_-]`. |
| `assets[].arch` | texto | sí | `arm64` \| `x86_64` \| `arm` \| `any`. Sólo `[a-z0-9_-]`. |
| `assets[].url` | texto | sí | **https** obligatorio, con host y sin espacios ni caracteres de control. |
| `assets[].sha256` | texto | sí | **64 hexadecimales** (SHA-256 del contenido *exacto* que se descarga). |
| `assets[].size` | entero | sí | **> 0** (bytes del fichero). |
| `assets[].signature` | texto | **no** | Firma o commit de referencia. APK: huella del certificado. Repo: **commit** (la identidad de un repo no es el hash de un fichero). Binario: firma de la release. |

La validación es **estricta** y vive en `Manifest::validar()`
(`rust-core/src/pkgmgr.rs`): cualquier desviación ⇒ el manifiesto se rechaza
**antes** de descargar un solo byte.

### 2.2 Por qué estos campos y no otros

* **El hash va en el manifiesto** (y no en un fichero `.sha256` hermano): si
  el hash va aparte, la descarga puede usar el hash de *otra* versión (caché
  CDN, `latest.json` viejo) ⇒ se instala algo que no es lo que se cree.
* **El tamaño va en el manifiesto**: permite (a) reservar espacio, (b) saber
  si la descarga terminó, y (c) **rechazar** un `size: 0` que delataría un
  manifiesto a medio generar.
* **`kind` + `os` + `arch`** permiten que **un solo manifiesto** sirva para el
  móvil (android/arm64), el PC (linux/x86_64) y el portátil (linux/arm64), sin
  cuatro manifiestos distintos.
* **`schema_version`** permite que un cliente viejo entienda (o rechace
  claramente) un manifiesto nuevo.

### 2.3 Un manifiesto por tipo de artefacto

```jsonc
// APK (el "name" es el package id)
{ "name": "org.fdroid.fdroid", "kind": "apk", "os": "android", "arch": "arm64",
  "url": "https://fdlf01.f-droid.org/repo/org.fdroid.fdroid_10800.apk",
  "sha256": "…", "size": 8123456, "signature": "sha256:ab12… (cert)" }

// Repo git (la identidad es el commit, que va en "signature")
{ "name": "megadaemon", "kind": "repo", "os": "any", "arch": "any",
  "url": "https://github.com/boolforge/megadaemon/archive/refs/heads/main.tar.gz",
  "sha256": "…", "size": 4194304, "signature": "c7575420000000000000000000000000000000" }

// Fuente (tipografía)
{ "name": "JetBrainsMono-Regular.ttf", "kind": "font", "os": "any", "arch": "any",
  "url": "https://…/JetBrainsMono-Regular.ttf", "sha256": "…", "size": 180224 }

// Diccionario
{ "name": "es_ES.dic", "kind": "dict", "os": "any", "arch": "any",
  "url": "https://…/es_ES.dic", "sha256": "…", "size": 1048576 }
```

---

## 3. Fuentes: qué proveedor usa qué

El enum `Fuente` tiene **cuatro** variantes y **una sola** API
(`listar_disponibles() -> Result<Vec<Manifest>, String>`): quien llama no
necesita saber de dónde sale el manifiesto.

| `Fuente` | Qué es | Dónde vive el manifiesto | Cómo se normaliza al esquema | Notas |
|---|---|---|---|---|
| `Release(GitHubRelease{owner,repo,tag})` | release de GitHub | API `GET /repos/{o}/{r}/releases/latest` (o `/tags/{tag}`) | cada *asset* de la release → `Asset`; `name` = nombre del fichero sin el sufijo de plataforma; `os`/`arch` se deducen del nombre (`…-android-arm64` → android/arm64); `sha256` del campo `digest` de la API, o del `.sha256` hermano | el CI publica una release por commit (tag `v<sha>`) |
| `Canal(ChannelJson{url})` | `channel.json` del daemon | URL del canal (`state/channel.json` publicado, `raw` del repo o endpoint del relay) | el documento puede traer `{"manifest": {…}}` (preferido) o `{"assets": […], "version": …, "released_at": …}` | `channel.json` **sigue siendo** el estado del canal; el manifiesto va **anidado**, no lo sustituye |
| `RepoGit{url, rama}` | repo git | `<raw>/<rama>/pkg-manifest.json` (o `manifest.json`) | el fichero es un `Manifest` (o `{"manifests":[…]}`) | la URL cruda se deriva (`github.com/o/r` → `raw.githubusercontent.com/o/r/<rama>/`) |
| `Directo{url}` | un manifiesto en una URL | la propia URL | un `Manifest` (o una lista) | el caso más simple: el CI publica el manifiesto y ya |

Configuración:

* Por defecto: la release de `boolforge/megadaemon` (tag `latest`), el canal
  del daemon y el repo git del proyecto.
* Por entorno: `MEGADAEMON_PKG_REPO`, `MEGADAEMON_PKG_TAG`,
  `MEGADAEMON_PKG_CHANNEL_URL`, `MEGADAEMON_PKG_MANIFEST_URL`,
  `MEGADAEMON_PKG_GIT_URL`, `MEGADAEMON_PKG_GIT_BRANCH`.
* Por fichero: `<state_dir>/pkgmgr/fuentes.json`
  (mismo formato `Fuente`, con `{"fuentes":[…]}`).

---

## 4. El flujo de actualización

```
   ┌──────────────┐   ┌──────────────┐   ┌───────────────┐
   │  DETECTAR    │   │   DECIDIR    │   │  DESCARGAR    │
   │              │   │              │   │               │
   │ listar_      │   │ comparar     │   │ GET con       │
   │ disponibles  │──▶│ SHA-256      │──▶│ Range: bytes= │
   │ (con cursor) │   │              │   │ N-  (reanuda) │
   └──────────────┘   └──────────────┘   └───────┬───────┘
        │                    │                    │
        │                    │ si el SHA-256      │
        │                    │ es el mismo ⇒      │
        │                    │ NO se descarga      │
        │                    ▼                    ▼
        │              ┌──────────────┐   ┌───────────────┐
        │              │  VERIFICAR   │   │  INSTALAR     │
        │              │              │   │  (atómico)    │
        │              │ SHA-256 del  │──▶│  rename()     │
        │              │ fichero !=   │   │  .prev se     │
        │              │ esperado ⇒   │   │  conserva     │
        │              │ se borra     │   └───────┬───────┘
        │              └──────────────┘           │
        │                                          ▼
        │                                 ┌───────────────┐
        │                                 │   ROLLBACK    │
        │                                 │               │
        │                                 │ si falla tras │
        │                                 │ tocar el      │
        │                                 │ destino ⇒     │
        │                                 │ restaura el   │
        │                                 │ .prev         │
        │                                 └───────────────┘
        ▼
   IndicePaquetes (tabla unificada)
```

### 4.1 Garantías, punto por punto

| Paso | Garantía | Dónde |
|---|---|---|
| Detectar | Con **cursor** (ETag / Last-Modified) el proveedor que no ha cambiado **no se relee**. Con **cuarentena**, el proveedor que falló hace poco **no se reintenta**. | `refrescar_proveedor()` |
| Decidir | La comparación es por **SHA-256**: contenido idéntico ⇒ `None` ⇒ **cero descarga** (aunque cambie la versión). Sin hash local ⇒ se usa la versión y la fila queda como `Desconocido` (no se finge que esté al día). | `comprobar_actualizacion()` |
| Descargar | **Reanudable**: si existe `<destino>.new`, se pide `Range: bytes=N-` y se continúa. Si el servidor ignora el rango (`200`), se reinicia. El SHA-256 se calcula **en la misma pasada** (el parcial ya descargado entra en el hash antes de continuar) ⇒ ni una copia extra en disco. | `descargar_resumible()` |
| Verificar | El hash se comprueba **antes** de instalar. Si no cuadra, el parcial **se borra** y no se instala nada. | `descargar_resumible()` |
| Instalar | **Atómico**: `rename` (nunca `cp`). El anterior se conserva como `<destino>.prev` mediante enlace duro (o copia) ⇒ **no hay ventana** en la que el destino no exista. | `instalar_atomicamente()` |
| Rollback | Si algo falla **después** de tocar el destino, se restaura el `.prev` y se devuelve un `Rollback { destino, previo, restaurado, motivo }`. | `instalar_con_rollback_detallado()` |

### 4.2 Por qué `rename` y no `cp`

`cp -f` sobre un binario **en ejecución** devuelve `ETXTBSY` ("Text file
busy"), y si se corta a medias deja el fichero **truncado**: el móvil se queda
sin daemon (por eso el supervisor de `deploy/99-megadaemon.sh` tiene que
resucitar el binario a mano). `rename(2)` dentro del mismo sistema de ficheros
es atómico: o está el binario viejo, o el nuevo, **nunca un mezcla**, y no
falla por "Text file busy".

---

## 5. La vista unificada (`IndicePaquetes`)

La tabla plana que pide el usuario: qué hay instalado × qué hay disponible ×
**qué está desfasado y por qué**.

| Columna | Significado |
|---|---|
| `kind` | `apk` \| `binary` \| `repo` \| `font` \| `dict` |
| `nombre` | package id / nombre del binario / nombre del repo |
| `instalado_version` / `instalado_sha` | lo que hay **instalado** |
| `disponible_version` / `disponible_sha` | lo que hay **disponible** |
| `accion` | `Actualizar` \| `YaAlDia` \| `NoInstalado` \| `Desconocido` |
| `motivo` | `mismo_sha` \| `sha_distinto` \| `version_distinta` \| `sin_hash` \| `sin_fuente` \| `no_instalado` |
| `detalle` | explicación legible del desfase (versión, sha, tamaño) |
| `instalado_size` / `disponible_size` | bytes |
| `ruta` | dónde vive lo instalado |
| `fuente` | de qué proveedor sale lo disponible |

Salida por pantalla:

```
KIND    NOMBRE            INSTALADA   DISPONIBLE   ACCION       MOTIVO
apk     org.fdroid.fdroid 1.0         1.1          Actualizar   sha_distinto
binary  megadaemon-core   13.4.0     13.4.0       YaAlDia      mismo_sha
repo    megadaemon        main        main         Actualizar   sha_distinto
```

Lo instalado se compone de **tres** fuentes, todas ya existentes en el
daemon (no se reimplementa nada):

| Origen | Módulo | Qué aporta |
|---|---|---|
| APKs | `apk_inventory::inventory()` | package, versión, **SHA-256**, ruta y tamaño de cada APK instalada |
| Binarios | `pkgmgr::instalados_desde_directorio()` (`<state_dir>/pkgmgr/bin`) | ficheros del `state_dir` con su SHA-256 calculado en streaming |
| Repos | `git_ops::status()` | rama y commit del repo de trabajo |

En un PC **sin Android** el inventario devuelve cero y el `git_ops` no
encuentra repo ⇒ la tabla sale vacía y la CLI lo dice honestamente
("no hay nada instalado en esta instancia"). **No inventa nada.**

---

## 6. Eficiencia: las reglas concretas

| Regla | Implementación | Efecto |
|---|---|---|
| Cero descargas si el contenido es idéntico | comparación por SHA-256 en `comprobar_actualizacion()` y en `instalar_con_rollback_detallado()` | un re-build con el mismo contenido **no** descarga nada |
| Reanudable | `Range: bytes=N-` sobre el parcial `<destino>.new` | una descarga cortada **continúa** (móvil sin red a media descarga) |
| Cursor por proveedor | `If-None-Match` / `If-Modified-Since` + manifiesto en caché | no se relee el manifiesto entero si el proveedor no ha cambiado |
| Cuarentena de fallos | backoff 5 min → 15 min → 1 h → 6 h por proveedor | no se reintenta lo que se sabe que falla |
| Una sola copia en disco | el hash se calcula en la misma pasada de la descarga | ni una copia extra del binario (el móvil va justo de espacio) |
| Sin red para el índice | `md-pkg index` lee la caché | el índice se puede pintar sin tocar la red |

---

## 7. Migración desde lo que hay hoy

### 7.1 Tabla de migración

| Hoy | Qué pasa | Estado |
|---|---|---|
| CI genera `dist/state/latest.json` (`assets` + `assets_versioned`, sin hash ni tamaño) | **Convive**: `pkgmgr::adaptar_latest_json()` lo convierte al esquema único si se le pasan los hashes (`.sha256`) y los tamaños. El adaptador normaliza la versión (`sha de commit` → `0.0.0+<sha>`, semver válido con metadatos de build). | **hecho** (adaptador) / el CI debería publicar el formato nuevo (fase 1) |
| CI publica ficheros `.sha256` sueltos | Se siguen publicando (el CI los usa para verificar); el manifiesto nuevo **incluye** el hash ⇒ no hace falta descargarlos aparte. | fase 1 |
| `state/channel.json` (estado del canal) | **No se toca**: sigue siendo el estado del canal. El manifiesto va **anidado** (`{"manifest": {…}}`) o en el `assets` del propio canal. | **hecho** (formato anidado) |
| `repo_updates` (índice F-Droid/Obtainium) | **No se toca**: se sigue usando para *descubrir* apps (el índice de F-Droid tiene su propio formato y su propio `versionCode`). `pkgmgr` lo **consume**: un asset `apk` del manifiesto puede venir del índice de F-Droid. | **consumido**, no unificado |
| `repo_updates::download()` | Sigue como está (descarga a `.part`, verifica, renombra). `pkgmgr` **no lo duplica**: aporta reanudabilidad y atomicidad en el camino del *daemon*. | **no unificado todavía** (ver §10) |
| `repo_updates::deploy_binary()` (`cp -f` sobre el binario vivo) | **Pendiente**: debe delegar en `pkgmgr::instalar_con_rollback()` (`rename` + `.prev`). | **pendiente** (no se toca en este cambio) |
| `deploy/install-phone.sh` (`cp -f "$SRC_BIN" "$BIN"`) | **Pendiente**: mismo cambio (`rename` + `.prev` + verificación del hash). | **pendiente** (no se toca en este cambio) |
| `deploy/99-megadaemon.sh` (supervisor, `git pull` + `auto-update.sh`) | **Pendiente**: el supervisor puede llamar a `md-pkg update` en vez de tirar de `auto-update.sh` (que hoy **no existe en el repo** ⇒ el auto-update no ocurre). | **pendiente** (no se toca en este cambio) |
| `drive_client.rs` (subida/descarga por rangos, SHA-256, cero copias) | **Se reutiliza** el patrón (y `drive_client::sha256_reader` para el hash de ficheros). `drive_client` **no se ha modificado**. | **reutilizado** |

### 7.2 Fase 1 — el CI publica el manifiesto nuevo (15 min, sin tocar Rust)

El CI **ya** calcula los `.sha256`; sólo hay que añadir el manifiesto. En el
job `release`, después de `softprops/action-gh-release`:

```yaml
      - name: Publicar el manifiesto de actualización (esquema único)
        run: |
          mkdir -p dist/state
          python3 - <<'PY'
          import json, os, glob, hashlib, datetime
          assets = []
          for path in sorted(glob.glob("dist/*")):
              if not os.path.isfile(path) or path.endswith(".sha256"):
                  continue
              h = hashlib.sha256(open(path, "rb").read()).hexdigest()
              base = os.path.basename(path)
              if base.endswith("-android-arm64"):      os_, arch = "android", "arm64"
              elif base.endswith("-linux-x86_64"):     os_, arch = "linux", "x86_64"
              elif base.endswith("-linux-arm64"):      os_, arch = "linux", "arm64"
              else:                                    os_, arch = "any", "any"
              name = base
              for suf in ("-android-arm64","-linux-x86_64","-linux-arm64"):
                  name = name.replace(suf, "")
              assets.append({
                  "name": name, "kind": "binary",
                  "os": os_, "arch": arch,
                  "url": f"https://github.com/boolforge/megadaemon/releases/download/v${{GITHUB_SHA}}/{base}",
                  "sha256": h, "size": os.path.getsize(path),
              })
          manifest = {
              "schema_version": 1,
              "version": "0.0.0+" + os.environ["GITHUB_SHA"][:7],
              "released_at": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
              "assets": assets,
          }
          json.dump(manifest, open("dist/state/pkg-manifest.json","w"), indent=2)
          PY
```

Sin este cambio, el `latest.json` sigue funcionando (el adaptador lo
convierte), pero el cliente tiene que ir a por los `.sha256` aparte.

### 7.3 Fase 2 — el daemon usa `md-pkg`

```bash
# En el ciclo del supervisor (cada 6 h), sustituyendo a `auto-update.sh`:
md-pkg check --json          # qué hay pendiente (usa cursores + cuarentena)
md-pkg update --kind binary --name megadaemon-core
```

### 7.4 Fase 3 — una sola ruta de instalación

Que `deploy/install-phone.sh`, `repo_updates::deploy_binary()` y el supervisor
**llamen** a `pkgmgr::instalar_con_rollback()` (o a `md-pkg update`), y que
`auto-update.sh` desaparezca (o se convierta en una envoltura de `md-pkg`).

---

## 8. Tabla de equivalencia de "estándares"

Qué cumple cada componente **hoy** (columnas: ✓ sí, ~ a medias, ✗ no):

| Componente | semver | sha-256 en el manifiesto | HTTP Range (reanudable) | instalación atómica | rollback |
|---|:--:|:--:|:--:|:--:|:--:|
| `apk_inventory` | ~ (`versionName`/`versionCode` de Android, no semver) | ~ (hashea las APK instaladas, pero no publica manifiesto) | ✗ | ✗ | ✗ |
| `repo_updates` (índice) | ~ (`versionCode`) | ✓ (F-Droid sí; Obtainium depende de la fuente) | ✗ | ✓ (`.part` + `rename`) | ✗ |
| `repo_updates::deploy_binary` | ✗ | ✓ (hashea el fichero) | ✗ | ✗ (`cp -f` ⇒ `ETXTBSY`) | ~ (`cp` de vuelta desde `.bak`, pero la copia inicial puede fallar) |
| `repo_updates::apply` (`pm install`) | ✗ | ✓ (verifica antes de instalar) | ✗ | ✗ (`pm install -r`) | ✗ |
| `channel.rs` | ✗ | ✗ | ✗ | ✗ | ✗ |
| CI `build.yml` → `latest.json` | ✗ (sha de commit) | ✗ (en ficheros `.sha256` sueltos) | ✗ | ✗ | ✗ |
| `deploy/install-phone.sh` | ✗ | ✓ (`sha256sum`) | ✗ (`curl`/`wget` completos) | ✗ (`cp -f` sobre el binario vivo) | ~ (`.bak` + `cp` de vuelta) |
| `deploy/99-megadaemon.sh` (supervisor) | ✗ | ~ (`sha16` del binario) | ✗ | ✓ (`mv` de `.restore`) | ✓ (auto-rollback tras N fallos) |
| `deploy/auto-update.sh` | — | — | — | — | — (**no existe en el repo**) |
| `drive_client` | ✗ | ✓ (SHA-256 durante el streaming) | ✓ (`Content-Range`) | ~ (escritura directa, no `rename`) | ✗ |
| **`pkgmgr` (nuevo)** | **✓** | **✓** | **✓** | **✓** (`rename`) | **✓** (`.prev`) |

---

## 9. CLI

```
md-pkg index   [--json] [--state-dir <dir>] [--manifest <fichero>]
md-pkg check   [--json] [--state-dir <dir>] [--manifest <fichero>] [--offline]
md-pkg update  --kind <apk|binary|repo|font|dict> --name <nombre>
               [--dry-run] [--state-dir <dir>] [--destino <ruta>]
md-pkg manifest validate <fichero>
```

* `index` — tabla unificada, **sin red** (lee la caché de manifiestos).
* `check` — refresca las fuentes (cursor + cuarentena) y lista lo pendiente.
* `update` — descarga reanudable, verifica, instala **atómicamente** y deja el
  `.prev`. Con `--dry-run` no descarga nada.
* `manifest validate` — valida un manifiesto contra el esquema único (salida
  legible o `--json`).

En el sandbox (sin móvil) `index` y `check` funcionan y **dicen la verdad**
("no hay nada instalado en esta instancia").

---

## 10. Qué queda SIN unificar (y por qué no se toca aquí)

1. **`repo_updates` (índice F-Droid/Obtainium)**: su formato (`package`,
   `versionCode`, `apk_url`) es el de F-Droid, no el nuestro, y hay una razón
   de peso: F-Droid **no publica semver**, sino `versionCode`. Traducirlo
   exigiría un `versionCode` → semver por app (con una tabla de equivalencias
   que nadie mantiene). `pkgmgr` lo **consume** (un APK del índice puede
   entrar en el manifiesto único) sin reescribirlo.
2. **`repo_updates::download()`**: ya verifica el hash y renombra; lo que le
   falta es la **reanudabilidad**. Sustituirlo por `pkgmgr::descargar_resumible()`
   es un cambio de una función, pero toca un módulo que **no** es de este
   cambio ⇒ se deja escrito en §7.1.
3. **`repo_updates::deploy_binary()`** y **`deploy/install-phone.sh`**: deben
   pasar a `rename` + `.prev` (§7.1, fase 3).
4. **`deploy/99-megadaemon.sh`**: debe llamar a `md-pkg update` en vez del
   `git pull` + `auto-update.sh` (que no existe).
5. **`channel.json`**: **no se toca** (es el estado del canal, no un
   manifiesto). El manifiesto va anidado.
6. **`apk_inventory`**: **no se toca**. `pkgmgr` sólo lo lee.
7. **Play Store / fuentes con credenciales**: no hay ningún endpoint público
   (`repo_updates` ya lo documenta). `pkgmgr` **no** inventa ninguno: un
   proveedor que necesite credencial se configura por entorno/fichero, y las
   credenciales **nunca** van en el manifiesto.

---

## 11. Onde está el código

| Qué | Dónde |
|---|---|
| Esquema, validación, fuentes, comparación, descarga, instalación, índice | `rust-core/src/pkgmgr.rs` |
| CLI | `rust-core/src/bin/md-pkg.rs` |
| Cabling (pendiente) | `rust-core/src/lib.rs` → `pub mod pkgmgr;` |
| Reutilizado sin modificar | `rust-core/src/drive_client.rs` (`sha256_reader`) |
| Consumidos sin modificar | `rust-core/src/apk_inventory.rs`, `rust-core/src/repo_updates.rs`, `rust-core/src/git_ops.rs` |
