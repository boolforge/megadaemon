//! Payload de hooks: **lo que se inyecta** dentro del proceso de la app.
//!
//! Qué hace
//! --------
//! * **SQLite**: intercepta `sqlite3_open_v2`, `sqlite3_prepare_v2` y
//!   `sqlite3_step` y registra la ruta de la base y la consulta.
//! * **Red**: intercepta `connect`, `sendto` y `getaddrinfo` y registra
//!   **host:puerto**.  Esto es lo que **verifica el parche del endpoint**:
//!   si después de parchear sigue apareciendo
//!   `daily-cloudcode-pa.googleapis.com`, el parche no está aplicado.
//! * **Ficheros**: intercepta `open`, `openat` y `unlink` y registra ruta y
//!   acción.
//!
//! Todo va a un **NDJSON** (una línea por evento) con las credenciales
//! **redactadas** con la misma política que el shim del daemon
//! (`megadaemon-redact`).
//!
//! Por qué hay que **parchear el GOT** y no basta con exportar el símbolo
//! -----------------------------------------------------------------------
//! En Android el enlazador resuelve los símbolos **al cargar** la librería
//! (enlace inmediato): cuando el payload entra, las llamadas de la app ya
//! apuntan a la implementación original ⇒ exportar `sqlite3_prepare_v2` no
//! cambiaría nada.  Lo que sí funciona es **reescribir la ranura del GOT** de
//! cada módulo que importa ese símbolo: a partir de ahí, todas las llamadas
//! de ese módulo pasan por nuestro hook.
//!
//! El payload **no modifica los datos**: sólo lee y registra.  Y está hecho
//! para que un fallo suyo **no tumbe** la app:
//!
//! * todas las llamadas al sistema que usa el propio payload van por
//!   `syscall()` directo (nunca por el PLT) ⇒ no se auto-intercepta;
//! * el constructor va envuelto en `catch_unwind`;
//! * si un símbolo no se puede resolver, ese gancho **no se instala**;
//! * si una ranura del GOT no se puede parchear, se salta y se sigue.
//!
//! Compilación: `cargo build --release --target aarch64-linux-android`
//! (el CI lo hace; ver `.github/workflows/build.yml`).

mod elf;
mod log;

use libc::{c_char, c_int, c_void, size_t, sockaddr, socklen_t};
use std::sync::OnceLock;

// ========================================================================= //
// Originales (dlsym)
// ========================================================================= //

type FnSqliteOpenV2 = unsafe extern "C" fn(*const c_char, *mut *mut c_void, c_int, *const c_char) -> c_int;
type FnSqlitePrepareV2 =
    unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut *mut c_void, *mut *const c_char) -> c_int;
type FnSqliteStep = unsafe extern "C" fn(*mut c_void) -> c_int;
type FnConnect = unsafe extern "C" fn(c_int, *const sockaddr, socklen_t) -> c_int;
type FnSendto =
    unsafe extern "C" fn(c_int, *const c_void, size_t, c_int, *const sockaddr, socklen_t) -> isize;
type FnGetaddrinfo =
    unsafe extern "C" fn(*const c_char, *const c_char, *const libc::addrinfo, *mut *mut libc::addrinfo) -> c_int;
type FnOpen = unsafe extern "C" fn(*const c_char, c_int, libc::c_uint) -> c_int;
type FnOpenat = unsafe extern "C" fn(c_int, *const c_char, c_int, libc::c_uint) -> c_int;
type FnUnlink = unsafe extern "C" fn(*const c_char) -> c_int;

/// Las implementaciones originales (resueltas con `dlsym`).
#[derive(Default)]
struct Originales {
    sqlite3_open_v2: Option<FnSqliteOpenV2>,
    sqlite3_prepare_v2: Option<FnSqlitePrepareV2>,
    sqlite3_step: Option<FnSqliteStep>,
    connect: Option<FnConnect>,
    sendto: Option<FnSendto>,
    getaddrinfo: Option<FnGetaddrinfo>,
    open: Option<FnOpen>,
    openat: Option<FnOpenat>,
    unlink: Option<FnUnlink>,
}

static ORIGINALES: OnceLock<Originales> = OnceLock::new();

/// Resuelve un símbolo **original**.
///
/// Se prueba `RTLD_NEXT` (el "siguiente" en el orden de búsqueda, que es lo
/// que corresponde cuando nuestra librería está en el espacio global) y, si
/// no aparece, `RTLD_DEFAULT`.  Si el puntero devuelto es **el nuestro**
/// (nuestra definición ganaría el enlace), se descarta: llamarse a sí mismo
/// es un bucle infinito.
fn resolver<T: Copy>(nombre: &str, evitar: *const c_void) -> Option<T> {
    let c = std::ffi::CString::new(nombre).ok()?;
    unsafe {
        let mut p = libc::dlsym(libc::RTLD_NEXT, c.as_ptr());
        if p.is_null() {
            p = libc::dlsym(libc::RTLD_DEFAULT, c.as_ptr());
        }
        if p.is_null() {
            return None;
        }
        if p as *const c_void == evitar {
            return None;
        }
        Some(std::mem::transmute(p))
    }
}

fn originales() -> &'static Originales {
    ORIGINALES.get_or_init(|| {
        // Las direcciones de nuestros ganchos: sirven para no resolverlos a
        // nosotros mismos.
        let p_prepare: unsafe extern "C" fn(
            *mut c_void,
            *const c_char,
            c_int,
            *mut *mut c_void,
            *mut *const c_char,
        ) -> c_int = sqlite3_prepare_v2;
        let p_step: unsafe extern "C" fn(*mut c_void) -> c_int = sqlite3_step;
        let p_open_v2: unsafe extern "C" fn(
            *const c_char,
            *mut *mut c_void,
            c_int,
            *const c_char,
        ) -> c_int = sqlite3_open_v2;
        let p_connect: unsafe extern "C" fn(c_int, *const sockaddr, socklen_t) -> c_int = connect;
        let p_sendto: unsafe extern "C" fn(
            c_int,
            *const c_void,
            size_t,
            c_int,
            *const sockaddr,
            socklen_t,
        ) -> isize = sendto;
        let p_getaddrinfo: unsafe extern "C" fn(
            *const c_char,
            *const c_char,
            *const libc::addrinfo,
            *mut *mut libc::addrinfo,
        ) -> c_int = getaddrinfo;
        let p_open: unsafe extern "C" fn(*const c_char, c_int, libc::c_uint) -> c_int = open;
        let p_openat: unsafe extern "C" fn(c_int, *const c_char, c_int, libc::c_uint) -> c_int =
            openat;
        let p_unlink: unsafe extern "C" fn(*const c_char) -> c_int = unlink;

        Originales {
            sqlite3_open_v2: resolver("sqlite3_open_v2", p_open_v2 as *const () as *const c_void),
            sqlite3_prepare_v2: resolver(
                "sqlite3_prepare_v2",
                p_prepare as *const () as *const c_void,
            ),
            sqlite3_step: resolver("sqlite3_step", p_step as *const () as *const c_void),
            connect: resolver("connect", p_connect as *const () as *const c_void),
            sendto: resolver("sendto", p_sendto as *const () as *const c_void),
            getaddrinfo: resolver("getaddrinfo", p_getaddrinfo as *const () as *const c_void),
            open: resolver("open", p_open as *const () as *const c_void),
            openat: resolver("openat", p_openat as *const () as *const c_void),
            unlink: resolver("unlink", p_unlink as *const () as *const c_void),
        }
    })
}

// ========================================================================= //
// Utilidades
// ========================================================================= //

/// Lee una cadena C (acotada): devuelve `None` si el puntero es nulo o
/// la cadena está vacía.
fn leer_cstr(p: *const c_char, max: usize) -> Option<String> {
    if p.is_null() {
        return None;
    }
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < max {
        let b = unsafe { *p.add(i) } as u8;
        if b == 0 {
            break;
        }
        out.push(b);
        i += 1;
    }
    if out.is_empty() {
        return None;
    }
    Some(String::from_utf8_lossy(&out).to_string())
}

/// Recorta un detalle para que el NDJSON no crezca sin límite.
fn recortar(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

/// ¿Es una ruta "interna" que no interesa observar?
fn ruta_interna(ruta: &str) -> bool {
    const INTERNAS: &[&str] = &[
        "/proc/",
        "/sys/",
        "/dev/",
        "/apex/",
        "/system/",
        "/vendor/",
        "/data/local/tmp/megadaemon",
        "/megadaemon/hooks",
    ];
    INTERNAS.iter().any(|p| ruta.starts_with(p) || ruta.contains(p))
}

/// Lee un puerto en big-endian sin desalinear la memoria.
fn puerto_be(addr: *const sockaddr, off: usize) -> u16 {
    unsafe {
        let b = addr as *const u8;
        let hi = *b.add(off) as u16;
        let lo = *b.add(off + 1) as u16;
        (hi << 8) | lo
    }
}

/// Describe un `sockaddr` como `host:puerto` (o `unix:...`).
fn describir_sockaddr(addr: *const sockaddr, len: socklen_t) -> String {
    if addr.is_null() || (len as usize) < 8 {
        return "?".to_string();
    }
    unsafe {
        let sa = &*addr;
        match sa.sa_family as i32 {
            libc::AF_INET => {
                let p = puerto_be(addr, 2);
                let ip = *((addr as *const u8).add(4).cast::<[u8; 4]>());
                format!("{}:{}", std::net::Ipv4Addr::from(ip), p)
            }
            libc::AF_INET6 => {
                let p = puerto_be(addr, 2);
                let ip = *((addr as *const u8).add(8).cast::<[u8; 16]>());
                format!("[{}]:{}", std::net::Ipv6Addr::from(ip), p)
            }
            libc::AF_UNIX => "unix".to_string(),
            _ => format!("familia-{}", sa.sa_family),
        }
    }
}

// ========================================================================= //
// Ganchos
// ========================================================================= //

/// `sqlite3_open_v2`: ruta de la base de datos.
#[no_mangle]
pub unsafe extern "C" fn sqlite3_open_v2(
    filename: *const c_char,
    pp_db: *mut *mut c_void,
    flags: c_int,
    vfs: *const c_char,
) -> c_int {
    let ruta = leer_cstr(filename, 1024);
    log::evento("sqlite", "open", ruta.as_deref());
    match originales().sqlite3_open_v2 {
        Some(f) => f(filename, pp_db, flags, vfs),
        None => -1,
    }
}

/// `sqlite3_prepare_v2`: la consulta.
#[no_mangle]
pub unsafe extern "C" fn sqlite3_prepare_v2(
    db: *mut c_void,
    sql: *const c_char,
    n_byte: c_int,
    stmt: *mut *mut c_void,
    tail: *mut *const c_char,
) -> c_int {
    let texto = leer_cstr(sql, 4096).map(|s| recortar(&s, 400));
    log::evento("sqlite", "prepare", texto.as_deref());
    match originales().sqlite3_prepare_v2 {
        Some(f) => f(db, sql, n_byte, stmt, tail),
        None => -1,
    }
}

/// `sqlite3_step`: la ejecución de la consulta.
#[no_mangle]
pub unsafe extern "C" fn sqlite3_step(stmt: *mut c_void) -> c_int {
    log::evento("sqlite", "step", Some("stmt"));
    match originales().sqlite3_step {
        Some(f) => f(stmt),
        None => -1,
    }
}

/// `connect`: el host y el puerto de cada socket.
#[no_mangle]
pub unsafe extern "C" fn connect(sockfd: c_int, addr: *const sockaddr, len: socklen_t) -> c_int {
    let destino = describir_sockaddr(addr, len);
    log::evento("red", "connect", Some(&destino));
    match originales().connect {
        Some(f) => f(sockfd, addr, len),
        None => -1,
    }
}

/// `sendto`: el destino y el tamaño ( **nunca** el contenido: puede llevar la
/// credencial).
#[no_mangle]
pub unsafe extern "C" fn sendto(
    sockfd: c_int,
    buf: *const c_void,
    len: size_t,
    flags: c_int,
    dest: *const sockaddr,
    addrlen: socklen_t,
) -> isize {
    let destino = describir_sockaddr(dest, addrlen);
    log::evento("red", "sendto", Some(&format!("{} bytes -> {}", len, destino)));
    match originales().sendto {
        Some(f) => f(sockfd, buf, len, flags, dest, addrlen),
        None => -1,
    }
}

/// `getaddrinfo`: el **nombre** que la app resuelve.  Es el gancho que
/// verifica el parche del endpoint.
#[no_mangle]
pub unsafe extern "C" fn getaddrinfo(
    node: *const c_char,
    service: *const c_char,
    hints: *const libc::addrinfo,
    res: *mut *mut libc::addrinfo,
) -> c_int {
    let n = leer_cstr(node, 512);
    let s = leer_cstr(service, 64);
    let mut detalle = String::new();
    if let Some(n) = &n {
        detalle.push_str(n);
    }
    if let Some(s) = &s {
        if !detalle.is_empty() {
            detalle.push(':');
        }
        detalle.push_str(s);
    }
    log::evento("red", "getaddrinfo", Some(&detalle));
    match originales().getaddrinfo {
        Some(f) => f(node, service, hints, res),
        None => -1,
    }
}

/// `open`: fichero abierto.
#[no_mangle]
pub unsafe extern "C" fn open(path: *const c_char, flags: c_int, mode: libc::c_uint) -> c_int {
    if let Some(p) = leer_cstr(path, 1024) {
        if !ruta_interna(&p) {
            log::evento("ficheros", "open", Some(&recortar(&p, 512)));
        }
    }
    match originales().open {
        Some(f) => f(path, flags, mode),
        None => -1,
    }
}

/// `openat`: fichero abierto (con directorio de referencia).
#[no_mangle]
pub unsafe extern "C" fn openat(
    dirfd: c_int,
    path: *const c_char,
    flags: c_int,
    mode: libc::c_uint,
) -> c_int {
    if let Some(p) = leer_cstr(path, 1024) {
        if !ruta_interna(&p) {
            log::evento("ficheros", "openat", Some(&recortar(&p, 512)));
        }
    }
    match originales().openat {
        Some(f) => f(dirfd, path, flags, mode),
        None => -1,
    }
}

/// `unlink`: fichero borrado.
#[no_mangle]
pub unsafe extern "C" fn unlink(path: *const c_char) -> c_int {
    if let Some(p) = leer_cstr(path, 1024) {
        if !ruta_interna(&p) {
            log::evento("ficheros", "unlink", Some(&recortar(&p, 512)));
        }
    }
    match originales().unlink {
        Some(f) => f(path),
        None => -1,
    }
}

// ========================================================================= //
// Instalación (parcheo del GOT)
// ========================================================================= //

/// Una zona de memoria del proceso actual.
struct Zona {
    inicio: u64,
    fin: u64,
    ruta: String,
    permisos: String,
}

impl Zona {
    fn contiene(&self, addr: u64) -> bool {
        addr >= self.inicio && addr < self.fin
    }
}

/// Lee `/proc/self/maps`.
fn zonas_propio() -> Vec<Zona> {
    let mut out = Vec::new();
    let texto = match std::fs::read_to_string("/proc/self/maps") {
        Ok(t) => t,
        Err(_) => return out,
    };
    for linea in texto.lines() {
        let partes: Vec<&str> = linea.split_whitespace().collect();
        if partes.len() < 5 {
            continue;
        }
        let (ini, fin) = match partes[0].split_once('-') {
            Some((a, b)) => (
                u64::from_str_radix(a, 16).unwrap_or(0),
                u64::from_str_radix(b, 16).unwrap_or(0),
            ),
            None => continue,
        };
        let ruta = if partes.len() > 5 {
            partes[5..].join(" ")
        } else {
            String::new()
        };
        out.push(Zona {
            inicio: ini,
            fin: fin,
            ruta,
            permisos: partes[1].to_string(),
        });
    }
    out
}

/// Bias de carga de una librería (dirección de su primer segmento).
fn bias_de(zonas: &[Zona], ruta: &str) -> u64 {
    let mut mejor: Option<u64> = None;
    for z in zonas {
        if z.ruta != ruta {
            continue;
        }
        let v = z.inicio;
        mejor = Some(match mejor {
            Some(b) => b.min(v),
            None => v,
        });
    }
    mejor.unwrap_or(0)
}

/// Dirección de nuestro gancho para un nombre de símbolo (si lo tenemos).
fn destino_de(nombre: &str) -> Option<u64> {
    let p: Option<*const ()> = match nombre {
        "sqlite3_open_v2" => {
            let f: unsafe extern "C" fn(*const c_char, *mut *mut c_void, c_int, *const c_char) -> c_int =
                sqlite3_open_v2;
            Some(f as *const ())
        }
        "sqlite3_prepare_v2" => {
            let f: unsafe extern "C" fn(
                *mut c_void,
                *const c_char,
                c_int,
                *mut *mut c_void,
                *mut *const c_char,
            ) -> c_int = sqlite3_prepare_v2;
            Some(f as *const ())
        }
        "sqlite3_step" => {
            let f: unsafe extern "C" fn(*mut c_void) -> c_int = sqlite3_step;
            Some(f as *const ())
        }
        "connect" => {
            let f: unsafe extern "C" fn(c_int, *const sockaddr, socklen_t) -> c_int = connect;
            Some(f as *const ())
        }
        "sendto" => {
            let f: unsafe extern "C" fn(
                c_int,
                *const c_void,
                size_t,
                c_int,
                *const sockaddr,
                socklen_t,
            ) -> isize = sendto;
            Some(f as *const ())
        }
        "getaddrinfo" => {
            let f: unsafe extern "C" fn(
                *const c_char,
                *const c_char,
                *const libc::addrinfo,
                *mut *mut libc::addrinfo,
            ) -> c_int = getaddrinfo;
            Some(f as *const ())
        }
        "open" => {
            let f: unsafe extern "C" fn(*const c_char, c_int, libc::c_uint) -> c_int = open;
            Some(f as *const ())
        }
        "openat" => {
            let f: unsafe extern "C" fn(c_int, *const c_char, c_int, libc::c_uint) -> c_int = openat;
            Some(f as *const ())
        }
        "unlink" => {
            let f: unsafe extern "C" fn(*const c_char) -> c_int = unlink;
            Some(f as *const ())
        }
        _ => None,
    };
    p.map(|x| x as u64)
}

/// Escribe un puntero en una ranura del GOT (con `mprotect` si hace falta).
fn escribir_ranura(zonas: &[Zona], addr: u64, valor: u64) -> bool {
    let pagina = addr & !0xFFF;
    let zona = zonas.iter().find(|z| z.contiene(pagina));
    let writable = zona.map(|z| z.permisos.as_bytes().get(1) == Some(&b'w')).unwrap_or(false);
    let prot_final = if zona
        .map(|z| z.permisos.starts_with("r-x"))
        .unwrap_or(false)
    {
        (libc::PROT_READ | libc::PROT_EXEC) as i32
    } else {
        (libc::PROT_READ) as i32
    };
    if !writable {
        let r = unsafe {
            libc::mprotect(
                pagina as *mut c_void,
                4096,
                libc::PROT_READ | libc::PROT_WRITE,
            )
        };
        if r != 0 {
            return false;
        }
    }
    unsafe {
        let p = addr as *mut u64;
        std::ptr::write_volatile(p, valor);
    }
    if !writable {
        unsafe {
            libc::mprotect(pagina as *mut c_void, 4096, prot_final);
        }
    }
    true
}

/// Instala los ganchos parcheando el GOT de los módulos cargados.
fn parchear_got() -> (usize, usize) {
    let zonas = zonas_propio();
    // Rutas únicas de librerías (con su bias).
    let mut vistas: Vec<(String, u64)> = Vec::new();
    for z in &zonas {
        if z.ruta.is_empty() || !z.ruta.starts_with('/') {
            continue;
        }
        if vistas.iter().any(|(r, _)| *r == z.ruta) {
            continue;
        }
        vistas.push((z.ruta.clone(), bias_de(&zonas, &z.ruta)));
    }
    let mut parcheadas = 0usize;
    let mut modulos = 0usize;
    for (ruta, bias) in &vistas {
        let e = match elf::Elf::cargar(ruta) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let mut tocadas = 0usize;
        for rel in e.relocaciones() {
            let destino = match destino_de(&rel.nombre) {
                Some(d) => d,
                None => continue,
            };
            let ranura = bias + rel.offset;
            if escribir_ranura(&zonas, ranura, destino) {
                tocadas += 1;
            }
        }
        if tocadas > 0 {
            modulos += 1;
            parcheadas += tocadas;
        }
    }
    (parcheadas, modulos)
}

/// Punto de entrada: instala los ganchos.
///
/// Se puede llamar a mano (`dlsym("megadaemon_hooks_init")`) y además se
/// llama **automáticamente** al cargar la librería (`.init_array`).
#[no_mangle]
pub extern "C" fn megadaemon_hooks_init() {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let o = originales();
        let resueltos = [
            o.sqlite3_open_v2.is_some(),
            o.sqlite3_prepare_v2.is_some(),
            o.sqlite3_step.is_some(),
            o.connect.is_some(),
            o.sendto.is_some(),
            o.getaddrinfo.is_some(),
            o.open.is_some(),
            o.openat.is_some(),
            o.unlink.is_some(),
        ]
        .iter()
        .filter(|x| **x)
        .count();
        log::evento(
            "meta",
            "arranque",
            Some(&format!(
                "payload cargado: {} originales resueltos, proceso={}",
                resueltos,
                std::process::id()
            )),
        );
        let (parcheadas, modulos) = parchear_got();
        log::evento(
            "meta",
            "parche",
            Some(&format!(
                "{} ranuras del GOT parcheadas en {} módulos",
                parcheadas, modulos
            )),
        );
    }));
}

/// Constructor: se ejecuta al hacer `dlopen` de la librería.
#[used]
#[link_section = ".init_array"]
static CONSTRUCTOR: extern "C" fn() = megadaemon_hooks_init;

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_nueve_ganchos_estan_declarados() {
        for n in [
            "sqlite3_open_v2",
            "sqlite3_prepare_v2",
            "sqlite3_step",
            "connect",
            "sendto",
            "getaddrinfo",
            "open",
            "openat",
            "unlink",
        ] {
            assert!(destino_de(n).is_some(), "falta el gancho {}", n);
        }
        assert!(destino_de("no_existe").is_none());
    }

    #[test]
    fn las_rutas_internas_se_ignoran() {
        assert!(ruta_interna("/proc/self/maps"));
        assert!(ruta_interna("/system/build.prop"));
        assert!(!ruta_interna("/data/data/com.agy.app/files/sesiones.db"));
    }

    #[test]
    fn recorta_los_detalles_largos() {
        let s = "a".repeat(500);
        let r = recortar(&s, 10);
        assert_eq!(r.chars().count(), 11);
    }
}
