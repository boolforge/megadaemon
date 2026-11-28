#!/bin/sh
# Limpieza de emergencia del "pastel" del movil (MEDIDO 2026-11-28).
# Que se ejecuta COMO ROOT: su -mm -c '...'
# Que hace: 1) mata a los keepers (que relanzan al supervisor en bucle),
# 2) mata a los supervisores NO canonicos (se queda el de la ruta canonica).
# NO toca: cloudflared, watchdog ni daemon (eso lo hace el one-liner).
set -u
SVC=/data/adb/service.d
CANON="$SVC/99-megadaemon.sh"

argv_de() {
  # El argv del proceso, elemento a elemento (NUL -> saltos de linea).
  cat "/proc/$1/cmdline" 2>/dev/null | tr '\000' '\n'
}

# --- 1) los keepers fuera (son los que relanzan al supervisor) ------------
for p in /proc/[0-9]*; do
  pid=${p##*/}
  argv_de "$pid" | grep -qx '97-megadaemon-keeper.sh' || continue
  kill -9 "$pid" 2>/dev/null && echo "keeper $pid muerto"
done

# --- 2) los supervisores NO canonicos fuera -------------------------------
# Canonico = su argv contiene EXACTAMENTE la ruta canonica (elemento a
# elemento): la copia ".bak-50k" tiene la ruta canonica como PREFIJO y NO
# cuenta (si contara, se quedaria el malo).
for p in /proc/[0-9]*; do
  pid=${p##*/}
  argv_de "$pid" | grep -qx '99-megadaemon.sh' || continue
  argv_de "$pid" | grep -qx "$CANON" && continue
  kill -9 "$pid" 2>/dev/null && echo "supervisor NO canonico $pid muerto"
done

echo "--- quedan vivos (los que deben quedar: 1 supervisor) ---"
for p in /proc/[0-9]*; do
  pid=${p##*/}
  a=$(argv_de "$pid" | tr '\n' ' ')
  case "$a" in
    *99-megadaemon.sh*|*97-megadaemon-keeper.sh*|*tunnel-watchdog.sh*|*cloudflared*|*megadaemon-core*)
      echo "$pid $a" ;;
  esac
done
