#!/usr/bin/env bash
# Ejecuta la suite COMPLETA de tests y falla si hay algún binario de test
# rojo que no esté en la lista de "rojos conocidos".
#
# POR QUÉ EXISTE (medido el 2026-11-21):
# el CI ejecutaba `cargo test --all-targets` con `continue-on-error: true`, así
# que CUALQUIER test roto pasaba desapercibido.  Llevaba tiempo así: tres tests
# de `agent_loop` estaban rojos (contradecían a la implementación) y nadie se
# enteró porque el job salía "verde".
#
# `audit_adversarial_v15` sí tiene rojos INTENCIONALES (son auditorías que
# fallan mientras el defecto exista).  Esos están en la lista y NO bloquean.
# Cualquier otro rojo, bloquea.
#
# Uso: tools/ci-suite.sh [directorio-de-cargo]
# Variables: CI_KNOWN_RED (fichero con la lista), CI_NO_FAIL_FAST=0 para
#            depurar (por defecto se para en el primer binario rojo).

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CARGO_DIR="${1:-$REPO_ROOT/rust-core}"
KNOWN_RED_FILE="${CI_KNOWN_RED:-$REPO_ROOT/tools/ci-known-red.txt}"
LOG="$(mktemp -t md-suite-XXXXXX.log)"
trap 'rm -f "$LOG"' EXIT

if [ ! -d "$CARGO_DIR" ]; then
    echo "::error::no existe el directorio de cargo: $CARGO_DIR"
    exit 2
fi

cd "$CARGO_DIR" || exit 2

echo "::group::Suite completa (rust-core)"
# Entorno LIMPIO: una API key o un token exportados en el runner hacen fallar
# tests de autenticación por el ENTORNO, no por el código.  Caso real medido:
# con una key obsoleta exportada, `api_key::acunar_y_revocar_una_clave_no_toca_
# la_principal` falla; con el entorno limpio pasa.
# En local se puede pedir el perfil debug (CI_RELEASE=0): el release tarda
# muchisimo y en una caja de 3 GB puede morir de OOM.
if [ "${CI_RELEASE:-1}" = "1" ]; then
    PERFIL="--release"
else
    PERFIL=""
fi

env -u MEGADAEMON_API_KEY -u MEGADAEMON_TOKEN -u SEERE_AUTH_TOKEN \
    cargo test $PERFIL --all-targets --no-fail-fast >"$LOG" 2>&1
RC=$?
echo "::endgroup::"

# Resumen legible
grep -E '^test result:' "$LOG" | sed 's/^/  /' | tail -60

# Qué binarios de test han fallado.  Cargo imprime la línea "Running <algo>.rs"
# justo antes del "test result:" de ese binario.
FALLANOS="$(awk '
  /^ *Running / {
    if (match($0, /[a-z0-9_]+\.rs/)) { bin = substr($0, RSTART, RLENGTH - 3) }
    else { bin = "?" }
    next
  }
  /^test result: FAILED/ { if (bin != "") print bin }
' "$LOG" | sort -u)"

if [ -z "$FALLANOS" ]; then
    if [ "$RC" -ne 0 ]; then
        # Rojo sin binario identificado: se mira el log entero.
        echo "::error::la suite fallo (rc=$RC) pero no se pudo identificar el binario"
        tail -40 "$LOG"
        exit 1
    fi
    echo "::notice::suite completa VERDE"
    exit 0
fi

CONOCIDOS=""
[ -f "$KNOWN_RED_FILE" ] && CONOCIDOS="$(grep -vE '^\s*(#|$)' "$KNOWN_RED_FILE" | tr -d '\r')"

INESPERADOS=""
for b in $FALLANOS; do
    if ! printf '%s\n' "$CONOCIDOS" | grep -qx "$b"; then
        INESPERADOS="$INESPERADOS $b"
    fi
done

for b in $FALLANOS; do
    if printf '%s\n' "$CONOCIDOS" | grep -qx "$b"; then
        echo "::warning::test $b: rojo CONOCIDO (auditoria adversarial: falla mientras el defecto exista)"
    fi
done

if [ -n "$INESPERADOS" ]; then
    echo "::error::tests rojos INESPERADOS:$INESPERADOS"
    echo "::error::el job se marca como fallido.  Arregla el test o, si el rojo es intencionado, anadelo a tools/ci-known-red.txt con un motivo."
    for b in $INESPERADOS; do
        echo ""
        echo "───── salida de $b ─────"
        awk -v target="$b" '
          $0 ~ ("^ *Running .*" target "\\.rs") { on = 1 }
          on { print }
          on && /^test result:/ { exit }
        ' "$LOG" | tail -40
    done
    exit 1
fi

echo "::notice::los unicos rojos son los conocidos ($FALLANOS)"
exit 0
