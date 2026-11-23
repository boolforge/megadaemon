# hooks-payload — payload de instrumentación con **frida-gum**

`cdylib` que se carga (`dlopen`) en el proceso objetivo y registra **SQLite**,
**red** y **ficheros** en un **NDJSON** (una línea JSON por evento), con las
credenciales **redactadas**.

Usa los **bindings oficiales de Frida en Rust** (`frida-gum` 0.17.2 de
crates.io), **no** un inyector artesanal.

## Crate AISLADO

Este paquete tiene su propio `[workspace]`: **no** entra en el build del
daemon (`rust-core`).  El binario principal no depende de gum, ni de
bindgen (libclang), ni del devkit de Frida.

## Compilar (Android ARM64)

```bash
export NDK=/opt/android-ndk-r26c
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android26-clang
export CC_aarch64_linux_android=$CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER
export AR_aarch64_linux_android=$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar
export RANLIB_aarch64_linux_android=$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ranlib
cargo build --release --target aarch64-linux-android
# => target/aarch64-linux-android/release/libhooks_payload.so
```

Requisitos: `libclang-dev` (bindgen), el clang del NDK (los shims en C de
`frida-gum-sys`) y **red** (con `auto-download`, `frida-build` descarga el
devkit **precompilado** de gum: **no** hacen falta meson ni ninja).

En el CI: job `build-hooks-payload` de `.github/workflows/build.yml`
(publica `hooks-payload-arm64` + `.sha256`) y job `hooks-payload-test`
(tests en el host).

## API de frida-gum usada (0.17.2)

`Gum::obtain()` · `Interceptor::obtain(&gum)` ·
`Process::obtain(&gum).find_module_by_name()` ·
`Module::find_export_by_name()` · `Module::find_global_export_by_name()` ·
`Interceptor::attach(ptr, &mut listener)` · `InvocationContext::arg(n)`.

Docs: <https://docs.rs/frida-gum/0.17.2/frida_gum/>

## Ficheros

| Fichero | Qué es |
|---|---|
| `src/lib.rs` | gum, instalador, `megadaemon_hooks_start()` |
| `src/hooks.rs` | los hooks (listeners) y la búsqueda de símbolos |
| `src/sink.rs` | escritor NDJSON (`libc::open`/`write`, `O_APPEND`) |
| `src/redact.rs` | redacción de credenciales (criterio de `agyshim::redactar`) |
| `src/sys.rs` | lecturas seguras (cadenas C, `sockaddr`, uid, paquete) |
| `src/json.rs` | escapado JSON mínimo (sin `serde`) |

La documentación completa (arquitectura, carga, lectura, estado real) está en
[`docs/INYECCION-HOOKS.md`](../../docs/INYECCION-HOOKS.md).
