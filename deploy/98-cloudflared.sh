#!/system/bin/sh
# ---------------------------------------------------------------------------
# CLOUDFLARED ÚNICO e INDEPENDIENTE del daemon  (Magisk service.d)
#
# Por qué este fichero existe (medido el 2026-11-21):
#
#   * El *quick tunnel* **regenera el hostname en cada reinicio** ⇒ si
#     `cloudflared` es hijo del daemon, **reiniciar el daemon tumba el túnel
#     Y cambia la URL** ⇒ el sandbox se queda ciego al móvil.
#   * Había **DOS `cloudflared` peleando** (pids 1934 y 7358) ⇒ el que se
#     ganaba el edge era aleatorio ⇒ la URL cambiaba sola y nadie sabía cuál
#     era la buena.
#
# Qué hace este script (arranque del móvil, FUERA del daemon):
#   1. mata TODOS los `cloudflared` y deja **exactamente uno**, lanzado con
#      `setsid nohup` ⇒ **no es hijo** del daemon ni del supervisor ⇒
#      reiniciar el daemon **no** tumba el túnel;
#   2. lee la URL del log y la escribe en `state/tunnel.url`;
#   3. lanza `tunnel-watchdog.sh` (el que sondea y relanza cuando toca).
#
# Instalación:  /data/adb/service.d/98-cloudflared.sh   (Magisk lo ejecuta)
# Variables:    CFT_TARGET (por defecto http://127.0.0.1:9090)
#               MAX_LOAD  (por defecto 8: con el móvil saturado NO relanza)
# ---------------------------------------------------------------------------
LOG_DIR=/data/local/tmp/megadaemon
CF="${CF:-/data/data/com.termux/files/usr/bin/cloudflared}"
TARGET="${CFT_TARGET:-http://127.0.0.1:9090}"
URL_FILE="$LOG_DIR/tunnel.url"
TUNNEL_LOG="$LOG_DIR/tunnel.log"
WATCHDOG="$LOG_DIR/tunnel-watchdog.sh"
MAX_LOAD="${MAX_LOAD:-8}"

mkdir -p "$LOG_DIR" 2>/dev/null

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] 98-cloudflared: $*" >> "$LOG_DIR/cloudflared.log"
}

# --- pids de cloudflared (sin depender de pidof) --------------------------
cf_pids() {
    ps -A -o PID,NAME 2>/dev/null | grep -i cloudflared | grep -v grep | awk '{print $1}'
}

# --- ¿el móvil está saturado? ---------------------------------------------
load1() {
    if [ -r /proc/loadavg ]; then
        set -- $(cat /proc/loadavg)
        echo "${1%%.*}"
    else
        echo 0
    fi
}

# --- deja EXACTAMENTE UN cloudflared ---------------------------------------
reiniciar_cloudflared() {
    L=$(load1)
    if [ "$L" -ge "$MAX_LOAD" ] 2>/dev/null; then
        log "movil SATURADO (load1=$L >= $MAX_LOAD): no relanzo cloudflared"
        return 1
    fi
    for p in $(cf_pids); do
        log "matando cloudflared pid $p (dejamos solo uno)"
        kill -9 "$p" 2>/dev/null
    done
    sleep 1
    # setsid + nohup => proceso independiente del daemon y del supervisor.
    setsid nohup "$CF" tunnel --no-autoupdate --url "$TARGET" >> "$TUNNEL_LOG" 2>&1 &
    log "lanzado cloudflared -> $TARGET (load1=$L)"
    return 0
}

# --- la URL nueva (la imprime cloudflared en el log) -----------------------
extraer_url() {
    grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' "$TUNNEL_LOG" 2>/dev/null | tail -1
}

# --- arranca ----------------------------------------------------------------
reiniciar_cloudflared
i=0
while [ "$i" -lt 24 ]; do
    url=$(extraer_url)
    if [ -n "$url" ]; then
        echo "$url" > "$URL_FILE"
        log "URL vigente: $url"
        break
    fi
    sleep 5
    i=$((i + 1))
done

# --- watchdog (sondeo + relanzado + publicación) ---------------------------
if [ -r "$WATCHDOG" ]; then
    setsid nohup sh "$WATCHDOG" >> "$LOG_DIR/tunnel-watchdog.log" 2>&1 &
    log "watchdog lanzado"
else
    log "NO existe $WATCHDOG (copia deploy/tunnel-watchdog.sh al movil)"
fi

log "cloudflared pids: $(cf_pids | tr '\n' ' ')"
