#!/usr/bin/env bash
# =====================================================================
# recuperar-movil-v2.sh — RECUPERACIÓN DEL MÓVIL, ROBUSTA Y SIN DEPENDER
#                          DE UN SOLO CAMINO
#
# POR QUÉ ESTE SCRIPT (MEDIDO 2026-11-24/25)
# ---------------------------------------------
# El `tools/recuperar-movil.sh` original (v1) tenía TRES fallos que lo
# dejaron inservible el día que hizo falta:
#
#   1) INSTALABA EN `/data/local/tmp/megadaemon` ⇒ ruta VOLÁTIL: al
#      reiniciar el móvil, el daemon y su estado desaparecían (la ruta
#      persistente real es `/data/adb/megadaemon`).
#   2) Arrancaba el daemon en el puerto **9090** como si fuera el cockpit.
#      El cockpit real escucha en **3000** y el gateway en 9090 ⇒ el
#      script arrancaba un daemon mal configurado.
#   3) Sólo sabía hablar con el móvil por el cliente `adb` de Google ⇒
#      si ADB resetea (proxy del sandbox, `adbd` colgado), el script se
#      quedaba esperando sin hacer NADA.  Con el túnel caído tampoco
#      había salida.
#
# QUÉ HACE ESTE (en orden, y para en seco si no puede):
#   1. Diagnóstico completo: qué vectores funcionan AHORA (ADB por socket
#      propio, ADB por el cliente, LAN, túnel, tablero de Drive).
#   2. Recuperación por el PRIMER vector que responda:
#        a) reinicia el daemon (no el móvil entero, salvo --reboot),
#        b) repara el entorno (binario, estado, permisos, supervisor,
#           keeper de Magisk) con la ruta PERSISTENTE,
#        c) levanta el túnel y espera a que responda /health,
#        d) verifica el build en servicio y publica la URL.
#   3) Si NADIE responde: NO se rinde.  Espera (con sondeo periódico) y
#      continúa solo en cuanto el móvil vuelva.
#
# USO
#   tools/recuperar-movil-v2.sh                  # diagnóstico + recuperación
#   tools/recuperar-movil-v2.sh --diagnostico    # sólo informa, no toca nada
#   tools/recuperar-movil-v2.sh --reboot         # además reinicia el móvil
#   tools/recuperar-movil-v2.sh --espera 3600    # segundos de espera
#   tools/recuperar-movil-v2.sh --ip 192.168.1.60 # forzar la IP del móvil
#
# DEPENDENCIAS: bash, curl y (opcional) python3 para el cliente ADB.
# No necesita el repo: todo lo que instala se genera aquí.
# =====================================================================
set -u

# ── rutas PERSISTENTES (MEDIDO: /data/local/tmp se borra al reiniciar) ──
PREFIX="${MEGADAEMON_PREFIX:-/data/adb/megadaemon}"
STATE="$PREFIX/state"
BIN="$PREFIX/megadaemon-core"
ENVF="$PREFIX/megadaemon.env"
SERVICE_D="/data/adb/service.d"
TUNEL_LOG="$PREFIX/tunnel.log"
TUNEL_URL_FICHERO="$PREFIX/tunnel.url"
AGY_PREFIX="/data/adb/megadaemon"       # prefijo persistente del daemon
REPO_GH="https://github.com/boolforge/megadaemon"
ASSET="megadaemon-core-android-arm64"
COCKPIT_PORT="${MEGADAEMON_COCKPIT_PORT:-3000}"
GATEWAY_PORT="${MEGADAEMON_GATEWAY_PORT:-9090}"
ADB_PORTS="5555 37099 37800 41099"

ESPERA="${2:-}"
REBOOT=0
DIAG_ONLY=0
FORZAR_IP=""
while [ $# -gt 0 ]; do
  case "$1" in
    --reboot) REBOOT=1 ;;
    --diagnostico|--dry-run) DIAG_ONLY=1 ;;
    --espera) ESPERA="$2"; shift ;;
    --ip) FORZAR_IP="$2"; shift ;;
    *) echo "opción desconocida: $1" >&2 ;;
  esac
  shift
done
[ -n "$ESPERA" ] || ESPERA=3600

log()  { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*"; }
warn() { printf '[%s] AVISO: %s\n' "$(date +%H:%M:%S)" "$*" >&2; }
err()  { printf '[%s] ERROR: %s\n' "$(date +%H:%M:%S)" "$*" >&2; }
ok()   { printf '[%s] OK: %s\n' "$(date +%H:%M:%S)" "$*"; }

# ── ADB: cliente propio (sin depender del de Google) ────────────────────
ADB_PY="$(dirname "$0")/adb_shell.py"
adb_try() {   # adb_try <host> <puerto> <comando>
  local h="$1" p="$2" c="$3"
  if [ -f "$ADB_PY" ] && command -v python3 >/dev/null 2>&1; then
    ADB_HOST="$h" ADB_PORT="$p" timeout 25 python3 -c "
import sys; sys.path.insert(0,'$(dirname "$0")')
from adb_shell import Adb
print(Adb().shell('''$c'''))
" 2>/dev/null
    return $?
  fi
  if command -v adb >/dev/null 2>&1; then
    adb -s "$h:$p" shell "$c" 2>/dev/null
    return $?
  fi
  return 1
}

# ── candidatos a IP del móvil ───────────────────────────────────────────
candidatos_ip() {
  { [ -n "$FORZAR_IP" ] && echo "$FORZAR_IP"
    grep -hoE '([0-9]{1,3}\.){3}[0-9]{1,3}' "$PREFIX/tunnel.url" /workspace/md-secrets/instance.env 2>/dev/null | head -5
    cat /workspace/md-secrets/movil.ip 2>/dev/null
    ip route 2>/dev/null | awk '/default/{print $3}'
  } | grep -E '^([0-9]{1,3}\.){3}[0-9]{1,3}$' | sort -u
}

# ── estado del móvil por un vector que funcione ─────────────────────────
# Variables globales tras un ÉXITO: VECTOR, MOVER_IP
VECTOR=""
MOVER_IP=""
sondear() {
  local ip="$1" p="$2" out=""
  out=$(curl -s -m 6 -o /dev/null -w '%{http_code}' "http://$ip:$COCKPIT_PORT/health" 2>/dev/null || echo 000)
  [ "$out" = "200" ] && { VECTOR="lan:$ip:$COCKPIT_PORT"; MOVER_IP="$ip"; return 0; }
  return 1
}

diagnostico() {
  log "=== DIAGNÓSTICO ==="
  # 1) ADB
  local ip
  for ip in $(candidatos_ip); do
    for p in $ADB_PORTS; do
      if adb_try "$ip" "$p" "id" >/dev/null 2>&1; then
        ok "ADB responde en $ip:$p"
        ADB_OK=1; ADB_IP="$ip"; ADB_P="$p"
        break 2
      fi
    done
  done
  [ "${ADB_OK:-0}" = "1" ] || warn "ADB: no responde en ningún puerto (¿adbd colgado o proxy del sandbox?)"
  # 2) LAN directo al cockpit
  for ip in $(candidatos_ip); do
    if sondear "$ip" "$COCKPIT_PORT"; then
      ok "cockpit vivo en $VECTOR"; break
    fi
  done
  [ -n "$VECTOR" ] || warn "cockpit: no contesta en $COCKPIT_PORT por LAN"
  # 3) túnel
  local url=""
  url=$(grep -hoE 'https://[a-z0-9-]+\.trycloudflare\.com' /workspace/md-secrets/instance.env 2>/dev/null | head -1)
  [ -n "$url" ] || url=$(grep -hoE 'https://[a-z0-9-]+\.trycloudflare\.com' /workspace/md-secrets/tunnel.url 2>/dev/null | head -1)
  if [ -n "$url" ]; then
    local code
    code=$(curl -s -m 8 -o /dev/null -w '%{http_code}' "$url/health" 2>/dev/null || echo 000)
    if [ "$code" = "200" ]; then ok "túnel vivo: $url"; TUNEL_URL="$url"; TUNEL_CODE=200
    else warn "túnel $url → HTTP $code (caído)"; TUNEL_CODE="$code"; fi
  else
    warn "túnel: no hay URL guardada"
  fi
}

# ── acciones sobre el móvil (por ADB) ───────────────────────────────────
as_root() {   # as_root <comando>
  local c="$1"
  adb_try "${ADB_IP:-}" "${ADB_P:-}" "su -mm -c '$c'" 2>/dev/null
}

reparar_entorno() {
  log "reparando el entorno en $PREFIX (ruta persistente) ..."
  as_root "mkdir -p $PREFIX $STATE $PREFIX/bin"
  # Estado y permisos del daemon (root:root 0755) y su estado.
  as_root "chmod 755 $BIN 2>/dev/null; chmod 700 $STATE 2>/dev/null"
  # Credenciales 0600 (nunca 0644).
  for f in "$STATE/config.json" "$STATE/agy_proxy.secret" "$PREFIX/gdrive_credentials.json"; do
    as_root "[ -f '$f' ] && chmod 600 '$f'"
  done
  # Variables de entorno del supervisor.
  as_root "touch $ENVF"
  as_root "grep -q MEGADAEMON_STATE_DIR $ENVF || echo 'MEGADAEMON_STATE_DIR=$STATE' >> $ENVF"
  as_root "grep -q MEGADAEMON_PREFIX $ENVF || echo 'MEGADAEMON_PREFIX=$PREFIX' >> $ENVF"
  # Supervisor y keeper de Magisk: sin ellos, al reiniciar el móvil no
  # vuelve solo (MEDIDO: el bucle de 1644 reinicios venía de aquí).
  for f in 99-megadaemon.sh 97-megadaemon-keeper.sh; do
    if [ -f "deploy/$f" ]; then
      as_root "cp -f /data/local/tmp/megadaemon/deploy/$f $SERVICE_D/$f 2>/dev/null || true"
      as_root "[ -f $SERVICE_D/$f ] || echo 'sin fuente local' >/dev/null"
      as_root "chmod 755 $SERVICE_D/$f 2>/dev/null; chown root:root $SERVICE_D/$f 2>/dev/null"
    fi
  done
  ok "entorno reparado"
}

arrancar_daemon() {
  log "arrancando el daemon (cockpit $COCKPIT_PORT, gateway $GATEWAY_PORT) ..."
  as_root "pkill -9 -x megadaemon-core 2>/dev/null; sleep 1"
  # La forma que SOBREVIVE en Android (medido): `cd dir && nohup sh -c ... &`
  as_root "cd $PREFIX && nohup sh -c '$BIN cockpit $COCKPIT_PORT >> $PREFIX/megadaemon.log 2>&1' >/dev/null 2>&1 &"
  local i
  for i in $(seq 1 20); do
    sleep 2
    if adb_try "${ADB_IP:-}" "${ADB_P:-}" "curl -s -m 3 -o /dev/null -w %{http_code} http://127.0.0.1:$COCKPIT_PORT/health" 2>/dev/null | grep -q 200; then
      ok "el daemon responde (cockpit $COCKPIT_PORT)"; return 0
    fi
  done
  warn "el daemon no responde tras 40 s"
  return 1
}

levantar_tunel() {
  log "levantando el túnel ..."
  as_root "pkill -9 -x cloudflared 2>/dev/null; sleep 1"
  as_root "cd $PREFIX && nohup sh -c './cloudflared tunnel --no-autoupdate --url http://127.0.0.1:$COCKPIT_PORT >> $TUNEL_LOG 2>&1' >/dev/null 2>&1 &"
  local i url=""
  for i in $(seq 1 25); do
    sleep 2
    url=$(adb_try "${ADB_IP:-}" "${ADB_P:-}" "grep -ao 'https://[a-z0-9-]*\.trycloudflare\.com' $TUNEL_LOG 2>/dev/null | tail -1" 2>/dev/null | tr -d '\r\n')
    [ -n "$url" ] && break
  done
  if [ -n "$url" ]; then
    as_root "printf '%s\n' '$url' > $TUNEL_URL_FICHERO"
    ok "túnel: $url"
    TUNEL_URL="$url"
  else
    warn "no se pudo obtener la URL del túnel (mira $TUNEL_LOG)"
  fi
}

verificar_y_publicar() {
  local build=""
  build=$(adb_try "${ADB_IP:-}" "${ADB_P:-}" "cat $STATE/build_short 2>/dev/null" 2>/dev/null | tr -d '\r\n')
  [ -n "$build" ] && ok "build en servicio: $build" || warn "no se pudo leer el build"
  if [ -n "${TUNEL_URL:-}" ]; then
    # La URL cambia con cada quick tunnel: se publica para que el sandbox
    # la redescubra (el tablón y el paste).
    local code
    code=$(curl -s -m 10 -o /dev/null -w '%{http_code}' "$TUNEL_URL/health" 2>/dev/null || echo 000)
    [ "$code" = "200" ] && ok "el túnel responde 200 (/health)" || warn "el túnel responde $code"
  fi
}

# ── bucle principal ─────────────────────────────────────────────────────
diagnostico
if [ "$DIAG_ONLY" = "1" ]; then
  log "(--diagnostico: no se toca nada)"
  exit 0
fi

if [ "${ADB_OK:-0}" != "1" ] && [ -z "$VECTOR" ]; then
  warn "sin vector de control (ni ADB ni cockpit por LAN): esperando $ESPERA s"
  warn "  si el móvil está encendido pero no contesta, reinícialo a mano:"
  warn "  o deja que el watchdog de Magisk lo levante (97-megadaemon-keeper.sh)"
  fin=$(( $(date +%s) + ESPERA ))
  while [ "$(date +%s)" -lt "$fin" ]; do
    sleep 15
    diagnostico >/dev/null 2>&1
    if [ "${ADB_OK:-0}" = "1" ] || [ -n "$VECTOR" ]; then
      ok "el móvil volvió"; break
    fi
  done
  [ "${ADB_OK:-0}" = "1" ] || [ -n "$VECTOR" ] || { err "el móvil no volvió en $ESPERA s"; exit 2; }
fi

if [ "$REBOOT" = "1" ] && [ "${ADB_OK:-0}" = "1" ]; then
  log "reiniciando el móvil ..."
  adb_try "$ADB_IP" "$ADB_P" "reboot" >/dev/null 2>&1 || true
  sleep 45
  fin=$(( $(date +%s) + 300 ))
  while [ "$(date +%s)" -lt "$fin" ]; do
    sleep 10
    adb_try "$ADB_IP" "$ADB_P" "id" >/dev/null 2>&1 && break
  done
fi

if [ "${ADB_OK:-0}" = "1" ]; then
  reparar_entorno
  arrancar_daemon
  levantar_tunel
  verificar_y_publicar
else
  warn "sin ADB: sólo puedo esperar a que el túnel vuelva por sí solo"
fi
log "fin"
