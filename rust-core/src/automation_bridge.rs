//! MegaDaemon v16 — PUENTE CON LAS AUTOMATIZACIONES YA INSTALADAS.
//!
//! En el móvil hay tres automatizadores instalados (medido el 2026-11-22):
//! `vadimerenkov.autasker` (AutoTasker 1.2.0), `com.termux.boot` (Termux:Boot)
//! y `com.termux` (Termux). Este módulo:
//!
//! 1. **detecta** cuáles están y qué superficie de disparo tienen de verdad;
//! 2. **construye** el comando `am`/`am broadcast` para lanzarles algo;
//! 3. **acepta** eventos desde ellos por una bandeja de entrada (`inbox/`);
//! 4. **documenta** por qué, con lo instalado hoy, el puente es marginal
//!    (AutoTasker sólo escucha `BOOT_COMPLETED` y no tiene tareas; Termux:Boot
//!    no tiene scripts de arranque) y por qué el motor de reglas propio de
//!    MegaDaemon es la pieza que sustituye a todo eso.
//!
//! Todo lo que este módulo afirma sobre las apps instaladas está **medido**
//! con `dumpsys package` en este dispositivo; lo que no está instalado se
//! marca como `measured: false` y hay que verificarlo al instalarlo.

use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Una acción de disparo que una app expone.
#[derive(Clone, Debug, PartialEq)]
pub struct BridgeAction {
    /// Nombre corto (`run_command`, `boot`, `task`…).
    pub name: String,
    /// Acción del broadcast (`com.termux.RUN_COMMAND`).
    pub broadcast_action: String,
    /// Componente receptor (`com.termux/.app.RunCommandService`).
    pub component: Option<String>,
    /// Extras por defecto.
    pub extras: Vec<(String, String)>,
    /// Permiso que exige el receptor (si lo exige).
    pub permission: Option<String>,
    /// true si la acción está **medida** en este dispositivo.
    pub measured: bool,
    pub notes: String,
}

/// Un automatizador (app) que MegaDaemon puede lanzar.
#[derive(Clone, Debug, PartialEq)]
pub struct BridgeTarget {
    /// Nombre de la app.
    pub app: String,
    pub package: String,
    pub version: Option<String>,
    /// Está instalada en el dispositivo.
    pub installed: bool,
    /// Tiene automatizaciones configuradas (tareas / scripts de arranque).
    pub configured: Option<bool>,
    pub actions: Vec<BridgeAction>,
    pub notes: String,
}

impl BridgeTarget {
    /// Busca una acción por nombre.
    pub fn action(&self, name: &str) -> Option<&BridgeAction> {
        self.actions.iter().find(|a| a.name == name)
    }
}

/// Apps de automatización que MegaDaemon conoce.
///
/// Los datos de las apps instaladas en ESTE dispositivo son medidos; las que
/// no están instaladas se listan con `measured: false` y sus acciones
/// públicas de fabricante, que hay que verificar al instalarlas.
pub fn known_apps() -> Vec<BridgeTarget> {
    vec![
        BridgeTarget {
            app: "Termux".into(),
            package: "com.termux".into(),
            version: None,
            installed: false,
            configured: None,
            actions: vec![BridgeAction {
                name: "run_command".into(),
                broadcast_action: "com.termux.RUN_COMMAND".into(),
                component: Some("com.termux/.app.RunCommandService".into()),
                extras: vec![
                    ("com.termux.RUN_COMMAND_PATH".into(), "/data/local/tmp/md_bridge.sh".into()),
                    ("com.termux.RUN_COMMAND_ARGUMENTS".into(), "-f".into()),
                    ("com.termux.RUN_COMMAND_BACKGROUND".into(), "true".into()),
                    ("com.termux.RUN_COMMAND_WAIT".into(), "false".into()),
                ],
                permission: Some("com.termux.permission.RUN_COMMAND".into()),
                measured: true,
                notes: "MEDIDO en el S21: `am startservice -a com.termux.RUN_COMMAND` \
                        (RunCommandService) con permiso com.termux.permission.RUN_COMMAND."
                    .into(),
            }],
            notes: "Termux 0.118 instalado (3,5 GB de datos). Es la superficie de \
                    ejecución de scripts más potentada del móvil."
                .into(),
        },
        BridgeTarget {
            app: "Termux:Boot".into(),
            package: "com.termux.boot".into(),
            version: None,
            installed: false,
            configured: None,
            actions: vec![BridgeAction {
                name: "boot".into(),
                broadcast_action: "android.intent.action.BOOT_COMPLETED".into(),
                component: Some("com.termux.boot/.BootReceiver".into()),
                extras: vec![],
                permission: None,
                measured: true,
                notes: "MEDIDO: `com.termux.boot/.BootReceiver` escucha BOOT_COMPLETED y \
                        ejecuta los scripts de ~/.termux/boot/. MEDIDO ALSO: ese \
                        directorio NO existe (no hay un solo script)."
                    .into(),
            }],
            notes: "Termux:Boot instalado pero INACTIVO: sin scripts de arranque.".into(),
        },
        BridgeTarget {
            app: "AutoTasker".into(),
            package: "vadimerenkov.autasker".into(),
            version: None,
            installed: false,
            configured: None,
            actions: vec![BridgeAction {
                name: "boot".into(),
                broadcast_action: "android.intent.action.BOOT_COMPLETED".into(),
                component: Some("vadimerenkov.autasker/.core.presentation.reminders.BootReceiver".into()),
                extras: vec![],
                permission: None,
                measured: true,
                notes: "MEDIDO: AutoTasker sólo declara un receptor, y es para los \
                        recordatorios al arrancar. NO expone ninguna acción para \
                        lanzar tareas desde fuera: un puente de intents hacia él \
                        no puede existir sin tocar su código."
                    .into(),
            }],
            notes: "AutoTasker 1.2.0 instalado, pero su directorio de datos ocupa 11 KB \
                    y NO tiene `shared_prefs` ni `databases`: no hay ni una tarea \
                    configurada. Instalado y muerto."
                .into(),
        },
        BridgeTarget {
            app: "Tasker".into(),
            package: "net.dinglisch.android.tasker".into(),
            version: None,
            installed: false,
            configured: None,
            actions: vec![BridgeAction {
                name: "task".into(),
                broadcast_action: "net.dinglisch.android.tasker.ACTION_TASK".into(),
                component: None,
                extras: vec![(
                    "net.dinglisch.android.tasker.extra.TASK_NAME".into(),
                    "N".into(),
                )],
                permission: None,
                measured: false,
                notes: "NO INSTALADO en este dispositivo. Acción pública de \
                        fabricante: exige que el usuario tenga una tarea con ese \
                        nombre y que la app esté en primer plano o con \
                        «ejecutar en segundo plano» activado."
                    .into(),
            }],
            notes: "No instalado: no hay nada que lanzar.".into(),
        },
        BridgeTarget {
            app: "MacroDroid".into(),
            package: "com.arlosoft.macrodroid".into(),
            version: None,
            installed: false,
            configured: None,
            actions: vec![],
            notes: "NO INSTALADO en este dispositivo. No se declara ninguna acción \
                    de disparo porque no está medido en este móvil: si se instala, \
                    hay que releer `dumpsys package com.arlosoft.macrodroid` y \
                    añadir las acciones que declare realmente."
                .into(),
        },
        BridgeTarget {
            app: "KAutomator".into(),
            package: "com.olynomial.kautomator".into(),
            version: None,
            installed: false,
            configured: None,
            actions: vec![],
            notes: "NO INSTALADO en este dispositivo. Sin superficie medida.".into(),
        },
    ]
}

/// Cruza la lista de apps conocidas con lo que hay instalado.
///
/// `installed` = mapa `paquete -> versión`.
pub fn detect(installed: &BTreeMap<String, String>) -> Vec<BridgeTarget> {
    known_apps()
        .into_iter()
        .map(|mut t| {
            if let Some(v) = installed.get(&t.package) {
                t.installed = true;
                t.version = Some(v.clone());
            }
            t
        })
        .collect()
}

/// Compara un comando de shell con una lista de ejecutables.
#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
}

impl Command {
    /// Texto listo para `sh -c`.
    pub fn to_shell(&self) -> String {
        let mut out = self.program.clone();
        for a in &self.args {
            out.push(' ');
            out.push_str(&quote(a));
        }
        out
    }
}

/// Comilla un argumento para `sh`.
///
/// REUTILIZA `crate::android::sh_quote` (el unico cotizador del repo, con su
/// escape de comillas ya verificado por los tests de `android.rs`).
pub fn quote(s: &str) -> String {
    crate::android::sh_quote(s)
}

/// Caracteres que rompen un comando si se cuelan sin cotizar.
fn tiene_metacaracteres(s: &str) -> bool {
    s.chars()
        .any(|c| matches!(c, ';' | '|' | '&' | '`' | '$' | '>' | '<' | '\n' | '\r' | '(' | ')' | '{' | '}' | '*' | '?' | '!' | '#' | '\\'))
}

/// Construye el comando para lanzar una acción de un automatizador.
///
/// Devuelve error si la acción no existe, si la app no está instalada o si
/// algún dato viene con metacaracteres (inyección de shell).
pub fn command_for(
    target: &BridgeTarget,
    action_name: &str,
    extras: &[(String, String)],
) -> Result<String, String> {
    if !target.installed {
        return Err(format!("{} no está instalada", target.app));
    }
    let action = target
        .action(action_name)
        .ok_or_else(|| format!("acción desconocida: {}", action_name))?;
    for (k, v) in extras {
        if tiene_metacaracteres(k) || tiene_metacaracteres(v) {
            return Err(format!("valor no permitido en '{}'", k));
        }
    }
    let mut args: Vec<String> = Vec::new();
    let es_servicio = action
        .component
        .as_deref()
        .map(|c| c.contains("Service"))
        .unwrap_or(false);
    if es_servicio {
        args.push("startservice".into());
    } else {
        args.push("broadcast".into());
    }
    args.push("-a".into());
    args.push(action.broadcast_action.clone());
    if let Some(c) = &action.component {
        args.push("-n".into());
        args.push(c.clone());
    }
    for (k, v) in action.extras.iter().chain(extras.iter()) {
        args.push("--es".into());
        args.push(k.clone());
        args.push(v.clone());
    }
    let cmd = Command {
        program: "am".into(),
        args,
    };
    Ok(cmd.to_shell())
}

/// Valida un comando de shell: sólo se admite un programa conocido y sin
/// metacaracteres fuera de las comillas.
pub fn parse_command(s: &str) -> Result<Command, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("comando vacío".into());
    }
    let mut parts: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_single = false;
    let mut in_double = false;
    for ch in s.chars() {
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
                continue;
            }
            '"' if !in_single => {
                in_double = !in_double;
                continue;
            }
            ' ' | '\t' if !in_single && !in_double => {
                if !cur.is_empty() {
                    parts.push(std::mem::take(&mut cur));
                }
                continue;
            }
            _ => cur.push(ch),
        }
    }
    if in_single || in_double {
        return Err("comillas sin cerrar".into());
    }
    if !cur.is_empty() {
        parts.push(cur);
    }
    if parts.is_empty() {
        return Err("comando vacío".into());
    }
    let program = parts[0].clone();
    if !["am", "cmd", "pm", "settings", "getprop"].contains(&program.as_str()) {
        return Err(format!("programa no permitido: {}", program));
    }
    for p in &parts[1..] {
        if tiene_metacaracteres(p) {
            return Err(format!("carácter no permitido en el argumento: {}", p));
        }
    }
    Ok(Command {
        program,
        args: parts[1..].to_vec(),
    })
}

// ========================================================================= //
// Bandeja de entrada (eventos que llegan de otras apps)
// ========================================================================= //

/// Escribe un evento en la bandeja de entrada (`<dir>/inbox/<ts>-<n>.json`).
pub fn inbox_write(dir: &str, event: &Value) -> Result<String, String> {
    let inbox = format!("{}/inbox", dir);
    std::fs::create_dir_all(&inbox).map_err(|e| e.to_string())?;
    let ts = event
        .get("ts")
        .and_then(|x| x.as_i64())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)
        });
    let path = format!("{}/{}-{:06}.json", inbox, ts, event_counter());
    std::fs::write(
        &path,
        serde_json::to_string_pretty(event).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(path)
}

/// Contador monotónico para no pisar dos eventos del mismo segundo.
fn event_counter() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    N.fetch_add(1, Ordering::SeqCst) % 1_000_000
}

/// Lee los eventos pendientes de la bandeja (y los marca como leídos).
pub fn inbox_read(dir: &str) -> Vec<Value> {
    let inbox = format!("{}/inbox", dir);
    let Ok(rd) = std::fs::read_dir(&inbox) else {
        return Vec::new();
    };
    let mut files: Vec<std::path::PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    files.sort();
    let mut out = Vec::new();
    for f in files {
        if f.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(txt) = std::fs::read_to_string(&f) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<Value>(&txt) else {
            continue;
        };
        let _ = std::fs::remove_file(&f);
        out.push(v);
    }
    out
}

// ========================================================================= //
// Informe
// ========================================================================= //

/// Informe del puente: qué hay instalado, qué superficie tiene y qué falta.
pub fn report(installed: &BTreeMap<String, String>) -> Value {
    let targets = detect(installed);
    let installed_list: Vec<Value> = targets
        .iter()
        .filter(|t| t.installed)
        .map(|t| {
            json!({
                "app": t.app,
                "package": t.package,
                "version": t.version,
                "configured": t.configured,
                "actions": t.actions.iter().map(|a| json!({
                    "name": a.name,
                    "action": a.broadcast_action,
                    "component": a.component,
                    "measured": a.measured,
                    "notes": a.notes,
                })).collect::<Vec<_>>(),
                "notes": t.notes,
            })
        })
        .collect();
    let missing: Vec<Value> = targets
        .iter()
        .filter(|t| !t.installed)
        .map(|t| json!({"app": t.app, "package": t.package, "notes": t.notes}))
        .collect();
    json!({
        "ok": true,
        "installed": installed_list,
        "not_installed": missing,
        "why_a_bridge_is_marginal": [
            "AutoTasker está instalada pero no declara ninguna acción para lanzar \
             tareas desde fuera: sólo escucha BOOT_COMPLETED (medido con dumpsys package).",
            "AutoTasker no tiene ni `databases` ni `shared_prefs`: no hay tareas configuradas.",
            "Termux:Boot está instalada pero ~/.termux/boot no existe: no arranca nada.",
            "Tasker, MacroDroid y KAutomator no están instaladas.",
            "Por eso el motor de reglas de MegaDaemon (`crate::automation`) es el \
             que sustituye a los automatizadores de terceros: eventos del sistema \
             -> acciones, en Rust, sin depender de ninguna app.",
        ],
    })
}

// ========================================================================= //
// Tests
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;

    fn instalado() -> BTreeMap<String, String> {
        let mut m = BTreeMap::new();
        m.insert("com.termux".to_string(), "0.118.0".to_string());
        m.insert("com.termux.boot".to_string(), "0.7.1".to_string());
        m.insert("vadimerenkov.autasker".to_string(), "1.2.0".to_string());
        m
    }

    #[test]
    fn la_deteccion_marca_las_instaladas() {
        let t = detect(&instalado());
        let termux = t.iter().find(|x| x.package == "com.termux").unwrap();
        assert!(termux.installed);
        assert_eq!(termux.version.as_deref(), Some("0.118.0"));
        let tasker = t.iter().find(|x| x.package == "net.dinglisch.android.tasker").unwrap();
        assert!(!tasker.installed);
    }

    #[test]
    fn el_comando_de_termux_se_construye_bien() {
        let t = detect(&instalado());
        let termux = t.iter().find(|x| x.package == "com.termux").unwrap();
        let cmd = command_for(termux, "run_command", &[]).unwrap();
        // El comando se parsea con el MISMO parser que valida lo que llega de
        // fuera: si se acepta al parsear, es un comando valido.
        let parsed = parse_command(&cmd).expect("el comando debe ser valido");
        assert_eq!(parsed.program, "am");
        assert!(parsed.args.contains(&"startservice".to_string()));
        assert!(parsed.args.contains(&"com.termux.RUN_COMMAND".to_string()));
        assert!(parsed
            .args
            .contains(&"com.termux/.app.RunCommandService".to_string()));
    }

    #[test]
    fn no_se_puede_lanzar_una_app_que_no_esta_instalada() {
        let t = detect(&instalado());
        let tasker = t.iter().find(|x| x.package == "net.dinglisch.android.tasker").unwrap();
        assert!(command_for(tasker, "task", &[]).is_err());
    }

    #[test]
    fn los_metacaracteres_se_rechazan() {
        let t = detect(&instalado());
        let termux = t.iter().find(|x| x.package == "com.termux").unwrap();
        let r = command_for(termux, "run_command", &[("x".into(), "; rm -rf /".into())]);
        assert!(r.is_err());
    }

    #[test]
    fn el_parser_de_comandos_filtra_lo_peligroso() {
        assert!(parse_command("am broadcast -a android.intent.action.BOOT_COMPLETED").is_ok());
        assert!(parse_command("rm -rf /").is_err());
        assert!(parse_command("am broadcast -a x; id").is_err());
        assert!(parse_command("").is_err());
    }

    #[test]
    fn el_inbox_guarda_y_las_entradas() {
        let dir = "/tmp/md_test_bridge_inbox";
        let _ = std::fs::remove_dir_all(dir);
        let ev = json!({"ts": 1000, "kind": "boot_completed"});
        let p = inbox_write(dir, &ev).unwrap();
        assert!(std::path::Path::new(&p).exists());
        let leidos = inbox_read(dir);
        assert_eq!(leidos.len(), 1);
        assert_eq!(leidos[0]["kind"], json!("boot_completed"));
        // Leído una vez: la bandeja queda vacía.
        assert!(inbox_read(dir).is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn el_informe_explica_por_que_el_puente_es_marginal() {
        let r = report(&instalado());
        assert_eq!(r["ok"], json!(true));
        assert_eq!(r["installed"].as_array().unwrap().len(), 3);
        assert_eq!(r["not_installed"].as_array().unwrap().len(), 3);
        let txt = r["why_a_bridge_is_marginal"].to_string();
        assert!(txt.contains("AutoTasker"));
        assert!(txt.contains("Termux:Boot"));
    }

    #[test]
    fn la_cotizacion_aguanta_los_espacios() {
        // REUTILIZA `android::sh_quote`: cotiza siempre (el escape de la
        // comilla simple ya lo cubre el test de `android.rs`).
        assert_eq!(quote("hola mundo"), "'hola mundo'");
        assert_eq!(quote("simple"), "'simple'");
        assert_eq!(quote(""), "''");
    }
}
