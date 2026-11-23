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

pub mod hooks;
pub mod plan;
mod json;
// La politica de redaccion vive en `crates/redact` (fuente unica para el
// shim, el payload de hooks y el payload del inyector).  Aqui solo se
// reexporta para no cambiar las llamadas.
pub use megadaemon_redact as redact;
mod sink;
mod sys;

use std::sync::{Mutex, MutexGuard, OnceLock};

use frida_gum::interceptor::Interceptor;
use frida_gum::Gum;

// El constructor de ELF sólo existe fuera de los tests (en el binario de
// test el payload no debe auto-instalarse).
#[cfg(not(test))]
use ctor::ctor;

/// Versión del payload (para el que lo carga).
pub const VERSION: u32 = 1;

/// Gum (uno solo en todo el proceso).
static GUM: OnceLock<Gum> = OnceLock::new();

/// El `Interceptor` (nunca se suelta: gum desengancha los hooks cuando se
/// libera el interceptor).
///
/// Va detrás de un `Mutex` y NO se expone como `&'static mut`: dos `&mut`
/// al mismo objeto serían UB (aliasing).  El mutex da acceso exclusivo.
struct InterceptorBloqueado(Interceptor);

// SAFETY: el `Interceptor` de gum contiene un puntero crudo y NO es `Send`.
// Aquí sólo se usa bajo el mutex de abajo (`instalar` es la única vía) y
// gum no exige que el hilo que adjunta y el que llama sean el mismo.
unsafe impl Send for InterceptorBloqueado {}

static INTERCEPTOR: OnceLock<Mutex<InterceptorBloqueado>> = OnceLock::new();

/// Informe de la instalación (para que la llamada sea idempotente).
static INFORME: OnceLock<hooks::Informe> = OnceLock::new();

/// Serializa la instalación (dos hilos a la vez no deben hookear dos veces).
static CERROJO: Mutex<()> = Mutex::new(());

/// El runtime de gum.
pub fn gum() -> &'static Gum {
    GUM.get_or_init(Gum::obtain)
}

/// El interceptor global (vive lo que el proceso).
fn interceptor() -> &'static Mutex<InterceptorBloqueado> {
    INTERCEPTOR.get_or_init(|| Mutex::new(InterceptorBloqueado(Interceptor::obtain(gum()))))
}

/// Acceso exclusivo al interceptor (sin envenenarse: un panic en un hook no
/// puede dejar el mutex envenenado para los demás hilos).
fn interceptor_bloqueado() -> MutexGuard<'static, InterceptorBloqueado> {
    match interceptor().lock() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    }
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
    let mut inter = interceptor_bloqueado();
    let mut informe = hooks::instalar(&mut inter.0, g);
    // 2b) Los hooks del PLAN: funciones Go del .so por direccion (pclntab).
    //     El .so real no se toca: gum intercepta en memoria.
    hooks::instala_plan(&mut inter.0, g, &crate::plan::ruta(), &mut informe);
    informe.ndjson = ruta.clone();
    // 3) El evento de arranque (con el informe completo).
    //    OJO: el escritor ya pone `ts`, `pid` y `tid` ⇒ aquí no se repiten.
    let mut campos = vec![
        json::campo("hook", "payload"),
        json::campo("event", "iniciado"),
        json::campo_num("version", VERSION as u64),
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
    // El evento de arranque NO se puede perder (dice qué ruta de NDJSON se
    // está usando y qué hooks quedaron instalados) ⇒ se escribe esperando
    // el mutex, no descartándolo.
    sink::Sink::global().evento_bloqueante(0, &json::une(&campos));
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
///
/// MEDIDO (2026-11-23, crate `ctor` 1.0.14 leído del `.crate`): el
/// atributo **exige** `unsafe`:
/// `src/parse.rs` tiene un `compile_error!("Missing unsafe keyword in
/// #[ctor] annotation. Use #[ctor(unsafe)]")` que salta cuando la función
/// NO es `unsafe fn` y el atributo no lleva `unsafe`.  Con `#[ctor]` a secas
/// el crate NO COMPILABA (y el CI `build-hooks-payload` se caía).
/// El `unsafe` de `#[ctor(unsafe)]` significa "puede correr antes de que el
/// runtime de Rust esté listo": aquí es seguro porque el payload se carga con
/// `dlopen` dentro de un proceso que ya tiene el runtime.
#[cfg(not(test))]
#[ctor(unsafe)]
fn autostart() {
    if std::env::var("MEGADAEMON_HOOKS_AUTOSTART")
        .map(|v| v == "0")
        .unwrap_or(false)
    {
        return;
    }
    let _ = megadaemon_hooks_start();
}
