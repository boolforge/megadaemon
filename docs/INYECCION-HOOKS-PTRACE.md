# Instrumentación en vivo del proceso de Antigravity (hooks por `ptrace`)

> **Nota**: este documento describe la vía **ptrace** (inyector propio,
> `md-inject`).  Hay **otra** vía en el repo, la de **frida-gum**, descrita en
> `docs/INYECCION-HOOKS.md`: ambas cargan una `.so` en el proceso de la app;
> la de gum usa los bindings oficiales de Frida y la de ptrace no depende de
> `frida-gum` (que no se puede compilar en un sandbox de 3 GB).
> **Estado de este documento**: 2026-11-23 · rama
> `rama/inyeccion-frida-2026-11-23`
>
> Lo que aquí está **MEDIDO** (se ha ejecutado o se ha medido en el móvil) y
> lo que está **NO VERIFICADO** (falta ejecutarlo en el móvil) están
> **separados y etiquetados**. No se afirma nada que no se haya medido.

---

## 0. El problema que resuelve esto

`rust-core/src/frida_hooks.rs` genera los scripts de gancho (SQLite, red y
ficheros) y `rust-core/src/frida_dbi.rs` genera cargadores e inspecciona
procesos. **Ninguno de los dos inyecta**: entregan el *qué* observar, no el
*cómo* entrar en el proceso.

Las dos vías que había se han medido como **cerradas**:

| vía | por qué no vale | MEDIDO |
|-----|-----------------|--------|
| `frida` (crate Rust) | compila `frida-gum` (C enorme) | inviable en este sandbox |
| `frida-server` en el móvil | **no está instalado** | MEDIDO 2026-11-23 |

Lo que **sí** hay en el móvil (MEDIDO 2026-11-23):

* Samsung SM-G998B, Android con **Magisk**;
* `root_exec` del daemon da `uid=0(root)` y `context=u:r:magisk:s0`;
* el daemon tiene ADB nativo, `fs_write` (base64) y `http_transfer`.

⇒ Con `ptrace` desde el dominio `magisk` se puede entrar en el proceso de una
app de terceros **sin `frida-server`**. Eso es lo que hace este trabajo.

---

## 1. Las tres piezas

```
┌──────────────────────────┐        ┌───────────────────────────────┐
│  DAEMON (en el móvil)    │        │  MÓVIL (root)                 │
│                          │        │                               │
│  tool MCP inject_hooks   │──────► │  md-inject  (ptrace)          │
│   ├ estado               │ root_  │      └─ dlopen() en el        │
│   ├ instalar             │ exec   │         proceso objetivo      │
│   ├ inyectar             │        │                               │
│   └ leer                 │        │  hooks-payload-arm64.so       │
│                          │◄────── │      └─ hooks SQLite/red/     │
│  lee el NDJSON (root)    │  NDJSON│         ficheros              │
└──────────────────────────┘        └───────────────────────────────┘
```

| pieza | dónde está | qué es |
|-------|-----------|--------|
| **payload** | `crates/injector-payload/` | la `.so` (`cdylib`) que se carga dentro del proceso de la app y pone los ganchos |
| **inyector** | `rust-core/src/inject.rs` + `rust-core/src/bin/md-inject.rs` | el que hace `ptrace` y llama a `dlopen` en el proceso ajeno |
| **puente** | `rust-core/src/hooks_bridge.rs` | la tool MCP `inject_hooks` (instalar / inyectar / leer / estado) |

---

## 2. El mecanismo de la inyección (paso a paso)

`md-inject inject <pid> <ruta.so>`:

1. **Localiza el PID** leyendo `/proc/*/cmdline` (`inject::resolver_pid`).
2. **`PTRACE_ATTACH`** + `waitpid`: el hilo queda parado.
3. **Resuelve `dlopen` DENTRO del objetivo**: lee `/proc/<pid>/maps`,
   localiza las librerías del objetivo y **parsea su tabla de símbolos
   dinámicos** (ELF64: `PT_DYNAMIC` → `DT_SYMTAB`/`DT_STRTAB`/`DT_HASH` o
   `DT_GNU_HASH`).  La dirección final es `bias_de_carga + st_value`.
   * No se puede usar el `dlsym` del inyector: en el objetivo la dirección de
     `dlopen` es **otra** (ASLR y, en Android, espacios de nombres de montaje
     distintos).
4. **Escribe la ruta de la `.so`** en la memoria del objetivo: **4 KB por
   debajo de su `SP`**, que es memoria libre (la pila crece hacia abajo) y
   donde no hay datos vivos de la app.
5. **Prepara la llamada**: los registros quedan como si el proceso fuese a
   hacer `dlopen(ruta, RTLD_NOW|RTLD_GLOBAL)` y a **volver a la dirección
   "mágica" `0`**, que no está mapeada en ningún proceso (`mmap_min_addr` =
   4096) ⇒ al volver ahí, el kernel para el proceso con un `SIGSEGV`.
6. **Ejecuta** (`PTRACE_CONT`) y **captura** ese `SIGSEGV`: el inyector lo
   **traga** (reanuda con señal `0`), así que la app no se entera.
7. **Restaura los registros originales** y **suelta** el proceso: sigue
   exactamente igual.
8. **Verifica de verdad**: comprueba que la `.so` **aparece en
   `/proc/<pid>/maps`**.  El informe dice `cargada: true/false` según eso, no
   "creo que sí".

### Decisiones y por qué

* **`ptrace` (`POKEDATA`/`PEEKDATA`) y no `/proc/<pid>/mem`**:
  `/proc/<pid>/mem` exige `PTRACE_MODE_ATTACH` para **abrir** el fichero
  (el mismo permiso y el mismo `SELinux` que `ptrace`), mientras que
  `PTRACE_POKEDATA` no abre nada: el permiso lo concede el propio
  `PTRACE_ATTACH`.  Y sin `PTRACE_ATTACH` no hay forma de parar el proceso ni
  de leer sus registros ⇒ sin registros no hay llamada remota posible.
  **Las dos vías están implementadas y son conmutables** (`--mem=ptrace` /
  `--mem=mem`) porque el veredicto de `SELinux` en **este** móvil **no se ha
  medido todavía** (§7).
* **La pila del objetivo como memoria auxiliar** en vez de un `mmap`
  remoto: `mmap` remoto exigiría un *gadget* de syscall en el código del
  objetivo (frágil); la pila es un sitio que ya existe y está mapeada.
* **Dirección de retorno mágica (`0`)** en vez de un *gadget*: no hace falta
  escribir código en el objetivo; sólo hace falta un sitio donde volver que
 provoque un `SIGSEGV` controlable.
* **Restaurar los registros SIEMPRE** (éxito o error): si no, la app queda
  con el `PC` en `0` y se muere al continuar.

---

## 3. Por qué el payload parchea el GOT

Exportar `sqlite3_prepare_v2` **no basta**: en Android el enlazador resuelve
los símbolos **al cargar** la librería (enlace inmediato), así que cuando el
payload entra, las llamadas de la app ya apuntan a la implementación original.

Lo que sí funciona es **reescribir la ranura del GOT** de cada módulo que
importa ese símbolo (`crates/injector-payload/src/elf.rs` lee las tablas
`RELA`/`JMPREL`; `escribir_ranura()` hace `mprotect` porque la GOT está en
RELRO, en solo lectura).

Ganchos instalados:

| grupo | símbolos |
|-------|----------|
| SQLite | `sqlite3_open_v2`, `sqlite3_prepare_v2`, `sqlite3_step` |
| red | `connect`, `sendto`, `getaddrinfo` |
| ficheros | `open`, `openat`, `unlink` |

`getaddrinfo` es el que **verifica el parche del endpoint**: si después de
parchear sigue apareciendo `daily-cloudcode-pa.googleapis.com`, el parche no
está aplicado.

### El NDJSON

* Ruta de referencia: `/data/local/tmp/megadaemon/state/hooks.ndjson`.
* **Si el payload corre dentro de una app de terceros, esa ruta NO se puede
  abrir**: `/data/local/tmp` tiene etiqueta `shell_data_file` y
  `untrusted_app` no tiene permiso de escritura.  El payload cae entonces al
  **directorio privado de la app**
  (`/data/data/<paquete>/files/megadaemon/hooks.ndjson`, etiqueta
  `app_data_file`), que sí puede escribir.  La ruta usada va en **cada
  línea** (campo `"f"`), así que no hay que adivinarla.
* Escritura con `O_APPEND` y **una sola** llamada a `write()` por línea ⇒ la
  línea entra entera, nunca partida.
* Permisos `0600`.
* **Credenciales redactadas** con la **misma** política que el shim del daemon
  (`crates/redact`, usada por `agyshim::redactar` y por el payload): un
  token que el shim tapa, el payload también.

Formato de una línea:

```json
{"ts":1732300000123,"pid":1234,"tid":5678,"hook":"red","tipo":"getaddrinfo","detalle":"daily-cloudcode-pa.googleapis.com:443","f":"/data/data/com.google.antigravity/files/megadaemon/hooks.ndjson"}
```

---

## 4. Compilar el payload

### En el CI (automático)

El job `build-hooks-payload` de `.github/workflows/build.yml` lo compila para
`aarch64-linux-android` con el NDK r26c y lo publica como asset del release:

* `hooks-payload-arm64`
* `hooks-payload-arm64.sha256`
* `hooks-payload-arm64-<sha7>` (+ `.sha256`)

El job es **aparte** del `build-android`: si el payload no compila, el
binario del daemon (el de la instancia maestra) se publica igual.

**El payload NO va en el manifiesto `latest.json`**: ese manifiesto manda la
auto-actualización del *binario del daemon*; el payload se instala a mano.

### En local

```bash
# NDK r26c (el mismo del CI)
export ANDROID_NDK_HOME=/ruta/al/ndk
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android26-clang"
export CC_aarch64_linux_android="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android26-clang"
export AR_aarch64_linux_android="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar"
export RANLIB_aarch64_linux_android="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ranlib"

cd crates/injector-payload
cargo build --release --target aarch64-linux-android
# -> target/aarch64-linux-android/release/libmegadaemon_hooks_payload.so
```

Prueba rápida (host x86_64, sin NDK):

```bash
cd crates/injector-payload && cargo test --release
```

---

## 5. Instalar en el móvil

### Con la tool MCP (recomendado)

```jsonc
// tool: inject_hooks
{ "action": "estado" }        // qué hay puesto: payload, inyector, frida-server, SELinux
{ "action": "instalar", "package": "com.google.antigravity" }
```

`instalar`:

1. busca el `.so` del payload y el binario `md-inject` **en el móvil**
   (`/data/local/tmp/megadaemon/`), o en el directorio de estado del daemon;
2. los copia al **directorio privado de la app**
   (`/data/data/<paquete>/files/megadaemon/`), que es donde el proceso de la
   app puede leerlos (`app_data_file`), y también a la ruta de referencia;
3. `chmod 755` + `restorecon` + `chown` al uid de la app;
4. devuelve la verificación (`ls -lZ`).

### A mano (si el MCP no está disponible)

```bash
su -mm -c '
  set -e
  mkdir -p /data/local/tmp/megadaemon
  mkdir -p /data/data/com.google.antigravity/files/megadaemon
  cp /data/local/tmp/megadaemon/hooks-payload-arm64 \
     /data/data/com.google.antigravity/files/megadaemon/hooks-payload-arm64.so
  cp /data/local/tmp/megadaemon/md-inject-arm64 /data/local/tmp/megadaemon/md-inject
  chmod 755 /data/data/com.google.antigravity/files/megadaemon/hooks-payload-arm64.so
  chmod 755 /data/local/tmp/megadaemon/md-inject
  restorecon /data/data/com.google.antigravity/files/megadaemon/hooks-payload-arm64.so
  chown -R $(stat -c %u /data/data/com.google.antigravity):$(stat -c %g /data/data/com.google.antigravity) \
        /data/data/com.google.antigravity/files/megadaemon
  ls -lZ /data/data/com.google.antigravity/files/megadaemon/hooks-payload-arm64.so
'
```

---

## 6. Inyectar y leer

### Inyectar

```jsonc
// tool: inject_hooks
{ "action": "inyectar", "package": "com.google.antigravity" }
{ "action": "inyectar", "pid": 12345, "via": "ptrace" }   // via: ptrace | mem
```

O a mano:

```bash
su -mm -c '/data/local/tmp/megadaemon/md-inject inject <PID> \
   /data/data/com.google.antigravity/files/megadaemon/hooks-payload-arm64.so --mem=ptrace'
```

Diagnóstico:

```bash
su -mm -c '/data/local/tmp/megadaemon/md-inject list antigravity'
su -mm -c '/data/local/tmp/megadaemon/md-inject find-dlopen <PID>'
su -mm -c '/data/local/tmp/megadaemon/md-inject status <PID>'
```

El informe (`Informe`, JSON) trae: `pid`, `proceso`, `adjunto`, `handle_valido`,
`cargada` (verificado en `/proc/<pid>/maps`), `dlopen_addr`, `via_memoria`,
`error` (con `errno` cuando lo hay) y `pasos` (la traza).

### Leer el NDJSON

```jsonc
// tool: inject_hooks
{ "action": "leer", "lines": 100 }
```

O a mano:

```bash
su -mm -c 'tail -n 100 /data/data/com.google.antigravity/files/megadaemon/hooks.ndjson'
su -mm -c 'tail -n 100 /data/local/tmp/megadaemon/state/hooks.ndjson'   # ruta de referencia
```

---

## 7. Estado real: MEDIDO y NO VERIFICADO

### MEDIDO (2026-11-23)

| hecho | cómo se midió |
|-------|---------------|
| El móvil **no** tiene `frida-server` | inspección del dispositivo |
| `root_exec` da `uid=0(root)`, `context=u:r:magisk:s0` | salida de `root_exec` |
| Los hooks de Frida **sólo entregaban** scripts (no inyectaban) | lectura de `frida_hooks.rs` / `frida_dbi.rs` |
| La crate `frida` (Rust) exige compilar `frida-gum` | `Cargo.toml` / documentación de la crate |
| `/proc/<pid>/cmdline`, `/proc/<pid>/maps` dan lo que hace falta para resolver `dlopen` | lectura del código y del parser ELF (test contra `dlsym` en el CI) |
| El `parser ELF` encuentra `dlopen` en la misma dirección que `dlsym` | test `el_parser_de_elf_encuentra_dlopen_como_dlsym` (corre en el CI, x86_64) |
| La llamada remota se construye con los registros correctos | tests puros de `preparar_llamada` (SP, `PC`, `x0`/`rdi`, `x30`, alineación) |
| La tool `inject_hooks` responde con un error claro (nunca un panic) cuando falta el payload | arnés `rust-core/tests/inject_hooks.rs` |
| El workflow del CI sigue siendo YAML válido | `tools/yaml_lint.py` |

### NO VERIFICADO (falta ejecutarlo en el móvil)

> Cada punto dice **el comando exacto** que lo verifica.  Hasta que se
> ejecuten, no se afirma nada de ellos.

| # | lo que falta | cómo se verifica |
|---|--------------|------------------|
| 1 | Que `PTRACE_ATTACH` a una app de terceros pase el filtro de `SELinux` | `su -mm -c '/data/local/tmp/megadaemon/md-inject inject <PID> <lib>'` y mirar `adjunto: true` en el informe.  Si sale `EPERM`: `su -mm -c 'magiskpolicy --live "allow magisk tracer trace:process"'` y repetir |
| 2 | Que `dlopen` en el proceso ajeno **cargue** la `.so` | el mismo comando: `cargada: true` **y** `ls -lZ` de la ruta, y la `.so` en `/proc/<PID>/maps` |
| 3 | Que el proceso **no se caiga** con la inyección | `su -mm -c 'ps -A -o PID,NAME \| grep antigravity'` antes y después; y que la app siga respondiendo |
| 4 | Que los ganchos **escriban** en el NDJSON | `su -mm -c 'tail -n 20 <ndjson>'` tras usar la app un minuto |
| 5 | Que la ruta de referencia `/data/local/tmp/...` sea escribible desde la app (creo que **no**) | `su -mm -c 'ls -lZ /data/local/tmp/megadaemon/state/hooks.ndjson'` y mirar el campo `"f"` de las líneas |
| 6 | Que el payload **compile** para `aarch64-linux-android` | el CI (job `build-hooks-payload`); en local: el §4 |
| 7 | Que el `md-inject` ARM64 compile y corra en el móvil | el CI (job `build-android`, asset `md-inject-arm64`) |
| 8 | Qué **hosts** contacta de verdad Antigravity | una vez inyectado: `su -mm -c 'grep -o "\"tipo\":\"getaddrinfo\",\"detalle\":\"[^\"]*" <ndjson>' \| sort -u'` |
| 9 | Si `SELinux` deja usar `/proc/<pid>/mem` (`--mem=mem`) | `su -mm -c '/data/local/tmp/megadaemon/md-inject inject <PID> <lib> --mem=mem'` |

---

## 8. Problemas frecuentes y cómo diagnosticarlos

| síntoma | causa probable | qué hacer |
|---------|----------------|-----------|
| `PTRACE_ATTACH ... EPERM` | `SELinux` (`untrusted_app` no es `traceable`) | `magiskpolicy --live "allow magisk tracer trace:process"` |
| `dlopen` devuelve `NULL` | la app no puede **leer** la `.so` (`SELinux` o permisos) | que esté en `/data/data/<paquete>/files/...` con `restorecon` y `chown` al uid de la app |
| `no se ha encontrado 'dlopen'` | la libc del objetivo no se puede parsear | `ls -lZ` de las librerías del objetivo y `md-inject find-dlopen <PID>` |
| El NDJSON no crece | los ganchos no se han instalado (el GOT no se pudo parchear) | el NDJSON tiene una línea `"tipo":"parche"` con cuántas ranuras se parchearon; si dice `0`, no se pudo |
| La app se queda colgada | el `SIGSEGV` de la dirección mágica se entregó | mirar `pasos` del informe; si falta el paso "registros originales restaurados", avisar |

---

## 9. Ficheros

| fichero | qué es |
|---------|--------|
| `rust-core/src/inject.rs` | el inyector (`ptrace`, `/proc`, parser ELF, llamada remota, informe) |
| `rust-core/src/bin/md-inject.rs` | el CLI del inyector (se ejecuta en el móvil como root) |
| `rust-core/src/hooks_bridge.rs` | la tool MCP `inject_hooks` |
| `crates/injector-payload/src/lib.rs` | los ganchos y la instalación (parcheo del GOT) |
| `crates/injector-payload/src/elf.rs` | lector ELF64 mínimo |
| `crates/injector-payload/src/log.rs` | escritor NDJSON (append, redactado) |
| `crates/redact/src/lib.rs` | la política de redacción (compartida con el shim) |
| `rust-core/tests/inject_hooks.rs` | arnés de la tool y del inyector |
| `.github/workflows/build.yml` | job `build-hooks-payload` + asset `md-inject-arm64` |
