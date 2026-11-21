#!/usr/bin/env bash
# md-telecom — lanzador de la CLI de telefonía para el PC del usuario.
#
# Qué hace: compila (si hace falta) y ejecuta el binario `md-telecom` del
# crate de MegaDaemon ⇒ **una sola fuente de verdad**: la CLI delega en los
# módulos `telecom`/`otp`/`antispam` del crate, no hay una reimplementación en
# shell.
#
# Uso:
#   ./tools/md-telecom.sh --help
#   ./tools/md-telecom.sh otp --file inbox.txt
#   ./tools/md-telecom.sh antispam --file inbox.txt --json
#
# Dónde se saca el buzón (en el MÓVIL, Termux):
#   su -mm -c 'content query --uri content://sms/inbox --projection _id:address:body:date' > inbox.txt
#
# ⚠️ Los códigos OTP se imprimen SÓLO en esta terminal local: no se mandan a
# ningún canal compartido, ni al tablón, ni a un chat.

set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATE="$RAIZ/rust-core"
BIN="$CRATE/target/release/md-telecom"

if [[ ! -x "$BIN" ]]; then
  echo "[md-telecom] compilando (la primera vez tarda; -j 1 para no morir de OOM)..." >&2
  # Target dir local: el compartido se corrompe con builds concurrentes.
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$CRATE/target-local-md-telecom}"
  (cd "$CRATE" && cargo build --release --bin md-telecom -j 1 >&2)
  BIN="$CARGO_TARGET_DIR/release/md-telecom"
fi

exec "$BIN" "$@"
