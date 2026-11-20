#!/usr/bin/env bash
# W3 · Backup ultra completo: subida **por streaming** a Google Drive.
#
# Restricción dura del móvil (S21 Ultra): 5,9 GB libres y un pack de decenas
# de GB ⇒ **no puede existir copia local intermedia**. Este guion:
#
#   1. Calcula el SHA-256 de cada fichero (y de cada trozo) leyéndolo.
#   2. Pipea el rango directamente a `curl` (sesión reanudable de Drive).
#   3. Nunca escribe los datos en disco: sólo el *manifiesto* (metadatos).
#   4. Con `--verify` relee cada trozo desde Drive y compara el SHA-256:
#      es la comprobación de que el backup **está** en el destino.
#
# Uso (desde Termux en el móvil, o desde un PC con adb):
#   deploy/backup_phone.sh --src /data/local/tmp/megadaemon/state \
#       --session 'https://www.googleapis.com/upload/drive/v3/files?...' \
#       --out MegaDaemon/estado --manifest state-manifest.json --dry-run
#
set -euo pipefail

SOURCES=()
SESSION=""
OUT_PREFIX="MegaDaemon/backup"
CHUNK_MIB=8
MANIFEST="backup-manifest.json"
VERIFY=0
DRY_RUN=1
FILE_ID=""
LIST_ONLY=0

log()  { printf '[backup] %s\n' "$*"; }
warn() { printf '[backup] AVISO: %s\n' "$*" >&2; }
die()  { printf '[backup] ERROR: %s\n' "$*" >&2; exit 1; }

usage() {
  cat <<'EOF'
Opciones:
  --src <ruta>        fichero o directorio a respaldar (repetible)
  --session <uri>     URI de sesión reanudable de Drive
  --file-id <id>      id del fichero en Drive (para --verify)
  --out <prefijo>     prefijo remoto (por defecto MegaDaemon/backup)
  --chunk-size <MiB>  tamaño de trozo (por defecto 8)
  --manifest <f>      fichero de manifiesto (metadatos, no datos)
  --verify            relee de Drive y compara los SHA-256
  --list              sólo genera el manifiesto; no sube nada
  --dry-run           plan solamente (es el defecto)
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --src) SOURCES+=("$2"); shift 2;;
    --session) SESSION="$2"; shift 2;;
    --file-id) FILE_ID="$2"; shift 2;;
    --out) OUT_PREFIX="$2"; shift 2;;
    --chunk-size) CHUNK_MIB="$2"; shift 2;;
    --manifest) MANIFEST="$2"; shift 2;;
    --verify) VERIFY=1; shift;;
    --list) LIST_ONLY=1; shift;;
    --dry-run) DRY_RUN=1; shift;;
    -h|--help) usage; exit 0;;
    *) die "opción desconocida: $1";;
  esac
done

[[ ${#SOURCES[@]} -gt 0 ]] || die "hace falta al menos un --src"
command -v sha256sum >/dev/null 2>&1 || die "sha256sum no está disponible"
command -v dd >/dev/null 2>&1 || die "dd no está disponible"

# --- Recorrer las fuentes (ficheros regulares, sin symlinks) ---------------
FILES=()
for s in "${SOURCES[@]}"; do
  if [[ -f "$s" ]]; then
    FILES+=("$s")
  elif [[ -d "$s" ]]; then
    while IFS= read -r f; do
      FILES+=("$f")
    done < <(find "$s" -type f 2>/dev/null | sort)
  else
    warn "no existe: $s"
  fi
done
[[ ${#FILES[@]} -gt 0 ]] || die "no hay ficheros que respaldar"

# --- Manifiesto: inventario + SHA-256 por fichero y por trozo --------------
log "generando manifiesto de ${#FILES[@]} ficheros → $MANIFEST"
python3 - "$MANIFEST" "$CHUNK_MIB" "${FILES[@]}" <<'PY'
import hashlib, json, os, sys

manifest_path, chunk_mib = sys.argv[1], int(sys.argv[2])
files = sys.argv[3:]
chunk = chunk_mib * 1024 * 1024
out = {"chunk_bytes": chunk, "files": []}
total = 0
for path in files:
    size = os.path.getsize(path)
    h = hashlib.sha256()
    chunks = []
    with open(path, "rb") as f:
        idx = 0
        off = 0
        while off < size:
            n = min(chunk, size - off)
            f.seek(off)
            data = f.read(n)
            ch = hashlib.sha256(data).hexdigest()
            h.update(data)
            chunks.append({"index": idx, "offset": off, "len": n, "sha256": ch})
            off += n
            idx += 1
    total += size
    out["files"].append({
        "path": path,
        "size": size,
        "sha256": h.hexdigest(),
        "chunks": chunks,
    })
out["total_bytes"] = total
with open(manifest_path, "w") as f:
    json.dump(out, f, indent=2)
print("[backup] %d ficheros, %d bytes, %d trozos" % (
    len(out["files"]), total, sum(len(f["chunks"]) for f in out["files"])))
PY

if [[ "$LIST_ONLY" == "1" ]]; then
  log "sólo manifiesto (--list): no se sube nada"
  exit 0
fi

if [[ "$DRY_RUN" == "1" || -z "$SESSION" ]]; then
  log "DRY-RUN: no se ha subido nada."
  [[ -z "$SESSION" ]] && warn "sin --session no hay destino: nada que subir."
  exit 0
fi

command -v curl >/dev/null 2>&1 || die "curl no está disponible"

# --- Subida por trozos ------------------------------------------------------
# Se relee cada trozo desde el origen (no hay copia local: el fichero ya
# está en el móvil) para calcular su hash y enviarlo.
uploaded=0
while IFS=$'\t' read -r path offset len idx sha; do
  skip_mib=$((offset / (1024 * 1024)))
  count_mib=$(( (len + 1048575) / 1048576 ))
  total_size="$(stat -c %s "$path")"
  range="bytes $offset-$((offset + len - 1))/$total_size"
  remote_name="$OUT_PREFIX/$(basename "$path")"
  log "subiendo trozo $idx de $path ($len bytes) → $remote_name"
  dd if="$path" bs=1048576 skip="$skip_mib" count="$count_mib" iflag=fullblock 2>/dev/null \
    | curl -sS -X PUT --data-binary @- \
        -H 'Content-Type: application/octet-stream' \
        -H "Content-Range: $range" \
        "$SESSION" >/dev/null
  uploaded=$((uploaded + 1))
done < <(python3 - "$MANIFEST" <<'PY'
import json, sys
m = json.load(open(sys.argv[1]))
for f in m["files"]:
    for c in f["chunks"]:
        print("%s\t%d\t%d\t%d\t%s" % (f["path"], c["offset"], c["len"], c["index"], c["sha256"]))
PY
)
log "subidos $uploaded trozos"

# --- Verificación en el destino --------------------------------------------
if [[ "$VERIFY" == "1" ]]; then
  [[ -n "$FILE_ID" ]] || die "--verify necesita --file-id"
  log "verificando: se relee cada trozo de Drive y se compara el SHA-256"
  bad=0
  total_chunks=0
  while IFS=$'\t' read -r path offset len idx sha; do
    total_chunks=$((total_chunks + 1))
    end=$((offset + len - 1))
    got="$(curl -sS -H "Range: bytes=$offset-$end" \
            "https://www.googleapis.com/drive/v3/files/$FILE_ID?alt=media" \
           | sha256sum | awk '{print $1}')"
    if [[ "$got" != "$sha" ]]; then
      warn "trozo $idx de $path NO casa: $got != $sha"
      bad=$((bad + 1))
    fi
  done < <(python3 - "$MANIFEST" <<'PY'
import json, sys
m = json.load(open(sys.argv[1]))
for f in m["files"]:
    for c in f["chunks"]:
        print("%s\t%d\t%d\t%d\t%s" % (f["path"], c["offset"], c["len"], c["index"], c["sha256"]))
PY
)
  if [[ "$bad" -gt 0 ]]; then
    die "$bad de $total_chunks trozos no verifican en el destino"
  fi
  log "verificados $total_chunks trozos en el destino"
fi
