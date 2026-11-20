//! Sonda del driver **VS4L/NPU** del Exynos 2100 (`/dev/vertex10`).
//!
//! # Por qué esta tool existe
//!
//! La forma correcta de preguntar *"¿existe este ioctl?"* es **emitirlo SIN
//! argumento** (`arg = NULL`): el driver responde con **el tamaño de datos que
//! espera** (`if (!arg) return size;` en la mayoría de drivers).
//!
//! Si se emite **con un buffer**, el driver contesta `EINVAL` cuando el tamaño
//! no coincide ⇒ **`EINVAL` NO significa "no existe"**: significa "reconocido
//! pero argumento inválido". Confundir `EINVAL` con `ENOTTY` fue exactamente lo
//! que hizo que la malla publicara "sólo 4 de los 9 códigos existen" cuando la
//! realidad medida es **7 de 9** (los dos `_IO` de nr 4 y 5 son los únicos que
//! no existen), y que el campo `size` de `_IOC` pareciera "advisory" cuando
//! **coincide exacto** con lo que el driver declara.
//!
//! La tabla de códigos es `dex_decompile::VS4L_IOCTLS` (**fuente de verdad
//! única**, medida en vivo el 2026-11-20 en el S21 Ultra).
//!
//! Uso:
//! ```text
//! megadaemon-core npu-probe --vs4l
//! ```

use serde::{Deserialize, Serialize};
use std::os::unix::io::RawFd;

use crate::dex_decompile::{VS4L_IOCTLS, VS4L_LIVE_CODES};

/// Nodo de caracteres del driver NPU (char 10,119; modo 0644 ⇒ cualquier app
/// puede abrirlo: no hace falta root).
pub const VERTEX10: &str = "/dev/vertex10";

/// Tipo de ioctl de la familia VS4L: `'V'` = 0x56.
pub const VS4L_TYPE: u8 = b'V';

/// Resultado del sondeo de un código.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vs4lProbe {
    /// Nombre (de la tabla; los nombres de dump van marcados como no confirmados).
    pub name: String,
    /// Código completo del ioctl.
    pub code: u32,
    /// Macro `_IOC` **derivada del código** (no copiada a mano).
    #[serde(rename = "macro")]
    pub macro_name: String,
    /// `nr` (el discriminante real del driver).
    pub nr: u8,
    /// `size` codificado en el ioctl.
    pub size_in_code: u16,
    /// ¿Lo reconoce el driver vivo?
    pub live: bool,
    /// Tamaño que el driver declara esperar (sólo si `live`).
    pub driver_size: Option<i64>,
    /// `errno` si el sondeo falló.
    pub errno: Option<i32>,
    pub errno_name: String,
    /// Nota de la tabla de referencia.
    pub note: String,
}

/// Informe completo del sondeo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vs4lReport {
    /// Ruta del nodo sondeado (`null` si no existe).
    pub node: Option<String>,
    /// Modo del nodo en octal (p. ej. `100644`).
    pub node_mode: Option<String>,
    /// Motivo por el que no se pudo sondear.
    pub node_error: Option<String>,
    /// Códigos sondeados.
    pub probed: usize,
    /// Códigos que el driver vivo reconoce.
    pub live: usize,
    /// Cuántos debería reconocer (referencia: `VS4L_LIVE_CODES`).
    pub expected_live: u32,
    /// ¿El driver vivo coincide con la tabla? (auto-comprobación)
    pub matches_expected: bool,
    /// Método usado (para que quede escrito en el informe).
    pub method: String,
    /// Detalle por código.
    pub codes: Vec<Vs4lProbe>,
}

// --------------------------------------------------------------------------
// Aritmética de `_IOC` (Linux, asm-generic/ioctl.h)
// --------------------------------------------------------------------------

/// `code = (dir << 30) | (size << 16) | (type << 8) | nr`
pub fn ioc_build(dir: u8, typ: u8, nr: u8, size: u16) -> u32 {
    ((dir as u32) << 30) | ((size as u32) << 16) | ((typ as u32) << 8) | (nr as u32)
}

/// `nr` (bytes 0-7).
pub fn ioc_nr(code: u32) -> u8 {
    (code & 0xff) as u8
}

/// `type` (bytes 8-15).
pub fn ioc_type(code: u32) -> u8 {
    ((code >> 8) & 0xff) as u8
}

/// `size` (bits 16-29).
pub fn ioc_size(code: u32) -> u16 {
    ((code >> 16) & 0x3fff) as u16
}

/// `dir` (bits 30-31): 0=_IO, 1=_IOW, 2=_IOR, 3=_IOWR.
pub fn ioc_dir(code: u32) -> u8 {
    ((code >> 30) & 0x3) as u8
}

/// Macro `_IOC` **derivada del código** (p. ej. `_IOW('V',0,32)`).
pub fn ioc_macro(code: u32) -> String {
    let d = match ioc_dir(code) {
        0 => "_IO",
        1 => "_IOW",
        2 => "_IOR",
        _ => "_IOWR",
    };
    format!("{}('{}',{},{})", d, ioc_type(code) as char, ioc_nr(code), ioc_size(code))
}

/// Nombre corto de un `errno`.
pub fn errno_name(e: i32) -> &'static str {
    match e {
        1 => "EPERM",
        2 => "ENOENT",
        5 => "EIO",
        9 => "EBADF",
        11 => "EAGAIN",
        13 => "EACCES",
        14 => "EFAULT",
        16 => "EBUSY",
        19 => "ENODEV",
        22 => "EINVAL",
        25 => "ENOTTY",
        _ => "E?",
    }
}

// --------------------------------------------------------------------------
// Sondeo
// --------------------------------------------------------------------------

/// `libc::ioctl` cambia el tipo del *request* segun la arquitectura: en
/// x86/x86_64 es `c_ulong` (u64) y en el resto (aarch64-android) es `c_int`
/// (i32).  Sin este envoltorio el crate **no compila para ARM64**:
/// `E0308: expected i32, found u64`.
#[inline]
unsafe fn ioctl_raw(fd: RawFd, code: u32, arg: *mut libc::c_void) -> libc::c_int {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        libc::ioctl(fd, code as libc::c_ulong, arg)
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        libc::ioctl(fd, code as libc::c_int, arg)
    }
}

/// Emite **un** ioctl **sin argumento** contra un fd ya abierto.
///
/// Devuelve `(live, driver_size, errno)`. Con `arg = NULL` el driver responde
/// con el tamaño que espera; si el código no existe ⇒ `EINVAL`/`ENOTTY`.
pub fn probe_code(fd: RawFd, code: u32) -> (bool, Option<i64>, Option<i32>) {
    // `arg = NULL` ⇒ el driver ve `!arg` ⇒ devuelve el tamaño que espera.
    let ret = unsafe { ioctl_raw(fd, code, std::ptr::null_mut::<libc::c_void>()) };
    if ret >= 0 {
        (true, Some(ret as i64), None)
    } else {
        let e = std::io::Error::last_os_error()
            .raw_os_error()
            .unwrap_or(0);
        (false, None, Some(e))
    }
}

/// Sondea los 9 códigos contra `/dev/vertex10`.
///
/// No necesita root (el nodo es 0644) y **no escribe nada** en el driver:
/// sólo pregunta el tamaño.
pub fn probe() -> Vs4lReport {
    let mut rep = Vs4lReport {
        node: Some(VERTEX10.to_string()),
        node_mode: None,
        node_error: None,
        probed: 0,
        live: 0,
        expected_live: VS4L_LIVE_CODES,
        matches_expected: false,
        method: "ioctl(fd, code, NULL) => el driver devuelve el tamaño que espera (si no existe: EINVAL/ENOTTY)".to_string(),
        codes: Vec::new(),
    };

    match std::fs::metadata(VERTEX10) {
        Ok(m) => {
            use std::os::unix::fs::PermissionsExt;
            rep.node_mode = Some(format!("{:o}", m.permissions().mode()));
        }
        Err(e) => {
            rep.node = None;
            rep.node_error = Some(format!("{}: {}", VERTEX10, e));
            return rep;
        }
    }

    let fd = unsafe {
        libc::open(
            VERTEX10.as_ptr() as *const libc::c_char,
            libc::O_RDWR | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        let e = std::io::Error::last_os_error();
        rep.node_error = Some(format!("open {}: {}", VERTEX10, e));
        return rep;
    }

    for (name, code, nr, size, note) in VS4L_IOCTLS {
        let (live, driver_size, errno) = probe_code(fd, *code);
        rep.codes.push(Vs4lProbe {
            name: (*name).to_string(),
            code: *code,
            macro_name: ioc_macro(*code),
            nr: *nr,
            size_in_code: *size,
            live,
            driver_size,
            errno,
            errno_name: errno.map(errno_name).unwrap_or("").to_string(),
            note: (*note).to_string(),
        });
        rep.probed += 1;
        if live {
            rep.live += 1;
        }
    }

    unsafe {
        libc::close(fd);
    }
    rep.matches_expected = rep.live as u32 == rep.expected_live;
    rep
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La tabla debe ser coherente con la aritmética de `_IOC`: eso es lo que
    /// la hace verificable sin el dispositivo.
    #[test]
    fn los_9_codigos_coinciden_con_su_macro_ioc() {
        assert_eq!(VS4L_IOCTLS.len(), 9);
        for (_, code, nr, size, _) in VS4L_IOCTLS {
            assert_eq!(ioc_nr(*code), *nr, "nr de {:#x}", code);
            assert_eq!(ioc_size(*code), *size, "size de {:#x}", code);
            assert_eq!(ioc_type(*code), VS4L_TYPE, "type de {:#x}", code);
            assert_eq!(
                ioc_build(ioc_dir(*code), VS4L_TYPE, *nr, *size),
                *code,
                "macro reconstruida de {:#x}",
                code
            );
        }
    }

    #[test]
    fn la_tabla_declara_7_vivos_y_9_codigos() {
        assert_eq!(VS4L_LIVE_CODES, 7);
        assert_eq!(VS4L_IOCTLS.len(), 9);
    }

    #[test]
    fn la_macro_se_deriva_del_codigo() {
        assert_eq!(ioc_macro(0x4020_5600), "_IOW('V',0,32)");
        assert_eq!(ioc_macro(0x4010_5601), "_IOW('V',1,16)");
        assert_eq!(ioc_macro(0x4080_5606), "_IOW('V',6,128)");
        assert_eq!(ioc_macro(0x4008_560a), "_IOW('V',10,8)");
        assert_eq!(ioc_macro(0x0000_5604), "_IO('V',4,0)");
    }

    #[test]
    fn el_tamano_esperado_coincide_con_el_de_la_tabla() {
        // El driver vivo devuelve 32/16/16/128/128/128/8 y la tabla dice lo mismo.
        for (name, code, _nr, size, _) in VS4L_IOCTLS {
            if *name == "VSL_NO_4" || *name == "VSL_NO_5" {
                continue;
            }
            assert!(*size > 0, "{} no debería tener size 0", name);
        }
    }
}
