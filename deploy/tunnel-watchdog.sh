#!/system/bin/sh
# ---------------------------------------------------------------------------
# WATCHDOG DEL CANAL — sondea, relanza si hace falta y PUBLICA la URL
#
# El criterio que evita el absurdo de matar el túnel por una key caducada:
#
#   * `530 / 000 / 502 / 503 / 504` => el túnel está **MUERTO** (o no hay red)
#     => se relanza `cloudflared`.
#   * `401 / 403` => **el daemon respondió** => el túnel está **VIVO** y lo
#     que falla es la **credencial** => **NO se toca el túnel** (matarlo
#     cambiaría la URL otra vez y regeneraría la key: más caos).
#   * Si el móvil está **saturado** (load1 >= MAX_LOAD) **no se relanza**:
#     con load 19-21 otro proceso no mejora nada.
#
# ⚠️ El móvil **no tiene curl ni wget** ⇒ el sondeo y la publicación los hace
# el binario `channel-publish` (el mismo código Rust que la tool MCP
# `channel_publish`): `--probe <url>` imprime el código HTTP y
# `--quick <url> --pids "<pids>"` publica el estado (channel.json + paste
# público) ⇒ **así el sandbox redescubre la URL aunque el túnel esté caído**.
#
# Variables: INTERVAL (60s), MAX_LOAD (8), LOG_DIR, CF, CFT_TARGET
# ---------------------------------------------------------------------------
INTERVAL="${INTERVAL:-60}"
MAX_LOAD="${MAX_LOAD:-8}"
LOG_DIR="${LOG_DIR:-/data/local/tmp/megadaemon}"
URL_FILE="$LOG_DIR/tunnel.url"
TUNNEL_LOG="$LOG_DIR/tunnel.log"
LOG="$LOG_DIR/tunnel-watchdog.log"
PUBLISH="${PUBLISH:-$LOG_DIR/channel-publish}"
CF="${CF:-/data/data/com.termux/files/usr/bin/cloudflared}"
TARGET="${CFT_TARGET:-http://127.0.0.1:9090}"

mkdir -p "$LOG_DIR" 2>/dev/null

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] watchdog: $*" >> "$LOG"
    echo "$*"
}

cf_pids() {
    ps -A -o PID,NAME 2>/dev/null | grep -i cloudflared | grep -v grep | awk '{print $1}'
}

load1() {
    if [ -r /proc/loadavg ]; then
        set -- $(cat /proc/loadavg)
        echo "${1%%.*}"
    else
        echo 0
    fi
}

extraer_url() {
    grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' "$TUNNEL_LOG" 2>/dev/null | tail -1
}

# Dead: 530/000/502/503/504.  401/403 = el daemon respondió = VIVO.
es_muerto() {
    case "$1" in
        0|530|502|503|504) return 0 ;;
        *) return 1 ;;
    esac
}

# Sondea con el binario propio (el móvil no tiene curl).
sondear() {
    url="$1"
    [ -z "$url" ] && { echo 0; return; }
    if [ ! -x "$PUBLISH" ]; then
        log "no encuentro $PUBLISH (sin él no puedo sondear)"
        echo 0
        return
    fi
    code=$("$PUBLISH" --probe "$url" 2>>"$LOG")
    [ -z "$code" ] && code=0
    echo "$code"
}

publicar() {
    url="$1"
    pids="$2"
    if [ ! -x "$PUBLISH" ]; then
        log "sin $PUBLISH: no puedo publicar (channel.json + paste)"
        return
    fi
    "$PUBLISH" --quick "$url" --pids "$pids" >>"$LOG" 2>&1
    log "publicado: $url (pids: $pids)"
}

log "watchdog arrancado (interval=${INTERVAL}s max_load=${MAX_LOAD})"

while : ; do
    url=""
    [ -r "$URL_FILE" ] && url=$(cat "$URL_FILE" 2>/dev/null)
    url=$(echo "$url" | tr -d '\r\n ')

    if [ -z "$url" ]; then
        log "sin URL en $URL_FILE: relanzo para publicarla"
        L=$(load1)
        if [ "$L" -lt "$MAX_LOAD" ] 2>/dev/null; then
            for p in $(cf_pids); do kill -9 "$p" 2>/dev/null; done
            sleep 1
            setsid nohup "$CF" tunnel --no-autoupdate --url "$TARGET" >> "$TUNNEL_LOG" 2>&1 &
            i=0
            while [ "$i" -lt 24 ]; do
                sleep 5
                nueva=$(extraer_url)
                [ -n "$nueva" ] && { url="$nueva"; echo "$url" > "$URL_FILE"; break; }
                i=$((i + 1))
            done
            log "URL nueva: ${url:-ninguna}"
            [ -n "$url" ] && publicar "$url" "$(cf_pids | tr '\n' ' ')"
        else
            log "movil SATURADO (load1=$L): no relanzo"
        fi
        sleep "$INTERVAL"
        continue
    fi

    code=$(sondear "$url")
    if es_muerto "$code"; then
        L=$(load1)
        log "tunel MUERTO (HTTP $code) en $url - load1=$L"
        if [ "$L" -ge "$MAX_LOAD" ] 2>/dev/null; then
            log "  movil SATURADO (load1=$L >= $MAX_LOAD): no relanzo"
            sleep "$INTERVAL"
            continue
        fi
        for p in $(cf_pids); do
            log "  matando cloudflared pid $p"
            kill -9 "$p" 2>/dev/null
        done
        sleep 1
        setsid nohup "$CF" tunnel --no-autoupdate --url "$TARGET" >> "$TUNNEL_LOG" 2>&1 &
        log "  relanzado cloudflared (setsid nohup, independiente del daemon)"
        i=0
        while [ "$i" -lt 24 ]; do
            sleep 5
            nueva=$(extraer_url)
            if [ -n "$nueva" ] && [ "$nueva" != "$url" ]; then
                log "  URL nueva: $nueva"
                echo "$nueva" > "$URL_FILE"
                publicar "$nueva" "$(cf_pids | tr '\n' ' ')"
                break
            fi
            i=$((i + 1))
        done
    else
        # 401/403 = el daemon respondió: el túnel está VIVO. No se toca nada.
        log "tunel vivo (HTTP $code) $url"
    fi

    sleep "$INTERVAL"
done
