//! MegaDaemon v13.4 — Extracción de strings y símbolos del motor RE unificado.
//!
//! ## Por qué existe
//!
//! El repo ya tenía piezas sueltas: `go_retool` (símbolos Go desde el `pclntab`),
//! `apk_inventory` (inventario de APK), `ghidra` (importador y decompilador) y
//! `re_disasm` (desensamblado).  Lo que **no** había era una capa que los unifique:
//! el motor RE tenía que poder responder "¿qué es este fichero y qué tiene dentro?"
//! sin que el agente tuviera que saber qué módulo usar para cada caso.
//!
//! Este módulo es esa capa para lo más básico (strings y símbolos), y delega el
//! resto en los módulos que ya existen (ver `re_engine`).
//!
//! ## Strings
//!
//! * **ASCII**:.Printable (`0x20..=0x7e`) y `\\t`, de longitud mínima configurable.
//! * **UTF-16LE**: byte par bajo printable + byte alto a cero (el caso típico de
//!   Windows/PE y de los `resources.arsc` de Android).
//!
//! Ambos devuelven el **desplazamiento** del hallazgo, que es lo que permite
//! después cruzar un string con su código (el motor de deofuscación de strings
//! necesita saber dónde está el blob cifrado, no sólo su texto).

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Codificación en la que se encontró un string.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrEncoding {
    Ascii,
    Utf16Le,
}

/// Un string localizado dentro del fichero.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StringHit {
    /// Desplazamiento (offset) del primer byte del string.
    pub offset: u64,
    /// Longitud en bytes (no en caracteres).
    pub length: usize,
    pub encoding: StrEncoding,
    pub text: String,
}

/// Longitud mínima por defecto de un string interesting.
pub const DEFAULT_MIN_LEN: usize = 6;

/// Extrae los strings ASCII imprimibles.
///
/// `limit` acota el número de resultados (0 = sin límite).  Se devuelven **todos**
/// los hits aunque se pase el límite: el límite sólo corta la lista de salida.
pub fn extract_ascii(data: &[u8], min_len: usize, limit: usize) -> Vec<StringHit> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, &b) in data.iter().enumerate() {
        let printable = (0x20..=0x7e).contains(&b) || b == b'\t';
        match start {
            None => {
                if printable {
                    start = Some(i);
                }
            }
            Some(s) => {
                if !printable {
                    let len = i - s;
                    if len >= min_len {
                        out.push(StringHit {
                            offset: s as u64,
                            length: len,
                            encoding: StrEncoding::Ascii,
                            text: String::from_utf8_lossy(&data[s..i]).to_string(),
                        });
                    }
                    start = None;
                }
            }
        }
    }
    if let Some(s) = start {
        let len = data.len() - s;
        if len >= min_len {
            out.push(StringHit {
                offset: s as u64,
                length: len,
                encoding: StrEncoding::Ascii,
                text: String::from_utf8_lossy(&data[s..]).to_string(),
            });
        }
    }
    if limit > 0 && out.len() > limit {
        out.truncate(limit);
    }
    out
}

/// Extrae los strings UTF-16LE (byte alto a cero).
pub fn extract_utf16le(data: &[u8], min_len: usize, limit: usize) -> Vec<StringHit> {
    let mut out = Vec::new();
    let mut chars: Vec<u16> = Vec::new();
    let mut start: Option<usize> = None;
    let mut i = 0usize;
    while i + 1 < data.len() {
        let lo = data[i];
        let hi = data[i + 1];
        let ok = hi == 0 && ((0x20..=0x7e).contains(&lo) || lo == 0x09);
        if ok {
            if start.is_none() {
                start = Some(i);
            }
            chars.push(lo as u16);
            i += 2;
        } else {
            if let Some(s) = start {
                if chars.len() >= min_len {
                    out.push(StringHit {
                        offset: s as u64,
                        length: chars.len() * 2,
                        encoding: StrEncoding::Utf16Le,
                        text: String::from_utf16_lossy(&chars),
                    });
                }
            }
            start = None;
            chars.clear();
            i += 1;
        }
    }
    if let Some(s) = start {
        if chars.len() >= min_len {
            out.push(StringHit {
                offset: s as u64,
                length: chars.len() * 2,
                encoding: StrEncoding::Utf16Le,
                text: String::from_utf16_lossy(&chars),
            });
        }
    }
    if limit > 0 && out.len() > limit {
        out.truncate(limit);
    }
    out
}

/// Extrae strings en las dos codificaciones, ordenados por desplazamiento.
pub fn extract_strings(data: &[u8], min_len: usize, limit: usize) -> Vec<StringHit> {
    let mut out = extract_ascii(data, min_len, 0);
    out.extend(extract_utf16le(data, min_len, 0));
    out.sort_by_key(|h| h.offset);
    if limit > 0 && out.len() > limit {
        out.truncate(limit);
    }
    out
}

/// Clase de un símbolo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymKind {
    Function,
    Object,
    /// Simbolo importado (no definido aquí).
    Import,
    /// Simbolo exportado (visible desde fuera).
    Export,
    /// Simbolo dinámico (tabla `.dynsym`).
    Dynamic,
}

/// Un símbolo localizado.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SymbolHit {
    pub name: String,
    /// Dirección (virtual) del símbolo.
    pub address: u64,
    pub size: u64,
    pub kind: SymKind,
    /// De dónde salió: `elf.symtab`, `elf.dynsym`, `pe.exports`…
    pub source: String,
}

/// Extrae los símbolos de un ELF (`.symtab` + `.dynsym`).
pub fn elf_symbols(data: &[u8]) -> Vec<SymbolHit> {
    let elf = match goblin::elf::Elf::parse(data) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for s in elf.syms.iter() {
        let name = match elf.strtab.get_at(s.st_name) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if name.is_empty() {
            continue;
        }
        let kind = if s.is_function() {
            SymKind::Function
        } else if s.is_import() {
            SymKind::Import
        } else {
            SymKind::Object
        };
        out.push(SymbolHit {
            name,
            address: s.st_value,
            size: s.st_size,
            kind,
            source: "elf.symtab".to_string(),
        });
    }
    for s in elf.dynsyms.iter() {
        let name = match elf.dynstrtab.get_at(s.st_name) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if name.is_empty() {
            continue;
        }
        let kind = if s.is_function() {
            SymKind::Function
        } else if s.is_import() {
            SymKind::Import
        } else {
            SymKind::Dynamic
        };
        out.push(SymbolHit {
            name,
            address: s.st_value,
            size: s.st_size,
            kind,
            source: "elf.dynsym".to_string(),
        });
    }
    out
}

/// Extrae los símbolos exportados de un PE.
pub fn pe_symbols(data: &[u8]) -> Vec<SymbolHit> {
    let pe = match goblin::pe::PE::parse(data) {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for e in pe.exports.iter() {
        let name = match e.name {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => continue,
        };
        out.push(SymbolHit {
            name,
            address: e.rva as u64,
            size: e.size as u64,
            kind: SymKind::Export,
            source: "pe.exports".to_string(),
        });
    }
    for i in pe.imports.iter() {
        let lib = i.dll.to_string();
        let name = if i.name.is_empty() {
            format!("{lib}#{}", i.ordinal)
        } else {
            i.name.to_string()
        };
        out.push(SymbolHit {
            name,
            address: i.rva as u64,
            size: i.size as u64,
            kind: SymKind::Import,
            source: format!("pe.imports[{lib}]"),
        });
    }
    out
}

/// Extrae los símbolos que se puedan del fichero (ELF o PE).
pub fn collect_symbols(data: &[u8]) -> Vec<SymbolHit> {
    if data.len() >= 4 && &data[..4] == b"\x7fELF" {
        return elf_symbols(data);
    }
    if data.len() >= 2 && &data[..2] == b"MZ" {
        return pe_symbols(data);
    }
    Vec::new()
}

// ---------------------------------------------------------------- tests
//
// MEDIDO (2026-11-28): este módulo tenía 284 líneas y **CERO** tests (uno de
// los 45 módulos sin cobertura que enumera `docs/VERIFICACION_AGENTES.md`).
// Los extractores son puros (sobre un slice de bytes) => se testean OFFLINE.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extrae_un_string_ascii() {
        let datos = b"\x00\x01Hello, world!\x00";
        let hits = extract_ascii(datos, 6, 0);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].offset, 2, "el offset es del primer byte imprimible");
        assert_eq!(hits[0].length, 13);
        assert_eq!(hits[0].text, "Hello, world!");
        assert_eq!(hits[0].encoding, StrEncoding::Ascii);
    }

    #[test]
    fn respeta_la_longitud_minima() {
        let datos = b"\x00\x01Hello, world!\x00";
        assert_eq!(extract_ascii(datos, 20, 0).len(), 0, "13 < 20");
        assert_eq!(extract_ascii(datos, 13, 0).len(), 1, "13 >= 13");
    }

    #[test]
    fn el_limite_corta_la_lista() {
        let datos = b"primero\x00segundo\x00";
        let todos = extract_ascii(datos, 4, 0);
        assert_eq!(todos.len(), 2);
        assert_eq!(todos[0].offset, 0);
        assert_eq!(todos[1].offset, 8);
        assert_eq!(extract_ascii(datos, 4, 1).len(), 1, "el limite acota la salida");
    }

    #[test]
    fn un_string_al_final_sin_terminador_tambien_se_encuentra() {
        // El ultimo run no tiene byte que lo cierre: se emite igual.
        let datos = b"\x00abcdefgh";
        let hits = extract_ascii(datos, 6, 0);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].offset, 1);
        assert_eq!(hits[0].length, 8);
        assert_eq!(hits[0].text, "abcdefgh");
    }

    #[test]
    fn extrae_un_string_utf16le() {
        let datos = [0x41u8, 0x00, 0x42, 0x00, 0x43, 0x00];
        let hits = extract_utf16le(&datos, 2, 0);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].text, "ABC");
        assert_eq!(hits[0].length, 6, "la longitud es en BYTES, no en caracteres");
        assert_eq!(hits[0].encoding, StrEncoding::Utf16Le);
        assert_eq!(hits[0].offset, 0);
    }

    #[test]
    fn el_utf16_no_confunde_el_ascii() {
        // "A" en ASCII son 2 bytes: con min_len 2 no sale.
        let datos = [0x41u8, 0x00, 0x42, 0x00];
        assert_eq!(extract_ascii(&datos, 2, 0).len(), 0);
        assert_eq!(extract_utf16le(&datos, 2, 0).len(), 1);
    }

    #[test]
    fn extract_strings_une_las_dos_codificaciones_por_offset() {
        let mut datos = b"ASCIIuno".to_vec();
        datos.extend_from_slice(&[0x00, 0x00]);
        datos.extend_from_slice(&[0x55, 0x00, 0x54, 0x00]); // "UT" en UTF-16LE
        let hits = extract_strings(&datos, 2, 0);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].offset, 0);
        assert_eq!(hits[0].text, "ASCIIuno");
        assert_eq!(hits[0].encoding, StrEncoding::Ascii);
        assert_eq!(hits[1].offset, 10, "el UTF-16 empieza en su primer byte");
        assert_eq!(hits[1].text, "UT");
        assert_eq!(hits[1].encoding, StrEncoding::Utf16Le);
        assert!(hits[0].offset < hits[1].offset, "vienen ordenados por offset");
    }

    #[test]
    fn un_binario_vacio_no_produce_nada() {
        assert!(extract_ascii(&[], 6, 0).is_empty());
        assert!(extract_utf16le(&[], 6, 0).is_empty());
        assert!(extract_strings(&[], 6, 0).is_empty());
    }

    #[test]
    fn la_longitud_minima_por_defecto() {
        assert_eq!(DEFAULT_MIN_LEN, 6);
    }

    #[test]
    fn los_simbolos_de_un_fichero_que_no_es_elf_ni_pe_estan_vacios() {
        let basura = vec![0x41u8; 128];
        assert!(elf_symbols(&basura).is_empty());
        assert!(pe_symbols(&basura).is_empty());
        assert!(collect_symbols(&basura).is_empty());
    }

    #[test]
    fn collect_symbols_elige_por_la_magic() {
        // Con la magic ELF se pasa al parser de ELF (que fallara, pero no
        // con panic): lo que se comprueba es que NO se entra en panic.
        let mut elf = b"\x7fELF".to_vec();
        elf.extend_from_slice(&[0u8; 60]);
        let _ = collect_symbols(&elf);
        let mut mz = b"MZ".to_vec();
        mz.extend_from_slice(&[0u8; 60]);
        let _ = collect_symbols(&mz);
    }
}
