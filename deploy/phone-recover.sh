#!/system/bin/sh
# =====================================================================
# phone-recover.sh — deja el móvil MEGADAEMON recuperado y autocurable
#
# Un comando, idempotente, pensado para ejecutarse EN el móvil:
#
#     su -mm -c 'sh /data/local/tmp/megadaemon/phone-recover.sh'
#
# Qué hace (y qué NO hace):
#   * NO reinicia el móvil.
#   * NO reinicia el daemon si está vivo y sano (sólo lo relanza si no
#     responde) => re-executarlo nunca deja el móvil peor.
#   * NO toca el binario salvo que se le pase --install <ruta>.
#   * NO toca la API key ni el token: sólo crea lo que falta.
#
# Deja el móvil en este estado:
#   1. /data/local/tmp/megadaemon/state existe (el estado REAL del daemon);
#   2. megadaemon.env declara MEGADAEMON_STATE_DIR y MEGADAEMON_LISTEN;
#   3. supervisor + cloudflared + watchdog instalados en /data/adb/service.d
#      (=> sobreviven a un reinicio del móvil);
#   4. el daemon vivo y con /health respondiendo;
#   5. un túnel VIVO con su URL en state/tunnel.url;
#   6. el canal publicado (state/channel.json con el 'quick' correcto);
#   7. el beacon LAN emitido (descubrimiento sin internet).
#
# Uso:
#     phone-recover.sh                 # recuperar y verificar
#     phone-recover.sh --diagnostico   # sólo informe, no cambia nada
#     phone-recover.sh --install /sdcard/Download/megadaemon-core
#     phone-recover.sh --auto-update   # baja la última release y verifica su SHA-256
#     phone-recover.sh --no-tunnel     # recuperación sin internet
# =====================================================================

PREFIX=/data/local/tmp/megadaemon
STATE="$PREFIX/state"
SERVICE_D=/data/adb/service.d
ENV_FILE="$PREFIX/megadaemon.env"
BIN="$PREFIX/megadaemon-core"
LOG="$PREFIX/recover.log"
PORT=9090
CF=/data/data/com.termux/files/usr/bin/cloudflared
[ -x "$CF" ] || CF=$(command -v cloudflared 2>/dev/null)
PUB="$PREFIX/channel-publish"

# El móvil NO tiene curl (medido).  El sondeo fiable es el propio binario
# (`channel-publish --probe` imprime el código HTTP en stdout); como
# respaldo, `toybox wget` (que sí trae Android) y `curl` si estuviera.
# MEDIDO el 2026-11-21 en el S21 (Android 14):
#   * `toybox wget` -> "Unknown command" (rc=127).  El movil NO tiene wget.
#   * `curl` tampoco esta (salvo en Termux, y no como root).
#   * `channel-publish` NO se puede descargar (su descarga usaba wget).
# Resultado: el sondeo daba SIEMPRE 0 y el script daba por muerto un daemon
# que estaba vivo -> "el daemon sigue sin responder" con el daemon sano.
#
# Ahora el sondeo lo hace el binario PRINCIPAL del daemon (siempre
# instalado): `megadaemon-core probe <url>` imprime el codigo HTTP.
http_code() {
    _u="$1"
    if [ -x "$BIN" ]; then
        _c=$("$BIN" probe "$_u" 2>/dev/null | head -1)
        case "$_c" in
            ''|*[!0-9]*) : ;;
            *) echo "$_c"; return ;;
        esac
    fi
    if [ -x "$PUB" ]; then
        _c=$("$PUB" --probe "$_u" 2>/dev/null | head -1)
        case "$_c" in
            ''|*[!0-9]*) : ;;
            *) echo "$_c"; return ;;
        esac
    fi
    if command -v curl >/dev/null 2>&1; then
        _c=$(curl -s -m 5 -o /dev/null -w '%{http_code}' "$_u" 2>/dev/null)
        case "$_c" in ''|*[!0-9]*) echo 0 ;; *) echo "$_c" ;; esac
        return
    fi
    echo 0
}
MODE=recuperar
NO_TUNNEL=0
INSTALL_BIN=""
AUTO_UPDATE=0
BINARY_CHANGED=0

# El directorio de este script: aquí se buscan los ficheros a desplegar.
SELF_DIR=$(cd "$(dirname "$0")" 2>/dev/null && pwd)
[ -n "$SELF_DIR" ] || SELF_DIR="$PREFIX"

log() {
    printf '[%s] %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*" | tee -a "$LOG"
}
step() { log "── $* ──"; }

# Parser con `while` (un `for` con `shift` dentro no es fiable).
while [ $# -gt 0 ]; do
    case "$1" in
        --diagnostico) MODE=diagnostico ;;
        --no-tunnel)   NO_TUNNEL=1 ;;
        --auto-update) AUTO_UPDATE=1 ;;
        --install)
            shift
            INSTALL_BIN="${1:-}"
            [ -n "$INSTALL_BIN" ] || { echo "Falta la ruta tras --install" >&2; exit 2; }
            ;;
        -h|--help)     sed -n '2,44p' "$0"; exit 0 ;;
        *)             echo "opción desconocida: $1" >&2; exit 2 ;;
    esac
    shift
done

mkdir -p "$PREFIX" "$STATE" 2>/dev/null
[ -f "$LOG" ] || : > "$LOG"

# ── Descarga (el móvil NO tiene curl; sí toybox wget) ──────────────────
REPO_RAW=https://raw.githubusercontent.com/boolforge/megadaemon/main
REPO_API=https://api.github.com/repos/boolforge/megadaemon
# OJO: la API y la descarga son hosts DISTINTOS.  La API sirve el JSON de la
# release en api.github.com; los ficheros se descargan de
# github.com/<owner>/<repo>/releases/download/<tag>/<asset>.  Usar
# api.github.com/.../releases/download/... devuelve 404 (medido).
REPO_DL=https://github.com/boolforge/megadaemon/releases/download

# El movil tiene varias rutas de descarga y NO todas sirven para HTTPS:
#   * `curl` de Termux (si está) => el mejor;
#   * `wget` de Termux;
#   * `toybox wget` => puede ir sin TLS según la build;
#   * `busybox wget`.
# Se prueban EN ORDEN y se usa la primera que funcione.
TERMUX_BIN=/data/data/com.termux/files/usr/bin

fetch() {
    # $1 = url  $2 = destino.  Devuelve 0 si se descargó.
    _u="$1"; _d="$2"
    for c in curl wget; do
        if command -v "$c" >/dev/null 2>&1; then
            "$c" -fsSL -m 120 -o "$_d" "$_u" 2>/dev/null && return 0
            "$c" -q -T 60 -O "$_d" "$_u" 2>/dev/null && return 0
        fi
        if [ -x "$TERMUX_BIN/$c" ]; then
            "$TERMUX_BIN/$c" -fsSL -m 120 -o "$_d" "$_u" 2>/dev/null && return 0
            "$TERMUX_BIN/$c" -q -T 60 -O "$_d" "$_u" 2>/dev/null && return 0
        fi
    done
    if command -v toybox >/dev/null 2>&1; then
        toybox wget -q -T 60 -O "$_d" "$_u" 2>/dev/null && return 0
    fi
    if command -v busybox >/dev/null 2>&1; then
        busybox wget -q -T 60 -O "$_d" "$_u" 2>/dev/null && return 0
    fi
    return 1
}

sha256_de() {
    # $1 = fichero.  Imprime el sha256 o nada.
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" 2>/dev/null | awk '{print $1}'
    elif command -v toybox >/dev/null 2>&1; then
        toybox sha256sum "$1" 2>/dev/null | awk '{print $1}'
    fi
}

# ─────────────────────────────────────────────────────────────────────
# 0) Preflight
# ─────────────────────────────────────────────────────────────────────
step "Preflight"
if [ "$(id -u)" != "0" ]; then
    log "ERROR: hace falta root. Ejecuta: su -mm -c 'sh $0'"
    exit 1
fi
log "root OK · PREFIX=$PREFIX · STATE=$STATE"
log "estado actual: $(ls "$STATE" 2>/dev/null | tr '\n' ' ')"

# ─────────────────────────────────────────────────────────────────────
# 1) state_dir + env
# ─────────────────────────────────────────────────────────────────────
step "Estado y entorno"
mkdir -p "$STATE" 2>/dev/null
if [ ! -d "$STATE" ]; then
    log "ERROR: no se pudo crear $STATE"
    exit 1
fi
chmod 700 "$STATE" 2>/dev/null

# El env tiene que declarar el state_dir: es lo que hace que daemon,
# cloudflared, watchdog, peers, auditoría y tablero escriban EN EL MISMO
# sitio (medido: sin esto el daemon caía en un directorio inexistente).
if [ ! -f "$ENV_FILE" ]; then
    {
        echo "MEGADAEMON_STATE_DIR=$STATE"
        echo "MEGADAEMON_LISTEN=127.0.0.1:$PORT"
    } > "$ENV_FILE"
    chmod 600 "$ENV_FILE"
    log "creado $ENV_FILE (sin secretos: la key se acuña aparte)"
else
    log "$ENV_FILE ya existe: NO se tocan sus secretos"
    if ! grep -q '^MEGADAEMON_STATE_DIR=' "$ENV_FILE" 2>/dev/null; then
        printf 'MEGADAEMON_STATE_DIR=%s\n' "$STATE" >> "$ENV_FILE"
        log "añadido MEGADAEMON_STATE_DIR al env"
    fi
    if ! grep -q '^MEGADAEMON_LISTEN=' "$ENV_FILE" 2>/dev/null; then
        printf 'MEGADAEMON_LISTEN=127.0.0.1:%s\n' "$PORT" >> "$ENV_FILE"
        log "añadido MEGADAEMON_LISTEN al env"
    fi
fi

# ─────────────────────────────────────────────────────────────────────
# 2) Ficheros de deployment en service.d (=> sobreviven al reinicio)
# ─────────────────────────────────────────────────────────────────────
step "Despliegue en $SERVICE_D"
mkdir -p "$SERVICE_D" 2>/dev/null
deploy() {
    # $1 = origen  $2 = destino  $3 = chmod
    [ -f "$1" ] || return 1
    cp -f "$1" "$2" 2>/dev/null || return 1
    chmod "$3" "$2" 2>/dev/null
    return 0
}
PAYLOAD_MISSING=0
for pair in "99-megadaemon.sh:$SERVICE_D/99-megadaemon.sh" \
            "98-cloudflared.sh:$SERVICE_D/98-cloudflared.sh" \
            "tunnel-watchdog.sh:$PREFIX/tunnel-watchdog.sh"; do
    src="$SELF_DIR/${pair%%:*}"
    dst="${pair#*:}"
    if deploy "$src" "$dst" 755; then
        log "desplegado $(basename "$dst") (local)"
        continue
    fi
    # No está al lado: se descarga del repo (el móvil tiene toybox wget).
    base="${pair%%:*}"
    if fetch "$REPO_RAW/deploy/$base" "$dst.tmp"; then
        if [ -s "$dst.tmp" ] && head -1 "$dst.tmp" | grep -q '#!'; then
            mv -f "$dst.tmp" "$dst"; chmod 755 "$dst"
            log "desplegado $base (descargado del repo)"
        else
            rm -f "$dst.tmp"
            log "AVISO: la descarga de $base no parece válida"
            PAYLOAD_MISSING=1
        fi
    else
        rm -f "$dst.tmp" 2>/dev/null
        log "AVISO: no encuentro $base y no se pudo descargar"
        PAYLOAD_MISSING=1
    fi
done
# El watchdog se lanza desde 98-cloudflared.sh con esta ruta:
[ -f "$SERVICE_D/98-cloudflared.sh" ] && {
    sed -i "s#^WATCHDOG=.*#WATCHDOG=\"$PREFIX/tunnel-watchdog.sh\"#" "$SERVICE_D/98-cloudflared.sh" 2>/dev/null
    log "watchdog apuntando a $PREFIX/tunnel-watchdog.sh"
}

# ─────────────────────────────────────────────────────────────────────
# 3) Binario (opcional)
# ─────────────────────────────────────────────────────────────────────
if [ "$AUTO_UPDATE" = "1" ] && [ "$MODE" != "diagnostico" ]; then
    step "Auto-actualización del binario"
    auto_update
fi
# Auto-actualización: baja la última release de GitHub, VERIFICA el SHA-256
# y sólo entonces cambia el binario (guardando el anterior para rollback).
auto_update() {
    log "buscando la última release..."
    _json="$PREFIX/.release.json"
    if ! fetch "$REPO_API/releases/latest" "$_json"; then
        log "AVISO: no se pudo consultar la release (¿sin internet?)"
        return 1
    fi
    _tag=$(grep -o '"tag_name"[[:space:]]*:[[:space:]]*"[^"]*"' "$_json" 2>/dev/null | head -1 | sed -E 's/.*"([^"]*)".*/\1/')
    [ -n "$_tag" ] || { log "AVISO: no se pudo leer el tag de la release"; return 1; }
    log "release: $_tag"
    _base="$REPO_DL/$_tag"
    _asset="$PREFIX/megadaemon-core.new"
    if ! fetch "$_base/megadaemon-core-android-arm64" "$_asset"; then
        log "AVISO: no se pudo bajar el binario"
        return 1
    fi
    # SHA-256 esperado: del asset .sha256 del propio release.
    _sha_file="$PREFIX/.asset.sha256"
    if fetch "$_base/megadaemon-core-android-arm64.sha256" "$_sha_file"; then
        _esperado=$(awk '{print $1}' "$_sha_file" 2>/dev/null | head -1)
        _real=$(sha256_de "$_asset")
        if [ -n "$_esperado" ] && [ "$_esperado" != "$_real" ]; then
            log "ERROR: SHA-256 no coincide (esperado ${_esperado:0:16}… real ${_real:0:16}…) => NO se instala"
            rm -f "$_asset"
            return 1
        fi
        log "SHA-256 verificado: ${_real:0:16}…"
    else
        log "AVISO: no hay .sha256 en el release (se instala sin verificar)"
    fi
    # Instalación con rollback: el binario actual se conserva como .prev.
    _sha_antes=$(sha256_de "$BIN" 2>/dev/null)
    if [ -f "$BIN" ]; then cp -f "$BIN" "$PREFIX/megadaemon-core.prev" 2>/dev/null; fi
    mv -f "$_asset" "$BIN" 2>/dev/null || { log "ERROR: no se pudo instalar el binario"; return 1; }
    chmod 755 "$BIN" 2>/dev/null
    _sha_despues=$(sha256_de "$BIN" 2>/dev/null)
    if [ -n "$_sha_antes" ] && [ "$_sha_antes" != "$_sha_despues" ]; then
        BINARY_CHANGED=1
        log "binario CAMBIADO (${_sha_antes:0:16} -> ${_sha_despues:0:16}) => el daemon se reiniciara"
    else
        log "binario instalado: ${_sha_despues:0:16} ($(stat -c %s "$BIN" 2>/dev/null) B)"
    fi

    # Binarios AUXILIARES: `channel-publish` es imprescindible (es el que
    # sondea el tunel —el movil no tiene curl— y el que publica el canal).
    for aux in channel-publish duckdns-updater md-lan-listen npu-probe; do
        _a="$PREFIX/$aux"
        _tmp="$PREFIX/.$aux.new"
        fetch "$_base/$aux-arm64" "$_tmp" || { rm -f "$_tmp" 2>/dev/null; continue; }
        _sf="$PREFIX/.$aux.sha256"
        if fetch "$_base/$aux-arm64.sha256" "$_sf"; then
            _esp=$(awk '{print $1}' "$_sf" 2>/dev/null | head -1)
            _rea=$(sha256_de "$_tmp")
            if [ -n "$_esp" ] && [ "$_esp" != "$_rea" ]; then
                log "ERROR: SHA-256 de $aux no coincide => NO se instala"
                rm -f "$_tmp"
                continue
            fi
        fi
        mv -f "$_tmp" "$_a" 2>/dev/null && chmod 755 "$_a" 2>/dev/null \
            && log "auxiliar instalado: $aux ($(sha256_de "$_a" | cut -c1-16)…)"
        rm -f "$_sf" 2>/dev/null
    done
    return 0
}

if [ -n "$INSTALL_BIN" ] && [ -f "$INSTALL_BIN" ]; then
    step "Instalando binario desde $INSTALL_BIN"
    _sha_antes=$(sha256_de "$BIN" 2>/dev/null)
    cp -f "$INSTALL_BIN" "$BIN.new" 2>/dev/null
    if [ -f "$BIN" ]; then cp -f "$BIN" "$BIN.prev" 2>/dev/null; fi
    mv -f "$BIN.new" "$BIN" 2>/dev/null
    chmod 755 "$BIN" 2>/dev/null
    _sha_despues=$(sha256_de "$BIN" 2>/dev/null)
    if [ -n "$_sha_antes" ] && [ "$_sha_antes" != "$_sha_despues" ]; then
        BINARY_CHANGED=1
        log "binario CAMBIADO => el daemon se reiniciara"
    fi
    log "binario instalado: ${_sha_despues:0:16} ($(stat -c %s "$BIN" 2>/dev/null) B)"
fi
[ -x "$BIN" ] || log "AVISO: no hay binario en $BIN (usa --install)"

# ─────────────────────────────────────────────────────────────────────
# 4) Daemon: vivo y sano
# ─────────────────────────────────────────────────────────────────────
step "Daemon"
daemon_ok() {
    # vivo = proceso + /health responde en el puerto local
    pidof megadaemon-core >/dev/null 2>&1 || return 1
    for p in $PORT 3030 8090 3000; do
        code=$(http_code "http://127.0.0.1:$p")
        case "$code" in 200|401|403) return 0 ;; esac
    done
    return 1
}
if [ "$BINARY_CHANGED" = "1" ]; then
    log "el binario cambio => reinicio del daemon inevitable (si no, seguiria el viejo)"
    pkill -f megadaemon-core >/dev/null 2>&1
    sleep 3
fi
if daemon_ok; then
    log "daemon VIVO y sano (pid $(pidof megadaemon-core 2>/dev/null)) — no se toca"
else
    if [ "$MODE" = "diagnostico" ]; then
        log "daemon NO sano (sólo informe: no se relanza)"
    else
        log "daemon muerto o mudo -> relanzando el supervisor"
        [ -f "$SERVICE_D/99-megadaemon.sh" ] && {
            # MEDIDO el 2026-11-21: el daemon tarda hasta ~90 s en responder
            # cuando el movil va saturado (load 18-21) o hace el arranque
            # completo.  Los 18 s fijos de antes daran un falso "sigue sin
            # responder" con el daemon sano.
            #
            # Se lanza bajo `su -mm` (Magisk) y NO como hijo de esta shell:
            # si el supervisor cuelga de Termux, al cerrar Termux se lo lleva
            # el sistema y el movil se queda sin daemon.
            su -mm -c "setsid nohup sh '$SERVICE_D/99-megadaemon.sh' >/sdcard/megadaemon.log 2>&1 </dev/null &" 2>/dev/null \
                || setsid nohup sh "$SERVICE_D/99-megadaemon.sh" >/sdcard/megadaemon.log 2>&1 </dev/null &
            _i=0
            while [ "$_i" -lt 30 ]; do
                sleep 5
                _i=$((_i + 1))
                daemon_ok && break
            done
        }
        daemon_ok && log "daemon arriba (pid $(pidof megadaemon-core 2>/dev/null))" \
                    || log "AVISO: el daemon sigue sin responder tras 150 s (mira /sdcard/megadaemon.log)"
    fi
fi

# ─────────────────────────────────────────────────────────────────────
# 5) Túnel: uno solo, vivo, con la URL en el state_dir
# ─────────────────────────────────────────────────────────────────────
tunnel_url() {
    [ -f "$STATE/tunnel.url" ] && head -1 "$STATE/tunnel.url" 2>/dev/null
}
tunnel_alive() {
    u=$(tunnel_url)
    [ -n "$u" ] || return 1
    code=$(http_code "$u")
    # 401/403 = el daemon vive detrás y exige credencial => el túnel funciona
    case "$code" in 200|401|403) return 0 ;; esac
    return 1
}
step "Túnel"
if [ "$NO_TUNNEL" = "1" ]; then
    log "omitido por --no-tunnel"
elif [ ! -x "$CF" ]; then
    log "AVISO: no encuentro cloudflared ($CF). Sin túnel, pero el canal LAN funciona."
else
    if tunnel_alive; then
        log "túnel VIVO: $(tunnel_url)"
    else
        if [ "$MODE" = "diagnostico" ]; then
            log "túnel muerto (sólo informe: no se relanza)"
        else
            log "túnel muerto -> relanzando cloudflared (sin tocar el daemon)"
            # Sólo se mata cloudflared: el daemon NO se toca.
            pkill -f cloudflared >/dev/null 2>&1
            sleep 3
            setsid nohup "$CF" tunnel --url "http://127.0.0.1:$PORT" --no-autoupdate \
                > "$STATE/cloudflared.log" 2>&1 &
            i=0
            while [ "$i" -lt 30 ]; do
                sleep 2
                u=$(grep -ao 'https://[a-z0-9-]*\.trycloudflare\.com' "$STATE/cloudflared.log" 2>/dev/null | grep -v '^https://api\.' | grep -v '^https://developers\.' | head -1)
                [ -n "$u" ] && break
                i=$((i + 1))
            done
            if [ -n "$u" ]; then
                echo "$u" > "$STATE/tunnel.url"
                chmod 644 "$STATE/tunnel.url"
                log "túnel nuevo: $u"
            else
                log "AVISO: no capturé la URL (mira $STATE/cloudflared.log)"
            fi
        fi
    fi
fi

# ─────────────────────────────────────────────────────────────────────
# 6) Canal: publicar el 'quick' (el enjambre lo lee)
# ─────────────────────────────────────────────────────────────────────
step "Canal"
if [ -x "$PUB" ]; then
    setsid nohup "$PUB" --state-dir "$STATE" >/dev/null 2>&1 &
    sleep 4
    if [ -f "$STATE/channel.json" ]; then
        q=$(grep -o '"quick"[^,]*' "$STATE/channel.json" 2>/dev/null | head -1)
        log "channel.json presente · $q"
    else
        log "AVISO: no hay $STATE/channel.json aún"
    fi
else
    log "AVISO: no está $PUB (el /channel.json del daemon sirve igualmente)"
fi

# ─────────────────────────────────────────────────────────────────────
# 7) Beacon LAN (descubrimiento sin internet)
# ─────────────────────────────────────────────────────────────────────
step "Canal LAN"
if [ -f "$STATE/lan.json" ]; then
    log "lan.json: $(cat "$STATE/lan.json" 2>/dev/null | head -c 200)"
else
    log "el beacon lo emite el daemon (tarda unos segundos en escribir state/lan.json)"
fi

# ─────────────────────────────────────────────────────────────────────
# 8) Informe final
# ─────────────────────────────────────────────────────────────────────
step "Estado final"
IP=""
for i in $(ip -4 -o addr show 2>/dev/null | awk '{print $2" "$4}'); do
    a=$(echo "$i" | awk '{print $2}' | cut -d/ -f1)
    case "$a" in 127.*) ;; *) IP="$a"; break ;; esac
done
[ -z "$IP" ] && IP=$(ip route get 1.1.1.1 2>/dev/null | awk '{print $7; exit}')
[ -z "$IP" ] && IP="(sin IP de LAN detectable)"

echo ""
echo "================= MEGADAEMON · RESUMEN ================="
printf 'daemon   : %s\n' "$(pidof megadaemon-core 2>/dev/null || echo 'MUERTO')"
printf 'health   : %s\n' "$(http_code "http://127.0.0.1:$PORT")"
printf 'tunel    : %s\n' "$(tunnel_url)"
printf 'LAN      : http://%s:%s\n' "$IP" "$PORT"
printf 'canonico : https://mdtmfnc.duckdns.org/channel.json\n'
printf 'state    : %s\n' "$STATE"
printf 'load     : %s\n' "$(cat /proc/loadavg 2>/dev/null)"
printf 'mem libre: %s\n' "$(grep MemAvailable /proc/meminfo 2>/dev/null | awk '{print $2" kB"}')"
echo "========================================================"
echo ""
echo "Desde el PC (misma red), comprueba con:"
echo "  python3 tools/lan_discover.py --probe"
echo "  curl -sS http://$IP:$PORT/health"
echo ""
echo "Desde el sandbox, el estado del canal:"
echo "  bash tools/verify_canal.sh"
