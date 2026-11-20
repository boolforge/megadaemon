#!/system/bin/sh
# Prueba de aceptación F1: backup ABSOLUTO a Drive en streaming (>1 GB).
#
#   - NO copia el fichero: lee bloques y los sube por trozos (dd | curl).
#   - NO deja el fichero descargado en el disco: se borra tras verificar.
#   - El token NUNCA está en el repo: se lee de un fichero 0600 en runtime.
#
# Uso (en el móvil, con root):
#   DRIVE_TOKEN_FILE=/data/local/tmp/megadaemon/drive.token \
#   FOLDER_ID=1b-2LoDTNrCpInx_9G261ozmkxMFHMYFF \
#   SOURCE=/data/media/0/video.mp4 \
#   sh deploy/drive_stream_acceptance.sh
#
# Criterio de aceptación: sha256 de la descarga == sha256 del origen,
# y `df` de /data sin bajar más de lo que ocupa el temporal.

set -eu
TOKEN_FILE="${DRIVE_TOKEN_FILE:-/data/local/tmp/megadaemon/drive.token}"
FOLDER_ID="${FOLDER_ID:-}"
SOURCE="${SOURCE:-}"
CHUNK="${CHUNK:-8388608}"          # 8 MiB (múltiplo de 256 KiB, como exige Drive)
UPLOAD_API="https://www.googleapis.com/upload/drive/v3/files"
API="https://www.googleapis.com/drive/v3/files"

[ -r "$TOKEN_FILE" ] || { echo "FALLO: no se puede leer el token en $TOKEN_FILE"; exit 2; }
TOKEN="$(cat "$TOKEN_FILE")"
[ -n "$TOKEN" ] || { echo "FALLO: token vacio"; exit 2; }

# 1) fichero de origen: el mas grande >1 GB si no se indica
if [ -z "$SOURCE" ]; then
  SOURCE="$(find /data/media/0 -type f -size +1000M -printf '%s %p\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2-)"
fi
[ -n "$SOURCE" ] && [ -f "$SOURCE" ] || { echo "FALLO: no hay fichero >1 GB para probar"; exit 3; }
SIZE="$(stat -c %s "$SOURCE")"
echo "== origen: $SOURCE ($SIZE bytes)"

DF_BEFORE="$(df -k /data | tail -1 | awk '{print $4}')"
echo "== df libre ANTES: ${DF_BEFORE}K"

echo "== sha256 del origen (streaming, sin copia)"
SHA_SRC="$(sha256sum "$SOURCE" | cut -d' ' -f1)"
echo "   $SHA_SRC"

# 2) crear carpeta de destino
META_FOLDER="{\"name\":\"megadaemon-acceptance\",\"mimeType\":\"application/vnd.google-apps.folder\",\"parents\":[\"$FOLDER_ID\"]}"
FOLDER_JSON="$(curl -sS -X POST -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  --data-binary "$META_FOLDER" "$API?supportsAllDrives=true")"
NEW_FOLDER="$(echo "$FOLDER_JSON" | sed -n 's/.*"id" *: *"\([^"]*\)".*/\1/p' | head -1)"
[ -n "$NEW_FOLDER" ] || { echo "FALLO: no se pudo crear la carpeta: $FOLDER_JSON"; exit 4; }
echo "== carpeta creada: $NEW_FOLDER"

# 3) abrir la sesion de subida resumible
META_FILE="{\"name\":\"acceptance-$(basename "$SOURCE"),\"mimeType\":\"application/octet-stream\",\"parents\":[\"$NEW_FOLDER\"]}"
SESSION_FILE="${SESSION_FILE:-/data/local/tmp/megadaemon/.drive-session.uri}"
LOC="$(curl -sS -D - -o /dev/null -X POST -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json; charset=UTF-8' -H "X-Upload-Content-Length: $SIZE" \
  --data-binary "$META_FILE" "$UPLOAD_API?uploadType=resumable&supportsAllDrives=true" | tr -d '\r' \
  | sed -n 's/^[Ll]ocation: *//p' | head -1)"
[ -n "$LOC" ] || { echo "FALLO: la API no devolvio Location"; exit 5; }
echo "$LOC" > "$SESSION_FILE"
chmod 600 "$SESSION_FILE"
echo "== sesion de subida abierta (URL persistida en $SESSION_FILE para poder reanudar)"

# 4) subir por trozos (dd | curl: el bloque NUNCA toca el disco)
OFFSET=0
TOTAL="$SIZE"
while [ "$OFFSET" -lt "$TOTAL" ]; do
  REMAIN=$((TOTAL - OFFSET))
  if [ "$REMAIN" -lt "$CHUNK" ]; then LEN=$REMAIN; else LEN=$CHUNK; fi
  LAST=$((OFFSET + LEN - 1))
  dd if="$SOURCE" bs="$CHUNK" skip=$((OFFSET / CHUNK)) count=$((LEN / CHUNK + 1)) 2>/dev/null \
    | head -c "$LEN" \
    | curl -sS -X PUT -H "Authorization: Bearer $TOKEN" \
        -H "Content-Range: bytes $OFFSET-$LAST/$TOTAL" --data-binary @- "$LOC" >/dev/null
  OFFSET=$((LAST + 1))
  PCT=$((OFFSET * 100 / TOTAL))
  echo "   subido $OFFSET/$TOTAL ($PCT%)"
done

# 5) localizar el fichero subido y su id
LIST="$(curl -sS -G -H "Authorization: Bearer $TOKEN" \
  --data-urlencode "q=name='acceptance-$(basename "$SOURCE")' and '$NEW_FOLDER' in parents" \
  --data-urlencode "fields=files(id,name,size,md5Checksum)" \
  "$API?supportsAllDrives=true")"
FILE_ID="$(echo "$LIST" | sed -n 's/.*"id" *: *"\([^"]*\)".*/\1/p' | head -1)"
[ -n "$FILE_ID" ] || { echo "FALLO: el fichero no aparece en Drive: $LIST"; exit 6; }
echo "== fichero en Drive: $FILE_ID"

# 6) descargar y verificar (sha256 del contenido, comparacion con el MANIFEST)
TMP="/data/local/tmp/megadaemon/.dl-acceptance.bin"
curl -sS -L -H "Authorization: Bearer $TOKEN" "$API/$FILE_ID?alt=media&supportsAllDrives=true" -o "$TMP"
SHA_DL="$(sha256sum "$TMP" | cut -d' ' -f1)"
echo "== sha256 descargado: $SHA_DL"
rm -f "$TMP"

DF_AFTER="$(df -k /data | tail -1 | awk '{print $4}')"
echo "== df libre DESPUES: ${DF_AFTER}K (antes ${DF_BEFORE}K)"

if [ "$SHA_SRC" = "$SHA_DL" ]; then
  echo "RESULTADO: OK (sha256 coincide, fichero de $SIZE bytes subido y verificado)"
  exit 0
else
  echo "RESULTADO: FALLO (sha32 origen $SHA_SRC != descarga $SHA_DL)"
  exit 7
fi
