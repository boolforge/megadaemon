use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

fn start_server() -> u16 {
    let port = 39312u16;
    std::thread::spawn(move || {
        let _ = megadaemon_core::cockpit_server::run_server(port);
    });
    for _ in 0..100 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return port;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("server did not start");
}

fn http(port: u16, req: &str) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
    s.write_all(req.as_bytes()).unwrap();
    let mut out = Vec::new();
    let _ = s.read_to_end(&mut out);
    String::from_utf8_lossy(&out).into_owned()
}

fn rss_kb() -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    for l in s.lines() {
        if l.starts_with("VmRSS:") {
            return l.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0);
        }
    }
    0
}

#[test]
fn probe_body_alloc() {
    let port = start_server();
    let before = rss_kb();
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.write_all(b"POST /api/chat HTTP/1.1\r\nHost: x\r\nContent-Length: 104857600\r\n\r\n")
        .unwrap();
    std::thread::sleep(Duration::from_millis(600));
    let after = rss_kb();
    println!("RSS before={} KB after={} KB delta={} KB", before, after, after - before);
    drop(s);
    assert!(after - before < 20_000, "una conexión reservó {} KB", after - before);
}

#[test]
fn probe_memory_freeze_arbwrite() {
    let port = start_server();
    let victim = "/tmp/scout_victim.bin";
    let _ = std::fs::remove_file(victim);
    std::fs::write(victim, b"ORIGINAL").unwrap();
    let body = serde_json::json!({
        "action": "add", "lock_id": "scoutlock", "target": victim,
        "offset": 0, "value_hex": "41"
    })
    .to_string();
    let req = format!(
        "POST /api/memory_freeze HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(), body
    );
    let resp = http(port, &req);
    println!("FREEZE RESP: {}", resp);
    std::thread::sleep(Duration::from_millis(300));
    let content = std::fs::read(victim).unwrap();
    println!("VICTIM CONTENT: {:?}", String::from_utf8_lossy(&content));
    let _ = std::fs::remove_file(victim);
    assert_eq!(content, b"ORIGINAL", "el daemon escribió en un path arbitrario");
}

#[test]
fn probe_fake_dotprod() {
    let dir = "/data/user/0/com.agy/files/usr/lib";
    std::fs::create_dir_all(&dir).unwrap();
    let so = format!("{}/libggml-cpu.so", dir);
    std::fs::write(&so, b"NOT AN ELF FILE").unwrap();
    let tel = megadaemon_core::npu_bridge::probe_exynos_npu();
    println!(
        "kleidiai_dotprod_active={} ggml_backends={:?}",
        tel.kleidiai_dotprod_active, tel.ggml_backends
    );
    let eden = &tel.eden_runtime;
    println!(
        "eden exists={} dlopen_ok={} symbols={:?}",
        eden.exists, eden.dlopen_ok, eden.resolved_symbols
    );
    println!(
        "cpu_temp_c={} battery_temp_c={} throttle={}",
        tel.cpu_temp_c, tel.battery_temp_c, tel.throttle_state
    );
    let _ = std::fs::remove_file(&so);
    assert!(!tel.kleidiai_dotprod_active, "KleidiAI declarado activo sin librería cargable");
}

#[test]
fn probe_retired_substitution() {
    let state = "/data/user/0/com.agy/files/megadaemon/state";
    std::fs::create_dir_all(state).unwrap();
    let vault_path = format!("{}/providers.json", state);
    let backup = std::fs::read(&vault_path).ok();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for _ in 0..2 {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = vec![0u8; 65536];
            let _ = sock.read(&mut buf);
            let body = r#"{"choices":[{"message":{"content":"OK"}}]}"#;
            let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
            let _ = sock.write_all(resp.as_bytes());
        }
    });
    let vault = serde_json::json!({
        "default_model": "p1",
        "providers": [{
            "id": "p1", "name": "P1", "vendor": "V",
            "endpoint": format!("http://127.0.0.1:{}/x", port),
            "model": "m", "api_key": "KEY123456789",
            "auth_type": "bearer", "tier": "free", "enabled": true
        }]
    });
    std::fs::write(&vault_path, serde_json::to_string_pretty(&vault).unwrap()).unwrap();
    let res = megadaemon_core::providers::execute_chat("hola", Some("minimax-m2.7"), None, None, 32);
    println!("RETIRED REQUEST -> model_id={} fallback_used={} reply={:?}", res.model_id, res.fallback_used, res.reply);
    if let Some(b) = backup {
        std::fs::write(&vault_path, b).unwrap();
    } else {
        let _ = std::fs::remove_file(&vault_path);
    }
    assert!(
        res.reply.contains("RETIRADO"),
        "un modelo retirado se sustituyó en silencio por {}",
        res.model_id
    );
}

#[test]
fn probe_mask_key() {
    let state = "/data/user/0/com.agy/files/megadaemon/state";
    std::fs::create_dir_all(state).unwrap();
    let vault_path = format!("{}/providers.json", state);
    let backup = std::fs::read(&vault_path).ok();
    let vault = serde_json::json!({
        "default_model": "p1",
        "providers": [{
            "id": "p1", "name": "P1", "vendor": "V",
            "endpoint": "https://example.com/x", "model": "m",
            "api_key": "ABCDEFGHIJK", "auth_type": "bearer", "tier": "free", "enabled": true
        }]
    });
    std::fs::write(&vault_path, serde_json::to_string_pretty(&vault).unwrap()).unwrap();
    let red = megadaemon_core::providers::list_providers_redacted();
    let masked = red["providers"][0]["key_masked"].as_str().unwrap().to_string();
    println!("MASKED='{}' (clave original de 11 chars)", masked);
    if let Some(b) = backup {
        std::fs::write(&vault_path, b).unwrap();
    } else {
        let _ = std::fs::remove_file(&vault_path);
    }
    assert!(
        !masked.contains("ABCDEFG"),
        "la máscara '{}' filtra la clave",
        masked
    );
}

#[test]
fn probe_attachment_exfil() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let captured = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let cap2 = captured.clone();
    std::thread::spawn(move || {
        for _ in 0..2 {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = vec![0u8; 65536];
            let n = sock.read(&mut buf).unwrap_or(0);
            *cap2.lock().unwrap() = String::from_utf8_lossy(&buf[..n]).into_owned();
            let body = r#"{"choices":[{"message":{"content":"OK"}}]}"#;
            let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
            let _ = sock.write_all(resp.as_bytes());
        }
    });
    let state = "/data/user/0/com.agy/files/megadaemon/state";
    std::fs::create_dir_all(state).unwrap();
    let vault_path = format!("{}/providers.json", state);
    let backup = std::fs::read(&vault_path).ok();
    let vault = serde_json::json!({
        "default_model": "p1",
        "providers": [{
            "id": "p1", "name": "P1", "vendor": "V",
            "endpoint": format!("http://127.0.0.1:{}/x", port),
            "model": "m", "api_key": "KEY123456789",
            "auth_type": "bearer", "tier": "free", "enabled": true
        }]
    });
    std::fs::write(&vault_path, serde_json::to_string_pretty(&vault).unwrap()).unwrap();
    let secret_path = "/tmp/scout_secret.txt";
    std::fs::write(secret_path, "SCOUT_TOP_SECRET_ABC123\n").unwrap();
    let _ = megadaemon_core::providers::execute_chat("analiza", Some("p1"), Some(secret_path), None, 32);
    let req = captured.lock().unwrap().clone();
    println!("ATTACHMENT REQUEST:\n{}", req);
    let _ = std::fs::remove_file(secret_path);
    if let Some(b) = backup {
        std::fs::write(&vault_path, b).unwrap();
    } else {
        let _ = std::fs::remove_file(&vault_path);
    }
    assert!(!req.contains("SCOUT_TOP_SECRET"), "el contenido del fichero local se envió al proveedor");
}
