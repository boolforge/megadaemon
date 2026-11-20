//! MegaDaemon — AUDITORÍA ADVERSARIAL DEL "HARDWARE" (scout)
//!
//! Suite **independiente** de `audit_v13.rs` y `npu_nnapi.rs`: NO repite ninguno
//! de los 38 defectos ya documentados (auditoría v13.1 + addendum v13.2).  Busca
//! lo que se ESCAPABA.
//!
//! Ejecutar:
//!   export PATH="$HOME/.cargo/bin:$PATH"
//!   cd /workspace/megadaemon/rust-core
//!   cargo test --test audit_scout_hw -- --nocapture
//!
//! Cada test reproduce un defecto REAL y falla con el código actual.
//! Los tests que tocan estado global (SHM, providers.json, cockpit) están
//! serializados con un mutex para que el orden de ejecución no altere el
//! resultado.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

// ─────────────────────────────────────────────────────────────────────────────
// Constantes / rutas (las mismas que usa el daemon)
// ─────────────────────────────────────────────────────────────────────────────

const SHM_PATH: &str = "/data/user/0/com.agy/files/snatcher_vla_shm.bin";
const SHM_SIZE: usize = 1_048_576;
const FRAME_OFFSET: usize = 32824;
const STATE_DIR: &str = "/data/user/0/com.agy/files/megadaemon/state";
const PROVIDERS_PATH: &str = "/data/user/0/com.agy/files/megadaemon/state/providers.json";
const APP_USR_LIB: &str = "/data/user/0/com.agy/files/usr/lib";

/// Serializa los tests que comparten estado global (SHM, vault, cockpit).
fn global_lock() -> &'static Mutex<()> {
    static L: OnceLock<Mutex<()>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(()))
}

/// Adquiere el cerrojo global ignorando el envenenamiento (un test que falla
/// no debe impedir que corran los demás).
fn lock_guard() -> std::sync::MutexGuard<'static, ()> {
    global_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

// ─────────────────────────────────────────────────────────────────────────────
// Utilidades
// ─────────────────────────────────────────────────────────────────────────────

/// Escribe un frame SHM válido con el tamaño indicado.
fn write_shm(width: u16, height: u16, frame_byte: u8) {
    let mut buf = vec![0u8; SHM_SIZE];
    buf[0..4].copy_from_slice(&0x534E4154u32.to_le_bytes()); // "SNAT"
    buf[4..8].copy_from_slice(&13u32.to_le_bytes());
    buf[8..12].copy_from_slice(&7u32.to_le_bytes());
    buf[12..20].copy_from_slice(&1_700_000_000_000_000u64.to_le_bytes());
    buf[20..22].copy_from_slice(&width.to_le_bytes());
    buf[22..24].copy_from_slice(&height.to_le_bytes());
    buf[24..26].copy_from_slice(&24u16.to_le_bytes());
    buf[26..28].copy_from_slice(&1u16.to_le_bytes());
    buf[32..37].copy_from_slice(b"SCOUT");
    // El frame se rellena hasta donde quepa; si no cabe entero, al menos se
    // marca un byte no nulo (si no, `is_active` sería false y el daemon ni
    // miraría la cabecera).
    let start = FRAME_OFFSET;
    let end = std::cmp::min(
        start + width as usize * height as usize * 3,
        buf.len(),
    );
    for b in buf[start..end].iter_mut() {
        *b = frame_byte;
    }
    std::fs::write(SHM_PATH, &buf).expect("no se pudo escribir el SHM de prueba");
}

/// Copia de seguridad/restaura un fichero (para no dejar basura en el device).
struct Backup {
    path: String,
    data: Option<Vec<u8>>,
}

impl Backup {
    fn take(path: &str) -> Backup {
        let data = std::fs::read(path).ok();
        Backup {
            path: path.to_string(),
            data,
        }
    }
    fn restore(&self) {
        match &self.data {
            Some(d) => {
                let _ = std::fs::write(&self.path, d);
            }
            None => {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
}

/// Petición HTTP cruda contra el cockpit.  Devuelve (cabeceras, cuerpo).
fn http_raw(port: u16, method: &str, path: &str, body: Option<&str>) -> (String, String) {
    http_raw_auth(port, method, path, body, false)
}

/// v13.2.3: el cockpit es fail-closed (todo /api/* exige token).  Con
/// `with_token` se manda `X-Seere-Auth-Token` para poder ejercitar los
/// handlers; sin él se comprueba que el endpoint RECHAZA la llamada.
fn http_raw_auth(
    port: u16,
    method: &str,
    path: &str,
    body: Option<&str>,
    with_token: bool,
) -> (String, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).expect("no se pudo conectar al cockpit");
    s.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
    s.set_write_timeout(Some(Duration::from_secs(20))).unwrap();
    let payload = body.unwrap_or("");
    let auth = if with_token {
        format!("X-Seere-Auth-Token: {}\r\n", megadaemon_core::cockpit_server::auth_token())
    } else {
        String::new()
    };
    let req = format!(
        "{} {} HTTP/1.1\r\nHost: 127.0.0.1\r\n{}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        method,
        path,
        auth,
        payload.len(),
        payload
    );
    s.write_all(req.as_bytes()).unwrap();
    let mut raw = Vec::new();
    let _ = s.read_to_end(&mut raw);
    let text = String::from_utf8_lossy(&raw).into_owned();
    match text.find("\r\n\r\n") {
        Some(i) => (text[..i].to_string(), text[i + 4..].to_string()),
        None => (text.clone(), String::new()),
    }
}

/// Arranca el cockpit en un hilo y espera a que acepte conexiones.
fn start_cockpit(port: u16) {
    std::thread::spawn(move || {
        let _ = megadaemon_core::cockpit_server::run_server(port);
    });
    for _ in 0..200 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("el cockpit no arrancó en el puerto {}", port);
}

/// Proveedor HTTP falso: captura las peticiones completas (cabeceras + cuerpo).
struct FakeProvider {
    port: u16,
    requests: Arc<Mutex<Vec<String>>>,
}

fn start_fake_provider() -> FakeProvider {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let sink = requests.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut sock = match stream {
                Ok(s) => s,
                Err(_) => continue,
            };
            let _ = sock.set_read_timeout(Some(Duration::from_secs(10)));
            let mut raw = Vec::new();
            let mut chunk = [0u8; 8192];
            // Lee cabecera + cuerpo (hasta Content-Length).
            let mut header_end = None;
            let mut content_length = 0usize;
            loop {
                match sock.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        raw.extend_from_slice(&chunk[..n]);
                        if header_end.is_none() {
                            if let Some(p) = find(&raw, b"\r\n\r\n") {
                                header_end = Some(p + 4);
                                let head = String::from_utf8_lossy(&raw[..p]).to_lowercase();
                                for l in head.lines() {
                                    if let Some(v) = l.strip_prefix("content-length:") {
                                        content_length = v.trim().parse().unwrap_or(0);
                                    }
                                }
                            }
                        }
                        if let Some(he) = header_end {
                            if raw.len() >= he + content_length {
                                break;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
            sink.lock()
                .unwrap()
                .push(String::from_utf8_lossy(&raw).into_owned());
            let body = r#"{"choices":[{"message":{"content":"PONG"}}],"usage":{"total_tokens":7}}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = sock.write_all(resp.as_bytes());
            let _ = sock.flush();
        }
    });
    FakeProvider { port, requests }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.len() > hay.len() {
        return None;
    }
    (0..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

/// Escribe un vault de providers con un único proveedor que apunta a `port`.
fn write_vault(endpoint: &str, api_key: &str) -> Backup {
    std::fs::create_dir_all(STATE_DIR).unwrap();
    let backup = Backup::take(PROVIDERS_PATH);
    let vault = serde_json::json!({
        "default_model": "scout-p1",
        "providers": [{
            "id": "scout-p1",
            "name": "Scout P1",
            "vendor": "Scout",
            "endpoint": endpoint,
            "model": "scout-model",
            "api_key": api_key,
            "auth_type": "bearer",
            "tier": "free",
            "enabled": true
        }]
    });
    std::fs::write(PROVIDERS_PATH, serde_json::to_string_pretty(&vault).unwrap()).unwrap();
    backup
}

/// RSS del proceso actual en KiB (para medir amplificaciones de memoria).
fn rss_kb() -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    for l in s.lines() {
        if let Some(v) = l.strip_prefix("VmRSS:") {
            return v.split_whitespace().next().unwrap_or("0").parse().unwrap_or(0);
        }
    }
    0
}

/// Ejecuta un test del propio binario en un proceso hijo (estado limpio).
fn run_child(test_name: &str, extra_env: &[(&str, &str)], limit: Duration) -> Option<std::process::Output> {
    let exe = std::env::current_exe().unwrap();
    let mut cmd = Command::new(exe);
    cmd.arg("--exact")
        .arg(test_name)
        .arg("--nocapture")
        .env("SCOUT_CHILD", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().unwrap();
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => break,
        }
    }
    Some(child.wait_with_output().unwrap())
}

// ========================================================================= //
// HW-01 · cockpit_server.rs · PANIC en un handler => el servidor MUERE
// ========================================================================= //
//
// `read_json_file_fallback` devuelve el JSON TAL CUAL (puede ser un array, un
// número o un string si el fichero de estado está corrupto pero es JSON
// válido).  `/api/status` hace `st["cockpit_engine"] = ...` sobre ese valor:
// el `IndexMut<&str>` de serde_json PANIQUE si el valor no es un objeto.
//
// El perfil release usa `panic = "unwind"` (v15, a proposito en Cargo.toml):
// un panic NO mata el proceso entero, solo el hilo del handler.  Por eso este
// test comprueba las DOS cosas: que el handler responde (500/err) y que el
// daemon SIGUE VIVO despues del panic.  (El 134 que se vio en el movil en otro
// momento no venia de aqui: con unwind un panic no produce 134.)
// ENTERO y el cockpit cae.  Además, un fichero de estado corrupto es
// precisamente lo que el daemon debe tolerar.

#[test]
fn hw01_status_handler_no_debe_panicar_con_estado_corrupto() {
    let _g = lock_guard();
    let state_file = format!("{}/megadaemon_state.json", STATE_DIR);
    std::fs::create_dir_all(STATE_DIR).unwrap();
    let backup = Backup::take(&state_file);
    // JSON válido pero NO objeto: lo que deja un fichero de estado a medias.
    std::fs::write(&state_file, "[]").unwrap();

    let port = 39411u16;
    start_cockpit(port);
    let (headers, body) = http_raw_auth(port, "GET", "/api/status", None, true);
    println!("[HW-01] headers={:?} body_len={}", headers, body.len());

    backup.restore();
    assert!(
        !body.is_empty() && headers.starts_with("HTTP/1.1 200"),
        "HW-01: el handler de /api/status murió (respuesta vacía) => con unwind \
         (release) el cockpit COMPLETO se cae por un fichero de estado corrupto"
    );
}

// ========================================================================= //
// HW-02 · cockpit_server.rs · ESCRITURA ARBITRARIA DE FICHEROS vía
//         /api/memory_freeze  (y el cockpit no tiene autenticación + CORS *)
// ========================================================================= //

#[test]
fn hw02_memory_freeze_no_escribe_fuera_del_sandbox() {
    let _g = lock_guard();
    let victim = std::env::temp_dir().join("scout_victim_hw02.bin");
    let victim_s = victim.to_string_lossy().to_string();
    std::fs::write(&victim, b"ORIGINAL").unwrap();

    let port = 39412u16;
    start_cockpit(port);
    let body = serde_json::json!({
        "action": "add",
        "lock_id": "scout-hw02",
        "target": victim_s,
        "offset": 0,
        "value_hex": "41"
    })
    .to_string();
    let (headers, resp) = http_raw(port, "POST", "/api/memory_freeze", Some(&body));
    println!("[HW-02] headers={:?} body={}", headers, resp);

    std::thread::sleep(Duration::from_millis(400));
    let content = std::fs::read(&victim).unwrap_or_default();
    println!("[HW-02] contenido de la víctima: {:?}", String::from_utf8_lossy(&content));
    let _ = megadaemon_core::freezer::remove_lock("scout-hw02");
    let _ = std::fs::remove_file(&victim);

    assert_eq!(
        content,
        b"ORIGINAL".to_vec(),
        "HW-02: el daemon escribió en un path arbitrario elegido por el cliente HTTP"
    );
}

// ========================================================================= //
// HW-03 · providers.rs · SSRF + EXFILTRACIÓN DE LA API KEY AL GUARDAR
//         EL `endpoint` de un proveedor existente
// ========================================================================= //

#[test]
fn hw03_endpoint_de_proveedor_no_puede_redirigir_la_key() {
    let _g = lock_guard();
    let fake = start_fake_provider();
    let backup = write_vault(
        &format!("http://127.0.0.1:{}/original", fake.port),
        "SCOUT-SECRET-KEY-0001",
    );

    // El cliente POST /api/providers re-apunta el endpoint de un proveedor
    // que YA tiene key guardada.  `update_provider` no toca `api_key`.
    let r = megadaemon_core::providers::update_provider(
        "scout-p1",
        None,
        None,
        Some(&format!("http://127.0.0.1:{}/atacante", fake.port)),
        None,
        false,
    );
    assert!(r.is_ok(), "no se pudo re-apuntar el endpoint: {:?}", r.err());

    let res = megadaemon_core::providers::execute_chat("hola", Some("scout-p1"), None, None, 32);
    println!("[HW-03] respuesta del proveedor: {:?}", res.reply);
    let reqs = fake.requests.lock().unwrap().clone();
    for q in &reqs {
        println!("[HW-03] petición capturada:\n{}", q);
    }
    backup.restore();

    let leaked = reqs.iter().any(|q| q.contains("SCOUT-SECRET-KEY-0001"));
    assert!(
        !leaked,
        "HW-03: la API key almacenada se envió a un host elegido por el cliente (SSRF + robo de credenciales)"
    );
}

// ========================================================================= //
// HW-04 · providers.rs · el adjunto puede ser CUALQUIER fichero local y su
//         contenido se sube al proveedor (fuga de datos del dispositivo)
// ========================================================================= //

#[test]
fn hw04_adjunto_no_filtra_contenido_de_ficheros_locales() {
    let _g = lock_guard();
    let fake = start_fake_provider();
    let backup = write_vault(
        &format!("http://127.0.0.1:{}/original", fake.port),
        "SCOUT-KEY-0002",
    );

    // Un fichero local cualquiera (fuera de uploads/): la clave del wallet.
    let secret = std::env::temp_dir().join("scout_secret_hw04.txt");
    std::fs::write(&secret, "SCOUT_TOP_SECRET_WALLET_ABC123\n").unwrap();
    let secret_s = secret.to_string_lossy().to_string();

    let _ = megadaemon_core::providers::execute_chat(
        "analiza esto",
        Some("scout-p1"),
        Some(&secret_s),
        None,
        32,
    );
    let reqs = fake.requests.lock().unwrap().clone();
    for q in &reqs {
        println!("[HW-04] petición capturada:\n{}", q);
    }
    let _ = std::fs::remove_file(&secret);
    backup.restore();

    assert!(
        !reqs.iter().any(|q| q.contains("SCOUT_TOP_SECRET_WALLET_ABC123")),
        "HW-04: el daemon leyó y subió el contenido de un fichero local arbitrario al proveedor"
    );
}

// ========================================================================= //
// HW-05 · cockpit_server.rs · `Content-Length` sin límite razonable =>
//         100 MB de RAM por conexión (y hay un hilo por conexión)
// ========================================================================= //

#[test]
fn hw05_content_length_no_reserva_100mb_por_conexion() {
    let _g = lock_guard();
    let port = 39413u16;
    start_cockpit(port);

    let before = rss_kb();
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    // 40 bytes de payload ⇒ ~100 MB reservados en el servidor.
    s.write_all(
        b"POST /api/chat HTTP/1.1\r\nHost: x\r\nContent-Length: 104857600\r\n\r\n",
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(700));
    let after = rss_kb();
    println!(
        "[HW-05] RSS antes={} KiB después={} KiB delta={} KiB",
        before,
        after,
        after - before
    );
    drop(s);

    assert!(
        after.saturating_sub(before) < 20_000,
        "HW-05: una sola conexión con `Content-Length: 100MB` reservó {} KiB de RAM",
        after.saturating_sub(before)
    );
}

// ========================================================================= //
// HW-06 · npu_bridge.rs · `su -c` SIN timeout: la sonda NPU se cuelga para
//         siempre y se lanza en CADA sondeo (npu_status, brain_status, ...)
// ========================================================================= //

#[test]
fn hw06_su_sin_timeout_no_cuelga_la_sonda() {
    let _g = lock_guard();

    if std::env::var("SCOUT_CHILD").is_ok() {
        // Hijo: corre como nobody y con un `su` falso que no devuelve nunca
        // (equivale al prompt de root de Android).
        let t = Instant::now();
        let _ = megadaemon_core::npu_bridge::probe_exynos_npu();
        println!("[HW-06] la sonda volvió en {} ms", t.elapsed().as_millis());
        return;
    }

    // `su` falso: se queda colgado para siempre.
    let dir = std::env::temp_dir().join("scout_fake_su_hw06");
    std::fs::create_dir_all(&dir).unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let su = dir.join("su");
    std::fs::write(&su, b"#!/bin/sh\nsleep 600\n").unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&su, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let exe = std::env::current_exe().unwrap();
    let path = format!(
        "{}:{}",
        dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut cmd = Command::new(exe);
    cmd.arg("--exact")
        .arg("hw06_su_sin_timeout_no_cuelga_la_sonda")
        .arg("--nocapture")
        .env("SCOUT_CHILD", "1")
        .env("PATH", path)
        .env_remove("MEGADAEMON_NPU_RAW")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    unsafe {
        use std::os::unix::process::CommandExt;
        cmd.pre_exec(|| {
            libc::setgid(65534);
            libc::setuid(65534);
            Ok(())
        });
    }
    let mut child = cmd.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut finished = false;
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) => {
                finished = true;
                break;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => break,
        }
    }
    if !finished {
        let _ = child.kill();
        let _ = child.wait();
    }
    let _ = std::fs::remove_file(&su);
    let _ = std::fs::remove_dir(&dir);

    assert!(
        finished,
        "HW-06: probe_exynos_npu() no volvió nunca: `su -c` se queda colgado sin timeout \
         (y el cockpit lanza una sonda por request)"
    );
}

// ========================================================================= //
// HW-07 · npu_cv.rs / shm.rs · la cabecera del SHM (28 bytes) controla una
//         asignación de hasta 12,9 GB => SIGABRT del proceso
// ========================================================================= //

#[test]
fn hw07_cabecera_shm_no_puede_forzar_12gb() {
    let _g = lock_guard();

    if std::env::var("SCOUT_CHILD").is_ok() {
        // Hijo: analiza el frame con una cabecera hostil (65535x65535).
        let rep = megadaemon_core::npu_cv::analyze_vla_frame_and_accel();
        println!(
            "[HW-07] análisis terminado: {}x{}",
            rep.frame_width, rep.frame_height
        );
        return;
    }

    let backup = Backup::take(SHM_PATH);
    write_shm(65535, 65535, 0xAB);

    // Límite de 1 GiB de espacio de direcciones: la asignación desborda.
    let exe = std::env::current_exe().unwrap();
    let cmd = format!(
        "ulimit -v 1000000; exec '{}' --exact hw07_cabecera_shm_no_puede_forzar_12gb --nocapture",
        exe.display()
    );
    let out = Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .env("SCOUT_CHILD", "1")
        .output()
        .unwrap();
    backup.restore();
    println!(
        "[HW-07] status={:?}\nstdout={}\nstderr={}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    assert!(
        out.status.success(),
        "HW-07: el proceso abortó (SIGABRT/OOM) por una cabecera de 28 bytes: {:?}",
        out.status
    );
}

// ========================================================================= //
// HW-08 · main.rs · `shm-frame-bmp` PANICA con una cabecera incoherente
// ========================================================================= //

#[test]
fn hw08_shm_frame_bmp_no_panea() {
    let _g = lock_guard();
    let backup = Backup::take(SHM_PATH);
    // 1000x1000x3 = 3 MB de frame en un fichero de 1 MB.
    write_shm(1000, 1000, 0xAB);

    let out = Command::new(env!("CARGO_BIN_EXE_megadaemon-core"))
        .arg("shm-frame-bmp")
        .output()
        .unwrap();
    backup.restore();
    println!(
        "[HW-08] status={:?} stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    assert!(
        out.status.success(),
        "HW-08: `shm-frame-bmp` paniqueó con una cabecera de frame incoherente: {:?}",
        out.status
    );
}

// ========================================================================= //
// HW-09 · main.rs · `--max-turns N` (con espacio) no se parsea: el número se
//         cuela en la consulta del usuario
// ========================================================================= //

#[test]
fn hw09_max_turns_no_altera_la_consulta() {
    let _g = lock_guard();
    let a = Command::new(env!("CARGO_BIN_EXE_megadaemon-core"))
        .args(["seere", "$ echo SCOUT_PROOF_HW09"])
        .output()
        .unwrap();
    let b = Command::new(env!("CARGO_BIN_EXE_megadaemon-core"))
        .args(["seere", "--max-turns", "8", "$ echo SCOUT_PROOF_HW09"])
        .output()
        .unwrap();
    // Se compara el CONTENIDO (respuesta + modo), no el JSON entero: el campo
    // "ms" cambia entre ejecuciones y no es lo que se quiere comparar.
    let key = |out: &[u8]| -> String {
        let v: serde_json::Value =
            serde_json::from_slice(String::from_utf8_lossy(out).trim().as_bytes())
                .unwrap_or(serde_json::Value::Null);
        format!(
            "mode={} answer={:?}",
            v.get("mode").and_then(|x| x.as_str()).unwrap_or("?"),
            v.get("answer").and_then(|x| x.as_str()).unwrap_or("?")
        )
    };
    let sa = key(&a.stdout);
    let sb = key(&b.stdout);
    println!("[HW-09] sin flag : {}", sa);
    println!("[HW-09] con flag : {}", sb);

    assert_eq!(
        sa, sb,
        "HW-09: `--max-turns N` alteró la consulta (el 8 se coló en el prompt)"
    );
}

// ========================================================================= //
// HW-10 · npu_bridge.rs · telemetría FALSA: se declara KleidiAI/dotprod
//         activo (y los símbolos EDEN "ELF-verified") cuando la carga FALLÓ
// ========================================================================= //

#[test]
fn hw10_telemetria_no_declara_kleidiai_sin_cargar_la_lib() {
    let _g = lock_guard();
    std::fs::create_dir_all(APP_USR_LIB).unwrap();
    let fake_so = format!("{}/libggml-cpu.so", APP_USR_LIB);
    let backup = Backup::take(&fake_so);
    // NO es una ELF válida: dlopen fallará.
    std::fs::write(&fake_so, b"NOT-AN-ELF").unwrap();

    let tel = megadaemon_core::npu_bridge::probe_exynos_npu();
    println!(
        "[HW-10] kleidiai_dotprod_active={} ggml_backends={:?} cpu_temp_c={} battery_temp_c={} throttle={}",
        tel.kleidiai_dotprod_active,
        tel.ggml_backends,
        tel.cpu_temp_c,
        tel.battery_temp_c,
        tel.throttle_state
    );
    backup.restore();

    assert!(
        !tel.kleidiai_dotprod_active,
        "HW-10: la telemetría declara KleidiAI/dotprod ACTIVO aunque la librería NO se pudo cargar"
    );
    assert!(
        !tel.ggml_backends.iter().any(|b| b.contains("ggml-cpu")),
        "HW-10: se anuncia el backend 'ggml-cpu (KleidiAI SDOT)' sin haberlo cargado"
    );
}

// ========================================================================= //
// HW-11 · npu_cv.rs · el PRIMER frame fabrica un delta L1 de 12,5 y dice
//         "scene_changed: true" aunque no haya frame anterior
// ========================================================================= //

#[test]
fn hw11_primer_frame_no_inventa_movimiento() {
    let _g = lock_guard();

    if std::env::var("SCOUT_CHILD").is_ok() {
        // Hijo: estado limpio (PREV_LUMA vacío).
        write_shm(320, 224, 0x20); // frame estático
        let r1 = megadaemon_core::npu_cv::analyze_vla_frame_and_accel();
        let r2 = megadaemon_core::npu_cv::analyze_vla_frame_and_accel();
        println!(
            "[HW-11] 1er análisis: delta={} scene_changed={} | 2º: delta={} scene_changed={}",
            r1.frame_delta_l1, r1.scene_changed, r2.frame_delta_l1, r2.scene_changed
        );
        println!("[HW-11] PRIMER_DELTA={} PRIMER_CHANGED={}", r1.frame_delta_l1, r1.scene_changed);
        return;
    }

    let backup = Backup::take(SHM_PATH);
    let out = run_child(
        "hw11_primer_frame_no_inventa_movimiento",
        &[],
        Duration::from_secs(120),
    );
    backup.restore();
    let out = out.expect("el hijo no terminó");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    println!("[HW-11] salida del hijo:\n{}", stdout);

    let first_delta = stdout
        .lines()
        .find(|l| l.contains("PRIMER_DELTA="))
        .map(|l| l.trim().to_string())
        .unwrap_or_default();
    let first_changed = stdout
        .lines()
        .find(|l| l.contains("PRIMER_CHANGED="))
        .map(|l| l.trim().to_string())
        .unwrap_or_default();
    println!("[HW-11] {} / {}", first_delta, first_changed);

    assert!(
        !first_changed.contains("PRIMER_CHANGED=true"),
        "HW-11: el primer análisis (sin frame anterior) inventó movimiento: {}",
        first_changed
    );
    assert!(
        !first_delta.contains("PRIMER_DELTA=12.5"),
        "HW-11: el delta L1 del primer frame es la constante inventada 12.5: {}",
        first_delta
    );
}

// ========================================================================= //
// HW-12 · providers.rs · `mask_key` filtra casi toda una key corta
// ========================================================================= //

#[test]
fn hw12_mascara_de_key_no_filtra_casi_toda_la_key() {
    let _g = lock_guard();
    let key = "ABCDEFGHIJK"; // 11 caracteres
    let backup = write_vault("https://example.invalid/x", key);
    let red = megadaemon_core::providers::list_providers_redacted();
    let masked = red["providers"][0]["key_masked"]
        .as_str()
        .unwrap_or("")
        .to_string();
    backup.restore();
    println!("[HW-12] key='{}' mask='{}'", key, masked);

    // Cuántos caracteres de la clave original quedan visibles en la máscara.
    let revealed = key.chars().filter(|c| masked.contains(*c)).count();
    println!("[HW-12] caracteres de la clave visibles en la máscara: {}/{}", revealed, key.len());

    assert!(
        revealed <= 4,
        "HW-12: la máscara '{}' deja ver {}/11 caracteres de la API key",
        masked,
        revealed
    );
}

// ========================================================================= //
// HW-13 · providers.rs · pedir un modelo RETIRADO lo sustituye en silencio
//         por otro proveedor y devuelve `fallback_used: false`
// ========================================================================= //

#[test]
fn hw13_modelo_retirado_no_se_sustituye_en_silencio() {
    let _g = lock_guard();
    let fake = start_fake_provider();
    let backup = write_vault(
        &format!("http://127.0.0.1:{}/x", fake.port),
        "SCOUT-KEY-0013",
    );

    let res = megadaemon_core::providers::execute_chat(
        "hola",
        Some("minimax-m2.7"), // RETIRADO el 2026-11-18 según provider_catalog
        None,
        None,
        32,
    );
    println!(
        "[HW-13] model_id={} fallback_used={} reply={:?}",
        res.model_id, res.fallback_used, res.reply
    );
    backup.restore();

    let avisó = res.reply.to_uppercase().contains("RETIRADO")
        || res.reply.to_uppercase().contains("RETIR")
        || res.model_id == "minimax-m2.7";
    assert!(
        avisó,
        "HW-13: se pidió 'minimax-m2.7' (RETIRADO) y el daemon respondió como otro \
         proveedor (model_id={}) sin avisar y con fallback_used={}",
        res.model_id, res.fallback_used
    );
}

// ========================================================================= //
// HW-14 · hw_mapper.rs · los "inferred_structs" son constantes (idem para
//         cualquier plataforma) y su `total_size_est` contradice los campos
// ========================================================================= //

#[test]
fn hw14_structs_inferidos_no_pueden_ser_contradicitorios() {
    let a = megadaemon_core::hw_mapper::analyze_text_for_mmio_and_structs("msx2", "0x98 0x44");
    let b = megadaemon_core::hw_mapper::analyze_text_for_mmio_and_structs("pce", "0x0000 0x0400");

    let na: Vec<&str> = a.inferred_structs.iter().map(|s| s.struct_name.as_str()).collect();
    let nb: Vec<&str> = b.inferred_structs.iter().map(|s| s.struct_name.as_str()).collect();
    println!("[HW-14] structs msx2={:?} pce={:?}", na, nb);

    // 1) Coherencia interna: el tamaño declarado debe cubrir los campos.
    for s in &a.inferred_structs {
        let last = s
            .fields
            .iter()
            .map(|f| {
                let bytes = if f.field_type.starts_with("uint8[") {
                    f.field_type
                        .trim_start_matches("uint8[")
                        .trim_end_matches(']')
                        .parse::<usize>()
                        .unwrap_or(1)
                } else if f.field_type.starts_with("uint16") {
                    2
                } else {
                    4
                };
                f.offset_dec + bytes
            })
            .max()
            .unwrap_or(0);
        println!(
            "[HW-14] {}: total_size_est={} último campo acaba en {}",
            s.struct_name, s.total_size_est, last
        );
        assert!(
            s.total_size_est >= last,
            "HW-14: '{}' declara size={} pero sus campos llegan a {}",
            s.struct_name,
            s.total_size_est,
            last
        );
    }

    // 2) "Inferidos" ≠ constantes: dos plataformas distintas no pueden dar
    //    exactamente la misma estructura de un platform-specific.
    let sa: Vec<(String, String, usize)> = a
        .inferred_structs
        .iter()
        .map(|s| (s.struct_name.clone(), s.base_var.clone(), s.total_size_est))
        .collect();
    let sb: Vec<(String, String, usize)> = b
        .inferred_structs
        .iter()
        .map(|s| (s.struct_name.clone(), s.base_var.clone(), s.total_size_est))
        .collect();
    assert!(
        sa != sb,
        "HW-14: las estructuras 'inferidas' son idénticas para msx2 y pce: no se infieren, \
         son constantes (base_var={:?})",
        sa.first().map(|x| x.1.clone())
    );
}

// ========================================================================= //
// HW-15 · power_policy.rs · el governor del cluster PEQUEÑO (policy0)
//         marca el sistema como "power save" y la rama "saver" tapa la
//         rama "thermal": un móvil al 70 °C se reporta como "ahorro"
// ========================================================================= //

#[test]
fn hw15_thermal_no_se_reporta_como_ahorro() {
    use megadaemon_core::power_policy::{policy_for, PowerState};

    // Estado real de un móvil Android: policy0 (little) en "powersave" — lo
    // normal —, cargando, al 100 % y con la CPU a 70 °C.
    let st = PowerState {
        charging: true,
        capacity_pct: Some(100),
        battery_temp_c: Some(30.0),
        cpu_temp_c: Some(70.0),
        system_power_save: true,
        governor: Some("powersave".to_string()),
        thermal_hot: true,
        usb_plugged: true,
    };
    let pol = policy_for(&st);
    println!(
        "[HW-15] policy={} reason={:?} max_tokens={} allow_heavy={}",
        pol.name, pol.reason, pol.max_tokens, pol.allow_heavy
    );

    assert_eq!(
        pol.name, "thermal",
        "HW-15: con la CPU a 70 °C la política debe ser 'thermal', no '{}' ({})",
        pol.name, pol.reason
    );
    assert!(
        pol.reason.contains("70") || pol.reason.to_lowercase().contains("term"),
        "HW-15: el motivo no menciona la temperatura: {:?}",
        pol.reason
    );
}

// ========================================================================= //
// HW-16 · npu_bridge.rs · temperaturas FABRICADAS (32,0 / 35,0) presentadas
//         como si fueran medidas
// ========================================================================= //

#[test]
fn hw16_telemetria_termica_no_inventa_lecturas() {
    let (freqs, batt, cpu, state, delay) = megadaemon_core::npu_bridge::read_cpu_and_thermal();
    println!(
        "[HW-16] freqs={:?} batt_temp={} cpu_temp={} throttle={} delay={}",
        freqs, batt, cpu, state, delay
    );
    assert!(
        batt != 32.0 && cpu != 35.0,
        "HW-16: sin sensores se reportan las constantes inventadas (batt={} cpu={}) como si \
         fuesen medidas; además el estado térmico se deriva de ellas",
        batt, cpu
    );
}
