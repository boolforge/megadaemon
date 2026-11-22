#!/data/data/com.termux/files/usr/bin/sh
# =============================================================================
# fix-movil.sh — Deja el MegaDaemon del SAMSUNG (instancia MAESTRA) en un
# estado CONOCIDO y VERIFICADO.  Un solo archivo, ejecutado EN EL MOVIL.
#
#   sh fix-movil.sh              # diagnostico + arreglo
#   sh fix-movil.sh --verificar  # SOLO diagnostico, no toca nada
#
# REVISION ADVERSARIA (W2/CI, 13 hallazgos: 3 P0 + 7 P1 + 3 P2) APLICADA:
#   P0-1 la URL releases/download/latest da 404 (los tags son v<sha>) y las
#        releases nuevas NO llevan el binario  => se RESUELVE POR API la
#        release que CONTIENE el asset y se exige su .sha256.
#   P0-2 se VERIFICA que lo descargado sea un ELF aarch64 (si no, el HTML de
#        error del 404 acababa instalado como binario => movil sin daemon).
#   P0-3 la comprobacion de sha256 es FATAL (antes se saltaba en silencio).
#   P1-1 `pgrep -x` (nombre exacto) y no `-f` (que encuentra al propio script).
#   P1-2 se espera HASTA 60 s a /health=200 en vez de 4 s (el supervisor tiene
#        watchdog de ~45 s) => ya no hay rollbacks innecesarios.
#   P1-3 el supervisor NUNCA se crea vacio (arrancaria sin daemon).
#   P1-4 el export se inserta AL PRINCIPIO del supervisor (lo hereda el daemon).
#   P1-5 NO se copia la key: se unifica el state_dir del proceso (una sola
#        copia, cero deriva) => el 403 desaparece sin duplicar credenciales.
#   P1-6 el .new entra al namespace de su por cp y el swap se hace DENTRO de
#        su (mv entre namespaces no existe).
#   P1-7 el rollback verifica que restauró (si no, dice que lorestore y qué).
#   P2-1 se verifica el MCP (el canal real de la flota), no solo /health.
#   P2-2 no se promete "un solo cloudflared": se informa y NO se mata ninguno.
#   P2-3 el diagnostico lee la key del state_dir que usa el daemon.
# =============================================================================
set -u

MD_DIR=/data/local/tmp/megadaemon
STATE_REAL=/data/local/tmp/megadaemon/state
STATE_DEFAULT=/data/user/0/com.agy/files/megadaemon/state
SUP=/data/adb/service.d/99-megadaemon.sh
BIN=$MD_DIR/megadaemon-core
API=https://api.github.com/repos/boolforge/megadaemon
CURL=$(command -v curl || echo /data/data/com.termux/files/usr/bin/curl)
HOME_DIR="${HOME:-/data/data/com.termux/files/home}"
MODE=arreglar
[ "${1:-}" = "--verificar" ] && MODE=verificar

say()  { printf '[%s] %s\n' "$(date -u +%H:%M:%S)" "$*"; }
warn() { printf '[%s] AVISO: %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; }
die()  { printf '[%s] ERROR: %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; exit 1; }
root() { su -mm -c "$1" 2>/dev/null | tr -d '\r'; }
sha16(){ printf '%s' "$1" | sha256sum | cut -c1-16; }

# ELF aarch64 sin depender de `file` (Termux puede no tenerlo):
#  bytes 0-3 = 7f 45 4c 46 (ELF) y bytes 18-19 = b7 00 (EM_AARCH64=183, LE)
is_elf_aarch64() {
  _m="$(od -An -tx1 -N20 "$1" 2>/dev/null | tr -d ' \n')"
  [ "${_m#7f454c46}" != "$_m" ] || return 1
  case "$_m" in *b700*) return 0;; *) return 1;; esac
}

say "=== 0) contexto ==="
ROOTSTATE="$(root 'echo $MEGADAEMON_STATE_DIR')"
[ -z "$ROOTSTATE" ] && ROOTSTATE="$STATE_DEFAULT"
DAEMON="$(root 'pgrep -x megadaemon-core | head -1')"
KEY="$(root "cat $STATE_REAL/megadaemon_api.key 2>/dev/null")"
say "state_dir por defecto del daemon: $ROOTSTATE"
say "daemon (pgrep -x): ${DAEMON:-NO}"
say "key en $STATE_REAL: ${KEY:+si (sha16 $(sha16 "$KEY"))}${KEY:-NO}"
say "supervisor: $(root "test -s $SUP && echo OK || echo AUSENTE_VACIO")"
TURL="$(root 'cat /sdcard/megadaemon.url 2>/dev/null; cat /data/local/tmp/megadaemon/tunnel.url 2>/dev/null' | tr -d '\r\n' | grep -o 'https://[a-z0-9.-]*' | tail -1)"
say "tunel: ${TURL:-desconocido} | cloudflared: $(root 'pgrep -c cloudflared' | tr -d ' \r\n') proceso(s)"

if [ "$MODE" = verificar ]; then
  say ""
  say "resumen: daemon=${DAEMON:-NO} key=${KEY:+si}${KEY:-NO} en $STATE_REAL"
  say "si el daemon usa $ROOTSTATE y la key esta en $STATE_REAL => el 403 es por state_dir, no por caducidad"
  exit 0
fi

# --- 1) state_dir unificado (UNA sola copia de la key) -----------------------
say "=== 1) state_dir unificado (P1-4, P1-5) ==="
[ -s "$SUP" ] || die "el supervisor $SUP no existe o esta vacio: NO lo creo (un supervisor vacio arranca el movil SIN daemon)"
if ! grep -q 'MEGADAEMON_STATE_DIR' "$SUP"; then
  # insertar el export AL PRINCIPIO (detras del shebang) para que lo herede el daemon
  TMP="$HOME_DIR/.99-megadaemon.sh.new"
  { printf '#!/system/bin/sh\n'; printf 'export MEGADAEMON_STATE_DIR=%s\n' "$STATE_REAL"; cat "$SUP"; } > "$TMP"
  root "cp -f $TMP $SUP && chmod 755 $SUP && rm -f $TMP"
  say "export MEGADAEMON_STATE_DIR=$STATE_REAL insertado al principio del supervisor"
else
  say "el supervisor ya define MEGADAEMON_STATE_DIR (no lo toco)"
fi
grep -q "MEGADAEMON_STATE_DIR=$STATE_REAL" "$SUP" || warn "el supervisor define otro state_dir: revisa a mano"

# --- 2) binario ARM64 del release que SI lo contiene -------------------------
say "=== 2) binario ARM64 (P0-1, P0-2, P0-3) ==="
rel_json="$("$CURL" -fsSL -m 25 "$API/releases?per_page=30" 2>/dev/null)" || die "no pude listar las releases (sin red?)"
TAG=""
for t in $(printf '%s' "$rel_json" | tr '{' '\n' | grep -o '"tag_name":"[^"]*"' | sed 's/.*:"//;s/"//'); do
  blk="$(printf '%s' "$rel_json" | tr '{' '\n' | grep -A40 "\"tag_name\":\"$t\"" | head -60)"
  printf '%s' "$blk" | grep -q 'megadaemon-core-android-arm64"' && { TAG="$t"; break; }
done
[ -n "$TAG" ] || die "ninguna release contiene megadaemon-core-android-arm64 (el CI no lo ha publicado)"
say "release con el binario: $TAG"
REL="https://github.com/boolforge/megadaemon/releases/download/$TAG"
cd "$HOME_DIR" || die "no puedo escribir en $HOME_DIR"
"$CURL" -fsSL -m 180 -o megadaemon-core.new "$REL/megadaemon-core-android-arm64" || die "fallo la descarga del binario de $TAG"
"$CURL" -fsSL -m 60 -o megadaemon-core.sha256.new "$REL/megadaemon-core-android-arm64.sha256" || die "fallo la descarga del .sha256 de $TAG"
EXPECT="$(cut -d' ' -f1 < megadaemon-core.sha256.new | tr -d '\r')"
GOT="$(sha256sum megadaemon-core.new | cut -d' ' -f1)"
[ -n "$EXPECT" ] || die "el .sha256 del release viene vacio"
[ "$EXPECT" = "$GOT" ] || die "sha256 ($EXPECT) != el descargado ($GOT)"
say "sha256 verificado: $GOT"
is_elf_aarch64 megadaemon-core.new || die "lo descargado NO es un ELF aarch64 (probablemente un HTML de error)"
chmod 755 megadaemon-core.new
say "ELF aarch64 OK"

# --- 3) instalacion (P1-6: el swap dentro del namespace de su) --------------
say "=== 3) instalacion ==="
root "mkdir -p $MD_DIR && cp -f $HOME_DIR/megadaemon-core.new $MD_DIR/megadaemon-core.new && chmod 755 $MD_DIR/megadaemon-core.new && cp -f $BIN $BIN.bak 2>/dev/null; mv -f $MD_DIR/megadaemon-core.new $MD_DIR/megadaemon-core && chmod 755 $MD_DIR/megadaemon-core && test -x $MD_DIR/megadaemon-core && echo OK" >/dev/null
[ "$(root "test -x $BIN && echo SI")" = "SI" ] || { warn "el binario no quedo ejecutable"; }
root "ls -la $BIN" | head -2
say "binario instalado (backup en $BIN.bak)"

# --- 4) reinicio y espera REAL de /health (P1-1, P1-2) ----------------------
say "=== 4) reinicio + espera de /health (hasta 60 s) ==="
root "pkill -x megadaemon-core 2>/dev/null; sleep 2"
ok=0
i=1
while [ "$i" -le 20 ]; do
  sleep 3
  code="$(curl -s -m 5 -o /dev/null -w '%{http_code}' http://127.0.0.1:9090/health 2>/dev/null)"
  if [ "$code" = "200" ]; then ok=1; break; fi
  i=$((i+1))
done
if [ "$ok" != "1" ]; then
  warn "/health no respondio 200 en 60 s => rollback REAL"
  root "cp -f $BIN.bak $BIN && chmod 755 $BIN && pkill -x megadaemon-core" >/dev/null
  sleep 5
  if [ "$(root "test -x $BIN && echo SI")" = "SI" ]; then
    die "rollback ejecutado: binario anterior restaurado (comprueba que /health responde antes de seguir)"
  else
    die "ROLLBACK FALLIDO: el binario anterior no quedo ejecutable. Arranca el movil en modo seguro o copia a mano el binario bueno"
  fi
fi
NEWPID="$(root 'pgrep -x megadaemon-core | head -1')"
say "daemon vivo (pgrep -x): ${NEWPID:-?} | /health 200"

# --- 5) verificacion final (P2-1, P2-2, P2-3) -------------------------------
say "=== 5) verificacion final ==="
RAM="$(root "grep MemAvailable /proc/meminfo | awk '{print int(\$2/1024)}'" | tr -d ' \r\n')"
LOAD="$(root 'cat /proc/loadavg' | tr -d ' \r\n')"
say "RAM libre: ${RAM:-?} MB | load: ${LOAD:-?}"
say "cloudflared: $(root 'pgrep -c cloudflared' | tr -d ' \r\n') (NO los mato: cada uno tiene su hostname y el canal de la flota depende de ellos)"
if [ -n "$TURL" ] && [ -n "$KEY" ]; then
  CODE="$(curl -s -m 20 -o /dev/null -w '%{http_code}' -H "X-API-Key: $KEY" "$TURL/health" 2>/dev/null)"
  say "health por el tunel: HTTP ${CODE:-000}"
  MCP="$(curl -s -m 25 -o /dev/null -w '%{http_code}' -X POST "$TURL/mcp" -H "Content-Type: application/json" -H "X-API-Key: $KEY" -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' 2>/dev/null)"
  say "MCP (tools/list): HTTP ${MCP:-000} (200 = la flota vuelve a tener canal)"
  case "$CODE" in
    200) say "OK: daemon + tunel + credencial operativos" ;;
    401|403) warn "el tunel llega pero la credencial no cuadra: comprueba el state_dir del daemon" ;;
    530) warn "530: el tunel no esta conectado al edge (reinicia cloudflared, NO el daemon)" ;;
    000) warn "000: sin ruta al tunel desde el movil" ;;
  esac
fi
say ""
say "ESTADO (3 lineas):"
say "  daemon pid ${NEWPID:-?} | ARM64 instalado con +x, sha256 $GOT | /health 200"
say "  credencial: UNA sola copia en $STATE_REAL | state_dir del daemon: $ROOTSTATE"
say "  tunel ${TURL:-?} (sha16 $(sha16 "$TURL")) | RAM ${RAM:-?} MB | load ${LOAD:-?}"
