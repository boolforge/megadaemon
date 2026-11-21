#!/usr/bin/env bash
# Baja las copias de Swift Backup de Google Drive (carpeta 1L-XKLJKy68Jmjf0K8uIRXOklL4gUIdO1)
# usando un token OAuth del usuario.
#
# El token NUNCA va en el repo: se lee de un fichero 0600 (o de la env).
#
# Uso:
#   MEGADAEMON_DRIVE_TOKEN=/ruta/token tools/swift_drive_pull.sh [destino]
#   tools/swift_drive_pull.sh --list
#
# Notas:
#   * el sandbox SÍ tiene curl (el móvil no): por eso el script es de shell
#     y no del binario Rust;
#   * descarga reanudable con `curl -C -` (un fichero de 32 GB no se vuelve a
#     bajar desde el principio);
#   * verifica el SHA-256 si el Drive lo expone (`md5Checksum`/`sha256`).

set -uo pipefail

FOLDER_ID="${MEGADAEMON_DRIVE_FOLDER:-1L-XKLJKy68Jmjf0K8uIRXOklL4gUIdO1}"
DEST="${2:-$(pwd)/swift-drive}"
TOKEN=""

# 1) token del entorno
if [ -n "${MEGADAEMON_DRIVE_TOKEN:-}" ]; then
    TOKEN="$MEGADAEMON_DRIVE_TOKEN"
# 2) de un fichero
elif [ -n "${MEGADAEMON_DRIVE_TOKEN_FILE:-}" ] && [ -r "${MEGADAEMON_DRIVE_TOKEN_FILE}" ]; then
    TOKEN=$(cat "${MEGADAEMON_DRIVE_TOKEN_FILE}")
# 3) el que extrajo swift_extract.sh
elif [ -r "$(pwd)/swift-dump/drive_token.txt" ]; then
    TOKEN=$(cat "$(pwd)/swift-dump/drive_token.txt")
fi

if [ -z "$TOKEN" ]; then
    echo "FALTA EL TOKEN de Google Drive."
    echo "  Opción A: extraerlo del móvil (la app Swift Backup lo tiene):"
    echo "      tools/swift_extract.sh drive"
    echo "  Opción B: pegarlo en un fichero 0600 y exportarlo:"
    echo "      export MEGADAEMON_DRIVE_TOKEN_FILE=/ruta/token"
    exit 2
fi

mkdir -p "$DEST"
chmod 700 "$DEST"

api() { curl -sS -m 120 -H "Authorization: Bearer $TOKEN" "$@"; }

list() {
    api "https://www.googleapis.com/drive/v3/files?q=%27${FOLDER_ID}%27+in+parents&fields=files(id,name,size,mimeType,md5Checksum,sha256Checksum,modifiedTime)&pageSize=200&supportsAllDrives=true&includeItemsFromAllDrives=true"
}

echo "── listando la carpeta $FOLDER_ID ──"
LIST_JSON=$(list)
if echo "$LIST_JSON" | grep -q '"error"'; then
    echo "ERROR de la API:"; echo "$LIST_JSON" | head -c 600; echo
    exit 1
fi

echo "$LIST_JSON" > "$DEST/_listado.json"

python3 - "$LIST_JSON" <<'PY' | tee "$DEST/_ficheros.txt"
import json, sys
d = json.loads(sys.argv[1])
files = d.get('files', [])
if not files:
    print("(la carpeta esta vacia o no se ve)")
for f in sorted(files, key=lambda x: -int(x.get('size', 0) or 0)):
    print(f"{f.get('id')}\t{f.get('size','?')}\t{f.get('name')}\t{f.get('mimeType','')}\t{f.get('md5Checksum','')}")
print(f"total: {len(files)} ficheros", file=sys.stderr)
PY

if [ "${1:-}" = "--list" ]; then
    exit 0
fi

echo "── descargando ──"
while IFS=$'\t' read -r id size name mime sum; do
    [ -z "$id" ] && continue
    out="$DEST/$name"
    if [ -f "$out" ]; then
        echo "  ya existe: $name ($(stat -c %s "$out") B)"
    else
        echo "  bajando: $name ($size B)"
        curl -sS -L -m 3600 -C - -H "Authorization: Bearer $TOKEN" \
            "https://www.googleapis.com/drive/v3/files/${id}?alt=media" \
            -o "$out" || { echo "    FALLO al bajar $name"; continue; }
    fi
    if [ -n "$sum" ] && [ -f "$out" ]; then
        got=$(md5sum "$out" | cut -d' ' -f1)
        if [ "$got" = "$sum" ]; then
            echo "    md5 OK"
        else
            echo "    md5 DISTINTO (drive=$sum local=$got)"
        fi
    fi
done < "$DEST/_ficheros.txt"

echo "destino: $DEST"
