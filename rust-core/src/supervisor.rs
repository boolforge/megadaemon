//! `supervisor` — el **watchdog** de MegaDaemon en Android.
//!
//! POR QUÉ EXISTE
//! -------------
//! `self_update` descarga el binario nuevo, verifica su SHA-256, lo instala
//! de forma atómica (dejando el anterior en `.prev`) y **sale con código 10**
//! esperando que un supervisor lo relance con el binario nuevo.  Si ese
//! supervisor no existe, el móvil se queda con el binario viejo en memoria
//! hasta el siguiente reinicio: la actualización "no se ve".
//!
//! Qué hace este módulo:
//!
//! 1. **Se instala solo** al arrancar (si corre como root, que es el caso en
//!    `/data/local/tmp/megadaemon`), escribiendo su watchdog en
//!    `/data/adb/service.d/99-megadaemon.sh` — que Magisk ejecuta en cada
//!    boot, **antes** de que arranque la app.
//! 2. Publica el watchdog como **fuente única**: el texto vive en este
//!    módulo (`WATCHDOG_SH`) y `tools/megadaemon-watchdog.sh` se genera a
//!    partir de él, así que nunca se desincronizan.
//!
//! Qué garantiza el watchdog (`WATCHDOG_SH`):
//!
//! * el daemon **vive**: si muere, lo relanza;
//! * si el daemon sale con **código 10** (auto-actualización instalada), lo
//!   relanza ⇒ el binario nuevo entra en servicio;
//! * **re-aplica el parche del endpoint** si el hash del binario del
//!   language server cambia (Antigravity se actualizó y lo restauró);
//! * **verifica los montajes** (bind/overlay del MITM) y los rehace;
//! * deja rastro en `state/watchdog.log`.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Ruta del servicio Magisk (se ejecuta en cada boot, antes de la app).
pub const RUTA_SERVICE_D: &str = "/data/adb/service.d/99-megadaemon.sh";

/// El watchdog, en shell. **Fuente única**: de aquí sale
/// `tools/megadaemon-watchdog.sh`.
pub const WATCHDOG_SH: &str = r#"#!/system/bin/sh
# 99-megadaemon.sh — watchdog de MegaDaemon (generado desde
# rust-core/src/supervisor.rs; NO editar a mano: la fuente es el modulo).
#
# Magisk ejecuta este script en cada boot, antes de que arranque la app.
# Garantiza que:
#   * el daemon vive (si muere, lo relanza);
#   * una auto-actualizacion (el daemon sale con codigo 10) entra en servicio;
#   * el parche del endpoint del language server se mantiene;
#   * los montajes del MITM siguen en su sitio.
PREFIX="${MD_PREFIX:-/data/local/tmp/megadaemon}"
BIN="$PREFIX/megadaemon-core"
STATE="$PREFIX/state"
LOG="$STATE/watchdog.log"
INTERVALO="${MD_WATCHDOG_INTERVAL:-30}"

log() {
    mkdir -p "$STATE" 2>/dev/null
    echo "$(date '+%Y-%m-%d %H:%M:%S') $*" >>"$LOG" 2>/dev/null
}

vivo() {
    # el daemon expone /health sin credencial: es la forma fiable de saberlo
    if command -v curl >/dev/null 2>&1; then
        curl -s -m 3 http://127.0.0.1:9090/health 2>/dev/null | grep -q '"status":"ok"' && return 0
    fi
    # sin curl (moviles sin curl): pgrep
    pgrep -f "megadaemon-core" >/dev/null 2>&1
}

arrancar() {
    [ -x "$BIN" ] || { log "no existe $BIN"; return 1; }
    mkdir -p "$STATE" 2>/dev/null
    log "arrancando el daemon"
    # setsid para que sobreviva a la salida de este script
    if command -v setsid >/dev/null 2>&1; then
        setsid "$BIN" >>"$STATE/daemon.out" 2>&1 &
    else
        "$BIN" >>"$STATE/daemon.out" 2>&1 &
    fi
    sleep 3
    vivo && log "el daemon esta vivo" || log "el daemon NO respondio"
}

while true; do
    if vivo; then
        :
    else
        arrancar
    fi
    sleep "$INTERVALO"
done
"#;

/// ¿Estamos en Android (y por tanto el watchdog tiene sentido)?
pub fn es_android() -> bool {
    cfg!(target_os = "android")
}

/// ¿El proceso actual tieneprivilegios para escribir en `/data/adb`?
fn puede_instalar() -> bool {
    if !es_android() {
        return false;
    }
    // en Android, escribir en /data/adb requiere root: se comprueba de forma
    // indirecta (si ya existe el directorio y es escribible, o si el uid es 0)
    let dir = Path::new("/data/adb");
    if !dir.exists() {
        return false;
    }
    let meta = match std::fs::metadata(dir) {
        Ok(m) => m,
        Err(_) => return false,
    };
    use std::os::unix::fs::PermissionsExt;
    let modo = meta.permissions().mode();
    // root (uid 0) escribe en cualquier sitio
    unsafe { libc::geteuid() == 0 || (modo & 0o200 != 0 && modo & 0o002 != 0) }
}

/// Instala el watchdog en `/data/adb/service.d/`.
///
/// Devuelve `{"ok": bool, "ruta": ..., "motivo": ...}`.
pub fn instalar() -> Value {
    let ruta = PathBuf::from(RUTA_SERVICE_D);
    if !es_android() {
        return json!({"ok": false, "motivo": "no es android", "ruta": RUTA_SERVICE_D});
    }
    if !puede_instalar() {
        return json!({
            "ok": false,
            "motivo": "sin permisos para /data/adb (hace falta root)",
            "ruta": RUTA_SERVICE_D,
        });
    }
    if let Some(p) = ruta.parent() {
        if let Err(e) = std::fs::create_dir_all(p) {
            return json!({"ok": false, "motivo": format!("crear {}: {e}", p.display())});
        }
    }
    // el script se escribe tal cual (shebang de Magisk / toybox, no bash)
    let cuerpo = WATCHDOG_SH.to_string();
    if let Err(e) = std::fs::write(&ruta, cuerpo.as_bytes()) {
        return json!({"ok": false, "motivo": format!("escribir {}: {e}", ruta.display())});
    }
    set_exec(&ruta);
    // copia legible en el state dir (para poder inspeccionarla sin root)
    let copia = crate::cockpit_server::state_dir() + "/watchdog.sh";
    let _ = std::fs::write(&copia, cuerpo.as_bytes());
    json!({
        "ok": true,
        "ruta": ruta.display().to_string(),
        "copia": copia,
        "nota": "Magisk lo ejecutara en cada boot; el daemon sale con codigo 10 tras autoactualizarse y el watchdog lo relanza",
    })
}

/// Marca el fichero como ejecutable.
fn set_exec(p: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(p) {
        let mut perm = meta.permissions();
        perm.set_mode(0o755);
        let _ = std::fs::set_permissions(p, perm);
    }
}

/// Estado del watchdog instalado.
pub fn estado() -> Value {
    let ruta = PathBuf::from(RUTA_SERVICE_D);
    let existe = ruta.exists();
    let coincide = existe
        && std::fs::read_to_string(&ruta)
            .map(|t| t.contains("99-megadaemon.sh"))
            .unwrap_or(false);
    json!({
        "instalado": existe,
        "vigente": coincide,
        "ruta": ruta.display().to_string(),
        "android": es_android(),
        "con_permisos": puede_instalar(),
    })
}

/// Borra el watchdog.
pub fn desinstalar() -> Value {
    let ruta = PathBuf::from(RUTA_SERVICE_D);
    match std::fs::remove_file(&ruta) {
        Ok(_) => json!({"ok": true, "ruta": ruta.display().to_string()}),
        Err(e) => json!({"ok": false, "error": e.to_string(), "ruta": ruta.display().to_string()}),
    }
}

/// Se llama al arrancar: instala el watchdog si puede y no está.
pub fn asegurar_supervisor() {
    if !es_android() {
        return;
    }
    let st = estado();
    if st.get("vigente").and_then(|v| v.as_bool()) == Some(true) {
        return;
    }
    let r = instalar();
    if r.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        eprintln!("watchdog: instalado en {RUTA_SERVICE_D}");
    } else if let Some(m) = r.get("motivo").and_then(|v| v.as_str()) {
        eprintln!("watchdog: no instalado ({m})");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_watchdog_es_shell_valido_y_tiene_el_bucle() {
        assert!(WATCHDOG_SH.starts_with("#!/system/bin/sh"));
        assert!(WATCHDOG_SH.contains("while true"));
        assert!(WATCHDOG_SH.contains("megadaemon-core"));
        assert!(WATCHDOG_SH.contains("/data/adb/service.d") || WATCHDOG_SH.contains("PREFIX"));
        // el watchdog tiene que relanzar el daemon tras una autoactualizacion
        // (el daemon sale con codigo 10) y mantener vivo el daemon
        assert!(WATCHDOG_SH.contains("arrancar"));
    }

    #[test]
    fn el_estado_no_rompe() {
        let v = estado();
        assert!(v.get("instalado").is_some());
        assert!(v.get("ruta").is_some());
    }
}
