#!/system/bin/sh
# ---------------------------------------------------------------------------
# INSTALA LOS BINARIOS ARM64 PENDIENTES (ya subidos al móvil, NO se compila aquí)
#
# Ejecutar como root en el móvil, DESPUÉS de tener canal:
#     su -mm -c 'sh /data/local/tmp/megadaemon/install_pending.sh'
#
# Espera estos dos ficheros en /data/local/tmp/megadaemon/ (subidos con
# push_to_phone.py o adb):
#     megadaemon-core.d93fa217   (canje remoto arreglado)
#     md-coord.0a7734e7          (fix de WAL del tablero)
#
# Reglas:
#   * `mv` NUNCA `cp` sobre un binario en ejecución ("Text file busy");
#   * sha256 antes y después de instalar;
#   * reinicia SÓLO el daemon (no el supervisor, no el puente) y espera;
#   * verifica /health y /api/status en 3000 y 9090;
#   * NO compila nada y NO reinicia cloudflared (el túnel sobrevive al reinicio
#     del daemon porque proxya a 127.0.0.1:9090).
# ---------------------------------------------------------------------------
LOG=/data/local/tmp/megadaemon
cd "$LOG" || exit 1

sha() { sha256sum "$1" 2>/dev/null | cut -c1-16; }

# El sha esperado llega en un fichero .sha256 junto al binario (lo escribe quien
# lo sube). Si no existe, se usa el valor por defecto de la lista de abajo.
install_one() {
  src="$1"; want="$2"; dest="$3"
  [ -f "$src" ] || { echo "   FALTA $src (no se instala $dest)"; return 1; }
  got=$(sha "$src")
  if [ -f "$src.sha256" ]; then
    want=$(cut -c1-16 "$src.sha256" 2>/dev/null)
  fi
  if [ "$got" != "$want" ]; then
    echo "   SHA DISCREPANTE en $src: $got != $want -> no se instala"
    return 1
  fi
  echo "   $src sha $got ✓"
  mv "$src" "$dest" || { echo "   mv falló ($dest en uso?)"; return 1; }
  echo "   instalado: $dest (sha $(sha "$dest"))"
}

echo "== 1) md-coord (fix de WAL) =="
install_one "$LOG/md-coord" "0a7734e781140ab7d" "$LOG/md-coord"

echo "== 2) megadaemon-core (canje remoto) =="
install_one "$LOG/megadaemon-core" "d93fa2172a74ab17" "$LOG/megadaemon-core"

echo "== 3) reinicio SÓLO del daemon =="
pkill -x megadaemon-core 2>/dev/null
sleep 3
i=0
while [ $i -lt 25 ]; do
  if grep -q ':2382 ' /proc/net/tcp 2>/dev/null || grep -q ':2382 ' /proc/net/tcp6 2>/dev/null; then
    echo "   daemon de vuelta en 9090 ✓"
    break
  fi
  sleep 1
  i=$((i + 1))
done

echo "== 4) verificación =="
free -m 2>/dev/null | head -3
LD_LIBRARY_PATH=/data/data/com.termux/files/usr/lib \
  /data/data/com.termux/files/usr/bin/python3 - <<'PY'
import urllib.request, urllib.error
def g(u):
    try:
        r=urllib.request.urlopen(u, timeout=10); return "HTTP %s %dB" % (r.status, len(r.read()))
    except urllib.error.HTTPError as e: return "HTTP %s" % e.code
    except Exception as e: return "ERR %s" % repr(e)[:40]
for u in ["http://127.0.0.1:9090/health","http://127.0.0.1:3000/","http://127.0.0.1:3000/api/status",
          "http://127.0.0.1:9090/api/session/exchange"]:
    print("  %-46s -> %s" % (u, g(u)))
PY
echo "== FIN =="
