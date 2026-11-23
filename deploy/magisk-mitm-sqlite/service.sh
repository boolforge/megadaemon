#!/system/bin/sh
# =====================================================================
# service.sh — arranque TARDE (late_start service).
#
# Aqui es donde el MITM se (re)aplica tras un reinicio: el estado vive en
#   $MEGADAEMON_STATE_DIR/mitm/estado.json
# y dice si el MITM estaba ACTIVO.  Si lo estaba y la DB servida existe,
# se vuelve a montar (el bind-mount NO sobrevive a un reinicio).
#
# Es idempotente: si el MITM no estaba activo, no se hace nada.
# Si la DB servida ya no existe (estado borrado), se avisa y se deja
# el movil como estaba (nunca se monta un fichero inexistente).
# =====================================================================
MODDIR=/data/adb/modules/megadaemon_mitm_sqlite
STATE_DIR="${MEGADAEMON_STATE_DIR:-/data/local/tmp/megadaemon/state}"
ESTADO="$STATE_DIR/mitm/estado.json"
LOG="/data/local/tmp/megadaemon/mitm_sqlite.log"

log() { echo "[megadaemon-mitm] $*" >> "$LOG" 2>/dev/null; }

[ -x "$MODDIR/mitm-sqlite.sh" ] || { log "no hay mitm-sqlite.sh en el modulo"; exit 0; }
[ -f "$ESTADO" ] || { log "no hay estado: el MITM no estaba activo"; exit 0; }

# El estado activo tiene "montado": true y la accion "activar".
if ! grep -q '"accion"[[:space:]]*:[[:space:]]*"activar"' "$ESTADO" 2>/dev/null; then
  log "el estado dice que el MITM no estaba activo: no se toca nada"
  exit 0
fi

SERVED="$STATE_DIR/mitm/antigravity_servida.db"
if [ ! -f "$SERVED" ]; then
  log "la DB servida no existe ($SERVED): el MITM queda desactivado"
  exit 0
fi

OUT="$(sh "$MODDIR/mitm-sqlite.sh" activar 2>&1)"
log "reaplicado: $OUT"
exit 0
