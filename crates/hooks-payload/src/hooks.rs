//! Los hooks del payload, con **frida-gum** (bindings oficiales de Frida).
//!
//! API REAL usada (frida-gum 0.17.2, la de crates.io):
//!
//! ```text
//! frida_gum::Gum::obtain()                       -> Gum
//! frida_gum::interceptor::Interceptor::obtain(&gum)
//! frida_gum::Process::obtain(&gum).find_module_by_name("libc.so")
//! Module::find_export_by_name("open")            -> Option<NativePointer>
//! Module::find_global_export_by_name("sqlite3_step")
//! Interceptor::attach(ptr, &mut listener)         -> Result<Listener>
//! InvocationContext::arg(n) / ::thread_id()       -> usize / u32
//! ```
//!
//! (En el crate `frida` —el de cliente remoto— existen `Module::get` y
//! `Function::by_name`; aquí, en gum embebido, la búsqueda es por
//! **módulo + export**, que es lo que hace el propio runtime de Frida.)

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use frida_gum::interceptor::{Interceptor, InvocationContext, InvocationListener};
use frida_gum::{Gum, Module, NativePointer};

use crate::json;
use crate::redact;
use crate::sink;
use crate::sys;

/// Cómo se llama a un hook en el NDJSON.
#[derive(Debug, Clone)]
pub struct InformeGancho {
    /// Símbolo hookeado (`open`, `sqlite3_step`…).
    pub simbolo: String,
    /// Módulo donde se encontró (`libc.so`, `global`…).
    pub modulo: String,
    /// `instalado`, `ausente` o `error: …`.
    pub estado: String,
}

/// Resultado de instalar los hooks.
#[derive(Debug, Clone)]
pub struct Informe {
    /// PID del proceso instrumentado.
    pub pid: i32,
    /// Ruta del NDJSON donde se está escribiendo.
    pub ndjson: String,
    /// Estado de cada hook.
    pub ganchos: Vec<InformeGancho>,
}

impl Informe {
    /// Número de hooks instalados.
    pub fn instalados(&self) -> usize {
        self.ganchos
            .iter()
            .filter(|g| g.estado == "instalado")
            .count()
    }
}

/// Mapa `stmt de sqlite -> SQL` (para saber qué tabla toca cada `step`).
fn mapa_stmt() -> &'static Mutex<HashMap<usize, String>> {
    static MAPA: OnceLock<Mutex<HashMap<usize, String>>> = OnceLock::new();
    MAPA.get_or_init(|| Mutex::new(HashMap::new()))
}

/// SQL pendiente del `prepare` actual (por hilo: `prepare` y su `leave`
/// ocurren en el mismo hilo, pero pueden intercalarse con otro hilo).
thread_local! {
    static SQL_PENDIENTE: std::cell::RefCell<Option<String>> = std::cell::RefCell::new(None);
}

/// Instala todos los hooks.  Devuelve el informe (qué quedó instalado y qué
/// no se encontró en el proceso).
pub fn instalar(interceptor: &mut Interceptor, gum: &Gum) -> Informe {
    let mut informe = Informe {
        pid: sys::pid(),
        ndjson: String::new(),
        ganchos: Vec::new(),
    };

    // --- FICHEROS (libc.so) ------------------------------------------------
    adjunta(
        interceptor,
        gum,
        "open",
        &["libc.so"],
        FileListener {
            simbolo: "open",
            indice: 0,
        },
        &mut informe,
    );
    adjunta(
        interceptor,
        gum,
        "openat",
        &["libc.so"],
        FileListener {
            simbolo: "openat",
            indice: 1,
        },
        &mut informe,
    );
    adjunta(
        interceptor,
        gum,
        "unlink",
        &["libc.so"],
        FileListener {
            simbolo: "unlink",
            indice: 0,
        },
        &mut informe,
    );

    // --- RED (libc.so) -----------------------------------------------------
    adjunta(
        interceptor,
        gum,
        "connect",
        &["libc.so"],
        ConnectListener,
        &mut informe,
    );
    adjunta(
        interceptor,
        gum,
        "sendto",
        &["libc.so"],
        SendToListener,
        &mut informe,
    );
    adjunta(
        interceptor,
        gum,
        "getaddrinfo",
        &["libc.so"],
        GetAddrInfoListener,
        &mut informe,
    );

    // --- SQLITE (libsqlite3.so) --------------------------------------------
    adjunta(
        interceptor,
        gum,
        "sqlite3_open_v2",
        &["libsqlite3.so", "libsqlite.so"],
        SqliteOpenListener,
        &mut informe,
    );
    adjunta(
        interceptor,
        gum,
        "sqlite3_prepare_v2",
        &["libsqlite3.so", "libsqlite.so"],
        SqlitePrepareListener,
        &mut informe,
    );
    adjunta(
        interceptor,
        gum,
        "sqlite3_step",
        &["libsqlite3.so", "libsqlite.so"],
        SqliteStepListener,
        &mut informe,
    );
    adjunta(
        interceptor,
        gum,
        "sqlite3_finalize",
        &["libsqlite3.so", "libsqlite.so"],
        SqliteFinalizeListener,
        &mut informe,
    );

    informe
}

/// Busca el símbolo: primero en los módulos indicados, luego en las
/// exportaciones globales y por último en la tabla de símbolos del binario
/// principal (SQLite enlazado estáticamente no exporta nada).
fn localiza(gum: &Gum, simbolo: &str, mods: &[&str]) -> Option<(NativePointer, String)> {
    let process = frida_gum::Process::obtain(gum);
    for m in mods {
        if let Some(module) = process.find_module_by_name(m) {
            if let Some(p) = module.find_export_by_name(simbolo) {
                return Some((p, (*m).to_string()));
            }
        }
    }
    if let Some(p) = Module::find_global_export_by_name(simbolo) {
        return Some((p, "global".to_string()));
    }
    let main = process.main_module();
    if let Some(p) = main.find_symbol_by_name(simbolo) {
        return Some((p, "main".to_string()));
    }
    None
}

/// Adjunta un listener.  El listener y el `Listener` que devuelve gum se
/// **fugan** (`Box::leak`): gum guarda el puntero al listener en su vtable y
/// el interceptor debe vivir mientras el proceso viva (si no, al soltar el
/// `Interceptor` se desenganchan los hooks).
fn adjunta<I: InvocationListener>(
    interceptor: &mut Interceptor,
    gum: &Gum,
    simbolo: &str,
    mods: &[&str],
    listener: I,
    informe: &mut Informe,
) {
    let Some((ptr, modulo)) = localiza(gum, simbolo, mods) else {
        informe.ganchos.push(InformeGancho {
            simbolo: simbolo.to_string(),
            modulo: String::new(),
            estado: "ausente".to_string(),
        });
        return;
    };
    // El `Box` se crea ANTES de adjuntar: la vtable de gum guarda el
    // puntero al listener, así que su dirección no puede cambiar después.
    let mut boxed: Box<I> = Box::new(listener);
    match interceptor.attach(ptr, &mut *boxed) {
        Ok(handle) => {
            let _ = Box::leak(boxed);
            // MEDIDO (2026-11-23, CI): el `Listener` de frida-gum NO es un
            // Box (es un handle plano) => `Box::leak(handle)` no compila
            // (E0308).  Para que el hook siga vivo hay que OLVIDARLO
            // (`mem::forget`), no filtrarlo.
            std::mem::forget(handle);
            informe.ganchos.push(InformeGancho {
                simbolo: simbolo.to_string(),
                modulo,
                estado: "instalado".to_string(),
            });
        }
        Err(e) => {
            informe.ganchos.push(InformeGancho {
                simbolo: simbolo.to_string(),
                modulo,
                estado: format!("error: {e}"),
            });
        }
    }
}

/// Envuelve el cuerpo de un hook: **nunca** deja que un panic atraviese los
/// frames de C de gum (un panic desenrollando por C es UB).
fn protegido(f: impl FnOnce()) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
}

/// Emite un evento en el NDJSON (si el escritor está activo).
fn evento(tid: u32, hook: &str, evento: &str, extra: &[String]) {
    if sink::en_escritura() {
        return;
    }
    let mut campos = vec![json::campo("hook", hook), json::campo("event", evento)];
    campos.extend_from_slice(extra);
    sink::Sink::global().evento(tid, &json::une(&campos));
}

// ---------------------------------------------------------------------------
// FICHEROS
// ---------------------------------------------------------------------------

/// Hook de `open` / `openat` / `unlink`: registra la ruta y la acción.
struct FileListener {
    simbolo: &'static str,
    /// Índice del argumento que contiene la ruta (0 = `open`, 1 = `openat`).
    indice: u32,
}

impl InvocationListener for FileListener {
    fn on_enter(&mut self, ctx: InvocationContext) {
        if sink::en_escritura() {
            return;
        }
        protegido(|| {
            let Some(ruta) = sys::lee_cstr(ctx.arg(self.indice), 4096) else {
                return;
            };
            let extra = vec![json::campo("ruta", &redact::redacta_corto(&ruta))];
            evento(ctx.thread_id(), "ficheros", self.simbolo, &extra);
        });
    }

    fn on_leave(&mut self, _ctx: InvocationContext) {}
}

// ---------------------------------------------------------------------------
// RED
// ---------------------------------------------------------------------------

/// Hook de `connect`: registra `host:puerto` (sirve para VERIFICAR el parche
/// del endpoint).
struct ConnectListener;

impl InvocationListener for ConnectListener {
    fn on_enter(&mut self, ctx: InvocationContext) {
        if sink::en_escritura() {
            return;
        }
        protegido(|| {
            let Some((host, puerto)) = sys::sockaddr_destino(ctx.arg(1)) else {
                return;
            };
            let extra = vec![
                json::campo("host", &redact::redacta_corto(&host)),
                json::campo_num("puerto", puerto as u64),
            ];
            evento(ctx.thread_id(), "red", "connect", &extra);
        });
    }

    fn on_leave(&mut self, _ctx: InvocationContext) {}
}

/// Hook de `sendto`: registra el destino del datagrama.
struct SendToListener;

impl InvocationListener for SendToListener {
    fn on_enter(&mut self, ctx: InvocationContext) {
        if sink::en_escritura() {
            return;
        }
        protegido(|| {
            let sa = ctx.arg(4);
            if sa == 0 {
                return;
            }
            let Some((host, puerto)) = sys::sockaddr_destino(sa) else {
                return;
            };
            let extra = vec![
                json::campo("host", &redact::redacta_corto(&host)),
                json::campo_num("puerto", puerto as u64),
            ];
            evento(ctx.thread_id(), "red", "sendto", &extra);
        });
    }

    fn on_leave(&mut self, _ctx: InvocationContext) {}
}

/// Hook de `getaddrinfo`: registra la resolución de nombre (host + servicio).
struct GetAddrInfoListener;

impl InvocationListener for GetAddrInfoListener {
    fn on_enter(&mut self, ctx: InvocationContext) {
        if sink::en_escritura() {
            return;
        }
        protegido(|| {
            let nodo = sys::lee_cstr(ctx.arg(0), 1024).unwrap_or_default();
            let servicio = sys::lee_cstr(ctx.arg(1), 64).unwrap_or_default();
            if nodo.is_empty() && servicio.is_empty() {
                return;
            }
            let extra = vec![
                json::campo("host", &redact::redacta_corto(&nodo)),
                json::campo("servicio", &redact::redacta_corto(&servicio)),
            ];
            evento(ctx.thread_id(), "red", "getaddrinfo", &extra);
        });
    }

    fn on_leave(&mut self, _ctx: InvocationContext) {}
}

// ---------------------------------------------------------------------------
// SQLITE
// ---------------------------------------------------------------------------

/// Hook de `sqlite3_open_v2`: registra el fichero de base de datos.
struct SqliteOpenListener;

impl InvocationListener for SqliteOpenListener {
    fn on_enter(&mut self, ctx: InvocationContext) {
        if sink::en_escritura() {
            return;
        }
        protegido(|| {
            let Some(fichero) = sys::lee_cstr(ctx.arg(0), 4096) else {
                return;
            };
            let extra = vec![json::campo("fichero", &redact::redacta_corto(&fichero))];
            evento(ctx.thread_id(), "sqlite", "open", &extra);
        });
    }

    fn on_leave(&mut self, _ctx: InvocationContext) {}
}

/// Hook de `sqlite3_prepare_v2`: registra la consulta (REDACTADA) y la
/// asocia al `sqlite3_stmt*` que devuelve.
struct SqlitePrepareListener;

impl InvocationListener for SqlitePrepareListener {
    fn on_enter(&mut self, ctx: InvocationContext) {
        if sink::en_escritura() {
            return;
        }
        protegido(|| {
            let sql = sys::lee_cstr(ctx.arg(1), 8192);
            SQL_PENDIENTE.with(|c| *c.borrow_mut() = sql);
        });
    }

    fn on_leave(&mut self, ctx: InvocationContext) {
        if sink::en_escritura() {
            return;
        }
        protegido(|| {
            let sql = SQL_PENDIENTE.with(|c| c.borrow_mut().take());
            let Some(sql) = sql else {
                return;
            };
            let limpia = redact::redacta_sql(&sql);
            // `ppStmt` (argumento 3) ya está escrito al salir de `prepare`.
            let stmt = leer_puntero(ctx.arg(3));
            if stmt != 0 {
                let mut mapa = match mapa_stmt().lock() {
                    Ok(g) => g,
                    Err(e) => e.into_inner(),
                };
                if mapa.len() > 1024 {
                    mapa.clear();
                }
                mapa.insert(stmt, limpia.clone());
            }
            let mut extra = vec![json::campo("sql", &limpia)];
            if let Some(tabla) = tabla_de(&limpia) {
                extra.push(json::campo("tabla", &tabla));
            }
            evento(ctx.thread_id(), "sqlite", "prepare", &extra);
        });
    }
}

/// Hook de `sqlite3_step`: registra la tabla que se está tocando.
struct SqliteStepListener;

impl InvocationListener for SqliteStepListener {
    fn on_enter(&mut self, ctx: InvocationContext) {
        if sink::en_escritura() {
            return;
        }
        protegido(|| {
            let stmt = ctx.arg(0);
            let sql = {
                let mapa = match mapa_stmt().lock() {
                    Ok(g) => g,
                    Err(e) => e.into_inner(),
                };
                mapa.get(&stmt).cloned()
            };
            let mut extra: Vec<String> = Vec::new();
            if let Some(sql) = sql {
                if let Some(tabla) = tabla_de(&sql) {
                    extra.push(json::campo("tabla", &tabla));
                }
            }
            evento(ctx.thread_id(), "sqlite", "step", &extra);
        });
    }

    fn on_leave(&mut self, _ctx: InvocationContext) {}
}

/// Hook de `sqlite3_finalize`: limpia el mapa de sentencias.
struct SqliteFinalizeListener;

impl InvocationListener for SqliteFinalizeListener {
    fn on_enter(&mut self, ctx: InvocationContext) {
        if sink::en_escritura() {
            return;
        }
        protegido(|| {
            let stmt = ctx.arg(0);
            let mut mapa = match mapa_stmt().lock() {
                Ok(g) => g,
                Err(e) => e.into_inner(),
            };
            mapa.remove(&stmt);
        });
    }

    fn on_leave(&mut self, _ctx: InvocationContext) {}
}

/// Lee un puntero (un `sqlite3_stmt**` en `prepare`).
fn leer_puntero(ptr: usize) -> usize {
    if ptr < 0x1000 {
        return 0;
    }
    // SAFETY: lectura de un puntero de 8 bytes; `ptr` es el `ppStmt` que
    // la propia SQLite escribió.
    unsafe { std::ptr::read_unaligned(ptr as *const usize) }
}

/// Tabla que toca una consulta (`from`/`into`/`update`/`join`).
fn tabla_de(sql: &str) -> Option<String> {
    let bajo = sql.to_ascii_lowercase();
    for clave in ["from ", "into ", "update ", "join "] {
        if let Some(i) = bajo.find(clave) {
            let resto = &sql[i + clave.len()..];
            let ident: String = resto
                .trim_start()
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                .collect();
            if !ident.is_empty() {
                return Some(ident);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extrae_la_tabla_de_la_consulta() {
        assert_eq!(
            tabla_de("SELECT * FROM conversations WHERE id=?").as_deref(),
            Some("conversations")
        );
        assert_eq!(
            tabla_de("insert into blobs (a) values (1)").as_deref(),
            Some("blobs")
        );
        assert_eq!(tabla_de("SELECT 1").as_deref(), None);
    }

    // ---------------------------------------------------------------------
    // ¿DISPARA GUM `on_leave`?
    //
    // MEDIDO (2026-11-26, móvil, LS de Antigravity): el NDJSON de hooks
    // tenía 473 líneas y **CERO** con el campo `ret` (el que escribe
    // `PlanListener::on_leave`) ⇒ sólo se registraba la ENTRADA de cada
    // llamada y nunca el retorno.  Sin el retorno no se puede leer el
    // valor que devuelve la función (p. ej. la URL del endpoint).
    //
    // Este test mide el comportamiento de gum en el host: si `on_leave` no
    // se dispara, el problema NO es del payload sino del enganche.
    // ---------------------------------------------------------------------

    use std::ffi::c_void;
    use std::sync::atomic::{AtomicU64, Ordering};

    // MEDIDO (2026-11-26, CI): con contadores GLOBALES los tests se
    // contaminaban (uno reseteaba el contador mientras otro lo leía) ⇒
    // fallo en el CI y no en local.  Cada test tiene los suyos.
    static ENTER_A: AtomicU64 = AtomicU64::new(0);
    static LEAVE_A: AtomicU64 = AtomicU64::new(0);
    static ENTER_B: AtomicU64 = AtomicU64::new(0);
    static LEAVE_B: AtomicU64 = AtomicU64::new(0);

    /// Función a hookear: `#[inline(never)]` para que gum la pueda enganchar.
    ///
    /// `black_box`: MEDIDO (2026-11-26, host) — sin él, LLVM **elimina la
    /// llamada** (la función es interna, sin efectos y su resultado es
    /// constante) ⇒ el hook no se disparaba nunca y el test era falso.
    #[inline(never)]
    extern "C" fn objetivo(x: u64) -> u64 {
        std::hint::black_box(x) + 7
    }

    /// SEGUNDA función a hookear: cada test usa una distinta porque enganchar
    /// dos veces la MISMA en el mismo proceso rompe el trampolín de gum
    /// (medido: SIGABRT con una reserva gigante).
    #[inline(never)]
    extern "C" fn objetivo2(x: u64) -> u64 {
        std::hint::black_box(x) + 9
    }

    /// TERCERA función: devuelve una `&str` (en Rust el ABI devuelve
    /// (puntero, longitud) IGUAL que una `string` de Go ⇒ sirve para probar
    /// la lectura del retorno).
    ///
    /// `black_box` en el argumento: sin él, LLVM **elimina la llamada**
    /// (función interna, sin efectos, resultado constante) ⇒ el hook no se
    /// disparaba nunca y el test era falso (medido: ENTER=0).
    #[inline(never)]
    #[no_mangle]
    pub extern "C" fn objetivo3(x: u64) -> &'static str {
        if std::hint::black_box(x) == 7 {
            "https://endpoint.de.prueba/v1/x"
        } else {
            "https://otro.de.prueba/"
        }
    }

    /// CUARTA función (otra `&str`): cada test hookea una DISTINTA porque
    /// enganchar dos veces la misma en el mismo proceso rompe el trampolín
    /// de gum (medido: SIGABRT con una reserva gigante) y los tests del CI
    /// se ejecutan en PARALELO.
    #[inline(never)]
    #[no_mangle]
    pub extern "C" fn objetivo4(x: u64) -> &'static str {
        if std::hint::black_box(x) == 7 {
            "https://otro.de.prueba/v1/x"
        } else {
            "https://cuarto.de.prueba/"
        }
    }

    /// Contador de entradas/salidas, con sus PROPIOS contadores.
    struct Contador {
        enter: &'static AtomicU64,
        leave: &'static AtomicU64,
    }

    impl Contador {
        fn nuevo(enter: &'static AtomicU64, leave: &'static AtomicU64) -> Self {
            Self { enter, leave }
        }
    }

    impl InvocationListener for Contador {
        fn on_enter(&mut self, _ctx: InvocationContext) {
            self.enter.fetch_add(1, Ordering::SeqCst);
        }
        fn on_leave(&mut self, _ctx: InvocationContext) {
            self.leave.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// El escritor NDJSON es un SINGLTON global ⇒ dos tests que lo abran
    /// con rutas distintas se pisan (el último `abrir` cambia el `fd`).
    /// MEDIDO (2026-11-26, CI): por eso los tests que lo usan se serializan
    /// con este cerrojo.
    static CERROJO_SINK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn cerrojo_sink() -> std::sync::MutexGuard<'static, ()> {
        match CERROJO_SINK.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        }
    }

    #[test]
    fn gum_llama_a_on_leave_al_salir_de_la_funcion() {
        let mut inter = crate::interceptor_bloqueado();
        let addr = objetivo as extern "C" fn(u64) -> u64;
        let ptr = frida_gum::NativePointer(addr as *mut c_void);
        let mut l = Contador::nuevo(&ENTER_A, &LEAVE_A);
        let handle = inter
            .0
            .attach(ptr, &mut l)
            .expect("gum no pudo enganchar la función de prueba");
        let r = objetivo(35);
        assert_eq!(r, 42, "la función hookeada no se puede llamar");
        assert_eq!(ENTER_A.load(Ordering::SeqCst), 1, "on_enter no se disparó");
        assert_eq!(
            LEAVE_A.load(Ordering::SeqCst),
            1,
            "on_leave NO se disparó (gum no llama al callback de salida)"
        );
        drop(handle);
    }

    /// El MISMO `PlanListener` del payload, contra un NDJSON temporal: tiene
    /// que escribir una línea de ENTRADA (`args`) y otra de SALIDA (`ret`).
    ///
    /// MEDIDO (2026-11-26, móvil, LS de Antigravity): en el LS sólo se
    /// escribía la de entrada ⇒ el retorno de la función (p. ej. la URL del
    /// endpoint) nunca se veía.
    #[test]
    fn el_plan_listener_escribe_el_retorno_de_la_llamada() {
        let _cerrojo = cerrojo_sink();
        let mut inter = crate::interceptor_bloqueado();
        let dir = std::env::temp_dir().join(format!("md_plan_listener_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("eventos.ndjson");
        let _ = std::fs::remove_file(&ruta);
        let cand = vec![ruta.to_string_lossy().to_string()];
        let abierta = sink::Sink::global().abrir(&cand).expect("no se pudo abrir el NDJSON");
        assert_eq!(abierta, cand[0]);

        let addr = objetivo2 as extern "C" fn(u64) -> u64;
        let ptr = frida_gum::NativePointer(addr as *mut c_void);
        let mut l = PlanListener {
            id: "objetivo".to_string(),
            tarea: "observar".to_string(),
            valor: None,
        };
        let handle = inter
            .0
            .attach(ptr, &mut l)
            .expect("gum no pudo enganchar la función de prueba");
        let r = objetivo2(33);
        assert_eq!(r, 42);
        drop(handle);

        let txt = std::fs::read_to_string(&ruta).unwrap_or_default();
        let lineas: Vec<&str> = txt.lines().filter(|l| !l.trim().is_empty()).collect();
        assert!(
            lineas.iter().any(|l| l.contains("\"args\"")),
            "no se escribió la línea de ENTRADA: {txt}"
        );
        assert!(
            lineas.iter().any(|l| l.contains("\"ret\"")),
            "no se escribió la línea de SALIDA (ret): {txt}"
        );
        let _ = std::fs::remove_file(&ruta);
    }

    /// Una función que devuelve una `string` (ptr+len) también dispara el
    /// `on_leave` (medido: el `ret` depende de él).
    #[test]
    fn una_funcion_que_devuelve_una_string_tambien_dispara_on_leave() {
        let mut inter = crate::interceptor_bloqueado();
        let addr = objetivo3 as extern "C" fn(u64) -> &'static str;
        let ptr = frida_gum::NativePointer(addr as *mut c_void);
        let mut l = Contador::nuevo(&ENTER_B, &LEAVE_B);
        let h = inter.0.attach(ptr, &mut l).expect("attach");
        let r = objetivo3(7);
        assert_eq!(r, "https://endpoint.de.prueba/v1/x");
        assert_eq!(ENTER_B.load(Ordering::SeqCst), 1, "on_enter no se disparó");
        assert_eq!(LEAVE_B.load(Ordering::SeqCst), 1, "on_leave no se dispara");
        drop(h);
    }

    /// La `string` que devuelve la función (ptr+len, como en Go) tiene que
    /// leerse y escribirse en el NDJSON.
    ///
    /// MEDIDO (2026-11-26, móvil): `GetEndpointURL` devuelve la URL del
    /// endpoint; sin esto no se ve QUÉ URL usa el LS.
    #[test]
    fn el_plan_listener_lee_la_string_que_devuelve_la_funcion() {
        // El escritor NDJSON es global: este cerrojo serializa los tests que
        // lo usan (MEDIDO 2026-11-26 en CI: sin él, los dos tests se pisan
        // el `fd` y el que pierde falla).
        let _cerrojo = cerrojo_sink();
        // El interceptor también es global: se toma ANTES de abrir el NDJSON
        // para que el `fd` no lo cambie otro test mientras éste corre.
        let mut inter = crate::interceptor_bloqueado();
        let dir = std::env::temp_dir().join(format!("md_plan_str_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("eventos.ndjson");
        let _ = std::fs::remove_file(&ruta);
        let cand = vec![ruta.to_string_lossy().to_string()];
        sink::Sink::global()
            .abrir(&cand)
            .expect("no se pudo abrir el NDJSON");

        let addr = objetivo4 as extern "C" fn(u64) -> &'static str;
        let ptr = frida_gum::NativePointer(addr as *mut c_void);
        let mut l = PlanListener {
            id: "endpoint".to_string(),
            tarea: "observar".to_string(),
            valor: None,
        };
        let handle = inter
            .0
            .attach(ptr, &mut l)
            .expect("gum no pudo enganchar la función de prueba");
        let r = objetivo4(7);
        assert_eq!(r, "https://otro.de.prueba/v1/x");
        drop(handle);
        let txt = std::fs::read_to_string(&ruta).unwrap_or_default();
        assert!(
            txt.contains("\"ret_str\""),
            "no se escribió el retorno como texto: {txt}"
        );
        assert!(
            txt.contains("otro.de.prueba"),
            "el texto del retorno no es el esperado: {txt}"
        );
        let _ = std::fs::remove_file(&ruta);
    }
}


// ---------------------------------------------------------------------------
// PLAN DE HOOKS GO (direcciones del pclntab)
// ---------------------------------------------------------------------------

/// Listener de un hook del plan: registra la llamada (argumentos y retorno)
/// en el NDJSON.  **No toca el flujo**: el `.so` real sigue ejecutándose
/// igual (esto es "dejar pasar" al original).
pub struct PlanListener {
    /// Nombre corto del hook (sale en el NDJSON).
    pub id: String,
    /// Qué se hace con la llamada (`observar`, `sustituir_url`).
    pub tarea: String,
    /// Valor a devolver (hex) si la tarea sustituye.
    pub valor: Option<Vec<u8>>,
}

impl InvocationListener for PlanListener {
    fn on_enter(&mut self, ctx: InvocationContext) {
        if sink::en_escritura() {
            return;
        }
        let tid = ctx.thread_id();
        let id = self.id.clone();
        let tarea = self.tarea.clone();
        protegido(|| {
            let args: Vec<String> = (0..6).map(|i| format!("0x{:x}", ctx.arg(i))).collect();
            let extra = vec![
                json::campo("id", &id),
                json::campo("tarea", &tarea),
                json::campo("args", &args.join(" ")),
            ];
            evento(tid, "plan", &id, &extra);
        });
    }

    fn on_leave(&mut self, ctx: InvocationContext) {
        if sink::en_escritura() {
            return;
        }
        let tid = ctx.thread_id();
        let id = self.id.clone();
        let tarea = self.tarea.clone();
        let valor = self.valor.clone();
        protegido(|| {
            // MEDIDO (2026-11-26, móvil, LS de Antigravity): el NDJSON tenía
            // 473 líneas y NINGUNA con `ret` ⇒ no se veía el retorno de
            // ninguna función.  Aquí se escribe PRIMERO una línea mínima
            // (marcador `leave`) para poder distinguir "on_leave no se
            // dispara" de "el retorno no se pudo leer".
            let marca = vec![
                json::campo("id", &id),
                json::campo("tarea", &tarea),
                json::campo("leave", "1"),
            ];
            evento(tid, "plan_leave", &id, &marca);

            let rv = ctx.return_value();
            // MEDIDO (2026-11-26, host): `ctx.arg(n)` devuelve los registros
            // de ENTRADA incluso en el punto de SALIDA ⇒ el retorno de una
            // `string` (puntero + longitud) NO se podía leer con él.  Hay que
            // leer el CONTEXTO DE CPU: x0/x1 en AArch64, rax/rdx en x86_64.
            let (ptr, len) = registros_retorno(&ctx);
            let mut extra = vec![
                json::campo("id", &id),
                json::campo("tarea", &tarea),
                json::campo("ret", &format!("0x{:x}", rv)),
                json::campo("ret_len", &format!("{len}")),
            ];
            // El retorno de una `string` de Go son DOS registros: el puntero
            // (x0) y la longitud (x1).  El payload vive DENTRO del proceso
            // instrumentado ⇒ puede leer esos bytes directamente.
            // MEDIDO (2026-11-26): es lo que devuelve la URL del endpoint.
            if let Some(s) = lee_string_go(ptr, len) {
                extra.push(json::campo("ret_str", &redact::redacta_corto(&s)));
            }
            if let Some(v) = valor {
                // El retorno de una `string` de Go son DOS registros:
                // el puntero (x0) y la longitud (x1).  Escribimos los bytes
                // nuevos en memoria del proceso y cambiamos los dos.
                let largo_viejo = len;
                match escribir_en_el_proceso(&v) {
                    Some(p) => {
                        ctx.set_return_value(p);
                        // El segundo registro (longitud) sólo existe en el
                        // ABI de Go (AArch64).  En x86_64 el C ABI no lo usa.
                        // MEDIDO (2026-11-26): hay que tocar el REGISTRO (x1),
                        // no `arg(1)` (que es el registro de ENTRADA).
                        #[cfg(target_arch = "aarch64")]
                        {
                            let mut cc = ctx.cpu_context();
                            let _ = cc.set_reg(1, v.len() as u64);
                        }
                        extra.push(json::campo("sustituto", &format!("0x{:x}", p)));
                        extra.push(json::campo("len_viejo", &format!("{largo_viejo}")));
                        extra.push(json::campo("len_nuevo", &format!("{}", v.len())));
                    }
                    None => {
                        extra.push(json::campo("sustitucion", "no se pudo escribir en memoria"));
                    }
                }
            }
            evento(tid, "plan", &id, &extra);
        });
    }
}

/// Los DOS registros del RETORNO: (puntero, longitud) en el ABI de Go.
///
/// MEDIDO (2026-11-26, host): `ctx.arg(n)` lee los registros de ENTRADA
/// incluso en el punto de SALIDA ⇒ con `arg(0)/arg(1)` se leía el
/// ARGUMENTO y no el retorno (y por eso no aparecía ningún `ret_str`).
/// El contexto de CPU sí tiene el valor de retorno: x0/x1 en AArch64 y
/// rax/rdx en x86_64.
fn registros_retorno(ctx: &InvocationContext) -> (usize, usize) {
    let cc = ctx.cpu_context();
    #[cfg(target_arch = "aarch64")]
    {
        (cc.reg(0) as usize, cc.reg(1) as usize)
    }
    #[cfg(target_arch = "x86_64")]
    {
        (cc.rax() as usize, cc.rdx() as usize)
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        let _ = cc;
        (ctx.return_value(), 0)
    }
}

/// Lee una `string` de Go (puntero + longitud) de la memoria del proceso.
///
/// MEDIDO (2026-11-26, móvil): `GetEndpointURL` devuelve la URL del endpoint
/// como `string` (x0=puntero, x1=longitud).  El payload se ejecuta DENTRO del
/// proceso instrumentado, así que puede leer los bytes sin ptrace.
///
/// **Seguridad**: los registros de retorno de una función que NO devuelve una
/// string son basura (en el test de host, `rdx` valía cualquier cosa) ⇒
/// leerlos a pelo provoca un SIGSEGV (medido).  Por eso se lee de
/// `/proc/self/mem` con `pread`: si la dirección no está mapeada, `pread`
/// devuelve -1 (EIO) y no hay señal.
fn lee_string_go(ptr: usize, len: usize) -> Option<String> {
    if ptr == 0 || len == 0 || len > 8192 {
        return None;
    }
    let fd = fd_memoria()?;
    let mut buf = vec![0u8; len];
    let n = unsafe {
        libc::pread(
            fd,
            buf.as_mut_ptr() as *mut libc::c_void,
            len,
            ptr as libc::off_t,
        )
    };
    if n != len as isize {
        return None;
    }
    std::str::from_utf8(&buf).ok().map(|s| s.to_string())
}

/// El descriptor de `/proc/self/mem` (se abre una vez y se reutiliza).
fn fd_memoria() -> Option<i32> {
    static FD: std::sync::OnceLock<i32> = std::sync::OnceLock::new();
    let fd = *FD.get_or_init(|| unsafe {
        libc::open(
            b"/proc/self/mem\0".as_ptr() as *const libc::c_char,
            libc::O_RDONLY | libc::O_CLOEXEC,
        )
    });
    if fd < 0 {
        None
    } else {
        Some(fd)
    }
}

/// Escribe bytes en memoria del proceso y devuelve la direccion.
///
/// `mmap` del propio proceso: la memoria es real para el binario objetivo
/// (el Go runtime la lee como un `string` normal), y no la recicla el GC
/// porque no es memoria gestionada por Go.
fn escribir_en_el_proceso(bytes: &[u8]) -> Option<usize> {
    if bytes.is_empty() {
        return None;
    }
    let largo = bytes.len().max(16);
    unsafe {
        let p = libc::mmap(
            std::ptr::null_mut(),
            largo,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        );
        if p == libc::MAP_FAILED || p.is_null() {
            return None;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p as *mut u8, bytes.len());
        Some(p as usize)
    }
}

/// Instala los hooks del plan: cada línea dice en qué módulo está la
/// función y su VA en el ELF; aquí se le suma el base del módulo **en el
/// proceso vivo** (el bias de carga) y se engancha con gum.
///
/// El `.so` original **no se modifica**: gum intercepta en memoria, así que
/// la app sigue usando su binario intacto.
pub fn instala_plan(
    interceptor: &mut Interceptor,
    gum: &Gum,
    ruta_plan: &str,
    informe: &mut Informe,
) {
    let objetivos = crate::plan::lee(ruta_plan);
    if objetivos.is_empty() {
        return;
    }
    let process = frida_gum::Process::obtain(gum);
    let mods = process.enumerate_modules();
    for o in objetivos {
        let base_nombre = o.modulo.rsplit('/').next().unwrap_or(&o.modulo).to_string();
        let Some(m) = mods
            .iter()
            .find(|m| m.path() == o.modulo)
            .or_else(|| mods.iter().find(|m| m.name() == base_nombre))
        else {
            informe.ganchos.push(InformeGancho {
                simbolo: o.id.clone(),
                modulo: o.modulo.clone(),
                estado: "modulo ausente".to_string(),
            });
            continue;
        };
        // frida-gum 0.17: `NativePointer` no tiene `add`; es un newtype
        // sobre `*mut c_void` => se desplaza en aritmética de 64 bits.
        let base = m.range().base_address().0 as u64;
        let addr = frida_gum::NativePointer((base + o.va_elf) as *mut libc::c_void);
        let mut boxed: Box<PlanListener> = Box::new(PlanListener {
            id: o.id.clone(),
            tarea: o.tarea.clone(),
            valor: o.valor.clone(),
        });
        match interceptor.attach(addr, &mut *boxed) {
            Ok(handle) => {
                std::mem::forget(handle);
                let _ = Box::leak(boxed);
                informe.ganchos.push(InformeGancho {
                    simbolo: o.id.clone(),
                    modulo: m.path(),
                    estado: format!("instalado en 0x{:x}", addr),
                });
            }
            Err(e) => {
                informe.ganchos.push(InformeGancho {
                    simbolo: o.id.clone(),
                    modulo: m.path(),
                    estado: format!("error: {e}"),
                });
            }
        }
    }
}
