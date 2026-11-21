#!/usr/bin/env bash
# Extrae de Swift Backup, en el móvil, todo lo necesario para el RE:
#   1. el APK (para descompilar);
#   2. las piezas de la copia de seguridad;
#   3. la base de datos de la app (índices y preferencias);
#   4. el token OAuth de Google Drive (si está en el almacenamiento).
#
# Uso:
#   tools/swift_extract.sh apk        # solo el APK
#   tools/swift_extract.sh backups    # solo las piezas de la copia
#   tools/swift_extract.sh db         # solo la base de datos
#   tools/swift_extract.sh drive      # solo el token de Drive
#   tools/swift_extract.sh all
#
# El destino es ./swift-dump/ (en el directorio de trabajo).
# Las credenciales se guardan en ficheros 0600 y NUNCA se suben al repo.

set -uo pipefail

T="${MD_TUNNEL:-}"
PKG="org.swiftapps.swiftbackup"
DEST="${SWIFT_DUMP_DIR:-$(pwd)/swift-dump}"
MODE="${1:-all}"

mkdir -p "$DEST"
chmod 700 "$DEST"

# ── cliente MCP del móvil ──────────────────────────────────────────────────
KEY_FILE="${MD_KEY_FILE:-/workspace/md-secrets/master_api.key}"
[ -r "$KEY_FILE" ] || { echo "no hay clave API en $KEY_FILE"; exit 2; }
KEY=$(cat "$KEY_FILE")

if [ -z "$T" ]; then
    # Se descubre por el beacon LAN.
    T=$(python3 "$(dirname "$0")/lan_discover.py" --json 2>/dev/null \
        | python3 -c "import sys,json;print(json.load(sys.stdin).get('url',''))" 2>/dev/null)
fi
[ -n "$T" ] || { echo "no se pudo localizar el móvil ( beacon LAN )"; exit 2; }
echo "móvil: $T"

TOK=$(curl -sS -m 25 -X POST -H "Content-Type: application/json" \
      -d "{\"api_key\":\"$KEY\"}" "$T/api/session/exchange" \
      | python3 -c "import sys,json;print(json.load(sys.stdin).get('token',''))" 2>/dev/null)
[ -n "$TOK" ] || { echo "no se pudo canjear la sesión"; exit 2; }

call() { # call <tool> <json-args>
    curl -sS -m "${MD_TIMEOUT:-600}" \
        -H "X-API-Key: $KEY" -H "X-Seere-Auth-Token: $TOK" \
        -H "Content-Type: application/json" \
        -X POST "$T/mcp" \
        -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"$1\",\"arguments\":$2}}"
}

root_out() { # root_out <comando>  -> stdout del móvil
    call root_exec "{\"command\":$(python3 -c 'import json,sys;print(json.dumps(sys.argv[1]))' "$1"),\"max_chars\":524288}" \
      | python3 -c "
import sys,json
try:
    d=json.load(sys.stdin); print(d.get('result',{}).get('stdout',''))
except Exception as e:
    print('ERROR:', e)
"
}

pull() { # pull <ruta-movil> <destino-local>
    local remoto="$1" local="$2"
    local tmp="$local.b64"
    mkdir -p "$(dirname "$local")"
    call fs_read "$(python3 -c 'import json,sys;print(json.dumps({"path":sys.argv[1],"base64":True,"max_bytes":8388608}))' "$remoto")" \
      > "$tmp.json"
    python3 - "$tmp.json" "$local" <<'PY'
import sys, json, base64, os
raw = open(sys.argv[1], encoding='utf-8', errors='replace').read()
try:
    d = json.loads(raw)
    b64 = d['result']['content'][0]['text']
except Exception:
    b64 = raw
try:
    data = base64.b64decode(''.join(b64.split()))
except Exception as e:
    print('base64 inválido:', e); sys.exit(1)
open(sys.argv[2], 'wb').write(data)
os.chmod(sys.argv[2], 0o600)
print(f"{sys.argv[2]}: {len(data)} bytes")
PY
    rm -f "$tmp.json"
}

case "$MODE" in
apk|all)
    echo "── APK ──"
    APK_PATH=$(root_out "pm path $PKG | head -1 | sed 's/package://'" | tr -d '\r')
    echo "  ruta: $APK_PATH"
    if [ -n "$APK_PATH" ]; then
        pull "$APK_PATH" "$DEST/swiftbackup.apk"
        # El DEX principal, por si se quiere sin el APK entero.
        root_out "unzip -l '$APK_PATH' 2>/dev/null | head -5" >/dev/null
    fi
    ;;
backups|all)
    echo "── piezas de la copia ──"
    BASE=$(root_out "ls -d /sdcard/SwiftBackup/accounts/*/backups 2>/dev/null | head -1" | tr -d '\r' | head -1)
    echo "  base: $BASE"
    if [ -n "$BASE" ]; then
        root_out "find '$BASE' -type f -printf '%s %p\n' 2>/dev/null | sort -rn | head -60" \
            > "$DEST/backup-files.txt"
        cat "$DEST/backup-files.txt"
        # Los ficheros de texto (v1::: y .wfi) caben enteros; los 7z y los
        # PNG se dejan listados para decidir (son grandes).
        root_out "find '$BASE' -type f \( -name '*.xml' -o -name '*.extra' -o -name '*.wfi' \) -printf '%p\n' 2>/dev/null" \
            | tr -d '\r' | head -40 | while read -r f; do
                [ -z "$f" ] && continue
                nome=$(basename "$f")
                pull "$f" "$DEST/backups/$nome"
            done
    fi
    ;;
db|all)
    echo "── base de datos de la app ──"
    for f in databases/swiftbackup-db databases/swiftbackup-db-wal shared_prefs/org.swiftapps.swiftbackup_preferences.xml; do
        pull "/data/data/$PKG/$f" "$DEST/app/$(basename "$f")"
    done
    ;;
drive|all)
    echo "── token de Google Drive ──"
    # Se busca el patron de token OAuth en el almacenamiento de la app.
    TOKFILE="$DEST/drive_token.txt"
    root_out "grep -rhaoE 'ya29\.[A-Za-z0-9_.-]{40,}' /data/data/$PKG 2>/dev/null | head -1" \
        | tr -d '\r' | head -1 > "$TOKFILE"
    if [ -s "$TOKFILE" ] && [ -n "$(cat "$TOKFILE")" ]; then
        chmod 600 "$TOKFILE"
        echo "  token guardado en $TOKFILE (600)"
    else
        rm -f "$TOKFILE"
        echo "  NO VERIFICADO: no se ha encontrado un token 'ya29.*' en el almacenamiento de la app"
        echo "  (puede estar cifrado con el Keystore: en ese caso hay que descargarlo desde la app)"
    fi
    ;;
*)
    echo "uso: $0 [apk|backups|db|drive|all]"; exit 2 ;;
esac

echo "destino: $DEST"
