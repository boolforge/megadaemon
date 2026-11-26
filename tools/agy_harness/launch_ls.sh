#!/usr/bin/env bash
# launch_ls.sh — arranca el Language Server REAL de Antigravity contra un
# servidor local y captura todo lo que envía y recibe.
#
# TRES REQUISITOS MEDIDOS (sin ellos el LS sale con rc=1 y SIN mensaje):
#   1. **PTY**: el LS hace `ioctl(1, TCGETS)` y aborta con EACCES si stdout
#      no es una terminal. Se resuelve con `script -qec ... /dev/null`.
#   2. **bit +x**: el binario extraído del APK viene con 0644; qemu-user
#      responde "Error while loading <prog>: Permission denied".
#   3. **copia local**: el NFS del workspace no es suitable para el mapeo
#      ejecutable del loader invitado; se copia a disco local (overlay).
#
# AISLAMIENTO DE RED (requisito: "tests no detectables por servidores, sin
# carga sobre Google, sin abuso de credenciales"):
#   - Todos los endpoints del LS apuntan a 127.0.0.1.
#   - `HTTP_PROXY`/`HTTPS_PROXY` -> 127.0.0.1:1 (puerto cerrado). El cliente
#     HTTP de Go respeta esas variables y EXCLUYE loopback del proxy, así que:
#     local -> directo; cualquier host externo -> connection refused
#     inmediato. Cero paquetes salen a Internet.
#   - No se usa ninguna credencial de Google.
#
# USO
#   launch_ls.sh [--ls RUTA] [--port 8085] [--extra "flags"] [--timeout 90]
set -uo pipefail

ROOT="/workspace/antigravity"
LS_BIN="${LS_BIN:-/workspace/agy-re/liblanguage_server.so}"
PORT="${PORT:-8085}"
TIMEOUT="${TIMEOUT:-90}"
EXTRA="${EXTRA:-}"
OUTDIR="$ROOT/recon/captures"
LOCAL="${LOCAL:-/tmp/agy-harness}"
STAMP=$(date -u +%Y%m%dT%H%M%SZ)

while [ $# -gt 0 ]; do
  case "$1" in
    --ls) LS_BIN="$2"; shift 2;;
    --port) PORT="$2"; shift 2;;
    --extra) EXTRA="$2"; shift 2;;
    --timeout) TIMEOUT="$2"; shift 2;;
    --token-file) TOKEN_FILE="$2"; export TOKEN_FILE; shift 2;;
    --no-server) NO_SERVER=1; shift;;
    --out) CAP_OVERRIDE="$2"; export CAP_OVERRIDE; shift 2;;
    --log) LOG_OVERRIDE="$2"; export LOG_OVERRIDE; shift 2;;
    --home) HOME_OVERRIDE="$2"; export HOME_OVERRIDE; shift 2;;
    *) echo "opción desconocida: $1" >&2; exit 2;;
  esac
done

mkdir -p "$OUTDIR" "$LOCAL"

# Rutas de salida: se calculan DESPUES de parsear los argumentos para que
# --out / --log manden sobre el valor por defecto.
CAP="${CAP_OVERRIDE:-$OUTDIR/capture_$STAMP.jsonl}"
LOG="${LOG_OVERRIDE:-$OUTDIR/ls_$STAMP.log}"
mkdir -p "$(dirname "$CAP")" "$(dirname "$LOG")"

# --- copia local del binario y del sysroot (requisitos 2 y 3) -------------
if [ ! -x "$LOCAL/language_server" ] || \
   [ "$LS_BIN" -nt "$LOCAL/language_server" ]; then
  echo "== copiando binario a disco local ($LOCAL)"
  cp -f "$LS_BIN" "$LOCAL/language_server" || exit 1
fi
chmod +x "$LOCAL/language_server"
if [ ! -d "$LOCAL/sysroot/lib/aarch64-linux-gnu" ]; then
  echo "== copiando sysroot arm64 a disco local"
  mkdir -p "$LOCAL/sysroot"
  cp -a "$ROOT/runtime/qemu/sysroot/." "$LOCAL/sysroot/" || exit 1
fi

# --- servidor local ------------------------------------------------------
SRV=""
if [ -z "${NO_SERVER:-}" ]; then
  node "$ROOT/tools/agy_harness/fake_cloudcode.mjs" --port "$PORT" --out "$CAP" \
    > "$OUTDIR/fake_$STAMP.log" 2>&1 &
  SRV=$!
  sleep 1
  if ! kill -0 $SRV 2>/dev/null; then
    echo "FALLO: el servidor local no arrancó" >&2
    cat "$OUTDIR/fake_$STAMP.log" >&2
    exit 1
  fi
else
  echo "== servidor local DESACTIVADO (--no-server): se espera otro oyente en $PORT"
  : > "$CAP"
fi

# --- aislamiento de red --------------------------------------------------
export HTTP_PROXY="http://127.0.0.1:1"
export HTTPS_PROXY="http://127.0.0.1:1"
export http_proxy="$HTTP_PROXY"
export https_proxy="$HTTPS_PROXY"
export NO_PROXY="localhost,127.0.0.1,::1"
export no_proxy="$NO_PROXY"
# HOME aislado: el LS escribe estado en ~/.gemini
export HOME="${HOME_OVERRIDE:-$LOCAL/home}"
rm -rf "$HOME/.gemini"
mkdir -p "$HOME/.gemini"

# Token OAuth.
#   MEDIDO con strace: el LS abre exactamente
#       $HOME/.gemini/jetski-standalone-oauth-token
#   y lo deserializa como JSON. Con un token FALSO responde
#   "error getting token source: You are not logged into Antigravity" y
#   NO emite ninguna peticion: el token tiene que ser real (firmado por
#   Google).
#
#   --token-file RUTA (o TOKEN_FILE=...) -> token real JSON, instalado en el
#       HOME aislado. Tiene PRIORIDAD sobre el token de mentira.
#   FAKE_TOKEN=0 -> no sembrar nada.
#
# El token se instala DESPUES de preparar el HOME: si se sembrase antes, el
# `rm -rf` del aislamiento lo borraria.
if [ -n "${TOKEN_FILE:-}" ]; then
  mkdir -p "$HOME/.gemini"
  install -m 600 "$TOKEN_FILE" "$HOME/.gemini/jetski-standalone-oauth-token"
  echo "== token REAL inyectado desde $TOKEN_FILE"
elif [ "${FAKE_TOKEN:-1}" = "1" ]; then
  mkdir -p "$HOME/.gemini"
  cat > "$HOME/.gemini/jetski-standalone-oauth-token" <<'JSON'
{"access_token":"local-harness-fake-token","refresh_token":"local-harness-fake-refresh","id_token":"local-harness-fake-id","token_type":"Bearer","expiry":"2099-01-01T00:00:00Z","scope":"https://www.googleapis.com/auth/cloud-platform","client_id":"local-harness","client_secret":"local-harness"}
JSON
  chmod 600 "$HOME/.gemini/jetski-standalone-oauth-token"
  echo "== AVISO: token FALSO sembrado; el LS no llegara a llamar a la API"
fi

ARGS=(
  --standalone
  --override_ide_name antigravity
  --subclient_type hub
  --override_ide_version 2.4.1
  --override_user_agent_name antigravity
  --api_server_url "http://127.0.0.1:$PORT"
  --cloud_code_endpoint "http://127.0.0.1:$PORT"
  --analytics_server_url "http://127.0.0.1:$PORT"
  --inference_api_server_url "http://127.0.0.1:$PORT"
  --generative_service_addr "blade:127.0.0.1:443"
  --disable_telemetry
  --http_server_port 0
  --https_server_port 0
  --csrf_token local-harness
  --app_data_dir antigravity
  --enable_sidecars
)
[ -n "$EXTRA" ] && ARGS+=($EXTRA)

echo "== flags: ${ARGS[*]}"
# `script` da el PTY que el LS exige; `timeout` corta la ejecución.
timeout "$TIMEOUT" script -qec \
  "qemu-aarch64-static -L '$LOCAL/sysroot' '$LOCAL/language_server' ${ARGS[*]}" /dev/null \
  > "$LOG" 2>&1
RC=$?
kill $SRV 2>/dev/null

echo "== rc=$RC  log=$LOG"
echo "== peticiones capturadas:"
python3 - "$CAP" <<'PY'
import json,sys
p=sys.argv[1]; n=0
try:
    for line in open(p,encoding="utf-8"):
        line=line.strip()
        if not line: continue
        r=json.loads(line)
        if r.get("kind")=="request":
            n+=1
            print(f"  [{r['seq']}] {r['method']} {r['path']} len={r['body_len']} "
                  f"campos={len(r.get('proto_fields') or [])}")
except FileNotFoundError:
    pass
print(f"  total peticiones: {n}")
PY
echo "== primeras lineas del log del LS:"
head -12 "$LOG" 2>/dev/null
exit 0
