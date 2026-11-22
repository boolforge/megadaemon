#!/usr/bin/env bash
# Helper mínimo para llamar al MCP del móvil (JSON-RPC) sin depender de
# `tools/md_mcp.sh` (que falló tras la actualización: no consegue token).
#
# Uso:
#   mdmcp.sh root_exec '{"command":"ls /data"}'
#   mdmcp.sh list
set -euo pipefail
URL="$(grep '^MEGADAEMON_URL=' /workspace/md-secrets/instance.env | cut -d= -f2- | tr -d "\"'")"
KEY="$(cat /workspace/.secrets/s21_api.key)"

tok() {
  curl -s -m 25 -X POST -H 'Content-Type: application/json' \
    -d "{\"api_key\":\"$KEY\"}" "$URL/api/session/exchange" \
    | python3 -c 'import sys,json;print(json.load(sys.stdin).get("auth_token",""))'
}

# MEDIDO 2026-11-22: tras la actualizacion a v13.4.0 el
# /api/session/exchange ya no devuelve auth_token, pero el MCP acepta
# directamente la API key como Bearer.  Se intenta el exchange y si falla
# se usa la API key.
TOK="$(tok)"
if [ -z "$TOK" ]; then
  TOK="$KEY"
fi

case "${1:-}" in
  list)
    curl -s -m 30 -X POST -H "Authorization: Bearer $TOK" -H 'Content-Type: application/json' \
      -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' "$URL/mcp" \
      | python3 -c 'import sys,json;d=json.load(sys.stdin);t=d["result"]["tools"];[print(" ",x["name"],"-",(x.get("description") or "")[:80]) for x in t];print("TOTAL:",len(t))'
    ;;
  *)
    python3 - "$1" "$2" <<'PY' > /tmp/mdmcp_rpc.json
import json, sys
tool = sys.argv[1]
args = json.loads(sys.argv[2]) if len(sys.argv) > 2 and sys.argv[2] else {}
args.setdefault("confirm_destructive", True)
print(json.dumps({"jsonrpc":"2.0","id":1,"method":"tools/call",
                  "params":{"name":tool,"arguments":args}}))
PY
    curl -s -m 180 -X POST -H "Authorization: Bearer $TOK" -H 'Content-Type: application/json' \
      --data-binary @/tmp/mdmcp_rpc.json "$URL/mcp" \
      | python3 -c '
import sys, json
d = json.load(sys.stdin)
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
            elif inner.get("error"): out = "ERR: " + json.dumps(inner["error"], ensure_ascii=False)
            elif not out: out = json.dumps(inner, ensure_ascii=False)
    sc = r.get("structuredContent")
    if not out and isinstance(sc, dict):
        out = json.dumps(sc, ensure_ascii=False)
print(out)
'
    ;;
esac
