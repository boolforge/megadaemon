//! Oráculo de formatos: reconoce ficheros por *magic*, estructura y campos.
//!
//! Es la capa "de abajo" del motor de RE: no sabe nada de APKs ni de DEX,
//! sólo de **formatos de fichero**.  Aporta:
//!
//! * [`identify`] — magic + *carving* de magics embebidos (con offset real).
//! * [`parse_elf`] — parser propio de ELF (32/64, LE/BE): secciones, símbolos
//!   dinámicos, imports, exports, `DT_NEEDED`, ¿stripped?.
//! * [`extract_strings`] — cadenas ASCII/UTF-16 impresas.
//! * [`detect_swift_backup`] — la *info page* de Swift Backup (`SBA1`).
//!
//! Todo es **sólo lectura** y **sin panic**: cualquier fichero corrupto
//! devuelve un error o un resultado vacío, nunca revienta.
//!
//! ```
//! use megadaemon_core::format_oracle;
//! let m = format_oracle::identify(b"PK\x03\x04resto");
//! assert_eq!(m[0].name, "ZIP/APK/JAR");
//! ```

use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Un formato reconocido dentro de un fichero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatMatch {
    /// Nombre del formato (`ZIP/APK/JAR`, `ELF64`, `DEX`…).
    pub name: &'static str,
    /// Familia (`archive`, `executable`, `image`, `data`, `crypto`, `container`).
    pub kind: &'static str,
    /// MIME de referencia.
    pub mime: &'static str,
    /// Offset donde empieza la instancia.
    pub offset: usize,
    /// Confianza 0..=100.
    pub confidence: u8,
    /// Evidencia concreta (magic en hex, "string pool", "DT_NEEDED"…).
    pub evidence: String,
}

impl FormatMatch {
    /// Versión serializable para el informe JSON.
    pub fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "kind": self.kind,
            "mime": self.mime,
            "offset": self.offset,
            "offset_hex": format!("0x{:08X}", self.offset),
            "confidence": self.confidence,
            "evidence": self.evidence,
        })
    }
}

// ========================================================================= //
// Tabla de magics
// ========================================================================= //

/// Una entrada de la tabla de magics.
struct Magic {
    name: &'static str,
    kind: &'static str,
    mime: &'static str,
    bytes: &'static [u8],
    /// Desplazamiento dentro del fichero donde se busca el magic.
    at: usize,
    /// Si el magic va seguido de más bytes, se comparan también.
    tail: &'static [u8],
    confidence: u8,
}

const MAGIC_TABLE: &[Magic] = &[
    Magic { name: "ZIP/APK/JAR", kind: "archive", mime: "application/zip", bytes: b"PK\x03\x04", at: 0, tail: b"", confidence: 95 },
    Magic { name: "ZIP/APK/JAR (empty)", kind: "archive", mime: "application/zip", bytes: b"PK\x05\x06", at: 0, tail: b"", confidence: 95 },
    Magic { name: "DEX", kind: "executable", mime: "application/octet-stream", bytes: b"dex\n", at: 0, tail: b"", confidence: 90 },
    Magic { name: "ODEX/VDEX", kind: "executable", mime: "application/octet-stream", bytes: b"dey\n", at: 0, tail: b"", confidence: 80 },
    Magic { name: "ELF", kind: "executable", mime: "application/x-executable", bytes: b"\x7fELF", at: 0, tail: b"", confidence: 99 },
    Magic { name: "PE/MZ (Windows)", kind: "executable", mime: "application/x-dosexec", bytes: b"MZ", at: 0, tail: b"", confidence: 70 },
    Magic { name: "Mach-O 64", kind: "executable", mime: "application/x-mach-binary", bytes: b"\xcf\xfa\xed\xfe", at: 0, tail: b"", confidence: 99 },
    Magic { name: "Mach-O 32", kind: "executable", mime: "application/x-mach-binary", bytes: b"\xce\xfa\xed\xfe", at: 0, tail: b"", confidence: 99 },
    Magic { name: "Java class", kind: "executable", mime: "application/java-vm", bytes: b"\xca\xfe\xba\xbe", at: 0, tail: b"", confidence: 90 },
    Magic { name: "SQLite 3", kind: "data", mime: "application/vnd.sqlite3", bytes: b"SQLite format 3\x00", at: 0, tail: b"", confidence: 99 },
    Magic { name: "GZIP", kind: "archive", mime: "application/gzip", bytes: b"\x1f\x8b\x08", at: 0, tail: b"", confidence: 95 },
    Magic { name: "XZ", kind: "archive", mime: "application/x-xz", bytes: b"\xfd7zXZ\x00", at: 0, tail: b"", confidence: 99 },
    Magic { name: "BZIP2", kind: "archive", mime: "application/x-bzip2", bytes: b"BZh", at: 0, tail: b"", confidence: 90 },
    Magic { name: "ZSTD", kind: "archive", mime: "application/zstd", bytes: b"\x28\xb5\x2f\xfd", at: 0, tail: b"", confidence: 99 },
    Magic { name: "LZ4", kind: "archive", mime: "application/x-lz4", bytes: b"\x04\x22\x4d\x18", at: 0, tail: b"", confidence: 90 },
    Magic { name: "PNG", kind: "image", mime: "image/png", bytes: b"\x89PNG\r\n\x1a\n", at: 0, tail: b"", confidence: 99 },
    Magic { name: "JPEG", kind: "image", mime: "image/jpeg", bytes: b"\xff\xd8\xff", at: 0, tail: b"", confidence: 95 },
    Magic { name: "GIF", kind: "image", mime: "image/gif", bytes: b"GIF8", at: 0, tail: b"", confidence: 95 },
    Magic { name: "WEBP", kind: "image", mime: "image/webp", bytes: b"RIFF", at: 0, tail: b"WEBP", confidence: 95 },
    Magic { name: "PDF", kind: "data", mime: "application/pdf", bytes: b"%PDF-", at: 0, tail: b"", confidence: 95 },
    Magic { name: "Android ARSC", kind: "data", mime: "application/octet-stream", bytes: b"\x02\x00\x0c\x00", at: 0, tail: b"", confidence: 40 },
    Magic { name: "AXML (AndroidManifest)", kind: "data", mime: "application/octet-stream", bytes: b"\x03\x00\x08\x00", at: 0, tail: b"", confidence: 60 },
    Magic { name: "Swift Backup info page", kind: "container", mime: "application/octet-stream", bytes: b"SBA1", at: 0, tail: b"", confidence: 90 },
    Magic { name: "Swift Backup tar", kind: "container", mime: "application/octet-stream", bytes: b"SBA_SWIFTTAR", at: 0, tail: b"", confidence: 90 },
    Magic { name: "protobuf (wire)", kind: "data", mime: "application/octet-stream", bytes: b"\x0a\x00", at: 0, tail: b"", confidence: 20 },
    Magic { name: "CERT.RSA/PKCS#7", kind: "data", mime: "application/pkcs7-mime", bytes: b"\x30\x82", at: 0, tail: b"", confidence: 30 },
    Magic { name: "TAR (ustar)", kind: "archive", mime: "application/x-tar", bytes: b"ustar", at: 257, tail: b"", confidence: 90 },
    Magic { name: "OGG", kind: "data", mime: "audio/ogg", bytes: b"OggS", at: 0, tail: b"", confidence: 90 },
    Magic { name: "FLAC", kind: "data", mime: "audio/flac", bytes: b"fLaC", at: 0, tail: b"", confidence: 95 },
    Magic { name: "WAV/RIFF", kind: "data", mime: "audio/wav", bytes: b"RIFF", at: 0, tail: b"WAVE", confidence: 90 },
];

/// Tamaño máximo que se inspecciona al hacer *carving* de magics embebidos.
const CARVE_SCAN_LIMIT: usize = 8 * 1024 * 1024;

/// Distancia mínima entre dos coincidencias del mismo magic (anti-repetición).
const CARVE_MIN_GAP: usize = 4;

/// Reconoce el **formato principal** de un buffer (sólo offset 0).
///
/// Devuelve una lista vacía si nada coincide.
pub fn identify(data: &[u8]) -> Vec<FormatMatch> {
    let mut out = Vec::new();
    for m in MAGIC_TABLE {
        if m.at + m.bytes.len() + m.tail.len() > data.len() {
            continue;
        }
        let head = &data[m.at..m.at + m.bytes.len()];
        if head != m.bytes {
            continue;
        }
        if !m.tail.is_empty() {
            let t = &data[m.at + m.bytes.len()..m.at + m.bytes.len() + m.tail.len()];
            if t != m.tail {
                continue;
            }
        }
        out.push(FormatMatch {
            name: m.name,
            kind: m.kind,
            mime: m.mime,
            offset: m.at,
            confidence: m.confidence,
            evidence: hex(m.bytes),
        });
    }
    out.sort_by(|a, b| b.confidence.cmp(&a.confidence));
    out
}

/// Reconoce magics **embebidos** dentro de un fichero (carving).
///
/// Devuelve el formato principal (offset 0) más los magics encontrados a
/// partir de `min_offset`.  El escaneo está acotado a [`CARVE_SCAN_LIMIT`]
/// para no bloquearse con ficheros enormes.
pub fn carve(data: &[u8], min_offset: usize, max_results: usize) -> Vec<FormatMatch> {
    let mut out = identify(data);
    let limit = data.len().min(CARVE_SCAN_LIMIT);
    let mut last_hit: BTreeMap<&'static str, usize> = BTreeMap::new();
    for m in MAGIC_TABLE {
        // Los magics de offset 0 ya se han informado; y los de baja
        // confianza (protobuf, PKCS#7) generarían demasiado ruido.
        if m.confidence < 50 {
            continue;
        }
        let mut from = min_offset.max(m.at);
        while from + m.bytes.len() + m.tail.len() <= limit {
            let hay = &data[from..from + m.bytes.len()];
            if hay == m.bytes {
                let tail_ok = m.tail.is_empty()
                    || &data[from + m.bytes.len()..from + m.bytes.len() + m.tail.len()] == m.tail;
                if tail_ok {
                    let prev = last_hit.get(m.name).copied().unwrap_or(0);
                    if from >= prev + CARVE_MIN_GAP {
                        last_hit.insert(m.name, from);
                        out.push(FormatMatch {
                            name: m.name,
                            kind: m.kind,
                            mime: m.mime,
                            offset: from,
                            confidence: m.confidence - 10,
                            evidence: hex(m.bytes),
                        });
                    }
                    from += m.bytes.len();
                    continue;
                }
            }
            from += 1;
        }
    }
    out.sort_by(|a, b| a.offset.cmp(&b.offset));
    if out.len() > max_results {
        out.truncate(max_results);
    }
    out
}

/// Hexadecimal en minúsculas de una rebanada.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

// ========================================================================= //
// ELF
// ========================================================================= //

/// Una sección ELF.
#[derive(Debug, Clone, Default)]
pub struct ElfSection {
    /// Índice de la sección en la tabla.
    pub index: u16,
    /// Nombre (vacío si no se pudo resolver).
    pub name: String,
    /// Tipo de sección (2 = SYMTAB, 11 = DYNSYM, 3 = STRTAB…).
    pub sh_type: u32,
    /// Offset y tamaño en el fichero.
    pub offset: u64,
    pub size: u64,
    /// Tamaño de cada entrada (`sh_entsize`).
    pub entsize: u64,
}

/// Un símbolo ELF (de `.dynsym` o `.symtab`).
#[derive(Debug, Clone, Default)]
pub struct ElfSymbol {
    /// Nombre del símbolo.
    pub name: String,
    /// Tipo (2 = FUNC, 1 = OBJECT).
    pub sym_type: u8,
    /// Binding (0 = LOCAL, 1 = GLOBAL, 2 = WEAK).
    pub binding: u8,
    /// Índice de sección (`0` = indefinido ⇒ importado).
    pub shndx: u16,
    /// Tamaño del símbolo.
    pub size: u64,
}

/// Todo lo que se puede saber de un ELF sin herramientas externas.
#[derive(Debug, Clone, Default)]
pub struct ElfInfo {
    /// `ELF64` o `ELF32`.
    pub class: String,
    /// `little` o `big`.
    pub endian: String,
    /// Tipo de fichero (`ET_DYN`, `ET_EXEC`…).
    pub type_name: String,
    /// Arquitectura (`AArch64`, `ARM`, `x86_64`…).
    pub machine_name: String,
    /// `true` si no hay `.symtab` (sólo `.dynsym`).
    pub stripped: bool,
    /// Tabla de secciones.
    pub sections: Vec<ElfSection>,
    /// Símbolos exportados (definidos y globales).
    pub exported: Vec<String>,
    /// Símbolos importados (definidos en otro sitio).
    pub imported: Vec<String>,
    /// Bibliotecas compartidas necesarias (`DT_NEEDED`).
    pub needed: Vec<String>,
    /// `DT_SONAME`, si existe.
    pub soname: Option<String>,
    /// Símbolos exportados con tamaño (sólo `.dynsym`).
    pub exported_symbols: Vec<ElfSymbol>,
}

impl ElfInfo {
    /// Versión serializable (compacta: sin la tabla completa de secciones).
    pub fn to_json(&self) -> Value {
        json!({
            "class": self.class,
            "endian": self.endian,
            "type": self.type_name,
            "machine": self.machine_name,
            "stripped": self.stripped,
            "section_count": self.sections.len(),
            "exported_count": self.exported.len(),
            "imported_count": self.imported.len(),
            "exported": self.exported,
            "imported": self.imported,
            "needed": self.needed,
            "soname": self.soname,
        })
    }
}

/// Nombre legible de `e_type`.
fn elf_type_name(t: u16) -> &'static str {
    match t {
        0 => "ET_NONE",
        1 => "ET_REL",
        2 => "ET_EXEC",
        3 => "ET_DYN",
        4 => "ET_CORE",
        _ => "ET_UNKNOWN",
    }
}

/// Nombre legible de `e_machine`.
fn elf_machine_name(m: u16) -> &'static str {
    match m {
        0 => "None",
        3 => "x86",
        8 => "MIPS",
        20 => "PowerPC",
        21 => "PowerPC64",
        40 => "ARM",
        62 => "x86_64",
        183 => "AArch64",
        243 => "RISC-V",
        _ => "unknown",
    }
}

/// Lee un entero del tamaño indicado respetando el *endianness*.
fn read_uint(data: &[u8], off: usize, size: usize, little: bool) -> Option<u64> {
    let end = off.checked_add(size)?;
    if end > data.len() {
        return None;
    }
    let slice = &data[off..end];
    let mut v: u64 = 0;
    if little {
        for (i, &b) in slice.iter().enumerate() {
            v |= (b as u64) << (8 * i);
        }
    } else {
        for &b in slice.iter() {
            v = (v << 8) | b as u64;
        }
    }
    Some(v)
}

/// Parsea una cabecera ELF y su tabla de secciones/símbolos.
///
/// Devuelve `Err` si no es un ELF válido.  **Nunca hace panic.**
pub fn parse_elf(data: &[u8]) -> Result<ElfInfo, String> {
    if data.len() < 16 || &data[0..4] != b"\x7fELF" {
        return Err("no es un ELF (magic incorrecto)".to_string());
    }
    let class = match data[4] {
        1 => "ELF32",
        2 => "ELF64",
        _ => return Err(format!("clase ELF desconocida: {}", data[4])),
    };
    let little = match data[5] {
        1 => true,
        2 => false,
        _ => return Err(format!("endianness ELF desconocido: {}", data[5])),
    };
    let is64 = class == "ELF64";

    let e_type = read_uint(data, 16, 2, little).unwrap_or(0) as u16;
    let e_machine = read_uint(data, 18, 2, little).unwrap_or(0) as u16;

    let (e_phoff, e_shoff, e_phentsize, e_phnum, e_shentsize, e_shnum, e_shstrndx) = if is64 {
        (
            read_uint(data, 32, 8, little).unwrap_or(0),
            read_uint(data, 40, 8, little).unwrap_or(0),
            read_uint(data, 54, 2, little).unwrap_or(0),
            read_uint(data, 56, 2, little).unwrap_or(0),
            read_uint(data, 58, 2, little).unwrap_or(0),
            read_uint(data, 60, 2, little).unwrap_or(0),
            read_uint(data, 62, 2, little).unwrap_or(0),
        )
    } else {
        (
            read_uint(data, 28, 4, little).unwrap_or(0),
            read_uint(data, 32, 4, little).unwrap_or(0),
            read_uint(data, 42, 2, little).unwrap_or(0),
            read_uint(data, 44, 2, little).unwrap_or(0),
            read_uint(data, 46, 2, little).unwrap_or(0),
            read_uint(data, 48, 2, little).unwrap_or(0),
            read_uint(data, 50, 2, little).unwrap_or(0),
        )
    };

    let mut info = ElfInfo {
        class: class.to_string(),
        endian: if little { "little".into() } else { "big".into() },
        type_name: elf_type_name(e_type).to_string(),
        machine_name: elf_machine_name(e_machine).to_string(),
        ..Default::default()
    };

    // ------------------------------------------------------------------ //
    // Secciones
    // ------------------------------------------------------------------ //
    let mut raw_sections: Vec<(u32, u32, u64, u64, u64, u64)> = Vec::new(); // name,type,off,size,link,entsize
    for i in 0..e_shnum {
        let base = e_shoff as usize + (i * e_shentsize) as usize;
        let (name, sh_type, offset, size, link, entsize) = if is64 {
            (
                read_uint(data, base, 4, little).unwrap_or(0) as u32,
                read_uint(data, base + 4, 4, little).unwrap_or(0) as u32,
                read_uint(data, base + 24, 8, little).unwrap_or(0),
                read_uint(data, base + 32, 8, little).unwrap_or(0),
                read_uint(data, base + 40, 4, little).unwrap_or(0),
                read_uint(data, base + 56, 8, little).unwrap_or(0),
            )
        } else {
            (
                read_uint(data, base, 4, little).unwrap_or(0) as u32,
                read_uint(data, base + 4, 4, little).unwrap_or(0) as u32,
                read_uint(data, base + 16, 4, little).unwrap_or(0),
                read_uint(data, base + 20, 4, little).unwrap_or(0),
                read_uint(data, base + 24, 4, little).unwrap_or(0),
                read_uint(data, base + 36, 4, little).unwrap_or(0),
            )
        };
        raw_sections.push((name, sh_type, offset, size, link, entsize));
    }

    // Tabla de nombres de sección (.shstrtab).
    let mut shstr: &[u8] = &[];
    if let Some(idx) = raw_sections
        .iter()
        .position(|&(_, t, _, _, _, _)| t == 3 && e_shstrndx == 0)
    {
        // Con e_shstrndx == 0 no hay .shstrtab: se busca el primer STRTAB.
        let (_, _, off, size, _, _) = raw_sections[idx];
        shstr = slice_at(data, off, size);
    } else if e_shstrndx > 0 && (e_shstrndx as usize) < raw_sections.len() {
        let (_, _, off, size, _, _) = raw_sections[e_shstrndx as usize];
        shstr = slice_at(data, off, size);
    }

    for (i, &(name_off, sh_type, offset, size, _link, entsize)) in raw_sections.iter().enumerate() {
        info.sections.push(ElfSection {
            index: i as u16,
            name: cstr_at(shstr, name_off as usize),
            sh_type,
            offset,
            size,
            entsize,
        });
    }

    info.stripped = !info.sections.iter().any(|s| s.sh_type == 2 && s.name == ".symtab");

    // ------------------------------------------------------------------ //
    // Símbolos (.dynsym y .symtab)
    // ------------------------------------------------------------------ //
    for sec in info.sections.clone() {
        if sec.sh_type != 2 && sec.sh_type != 11 {
            continue;
        }
        let entsize = if sec.entsize > 0 {
            sec.entsize
        } else if is64 {
            24
        } else {
            16
        };
        if entsize == 0 {
            continue;
        }
        let count = sec.size / entsize;
        // La tabla de nombres es la sección(link).
        let strtab = info
            .sections
            .get(sec.index as usize)
            .map(|_| ())
            .and_then(|_| {
                raw_sections
                    .get(sec.index as usize)
                    .map(|&(_, _, off, size, link, _)| (off, size, link))
            });
        let Some((_, _, link_idx)) = strtab else { continue };
        let strdata = raw_sections
            .get(link_idx as usize)
            .map(|&(_, _, off, size, _, _)| slice_at(data, off, size))
            .unwrap_or(&[]);

        for i in 0..count {
            let base = sec.offset as usize + (i * entsize) as usize;
            let (name_off, info_byte, shndx, size) = if is64 {
                (
                    read_uint(data, base, 4, little).unwrap_or(0) as u32,
                    read_uint(data, base + 4, 1, little).unwrap_or(0) as u8,
                    read_uint(data, base + 6, 2, little).unwrap_or(0) as u16,
                    read_uint(data, base + 16, 8, little).unwrap_or(0),
                )
            } else {
                (
                    read_uint(data, base, 4, little).unwrap_or(0) as u32,
                    read_uint(data, base + 12, 1, little).unwrap_or(0) as u8,
                    read_uint(data, base + 14, 2, little).unwrap_or(0) as u16,
                    read_uint(data, base + 8, 4, little).unwrap_or(0),
                )
            };
            let name = cstr_at(strdata, name_off as usize);
            if name.is_empty() {
                continue;
            }
            let sym = ElfSymbol {
                name: name.clone(),
                sym_type: info_byte & 0x0f,
                binding: info_byte >> 4,
                shndx,
                size,
            };
            if sym.shndx == 0 {
                info.imported.push(name);
            } else if sym.binding == 1 || sym.binding == 2 {
                if sym.sym_type == 2 {
                    info.exported_symbols.push(sym);
                }
                if !info.exported.contains(&name) {
                    info.exported.push(name);
                }
            }
        }
    }

    // ------------------------------------------------------------------ //
    // Segmentos PT_LOAD (para traducir vaddr → offset)
    // ------------------------------------------------------------------ //
    let mut loads: Vec<(u64, u64, u64)> = Vec::new(); // vaddr, filesz, offset
    for i in 0..e_phnum {
        let base = e_phoff as usize + (i * e_phentsize) as usize;
        let (p_type, p_offset, p_vaddr, p_filesz) = if is64 {
            (
                read_uint(data, base, 4, little).unwrap_or(0),
                read_uint(data, base + 8, 8, little).unwrap_or(0),
                read_uint(data, base + 16, 8, little).unwrap_or(0),
                read_uint(data, base + 32, 8, little).unwrap_or(0),
            )
        } else {
            (
                read_uint(data, base, 4, little).unwrap_or(0),
                read_uint(data, base + 4, 4, little).unwrap_or(0),
                read_uint(data, base + 8, 4, little).unwrap_or(0),
                read_uint(data, base + 16, 4, little).unwrap_or(0),
            )
        };
        if p_type == 1 {
            loads.push((p_vaddr, p_filesz, p_offset));
        }
    }

    // ------------------------------------------------------------------ //
    // .dynamic → DT_NEEDED / DT_SONAME
    // ------------------------------------------------------------------ //
    if let Some(dyn_sec) = info.sections.iter().find(|s| s.sh_type == 6) {
        let dynsz = if is64 { 16 } else { 8 };
        let count = dyn_sec.size / dynsz;
        let mut dt_strtab = 0u64;
        let mut dt_strsz = 0u64;
        let mut needed_offsets: Vec<u64> = Vec::new();
        let mut soname_off: Option<u64> = None;
        for i in 0..count {
            let base = dyn_sec.offset as usize + (i * dynsz) as usize;
            let (tag, val) = if is64 {
                (
                    read_uint(data, base, 8, little).unwrap_or(0),
                    read_uint(data, base + 8, 8, little).unwrap_or(0),
                )
            } else {
                (
                    read_uint(data, base, 4, little).unwrap_or(0),
                    read_uint(data, base + 4, 4, little).unwrap_or(0),
                )
            };
            match tag {
                1 => needed_offsets.push(val),          // DT_NEEDED
                5 => dt_strtab = val,                   // DT_STRTAB
                10 => dt_strsz = val,                   // DT_STRSZ
                14 => soname_off = Some(val),           // DT_SONAME
                _ => {}
            }
        }
        let _ = dt_strsz;
        // vaddr → offset de fichero
        let vaddr_to_off = |v: u64| -> Option<u64> {
            loads
                .iter()
                .find(|&&(va, fsz, _)| v >= va && v < va + fsz)
                .map(|&(va, _, off)| off + (v - va))
        };
        if dt_strtab != 0 {
            if let Some(str_off) = vaddr_to_off(dt_strtab) {
                let strdata = slice_at(data, str_off, 0x4000);
                for &off in &needed_offsets {
                    let n = cstr_at(strdata, off as usize);
                    if !n.is_empty() {
                        info.needed.push(n);
                    }
                }
                if let Some(off) = soname_off {
                    let n = cstr_at(strdata, off as usize);
                    if !n.is_empty() {
                        info.soname = Some(n);
                    }
                }
            }
        }
    }

    Ok(info)
}

/// Rebanada segura: si `size` es 0 devuelve todo lo que haya desde `off`.
fn slice_at(data: &[u8], off: u64, size: u64) -> &[u8] {
    let start = off as usize;
    if start >= data.len() {
        return &[];
    }
    let end = if size == 0 {
        data.len()
    } else {
        start.saturating_add(size as usize).min(data.len())
    };
    &data[start..end]
}

/// Lee una cadena terminada en NUL.
fn cstr_at(data: &[u8], off: usize) -> String {
    if off >= data.len() {
        return String::new();
    }
    let end = data[off..]
        .iter()
        .position(|&b| b == 0)
        .map(|p| off + p)
        .unwrap_or(data.len());
    String::from_utf8_lossy(&data[off..end]).to_string()
}

// ========================================================================= //
// Cadenas
// ========================================================================= //

/// Extrae cadenas ASCII imprimibles de al menos `min_len` caracteres.
pub fn extract_strings(data: &[u8], min_len: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for &b in data {
        if (0x20..=0x7e).contains(&b) {
            cur.push(b as char);
        } else {
            if cur.len() >= min_len {
                out.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
        }
    }
    if cur.len() >= min_len {
        out.push(cur);
    }
    out
}

/// Extrae cadenas UTF-16LE (para `.rodata` de binarios Windows/Android).
pub fn extract_utf16_strings(data: &[u8], min_len: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut i = 0usize;
    while i + 1 < data.len() {
        let lo = data[i];
        let hi = data[i + 1];
        if hi == 0 && (0x20..=0x7e).contains(&lo) {
            cur.push(lo as char);
            i += 2;
        } else {
            if cur.chars().count() >= min_len {
                out.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
            i += 1;
        }
    }
    if cur.chars().count() >= min_len {
        out.push(cur);
    }
    out
}

// ========================================================================= //
// Swift Backup
// ========================================================================= //

/// Lo que se sabe de una *info page* de Swift Backup.
#[derive(Debug, Clone, Default)]
pub struct SwiftBackupInfo {
    /// `true` si el buffer empieza por el magic `SBA1`.
    pub has_magic: bool,
    /// Tamaño de página declarado en la cabecera (96 o 144).
    pub declared_page_size: u16,
    /// Flags de la cabecera.
    pub flags: u16,
    /// Tamaño declarado del *payload*.
    pub payload_size: u64,
    /// Reserva: campos todavía sin nombre confirmado.
    pub unknown_fields: Vec<(usize, u64, usize)>,
}

/// Analiza la *info page* de Swift Backup (128 B) si la hay.
///
/// El layout **no** está totalmente confirmado: lo que sí es seguro es la
/// posición del magic (`SBA1` en el byte 0) y de los campos de tamaño
///little-endian.  Lo que no se confirma se deja en `unknown_fields` en vez
/// de inventar un nombre — coherente con la regla de "nunca inventes".
pub fn detect_swift_backup(data: &[u8]) -> Option<SwiftBackupInfo> {
    if data.len() < 16 || &data[0..4] != b"SBA1" {
        return None;
    }
    let u16at = |o: usize| -> u16 {
        if o + 2 <= data.len() {
            u16::from_le_bytes([data[o], data[o + 1]])
        } else {
            0
        }
    };
    let u32at = |o: usize| -> u32 {
        if o + 4 <= data.len() {
            u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]])
        } else {
            0
        }
    };
    let u64at = |o: usize| -> u64 {
        if o + 8 <= data.len() {
            let mut b = [0u8; 8];
            b.copy_from_slice(&data[o..o + 8]);
            u64::from_le_bytes(b)
        } else {
            0
        }
    };
    let mut info = SwiftBackupInfo {
        has_magic: true,
        flags: u16at(4),
        declared_page_size: u16at(6),
        payload_size: u64at(12),
        ..Default::default()
    };
    // Los campos de 8 bytes a partir del byte 20 que no we've nombrado.
    let mut off = 20usize;
    while off + 8 <= data.len().min(128) {
        info.unknown_fields.push((off, u64at(off), 8));
        off += 8;
    }
    let _ = u32at;
    Some(info)
}

// ========================================================================= //
// AXML (AndroidManifest.xml binario)
// ========================================================================= //

/// Un filtro de intención.
#[derive(Debug, Clone, Default)]
pub struct AxmlFilter {
    /// Acciones declaradas.
    pub actions: Vec<String>,
    /// Categorías declaradas.
    pub categories: Vec<String>,
    /// `android:priority`, si se fijó.
    pub priority: Option<i64>,
}

/// Un componente declarado en el manifiesto.
#[derive(Debug, Clone, Default)]
pub struct AxmlComponent {
    /// `activity`, `activity-alias`, `service`, `receiver` o `provider`.
    pub kind: String,
    /// Nombre de la clase (fully qualified o relativo).
    pub name: String,
    /// `android:exported` explícito.
    pub exported: Option<bool>,
    /// `android:permission` (permiso que protege el componente).
    pub permission: Option<String>,
    /// `android:permissionGroup`.
    pub permission_group: Option<String>,
    /// `android:readPermission` (providers).
    pub read_permission: Option<String>,
    /// `android:writePermission` (providers).
    pub write_permission: Option<String>,
    /// `android:authorities` (providers).
    pub authorities: Option<String>,
    /// `android:enabled`.
    pub enabled: Option<bool>,
    /// Filtros de intención.
    pub intent_filters: Vec<AxmlFilter>,
}

impl AxmlComponent {
    /// `exported` **efectivo** (tal como lo calcula el sistema).
    ///
    /// Regla de Android: si `exported` no se declara, un componente con
    /// filtro de intención se exporta cuando `targetSdk < 31`; a partir de
    /// 31 el sistema **lanza** `InstallFailedManifestParserException` si no
    /// se declara.  Devuelve `false` cuando no hay permiso ni filtro.
    pub fn effective_exported(&self, target_sdk: Option<i64>) -> bool {
        if let Some(e) = self.exported {
            return e;
        }
        if self.intent_filters.is_empty() {
            return false;
        }
        // targetSdk >= 31 obliga a declararlo; si no está declarado, el
        // componente NO se exporta.
        match target_sdk {
            Some(t) if t >= 31 => false,
            _ => true,
        }
    }

    /// `true` si el componente está protegido por algún permiso.
    pub fn is_permission_protected(&self) -> bool {
        self.permission.is_some()
            || self.permission_group.is_some()
            || self.read_permission.is_some()
            || self.write_permission.is_some()
    }
}

/// Atributos de `<application>`.
#[derive(Debug, Clone, Default)]
pub struct AxmlApplication {
    /// Clase `Application` (puede ser una ofuscada).
    pub name: Option<String>,
    /// `android:debuggable`.
    pub debuggable: Option<bool>,
    /// `android:allowBackup`.
    pub allow_backup: Option<bool>,
    /// `android:usesCleartextTraffic`.
    pub uses_cleartext_traffic: Option<bool>,
    /// `android:networkSecurityConfig`.
    pub network_security_config: Option<String>,
    /// `android:testOnly`.
    pub test_only: Option<bool>,
    /// `android:extractNativeLibs`.
    pub extract_native_libs: Option<bool>,
    /// `android:hasCode`.
    pub has_code: Option<bool>,
    /// `android:largeHeap`.
    pub large_heap: Option<bool>,
}

/// Todo lo que se extrae de un `AndroidManifest.xml` binario.
#[derive(Debug, Clone, Default)]
pub struct AxmlManifest {
    /// `package` del manifiesto.
    pub package: Option<String>,
    /// `versionCode`.
    pub version_code: Option<i64>,
    /// `versionName`.
    pub version_name: Option<String>,
    /// `minSdkVersion`.
    pub min_sdk: Option<i64>,
    /// `targetSdkVersion`.
    pub target_sdk: Option<i64>,
    /// Permisos `uses-permission`.
    pub permissions: Vec<String>,
    /// Permisos con `maxSdkVersion` (menos relevantes).
    pub permissions_max_sdk: Vec<String>,
    /// Características `uses-feature`.
    pub features: Vec<String>,
    /// Componentes declarados.
    pub components: Vec<AxmlComponent>,
    /// Atributos de `<application>`.
    pub application: AxmlApplication,
    /// `meta-data` de primer nivel.
    pub meta_data: Vec<(String, String)>,
    /// Errores de parseo (no rompen el análisis).
    pub errors: Vec<String>,
}

// --- Lectores little-endian locales ------------------------------------- //

fn rd_u16(b: &[u8], o: usize) -> u16 {
    if o + 2 > b.len() {
        return 0;
    }
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn rd_u32(b: &[u8], o: usize) -> u32 {
    if o + 4 > b.len() {
        return 0;
    }
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// Tipo de valor de un atributo AXML.
enum AxVal {
    Str(String),
    Int(i64),
    Bool(bool),
}

impl AxVal {
    fn as_string(&self) -> Option<String> {
        match self {
            AxVal::Str(s) => Some(s.clone()),
            AxVal::Int(i) => Some(i.to_string()),
            AxVal::Bool(b) => Some(b.to_string()),
        }
    }
    fn as_int(&self) -> Option<i64> {
        match self {
            AxVal::Int(i) => Some(*i),
            AxVal::Bool(b) => Some(if *b { 1 } else { 0 }),
            AxVal::Str(s) => s.parse().ok(),
        }
    }
    fn as_bool(&self) -> Option<bool> {
        match self {
            AxVal::Bool(b) => Some(*b),
            AxVal::Int(i) => Some(*i != 0),
            AxVal::Str(s) => match s.as_str() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            },
        }
    }
}

/// Un atributo con su espacio de nombres.
struct AxAttr {
    ns: String,
    name: String,
    value: AxVal,
}

/// Lee el *string pool* de un AXML.
fn axml_string_pool(buf: &[u8], start: usize) -> Vec<String> {
    if start + 28 > buf.len() {
        return Vec::new();
    }
    let header_size = rd_u16(buf, start + 2) as usize;
    let string_count = rd_u32(buf, start + 8) as usize;
    let flags = rd_u32(buf, start + 16);
    let strings_start = rd_u32(buf, start + 20) as usize;
    let utf8 = flags & (1 << 8) != 0;
    let offsets_base = start + header_size;
    let data_base = start + strings_start;
    let mut out = Vec::with_capacity(string_count.min(65536));
    for i in 0..string_count {
        let off = rd_u32(buf, offsets_base + i * 4) as usize;
        out.push(axml_pool_string(buf, data_base + off, utf8));
    }
    out
}

/// Lee una cadena del pool (precedida de su longitud).
fn axml_pool_string(buf: &[u8], p: usize, utf8: bool) -> String {
    if p >= buf.len() {
        return String::new();
    }
    if utf8 {
        let first = buf[p];
        let (len, start) = if first & 0x80 != 0 {
            if p + 1 >= buf.len() {
                return String::new();
            }
            ((((first & 0x7f) as usize) << 8) | buf[p + 1] as usize, p + 2)
        } else {
            (first as usize, p + 1)
        };
        let end = (start + len).min(buf.len());
        String::from_utf8_lossy(&buf[start..end]).into_owned()
    } else {
        let first = rd_u16(buf, p);
        let (units, start) = if first & 0x8000 != 0 {
            let second = rd_u16(buf, p + 2);
            ((((first & 0x7fff) as usize) << 16) | second as usize, p + 4)
        } else {
            (first as usize, p + 2)
        };
        let end = (start + units * 2).min(buf.len());
        let mut codes: Vec<u16> = buf[start..end]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        while codes.last() == Some(&0) {
            codes.pop();
        }
        String::from_utf16_lossy(&codes)
    }
}

/// Convierte el `Res_value` de un atributo.
fn axml_attr_value(pool: &[String], v_type: u8, data: u32, raw: u32) -> AxVal {
    match v_type {
        0x03 => {
            let idx = if raw != u32::MAX && (raw as usize) < pool.len() {
                raw as usize
            } else {
                data as usize
            };
            AxVal::Str(pool.get(idx).cloned().unwrap_or_default())
        }
        0x10 | 0x11 => AxVal::Int(data as i32 as i64),
        0x12 => AxVal::Bool(data != 0),
        0x04 => AxVal::Int(f32::from_bits(data) as i64),
        _ => AxVal::Int(data as i64),
    }
}

/// Lee un elemento inicial con sus atributos.
fn axml_start_element(buf: &[u8], pos: usize, pool: &[String]) -> (String, Vec<AxAttr>) {
    let header_size = rd_u16(buf, pos + 2) as usize;
    let ext = pos + header_size;
    let name_idx = rd_u32(buf, ext + 4);
    let attr_start = rd_u16(buf, ext + 8) as usize;
    let attr_size = rd_u16(buf, ext + 10) as usize;
    let attr_count = rd_u16(buf, ext + 12) as usize;
    let base = ext + attr_start;
    let mut attrs = Vec::with_capacity(attr_count as usize);
    for i in 0..attr_count as usize {
        let a = base + i * attr_size;
        if a + 20 > buf.len() {
            break;
        }
        attrs.push(AxAttr {
            ns: pool.get(rd_u32(buf, a) as usize).cloned().unwrap_or_default(),
            name: pool.get(rd_u32(buf, a + 4) as usize).cloned().unwrap_or_default(),
            value: axml_attr_value(
                pool,
                buf[a + 15],
                rd_u32(buf, a + 16),
                rd_u32(buf, a + 8),
            ),
        });
    }
    (pool.get(name_idx as usize).cloned().unwrap_or_default(), attrs)
}

/// Busca un atributo por nombre (prefirando el namespace de Android).
fn axml_find<'a>(attrs: &'a [AxAttr], name: &str) -> Option<&'a AxVal> {
    attrs
        .iter()
        .find(|a| a.name == name && a.ns.contains("schemas.android.com"))
        .or_else(|| attrs.iter().find(|a| a.name == name))
        .map(|a| &a.value)
}

fn axml_str(attrs: &[AxAttr], name: &str) -> Option<String> {
    axml_find(attrs, name).and_then(|v| v.as_string())
}

fn axml_bool(attrs: &[AxAttr], name: &str) -> Option<bool> {
    axml_find(attrs, name).and_then(|v| v.as_bool())
}

fn axml_int(attrs: &[AxAttr], name: &str) -> Option<i64> {
    axml_find(attrs, name).and_then(|v| v.as_int())
}

/// Parsea un `AndroidManifest.xml` binario (AXML) con **todos** los atributos
/// que necesita una auditoría de seguridad.
///
/// A diferencia de un parser mínimo, éste conserva `permission`,
/// `permissionGroup`, `readPermission`, `writePermission`, `authorities`,
/// `enabled` y los atributos de `<application>` (`debuggable`, `allowBackup`,
/// `usesCleartextTraffic`…).  Devuelve un modelo con errores collecting: un
/// manifiesto corrupto devuelve lo que se pudo leer y una lista de errores.
pub fn parse_axml(buf: &[u8]) -> AxmlManifest {
    let mut m = AxmlManifest::default();
    if buf.len() < 8 || rd_u32(buf, 0) != 0x0008_0003 {
        m.errors
            .push("no es un AXML válido (magic incorrecto)".to_string());
        return m;
    }
    let total = (rd_u32(buf, 4) as usize).min(buf.len());
    let mut pool: Vec<String> = Vec::new();
    let mut pos = 8usize;
    let mut stack: Vec<String> = Vec::new();
    let mut cur_component: Option<usize> = None;
    let mut cur_filter: Option<AxmlFilter> = None;

    while pos + 8 <= total {
        let chunk_type = rd_u16(buf, pos);
        let chunk_size = rd_u32(buf, pos + 4) as usize;
        if chunk_size < 8 {
            break;
        }
        match chunk_type {
            0x0001 => pool = axml_string_pool(buf, pos),
            0x0102 => {
                let (name, attrs) = axml_start_element(buf, pos, &pool);
                match name.as_str() {
                    "manifest" => {
                        m.package = axml_str(&attrs, "package");
                        m.version_code = axml_int(&attrs, "versionCode");
                        m.version_name = axml_str(&attrs, "versionName");
                    }
                    "uses-sdk" => {
                        m.min_sdk = axml_int(&attrs, "minSdkVersion");
                        m.target_sdk = axml_int(&attrs, "targetSdkVersion");
                    }
                    "uses-permission" | "uses-permission-sdk-23" | "uses-permission-sdk-m" => {
                        if let Some(p) = axml_str(&attrs, "name") {
                            if p.is_empty() {
                                continue;
                            }
                            if axml_int(&attrs, "maxSdkVersion").is_some() {
                                m.permissions_max_sdk.push(p);
                            } else {
                                m.permissions.push(p);
                            }
                        }
                    }
                    "uses-feature" => {
                        if let Some(f) = axml_str(&attrs, "name") {
                            if !f.is_empty() {
                                m.features.push(f);
                            }
                        }
                    }
                    "application" => {
                        m.application.name = axml_str(&attrs, "name");
                        m.application.debuggable = axml_bool(&attrs, "debuggable");
                        m.application.allow_backup = axml_bool(&attrs, "allowBackup");
                        m.application.uses_cleartext_traffic =
                            axml_bool(&attrs, "usesCleartextTraffic");
                        m.application.network_security_config =
                            axml_str(&attrs, "networkSecurityConfig");
                        m.application.test_only = axml_bool(&attrs, "testOnly");
                        m.application.extract_native_libs = axml_bool(&attrs, "extractNativeLibs");
                        m.application.has_code = axml_bool(&attrs, "hasCode");
                        m.application.large_heap = axml_bool(&attrs, "largeHeap");
                    }
                    "meta-data" => {
                        if let (Some(n), Some(v)) = (axml_str(&attrs, "name"), axml_str(&attrs, "value"))
                        {
                            m.meta_data.push((n, v));
                        }
                    }
                    "activity" | "activity-alias" | "service" | "receiver" | "provider" => {
                        let kind = match name.as_str() {
                            "activity" | "activity-alias" => "activity",
                            "service" => "service",
                            "receiver" => "receiver",
                            _ => "provider",
                        };
                        let comp = AxmlComponent {
                            kind: kind.to_string(),
                            name: axml_str(&attrs, "name").unwrap_or_default(),
                            exported: axml_bool(&attrs, "exported"),
                            permission: axml_str(&attrs, "permission"),
                            permission_group: axml_str(&attrs, "permissionGroup"),
                            read_permission: axml_str(&attrs, "readPermission"),
                            write_permission: axml_str(&attrs, "writePermission"),
                            authorities: axml_str(&attrs, "authorities"),
                            enabled: axml_bool(&attrs, "enabled"),
                            intent_filters: Vec::new(),
                        };
                        m.components.push(comp);
                        cur_component = Some(m.components.len() - 1);
                    }
                    "intent-filter" => {
                        cur_filter = Some(AxmlFilter::default());
                    }
                    "action" => {
                        if let Some(a) = axml_str(&attrs, "name") {
                            if let Some(f) = cur_filter.as_mut() {
                                f.actions.push(a);
                            }
                        }
                    }
                    "category" => {
                        if let Some(a) = axml_str(&attrs, "name") {
                            if let Some(f) = cur_filter.as_mut() {
                                f.categories.push(a);
                            }
                        }
                    }
                    _ => {}
                }
                stack.push(name);
            }
            0x0103 => {
                let name = stack.pop().unwrap_or_default();
                if name == "intent-filter" {
                    if let (Some(f), Some(idx)) = (cur_filter.take(), cur_component) {
                        if let Some(c) = m.components.get_mut(idx) {
                            c.intent_filters.push(f);
                        }
                    }
                } else if matches!(
                    name.as_str(),
                    "activity" | "activity-alias" | "service" | "receiver" | "provider"
                ) {
                    cur_component = None;
                }
            }
            _ => {}
        }
        pos += chunk_size;
    }

    if m.package.is_none() {
        m.errors
            .push("el manifiesto no contiene un <manifest> legible".to_string());
    }
    m
}

// ========================================================================= //
// ========================================================================= //
// Utilidades de construccion de AXML para los tests (compartidas)
// ========================================================================= //

/// Utilidades compartidas entre los modulos de test.
pub mod testutil {
// ------------------------------------------------------------------ //
// Constructor de AXML para los tests
// ------------------------------------------------------------------ //

/// Construye un AXML con un pool UTF-8 y los elementos indicados.
pub fn build_axml(elements: &[(&str, Vec<(&str, bool, &str)>)]) -> Vec<u8> {
    // 1. Pool de cadenas: nombres de elementos + nombres y valores de attr.
    let mut pool: Vec<String> = Vec::new();
    let push = |s: &str, pool: &mut Vec<String>| -> u32 {
        if let Some(i) = pool.iter().position(|x| x == s) {
            return i as u32;
        }
        pool.push(s.to_string());
        (pool.len() - 1) as u32
    };
    for (tag, attrs) in elements {
        push(tag, &mut pool);
        for (an, _, av) in attrs.iter() {
            push(an, &mut pool);
            push(av, &mut pool);
        }
    }

    // 2. Pool chunk.
    let mut data = Vec::new();
    let mut offsets = Vec::new();
    for s in &pool {
        // OJO: el offset es RELATIVO al inicio del bloque de datos de
        // cadenas (`stringsStart`), no absoluto en el fichero.
        offsets.push(data.len() as u32);
        let b = s.as_bytes();
        if b.len() < 0x80 {
            data.push(b.len() as u8);
        } else {
            data.push(0x80 | (b.len() >> 8) as u8);
            data.push((b.len() & 0xff) as u8);
        }
        data.extend_from_slice(b);
        data.push(0);
    }
    let strings_start = 28 + pool.len() * 4;
    let mut sp = Vec::new();
    sp.extend_from_slice(&1u16.to_le_bytes()); // tipo
    sp.extend_from_slice(&28u16.to_le_bytes()); // headerSize
    sp.extend_from_slice(&((28 + pool.len() * 4 + data.len()) as u32).to_le_bytes());
    sp.extend_from_slice(&(pool.len() as u32).to_le_bytes());
    sp.extend_from_slice(&0u32.to_le_bytes()); // styleCount
    sp.extend_from_slice(&0x100u32.to_le_bytes()); // flags: UTF-8
    sp.extend_from_slice(&(strings_start as u32).to_le_bytes());
    sp.extend_from_slice(&0u32.to_le_bytes()); // stylesStart
    for o in &offsets {
        sp.extend_from_slice(&o.to_le_bytes());
    }
    sp.extend_from_slice(&data);

    // 3. Cabecera + elementos.
    let mut out = Vec::new();
    out.extend_from_slice(&0x0008_0003u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // tamaño total (parcheado al final)
    out.extend_from_slice(&sp);

    for (tag, attrs) in elements {
        // Índice real del nombre del tag en el pool.
        let tag_idx = pool.iter().position(|x| x == tag).unwrap_or(0) as u32;
        // Nodo (ResXMLTree_attrExt + ResXMLTree_node):
        //   line(4) comment(4) ns(4) name(4) attributeStart(2)
        //   attributeSize(2) attributeCount(2) idIndex(2) classIndex(2)
        //   styleIndex(2)  +  atributos (20 B cada uno)
        let mut body = Vec::new();
        body.extend_from_slice(&0u32.to_le_bytes()); // line
        body.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // comment
        body.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // ns
        body.extend_from_slice(&tag_idx.to_le_bytes()); // name
        body.extend_from_slice(&20u16.to_le_bytes()); // attributeStart
        body.extend_from_slice(&20u16.to_le_bytes()); // attributeSize
        body.extend_from_slice(&(attrs.len() as u16).to_le_bytes());
        body.extend_from_slice(&0u16.to_le_bytes()); // idIndex
        body.extend_from_slice(&0u16.to_le_bytes()); // classIndex
        body.extend_from_slice(&0u16.to_le_bytes()); // styleIndex
        for (an, is_bool, av) in attrs.iter() {
            let an_idx = pool.iter().position(|x| x == an).unwrap_or(0) as u32;
            let av_idx = pool.iter().position(|x| x == av).unwrap_or(0) as u32;
            body.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // ns
            body.extend_from_slice(&an_idx.to_le_bytes());
            body.extend_from_slice(&av_idx.to_le_bytes()); // rawValue
            body.extend_from_slice(&8u16.to_le_bytes()); // typedValue size
            body.push(0); // res0
            body.push(if *is_bool { 0x12 } else { 0x03 }); // dataType
            if *is_bool {
                body.extend_from_slice(&(if *av == "true" { 1u32 } else { 0u32 }).to_le_bytes());
            } else {
                body.extend_from_slice(&av_idx.to_le_bytes());
            }
        }
        // Chunk: type(2) + headerSize(2) + size(4) + cuerpo.
        let total = 8 + body.len();
        out.extend_from_slice(&0x0102u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes()); // headerSize
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&body);

        // End element: type(2) + headerSize(2) + size(4) + line(4) +
        //              comment(4) + ns(4) + name(4)  =  24 bytes
        out.extend_from_slice(&0x0103u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes()); // headerSize
        out.extend_from_slice(&24u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // line
        out.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // comment
        out.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // ns
        out.extend_from_slice(&tag_idx.to_le_bytes());
    }

    let total = out.len() as u32;
    out[4..8].copy_from_slice(&total.to_le_bytes());
    out
}

}

// Tests
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;
    use super::testutil::build_axml;

    #[test]
    fn identifica_zip_y_dex_por_magic() {
        let z = identify(b"PK\x03\x04resto");
        assert_eq!(z.len(), 1);
        assert_eq!(z[0].name, "ZIP/APK/JAR");
        assert_eq!(z[0].offset, 0);

        let d = identify(b"dex\n035\x00resto");
        assert_eq!(d[0].name, "DEX");

        assert!(identify(b"nada de nada").is_empty());
    }

    #[test]
    fn identifica_tar_por_ustar_en_257() {
        let mut buf = vec![0u8; 512];
        buf[257..262].copy_from_slice(b"ustar");
        let m = identify(&buf);
        assert!(m.iter().any(|x| x.name == "TAR (ustar)" && x.offset == 257));
    }

    #[test]
    fn carving_encuentra_elf_embebido() {
        let mut buf = vec![0u8; 4096];
        buf[1000..1004].copy_from_slice(b"\x7fELF");
        let m = carve(&buf, 0, 32);
        assert!(m.iter().any(|x| x.name == "ELF" && x.offset == 1000));
    }

    #[test]
    fn parse_elf_rechaza_no_elf_sin_entrar_en_panic() {
        assert!(parse_elf(b"no soy elf").is_err());
        assert!(parse_elf(&[]).is_err());
        assert!(parse_elf(b"\x7fELF\x09\x01").is_err()); // clase inválida
    }

    /// Construye un ELF64 mínimo con una sección `.dynsym` y dos símbolos.
    fn tiny_elf64() -> Vec<u8> {
        let mut d = vec![0u8; 0x400];
        d[0..4].copy_from_slice(b"\x7fELF");
        d[4] = 2; // 64-bit
        d[5] = 1; // little endian
        d[6] = 1;
        d[16..18].copy_from_slice(&3u16.to_le_bytes()); // ET_DYN
        d[18..20].copy_from_slice(&183u16.to_le_bytes()); // AArch64
        d[52..54].copy_from_slice(&64u16.to_le_bytes()); // e_ehsize
        d[54..56].copy_from_slice(&56u16.to_le_bytes()); // e_phentsize
        d[58..60].copy_from_slice(&64u16.to_le_bytes()); // e_shentsize
        d[60..62].copy_from_slice(&1u16.to_le_bytes()); // e_shnum

        // Una sección: tipo 11 (.dynsym) sin strings.
        let sh = 64usize;
        d[sh..sh + 4].copy_from_slice(&0u32.to_le_bytes()); // name
        d[sh + 4..sh + 8].copy_from_slice(&11u32.to_le_bytes()); // DYNSYM
        d[sh + 8..sh + 16].copy_from_slice(&0u64.to_le_bytes()); // flags
        d[sh + 16..sh + 24].copy_from_slice(&0u64.to_le_bytes()); // addr
        d[sh + 24..sh + 32].copy_from_slice(&0x100u64.to_le_bytes()); // offset
        d[sh + 32..sh + 40].copy_from_slice(&0u64.to_le_bytes()); // size
        d[sh + 40..sh + 44].copy_from_slice(&0u32.to_le_bytes()); // link
        d
    }

    #[test]
    fn parse_elf_64_lee_tipo_y_maquina() {
        let d = tiny_elf64();
        let e = parse_elf(&d).expect("debería parsear");
        assert_eq!(e.class, "ELF64");
        assert_eq!(e.endian, "little");
        assert_eq!(e.type_name, "ET_DYN");
        assert_eq!(e.machine_name, "AArch64");
        assert_eq!(e.sections.len(), 1);
    }

    #[test]
    fn extrae_cadenas_ascii_con_el_minimo_pedido() {
        let data = b"abc\x00\x01hello world\x00xyz";
        let s = extract_strings(data, 4);
        assert!(s.contains(&"hello world".to_string()));
        assert!(!s.contains(&"abc".to_string()));
        assert!(!s.contains(&"xyz".to_string()));
    }

    #[test]
    fn detecta_info_page_de_swift_backup() {
        let mut page = vec![0u8; 128];
        page[0..4].copy_from_slice(b"SBA1");
        page[4..6].copy_from_slice(&1u16.to_le_bytes());
        page[6..8].copy_from_slice(&96u16.to_le_bytes());
        page[12..20].copy_from_slice(&4096u64.to_le_bytes());
        let info = detect_swift_backup(&page).expect("debería detectar SBA1");
        assert!(info.has_magic);
        assert_eq!(info.declared_page_size, 96);
        assert_eq!(info.payload_size, 4096);
        assert!(detect_swift_backup(b"nope").is_none());
    }

    #[test]
    fn parse_axml_extrae_componentes_y_permisos() {
        let buf = build_axml(&[
            (
                "manifest",
                vec![("package", false, "com.demo.app"), ("versionCode", false, "42")],
            ),
            ("uses-sdk", vec![("minSdkVersion", false, "24"), ("targetSdkVersion", false, "33")]),
            ("uses-permission", vec![("name", false, "android.permission.INTERNET")]),
            (
                "activity",
                vec![
                    ("name", false, "com.demo.app.MainActivity"),
                    ("exported", true, "true"),
                ],
            ),
            (
                "service",
                vec![
                    ("name", false, "com.demo.app.Svc"),
                    ("permission", false, "android.permission.BIND_JOB_SERVICE"),
                ],
            ),
        ]);
        let m = parse_axml(&buf);
        assert_eq!(m.package.as_deref(), Some("com.demo.app"));
        assert_eq!(m.version_code, Some(42));
        assert_eq!(m.min_sdk, Some(24));
        assert_eq!(m.target_sdk, Some(33));
        assert_eq!(m.permissions, vec!["android.permission.INTERNET".to_string()]);
        assert_eq!(m.components.len(), 2);
        let act = &m.components[0];
        assert_eq!(act.kind, "activity");
        assert_eq!(act.exported, Some(true));
        assert!(act.effective_exported(Some(33)));
        assert!(!act.is_permission_protected());
        let svc = &m.components[1];
        assert!(svc.is_permission_protected());
        assert!(!svc.effective_exported(Some(33)));
    }

    #[test]
    fn parse_axml_invalido_no_entra_en_panic() {
        let m = parse_axml(&[0, 1, 2, 3]);
        assert!(m.package.is_none());
        assert!(!m.errors.is_empty());
        let m2 = parse_axml(&[]);
        assert!(m2.components.is_empty());
    }

    #[test]
    fn exported_efectivo_depende_del_target_sdk() {
        let mut c = AxmlComponent {
            kind: "activity".into(),
            name: "X".into(),
            ..Default::default()
        };
        // Sin `exported` y con filtro de intención: exportado hasta SDK 30.
        c.intent_filters.push(AxmlFilter::default());
        assert!(c.effective_exported(Some(30)));
        // A partir de 31 no se exporta si no se declara.
        assert!(!c.effective_exported(Some(31)));
        // Declarado explícitamente: manda lo declarado.
        c.exported = Some(true);
        assert!(c.effective_exported(Some(31)));
    }
}
