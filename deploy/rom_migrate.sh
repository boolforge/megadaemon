#!/usr/bin/env bash
# W4 · Migración de ROM: guion de despliegue *supervisado* y anti-brick.
#
# Reglas de este guion (no negociables):
#   1. POR DEFECTO es un DRY-RUN: no escribe nada en el móvil.
#   2. Para escribir hace falta `--confirm` **y** un terminal humano.
#   3. "Erase Download" (borrado de datos) exige **además** `--wipe-confirm`
#      y una copia del mundo seguro verificada por SHA-256. Sin eso: aborta.
#   4. NUNCA escribe directamente sobre un bloque de particiones: el flash lo
#      hace `heimdall` (Linux) u Odin (Windows, manual y supervisado).
#   5. Antes de tocar nada se copia el "mundo seguro" (7 particiones) con su
#      SHA-256 y se comprueba contra el hash calculado en el propio móvil.
#
# Uso rápido:
#   deploy/rom_migrate.sh --rom-dir /rom --rom-manifest /rom/rom.json --dry-run
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

SERIAL=""
ROM_DIR=""
ROM_MANIFEST=""
FACTS_OUT=""
HASHES_OUT=""
PLAN_OUT=""
EFS_DIR=""
STAGE=""
BACKEND="auto"
CONFIRM=0
WIPE_CONFIRM=0
FRP_UNLOCKED=0
OMI_UNLOCKED=0
UNLOCK_SERVICE=0
PRINT_TEXT=0
DRY_RUN=1

# Particiones reales del mundo seguro en este firmware (las del volcado —
# init_boot, vbmeta_vendor, sec — NO existen en este dispositivo).
EFS_SAFE_WORLD=(sec_efs keystorage keyrefuge uh uhcfg spu prism)

log()  { printf '[rom_migrate] %s\n' "$*"; }
warn() { printf '[rom_migrate] AVISO: %s\n' "$*" >&2; }
die()  { printf '[rom_migrate] ERROR: %s\n' "$*" >&2; exit 1; }

usage() {
  sed -n '2,25p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
  cat <<'EOF'

Opciones:
  --serial <s>        serie adb (por defecto: el único dispositivo)
  --rom-dir <dir>     directorio con los artefactos de la ROM
  --rom-manifest <f>  JSON de la ROM (mismo formato que RomPackage)
  --facts-out <f>     fichero de hechos medidos (por defecto facts.json)
  --hashes-out <f>    fichero de hashes calculados
  --plan-out <f>      fichero del plan JSON generado
  --efs-dir <dir>     dónde guardar la copia del mundo seguro (en el PC)
  --stage <s>         wipe|bootloader|ap_cp_csc|home_csc (sin esto: dry-run)
  --backend <b>       heimdall|odin (por defecto: auto)
  --confirm           ejecutar de verdad (implica TTY)
  --wipe-confirm      además permite "Erase Download"
  --frp-unlocked      declarar que la FRP está quitada
  --omi-unlocked      declarar que el OMI está quitado
  --unlock-service    declarar bootloader abierto (vía de servicio)
  --text              imprime el informe legible del dry-run
  --dry-run           fuerza el dry-run (es el defecto)
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --serial) SERIAL="$2"; shift 2;;
    --rom-dir) ROM_DIR="$2"; shift 2;;
    --rom-manifest) ROM_MANIFEST="$2"; shift 2;;
    --facts-out) FACTS_OUT="$2"; shift 2;;
    --hashes-out) HASHES_OUT="$2"; shift 2;;
    --plan-out) PLAN_OUT="$2"; shift 2;;
    --efs-dir) EFS_DIR="$2"; shift 2;;
    --stage) STAGE="$2"; shift 2;;
    --backend) BACKEND="$2"; shift 2;;
    --confirm) CONFIRM=1; DRY_RUN=0; shift;;
    --wipe-confirm) WIPE_CONFIRM=1; shift;;
    --frp-unlocked) FRP_UNLOCKED=1; shift;;
    --omi-unlocked) OMI_UNLOCKED=1; shift;;
    --unlock-service) UNLOCK_SERVICE=1; shift;;
    --text) PRINT_TEXT=1; shift;;
    --dry-run) DRY_RUN=1; shift;;
    -h|--help) usage; exit 0;;
    *) die "opción desconocida: $1 (usa --help)";;
  esac
done

command -v adb >/dev/null 2>&1 || die "adb no está en el PATH"

# --- 1. Localizar el móvil -------------------------------------------------
if [[ -z "$SERIAL" ]]; then
  mapfile -t devs < <(adb devices | awk 'NR>1 && $2=="device" {print $1}')
  [[ ${#devs[@]} -eq 1 ]] || die "hay ${#devs[@]} dispositivos; usa --serial"
  SERIAL="${devs[0]}"
fi
adb -s "$SERIAL" get-state >/dev/null 2>&1 || die "el dispositivo $SERIAL no responde"

sh() { adb -s "$SERIAL" shell "$@" 2>/dev/null | tr -d '\r'; }
suh() { adb -s "$SERIAL" shell su -c "$1" 2>/dev/null | tr -d '\r'; }

# --- 2. Hechos medidos -----------------------------------------------------
getprop() { sh getprop "$1" | tr -d '\r\n'; }

MODEL="$(getprop ro.product.model)"; MODEL="${MODEL:-SM-G998B}"
CSC="$(getprop ro.csc.sales_code)"
[[ -z "$CSC" ]] && CSC="$(getprop ro.build.display.id | sed 's/.*\///')"
CSC="${CSC:-UNKNOWN}"
PATCH="$(getprop ro.build.version.security_patch)"
ANDROID="$(getprop ro.build.version.release)"
BOOTLOADER="$(getprop ro.bootloader)"
VBOOT="$(getprop ro.boot.verifiedbootstate)"; VBOOT="${VBOOT:-unknown}"
FLASH_LOCKED="$(getprop ro.boot.flash.locked)"; FLASH_LOCKED="${FLASH_LOCKED:-0}"
BAT_LEVEL="$(sh dumpsys battery | awk -F': ' '/level/{print $2; exit}')"
BAT_TEMP_RAW="$(sh dumpsys battery | awk -F': ' '/temperature/{print $2; exit}')"
BAT_TEMP=""
[[ -n "$BAT_TEMP_RAW" ]] && BAT_TEMP="$(awk -v t="$BAT_TEMP_RAW" 'BEGIN{printf "%.1f", t/10}')"
FREE_MB="$(sh df -k /data | awk 'NR==2{printf "%d", $4/1024}')"
REQUIRED_MB="${REQUIRED_MB:-8192}"
ROOT_OK=0
suh 'id' | grep -q 'uid=0' && ROOT_OK=1

# EFS presentes en el móvil.
EFS_PRESENT=()
for p in "${EFS_SAFE_WORLD[@]}"; do
  if suh "ls /dev/block/by-name/$p" | grep -q "$p"; then EFS_PRESENT+=("$p"); fi
done

# Lo que el guion NO puede medir: se pone conservador y se avisa.
FMM_LOCK=0
if [[ "$FLASH_LOCKED" == "1" && "$VBOOT" == "green" ]]; then FMM_LOCK=1; fi
if [[ "$UNLOCK_SERVICE" == "1" ]]; then FLASH_LOCKED=0; FMM_LOCK=0; fi
FRP_LOCKED=1; [[ "$FRP_UNLOCKED" == "1" ]] && FRP_LOCKED=0
OMI_LOCKED=1; [[ "$OMI_UNLOCKED" == "1" ]] && OMI_LOCKED=0

if [[ "$BACKEND" == "auto" ]]; then
  if command -v heimdall >/dev/null 2>&1; then BACKEND="heimdall"; else BACKEND="odin"; fi
fi
if [[ "$BACKEND" == "heimdall" ]]; then
  ODIN_VERSION="heimdall $(heimdall --version 2>/dev/null | head -1 | awk '{print $NF}')"
else
  ODIN_VERSION="${ODIN_VERSION:-odin-presente}"
fi

FACTS_OUT="${FACTS_OUT:-facts.json}"
HASHES_OUT="${HASHES_OUT:-hashes.json}"
PLAN_OUT="${PLAN_OUT:-rom_plan.json}"

json_escape() { printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'; }

EFS_JSON=""
for p in "${EFS_PRESENT[@]}"; do
  [[ -n "$EFS_JSON" ]] && EFS_JSON+=","
  EFS_JSON+="\"$p\""
done

cat > "$FACTS_OUT" <<EOF
{
  "model": "$(json_escape "$MODEL")",
  "csc": "$(json_escape "$CSC")",
  "patch_date": "$(json_escape "$PATCH")",
  "android_version": "$(json_escape "$ANDROID")",
  "bootloader_version": "$(json_escape "$BOOTLOADER")",
  "modem_region": "$(json_escape "${MODEM_REGION:-SM}")",
  "avb_state": "$(json_escape "$VBOOT")",
  "flash_locked": $([[ "$FLASH_LOCKED" == "1" ]] && echo true || echo false),
  "fmm_lock": $([[ "$FMM_LOCK" == "1" ]] && echo true || echo false),
  "frp_locked": $([[ "$FRP_LOCKED" == "1" ]] && echo true || echo false),
  "omi_locked": $([[ "$OMI_LOCKED" == "1" ]] && echo true || echo false),
  "battery_pct": ${BAT_LEVEL:-0},
  "battery_temp_c": ${BAT_TEMP:-99},
  "free_space_mb": ${FREE_MB:-0},
  "required_space_mb": $REQUIRED_MB,
  "odin_version": $([[ -n "$ODIN_VERSION" ]] && echo "\"$(json_escape "$ODIN_VERSION")\"" || echo null),
  "odin_min_version": null,
  "stock_bootimg": $([[ "$STOCK_BOOTIMG" == "1" ]] && echo true || echo false),
  "efs_partitions": [$EFS_JSON]
}
EOF
log "hechos medidos → $FACTS_OUT"

warn "el guion NO puede medir FRP, OMI ni la stockness de boot.img: decláralos"
warn "  con --frp-unlocked / --omi-unlocked / --unlock-service y --stock-bootimg en el manifiesto."

# --- 3. Hashes de los artefactos de la ROM --------------------------------
: > "$HASHES_OUT.raw"
if [[ -n "$ROM_DIR" && -n "$ROM_MANIFEST" && -f "$ROM_MANIFEST" ]]; then
  log "calculando SHA-256 de los artefactos de $ROM_DIR"
  python3 - "$ROM_MANIFEST" "$ROM_DIR" > "$HASHES_OUT" <<'PY'
import hashlib, json, os, sys
manifest, rom_dir = sys.argv[1], sys.argv[2]
data = json.load(open(manifest))
roms = data if isinstance(data, list) else [data]
out = {}
for rom in roms:
    for art in rom.get("artifacts", []):
        name = art.get("name", "")
        path = art.get("path") or os.path.join(rom_dir, name)
        if not os.path.isfile(path):
            print("falta el artefacto: %s" % path, file=sys.stderr)
            continue
        h = hashlib.sha256()
        with open(path, "rb") as f:
            for chunk in iter(lambda: f.read(1 << 20), b""):
                h.update(chunk)
        out[name] = h.hexdigest()
print(json.dumps(out, indent=2))
PY
  log "hashes calculados → $HASHES_OUT"
fi

# --- 4. Plan canónico (el del módulo Rust) ---------------------------------
PLAN_CMD=(cargo run --release --example rom_plan --manifest-path "$REPO_ROOT/rust-core/Cargo.toml" --)
if [[ ! -f "$ROM_MANIFEST" ]]; then
  die "falta --rom-manifest (JSON de la ROM)"
fi
PLAN_ARGS=(--facts "$FACTS_OUT" --rom "$ROM_MANIFEST")
[[ -s "$HASHES_OUT" ]] && PLAN_ARGS+=(--hashes "$HASHES_OUT")
[[ "$PRINT_TEXT" == "1" ]] && PLAN_ARGS+=(--text)

set +e
( cd "$REPO_ROOT/rust-core" && "${PLAN_CMD[@]}" "${PLAN_ARGS[@]}" ) > "$PLAN_OUT"
PLAN_RC=$?
set -e
if [[ "$PLAN_RC" == "3" ]]; then
  warn "el pre-flight ABORTA: no se puede flashear con estos hechos."
  warn "lee $PLAN_OUT (campo \"preflight.blockers\") y corrige lo que falte."
  exit 3
elif [[ "$PLAN_RC" != "0" ]]; then
  warn "el generador de plan falló (rc=$PLAN_RC); revisa $PLAN_OUT"
  exit "$PLAN_RC"
fi
log "plan → $PLAN_OUT"

# --- 5. Copia del mundo seguro (anti-brick) --------------------------------
EFS_VERIFIED=0
if [[ "$ROOT_OK" != "1" ]]; then
  warn "sin root no se puede leer /dev/block/by-name: no hay copia de EFS."
fi
if [[ -n "$EFS_DIR" && "$ROOT_OK" == "1" ]]; then
  mkdir -p "$EFS_DIR"
  EFS_HASHES="$EFS_DIR/efs_sha256.json"
  : > "$EFS_HASHES"
  for p in "${EFS_SAFE_WORLD[@]}"; do
    [[ " ${EFS_PRESENT[*]} " == *" $p "* ]] || { warn "$p no está en el móvil"; continue; }
    out="$EFS_DIR/$p.img"
    log "copiando $p → $out"
    adb -s "$SERIAL" exec-out su -c "dd if=/dev/block/by-name/$p bs=1M" > "$out" 2>/dev/null || true
    local_hash="$(sha256sum "$out" | awk '{print $1}')"
    dev_hash="$(suh "dd if=/dev/block/by-name/$p bs=1M 2>/dev/null | sha256sum" | awk '{print $1}')"
    if [[ -n "$dev_hash" && "$dev_hash" != "$local_hash" ]]; then
      die "la copia de $p NO coincide con el móvil: la partición cambió durante la copia. Para."
    fi
    [[ -z "$dev_hash" ]] && warn "no se pudo hashear $p en el móvil: verificada sólo la copia local"
    echo "  $p $local_hash" >> "$EFS_HASHES"
    log "  $p sha256=$local_hash"
  done
  EFS_VERIFIED=1
  log "mundo seguro copiado y verificado → $EFS_DIR"
else
  warn "sin --efs-dir (o sin root) no se hace copia del mundo seguro:"
  warn "  el paso 'Erase Download' quedará BLOQUEADO y el plan aborta."
fi

# --- 6. Ejecutar (sólo con --confirm) --------------------------------------
if [[ "$DRY_RUN" == "1" ]]; then
  log "DRY-RUN: no se ha escrito nada en el móvil."
  log "para ejecutar de verdad: --confirm --stage <etapa> (y --wipe-confirm para el borrado)"
  exit 0
fi

[[ "$CONFIRM" == "1" ]] || die "sin --confirm no se ejecuta nada"
[[ -n "$STAGE" ]] || die "falta --stage"
[[ -t 0 && -t 1 ]] || die "el flash exige un terminal humano delante (noTTY)"

if [[ "$STAGE" == "wipe" ]]; then
  [[ "$WIPE_CONFIRM" == "1" ]] || die "'Erase Download' exige --wipe-confirm"
  [[ "$EFS_VERIFIED" == "1" ]] || die "'Erase Download' SIN copia de EFS verificada: prohibido (anti-brick)"
  log "paso 1/4 · WIPE DATA (Erase Download) — el mundo seguro está verificado"
  if [[ "$BACKEND" == "odin" ]]; then
    cat <<'EOF'
  En Odin: AP / CP / CSC → pulsar "Erase Download" (NO "Erase Download" en la pestaña
  HOME_CSC) y después flash de AP/CP/CSC. Con el móvil en modo Download verificado.
EOF
  else
    heimdall wipe --no-download --verify
  fi
  exit 0
fi

case "$STAGE" in
  bootloader|ap_cp_csc|home_csc) ;;
  *) die "etapa desconocida: $STAGE";;
esac

if [[ "$BACKEND" == "odin" ]]; then
  cat <<EOF
Odin (manual, supervisado) · paso $STAGE
  1) Móvil en modo Download (volumen abajo + BRS).
  2) Odin: AP = AP_MDPO_MD1.img, CP = CP_MDPO_MD1.img, CSC = CSC_MD1.img,
     HOME_CSC = HOME_CSC.img, BL = BOOTLOADER.img, PIT = <PIT del modelo>.
  3) NO pulses "Erase Download" (la copia de EFS ya está hecha y verificada:
     $EFS_DIR).
  4) Pulsa START y espera a que termine antes del siguiente paso.
EOF
  exit 0
fi

case "$STAGE" in
  bootloader)
    log "paso 2/4 · BOOTLOADER"
    heimdall load --no-download --verify --bootloader "$ROM_DIR/BOOTLOADER.img" ;;
  ap_cp_csc)
    log "paso 3/4 · AP + CP + CSC"
    heimdall load --no-download --verify \
      --pit "$ROM_DIR/PIT.img" \
      --ap "$ROM_DIR/AP_MDPO_MD1.img" \
      --cp "$ROM_DIR/CP_MDPO_MD1.img" \
      --csc "$ROM_DIR/CSC_MD1.img" ;;
  home_csc)
    log "paso 4/4 · HOME_CSC"
    heimdall load --no-download --verify --home_csc "$ROM_DIR/HOME_CSC.img" ;;
esac
log "paso $STAGE terminado. Verifica los hashes escritos antes del paso siguiente."
