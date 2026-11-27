#!/usr/bin/env bash
# Commit con reintentos por index.lock (otros workers commitean en el mismo worktree).
# Uso: git-commit-retry.sh "<mensaje>" <ruta> [ruta...]
set -uo pipefail
MSG="$1"; shift
for i in $(seq 1 12); do
  git add -- "$@" 2>/tmp/git-add.err && {
    if git commit -m "$MSG" -m "Co-Authored-By: Mavis <mavis@agy.internal>" 2>/tmp/git-commit.err; then
      echo "COMMIT OK (intento $i)"; exit 0
    fi
    grep -q 'nothing to commit' /tmp/git-commit.err && { echo "nada que commitear"; exit 0; }
  }
  sleep $((3 + RANDOM % 7))
done
echo "FALLO tras 12 intentos"; cat /tmp/git-add.err /tmp/git-commit.err 2>/dev/null | tail -5; exit 1
