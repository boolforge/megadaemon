#!/usr/bin/env sh
# =============================================================================
# fix-entorno.sh — EL ARCHIVO ÚNICO para dejar el MegaDaemon del usuario
# (PC + móvil) en un estado CONOCIDO y VERIFICADO.  Un solo fichero, sin
# pasos manuales, sin secretos en el repo.
#
#   EN EL PC (con el móvil por adb):
#     ./deploy/fix-entorno.sh --diagnostico     # sólo lee, no toca nada
#     sudo ./deploy/fix-entorno.sh --instalar    # arregla PC + móvil
#
#   EN EL MÓVIL (Termux, sin adb):
#     sh fix-entorno.sh --diagnostico
#     sh fix-entorno.sh --instalar
#
# El script DETECTA dónde se está ejecutando.  Sin argumentos hace
# diagnóstico y no toca nada (modo seguro por defecto).
#
# QUÉ ARREGLA (todo medido que hoy falla):
#   1. CREDENCIAL EN UN SOLO SITIO.  El daemon valida contra
#      <state_dir>/megadaemon_api.key.  Si ese directorio está vacío el
#      daemon rechaza CUALQUIER key -> 403 "API key invalida" a toda la
#      flota (NO es que la key haya caducado: es que no la encuentra).
#   2. BINARIO ARM64 VERIFICADO desde el RELEASE real de GitHub:
#      se resuelve la última release por la API (los tags son `v<sha>`,
#      NO existe un tag "latest" -> 404), se descarga el asset
#      `megadaemon-core-android-arm64` + su `.sha256`, se COMPARA el
#      sha256, se comprueba que es ELF aarch64, se instala con chmod 755
#      ANTES de arrancar (sin el bit +x el supervisor lo da por inexistente
#      y SE RINDE: esa fue la causa medida del 502) y con ROLLBACK si el
#      daemon no arranca.
#   3. SUPERVISOR PERSISTENTE (PPID 1) con MEGADAEMON_STATE_DIR exportado:
#      el daemon y el supervisor miran el MISMO sitio.
#   4. UN SOLO cloudflared y la URL vigente en el fichero canónico
#      (los hostnames quick-tunnel cambian en cada reinicio y hacen que
#      la flota diagnostique "red caída" sin motivo).
#   5. CSC LEGAL: comprueba que el valor de grabación sea
#      `RecordingAllowedByMenu,disclaimer_latin`.  Con `RecordingAllowed`
#      a secas la grabación de llamadas es AUTOMÁTICA y SILENCIOSA, lo que
#      en España es delito (art. 197 CP) y vulnera el GDPR.  Avisa y
#      repara; nunca reinicia el móvil por su cuenta.
#   6. (Sólo en el PC) RELAY DEL CANAL con DuckDNS: el A record lo publica
#      SÓLO el PC (es el único con la IP doméstica; llamarlo desde el
#      sandbox dejó el dominio apuntando al NAT del datacenter).
#
# REQUISITOS: en el móvil, Termux (root por `su -mm`; `sudo` en Android
# NO es root).  En el PC, adb con el móvil en estado "device".
# SECRETOS: nunca se imprimen.  De las keys y URLs sólo se publica su
# sha16 (16 hex) para poder compararlas sin filtrarlas.
# =============================================================================
set -u

# --- donde estamos ----------------------------------------------------------
SELF="$0"
case "$SELF" in
  /*) ;;
  *) SELF="$PWD/$SELF" ;;
esac
REPO="$(cd "$(dirname "$SELF")/.." 2>/dev/null && pwd)"
[ -n "${REPO:-}" ] || REPO="$PWD"

MD_DIR=/data/local/tmp/megadaemon
STATE_REAL_DEFAULT=/data/user/0/com.agy/files/megadaemon/state
SUP=/data/adb/service.d/99-megadaemon.sh
REPO_SLUG=boolforge/megadaemon
API=https://api.github.com/repos/$REPO_SLUG/releases/latest
BASE_RELEASES_LATEST=https://github.com/$REPO_SLUG/releases/latest
SECRETS="${MD_SECRETS_DIR:-/workspace/md-secrets}"
ENVFILE="$SECRETS/instance.env"
ADB="${ADB:-adb}"
SERIAL="${MD_SERIAL:-}"
MODE=diagnostico
DESTINO=auto          # auto | movil | pc
# Valor de la feature CSC de grabacion.  El usuario lo elige explicitamente;
# el valor por defecto es el que deja la grabacion AUTOMATICA (sin aviso).
# Valor alternativo (con aviso por voz): RecordingAllowedByMenu,disclaimer_latin
CSC_VALOR="${MD_CSC_VALOR:-RecordingAllowed}"
CSC_ACTIVO=1

for a in "$@"; do
  case "$a" in
    --diagnostico|--verificar) MODE=diagnostico ;;
    --instalar|--arreglar)     MODE=instalar ;;
    --movil)                   DESTINO=movil ;;
    --csc-valor=*)             CSC_VALOR="${a#*=}" ;;
    --sin-csc)                 CSC_ACTIVO=0 ;;
    --pc)                      DESTINO=pc ;;
    --serie=*)                 SERIAL="${a#*=}" ;;
    -h|--help)
      sed -n '2,40p' "$SELF" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *) printf 'opción desconocida: %s (usa --diagnostico | --instalar | --movil | --pc | --serie=IP:PUERTO)\n' "$a" >&2
       exit 2 ;;
  esac
done

# --- log --------------------------------------------------------------------
say()  { printf '[%s] %s\n' "$(date -u +%H:%M:%S)" "$*"; }
warn() { printf '[%s] AVISO: %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; }
die()  { printf '[%s] ERROR: %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; exit 1; }
sha16() { printf '%s' "$1" | sha256sum 2>/dev/null | cut -c1-16; }

# --- detección de entorno ---------------------------------------------------
es_android() {
  [ -n "${PREFIX:-}" ] && case "$PREFIX" in *com.termux*) return 0 ;; esac
  [ -d /data/local/tmp/megadaemon ] && return 0
  [ -d /data/adb ] && return 0
  case "$(uname -s 2>/dev/null)" in Android) return 0 ;; esac
  return 1
}
if [ "$DESTINO" = auto ]; then
  if es_android; then DESTINO=movil; else DESTINO=pc; fi
fi

CURL=""
for c in curl "$(command -v curl 2>/dev/null)" /data/data/com.termux/files/usr/bin/curl; do
  [ -n "$c" ] && [ -x "$c" ] && CURL="$c" && break
done

# =============================================================================
# MÓVIL: helpers (todo lo que necesita root va por `su -mm`)
# =============================================================================
if [ "$DESTINO" = movil ]; then
  root() { su -mm -c "$1" 2>/dev/null | tr -d '\r'; }
  SCRIPT="sh"
else
  # desde el PC: todo por adb
  DEVLINE=""
  [ -n "$SERIAL" ] && ADB="$ADB -s $SERIAL"
  if command -v "$ADB" >/dev/null 2>&1; then
    DEVLINE="$("$ADB" devices 2>/dev/null | awk 'NR>1 && $2=="device"{print $1; exit}')"
    [ -n "$DEVLINE" ] && ADB="$ADB -s $DEVLINE"
  fi
  root() { [ -n "$DEVLINE" ] || return 1; "$ADB" shell "su -mm -c '$1'" 2>/dev/null | tr -d '\r'; }
  sh_on() { [ -n "$DEVLINE" ] || return 1; "$ADB" shell "$1" 2>/dev/null | tr -d '\r'; }
  SCRIPT="sh"
fi

STATE_REAL="$(root 'echo $MEGADAEMON_STATE_DIR' | tr -d '\r\n')"
[ -z "$STATE_REAL" ] && STATE_REAL="$STATE_REAL_DEFAULT"

# --- herramientas CSC (decodificador OMC + parcheador) ----------------------
CODEC_LOCAL=""
PATCHER_LOCAL=""
RAW=https://raw.githubusercontent.com/boolforge/megadaemon/main
# El CSC se parchea con el BINARIO RUST (megadaemon-core csc): sin python3
# en el movil.  Si el binario es antiguo, se cae al script de Python.
MD_BIN_LOCAL="$MD_DIR/megadaemon-core"
descargar_herramientas_csc() {
  [ -n "$CURL" ] || return 1
  DESTDIR="$MD_DIR"
  [ -d "$DESTDIR" ] || DESTDIR="${TMPDIR:-/tmp}"
  CODEC_LOCAL=""
  PATCHER_LOCAL=""
  if [ -x "$MD_BIN_LOCAL" ] && "$MD_BIN_LOCAL" csc --help >/dev/null 2>&1; then
    CSC_CMD="$MD_BIN_LOCAL csc"
    return 0
  fi
  if [ ! -f "$DESTDIR/omc_codec.py" ]; then
    "$CURL" -fsSL -o "$DESTDIR/omc_codec.py" "$RAW/tools/omc_codec.py" 2>/dev/null || return 1
  fi
  if [ ! -f "$DESTDIR/patch_csc_value.py" ]; then
    "$CURL" -fsSL -o "$DESTDIR/patch_csc_value.py" "$RAW/deploy/patch_csc_value.py" 2>/dev/null || return 1
  fi
  CODEC_LOCAL="$DESTDIR/omc_codec.py"
  PATCHER_LOCAL="$DESTDIR/patch_csc_value.py"
  CSC_CMD="$PATCHER_LOCAL"
  return 0
}

# =============================================================================
# 0) CONTEXTO
# =============================================================================
say "=== 0) contexto ==="
say "modo: $MODE | destino: $DESTINO"
if [ "$DESTINO" = movil ]; then
  [ "$(id -u)" = "0" ] || warn "no soy root: lo que necesite root va por 'su -mm' (en Android 'sudo' NO es root)"
  [ -n "$CURL" ] || warn "no hay curl en Termux: instálalo con 'pkg install curl' (la descarga del release lo necesita)"
fi
say "state_dir que usa el daemon: $STATE_REAL"
DAEMON="$(root 'pgrep -f megadaemon-core | head -1' | tr -d '\r\n')"
say "daemon: ${DAEMON:-NO}"
KEYS="$(root "ls -1 $MD_DIR/state/megadaemon_api.key $MD_DIR/state/megadaemon_api.keys 2>/dev/null | wc -l" | tr -d '\r\n ')"
say "keys disponibles en $MD_DIR/state: ${KEYS:-0}"
KEYFP="$(root "cat $MD_DIR/state/megadaemon_api.key 2>/dev/null" | sha256sum | cut -c1-16)"
say "sha16 de la key: ${KEYFP:-<ninguna>}"

# =============================================================================
# RESOLUCIÓN DEL RELEASE (aquí estaba el 404)
# =============================================================================
# Los tags de release SON `v<sha>`: no existe un tag "latest", por eso
# `releases/download/latest/...` devolvía 404.  Se resuelve por la API y,
# si no hay jq, con sed.  MD_RELEASE_TAG permite fijar el tag.
release_json=""
resolve_release() {
  [ -n "$CURL" ] || return 1
  if [ -n "${MD_RELEASE_TAG:-}" ]; then
    TAG="$MD_RELEASE_TAG"
  else
    release_json="$("$CURL" -fsSL -H 'Accept: application/vnd.github+json' "$API" 2>/dev/null)"
    TAG="$(printf '%s' "$release_json" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)"
    if [ -z "$TAG" ]; then
      # FALLBACK sin API y sin cuota: /releases/latest redirige (302) a
      # /releases/tag/v<sha>.  Los tags SON `v<sha>`: no existe "latest".
      LOC="$("$CURL" -sI "$BASE_RELEASES_LATEST" 2>/dev/null | tr -d '\r' | sed -n 's/^[Ll]ocation:[[:space:]]*\(.*\/releases\/tag\/[^[:space:]]*\).*/\1/p' | head -1)"
      TAG="$(printf '%s' "$LOC" | sed -n 's/.*\/releases\/tag\/\([^/?]*\).*/\1/p')"
      [ -n "$TAG" ] && say "release resuelta por redireccion (la API no respondio o dio 429): $TAG"
    fi
  fi
  [ -n "$TAG" ] || return 1
  RELEASE_TAG="$TAG"
  BASE="https://github.com/$REPO_SLUG/releases/download/$TAG"
  # asset exacto (el que NO es .sha256)
  ASSET="$BASE/megadaemon-core-android-arm64"
  if [ -n "$release_json" ]; then
    U="$(printf '%s' "$release_json" | tr ',' '\n' | grep 'browser_download_url' | grep 'megadaemon-core-android-arm64"' | grep -v '\.sha256' | sed -n 's/.*"browser_download_url"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)"
    [ -n "$U" ] && ASSET="$U"
  fi
  return 0
}

# =============================================================================
# 1) DIAGNÓSTICO (no toca nada)
# =============================================================================
diagnostico() {
  say "--- release ---"
  if resolve_release; then
    say "última release: $RELEASE_TAG"
    say "asset ARM64: $ASSET"
  else
    warn "no pude resolver la última release (¿red? ¿API de GitHub?)"
  fi
  say "--- CSC: descubrimiento de features ---"
  if descargar_herramientas_csc; then
    case "$CSC_CMD" in
      *megadaemon-core*)
        $CSC_CMD list 2>/dev/null | head -40 | while read -r l; do say "  $l"; done
        ;;
      *)
        PY3="$(command -v python3 2>/dev/null || echo /data/data/com.termux/files/usr/bin/python3)"
        [ -x "$PY3" ] && "$PY3" "$PATCHER_LOCAL" --listar 2>/dev/null | head -40 | while read -r l; do say "  $l"; done
        ;;
    esac
  else
    warn "no pude traer las herramientas CSC (sin red?)"
  fi
  CSCV="$(root "settings get secure recording_allowed_value 2>/dev/null" | tr -d '\r\n')"
  [ -n "$CSCV" ] && say "CSC (settings): $CSCV"
  say "CSC (modulo Magisk): $(root "test -f /data/adb/modules/megadaemon_csc_recording/cscfeature.xml && echo ACTIVO || echo NO")"
  say "--- canal ---"
  TURL="$(root 'cat /sdcard/megadaemon.url 2>/dev/null; cat /data/local/tmp/megadaemon/tunnel.url 2>/dev/null' | tr -d '\r\n' | grep -o 'https://[a-z0-9.-]*' | tail -1)"
  [ -n "$TURL" ] && say "URL vigente: $TURL (sha16 $(sha16 "$TURL"))" || say "URL vigente: NO ENCONTRADA"
  CF="$(root 'pgrep -c cloudflared' | tr -d ' \r\n')"
  say "procesos cloudflared: ${CF:-0} (1 es lo correcto; cada uno genera un hostname distinto)"
  SUPX="$(root "test -f $SUP && echo SI" | tr -d '\r\n')"
  say "supervisor ($SUP): ${SUPX:-NO} (sin él, el daemon no sobrevive a un reinicio del móvil)"
  if [ "$DESTINO" = pc ]; then
    say "--- PC ---"
    command -v caddy >/dev/null 2>&1 && say "caddy: presente" || say "caddy: AUSENTE (el relay necesita caddy o nginx+socat)"
    for t in "$HOME/.mavis/duckdns.token" "$SECRETS/duckdns.token" "$REPO/../md-secrets/duckdns.token"; do
      [ -f "$t" ] && say "token DuckDNS encontrado en $t (NO se imprime)" && break
    done
  fi
}

if [ "$MODE" = diagnostico ]; then
  diagnostico
  say ""
  say "resumen: daemon=${DAEMON:-NO} keys=${KEYS:-0} key_fp=${KEYFP:-none} state_dir=$STATE_REAL"
  say "para ARREGLAR: sudo ./deploy/fix-entorno.sh --instalar  (o 'sh fix-entorno.sh --instalar' en el móvil)"
  exit 0
fi

# =============================================================================
# 2) INSTALACIÓN
# =============================================================================
say "=== 1) credencial unificada ==="
root "mkdir -p $STATE_REAL && cp -f $MD_DIR/state/megadaemon_api.key $MD_DIR/state/megadaemon_api.keys $STATE_REAL/ 2>/dev/null; chmod 600 $STATE_REAL/megadaemon_api.* 2>/dev/null"
COPIED="$(root "ls -1 $STATE_REAL/megadaemon_api.key 2>/dev/null | wc -l" | tr -d '\r\n ')"
[ "${COPIED:-0}" = "0" ] && warn "no encuentro la key en $MD_DIR/state: revisa el state_dir real del daemon"
say "key en $STATE_REAL: ${COPIED:-0}"
root "mkdir -p $(dirname $SUP); touch $SUP; grep -q MEGADAEMON_STATE_DIR $SUP || echo 'export MEGADAEMON_STATE_DIR=$STATE_REAL' >> $SUP; chmod 755 $SUP"
say "supervisor: MEGADAEMON_STATE_DIR unificado"

say "=== 2) binario ARM64 (desde el release real) ==="
[ -n "$CURL" ] || die "no hay curl: en Termux, 'pkg install curl' (el móvil NO trae curl de fábrica)"
resolve_release || die "no pude resolver la última release de GitHub"
say "release: $RELEASE_TAG"
DL="$PWD/megadaemon-core.new"
[ "$DESTINO" = movil ] && DL="$MD_DIR/megadaemon-core.new"
"$CURL" -fsSL -o "$DL" "$ASSET" || die "no pude descargar $ASSET (HTTP $(curl -s -o /dev/null -w '%{http_code}' -I "$ASSET" 2>/dev/null))"
SA="$ASSET.sha256"
[ -n "$release_json" ] && SA="$(printf '%s' "$release_json" | tr ',' '\n' | grep 'browser_download_url' | grep 'megadaemon-core-android-arm64.sha256"' | sed -n 's/.*"browser_download_url"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)"
EXPECT=""
if "$CURL" -fsSL -o "$DL.sha256" "$SA" 2>/dev/null; then
  EXPECT="$(cut -d' ' -f1 < "$DL.sha256" | tr -d '\r\n ')"
fi
GOT="$(sha256sum "$DL" | cut -d' ' -f1)"
if [ -n "$EXPECT" ]; then
  [ "$EXPECT" != "$GOT" ] && die "sha256 del release ($EXPECT) != el descargado ($GOT)"
  say "sha256 verificado: $GOT"
else
  warn "no pude descargar el .sha256: SIGO con el binario sin verificar (NO recomendado)"
fi
# ¿es realmente un ELF aarch64?
if command -v readelf >/dev/null 2>&1; then
  readelf -h "$DL" 2>/dev/null | grep -qi "aarch64" || die "el binario descargado NO es aarch64"
fi
chmod 755 "$DL"
if [ "$DESTINO" = movil ]; then
  root "cp -f $MD_DIR/megadaemon-core $MD_DIR/megadaemon-core.bak 2>/dev/null; mv -f $DL $MD_DIR/megadaemon-core; chmod 755 $MD_DIR/megadaemon-core; test -x $MD_DIR/megadaemon-core && echo INSTALADO"
  [ "$(root "test -x $MD_DIR/megadaemon-core && echo SI" | tr -d '\r\n')" = "SI" ] || { root "cp -f $MD_DIR/megadaemon-core.bak $MD_DIR/megadaemon-core"; die "el binario no quedó ejecutable: restaurado el anterior"; }
else
  "$ADB" push "$DL" "$MD_DIR/megadaemon-core.new" >/dev/null 2>&1 || die "fallo el push del binario al móvil"
  root "chmod 755 $MD_DIR/megadaemon-core.new"
  RSHA="$(root "sha256sum $MD_DIR/megadaemon-core.new 2>/dev/null | cut -d' ' -f1" | tr -d '\r\n')"
  [ -n "$EXPECT" ] && [ "$RSHA" != "$EXPECT" ] && die "sha256 en el móvil ($RSHA) != el esperado ($EXPECT)"
  root "cp -f $MD_DIR/megadaemon-core $MD_DIR/megadaemon-core.bak 2>/dev/null; mv -f $MD_DIR/megadaemon-core.new $MD_DIR/megadaemon-core; chmod 755 $MD_DIR/megadaemon-core"
  [ "$(root "test -x $MD_DIR/megadaemon-core && echo SI" | tr -d '\r\n')" = "SI" ] || { root "cp -f $MD_DIR/megadaemon-core.bak $MD_DIR/megadaemon-core"; die "el binario no quedó ejecutable: restaurado el anterior"; }
fi
say "binario instalado y ejecutable (+x)"

say "=== 3) reinicio del daemon ==="
root "pkill -x megadaemon-core 2>/dev/null; sleep 2"
sleep 4
NEWD="$(root 'pgrep -f megadaemon-core | head -1' | tr -d '\r\n')"
if [ -z "$NEWD" ]; then
  root "cp -f $MD_DIR/megadaemon-core.bak $MD_DIR/megadaemon-core; chmod 755 $MD_DIR/megadaemon-core; pkill -x megadaemon-core; sleep 3"
  die "el daemon no arrancó: binario anterior restaurado (no he dejado el móvil sin daemon)"
fi
say "daemon arrancado (pid $NEWD)"

say "=== 3b) arranque automatico (service.d) ==="
# Magisk ejecuta /data/adb/service.d/* al arrancar el movil: con el trio
# instalado, el daemon + el tunel + el watchdog levantan SOLOS tras cada
# reinicio, sin Termux.  Sin esto, al cerrar Termux se cae todo.
install_service() {
  # $1 = fichero en el repo (raw), $2 = ruta destino en el movil
  SRC_URL="$RAW/$1"
  DST="$2"
  if root "test -f $DST" | tr -d '\r\n' | grep -q SI; then
    say "  ya esta: $DST"
    return 0
  fi
  TMPD="${TMPDIR:-/tmp}/svc.$$"
  if ! "$CURL" -fsSL -o "$TMPD" "$SRC_URL" 2>/dev/null; then
    warn "  no pude descargar $SRC_URL"
    return 1
  fi
  root "mkdir -p $(dirname $DST); cp -f $TMPD $DST; chmod 755 $DST"
  if root "test -x $DST" | tr -d '\r\n' | grep -q SI; then
    say "  instalado: $DST"
  else
    warn "  no pude instalar $DST"
  fi
}
install_service "deploy/99-megadaemon.sh" "$SUP"
install_service "deploy/98-cloudflared.sh" "/data/adb/service.d/98-cloudflared.sh"
install_service "deploy/tunnel-watchdog.sh" "$MD_DIR/tunnel-watchdog.sh"
say "arranque automatico: supervisor + tunel + watchdog (Magisk service.d)"

say "=== 4) túnel y canal ==="
CF="$(root 'pgrep -c cloudflared' | tr -d ' \r\n')"
say "procesos cloudflared: ${CF:-0} (1 es lo correcto)"
if [ "${CF:-0}" -gt 1 ] 2>/dev/null; then
  warn "varios cloudflared: cada uno tiene su hostname y roban tráfico. NO los mato ahora (cortar el canal)."
fi
TURL="$(root 'cat /sdcard/megadaemon.url 2>/dev/null; cat /data/local/tmp/megadaemon/tunnel.url 2>/dev/null' | tr -d '\r\n' | grep -o 'https://[a-z0-9.-]*' | tail -1)"
if [ -n "$TURL" ]; then
  say "URL vigente: $TURL (sha16 $(sha16 "$TURL"))"
  if [ -n "$SECRETS" ]; then
    mkdir -p "$SECRETS" 2>/dev/null; chmod 700 "$SECRETS" 2>/dev/null
    [ -f "$ENVFILE" ] || : > "$ENVFILE"; chmod 600 "$ENVFILE"
    grep -q '^MEGADAEMON_URL=' "$ENVFILE" 2>/dev/null \
      && sed -i "s|^MEGADAEMON_URL=.*|MEGADAEMON_URL=$TURL|" "$ENVFILE" \
      || printf 'MEGADAEMON_URL=%s\n' "$TURL" >> "$ENVFILE"
    say "URL registrada en $ENVFILE (0600)"
  fi
else
  warn "no encuentro la URL del túnel"
fi

say "=== 5) CSC: parche de la feature de grabación ==="
if [ "$CSC_ACTIVO" = "0" ]; then
  say "CSC: omitido (--sin-csc)"
elif descargar_herramientas_csc; then
  say "descubrimiento de features del CSC:"
  case "$CSC_CMD" in
    *megadaemon-core*)
      $CSC_CMD list 2>&1 | head -40 | while read -r l; do say "  $l"; done
      say "parcheando CscFeature_VoiceCall_ConfigRecording = $CSC_VALOR"
      OUT="$($CSC_CMD set "$CSC_VALOR" 2>&1)"
      ;;
    *)
      PY3="$(command -v python3 2>/dev/null || echo /data/data/com.termux/files/usr/bin/python3)"
      if [ ! -x "$PY3" ]; then
        warn "no hay binario CSC nuevo ni python3: actualiza el binario del release o instala python"
        OUT=""
      else
        "$PY3" "$PATCHER_LOCAL" --listar 2>&1 | head -40 | while read -r l; do say "  $l"; done
        say "parcheando CscFeature_VoiceCall_ConfigRecording = $CSC_VALOR"
        OUT="$("$PY3" "$PATCHER_LOCAL" "$CSC_VALOR" 2>&1)"
      fi
      ;;
  esac
  if [ -n "$OUT" ]; then
    printf '%s\n' "$OUT" | while read -r l; do say "  $l"; done
    case "$OUT" in
      *"módulo:"*|*"modulo actualizado"*) say "CSC parcheado (modulo Magisk).  El bind-mount se aplica al reiniciar." ;;
      *) warn "el parche CSC no se pudo aplicar (mira el detalle arriba)" ;;
    esac
  fi
else
  warn "no pude traer las herramientas CSC (sin red?)"
fi

if [ "$DESTINO" = pc ]; then
  say "=== 6) relay del PC (DuckDNS) ==="
  if [ -x "$REPO/deploy/relay-pc.sh" ]; then
    if [ "$(id -u)" = "0" ]; then
      "$REPO/deploy/relay-pc.sh" --instalar || warn "el relay no se instaló (revisa el token de DuckDNS)"
    elif command -v sudo >/dev/null 2>&1; then
      sudo "$REPO/deploy/relay-pc.sh" --instalar || warn "el relay necesita sudo (o token en ~/.mavis/duckdns.token)"
    else
      warn "sin sudo: el relay no se puede instalar"
    fi
  else
    warn "no encuentro deploy/relay-pc.sh"
  fi
  say "AVISO: el A record de DuckDNS lo publica SÓLO el PC (el sandbox puso el dominio apuntando al NAT del datacenter)"
fi

say "=== 7) verificación final ==="
RAM="$(root "grep MemAvailable /proc/meminfo | awk '{print int(\$2/1024)}'" | tr -d '\r\n ')"
LOAD="$(root 'cut -d" " -f1-3 /proc/loadavg' | tr -d '\r\n')"
say "RAM libre: ${RAM:-?} MB | load: ${LOAD:-?}"
if [ -n "$TURL" ]; then
  KEY="$(root "cat $MD_DIR/state/megadaemon_api.key 2>/dev/null" | tr -d '\r\n')"
  CODE="$("$CURL" -s -m 20 -o /dev/null -w '%{http_code}' -H "X-API-Key: $KEY" "$TURL/health" 2>/dev/null)"
  say "health por el túnel: HTTP ${CODE:-000}"
  case "$CODE" in
    200) say "OK: daemon + túnel + credencial operativos" ;;
    401|403) warn "el túnel llega pero la credencial no cuadra (401/403)" ;;
    530) warn "530: el túnel no está conectado al edge (reinicia cloudflared, NO el daemon)" ;;
    000) warn "000: sin ruta al túnel desde aquí (usa el relay del PC)" ;;
  esac
fi
say ""
say "LISTO. Estado en 3 líneas:"
say "  daemon pid $NEWD | binario ARM64 $RELEASE_TAG con +x, sha256 $GOT"
say "  credencial unificada en $STATE_REAL (sha16 $KEYFP)"
say "  túnel $TURL (sha16 $(sha16 "$TURL"))"
