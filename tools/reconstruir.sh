#!/usr/bin/env bash
# tools/reconstruir.sh — reconstruccion multi-motor de un binario, de punta a
# punta.  Es el equivalente de shell de `md-re fusion` y del tool MCP
# `re_fusion`: mismo pipeline, mismos artefactos.
#
#   Ghidra (pseudocodigo)  +  Rizin (funciones/imports)  +  DiE/Binwalk
#   +  motor propio (MBA/CFF/crypto)  +  pclntab (simbolos Go con fichero:linea)
#   => comparativa + libro fusionado
#
# uso:
#   tools/reconstruir.sh <binario> [opciones]
#
# opciones:
#   --out DIR            donde escribir (def: /workspace/agy-re/tools-out/fusion)
#   --project-dir DIR    proyecto Ghidra (def: /workspace/agy-re/tools-out/proj)
#   --project NAME       nombre del proyecto (def: Go)
#   --symbols TSV        export de simbolos (def: <out>/../go.funcs.tsv)
#   --anchors TSV        anclas a decompilar (def: se generan del TSV de simbolos)
#   --ghidra-dir DIR     instalacion de Ghidra (def: autodeteccion)
#   --java-home DIR      JAVA_HOME (def: autodeteccion; Ghidra 11.2+ exige JDK 21)
#   --batch N            funciones por lote de decompilacion (def: 200)
#   --skip-ghidra        no ejecutar Ghidra (todo lo demas)
#   --skip-import        no reimportar (el proyecto ya existe)

set -uo pipefail

BIN="${1:-}"; shift || true
OUT="/workspace/agy-re/tools-out/fusion"
PROJ_DIR="/workspace/agy-re/tools-out/proj"
PROJ="Go"
SYMS=""
ANCHORS=""
GHIDRA_DIR=""
JAVA_HOME_DIR=""
BATCH=200
SKIP_GHIDRA=0
SKIP_IMPORT=0

while [ $# -gt 0 ]; do
  case "$1" in
    --out) OUT="$2"; shift 2;;
    --project-dir) PROJ_DIR="$2"; shift 2;;
    --project) PROJ="$2"; shift 2;;
    --symbols) SYMS="$2"; shift 2;;
    --anchors) ANCHORS="$2"; shift 2;;
    --ghidra-dir) GHIDRA_DIR="$2"; shift 2;;
    --java-home) JAVA_HOME_DIR="$2"; shift 2;;
    --batch) BATCH="$2"; shift 2;;
    --skip-ghidra) SKIP_GHIDRA=1; shift;;
    --skip-import) SKIP_IMPORT=1; shift;;
    *) echo "opcion desconocida: $1" >&2; exit 2;;
  esac
done

if [ -z "$BIN" ] || [ ! -f "$BIN" ]; then
  echo "uso: $0 <binario> [opciones]" >&2; exit 2
fi

mkdir -p "$OUT"
# el GhidraScript sale del propio modulo Rust (fuente unica)
"$(dirname "$(readlink -f "$0")")/extraer-ghidra-script.sh" || true
LOG="$OUT/pipeline.log"
say() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }

# ---------------------------------------------------------------- entorno
GH=""
if [ -z "$GHIDRA_DIR" ]; then
  for c in /workspace/agy-re/tools-out/ghidra_*_PUBLIC /opt/ghidra* /usr/local/ghidra*; do
    [ -x "$c/support/analyzeHeadless" ] && GH="$c" && break
  done
else
  GH="$GHIDRA_DIR"
fi
if [ -z "$JAVA_HOME_DIR" ]; then
  for c in /opt/jdk-21* /usr/lib/jvm/*21* /opt/java/*21*; do
    [ -x "$c/bin/javac" ] && JAVA_HOME_DIR="$c" && break
  done
else
  JAVA_HOME_DIR="$JAVA_HOME_DIR"
fi
say "binario : $BIN ($(stat -c%s "$BIN") bytes)"
say "ghidra  : ${GH:-<no encontrado>}"
say "java    : ${JAVA_HOME_DIR:-<no encontrado>}"

# ------------------------------------------------- simbolos (unstrip/pclntab)
if [ -z "$SYMS" ]; then
  for c in /workspace/agy-re/tools-out/go.funcs.tsv "$OUT/../go.funcs.tsv"; do
    [ -f "$c" ] && SYMS="$c" && break
  done
fi
if [ -z "$SYMS" ] || [ ! -f "$SYMS" ]; then
  say "AVISO: sin export de simbolos; el pclntab no se podra cruzar"
  SYMS=""
else
  say "simbolos: $SYMS ($(wc -l < "$SYMS") lineas)"
fi

# --------------------------------------------------------------- anclas
if [ -z "$ANCHORS" ] && [ -n "$SYMS" ]; then
  ANCHORS="$OUT/anchors.tsv"
  say "generando anclas desde el export de simbolos (lote de $BATCH)"
  awk -F'\t' -v n="$BATCH" '
    { print $1 "\t" $2 "\t" $3 "\t" $3 "\tauto" }
  ' "$SYMS" | head -n 200000 > "$ANCHORS"
fi

# ---------------------------------------------------------------- Ghidra
if [ "$SKIP_GHIDRA" -eq 0 ] && [ -n "$GH" ]; then
  export JAVA_HOME="$JAVA_HOME_DIR"
  MAXMEM="${MD_GHIDRA_MAXMEM:-1200M}"
  AH="$GH/support/analyzeHeadless"
  if [ "$SKIP_IMPORT" -eq 0 ]; then
    say "Ghidra: importando (puede tardar varios minutos)..."
    "$AH" "$PROJ_DIR" "$PROJ" -import "$BIN" -noanalysis -overwrite -max-cpu 1 \
        -log "$PROJ_DIR/import.log" >>"$LOG" 2>&1
    say "Ghidra: import rc=$?"
  fi
  if [ -n "$SYMS" ]; then
    say "Ghidra: aplicando simbolos..."
    "$AH" "$PROJ_DIR" "$PROJ" -process -noanalysis -max-cpu 1 \
        -scriptPath "$(dirname "$(readlink -f "$0")")/../ghidra-scripts" \
        -postScript MdGhidra.java symbols "$SYMS" "$ANCHORS" "$OUT" >>"$LOG" 2>&1
    say "Ghidra: simbolos rc=$? (mira 'MDGHIDRA: symbols done' en el log)"
  fi
  if [ -n "$ANCHORS" ]; then
    say "Ghidra: decompilando..."
    "$AH" "$PROJ_DIR" "$PROJ" -process -noanalysis -max-cpu 1 \
        -scriptPath "$(dirname "$(readlink -f "$0")")/../ghidra-scripts" \
        -postScript MdGhidra.java decompile "$SYMS" "$ANCHORS" "$OUT/pseudocodigo" >>"$LOG" 2>&1
    say "Ghidra: pseudocodigo rc=$? ($(ls "$OUT/pseudocodigo" 2>/dev/null | wc -l) ficheros)"
  fi
  # resolver los strings referenciados por el pseudocodigo
  RES="$(dirname "$(readlink -f "$0")")/resolver-strings-ghidra.py"
  if [ -f "$RES" ] && [ -d "$OUT/pseudocodigo" ]; then
    say "resolviendo strings referenciados..."
    python3 "$RES" "$OUT/pseudocodigo" "$OUT/legible" 2>&1 | tee -a "$LOG"
  fi
else
  say "Ghidra: omitido"
fi

# ------------------------------------------------- comparativa + fusion (Rust)
MD_RE="$(dirname "$(readlink -f "$0")")/../rust-core/target/debug/md-re"
[ -x "$MD_RE" ] || MD_RE="$(dirname "$(readlink -f "$0")")/../rust-core/target/release/md-re"
if [ -x "$MD_RE" ]; then
  say "fusión multi-motor (md-re fusion)..."
  ARGS=(fusion "$BIN" --out "$OUT" --project-dir "$PROJ_DIR" --project "$PROJ")
  [ -n "$SYMS" ] && ARGS+=(--symbols "$SYMS")
  [ -n "$ANCHORS" ] && ARGS+=(--anchors "$ANCHORS")
  [ -n "$GHIDRA_DIR" ] && ARGS+=(--ghidra-dir "$GHIDRA_DIR")
  [ -n "$JAVA_HOME_DIR" ] && ARGS+=(--java-home "$JAVA_HOME_DIR")
  "$MD_RE" "${ARGS[@]}" > "$OUT/fusion.json" 2>>"$LOG"
  say "fusión rc=$? -> $OUT/fusion.json"
else
  say "AVISO: md-re no está compilado ($MD_RE); usa el arnés:"
  say "  cd tools/re_fusion-harness && cargo run --offline --bin fusion"
fi

say "hecho. Artefactos en $OUT"
ls -la "$OUT" | tee -a "$LOG"
