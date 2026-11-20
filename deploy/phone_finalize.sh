#!/system/bin/sh
# ---------------------------------------------------------------------------
# Finalizacion del movil en UN solo pegado (desde Termux, con root).
#
#   su -mm -c 'sh /data/local/tmp/megadaemon/phone_finalize.sh'
#
# Que hace, en orden:
#   1. Cierra Chrome (531 MB => es lo que tumba el movil: load 17,6).
#   2. Instala el binario oficial de la release (verifica sha256, mv atomico,
#      chmod 755 DESPUES del mv) y reinicia el daemon.
#   3. Reparchea el modulo CSC con el valor QUE AVISA (legal en Espana:
#      'RecordingAllowedByMenu,disclaimer_latin' y NO 'RecordingAllowed'
#      a secas, que graba en silencio => art. 197 CP + GDPR).
#   4. Mide y publica: URL del tunel (la lee del movil, no hardcodeada),
#      salud del daemon, CSC, logcat del framework y el sha256 del fichero
#      de CSC servido.
#
# NO es destructivo: el modulo CSC se puede revertir con
#   touch /data/adb/modules/megadaemon_csc_recording/disable && reboot
# ---------------------------------------------------------------------------
set -e
PREFIX=/data/local/tmp/megadaemon
MOD=/data/adb/modules/megadaemon_csc_recording
LOG=$PREFIX/phone_finalize.log
say() { echo "[$(date +%H:%M:%S)] $*"; }
exec >>"$LOG" 2>&1
say "=== inicio ==="

# 1) liberar memoria
say "cerrando Chrome (si esta abierto)"
am force-stop com.chrome 2>/dev/null || true
am force-stop com.chrome.beta 2>/dev/null || true

# 2) binario oficial
RELEASE=va82941ca63e5dbc244d462b7f805290e5e11a063
ASSET=megadaemon-core-android-arm64
URL="https://github.com/boolforge/megadaemon/releases/download/${RELEASE}/${ASSET}"
SHA256=718c1c59ccf6b6bfcf15955a9f2c27c46b579e07141ddcce3242c03daba70751
say "descargando $ASSET"
if curl -fsSL -o "$PREFIX/megadaemon-core.new" "$URL"; then
  GOT=$(sha256sum "$PREFIX/megadaemon-core.new" | cut -d' ' -f1)
  if [ "$GOT" = "$SHA256" ]; then
    mv "$PREFIX/megadaemon-core.new" "$PREFIX/megadaemon-core"
    chmod 755 "$PREFIX/megadaemon-core"
    say "binario instalado (sha256 verificado)"
  else
    say "ABORTO: sha256 $GOT != $SHA256 (se deja el binario actual)"
    rm -f "$PREFIX/megadaemon-core.new"
  fi
else
  say "no se pudo descargar el binario (se deja el actual)"
fi

# 3) reparche legal del CSC
PY=/data/data/com.termux/files/usr/bin/python3
[ -x "$PY" ] || PY=python3
if [ -f "$PREFIX/patch_csc_value.py" ]; then
  say "reparcheando el modulo CSC con el valor con aviso"
  $PY "$PREFIX/patch_csc_value.py" "RecordingAllowedByMenu,disclaimer_latin" || say "fallo el reparche"
else
  say "falta $PREFIX/patch_csc_value.py (hay que subirlo desde el repo)"
fi

# 4) reiniciar el daemon
PID=$(pidof megadaemon-core || echo 0)
[ "$PID" != "0" ] && { say "parando el daemon $PID"; kill "$PID"; }
sleep 5
setsid nohup sh /data/adb/service.d/99-megadaemon.sh >/sdcard/megadaemon.log 2>&1 &
sleep 15

# 5) mediciones
say "=== VERIFICACION ==="
say "pid: $(pidof megadaemon-core || echo 0)"
say "health: $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:9090/health 2>/dev/null)"
say "tunel (leido del movil, no hardcodeado): $(cat $PREFIX/tunnel.url 2>/dev/null || cat $PREFIX/state/tunnel.url 2>/dev/null)"
say "ro.csc.sales_code: $(getprop ro.csc.sales_code)"
say "cscfeature servido: $(sha256sum /optics/configs/carriers/single/AMO/conf/system/cscfeature.xml 2>/dev/null | cut -d' ' -f1)"
say "valor de la feature: $($PY - <<'PYX' 2>/dev/null
import sys
sys.path.insert(0, "/data/local/tmp/megadaemon")
try:
    import omc_codec, re
    x = omc_codec.decode_file("/data/adb/modules/megadaemon_csc_recording/cscfeature.xml").decode("utf-8","replace")
    m = re.search(r"CscFeature_VoiceCall_ConfigRecording>([^<]*)<", x)
    print(m.group(1) if m else "AUSENTE")
except Exception as e:
    print("sin leer:", e)
PYX
)"
say "logcat del framework (CSC):"
logcat -d -t 400 2>/dev/null | grep -iE "SemCscFeature|cscfeature" | tail -5 | sed 's/^/    /'
say "=== fin (log completo en $LOG) ==="
