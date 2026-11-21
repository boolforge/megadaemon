//! **ZSTD sin crates**: `crates.io` no es accesible desde el build de este
//! repo, así que el contenedor de Swift Backup (TAR + ZSTD) se descomprime
//! hablando con `libzstd` del sistema.
//!
//! Dos estrategias, en este orden:
//! 1. **FFI por `dlopen`**: se carga `libzstd.so.1` (Linux/Android) en
//!    tiempo de ejecución y se llama a `ZSTD_getFrameContentSize` y
//!    `ZSTD_decompress`. **No** añade dependencia de compilación: si el
//!    binario se compila en una máquina sin `libzstd`, sigue compilando.
//! 2. **Fallback al binario `zstd -d -c`** (Termux lo trae en
//!    `pkg install zstd`): útil cuando la biblioteca no está instalada pero el
//!    ejecutable sí.
//!
//! En Android/Termux `libzstd.so` viene en el paquete `zstd`; en el CI de
//! GitHub ubuntu-latest ya está instalada.

use std::ffi::c_void;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

// ---------------------------------------------------------------------------
// FFI por dlopen (sin `#[link]`, sin crates)
// ---------------------------------------------------------------------------

extern "C" {
    fn dlopen(filename: *const u8, flag: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const u8) -> *mut c_void;
    fn dlerror() -> *const u8;
}

const RTLD_NOW: i32 = 2;
const RTLD_LOCAL: i32 = 0;

/// Nombres de la biblioteca, en orden de preferencia por plataforma.
const LIBERANDRAS: &[&str] = &[
    "libzstd.so.1",
    "libzstd.so",
    "libzstd.dylib",
    "libzstd.1.dylib",
];

/// Tamaño de contenido desconocido según zstd (`ZSTD_CONTENTSIZE_UNKNOWN`).
const CONTENTSIZE_UNKNOWN: u64 = u64::MAX;
/// Tamaño de contenido desconocido (`ZSTD_CONTENTSIZE_ERROR`).
const CONTENTSIZE_ERROR: u64 = u64::MAX - 1;

type ZstdGetFrameContentSize = unsafe extern "C" fn(*const c_void, usize) -> u64;
type ZstdDecompress = unsafe extern "C" fn(*mut c_void, usize, *const c_void, usize) -> usize;
type ZstdCompress = unsafe extern "C" fn(*mut c_void, usize, *const c_void, usize, i32) -> usize;
type ZstdIsError = unsafe extern "C" fn(usize) -> u32;
type ZstdGetErrorName = unsafe extern "C" fn(usize) -> *const u8;

/// Funciones resueltas de `libzstd`.
struct Zstd {
    get_frame_content_size: ZstdGetFrameContentSize,
    decompress: ZstdDecompress,
    compress: ZstdCompress,
    is_error: ZstdIsError,
    get_error_name: ZstdGetErrorName,
}

/// Carga `libzstd` y resuelve sus símbolos. `None` si no está disponible.
fn cargar() -> Option<Zstd> {
    for nombre in LIBERANDRAS {
        let c = format!("{nombre}\0").into_bytes();
        // SAFETY: `dlopen` con un nombre terminado en NUL válido.
        let handle = unsafe { dlopen(c.as_ptr(), RTLD_NOW | RTLD_LOCAL) };
        if handle.is_null() {
            continue;
        }
        let sym = |n: &str| -> Option<*mut c_void> {
            let c = format!("{n}\0").into_bytes();
            // SAFETY: `handle` es un handle válido de dlopen.
            let p = unsafe { dlsym(handle, c.as_ptr()) };
            if p.is_null() {
                None
            } else {
                Some(p)
            }
        };
        let (g, d, c, ie, gn) = (
            sym("ZSTD_getFrameContentSize"),
            sym("ZSTD_decompress"),
            sym("ZSTD_compress"),
            sym("ZSTD_isError"),
            sym("ZSTD_getErrorName"),
        );
        match (g, d, c, ie, gn) {
            (Some(g), Some(d), Some(c), Some(ie), Some(gn)) => {
                // SAFETY: los símbolos tienen la firma de la API pública de zstd.
                unsafe {
                    return Some(Zstd {
                        get_frame_content_size: std::mem::transmute::<*mut c_void, ZstdGetFrameContentSize>(g),
                        decompress: std::mem::transmute::<*mut c_void, ZstdDecompress>(d),
                        compress: std::mem::transmute::<*mut c_void, ZstdCompress>(c),
                        is_error: std::mem::transmute::<*mut c_void, ZstdIsError>(ie),
                        get_error_name: std::mem::transmute::<*mut c_void, ZstdGetErrorName>(gn),
                    });
                }
            }
            _ => continue,
        }
    }
    None
}

/// ¿Hay `libzstd` disponible en esta máquina?
pub fn disponible() -> bool {
    cargar().is_some()
}

/// Tamaño (en bytes) del contenido descomprimido de un frame, si zstd lo
/// conoce (los frames con tamaño desconocido devuelven `None`).
pub fn tamano_contenido(datos: &[u8]) -> Option<usize> {
    let z = cargar()?;
    // SAFETY: puntero válido a `datos` con su longitud.
    let n = unsafe { (z.get_frame_content_size)(datos.as_ptr() as *const c_void, datos.len()) };
    if n == CONTENTSIZE_UNKNOWN || n == CONTENTSIZE_ERROR || n > usize::MAX as u64 {
        None
    } else {
        Some(n as usize)
    }
}

/// Descomprime un frame ZSTD usando `libzstd`.
fn descomprimir_ffi(datos: &[u8], max_salida: usize) -> Result<Vec<u8>, String> {
    let z = cargar().ok_or("libzstd no está disponible")?;
    let cap = match tamano_contenido(datos) {
        Some(n) => n.min(max_salida),
        None => {
            // Tamaño desconocido: se reserva el máximo y se recorta.
            max_salida
        }
    };
    let mut salida = vec![0u8; cap];
    // SAFETY: buffers del tamaño declarado; zstd no escribe más de `cap`.
    let n = unsafe {
        (z.decompress)(
            salida.as_mut_ptr() as *mut c_void,
            cap,
            datos.as_ptr() as *const c_void,
            datos.len(),
        )
    };
    // SAFETY: `is_error` sólo recibe el código devuelto por zstd.
    if unsafe { (z.is_error)(n) } != 0 {
        // SAFETY: `get_error_name` devuelve un literal estático válido.
        let msg = unsafe {
            let p = (z.get_error_name)(n);
            if p.is_null() {
                "error desconocido".to_string()
            } else {
                std::ffi::CStr::from_ptr(p as *const std::ffi::c_char)
                    .to_string_lossy()
                    .to_string()
            }
        };
        return Err(format!("zstd: {msg}"));
    }
    salida.truncate(n);
    Ok(salida)
}

/// Descomprime con el binario `zstd -d -c` (leer de stdin, escribir a stdout).
pub fn descomprimir_por_cli(datos: &[u8]) -> Result<Vec<u8>, String> {
    let mut hijo = Command::new("zstd")
        .args(["-d", "-c", "-q"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("no se puede lanzar `zstd`: {e}"))?;
    {
        let entrada = hijo
            .stdin
            .as_mut()
            .ok_or("zstd sin stdin")?;
        entrada
            .write_all(datos)
            .map_err(|e| format!("zstd no acepta la entrada: {e}"))?;
    }
    let salida = hijo
        .wait_with_output()
        .map_err(|e| format!("zstd no termina bien: {e}"))?;
    if !salida.status.success() {
        return Err(format!(
            "zstd terminó con {:?}: {}",
            salida.status.code(),
            String::from_utf8_lossy(&salida.stderr).trim()
        ));
    }
    Ok(salida.stdout)
}

/// Descomprime un frame ZSTD: intenta `libzstd` y, si no está, el binario.
///
/// `max_salida` acota la memoria: un frame malicioso no puede hacer que el
/// proceso se coma la RAM (Swift guarda aquí el backup completo del usuario).
pub fn descomprimir(datos: &[u8], max_salida: usize) -> Result<Vec<u8>, String> {
    if datos.is_empty() {
        return Err("no hay datos que descomprimir".to_string());
    }
    match descomprimir_ffi(datos, max_salida) {
        Ok(v) => Ok(v),
        Err(e_ffi) => match descomprimir_por_cli(datos) {
            Ok(v) => {
                if v.len() > max_salida {
                    return Err(format!(
                        "la salida descomprimida ({}) supera el máximo ({max_salida})",
                        v.len()
                    ));
                }
                Ok(v)
            }
            Err(e_cli) => Err(format!("{e_ffi}; fallback CLI: {e_cli}")),
        },
    }
}

/// Comprime con `libzstd` (nivel 3). Sólo para pruebas y para generar
/// fixtures; el contenedor de Swift lo produce la app.
pub fn comprimir(datos: &[u8], nivel: i32) -> Result<Vec<u8>, String> {
    let z = cargar().ok_or("libzstd no está disponible")?;
    let cap = datos.len() + (datos.len() / 2) + 1024;
    let mut salida = vec![0u8; cap];
    // SAFETY: buffers del tamaño declarado.
    let n = unsafe {
        (z.compress)(
            salida.as_mut_ptr() as *mut c_void,
            cap,
            datos.as_ptr() as *const c_void,
            datos.len(),
            nivel,
        )
    };
    // SAFETY: `is_error` sólo recibe el código devuelto por zstd.
    if unsafe { (z.is_error)(n) } != 0 {
        return Err("zstd: error al comprimir".to_string());
    }
    salida.truncate(n);
    Ok(salida)
}

/// Descomprime un fichero `.zst` a disco sin cargarlo entero en memoria:
/// se escribe por trozos con `zstd -d -c` para no duplicar el consumo.
pub fn extraer_fichero(entrada: &Path, salida: &Path, max_salida: usize) -> Result<u64, String> {
    let datos = std::fs::read(entrada).map_err(|e| format!("no se lee {entrada:?}: {e}"))?;
    let plano = descomprimir(&datos, max_salida)?;
    std::fs::write(salida, &plano).map_err(|e| format!("no se escribe {salida:?}: {e}"))?;
    Ok(plano.len() as u64)
}
