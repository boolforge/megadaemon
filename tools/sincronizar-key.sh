#!/usr/bin/env bash
# Sincroniza la API key del movil con la del PC (flota).
#
# MEDIDO 2026-11-22: la key del PC y la del movil se desincronizaron
# (el supervisor reinicio el daemon con una key antigua) y la flota se
# quedo horas sin poder hablar con el movil: /health seguia respondiendo
# 200 pero /mcp daba 403 "API key invalida".
#
# ESTE SCRIPT SE EJECUTA EN EL MOVIL (Termux o adb shell), NO en el PC:
#   bash sincronizar-key.sh
#
# Que hace:
#   1. busca la key vigente en los state_dir conocidos del movil;
#   2. la deja en /data/local/tmp/megadaemon/state/megadaemon_api.key y en
#      <state_dir de la app>/megadaemon_api.key (0600);
#   3. imprime el fingerprint fnv1a64 (NO la key) para que el PC lo
#      compare con el suyo.
set -uo pipefail

CANDIDATOS=(
  /data/local/tmp/megadaemon/state/megadaemon_api.key
  /data/user/0/com.agy/files/megadaemon/state/megadaemon_api.key
  /data/local/tmp/megadaemon/megadaemon_api.key
)
DST=(
  /data/local/tmp/megadaemon/state
  /data/user/0/com.agy/files/megadaemon/state
)

SRC=""
for f in "${CANDIDATOS[@]}"; do
  [ -s "$f" ] && { SRC="$f"; break; }
done
[ -n "$SRC" ] || { echo "NO encuentro ninguna key en el movil"; exit 1; }

KEY="$(head -1 "$SRC" | tr -d '\r\n')"
[ "${#KEY}" -ge 16 ] || { echo "la key de $SRC parece vacia"; exit 1; }

for d in "${DST[@]}"; do
  mkdir -p "$d" 2>/dev/null || continue
  printf '%s\n' "$KEY" > "$d/megadaemon_api.key" 2>/dev/null && chmod 600 "$d/megadaemon_api.key"
  printf '%s\n' "$KEY" > "$d/megadaemon_api.keys" 2>/dev/null && chmod 600 "$d/megadaemon_api.keys"
  echo "sincronizada -> $d"
done

# fnv1a64 (el mismo que usa api_key::fingerprint)
python3 - "$KEY" <<'PY' 2>/dev/null || echo "fp: (sin python)"
import sys
k = sys.argv[1].encode()
h = 0xcbf29ce484222325
for b in k:
    h ^= b
    h = (h * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
print("fp:", format(h, "016x"))
PY
