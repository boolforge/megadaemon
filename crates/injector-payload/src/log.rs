//! Escritor **NDJSON** del payload: una línea por evento observado.
//!
//! Por qué NDJSON y no JSON: se **añade** (`O_APPEND`) línea a línea, así que
//! el daemon puede leerlo con `tail` mientras el proceso sigue vivo, y una
//! línea rota no invalida lo anterior.
//!
//! Por qué hay **varias** rutas candidatas: el payload corre **dentro** de una
//! app de terceros, con el uid de esa app.  `/data/local/tmp` tiene etiqueta
//! `shell_data_file` y `untrusted_app` **no puede escribir** ahí ⇒ el
//! `open()` falla con `EACCES` y hay que caer al directorio privado de la app
//! (`/data/data/<paquete>/files`), que sí es `app_data_file`.  El daemon (root)
//! lee las dos.
//!
//! La ruta que **sí** se pudo abrir se escribe en la primera línea del NDJSON
//! (campo `"f"`), así que el operador no tiene que adivinarla.

use megadaemon_redact::redactar;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;

/// Descriptor del NDJSON: `0` = sin abrir, `-1` = no se pudo abrir, `>0` = fd.
static FD: AtomicI32 = AtomicI32::new(0);

/// Ruta en uso (para ponerla en cada línea).
static RUTA: Mutex<Option<String>> = Mutex::new(None);

/// Directorio y fichero "de referencia" (los que usa el daemon).
pub const DIR_REFERENCIA: &str = "/data/local/tmp/megadaemon/state";
pub const FICHERO_REFERENCIA: &str = "/data/local/tmp/megadaemon/state/hooks.ndjson";

/// Devuelve la lista de rutas candidatas, en orden de preferencia.
pub fn rutas_candidatas() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    // 1) Override por entorno (lo usa el daemon si lo quiere).
    if let Ok(p) = std::env::var("MEGADAEMON_HOOKS_PATH") {
        if !p.trim().is_empty() {
            out.push(p.trim().to_string());
        }
    }
    // 2) La ruta de referencia (funciona si el proceso es root/shell).
    out.push(FICHERO_REFERENCIA.to_string());
    // 3) El directorio privado de la app (el que de verdad funciona dentro
    //    de una app de terceros).
    if let Some(paquete) = paquete_actual() {
        out.push(format!("/data/data/{}/files/megadaemon/hooks.ndjson", paquete));
        out.push(format!("/data/user/0/{}/files/megadaemon/hooks.ndjson", paquete));
    }
    out
}

/// Nombre del paquete/proceso actual (lo que hay en `/proc/self/cmdline`).
pub fn paquete_actual() -> Option<String> {
    let b = std::fs::read("/proc/self/cmdline").ok()?;
    let fin = b.iter().position(|c| *c == 0).unwrap_or(b.len());
    if fin == 0 {
        return None;
    }
    let s = String::from_utf8_lossy(&b[..fin]).to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Crea el directorio del NDJSON (mkdir -p, ignorando "ya existe").
fn mkdir_p(ruta: &str) -> bool {
    let mut acumulado = String::new();
    for parte in ruta.split('/') {
        if parte.is_empty() {
            acumulado.push('/');
            continue;
        }
        if !acumulado.is_empty() && !acumulado.ends_with('/') {
            acumulado.push('/');
        }
        acumulado.push_str(parte);
        let c = match std::ffi::CString::new(acumulado.clone()) {
            Ok(c) => c,
            Err(_) => return false,
        };
        // 0700: el NDJSON puede contener fragmentos de consultas.
        let r = unsafe { libc::mkdir(c.as_ptr(), 0o700) };
        if r != 0 {
            let e = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
            if e != libc::EEXIST {
                return false;
            }
        }
    }
    true
}

/// Abre el NDJSON en modo "añadir".
fn abrir(ruta: &str) -> Option<i32> {
    let dir = match ruta.rfind('/') {
        Some(i) => &ruta[..i],
        None => return None,
    };
    if !mkdir_p(dir) {
        return None;
    }
    let c = match std::ffi::CString::new(ruta) {
        Ok(c) => c,
        Err(_) => return None,
    };
    // `openat` por syscall directo: si usáramos `open()` entraríamos en
    // nuestro propio hook (que registraria su propia escritura: bucle).
    let flags = libc::O_WRONLY | libc::O_CREAT | libc::O_APPEND | libc::O_CLOEXEC;
    let fd = unsafe {
        libc::syscall(
            libc::SYS_openat,
            libc::AT_FDCWD,
            c.as_ptr(),
            flags,
            0o600 as libc::c_uint,
        )
    };
    if fd < 0 {
        return None;
    }
    Some(fd as i32)
}

/// El descriptor del NDJSON (abre la primera ruta que funcione).
fn descriptor() -> i32 {
    let actual = FD.load(Ordering::Acquire);
    if actual != 0 {
        return actual;
    }
    for ruta in rutas_candidatas() {
        if let Some(f) = abrir(&ruta) {
            match FD.compare_exchange(0, f, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => {
                    if let Ok(mut g) = RUTA.lock() {
                        *g = Some(ruta);
                    }
                    return f;
                }
                Err(v) => {
                    unsafe {
                        libc::close(f);
                    }
                    return v;
                }
            }
        }
    }
    FD.store(-1, Ordering::Release);
    -1
}

/// Ruta del NDJSON en uso (o la de referencia si aún no se ha abierto).
pub fn ruta_actual() -> String {
    if let Ok(g) = RUTA.lock() {
        if let Some(r) = g.as_ref() {
            return r.clone();
        }
    }
    FICHERO_REFERENCIA.to_string()
}

/// Escapa una cadena para JSON.
pub fn escapar(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Marca de tiempo en milisegundos (reloj de pared).
fn ahora_ms() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe {
        libc::clock_gettime(libc::CLOCK_REALTIME, &mut ts);
    }
    (ts.tv_sec as u64) * 1000 + (ts.tv_nsec as u64) / 1_000_000
}

/// ID del hilo (para distinguir hilos en el NDJSON).
fn tid() -> i64 {
    unsafe { libc::syscall(libc::SYS_gettid) as i64 }
}

/// Escribe un evento en el NDJSON.
///
/// El `detalle` se **redacta** con la misma política que el shim del daemon
/// (`megadaemon-redact`): una credencial no puede salir al bus.
pub fn evento(hook: &str, tipo: &str, detalle: Option<&str>) {
    let fd = descriptor();
    if fd <= 0 {
        return;
    }
    let det = match detalle {
        Some(d) => {
            let r = redactar(d);
            escapar(&r)
        }
        None => String::new(),
    };
    let linea = format!(
        "{{\"ts\":{},\"pid\":{},\"tid\":{},\"hook\":\"{}\",\"tipo\":\"{}\",\"detalle\":\"{}\",\"f\":\"{}\"}}\n",
        ahora_ms(),
        std::process::id(),
        tid(),
        escapar(hook),
        escapar(tipo),
        det,
        escapar(&ruta_actual()),
    );
    unsafe {
        // Una sola llamada a `write`: con O_APPEND la línea entra entera (no
        // se parte en dos escrituras de otro proceso).
        libc::write(fd, linea.as_ptr() as *const libc::c_void, linea.len());
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn escapa_el_json() {
        assert_eq!(escapar("a\"b"), "a\\\"b");
        assert_eq!(escapar("a\nb"), "a\\nb");
        assert_eq!(escapar("a\\b"), "a\\\\b");
    }

    #[test]
    fn las_rutas_candidatas_empiezan_por_la_de_referencia() {
        let r = rutas_candidatas();
        assert!(!r.is_empty());
        assert_eq!(r[0], FICHERO_REFERENCIA);
    }
}
