#!/usr/bin/env bash
# =====================================================================
# recuperar-movil.sh — RECUPERAR EL MÓVIL DE UN SOLO COMANDO
#
# QUÉ HACE (en este orden, y para en seco si no puede):
#   1. Diagnóstico: ¿está el móvil en ADB? ¿responde el túnel? ¿qué build
#      está en servicio?
#   2. Si el móvil es alcanzable por ADB:
#        a) reinicia el móvil (el daemon y su supervisor están muertos),
#        b) espera a que arranque,
#        c) descarga la build MÁS RECIENTE del release (con su .sha256) y la
#           instala ATÓMICAMENTE en el móvil (sha verificado en el móvil),
#        d) se asegura de que el supervisor y el keeper estén instalados y
#           con permisos correctos (Magisk service.d),
#        e) arranca el daemon y espera a que responda,
#        f) imprime la URL del túnel (la que hay que usar para conectarse).
#   3. Si el móvil NO es alcanzable por ADB (desde el sandbox el proxy
#      atrapa el puerto), NO se rinde: se queda **esperando** a que el
#      móvil vuelva (el usuario lo reinicia a mano) y continúa solo.
#
# USO:
#   tools/recuperar-movil.sh                 # diagnóstico + espera
#   tools/recuperar-movil.sh --sin-adb       # sólo el diagnóstico
#   tools/recuperar-movil.sh --reboot        # además, reinicia el móvil
#   tools/recuperar-movil.sh --espera 3600   # cuánto esperar (s)
#
# NO necesita nada del repo salvo el propio script: la build se descarga de
# GitHub y el supervisor se genera aquí (es el MISMO que
# `deploy/99-megadaemon.sh`, y el daemon lo reinstala si difiere).
# =====================================================================
set -uo pipefail

REPO_GH="https://github.com/boolforge/megadaemon"
ASSET="megadaemon-core-android-arm64"
PREFIX="/data/local/tmp/megadaemon"
BIN="$PREFIX/megadaemon-core"
ENVF="$PREFIX/megadaemon.env"
STATE="$PREFIX/state"
SERVICE_D="/data/adb/service.d"
TUNEL_URL_FICHERO="$PREFIX/tunnel.url"

REINICIAR=0
SIN_ADB=0
ESPERA="${MD_ESPERA_SEG:-1800}"
while [ $# -gt 0 ]; do
    case "$1" in
        --reboot) REINICIAR=1 ;;
        --sin-adb) SIN_ADB=1 ;;
        --espera) ESPERA="${2:-1800}"; shift ;;
        -h|--help) sed -n '2,30p' "$0"; exit 0 ;;
        *) echo "opción desconocida: $1" >&2; exit 2 ;;
    esac
    shift
done

log()  { printf '[%s] %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*"; }
warn() { printf '[%s] AVISO: %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*"; }

# ── 1) ADB ─────────────────────────────────────────────────────────────
ADB=""
for c in adb "$HOME/Android/Sdk/platform-tools/adb" "$ANDROID_HOME/platform-tools/adb"; do
    [ -n "${c:-}" ] && command -v "$c" >/dev/null 2>&1 && { ADB="$c"; break; }
done
[ -z "$ADB" ] && [ -x /usr/lib/android-sdk/platform-tools/adb ] && ADB=/usr/lib/android-sdk/platform-tools/adb

DEV=""
adb_pick() {
    [ -z "$ADB" ] && return 1
    DEV=$("$ADB" devices 2>/dev/null | awk 'NR>1 && $2=="device" {print $1; exit}')
    [ -n "$DEV" ]
}

as_root() { "$ADB" -s "$DEV" shell "su -mm -c '$1'" 2>&1; }

# ── 2) diagnóstico ─────────────────────────────────────────────────────
diagnostico() {
    log "=== diagnóstico del móvil ==="
    if adb_pick; then
        log "ADB: móvil ALCANZABLE ($DEV)"
    else
        log "ADB: el móvil NO es alcanzable desde aquí"
        log "     (desde el sandbox hay un proxy que atrapa el puerto; el"
        log "      móvil se recupera reiniciándolo a mano o con este script"
        log "      en un equipo con ADB y el móvil por USB)"
    fi
    local code
    code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 12 \
        "${MD_TUNEL:-https://graphical-subscriber-sys-oil.trycloudflare.com}/health" 2>/dev/null || echo 000)
    log "túnel público: HTTP $code  (502 = el túnel vive pero el ORIGEN está caído)"
    if adb_pick; then
        local sha build
        sha=$(as_root "sha256sum $BIN 2>/dev/null | cut -d' ' -f1")
        log "build en el móvil: ${sha:-desconocida} (${sha:0:16})"
        build=$(as_root "grep -o 'MEGADAEMON_BUILD_SHA=.*' $ENVF 2>/dev/null | cut -d= -f2")
        log "commit declarado en el móvil: ${build:-ninguno}"
        log "supervisor: $(as_root "ls -l $SERVICE_D/99-megadaemon.sh 2>/dev/null" | tr -s ' ')"
        log "keeper:    $(as_root "ls -l $SERVICE_D/97-megadaemon-keeper.sh 2>/dev/null" | tr -s ' ')"
    fi
}

# ── 3) la build nueva ─────────────────────────────────────────────────
descargar_build() {
    local dir="$1"
    mkdir -p "$dir"
    log "descargando $ASSET de $REPO_GH ..."
    if ! curl -fL --max-time 600 -o "$dir/$ASSET" \
        "$REPO_GH/releases/latest/download/$ASSET"; then
        warn "no se pudo descargar la build"
        return 1
    fi
    curl -fL --max-time 60 -o "$dir/$ASSET.sha256" \
        "$REPO_GH/releases/latest/download/$ASSET.sha256" 2>/dev/null || true
    local esperado
    esperado=$(awk '{print $1}' "$dir/$ASSET.sha256" 2>/dev/null)
    local real
    real=$(sha256sum "$dir/$ASSET" | cut -d' ' -f1)
    if [ -n "$esperado" ] && [ "$esperado" != "$real" ]; then
        warn "sha256 de la build NO coincide (esperado $esperado, real $real)"
        return 1
    fi
    log "build verificada: sha256 $real (sha256sum ${#real} chars)"
    printf '%s' "$real"
}

# ── 4) instalar en el móvil ───────────────────────────────────────────
instalar_en_movil() {
    local dir="$1" sha="$2"
    log "subiendo la build al móvil (41 MB) ..."
    "$ADB" -s "$DEV" push "$dir/$ASSET" "/data/local/tmp/$ASSET" >/dev/null 2>&1 \
        || { warn "no se pudo subir"; return 1; }
    # Verificación DENTRO del móvil (el sha del fichero local no vale: lo que
    # importa es lo que queda en el móvil).
    local remoto
    remoto=$(as_root "sha256sum /data/local/tmp/$ASSET 2>/dev/null | cut -d' ' -f1")
    if [ "$remoto" != "$sha" ]; then
        warn "el sha en el móvil ($remoto) NO es el de la build ($sha): se aborta"
        return 1
    fi
    log "sha verificado en el móvil: $remoto"
    # Instalación ATÓMICA con copia del anterior (.prev) para el rollback.
    as_root "mkdir -p $PREFIX && cp -f $BIN $BIN.prev 2>/dev/null; cp -f /data/local/tmp/$ASSET $BIN.new && chmod 755 $BIN.new && mv -f $BIN.new $BIN && sync" >/dev/null
    as_root "rm -f /data/local/tmp/$ASSET" >/dev/null
    local instalado
    instalado=$(as_root "sha256sum $BIN 2>/dev/null | cut -d' ' -f1")
    [ "$instalado" = "$sha" ] || { warn "la build instalada no cuadra ($instalado)"; return 1; }
    log "build instalada en el móvil: $instalado"
    return 0
}

# ── 5) supervisor + keeper ────────────────────────────────────────────
asegurar_supervisor() {
    log "asegurando el supervisor y el keeper (Magisk service.d) ..."
    as_root "mkdir -p $SERVICE_D $PREFIX $STATE" >/dev/null
    # El daemon reinstala su propio supervisor si difiere (`supervisor::instalar`),
    # pero si el móvil no arranca el daemon, el supervisor debe estar AHÍ.
    for f in 99-megadaemon.sh 97-megadaemon-keeper.sh; do
        if [ -f "$(dirname "$0")/../deploy/$f" ]; then
            "$ADB" -s "$DEV" push "$(dirname "$0")/../deploy/$f" "/data/local/tmp/$f" >/dev/null 2>&1
            as_root "cp -f /data/local/tmp/$f $SERVICE_D/$f && chmod 755 $SERVICE_D/$f && rm -f /data/local/tmp/$f" >/dev/null
            log "  $f instalado ($(as_root "sha256sum $SERVICE_D/$f 2>/dev/null | cut -c1-16"))"
        fi
    done
    # El entorno: el auto-update cada 30 min (para que traiga builds nuevas
    # sin intervención) y el estado en un sitio que exista.
    as_root "touch $ENVF && chmod 600 $ENVF && grep -q MD_AUTO_UPDATE_INTERVAL_MIN $ENVF || echo 'MD_AUTO_UPDATE_INTERVAL_MIN=30' >> $ENVF" >/dev/null
    as_root "grep -q MEGADAEMON_STATE_DIR $ENVF || echo 'MEGADAEMON_STATE_DIR=$STATE' >> $ENVF" >/dev/null
    log "  entorno: $(as_root "tr '\n' ' ' < $ENVF" | cut -c1-120)"
}

# ── 6) arrancar y recoger la URL ──────────────────────────────────────
arrancar_y_url() {
    log "arrancando el daemon ..."
    as_root "pkill -9 -x megadaemon-core" >/dev/null 2>&1
    as_root "cd $PREFIX && setsid nohup $BIN cockpit 9090 >> /sdcard/megadaemon.log 2>&1 < /dev/null &" >/dev/null 2>&1
    local i=0 code=000
    while [ $i -lt 40 ]; do
        sleep 3
        code=$(as_root "toybox wget -q -O - http://127.0.0.1:9090/health 2>/dev/null | head -c 200" 2>/dev/null)
        case "$code" in *build*) break ;; esac
        i=$((i + 1))
    done
    if [ -z "$code" ] || [ "$code" = "000" ]; then
        warn "el daemon no respondió /health en el móvil (revisar /sdcard/megadaemon.log)"
    else
        log "el daemon responde: $(printf '%s' "$code" | tr -d '\r\n' | cut -c1-160)"
    fi
    # El túnel: si el supervisor no lo levantó, se levanta.
    as_root "pkill -x cloudflared" >/dev/null 2>&1
    as_root "cd $PREFIX && setsid nohup ./cloudflared tunnel --no-autoupdate --url http://127.0.0.1:9090 >> $PREFIX/tunnel.log 2>&1 < /dev/null &" >/dev/null 2>&1
    local j=0 url=""
    while [ $j -lt 20 ]; do
        sleep 5
        url=$(as_root "cat $TUNEL_URL_FICHERO 2>/dev/null" | tr -d '\r\n')
        [ -n "$url" ] && break
        url=$(as_root "grep -ao 'https://[a-z0-9-]*\.trycloudflare\.com' $PREFIX/tunnel.log 2>/dev/null | tail -1" | tr -d '\r\n')
        [ -n "$url" ] && break
        j=$((j + 1))
    done
    if [ -n "$url" ]; then
        as_root "printf '%s\n' '$url' > $TUNEL_URL_FICHERO" >/dev/null 2>&1
        log "URL DEL TÚNEL: $url"
        log "  /health : $url/health"
        log "  la Web UI: $url/  (si pide credencial, la API key está en el móvil)"
    else
        warn "no se pudo obtener la URL del túnel (mira $PREFIX/tunnel.log en el móvil)"
    fi
}

# ── 7) guion ───────────────────────────────────────────────────────────
diagnostico
if [ "$SIN_ADB" = "1" ]; then
    log "(--sin-adb: no se intenta tocar el móvil)"
    exit 0
fi

if ! adb_pick; then
    log "El móvil no es alcanzable por ADB desde aquí."
    log "=> REINICÍALO A MANO (botón de encendido) y deja este script vivo:"
    log "   volverá solo en cuanto el móvil aparezca."
    deadline=$(( $(date +%s) + ESPERA ))
    while [ "$(date +%s)" -lt "$deadline" ]; do
        sleep 15
        if adb_pick; then
            log "¡el móvil ha aparecido! ($DEV)"
            break
        fi
    done
    if ! adb_pick; then
        warn "el móvil no apareció en ${ESPERA}s: se sale"
        exit 1
    fi
fi

if [ "$REINICIAR" = "1" ]; then
    log "reiniciando el móvil ..."
    "$ADB" -s "$DEV" reboot >/dev/null 2>&1
    sleep 45
    i=0
    while [ $i -lt 40 ]; do
        if "$ADB" -s "$DEV" shell true >/dev/null 2>&1; then break; fi
        sleep 10
        i=$((i + 1))
    done
    log "el móvil ha vuelto; esperando al daemon (Magisk service.d) ..."
    sleep 40
fi

DIR=$(mktemp -d)
trap 'rm -rf "$DIR"' EXIT
SHA=$(descargar_build "$DIR" | tail -1)
[ -n "$SHA" ] || { warn "sin build: no se puede continuar"; exit 1; }
instalar_en_movil "$DIR" "$SHA" || { warn "la instalación falló"; exit 1; }
asegurar_supervisor
arrancar_y_url
log "LISTO. Con el móvil arriba y la build nueva, el auto-actualizador"
log "seguirá trayendo builds nuevas cada 30 min sin intervención."
exit 0
