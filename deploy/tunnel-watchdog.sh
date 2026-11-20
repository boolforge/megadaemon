#!/system/bin/sh
# ---------------------------------------------------------------------------
# WATCHDOG DEL TÚNEL (Cloudflare quick tunnel) — canal de RESPALDO
#
# Mientras DuckDNS sea el canal primario (dominio estable), este script
# vigila el *quick tunnel* de Cloudflare, que rota el hostname en cada
# reinicio y no resuelve. Si nadie lo cuida, cada reinicio del móvil deja a
# los agentes sin canal (530/403) y hay que hardcodear URLs nuevas.
#
# Qué hace, en bucle:
#   1. lee la URL vigente de `tunnel.url` (la escribe `recover_channels.sh`);
#   2. la sondea: 530/000/502/503/504 = túnel MUERTO; 401/403 = el daemon
#      respondió (túnel VIVO, lo que pasa si la key rotó);
#   3. si está muerto, comprueba la SALUD del móvil: con **load > MAX_LOAD**
#      NO relanza (el móvil saturado no va a improving con otro proceso);
#   4. si el móvil está bien, relanza cloudflared usando el kit existente
#      `recover_channels.sh` (no se duplica la lógica de arranque) y espera
#      a que la URL nueva aparezca en `tunnel.url`.
#
# Uso (root en el móvil):
#     su -mm -c 'sh /data/local/tmp/megadaemon/tunnel-watchdog.sh'
#
# Variables de entorno:
#     INTERVAL=60      segundos entre comprobaciones
#     MAX_LOAD=8       load 1-min por encima del cual NO se relanza
#     LOG_DIR=/data/local/tmp/megadaemon
#
# Sin dependencias: POSIX sh + curl de Termux. No reinicia el daemon.
# ---------------------------------------------------------------------------
INTERVAL="${INTERVAL:-60}"
MAX_LOAD="${MAX_LOAD:-8}"
LOG_DIR="${LOG_DIR:-/data/local/tmp/megadaemon}"
URL_FILE="$LOG_DIR/tunnel.url"
RECOVER="$LOG_DIR/recover_channels.sh"
LOG="$LOG_DIR/tunnel-watchdog.log"
LOCK="$LOG_DIR/tunnel-watchdog.lock"
CURL="${CURL:-/data/data/com.termux/files/usr/bin/curl}"

mkdir -p "$LOG_DIR" 2>/dev/null

# --- un solo watchdog ------------------------------------------------------
if ! mkdir "$LOCK" 2>/dev/null; then
    echo "ya hay un watchdog corriendo ($LOCK)" >&2
    exit 0
fi
trap 'rmdir "$LOCK" 2>/dev/null' EXIT INT TERM

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" >> "$LOG"
    echo "$*"
}

# load 1-min como entero (parte entera antes del punto).
load1() {
    if [ -r /proc/loadavg ]; then
        set -- $(cat /proc/loadavg)
        echo "${1%%.*}"
    else
        echo 0
    fi
}

# 530/000/502/503/504 = túnel muerto.  401/403 = el daemon respondió = vivo.
probe() {
    url="$1"
    [ -z "$url" ] && { echo 000; return; }
    code=$("$CURL" -s -o /dev/null -w '%{http_code}' --max-time 12 "$url/mcp" 2>/dev/null)
    [ -z "$code" ] && code=000
    echo "$code"
}

dead() {
    case "$1" in
        000|530|502|503|504) return 0 ;;
        *) return 1 ;;
    esac
}

log "watchdog arrancado (interval=${INTERVAL}s max_load=${MAX_LOAD})"

while : ; do
    url=""
    [ -r "$URL_FILE" ] && url=$(cat "$URL_FILE" 2>/dev/null)
    url=$(echo "$url" | tr -d '\r\n ')

    if [ -z "$url" ]; then
        log "sin URL en $URL_FILE: relanzo el kit para publicarla"
        [ -r "$RECOVER" ] && sh "$RECOVER" >> "$LOG" 2>&1
        sleep "$INTERVAL"
        continue
    fi

    code=$(probe "$url")
    if dead "$code"; then
        L=$(load1)
        log "túnel MUERTO ($code) en $url — load1=${L}"
        if [ "$L" -ge "$MAX_LOAD" ] 2>/dev/null; then
            log "  móvil SATURADO (load1=${L} >= ${MAX_LOAD}): no relanzo (sería peor)"
            sleep "$INTERVAL"
            continue
        fi
        if [ -r "$RECOVER" ]; then
            log "  relanzo cloudflared con recover_channels.sh"
            sh "$RECOVER" >> "$LOG" 2>&1
        else
            log "  ERROR: no existe $RECOVER (¿despliegue incompleto?)"
        fi
        # esperar la URL nueva
        i=0
        while [ "$i" -lt 18 ]; do
            sleep 5
            nueva=$(cat "$URL_FILE" 2>/dev/null | tr -d '\r\n ')
            if [ -n "$nueva" ] && [ "$nueva" != "$url" ]; then
                log "  URL nueva: $nueva"
                break
            fi
            i=$((i + 1))
        done
    else
        log "túnel vivo ($code) $url"
    fi

    sleep "$INTERVAL"
done
