#!/usr/bin/env bash
# Completa la subida de un binario al móvil: compara DENTRO del dispositivo
# el tamaño de cada trozo con el esperado y reenvía sólo los que faltan.
#
# POR QUÉ ESTA FORMA (medido 2026-11-22):
#  * el móvil no tiene curl ni wget;
#  * el comando entero es UN argumento de `su -mm -c` y Android limita cada
#    ARGUMENTO a 128 KB (MAX_ARG_STRLEN) => con 80 KiB de binario el
#    comando ronda los 108 KB y cabe;
#  * la SALIDA del root_exec viene truncada (~4 KB) => listar los 147 trozos
#    desde fuera no cabe.  Por eso la comparación se hace en el móvil: se le
#    pasan los tamaños esperados y devuelve sólo los que no cuadran.
#
# Uso: tools/completar_subida_movil.sh <fichero_local> <ruta_destino_movil> [sha256]
set -euo pipefail

SECRETS=/workspace/md-secrets
URL="$(grep '^MEGADAEMON_URL=' "$SECRETS/instance.env" | cut -d= -f2- | tr -d "\"'")"
KEY="$(cat /workspace/.secrets/s21_api.key)"
LOCAL="$1"
DEST="${2:-/data/local/tmp/megadaemon/megadaemon-core}"
SHA="${3:-}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

SHA_LOCAL=$(sha256sum "$LOCAL" | awk '{print $1}')
TOK="$(curl -s -m 25 -X POST -H 'Content-Type: application/json' \
  -d "{\"api_key\":\"$KEY\"}" "$URL/api/session/exchange" \
  | python3 -c 'import sys,json;print(json.load(sys.stdin).get("auth_token",""))')"
[ -n "$TOK" ] || { echo "ERROR: sin token de sesion" >&2; exit 3; }

mcp_root() {
  python3 - "$1" "$TMP" > "$TMP/rpc.json" <<'JSONEOF'
import json, sys
jf, tmp = sys.argv[1], sys.argv[2]
print(json.dumps({"jsonrpc":"2.0","id":1,"method":"tools/call",
                  "params":{"name":"root_exec","arguments":json.load(open(jf))}}))
JSONEOF
  curl -s -m 180 -X POST -H "Authorization: Bearer $TOK" -H 'Content-Type: application/json' \
    --data-binary "@$TMP/rpc.json" "$URL/mcp"
}

mcp_out() {
  printf '%s' "$1" | python3 -c '
import sys, json
try: d = json.load(sys.stdin)
except Exception: sys.exit(0)
r = d.get("result", {}); out = ""
if isinstance(r, dict):
    cont = r.get("content")
    if isinstance(cont, list) and cont and isinstance(cont[0], dict):
        try: inner = json.loads(cont[0].get("text", "{}"))
        except Exception: inner = {}
        if isinstance(inner, dict):
            res = inner.get("result")
            if isinstance(res, dict): out = res.get("stdout", "")
            if not out: out = inner.get("stdout", "")
print(out)
' 2>/dev/null
}

split -b 80K -d -a 3 "$LOCAL" "$TMP/parte_"
TOTAL=$(ls "$TMP"/parte_* | wc -l)
echo "trozos locales: $TOTAL"

# --- 1. Comparar dentro del movil -----------------------------------------
python3 - "$TMP" "$TOTAL" > "$TMP/args.json" <<'JSONEOF'
import json, os, sys
tmp, total = sys.argv[1], int(sys.argv[2])
locales = sorted(f for f in os.listdir(tmp) if f.startswith("parte_"))
tam = " ".join(str(os.path.getsize(os.path.join(tmp, f))) for f in locales)
cmd = ("cd /data/local/tmp/megadaemon/.up && set -- " + tam + " && "
       "for f in $(ls parte_* 2>/dev/null | sort); do "
       "s=$(stat -c %s \"$f\"); e=$1; shift; "
       "[ \"$s\" = \"$e\" ] || echo \"MAL $f $s $e\"; done; "
       "echo TOTALREMOTO $(ls parte_* 2>/dev/null | wc -l)")
json.dump({"command": cmd}, open("/dev/stdout", "w"))
JSONEOF
remote=$(mcp_out "$(mcp_root "$TMP/args.json")")
malos=$(echo "$remote" | grep '^MAL ' | awk '{print $2}')
nremoto=$(echo "$remote" | grep '^TOTALREMOTO ' | awk '{print $2}')
echo "trozos en el movil: ${nremoto:-0} / $TOTAL"
echo "trozos con tamano incorrecto: $(echo "$malos" | grep -c . || true)"

# --- 2. Reenviar los malos y los que faltan -------------------------------
python3 - "$TMP" "$TOTAL" > "$TMP/faltan.txt" <<'JSONEOF'
import os, sys
tmp, total = sys.argv[1], int(sys.argv[2])
locales = sorted(f for f in os.listdir(tmp) if f.startswith("parte_"))
for f in locales:
    print(f)
JSONEOF
# se marcan como "a reenviar" los que el movil dijo MAL
if [ -n "$malos" ]; then
  python3 - "$TMP/faltan.txt" "$malos" <<'JSONEOF'
import sys
fich, malos = sys.argv[1], set(sys.argv[2].split())
lineas = [l for l in open(fich).read().splitlines() if l and l not in malos]
open(fich, "w").write("\n".join(lineas))
JSONEOF
fi
# los que faltan por completo: se reenvian si el movil tiene menos de TOTAL
if [ "${nremoto:-0}" -lt "$TOTAL" ]; then
  echo "  (el movil tiene menos trozos de los esperados: se reintentan los no verificados)"
  python3 - "$TMP/faltan.txt" <<'JSONEOF'
import sys
fich = sys.argv[1]
lineas = [l for l in open(fich).read().splitlines() if l]
open(fich, "w").write("\n".join(lineas))
JSONEOF
fi
N=$(grep -c . "$TMP/faltan.txt" || true)
echo "a reenviar: $N"
if [ "$N" -gt 0 ]; then
  while read -r f; do
    [ -z "$f" ] && continue
    idx="${f#parte_}"
    python3 - "$TMP/$f" "$idx" "$TMP" > "$TMP/args.json" <<'JSONEOF'
import base64, json, shlex, sys
data = open(sys.argv[1], 'rb').read()
idx, tmp = sys.argv[2], sys.argv[3]
b = base64.b64encode(data).decode()
cmd = (f"printf %s {shlex.quote(b)} | base64 -d > "
       f"/data/local/tmp/megadaemon/.up/parte_{idx} && "
       f"stat -c %s /data/local/tmp/megadaemon/.up/parte_{idx}")
json.dump({"command": cmd}, open("/dev/stdout", "w"))
JSONEOF
    out=$(mcp_out "$(mcp_root "$TMP/args.json")")
    esperado=$(stat -c %s "$TMP/$f")
    got=$(echo "$out" | tr -d ' \n')
    if [ "$got" = "$esperado" ]; then
      echo "  ok $f ($esperado B)"
    else
      echo "  FALLO $f (esperado $esperado, obtenido '$got')" >&2
    fi
  done < "$TMP/faltan.txt"
fi

# --- 3. Ensamblar y verificar ---------------------------------------------
python3 - "$DEST" "$TMP" > "$TMP/args.json" <<'JSONEOF'
import json, sys
dest, tmp = sys.argv[1], sys.argv[2]
locales = sorted(f for f in os.listdir(tmp) if f.startswith("parte_"))
# Lista EXPLICITA: con un glob entran tambien ficheros de subidas
# anteriores y el ensamblado sale mas largo que el original.
cmd = ("cd /data/local/tmp/megadaemon/.up && cat " + " ".join(locales) +
       f" > {dest}.new && chmod 755 {dest}.new && sha256sum {dest}.new && "
       f"stat -c %s {dest}.new")
json.dump({"command": cmd}, open("/dev/stdout", "w"))
JSONEOF
out=$(mcp_out "$(mcp_root "$TMP/args.json")")
echo "$out" | head -c 400
echo
if echo "$out" | grep -q "$SHA_LOCAL"; then
  echo "SHA-256 VERIFICADO en el móvil: $SHA_LOCAL"
else
  echo "AVISO: hash remoto NO confirmado" >&2
  exit 2
fi
