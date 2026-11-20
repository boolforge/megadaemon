#!/system/bin/sh
# Anade la grabacion nativa de llamadas al CSC actual (AMO) SIN cambiar de CSC.
#
# Magisk ejecuta post-fs-data.sh antes de que arranque zygote => el framework
# (com.samsung.android.feature.SemCscFeature, en framework.jar -> classes5.dex)
# lee el cscfeature.xml YA parcheado. El fichero original no se toca: esto es
# un bind-mount => revertir = desactivar el modulo (touch .../disable) y reiniciar.
#
# El valor NO es "true": los 19 CSC que traen la bandera usan
# "RecordingAllowed" (o "RecordingAllowedByMenu,disclaimer_latin").
MOD=/data/adb/modules/megadaemon_csc_recording
LOG=/data/local/tmp/megadaemon/csc_patch.log
TARGET=/optics/configs/carriers/single/AMO/conf/system/cscfeature.xml
DB_TARGET=/prism/etc/carriers/single/AMO/contents.db
echo "--- $(date) inicio" >> $LOG
[ -f "$TARGET" ] || { echo "no existe $TARGET: nada que hacer" >> $LOG; exit 0; }
if mount --bind "$MOD/cscfeature.xml" "$TARGET" 2>>$LOG; then
  echo "OK bind cscfeature.xml -> $TARGET" >> $LOG
else
  echo "FALLO bind cscfeature.xml" >> $LOG
fi
# contents.db guarda el fileSize de cada fichero del CSC: si no se actualiza,
# el inventario del CSC no cuadra con el fichero parcheado.
if [ -f "$MOD/contents.db" ]; then
  if mount --bind "$MOD/contents.db" "$DB_TARGET" 2>>$LOG; then
    echo "OK bind contents.db -> $DB_TARGET" >> $LOG
  else
    echo "FALLO bind contents.db" >> $LOG
  fi
fi
