#!/system/bin/sh
# =====================================================================
# 99-megadaemon.sh — supervisor de arranque de MegaDaemon (Magisk)
#
# Se instala en: /data/adb/service.d/99-megadaemon.sh  (chmod 755)
# Magisk ejecuta todo lo que hay en service.d al arrancar el móvil.
#
# Garantías:
#   * megadaemon arranca como demonio DESACOPLADO (setsid + nohup): no muere
#     al cerrar Termux ni la sesión de shell que lo lanzó;
#   * si el proceso cae, se relanza (supervisor con backoff);
#   * log rotativo: 10 MB por fichero, 5 ficheros (=> 50 MB máx.);
#   * la API key y el resto de secretos se leen de un fichero 600, NUNCA
#     de la línea de comandos (quedaría en /proc/*/cmdline).
#
# v13.2.5 — https://github.com/boolforge/megadaemon
# =====================================================================

PREFIX=/data/local/tmp/megadaemon
BIN="$PREFIX/megadaemon-core"
ENV_FILE="$PREFIX/megadaemon.env"
LOG=/sdcard/megadaemon.log
LOG_DIR=/sdcard/megadaemon-logs
MAX_BYTES=$((10 * 1024 * 1024))
KEEP=5
PORT=9090

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" >> "$LOG"
}

# --- rotación del log -----------------------------------------------------
rotate() {
    [ -f "$LOG" ] || return 0
    sz=$(stat -c %s "$LOG" 2>/dev/null || echo 0)
    [ "$sz" -gt "$MAX_BYTES" ] || return 0
    mkdir -p "$LOG_DIR" 2>/dev/null
    # renombra .4 -> .5, .3 -> .4 ... .1 -> .2 y el actual a .1
    i=$((KEEP - 1))
    while [ "$i" -ge 1 ]; do
        [ -f "$LOG_DIR/megadaemon.log.$i" ] && mv -f "$LOG_DIR/megadaemon.log.$i" "$LOG_DIR/megadaemon.log.$((i + 1))"
        i=$((i - 1))
    done
    mv -f "$LOG" "$LOG_DIR/megadaemon.log.1"
    log "log rotado (tamaño máximo $MAX_BYTES bytes)"
}

# --- interfaz web: la plantilla tiene que existir ANTES de arrancar -------
# El cockpit busca la plantilla en $MEGADAEMON_WEB_TEMPLATE, <bin>/web/,
# ./web/ y en el path privado de la app.  En el móvil el binario vive en
# /data/local/tmp/megadaemon y el despliegue NO dejaba la plantilla ahí:
# el cockpit servía un <h1> de 45 bytes y la UI "no funcionaba" sin que
# saliera ningún error.  Aquí se copia al primer sitio donde aparezca.
ensure_web_template() {
    if [ -f "$PREFIX/web/cockpit_template.html" ]; then
        export MEGADAEMON_WEB_TEMPLATE="$PREFIX/web/cockpit_template.html"
        return 0
    fi
    mkdir -p "$PREFIX/web" 2>/dev/null
    for src in \
        /data/user/0/com.agy/files/megadaemon/web/cockpit_template.html \
        /sdcard/megadaemon/web/cockpit_template.html \
        /sdcard/Download/megadaemon/web/cockpit_template.html
    do
        if [ -f "$src" ]; then
            cp -f "$src" "$PREFIX/web/cockpit_template.html" 2>/dev/null && {
                export MEGADAEMON_WEB_TEMPLATE="$PREFIX/web/cockpit_template.html"
                log "plantilla web copiada desde $src"
                return 0
            }
        fi
    done
    log "AVISO: no se encuentra cockpit_template.html; la UI caera en el placeholder"
    return 1
}

# --- identidad de instancia ----------------------------------------------
# El movil Samsung es la instancia MAESTRA (rol primary): es la que reparte
# tareas, decide y sincroniza.  El PC Linux y el resto son workers.
export MEGADAEMON_INSTANCE_ROLE="${MEGADAEMON_INSTANCE_ROLE:-primary}"
export MEGADAEMON_INSTANCE_NAME="${MEGADAEMON_INSTANCE_NAME:-samsung}"
export MEGADAEMON_INSTANCE_DEVICE="${MEGADAEMON_INSTANCE_DEVICE:-samsung}"

# --- arranque -------------------------------------------------------------
start_daemon() {
    [ -x "$BIN" ] || { log "ERROR: no existe $BIN"; return 1; }
    if [ -f "$ENV_FILE" ]; then
        # set -a para exportar todo lo definido y que lo lean los hijos
        set -a
        . "$ENV_FILE"
        set +a
    else
        log "AVISO: no hay $ENV_FILE (arranque sin secretos externos)"
    fi
    # La identidad se define antes de exportar el env: `set -a` ya ha
    # exportado el fichero, estas variables se añaden encima.
    export MEGADAEMON_INSTANCE_ROLE="${MEGADAEMON_INSTANCE_ROLE:-primary}"
    export MEGADAEMON_INSTANCE_NAME="${MEGADAEMON_INSTANCE_NAME:-samsung}"
    export MEGADAEMON_INSTANCE_DEVICE="${MEGADAEMON_INSTANCE_DEVICE:-samsung}"
    ensure_web_template
    cd "$PREFIX" || return 1
    setsid nohup "$BIN" cockpit "$PORT" >> "$LOG" 2>&1 < /dev/null &
    pid=$!
    log "megadaemon-core arrancado PID $pid (cockpit en 127.0.0.1:$PORT, instancia=$MEGADAEMON_INSTANCE_NAME/$MEGADAEMON_INSTANCE_ROLE)"
}

# --- loop del supervisor --------------------------------------------------
i=0
while true; do
    rotate
    if ! pgrep -f "megadaemon-core cockpit" > /dev/null 2>&1; then
        log "megadaemon no está corriendo: relanzando (intento $i)"
        start_daemon
        i=$((i + 1))
        # backoff: 10s, 20s, 40s ... hasta 300s
        s=$((10 * (1 << (i < 5 ? i : 5))))
        [ "$s" -gt 300 ] && s=300
        sleep "$s"
    else
        i=0
        sleep 60
    fi
done
