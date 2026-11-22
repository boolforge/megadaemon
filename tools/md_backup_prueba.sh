#!/bin/sh
# Prueba real del motor de backup nativo de apps (`md-backup`) en el móvil.
#
# Qué hace (todo se mide en el propio móvil):
#   1. estado ANTES: loadavg, RAM y disco (números, no estimas);
#   2. inventario de las apps elegidas (sin escribir nada);
#   3. SUBIDA PIPELINING de verdad (sin `--delete-after-upload`: el
#      contenedor local se necesita para verificar y restaurar después; la
#      limpieza se hace al final, paso 8): `upload --create` recoge y sube a la vez
#      (cada segmento ya cerrado se sube mientras se recoge el siguiente) y
#      mide MiB/s y ETA reales;
#   4. verificación LOCAL (manifiesto + AEAD);
#   5. verificación REMOTA: se baja el contenedor de Drive y se comprueba
#      el SHA-256 del fichero entero y el de CADA trozo;
#   6. restauración a un directorio temporal (nunca sobre datos vivos);
#   7. estado DESPUÉS: loadavg, RAM y disco.
#
# Uso en el móvil:
#   sh /data/local/tmp/megadaemon/md_backup_prueba.sh <binario> [carpeta_drive] [subcarpeta]
#
# La credencial de Drive **nunca** se escribe aquí: se lee del `state_dir`
# (`gdrive_credentials.json`, 0600). La clave del contenedor sale en un
# fichero 0600 **fuera** del contenedor.

set -u

BIN="${1:-/data/local/tmp/megadaemon/md-backup}"
FOLDER="${2:-15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0}"
SUBFOLDER="${3:-P0-apps}"

STATE=/data/local/tmp/megadaemon/state
WORK=/data/local/tmp/megadaemon/backup
KEYS="$STATE/backup_keys"
OUT="$WORK/prueba_p0"
REST="$WORK/restore_prueba"
LOG=/data/local/tmp/megadaemon/md_backup_prueba.log

# 5 apps: tres pequeñas, una mediana (YouTube) y una con bastante dato
# (Mensajes). Medido en el móvil el 2026-11-22:
#   app.lawnchair 3,9 MiB · org.schabi.newpipe 4,8 MiB · com.android.settings 11 KiB
#   com.google.android.youtube 36,9 MiB · com.google.android.apps.messaging 136,9 MiB
PACKAGES="${PACKAGES:-com.android.settings,app.lawnchair,org.schabi.newpipe,com.google.android.youtube,com.google.android.apps.messaging}"

# El móvil estaba con loadavg 31 (medido): el tope de espera por carga se
# sube para que la prueba no se congele ni para siempre.
LOAD_HIGH="${LOAD_HIGH:-40}"

mkdir -p "$WORK" "$KEYS"
: > "$LOG"

say() { echo "=== $* ===" | tee -a "$LOG"; }

run() {
    say "$*"
    "$@" >> "$LOG" 2>&1
    echo "exit=$?" >> "$LOG"
}

estado() {
    echo "fecha      : $(date -Is)"
    echo "loadavg    : $(cat /proc/loadavg)"
    echo "mem libre  : $(awk '/MemAvailable/{print $2" kB"}' /proc/meminfo)"
    echo "disco /data: $(df -k /data | tail -1 | awk '{print $4" kB libres de "$2" kB ("$5" usado)"}')"
    echo "nproc      : $(grep -c ^processor /proc/cpuinfo)"
}

# --- 0. Estado del móvil ANTES (carga, RAM y disco) ------------------------
say "ESTADO ANTES"
{
    estado
    echo "nice del daemon:"
    for p in $(pidof megadaemon-core 2>/dev/null); do
        awk '{print "  pid '"$p"' nice=" $19}' /proc/$p/stat 2>/dev/null
    done
    echo "md-backup en marcha: $(pidof md-backup 2>/dev/null | tr '\n' ' ')"
} >> "$LOG" 2>&1

# --- 1. Inventario (sin escribir nada) -------------------------------------
run "$BIN" inventory --root / --packages "$PACKAGES" --measure

# --- 2. SUBIDA PIPELINING (recoger y subir a la vez) -----------------------
# `upload --create` es el pipelining de verdad: el productor entrega cada
# segmento cerrado y el consumidor lo sube mientras sigue la recogida.
say "SUBIDA PIPELINING (upload --create)"
{
    echo "inicio: $(date -Is)"
    echo "loadavg antes: $(cat /proc/loadavg)"
} >> "$LOG" 2>&1
run "$BIN" upload --out "$OUT" --folder "$FOLDER" --subfolder "$SUBFOLDER" \
    --create --root / --packages "$PACKAGES" \
    --segment-bytes 16777216 --chunk-bytes 1048576 \
    --workers 3 --state-dir "$STATE" \
    --key-out "$KEYS/prueba_p0.keys.json" \
    --nice 15 --ionice 3 --load-high "$LOAD_HIGH" --sleep-ms 120 --batch 64 \
    --name prueba_p0 --verify-download
{
    echo "fin   : $(date -Is)"
    echo "loadavg despues: $(cat /proc/loadavg)"
} >> "$LOG" 2>&1

# --- 3. Verificar LOCAL (manifiesto + AEAD) -------------------------------
run "$BIN" verify "$OUT" --key "$KEYS/prueba_p0.keys.json"

# --- 4. Verificar REMOTO (Drive: tamaño + SHA-256 por trozo) --------------
run "$BIN" fetch --folder "$FOLDER" --name prueba_p0 --dest "$WORK/fetch_p0" \
    --state-dir "$STATE"

# --- 5. Restaurar a un directorio temporal ---------------------------------
run "$BIN" restore "$OUT" --key "$KEYS/prueba_p0.keys.json" --dest "$REST" \
    --confirm --overwrite

# --- 6. Comparar lo restaurado con el original ----------------------------
say "COMPARACION"
{
    for rel in \
        "data/user/0/com.android.settings/shared_prefs/settings.xml" \
        "data/user/0/app.lawnchair/databases" ; do
        if [ -e "$REST/$rel" ]; then
            echo "OK  $rel"
        else
            echo "--  $rel (no estaba en el backup)"
        fi
    done
    echo "ficheros restaurados: $(find "$REST" -type f 2>/dev/null | wc -l)"
    echo "bytes restaurados    : $(du -sk "$REST" 2>/dev/null | cut -f1) kB"
} >> "$LOG" 2>&1

# --- 7. Estado del móvil DESPUÉS ------------------------------------------
say "ESTADO DESPUES"
{
    estado
    echo "md-backup en marcha: $(pidof md-backup 2>/dev/null | tr '\n' ' ')"
} >> "$LOG" 2>&1

# --- 8. Limpieza del contenedor local (el disco del móvil es escaso) ------
say "LIMPIEZA"
{
    echo "contenedor local antes: $(du -sk "$OUT" 2>/dev/null | cut -f1) kB"
    rm -rf "$OUT"
    echo "contenedor local borrado (en Drive sigue)"
    echo "disco /data: $(df -k /data | tail -1 | awk '{print $4" kB libres"}')"
} >> "$LOG" 2>&1

say "FIN (log en $LOG)"
cat "$LOG"
