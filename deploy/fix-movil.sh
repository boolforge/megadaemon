#!/data/data/com.termux/files/usr/bin/sh
# fix-movil.sh — ENTRADA DE COMPATIBILIDAD (esta URL ya se usaba).
# La implementación ÚNICA es deploy/instalador-megadaemon.sh: los dos
# caminos (móvil y PC) en un solo instalador.
#
#   sh fix-movil.sh              # instala (arregla)
#   sh fix-movil.sh --diagnostico
#
# MEDIDO el 2026-11-21: la versión vieja de este fichero se rompía con
# "curl: (22) 404" porque buscaba el tag de release "latest", que NO
# existe (los tags son v<sha>).  El canónico usa la URL estable
# /releases/latest/download/<asset>.
set -u
CANON="https://raw.githubusercontent.com/boolforge/megadaemon/main/deploy/instalador-megadaemon.sh"
LOCAL="$(dirname "$0")/instalador-megadaemon.sh"
if [ -f "$LOCAL" ]; then exec sh "$LOCAL" "$@"; fi
CURL="$(command -v curl 2>/dev/null || echo /data/data/com.termux/files/usr/bin/curl)"
[ -x "$CURL" ] || { echo "ERROR: no hay curl (pkg install curl)"; exit 1; }
DEST="${TMPDIR:-/tmp}/instalador-megadaemon.sh"
"$CURL" -fsSL -o "$DEST" "$CANON" || { echo "ERROR: no pude descargar $CANON"; exit 1; }
chmod 755 "$DEST"
exec sh "$DEST" "$@"
