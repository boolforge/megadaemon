#!/usr/bin/env bash
# =============================================================================
# test-energia.sh — la política de energía NO puede parpadear.
#
# Por qué: MEDIDO (2026-11-27, SM-G998B, Android 14) el brillo del móvil iba
# 247 -> 60 -> 247 cada ~75 s.  No era el sensor de luz ambiente: era la clase
# de energía de deploy/99-megadaemon.sh, que entraba y salía de "thermal"/
# "conserve" según el valor rozase el umbral.  El daemon se ponía el tope de
# 60, guardaba 247 y un minuto después lo restauraba.
#
# Aquí se extrae `energy_classify` del script real y se comprueba que:
#   * ESCALAR es inmediato (batería crítica, temperatura crítica);
#   * RELAJAR exige un margen (histéresis): el caso medido 45 °C -> 44 °C
#     NO puede devolver "normal" (eso era el parpadeo);
#   * con margen, la classe vuelve a "normal" cuando el valor está claro.
#
# Uso: bash tools/test-energia.sh
# =============================================================================
set -u

RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
FALLOS=0

ok()  { printf '  OK    %s\n' "$*"; }
mal() { printf '  FALLO %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

# --- la función REAL del script de servicio (no una copia) ------------------
eval "$(sed -n '/^energy_classify() {/,/^}/p' "$RAIZ/deploy/99-megadaemon.sh")"
if ! command -v energy_classify >/dev/null 2>&1; then
  printf '  FALLO no se pudo extraer energy_classify de deploy/99-megadaemon.sh\n'
  exit 1
fi

# --- los mismos umbrales que el script (deploy/99-megadaemon.sh:1060-1090) --
ENERGY_SEAL_BELOW=20
ENERGY_THERMAL_WARN=45
ENERGY_THERMAL_CRIT=55
ENERGY_THERMAL_HARD=80
ENERGY_RAM_FLOOR_KB=524288
ENERGY_HYST_C=3
ENERGY_HYST_RAM_KB=65536
ENERGY_HYST_PCT=5

# cls pct temp ram charging [clase_anterior]
cls() { energy_classify "$1" "$2" "$3" "$4" "${5:-normal}"; }

# --- comprobaciones ---------------------------------------------------------
printf '\n== 1) escalar es INMEDIATO (sin margen) ==\n'
[ "$(cls 90 60 1000000 Discharging normal)" = "thermal" ] \
  && ok "60 °C => thermal" || mal "60 °C debería ser thermal"
[ "$(cls 90 85 1000000 Discharging normal)" = "seal" ] \
  && ok "85 °C => seal" || mal "85 °C debería ser seal"
[ "$(cls 15 30 1000000 Discharging normal)" = "seal" ] \
  && ok "15 % de batería => seal" || mal "15 % debería ser seal"

printf '\n== 2) el caso MEDIDO: 45 °C -> 44 °C NO puede parpadear ==\n'
# Así se midió el parpadeo: la temperatura rozaba los 45 °C y la clase
# entraba y salía de "thermal", y el brillo iba 247 -> 60 -> 247.
c1=$(cls 90 45 1000000 Discharging normal)
c2=$(cls 90 44 1000000 Discharging "$c1")
[ "$c1" = "thermal" ] && ok "45 °C => thermal" || mal "45 °C debería ser thermal (fue $c1)"
[ "$c2" = "thermal" ] \
  && ok "44 °C tras estar en thermal => sigue thermal (histéresis)" \
  || mal "44 °C tras thermal volvió a $c2: eso es el parpadeo"
c3=$(cls 90 41 1000000 Discharging "$c2")
[ "$c3" = "normal" ] \
  && ok "41 °C (por debajo del margen) => normal: se restaura el brillo" \
  || mal "41 °C debería ser normal (fue $c3)"

printf '\n== 3) la RAM igual: un pico bajo no destapa el brillo ==\n'
c1=$(cls 90 30 500000 Discharging normal)
[ "$c1" = "conserve" ] && ok "RAM 500 MB => conserve" || mal "RAM 500 MB debería ser conserve"
c2=$(cls 90 30 550000 Discharging "$c1")
[ "$c2" = "conserve" ] \
  && ok "RAM 550 MB tras conserve => sigue conserve (margen: hace falta 576 MB)" \
  || mal "RAM 550 MB volvió a $c2: parpadeo de RAM"
c3=$(cls 90 30 700000 Discharging "$c2")
[ "$c3" = "normal" ] && ok "RAM 700 MB => normal" || mal "RAM 700 MB debería ser normal (fue $c3)"

printf '\n== 4) la bateria igual, y cargando no hay clase restrictiva ==\n'
c1=$(cls 30 30 1000000 Discharging normal)
[ "$c1" = "conserve" ] && ok "30 % sin cargar => conserve" || mal "30 % debería ser conserve"
c2=$(cls 37 30 1000000 Discharging "$c1")
[ "$c2" = "conserve" ] && ok "37 % tras conserve => sigue conserve (margen de 5 %)" || mal "37 % volvió a $c2"
c3=$(cls 42 30 1000000 Discharging "$c2")
[ "$c3" = "normal" ] && ok "42 % => normal" || mal "42 % debería ser normal (fue $c3)"
[ "$(cls 30 30 1000000 Charging normal)" = "normal" ] \
  && ok "30 % PERO cargando => normal" || mal "cargando no debería ser restrictiva"

printf '\n== 5) desde "normal" los umbrales son los de siempre (sinceridad) ==\n'
[ "$(cls 90 44 1000000 Discharging normal)" = "normal" ] \
  && ok "44 °C desde normal => normal (no se recorta de más)" \
  || mal "44 °C desde normal no debería recortarse"
[ "$(cls 90 30 600000 Discharging normal)" = "normal" ] \
  && ok "RAM 600 MB desde normal => normal" || mal "RAM 600 MB desde normal no debería recortarse"

printf '\n===== RESULTADO: %d fallo(s) =====\n' "$FALLOS"
[ "$FALLOS" = "0" ] || exit 1
exit 0
