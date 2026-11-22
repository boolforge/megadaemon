#!/usr/bin/env bash
# Cliente del MCP del móvil (JSON-RPC 2.0) con la auth de v13.4.0.
#
# MEDIDO 2026-11-22:
#  * /api/session/exchange devuelve {"ok":true,"token":...} (antes
#    "auth_token") con la API key en el cuerpo;
#  * las rutas protegidas exigen DOS cabeceras: X-Seere-Auth-Token (el
#    token de sesión) y X-API-Key (la API key de la instancia).  Con una
#    sola el daemon responde {"error":"unauthorized"}.
#
# Uso:
#   mdmcp.sh list
#   mdmcp.sh <tool> '<args json>'
set -uo pipefail
URL="$(grep '^MEGADAEMON_URL=' /workspace/md-secrets/instance.env | cut -d= -f2- | tr -d "\"'")"
KEY="$(cat /workspace/.secrets/s21_api.key)"

TOK="$(curl -s -m 25 -X POST -H 'Content-Type: application/json' \
  -d "{\"api_key\":\"$KEY\"}" "$URL/api/session/exchange" 2>/dev/null \
  | python3 -c 'import sys,json
try:
    d = json.load(sys.stdin)
    print(d.get("token") or d.get("auth_token") or "")
except Exception:
    print("")' 2>/dev/null)"
[ -n "${TOK:-}" ] || TOK="$KEY"

case "${1:-}" in
  list)
    curl -s -m 30 -X POST -H "Authorization: Bearer $TOK" -H "X-Seere-Auth-Token: $TOK" \
      -H "X-API-Key: $KEY" -H 'Content-Type: application/json' \
      -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' "$URL/mcp" \
      | python3 -c 'import sys,json
try:
    d = json.load(sys.stdin)
    t = d["result"]["tools"]
    for x in t: print(" ", x["name"], "-", (x.get("description") or "")[:80])
    print("TOTAL:", len(t))
except Exception as e:
    print("no se pudo listar:", e)'
    ;;
  *)
    python3 - "$1" "${2:-{\}}" <<'PY' > /tmp/mdmcp_rpc.json
import json, sys
tool = sys.argv[1]
try:
    args = json.loads(sys.argv[2])
except Exception:
    args = {}
if not isinstance(args, dict):
    args = {}
args.setdefault("confirm_destructive", True)
print(json.dumps({"jsonrpc":"2.0","id":1,"method":"tools/call",
                  "params":{"name":tool,"arguments":args}}))
PY
    curl -s -m 180 -X POST -H "Authorization: Bearer $TOK" -H "X-Seere-Auth-Token: $TOK" \
      -H "X-API-Key: $KEY" -H 'Content-Type: application/json' \
      --data-binary @/tmp/mdmcp_rpc.json "$URL/mcp" \
      | python3 -c '
import sys, json
try:
    d = json.load(sys.stdin)
except Exception as e:
    print("(respuesta no JSON del movil)"); sys.exit(0)
r = d.get("result", {})
out = ""
if isinstance(r, dict):
    cont = r.get("content")
    if isinstance(cont, list) and cont and isinstance(cont[0], dict):
        try: inner = json.loads(cont[0].get("text", "{}"))
        except Exception: inner = {}
        if isinstance(inner, dict):
            res = inner.get("result")
            if isinstance(res, dict):
                out = res.get("stdout", "")
                if res.get("stderr"): out += "\nSTDERR: " + res["stderr"]
                if not out: out = json.dumps(res, ensure_ascii=False)
            elif inner.get("error"):
                out = "ERR: " + json.dumps(inner["error"], ensure_ascii=False)
            elif not out:
                out = json.dumps(inner, ensure_ascii=False)
    sc = r.get("structuredContent")
    if not out and isinstance(sc, dict):
        out = json.dumps(sc, ensure_ascii=False)
if not out:
    out = json.dumps(d, ensure_ascii=False)[:800]
print(out)
'
    ;;
esac
