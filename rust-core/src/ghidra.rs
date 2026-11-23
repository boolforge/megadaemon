//! MegaDaemon v15 — Soporte de Ghidra headless (análisis de binarios ELF/Go).
//!
//! Este módulo envuelve a `analyzeHeadless` de NSA/Ghidra para poder:
//!
//! 1. **Detectar** la herramienta (`analyzeHeadless`) y un **JDK** utilizable,
//!    tanto en el PC de trabajo como en el móvil (Android) o en un directorio
//!    configurable por el operador.
//! 2. **Importar** un binario en un proyecto Ghidra headless (`-import`).
//! 3. **Aplicar** el export de símbolos de `unstrip` (161k funciones Go)
//!    sobre el programa del proyecto.
//! 4. **Volcar** el pseudocódigo de una función (o de un conjunto de anclas)
//!    a fichero de texto.
//!
//! Reglas de diseño (no negociables en este módulo):
//!
//! * **Nunca** se cuelga el agente: todo proceso externo corre con *timeout*
//!   duro y, si se agota, se mata el **grupo de procesos** completo.
//! * **Cola**: sólo hay una ejecución de Ghidra simultánea en todo el proceso.
//!   Las demás esperan con presupuesto de tiempo y, si no les toca,
//!   devuelven un error claro (`GhidraError::Busy`) en vez de bloquear.
//! * **Nunca** hay `panic!` por falta de Java: se devuelve
//!   `GhidraError::JavaMissing` con un mensaje que dice exactamente qué
//!   instalar y dónde se buscó.
//!
//! Nota de versiones: Ghidra 11.2+ exige **JDK 21** (`application.java.min=21`)
//! y un **JDK**, no un JRE, porque compila los GhidraScript de Java al vuelo
//! (`javac` es obligatorio para `-postscript Foo.java`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Errores
// ---------------------------------------------------------------------------

/// Errores del soporte de Ghidra. Todos implementan `Display` con un mensaje
/// accionable: nunca se propagan como `panic!`.
#[derive(Debug, Clone)]
pub enum GhidraError {
    /// No hay `java` utilizable (ni JRE ni JDK) en el entorno.
    JavaMissing { searched: Vec<String> },
    /// Hay Java pero es un JRE: Ghidra necesita JDK para compilar los scripts.
    JavaNotJdk { java_home: String },
    /// No se encontró `analyzeHeadless`.
    AnalyzeHeadlessMissing { searched: Vec<String> },
    /// La cola global está ocupada y se agotó el presupuesto de espera.
    Busy { waited_secs: u64 },
    /// Se agotó el timeout de una fase.
    Timeout { stage: String, limit_secs: u64 },
    /// `analyzeHeadless` terminó con código distinto de 0.
    Failed { stage: String, code: Option<i32>, stderr: String },
    /// Configuración inválida (rutas vacías, argumentos mal formados...).
    Config(String),
    /// Error de E/S.
    Io(String),
}

impl fmt::Display for GhidraError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GhidraError::JavaMissing { searched } => write!(
                f,
                "Ghidra necesita Java (JDK 21 o superior para Ghidra 11.2+) y no se \
                 encontró en este entorno. Se buscó en: {}. \
                 Instala `openjdk-21-jdk-headless` o define JAVA_HOME apuntando a un JDK \
                 (no basta un JRE: Ghidra compila los scripts con javac).",
                searched.join(", ")
            ),
            GhidraError::JavaNotJdk { java_home } => write!(
                f,
                "El Java de {} es un JRE, no un JDK. Ghidra necesita `javac` para compilar \
                 los GhidraScript de Java (`-postscript Foo.java`). Instala \
                 `openjdk-21-jdk-headless` o apunta JAVA_HOME a un JDK completo.",
                java_home
            ),
            GhidraError::AnalyzeHeadlessMissing { searched } => write!(
                f,
                "No se encontró `analyzeHeadless` (Ghidra no está instalado o no está en el \
                 PATH). Se buscó en: {}. Define MD_GHIDRA_DIR con el directorio de \
                 instalación de Ghidra o instala Ghidra 11.x.",
                searched.join(", ")
            ),
            GhidraError::Busy { waited_secs } => write!(
                f,
                "Otra ejecución de Ghidra está en curso (cola global ocupada) y se agotaron \
                 los {} s de espera. Reintenta más tarde o aumenta el presupuesto de cola.",
                waited_secs
            ),
            GhidraError::Timeout { stage, limit_secs } => write!(
                f,
                "La fase '{}' de Ghidra superó el timeout de {} s y se abortó (grupo de \
                 procesos terminado). El binario puede ser demasiado grande para el \
                 presupuesto de memoria/tiempo disponible.",
                stage,
                limit_secs
            ),
            GhidraError::Failed { stage, code, stderr } => {
                let c = match code {
                    Some(n) => n.to_string(),
                    None => "desconocido".to_string(),
                };
                let tail: String = stderr.chars().rev().take(1200).collect::<String>();
                let tail: String = tail.chars().rev().collect();
                write!(f, "Ghidra falló en la fase '{}' (código {}): {}", stage, c, tail.trim())
            }
            GhidraError::Config(m) => write!(f, "Configuración inválida de Ghidra: {}", m),
            GhidraError::Io(m) => write!(f, "Error de E/S en el soporte de Ghidra: {}", m),
        }
    }
}

impl std::error::Error for GhidraError {}

pub type GhidraResult<T> = Result<T, GhidraError>;

// ---------------------------------------------------------------------------
// Configuración
// ---------------------------------------------------------------------------

/// Configuración de una sesión de Ghidra.
#[derive(Debug, Clone)]
pub struct GhidraConfig {
    /// Directorio de instalación de Ghidra (el que contiene `support/`).
    /// Si es `None` se autodetecta.
    pub ghidra_dir: Option<PathBuf>,
    /// `JAVA_HOME` a usar. Si es `None` se autodetecta.
    pub java_home: Option<PathBuf>,
    /// Directorio donde viven los proyectos Ghidra.
    pub project_dir: PathBuf,
    /// Nombre del proyecto Ghidra.
    pub project_name: String,
    /// Timeout por fase (import / scripts).
    pub timeout: Duration,
    /// Presupuesto de espera en la cola global.
    pub queue_wait: Duration,
    /// Memoria máxima del heap de la JVM (`2G`, `1400M`, ...).
    pub max_memory: String,
    /// Directorio donde se escriben los scripts Java que se pasan a Ghidra.
    pub script_dir: PathBuf,
}

impl Default for GhidraConfig {
    fn default() -> Self {
        GhidraConfig {
            ghidra_dir: None,
            java_home: None,
            project_dir: PathBuf::from("/workspace/agy-re/tools-out/proj"),
            project_name: "Go".to_string(),
            timeout: Duration::from_secs(1800),
            queue_wait: Duration::from_secs(60),
            max_memory: "2G".to_string(),
            script_dir: PathBuf::from("/workspace/agy-re/tools-out/scripts"),
        }
    }
}

impl GhidraConfig {
    /// Configuración orientada al móvil (Android): todo bajo el directorio
    /// privado de la app, que es lo único escribible en un móvil.
    pub fn mobile(root: &Path) -> Self {
        GhidraConfig {
            ghidra_dir: Some(root.join("ghidra")),
            java_home: Some(root.join("jdk")),
            project_dir: root.join("ghidra-projects"),
            project_name: "Go".to_string(),
            timeout: Duration::from_secs(1800),
            queue_wait: Duration::from_secs(30),
            max_memory: "2G".to_string(),
            script_dir: root.join("ghidra-scripts"),
        }
    }
}

// ---------------------------------------------------------------------------
// Detección
// ---------------------------------------------------------------------------

/// Toolchain detectado: `analyzeHeadless` + JDK.
#[derive(Debug, Clone)]
pub struct GhidraTool {
    /// Ruta absoluta a `analyzeHeadless`.
    pub analyze_headless: PathBuf,
    /// Directorio de instalación de Ghidra (padre de `support/`).
    pub ghidra_dir: Option<PathBuf>,
    /// `JAVA_HOME` detectado (o el del sistema).
    pub java_home: Option<PathBuf>,
    /// Ruta al ejecutable `java`.
    pub java_bin: Option<PathBuf>,
    /// `true` si el Java detectado trae `javac` (JDK completo).
    pub has_jdk: bool,
    /// Rutas donde se buscó cada pieza (para el diagnóstico).
    pub searched: BTreeMap<String, Vec<String>>,
}

/// Directorios donde se busca una instalación de Ghidra si no se configura.
pub const GHIDRA_DIR_CANDIDATES: &[&str] = &[
    "/opt/ghidra",
    "/opt/ghidra_11.2.1_PUBLIC",
    "/usr/local/ghidra",
    "/usr/share/ghidra",
    "/workspace/agy-re/tools-out/ghidra_11.2.1_PUBLIC",
    // Rutas del móvil (Android): la app escribe en su directorio privado.
    "/data/data/com.agy.app/files/ghidra",
    "/data/local/ghidra",
];

/// Nombres de carpeta que se reconocen como instalación de Ghidra.
const GHIDRA_DIR_NAMES: &[&str] = &["ghidra", "ghidra_11.2.1_PUBLIC", "ghidra_11.3.2_PUBLIC"];

fn is_executable_file(p: &Path) -> bool {
    if !p.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match fs::metadata(p) {
            Ok(m) => m.permissions().mode() & 0o111 != 0,
            Err(_) => false,
        }
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn path_dirs() -> Vec<PathBuf> {
    match std::env::var("PATH") {
        Ok(p) => std::env::split_paths(&p).collect(),
        Err(_) => Vec::new(),
    }
}

/// Busca `analyzeHeadless` en el PATH, en los directorios candidatos y en
/// los subdirectorios "con nombre de Ghidra" de los candidatos.
fn find_analyze_headless(cfg: &GhidraConfig) -> (Option<PathBuf>, Vec<String>) {
    let mut searched: Vec<String> = Vec::new();

    // 1) ruta explícita del operador
    if let Some(dir) = &cfg.ghidra_dir {
        let p = dir.join("support").join("analyzeHeadless");
        searched.push(p.display().to_string());
        if is_executable_file(&p) {
            return (Some(p), searched);
        }
    }

    // 2) variable de entorno
    if let Ok(dir) = std::env::var("MD_GHIDRA_DIR") {
        let p = PathBuf::from(&dir).join("support").join("analyzeHeadless");
        searched.push(p.display().to_string());
        if is_executable_file(&p) {
            return (Some(p), searched);
        }
    }

    // 3) PATH
    for d in path_dirs() {
        let p = d.join("analyzeHeadless");
        searched.push(p.display().to_string());
        if is_executable_file(&p) {
            return (Some(p), searched);
        }
    }

    // 4) directorios candidatos (+ un nivel de subdirectorios con nombre Ghidra)
    let mut roots: Vec<PathBuf> = Vec::new();
    for c in GHIDRA_DIR_CANDIDATES {
        roots.push(PathBuf::from(c));
    }
    if let Some(d) = &cfg.ghidra_dir {
        roots.push(d.clone());
    }
    for root in roots {
        let p = root.join("support").join("analyzeHeadless");
        searched.push(p.display().to_string());
        if is_executable_file(&p) {
            return (Some(p), searched);
        }
        // subdirectorios del tipo ghidra_* dentro del padre
        if let Some(parent) = root.parent() {
            if let Ok(entries) = fs::read_dir(parent) {
                for e in entries.flatten() {
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.starts_with("ghidra") && e.path().is_dir() {
                        let p2 = e.path().join("support").join("analyzeHeadless");
                        searched.push(p2.display().to_string());
                        if is_executable_file(&p2) {
                            return (Some(p2), searched);
                        }
                    }
                }
            }
        }
    }
    (None, searched)
}

/// Busca un JDK utilizable: `JAVA_HOME` configurado, `JAVA_HOME` del entorno,
/// PATH, o `/usr/lib/jvm/*`.
fn find_java(cfg: &GhidraConfig) -> (Option<PathBuf>, Option<PathBuf>, bool, Vec<String>) {
    let mut searched: Vec<String> = Vec::new();
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(jh) = &cfg.java_home {
        candidates.push(jh.clone());
    }
    if let Ok(jh) = std::env::var("JAVA_HOME") {
        candidates.push(PathBuf::from(jh));
    }
    if let Ok(jh) = std::env::var("MD_JAVA_HOME") {
        candidates.push(PathBuf::from(jh));
    }
    for d in path_dirs() {
        candidates.push(d);
    }
    if let Ok(entries) = fs::read_dir("/usr/lib/jvm") {
        for e in entries.flatten() {
            candidates.push(e.path());
        }
    }

    for base in candidates {
        let java = base.join("bin").join("java");
        if !is_executable_file(&java) {
            continue;
        }
        searched.push(java.display().to_string());
        let javac = base.join("bin").join("javac");
        let is_jdk = is_executable_file(&javac);
        let home = base
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| base.clone());
        return (Some(java), Some(home), is_jdk, searched);
    }
    (None, None, false, searched)
}

impl GhidraTool {
    /// Detecta `analyzeHeadless` y el JDK. Devuelve un error **claro** (nunca
    /// un panic) si falta Java.
    pub fn detect(cfg: &GhidraConfig) -> GhidraResult<GhidraTool> {
        let (java_bin, java_home, has_jdk, java_searched) = find_java(cfg);
        let (headless, hl_searched) = find_analyze_headless(cfg);

        let mut searched = BTreeMap::new();
        searched.insert("java".to_string(), java_searched);
        searched.insert("analyzeHeadless".to_string(), hl_searched);

        let java_bin = match java_bin {
            Some(j) => j,
            None => {
                return Err(GhidraError::JavaMissing {
                    searched: searched.get("java").cloned().unwrap_or_default(),
                })
            }
        };
        let headless = match headless {
            Some(h) => h,
            None => {
                return Err(GhidraError::AnalyzeHeadlessMissing {
                    searched: searched.get("analyzeHeadless").cloned().unwrap_or_default(),
                })
            }
        };

        Ok(GhidraTool {
            analyze_headless: headless,
            ghidra_dir: cfg
                .ghidra_dir
                .clone()
                .or_else(|| {
                    // deduce del propio analyzeHeadless: <dir>/support/analyzeHeadless
                    cfg.ghidra_dir.clone()
                })
                .or_else(|| std::env::var("MD_GHIDRA_DIR").ok().map(PathBuf::from))
                .or_else(|| {
                    let mut it = searched.get("analyzeHeadless").cloned().unwrap_or_default().into_iter();
                    it.next()
                        .and_then(|s| PathBuf::from(s).parent().and_then(|p| p.parent()).map(|x| x.to_path_buf()))
                }),
            java_home,
            java_bin: Some(java_bin),
            has_jdk,
            searched,
        })
    }

    /// Igual que [`GhidraTool::detect`] pero tolerante: si falta Java devuelve
    /// `Ok(None)` en vez de error. Útil para un `doctor` informativo.
    pub fn detect_optional(cfg: &GhidraConfig) -> Option<GhidraTool> {
        GhidraTool::detect(cfg).ok()
    }
}

// ---------------------------------------------------------------------------
// Cola global (una sola ejecución de Ghidra a la vez)
// ---------------------------------------------------------------------------

fn queue() -> &'static Mutex<()> {
    static Q: OnceLock<Mutex<()>> = OnceLock::new();
    Q.get_or_init(|| Mutex::new(()))
}

/// Adquiere la cola global con presupuesto. Devuelve el guard o `Busy`.
fn acquire_queue(wait: Duration) -> GhidraResult<std::sync::MutexGuard<'static, ()>> {
    let q = queue();
    let deadline = Instant::now() + wait;
    loop {
        match q.try_lock() {
            Ok(g) => return Ok(g),
            Err(std::sync::TryLockError::Poisoned(p)) => {
                // Un panic previo envenenó el mutex: lo recuperamos y seguimos.
                return Ok(p.into_inner());
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                if Instant::now() >= deadline {
                    return Err(GhidraError::Busy {
                        waited_secs: wait.as_secs(),
                    });
                }
                thread::sleep(Duration::from_millis(150));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Ejecución de procesos con timeout
// ---------------------------------------------------------------------------

/// Resultado de una ejecución de `analyzeHeadless`.
#[derive(Debug, Clone)]
pub struct HeadlessRun {
    pub stage: String,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub elapsed_secs: u64,
}

/// Ejecuta un comando con timeout duro, matando el **grupo de procesos** si se
/// agota (así no quedan javas huérfanos comiéndose la RAM).
pub fn run_with_timeout(
    mut cmd: Command,
    stage: &str,
    limit: Duration,
    max_bytes: usize,
) -> GhidraResult<HeadlessRun> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Grupo de procesos propio: permite matar a los nietos (la JVM).
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let started = Instant::now();
    let mut child = cmd
        .spawn()
        .map_err(|e| GhidraError::Io(format!("no se pudo lanzar '{}': {}", stage, e)))?;
    let pid = child.id() as i32;

    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    let out_handle = thread::spawn(move || {
        let mut buf: Vec<u8> = Vec::new();
        if let Some(mut p) = out_pipe.take() {
            use std::io::Read;
            let mut chunk = [0u8; 8192];
            loop {
                match p.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        buf.extend_from_slice(&chunk[..n]);
                        if buf.len() > max_bytes {
                            buf.drain(0..buf.len() - max_bytes);
                        }
                    }
                }
            }
        }
        buf
    });
    let err_handle = thread::spawn(move || {
        let mut buf: Vec<u8> = Vec::new();
        if let Some(mut p) = err_pipe.take() {
            use std::io::Read;
            let mut chunk = [0u8; 8192];
            loop {
                match p.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        buf.extend_from_slice(&chunk[..n]);
                        if buf.len() > max_bytes {
                            buf.drain(0..buf.len() - max_bytes);
                        }
                    }
                }
            }
        }
        buf
    });

    let deadline = started + limit;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) => {
                if Instant::now() >= deadline {
                    kill_group(pid);
                    let _ = child.wait();
                    let _ = out_handle.join();
                    let _ = err_handle.join();
                    return Err(GhidraError::Timeout {
                        stage: stage.to_string(),
                        limit_secs: limit.as_secs(),
                    });
                }
                thread::sleep(Duration::from_millis(120));
            }
            Err(e) => {
                return Err(GhidraError::Io(format!("espera fallida en '{}': {}", stage, e)));
            }
        }
    };

    let out = out_handle.join().unwrap_or_default();
    let err = err_handle.join().unwrap_or_default();
    Ok(HeadlessRun {
        stage: stage.to_string(),
        code: status.and_then(|s| s.code()),
        stdout: String::from_utf8_lossy(&out).to_string(),
        stderr: String::from_utf8_lossy(&err).to_string(),
        elapsed_secs: started.elapsed().as_secs(),
    })
}

#[cfg(unix)]
fn kill_group(pid: i32) {
    // killing al grupo entero evita javas huérfanas.
    unsafe {
        libc::kill(-pid, libc::SIGKILL);
        libc::kill(pid, libc::SIGKILL);
    }
}

#[cfg(not(unix))]
fn kill_group(pid: i32) {
    let _ = pid;
}

// ---------------------------------------------------------------------------
// Parseo de los TSV de unstrip
// ---------------------------------------------------------------------------

/// Una función recuperada por `unstrip`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolRef {
    pub addr: u64,
    pub name: String,
    pub comment: String,
}

/// Una ancla (función interesting) a decompilar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anchor {
    pub addr: u64,
    pub name: String,
    pub orig: String,
    pub location: String,
    pub tag: String,
}

fn parse_hex(s: &str) -> Option<u64> {
    let t = s.trim();
    let t = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")).unwrap_or(t);
    u64::from_str_radix(t, 16).ok()
}

/// Lee el export de `unstrip` en formato TSV: `0xADDR \t nombre \t comentario`.
pub fn parse_symbols_tsv(path: &Path) -> GhidraResult<Vec<SymbolRef>> {
    let raw = fs::read_to_string(path)
        .map_err(|e| GhidraError::Io(format!("no se pudo leer {}: {}", path.display(), e)))?;
    let mut out = Vec::new();
    for (i, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let mut parts = line.splitn(3, '\t');
        let a = parts.next().unwrap_or("");
        let name = parts.next().unwrap_or("").trim().to_string();
        let comment = parts.next().unwrap_or("").trim().to_string();
        if name.is_empty() {
            continue;
        }
        let addr = match parse_hex(a) {
            Some(v) => v,
            None => {
                return Err(GhidraError::Config(format!(
                    "{}: línea {} con dirección inválida: '{}'",
                    path.display(),
                    i + 1,
                    a
                )))
            }
        };
        out.push(SymbolRef {
            addr,
            name,
            comment,
        });
    }
    Ok(out)
}

/// Lee el fichero de anclas: `0xADDR \t nombre \t simbolo_original \t loc \t tag`.
pub fn parse_anchors_tsv(path: &Path) -> GhidraResult<Vec<Anchor>> {
    let raw = fs::read_to_string(path)
        .map_err(|e| GhidraError::Io(format!("no se pudo leer {}: {}", path.display(), e)))?;
    let mut out = Vec::new();
    for (i, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(5, '\t').collect();
        if parts.len() < 3 {
            continue;
        }
        let addr = match parse_hex(parts[0]) {
            Some(v) => v,
            None => {
                return Err(GhidraError::Config(format!(
                    "{}: línea {} con dirección inválida: '{}'",
                    path.display(),
                    i + 1,
                    parts[0]
                )))
            }
        };
        out.push(Anchor {
            addr,
            name: parts[1].to_string(),
            orig: parts[2].to_string(),
            location: parts.get(3).unwrap_or(&"").to_string(),
            tag: parts.get(4).unwrap_or(&"anchor").to_string(),
        });
    }
    Ok(out)
}

/// Busca en el export de símbolos la función cuyo nombre contiene `needle`.
pub fn find_symbol<'a>(symbols: &'a [SymbolRef], needle: &str) -> Option<&'a SymbolRef> {
    if let Some(exact) = symbols.iter().find(|s| s.name == needle) {
        return Some(exact);
    }
    symbols.iter().find(|s| s.name.contains(needle))
}

// ---------------------------------------------------------------------------
// El GhidraScript que usa MegaDaemon
// ---------------------------------------------------------------------------

/// Script Java que MegaDaemon inyecta en Ghidra.
///
/// modo `symbols`   : aplica el export de símbolos (nombre + comentario).
/// modo `decompile` : desensambla, crea la función, decompila y vuelca a
///                    `<out_dir>/<tag>__<nombre>.c`; escribe además
///                    `<out_dir>/_decompile_report.tsv`.
/// modo `info`      : vuelca el inventario del programa (funciones, bloques,
///                    símbolos externos) a `<out_dir>/_program_info.txt`.
///
/// Argumentos: `<modo> <symbols.tsv> <anchors.tsv> <out_dir> <filtro>`
pub const MD_GHIDRA_SCRIPT: &str = r#"//MdGhidra.java
//MegaDaemon Ghidra helper (headless). @category megadaemon
import java.io.BufferedReader;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.cmd.function.CreateFunctionCmd;
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileOptions;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressFactory;
import ghidra.program.model.data.StringDataType;
import ghidra.program.model.listing.CodeUnit;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;
import ghidra.program.model.listing.Listing;
import ghidra.program.model.listing.Program;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolTable;
import ghidra.util.task.ConsoleTaskMonitor;

public class MdGhidra extends GhidraScript {

    private static final Pattern DAT_RE =
        Pattern.compile("(?:&|\\()?(DAT|LST)_([0-9a-fA-F]{4,16})");

    private AddressFactory af;
    private SymbolTable st;
    private FunctionManager fm;
    private Listing listing;
    private Program prog;
    private long imageBase = 0;
    private Map<Long, String> stringByAddr = new HashMap<Long, String>();

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 4) {
            println("MdGhidra: se esperan >=4 argumentos, llegaron " + args.length);
            return;
        }
        String mode = args[0];
        String symbolsTsv = args[1];
        String anchorsTsv = args[2];
        String outDir = args[3];
        String filter = args.length > 4 ? args[4] : "";

        af = currentProgram.getAddressFactory();
        st = currentProgram.getSymbolTable();
        fm = currentProgram.getFunctionManager();
        listing = currentProgram.getListing();
        prog = currentProgram;
        try {
            imageBase = currentProgram.getImageBase().getOffset();
        }
        catch (Exception e) {
            imageBase = 0;
        }
        new File(outDir).mkdirs();

        println("MDGHIDRA mode=" + mode + " program=" + currentProgram.getName()
            + " lang=" + currentProgram.getLanguageID()
            + " imageBase=0x" + Long.toHexString(imageBase));

        if (mode.equals("symbols")) {
            applySymbols(symbolsTsv);
        }
        else if (mode.equals("decompile")) {
            decompile(anchorsTsv, outDir, filter);
        }
        else if (mode.equals("info")) {
            programInfo(outDir);
        }
        else {
            println("MDGHIDRA: modo desconocido '" + mode + "'");
        }
        println("MDGHIDRA: DONE functions=" + fm.getFunctionCount()
            + " symbols=" + st.getNumSymbols());
    }

    /** Resuelve un offset del ELF a una direccion de Ghidra (respeta el image base). */
    private Address addrFor(long off) {
        Address a = safeAddr(off + imageBase);
        if (initialized(a)) {
            return a;
        }
        a = safeAddr(off);
        if (initialized(a)) {
            return a;
        }
        return safeAddr(off + imageBase);
    }

    private Address safeAddr(long v) {
        try {
            return toAddr(v);
        }
        catch (Throwable t) {
            return null;
        }
    }

    private boolean initialized(Address a) {
        if (a == null) {
            return false;
        }
        try {
            MemoryBlock b = currentProgram.getMemory().getBlock(a);
            return b != null && b.isInitialized();
        }
        catch (Throwable t) {
            return false;
        }
    }

    private void applySymbols(String path) throws Exception {
        long t0 = System.currentTimeMillis();
        int ok = 0, dup = 0, err = 0, skipped = 0, n = 0;
        BufferedReader br = open(path);
        String line;
        while ((line = br.readLine()) != null) {
            if (line.isEmpty()) {
                continue;
            }
            n++;
            String[] p = line.split("\t", 3);
            if (p.length < 2) {
                err++;
                continue;
            }
            Long off = parseHex(p[0]);
            if (off == null) {
                err++;
                continue;
            }
            Address addr = addrFor(off.longValue());
            if (addr == null) {
                err++;
                continue;
            }
            if (!initialized(addr)) {
                skipped++;
                continue;
            }
            String name = p[1];
            String comment = p.length > 2 ? p[2] : "";
            try {
                Function f = fm.getFunctionAt(addr);
                if (f != null) {
                    try {
                        f.setName(name, SourceType.USER_DEFINED);
                        if (comment.length() > 0) {
                            f.setComment(comment.replace(" | ", "\n"));
                        }
                        ok++;
                    }
                    catch (Exception e) {
                        dup++;
                    }
                    continue;
                }
                Symbol ex = st.getPrimarySymbol(addr);
                if (ex != null && ex.getName().equals(name)) {
                    if (comment.length() > 0) {
                        listing.setComment(addr, CodeUnit.PLATE_COMMENT,
                            comment.replace(" | ", "\n"));
                    }
                    ok++;
                    continue;
                }
                st.createLabel(addr, name, SourceType.USER_DEFINED);
                if (comment.length() > 0) {
                    listing.setComment(addr, CodeUnit.PLATE_COMMENT,
                        comment.replace(" | ", "\n"));
                }
                ok++;
            }
            catch (Exception e) {
                dup++;
            }
            if ((n % 20000) == 0) {
                println("MDGHIDRA: symbols " + n + " ok=" + ok + " dup=" + dup
                    + " elapsed=" + (System.currentTimeMillis() - t0) + "ms");
            }
        }
        br.close();
        println("MDGHIDRA: symbols done rows=" + n + " ok=" + ok + " dup=" + dup
            + " err=" + err + " skipped=" + skipped
            + " elapsed=" + (System.currentTimeMillis() - t0) + "ms");
    }

    private void decompile(String anchorsTsv, String outDir, String filter) throws Exception {
        DecompInterface ifc = new DecompInterface();
        ifc.setOptions(new DecompileOptions());
        ifc.toggleCCode(true);
        ifc.toggleSyntaxTree(true);
        ifc.setSimplificationStyle("decompile");
        if (!ifc.openProgram(currentProgram)) {
            println("MDGHIDRA: ERROR no se pudo abrir el programa en el decompilador");
            return;
        }
        PrintWriter rep = new PrintWriter(new OutputStreamWriter(
            new FileOutputStream(new File(outDir, "_decompile_report.tsv")), StandardCharsets.UTF_8));
        rep.println("addr\tname\torig\tloc\ttag\tghidra_name\tsig\tbody\tdec_ok\tdec_err\tout");

        BufferedReader br = open(anchorsTsv);
        String line;
        int idx = 0;
        while ((line = br.readLine()) != null) {
            if (line.isEmpty()) {
                continue;
            }
            String[] p = line.split("\t", 5);
            if (p.length < 3) {
                continue;
            }
            idx++;
            String name = p[1];
            if (filter != null && filter.length() > 0 && !name.contains(filter)) {
                continue;
            }
            String orig = p[2];
            String loc = p.length > 3 ? p[3] : "";
            String tag = p.length > 4 ? p[4] : "anchor";
            Long off = parseHex(p[0]);
            if (off == null) {
                continue;
            }
            Address addr = addrFor(off.longValue());
            if (addr == null) {
                continue;
            }
            String gname = "";
            String sig = "";
            String err = "";
            long body = 0;
            boolean ok = false;
            String outFile = "";
            try {
                DisassembleCommand dc = new DisassembleCommand(addr,
                    (ghidra.program.model.address.AddressSetView) null, true);
                dc.applyTo(currentProgram, monitor);
                Function f = fm.getFunctionAt(addr);
                if (f == null) {
                    CreateFunctionCmd cfc = new CreateFunctionCmd(addr);
                    cfc.applyTo(currentProgram, monitor);
                    f = fm.getFunctionAt(addr);
                }
                if (f != null) {
                    gname = f.getName();
                    body = f.getBody().getNumAddresses();
                    try {
                        if (!f.getName().equals(name)) {
                            f.setName(name, SourceType.USER_DEFINED);
                            gname = f.getName();
                        }
                    }
                    catch (Exception e) {
                    }
                    DecompileResults r = ifc.decompileFunction(f, 120, monitor);
                    if (r != null && r.decompileCompleted() && r.getDecompiledFunction() != null) {
                        sig = r.getDecompiledFunction().getSignature();
                        String c = r.getDecompiledFunction().getC();
                        outFile = write(outDir, tag, name, f, orig, loc, sig, c);
                        ok = true;
                    }
                    else if (r != null) {
                        err = String.valueOf(r.getErrorMessage());
                    }
                }
                else {
                    err = "no-function-created";
                }
            }
            catch (Throwable t) {
                err = String.valueOf(t.toString());
            }
            rep.println(p[0] + "\t" + name + "\t" + orig + "\t" + loc + "\t" + tag + "\t" + gname
                + "\t" + sig.replace("\t", " ") + "\t" + body + "\t" + ok + "\t"
                + err.replace("\t", " ").replace("\n", " ") + "\t" + outFile);
            rep.flush();
            println("MDGHIDRA: anchor " + idx + " " + name + " dec=" + ok
                + (err.isEmpty() ? "" : (" err=" + err)));
        }
        br.close();
        rep.close();
        ifc.dispose();
    }

    private void programInfo(String outDir) throws Exception {
        PrintWriter pw = new PrintWriter(new OutputStreamWriter(
            new FileOutputStream(new File(outDir, "_program_info.txt")), StandardCharsets.UTF_8));
        pw.println("program=" + currentProgram.getName());
        pw.println("language=" + currentProgram.getLanguageID());
        pw.println("compiler=" + currentProgram.getCompilerSpec().getCompilerSpecID());
        pw.println("image_base=" + currentProgram.getImageBase());
        pw.println("functions=" + fm.getFunctionCount());
        pw.println("symbols=" + st.getNumSymbols());
        pw.println("--- memory blocks ---");
        for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
            pw.println(b.getName() + "\t" + b.getStart() + "\t" + b.getEnd() + "\t"
                + b.getSize() + "\t" + (b.isRead() ? "r" : "-") + (b.isWrite() ? "w" : "-")
                + (b.isExecute() ? "x" : "-") + "\t" + b.getSourceName());
        }
        pw.println("--- external symbols (imports) ---");
        for (Symbol s : st.getExternalSymbols()) {
            pw.println(s.getName());
        }
        pw.close();
        println("MDGHIDRA: info written to " + outDir + "/_program_info.txt");
    }

    private String write(String outDir, String tag, String name, Function f, String orig,
            String loc, String sig, String c) throws Exception {
        String safe = name.replaceAll("[^A-Za-z0-9._-]", "_");
        if (safe.length() > 150) {
            safe = safe.substring(0, 150);
        }
        String base = tag + "__" + safe;
        PrintWriter pw = new PrintWriter(new OutputStreamWriter(
            new FileOutputStream(new File(outDir, base + ".c")), StandardCharsets.UTF_8));
        pw.println("// " + tag + "  " + name);
        pw.println("// original Go symbol : " + orig);
        pw.println("// source location    : " + loc);
        pw.println("// entry address      : " + f.getEntryPoint());
        pw.println("// ghidra function    : " + f.getName());
        pw.println("// body instructions  : " + f.getBody().getNumAddresses());
        pw.println("// signature          : " + (sig.isEmpty() ? "(none)" : sig));
        pw.println("");
        pw.println(c);
        pw.close();
        return base + ".c";
    }

    private static Long parseHex(String s) {
        String t = s.trim();
        if (t.startsWith("0x") || t.startsWith("0X")) {
            t = t.substring(2);
        }
        if (t.isEmpty()) {
            return null;
        }
        try {
            return Long.valueOf(Long.parseUnsignedLong(t, 16));
        }
        catch (Exception e) {
            return null;
        }
    }

    private BufferedReader open(String path) throws Exception {
        return new BufferedReader(new java.io.InputStreamReader(
            new FileInputStream(path), StandardCharsets.UTF_8), 1 << 20);
    }
}
"#;

// ---------------------------------------------------------------------------
// API de alto nivel
// ---------------------------------------------------------------------------

/// Resumen de una operación de Ghidra.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhidraReport {
    pub stage: String,
    pub ok: bool,
    pub exit_code: Option<i32>,
    pub elapsed_secs: u64,
    pub message: String,
    /// Métricas opcionales leídas del script.
    pub functions: Option<u64>,
    pub symbols_applied: Option<u64>,
    pub functions_decompiled: Option<u64>,
    /// Rutas de los ficheros de pseudocódigo generados.
    pub outputs: Vec<String>,
}

fn headless_base(tool: &GhidraTool, cfg: &GhidraConfig) -> Command {
    let mut cmd = Command::new(&tool.analyze_headless);
    if let Some(jh) = &tool.java_home {
        cmd.env("JAVA_HOME", jh);
    }
    cmd.env("MAXMEM", cfg.max_memory.clone());
    cmd
}

/// Escribe el GhidraScript embebido en el disco y devuelve su directorio.
fn stage_script(cfg: &GhidraConfig) -> GhidraResult<PathBuf> {
    fs::create_dir_all(&cfg.script_dir)
        .map_err(|e| GhidraError::Io(format!("no se pudo crear {}: {}", cfg.script_dir.display(), e)))?;
    let p = cfg.script_dir.join("MdGhidra.java");
    fs::write(&p, MD_GHIDRA_SCRIPT)
        .map_err(|e| GhidraError::Io(format!("no se pudo escribir {}: {}", p.display(), e)))?;
    Ok(cfg.script_dir.clone())
}

/// Importa un binario en el proyecto Ghidra (`-import`, sin auto-análisis).
pub fn import_binary(
    tool: &GhidraTool,
    cfg: &GhidraConfig,
    binary: &Path,
) -> GhidraResult<GhidraReport> {
    if !binary.exists() {
        return Err(GhidraError::Config(format!(
            "el binario {} no existe",
            binary.display()
        )));
    }
    let _guard = acquire_queue(cfg.queue_wait)?;
    let mut cmd = headless_base(tool, cfg);
    cmd.arg(&cfg.project_dir)
        .arg(&cfg.project_name)
        .arg("-import")
        .arg(binary)
        .arg("-noanalysis")
        .arg("-overwrite")
        .arg("-max-cpu")
        .arg("1")
        .arg("-log")
        .arg(cfg.project_dir.join("import.log"));
    let run = run_with_timeout(cmd, "import", cfg.timeout, 1 << 20)?;
    let ok = run.code == Some(0);
    Ok(GhidraReport {
        stage: "import".to_string(),
        ok,
        exit_code: run.code,
        elapsed_secs: run.elapsed_secs,
        message: if ok {
            format!("importado en {}/{}", cfg.project_dir.display(), cfg.project_name)
        } else {
            run.stderr
        },
        functions: None,
        symbols_applied: None,
        functions_decompiled: None,
        outputs: Vec::new(),
    })
}

/// Aplica el export de símbolos de `unstrip` sobre el programa del proyecto.
pub fn apply_symbols(
    tool: &GhidraTool,
    cfg: &GhidraConfig,
    symbols_tsv: &Path,
) -> GhidraResult<GhidraReport> {
    let script_dir = stage_script(cfg)?;
    let _guard = acquire_queue(cfg.queue_wait)?;
    let mut cmd = headless_base(tool, cfg);
    cmd.arg(&cfg.project_dir)
        .arg(&cfg.project_name)
        .arg("-process")
        .arg("-noanalysis")
        .arg("-max-cpu")
        .arg("1")
        .arg("-scriptPath")
        .arg(&script_dir)
        .arg("-postscript")
        .arg("MdGhidra.java")
        .arg("symbols")
        .arg(symbols_tsv)
        .arg("")
        .arg(&cfg.script_dir)
        .arg("");
    let run = run_with_timeout(cmd, "symbols", cfg.timeout, 1 << 21)?;
    let ok = run.code == Some(0);
    let applied = parse_counter(&run.stdout, "symbols done rows=");
    Ok(GhidraReport {
        stage: "symbols".to_string(),
        ok,
        exit_code: run.code,
        elapsed_secs: run.elapsed_secs,
        message: if ok {
            format!("símbolos aplicados: {}", applied.unwrap_or(0))
        } else {
            run.stderr
        },
        functions: None,
        symbols_applied: applied,
        functions_decompiled: None,
        outputs: Vec::new(),
    })
}

/// Resultado de decompilar un conjunto de anclas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecompileOutcome {
    pub report: GhidraReport,
    /// Un fichero por función decompilada.
    pub files: Vec<String>,
    /// Filas del informe `_decompile_report.tsv`.
    pub rows: Vec<DecompileRow>,
}

/// Una fila del informe de decompilación.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecompileRow {
    pub addr: String,
    pub name: String,
    pub orig: String,
    pub location: String,
    pub tag: String,
    pub ghidra_name: String,
    pub signature: String,
    pub body_size: String,
    pub dec_ok: String,
    pub dec_err: String,
    pub out_file: String,
}

/// Decompila las anclas (o la que case con `filter`) y vuelca el pseudocódigo.
pub fn decompile_anchors(
    tool: &GhidraTool,
    cfg: &GhidraConfig,
    anchors_tsv: &Path,
    out_dir: &Path,
    filter: &str,
) -> GhidraResult<DecompileOutcome> {
    fs::create_dir_all(out_dir)
        .map_err(|e| GhidraError::Io(format!("no se pudo crear {}: {}", out_dir.display(), e)))?;
    let script_dir = stage_script(cfg)?;
    let _guard = acquire_queue(cfg.queue_wait)?;
    let mut cmd = headless_base(tool, cfg);
    cmd.arg(&cfg.project_dir)
        .arg(&cfg.project_name)
        .arg("-process")
        .arg("-noanalysis")
        .arg("-max-cpu")
        .arg("1")
        .arg("-scriptPath")
        .arg(&script_dir)
        .arg("-postscript")
        .arg("MdGhidra.java")
        .arg("decompile")
        .arg("")
        .arg(anchors_tsv)
        .arg(out_dir)
        .arg(filter);
    let run = run_with_timeout(cmd, "decompile", cfg.timeout, 1 << 21)?;
    let ok = run.code == Some(0);

    let rows = read_decompile_report(&out_dir.join("_decompile_report.tsv"));
    let files: Vec<String> = rows
        .iter()
        .filter(|r| r.dec_ok == "true" && !r.out_file.is_empty())
        .map(|r| out_dir.join(&r.out_file).display().to_string())
        .collect();
    let n_ok = rows.iter().filter(|r| r.dec_ok == "true").count() as u64;

    Ok(DecompileOutcome {
        report: GhidraReport {
            stage: "decompile".to_string(),
            ok,
            exit_code: run.code,
            elapsed_secs: run.elapsed_secs,
            message: if ok {
                format!("{} función(es) decompilada(s)", n_ok)
            } else {
                run.stderr
            },
            functions: None,
            symbols_applied: None,
            functions_decompiled: Some(n_ok),
            outputs: files.clone(),
        },
        files,
        rows,
    })
}

/// Vuelca el inventario del programa (funciones, bloques, imports).
pub fn program_info(
    tool: &GhidraTool,
    cfg: &GhidraConfig,
    out_dir: &Path,
) -> GhidraResult<GhidraReport> {
    fs::create_dir_all(out_dir)
        .map_err(|e| GhidraError::Io(format!("no se pudo crear {}: {}", out_dir.display(), e)))?;
    let script_dir = stage_script(cfg)?;
    let _guard = acquire_queue(cfg.queue_wait)?;
    let mut cmd = headless_base(tool, cfg);
    cmd.arg(&cfg.project_dir)
        .arg(&cfg.project_name)
        .arg("-process")
        .arg("-noanalysis")
        .arg("-max-cpu")
        .arg("1")
        .arg("-scriptPath")
        .arg(&script_dir)
        .arg("-postscript")
        .arg("MdGhidra.java")
        .arg("info")
        .arg("")
        .arg("")
        .arg(out_dir)
        .arg("");
    let run = run_with_timeout(cmd, "info", cfg.timeout, 1 << 20)?;
    let ok = run.code == Some(0);
    let mut outputs = Vec::new();
    let info = out_dir.join("_program_info.txt");
    if info.exists() {
        outputs.push(info.display().to_string());
    }
    Ok(GhidraReport {
        stage: "info".to_string(),
        ok,
        exit_code: run.code,
        elapsed_secs: run.elapsed_secs,
        message: if ok {
            "inventario escrito".to_string()
        } else {
            run.stderr
        },
        functions: None,
        symbols_applied: None,
        functions_decompiled: None,
        outputs,
    })
}

fn parse_counter(text: &str, marker: &str) -> Option<u64> {
    for line in text.lines() {
        if let Some(i) = line.find(marker) {
            let rest = &line[i + marker.len()..];
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(v) = digits.parse::<u64>() {
                return Some(v);
            }
        }
    }
    None
}

fn read_decompile_report(path: &Path) -> Vec<DecompileRow> {
    let mut out = Vec::new();
    let raw = match fs::read_to_string(path) {
        Ok(r) => r,
        Err(_) => return out,
    };
    for (i, line) in raw.lines().enumerate() {
        if i == 0 || line.trim().is_empty() {
            continue;
        }
        let p: Vec<&str> = line.split('\t').collect();
        if p.len() < 11 {
            continue;
        }
        out.push(DecompileRow {
            addr: p[0].to_string(),
            name: p[1].to_string(),
            orig: p[2].to_string(),
            location: p[3].to_string(),
            tag: p[4].to_string(),
            ghidra_name: p[5].to_string(),
            signature: p[6].to_string(),
            body_size: p[7].to_string(),
            dec_ok: p[8].to_string(),
            dec_err: p[9].to_string(),
            out_file: p[10].to_string(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_handles_prefixes() {
        assert_eq!(parse_hex("0x4d06cb8"), Some(0x4d06cb8));
        assert_eq!(parse_hex("4d06cb8"), Some(0x4d06cb8));
        assert_eq!(parse_hex("zzz"), None);
    }

    #[test]
    fn detect_reports_missing_java_without_panicking() {
        let cfg = GhidraConfig {
            java_home: Some(PathBuf::from("/nonexistent-jdk")),
            ghidra_dir: Some(PathBuf::from("/nonexistent-ghidra")),
            ..GhidraConfig::default()
        };
        match GhidraTool::detect(&cfg) {
            Ok(_) => {
                // Si el host tiene Java y Ghidra, no hay nada que comprobar.
            }
            Err(e) => {
                let msg = e.to_string();
                assert!(!msg.is_empty());
            }
        }
    }

    #[test]
    fn embedded_script_is_not_empty_and_has_all_modes() {
        assert!(MD_GHIDRA_SCRIPT.contains("public class MdGhidra"));
        assert!(MD_GHIDRA_SCRIPT.contains("\"symbols\""));
        assert!(MD_GHIDRA_SCRIPT.contains("\"decompile\""));
        assert!(MD_GHIDRA_SCRIPT.contains("\"info\""));
    }
}
