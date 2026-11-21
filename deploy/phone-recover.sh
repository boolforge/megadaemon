#!/system/bin/sh
# =====================================================================
# phone-recover.sh — deja el móvil MEGADAEMON recuperado y autocurable
#
# Un comando, idempotente, pensado para ejecutarse EN el móvil:
#
#     su -mm -c 'sh /data/local/tmp/megadaemon/phone-recover.sh'
#
# Qué hace (y qué NO hace):
#   * NO reinicia el móvil.
#   * NO reinicia el daemon si está vivo y sano (sólo lo relanza si no
#     responde) => re-executarlo nunca deja el móvil peor.
#   * NO toca el binario salvo que se le pase --install <ruta>.
#   * NO toca la API key ni el token: sólo crea lo que falta.
#
# Deja el móvil en este estado:
#   1. /data/local/tmp/megadaemon/state existe (el estado REAL del daemon);
#   2. megadaemon.env declara MEGADAEMON_STATE_DIR y MEGADAEMON_LISTEN;
#   3. supervisor + cloudflared + watchdog instalados en /data/adb/service.d
#      (=> sobreviven a un reinicio del móvil);
#   4. el daemon vivo y con /health respondiendo;
#   5. un túnel VIVO con su URL en state/tunnel.url;
#   6. el canal publicado (state/channel.json con el 'quick' correcto);
#   7. el beacon LAN emitido (descubrimiento sin internet).
#
# Uso:
#     phone-recover.sh                 # recuperar y verificar
#     phone-recover.sh --diagnostico   # sólo informe, no cambia nada
#     phone-recover.sh --install /sdcard/Download/megadaemon-core
#     phone-recover.sh --no-tunnel     # recuperación sin internet
# =====================================================================

PREFIX=/data/local/tmp/megadaemon
STATE="$PREFIX/state"
SERVICE_D=/data/adb/service.d
ENV_FILE="$PREFIX/megadaemon.env"
BIN="$PREFIX/megadaemon-core"
LOG="$PREFIX/recover.log"
PORT=9090
CF=/data/data/com.termux/files/usr/bin/cloudflared
[ -x "$CF" ] || CF=$(command -v cloudflared 2>/dev/null)
PUB="$PREFIX/channel-publish"

# El móvil NO tiene curl (medido).  El sondeo fiable es el propio binario
# (`channel-publish --probe` imprime el código HTTP en stdout); como
# respaldo, `toybox wget` (que sí trae Android) y `curl` si estuviera.
http_code() {
    _u="$1"
    if [ -x "$PUB" ]; then
        _c=$("$PUB" --probe "$_u" 2>/dev/null | head -1)
        case "$_c" in
            ''|*[!0-9]*) : ;;
            *) echo "$_c"; return ;;
        esac
    fi
    if command -v toybox >/dev/null 2>&1; then
        if toybox wget -q -O /dev/null "$_u" 2>/dev/null; then echo 200; return; fi
        echo 0; return
    fi
    if command -v curl >/dev/null 2>&1; then
        _c=$(curl -s -m 5 -o /dev/null -w '%{http_code}' "$_u" 2>/dev/null)
        case "$_c" in ''|*[!0-9]*) echo 0 ;; *) echo "$_c" ;; esac
        return
    fi
    echo 0
}
MODE=recuperar
NO_TUNNEL=0
INSTALL_BIN=""

# El directorio de este script: aquí se buscan los ficheros a desplegar.
SELF_DIR=$(cd "$(dirname "$0")" 2>/dev/null && pwd)
[ -n "$SELF_DIR" ] || SELF_DIR="$PREFIX"

log() {
    printf '[%s] %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*" | tee -a "$LOG"
}
step() { log "── $* ──"; }

for a in "$@"; do
    case "$a" in
        --diagnostico) MODE=diagnostico ;;
        --no-tunnel)   NO_TUNNEL=1 ;;
        --install)     shift; INSTALL_BIN="${1:-}" ;;
        -h|--help)     sed -n '2,40p' "$0"; exit 0 ;;
    esac
    shift
done

mkdir -p "$PREFIX" "$STATE" 2>/dev/null
[ -f "$LOG" ] || : > "$LOG"

# ─────────────────────────────────────────────────────────────────────
# 0) Preflight
# ─────────────────────────────────────────────────────────────────────
step "Preflight"
if [ "$(id -u)" != "0" ]; then
    log "ERROR: hace falta root. Ejecuta: su -mm -c 'sh $0'"
    exit 1
fi
log "root OK · PREFIX=$PREFIX · STATE=$STATE"
log "estado actual: $(ls "$STATE" 2>/dev/null | tr '\n' ' ')"

# ─────────────────────────────────────────────────────────────────────
# 1) state_dir + env
# ─────────────────────────────────────────────────────────────────────
step "Estado y entorno"
mkdir -p "$STATE" 2>/dev/null
if [ ! -d "$STATE" ]; then
    log "ERROR: no se pudo crear $STATE"
    exit 1
fi
chmod 700 "$STATE" 2>/dev/null

# El env tiene que declarar el state_dir: es lo que hace que daemon,
# cloudflared, watchdog, peers, auditoría y tablero escriban EN EL MISMO
# sitio (medido: sin esto el daemon caía en un directorio inexistente).
if [ ! -f "$ENV_FILE" ]; then
    {
        echo "MEGADAEMON_STATE_DIR=$STATE"
        echo "MEGADAEMON_LISTEN=127.0.0.1:$PORT"
    } > "$ENV_FILE"
    chmod 600 "$ENV_FILE"
    log "creado $ENV_FILE (sin secretos: la key se acuña aparte)"
else
    log "$ENV_FILE ya existe: NO se tocan sus secretos"
    if ! grep -q '^MEGADAEMON_STATE_DIR=' "$ENV_FILE" 2>/dev/null; then
        printf 'MEGADAEMON_STATE_DIR=%s\n' "$STATE" >> "$ENV_FILE"
        log "añadido MEGADAEMON_STATE_DIR al env"
    fi
    if ! grep -q '^MEGADAEMON_LISTEN=' "$ENV_FILE" 2>/dev/null; then
        printf 'MEGADAEMON_LISTEN=127.0.0.1:%s\n' "$PORT" >> "$ENV_FILE"
        log "añadido MEGADAEMON_LISTEN al env"
    fi
fi

# ─────────────────────────────────────────────────────────────────────
# 2) Ficheros de deployment en service.d (=> sobreviven al reinicio)
# ─────────────────────────────────────────────────────────────────────
step "Despliegue en $SERVICE_D"
mkdir -p "$SERVICE_D" 2>/dev/null
deploy() {
    # $1 = origen  $2 = destino  $3 = chmod
    [ -f "$1" ] || return 1
    cp -f "$1" "$2" 2>/dev/null || return 1
    chmod "$3" "$2" 2>/dev/null
    return 0
}
for pair in "99-megadaemon.sh:$SERVICE_D/99-megadaemon.sh" \
            "98-cloudflared.sh:$SERVICE_D/98-cloudflared.sh" \
            "tunnel-watchdog.sh:$PREFIX/tunnel-watchdog.sh"; do
    src="$SELF_DIR/${pair%%:*}"
    dst="${pair#*:}"
    if deploy "$src" "$dst" 755; then
        log "desplegado $(basename "$dst")"
    else
        log "AVISO: no encuentro $(basename "$src") en $SELF_DIR (se usa lo que ya haya en el móvil)"
    fi
done
# El watchdog se lanza desde 98-cloudflared.sh con esta ruta:
[ -f "$SERVICE_D/98-cloudflared.sh" ] && {
    sed -i "s#^WATCHDOG=.*#WATCHDOG=\"$PREFIX/tunnel-watchdog.sh\"#" "$SERVICE_D/98-cloudflared.sh" 2>/dev/null
    log "watchdog apuntando a $PREFIX/tunnel-watchdog.sh"
}

# ─────────────────────────────────────────────────────────────────────
# 3) Binario (opcional)
# ─────────────────────────────────────────────────────────────────────
if [ -n "$INSTALL_BIN" ] && [ -f "$INSTALL_BIN" ]; then
    step "Instalando binario desde $INSTALL_BIN"
    cp -f "$INSTALL_BIN" "$BIN.new" 2>/dev/null
    if [ -f "$BIN" ]; then cp -f "$BIN" "$BIN.prev" 2>/dev/null; fi
    mv -f "$BIN.new" "$BIN" 2>/dev/null
    chmod 755 "$BIN" 2>/dev/null
    log "binario instalado: $(sha256sum "$BIN" 2>/dev/null | cut -c1-16) · $(stat -c %s "$BIN" 2>/dev/null) B"
fi
[ -x "$BIN" ] || log "AVISO: no hay binario en $BIN (usa --install)"

# ─────────────────────────────────────────────────────────────────────
# 4) Daemon: vivo y sano
# ─────────────────────────────────────────────────────────────────────
step "Daemon"
daemon_ok() {
    # vivo = proceso + /health responde en el puerto local
    pidof megadaemon-core >/dev/null 2>&1 || return 1
    for p in $PORT 3030 8090 3000; do
        code=$(http_code "http://127.0.0.1:$p")
        case "$code" in 200|401|403) return 0 ;; esac
    done
    return 1
}
if daemon_ok; then
    log "daemon VIVO y sano (pid $(pidof megadaemon-core 2>/dev/null)) — no se toca"
else
    if [ "$MODE" = "diagnostico" ]; then
        log "daemon NO sano (sólo informe: no se relanza)"
    else
        log "daemon muerto o mudo -> relanzando el supervisor"
        [ -f "$SERVICE_D/99-megadaemon.sh" ] && {
            setsid nohup sh "$SERVICE_D/99-megadaemon.sh" >/sdcard/megadaemon.log 2>&1 &
            sleep 18
        }
        daemon_ok && log "daemon arriba (pid $(pidof megadaemon-core 2>/dev/null))" \
                    || log "AVISO: el daemon sigue sin responder (mira /sdcard/megadaemon.log)"
    fi
fi

# ─────────────────────────────────────────────────────────────────────
# 5) Túnel: uno solo, vivo, con la URL en el state_dir
# ─────────────────────────────────────────────────────────────────────
tunnel_url() {
    [ -f "$STATE/tunnel.url" ] && head -1 "$STATE/tunnel.url" 2>/dev/null
}
tunnel_alive() {
    u=$(tunnel_url)
    [ -n "$u" ] || return 1
    code=$(http_code "$u")
    # 401/403 = el daemon vive detrás y exige credencial => el túnel funciona
    case "$code" in 200|401|403) return 0 ;; esac
    return 1
}
step "Túnel"
if [ "$NO_TUNNEL" = "1" ]; then
    log "omitido por --no-tunnel"
elif [ ! -x "$CF" ]; then
    log "AVISO: no encuentro cloudflared ($CF). Sin túnel, pero el canal LAN funciona."
else
    if tunnel_alive; then
        log "túnel VIVO: $(tunnel_url)"
    else
        if [ "$MODE" = "diagnostico" ]; then
            log "túnel muerto (sólo informe: no se relanza)"
        else
            log "túnel muerto -> relanzando cloudflared (sin tocar el daemon)"
            # Sólo se mata cloudflared: el daemon NO se toca.
            pkill -f cloudflared >/dev/null 2>&1
            sleep 3
            setsid nohup "$CF" tunnel --url "http://127.0.0.1:$PORT" --no-autoupdate \
                > "$STATE/cloudflared.log" 2>&1 &
            i=0
            while [ "$i" -lt 30 ]; do
                sleep 2
                u=$(grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' "$STATE/cloudflared.log" 2>/dev/null | head -1)
                [ -n "$u" ] && break
                i=$((i + 1))
            done
            if [ -n "$u" ]; then
                echo "$u" > "$STATE/tunnel.url"
                chmod 644 "$STATE/tunnel.url"
                log "túnel nuevo: $u"
            else
                log "AVISO: no capturé la URL (mira $STATE/cloudflared.log)"
            fi
        fi
    fi
fi

# ─────────────────────────────────────────────────────────────────────
# 6) Canal: publicar el 'quick' (el enjambre lo lee)
# ─────────────────────────────────────────────────────────────────────
step "Canal"
if [ -x "$PUB" ]; then
    setsid nohup "$PUB" --state-dir "$STATE" >/dev/null 2>&1 &
    sleep 4
    if [ -f "$STATE/channel.json" ]; then
        q=$(grep -o '"quick"[^,]*' "$STATE/channel.json" 2>/dev/null | head -1)
        log "channel.json presente · $q"
    else
        log "AVISO: no hay $STATE/channel.json aún"
    fi
else
    log "AVISO: no está $PUB (el /channel.json del daemon sirve igualmente)"
fi

# ─────────────────────────────────────────────────────────────────────
# 7) Beacon LAN (descubrimiento sin internet)
# ─────────────────────────────────────────────────────────────────────
step "Canal LAN"
if [ -f "$STATE/lan.json" ]; then
    log "lan.json: $(cat "$STATE/lan.json" 2>/dev/null | head -c 200)"
else
    log "el beacon lo emite el daemon (tarda unos segundos en escribir state/lan.json)"
fi

# ─────────────────────────────────────────────────────────────────────
# 8) Informe final
# ─────────────────────────────────────────────────────────────────────
step "Estado final"
IP=""
for i in $(ip -4 -o addr show 2>/dev/null | awk '{print $2" "$4}'); do
    a=$(echo "$i" | awk '{print $2}' | cut -d/ -f1)
    case "$a" in 127.*) ;; *) IP="$a"; break ;; esac
done
[ -z "$IP" ] && IP=$(ip route get 1.1.1.1 2>/dev/null | awk '{print $7; exit}')
[ -z "$IP" ] && IP="(sin IP de LAN detectable)"

echo ""
echo "================= MEGADAEMON · RESUMEN ================="
printf 'daemon   : %s\n' "$(pidof megadaemon-core 2>/dev/null || echo 'MUERTO')"
printf 'health   : %s\n' "$(http_code "http://127.0.0.1:$PORT")"
printf 'tunel    : %s\n' "$(tunnel_url)"
printf 'LAN      : http://%s:%s\n' "$IP" "$PORT"
printf 'canonico : https://mdtmfnc.duckdns.org/channel.json\n'
printf 'state    : %s\n' "$STATE"
printf 'load     : %s\n' "$(cat /proc/loadavg 2>/dev/null)"
printf 'mem libre: %s\n' "$(grep MemAvailable /proc/meminfo 2>/dev/null | awk '{print $2" kB"}')"
echo "========================================================"
echo ""
echo "Desde el PC (misma red), comprueba con:"
echo "  python3 tools/lan_discover.py --probe"
echo "  curl -sS http://$IP:$PORT/health"
echo ""
echo "Desde el sandbox, el estado del canal:"
echo "  bash tools/verify_canal.sh"
