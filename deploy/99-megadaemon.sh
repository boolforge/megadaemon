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
# Puerto del cockpit.  Configurable por MEGADAEMON_COCKPIT_PORT (se lee del
# fichero de entorno); el valor por defecto es el que usa el despliegue actual.
PORT=9090
URL_FILE=/sdcard/megadaemon.url
STATE_FILE=/sdcard/megadaemon.state
# Fallos de arranque seguidos => se considera fallo de código y el supervisor
# para (antes reintentaba en bucle infinito y se comía la batería).
MAX_FAST_FAILS=3
fast_fails=0

# Estado del daemon para el usuario: un fichero con la última línea conocida.
write_state() {
    printf '%s %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$1" > "$STATE_FILE" 2>/dev/null
}

# URL real del cockpit: el usuario no tiene que adivinar el puerto.
#
# OJO (P0, 2026-11-19): el puerto 9090 esta protegido por el gate de API key
# y el NAVEGADOR NO PUEDE mandar la cabecera X-API-Key -> en Chrome / sale un
# 401 con un JSON de error, no la interfaz.  Las URLs que de verdad funcionan
# en el navegador son las de alias (3000/3030/8090/8080): las sirve el puente
# web con binarios antiguos y el propio daemon con el binario nuevo.
write_url() {
    {
        echo "# MegaDaemon - cockpit (generado por 99-megadaemon.sh)"
        echo "# ABRE EN EL MOVIL (Chrome) - cualquiera de estos 4 funciona:"
        for p in 3000 3030 8090 8080; do
            echo "UI=http://localhost:$p/"
        done
        echo "HEALTH=http://localhost:3000/health"
        echo "# Cockpit puro (solo loopback, EXIGE cabecera X-API-Key):"
        echo "RAW=http://127.0.0.1:$PORT/"
    } > "$URL_FILE" 2>/dev/null
}

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

# --- deteccion de puertos alias ya servidos ------------------------------
# El binario nuevo (v13.4+) escucha el cockpit en 0.0.0.0 y ademas en los
# puertos alias 3000/3030/8090/8080.  Si el daemon ya los sirve, el puente
# web sobra y arrancarlo seria pelear por el bind (EADDRINUSE), lo que puede
# tumbar al daemon nuevo al arrancar.  Se lee /proc/net/tcp{,6}: sin forks y
# sin procesos extra (el movil va justo de RAM).
#   3000=0BB8  3030=0BC6  8080=1F90  8090=1F9A   (hex)
alias_ports_listening() {
    for hp in 0BB8 0BC6 1F90 1F9A; do
        # <local>:<puerto> <remoto> 0A   (0A = estado LISTEN)
        if grep -qiE ":$hp [0-9a-f]{8}:[0-9a-f]{4} 0A" /proc/net/tcp /proc/net/tcp6 2>/dev/null; then
            return 0
        fi
    done
    return 1
}

# --- puente web (binarios antiguos sin puertos alias) ---------------------
# Hasta que se despliegue un binario que escuche en 3000/3030/8090 (alias),
# el cockpit real sólo escucha en 9090 y el navegador recibe 401 (el gate de
# API key necesita cabeceras que Chrome no envía).  Este puente local
# reenvía 3000/3030/8090/8080 -> 9090 e inyecta la X-API-Key del propio
# dispositivo, de modo que la UI carga igual en el móvil.
BRIDGE="$PREFIX/web_bridge.py"

ensure_web_bridge() {
    [ -f "$BRIDGE" ] || return 0
    if pgrep -f "web_bridge.py" > /dev/null 2>&1; then
        return 0
    fi
    # El daemon ya sirve los alias (binario nuevo): el puente sobra.
    if alias_ports_listening; then
        log "el daemon ya escucha los puertos alias: no hace falta puente web"
        return 0
    fi
    py=""
    for cand in /data/data/com.termux/files/usr/bin/python3 \
                /data/data/com.agy/files/usr/bin/python3 \
                /system/bin/python3; do
        [ -x "$cand" ] && { py="$cand"; break; }
    done
    if [ -z "$py" ]; then
        log "AVISO: sin python3 no se puede arrancar el puente web ($BRIDGE)"
        return 1
    fi
    setsid nohup "$py" "$BRIDGE" >> "$PREFIX/bridge.log" 2>&1 < /dev/null &
    log "puente web arrancado: 3000/3030/8090/8080 -> 127.0.0.1:$PORT (python3: $py)"
    return 0
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
    # El puerto puede venir del fichero de entorno (se carga justo arriba).
    PORT="${MEGADAEMON_COCKPIT_PORT:-$PORT}"
    setsid nohup "$BIN" cockpit "$PORT" >> "$LOG" 2>&1 < /dev/null &
    pid=$!
    log "megadaemon-core arrancado PID $pid (cockpit en 127.0.0.1:$PORT, instancia=$MEGADAEMON_INSTANCE_NAME/$MEGADAEMON_INSTANCE_ROLE)"

    # ¿Sigue vivo a los 3 s?  Si muere inmediatamente no es un fallo de red:
    # es un fallo de arranque (binario roto, plantilla que revienta, etc.).
    sleep 3
    if ! kill -0 "$pid" 2>/dev/null; then
        fast_fails=$((fast_fails + 1))
        log "ERROR: megadaemon-core ha muerto al arrancar (fallo rapido #$fast_fails/$MAX_FAST_FAILS)"
        log "----- últimas líneas del log -----"
        tail -n 25 "$LOG" 2>/dev/null | while IFS= read -r l; do log "  | $l"; done
        log "-------------------------------"
        if [ "$fast_fails" -ge "$MAX_FAST_FAILS" ]; then
            write_state "FALLO_CODIGO: el binario muere al arrancar; el supervisor NO insiste mas"
            log "ERROR: $MAX_FAST_FAILS fallos de arranque seguidos => se para el supervisor (fallo de codigo, no transitorio)."
            log "       Estado en $STATE_FILE ; log en $LOG ; URL en $URL_FILE"
            return 2
        fi
        write_state "FALLO_ARRANQUE: reintentando (fallo #$fast_fails/$MAX_FAST_FAILS)"
        return 1
    fi
    fast_fails=0
    ensure_web_bridge
    write_state "OK: cockpit vivo en http://localhost:$PORT/"
    write_url
    log "cockpit verificado vivo -> http://localhost:$PORT/ (URL guardada en $URL_FILE)"
    return 0
}

# --- loop del supervisor --------------------------------------------------
last_update=0
# El auto-update se comprueba cada 6 h (21600 s).  Se hace FUERA del camino
# crítico: si falla, el daemon sigue sirviendo la versión anterior.
UPDATE_EVERY=${MEGADAEMON_UPDATE_EVERY:-21600}

try_update() {
    now=$(date +%s)
    [ $((now - last_update)) -lt "$UPDATE_EVERY" ] && return 0
    last_update=$now
    upd="$PREFIX/auto-update.sh"
    if [ ! -f "$upd" ]; then
        # Se intenta traer el script desde el propio repo (el móvil tiene git).
        if [ -d "$PREFIX/repo/.git" ]; then
            (cd "$PREFIX/repo" && git pull --ff-only origin main >>"$LOG" 2>&1)
            cp -f "$PREFIX/repo/deploy/auto-update.sh" "$upd" 2>/dev/null && chmod 755 "$upd"
        fi
    fi
    [ -f "$upd" ] || { log "auto-update: script no disponible"; return 0; }
    log "auto-update: comprobando releases..."
    "$upd" >>"$LOG" 2>&1 || log "auto-update: fallo (se mantiene la version actual)"
}

i=0
last_update=0
write_state "ARRANCANDO: supervisor activo, puerto $PORT"
while true; do
    rotate
    if ! pgrep -f "megadaemon-core cockpit" > /dev/null 2>&1; then
        log "megadaemon no está corriendo: relanzando (intento $i)"
        start_daemon
        rc=$?
        if [ "$rc" -eq 2 ]; then
            log "supervisor detenido: fallo de codigo al arrancar (ver $LOG y $STATE_FILE)"
            exit 0
        fi
        i=$((i + 1))
        # backoff: 10s, 20s, 40s ... hasta 300s
        s=$((10 * (1 << (i < 5 ? i : 5))))
        [ "$s" -gt 300 ] && s=300
        sleep "$s"
    else
        i=0
        ensure_web_bridge
        try_update
        # Si el puerto que usa el navegador no está escuchando, se reintenta
        # pronto (10 s): durante ese hueco el usuario ve ERR_CONNECTION_REFUSED.
        # En el caso normal la cadencia sigue siendo de 60 s -> no se añaden
        # procesos ni polling agresivo (el móvil va justo de RAM).
        if alias_ports_listening; then
            sleep 60
        else
            sleep 10
        fi
    fi
done
