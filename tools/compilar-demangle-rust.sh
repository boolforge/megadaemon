#!/usr/bin/env bash
# Compila el demangler canonico de Rust (rustc-demangle) SIN `cargo`.
#
# POR QUE (medido 2026-11-27): el sandbox no tiene `rustfilt` ni
# `rustc-demangle` instalados y PyPI esta bloqueado (`pip install
# rust-demangle` -> ProtocolError: Connection reset). Pero la cache local de
# cargo tiene el crate `rustc-demangle-0.1.28` (MIT OR Apache-2.0) y hay
# `rustc 1.100.0`.
#
# POR QUE SIN `CARGO`: el root prohibio `cargo` en local (2 CPU, 3 GB sin swap
# => OOM al enlazar). Aqui no se enlaza nada: se compila UN rlib sin
# dependencias con `rustc` y se enlaza un binario de 4,5 MB. Medido: 33 s y
# ~400 MB de RAM.
#
# NOTA: el demangler debe ser STREAMING (linea a linea, con flush). Con un
# binario que lee todo el stdin antes de procesar, escribir linea a linea desde
# Python se queda en deadlock (medido).
set -euo pipefail

CARGO_HOME="${CARGO_HOME:-/workspace/.home/.cargo}"
export PATH="$CARGO_HOME/bin:$PATH"

SRC=$(ls -d "$CARGO_HOME"/registry/src/*/rustc-demangle-* 2>/dev/null | head -1)
if [ -z "$SRC" ]; then
  echo "no encuentro rustc-demangle en la cache de cargo ($CARGO_HOME/registry/src)" >&2
  exit 1
fi

OUT="${1:-/tmp/demang}"
mkdir -p "$OUT"
TMP=$(mktemp -d)

echo "[demangle] crate: $SRC"
rustc --edition 2021 -O --crate-type=rlib --crate-name rustc_demangle \
      "$SRC/src/lib.rs" -o "$TMP/librustc_demangle.rlib"

cat > "$TMP/main.rs" <<'EOF'
use std::io::{BufRead, BufWriter, Write};
fn main() {
    let stdin = std::io::stdin();
    let out = std::io::stdout();
    let mut w = BufWriter::new(out.lock());
    for line in stdin.lock().lines() {
        let l = match line { Ok(s) => s, Err(_) => break };
        let t = l.trim_end_matches('\r');
        if t.is_empty() { continue; }
        writeln!(w, "{}\t{}", t, rustc_demangle::demangle(t)).unwrap();
        w.flush().unwrap();
    }
}
EOF

rustc --edition 2021 -O "$TMP/main.rs" \
      --extern rustc_demangle="$TMP/librustc_demangle.rlib" -o "$OUT/rustdemangle"
rm -rf "$TMP"
echo "[demangle] binario: $OUT/rustdemangle"
printf '_RNvNCNKNvNtCscKrEppzIDTR_12codex_config12path_context12PATH_CONTEXT0023___RUST_STD_INTERNAL_VAL\n' \
  | "$OUT/rustdemangle"
