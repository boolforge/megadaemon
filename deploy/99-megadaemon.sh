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
# v15: binario anterior conocido-bueno (para auto-rollback si el nuevo no arranca).
PREV_BIN="$PREFIX/megadaemon-core.prev"
ENV_FILE="$PREFIX/megadaemon.env"
# ⚠️ El entorno se lee AQUÍ, en la cabecera, y no justo antes de arrancar el
# daemon: los scripts hermanos que se lanzan por separado (Magisk service.d:
# 98-cloudflared.sh y su watchdog) son hermanos del supervisor, NO hijos =>
# no heredan su entorno => leen el env por su cuenta. Y todo hijo del
# supervisor necesita `MEGADAEMON_STATE_DIR` (tunnel.url, channel.json,
# peers.json, audit.ndjson, board.sqlite y la key: TODO en el mismo sitio).
if [ -f "$ENV_FILE" ]; then
    . "$ENV_FILE" 2>/dev/null
    # Sólo las variables NO secretas se exportan a los hijos: la API key se
    # exporta más abajo, sólo al daemon (igual que antes).
    export MEGADAEMON_STATE_DIR="${MEGADAEMON_STATE_DIR:-/data/local/tmp/megadaemon/state}"
    export MEGADAEMON_LISTEN="${MEGADAEMON_LISTEN:-127.0.0.1:9090}"
fi
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
write_url() {
    {
        echo "# MegaDaemon - cockpit (generado por 99-megadaemon.sh)"
        echo "URL=http://localhost:$PORT/"
        echo "HEALTH=http://localhost:$PORT/health"
    } > "$URL_FILE" 2>/dev/null
}

# Intérprete python3 del sistema (Termux o el sandbox de Antigravity): lo
# usan el puente web y la sincronización del puente MCP de Antigravity.
# Se resuelve UNA vez: el móvil va justo de RAM.
PY_BIN=""
for cand in /data/data/com.termux/files/usr/bin/python3 \
            /data/data/com.agy/files/usr/bin/python3 \
            /system/bin/python3; do
    [ -x "$cand" ] && { PY_BIN="$cand"; break; }
done
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
# en el navegador son las de alias (80/3000/3030/8090/8080): las sirve el
# puente web con binarios antiguos y el propio daemon con el binario nuevo.
write_url() {
    {
        echo "# MegaDaemon - cockpit (generado por 99-megadaemon.sh)"
        echo "# ABRE EN EL MOVIL (Chrome) - cualquiera de estos funciona:"
        for p in 80 3000 3030 8090 8080; do
            hp=$(printf '%04X' "$p")
            # Solo los que de verdad estan escuchando (si no, el navegador
            # da ERR_CONNECTION_REFUSED).
            if grep -qiE ":$hp [0-9a-f]{8}:[0-9a-f]{4} 0A" /proc/net/tcp /proc/net/tcp6 2>/dev/null; then
                echo "UI=http://localhost:$p/"
            fi
        done
        echo "HEALTH=http://localhost:$PORT/health"
        echo "# Cockpit puro (solo loopback, EXIGE cabecera X-API-Key):"
        echo "RAW=http://127.0.0.1:$PORT/"
        tun=$(cat "$WEB_TUNNEL_URL_FILE" 2>/dev/null)
        if [ -n "$tun" ]; then
            echo "# Tunel publico (URL EFIMERA: se regenera al reiniciar el tunel):"
            echo "PUBLIC=$tun/"
            echo "PUBLIC_HEALTH=$tun/health"
        fi
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

# --- PODA DE CACHES (v17) --------------------------------------------------
# MEDIDO el 2026-11-21 en el movil: /data/local/tmp/megadaemon ocupaba 257 MB,
# de los que 90 MB eran caches REGENERABLES:
#   * dex/       (29 MB) -> DEX decompilados: se regeneran al vuelo;
#   * sync_snap/ (60 MB) -> snapshots de conversaciones sincronizadas: se
#     regeneran, y los antiguos no aportan nada.
#
# Se podan por ANTIGUEDAD (no se toca lo reciente) y una sola vez al dia:
# en reposo no cuesta NADA.
PRUNE_DIAS_DEX=${MEGADAEMON_PRUNE_DIAS_DEX:-7}
PRUNE_DIAS_SNAP=${MEGADAEMON_PRUNE_DIAS_SNAP:-14}
last_prune=0
prune_caches() {
    now=$(date +%s)
    [ $((now - last_prune)) -lt 86400 ] && return 0
    last_prune=$now
    _liberado=0
    for d in "$PREFIX/dex" "$PREFIX/sync_snap"; do
        [ -d "$d" ] || continue
        case "$d" in
            *dex) dias=$PRUNE_DIAS_DEX ;;
            *)     dias=$PRUNE_DIAS_SNAP ;;
        esac
        _antes=$(du -sk "$d" 2>/dev/null | cut -f1)
        # ficheros con mas de $dias de antiguedad
        find "$d" -type f -mtime "+$dias" -delete 2>/dev/null
        _despues=$(du -sk "$d" 2>/dev/null | cut -f1)
        if [ -n "$_antes" ] && [ -n "$_despues" ] && [ "$_antes" -gt "$_despues" ]; then
            log "poda: $d ${_antes}KB -> ${_despues}KB (ficheros de mas de ${dias} dias)"
        fi
    done
    # Directorios vacios que quedan tras la poda
    find "$PREFIX/dex" "$PREFIX/sync_snap" -type d -empty -delete 2>/dev/null
    return 0
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
# ---------------------------------------------------------------------------
# v15: watchdog de RESPONSIVIDAD (no de "vivo").
#
# `pgrep` sólo dice que el proceso existe.  Con un mutex envenenado por un
# panic, el proceso sigue vivo pero CADA petición muere a medias => el
# túnel de Cloudflare ve 502 y el móvil parece "caído" sin estarlo.
# Aquí se pregunta de verdad: ¿contesta /health por localhost?  Con DOS
# fallos seguidos (la red del móvil da falsos positivos) se reinicia.
# El sondeo es LOCAL (127.0.0.1): salir a internet y volver por el túnel
# daría falsos positivos y no mide lo que importa.
# ---------------------------------------------------------------------------
# v15: watchdog de RESPONSIVIDAD (no de "vivo").
#
# `pgrep` sólo dice que el proceso existe.  Con un mutex envenenado por un
# panic, el proceso sigue vivo pero CADA petición muere a medias => el
# túnel de Cloudflare ve 502 y el móvil parece "caído" sin estarlo.
#
# Reglas (importantes, aprendidas en caliente): el sondeo es LOCAL
# (127.0.0.1): salir a internet y volver por el túnel daría falsos
# positivos.  NO se sondea en bucle rápido: durante un arranque /health no
# contesta y eso NO es un fallo => se exige 1 sondeo cada 30 s y 3 fallos
# SEGUIDOS antes de reiniciar, y tras reiniciar se ESPERA a que vuelva
# (si no, el propio watchdog se dispara en bucle y tumba el móvil solo).
daemon_unresponsive_streak=0
last_resp_check=0
# Sonda de UNA ruta.  Devuelve 0 si contesta 200.
# Usa 127.0.0.3 como IP de origen cuando se puede (cubo de rate limit
# propio: el del túnel es 127.0.0.1).  Si el bind falla, se usa el normal.
probe_path() {
    local port="$1" path="$2" out code
    out=$("$TUNNEL_CURL" -s -m 8 ${PROBE_IFACE:+--interface $PROBE_IFACE} \
        -o /dev/null -w '%{http_code}' "http://127.0.0.1:$port$path" 2>/dev/null)
    code=$(echo "$out" | tr -dc '0-9')
    [ "$code" = "200" ]
}

# Detecta una sola vez si el interface de sondeo propio está disponible.
probe_iface_init() {
    [ -n "$PROBE_IFACE" ] && return 0
    [ -x "$TUNNEL_CURL" ] || return 0
    if "$TUNNEL_CURL" -s -m 5 --interface 127.0.0.3 -o /dev/null \
        "http://127.0.0.1:$PORT/health" 2>/dev/null; then
        PROBE_IFACE="127.0.0.3"
    fi
}

daemon_responsive() {
    local port="${1:-9090}"
    # DOS sondas, no una:
    #   /health      -> ¿vive el servidor HTTP?
    #   /api/power   -> ¿responde el ESTADO compartido (mutex/caché)?
    # Un daemon con un mutex envenenado puede seguir contestando /health
    # con 200 mientras /api/power revienta: por eso hay que mirar las dos.
    if [ -x "$TUNNEL_CURL" ]; then
        probe_path "$port" "/health" || return 1
        probe_path "$port" "/api/power" || return 1
        return 0
    fi
    if [ -x /data/data/com.termux/files/usr/bin/python3 ]; then
        /data/data/com.termux/files/usr/bin/python3 -c \
            "import http.client,sys
try:
    c=http.client.HTTPConnection('127.0.0.1',$port,timeout=8)
    c.request('GET','/health')
    sys.exit(0 if c.getresponse().status==200 else 1)
except Exception:
    sys.exit(1)" >/dev/null 2>&1 && return 0
        return 1
    fi
    return 0   # sin sonda: no se puede afirmar nada => no se reinicia
}

wait_daemon_responsive() {
    local port="${1:-9090}" i=0
    while [ $i -lt 15 ]; do
        if daemon_responsive "$port"; then return 0; fi
        i=$((i + 1)); sleep 3
    done
    return 1
}

check_daemon_responsive() {
    local port="${1:-9090}" now
    now=$(date +%s)
    # Como mucho una comprobación cada 30 s.
    [ $((now - last_resp_check)) -lt 30 ] && return 0
    last_resp_check=$now
    if daemon_responsive "$port"; then
        daemon_unresponsive_streak=0
        # Tope de RAM: el daemon no puede inflarse (el móvil va al 98 % de
        # RAM y el OOM killer ya se ha carried el túnel dos veces).
        rss=$(energy_daemon_rss_kb)
        if [ -n "$rss" ] && [ "$rss" -gt "$ENERGY_DAEMON_RSS_MAX_KB" ]; then
            log "el daemon usa ${rss}kB (tope ${ENERGY_DAEMON_RSS_MAX_KB}kB): lo reinicio para no empujar al OOM killer"
            pkill -x megadaemon-core 2>/dev/null
            sleep 3
            start_daemon
            wait_daemon_responsive "$port" || log "el daemon no respondio tras el reinicio por RSS"
        fi
        return 0
    fi
    daemon_unresponsive_streak=$((daemon_unresponsive_streak + 1))
    log "el daemon no responde /health ni /api/power (fallo $daemon_unresponsive_streak/3)"
    if [ "$daemon_unresponsive_streak" -ge 3 ]; then
        daemon_unreachable=1
        daemon_unresponsive_streak=0
        log "el daemon esta VIVO pero NO responde /health: lo reinicio (puede ser un mutex envenenado por un panic)"
        pkill -x megadaemon-core 2>/dev/null
        sleep 3
        start_daemon
        wait_daemon_responsive "$port" || log "el daemon no respondio tras el reinicio (sigue el supervisor)"
    fi
    return 0
}

start_daemon() {
    # v15: si el binario existe pero NO es ejecutable (un `mv` sin `chmod`
    # lo deja en 644), el daemon no arranca nunca y el supervisor dice
    # "no existe", que manda el diagnostico por donde no es.  Ahora se
    # AUTO-REPARA y el log dice la verdad.
    if [ ! -f "$BIN" ]; then
        # v15 (W5): durante una instalacion el binario puede estar AUSENTE
        # (el `mv` de megadaemon-core.new -> megadaemon-core deja una
        # ventana de segundos).  Medido el 2026-11-20 02:54: el supervisor
        # vio "no existe" 4 veces, gastó sus reintentos y, como el
        # instalador reinició SOLO el daemon, el movil se quedó sin daemon
        # NI supervisor => 502 (origen caido, no saturacion).
        # Ahora: si hay un binario en curso (.new / .restore) se ESPERA a
        # que termine el despliegue sin contar esto como fallo de arranque.
        if [ -f "$BIN.new" ] || [ -f "$BIN.restore" ]; then
            log "AVISO: $BIN ausente pero hay un despliegue en curso (.new/.restore): esperando 30 s"
            sleep 30
            if [ ! -f "$BIN" ]; then
                log "ERROR: $BIN sigue ausente tras esperar al despliegue"
                return 1
            fi
            log "el binario aparecio al terminar el despliegue: continuando"
        else
            log "ERROR: no existe $BIN (y no hay despliegue en curso)"
            return 1
        fi
    fi
    if [ ! -x "$BIN" ]; then
        chmod 755 "$BIN" 2>/dev/null
        if [ -x "$BIN" ]; then
            log "AVISO: $BIN no era ejecutable (permisos); chmod 755 aplicado"
        else
            log "ERROR: $BIN existe pero no se puede hacer ejecutable"
            return 1
        fi
    fi
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
    prune_caches
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
        sleep 60
    fi
done
