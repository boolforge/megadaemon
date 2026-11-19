#!/system/bin/sh
# =====================================================================
# migrate_to_un1ca.sh — migración automatizada y REVERSIBLE a UN1CA
#
# Garlic:  1 backup completo  2 inventario  3 verificación  4 (flash manual)
#          5 post-flash (Magisk, módulos, Swift Backup)  6 verificación
#
#GARANTÍAS
#   * IDEMPOTENTE: se puede relanzar sin romper nada (todo se comprueba antes
#     de actuar).
#   * NUNCA borra: los datos del sistema actual se quedan intactos; el
#     "wipe" sólo ocurre si el operador lo pide explícitamente con --wipe.
#   * TRAZABLE: cada paso se registra en /sdcard/migracion_log.txt.
#   * ABORTABLE: Ctrl-C en cualquier momento deja el sistema como estaba
#     (salvo el flash, que es el punto de no retorno y va fuera del script).
#
# Uso:
#   sh migrate_to_un1ca.sh --backup        # sólo backup (seguro, por defecto)
#   sh migrate_to_un1ca.sh --restore       # post-flash: restaura módulos y apps
#   sh migrate_to_un1ca.sh --verify        # sólo verificación
#   sh migrate_to_un1ca.sh --wipe          # AVISO: borra /data (confirmación)
#
# v13.2.5 — https://github.com/boolforge/megadaemon
# =====================================================================

set -u

STAMP=$(date '+%Y%m%d_%H%M%S')
BK=/sdcard/backup/$STAMP
LOG=/sdcard/migracion_log.txt
STATE=/data/local/tmp/megadaemon/migration_state
DRIVE=${DRIVE_REMOTE:-gdrive:Backup_Android}

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" | tee -a "$LOG"
}
die() { log "ERROR: $*"; exit 1; }

need_root() {
    id=$(id -u)
    [ "$id" = "0" ] || die "necesita root (su -mm -c)"
}

usage() {
    sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'
    exit 0
}

[ $# -ge 1 ] || usage
MODE=$1
shift || true

need_root
mkdir -p "$BK" 2>/dev/null || true
: > "$LOG" 2>/dev/null || true

# ---------------------------------------------------------------------
# Inventario (idempotente, siempre)
# ---------------------------------------------------------------------
inventory() {
    log "== INVENTARIO =="
    mkdir -p "$BK/inventory"
    {
        echo "device=$(getprop ro.product.device)"
        echo "model=$(getprop ro.product.model)"
        echo "soc=$(getprop ro.soc.model)"
        echo "release=$(getprop ro.build.version.release)"
        echo "patch=$(getprop ro.build.version.security_patch)"
        echo "csc=$(getprop ro.csc.sales_code)/$(getprop ro.csc.country_code)"
        echo "fingerprint=$(getprop ro.build.fingerprint)"
    } > "$BK/inventory/device.txt"
    # módulos Magisk
    for d in /data/adb/modules/*/; do
        [ -d "$d" ] || continue
        id=$(basename "$d")
        [ "$id" = "update" ] && continue
        dis=$( [ -f "$d/disable" ] && echo DISABLED || echo enabled )
        ver=$(grep -m1 '^version=' "$d/module.prop" 2>/dev/null | cut -d= -f2-)
        echo "$id|$ver|$dis"
    done > "$BK/inventory/magisk_modules.txt"
    # módulos LSPosed (scope en la BD sqlite)
    cp -f /data/adb/lspd/config/modules_config.db "$BK/inventory/modules_config.db" 2>/dev/null || true
    # apps instaladas
    pm list packages > "$BK/inventory/packages.txt" 2>/dev/null
    pm list packages -3 >> "$BK/inventory/packages.txt" 2>/dev/null
    # CSC
    for p in $(find /system/optics/configs/carriers /odm/etc/carriers /vendor/etc/carriers \
                    -name cscfeature.xml 2>/dev/null); do
        d="$BK/inventory/csc"; mkdir -p "$d"
        cp -f "$p" "$d/$(echo "$p" | tr '/' '_')" 2>/dev/null
    done
    # APNs / ajustes
    settings list global > "$BK/inventory/settings_global.txt" 2>/dev/null
    settings list secure > "$BK/inventory/settings_secure.txt" 2>/dev/null
    settings list system  > "$BK/inventory/settings_system.txt" 2>/dev/null
    log "inventario guardado en $BK/inventory ($(ls -1 "$BK/inventory" | wc -l) ficheros)"
}

# ---------------------------------------------------------------------
# Backup (particiones críticas + módulos + configuración)
# ---------------------------------------------------------------------
backup() {
    log "== BACKUP =="
    mkdir -p "$BK/partitions" "$BK/modules" "$BK/config"
    for part in boot init_boot vendor_boot dtbo; do
        src="/dev/block/by-name/$part"
        [ -b "$src" ] || continue
        log "  volcando $part ..."
        dd if="$src" of="$BK/partitions/$part.img" bs=4M 2>>"$LOG" && \
            sha256sum "$BK/partitions/$part.img" >> "$BK/partitions/manifest.sha256"
    done
    # efs/persist: NO se copian por defecto (contienen IMEI; se documenta la ruta)
    log "  NOTE: efs/persist NO se copian (contienen IMEI); su copia exige --efs"
    if [ "${1:-}" = "--efs" ]; then
        for part in efs persist; do
            src="/dev/block/by-name/$part"
            [ -b "$src" ] && dd if="$src" of="$BK/partitions/$part.img" bs=4M 2>>"$LOG"
        done
    fi
    # módulos Magisk completos
    cp -a /data/adb/modules "$BK/modules/" 2>/dev/null
    # configuración de Magisk
    cp -a /data/adb/magisk "$BK/config/magisk" 2>/dev/null
    # CSC original (ya collected en inventory)
    # manifiesto final
    (cd "$BK" && find . -type f -printf '%s %p\n' 2>/dev/null | sort) > "$BK/manifest.txt"
    sha256sum "$BK/manifest.txt" > "$BK/manifest.txt.sha256"
    log "  backup completo en $BK ($(du -sh "$BK" 2>/dev/null | cut -f1))"
    # subir a Drive si hay rclone configurado
    if command -v rclone >/dev/null 2>&1; then
        log "  subiendo a $DRIVE/$STAMP ..."
        rclone copy "$BK" "$DRIVE/$STAMP" --stats 30s --stats-one-line >>"$LOG" 2>&1 && \
            rclone check "$BK" "$DRIVE/$STAMP" >>"$LOG" 2>&1 && \
            log "  subida VERIFICADA con rclone check" || log "  AVISO: la subida falló (revisa $LOG)"
    else
        log "  rclone no disponible: copia manual a Drive con la app o 'rclone copy $BK $DRIVE/$STAMP'"
    fi
    echo "$BK" > "$STATE"
    log "  estado guardado en $STATE"
}

# ---------------------------------------------------------------------
# Restauración (tras el flash)
# ---------------------------------------------------------------------
restore() {
    log "== RESTAURACIÓN POST-FLASH =="
    BK=$(cat "$STATE" 2>/dev/null)
    [ -n "$BK" ] && [ -d "$BK" ] || die "no hay backup en $STATE (ejecuta --backup antes)"
    log "  restaurando desde $BK"
    # 1) módulos Magisk
    for d in "$BK"/modules/modules/*/; do
        [ -d "$d" ] || continue
        id=$(basename "$d")
        [ -f "/data/adb/modules/$id/module.prop" ] && { log "  módulo $id ya presente: se conserva"; continue; }
        log "  instalando módulo $id"
        mkdir -p "/data/adb/modules/$id"
        cp -a "$d"/. "/data/adb/modules/$id/" 2>/dev/null
    done
    # 2) configuración de Magisk
    [ -d "$BK/config/magisk" ] && cp -a "$BK/config/magisk/." /data/adb/magisk/ 2>/dev/null
    log "  módulos restaurados. REINICIA y verifica antes de restaurar apps."
    log "  (las apps y sus datos se restauran con Swift Backup, no aquí)"
}

# ---------------------------------------------------------------------
# Verificación
# ---------------------------------------------------------------------
verify() {
    log "== VERIFICACIÓN =="
    echo "--- SELinux ---"; getenforce
    echo "--- Zygisk/DenyList ---"; magisk --sqlite "SELECT key,value FROM settings;" 2>/dev/null | head
    echo "--- Módulos ---"; ls -1 /data/adb/modules 2>/dev/null
    echo "--- Espacio ---"; df -h /data | tail -1
    echo "--- RAM ---"; free -m 2>/dev/null | head -2
    echo "--- NPU/Megadaemon ---"; command -v megadaemon-core >/dev/null 2>&1 && megadaemon-core npu-probe 2>/dev/null | head -20
    echo "--- Play Integrity keybox ---"; ls -l /data/adb/tricky_store/keybox.xml 2>/dev/null
    log "verificación registrada en $LOG"
}

case "$MODE" in
    --backup)   inventory; backup "$@" ;;
    --restore)  restore ;;
    --verify)   verify ;;
    --wipe)     log "WIPE solicitado: esto BORRA /data"; log "Usa este comando SÓLO con el flash de UN1CA delante y tras backup verificado."
               read -r -p "¿Seguro? (escribe SI): " a; [ "$a" = "SI" ] || die "abortado"
               recovery --wipe_data ;;
    *)          usage ;;
esac

log "fin ($MODE) — registro en $LOG"
