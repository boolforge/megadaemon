#!/data/data/com.termux/files/usr/bin/sh
# =============================================================================
# fix-movil.sh — UN SOLO SCRIPT que deja el MegaDaemon del SAMSUNG (instancia
# MAESTRA) en un estado conocido, verificado, ACTUALIZADO y VIGILADO.
#
#   sh fix-movil.sh                       # comprueba si hay build nueva, la
#                                         # instala si hace falta, arregla
#                                         # la credencial, verifica y sale
#   sh fix-movil.sh --verificar           # SOLO diagnóstico, no toca nada
#   sh fix-movil.sh --watch [segundos]    # WATCHDOG en primer plano (Ctrl-C)
#   sh fix-movil.sh --instalar-watchdog [seg]  # watchdog como servicio + sale
#
# QUE HACE (todo medido que hoy falla):
#  1) CREDENCIAL EN UN SOLO SITIO.  El daemon valida contra
#     <state_dir>/megadaemon_api.key; si ese dir esta vacio rechaza
#     CUALQUIER key => 403 "API key invalida" a toda la flota (NO es caducidad).
#     Se busca la key en TODOS los sitios conocidos y se copia al state_dir
#     que el daemon usa de verdad.
#  2) BUILD NUEVA, AUTOMATICA.  Se consulta el ultimo release de GitHub, se
#     compara su sha256 con el del binario instalado y, si es DISTINTA, se
#     descarga, se VERIFICA (sha256 + ELF aarch64 de verdad) y se instala con
#     backup y ROLLBACK automatico.  Si es la MISMA no se toca nada.
#  3) SUPERVISOR con el STATE_DIR exportado ANTES de arrancar (si se exporta
#     al final, no tiene efecto).
#  4) UN SOLO cloudflared y la URL vigente en el fichero canonico.
#  5) WATCHDOG: cada N segundos comprueba que el daemon vive y que /health
#     responde (si no, lo relanza) Y comprueba si hay build nueva (si la hay,
#     la instala).  Todo queda en $MD_DIR/fix-movil.log y $MD_DIR/estado.txt
#     para leerlo sin mas (-> cat /data/local/tmp/megadaemon/estado.txt).
#
# REQUISITOS: Termux (root via 'su -mm') y red.  SIN SECRETOS en el repo: la
# key se queda en el movil y solo se imprime su sha16.
#
# v2 (2026-11-21, revision adversarial de W7): los fallos de la v1 estan
# corregidos y marcados con [v2] (release 404, sha256 opcional, ELF no
# comprobado, pkill sin relanzar, export al final del supervisor, key solo en
# un sitio, pgrep -f, sin comprobacion de build nueva, sin watchdog).
# =============================================================================
set -u

MD_DIR=/data/local/tmp/megadaemon
STATE_REAL=$MD_DIR/state
STATE_DEFAULT=/data/user/0/com.agy/files/megadaemon/state
SUP=/data/adb/service.d/99-megadaemon.sh
WDSVC=/data/adb/service.d/99-megadaemon-watchdog.sh
BIN=$MD_DIR/megadaemon-core
LOG=$MD_DIR/fix-movil.log
ST=$MD_DIR/estado.txt
LOCK=$MD_DIR/.fix-movil.lock
REPO=boolforge/megadaemon
# [v2] la v1 usaba ".../releases/download/latest/..." => HTTP 404 (ese tag no
# existe: los tags reales son "v<sha40>").  "/releases/latest/download/<asset>"
# SI redirige al ultimo release publicado.
REL_BASE=https://github.com/$REPO/releases/latest/download
API_LATEST=https://api.github.com/repos/$REPO/releases/latest
INTERVALO=1800
CURL=$(command -v curl 2>/dev/null || echo /data/data/com.termux/files/usr/bin/curl)
MODO=arreglar
WATCH=0
INSTALAR_WD=0

[ "${1:-}" = "--verificar" ] && MODO=verificar
[ "${1:-}" = "--watch" ] && { WATCH=1; INTERVALO="${2:-$INTERVALO}"; }
[ "${1:-}" = "--instalar-watchdog" ] && { INSTALAR_WD=1; INTERVALO="${2:-$INTERVALO}"; }

say()  { printf '[%s] %s\n' "$(date -u +%H:%M:%S)" "$*"; }
warn() { printf '[%s] AVISO: %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; }
die()  { printf '[%s] ERROR: %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; exit 1; }
root() { su -mm -c "$1" 2>/dev/null | tr -d '\r'; }
sha16() { printf '%s' "$1" | sha256sum | cut -c1-16; }
log()  { printf '[%s] %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" >>"$LOG" 2>/dev/null || true; }

# --- ¿hay root de verdad? --------------------------------------------------
# [v2] sin root NO se toca nada: se informa y se sale (antes el script
# Seguia y `die`aba a mitad, dejando el movil a medias).
HAVE_ROOT=0
if [ "$(id -u)" = "0" ]; then
  HAVE_ROOT=1
elif command -v su >/dev/null 2>&1 && su -mm -c true >/dev/null 2>&1; then
  HAVE_ROOT=1
fi

# --- lock (mkdir es atomico; no hay flock en Android) ----------------------
take_lock() {
  mkdir -p "$(dirname "$LOCK")" 2>/dev/null || true
  mkdir "$LOCK" 2>/dev/null || return 1
  trap 'rmdir "$LOCK" 2>/dev/null' EXIT INT TERM
  return 0
}
free_lock() { rmdir "$LOCK" 2>/dev/null || true; }

# --- estado legible al despertar -------------------------------------------
estado() {
  mkdir -p "$MD_DIR" 2>/dev/null || true
  {
    printf 'actualizado: %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    printf 'daemon: %s\n' "${DAEMON:-NO}"
    printf 'binario_sha256: %s\n' "${GOT_LOCAL:-?}"
    printf 'release_vigente: %s\n' "${TAG:-?}"
    printf 'salud: %s\n' "${HEALTH:-?}"
    printf 'tunel: %s\n' "${TURL:-?}"
    printf 'key_sha16: %s\n' "${KEYFP:-<ninguna>}"
    printf 'notas: %s\n' "${NOTAS:-}"
  } >"$ST" 2>/dev/null || true
}

# --- state_dir real: el que usa el daemon ----------------------------------
# [v2] la v1 no validaba ROOTSTATE: un banner de `su` lo contaminaba y el
# mkdir/cp iban a una ruta basura.
state_dir_real() {
  s="$(root 'echo $MEGADAEMON_STATE_DIR' | tr -d '\r' | head -1 | tr -cd 'A-Za-z0-9_./-')"
  [ -z "$s" ] && s="$STATE_DEFAULT"
  [ -d "$s" ] && { printf '%s' "$s"; return; }
  printf '%s' "$STATE_DEFAULT"
}

# --- donde esta la key (la v1 solo miraba STATE_REAL) ---------------------
find_key() {
  for d in "$STATE_REAL" "$STATE_DEFAULT" "$MD_DIR"; do
    for f in megadaemon_api.key megadaemon_api.keys; do
      k="$(root "cat $d/$f 2>/dev/null" | tr -d '\r\n')"
      [ -n "$k" ] && { printf '%s' "$d/$f"; return; }
    done
  done
  printf ''
}

# --- daemon vivo? (la v1 usaba pgrep -f: matcheaba el propio script) ------
daemon_pid() { root 'pgrep -x megadaemon-core | head -1' | tr -d ' \r\n'; }

# --- salud local (sin red; se prueban los puertos conocidos) --------------
health_local() {
  for p in 8080 8090 3000 9090; do
    c="$(curl -s -m 4 -o /dev/null -w '%{http_code}' "http://127.0.0.1:$p/health" 2>/dev/null)"
    [ "$c" = "200" ] && { printf '%s' "$p"; return; }
  done
  printf ''
}

# --- release vigente (la v1 usaba un tag inexistente) ---------------------
# Devuelve "<tag>|<url_binario>|<url_sha256>"
release_info() {
  j="$("$CURL" -fsSL -m 25 "$API_LATEST" 2>/dev/null)"
  [ -z "$j" ] && { printf ''; return; }
  tag="$(printf '%s' "$j" | grep -o '"tag_name"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 | sed 's/.*"\([^"]*\)"$/\1/')"
  ub="$(printf '%s' "$j" | grep -o '"browser_download_url"[[:space:]]*:[[:space:]]*"[^"]*megadaemon-core-android-arm64"' | head -1 | sed 's/.*"\(https[^"]*\)"$/\1/')"
  us="$(printf '%s' "$j" | grep -o '"browser_download_url"[[:space:]]*:[[:space:]]*"[^"]*megadaemon-core-android-arm64\.sha256"' | head -1 | sed 's/.*"\(https[^"]*\)"$/\1/')"
  [ -z "$ub" ] && ub="$REL_BASE/megadaemon-core-android-arm64"
  [ -z "$us" ] && us="$REL_BASE/megadaemon-core-android-arm64.sha256"
  [ -z "$tag" ] && return
  printf '%s|%s|%s' "$tag" "$ub" "$us"
}

# --- sha256 del release (el que quiero) -----------------------------------
want_sha() {
  "$CURL" -fsSL -m 60 "$1" 2>/dev/null | cut -d' ' -f1 | tr -d '\r\n'
}

# --- el binario es realmente un ELF aarch64? (la v1 hacia un no-op) ------
elf_ok() {
  h="$(od -An -tx1 -N20 "$1" 2>/dev/null | tr -d ' \n')"
  [ "${h:0:8}" = "7f454c46" ] || return 1
  [ "${h:36:4}" = "b700" ] || return 1
  return 0
}

# --- (re)arrancar el daemon ------------------------------------------------
# [v2] la v1 hacia `pkill` y esperaba a que "alguien" lo relanzara: en el
# movil el supervisor es un script de service.d => corre SOLO en boot => tras
# el pkill NO lo relanza nadie => el movil se queda sin daemon.  Aqui se
# relanza EXPLICITAMENTE.
reiniciar_daemon() {
  root 'pkill -x megadaemon-core' 2>/dev/null || true
  sleep 2
  root "sh $SUP" >/dev/null 2>&1 || true
  i=0
  while [ $i -lt 20 ]; do
    sleep 1
    d="$(daemon_pid)"
    [ -n "$d" ] && { DAEMON="$d"; say "daemon arrancado (pid $d)"; return 0; }
    i=$((i+1))
  done
  [ -n "${ROOTSTATE:-}" ] && root "MEGADAEMON_STATE_DIR=$ROOTSTATE $BIN >/dev/null 2>&1 &" >/dev/null
  i=0
  while [ $i -lt 20 ]; do
    sleep 1
    d="$(daemon_pid)"
    [ -n "$d" ] && { DAEMON="$d"; say "daemon arrancado a pelo (pid $d)"; return 0; }
    i=$((i+1))
  done
  return 1
}

# --- instalar una build (verificada) con rollback --------------------------
instalar_build() {
  ub="$1"; us="$2"; tag="$3"
  cd "$MD_DIR" || die "no existe $MD_DIR"
  [ -x "$CURL" ] || die "no hay curl (Termux): $CURL"
  say "descargando build $tag"
  "$CURL" -fsSL -m 300 -o megadaemon-core.new "$ub" || die "no pude descargar el binario ($ub)"
  # [v2] el sha256 es OBLIGATORIO: la v1 hacia `|| true` y si fallaba no
  # verificaba nada (instalaba sin comprobar).
  "$CURL" -fsSL -m 60 -o megadaemon-core.sha256.new "$us" || die "no pude descargar el sha256 ($us)"
  EXPECT="$(cut -d' ' -f1 < megadaemon-core.sha256.new 2>/dev/null | tr -d '\r\n')"
  [ -n "$EXPECT" ] || { rm -f megadaemon-core.new; die "el .sha256 no trae hash => no instalo nada"; }
  GOT="$(sha256sum megadaemon-core.new | cut -d' ' -f1)"
  [ "$EXPECT" = "$GOT" ] || { rm -f megadaemon-core.new; die "sha256 del release ($EXPECT) != el descargado ($GOT)"; }
  elf_ok megadaemon-core.new || { rm -f megadaemon-core.new; die "no es un ELF aarch64 => no instalo nada"; }
  say "sha256 verificado ($GOT) y ELF aarch64 OK"

  # [v2] el backup se hace del binario que NO esta corriendo (el daemon se
  # para DESPUES): con el daemon vivo, `cp` da "Text file busy" y el backup
  # queda vacio => el rollback no tendria nada que restaurar.
  OLD="$(sha256sum "$BIN" 2>/dev/null | cut -d' ' -f1)"
  root "cp -f $BIN $BIN.bak" 2>/dev/null || true
  root "mv -f $MD_DIR/megadaemon-core.new $BIN; chmod 755 $BIN; test -x $BIN && echo OK" >/dev/null
  [ "$(root "test -x $BIN && echo SI")" = "SI" ] || { root "cp -f $BIN.bak $BIN"; die "el binario no quedo ejecutable: restaurado el anterior"; }
  say "binario instalado (+x). Anterior: ${OLD:-ninguno}"

  reiniciar_daemon || { root "cp -f $BIN.bak $BIN; chmod 755 $BIN"; reiniciar_daemon; die "el daemon no arranco: binario anterior restaurado"; }
  sleep 6
  p="$(health_local)"
  if [ -z "$p" ]; then
    warn "el daemon arranca pero /health no responde => rollback"
    root "cp -f $BIN.bak $BIN; chmod 755 $BIN"; reiniciar_daemon
    die "rollback hecho: vuelve el binario anterior"
  fi
  say "salud OK en 127.0.0.1:$p"
  return 0
}

# --- credencial en un solo sitio -------------------------------------------
unificar_credencial() {
  src="$(find_key)"
  [ -n "$src" ] || die "no encuentro la key en $STATE_REAL ni en $STATE_DEFAULT ni en $MD_DIR"
  KEYFP="$(sha16 "$(root "cat $src" | tr -d '\r\n')")"
  say "key encontrada en $src (sha16 $KEYFP)"
  root "mkdir -p $ROOTSTATE && cp -f $src $ROOTSTATE/megadaemon_api.key && chmod 600 $ROOTSTATE/megadaemon_api.key" >/dev/null
  # [v2] la v1 exportaba MEGADAEMON_STATE_DIR AL FINAL del supervisor => no
  # tenia efecto (el daemon ya se habia arrancado).  Aqui va al PRINCIPIO.
  root "touch $SUP" >/dev/null 2>&1 || true
  if ! root "grep -q '^export MEGADAEMON_STATE_DIR=' $SUP" >/dev/null 2>&1; then
    root "sed -i '/^export MEGADAEMON_STATE_DIR=/d' $SUP; sed -i '1a export MEGADAEMON_STATE_DIR=$ROOTSTATE' $SUP" >/dev/null
  fi
  root "chmod 755 $SUP" >/dev/null 2>&1 || true
  say "state_dir unificado: $ROOTSTATE"
}

# --- un solo cloudflared ----------------------------------------------------
tunel() {
  cf="$(root 'pgrep -x cloudflared | wc -l' | tr -d ' \r\n')"
  say "cloudflared: ${cf:-0} proceso(s)"
  if [ "${cf:-0}" -gt 1 ] 2>/dev/null; then
    warn "varios cloudflared: cada uno con su hostname. No los mato (cortar el canal es peor)."
  fi
  TURL="$(root 'cat /sdcard/megadaemon.url 2>/dev/null; cat /data/local/tmp/megadaemon/tunnel.url 2>/dev/null' | tr -d '\r\n' | grep -o 'https://[a-z0-9.-]*' | tail -1)"
  [ -n "$TURL" ] && say "URL vigente: $TURL (sha16 $(sha16 "$TURL"))" || warn "no encuentro la URL del tunel"
}

# ===========================================================================
# MODO: VERIFICAR
# ===========================================================================
diagnosticar() {
  ROOTSTATE="$(state_dir_real)"
  DAEMON="$(daemon_pid)"
  src="$(find_key)"
  KEYFP=""; [ -n "$src" ] && KEYFP="$(sha16 "$(root "cat $src" | tr -d '\r\n')")"
  GOT_LOCAL="$(sha256sum "$BIN" 2>/dev/null | cut -d' ' -f1)"
  info="$(release_info)"
  TAG="$(printf '%s' "$info" | cut -d'|' -f1)"
  HEALTH="$(health_local)"
  [ -n "$HEALTH" ] && HEALTH="OK en 127.0.0.1:$HEALTH" || HEALTH="SIN RESPUESTA"
  tunel >/dev/null 2>&1 || true
  say "state_dir: $ROOTSTATE | daemon: ${DAEMON:-NO} | key: ${KEYFP:-<ninguna>}"
  say "binario: ${GOT_LOCAL:-<no instalado>} | release vigente: ${TAG:-?}"
  say "salud: $HEALTH"
  estado
  say "resumen: si keys=0 o la key no esta en $ROOTSTATE => 403 para todos (no es caducidad)"
  say "         si salud=SIN RESPUESTA => el daemon no vive (el watchdog lo relanza)"
}

if [ "$MODO" = verificar ] && [ "$WATCH" = 0 ] && [ "$INSTALAR_WD" = 0 ]; then
  diagnosticar
  exit 0
fi

# --- diagnostico sin root (solo lectura) -----------------------------------
diagnostico_sin_root() {
  d="$(pgrep -x megadaemon-core 2>/dev/null | head -1 | tr -d ' \r\n')"
  h="$(health_local)"
  say "  daemon: ${d:-NO} | salud: ${h:-SIN_RESPUESTA}"
  for f in "$STATE_REAL/megadaemon_api.key" "$STATE_DEFAULT/megadaemon_api.key"; do
    if [ -r "$f" ]; then say "  key en $f (sha16 $(sha16 "$(cat "$f")"))"; fi
  done
  say "  (para arreglarlo de verdad: ejecuta esto con root, p.ej. 'su -mm -c \"sh fix-movil.sh\"')"
}

# ===========================================================================
# MODO: INSTALAR EL WATCHDOG COMO SERVICIO (sobrevive a cerrar Termux)
# ===========================================================================
if [ "$INSTALAR_WD" = 1 ]; then
  mkdir -p "$MD_DIR" 2>/dev/null || true
  cp "$0" "$MD_DIR/fix-movil.sh" 2>/dev/null || true
  chmod 755 "$MD_DIR/fix-movil.sh" 2>/dev/null || true
  cat >"$WDSVC" <<EOF
#!/system/bin/sh
# Watchdog del MegaDaemon (lo genera fix-movil.sh): comprueba que el daemon
# vive, que /health responde y si hay BUILD NUEVA; la instala solo.
/data/data/com.termux/files/usr/bin/sh $MD_DIR/fix-movil.sh --watch $INTERVALO >>$LOG 2>&1
EOF
  chmod 755 "$WDSVC" 2>/dev/null || true
  say "watchdog instalado en $WDSVC (se ejecuta en cada boot)"
  nohup /data/data/com.termux/files/usr/bin/sh "$MD_DIR/fix-movil.sh" --watch "$INTERVALO" >>"$LOG" 2>&1 &
  say "watchdog arrancado (cada ${INTERVALO}s). Log: $LOG | Estado: $ST"
  exit 0
fi

# ===========================================================================
# MODO: ARREGLAR (una vez)
# ===========================================================================
if [ "$HAVE_ROOT" != "1" ]; then
  say "SIN ROOT: no puedo tocar nada (hace falta 'su -mm' o ejecutarlo como root)."
  say "  el estado actual, sin cambios:"
  diagnostico_sin_root
  exit 0
fi
take_lock || { say "ya hay otro fix-movil.sh corriendo (lock: $LOCK) => no hago nada"; exit 0; }
mkdir -p "$MD_DIR" 2>/dev/null || true
say "=== 0) contexto ==="
ROOTSTATE="$(state_dir_real)"
say "state_dir que usa el daemon: $ROOTSTATE"
DAEMON="$(daemon_pid)"
say "daemon: ${DAEMON:-NO}"
unificar_credencial
info="$(release_info)"
TAG="$(printf '%s' "$info" | cut -d'|' -f1)"
UB="$(printf '%s' "$info" | cut -d'|' -f2)"
US="$(printf '%s' "$info" | cut -d'|' -f3)"
say "release vigente: ${TAG:-<sin acceso a la API de GitHub>}"
GOT_LOCAL="$(sha256sum "$BIN" 2>/dev/null | cut -d' ' -f1)"
[ -n "$TAG" ] && [ -n "$GOT_LOCAL" ] && say "binario local: $GOT_LOCAL"
# [v2] aqui se comprueba si hay BUILD NUEVA (la v1 no lo miraba: reinstalaba
# siempre).  Si es la misma => no se descarga nada (idempotente y sin gastar
# datos).
if [ -n "$TAG" ] && [ -n "$UB" ]; then
  want="$(want_sha "$US")"
  if [ -n "$want" ] && [ "$want" = "$GOT_LOCAL" ]; then
    say "la build instalada YA es la vigente ($TAG) => no descargo nada"
  else
    instalar_build "$UB" "$US" "$TAG" || die "no pude instalar la build nueva"
    GOT_LOCAL="$want"
  fi
else
  warn "sin info de release (sin red?): no compruebo build nueva, solo verifico lo que hay"
fi
reiniciar_daemon || warn "el daemon no arranco (puede que ya lo este otro)"
HEALTH="$(health_local)"
[ -n "$HEALTH" ] && HEALTH="OK en 127.0.0.1:$HEALTH" || HEALTH="SIN RESPUESTA"
tunel
NOTAS="build ${TAG:-?}; salud $HEALTH"
estado
say "ESTADO (3 lineas):"
say "  daemon pid ${DAEMON:-NO} | binario sha256 ${GOT_LOCAL:-?}"
say "  credencial unificada en $ROOTSTATE (sha16 ${KEYFP:-?})"
say "  salud $HEALTH | release ${TAG:-?}"
free_lock
[ "$WATCH" = 1 ] || exit 0

# ===========================================================================
# MODO: WATCHDOG (bucle) — comprueba TODO y repara SOLO lo que hace falta
# ===========================================================================
say "=== WATCHDOG: cada ${INTERVALO}s (Ctrl-C para salir) ==="
while :; do
  sleep "$INTERVALO"
  take_lock || { log "watchdog: otro fix-movil.sh corriendo => salto este ciclo"; continue; }
  ROOTSTATE="$(state_dir_real)"
  DAEMON="$(daemon_pid)"
  HEALTH="$(health_local)"
  if [ -z "$DAEMON" ] || [ -z "$HEALTH" ]; then
    log "watchdog: daemon=${DAEMON:-NO} salud=${HEALTH:-SIN_RESPUESTA} => lo relanzo"
    say "watchdog: daemon caido => relanzo"
    reiniciar_daemon >>"$LOG" 2>&1 || log "watchdog: no se pudo relanzar"
  else
    log "watchdog: todo OK (pid $DAEMON, salud $HEALTH)"
  fi
  info="$(release_info)"
  TAG="$(printf '%s' "$info" | cut -d'|' -f1)"
  UB="$(printf '%s' "$info" | cut -d'|' -f2)"
  US="$(printf '%s' "$info" | cut -d'|' -f3)"
  if [ -n "$UB" ]; then
    GOT_LOCAL="$(sha256sum "$BIN" 2>/dev/null | cut -d' ' -f1)"
    want="$(want_sha "$US")"
    if [ -n "$want" ] && [ "$want" != "$GOT_LOCAL" ]; then
      log "watchdog: build nueva $TAG => instalo"
      say "watchdog: build nueva ($TAG) => instalo"
      instalar_build "$UB" "$US" "$TAG" >>"$LOG" 2>&1 || log "watchdog: la instalacion fallo (rollback aplicado)"
    fi
  fi
  HEALTH="$(health_local)"
  NOTAS="watchdog; build ${TAG:-?}; salud ${HEALTH:-SIN_RESPUESTA}"
  estado
  free_lock
done
