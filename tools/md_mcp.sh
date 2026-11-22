#!/usr/bin/env bash
# md_mcp.sh — Cliente MCP del MÓVIL para la flota (acceso root medido).
#
# El móvil publica un túnel Cloudflare; por él el daemon responde al MCP.
# MEDIDO el 2026-11-21 23:52 (Samsung, instalador-megadaemon.sh):
#   URL viva : https://graphical-subscriber-sys-oil.trycloudflare.com
#   /api/status -> 200 y el canje de token con la key de la flota da ok:true
#   POST /mcp tools/list -> 26 herramientas (root_exec, fs_read, fs_write…)
#   root_exec -> uid=0(root) context=u:r:magisk:s0
#
# USO:
#   tools/md_mcp.sh <herramienta> '<json de argumentos>'
#   tools/md_mcp.sh list                      # catálogo de herramientas
#   tools/md_mcp.sh raw <ruta>                # GET a una ruta del daemon
#
# La URL se lee de md-secrets/instance.env (MEGADAEMON_URL) y la key de
# md-secrets/master_api.key.  Si la URL caduca (los hostnames quick-tunnel
# cambian en cada reinicio), se lee la nueva de <state_dir>/tunnel.url vía
# el propio MCP o del móvil.
set -uo pipefail
SECRETS="${MD_SECRETS_DIR:-/workspace/md-secrets}"
URL="$(grep '^MEGADAEMON_URL=' "$SECRETS/instance.env" 2>/dev/null | cut -d= -f2- | tr -d '"'\''\r\n')"
KEY="$(head -1 "$SECRETS/master_api.key" 2>/dev/null | tr -d '\r\n')"
[ -n "$KEY" ] || KEY="$(head -1 /workspace/.secrets/s21_api.key 2>/dev/null | tr -d '\r\n')"
[ -n "$URL" ] || { echo "ERROR: no hay MEGADAEMON_URL en $SECRETS/instance.env" >&2; exit 2; }
[ -n "$KEY" ] || { echo "ERROR: no hay key en $SECRETS/master_api.key" >&2; exit 2; }

# --- AUTO-RECUPERACION: si la URL guardada no responde, se busca la viva
# MEDIDO el 2026-11-21: el hostname del tunel rapido CAMBIA en cada
# reinicio del movil => la URL guardada se queda vieja y la flota cree
# que el movil esta caido.  La fuente de verdad es state/channel.json en
# el repo (lo publica el propio movil); se prueban sus URLs en orden.
recuperar_url() {
  [ "${MD_MCP_NO_REFRESH:-0}" = "1" ] && return 1
  CH="$(curl -fsSL -m 20 "https://raw.githubusercontent.com/boolforge/megadaemon/main/state/channel.json" 2>/dev/null)" || return 1
  CAND="$(printf '%s' "$CH" | python3 -c '
import sys,json
try:
    d=json.load(sys.stdin)
except Exception:
    raise SystemExit
for k in ("quick","duckdns","public","url"):
    v=d.get(k)
    if isinstance(v,str) and v.startswith("http"):
        print(v.rstrip("/")); break
' 2>/dev/null)"
  for C in $CAND; do
    [ "$C" = "$URL" ] && continue
    if curl -s -m 10 -o /dev/null "$C/api/status" 2>/dev/null; then
      printf 'URL recuperada de state/channel.json: %s\n' "$C" >&2
      URL="$C"
      if [ -f "$SECRETS/instance.env" ]; then
        sed -i "s|^MEGADAEMON_URL=.*|MEGADAEMON_URL=$C|" "$SECRETS/instance.env" 2>/dev/null || true
      fi
      return 0
    fi
  done
  return 1
}

TOK="$(curl -s -m 25 -X POST -H 'Content-Type: application/json' \
  -d "{\"api_key\":\"$KEY\"}" "$URL/api/session/exchange" 2>/dev/null \
  | python3 -c 'import sys,json;d=json.load(sys.stdin);print(d.get("auth_token") or d.get("token") or "")' 2>/dev/null)"
if [ -z "$TOK" ]; then
  if ! recuperar_url; then
    # Diagnostico real (2026-11-22): el movil puede estar VIVO y responder
    # 403 porque la key local no coincide con la vigente.  /health es publico
    # y publica el fingerprint (fnv1a64) de la key que espera el movil.
    ESPERADO="$(curl -s -m 12 "$URL/health" 2>/dev/null | python3 -c 'import sys,json;print(json.load(sys.stdin).get("key_fp",""))' 2>/dev/null)"
    echo "ERROR: el movil no respondio con la credencial local." >&2
    if [ -n "$ESPERADO" ]; then
      echo "  el movil esta VIVO y espera la key con fingerprint '$ESPERADO'." >&2
      echo "  la key local no coincide => resincronizala (ver tools/sincronizar-key.sh)." >&2
    else
      echo "  (sin /health legible: el movil no es accesible por el tunel)" >&2
    fi
    exit 3
  fi
  TOK="$(curl -s -m 25 -X POST -H 'Content-Type: application/json' \
    -d "{\"api_key\":\"$KEY\"}" "$URL/api/session/exchange" 2>/dev/null \
    | python3 -c 'import sys,json;d=json.load(sys.stdin);print(d.get("auth_token") or d.get("token") or "")' 2>/dev/null)"
fi
[ -n "$TOK" ] || { echo "ERROR: el movil no acepto la credencial (403?)" >&2; exit 3; }

case "${1:-}" in
  list)
    curl -s -m 30 -X POST -H "Authorization: Bearer $TOK" -H 'Content-Type: application/json' \
      -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' "$URL/mcp" \
      | python3 -c 'import sys,json;d=json.load(sys.stdin);[print(" ",t["name"],"-",(t.get("description") or "")[:90]) for t in d["result"]["tools"]]'
    ;;
  raw)
    curl -s -m 30 -H "Authorization: Bearer $TOK" "$URL$2"
    ;;
  "")
    echo "uso: md_mcp.sh <herramienta> '<args json>' | list | raw <ruta>" >&2; exit 2 ;;
  *)
    ARGS="${2:-\{\}}"
    curl -s -m 180 -X POST -H "Authorization: Bearer $TOK" -H 'Content-Type: application/json' \
      -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"$1\",\"arguments\":$ARGS}}" \
      "$URL/mcp" \
      | python3 -c '
import sys,json
d=json.load(sys.stdin)
r=d.get("result",{})
inner=None
if isinstance(r,dict):
    if isinstance(r.get("structuredContent"),dict):
        inner=r["structuredContent"].get("result") or r["structuredContent"]
    elif "result" in r and isinstance(r["result"],dict):
        inner=r["result"]
if isinstance(inner,dict):
    print(inner.get("stdout",""),end="")
    if inner.get("stderr"): print("STDERR:",inner["stderr"][:800],file=sys.stderr)
    print("[rc=%s %sms]"%(inner.get("exit_code"),inner.get("duration_ms")),file=sys.stderr)
else:
    for c in (r.get("content") or []):
        t=c.get("text","")
        try:
            j=json.loads(t); rr=j.get("result",j)
            if isinstance(rr,dict) and "stdout" in rr:
                print(rr.get("stdout",""),end="")
                print("[rc=%s]"%rr.get("exit_code"),file=sys.stderr); continue
        except Exception: pass
        print(t)
'
    ;;
esac
