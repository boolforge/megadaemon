#!/usr/bin/env bash
# Entrega de un bloque de trabajo de un AGENTE al root (sin compilar).
#
# PROTOCOLO DE LA FLOTA (2026-11-22, ver docs/PROTOCOLO_FLOCKA.md):
#   1. El agente trabaja en SU rama (nunca main) y en SU worktree.
#   2. Commitea en bloques PEQUENOS (cada fichero/funcion = un commit).
#   3. NO compila, NO testea, NO hace push a main: lo hace el root
#      (el sandbox tiene 3 GB y los builds en paralelo mueren por OOM).
#   4. Publica el SHA y los ficheros tocados en docs/COORDINATION.md.
#
# USO: tools/agente-entrega.sh "<mensaje>" [fichero...]
set -uo pipefail
MSG="${1:-}"
shift || true
RAIZ="$(git rev-parse --show-toplevel 2>/dev/null || echo .)"
cd "$RAIZ" || exit 1
RAMO="$(git rev-parse --abbrev-ref HEAD)"
SHA="$(git rev-parse --short HEAD)"
{
  echo ""
  echo "### Entrega de \`$RAMO\` — $(date -u +%FT%TZ)"
  echo "$MSG"
  [ "$#" -gt 0 ] && echo "- ficheros: $*"
  echo "- rama: \`$RAMO\` · SHA: \`$SHA\`"
  echo "- **NO compilado aqui**: lo compila y testea el root (sandbox de 3 GB)."
} >> docs/COORDINATION.md
echo "entrega registrada en docs/COORDINATION.md"
echo "rama: $RAMO"
echo "sha:  $SHA"
echo "ficheros: $*"
