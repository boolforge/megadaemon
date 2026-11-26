#!/usr/bin/env sh
# =============================================================================
# build-mddoctor.sh — compila el núcleo forense `mddoctor` (C)
# =============================================================================
#
#   POR QUÉ UN BINARIO EN C Y NO UN MÓDULO RUST
#   -------------------------------------------
#   1. COMPILA SIEMPRE: `gcc -O2` tarda menos de un segundo y usa ~10 MB de
#      RAM.  En este sandbox (2 CPU, 3 GB sin swap) el enlazado del crate de
#      Rust de 160 MB MUERE por OOM (SIGKILL medido).  Un diagnóstico tiene
#      que poder construirse donde no hay RAM.
#   2. ES DIMINUTO: ~26 KB.  Se sube al móvil con un `cat` y arranca en
#      cualquier Android/Termux, con o sin root.
#   3. NO DEPENDE DE MEGADAEMON: justamente tiene que funcionar cuando el
#      daemon está roto, que es cuando hace falta.
#
#   USO
#     tools/build-mddoctor.sh              # binario nativo (host)
#     tools/build-mddoctor.sh aarch64      # ARM64 (Android o Linux)
#
#   Para Android/ARM64 usa el clang del NDK si está; si no, el gcc cross.
# =============================================================================

set -eu
RAIZ="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
SRC="$RAIZ/tools/mddoctor.c"
DEST="${1:-}"

[ -f "$SRC" ] || { echo "build-mddoctor: no existe $SRC" >&2; exit 1; }

compilar_host() {
    cc="${CC:-gcc}"
    command -v "$cc" >/dev/null 2>&1 || { echo "build-mddoctor: falta $cc" >&2; exit 1; }
    out="$RAIZ/tools/mddoctor"
    "$cc" -O2 -Wall -o "$out" "$SRC"
    chmod +x "$out"
    echo "  $out ($(wc -c < "$out") B)"
}

compilar_aarch64() {
    out="$RAIZ/tools/mddoctor-arm64"
    # 1) clang del NDK (es el que usa el CI)
    ndk="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
    if [ -n "$ndk" ] && [ -x "$ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android21-clang" ]; then
        "$ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android21-clang" \
            -O2 -Wall -o "$out" "$SRC"
        chmod +x "$out"
        echo "  $out ($(wc -c < "$out") B, NDK)"
        return 0
    fi
    # 2) gcc cross de Debian
    if command -v aarch64-linux-gnu-gcc >/dev/null 2>&1; then
        aarch64-linux-gnu-gcc -O2 -Wall -static -o "$out" "$SRC"
        chmod +x "$out"
        echo "  $out ($(wc -c < "$out") B, gcc cross)"
        return 0
    fi
    echo "build-mddoctor: no hay NDK ni aarch64-linux-gnu-gcc" >&2
    return 1
}

case "${DEST:-host}" in
    aarch64|arm64) compilar_aarch64 ;;
    *)             compilar_host ;;
esac
