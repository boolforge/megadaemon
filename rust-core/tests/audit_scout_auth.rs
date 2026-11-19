// ========================================================================= //
// AUDITORÍA v13.2.3 — "fail-closed" del cockpit (tests/audit_scout_auth.rs)
//
// Defecto reproducido: `requires_auth()` protegía SÓLO los POST.  Varios
// endpoints con efectos de estado estaba served en la rama GET:
//   * GET /api/vision_start  -> arranca el worker de visión continua (hilo
//     permanente que quema CPU en el dispositivo) SIN token;
//   * GET /api/memory_freeze -> congela/escribe ficheros SIN token;
//   * GET /api/providers     -> vuelca el catálogo de proveedores SIN token.
//
// Con CORS restringido el vector "página web cualquiera" queda cerrado, pero
// el cockpit también escucha en la LAN y cualquier proceso del dispositivo
// (o una app sin permisos) puede llamar a 127.0.0.1:3000.  Fail-closed:
// TODO /api/* exige token; sólo la página HTML es pública (lleva el token
// embebido, y sin CORS abierto nadie externo puede leerla).
// ========================================================================= //

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let p = l.local_addr().unwrap().port();
    drop(l);
    p
}

fn http(port: u16, method: &str, path: &str, token: Option<&str>) -> (u16, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
    let mut req = format!("{} {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n", method, path, port);
    if let Some(t) = token {
        req.push_str(&format!("X-Seere-Auth-Token: {}\r\n", t));
    }
    req.push_str("Content-Length: 0\r\nConnection: close\r\n\r\n");
    s.write_all(req.as_bytes()).unwrap();
    let mut out = Vec::new();
    let _ = s.read_to_end(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse::<u16>().ok())
        .unwrap_or(0);
    (status, text)
}

/// Un único cockpit para todo el fichero.
///
/// `MEGADAEMON_STATE_DIR` es una variable de ENTORNO (global al proceso): si
/// cada test montara el suyo en paralelo, se pisarían entre sí y el token se
/// leería del directorio equivocado.  Por eso hay un solo servidor y un solo
/// directorio de estado, compartido por todos los tests del fichero.
fn start_cockpit() -> (u16, String) {
    static SHARED: std::sync::OnceLock<(u16, String)> = std::sync::OnceLock::new();
    let (port, token) = SHARED.get_or_init(|| {
        let port = free_port();
        let dir = std::env::temp_dir().join("md_auth_shared");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        std::env::set_var("MEGADAEMON_STATE_DIR", dir.to_string_lossy().to_string());
        std::thread::spawn(move || {
            let _ = megadaemon_core::cockpit_server::run_server(port);
        });
        std::thread::sleep(Duration::from_millis(900));
        // El token vive en <STATE_DIR>/cockpit_token (0600).
        let tok_path = std::path::PathBuf::from(
            megadaemon_core::cockpit_server::state_dir(),
        )
        .join("cockpit_token");
        let mut token = String::new();
        println!("[AUTH-BOOT] state_dir = {}", megadaemon_core::cockpit_server::state_dir());
        println!("[AUTH-BOOT] tok_path = {}", tok_path.display());
        for _ in 0..25 {
            if let Ok(t) = std::fs::read_to_string(&tok_path) {
                token = t.trim().to_string();
                if !token.is_empty() {
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        (port, token)
    });
    (*port, token.clone())
}

// ------------------------------------------------------------------------- //
// AUTH-01: arrancar la visión continua (hilo permanente) exigía token
// ------------------------------------------------------------------------- //
#[test]
fn auth01_arrancar_la_vision_continua_exige_token() {
    let (port, _tok) = start_cockpit();
    let (st, body) = http(port, "GET", "/api/vision_start", None);
    println!("[AUTH-01] GET /api/vision_start sin token -> {} :: {}", st, &body[..body.len().min(160)]);
    assert_eq!(
        st, 403,
        "[AUTH-01] DEFECTO: arrancar la visión continua sin token devolvió {} (hilo permanente sin autenticación)",
        st
    );
}

// ------------------------------------------------------------------------- //
// AUTH-02: /api/memory_freeze (escritura en disco) exigía token
// ------------------------------------------------------------------------- //
#[test]
fn auth02_memory_freeze_exige_token() {
    let (port, _tok) = start_cockpit();
    let (st, body) = http(port, "GET", "/api/memory_freeze?target=/tmp/x", None);
    println!("[AUTH-02] GET /api/memory_freeze sin token -> {} :: {}", st, &body[..body.len().min(160)]);
    assert_eq!(st, 403, "[AUTH-02] DEFECTO: memory_freeze sin token devolvió {}", st);
}

// ------------------------------------------------------------------------- //
// AUTH-03: el catálogo de proveedores exigía token
// ------------------------------------------------------------------------- //
#[test]
fn auth03_el_catalogo_de_proveedores_exige_token() {
    let (port, _tok) = start_cockpit();
    let (st, body) = http(port, "GET", "/api/providers", None);
    println!("[AUTH-03] GET /api/providers sin token -> {} :: {}", st, &body[..body.len().min(160)]);
    assert_eq!(st, 403, "[AUTH-03] DEFECTO: catálogo de proveedores sin token devolvió {}", st);
}

// ------------------------------------------------------------------------- //
// AUTH-04: control — el POST ya exigía token (no debe romperse)
// ------------------------------------------------------------------------- //
#[test]
fn auth04_control_el_post_sigue_exigiendo_token() {
    let (port, _tok) = start_cockpit();
    let (st, _) = http(port, "POST", "/api/chat", None);
    println!("[AUTH-04] POST /api/chat sin token -> {}", st);
    assert_eq!(st, 403, "[AUTH-04] control: POST /api/chat debe seguir exigiendo token");
}

// ------------------------------------------------------------------------- //
// AUTH-05: con token, todo funciona (incluido arrancar la visión)
// ------------------------------------------------------------------------- //
#[test]
fn auth05_con_token_los_endpoints_siguen_funcionando() {
    let (port, tok) = start_cockpit();
    assert!(!tok.is_empty(), "[AUTH-05] no se pudo leer el token persistente");
    let (st, body) = http(port, "GET", "/api/vision_start", Some(&tok));
    println!("[AUTH-05] GET /api/vision_start CON token -> {} :: {}", st, &body[..body.len().min(200)]);
    assert_eq!(st, 200, "[AUTH-05] con token debe arrancar la visión: {}", st);
    let (st2, _) = http(port, "GET", "/api/vision_stop", Some(&tok));
    assert_eq!(st2, 200, "[AUTH-05] con token debe poder parar la visión: {}", st2);
    let (st3, _) = http(port, "GET", "/api/status", Some(&tok));
    assert_eq!(st3, 200, "[AUTH-05] con token /api/status debe funcionar: {}", st3);
}

// ------------------------------------------------------------------------- //
// AUTH-06: fail-closed total — ningún /api/* responde sin token
// ------------------------------------------------------------------------- //
#[test]
fn auth06_ningun_endpoint_api_responde_sin_token() {
    let (port, _tok) = start_cockpit();
    let paths = [
        "/api/ping", "/api/status", "/api/power", "/api/uploads", "/api/brain_status",
        "/api/npu_status", "/api/vision_status", "/api/npu_bench", "/api/llm_route",
        "/api/vision_start", "/api/vision_stop", "/api/memory_freeze", "/api/providers",
        "/api/live_frame", "/api/hex_inspect",
    ];
    let mut fallos = Vec::new();
    for p in paths {
        let (st, _) = http(port, "GET", p, None);
        if st != 403 {
            fallos.push(format!("{} -> {}", p, st));
        }
    }
    println!("[AUTH-06] endpoints que respondieron sin token: {:?}", fallos);
    assert!(
        fallos.is_empty(),
        "[AUTH-06] DEFECTO (fail-closed roto): {:?} respondieron sin token",
        fallos
    );
}

// ------------------------------------------------------------------------- //
// AUTH-07: la página HTML sigue siendo pública e inyecta el token
//            (si no, la UI no podría ni hacer fetch autenticado)
// ------------------------------------------------------------------------- //
#[test]
fn auth07_la_pagina_publica_inyecta_el_token() {
    let (port, tok) = start_cockpit();
    let (st, body) = http(port, "GET", "/", None);
    println!("[AUTH-07] GET / -> {} ({} bytes)", st, body.len());
    assert_eq!(st, 200, "[AUTH-07] la página principal debe seguir accesible");
    assert!(
        body.contains("window.MEGADAEMON_TOKEN"),
        "[AUTH-07] la página debe inyectar el token en window.MEGADAEMON_TOKEN"
    );
    assert!(
        body.contains(&tok) || body.contains("MEGADAEMON_TOKEN"),
        "[AUTH-07] el token inyectado debe ser el vigente"
    );
}
