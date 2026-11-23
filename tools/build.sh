#!/usr/bin/env bash
# Build/test de MegaDaemon con LOCK y GUARDIÁN: un solo actor compila a la vez.
#
# POR QUE (2026-11-22/23): el sandbox tiene ~3 GB de RAM.  El crate tiene
# ~700 lineas propias y ~200 dependencias; el ENLAZADO de la lib o de los
# tests se come >1,5 GB y muere por SIGKILL (OOM).  Con 3 actores compilando
# a la vez, los tres mueren, compiten por el mismo codigo y DEJAN MUERTOS a
# los workers de Ghidra (medido: 3 OOM en dmesg, los 3 matando un rustc de
# 2,8 GB).
#
# REGLA: SOLO el root (la sesion principal) compila, testea y pushea.
# Los agentes entregan CODIGO (rama + commit + nota en el bus) y NO compilan:
# para comprobar sintaxis usan `tools/check-sintaxis.sh` (instantaneo, sin
# RAM) y `tools/guardia-compilacion.sh` les dice si pueden.
#
# USO:
#   tools/build.sh cargo test --lib          # con candado + guardian
#   tools/build.sh --libre                   # libera el candado
#   tools/build.sh --estado                  # veredicto del guardian
set -uo pipefail

RAIZ="$(cd "$(dirname "$(readlink -f "$0")")/.." && pwd)"
GUARDIA="$RAIZ/tools/guardia-compilacion.sh"
LOCK="${MD_BUILD_LOCK:-/workspace/.build-lock}"
LOG="${MD_BUILD_LOG:-/workspace/.build.log}"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/workspace/md-target-rescate}"
export CARGO_TARGET_DIR
# Perfil LIGERO: sin debug info, un solo unit de codegen, optimizacion baja.
# El link es la parte que se come la RAM.
# Perfil LIGERO y de POCA MEMORIA (medido el 2026-11-23: con
# CODEGEN_UNITS=1 y el paralelismo por defecto, un `rustc` de este crate llega
# a 2,0 GB de RSS y el OOM killer lo mata en un sandbox de 3 GB => 4 OOM
# medidos).  Con 16 unidades y `-j 1` cada `rustc` se queda en ~800 MB.
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_CODEGEN_UNITS=16 CARGO_PROFILE_DEV_OPT_LEVEL=1
export CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_TEST_CODEGEN_UNITS=16 CARGO_PROFILE_TEST_OPT_LEVEL=1
export CARGO_INCREMENTAL=0
# Un solo `rustc` a la vez: el limite real es la RAM, no la CPU.
export CARGO_BUILD_JOBS=1

if [ "${1:-}" = "--libre" ]; then rmdir "$LOCK" 2>/dev/null && echo "lock liberado"; exit 0; fi
if [ "${1:-}" = "--estado" ]; then exec "$GUARDIA"; fi

# --- guardian (RAM + actores) ANTES de coger el candado ----------------------
# OJO: se consulta con MD_BUILD_IGNORE_LOCK=1 porque todavia no hemos cogido
# el candado; si se consultase despues, el guardian veria SU PROPIO lock y se
# vetaria siempre (bug medido el 2026-11-23).
if ! MD_BUILD_IGNORE_LOCK=1 "$GUARDIA"; then
  echo "[build] NO se compila: el guardian lo veta (ver arriba)."
  exit 1
fi

# --- candado (un solo actor) ------------------------------------------------
t=0
while ! mkdir "$LOCK" 2>/dev/null; do
  t=$((t+1))
  if [ "$t" -gt 180 ]; then echo "timeout esperando el lock de build ($LOCK)"; exit 1; fi
  sleep 2
done
trap 'rmdir "$LOCK" 2>/dev/null' EXIT

# --- segundo control: otro actor puede haber arrancado mientras esperabamos --
if ! MD_BUILD_IGNORE_LOCK=1 "$GUARDIA"; then
  echo "[build] NO se compila: aparecio otro actor mientras esperaba el candado."
  exit 1
fi

echo "[build] $(date +%T) $* -> $LOG"
{ echo "=== $(date +%T) $* ==="; "$@"; echo "=== rc=$? $(date +%T) ==="; } >> "$LOG" 2>&1
rc=$?
tail -30 "$LOG"
exit $rc
