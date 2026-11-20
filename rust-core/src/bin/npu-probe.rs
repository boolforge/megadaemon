//! `npu-probe` — sonda en vivo de los ioctl VS4L del driver NPU (Exynos 2100).
//!
//! Uso:
//!
//! ```text
//! npu-probe --vs4l [--device /dev/vertex10] [--deep] [--pretty]
//! ```
//!
//! Por qué el sondeo **sin argumento** (y no "con buffer"):
//!
//! * Muchos drivers de ioctl, al recibir la llamada **sin puntero**,
//!   devuelven **el tamaño de datos que esperan** en vez de ejecutar la
//!   operación. En el driver VS4L del móvil eso es exactamente lo que pasa:
//!   `VSL_QUERY_CAP` → 32, `VSL_CONFIG_1/2` → 16, `VSL_SET_6/7/8` → 128,
//!   `VSL_CONTROL_10` → 8.
//! * Ese valor **coincide exacto con el campo `size` de `_IOC`** ⇒ el `size`
//!   no es *advisory*: es parte del contrato del driver.
//! * Por tanto el sondeo sin argumento **es el test de existencia**:
//!   retorno `> 0` ⇒ el ioctl existe; `EINVAL` (22) ⇒ el driver no lo
//!   despacha (no existe).
//!
//! ⚠️ `EINVAL` **no** significa "no existe" cuando se sondea *con* buffer:
//!   significa "reconocido pero argumento inválido". Por eso la forma con
//!   buffer (`--deep`) es **opcional y está desactivada por defecto**: los
//!   ioctl de tipo `SET` alterarían el estado del NPU.
//!
//! La tabla canónica de ioctls vive en `crate::dex_decompile::VS4L_IOCTLS`
//! (medida en vivo sobre el Samsung). Este binario sólo la **lee**: si algún
//! día se mueve de módulo, hay que actualizar el `use` de aquí.
//!
//! Salida: **JSON por stdout** (para que lo consuma la flota), resumen legible
//! por stderr.


/// `libc::ioctl` pide el *request* como `c_ulong` en x86/x86_64 y como
/// `c_int` en aarch64-android => sin esto el binario **no compila para
/// ARM64** (E0308: expected i32, found u64).
macro_rules! ioctl_req {
    ($code:expr) => {{
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        { $code as libc::c_ulong }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        { $code as libc::c_int }
    }};
}

use megadaemon_core::dex_decompile::{VS4L_IOCTLS, VS4L_LIVE_CODES};
use serde_json::{json, Value};
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;

// errno de Linux que nos importan.
const EACCES: i32 = 13;
const EFAULT: i32 = 14;
const EINVAL: i32 = 22;
const ENODEV: i32 = 19;
const ENOTTY: i32 = 25;

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 0;
    while i < args.len() {
        if args[i] == name && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
        if let Some(v) = args[i].strip_prefix(&format!("{}=", name)) {
            return Some(v.to_string());
        }
        i += 1;
    }
    None
}

fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}

/// Dispositivos candidatos: el nodo del NPU del Exynos y los nodos V4L2.
fn candidates() -> Vec<String> {
    let mut v = vec!["/dev/vertex10".to_string()];
    for i in 0..12 {
        v.push(format!("/dev/video{i}"));
    }
    v
}

/// Abre el dispositivo para lectura (y escritura si se puede: los ioctl de
/// tipo `SET` la necesitan, pero por defecto no los tocamos).
fn open_device(path: &str) -> Option<std::fs::File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .or_else(|_| OpenOptions::new().read(true).open(path))
        .ok()
}

/// ioctl **sin argumento**: el driver devuelve el tamaño que espera.
fn ioctl_no_arg(fd: i32, code: u32) -> i32 {
    unsafe { libc::ioctl(fd, ioctl_req!(code), 0usize) }
}

/// ioctl **con buffer** del tamaño declarado (sólo con `--deep`).
fn ioctl_with_buf(fd: i32, code: u32, size: usize) -> (i32, i32) {
    let mut buf = vec![0u8; size.max(1)];
    let ret = unsafe { libc::ioctl(fd, ioctl_req!(code), buf.as_mut_ptr()) };
    let errno = if ret < 0 {
        std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
    } else {
        0
    };
    (ret, errno)
}

/// Veredicto de un ioctl sondeado **sin** argumento.
fn verdict_no_arg(ret: i32, errno: i32) -> (&'static str, bool) {
    if ret >= 0 {
        return ("existe", true);
    }
    match errno {
        EINVAL => ("no_existe", false),
        ENOTTY => ("no_existe", false),
        EFAULT => ("reconocido", true),
        ENODEV | EACCES => ("error_dispositivo", false),
        _ => ("error", false),
    }
}

fn errno_name(e: i32) -> Option<&'static str> {
    match e {
        0 => None,
        EACCES => Some("EACCES"),
        EFAULT => Some("EFAULT"),
        EINVAL => Some("EINVAL"),
        ENODEV => Some("ENODEV"),
        ENOTTY => Some("ENOTTY"),
        _ => None,
    }
}

fn main() -> std::process::ExitCode {
    if !flag("--vs4l") {
        eprintln!("uso: npu-probe --vs4l [--device <ruta>] [--deep] [--pretty]");
        eprintln!("  --device <ruta>  nodo del dispositivo (por defecto: /dev/vertex10 y /dev/video*)");
        eprintln!("  --deep           además sondea con buffer del tamaño declarado (ALTERA el estado)");
        eprintln!("  --pretty         JSON indentado");
        return std::process::ExitCode::from(2);
    }

    let deep = flag("--deep");
    let pretty = flag("--pretty");
    let explicitos: Vec<String> = arg("--device").map(|d| vec![d]).unwrap_or_default();
    let rutas = if explicitos.is_empty() {
        candidates()
    } else {
        explicitos
    };

    // Primer dispositivo que se pueda abrir.
    let mut abierto: Option<(String, std::fs::File)> = None;
    for ruta in &rutas {
        if let Some(f) = open_device(&ruta) {
            abierto = Some((ruta.to_string(), f));
            break;
        }
    }

    let (device, fd) = match abierto {
        Some((ruta, f)) => (ruta, f.as_raw_fd()),
        None => {
            let json = json!({
                "ok": false,
                "error": "no_device",
                "detail": format!("no se pudo abrir ninguno de: {}", rutas.join(", ")),
                "hint": "hace falta root y que /dev/vertex10 exista (driver VS4L del Exynos)",
            });
            println!("{}", json);
            return std::process::ExitCode::from(1);
        }
    };

    let mut entries: Vec<Value> = Vec::new();
    let mut live = 0u32;

    for (name, code, nr, size, _desc) in VS4L_IOCTLS.iter() {
        let ret = ioctl_no_arg(fd, *code);
        let errno = if ret < 0 {
            std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
        } else {
            0
        };
        let (verdict, is_live) = verdict_no_arg(ret, errno);
        if is_live {
            live += 1;
        }
        let mut entry = json!({
            "name": name,
            "code": format!("0x{:08x}", code),
            "code_dec": code,
            "nr": nr,
            "declared_size": size,
            "live": is_live,
            "verdict": verdict,
            "driver_size": if ret >= 0 { Value::from(ret) } else { Value::Null },
            "size_match": if ret >= 0 { Value::from(ret as u16 == *size) } else { Value::Null },
            "errno": errno_name(errno),
        });
        if deep {
            let (bret, berrno) = ioctl_with_buf(fd, *code, *size as usize);
            let bobj = json!({
                "ret": bret,
                "errno": errno_name(berrno),
                "ok": bret >= 0,
            });
            if let Some(o) = entry.as_object_mut() {
                o.insert("with_buffer".to_string(), bobj);
            }
        }
        entries.push(entry);
    }

    let out = json!({
        "ok": true,
        "tool": "npu-probe",
        "mode": if deep { "no_arg+buffer" } else { "no_arg" },
        "device": device,
        "live_count": live,
        "expected_live": VS4L_LIVE_CODES,
        "matches_table": live == VS4L_LIVE_CODES,
        "entries": entries,
    });

    let texto = if pretty {
        serde_json::to_string_pretty(&out).unwrap_or_else(|_| out.to_string())
    } else {
        out.to_string()
    };
    println!("{texto}");
    eprintln!(
        "npu-probe: {device} => {live}/{} ioctl vivos (tabla: {})",
        VS4L_LIVE_CODES,
        if live == VS4L_LIVE_CODES { "coincide" } else { "NO coincide" }
    );

    std::process::ExitCode::from(if live == VS4L_LIVE_CODES { 0 } else { 1 })
}
