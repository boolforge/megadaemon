#!/system/bin/sh
# ===========================================================================
# verificar-movil-dentro.sh — UNA pasada de comprobaciones DENTRO del móvil
# ===========================================================================
# QUÉ HACE
# --------
# Comprueba, de una sentada, lo que el jefe tiene que mirar a mano después
# de instalar: que no queden duplicados, que los puertos sean los que deben,
# que la URL del túnel sea la buena, que la base tenga el esquema medido y
# que el daemon que corre sea el nuevo.
#
# NO mata nada, NO reinicia nada, NO descarga nada, NO escribe nada: es
# sólo lectura.  Para ARRANCAR/COMPARAR lo que hace falta es el restaurador
# (`tools/restaurar-movil.sh`).
#
# LAS DOS VERIFICACIONES (no se confunden)
# ----------------------------------------
#   * `tools/verificar-movil.sh`       -> se ejecuta DESDE EL PC, contra la
#                                         URL del túnel (curl + python3).
#   * `tools/verificar-movil-dentro.sh`-> se ejecuta DENTRO del móvil, como
#                                         root (este fichero).
#
# POR QUÉ EXISTE
# --------------
# La lista de comprobaciones estaba repartida en 4 sitios del doc de
# verificación (los 7 del instalador, los 5 de §3.5, los 5 de Antigravity y
# los 3 de §7) y el jefe tenía que ejecutarlas una a una en el shell de
# Android, donde no hay `curl`, ni `wget`, ni `pgrep`, ni `sqlite3`.
# Aquí está todo en un comando y sale un veredicto con contadores.
#
# Uso (desde Termux):
#   curl -fsSL https://raw.githubusercontent.com/boolforge/megadaemon/main/tools/verificar-movil-dentro.sh -o $HOME/verificar.sh
#   su -mm -c "sh $HOME/verificar.sh"
#
# MEDIDO (2026-11-28): el shell de Android no trae `curl`/`wget`/`pgrep`/
# `sha256sum`; aquí no se usa ninguno de los cuatro.  Los puertos se leen de
# `/proc/net/tcp` con `awk` y los procesos de `/proc/<pid>/cmdline` elemento a
# elemento (la ruta canónica como PREFIJO no basta: la copia `.bak-50k` la
# tiene y por eso se compara el elemento ENTERO, no una subcadena).
# ===========================================================================
set -u

PREFIX=/data/adb/megadaemon
SVC=/data/adb/service.d
STATE="$PREFIX/state"
LOG=/sdcard/megadaemon.log
CANON="$SVC/99-megadaemon.sh"
PROC=/proc

OK=0
FALLO=0
SINDATOS=0
TOTAL=0

if [ "$(id -u)" != "0" ]; then
  echo "necesita ROOT: su -mm -c \"sh $0\"" >&2
  exit 1
fi

res() {
  # $1 = PASA|FALLA|SINDATOS ; $2 = texto
  TOTAL=$((TOTAL + 1))
  case "$1" in
    PASA)    OK=$((OK + 1));       printf '  [PASA]      %s\n' "$2" ;;
    FALLA)   FALLO=$((FALLO + 1));  printf '  [FALLA]     %s\n' "$2" ;;
    *)  SINDATOS=$((SINDATOS + 1)); printf '  [SIN DATOS] %s\n' "$2" ;;
  esac
}

# --- una sola pasada por /proc ----------------------------------------------
# MEDIDO (2026-11-28): la primera versión counts un rol por pasada y con
# `cat|tr|grep` por proceso => 3 forks x N procesos x 5 roles.  En el móvil
# (cientos de procesos) eso son miles de forks: lento.  Ahora se hace UNA
# pasada y las comparaciones son del shell (sin fork).
C_SUP=0; C_KEE=0; C_WAT=0; C_CFL=0; C_DAE=0
P_SUP=""; P_CANON=""

argv_de() { tr '\000' '\n' < "$PROC/$1/cmdline" 2>/dev/null; }

contar_roles() {
  for _p in "$PROC"/[0-9]*; do
    _pid=${_p##*/}
    [ -e "$_p/cmdline" ] || continue
    _argv=$(tr '\000' '\n' < "$_p/cmdline" 2>/dev/null)
    [ -n "$_argv" ] || continue
    # Con IFS=salto de línea, `for` parte el argv por ELEMENTOS y no por
    # espacios: un elemento con espacios no se rompe en varios.
    _ifs=$IFS
    IFS='
'
    for _e in $_argv; do
      case "${_e##*/}" in
        99-megadaemon.sh)        C_SUP=$((C_SUP + 1)); [ -z "$P_SUP" ] && P_SUP=$_pid ;;
        97-megadaemon-keeper.sh) C_KEE=$((C_KEE + 1)) ;;
        tunnel-watchdog.sh)      C_WAT=$((C_WAT + 1)) ;;
        cloudflared)             C_CFL=$((C_CFL + 1)) ;;
        megadaemon-core)         C_DAE=$((C_DAE + 1)) ;;
      esac
      # El supervisor CANÓNICO es el que tiene este elemento ENTERO (la copia
      # .bak-50k tiene la ruta canónica como PREFIJO, no como elemento).
      [ "$_e" = "$CANON" ] && P_CANON=$_pid
    done
    IFS=$_ifs
  done
}

# ¿El puerto $1 (decimal) tiene un listener?  Se lee /proc/net/tcp{,6}.
puerto_escucha() {
  _hex=$(printf '%04X' "$1")
  for _f in "$PROC/net/tcp" "$PROC/net/tcp6"; do
    [ -r "$_f" ] || continue
    awk -v p="$_hex" '$4 == "0A" { split($2, a, ":"); if (a[2] == p) { found = 1 } }
                         END { exit(found ? 0 : 1) }' "$_f" && return 0
  done
  return 1
}

contar_roles

# --- 1) service.d: sólo los canónicos --------------------------------------
_sobras=""
for _f in $(ls -1 "$SVC" 2>/dev/null); do
  case "$_f" in
    99-megadaemon.sh|97-megadaemon-keeper.sh|98-cloudflared.sh|91-adb-tcp.sh) ;;
    *) case "$_f" in
         *megadaemon*|*cloudflared*) _sobras="$_sobras $_f" ;;
       esac ;;
  esac
done
if [ ! -d "$SVC" ]; then
  res SINDATOS "no existe $SVC: no se puede mirar service.d"
elif [ -z "$_sobras" ]; then
  res PASA "service.d: sólo los canónicos"
else
  res FALLA "service.d: sobran ficheros:$_sobras"
fi

# --- 2..6) un proceso de cada rol ------------------------------------------
if [ "$C_SUP" = "1" ]; then
  res PASA "supervisor: 1 proceso"
elif [ "$C_SUP" = "0" ]; then
  res FALLA "supervisor: NINGUN proceso"
else
  res FALLA "supervisor: $C_SUP procesos (duplicados)"
fi
if [ "$C_KEE" = "1" ]; then
  res PASA "keeper: 1 proceso"
elif [ "$C_KEE" = "0" ]; then
  res FALLA "keeper: NINGUN proceso"
else
  res FALLA "keeper: $C_KEE procesos (duplicados)"
fi
if [ "$C_WAT" = "1" ]; then
  res PASA "watchdog: 1 proceso"
elif [ "$C_WAT" = "0" ]; then
  res FALLA "watchdog: NINGUN proceso"
else
  res FALLA "watchdog: $C_WAT procesos (duplicados)"
fi
if [ "$C_CFL" = "1" ]; then
  res PASA "cloudflared: 1 proceso"
elif [ "$C_CFL" = "0" ]; then
  res FALLA "cloudflared: NINGUN proceso"
else
  res FALLA "cloudflared: $C_CFL procesos (duplicados)"
fi
if [ "$C_DAE" = "1" ]; then
  res PASA "daemon: 1 proceso"
elif [ "$C_DAE" = "0" ]; then
  res FALLA "daemon: NINGUN proceso"
else
  res FALLA "daemon: $C_DAE procesos (duplicados)"
fi

# --- 6b) el supervisor vivo es el CANÓNICO (elemento a elemento) -----------
if [ -z "$P_SUP" ]; then
  res SINDATOS "no hay supervisor vivo: no se puede decir cuál es el canónico"
elif [ -n "$P_CANON" ] && [ "$P_CANON" = "$P_SUP" ]; then
  res PASA "el supervisor vivo es el canónico ($CANON)"
else
  res FALLA "el supervisor vivo NO es el canónico: $(argv_de "$P_SUP" | tr '\n' ' ')"
fi

# --- 7..9) puertos ----------------------------------------------------------
if puerto_escucha 8765; then
  res FALLA "8765 (túnel legacy) tiene listener: sobra"
else
  res PASA "8765 (túnel legacy) sin listener"
fi
if puerto_escucha 8080; then
  res PASA "8080 con listener (el que usa la app)"
else
  res FALLA "8080 SIN listener: la app dará ERR_LLM_UNREACHABLE"
fi
if puerto_escucha 9090; then
  res PASA "9090 con listener (el cockpit)"
else
  res FALLA "9090 SIN listener: la Web UI no carga"
fi

# --- 10) la URL del túnel --------------------------------------------------
_url=$(cat "$PREFIX/tunnel.url" 2>/dev/null | tr -d '\r\n')
case "$_url" in
  "") res FALLA "no hay tunnel.url: el túnel no está publicado" ;;
  https://api.*|https://developers.*)
    res FALLA "tunnel.url INVÁLIDA (es la API, no el túnel): $_url" ;;
  https://*.trycloudflare.com)
    res PASA "tunnel.url válida: $_url" ;;
  *) res FALLA "tunnel.url rara: $_url" ;;
esac

# --- 11) el log no anuncia el túnel legacy --------------------------------
if [ -r "$LOG" ]; then
  _legacy=$(grep -c 'mcp-legacy caido' "$LOG" 2>/dev/null | tr -d ' ')
  if [ "${_legacy:-0}" = "0" ]; then
    res PASA "el log no anuncia el túnel legacy (0 apariciones)"
  else
    res FALLA "el log anuncia el túnel legacy $_legacy vez/veces"
  fi
else
  res SINDATOS "no se puede leer $LOG"
fi

# --- 12/13) la base de la instancia ----------------------------------------
_db="$STATE/board.sqlite"
if [ -s "$_db" ]; then
  res PASA "board.sqlite existe ($(wc -c < "$_db" | tr -d ' ') B)"
  _faltan=""
  for _t in sessions messages message_nodes subagent_heads tool_call_state \
           prompt_history rendered_commits live_row_times session_window_tools \
           app_state; do
    if ! grep -qa "$_t" "$_db" 2>/dev/null && ! grep -q "$_t" "$_db" 2>/dev/null; then
      _faltan="$_faltan $_t"
    fi
  done
  if [ -z "$_faltan" ]; then
    res PASA "board.sqlite tiene las 10 tablas medidas"
  else
    res FALLA "board.sqlite sin tablas:$_faltan"
  fi
else
  res FALLA "no existe $_db (o está vacío): el binario es viejo"
fi

# --- 14) el binario en servicio --------------------------------------------
if [ -x "$PREFIX/megadaemon-core" ]; then
  _bytes=$(wc -c < "$PREFIX/megadaemon-core" | tr -d ' ')
  if [ "$_bytes" -gt 1000000 ]; then
    res PASA "binario en servicio: $_bytes B"
  else
    res FALLA "el binario en servicio son sólo $_bytes B: parece viejo"
  fi
else
  res FALLA "no hay binario ejecutable en $PREFIX/megadaemon-core"
fi

# --- 15/16) lo que el daemon dice de sí mismo ------------------------------
_djson=$(MEGADAEMON_PREFIX="$PREFIX" MEGADAEMON_STATE_DIR="$STATE" \
         "$PREFIX/megadaemon-core" diagnostico --json 2>/dev/null | tr -d '\r\n')
case "$_djson" in
  \{*\}) res PASA "el daemon responde al diagnóstico" ;;
  *) res FALLA "el daemon NO responde al diagnóstico (¿binario viejo?)" ;;
esac
_desf=$(printf '%s' "$_djson" | grep -o '"desfasados":[0-9]*' | head -1 | cut -d: -f2)
if [ -z "$_desf" ]; then
  res SINDATOS "el daemon no informa de builds desfasados"
elif [ "$_desf" = "0" ]; then
  res PASA "0 builds desfasados: el binario en servicio es el nuevo"
else
  res FALLA "$_desf build(s) desfasado(s): el binario en servicio NO es el nuevo"
fi

# --- 17) /api/health (si hay un descargador) -------------------------------
_desc=""
for _c in /data/data/com.termux/files/usr/bin/wget /system/bin/wget \
          /data/data/com.termux/files/usr/bin/curl /system/bin/curl; do
  [ -x "$_c" ] && { _desc="$_c"; break; }
done
if [ -z "$_desc" ]; then
  res SINDATOS "no hay wget/curl: no se puede llamar a /api/health"
else
  case "$_desc" in
    *curl) _code=$("$_desc" -s --max-time 10 -o /dev/null -w '%{http_code}' \
                     http://127.0.0.1:9090/api/health 2>/dev/null) ;;
    *)     _code=$("$_desc" -qO- -T 10 http://127.0.0.1:9090/api/health \
                     >/dev/null 2>&1 && echo 200 || echo 000) ;;
  esac
  if [ "$_code" = "200" ]; then
    res PASA "/api/health responde 200"
  else
    res FALLA "/api/health respondió '$_code'"
  fi
fi

# --- 18) carga y memoria ----------------------------------------------------
_load=$(cut -d' ' -f1 /proc/loadavg 2>/dev/null)
case "$_load" in
  ''|*[!0-9.]*) res SINDATOS "no se puede leer la carga" ;;
  *) if [ "${_load%%.*}" -lt 5 ]; then
       res PASA "carga (load1) = $_load"
     else
       res FALLA "carga (load1) = $_load: el móvil va cargado (el stack se multiplica)"
     fi ;;
esac
_mem=$(grep '^MemAvailable' /proc/meminfo 2>/dev/null | tr -dc '0-9')
if [ -n "$_mem" ] && [ "$_mem" -gt 100000 ]; then
  res PASA "memoria disponible: $((_mem / 1024)) MB"
elif [ -n "$_mem" ]; then
  res FALLA "memoria disponible: sólo $((_mem / 1024)) MB"
else
  res SINDATOS "no se puede leer la memoria"
fi

# --- veredicto --------------------------------------------------------------
echo ""
echo "=== VEREDICTO: $OK PASA / $FALLO FALLA / $SINDATOS SIN DATOS (de $TOTAL) ==="
if [ "$FALLO" = "0" ]; then
  echo "el móvil está BIEN"
else
  echo "el móvil NO está bien: mira los [FALLA] de arriba"
  echo "(para arreglarlo SIN reiniciar: tools/restaurar-movil.sh)"
fi
exit 0
