# INYECCIÓN DE HOOKS — payload nativo en Rust con **frida-gum**

> **Qué es esto**: el motor de instrumentación **en Rust** que observa el
> proceso de Antigravity (`com.agy`) en el móvil: **SQLite**, **red** y
> **ficheros**, escribiendo **una línea JSON por evento** (NDJSON) que el
> daemon lee con una tool MCP.
>
> **Cómo se instrumenta**: con los **bindings oficiales de Frida en Rust**
> (`frida-gum` / `frida-gum-sys` de crates.io), **no** con un inyector
> artesanal: gum se encarga de localizar la función, parchear la entrada y
> devolver el control al original.

---

## 1. Arquitectura

```
┌──────────────────────── MÓVIL (ARM64, Magisk/root) ─────────────────────────┐
│                                                                            │
│  proceso objetivo (com.agy)                                               │
│    └─ libhooks_payload.so  (dlopen)                                        │
│         ├─ frida-gum 0.17.2  ──► hooks sobre libc.so / libsqlite3.so     │
│         │    open / openat / unlink                                        │
│         │    connect / sendto / getaddrinfo                                │
│         │    sqlite3_open_v2 / sqlite3_prepare_v2 / sqlite3_step           │
│         └─ escritor NDJSON (libc::open + libc::write, O_APPEND)           │
│                    │                                                        │
│                    ▼                                                        │
│    /data/local/tmp/megadaemon/state/hooks.ndjson   (1 línea = 1 evento)   │
│                                                                            │
│  MegaDaemon (root) ── tool MCP `hooks_payload`                             │
│    estado | instalar | activar | leer                                      │
└────────────────────────────────────────────────────────────────────────────┘
```

* El **payload** (`crates/hooks-payload/`) es una `cdylib` **aislada**: su
  propio `[workspace]`, sus propias dependencias.  El crate del daemon
  (`rust-core`) **no** depende de gum, ni de bindgen, ni del devkit de Frida:
  el build por defecto (CI, móvil) no se entera de que esto existe.
* El **lado daemon** (`rust-core/src/hooks_payload.rs`) no lleva gum: usa
  `self_update` (descarga + SHA-256), `android::root_exec` (root real) y
  `std::fs` (ficheros del propio móvil).

---

## 2. La API real de frida-gum que se usa

Todo lo de abajo está **medido** sobre el crate publicado
(`frida-gum 0.17.2`, 2026-05-03), leído del `.crate` de crates.io:

| Qué | Cómo (firma real) |
|---|---|
| Runtime | `frida_gum::Gum::obtain() -> Gum` |
| Motor de hooks | `frida_gum::interceptor::Interceptor::obtain(&Gum) -> Interceptor` |
| Localizar el módulo | `frida_gum::Process::obtain(&Gum).find_module_by_name(&str) -> Option<Module>` |
| Localizar la función | `Module::find_export_by_name(&str) -> Option<NativePointer>` (o `Module::find_global_export_by_name(&str)`) |
| Hookear | `Interceptor::attach(NativePointer, &mut I) -> Result<Listener, Error>` (`I: InvocationListener`) |
| Callback | `fn on_enter(&mut self, ctx: InvocationContext)` / `fn on_leave(&mut self, ctx: InvocationContext)` |
| Argumentos | `InvocationContext::arg(n: u32) -> usize`, `::thread_id() -> u32` |

Docs: <https://docs.rs/frida-gum/0.17.2/frida_gum/> ·
<https://docs.rs/frida-gum/0.17.2/frida_gum/interceptor/>

**OJO con dos cosas** (medidas, no supuestas):

1. `Module::get(...)` y `Function::by_name(...)` **no existen en `frida-gum`**:
   son del crate `frida` (el cliente remoto de Frida).  En gum embebido la
   búsqueda es **módulo + export** (`find_export_by_name`), que es
   exactamente lo que gum usa por debajo.  En `main` de `frida-rust` la API
   ya cambió (`Module::obtain`, `find_export_by_name(None, …)`), pero **eso no
   está publicado**: por eso el `Cargo.toml` del payload fija
   `frida-gum = "=0.17.2"` (versión exacta) y no `"0.17"`.
2. El `Interceptor` **no se puede soltar**: gum desengancha los hooks cuando
   se libera el interceptor, y el `Listener` debe seguir vivo (gum guarda su
   puntero en la vtable).  Por eso en el payload **ambos se fugan**
   (`Box::leak`): viven lo que el proceso.

---

## 3. Hooks implementados

| Hook | Qué se registra | Campo |
|---|---|---|
| `open`, `openat`, `unlink` | ruta + acción | `{"hook":"ficheros","event":"open","ruta":"…"}` |
| `connect` | host:puerto del socket | `{"hook":"red","event":"connect","host":"…","puerto":443}` |
| `sendto` | host:puerto del datagrama | `{"hook":"red","event":"sendto",…}` |
| `getaddrinfo` | nombre resuelto + servicio | `{"hook":"red","event":"getaddrinfo","host":"…","servicio":"443"}` |
| `sqlite3_open_v2` | fichero de la base de datos | `{"hook":"sqlite","event":"open","fichero":"…"}` |
| `sqlite3_prepare_v2` | consulta (redactada) + tabla | `{"hook":"sqlite","event":"prepare","sql":"…","tabla":"…"}` |
| `sqlite3_step` | tabla que se está tocando | `{"hook":"sqlite","event":"step","tabla":"…"}` |
| `sqlite3_finalize` | (limpia el mapa de sentencias) | — |

El **NDJSON** (una línea por evento):

```json
{"ts":1764000000123,"pid":8123,"tid":8140,"hook":"red","event":"connect","host":"daily-cloudcode-pa.googleapis.com","puerto":443}
{"ts":1764000000456,"pid":8123,"tid":8140,"hook":"sqlite","event":"prepare","sql":"SELECT * FROM conversations WHERE id=?","tabla":"conversations"}
```

La primera línea de cada carga es el evento de arranque, que dice **qué ruta
de NDJSON se está usando** y **qué hooks quedaron instalados**:

```json
{"ts":1764000000000,"pid":8123,"tid":0,"hook":"payload","event":"iniciado","version":1,"ndjson":"/data/local/tmp/megadaemon/state/hooks.ndjson","ganchos":[{"simbolo":"open","modulo":"libc.so","estado":"instalado"}, …]}
```

### 3.1 Redacción de credenciales (obligatoria)

El payload **no** escribe nunca un secreto.  Se replica el **criterio** de
`rust-core/src/agyshim.rs` (`redactar`: mismos patrones `api_key`, `apiKey`,
`Authorization`, `bearer `, `x-goog-api-key`, y mismo formato de salida
`patron=<REDACTADO>`), con dos añadidos que el shim no necesitaba:

* `password`, `passwd`, `secret`, `token`, `cookie`, `access_token`,
  `refresh_token`, `csrf_token`, `x-api-key`, `session_id`… (sólo si van
  seguidos de `=` o `:`, para no mutilar una tabla que se llame `tokens`);
* las **credenciales embebidas en una URL** (`https://user:pass@host/…` →
  `https://user:<REDACTADO>@host/…`).

Además el payload **no** hookea `SSL_write`/`SSL_read` (a diferencia de los
scripts JS de `frida_hooks.rs`): así **no hay cuerpos HTTP** que puedan
filtrar un token.  De la red sólo se registra `host:puerto`.

Implementación: `crates/hooks-payload/src/redact.rs` (con tests que
reproducen el test del shim: `redacta_credenciales`).

### 3.2 Concurrencia y robustez

* **Nunca panic**: cada hook va dentro de `catch_unwind` (un panic
  desenrollando por los frames de C de gum es UB).
* **Nunca deadlock**: el escritor NDJSON tiene un **guardián de
  re-entrada** por hilo (`EN_ESCRITURA`): nuestro propio `open()` dispara el
  hook de ficheros y ese hook se hace el sordo.  El mutex sólo protege el
  `fd`; nunca se llama a una función hookeada con el mutex tomado (salvo
  `open`, protegido por el guardián).
* **Nunca bloquea**: en el camino caliente no hay `std::fs`: sólo
  `libc::open` + `libc::write` (una llamada al sistema por línea, con
  `O_APPEND`).
* **Sin estado por hilo en el listener**: gum llama al listener desde
  varios hilos con el mismo `&mut` ⇒ el estado compartido (SQL→sentencia)
  va en un `Mutex<HashMap<..>>` global, no en el listener.

---

## 4. Compilar el payload

### 4.1 Requisitos EXACTOS (medidos el 2026-11-23)

| Requisito | Por qué | Cómo se cumple |
|---|---|---|
| **Rust estable** + target `aarch64-linux-android` | el payload es una `.so` ARM64 | `dtolnay/rust-toolchain@stable` con `targets: aarch64-linux-android` |
| **NDK r26c** | `cc` compila los shims en C de `frida-gum-sys` y el linker final | `nttld/setup-ndk@v1` con `ndk-version: r26c` |
| **`libclang-dev` + `clang`** | `frida-gum-sys` usa **bindgen 0.72.1** para generar los bindings a `gum.h` | `sudo apt-get install -y libclang-dev clang` |
| **Red (github.com/frida/frida/releases)** | con la feature `auto-download`, `frida-build` descarga el **devkit precompilado** de gum | el propio `cargo build` lo hace |
| **clang del NDK** (`CC_aarch64_linux_android`) | `cc` necesita un compilador C **del target** | `$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android26-clang` |
| **linker del NDK** | enlazar la `.so` | `CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=…/aarch64-linux-android26-clang` |

> **NO hacen falta `meson` ni `ninja`**: con `auto-download`, `frida-build`
> **no compila gum**, descarga el devkit ya compilado.  Medido:
> `frida-gum-sys 0.17.2` fija `FRIDA_VERSION = 17.9.5`, y el asset
> `frida-gum-devkit-17.9.5-android-arm64.tar.xz` (9,5 MB) existe en el
> release 17.9.5 de Frida (HTTP 200 medido).  El devkit contiene
> `libfrida-gum.a` (estática), `frida-gum.h` y `frida-gum-example.c`.

### 4.2 El comando

```bash
# desde la raíz del repo
cd crates/hooks-payload
export NDK=/opt/android-ndk-r26c   # el que use el runner
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android26-clang
export CC_aarch64_linux_android=$CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER
export AR_aarch64_linux_android=$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar
export RANLIB_aarch64_linux_android=$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ranlib
cargo build --release --target aarch64-linux-android
# => target/aarch64-linux-android/release/libhooks_payload.so
```

En el CI esto es el job **`build-hooks-payload`** de
`.github/workflows/build.yml`; publica **`hooks-payload-arm64`** y
**`hooks-payload-arm64.sha256`** como assets del release.  Hay además un job
**`hooks-payload-test`** que corre los tests del payload en el host (x86_64),
para validar la redacción y el escritor NDJSON sin tocar el móvil.

> **El payload NO entra en `latest.json`**: ese manifiesto es el del **binario
> del daemon** (el móvil se autoactualiza comparando `version` y bajando el
> asset de su plataforma).  El payload tiene su propio ciclo: la tool
> `hooks_payload` baja siempre el asset `hooks-payload-arm64` del último
> release, verifica su SHA-256 y lo reinstala.  Meterlo en `latest.json`
> haría que el daemon se autoactualizara (y se reiniciara) sin motivo.

---

## 5. Instalar, activar y leer (desde el daemon)

Tool MCP **`hooks_payload`** (`rust-core/src/hooks_payload.rs`):

| Acción | Qué hace |
|---|---|
| `estado` | ¿existe el payload? sha256, bytes y modo · ¿vive el proceso objetivo? (pids) · ¿qué cargadores hay? · ¿hay NDJSON y de qué tamaño? |
| `instalar` | descarga `hooks-payload-arm64` + `.sha256` del release, **verifica el SHA-256** y lo deja en `/data/local/tmp/megadaemon/hooks-payload.so` con modo **0700** (escritura atómica `.new` + `rename`; si el daemon no tiene permiso, usa `su -mm -c`) |
| `activar` | carga el payload en el proceso objetivo invocando el cargador por `root_exec` (ver §6) |
| `leer` | las últimas N líneas del NDJSON (lee sólo la cola: 64 KiB) |

Ejemplo (JSON-RPC del MCP):

```json
{"name":"hooks_payload","arguments":{"accion":"instalar"}}
{"name":"hooks_payload","arguments":{"accion":"activar","cargador":"auto","paquete":"com.agy"}}
{"name":"hooks_payload","arguments":{"accion":"leer","n":40}}
```

---

## 6. Cómo se carga en el proceso objetivo

### Vía A — `frida-server` + cliente `frida` (la implementada)

1. El daemon escribe el cargador JS en
   `/data/local/tmp/megadaemon/load-hooks-payload.js`:
   ```js
   const so = '/data/local/tmp/megadaemon/hooks-payload.so';
   const m = Module.load(so);     // API oficial de Frida: Module.load(path)
   ```
   (`Module.load(path)` carga una biblioteca nativa **desde una ruta**:
   documentado en <https://frida.re/docs/javascript-api/>.)
2. Y ejecuta (por `root_exec`, o sea `su -mm -c`):
   ```bash
   timeout 25 frida -U -n com.agy -l /data/local/tmp/megadaemon/load-hooks-payload.js -q
   ```
3. La `.so` **se queda cargada** en el proceso aunque el cliente salga ⇒ los
   hooks siguen vivos.

### Vía B — módulo Magisk/Zygisk  ⚠️ `NO VERIFICADO`

Un módulo Magisk con Zygisk que haga `dlopen` del payload al arrancar el
proceso.  La tool lo detecta (`/data/adb/modules/*hook*`, `*frida*`) y
`activar` con `cargador=zygisk` para la app (Zygisk inyecta al arrancar el
proceso).  **El módulo en sí NO está escrito**: es trabajo pendiente.

---

## 7. Verificación en el móvil (comandos exactos)

```bash
# 0) el payload está instalado y es el bueno
adb shell su -mm -c 'ls -l /data/local/tmp/megadaemon/hooks-payload.so'
adb shell su -mm -c 'sha256sum /data/local/tmp/megadaemon/hooks-payload.so'
#    y comparar con el .sha256 del release:
curl -sSL https://github.com/boolforge/megadaemon/releases/latest/download/hooks-payload-arm64.sha256

# 1) el proceso objetivo vive
adb shell su -mm -c 'pidof com.agy'

# 2) cargar el payload (Vía A)
adb shell su -mm -c 'frida -U -n com.agy -l /data/local/tmp/megadaemon/load-hooks-payload.js -q'

# 3) el NDJSON crece
adb shell su -mm -c 'tail -f /data/local/tmp/megadaemon/state/hooks.ndjson'

# 4) desde el daemon (MCP)
hooks_payload{accion: estado}
hooks_payload{accion: leer, n: 40}

# 5) ¿qué hooks se instalaron de verdad?  (la 1ª línea del NDJSON)
adb shell su -mm -c "head -1 /data/local/tmp/megadaemon/state/hooks.ndjson"

# 6) ¿se ve el parche del endpoint?  (host:puerto de red)
adb shell su -mm -c "grep '\"hook\":\"red\"' /data/local/tmp/megadaemon/state/hooks.ndjson | tail -20"
```

Si el NDJSON no está en la ruta canónica, el evento `payload/iniciado` dice
**cuál** se usó (el payload prueba, en orden: `$MEGADAEMON_HOOKS_NDJSON`,
`/data/local/tmp/megadaemon/state/hooks.ndjson`,
`…/hooks-<uid>.ndjson`, `/data/local/tmp/hooks-<uid>.ndjson`,
`<cache de la app>/megadaemon-hooks.ndjson`, `$TMPDIR/…`).

---

## 8. Estado real: MEDIDO vs `NO VERIFICADO`

### MEDIDO (en este sandbox, 2026-11-23)

* crates.io: `frida-gum 0.17.2` (2026-05-03) existe y publica las features
  `std`, `invocation-listener`, `auto-download`, `event-sink`,
  `stalker-observer`, `stalker-params`, `memory-access-monitor`,
  `module-names`, `backtrace`, `script`.
* `frida-gum-sys 0.17.2`: build-dependencies `bindgen 0.72.1`, `cc 1.11.3`,
  `frida-build 0.17.2`; `FRIDA_VERSION = 17.9.5`.
* `frida-build` **descarga el devkit precompilado** (no compila gum) ⇒ sin
  meson/ninja.
* El asset `frida-gum-devkit-17.9.5-android-arm64.tar.xz` (9.574.924 B)
  existe (HTTP 200) y contiene `libfrida-gum.a`, `frida-gum.h`,
  `frida-gum-example.c`.
* El grafo de dependencias **resuelve**: `cargo generate-lockfile` →
  176 paquetes, con `frida-gum`/`frida-gum-sys`/`frida-build` 0.17.2,
  `bindgen` 0.72.1, `ctor` 1.0.14.
* API de gum 0.17.2 **leída del `.crate`** (no de memoria) y usada tal cual.
* `Module.load(path)` carga una `.so` desde una ruta (docs oficiales de
  Frida).
* Todos los `.rs` del repo pasan `tools/check-sintaxis.sh` (rustfmt parsea).

### `NO VERIFICADO` (falta el móvil / el CI)

* **El payload NO se ha compilado aquí**: el sandbox tiene 3 JVM de Ghidra
  (2,4 GB) y compilar lo mata por OOM (protocolo de la flota).  Lo compila
  el root o el CI (`build-hooks-payload`).
* **El payload NO se ha inyectado en ningún proceso real**: no hay proceso
  instrumentado en este entorno.
* **La Vía A (frida-server + cliente `frida`) no se ha probado en el móvil**:
  el móvil no tiene (medido) cliente `frida` instalado; la tool lo detecta y
  devuelve los comandos exactos.
* **La Vía B (módulo Zygisk/Magisk) no está escrita**: sólo la detección y el
  `force-stop` que la deja injectable.
* **Los hooks dentro de una app real no se han visto**: que `open`,
  `connect` o `sqlite3_prepare_v2` se hookeen depende de que el símbolo
  esté **exportado** en el proceso (SQLite enlazado estáticamente no exporta
  nada ⇒ el payload lo reporta como `ausente` en el informe de arranque).
* La ruta del NDJSON en el móvil depende de los permisos del uid de la app:
  `/data/local/tmp` es de root ⇒ el payload cae a la `cache` de la app.

---

## 9. Ficheros

| Ruta | Qué es |
|---|---|
| `crates/hooks-payload/Cargo.toml` | crate **aislado** (`[workspace]` propio), `cdylib`, `frida-gum = "=0.17.2"` |
| `crates/hooks-payload/src/lib.rs` | punto de entrada: gum, instalador, `megadaemon_hooks_start()` |
| `crates/hooks-payload/src/hooks.rs` | los hooks (listeners de gum) y la búsqueda de símbolos |
| `crates/hooks-payload/src/sink.rs` | escritor NDJSON (`libc::open`/`write`, `O_APPEND`, guardián de re-entrada) |
| `crates/hooks-payload/src/redact.rs` | redacción de credenciales (criterio de `agyshim::redactar`) |
| `crates/hooks-payload/src/sys.rs` | lecturas seguras (cadenas C, `sockaddr`, uid, paquete) |
| `crates/hooks-payload/src/json.rs` | escapado JSON mínimo (sin `serde`) |
| `rust-core/src/hooks_payload.rs` | lado daemon: `estado`/`instalar`/`activar`/`leer` (SIN gum) |
| `rust-core/src/mcp_ext.rs` | tool MCP `hooks_payload` (definición + despacho) |
| `.github/workflows/build.yml` | jobs `build-hooks-payload` y `hooks-payload-test` |
