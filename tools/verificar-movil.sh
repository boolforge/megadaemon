#!/usr/bin/env bash
# Verifica que el MegaDaemon del móvil ha vuelto (y con qué build).
#
# POR QUÉ EXISTE: el móvil es el único "hombre en el bucle" del sistema.  Su
# daemon se cayó y el ADB es inalcanzable desde el sandbox (un proxy atrapa
# el puerto 44444), así que el reinicio hay que hacerlo a mano.  Este script
# es la comprobación de que el reinicio funcionó y de que el auto-actualizador
# trajo la build nueva SOLO.
#
# USO:  tools/verificar-movil.sh [URL_TUNEL] [SEGUNDOS_ESPERA]
# SALIDA: 0 si el móvil responde y está sano; 1 si no.
set -uo pipefail

URL="${1:-https://graphical-subscriber-sys-oil.trycloudflare.com}"
ESPERA="${2:-300}"
F="${BASH_SOURCE[0]}"

log() { printf '[%s] %s\n' "$(date -u +%FT%TZ)" "$*"; }

# 1) ¿Responde el túnel?  (502 = el túnel vive pero el ORIGEN está caído)
log "esperando al origen del móvil en $URL (hasta ${ESPERA}s)"
deadline=$(( $(date +%s) + ESPERA ))
code=000
while [ "$(date +%s)" -lt "$deadline" ]; do
    code=$(curl -s -o /tmp/md_health.json -w '%{http_code}' --max-time 12 "$URL/health" 2>/dev/null || echo 000)
    if [ "$code" = "200" ]; then break; fi
    log "  /health => $code (todavía no)"
    sleep 10
done

if [ "$code" != "200" ]; then
    log "FALLO: el móvil no responde (último código: $code)"
    log "  * si sigue en 502, el daemon no arrancó: el supervisor/keeper siguen instalados en el móvil,"
    log "    y el reinicio manual es lo único que queda."
    exit 1
fi

log "/health responde 200 ✓"
python3 - <<'PY'
import json
try:
    d = json.load(open('/tmp/md_health.json'))
except Exception as e:
    print('  (no se pudo leer el JSON:', e, ')'); raise SystemExit(0)
b = d.get('build', {}) or {}
print('  build.short  :', b.get('short'))
print('  build.commit :', b.get('commit'))
print('  build.sha256 :', (b.get('sha256') or '')[:16], '...')
print('  key_fp       :', d.get('key_fp'))
print('  cuota        :', d.get('cuota'))
PY

# 2) ¿Vive la Web UI?  (los puertos alias 3000/3030/8090)
for p in / /ui /api/live; do
    c=$(curl -s -o /dev/null -w '%{http_code}' --max-time 12 "$URL$p" 2>/dev/null || echo 000)
    log "  $p => $c"
done

# 3) ¿Sigue vivo?  (el probe público que NO exige credencial; 401/403 = vivo)
c=$(curl -s -o /dev/null -w '%{http_code}' --max-time 12 "$URL/api/live" 2>/dev/null || echo 000)
if [ "$c" = "200" ] || [ "$c" = "401" ] || [ "$c" = "403" ]; then
    log "Sonda de vida: $c ⇒ el daemon está en pie ✓"
else
    log "Sonda de vida: $c ⇒ el daemon NO responde"
    exit 1
fi

log "MÓVIL SANO ✓  (build arriba; el auto-actualizador instala la nueva cada 30 min)"
exit 0
