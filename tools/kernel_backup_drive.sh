#!/data/data/com.termux/files/usr/bin/bash
# Sube a Google Drive las imágenes de partición de arranque YA volcadas en el
# móvil (punto de retorno antes de tocar el kernel).
#
#   su -mm -c 'bash /data/local/tmp/kernel_backup_drive.sh [DIR] [FOLDER_ID] [PREFIX]'
#
# Datos:
#   * el token de OAuth lo MINTA EL PROPIO MÓVIL con su `gdrive_credentials.json`
#     (0600, en el state dir del daemon) ⇒ la credencial nunca pasa por el
#     túnel ni por este repo (que es público);
#   * subida resumable en trozos de 8 MiB (Drive exige múltiplos de 256 KiB);
#   * verificación: MD5 que devuelve Drive + SHA-256 local del fichero;
#   * NO toca ninguna partición: sólo lee ficheros ya volcados.
#
# Salida: un JSON por partición con {file, drive_id, md5_drive, sha256, bytes}.
set -u

PREFIX_BIN=/data/data/com.termux/files/usr/bin
export PATH="$PREFIX_BIN:$PATH"
STATE_DIR=/data/user/0/com.agy/files/megadaemon/state
CRED="$STATE_DIR/gdrive_credentials.json"
SRC_DIR="${1:-/data/local/tmp/kernel-backup}"
FOLDER_ID="${2:-15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0}"
NAME_PREFIX="${3:-kernel-backup}"
CHUNK_MB=8

say() { echo "[$(date -u +%H:%M:%S)] $*"; }

[ -f "$CRED" ] || { say "ABORT: falta $CRED"; exit 1; }
[ -d "$SRC_DIR" ] || { say "ABORT: no existe $SRC_DIR"; exit 1; }

# --- 1. token de acceso (minted en el móvil) -------------------------------
ACCESS_TOKEN=$(python3 - "$CRED" <<'PY'
import json, sys, urllib.parse, urllib.request
c = json.load(open(sys.argv[1]))
data = urllib.parse.urlencode({
    "client_id": c["client_id"],
    "client_secret": c["client_secret"],
    "refresh_token": c["refresh_token"],
    "grant_type": "refresh_token",
}).encode()
req = urllib.request.Request("https://oauth2.googleapis.com/token", data=data, method="POST")
with urllib.request.urlopen(req, timeout=60) as r:
    obj = json.loads(r.read().decode())
print(obj.get("access_token", ""))
PY
)
if [ -z "$ACCESS_TOKEN" ]; then say "ABORT: no se pudo mintear el token"; exit 1; fi
say "token minteado en el móvil (${#ACCESS_TOKEN} chars)"

# --- 2. subir cada .img ------------------------------------------------------
mkdir -p "$SRC_DIR"
for f in "$SRC_DIR"/*.img; do
    [ -f "$f" ] || continue
    base=$(basename "$f")
    size=$(stat -c %s "$f")
    sha=$(sha256sum "$f" | cut -d' ' -f1)
    name="$NAME_PREFIX-$base"

    say "subiendo $base ($size B) -> Drive"
    # 2.1 abrir la sesión resumable
    session=$(curl -sS -D - -o /dev/null -X POST \
        "https://www.googleapis.com/upload/drive/v3/files?uploadType=resumable&supportsAllDrives=true" \
        -H "Authorization: Bearer $ACCESS_TOKEN" \
        -H "Content-Type: application/json; charset=UTF-8" \
        -H "X-Upload-Content-Type: application/octet-stream" \
        -H "X-Upload-Content-Length: $size" \
        --data "{\"name\":\"$name\",\"parents\":[\"$FOLDER_ID\"],\"appProperties\":{\"sha256\":\"$sha\"}}" \
        | tr -d '\r' | awk 'tolower($1)=="location:"{print $2}')
    if [ -z "$session" ]; then say "ABORT: no se pudo abrir la sesion de subida para $base"; exit 1; fi

    # 2.2 trozos
    offset=0
    chunk=$((CHUNK_MB * 1024 * 1024))
    while [ "$offset" -lt "$size" ]; do
        len=$chunk
        [ $((offset + len)) -gt "$size" ] && len=$((size - offset))
        end=$((offset + len - 1))
        # el último trozo puede no ser múltiplo de 256 KiB: Drive lo exige
        # sólo en los trozos intermedios; el final puede llevar la longitud real.
        dd if="$f" bs=1048576 skip=$((offset / 1048576)) count=$(( (len + 1048575) / 1048576 )) 2>/dev/null \
            | head -c "$len" \
            | curl -sS -o /dev/null -w "" -X PUT \
                -H "Authorization: Bearer $ACCESS_TOKEN" \
                -H "Content-Type: application/octet-stream" \
                -H "Content-Range: bytes $offset-$end/$size" \
                --data-binary @- "$session"
        rc=$?
        if [ $rc -ne 0 ]; then say "ABORT: fallo en el trozo $offset de $base (rc=$rc)"; exit 1; fi
        offset=$((offset + len))
        say "  $base: $offset/$size"
    done

    # 2.3 id + md5 de Drive
    meta=$(curl -sS -G "https://www.googleapis.com/drive/v3/files" \
        -H "Authorization: Bearer $ACCESS_TOKEN" \
        --data-urlencode "q=name='$name' and trashed=false" \
        --data-urlencode "fields=files(id,name,size,md5Checksum,mimeType)" \
        --data-urlencode "supportsAllDrives=true")
    echo "$meta" | python3 -c "
import json,sys
m=json.load(sys.stdin)
fs=m.get('files',[])
if fs:
    f=fs[0]
    print(json.dumps({'file':'$base','drive_id':f.get('id'),'name':f.get('name'),
                      'md5_drive':f.get('md5Checksum'),'size_drive':f.get('size'),
                      'sha256_local':'$sha','bytes':$size}))
else:
    print(json.dumps({'file':'$base','error':'no encontrado en Drive','sha256_local':'$sha','bytes':$size}))
"
done
say "fin"
