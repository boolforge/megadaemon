#!/system/bin/sh
# ---------------------------------------------------------------------------
# KIT DE RECUPERACIÓN (Samsung / primary) — OOM killer se llevó daemon y túneles
#
# Ejecutar como root en el móvil:
#     su -mm -c 'sh /data/local/tmp/megadaemon/recover_channels.sh'
#
# Qué hace, en orden:
#   1. relanza el supervisor (/data/adb/service.d/99-megadaemon.sh) — el que
#      debe sobrevivir al OOM y resucitar daemon + túnel;
#   2. espera a que el daemon abra 9090;
#   3. si no hay túnel (porque el supervisor también murió), arranca cloudflared
#      a mano contra 9090 y contra el MCP legacy 8765;
#   4. captura la URL pública NUEVA en /data/local/tmp/megadaemon/tunnel.url
#      y la imprime (es EFÍMERA: cambia en cada relanzamiento);
#   5. imprime el estado (RAM, procesos, puertos) para que quien lo ejecute
#      pueda reportar.
#
# No compila nada, no reinicia Chrome y no toca el repo: sólo levantar canales.
# ---------------------------------------------------------------------------
LOG=/data/local/tmp/megadaemon
CF=/data/data/com.termux/files/usr/bin/cloudflared
SUP=/data/adb/service.d/99-megadaemon.sh

mkdir -p "$LOG" 2>/dev/null

echo "== 1) supervisor =="
if [ -f "$SUP" ]; then
  setsid sh "$SUP" >/dev/null 2>&1 &
  echo "   lanzado: $SUP"
else
  echo "   NO EXISTE $SUP  -> se arranca cloudflared a mano en el paso 3"
fi

# 2) esperar al daemon en 9090 (9090 decimal = 0x2382)
echo "== 2) daemon en 9090 =="
i=0
while [ $i -lt 20 ]; do
  if grep -q ':2382 ' /proc/net/tcp 2>/dev/null || grep -q ':2382 ' /proc/net/tcp6 2>/dev/null; then
    echo "   daemon escuchando en 9090 ✓"
    break
  fi
  sleep 1
  i=$((i + 1))
done

# 3) túnel: primero el del supervisor, y si no hay URL, a mano
echo "== 3) túnel =="
U=""
if [ -f "$LOG/tunnel.log" ]; then
  U=$(grep -o 'https://[a-z0-9-]*\.trycloudflare\.com' "$LOG/tunnel.log" 2>/dev/null | head -1)
fi
if [ -z "$U" ] && [ -x "$CF" ]; then
  echo "   el supervisor no levantó túnel (o murió): arrancando cloudflared a mano"
  setsid "$CF" tunnel --no-autoupdate --url http://127.0.0.1:9090 >"$LOG/tunnel.log" 2>&1 &
  i=0
  while [ $i -lt 40 ]; do
    U=$(grep -o 'https://[a-z0-9-]*\.trycloudflare\.com' "$LOG/tunnel.log" 2>/dev/null | head -1)
    [ -n "$U" ] && break
    sleep 1
    i=$((i + 1))
  done
fi
if [ -n "$U" ]; then
  echo "$U" >"$LOG/tunnel.url"
  echo "   TUNEL NUEVO = $U"
  echo "   (EFÍMERA: mándala a la flota; los que la tienen hardcodeada caerán a 530)"
else
  echo "   SIN URL: revisa $LOG/tunnel.log"
fi

# 3b) MCP legacy 8765 (es OTRO cloudflared, aparte)
if [ -x "$CF" ]; then
  if ! pgrep -f "cloudflared.*8765" >/dev/null 2>&1; then
    setsid "$CF" tunnel --no-autoupdate --url http://127.0.0.1:8765 >"$LOG/tunnel8765.log" 2>&1 &
    sleep 6
    U2=$(grep -o 'https://[a-z0-9-]*\.trycloudflare\.com' "$LOG/tunnel8765.log" 2>/dev/null | head -1)
    [ -n "$U2" ] && echo "$U2" >"$LOG/tunnel8765.url" && echo "   TUNEL LEGACY 8765 = $U2"
  fi
fi

# 4) estado
echo "== 4) estado =="
free -m 2>/dev/null | head -3
echo "-- procesos megadaemon/cloudflared --"
ps -A -o PID,ARGS 2>/dev/null | grep -E 'megadaemon|cloudflared|99-megadaemon' | grep -v grep | head -8
echo "-- puertos --"
ss -ltn 2>/dev/null | grep -E ':(80|3000|3030|8080|8090|8765|9090)' | head -12
echo "== FIN =="
