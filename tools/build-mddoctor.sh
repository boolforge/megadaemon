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

# FLAGS DE TAMAÑO (MEDIDO 2026-11-27): el binario SIN estos flags pesa
# 57.536 B y CON ellos 43.384 B (-25%), con las MISMAS capacidades y SIN
# perder rapidez ni robustez.
#   -O2 + LTO + --gc-sections  : el codigo que no se usa no se genera
#   -fmerge-all-constants      : una sola copia de cada cadena repetida
#   -fno-plt                   : sin trampolines PLT (arranque mas ligero)
#   -fno-asynchronous-unwind-tables : sin tabla de desenrollado (no hay C++
#                                    que desenrollar: es C puro)
#   -s + objcopy               : sin tabla de simbolos ni .comment
# El codigo tiene ademas los atributos MD_HOT (O3) y MD_COLD (Os) por
# funcion: RAPIDEZ en el camino critico (sha256, HTTP) y TAMANO en las
# funciones voluminosas, que se ejecutan una vez y dependen de E/S.
# MEDIDO: en el banco caliente las diferencias entre -O2/-Os/mixto caen
# DENTRO del ruido del sandbox, asi que se elige la que mas gana en tamaño
# sin tocar la velocidad.  Se conservan el stack protector (robustez), las
# comprobaciones de rango y el enlazado dinamico (el estatico pesa 762 KB:
# 13x mas y con la MISMA RAM).
OPT="-O2 -s -flto -ffunction-sections -fdata-sections -Wl,--gc-sections"
OPT="$OPT -fmerge-all-constants -fno-ident -fno-plt -Wl,--as-needed"
OPT="$OPT -fno-unwind-tables -fno-asynchronous-unwind-tables"
# -z noseparate-code: el enlazador pone .text y .rodata en segmentos de
# pagina distintos y deja ~8 KB de RELLENO entre medias (MEDIDO: 51.576 B
# con, 43.384 B sin).  Se cede aqui y NO en el daemon principal, que si es
# un daemon de red: W^X protege frente a inyeccion de codigo, y este
# binario es una herramienta local que ya corre como root.
OPT="$OPT -Wl,-z,noseparate-code"

recortar() {
    # Quita .comment y .note.gnu.build-id: son ruido para el telefono.
    command -v objcopy >/dev/null 2>&1 || return 0
    objcopy --remove-section=.comment --remove-section=.note.gnu.build-id "$1" 2>/dev/null || true
}
SRC="$RAIZ/tools/mddoctor.c"
DEST="${1:-}"

[ -f "$SRC" ] || { echo "build-mddoctor: no existe $SRC" >&2; exit 1; }

compilar_host() {
    cc="${CC:-gcc}"
    command -v "$cc" >/dev/null 2>&1 || { echo "build-mddoctor: falta $cc" >&2; exit 1; }
    out="$RAIZ/tools/mddoctor"
    "$cc" $OPT -Wall -o "$out" "$SRC"
    recortar "$out"
    chmod +x "$out"
    echo "  $out ($(wc -c < "$out") B)"
}

compilar_aarch64() {
    out="$RAIZ/tools/mddoctor-arm64"
    # 1) clang del NDK (es el que usa el CI)
    ndk="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
    if [ -n "$ndk" ] && [ -x "$ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android21-clang" ]; then
        "$ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android21-clang" \
            $OPT -Wall -o "$out" "$SRC"
        recortar "$out"
        chmod +x "$out"
        echo "  $out ($(wc -c < "$out") B, NDK)"
        return 0
    fi
    # 2) gcc cross de Debian
    if command -v aarch64-linux-gnu-gcc >/dev/null 2>&1; then
        aarch64-linux-gnu-gcc $OPT -Wall -static -o "$out" "$SRC"
        recortar "$out"
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
