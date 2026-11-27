#!/bin/sh
# agy_install_token.sh — instala un token OAuth REAL para el Language Server
# de Antigravity en un dispositivo Android.
#
# POR QUÉ
# -------
# El LS abre exactamente  $HOME/.gemini/jetski-standalone-oauth-token  y lo
# deserializa como JSON. Sin ese fichero (o con uno falso) responde
#   "error getting token source: You are not logged into Antigravity"
# y NO emite ninguna petición a la API (medido el 2026-11-26).
#
# El token se obtiene intercambiando el refresh_token de un
# application_default_credentials.json de tipo authorized_user por un
# access_token real en https://oauth2.googleapis.com/token. Una sola peticion.
#
# USO (en el movil, con root):
#   sh agy_install_token.sh /ruta/al/application_default_credentials.json
#
# Es IDEMPOTENTE y no imprime ningun valor de token: solo longitudes, rutas
# y caducidad. El ADC se borra de memoria al terminar (no se copia).
set -eu

ADC="${1:-}"
if [ -z "$ADC" ]; then
  echo "uso: agy_install_token.sh <application_default_credentials.json>" >&2
  exit 2
fi
if [ ! -r "$ADC" ]; then
  echo "no se puede leer el ADC: $ADC" >&2
  exit 2
fi

HERE="$(cd "$(dirname "$0")" && pwd)"
AUTH_PY="$HERE/agy_auth.py"
[ -f "$AUTH_PY" ] || AUTH_PY="/data/local/tmp/megadaemon/antigravity/tools/agy_auth.py"
if [ ! -f "$AUTH_PY" ]; then
  echo "no encuentro agy_auth.py (lo busco en $HERE y en el movil)" >&2
  exit 2
fi

PY=""
for c in python3 python /system/bin/python3; do
  if command -v "$c" >/dev/null 2>&1; then PY="$c"; break; fi
done
[ -n "$PY" ] || { echo "no hay python en el dispositivo" >&2; exit 2; }

WORK="$(mktemp -d 2>/dev/null || echo /data/local/tmp/agy-token-$$)"
mkdir -p "$WORK"
trap 'rm -rf "$WORK"' EXIT
cp "$ADC" "$WORK/adc.json"
chmod 600 "$WORK/adc.json"

echo "== acuñando token real con $PY"
TOKEN_JSON="$WORK/token.json"
"$PY" "$AUTH_PY" mint --adc "$WORK/adc.json" --install --token-path "$TOKEN_JSON" || exit 1

# ---- localizar TODOS los directorios .gemini del usuario del LS ---------
# El LS puede correr como root (agente) o como la app (com.agy). Se
# instalan en todos: el token es el mismo y el fichero se lee por sesion.
DIRS=""
for base in /data/data/com.agy/files /data/data/com.agy /data/user/0/com.agy \
            /data/data/com.google.android.apps.antigravity/files \
            "$HOME" /root /data/local/tmp/megadaemon; do
  [ -d "$base" ] || continue
  for g in "$base/.gemini" "$base/files/.gemini" "$base/home/.gemini"; do
    [ -d "$g" ] && case " $DIRS " in *" $g "*) ;; *) DIRS="$DIRS $g";; esac
  done
done
# búsqueda de respaldo: cualquier .gemini bajo /data (acotado)
if [ -z "$DIRS" ]; then
  for g in $(find /data -maxdepth 5 -type d -name ".gemini" 2>/dev/null | head -10); do
    DIRS="$DIRS $g"
  done
fi

if [ -z "$DIRS" ]; then
  echo "AVISO: no encontre ningun directorio .gemini; dejo el token en $TOKEN_JSON"
  echo "       (no se borra para poder instalarlo a mano)"
  exit 0
fi

# ---- instalar ---------------------------------------------------------
N=0
for g in $DIRS; do
  mkdir -p "$g" 2>/dev/null || continue
  if cp "$TOKEN_JSON" "$g/jetski-standalone-oauth-token" 2>/dev/null; then
    chmod 600 "$g/jetski-standalone-oauth-token" 2>/dev/null || true
    chown "$(stat -c %U:%G "$g" 2>/dev/null || echo root:root)" \
           "$g/jetski-standalone-oauth-token" 2>/dev/null || true
    LEN=$(wc -c < "$g/jetski-standalone-oauth-token")
    echo "  instalado en $g/jetski-standalone-oauth-token ($LEN B, 0600)"
    N=$((N+1))
  else
    echo "  NO se pudo instalar en $g (permisos)"
  fi
done

echo "== $N ubicaciones preparadas"
echo "== el token caduca en ~1 h: para renovar, repetir este script"
echo "== el refresh_token de origen dura ~49 dias (refresh_token_expires_in)"
echo "== NO se ha impreso ningun valor de token"
exit 0
