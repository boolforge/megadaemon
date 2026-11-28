#!/bin/sh
# ===========================================================================
# restaurar-movil.sh — deja el móvil FUNCIONAL y ACTUALIZADO, SIN REINICIAR
# ===========================================================================
# MEDIDO (2026-11-28, móvil Samsung + Magisk): el "pastel" de procesos (4
# supervisores, 4 watchdogs, 5 cloudflared, 6 daemons, load1=58) NO es un
# residuo: se RELANZA.  La causa está en `97-megadaemon-keeper.sh`:
# `[ -r "$_pp/cmdline" ] || continue` ⇒ si el cmdline no es legible, el
# proceso se cuenta como MUERTO ⇒ el keeper relanza al supervisor detrás del
# que ya está vivo ⇒ N supervisores ⇒ N cloudflared + N watchdogs + N
# daemons ⇒ load 58 ⇒ el túnel no arranca (HTTP 0).
#
# Este script NO reinicia el móvil.  Hace, en este orden:
#   1. INSTALAR/ACTUALIZAR (el one-liner: release, sha256, binario, stack,
#      núcleo forense).
#   2. PARAR todo el stack (primero los keepers: son los que relanzan).
#   3. LIMPIAR (copias .bak* en service.d, URLs de túnel obsoletas).
#   4. ARRANCAR uno de cada: supervisor canónico + 98-cloudflared (que
#      levanta cloudflared + watchdog) + un guardian.
#   5. VERIFICAR (recuento por rol, URL del túnel, build, /health).
#
# Uso (desde Termux):
#   curl -fsSL https://raw.githubusercontent.com/boolforge/megadaemon/main/tools/restaurar-movil.sh -o $HOME/restaurar.sh
#   su -mm -c "sh $HOME/restaurar.sh"
#
# Opciones:
#   --token=XXX   pasa el token de GitHub al one-liner (canal al repo).
#   --sin-guardian no arranca el guardian (si no quieres que reinicie el
#                 supervisor).
#   --timeout=N   segundos que espera el one-liner al túnel (por defecto 45).
# ===========================================================================
set -u

PREFIX=/data/adb/megadaemon
SVC=/data/adb/service.d
STATE="$PREFIX/state"
# MEDIDO (2026-11-28, banco): el `/proc` es parametrizable para poder probar
# la deteccion de roles en un laboratorio falso (MD_PROC).  En el movil es
# el de verdad.
PROC="${MD_PROC:-/proc}"
RAW=https://raw.githubusercontent.com/boolforge/megadaemon/main
CANON="$SVC/99-megadaemon.sh"
TOKEN=""
TIMEOUT=45
GUARDIAN=1

while [ $# -gt 0 ]; do
  case "$1" in
    --token=*) TOKEN="${1#--token=}" ;;
    --sin-guardian) GUARDIAN=0 ;;
    --timeout=*) TIMEOUT="${1#--timeout=}" ;;
    -h|--help) sed -n '2,40p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "opción desconocida: $1" >&2; exit 2 ;;
  esac
  shift
done

# --- modo biblioteca (para el banco) ----------------------------------------
# MEDIDO (2026-11-28): con MD_LIB_ONLY=1 este script sólo DEFINE sus
# funciones (el banco las prueba contra un /proc falso) y NO ejecuta las
# fases ni exige root.  En el móvil, sin esa variable, se comporta igual que
# siempre.  El guard está AL FINAL (justo antes de las fases) para que las
# funciones existan cuando se carga en modo biblioteca.
MD_LIB_ONLY="${MD_LIB_ONLY:-0}"

if [ "$MD_LIB_ONLY" != "1" ] && [ "$(id -u)" != "0" ]; then
  echo "este script necesita ROOT: su -mm -c \"sh $0\"" >&2
  exit 1
fi

say() { echo "[$(date '+%H:%M:%S')] $*"; }

# --- helpers: el argv de un proceso, ELEMENTO a elemento --------------------
# MEDIDO: el `case` sobre el texto UNIDO no vale: la copia `.bak-50k` tiene
# la ruta canónica como PREFIJO y pasaría por canónica.
argv_de() { cat "$PROC/$1/cmdline" 2>/dev/null | tr '\000' '\n'; }

# ¿algún elemento del argv de $1 TERMINA en $2? (p. ej. "cloudflared")
#
# MEDIDO (2026-11-28, banco): esto era `grep -q "$2\$"` => el patrón
# resultante era `99-megadaemon\.sh$$` (DOS anclas de fin) y GNU grep lo
# trata como un `$` LITERAL => NO casa con NADA y NO dice error: el
# restaurador era CIEGO (no encontraba ni el supervisor canónico ni la
# copia .bak) => paraba 0 procesos, no dejaba uno de cada y la verificación
# reportaba "vivos: 0" para todos los roles.  Con ERE y UN solo `$` el
# comportamiento es el correcto: el canónico casa y la copia .bak no.
argv_acaba_en() {
  argv_de "$1" | grep -qE "$2"
}

pids_del_rol() {
  # $1 = expresión regular ERE (los llamantes pasan el sufijo con `$`).
  # Imprime los PID (uno por línea).
  for _p in "$PROC"/[0-9]*; do
    _pid=${_p##*/}
    argv_acaba_en "$_pid" "$1" && printf '%s\n' "$_pid"
  done
  return 0
}

matar_rol() {
  # $1 = sufijo del argv, $2 = etiqueta
  _n=0
  for _pid in $(pids_del_rol "$1"); do
    kill -9 "$_pid" 2>/dev/null && { echo "  parado $_pid ($2)"; _n=$((_n + 1)); }
  done
  echo "  ($2 parados: $_n)"
  return 0
}

dejar_uno_rol() {
  # $1 = sufijo del argv, $2 = etiqueta.  Se deja el MAS ANTIGUO (el de menor
  # PID) y se matan los demas.
  _pids=$(pids_del_rol "$1")
  [ -z "$_pids" ] && return 0
  _n=0
  for _pid in $_pids; do
    _n=$((_n + 1))
    if [ "$_n" -gt 1 ]; then
      kill -9 "$_pid" 2>/dev/null && echo "  duplicado $_pid ($2) eliminado (se deja el mas antiguo)"
    fi
  done
  return 0
}

dejar_un_supervisor() {
  # El que se queda es el CANONICO (argv = ruta canonica, elemento a
  # elemento): la copia .bak-50k tiene la ruta canonica como PREFIJO.
  for _p in "$PROC"/[0-9]*; do
    _pid=${_p##*/}
    argv_de "$_pid" | grep -q '99-megadaemon\.sh$' || continue
    argv_de "$_pid" | grep -qx "$CANON" && continue
    kill -9 "$_pid" 2>/dev/null && echo "  supervisor NO canonico $_pid eliminado"
  done
  return 0
}

# --- descargar: el `curl` de Termux NO esta en el PATH de root -------------
# MEDIDO (2026-11-28, movil): con `su -mm -c` el PATH es el de root
# (/system/bin) => "curl: inaccessible or not found" y el paso de instalar
# se saltaba (el movil se quedaba con el binario viejo, sin avisar).
# Aqui se busca un descargador de verdad: el del PATH, el de Termux, o wget.
BUSCADOR=""
for _c in curl wget; do
  if command -v "$_c" >/dev/null 2>&1; then BUSCADOR="$_c"; break; fi
done
if [ -z "$BUSCADOR" ]; then
  for _c in /data/data/com.termux/files/usr/bin/curl /data/data/com.termux/files/usr/bin/wget \
            /system/bin/curl /system/bin/wget /data/data/com.termux/files/usr/bin/curl; do
    if [ -x "$_c" ]; then BUSCADOR="$_c"; break; fi
  done
fi
if [ -z "$BUSCADOR" ]; then
  echo "AVISO: no hay curl ni wget: NO se puede instalar/actualizar" >&2
  echo "        (el resto del restaurador sigue: para, limpia, arranca, verifica)" >&2
fi

descargar() {
  # $1 = url.  Escribe el contenido por stdout.
  case "$BUSCADOR" in
    *curl) "$BUSCADOR" -fsSL --retry 3 "$1" ;;
    *wget) "$BUSCADOR" -qO- "$1" ;;
    *) return 1 ;;
  esac
}

# --- 1/5 INSTALAR Y ACTUALIZAR ---------------------------------------------
fase_instalar() {
  say "=== 1/5 instalando/actualizando (one-liner: release + sha256 + binario + stack) ==="
  if [ -n "$TOKEN" ]; then
    descargar "$RAW/deploy/megadaemon-oneliner.sh" \
      | sh -s -- --instalar --timeout="$TIMEOUT" --token="$TOKEN"
  else
    descargar "$RAW/deploy/megadaemon-oneliner.sh" \
      | sh -s -- --instalar --timeout="$TIMEOUT"
  fi
  say "  (el one-liner deja 1 daemon y 1 cloudflared; los paramos en el paso 2)"
}

# --- 2/5 PARAR TODO -------------------------------------------------------
# El ORDEN es el inverso al de arranque: primero el keeper (es el que
# relanza al supervisor), luego watchdog, supervisor, cloudflared y daemon.
fase_parar() {
  say "=== 2/5 parando TODO el stack (para que nadie relance) ==="
  matar_rol '97-megadaemon-keeper\.sh$' keeper
  matar_rol 'tunnel-watchdog\.sh$' watchdog
  matar_rol '99-megadaemon\.sh$' supervisor
  matar_rol 'cloudflared$' cloudflared
  matar_rol 'megadaemon-core$' daemon
  sleep 3
}

# --- 3/5 LIMPIAR ----------------------------------------------------------
fase_limpiar() {
  say "=== 3/5 limpiando restos (copias .bak* en service.d y URLs viejas) ==="
  for _c in 99-megadaemon.sh 97-megadaemon-keeper.sh 98-cloudflared.sh 91-adb-tcp.sh; do
    for _f in $(ls -1 "$SVC" 2>/dev/null); do
      [ "$_f" = "$_c" ] && continue
      case "$_f" in
        "${_c%.sh}"*)
          rm -f "$SVC/$_f" 2>/dev/null && echo "  borrada la copia $_f" ;;
      esac
    done
  done
  # Las URLs las reescribe el supervisor al arrancar (con su extraccion
  # correcta, que NO casa api.trycloudflare.com).
  rm -f "$PREFIX/tunnel.url" "$PREFIX/mcp_tunnel.url" "$STATE/tunnel.url" 2>/dev/null
  echo "  URLs de tunel reseteadas (el supervisor las vuelve a publicar)"
}

# --- 4/5 ARRANCAR UNO DE CADA ---------------------------------------------
fase_arrancar() {
  say "=== 4/5 arrancando el stack CANONICO (uno de cada) ==="
  # El supervisor: el que sabe levantar el daemon y su cloudflared.
  if [ -x "$CANON" ]; then
    MEGADAEMON_PREFIX="$PREFIX" MEGADAEMON_STATE_DIR="$STATE" \
      setsid nohup sh "$CANON" >/dev/null 2>&1 </dev/null &
    say "  supervisor arrancado"
  else
    say "  NO existe $CANON"
  fi
  sleep 15
  # 98-cloudflared.sh: levanta el cloudflared Y el watchdog (hermanos del
  # supervisor, NO hijos: por eso hay que lanzarlos aqui).
  if [ -x "$SVC/98-cloudflared.sh" ]; then
    MEGADAEMON_PREFIX="$PREFIX" MEGADAEMON_STATE_DIR="$STATE" \
      setsid nohup sh "$SVC/98-cloudflared.sh" >/dev/null 2>&1 </dev/null &
    say "  cloudflared + watchdog arrancados (98-cloudflared.sh)"
  fi
  sleep 15
  # El guardian: el keeper de Magisk solo corre en el BOOT, asi que sin el
  # nadie relanzaria el supervisor.  Si el keeper ya esta ARREGLADO (usa el
  # fichero de PID) lo usamos; si no, ponemos uno provisional que comprueba
  # el argv ELEMENTO a elemento (esto si se puede leer: corre como root).
  if [ "$GUARDIAN" = "1" ]; then
    if grep -q 'supervisor\.pid' "$SVC/97-megadaemon-keeper.sh" 2>/dev/null; then
      MEGADAEMON_PREFIX="$PREFIX" MEGADAEMON_STATE_DIR="$STATE" \
        setsid nohup sh "$SVC/97-megadaemon-keeper.sh" >/dev/null 2>&1 </dev/null &
      say "  keeper arrancado (el del repo, ya usa PID files)"
    else
      say "  AVISO: el keeper del repo sigue con el check fragil del cmdline:"
      say "        arrancamos un guardian PROVISIONAL (se queda 1 supervisor)"
      rm -f "$PREFIX/guardian.pid"
      (
        echo $$ > "$PREFIX/guardian.pid"
        while true; do
          sleep 60
          _vivo=0
          for _p in "$PROC"/[0-9]*; do
            _pid=${_p##*/}
            argv_de "$_pid" | grep -qx "$CANON" && { _vivo=1; break; }
          done
          if [ "$_vivo" = "0" ] && [ -x "$CANON" ]; then
            echo "[$(date '+%H:%M:%S')] guardian: el supervisor no vive: lo relanzo" \
              >> "$PREFIX/guardian.log"
            MEGADAEMON_PREFIX="$PREFIX" MEGADAEMON_STATE_DIR="$STATE" \
              setsid nohup sh "$CANON" >/dev/null 2>&1 </dev/null &
          fi
        done
      ) >/dev/null 2>&1 </dev/null &
      say "  guardian provisional arrancado"
    fi
  fi
  sleep 10
  # --- pasada final: UNO de cada rol ---------------------------------------
  # El cloudflared que se deja es el MAS ANTIGUO: es el del supervisor, y el
  # supervisor es quien publica la URL correcta en tunnel.url (su extraccion
  # NO casa api.trycloudflare.com).
  say "  --- dejando uno de cada ---"
  dejar_un_supervisor
  dejar_uno_rol '97-megadaemon-keeper\.sh$' keeper
  dejar_uno_rol 'tunnel-watchdog\.sh$' watchdog
  dejar_uno_rol 'cloudflared$' cloudflared
  dejar_uno_rol 'megadaemon-core$' daemon
  sleep 5
}

# --- 5/5 VERIFICAR --------------------------------------------------------
fase_verificar() {
  say "=== 5/5 verificando ==="
  for _rol in '99-megadaemon\.sh$' '97-megadaemon-keeper\.sh$' 'tunnel-watchdog\.sh$' 'cloudflared$' 'megadaemon-core$'; do
    _n=$(pids_del_rol "$_rol" | wc -l | tr -d ' ')
    echo "  vivos ($_rol): $_n"
  done
  echo "  --- service.d ---"
  ls -1 "$SVC" 2>/dev/null | sed 's/^/    /'
  echo "  --- URL del tunel ---"
  _u=$(cat "$PREFIX/tunnel.url" 2>/dev/null)
  case "$_u" in
    https://api.*|https://developers.*|"") echo "    (vacia o INVALIDA: $_u)" ;;
    *) echo "    $_u" ;;
  esac
  echo "  --- daemon ---"
  _d="$PREFIX/megadaemon-core"
  if [ -x "$_d" ]; then
    echo "    binario: $(wc -c < "$_d" | tr -d ' ') B"
    MEGADAEMON_PREFIX="$PREFIX" MEGADAEMON_STATE_DIR="$STATE" \
      "$_d" diagnostico --json 2>/dev/null | head -c 400
    echo
  else
    echo "    NO hay binario en $_d"
  fi
  echo "  --- carga ---"
  cat "$PROC/loadavg" 2>/dev/null || cat /proc/loadavg
}

# --- modo biblioteca: aquí se corta (el resto son las fases) ---------------
if [ "$MD_LIB_ONLY" = "1" ]; then
  return 0 2>/dev/null || exit 0
fi

fase_instalar
fase_parar
fase_limpiar
fase_arrancar
fase_verificar
say "=== restaurado (sin reiniciar). El guardian mantendra UN supervisor vivo. ==="
exit 0
