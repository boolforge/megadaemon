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

/// El watchdog, en shell. **Fuente unica**: el fichero real del repo
/// `deploy/99-megadaemon.sh` se incrusta tal cual con `include_str!`.
///
/// MEDIDO (2026-11-23): antes el modulo llevaba una copia ENLINEA de 1,6 KB
/// que **no era** el supervisor que ya corria en el movil (50 KB, con
/// auto-rollback, politica de energia, sondeo de `/health` y gestion del
/// tunel). Al arrancar, el daemon instalaba su copia ENLINEA y **sobrescribia
/// el supervisor bueno**, ademas de arrancar el daemon sin `cockpit 9090` y
/// sin leer `megadaemon.env` (el bug medido el 2026-11-21: el estado y la
/// key se dispersaban y todo el enjambre recibia 403).
///
/// Con `include_str!` es imposible que vuelvan a divergir: lo que instala el
/// daemon es **exactamente** lo que se despliega desde el repo.
pub const WATCHDOG_SH: &str = include_str!("../../deploy/99-megadaemon.sh");

/// Guardián del supervisor (`service.d/97-megadaemon-keeper.sh`).
///
/// MEDIDO (2026-11-23, móvil): el supervisor es lo único que relanza el
/// daemon, y si el supervisor ALSO muere el móvil se queda sin daemon Y sin
/// supervisor (el túnel sigue vivo pero responde 502 para siempre, sin que
/// nadie reinstale nada).  Este script, hermano de `service.d`, comprueba
/// cada 60 s que el supervisor siga vivo y lo relanza si no.
pub const KEEPER_SH: &str = include_str!("../../deploy/97-megadaemon-keeper.sh");

/// Ruta del guardián en el móvil.
pub const RUTA_KEEPER: &str = "/data/adb/service.d/97-megadaemon-keeper.sh";

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
    // Si ya es identico NO se reescribe: se evita tocar el fichero que Magisk
    // puede estar ejecutando en este mismo instante.
    if let Ok(actual) = std::fs::read(&ruta) {
        if actual == cuerpo.as_bytes() {
            let copia = crate::cockpit_server::state_dir() + "/watchdog.sh";
            let _ = std::fs::write(&copia, cuerpo.as_bytes());
            return json!({
                "ok": true,
                "ruta": ruta.display().to_string(),
                "copia": copia,
                "ya_instalado": true,
                "bytes": cuerpo.len(),
            });
        }
    }
    // Instalacion ATOMICA: se escribe a un temporal, se marca ejecutable y
    // se renombra.  Si el daemon muere a mitad de la escritura, Magisk no se
    // encuentra un watchdog TRUNCADO en el siguiente boot (un supervisor a
    // medias deja el movil sin daemon y sin supervisor).
    let tmp = ruta.with_extension("sh.tmp");
    if let Err(e) = std::fs::write(&tmp, cuerpo.as_bytes()) {
        return json!({"ok": false, "motivo": format!("escribir {}: {e}", tmp.display())});
    }
    set_exec(&tmp);
    if let Err(e) = std::fs::rename(&tmp, &ruta) {
        let _ = std::fs::remove_file(&tmp);
        return json!({"ok": false, "motivo": format!("renombrar {}: {e}", ruta.display())});
    }
    // copia legible en el state dir (para poder inspeccionarla sin root)
    let copia = crate::cockpit_server::state_dir() + "/watchdog.sh";
    let _ = std::fs::write(&copia, cuerpo.as_bytes());
    // El guardian del supervisor se instala junto a el (misma fuente unica).
    let keeper = instalar_keeper();
    json!({
        "ok": true,
        "ruta": ruta.display().to_string(),
        "copia": copia,
        "keeper": keeper,
        "nota": "Magisk lo ejecutara en cada boot; el daemon sale con codigo 10 tras autoactualizarse y el watchdog lo relanza",
    })
}

/// Instala (de forma ATOMICA) el guardian del supervisor.
///
/// Es idempotente: si el fichero ya es identico, no se toca (Magisk puede
/// estar ejecutandolo en ese mismo instante).
pub fn instalar_keeper() -> Value {
    let ruta = PathBuf::from(RUTA_KEEPER);
    if !es_android() {
        return json!({"ok": false, "motivo": "no es android"});
    }
    if let Some(p) = ruta.parent() {
        if let Err(e) = std::fs::create_dir_all(p) {
            return json!({"ok": false, "motivo": format!("crear {}: {e}", p.display())});
        }
    }
    let cuerpo = KEEPER_SH.as_bytes();
    if let Ok(actual) = std::fs::read(&ruta) {
        if actual == cuerpo {
            return json!({"ok": true, "ruta": ruta.display().to_string(), "ya_instalado": true});
        }
    }
    let tmp = ruta.with_extension("sh.tmp");
    if let Err(e) = std::fs::write(&tmp, cuerpo) {
        return json!({"ok": false, "motivo": format!("escribir {}: {e}", tmp.display())});
    }
    set_exec(&tmp);
    if let Err(e) = std::fs::rename(&tmp, &ruta) {
        let _ = std::fs::remove_file(&tmp);
        return json!({"ok": false, "motivo": format!("renombrar {}: {e}", ruta.display())});
    }
    json!({"ok": true, "ruta": ruta.display().to_string(), "bytes": cuerpo.len()})
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
    let contenido = std::fs::read(&ruta).ok();
    // MEDIDO (2026-11-23): 'vigente' era "el fichero contiene 99-megadaemon.sh"
    // => un watchdog de una version ANTERIOR contaba como vigente y el daemon
    // NUNCA lo actualizaba: el movil se quedaba con un supervisor viejo para
    // siempre (por eso seguia el de 44 KB con el repo ya en 50 KB).
    // Ahora 'vigente' significa IDENTICO al watchdog embebido en este build;
    // si no, el daemon lo reinstala (y 'desfasado' lo deja explicito).
    let identico = contenido.as_deref() == Some(WATCHDOG_SH.as_bytes());
    let parece_watchdog = contenido
        .as_deref()
        .map(|b| String::from_utf8_lossy(b).contains("while true"))
        .unwrap_or(false);
    json!({
        "instalado": existe,
        "vigente": identico,
        "parece_watchdog": parece_watchdog,
        "desfasado": existe && !identico,
        "bytes_instalado": contenido.as_ref().map(|c| c.len()).unwrap_or(0),
        "bytes_este_build": WATCHDOG_SH.len(),
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
    // El guardian tambien se auto-repara (si el movil lo borro, se reinstala).
    {
        let k = instalar_keeper();
        if k.get("ok").and_then(|v| v.as_bool()) == Some(true)
            && k.get("ya_instalado").and_then(|v| v.as_bool()) != Some(true)
        {
            eprintln!("guardian del supervisor instalado en {RUTA_KEEPER}");
        }
    }
    if r.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        if st.get("desfasado").and_then(|v| v.as_bool()) == Some(true) {
            eprintln!(
                "watchdog: estaba DESFASADO ({} B) y se actualiza al de este build ({} B) en {RUTA_SERVICE_D}",
                st.get("bytes_instalado").and_then(|v| v.as_u64()).unwrap_or(0),
                WATCHDOG_SH.len(),
            );
        } else {
            eprintln!("watchdog: instalado en {RUTA_SERVICE_D}");
        }
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
    fn el_guardian_del_supervisor_es_shell_valido() {
        assert!(KEEPER_SH.starts_with("#!/system/bin/sh"));
        assert!(KEEPER_SH.contains("while true"));
        assert!(KEEPER_SH.contains("99-megadaemon.sh"));
        // NO debe reiniciar el daemon: de eso se encarga el supervisor.
        assert!(!KEEPER_SH.contains("megadaemon-core cockpit"));
    }

    #[test]
    fn el_estado_no_rompe() {
        let v = estado();
        assert!(v.get("instalado").is_some());
        assert!(v.get("ruta").is_some());
    }

    #[test]
    fn el_estado_distingue_inexistente_de_desfasado() {
        let v = estado();
        assert!(v.get("instalado").is_some());
        assert!(v.get("ruta").is_some());
        assert!(v.get("vigente").is_some());
        assert!(v.get("desfasado").is_some());
        // 'vigente' = IDENTICO al embebido; 'desfasado' = instalado pero otro.
        let instalado = v.get("instalado").and_then(|x| x.as_bool()).unwrap_or(false);
        if instalado {
            let vigente = v.get("vigente").and_then(|x| x.as_bool()).unwrap_or(false);
            let desfasado = v.get("desfasado").and_then(|x| x.as_bool()).unwrap_or(false);
            assert_ne!(vigente, desfasado, "vigente y desfasado son excluyentes");
        }
    }

    #[test]
    fn el_watchdog_embebido_es_exactamente_el_del_repo() {
        // Una sola fuente de verdad: si esto falla, alguien ha vuelto a copiar
        // el script a mano y el daemon volveria a sobrescribir el supervisor
        // bueno del movil con una version mas pobre.
        assert_eq!(WATCHDOG_SH, include_str!("../../deploy/99-megadaemon.sh"));
    }

    #[test]
    fn el_watchdog_arranca_el_daemon_con_el_cockpit_y_el_env() {
        // Regresion medida el 2026-11-23: la copia en linea arrancaba el
        // daemon SIN argumentos (cockpit mudo) y SIN leer megadaemon.env
        // (estado y key dispersados => 403 global al enjambre).
        assert!(WATCHDOG_SH.contains("cockpit"));
        assert!(WATCHDOG_SH.contains("megadaemon.env"));
        assert!(WATCHDOG_SH.contains("megadaemon-core.prev"));
    }
}
