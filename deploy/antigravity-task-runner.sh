#!/system/bin/sh
# ---------------------------------------------------------------------------
# RUNNER DE UN CICLO DE LA TAREA RECURRENTE DE ANTIGRAVITY  (S21 Ultra)
#
# Por qué este fichero existe (medido el 2026-11-22 en el móvil, por root):
#
#   * La tarea vive DENTRO del proceso de la app Antigravity (`com.agy`).
#     Cuando ese proceso muere, el backlog se queda colgado para siempre:
#     se comprobó que `tasks.json` seguía con las 2 tareas `enabled=true`
#     tres días después de que el micro-daemon dejar de escribir (2026-11-19
#     10:21:33 -> 2026-11-22 01:00), y que el PID guardado en `daemon.pid`
#     (6905) ya era otro proceso (`io.github.mekhontsev.magicdesk`).
#   * El comando guardado en `tasks.json` es
#     `python3 .../snatcher_megadaemon.py --once`, pero **`python3` NO está
#     en el PATH del sistema**: medido `python3: inaccessible or not found`
#     (rc=127). Sólo existe en
#     `/data/user/0/com.agy/files/runtime/bin/python3` (wrapper a Python
#     3.14.10). Este runner pone ese directorio en el PATH => el comando es
#     reproducible desde un shell root, no sólo desde dentro de la app.
#   * El `interval` de la tarea (300 s) es **menor** que la duración real del
#     ciclo (211 s el último; hasta 570 s en el historial de `megadaemon_state.json`)
#     => dos workers pueden solaparse. Aquí hay un lock atómico (`mkdir`)
#     que lo impide.
#   * El daemon recorta la salida a ~1 KB (`last_output` = 1039 B) => el
#     enjambre no puede leer el resultado del ciclo. Aquí la salida completa
#     se queda en un log CON FECHA y el estado en un JSON legible.
#   * Sin timeout, un ciclo colgado se queda colgado para siempre (y con el
#     daemon muerto ya no hay quien lo mate). Aquí `timeout` lo acota.
#
# Garantías de este runner (medibles desde fuera):
#   1. Nunca se solapan dos ciclos            (lock atómico con `mkdir`).
#   2. Nunca se queda colgado                 (timeout con valor por defecto).
#   3. Cada ciclo deja un log con fecha       (logs/cycle-AAAAMMDD-HHMMSS.log).
#   4. Cada ciclo deja un estado legible      (last_status.json).
#   5. Si no puede arrancar, lo dice          (rc != 0 + motivo en el estado).
#
# Uso:
#   antigravity-task-runner.sh [--once] [--status] [--selftest] [--help]
#
# Variables de entorno (todas con valor por defecto):
#   ANTIGRAVITY_STATE_DIR     raíz de estado   (default /data/local/tmp/megadaemon/antigravity)
#   ANTIGRAVITY_TASK_CMD      comando a ejecutar (default: el de tasks.json)
#   ANTIGRAVITY_MAX_RUNTIME   segundos máximos del ciclo (default 900)
#   ANTIGRAVITY_KEEP_LOGS     cuántos logs se conservan (default 50)
# ---------------------------------------------------------------------------
set -u

AGY_ROOT="${AGY_ROOT:-/data/user/0/com.agy/files}"
RUNTIME_BIN="$AGY_ROOT/runtime/bin"
SNATCHER="$AGY_ROOT/scummvm/tools/re_pipeline/snatcher_megadaemon.py"

STATE_DIR="${ANTIGRAVITY_STATE_DIR:-/data/local/tmp/megadaemon/antigravity}"
LOG_DIR="$STATE_DIR/logs"
STATUS_FILE="$STATE_DIR/last_status.json"
# Lock atómico con `mkdir` (no `flock`): MEDIDO el 2026-11-22 en este móvil,
# `flock` es el de toybox 0.8.9-android y su sintaxis es `flock [-sxun] fd`
# (sólo descriptor) => no admite ni la forma `flock -n 9` sobre un fd abierto
# por el shell (`Bad file descriptor`) ni `flock -n fichero -c cmd`
# (`Unknown option 'c'`). `mkdir` es atómico en cualquier POSIX ⇒ portable.
LOCK_DIR="$STATE_DIR/runner.lock.d"

MAX_RUNTIME="${ANTIGRAVITY_MAX_RUNTIME:-900}"
KEEP_LOGS="${ANTIGRAVITY_KEEP_LOGS:-50}"

DEFAULT_CMD="python3 $SNATCHER --once"
TASK_CMD="${ANTIGRAVITY_TASK_CMD:-$DEFAULT_CMD}"

# `python3` y `rish` viven en el runtime de la app, no en el PATH del sistema.
PATH="$RUNTIME_BIN:$PATH"
export PATH

RC_OK=0
RC_SKIP=75        # otro ciclo en marcha (lock)
RC_PREFLIGHT=78   # falta algo para arrancar (preflight)
RC_TIMEOUT=124    # el ciclo excede MAX_RUNTIME
RC_FAILED=1       # el ciclo terminó con error

mkdir -p "$LOG_DIR" 2>/dev/null || exit "$RC_PREFLIGHT"

now_iso()  { date '+%Y-%m-%dT%H:%M:%S%z'; }
stamp()    { date '+%Y%m%d-%H%M%S'; }
say()      { echo "[$(now_iso)] [antigravity-runner] $*"; }

usage() {
  sed -n '2,60p' "$0" | sed 's/^# \{0,1\}//'
}

# --- estado ---------------------------------------------------------------
# Estado legible por el enjambre. Se escribe atómicamente (tmp + mv).
write_status() {
  # $1 rc, $2 duración (s), $3 log, $4 ciclo, $5 motivo
  _rc="$1"; _dur="$2"; _log="$3"; _cyc="$4"; _why="$5"
  _tmp="$STATUS_FILE.tmp.$$"
  {
    echo "{"
    echo "  \"ts\": \"$(now_iso)\","
    echo "  \"ok\": $([ "$_rc" = "$RC_OK" ] && echo true || echo false),"
    echo "  \"rc\": $_rc,"
    echo "  \"duration_s\": $_dur,"
    echo "  \"cycle\": \"$_cyc\","
    echo "  \"reason\": \"$_why\","
    echo "  \"log\": \"$_log\","
    echo "  \"max_runtime_s\": $MAX_RUNTIME,"
    echo "  \"task_cmd_digest\": \"$(cmd_digest "$TASK_CMD")\","
    echo "  \"task_cmd_len\": ${#TASK_CMD}"
    echo "}"
  } > "$_tmp" 2>/dev/null && mv -f "$_tmp" "$STATUS_FILE"
}

detect_cycle() {
  # Último "STARTING RECURSIVE CYCLE #N" del log (si el ciclo lo imprime).
  grep -o 'STARTING RECURSIVE CYCLE #[0-9]*' "$1" 2>/dev/null | tail -1 | tr -d '\r'
}

cmd_digest() {
  # Identificador corto y reproducible del comando (NO el valor de secretos).
  printf '%s' "$1" | cksum 2>/dev/null | awk '{print $1"-"$2}'
}

selftest() {
  # Autocomprobación sin ejecutar el ciclo real: valida preflight, lock,
  # log con fecha y estado JSON. Es lo que se ejecuta en el móvil para
  # probar el arranque sin lanzar un ciclo de 200+ segundos.
  say "selftest: state_dir=$STATE_DIR"
  command -v timeout >/dev/null 2>&1 || { say "selftest: FALTA timeout"; return "$RC_PREFLIGHT"; }
  command -v python3 >/dev/null 2>&1 || { say "selftest: FALTA python3 en el PATH"; return "$RC_PREFLIGHT"; }
  say "selftest: python3 -> $(python3 -V 2>&1)"
  [ -f "$SNATCHER" ] && say "selftest: script presente ($SNATCHER)" \
                    || say "selftest: AVISO, no existe $SNATCHER"
  # Lock: se toma, un segundo intento se salta, y al soltar se puede repetir.
  if acquire_lock; then
    if acquire_lock; then
      say "selftest: FALLO, el lock se puede tomar dos veces"
      release_lock
      return "$RC_PREFLIGHT"
    fi
    say "selftest: lock ok (el segundo intento se salta)"
    release_lock
  else
    say "selftest: FALLO, no se pudo tomar el lock"
    return "$RC_PREFLIGHT"
  fi
  _t0=$(date +%s)
  _log="$LOG_DIR/selftest-$(stamp).log"
  { echo "=== selftest $(now_iso) ==="; echo "cmd: $TASK_CMD"; echo "digest: $(cmd_digest "$TASK_CMD")"; } > "$_log"
  say "selftest: log escrito -> $_log"
  write_status 0 $(( $(date +%s) - _t0 )) "$_log" "selftest" "selftest-ok"
  say "selftest: estado escrito -> $STATUS_FILE"
  return 0
}

show_status() {
  [ -f "$STATUS_FILE" ] || { say "sin estado en $STATUS_FILE"; return 1; }
  cat "$STATUS_FILE"
}

# --- lock -----------------------------------------------------------------
LOCKED=0

release_lock() {
  [ "$LOCKED" = "1" ] && rm -rf "$LOCK_DIR" 2>/dev/null
  LOCKED=0
}

acquire_lock() {
  # 1) intento normal
  if mkdir "$LOCK_DIR" 2>/dev/null; then
    echo "$$" > "$LOCK_DIR/pid" 2>/dev/null
    LOCKED=1
    return 0
  fi
  # 2) ¿lock huérfano? (el proceso murió sin soltar) -> se rompe si es viejo.
  _st="$(stat -c %Y "$LOCK_DIR" 2>/dev/null)"
  _now="$(date +%s)"
  if [ -n "$_st" ] && [ $(( _now - _st )) -gt $(( MAX_RUNTIME * 2 + 60 )) ]; then
    say "lock huérfano (antigüedad $(( _now - _st ))s) -> lo rompo"
    rm -rf "$LOCK_DIR" 2>/dev/null
    if mkdir "$LOCK_DIR" 2>/dev/null; then
      echo "$$" > "$LOCK_DIR/pid" 2>/dev/null
      LOCKED=1
      return 0
    fi
  fi
  return 1
}

# --- cuerpo ---------------------------------------------------------------
MODE=once
case "${1:-}" in
  --selftest) MODE=selftest ;;
  --status)   MODE=status ;;
  --help|-h)  usage; exit 0 ;;
  --once)     MODE=once ;;
  "")         MODE=once ;;
  *) echo "opción desconocida: $1" >&2; usage >&2; exit 2 ;;
esac

[ "$MODE" = status ]   && { show_status; exit $?; }
[ "$MODE" = selftest ] && { selftest; exit $?; }

# 1) Lock: dos ciclos del mismo backlog nunca a la vez.
if ! acquire_lock; then
  say "hay otro ciclo en marcha (lock=$LOCK_DIR) -> me salgo sin tocar nada"
  write_status "$RC_SKIP" 0 "" "" "otro-ciclo-en-marcha"
  exit "$RC_SKIP"
fi
trap 'release_lock' EXIT INT TERM

# 2) Preflight: si falta lo imprescindible, se dice y se sale (no se cuelga).
if ! command -v python3 >/dev/null 2>&1; then
  say "preflight: NO hay python3 en el PATH (lo busco en $RUNTIME_BIN)"
  write_status "$RC_PREFLIGHT" 0 "" "" "sin-python3-en-el-path"
  exit "$RC_PREFLIGHT"
fi
if [ -z "${ANTIGRAVITY_TASK_CMD:-}" ] && [ ! -f "$SNATCHER" ]; then
  say "preflight: NO existe el script $SNATCHER"
  write_status "$RC_PREFLIGHT" 0 "" "" "falta-el-script-del-ciclo"
  exit "$RC_PREFLIGHT"
fi

# 3) Ciclo: log con fecha + timeout duro.
LOG_FILE="$LOG_DIR/cycle-$(stamp).log"
T0=$(date +%s)
say "arranco ciclo -> $LOG_FILE (timeout ${MAX_RUNTIME}s)"
say "comando: $TASK_CMD (digest $(cmd_digest "$TASK_CMD"))"

# `timeout` devuelve 124 si se agota; el log conserva lo que salió hasta ese punto.
timeout -k 10 "$MAX_RUNTIME" sh -c "$TASK_CMD" >> "$LOG_FILE" 2>&1
RC=$?
DUR=$(( $(date +%s) - T0 ))

CYCLE="$(detect_cycle "$LOG_FILE")"
case "$CYCLE" in
  *"#"*) CYCLE="${CYCLE##*#}" ;;
  *)     CYCLE="desconocido" ;;
esac

case "$RC" in
  0)   WHY="ok";                        say "ciclo OK en ${DUR}s (ciclo=$CYCLE)" ;;
  124) WHY="timeout-max-runtime";       say "TIMEOUT tras ${MAX_RUNTIME}s (ciclo=$CYCLE)"; \
       RC="$RC_TIMEOUT" ;;
  125) WHY="timeout-no-lanzado";        say "el timeout no pudo lanzar el comando"; \
       RC="$RC_FAILED" ;;
  *)   WHY="ciclo-con-error";            say "el ciclo falló con rc=$RC tras ${DUR}s" ;;
esac

write_status "$RC" "$DUR" "$LOG_FILE" "$CYCLE" "$WHY"

# 4) Rotación: el móvil no tiene RAM de sobra; los logs viejos se van.
ls -1t "$LOG_DIR"/cycle-*.log 2>/dev/null | tail -n +"$((KEEP_LOGS + 1))" | while read -r old; do
  rm -f "$old" 2>/dev/null
done

say "estado -> $STATUS_FILE"
exit "$RC"
