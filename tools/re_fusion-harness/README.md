# Arnés de pruebas de `re_fusion.rs`

`rust-core/src/re_fusion.rs` depende de la lib completa de MegaDaemon (~200
dependencias). En un sandbox de 3 GB **el enlace de esa lib no cabe**
(SIGKILL del OOM killer), así que este arnés compila **el módulo real**
(vía `#[path]`) contra **stubs** de los motores pesados y ejecuta sus tests.

```bash
cd tools/re_fusion-harness
CARGO_HOME=/workspace/.home/.cargo cargo test --offline      # 3 tests del módulo
CARGO_HOME=/workspace/.home/.cargo cargo run --offline --bin fusion   # fusion real
```

Lo que valida:
* `leer_simbolos` (161.682 símbolos del export de `unstrip`).
* `comparar` (quién gana en cada métrica; los métodos que fallan no ganan).
* `cuenta` (número / array / `total`).
* `fusionar()` real: cruza el pseudocódigo de Ghidra (734 funciones) con el
  pclntab y con las strings ⇒ **734/734 con su origen Go**.

En el CI (con memoria de sobra) el módulo se compila dentro de la lib
completa: `cargo build --bin md-ghidra`.
