//! Lecturas de memoria del proceso hookeado (punteros de argumentos).
//!
//! Todo lo que aquí se lee viene de los argumentos de una función hookeada:
//! son punteros válidos *en el caso normal*, pero un hook no puede
//! comprobarlo sin coste.  Mitigaciones aplicadas:
//!
//! * **tope de longitud**: nunca se leen más de N bytes (una cadena sin NUL
//!   no se sale del buffer);
//! * **puntero mínimo**: se descartan los punteros obviously inválidos
//!   (`< 0x1000`);
//! * **todo el acceso va con `unsafe` y está acotado**: no hay iteraciones
//!   infinitas.

use std::ptr;

/// Familias de direcciones que sabemos interpretar.
const AF_UNIX: u16 = 1;
const AF_INET: u16 = 2;
const AF_INET6: u16 = 10;

/// Lee una cadena C terminada en NUL (como mucho `max` bytes).
pub fn lee_cstr(ptr: usize, max: usize) -> Option<String> {
    if ptr < 0x1000 {
        return None;
    }
    let mut buf: Vec<u8> = Vec::with_capacity(64);
    let mut i = 0usize;
    while i < max {
        // SAFETY: se lee un byte cada vez dentro del rango `max`; si el
        // puntero no es válido, la aplicación ya estaba rota antes de
        // hookearla (mismo riesgo que el `readCString()` de los scripts JS).
        let b = unsafe { *(ptr.wrapping_add(i) as *const u8) };
        if b == 0 {
            break;
        }
        buf.push(b);
        i += 1;
    }
    if buf.is_empty() {
        return None;
    }
    Some(String::from_utf8_lossy(&buf).into_owned())
}

/// Lee un `u16` nativo en el puntero.
fn lee_u16(ptr: usize, offset: usize) -> Option<u16> {
    if ptr < 0x1000 {
        return None;
    }
    // SAFETY: lectura de 2 bytes dentro del `sockaddr` que nos pasó la app.
    let v = unsafe { ptr::read_unaligned((ptr + offset) as *const u16) };
    Some(v)
}

/// Lee `n` bytes como vector.
fn lee_bytes(ptr: usize, offset: usize, n: usize) -> Option<Vec<u8>> {
    if ptr < 0x1000 {
        return None;
    }
    // SAFETY: `n` bytes dentro de la estructura que nos pasó la app.
    let v = unsafe { std::slice::from_raw_parts((ptr + offset) as *const u8, n).to_vec() };
    Some(v)
}

/// Interpreta un `struct sockaddr` → `(host, puerto)`.
///
/// * `AF_INET`  → IPv4 y puerto (orden de red).
/// * `AF_INET6` → IPv6 y puerto (orden de red).
/// * `AF_UNIX`  → la ruta del socket (el puerto es 0).
pub fn sockaddr_destino(ptr: usize) -> Option<(String, u16)> {
    let familia = lee_u16(ptr, 0)?;
    match familia {
        AF_INET => {
            let puerto = u16::from_be(lee_u16(ptr, 2)?);
            let ip = lee_bytes(ptr, 4, 4)?;
            Some((formatea_ipv4(&ip), puerto))
        }
        AF_INET6 => {
            let puerto = u16::from_be(lee_u16(ptr, 2)?);
            let ip = lee_bytes(ptr, 8, 16)?;
            Some((formatea_ipv6(&ip), puerto))
        }
        AF_UNIX => {
            let ruta = lee_cstr(ptr + 2, 108)?;
            Some((ruta, 0))
        }
        _ => None,
    }
}

/// `1.2.3.4` a partir de 4 bytes.
fn formatea_ipv4(b: &[u8]) -> String {
    if b.len() != 4 {
        return String::new();
    }
    format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3])
}

/// IPv6 en forma completa (`2001:db8:0:0:0:0:0:1`).
fn formatea_ipv6(b: &[u8]) -> String {
    if b.len() != 16 {
        return String::new();
    }
    let mut partes: Vec<String> = Vec::with_capacity(8);
    for i in 0..8 {
        let v = u16::from_be_bytes([b[i * 2], b[i * 2 + 1]]);
        partes.push(format!("{v:x}"));
    }
    partes.join(":")
}

/// UID del proceso (para el nombre del fichero cuando el NDJSON va al
/// directorio del proceso).
pub fn uid() -> u32 {
    unsafe { libc::getuid() }
}

/// PID del proceso.
pub fn pid() -> i32 {
    unsafe { libc::getpid() }
}

/// Nombre del proceso (de `/proc/self/cmdline`).  En una app de Android es el
/// nombre del paquete (`com.agy`).
pub fn nombre_proceso() -> Option<String> {
    let datos = std::fs::read("/proc/self/cmdline").ok()?;
    let s = String::from_utf8_lossy(&datos);
    let nombre = s.split('\0').next().unwrap_or("").trim().to_string();
    if nombre.is_empty() {
        None
    } else {
        Some(nombre)
    }
}

/// Deducir el paquete de una app de Android a partir del nombre del proceso.
///
/// Se acepta sólo lo que parece un paquete (`com.agy`, `com.google.android.…`):
/// un nombre de proceso con puntos es, casi siempre, un paquete.
pub fn paquete_android() -> Option<String> {
    let n = nombre_proceso()?;
    if n.contains('.') && !n.contains(' ') && n.len() > 4 {
        Some(n)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_puntero_pequeno_se_rechaza() {
        assert!(lee_cstr(0, 16).is_none());
        assert!(lee_cstr(0x10, 16).is_none());
    }

    #[test]
    fn formatea_las_direcciones() {
        assert_eq!(formatea_ipv4(&[1, 2, 3, 4]), "1.2.3.4");
        assert_eq!(
            formatea_ipv6(&[0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]),
            "2001:db8:0:0:0:0:0:1"
        );
    }
}
