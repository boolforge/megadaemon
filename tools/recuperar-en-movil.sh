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
    [ -f "$BIN.prev" ] && echo "  hay .prev (sha16 $(sha256sum "$BIN.prev" 2>/dev/null | cut -c1-16))" || echo "  no hay .prev"
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
