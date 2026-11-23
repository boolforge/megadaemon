#!/usr/bin/env bash
# Build/test de MegaDaemon con LOCK: un solo actor compila a la vez.
#
# POR QUE (2026-11-22): el sandbox tiene ~3 GB de RAM.  El crate tiene
# ~700 lineas propias y ~200 dependencias; el ENLAZADO de la lib o de los
# tests se come >3 GB y muere por SIGKILL (OOM).  Con 3 agentes compilando
# a la vez, los tres mueren y ademas compiten por el mismo codigo.
#
# REGLA: SOLO el root (la sesion principal) compila, testea y pushea.
# Los agentes entregan CODIGO (rama + commit + nota en el bus) y NO compilan.
set -uo pipefail
LOCK="${MD_BUILD_LOCK:-/workspace/.build-lock}"
LOG="${MD_BUILD_LOG:-/workspace/.build.log}"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/workspace/md-target-rescate}"
export CARGO_TARGET_DIR
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_CODEGEN_UNITS=1 CARGO_PROFILE_DEV_OPT_LEVEL=1
export CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_TEST_CODEGEN_UNITS=1 CARGO_PROFILE_TEST_OPT_LEVEL=1

if [ "${1:-}" = "--libre" ]; then rmdir "$LOCK" 2>/dev/null && echo "lock liberado"; exit 0; fi

t=0
while ! mkdir "$LOCK" 2>/dev/null; do
  t=$((t+1))
  if [ "$t" -gt 180 ]; then echo "timeout esperando el lock de build"; exit 1; fi
  sleep 2
done
trap 'rmdir "$LOCK" 2>/dev/null' EXIT
echo "[build] $(date +%T) $* -> $LOG"
{ echo "=== $(date +%T) $* ==="; "$@"; echo "=== rc=$? $(date +%T) ==="; } >> "$LOG" 2>&1
tail -30 "$LOG"
