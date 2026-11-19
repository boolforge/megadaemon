#!/system/bin/sh
# =============================================================================
# 91-adb-tcp.sh — ADB sobre TCP persistente para el Samsung (instancia MAESTRA)
#
# POR QUÉ
#   El usuario quiere que varios equipos (PC, otro móvil) se conecten al
#   Samsung a la vez por ADB:  `adb connect 192.168.1.124:5555`.
#
#   El truco de `setprop service.adb.tcp.port 5555` + `stop adbd; start adbd`
#   funciona, pero NO es estable: cada vez que algo reinicia `adbd` (un
#   `stop adbd`, un cambio de red o el propio sistema) el puerto se cae y
#   adbd vuelve a escuchar sólo en el socket local de USB.  Este script lo
#   deja PERMANENTE:
#
#     * Magisk ejecuta todo lo que hay en service.d al arrancar -> se aplica
#       solo en cada reinicio (sin tocar el móvil a mano).
#     * Un vigilante comprueba cada 60 s que adbd sigue escuchando en 5555 y
#       lo vuelve a levantar si se ha caído.
#
# SEGURIDAD (importante)
#   `adbd` por TCP da control del dispositivo a quien se conecte (y con `su`,
#   control root).  Por eso:
#     * el puerto se puede limitar a una IP con MEGADAEMON_ADB_ALLOW_FROM
#       (regla de iptables; sin la variable, se acepta la red local);
#     * se puede apagar en cualquier momento: `sh 91-adb-tcp.sh off`.
#
# VARIABLES
#   MEGADAEMON_ADB_PORT           puerto (por defecto 5555)
#   MEGADAEMON_ADB_ALLOW_FROM     IP o CIDR permitida (opcional)
#   MEGADAEMON_ADB_WATCH=0        desactiva el vigilante (por defecto activo)
# =============================================================================

PORT="${MEGADAEMON_ADB_PORT:-5555}"
ALLOW="${MEGADAEMON_ADB_ALLOW_FROM:-}"
WATCH="${MEGADAEMON_ADB_WATCH:-1}"
LOG=/sdcard/megadaemon-adb.log

log() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" >> "$LOG" 2>/dev/null; }

listening() {
    # ss no está en todas las versiones: se mira /proc/net/tcp{,6}.
    grep -qiE ":$(printf '%04X' "$PORT") " /proc/net/tcp /proc/net/tcp6 2>/dev/null
}

enable() {
    setprop service.adb.tcp.port "$PORT" 2>/dev/null
    if [ -n "$ALLOW" ]; then
        iptables -I INPUT -p tcp --dport "$PORT" -s "$ALLOW" -j ACCEPT 2>/dev/null
        iptables -I INPUT -p tcp --dport "$PORT" ! -s "$ALLOW" -j DROP 2>/dev/null
    fi
    # Reiniciar adbd es lo que hace que adbd abra el socket TCP.
    stop adbd 2>/dev/null
    start adbd 2>/dev/null
    sleep 2
    if listening; then
        log "ADB TCP habilitado en $PORT${ALLOW:+ (sólo desde $ALLOW)}"
    else
        log "ADB TCP: el puerto $PORT no quedó escuchando"
    fi
}

disable() {
    setprop service.adb.tcp.port -1 2>/dev/null
    iptables -D INPUT -p tcp --dport "$PORT" 2>/dev/null
    stop adbd 2>/dev/null
    start adbd 2>/dev/null
    log "ADB TCP desactivado"
}

case "${1:-on}" in
    off) disable ;;
    on|enable) enable ;;
    status)
        echo "port=$(getprop service.adb.tcp.port 2>/dev/null)"
        listening && echo "listening=yes" || echo "listening=no"
        ;;
    *)
        # Sin argumentos: se comporta como service.d (habilita + vigila).
        enable
        if [ "$WATCH" != "0" ]; then
            (
                while true; do
                    sleep 60
                    listening || enable
                done
            ) &
        fi
        ;;
esac
