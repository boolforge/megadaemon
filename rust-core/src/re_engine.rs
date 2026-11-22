//! Motor de RE/DEOFUSCACIÓN de MegaDaemon: **un único punto de entrada**.
//!
//! POR QUÉ (2026-11-22): el repo tenía **10 módulos de RE** en paralelo
//! (`apk_deobfuscator`, `apk_inventory`, `axml_parser`, `binwalk_die`,
//! `cirx_rtl`, `deobfuscate`, `dex_decompile`, `dex_origins`, `ghidra`,
//! `re_disasm`) y el agente no sabía cuál pedir ni qué esperar de cada uno.
//! Este módulo NO reescribe ninguno: los **orquesta**, los **documenta**
//! (catálogo de capacidades) y devuelve un **informe uniforme** con la
//! evidencia de cada paso.
//!
//! Lo que gana el agente (y lo que se midió):
//! * `catalogo()` ⇒ una sola respuesta con TODO lo que MegaDaemon sabe
//!   deofuscar/decompilar y cuándo usar cada módulo: el agente no tiene
//!   que adivinar nombres de tools.
//! * `identificar()` ⇒ el formato **con la evidencia** (magics, secciones,
//!   pclntab), no una suposición.
//! * `analizar()` ⇒ informe único con el mismo esquema para ELF, APK, DEX
//!   y Go: strings, secciones, símbolos, ofuscación detectada y qué se
//!   ha aplicado para deshacerla.
//!
//! Todo en Rust puro. Los binarios se leen; **no se ejecuta código del
//! analizado** (el sandbox es de análisis, no de ejecución).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

/// Formato del fichero analizado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Formato {
    Elf,
    Pe,
    MachO,
    Dex,
    Apk,
    /// ELF compilado con Go (detectado por el `pclntab`).
    GoElf,
    /// ELF empaquetado con UPX.
    Upx,
    Texto,
    Desconocido,
}

impl Formato {
    pub fn as_str(self) -> &'static str {
        match self {
            Formato::Elf => "elf",
            Formato::Pe => "pe",
            Formato::MachO => "macho",
            Formato::Dex => "dex",
            Formato::Apk => "apk",
            Formato::GoElf => "go-elf",
            Formato::Upx => "upx",
            Formato::Texto => "texto",
            Formato::Desconocido => "desconocido",
        }
    }
}

/// Resultado de identificar un binario, **con la evidencia**.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identificacion {
    pub formato: Formato,
    pub bytes: u64,
    /// Magics/heurísticas que lo determinan (con su偏移 en el fichero).
    pub evidencia: Vec<String>,
    pub bits: u32,
    pub endian: String,
    /// Si es Go: número de funciones recuperadas del `pclntab`.
    pub funciones_go: Option<u64>,
    /// Si es Go: `go.funcs.tsv` o similar generado.
    pub go_toolchain: Option<String>,
    /// Señales de ofuscación detectadas (con el porqué).
    pub ofuscacion: Vec<String>,
    /// Módulos del repo que sabe analizar este formato.
    pub modulos_aplicables: Vec<String>,
}

/// Identifica el formato de un fichero **con evidencia**.
pub fn identificar(path: &Path) -> Result<Identificacion, String> {
    let datos = std::fs::read(path).map_err(|e| format!("leer {}: {e}", path.display()))?;
    let bytes = datos.len() as u64;
    let mut ev: Vec<String> = Vec::new();
    let mut formato = Formato::Desconocido;
    let mut bits = 0u32;
    let mut endian = String::new();
    let mut funcs_go = None;
    let mut go_toolchain = None;
    let mut ofusc = Vec::new();

    if datos.starts_with(b"\x7fELF") {
        formato = Formato::Elf;
        bits = if datos.len() > 4 && datos[4] == 2 { 64 } else { 32 };
        // EI_DATA: 1 = LSB (little), 2 = MSB (big).
        endian = if datos.len() > 5 && datos[5] == 1 {
            "little".to_string()
        } else {
            "big".to_string()
        };
        ev.push(format!(
            "ELF magic @0 (EI_CLASS={} EI_DATA={})",
            if bits == 64 { "ELF64".to_string() } else { "ELF32".to_string() },
            endian
        ));
        // UPX: la firma aparece en el nombre de las secciones.
        if let Ok(elf) = goblin::elf::Elf::parse(&datos) {
            let secs: Vec<String> = elf
                .section_headers
                .iter()
                .filter_map(|sh| elf.shdr_strtab.get_at(sh.sh_name))
                .map(|s| s.to_string())
                .collect();
            if secs.iter().any(|s| s.starts_with("UPX")) {
                formato = Formato::Upx;
                ev.push(format!("secciones UPX: {secs:?}"));
            } else {
                ev.push(format!("secciones: {}", secs.len()));
            }
            // Símbolos stripped: pista de ofuscación/compilación.
            if elf.syms.is_empty() && elf.dynsyms.len() < 8 {
                ofusc.push(format!(
                    "sin tabla de simbolos (syms={}, dynsyms={}): binario stripped o empaquetado",
                    elf.syms.len(),
                    elf.dynsyms.len()
                ));
            }
        }
        // Go: magic del pclntab (0xfffffff1 = go1.18+).
        if let Some((n, ver)) = detectar_go_pclntab(&datos) {
            formato = Formato::GoElf;
            funcs_go = Some(n);
            go_toolchain = ver;
            ev.push(format!("pclntab go1.18+ encontrado: {n} funciones"));
        }
    } else if datos.starts_with(b"MZ") {
        formato = Formato::Pe;
        ev.push("MZ magic @0 (PE)".to_string());
    } else if datos.starts_with(b"\xcf\xfa\xed\xfe")
        || datos.starts_with(b"\xce\xfa\xed\xfe")
        || datos.starts_with(b"\xfe\xed\xfa\xcf")
    {
        formato = Formato::MachO;
        ev.push("Mach-O magic @0".to_string());
    } else if datos.starts_with(b"dex\n") {
        formato = Formato::Dex;
        ev.push("dex magic @0".to_string());
    } else if datos.starts_with(b"PK\x03\x04") {
        formato = Formato::Apk;
        ev.push("PK zip magic @0".to_string());
    } else if datos.iter().take(512).all(|b| *b >= 0x09 && *b < 0x7f) {
        formato = Formato::Texto;
        ev.push("todos los bytes imprimibles en los primeros 512".to_string());
    }

    let modulos = modulos_para(formato);
    Ok(Identificacion {
        formato,
        bytes,
        evidencia: ev,
        bits,
        endian,
        funciones_go: funcs_go,
        go_toolchain,
        ofuscacion: ofusc,
        modulos_aplicables: modulos,
    })
}

/// Módulos del repo que saben analizar cada formato.
pub fn modulos_para(f: Formato) -> Vec<String> {
    let mut v = Vec::new();
    match f {
        Formato::Elf | Formato::GoElf | Formato::Upx => {
            v.push("re_disasm::inspect_binary_provenance".to_string());
            v.push("re_disasm::disasm_capstone".to_string());
            v.push("go-retool (pclntab)".to_string());
            v.push("ghidra::import_binary".to_string());
            v.push("ghidra::decompile_anchors".to_string());
            v.push("binwalk_die".to_string());
            v.push("apk_deobfuscator (strings de assets)".to_string());
        }
        Formato::Pe | Formato::MachO => {
            v.push("re_disasm::inspect_binary_provenance".to_string());
            v.push("re_disasm::disasm_capstone".to_string());
            v.push("ghidra::import_binary".to_string());
        }
        Formato::Dex => {
            v.push("dex_decompile::decompile_class".to_string());
            v.push("dex_decompile::decompile_all".to_string());
            v.push("dex_origins".to_string());
            v.push("apk_deobfuscator".to_string());
        }
        Formato::Apk => {
            v.push("apk_inventory::apk_entries".to_string());
            v.push("apk_inventory::manifest_axml".to_string());
            v.push("axml_parser".to_string());
            v.push("dex_decompile".to_string());
            v.push("apk_deobfuscator".to_string());
            v.push("binwalk_die".to_string());
        }
        _ => {}
    }
    v
}

/// Busca el `pclntab` de Go (magic `0xfffffff1`, go1.18+) y cuenta funciones.
///
/// MEDIDO 2026-11-22 en el `liblanguage_server.so` de Antigravity: magic en
/// `.lrodata` @ `0x1449ff0`, **161.682 funciones**.
pub fn detectar_go_pclntab(datos: &[u8]) -> Option<(u64, Option<String>)> {
    // El magic 0xFFFFFFF1 se guarda en el fichero en ORDEN LITTLE-ENDIAN:
    // los bytes en el disco son F1 FF FF FF.  Con los bytes invertidos
    // (FF FF FF F1) la comparacion NUNCA se cumplia y el motor no detectaba
    // NINGUN binario Go: era el bug que hacia rojo el test del repo.
    const MAGIC: [u8; 4] = [0xf1, 0xff, 0xff, 0xff];
    // MEDIDO 2026-11-22 (liblanguage_server.so de Antigravity): hay **189**
    // apariciones del magic en el binario y sólo UNA es la pclntab de verdad.
    // La version anterior se quedaba con la PRIMERA que-"cuadraba" y devolvia
    // 17.684 en vez de 161.682 (el test del repo fallaba).
    //
    // El pcHeader de go1.18+ es:
    //   magic(4) pad1(1) pad2(1) minLC(1) ptrSize(1) nfunc(ptr) nfiles(ptr) ...
    // y se valida igual que lo hace `go-retool`: pad1==pad2==0, minLC!=0 y
    // ptrSize en {4,8}.  Entre las cabeceras validas se queda la de MAYOR
    // nfunc (la pclntab real tiene 161.682; las demas son copias parciales).
    let mut mejor: Option<(u64, String)> = None;
    let mut i = 0usize;
    while i + 16 <= datos.len() {
        if datos[i..i + 4] == MAGIC
            && datos[i + 4] == 0
            && datos[i + 5] == 0
            && datos[i + 6] != 0
            && (datos[i + 7] == 4 || datos[i + 7] == 8)
        {
            let ptr = datos[i + 7] as usize;
            let mut n: u64 = 0;
            for k in 0..ptr {
                n |= (datos[i + 8 + k] as u64) << (8 * k);
            }
            if n > 0 && n < 5_000_000 {
                let etiqueta = format!("go1.18+ (minLC={})", datos[i + 6]);
                if mejor.as_ref().map(|(m, _)| n > *m).unwrap_or(true) {
                    mejor = Some((n, etiqueta));
                }
            }
        }
        i += 4;
    }
    mejor.map(|(n, t)| (n, Some(t)))
}

/// Extrae cadenas imprimibles con su desplazamiento.
pub fn extraer_cadenas(datos: &[u8], min_len: usize, limite: usize) -> Vec<(u64, String)> {
    let mut out = Vec::new();
    let mut ini = 0usize;
    while ini < datos.len() && out.len() < limite {
        let c = datos[ini];
        if (0x20..0x7f).contains(&c) {
            let mut fin = ini;
            while fin < datos.len() && (0x20..0x7f).contains(&datos[fin]) {
                fin += 1;
            }
            if fin - ini >= min_len {
                let s = String::from_utf8_lossy(&datos[ini..fin]).to_string();
                out.push((ini as u64, s));
            }
            ini = fin;
        } else {
            ini += 1;
        }
    }
    out
}

/// Cadenas "interesantes" para un analista: URLs, rutas, paquetes, claves.
pub fn cadenas_interesantes(datos: &[u8], limite: usize) -> Vec<String> {
    let todas = extraer_cadenas(datos, 6, 400_000);
    let mut out = Vec::new();
    for (_, s) in todas {
        let baja = s.to_ascii_lowercase();
        let hit = baja.starts_with("http")
            || baja.contains("api_key")
            || baja.contains("apikey")
            || baja.contains("token")
            || baja.contains("password")
            || baja.contains("secret")
            || baja.contains("/data/")
            || baja.contains("android.")
            || baja.contains("com.")
            || baja.contains("dex")
            || baja.contains("jni")
            || baja.contains(".so")
            || baja.contains("/proc/")
            || baja.contains("magisk")
            || baja.contains("root");
        if hit && out.len() < limite {
            out.push(s);
        }
    }
    out
}

/// **Catálogo de capacidades**: qué sabe hacer MegaDaemon de RE.
///
/// Es la respuesta que el agente necesita para no adivinar: un único sitio
/// con el nombre de la capacidad, el módulo que la implementa y cuándo
/// usarla. Cuesta ~700 tokens y ahorra decenas de llamadas a ciegas.
pub fn catalogo() -> Value {
    json!({
        "version": "1",
        "principio": "MegaDaemon orquesta sus modulos de RE; no los reimplementa.",
        "capacidades": [
            {"cap": "identificar", "modulo": "re_engine::identificar",
             "para": "saber que es un binario (ELF/PE/MachO/DEX/APK/Go/UPX) CON evidencia"},
            {"cap": "strings", "modulo": "re_engine::extraer_cadenas / cadenas_interesantes",
             "para": "URLs, rutas, paquetes, claves, referencias a magisk/root/jni"},
            {"cap": "secciones-elf", "modulo": "re_disasm::inspect_binary_provenance",
             "para": "secciones, entry, syms/dynsyms, presencia de DWARF"},
            {"cap": "simbolos-go", "modulo": "go-retool (pclntab)",
             "para": "161.682 funciones del binario Go de Antigravity; export TSV/JSON"},
            {"cap": "import-ghidra", "modulo": "ghidra::import_binary + apply_symbols",
             "para": "meter los simbolos de Go en Ghidra headless"},
            {"cap": "decompilar-anchors", "modulo": "ghidra::decompile_anchors",
             "para": "pseudocodigo de los puntos de interes (handlers, catalogos)"},
            {"cap": "desensamblar", "modulo": "re_disasm::disasm_capstone / disasm_x86_iced",
             "para": "x86-64 (iced) y ARM64/x86 (capstone) con direcciones"},
            {"cap": "decompilar-dex", "modulo": "dex_decompile::decompile_class / decompile_all",
             "para": "DEX a Java legible (sin dex2jar)"},
            {"cap": "axml", "modulo": "apk_inventory::manifest_axml + axml_parser",
             "para": "AndroidManifest.xml binario"},
            {"cap": "inventario-apk", "modulo": "apk_inventory",
             "para": "entradas, hashes, manifiesto, permisos, DEX, .so"},
            {"cap": "deobfuscar-dex", "modulo": "apk_deobfuscator",
             "para": "nombres ofuscados, recursos y assets"},
            {"cap": "strings-cifrados", "modulo": "deobfuscate::deobfuscate_binary",
             "para": "strings con clave de plataforma (plat_key)"},
            {"cap": "scanner-firmas", "modulo": "binwalk_die",
             "para": "identificar el empaquetador y la familia del binario"},
            {"cap": "motor-unificado", "modulo": "re_engine::analizar",
             "para": "informe UNIFORME de cualquier formato, con la evidencia"},
            {"cap": "mba", "modulo": "re_mba + re_bdd",
             "para": "simplificar MBA y DEMOSTRAR la equivalencia (ROBDD, decision completa)"},
            {"cap": "control-flow-flattening", "modulo": "re_cff + re_ir",
             "para": "detectar el dispatcher, reaplanar y PROBAR la equivalencia (diferencial)"},
            {"cap": "strings-cifrados", "modulo": "re_crypto",
             "para": "buscar la clave periodica y extraer el texto descifrado (ida y vuelta)"},
            {"cap": "constantes-ofuscadas", "modulo": "re_crypto + re_cff",
             "para": "patron 'cargar constante + XOR con clave' y prueba en claro"},
            {"cap": "predicados-opacos", "modulo": "re_opaque (+ re_mba)",
             "para": "detectar y PLEGAR ramas siempre ciertas/falsas (bloque y expresion)"},
            {"cap": "motor-deofuscacion", "modulo": "re_engine::deobfuscar",
             "para": "TODO el motor de deofuscacion en un solo informe (md-re deobfuscate)"}
        ],
        "notas": [
            "Ninguna operacion ejecuta el binario analizado: es analisis estatico.",
            "Ghidra es headless y puede tardar: pasale solo los anchors que importan.",
            "El sandbox tiene 3 GB de RAM: el enlazado de los tests puede morir por OOM."
        ]
    })
}

/// Opciones del análisis.
#[derive(Debug, Clone)]
pub struct OpcionesAnalisis {
    pub strings: bool,
    pub max_cadenas: usize,
    pub secciones: bool,
    pub simbolos_go: bool,
    pub apk: bool,
}

impl Default for OpcionesAnalisis {
    fn default() -> Self {
        OpcionesAnalisis {
            strings: true,
            max_cadenas: 60,
            secciones: true,
            simbolos_go: true,
            apk: true,
        }
    }
}

/// Análisis completo con **informe uniforme**.
pub fn analizar(path: &Path, o: &OpcionesAnalisis) -> Result<Value, String> {
    let id = identificar(path)?;
    let datos = std::fs::read(path).map_err(|e| format!("leer: {e}"))?;
    let mut out = json!({
        "ok": true,
        "ruta": path.display().to_string(),
        "identificacion": serde_json::to_value(&id).unwrap_or(Value::Null),
    });

    if o.secciones {
        if let Ok(p) = crate::re_disasm::inspect_binary_provenance(path) {
            out["provenance"] = p;
        }
    }

    if o.strings {
        let inter = cadenas_interesantes(&datos, o.max_cadenas);
        out["cadenas_interesantes"] = json!(inter);
        out["cadenas_interesantes_total"] = json!(inter.len());
    }

    if o.simbolos_go && id.formato == Formato::GoElf {
        // go-retool es un crate aparte: se delega en su API pública.
        out["go"] = json!({
            "funciones": id.funciones_go,
            "toolchain": id.go_toolchain,
            "nota": "export TSV/JSON con go-retool (crates/go-retool) o md-re",
        });
    }

    if o.apk && id.formato == Formato::Apk {
        let entradas = crate::apk_inventory::apk_entries(&path.display().to_string());
        let total: u64 = entradas.iter().map(|e| e.uncompressed_size).sum();
        out["apk"] = json!({
            "entradas": entradas.len(),
            "bytes": total,
            "muestra": entradas.iter().take(15).map(|e| json!({
                "nombre": e.name, "tamano": e.uncompressed_size, "comprimido": e.compressed_size,
            })).collect::<Vec<_>>(),
        });
    }

    Ok(out)
}

// ---------------------------------------------------------------------------
// 10. MOTOR DE DEOFUSCACIÓN (todo en un solo sitio)
// ---------------------------------------------------------------------------

/// Opciones del motor de deofuscación.
#[derive(Debug, Clone)]
pub struct OpcionesDeofuscacion {
    /// Longitud mínima (bytes) para considerar una cadena cifrada.
    pub min_len_cifrado: usize,
    /// Máximo de cadenas cifradas a devolver.
    pub max_cadenas: usize,
    /// Ventana de código a desensamblar (bytes) para CFF y predicados opacos.
    pub ventana_disasm: usize,
    /// Entradas para la prueba diferencial del aplanado.
    pub entradas_prueba: usize,
}

impl Default for OpcionesDeofuscacion {
    fn default() -> Self {
        OpcionesDeofuscacion {
            min_len_cifrado: 12,
            max_cadenas: 20,
            ventana_disasm: 256 * 1024,
            entradas_prueba: 64,
        }
    }
}

/// Desensambla una ventana de código y devuelve el CFG del IR.
///
/// Devuelve `None` si la arquitectura no está soportada por Capstone.
pub fn cfg_de_ventana(
    datos: &[u8],
    arch: &str,
    base: u64,
    ventana: usize,
) -> Option<crate::re_ir::Cfg> {
    let rep = crate::re_disasm::disasm_capstone(datos, arch, base, 200_000).ok()?;
    let instrs: Vec<(u64, String)> = rep
        .instructions
        .iter()
        .filter_map(|i| {
            let addr = u64::from_str_radix(i.address.trim_start_matches("0x"), 16).ok()?;
            Some((addr, i.text.clone()))
        })
        .collect();
    if instrs.is_empty() {
        return None;
    }
    Some(crate::re_ir::Cfg::desde_disasm(&instrs))
}

/// Localiza la sección de código de un ELF y devuelve `(arquitectura, offset,
/// tamaño, dirección virtual)`.
pub fn seccion_de_codigo(datos: &[u8]) -> Option<(String, usize, usize, u64)> {
    let elf = goblin::elf::Elf::parse(datos).ok()?;
    let arch = match elf.header.e_machine {
        goblin::elf::header::EM_AARCH64 => "arm64".to_string(),
        goblin::elf::header::EM_X86_64 => "x86_64".to_string(),
        goblin::elf::header::EM_386 => "x86".to_string(),
        _ => return None,
    };
    for sh in &elf.section_headers {
        if elf.shdr_strtab.get_at(sh.sh_name) == Some(".text") {
            return Some((
                arch,
                sh.sh_offset as usize,
                sh.sh_size as usize,
                sh.sh_addr,
            ));
        }
    }
    None
}

/// Construye el programa plano de `re_cff` a partir del CFG real, para que el
/// aplanado se pueda **demostrar** por ejecución diferencial.
///
/// Cada bloque del CFG se traduce a un bloque del programa: las constantes
/// conocidas (`mov reg, inm`) pasan a asignaciones y el resto de instrucciones
/// pasan a *efectos* observables.  El dispatcher se traduce a un `switch` del
/// registro de estado, que es exactamente lo que es.
pub fn programa_desde_cfg(
    cfg: &crate::re_ir::Cfg,
    disp: &crate::re_ir::DispatcherCfg,
) -> Option<crate::re_cff::FlatProgram> {
    use crate::re_cff::{Block, FlatProgram, SVal, Stmt, Terminator};
    let idx_de = |addr: u64| -> Option<usize> { cfg.index_of(addr) };
    let mut bloques = Vec::with_capacity(cfg.blocks.len());
    for (i, b) in cfg.blocks.iter().enumerate() {
        let mut stmts: Vec<Stmt> = Vec::new();
        for ins in &b.ins {
            match ins.mnem.as_str() {
                "mov" | "movabs" | "movz" => {
                    if let Some(v) = crate::re_ir::parse_int(ins.op1()) {
                        stmts.push(Stmt::Assign {
                            dst: ins.op0().to_string(),
                            value: SVal::Const(v),
                        });
                        continue;
                    }
                    stmts.push(Stmt::Effect(ins.mnem.clone()));
                }
                "xor" if ins.op0() == ins.op1() => {
                    stmts.push(Stmt::Assign {
                        dst: ins.op0().to_string(),
                        value: SVal::Const(0),
                    });
                }
                _ => stmts.push(Stmt::Effect(
                    format!("{} {}", ins.mnem, ins.ops).trim().to_string(),
                )),
            }
        }
        let term = match &b.flow {
            crate::re_ir::Flow::Jump(t) => match idx_de(*t) {
                Some(x) => Terminator::Goto(x),
                None => Terminator::Halt,
            },
            crate::re_ir::Flow::Cond { target, .. } => {
                let then = idx_de(*target);
                let else_ = cfg.next_addr(b.addr).and_then(idx_de);
                match (then, else_) {
                    (Some(t), Some(e)) => Terminator::Cond {
                        cond: SVal::Const(1),
                        then_block: t,
                        else_block: e,
                    },
                    (Some(t), None) => Terminator::Goto(t),
                    _ => Terminator::Halt,
                }
            }
            crate::re_ir::Flow::Indirect { .. } => {
                if b.addr == disp.addr {
                    let cases: Vec<(i64, usize)> = disp
                        .casos
                        .iter()
                        .filter_map(|(k, dest)| idx_de(*dest).map(|x| (*k, x)))
                        .collect();
                    Terminator::Switch {
                        state_var: disp.variable_estado.clone(),
                        cases,
                        default: None,
                    }
                } else {
                    Terminator::Halt
                }
            }
            crate::re_ir::Flow::Ret => Terminator::Halt,
            _ => Terminator::Halt,
        };
        bloques.push(Block {
            id: i,
            stmts,
            term,
        });
    }
    Some(FlatProgram {
        entry: 0,
        blocks: bloques,
    })
}

/// **Motor de deofuscación**: un único punto de entrada que aplica TODAS las
/// técnicas y devuelve un informe uniforme con la evidencia de cada paso.
///
/// Hace:
/// * **MBA**: simplificación de expresiones con prueba de equivalencia (BDD).
/// * **Control-flow flattening**: detección del dispatcher, reaplanado y
///   **prueba diferencial** de que el resultado se comporta igual.
/// * **Strings cifrados**: búsqueda de claves periódicas y extracción del
///   texto descifrado (con prueba de ida y vuelta).
/// * **Constantes ofuscadas**: patrón "cargar constante + XOR con clave" y
///   comprobación de que la constante real aparece en claro en el binario.
/// * **Predicados opacos**: a nivel de bloque (valor constante conocido) y a
///   nivel de expresión (forma canónica BDD).
pub fn deobfuscar(path: &Path, o: &OpcionesDeofuscacion) -> Result<Value, String> {
    let datos = std::fs::read(path).map_err(|e| format!("leer: {e}"))?;
    let id = identificar(path)?;
    let mut out = json!({
        "ok": true,
        "ruta": path.display().to_string(),
        "identificacion": serde_json::to_value(&id).unwrap_or(Value::Null),
        "tecnicas": [
            "mba (prueba BDD)",
            "control-flow flattening (prueba diferencial)",
            "strings cifrados (ida y vuelta)",
            "constantes ofuscadas (prueba en claro)",
            "predicados opacos (bloque + expresion)"
        ],
    });

    // --- cadenas cifradas (datos) -------------------------------------------
    let cifradas = crate::re_crypto::detectar_cifrado(&datos, o.min_len_cifrado, o.max_cadenas);
    out["cadenas_cifradas"] = json!({
        "total": cifradas.len(),
        "muestra": cifradas.iter().take(10).map(|c| json!({
            "offset": c.offset,
            "len": c.len,
            "clave": c.clave.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>(),
            "texto": c.texto,
            "prueba": format!("{:?}", c.prueba),
        })).collect::<Vec<_>>(),
    });

    // --- desensamblado -> CFG ----------------------------------------------
    if let Some((arch, off, size, vaddr)) = seccion_de_codigo(&datos) {
        let ventana = size.min(o.ventana_disasm);
        let code = &datos[off..off + ventana];
        out["codigo"] = json!({
            "arquitectura": arch,
            "seccion": ".text",
            "offset": off,
            "vaddr": vaddr,
            "bytes_analizados": ventana,
        });
        if let Some(cfg) = cfg_de_ventana(code, &arch, vaddr, ventana) {
            out["bloques"] = json!(cfg.blocks.len());

            // predicados opacos a nivel de bloque
            let opacos = crate::re_opaque::detectar_opacos(&cfg);
            out["predicados_opacos"] = json!({
                "total": opacos.len(),
                "muestra": opacos.iter().take(10).map(|o| json!({
                    "addr": o.addr,
                    "condicion": o.cc,
                    "destino": o.destino,
                    "valor": o.valor,
                    "veredicto": o.veredicto.label(),
                    "prueba": o.prueba,
                })).collect::<Vec<_>>(),
            });

            // constantes ofuscadas (cargar + XOR con clave)
            let constantes = crate::re_crypto::detectar_constantes_ofuscadas(&cfg, &datos);
            out["constantes_ofuscadas"] = json!({
                "total": constantes.len(),
                "muestra": constantes.iter().take(10).map(|c| json!({
                    "addr": c.addr,
                    "cifrada": format!("{:#x}", c.cifrada),
                    "clave": format!("{:#x}", c.clave),
                    "real": format!("{:#x}", c.real),
                    "prueba": c.prueba.as_ref().map(|p| format!("{p:?}")),
                })).collect::<Vec<_>>(),
            });

            // control-flow flattening
            match crate::re_ir::detectar_dispatcher_cfg(&cfg) {
                Some(disp) => {
                    let mut cff = json!({
                        "dispatcher": disp.addr,
                        "grado_entrada": disp.grado_entrada,
                        "variable_estado": disp.variable_estado,
                        "casos": disp.casos.len(),
                    });
                    if let Some(prog) = programa_desde_cfg(&cfg, &disp) {
                        let rep = crate::re_cff::analyze_cff(&prog, o.entradas_prueba);
                        cff["detectado"] = json!(rep.detected);
                        cff["cadena"] = json!(rep.cadena);
                        cff["prueba"] = match &rep.proof {
                            Some(p) => json!({
                                "passed": p.passed,
                                "entradas": p.inputs_tested,
                                "detalle": p.detail,
                                "diferencia": p.mismatch.as_ref().map(|m| json!({
                                    "motivo": m.motivo,
                                    "original": m.original.effects,
                                    "aplanado": m.aplanado.effects,
                                })),
                            }),
                            None => Value::Null,
                        };
                    }
                    out["control_flow_flattening"] = cff;
                }
                None => {
                    out["control_flow_flattening"] = json!({
                        "detectado": false,
                        "nota": "no hay dispatcher (bloque con grado de entrada >= 3) en la ventana analizada",
                    });
                }
            }
        }
    }

    Ok(out)
}

/// Simplifica una expresión MBA y devuelve el informe con su prueba.
pub fn mba(expr: &str) -> Value {
    match crate::re_mba::parse(expr) {
        Ok(e) => {
            let rep = crate::re_mba::simplify(&e);
            serde_json::to_value(rep).unwrap_or(Value::Null)
        }
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_motor_mba_devuelve_la_prueba() {
        let v = mba("(x ^ y) + 2*(x & y)");
        assert_eq!(v["changed"], json!(true));
        assert_eq!(v["simplified"], json!("(x + y)"));
        assert_eq!(v["proof"]["passed"], json!(true));
        assert_eq!(v["proof"]["kind"], json!("CanonicalForm"));
    }

    #[test]
    fn una_expresion_mba_invalida_no_revienta() {
        let v = mba("(x ^ ");
        assert_eq!(v["ok"], json!(false));
    }

    #[test]
    fn detecta_elf_y_go() {
        let p = Path::new("/workspace/agy-re/liblanguage_server.so");
        if !p.exists() {
            // Sin el artefacto real, el test no puede afirmar nada.
            return;
        }
        let id = identificar(p).expect("identificar");
        assert_eq!(id.formato, Formato::GoElf);
        assert!(id.bits == 64);
        assert!(id.funciones_go.unwrap_or(0) > 100_000);
    }

    #[test]
    fn el_magic_de_go_se_detecta_en_un_blob_sintetico() {
        // pcHeader de go1.18+: magic(4) pad1(1) pad2(1) minLC(1) ptrSize(1) nfunc(ptr)
        let mut d = vec![0u8; 64];
        d[16..20].copy_from_slice(&[0xf1, 0xff, 0xff, 0xff]);
        d[20] = 0; // pad1
        d[21] = 0; // pad2
        d[22] = 1; // minLC
        d[23] = 8; // ptrSize = 8 (64 bits)
        d[24..32].copy_from_slice(&1234u64.to_le_bytes());
        let (n, _) = detectar_go_pclntab(&d).expect("debe detectar");
        assert_eq!(n, 1234);
    }

    #[test]
    fn extrae_cadenas_con_offset() {
        // FIX 2026-11-22: la cadena de ejemplo era "fin" (3 bytes) con
        // min_len=4, asi que el motor la filtraba y devolvia 1 cadena, no 2.
        // El test estaba ROJO en main; ahora la segunda cadena tiene 4 bytes.
        let d = b"\x00\x01hola mundo\x00final\x00";
        let v = extraer_cadenas(d, 4, 10);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].1, "hola mundo");
        assert_eq!(v[0].0, 2);
        assert_eq!(v[1].1, "final");
        assert_eq!(v[1].0, 13);
    }

    #[test]
    fn el_catalogo_no_es_vacio_y_cuesta_poco() {
        let v = catalogo();
        let txt = serde_json::to_string(&v).unwrap();
        assert!(txt.len() < 4000, "catalogo demasiado grande: {}", txt.len());
        assert!(txt.contains("identificar"));
    }
}
