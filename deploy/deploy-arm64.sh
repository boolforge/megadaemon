#!/system/bin/sh
# =====================================================================
# deploy-arm64.sh — instala el binario ARM64 nuevo de MegaDaemon en el
# Samsung (instancia primary) y deja la web sirviéndose SIN el puente
# python (que hoy es lo único que sostiene la interfaz).
#
# Por qué existe (medido, no teórico):
#   * El binario desplegado (v13.3.0) hace bind SOLO de 127.0.0.1:9090 y
#     su gate de API key responde 401 JSON también en '/' ⇒ el navegador
#     (que no puede mandar cabeceras) no ve la interfaz.
#   * El puente python (deploy/web_bridge.py) tapaba ese hole sirviendo
#     3000/3030/8080/8090 e inyectando las cabeceras, pero es un parche:
#     con el binario nuevo sobra.
#   * Si el daemon nuevo hace bind de esos puertos con el puente vivo =>
#     EADDRINUSE y el daemon MUERE.  Por eso el orden es: parar puente y
#     supervisor -> instalar -> arrancar -> verificar.
#
# Uso (desde root, con el binario y este script ya subidos por
# /api/chat_upload):
#     sh deploy-arm64.sh <ruta_binario> <sha256_esperado>
#
# NO toca: nada fuera de /data/local/tmp/megadaemon y /data/adb/service.d.
# =====================================================================
set -u

PREFIX=/data/local/tmp/megadaemon
STATE="$PREFIX/state"
UPLOADS=/data/user/0/com.agy/files/megadaemon/uploads
SERVICE=/data/adb/service.d/99-megadaemon.sh
LOG=/sdcard/megadaemon.log
PY=/data/data/com.termux/files/usr/bin/python3
CURL=/data/data/com.termux/files/usr/bin/curl

BIN_SRC="${1:-}"
SHA_EXPECTED="${2:-}"
[ -n "$BIN_SRC" ] || { echo "FATAL: falta la ruta del binario"; exit 2; }

say() { echo "[deploy] $*"; }

# ── 0. Material local ──────────────────────────────────────────────────
[ -f "$BIN_SRC" ] || { echo "FATAL: no existe $BIN_SRC"; exit 2; }
SHA_REAL="$(sha256sum "$BIN_SRC" | awk '{print $1}')"
say "binario: $BIN_SRC"
say "sha256 real: $SHA_REAL"
if [ -n "$SHA_EXPECTED" ] && [ "$SHA_REAL" != "$SHA_EXPECTED" ]; then
  say "ABORTO: sha256 distinto al esperado ($SHA_EXPECTED)"
  exit 3
fi

# ── 1. Parar el puente y el supervisor viejo (evita EADDRINUSE) ─────────
say "parando el supervisor viejo y el puente..."
for p in $(pidof megadaemon-core 2>/dev/null); do kill "$p" 2>/dev/null; done
for p in $(ps -A -o pid,args 2>/dev/null | grep web_bridge.py | grep -v grep | awk '{print $1}'); do
  say "  kill puente pid $p"; kill "$p" 2>/dev/null
done
for p in $(ps -A -o pid,args 2>/dev/null | grep 'service.d/99-megadaemon.sh' | grep -v grep | awk '{print $1}'); do
  say "  kill supervisor pid $p"; kill "$p" 2>/dev/null
done
sleep 2

# ── 2. Instalar el binario nuevo (con .bak) ───────────────────────────
cp -f "$BIN_SRC" "$PREFIX/megadaemon-core.new" 2>/dev/null
chmod 755 "$PREFIX/megadaemon-core.new"
if [ -f "$PREFIX/megadaemon-core" ]; then
  cp -f "$PREFIX/megadaemon-core" "$PREFIX/megadaemon-core.prev" 2>/dev/null
fi
cp -f "$PREFIX/megadaemon-core.new" "$PREFIX/megadaemon-core"
chmod 755 "$PREFIX/megadaemon-core"
rm -f "$PREFIX/megadaemon-core.new"
say "binario instalado: $(ls -la $PREFIX/megadaemon-core | awk '{print $5" bytes"}')"

# ── 3. Supervisor canónico (si está subido) ───────────────────────────
if [ -f "$UPLOADS/99-megadaemon.sh" ]; then
  cp -f "$UPLOADS/99-megadaemon.sh" "$SERVICE" && chmod 755 "$SERVICE"
  cp -f "$UPLOADS/99-megadaemon.sh" "$PREFIX/99-megadaemon.sh" && chmod 755 "$PREFIX/99-megadaemon.sh"
  say "supervisor canónico instalado"
fi

# ── 4. Arrancar el daemon (a mano; el supervisor lo releva al reboot) ──
cd "$PREFIX" || exit 4
if [ -f "$PREFIX/megadaemon.env" ]; then
  set -a; . "$PREFIX/megadaemon.env" 2>/dev/null; set +a
fi
setsid nohup "$PREFIX/megadaemon-core" cockpit 9090 >> "$LOG" 2>&1 &
sleep 6

# ── 5. Verificar (espera activa de /health = 200 EXACTO) ───────────────
CODE=""
for i in $(seq 1 20); do
  CODE="$($CURL -s -m 4 -o /dev/null -w '%{http_code}' http://127.0.0.1:9090/health 2>/dev/null)"
  [ "$CODE" = "200" ] && break
  sleep 2
done
say "/health en 9090 => ${CODE:-sin respuesta}"

# Los alias los sirve el daemon nuevo (no el puente): se comprueba que
# / devuelve la interfaz y no un 401 JSON.
ALIAS_OK=0
for p in 3000 3030 8090 8080; do
  C="$($CURL -s -m 4 -o /dev/null -w '%{http_code}' "http://127.0.0.1:$p/" 2>/dev/null)"
  S="$($CURL -s -m 4 "http://127.0.0.1:$p/" 2>/dev/null | wc -c)"
  say "  alias $p => ${C:-000} ($S bytes)"
  [ "$C" = "200" ] && [ "$S" -gt 10000 ] && ALIAS_OK=$((ALIAS_OK+1))
done

# IPv6 (Chrome en Android resuelve localhost a ::1 primero)
C6="$($CURL -s -m 4 -o /dev/null -w '%{http_code}' "http://[::1]:3000/" 2>/dev/null)"
say "  alias [::1]:3000 => ${C6:-000}"

say "RESULTADO: health=${CODE:-none} alias_ok=$ALIAS_OK/4 ipv6=${C6:-000}"
say "puertos:"; ss -ltn 2>/dev/null | grep -E ':(80|3000|3030|8090|8080|9090) ' | head -8
exit 0
