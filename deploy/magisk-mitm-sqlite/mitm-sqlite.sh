#!/system/bin/sh
# =====================================================================
# mitm-sqlite.sh — MITM REAL de la base de datos de Antigravity.
#
# Qué hace (y por qué cada paso):
#
#   1. ¿QUÉ FICHERO SE SUSTITUYE?
#      La DB de Antigravity:
#        /data/user/0/com.agy/files/.gemini/antigravity-app/conversation_summaries.db
#      o, en la variante con subcarpeta:
#        .../antigravity-app/conversations/conversation_summaries.db
#      El script AUTO-DETECTA cuál de las dos existe (MEDIDO 2026-11-22: el
#      índice de conversaciones está en el nivel superior del dir de la app;
#      `sqlite_mitm.rs` asumía la subcarpeta `conversations/` ⇒ el MITM
#      habría montado un fichero inexistente).
#
#      Se sustituye con un BIND-MOUNT de FICHERO:
#          mount -o bind <served.db> <original.db>
#      ⇒ la app escribe en <served.db> y el fichero ORIGINAL queda intacto
#      en su partición (intacto = recuperable).
#
#   2. ¿CÓMO SE CAPTURAN LAS ESCRITURAS?
#      El bind-mount hace que la RUTA original apunte a la DB servida.  El
#      motor de réplica ya existente (`sqlite_mitm::replicar`, lectura
#      SOLO LECTURA + busy_timeout 2000 ms) lee por ESA ruta ⇒ captura
#      automáticamente lo que la app escribe, sin tocar la original.
#      Para que el lector NO bloquee al escritor, la DB servida se prepara
#      en modo WAL (el mismo modo que usa la app): en WAL el lector y el
#      escritor conviven.
#
#   3. ¿CÓMO SE RESTAURA?
#      `umount <original.db>` ⇒ la ruta vuelve a ser el FICHERO REAL.
#      Los ficheros `-wal`/`-shm` de la original se apartan ANTES de montar
#      (si no, la app intentaría reproducir un WAL viejo sobre la DB
#      servida => "database disk image is malformed") y se dejan COPIADOS
#      en el directorio de respaldo (nunca se destruyen).
#      Tras el `umount`, el daemon re-mergea las filas servidas en la
#      original ⇒ Antigravity no pierde ninguna conversación.
#
# USO:
#   mitm-sqlite.sh activar    [--original RUTA] [--served RUTA] [--estado RUTA]
#   mitm-sqlite.sh desactivar [--original RUTA] [--served RUTA] [--estado RUTA]
#   mitm-sqlite.sh estado     [--original RUTA] [--served RUTA] [--estado RUTA]
#
# Imprime UN objeto JSON por stdout.  Sale con 0 si la acción se completó,
# con 1 si falló (el motivo va en el JSON).
# =====================================================================
set -u

STATE_DIR="${MEGADAEMON_STATE_DIR:-/data/local/tmp/megadaemon/state}"
APP_DIR="${MEGADAEMON_AGY_DIR:-/data/user/0/com.agy/files/.gemini/antigravity-app}"

ORIGINAL=""
SERVED=""
ESTADO=""

ACCION="${1:-estado}"
[ $# -gt 0 ] && shift

while [ $# -gt 0 ]; do
  case "$1" in
    --original) ORIGINAL="${2:-}"; shift 2 ;;
    --served)   SERVED="${2:-}";   shift 2 ;;
    --estado)   ESTADO="${2:-}";   shift 2 ;;
    --original=*) ORIGINAL="${1#*=}"; shift ;;
    --served=*)   SERVED="${1#*=}";   shift ;;
    --estado=*)   ESTADO="${1#*=}";   shift ;;
    *) shift ;;
  esac
done

[ -n "$ESTADO" ] || ESTADO="$STATE_DIR/mitm/estado.json"
[ -n "$SERVED" ] || SERVED="$STATE_DIR/mitm/antigravity_servida.db"

json_esc() {
  # Escapa lo mínimo para un valor JSON (comillas, barras, control).
  printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g' -e 's/\t/\\t/g' | tr -d '\r\n'
}

# ¿Existe ya un punto de montaje en ESTA ruta?
es_montado() {
  awk -v p="$1" '$5==p {f=1} END{exit !f}' /proc/self/mountinfo 2>/dev/null
}

# Detecta la DB real de Antigravity (dos ubicaciones posibles).
detectar_original() {
  if [ -n "$ORIGINAL" ]; then
    printf '%s' "$ORIGINAL"
    return 0
  fi
  for c in \
    "$APP_DIR/conversation_summaries.db" \
    "$APP_DIR/conversations/conversation_summaries.db" \
    "/data/user/0/com.agy/files/.gemini/antigravity-app/conversation_summaries.db" \
    "/data/user/0/com.agy/files/.gemini/antigravity-app/conversations/conversation_summaries.db"
  do
    if [ -f "$c" ]; then printf '%s' "$c"; return 0; fi
  done
  printf '%s' ""
  return 1
}

leer_estado() {
  [ -f "$ESTADO" ] || return 1
  cat "$ESTADO" 2>/dev/null
}

# --- Activar -----------------------------------------------------------------
do_activar() {
  orig="$(detectar_original)"
  if [ -z "$orig" ]; then
    printf '{"ok":false,"accion":"activar","error":"no encuentro la DB de Antigravity (ni %s ni %s/conversations/...)"}\n' \
      "$APP_DIR" "$APP_DIR"
    return 1
  fi
  if [ ! -f "$SERVED" ]; then
    printf '{"ok":false,"accion":"activar","error":"la DB servida no existe: %s (la genera el daemon: sqlite_mitm servir/activar)"}\n' "$SERVED"
    return 1
  fi

  mkdir -p "$(dirname "$SERVED")" 2>/dev/null
  mkdir -p "$(dirname "$ESTADO")" 2>/dev/null
  BACKUP_DIR="$(dirname "$ESTADO")/backup"
  mkdir -p "$BACKUP_DIR" 2>/dev/null

  if es_montado "$orig"; then
    printf '{"ok":true,"accion":"activar","ya_montado":true,"original":"%s","served":"%s","montaje":"mount -o bind %s %s"}\n' \
      "$(json_esc "$orig")" "$(json_esc "$SERVED")" "$(json_esc "$SERVED")" "$(json_esc "$orig")"
    return 0
  fi

  # 1) La app debe poder ESCRIBIR la DB servida => mismo uid/gid y modo.
  own="$(stat -c '%u:%g' "$orig" 2>/dev/null)"
  [ -n "$own" ] && chown "$own" "$SERVED" 2>/dev/null
  perm="$(stat -c '%a' "$orig" 2>/dev/null)"
  [ -n "$perm" ] && chmod "$perm" "$SERVED" 2>/dev/null

  # 2) Apartar el -wal/-shm de la ORIGINAL (COPIADOS, nunca borrados):
  #    si la app los encontrara, intentaria reproducir transactions de la
  #    DB original sobre la DB servida => corrupcion.
  wal="$orig-wal"; shm="$orig-shm"
  bwal="$BACKUP_DIR/$(basename "$orig").wal.bak"
  bshm="$BACKUP_DIR/$(basename "$orig").shm.bak"
  had_wal=0; had_shm=0
  if [ -f "$wal" ]; then
    cp -f "$wal" "$bwal" 2>/dev/null && rm -f "$wal" && had_wal=1
  fi
  if [ -f "$shm" ]; then
    cp -f "$shm" "$bshm" 2>/dev/null && rm -f "$shm" && had_shm=1
  fi

  # 3) El bind-mount.
  if ! mount -o bind "$SERVED" "$orig" 2>/dev/null; then
    # Reintentamos con la forma larga (util-linux/busybox).
    if ! mount --bind "$SERVED" "$orig" 2>/dev/null; then
      # Se restauran los -wal/-shm que apartamos (el MITM no ha empezado).
      [ "$had_wal" = "1" ] && cp -f "$bwal" "$wal" 2>/dev/null
      [ "$had_shm" = "1" ] && cp -f "$bshm" "$shm" 2>/dev/null
      printf '{"ok":false,"accion":"activar","error":"mount -o bind fallo: no se puede montar %s sobre %s","original":"%s","served":"%s"}\n' \
        "$(json_esc "$SERVED")" "$(json_esc "$orig")" "$(json_esc "$orig")" "$(json_esc "$SERVED")"
      return 1
    fi
  fi

  ts="$(date +%s 2>/dev/null)"
  cat > "$ESTADO" <<EOF
{
  "accion": "activar",
  "ts": $ts,
  "original": "$(json_esc "$orig")",
  "served": "$(json_esc "$SERVED")",
  "backup_wal": "$(json_esc "$bwal")",
  "backup_shm": "$(json_esc "$bshm")",
  "had_wal": $had_wal,
  "had_shm": $had_shm,
  "montado": true
}
EOF
  chmod 600 "$ESTADO" 2>/dev/null

  printf '{"ok":true,"accion":"activar","original":"%s","served":"%s","montaje":"mount -o bind %s %s","backup_wal":"%s","had_wal":%s,"had_shm":%s,"ts":%s}\n' \
    "$(json_esc "$orig")" "$(json_esc "$SERVED")" "$(json_esc "$SERVED")" "$(json_esc "$orig")" \
    "$(json_esc "$bwal")" "$had_wal" "$had_shm" "$ts"
  return 0
}

# --- Desactivar --------------------------------------------------------------
do_desactivar() {
  orig="$(detectar_original)"
  if [ -z "$orig" ]; then
    printf '{"ok":false,"accion":"desactivar","error":"no encuentro la DB de Antigravity"}\n'
    return 1
  fi

  montado=0
  if es_montado "$orig"; then
    if umount "$orig" 2>/dev/null || umount -l "$orig" 2>/dev/null; then
      montado=1
    fi
  fi

  # El -wal/-shm apartados NO se restauran: la DB servida ya se ha
  # re-mergeado en la original y reproducir un WAL viejo sobre ella seria
  # corromperla.  Las copias de respaldo se conservan para forense.
  ts="$(date +%s 2>/dev/null)"
  cat > "$ESTADO" <<EOF
{
  "accion": "desactivar",
  "ts": $ts,
  "original": "$(json_esc "$orig")",
  "served": "$(json_esc "$SERVED")",
  "montado": false,
  "desmontado": $montado
}
EOF
  chmod 600 "$ESTADO" 2>/dev/null

  printf '{"ok":true,"accion":"desactivar","original":"%s","desmontado":%s,"nota":"la DB servida se re-mergea en la original desde el daemon; los -wal/-shm apartados quedan como copia en el directorio de respaldo"}\n' \
    "$(json_esc "$orig")" "$montado"
  return 0
}

# --- Estado ------------------------------------------------------------------
do_estado() {
  orig="$(detectar_original)"
  montado=0
  [ -n "$orig" ] && es_montado "$orig" && montado=1
  st="$(leer_estado)"
  printf '{"ok":true,"accion":"estado","original":"%s","served":"%s","montado":%s,"served_existe":%s,"estado_previo":%s}\n' \
    "$(json_esc "$orig")" "$(json_esc "$SERVED")" "$montado" \
    "$([ -f "$SERVED" ] && echo true || echo false)" \
    "$(json_esc "${st:-}")"
  return 0
}

case "$ACCION" in
  activar|activate|on)    do_activar ;;
  desactivar|deactivate|off) do_desactivar ;;
  estado|status)            do_estado ;;
  *)
    printf '{"ok":false,"error":"accion desconocida: %s (usa activar|desactivar|estado)"}\n' "$ACCION"
    exit 1 ;;
esac
