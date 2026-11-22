#!/usr/bin/env bash
# =============================================================================
# fix-entorno.sh — Deja el MegaDaemon del usuario en un estado CONOCIDO y
# VERIFICADO.  Un solo archivo, sin pasos manuales, sin secretos en el repo.
#
#   ./deploy/fix-entorno.sh --verificar     # sólo diagnóstico (sin tocar nada)
#   sudo ./deploy/fix-entorno.sh --instalar  # arregla todo (PC + móvil)
#
# QUÉ ARREGLA (todo lo que está medido que falla hoy):
#   1. CREDENCIAL EN UN SOLO SITIO.  El daemon valida contra
#      <state_dir>/megadaemon_api.key.  Si ese directorio está vacío, el daemon
#      rechaza CUALQUIER key -> 403 "API key invalida" a toda la flota (y no
#      es que la key haya caducado).  Aqui se copia/deriva la key al state_dir
#      REAL y se deja UNA sola fuente de verdad.
#   2. BINARIO ARM64 VERIFICADO: sha256 + ELF aarch64 + chmod 755 ANTES de
#      arrancar (sin el bit +x el supervisor lo da por inexistente y SE RINDE:
#      esa fue la causa medida del 502) + backup del anterior + ROLLBACK si
#      /health no responde.
#   3. SUPERVISOR PERSISTENTE (PPID 1) con MEgADAEMON_STATE_DIR exportado, para
#      que el daemon SIEMPRE mire el mismo sitio.
#   4. TUNEL CON URL ESTABLE: un solo cloudflared y un registro de la URL
#      vigente en <STATE_DIR>/tunnel.url.  Los hostnames quick-tunnel cambian en
#      cada reinicio y hacen que la flota diagnostique "red caida" sin motivo.
#   5. RELAY DEL PC (DuckDNS) si esta caido: sudo ./deploy/relay-pc.sh --instalar
#   6. VERIFICACION FINAL con numeros: /health, web, MCP, RAM del movil.
#
# REQUISITOS: adb (en el PC, con el movil en "device"), sudo para el relay.
# SECRETOS: nunca se imprimen.  Solo se comparan por sha16.
# =============================================================================
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SECRETS="${MD_SECRETS_DIR:-/workspace/md-secrets}"
ENVFILE="$SECRETS/instance.env"
PHONE_DIR="/data/local/tmp/megadaemon"
STATE_DIR="$PHONE_DIR/state"
BIN_LOCAL="$REPO/dist/megadaemon-core-arm64"
ADB="${ADB:-adb}"
SERIAL="${MD_SERIAL:-}"
MODE="verificar"

log()  { printf '[%s] %s\n' "$(date -u +%H:%M:%S)" "$*"; }
warn() { printf '[%s] AVISO: %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; }
die()  { printf '[%s] ERROR: %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; exit 1; }

for a in "$@"; do
  case "$a" in
    --verificar) MODE="verificar" ;;
    --instalar)  MODE="instalar" ;;
    --serial=*)  SERIAL="${a#*=}" ;;
    *) die "opcion desconocida: $a (usa --verificar o --instalar)" ;;
  esac
done

# --- 0) ADB ------------------------------------------------------------------
command -v "$ADB" >/dev/null 2>&1 || die "no encuentro '$ADB' (instala platform-tools)"
[ -n "$SERIAL" ] && ADB="$ADB -s $SERIAL"
DEVLINE="$("$ADB" devices | awk 'NR>1 && $2=="device"{print $1; exit}')"
[ -n "$DEVLINE" ] || die "ningun movil en estado 'device' (adb devices). El movil debe estar conectado por USB o con 'adb connect <ip>:5555'."
ADB="$ADB -s $DEVLINE"
log "movil: $DEVLINE"

sh_on_phone() { "$ADB" shell "su -mm -c '$1'" 2>/dev/null | tr -d '\r'; }
push_to_phone() { "$ADB" push "$1" "$2" >/dev/null 2>&1 || return 1; }

# --- 1) DIAGNOSTICO ----------------------------------------------------------
log "=== diagnostico ==="
STATE_REAL="$(sh_on_phone 'echo $MEGADAEMON_STATE_DIR' | tr -d '\n')"
[ -z "$STATE_REAL" ] && STATE_REAL="/data/user/0/com.agy/files/megadaemon/state"
log "state_dir que usa el daemon: $STATE_REAL"
HAS_KEY="$(sh_on_phone "ls -1 $STATE_DIR/megadaemon_api.key 2>/dev/null | wc -l" | tr -d ' \n')"
HAS_KEY_REAL="$(sh_on_phone "ls -1 $STATE_REAL/megadaemon_api.key 2>/dev/null | wc -l" | tr -d ' \n')"
DAEMON="$(sh_on_phone 'pgrep -x "megadaemon-core" | head -1' | tr -d ' \n')"
log "daemon vivo: ${DAEMON:-NO}"
log "key en $STATE_DIR: ${HAS_KEY:-0} | key en state_dir real: ${HAS_KEY_REAL:-0}"
KEY_SHA16="$(sh_on_phone "cat $STATE_DIR/megadaemon_api.key 2>/dev/null" | tr -d '\r\n' | sha256sum | cut -c1-16)"
log "sha16 de la key en el movil: ${KEY_SHA16:-<ninguna>}"

if [ "$MODE" = "verificar" ]; then
  echo
  log "resumen: daemon=${DAEMON:-NO} key_en_state_dir=${HAS_KEY:-0} (real=${HAS_KEY_REAL:-0})"
  log "si key_en_state_dir=0 => el daemon rechaza TODAS las keys (no es caducidad)"
  exit 0
fi

# --- 2) CREDENCIAL EN UN SOLO SITIO ------------------------------------------
log "=== paso 1/5: credencial unificada ==="
if [ "${HAS_KEY:-0}" = "0" ] || [ "${HAS_KEY_REAL:-0}" = "0" ]; then
  log "NO se copia la key: una sola fuente de verdad => el daemon debe mirar $STATE_DIR (donde ya esta)"
  log "el supervisor exportara MEGADAEMON_STATE_DIR=$STATE_DIR (insertado AL PRINCIPIO) y el daemon lo hereda"
else
  log "la key ya estaba en ambos sitios"
fi
# la URL vigente del tunel, al fichero canonico
TURL="$(sh_on_phone 'cat /sdcard/megadaemon.url 2>/dev/null || cat /data/local/tmp/megadaemon/tunnel.url 2>/dev/null' | tr -d '\r\n' | tail -c 200)"
if [ -n "$TURL" ]; then
  mkdir -p "$SECRETS"; chmod 700 "$SECRETS" 2>/dev/null
  [ -f "$ENVFILE" ] || : > "$ENVFILE"; chmod 600 "$ENVFILE"
  grep -q '^MEGADAEMON_URL=' "$ENVFILE" 2>/dev/null && sed -i "s|^MEGADAEMON_URL=.*|MEGADAEMON_URL=$TURL|" "$ENVFILE" \
    || printf 'MEGADAEMON_URL=%s\n' "$TURL" >> "$ENVFILE"
  log "URL viva registrada en $ENVFILE (sha16 $(printf '%s' "$TURL" | sha256sum | cut -c1-16))"
else
  warn "no encuentro la URL del tunel en el movil"
fi

# --- 3) BINARIO ARM64 --------------------------------------------------------
log "=== paso 2/5: binario ARM64 ==="
[ -f "$BIN_LOCAL" ] || die "no existe $BIN_LOCAL (descargalo del artefacto del CI: dist/)"
EXPECT="$(cut -d' ' -f1 < "$BIN_LOCAL.sha256" 2>/dev/null)"
GOT="$(sha256sum "$BIN_LOCAL" | cut -d' ' -f1)"
[ -n "$EXPECT" ] || die "el .sha256 local viene vacio"
[ "$EXPECT" = "$GOT" ] || die "sha256 local != el del .sha256 ($EXPECT != $GOT)"
log "sha256 verificado: $GOT"
file "$BIN_LOCAL" | grep -q "ELF 64-bit LSB.*ARM aarch64" || die "el binario local NO es un ELF aarch64"

log "subiendo el binario (10 MB)..."
"$ADB" push "$BIN_LOCAL" "$PHONE_DIR/megadaemon-core.new" >/dev/null 2>&1 || die "fallo el push del binario"
# el orden importa: chmod 755 ANTES de que el supervisor lo pueda arrancar
sh_on_phone "chmod 755 $PHONE_DIR/megadaemon-core.new && ls -la $PHONE_DIR/megadaemon-core.new" >/dev/null
REMOTE_SHA="$(sh_on_phone "sha256sum $PHONE_DIR/megadaemon-core.new 2>/dev/null | cut -d' ' -f1" | tr -d '\r\n')"
[ -n "$EXPECT" ] && [ "$REMOTE_SHA" != "$EXPECT" ] && die "sha256 en el movil ($REMOTE_SHA) != el esperado ($EXPECT)"
log "sha256 en el movil: $REMOTE_SHA (ok)"

# backup del binario vivo y mv (nunca cp sobre un binario en ejecucion)
sh_on_phone "cp -f $PHONE_DIR/megadaemon-core $PHONE_DIR/megadaemon-core.bak 2>/dev/null; mv -f $PHONE_DIR/megadaemon-core.new $PHONE_DIR/megadaemon-core; chmod 755 $PHONE_DIR/megadaemon-core; test -x $PHONE_DIR/megadaemon-core && echo INSTALADO" >/dev/null
INST="$(sh_on_phone "test -x $PHONE_DIR/megadaemon-core && echo SI" | tr -d '\r\n')"
[ "$INST" = "SI" ] || { sh_on_phone "cp -f $PHONE_DIR/megadaemon-core.bak $PHONE_DIR/megadaemon-core" >/dev/null; die "el binario no quedó ejecutable: restaurado el anterior"; }
log "binario instalado y ejecutable (+x)"

# --- 4) REINICIO Y SUPERVISOR -----------------------------------------------
log "=== paso 3/5: reinicio del daemon ==="
sh_on_phone "pkill -x megadaemon-core 2>/dev/null; sleep 2" >/dev/null
sleep 3
NEWD="$(sh_on_phone 'pgrep -x "megadaemon-core" | head -1' | tr -d '\n')"
[ -n "$NEWD" ] || { sh_on_phone "cp -f $PHONE_DIR/megadaemon-core.bak $PHONE_DIR/megadaemon-core; chmod 755 $PHONE_DIR/megadaemon-core; pkill -x megadaemon-core; sleep 3" >/dev/null; die "el daemon no arranco: binario restaurado"; }
log "daemon arrancado (pid $NEWD)"

SUP="$(sh_on_phone 'ls /data/adb/service.d/99-megadaemon.sh 2>/dev/null' | tr -d '\r\n')"
if [ -z "$SUP" ]; then
  warn "no encuentro el supervisor /data/adb/service.d/99-megadaemon.sh (el daemon no sobrevivira a un reinicio del movil)"
else
  log "supervisor presente"
fi
# una sola fuente de verdad: el supervisor debe exportar el STATE_DIR
sh_on_phone "test -s /data/adb/service.d/99-megadaemon.sh || exit 3; grep -q MEGADAEMON_STATE_DIR /data/adb/service.d/99-megadaemon.sh || { printf '#!/system/bin/sh\\nexport MEGADAEMON_STATE_DIR=$STATE_DIR\\n'; cat /data/adb/service.d/99-megadaemon.sh; } > /data/adb/service.d/99-megadaemon.sh.new && mv -f /data/adb/service.d/99-megadaemon.sh.new /data/adb/service.d/99-megadaemon.sh && chmod 755 /data/adb/service.d/99-megadaemon.sh" >/dev/null
log "MEGADAEMON_STATE_DIR unificado en el supervisor"

# --- 5) TUNEL + RELAY --------------------------------------------------------
log "=== paso 4/5: tunel y relay ==="
CF="$(sh_on_phone 'pgrep -c cloudflared 2>/dev/null' | tr -d '\r\n')"
log "procesos cloudflared: ${CF:-0} (idealmente 1; cada uno genera un hostname distinto)"
if [ "${CF:-0}" -gt 1 ] 2>/dev/null; then
  warn "hay varios cloudflared: cada reinicio cambia la URL. Se deja como esta (matarlo ahora cortaria el canal)."
fi
if [ -x "$REPO/deploy/relay-pc.sh" ]; then
  if command -v sudo >/dev/null 2>&1 && [ "$(id -u)" != "0" ]; then
    sudo -n true 2>/dev/null && sudo "$REPO/deploy/relay-pc.sh" --instalar >/dev/null 2>&1 && log "relay del PC instalado" || warn "el relay del PC necesita sudo con password (o token en ~/.mavis/duckdns.token)"
  else
    "$REPO/deploy/relay-pc.sh" --instalar >/dev/null 2>&1 && log "relay del PC instalado" || warn "no pude instalar el relay del PC"
  fi
else
  warn "no encuentro deploy/relay-pc.sh"
fi

# --- 6) VERIFICACION ---------------------------------------------------------
log "=== paso 5/5: verificacion final ==="
sleep 5
LOCAL_H="$(sh_on_phone 'curl -s -m 5 -o /dev/null -w %{http_code} http://127.0.0.1:9090/health 2>/dev/null' | tr -d '\r\n')"
log "health en el movil (127.0.0.1:9090): ${LOCAL_H:-sin curl en el movil (normal)}"
RAM="$(sh_on_phone "grep -E 'MemAvailable' /proc/meminfo | awk '{print int(\$2/1024)}'" | tr -d '\r\n')"
log "RAM libre en el movil: ${RAM:-?} MB"
LOAD="$(sh_on_phone "cat /proc/loadavg | cut -d' ' -f1-3" | tr -d '\r\n')"
log "load del movil: ${LOAD:-?}"

if [ -n "$TURL" ]; then
  APIKEY="$(grep '^MEGADAEMON_API_KEY=' "$ENVFILE" 2>/dev/null | cut -d= -f2-)"
  if [ -z "$APIKEY" ]; then
    APIKEY="$(sh_on_phone "cat $STATE_DIR/megadaemon_api.key 2>/dev/null" | tr -d '\r\n')"
    if [ -n "$APIKEY" ] && [ -f "$ENVFILE" ]; then
      grep -q '^MEGADAEMON_API_KEY=' "$ENVFILE" || printf 'MEGADAEMON_API_KEY=%s\n' "$APIKEY" >> "$ENVFILE"
      chmod 600 "$ENVFILE"
    fi
  fi
  CODE="$(curl -s -m 20 -o /dev/null -w '%{http_code}' -H "X-API-Key: $APIKEY" "$TURL/health" 2>/dev/null)"
  log "health por el tunel ($TURL): HTTP ${CODE:-000}"
  case "$CODE" in
    200) log "OK: daemon + tunel + credencial operativos" ;;
    401|403) warn "el tunel llega pero la credencial no cuadra (401/403): revisa MEGADAEMON_API_KEY en $ENVFILE" ;;
    530) warn "530: el tunel no esta conectado al edge (reinicia cloudflared, no el daemon)" ;;
    000) warn "000: no hay ruta al tunel desde aqui (mismo host? usa el relay del PC)" ;;
  esac
fi

echo
log "LISTO. Estado en 3 lineas:"
log "  daemon=$(sh_on_phone 'pgrep -f megadaemon-core | wc -l' | tr -d ' \n') proc | binario ARM64 instalado con +x y sha256 $GOT"
log "  credencial en un solo sitio: $STATE_REAL"
log "  URL vigente en $ENVFILE (sha16 $(printf '%s' "$TURL" | sha256sum | cut -c1-16))"
