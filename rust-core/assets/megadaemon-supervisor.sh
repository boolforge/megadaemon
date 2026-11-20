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
