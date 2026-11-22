#!/system/bin/sh
# ---------------------------------------------------------------------------
# SUPERVISOR DEL BACKLOG DE ANTIGRAVITY  (S21 Ultra)
#
# Por qué este fichero existe (medido el 2026-11-22 en el móvil, por root):
#
#   * El micro-daemon de Antigravity vive DENTRO del proceso `com.agy`.
#     Medido: su última línea de log es
#     `[2026-11-19 10:21:33] Worker started for task [Snatcher-MegaDaemon-SuperLoop]`
#     y el `daemon.pid` (6905) hoy pertenece a OTRO proceso
#     (`io.github.mekhontsev.magicdesk`) ⇒ el planificador murió y las 2
#     tareas de `tasks.json` quedaron `enabled=true` pero sin ejecutar 3 días.
#   * Los reinicios tampoco lo salvan: el móvil reinició 4 veces después
#     (dropbox `SYSTEM_BOOT` 20-Nov 07:23, 20-Nov 20:25, 21-Nov 02:55,
#     21-Nov 05:18) y el backlog siguió muerto.
#
# Qué hace este supervisor:
#   1. Mantiene vivo el runner (`antigravity-task-runner.sh`) cada N segundos.
#   2. **Catch-up**: ejecuta las tareas con disparo `delay` cuyo `run_at` ya
#      pasó y que NUNCA corrieron. Medido: `task_553653` (`minimax_bootstrap`,
#      `run_at` 2026-11-19 12:09:13) nunca corrió y su log
#      (`agy_standalone/bootstrap.log`) no existe.
#   3. No pisa al daemon de la app: si `daemon.log` se ha tocado hace poco,
#      el daemon vivo es el dueño del ciclo y el supervisor se aparta.
#   4. Arranca solo tras cada reinicio: se instala como script de `service.d`
#      de Magisk (`/data/adb/service.d/97-antigravity-tasks.sh`), que es
#      exactamente el mecanismo que ya usa este móvil
#      (`91-adb-tcp.sh`, `98-cloudflared.sh`, `99-megadaemon.sh`).
#
# Uso:
#   antigravity-task-supervisor.sh --install     # copia + service.d + arranca
#   antigravity-task-supervisor.sh --uninstall
#   antigravity-task-supervisor.sh --start        # arranca el bucle (detached)
#   antigravity-task-supervisor.sh --stop
#   antigravity-task-supervisor.sh --status
#   antigravity-task-supervisor.sh                # bucle en primer plano
# ---------------------------------------------------------------------------
set -u

AGY_ROOT="${AGY_ROOT:-/data/user/0/com.agy/files}"
RUNTIME_BIN="$AGY_ROOT/runtime/bin"
DAEMON_LOG="${ANTIGRAVITY_DAEMON_LOG:-$AGY_ROOT/.gemini/antigravity-app/daemon/daemon.log}"
TASKS_JSON="${ANTIGRAVITY_TASKS_JSON:-$AGY_ROOT/.gemini/antigravity-app/daemon/tasks.json}"

STATE_DIR="${ANTIGRAVITY_STATE_DIR:-/data/local/tmp/megadaemon/antigravity}"
LOG_DIR="$STATE_DIR/logs"
SUP_LOG="$LOG_DIR/supervisor.log"
PID_FILE="$STATE_DIR/supervisor.pid"
STATUS_FILE="$STATE_DIR/supervisor.status"

INTERVAL="${ANTIGRAVITY_INTERVAL:-300}"
CATCHUP_MAX_RUNTIME="${ANTIGRAVITY_CATCHUP_MAX_RUNTIME:-300}"
DAEMON_ALIVE_WINDOW="${ANTIGRAVITY_DAEMON_ALIVE_WINDOW:-600}"  # s: log tocado = daemon vivo
SERVICE_D="${ANTIGRAVITY_SERVICE_D:-/data/adb/service.d}"
SERVICE_SCRIPT="$SERVICE_D/97-antigravity-tasks.sh"

PATH="$RUNTIME_BIN:$PATH"
export PATH

SRC_DIR="$(cd "$(dirname "$0")" 2>/dev/null && pwd)"

mkdir -p "$LOG_DIR" 2>/dev/null

say() { echo "[$(date '+%Y-%m-%dT%H:%M:%S%z')] [antigravity-supervisor] $*" | tee -a "$SUP_LOG"; }

is_running() {
  [ -f "$PID_FILE" ] || return 1
  _p="$(cat "$PID_FILE" 2>/dev/null)"
  [ -n "$_p" ] || return 1
  [ -d "/proc/$_p" ]
}

daemon_alive() {
  # Si el log del daemon se ha tocado hace poco, el daemon VIVO es el dueño.
  [ -f "$DAEMON_LOG" ] || return 1
  _m="$(stat -c %Y "$DAEMON_LOG" 2>/dev/null)"
  [ -n "$_m" ] || return 1
  _now="$(date +%s)"
  [ $(( _now - _m )) -lt "$DAEMON_ALIVE_WINDOW" ]
}

catchup_pending() {
  # Ejecuta las tareas `delay` que nunca corrieron. TSV: id \t comando
  _py="$RUNTIME_BIN/python3"
  [ -x "$_py" ] || _py="python3"
  _helper="$STATE_DIR/antigravity-tasks.py"
  [ -f "$_helper" ] || return 0
  _out="$("$_py" "$_helper" --pending "$TASKS_JSON" 2>/dev/null | tail -n +2)"
  [ -n "$_out" ] || return 0
  echo "$_out" | while IFS="$(printf '\t')" read -r id cmd; do
    [ -n "$id" ] || continue
    [ -n "$cmd" ] || continue
    _log="$LOG_DIR/catchup-$(date '+%Y%m%d-%H%M%S')-$id.log"
    say "catch-up: ejecuto '$id' (timeout ${CATCHUP_MAX_RUNTIME}s) -> $_log"
    timeout -k 10 "$CATCHUP_MAX_RUNTIME" sh -c "$cmd" >> "$_log" 2>&1
    say "catch-up: '$id' terminado con rc=$? (log $_log)"
  done
}

run_cycle() {
  "$STATE_DIR/antigravity-task-runner.sh" --once
  _rc=$?
  {
    echo "ts=$(date '+%Y-%m-%dT%H:%M:%S%z')"
    echo "rc=$_rc"
  } > "$STATUS_FILE"
  return $_rc
}

install() {
  mkdir -p "$STATE_DIR" || return 1
  cp -f "$SRC_DIR/antigravity-task-runner.sh" "$STATE_DIR/antigravity-task-runner.sh" || return 1
  cp -f "$SRC_DIR/antigravity-tasks.py"        "$STATE_DIR/antigravity-tasks.py"        || return 1
  chmod 0755 "$STATE_DIR/antigravity-task-runner.sh" "$STATE_DIR/antigravity-tasks.py"
  [ -f "$SRC_DIR/antigravity-task-supervisor.sh" ] && \
    cp -f "$SRC_DIR/antigravity-task-supervisor.sh" "$STATE_DIR/antigravity-task-supervisor.sh"
  [ -f "$STATE_DIR/antigravity-task-supervisor.sh" ] && \
    chmod 0755 "$STATE_DIR/antigravity-task-supervisor.sh"

  mkdir -p "$SERVICE_D" 2>/dev/null
  if [ -d "$SERVICE_D" ]; then
    cat > "$SERVICE_SCRIPT" <<EOF
#!/system/bin/sh
# Magisk service.d: arranca el backlog de Antigravity tras cada reinicio.
# Generado por antigravity-task-supervisor.sh --install
STATE_DIR="$STATE_DIR"
[ -x "\$STATE_DIR/antigravity-task-supervisor.sh" ] || exit 0
setsid nohup "\$STATE_DIR/antigravity-task-supervisor.sh" --start >/dev/null 2>&1
EOF
    chmod 0755 "$SERVICE_SCRIPT" 2>/dev/null
    say "instalado: $SERVICE_SCRIPT"
  else
    say "AVISO: $SERVICE_D no existe -> no hay arranque automático tras reinicio"
  fi
  say "instalado en $STATE_DIR"
}

uninstall() {
  rm -f "$SERVICE_SCRIPT" 2>/dev/null
  stop
  say "desinstalado (se deja el estado en $STATE_DIR)"
}

start() {
  if is_running; then
    say "ya está corriendo (pid $(cat "$PID_FILE"))"
    return 0
  fi
  setsid nohup "$0" >> "$SUP_LOG" 2>&1 &
  echo $! > "$PID_FILE"
  say "arrancado (pid $(cat "$PID_FILE"))"
}

stop() {
  if is_running; then
    _p="$(cat "$PID_FILE")"
    kill "$_p" 2>/dev/null
    say "parado (pid $_p)"
  else
    say "no estaba corriendo"
  fi
  rm -f "$PID_FILE"
}

status() {
  echo "supervisor: $(is_running && echo "vivo (pid $(cat "$PID_FILE"))" || echo "parado")"
  echo "estado_dir: $STATE_DIR"
  echo "interval: ${INTERVAL}s   daemon_alive_window: ${DAEMON_ALIVE_WINDOW}s"
  daemon_alive && echo "daemon de la app: VIVO (es el dueño del ciclo)" \
                 || echo "daemon de la app: sin señal -> el supervisor puede correr el ciclo"
  [ -f "$STATUS_FILE" ] && { echo "--- $STATUS_FILE"; cat "$STATUS_FILE"; }
  [ -f "$STATE_DIR/last_status.json" ] && { echo "--- last_status.json"; cat "$STATE_DIR/last_status.json"; }
  return 0
}

case "${1:-}" in
  --install)   install && start ;;
  --uninstall) uninstall ;;
  --start)     start ;;
  --stop)      stop ;;
  --status)    status ;;
  --help|-h)   sed -n '2,60p' "$0" | sed 's/^# \{0,1\}//' ;;
  *)
    # Bucle en primer plano (lo que lanza service.d con `setsid nohup`).
    trap 'say "parado por señal"; exit 0' TERM INT
    say "bucle en marcha (interval ${INTERVAL}s, catch-up al arrancar)"
    catchup_pending
    while :; do
      if daemon_alive; then
        say "el daemon de la app está vivo -> no duplo el ciclo"
      else
        say "el daemon de la app no responde -> lanzo el ciclo"
        run_cycle || true
      fi
      sleep "$INTERVAL"
    done
    ;;
esac
