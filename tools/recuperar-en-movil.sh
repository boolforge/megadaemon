#!/system/bin/sh
# recuperar-en-movil.sh — RECUPERACIÓN DEL MÓVIL, EJECUTADA EN EL MÓVIL.
#
# POR QUÉ EXISTE (medido el 2026-11-25):
#   El móvil estaba encendido (Antigravity abría, con un 404 del backend) pero
#   el daemon NO corría: el tablón de Drive tenía UNA carta, del 2026-11-23
#   10:22 (latido del nodo `samsung`), y el túnel de Cloudflare había
#   caducado (NXDOMAIN).  Es decir: el móvil no se autorrepara.
#
#   Desde el sandbox no hay ADB ni túnel, así que la recuperación tiene que
#   poder hacerse **desde el propio móvil**, sin PC.
#
# QUÉ HACE (todo idempotente, nada destructivo fuera del prefijo):
#   1. Diagnóstico: ¿daemon? ¿túnel? ¿binario? ¿estado?
#   2. Relanza el daemon (cockpit 3000, gateway 9090).
#   3. Relanza el túnel de Cloudflare y ESPERA a que la URL responda.
#   4. Publica/verifica la URL y muestra el NDJSON del shim (para ver qué
#      rutas pide Antigravity).
#   5. Deja el estado en `$PREFIX/state/` para que la Web UI lo vea.
#
# USO (desde Termux o cualquier shell con root):
#   su -mm -c 'sh /data/local/tmp/recuperar-en-movil.sh'
#   su -mm -c 'sh /data/local/tmp/recuperar-en-movil.sh --diagnostico'
#
# MEDIDO 2026-11-25: `sudo` en Android NO es root; para root puntual, `su -mm -c`.

PREFIX=/data/adb/megadaemon
STATE="$PREFIX/state"
BIN="$PREFIX/megadaemon-core"
ENV_FILE="$PREFIX/megadaemon.env"
LOG=/sdcard/megadaemon.log

# MEDIDO (2026-11-25, movil): el daemon NO sirve en el 3000.  El supervisor
# (deploy/99-megadaemon.sh) lo arranca con "cockpit $PORT", y su PORT por
# defecto es **9090** (configurable con MEGADAEMON_COCKPIT_PORT en
# megadaemon.env).  Con 3000 el script arrancaba un daemon que nadie
# escuchaba y daba por muerto un cockpit que estaba vivo => falso negativo.
PORT=9090
if [ -f "$ENV_FILE" ]; then
    p=$(sed -n 's/^MEGADAEMON_COCKPIT_PORT=//p' "$ENV_FILE" 2>/dev/null | head -1 | tr -d '\r"')
    case "$p" in
        ''|*[!0-9]*) ;;
        *) PORT="$p";;
    esac
fi
GW="$PORT"

log() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] recuperar: $*" >> "$LOG" 2>/dev/null; echo "$*"; }

daemon_vivo() {
    for p in /proc/[0-9]*; do
        [ -r "$p/cmdline" ] || continue
        c=$(tr '\0' ' ' < "$p/cmdline" 2>/dev/null)
        case "$c" in *megadaemon-core*) echo "$p"; return 0;; esac
    done
    return 1
}

health() {
    # MEDIDO: /health responde sin credencial; 401/403 también significa vivo.
    code=$(toybox wget -q -O /dev/null -T 5 "http://127.0.0.1:$PORT/health" 2>/dev/null; echo $?)
    [ "$code" = "0" ] && return 0
    return 1
}

diagnostico() {
    echo "== daemon"
    if d=$(daemon_vivo); then echo "  vivo: $d ($(tr '\0' ' ' < "$d/cmdline"))"; else echo "  NO CORRE"; fi
    echo "== binario"
    if [ -x "$BIN" ]; then echo "  $BIN ($(stat -c %s "$BIN") B, sha16 $(sha256sum "$BIN" 2>/dev/null | cut -c1-16))"; else echo "  FALTA $BIN"; fi
    echo "== previous"
    if [ -f "$BIN.prev" ]; then
        echo "  hay .prev (sha16 $(sha256sum "$BIN.prev" 2>/dev/null | cut -c1-16))"
    else
        echo "  no hay .prev"
    fi
    echo "== tunel"
    [ -f "$PREFIX/tunnel.url" ] && echo "  url: $(cat "$PREFIX/tunnel.url")" || echo "  sin tunnel.url"
    echo "== estado"
    [ -f /sdcard/megadaemon.state ] && cat /sdcard/megadaemon.state
    echo "== ultimas lineas del log"
    [ -f "$LOG" ] && tail -n 15 "$LOG"
    echo "== shim (ultimas rutas pedidas)"
    if [ -f "$STATE/agyshim.ndjson" ]; then
        tail -n 12 "$STATE/agyshim.ndjson" | cut -c1-300
    else
        echo "  no hay $STATE/agyshim.ndjson"
    fi
}

if [ "$1" = "--diagnostico" ]; then
    diagnostico
    exit 0
fi

mkdir -p "$STATE" 2>/dev/null
log "=== recuperacion manual"

# 0) BINARIO: si falta, o si su SHA-256 no es el de la release, se
#    descarga, se VERIFICA y se instala (con copia del anterior a .prev).
#
#    MEDIDO 2026-11-25: el binario del movil era del 2026-11-23 (d7d72f1)
#    y su `update`_buscaba el binario en la ruta VOLATIL antigua => no se
#    actualizaba nunca.  Por eso aqui se hace a mano, con verificacion.
#
#    Descarga con las herramientas que haya: `curl` (Termux) o `wget`
#    (toybox).  Con `toybox wget` el codigo de salida es 0.
REPO_RELEASE=https://github.com/boolforge/megadaemon/releases/latest/download
ASSET=megadaemon-core-android-arm64

descargar() {
    # $1 = url, $2 = destino
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --max-time 600 -o "$2" "$1" && return 0
    fi
    if command -v wget >/dev/null 2>&1; then
        wget -q -O "$2" "$1" && return 0
    fi
    toybox wget -q -O "$2" "$1" 2>/dev/null && return 0
    return 1
}

sha256_de() { sha256sum "$1" 2>/dev/null | cut -d' ' -f1; }

instalar_nuevo() {
    echo "== binario: comprobando release"
    if ! descargar "$REPO_RELEASE/$ASSET.sha256" "$STATE/asset.sha256"; then
        log "AVISO: no se pudo descargar el .sha256 (sin red?): se usa el binario actual"
        return 1
    fi
    esperado=$(cut -d' ' -f1 < "$STATE/asset.sha256" 2>/dev/null | tr -d '\r\n')
    case "$esperado" in
        [0-9a-f][0-9a-f]*) ;;
        *) log "AVISO: el .sha256 no es valido ('$esperado'): no se toca el binario"; return 1;;
    esac
    actual=""
    [ -f "$BIN" ] && actual=$(sha256_de "$BIN")
    if [ "$actual" = "$esperado" ]; then
        echo "  el binario ya es el de la release ($esperado)"
        return 0
    fi
    echo "  descargando el binario de la release (sha256 $esperado)..."
    if ! descargar "$REPO_RELEASE/$ASSET" "$STATE/megadaemon-core.new"; then
        log "ERROR: no se pudo descargar el binario: no se toca el actual"
        return 1
    fi
    got=$(sha256_de "$STATE/megadaemon-core.new")
    if [ "$got" != "$esperado" ]; then
        log "ERROR: SHA-256 no coincide (esperado $esperado, obtenido $got): no se instala"
        rm -f "$STATE/megadaemon-core.new"
        return 1
    fi
    [ -f "$BIN" ] && cp -f "$BIN" "$BIN.prev"
    chmod 755 "$STATE/megadaemon-core.new"
    mv -f "$STATE/megadaemon-core.new" "$BIN"
    echo "  instalado (sha256 verificado)"
    return 0
}

instalar_nuevo

# 0) SUPERVISOR: actualizarlo es parte de la recuperacion.
#
#    MEDIDO (2026-11-25, movil, log del supervisor):
#      "ERROR: no existe /data/local/tmp/megadaemon/megadaemon-core (y no hay
#       despliegue en curso)"
#    El supervisor INSTALADO en el movil es una version ANTIGUA cuyo PREFIX
#    era la ruta VOLATIL (/data/local/tmp/megadaemon); el binario canonico
#    vive en /data/adb/megadaemon => el supervisor no lo encontraba y NO
#    PODIA relanzar el daemon => el movil se quedaba sin daemon (UI y
#    gateway caidos) y en bucle de reinicios.
#    Ademas el supervisor arrancaba el daemon en el 9090 mientras este
#    script lo arrancaba en el 3000 => dos daemons compitiendo por puertos
#    distintos y el supervisor matando al que estaba en el 3000 por "no
#    responder /health".
#
#    Solucion: este script instala el supervisor ACTUAL (y el keeper) desde
#    el repo, reinicia el stack y deja que el supervisor sea el UNICO que
#    levanta el daemon, en el puerto real.
SERVICED=/data/adb/service.d
SUPERVISOR_NUEVO="$STATE/99-megadaemon.sh"
KEEPER_NUEVO="$STATE/97-megadaemon-keeper.sh"
REPO_RAW=https://raw.githubusercontent.com/boolforge/megadaemon
REF="${MEGADAEMON_REF:-main}"

actualizar_supervisor() {
    echo "== supervisor: comprobando version"
    mkdir -p "$STATE" 2>/dev/null
    if ! descargar "$REPO_RAW/$REF/deploy/99-megadaemon.sh" "$SUPERVISOR_NUEVO"; then
        log "AVISO: no se pudo descargar el supervisor (sin red?): se deja el que hay"
        return 1
    fi
    # El supervisor nuevo DEBE apuntar al prefijo persistente.
    if ! grep -q '^PREFIX=/data/adb/megadaemon' "$SUPERVISOR_NUEVO"; then
        log "AVISO: el supervisor descargado no usa el prefijo persistente: no se instala"
        return 1
    fi
    if [ -f "$SERVICED/99-megadaemon.sh" ] && cmp -s "$SUPERVISOR_NUEVO" "$SERVICED/99-megadaemon.sh"; then
        echo "  el supervisor ya es el actual"
    else
        mkdir -p "$SERVICED" 2>/dev/null
        [ -f "$SERVICED/99-megadaemon.sh" ] && cp -f "$SERVICED/99-megadaemon.sh" "$SERVICED/99-megadaemon.sh.bak"
        cp -f "$SUPERVISOR_NUEVO" "$SERVICED/99-megadaemon.sh" && chmod 755 "$SERVICED/99-megadaemon.sh"
        log "supervisor actualizado en $SERVICED/99-megadaemon.sh (copia .bak)"
        echo "  supervisor actualizado"
    fi
    # El keeper tambien (vigila al supervisor: sin el, el movil se queda sin
    # supervisor tras un reinicio).
    if descargar "$REPO_RAW/$REF/deploy/97-megadaemon-keeper.sh" "$KEEPER_NUEVO"; then
        mkdir -p "$SERVICED" 2>/dev/null
        cp -f "$KEEPER_NUEVO" "$SERVICED/97-megadaemon-keeper.sh" 2>/dev/null && \
            chmod 755 "$SERVICED/97-megadaemon-keeper.sh" 2>/dev/null
        echo "  keeper actualizado"
    fi
    return 0
}
actualizar_supervisor

# 0.b) Reiniciar el stack: al supervisor viejo hay que matarlo para que
#     cargue el nuevo (Magisk solo los lanza al arrancar el movil).
supervisor_vivo() {
    for p in /proc/[0-9]*; do
        [ -r "$p/cmdline" ] || continue
        c=$(tr '\0' ' ' < "$p/cmdline" 2>/dev/null)
        case "$c" in *99-megadaemon.sh*) echo "$p"; return 0;; esac
    done
    return 1
}
if s=$(supervisor_vivo); then
    log "reiniciando el supervisor (pid $s) para que cargue la version nueva"
    kill -9 $(echo "$s" | tr '\n' ' ') 2>/dev/null
    sleep 2
fi

# 1) Daemon: hay que matarlo DE VERD y esperar a que quede libre el puerto.
#
#    MEDIDO (2026-11-25, movil): con un simple "kill + 2 s" el daemon NUEVO
#    arranco y fallo con "Failed to bind Rust Cockpit on 0.0.0.0:3000:
#    Address already in use" => el daemon VIEJO seguia sirviendo la UI (la
#    V1, sin la vista Antigravity) y el usuario creia que la UI habia
#    "perdido cosas".  El supervisor ya lo hacia bien (SIGKILL + esperar a
#    que /proc/net/tcp deje de mostrar el puerto en LISTEN); aqui se copia
#    esa tecnica.
if d=$(daemon_vivo); then
    log "el daemon ya corria ($d): lo mato para cargar el binario actual"
    kill -9 $(echo "$d" | tr '\n' ' ') 2>/dev/null
fi
puerto_libre() {
    hp=$(printf '%04X' "$PORT")
    ! grep -qiE ":$hp [0-9a-f]{8}:[0-9a-f]{4} 0A" /proc/net/tcp /proc/net/tcp6 2>/dev/null
}
i=0
while [ $i -lt 30 ]; do
    puerto_libre && break
    i=$((i + 1)); sleep 1
done
if ! puerto_libre; then
    log "el puerto $PORT sigue ocupado: mato por nombre"
    pkill -9 -x megadaemon-core 2>/dev/null
    sleep 3
fi
log "puerto $PORT libre"
if [ ! -x "$BIN" ]; then
    log "ERROR: no hay binario en $BIN"
    exit 1
fi

# MEDIDO (2026-11-25): el daemon lo debe levantar EL SUPERVISOR (es el que
# gestiona energia, tunel, auto-update y watchdog).  Este script solo lo
# levanta si el supervisor no lo hace.
arrancar_daemon_a_mano() {
    export MEGADAEMON_BIN="$BIN"
    export MEGADAEMON_STATE_DIR="$STATE"
    export MEGADAEMON_INSTANCE_NAME=samsung
    export MEGADAEMON_INSTANCE_ROLE=primary
    export MEGADAEMON_COCKPIT_PORT="$PORT"
    cd "$PREFIX" || return 1
    setsid nohup "$BIN" cockpit "$PORT" >> "$LOG" 2>&1 < /dev/null &
    log "daemon arrancado a mano (pid $!)"
}
i=0
while [ $i -lt 30 ]; do
    if health; then log "el supervisor levanto el daemon (cockpit responde)"; break; fi
    i=$((i + 1)); sleep 2
done
if [ $i -ge 30 ]; then
    log "el supervisor no levanto el daemon en 60 s: lo arranco yo"
    arrancar_daemon_a_mano
fi

# 2) Espera a que responda y COMPRUEBA que es el daemon NUEVO.
i=0
while [ $i -lt 25 ]; do
    if health; then log "cockpit responde en $PORT"; break; fi
    i=$((i+1)); sleep 2
done
if [ $i -ge 25 ]; then
    log "AVISO: el cockpit no respondio tras 50 s"
else
    # MEDIDO (2026-11-25): hay que comprobar que el que responde es el NUEVO:
    # si el viejo sobrevive, la UI que se ve es la VIEJA (y parece que "la UI
    # ha perdido la vista Antigravity").
    d2=$(daemon_vivo)
    log "daemon en marcha: ${d2:-desconocido}"
    h=$(toybox wget -q -O - "http://127.0.0.1:$PORT/health" 2>/dev/null | head -c 500)
    log "health: $h"
fi

# 2.b) El LS instrumentado, DESPUES del parche.
#
#      MEDIDO (2026-11-25, movil): la app RE-EXTRAE files/runtime/ en CADA
#      arranque => el .so parcheado se pierde.  Lo que funciona es MATAR el
#      LS (libloader.so) DESPUES de que el daemon haya re-aplicado el
#      parche: la app lo relanza en ~1 s y el LS nuevo carga el .so ya
#      parcheado => el fix del passthrough (la ruta con dos puntos) llega al
#      LS.  Si se mata ANTES, el LS se relanza con el .so sin parchear y el
#      parche posterior no surte efecto.
#
#      El daemon aplica la instrumentacion en su ciclo automatico (60 s por
#      defecto), asi que aqui se espera a que pase.
ESPERA_INSTRUMENTACION="${ESPERA_INSTRUMENTACION:-75}"
ls_vivo() {
    for p in /proc/[0-9]*; do
        [ -r "$p/cmdline" ] || continue
        c=$(tr '\0' ' ' < "$p/cmdline" 2>/dev/null)
        case "$c" in *libloader.so*) echo "$p"; return 0;; esac
    done
    return 1
}
log "esperando $ESPERA_INSTRUMENTACION s a que el daemon re-aplique el parche del .so"
sleep "$ESPERA_INSTRUMENTACION"
if l=$(ls_vivo); then
    log "el LS instrumentado esta vivo ($l): lo mato para que recargue el .so parcheado"
    kill -9 $(echo "$l" | tr '\n' ' ') 2>/dev/null
    i=0
    while [ $i -lt 10 ]; do
        if ls_vivo >/dev/null 2>&1; then i=$((i + 1)); sleep 1; else break; fi
    done
    if ls_vivo >/dev/null 2>&1; then
        log "el LS se ha relanzado (eso es lo normal: la app lo vuelve a levantar)"
    else
        log "el LS esta muerto: la app lo relanzara con el .so parcheado"
    fi
else
    log "no hay ningun LS vivo (la app lo relanzara al abrirla)"
fi

# 3) Tunel: lo relanza el watchdog (98) o el propio daemon; aqui se espera
#    a que exista una URL valida.
i=0
while [ $i -lt 30 ]; do
    if [ -s "$PREFIX/tunnel.url" ]; then
        u=$(cat "$PREFIX/tunnel.url" 2>/dev/null)
        case "$u" in https://*) log "tunel: $u"; break;; esac
    fi
    i=$((i+1)); sleep 2
done
[ $i -ge 30 ] && log "AVISO: no hay URL de tunel (el watchdog deberia levantarlo)"

diagnostico
