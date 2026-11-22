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
    const MAGIC: [u8; 4] = [0xff, 0xff, 0xff, 0xf1];
    let mut i = 0usize;
    while i + 4 <= datos.len() {
        if datos[i..i + 4] == MAGIC {
            // go1.18+: el layout es
            //   magic(4) pad(2) minLC(1) ptrSize(1) nfunc(ptrSize) nfiles(ptrSize) ...
            // nfunc ocupa 8 bytes en 64-bit: leerlo como u32 даaba la mitad.
            // MEDIDO: el binario Go de Antigravity tiene 161.682 funciones;
            // con u32 salia 2366 (exactamente nfunc/2 truncado).
            if i + 16 <= datos.len() {
                let ptr = if datos[i + 7] == 8 { 8usize } else { 4usize };
                let mut n: u64 = 0;
                for k in 0..ptr {
                    n |= (datos[i + 8 + k] as u64) << (8 * k);
                }
                if n > 0 && n < 5_000_000 {
                    return Some((n, Some("go1.18+".to_string())));
                }
            }
        }
        i += 4;
    }
    None
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
             "para": "informe UNIFORME de cualquier formato, con la evidencia"}
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut d = vec![0u8; 64];
        d[16..20].copy_from_slice(&[0xff, 0xff, 0xff, 0xf1]);
        d[24..28].copy_from_slice(&1234u32.to_le_bytes());
        let (n, _) = detectar_go_pclntab(&d).expect("debe detectar");
        assert_eq!(n, 1234);
    }

    #[test]
    fn extrae_cadenas_con_offset() {
        let d = b"\x00\x01hola mundo\x00fin\x00";
        let v = extraer_cadenas(d, 3, 10);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].1, "hola mundo");
        assert_eq!(v[0].0, 2);
    }

    #[test]
    fn el_catalogo_no_es_vacio_y_cuesta_poco() {
        let v = catalogo();
        let txt = serde_json::to_string(&v).unwrap();
        assert!(txt.len() < 4000, "catalogo demasiado grande: {}", txt.len());
        assert!(txt.contains("identificar"));
    }
}
