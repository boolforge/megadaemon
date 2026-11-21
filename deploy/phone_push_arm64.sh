#!/usr/bin/env bash
# MegaDaemon — despliegue del binario ARM64 al Samsung en UN comando.
#
# Por qué este script
# -------------------
# El artefacto ARM64 (el que vive en el móvil) no se desplegaba porque:
#   1. el build de CI moría por OOM (perfil release con lto=true +
#      codegen-units=1) ⇒ artefacto inexistente;
#   2. `cancel-in-progress: true` ⇒ con pushes frecuentes ningún run
#      terminaba y nunca se publicaba el release;
#   3. al subir el binario con `adb push` se quedaba en 644 (sin bit +x) y el
#      supervisor lo daba por inexistente ⇒ se rendía (causa medida del 502).
#
# Este script encadena todo en orden y **verifica**:
#   sha256 (si hay .sha256) → push → chmod 755 → reinicio → /health == 200
#
# Uso (desde un PC en la misma LAN que el móvil, con platform-tools y la
# clave RSA del host ya autorizada en el teléfono):
#   ./deploy/phone_push_arm64.sh [--serial R5CT30XXXXX] [--bin RUTA] [--port 9090]
#
# La clave RSA del host: /workspace/md-secrets/adb_host_key.pem → copiala a
# ~/.android/adbkey (chmod 600) antes de la primera vez.

set -euo pipefail

PREFIX="/data/local/tmp/megadaemon"
STATE="$PREFIX/state"
PORT=9090
SERIAL=""
BIN=""
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

while [ $# -gt 0 ]; do
  case "$1" in
    --serial) SERIAL="$2"; shift 2 ;;
    --bin) BIN="$2"; shift 2 ;;
    --port) PORT="$2"; shift 2 ;;
    -h|--help) sed -n '2,30p' "$0"; exit 0 ;;
    *) echo "opción desconocida: $1" >&2; exit 2 ;;
  esac
done

say() { printf '[phone_push] %s\n' "$*"; }
die() { printf '[phone_push] ERROR: %s\n' "$*" >&2; exit 1; }

command -v adb >/dev/null 2>&1 || die "no está 'adb' (instala platform-tools)"
[ -n "$BIN" ] || BIN="$REPO/rust-core/target/aarch64-linux-android/release/megadaemon-core"
[ -f "$BIN" ] || die "no existe el binario ARM64 en $BIN (compílalo: cargo build --release --target aarch64-linux-android)"
[ -x "$BIN" ] || chmod 755 "$BIN"

# --- 0. el móvil tiene que estar 'device' (no 'offline': clave RSA) ------
if [ -z "$SERIAL" ]; then
  SERIAL="$(adb devices | awk 'NR>1 && $2=="device" {print $1; exit}')"
  [ -n "$SERIAL" ] || die "adb no ve ningún móvil en estado 'device' (¿clave RSA en ~/.android/adbkey? ¿depuración USB autorizada?)"
fi
say "móvil: $SERIAL"

# --- 1. sha256: el binario que sube es el que se midió -------------------
SHA_FILE="$BIN.sha256"
if [ -f "$SHA_FILE" ]; then
  WANT="$(awk '{print $1}' "$SHA_FILE")"
  GOT="$(sha256sum "$BIN" | awk '{print $1}')"
  [ "$WANT" = "$GOT" ] || die "sha256 NO coincide: esperado $WANT, obtenido $GOT"
  say "sha256 verificado: ${GOT:0:16}…"
else
  GOT="$(sha256sum "$BIN" | awk '{print $1}')"
  say "AVISO: no hay .sha256; el sha256 de este build es $GOT"
fi

# --- 2. subir con copia de seguridad --------------------------------------
adb -s "$SERIAL" shell "su -mm -c 'cp -f $PREFIX/megadaemon-core $PREFIX/megadaemon-core.prev 2>/dev/null'" >/dev/null 2>&1 || true
adb -s "$SERIAL" push "$BIN" "$PREFIX/megadaemon-core" >/dev/null || die "adb push falló"

# --- 3. chmod 755 ANTES de reiniciar (si no, el supervisor se rinde) ------
adb -s "$SERIAL" shell "su -mm -c 'chmod 755 $PREFIX/megadaemon-core'" || die "chmod falló"
adb -s "$SERIAL" shell "su -mm -c 'ls -l $PREFIX/megadaemon-core'" | head -1

# --- 4. la plantilla web también ------------------------------------------
TPL="$REPO/web/cockpit_template.html"
if [ -f "$TPL" ]; then
  adb -s "$SERIAL" shell "su -mm -c 'mkdir -p /data/user/0/com.agy/files/megadaemon/web $PREFIX/web'" >/dev/null 2>&1 || true
  adb -s "$SERIAL" push "$TPL" "/data/user/0/com.agy/files/megadaemon/web/cockpit_template.html" >/dev/null 2>&1 \
    && adb -s "$SERIAL" shell "su -mm -c 'chmod 644 /data/user/0/com.agy/files/megadaemon/web/cockpit_template.html'" >/dev/null 2>&1 \
    && say "plantilla web desplegada" || say "AVISO: no se pudo desplegar la plantilla"
fi

# --- 5. reiniciar el daemon (el supervisor lo relanza) --------------------
adb -s "$SERIAL" shell "su -mm -c 'pkill -f \"megadaemon-core cockpit\"'" >/dev/null 2>&1 || true
say "daemon detenido; el supervisor lo relanza"

# --- 6. verificar /health (no se da por bueno antes) ----------------------
for i in $(seq 1 30); do
  CODE="$(adb -s "$SERIAL" shell "curl -s -o /dev/null -m 4 -w '%{http_code}' http://127.0.0.1:$PORT/health" 2>/dev/null | tr -d '\r')"
  if [ "$CODE" = "200" ]; then
    say "OK: /health = 200 (móvil operativo)"
    adb -s "$SERIAL" shell "su -mm -c 'cat /data/local/tmp/megadaemon/state/megadaemon.url'" 2>/dev/null || true
    exit 0
  fi
  sleep 2
done
die "/health no devolvió 200 tras 60 s (último código: ${CODE:-sin respuesta}). Log: adb shell su -mm -c 'tail -30 /sdcard/megadaemon.log'"
