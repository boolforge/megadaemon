#!/usr/bin/env bash
# =============================================================================
# MegaDaemon v14 — conectar un agente externo a la instancia PRINCIPAL
# =============================================================================
# Uso:
#   source megadaemon-connect.sh          # carga las variables de entorno
#   megadaemon-connect.sh list             # lista las tools disponibles
#   megadaemon-connect.sh call <tool> [json]
#   megadaemon-connect.sh doctor           # comprueba la conexión
#
# Las credenciales NO están en este fichero: se leen del dispositivo (o del
# gestor de secretos) en tiempo de ejecución.  Nada de claves en el repo.
# =============================================================================
set -u

MEGADAEMON_URL="${MEGADAEMON_URL:-https://webpage-ultram-please-communicate.trycloudflare.com}"
MEGADAEMON_MCP="${MEGADAEMON_MCP:-$MEGADAEMON_URL/mcp}"
MEGADAEMON_AGENT_ID="${MEGADAEMON_AGENT_ID:-agent-android-re-01}"
MEGADAEMON_AGENT_ROLE="${MEGADAEMON_AGENT_ROLE:-worker}"

# --- credenciales -----------------------------------------------------------
# 1) Variables de entorno (preferido)
# 2) Ficheros locales del agente
# 3) Copia en el móvil (requiere acceso root al dispositivo)
_load_creds() {
  if [ -z "${MEGADAEMON_API_KEY:-}" ]; then
    for f in "$HOME/.config/megadaemon/api.key" "$HOME/.md_key"; do
      [ -f "$f" ] && MEGADAEMON_API_KEY="$(tr -d '\r\n' < "$f")" && break
    done
  fi
  if [ -z "${MEGADAEMON_TOKEN:-}" ]; then
    for f in "$HOME/.config/megadaemon/cockpit_token" "$HOME/.md_token"; do
      [ -f "$f" ] && MEGADAEMON_TOKEN="$(tr -d '\r\n' < "$f")" && break
    done
  fi
  export MEGADAEMON_API_KEY="${MEGADAEMON_API_KEY:-}"
  export MEGADAEMON_TOKEN="${MEGADAEMON_TOKEN:-}"
}

_mc() {
  # _mc <method> <params-json>
  _load_creds
  python3 - "$1" "$2" <<'PY'
import json, os, sys, urllib.request, urllib.error
method, params = sys.argv[1], sys.argv[2]
url = os.environ.get("MEGADAEMON_MCP", "https://webpage-ultram-please-communicate.trycloudflare.com/mcp")
body = json.dumps({"jsonrpc":"2.0","id":1,"method":method,"params":json.loads(params or "{}")}).encode()
h = {"Content-Type":"application/json"}
k = os.environ.get("MEGADAEMON_API_KEY","")
if k: h["X-API-Key"] = k
t = os.environ.get("MEGADAEMON_TOKEN","")
if t: h["X-Seere-Auth-Token"] = t
req = urllib.request.Request(url, data=body, headers=h)
try:
    r = urllib.request.urlopen(req, timeout=120)
    sys.stdout.write(r.read().decode("utf-8","replace"))
except urllib.error.HTTPError as e:
    sys.stderr.write("HTTP %s: %s\n" % (e.code, e.read().decode("utf-8","replace")[:400]))
    sys.exit(2)
except Exception as e:
    sys.stderr.write("ERROR de red: %s\n" % e)
    sys.exit(3)
PY
}

case "${1:-help}" in
  list)
    _mc tools/list '{}' | python3 -c "
import json,sys
d=json.load(sys.stdin)
for t in d.get('result',{}).get('tools',[]):
    print('  %-22s %s' % (t.get('name',''), (t.get('description') or '')[:90]))
"
    ;;
  call)
    [ $# -lt 2 ] && { echo "uso: megadaemon-connect.sh call <tool> [json-args]"; exit 1; }
    _mc tools/call "{\"name\":\"$2\",\"arguments\":${3:-\{\}}}"
    ;;
  doctor)
    _load_creds
    echo "URL MCP  : $MEGADAEMON_MCP"
    echo "Agente   : $MEGADAEMON_AGENT_ID (rol: $MEGADAEMON_AGENT_ROLE)"
    echo "API key  : ${MEGADAEMON_API_KEY:+presente}${MEGADAEMON_API_KEY:- (FALTA)}"
    echo "Token    : ${MEGADAEMON_TOKEN:+presente}${MEGADAEMON_TOKEN:- (FALTA)}"
    echo "--- ping ---"
    _mc tools/call '{"name":"system_overview","arguments":{}}' | head -c 400
    echo
    ;;
  *)
    grep '^#' "$0" | sed 's/^# \{0,1\}//' | head -20
    ;;
esac
