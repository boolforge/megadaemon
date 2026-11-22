#!/usr/bin/env bash
# md-board — CLI del tablón unificado de tareas (Board v1).
#
# Envoltorio fino: localiza el binario `md-board` (release del repo, cargo o
# PATH) y lo ejecuta.  Sirve para usar el tablón **sin el enjambre**: un
# usuario puede listar, reclamar y reportar tareas desde su máquina.
#
#   ./tools/md-board.sh --help
#   ./tools/md-board.sh --state-dir /ruta/state tasks
set -euo pipefail

find_bin() {
  if command -v md-board >/dev/null 2>&1; then command -v md-board; return; fi
  local root
  root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  for c in \
    "$root/rust-core/target/release/md-board" \
    "$root/rust-core/target/debug/md-board" \
    "$root/dist/md-board"; do
    [ -x "$c" ] && { echo "$c"; return; }
  done
  echo ""
}

BIN="$(find_bin)"
if [ -z "$BIN" ]; then
  echo "md-board no está compilado todavía." >&2
  echo "  cd rust-core && cargo build --release --bin md-board" >&2
  echo "  (o: cargo run --bin md-board -- --help)" >&2
  exit 127
fi
exec "$BIN" "$@"
