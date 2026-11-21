//! MegaDaemon — TERCERA AUDITORÍA ADVERSARIAL (scout UI / HTTP / providers / MCP)
//!
//! Suite **independiente** de `audit_scout_hw.rs` (HW-01..HW-16) y de
//! `audit_scout_seere.rs` (S01..S14b).  No repite ninguno de los defectos ya
//! documentados por esas dos auditorías:
//!
//!   * NO se prueba el RCE sin token de POST /api/chat (S08), ni el CORS (S08c),
//!     ni el sandbox de memory_freeze con una ruta estática (HW-02), ni el
//!     adjunto de `providers.rs` (HW-04), ni la key atada al endpoint (HW-03),
//!     ni el Content-Length (HW-05), ni la cabecera SHM en `npu_cv` (HW-07),
//!     ni los modelos retired (HW-13/S10), ni la gramática/parser de SEERE.
//!   * Lo que se busca aquí es lo que se ESCAPÓ: la capa HTTP escrita a mano,
//!     la bóveda de proveedores, el protocolo MCP, la lectura de ficheros del
//!     cockpit y el JavaScript de la UI web.
//!
//! Ejecutar:
//!   export PATH="$HOME/.cargo/bin:$PATH"
//!   cd /workspace/megadaemon/rust-core
//!   cargo test --test audit_scout_ui -- --nocapture --test-threads=1
//!
//! Cada test reproduce un defecto REAL y FALLA con el código actual.
//! Los tests que tocan estado global (bóveda de providers, SHM, cockpit) van
//! serializados con un mutex para que el orden no altere el resultado.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

const STATE_DIR: &str = "/data/user/0/com.agy/files/megadaemon/state";
const PROVIDERS_PATH: &str = "/data/user/0/com.agy/files/megadaemon/state/providers.json";
const TOKEN_PATH: &str = "/data/user/0/com.agy/files/megadaemon/state/cockpit_token";
const SHM_PATH: &str = "/data/user/0/com.agy/files/snatcher_vla_shm.bin";
const UPLOADS_DIR: &str = "/data/user/0/com.agy/files/megadaemon/uploads";
const SHM_SIZE: usize = 1_048_576;
const FRAME_OFFSET: usize = 32824;

// ─────────────────────────────────────────────────────────────────────────────
// Utilidades
// ─────────────────────────────────────────────────────────────────────────────

fn lock_guard() -> MutexGuard<'static, ()> {
    static L: OnceLock<Mutex<()>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

fn free_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

/// Arranca UN cockpit compartido (127.0.0.1) y devuelve su puerto.
fn cockpit_port() -> u16 {
    static P: OnceLock<u16> = OnceLock::new();
    *P.get_or_init(|| {
        let port = free_port();
        std::thread::spawn(move || {
            let _ = megadaemon_core::cockpit_server::run_server(port);
        });
        for _ in 0..300 {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                // Un GET "/" fuerza la materialización del token en disco.
                let _ = http_raw(port, "GET", "/", None, "");
                return port;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("el cockpit no arrancó en el puerto {}", port);
    })
}

/// Token de autenticación del cockpit (lo genera el servidor al arrancar).
fn auth_token() -> String {
    for _ in 0..100 {
        if let Ok(s) = fs::read_to_string(TOKEN_PATH) {
            let t = s.trim().to_string();
            if !t.is_empty() {
                return t;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("no aparece {}", TOKEN_PATH);
}

/// Petición HTTP cruda.  Devuelve (status, cabeceras, cuerpo).
fn http_raw(
    port: u16,
    method: &str,
    path: &str,
    body: Option<&str>,
    extra_headers: &str,
) -> (u16, String, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).expect("no se pudo conectar al cockpit");
    s.set_read_timeout(Some(Duration::from_secs(25))).unwrap();
    s.set_write_timeout(Some(Duration::from_secs(25))).unwrap();
    let payload = body.unwrap_or("");
    let req = format!(
        "{} {} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\n{}Content-Length: {}\r\nConnection: close\r\n\r\n{}",
        method,
        path,
        extra_headers,
        payload.len(),
        payload
    );
    let _ = s.write_all(req.as_bytes());
    let mut raw = Vec::new();
    let _ = s.read_to_end(&mut raw);
    let text = String::from_utf8_lossy(&raw).into_owned();
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    match text.find("\r\n\r\n") {
        Some(i) => (status, text[..i].to_string(), text[i + 4..].to_string()),
        None => (status, text.clone(), String::new()),
    }
}

fn http_with_token(
    port: u16,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> (u16, String, String) {
    let h = format!("X-Seere-Auth-Token: {}\r\n", auth_token());
    http_raw(port, method, path, body, &h)
}

/// Copia de seguridad / restauración de un fichero.
struct Backup {
    path: String,
    data: Option<Vec<u8>>,
}

impl Backup {
    fn take(path: &str) -> Backup {
        Backup {
            path: path.to_string(),
            data: fs::read(path).ok(),
        }
    }
    fn restore(&self) {
        match &self.data {
            Some(d) => {
                let _ = fs::write(&self.path, d);
            }
            None => {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}

/// Escribe una SHM válida (o hostil) de 1 MiB.
fn write_shm(width: u16, height: u16, payload: u8) {
    let mut buf = vec![0u8; SHM_SIZE];
    buf[0..4].copy_from_slice(&0x534E4154u32.to_le_bytes()); // "SNAT"
    buf[4..8].copy_from_slice(&13u32.to_le_bytes());
    buf[8..12].copy_from_slice(&7u32.to_le_bytes());
    buf[20..22].copy_from_slice(&width.to_le_bytes());
    buf[22..24].copy_from_slice(&height.to_le_bytes());
    buf[24..26].copy_from_slice(&24u16.to_le_bytes());
    buf[26..28].copy_from_slice(&1u16.to_le_bytes());
    buf[32..40].copy_from_slice(b"SCOUTFRAM");
    let end = std::cmp::min(FRAME_OFFSET + width as usize * height as usize * 3, buf.len());
    for b in buf[FRAME_OFFSET..end].iter_mut() {
        *b = payload;
    }
    fs::write(SHM_PATH, &buf).expect("no se pudo escribir la SHM de prueba");
}

/// Escribe una bóveda de providers de prueba.
fn write_vault(providers: serde_json::Value, default_model: &str) {
    let _ = fs::create_dir_all(STATE_DIR);
    let v = serde_json::json!({ "default_model": default_model, "providers": providers });
    fs::write(PROVIDERS_PATH, serde_json::to_string_pretty(&v).unwrap()).unwrap();
}

/// Proveedor HTTP falso: captura las peticiones (para A-07).
struct FakeProvider {
    port: u16,
    hits: Arc<AtomicUsize>,
    auths: Arc<Mutex<Vec<String>>>,
}

fn start_fake_provider() -> FakeProvider {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let hits = Arc::new(AtomicUsize::new(0));
    let auths = Arc::new(Mutex::new(Vec::new()));
    let h2 = hits.clone();
    let a2 = auths.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut sock = stream;
            let _ = sock.set_read_timeout(Some(Duration::from_secs(10)));
            let mut raw = Vec::new();
            let mut chunk = [0u8; 8192];
            let mut header_end = 0usize;
            let mut content_length = 0usize;
            loop {
                match sock.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        raw.extend_from_slice(&chunk[..n]);
                        if header_end == 0 {
                            if let Some(p) = find(&raw, b"\r\n\r\n") {
                                header_end = p + 4;
                                let head = String::from_utf8_lossy(&raw[..p]).to_lowercase();
                                for l in head.lines() {
                                    if let Some(v) = l.strip_prefix("content-length:") {
                                        content_length = v.trim().parse().unwrap_or(0);
                                    }
                                }
                            }
                        }
                        if header_end > 0 && raw.len() >= header_end + content_length {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            let text = String::from_utf8_lossy(&raw).into_owned();
            let auth = text
                .lines()
                .find(|l| l.to_lowercase().starts_with("authorization:"))
                .map(|l| l[14..].trim().to_string())
                .unwrap_or_else(|| "<none>".to_string());
            a2.lock().unwrap().push(auth);
            h2.fetch_add(1, Ordering::SeqCst);
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
    FakeProvider { port, hits, auths }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.len() > hay.len() {
        return None;
    }
    (0..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

// ========================================================================= //
// UI-01 · cockpit_server.rs:1195-1201 · GET /api/input ESCRIBE en la SHM
//        (registro de mando del juego) y SIEMPRE responde "ok", aunque la
//        escritura haya sido imposible
// ========================================================================= //
//
// El registro INPUT (offset 28) lo lee el driver de Snatcher
// (`re_pipeline/snatcher_headless_driver.py`): es el mando del juego.
//   * v13.2.3 (el principal) ya puso el endpoint en modo fail-closed: sin
//     token responde 403.  Ese hueco está CERRADO (control del test).
//   * Lo que SIGUE roto: el handler ignora el resultado de la escritura
//     (`let _ = f.write_all(...)`) y responde `{"status":"ok"}` SIEMPRE.  Con
//     la SHM ausente o sin permisos, la UI y el API dicen "ok" y el caller
//     cree que el mando se aplicó: respuesta incorrecta + pérdida de estado.

#[test]
fn ui01_get_api_input_no_puede_mutar_la_shm_sin_token() {
    let _g = lock_guard();
    let port = cockpit_port();
    let shm_backup = Backup::take(SHM_PATH);

    // ── control: sin token NO debe poder mutar el registro de mando ──
    write_shm(64, 64, 0x11);
    let (status, _h, body) = http_raw(port, "GET", "/api/input?mask=16909060", None, "");
    println!("[UI-01] GET /api/input?mask=16909060 (SIN token) -> {}\n{}", status, body);
    let reg = {
        let b = fs::read(SHM_PATH).unwrap();
        u32::from_le_bytes(b[28..32].try_into().unwrap())
    };
    println!("[UI-01] registro INPUT (offset 28) tras la petición = {:#x}", reg);
    assert_eq!(
        status, 403,
        "UI-01: un GET que MUTA el registro de mando del juego debe exigir token (403), no {}",
        status
    );
    assert_eq!(
        reg, 0,
        "UI-01: sin token la SHM no debe modificarse (registro INPUT = {:#x})",
        reg
    );

    // ── DEFECTO: con token, si la escritura falla NO debe decir "ok" ──
    fs::rename(SHM_PATH, "/tmp/ui01_shm_hidden.bin").unwrap();
    let (st2, _h2, body2) = http_with_token(port, "GET", "/api/input?mask=1", None);
    println!("[UI-01b] GET /api/input con la SHM ausente -> {} {}", st2, body2);
    let _ = fs::rename("/tmp/ui01_shm_hidden.bin", SHM_PATH);
    assert!(
        !body2.contains("\"status\":\"ok\""),
        "UI-01: la respuesta dice 'ok' aunque la escritura en la SHM fue imposible: {}",
        body2
    );

    shm_backup.restore();
}

// ========================================================================= //
// UI-02 · CONTROL (v13.2.3, ya corregido por el principal) ·
//        POST /api/runners/control NO exigía token y `ensure_live_shm()`
//        DESTRUÍA el frame vivo de la VLA
// ========================================================================= //
//
// Test de REGRESIÓN: el endpoint ya está en modo fail-closed y no debe
// reproducir el hueco.  Se conserva porque el defecto era destructor
// (`ensure_live_shm()` sigue escribiendo un frame sintético sobre el
// fichero SHM que la app Android está capturando).

#[test]
fn ui02_runners_control_exige_token_y_no_destruye_el_frame_vivo() {
    let _g = lock_guard();
    let port = cockpit_port();
    let shm_backup = Backup::take(SHM_PATH);

    write_shm(64, 64, 0x5A); // frame "vivo" recognizable
    let before = fs::read(SHM_PATH).unwrap();
    let tag_before = String::from_utf8_lossy(&before[32..44]).to_string();
    println!("[UI-02] frame vivo antes: tag={:?} byte_32700={:#x}", tag_before, before[32700]);

    let (status, _h, body) = http_raw(
        port,
        "POST",
        "/api/runners/control",
        Some(r#"{"action":"launch","target":"segacd"}"#),
        "",
    );
    println!("[UI-02] POST /api/runners/control (SIN token) -> {}\n{}", status, body);

    let after = fs::read(SHM_PATH).unwrap();
    let tag_after = String::from_utf8_lossy(&after[32..80]).to_string();
    println!("[UI-02] SHM después: tag={:?} byte_32700={:#x}", tag_after, after[32700]);

    assert_eq!(
        status, 403,
        "UI-02: /api/runners/control muta el estado (lanza el runner) y debe exigir token, no {}",
        status
    );
    assert!(
        after[32700] == before[32700] && tag_after == tag_before,
        "UI-02: la SHM viva fue destruida sin token (antes tag={:?}/byte={:#x}, después tag={:?}/byte={:#x})",
        tag_before,
        before[32700],
        tag_after,
        after[32700]
    );

    shm_backup.restore();
}

// ========================================================================= //
// UI-03 · cockpit_server.rs:900-917 + shm.rs:73-122 · /api/live_frame usa
//        read_shm_status() (SIN validar dimensiones) => la cabecera de 28
//        bytes provoca un panic por overflow en encode_bmp_24() (debug) o una
//        asignación de GB (release, panic=abort => MUERE el cockpit entero)
// ========================================================================= //
//
// HW-07 probó la ruta `npu_cv::analyze_vla_frame_and_accel` (que sí valida las
// dimensiones en `shm::read_frame_snapshot`).  El endpoint HTTP del cockpit
// usa OTRA ruta (`shm::read_shm_status()` + `shm::encode_bmp_24()`), donde las
// dimensiones NO se validan: 65535x65535 desborda `row_size * height` (u32).

#[test]
fn ui03_live_frame_no_muere_con_una_cabecera_shm_hostil() {
    let _g = lock_guard();
    let port = cockpit_port();
    let shm_backup = Backup::take(SHM_PATH);

    write_shm(65535, 65535, 0xAB); // cabecera hostil
    let (status, headers, body) = http_with_token(port, "GET", "/api/live_frame", None);
    println!(
        "[UI-03] GET /api/live_frame con SHM 65535x65535 -> status={} len_cuerpo={}\ncabeceras:\n{}",
        status,
        body.len(),
        headers
    );

    // El servidor debe seguir respondiendo (el handler no debe morir).
    let (ping_status, _h, ping_body) = http_with_token(port, "GET", "/api/ping", None);
    println!("[UI-03] /api/ping después -> {} {}", ping_status, ping_body);

    assert!(
        status == 200 || status == 400 || status == 413,
        "UI-03: /api/live_frame debe responder (200/4xx), status={} cuerpo={} bytes (la conexión murió: el handler paniqueó)",
        status,
        body.len()
    );
    assert!(
        !body.is_empty(),
        "UI-03: /api/live_frame devolvió un cuerpo vacío: el handler murió (panic en shm::encode_bmp_24)"
    );
    assert_eq!(
        ping_status, 200,
        "UI-03: el cockpit dejó de responder tras /api/live_frame (panic=abort en release)"
    );

    shm_backup.restore();
}

// ========================================================================= //
// UI-04 · cockpit_server.rs:213-239 (resolve_target_or_file) + 790-800
//        · LECTURA ARBITRARIA DE FICHEROS por los endpoints RE del cockpit
//        (sin token): incluye la bóveda con las API keys
// ========================================================================= //
//
// HW-04 corrigió el sandbox de los ADJUNTOS (`providers::attachment_allowed`).
// El mismo primitive sigue vivo en el cockpit: `resolve_target_or_file()`
// acepta cualquier ruta absoluta existente y la devuelve tal cual a
// `disasm::inspect_hex`, que vuelca el contenido (hex + ASCII) del fichero.
// Un GET sin token lee /etc/passwd, los datos privados de otras apps (el
// daemon corre como root en el móvil) y la bóveda de keys.

#[test]
fn ui04_los_endpoints_re_no_leen_ficheros_fuera_del_sandbox() {
    let _g = lock_guard();
    let port = cockpit_port();

    let secret_path = format!("{}/scout_secret_probe.txt", STATE_DIR);
    let _ = fs::create_dir_all(STATE_DIR);
    fs::write(&secret_path, "SCOUT_SECRET_PROBE_9F3A2B").unwrap();
    let backup = Backup::take(&secret_path);

    // El token NO hay que traerlo de ningún sitio "privado": la página PÚBLICA
    // GET / lo lleva embebido (window.MEGADAEMON_TOKEN).  Cadena real de
    // ataque: GET / -> token -> lectura de cualquier fichero del móvil.
    let (_st0, _h0, page) = http_raw(port, "GET", "/", None, "");
    let token = page
        .split("window.MEGADAEMON_TOKEN=\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .unwrap_or("")
        .to_string();
    println!("[UI-04] token extraído de la página pública: {} bytes", token.len());
    assert!(!token.is_empty(), "UI-04: la página pública debe llevar el token (si no, este test no aplica)");

    let url = format!(
        "/api/hex_inspect?file={}&offset=0&size=256",
        secret_path
    );
    let (status, _h, body) = http_raw(
        port,
        "GET",
        &url,
        None,
        &format!("X-Seere-Auth-Token: {}\r\n", token),
    );
    println!("[UI-04] GET {} (con el token de la página PÚBLICA) -> {}\n{}", url, status, body);
    assert!(
        !body.contains("SCOUT_SECRET_PROBE_9F3A2B"),
        "UI-04: el cockpit devolvió el contenido de un fichero arbitrario sin token: {:?}",
        body.chars().take(400).collect::<String>()
    );
    assert!(
        status == 403 || status == 400 || status == 404,
        "UI-04: una ruta fuera del sandbox debe rechazarse, status={}",
        status
    );

    backup.restore();
}

// ========================================================================= //
// UI-05 · providers.rs:186-217 · al guardar key + endpoint nuevo, la key se
//        BORRA y `auth_type` queda en "none" PARA SIEMPRE => el proveedor
//        queda muerto, pero la UI lo muestra como [READY]
// ========================================================================= //
//
// En `update_provider` el bloque de `api_key` se aplica ANTES que el de
// `endpoint`; el bloque de endpoint vacía la key si el host cambia y pone
// `auth_type = "none"`.  Como `auth_type` nunca se restaura, `execute_chat`
//|matchea "none" => key vacía => NUNCA se manda Authorization => 401 para
// siempre.  Y `list_providers_redacted` marca `ready=true` para "none" => la
// UI muestra [READY] sobre un proveedor muerto.

#[test]
fn ui05_cambiar_endpoint_no_puede_destruir_la_key_justo_introducida() {
    let _g = lock_guard();
    let port = cockpit_port();
    let vault_backup = Backup::take(PROVIDERS_PATH);

    write_vault(
        serde_json::json!([{
            "id": "scout-p1", "name": "Scout P1", "vendor": "Scout",
            "endpoint": "https://viejo.invalid/v1/chat/completions",
            "model": "scout-model", "api_key": "sk-VIEJA-000000",
            "auth_type": "bearer", "tier": "free", "enabled": true
        }]),
        "scout-p1",
    );

    let (status, _h, body) = http_with_token(
        port,
        "POST",
        "/api/providers",
        Some(r#"{"id":"scout-p1","api_key":"sk-NUEVA-999999","endpoint":"https://nuevo.invalid/v1/chat/completions"}"#),
    );
    println!("[UI-05] POST /api/providers (key nueva + endpoint nuevo) -> {}\n{}", status, body);

    let v: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::json!({}));
    let p = v["vault"]["providers"]
        .as_array()
        .and_then(|a| a.iter().find(|p| p["id"] == "scout-p1"))
        .cloned()
        .unwrap_or(serde_json::json!({}));
    println!("[UI-05] proveedor tras el guardado: {}", p);

    assert_eq!(
        p["has_key"], serde_json::json!(true),
        "UI-05: la API key que el usuario acababa de escribir se perdió al cambiar el endpoint: {}",
        p
    );
    assert_ne!(
        p["auth_type"], serde_json::json!("none"),
        "UI-05: auth_type quedó en 'none': el proveedor ya no podrá autenticarse nunca: {}",
        p
    );
    assert_ne!(
        p["ready"], serde_json::json!(true),
        "UI-05: la UI marcaría [READY] a un proveedor que no puede autenticarse: {}",
        p
    );

    vault_backup.restore();
}

// ========================================================================= //
// UI-06 · providers.rs:76-110 · `load_vault()`/`save_vault()` SIN LOCK:
//        dos actualizaciones concurrentes de la bóveda se pisan y se PIERDEN
//        API keys (lectura-modificación-escritura sin serializar)
// ========================================================================= //

#[test]
fn ui06_actualizaciones_concurrentes_de_la_boveda_no_pierden_keys() {
    let _g = lock_guard();
    let port = cockpit_port();
    let vault_backup = Backup::take(PROVIDERS_PATH);

    // Bóveda grande: ensancha la ventana de carrera entre read y write.
    let mut provs = Vec::new();
    for i in 0..200 {
        provs.push(serde_json::json!({
            "id": format!("pre-p{}", i), "name": format!("Pre {}", i), "vendor": "pre",
            "endpoint": "https://pre.invalid/v1/chat/completions", "model": "m",
            "api_key": "sk-PRE-000000", "auth_type": "bearer", "tier": "free", "enabled": false
        }));
    }
    write_vault(serde_json::Value::Array(provs), "pre-p0");

    let n = 12usize;
    let mut handles = Vec::new();
    for i in 0..n {
        handles.push(std::thread::spawn(move || {
            let body = format!(
                r#"{{"id":"race-p{}","api_key":"sk-RACE-{:04}-XXXX"}}"#,
                i, i
            );
            let _ = http_with_token(port, "POST", "/api/providers", Some(&body));
        }));
    }
    for h in handles {
        let _ = h.join();
    }

    let (_s, _h, body) = http_with_token(port, "GET", "/api/providers", None);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::json!({}));
    let saved: Vec<String> = v["providers"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|p| {
                    p["id"].as_str().unwrap_or("").starts_with("race-p")
                        && p["has_key"] == serde_json::json!(true)
                })
                .map(|p| p["id"].as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default();
    println!("[UI-06] keys de proveedor guardadas: {}/{} -> {:?}", saved.len(), n, saved);
    assert_eq!(
        saved.len(),
        n,
        "UI-06: se perdieron {} API keys por la carrera read-modify-write de la bóveda (load_vault/save_vault sin lock)",
        n - saved.len()
    );

    vault_backup.restore();
}

// ========================================================================= //
// UI-07 · providers.rs:671-679 · `enabled=false` NO desactiva el proveedor
//        pedido explícitamente (y la UI lo ofrece igual, sin marcarlo)
// ========================================================================= //

#[test]
fn ui07_un_proveedor_deshabilitado_no_debe_recibir_peticiones() {
    let _g = lock_guard();
    let port = cockpit_port();
    let vault_backup = Backup::take(PROVIDERS_PATH);

    let fake = start_fake_provider();
    write_vault(
        serde_json::json!([{
            "id": "scout-p1", "name": "Scout P1", "vendor": "Scout",
            "endpoint": format!("http://127.0.0.1:{}/v1/chat/completions", fake.port),
            "model": "scout-model", "api_key": "sk-SCOUT-KEY-123",
            "auth_type": "bearer", "tier": "free", "enabled": false
        }]),
        "scout-p1",
    );

    // El chat pide explícitamente el proveedor deshabilitado.
    let port_c = port;
    std::thread::spawn(move || {
        let _ = http_with_token(port_c, "POST", "/api/chat", Some(r#"{"message":"hola","model":"scout-p1"}"#));
    });

    let deadline = Instant::now() + Duration::from_secs(20);
    while fake.hits.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    let hits = fake.hits.load(Ordering::SeqCst);
    let auths = fake.auths.lock().unwrap().clone();
    println!("[UI-07] peticiones recibidas por el proveedor DESHABILITADO: {}", hits);
    println!("[UI-07] cabeceras Authorization: {:?}", auths);
    assert_eq!(
        hits, 0,
        "UI-07: un proveedor con enabled=false recibió {} petición(es): el flag no desactiva al proveedor pedido explícitamente",
        hits
    );

    vault_backup.restore();
}

// ========================================================================= //
// UI-08 · cockpit_server.rs:970-981 + providers.rs:235-255 · POST
//        /api/providers con id VACÍO responde {"status":"OK"} y PIERDE la key
// ========================================================================= //

#[test]
fn ui08_guar_provider_con_id_vacio_no_puede_perder_la_key_en_silencio() {
    let _g = lock_guard();
    let port = cockpit_port();
    let vault_backup = Backup::take(PROVIDERS_PATH);

    write_vault(serde_json::json!([]), "local-seere");
    let (status, _h, body) = http_with_token(
        port,
        "POST",
        "/api/providers",
        Some(r#"{"id":"","api_key":"sk-ORFAN-123456"}"#),
    );
    println!("[UI-08] POST /api/providers {{\"id\":\"\",\"api_key\":\"sk-ORFAN-123456\"}} -> {}\n{}", status, &body.chars().take(200).collect::<String>());

    let raw = fs::read_to_string(PROVIDERS_PATH).unwrap_or_default();
    println!("[UI-08] la bóveda contiene la key: {}", raw.contains("sk-ORFAN-123456"));

    let says_ok = body.contains("\"status\":\"OK\"");
    assert!(
        !says_ok || raw.contains("sk-ORFAN-123456"),
        "UI-08: el servidor respondió OK pero la API key quedó descartada (id vacío); el usuario cree que la guardó: {}",
        body.chars().take(200).collect::<String>()
    );

    vault_backup.restore();
}

// ========================================================================= //
// UI-09 · cockpit_server.rs:568-578 · el token de autenticación se PUBLICA
//        en el HTML que sirve GET / (sin token) => la autenticación por token
//        es evitable por cualquier cliente local
// ========================================================================= //
//
// El token es la ÚNICA barrera de /api/chat (RCE como root) y de
// /api/providers (bóveda de keys).  Se inyecta en el HTML servido a CUALQUIER
// visitor de GET /, así que "obtener el token" no requiere ninguna credencial.

#[test]
fn ui09_el_html_publico_no_debe_contener_el_token_de_autenticacion() {
    let _g = lock_guard();
    let port = cockpit_port();
    let token = auth_token();

    let (status, _h, body) = http_raw(port, "GET", "/", None, "");
    println!("[UI-09] GET / (SIN token) -> {} ({} bytes)", status, body.len());
    let leaked = body.contains(&token);
    let idx = body.find("window.MEGADAEMON_TOKEN").unwrap_or(0);
    println!(
        "[UI-09] fragmento servido: {:?}",
        body[idx..std::cmp::min(idx + 90, body.len())].to_string()
    );

    // Impacto: el token extraído de la página sirve para un endpoint protegido.
    let (st2, _h2, b2) = http_raw(
        port,
        "POST",
        "/api/providers",
        Some(r#"{"id":"scout-leak"}"#),
        &format!("X-Seere-Auth-Token: {}\r\n", token),
    );
    println!("[UI-09] POST /api/providers con el token saco de la pagina -> {}", st2);

    assert!(
        !leaked,
        "UI-09: GET / publica el token de autenticación en claro: cualquiera que llegue a 127.0.0.1 lo lee y ya puede ejecutar /api/chat (root)"
    );
    assert_ne!(
        st2, 200,
        "UI-09: el token extraido de la página autorizó un POST protegido"
    );
}

// ========================================================================= //
// UI-10 · mcp_server.rs:179-250 · el servidor MCP NO responde a las
//        peticiones JSON-RPC malformadas ni a los batches => el cliente MCP
//        se queda colgado esperando una respuesta que nunca llega
// ========================================================================= //

#[test]
fn ui10_el_servidor_mcp_responde_a_toda_peticion_con_id() {
    let bin = env!("CARGO_BIN_EXE_megadaemon-core");
    let mut child = Command::new(bin)
        .arg("mcp-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("no se pudo lanzar el servidor MCP");

    {
        let stdin = child.stdin.as_mut().unwrap();
        // 1) ping válido (control), 2) línea malformada, 3) batch con id 2.
        let _ = stdin.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n");
        let _ = stdin.write_all(b"{ESTO NO ES JSON RPC}\n");
        let _ = stdin.write_all(b"[{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}]\n");
        let _ = stdin.flush();
    }
    // EOF: el servidor sale del bucle.
    drop(child.stdin.take());

    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    println!("[UI-10] stdout del servidor MCP:\n{}", stdout);
    println!("[UI-10] stderr:\n{}", String::from_utf8_lossy(&out.stderr));

    let mut ids = Vec::new();
    let mut parse_errors = 0;
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') && !line.starts_with('[') {
            continue;
        }
        match serde_json::from_str::<serde_json::Value>(line) {
            Ok(v) => {
                if let Some(err) = v.get("error") {
                    if err["code"] == serde_json::json!(-32700) {
                        parse_errors += 1;
                    }
                }
                if let Some(id) = v.get("id") {
                    ids.push(id.to_string());
                }
                if let Some(arr) = v.as_array() {
                    for item in arr {
                        if let Some(id) = item.get("id") {
                            ids.push(id.to_string());
                        }
                    }
                }
            }
            Err(_) => {}
        }
    }
    println!("[UI-10] ids respondidos: {:?} · errores de parseo: {}", ids, parse_errors);

    assert!(
        ids.iter().any(|i| i == "2"),
        "UI-10: el servidor MCP ignoró el batch JSON-RPC (id 2) y NO respondió: el cliente se queda colgado. ids respondidos: {:?}",
        ids
    );
    assert!(
        parse_errors > 0,
        "UI-10: una petición malformada no produce error JSON-RPC -32700: el cliente espera una respuesta que nunca llega"
    );
}

// ========================================================================= //
// UI-11 · freezer.rs:127-156 + 82-108 · TOCTOU: se valida la ruta CANÓNICA
//        pero el lock guarda la ruta CRUDA y el hilo de 60 Hz la reabre
//        ⇒ un symlink cambiado después de la validación evade el sandbox
// ========================================================================= //
//
// HW-02 probó la ruta estática fuera del sandbox.  Aquí el sandbox se respeta
// en el instante de la validación y se evade después: el hilo freezer abre
// `lock.target` (la ruta CRUDA, no la canónica) 60 veces por segundo.

#[test]
fn ui11_el_freezer_no_puede_escribir_fuera_del_sandbox_con_un_symlink() {
    let _g = lock_guard();
    let port = cockpit_port();
    let _ = fs::create_dir_all(UPLOADS_DIR);
    let target = format!("{}/scout_toctou_target.bin", UPLOADS_DIR);
    let victim = "/tmp/ui11_victim.bin";
    fs::write(&target, b"SAFE-CONTENT-INSIDE-SANDBOX").unwrap();
    fs::write(victim, b"VICTIM-ORIGINAL-CONTENT").unwrap();

    let (status, _h, body) = http_with_token(
        port,
        "POST",
        "/api/memory_freeze",
        Some(&format!(
            r#"{{"action":"add","lock_id":"scout-toctou","target":"{}","offset":0,"value_hex":"41"}}"#,
            target
        )),
    );
    println!("[UI-11] add lock -> {} {}", status, body.chars().take(200).collect::<String>());
    assert_eq!(status, 200, "UI-11: no se pudo crear el lock: {}", body);

    // TOCTOU: se sustituye el fichero por un symlink a una ruta fuera del sandbox.
    let _ = fs::remove_file(&target);
    std::os::unix::fs::symlink(victim, &target).unwrap();

    std::thread::sleep(Duration::from_millis(600));

    let after = fs::read(victim).unwrap_or_default();
    println!(
        "[UI-11] contenido de la víctima tras 600 ms: {:?}",
        String::from_utf8_lossy(&after)
    );

    // Limpieza.
    let _ = http_with_token(
        port,
        "POST",
        "/api/memory_freeze",
        Some(r#"{"action":"remove","lock_id":"scout-toctou"}"#),
    );
    let _ = fs::remove_file(&target);

    assert!(
        after.starts_with(b"VICTIM"),
        "UI-11: el hilo del freezer siguió el symlink y escribió FUERA del sandbox: {:?}",
        String::from_utf8_lossy(&after)
    );
}

// ========================================================================= //
// UI-12 · web/cockpit_template.html:778-786 + 873-880 · XSS: `escHtml()`
//        NO escapa la comilla simple y el id de proveedor se mete dentro de
//        un `onclick="testProviderById('...')"` => inyección de JS en la UI
// ========================================================================= //
//
// El id del proveedor viene del servidor (bóveda) y se interpola en un
// manejador inline.  Con un `'` en el id, el JS del atributo se escapa y se
// ejecuta código arbitrario al pulsar el botón TEST (el token vive en la
// misma página, así que el XSS puede用它 para llamar a la API).
//
// El test usa el harness de Node (abajo) que ejecuta el JavaScript REAL que
// sirve el cockpit, con un DOM mínimo, y evalúa el `onclick` resultante.

#[test]
fn ui12_la_ui_no_puede_ejecutar_js_del_id_de_proveedor() {
    let _g = lock_guard();
    let port = cockpit_port();
    let vault_backup = Backup::take(PROVIDERS_PATH);

    let evil_id = "x');globalThis.__XSS_MARKER=1;//";
    let (status, _h, body) = http_with_token(
        port,
        "POST",
        "/api/providers",
        Some(&format!(
            r#"{{"id":"{}","api_key":"sk-XSS-PROBE-123456","model":"m"}}"#,
            evil_id
        )),
    );
    println!("[UI-12] alta de proveedor con id malicioso -> {}", status);

    let res = run_node_harness(port, "xss");
    println!("[UI-12] harness node (xss): {}", res);

    vault_backup.restore();

    assert!(
        !res.contains("\"xss_executed\":true"),
        "UI-12: el onclick generado a partir del id del proveedor EJECUTA JavaScript inyectado: {}",
        res
    );
}

// ========================================================================= //
// UI-13 · web/cockpit_template.html:1479-1537 · carrera en el refresco:
//        dos `refreshHardware()` solapados => la respuesta VIEJA pisa a la
//        NUEVA y la UI queda con estado obsoleto (status dot incorrecto)
// ========================================================================= //

#[test]
fn ui13_un_refresco_antiguo_no_puede_pisar_al_reciente() {
    let _g = lock_guard();
    let port = cockpit_port();
    let res = run_node_harness(port, "race");
    println!("[UI-13] harness node (race): {}", res);
    assert!(
        res.contains("\"stale\":false"),
        "UI-13: con dos refrescos solapados la UI conserva el estado del refresco ANTIGUO: {}",
        res
    );
}

// ========================================================================= //
// UI-14 · web/cockpit_template.html:839-916 (loadProviders) · tras guardar
//        una key, el <select> de la bóveda se reinicia al primer proveedor
//        => la siguiente acción (SET DEFAULT / GUARDAR KEY) se aplica al
//        proveedor EQUIVOCADO
// ========================================================================= //

#[test]
fn ui14_guardar_una_key_no_debe_perder_el_proveedor_seleccionado() {
    let _g = lock_guard();
    let port = cockpit_port();
    let res = run_node_harness(port, "vaultsel");
    println!("[UI-14] harness node (vaultsel): {}", res);
    assert!(
        res.contains("\"selection_preserved\":true"),
        "UI-14: tras guardar, el selector de proveedor se reinició (la siguiente acción afecta al proveedor equivocado): {}",
        res
    );
}

// ========================================================================= //
// Harness de Node: ejecuta el JavaScript REAL que sirve el cockpit
// ========================================================================= //

const NODE_HARNESS: &str = r#"
const vm = require('node:vm');
const port = process.argv[2];
const mode = process.argv[3];
const base = 'http://127.0.0.1:' + port;

function makeEl(tag) {
  const t = (tag || 'div').toUpperCase();
  const el = {
    tagName: t,
    _html: '',
    htmlLog: [],
    textContent: '',
    className: '',
    value: '',
    title: '',
    src: '',
    style: {},
    children: [],
    options: [],
    classList: {
      _s: new Set(),
      add(c) { this._s.add(c); },
      remove(c) { this._s.delete(c); },
      contains(c) { return this._s.has(c); }
    },
    appendChild(c) {
      this.children.push(c);
      if (c && c.tagName === 'OPTION') {
        this.options.push(c);
        if (this.value === '') this.value = c.value;
      }
      return c;
    },
    remove() { this.removed = true; },
    addEventListener() {},
    getContext() {
      return { clearRect() {}, fillText() {}, createImageData() { return { data: [] }; }, putImageData() {}, fillStyle: '', font: '' };
    },
    set innerHTML(v) {
      this._html = v;
      this.htmlLog.push(v);
      if (this.tagName === 'SELECT') { this.options = []; this.value = ''; }
    },
    get innerHTML() { return this._html; }
  };
  return el;
}

const els = {};
function getEl(id) { if (!els[id]) els[id] = makeEl('div'); return els[id]; }
const tbodyEl = makeEl('tbody');
const document = {
  getElementById: getEl,
  createElement: (t) => makeEl(t),
  querySelector: (sel) => {
    if (sel === '#providersTable tbody') return tbodyEl;
    return makeEl('div');
  },
  querySelectorAll: () => [],
  addEventListener() {}
};

const win = {
  addEventListener() {}, removeEventListener() {},
  setTimeout: (f, t) => setTimeout(f, t), clearTimeout: (t) => clearTimeout(t),
  setInterval: () => 0, clearInterval() {},
  location: { href: base + '/' }
};
win.window = win;

async function main() {
  const html = await (await fetch(base + '/')).text();
  const tm = html.match(/window\.MEGADAEMON_TOKEN="([^"]*)"/);
  win.MEGADAEMON_TOKEN = tm ? tm[1] : '';
  const s0 = html.lastIndexOf('<script>');
  const s1 = html.lastIndexOf('</script>');
  if (s0 < 0 || s1 < 0) { console.log(JSON.stringify({ error: 'no script' })); return; }
  const script = html.slice(s0 + 8, s1);

  const sandbox = {
    window: win, document: document,
    history: { replaceState() {} },
    navigator: { clipboard: null },
    location: win.location,
    console: console,
    setTimeout: win.setTimeout, clearTimeout: win.clearTimeout,
    setInterval: win.setInterval, clearInterval: win.clearInterval,
    Promise: Promise, JSON: JSON, Math: Math, Date: Date, String: String,
    Number: Number, Boolean: Boolean, Array: Array, Object: Object,
    encodeURIComponent: encodeURIComponent, decodeURIComponent: decodeURIComponent,
    parseInt: parseInt, parseFloat: parseFloat, isNaN: isNaN,
    // node no resuelve URLs relativas contra el origen: lo hacemos nosotros.
    fetch: (url, opts) => fetch(String(url).startsWith('http') ? url : base + url, opts)
  };
  sandbox.globalThis = sandbox;
  const ctx = vm.createContext(sandbox);
  vm.runInContext(script, ctx, { filename: 'cockpit_template.html' });

  if (mode === 'xss') {
    await vm.runInContext('loadProviders()', ctx);
    const rows = tbodyEl.children.map(c => c._html || '');
    let executed = false;
    const samples = [];
    for (const row of rows) {
      const m = row.match(/onclick="([^"]*)"/);
      if (!m) continue;   // v13.2.3: el onclick inline ya no existe (data-test-provider)
      const js = m[1];
      if (js.indexOf('__XSS_MARKER') >= 0) samples.push(js);
      const probe = { __XSS_MARKER: 0, testProviderById: function () {} };
      probe.globalThis = probe;
      const pctx = vm.createContext(probe);
      try { vm.runInContext('(function(){' + js + '})()', pctx); } catch (e) {}
      if (probe.__XSS_MARKER) executed = true;
    }
    console.log(JSON.stringify({ xss_executed: executed, injected_handlers: samples, rows: rows.length }));
    return;
  }

  if (mode === 'race') {
    const pending = [];
    function fakeFetch(url) {
      const u = String(url);
      if (u.indexOf('/api/brain_status') >= 0) {
        return new Promise((resolve) => pending.push({ url: u, resolve: resolve }));
      }
      if (u.indexOf('/api/power') >= 0) {
        return Promise.resolve({ ok: true, json: async () => ({ policy: 'saver', battery_pct: 50, charging: false, allow_heavy: true, max_tokens: 100, max_turns: 3, reason: 'x', cpu_temp_c: 30 }) });
      }
      return Promise.resolve({ ok: true, json: async () => ({}) });
    }
    sandbox.fetch = fakeFetch;
    const ctx2 = vm.createContext(sandbox);
    vm.runInContext(script, ctx2, { filename: 'cockpit_template.html' });

    const p1 = vm.runInContext('refreshHardware()', ctx2);   // más ANTIGUO
    const p2 = vm.runInContext('refreshHardware()', ctx2);   // más NUEVO
    await new Promise(r => setTimeout(r, 30));
    const mk = (active) => ({ ok: true, json: async () => ({ brain_active: active, cpu_temp_c: 30, battery_temp_c: 30, cpu_freqs_mhz: [], npu_vision_backend: 'cpu', npu_offload_implemented: false, eden_dlopen: false, default_model: 'local-seere', engine: 'x' }) });
    // Resuelve primero la PETICIÓN NUEVA y después la VIEJA (orden inverso al natural).
    pending[1].resolve(mk(true));
    await new Promise(r => setTimeout(r, 10));
    pending[0].resolve(mk(false));
    await p1; await p2;
    const cls = getEl('statusDot').className;
    console.log(JSON.stringify({ status_dot_class: cls, stale: cls.indexOf('degraded') >= 0 }));
    return;
  }

  if (mode === 'vaultsel') {
    const payload = {
      default_model: 'prov-a',
      providers: [
        { id: 'prov-a', name: 'A', vendor: 'v', endpoint: 'e', model: 'm', ready: true, has_key: true, key_masked: 'a...b', tier: 'free', free_signup_url: '', auth_type: 'bearer', enabled: true, supports_reasoning: false },
        { id: 'prov-b', name: 'B', vendor: 'v', endpoint: 'e', model: 'm', ready: true, has_key: false, key_masked: '', tier: 'free', free_signup_url: '', auth_type: 'bearer', enabled: true, supports_reasoning: false }
      ],
      retired: []
    };
    sandbox.fetch = async (url, opts) => {
      if (opts && opts.method === 'POST') return { ok: true, json: async () => ({ status: 'OK' }) };
      return { ok: true, json: async () => payload };
    };
    const ctx2 = vm.createContext(sandbox);
    vm.runInContext(script, ctx2, { filename: 'cockpit_template.html' });
    await vm.runInContext('loadProviders()', ctx2);
    const before = getEl('vaultProviderId').value;
    getEl('vaultProviderId').value = 'prov-b';
    getEl('vaultApiKey').value = 'sk-PROBE';
    await vm.runInContext('saveProviderKey(false)', ctx2);
    const after = getEl('vaultProviderId').value;
    console.log(JSON.stringify({ before: before, after: after, selection_preserved: after === 'prov-b' }));
    return;
  }

  console.log(JSON.stringify({ error: 'modo desconocido' }));
}

main().catch(e => { console.log(JSON.stringify({ error: String(e && e.stack || e) })); });
"#;

/// Ejecuta el harness de Node contra el cockpit y devuelve su salida.
fn run_node_harness(port: u16, mode: &str) -> String {
    let path = std::env::temp_dir().join(format!("megadaemon_ui_harness_{}.js", mode));
    fs::write(&path, NODE_HARNESS).expect("no se pudo escribir el harness");
    let out = Command::new("node")
        .arg(&path)
        .arg(port.to_string())
        .arg(mode)
        .output();
    let _ = fs::remove_file(&path);
    match out {
        Ok(o) => {
            let so = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if so.is_empty() {
                let se = String::from_utf8_lossy(&o.stderr).trim().to_string();
                format!("{{\"error\":\"sin salida de node\",\"stderr\":\"{}\"}}", se.replace('\n', " "))
            } else {
                so
            }
        }
        Err(e) => format!("{{\"error\":\"node no disponible: {}\"}}", e),
    }
}
