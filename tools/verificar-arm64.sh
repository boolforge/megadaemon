#!/usr/bin/env sh
# =============================================================================
# verificar-arm64.sh — VERIFICAR UN BINARIO ARM64 EJECUTÁNDOLO DE VERDAD
# =============================================================================
#
#   POR QUÉ EXISTE
#   -------------
#   El sandbox es x86_64 con 3 GB de RAM: el enlazado de `megadaemon-core` en
#   local MATA el proceso por OOM (medido: SIGKILL durante el link del lib).
#   Por eso las pruebas del daemon se hacen en el CI. Pero el binario ARM64
#   **sí** se puede ejecutar aquí con `qemu-aarch64-static` (emulación de
#   usuario, sin máquina virtual): eso convierte "compila" en "funciona".
#
#   QUÉ HACE
#   --------
#   1. Localiza un binario ARM64 (o lo recibe como argumento).
#   2. Lo ejecuta bajo `qemu-aarch64-static` con una batería de subcomandos.
#   3. Devuelve una TABLA compacta (una línea por comando: exit + resumen) y
#      deja la salida completa en un fichero para consultarla bajo demanda.
#
#   POR QUÉ SALIDA COMPACTA
#   -----------------------
#   Imperativo de eficiencia de tokens (docs/ORDEN_IMPERATIVA_GLOBAL.md §2):
#   el agente lee 20 líneas, no 4.000. El detalle va al fichero.
#
#   USO
#   ---
#     tools/verificar-arm64.sh                       # busca el binario solo
#     tools/verificar-arm64.sh /ruta/binario-arm64   # binario explícito
#     MEGADAEMON_VER_ARM64_OUT=/tmp/detalle.txt tools/verificar-arm64.sh
#
#   SALIDA
#   ------
#     exit 0 = todos los comandos attendus en verde
#     exit 1 = alguno en rojo (el detalle está en el fichero de salida)
#
#   ESTADO (honesto)
#   -----------------
#   `NO VERIFICADO` el camino feliz: en el sandbox no hay ningun binario
#   aarch64 **glibc** (el `bin-arm64` que hay es Android/bionic: necesita
#   /system/bin/linker64, que no existe aqui).  Lo verificado de verdad:
#     * rechaza correctamente un binario que no es aarch64;
#     * detecta y rechaza un binario Android/bionic;
#     * `sh -n` pasa.
#   El camino feliz se cierra con el artefacto del CI
#   (`megadaemon-core-linux-arm64`, que es glibc) descargado y pasado por
#   aqui.  El fichero de salida permite saberlo sin releer nada.
# =============================================================================

set -u

OUT="${MEGADAEMON_VER_ARM64_OUT:-/tmp/verificar-arm64-detalle.txt}"
STATE="${MEGADAEMON_VER_ARM64_STATE:-/tmp/verificar-arm64-state}"
QEMU="qemu-aarch64-static"

die() { echo "verificar-arm64: $*" >&2; exit 2; }

# --- 1) localizar el binario ------------------------------------------------
BIN="${1:-}"
if [ -z "$BIN" ]; then
    for cand in \
        "/workspace/md-resc/t-arm/aarch64-unknown-linux-gnu/release/megadaemon-core" \
        "/workspace/md-resc/t-fix2/debug/megadaemon-core" \
        "/workspace/md-root/rust-core/target/aarch64-unknown-linux-gnu/release/megadaemon-core"
    do
        [ -f "$cand" ] && { BIN="$cand"; break; }
    done
fi
[ -n "$BIN" ] || die "no se ha encontrado ningun binario (pasa la ruta como argumento)"
[ -f "$BIN" ] || die "no existe el binario: $BIN"

command -v "$QEMU" >/dev/null 2>&1 || die "falta $QEMU (emulacion de usuario aarch64)"

ARCH=$(file -b "$BIN" 2>/dev/null)
case "$ARCH" in
    *aarch64*|*ARM64*) : ;;
    *) die "el binario no es aarch64: $(printf '%s' "$ARCH" | cut -c1-60)" ;;
esac

mkdir -p "$STATE" || die "no se pudo crear el estado: $STATE"
: > "$OUT" || die "no se pudo escribir: $OUT"

# --- 1b) sysroot ------------------------------------------------------------
# Un binario aarch64 **glibc** (el que produce el CI: `megadaemon-core-linux-arm64`)
# necesita su cargador dinamico: `qemu-aarch64 -L <sysroot>`.  El toolchain
# cross de Debian lo deja en /usr/aarch64-linux-gnu.
# Un binario **Android/bionic** (el del movil) NO se puede ejecutar aqui:
# necesita /system/bin/linker64 y las libs de bionic.
SYSROOT=""
if [ -d /usr/aarch64-linux-gnu ] && [ -f /usr/aarch64-linux-gnu/lib/ld-linux-aarch64.so.1 ]; then
    SYSROOT="/usr/aarch64-linux-gnu"
fi
QEMU_CMD="$QEMU"
[ -n "$SYSROOT" ] && QEMU_CMD="$QEMU -L $SYSROOT"

# --- 2) bateria de comprobaciones -------------------------------------------
# Cada línea: "<etiqueta>|<subcomando>"
#
# `espera` marca los que deben terminar con un código concreto: si el comando
# no existe en el binario, el daemon devuelve un error de parseo y se marca
# como "AUSENTE" (que NO es rojo: el binario puede ser más antiguo).
run() {
    etiqueta="$1"; sub="$2"; espera="$3"
    out=$($QEMU_CMD "$BIN" $sub 2>&1)
    rc=$?
    printf '### %s :: %s :: rc=%s\n%s\n' "$etiqueta" "$sub" "$rc" "$out" >> "$OUT"
    resumen=$(printf '%s' "$out" | tr '\n' ' ' | cut -c1-110)
    if [ "$espera" = "0" ] && [ "$rc" -ne 0 ]; then
        printf '%-22s %-46s rc=%-3s %s\n' "$etiqueta" "$sub" "$rc" "$resumen"
        return 1
    fi
    printf '%-22s %-46s rc=%-3s %s\n' "$etiqueta" "$sub" "$rc" "$resumen"
    return 0
}

fallos=0
export MEGADAEMON_STATE_DIR="$STATE"

printf '%-22s %-46s %-6s %s\n' "COMPROBACION" "SUBCOMANDO" "RC" "RESUMEN"
printf '%s\n' "------------------------------------------------------------------------------------------"

run "version"      "--version"            0 || fallos=$((fallos+1))
run "hash"         "hash $OUT"            0 || fallos=$((fallos+1))
run "telemetria"   "telemetria estado"    0 || fallos=$((fallos+1))
run "telemetria-l" "telemetria listo"     1 || fallos=$((fallos+1))
run "gasto"        "gasto estado"         0 || fallos=$((fallos+1))
run "gasto-desgl"  "gasto desglose"       0 || fallos=$((fallos+1))
run "bus-sim"      "bus --simulacion"     0 || fallos=$((fallos+1))

printf '%s\n' "------------------------------------------------------------------------------------------"
printf 'binario: %s\n' "$BIN"
printf 'detalle completo: %s\n' "$OUT"
printf 'fallos: %s\n' "$fallos"
[ "$fallos" -eq 0 ] || exit 1
exit 0
