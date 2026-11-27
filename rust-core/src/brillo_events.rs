//! Vigilancia **por eventos** del brillo del móvil (SM-G998B, One UI 6).
//!
//! POR QUÉ ESTE MÓDULO
//! --------------------
//! El shim de brillo (v15) **sondeaba**: el vigilante leía el brillo cada
//! 15 s ⇒ 5 interacciones con el móvil por ciclo (MEDIDO).  Eso es un
//! constante coste y, además, llega tarde.
//!
//! La vía buena en Android es que **el propio sistema avise**. Y avisa por
//! fichero: tanto el proveedor de ajustes como el gestor de display
//! reescriben su estado con `AtomicFile` (escriben un temporal y lo
//! renombran), así que **cada cambio de brillo deja una huella en el disco**
//! que se puede vigilar con **inotify** ⇒ notificación en milisegundos,
//! **cero sondeos, cero JVM, cero inyecciones**.
//!
//! LO QUE SE VIGILA (medido en el SM-G998B)
//! ------------------------------------------
//! * `/data/system/users/0/settings_system.xml` — el proveedor de ajustes:
//!   aquí caen el deslizador del sistema, la política de energía del daemon
//!   (`settings put system screen_brightness`) y el modo
//!   (`screen_brightness_mode`).
//! * `/data/system/display-manager-state.xml` — el gestor de display: aquí
//!   cae `cmd display set-brightness` (AOSP 14: `PersistentDataStore.java:972`).
//!
//! `settings list system` **no funciona** en este firmware (MEDIDO:
//! devuelve vacío) ⇒ el XML es además la vía de lectura fiable.
//!
//! EL SHIM DE VERDAD (hook) Y SU PUENTE
//! -----------------------------------
//! Un hook ART sobre un método Java de `system_server` **no es viable hoy**
//! con este repo: el payload (`crates/hooks-payload`) es frida-gum puro, sin
//! puente Java (`docs/RE_GASTO_DATOS_JAZZTEL.md` §C.2; backlog J/K) ⇒ no
//! puede enganchar un `ArtMethod`.
//!
//! Por eso este módulo deja **el puente preparado**: un **FIFO** en
//! `/data/local/tmp/megadaemon/brillo.fifo` donde un shim (hook o binario
//!helper) escribe una línea por evento.  El daemon lo consume **bloqueando**
//! (0 sondeos).  El día que exista el puente Java, el shim es una línea de
//! escritura y **no hay que tocar el daemon**.
//!
//! FORMATO DE LA LÍNEA (una por evento, NDJSON mínimo)
//! --------------------------------------------------
//! ```text
//! {"ts":1795737000,"fuente":"fifo","clave":"screen_brightness","valor":"247"}
//! ```
//! Se acepta también la forma mínima `clave=valor` (una por línea).
//!
//! (b) EL CAMINO TÉCNICO DEL HOOK, CON SU MEDIDA
//! ----------------------------------------------
//! **Dónde enganchar** (AOSP 14, rama `android14-release`, leída):
//!
//! | Qué se quiere avisar | Clase y método exactos | Evidencia |
//! |---|---|---|
//! | el **valor** de brillo cambió | `com.android.server.display.BrightnessSetting#setBrightness(float)` | `BrightnessSetting.java:103`; la llama `DisplayPowerController#setBrightness` (`:2673`) y el deslizador del sistema (`SystemUI/.../settings/brightness/BrightnessController.java:342` → `DisplayManagerService$LocalService#setBrightness` → `DisplayManagerService.java:3985`) |
//! | el **modo** (manual/auto) cambió | `com.android.server.display.DisplayPowerController#handleBrightnessModeChange()` | `DisplayPowerController.java:2626-2636` (lee `SCREEN_BRIGHTNESS_MODE` y decide `mUseAutoBrightness`) |
//! | el ALS quiso cambiar el brillo | `com.android.server.display.AutomaticBrightnessController#updateAutoBrightness(boolean, boolean)` | `AutomaticBrightnessController.java:916` |
//!
//! **Cómo se llega al `ArtMethod`** (el camino real, con su fragility):
//!
//! 1. `dlopen("libart.so")` → `dlsym("_ZN3art6Runtime9CurrentEP")` (exportado)
//!    → `Runtime::Current()->GetClassLinker()` (exportado);
//! 2. `ClassLinker` → la `Class` de `com.android.server.display.*`;
//! 3. `Class::GetMethod` → el puntero `ArtMethod*`;
//! 4. **AHÍ ESTÁ LA FRAGILIDAD**: el código compilado (quick code) del método
//!    está en un **offset interno del `ArtMethod` que cambia entre versiones
//!    de ART**.  `art::ArtMethod::Hook` y `GetCompiledCode()` **no están
//!    exportados** ⇒ sin el puente Java de Frida no hay forma *robusta* de
//!    obtener la dirección; hay que leer el offset a mano para la versión de
//!    ART del aparato (`getprop ro.build.version.sdk` + el fingerprint del
//!    framework).
//!
//! **Coste**: además del ptrace a `system_server` (root ✓, Magisk), el hook
//! necesita (i) resolver el offset del `ArtMethod` para **esa** versión de
//! ART, (ii) un payload con puente Java, y (iii) un canal fiable hacia el
//! daemon (el FIFO de aquí).  Con el payload actual **no se puede** ⇒ no es
//! un problema de "es que cuesta": **falta el puente Java**.
//!
//! (c) EL SUSTITUTO QUE **SÍ** FUNCIONA HOY, CON SU MEDIDA
//! -------------------------------------------------------
//! Los tres métodos de arriba **terminan escribiendo un fichero**:
//!
//! * el del valor → `PersistentDataStore#setBrightness` → `saveIfNeeded()` →
//!   reescribe **`/data/system/display-manager-state.xml`** (`AtomicFile`:
//!   temporal + rename) ⇒ **`PersistentDataStore.java:972`**;
//! * el del modo → el proveedor de ajustes reescribe
//!   **`/data/system/users/0/settings_system.xml`**;
//!
//! ⇒ **vigilando esos ficheros con `inotify` se observa el *efecto* del hook
//! sin tocar `system_server`**: 0 sondeos, 0 JVM, latencia de milisegundos y
//! **sin el puente Java**.  Es lo que hace este módulo por defecto.
//!
//! Y el **FIFO** queda como el sitio donde escribirá el hook el día que
//! exista: [`escribir_evento`] (o `echo '{...}' > /data/local/tmp/megadaemon/
//! brillo.fifo` desde un shell) ⇒ el daemon lo consume bloqueando.
//! `brillo shim_probar` lo prueba end-to-end sin hook.

use std::io::Read;
use std::os::unix::io::FromRawFd;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// FIFO donde un shim escribe los eventos (si existe).
pub const RUTA_FIFO: &str = "/data/local/tmp/megadaemon/brillo.fifo";
/// Fichero con el latido del vigilante (para el arranque en frío).
pub const RUTA_HEARTBEAT: &str = "/data/local/tmp/megadaemon/brillo.heartbeat";
/// Ficheros cuyo cambio significa "el brillo ha cambiado".
pub const FICHEROS_VIGILADOS: &[&str] = &[
    "/data/system/users/0/settings_system.xml",
    "/data/system/display-manager-state.xml",
];

/// Un evento de brillo.
#[derive(Clone, Debug, PartialEq)]
pub struct Evento {
    /// Epoch en segundos.
    pub ts: u64,
    /// De dónde sale: `inotify` (fichero del sistema) o `fifo` (shim).
    pub fuente: &'static str,
    /// Qué cambió: el nombre del fichero o la clave del ajuste.
    pub clave: String,
    /// El valor nuevo (si se pudo leer).
    pub valor: Option<String>,
}

impl Evento {
    /// A JSON.
    pub fn a_json(&self) -> serde_json::Value {
        serde_json::json!({
            "ts": self.ts,
            "fuente": self.fuente,
            "clave": self.clave,
            "valor": self.valor,
        })
    }
}

/// Estado del vigilante (contadores atómicos: el hilo es el que escribe).
#[derive(Debug, Default)]
pub struct Contadores {
    /// Eventos recibidos por inotify.
    pub inotify: AtomicU64,
    /// Eventos recibidos por el FIFO del shim.
    pub fifo: AtomicU64,
    /// Heartbeats escritos.
    pub latidos: AtomicU64,
    /// Errores.
    pub errores: AtomicU64,
    /// Epoch del último evento.
    pub ultimo_ts: AtomicU64,
    /// Epoch del arranque del vigilante.
    pub arranque_ts: AtomicU64,
}

impl Contadores {
    /// Snapshot a JSON.
    pub fn a_json(&self) -> serde_json::Value {
        serde_json::json!({
            "eventos_inotify": self.inotify.load(Ordering::Relaxed),
            "eventos_fifo": self.fifo.load(Ordering::Relaxed),
            "latidos": self.latidos.load(Ordering::Relaxed),
            "errores": self.errores.load(Ordering::Relaxed),
            "ultimo_evento_ts": self.ultimo_ts.load(Ordering::Relaxed),
            "arranque_ts": self.arranque_ts.load(Ordering::Relaxed),
            "vivo": self.arranque_ts.load(Ordering::Relaxed) > 0,
        })
    }
}

/// Contadores globales del vigilante.
pub static CONTADORES: Contadores = Contadores {
    inotify: AtomicU64::new(0),
    fifo: AtomicU64::new(0),
    latidos: AtomicU64::new(0),
    errores: AtomicU64::new(0),
    ultimo_ts: AtomicU64::new(0),
    arranque_ts: AtomicU64::new(0),
};

/// Bandera para parar el vigilante.
static PARAR: AtomicBool = AtomicBool::new(false);

/// Ahora (epoch en segundos).
fn ahora() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Parsea una línea de evento del FIFO.
///
/// Acepta el NDJSON mínimo (`{"clave":"x","valor":"y"}`) y la forma
/// `clave=valor`.
pub fn parse_linea(linea: &str) -> Option<Evento> {
    let l = linea.trim();
    if l.is_empty() {
        return None;
    }
    if l.starts_with('{') {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(l) {
            let clave = v
                .get("clave")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            if clave.is_empty() {
                return None;
            }
            return Some(Evento {
                ts: v.get("ts").and_then(|x| x.as_u64()).unwrap_or_else(ahora),
                fuente: "fifo",
                clave,
                valor: v
                    .get("valor")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
            });
        }
        return None;
    }
    let (clave, valor) = l.split_once('=')?;
    Some(Evento {
        ts: ahora(),
        fuente: "fifo",
        clave: clave.trim().to_string(),
        valor: Some(valor.trim().to_string()),
    })
}

/// Crea el FIFO si no existe (best-effort).
pub fn asegurar_fifo() -> bool {
    let p = std::path::Path::new(RUTA_FIFO);
    if p.exists() {
        return true;
    }
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let c = match std::ffi::CString::new(RUTA_FIFO) {
        Ok(c) => c,
        Err(_) => return false,
    };
    // 0o666: el shim puede ser otro usuario; el contenido no lleva secretos.
    unsafe { libc::mkfifo(c.as_ptr(), 0o666) == 0 }
}

/// Escribe el latido (para el diagnóstico del arranque en frío).
pub fn escribir_latido(estado: &str) {
    let ts = ahora();
    CONTADORES.latidos.fetch_add(1, Ordering::Relaxed);
    let linea = format!("ts={ts} estado={estado}\n");
    let _ = std::fs::write(RUTA_HEARTBEAT, linea);
}

/// Escribe un evento en el FIFO del shim.
///
/// **Ésta es la interfaz que usaría el hook**: un shim dentro de
/// `system_server` (o un binario helper) escribe una línea por cambio y el
/// daemon la consume bloqueando (0 sondeos, 0 JVM).
///
/// La línea es NDJSON mínimo:
/// `{"ts":1795737000,"fuente":"fifo","clave":"screen_brightness","valor":"247"}`
///
/// Se puede escribir **desde un shell** (el shim más simple posible):
/// ```sh
/// echo '{"clave":"screen_brightness","valor":"247"}' > /data/local/tmp/megadaemon/brillo.fifo
/// ```
pub fn escribir_evento(clave: &str, valor: &str) -> std::io::Result<()> {
    let _ = asegurar_fifo();
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().write(true).open(RUTA_FIFO)?;
    writeln!(
        f,
        "{{\"ts\":{},\"fuente\":\"fifo\",\"clave\":\"{}\",\"valor\":\"{}\"}}",
        ahora(),
        clave.replace('"', ""),
        valor.replace('"', "")
    )?;
    f.flush()
}

/// Escribe un evento desde un shell (para probar el puente sin hook).
///
/// Devuelve la salida del comando (el shim hypothetical).
pub fn escribir_evento_shell(clave: &str, valor: &str) -> String {
    let _ = asegurar_fifo();
    let linea = format!(
        "{{\"ts\":{},\"fuente\":\"fifo\",\"clave\":\"{}\",\"valor\":\"{}\"}}",
        ahora(),
        clave,
        valor
    );
    let cmd = format!(
        "printf '%s\\n' '{}' > {} 2>&1",
        linea.replace('\'', "'\\''"),
        RUTA_FIFO
    );
    let r = crate::android::root_exec(&cmd, 10_000, 8 * 1024);
    let mut out = r.stdout.clone();
    out.push_str(&r.stderr);
    out
}

/// Arranca el vigilante por eventos.
///
/// * `inotify` sobre los **directorios** de los ficheros vigilados (con
///   filtro por nombre, porque `AtomicFile` renombra: el evento real es
///   `IN_MOVED_TO` en el directorio).
/// * el **FIFO** del shim, leído en bloque (0 sondeos).
///
/// Devuelve `true` si el hilo arrancó.
pub fn arrancar<F>(on_event: F) -> Arranque
where
    F: Fn(Evento) + Send + 'static + Clone,
{
    PARAR.store(false, Ordering::Relaxed);
    CONTADORES.arranque_ts.store(ahora(), Ordering::Relaxed);

    // El fd se abre AQUI (no en el hilo) para poder decir si inotify sirve:
    // si no, el llamador cae al sondeo y el brillo no se queda sin vigilar.
    let fd = unsafe { libc::inotify_init1(libc::IN_CLOEXEC) };
    let inotify_ok = fd >= 0 && registrar_watches(fd);
    if !inotify_ok {
        CONTADORES.errores.fetch_add(1, Ordering::Relaxed);
    }

    let manejador_inotify = on_event.clone();
    let hilo_inotify = if inotify_ok {
        std::thread::Builder::new()
            .name("brillo-inotify".into())
            .spawn(move || bucle_inotify(fd, manejador_inotify))
            .is_ok()
    } else {
        false
    };

    let manejador_fifo = on_event.clone();
    let hilo_fifo = std::thread::Builder::new()
        .name("brillo-fifo".into())
        .spawn(move || bucle_fifo(manejador_fifo))
        .is_ok();

    escribir_latido(if hilo_inotify {
        "arrancado:eventos"
    } else {
        "arrancado:sin-inotify"
    });
    Arranque {
        inotify: hilo_inotify,
        fifo: hilo_fifo,
    }
}

/// Qué partes del vigilante arrancaron.
#[derive(Clone, Copy, Debug, Default)]
pub struct Arranque {
    /// El hilo de inotify (los ficheros del sistema).
    pub inotify: bool,
    /// El hilo del FIFO (el shim).
    pub fifo: bool,
}

impl Arranque {
    /// ¿Hay vigilancia por eventos?
    pub fn eventos(&self) -> bool {
        self.inotify || self.fifo
    }
    /// A JSON.
    pub fn a_json(&self) -> serde_json::Value {
        serde_json::json!({ "inotify": self.inotify, "fifo": self.fifo, "eventos": self.eventos() })
    }
}

/// Registra los watches de inotify (los directorios de los ficheros vigilados).
fn registrar_watches(fd: i32) -> bool {
    let mut alguno = false;
    for f in FICHEROS_VIGILADOS {
        let p = std::path::Path::new(f);
        let Some(dir) = p.parent() else {
            continue;
        };
        let c = std::ffi::CString::new(dir.to_string_lossy().to_string()).unwrap_or_default();
        let wd = unsafe {
            libc::inotify_add_watch(
                fd,
                c.as_ptr(),
                libc::IN_MOVED_TO | libc::IN_CREATE | libc::IN_CLOSE_WRITE | libc::IN_MODIFY,
            )
        };
        if wd >= 0 {
            alguno = true;
        }
    }
    alguno
}

/// Pide al vigilante que pare.
pub fn parar() {
    PARAR.store(true, Ordering::Relaxed);
    escribir_latido("parado");
}

/// Bucle de inotify: bloquea en `read()` hasta que hay un evento.
fn bucle_inotify<F>(fd: i32, on_event: F)
where
    F: Fn(Evento) + Send + 'static,
{
    // Directorios a vigilar con el filtro por nombre.
    let mut dirs: Vec<(String, Vec<String>)> = Vec::new();
    for f in FICHEROS_VIGILADOS {
        let p = std::path::Path::new(f);
        let Some(dir) = p.parent() else {
            continue;
        };
        let Some(nombre) = p.file_name() else {
            continue;
        };
        let dir_s = dir.to_string_lossy().to_string();
        let nombre_s = nombre.to_string_lossy().to_string();
        if !dirs.iter().any(|(d, _)| *d == dir_s) {
            dirs.push((dir_s.clone(), Vec::new()));
        }
        if let Some(e) = dirs.iter_mut().find(|(d, _)| *d == dir_s) {
            e.1.push(nombre_s);
        }
    }
    let mut buf = vec![0u8; 8192];
    let tam_evento = std::mem::size_of::<libc::inotify_event>();
    loop {
        if PARAR.load(Ordering::Relaxed) {
            break;
        }
        let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
        if n <= 0 {
            if n < 0 {
                CONTADORES.errores.fetch_add(1, Ordering::Relaxed);
            }
            break;
        }
        let total = n as usize;
        let mut off = 0usize;
        while off + tam_evento <= total {
            let (len, nombre_off) = unsafe {
                let ev = &*(buf.as_ptr().add(off) as *const libc::inotify_event);
                (ev.len as usize, off + tam_evento)
            };
            if len > 0 && nombre_off + len <= total {
                let bruto = &buf[nombre_off..nombre_off + len];
                let nombre = String::from_utf8_lossy(bruto)
                    .trim_end_matches('\0')
                    .trim()
                    .to_string();
                if !nombre.is_empty() && dirs.iter().any(|(_, n)| n.iter().any(|x| *x == nombre)) {
                    let clave = FICHEROS_VIGILADOS
                        .iter()
                        .find(|f| f.ends_with(&nombre))
                        .map(|f| f.to_string())
                        .unwrap_or(nombre);
                    CONTADORES.inotify.fetch_add(1, Ordering::Relaxed);
                    CONTADORES.ultimo_ts.store(ahora(), Ordering::Relaxed);
                    on_event(Evento {
                        ts: ahora(),
                        fuente: "inotify",
                        clave,
                        valor: None,
                    });
                }
            }
            off = nombre_off + len;
        }
    }
    unsafe {
        libc::close(fd);
    }
}

/// Bucle del FIFO del shim: lee líneas en bloque.
fn bucle_fifo<F>(on_event: F)
where
    F: Fn(Evento) + Send + 'static,
{
    let _ = asegurar_fifo();
    let c = std::ffi::CString::new(RUTA_FIFO).unwrap_or_default();
    // O_RDWR: al mantener nosotros el extremo de escritura, el FIFO no da
    // EOF cuando el shim cierra ⇒ `read` bloquea sin perder eventos.
    let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDWR | libc::O_NONBLOCK) };
    if fd < 0 {
        CONTADORES.errores.fetch_add(1, Ordering::Relaxed);
        return;
    }
    unsafe {
        libc::fcntl(fd, libc::F_SETFL, 0);
    }
    let mut fichero = unsafe { std::fs::File::from_raw_fd(fd) };
    let mut resto = String::new();
    loop {
        if PARAR.load(Ordering::Relaxed) {
            break;
        }
        let mut trozo = [0u8; 512];
        let n = match fichero.read(&mut trozo) {
            Ok(n) => n,
            Err(_) => {
                CONTADORES.errores.fetch_add(1, Ordering::Relaxed);
                break;
            }
        };
        if n == 0 {
            break;
        }
        resto.push_str(&String::from_utf8_lossy(&trozo[..n]));
        while let Some(pos) = resto.find('\n') {
            let linea = resto[..pos].to_string();
            resto.drain(..=pos);
            if let Some(ev) = parse_linea(&linea) {
                CONTADORES.fifo.fetch_add(1, Ordering::Relaxed);
                CONTADORES.ultimo_ts.store(ahora(), Ordering::Relaxed);
                on_event(ev);
            }
        }
    }
}
