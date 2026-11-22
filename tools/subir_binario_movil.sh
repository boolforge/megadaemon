#!/usr/bin/env bash
# Sube un binario grande al móvil SIN curl ni wget en el móvil.
#
# El móvil no tiene curl ni wget (toybox wget -> rc=127), y su `fs_write`
# pasa el contenido en el argv del shell (límite ARG_MAX), así que un binario
# de 12 MB no cabe en una sola llamada.  La estrategia es:
#   1. trocear el binario (512 KB por trozo);
#   2. subir cada trozo por `POST /api/tool/fs_write` en base64;
#   3. concatenar en el móvil, verificar SHA-256 y atomicamente sustituir.
#
# Uso: tools/subir_binario_movil.sh <fichero_local> <ruta_destino_movil> [sha256]
set -euo pipefail

SECRETS=/workspace/md-secrets
URL="$(grep '^MEGADAEMON_URL=' "$SECRETS/instance.env" | cut -d= -f2- | tr -d "\"'")"
KEY="$(cat /workspace/.secrets/s21_api.key)"
LOCAL="$1"
DEST="${2:-/data/local/tmp/megadaemon/megadaemon-core}"
SHA="${3:-}"
TMPDIR_LOCAL="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_LOCAL"' EXIT

SIZE=$(stat -c %s "$LOCAL")
echo "subiendo $LOCAL ($SIZE B) -> $DEST"
echo "sha256 local: $(sha256sum "$LOCAL" | awk '{print $1}')"

STAGE="$TMPDIR_LOCAL/parte_"
i=0
split -b 512K -d -a 3 "$LOCAL" "$STAGE"
TOTAL=$(ls "$TMPDIR_LOCAL"/parte_* | wc -l)
echo "trozos: $TOTAL"

for f in "$TMPDIR_LOCAL"/parte_*; do
  i=$((i + 1))
  b64="$TMPDIR_LOCAL/$(basename "$f").b64"
  base64 -w 0 "$f" > "$b64"
  payload=$(cat "$b64")
  ruta="/data/local/tmp/megadaemon/.up/$(basename "$f")"
  resp=$(curl -s -m 120 -X POST -H "Authorization: Bearer $KEY" -H 'Content-Type: application/json' \
    --data "$(python3 -c '
import json,sys
print(json.dumps({"path":sys.argv[1],"content":sys.argv[2],"base64":True,"chmod":"0644"}))
' "$ruta" "$payload")" "$URL/api/tool/fs_write")
  if ! echo "$resp" | grep -q '"status":"executed"'; then
    echo "FALLO en el trozo $i: $resp" >&2
    exit 1
  fi
  printf '\rsubidos %d/%d' "$i" "$TOTAL"
done
echo

echo "ensamblando en el móvil..."
MD_EXEC() {
  curl -s -m 120 -X POST -H "Authorization: Bearer $KEY" -H 'Content-Type: application/json' \
    --data "$(python3 -c 'import json,sys; print(json.dumps({"command":sys.argv[1]}))' "$1")" \
    "$URL/api/tool/root_exec"
}
MD_EXEC "cat /data/local/tmp/megadaemon/.up/parte_* > ${DEST}.new && chmod 755 ${DEST}.new && sha256sum ${DEST}.new" | head -c 400
echo
if [ -n "$SHA" ]; then
  echo "sha256 esperado: $SHA"
fi
echo "OK: ensamblado en ${DEST}.new (falta verificar y reiniciar)"
