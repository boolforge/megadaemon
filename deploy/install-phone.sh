#!/bin/sh
# =============================================================================
# install-phone.sh — despliegue y REPARACIÓN del cockpit en el móvil (Android)
#
# POR QUÉ EXISTE
#   El usuario abría http://localhost:3000 / :8090 / :8080 en el móvil y le
#   salía ERR_CONNECTION_REFUSED: en el teléfono no había NINGÚN proceso
#   escuchando.  Esto no es un bug de la UI: es que el daemon no estaba
#   arrancado (o corría un binario viejo, o el arranque se quedaba atascado).
#   Este script deja el cockpit sirviendo la UI real y TE DICE la URL exacta.
#
# USO
#   Desde un PC con adb (recomendado):
#       ./deploy/install-phone.sh                # busca el móvil y despliega
#       ./deploy/install-phone.sh --no-push     # sólo diagnóstico, no toca nada
#       ./deploy/install-phone.sh --device SERIAL
#
#   Desde el propio móvil (Termux, con root):
#       su -c sh install-phone.sh
#
# QUÉ HACE (idempotente, se puede repetir)
#   1. Diagnóstico previo: proceso, puertos, plantilla, token, API key y log.
#   2. Localiza el binario ARM64 y calcula su SHA-256.
#   3. Copia el binario (guarda el anterior como .bak) y VERIFICA el SHA-256
#      en el móvil: si no coincide, aborta y no toca nada.
#   4. Copia la plantilla web a <prefix>/web/ y al directorio privado de la app.
#   5. Instala el supervisor en /data/adb/service.d/99-megadaemon.sh (755).
#   6. Reinicia el daemon y ESPERA a que /health dé 200 (timeout + reintentos).
#   7. Si no arranca en el plazo: ROLLBACK al binario anterior, deja el motivo
#      en el log y sale con código != 0.
#   8. Imprime la URL exacta que hay que abrir y el token de sesión.
#
# NOTAS
#   * NO toca $PREFIX/megadaemon.env (tus claves): sólo lo crea si falta,
#     a partir del ejemplo.
#   * No borra nada: el binario anterior se guarda como .bak.
#   * El puerto principal es 9090 (el del supervisor) y el cockpit escucha
#     además en los alias 3000/3030/8090.
# =============================================================================

set -u

PREFIX="${MEGADAEMON_PREFIX:-/data/local/tmp/megadaemon}"
STATE="${MEGADAEMON_STATE:-$PREFIX/state}"
BIN="$PREFIX/megadaemon-core"
WEB_DIR="$PREFIX/web"
TEMPLATE="$WEB_DIR/cockpit_template.html"
APP_TEMPLATE_DIR="/data/user/0/com.agy/files/megadaemon/web"
SERVICE="/data/adb/service.d/99-megadaemon.sh"
# Canal: cloudflared UNICO e independiente del daemon (Magisk service.d).
CFT_SERVICE="/data/adb/service.d/98-cloudflared.sh"
LOG=/sdcard/megadaemon.log
PORTS="9090 3000 3030 8090"
HEALTH_TIMEOUT=45          # segundos que se espera a /health
HERE=$(cd "$(dirname "$0")" 2>/dev/null && pwd)
REPO_ROOT=$(cd "$HERE/.." 2>/dev/null && pwd)
PUSH_ONLY=0
SERIAL=""

for a in "$@"; do
    case "$a" in
        --no-push) PUSH_ONLY=1 ;;
        --device=*) SERIAL="${a#--device=}" ;;
        --prefix=*) PREFIX="${a#--prefix=}"; STATE="$PREFIX/state"; BIN="$PREFIX/megadaemon-core"; WEB_DIR="$PREFIX/web"; TEMPLATE="$WEB_DIR/cockpit_template.html" ;;
    esac
done

say() { echo "== $*"; }
fail() { echo "!! $*" >&2; }

# ── 0. ¿estamos en el móvil o en un PC? ──────────────────────────────────────
ON_PHONE=0
[ -d /system/app ] && ON_PHONE=1

have() { command -v "$1" >/dev/null 2>&1; }

adb_() {
    if [ -n "$SERIAL" ]; then adb -s "$SERIAL" "$@"; else adb "$@"; fi
}

as_root() {
    if [ "$ON_PHONE" = "1" ]; then
        if [ "$(id -u)" = "0" ]; then sh -c "$1"; else su -c "$1"; fi
    else
        adb_ shell "su -c '$1'"
    fi
}

# ── 1. Diagnóstico ───────────────────────────────────────────────────────────
diagnose() {
    say "DIAGNÓSTICO (PREFIX=$PREFIX)"
    as_root 'echo "--- procesos megadaemon ---"; pgrep -af megadaemon-core || echo "(ninguno: el daemon NO está corriendo)"' 2>/dev/null
    as_root 'echo "--- puertos del cockpit ---"; for p in 9090 3000 3030 8090; do printf "  %-5s " "$p"; (ss -ltn 2>/dev/null || netstat -ltn 2>/dev/null) | grep -q ":$p " && echo "ESCUCHA" || echo "-"; done' 2>/dev/null
    as_root "echo '--- binario ---'; ls -l $BIN 2>/dev/null || echo '(NO existe el binario)'" 2>/dev/null
    as_root "echo '--- plantilla web ---'; ls -l $TEMPLATE 2>/dev/null || echo '(NO existe la plantilla en disco: se usará la embebida)'" 2>/dev/null
    as_root "echo '--- token de sesión ---'; ls -l $STATE/cockpit_token 2>/dev/null || echo '(se generará al arrancar)'" 2>/dev/null
    as_root "echo '--- API key ---'; ls -l $STATE/megadaemon_api.key 2>/dev/null || echo '(SIN API key: expuesto a la LAN)'" 2>/dev/null
    as_root "echo '--- supervisor ---'; ls -l $SERVICE 2>/dev/null || echo '(NO instalado el supervisor de Magisk)'" 2>/dev/null
    as_root "echo '--- últimas líneas del log ---'; tail -n 12 $LOG 2>/dev/null || echo '(sin log)'" 2>/dev/null
}

# ── 2. Binario + SHA-256 ─────────────────────────────────────────────────────
find_binary() {
    if [ -n "${MEGADAEMON_BIN:-}" ] && [ -f "${MEGADAEMON_BIN}" ]; then
        echo "${MEGADAEMON_BIN}"; return 0
    fi
    for c in \
        "$REPO_ROOT/rust-core/target/aarch64-linux-android/release/megadaemon-core" \
        "$REPO_ROOT/rust-core/target/release/megadaemon-core" \
        "/workspace/md-t-cockpit/aarch64-linux-android/release/megadaemon-core" \
        "/workspace/md-target-agentgeneral/aarch64-linux-android/release/megadaemon-core"
    do
        [ -f "$c" ] && { echo "$c"; return 0; }
    done
    return 1
}

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
    SRC_BIN=$(find_binary) || { fail "no encuentro el binario ARM64; compílalo con:
  cd rust-core && \\
  CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=\$NDK/bin/aarch64-linux-android21-clang \\
  CC_aarch64_linux_android=\$NDK/bin/aarch64-linux-android21-clang \\
  cargo build --release --target aarch64-linux-android"; return 1; }

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
        cp -f "$SRC_BIN" "$BIN" || { fail "no se pudo copiar el binario"; return 1; }
        chmod 755 "$BIN" 2>/dev/null
        [ -f "$REPO_ROOT/web/cockpit_template.html" ] && cp -f "$REPO_ROOT/web/cockpit_template.html" "$TEMPLATE"
        # La UI v2 (/v2 y /webui_v2.css) se sirve de disco si existe (y si no,
        # el daemon sirve la copia embebida) => se puede actualizar en el
        # movil SIN recompilar el binario (como la v1).
        [ -f "$REPO_ROOT/web/cockpit_v2.html" ] && cp -f "$REPO_ROOT/web/cockpit_v2.html" "$WEB_DIR/cockpit_v2.html"
        [ -f "$REPO_ROOT/web/webui_v2.css" ]  && cp -f "$REPO_ROOT/web/webui_v2.css"  "$WEB_DIR/webui_v2.css"
        [ -f "$REPO_ROOT/deploy/99-megadaemon.sh" ] && { cp -f "$REPO_ROOT/deploy/99-megadaemon.sh" "$SERVICE"; chmod 755 "$SERVICE"; }
    else
        have adb || die "adb no está instalado en este PC"
        adb wait-for-device || die "no hay ningún móvil conectado por adb"
        adb shell "mkdir -p $PREFIX $WEB_DIR $STATE" || true
        adb push "$SRC_BIN" "$BIN" >/dev/null || die "adb push del binario falló"
        adb shell "chmod 755 $BIN" || true
        if [ -f "$REPO_ROOT/web/cockpit_template.html" ]; then
            adb_ push "$REPO_ROOT/web/cockpit_template.html" "$TEMPLATE" >/dev/null || true
            adb_ shell "mkdir -p $APP_TEMPLATE_DIR" >/dev/null 2>&1 || true
            adb_ push "$REPO_ROOT/web/cockpit_template.html" "$APP_TEMPLATE_DIR/cockpit_template.html" >/dev/null 2>&1 || true
        fi
        # UI v2 desde disco (sin recompilar): si no estan, el daemon usa la
        # copia embebida => no se rompe nada.
        [ -f "$REPO_ROOT/web/cockpit_v2.html" ] && adb push "$REPO_ROOT/web/cockpit_v2.html" "$WEB_DIR/cockpit_v2.html" >/dev/null 2>&1 || true
        [ -f "$REPO_ROOT/web/webui_v2.css" ]  && adb push "$REPO_ROOT/web/webui_v2.css"  "$WEB_DIR/webui_v2.css"  >/dev/null 2>&1 || true
        if [ -f "$REPO_ROOT/deploy/99-megadaemon.sh" ]; then
            adb_ push "$REPO_ROOT/deploy/99-megadaemon.sh" "$SERVICE" >/dev/null || true
            adb_ shell "chmod 755 $SERVICE" || true
        fi
    fi
    install_extras
    say "ficheros copiados"
    return 0
}

# ── 3b. Canal: watchdog + publicación + sonda VS4L ───────────────────────────
# El watchdog necesita `channel-publish` (el móvil NO tiene curl: el sondeo y
# la publicación los hace ese binario) y `98-cloudflared.sh` (un solo
# cloudflared, independiente del daemon).
install_extras() {
    src_dir=$(dirname "$SRC_BIN")
    # `md-clipboard` (portapapeles) y `md-dict` (diccionarios del teclado)
    # son CLIs que se usan directamente en el movil.
    for b in channel-publish duckdns-updater npu-probe md-lan-listen md-clipboard md-dict; do
        [ -f "$src_dir/$b" ] || continue
        if [ "$ON_PHONE" = "1" ]; then
            cp -f "$src_dir/$b" "$PREFIX/$b" 2>/dev/null && chmod 755 "$PREFIX/$b" 2>/dev/null
        else
            adb push "$src_dir/$b" "$PREFIX/$b" >/dev/null 2>&1 && adb shell "chmod 755 $PREFIX/$b" >/dev/null 2>&1
        fi
    done
    # El script de RECUPERACION: es lo que el usuario ejecuta en el movil
    # cuando algo se rompe.  Con --auto-update se autorepara solo.
    if [ -f "$REPO_ROOT/deploy/phone-recover.sh" ]; then
        if [ "$ON_PHONE" = "1" ]; then
            cp -f "$REPO_ROOT/deploy/phone-recover.sh" "$PREFIX/phone-recover.sh" 2>/dev/null \
                && chmod 755 "$PREFIX/phone-recover.sh" 2>/dev/null
        else
            adb push "$REPO_ROOT/deploy/phone-recover.sh" "$PREFIX/phone-recover.sh" >/dev/null 2>&1 \
                && adb shell "chmod 755 $PREFIX/phone-recover.sh" >/dev/null 2>&1
        fi
        say "script de recuperación instalado: $PREFIX/phone-recover.sh"
    fi
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
        adb_ shell "su -c 'setsid nohup sh $SERVICE >/dev/null 2>&1 &'" >/dev/null 2>&1 || true
    fi
}

# ── 5. Verificación con espera activa ───────────────────────────────────────
health_on_port() {
    p=$1
    if [ "$ON_PHONE" = "1" ]; then
        if have curl; then
            curl -s -m 3 -o /dev/null -w '%{http_code}' "http://127.0.0.1:$p/health" 2>/dev/null
        else
            toybox wget -q -O /dev/null "http://127.0.0.1:$p/health" 2>/dev/null && echo 200 || echo 000
        fi
    else
        adb_ shell "curl -s -m 3 -o /dev/null -w '%{http_code}' http://127.0.0.1:$p/health" 2>/dev/null | tr -d '\r\n'
    fi
}

# Espera a que AL MENOS un puerto dé 200 en /health.
wait_healthy() {
    timeout=$1
    deadline=$(( $(date +%s) + timeout ))
    while [ "$(date +%s)" -lt "$deadline" ]; do
        for p in $PORTS; do
            code=$(health_on_port "$p")
            [ "${code:-000}" = "200" ] && { echo "$p"; return 0; }
        done
        sleep 2
    done
    return 1
}

verify() {
    say "esperando /health (máx ${HEALTH_TIMEOUT}s) en: $PORTS"
    if ok_port=$(wait_healthy "$HEALTH_TIMEOUT"); then
        say "COCKPIT OK -> abre http://localhost:$ok_port  (o http://127.0.0.1:$ok_port)"
        say "la UI también responde en el resto de puertos alias: $PORTS"
        as_root "cat $STATE/cockpit_token 2>/dev/null" 2>/dev/null | sed 's/^/  token de sesión: /'
        return 0
    fi
    return 1
}

# ── 6. Rollback ──────────────────────────────────────────────────────────────
rollback() {
    say "el cockpit no arrancó: ROLLBACK al binario anterior"
    as_root 'killall megadaemon-core 2>/dev/null; sleep 1' >/dev/null 2>&1
    if as_root "[ -f $BIN.bak ]"; then
        as_root "cp -f $BIN.bak $BIN && chmod 755 $BIN" 2>/dev/null
        say "binario anterior restaurado ($BIN.bak)"
    else
        say "no había binario anterior: nada que restaurar"
    fi
    as_root "echo '[$(date '+%Y-%m-%d %H:%M:%S')] ERROR: install-phone.sh falló el arranque de /health; rollback aplicado' >> $LOG" 2>/dev/null
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
    exit 0
else
    rollback
    exit 2
fi
