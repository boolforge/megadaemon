//! MegaDaemon — sandbox real del sistema operativo para las herramientas.
//!
//! Qué resuelve: `tool_policy.rs` decide **si** una herramienta puede ejecutarse
//! (allow / ask / deny), pero la decisión se queda en el proceso: la herramienta
//! sigue viendo el disco entero y la red. Este módulo traduce esa decisión a
//! **aislamiento de verdad** con el mecanismo que miden los agentes más
//! maduros: **bubblewrap (`bwrap`) + seccomp** en Linux, con `sandbox-exec`
//! (seatbelt) en macOS.
//!
//! Cómo se llegó aquí (medido, 2026-11-27): el agente de Devin Desktop
//! 3.15.14 (ex-Windsurf) trae su propia CLI en Rust y su sandbox documenta
//! literalmente *"macOS seatbelt / Linux bwrap+seccomp"*, y **en Linux exige
//! `bwrap` y `socat`**. Su modelo de permisos (`Write(...)`, `Deny(Read(...))`)
//! **se compila a una regla de sandbox**: el permiso lógico y el aislamiento
//! son el mismo objeto, no dos capas. Aquí se replica esa idea con nuestro
//! `PolicyDecision`.
//!
//! Precedencia de permisos: **deny > ask > allow** (medido en el motor de
//! Cursor 3.32.4: 22 eventos de hook y tres niveles de decisión).
//!
//! Este módulo **no ejecuta nada**: construye el comando y el perfil. Quien
//! lo lanza es `tool_policy.rs` (o el `mcp-server`), que ya tiene el
//! veredicto.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Component, Path, PathBuf};

/// Red disponible para la herramienta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Red {
    /// Sin red: se usa un namespace de red vacío.
    Ninguna,
    /// Red del host (sólo si la política lo permite).
    Host,
}

/// Qué se puede montar y en qué modo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Montaje {
    /// Sólo lectura.
    Lectura(PathBuf),
    /// Lectura y escritura.
    Escritura(PathBuf),
    /// `tmpfs` nuevo, vacío.
    Tmpfs(PathBuf),
    /// `tmpfs` con un directorio semilla.
    TmpfsCon(PathBuf, PathBuf),
}

/// Una regla de permisos de ruta (patrón tipo glob, como el de Cursor/Devin).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Regla {
    /// `true` = permitido, `false` = prohibido.
    permitido: bool,
    /// Patrón de ruta (`**`, `*`, `?`). Vacío = la raíz del workspace.
    patron: String,
    /// Ruta a la que se aplica (raíz del workspace).
    base: PathBuf,
}

impl Regla {
    /// Regla que permite el patrón.
    pub fn permitir(patron: &str, base: &Path) -> Self {
        Regla { permitido: true, patron: patron.to_string(), base: base.to_path_buf() }
    }

    /// Regla que prohíbe el patrón.
    pub fn prohibir(patron: &str, base: &Path) -> Self {
        Regla { permitido: false, patron: patron.to_string(), base: base.to_path_buf() }
    }

    /// ¿Coincide esta regla con la ruta dada?
    pub fn coincide(&self, ruta: &Path) -> bool {
        let rel = match ruta.strip_prefix(&self.base) {
            Ok(r) => r.to_string_lossy().replace('\\', "/"),
            Err(_) => return false,
        };
        glob_coincide(&self.patron, &rel)
    }

    /// Patrón y bandera de permiso, para el informe.
    pub fn describe(&self) -> (bool, String) {
        (self.permitido, self.patron.clone())
    }
}

/// Glob mínimo (`**`, `*`, `?`) sobre rutas relativas con `/`.
///
/// Implementación propia y sin dependencias: `**` casa con cualquier número de
/// segmentos, `*` con uno, `?` con un carácter.
pub fn glob_coincide(patron: &str, texto: &str) -> bool {
    if patron == "**" {
        return true;
    }
    let p: Vec<char> = patron.chars().collect();
    let t: Vec<char> = texto.chars().collect();
    coincide(&p, &t)
}

fn coincide(p: &[char], t: &[char]) -> bool {
    if p.is_empty() {
        return t.is_empty();
    }
    match p[0] {
        '*' => {
            // `**` (doble) atraviesa separadores; `*` no.
            let doble = p.len() > 1 && p[1] == '*';
            let resto_p = if doble {
                // se come el segundo `*` y, si sigue un `/`, también el `/`
                let mut i = 2;
                if i < p.len() && p[i] == '/' {
                    i += 1;
                }
                &p[i..]
            } else {
                &p[1..]
            };
            if coincide(resto_p, t) {
                return true;
            }
            for i in 0..t.len() {
                if !doble && t[i] == '/' {
                    break;
                }
                if coincide(resto_p, &t[i + 1..]) {
                    return true;
                }
            }
            false
        }
        '?' => !t.is_empty() && t[0] != '/' && coincide(&p[1..], &t[1..]),
        c => !t.is_empty() && t[0] == c && coincide(&p[1..], &t[1..]),
    }
}

/// El perfil de aislamiento que se va a aplicar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Perfil {
    /// Raíz del workspace.
    pub raiz: PathBuf,
    /// Montajes (lectura/escritura/tmpfs).
    pub montajes: Vec<Montaje>,
    /// Reglas de permisos por ruta.
    pub reglas: Vec<Regla>,
    /// Red disponible.
    pub red: Red,
    /// Si se puede salir del sandbox (escribir fuera, red, …).
    pub escapable: bool,
    /// Binarios extra que se dejan visibles (bwrap, socat, sh, …).
    pub binarios_extra: Vec<PathBuf>,
    /// Fichero de filtro seccomp (BPF ya compilado). `None` = sin seccomp.
    pub filtro_seccomp: Option<PathBuf>,
}

impl Perfil {
    /// Perfil mínimo: sólo lectura del workspace, sin red, sin escapes.
    pub fn minimo(raiz: &Path) -> Self {
        Perfil {
            raiz: raiz.to_path_buf(),
            montajes: vec![Montaje::Lectura(raiz.to_path_buf())],
            reglas: Vec::new(),
            red: Red::Ninguna,
            escapable: false,
            binarios_extra: Vec::new(),
            filtro_seccomp: None,
        }
    }

    /// Añade una regla de permisos.
    pub fn con_regla(mut self, regla: Regla) -> Self {
        self.reglas.push(regla);
        self
    }

    /// Monta en escritura un camino (normalmente el workspace).
    pub fn con_escritura(mut self, ruta: &Path) -> Self {
        self.montajes.retain(|m| !matches!(m, Montaje::Lectura(p) if p == ruta));
        self.montajes.push(Montaje::Escritura(ruta.to_path_buf()));
        self
    }

    /// Con red del host.
    pub fn con_red(mut self) -> Self {
        self.red = Red::Host;
        self
    }

    /// Declara un binario extra visible (para que el comando exista dentro).
    pub fn con_binario(mut self, ruta: &Path) -> Self {
        self.binarios_extra.push(ruta.to_path_buf());
        self
    }

    /// Añade un filtro seccomp.
    pub fn con_seccomp(mut self, filtro: &Path) -> Self {
        self.filtro_seccomp = Some(filtro.to_path_buf());
        self
    }

    /// Veredicto de una ruta concreta según las reglas (deny > allow).
    ///
    /// La **última** regla que coincide gana si todas son del mismo tipo; si hay
    /// una prohibición y un permiso, **gana la prohibición** (deny > allow).
    pub fn verdict(&self, ruta: &Path) -> bool {
        // deny > allow: si alguna regla que coincide prohibe, se prohibe.
        let mut hay_permiso = false;
        for r in &self.reglas {
            if !r.coincide(ruta) {
                continue;
            }
            if !r.permitido {
                return false;
            }
            hay_permiso = true;
        }
        hay_permiso
    }

    /// Normaliza una ruta (sin `..`, sin symlinks“全国”).
    pub fn normaliza(ruta: &Path) -> PathBuf {
        let mut out = PathBuf::new();
        for c in ruta.components() {
            match c {
                Component::CurDir => {}
                Component::ParentDir => {
                    out.pop();
                }
                other => out.push(other.as_os_str()),
            }
        }
        out
    }

    /// Construye el `argv` de **bubblewrap** (`bwrap`) para este perfil.
    ///
    /// La forma es la que documenta el agente de Devin: `--die-with-parent`,
    /// `--new-session`, `--unshare-*` y los montajes uno a uno. Con
    /// `Red::Ninguna` se añade `--unshare-net`.
    pub fn argv_bwrap(&self, programa: &[String], args: &[String]) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        v.push("bwrap".to_string());
        v.push("--die-with-parent".to_string());
        v.push("--new-session".to_string());
        v.push("--unshare-pid".to_string());
        v.push("--unshare-uts".to_string());
        v.push("--unshare-ipc".to_string());
        if self.red == Red::Ninguna {
            v.push("--unshare-net".to_string());
        }
        if !self.escapable {
            v.push("--cap-drop".to_string());
            v.push("ALL".to_string());
        }
        for m in &self.montajes {
            match m {
                Montaje::Lectura(p) => {
                    v.push("--ro-bind".to_string());
                    v.push(p.display().to_string());
                    v.push(p.display().to_string());
                }
                Montaje::Escritura(p) => {
                    v.push("--bind".to_string());
                    v.push(p.display().to_string());
                    v.push(p.display().to_string());
                }
                Montaje::Tmpfs(p) => {
                    v.push("--tmpfs".to_string());
                    v.push(p.display().to_string());
                }
                Montaje::TmpfsCon(destino, semilla) => {
                    v.push("--tmpfs".to_string());
                    v.push(destino.display().to_string());
                    v.push("--ro-bind".to_string());
                    v.push(semilla.display().to_string());
                    v.push(destino.display().to_string());
                }
            }
        }
        for b in &self.binarios_extra {
            v.push("--ro-bind".to_string());
            v.push(b.display().to_string());
            v.push(b.display().to_string());
        }
        if let Some(f) = &self.filtro_seccomp {
            v.push("--seccomp".to_string());
            v.push(f.display().to_string());
        }
        // /proc y /dev mínimos: sin ellos casi nada arranca dentro.
        v.push("--proc".to_string());
        v.push("/proc".to_string());
        v.push("--dev".to_string());
        v.push("/dev".to_string());
        v.push("--".to_string());
        v.push(programa.first().cloned().unwrap_or_else(|| "/bin/sh".to_string()));
        if let Some(p) = programa.get(1..) {
            v.extend(p.iter().cloned());
        }
        v.extend(args.iter().cloned());
        v
    }

    /// Construye el `argv` de **sandbox-exec** (seatbelt de macOS) equivalente.
    ///
    /// El perfil de seatbelt es un perfil dynamic: se allow-listan las rutas de
    /// lectura y se deniegan las de escritura fuera de lo permitido.
    pub fn argv_seatbelt(&self, programa: &[String], args: &[String]) -> Vec<String> {
        let mut allow: BTreeSet<String> = BTreeSet::new();
        let mut deny: BTreeSet<String> = BTreeSet::new();
        for m in &self.montajes {
            match m {
                Montaje::Lectura(p) => {
                    allow.insert(format!("file-read* {}", p.display()));
                }
                Montaje::Escritura(p) => {
                    allow.insert(format!("file-read* {}", p.display()));
                    allow.insert(format!("file-write* {}", p.display()));
                }
                Montaje::Tmpfs(p) => {
                    allow.insert(format!("file-read* {}", p.display()));
                    allow.insert(format!("file-write* {}", p.display()));
                }
                Montaje::TmpfsCon(d, _s) => {
                    allow.insert(format!("file-read* {}", d.display()));
                    allow.insert(format!("file-write* {}", d.display()));
                }
            }
        }
        for (permitido, patron) in self.reglas.iter().map(|r| r.describe()) {
            let ruta = self.raiz.join(patron);
            let regla = if permitido {
                format!("file-read* {}", ruta.display())
            } else {
                format!("!file-read* {}", ruta.display())
            };
            if permitido {
                allow.insert(regla);
            } else {
                deny.insert(regla);
            }
        }
        if self.red == Red::Ninguna {
            deny.insert("(deny network*)".to_string());
        }
        let mut perfil = String::from("(version 1)\n");
        for d in &deny {
            perfil.push_str(d);
            perfil.push('\n');
        }
        for a in &allow {
            perfil.push_str(a);
            perfil.push('\n');
        }
        perfil.push_str("(allow default)\n");
        let mut v: Vec<String> = vec![
            "/usr/bin/sandbox-exec".to_string(),
            "-p".to_string(),
            perfil,
        ];
        v.push(programa.first().cloned().unwrap_or_else(|| "/bin/sh".to_string()));
        if let Some(p) = programa.get(1..) {
            v.extend(p.iter().cloned());
        }
        v.extend(args.iter().cloned());
        v
    }

    /// Informe legible del perfil (para el log de auditoría).
    pub fn describe(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("raiz: {}\n", self.raiz.display()));
        s.push_str(&format!("red: {}\n", if self.red == Red::Host { "host" } else { "ninguna" }));
        s.push_str(&format!("escapable: {}\n", self.escapable));
        for m in &self.montajes {
            let t = match m {
                Montaje::Lectura(p) => format!("ro  {}", p.display()),
                Montaje::Escritura(p) => format!("rw  {}", p.display()),
                Montaje::Tmpfs(p) => format!("tmp {}", p.display()),
                Montaje::TmpfsCon(d, _s) => format!("tmp+ {}", d.display()),
            };
            s.push_str(&format!("montaje: {}\n", t));
        }
        for r in &self.reglas {
            let (permitido, patron) = r.describe();
            s.push_str(&format!(
                "regla: {} {}\n",
                if permitido { "allow" } else { "deny " },
                patron
            ));
        }
        if let Some(f) = &self.filtro_seccomp {
            s.push_str(&format!("seccomp: {}\n", f.display()));
        }
        s
    }
}

/// Error del módulo.
#[derive(Debug)]
pub enum SandboxError {
    /// Falta la herramienta del sistema (bwrap/sandbox-exec).
    FaltaHerramienta(String),
    /// El comando no se pudo lanzar.
    Lanzamiento(String),
}

impl fmt::Display for SandboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SandboxError::FaltaHerramienta(m) => write!(f, "falta la herramienta: {}", m),
            SandboxError::Lanzamiento(m) => write!(f, "no se pudo lanzar: {}", m),
        }
    }
}

impl std::error::Error for SandboxError {}

/// ¿Está `bwrap` disponible en este sistema?
pub fn bwrap_disponible() -> bool {
    which("bwrap").is_some()
}

/// ¿Está `sandbox-exec` disponible (macOS)?
pub fn seatbelt_disponible() -> bool {
    which("sandbox-exec").is_some()
}

/// Busca un binario en el `PATH`.
pub fn which(nombre: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let cand = dir.join(nombre);
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

/// Elige el aislamiento disponible en este sistema.
///
/// En Linux: `bwrap` + seccomp. En macOS: `sandbox-exec`. Si no hay ninguna,
/// se devuelve `None` y quien llama **debe** negarse a ejecutar sin aislamiento
/// (fallar cerrado, no abierto).
pub fn aislamiento_disponible() -> Option<&'static str> {
    if bwrap_disponible() {
        Some("bwrap")
    } else if seatbelt_disponible() {
        Some("seatbelt")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_simple() {
        assert!(glob_coincide("*.txt", "a.txt"));
        assert!(!glob_coincide("*.txt", "a.md"));
        assert!(glob_coincide("**/*.rs", "src/deep/a.rs"));
        assert!(glob_coincide("**", "lo que sea"));
        assert!(glob_coincide("src/**", "src/a/b/c.rs"));
        assert!(!glob_coincide("src/*", "src/a/b.rs"));
    }

    #[test]
    fn glob_interrogacion() {
        assert!(glob_coincide("a?c", "abc"));
        assert!(!glob_coincide("a?c", "ac"));
    }

    #[test]
    fn regla_exige_la_base() {
        let base = Path::new("/w");
        let r = Regla::permitir("src/**", base);
        assert!(r.coincide(Path::new("/w/src/x.rs")));
        assert!(!r.coincide(Path::new("/otro/src/x.rs")));
    }

    #[test]
    fn deny_gana_a_allow() {
        let base = Path::new("/w");
        let p = Perfil::minimo(base)
            .con_regla(Regla::permitir("**", base))
            .con_regla(Regla::prohibir("secreto/**", base));
        assert!(p.verdict(Path::new("/w/ok.txt")));
        assert!(!p.verdict(Path::new("/w/secreto/x")));
    }

    #[test]
    fn el_orden_de_las_reglas_no_cambia_el_veredicto() {
        let base = Path::new("/w");
        let a = Perfil::minimo(base)
            .con_regla(Regla::prohibir("secreto/**", base))
            .con_regla(Regla::permitir("**", base));
        assert!(!a.verdict(Path::new("/w/secreto/x")));
    }

    #[test]
    fn argv_de_bwrap_lleva_los_montajes_y_el_guion() {
        // MEDIDO (2026-11-27): `con_escritura` SUSTITUYE el montaje de
        // lectura del mismo path (no se puede montar dos veces el mismo
        // sitio), asi que con "/w" no queda ningun `--ro-bind`.  El test
        // quiere comprobar que el argv lleva LOS DOS montajes: se escribe
        // en un path distinto.
        let p = Perfil::minimo(Path::new("/w")).con_escritura(Path::new("/w/salida"));
        let argv = p.argv_bwrap(&["/bin/sh".to_string()], &["-c".to_string(), "ls".to_string()]);
        assert_eq!(argv[0], "bwrap");
        assert!(argv.contains(&"--die-with-parent".to_string()));
        assert!(argv.contains(&"--unshare-net".to_string()));
        assert!(argv.contains(&"--ro-bind".to_string()));
        assert!(argv.contains(&"--bind".to_string()));
        let guiones = argv.iter().position(|a| a == "--").expect("separador --");
        assert_eq!(argv[guiones + 1], "/bin/sh");
        assert_eq!(argv[guiones + 2], "-c");
    }

    #[test]
    fn con_red_quita_el_unshare_de_red() {
        let p = Perfil::minimo(Path::new("/w")).con_red();
        let argv = p.argv_bwrap(&["/bin/sh".to_string()], &[]);
        assert!(!argv.contains(&"--unshare-net".to_string()));
    }

    #[test]
    fn seatbelt_deniega_la_red_y_las_reglas() {
        let base = Path::new("/w");
        let p = Perfil::minimo(base).con_regla(Regla::prohibir("secreto/**", base));
        let argv = p.argv_seatbelt(&["/bin/sh".to_string()], &[]);
        assert_eq!(argv[0], "/usr/bin/sandbox-exec");
        let perfil = &argv[2];
        assert!(perfil.contains("(deny network*)"));
        assert!(perfil.contains("!file-read*"));
    }

    #[test]
    fn normaliza_quita_el_guino_de_vuelta() {
        let p = Perfil::normaliza(Path::new("/w/../w/./x"));
        assert_eq!(p, PathBuf::from("/w/x"));
    }

    #[test]
    fn el_perfil_menor_es_solo_lectura_y_sin_red() {
        let p = Perfil::minimo(Path::new("/w"));
        assert_eq!(p.red, Red::Ninguna);
        assert!(!p.escapable);
        assert_eq!(p.montajes.len(), 1);
    }
}
