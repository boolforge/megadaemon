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
# Release: por defecto el ULTIMO publicado (v15.3).  Se puede fijar con
# RELEASE=<tag>.  MEDIDO el 2026-11-21: el release estaba FIJO a un binario
# viejo, asi que el "pegado unico" instalaba un daemon que ya no era el del
# repo => el usuario creia tener la ultima version y no la tenia.
RELEASE="${RELEASE:-latest}"
ASSET=megadaemon-core-android-arm64
REPO_URL="https://github.com/boolforge/megadaemon"
REBOOT="${REBOOT:-0}"
SHA256=""

# Descarga con curl o, si el movil NO tiene curl (medido: el movil puede no
# tenerlo), con el python3 de Termux/Antigravity.
PY=""
for c in /data/data/com.termux/files/usr/bin/python3 /data/data/com.agy/files/usr/bin/python3; do
  [ -x "$c" ] && PY="$c" && break
done
http_code() { # http_code <url> [valor-de-X-API-Key]
  if command -v curl >/dev/null 2>&1; then
    if [ -n "${2:-}" ]; then
      curl -s -o /dev/null -w '%{http_code}' --max-time 15 -H "X-API-Key: $2" "$1" 2>/dev/null || echo 000
    else
      curl -s -o /dev/null -w '%{http_code}' --max-time 15 "$1" 2>/dev/null || echo 000
    fi
  elif [ -n "$PY" ]; then
    "$PY" -c 'import sys,urllib.request as u,urllib.error
url=sys.argv[1]; hdr=sys.argv[2] if len(sys.argv)>2 else ""
req=u.Request(url, headers={"X-API-Key":hdr} if hdr else {})
try:
    print(u.urlopen(req,timeout=15).status)
except urllib.error.HTTPError as e: print(e.code)
except Exception: print("000")' "$1" "${2:-}"
  else
    echo 000
  fi
}
dl() { # dl <url> <destino>
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL -o "$2" "$1"
  elif [ -n "$PY" ]; then
    "$PY" -c "import sys,urllib.request as u; open(sys.argv[2],'wb').write(u.urlopen(sys.argv[1],timeout=120).read())" "$1" "$2"
  else
    return 127
  fi
}
say() { echo "[$(date +%H:%M:%S)] $*"; }
mkdir -p "$PREFIX"
exec >>"$LOG" 2>&1
say "=== phone_finalize: inicio (log en $LOG) ==="

# --- 0) credencial -----------------------------------------------------------
say "0) credencial"
KEYFILE=""
KEY=""
# La credencial puede estar en uno de estos sitios (no se sabe cual es el
# "bueno": por eso el daemon recibe 403 con la key correcta en la mano).
for cand in "$PREFIX/state/megadaemon_api.key" "$PREFIX/megadaemon_api.key" "$DEF/megadaemon_api.key"; do
  if [ -f "$cand" ]; then
    K2=$(tr -d '\r\n' < "$cand")
    [ -n "$K2" ] && { KEYFILE="$cand"; KEY="$K2"; break; }
  fi
done
if [ -n "$KEYFILE" ]; then
  say "   key encontrada en $KEYFILE"
  say "   sha16 $(printf '%s' "$KEY" | sha256sum | cut -c1-16)  (no se imprime el secreto)"
else
  say "   NO hay key en ninguno de los sitios conocidos:"
  say "     $PREFIX/state/megadaemon_api.key"
  say "     $PREFIX/megadaemon_api.key"
  say "     $DEF/megadaemon_api.key"
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
S=$(http_code http://127.0.0.1:9090/api/status "$KEY")
say "   api/status local con la key: $S   (200 = desbloqueado)"

# --- 1) binario --------------------------------------------------------------
say "1) binario oficial ($ASSET)"
# 1a) que release?  (latest => se consulta la API de GitHub)
if [ "$RELEASE" = "latest" ]; then
  if dl "https://api.github.com/repos/boolforge/megadaemon/releases/latest" "$PREFIX/.rel.json"; then
    R=$(grep -oE '"tag_name"[[:space:]]*:[[:space:]]*"[^"]+"' "$PREFIX/.rel.json" | head -1 | sed 's/.*"\(v[^"]*\)"/\1/')
    [ -n "$R" ] && RELEASE="$R" && say "   release mas reciente: $RELEASE"
  fi
  [ "$RELEASE" = "latest" ] && { say "   ABORTO: no se pudo resolver el release mas reciente"; RELEASE=""; }
fi
URL="$REPO_URL/releases/download/${RELEASE}/${ASSET}"
# 1b) el sha256 sale del propio release (fichero .sha256 publicado), no de
#     una constante aqui => si el release cambia, la comprobacion sigue
#     siendo valida (fail-closed si no cuadra).
if dl "$REPO_URL/releases/download/${RELEASE}/${ASSET}.sha256" "$PREFIX/.sha256"; then
  SHA256=$(awk '{print $1}' "$PREFIX/.sha256" | head -1)
fi
if [ -z "$SHA256" ]; then
  say "   ABORTO: no se pudo obtener el sha256 del release $RELEASE (se deja el binario actual)"
elif dl "$URL" "$PREFIX/megadaemon-core.new"; then
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
  dl https://raw.githubusercontent.com/boolforge/megadaemon/main/tools/omc_codec.py "$CODEC" && say "   omc_codec.py descargado"
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
say "   health local: $(http_code http://127.0.0.1:9090/health)"
say "   api/status  : $(http_code http://127.0.0.1:9090/api/status "$KEY")"
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
DPID=$(pidof megadaemon-core 2>/dev/null | awk '{print $1}')
if [ -n "$DPID" ]; then
  say "   env del daemon en ejecucion (PID $DPID):"
  tr '\0' '\n' < "/proc/$DPID/environ" 2>/dev/null \
    | grep -E '^MEGADAEMON_(API_KEY|STATE_DIR|REQUIRE_API_KEY)=' \
    | sed -e 's/^\(.*API_KEY=\).*/\1<oculto>/' | sed 's/^/     /'
  DKEY=$(tr '\0' '\n' < "/proc/$DPID/environ" 2>/dev/null | grep '^MEGADAEMON_API_KEY=' | cut -d= -f2-)
  [ -n "$DKEY" ] && say "     sha16 de la key en su env: $(printf '%s' "$DKEY" | sha256sum | cut -c1-16)"
  [ -z "$DKEY" ] && say "     el daemon NO tiene MEGADAEMON_API_KEY en su env => hereda la del fichero de estado"
  DSD=$(tr '\0' '\n' < "/proc/$DPID/environ" 2>/dev/null | grep '^MEGADAEMON_STATE_DIR=' | cut -d= -f2-)
  say "     state_dir efectivo: ${DSD:-$DEF (por defecto)}"
fi
say "   disco: $(df -h /data | tail -1)"
say "   RAM  : $(free -m | sed -n 2p)"
say "   carga: $(cat /proc/loadavg)"

# --- 5) tunel roto -> relanzar ---------------------------------------------
UV=$(cat "$PREFIX/state/tunnel.url" 2>/dev/null | tr -d '\r\n')
CODE=$(http_code "$UV/health")
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
