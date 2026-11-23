#!/usr/bin/env bash
# =====================================================================
# estado-movil.sh — DIAGNÓSTICO DEL MÓVIL, en un solo comando.
#
# Se ejecuta EN EL MÓVIL (Termux, sin root: usa `su -mm` sólo para leer
# ficheros de root) y devuelve TODO lo que necesito para reconectar:
#   * la URL REAL del túnel público (con el fallo del grep corregido:
#     `api.trycloudflare.com` NO es un túnel);
#   * si el daemon responde y con QUÉ build está en servicio;
#   * si el supervisor y el keeper están instalados y vivos;
#   * el estado del auto-actualizador (qué versión tiene y cuál es la nueva).
#
# USO (en Termux, en el móvil):
#     bash estado-movil.sh
# y pegar la salida.
# =====================================================================
set -u

PREFIX=/data/local/tmp/megadaemon
SERVICE_D=/data/adb/service.d
LOG=/sdcard/megadaemon.log

r() { su -mm -c "$1" 2>/dev/null; }

echo "=== 1) URL REAL del túnel público ==="
URL=$(r "grep -ao 'https://[a-z0-9-]*\.trycloudflare\.com' $PREFIX/tunnel.log 2>/dev/null | grep -v '^https://api\.' | tail -1")
if [ -z "$URL" ]; then
    URL=$(r "cat $PREFIX/tunnel.url 2>/dev/null")
fi
echo "tunel_url_file: $(r "cat $PREFIX/tunnel.url 2>/dev/null")"
echo "tunel_real    : ${URL:-NO ENCONTRADO}"
echo "--- todas las candidatas que hay en el log ---"
r "grep -ao 'https://[a-z0-9-]*\.trycloudflare\.com' $PREFIX/tunnel.log 2>/dev/null | sort -u" | head -6

echo
echo "=== 2) ¿responde el daemon? (build en servicio) ==="
H=$(curl -s --max-time 8 http://127.0.0.1:9090/health 2>/dev/null)
if [ -z "$H" ]; then
    H=$(toybox wget -q -O - --timeout 8 http://127.0.0.1:9090/health 2>/dev/null)
fi
if [ -z "$H" ]; then
    echo "el daemon NO responde en 127.0.0.1:9090 (puerto ocupado? proceso muerto?)"
    echo "procesos: $(r "pgrep -af megadaemon-core | head -3")"
else
    echo "$H" | head -c 700
    echo
fi

echo
echo "=== 3) supervisor / keeper / procesos ==="
echo "supervisor: $(r "ls -l $SERVICE_D/99-megadaemon.sh 2>/dev/null" | tr -s ' ')"
echo "keeper   : $(r "ls -l $SERVICE_D/97-megadaemon-keeper.sh 2>/dev/null" | tr -s ' ')"
echo "procesos : $(r "pgrep -af 'megadaemon-core|99-megadaemon|cloudflared' | head -6" | tr '\n' ' ')"

echo
echo "=== 4) auto-actualizador ==="
echo "intervalo: $(r "grep -h MD_AUTO_UPDATE_INTERVAL_MIN $PREFIX/megadaemon.env 2>/dev/null")"
echo "sha del binario en servicio: $(r "sha256sum $PREFIX/megadaemon-core 2>/dev/null | cut -d' ' -f1")"
echo "ultimas lineas de auto_update.log:"
r "tail -6 $PREFIX/state/auto_update.log 2>/dev/null" | sed 's/^/   /'

echo
echo "=== 5) ultimas lineas del log del daemon (¿reinicios?) ==="
r "tail -12 $LOG 2>/dev/null" | sed 's/^/   /'
