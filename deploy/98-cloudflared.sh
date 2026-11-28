#!/system/bin/sh
# ---------------------------------------------------------------------------
# CLOUDFLARED ÚNICO e INDEPENDIENTE del daemon  (Magisk service.d)
#
# Por qué este fichero existe (medido el 2026-11-21):
#
#   * El *quick tunnel* **regenera el hostname en cada reinicio** ⇒ si
#     `cloudflared` es hijo del daemon, **reiniciar el daemon tumba el túnel
#     Y cambia la URL** ⇒ el sandbox se queda ciego al móvil.
#   * Había **DOS `cloudflared` peleando** (pids 1934 y 7358) ⇒ el que se
#     ganaba el edge era aleatorio ⇒ la URL cambiaba sola y nadie sabía cuál
#     era la buena.
#
# Qué hace este script (arranque del móvil, FUERA del daemon):
#   1. mata TODOS los `cloudflared` y deja **exactamente uno**, lanzado con
#      `setsid nohup` ⇒ **no es hijo** del daemon ni del supervisor ⇒
#      reiniciar el daemon **no** tumba el túnel;
#   2. lee la URL del log y la escribe en `state/tunnel.url`;
#   3. lanza `tunnel-watchdog.sh` (el que sondea y relanza cuando toca).
#
# Instalación:  /data/adb/service.d/98-cloudflared.sh   (Magisk lo ejecuta)
# Variables:    CFT_TARGET (por defecto http://127.0.0.1:9090)
#               MAX_LOAD  (por defecto 8: con el móvil saturado NO relanza)
# ---------------------------------------------------------------------------
# MEDIDO (2026-11-27, movil SM-G998B): el tunel escribia su `tunnel.log` y su
# `tunnel.url` en `/data/local/tmp/megadaemon` (VOLATIL: se borra al
# reiniciar) mientras el one-liner los busca en `/data/adb/megadaemon/state`
# (PERSISTENTE) => el movil quedaba "sin URL de tunel" y el watchdog
# reiniciaba el tunel cada 60 s sin motivo.  Ahora el prefijo por defecto es
# el PERSISTENTE, con el volátil como plan B.
if [ -z "${MEGADAEMON_PREFIX:-}" ]; then
    if [ -d /data/adb/megadaemon ] || [ -d /data/adb ]; then
        MEGADAEMON_PREFIX=/data/adb/megadaemon
    else
        MEGADAEMON_PREFIX=/data/local/tmp/megadaemon
    fi
fi
PREFIX="$MEGADAEMON_PREFIX"
LOG_DIR="$PREFIX"
# El tunel NO puede depender de Termux (es una app: si el usuario la cierra
# o la reinstala, el tunel se cae y el movil se queda sin canal publico).
# Ver MEDIDO el 2026-11-21.  Orden: prefijo propio -> copia migrada desde
# Termux -> Termux como ultimo recurso.
CF="${CF:-}"
if [ -z "$CF" ]; then
    for cand in "$PREFIX/cloudflared" \
                /data/local/tmp/megadaemon/cloudflared \
                /data/data/com.termux/files/usr/bin/cloudflared \
                /data/data/com.agy/files/usr/bin/cloudflared; do
        [ -x "$cand" ] && { CF="$cand"; break; }
    done
fi
# Si solo esta en Termux, se copia al prefijo (root puede leerlo).
if [ -n "$CF" ] && [ "$CF" != "$PREFIX/cloudflared" ] \
   && [ -x "$CF" ] && [ ! -x "$PREFIX/cloudflared" ]; then
    mkdir -p "$PREFIX" 2>/dev/null
    cp -f "$CF" "$PREFIX/cloudflared.new" 2>/dev/null \
        && chmod 755 "$PREFIX/cloudflared.new" 2>/dev/null \
        && mv -f "$PREFIX/cloudflared.new" "$PREFIX/cloudflared" 2>/dev/null \
        && CF="$PREFIX/cloudflared"
fi
[ -n "$CF" ] || CF=/data/data/com.termux/files/usr/bin/cloudflared
TARGET="${CFT_TARGET:-http://127.0.0.1:9090}"
URL_FILE="$LOG_DIR/tunnel.url"
# ⚠️ El daemon lee su estado de AQUÍ (cockpit_server::state_dir()) ⇒ la URL
# del túnel tiene que estar en el MISMO directorio o el `quick` del
# /channel.json nunca se rellena (medido: el script escribía en el log_dir
# y el daemon leía en otro sitio ⇒ el `quick` salía vacío siempre).
# ⚠️ Este script lo ejecuta **Magisk** (service.d) => es HERMANO del
# supervisor, no hijo suyo => NO hereda su entorno => hay que leer el env
# aquí (sin `set -a`: la API key no se exporta a cloudflared ni al watchdog).
ENV_FILE="${MEGADAEMON_ENV:-$PREFIX/megadaemon.env}"
if [ -f "$ENV_FILE" ]; then
    . "$ENV_FILE" 2>/dev/null
fi
# Ni la key ni el token de sesión viajan a cloudflared/watchdog: este
# script no los necesita (y una variable que ya estaba exportada en el
# entorno mantiene su atributo de exportación al reasignarla).
unset MEGADAEMON_API_KEY MEGADAEMON_COCKPIT_TOKEN 2>/dev/null
# El valor por defecto es el del despliegue (`install-phone.sh`): en el móvil
# el estado vive en /data/local/tmp/megadaemon/state.
STATE_DIR="${MEGADAEMON_STATE_DIR:-$PREFIX/state}"
STATE_URL_FILE="$STATE_DIR/tunnel.url"
# Publicador del canal (opcional): al subir el túnel, se publica el estado
# para que el enjambre conozca la URL nueva sin intervención manual.
PUBLISH="${MEGADAEMON_PUBLISH:-$PREFIX/channel-publish}"
TUNNEL_LOG="$LOG_DIR/tunnel.log"
WATCHDOG="$LOG_DIR/tunnel-watchdog.sh"
MAX_LOAD="${MAX_LOAD:-8}"

# --- ficheros de PID y cerrojos -------------------------------------------
# MEDIDO (2026-11-28, movil): el 98, el watchdog y el supervisor se relanzaban
# el tunel ENTRE ELLOS (cada relanzamiento cambia el hostname publico y deja
# ciega a la flota) y ademas el guardián multiplicaba el stack entero (load
# 58).  Ahora el "¿existe ya?" se responde con ficheros de PID + cerrojos,
# que NO dependen de leer /proc/<pid>/cmdline.
PID_CLOUDFLARED="$PREFIX/cloudflared.pid"
PID_WATCHDOG="$PREFIX/watchdog.pid"
LOCK_CLOUDFLARED="$PREFIX/.lock-cloudflared"
LOCK_WATCHDOG="$PREFIX/.lock-watchdog"

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
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] 98-cloudflared: $*" >> "$LOG_DIR/cloudflared.log"
}

# --- pids de cloudflared (sin depender de pidof) --------------------------
cf_pids() {
    ps -A -o PID,NAME 2>/dev/null | grep -i cloudflared | grep -v grep | awk '{print $1}'
}

# --- ¿el móvil está saturado? ---------------------------------------------
load1() {
    if [ -r /proc/loadavg ]; then
        set -- $(cat /proc/loadavg)
        echo "${1%%.*}"
    else
        echo 0
    fi
}

# --- deja EXACTAMENTE UN cloudflared ---------------------------------------
reiniciar_cloudflared() {
    L=$(load1)
    if [ "$L" -ge "$MAX_LOAD" ] 2>/dev/null; then
        log "movil SATURADO (load1=$L >= $MAX_LOAD): no relanzo cloudflared"
        return 1
    fi
    # --- ¿hay ya un cloudflared vivo? (fichero de PID: NO cmdline) -------
    # Relanzar el tunel REGENERA el hostname publico => si ya hay uno vivo no
    # se toca (es lo que deja ciega a la flota).
    pid_estado "$PID_CLOUDFLARED"; _cf_e=$?
    if [ "$_cf_e" = "0" ]; then
        log "ya hay un cloudflared vivo: no lo relanzo (el hostname se conserva)"
        return 0
    fi
    # --- el cerrojo: si otro actor esta relanzando, no se hace nada ------
    lock_toma "$LOCK_CLOUDFLARED"
    case $? in
        1) log "otro actor esta relanzando el tunel: no lo hago"; return 0 ;;
        2) log "no se puede saber si otro actor esta relanzando el tunel: no lo hago"; return 0 ;;
    esac
    # Dentro del cerrojo: se vuelve a comprobar (otro puede haber relanzado
    # mientras tomabamos el cerrojo).
    pid_estado "$PID_CLOUDFLARED"; _cf_e=$?
    if [ "$_cf_e" = "0" ]; then
        lock_suelta "$LOCK_CLOUDFLARED"
        log "otro actor relanzo el tunel: no lo hago"
        return 0
    fi
    for p in $(cf_pids); do
        log "matando cloudflared pid $p (dejamos solo uno)"
        kill -9 "$p" 2>/dev/null
    done
    sleep 1
    # setsid + nohup => proceso independiente del daemon y del supervisor.
    setsid nohup "$CF" tunnel --no-autoupdate --url "$TARGET" >> "$TUNNEL_LOG" 2>&1 &
    _cf_pid=$!
    sleep 1
    pid_escribe "$PID_CLOUDFLARED" "$_cf_pid" 2>/dev/null \
        || log "AVISO: no se pudo registrar el PID del cloudflared"
    lock_suelta "$LOCK_CLOUDFLARED"
    log "lanzado cloudflared -> $TARGET (load1=$L)"
    return 0
}

# --- la URL nueva (la imprime cloudflared en el log) -----------------------
extraer_url() {
    grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' "$TUNNEL_LOG" 2>/dev/null | tail -1
}

# --- arranca ----------------------------------------------------------------
reiniciar_cloudflared
i=0
while [ "$i" -lt 24 ]; do
    url=$(extraer_url)
    if [ -n "$url" ]; then
        echo "$url" > "$URL_FILE"
        # y en el directorio de estado del daemon (el que lee el canal)
        mkdir -p "$STATE_DIR" 2>/dev/null
        if echo "$url" > "$STATE_URL_FILE" 2>/dev/null; then
            log "URL vigente: $url (también en $STATE_URL_FILE)"
        else
            log "URL vigente: $url (NO se pudo escribir $STATE_URL_FILE)"
        fi
        break
    fi
    sleep 5
    i=$((i + 1))
done

# --- publica el canal (para que el enjambre vea la URL nueva) -------------
# Best effort y en segundo plano: el túnel NO depende de esto (además el
# móvil no lleva PAT ⇒ se publica en un paste, no en el repo).
if [ -x "$PUBLISH" ] && [ -r "$STATE_URL_FILE" ]; then
    setsid nohup sh -c "'$PUBLISH' --state-dir '$STATE_DIR'" >>"$LOG_DIR/publish.log" 2>&1 &
    log "publicación del canal lanzada (paste; sin PAT no se publica en el repo)"
else
    log "sin publicador ($PUBLISH) o sin URL: no se publica (el /channel.json del daemon sigue siendo la vía principal)"
fi

# --- watchdog (sondeo + relanzado + publicación) ---------------------------
if [ -r "$WATCHDOG" ]; then
    # Si ya hay un watchdog vivo, NO se relanza: dos watchdogs se pelean
    # por el tunel (cada uno lo relanza y el hostname cambia cada 60 s).
    pid_estado "$PID_WATCHDOG"; _wd_e=$?
    if [ "$_wd_e" = "0" ]; then
        log "ya hay un watchdog vivo: no lo relanzo"
    else
        setsid nohup sh "$WATCHDOG" >> "$LOG_DIR/tunnel-watchdog.log" 2>&1 &
        log "watchdog lanzado"
    fi
else
    log "NO existe $WATCHDOG (copia deploy/tunnel-watchdog.sh al movil)"
fi

log "cloudflared pids: $(cf_pids | tr '\n' ' ')"
