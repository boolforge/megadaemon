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

# Al arrancar, Magisk lanza TODOS los scripts de service.d a la vez: dar
# margen para que el supervisor se instale y el sistema se asiente.
sleep 90

while true; do
    sleep 60
    # Si el supervisor no esta vivo, relanzarlo.
    if ! pgrep -f "service.d/99-megadaemon.sh" > /dev/null 2>&1; then
        log "el supervisor no esta vivo: lo relanzo"
        if [ -x "$SUPERVISOR" ]; then
            setsid nohup sh "$SUPERVISOR" >/dev/null 2>&1 &
            sleep 20
            if pgrep -f "service.d/99-megadaemon.sh" > /dev/null 2>&1; then
                log "supervisor relanzado"
            else
                log "AVISO: el supervisor no arranco"
            fi
        else
            log "ERROR: no existe $SUPERVISOR (no se puede relanzar)"
        fi
    fi
done
