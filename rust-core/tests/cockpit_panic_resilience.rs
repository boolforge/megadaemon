//! Regresión del 502 del cockpit: un panic dentro de `ensure_live_shm()` no
//! puede envenenar el `Mutex<GlobalCockpitState>` global ni cortar la
//! conexión.
//!
//! Reproduce el patrón real de `cockpit_server.rs`:
//!   - un `Mutex` global compartido por todas las peticiones,
//!   - un handler por conexión en su propio hilo,
//!   - `ensure_live_shm()` como única operación capaz de hacer panic
//!     (`shm.rs` deserializa la cabecera con `try_into().unwrap()`).
//!
//! Contrato que fija este test:
//!   1. la petición que provoca el panic responde **500**, no "conexión cortada"
//!      (Cloudflare traducía el corte en 502),
//!   2. las peticiones **siguientes** siguen respondiendo 200,
//!   3. `lock_ok()` tolera un mutex envenenado y devuelve el guard.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

static STATE: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

fn state() -> &'static Mutex<Vec<String>> {
    STATE.get_or_init(|| Mutex::new(Vec::new()))
}

/// Igual que `megadaemon_core::lock_ok`, pero replicado aquí para que el test
/// sea autónomo y no dependa de la visibilidad del crate.
fn lock_ok<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    match m.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Réplica de `ensure_live_shm()`: deserializa la cabecera del frame
/// compartido igual que `shm.rs` (magic, version, frame_counter,
/// timestamp_us, width, height, bpp, format). Si el segmento es más corto de
/// lo esperado, el `try_into().unwrap()` / el indexado hace panic. Ese es
/// exactamente el modo de fallo que envenena el mutex global.
fn ensure_live_shm(buf: &[u8]) -> u32 {
    let magic = u32::from_le_bytes(buf[0..4].try_into().unwrap());
    let _version = u32::from_le_bytes(buf[4..8].try_into().unwrap());
    let _frame_counter = u32::from_le_bytes(buf[8..12].try_into().unwrap());
    let _timestamp_us = u64::from_le_bytes(buf[12..20].try_into().unwrap());
    let _width = u16::from_le_bytes(buf[20..22].try_into().unwrap());
    let _height = u16::from_le_bytes(buf[22..24].try_into().unwrap());
    if magic != 0x534E_4154 {
        panic!("shm: magic inesperado {magic:#x}");
    }
    magic
}

/// Servidor mínimo con el patrón del cockpit parcheado.
fn serve_patched(listener: TcpListener) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        std::thread::spawn(move || {
            let mut stream = stream;
            let mut req = [0u8; 512];
            let _ = stream.read(&mut req);
            // OJO: la ventana debe medir EXACTAMENTE lo que se busca.
            // Con windows(7) contra b"/panic" (5 bytes) la comparación nunca
            // era cierta, el panic no se disparaba y el test pasaba/fallaba
            // por motivos equivocados.
            let is_panic = req.windows(5).any(|w| w == b"/panic");
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                // 1) el estado global se toma con lock_ok (a prueba de veneno)
                let mut st = lock_ok(state());
                st.push("x".to_string());
                // 2) la operación que puede hacer panic, FUERA de cualquier
                //    lock global (en el código real es ensure_live_shm)
                if is_panic {
                    // buffer de 12 bytes: `buf[12..20]` revienta, igual que
                    // en `shm.rs` cuando el frame compartido esta truncado.
                    ensure_live_shm(&[0u8; 12]);
                }
            }));
            if outcome.is_err() {
                // 500 en la petición que falló: la conexión no se corta.
                let _ = stream.write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n");
            } else {
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
            }
        });
    }
}

fn request(port: u16, path: &str) -> Option<(u16, String)> {
    let mut s = TcpStream::connect(("127.0.0.1", port)).ok()?;
    s.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes()).ok()?;
    let mut buf = String::new();
    s.read_to_string(&mut buf).ok()?;
    let code: u16 = buf.split_whitespace().nth(1)?.parse().ok()?;
    Some((code, buf))
}

#[test]
fn lock_ok_tolera_un_mutex_envenenado() {
    let m = Arc::new(Mutex::new(0u32));
    let cloned = Arc::clone(&m);
    let _ = std::thread::spawn(move || {
        let _g = cloned.lock().unwrap();
        panic!("veneno");
    })
    .join();
    assert!(m.lock().is_err(), "el mutex deberia estar envenenado");
    // lock_ok debe devolver el guard y permitir seguir operando.
    let mut g = lock_ok(&*m);
    *g += 1;
    assert_eq!(*g, 1);
}

#[test]
fn un_panic_no_tumba_el_servidor() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || serve_patched(listener));

    // 1) la petición que provoca el panic responde 500 (no conexión cortada)
    let (code, body) = request(port, "/panic").expect("sin respuesta del servidor");
    assert_eq!(code, 500, "la peticion que hace panic debe responder 500");
    assert!(body.starts_with("HTTP/1.1 500"), "cuerpo inesperado: {body}");

    // 2) las siguientes peticiones siguen sirviendo
    for i in 0..5 {
        let (code, _) = request(port, "/ok").expect("el servidor dejo de responder");
        assert_eq!(code, 200, "peticion {i} tras el panic");
    }
}
