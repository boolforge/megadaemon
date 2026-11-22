#!/usr/bin/env bash
# Sube un binario grande al móvil SIN curl ni wget en el móvil.
#
# RESTRICCIONES REALES (medidas 2026-11-22 en el S21 Ultra):
#  * el móvil NO tiene curl ni wget (`toybox wget` -> rc=127);
#  * Linux/Android limita cada ARGUMENTO a 128 KB (MAX_ARG_STRLEN) y el
#    total a 2 MB (ARG_MAX) => ni el shell del sandbox ni el del móvil
#    aceptan un argumento de 700 KB;
#  * `printf %s a b c` concatena sus argumentos => se pueden pasar ~1,4 MB
#    de base64 repartidos en trozos de 90 KB, que es lo que se usa aquí;
#  * el cuerpo HTTP no tiene ese límite => `curl --data-binary @fichero`.
#
# Uso: tools/subir_binario_movil.sh <fichero_local> <ruta_destino_movil> [sha256]
set -euo pipefail

SECRETS=/workspace/md-secrets
URL="$(grep '^MEGADAEMON_URL=' "$SECRETS/instance.env" | cut -d= -f2- | tr -d "\"'")"
KEY="$(cat /workspace/.secrets/s21_api.key)"
LOCAL="$1"
DEST="${2:-/data/local/tmp/megadaemon/megadaemon-core}"
SHA="${3:-}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

SIZE=$(stat -c %s "$LOCAL")
SHA_LOCAL=$(sha256sum "$LOCAL" | awk '{print $1}')
echo "subiendo $LOCAL ($SIZE B) -> $DEST"
echo "sha256 local: $SHA_LOCAL"

TOK="$(curl -s -m 25 -X POST -H 'Content-Type: application/json' \
  -d "{\"api_key\":\"$KEY\"}" "$URL/api/session/exchange" \
  | python3 -c 'import sys,json;print(json.load(sys.stdin).get("auth_token",""))')"
[ -n "$TOK" ] || { echo "ERROR: no obtuve token de sesión" >&2; exit 3; }

# Llama a root_exec con el cuerpo desde un fichero (sin límite de argv).
mcp_root() {
  local jsonfile="$1"
  python3 - "$jsonfile" "$TMP" <<'PY' > "$TMP/rpc.json"
import json, sys
jf, tmp = sys.argv[1], sys.argv[2]
args = json.load(open(jf))
print(json.dumps({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                  "params": {"name": "root_exec", "arguments": args}}))
PY
  curl -s -m 180 -X POST -H "Authorization: Bearer $TOK" -H 'Content-Type: application/json' \
    --data-binary "@$TMP/rpc.json" "$URL/mcp"
}

# Saca el stdout del resultado de root_exec (el plugin MCP lo devuelve
# serializado dentro de result.content[0].text).
mcp_out() {
  printf '%s' "$1" | python3 -c '
import sys, json
try:
    d = json.load(sys.stdin)
except Exception:
    sys.exit(0)
r = d.get("result", {})
out = ""
if isinstance(r, dict):
    cont = r.get("content")
    if isinstance(cont, list) and cont and isinstance(cont[0], dict):
        try:
            inner = json.loads(cont[0].get("text", "{}"))
        except Exception:
            inner = {}
        if isinstance(inner, dict):
            res = inner.get("result")
            if isinstance(res, dict):
                out = res.get("stdout", "")
            if not out:
                out = inner.get("stdout", "")
print(out)
' 2>/dev/null
}

split -b 80K -d -a 3 "$LOCAL" "$TMP/parte_"
TOTAL=$(ls "$TMP"/parte_* | wc -l)
echo "trozos: $TOTAL (80 KiB cada uno; el comando completo debe caber en 128 KB)"

echo '{"command":"mkdir -p /data/local/tmp/megadaemon/.up && rm -f /data/local/tmp/megadaemon/.up/parte_*"}' > "$TMP/args.json"
mcp_root "$TMP/args.json" >/dev/null

i=0
for f in "$TMP"/parte_*; do
  i=$((i + 1))
  python3 - "$f" "$i" "$TMP" <<'PY'
import base64, json, shlex, sys
data = open(sys.argv[1], 'rb').read()
idx = int(sys.argv[2]); tmp = sys.argv[3]
b = base64.b64encode(data).decode()
# El comando entero es UN argumento de `su -mm -c`: maximo 128 KB.
# 80 KiB de binario -> ~107 KB de base64 -> comando de ~108 KB: cabe.
args = shlex.quote(b)
cmd = (f"printf %s {args} | base64 -d > "
       f"/data/local/tmp/megadaemon/.up/parte_{idx:03d}")
json.dump({"command": cmd}, open(f"{tmp}/args.json", "w"))
PY
  resp=$(mcp_root "$TMP/args.json")
  if [ -z "$resp" ]; then
    echo "FALLO: sin respuesta en el trozo $i" >&2
    exit 1
  fi
  printf '\rsubidos %d/%d' "$i" "$TOTAL"
done
echo

echo "ensamblando y verificando..."
python3 - "$DEST" "$TMP" <<'PY'
import json, sys
dest, tmp = sys.argv[1], sys.argv[2]
cmd = (f"cat /data/local/tmp/megadaemon/.up/parte_* > {dest}.new && "
       f"chmod 755 {dest}.new && sha256sum {dest}.new && stat -c %s {dest}.new")
json.dump({"command": cmd}, open(f"{tmp}/args.json", "w"))
PY
out=$(mcp_out "$(mcp_root "$TMP/args.json")")
echo "$out" | head -c 500
echo
if echo "$out" | grep -q "$SHA_LOCAL"; then
  echo "SHA-256 VERIFICADO en el móvil: $SHA_LOCAL"
else
  echo "AVISO: hash remoto no confirmado (ver salida arriba)" >&2
  exit 2
fi
