#!/data/data/com.termux/files/usr/bin/sh
# fix-movil.sh — ENTRADA DE COMPATIBILIDAD (el móvil ya tenía esta URL).
#
# La implementación ÚNICA está en deploy/fix-entorno.sh: un solo fichero
# que detecta si se ejecuta en el móvil o en el PC y hace lo correcto en
# cada caso.  Este wrapper sólo lo descarga y lo ejecuta, para que la
# orden que ya usas siga funcionando y tome la versión FIXED.
#
#   sh fix-movil.sh              # instala (arregla)
#   sh fix-movil.sh --diagnostico  # sólo diagnostica, no toca nada
#
# MEDIDO el 2026-11-21: esta versión vieja se rompía en el paso 2 con
# "curl: (22) 404" porque buscaba el tag de release "latest", que NO
# existe: los tags son `v<sha>`.  El canónico resuelve la release por la
# API de GitHub y por eso ya no da 404.
set -u

CANON="https://raw.githubusercontent.com/boolforge/megadaemon/main/deploy/fix-entorno.sh"
DEST="${TMPDIR:-/tmp}/fix-entorno.sh"

# si ya estamos en un clon del repo, se usa el fichero local (más fiable)
LOCAL="$(dirname "$0")/fix-entorno.sh"
if [ -f "$LOCAL" ]; then
  exec sh "$LOCAL" "$@"
fi

CURL="$(command -v curl 2>/dev/null || echo /data/data/com.termux/files/usr/bin/curl)"
[ -x "$CURL" ] || { echo "ERROR: no hay curl (pkg install curl)"; exit 1; }
"$CURL" -fsSL -o "$DEST" "$CANON" || { echo "ERROR: no pude descargar $CANON"; exit 1; }
chmod 755 "$DEST"
exec sh "$DEST" "$@"
