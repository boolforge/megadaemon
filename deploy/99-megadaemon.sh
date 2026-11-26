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

# MEDIDO (2026-11-23, movil): `/data/local/tmp` es VOLATIL: al reiniciar el
# movil desaparece el binario => el supervisor se quedaba relanzando y
# fallando con "no existe megadaemon-core" (sin daemon, sin tunel, sin
# nada).  Ahora todo vive en `/data/adb/megadaemon` (persistente, root).
PREFIX=/data/adb/megadaemon
# Si el movil tiene el binario en la ruta antigua (instalado a mano), se
# migra en el primer arranque.
LEGACY_PREFIX=/data/local/tmp/megadaemon
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
fi
# Los defaults se exportan SIEMPRE, no sólo cuando existe el env file.
# BUG MEDIDO (2026-11-21): con el export dentro del `if`, si el móvil no
# tiene $ENV_FILE el daemon caía al default DEL CRATE RUST
# (/data/user/0/com.agy/files/megadaemon/state), que en el móvil NO existe
# => la key, tunnel.url, channel.json, peers.json, audit.ndjson y
# board.sqlite se dispersaban => 403 global y /channel.json sin 'quick'.
# Sólo las variables NO secretas se exportan a los hijos: la API key se
# exporta más abajo, sólo al daemon (igual que antes).
export MEGADAEMON_STATE_DIR="${MEGADAEMON_STATE_DIR:-$PREFIX/state}"
# MEDIDO (2026-11-23): el daemon tomaba la ruta del binario de una
# constante hardcodeada que apuntaba a la ruta VOLATIL /data/local/tmp =>
# el auto-actualizador no encontraba el binario ("no se pudo calcular el
# sha256") y el movil NUNCA se actualizaba.  Aqui se exportan el prefijo y
# la ruta real: una sola fuente de verdad para el supervisor, el daemon,
# el auto-actualizador y el reparador de entorno.
export MEGADAEMON_PREFIX="$PREFIX"
export MEGADAEMON_BIN="$PREFIX/megadaemon-core"
export MEGADAEMON_LISTEN="${MEGADAEMON_LISTEN:-127.0.0.1:9090}"
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
    # MEDIDO (2026-11-23): se servia cockpit_template.html (la V1: 7 vistas,
    # SIN nodos, SIN logs, SIN configuracion) => "la web UI es lo peor del
    # proyecto".  Ahora se materializa la **V2** (cockpit_v2.html: 12
    # vistas) y la V1 se queda disponible en `/legacy`.
    #
    # La V2 va embebida en el binario, asi que si no hay ninguna en disco se
    # sirve la embebida: la UI nunca cae en el placeholder.
    if [ -f "$PREFIX/web/cockpit_v2.html" ]; then
        export MEGADAEMON_WEB_TEMPLATE_V2="$PREFIX/web/cockpit_v2.html"
    else
        mkdir -p "$PREFIX/web" 2>/dev/null
        for src in \
            /data/user/0/com.agy/files/megadaemon/web/cockpit_v2.html \
            /sdcard/megadaemon/web/cockpit_v2.html \
            /sdcard/Download/megadaemon/web/cockpit_v2.html
        do
            if [ -f "$src" ]; then
                cp -f "$src" "$PREFIX/web/cockpit_v2.html" 2>/dev/null && {
                    export MEGADAEMON_WEB_TEMPLATE_V2="$PREFIX/web/cockpit_v2.html"
                    log "plantilla web V2 copiada desde $src"
                    break
                }
            fi
        done
    fi
    # La V1 (legacy) se deja donde estaba: el daemon la sirve en /legacy.
    if [ -f "$PREFIX/web/cockpit_template.html" ]; then
        export MEGADAEMON_WEB_TEMPLATE="$PREFIX/web/cockpit_template.html"
    fi
    [ -n "${MEGADAEMON_WEB_TEMPLATE_V2:-}" ] && return 0
    log "AVISO: no hay cockpit_v2.html en disco; se servira la V2 embebida"
    return 0
}

# --- deteccion de puertos alias ya servidos ------------------------------
# El binario nuevo (v13.4+) escucha el cockpit en 0.0.0.0 y ademas en los
# puertos alias 3000/3030/8090/8080.  Si el daemon ya los sirve, el puente
# web sobra y arrancarlo seria pelear por el bind (EADDRINUSE), lo que puede
# tumbar al daemon nuevo al arrancar.  Se lee /proc/net/tcp{,6}: sin forks y
# sin procesos extra (el movil va justo de RAM).
#   80=0050  3000=0BB8  3030=0BC6  8080=1F90  8090=1F9A   (hex)
alias_ports_listening() {
    for hp in 0050 0BB8 0BC6 1F90 1F9A; do
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

# Puertos alias que el puente puede coger.  El daemon nuevo sirve 3000/3030/
# 8090 (y 9090); el puente solo debe quedarse con los que el daemon NO
# sirve (80 y 8080).  Menos puertos = menos hilos = menos RAM en el movil.
BRIDGE_CANDIDATES="80 3000 3030 8090 8080"

# Puertos alias que NADIE esta sirviendo todavia: son los que el puente debe
# coger.  Si el daemon los sirve ya, se los cede (si no, EADDRINUSE y el
# daemon nuevo se cae al arrancar).
alias_ports_free() {
    out=""
    for p in $BRIDGE_CANDIDATES; do
        hp=$(printf '%04X' "$p")
        if ! grep -qiE ":$hp [0-9a-f]{8}:[0-9a-f]{4} 0A" /proc/net/tcp /proc/net/tcp6 2>/dev/null; then
            if [ -z "$out" ]; then out="$p"; else out="$out,$p"; fi
        fi
    done
    echo "$out"
}

ensure_web_bridge() {
    [ -f "$BRIDGE" ] || return 0
    if pgrep -f "web_bridge.py" > /dev/null 2>&1; then
        return 0
    fi
    # Si el daemon YA escucha en los puertos alias (binario v13.4+: 3000/3030/
    # 8090), el puente sobra y ademas choca con el (EADDRINUSE).  Se comprueba
    # con ss y, si no existe, con un connect() de toybox.
    if ss -ltn 2>/dev/null | grep -q ":3000"; then
        return 0
    fi
    if printf '' | timeout 1 toybox nc -w 1 127.0.0.1 3000 > /dev/null 2>&1; then
        return 0
    fi
    # El daemon ya sirve los alias (binario nuevo): el puente sobra y solo
    # provocaria un EADDRINUSE que tumba al daemon.
    if alias_ports_listening; then
        free=$(alias_ports_free)
        if [ -z "$free" ]; then
            log "el daemon sirve TODOS los puertos alias: no hace falta puente web (ahorra RAM)"
            return 0
        fi
        # La lista del puente va separada por COMAS (ver read_ports()).
        export MEGADAEMON_BRIDGE_PORTS="$free"
        log "el daemon ya escucha los puertos alias: el puente solo cogera $MEGADAEMON_BRIDGE_PORTS"
    fi
    py="$PY_BIN"
    if [ -z "$py" ]; then
        for cand in /data/data/com.termux/files/usr/bin/python3 \
                    /data/data/com.agy/files/usr/bin/python3 \
                    /system/bin/python3; do
            [ -x "$cand" ] && { py="$cand"; break; }
        done
    fi
    if [ -z "$py" ]; then
        log "AVISO: sin python3 no se puede arrancar el puente web ($BRIDGE)"
        return 1
    fi
    # Keeper: si el puente muere (lo mata una shell de agente, un OOM o un
    # reinicio), vuelve a levantarse en 5 s.  Antes tardaba hasta 60 s y el
    # usuario veia ERR_CONNECTION_REFUSED en el navegador.
    setsid nohup sh -c "while true; do '$py' '$BRIDGE' >>'$PREFIX/bridge.log' 2>&1; sleep 5; done" >/dev/null 2>&1 </dev/null &
    log "puente web arrancado (con keeper): 3000/3030/8090/8080/80 -> 127.0.0.1:$PORT (python3: $py)"
    return 0
}

# --- túneles públicos (cloudflared) ---------------------------------------
# La URL de un túnel rápido de trycloudflare es EFÍMERA: cambia en cada
# arranque.  Si el móvil se queda sin túnel, TODOS los que tienen la URL
# hardcodeada caen a 530.  Por eso:
#   * el supervisor mantiene el túnel vivo (watchdog),
#   * la URL vigente se escribe SIEMPRE en $PREFIX/tunnel.url (y en
#     /sdcard/megadaemon.url), que es la fuente de verdad,
#   * el protocolo es el de por defecto (QUIC): MEDIDO en este movil, el
#     tunel legacy 8765 lleva 9 h con QUIC y 0 cortes, mientras que con
#     --protocol http2 el de 9090 perdia la conexion con el edge cada
#     ~50 s (=> 530 para la flota).  Los fallos de QUIC que se vieron eran
#     del incidente de OOM (184 MB libres), no de la red.
# CLOUDFLARED: PRIMERO el prefijo propio (independiente de Termux).
#
# MEDIDO el 2026-11-21: `cloudflared` vivia en el directorio PRIVADO de
# Termux.  Termux es una app: cuando el usuario la cierra, la matan; cuando
# la reinstala, se pierde.  Con el tunel dentro de Termux, el movil se
# queda sin canal PUBLICO en cuanto se toca Termux.
#
# Ahora el binario se COPIA al prefijo (root lo puede leer) y a partir de ahi
# ya no depende de ninguna app.  Termux queda como ultimo recurso.
CLOUDFLARED=""
for cand in "$PREFIX/cloudflared" \
            /data/local/tmp/megadaemon/cloudflared; do
    [ -x "$cand" ] && { CLOUDFLARED="$cand"; break; }
done
# MEDIDO (2026-11-25, movil): si el cloudflared estaba en la ruta LEGACY
# (/data/local/tmp/megadaemon, VOLATIL) se usaba desde ahi y NO se migraba
# => al reiniciar el movil, /data/local/tmp se limpiava y el movil se
# quedaba SIN TUNEL (medido: el proceso corria desde
# /data/local/tmp/megadaemon/cloudflared).  Ahora se MIGRABA al prefijo
# persistente nada mas encontrarlo.
if [ -n "$CLOUDFLARED" ] && [ "$CLOUDFLARED" != "$PREFIX/cloudflared" ]; then
    if cp -f "$CLOUDFLARED" "$PREFIX/cloudflared.new" 2>/dev/null; then
        chmod 755 "$PREFIX/cloudflared.new" 2>/dev/null
        if mv -f "$PREFIX/cloudflared.new" "$PREFIX/cloudflared" 2>/dev/null; then
            log "cloudflared MIGRADO al prefijo persistente (estaba en $CLOUDFLARED)"
            CLOUDFLARED="$PREFIX/cloudflared"
        else
            rm -f "$PREFIX/cloudflared.new" 2>/dev/null
        fi
    fi
fi
if [ -z "$CLOUDFLARED" ]; then
    for cand in /data/data/com.termux/files/usr/bin/cloudflared \
                /data/data/com.agy/files/usr/bin/cloudflared; do
        if [ -x "$cand" ]; then
            # MIGRACION: se copia al prefijo y se usa la copia.  A partir de
            # aqui, aunque Termux muera, el tunel sigue.
            if cp -f "$cand" "$PREFIX/cloudflared.new" 2>/dev/null; then
                chmod 755 "$PREFIX/cloudflared.new" 2>/dev/null
                mv -f "$PREFIX/cloudflared.new" "$PREFIX/cloudflared" 2>/dev/null \
                    && log "cloudflared MIGRADO al prefijo (ya no depende de Termux)" \
                    || rm -f "$PREFIX/cloudflared.new" 2>/dev/null
            fi
            [ -x "$PREFIX/cloudflared" ] && { CLOUDFLARED="$PREFIX/cloudflared"; break; }
            CLOUDFLARED="$cand"
            break
        fi
    done
fi
TUNNEL_CURL=""
for cand in /data/data/com.termux/files/usr/bin/curl \
            /data/data/com.agy/files/usr/bin/curl; do
    [ -x "$cand" ] && { TUNNEL_CURL="$cand"; break; }
done
WEB_TUNNEL_LOG=$PREFIX/tunnel.log
WEB_TUNNEL_URL_FILE=$PREFIX/tunnel.url
MCP_TUNNEL_LOG=$PREFIX/mcp_tunnel.log
MCP_TUNNEL_URL_FILE=$PREFIX/mcp_tunnel.url

tunnel_url_from_log() {
    # MEDIDO (2026-11-23, movil): el grep cogia "https://api.trycloudflare.com"
    # (la URL de la API que el propio cloudflared escribe en su log) => el
    # fichero tunnel.url guardaba una URL que NO es el tunel => el movil se
    # quedaba "sin URL publica" y la Web UI era inalcanzable desde fuera.
    # Ahora: solo se aceptan hosts de TUNEL RAPIDO y se excluye la API.
    grep -ao 'https://[a-z0-9-]*\.trycloudflare\.com' "$1" 2>/dev/null \
        | grep -v '^https://api\.' \
        | grep -v '^https://developers\.' \
        | tail -1
}

# URL del log principal y, si no aparece, del log de respaldo (el túnel
# legacy lo escribe en el home de Termux, no en $PREFIX).
tunnel_url_any() {
    u=$(tunnel_url_from_log "$1")
    [ -n "$u" ] || u=$(tunnel_url_from_log "$2")
    echo "$u"
}

# El proceso puede estar vivo y el túnel CAÍDO (530): el proceso solo no
# basta.  Se comprueba el túnel de verdad (hairpin desde el propio móvil).
# MUY importante: en este móvil la red hacia Cloudflare es INESTABLE
# (timeouts de DNS y de UDP).  Por eso SOLO se reinicia el túnel cuando la
# respuesta es un error EXPLICITO de túnel (530/502/503/504): un timeout o
# un DNS fallido NO se tocan (si no, cada micro-corte de red cambiaría la
# URL y dejaría a toda la flota con un 530).
tunnel_status() {
    url=$1
    [ -n "$url" ] || { echo "sin-url"; return; }
    [ -x "$TUNNEL_CURL" ] || { echo "sin-curl"; return; }
    code=$("$TUNNEL_CURL" -s -m 15 -o /dev/null -w '%{http_code}' "$url/health" 2>/dev/null)
    [ -n "$code" ] || code="000"
    echo "$code"
}

# Un tunel CAIDO devuelve 530.  Un 502/503/504 significa otra cosa: el
# tunel esta bien pero el ORIGEN (el daemon) no responde -> no hay que
# tocar el tunel (el supervisor ya relanza el daemon).  Reiniciar el tunel
# por eso cambia la URL publica y deja a toda la flota con un 530.
#
# MEDIDO (2026-11-25, movil): el tunel estaba MUERTO y nadie lo relanzaba.
# El proceso de cloudflared seguia vivo (pid 28610) pero su host ya no
# resolvia (NXDOMAIN) => `curl` devolvia 000 => la funcion decia "esta
# vivo" => el tunel NUNCA se reiniciaba => el movil era inalcanzable desde
# fuera hasta que alguien lo relanzase a mano.
#
# 000 = no hay ni edge ni origen (DNS/TCP caidos) => el tunel esta MUERTO.
# El reinicio sigue exigiendo DOS comprobaciones seguidas (tunnel_strikes),
# asi que un fallo puntual de red no cambia la URL publica.
tunnel_is_dead() {
    case "$1" in
        000|530|521|522) return 0 ;;
        *) return 1 ;;
    esac
}

# Conexiones activas con el edge segun las metricas de cloudflared.  Es la
# SENAL FIABLE: el proceso puede seguir vivo sin ninguna conexion (eso es
# exactamente lo que pasaba y devolvia 530 a toda la flota).  El puerto de
# metricas lo anuncia cloudflared en su propio log.
tunnel_ha_connections() {
    t_mlog=$1
    t_mport=$(grep -ao "Starting metrics server on 127.0.0.1:[0-9]*" "$t_mlog" 2>/dev/null | tail -1 | cut -d: -f2)
    [ -n "$t_mport" ] || return 1
    [ -x "$TUNNEL_CURL" ] || return 1
    t_mval=$("$TUNNEL_CURL" -s -m 6 "http://127.0.0.1:$t_mport/metrics" 2>/dev/null | awk "/^cloudflared_tunnel_ha_connections /{print \$2}")
    [ -n "$t_mval" ] || return 1
    echo "$t_mval"
    return 0
}

# ensure_tunnel <puerto_origen> <log> <fichero_url> <etiqueta> <dedup>
tunnel_strikes=0

# ─────────────────────────────────────────────────────────────────────────────
# Túnel MCP LEGACY (puerto 8765): DESACTIVADO por defecto.
#
# MEDIDO (2026-11-23, S21): el MCP ya lo sirve el daemon en el MISMO puerto
# que el cockpit ($PORT).  El puerto 8765 no tiene NADIE escuchando => ese
# túnel no puede funcionar nunca, y sin embargo el supervisor lo relanzaba
# cada ~30 s: el log se llenaba de "la URL nueva NO responde" y el móvil
# estaba en clase "thermal" (53-62 C) por el consumo de ese bucle.
#
# Por defecto se APAGA (y se mata el que hubiera), y se puede volver a
# encender con MD_MCP_LEGACY_TUNNEL=1 (por si algún cliente antiguo sigue
# apuntando a 8765).
# ─────────────────────────────────────────────────────────────────────────────
mcp_legacy_enabled() {
    [ "${MD_MCP_LEGACY_TUNNEL:-0}" = "1" ] || return 1
    # MEDIDO (2026-11-25, movil): aunque MD_MCP_LEGACY_TUNNEL estuviera a 1,
    # el tunel para el 8765 NO puede funcionar (el daemon sirve el MCP en el
    # MISMO puerto que el cockpit, $PORT) y el supervisor lo relanzaba cada
    # ~30 s: cloudflared escribiendo logs, quic, temperatura y bateria.
    # El movil estaba al 30 % de bateria descargando por esto.
    #
    # Aqui se COMPRUEBA que el 8765 lo sirva de verdad alguien (esta en
    # LISTEN).  Si no hay nadie escuchando, el tunel se apaga solo, aunque la
    # variable diga 1.
    hp=$(printf '%04X' 8765)
    if grep -qiE ":$hp [0-9a-f]{8}:[0-9a-f]{4} 0A" /proc/net/tcp /proc/net/tcp6 2>/dev/null; then
        return 0
    fi
    return 1
}

mcp_legacy_stop() {
    pids=$(pgrep -f "cloudflared tunnel .*127.0.0.1:8765" 2>/dev/null)
    for p in $pids; do
        kill "$p" 2>/dev/null && log "tunel mcp-legacy: instance obsoleta PID $p eliminada (8765 ya no lo sirve el daemon)"
    done
    # La URL que se dejaba era la de la API de cloudflared
    # (https://api.trycloudflare.com): no es un túnel y confundía a la flota.
    _ml_url=$(cat "$MCP_TUNNEL_URL_FILE" 2>/dev/null)
    case "$_ml_url" in
        https://api.*|https://developers.*)
            # Solo se avisa si HABIA una URL obsoleta: si el fichero ya
            # estaba vacio no se repite el aviso en cada vuelta (ruido).
            : > "$MCP_TUNNEL_URL_FILE" 2>/dev/null
            log "tunel mcp-legacy: URL obsoleta limpiada de $MCP_TUNNEL_URL_FILE"
            ;;
    esac
}

ensure_mcp_legacy_tunnel() {
    if mcp_legacy_enabled; then
        ensure_tunnel 8765 "$MCP_TUNNEL_LOG" "$MCP_TUNNEL_URL_FILE" "mcp-legacy" 0 \
            /data/data/com.termux/files/home/android-mcp/cloudflared.log
    else
        mcp_legacy_stop
    fi
}

ensure_tunnel() {
    t_port=$1; t_log=$2; t_urlfile=$3; t_tag=$4; t_dedup=${5:-1}; t_fallback=${6:-}
    [ -x "$CLOUDFLARED" ] || return 0
    t_url=$(cat "$t_urlfile" 2>/dev/null)
    t_pids=$(pgrep -f "cloudflared tunnel .*127.0.0.1:$t_port" 2>/dev/null)
    t_n=0
    for p in $t_pids; do t_n=$((t_n + 1)); done

    if [ "$t_n" -eq 0 ]; then
        log "tunel $t_tag caido: arrancando cloudflared (quic) -> 127.0.0.1:$t_port"
        setsid nohup "$CLOUDFLARED" tunnel --no-autoupdate \
            --url "http://127.0.0.1:$t_port" >>"$t_log" 2>&1 </dev/null &
        sleep 6
    elif [ "$t_dedup" -eq 1 ] && [ "$t_n" -gt 1 ]; then
        # Varias instancias para el MISMO origen: sobra una y cada una
        # consume RAM en un movil que va justo.  Se queda la mas antigua.
        keep=$(echo "$t_pids" | head -1)
        for p in $t_pids; do
            [ "$p" = "$keep" ] && continue
            kill "$p" 2>/dev/null && log "tunel $t_tag: instance duplicada PID $p eliminada"
        done
    fi

    # La URL vigente: del log si acaba de arrancar, si no del fichero.
    # Tras arrancar se espera a que cloudflared la publique (tarda ~10 s).
    t_wait=0
    while [ "$t_wait" -lt 6 ]; do
        t_new=$(tunnel_url_any "$t_log" "$t_fallback")
        [ -n "$t_new" ] && break
        sleep 4
        t_wait=$((t_wait + 1))
    done
    if [ -n "$t_new" ] && [ "$t_new" != "$t_url" ]; then
        # MEDIDO (2026-11-23): se publicaba la URL sin comprobar que el
        # tunel responde => un host equivocado (o un tunel recien salido)
        # se guardaba como "URL vigente" y la flota se quedaba fuera.
        # Ahora se sondea: si responde (200/401/403 = el origen esta vivo),
        # se publica; si no, se conserva la anterior y se dice en el log.
        _tc=$(tunnel_status "$t_new")
        case "$_tc" in
            200|401|403)
                printf '%s\n' "$t_new" > "$t_urlfile" 2>/dev/null
                log "tunel $t_tag URL VIGENTE: $t_new  (guardada en $t_urlfile, sondeada: HTTP $_tc)"
                ;;
            *)
                log "tunel $t_tag: la URL nueva ($t_new) NO responde (HTTP $_tc): se conserva la anterior"
                ;;
        esac
    fi
    t_url=$(cat "$t_urlfile" 2>/dev/null)

    # Vivo el proceso pero túnel caido (530 explicito): se reinicia, pero
    # solo tras DOS comprobaciones seguidas: con la red del movil
    # inestable, un unico fallo puede ser un falso positivo y reiniciar el
    # túnel cambia la URL publica (y deja a la flota con un 530).
    if [ "$t_n" -gt 0 ]; then
        t_code=$(tunnel_status "$t_url")
        t_ha=$(tunnel_ha_connections "$t_log")
        if [ -n "$t_ha" ] && [ "$t_ha" -gt 0 ] 2>/dev/null; then
            # Hay conexion con el edge: el tunel esta VIVO.  Un 530 aqui
            # puede ser un falso positivo del propio hairpin (el tunel
            # acaba de arrancar y aun no se propagó) -> no se toca nada.
            tunnel_strikes=0
            t_code="edge-ok"
        elif [ -n "$t_ha" ]; then
            # 0 conexiones con el edge: el tunel esta CAIDO de verdad.
            t_code="0-edge"
        fi
        if tunnel_is_dead "$t_code" || [ "$t_code" = "0-edge" ]; then
            tunnel_strikes=$((tunnel_strikes + 1))
            log "tunel $t_tag sin conexion (HTTP $t_code) [$tunnel_strikes/2]"
            if [ "$tunnel_strikes" -ge 2 ]; then
                tunnel_strikes=0
                log "tunel $t_tag caido de verdad: reiniciando"
                for p in $t_pids; do kill "$p" 2>/dev/null; done
                sleep 2
                setsid nohup "$CLOUDFLARED" tunnel --no-autoupdate \
                    --url "http://127.0.0.1:$t_port" >>"$t_log" 2>&1 </dev/null &
                sleep 8
                t_new=$(tunnel_url_any "$t_log" "$t_fallback")
                if [ -n "$t_new" ]; then
                    printf '%s\n' "$t_new" > "$t_urlfile" 2>/dev/null
                    log "tunel $t_tag URL VIGENTE: $t_new  (guardada en $t_urlfile)"
                fi
            fi
        else
            [ "$tunnel_strikes" -gt 0 ] && log "tunel $t_tag recuperado (HTTP $t_code)"
            tunnel_strikes=0
        fi
    fi
}

# --- puente con Antigravity (cliente MCP) ---------------------------------
# Antigravity (paquete com.agy) es un CLIENTE MCP: su config
# .gemini/config/mcp_config.json puede apuntar a un servidor Streamable HTTP
# con cabeceras.  El daemon expone POST /mcp y acepta el TOKEN DE SESION
# (X-Seere-Auth-Token); la API key sola NO abre /mcp (medido: 403).
#
# El token de sesion se materializa al arrancar el daemon => en cada
# reinicio el valor que hay en la config de Antigravity queda caduco y el
# puente se rompe en silencio.  Por eso, tras cada arranque del daemon, se
# re-sincroniza la config con el token vigente.  Es IDEMPOTENTE: no se
# duplica nada, se reescribe solo la cabecera del servidor HTTP local, y
# se respetan el resto de la config (providers, otros servidores).
ANTIGRAVITY_PKG=com.agy
ANTIGRAVITY_CFG=/data/user/0/$ANTIGRAVITY_PKG/files/.gemini/config/mcp_config.json

sync_antigravity_bridge() {
    [ -f "$ANTIGRAVITY_CFG" ] || return 0
    [ -x "$PY_BIN" ] || return 0
    tok=$(cat "$PREFIX/state/cockpit_token" 2>/dev/null)
    [ -n "$tok" ] || return 0
    # Solo se lanza python cuando puede hacer falta: si el token cambio O si
    # el fichero de config se modifico por fuera (otra herramienta, el IDE,
    # un despliegue).  Asi el movil no arrastra un python por minuto y el
    # puente se autorepara igualmente.
    cur=$(printf "%s" "$tok" | sha256sum 2>/dev/null | cut -c1-16)
    prev=$(cat "$PREFIX/state/agy_bridge.sha16" 2>/dev/null)
    cfg_mtime=$(stat -c %Y "$ANTIGRAVITY_CFG" 2>/dev/null)
    prev_mtime=$(cat "$PREFIX/state/agy_bridge.mtime" 2>/dev/null)
    if [ -n "$cur" ] && [ "$cur" = "$prev" ] && [ -n "$cfg_mtime" ] && [ "$cfg_mtime" = "$prev_mtime" ]; then
        return 0
    fi
    "$PY_BIN" - "$ANTIGRAVITY_CFG" "$tok" <<'PYEOF' 2>/dev/null
import json, os, sys
cfg, tok = sys.argv[1], sys.argv[2]
try:
    with open(cfg, "r", encoding="utf-8") as f:
        data = json.load(f)
except Exception:
    sys.exit(0)
servers = data.get("mcpServers")
if not isinstance(servers, dict):
    sys.exit(0)
changed = False
for name, spec in servers.items():
    if not isinstance(spec, dict):
        continue
    url = str(spec.get("url", ""))
    if "127.0.0.1:9090/mcp" not in url and "localhost:9090/mcp" not in url:
        continue
    headers = spec.setdefault("headers", {})
    if headers.get("X-Seere-Auth-Token") != tok:
        headers["X-Seere-Auth-Token"] = tok
        changed = True
if not changed:
    # El token ya estaba bien, pero el fichero puede tener el modo o el
    # propietario equivocados (lo escribe root): la app debe poder leerlo
    # y gestionarlo.
    try:
        st = os.stat("/data/user/0/com.agy")
        os.chown(cfg, st.st_uid, st.st_gid)
    except Exception:
        pass
    try:
        os.chmod(cfg, 0o600)
    except Exception:
        pass
    sys.exit(0)
tmp = cfg + ".tmp"
with open(tmp, "w", encoding="utf-8") as f:
    json.dump(data, f, indent=2)
    f.write("\n")
os.replace(tmp, cfg)
try:
    st = os.stat("/data/user/0/com.agy")
    os.chown(cfg, st.st_uid, st.st_gid)
except Exception:
    pass
os.chmod(cfg, 0o600)
PYEOF
    [ -n "$cur" ] && printf "%s" "$cur" > "$PREFIX/state/agy_bridge.sha16" 2>/dev/null
    stat -c %Y "$ANTIGRAVITY_CFG" 2>/dev/null > "$PREFIX/state/agy_bridge.mtime" 2>/dev/null
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
# Devuelve el codigo HTTP de la sonda (o "000" si no hubo respuesta).
probe_code() {
    local port="$1" path="$2" out code
    out=$("$TUNNEL_CURL" -s -m 8 ${PROBE_IFACE:+--interface $PROBE_IFACE} \
        -o /dev/null -w '%{http_code}' "http://127.0.0.1:$port$path" 2>/dev/null)
    code=$(echo "$out" | tr -dc '0-9')
    [ -n "$code" ] || code=000
    echo "$code"
}

probe_path() {
    local port="$1" path="$2" code
    code=$(probe_code "$port" "$path")
    # 200 = OK.  401/403 = el servidor RESPONDIO (pide credencial) => esta
    # VIVO: no es un fallo de vida.
    #
    # BUG MEDIDO (2026-11-23, movil): antes se exigia 200 y la sonda
    # usaba /api/power, que con la API key obligatoria responde 401 => el
    # watchdog contaba "el daemon no responde" 3 veces => lo reiniciaba, y
    # el proceso nuevo no encontraba los puertos alias => la Web UI
    # quedaba muerta.  Resultado medido: 1644 reinicios en el log.
    case "$code" in
        200|401|403) return 0 ;;
        *) return 1 ;;
    esac
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
        # /api/live: sonda MINIMA y PUBLICA (no exige API key) que ademas
        # toca el estado compartido => sigue detectando un mutex envenenado
        # sin necesitar credenciales.  Si el binario en curso es anterior y
        # no la tiene, responde 404 y se ignora (no es un fallo de vida).
        code=$(probe_code "$port" "/api/live")
        case "$code" in
            200|401|403) ;;
            404) ;;
            *) return 1 ;;
        esac
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
        # Muerte inmediata y ESPERA a que el puerto quede libre.
        #
        # BUG MEDIDO (2026-11-23): con `pkill` (SIGTERM) + 3 s de espera el
        # proceso anterior seguia escuchando los puertos alias => el daemon
        # nuevo no los podia enlazar ("Address already in use") => la Web UI
        # (3000/3030/8090) quedaba MUERTA.  Ahora: SIGKILL + espera real a
        # que /proc/net/tcp deje de mostrar el puerto en LISTEN.
        pkill -9 -x megadaemon-core 2>/dev/null
        i=0
        while [ $i -lt 20 ]; do
            hp=$(printf '%04X' "$port")
            if ! grep -qiE ":$hp [0-9a-f]{8}:[0-9a-f]{4} 0A" /proc/net/tcp /proc/net/tcp6 2>/dev/null; then
                break
            fi
            i=$((i + 1)); sleep 1
        done
        start_daemon
        wait_daemon_responsive "$port" || log "el daemon no respondio tras el reinicio (sigue el supervisor)"
    fi
    return 0
}

# ---------------------------------------------------------------------------
# v15 (F3): shell root PERSISTENTE para el bucle agentico.
#
# El bucle agentico NO puede abrir un 'su' por llamada: en este movil cada
# llamada crea 2+ procesos (su -mm -c sh -c ...) y la maquina va al limite
# (load 17,65, 876 procesos) => un bucle cerrado tumba el movil.  Con UN
# shell vivo por socket unix, las llamadas cuestan 0 procesos.
#
# El socket va 0600 (acceso root) y las acciones destructivas exigen
# 'confirm: true' explicito (politica del veto de la voz).
#
# BUG MEDIDO (2026-11-21): esta funcion usaba $P, que NO existe en este
# script (la variable es $PREFIX).  Con $P vacio => socket en /state/... y
# el test `[ -x "$P/md-shell-serve" ]` era SIEMPRE falso => el shell root
# persistente NUNCA se arranco => el bucle agentico seguia abriendo un 'su'
# por llamada (el incidente de saturacion que esta funcion evita).
SHELL_SOCK="$PREFIX/state/shell.sock"
ensure_shell_service() {
    [ -x "$PREFIX/md-shell-serve" ] || return 0
    if pgrep -f "md-shell-serve" > /dev/null 2>&1; then
        return 0
    fi
    mkdir -p "$PREFIX/state" 2>/dev/null
    log "F3: arrancando el shell root persistente (socket $SHELL_SOCK)"
    setsid sh -c "su -mm -c '$PREFIX/md-shell-serve $SHELL_SOCK'" > /dev/null 2>&1 < /dev/null &
}

start_daemon() {
    # v15: si el binario existe pero NO es ejecutable (un `mv` sin `chmod`
    # lo deja en 644), el daemon no arranca nunca y el supervisor dice
    # "no existe", que manda el diagnostico por donde no es.  Ahora se
    # AUTO-REPARA y el log dice la verdad.
    # Migracion: si el binario esta en la ruta VOLATIL antigua (instalado
    # a mano) y el prefijo nuevo esta vacio, se copia (el movil se reinicio
    # y se llevo el binario).
    if [ ! -f "$BIN" ] && [ -f "$LEGACY_PREFIX/megadaemon-core" ]; then
        mkdir -p "$PREFIX" 2>/dev/null
        cp -f "$LEGACY_PREFIX/megadaemon-core" "$BIN.new" 2>/dev/null && \
            chmod 755 "$BIN.new" 2>/dev/null && mv -f "$BIN.new" "$BIN" 2>/dev/null && \
            log "migrado el binario de $LEGACY_PREFIX a $PREFIX (ruta persistente)"
    fi
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
    # MEDIDO (2026-11-25): el log de texto se trunca a 46 lineas en el
    # diagnostico y no se veía si el daemon se reiniciaba en bucle.  Este
    # registro es dedicado y lo lee el diagnostico.
    # MEDIDO (2026-11-25): con el sha16 del binario se sabe de un vistazo
    # que CODIGO esta en servicio (el build puede estar en disco y el
    # proceso con otro: asi se detecto el desfase de 7 horas).
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] arrancado PID $pid puerto=$PORT sha16=$(sha256sum "$BIN" 2>/dev/null | cut -c1-16)" \
        >> "$PREFIX/reinicios.log" 2>/dev/null

    # --- lmkd / OOM killer: el daemon NO debe ser la primera victima -----
    # MEDIDO (2026-11-23): el movil se caia de forma DETERMINISTA porque
    # lmkd mataba al daemon (el que mas RAM usa) y con el caia el tunel
    # (502 = origen caido) => la flota entera se caia.
    # `oom_score_adj` negativo lo deja como la ULTIMA victima: el kernel
    # mata antes a las apps del usuario que al daemon.
    echo -800 > "/proc/$pid/oom_score_adj" 2>/dev/null \
        && log "daemon con oom_score_adj=-800 (lmkd lo mata el ultimo)"
    # El supervisor tampoco debe morir por lo mismo.
    echo -900 > "/proc/$$/oom_score_adj" 2>/dev/null
    # glibc (el allocator de Rust) retiene la memoria liberada en su cache:
    # con estos limites devuelve las paginas al SO antes de que lmkd mire.
    export MALLOC_ARENA_MAX=2
    export MALLOC_TRIM_THRESHOLD_=131072
    export MALLOC_MMAP_THRESHOLD_=131072

    # ¿Sigue vivo a los 3 s?  Si muere inmediatamente no es un fallo de red:
    # es un fallo de arranque (binario roto, plantilla que revienta, etc.).
    sleep 3
    if ! kill -0 "$pid" 2>/dev/null; then
        fast_fails=$((fast_fails + 1))
        log "ERROR: megadaemon-core ha muerto al arrancar (fallo rapido #$fast_fails/$MAX_FAST_FAILS)"
        log "----- últimas líneas del log -----"
        tail -n 25 "$LOG" 2>/dev/null | while IFS= read -r l; do log "  | $l"; done
        log "-------------------------------"
        write_state "FALLO_ARRANQUE: el binario muere al arrancar (fallo #$fast_fails/$MAX_FAST_FAILS)"
        # v15 · AUTO-ROLLBACK: antes, tras MAX_FAST_FAILS el supervisor hacia
        # `exit 0` => el movil se quedaba SIN daemon Y SIN supervisor y NADIE
        # volvia a lanzarlo (el canal del enjambre moria para siempre).
        # Ahora, si hay un binario anterior conocido-bueno, se restaura y se
        # sigue; el supervisor NUNCA se rinde (ver el `rc -eq 2` del bucle).
        if [ "$fast_fails" -ge "$MAX_FAST_FAILS" ] && [ -f "$PREV_BIN" ]; then
            log "AUTO-ROLLBACK: restaurando el binario anterior ($PREV_BIN)"
            cp -a "$BIN" "$BIN.bad-$(date +%s)" 2>/dev/null || true
            # MEDIDO 2026-11-25 (auto-actualizacion SIN bucles): al restaurar
            # el binario anterior, el auto-actualizador volveria a instalar el
            # MISMO asset (que es el que acaba de morir al arrancar) => bucle
            # infinito de installs y reinicios.  Se apunta el SHA-256 del
            # binario que ha fallado y el auto-actualizador lo salta.
            _malo=$(sha256sum "$BIN" 2>/dev/null | cut -d' ' -f1)
            if [ -n "$_malo" ]; then
                mkdir -p "$MEGADAEMON_STATE_DIR" 2>/dev/null
                echo "$_malo" > "$MEGADAEMON_STATE_DIR/binario_malo.sha256" 2>/dev/null && \
                    log "binario marcado como malo (sha16 ${_malo:0:16}): el auto-actualizador lo saltara"
            fi
            # MEDIDO el 2026-11-21: estos `.bad-*` se acumulaban SIN
            # limite (12 ficheros, 104 MB en el movil).  El rollback solo
            # necesita el ULTIMO: se podan todos menos el mas reciente.
            _nbad=$(ls -1t "$BIN".bad-* 2>/dev/null | tail -n +3)
            [ -n "$_nbad" ] && echo "$_nbad" | xargs rm -f 2>/dev/null
            cp -a "$PREV_BIN" "$BIN.restore" 2>/dev/null || true
            if [ -f "$BIN.restore" ]; then
                mv "$BIN.restore" "$BIN"
                log "AUTO-ROLLBACK: binario restaurado (sha16 $(sha256sum "$BIN" 2>/dev/null | cut -c1-16))"
                write_state "AUTO-ROLLBACK: binario restaurado; reintentando"
                fast_fails=0
                return 1
            fi
            log "AUTO-ROLLBACK: no se pudo restaurar $PREV_BIN"
        fi
        if [ "$fast_fails" -ge "$MAX_FAST_FAILS" ]; then
            write_state "FALLO_CODIGO: el binario muere al arrancar; se reintentara cada 5 min"
            log "ERROR: $MAX_FAST_FAILS fallos de arranque seguidos => el supervisor NO se rinde (reintenta cada 5 min)."
            log "       Estado en $STATE_FILE ; log en $LOG ; URL en $URL_FILE"
            return 2
        fi
        write_state "FALLO_ARRANQUE: reintentando (fallo #$fast_fails/$MAX_FAST_FAILS)"
        return 1
    fi
    fast_fails=0
    ensure_web_bridge
    sync_antigravity_bridge
    write_state "OK: cockpit vivo en http://localhost:$PORT/"
    write_url
    log "cockpit verificado vivo -> http://localhost:$PORT/ (URL guardada en $URL_FILE)"
    return 0
}


# --- v15: política de energía + guardas de RAM/disco ------------------------
#
# UMBRALES (medidos en este SM-G998B el 2026-11-20; configurables por entorno
# para no tener que recompilar el daemon en el móvil):
#   carga 80 % | aviso térmico 45 °C | crítico 55 °C | techo duro 80 °C
#   batería < 20 % => SELLAR | brillo máx 60 | suelo de RAM 512 MB
#   suelo de disco 1 GB | RSS máx del daemon 200 MB | se guardan 2 instantáneas
#
# NOTA HONESTA: este kernel NO expone `charge_control_end_threshold`
# (comprobado el 2026-11-20), así que el límite de carga NO se puede forzar
# por sysfs.  Se registra como NO SOPORTADO en vez de fingir que se aplica.
ENERGY_CHARGE_LIMIT=${MEGADAEMON_CHARGE_LIMIT_PCT:-80}
ENERGY_THERMAL_WARN=${MEGADAEMON_THERMAL_WARN_C:-45}
ENERGY_THERMAL_CRIT=${MEGADAEMON_THERMAL_CRIT_C:-55}
ENERGY_THERMAL_HARD=${MEGADAEMON_THERMAL_HARD_C:-80}
ENERGY_SEAL_BELOW=${MEGADAEMON_SEAL_BELOW_PCT:-20}
ENERGY_BRIGHTNESS_MAX=${MEGADAEMON_BRIGHTNESS_MAX:-60}
ENERGY_RAM_FLOOR_KB=${MEGADAEMON_RAM_FLOOR_KB:-524288}
ENERGY_DISK_FLOOR_KB=${MEGADAEMON_DISK_FLOOR_KB:-1048576}
# Tope de RSS del daemon.
#
# MEDIDO (2026-11-23): el tope FIJO de 200 MB era la CAUSA de la muerte del
# móvil.  Con el IDE de Google vivo (language server ~251 MB + app ~156 MB,
# load average 19,85 y 3,9 GB libres) el daemon roza ese tope con facilidad,
# y el propio supervisor lo mata (bloque `check_daemon_responsive`:
# "el daemon usa <rss>kB (tope ...kB): lo reinicio para no empujar al OOM
# killer").  Con el móvil saturado, el relanzamiento no prosperaba: el
# origen (127.0.0.1:9090) se quedaba MUERTO mientras `cloudflared` seguía
# vivo => el túnel respondía 502 y la flota veía el móvil "caído".
#
# Ahora el tope es ADAPTATIVO: 1/8 de la RAM libre, acotado a [200 MB,
# 512 MB].  Se puede fijar con MEGADAEMON_DAEMON_RSS_MAX_KB (entorno o
# $PREFIX/megadaemon.env, que el supervisor carga al arrancar).
if [ -z "${MEGADAEMON_DAEMON_RSS_MAX_KB:-}" ]; then
  _md_mem_free_kb=$(awk '/MemAvailable/{print $2}' /proc/meminfo 2>/dev/null)
  if [ -n "$_md_mem_free_kb" ] && [ "$_md_mem_free_kb" -gt 0 ]; then
    ENERGY_DAEMON_RSS_MAX_KB=$(( _md_mem_free_kb / 8 ))
    [ "$ENERGY_DAEMON_RSS_MAX_KB" -lt 204800 ] && ENERGY_DAEMON_RSS_MAX_KB=204800
    [ "$ENERGY_DAEMON_RSS_MAX_KB" -gt 512000 ] && ENERGY_DAEMON_RSS_MAX_KB=512000
  else
    ENERGY_DAEMON_RSS_MAX_KB=204800
  fi
else
  ENERGY_DAEMON_RSS_MAX_KB="$MEGADAEMON_DAEMON_RSS_MAX_KB"
fi
ENERGY_STATE_KEEP=${MEGADAEMON_STATE_KEEP:-2}
ENERGY_STATE_FILE="$PREFIX/energy_state"
ENERGY_BRIGHT_SAVED="$PREFIX/brightness_saved"

# Interface de sondeo: 127.0.0.3 para que el watchdog tenga SU PROPIO cubo
# del rate limit (el del túnel es 127.0.0.1 y el del navegador 127.0.0.2).
# Antes, el sondeo del watchdog competía con el de los agentes remotos.
PROBE_IFACE=""

energy_battery_pct() {
    v=$(cat /sys/class/power_supply/battery/capacity 2>/dev/null | tr -dc '0-9')
    [ -n "$v" ] && echo "$v" || echo ""
}
energy_battery_status() {
    cat /sys/class/power_supply/battery/status 2>/dev/null | tr -d '\r\n'
}
energy_battery_temp_c() {
    v=$(cat /sys/class/power_supply/battery/temp 2>/dev/null | tr -dc '0-9')
    [ -n "$v" ] || return 1
    if [ "$v" -gt 1000 ]; then echo $((v / 1000)); else echo "$v"; fi
}
# Temperatura máxima del sistema (°C) entre todas las zonas térmicas.
energy_max_temp_c() {
    max=0
    for z in /sys/class/thermal/thermal_zone*/temp; do
        [ -r "$z" ] || continue
        v=$(cat "$z" 2>/dev/null | tr -dc '0-9')
        [ -n "$v" ] || continue
        [ "$v" -gt "$max" ] && max=$v
    done
    [ "$max" -gt 0 ] && echo $((max / 1000)) || echo ""
}
energy_ram_available_kb() {
    awk '/^MemAvailable:/ {print $2}' /proc/meminfo 2>/dev/null
}
energy_disk_free_kb() {
    df -k /storage/emulated/0 2>/dev/null | awk 'NR==2 {print $4}'
}
energy_daemon_rss_kb() {
    pid=$(pgrep -f "megadaemon-core cockpit" 2>/dev/null | head -1)
    [ -n "$pid" ] || return 1
    awk '/^VmRSS:/ {print $2}' "/proc/$pid/status" 2>/dev/null
}

# Clasifica el estado: normal | conserve | thermal | seal
energy_classify() {   # $1=pct $2=temp_c $3=ram_kb $4=charging
    [ -n "$1" ] || return 0
    if [ "$1" -lt "$ENERGY_SEAL_BELOW" ]; then echo seal; return 0; fi
    if [ -n "$2" ] && [ "$2" -ge "$ENERGY_THERMAL_HARD" ]; then echo seal; return 0; fi
    if [ -n "$2" ] && [ "$2" -ge "$ENERGY_THERMAL_CRIT" ]; then echo thermal; return 0; fi
    if [ -n "$2" ] && [ "$2" -ge "$ENERGY_THERMAL_WARN" ]; then echo thermal; return 0; fi
    if [ -n "$3" ] && [ "$3" -gt 0 ] && [ "$3" -lt "$ENERGY_RAM_FLOOR_KB" ]; then echo conserve; return 0; fi
    if [ "$4" != "Charging" ] && [ "$1" -lt 35 ]; then echo conserve; return 0; fi
    echo normal
}

# Aplica la política.  Cada acción se VERIFICA y se registra con números.
apply_energy_policy() {
    pct=$(energy_battery_pct)
    [ -n "$pct" ] || pct=100
    status=$(energy_battery_status)
    temp=$(energy_max_temp_c)
    btemp=$(energy_battery_temp_c)
    ram=$(energy_ram_available_kb)
    disk=$(energy_disk_free_kb)
    cls=$(energy_classify "$pct" "$temp" "$ram" "$status")
    prev=""
    [ -f "$ENERGY_STATE_FILE" ] && prev=$(cat "$ENERGY_STATE_FILE" 2>/dev/null)

    # --- acciones (sólo si la clase ha cambiado: no se martillea el sistema) ---
    if [ "$cls" != "$prev" ]; then
        case "$cls" in
            seal|thermal)
                cmd appops set com.google.android.youtube RUN_ANY_IN_BACKGROUND ignore >/dev/null 2>&1
                cmd appops set com.android.chrome RUN_ANY_IN_BACKGROUND ignore >/dev/null 2>&1
                settings put global low_power 1 >/dev/null 2>&1
                for g in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
                    [ -w "$g" ] && echo powersave > "$g" 2>/dev/null
                done
                ;;
            conserve)
                cmd appops set com.google.android.youtube RUN_ANY_IN_BACKGROUND ignore >/dev/null 2>&1
                ;;
            normal)
                settings put global low_power 0 >/dev/null 2>&1
                # restaurar el brillo que tenía el usuario, si lo guardamos
                if [ -f "$ENERGY_BRIGHT_SAVED" ]; then
                    settings put system screen_brightness "$(cat "$ENERGY_BRIGHT_SAVED" 2>/dev/null)" >/dev/null 2>&1
                    rm -f "$ENERGY_BRIGHT_SAVED"
                fi
                ;;
        esac
        echo "$cls" > "$ENERGY_STATE_FILE" 2>/dev/null
    fi

    # --- brillo (sólo si el de ahora supera el techo) ---
    if [ "$cls" != "normal" ]; then
        cap=$ENERGY_BRIGHTNESS_MAX
        [ "$cls" = "seal" ] && cap=$((ENERGY_BRIGHTNESS_MAX - 20))
        cur=$(settings get system screen_brightness 2>/dev/null | tr -dc '0-9')
        if [ -n "$cur" ] && [ "$cur" -gt "$cap" ]; then
            [ -f "$ENERGY_BRIGHT_SAVED" ] || echo "$cur" > "$ENERGY_BRIGHT_SAVED" 2>/dev/null
            settings put system screen_brightness "$cap" >/dev/null 2>&1
        fi
    fi

    # --- disco: si estamos por debajo del suelo, podamos nuestro estado ---
    if [ -n "$disk" ] && [ "$disk" -gt 0 ] && [ "$disk" -lt "$ENERGY_DISK_FLOOR_KB" ]; then
        prune_state
    fi

    # --- registro con NÚMEROS (no adjetivos) ---
    charge_lim="NO_SOPORTADO"
    [ -r /sys/class/power_supply/battery/charge_control_end_threshold ] && \
        charge_lim="leido=$(cat /sys/class/power_supply/battery/charge_control_end_threshold 2>/dev/null)"
    log "energia: clase=$cls (antes=$prev) bateria=${pct}% estado=$status temp_sistema=${temp:-?}C temp_bat=${btemp:-?}C ram_disponible=${ram:-?}kB disco_libre=${disk:-?}kB rss_daemon=$(energy_daemon_rss_kb || echo ?)kB carga80=$charge_lim brillo=$(settings get system screen_brightness 2>/dev/null | tr -dc '0-9')"
}

# Poda el estado del daemon para que NO crezca sin límite (medido: sync_snap
# 59.860 kB + state/inventory 11.170 kB con instantáneas de 9.104 kB).
prune_state() {
    kept=$ENERGY_STATE_KEEP
    for d in "$PREFIX/state/inventory" "$PREFIX/sync_snap"; do
        [ -d "$d" ] || continue
        tmp="$PREFIX/.prune.$$"
        ls -1 "$d" 2>/dev/null | sort > "$tmp"
        total=$(wc -l < "$tmp" 2>/dev/null)
        total=$(echo "$total" | tr -dc '0-9')
        [ -n "$total" ] || continue
        i=0
        while IFS= read -r f; do
            [ -n "$f" ] || continue
            i=$((i + 1))
            if [ "$i" -le "$((total - kept))" ]; then
                rm -f "$d/$f" 2>/dev/null
            fi
        done < "$tmp"
        rm -f "$tmp"
    done
}

# --- loop del supervisor --------------------------------------------------
last_update=0
# El auto-update se comprueba cada 6 h (21600 s).  Se hace FUERA del camino
# crítico: si falla, el daemon sigue sirviendo la versión anterior.
UPDATE_EVERY=${MEGADAEMON_UPDATE_EVERY:-21600}

# AUTO-ACTUALIZACION (v17).
#
# MEDIDO el 2026-11-21: esta funcion pedia `$PREFIX/auto-update.sh`, que NO
# EXISTE en el repo => el auto-update no habia hecho NUNDA nada.  Y aunque
# existiera, no podria descargar nada: el movil no tiene curl ni wget.
#
# Ahora el propio binario del daemon se actualiza (`megadaemon-core update`):
#   * `--check` -> 1 peticion pequena; codigo 10 = "hay version nueva";
#   * sin `--check` -> descarga, VERIFICA el SHA-256 e instala de forma
#     atomica (el anterior queda en .prev).
#
# Coste en reposo: UNA peticion HTTP cada UPDATE_EVERY segundos.
try_update() {
    now=$(date +%s)
    [ $((now - last_update)) -lt "$UPDATE_EVERY" ] && return 0
    last_update=$now
    [ -x "$BIN" ] || return 0
    out=$("$BIN" update --check 2>&1 | head -2)
    case "$out" in
        *actualizable*)
            log "auto-update: version nueva disponible -> instalando"
            "$BIN" update >>"$LOG" 2>&1
            rc=$?
            if [ "$rc" = "10" ]; then
                log "auto-update: version nueva INSTALADA -> reiniciando el daemon"
                # El binario nuevo se carga al reiniciar el proceso.
                pkill -f "megadaemon-core cockpit" 2>/dev/null
                sleep 3
            else
                log "auto-update: fallo (se mantiene la version actual)"
            fi
            ;;
        *actual*)
            : # ya al dia: no se hace nada (0 peticiones de peso)
            ;;
        *)
            log "auto-update: no se pudo comprobar ($out)"
            ;;
    esac
}

i=0
last_update=0
mkdir -p "$PREFIX" "$STATE" 2>/dev/null
write_state "ARRANCANDO: supervisor activo, puerto $PORT (prefijo $PREFIX)"
# Los túneles se levantan ANTES de arrancar el daemon: la URL pública (y la
# del MCP legacy) no dependen de que el cockpit esté listo, y así el túnel
# ya está cuando el daemon tarda unos segundos.
ensure_tunnel "$PORT" "$WEB_TUNNEL_LOG" "$WEB_TUNNEL_URL_FILE" "web" 1
ensure_mcp_legacy_tunnel
while true; do
    rotate
    prune_caches
    if ! pgrep -f "megadaemon-core cockpit" > /dev/null 2>&1; then
        log "megadaemon no está corriendo: relanzando (intento $i)"
        start_daemon
        rc=$?
        if [ "$rc" -eq 2 ]; then
            # v15: el supervisor NO se detiene nunca.  Antes `exit 0` dejaba el
            # movil sin daemon y sin supervisor => nadie relanzaba nunca.
            # Ahora espera 5 min y reintenta (1 intento/5 min = coste nulo en
            # un movil con 390 MB libres) y asi el canal vuelve solo cuando se
            # instale un binario bueno.
            log "fallo de codigo al arrancar: el supervisor espera 5 min y reintenta (ver $LOG y $STATE_FILE)"
            i=0
            sleep 300
        fi
        i=$((i + 1))
        # backoff: 10s, 20s, 40s ... hasta 300s
        s=$((10 * (1 << (i < 5 ? i : 5))))
        [ "$s" -gt 300 ] && s=300
        sleep "$s"
    else
        i=0
        # v15: vivo != funcional (mutex envenenado => 502 con el proceso
        # vivo).  Dos fallos seguidos de /health => se reinicia.
        probe_iface_init
        check_daemon_responsive "$PORT" || true
        apply_energy_policy
        ensure_web_bridge
        sync_antigravity_bridge
        ensure_tunnel "$PORT" "$WEB_TUNNEL_LOG" "$WEB_TUNNEL_URL_FILE" "web" 1
        ensure_mcp_legacy_tunnel
        write_url
        try_update
        # Vigilancia cada 5 s en vez de cada 60 s: si el daemon o el puente
        # mueren, se relanzan casi al instante (antes el usuario podia verse
        # ERR_CONNECTION_REFUSED durante un minuto entero).
        k=0
        while [ $k -lt 12 ]; do
            sleep 5
            k=$((k + 1))
            ensure_web_bridge
            if ! pgrep -f "megadaemon-core cockpit" > /dev/null 2>&1; then
                break
            fi
        done
        # ROBUSTEZ: vigilar RESPONSIVIDAD, no solo la EXISTENCIA del proceso.
        # Un daemon vivo pero envenenado (mutex envenenado por un panic) o
        # colgado seguia dando 502 desde el tunel mientras el watchdog, que
        # solo miraba pgrep, lo daba por bueno. Ahora se le pregunta de
        # verdad: si /health no responde, se reinicia.
        # MEDIDO (2026-11-25, movil): el probe usaba PY_BIN (el python3 de
        # TERMUX).  Si Termux esta frio, apagado o su python3 falla, el probe
        # daba "sin responder" y el supervisor MATABA un daemon SANO
        # (medido: "daemon VIVO pero NO RESPONDE: se reinicia" y el daemon
        # se relanzaba, pero /health respondia bien).  Ademas, si Termux no
        # esta instalado, el probe NUNCA podia pasar y el supervisor
        # reiniciaba el daemon sin parar.
        #
        # Ahora el probe usa HERRAMIENTAS NATIVAS del sistema (toybox wget /
        # busybox wget / curl), sin depender de ninguna app.  Si no hay
        # ninguna, se usa el /proc/net/tcp (el daemon escucha aunque /health
        # tarde): es unindicador de "hay alguien escuchando en el puerto".
        probe_health() {
            local u="http://127.0.0.1:$PORT/health"
            for cand in /system/bin/toybox /data/adb/megadaemon/busybox \
                        /data/adb/magisk/busybox /data/local/tmp/megadaemon/busybox \
                        /data/data/com.termux/files/usr/bin/curl \
                        /data/data/com.termux/files/usr/bin/wget; do
                [ -x "$cand" ] || continue
                case "$cand" in
                    */toybox) "$cand" wget -O /dev/null -T 4 "$u" >/dev/null 2>&1 && return 0 ;;
                    */busybox) "$cand" wget -q -O /dev/null -T 4 "$u" >/dev/null 2>&1 && return 0 ;;
                    */curl)   "$cand" -fsS -m 4 -o /dev/null "$u" >/dev/null 2>&1 && return 0 ;;
                    */wget)   "$cand" -q -T 4 -O /dev/null "$u" >/dev/null 2>&1 && return 0 ;;
                esac
            done
            # Sin herramientas: se mira /proc/net/tcp (puerto en LISTEN = 0A).
            local hexport
            hexport=$(printf '%04X' "$PORT")
            if [ -r /proc/net/tcp ] && grep -qi ":$hexport .*0A" /proc/net/tcp 2>/dev/null; then
                return 0
            fi
            return 1
        }
        miss=0
        i=0
        while [ $i -lt 3 ]; do
            i=$((i + 1))
            if probe_health; then
                miss=0
                break
            fi
            miss=$((miss + 1))
            log "daemon sin responder en /health (intento $miss/3)"
            sleep 2
        done
        if [ "$miss" -ge 3 ]; then
            log "daemon VIVO pero NO RESPONDE: se reinicia (antes se quedaba en 502 para siempre)"
            for p in $(pgrep -f "megadaemon-core cockpit" 2>/dev/null); do
                kill "$p" 2>/dev/null
            done
            sleep 3
        fi
    fi
done
