#!/usr/bin/env bash
# Envoltorio LOCAL del helper del MITM (el que se instala en el movil).
#
# La implementación vive en UN solo sitio:
#   deploy/magisk-mitm-sqlite/mitm-sqlite.sh
# y este script es su equivalente en el repo (mismo argumentos, misma
# salida JSON).  En el movil, el modulo Magisk usa la copia que hay en
#   /data/adb/modules/megadaemon_mitm_sqlite/mitm-sqlite.sh
#
# USO: tools/mitm-sqlite.sh activar|desactivar|estado [opciones]
set -uo pipefail
RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
IMPL="$RAIZ/deploy/magisk-mitm-sqlite/mitm-sqlite.sh"
if [ ! -x "$IMPL" ]; then
  echo '{"ok":false,"error":"no encuentro el helper: '"$IMPL"'"}' >&2
  exit 1
fi
exec sh "$IMPL" "$@"
