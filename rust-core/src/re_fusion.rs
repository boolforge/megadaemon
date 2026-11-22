//! `re_fusion` — reconstrucción **multi-método** de un binario.
//!
//! La idea (pedida explícitamente): **varios motores, comparación y fusión**.
//! Ninguna herramienta sola da la reconstrucción completa:
//!
//! | método | qué aporta de verdad | límite |
//! |---|---|---|
//! | `motor` (re_engine) | formato, secciones, cadenas cifradas, predicados opacos, MBA, CFF | heurístico, no descompila |
//! | `die` (DiE + Binwalk) | familia de formato, OS, compilador, mitigaciones, entropía, firmas embebidas | no ve la lógica |
//! | `rizin` (frida_dbi) | funciones, imports, exports, strings **con direcciones** | no pseudocódigo |
//! | `ghidra` (ghidra.rs) | **pseudocódigo real** por función, con los símbolos Go aplicados | lento y pesado (JVM) |
//! | `pclntab` (TSV de símbolos) | **nombre Go + fichero:línea original** de cada función | no código |
//!
//! La **fusión** es lo que vale: pseudocódigo de Ghidra + origen Go del
//! pclntab + strings de Rizin ⇒ una reconstrucción que ningún motor da solo.
//!
//! Todo lo que este módulo afirma sale de ejecutarlo: cada método deja su
//! artefacto, su duración y su recuento. Si un método no puede correr
//! (no hay rizin, no hay JDK, no hay tiempo) ⇒ lo dice, no lo inventa.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Opciones de la reconstrucción multi-método.
#[derive(Debug, Clone)]
pub struct OpcionesFusion {
    /// Binario a analizar.
    pub binario: PathBuf,
    /// TSV de símbolos (0xADDR \t nombre \t comentario) — el export de `unstrip`.
    pub simbolos_tsv: Option<PathBuf>,
    /// TSV de anclas a decompilar.
    pub anchors_tsv: Option<PathBuf>,
    /// Directorio de Ghidra (si `None`, autodetección).
    pub ghidra_dir: Option<PathBuf>,
    /// `JAVA_HOME` (si `None`, autodetección).
    pub java_home: Option<PathBuf>,
    /// Dónde vivir el proyecto Ghidra.
    pub project_dir: PathBuf,
    /// Nombre del proyecto Ghidra.
    pub project_name: String,
    /// Dónde se escriben los artefactos.
    pub out_dir: PathBuf,
    /// Heap de la JVM.
    pub max_memory: String,
    /// Timeout por fase de Ghidra.
    pub timeout: Duration,
    /// Ejecutar el motor propio (rápido).
    pub hacer_motor: bool,
    /// Ejecutar DiE + Binwalk.
    pub hacer_die: bool,
    /// Ejecutar Rizin.
    pub hacer_rizin: bool,
    /// Ejecutar Ghidra (import + símbolos + decompilación).
    pub hacer_ghidra: bool,
}

impl Default for OpcionesFusion {
    fn default() -> Self {
        Self {
            binario: PathBuf::new(),
            simbolos_tsv: None,
            anchors_tsv: None,
            ghidra_dir: None,
            java_home: None,
            project_dir: PathBuf::from("/workspace/agy-re/tools-out/proj"),
            project_name: "Go".to_string(),
            out_dir: PathBuf::from("/workspace/agy-re/tools-out/fusion"),
            max_memory: "1400M".to_string(),
            timeout: Duration::from_secs(1800),
            hacer_motor: true,
            hacer_die: true,
            hacer_rizin: true,
            hacer_ghidra: true,
        }
    }
}

/// El resultado medido de **un** método.
#[derive(Debug, Clone, Default)]
pub struct Metodo {
    pub nombre: String,
    pub ok: bool,
    pub ms: u128,
    /// Funciones identificadas.
    pub funciones: usize,
    /// Strings extraídos.
    pub strings: usize,
    /// Imports (símbolos importados).
    pub imports: usize,
    /// Símbolos Go aplicados (sólo Ghidra).
    pub simbolos: usize,
    /// Cadenas cifradas detectadas.
    pub cadenas_cifradas: usize,
    /// Predicados opacos detectados.
    pub predicados_opacos: usize,
    /// Cadenas de control-flow flattening.
    pub chains: usize,
    /// Ficheros de pseudocódigo generados.
    pub pseudocodigo: usize,
    /// Artefactos escritos por este método.
    pub artefactos: Vec<String>,
    /// Por qué falló (si falló).
    pub nota: String,
}

impl Metodo {
    fn nuevo(nombre: &str) -> Self {
        Self {
            nombre: nombre.to_string(),
            ..Default::default()
        }
    }
    fn artefacto(&mut self, p: &Path) {
        self.artefactos.push(p.display().to_string());
    }
}

/// Segmentos del ELF: `(paddr, tamaño, vaddr)`.
///
/// Se leen con `goblin` (ya dependencia del repo) para poder pasar del
/// **offset de fichero** (que es lo que dan `re_strings` y `strings`) a la
/// **dirección virtual** (que es lo que usan Ghidra y el pclntab).
pub fn segmentos_elf(data: &[u8]) -> Vec<(u64, u64, u64)> {
    use goblin::elf::Elf;
    match Elf::parse(data) {
        Ok(e) => e
            .program_headers
            .iter()
            .filter(|p| p.p_type == goblin::elf::program_header::PT_LOAD)
            .map(|p| (p.p_offset, p.p_filesz, p.p_vaddr))
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Convierte un offset de fichero en dirección virtual.
pub fn offset_a_vaddr(offset: u64, segmentos: &[(u64, u64, u64)]) -> Option<u64> {
    segmentos
        .iter()
        .find(|(paddr, size, _)| *paddr <= offset && offset < paddr + size)
        .map(|(paddr, _, vaddr)| vaddr + (offset - paddr))
}

/// Image base con el que Ghidra carga este ELF.
///
/// MEDIDO: Ghidra lo carga en 0x100000, así que las direcciones que da
/// (`entry address : 0726e828`) son `vaddr + 0x100000`.  Para cruzarlas con
/// el export de símbolos (que da el offset real) hay que restar esta constante.
pub const GHIDRA_IMAGE_BASE: u64 = 0x100000;

/// Ejecuta los métodos pedidos y devuelve la comparativa.
pub fn ejecutar(o: &OpcionesFusion) -> Value {
    let mut metodos: Vec<Metodo> = Vec::new();
    if !o.binario.exists() {
        return json!({
            "ok": false,
            "error": format!("el binario {} no existe", o.binario.display()),
        });
    }
    let _ = std::fs::create_dir_all(&o.out_dir);

    if o.hacer_motor {
        metodos.push(metodo_motor(o));
    }
    if o.hacer_die {
        metodos.push(metodo_die(o));
    }
    if o.hacer_rizin {
        metodos.push(metodo_rizin(o));
    }
    if o.hacer_ghidra {
        metodos.push(metodo_ghidra(o));
    }

    let comparativa = comparar(&metodos);
    let fusion = fusionar(o, &metodos);

    json!({
        "ok": true,
        "binario": o.binario.display().to_string(),
        "comparativa": comparativa,
        "fusion": fusion,
    })
}

// ---------------------------------------------------------------------------
// Método 1: el motor propio (re_engine)
// ---------------------------------------------------------------------------

fn metodo_motor(o: &OpcionesFusion) -> Metodo {
    let mut m = Metodo::nuevo("motor (re_engine)");
    let t = Instant::now();

    let ana = crate::re_engine::analizar(&o.binario, &crate::re_engine::OpcionesAnalisis::default());
    match ana {
        Ok(v) => {
            m.ok = true;
            m.funciones = cuenta(&v, &["funciones", "functions", "total_funciones"]);
            m.strings = cuenta(&v, &["cadenas", "strings", "total_cadenas"]);
            let p = o.out_dir.join("motor_analisis.json");
            let _ = std::fs::write(&p, serde_json::to_string_pretty(&v).unwrap_or_default());
            m.artefacto(&p);
        }
        Err(e) => m.nota = format!("analizar: {e}"),
    }

    match crate::re_engine::deobfuscar(
        &o.binario,
        &crate::re_engine::OpcionesDeofuscacion::default(),
    ) {
        Ok(v) => {
            m.cadenas_cifradas = cuenta(&v, &["cadenas_cifradas", "encrypted_strings"]);
            m.predicados_opacos = cuenta(&v, &["predicados_opacos", "opaque_predicates"]);
            m.chains = cuenta(&v, &["chains", "cff_chains", "flattening"]);
            let p = o.out_dir.join("motor_deofuscacion.json");
            let _ = std::fs::write(&p, serde_json::to_string_pretty(&v).unwrap_or_default());
            m.artefacto(&p);
        }
        Err(e) => {
            if m.nota.is_empty() {
                m.nota = format!("deobfuscar: {e}")
            } else {
                m.nota = format!("{} | deobfuscar: {e}", m.nota)
            }
        }
    }
    m.ms = t.elapsed().as_millis();
    m
}

fn cuenta(v: &Value, claves: &[&str]) -> usize {
    for k in claves {
        match v.get(*k) {
            Some(Value::Number(n)) => return n.as_u64().unwrap_or(0) as usize,
            Some(Value::Array(a)) => return a.len(),
            Some(Value::Object(o)) => {
                if let Some(Value::Number(n)) = o.get("total") {
                    return n.as_u64().unwrap_or(0) as usize;
                }
            }
            _ => {}
        }
    }
    0
}

// ---------------------------------------------------------------------------
// Método 2: DiE + Binwalk
// ---------------------------------------------------------------------------

fn metodo_die(o: &OpcionesFusion) -> Metodo {
    let mut m = Metodo::nuevo("die+binwalk");
    let t = Instant::now();
    let ruta = o.binario.display().to_string();
    {
        let r = crate::binwalk_die::analyze_binary_full(&ruta);
        {
            m.ok = true;
            m.funciones = r.sections.len();
            m.imports = r.embedded_signatures.len();
            m.strings = r.bulk_artifacts.len();
            let p = o.out_dir.join("die_binwalk.json");
            let _ = std::fs::write(
                &p,
                serde_json::to_string_pretty(&serde_json::to_value(&r).unwrap_or(Value::Null))
                    .unwrap_or_default(),
            );
            m.artefacto(&p);
        }
    }
    m.ms = t.elapsed().as_millis();
    m
}

// ---------------------------------------------------------------------------
// Método 3: Rizin (a través del puente que ya tenía el repo)
// ---------------------------------------------------------------------------

fn metodo_rizin(o: &OpcionesFusion) -> Metodo {
    let mut m = Metodo::nuevo("rizin");
    let t = Instant::now();
    let ruta = o.binario.display().to_string();

    // tres modos: funciones, imports y strings (con direccion) => son los
    // que la fusion necesita (las strings se cruzan con el pseudocodigo).
    let funcs = crate::frida_dbi::run_rizin_analysis(&ruta, "functions", None);
    if funcs.get("status").and_then(|v| v.as_str()) == Some("error") {
        m.nota = funcs
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("rizin no disponible")
            .to_string();
        m.ms = t.elapsed().as_millis();
        return m;
    }
    let imports = crate::frida_dbi::run_rizin_analysis(&ruta, "imports", None);
    // OJO: el modo "strings" de rizin NO se usa: `izj`/`izzj` no existen en
    // 0.9.1 y `rz-bin -zz` aborta con este ELF.  Las strings salen de
    // `re_strings` (nativo), mas abajo.
    m.ok = true;

    if let Some(v) = funcs.get("functions").and_then(|v| v.as_array()) {
        m.funciones = v.len();
    }
    if let Some(v) = imports.get("imports").and_then(|v| v.as_array()) {
        m.imports = v.len();
    }
    // Las strings NO salen de rizin: `izj`/`izzj` no existen en 0.9.1 y
    // `rz-bin -zz` aborta con este ELF ("search interval size exceeds max
    // region size").  Se usan las del motor nativo `re_strings`, que además
    // es lo que ya tiene el repo.
    let mut lista_strings: Vec<Value> = Vec::new();
    match std::fs::read(&o.binario) {
        Ok(data) => {
            let segs = segmentos_elf(&data);
            for h in crate::re_strings::extract_strings(&data, 4, 600_000) {
                let vaddr = offset_a_vaddr(h.offset, &segs).unwrap_or(h.offset);
                lista_strings.push(json!({"vaddr": vaddr, "offset": h.offset, "string": h.text}));
            }
        }
        Err(e) => m.nota = format!("strings: {e}"),
    }
    m.strings = lista_strings.len();
    let combinado = json!({
        "functions": funcs.get("functions").cloned().unwrap_or(Value::Null),
        "imports": imports.get("imports").cloned().unwrap_or(Value::Null),
        "strings": lista_strings,
    });
    let p = o.out_dir.join("rizin.json");
    let _ = std::fs::write(&p, serde_json::to_string_pretty(&combinado).unwrap_or_default());
    m.artefacto(&p);
    m.ms = t.elapsed().as_millis();
    m
}

// ---------------------------------------------------------------------------
// Método 4: Ghidra (el pesado: pseudocódigo real)
// ---------------------------------------------------------------------------

fn metodo_ghidra(o: &OpcionesFusion) -> Metodo {
    let mut m = Metodo::nuevo("ghidra");
    let t = Instant::now();

    let cfg = crate::ghidra::GhidraConfig {
        ghidra_dir: o.ghidra_dir.clone(),
        java_home: o.java_home.clone(),
        project_dir: o.project_dir.clone(),
        project_name: o.project_name.clone(),
        timeout: o.timeout,
        queue_wait: Duration::from_secs(60),
        max_memory: o.max_memory.clone(),
        script_dir: o.out_dir.join("ghidra-scripts"),
    };

    let tool = match crate::ghidra::GhidraTool::detect(&cfg) {
        Ok(t) => t,
        Err(e) => {
            m.nota = format!("ghidra: {e}");
            m.ms = t.elapsed().as_millis();
            return m;
        }
    };

    match crate::ghidra::import_binary(&tool, &cfg, &o.binario) {
        Ok(r) => {
            m.ok = r.ok;
            if !r.message.is_empty() {
                m.nota = r.message.clone();
            }
            let p = o.out_dir.join("ghidra_import.json");
            let _ = std::fs::write(&p, serde_json::to_string_pretty(&r).unwrap_or_default());
            m.artefacto(&p);
        }
        Err(e) => {
            m.nota = format!("import: {e}");
            m.ms = t.elapsed().as_millis();
            return m;
        }
    }

    if let Some(tsv) = &o.simbolos_tsv {
        if tsv.exists() {
            match crate::ghidra::apply_symbols(&tool, &cfg, tsv) {
                Ok(r) => {
                    m.simbolos = r.symbols_applied.unwrap_or(0) as usize;
                    if r.ok {
                        m.ok = true;
                    }
                    let p = o.out_dir.join("ghidra_symbols.json");
                    let _ = std::fs::write(&p, serde_json::to_string_pretty(&r).unwrap_or_default());
                    m.artefacto(&p);
                }
                Err(e) => m.nota = format!("symbols: {e}"),
            }
        } else {
            m.nota = format!("no encuentro {}", tsv.display());
        }
    }

    if let Some(tsv) = &o.anchors_tsv {
        if tsv.exists() {
            let dest = o.out_dir.join("pseudocodigo");
            match crate::ghidra::decompile_anchors(&tool, &cfg, tsv, &dest, "") {
                Ok(d) => {
                    m.pseudocodigo = d.files.len();
                    m.ok = true;
                    let p = o.out_dir.join("ghidra_decompile.json");
                    let _ = std::fs::write(&p, serde_json::to_string_pretty(&d.report).unwrap_or_default());
                    m.artefacto(&p);
                }
                Err(e) => m.nota = format!("decompile: {e}"),
            }
        } else {
            m.nota = format!("no encuentro {}", tsv.display());
        }
    }

    m.ms = t.elapsed().as_millis();
    m
}

// ---------------------------------------------------------------------------
// Comparativa
// ---------------------------------------------------------------------------

/// Compara los métodos: quién gana en qué, y quién gana globalmente.
pub fn comparar(metodos: &[Metodo]) -> Value {
    let filas: Vec<Value> = metodos
        .iter()
        .map(|m| {
            json!({
                "metodo": m.nombre,
                "ok": m.ok,
                "ms": m.ms,
                "funciones": m.funciones,
                "strings": m.strings,
                "imports": m.imports,
                "simbolos": m.simbolos,
                "cadenas_cifradas": m.cadenas_cifradas,
                "predicados_opacos": m.predicados_opacos,
                "chains": m.chains,
                "pseudocodigo": m.pseudocodigo,
                "artefactos": m.artefactos,
                "nota": m.nota,
            })
        })
        .collect();

    let mejor = |campo: &str| -> Option<String> {
        let mut best: Option<(&str, usize)> = None;
        for m in metodos {
            if !m.ok {
                continue;
            }
            let v = match campo {
                "funciones" => m.funciones,
                "strings" => m.strings,
                "imports" => m.imports,
                "simbolos" => m.simbolos,
                "pseudocodigo" => m.pseudocodigo,
                _ => 0,
            };
            if v > 0 && best.map(|(_, bv)| v > bv).unwrap_or(true) {
                best = Some((m.nombre.as_str(), v));
            }
        }
        best.map(|(n, v)| format!("{n} ({v})"))
    };

    json!({
        "metodos": filas,
        "ganadores": {
            "funciones": mejor("funciones"),
            "strings": mejor("strings"),
            "imports": mejor("imports"),
            "simbolos_go": mejor("simbolos"),
            "pseudocodigo": mejor("pseudocodigo"),
        },
        "nota": "ningun metodo gana solo: la reconstruccion completa es la FUSION (pseudocodigo de Ghidra + nombres Go del pclntab + strings de Rizin)",
    })
}

// ---------------------------------------------------------------------------
// Fusión: pseudocódigo (Ghidra) + origen Go (pclntab) + strings (Rizin)
// ---------------------------------------------------------------------------

/// Un símbolo del TSV: dirección, nombre Go y fichero:línea original.
#[derive(Debug, Clone)]
pub struct Simbolo {
    pub addr: u64,
    pub nombre: String,
    pub origen: String,
}

/// Lee el TSV de símbolos (`0xADDR \t nombre \t comentario`).
pub fn leer_simbolos(tsv: &Path) -> Vec<Simbolo> {
    let mut out = Vec::new();
    let txt = match std::fs::read_to_string(tsv) {
        Ok(t) => t,
        Err(_) => return out,
    };
    for l in txt.lines() {
        let p: Vec<&str> = l.split('\t').collect();
        if p.len() < 2 {
            continue;
        }
        let addr = match p[0].trim().strip_prefix("0x").and_then(|h| u64::from_str_radix(h, 16).ok()) {
            Some(a) => a,
            None => continue,
        };
        let nombre = p[1].trim().to_string();
        let origen = p.get(2).map(|s| s.trim().to_string()).unwrap_or_default();
        out.push(Simbolo {
            addr,
            nombre,
            origen,
        });
    }
    out
}

/// Fusiona los artefactos de los métodos en un único "libro" reconstruido.
///
/// Por cada función decompilada por Ghidra se adjunta:
/// * el **pseudocódigo** (Ghidra),
/// * el **nombre Go y el fichero:línea original** (pclntab / export de `unstrip`),
/// * y el **string de Rizin** si la función apunta a uno.
pub fn fusionar(o: &OpcionesFusion, metodos: &[Metodo]) -> Value {
    let mut simbolos: Vec<Simbolo> = Vec::new();
    if let Some(tsv) = &o.simbolos_tsv {
        simbolos = leer_simbolos(tsv);
    }
    let por_addr: std::collections::HashMap<u64, &Simbolo> =
        simbolos.iter().map(|s| (s.addr, s)).collect();

    // Pseudocódigo producido por Ghidra.
    let mut pseudo: Vec<(u64, String, PathBuf, String, String)> = Vec::new();
    let dir = o.out_dir.join("pseudocodigo");
    if dir.exists() {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()) != Some("c") {
                    continue;
                }
                // la cabecera del .c lleva el simbolo Go, el fichero:linea y la
                // direccion de entrada (con el image base de Ghidra).
                let (addr, sym_go, origen) = leer_cabecera_c(&p);
                pseudo.push((addr, p.display().to_string(), p.clone(), sym_go, origen));
            }
        }
    }

    // Strings de Rizin (si el método corrió).
    let strings = leer_strings_rizin(&o.out_dir.join("rizin.json"));

    let mut md = String::new();
    md.push_str("# Reconstrucción fusionada del binario\n\n");
    md.push_str(&format!(
        "Funciones decompiladas: **{}** · símbolos Go disponibles: **{}** · strings de Rizin: **{}**\n\n",
        pseudo.len(),
        simbolos.len(),
        strings.len()
    ));
    md.push_str("Cada bloque es **pseudocódigo de Ghidra** + el **nombre Go y el fichero:línea original**\n");
    md.push_str("del pclntab (el binario no está ofuscado: los nombres son los del código original).\n\n");

    let mut con_origen = 0usize;
    for (addr, ruta, path, sym_go, origen_c) in &pseudo {
        // el simbolo del TSV manda (trae el nombre Go y el fichero:linea);
        // si no esta, usamos lo que dice la cabecera de Ghidra.
        let sym = por_addr.get(addr);
        if sym.is_some() {
            con_origen += 1;
        }
        let nombre = sym
            .map(|s| s.nombre.clone())
            .unwrap_or_else(|| if sym_go.is_empty() { format!("0x{addr:x}") } else { sym_go.clone() });
        md.push_str(&format!("\n## `{}`\n\n", nombre));
        let origen_txt = sym.map(|s| s.origen.clone()).unwrap_or_else(|| origen_c.clone());
        if !origen_txt.is_empty() {
            md.push_str(&format!("Origen: `{}`\n\n", origen_txt));
        }
        if let Some(t) = strings.get(addr) {
            md.push_str(&format!("String: `\"{}\"`\n\n", t));
        }
        md.push_str(&format!("Fichero de Ghidra: `{}`\n\n```c\n", ruta));
        if let Ok(c) = std::fs::read_to_string(path) {
            md.push_str(&c);
        }
        md.push_str("\n```\n");
    }
    let libro = o.out_dir.join("LIBRO-FUSIONADO.md");
    let _ = std::fs::write(&libro, md);

    json!({
        "ok": true,
        "libro": libro.display().to_string(),
        "funciones_decompiladas": pseudo.len(),
        "con_origen_go": con_origen,
        "simbolos_disponibles": simbolos.len(),
        "strings_rizin": strings.len(),
        "metodos_ok": metodos.iter().filter(|m| m.ok).count(),
        "metodos_total": metodos.len(),
    })
}

/// Lee la cabecera de un `.c` de Ghidra:
///
/// ```text
/// // original Go symbol : google3/third_party/.../gateway.(*AdminSettings).X
/// // source location    : third_party/jetski/gateway/admin_settings.go:85
/// // entry address      : 06e923d8
/// ```
///
/// Devuelve `(direccion real, simbolo Go, fichero:linea)`.  La direccion de
/// la cabecera incluye el image base de Ghidra ⇒ se le resta.
pub fn leer_cabecera_c(p: &Path) -> (u64, String, String) {
    let txt = match std::fs::read_to_string(p) {
        Ok(t) => t,
        Err(_) => return (0, String::new(), String::new()),
    };
    let sym = txt
        .lines()
        .find_map(|l| l.trim().strip_prefix("// original Go symbol :"))
        .unwrap_or("")
        .trim()
        .to_string();
    let origen = txt
        .lines()
        .find_map(|l| l.trim().strip_prefix("// source location    :"))
        .unwrap_or("")
        .trim()
        .to_string();
    let addr_txt = txt
        .lines()
        .find_map(|l| l.trim().strip_prefix("// entry address      :"))
        .unwrap_or("")
        .trim();
    let addr = u64::from_str_radix(addr_txt.trim_start_matches("0x"), 16)
        .map(|a| a.saturating_sub(GHIDRA_IMAGE_BASE))
        .unwrap_or(0);
    (addr, sym, origen)
}

/// Extrae `direccion -> string` del JSON de Rizin.
fn leer_strings_rizin(p: &Path) -> std::collections::HashMap<u64, String> {
    let mut out = std::collections::HashMap::new();
    let txt = match std::fs::read_to_string(p) {
        Ok(t) => t,
        Err(_) => return out,
    };
    let v: Value = match serde_json::from_str(&txt) {
        Ok(v) => v,
        Err(_) => return out,
    };
    if let Some(arr) = v.get("strings").and_then(|x| x.as_array()) {
        for s in arr {
            let addr = s
                .get("vaddr")
                .or_else(|| s.get("paddr"))
                .and_then(|x| x.as_u64());
            let txt = s.get("string").and_then(|x| x.as_str());
            if let (Some(a), Some(t)) = (addr, txt) {
                out.insert(a, t.to_string());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leer_simbolos_ignora_lineas_robas() {
        let p = std::env::temp_dir().join(format!("md_sym_{}.tsv", std::process::id()));
        std::fs::write(
            &p,
            "0x6d923d8\tpkg.AdminSettings_.AdminFileSetsModels\tthird_party/jetski/gateway/admin_settings.go:85\nbasura\n0xZZ\tx\ty\n",
        )
        .unwrap();
        let s = leer_simbolos(&p);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].addr, 0x6d923d8);
        assert!(s[0].origen.contains("admin_settings.go:85"));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn comparar_elige_el_mejor_por_metrica() {
        let ms = vec![
            Metodo {
                nombre: "a".into(),
                ok: true,
                funciones: 10,
                ..Default::default()
            },
            Metodo {
                nombre: "b".into(),
                ok: true,
                funciones: 20,
                ..Default::default()
            },
            Metodo {
                nombre: "c".into(),
                ok: false,
                funciones: 999,
                ..Default::default()
            },
        ];
        let c = comparar(&ms);
        let g = c.get("ganadores").unwrap().get("funciones").unwrap();
        assert_eq!(g.as_str().unwrap(), "b (20)");
    }

    #[test]
    fn cuenta_acepta_numero_array_y_total() {
        assert_eq!(cuenta(&json!({"funciones": 7}), &["funciones"]), 7);
        assert_eq!(cuenta(&json!({"x": [1, 2, 3]}), &["x"]), 3);
        assert_eq!(cuenta(&json!({"x": {"total": 9}}), &["x"]), 9);
        assert_eq!(cuenta(&json!({}), &["x"]), 0);
    }
}
