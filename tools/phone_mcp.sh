#!/bin/sh
# Cliente del móvil (S21) por el túnel: MCP JSON-RPC sobre `/mcp`.
#
#   sh tools/phone_mcp.sh <tool> '<json-args>'
#
# Las credenciales **no** se imprimen ni se versionan: se leen del fichero
# canónico de secretos (`/workspace/md-secrets/instance.env`), que está 0600.
set -u

ENV_FILE="${MEGADAEMON_ENV:-/workspace/md-secrets/instance.env}"
URL="${MEGADAEMON_URL:-https://graphical-subscriber-sys-oil.trycloudflare.com}"
TOOL="${1:-}"
ARGS="${2:-{\}}"

if [ -z "$TOOL" ]; then
    echo "uso: phone_mcp.sh <tool> <json-args>" >&2
    exit 2
fi

# shellcheck disable=SC1090
. "$ENV_FILE"

python3 - "$URL" "$MEGADAEMON_API_KEY" "$MEGADAEMON_COCKPIT_TOKEN" "$TOOL" "$ARGS" <<'PY'
import json, sys, urllib.request

url, key, tok, tool, args = sys.argv[1:6]
body = json.dumps({
    "jsonrpc": "2.0", "id": 1, "method": "tools/call",
    "params": {"name": tool, "arguments": json.loads(args)},
}).encode()
req = urllib.request.Request(url + "/mcp", data=body, method="POST", headers={
    "Content-Type": "application/json",
    "X-API-Key": key,
    "X-Seere-Auth-Token": tok,
})
try:
    with urllib.request.urlopen(req, timeout=1800) as r:
        raw = r.read().decode("utf-8", "replace")
except Exception as e:  # noqa: BLE001
    print(json.dumps({"error": str(e)}))
    sys.exit(1)
try:
    v = json.loads(raw)
except Exception:  # noqa: BLE001
    print(raw)
    sys.exit(0)
# El MCP devuelve el texto de la tool; se imprime tal cual (JSON si lo es).
res = v.get("result", v)
if isinstance(res, dict) and "content" in res:
    for c in res.get("content", []):
        if c.get("type") == "text":
            t = c.get("text", "")
            try:
                print(json.dumps(json.loads(t), ensure_ascii=False, indent=2))
            except Exception:  # noqa: BLE001
                print(t)
        else:
            print(json.dumps(c, ensure_ascii=False))
    if res.get("isError"):
        sys.exit(1)
else:
    print(json.dumps(v, ensure_ascii=False, indent=2))
PY
