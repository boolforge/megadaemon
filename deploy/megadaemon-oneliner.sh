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
    --doctor)                  MODO="doctor" ;;
    --reconectar)              MODO="reconectar" ;;
    --instalar|--arreglar)     MODO="instalar" ;;
    --pc)                      MODO="pc" ;;
    --solo-telemetria)         MODO="solo-telemetria" ;;
    --force)                   FORCE=1 ;;
    --sin-telemetria)          SIN_TELEMETRIA=1 ;;
    --sin-publicar)            SIN_PUBLICAR=1 ;;
    --reintentos=*)            REINTENTOS="${a#*=}" ;;
    --timeout=*)               TIMEOUT_S="${a#*=}" ;;
    --token=*)                 GH_TOKEN="${a#*=}" ;;
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
#
# MEDIDO (2026-11-27) en Termux SIN root: el log estaba en
# /data/adb/megadaemon/state/instalador.log, que sin root NO se puede
# crear, y salia "Permission denied" en CADA línea del instalador (el
# mensaje tapaba la instalación entera).  Ahora se busca un sitio
# ESCRIBIBLE; si no hay ninguno, se sigue en silencio: la pantalla vale.
LOGFILE=""
WORKDIR=""
for _cand in "$STATE/instalador.log" \
            "$HOME/.megadaemon/instalador.log" \
            "/data/data/com.termux/files/home/.megadaemon/instalador.log" \
            "${TMPDIR:-/tmp}/megadaemon-instalador.log"; do
  _d="${_cand%/*}"
  [ "$_d" = "$_cand" ] && _d="."
  if mkdir -p "$_d" 2>/dev/null && : >>"$_cand" 2>/dev/null; then
    LOGFILE="$_cand"
    WORKDIR="$_d"
    break
  fi
done
[ -z "$WORKDIR" ] && WORKDIR="${TMPDIR:-/tmp}"
say() {
  printf '[%s] %s\n' "$(date -u +%H:%M:%S)" "$*"
  if [ -n "$LOGFILE" ]; then
    { printf '[%s] %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" >>"$LOGFILE"; } 2>/dev/null
  fi
}
warn() { printf '[%s] AVISO: %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; }
die()  { printf '[%s] ERROR: %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; exit 1; }

# ===========================================================================
# MODO DOCTOR / RECONECTAR: el nucleo forense (mddoctor, en C) como ROOT.
# No instala nada: diagnostica (y opcionalmente reconecta) el movil tal y
# como esta.  Es lo que se usa cuando el daemon esta MUERTO, que es
# exactamente cuando el bundle no se puede instalar.
# ===========================================================================
DOC="$PREFIX/mddoctor"

modo_doctor() {
  say "=== doctor: diagnostico forense del movil (como root) ==="
  if ! r "test -x '$DOC'" >/dev/null 2>&1; then
    warn "no hay mddoctor ejecutable en $DOC"
    if [ "$SOBRA_ROOT" != "1" ] && [ -z "$AS_ROOT" ]; then
      warn "y ademas no hay root: el prefijo $PREFIX es de root"
      warn "concede root a Termux (Magisk) y reintenta"
    else
      warn "para instalarlo: ejecuta el one-liner SIN --doctor"
    fi
    warn "mientras tanto, el diagnostico nativo del daemon:"
    r "'$DAEMON' doctor" 2>/dev/null || true
    return 1
  fi
  say "nucleo forense: $(r "wc -c < '$DOC'" | tr -d ' \r\n') B"
  if [ "$MODO" = "reconectar" ]; then
    say "--- reconexion (relanza tunel/daemon si hacen falta) ---"
    r "'$DOC' reconectar --apply --espera 30" 2>&1
    return $?
  fi
  say "--- estado (salud) ---"
  r "'$DOC' salud" 2>&1
  say "--- entorno y permisos reales ---"
  r "'$DOC' entorno" 2>&1
  return 0
}

# ===========================================================================
# 0) AUTODETECCIÓN
# ===========================================================================
ES_ANDROID=0
ES_TERMUX=0
# MEDIDO (2026-11-27): el detector miraba `/data/adb`, que Termux NO puede
# leer sin root => decia "android: 0", se creia en un PC y NUNCA pedia
# root => no instalaba nada.  Ahora se detecta por lo que SI se puede ver
# sin root: /system (legible por todos), la propia ruta de Termux, getprop y
# uname -o.
if [ -d "/system/bin" ] || [ -f "/system/build.prop" ] || [ -x /system/bin/getprop ] \
   || [ -d "/data/data/com.termux" ] || [ "$(uname -o 2>/dev/null)" = "Android" ]; then
  ES_ANDROID=1
fi
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
elif [ "$ES_ANDROID" = "1" ] || command -v su >/dev/null 2>&1 \
     || command -v sudo >/dev/null 2>&1; then
  # Si YA hay root, se USA: no se le pide al usuario que lo conceda otra vez.
  # Se prueban en orden y se queda con el primero que DE VERDAD da uid 0:
  #   sudo -n              (sin password: -n, para no colgarse esperando una
  #                         contrasena que en un terminal no hay)
  #   su -c                (Magisk moderno)
  #   su -mm -c            (Magisk legacy, "mount master")
  #   /system/bin/su ...   (Magisk lo monta ahi, y no siempre esta en el PATH
  #                         de Termux)
  # Con `</dev/null` el `su` que pida contrasena falla en vez de COLGARSE.
  # Magisk pide permiso en pantalla: es normal, hay que aceptarlo.
  for _c in "sudo -n" "su -c" "su -mm -c" "/system/bin/su -c" "/system/bin/su -mm -c"; do
    if $_c "id -u" </dev/null 2>/dev/null | grep -q '^0$'; then
      AS_ROOT="$_c"
      break
    fi
  done
  if [ -z "$AS_ROOT" ]; then
    warn "no se pudo obtener root: concede root a Termux en el panel de Magisk y reintenta"
  else
    say "root obtenido con: $AS_ROOT"
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

# --- preguntas y ejecuciones sobre rutas de ROOT ---------------------------
# MEDIDO (2026-11-27, móvil SM-G998B): /data/adb es 0700 (sólo root).  Con el
# script corriendo como usuario normal, `[ -f "$PREFIX/..." ]` da FALSO
# SIEMPRE y ejecutar "$DAEMON" da "Permission denied".  Eso producía cuatro
# bugs reales, todos vistos en el móvil:
#   * "el bundle no trae el binario" con el bundle CORRECTO delante
#     (el bundle sí traía bin/megadaemon-core: 13.101.098 B, sha256 OK);
#   * sin idempotencia: reinstalaba y cambiaba la URL del túnel en cada
#     ejecución (el bug más caro de este script);
#   * sin copia de seguridad: el rollback no tenía de qué volver;
#   * un diagnóstico que siempre decía "NO INSTALADO".
# Toda pregunta, lectura y ejecución sobre rutas de root se hace COMO ROOT.
existe()     { r "test -e '$1'" >/dev/null 2>&1; }
ejecutable() { r "test -x '$1'" >/dev/null 2>&1; }
# Ejecuta el daemon COMO ROOT (vive en $PREFIX, que es de root).
daemon()     { r "'$DAEMON' $*" 2>&1; }

# --- inventario de procesos POR /proc (toybox no tiene pgrep -f fiable) ---
# MEDIDO (2026-11-27, móvil SM-G998B): `pgrep -f` y `pkill -f` NO son de
# fiar en toybox (el shell de Android) => la limpieza de duplicados NO
# MATABA NADA y el móvil se quedaba con 6 supervisores, 6 daemons, 6
# cloudflared y 5 watchdogs (load 58 => el túnel no podía arrancar y cada
# watchdog relanzaba cloudflared por su cuenta => hostname nuevo cada 60 s
# y la flota ciega).
# Aquí los PIDs se listan leyendo /proc COMO ROOT (el cmdline de un proceso
# de root sólo lo lee root) y luego se descarta el envoltorio `sh -c` (matar
# un envoltorio NO mata al daemon: al revés, se mata al bueno y queda el
# envoltorio).  Se admiten varias subcadenas ("cloudflared" + "tunnel").
# MEDIDO (2026-11-27, banco): si el cmdline NO se puede leer (proceso zombie
# o PID ficticio del laboratorio), el PID se acepta tal cual => el banco
# puede simular duplicados sin tocar procesos de verdad del host.
proc_pids() {
  _pat=$1; shift
  # ⚠️ MEDIDO (2026-11-28, banco): el `sed` va en este ORDEN a proposito.
  # `grep -l` devuelve la RUTA ("/proc/1234/cmdline").  Con `s#^.*/##`
  # (VORAZ) se comia "/proc/1234/" y el resultado era "cmdline", no el PID
  # => `kill -9 cmdline` no mataba NADA y la limpieza de duplicados del movil
  # seguia siendo un no-op.  Primero se quita "/cmdline" y LUEGO el
  # directorio (que es el PID).
  _cand=$(r "grep -lsa '$_pat' /proc/[0-9]*/cmdline 2>/dev/null" | sed 's#/cmdline$##; s#.*/##')
  [ -z "$_cand" ] && return 0
  for _p in $_cand; do
    # MEDIDO (2026-11-28, banco): `r` fusiona stderr con stdout => el
    # error del shell ("cannot open /proc/999/cmdline") llegaba como si
    # fuera el cmdline => el PID se DESCARTABA y la limpieza no mataba
    # nada.  Con `cat 2>/dev/null < ...` el 2>/dev/null se aplica ANTES de
    # la redireccion => el error del shell tambien se calla y el cmdline
    # ilegible sale VACIO (que es lo que distingue "no se puede leer").
    _pc=$(r "cat '/proc/$_p/cmdline' 2>/dev/null | tr '\0' ' '")
    if [ -n "$_pc" ]; then
      case "$_pc" in *"sh -c"*) continue ;; esac
      case "$_pc" in *"$_pat"*) ;; *) continue ;; esac
      _ok=1
      for _x in "$@"; do
        case "$_pc" in *"$_x"*) ;; *) _ok=0 ;; esac
      done
      [ "$_ok" = "1" ] || continue
    fi
    printf '%s\n' "$_p"
  done
  return 0
}
proc_vivo() { [ -n "$(proc_pids "$@")" ]; }

# Mata un PID forzando el binario EXTERNO de `kill`.
# MEDIDO (2026-11-28, banco): `kill` es un BUILTIN de sh/dash => el builtin
# tapa al binario y el `kill` de mentira del banco (que es el que sabe qué
# PIDs "existen") NUNCA se ejecutaba.  `env kill` sí llega al binario: en el
# móvil el de toybox, en el banco el falso.  El builtin queda de reserva.
matar_pid() {
  r "env kill -9 $1 2>/dev/null || kill -9 $1 2>/dev/null" >/dev/null 2>&1
}

# Deja UNO de cada rol del stack (el más ANTIGUO: el orden de /proc es
# numérico) y mata los sobrantes.
# ⚠️ NUNCA se tocan procesos del usuario: sólo los del stack de MegaDaemon.
dejar_uno() {
  _pat=$1; _tag=$2; shift 2
  _pids=$(proc_pids "$_pat" "$@")
  [ -z "$_pids" ] && return 0
  _n=0
  for _p in $_pids; do
    _n=$((_n + 1))
    if [ "$_n" -gt 1 ]; then
      # `env kill` (y no el builtin) para que en el banco llegue al `kill`
      # de mentira; en el móvil cae en el kill de toybox.
      matar_pid "$_p"
      say "  $_tag duplicado (PID $_p): eliminado (se deja el más antiguo)"
    fi
  done
  [ "$_n" -gt 1 ] && return 1
  return 0
}

# Limpia el stack MULTIPLICADO antes de arrancar nada más.
# MEDIDO (2026-11-27, móvil): era la causa del "pastel de Android": 6
# supervisores + 6 cloudflared + 5 watchdogs => load 58, el túnel no
# arrancaba y la URL cambiando cada minuto.
limpiar_stack_duplicado() {
  say "--- limpiando duplicados del stack (si los hay) ---"
  dejar_uno "tunnel-watchdog.sh" "watchdog"
  dejar_uno "97-megadaemon-keeper.sh" "keeper"
  dejar_uno "99-megadaemon.sh" "supervisor"
  dejar_uno "cloudflared" "cloudflared" "tunnel"
  dejar_uno "megadaemon-core" "daemon" "cockpit"
  return 0
}

# Mata TODOS los daemons (no sólo los duplicados) antes de relanzar el
# nuestro: así el daemon nuevo es el único y el más reciente.
matar_daemons() {
  for _p in $(proc_pids "megadaemon-core" "cockpit"); do
    matar_pid "$_p"
  done
  return 0
}

# Planta el token de GitHub en el prefijo (0600) si se ha pasado uno.
# MEDIDO (2026-11-27): el canal al repo NO se publicaba porque el token no
# estaba en el móvil; el paso manual (printf > .gh_token) queda aquí
# codificado, sin imprimir el token en ningún log.
# ⚠️ El token de la LÍNEA DE COMANDOS manda sobre el del entorno (este bloque
# se ejecuta DESPUÉS del parseo de opciones: si no, lo pisaría).
GH_TOKEN="${MEGADAEMON_GH_TOKEN:-${GH_TOKEN:-}}"
plantar_token() {
  [ -n "$GH_TOKEN" ] || return 0
  case "$GH_TOKEN" in
    ghp_*|github_pat_*|gho_*) : ;;
    *) warn "el token no tiene forma de token de GitHub: se ignora"; return 1 ;;
  esac
  if r "printf '%s' '$GH_TOKEN' > '$PREFIX/.gh_token' && chmod 600 '$PREFIX/.gh_token'" >/dev/null 2>&1; then
    say "token de GitHub plantado en $PREFIX/.gh_token (0600)"
    informe_paso "token_github" true "plantado en $PREFIX/.gh_token"
  else
    warn "no se pudo plantar el token en $PREFIX/.gh_token"
    informe_paso "token_github" false "no se pudo escribir"
  fi
  return 0
}

# El BUS DE ÓRDENES (Drive): el único canal de mando que funciona SIN
# túnel.  MEDIDO (2026-11-27): con el quick tunnel caído, el móvil siguió
# recibiendo órdenes por el tablón de Drive => hay que dejarlo SIEMPRE
# encendido (`bus.ciclo_min`).
asegurar_bus() {
  r "test -f '$ENV_FILE'" >/dev/null 2>&1 || return 0
  if r "grep -q '^bus.ciclo_min=' '$ENV_FILE'" >/dev/null 2>&1; then
    r "sed -i 's/^bus.ciclo_min=.*/bus.ciclo_min=10/' '$ENV_FILE'" >/dev/null 2>&1
  else
    r "printf 'bus.ciclo_min=10\n' >> '$ENV_FILE'" >/dev/null 2>&1
  fi
  r "grep -q '^bus.ciclo_min=' '$ENV_FILE'" >/dev/null 2>&1 \
    && say "bus de ordenes (Drive) activo: bus.ciclo_min=10"
  return 0
}

# --- las copias .bak de service.d: la causa del supervisor duplicado ------
# MEDIDO (2026-11-28, movil): Magisk ejecuta TODO lo que sea EJECUTABLE en
# `/data/adb/service.d/`.  Una copia de seguridad del supervisor ahi
# (`99-megadaemon.sh.bak-50k`) = un supervisor MAS en cada arranque, que
# relanzaba el tunel legacy contra el 8765 (sin listener) y se peleaba con el
# canonico (medido: 70 s / 133 s alternados).  Matar el proceso NO basta: el
# FICHERO vuelve a cerrarlo en el siguiente arranque => hay que BORRARLO.
#
# La regla es la MISMA que `stack::limpiar_servicios_sobrantes` (Rust): solo
# los ficheros cuyo nombre empieza por el de un canonico del stack y que NO
# son el canonico.  Los ficheros de OTRAS apps no se tocan, y los directorios
# tampoco.
SERVICE_D="/data/adb/service.d"
STACK_CANONICOS="99-megadaemon.sh 97-megadaemon-keeper.sh 98-cloudflared.sh 91-adb-tcp.sh"

limpiar_copias_service_d() {
  _n=0
  for _f in $(r "ls -1 '$SERVICE_D' 2>/dev/null" | tr -d '\r'); do
    _es_canonico=0
    for _c in $STACK_CANONICOS; do
      [ "$_f" = "$_c" ] && _es_canonico=1
    done
    [ "$_es_canonico" = "1" ] && continue
    # Solo los NUESTROS: el nombre empieza por el de un canonico.
    _nuestro=0
    for _c in $STACK_CANONICOS; do
      case "$_f" in
        "${_c%.sh}"*) _nuestro=1 ;;
      esac
    done
    [ "$_nuestro" = "1" ] || continue
    if r "rm -f '$SERVICE_D/$_f'" >/dev/null 2>&1; then
      say "  service.d: copia sobrante eliminada ($_f)"
      _n=$((_n + 1))
    fi
  done
  [ "$_n" -gt 0 ] && informe_paso "service_d_limpio" true "$_n copia(s) eliminada(s)"
  return 0
}

# --- el supervisor CANONICO se queda; las copias mueren --------------------
# MEDIDO (2026-11-28): dejar "el mas antiguo" (lo que hace `dejar_uno`) es
# ERROR: la copia `.bak-50k` se creo al actualizar, asi que es MAS ANTIGUA
# que el canonico => se quedaba el malo y se mataba el bueno.  El que se
# queda es el que apunta a la RUTA CANONICA.
matar_supervisores_no_canonicos() {
  # ⚠️ El patron es "99-megadaemon.sh" (con .sh): el grep del banco indexa
  # los PID por nombre de fichero ("99-megadaemon.sh.pids") y con
  # "99-megadaemon" no encontraba NINGUNO => la funcion era un no-op y la
  # copia .bak se quedaba de supervisor.
  _pids=$(r "grep -lsa '99-megadaemon.sh' /proc/[0-9]*/cmdline 2>/dev/null" | sed 's#/cmdline$##; s#.*/##')
  [ -z "$_pids" ] && return 0
  _n=0
  for _p in $_pids; do
    # El argv, elemento a elemento (los NUL son separadores).
    # (mismo motivo que en `proc_pids`: el error del shell contaminaba el
    # argv => se creia haber leido el cmdline de un proceso que no existe)
    _argv=$(r "cat '/proc/$_p/cmdline' 2>/dev/null | tr '\0' '\n'")
    # Si el cmdline NO se puede leer (zombie, PID ficticio del banco):
    # NO se mata a nadie (matar a ciegas es peor que dejar un duplicado).
    [ -z "$_argv" ] && continue
    _pc=$(printf '%s' "$_argv" | tr '\n' ' ')
    case "$_pc" in
      *"sh -c"*) continue ;;   # el envoltorio, no el supervisor
    esac
    # ¿Es el supervisor CANONICO?  Se comparan los argumentos ELEMENTO a
    # elemento: la copia tiene la ruta canonica como PREFIJO
    # (`.../99-megadaemon.sh.bak-50k`) => con un `case` sobre el texto
    # unido la copia PASABA por canonica y se quedaba el malo.
    if printf '%s\n' "$_argv" | grep -qx "$SERVICE_D/99-megadaemon.sh"; then
      continue
    fi
    matar_pid "$_p"
    say "  supervisor NO canonico eliminado (PID $_p)"
    _n=$((_n + 1))
  done
  [ "$_n" -gt 0 ] && informe_paso "supervisores_no_canonicos" true "$_n eliminado(s)"
  return 0
}

# --- VERIFICACION FINAL: lo que se puede MIRAR en el movil ----------------
# MEDIDO (2026-11-28): el instalador decia "terminado" sin comprobar nada de
# lo que el jefe tiene que verificar a mano.  Aqui se comprueba y se ANOTA en
# el informe JSON: pegar la salida del one-liner basta para saber el estado.
#
# Las comprobaciones (cada una con su por que):
#  1) service.d tiene SOLO el supervisor canonico (Magisk ejecuta todo lo
#     ejecutable de ahi => una copia = un supervisor mas en cada arranque).
#  2) hay UN supervisor vivo (no cuatro).
#  3) el 8765 (0x2235, el tunel legacy) NO tiene listener.
#  4) el 8080 (el que usa la app) y el 9090 (el del daemon) SI lo tienen.
#  5) el tunel legacy esta APAGADO (su fichero de URL vacio).
#  6) el tunel web ha publicado su URL (sin esto no hay nada que pegar).
#  7) la base de la instancia (board.sqlite) existe: ahi vive el esquema
#     medido (las sesiones ACP y el tablon).
#  8) el diagnostico no reporta el stack desfasado.
VERIFICACIONES_OK=0
VERIFICACIONES_TOTAL=0
verificar_movil() {
  say "--- verificacion final (lo que se puede mirar en el movil) ---"
  # Tres contadores, no dos: una comprobacion SIN DATOS (el daemon no
  # informa de esa clave) NO es un fallo => no puede poner el veredicto en
  # "ok_con_avisos".  MEDIDO (2026-11-28, banco): el daemon de mentira
  # devolvia `{"ok":true}` => todo salia "sin datos".
  VERIFICACIONES_OK=0
  VERIFICACIONES_FALLO=0
  VERIFICACIONES_SIN_DATOS=0
  VERIFICACIONES_TOTAL=0
  _djson=$(r "'$DAEMON' diagnostico --json 2>/dev/null" | tr -d '\r\n')
  _sup=$(printf '%s' "$_djson" | grep -o '"supervisores":[0-9]*' | head -1 | cut -d: -f2)
  _pt=$(printf '%s' "$_djson" | grep -o '"puertos":{[^}]*}' | head -1)
  _desfasados=$(printf '%s' "$_djson" | grep -o '"desfasados":[0-9]*' | head -1 | cut -d: -f2)

  # 1) service.d limpio (POSIX: sin sustitucion de procesos).
  _sobras=0
  for _f in $(r "ls -1 '$SERVICE_D' 2>/dev/null" | tr -d '\r'); do
    _es_canonico=0
    for _c in $STACK_CANONICOS; do
      [ "$_f" = "$_c" ] && _es_canonico=1
    done
    [ "$_es_canonico" = "1" ] && continue
    case "$_f" in
      *megadaemon*|*cloudflared*) _sobras=$((_sobras + 1)) ;;
    esac
  done
  if [ "$_sobras" = "0" ]; then
    say "  service.d: solo los canonicos"
    informe_paso "verif_service_d" true "solo los canonicos"
    VERIFICACIONES_OK=$((VERIFICACIONES_OK + 1))
  else
    warn "  service.d: $_sobras fichero(s) nuestro(s) de mas"
    informe_paso "verif_service_d" false "$_sobras fichero(s) de mas"
    VERIFICACIONES_FALLO=$((VERIFICACIONES_FALLO + 1))
  fi
  VERIFICACIONES_TOTAL=$((VERIFICACIONES_TOTAL + 1))

  # 2) un supervisor vivo (el canonico).
  if [ -z "${_sup:-}" ]; then
    say "  supervisores: sin datos (el daemon no lo informa)"
    informe_paso "verif_supervisores" null "sin datos"
    VERIFICACIONES_SIN_DATOS=$((VERIFICACIONES_SIN_DATOS + 1))
  elif [ "$_sup" = "1" ]; then
    say "  supervisores: 1 (el canonico)"
    informe_paso "verif_supervisores" true "1"
    VERIFICACIONES_OK=$((VERIFICACIONES_OK + 1))
  else
    warn "  supervisores: $_sup (deberia ser 1)"
    informe_paso "verif_supervisores" false "$_sup"
    VERIFICACIONES_FALLO=$((VERIFICACIONES_FALLO + 1))
  fi
  VERIFICACIONES_TOTAL=$((VERIFICACIONES_TOTAL + 1))

  # 3) el 8765 (el tunel legacy) NO debe tener listener.
  if [ -z "${_pt:-}" ]; then
    say "  8765 (tunel legacy): sin datos (el daemon no informa de puertos)"
    informe_paso "verif_8765" null "sin datos"
    VERIFICACIONES_SIN_DATOS=$((VERIFICACIONES_SIN_DATOS + 1))
  elif printf '%s' "$_pt" | grep -q '"2235":true'; then
    warn "  el 8765 (tunel legacy) tiene listener: sobra"
    informe_paso "verif_8765" false "el 8765 tiene listener"
    VERIFICACIONES_FALLO=$((VERIFICACIONES_FALLO + 1))
  else
    say "  8765 (tunel legacy): sin listener (correcto)"
    informe_paso "verif_8765" true "sin listener"
    VERIFICACIONES_OK=$((VERIFICACIONES_OK + 1))
  fi
  VERIFICACIONES_TOTAL=$((VERIFICACIONES_TOTAL + 1))

  # 4) los puertos que importan (la app en el 8080, el daemon en el 9090).
  if [ -z "${_pt:-}" ]; then
    say "  puertos: sin datos"
    informe_paso "verif_puertos" null "sin datos"
    VERIFICACIONES_SIN_DATOS=$((VERIFICACIONES_SIN_DATOS + 1))
  else
    _p8080=0; _p9090=0
    printf '%s' "$_pt" | grep -q '"8080":true' && _p8080=1
    printf '%s' "$_pt" | grep -q '"9090":true' && _p9090=1
    if [ "$_p8080" = "1" ] && [ "$_p9090" = "1" ]; then
      say "  puertos: 8080 y 9090 con listener (la app y el daemon)"
      informe_paso "verif_puertos" true "8080 y 9090"
      VERIFICACIONES_OK=$((VERIFICACIONES_OK + 1))
    else
      warn "  puertos: 8080=$_p8080 9090=$_p9090 (la app usa el 8080)"
      informe_paso "verif_puertos" false "8080=$_p8080 9090=$_p9090"
      VERIFICACIONES_FALLO=$((VERIFICACIONES_FALLO + 1))
    fi
  fi
  VERIFICACIONES_TOTAL=$((VERIFICACIONES_TOTAL + 1))

  # 5) el tunel legacy (8765) esta APAGADO: su fichero de URL vacio.
  #    MEDIDO (2026-11-27, movil): el supervisor VIEJO relanzaba el tunel
  #    `mcp-legacy` en bucle (12 reinicios/15 min medidos) => el pastel de
  #    puertos y la URL cambiando cada minuto.  El invariante real no es el
  #    log (puede conservar una mencion historica) sino que no haya URL.
  _legacy_url=$(r "cat '$PREFIX/mcp_tunnel.url' 2>/dev/null" | tr -d '\r\n')
  if [ -z "$_legacy_url" ]; then
    say "  tunel legacy (8765): apagado (sin URL)"
    informe_paso "verif_legacy" true "sin URL"
    VERIFICACIONES_OK=$((VERIFICACIONES_OK + 1))
  else
    warn "  tunel legacy (8765): VIVO ($_legacy_url)"
    informe_paso "verif_legacy" false "$_legacy_url"
    VERIFICACIONES_FALLO=$((VERIFICACIONES_FALLO + 1))
  fi
  VERIFICACIONES_TOTAL=$((VERIFICACIONES_TOTAL + 1))

  # 6) el tunel web (el bueno) ha publicado su URL: sin esto no hay nada
  #    que pegar en el chat.
  _web_url=$(r "cat '$PREFIX/tunnel.url' 2>/dev/null" | tr -d '\r\n')
  if [ -n "$_web_url" ]; then
    say "  tunel web: URL viva"
    informe_paso "verif_url" true "$_web_url"
    VERIFICACIONES_OK=$((VERIFICACIONES_OK + 1))
  else
    warn "  tunel web: SIN URL (el supervisor no ha publicado tunnel.url)"
    informe_paso "verif_url" false "sin tunnel.url"
    VERIFICACIONES_FALLO=$((VERIFICACIONES_FALLO + 1))
  fi
  VERIFICACIONES_TOTAL=$((VERIFICACIONES_TOTAL + 1))

  # 7) la base de la instancia (ahi vive el esquema medido).
  if r "test -f '$STATE/board.sqlite'" >/dev/null 2>&1; then
    say "  base de la instancia: $STATE/board.sqlite"
    informe_paso "verif_base" true "$STATE/board.sqlite"
    VERIFICACIONES_OK=$((VERIFICACIONES_OK + 1))
  else
    warn "  no hay base en $STATE/board.sqlite"
    informe_paso "verif_base" false "falta board.sqlite"
    VERIFICACIONES_FALLO=$((VERIFICACIONES_FALLO + 1))
  fi
  VERIFICACIONES_TOTAL=$((VERIFICACIONES_TOTAL + 1))

  # 8) el stack no esta desfasado (el binario en disco = el desplegado).
  if [ -z "${_desfasados:-}" ]; then
    say "  stack: sin datos (el daemon no lo informa)"
    informe_paso "verif_stack" null "sin datos"
    VERIFICACIONES_SIN_DATOS=$((VERIFICACIONES_SIN_DATOS + 1))
  elif [ "$_desfasados" = "0" ]; then
    say "  el stack esta en linea con el binario"
    informe_paso "verif_stack" true "en linea"
    VERIFICACIONES_OK=$((VERIFICACIONES_OK + 1))
  else
    warn "  el stack tiene $_desfasados fichero(s) desfasado(s) (mira 'stack --json')"
    informe_paso "verif_stack" false "$_desfasados desfasados"
    VERIFICACIONES_FALLO=$((VERIFICACIONES_FALLO + 1))
  fi
  VERIFICACIONES_TOTAL=$((VERIFICACIONES_TOTAL + 1))

  say "  verificaciones: $VERIFICACIONES_OK/$VERIFICACIONES_TOTAL"
  [ "$VERIFICACIONES_SIN_DATOS" -gt 0 ] && \
    say "  ($VERIFICACIONES_SIN_DATOS sin datos: el daemon no las informa)"
  informe_add "verificaciones_ok" "$VERIFICACIONES_OK"
  informe_add "verificaciones_total" "$VERIFICACIONES_TOTAL"
  informe_add "verificaciones_fallo" "$VERIFICACIONES_FALLO"
  informe_add "verificaciones_sin_datos" "$VERIFICACIONES_SIN_DATOS"
  return 0
}

# Copia el stack del bundle a service.d y el watchdog al prefijo.
# Se extrae a una funcion para que la AUTOREPARACION pueda reponerlo.
instalar_stack() {
  st="$1"
  for f in 99-megadaemon.sh 97-megadaemon-keeper.sh 98-cloudflared.sh 91-adb-tcp.sh; do
    for cand in "$st/deploy/$f" "$st/megadaemon-movil/$f" "$st/$f"; do
      if existe "$cand"; then
        r "mkdir -p /data/adb/service.d && cp -f '$cand' /data/adb/service.d/$f && chmod 755 /data/adb/service.d/$f" >/dev/null
        break
      fi
    done
  done
  # El watchdog, al prefijo (lo lanza 98-cloudflared.sh con su entorno).
  for cand in "$st/deploy/tunnel-watchdog.sh" "$st/tunnel-watchdog.sh"; do
    if existe "$cand"; then
      r "cp -f '$cand' '$PREFIX/tunnel-watchdog.sh' && chmod 755 '$PREFIX/tunnel-watchdog.sh'" >/dev/null
      break
    fi
  done
}

# --- AUTOREPARACION -------------------------------------------------------
# MEDIDO (2026-11-27, movil): la instalacion terminaba con 8/8 y el movil
# MUERTO (vivos: supervisor=0 watchdog=0 cloudflared=0, sin mddoctor, sin
# URL del tunel) y solo avisaba.  Aqui se comprueba y se ARREGLA SOLO:
# se repone lo que falte y, si el tunel no responde, se relanza el stack
# con el nucleo forense (que sabe relanzar el tunel y el daemon).
autorreparar() {
  say "--- autorreparacion (lo que falte, se repone) ---"
  faltan=0
  for f in 99-megadaemon.sh 97-megadaemon-keeper.sh 98-cloudflared.sh 91-adb-tcp.sh; do
    ejecutable "/data/adb/service.d/$f" || { warn "falta /data/adb/service.d/$f"; faltan=1; }
  done
  ejecutable "$PREFIX/tunnel-watchdog.sh" || { warn "falta el watchdog en el prefijo"; faltan=1; }
  ejecutable "$DOC" || { warn "falta el nucleo forense ($DOC)"; faltan=1; }
  if [ "$faltan" = "1" ] && [ -n "$STAGE" ] && [ -d "$STAGE" ]; then
    say "reponiendo el stack desde el bundle"
    instalar_stack "$STAGE"
  fi
  # El tunel: si no responde, se relanza con el nucleo forense y se re-sonda.
  if [ "$listo" != "1" ] && ejecutable "$DOC"; then
    say "el tunel no responde: relanzando el stack con el nucleo forense"
    r "'$DOC' reconectar --apply --espera 30" 2>&1 | tail -n 8 | sed 's/^/    /'
    U2=$(r "cat '$STATE/tunnel.url' 2>/dev/null" | tr -d '\r\n ')
    if [ -z "$U2" ]; then
      U2=$(r "grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' '$LOGDIR/tunnel.log' 2>/dev/null | tail -1" | tr -d '\r\n ')
      [ -n "$U2" ] && r "printf '%s' '$U2' > '$STATE/tunnel.url' && chmod 644 '$STATE/tunnel.url'" >/dev/null
    fi
    if [ -n "$U2" ] && r "'$DAEMON' telemetria sonda '$U2'" >/dev/null 2>&1; then
      listo=1
      URL="$U2"
      say "el tunel responde TRAS la autorreparacion: $URL"
      informe_paso "autorreparacion" true "el tunel responde: $URL"
    else
      warn "tras la autorreparacion el tunel sigue sin responder"
      informe_paso "autorreparacion" false "el tunel sigue caido"
    fi
  fi
  return 0
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
  if ejecutable "$DAEMON"; then
    daemon probe "$d_url" >/dev/null 2>&1
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
  if ejecutable "$DAEMON"; then
    printf '%s' "$(daemon probe "$c_url" | tr -d ' \r\n')"
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
    # ⚠️ MEDIDO (2026-11-27) por el banco del instalador: `tr -d ' '` pegaba
    # el hash con el nombre del fichero ("<hash>  <ruta>" -> "<hash><ruta>")
    # y la comparación NUNCA cuadraba => en el móvil (sin `sha256sum`) el
    # instalador moría siempre con "el sha256 NO cuadra".  Ahora se quita
    # sólo el CR/LF y se coge el PRIMER campo.
    real=$(r "'$DAEMON' hash '$f' 2>/dev/null" | tr -d '\r\n' | awk '{print $1}')
  fi
  [ -n "$real" ] || return 2
  [ "$esperado" = "$real" ] || return 1
  return 0
}

# sha16 de un fichero (para el informe: identificador, no secreto).
sha16_de() {
  # MEDIDO (2026-11-27, movil): sha256sum del usuario normal NO puede leer
  # un fichero de /data/adb (0700) => salia vacio.  Ahora, si falla, se
  # pregunta al daemon COMO ROOT.
  if command -v sha256sum >/dev/null 2>&1; then
    h=$(sha256sum "$1" 2>/dev/null | cut -c1-16)
    [ -n "$h" ] && { printf '%s' "$h"; return 0; }
  fi
  r "'$DAEMON' hash '$1' 2>/dev/null" | tr -d '\r\n' | awk '{print $1}' | cut -c1-16
}

# ===========================================================================
# 4) DIAGNÓSTICO REAL (patrones de tools/recuperar-en-movil.sh)
# ===========================================================================
# ¿Hay un proceso megadaemon-core?  Devuelve el PID.
daemon_vivo() {
  # MEDIDO (2026-11-27, movil): como usuario normal, `/proc/<pid>/cmdline` de
  # un proceso de ROOT no se puede leer => el daemon NUNCA aparecia como
  # vivo.  Se busca en el ambito de ROOT.
  #
  # ⚠️ MEDIDO (2026-11-27) por el banco del instalador: `grep -l` devuelve la
  # RUTA del fichero que casa ("/proc/1234/cmdline").  Con `sed 's#.*/##'`
  # salia "cmdline" (no el PID) => el diagnostico decia
  #   "cannot open /proc/cmdline/cmdline"
  # y la comprobacion de "el binario en servicio es el nuevo" NUNCA podia
  # hacerse (miraba /proc/cmdline/stat).  Ahora se quita "/cmdline" y LUEGO
  # el directorio, que es el PID.
  r "grep -lsa megadaemon-core /proc/[0-9]*/cmdline 2>/dev/null | head -1" \
    | tr -d ' \r\n' | sed 's#/cmdline$##; s#.*/##'
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
  # MEDIDO (2026-11-27, movil SM-G998B): esto leia `/proc/*/cmdline` como
  # USUARIO NORMAL.  Los daemons corren como ROOT => su `cmdline` da
  # "Permission denied" => se contaban 0 => el diagnostico decia "NO
  # CORRE" y "vivos: supervisor=0 watchdog=0 cloudflared=0" CON EL DAEMON
  # SANO (el `/health` respondia 200 y el watchdog logueaba "tunel vivo").
  # Ahora se cuenta COMO ROOT (los helpers `daemon_vivo` y los contadores
  # ya usaban `r`; aqui faltaba el counting).
  CONTAR_DAEMONS=$(r "grep -lsa megadaemon-core /proc/[0-9]*/cmdline 2>/dev/null | wc -l" | tr -d ' \r\n')
  CONTAR_SUP=$(r "grep -lsa 99-megadaemon.sh /proc/[0-9]*/cmdline 2>/dev/null | wc -l" | tr -d ' \r\n')
  CONTAR_WD=$(r "grep -lsa tunnel-watchdog.sh /proc/[0-9]*/cmdline 2>/dev/null | wc -l" | tr -d ' \r\n')
  # El envoltorio `sh -c "… cloudflared …"` NO es un cloudflared: no cuenta.
  CONTAR_CF=$(r "grep -lsa cloudflared /proc/[0-9]*/cmdline 2>/dev/null | grep -v 'sh -c' | wc -l" | tr -d ' \r\n')
  [ -n "$CONTAR_DAEMONS" ] || CONTAR_DAEMONS=0
  [ -n "$CONTAR_SUP" ] || CONTAR_SUP=0
  [ -n "$CONTAR_WD" ] || CONTAR_WD=0
  [ -n "$CONTAR_CF" ] || CONTAR_CF=0
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
  if ejecutable "$DAEMON"; then
    daemon telemetria sonda "$t_u" >/dev/null 2>&1 && return 0
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
  # MEDIDO (2026-11-27): `df` sobre /data/adb como usuario normal NO puede
  # leerla => decia siempre "0 MB libres" y hacia creer que el movil estaba
  # sin disco.  Ahora se pregunta COMO ROOT.
  e_kb=$(r "df -k '$PREFIX' 2>/dev/null" | tail -1 | awk '{print $4}')
  case "$e_kb" in ''|*[!0-9]*) echo 0; return ;; esac
  echo $(( e_kb / 1024 ))
}

# ¿Hay token de GitHub (0600) en las rutas habituales?  NUNCA se imprime.
token_github_ruta() {
  for t in "$PREFIX/.gh_token" "$PREFIX/github.token" \
           "$STATE/.gh_token" "$STATE/github.token" \
           "$HOME/.mavis/github.token"; do
    if existe "$t"; then printf '%s' "$t"; return 0; fi
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
  # Los temporales van al WORKDIR (escribible SIN root): antes iban a $STATE
  # y sin root fallaba con "Permission denied" (MEDIDO 2026-11-27).
  INFORME_TMP="$WORKDIR/instalacion.json.entradas"
  INFORME_PASOS_TMP="$WORKDIR/instalacion.json.pasos"
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
    # Los campos de nivel superior: si NO hay ninguno (instalación que ha
    # muerto antes de cerrar el informe) NO se pone la coma, o el JSON sale
    # INVÁLIDO ("{,").  MEDIDO (2026-11-27) por el banco del instalador.
    if [ -s "$INFORME_TMP" ]; then
      sed '$ s/,$//' "$INFORME_TMP" 2>/dev/null
      printf ',\n'
    fi
    printf '  "pasos": [\n'
    if [ -s "$INFORME_PASOS_TMP" ]; then
      sed '$ s/,$//' "$INFORME_PASOS_TMP" 2>/dev/null
    fi
    printf '\n  ]\n}\n'
  } > "$WORKDIR/instalacion.json.final" 2>/dev/null
  # El informe final vive en $STATE (lo lee el root desde Drive): se escribe
  # COMO ROOT, que es lo que exige el prefijo.
  if [ -s "$WORKDIR/instalacion.json.final" ]; then
    r "mkdir -p '$STATE' && cat '$WORKDIR/instalacion.json.final' > '$INFORME'" >/dev/null 2>&1
  fi
  rm -f "$INFORME_TMP" "$INFORME_PASOS_TMP" "$WORKDIR/instalacion.json.final" 2>/dev/null
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
  # MEDIDO (2026-11-27, movil): `[ ! -x "$DAEMON" ]` como usuario NORMAL da
  # FALSO siempre (/data/adb es 0700 de root) => decia "no hay daemon" con el
  # daemon VIVO y la telemetría NUNCA se publicaba.  Se pregunta COMO ROOT.
  if ! ejecutable "$DAEMON"; then
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
    # MEDIDO (2026-11-27, movil): leer /proc/<pid>/cmdline como usuario NORMAL
    # falla ("cannot open ...") porque el proceso es de ROOT => el PID salia
    # sin linea de comandos.  Se lee COMO ROOT.
    say "  proceso: PID $DIAG_PID ($(r "cat '/proc/$DIAG_PID/cmdline' 2>/dev/null | tr '\0' ' ' | cut -c1-90"))"
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
  if ejecutable "$DAEMON"; then
    DIAG_BIN_SZ=$(r "wc -c < '$DAEMON'" | tr -d ' \r\n ')
    DIAG_BIN_SHA=$(sha16_de "$DAEMON")
    say "  $DAEMON ($DIAG_BIN_SZ B, sha16 ${DIAG_BIN_SHA:-?})"
  else
    warn "  NO INSTALADO"
  fi
  if existe "$DAEMON.prev"; then
    say "  hay respaldo (.prev, sha16 $(sha16_de "$DAEMON.prev"))"
  fi

  say "--- stack ---"
  for f in 99-megadaemon.sh 97-megadaemon-keeper.sh 98-cloudflared.sh 91-adb-tcp.sh; do
    p="/data/adb/service.d/$f"
    if existe "$p"; then say "  $f: OK ($(r "wc -c < '$p'" | tr -d ' \r\n ') B)"; else warn "  $f: FALTA"; fi
  done
  if existe "$PREFIX/tunnel-watchdog.sh"; then
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
  if existe "$STATE/tunnel.url"; then
    DIAG_URL=$(r "cat '$STATE/tunnel.url' 2>/dev/null" | tr -d '\r\n ')
    say "  tunnel.url: $DIAG_URL"
  else
    warn "  no hay $STATE/tunnel.url"
  fi
  if existe "$STATE/channel.json"; then
    DIAG_QUICK=$(r "cat '$STATE/channel.json' 2>/dev/null" | sed -n 's/.*"quick"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)
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
  if existe "$STATE/gdrive_credentials.json"; then
    DIAG_DRIVE=true
    # MEDIDO (2026-11-27, movil): el `wc -c < fichero` se ejecutaba como
    # USUARIO NORMAL y el fichero de credenciales es de ROOT con 0600 =>
    # "Permission denied" (y el tamaño salia vacio).  El tamano se pide en
    # el ambito de ROOT (`r`), que si puede leerlo.
    BYTES=$(r "wc -c < '$STATE/gdrive_credentials.json' 2>/dev/null" | tr -d ' \r\n')
    say "  Drive: credenciales presentes (${BYTES:-?} B)"
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
    if existe "$l"; then
      say "  $l:"
      r "tail -n 4 '$l' 2>/dev/null" | sed 's/^/    /'
    fi
  done

  say "--- canal del daemon ---"
  if ejecutable "$DAEMON"; then
    daemon telemetria conexion | head -18 | sed 's/^/  /'
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

  # --- root: sin esto NO se puede instalar nada --------------------------------
  # MEDIDO (2026-11-27): el instalador se quedaba mudo cuando no tenia root
  # (fallaba al escribir en /data/adb y seguia como si nada).  Aqui se dice
  # ANTES de tocar nada, que es lo unico que el usuario puede arreglar.
  if [ "$SOBRA_ROOT" != "1" ] && [ -z "$AS_ROOT" ]; then
    if [ "$ES_ANDROID" = "1" ]; then
      die "necesito root para instalar en $PREFIX: concede root a Termux (Magisk) y reintenta"
    fi
    die "no hay root: en un PC, ejecuta esto con sudo"
  fi

  # --- atajo de idempotencia -------------------------------------------------
  # Si ya está instalado Y el túnel responde, NO se toca nada: matar el
  # daemon y relanzar cloudflared cambiaría el hostname y ciegaría a la flota
  # en cada reejecución (el bug más caro de este script).
  if [ "$FORCE" != "1" ] && ejecutable "$DAEMON"; then
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

  # --- espacio: sin sitio el movil instala y luego no arranca NADA --------
  # MEDIDO (2026-11-27): el aviso de espacio era una lectura falsa (df como
  # usuario normal sobre /data/adb).  Ahora se mide COMO ROOT y se avisa
  # ANTES de descargar 13 MB.
  ESPACIO=$(espacio_libre_mb)
  if [ "$ESPACIO" -lt 50 ]; then
    warn "quedan $ESPACIO MB libres en $PREFIX: el bundle son 13 MB y el daemon necesita mas"
    warn "libera espacio (o borra releases viejos) y reintenta"
    informe_paso "espacio" false "$ESPACIO MB libres"
  else
    say "espacio: $ESPACIO MB libres en $PREFIX"
    informe_paso "espacio" true "$ESPACIO MB libres"
  fi

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
  # ⚠️ MEDIDO (2026-11-27) por el banco del instalador: un `.sha256` VACÍO
  # no es lo mismo que "no disponible".  Antes, el `&& [ -s "$SHA" ]` lo
  # treataba como ausente (=> "sigo sin verificar nada"), con lo que un
  # fichero corrupto o manipulado pasaba el control.  Ahora: si el `.sha256`
  # se descarga pero viene VACÍO, no se puede verificar NADA => no se instala.
  if descargar "$ASSET_URL.sha256" "$SHA" 120; then
    if [ ! -s "$SHA" ]; then
      informe_paso "sha256" false "vacío"
      informe_cierra
      die "el .sha256 del bundle está VACÍO => no puedo verificar nada => no instalo nada"
    fi
    verificar_sha "$TAR" "$SHA"
    rc=$?
    if [ "$rc" = "0" ]; then
      say "sha256: VERIFICADO por valor"
      informe_paso "sha256" true "verificado por valor"
    elif [ "$rc" = "2" ]; then
      # ⚠️ MEDIDO (2026-11-27) por el banco del instalador: en una
      # instalación EN LIMPIO no hay `sha256sum` (el shell de Android no lo
      # trae) y el daemon aún NO está instalado => el hash no se puede
      # calcular aquí.  Antes el instalador MORÍA ("no se pudo calcular el
      # sha256") y NO INSTALABA NADA: el móvil se quedaba sin daemon.
      # Ahora la verificación se difiere: se hace con el daemon recién
      # extraído, ANTES de instalarlo (ver más abajo, tras extraer).
      say "sha256: no se puede calcular AÚN (ni sha256sum ni el daemon instalado)"
      say "  lo verifico con el daemon del propio bundle antes de instalarlo"
      SHA_PENDIENTE=1
      SHA_ESPERADO=$(tr -d '\r\n' < "$SHA" | awk '{print $1}')
      informe_paso "sha256" true "pendiente de verificar (sin sha256sum)"
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
  if existe "$DAEMON"; then
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

  # Verificación DIFERIDA: si el hash no se pudo calcular antes (no hay
  # `sha256sum` y el daemon aún no estaba instalado), se calcula AHORA con
  # el daemon recién extraído, y se compara con el hash esperado.  Si no
  # cuadra, NO se instala nada.
  if [ "${SHA_PENDIENTE:-0}" = "1" ] && [ -n "${SHA_ESPERADO:-}" ]; then
    NUEVO_HASH=""
    for cand in "$STAGE/bin/megadaemon-core" "$STAGE/megadaemon-core" "$STAGE/megadaemon-movil/megadaemon-core"; do
      if ejecutable "$cand"; then
        NUEVO_HASH=$(r "'$cand' hash '$TAR' 2>/dev/null" | tr -d '\r\n' | awk '{print $1}')
        [ -n "$NUEVO_HASH" ] && break
      fi
    done
    if [ -z "$NUEVO_HASH" ]; then
      r "rm -rf '$STAGE'" >/dev/null 2>&1
      informe_paso "sha256" false "no se pudo calcular"
      informe_cierra
      die "no se pudo calcular el sha256 del bundle (ni sha256sum ni el daemon) => no instalo nada"
    fi
    if [ "$NUEVO_HASH" = "$SHA_ESPERADO" ]; then
      say "sha256: VERIFICADO por valor (con el daemon del bundle)"
      informe_paso "sha256" true "verificado por valor (con el daemon del bundle)"
    else
      r "rm -rf '$STAGE'" >/dev/null 2>&1
      informe_paso "sha256" false "NO cuadra"
      informe_cierra
      die "el sha256 del bundle NO cuadra => el fichero está manipulado o corrupto. NO instalo nada"
    fi
  fi

  # Instalar el binario (del stage o del propio bundle).
  NUEVO=""
  for cand in "$STAGE/bin/megadaemon-core" "$STAGE/megadaemon-core" "$STAGE/megadaemon-movil/megadaemon-core"; do
    ejecutable "$cand" && { NUEVO="$cand"; break; }
  done
  if [ -z "$NUEVO" ]; then
    NUEVO=$(r "find '$STAGE' -name 'megadaemon-core' -type f 2>/dev/null | head -1" | tr -d '\r\n')
  fi
  # La comprobacion va COMO ROOT: /data/adb es 0700 y el usuario normal no
  # puede ni mirar dentro.  MEDIDO (2026-11-27, movil): el bundle SI traia
  # bin/megadaemon-core (13.101.098 B, sha256 verificado) y el instalador
  # decia que no => no instalaba nada.
  if [ -z "$NUEVO" ] || ! ejecutable "$NUEVO"; then
    r "rm -rf '$STAGE'" >/dev/null 2>&1
    informe_paso "instalar_binario" false "no viene en el bundle"
    informe_cierra
    die "el bundle no trae el binario (esperado: bin/megadaemon-core)"
  fi
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
  instalar_stack "$STAGE"
  say "stack instalado"

  # El NUCLEO FORENSE (mddoctor, en C): se instala como root desde el bundle
  # y se comprueba que arranca.  MEDIDO 2026-11-27: el bundle se llevaba el
  # binario compilado en el HOST (x86-64) y el movil descargaba algo que no
  # puede ejecutar; ahora el CI mete el ARM64 y se verifica la arquitectura.
  for cand in "$STAGE/bin/mddoctor" "$STAGE/megadaemon-movil/bin/mddoctor" "$STAGE/mddoctor"; do
    if existe "$cand"; then
      r "cp -f '$cand' '$DOC.new' && chmod 755 '$DOC.new' && mv -f '$DOC.new' '$DOC'" >/dev/null
      break
    fi
  done
  if r "test -x '$DOC'" >/dev/null 2>&1; then
    say "nucleo forense instalado: $(r "wc -c < '$DOC'" | tr -d ' \r\n') B"
    # Se EJECUTA como root ahora mismo: el instalador demuestra que el
    # enlace "online -> root" funciona, no lo promete.
    say "--- comprobacion del nucleo forense (root) ---"
    r "'$DOC' entorno" 2>&1 | head -6
  else
    warn "el bundle no traia mddoctor utilizable (arquitectura o empaquetado)"
  fi

  say "=== 4/8 arrancando el stack ==="
  r "mkdir -p '$STATE' '$LOGDIR'" >/dev/null
  # ANTES de arrancar: si el stack está MULTIPLICADO (pastel de Android) se
  # deja uno de cada.  MEDIDO (2026-11-27, móvil): 6 supervisores + 6
  # cloudflared + 5 watchdogs => load 58 y el túnel sin poder arrancar.
  # MEDIDO (2026-11-28): PRIMERO el supervisor canonico se queda.  El
  # cleanup generico (`dejar_uno`) deja "el mas antiguo", y la copia
  # `.bak-50k` es MAS ANTIGUA que el canonico => si corriera despues, se
  # quedaria con el malo y mataria al bueno.
  matar_supervisores_no_canonicos
  limpiar_stack_duplicado
  # Y las COPIAS de `service.d` (p.ej. el `.bak-50k`) se BORRAN: son la
  # causa real del pastel (Magisk ejecuta todo lo ejecutable de ahi) y
  # matarlas no basta: el fichero las vuelve a cerrar en el arranque.
  limpiar_copias_service_d
  # El token de GitHub (canal al repo), plantado sin imprimirlo.
  plantar_token
  # El bus de órdenes por Drive: el único mando que funciona sin túnel.
  asegurar_bus
  # El daemon, CON SU ENTORNO (sin esto escribe el estado en el sitio
  # equivocado) y CON `< /dev/null` (si hereda el stdin, la sesión adb se
  # queda colgada).
  # MEDIDO (2026-11-27): `pkill -f` no es de fiar en toybox => los daemons
  # viejos se quedaban vivos y el móvil acababa con 6.  Ahora se matan
  # leyendo /proc (mismo criterio que `proc_pids`).
  matar_daemons
  sleep 1
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
  # ⚠️ MEDIDO (2026-11-27) por el banco del instalador: relanzar el watchdog
  # a ciegas deja DOS watchdogs vivos (este + el que arranca 98-cloudflared.sh
  # en el boot) y se pelean por el túnel: cada uno relanza cloudflared y el
  # hostname cambia cada 60 s => la flota se queda ciega.  Igual que con
  # cloudflared, si ya hay uno vivo, NO se relanza.
  contar_procesos
  if [ "${CONTAR_WD:-0}" -gt 0 ]; then
    say "hay $CONTAR_WD watchdogs vivos: no relanzo (se pelean por el túnel)"
  else
    r "LOG_DIR='$PREFIX' DAEMON_BIN='$DAEMON' PUBLISH='$PREFIX/channel-publish' MEGADAEMON_STATE_DIR='$STATE' MEGADAEMON_ENV='$PREFIX/megadaemon.env' CF='$CF_BIN' CFT_TARGET='http://127.0.0.1:$PORT' setsid nohup sh '$PREFIX/tunnel-watchdog.sh' >> '$LOGDIR/tunnel-watchdog.log' 2>&1 < /dev/null &" >/dev/null
  fi
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
    # MEDIDO (2026-11-27, movil): `pgrep -f` NO EXISTE en toybox (el shell de
    # Android) => la limpieza de daemons duplicados NO MATABA NADA y el movil
    # se quedaba con 6 daemons, 6 cloudflared y 5 watchdogs (load 58 => el
    # tunel no podia arrancar).  Ahora los PIDs se listan leyendo /proc
    # COMO ROOT, y se deja UN solo daemon de verdad (los envoltorios `sh -c`
    # no cuentan: si cuentan, se mata el daemon bueno y se queda el envoltorio).
    for p in $(r "grep -lsa 'megadaemon-core cockpit' /proc/[0-9]*/cmdline 2>/dev/null" | sed 's#/cmdline$##; s#.*/##'); do
      # Si la linea de comandos es un envoltorio `sh -c ...`, NO es el daemon:
      # matarlo no mata al daemon (y al revés: matar al hijo del daemon SI
      # lo rompe).  MEDIDO (2026-11-27, movil).
      CMDLINE=$(r "cat '/proc/$p/cmdline' 2>/dev/null | tr '\0' ' '")
      case "$CMDLINE" in
        *"sh -c"*) continue ;;
      esac
      n=$((n + 1))
      if [ "$n" -gt 1 ]; then
        matar_pid "$p"
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
  if ejecutable "$DAEMON"; then
    r "'$DAEMON' channel publish" 2>/dev/null | head -12 | sed 's/^/  /'
  fi
  # El REPO (Contents API: NO cachea en CDN) es el punto canónico de
  # descubrimiento: sin esto, cuando cambia el hostname del quick tunnel la
  # flota se queda ciega.  Lo publica `channel-publish --repo-publish`.
  if ejecutable "$PREFIX/channel-publish"; then
    publico=0
    # MEDIDO (2026-11-27, movil): el bucle buscaba el token en 4 rutas y
    # `token_github_ruta` en 5 => con el token en $HOME/.mavis/github.token el
    # diagnostico decia "hay token" pero el canal NO iba al repo.  Ahora se
    # usa la MISMA funcion que el diagnostico.
    T_TOKEN=$(token_github_ruta)
    if [ -n "$T_TOKEN" ]; then
      r "'$PREFIX/channel-publish' --repo-publish --state-dir '$STATE' --token-file '$T_TOKEN'" >/dev/null 2>&1 && publico=1
    fi
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
  # Si el movil no ha quedado alcanzable, se ARREGLA antes de rendirse.
  # ⚠️ MEDIDO (2026-11-27) por el banco del instalador: la llamada estaba
  # escrita "autoreparar" (una sola r) y la función se llama
  # "autorreparar" (dos r) => en el móvil salía
  #   "sh: 1226: autoreparar: not found"
  # y el móvil quedaba MUERTO sin que nadie lo arreglara.  El atajo de
  # idempotencia (que sí funciona) lo camufla: el 2º arranque dice "no toco
  # nada" y no repara nada.
  [ "$listo" = "1" ] || autorreparar
  # MEDIDO (2026-11-28): antes de la verificacion se reconcilia el stack:
  # `stack --reconciliar` borra las copias de service.d y reinicia el
  # supervisor si el canonico ha cambiado (idempotente).
  if ejecutable "$DAEMON"; then
    r "'$DAEMON' stack --reconciliar --json 2>/dev/null" | head -6 | sed 's/^/  /'
  fi
  matar_supervisores_no_canonicos
  limpiar_copias_service_d
  verificar_movil
  diagnostico
  if r "test -x '$DOC'" >/dev/null 2>&1; then
    say "--- veredicto del nucleo forense (root) ---"
    r "'$DOC' todo" 2>&1 | tail -n 14
  fi
  r "rm -rf '$STAGE' '$WORK'" >/dev/null 2>&1
  T1=$(date +%s 2>/dev/null || echo 0)
  say ""
  if [ "$listo" = "1" ]; then
    say "instalación terminada: el móvil es alcanzable en $URL"
    say "y su telemetría se lee desde Drive sin depender del túnel."
    INFORME_RESULTADO="ok"
    # MEDIDO (2026-11-28): el tunnel puede estar vivo y el movil ESTAR MAL
    # (4 supervisores, el 8765 con listener...).  El veredicto lo dice.
    if [ "${VERIFICACIONES_TOTAL:-0}" -gt 0 ] 2>/dev/null && [ "${VERIFICACIONES_OK:-0}" != "${VERIFICACIONES_TOTAL:-0}" ]; then
      warn "ATENCION: el tunel responde pero $(( ${VERIFICACIONES_TOTAL:-0} - ${VERIFICACIONES_OK:-0} )) verificacion(es) fallan (mira 'pasos' del informe)"
      INFORME_RESULTADO="ok_con_avisos"
    fi
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
  doctor|reconectar)
    modo_doctor
    ;;
  instalar)         instalar ;;
  pc)               instalar_pc ;;
  solo-telemetria)
    ejecutable "$DAEMON" || die "el daemon no está instalado"
    daemon telemetria publicar
    "$DAEMON" telemetria conexion
    ;;
  *) die "modo desconocido: $MODO" ;;
esac
