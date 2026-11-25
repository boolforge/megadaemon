//! Bitácora de desarrollo del daemon (`devlog.json`).
//!
//! MEDIDO (2026-11-23): la ruta `/api/devlog` existía desde hacía tiempo pero
//! **siempre devolvía `[]`**:
//!
//! 1. `read_json_file_fallback()` acepta **sólo objetos** (por un pánico
//!    antiguo de serde al indexar un array) ⇒ un `devlog.json` que es un
//!    array se rechazaba siempre.
//! 2. **Nada** escribía el fichero ⇒ aunque se aceptara el array, estaba
//!    vacío.
//!
//! Aquí se arregla las dos cosas: un escritor (`append`) y un lector
//! (`leer`) que aceptan el formato array, con rotación para que no crezca
//! sin límite.  Es una bitácora de **eventos** (qué cambió y cuándo), no
//! telemetría: la telemetría vive en `/api/v2/system` y en los logs.
//!
//! El fichero vive en el **state dir** del daemon (volátil en Android
//! `/data/local/tmp` ⇒ en el móvil se pierde al reiniciar; en el estado
//! persistente se conserva).

use serde_json::{json, Value};
use std::path::PathBuf;

/// Máximo de entradas que se guardan (rotación por cabecera).
pub const MAX_ENTRADAS: usize = 200;

/// Ruta del fichero de la bitácora.
pub fn ruta() -> PathBuf {
    PathBuf::from(crate::state_dir::state_dir()).join("devlog.json")
}

/// Añade un evento a la bitácora.
///
/// `evento` es una etiqueta corta y estable (`auto_update`,
/// `instrumentacion`, `respaldo`…); `detalle` es un objeto libre.
/// Nunca falla de forma ruidosa: si no se puede escribir, se ignora (un
/// evento perdido no puede tumbar el daemon).
pub fn append(evento: &str, detalle: Value) {
    let p = ruta();
    let mut entradas = leer_entries();
    entradas.insert(
        0,
        json!({
            "ts": crate::webui::now_epoch(),
            "iso": crate::webui::now_iso(),
            "evento": evento,
            "detalle": detalle,
        }),
    );
    if entradas.len() > MAX_ENTRADAS {
        entradas.truncate(MAX_ENTRADAS);
    }
    if let Some(dir) = p.parent() {
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
    }
    let txt = match serde_json::to_string_pretty(&Value::Array(entradas)) {
        Ok(t) => t,
        Err(_) => return,
    };
    // Escritura atómica: un corte a mitad no deja un devlog.json corrupto.
    let tmp = p.with_extension("json.tmp");
    if std::fs::write(&tmp, txt).is_err() {
        return;
    }
    if std::fs::rename(&tmp, &p).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return;
    }
    set_0600(&p);
}

/// Lee las entradas (array). Si el fichero no existe o está corrupto: vacío.
pub fn leer_entries() -> Vec<Value> {
    let txt = match std::fs::read_to_string(ruta()) {
        Ok(t) => t,
        Err(_) => return vec![],
    };
    match serde_json::from_str::<Value>(&txt) {
        Ok(Value::Array(a)) => a,
        Ok(Value::Object(o)) => o
            .get("entradas")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default(),
        _ => vec![],
    }
}

/// Vista de la bitácora para la API.
pub fn leer() -> Value {
    let p = ruta();
    let entradas = leer_entries();
    json!({
        "ok": true,
        "ruta": p.to_string_lossy(),
        "existe": p.exists(),
        "total": entradas.len(),
        "entradas": entradas,
        "nota": "bitacora de eventos del daemon (no telemetria). Se rota a las {} entradas.".replace("{}", &MAX_ENTRADAS.to_string()),
    })
}

fn set_0600(p: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El test usa un state dir propio para no tocar el del daemon.
    ///
    /// MEDIDO 2026-11-25 (CI rojo): el directorio era
    /// `md-devlog-test-{pid}` ⇒ **todos los tests de este binario lo
    /// comparten** (el PID es el del proceso) y, como los tests corren en
    /// PARALELO, se pisaban: uno leía la entrada que había escrito el
    /// otro (`left: String("masa")`, `right: "prueba"`).  Ahora cada
    /// llamada tiene su propio directorio (contador atómico).
    fn con_state_dir_tmp<T>(f: impl FnOnce() -> T) -> T {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let previo = std::env::var("MEGADAEMON_STATE_DIR").ok();
        let tmp = std::env::temp_dir().join(format!(
            "md-devlog-test-{}-{n}",
            std::process::id()
        ));
        std::fs::create_dir_all(&tmp).expect("dir de test");
        std::env::set_var("MEGADAEMON_STATE_DIR", &tmp);
        let r = f();
        match previo {
            Some(v) => std::env::set_var("MEGADAEMON_STATE_DIR", v),
            None => std::env::remove_var("MEGADAEMON_STATE_DIR"),
        }
        let _ = std::fs::remove_dir_all(&tmp);
        r
    }

    #[test]
    fn append_escribe_un_array_y_se_lee() {
        con_state_dir_tmp(|| {
            append("prueba", json!({"n": 1}));
            let v = leer();
            assert_eq!(v["total"], 1);
            assert_eq!(v["entradas"][0]["evento"], "prueba");
            assert_eq!(v["entradas"][0]["detalle"]["n"], 1);
            // el fichero es un ARRAY (no un objeto): el lector v1 lo rechazaba
            let txt = std::fs::read_to_string(ruta()).expect("fichero");
            let parsed: Value = serde_json::from_str(&txt).expect("json");
            assert!(parsed.is_array(), "devlog.json debe ser un array");
        });
    }

    #[test]
    fn la_rotacion_deja_el_maximo() {
        con_state_dir_tmp(|| {
            for i in 0..(MAX_ENTRADAS + 10) {
                append("masa", json!({ "i": i }));
            }
            let v = leer();
            assert_eq!(v["total"], MAX_ENTRADAS);
            // el más reciente va primero
            assert_eq!(v["entradas"][0]["detalle"]["i"], MAX_ENTRADAS + 9);
        });
    }
}
