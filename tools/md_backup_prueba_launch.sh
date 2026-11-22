#!/bin/sh
# Lanza la prueba real del backup en segundo plano (para que sobreviva al
# `root_exec`, que tiene tope de 300 s y la prueba tarda media hora).
#
# Uso: sh md_backup_prueba_launch.sh [binario] [carpeta_drive] [subcarpeta]
set -u

BIN="${1:-/data/local/tmp/megadaemon/md-backup}"
FOLDER="${2:-15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0}"
SUBFOLDER="${3:-P0-apps}"
PREFIX=/data/local/tmp/megadaemon

cd "$PREFIX" || exit 1
rm -rf "$PREFIX/backup/prueba_p0" "$PREFIX/backup/restore_prueba" "$PREFIX/backup/fetch_p0"
: > "$PREFIX/nohup_prueba.log"

setsid nohup sh "$PREFIX/md_backup_prueba.sh" "$BIN" "$FOLDER" "$SUBFOLDER" \
    > "$PREFIX/nohup_prueba.log" 2>&1 < /dev/null &
echo "lanzado"
sleep 2
echo "md-backup en marcha: $(pidof md-backup 2>/dev/null | tr '\n' ' ')"
cat /proc/loadavg
