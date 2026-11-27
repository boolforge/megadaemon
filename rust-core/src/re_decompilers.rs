//! MegaDaemon — registro de motores de decompilación (multi-motor).
//!
//! Un **mismo contrato** de entrada y de salida para varios motores, de modo
//! que se puedan lanzar por separado, comparar y luego fusionar:
//!
//! * **Entrada**: un TSV de anclas `0xADDR \t nombre \t simbolo_original \t loc \t tag`
//!   (el mismo que consume el GhidraScript embebido en `crate::ghidra`).
//! * **Salida**: un `.c` por función en `<out>/<tag>__<nombre>.c`, con la misma
//!   cabecera `//` que escribe Ghidra, más un `_decompile_report.tsv` con las
//!   mismas columnas ⇒ `crate::re_fusion` puede fusionar cualquiera de los dos.
//!
//! Motores:
//! * [`Backend::Ghidra`] — el motor propio: Ghidra headless + `MdGhidra.java`.
//! * [`Backend::RizinGhidra`] — radare2 + el plugin **r2ghidra**: el *mismo*
//!   decompilador de Ghidra compilado a C++, sin JVM. Medido en este repo:
//!   ~5,6 funciones/s frente a ~1,2 funciones/s de la JVM (misma máquina).
//!
//! Nada de esto sustituye a `crate::ghidra`: es el segundo motor para
//! **contrastar** el resultado (doctrina de `docs/RECONSTRUCCION_MULTI_METODO.md`).

use crate::ghidra::{Anchor, DecompileOutcome, DecompileRow, GhidraReport};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

/// Error del registro de motores.
#[derive(Debug)]
pub enum DeError {
    /// Falta una herramienta o una ruta mal puesta.
    Config(String),
    /// Fallo de E/S.
    Io(String),
    /// La herramienta se ejecutó pero terminó mal.
    Exec(String),
}

impl fmt::Display for DeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeError::Config(m) => write!(f, "config: {}", m),
            DeError::Io(m) => write!(f, "io: {}", m),
            DeError::Exec(m) => write!(f, "exec: {}", m),
        }
    }
}

impl std::error::Error for DeError {}

pub type DeResult<T> = Result<T, DeError>;

/// Motor de decompilación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Backend {
    /// Ghidra headless con el GhidraScript embebido (`crate::ghidra`).
    Ghidra,
    /// radare2 con el plugin r2ghidra.
    RizinGhidra,
}

impl Backend {
    /// Todos los motores que el registro sabe lanzar.
    pub const ALL: [Backend; 2] = [Backend::Ghidra, Backend::RizinGhidra];

    /// Nombre estable para CLI y para los informes.
    pub fn as_str(self) -> &'static str {
        match self {
            Backend::Ghidra => "ghidra",
            Backend::RizinGhidra => "rizin-ghidra",
        }
    }

    /// Interpreta el nombre de un motor (`ghidra`, `rizin-ghidra`, `r2ghidra`…).
    pub fn parse(s: &str) -> Option<Backend> {
        match s.trim().to_ascii_lowercase().as_str() {
            "ghidra" => Some(Backend::Ghidra),
            "rizin-ghidra" | "r2ghidra" | "rizin" => Some(Backend::RizinGhidra),
            _ => None,
        }
    }
}

impl fmt::Display for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Configuración del motor r2ghidra
// ---------------------------------------------------------------------------

/// Dónde está radare2, el plugin r2ghidra y cómo hay que lanzarlo.
///
/// Las rutas por defecto son **las medidas en este sandbox** (2026-11-27):
/// el plugin exige `GLIBC_2.38` y `GLIBCXX_3.4.32`, así que hay que correr
/// bajo una glibc más nueva mediante el cargador explícito.
#[derive(Debug, Clone)]
pub struct RizinGhidraConfig {
    /// Binario de radare2 (o el cargador que lo envuelve).
    pub r2_bin: PathBuf,
    /// Cargador ELF (`ld-linux-x86-64.so.2`) si hace falta otra glibc.
    pub loader: Option<PathBuf>,
    /// Rutas de bibliotecas para el cargador.
    pub library_path: Option<String>,
    /// Directorio con los `.sla`/`.pspec` de r2ghidra (`SLEIGHHOME`).
    pub sleigh_home: Option<PathBuf>,
    /// Segundos máximos por lote.
    pub timeout_secs: u64,
    /// Funciones por invocación de radare2 (amortiza el arranque).
    pub chunk: usize,
    /// Tope del tamaño de una función cuando se estima por la siguiente VA.
    pub max_func_bytes: u64,
}

impl Default for RizinGhidraConfig {
    fn default() -> Self {
        Self {
            r2_bin: PathBuf::from("/workspace/re-tools/r2/usr/bin/r2"),
            loader: Some(PathBuf::from(
                "/workspace/ubuntu-glibc/usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2",
            )),
            library_path: Some(
                "/workspace/re-tools/glibc39/usr/lib/x86_64-linux-gnu:\
                 /workspace/ubuntu-glibc/usr/lib/x86_64-linux-gnu:\
                 /workspace/re-tools/r2/usr/lib:\
                 /lib/x86_64-linux-gnu:/usr/lib/x86_64-linux-gnu"
                    .to_string(),
            ),
            sleigh_home: Some(PathBuf::from(
                "/workspace/.home/.local/share/radare2/plugins/r2ghidra_sleigh",
            )),
            timeout_secs: 3600,
            chunk: 400,
            max_func_bytes: 0x40000,
        }
    }
}

impl RizinGhidraConfig {
    /// Configuración desde variables de entorno (para el móvil y para CI).
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        if let Ok(v) = std::env::var("MD_R2_BIN") {
            cfg.r2_bin = PathBuf::from(v);
        }
        if let Ok(v) = std::env::var("MD_R2_LOADER") {
            cfg.loader = if v.is_empty() { None } else { Some(PathBuf::from(v)) };
        }
        if let Ok(v) = std::env::var("MD_R2_LIBRARY_PATH") {
            cfg.library_path = Some(v);
        }
        if let Ok(v) = std::env::var("MD_R2_SLEIGH_HOME") {
            cfg.sleigh_home = if v.is_empty() { None } else { Some(PathBuf::from(v)) };
        }
        if let Ok(v) = std::env::var("MD_R2_CHUNK") {
            if let Ok(n) = v.parse::<usize>() {
                if n > 0 {
                    cfg.chunk = n;
                }
            }
        }
        if let Ok(v) = std::env::var("MD_R2_TIMEOUT") {
            if let Ok(n) = v.parse::<u64>() {
                cfg.timeout_secs = n;
            }
        }
        cfg
    }
}

/// Comprueba que el motor está instalado y devuelve la ruta del ejecutable.
pub fn detect_rizin_ghidra(cfg: &RizinGhidraConfig) -> DeResult<PathBuf> {
    if !cfg.r2_bin.exists() {
        return Err(DeError::Config(format!(
            "no encuentro radare2 en {} (ajusta MD_R2_BIN)",
            cfg.r2_bin.display()
        )));
    }
    if let Some(home) = &cfg.sleigh_home {
        if !home.exists() {
            return Err(DeError::Config(format!(
                "no encuentro el directorio sleigh de r2ghidra en {} (ajusta MD_R2_SLEIGH_HOME)",
                home.display()
            )));
        }
    }
    Ok(cfg.r2_bin.clone())
}

// ---------------------------------------------------------------------------
// Utilidades compartidas
// ---------------------------------------------------------------------------

/// Convierte un nombre de símbolo en un nombre de fichero seguro.
///
/// Misma regla que el GhidraScript embebido: `[A-Za-z0-9._-]` y 150 caracteres.
pub fn sanitize_symbol(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if out.len() > 150 {
        out.truncate(150);
    }
    out
}

/// Estima el tamaño de cada función: la diferencia con la siguiente VA.
///
/// Las fronteras **no se adivinan**: salen de la tabla del `pclntab` de Go
/// (`go.funcs.tsv`), donde las funciones van en orden de dirección. El tope
/// `max_func_bytes` evita que un hueco enorme (datos entre funciones) se
/// interprete como cuerpo.
pub fn sizes_por_frontera(anchors: &[Anchor], max_func_bytes: u64) -> std::collections::BTreeMap<u64, u64> {
    let mut orden: Vec<&Anchor> = anchors.iter().collect();
    orden.sort_by_key(|a| a.addr);
    let mut mapa = std::collections::BTreeMap::new();
    for (i, a) in orden.iter().enumerate() {
        let tam = if i + 1 < orden.len() {
            let d = orden[i + 1].addr.saturating_sub(a.addr);
            if d == 0 || d > max_func_bytes {
                max_func_bytes
            } else {
                d
            }
        } else {
            max_func_bytes
        };
        mapa.insert(a.addr, tam);
    }
    mapa
}

/// Construye el script de radare2 para un lote de anclas.
///
/// Receta **medida** (2026-11-27) contra `liblanguage_server.so`:
/// `af+ addr nombre; afb+ addr addr tamano; pdg @ addr > fichero`
///
/// * El nombre debe ser **único**: si se repite, radare2 dice "dupped function".
/// * `afb+` toma `<fcn_at> <bbat> <bbsz>`: con el orden equivocado, radare2
///   avisa "Unaligned function" y el resultado es basura.
pub fn build_r2_script(anchors: &[Anchor], sizes: &std::collections::BTreeMap<u64, u64>, out_dir: &Path) -> String {
    let mut usados: BTreeSet<String> = BTreeSet::new();
    let mut script = String::new();
    for a in anchors {
        let tam = sizes.get(&a.addr).copied().unwrap_or(0x100);
        let mut nombre = sanitize_symbol(&a.name);
        if !usados.insert(nombre.clone()) {
            nombre = format!("{}_{:x}", nombre, a.addr);
        }
        // MEDIDO 2026-11-27: el `tag` es la ruta del paquete Go y lleva "/" (p.ej.
        // "third_party/go"); el script de Ghidra lo usaba tal cual en el nombre
        // de fichero y fallaba con FileNotFoundException en 351 de 400
        // funciones. Aqui se sanea igual que en el motor propio.
        let safe_tag = sanitize_symbol(&a.tag);
        let destino = out_dir.join(format!("{}__{}.c", safe_tag, nombre));
        script.push_str(&format!(
            "af+ {:#x} {}; afb+ {:#x} {:#x} {:#x}; pdg @ {:#x} > {}\n",
            a.addr,
            nombre,
            a.addr,
            a.addr,
            tam,
            a.addr,
            destino.display()
        ));
    }
    script
}

/// Cabecera `//` que escribe Ghidra: la misma, para que la fusión no distinga.
fn cabecera_comentario(a: &Anchor, motor: &str) -> String {
    format!(
        "// {}\n// original Go symbol : {}\n// source location    : {}\n// entry address      : {:#x}\n// decompiler         : {}\n\n",
        a.tag, a.orig, a.location, a.addr, motor
    )
}

// ---------------------------------------------------------------------------
// El motor r2ghidra
// ---------------------------------------------------------------------------

/// Lanza radare2 con el plugin r2ghidra y decompila las anclas.
///
/// Devuelve el mismo `DecompileOutcome` que el motor de Ghidra ⇒ quien llama
/// no distingue el motor salvo por `report.stage`.
pub fn decompile_anchors_rizin(
    binario: &Path,
    cfg: &RizinGhidraConfig,
    anchors: &[Anchor],
    out_dir: &Path,
) -> DeResult<DecompileOutcome> {
    let inicio = Instant::now();
    let r2 = detect_rizin_ghidra(cfg)?;
    fs::create_dir_all(out_dir)
        .map_err(|e| DeError::Io(format!("no se pudo crear {}: {}", out_dir.display(), e)))?;

    let sizes = sizes_por_frontera(anchors, cfg.max_func_bytes);
    let mut filas: Vec<DecompileRow> = Vec::new();
    let mut ficheros: Vec<String> = Vec::new();
    let mut n_ok = 0u64;
    let mut ultimo_error = String::new();

    let mut lote: Vec<&Anchor> = Vec::new();
    let mut procesadas = 0usize;
    for a in anchors {
        lote.push(a);
        if lote.len() >= cfg.chunk {
            procesadas += lote.len();
            match decompile_lote(&r2, cfg, binario, &lote, &sizes, out_dir) {
                Ok((rows, files)) => {
                    n_ok += rows.iter().filter(|r| r.dec_ok == "true").count() as u64;
                    ficheros.extend(files);
                    filas.extend(rows);
                }
                Err(e) => ultimo_error = e.to_string(),
            }
            lote.clear();
        }
    }
    if !lote.is_empty() {
        procesadas += lote.len();
        match decompile_lote(&r2, cfg, binario, &lote, &sizes, out_dir) {
            Ok((rows, files)) => {
                n_ok += rows.iter().filter(|r| r.dec_ok == "true").count() as u64;
                ficheros.extend(files);
                filas.extend(rows);
            }
            Err(e) => ultimo_error = e.to_string(),
        }
    }

    // Informe con las mismas columnas que el de Ghidra.
    let informe = out_dir.join("_decompile_report.tsv");
    let mut w = fs::File::create(&informe)
        .map_err(|e| DeError::Io(format!("no se pudo escribir {}: {}", informe.display(), e)))?;
    use std::io::Write;
    writeln!(
        w,
        "addr\tname\torig\tloc\ttag\tghidra_name\tsig\tbody\tdec_ok\tdec_err\tout"
    )
    .map_err(|e| DeError::Io(e.to_string()))?;
    for r in &filas {
        writeln!(
            w,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            r.addr, r.name, r.orig, r.location, r.tag, r.ghidra_name, r.signature, r.body_size, r.dec_ok, r.dec_err, r.out_file
        )
        .map_err(|e| DeError::Io(e.to_string()))?;
    }

    let ok = n_ok > 0 && ultimo_error.is_empty();
    Ok(DecompileOutcome {
        report: GhidraReport {
            stage: "decompile:rizin-ghidra".to_string(),
            ok,
            exit_code: if ok { Some(0) } else { Some(1) },
            elapsed_secs: inicio.elapsed().as_secs(),
            message: if ok {
                format!("{} función(es) decompilada(s) de {}", n_ok, procesadas)
            } else if ultimo_error.is_empty() {
                "ninguna función decompilada".to_string()
            } else {
                ultimo_error.clone()
            },
            functions: None,
            symbols_applied: None,
            functions_decompiled: Some(n_ok),
            outputs: ficheros.clone(),
        },
        files: ficheros,
        rows: filas,
    })
}

/// Decompila un lote en una única invocación de radare2.
fn decompile_lote(
    r2: &Path,
    cfg: &RizinGhidraConfig,
    binario: &Path,
    lote: &[&Anchor],
    sizes: &std::collections::BTreeMap<u64, u64>,
    out_dir: &Path,
) -> DeResult<(Vec<DecompileRow>, Vec<String>)> {
    let script = build_r2_script(lote, sizes, out_dir);
    let ruta_script = out_dir.join("_lote.r2");
    fs::write(&ruta_script, &script)
        .map_err(|e| DeError::Io(format!("no se pudo escribir {}: {}", ruta_script.display(), e)))?;

    let mut cmd = if let Some(loader) = &cfg.loader {
        let mut c = Command::new(loader);
        c.arg("--library-path");
        if let Some(lp) = &cfg.library_path {
            c.arg(lp);
        }
        c.arg(r2);
        c
    } else {
        let mut c = Command::new(r2);
        if let Some(lp) = &cfg.library_path {
            c.env("LD_LIBRARY_PATH", lp);
        }
        c
    };
    if let Some(home) = &cfg.sleigh_home {
        cmd.env("SLEIGHHOME", home);
    }
    cmd.arg("-q")
        .arg("-N")
        .arg("-e")
        .arg("scr.color=0")
        .arg("-i")
        .arg(&ruta_script)
        .arg(binario)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let salida = cmd.output().map_err(|e| DeError::Exec(format!("no se pudo lanzar radare2: {}", e)))?;

    let mut filas = Vec::new();
    let mut ficheros = Vec::new();
    for a in lote {
        let nombre = sanitize_symbol(&a.name);
        let base = format!("{}__{}.c", sanitize_symbol(&a.tag), nombre);
        let ruta = out_dir.join(&base);
        let (dec_ok, dec_err, cuerpo) = match fs::read_to_string(&ruta) {
            Ok(c) if c.trim().len() > 20 => ("true".to_string(), String::new(), Some(c)),
            Ok(_) => ("false".to_string(), "salida-vacia".to_string(), None),
            Err(_) => ("false".to_string(), "sin-salida".to_string(), None),
        };
        if let Some(cuerpo) = cuerpo {
            // Se antepone la cabecera de Ghidra para que la fusión sea homogénea.
            let mut final_txt = cabecera_comentario(a, "r2ghidra (radare2 6.2.6 + r2ghidra 6.2.6)");
            final_txt.push_str(&cuerpo);
            let _ = fs::write(&ruta, final_txt);
            ficheros.push(ruta.display().to_string());
        }
        filas.push(DecompileRow {
            addr: format!("{:#x}", a.addr),
            name: a.name.clone(),
            orig: a.orig.clone(),
            location: a.location.clone(),
            tag: a.tag.clone(),
            ghidra_name: nombre,
            signature: String::new(),
            body_size: sizes.get(&a.addr).copied().unwrap_or(0).to_string(),
            dec_ok,
            dec_err,
            out_file: base,
        });
    }
    let _ = salida;
    Ok((filas, ficheros))
}

// ---------------------------------------------------------------------------
// Comparativa entre motores
// ---------------------------------------------------------------------------

/// Resultado de comparar el pseudocódigo de dos motores sobre las mismas
/// funciones. Es la forma *medible* de la doctrina multi-motor: no se fía de
/// "parece que el segundo es mejor", lo cuenta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparacionFila {
    pub addr: String,
    pub nombre: String,
    pub lineas_a: usize,
    pub lineas_b: usize,
    pub jaccard_identificadores: f64,
    pub jaccard_llamadas: f64,
}

/// Resumen de la comparación.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparacionResumen {
    pub muestra: usize,
    pub con_pseudocodigo_a: usize,
    pub con_pseudocodigo_b: usize,
    pub jaccard_identificadores_mediana: f64,
    pub jaccard_llamadas_mediana: f64,
    pub motor_a: String,
    pub motor_b: String,
}

/// Identificadores significativos de un texto (los que no son palabras clave).
pub fn identificadores(texto: &str) -> BTreeSet<String> {
    const COMUN: [&str; 40] = [
        "if", "for", "while", "switch", "return", "int", "long", "char", "void", "uint", "ulong",
        "undefined", "undefined8", "code", "byte", "short", "float", "double", "bool", "size_t",
        "const", "static", "struct", "union", "enum", "typedef", "break", "continue", "else", "do",
        "goto", "default", "case", "sizeof", "true", "false", "null", "NULL", "var", "param",
    ];
    let mut out = BTreeSet::new();
    let mut palabra = String::new();
    for ch in texto.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            palabra.push(ch);
        } else {
            if palabra.len() > 2 && !COMUN.contains(&palabra.as_str()) {
                out.insert(std::mem::take(&mut palabra));
            }
            palabra.clear();
        }
    }
    out
}

/// Nombres de función que aparecen llamados (`nombre(`).
pub fn llamadas(texto: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let bytes: Vec<char> = texto.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_alphabetic() || bytes[i] == '_' {
            let inicio = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == '_') {
                i += 1;
            }
            // mirar el siguiente caracter no blanco
            let mut j = i;
            while j < bytes.len() && bytes[j].is_whitespace() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == '(' && j > inicio {
                out.insert(bytes[inicio..i].iter().collect());
            }
        } else {
            i += 1;
        }
    }
    out
}

/// Jaccard de dos conjuntos.
pub fn jaccard(a: &BTreeSet<String>, b: &BTreeSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let inter = a.intersection(b).count() as f64;
    let union = a.union(b).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

/// Compara dos conjuntos de pseudocódigo (motor A frente a motor B).
///
/// `a` y `b` van emparejados por índice: cada elemento es `(nombre, texto)`.
pub fn comparar_pseudocodigo(
    motor_a: &str,
    motor_b: &str,
    a: &[(String, String)],
    b: &[(String, String)],
) -> (Vec<ComparacionFila>, ComparacionResumen) {
    let n = a.len().min(b.len());
    let mut filas = Vec::with_capacity(n);
    let mut con_a = 0usize;
    let mut con_b = 0usize;
    let mut ids: Vec<f64> = Vec::with_capacity(n);
    let mut lla: Vec<f64> = Vec::with_capacity(n);
    for i in 0..n {
        let (nombre, ta) = &a[i];
        let (_, tb) = &b[i];
        let ok_a = !ta.trim().is_empty();
        let ok_b = !tb.trim().is_empty();
        if ok_a {
            con_a += 1;
        }
        if ok_b {
            con_b += 1;
        }
        let ji = jaccard(&identificadores(ta), &identificadores(tb));
        let jl = jaccard(&llamadas(ta), &llamadas(tb));
        ids.push(ji);
        lla.push(jl);
        filas.push(ComparacionFila {
            addr: nombre.clone(),
            nombre: nombre.clone(),
            lineas_a: ta.lines().count(),
            lineas_b: tb.lines().count(),
            jaccard_identificadores: (ji * 1000.0).round() / 1000.0,
            jaccard_llamadas: (jl * 1000.0).round() / 1000.0,
        });
    }
    let resumen = ComparacionResumen {
        muestra: n,
        con_pseudocodigo_a: con_a,
        con_pseudocodigo_b: con_b,
        jaccard_identificadores_mediana: mediana(&ids),
        jaccard_llamadas_mediana: mediana(&lla),
        motor_a: motor_a.to_string(),
        motor_b: motor_b.to_string(),
    };
    (filas, resumen)
}

fn mediana(v: &[f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut o = v.to_vec();
    o.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let m = o.len() / 2;
    let v = if o.len() % 2 == 0 {
        (o[m - 1] + o[m]) / 2.0
    } else {
        o[m]
    };
    (v * 1000.0).round() / 1000.0
}

/// Lee un `_decompile_report.tsv` (el que escribe Ghidra y el que escribe este
/// módulo): `addr name orig loc tag ghidra_name sig body dec_ok dec_err out`.
pub fn leer_informe(path: &Path) -> DeResult<Vec<DecompileRow>> {
    let raw = fs::read_to_string(path)
        .map_err(|e| DeError::Io(format!("no se pudo leer {}: {}", path.display(), e)))?;
    let mut out = Vec::new();
    for line in raw.lines() {
        if line.trim().is_empty() {
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
    Ok(out)
}

/// Compara el pseudocódigo que dejaron **dos motores** en dos directorios.
///
/// Se empareja por nombre de función (no por posición) ⇒ el resultado no
/// depende del orden en que cada motor wrote su informe.
pub fn comparar_con_pseudocodigo(
    dir_a: &Path,
    dir_b: &Path,
    motor_a: &str,
    motor_b: &str,
) -> (Vec<ComparacionFila>, ComparacionResumen) {
    let fa = leer_informe(&dir_a.join("_decompile_report.tsv")).unwrap_or_default();
    let fb = leer_informe(&dir_b.join("_decompile_report.tsv")).unwrap_or_default();
    let mapa_b: std::collections::BTreeMap<&str, &DecompileRow> =
        fb.iter().map(|r| (r.name.as_str(), r)).collect();
    let mut pares: Vec<(String, String)> = Vec::new();
    let mut nombres: Vec<String> = Vec::new();
    for r in fa.iter() {
        if r.dec_ok != "true" {
            continue;
        }
        let otro = match mapa_b.get(r.name.as_str()) {
            Some(o) => *o,
            None => continue,
        };
        let ta = fs::read_to_string(dir_a.join(&r.out_file)).unwrap_or_default();
        let tb = fs::read_to_string(dir_b.join(&otro.out_file)).unwrap_or_default();
        nombres.push(r.name.clone());
        pares.push((r.name.clone(), format!("{}\u{1}{}", ta, tb)));
    }
    // separamos los dos textos
    let mut a_txt: Vec<(String, String)> = Vec::with_capacity(pares.len());
    let mut b_txt: Vec<(String, String)> = Vec::with_capacity(pares.len());
    for (i, (n, t)) in pares.into_iter().enumerate() {
        let mut partes = t.splitn(2, '\u{1}');
        let ta = partes.next().unwrap_or("").to_string();
        let tb = partes.next().unwrap_or("").to_string();
        a_txt.push((nombres[i].clone(), ta));
        b_txt.push((nombres[i].clone(), tb));
    }
    comparar_pseudocodigo(motor_a, motor_b, &a_txt, &b_txt)
}

// ---------------------------------------------------------------------------
// Pruebas (sin herramientas externas: son sobre funciones puras)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn anchor(addr: u64, name: &str) -> Anchor {
        Anchor {
            addr,
            name: name.to_string(),
            orig: format!("pkg/{}", name),
            location: "pkg/file.go:1".to_string(),
            tag: "pkg".to_string(),
        }
    }

    #[test]
    fn nombres_de_motor_se_parsean() {
        assert_eq!(Backend::parse("ghidra"), Some(Backend::Ghidra));
        assert_eq!(Backend::parse("r2ghidra"), Some(Backend::RizinGhidra));
        assert_eq!(Backend::parse("RIZIN-GHIDRA"), Some(Backend::RizinGhidra));
        assert_eq!(Backend::parse("nada"), None);
        assert_eq!(Backend::Ghidra.as_str(), "ghidra");
        assert_eq!(Backend::RizinGhidra.as_str(), "rizin-ghidra");
    }

    #[test]
    fn tamaños_por_frontera_usan_la_siguiente_va() {
        let a = vec![anchor(0x100, "a"), anchor(0x180, "b")];
        let m = sizes_por_frontera(&a, 0x40000);
        assert_eq!(m.get(&0x100), Some(&0x80));
        assert_eq!(m.get(&0x180), Some(&0x40000));
    }

    #[test]
    fn tamaños_por_frontera_acotan_el_hueco() {
        let a = vec![anchor(0x100, "a"), anchor(0x100000, "b")];
        let m = sizes_por_frontera(&a, 0x400);
        assert_eq!(m.get(&0x100), Some(&0x400));
    }

    #[test]
    fn script_de_radare2_lleva_af_mas_afb_mas_pdg() {
        let a = vec![anchor(0x4d06cb8, "internal_abi.__RegArgs_.Dump")];
        let m = sizes_por_frontera(&a, 0x1000);
        let s = build_r2_script(&a, &m, Path::new("/tmp/x"));
        assert!(s.contains("af+ 0x4d06cb8"));
        assert!(s.contains("afb+ 0x4d06cb8 0x4d06cb8"));
        assert!(s.contains("pdg @ 0x4d06cb8"));
        assert!(s.contains("/tmp/x/pkg__internal_abi.__RegArgs_.Dump.c"));
    }

    #[test]
    fn nombres_duplicados_se_hacen_unicos() {
        let a = vec![anchor(0x10, "mismo"), anchor(0x20, "mismo")];
        let m = sizes_por_frontera(&a, 0x100);
        let s = build_r2_script(&a, &m, Path::new("/tmp/x"));
        assert!(s.contains("af+ 0x10 mismo"));
        assert!(s.contains("af+ 0x20 mismo_20"));
    }

    #[test]
    fn identificadores_ignoran_palabras_clave() {
        let ids = identificadores("int main(void) { return aux(1); }");
        assert!(ids.contains("main".to_string()));
        assert!(ids.contains("aux".to_string()));
        assert!(!ids.contains("int".to_string()));
        assert!(!ids.contains("return".to_string()));
    }

    #[test]
    fn llamadas_detectan_el_callee() {
        let c = llamadas("void f() { g(); h(1); }");
        assert!(c.contains("g".to_string()));
        assert!(c.contains("h".to_string()));
        assert!(!c.contains("f".to_string()));
    }

    #[test]
    fn jaccard_de_conjuntos() {
        let a: BTreeSet<String> = ["x", "y"].iter().map(|s| s.to_string()).collect();
        let b: BTreeSet<String> = ["y", "z"].iter().map(|s| s.to_string()).collect();
        let j = jaccard(&a, &b);
        assert!((j - 1.0 / 3.0).abs() < 0.0001);
        assert_eq!(jaccard(&a, &a), 1.0);
        assert_eq!(jaccard(&BTreeSet::new(), &BTreeSet::new()), 1.0);
    }

    #[test]
    fn comparacion_devuelve_resumen() {
        let a = vec![("f1".to_string(), "void f1(){ g(); }".to_string())];
        let b = vec![("f1".to_string(), "void f1(){ g(); }".to_string())];
        let (filas, resumen) = comparar_pseudocodigo("ghidra", "rizin-ghidra", &a, &b);
        assert_eq!(filas.len(), 1);
        assert_eq!(resumen.muestra, 1);
        assert_eq!(resumen.con_pseudocodigo_a, 1);
        assert!((resumen.jaccard_identificadores_mediana - 1.0).abs() < 0.0001);
    }


    #[test]
    fn informe_se_lee_con_las_mismas_columnas_que_ghidra() {
        let dir = std::env::temp_dir().join("md_re_decomp_test");
        let _ = fs::create_dir_all(&dir);
        let ruta = dir.join("_decompile_report.tsv");
        fs::write(
            &ruta,
            "addr\tname\torig\tloc\ttag\tghidra_name\tsig\tbody\tdec_ok\tdec_err\tout\n\
0x10\tf1\tpkg.f1\tpkg/f.go:1\tpkg\tf1\tvoid f1()\t32\ttrue\t\tpkg__f1.c\n",
        )
        .unwrap();
        let filas = leer_informe(&ruta).unwrap();
        assert_eq!(filas.len(), 1);
        assert_eq!(filas[0].name, "f1");
        assert_eq!(filas[0].dec_ok, "true");
        assert_eq!(filas[0].out_file, "pkg__f1.c");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sanea_simbolos_como_el_script_de_ghidra() {
        assert_eq!(sanitize_symbol("a/b.c d"), "a_b.c_d");
        assert_eq!(sanitize_symbol(&"x".repeat(300)).len(), 150);
    }
}
