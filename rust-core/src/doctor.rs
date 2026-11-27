//! Tool MCP `doctor`: el diagnóstico y la recuperación del móvil en UN comando.
//!
//! POR QUÉ ESTE MÓDULO
//! --------------------
//! El binario `mddoctor` (C, 48 KB, sin dependencias) es el que sabe
//! diagnosticar el móvil **cuando el daemon está roto**, que es justo cuando
//! hace falta: no depende de la librería de MegaDaemon, se compila donde no
//! hay RAM y se sube al móvil con un `cat`.
//!
//! Pero el binario solo sirve si alguien lo llama. Esta tool lo invoca a
//! través de la API de MegaDaemon, de modo que el root (o cualquier agente)
//! puede diagnosticar el nodo desde el mismo sitio que usa para todo lo
//! demás, sin ssh ni ADB.
//!
//! LO QUE AÑADE RESPECTO A LOS SCRIPTS QUE REIMPLEMENTA
//! ------------------------------------------------------
//! `tools/estado-movil.sh` y `tools/verificar-movil.sh` hacen lo mismo a
//! pelo, con una diferencia: el shell de Android **no tiene curl ni wget**
//! (MEDIDO), así que el chequeo de `/health` o no se hacía o dependía de
//! `toybox`.  Aquí el chequeo va dentro del binario en C, con sockets POSIX.
//!
//! Y hay una diferencia de SEGURIDAD: el binario calcula el SHA-256 del
//! binario **en servicio**, de modo que se puede comprobar que el móvil
//! está sirviendo la build que dice servir (y no una vieja o manipulada).
//!
//! CAÍDA
//! ------
//! Si el binario no está instalado, esta tool NO falla: devuelve el
//! diagnóstico nativo (`diagnostico::recoger()`), que es menos bueno pero
//! suficiente.  Un diagnóstico que no funciona cuando el sistema está roto
//! es peor que uno que degrade.

use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;

/// Prefijo de instalación del daemon (el bundle instala `mddoctor` ahí).
fn prefijo() -> PathBuf {
    PathBuf::from(
        std::env::var("MEGADAEMON_PREFIX").unwrap_or_else(|_| "/data/adb/megadaemon".to_string()),
    )
}

/// Dónde puede estar el binario.  El primero que exista gana.
fn candidatos() -> Vec<PathBuf> {
    vec![
        prefijo().join("mddoctor"),
        prefijo().join("bin").join("mddoctor"),
        PathBuf::from("/data/local/tmp/megadaemon/mddoctor"),
        PathBuf::from("./tools/mddoctor"),
    ]
}

fn buscar() -> Option<PathBuf> {
    candidatos().into_iter().find(|p| p.is_file())
}

/// Ejecuta `mddoctor` con la acción pedida.  `None` si no hay binario.
fn ejecutar(accion: &str, aplicar: bool, espera: u64) -> Option<String> {
    let bin = buscar()?;
    let mut cmd = Command::new(&bin);
    match accion {
        "recover" => {
            cmd.arg("recover");
            if aplicar {
                cmd.arg("--apply");
            }
        }
        "salud" => {
            cmd.arg("salud");
            if espera > 0 {
                cmd.arg("--espera").arg(espera.to_string());
            }
        }
        "entorno" => {
            cmd.arg("entorno");
        }
        "repair" => {
            cmd.arg("repair");
            if aplicar {
                cmd.arg("--apply");
            }
        }
        "reconectar" => {
            cmd.arg("reconectar");
            if aplicar {
                cmd.arg("--apply");
            }
            if espera > 0 {
                cmd.arg("--espera").arg(espera.to_string());
            }
        }
        "todo" => {
            cmd.arg("todo");
            if espera > 0 {
                cmd.arg("--espera").arg(espera.to_string());
            }
        }
        "scan" => {
            cmd.arg("scan");
        }
        "selftest" => {
            cmd.arg("selftest");
        }
        _ => {
            cmd.arg("doctor");
        }
    }
    let salida = cmd.output().ok()?;
    let mut txt = String::from_utf8_lossy(&salida.stdout).to_string();
    if txt.trim().is_empty() {
        txt = String::from_utf8_lossy(&salida.stderr).to_string();
    }
    Some(txt)
}

/// Dispatch de la tool MCP `doctor`.
pub fn dispatch(args: &Value) -> Value {
    // Se aceptan "accion" y "action" (como en `telemetria`): una tool
    // llamada con la clave en ingles no debe caer'se al caso por defecto.
    let accion = args
        .get("accion")
        .or_else(|| args.get("action"))
        .and_then(|v| v.as_str())
        .unwrap_or("doctor")
        .trim()
        .to_string();
    let aplicar = args
        .get("aplicar")
        .or_else(|| args.get("apply"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let espera = args.get("espera").and_then(|v| v.as_u64()).unwrap_or(0);

    // `recover --apply` BORRA ficheros viejos del directorio de estado: es
    // destructivo, así que exige confirmación explícita (misma convención
    // que `root_stealth.apply` y `battery_saver.apply`).
    // `repair --apply` también toca el sistema (mkdir, chmod, mata procesos
    // y relanza el supervisor): misma exigencia.
    if (accion == "recover" || accion == "repair" || accion == "reconectar") && aplicar {
        let confirmar = args
            .get("confirmar")
            .or_else(|| args.get("confirm"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !confirmar {
            return json!({
                "ok": false,
                "accion": accion,
                "error": "esta accion con aplicar=true toca el sistema: hace falta confirmar=true",
                "pista": "primero pide el plan con accion=recover y sin aplicar"
            });
        }
    }

    match ejecutar(&accion, aplicar, espera) {
        Some(salida) => json!({
            "ok": true,
            "accion": accion,
            "fuente": "mddoctor",
            "salida": salida
        }),
        None => json!({
            "ok": true,
            "accion": accion,
            "fuente": "nativo",
            "aviso": "mddoctor no esta instalado: se devuelve el diagnostico nativo (menos completo)",
            "diagnostico": crate::diagnostico::recoger()
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_accion_por_defecto_es_doctor() {
        let v = dispatch(&json!({}));
        assert_eq!(v.get("accion").and_then(|x| x.as_str()), Some("doctor"));
    }

    #[test]
    fn se_aceptan_accion_y_action() {
        let v = dispatch(&json!({ "accion": "salud" }));
        assert_eq!(v.get("accion").and_then(|x| x.as_str()), Some("salud"));
        let v2 = dispatch(&json!({ "action": "scan" }));
        assert_eq!(v2.get("accion").and_then(|x| x.as_str()), Some("scan"));
    }

    #[test]
    fn si_no_hay_binario_devuelve_el_diagnostico_nativo() {
        // En este entorno de test puede o no estar el binario: lo que NO
        // puede pasar es que la tool devuelva un error.
        let v = dispatch(&json!({ "accion": "doctor" }));
        assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(true));
        let fuente = v.get("fuente").and_then(|x| x.as_str()).unwrap_or("");
        assert!(fuente == "mddoctor" || fuente == "nativo", "fuente inesperada: {fuente}");
    }

    #[test]
    fn recover_aplicar_exige_confirmacion() {
        // Sin binario da igual: la comprobacion es ANTES de ejecutarlo.
        let v = dispatch(&json!({ "accion": "recover", "aplicar": true }));
        assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(false));
        assert!(v.get("error").and_then(|x| x.as_str()).is_some());
    }
}
