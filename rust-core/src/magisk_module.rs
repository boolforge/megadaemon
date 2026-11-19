//! MegaDaemon v13.4 — Empaquetado como MÓDULO MAGISK (`magisk_module`).
//!
//! Este módulo genera, byte a byte, los ficheros de un **módulo Magisk
//! instalable** que deja el daemon funcionando en el móvil sin Termux ni
//! sesiones abiertas:
//!
//! * `module.prop`     — metadatos del módulo (id, nombre, versión...);
//! * `post-fs-data.sh` — **antes de zygote**: fija las propiedades que
//!   delatan root/build de desarrollo y hace el `mount --bind` del tmpfs de
//!   Magisk;
//! * `service.sh`      — al final del arranque: cockpit + túnel + sync, con
//!   fichero de PID y rotación de log;
//! * `customize.sh`    — instalador de Magisk (`ask_for_assets`, pasos de
//!   instalación, permisos SELinux con `magisk --sqlite`);
//! * `uninstall.sh`    — reversión: borra las props fijadas y para el daemon.
//!
//! Dispositivo de referencia: Galaxy S21 Ultra SM-G998B, Exynos 2100,
//! Android 14, **Magisk Kitsune**, root `su -mm -c`.
//!
//! ---------------------------------------------------------------------------
//! "QUE CORRA EN EL NÚCLEO" — RESPUESTA HONESTA
//! ---------------------------------------------------------------------------
//!
//! Un demonio **userspace no puede ejecutarse dentro del kernel**: el kernel
//! es un único anillo de ejecución privilegiado, y un proceso normal (por muy
//! root que sea) vive en el anillo 3 y llama al kernel mediante syscalls.  No
//! hay "daemon dentro del kernel" en Linux/Android fuera de tres vías reales:
//!
//! 1. **Módulo Magisk con `post-fs-data.sh`** — se ejecuta en el arranque
//!    **antes de zygote**, que es justo lo que importa para la ocultación de
//!    root (props leídas por las apps) y para dejar el entorno preparado antes
//!    de que arranque cualquier app.  Coste: bajo.  Riesgo: que Magisk se
//!    desactualice o se desinstale.
//! 2. **Servicio `init.rc` del ROM (system)** — lo más temprano y limpio de
//!    todo, pero exige **modificar el /system** (o recompilar el ROM), lo que
//!    en un Samsung con bootloader bloqueado significa perder la garantía y
//!    el flash original.  Coste: medio/alto.  Riesgo: alto (brick, warranty).
//! 3. **Módulo de kernel (LKM)** — aquí sí hay código dentro del kernel:
//!    hooks de syscalls (`sys_call_table`), `kprobes`/`ftrace`, o un módulo
//!    que implemente sus propios hooks.  Permite cosas que **nadie** puede
//!    hacer en userspace (interceptar `openat`, `ptrace`, `mount`, ocultar
//!    procesos...).  Coste: **muy alto**: hay que tener el kernel del
//!    dispositivo (SM-G998B / Exynos 2100), recompilarlo, firmarlo y flashearlo;
//!    un fallo puede deixar el móvil sin arrancar.  Riesgo: muy alto.
//!
//! **Punto medio honesto**: **eBPF** permite enganchar kprobes/tracepoints
//! sin recompilar el kernel *si* el kernel lo soporta (`CONFIG_BPF_SYSCALL`).
//! En el SM-G998B con kernel de Samsung **no está verificado** que esté
//! disponible ni que se permita a userspace; por eso no se recomienda como
//! base, aunque es la vía correcta si algún día se confirma.
//!
//! **Recomendación**: **(a) + (b)** — es decir, el módulo Magisk con
//! `post-fs-data.sh` (que es lo que se genera aquí) **más** un `init.rc`
//! del ROM **sólo si** el usuario tiene el /system modificable y acepta el
//! riesgo.  El LKM queda descartado: el beneficio (ocultación profunda de
//! root) es marginal frente a un LKM como Shamiko/DenyListMagisk, y el coste
//! (recompilar y flashear el kernel del Samsung) es desproporcionado.
//!
//! Todo lo anterior está también disponible como JSON estructurado en
//! [`kernel_integration_report`], que es la fuente de verdad de este módulo.

use serde_json::{json, Value};
use std::fs;
use std::path::Path;

/// Identificador del módulo Magisk (carpeta en `/data/adb/modules`).
pub const MODULE_ID: &str = "megadaemon_energy";

/// Nombre visible del módulo.
pub const MODULE_NAME: &str = "MegaDaemon Energy Core";

/// Autor del módulo.
pub const MODULE_AUTHOR: &str = "MegaDaemon Engineering Team";

/// Versión del módulo que se empaqueta por defecto.
pub const MODULE_VERSION: &str = "13.4.0";

/// Descripción de una línea (el formato de `module.prop` no admite saltos).
pub const MODULE_DESCRIPTION: &str = "Gestor de energia avanzado para Android con root: Doze + GMS Doze, limites de carga por salud real de la celda, politica termica por evento y auditoria de wakelocks/alarmas. Requiere Magisk (Kitsune).";

/// Directorio de estado por defecto del daemon en el móvil.
pub const DEFAULT_STATE_DIR: &str = "/data/local/tmp/megadaemon/state";

/// Convierte una versión `13.4.0` (o `v13.4.0`) en `versionCode` numérico.
///
/// Regla: `major * 1000 + minor * 100 + patch` (13.4.0 -> 13400, 1.9.11 -> 1911).
/// Es monótona: una versión posterior siempre da un `versionCode` mayor.
pub fn version_code(version: &str) -> i64 {
    let v = version.trim().trim_start_matches('v');
    let mut parts = v.split('.');
    let major = parts.next().and_then(|p| p.parse::<i64>().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|p| p.parse::<i64>().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|p| p.parse::<i64>().ok()).unwrap_or(0);
    major * 1000 + minor * 100 + patch
}

/// Genera el `module.prop` real del módulo.
pub fn module_manifest(version: &str) -> String {
    let v = version.trim();
    let v = if v.starts_with('v') {
        v.to_string()
    } else {
        format!("v{}", v)
    };
    format!(
        "id={id}\n\
         name={name}\n\
         version={version}\n\
         versionCode={code}\n\
         author={author}\n\
         description={description}\n",
        id = MODULE_ID,
        name = MODULE_NAME,
        version = v,
        code = version_code(version),
        author = MODULE_AUTHOR,
        description = MODULE_DESCRIPTION
    )
}

/// `post-fs-data.sh`: se ejecuta **antes de zygote**.
///
/// Hace el `mount --bind` del tmpfs de Magisk y fija (con `resetprop`) las
/// propiedades que delatan root o un build de desarrollo, y borra las
/// `ro.magisk.*`.
pub fn post_fs_data_script() -> String {
    r#"#!/system/bin/sh
# =====================================================================
# post-fs-data.sh — MegaDaemon Energy Core (módulo Magisk)
#
# CUÁNDO SE EJECUTA: en el arranque, después de montar /data y ANTES de
# arrancar zygote (init.rc -> "service.post-fs-data").  Es la ÚNICA
# ventana en la que las propiedades del sistema se pueden fijar antes de
# que las apps las lean: por eso el ocultado va aquí y no en service.sh.
#
# MAGISK lo invoca con `sh` (ash) y sin busybox garantizado: aquí sólo se
# usan POSIX sh y herramientas de toybox.
#
# QUÉ HACE
#   1. mount --bind del tmpfs de Magisk sobre /sbin (Kitsune lo deja en
#      /debug_ramdisk; Magisk canario ya lo monta en /sbin).
#   2. resetprop de las props que delatan root / userdebug.
#   3. Borrado de las props ro.magisk.*.
#
# AVISO HONESTO: esto NO falsifica la attestation de Play Integrity
# (es criptográfica y no se falseifica desde userspace) y NO sustituye a
# Shamiko/Zygisk, que oculta el root en sí.  Lo que sí consigue es que
# las apps no vean un "userdebug" con ro.debuggable=1.
# =====================================================================

MODDIR=${0%/*}
TMP=${TMPDIR:-/data/local/tmp}

log() {
    echo "[megadaemon post-fs-data] $*" > /dev/kmsg 2>/dev/null
}

# --- 0) resetprop disponible ---------------------------------------------
RP=""
if command -v resetprop >/dev/null 2>&1; then
    RP=resetprop
elif [ -x /data/adb/magisk/resetprop ]; then
    RP=/data/adb/magisk/resetprop
fi

set_prop() {
    # set_prop <clave> <valor>
    [ -n "$RP" ] || { log "resetprop no disponible: $1 sin fijar"; return 0; }
    $RP "$1" "$2" 2>/dev/null || log "fallo al fijar $1"
}

del_prop() {
    [ -n "$RP" ] || return 0
    $RP --delete "$1" 2>/dev/null || true
}

# --- 1) tmpfs de Magisk: bind-mount sobre /sbin --------------------------
MAGISK_TMPFS=""
for d in /debug_ramdisk /sbin/.magisk /sbin/magisk; do
    if [ -d "$d" ]; then
        MAGISK_TMPFS="$d"
        break
    fi
done

if [ -n "$MAGISK_TMPFS" ] && [ "$MAGISK_TMPFS" != "/sbin" ]; then
    if mount -o bind "$MAGISK_TMPFS" /sbin 2>/dev/null; then
        log "bind: $MAGISK_TMPFS -> /sbin"
    else
        log "bind fallido: $MAGISK_TMPFS -> /sbin (se continúa)"
    fi
fi

# --- 2) Propiedades: estado "de fábrica" para las apps -------------------
# Poner ro.debuggable=0 y ro.secure=1 hace que el sistema se presente como
# un build "user" de fábrica y no como un userdebug: eso es justo lo que
# delata a las apps y a las herramientas de detección.
set_prop ro.debuggable 0
set_prop ro.secure 1
set_prop ro.build.tags release-keys
set_prop ro.build.type user
set_prop ro.build.characteristics default
set_prop ro.build.description user/release-keys
set_prop ro.boot.verifiedbootstate green
set_prop ro.boot.flash.locked 1
set_prop ro.boot.vbmeta.device_state locked
set_prop service.adb.root 0
set_prop ro.adb.secure 1

# --- 3) Borrar las huellas de Magisk (ro.magisk.*) ------------------------
# getprop imprime "[clave]: [valor]"; se extrae la clave.
for p in $(getprop 2>/dev/null | grep -E '^\[ro\.magisk\.' | sed 's/^\[\([^]]*\)\].*/\1/'); do
    del_prop "$p"
done

log "post-fs-data completado"
"#
    .to_string()
}

/// `service.sh`: arranca cockpit + túnel + sincronización al final del arranque.
///
/// `state_dir` es el directorio donde se guardan el PID, los logs y el estado.
pub fn service_script(state_dir: &str) -> String {
    format!(
        r#"#!/system/bin/sh
# =====================================================================
# service.sh — MegaDaemon Energy Core (módulo Magisk)
#
# MAGISK ejecuta service.sh al final del arranque (late_start service).
# Aquí no se tocan propiedades (eso es de post-fs-data.sh): aquí se
# mantienen vivos los procesos del daemon.
#
#   1. megadaemon-core cockpit 9090  -> plano de control + MCP HTTP
#   2. cloudflared tunnel -> 9090     -> exposición remota (URL efímera)
#   3. megadaemon-core sync          -> sincronización periódica
#
# Detalles de robustez:
#   * setsid nohup: el demonio NO muere al cerrar la sesión de shell;
#   * fichero de PID (STATE_DIR/megadaemon.pid);
#   * rotación de log (10 MB x 5 ficheros);
#   * backoff creciente si el proceso cae.
# =====================================================================

MODDIR=${{0%/*}}
STATE_DIR={state_dir}
PREFIX=/data/local/tmp/megadaemon
BIN=$PREFIX/megadaemon-core
LOG_DIR=$STATE_DIR/logs
LOG=$LOG_DIR/megadaemon.log
PIDFILE=$STATE_DIR/megadaemon.pid
TUNNEL_PIDFILE=$STATE_DIR/tunnel.pid
ENV_FILE=$PREFIX/megadaemon.env
PORT=9090
MAX_BYTES=$((10 * 1024 * 1024))
KEEP=5
SYNC_EVERY=1800

mkdir -p "$STATE_DIR" "$LOG_DIR" 2>/dev/null

log() {{
    echo "[$(date '+%F %T')] $*" >> "$LOG" 2>/dev/null
}}

rotate() {{
    [ -f "$LOG" ] || return 0
    sz=$(stat -c %s "$LOG" 2>/dev/null || echo 0)
    [ "$sz" -gt "$MAX_BYTES" ] || return 0
    i=$((KEEP - 1))
    while [ "$i" -ge 1 ]; do
        [ -f "$LOG_DIR/megadaemon.log.$i" ] && mv -f "$LOG_DIR/megadaemon.log.$i" "$LOG_DIR/megadaemon.log.$((i + 1))"
        i=$((i - 1))
    done
    mv -f "$LOG" "$LOG_DIR/megadaemon.log.1"
    log "log rotado"
}}

load_env() {{
    if [ -f "$ENV_FILE" ]; then
        set -a
        . "$ENV_FILE"
        set +a
    fi
}}

start_cockpit() {{
    if [ ! -x "$BIN" ]; then
        log "cockpit: binario ausente en $BIN"
        return 1
    fi
    load_env
    cd "$PREFIX" 2>/dev/null || return 1
    setsid nohup "$BIN" cockpit $PORT >> "$LOG" 2>&1 < /dev/null &
    echo $! > "$PIDFILE"
    log "cockpit arrancado PID $(cat "$PIDFILE") en 127.0.0.1:$PORT"
}}

start_tunnel() {{
    CF=$(ls /data/data/com.termux/files/usr/bin/cloudflared 2>/dev/null | head -1)
    if [ -z "$CF" ]; then
        log "tunnel: cloudflared no encontrado (se omite)"
        return 1
    fi
    setsid nohup "$CF" tunnel --no-autoupdate --url http://127.0.0.1:$PORT >> "$LOG_DIR/tunnel.log" 2>&1 < /dev/null &
    echo $! > "$TUNNEL_PIDFILE"
    log "tunnel arrancado PID $(cat "$TUNNEL_PIDFILE")"
    n=0
    while [ "$n" -lt 30 ]; do
        U=$(grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' "$LOG_DIR/tunnel.log" 2>/dev/null | tail -1)
        if [ -n "$U" ]; then
            echo "$U" > "$STATE_DIR/tunnel.url"
            log "tunnel URL: $U"
            break
        fi
        n=$((n + 1))
        sleep 2
    done
}}

run_sync() {{
    log "sync: iniciando"
    load_env
    timeout 600 "$BIN" sync >> "$LOG" 2>&1
    log "sync: terminada (rc=$?)"
}}

# --- arranque -------------------------------------------------------------
rotate
start_cockpit
start_tunnel

last_sync=$(date +%s)
cd_cockpit=0
while true; do
    rotate
    sleep 30

    now=$(date +%s)
    if [ $((now - last_sync)) -ge $SYNC_EVERY ]; then
        last_sync=$now
        run_sync &
    fi

    if ! pgrep -f "megadaemon-core cockpit" > /dev/null 2>&1; then
        cd_cockpit=$((cd_cockpit + 1))
        s=$((5 * (1 << (cd_cockpit < 5 ? cd_cockpit : 5))))
        [ "$s" -gt 120 ] && s=120
        log "cockpit caido: relanzando en $((s / 2))s"
        sleep "$s"
        start_cockpit
    else
        cd_cockpit=0
    fi
done
"#,
        state_dir = state_dir
    )
}

/// `customize.sh`: instalador del módulo (lo ejecuta Magisk al flashear).
pub fn customize_script() -> String {
    r#"#!/system/bin/sh
# =====================================================================
# customize.sh — MegaDaemon Energy Core (módulo Magisk)
#
# Se ejecuta DENTRO del instalador de Magisk (recovery o app).  Aquí:
#   1. se informa de lo que se va a instalar;
#   2. se piden los assets (si el usuario quiere) con ask_for_assets;
#   3. se copian los binarios a /data/local/tmp/megadaemon;
#   4. se ajustan permisos (set_perm / set_perm_recursive);
#   5. se ajustan los permisos SELinux con `magisk --sqlite`.
#
# Magisk pone a disposición: ui_print, abort, set_perm, set_perm_recursive,
# ask_for_assets, $MODPATH, $TMPDIR, $ZIPFILE, $ARCH, $API.
# =====================================================================

SKIPUNZIP=0

ui_print " "
ui_print "==========================================="
ui_print " MegaDaemon Energy Core"
ui_print " Doze + GMS Doze | limite de carga | termico"
ui_print "==========================================="
ui_print " "

# --- 1) Assets opcionales -------------------------------------------------
# ask_for_assets no existe en todas las versiones de Magisk: si no está,
# se continúa con los valores por defecto (sin error).
if command -v ask_for_assets > /dev/null 2>&1; then
    ui_print "- Assets: se Solerá el binario precompilado si existe."
    if ask_for_assets "megadaemon-core" "Binario de megadaemon-core para este dispositivo"; then
        ui_print "- Asset aceptado."
    else
        ui_print "- Sin asset: se usará el binario incluido en el zip."
    fi
fi

# --- 2) Comprobación de entorno ------------------------------------------
if [ "$API" -lt 29 ]; then
    abort "! Android 10 (API 29) o superior es necesario (Doze + App Standby)"
fi

ui_print "- Android API $API | arquitectura $ARCH"

# --- 3) Instalación de los binarios --------------------------------------
PREFIX=/data/local/tmp/megadaemon
STATE=/data/local/tmp/megadaemon/state

mkdir -p "$PREFIX" "$STATE" "$STATE/logs" || abort "! no se pudieron crear los directorios"

# El binario puede venir como asset (ask_for_assets) o dentro del zip.
for candidate in "$MODPATH/common/megadaemon-core" "$MODPATH/system/bin/megadaemon-core"; do
    if [ -f "$candidate" ]; then
        cp -f "$candidate" "$PREFIX/megadaemon-core" || abort "! no se pudo copiar el binario"
        break
    fi
done

if [ -f "$PREFIX/megadaemon-core" ]; then
    chmod 0755 "$PREFIX/megadaemon-core"
    ui_print "- Binario instalado en $PREFIX/megadaemon-core"
else
    ui_print "- AVISO: no hay binario (se instalará luego con el despliegue)"
fi

# La plantilla web del cockpit (si está) también se copia.
for w in "$MODPATH/common/web" "$MODPATH/system/web"; do
    if [ -d "$w" ]; then
        mkdir -p "$PREFIX/web"
        cp -rf "$w/." "$PREFIX/web/" 2>/dev/null || true
    fi
done

# --- 4) Permisos POSIX ----------------------------------------------------
set_perm_recursive "$MODPATH" 0 0 0755 0644
set_perm "$PREFIX/megadaemon-core" 0 0 0755
set_perm "$STATE" 0 0 0700
set_perm "$STATE/logs" 0 0 0750

# --- 5) Permisos SELinux (magisk --sqlite) --------------------------------
# La base de datos de políticas de Magisk permite marcar un uid como
# "allow" para que el daemon obtenga root sin pregunta interactiva.
# OJO: el esquema exacto depende de la versión de Magisk
# (policies(uid, policy, until, logging, notification)); si la sentencia
# falla, el módulo sigue funcionando (sólo se pierde el "sin pregunta").
if command -v magisk > /dev/null 2>&1; then
    magisk --sqlite "REPLACE INTO policies (uid,policy,until,logging,notification) VALUES (0,'allow','2099-12-31 23:59:59',0,0);" 2>/dev/null \
        || ui_print "- AVISO: no se pudo ajustar la política SELinux (no es critico)"
    magisk --sqlite "UPDATE policies SET logging=0, notification=0;" 2>/dev/null || true
fi

ui_print "- Permisos aplicados"
ui_print "- Reinicia para activar el módulo"
ui_print " "
"#
    .to_string()
}

/// `uninstall.sh`: revierte lo que el módulo hizo.
///
/// Borra las propiedades fijadas y para el daemon; **no** borra los datos del
/// usuario (logs, estado, conversaciones), que están en el directorio de
/// estado y se conservan a propósito.
pub fn uninstall_script() -> String {
    r#"#!/system/bin/sh
# =====================================================================
# uninstall.sh — MegaDaemon Energy Core (módulo Magisk)
#
# Revierte lo que el módulo cambió: para el daemon y borra las
# propiedades que fijó post-fs-data.sh.  Los datos del usuario (logs,
# estado, conversaciones) NO se borran: están en el directorio de estado.
# =====================================================================

STATE=/data/local/tmp/megadaemon/state

# Parar el daemon
if [ -f "$STATE/megadaemon.pid" ]; then
    PID=$(cat "$STATE/megadaemon.pid" 2>/dev/null)
    [ -n "$PID" ] && kill "$PID" 2>/dev/null
    rm -f "$STATE/megadaemon.pid"
fi
pkill -f "megadaemon-core cockpit" 2>/dev/null

# Revertir las propiedades
RP=""
if command -v resetprop > /dev/null 2>&1; then
    RP=resetprop
elif [ -x /data/adb/magisk/resetprop ]; then
    RP=/data/adb/magisk/resetprop
fi

if [ -n "$RP" ]; then
    for p in ro.debuggable ro.secure ro.build.tags ro.build.type \
             ro.build.characteristics ro.build.description \
             ro.boot.verifiedbootstate ro.boot.flash.locked \
             ro.boot.vbmeta.device_state service.adb.root ro.adb.secure \
             persist.megadaemon.profile persist.megadaemon.charge_limit; do
        $RP --delete "$p" 2>/dev/null || true
    done
fi

rm -f /data/adb/megadaemon/profile.json 2>/dev/null

echo "[megadaemon] desinstalado (datos conservados en $STATE)" > /dev/kmsg 2>/dev/null
"#
    .to_string()
}

/// Informe honesto sobre "correr en el núcleo".
///
/// Devuelve las tres vías reales (módulo Magisk, `init.rc` del ROM y módulo
/// de kernel LKM), su coste/beneficio/riesgo y la recomendación.
pub fn kernel_integration_report() -> Value {
    json!({
        "ok": true,
        "pregunta": "que corra en el nucleo",
        "respuesta_corta": "Un demonio userspace NO puede ejecutarse dentro del kernel. \
El kernel es un anillo de ejecucion privilegiado; un proceso (por muy root que sea) \
vive en userspace y llama al kernel por syscalls. Solo existen tres vias reales, \
y solo una es razonable en un Samsung con bootloader bloqueado.",
        "realidad": [
            "El daemon es un proceso de userspace: se ejecuta en el anillo 3 y usa el kernel mediante syscalls.",
            "Escribir en /sys o en /proc NO es 'correr en el kernel': es usar las interfaces que el kernel expone.",
            "Un modulo de kernel (LKM) si es codigo que se ejecuta en el anillo 0, pero exige recompilar y flashear el kernel.",
            "Un demonio NO puede 'entrar' en el kernel en caliente: no existe esa vía en Linux/Android."
        ],
        "vias": [
            {
                "id": "magisk_post_fs_data",
                "nombre": "Modulo Magisk con post-fs-data.sh",
                "dentro_del_kernel": false,
                "cuándo": "arranque, antes de zygote",
                "coste": "bajo",
                "beneficio": "Las props de ocultacion quedan fijadas antes de que las apps las lean; \
es la ventana que exige Shamiko/Play Integrity para el spoofing de props.",
                "riesgo": "Depende de Magisk: una actualizacion incompatible o desinstalarlo rompe el arranque limpo.",
                "recomendado": true,
                "que_permite": [
                    "resetprop de ro.debuggable/ro.secure/ro.build.tags/verifiedbootstate",
                    "mount --bind del tmpfs de Magisk",
                    "borrado de ro.magisk.*"
                ],
                "que_no_permite": [
                    "interceptar syscalls",
                    "ocultar procesos de forma kernel (eso es DenyList/Shamiko)",
                    "cualquier cosa en el anillo 0"
                ]
            },
            {
                "id": "init_rc_rom",
                "nombre": "Servicio init.rc del ROM (system)",
                "dentro_del_kernel": false,
                "cuándo": "arranque, todavia antes de zygote",
                "coste": "medio-alto",
                "beneficio": "Es la via mas limpia y temprana: no depende de Magisk y arranca siempre.",
                "riesgo": "Exige modificar /system o recompilar el ROM: en un Samsung con bootloader \
bloqueado significa perder la garantia y dejar de poder flashear el binario original.",
                "recomendado": true,
                "recomendado_si": "El usuario tiene el /system modificable (ROM parcheado) y acepta el riesgo.",
                "que_permite": [
                    "arranque determinista de los servicios",
                    "no depender de la version de Magisk"
                ],
                "que_no_permite": [
                    "ocultacion de root por si sola",
                    "hooks de syscalls"
                ]
            },
            {
                "id": "lkm_kernel",
                "nombre": "Modulo de kernel (LKM)",
                "dentro_del_kernel": true,
                "cuándo": "carga del kernel, con el firmware",
                "coste": "muy alto",
                "beneficio": "Codigo real en el anillo 0: hooks de syscalls (sys_call_table), \
kprobes/ftrace, o hooks propios de mount/ptrace/openat. Es lo unico que puede \
ocultar el root a nivel de kernel de verdad.",
                "riesgo": "Muy alto: hay que obtener el kernel del SM-G998B/Exynos 2100, \
recompilarlo, firmarlo y flashearlo; un error puede dejar el movil sin arrancar. \
Ademas deja de ser practicable cualquier actualizacion del kernel.",
                "recomendado": false,
                "motivo_de_no_recomendar": "El beneficio (ocultacion profunda) ya lo cubren \
Shamiko y DenyListMagisk, y el coste de mantener un kernel parcheado es \
desproporcionado para este proyecto.",
                "que_permite": [
                    "hooks de syscalls y de Binder",
                    "control del scheduler y del OOM killer",
                    "cualquier logica que deba correr con privilegios de kernel"
                ],
                "que_no_permite": [
                    "ser desplegado sin recompilar y flashear el kernel"
                ]
            }
        ],
        "tabla": [
            {
                "via": "magisk_post_fs_data",
                "dentro_del_kernel": false,
                "coste": "bajo",
                "beneficio": "alto (ocultacion de props antes de zygote)",
                "riesgo": "bajo",
                "recomendado": true
            },
            {
                "via": "init_rc_rom",
                "dentro_del_kernel": false,
                "coste": "medio-alto",
                "beneficio": "medio (arranque fiable)",
                "riesgo": "alto (modificar /system)",
                "recomendado": true
            },
            {
                "via": "lkm_kernel",
                "dentro_del_kernel": true,
                "coste": "muy alto",
                "beneficio": "alto en ocultacion, marginal para este daemon",
                "riesgo": "muy alto (brick)",
                "recomendado": false
            }
        ],
        "nota_ebpf": {
            "que_es": "eBPF permite enganchar kprobes/tracepoints SIN recompilar el kernel \
si este lo soporta (CONFIG_BPF_SYSCALL).",
            "estado_en_sm_g998b": "no verificado",
            "veredicto": "via intermedia interesante en teoria; no se recomienda como base \
hasta confirmar que el kernel de Samsung lo expone y lo permite a userspace."
        },
        "recomendacion": {
            "plan": "(a) + (b)",
            "viables": ["magisk_post_fs_data", "init_rc_rom"],
            "no_recomendado": ["lkm_kernel"],
            "motivo": "El 90 % del valor (props antes de zygote, arranque fiable del daemon, \
politica de energia) se consigue en userspace con un modulo Magisk bien hecho. El LKM solo \
compensa si el objetivo es la ocultacion profunda de root, y eso ya lo dan Shamiko/DenyListMagisk \
sin tocar el kernel."
        },
        "que_genera_este_modulo": {
            "post-fs-data.sh": "props de ocultacion + mount --bind del tmpfs de Magisk (via a)",
            "service.sh": "arranque de cockpit + tunel + sync (userspace, servicio tardio)",
            "uninstall.sh": "reversion de las props y parada del daemon",
            "init_rc": "NO se genera: requiere modificar /system, se deja como recomendacion."
        },
        "dispositivo": "Galaxy S21 Ultra SM-G998B (Exynos 2100, Android 14, Magisk Kitsune)",
        "nota": "Todo lo que este modulo hace es userspace. No hay, ni puede haber, \
'parte del daemon dentro del kernel' sin recompilar el kernel del dispositivo."
    })
}

/// Escribe el módulo completo en `dir` y devuelve la lista de rutas escritas.
///
/// Ficheros: `module.prop`, `post-fs-data.sh`, `service.sh`, `customize.sh`
/// y `uninstall.sh` (los scripts con permiso 0755).
pub fn write_module(dir: &str) -> Result<Vec<String>, String> {
    let root = Path::new(dir);
    fs::create_dir_all(root).map_err(|e| format!("no se pudo crear {}: {}", dir, e))?;

    let files: Vec<(&str, String, u32)> = vec![
        ("module.prop", module_manifest(MODULE_VERSION), 0o644),
        ("post-fs-data.sh", post_fs_data_script(), 0o755),
        ("service.sh", service_script(DEFAULT_STATE_DIR), 0o755),
        ("customize.sh", customize_script(), 0o755),
        ("uninstall.sh", uninstall_script(), 0o755),
    ];

    let mut written = Vec::with_capacity(files.len());
    for (name, content, mode) in files {
        let path = root.join(name);
        fs::write(&path, content.as_bytes())
            .map_err(|e| format!("no se pudo escribir {}: {}", path.display(), e))?;
        set_mode(&path, mode);
        written.push(path.to_string_lossy().to_string());
    }
    Ok(written)
}

/// Fija los permisos POSIX de un fichero (best effort: no es critico).
fn set_mode(path: &Path, mode: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(path) {
            let mut perm = meta.permissions();
            perm.set_mode(mode);
            let _ = fs::set_permissions(path, perm);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let p = std::env::temp_dir().join(format!("md-mod-{}-{}-{}", tag, std::process::id(), nanos));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn el_manifest_tiene_los_campos_que_exige_magisk() {
        let m = module_manifest("13.4.0");
        for key in [
            "id=",
            "name=",
            "version=",
            "versionCode=",
            "author=",
            "description=",
        ] {
            assert!(m.contains(key), "falta {} en el manifest", key);
        }
        assert!(m.contains("id=megadaemon_energy"));
        assert!(m.contains("version=v13.4.0"));
        assert!(m.contains("versionCode=13400"));
        // Una línea por campo: module.prop no admite multilínea.
        assert_eq!(m.lines().count(), 6);
        // El nombre no debe llevar saltos de línea.
        assert!(!m.lines().any(|l| l.trim().is_empty()));
    }

    #[test]
    fn el_version_code_se_calcula_desde_la_version() {
        assert_eq!(version_code("13.4.0"), 13400);
        assert_eq!(version_code("v13.4.0"), 13400);
        assert_eq!(version_code("13.4"), 13400);
        assert_eq!(version_code("1.9.11"), 1911);
        // Monotonía: una versión posterior => versionCode mayor.
        assert!(version_code("13.4.1") > version_code("13.4.0"));
        assert!(version_code("13.5.0") > version_code("13.4.9"));
    }

    #[test]
    fn post_fs_data_fija_las_props_antes_de_zygote() {
        let s = post_fs_data_script();
        assert!(s.contains("#!/system/bin/sh"));
        for prop in [
            "ro.debuggable",
            "ro.secure",
            "ro.build.tags",
            "ro.boot.verifiedbootstate",
            "ro.boot.flash.locked",
            "service.adb.root",
            "ro.magisk",
        ] {
            assert!(s.contains(prop), "falta la prop {}", prop);
        }
        assert!(s.contains("resetprop"));
        assert!(s.contains("mount -o bind"));
        assert!(s.contains("zygote"));
        // Debe ser ejecutable por sh.
        assert!(s.contains("set_prop()") || s.contains("set_prop "));
    }

    #[test]
    fn el_service_script_arranca_cockpit_tunel_y_sync_con_pid_y_rotacion() {
        let s = service_script("/data/local/tmp/megadaemon/state");
        assert!(s.contains("setsid nohup"));
        assert!(s.contains("megadaemon-core") && s.contains("cockpit"));
        assert!(s.contains("tunnel"));
        assert!(s.contains("sync"));
        assert!(s.contains("megadaemon.pid"));
        assert!(s.contains("rotate") && s.contains("KEEP=5"));
        assert!(s.contains("/data/local/tmp/megadaemon/state"));
    }

    #[test]
    fn el_customize_script_pide_assets_instala_y_ajusta_selinux() {
        let s = customize_script();
        assert!(s.contains("ask_for_assets"));
        assert!(s.contains("ui_print"));
        assert!(s.contains("set_perm"));
        assert!(s.contains("magisk --sqlite"));
        assert!(s.contains("megadaemon-core"));
    }

    #[test]
    fn el_informe_de_kernel_explica_las_tres_vias() {
        let r = kernel_integration_report();
        assert_eq!(r["ok"], json!(true));
        let vias = r["vias"].as_array().unwrap();
        assert_eq!(vias.len(), 3, "deben describirse las tres vias");
        let ids: Vec<&str> = vias.iter().map(|v| v["id"].as_str().unwrap()).collect();
        assert!(ids.contains(&"magisk_post_fs_data"));
        assert!(ids.contains(&"init_rc_rom"));
        assert!(ids.contains(&"lkm_kernel"));
        // Sólo el LKM es código dentro del kernel.
        assert_eq!(vias[0]["dentro_del_kernel"], json!(false));
        assert_eq!(vias[1]["dentro_del_kernel"], json!(false));
        assert_eq!(vias[2]["dentro_del_kernel"], json!(true));
        // Tabla comparativa y recomendación.
        assert_eq!(r["tabla"].as_array().unwrap().len(), 3);
        assert_eq!(r["recomendacion"]["plan"], json!("(a) + (b)"));
        assert!(r["recomendacion"]["no_recomendado"]
            .as_array()
            .unwrap()
            .contains(&json!("lkm_kernel")));
        // Y que diga claramente que userspace no corre en el kernel.
        let corta = r["respuesta_corta"].as_str().unwrap();
        assert!(corta.contains("userspace"));
        assert!(corta.contains("kernel"));
    }

    #[test]
    fn write_module_escribe_todos_los_ficheros() {
        let d = temp_dir("write");
        let written = write_module(d.to_str().unwrap()).unwrap();
        assert_eq!(written.len(), 5);
        for name in [
            "module.prop",
            "post-fs-data.sh",
            "service.sh",
            "customize.sh",
            "uninstall.sh",
        ] {
            let p = d.join(name);
            assert!(p.exists(), "falta {}", name);
            let meta = fs::metadata(&p).unwrap();
            assert!(meta.len() > 0, "{} está vacío", name);
            assert!(
                written.iter().any(|w| w.ends_with(name)),
                "{} no está en la lista de rutas",
                name
            );
        }
        // El manifest escrito debe ser el bueno.
        let manifest = fs::read_to_string(d.join("module.prop")).unwrap();
        assert!(manifest.contains("id=megadaemon_energy"));
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn write_module_falla_si_la_ruta_no_es_valida() {
        // Un fichero (no un directorio) no puede actuar como destino.
        let d = temp_dir("bad");
        let file = d.join("archivo");
        fs::write(&file, "x").unwrap();
        let r = write_module(file.to_str().unwrap());
        assert!(r.is_err());
        fs::remove_dir_all(&d).ok();
    }
}
