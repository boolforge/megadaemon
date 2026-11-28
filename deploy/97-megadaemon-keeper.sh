#!/system/bin/sh
# 97-megadaemon-keeper.sh — GUARDIÁN del supervisor (Magisk service.d)
#
# Se instala en: /data/adb/service.d/97-megadaemon-keeper.sh (chmod 755)
#
# POR QUÉ EXISTE (medido el 2026-11-23 en el Samsung del usuario):
#   El supervisor (99-megadaemon.sh) es lo único que relanza el daemon si
#   este muere.  Si el supervisor ALSO muere, el móvil se queda sin daemon
#   Y sin supervisor: el túnel de Cloudflare sigue vivo (lo sostiene otro
#   script de service.d) pero responde 502 para siempre, y no hay quien
#   reinstale nada.  MEDIDO: 1644 reinicios del daemon por un falso positivo
#   del watchdog, y después el supervisor murió sin dejar NADIE que lo
#   resucitara => móvil inalcanzable hasta un reinicio manual.
#
#   Este script es el guardián del guardián: cada 60 s comprueba que el
#   supervisor siga vivo y, si no, lo relanza.  Es un script hermano de
#   service.d (no un hijo del supervisor), así que no hereda su entorno ni
#   muere con él.
#
# MEDIDO (2026-11-28, móvil Samsung+Magisk): ESTE GUARDIÁN ERA EL QUE
# MULTIPLICABA EL MÓVIL.  Decidía si el supervisor vivía LEYENDO el cmdline:
#       [ -r "$_pp/cmdline" ] || continue        <-- FALSO NEGATIVO
#   Con SELinux el cmdline de un proceso de otro dominio/UID NO se puede
#   leer => el proceso se descartaba en silencio => el guardián creía que el
#   supervisor estaba MUERTO y lo relanzaba DETRÁS del que ya estaba vivo.
#   Cascada: N supervisores => N cloudflared + N daemons + N watchdogs =>
#   load1=58 => "movil SATURADO (load1=58 >= 8): no relanzo" y "tunel MUERTO
#   (HTTP 0)".  MEDIDO en las DOS ejecuciones del one-liner: supervisor=4
#   watchdog=4 cloudflared=5, daemons=6 y load1=58 (idéntico en las dos).
#
#   AHORA la comprobación NO depende del cmdline: usa el FICHERO DE PID del
#   supervisor y su CERROJO (BLOQUE PID-FILES más abajo) y, cuando hay que
#   mirar /proc, el barrido es TRIVIAL ("no se pudo leer" NO es "no existe").
#
# REGLAS:
#   * NO reinicia el daemon directamente: de eso se encarga el supervisor.
#   * Espera a que el sistema se asiente (arranque) antes de actuar.
#   * Es idempotente: si ya hay un supervisor, no hace nada.
#   * Si la comprobación es INCONCLUYENTE, NO relanza NADA (regla de oro:
#     es preferible quedarse sin supervisor a tener seis).
#   * Escribe en /sdcard/megadaemon.log (mismo log que el supervisor).
#
# SI HAY QUE FORZAR UN REINICIO (a mano): el puesto del supervisor se
# guarda en $PREFIX/.lock-supervisor y su PID en $PREFIX/supervisor.pid.
# Con el supervisor vivo pero ATASCADO, uno nuevo no arranca (no puede haber
# dos): se borran los dos ficheros y se lanza el supervisor a mano.

LOG="${MEGADAEMON_LOG:-/sdcard/megadaemon.log}"
SUPERVISOR="${MEGADAEMON_SUPERVISOR:-/data/adb/service.d/99-megadaemon.sh}"
# MEDIDO (2026-11-23): ruta PERSISTENTE (la vieja, /data/local/tmp, es
# volatil y el reinicio del movil se llevaba el binario).
PREFIX="${MEGADAEMON_PREFIX:-/data/adb/megadaemon}"
# Tiempos configurables (para poder PROBAR el script sin esperar minutos).
START_WAIT="${KEEPER_START_WAIT:-90}"
INTERVAL="${KEEPER_INTERVAL:-60}"
RETRY_WAIT="${KEEPER_RETRY_WAIT:-20}"
# Ficheros de PID y cerrojos (prefijo PERSISTENTE).
PID_SUPERVISOR="$PREFIX/supervisor.pid"
PID_KEEPER="$PREFIX/keeper.pid"
LOCK_KEEPER="$PREFIX/.lock-keeper"
LOCK_SUPERVISOR="$PREFIX/.lock-supervisor"

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] keeper: $*" >> "$LOG" 2>/dev/null
}

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


# --- ¿está vivo el supervisor? (TRES estados) ------------------------------
#   0 = VIVO   1 = MUERTO   2 = INCONCLUYENTE
supervisor_estado() {
    # Las señales SIN cmdline mandan: el cerrojo (quién tiene el puesto) y
    # el fichero de PID (pid + starttime).
    lock_estado "$LOCK_SUPERVISOR"; _se_l=$?
    pid_estado "$PID_SUPERVISOR"; _se_p=$?
    [ "$_se_l" = "0" ] && return 0
    [ "$_se_p" = "0" ] && return 0
    # El cerrojo libre Y el PID registrado ya no existe => muerto (prueba
    # sin cmdline: todo supervisor toma el cerrojo al arrancar).
    if [ "$_se_l" = "1" ] && [ "$_se_p" = "1" ]; then
        return 1
    fi
    # Sin pruebas claras: decide el barrido, y SÓLO si es CONCLUYENTE.
    proc_pids_scan "service.d/99-megadaemon.sh"
    if [ -n "$_PIDS" ]; then
        set -- $_PIDS
        pid_escribe "$PID_SUPERVISOR" "$1" 2>/dev/null
        return 0
    fi
    [ "$_LECTURA" = "ok" ] && return 1
    return 2
}

# --- los supervisores que SOBRAN (nunca al revés) --------------------------
# Si el barrido es CONCLUYENTE y hay más de un supervisor, se queda el
# canónico (el del fichero de PID; si no, el más antiguo) y se matan los
# demás.  Antes de cada kill se relee el cmdline: si ya no se puede leer, NO
# se mata (el PID podría ser ya otro proceso).
matar_duplicados() {
    proc_pids_scan "service.d/99-megadaemon.sh"
    [ "$_LECTURA" = "ok" ] || return 0
    [ -n "$_PIDS" ] || return 0
    # Quién se queda: el que tiene el PUESTO (el cerrojo), si no el del
    # fichero de PID, si no el más antiguo.  NUNCA se matan todos.
    _md_lk=$(cat "$LOCK_SUPERVISOR" 2>/dev/null)
    _md_keep=${_md_lk%% *}
    _md_found=0
    for _md_p in $_PIDS; do
        [ "$_md_p" = "$_md_keep" ] && _md_found=1
    done
    if [ "$_md_found" != "1" ]; then
        _md_l=$(cat "$PID_SUPERVISOR" 2>/dev/null)
        _md_keep=${_md_l%% *}
        for _md_p in $_PIDS; do
            [ "$_md_p" = "$_md_keep" ] && _md_found=1
        done
    fi
    if [ "$_md_found" != "1" ]; then
        set -- $_PIDS
        _md_keep=$1
    fi
    for _md_p in $_PIDS; do
        [ "$_md_p" = "$_md_keep" ] && continue
        if matar_pid_barrido "$_md_p" "service.d/99-megadaemon.sh"; then
            log "supervisor duplicado PID $_md_p eliminado (se queda el canónico)"
        fi
    done
    return 0
}

# --- el puesto del keeper (UNO solo) --------------------------------------
mkdir -p "$PREFIX" 2>/dev/null
lock_toma "$LOCK_KEEPER"
case $? in
    0) : ;;
    1) log "ya hay un keeper vivo: no arranco"; exit 0 ;;
    2) log "AVISO: no se puede saber si hay otro keeper vivo: sigo (el puesto del supervisor es el que cuenta)";;
esac
pid_escribe "$PID_KEEPER" "$$"

# Al arrancar, Magisk lanza TODOS los scripts de service.d a la vez: dar
# margen para que el supervisor se instale y el sistema se asiente.
sleep "$START_WAIT"

while true; do
    sleep "$INTERVAL"
    supervisor_estado
    _ke=$?
    case "$_ke" in
        0)
            # El supervisor vive: no se relanza NADA.  Si el barrido es
            # concluyente y hay más de uno, se matan los sobrantes.
            matar_duplicados
            ;;
        1)
            log "el supervisor no esta vivo: lo relanzo"
            if [ -x "$SUPERVISOR" ]; then
                setsid nohup sh "$SUPERVISOR" >/dev/null 2>&1 &
                sleep "$RETRY_WAIT"
                supervisor_estado
                _kr=$?
                if [ "$_kr" = "0" ]; then
                    log "supervisor relanzado"
                elif [ "$_kr" = "2" ]; then
                    log "AVISO: el supervisor se lanzo pero no se puede comprobar si vive"
                else
                    log "AVISO: el supervisor no arranco"
                fi
            else
                log "ERROR: no existe $SUPERVISOR (no se puede relanzar)"
            fi
            ;;
        2)
            # REGLA DE ORO: comprobación INCONCLUYENTE (no se pudo leer el
            # cmdline de un proceso que podría ser el supervisor) => NO se
            # relanza NADA.  Preferimos quedarnos sin supervisor a tener
            # seis: un falso positivo se cura solo, un falso negativo
            # multiplica.
            log "no se puede comprobar si el supervisor vive (cmdline ilegible): NO relanzo"
            ;;
    esac
done
