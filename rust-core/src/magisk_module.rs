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

    // v15: el módulo que se instala es el NUEVO (install/uninstall con
    // manifiesto y chmod 755 después del mv).  El heredado se conserva como
    // `uninstall_script()` para no romper a quien lo use.
    let files: Vec<(&str, String, u32)> = vec![
        ("module.prop", module_prop(), 0o644),
        ("post-fs-data.sh", module_post_fs_data_script(), 0o755),
        ("service.sh", module_service_script(), 0o755),
        ("customize.sh", customize_script(), 0o755),
        ("uninstall.sh", uninstall_script_v15(), 0o755),
        ("install-energy.sh", install_script(), 0o755),
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
        assert_eq!(written.len(), 6);
        for name in [
            "module.prop",
            "post-fs-data.sh",
            "service.sh",
            "customize.sh",
            "uninstall.sh",
            "install-energy.sh",
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

// ===========================================================================
// v15 — MÓDULO MAGISK QUE SE INSTALA Y SE DESINSTALA LIMPIO
// ===========================================================================
//
// REGLAS DEL ENCARGO (y cómo se cumplen en el código):
//
//  1. "sin panic=abort": el daemon que instala este módulo se compila SIN
//     `panic = "abort"` (ver `rust-core/Cargo.toml`, perfil release).  Con
//     abort, CUALQUIER panic (por ejemplo un mutex envenenado) mata el
//     proceso entero => el origen del túnel cae => 502/530.  Sin abort, el
//     panic se queda en la conexión y el daemon sigue sirviendo.
//
//  2. "sin dejar el móvil sin root": este módulo NUNCA toca el Magisk de la
//     máquina: ni `magisk --remove-modules`, ni `/data/adb/magisk`, ni los
//     módulos de otros.  Sólo escribe dentro de SU PROPIO directorio de
//     módulo y en `/data/local/tmp/megadaemon`.  El lint de abajo lo verifica.
//
//  3. "el supervisor tiene que poder relanzar el daemon aunque el módulo
//     falle": el lanzador real es `/data/adb/service.d/99-megadaemon.sh`, que
//     Magisk ejecuta en cada arranque aunque no haya NINGÚN módulo instalado.
//     El `service.sh` del módulo sólo comprueba que el supervisor existe y
//     está sano; si el módulo falla, el service.d sigue relanzando el daemon.
//
//  4. "el `mv` de un binario deja el fichero SIN bit +x => chmod 755 DESPUÉS
//     del `mv`": todas las instalaciones pasan por `install_exec()`, que hace
//     `mv` y LUEGO `chmod 755`.  El lint comprueba el ORDEN en el texto
//     generado, no sólo que aparezca un chmod.

/// Versión del módulo (código numérico para Magisk).
pub const MODULE_VERSION_CODE: i64 = 15_00;

/// Manifiesto de lo instalado: sin esto no se puede desinstalar limpio.
pub const INSTALL_MANIFEST: &str = "/data/adb/modules/megadaemon_energy/installed.manifest";

/// Ruta del supervisor (el lanzador real, de Magisk `service.d`).
pub const SUPERVISOR_PATH: &str = "/data/adb/service.d/99-megadaemon.sh";

/// Prefijo de trabajo del daemon.
pub const PREFIX_DIR: &str = "/data/local/tmp/megadaemon";

/// Genera el script de instalación EN MARCHA del payload del módulo.
///
/// Instala (si existen) el binario del daemon y el supervisor, valida el
/// supervisor con `sh -n` ANTES de moverlo, deja copia del anterior para
/// rollback y escribe el manifiesto de lo instalado.
pub fn install_script() -> String {
    r#"#!/system/bin/sh
# =====================================================================
# install-energy.sh — instala el payload del módulo en marcha.
#
# REGLA DE ORO (medida el 2026-11-20): un `mv` puede dejar el fichero
# SIN el bit +x (el supervisor llegaba a decir "no existe" cuando lo que
# pasaba es "no ejecutable").  Por eso TODO lo que se instala pasa por
# install_exec(), que hace `mv` y LUEGO `chmod 755`.
#
# Si algo falla, se RESTAURA lo anterior: el móvil nunca se queda sin
# daemon por un módulo fallido.
# =====================================================================
set -u

PREFIX=/data/local/tmp/megadaemon
MODDIR=/data/adb/modules/megadaemon_energy
MANIFEST=$MODDIR/installed.manifest
SUPERVISOR=/data/adb/service.d/99-megadaemon.sh
SRC_DIR="${1:-$PREFIX}"

log() { echo "[megadaemon-energy] $*"; }
die() { log "ERROR: $*"; exit 1; }

# --- install_exec <origen> <destino> ------------------------------------
# Copia, valida, mueve y PERMITE LA EJECUCIÓN DESPUÉS DEL mv.
install_exec() {
    src="$1"; dst="$2"
    [ -f "$src" ] || { log "no existe el origen $src"; return 1; }
    tmp="$dst.tmp.$$"
    cp -f "$src" "$tmp" || { log "no se pudo copiar $src"; return 1; }
    chmod 755 "$tmp" || { log "no se pudo chmod 755 el temporal"; return 1; }
    mv -f "$tmp" "$dst" || { log "no se pudo mover a $dst"; rm -f "$tmp"; return 1; }
    # >>> AQUÍ va el chmod 755: DESPUÉS del mv, nunca antes <<<
    chmod 755 "$dst" || { log "no se pudo chmod 755 $dst"; return 1; }
    [ -x "$dst" ] || { log "$dst NO es ejecutable tras el mv"; return 1; }
    return 0
}

mkdir -p "$MODDIR" "$PREFIX" || die "no se pudieron crear los directorios"

: > "$MANIFEST"

# --- 1) daemon ----------------------------------------------------------
if [ -f "$SRC_DIR/megadaemon-core" ]; then
    [ -f "$PREFIX/megadaemon-core" ] && cp -f "$PREFIX/megadaemon-core" "$PREFIX/megadaemon-core.prev"
    if install_exec "$SRC_DIR/megadaemon-core" "$PREFIX/megadaemon-core"; then
        echo "$PREFIX/megadaemon-core" >> "$MANIFEST"
        log "daemon instalado: $PREFIX/megadaemon-core ($(stat -c %s "$PREFIX/megadaemon-core" 2>/dev/null) bytes, +x=$( [ -x "$PREFIX/megadaemon-core" ] && echo si || echo NO ))"
    else
        log "fallo al instalar el daemon; se restaura el anterior"
        [ -f "$PREFIX/megadaemon-core.prev" ] && cp -f "$PREFIX/megadaemon-core.prev" "$PREFIX/megadaemon-core" && chmod 755 "$PREFIX/megadaemon-core"
        die "no se pudo instalar el daemon (se restauró el anterior)"
    fi
else
    log "no hay binario en $SRC_DIR: se conserva el ya instalado"
fi

# --- 2) supervisor ------------------------------------------------------
# Se valida con `sh -n` ANTES de tocar el que está en marcha: un supervisor
# con error de sintaxis dejaría el móvil sin daemon.
if [ -f "$SRC_DIR/99-megadaemon.sh" ]; then
    if sh -n "$SRC_DIR/99-megadaemon.sh" 2>/dev/null; then
        cp -f "$SUPERVISOR" "$SUPERVISOR.prev-energy" 2>/dev/null
        if install_exec "$SRC_DIR/99-megadaemon.sh" "$SUPERVISOR"; then
            echo "$SUPERVISOR" >> "$MANIFEST"
            echo "$SUPERVISOR.prev-energy" >> "$MANIFEST"
            log "supervisor instalado (sintaxis validada)"
        else
            log "fallo al instalar el supervisor; se restaura el anterior"
            [ -f "$SUPERVISOR.prev-energy" ] && cp -f "$SUPERVISOR.prev-energy" "$SUPERVISOR" && chmod 755 "$SUPERVISOR"
        fi
    else
        log "el supervisor propuesto NO pasa sh -n: no se instala (se queda el actual)"
    fi
else
    log "no hay supervisor en $SRC_DIR: se conserva el actual"
fi

chmod 700 "$MANIFEST" 2>/dev/null
log "instalación terminada; manifiesto en $MANIFEST"
"#
    .to_string()
}

/// Genera el script de desinstalación (v15).
///
/// Desinstala SÓLO lo que está en el manifiesto, restaura el daemon anterior
/// (no lo borra: si lo borrara, el móvil se quedaría sin binario para
/// reiniciar) y NO toca el Magisk de la máquina ni otros módulos.
pub fn uninstall_script_v15() -> String {
    r#"#!/system/bin/sh
# =====================================================================
# uninstall-energy.sh — desinstalación limpia del módulo.
#
# Reglas:
#   * sólo se borra lo que aparece en el manifiesto de instalación;
#   * el daemon se RESTAURA (copia .prev), nunca se borra: si se borrara, el
#     supervisor no podría relanzarlo y el móvil se quedaría sin daemon;
#   * NO se toca /data/adb/magisk, ni el binario de Magisk, ni los módulos
#     de otros: el móvil conserva root SIEMPRE;
#   * el daemon se relanza desde service.d en el siguiente arranque.
# =====================================================================
set -u

PREFIX=/data/local/tmp/megadaemon
MODDIR=/data/adb/modules/megadaemon_energy
MANIFEST=$MODDIR/installed.manifest
SUPERVISOR=/data/adb/service.d/99-megadaemon.sh

log() { echo "[megadaemon-energy] $*"; }

if [ -f "$MANIFEST" ]; then
    while IFS= read -r f; do
        [ -n "$f" ] || continue
        case "$f" in
            "$PREFIX/megadaemon-core")
                # NO se borra el daemon: se RESTAURA el anterior.
                if [ -f "$PREFIX/megadaemon-core.prev" ]; then
                    mv -f "$PREFIX/megadaemon-core.prev" "$PREFIX/megadaemon-core"
                    # el mv puede perder el bit +x: chmod DESPUÉS
                    chmod 755 "$PREFIX/megadaemon-core"
                    log "daemon restaurado desde $PREFIX/megadaemon-core.prev (ejecutable=$([ -x "$PREFIX/megadaemon-core" ] && echo si || echo NO))"
                else
                    log "no hay copia anterior del daemon: se conserva el actual"
                fi
                ;;
            "$PREFIX"/*|"$MODDIR"/*)
                rm -f "$f" && log "borrado $f"
                ;;
            *)
                log "NO se borra (fuera del ámbito): $f"
                ;;
        esac
    done < "$MANIFEST"
    rm -f "$MANIFEST"
fi

# Restaurar el supervisor anterior si el módulo lo había sustituido.
if [ -f "$SUPERVISOR.prev-energy" ]; then
    cp -f "$SUPERVISOR.prev-energy" "$SUPERVISOR" && chmod 755 "$SUPERVISOR" \
        && log "supervisor restaurado desde $SUPERVISOR.prev-energy"
    rm -f "$SUPERVISOR.prev-energy"
fi

log "desinstalación terminada. Magisk y el root NO se han tocado."
"#
    .to_string()
}

/// Genera el `service.sh` del módulo.
///
/// NO es el lanzador del daemon: el lanzador es `service.d` (que corre
/// aunque no haya módulos).  Este script sólo comprueba que el supervisor
/// está y es sano, y lo relanza si falta.
pub fn module_service_script() -> String {
    r#"#!/system/bin/sh
# =====================================================================
# service.sh — arranque tardío del MÓDULO (no del daemon).
#
# El daemon lo lanza /data/adb/service.d/99-megadaemon.sh, que Magisk
# ejecuta en CADA arranque aunque no haya módulos instalados.  Si este
# módulo falla o se desinstala, el supervisor sigue ahí y el daemon se
# relanza igual: el módulo no es un punto único de fallo.
# =====================================================================
MODDIR=/data/adb/modules/megadaemon_energy
SUPERVISOR=/data/adb/service.d/99-megadaemon.sh

log() { echo "[megadaemon-energy] $*" >> /data/local/tmp/megadaemon/energy_module.log; }

log "service.sh ejecutado"

if [ ! -f "$SUPERVISOR" ]; then
    log "AVISO: no hay supervisor en $SUPERVISOR"
    exit 0
fi

if ! sh -n "$SUPERVISOR" 2>/dev/null; then
    log "AVISO: el supervisor tiene error de sintaxis; no se toca"
    exit 0
fi

if ! pgrep -f "99-megadaemon.sh" > /dev/null 2>&1; then
    log "el supervisor no está corriendo: se relanza"
    sh "$SUPERVISOR" >> /data/local/tmp/megadaemon/supervisor.log 2>&1 &
fi

log "estado: supervisor presente y sano"
exit 0
"#
    .to_string()
}

/// Genera el `post-fs-data.sh` del módulo.
///
/// Corre antes de que /data esté descifrado por completo: aquí SÓLO se
/// preparan directorios y se registra la versión.  Ninguna acción que
/// pueda fallar y dejar el móvil sin root.
pub fn module_post_fs_data_script() -> String {
    r#"#!/system/bin/sh
# =====================================================================
# post-fs-data.sh — fase temprana (antes de /data descifrado).
#
# Aquí NO se arranca nada: en esta fase el sistema todavía no está listo
# y un fallo dejaría el móvil sin servicio.  Sólo se preparan rutas.
# =====================================================================
MODDIR=/data/adb/modules/megadaemon_energy
PREFIX=/data/local/tmp/megadaemon

mkdir -p "$PREFIX" 2>/dev/null
mkdir -p "$PREFIX/state" 2>/dev/null
mkdir -p "$MODDIR" 2>/dev/null
exit 0
"#
    .to_string()
}

/// Genera el `module.prop` (manifiesto) del módulo.
pub fn module_prop() -> String {
    format!(
        "id={id}\nname={name}\nversion={version}\nversionCode={code}\nauthor={author}\ndescription={desc}\n",
        id = MODULE_ID,
        name = MODULE_NAME,
        version = MODULE_VERSION,
        code = MODULE_VERSION_CODE,
        author = MODULE_AUTHOR,
        desc = MODULE_DESCRIPTION,
    )
}

/// Comprueba que los scripts generados NO violan las reglas del encargo.
///
/// Devuelve la lista de violaciones (vacía = todo correcto).  Es la forma de
/// que las reglas sean verificables y no una promesa.
pub fn lint_scripts() -> Vec<String> {
    let mut bad = Vec::new();
    let scripts: Vec<(&str, String)> = vec![
        ("install", install_script()),
        ("uninstall", uninstall_script_v15()),
        ("service", module_service_script()),
        ("post-fs-data", module_post_fs_data_script()),
        ("customize", customize_script()),
        ("post_fs_data_legacy", post_fs_data_script()),
        ("service_legacy", service_script(DEFAULT_STATE_DIR)),
    ];

    for (name, body) in &scripts {
        // 1) panic=abort
        if body.contains("panic=abort") || body.contains("panic = \"abort\"") {
            bad.push(format!("{}: menciona panic=abort", name));
        }
        // 2) nunca tocar el Magisk de la máquina
        for pat in [
            "magisk --remove-modules",
            "rm -rf /data/adb",
            "rm -rf /data/adb/magisk",
            "magisk --remove",
        ] {
            if body.contains(pat) {
                bad.push(format!("{}: toca el Magisk de la máquina ({})", name, pat));
            }
        }
        // 3) chmod 755 SIEMPRE después del mv, y sólo para los mv de
        // BINARIOS/SCRIPTS: un .log que rota no necesita el bit +x.
        let mut rest = body.as_str();
        while let Some(m) = rest.find("mv -f") {
            let linea = rest[m..].lines().next().unwrap_or("");
            let es_binario = linea.contains("megadaemon-core")
                || linea.contains("99-megadaemon.sh")
                || linea.contains("$dst");
            if es_binario {
                let after = rest[m..].find("chmod 755");
                if after.is_none() {
                    bad.push(format!(
                        "{}: mv de binario sin chmod 755 DESPUÉS: {}",
                        name,
                        linea.trim()
                    ));
                }
            }
            rest = &rest[m + 5..];
        }
        // 4) el instalador tiene que comprobar el bit +x después de mover
        if *name == "install" && !body.contains("[ -x \"$dst\" ]") {
            bad.push("install: no comprueba que el destino sea ejecutable".to_string());
        }
        // 5) el desinstalador tiene que usar el manifiesto
        if *name == "uninstall" && !body.contains("installed.manifest") {
            bad.push("uninstall: no usa el manifiesto".to_string());
        }
        // 6) el servicio del módulo no puede ser el único lanzador
        if *name == "service" && !body.contains("service.d/99-megadaemon.sh") {
            bad.push("service: no comprueba el supervisor de service.d".to_string());
        }
    }
    bad
}

#[cfg(test)]
mod v15_tests {
    use super::*;

    #[test]
    fn los_scripts_no_violan_las_reglas() {
        let bad = lint_scripts();
        assert!(bad.is_empty(), "violaciones: {:?}", bad);
    }

    #[test]
    fn el_chmod_755_viene_despues_del_mv() {
        let s = install_script();
        let mv = s.find("mv -f").expect("debe haber un mv");
        // el chmod que importa es el que va DESPUÉS del mv (offset absoluto)
        let chmod = mv
            + s[mv..]
                .find("chmod 755")
                .expect("debe haber un chmod 755 despues del mv");
        assert!(chmod > mv, "el chmod 755 debe ir después del mv");
        // y el ejecutable se comprueba al final
        assert!(s.contains("[ -x \"$dst\" ]"));
    }

    #[test]
    fn el_instalador_hace_rollback_si_falla() {
        let s = install_script();
        assert!(s.contains("megadaemon-core.prev"));
        assert!(s.contains("se restauró el anterior"));
    }

    #[test]
    fn el_supervisor_se_valida_antes_de_sustituir_al_que_funciona() {
        let s = install_script();
        let check = s.find("sh -n").expect("debe validar la sintaxis");
        let install = s
            .find("install_exec \"$SRC_DIR/99-megadaemon.sh\"")
            .unwrap();
        assert!(check < install, "se valida antes de instalar");
    }

    #[test]
    fn el_desinstalador_restaura_el_daemon_en_vez_de_borrarlo() {
        let s = uninstall_script_v15();
        assert!(s.contains("megadaemon-core.prev"));
        assert!(s.contains("daemon restaurado"));
        // y el chmod va DESPUÉS del mv del daemon
        let mv = s
            .find("mv -f \"$PREFIX/megadaemon-core.prev\"")
            .expect("mv del .prev");
        let chmod = mv
            + s[mv..]
                .find("chmod 755 \"$PREFIX/megadaemon-core\"")
                .expect("chmod del daemon");
        assert!(chmod > mv, "el chmod 755 debe ir después del mv");
    }

    #[test]
    fn el_desinstalador_no_toca_magisk_ni_fuera_del_ambito() {
        let s = uninstall_script_v15();
        assert!(!s.contains("magisk --remove"));
        // lo MENCIONA en un comentario para decir que no lo toca, pero no
        // ejecuta NADA contra el Magisk de la máquina: se comprueba línea a
        // línea, ignorando los comentarios.
        for linea in s.lines() {
            let l = linea.trim();
            if l.starts_with('#') {
                continue;
            }
            assert!(!l.contains("magisk --remove"), "ejecuta Magisk: {}", l);
            assert!(!l.contains("/data/adb/magisk"), "toca Magisk: {}", l);
        }
        assert!(s.contains("NO se borra (fuera del ámbito)"));
        assert!(s.contains("installed.manifest"));
    }

    #[test]
    fn el_service_del_modulo_no_es_el_lanzador_unico() {
        let s = module_service_script();
        assert!(s.contains("service.d/99-megadaemon.sh"));
        assert!(s.contains("sh -n"));
    }

    #[test]
    fn el_post_fs_data_no_arranca_nada() {
        let s = module_post_fs_data_script();
        assert!(!s.contains("megadaemon-core"));
        assert!(!s.contains("service.d"));
    }

    #[test]
    fn el_module_prop_tiene_los_campos_que_magisk_exige() {
        let p = module_prop();
        for k in [
            "id=",
            "name=",
            "version=",
            "versionCode=",
            "author=",
            "description=",
        ] {
            assert!(p.contains(k), "falta {}", k);
        }
        assert!(p.contains("versionCode=1500"));
        assert_eq!(p.lines().count(), 6);
    }

    #[test]
    fn el_manifiesto_de_instalacion_esta_declarado() {
        assert!(INSTALL_MANIFEST.starts_with("/data/adb/modules/megadaemon_energy/"));
        assert_eq!(SUPERVISOR_PATH, "/data/adb/service.d/99-megadaemon.sh");
    }
}
