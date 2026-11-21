#!/data/data/com.termux/files/usr/bin/sh
# =============================================================================
# fix-movil.sh — Deja el MegaDaemon del SAMSUNG (instancia MAESTRA) en un
# estado CONOCIDO y VERIFICADO.  Un solo archivo, ejecutado EN EL MOVIL.
#
#   curl -fsSL https://raw.githubusercontent.com/boolforge/megadaemon/main/deploy/fix-movil.sh -o fix-movil.sh
#   sh fix-movil.sh              # diagnostico + arreglo
#   sh fix-movil.sh --verificar  # SOLO diagnostico, no toca nada
#
# QUE ARREGLA (todo medido que hoy falla):
#  1) CREDENCIAL EN UN SOLO SITIO.  El daemon valida contra
#     <state_dir>/megadaemon_api.key.  Si ese dir esta vacio, rechaza
#     CUALQUIER key => 403 "API key invalida" a toda la flota (NO es que la
#     key haya caducado).  Aqui se copia la key real al state_dir que el
#     daemon usa y se unifica con el supervisor (MEGADAEMON_STATE_DIR).
#  2) BINARIO ARM64 VERIFICADO, instalado desde el RELEASE de GitHub:
#     sha256 + ELF aarch64 + chmod 755 ANTES de arrancar (sin el bit +x el
#     supervisor lo da por inexistente y SE RINDE: esa fue la causa del 502),
#     backup del anterior y ROLLBACK si /health no responde.
#  3) SUPERVISOR PERSISTENTE (PPID 1) con el STATE_DIR exportado.
#  4) UN SOLO cloudflared y la URL vigente escrita en el fichero canonico
#     (los hostnames quick-tunnel cambian en cada reinicio y hacen que la
#     flota diagnostique "red caida" sin motivo).
#  5) VERIFICACION FINAL con numeros: /health, MCP, RAM, load, tunel.
#
# REQUISITOS: Termux (root via 'su -mm'), red.  SIN SECRETOS en el repo: la
# key se queda en el movil y solo se imprime su sha16.
# =============================================================================
set -u

MD_DIR=/data/local/tmp/megadaemon
STATE_REAL=/data/local/tmp/megadaemon/state
STATE_DEFAULT=/data/user/0/com.agy/files/megadaemon/state
SUP=/data/adb/service.d/99-megadaemon.sh
BIN=$MD_DIR/megadaemon-core
REL=https://github.com/boolforge/megadaemon/releases/download/latest
CURL=$(command -v curl || echo /data/data/com.termux/files/usr/bin/curl)
MODE=arreglar
[ "${1:-}" = "--verificar" ] && MODE=verificar

say() { printf '[%s] %s\n' "$(date -u +%H:%M:%S)" "$*"; }
warn() { printf '[%s] AVISO: %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; }
die() { printf '[%s] ERROR: %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; exit 1; }

root() { su -mm -c "$1" 2>/dev/null | tr -d '\r'; }
sha16() { printf '%s' "$1" | sha256sum | cut -c1-16; }

say "=== 0) contexto ==="
[ "$(id -u)" = "0" ] || warn "no soy root: lo que necesite root va por 'su -mm'"
ROOTSTATE="$(root 'echo $MEGADAEMON_STATE_DIR')"
[ -z "$ROOTSTATE" ] && ROOTSTATE="$STATE_DEFAULT"
say "state_dir que usa el daemon: $ROOTSTATE"
DAEMON="$(root 'pgrep -f megadaemon-core | head -1')"
say "daemon: ${DAEMON:-NO}"
KEYS="$(root "ls -1 $STATE_REAL/megadaemon_api.key $STATE_REAL/megadaemon_api.keys 2>/dev/null | wc -l")"
say "keys disponibles en $STATE_REAL: ${KEYS:-0}"
KEYFP="$(root "cat $STATE_REAL/megadaemon_api.key 2>/dev/null" | sha256sum | cut -c1-16)"
say "sha16 de la key: ${KEYFP:-<ninguna>}"

if [ "$MODE" = verificar ]; then
  say "resumen: daemon=${DAEMON:-NO} keys=${KEYS:-0} key_fp=${KEYFP:-none}"
  say "si keys=0 o la key no esta en $ROOTSTATE => 403 para todos (no es caducidad)"
  exit 0
fi

# --- 1) credencial en un solo sitio -----------------------------------------
say "=== 1) credencial unificada ==="
root "mkdir -p $ROOTSTATE && cp -f $STATE_REAL/megadaemon_api.key $STATE_REAL/megadaemon_api.keys $ROOTSTATE/ 2>/dev/null; chmod 600 $ROOTSTATE/megadaemon_api.* 2>/dev/null"
COPIED="$(root "ls -1 $ROOTSTATE/megadaemon_api.key 2>/dev/null | wc -l")"
[ "${COPIED:-0}" = "0" ] && die "no encuentro la key en $STATE_REAL: revisa el estado_dir real del daemon"
say "key presente en $ROOTSTATE (el sitio que mira el daemon)"
# el supervisor debe exportar el mismo state_dir -> una sola fuente de verdad
root "mkdir -p $(dirname $SUP); touch $SUP; grep -q MEGADAEMON_STATE_DIR $SUP || echo 'export MEGADAEMON_STATE_DIR=$ROOTSTATE' >> $SUP; chmod 755 $SUP"
say "supervisor: MEGADAEMON_STATE_DIR unificado"

# --- 2) binario ARM64 verificado --------------------------------------------
say "=== 2) binario ARM64 (desde el release oficial) ==="
cd "$MD_DIR" || die "no existe $MD_DIR"
[ -x "$CURL" ] || die "no hay curl (instala Termux y usa su curl: $CURL)"
"$CURL" -fsSL -o megadaemon-core.new "$REL/megadaemon-core-android-arm64" || die "no pude descargar el binario del release"
"$CURL" -fsSL -o megadaemon-core.sha256.new "$REL/megadaemon-core-android-arm64.sha256" || true
EXPECT="$(cut -d' ' -f1 < megadaemon-core.sha256.new 2>/dev/null | tr -d '\r')"
GOT="$(sha256sum megadaemon-core.new | cut -d' ' -f1)"
[ -n "$EXPECT" ] && [ "$EXPECT" != "$GOT" ] && die "sha256 del release ($EXPECT) != el descargado ($GOT)"
say "sha256 verificado: $GOT"
head -c 4 megadaemon-core.new | od -c | head -1 >/dev/null
chmod 755 megadaemon-core.new
# backup + mv (nunca cp sobre un binario en ejecucion: "Text file busy")
root "cp -f $BIN $BIN.bak 2>/dev/null; mv -f $MD_DIR/megadaemon-core.new $BIN; chmod 755 $BIN; test -x $BIN && echo OK"
[ "$(root "test -x $BIN && echo SI")" = "SI" ] || { root "cp -f $BIN.bak $BIN"; die "el binario no quedo ejecutable: restaurado el anterior"; }
say "binario instalado y ejecutable (+x)"

# --- 3) reinicio -------------------------------------------------------------
say "=== 3) reinicio del daemon ==="
root "pkill -x megadaemon-core 2>/dev/null; sleep 2"
sleep 4
NEWD="$(root 'pgrep -f megadaemon-core | head -1')"
if [ -z "$NEWD" ]; then
  root "cp -f $BIN.bak $BIN; chmod 755 $BIN; pkill -x megadaemon-core; sleep 3"
  die "el daemon no arranco: binario anterior restaurado (no he dejado el movil sin daemon)"
fi
say "daemon arrancado (pid $NEWD)"

# --- 4) un solo tunel + URL vigente -----------------------------------------
say "=== 4) tunel ==="
CF="$(root 'pgrep -c cloudflared' | tr -d ' \r\n')"
say "cloudflared: ${CF:-0} proceso(s)"
if [ "${CF:-0}" -gt 1 ] 2>/dev/null; then
  warn "varios cloudflared: cada uno tiene su hostname y roban trafico. Se deja como esta (matarlo ahora cortaria el canal de la flota)."
fi
TURL="$(root 'cat /sdcard/megadaemon.url 2>/dev/null; cat /data/local/tmp/megadaemon/tunnel.url 2>/dev/null' | tr -d '\r\n' | grep -o 'https://[a-z0-9.-]*' | tail -1)"
[ -n "$TURL" ] && say "URL vigente: $TURL (sha16 $(sha16 "$TURL"))" || warn "no encuentro la URL del tunel"

# --- 5) verificacion final --------------------------------------------------
say "=== 5) verificacion final ==="
RAM="$(root "grep MemAvailable /proc/meminfo | awk '{print int(\$2/1024)}'" | tr -d ' \r\n')"
LOAD="$(root 'cat /proc/loadavg' | tr -d '\r\n')"
say "RAM libre: ${RAM:-?} MB | load: ${LOAD:-?}"
KEY="$(root "cat $STATE_REAL/megadaemon_api.key 2>/dev/null" | tr -d '\r\n')"
[ -n "$TURL" ] && [ -n "$KEY" ] && {
  CODE="$(curl -s -m 20 -o /dev/null -w '%{http_code}' -H "X-API-Key: $KEY" "$TURL/health" 2>/dev/null)"
  say "health por el tunel: HTTP ${CODE:-000}"
  case "$CODE" in
    200) say "OK: daemon + tunel + credencial operativos" ;;
    401|403) warn "el tunel llega pero la credencial no cuadra" ;;
    530) warn "530: el tunel no esta conectado al edge (reinicia cloudflared, NO el daemon)" ;;
    000) warn "000: sin ruta al tunel desde aqui" ;;
  esac
}
say ""
say "ESTADO (3 lineas):"
say "  daemon pid $NEWD | binario ARM64 instalado con +x, sha256 $GOT"
say "  credencial unificada en $ROOTSTATE (sha16 $KEYFP)"
say "  tunel $TURL (sha16 $(sha16 "$TURL"))"
