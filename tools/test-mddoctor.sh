#!/usr/bin/env sh
# =============================================================================
# test-mddoctor.sh — banco de pruebas del núcleo forense en C (mddoctor)
# =============================================================================
#
#   POR QUÉ ESTE SCRIPT EXISTE
#   --------------------------
#   `mddoctor` se publica en el móvil y se usa CUANDO el daemon está roto.
#   Si su SHA-256 o su cliente HTTP estuvieran mal, el verificador de "la
#   build en servicio es la nueva" daría una falsa confianza: PEOR que no
#   verificar.  Por eso el banco contrasta el hash contra `sha256sum`, el
#   cliente HTTP contra un servidor real y el parser del túnel contra un log
#   con trampas (api.trycloudflare.com NO es un túnel).
#
#   No necesita red: el servidor es un python local.
#
#   USO:  tools/test-mddoctor.sh
#   SALIDA: 0 si todo verde; 1 si hay algun fallo.
# =============================================================================

set -eu
RAIZ="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
TMP="${TMPDIR:-/tmp}/mddoctor-t$$"
PUERTO="${MDDOCTOR_TEST_PORT:-18099}"
mkdir -p "$TMP"
trap 'rm -rf "$TMP"; [ -n "${SRV_PID:-}" ] && kill "$SRV_PID" 2>/dev/null || true' EXIT

CC="${CC:-gcc}"

# --- Fixtures --------------------------------------------------------------
: > "$TMP/vacio"
printf 'abc' > "$TMP/abc"
head -c 100000 /dev/urandom > "$TMP/grande"
i=1
: > "$TMP/lineas"
while [ "$i" -le 20 ]; do echo "linea$i" >> "$TMP/lineas"; i=$((i + 1)); done

# Un tunnel.log con trampas: la API de Cloudflare NO es un túnel y hay dos
# candidatos buenos (el que vale es el ÚLTIMO, como el `tail -1` del script).
mkdir -p "$TMP/prefix/state" "$TMP/service"
cat > "$TMP/prefix/tunnel.log" <<'EOF'
2026-11-20T10:00:00Z | url: https://api.trycloudflare.com/tunnel
2026-11-20T10:00:01Z | url: https://abc-def.trycloudflare.com
2026-11-20T10:05:00Z | url: https://segundo-tunel.trycloudflare.com
EOF
printf 'https://abc-def.trycloudflare.com\n' > "$TMP/prefix/tunnel.url"
printf '#!/system/bin/sh\n' > "$TMP/service/99-megadaemon.sh"
printf '#!/system/bin/sh\n' > "$TMP/service/97-megadaemon-keeper.sh"

# --- Servidor HTTP de prueba ----------------------------------------------
cat > "$TMP/srv.py" <<EOF
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
class H(BaseHTTPRequestHandler):
    def do_GET(self):
        b = json.dumps({"status": "ok", "build": {"short": "abc1234", "sha": "deadbeef"}}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)
    def log_message(self, *a):
        pass
HTTPServer(("127.0.0.1", $PUERTO), H).serve_forever()
EOF
python3 "$TMP/srv.py" &
SRV_PID=$!
# Espera activa: si el servidor no levanta, el test diria que el HTTP falla
# y no que el banco esta mal preparado.
i=0
while [ "$i" -lt 40 ]; do
    if python3 -c "import socket,sys; s=socket.socket(); s.settimeout(0.3); sys.exit(0 if s.connect_ex(('127.0.0.1',$PUERTO))==0 else 1)"; then
        break
    fi
    i=$((i + 1))
    sleep 0.1
done

# --- Compilar y ejecutar ---------------------------------------------------
"$CC" -O2 -Wall -o "$TMP/test" "$RAIZ/tools/test-mddoctor.c"
# Un ELF de verdad para probar la cabecera (el propio binario sirve).
cp "$TMP/test" "$TMP/elf_real"
MDDOCTOR_TEST_FIX="$TMP" MDDOCTOR_TEST_PORT="$PUERTO" "$TMP/test"

# --- GATE DE TAMAÑO -------------------------------------------------------
# El usuario fijó un tope ESTRICTO de 100 KB para el binario forense (va al
# movil con un `cat`, y el movil es el que tiene la RAM justa).
# MEDIDO 2026-11-27: 43.576 B con los flags optimizados.
# Los MISMOS flags que tools/build-mddoctor.sh: si el gate midiera con otros,
# mediria un binario que no existe.
"$CC" -O2 -s -flto -ffunction-sections -fdata-sections -Wl,--gc-sections \
    -fmerge-all-constants -fno-ident -fno-plt -Wl,--as-needed \
    -fno-unwind-tables -fno-asynchronous-unwind-tables -Wl,-z,noseparate-code \
    -o "$TMP/md" "$RAIZ/tools/mddoctor.c"
TAM=$(wc -c < "$TMP/md")
echo "mddoctor: $TAM B (tope: 102400 B)"
if [ "$TAM" -gt 102400 ]; then
    echo "FALLO: mddoctor pesa $TAM B y el tope es 102400 B" >&2
    exit 1
fi
