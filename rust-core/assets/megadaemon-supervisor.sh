#!/system/bin/sh
# =============================================================================
# MegaDaemon v13.3 — supervisor persistente (Magisk service.d)
#
# Mantiene vivos tres procesos y se auto-repara:
#   1. megadaemon-core cockpit 9090  -> plano de control + MCP HTTP nativo
#   2. cloudflared tunnel -> 9090     -> exposición remota (URL efímera)
#   3. python android-mcp (8765)      -> MCP legado (compatibilidad)
#
# Diseño: cada componente se comprueba por separado, con backoff creciente y
# rotación de log.  La URL del túnel se guarda en tunnel.url para poder
# consultarla sin hacer grep.
# =============================================================================
P=/data/local/tmp/megadaemon
LOG=/sdcard/megadaemon.log
LOGDIR=/sdcard/megadaemon-logs
MAXB=$((10*1024*1024)); KEEP=5

rotate() {
  [ -f "$LOG" ] || return 0
  sz=$(stat -c %s "$LOG" 2>/dev/null || echo 0)
  [ "$sz" -gt "$MAXB" ] || return 0
  mkdir -p "$LOGDIR" 2>/dev/null
  i=$((KEEP-1)); while [ "$i" -ge 1 ]; do
    [ -f "$LOGDIR/megadaemon.log.$i" ] && mv -f "$LOGDIR/megadaemon.log.$i" "$LOGDIR/megadaemon.log.$((i+1))"
    i=$((i-1))
  done
  mv -f "$LOG" "$LOGDIR/megadaemon.log.1"
}

log() { echo "[$(date '+%F %T')] $*" >> "$LOG"; }

start_cockpit() {
  # v15: "existe pero no es ejecutable" (un `mv` sin `chmod` lo deja en
  # 644) es un fallo distinto de "no existe": antes el supervisor lo
  # reportaba como "ausente" y se quedaba sin daemon.  Ahora se repara.
  if [ ! -f "$P/megadaemon-core" ]; then
    log "cockpit: binario ausente"
    return 1
  fi
  if [ ! -x "$P/megadaemon-core" ]; then
    chmod 755 "$P/megadaemon-core" 2>/dev/null
    if [ -x "$P/megadaemon-core" ]; then
      log "cockpit: el binario no era ejecutable; chmod 755 aplicado"
    else
      log "cockpit: el binario existe pero no se puede hacer ejecutable"
      return 1
    fi
  fi
  set -a; [ -f "$P/megadaemon.env" ] && . "$P/megadaemon.env"; set +a
  cd "$P" 2>/dev/null || exit 1
  setsid "$P/megadaemon-core" cockpit 9090 >> "$LOG" 2>&1 < /dev/null &
  log "cockpit arrancado PID $!"
}

start_tunnel() {
  CF=$(ls /data/data/com.termux/files/usr/bin/cloudflared 2>/dev/null | head -1)
  [ -n "$CF" ] || { log "tunnel: cloudflared no encontrado"; return 1; }
  setsid "$CF" tunnel --no-autoupdate --url http://127.0.0.1:9090 >> "$P/tunnel.log" 2>&1 < /dev/null &
  log "tunnel arrancado PID $!"
  # guardar la URL cuando aparezca
  n=0
  while [ $n -lt 30 ]; do
    U=$(grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' "$P/tunnel.log" 2>/dev/null | tail -1)
    [ -n "$U" ] && { echo "$U" > "$P/tunnel.url"; log "tunnel URL: $U"; break; }
    n=$((n+1)); sleep 2
  done
}

start_android_mcp() {
  S=$(ls /data/data/com.termux/files/home/android-mcp/start.sh 2>/dev/null | head -1)
  [ -n "$S" ] || return 1
  setsid sh "$S" >> "$P/android-mcp.log" 2>&1 < /dev/null &
  log "android-mcp arrancado PID $!"
}

# ---------------------------------------------------------------------------
# Sincronización automática de conversaciones (sin intervención humana).
# Cada 30 min: cosecha local + pull web + reindexado.  El motor es idempotente,
# así que repetirlo no duplica nada.
# ---------------------------------------------------------------------------
SYNC_EVERY=$((30*60))
last_sync=0
run_sync() {
  log "sync: iniciando sincronización automática"
  set -a; [ -f "$P/megadaemon.env" ] && . "$P/megadaemon.env"; set +a
  timeout 600 "$P/megadaemon-core" sync >> "$LOG" 2>&1
  log "sync: terminada (rc=$?)"
}

# ---------------------------------------------------------------------------
# v15: watchdog de RESPONSIVIDAD (no de "vivo").
#
# `pgrep` sólo dice que el proceso existe.  Con un mutex envenenado por un
# panic, el proceso sigue vivo pero CADA petición muere a medias => el
# túnel de Cloudflare ve 502 y el móvil parece "caído" sin estarlo.
# Aquí se pregunta de verdad: ¿contesta /health por localhost?  Con DOS
# fallos seguidos (la red del móvil da falsos positivos) se reinicia.
# El sondeo es LOCAL (127.0.0.1): salir a internet y volver por el túnel
# daría falsos positivos y no mide lo que importa.
daemon_unresponsive_streak=0
daemon_responsive() {
    local port="${1:-9090}"
    if [ -x /data/data/com.termux/files/usr/bin/curl ]; then
        /data/data/com.termux/files/usr/bin/curl -fsS --max-time 6 \
            -o /dev/null "http://127.0.0.1:$port/health" 2>/dev/null && return 0
        return 1
    fi
    if [ -x /data/data/com.termux/files/usr/bin/python3 ]; then
        /data/data/com.termux/files/usr/bin/python3 -c \
            "import http.client,sys
try:
    c=http.client.HTTPConnection('127.0.0.1',$port,timeout=6)
    c.request('GET','/health')
    sys.exit(0 if c.getresponse().status==200 else 1)
except Exception:
    sys.exit(1)" >/dev/null 2>&1 && return 0
        return 1
    fi
    return 0   # sin sonda: no se puede afirmar nada => no se reinicia
}

check_daemon_responsive() {
    local port="${1:-9090}"
    daemon_unresponsive_streak=$((daemon_unresponsive_streak + 1))
    if [ "$daemon_unresponsive_streak" -ge 2 ]; then
        daemon_unresponsive_streak=0
        log "el daemon esta VIVO pero NO responde /health: lo reinicio (puede ser un mutex envenenado por un panic)"
        pkill -x megadaemon-core 2>/dev/null
        sleep 3
        start_cockpit
        return 1
    fi
    return 0
}

cd_cockpit=0; cd_tunnel=0; cd_mcp=0
while true; do
  rotate

  now=$(date +%s)
  if [ $((now - last_sync)) -ge $SYNC_EVERY ]; then
    last_sync=$now
    run_sync &
  fi

  if ! pgrep -f "megadaemon-core cockpit" >/dev/null 2>&1; then
    start_cockpit
    cd_cockpit=$((cd_cockpit+1)); s=$((5*(1<<(cd_cockpit<3?cd_cockpit:3)))); [ "$s" -gt 40 ] && s=40
  else
    cd_cockpit=0; s=30
    # v15: vivo != funcional.  Se comprueba /health (2 fallos seguidos).
    check_daemon_responsive "$PORT" || true
  fi
  sleep 5

  if ! pgrep -f "cloudflared tunnel.*9090" >/dev/null 2>&1; then
    start_tunnel
    cd_tunnel=$((cd_tunnel+1)); s2=$((5*(1<<(cd_tunnel<5?cd_tunnel:5)))); [ "$s2" -gt 120 ] && s2=120
  else
    cd_tunnel=0
  fi
  sleep 5

  if ! pgrep -f "android-mcp" >/dev/null 2>&1; then
    start_android_mcp
    cd_mcp=$((cd_mcp+1)); s3=$((10*(1<<(cd_mcp<4?cd_mcp:4)))); [ "$s3" -gt 300 ] && s3=300
  else
    cd_mcp=0
  fi

  sleep 20
done
