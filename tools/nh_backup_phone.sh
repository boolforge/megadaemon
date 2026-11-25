#!/data/data/com.termux/files/usr/bin/bash
# Backup 100% en STREAM del S21 Ultra a Google Drive con rclone (desde el propio movil).
#
#   su u0_a622 -c 'bash /data/local/tmp/nh_backup.sh <fase>'
#
# Fases: media | estado | userdata | apps | parts | manifest
#  - nunca escribe una copia local: rclone sube cada fichero a medida que lo lee
#  - el token de Drive vive en $HOME/.config/rclone/rclone.conf (0600)
#  - las particiones se streamean con dd | rclone rcat (cero staging)
set -u
PREFIX=/data/data/com.termux/files/usr
export PATH=$PREFIX/bin:$HOME/bin:$PATH
export HOME=/data/data/com.termux/files/home
export RCLONE_CONFIG=$HOME/.config/rclone/rclone.conf
PHASE="${1:-media}"
REMOTE="${REMOTE:-gdrive:Backup-S21Ultra-20261120}"
LOG=/data/local/tmp/nh_backup_${PHASE}.log

say() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }

check() {
  [ -f "$RCLONE_CONFIG" ] || { say "ABORT: falta $RCLONE_CONFIG (autoriza rclone primero)"; exit 1; }
  rclone about "$REMOTE" --json >/dev/null 2>&1 || { say "ABORT: el remoto no responde"; rclone about "$REMOTE" 2>&1 | head -3; exit 1; }
  say "remoto OK: $REMOTE"
  rclone about "$REMOTE" 2>&1 | head -4 | tee -a "$LOG"
}

common() {
  # 4 transferencias, checksum (Drive compara MD5), sin limite de velocidad
  RCLONE_OPTS=(--transfers 4 --checksum --no-traverse --stats 30s --stats-one-line \
               --log-file "$LOG" --log-level INFO --retries 10 --low-level-retries 20 \
               --order-by size,desc)
}

case "$PHASE" in
  media)     # datos del usuario (86 GB) - prioridad 1
    check; common
    say "fase media: /data/media/0 -> $REMOTE/data/media"
    rclone copy /data/media/0 "$REMOTE/data/media" "${RCLONE_OPTS[@]}"
    ;;
  estado)    # /data/adb (Magisk), /data/system, /data/local
    check; common
    for d in adb system local per_boot; do
      say "fase estado: /data/$d"
      rclone copy "/data/$d" "$REMOTE/data/$d" "${RCLONE_OPTS[@]}"
    done
    ;;
  userdata)  # /data/user + /data/data (47 GB)
    check; common
    say "fase userdata: /data/user"
    rclone copy /data/user "$REMOTE/data/user" "${RCLONE_OPTS[@]}"
    ;;
  apps)      # /data/app (88 GB): APK instalados
    check; common
    say "fase apps: /data/app"
    rclone copy /data/app "$REMOTE/data/app" "${RCLONE_OPTS[@]}"
    ;;
  parts)     # imagenes de particion en stream (dd | rclone rcat), sin staging
    check
    mkdir -p /data/local/tmp/parts
    for p in super system vendor product odm prism optics boot init_boot vendor_boot \
             vbmeta vbmeta_system vbmeta_vendor efs keyrefuge metadata; do
      blk=$(readlink -f /dev/block/by-name/$p 2>/dev/null)
      [ -n "$blk" ] && [ -b "$blk" ] || { say "skip $p (no existe)"; continue; }
      sz=$(blockdev --getsize64 "$blk" 2>/dev/null || echo 0)
      say "particion $p ($blk, $sz bytes) -> stream"
      dd if="$blk" bs=4M 2>/dev/null | rclone rcat "$REMOTE/partitions/$p.img" \
         --drive-chunk-size 32M --stats 60s --stats-one-line
      say "particion $p subida"
    done
    ;;
  manifest)
    check
    say "generando manifiesto"
    { echo "# Backup S21 Ultra - $(date -u +%Y-%m-%dT%H:%M:%SZ)"
      echo "# origen: dispositivo /data + particiones"
      rclone lsf "$REMOTE" --recursive --files-only --format "pst" 2>/dev/null \
        | awk '{print $3"  "$1}' | tee -a "$LOG"
    } > /data/local/tmp/parts/MANIFEST.txt
    rclone copy /data/local/tmp/parts/MANIFEST.txt "$REMOTE/MANIFEST.txt"
    say "manifiesto subido"
    ;;
  *) say "fase desconocida: $PHASE"; exit 1;;
esac
say "fase $PHASE terminada"
