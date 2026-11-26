#!/usr/bin/env sh
# =============================================================================
# mdo.sh — EL ONE-LINER DE VERDAD: una línea y ya está.
#
#   sh -c 'curl -fsSL https://raw.githubusercontent.com/boolforge/megadaemon/main/deploy/mdo.sh | sh'
#
# Qué hace, en este orden, SIN preguntar nada:
#   1. Se descarga a sí mismo (por si el `|` se parte) y busca el instalador
#      completo en el repo (3 URLs de fallback).
#   2. Delega en `deploy/megadaemon-oneliner.sh`, que es el instalador real:
#      autodetecta el entorno, resuelve el release, verifica el bundle POR
#      VALOR, instala de forma atómica con rollback, arranca el stack,
#      ESPERA A QUE ESTÉ LISTO (health-check real), publica el canal y sube
#      la telemetría a Google Drive.
#   3. Si todo va bien, deja el móvil AUTO-CURADO: el watchdog relanza el
#      túnel si muere, el keeper revive el daemon y el propio daemon publica
#      telemetría cada 15 min (el doble en modo ahorro de batería).
#
# Por qué este fichero existe (y no basta con el instalador):
#   * El `raw.githubusercontent.com` **cachea en CDN**: una versión vieja del
#     instalador puede servirse durante minutos.  Este script comprueba la
#     versión (con la API de Contents, que NO cachea) y avisa si lo que ha
#     ejecutado no es lo último.
#   * El instalador necesita un shell con `curl`/`wget`; si el móvil no los
#     tiene, este script lo dice con claridad en vez de fallar en silencio.
#
# IDEMPOTENTE: se puede ejecutar 20 veces seguidas sin romper nada.
# =============================================================================
set -u

REPO="boolforge/megadaemon"
RAW="https://raw.githubusercontent.com/$REPO/main"
API="https://api.github.com/repos/$REPO"
# ⚠️ `/data/local/tmp` NO es escribible por el uid de Termux (pertenece a
# shell:shell) ⇒ el lanzador no podía ni descargar el instalador.  Termux
# define TMPDIR; si no, /tmp.
TMP="${TMPDIR:-/tmp}/mdo.$$.sh"

say() { printf '[mdo] %s\n' "$*"; }
warn() { printf '[mdo] AVISO: %s\n' "$*" >&2; }

# --- ayuda ------------------------------------------------------------------
# ⚠️ El script puede llegar por `curl | sh`: entonces `$0` es "sh" y no hay
# fichero que leer.  La ayuda va en un `case` aparte, con su propio texto.
for a in "$@"; do
  case "$a" in
    -h|--help)
      cat <<'AYUDA'
mdo.sh — el one-liner de MegaDaemon.

  sh -c 'curl -fsSL https://raw.githubusercontent.com/boolforge/megadaemon/main/deploy/mdo.sh | sh'

Descarga el instalador completo y lo ejecuta.  El instalador autodetecta el
entorno (móvil/Termux/Magisk o PC), resuelve el release, verifica el bundle
por valor, instala de forma atómica con rollback, arranca el stack, ESPERA A
QUE ESTÉ LISTO (health-check real del túnel), publica el canal en el repo y
sube la telemetría y los datos de conexión a Google Drive.

Opciones que se pasan al instalador:
  --diagnostico     sólo lee el estado (no toca nada)
  --instalar        instala (por defecto)
  --pc              instala en el móvil por adb
  --force           reinstala aunque ya esté sano
  --sin-telemetria  no sube telemetría a Drive
AYUDA
      exit 0
      ;;
  esac
done

# --- 0) downloader (curl > wget) --------------------------------------------
CURL=""
for c in curl wget; do
  if command -v "$c" >/dev/null 2>&1; then CURL="$c"; break; fi
done
if [ -z "$CURL" ]; then
  warn "no hay curl ni wget: no puedo descargar el instalador."
  warn "en Termux: 'pkg install curl'  ·  o usa el modo PC (--pc)"
  exit 1
fi

descargar_a() {
  url="$1"; dest="$2"
  if [ "$CURL" = "curl" ]; then
    curl -fsSL --retry 3 --retry-delay 2 --connect-timeout 15 --max-time 120 -o "$dest" "$url" 2>/dev/null
  else
    wget -q -T 30 -O "$dest" "$url" 2>/dev/null
  fi
}

# --- 1) guardar el script (por si viene por tubería) -----------------------
# Si el script llegó por `curl | sh`, stdin es el propio script: no se puede
# leer.  Por eso se descarga de nuevo a un fichero temporal.
if [ ! -f "/proc/self/fd/0" ] || [ -t 0 ]; then
  : # ejecución normal desde un fichero
fi

# --- 2) localizar el instalador --------------------------------------------
INSTALADOR=""
for cand in \
  "$RAW/deploy/megadaemon-oneliner.sh" \
  "https://github.com/$REPO/raw/main/deploy/megadaemon-oneliner.sh" \
  "$RAW/deploy/instalador-megadaemon.sh"
do
  if descargar_a "$cand" "$TMP" && [ -s "$TMP" ]; then
    INSTALADOR="$TMP"
    say "instalador descargado de: $cand"
    break
  fi
done

if [ -z "$INSTALADOR" ]; then
  # Último recurso: el propio daemon ya instalado sabe actualizarse.
  if [ -x "/data/adb/megadaemon/megadaemon-core" ]; then
    say "sin red para el instalador: delego en el daemon instalado (auto-actualizador)"
    # ⚠️ `update` actualiza $MEGADAEMON_BIN (main.rs), no el binario que hay
    # al lado: sin esto escribía en la ruta legacy /data/local/tmp.
    MEGADAEMON_BIN=/data/adb/megadaemon/megadaemon-core \
      exec /data/adb/megadaemon/megadaemon-core update
  fi
  warn "no se pudo descargar el instalador y no hay daemon instalado."
  warn "Comprueba la red del móvil (o instálalo desde el PC: --pc)."
  exit 1
fi

chmod 755 "$INSTALADOR" 2>/dev/null

# --- 3) aviso de versión (la API no cachea; el raw sí) --------------------
# ⚠️ python3 NO existe en Android/Termux por defecto ⇒ el aviso nunca se
# ejecutaba.  Se compara el "size" que devuelve la API de Contents (que NO
# cachea) con el tamaño local: shell puro.
VER_LOCAL=$(wc -c < "$INSTALADOR" 2>/dev/null | tr -d ' ')
if [ -n "$VER_LOCAL" ]; then
  SZ_API=$(
    if [ "$CURL" = "curl" ]; then
      curl -fsSL --connect-timeout 15 -H "Accept: application/vnd.github+json" \
        "$API/contents/deploy/megadaemon-oneliner.sh?ref=main" 2>/dev/null
    else
      wget -q -T 20 -O - --header="Accept: application/vnd.github+json" \
        "$API/contents/deploy/megadaemon-oneliner.sh?ref=main" 2>/dev/null
    fi | tr ',' '\n' | sed -n 's/.*"size"[[:space:]]*:[[:space:]]*\([0-9]*\).*/\1/p' | head -1
  )
  if [ -n "$SZ_API" ] && [ "$SZ_API" != "$VER_LOCAL" ]; then
    warn "el instalador ejecutado NO es el último (CDN cacheado). Se ejecuta igualmente,"
    warn "pero conviene reintentar en un par de minutos para coger la versión nueva."
  fi
fi

# --- 4) delegar ------------------------------------------------------------
say "delego la instalación en el instalador completo"
sh "$INSTALADOR" --instalar "$@"
RC=$?
rm -f "$TMP" 2>/dev/null

# --- 5) resumen ------------------------------------------------------------
if [ "$RC" = "0" ]; then
  say "OK. El móvil queda:"
  say "  - instalado (atómico, con rollback)"
  say "  - arrancado y auto-curado (watchdog + keeper)"
  say "  - con telemetría y datos de conexión subidos a Google Drive"
  say "  - listo para recibir instrumentación y tareas sin conexión directa"
else
  warn "el instalador devolvió el código $RC"
  warn "el estado queda en disco; se puede reejecutar este mismo comando (es idempotente)"
fi
exit "$RC"
