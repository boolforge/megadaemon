#!/system/bin/sh
# Reparchea el modulo CSC con el valor QUE AVISA (legal en Espana).
#   sh deploy/repatch_csc_disclaimer.sh
set -e
PY=/data/data/com.termux/files/usr/bin/python3
[ -x "$PY" ] || PY=python3
$PY /data/local/tmp/megadaemon/patch_csc_value.py \
    "RecordingAllowedByMenu,disclaimer_latin"
echo "---"
echo "modulo: /data/adb/modules/megadaemon_csc_recording/cscfeature.xml"
echo "sha256: $(sha256sum /data/adb/modules/megadaemon_csc_recording/cscfeature.xml | cut -d' ' -f1)"
