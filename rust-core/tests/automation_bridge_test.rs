//! Arnés de pruebas del puente con los automatizadores instalados
//! (`src/automation_bridge.rs`).
//!
//! Usa la biblioteca (`use megadaemon_core::…;`): los módulos ya están
//! cableados en `src/lib.rs`.

use megadaemon_core::automation_bridge::{
    command_for, detect, inbox_read, inbox_write, known_apps, parse_command, report,
};
use std::collections::BTreeMap;

/// Estado real medido en el S21 Ultra el 2026-11-22.
fn instalado() -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert("com.termux".to_string(), "0.118.0".to_string());
    m.insert("com.termux.boot".to_string(), "0.7.1".to_string());
    m.insert("vadimerenkov.autasker".to_string(), "1.2.0".to_string());
    m
}

#[test]
fn la_deteccion_distingue_instaladas_de_ausentes() {
    let t = detect(&instalado());
    let termux = t.iter().find(|x| x.package == "com.termux").unwrap();
    assert!(termux.installed);
    assert_eq!(termux.version.as_deref(), Some("0.118.0"));
    let tasker = t
        .iter()
        .find(|x| x.package == "net.dinglisch.android.tasker")
        .unwrap();
    assert!(!tasker.installed);
}

#[test]
fn el_comando_de_termux_usa_el_servicio_medido() {
    let t = detect(&instalado());
    let termux = t.iter().find(|x| x.package == "com.termux").unwrap();
    let cmd = command_for(termux, "run_command", &[]).unwrap();
    // OJO: el comando se arma con `to_shell()`, que COTIZA cada argumento
    // (`am 'startservice' -a 'com.termux.RUN_COMMAND' ...`): no se puede
    // comparar con una cadena plana.  Se parsea con el parser del crate.
    let parsed = megadaemon_core::automation_bridge::parse_command(&cmd).unwrap();
    assert_eq!(parsed.program, "am");
    assert_eq!(parsed.args[0], "startservice");
    assert!(cmd.contains("com.termux.RUN_COMMAND"));
    assert!(cmd.contains("com.termux/.app.RunCommandService"));
}

#[test]
fn no_se_puede_lanzar_lo_que_no_esta_instalado() {
    let t = detect(&instalado());
    let tasker = t
        .iter()
        .find(|x| x.package == "net.dinglisch.android.tasker")
        .unwrap();
    assert!(command_for(tasker, "task", &[]).is_err());
}

#[test]
fn los_metacaracteres_se_rechazan() {
    let t = detect(&instalado());
    let termux = t.iter().find(|x| x.package == "com.termux").unwrap();
    let r = command_for(termux, "run_command", &[("x".into(), "; id".into())]);
    assert!(r.is_err());
}

#[test]
fn el_parser_de_comandos_es_estricto() {
    assert!(parse_command("am broadcast -a android.intent.action.BOOT_COMPLETED").is_ok());
    assert!(parse_command("am broadcast -a 'x y'").is_ok());
    assert!(parse_command("rm -rf /").is_err());
    assert!(parse_command("am broadcast -a x; id").is_err());
    assert!(parse_command("").is_err());
}

#[test]
fn la_bandeja_recibe_y_consume_eventos() {
    let dir = std::env::temp_dir().join("md_test_bridge_inbox");
    let _ = std::fs::remove_dir_all(&dir);
    let d = dir.to_str().unwrap();
    inbox_write(d, &serde_json::json!({"ts": 1000, "kind": "boot_completed"})).unwrap();
    let leidos = inbox_read(d);
    assert_eq!(leidos.len(), 1);
    assert_eq!(leidos[0]["kind"], serde_json::json!("boot_completed"));
    assert!(inbox_read(d).is_empty(), "la bandeja se consume");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn el_informe_explica_por_que_el_puente_es_marginal() {
    let r = report(&instalado());
    assert_eq!(r["ok"], serde_json::json!(true));
    assert_eq!(r["installed"].as_array().unwrap().len(), 3);
    assert_eq!(r["not_installed"].as_array().unwrap().len(), 3);
    let txt = r["why_a_bridge_is_marginal"].to_string();
    assert!(txt.contains("AutoTasker"));
    assert!(txt.contains("Termux:Boot"));
}

#[test]
fn las_apps_conocidas_declaran_su_superficie_measureada() {
    for t in known_apps() {
        for a in &t.actions {
            assert!(
                a.measured || a.notes.contains("NO INSTALADO"),
                "{} / {}: una acción sin medir debe decirlo",
                t.app,
                a.name
            );
        }
    }
}
