#!/system/bin/sh
# =====================================================================
# uninstall.sh — se desinstala el modulo: se DESMONTA el bind-mount y se
# deja el movil como estaba (la DB real de Antigravity, sin tocar).
#
# Se desmonta siempre, aunque el estado no exista: puede haber un
# bind-mount huerfano de una sesion anterior.
# =====================================================================
MODDIR=/data/adb/modules/megadaemon_mitm_sqlite
STATE_DIR="${MEGADAEMON_STATE_DIR:-/data/local/tmp/megadaemon/state}"
LOG="/data/local/tmp/megadaemon/mitm_sqlite.log"

log() { echo "[megadaemon-mitm] $*" >> "$LOG" 2>/dev/null; }

OUT="$(sh "$MODDIR/mitm-sqlite.sh" desactivar 2>&1)"
log "desmontado: $OUT"

# Por si el script no estuviera (modulo a medias), se desmonta a pelo.
for d in \
  "/data/user/0/com.agy/files/.gemini/antigravity-app/conversation_summaries.db" \
  "/data/user/0/com.agy/files/.gemini/antigravity-app/conversations/conversation_summaries.db"
do
  umount "$d" 2>/dev/null && log "umount directo de $d"
done
exit 0
