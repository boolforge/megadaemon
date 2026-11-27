#!/system/bin/sh
# 97-megadaemon-keeper.sh — GUARDIÁN del supervisor (Magisk service.d)
#
# Se instala en: /data/adb/service.d/97-megadaemon-keeper.sh (chmod 755)
#
# POR QUÉ EXISTE (medido el 2026-11-23 en el Samsung del usuario):
#   El supervisor (99-megadaemon.sh) es lo único que relanza el daemon si
#   este muere.  Si el supervisor ALSO muere, el móvil se queda sin daemon
#   Y sin supervisor: el túnel de Cloudflare sigue vivo (lo sostiene otro
#   script de service.d) pero responde 502 para siempre, y no hay quien
#   reinstale nada.  MEDIDO: 1644 reinicios del daemon por un falso positivo
#   del watchdog, y después el supervisor murió sin dejar NADIE que lo
#   resucitara => móvil inalcanzable hasta un reinicio manual.
#
#   Este script es el guardián del guardián: cada 60 s comprueba que el
#   supervisor siga vivo y, si no, lo relanza.  Es un script hermano de
#   service.d (no un hijo del supervisor), así que no hereda su entorno ni
#   muere con él.
#
# REGLAS:
#   * NO reinicia el daemon directamente: de eso se encarga el supervisor.
#   * Espera a que el sistema se asiente (arranque) antes de actuar.
#   * Es idempotente: si ya hay un supervisor, no hace nada.
#   * Escribe en /sdcard/megadaemon.log (mismo log que el supervisor).

LOG=/sdcard/megadaemon.log
SUPERVISOR=/data/adb/service.d/99-megadaemon.sh
# MEDIDO (2026-11-23): ruta PERSISTENTE (la vieja, /data/local/tmp, es
# volatil y el reinicio del movil se llevaba el binario).
PREFIX=/data/adb/megadaemon

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] keeper: $*" >> "$LOG" 2>/dev/null
}

# --- ¿hay proceso vivo? (portable, sin `pgrep -f`) -------------------------
# MEDIDO (2026-11-27, movil): `pgrep -f` NO es de fiar en toybox (el shell de
# Android) y ademas cuenta como "vivo" a los ENVOLTORIOS `sh -c ...` (que no
# son el proceso real) => el guardian creia que el supervisor estaba muerto y
# lo relanzaba DETRAS de otro (6 supervisores, 6 daemons, 6 cloudflared y 5
# watchdogs en el movil: load 58 => el tunel no podia arrancar).
# Ahora: se listan los PIDs leyendo /proc, se descartan los envoltorios
# `sh -c` y se pueden pedir VARIAS subcadenas (todas tienen que estar).
# PROC_DIR se puede cambiar para poder PROBAR la funcion sin /proc real.
PROC_DIR="${PROC_DIR:-/proc}"
proc_pids() {
    _pat=$1; shift
    for _pp in "$PROC_DIR"/[0-9]*; do
        [ -r "$_pp/cmdline" ] || continue
        _pc=$(tr '\0' ' ' < "$_pp/cmdline" 2>/dev/null) || continue
        case "$_pc" in
            "sh -c "*|*/sh\ -c\ *) continue ;;
        esac
        _ok=1
        case "$_pc" in *$_pat*) ;; *) _ok=0 ;; esac
        for _x in "$@"; do
            case "$_pc" in *"$_x"*) ;; *) _ok=0 ;; esac
        done
        [ "$_ok" = "1" ] && printf '%s\n' "${_pp##*/}"
    done
    return 0
}
proc_vivo() { [ -n "$(proc_pids "$@")" ]; }

# Al arrancar, Magisk lanza TODOS los scripts de service.d a la vez: dar
# margen para que el supervisor se instale y el sistema se asiente.
sleep 90

while true; do
    sleep 60
    # Si el supervisor no esta vivo, relanzarlo.
    if ! proc_vivo "service.d/99-megadaemon.sh"; then
        log "el supervisor no esta vivo: lo relanzo"
        if [ -x "$SUPERVISOR" ]; then
            setsid nohup sh "$SUPERVISOR" >/dev/null 2>&1 &
            sleep 20
            if proc_vivo "service.d/99-megadaemon.sh"; then
                log "supervisor relanzado"
            else
                log "AVISO: el supervisor no arranco"
            fi
        else
            log "ERROR: no existe $SUPERVISOR (no se puede relanzar)"
        fi
    fi
done
