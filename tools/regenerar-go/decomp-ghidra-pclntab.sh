#!/usr/bin/env bash
# decomp-ghidra-pclntab.sh — 4.º motor: Ghidra headless con el CUERPO de la
# funcion forzado desde el pclntab (ver docs/RE_FUNCIONES_IRRECONSTRUIBLES.md).
#
# El camino de decompilacion de `ImportGoRetoolSymbols.java` usa `f.getBody()`
# (la extension que ADIVINA Ghidra); para las 103 funciones que fallan esa
# adivinacion es erronea (Ghidra se atribuia un cuerpo 6,54x mayor que el real)
# y el decompilador muera con "Cannot properly adjust input varnodes".
# Este script usa `DecompileCuerpoPclntab.java`, que fuerza el cuerpo a
# [va, va+tam) con tam = siguiente VA - VA del pclntab.
#
# uso: decomp-ghidra-pclntab.sh <lista.tsv> <outDir> <report.tsv>
set -u
LISTA="${1:-/workspace/agy-re/tools-out/anchors_md/lista_103_pclntab.tsv}"
OUT="${2:-/workspace/agy-re/tools-out/anchors_md/ghidra_pclntab_103}"
REPORT="${3:-/workspace/agy-re/tools-out/anchors_md/ghidra_pclntab_103/_decompile_report.tsv}"

export JAVA_HOME=/workspace/agy-re/tools-out/jdk-21.0.13.1+2-jre
export PATH="$JAVA_HOME/bin:$PATH"
export MAXMEM=1600M          # MEDIDO: el import anterior murio por OOM
GH=/workspace/agy-re/tools-out/ghidra_11.2.1_PUBLIC
PROJ=/workspace/agy-re/tools-out/gh2/proj
SCRIPTS=/workspace/md-re-productos/tools/regenerar-go/ghidra

mkdir -p "$OUT"
echo "[ghidra-pclntab] $(date -u +%T) lista=$LISTA out=$OUT"

# 1 solo hilo y heap acotado: NUNCA 2 JVMs en este sandbox de 2 CPU / 3 GB.
timeout 5400 "$GH/support/analyzeHeadless" "$PROJ" Go \
  -process liblanguage_server.so \
  -noanalysis -max-cpu 1 -readOnly \
  -scriptPath "$SCRIPTS" \
  -postScript DecompileCuerpoPclntab.java "$LISTA" "$OUT" "$REPORT" 300 \
  2>&1 | tail -40
rc=${PIPESTATUS[0]}
echo "[ghidra-pclntab] fin rc=$rc $(date -u +%T)"
echo "--- resumen del informe ---"
if [ -f "$REPORT" ]; then
    awk -F'\t' 'NR>1 {n++; if ($6=="true") ok++} END {printf "  funciones=%d ok=%d fallo=%d\n", n, ok, n-ok}' "$REPORT"
    grep -c 'dec_ok' "$REPORT" >/dev/null 2>&1
    echo "  .c generados: $(find "$OUT" -name '*.c' -size +0c | wc -l)"
fi
