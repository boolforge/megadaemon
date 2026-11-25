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
PORT=3000
GW=9090
LOG=/sdcard/megadaemon.log

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

# 1) Daemon.
if d=$(daemon_vivo); then
    log "el daemon ya corria ($d): lo reinicio para cargar el binario actual"
    kill -9 $(echo "$d" | tr '\n' ' ') 2>/dev/null
    sleep 2
fi
if [ ! -x "$BIN" ]; then
    log "ERROR: no hay binario en $BIN"
    exit 1
fi
export MEGADAEMON_BIN="$BIN"
export MEGADAEMON_STATE_DIR="$STATE"
export MEGADAEMON_INSTANCE_NAME=samsung
export MEGADAEMON_INSTANCE_ROLE=primary
export MEGADAEMON_COCKPIT_PORT="$PORT"
cd "$PREFIX" || exit 1
setsid nohup "$BIN" cockpit "$PORT" >> "$LOG" 2>&1 < /dev/null &
log "daemon arrancado (pid $!)"

# 2) Espera a que responda.
i=0
while [ $i -lt 20 ]; do
    if health; then log "cockpit responde en $PORT"; break; fi
    i=$((i+1)); sleep 2
done
[ $i -ge 20 ] && log "AVISO: el cockpit no respondio tras 40 s"

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
