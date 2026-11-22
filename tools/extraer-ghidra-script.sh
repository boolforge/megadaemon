#!/usr/bin/env bash
# Extrae el GhidraScript embebido en rust-core/src/ghidra.rs (MD_GHIDRA_SCRIPT)
# a tools/ghidra-scripts/MdGhidra.java.
# La FUENTE UNICA es el modulo Rust: asi el script usado por linea de
# comandos nunca se desincroniza del que usa `md-ghidra`.
set -euo pipefail
RAIZ="$(cd "$(dirname "$(readlink -f "$0")")/.." && pwd)"
SRC="$RAIZ/rust-core/src/ghidra.rs"
DST_DIR="$RAIZ/tools/ghidra-scripts"
DST="$DST_DIR/MdGhidra.java"
mkdir -p "$DST_DIR"
python3 - "$SRC" "$DST" <<'PY'
import io, re, sys
src, dst = sys.argv[1], sys.argv[2]
s = io.open(src, encoding='utf-8').read()
m = re.search(r'MD_GHIDRA_SCRIPT[^=]*=\s*r#"(.*?)"#;', s, re.S) or \
    re.search(r'MD_GHIDRA_SCRIPT[^=]*=\s*r"(.*?)";', s, re.S)
if not m:
    sys.exit("no encuentro MD_GHIDRA_SCRIPT en " + src)
java = m.group(1)
old = io.open(dst, encoding='utf-8').read() if __import__('os').path.exists(dst) else None
if old != java:
    io.open(dst, 'w', encoding='utf-8').write(java)
    print("escrito:", dst, len(java), "bytes")
else:
    print("sin cambios:", dst)
PY
