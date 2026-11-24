#!/usr/bin/env bash
# Descarga el binario más nuevo de Claude Code y extrae lo legible.
#
# MEDIDO 2026-11-24: el paquete npm de la CLI ya no publica código fuente.
# Son 32 KB con un lanzador (`cli-wrapper.cjs`) que copia encima un BINARIO
# NATIVO por plataforma (el paquete opcional `@anthropic-ai/claude-code-linux-x64`).
# Ese binario (398 MB, Bun compilado a ELF) SÍ tiene dentro las prompts, los
# manifiestos de plugin y las reglas: en la tabla de cadenas (UTF-16LE) y en
# algunos bloques JS en texto plano.
#
# NO se sube al repo nada de ese contenido: esto deja las MEDIDAS y permite
# repetirlas.  Lo que se documenta está en docs/RE_CLAUDE_CODE_2_1_331.md.
#
# Uso:  tools/re-claude-code.sh [version]      (por defecto: la última de npm)
set -euo pipefail
VER="${1:-latest}"
TMP="${TMPDIR:-/tmp}/cc-re"
mkdir -p "$TMP"; cd "$TMP"

echo "== 1) metadatos del paquete de la CLI =="
if [ "$VER" = "latest" ]; then
  VER=$(curl -sL "https://registry.npmjs.org/@anthropic-ai/claude-code" |
        python3 -c 'import json,sys; print(json.load(sys.stdin)["dist-tags"]["latest"])')
fi
echo "   version: $VER"

echo "== 2) binario nativo (linux-x64) =="
TB=$(curl -sL "https://registry.npmjs.org/@anthropic-ai/claude-code-linux-x64/$VER" |
     python3 -c 'import json,sys; print(json.load(sys.stdin)["dist"]["tarball"])')
curl -sL "$TB" -o pkg.tgz
tar -xzf pkg.tgz
BIN=$(find . -type f -size +1M | head -1)
echo "   binario: $BIN ($(stat -c%s "$BIN") bytes)"
file "$BIN" | sed 's/^/   /'

echo "== 3) marcadores dentro del binario =="
python3 - "$BIN" <<'PY'
import mmap, sys
mm = mmap.mmap(open(sys.argv[1],'rb').fileno(), 0, access=mmap.ACCESS_READ)
for m in [b"// @bun", b"You are Claude Code", b"EnterPlanMode", b"SKILL.md",
          b".claude-plugin", b".lsp.json", b"session goal", b"stop check",
          b"checkpoint", b"compaction"]:
    i = mm.find(m)
    j = mm.find(m.decode().encode('utf-16-le'))
    print(f"   {m.decode():18} ascii={i:>12} utf16={j:>12}")
PY

echo "== 4) ¿el paquete trae mapa de fuentes? =="
curl -sL "https://registry.npmjs.org/@anthropic-ai/claude-code/$VER" |
  python3 -c 'import json,sys; d=json.load(sys.stdin); print("   tarball:", d["dist"]["tarball"])'
echo "   (0 mapas => a partir de 2.1.x no se publica código JS)"
