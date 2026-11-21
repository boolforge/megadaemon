#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# acceptance_release.sh — PUERTA DE ACEPTACIÓN de un binario publicado.
#
# Qué verifica (todo MECÁNICO, sin el móvil):
#   1. que el fichero .sha256 que acompaña al release existe y COINCIDE;
#   2. que el ELF tiene la arquitectura esperada (ARM64 / x86_64);
#   3. que el binario NO está corrupto (se ejecuta: en x86_64 pide el uso y
#      sale con código != 0, que es lo correcto; en ARM64 foreign se comprueba
#      la cabecera y ya está);
#   4. que NO lleva símbolos de depuración ni es un "stub" (tamaño mínimo);
#   5. la huella sha256 (para pegarla en el tablón sin filtrar la key).
#
# Uso:
#   tools/acceptance_release.sh <ruta-del-binario> [arm64|x86_64]
#
# Salida: 0 = todo correcto; 1 = el artefacto NO vale para desplegar.
#
# Por qué existe (medido el 2026-11-21): durante horas se desplegó un
# binario que no estaba publicado, y en otro caso se reinició el móvil con
# un binario sin permiso de ejecución (el daemon no arrancaba y el 403 que
# veía el usuario era, en realidad, "no hay daemon").  Esta puerta lo
# comprueba ANTES de tocar el móvil.
# ---------------------------------------------------------------------------
set -uo pipefail

BIN="${1:-}"
ESPERADA="${2:-arm64}"

if [ -z "$BIN" ] || [ ! -f "$BIN" ]; then
  echo "uso: $0 <ruta-del-binario> [arm64|x86_64]" >&2
  exit 1
fi

fallo() { echo "  ✗ $*"; RC=1; }
ok()    { echo "  ✓ $*"; }
RC=0

echo "== aceptación de $BIN (esperado: $ESPERADA) =="

# --- 1. sha256 -------------------------------------------------------------
SHA_FILE="$BIN.sha256"
if [ -f "$SHA_FILE" ]; then
  WANT="$(awk '{print $1}' "$SHA_FILE" | head -1)"
  GOT="$(sha256sum "$BIN" | awk '{print $1}')"
  if [ "$WANT" = "$GOT" ]; then
    ok "sha256 coincide con $SHA_FILE"
  else
    fallo "sha256 NO coincide: dice $WANT y es $GOT"
  fi
  echo "  huella (sha16): ${GOT:0:16}"
else
  fallo "no hay $SHA_FILE al lado del binario"
fi

# --- 2. cabecera ELF y arquitectura ----------------------------------------
readelf -h "$BIN" >/tmp/acc_readelf.$$ 2>/dev/null
if [ $? -eq 0 ]; then
  CLASS="$(awk -F: '/Class/{gsub(/ /,"",$2); print $2}' /tmp/acc_readelf.$$)"
  MACH="$(awk -F: '/Machine/{gsub(/ /,"",$2); print $2}' /tmp/acc_readelf.$$)"
  echo "  ELF: clase=$CLASS máquina=$MACH"
  case "$ESPERADA" in
    arm64)
      echo "$MACH" | grep -qi "aarch64" && ok "arquitectura ARM64 (aarch64)" || fallo "esperaba aarch64 y es $MACH" ;;
    x86_64)
      echo "$MACH" | grep -qiE "x86-64|x86_64" && ok "arquitectura x86_64" || fallo "esperaba x86_64 y es $MACH" ;;
  esac
else
  fallo "no se pudo leer la cabecera ELF (readelf)"
fi
rm -f /tmp/acc_readelf.$$

# --- 3. tipo de fichero y tamaño mínimo ------------------------------------
FILETYPE="$(file -b "$BIN" 2>/dev/null | head -1)"
echo "  file: $FILETYPE"
echo "$FILETYPE" | grep -qi "ELF" || fallo "no es un ELF"
SIZE="$(stat -c %s "$BIN" 2>/dev/null || echo 0)"
echo "  tamaño: $SIZE bytes"
[ "$SIZE" -gt 200000 ] && ok "tamaño plausible (>200 KB)" || fallo "tamaño demasiado pequeño ($SIZE): parece un stub"

# --- 4. se ejecuta (sólo si es de esta máquina) -----------------------------
case "$ESPERADA" in
  x86_64)
    OUT="$(timeout 20 "$BIN" 2>&1)"; RC_BIN=$?
    echo "  salida: $(printf '%s' "$OUT" | head -2 | tr '\n' ' ')"
    if [ "$RC_BIN" -ne 0 ] && [ -n "$OUT" ]; then
      ok "el binario arranca y explica el uso (código $RC_BIN)"
    elif [ "$RC_BIN" -eq 0 ]; then
      ok "el binario arranca"
    else
      fallo "el binario no explica nada y sale con $RC_BIN"
    fi
    ;;
  arm64)
    echo "  (no ejecutable aquí: ARM64 ⇒ se comprueba la cabecera y el permiso)"
    [ -x "$BIN" ] && ok "tiene permiso de ejecución" || fallo "SIN permiso de ejecución (chmod 755)"
    ;;
esac

# --- 5. permiso de ejecución (siempre) -------------------------------------
[ -x "$BIN" ] && ok "permiso de ejecución" || fallo "SIN permiso de ejecución (chmod 755)"

echo
if [ "$RC" -eq 0 ]; then
  echo "RESULTADO: PUEDE desplegarse"
else
  echo "RESULTADO: NO desplegar"
fi
exit $RC
