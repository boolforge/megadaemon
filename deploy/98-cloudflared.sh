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
PREFIX="${MEGADAEMON_PREFIX:-/data/local/tmp/megadaemon}"
LOG_DIR="$PREFIX"
# El tunel NO puede depender de Termux (es una app: si el usuario la cierra
# o la reinstala, el tunel se cae y el movil se queda sin canal publico).
# Ver MEDIDO el 2026-11-21.  Orden: prefijo propio -> copia migrada desde
# Termux -> Termux como ultimo recurso.
CF="${CF:-}"
if [ -z "$CF" ]; then
    for cand in /data/local/tmp/megadaemon/cloudflared \
                /data/data/com.termux/files/usr/bin/cloudflared \
                /data/data/com.agy/files/usr/bin/cloudflared; do
        [ -x "$cand" ] && { CF="$cand"; break; }
    done
fi
# Si solo esta en Termux, se copia al prefijo (root puede leerlo).
if [ -n "$CF" ] && [ "$CF" != "/data/local/tmp/megadaemon/cloudflared" ] \
   && [ -x "$CF" ] && [ ! -x /data/local/tmp/megadaemon/cloudflared ]; then
    cp -f "$CF" /data/local/tmp/megadaemon/cloudflared.new 2>/dev/null \
        && chmod 755 /data/local/tmp/megadaemon/cloudflared.new 2>/dev/null \
        && mv -f /data/local/tmp/megadaemon/cloudflared.new /data/local/tmp/megadaemon/cloudflared 2>/dev/null \
        && CF=/data/local/tmp/megadaemon/cloudflared
fi
[ -n "$CF" ] || CF=/data/data/com.termux/files/usr/bin/cloudflared
TARGET="${CFT_TARGET:-http://127.0.0.1:9090}"
URL_FILE="$LOG_DIR/tunnel.url"
# ⚠️ El daemon lee su estado de AQUÍ (cockpit_server::state_dir()) ⇒ la URL
# del túnel tiene que estar en el MISMO directorio o el `quick` del
# /channel.json nunca se rellena (medido: el script escribía en el log_dir
# y el daemon leía en otro sitio ⇒ el `quick` salía vacío siempre).
# ⚠️ Este script lo ejecuta **Magisk** (service.d) => es HERMANO del
# supervisor, no hijo suyo => NO hereda su entorno => hay que leer el env
# aquí (sin `set -a`: la API key no se exporta a cloudflared ni al watchdog).
ENV_FILE="${MEGADAEMON_ENV:-$PREFIX/megadaemon.env}"
if [ -f "$ENV_FILE" ]; then
    . "$ENV_FILE" 2>/dev/null
fi
# Ni la key ni el token de sesión viajan a cloudflared/watchdog: este
# script no los necesita (y una variable que ya estaba exportada en el
# entorno mantiene su atributo de exportación al reasignarla).
unset MEGADAEMON_API_KEY MEGADAEMON_COCKPIT_TOKEN 2>/dev/null
# El valor por defecto es el del despliegue (`install-phone.sh`): en el móvil
# el estado vive en /data/local/tmp/megadaemon/state.
STATE_DIR="${MEGADAEMON_STATE_DIR:-/data/local/tmp/megadaemon/state}"
STATE_URL_FILE="$STATE_DIR/tunnel.url"
# Publicador del canal (opcional): al subir el túnel, se publica el estado
# para que el enjambre conozca la URL nueva sin intervención manual.
PUBLISH="${MEGADAEMON_PUBLISH:-/data/local/tmp/megadaemon/channel-publish}"
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
        # y en el directorio de estado del daemon (el que lee el canal)
        mkdir -p "$STATE_DIR" 2>/dev/null
        if echo "$url" > "$STATE_URL_FILE" 2>/dev/null; then
            log "URL vigente: $url (también en $STATE_URL_FILE)"
        else
            log "URL vigente: $url (NO se pudo escribir $STATE_URL_FILE)"
        fi
        break
    fi
    sleep 5
    i=$((i + 1))
done

# --- publica el canal (para que el enjambre vea la URL nueva) -------------
# Best effort y en segundo plano: el túnel NO depende de esto (además el
# móvil no lleva PAT ⇒ se publica en un paste, no en el repo).
if [ -x "$PUBLISH" ] && [ -r "$STATE_URL_FILE" ]; then
    setsid nohup sh -c "'$PUBLISH' --state-dir '$STATE_DIR'" >>"$LOG_DIR/publish.log" 2>&1 &
    log "publicación del canal lanzada (paste; sin PAT no se publica en el repo)"
else
    log "sin publicador ($PUBLISH) o sin URL: no se publica (el /channel.json del daemon sigue siendo la vía principal)"
fi

# --- watchdog (sondeo + relanzado + publicación) ---------------------------
if [ -r "$WATCHDOG" ]; then
    setsid nohup sh "$WATCHDOG" >> "$LOG_DIR/tunnel-watchdog.log" 2>&1 &
    log "watchdog lanzado"
else
    log "NO existe $WATCHDOG (copia deploy/tunnel-watchdog.sh al movil)"
fi

log "cloudflared pids: $(cf_pids | tr '\n' ' ')"
