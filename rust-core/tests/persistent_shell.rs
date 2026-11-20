//! F3 — arnés de integración del shell root **persistente**.
//!
//! El requisito de la fase: el bucle agéntico NO puede abrir un `su` por
//! llamada (el móvil tiene load 17,65 y 876 procesos); necesita **un solo
//! shell vivo** al que enviar comandos por un socket unix.

use megadaemon_core::agent_loop::{Action, ToolExecutor};
use megadaemon_core::persistent_shell::{
    PersistentShell, ShellRequest, ShellToolExecutor,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn sock(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("megadaemon-f3-shell-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_file(&p);
    p
}

#[test]
fn el_shell_persiste_entre_llamadas() {
    let path = sock("persiste");
    let sh = PersistentShell::start(&path, "sh -s").expect("arranca el shell");
    let r1 = sh
        .run_inline(&ShellRequest::new("export MD_TOKEN=7"))
        .expect("cmd 1");
    assert_eq!(r1.code, 0);
    let r2 = sh
        .run_inline(&ShellRequest::new("echo t=$MD_TOKEN"))
        .expect("cmd 2");
    assert!(r2.stdout.contains("t=7"), "el shell debe ser el mismo: {:?}", r2.stdout);
    sh.stop();
}

#[test]
fn el_ejecutor_del_bucle_no_crea_procesos() {
    let path = sock("ejecutor");
    let sh = Arc::new(Mutex::new(
        PersistentShell::start(&path, "sh -s").expect("arranca el shell"),
    ));
    let mut ex = ShellToolExecutor::new(sh.clone());
    assert_eq!(ex.spawns_per_call(), 0, "0 procesos por llamada");
    assert!(ex.uses_persistent_shell());

    let action = Action::Tool {
        name: "shell".to_string(),
        args: "{\"cmd\":\"echo hola-bucle\"}".to_string(),
    };
    let out = ex.call(&action, 1).expect("ejecuta");
    assert!(out.contains("hola-bucle"), "out: {}", out);

    // El comando sale del `args.cmd`, no del nombre de la tool.
    assert_eq!(
        ShellToolExecutor::command_of(&action),
        "echo hola-bucle".to_string()
    );
    let guard = sh.lock().unwrap_or_else(|e| e.into_inner());
    guard.stop();
}

#[test]
fn el_timeout_no_deja_procesos_colgados() {
    let path = sock("timeout");
    let sh = PersistentShell::start(&path, "sh -s").expect("arranca el shell");
    let mut req = ShellRequest::new("sleep 30");
    req.timeout_ms = 500;
    let err = sh.run_inline(&req).expect_err("debe expirar");
    assert!(err.to_string().contains("timeout"), "err: {}", err);
    assert!(sh.is_broken());
    sh.rearm().expect("rearme");
    let r = sh.run_inline(&ShellRequest::new("echo vivo")).expect("cmd");
    assert!(r.stdout.contains("vivo"));
    sh.stop();
}
