#!/usr/bin/env bash
# Gate de SINTAXIS: parsea cada .rs con rustfmt ANTES de compilar.
#
# MEDIDO (2026-11-22): main se rompio DOS veces por merges que concatenaron
# texto:
#   * rust-core/src/backup_catalog.rs  -> "ResumenInventario {" DUPLICADO
#     => "unclosed delimiter" => tumbaba los 3 jobs de build y el release;
#   * rust-core/tests/llm_routing.rs  -> un '<' en el NOMBRE de un test
#     => el binario de test no compilaba => tumbaba el job de calidad.
# Compilar el crate entero tarda 10 min y se come la RAM del runner; rustfmt
# detecta esto en SEGUNDOS y sin RAM.
#
# NO es un gate de FORMATO: rustfmt solo falla aqui si el fichero NO PARSEA.
set -uo pipefail
RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
cd "$RAIZ" || exit 1
MAL=0
N=0
while IFS= read -r -d '' f; do
  N=$((N+1))
  if ! out="$(rustfmt --edition 2021 --emit stdout --quiet "$f" 2>&1 >/dev/null)"; then
    echo "::error file=$f::no parsea (sintaxis):"
    printf '%s\n' "$out" | head -6
    MAL=$((MAL+1))
  fi
# MEDIDO (2026-11-27): `artifacts/` contiene material de TERCEROS descargado
# para analisis (ACPira, acpschema, mcode, goose, Codewhale) mas el codigo
# descompilado de los productos. Son 6.206 ficheros .rs ajenos al crate y 169
# de ellos no parsean (son Rust de otras versiones/ediciones): el gate senalaba
# 169 errores que NO eran del repo. Solo se analiza el codigo del crate.
done < <(find . -name '*.rs' \
  -not -path './target/*' -not -path './*/target/*' \
  -not -path './artifacts/*' \
  -not -path './vendor/*' \
  -print0)
echo "::notice::ficheros rust analizados: $N · con error de sintaxis: $MAL"
if [ "$MAL" -gt 0 ]; then
  echo "SINTASIS ROTA en $MAL fichero(s) - no compiles todavia"
  exit 1
fi
echo "sintaxis OK en los $N ficheros"

# MEDIDO (2026-11-28): main estaba en ROJO desde el 2026-11-21 por un arnés
# de test que copiaba un modulo de la libreria (`#[path = "../src/x.rs"]`):
# dentro del arnes `crate::` es la raiz del ARNES, no de la libreria => el
# target de test no compilaba => el job "calidad" moria con rc=101 SIN
# ejecutar ni un test (y tardaba 15-20 min en descubrirlo).  Este gate lo
# ve en SEGUNDOS.
if ! out="$(python3 tools/check-arnes.py 2>&1)"; then
  printf '%s\n' "$out"
  echo "ARNES ROTOS - no compiles todavia"
  exit 1
fi
printf '%s\n' "$out" | tail -2
