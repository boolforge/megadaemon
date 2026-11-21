#!/usr/bin/env sh
# fix-entorno.sh — ENTRADA DE COMPATIBILIDAD en el PC.
# La implementación ÚNICA es deploy/instalador-megadaemon.sh.
set -u
LOCAL="$(dirname "$0")/instalador-megadaemon.sh"
if [ -f "$LOCAL" ]; then exec sh "$LOCAL" "$@"; fi
CANON="https://raw.githubusercontent.com/boolforge/megadaemon/main/deploy/instalador-megadaemon.sh"
CURL="$(command -v curl 2>/dev/null)"
[ -x "$CURL" ] || { echo "ERROR: no hay curl"; exit 1; }
DEST="${TMPDIR:-/tmp}/instalador-megadaemon.sh"
"$CURL" -fsSL -o "$DEST" "$CANON" || { echo "ERROR: no pude descargar $CANON"; exit 1; }
chmod 755 "$DEST"
exec sh "$DEST" "$@"
