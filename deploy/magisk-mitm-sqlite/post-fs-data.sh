#!/system/bin/sh
# =====================================================================
# post-fs-data.sh — fase TEMPRANA (antes de zygote).
#
# Aquí NO se monta nada: en esta fase /data todavía puede no estar
# descifrado y un fallo dejaria el movil sin base de datos de sesiones.
# Solo se preparan rutas (idempotente y sin efectos).
# =====================================================================
MODDIR=/data/adb/modules/megadaemon_mitm_sqlite
STATE_DIR="${MEGADAEMON_STATE_DIR:-/data/local/tmp/megadaemon/state}"

mkdir -p "$STATE_DIR/mitm" 2>/dev/null
mkdir -p "$STATE_DIR/mitm/backup" 2>/dev/null
mkdir -p "$MODDIR" 2>/dev/null
exit 0
