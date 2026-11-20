#!/system/bin/sh
# ---------------------------------------------------------------------------
# FINALIZACION DEL MOVIL EN UN SOLO PEGADO (Termux, con root).
#
#   curl -sL https://raw.githubusercontent.com/boolforge/megadaemon/main/deploy/phone_finalize.sh \
#        -o /data/local/tmp/megadaemon/phone_finalize.sh
#   su -mm -c 'sh /data/local/tmp/megadaemon/phone_finalize.sh'
#
# Pasos:
#   0) CREDENCIAL: copia el fichero de key al state_dir por defecto que mira
#      el daemon (/data/user/0/com.agy/files/megadaemon/state/). Sin esto, el
#      daemon responde 403 "API key invalida" a TODA la flota (la key es
#      correcta; el daemon simply no la encuentra).
#   1) Instala el binario ARM64 oficial (sha256 verificado, mv atomico,
#      chmod 755 DESPUES del mv) y reinicia SOLO el daemon (no cloudflared).
#   2) REPARCHE LEGAL DEL CSC: pone el valor CON AVISO
#      ('RecordingAllowedByMenu,disclaimer_latin'). El valor a secas
#      ('RecordingAllowed') deja la grabacion NATIVA AUTOMATICA Y SILENCIOSA
#      => en Espana es delito (art. 197 CP) y vulnera el GDPR.
#   3) Reinicia el daemon y comprueba /health y el tunel (lo relanza si esta
#      muerto: 530/000).
#   4) Publica las mediciones: CSC, sha256 del cscfeature servido, valor de la
#      feature, logcat del framework, binario, disco y RAM.
#   5) Reinicio del sistema SOLO si se pide con REBOOT=1 (lo hace falta para
#      que el framework lea el cscfeature parcheado).
#
# Revertir el modulo CSC: touch /data/adb/modules/megadaemon_csc_recording/disable
# y reiniciar.
# ---------------------------------------------------------------------------
PREFIX=/data/local/tmp/megadaemon
MOD=/data/adb/modules/megadaemon_csc_recording
LOG=$PREFIX/phone_finalize.log
DEF=/data/user/0/com.agy/files/megadaemon/state
RELEASE=va82941ca63e5dbc244d462b7f805290e5e11a063
ASSET=megadaemon-core-android-arm64
URL="https://github.com/boolforge/megadaemon/releases/download/${RELEASE}/${ASSET}"
SHA256=718c1c59ccf6b6bfcf15955a9f2c27c46b579e07141ddcce3242c03daba70751
REBOOT="${REBOOT:-0}"
say() { echo "[$(date +%H:%M:%S)] $*"; }
mkdir -p "$PREFIX"
exec >>"$LOG" 2>&1
say "=== phone_finalize: inicio (log en $LOG) ==="

# --- 0) credencial -----------------------------------------------------------
say "0) credencial"
KEYFILE="$PREFIX/state/megadaemon_api.key"
KEY=""
if [ -f "$KEYFILE" ]; then
  KEY=$(tr -d '\r\n' < "$KEYFILE")
  say "   key en el estado: sha16 $(printf '%s' "$KEY" | sha256sum | cut -c1-16)"
else
  say "   no hay $KEYFILE"
fi
mkdir -p "$DEF" "$PREFIX/state"
[ -f "$KEYFILE" ] && { cp -f "$KEYFILE" "$DEF/megadaemon_api.key"; chmod 600 "$DEF/megadaemon_api.key"; say "   copiada a $DEF"; }
[ -f "$PREFIX/state/megadaemon_api.keys" ] && { cp -f "$PREFIX/state/megadaemon_api.keys" "$DEF/megadaemon_api.keys"; chmod 600 "$DEF/megadaemon_api.keys"; }
ENVF="$PREFIX/megadaemon.env"
if [ -n "$KEY" ] && [ -f "$ENVF" ]; then
  cp "$ENVF" "$ENVF.bak" 2>/dev/null
  sed -i "s|^MEGADAEMON_API_KEY=.*|MEGADAEMON_API_KEY=$KEY|" "$ENVF"
  chmod 600 "$ENVF"
  say "   megadaemon.env sincronizado"
fi
S=$(curl -s -o /dev/null -w '%{http_code}' -H "X-API-Key: $KEY" http://127.0.0.1:9090/api/status 2>/dev/null || echo 000)
say "   api/status local con la key: $S   (200 = desbloqueado)"

# --- 1) binario --------------------------------------------------------------
say "1) binario oficial ($ASSET)"
if curl -fsSL -o "$PREFIX/megadaemon-core.new" "$URL"; then
  GOT=$(sha256sum "$PREFIX/megadaemon-core.new" | cut -d' ' -f1)
  if [ "$GOT" = "$SHA256" ]; then
    cp "$PREFIX/megadaemon-core" "$PREFIX/megadaemon-core.prev" 2>/dev/null
    mv "$PREFIX/megadaemon-core.new" "$PREFIX/megadaemon-core"
    chmod 755 "$PREFIX/megadaemon-core"
    say "   instalado (sha256 verificado)"
  else
    say "   ABORTO: sha256 $GOT != $SHA256 (se deja el binario actual)"
    rm -f "$PREFIX/megadaemon-core.new"
  fi
else
  say "   no se pudo descargar (se deja el actual)"
fi

# --- 2) reparche legal del CSC ----------------------------------------------
say "2) reparche del CSC con el valor QUE AVISA"
CODEC=/data/local/tmp/megadaemon/omc_codec.py
if [ ! -f "$CODEC" ]; then
  curl -fsSL -o "$CODEC" https://raw.githubusercontent.com/boolforge/megadaemon/main/tools/omc_codec.py && say "   omc_codec.py descargado"
fi
PY=/data/data/com.termux/files/usr/bin/python3
[ -x "$PY" ] || PY=python3
$PY - <<'PYX' 2>&1 | sed 's/^/   /'
import os, re, sqlite3, sys, gzip
CSC="AMO"; FEAT="CscFeature_VoiceCall_ConfigRecording"
VAL="RecordingAllowedByMenu,disclaimer_latin"
MOD="/data/adb/modules/megadaemon_csc_recording"
SRC=f"/optics/configs/carriers/single/{CSC}/conf/system/cscfeature.xml"
DB=f"/prism/etc/carriers/single/{CSC}/contents.db"
sys.path.insert(0,"/data/local/tmp/megadaemon")
try:
    import omc_codec
except Exception as e:
    print("no se pudo importar omc_codec:", e); raise SystemExit(0)
fuente=os.path.join(MOD,"cscfeature.xml")
if not os.path.exists(fuente): fuente=SRC
try:
    xml=omc_codec.decode_file(fuente).decode("utf-8","replace")
except Exception as e:
    print("no se pudo decodificar:", e); raise SystemExit(0)
m=re.search(re.escape(FEAT)+r">([^<]*)<", xml)
print("valor actual:", m.group(1) if m else "AUSENTE")
nuevo=re.sub(re.escape(FEAT)+r">[^<]*<", f"{FEAT}>{VAL}<", xml, count=1)
if m is None:
    nuevo=nuevo.replace("</root>", f"<{FEAT}>{VAL}</{FEAT}></root>")
datos=omc_codec.omc_encode(gzip.compress(nuevo.encode("utf-8")))
if os.path.exists(DB):
    con=sqlite3.connect(DB); cur=con.cursor()
    for _id,_old in cur.execute("SELECT _id,fileSize FROM contents WHERE filePath LIKE ?", ("%cscfeature.xml",)).fetchall():
        cur.execute("UPDATE contents SET fileSize=? WHERE _id=?", (len(datos), _id))
        print(f"contents.db: _id={_id} fileSize -> {len(datos)}")
    con.commit(); con.close()
os.makedirs(MOD, exist_ok=True)
open(os.path.join(MOD,"cscfeature.xml"),"wb").write(datos)
print("modulo actualizado:", len(datos), "B  valor:", VAL)
PYX

# --- 3) reinicio del daemon -------------------------------------------------
say "3) reinicio SOLO del daemon (cloudflared no se toca)"
PID=$(pidof megadaemon-core 2>/dev/null || echo 0)
[ "$PID" != "0" ] && { say "   parando $PID"; kill "$PID"; }
sleep 8
if [ "$(pidof megadaemon-core 2>/dev/null || echo 0)" = "0" ]; then
  setsid nohup sh /data/adb/service.d/99-megadaemon.sh >/sdcard/megadaemon.log 2>&1 &
  sleep 15
fi
say "   pid: $(pidof megadaemon-core 2>/dev/null || echo 0)  cloudflared: $(pidof cloudflared 2>/dev/null || echo 'no corre')"

# --- 4) mediciones -----------------------------------------------------------
say "4) mediciones"
say "   health local: $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:9090/health 2>/dev/null)"
say "   api/status  : $(curl -s -o /dev/null -w '%{http_code}' -H "X-API-Key: $KEY" http://127.0.0.1:9090/api/status 2>/dev/null)"
say "   tunel       : $(cat $PREFIX/state/tunnel.url 2>/dev/null || cat $PREFIX/tunnel.url 2>/dev/null)"
say "   ro.csc.sales_code: $(getprop ro.csc.sales_code)"
say "   cscfeature servido: $(sha256sum /optics/configs/carriers/single/AMO/conf/system/cscfeature.xml 2>/dev/null | cut -d' ' -f1)"
say "   modulo sha256    : $(sha256sum $MOD/cscfeature.xml 2>/dev/null | cut -d' ' -f1)"
say "   valor de la feature:"
$PY - <<'PYX' 2>/dev/null | sed 's/^/     /'
import re,sys
sys.path.insert(0,"/data/local/tmp/megadaemon")
import omc_codec
try:
    x=omc_codec.decode_file("/data/adb/modules/megadaemon_csc_recording/cscfeature.xml").decode("utf-8","replace")
    m=re.search(r"CscFeature_VoiceCall_ConfigRecording>([^<]*)<",x)
    print(m.group(1) if m else "AUSENTE")
except Exception as e: print("error:", e)
PYX
say "   mount del cscfeature:"; mount | grep cscfeature | sed 's/^/     /'
say "   logcat (CSC):"; logcat -d -t 600 2>/dev/null | grep -iE "SemCscFeature|cscfeature" | tail -5 | sed 's/^/     /'
say "   binario sha256: $(sha256sum $PREFIX/megadaemon-core 2>/dev/null | cut -d' ' -f1)  (oficial: $SHA256)"
say "   disco: $(df -h /data | tail -1)"
say "   RAM  : $(free -m | sed -n 2p)"
say "   carga: $(cat /proc/loadavg)"

# --- 5) tunel roto -> relanzar ---------------------------------------------
UV=$(cat "$PREFIX/state/tunnel.url" 2>/dev/null | tr -d '\r\n')
CODE=$(curl -s -o /dev/null -w '%{http_code}' --max-time 12 "$UV/health" 2>/dev/null || echo 000)
if [ -z "$UV" ] || [ "$CODE" = "530" ] || [ "$CODE" = "000" ]; then
  say "5) tunel no responde ($CODE) -> relanzando cloudflared"
  pkill -f cloudflared 2>/dev/null; sleep 3
  setsid nohup cloudflared tunnel --url http://127.0.0.1:9090 --no-autoupdate > "$PREFIX/state/cloudflared.log" 2>&1 &
  sleep 25
  NEWURL=$(grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' "$PREFIX/state/cloudflared.log" 2>/dev/null | head -1)
  [ -n "$NEWURL" ] && { printf '%s\n' "$NEWURL" > "$PREFIX/state/tunnel.url"; say "   nuevo tunel: $NEWURL"; }
fi

say "=== fin (log en $LOG) ==="
if [ "$REBOOT" = "1" ]; then
  say "REBOOT=1 -> reiniciando el sistema (para que el framework lea el cscfeature parcheado)"
  su -mm -c "svc power reboot"
else
  say "NO se reinicia (REBOOT=1 para reiniciar tras el reparche del CSC)"
fi
