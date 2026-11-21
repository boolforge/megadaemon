#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# md-recordings.sh — índice unificado de grabaciones + backup de seguridad
#
# PARA QUÉ
#   El móvil guarda las grabaciones en varios sitios (grabadora nativa de
#   Samsung, Telegram, WhatsApp, BCR…) y cada una con su propio nombre.  Este
#   script deja UNA lista unificada y hace una copia de seguridad incremental
#   con MANIFEST, para que el usuario pueda hacerlo en su PC sin el enjambre.
#
#   NO graba nada: sólo indexa, unifica y respalda lo que YA existe.
#
#   Motor: `rust-core/src/bin/md-recordings.rs` (módulo `recordings` de la
#   lib).  Modelo de datos y formato del MANIFEST:
#   `docs/GRABACIONES_INDICE_Y_BACKUP.md`.
#
# USO
#     tools/md-recordings.sh --help
#     tools/md-recordings.sh scan   --root <dir> [--app <json|dir>]...
#     tools/md-recordings.sh backup --root <dir> [--dest <dir>]
#     tools/md-recordings.sh verify --dest <dir>
#     tools/md-recordings.sh --self-test        # prueba determinista local
#
# CÓDIGOS DE SALIDA
#     0 = sin problemas (puede haber huérfanos o duplicados: son datos)
#     1 = con ausentes / sin audio / corruptos (o falló el backup)
#     2 = uso incorrecto
#
# COMPILACIÓN
#     Si no hay binario, se compila el de la lib (cargo, Rust puro).  Se usa
#     CARGO_TARGET_DIR local si no viene definido: compilar sobre el NAS es
#     lentísimo y falla.
# ---------------------------------------------------------------------------
set -uo pipefail

RAIZ_REPO="$(cd "$(dirname "$0")/.." && pwd)"
CARGO="${CARGO:-cargo}"
: "${CARGO_TARGET_DIR:=/tmp/md-recordings-target}"
export CARGO_TARGET_DIR

# Binario ya compilado (release > debug > no existe).
buscar_binario() {
    for cand in \
        "$CARGO_TARGET_DIR/release/md-recordings" \
        "$CARGO_TARGET_DIR/debug/md-recordings" \
        "$RAIZ_REPO/rust-core/target/release/md-recordings" \
        "$RAIZ_REPO/rust-core/target/debug/md-recordings"
    do
        [ -x "$cand" ] && { echo "$cand"; return 0; }
    done
    return 1
}

compilar() {
    echo "==> compilando md-recordings (Rust puro, puede tardar)..." >&2
    ( cd "$RAIZ_REPO/rust-core" && "$CARGO" build --release --bin md-recordings ) >&2 || {
        echo "ERROR: no se pudo compilar. ¿cargo en el PATH?" >&2
        return 1
    }
}

# --- prueba determinista: árbol temporal, scan + backup x2 + verify ---------
self_test() {
    local tmp rc=0
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/md-recordings-selftest.XXXXXX")" || return 2
    trap 'rm -rf "$tmp"' RETURN

    mkdir -p "$tmp/audio/whatsapp" "$tmp/audio/samsung" "$tmp/dest"
    printf 'audio-de-whatsapp' > "$tmp/audio/whatsapp/call-2024-01-01_10-30-00.wav"
    printf 'audio-de-samsung'    > "$tmp/audio/samsung/0001.amr"

    local bin; bin="$(buscar_binario)" || { echo "ERROR: no hay binario" >&2; return 2; }

    echo "==> scan"
    "$bin" scan --root "$tmp/audio" || rc=1
    echo "==> backup (1a vez)"
    "$bin" backup --root "$tmp/audio" --dest "$tmp/dest" || rc=1
    local m1; m1="$(sha256sum "$tmp/dest/MANIFEST.ndjson" | cut -d' ' -f1)"
    echo "==> backup (2a vez: NO debe copiar nada)"
    "$bin" backup --root "$tmp/audio" --dest "$tmp/dest" || rc=1
    local m2; m2="$(sha256sum "$tmp/dest/MANIFEST.ndjson" | cut -d' ' -f1)"
    echo "==> verify"
    "$bin" verify --dest "$tmp/dest" || rc=1

    if [ "$m1" != "$m2" ]; then
        echo "FALLO: el MANIFEST cambió entre ejecuciones (no es idempotente)" >&2
        rc=1
    else
        echo "OK: el MANIFEST es idéntico en la 2a ejecución (idempotente)"
    fi
    if [ $rc -eq 0 ]; then
        echo "OK: la prueba determinista pasó"
    else
        echo "FALLO: la prueba determinista falló" >&2
    fi
    return $rc
}

uso() {
    sed -n '2,40p' "$0" | sed 's/^# \{0,1\}//'
}

case "${1:-}" in
    -h|--help)
        uso
        exit 0
        ;;
    --self-test)
        BIN="$(buscar_binario)" || { compilar || exit 2; BIN="$(buscar_binario)" || exit 2; }
        self_test
        exit $?
        ;;
esac

[ $# -ge 1 ] || { uso; exit 2; }

BIN="$(buscar_binario)" || { compilar || exit 2; BIN="$(buscar_binario)" || exit 2; }

case "$1" in
    scan|backup|verify)
        shift
        exec "$BIN" "$@"   # el resto de los flags es del binario Rust
        ;;
    *)
        echo "orden desconocida: $1" >&2
        uso >&2
        exit 2
        ;;
esac
