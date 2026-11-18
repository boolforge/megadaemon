#!/usr/bin/env bash
# Reproducible checks for v3.4 final.
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
OUT="$HERE/../verify/checks_output.md"
{
  echo "# Executable Verification Output — v3.4 Final"
  echo
  echo "Generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "Host: $(uname -a)"
  echo
  echo "## Environment"
  echo
  echo '```'
  for t in python3 gcc g++ node as z3 rustc nasm wasmtime wat2wasm; do
    printf "%-12s " "$t"
    if command -v "$t" >/dev/null 2>&1; then
      "$t" --version 2>&1 | head -1 || echo present
    else
      echo ABSENT
    fi
  done
  printf "%-12s " "numpy"
  python3 -c "import numpy; print('numpy', numpy.__version__)" 2>&1 | head -1 || echo ABSENT
  echo '```'
  echo
  echo "## Check 1 — Overlap predicate §3.3.2"
  echo '```'
  python3 "$HERE/overlap.py"
  echo '```'
  echo
  echo "## Check 2 — x86 vs AArch64 carry polarity"
  echo '```'
  python3 "$HERE/polarity.py"
  echo '```'
  echo
  echo "## Check 3 — x86-64 alignment idiom"
  echo '```'
  python3 "$HERE/align.py"
  echo '```'
  echo
  echo "## Check 4 — Conformal quantile"
  echo '```'
  python3 "$HERE/conformal.py"
  echo '```'
  echo
  echo "## Check 5 — §10.3 badge SHA-256 digests"
  echo '```'
  python3 "$HERE/badges.py"
  echo '```'
  echo
  echo "## Linter"
  echo '```'
  python3 "$HERE/lint_spec.py" "$HERE/../LTO-Decomp-Architecture-Specification-v3.4.md"
  echo '```'
} > "$OUT"
cat "$OUT"
