//! Escritor **NDJSON** del payload: una línea JSON por evento.
//!
//! Reglas (durdas):
//!
//! * **Nunca `std::fs`**: el escritor se llama DENTRO de un interceptor (el
//!   hilo de la app hookeada).  `std::fs` puede bloquear y paniquear; aquí
//!   sólo `libc::open` + `libc::write` (una llamada al sistema por línea).
//! * **O_APPEND**: cada línea se añade al final; dos hilos que escriben a la
//!   vez no se pisan.
//! * **Una línea = un `write`**: la línea completa (con su `\n`) se escribe
//!   de una vez, así que el fichero siempre es un NDJSON válido.
//! * **Re-entrada**: nuestro propio `open()` dispara el hook de ficheros ⇒
//!   un guardián por hilo (`EN_ESCRITURA`) hace que ese hook se ignore.
//! * **Nunca deadlock**: el mutex sólo protege el `fd` y el contador; dentro
//!   de la seccion critica no se llama a ninguna funcion hookeada salvo
//!   `open` (protegido por el guardian) y `write` (que no hookeamos).

use std::cell::Cell;
use std::ffi::CString;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

thread_local! {
    /// Guardián de re-entrada: `true` mientras el escritor está trabajando
    /// (los hooks miran esta bandera y se hacen los sordos).
    static EN_ESCRITURA: Cell<bool> = Cell::new(false);
}

/// ¿Estamos dentro del escritor?  (los hooks lo consultan para no recursar)
pub fn en_escritura() -> bool {
    EN_ESCRITURA.with(|c| c.get())
}

/// Estado interno del escritor.
#[derive(Default)]
struct Estado {
    /// `fd` abierto en modo `O_APPEND` (o `None` si no se pudo abrir).
    fd: Option<i32>,
    /// Ruta elegida (la que sí se pudo abrir).
    ruta: Option<String>,
    /// Último error (para poder informarlo sin escribirlo en el NDJSON).
    error: Option<String>,
    /// Líneas escritas.
    lineas: u64,
    /// Bytes escritos.
    bytes: u64,
}

/// El escritor NDJSON (uno solo en todo el proceso).
pub struct Sink {
    estado: Mutex<Estado>,
}

impl Sink {
    fn nuevo() -> Self {
        Sink {
            estado: Mutex::new(Estado::default()),
        }
    }

    /// El escritor global.
    pub fn global() -> &'static Sink {
        static SINK: OnceLock<Sink> = OnceLock::new();
        SINK.get_or_init(Sink::nuevo)
    }

    /// Bloquea el estado sin envenenarse (un panic dentro del escritor no
    /// puede dejar el mutex envenenado para los demás hilos).
    fn lock(&self) -> MutexGuard<'_, Estado> {
        match self.estado.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        }
    }

    /// Abre el NDJSON: prueba las rutas candidatas **en orden** y se queda
    /// con la primera que se puede abrir (creando el directorio si hace
    /// falta).  Devuelve la ruta elegida.
    pub fn abrir(&self, candidatas: &[String]) -> Result<String, String> {
        let mut ultimo: Option<String> = None;
        for ruta in candidatas {
            if ruta.trim().is_empty() {
                continue;
            }
            let Some((dir, _archivo)) = ruta.rsplit_once('/') else {
                continue;
            };
            if !dir.is_empty() {
                mkdir_p(dir);
            }
            match abrir_append(ruta) {
                Ok(fd) => {
                    let mut e = self.lock();
                    e.fd = Some(fd);
                    e.ruta = Some(ruta.clone());
                    e.error = None;
                    return Ok(ruta.clone());
                }
                Err(err) => ultimo = Some(format!("{ruta}: {err}")),
            }
        }
        let msg = match ultimo {
            Some(u) => format!("no se pudo abrir el NDJSON ({u})"),
            None => "no se pudo abrir el NDJSON (sin rutas candidatas)".to_string(),
        };
        let mut e = self.lock();
        e.error = Some(msg.clone());
        Err(msg)
    }

    /// Escribe una línea (ya terminada en `\n`).
    pub fn escribir(&self, linea: &str) -> bool {
        EN_ESCRITURA.with(|c| c.set(true));
        let res = self.escribir_interno(linea);
        EN_ESCRITURA.with(|c| c.set(false));
        res
    }

    fn escribir_interno(&self, linea: &str) -> bool {
        let bytes = linea.as_bytes();
        let mut e = self.lock();
        let Some(fd) = e.fd else {
            return false;
        };
        if !write_all(fd, bytes) {
            // El descriptor se cerró por fuera (o el disco falló): se anota
            // y se deja de escribir (no se reintenta en bucle: el hook está
            // en el hilo de la app).
            e.error = Some("write() falló".to_string());
            return false;
        }
        e.lineas += 1;
        e.bytes += bytes.len() as u64;
        true
    }

    /// Registra un evento: `cuerpo` son los campos JSON **sin** llaves
    /// (p.ej. `"hook":"red","event":"connect"`).
    pub fn evento(&self, tid: u32, cuerpo: &str) {
        let ts = ahora_ms();
        let pid = unsafe { libc::getpid() };
        let mut linea = String::with_capacity(cuerpo.len() + 64);
        linea.push_str("{\"ts\":");
        linea.push_str(&ts.to_string());
        linea.push_str(",\"pid\":");
        linea.push_str(&pid.to_string());
        linea.push_str(",\"tid\":");
        linea.push_str(&tid.to_string());
        linea.push(',');
        linea.push_str(cuerpo);
        linea.push('}');
        linea.push('\n');
        self.escribir(&linea);
    }

    /// Ruta en la que se está escribiendo (si alguna).
    pub fn ruta(&self) -> Option<String> {
        self.lock().ruta.clone()
    }

    /// Último error del escritor.
    pub fn error(&self) -> Option<String> {
        self.lock().error.clone()
    }

    /// Número de líneas escritas.
    pub fn lineas(&self) -> u64 {
        self.lock().lineas
    }
}

/// Milisegundos desde la época Unix.
pub fn ahora_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Abre un fichero en modo `O_APPEND|O_CREAT|O_WRONLY|O_CLOEXEC`.
fn abrir_append(ruta: &str) -> Result<i32, String> {
    let c = CString::new(ruta.as_str()).map_err(|_| "ruta con NUL".to_string())?;
    let fd = unsafe {
        libc::open(
            c.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_APPEND | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        let e = std::io::Error::last_os_error();
        return Err(e.to_string());
    }
    Ok(fd)
}

/// Escribe todo el buffer (reintentando ante `EINTR`).
fn write_all(fd: i32, buf: &[u8]) -> bool {
    let mut off = 0usize;
    while off < buf.len() {
        let n = unsafe {
            libc::write(
                fd,
                buf[off..].as_ptr() as *const libc::c_void,
                buf.len() - off,
            )
        };
        if n < 0 {
            let e = std::io::Error::last_os_error();
            if e.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return false;
        }
        if n == 0 {
            return false;
        }
        off += n as usize;
    }
    true
}

/// Crea el directorio (y sus ancestros) con modo 0700.  Los fallos NO son
/// fatales: el escritor probará la siguiente ruta candidata.
fn mkdir_p(dir: &str) -> bool {
    let mut ok = true;
    let mut ruta = String::from("/");
    for parte in dir.split('/') {
        if parte.is_empty() {
            continue;
        }
        if !ruta.ends_with('/') {
            ruta.push('/');
        }
        ruta.push_str(parte);
        let c = match CString::new(ruta.as_str()) {
            Ok(c) => c,
            Err(_) => {
                ok = false;
                continue;
            }
        };
        if unsafe { libc::mkdir(c.as_ptr(), 0o700) } != 0 {
            let e = std::io::Error::last_os_error();
            if e.raw_os_error() != Some(libc::EEXIST) {
                ok = false;
            }
        }
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escribe_una_linea_por_evento() {
        let dir = format!("/tmp/megadaemon_sink_{}", std::process::id());
        let ruta = format!("{dir}/eventos.ndjson");
        let _ = std::fs::remove_file(&ruta);
        let sink = Sink::nuevo();
        let cand = vec![ruta.clone()];
        assert_eq!(sink.abrir(&cand).unwrap(), ruta);
        sink.evento(7, "\"hook\":\"red\",\"event\":\"connect\"");
        sink.evento(8, "\"hook\":\"sqlite\",\"event\":\"step\"");
        let txt = std::fs::read_to_string(&ruta).unwrap();
        let lineas: Vec<&str> = txt.lines().collect();
        assert_eq!(lineas.len(), 2, "{txt}");
        assert!(lineas[0].contains("\"hook\":\"red\""));
        assert!(lineas[0].contains("\"tid\":7"));
        assert!(lineas[1].contains("\"pid\":"));
        let _ = std::fs::remove_file(&ruta);
    }

    #[test]
    fn una_ruta_imposible_da_error_explicito() {
        let sink = Sink::nuevo();
        let cand = vec!["/proc/esto-no-existe/x.ndjson".to_string()];
        assert!(sink.abrir(&cand).is_err());
    }
}
