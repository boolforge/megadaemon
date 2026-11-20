#!/system/bin/sh
# Parchea el cscfeature.xml del CSC actual para habilitar la grabacion nativa
# de llamadas y crea el modulo Magisk systemless que lo monta.
#
#   sh deploy/patch_csc_recording.sh            # dry-run (no escribe nada)
#   sh deploy/patch_csc_recording.sh --apply    # escribe el modulo en el movil
#
# Se ejecuta EN el movil (necesita root y el fichero original en /optics).
# El algoritmo OMC (rotacion + XOR posicional + gzip) esta en Rust
# (rust-core/src/rom_migration.rs) y en Python (tools/omc_codec.py).
set -e
CSC=AMO
FEAT=CscFeature_VoiceCall_ConfigRecording
VAL=RecordingAllowed
SRC=/optics/configs/carriers/single/$CSC/conf/system/cscfeature.xml
DB=/prism/etc/carriers/single/$CSC/contents.db
MOD=/data/adb/modules/megadaemon_csc_recording
PY=/data/data/com.termux/files/usr/bin/python3
APPLY=0
[ "$1" = "--apply" ] && APPLY=1

echo "CSC=$CSC  fichero=$SRC"
[ -f "$SRC" ] || { echo "no existe el fichero original"; exit 1; }
echo "sha256 original: $(sha256sum $SRC | cut -d' ' -f1)"

if [ "$APPLY" = "1" ]; then
  $PY /data/local/tmp/megadaemon/patch_csc.py
  mkdir -p $MOD
  cp /data/local/tmp/megadaemon/csc_patch/cscfeature.xml $MOD/cscfeature.xml
  cp /data/local/tmp/megadaemon/csc_patch/contents.db $MOD/contents.db
  cp /data/adb/modules/megadaemon_csc_recording/module.prop $MOD/module.prop 2>/dev/null || true
  chmod 0755 $MOD/post-fs-data.sh 2>/dev/null || true
  echo "modulo listo en $MOD (se activa en el siguiente reinicio)"
else
  echo "dry-run: no se escribe nada (usa --apply)"
fi
