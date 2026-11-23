//! **Payload de instrumentación en Rust con frida-gum** (bindings oficiales
//! de Frida) para observar el proceso objetivo: **SQLite**, **red** y
//! **ficheros**, sin tocar sus datos.
//!
//! # Qué es
//!
//! Una `cdylib` que se carga (`dlopen`) en el proceso que se quiere observar
//! (la app de Antigravity, `com.agy`, en el móvil).  Al cargarse, instala
//! hooks con **frida-gum** y escribe **una línea JSON por evento** (NDJSON)
//! en un fichero que el daemon lee.
//!
//! # API de frida-gum usada (0.17.2, la publicada en crates.io)
//!
//! | Qué | Cómo |
//! |---|---|
//! | Runtime | `frida_gum::Gum::obtain()` |
//! | Motor de hooks | `frida_gum::interceptor::Interceptor::obtain(&gum)` |
//! | Buscar el módulo | `frida_gum::Process::obtain(&gum).find_module_by_name("libc.so")` |
//! | Buscar la función | `Module::find_export_by_name("open")` (o `Module::find_global_export_by_name`) |
//! | Hookear | `Interceptor::attach(ptr, &mut listener) -> Result<Listener>` |
//! | Leer argumentos | `InvocationContext::arg(n) -> usize`, `::thread_id() -> u32` |
//!
//! Docs: <https://docs.rs/frida-gum/0.17.2/frida_gum/> y
//! <https://docs.rs/frida-gum/0.17.2/frida_gum/interceptor/>.
//!
//! # Reglas
//!
//! * **observar, no modificar**: los hooks sólo leen y registran;
//! * **nada de credenciales**: todo pasa por [`redact`];
//! * **nunca panic**: cada hook va envuelto en `catch_unwind`;
//! * **nunca deadlock**: el escritor NDJSON tiene un guardián de re-entrada;
//! * **sin `std::fs` en el camino caliente**: `libc::open` + `libc::write`.

mod hooks;
mod json;
mod redact;
mod sink;
mod sys;

use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{Mutex, OnceLock};

use frida_gum::interceptor::Interceptor;
use frida_gum::Gum;

use ctor::ctor;

/// Versión del payload (para el que lo carga).
pub const VERSION: u32 = 1;

/// Gum (uno solo en todo el proceso).
static GUM: OnceLock<Gum> = OnceLock::new();

/// El `Interceptor` (nunca se suelta: gum desengancha los hooks cuando se
/// libera el interceptor).
static INTERCEPTOR: AtomicPtr<Interceptor> = AtomicPtr::new(std::ptr::null_mut());

/// Informe de la instalación (para que la llamada sea idempotente).
static INFORME: OnceLock<hooks::Informe> = OnceLock::new();

/// Serializa la instalación (dos hilos a la vez no deben hookear dos veces).
static CERROJO: Mutex<()> = Mutex::new(());

/// El runtime de gum.
pub fn gum() -> &'static Gum {
    GUM.get_or_init(Gum::obtain)
}

/// El interceptor (fugado a propósito: vive lo que el proceso).
fn interceptor() -> &'static mut Interceptor {
    let p = INTERCEPTOR.load(Ordering::SeqCst);
    if !p.is_null() {
        // SAFETY: el puntero se publica una sola vez (bajo `CERROJO`) y no
        // se libera nunca.
        return unsafe { &mut *p };
    }
    let raw = Box::into_raw(Box::new(Interceptor::obtain(gum())));
    INTERCEPTOR.store(raw, Ordering::SeqCst);
    // SAFETY: ver arriba.
    unsafe { &mut *raw }
}

/// Rutas candidatas del NDJSON, **en orden de preferencia**.
///
/// La primera (la del contrato) es
/// `/data/local/tmp/megadaemon/state/hooks.ndjson`; pero el payload se
/// ejecuta en el proceso de una app, que normalmente **no** puede escribir
/// ahí (`/data/local/tmp` es de root), así que se prueban también rutas que
/// la app sí puede escribir (su `cache`).  La ruta que se acaba usando se
/// escribe en el primer evento del NDJSON.
pub fn candidatas_ndjson(gum: &Gum) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    if let Ok(p) = std::env::var("MEGADAEMON_HOOKS_NDJSON") {
        if !p.trim().is_empty() {
            v.push(p.trim().to_string());
        }
    }
    let uid = sys::uid();
    v.push("/data/local/tmp/megadaemon/state/hooks.ndjson".to_string());
    v.push(format!(
        "/data/local/tmp/megadaemon/state/hooks-{uid}.ndjson"
    ));
    v.push(format!("/data/local/tmp/hooks-{uid}.ndjson"));
    if let Some(pkg) = sys::paquete_android() {
        v.push(format!("/data/user/0/{pkg}/cache/megadaemon-hooks.ndjson"));
        v.push(format!("/data/data/{pkg}/cache/megadaemon-hooks.ndjson"));
    }
    let tmp = frida_gum::Process::obtain(gum).tmp_dir();
    let tmp = tmp.trim().trim_end_matches('/');
    if !tmp.is_empty() {
        v.push(format!("{tmp}/megadaemon-hooks-{uid}.ndjson"));
    }
    // Último recurso: el TMPDIR del entorno (en el host y en CI).
    if let Ok(t) = std::env::var("TMPDIR") {
        let t = t.trim().trim_end_matches('/');
        if !t.is_empty() {
            v.push(format!("{t}/megadaemon-hooks-{uid}.ndjson"));
        }
    }
    v
}

/// Instala los hooks (idempotente).
pub fn instalar() -> Result<hooks::Informe, String> {
    if let Some(i) = INFORME.get() {
        return Ok(i.clone());
    }
    let _guard = CERROJO.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(i) = INFORME.get() {
        return Ok(i.clone());
    }
    let g = gum();
    // 1) El NDJSON PRIMEERO: a partir de aquí, cualquier `open` del propio
    //    proceso ya se registra (y el guardián de re-entrada evita el bucle).
    let candidatas = candidatas_ndjson(g);
    let ruta = sink::Sink::global().abrir(&candidatas)?;
    // 2) Los hooks.
    let inter = interceptor();
    let mut informe = hooks::instalar(inter, g);
    informe.ndjson = ruta.clone();
    // 3) El evento de arranque (con el informe completo).
    let mut campos = vec![
        json::campo("hook", "payload"),
        json::campo("event", "iniciado"),
        json::campo_num("version", VERSION as u64),
        json::campo_num("pid", sys::pid() as u64),
        json::campo("ndjson", &ruta),
    ];
    let ganchos: Vec<String> = informe
        .ganchos
        .iter()
        .map(|g| {
            format!(
                "{{{},{},{}}}",
                json::campo("simbolo", &g.simbolo),
                json::campo("modulo", &g.modulo),
                json::campo("estado", &g.estado)
            )
        })
        .collect();
    campos.push(format!("\"ganchos\":[{}]", ganchos.join(",")));
    sink::Sink::global().evento(0, &json::une(&campos));
    let _ = INFORME.set(informe.clone());
    Ok(informe)
}

/// **Punto de entrada exportado**: lo puede llamar el cargador (por
/// `dlsym`) o el constructor de la librería.
///
/// Devuelve `0` si los hooks quedaron instalados, `-1` si no.
#[no_mangle]
pub extern "C" fn megadaemon_hooks_start() -> i32 {
    match instalar() {
        Ok(informe) => {
            if informe.instalados() == 0 {
                -1
            } else {
                0
            }
        }
        Err(_) => -1,
    }
}

/// ¿Están los hooks instalados? (1 = sí, 0 = no).
#[no_mangle]
pub extern "C" fn megadaemon_hooks_installed() -> i32 {
    match INFORME.get() {
        Some(i) if i.instalados() > 0 => 1,
        _ => 0,
    }
}

/// Versión del payload.
#[no_mangle]
pub extern "C" fn megadaemon_hooks_version() -> u32 {
    VERSION
}

/// Se auto-instala al cargar la librería (constructor de ELF).
///
/// Se puede desactivar con `MEGADAEMON_HOOKS_AUTOSTART=0` (útil para
/// cargar la librería y activarla después a mano).
#[cfg(not(test))]
#[ctor]
fn autostart() {
    if std::env::var("MEGADAEMON_HOOKS_AUTOSTART")
        .map(|v| v == "0")
        .unwrap_or(false)
    {
        return;
    }
    let _ = megadaemon_hooks_start();
}
