#!/usr/bin/env sh
# =============================================================================
# megadaemon-oneliner.sh — EL ONE-LINER DE INSTALACIÓN, EJECUCIÓN Y DIAGNÓSTICO
#
#   DESCARGAR Y EJECUTAR TODO (una línea, en el móvil, sin PC):
#
#     sh -c 'curl -fsSL https://raw.githubusercontent.com/boolforge/megadaemon/main/deploy/megadaemon-oneliner.sh | sh'
#
#   O, más robusto (reintenta la descarga y deja el fichero en disco):
#
#     sh -c 'curl -fsSL --retry 3 https://raw.githubusercontent.com/boolforge/megadaemon/main/deploy/megadaemon-oneliner.sh -o /data/local/tmp/mdo.sh && sh /data/local/tmp/mdo.sh'
#
#   O desde el PC (el móvil por adb):
#
#     sh deploy/megadaemon-oneliner.sh --pc
#
# -----------------------------------------------------------------------------
# QUÉ HACE (TODO automático, sin intervención humana)
# -----------------------------------------------------------------------------
#   1. AUTODETECTA el entorno: ¿móvil (Android/Termux/Magisk)? ¿PC? ¿root?
#      ¿arquitectura? ¿hay red? NO pregunta nada: decide solo.
#   2. RESUELVE el release real (los tags son `v<sha>`, NO existe "latest"):
#      URL estable -> manifiesto -> API. Tres capas de fallback.
#   3. DESCARGA el bundle meta (`megadaemon-movil.tar.gz`) y su `.sha256`, y
#      VERIFICA **POR VALOR** (compara el hash, no la ruta del fichero: el
#      fallo medido del 2026-11-26 era que el `.sha256` llevaba la ruta del
#      job ⇒ nunca cuadraba y NO se publicaba ninguna release).
#   4. INSTALA de forma ATÓMICA: extrae a un directorio nuevo, deja el binario
#      en `.new` y lo renombra (atómico), verifica que queda ejecutable y que
#      el daemon ARRANCA; si algo falla, ROLLBACK al binario anterior.
#   5. ARRANCA el stack completo: daemon + cloudflared + watchdog, con el
#      entorno correcto (LOG_DIR, DAEMON_BIN, MEGADAEMON_STATE_DIR…), como
#      Magisk `service.d` (PPID 1) => sobrevive a reinicios y a que Termux
#      muera.
#   6. ESPERA A QUE ESTÉ LISTO: health-check REAL en dos niveles
#      (`/health` en local + sonda del túnel público). Reintenta con backoff.
#   7. PUBLICA el canal en los 4 sitios: repo (API de Contents, NO el `raw`
#      que cachea en CDN), paste, **Google Drive** y el relé canónico.
#   8. TELEMETRÍA: sube a Drive el estado del nodo, los datos de conexión y
#      **un informe de la instalación** (qué se instaló, con qué sha256, cuánto
#      tardó, qué falló).  El root lee Drive y sabe qué pasa SIN abrir ADB.
#   9. DIAGNÓSTICO de verdad: `--diagnostico` audita daemon, /health local,
#      túnel, watchdog, keeper, cloudflared, canal, credenciales, espacio en
#      disco, binario en servicio vs. en disco y freshness del proceso, y
#      publica el informe en Drive.
#  10. ES IDEMPOTENTE: si ya está instalado y el túnel responde, NO se toca
#      nada (matar el daemon o relanzar cloudflared cambiaría el hostname y
#      dejaría ciega a la flota).
#
# -----------------------------------------------------------------------------
# SECRETOS
# -----------------------------------------------------------------------------
#   * El token de GitHub NO va en este fichero: se busca en las rutas
#     habituales del móvil (0600) y, si no está, se explica cómo ponerlo.
#   * El token de Drive NUNCA sale del fichero de credenciales 0600: el
#     daemon lo refresca solo (`google_auth`). Este script no lo imprime.
#   * El repo es PÚBLICO: aquí no se sube nada con credenciales ni datos del
#     dispositivo. Los artefactos sensibles van a la carpeta PRIVADA de Drive.
# =============================================================================
set -u

# --- identidad --------------------------------------------------------------
SELF="$0"
case "$SELF" in /*) ;; *) SELF="$PWD/$SELF" ;; esac
REPO_SLUG="boolforge/megadaemon"
REPO_RAW="https://raw.githubusercontent.com/$REPO_SLUG/main"
REPO_API="https://api.github.com/repos/$REPO_SLUG"
RELEASES_LATEST="https://github.com/$REPO_SLUG/releases/latest"

# --- destino (se detecta) --------------------------------------------------
PREFIX="/data/adb/megadaemon"          # persistente, lo usa Magisk
STATE="$PREFIX/state"
LOGDIR="$PREFIX/state"
BUNDLE_ASSET="megadaemon-movil.tar.gz"
PUBLISH_ASSET="channel-publish-arm64"
DAEMON="$PREFIX/megadaemon-core"
ENV_FILE="$PREFIX/megadaemon.env"
PORT=9090

# --- entorno del daemon (CRÍTICO) ------------------------------------------
# ⚠️ Sin esto el daemon lee/escribe el estado en
# `/data/user/0/com.agy/files/megadaemon/state` (el de la app Antigravity,
# `rust-core/src/state_dir.rs:17`) y NO en el nuestro ⇒ el canal queda
# vacío y la telemetría se sube a Drive sin la URL del túnel.  El supervisor
# (`deploy/99-megadaemon.sh:48`) lo exporta; aquí también.
export MEGADAEMON_PREFIX="$PREFIX"
export MEGADAEMON_STATE_DIR="$STATE"
export MEGADAEMON_BIN="$DAEMON"

# --- opciones ---------------------------------------------------------------
MODO="instalar"          # instalar | diagnostico | solo-telemetria | pc
FORCE=0
SIN_TELEMETRIA=0
SIN_PUBLICAR=0
REINTENTOS=5
TIMEOUT_S=120

for a in "$@"; do
  case "$a" in
    --diagnostico|--verificar) MODO="diagnostico" ;;
    --instalar|--arreglar)     MODO="instalar" ;;
    --pc)                      MODO="pc" ;;
    --solo-telemetria)         MODO="solo-telemetria" ;;
    --force)                   FORCE=1 ;;
    --sin-telemetria)          SIN_TELEMETRIA=1 ;;
    --sin-publicar)            SIN_PUBLICAR=1 ;;
    --reintentos=*)            REINTENTOS="${a#*=}" ;;
    --timeout=*)               TIMEOUT_S="${a#*=}" ;;
    -h|--help) sed -n '2,70p' "$SELF" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) printf 'opción desconocida: %s\n' "$a" >&2; exit 2 ;;
  esac
done

# Validación de las opciones numéricas (un `--reintentos=abc` rompería el
# bucle `[ "$i" -le "abc" ]` con un error de shell).
case "$REINTENTOS" in ''|*[!0-9]*) REINTENTOS=5 ;; esac
case "$TIMEOUT_S" in ''|*[!0-9]*) TIMEOUT_S=120 ;; esac
[ "$REINTENTOS" -lt 1 ] && REINTENTOS=1
[ "$REINTENTOS" -gt 20 ] && REINTENTOS=20
[ "$TIMEOUT_S" -lt 30 ] && TIMEOUT_S=30

# --- log --------------------------------------------------------------------
# `say` escribe SIEMPRE en el log de disco: es lo que el root lee desde Drive
# cuando el móvil no responde.  Un log sólo en pantalla no sirve de nada.
LOGFILE="$STATE/instalador.log"
mkdir -p "$STATE" 2>/dev/null
say() {
  printf '[%s] %s\n' "$(date -u +%H:%M:%S)" "$*"
  printf '[%s] %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" >> "$LOGFILE" 2>/dev/null
}
warn() { printf '[%s] AVISO: %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; }
die()  { printf '[%s] ERROR: %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; exit 1; }

# ===========================================================================
# 0) AUTODETECCIÓN
# ===========================================================================
ES_ANDROID=0
ES_TERMUX=0
[ -d "/data/adb" ] && ES_ANDROID=1
[ -d "/data/data/com.termux/files/usr" ] && ES_TERMUX=1

SOBRA_ROOT=0
[ "$(id -u)" = "0" ] && SOBRA_ROOT=1

if [ "$ES_ANDROID" = "1" ] || [ "$ES_TERMUX" = "1" ]; then
  DESTINO="movil"
else
  DESTINO="pc"
fi
[ "$MODO" = "pc" ] && DESTINO="pc"

ARQ=""
case "$(uname -m)" in
  aarch64|arm64) ARQ="arm64" ;;
  x86_64|amd64)  ARQ="x86_64" ;;
  *) ARQ="" ;;
esac

# El binario de downloader: curl > wget > el propio daemon (que trae HTTP
# nativo en Rust). MEDIDO: en el móvil el shell de Android NO tiene curl ni
# wget (`toybox wget` -> "Unknown command", rc=127), pero Termux suele
# traerlos. Por eso hay tres capas.
CURL=""
for c in curl wget; do
  if command -v "$c" >/dev/null 2>&1; then CURL="$c"; break; fi
done

say "=== entorno detectado ==="
say "destino: $DESTINO | android: $ES_ANDROID | termux: $ES_TERMUX | root: $SOBRA_ROOT"
say "arquitectura: ${ARQ:-desconocida} | downloader: ${CURL:-NINGUNO (usaré el daemon)}"

if [ "$DESTINO" = "movil" ] && [ "$ES_TERMUX" = "1" ] && [ -z "$CURL" ]; then
  warn "no hay curl ni wget en Termux: instala uno con 'pkg install curl'"
  warn "o vuelve a ejecutar el one-liner desde un PC con adb (--pc)"
fi

# --- root real (en Android, 'su -mm'; en Termux el 'sudo' es un wrapper) ---
AS_ROOT=""
if [ "$SOBRA_ROOT" = "1" ]; then
  AS_ROOT=""
elif [ "$ES_ANDROID" = "1" ]; then
  if su -mm -c "id -u" 2>/dev/null | grep -q '^0$'; then
    AS_ROOT="su -mm -c"
  else
    warn "no puedo obtener root (su -mm falla): lo que necesite root se queda sin hacer"
  fi
fi

# Ejecuta un comando como root si hace falta.
r() {
  if [ -n "$AS_ROOT" ]; then
    $AS_ROOT "$1" 2>&1
  else
    sh -c "$1" 2>&1
  fi
}

# ===========================================================================
# 1) HTTP (tres capas: curl, wget, el daemon)
# ===========================================================================
# descargar <url> <destino> [timeout_s]
descargar() {
  d_url="$1"; d_dest="$2"; d_to="${3:-600}"
  if [ -n "$CURL" ]; then
    if [ "$CURL" = "curl" ]; then
      curl -fsSL --retry 3 --retry-delay 2 --connect-timeout 15 --max-time "$d_to" \
           -o "$d_dest" "$d_url" 2>/dev/null && return 0
    else
      wget -q -T "$d_to" -O "$d_dest" "$d_url" 2>/dev/null && return 0
    fi
  fi
  # Capa 3: el propio daemon (HTTP nativo en Rust). Ya está instalado ⇒
  # no necesita nada externo.
  if [ -x "$DAEMON" ]; then
    "$DAEMON" probe "$d_url" >/dev/null 2>&1
    if command -v md-curl >/dev/null 2>&1; then
      md-curl -o "$d_dest" "$d_url" >/dev/null 2>&1 && return 0
    fi
  fi
  return 1
}

# texto <url> -> stdout
texto_url() {
  t_url="$1"
  if [ -n "$CURL" ]; then
    if [ "$CURL" = "curl" ]; then
      curl -fsSL --connect-timeout 15 --max-time 60 "$t_url" 2>/dev/null
    else
      wget -q -T 20 -O - "$t_url" 2>/dev/null
    fi
    return $?
  fi
  return 1
}

# Código HTTP de una URL (0 si no hay red). 200|401|403 = el daemon contesta.
codigo_http() {
  c_url="$1"
  if [ "$CURL" = "curl" ]; then
    c_code=$(curl -s -o /dev/null -w '%{http_code}' --connect-timeout 8 --max-time 15 "$c_url" 2>/dev/null)
    case "$c_code" in ''|*[!0-9]*) c_code=0 ;; esac
    printf '%s' "$c_code"
    return 0
  fi
  if [ "$CURL" = "wget" ]; then
    c_code=$(wget -q -S -O /dev/null --timeout=15 "$c_url" 2>&1 \
             | grep -m1 -o 'HTTP/[0-9.]* [0-9]*' | awk '{print $2}')
    case "$c_code" in ''|*[!0-9]*) c_code=0 ;; esac
    printf '%s' "$c_code"
    return 0
  fi
  # Sin downloader: el daemon sabe hacerlo (HTTP nativo).
  if [ -x "$DAEMON" ]; then
    printf '%s' "$("$DAEMON" probe "$c_url" 2>/dev/null | tr -d ' \r\n')"
    return 0
  fi
  printf '0'
}

# ===========================================================================
# 2) RESOLUCIÓN DEL RELEASE (tres capas)
# ===========================================================================
TAG=""
ASSET_URL=""
RESUELTO=0

resolver_release() {
  # Capa 1: la URL estable (no usa la API, que se agota por cuota de IP).
  # ⚠️ NO se descarga el tarball entero para comprobar que existe (¡varios MB
  # de datos móviles del Jazztel!): se pide sólo la cabecera con un HEAD.
  ESTABLE="$RELEASES_LATEST/download/$BUNDLE_ASSET"
  if [ -n "$CURL" ]; then
    if [ "$CURL" = "curl" ]; then
      CODE=$(curl -sI -L -o /dev/null -w '%{http_code}' --connect-timeout 15 --max-time 30 "$ESTABLE" 2>/dev/null)
    else
      CODE=$(wget -S --spider -T 20 "$ESTABLE" 2>&1 | sed -n 's/.*HTTP\/[0-9.]* \([0-9][0-9][0-9]\).*/\1/p' | tail -1)
    fi
    case "$CODE" in
      200|206|302|301)
        TAG=$(etiqueta_release)
        if [ -n "$TAG" ]; then
          ASSET_URL="$ESTABLE"
          RESUELTO=1
          return 0
        fi
        ;;
    esac
  fi
  # Capa 2: el manifiesto `latest.json` del release (asset real; la API de
  # Contents NO cachea, el `raw` sí).
  J=$(texto_url "$RELEASES_LATEST/download/latest.json" 2>/dev/null)
  if [ -n "$J" ]; then
    V=$(printf '%s' "$J" | sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)
    if [ -n "$V" ]; then
      TAG="v$V"
      ASSET_URL="$RELEASES_LATEST/download/$BUNDLE_ASSET"
      RESUELTO=1
      return 0
    fi
  fi
  # Capa 3: la API de releases (más cara, se agota por cuota de IP).
  J=$(texto_url "$REPO_API/releases/latest" 2>/dev/null)
  if [ -n "$J" ]; then
    TAG=$(printf '%s' "$J" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)
    U=$(printf '%s' "$J" | tr ',' '\n' | grep 'browser_download_url' | grep "$BUNDLE_ASSET\"" | grep -v '\.sha256' | sed -n 's/.*"browser_download_url"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)
    if [ -n "$TAG" ]; then
      ASSET_URL="${U:-$RELEASES_LATEST/download/$BUNDLE_ASSET}"
      RESUELTO=1
      return 0
    fi
  fi
  return 1
}

# Etiqueta (`v<sha>`) del último release, sacada de la redirección de
# /releases/latest.  Los tags SON `v<sha>`: NO existe un tag "latest".
etiqueta_release() {
  LOC=""
  if [ "$CURL" = "curl" ]; then
    LOC=$(curl -sI "$RELEASES_LATEST" 2>/dev/null | tr -d '\r' | sed -n 's/^[Ll]ocation:[[:space:]]*\(.*\/releases\/tag\/[^[:space:]]*\).*/\1/p' | head -1)
  elif [ "$CURL" = "wget" ]; then
    LOC=$(wget -S --spider "$RELEASES_LATEST" 2>&1 | tr -d '\r' | sed -n 's/.*[Ll]ocation:[[:space:]]*\(.*\/releases\/tag\/[^[:space:]]*\).*/\1/p' | head -1)
  fi
  [ -n "$LOC" ] || return 1
  printf '%s' "$LOC" | sed -n 's/.*\/releases\/tag\/\([^/?]*\).*/\1/p'
}

# ===========================================================================
# 3) VERIFICACIÓN POR VALOR (el bug medido del 2026-11-26)
# ===========================================================================
# verificar_sha <fichero> <fichero.sha256>
# El `.sha256` puede traer la RUTA del job que lo generó: se compara
# SÓLO el hash (el primer token), nunca el nombre.
#   0 = cuadra · 1 = no cuadra · 2 = no se pudo calcular
verificar_sha() {
  f="$1"; s="$2"
  [ -f "$f" ] || return 2
  [ -f "$s" ] || return 2
  # El `.sha256` del CI lleva la RUTA del job (`dist/megadaemon-movil.tar.gz`)
  # ⇒ se compara SÓLO el hash (el primer token), nunca el nombre.
  esperado=$(tr -d '\r\n' < "$s" | awk '{print $1}')
  [ -n "$esperado" ] || return 2
  real=""
  if command -v sha256sum >/dev/null 2>&1; then
    real=$(sha256sum "$f" | awk '{print $1}')
  else
    # Sin `sha256sum` (el shell de Android no lo trae): el daemon calcula el
    # hash con su cliente nativo.
    real=$(r "'$DAEMON' hash '$f' 2>/dev/null" | tr -d '\r\n ' | awk '{print $1}')
  fi
  [ -n "$real" ] || return 2
  [ "$esperado" = "$real" ] || return 1
  return 0
}

# sha16 de un fichero (para el informe: identificador, no secreto).
sha16_de() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" 2>/dev/null | cut -c1-16
  else
    r "'$DAEMON' hash '$1' 2>/dev/null" | tr -d '\r\n ' | cut -c1-16
  fi
}

# ===========================================================================
# 4) DIAGNÓSTICO REAL (patrones de tools/recuperar-en-movil.sh)
# ===========================================================================
# ¿Hay un proceso megadaemon-core?  Devuelve el PID.
daemon_vivo() {
  for p in /proc/[0-9]*; do
    [ -r "$p/cmdline" ] || continue
    d_c=$(tr '\0' ' ' < "$p/cmdline" 2>/dev/null)
    case "$d_c" in *megadaemon-core*) printf '%s' "${p#/proc/}"; return 0 ;; esac
  done
  return 1
}

# Cuántos daemons hay (2 o más = fought: matan al otro).
daemons_vivos() {
  contar_procesos
  printf '%s' "$CONTAR_DAEMONS"
}

# Cuántos supervisores hay (el keeper relanza al suyo: no puede haber dos).
supervisores_vivos() {
  contar_procesos
  printf '%s' "$CONTAR_SUP"
}

# Cuántos watchdogs hay (1 = bien; 2+ = se pelean por el túnel).
watchdogs_vivos() {
  contar_procesos
  printf '%s' "$CONTAR_WD"
}

# Un SOLO barrido de /proc para los cuatro contadores.
#
# MEDIDO: con cuatro barridos separados (uno por contador) el diagnóstico
# tardaba ~16 s en el móvil (el móvil tiene cientos de procesos).  Con uno
# solo, ~4 s.
CONTAR_DAEMONS=0
CONTAR_SUP=0
CONTAR_WD=0
CONTAR_CF=0
contar_procesos() {
  CONTAR_DAEMONS=0
  CONTAR_SUP=0
  CONTAR_WD=0
  CONTAR_CF=0
  for p in /proc/[0-9]*; do
    [ -r "$p/cmdline" ] || continue
    c_c=$(tr '\0' ' ' < "$p/cmdline" 2>/dev/null)
    case "$c_c" in *megadaemon-core*) CONTAR_DAEMONS=$((CONTAR_DAEMONS + 1)) ;; esac
    case "$c_c" in *99-megadaemon.sh*) CONTAR_SUP=$((CONTAR_SUP + 1)) ;; esac
    case "$c_c" in *tunnel-watchdog.sh*) CONTAR_WD=$((CONTAR_WD + 1)) ;; esac
    # El envoltorio `sh -c "… cloudflared …"` no es un cloudflared: no cuenta.
    case "$c_c" in
      "sh -c"*) ;;
      *cloudflared*) CONTAR_CF=$((CONTAR_CF + 1)) ;;
    esac
  done
}

# ¿El cockpit local responde?  (/health va sin credencial; 401/403 = vivo).
#
# MEDIDO (2026-11-25, móvil): el chequeo usaba `toybox wget`, que NO existe en
# este móvil ("toybox: Unknown command wget") ⇒ `health()` SIEMPRE fallaba y el
# script anunciaba "el cockpit no respondió" con el daemon SANO.  Ahora usa
# `descargar()`, que prueba curl y wget de verdad.
health_local() {
  # El fichero de sonda va a un temporal: el state dir puede no existir (o no
  # ser escribible) en un móvil donde el script corre sin root.
  descargar "http://127.0.0.1:$PORT/health" "${TMPDIR:-/tmp}/md-health.$$" 6 >/dev/null 2>&1
}

# ¿El túnel público responde?  Usa la sonda del daemon (distingue 401/403 =
# "el daemon contesta" de 530 = "túnel muerto").
tunel_responde() {
  t_u="$1"
  case "$t_u" in https://*) ;; *) return 1 ;; esac
  if [ -x "$DAEMON" ]; then
    "$DAEMON" telemetria sonda "$t_u" >/dev/null 2>&1 && return 0
  fi
  t_c=$(codigo_http "$t_u")
  case "$t_c" in 200|401|403) return 0 ;; esac
  return 1
}

# Epoch (segundos) en que arrancó el proceso `pid`.
#
# `starttime` de /proc/<pid>/stat (campo 22) son jiffies desde el boot; con
# `btime` de /proc/stat y HZ=100 sale el epoch.
arranque_epoch() {
  a_pid="$1"
  a_st=$(cat "/proc/$a_pid/stat" 2>/dev/null) || return 1
  # El nombre del proceso va entre paréntesis y puede tener espacios: se
  # parte por el ÚLTIMO ") ".
  a_rest=${a_st##*) }
  set -- $a_rest
  a_start=${20}
  a_btime=$(awk '/^btime /{print $2; exit}' /proc/stat 2>/dev/null)
  [ -n "$a_start" ] && [ -n "$a_btime" ] || return 1
  echo $(( a_btime + a_start / 100 ))
}

# ¿El daemon corre el binario que hay en disco?
#
# MEDIDO (2026-11-25, móvil): `readlink /proc/<pid>/exe` NO marca "(deleted)"
# cuando el binario se SUSTITUYE sobrescribiendo el fichero en el sitio (mismo
# inode): el proceso sigue ejecutando el código VIEJO en memoria mientras el
# symlink apunta al binario nuevo.  El criterio fiable es la FECHA: si el
# binario en disco es más nuevo que el arranque del proceso, corre el viejo.
binario_al_dia() {
  b_pid="$1"
  b_exe=$(readlink "/proc/$b_pid/exe" 2>/dev/null)
  case "$b_exe" in
    *"(deleted)") return 1 ;;   # corre el binario viejo
    "$DAEMON") ;;
    "") return 0 ;;             # no se puede saber: no se toca
    *) return 0 ;;
  esac
  b_mtime=$(stat -c %Y "$DAEMON" 2>/dev/null)
  b_arr=$(arranque_epoch "$b_pid")
  [ -n "$b_mtime" ] && [ -n "$b_arr" ] || return 0
  if [ "$b_mtime" -gt "$b_arr" ]; then
    return 1
  fi
  return 0
}

# Espacio libre en el prefijo (MB).  Si no se puede saber => 0.
espacio_libre_mb() {
  e_kb=$(df -k "$PREFIX" 2>/dev/null | tail -1 | awk '{print $4}')
  case "$e_kb" in ''|*[!0-9]*) echo 0; return ;; esac
  echo $(( e_kb / 1024 ))
}

# ¿Hay token de GitHub (0600) en las rutas habituales?  NUNCA se imprime.
token_github_ruta() {
  for t in "$PREFIX/.gh_token" "$PREFIX/github.token" \
           "$STATE/.gh_token" "$STATE/github.token" \
           "$HOME/.mavis/github.token"; do
    if [ -f "$t" ]; then printf '%s' "$t"; return 0; fi
  done
  return 1
}

# ===========================================================================
# 5) INFORME + TELEMETRÍA (el root lo lee desde Drive, sin ADB)
# ===========================================================================
INFORME=""
INFORME_TMP=""
INFORME_PASOS_TMP=""

informe_reset() {
  INFORME="$STATE/instalacion.json"
  INFORME_TMP="$STATE/instalacion.json.entradas"
  INFORME_PASOS_TMP="$STATE/instalacion.json.pasos"
  : > "$INFORME_TMP" 2>/dev/null
  : > "$INFORME_PASOS_TMP" 2>/dev/null
}

# Escapar un texto para JSON (comillas y barras invertidas).
json_esc() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g' | tr '\n' ' '
}

informe_add() {
  # $1 = clave, $2 = valor JSON ya literal
  printf '  "%s": %s,\n' "$1" "$2" >> "$INFORME_TMP" 2>/dev/null
}

informe_add_txt() {
  informe_add "$1" "\"$(json_esc "$2")\""
}

informe_paso() {
  # $1 = nombre, $2 = ok (true/false), $3 = detalle
  printf '    {"nombre": "%s", "ok": %s, "detalle": "%s"},\n' \
    "$(json_esc "$1")" "$2" "$(json_esc "$3")" >> "$INFORME_PASOS_TMP" 2>/dev/null
}

informe_cierra() {
  {
    printf '{\n'
    sed '$ s/,$//' "$INFORME_TMP" 2>/dev/null
    printf ',\n  "pasos": [\n'
    sed '$ s/,$//' "$INFORME_PASOS_TMP" 2>/dev/null
    printf '\n  ]\n}\n'
  } > "$INFORME" 2>/dev/null
  rm -f "$INFORME_TMP" "$INFORME_PASOS_TMP" 2>/dev/null
  [ -s "$INFORME" ] && return 0
  return 1
}

# Publica todo lo publicable: telemetría, conexión, informe y diagnóstico.
publicar_telemetria() {
  if [ "$SIN_TELEMETRIA" = "1" ]; then
    say "  telemetría omitida por --sin-telemetria"
    return 0
  fi
  if [ "$SIN_PUBLICAR" = "1" ]; then
    say "  publicación omitida por --sin-publicar"
    return 0
  fi
  if [ ! -x "$DAEMON" ]; then
    warn "  no hay daemon: no hay nada que publicar"
    return 1
  fi
  say "  publicando telemetría, conexión, informe y diagnóstico en Drive…"
  "$DAEMON" telemetria publicar 2>&1 | head -6 | sed 's/^/    /'
  "$DAEMON" telemetria conexion 2>&1 | head -6 | sed 's/^/    /'
  if [ -s "$INFORME" ]; then
    # El informe se sube como ARTEFACTO a la carpeta PRIVADA de Drive: el
    # repo es público, aquí no van datos del dispositivo.
    if r "'$DAEMON' telemetria artefacto '$INFORME'" 2>&1 | head -4 | sed 's/^/    /'; then
      say "    informe de instalación subido a Drive ($INFORME)"
    fi
  fi
  "$DAEMON" diagnostico --publicar 2>&1 | head -4 | sed 's/^/    /'
  return 0
}

# ===========================================================================
# 6) ESTADO (diagnóstico completo)
# ===========================================================================
DIAG_JSON=""

diagnostico() {
  say "--- daemon ---"
  DIAG_DAEMON=""
  if DIAG_PID=$(daemon_vivo); then
    DIAG_DAEMON="vivo"
    say "  proceso: PID $DIAG_PID ($(tr '\0' ' ' < "/proc/$DIAG_PID/cmdline" 2>/dev/null | cut -c1-90))"
    if binario_al_dia "$DIAG_PID"; then
      DIAG_AL_DIA=true
      say "  binario en servicio = binario en disco ✓"
    else
      DIAG_AL_DIA=false
      warn "  el proceso corre el binario VIEJO (el disco es más nuevo): hay que reiniciarlo"
    fi
  else
    DIAG_DAEMON="muerto"
    DIAG_AL_DIA=false
    warn "  NO CORRE"
  fi
  contar_procesos
  DIAG_N_DAEMONS=$CONTAR_DAEMONS
  say "  daemons en el sistema: $DIAG_N_DAEMONS"
  [ "$DIAG_N_DAEMONS" -gt 1 ] && warn "  hay más de un daemon: se matan entre ellos"

  say "--- salud local (/health) ---"
  if health_local; then
    DIAG_HEALTH=true
    say "  el cockpit responde en 127.0.0.1:$PORT ✓"
  else
    DIAG_HEALTH=false
    warn "  el cockpit NO responde en 127.0.0.1:$PORT"
  fi

  say "--- binario ---"
  DIAG_BIN_SZ=0
  DIAG_BIN_SHA=""
  if [ -x "$DAEMON" ]; then
    DIAG_BIN_SZ=$(wc -c < "$DAEMON" 2>/dev/null | tr -d ' ')
    DIAG_BIN_SHA=$(sha16_de "$DAEMON")
    say "  $DAEMON ($DIAG_BIN_SZ B, sha16 ${DIAG_BIN_SHA:-?})"
  else
    warn "  NO INSTALADO"
  fi
  if [ -f "$DAEMON.prev" ]; then
    say "  hay respaldo (.prev, sha16 $(sha16_de "$DAEMON.prev"))"
  fi

  say "--- stack ---"
  for f in 99-megadaemon.sh 97-megadaemon-keeper.sh 98-cloudflared.sh 91-adb-tcp.sh; do
    p="/data/adb/service.d/$f"
    if [ -f "$p" ]; then say "  $f: OK ($(wc -c < "$p" | tr -d ' ') B)"; else warn "  $f: FALTA"; fi
  done
  if [ -f "$PREFIX/tunnel-watchdog.sh" ]; then
    say "  tunnel-watchdog.sh: OK en el prefijo"
  else
    warn "  tunnel-watchdog.sh: FALTA en el prefijo"
  fi
  DIAG_SUP=$CONTAR_SUP
  DIAG_WD=$CONTAR_WD
  DIAG_CF=$CONTAR_CF
  say "  vivos: supervisor=$DIAG_SUP watchdog=$DIAG_WD cloudflared=$DIAG_CF"
  [ "$DIAG_CF" -gt 1 ] && warn "  hay más de un cloudflared: el hostname gana a sorts"

  say "--- canal ---"
  DIAG_URL=""
  DIAG_QUICK=""
  if [ -f "$STATE/tunnel.url" ]; then
    DIAG_URL=$(cat "$STATE/tunnel.url" 2>/dev/null | tr -d '\r\n ')
    say "  tunnel.url: $DIAG_URL"
  else
    warn "  no hay $STATE/tunnel.url"
  fi
  if [ -f "$STATE/channel.json" ]; then
    DIAG_QUICK=$(sed -n 's/.*"quick"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$STATE/channel.json" 2>/dev/null | head -1)
    say "  channel.json: quick=${DIAG_QUICK:-vacío}"
  else
    warn "  no hay $STATE/channel.json"
  fi
  DIAG_TUNEL=false
  if [ -n "$DIAG_URL" ] && tunel_responde "$DIAG_URL"; then
    DIAG_TUNEL=true
    say "  el túnel responde ✓ (código $(codigo_http "$DIAG_URL"))"
  else
    warn "  el túnel NO responde (si está muerto: el watchdog lo relanza)"
  fi

  say "--- credenciales (0600; nunca se imprimen) ---"
  DIAG_DRIVE=false
  if [ -f "$STATE/gdrive_credentials.json" ]; then
    DIAG_DRIVE=true
    say "  Drive: credenciales presentes ($(wc -c < "$STATE/gdrive_credentials.json" | tr -d ' ') B)"
  else
    warn "  Drive: NO HAY credenciales (no podrá subir telemetría)"
  fi
  DIAG_GH=false
  if token_github_ruta >/dev/null 2>&1; then
    DIAG_GH=true
    say "  GitHub: hay token (el canal podrá ir al repo)"
  else
    warn "  GitHub: sin token (el canal NO irá al repo: la flota puede quedarse ciega)"
  fi

  say "--- disco ---"
  DIAG_ESPACIO=$(espacio_libre_mb)
  say "  libres en $PREFIX: ${DIAG_ESPACIO} MB"

  say "--- logs (últimas líneas) ---"
  for l in "$LOGDIR/daemon.log" "$LOGDIR/tunnel.log" "$LOGDIR/tunnel-watchdog.log"; do
    if [ -f "$l" ]; then
      say "  $l:"
      tail -n 4 "$l" 2>/dev/null | sed 's/^/    /'
    fi
  done

  say "--- canal del daemon ---"
  if [ -x "$DAEMON" ]; then
    "$DAEMON" telemetria conexion 2>/dev/null | head -18 | sed 's/^/  /'
  fi

  # Datos para el informe JSON.
  DIAG_JSON="\"daemon\": \"$DIAG_DAEMON\", \"daemons\": $DIAG_N_DAEMONS, \"health_local\": $DIAG_HEALTH, \"binario_al_dia\": $DIAG_AL_DIA, \"tunel_responde\": $DIAG_TUNEL, \"quick\": \"$(json_esc "$DIAG_QUICK")\", \"supervisores\": $DIAG_SUP, \"watchdogs\": $DIAG_WD, \"cloudflared\": $DIAG_CF, \"drive_creds\": $DIAG_DRIVE, \"gh_token\": $DIAG_GH, \"espacio_mb\": $DIAG_ESPACIO, \"binario_bytes\": $DIAG_BIN_SZ, \"binario_sha16\": \"$DIAG_BIN_SHA\""
}

# ===========================================================================
# 7) INSTALACIÓN
# ===========================================================================
instalar() {
  T0=$(date +%s 2>/dev/null || echo 0)
  informe_reset
  INFORME_RESULTADO="ko"
  INFORME_ERRORES=""

  # --- atajo de idempotencia -------------------------------------------------
  # Si ya está instalado Y el túnel responde, NO se toca nada: matar el
  # daemon y relanzar cloudflared cambiaría el hostname y ciegaría a la flota
  # en cada reejecución (el bug más caro de este script).
  if [ "$FORCE" != "1" ] && [ -x "$DAEMON" ]; then
    URL0=$(r "cat '$STATE/tunnel.url' 2>/dev/null" | tr -d '\r\n ')
    if [ -n "$URL0" ] && tunel_responde "$URL0"; then
      say "ya está instalado y el túnel responde ($URL0): no toco nada."
      say "usa --force si de verdad quieres reinstalar."
      say ""
      say "estado:"
      diagnostico
      informe_add_txt "resultado" "sin cambios (ya instalado y sano)"
      informe_cierra
      publicar_telemetria
      return 0
    fi
  fi

  say "=== 1/8 resolviendo el release ==="
  if ! resolver_release; then
    informe_add_txt "resultado" "no se pudo resolver el release"
    informe_cierra
    die "no se pudo resolver el release (sin tag). ¿Hay red? ¿El repo es público?"
  fi
  say "release: ${TAG:-(última)}"
  informe_paso "resolver_release" true "${TAG:-(última)}"

  say "=== 2/8 descargando el bundle ==="
  # ⚠️ `/data/local/tmp` NO es escribible por el uid de Termux ⇒ el temporal
  # va a $TMPDIR (Termux lo define) o a /tmp.
  WORK=$(mktemp -d 2>/dev/null || echo "${TMPDIR:-/tmp}/mdo.$$")
  mkdir -p "$WORK" 2>/dev/null || die "no se pudo crear el temporal en $WORK"
  TAR="$WORK/$BUNDLE_ASSET"
  SHA="$TAR.sha256"
  # Limpieza al salir, pase lo que pase (incluido un `die`).
  trap 'rm -rf "$WORK" "$PREFIX/.stage.$$" 2>/dev/null' EXIT INT TERM

  # Espacio: el bundle descomprimido + el binario nuevo + el .prev.
  LIBRES=$(espacio_libre_mb)
  if [ "$LIBRES" -gt 0 ] && [ "$LIBRES" -lt 64 ]; then
    warn "quedan ${LIBRES} MB en $PREFIX: puede que la extracción falle"
  fi

  i=1; ok=0
  while [ "$i" -le "$REINTENTOS" ]; do
    if descargar "$ASSET_URL" "$TAR" 900 && [ -s "$TAR" ]; then ok=1; break; fi
    # Backoff cuadrático con tope (no lineal: si la red está caída, insistir
    # cada 3 s no ayuda y gasta datos).
    d=$((i * i)); [ "$d" -gt 30 ] && d=30
    warn "  descarga fallida (intento $i/$REINTENTOS); reintento en ${d}s"
    sleep "$d"
    i=$((i + 1))
  done
  [ "$ok" = "1" ] || {
    informe_paso "descargar" false "bundle"
    informe_cierra
    die "no se pudo descargar el bundle tras $REINTENTOS intentos"
  }
  TAR_BYTES=$(wc -c < "$TAR" | tr -d ' ')
  say "bundle: ${TAR_BYTES} B"

  # El `.sha256`: si existe, la verificación es OBLIGATORIA.  Si el hash
  # NO cuadra, el bundle está manipulado ⇒ se aborta (no se instala).
  if descargar "$ASSET_URL.sha256" "$SHA" 120 && [ -s "$SHA" ]; then
    verificar_sha "$TAR" "$SHA"
    rc=$?
    if [ "$rc" = "0" ]; then
      say "sha256: VERIFICADO por valor"
      informe_paso "sha256" true "verificado por valor"
    elif [ "$rc" = "2" ]; then
      informe_paso "sha256" false "no se pudo calcular"
      informe_cierra
      die "no se pudo calcular el sha256 del bundle (ni sha256sum ni el daemon) => no instalo nada"
    else
      informe_paso "sha256" false "NO cuadra"
      informe_cierra
      die "el sha256 del bundle NO cuadra => el fichero está manipulado o corrupto. NO instalo nada"
    fi
  else
    warn "sha256: no disponible en el release (sigo: la verificación real es que el daemon arranque)"
    informe_paso "sha256" true "no disponible en el release"
  fi

  # El publicador del canal (lo necesita para publicar en el REPO, que es el
  # punto canónico de descubrimiento: el `raw` de GitHub cachea en CDN).
  PUBLISH_BIN="$PREFIX/channel-publish"
  if [ ! -x "$PUBLISH_BIN" ]; then
    if descargar "$RELEASES_LATEST/download/$PUBLISH_ASSET" "$WORK/$PUBLISH_ASSET" 300; then
      r "cp -f '$WORK/$PUBLISH_ASSET' '$PUBLISH_BIN' && chmod 755 '$PUBLISH_BIN'" >/dev/null 2>&1 \
        && say "channel-publish instalado" \
        || warn "no se pudo instalar channel-publish (el canal no irá al repo)"
    else
      warn "no se pudo descargar channel-publish (el canal no irá al repo)"
    fi
  fi

  say "=== 3/8 instalando (atómico, con rollback) ==="
  BACKUP=""
  if [ -f "$DAEMON" ]; then
    BACKUP="$PREFIX/megadaemon-core.prev"
    r "cp -f '$DAEMON' '$BACKUP'" >/dev/null 2>&1 && say "respaldo del binario actual: $BACKUP"
  fi

  STAGE="$PREFIX/.stage.$$"
  r "rm -rf '$STAGE' && mkdir -p '$STAGE'" >/dev/null
  # Si el bundle no se extrae, NO se toca nada y se aborta: el peor resultado
  # posible es "móvil sin daemon + dice que lo instaló".
  if ! r "tar -xzf '$TAR' -C '$STAGE'"; then
    r "rm -rf '$STAGE'" >/dev/null 2>&1
    informe_paso "extraer" false "tar"
    informe_cierra
    die "no se pudo extraer el bundle (tar) => no instalo nada"
  fi

  # Instalar el binario (del stage o del propio bundle).
  NUEVO=""
  for cand in "$STAGE/bin/megadaemon-core" "$STAGE/megadaemon-core" "$STAGE/megadaemon-movil/megadaemon-core"; do
    [ -f "$cand" ] && { NUEVO="$cand"; break; }
  done
  if [ -z "$NUEVO" ]; then
    NUEVO=$(r "find '$STAGE' -name 'megadaemon-core' -type f 2>/dev/null | head -1" | tr -d '\r\n')
  fi
  [ -n "$NUEVO" ] && [ -f "$NUEVO" ] || {
    r "rm -rf '$STAGE'" >/dev/null 2>&1
    informe_paso "instalar_binario" false "no viene en el bundle"
    informe_cierra
    die "el bundle no trae el binario (esperado: bin/megadaemon-core)"
  }
  # .new + mv (atómico en el MISMO sistema de ficheros) + ejecutable.
  # Nunca se copia ENCIMA del binario en ejecución: si el móvil se duerme a
  # mitad, el binario queda truncado y el móvil se queda sin daemon.
  r "mkdir -p '$PREFIX' && cp -f '$NUEVO' '$DAEMON.new' && chmod 755 '$DAEMON.new' && mv -f '$DAEMON.new' '$DAEMON'" >/dev/null
  r "test -x '$DAEMON'" >/dev/null 2>&1 || {
    [ -n "$BACKUP" ] && r "cp -f '$BACKUP' '$DAEMON' && chmod 755 '$DAEMON'" >/dev/null 2>&1
    informe_paso "instalar_binario" false "no ejecutable"
    informe_cierra
    die "el binario nuevo no quedó ejecutable: binario anterior restaurado"
  }
  say "binario instalado: $(r "wc -c < '$DAEMON'" | tr -d ' \r\n') B (sha16 $(sha16_de "$DAEMON"))"
  informe_paso "instalar_binario" true "sha16 $(sha16_de "$DAEMON")"

  # Instalar el stack (Magisk service.d) desde el bundle.
  # ⚠️ `tunnel-watchdog.sh` NO va a service.d: Magisk lo ejecutaría en cada
  # arranque SIN entorno (LOG_DIR, DAEMON_BIN, MEGADAEMON_STATE_DIR) ⇒
  # entraría en un ciclo de relanzamiento de cloudflared (cambia el hostname
  # cada 60 s y ciega a la flota).  Su sitio es el PREFIJO
  # (`rust-core/src/stack.rs`: `Destino::Prefijo("tunnel-watchdog.sh")`).
  for f in 99-megadaemon.sh 97-megadaemon-keeper.sh 98-cloudflared.sh 91-adb-tcp.sh; do
    for cand in "$STAGE/deploy/$f" "$STAGE/megadaemon-movil/$f" "$STAGE/$f"; do
      if [ -f "$cand" ]; then
        r "mkdir -p /data/adb/service.d && cp -f '$cand' /data/adb/service.d/$f && chmod 755 /data/adb/service.d/$f" >/dev/null
        break
      fi
    done
  done
  # El watchdog, al prefijo (lo lanza 98-cloudflared.sh con su entorno).
  for cand in "$STAGE/deploy/tunnel-watchdog.sh" "$STAGE/tunnel-watchdog.sh"; do
    if [ -f "$cand" ]; then
      r "cp -f '$cand' '$PREFIX/tunnel-watchdog.sh' && chmod 755 '$PREFIX/tunnel-watchdog.sh'" >/dev/null
      break
    fi
  done
  say "stack instalado"

  say "=== 4/8 arrancando el stack ==="
  r "mkdir -p '$STATE' '$LOGDIR'" >/dev/null
  # El daemon, CON SU ENTORNO (sin esto escribe el estado en el sitio
  # equivocado) y CON `< /dev/null` (si hereda el stdin, la sesión adb se
  # queda colgada).
  r "pkill -f 'megadaemon-core cockpit' 2>/dev/null; sleep 1" >/dev/null
  r "MEGADAEMON_PREFIX='$PREFIX' MEGADAEMON_STATE_DIR='$STATE' MEGADAEMON_BIN='$DAEMON' setsid nohup '$DAEMON' cockpit $PORT >> '$LOGDIR/daemon.log' 2>&1 < /dev/null &" >/dev/null

  # El túnel: si ya hay un cloudflared (el supervisor puede estar vivo), se
  # RESPETA.  Relanzarlo a ciegas genera un hostname nuevo ⇒ la flota se
  # queda ciega, que es justo lo que este script debe evitar.
  CF_BIN="$PREFIX/cloudflared"
  [ -x "$CF_BIN" ] || CF_BIN="/data/data/com.termux/files/usr/bin/cloudflared"
  if ! r "ps -A -o NAME 2>/dev/null | grep -q '^cloudflared$'" >/dev/null 2>&1; then
    r "setsid nohup '$CF_BIN' tunnel --no-autoupdate --url http://127.0.0.1:$PORT >> '$LOGDIR/tunnel.log' 2>&1 < /dev/null &" >/dev/null
  else
    say "hay un cloudflared vivo: no lo relanzo (el hostname se conserva)"
  fi

  # El watchdog, con SU entorno (tunnel-watchdog.sh:25,39,40,43,52,53).
  r "LOG_DIR='$PREFIX' DAEMON_BIN='$DAEMON' PUBLISH='$PREFIX/channel-publish' MEGADAEMON_STATE_DIR='$STATE' MEGADAEMON_ENV='$PREFIX/megadaemon.env' CF='$CF_BIN' CFT_TARGET='http://127.0.0.1:$PORT' setsid nohup sh '$PREFIX/tunnel-watchdog.sh' >> '$LOGDIR/tunnel-watchdog.log' 2>&1 < /dev/null &" >/dev/null
  say "servicios lanzados"

  # El daemon tiene que ARRANCAR: si no, se restaura el anterior.
  i=0
  while [ "$i" -lt 25 ]; do
    r "pgrep -f 'megadaemon-core cockpit' >/dev/null 2>&1" && break
    sleep 1
    i=$((i + 1))
  done
  if [ "$i" -ge 25 ]; then
    [ -n "$BACKUP" ] && r "cp -f '$BACKUP' '$DAEMON' && chmod 755 '$DAEMON'" >/dev/null 2>&1
    r "pkill -f 'megadaemon-core cockpit' 2>/dev/null" >/dev/null
    r "setsid nohup '$DAEMON' cockpit $PORT >> '$LOGDIR/daemon.log' 2>&1 < /dev/null &" >/dev/null
    informe_paso "arrancar_daemon" false "no arrancó"
    informe_cierra
    die "el daemon no arrancó con el binario nuevo: binario anterior restaurado"
  fi
  # Un daemon y sólo uno: si el supervisor había dejado otro, se limpian los
  # sobrantes (se matan entre ellos y el watchdog hace lo mismo).
  # ⚠️ Sin `xargs -r`: toybox NO lo tiene ("Unknown option") ⇒ bucle a mano.
  N_DAEMONS=$(daemons_vivos)
  if [ "$N_DAEMONS" -gt 1 ]; then
    warn "había $N_DAEMONS daemons: se deja uno"
    n=0
    for p in $(r "pgrep -f 'megadaemon-core cockpit'" | tr ' ' '\n' | grep -E '^[0-9]+$'); do
      n=$((n + 1))
      if [ "$n" -gt 1 ]; then
        r "kill -9 $p" >/dev/null 2>&1
      fi
    done
  fi

  say "=== 5/8 esperando a que esté LISTO (health-check real) ==="
  # La URL viva: del log de cloudflared -> <state_dir>/tunnel.url (lo lee el
  # daemon: `channel.rs`).  Sin esto, `telemetria listo` NUNCA es true.
  URL=""
  i=1
  # Presupuesto de tiempo REAL (--timeout): 2/3 para que aparezca la URL.
  AHORA=$(date +%s 2>/dev/null || echo 0)
  DEADLINE_URL=$(( AHORA + TIMEOUT_S * 2 / 3 ))
  while [ "$i" -le 24 ]; do
    URL=$(r "cat '$STATE/tunnel.url' 2>/dev/null" | tr -d '\r\n ')
    [ -n "$URL" ] && break
    URL=$(r "grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' '$LOGDIR/tunnel.log' 2>/dev/null | tail -1" | tr -d '\r\n ')
    if [ -n "$URL" ]; then
      r "printf '%s' '$URL' > '$STATE/tunnel.url' && chmod 644 '$STATE/tunnel.url'" >/dev/null
      r "printf '%s' '$URL' > '$PREFIX/tunnel.url'" >/dev/null 2>&1
      break
    fi
    [ "$(date +%s 2>/dev/null || echo 0)" -ge "$DEADLINE_URL" ] && break
    sleep 5
    i=$((i + 1))
  done
  [ -n "$URL" ] || warn "no se encontró la URL del túnel (mira $LOGDIR/tunnel.log)"

  listo=0
  i=1
  # El resto del presupuesto (1/3) para la sonda del túnel.
  DEADLINE_SONDA=$(( AHORA + TIMEOUT_S ))
  while [ "$i" -le 12 ]; do
    # El health-check REAL en dos niveles: 401/403 = el daemon contesta =
    # túnel VIVO; 0/502/503/504/530 = muerto (`channel.rs::es_canal_muerto`).
    if [ -n "$URL" ] && r "'$DAEMON' telemetria sonda '$URL'" >/dev/null 2>&1; then
      listo=1
      say "el túnel responde (intento $i): $URL"
      break
    fi
    [ "$(date +%s 2>/dev/null || echo 0)" -ge "$DEADLINE_SONDA" ] && break
    sleep 5
    i=$((i + 1))
  done
  if [ "$listo" = "1" ]; then
    say "LISTO: el móvil es alcanzable en $URL"
    informe_paso "health_check" true "$URL"
  else
    warn "el móvil NO es alcanzable todavía (daemon vivo ≠ túnel vivo)"
    say "  log: $LOGDIR/daemon.log"
    informe_paso "health_check" false "el túnel no responde"
  fi

  say "=== 6/8 publicando el canal (repo + relé + paste + Drive) ==="
  if [ -x "$DAEMON" ]; then
    r "'$DAEMON' channel publish" 2>/dev/null | head -12 | sed 's/^/  /'
  fi
  # El REPO (Contents API: NO cachea en CDN) es el punto canónico de
  # descubrimiento: sin esto, cuando cambia el hostname del quick tunnel la
  # flota se queda ciega.  Lo publica `channel-publish --repo-publish`.
  if [ -x "$PREFIX/channel-publish" ]; then
    publico=0
    for t in "$PREFIX/.gh_token" "$PREFIX/github.token" "$STATE/.gh_token" "$STATE/github.token"; do
      [ -f "$t" ] || continue
      r "'$PREFIX/channel-publish' --repo-publish --state-dir '$STATE' --token-file '$t'" >/dev/null 2>&1 && publico=1
      break
    done
    if [ "$publico" = "1" ]; then
      say "  canal publicado en el repo (state/channel.json)"
      informe_paso "canal_repo" true "state/channel.json"
    else
      warn "  el canal NO se pudo publicar en el repo (¿token de GitHub?)"
      informe_paso "canal_repo" false "sin token o fallo"
    fi
  else
    warn "  falta channel-publish: el canal no se publica en el repo"
  fi

  say "=== 7/8 telemetría a Google Drive ==="
  publicar_telemetria

  say "=== 8/8 comprobación final ==="
  diagnostico
  r "rm -rf '$STAGE' '$WORK'" >/dev/null 2>&1
  T1=$(date +%s 2>/dev/null || echo 0)
  say ""
  if [ "$listo" = "1" ]; then
    say "instalación terminada: el móvil es alcanzable en $URL"
    say "y su telemetría se lee desde Drive sin depender del túnel."
    INFORME_RESULTADO="ok"
  else
    warn "instalación terminada, pero el túnel no responde aún."
    warn "revisar $LOGDIR/daemon.log y $LOGDIR/tunnel.log"
    INFORME_RESULTADO="ko"
  fi
  informe_add_txt "resultado" "$INFORME_RESULTADO"
  informe_add_txt "release" "${TAG:-(última)}"
  informe_add_txt "url" "${URL:-}"
  informe_add "bundle_bytes" "${TAR_BYTES:-0}"
  informe_add "duracion_s" "$(( T1 - T0 ))"
  informe_add "modo" "\"instalar\""
  informe_add "diagnostico" "{ $DIAG_JSON }"
  informe_cierra
  publicar_telemetria
  return 0
}

# ===========================================================================
# MODO PC (el móvil por adb)
# ===========================================================================
instalar_pc() {
  say "=== modo PC: instalo en el móvil por adb ==="
  if ! command -v adb >/dev/null 2>&1; then
    die "no hay adb en el PC"
  fi
  SERIE=""
  if [ -n "${MD_SERIAL:-}" ]; then
    SERIE="-s $MD_SERIAL"
  fi
  adb devices | sed 's/^/  /'
  adb $SERIE shell "id" 2>/dev/null | sed 's/^/  /' || die "adb no contesta"
  # Se sube el script al móvil y se ejecuta allí (allí tiene root y red).
  LOCAL="/data/local/tmp/megadaemon-oneliner.sh"
  adb $SERIE push "$SELF" "$LOCAL" >/dev/null 2>&1 || die "no se pudo subir el script"
  adb $SERIE shell "chmod 755 $LOCAL" >/dev/null 2>&1
  say "ejecutando en el móvil…"
  adb $SERIE shell "sh $LOCAL --instalar" 2>&1 | sed 's/^/  /'
  say "hecho. El móvil ya sube su telemetría a Drive solo."
}

# ===========================================================================
# MAIN
# ===========================================================================
case "$MODO" in
  diagnostico)
    informe_reset
    diagnostico
    informe_add_txt "resultado" "diagnostico"
    informe_add "diagnostico" "{ $DIAG_JSON }"
    informe_cierra
    publicar_telemetria
    ;;
  instalar)         instalar ;;
  pc)               instalar_pc ;;
  solo-telemetria)
    [ -x "$DAEMON" ] || die "el daemon no está instalado"
    "$DAEMON" telemetria publicar
    "$DAEMON" telemetria conexion
    ;;
  *) die "modo desconocido: $MODO" ;;
esac
