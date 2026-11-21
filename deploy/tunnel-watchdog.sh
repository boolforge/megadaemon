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
# MEDIDO el 2026-11-21: el movil NO tiene curl ni wget (`toybox wget` ->
# "Unknown command", rc=127) y `channel-publish` NO se puede descargar
# (su descarga usaba wget).  Resultado: el watchdog se quedaba SIN SONDEAR
# ("no encuentro ... (sin el no puedo sondear)") => nadie vigilaba el
# tunel y nadie publicaba el canal.  El daemon died -> el movil quedaba
# inalcanzable sin intervencion manual.
#
# Ahora el sondeo y la publicacion los hace el binario PRINCIPAL del daemon
# (`megadaemon-core probe|channel`), que SIEMPRE esta instalado.  Si
# existe `channel-publish`, se usa como alternativa.
DAEMON_BIN="${DAEMON_BIN:-$LOG_DIR/megadaemon-core}"
PUBLISH="${PUBLISH:-$LOG_DIR/channel-publish}"
# ⚠️ Lo lanza Magisk (hermano del supervisor) => hay que leer el env aquí.
# Sin `set -a`: la API key no se exporta a ningún hijo.
ENV_FILE="${MEGADAEMON_ENV:-/data/local/tmp/megadaemon/megadaemon.env}"
if [ -f "$ENV_FILE" ]; then
    . "$ENV_FILE" 2>/dev/null
fi
# Ni la key ni el token de sesión viajan a cloudflared/watchdog: este
# script no los necesita (y una variable que ya estaba exportada en el
# entorno mantiene su atributo de exportación al reasignarla).
unset MEGADAEMON_API_KEY MEGADAEMON_COCKPIT_TOKEN 2>/dev/null
# El estado del daemon (tunnel.url, channel.json, key) vive AQUÍ.
STATE_DIR="${MEGADAEMON_STATE_DIR:-/data/local/tmp/megadaemon/state}"
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
    if [ -x "$DAEMON_BIN" ]; then
        code=$("$DAEMON_BIN" probe "$url" 2>>"$LOG")
        case "$code" in
            ''|*[!0-9]*) code=0 ;;
        esac
        echo "$code"
        return
    fi
    if [ -x "$PUBLISH" ]; then
        code=$("$PUBLISH" --probe "$url" 2>>"$LOG")
        [ -z "$code" ] && code=0
        echo "$code"
        return
    fi
    # Sin binario: se responde por el SOCKET (sin red, sin proceso): se
    # comprueba el puerto en /proc/net/tcp.  Es un "vivo/no vivo", no un
    # codigo HTTP, pero evita depender de binarios externos.
    echo 0
}

publicar() {
    url="$1"
    pids="$2"
    # El binario principal publica el estado del canal (channel.json + paste).
    # El daemon tambien lo hace por su cuenta (cada 5 min, si cambia), asi
    # que esto es solo el refresco inmediato tras un tunel nuevo.
    if [ -x "$DAEMON_BIN" ]; then
        "$DAEMON_BIN" channel publish >>"$LOG" 2>&1
        log "publicado (daemon): $url (pids: $pids)"
        return
    fi
    if [ -x "$PUBLISH" ]; then
        # --state-dir explícito: si no, el binario usaría su valor por defecto
        # (otra ruta) => el estado se escribiría donde el daemon no lo lee.
        "$PUBLISH" --state-dir "$STATE_DIR" --quick "$url" --pids "$pids" >>"$LOG" 2>&1
        log "publicado: $url (pids: $pids)"
        return
    fi
    log "sin binario de publicacion: no puedo publicar (channel.json + paste)"
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
