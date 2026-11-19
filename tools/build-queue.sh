#!/usr/bin/env bash
# =====================================================================
# tools/build-queue.sh — COLA DE BUILDS de MegaDaemon (v14)
# =====================================================================
# POR QUÉ EXISTE (medido, no opiniones):
#   En una sesión con 24 agentes se llegaron a contar 52 procesos `cargo`
#   simultáneos sobre un NAS compartido. Resultado: fallos FALSOS
#   ("failed to copy bitcode to object file", "rust-lld: cannot open
#   ...rcgu.o", "database is locked") que NO eran bugs del código, y
#   builds de minutos que se quedaban esperando el lock.
#
# QUÉ HACE:
#   1. SERIALIZA los builds con `flock`: como máximo N slots a la vez
#      (3 por defecto). Nadie más compila en paralelo → el FS no se
#      satura y desaparecen los errores falsos.
#   2. CACHE COMPARTIDA: todos usan el MISMO `CARGO_TARGET_DIR`
#      (/workspace/md-build). Como el árbol de trabajo es el mismo, cargo
#      reutiliza los artefactos: el segundo build es incremental de
#      verdad (segundos, no minutos).
#   3. `CARGO_INCREMENTAL=0` y `-j 2`: el incremental sobre NAS es la
#      causa del "failed to copy bitcode"; con 2 jobs no saturas la I/O.
#
# USO:
#   tools/build-queue.sh build --release
#   tools/build-queue.sh test --release
#   tools/build-queue.sh check
#   CARGO_TARGET_DIR_PER_AGENT=1 tools/build-queue.sh build --release
#
# EXIT: el de cargo. Bloquea (no falla) si no hay slot: eso es correcto.
# =====================================================================
set -uo pipefail

SLOTS="${MD_BUILD_SLOTS:-2}"
SHARED_TARGET="${MD_BUILD_TARGET:-/workspace/md-build}"
LOCKDIR="${MD_BUILD_LOCKDIR:-/workspace/md-build-slots}"
PER_AGENT="${CARGO_TARGET_DIR_PER_AGENT:-0}"
AGENT="${MD_AGENT_NAME:-$(basename "${PWD}")}"

# Directorio del repo (raíz del workspace de MegaDaemon).
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATE_DIR="${REPO_ROOT}/rust-core"

mkdir -p "$LOCKDIR" 2>/dev/null || true
chmod 700 "$LOCKDIR" 2>/dev/null || true

# --- Target dir ------------------------------------------------------------
if [ "$PER_AGENT" = "1" ]; then
  TARGET_DIR="${SHARED_TARGET}-${AGENT}"
else
  TARGET_DIR="$SHARED_TARGET"
fi
mkdir -p "$TARGET_DIR" 2>/dev/null || true

# --- Semáforo de slots ----------------------------------------------------
acquire_slot() {
  local i fd
  for i in $(seq 1 "$SLOTS"); do
    exec {fd}>"${LOCKDIR}/slot.${i}" || continue
    if flock -n "$fd"; then
      echo "$fd"
      return 0
    fi
    exec {fd}>&-
  done
  # Todos ocupados: esperar (esto es lo correcto, no un fallo).
  echo "⏳ cola de builds: esperando slot (SLOTS=$SLOTS)" >&2
  exec {fd}>"${LOCKDIR}/slot.1"
  flock "$fd"
  echo "$fd"
}

SLOT_FD="$(acquire_slot)"
# El fd queda abierto durante la vida del script → mantiene el lock.

# --- Guardia de memoria ----------------------------------------------------
# El entorno tiene 3 GB de RAM: con 15+ builds concurrentes hubo OOM
# (load 31). Si la memoria libre cae por debajo del umbral, este build
# espera a que la liberen en lugar de empujar la máquina al limite.
MIN_FREE_MB="${MD_BUILD_MIN_FREE_MB:-512}"
free_mb() {
  local kb
  kb="$(awk '/^MemAvailable:/ {print $2}' /proc/meminfo 2>/dev/null)"
  [ -n "$kb" ] && echo $((kb / 1024)) || echo 999999
}
while :; do
  avail="$(free_mb)"
  [ "$avail" -ge "$MIN_FREE_MB" ] && break
  echo "⏳ memoria baja (${avail} MB libres < ${MIN_FREE_MB} MB): esperando" >&2
  sleep 20
done

cd "$CRATE_DIR" || exit 1

export CARGO_TARGET_DIR="$TARGET_DIR"
export CARGO_INCREMENTAL=0
export CARGO_TERM_COLOR=never

CMD="${1:-build}"
shift || true

echo "🔨 $CMD $* | target=$TARGET_DIR | slot=${SLOT_FD##* } | $(date +%H:%M:%S)" >&2

case "$CMD" in
  test)   exec cargo test -j 2 "$@" ;;
  check)  exec cargo check -j 2 "$@" ;;
  clippy) exec cargo clippy -j 2 "$@" ;;
  build)  exec cargo build -j 2 "$@" ;;
  *)      exec cargo "$CMD" -j 2 "$@" ;;
esac
