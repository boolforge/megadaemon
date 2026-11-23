#!/system/bin/sh
# ---------------------------------------------------------------------------
# KIT DE RECUPERACIÓN (Samsung / primary) — OOM killer se llevó daemon y túneles
#
# Ejecutar como root en el móvil:
#     su -mm -c 'sh /data/local/tmp/megadaemon/recover_channels.sh'
#
# REGLAS (pedidas por el bus de coordinación, para no pisar a otro agente):
#   * NO reinicia el daemon si /health ya responde 200 → en ese caso sólo
#     relanza cloudflared. Si el daemon está muerto, THEN sí relanza el
#     supervisor completo.
#   * NO toca ni borra board.sqlite (5 tasks + 1 instancia + 7 eventos).
#   * La URL del túnel se EXTRAE DE tunnel.log (cloudflared la imprime ahí),
#     nunca se inventa ni se reescribe a mano: se escribe en bucle cada 15 s
#     hasta que aparece, y tunnel.url se vacía al principio para que nadie
#     use una URL muerta.
#
# Qué hace:
#   1. ¿daemon vivo? (9090 escuchando) → decide si relanzar supervisor o no;
#   2. relanza cloudflared (9090) y el legacy (8765) si faltan;
#   3. escribe tunnel.url / tunnel8765.url desde los logs, en bucle;
#   4. imprime estado (RAM, procesos, puertos) y la URL nueva.
# ---------------------------------------------------------------------------
LOG=/data/local/tmp/megadaemon
CF=/data/data/com.termux/files/usr/bin/cloudflared
SUP=/data/adb/service.d/99-megadaemon.sh
URL_RE='https://[a-z0-9-]*\.trycloudflare\.com'

mkdir -p "$LOG" 2>/dev/null

listening() {
  # 9090 = 0x2382 ; 8765 = 0x2235  (lee /proc, sin forks)
  grep -q ':2382 ' /proc/net/tcp 2>/dev/null || grep -q ':2382 ' /proc/net/tcp6 2>/dev/null
}

# --- 1) ¿daemon vivo? -------------------------------------------------------
echo "== 1) daemon =="
if listening; then
  DAEMON_UP=1
  echo "   9090 escuchando → NO se toca el daemon (otro agente podría estar desplegando)"
else
  DAEMON_UP=0
  echo "   9090 mudo → hay que relanzar el supervisor"
fi

# --- 2) supervisor -----------------------------------------------------------
if [ "$DAEMON_UP" = "0" ] && [ -f "$SUP" ]; then
  echo "== 2) supervisor =="
  setsid sh "$SUP" >/dev/null 2>&1 &
  echo "   lanzado: $SUP"
  i=0
  while [ $i -lt 30 ]; do
    listening && { echo "   daemon en 9090 ✓"; break; }
    sleep 1
    i=$((i + 1))
  done
fi

# --- 3) túneles -------------------------------------------------------------
echo "== 3) túneles =="
U=$(grep -o "$URL_RE" "$LOG/tunnel.log" 2>/dev/null | head -1)
if [ -z "$U" ] && [ -x "$CF" ]; then
  echo "   sin URL en tunnel.log → arrancando cloudflared (9090)"
  : > "$LOG/tunnel.url"
  setsid "$CF" tunnel --no-autoupdate --url http://127.0.0.1:9090 >"$LOG/tunnel.log" 2>&1 &
fi
# MEDIDO (2026-11-23): el MCP ya lo sirve el daemon en el MISMO puerto que el
# cockpit (9090).  El 8765 no tiene NADIE escuchando => ese túnel no puede
# funcionar y solo consume RAM (y calor) en el móvil.  Por defecto NO se
# relanza; se puede forzar con MD_MCP_LEGACY_TUNNEL=1.
if [ "${MD_MCP_LEGACY_TUNNEL:-0}" = "1" ]; then
  if ! pgrep -f "cloudflared.*8765" >/dev/null 2>&1 && [ -x "$CF" ]; then
    echo "   MCP legacy 8765 → relanzando (es OTRO cloudflared)"
    : > "$LOG/tunnel8765.url"
    setsid "$CF" tunnel --no-autoupdate --url http://127.0.0.1:8765 >"$LOG/tunnel8765.log" 2>&1 &
  fi
else
  pids=$(pgrep -f "cloudflared.*8765" 2>/dev/null)
  for p in $pids; do kill "$p" 2>/dev/null; done
  [ -n "$pids" ] && echo "   MCP legacy 8765 desactivado (nadie lo sirve); instances terminadas"
fi

# --- 4) tunnel.url desde el log, en bucle cada 15 s -------------------------
echo "== 4) tunnel.url =="
i=0
while [ $i -lt 8 ]; do
  U=$(grep -o "$URL_RE" "$LOG/tunnel.log" 2>/dev/null | head -1)
  if [ -n "$U" ]; then
    printf '%s\n' "$U" > "$LOG/tunnel.url"
    echo "   tunnel.url = $U"
    break
  fi
  sleep 15
  i=$((i + 1))
done
[ -n "$U" ] || echo "   aún sin URL: revisa $LOG/tunnel.log"

if [ "${MD_MCP_LEGACY_TUNNEL:-0}" = "1" ]; then
  U2=$(grep -o "$URL_RE" "$LOG/tunnel8765.log" 2>/dev/null | head -1)
  if [ -n "$U2" ]; then
    printf '%s\n' "$U2" > "$LOG/tunnel8765.url"
    echo "   tunnel8765.url = $U2"
  fi
fi

# --- 5) estado ---------------------------------------------------------------
echo "== 5) estado =="
free -m 2>/dev/null | head -3
echo "-- procesos --"
ps -A -o PID,ARGS 2>/dev/null | grep -E 'megadaemon|cloudflared' | grep -v grep | head -8
echo "-- puertos --"
ss -ltn 2>/dev/null | grep -E ':(80|3000|3030|8080|8090|8765|9090)' | head -12
echo "== FIN =="
