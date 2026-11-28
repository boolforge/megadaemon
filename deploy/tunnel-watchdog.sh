#!/system/bin/sh
# ---------------------------------------------------------------------------
# WATCHDOG DEL CANAL — sondea, relanza si hace falta y PUBLICA la URL
#
# El criterio que evita el absurdo de matar el túnel por una key caducada:
#
#   * `530 / 000 / 502 / 503 / 504` => el túnel está **MUERTO** (o no hay red)
#     => se relanza `cloudflared`.
#   * `401 / 403` => **el daemon respondió** => el túnel está **VIVO** y lo
#     que falla es la **credencial** => **NO se toca el túnel** (matarlo
#     cambiaría la URL otra vez y regeneraría la key: más caos).
#   * Si el móvil está **saturado** (load1 >= MAX_LOAD) **no se relanza**:
#     con load 19-21 otro proceso no mejora nada.
#
# ⚠️ El móvil **no tiene curl ni wget** ⇒ el sondeo y la publicación los hace
# el binario `channel-publish` (el mismo código Rust que la tool MCP
# `channel_publish`): `--probe <url>` imprime el código HTTP y
# `--quick <url> --pids "<pids>"` publica el estado (channel.json + paste
# público) ⇒ **así el sandbox redescubre la URL aunque el túnel esté caído**.
#
# Variables: INTERVAL (60s), MAX_LOAD (8), LOG_DIR, CF, CFT_TARGET
# ---------------------------------------------------------------------------
INTERVAL="${INTERVAL:-60}"
MAX_LOAD="${MAX_LOAD:-8}"
# MEDIDO (2026-11-27, movil SM-G998B): el watchdog leia `tunnel.url`/
# `tunnel.log` de `/data/local/tmp/megadaemon` (VOLATIL) mientras el
# one-liner los escribe en `/data/adb/megadaemon/state` (PERSISTENTE) =>
# el watchdog creia que el tunel estaba muerto y lo relanzaba cada 60 s.
# Ahora el prefijo por defecto es el PERSISTENTE, con el volatil de plan B.
if [ -z "${LOG_DIR:-}" ]; then
    if [ -d /data/adb/megadaemon ] || [ -d /data/adb ]; then
        LOG_DIR=/data/adb/megadaemon
    else
        LOG_DIR=/data/local/tmp/megadaemon
    fi
fi
LOG_DIR="$LOG_DIR"
URL_FILE="$LOG_DIR/tunnel.url"
TUNNEL_LOG="$LOG_DIR/tunnel.log"
LOG="$LOG_DIR/tunnel-watchdog.log"
# MEDIDO el 2026-11-21: el movil NO tiene curl ni wget (`toybox wget` ->
# "Unknown command", rc=127) y `channel-publish` NO se puede descargar
# (su descarga usaba wget).  Resultado: el watchdog se quedaba SIN SONDEAR
# ("no encuentro ... (sin el no puedo sondear)") => nadie vigilaba el
# tunel y nadie publicaba el canal.  El daemon died -> el movil quedaba
# inalcanzable sin intervencion manual.
#
# Ahora el sondeo y la publicacion los hace el binario PRINCIPAL del daemon
# (`megadaemon-core probe|channel`), que SIEMPRE esta instalado.  Si
# existe `channel-publish`, se usa como alternativa.
DAEMON_BIN="${DAEMON_BIN:-$LOG_DIR/megadaemon-core}"
PUBLISH="${PUBLISH:-$LOG_DIR/channel-publish}"
# ⚠️ Lo lanza Magisk (hermano del supervisor) => hay que leer el env aquí.
# Sin `set -a`: la API key no se exporta a ningún hijo.
ENV_FILE="${MEGADAEMON_ENV:-$LOG_DIR/megadaemon.env}"
if [ -f "$ENV_FILE" ]; then
    . "$ENV_FILE" 2>/dev/null
fi
# Ni la key ni el token de sesión viajan a cloudflared/watchdog: este
# script no los necesita (y una variable que ya estaba exportada en el
# entorno mantiene su atributo de exportación al reasignarla).
unset MEGADAEMON_API_KEY MEGADAEMON_COCKPIT_TOKEN 2>/dev/null
# El estado del daemon (tunnel.url, channel.json, key) vive AQUÍ.
STATE_DIR="${MEGADAEMON_STATE_DIR:-$LOG_DIR/state}"
CF="${CF:-/data/data/com.termux/files/usr/bin/cloudflared}"
TARGET="${CFT_TARGET:-http://127.0.0.1:9090}"

# --- ficheros de PID y cerrojos -------------------------------------------
# MEDIDO (2026-11-28, movil): el watchdog se relanzaba el tunel cada 60 s
# (el hostname cambiaba y la flota se quedaba ciega) y se multiplicaban los
# watchdogs.  Ahora el "¿existe ya?" se responde con ficheros de PID +
# cerrojos, que NO dependen de leer /proc/<pid>/cmdline.
PID_CLOUDFLARED="$LOG_DIR/cloudflared.pid"
PID_WATCHDOG="$LOG_DIR/watchdog.pid"
LOCK_CLOUDFLARED="$LOG_DIR/.lock-cloudflared"
LOCK_WATCHDOG="$LOG_DIR/.lock-watchdog"

# >>> BLOQUE PID-FILES (copia idéntica en los 4 scripts del stack) >>>
# --- PID: identidad del proceso (SIN cmdline) -----------------------------
# Imprime "<starttime> <estado>" de /proc/<pid>/stat.  rc=1 si no se puede
# leer (entonces NO se puede confirmar la identidad del proceso).
pid_proc_info() {
    _pi_s=$(cat "/proc/$1/stat" 2>/dev/null) || return 1
    [ -n "$_pi_s" ] || return 1
    # "pid (comm) ..." => se quita hasta el ÚLTIMO ") " (el comm puede llevar
    # paréntesis dentro).  Lo que queda empieza por el estado.
    _pi_s=${_pi_s##*) }
    set -- $_pi_s
    [ $# -ge 20 ] || return 1
    printf '%s %s\n' "${20}" "${1}"
}

# --- FICHEROS DE PID ------------------------------------------------------
# "<pid> <starttime>", escritos de forma ATÓMICA (a .tmp y mv) para que
# nadie lea un PID a medias.
pid_escribe() {
    _pe_f=$1
    _pe_p=$2
    case "$_pe_p" in ''|*[!0-9]*) return 1 ;; esac
    _pe_i=$(pid_proc_info "$_pe_p") || _pe_i="0 ?"
    _pe_s=${_pe_i%% *}
    [ -n "$_pe_s" ] || _pe_s=0
    printf '%s %s\n' "$_pe_p" "$_pe_s" > "$_pe_f.tmp" 2>/dev/null || return 1
    mv -f "$_pe_f.tmp" "$_pe_f" 2>/dev/null || {
        rm -f "$_pe_f.tmp" 2>/dev/null
        return 1
    }
    return 0
}

# --- Estado del actor con fichero de PID ----------------------------------
#   0 = VIVO           (demostrado: el PID existe y es el nuestro)
#   1 = MUERTO         (demostrado: el PID ya no existe)
#   2 = INCONCLUYENTE  (el fichero existe pero no se puede interpretar)
#   3 = SIN REGISTRO   (no hay fichero)
pid_estado() {
    _pe_f=$1
    if [ ! -e "$_pe_f" ]; then return 3; fi
    if [ ! -r "$_pe_f" ]; then return 2; fi
    _pe_l=$(cat "$_pe_f" 2>/dev/null) || return 2
    _pe_p=${_pe_l%% *}
    case "$_pe_p" in ''|*[!0-9]*) return 2 ;; esac
    case "$_pe_l" in
        *" "*) _pe_s=${_pe_l#* } ;;
        *) _pe_s=0 ;;
    esac
    if ! kill -0 "$_pe_p" 2>/dev/null; then
        # Si /proc/<pid> sigue ahí, el proceso existe pero no se puede
        # señalar (permisos): NO se puede afirmar que esté muerto.
        [ -d "/proc/$_pe_p" ] && return 2
        return 1
    fi
    _pe_i=$(pid_proc_info "$_pe_p") || return 2
    _pe_now=${_pe_i%% *}
    _pe_st=${_pe_i#* }
    case "$_pe_st" in Z|z) return 1 ;; esac    # zombie: ya no hace nada
    [ "$_pe_s" = "0" ] && return 2             # sin starttime: no se confirma
    [ "$_pe_now" = "$_pe_s" ] && return 0      # es NUESTRO proceso
    return 1                                    # el PID se ha reciclado
}

# --- CERROJOS (creación ATÓMICA con noclobber) ----------------------------
# Un cerrojo es un fichero con "<pid> <starttime>" de quien lo tiene.
#   0 = lo tiene un proceso VIVO
#   1 = libre (o el titular está muerto)
#   2 = INCONCLUYENTE (no se puede saber)
lock_estado() {
    _le_f=$1
    if [ ! -e "$_le_f" ]; then return 1; fi
    if [ ! -r "$_le_f" ]; then return 2; fi
    _le_l=$(cat "$_le_f" 2>/dev/null) || return 2
    _le_p=${_le_l%% *}
    case "$_le_p" in ''|*[!0-9]*) return 2 ;; esac
    case "$_le_l" in
        *" "*) _le_s=${_le_l#* } ;;
        *) _le_s=0 ;;
    esac
    if ! kill -0 "$_le_p" 2>/dev/null; then
        [ -d "/proc/$_le_p" ] && return 2
        return 1
    fi
    [ "$_le_s" = "0" ] && return 2
    _le_i=$(pid_proc_info "$_le_p") || return 2
    _le_now=${_le_i%% *}
    if [ "$_le_now" = "$_le_s" ]; then return 0; fi
    return 1
}

# Toma el cerrojo.  rc: 0 = tomado, 1 = otro lo tiene (vivo), 2 = no se pudo
# saber (NO se toca).  La creación es ATÓMICA: con `set -C` el `>` falla si
# el fichero ya existe => dos actores no pueden tomar el mismo puesto.
lock_toma() {
    _lt_f=$1
    lock_estado "$_lt_f"
    _lt_r=$?
    # 0 = lo tiene uno vivo: NO se toca.
    if [ "$_lt_r" = "0" ]; then return 1; fi
    if [ "$_lt_r" = "2" ]; then
        # No se puede saber si lo tiene alguien vivo.  Sólo se roba si el
        # fichero es BASURA (no tiene un PID válido): si tiene un PID pero no
        # se puede comprobar, NO se toca (regla de oro).
        if [ -r "$_lt_f" ]; then
            _lt_e=$(cat "$_lt_f" 2>/dev/null)
            _lt_p=${_lt_e%% *}
            case "$_lt_p" in
                ''|*[!0-9]*) : ;;
                *) return 2 ;;
            esac
        else
            return 2
        fi
    fi
    # Libre (o basura): se roba.  El fichero viejo se borra ANTES (el `>`
    # con noclobber no pisa) y la creación nueva es ATÓMICA: si otro lo
    # toma entre medias, el `>` falla y su estado decide.
    [ -e "$_lt_f" ] && rm -rf "$_lt_f" 2>/dev/null
    _lt_s=$(pid_proc_info "$$") || _lt_s="0 ?"
    _lt_st=${_lt_s%% *}
    [ -n "$_lt_st" ] || _lt_st=0
    if ( set -C; printf '%s %s\n' "$$" "$_lt_st" > "$_lt_f" ) 2>/dev/null; then
        return 0
    fi
    lock_estado "$_lt_f"
    _lt_r=$?
    [ "$_lt_r" = "0" ] && return 1
    return 2
}

# Suelta el cerrojo (sólo si lo tenemos nosotros).
lock_suelta() {
    _ls_f=$1
    _ls_l=$(cat "$_ls_f" 2>/dev/null)
    _ls_p=${_ls_l%% *}
    [ "$_ls_p" = "$$" ] && rm -f "$_ls_f" 2>/dev/null
    return 0
}

# --- BARRIDO DE /proc (TRIVIAL) -------------------------------------------
# Rellena _PIDS (PIDs separados por espacios) y _LECTURA:
#   ok      = se pudo leer TODOS los cmdlines (barrido CONCLUYENTE)
#   parcial = algún cmdline NO se pudo leer (barrido INCONCLUYENTE)
# PROC_DIR se puede cambiar para poder PROBAR la función sin /proc real.
PROC_DIR="${PROC_DIR:-/proc}"
_PIDS=""
_LECTURA=ok
proc_pids_scan() {
    _pat=$1; shift
    _PIDS=""
    _LECTURA=ok
    for _pp in "$PROC_DIR"/[0-9]*; do
        [ -d "$_pp" ] || continue
        if [ ! -r "$_pp/cmdline" ]; then
            # El proceso se ha ido entre el glob y ahora: no es un problema.
            # Si el directorio SIGUE ahí pero el cmdline no se puede leer
            # (SELinux), el barrido es PARCIAL: no se puede concluir que ese
            # proceso no existe (y antes se descartaba en silencio => el
            # guardián relanzaba el supervisor DETRÁS del vivo).
            [ -d "$_pp" ] || continue
            _LECTURA=parcial
            continue
        fi
        _pc=$(tr '\0' ' ' 2>/dev/null < "$_pp/cmdline") || {
            _LECTURA=parcial
            continue
        }
        case "$_pc" in
            "sh -c "*|*/sh\ -c\ *) continue ;;
        esac
        _ok=1
        case "$_pc" in *"$_pat"*) ;; *) _ok=0 ;; esac
        for _x in "$@"; do
            case "$_pc" in *"$_x"*) ;; *) _ok=0 ;; esac
        done
        [ "$_ok" = "1" ] && _PIDS="$_PIDS ${_pp##*/}"
    done
    return 0
}

# La versión que imprime (la de siempre).
proc_pids() {
    proc_pids_scan "$@"
    for _p in $_PIDS; do printf '%s\n' "$_p"; done
}

# --- matar SOLO lo que es NUESTRO ------------------------------------------
# Por fichero de PID: el starttime registrado tiene que coincidir con el de
# /proc/<pid>/stat (si el PID se ha reciclado, NO se mata).
matar_pid_fichero() {
    _mf_f=$1
    _mf_l=$(cat "$_mf_f" 2>/dev/null) || return 1
    _mf_p=${_mf_l%% *}
    case "$_mf_p" in ''|*[!0-9]*) return 1 ;; esac
    case "$_mf_l" in
        *" "*) _mf_s=${_mf_l#* } ;;
        *) _mf_s=0 ;;
    esac
    [ "$_mf_s" = "0" ] && return 1
    _mf_i=$(pid_proc_info "$_mf_p") || return 1
    _mf_now=${_mf_i%% *}
    [ "$_mf_now" = "$_mf_s" ] || return 1
    kill -9 "$_mf_p" 2>/dev/null
}

# Por barrido: el cmdline se relee JUSTO antes del kill; si ya no se puede
# leer, NO se mata (podría ser otro proceso con el PID reciclado).
matar_pid_barrido() {
    _mb_p=$1
    _mb_pat=$2
    _mb_c=$(tr '\0' ' ' 2>/dev/null < "$PROC_DIR/$_mb_p/cmdline") || return 1
    case "$_mb_c" in *"$_mb_pat"*) ;; *) return 1 ;; esac
    kill -9 "$_mb_p" 2>/dev/null
}
# <<< BLOQUE PID-FILES (fin de la copia) <<<


mkdir -p "$LOG_DIR" 2>/dev/null

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] watchdog: $*" >> "$LOG"
    echo "$*"
}

cf_pids() {
    ps -A -o PID,NAME 2>/dev/null | grep -i cloudflared | grep -v grep | awk '{print $1}'
}

load1() {
    if [ -r /proc/loadavg ]; then
        set -- $(cat /proc/loadavg)
        echo "${1%%.*}"
    else
        echo 0
    fi
}

extraer_url() {
    grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' "$TUNNEL_LOG" 2>/dev/null | tail -1
}

# --- relanzar el tunel (SOLO si no hay uno vivo) --------------------------
# El cerrojo y el fichero de PID son la fuente de verdad: el 98, el watchdog
# y el supervisor COMPARTEN el mismo puesto del tunel => no se relanzan el
# tunel entre ellos (cada relanzamiento regenera el hostname publico).
relanzar_tunel() {
    lock_toma "$LOCK_CLOUDFLARED"
    case $? in
        1) log "otro actor esta relanzando el tunel: no lo hago"; return 1 ;;
        2) log "no se puede saber si otro actor esta relanzando el tunel: no lo hago"; return 1 ;;
    esac
    pid_estado "$PID_CLOUDFLARED"; _wd_e=$?
    if [ "$_wd_e" = "0" ]; then
        lock_suelta "$LOCK_CLOUDFLARED"
        log "el cloudflared registrado sigue vivo: no lo relanzo"
        return 0
    fi
    for p in $(cf_pids); do kill -9 "$p" 2>/dev/null; done
    sleep 1
    setsid nohup "$CF" tunnel --no-autoupdate --url "$TARGET" >> "$TUNNEL_LOG" 2>&1 &
    _wd_pid=$!
    sleep 1
    pid_escribe "$PID_CLOUDFLARED" "$_wd_pid" 2>/dev/null \
        || log "AVISO: no se pudo registrar el PID del cloudflared"
    lock_suelta "$LOCK_CLOUDFLARED"
    log "  relanzado cloudflared (setsid nohup, independiente del daemon)"
    return 0
}

# Dead: 530/000/502/503/504.  401/403 = el daemon respondió = VIVO.
es_muerto() {
    case "$1" in
        0|530|502|503|504) return 0 ;;
        *) return 1 ;;
    esac
}

# Sondea con el binario propio (el móvil no tiene curl).
sondear() {
    url="$1"
    [ -z "$url" ] && { echo 0; return; }
    if [ -x "$DAEMON_BIN" ]; then
        code=$("$DAEMON_BIN" probe "$url" 2>>"$LOG")
        case "$code" in
            ''|*[!0-9]*) code=0 ;;
        esac
        echo "$code"
        return
    fi
    if [ -x "$PUBLISH" ]; then
        code=$("$PUBLISH" --probe "$url" 2>>"$LOG")
        [ -z "$code" ] && code=0
        echo "$code"
        return
    fi
    # Sin binario: se responde por el SOCKET (sin red, sin proceso): se
    # comprueba el puerto en /proc/net/tcp.  Es un "vivo/no vivo", no un
    # codigo HTTP, pero evita depender de binarios externos.
    echo 0
}

publicar() {
    url="$1"
    pids="$2"
    # El binario principal publica el estado del canal (channel.json + paste).
    # El daemon tambien lo hace por su cuenta (cada 5 min, si cambia), asi
    # que esto es solo el refresco inmediato tras un tunel nuevo.
    if [ -x "$DAEMON_BIN" ]; then
        "$DAEMON_BIN" channel publish >>"$LOG" 2>&1
        log "publicado (daemon): $url (pids: $pids)"
        publicar_repo
        return
    fi
    if [ -x "$PUBLISH" ]; then
        # --state-dir explícito: si no, el binario usaría su valor por defecto
        # (otra ruta) => el estado se escribiría donde el daemon no lo lee.
        "$PUBLISH" --state-dir "$STATE_DIR" --quick "$url" --pids "$pids" >>"$LOG" 2>&1
        log "publicado: $url (pids: $pids)"
        publicar_repo
        return
    fi
    log "sin binario de publicacion: no puedo publicar (channel.json + paste)"
}

# Publica la URL VIVA en el REPO (state/channel.json).
#
# MEDIDO (2026-11-26): el "quick tunnel" regenera el hostname cada vez que
# cloudflared se relanza => si el relay (canónico) está caído, el sandbox se
# queda CIEGO: la URL nueva sólo existe en el paste (que rota y no se puede
# descubrir) y en el móvil.  El repo NO cachea (Contents API) => publicando
# allí, la flota siempre puede redescubrir el móvil.
publicar_repo() {
    tok=""
    for t in "$LOG_DIR/.gh_token" "$LOG_DIR/github.token" \
             "$STATE_DIR/.gh_token" "$STATE_DIR/github.token" \
             "$HOME/.mavis/github.token"; do
        if [ -f "$t" ]; then
            tok=$(tr -d '\r\n ' < "$t")
            break
        fi
    done
    if [ -z "$tok" ]; then
        log "sin token de GitHub: no puedo publicar el canal en el repo"
        return 1
    fi
    pub="$LOG_DIR/channel-publish"
    [ -x "$pub" ] || pub="$PUBLISH"
    if [ ! -x "$pub" ]; then
        log "sin channel-publish: no puedo publicar el canal en el repo"
        return 1
    fi
    # MEGADAEMON_GITHUB_TOKEN: el nombre que lee el binario (también acepta
    # GITHUB_TOKEN).  El token NUNCA se imprime.
    MEGADAEMON_GITHUB_TOKEN="$tok" "$pub" --repo-publish --state-dir "$STATE_DIR" \
        >>"$LOG" 2>&1
    log "canal publicado en el repo (state/channel.json)"
}

# --- el puesto del watchdog (UNO solo) ------------------------------------
# Dos watchdogs se pelean por el tunel: cada uno lo relanza y el hostname
# publico cambia cada 60 s (la flota se queda ciega).
lock_toma "$LOCK_WATCHDOG"
case $? in
    0) : ;;
    1) log "ya hay un watchdog vivo: no arranco"; exit 0 ;;
    2) log "AVISO: no se puede saber si hay otro watchdog vivo: sigo";;
esac
pid_escribe "$PID_WATCHDOG" "$$"

log "watchdog arrancado (interval=${INTERVAL}s max_load=${MAX_LOAD})"

while : ; do
    url=""
    [ -r "$URL_FILE" ] && url=$(cat "$URL_FILE" 2>/dev/null)
    url=$(echo "$url" | tr -d '\r\n ')

    if [ -z "$url" ]; then
        log "sin URL en $URL_FILE: relanzo para publicarla"
        L=$(load1)
        if [ "$L" -lt "$MAX_LOAD" ] 2>/dev/null; then
            relanzar_tunel
            i=0
            while [ "$i" -lt 24 ]; do
                sleep 5
                nueva=$(extraer_url)
                [ -n "$nueva" ] && { url="$nueva"; echo "$url" > "$URL_FILE"; break; }
                i=$((i + 1))
            done
            log "URL nueva: ${url:-ninguna}"
            [ -n "$url" ] && publicar "$url" "$(cf_pids | tr '\n' ' ')"
        else
            log "movil SATURADO (load1=$L): no relanzo"
        fi
        sleep "$INTERVAL"
        continue
    fi

    code=$(sondear "$url")
    if es_muerto "$code"; then
        L=$(load1)
        log "tunel MUERTO (HTTP $code) en $url - load1=$L"
        if [ "$L" -ge "$MAX_LOAD" ] 2>/dev/null; then
            log "  movil SATURADO (load1=$L >= $MAX_LOAD): no relanzo"
            sleep "$INTERVAL"
            continue
        fi
        relanzar_tunel
        i=0
        while [ "$i" -lt 24 ]; do
            sleep 5
            nueva=$(extraer_url)
            if [ -n "$nueva" ] && [ "$nueva" != "$url" ]; then
                log "  URL nueva: $nueva"
                echo "$nueva" > "$URL_FILE"
                publicar "$nueva" "$(cf_pids | tr '\n' ' ')"
                break
            fi
            i=$((i + 1))
        done
    else
        # 401/403 = el daemon respondió: el túnel está VIVO. No se toca nada.
        log "tunel vivo (HTTP $code) $url"
    fi

    sleep "$INTERVAL"
done
