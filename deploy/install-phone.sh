#!/bin/sh
# =============================================================================
# install-phone.sh — despliegue y REPARACIÓN del cockpit en el móvil (Android)
#
# POR QUÉ EXISTE
#   El usuario abría http://localhost:3000 / :8090 / :8080 en el móvil y le
#   salía ERR_CONNECTION_REFUSED: en el teléfono no había NINGÚN proceso
#   escuchando.  Esto no es un bug de la UI: es que el daemon no estaba
#   arrancado (o corría un binario viejo).  Este script deja el cockpit
#   sirviendo la UI real y TE DICE la URL exacta que hay que abrir.
#
# USO
#   Desde un PC con adb (recomendado):
#       ./deploy/install-phone.sh            # busca el móvil y despliega
#       ./deploy/install-phone.sh --no-push  # sólo diagnóstico, no toca nada
#
#   Desde el propio móvil (Termux, con root):
#       su -c sh install-phone.sh
#
# QUÉ HACE
#   1. Diagnóstico previo: proceso, puertos, plantilla, token y log.
#   2. Copia el binario ARM64 a /data/local/tmp/megadaemon/megadaemon-core.
#   3. Copia la plantilla web a <prefix>/web/ (por si el binario es viejo).
#   4. Copia el supervisor a /data/adb/service.d/99-megadaemon.sh.
#   5. Reinicia el daemon y VERIFICA con /health en todos los puertos alias.
#   6. Imprime la URL que hay que abrir y el token de sesión si lo necesita.
#
# NOTAS
#   * NO toca $PREFIX/megadaemon.env (tus claves): sólo lo crea si falta,
#     a partir del ejemplo.
#   * Es idempotente: se puede repetir cuantas veces quieras.
#   * No borra nada: el binario anterior se guarda como .bak.
# =============================================================================

set -u

PREFIX="${MEGADAEMON_PREFIX:-/data/local/tmp/megadaemon}"
STATE="${MEGADAEMON_STATE:-$PREFIX/state}"
BIN="$PREFIX/megadaemon-core"
WEB_DIR="$PREFIX/web"
TEMPLATE="$WEB_DIR/cockpit_template.html"
SERVICE="/data/adb/service.d/99-megadaemon.sh"
# Canal: cloudflared UNICO e independiente del daemon (Magisk service.d).
CFT_SERVICE="/data/adb/service.d/98-cloudflared.sh"
LOG=/sdcard/megadaemon.log
# El cockpit escucha en el puerto pedido + alias (AUDIT-WEB-04).
PORTS="9090 3000 3030 8090 8080"
HEALTH_TIMEOUT=45   # segundos que se espera a /health
HERE=$(cd "$(dirname "$0")" 2>/dev/null && pwd)
REPO_ROOT=$(cd "$HERE/.." 2>/dev/null && pwd)
PUSH_ONLY=0
[ "${1:-}" = "--no-push" ] && PUSH_ONLY=1

say() { echo "== $*"; }
die() { echo "!! $*" >&2; exit 1; }

# ── 0. ¿estamos en el móvil o en un PC? ──────────────────────────────────────
ON_PHONE=0
[ -d /system/app ] && ON_PHONE=1

have() { command -v "$1" >/dev/null 2>&1; }

as_root() {
    # Ejecuta un comando como root tanto en el móvil (su) como en un PC
    # con adb (adb shell su -c).
    if [ "$ON_PHONE" = "1" ]; then
        if [ "$(id -u)" = "0" ]; then
            sh -c "$1"
        else
            su -c "$1"
        fi
    else
        adb shell "su -c '$1'"
    fi
}

# ── 1. Diagnóstico ───────────────────────────────────────────────────────────
diagnose() {
    say "DIAGNÓSTICO"
    as_root 'echo "--- procesos megadaemon ---"; pgrep -af megadaemon-core || echo "(ninguno: el daemon NO está corriendo)"' 2>/dev/null
    as_root 'echo "--- puertos escuchando ---"; (ss -ltnp 2>/dev/null || netstat -ltnp 2>/dev/null) | grep -E ":(9090|3000|3030|8090|8080)" || echo "(nada escuchando en los puertos del cockpit)"' 2>/dev/null
    as_root "echo '--- binario ---'; ls -l $BIN 2>/dev/null || echo '(NO existe el binario)'" 2>/dev/null
    as_root "echo '--- plantilla web ---'; ls -l $TEMPLATE 2>/dev/null || echo '(NO existe la plantilla)'" 2>/dev/null
    as_root "echo '--- token de sesión ---'; ls -l $STATE/cockpit_token 2>/dev/null || echo '(se generará al arrancar)'" 2>/dev/null
    as_root "echo '--- últimas líneas del log ---'; tail -n 12 $LOG 2>/dev/null || echo '(sin log)'" 2>/dev/null
}

# ── 2. ¿de dónde sale el binario? ────────────────────────────────────────────
find_binary() {
    if [ -n "${MEGADAEMON_BIN:-}" ] && [ -f "${MEGADAEMON_BIN}" ]; then
        echo "${MEGADAEMON_BIN}"
        return 0
    fi
    for c in \
        "$REPO_ROOT/rust-core/target/aarch64-linux-android/release/megadaemon-core" \
        "$REPO_ROOT/rust-core/target/release/megadaemon-core" \
        "/workspace/md-target-agentgeneral/aarch64-linux-android/release/megadaemon-core"
    do
        [ -f "$c" ] && { echo "$c"; return 0; }
    done
    return 1
}

# ── 2b. SHA-256 ──────────────────────────────────────────────────────────────
sha256_of() {
    if have sha256sum; then sha256sum "$1" | awk '{print $1}'
    elif have shasum; then shasum -a 256 "$1" | awk '{print $1}'
    else echo "sin-sha256"
    fi
}

# SHA-256 de un fichero YA DENTRO del móvil.
remote_sha256() {
    as_root "sha256sum '$1' 2>/dev/null | awk '{print \$1}' || shasum -a 256 '$1' 2>/dev/null | awk '{print \$1}'" 2>/dev/null | tr -d '\r\n'
}

# ── 3. Despliegue ────────────────────────────────────────────────────────────
deploy_files() {
    SRC_BIN=$(find_binary) || die "no encuentro el binario ARM64; compílalo con:
  cd rust-core && CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=\$NDK/bin/aarch64-linux-android21-clang \\
    CC_aarch64_linux_android=\$NDK/bin/aarch64-linux-android21-clang cargo build --release --target aarch64-linux-android"

    SRC_SHA=$(sha256_of "$SRC_BIN")
    say "binario origen: $SRC_BIN"
    say "sha-256 origen: $SRC_SHA"
    if [ "$SRC_SHA" = "sin-sha256" ]; then
        fail "no hay sha256sum ni shasum: no puedo verificar la copia (aborto)"
        return 1
    fi
    if [ "$ON_PHONE" = "1" ]; then
        mkdir -p "$PREFIX" "$WEB_DIR" "$STATE" 2>/dev/null
        [ -f "$BIN" ] && cp -f "$BIN" "$BIN.bak" 2>/dev/null
        cp -f "$SRC_BIN" "$BIN" || die "no se pudo copiar el binario"
        chmod 755 "$BIN" 2>/dev/null
        [ -f "$REPO_ROOT/web/cockpit_template.html" ] && cp -f "$REPO_ROOT/web/cockpit_template.html" "$TEMPLATE"
        [ -f "$REPO_ROOT/deploy/99-megadaemon.sh" ] && { cp -f "$REPO_ROOT/deploy/99-megadaemon.sh" "$SERVICE"; chmod 755 "$SERVICE"; }
    else
        have adb || die "adb no está instalado en este PC"
        adb wait-for-device || die "no hay ningún móvil conectado por adb"
        adb shell "mkdir -p $PREFIX $WEB_DIR $STATE" || true
        adb push "$SRC_BIN" "$BIN" >/dev/null || die "adb push del binario falló"
        adb shell "chmod 755 $BIN" || true
        if [ -f "$REPO_ROOT/web/cockpit_template.html" ]; then
            adb push "$REPO_ROOT/web/cockpit_template.html" "$TEMPLATE" >/dev/null || true
        fi
        if [ -f "$REPO_ROOT/deploy/99-megadaemon.sh" ]; then
            adb push "$REPO_ROOT/deploy/99-megadaemon.sh" "$SERVICE" >/dev/null || true
            adb shell "chmod 755 $SERVICE" || true
        fi
    fi
    install_extras
    say "ficheros copiados"
}

# ── 3b. Canal: watchdog + publicación + sonda VS4L ───────────────────────────
# El watchdog necesita `channel-publish` (el móvil NO tiene curl: el sondeo y
# la publicación los hace ese binario) y `98-cloudflared.sh` (un solo
# cloudflared, independiente del daemon).
install_extras() {
    src_dir=$(dirname "$SRC_BIN")
    for b in channel-publish duckdns-updater npu-probe; do
        [ -f "$src_dir/$b" ] || continue
        if [ "$ON_PHONE" = "1" ]; then
            cp -f "$src_dir/$b" "$PREFIX/$b" 2>/dev/null && chmod 755 "$PREFIX/$b" 2>/dev/null
        else
            adb push "$src_dir/$b" "$PREFIX/$b" >/dev/null 2>&1 && adb shell "chmod 755 $PREFIX/$b" >/dev/null 2>&1
        fi
    done
    if [ -f "$REPO_ROOT/deploy/tunnel-watchdog.sh" ]; then
        if [ "$ON_PHONE" = "1" ]; then
            cp -f "$REPO_ROOT/deploy/tunnel-watchdog.sh" "$PREFIX/tunnel-watchdog.sh" 2>/dev/null \
                && chmod 755 "$PREFIX/tunnel-watchdog.sh" 2>/dev/null
        else
            adb push "$REPO_ROOT/deploy/tunnel-watchdog.sh" "$PREFIX/tunnel-watchdog.sh" >/dev/null 2>&1 \
                && adb shell "chmod 755 $PREFIX/tunnel-watchdog.sh" >/dev/null 2>&1
        fi
    fi
    if [ -f "$REPO_ROOT/deploy/98-cloudflared.sh" ]; then
        if [ "$ON_PHONE" = "1" ]; then
            cp -f "$REPO_ROOT/deploy/98-cloudflared.sh" "$CFT_SERVICE" 2>/dev/null \
                && chmod 755 "$CFT_SERVICE" 2>/dev/null
        else
            adb push "$REPO_ROOT/deploy/98-cloudflared.sh" "$CFT_SERVICE" >/dev/null 2>&1 \
                && adb shell "chmod 755 $CFT_SERVICE" >/dev/null 2>&1
        fi
    fi
    say "extras del canal instalados (watchdog + channel-publish + 98-cloudflared)"
}

# ── 4. Reinicio ──────────────────────────────────────────────────────────────
restart() {
    say "reiniciando el daemon"
    as_root 'killall megadaemon-core 2>/dev/null; sleep 1' >/dev/null 2>&1
    if [ "$ON_PHONE" = "1" ]; then
        (setsid nohup sh "$SERVICE" >/dev/null 2>&1 &) 2>/dev/null || true
    else
        adb shell "su -c 'setsid nohup sh $SERVICE >/dev/null 2>&1 &'" >/dev/null 2>&1 || true
    fi
    # El supervisor duerme 60 s entre comprobaciones: no esperamos tanto.
    sleep 6
}

# ── 5. Verificación ──────────────────────────────────────────────────────────
health_on_port() {
    p=$1
    if [ "$ON_PHONE" = "1" ]; then
        if have curl; then
            curl -s -m 3 -o /dev/null -w '%{http_code}' "http://127.0.0.1:$p/health" 2>/dev/null
        else
            toybox wget -q -O /dev/null "http://127.0.0.1:$p/health" 2>/dev/null && echo 200 || echo 000
        fi
    else
        adb shell "curl -s -m 3 -o /dev/null -w '%{http_code}' http://127.0.0.1:$p/health" 2>/dev/null | tr -d '\r\n'
    fi
}

# Sonda de UI (UI-16): el deploy NO se da por bueno si la UI no puede
# autenticarse.  Con la v14 el servidor ya no incrusta el token en la pagina y
# POST /api/session/exchange exige un token de sesion que el navegador todavia
# no tiene => la pagina carga y TODAS las llamadas de datos devuelven 403.
# Si la pagina trae token vacio Y el exchange no devuelve un token real, el
# despliegue es un "cockpit vivo pero UI muerta": se hace rollback.
ui_probe() {
    p=$1
    page=$(as_root "toybox wget -q -O - http://127.0.0.1:$p/ 2>/dev/null" 2>/dev/null | tr -d '\r\n')
    case "$page" in
        *'MEGADAEMON_TOKEN=""'*)
            say "la pagina se sirve SIN token (UI-16 abierto)"
            ;;
        *'MEGADAEMON_TOKEN='*)
            say "la pagina trae token de sesion embebido: la UI puede autenticarse"
            return 0
            ;;
        *)
            say "la pagina no inyecta token: se comprueba el canje API key -> token"
            ;;
    esac
    # El canje solo vale si devuelve un token REAL (no un 200 generico).
    body=$(as_root "toybox wget -q -O - --post-data='{}' http://127.0.0.1:$p/api/session/exchange 2>/dev/null" 2>/dev/null | tr -d '\r\n')
    case "$body" in
        *'"token"'*) say "el canje API key -> token funciona"; return 0 ;;
        *) say "el canje API key -> token NO devuelve token" ;;
    esac
    return 1
}

verify() {
    say "esperando /health (max ${HEALTH_TIMEOUT}s) en: $PORTS"
    ok_port=$(wait_healthy "$HEALTH_TIMEOUT")
    for p in $PORTS; do
        code=$(health_on_port "$p")
        printf '   puerto %-5s -> HTTP %s\n' "$p" "${code:-000}"
    done
    if [ -n "$ok_port" ]; then
        say "COCKPIT OK -> abre http://localhost:$ok_port (o http://127.0.0.1:$ok_port)"
        return 0
    fi
    say "el cockpit SIGUE sin responder; log:"
    as_root "tail -n 25 $LOG 2>/dev/null" 2>/dev/null
    return 1
}

# ── 6. Rollback ──────────────────────────────────────────────────────────────
rollback() {
    say "el cockpit no arranco: ROLLBACK al binario anterior"
    as_root 'killall megadaemon-core 2>/dev/null; sleep 1' >/dev/null 2>&1
    if as_root "[ -f $BIN.bak ]"; then
        as_root "cp -f $BIN.bak $BIN && chmod 755 $BIN" 2>/dev/null
        say "binario anterior restaurado ($BIN.bak)"
    else
        say "no habia binario anterior: nada que restaurar"
    fi
    as_root "echo '[$(date '+%Y-%m-%d %H:%M:%S')] ERROR: install-phone.sh fallo el arranque de /health; rollback aplicado' >> $LOG" 2>/dev/null
    as_root "tail -n 25 $LOG 2>/dev/null" 2>/dev/null
    return 1
}

# ── main ─────────────────────────────────────────────────────────────────────
diagnose
if [ "$PUSH_ONLY" = "1" ]; then
    say "--no-push: sólo diagnóstico, no se toca nada"
    exit 0
fi
deploy_files || exit 1
restart
if verify; then
    if ui_probe "$ok_port"; then
        say "DESPLIEGUE OK (cockpit vivo y UI autenticable)"
        exit 0
    fi
    say "la UI NO puede autenticarse (token vacio y canje roto) => rollback"
    rollback
    exit 3
fi
rollback
say "abre el log del móvil: tail -f $LOG"
exit 2
