//! Motor genérico de **deofuscación y auditoría de APKs**.
//!
//! Pipeline completo: `ZIP → AndroidManifest.xml (AXML) → classes*.dex →
//! tabla de clases/métodos/campos → strings → grafo de llamadas (xrefs) →
//! informe JSON`.
//!
//! ## Por qué hay un lector de DEX propio
//!
//! El lector compartido (`dex_decompile.rs`) tiene **dos defectos medidos**
//! que lo hacen inservible sobre APK reales:
//!
//! 1. `read_u16` lee **big-endian**, cuando el formato DEX es little-endian
//!    ⇒ los índices de `field_ids`/`method_ids`, los `type_list`, los
//!    `code_item` y las unidades de instrucción salen **byte-swapeados**.
//! 2. El parser de `encoded_value` aborta con los tipos `VALUE_ARRAY` (0x0e),
//!    `VALUE_ENUM` (0x0d) y `VALUE_ANNOTATION` (0x0f) ⇒ falla al parsear el
//!    `classes.dex` de apps normales.
//!
//! Este módulo trae un lector **tolerante** (lo desconocido se salta, no
//! aborta) y **correcto** (little-endian, como manda la spec), para que el
//! escáner funcione sobre APK reales sin depender del estado del módulo
//! compartido.
//!
//! ## Alcance legal
//!
//! Esto es un **motor de análisis y auditoría**.  Analiza, documenta y
//! señala riesgos; **no** parchea, **no** descifraDRM, **no** bypasea
//! verificaciones de compra ni reimplementa la lógica de un protector.
//! Lo que sí hace es describir *qué* hace cada componente (clase → método →
//! efecto), que es exactamente lo que necesita un auditor.
//!
//! ## Uso
//!
//! ```no_run
//! let informe = megadaemon_core::apk_deobfuscator::scan("/ruta/app.apk");
//! assert!(informe["ok"] == true);
//! ```

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs::File;
use std::io::{Cursor, Read};

use crate::format_oracle;

// ========================================================================= //
// Límites (protegen contra APK patológicos y contra la RAM del móvil)
// ========================================================================= //

/// Máximo de entradas del ZIP que se listan en el informe.
const MAX_ENTRIES_REPORTED: usize = 400;
/// Máximo de clases analizadas por DEX.
const MAX_CLASSES: usize = 20_000;
/// Máximo de métodos desensamblados por DEX.
const MAX_METHODS: usize = 120_000;
/// Máximo de strings guardados por DEX.
const MAX_STRINGS: usize = 60_000;
/// Máximo de endpoints reportados.
const MAX_ENDPOINTS: usize = 300;
/// Máximo de símbolos nativos listados por `.so`.
const MAX_NATIVE_SYMBOLS: usize = 120;
/// Tamaño máximo de una `.so` que se analiza en memoria (8 MiB).
const MAX_NATIVE_SIZE: u64 = 8 * 1024 * 1024;

// ========================================================================= //
// Utilidades
// ========================================================================= //

/// SHA-256 en hexadecimal minúsculo.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    let d = h.finalize();
    d.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Lee un `u16` little-endian (el formato DEX es SIEMPRE little-endian).
fn rd_u16(b: &[u8], o: usize) -> Option<u16> {
    if o + 2 > b.len() {
        return None;
    }
    Some(u16::from_le_bytes([b[o], b[o + 1]]))
}

/// Lee un `u32` little-endian.
fn rd_u32(b: &[u8], o: usize) -> Option<u32> {
    if o + 4 > b.len() {
        return None;
    }
    Some(u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]))
}

/// Lee un `uleb128` (unsigned little-endian base-128).
fn read_uleb128(b: &[u8], o: &mut usize) -> Option<u32> {
    // Se acumula en u64: un uleb de 5 bytes desplazaría 7 bits x 4 = 28 y el
    // quinto byte necesita 35 bits (desbordaría un u32).
    let mut result: u64 = 0;
    let mut shift = 0u32;
    loop {
        if *o >= b.len() || shift > 28 {
            return None;
        }
        let byte = b[*o];
        *o += 1;
        result |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some(result as u32);
        }
        shift += 7;
    }
}

/// Decodifica una cadena MUTF-8 (la variante que usa DEX).
///
/// A diferencia de UTF-8, MUTF-8 codifica el NUL como `C0 80` y **no**
/// permite caracteres por encima de U+FFFF (usa pares surrogados).
fn decode_mutf8(bytes: &[u8]) -> String {
    let mut units: Vec<u16> = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b < 0x80 {
            units.push(b as u16);
            i += 1;
        } else if b & 0xe0 == 0xc0 {
            if i + 1 < bytes.len() {
                units.push((((b & 0x1f) as u16) << 6) | (bytes[i + 1] & 0x3f) as u16);
                i += 2;
            } else {
                i += 1;
            }
        } else if b & 0xf0 == 0xe0 {
            if i + 2 < bytes.len() {
                let c = (((b & 0x0f) as u16) << 12)
                    | (((bytes[i + 1] & 0x3f) as u16) << 6)
                    | (bytes[i + 2] & 0x3f) as u16;
                units.push(c);
                i += 3;
            } else {
                i += 1;
            }
        } else {
            // Byte suelto: se conserva como latin-1 para no perder info.
            units.push(b as u16);
            i += 1;
        }
    }
    // Reensambla pares surrogados inválidos como U+FFFD.
    let mut out = String::with_capacity(units.len());
    let mut i = 0usize;
    while i < units.len() {
        let u = units[i];
        if (0xd800..0xdc00).contains(&u) && i + 1 < units.len() {
            let lo = units[i + 1];
            if (0xdc00..0xe000).contains(&lo) {
                let cp = 0x10000u32 + (((u as u32) - 0xd800) << 10) + ((lo as u32) - 0xdc00);
                if let Some(c) = char::from_u32(cp) {
                    out.push(c);
                    i += 2;
                    continue;
                }
            }
            out.push('\u{fffd}');
            i += 1;
            continue;
        }
        match char::from_u32(u as u32) {
            Some(c) => out.push(c),
            None => out.push('\u{fffd}'),
        }
        i += 1;
    }
    out
}

// ========================================================================= //
// Lector de DEX
// ========================================================================= //

/// Un prototipo de método (`proto_id`).
#[derive(Debug, Clone, Default)]
pub struct DexProto {
    /// Descriptor corto (`V`, `Ljava/lang/String;`…).
    pub shorty: String,
    /// Tipo de retorno.
    pub return_type: String,
    /// Tipos de los parámetros, en orden.
    pub parameters: Vec<String>,
}

/// Una definición de campo (`field_id`).
#[derive(Debug, Clone, Default)]
pub struct DexFieldDef {
    /// Clase propietaria (descriptor).
    pub class_name: String,
    /// Tipo del campo (descriptor).
    pub field_type: String,
    /// Nombre del campo.
    pub name: String,
}

/// Una definición de método (`method_id`).
#[derive(Debug, Clone, Default)]
pub struct DexMethodDef {
    /// Clase propietaria (descriptor).
    pub class_name: String,
    /// Nombre del método.
    pub name: String,
    /// Prototipo (`(Ljava/lang/String;)V`).
    pub proto: String,
    /// Parámetros (descriptores).
    pub parameters: Vec<String>,
    /// Tipo de retorno.
    pub return_type: String,
}

/// Un bloque `try`/`catch`.
#[derive(Debug, Clone, Default)]
pub struct DexTry {
    /// Dirección inicial (unidades de instrucción).
    pub start_addr: u32,
    /// Número de unidades cubiertas.
    pub insn_count: u16,
    /// manejadores de excepción (offset en la lista de handlers).
    pub handler_off: u16,
}

/// Un método con cuerpo (`method` + `code_item`).
#[derive(Debug, Clone, Default)]
pub struct DexMethodBody {
    /// Índice en `method_ids`.
    pub method_idx: u32,
    /// Flags de acceso.
    pub access_flags: u32,
    /// Registros totales del frame.
    pub registers_size: u16,
    /// Registros de entrada.
    pub ins_size: u16,
    /// Registros de salida.
    pub outs_size: u16,
    /// Código en unidades de 16 bits (**little-endian**, como en el fichero).
    pub insns: Vec<u16>,
    /// Bloques try/catch.
    pub tries: Vec<DexTry>,
    /// ¿El método es `static`?
    pub is_static: bool,
}

/// Una clase (`class_def_item`).
#[derive(Debug, Clone, Default)]
pub struct DexClassDef {
    /// Descriptor de la clase.
    pub class_name: String,
    /// Descriptor de la superclase.
    pub superclass: String,
    /// Interfaces implementadas.
    pub interfaces: Vec<String>,
    /// Flags de acceso.
    pub access_flags: u32,
    /// Fichero fuente (si se conservó).
    pub source_file: String,
    /// Campos estáticos: (índice en `field_ids`, flags).
    pub static_fields: Vec<(u32, u32)>,
    /// Campos de instancia: (índice en `field_ids`, flags).
    pub instance_fields: Vec<(u32, u32)>,
    /// Métodos con cuerpo.
    pub methods: Vec<DexMethodBody>,
}

/// Un DEX completo ya parseado.
#[derive(Debug, Clone, Default)]
pub struct DexFile {
    /// Versión del formato (`035`…`041`).
    pub version: String,
    /// Tabla de strings.
    pub strings: Vec<String>,
    /// Tabla de tipos (descriptores).
    pub types: Vec<String>,
    /// Tabla de prototipos.
    pub protos: Vec<DexProto>,
    /// Tabla de campos.
    pub fields: Vec<DexFieldDef>,
    /// Tabla de métodos.
    pub methods: Vec<DexMethodDef>,
    /// Tabla de clases.
    pub classes: Vec<DexClassDef>,
    /// Flags de acceso por índice de método (`method_idx` → flags).
    ///
    /// Hace falta para saber si un método llamado es `static` (y, por tanto,
    /// si el primer registro de la invocación es `this` o el primer
    /// parámetro).
    pub method_flags: HashMap<u32, u32>,
    /// SHA-256 del fichero.
    pub sha256: String,
    /// Errores no fatales encontrados al parsear.
    pub warnings: Vec<String>,
}

/// Convierte un descriptor DEX a forma Java legible.
pub fn descriptor_to_java(desc: &str) -> String {
    if desc.is_empty() {
        return String::new();
    }
    match desc.as_bytes()[0] {
        b'V' => "void".to_string(),
        b'Z' => "boolean".to_string(),
        b'B' => "byte".to_string(),
        b'S' => "short".to_string(),
        b'C' => "char".to_string(),
        b'I' => "int".to_string(),
        b'J' => "long".to_string(),
        b'F' => "float".to_string(),
        b'D' => "double".to_string(),
        b'[' => format!("{}[]", descriptor_to_java(&desc[1..])),
        b'L' => {
            let inner = desc.trim_start_matches('L');
            let inner = inner.trim_end_matches(';');
            inner.replace('/', ".").replace('$', ".")
        }
        _ => desc.to_string(),
    }
}

impl DexFile {
    /// Parsea un DEX desde sus bytes.  **Tolerante**: lo que no entiende se
    /// salta y se anota en `warnings`; sólo aborta si el fichero no es un DEX.
    pub fn parse(bytes: &[u8]) -> Result<DexFile, String> {
        if bytes.len() < 112 {
            return Err(format!(
                "fichero demasiado corto para ser un DEX: {} bytes",
                bytes.len()
            ));
        }
        if &bytes[0..4] != b"dex\n" {
            return Err(format!(
                "magic inválido: {:?} (se esperaba \"dex\\n\")",
                String::from_utf8_lossy(&bytes[0..4])
            ));
        }
        let version = String::from_utf8_lossy(&bytes[4..7]).to_string();
        let mut dex = DexFile {
            version: version.clone(),
            sha256: sha256_hex(bytes),
            ..Default::default()
        };

        let file_size = rd_u32(bytes, 56).unwrap_or(0) as usize;
        let header_size = rd_u32(bytes, 36).unwrap_or(0) as usize;
        if header_size < 112 {
            return Err(format!("header_size inválido: {}", header_size));
        }

        // Cabecera DEX (spec): los ids empiezan en 0x38 y van de 4 en 4.
        //   0x38 string_ids  0x40 type_ids  0x48 proto_ids
        //   0x50 field_ids   0x58 method_ids 0x60 class_defs
        let string_ids_size = rd_u32(bytes, 0x38).unwrap_or(0) as usize;
        let string_ids_off = rd_u32(bytes, 0x3c).unwrap_or(0) as usize;
        let type_ids_size = rd_u32(bytes, 0x40).unwrap_or(0) as usize;
        let type_ids_off = rd_u32(bytes, 0x44).unwrap_or(0) as usize;
        let proto_ids_size = rd_u32(bytes, 0x48).unwrap_or(0) as usize;
        let proto_ids_off = rd_u32(bytes, 0x4c).unwrap_or(0) as usize;
        let field_ids_size = rd_u32(bytes, 0x50).unwrap_or(0) as usize;
        let field_ids_off = rd_u32(bytes, 0x54).unwrap_or(0) as usize;
        let method_ids_size = rd_u32(bytes, 0x58).unwrap_or(0) as usize;
        let method_ids_off = rd_u32(bytes, 0x5c).unwrap_or(0) as usize;
        let class_defs_size = rd_u32(bytes, 0x60).unwrap_or(0) as usize;
        let class_defs_off = rd_u32(bytes, 0x64).unwrap_or(0) as usize;

        let _ = file_size;

        // -------------------------------------------------------------- //
        // 1. Strings
        // -------------------------------------------------------------- //
        for i in 0..string_ids_size.min(MAX_STRINGS) {
            let Some(off) = rd_u32(bytes, string_ids_off + i * 4) else {
                break;
            };
            let off = off as usize;
            if off >= bytes.len() {
                continue;
            }
            let mut o = off;
            // El tamaño (uleb128) no hace falta para decodificar, pero sí
            // para validar: si no se puede leer, el string_data_item está
            // corrupto y se salta.
            if read_uleb128(bytes, &mut o).is_none() || o >= bytes.len() {
                continue;
            }
            let start = o;
            let mut end = start;
            // La cadena termina en NUL: se acota el búsqueda para que un
            // `string_data_item` corrupto no se coma el resto del fichero.
            let limit = (start + 64 * 1024).min(bytes.len());
            while end < limit && bytes[end] != 0 {
                end += 1;
            }
            dex.strings.push(decode_mutf8(&bytes[start..end]));
        }

        // -------------------------------------------------------------- //
        // 2. Tipos
        // -------------------------------------------------------------- //
        for i in 0..type_ids_size {
            let Some(idx) = rd_u32(bytes, type_ids_off + i * 4) else {
                break;
            };
            dex.types.push(
                dex.strings
                    .get(idx as usize)
                    .cloned()
                    .unwrap_or_else(|| format!("<tipo#{}>", idx)),
            );
        }

        // -------------------------------------------------------------- //
        // 3. Prototipos
        // -------------------------------------------------------------- //
        for i in 0..proto_ids_size {
            let base = proto_ids_off + i * 12;
            let shorty_idx = rd_u32(bytes, base).unwrap_or(0);
            let ret_idx = rd_u32(bytes, base + 4).unwrap_or(0);
            let params_off = rd_u32(bytes, base + 8).unwrap_or(0) as usize;
            let mut params: Vec<String> = Vec::new();
            if params_off != 0 && params_off + 4 <= bytes.len() {
                let size = rd_u32(bytes, params_off).unwrap_or(0) as usize;
                for p in 0..size.min(64) {
                    let Some(tidx) = rd_u16(bytes, params_off + 4 + p * 2) else {
                        break;
                    };
                    params.push(
                        dex.types
                            .get(tidx as usize)
                            .cloned()
                            .unwrap_or_else(|| "<tipo?>".to_string()),
                    );
                }
            }
            dex.protos.push(DexProto {
                shorty: dex
                    .strings
                    .get(shorty_idx as usize)
                    .cloned()
                    .unwrap_or_default(),
                return_type: dex
                    .types
                    .get(ret_idx as usize)
                    .cloned()
                    .unwrap_or_else(|| "V".to_string()),
                parameters: params,
            });
        }

        // -------------------------------------------------------------- //
        // 4. Campos
        // -------------------------------------------------------------- //
        for i in 0..field_ids_size {
            let base = field_ids_off + i * 8;
            let class_idx = rd_u16(bytes, base).unwrap_or(0) as usize;
            let type_idx = rd_u16(bytes, base + 2).unwrap_or(0) as usize;
            let name_idx = rd_u32(bytes, base + 4).unwrap_or(0) as usize;
            dex.fields.push(DexFieldDef {
                class_name: dex.types.get(class_idx).cloned().unwrap_or_default(),
                field_type: dex.types.get(type_idx).cloned().unwrap_or_default(),
                name: dex.strings.get(name_idx).cloned().unwrap_or_default(),
            });
        }

        // -------------------------------------------------------------- //
        // 5. Métodos
        // -------------------------------------------------------------- //
        for i in 0..method_ids_size {
            let base = method_ids_off + i * 8;
            let class_idx = rd_u16(bytes, base).unwrap_or(0) as usize;
            let proto_idx = rd_u16(bytes, base + 2).unwrap_or(0) as usize;
            let name_idx = rd_u32(bytes, base + 4).unwrap_or(0) as usize;
            let proto = dex.protos.get(proto_idx).cloned().unwrap_or_default();
            let proto_str = format!(
                "({}){}",
                proto.parameters.join(""),
                proto.return_type
            );
            dex.methods.push(DexMethodDef {
                class_name: dex.types.get(class_idx).cloned().unwrap_or_default(),
                name: dex.strings.get(name_idx).cloned().unwrap_or_default(),
                proto: proto_str,
                parameters: proto.parameters,
                return_type: proto.return_type,
            });
        }

        // -------------------------------------------------------------- //
        // 6. Clases
        // -------------------------------------------------------------- //
        for i in 0..class_defs_size.min(MAX_CLASSES) {
            let base = class_defs_off + i * 32;
            let class_idx = rd_u32(bytes, base).unwrap_or(0) as usize;
            let access_flags = rd_u32(bytes, base + 4).unwrap_or(0);
            let superclass_idx = rd_u32(bytes, base + 8).unwrap_or(0) as usize;
            let interfaces_off = rd_u32(bytes, base + 12).unwrap_or(0) as usize;
            let source_file_idx = rd_u32(bytes, base + 16).unwrap_or(0) as usize;
            let class_data_off = rd_u32(bytes, base + 24).unwrap_or(0) as usize;

            let mut interfaces = Vec::new();
            if interfaces_off != 0 && interfaces_off + 4 <= bytes.len() {
                let size = rd_u32(bytes, interfaces_off).unwrap_or(0) as usize;
                for p in 0..size.min(64) {
                    let Some(tidx) = rd_u16(bytes, interfaces_off + 4 + p * 2) else {
                        break;
                    };
                    if let Some(t) = dex.types.get(tidx as usize) {
                        interfaces.push(t.clone());
                    }
                }
            }

            let mut class = DexClassDef {
                class_name: dex.types.get(class_idx).cloned().unwrap_or_default(),
                superclass: dex
                    .types
                    .get(superclass_idx)
                    .cloned()
                    .unwrap_or_else(|| "Ljava/lang/Object;".to_string()),
                interfaces,
                access_flags,
                source_file: dex.strings.get(source_file_idx as usize).cloned().unwrap_or_default(),
                ..Default::default()
            };

            if class_data_off != 0 {
                read_class_data(bytes, class_data_off, &mut class, &mut dex.method_flags);
            }
            dex.classes.push(class);
        }

        if dex.strings.is_empty() {
            dex.warnings
                .push("el pool de strings salió vacío: el DEX puede estar truncado".to_string());
        }
        Ok(dex)
    }
}

/// Lee un `class_data_item` y mete los métodos (con su `code_item`) en la clase.
fn read_class_data(
    bytes: &[u8],
    off: usize,
    class: &mut DexClassDef,
    method_flags: &mut HashMap<u32, u32>,
) {
    let mut o = off;
    let Some(sf) = read_uleb128(bytes, &mut o) else {
        return;
    };
    let Some(inf) = read_uleb128(bytes, &mut o) else {
        return;
    };
    let Some(dm) = read_uleb128(bytes, &mut o) else {
        return;
    };
    let Some(vm) = read_uleb128(bytes, &mut o) else {
        return;
    };

    // OJO (spec DEX): cada lista (`static_fields`, `instance_fields`,
    // `direct_methods`, `virtual_methods`) tiene su **propio** indice
    // relativo: el `*_idx_diff` es la diferencia respecto al elemento
    // ANTERIOR **de esa misma lista**, y el primero va directo.
    // Continuar el indice de los metodos directos en los virtuales
    // (error clasico) atribuye cada metodo a la clase equivocada.
    let mut idx = 0u32;
    for _ in 0..sf {
        let Some(d) = read_uleb128(bytes, &mut o) else {
            return;
        };
        idx += d;
        let Some(flags) = read_uleb128(bytes, &mut o) else {
            return;
        };
        class.static_fields.push((idx, flags));
    }
    idx = 0;
    for _ in 0..inf {
        let Some(d) = read_uleb128(bytes, &mut o) else {
            return;
        };
        idx += d;
        let Some(flags) = read_uleb128(bytes, &mut o) else {
            return;
        };
        class.instance_fields.push((idx, flags));
    }

    for (list, total) in [(dm, true), (vm, false)] {
        idx = 0;
        for _ in 0..list {
            let Some(d) = read_uleb128(bytes, &mut o) else {
                return;
            };
            idx += d;
            let Some(flags) = read_uleb128(bytes, &mut o) else {
                return;
            };
            let Some(code_off) = read_uleb128(bytes, &mut o) else {
                return;
            };
            if code_off == 0 {
                continue;
            }
            if class.methods.len() >= MAX_METHODS {
                return;
            }
            let mut body = DexMethodBody {
                method_idx: idx,
                access_flags: flags,
                is_static: flags & 0x0008 != 0,
                ..Default::default()
            };
            method_flags.insert(idx, flags);
            if let Some(b) = read_code_item(bytes, code_off as usize) {
                body.registers_size = b.0;
                body.ins_size = b.1;
                body.outs_size = b.2;
                body.insns = b.3;
                body.tries = b.4;
            }
            class.methods.push(body);
        }
        let _ = total;
    }
}

/// Lee un `code_item` → (registros, ins, outs, insns, tries).
fn read_code_item(
    bytes: &[u8],
    off: usize,
) -> Option<(u16, u16, u16, Vec<u16>, Vec<DexTry>)> {
    let registers_size = rd_u16(bytes, off)?;
    let ins_size = rd_u16(bytes, off + 2)?;
    let outs_size = rd_u16(bytes, off + 4)?;
    let tries_size = rd_u16(bytes, off + 6)?;
    let insns_size = rd_u32(bytes, off + 12)? as usize;
    let insns_start = off + 16;
    // Tope duro + comprobacion de que las unidades caben en el fichero.
    if insns_size > 2_000_000 || insns_start + insns_size * 2 > bytes.len() + 2 {
        return None;
    }
    let mut insns = Vec::with_capacity(insns_size);
    for i in 0..insns_size {
        insns.push(rd_u16(bytes, insns_start + i * 2)?);
    }
    // Los `try_item` van después, alineados a 4 bytes si `insns_size` es impar.
    let mut tries = Vec::new();
    if tries_size > 0 {
        let mut t = insns_start + insns_size * 2;
        if insns_size % 2 == 1 {
            t += 2;
        }
        for _ in 0..tries_size {
            let start_addr = rd_u32(bytes, t)?;
            let insn_count = rd_u16(bytes, t + 4)?;
            let handler_off = rd_u16(bytes, t + 6)?;
            tries.push(DexTry {
                start_addr,
                insn_count,
                handler_off,
            });
            t += 8;
        }
    }
    Some((registers_size, ins_size, outs_size, insns, tries))
}

// ========================================================================= //
// Desensamblador (Dalvik, little-endian, spec-compliant)
// ========================================================================= //

/// Una instrucción Dalvik decodificada **con sus referencias resueltas**.
#[derive(Debug, Clone, Default)]
pub struct Insn {
    /// Offset en unidades de 16 bits dentro del método.
    pub offset: u32,
    /// Byte del opcode.
    pub opcode: u8,
    /// Mnemónico.
    pub mnemonic: &'static str,
    /// Formato de la instrucción (`35c`, `21c`…).
    pub format: &'static str,
    /// Índice en `string_ids` (const-string).
    pub string_idx: Option<u32>,
    /// Índice en `type_ids`.
    pub type_idx: Option<u32>,
    /// Índice en `field_ids`.
    pub field_idx: Option<u32>,
    /// Índice en `method_ids`.
    pub method_idx: Option<u32>,
    /// Registros de argumento (para `invoke-*`).
    pub regs: Vec<u8>,
    /// Registro destino (para `const-*`, `move-*`, `aget`…).
    pub dst: Option<u8>,
    /// Registro origen (para `move*`).
    pub src: Option<u8>,
    /// Constante entera si la instrucción la define.
    pub int_const: Option<i64>,
    /// Constante de texto si la instrucción la define.
    pub str_const: Option<String>,
}

/// ¿Es una instrucción de invocación?
pub fn is_invoke(opcode: u8) -> bool {
    matches!(opcode, 0x6e..=0x72 | 0x74..=0x78 | 0xfa..=0xfd)
}

/// Mnemónico y formato de un opcode (tabla de la spec Dalvik).
pub fn opcode_info(opcode: u8) -> (&'static str, &'static str) {
    match opcode {
        0x00 => ("nop", "10x"),
        0x01 => ("move", "12x"),
        0x02 => ("move/from16", "22x"),
        0x03 => ("move/16", "32x"),
        0x04 => ("move-wide", "12x"),
        0x05 => ("move-wide/from16", "22x"),
        0x06 => ("move-wide/16", "32x"),
        0x07 => ("move-object", "12x"),
        0x08 => ("move-object/from16", "22x"),
        0x09 => ("move-object/16", "32x"),
        0x0a => ("move-result", "11x"),
        0x0b => ("move-result-wide", "11x"),
        0x0c => ("move-result-object", "11x"),
        0x0d => ("move-exception", "11x"),
        0x0e => ("return-void", "10x"),
        0x0f => ("return", "11x"),
        0x10 => ("return-wide", "11x"),
        0x11 => ("return-object", "11x"),
        0x12 => ("const/4", "11n"),
        0x13 => ("const/16", "21s"),
        0x14 => ("const", "31i"),
        0x15 => ("const/high16", "21h"),
        0x16 => ("const-wide/16", "21s"),
        0x17 => ("const-wide/32", "31i"),
        0x18 => ("const-wide", "51l"),
        0x19 => ("const-wide/high16", "21h"),
        0x1a => ("const-string", "21c"),
        0x1b => ("const-string/jumbo", "31c"),
        0x1c => ("const-class", "21c"),
        0x1d => ("monitor-enter", "11x"),
        0x1e => ("monitor-exit", "11x"),
        0x1f => ("check-cast", "21c"),
        0x20 => ("instance-of", "22c"),
        0x21 => ("array-length", "12x"),
        0x22 => ("new-instance", "21c"),
        0x23 => ("new-array", "22c"),
        0x24 => ("filled-new-array", "35c"),
        0x25 => ("filled-new-array/range", "3rc"),
        0x26 => ("fill-array-data", "31t"),
        0x27 => ("throw", "11x"),
        0x28 => ("goto", "10t"),
        0x29 => ("goto/16", "20t"),
        0x2a => ("goto/32", "30t"),
        0x2b => ("packed-switch", "31t"),
        0x2c => ("sparse-switch", "31t"),
        0x2d => ("cmpl-float", "23x"),
        0x2e => ("cmpg-float", "23x"),
        0x2f => ("cmpl-double", "23x"),
        0x30 => ("cmpg-double", "23x"),
        0x31 => ("cmp-long", "23x"),
        0x32 => ("if-eq", "22t"),
        0x33 => ("if-ne", "22t"),
        0x34 => ("if-lt", "22t"),
        0x35 => ("if-ge", "22t"),
        0x36 => ("if-gt", "22t"),
        0x37 => ("if-le", "22t"),
        0x38 => ("if-eqz", "21t"),
        0x39 => ("if-nez", "21t"),
        0x3a => ("if-ltz", "21t"),
        0x3b => ("if-gez", "21t"),
        0x3c => ("if-gtz", "21t"),
        0x3d => ("if-lez", "21t"),
        0x3e..=0x43 => ("unused", "10x"),
        0x44 => ("aget", "23x"),
        0x45 => ("aget-wide", "23x"),
        0x46 => ("aget-object", "23x"),
        0x47 => ("aget-boolean", "23x"),
        0x48 => ("aget-byte", "23x"),
        0x49 => ("aget-char", "23x"),
        0x4a => ("aget-short", "23x"),
        0x4b => ("aput", "23x"),
        0x4c => ("aput-wide", "23x"),
        0x4d => ("aput-object", "23x"),
        0x4e => ("aput-boolean", "23x"),
        0x4f => ("aput-byte", "23x"),
        0x50 => ("aput-char", "23x"),
        0x51 => ("aput-short", "23x"),
        0x52 => ("iget", "22c"),
        0x53 => ("iget-wide", "22c"),
        0x54 => ("iget-object", "22c"),
        0x55 => ("iget-boolean", "22c"),
        0x56 => ("iget-byte", "22c"),
        0x57 => ("iget-char", "22c"),
        0x58 => ("iget-short", "22c"),
        0x59 => ("iput", "22c"),
        0x5a => ("iput-wide", "22c"),
        0x5b => ("iput-object", "22c"),
        0x5c => ("iput-boolean", "22c"),
        0x5d => ("iput-byte", "22c"),
        0x5e => ("iput-char", "22c"),
        0x5f => ("iput-short", "22c"),
        0x60 => ("sget", "21c"),
        0x61 => ("sget-wide", "21c"),
        0x62 => ("sget-object", "21c"),
        0x63 => ("sget-boolean", "21c"),
        0x64 => ("sget-byte", "21c"),
        0x65 => ("sget-char", "21c"),
        0x66 => ("sget-short", "21c"),
        0x67 => ("sput", "21c"),
        0x68 => ("sput-wide", "21c"),
        0x69 => ("sput-object", "21c"),
        0x6a => ("sput-boolean", "21c"),
        0x6b => ("sput-byte", "21c"),
        0x6c => ("sput-char", "21c"),
        0x6d => ("sput-short", "21c"),
        0x6e => ("invoke-virtual", "35c"),
        0x6f => ("invoke-super", "35c"),
        0x70 => ("invoke-direct", "35c"),
        0x71 => ("invoke-static", "35c"),
        0x72 => ("invoke-interface", "35c"),
        0x73 => ("unused", "10x"),
        0x74 => ("invoke-virtual/range", "3rc"),
        0x75 => ("invoke-super/range", "3rc"),
        0x76 => ("invoke-direct/range", "3rc"),
        0x77 => ("invoke-static/range", "3rc"),
        0x78 => ("invoke-interface/range", "3rc"),
        0x79 | 0x7a => ("unused", "10x"),
        0x7b => ("neg-int", "12x"),
        0x7c => ("not-int", "12x"),
        0x7d => ("neg-long", "12x"),
        0x7e => ("not-long", "12x"),
        0x7f => ("neg-float", "12x"),
        0x80 => ("neg-double", "12x"),
        0x81 => ("int-to-long", "12x"),
        0x82 => ("int-to-float", "12x"),
        0x83 => ("int-to-double", "12x"),
        0x84 => ("long-to-int", "12x"),
        0x85 => ("long-to-float", "12x"),
        0x86 => ("long-to-double", "12x"),
        0x87 => ("float-to-int", "12x"),
        0x88 => ("float-to-long", "12x"),
        0x89 => ("float-to-double", "12x"),
        0x8a => ("double-to-int", "12x"),
        0x8b => ("double-to-long", "12x"),
        0x8c => ("double-to-float", "12x"),
        0x8d => ("int-to-byte", "12x"),
        0x8e => ("int-to-char", "12x"),
        0x8f => ("int-to-short", "12x"),
        0x90 => ("add-int", "23x"),
        0x91 => ("sub-int", "23x"),
        0x92 => ("mul-int", "23x"),
        0x93 => ("div-int", "23x"),
        0x94 => ("rem-int", "23x"),
        0x95 => ("and-int", "23x"),
        0x96 => ("or-int", "23x"),
        0x97 => ("xor-int", "23x"),
        0x98 => ("shl-int", "23x"),
        0x99 => ("shr-int", "23x"),
        0x9a => ("ushr-int", "23x"),
        0x9b => ("add-long", "23x"),
        0x9c => ("sub-long", "23x"),
        0x9d => ("mul-long", "23x"),
        0x9e => ("div-long", "23x"),
        0x9f => ("rem-long", "23x"),
        0xa0 => ("and-long", "23x"),
        0xa1 => ("or-long", "23x"),
        0xa2 => ("xor-long", "23x"),
        0xa3 => ("shl-long", "23x"),
        0xa4 => ("shr-long", "23x"),
        0xa5 => ("ushr-long", "23x"),
        0xa6 => ("add-float", "23x"),
        0xa7 => ("sub-float", "23x"),
        0xa8 => ("mul-float", "23x"),
        0xa9 => ("div-float", "23x"),
        0xaa => ("rem-float", "23x"),
        0xab => ("add-double", "23x"),
        0xac => ("sub-double", "23x"),
        0xad => ("mul-double", "23x"),
        0xae => ("div-double", "23x"),
        0xaf => ("rem-double", "23x"),
        0xb0 => ("add-int/2addr", "12x"),
        0xb1 => ("sub-int/2addr", "12x"),
        0xb2 => ("mul-int/2addr", "12x"),
        0xb3 => ("div-int/2addr", "12x"),
        0xb4 => ("rem-int/2addr", "12x"),
        0xb5 => ("and-int/2addr", "12x"),
        0xb6 => ("or-int/2addr", "12x"),
        0xb7 => ("xor-int/2addr", "12x"),
        0xb8 => ("shl-int/2addr", "12x"),
        0xb9 => ("shr-int/2addr", "12x"),
        0xba => ("ushr-int/2addr", "12x"),
        0xbb => ("add-long/2addr", "12x"),
        0xbc => ("sub-long/2addr", "12x"),
        0xbd => ("mul-long/2addr", "12x"),
        0xbe => ("div-long/2addr", "12x"),
        0xbf => ("rem-long/2addr", "12x"),
        0xc0 => ("and-long/2addr", "12x"),
        0xc1 => ("or-long/2addr", "12x"),
        0xc2 => ("xor-long/2addr", "12x"),
        0xc3 => ("shl-long/2addr", "12x"),
        0xc4 => ("shr-long/2addr", "12x"),
        0xc5 => ("ushr-long/2addr", "12x"),
        0xc6 => ("add-float/2addr", "12x"),
        0xc7 => ("sub-float/2addr", "12x"),
        0xc8 => ("mul-float/2addr", "12x"),
        0xc9 => ("div-float/2addr", "12x"),
        0xca => ("rem-float/2addr", "12x"),
        0xcb => ("add-double/2addr", "12x"),
        0xcc => ("sub-double/2addr", "12x"),
        0xcd => ("mul-double/2addr", "12x"),
        0xce => ("div-double/2addr", "12x"),
        0xcf => ("rem-double/2addr", "12x"),
        0xd0 => ("add-int/lit16", "22s"),
        0xd1 => ("rsub-int", "22s"),
        0xd2 => ("mul-int/lit16", "22s"),
        0xd3 => ("div-int/lit16", "22s"),
        0xd4 => ("rem-int/lit16", "22s"),
        0xd5 => ("and-int/lit16", "22s"),
        0xd6 => ("or-int/lit16", "22s"),
        0xd7 => ("xor-int/lit16", "22s"),
        0xd8 => ("add-int/lit8", "22b"),
        0xd9 => ("rsub-int/lit8", "22b"),
        0xda => ("mul-int/lit8", "22b"),
        0xdb => ("div-int/lit8", "22b"),
        0xdc => ("rem-int/lit8", "22b"),
        0xdd => ("and-int/lit8", "22b"),
        0xde => ("or-int/lit8", "22b"),
        0xdf => ("xor-int/lit8", "22b"),
        0xe0 => ("shl-int/lit8", "22b"),
        0xe1 => ("shr-int/lit8", "22b"),
        0xe2 => ("ushr-int/lit8", "22b"),
        0xe3..=0xf9 => ("unused", "10x"),
        0xfa => ("invoke-polymorphic", "45cc"),
        0xfb => ("invoke-polymorphic/range", "4rcc"),
        0xfc => ("invoke-custom", "35c"),
        0xfd => ("invoke-custom/range", "3rc"),
        0xfe => ("const-method-handle", "21c"),
        0xff => ("const-method-type", "21c"),
    }
}

/// Anchura en unidades de 16 bits de un opcode.
///
/// OJO: `goto` (10t) ocupa **dos** unidades (opcode + offset).  Con 1 el
/// desensamblado se desalinea tras cada salto.
pub fn instruction_width(opcode: u8) -> usize {
    match opcode_info(opcode).1 {
        "10x" | "12x" | "11n" | "11x" => 1,
        "10t" => 2,
        "20t" | "20bc" | "22x" | "21t" | "21s" | "21h" | "21c" | "23x" | "22b" | "22t"
        | "22s" | "22c" | "22cs" => 2,
        "30t" | "32x" | "31i" | "31t" | "31c" | "35c" | "3rc" => 3,
        "45cc" | "4rcc" => 4,
        "51l" => 5,
        _ => 1,
    }
}

/// Decodifica una instrucción en la posición `i` de `insns`.
///
/// `dex` se usa para resolver el texto de `const-string`.  Devuelve `None`
/// si la instrucción no cabe (DEX truncado).
pub fn decode_insn(dex: &DexFile, insns: &[u16], i: usize) -> Option<Insn> {
    let u0 = *insns.get(i)?;
    let opcode = (u0 & 0xff) as u8;
    let (mnemonic, format) = opcode_info(opcode);
    let width = instruction_width(opcode).max(1);
    if i + width > insns.len() {
        // Si la instrucción se sale, se devuelve lo que haya (tolerante).
        return Some(Insn {
            offset: i as u32,
            opcode,
            mnemonic,
            format,
            ..Default::default()
        });
    }
    let u1 = *insns.get(i + 1).unwrap_or(&0);
    let u2 = *insns.get(i + 2).unwrap_or(&0);
    let u3 = *insns.get(i + 3).unwrap_or(&0);

    let mut ins = Insn {
        offset: i as u32,
        opcode,
        mnemonic,
        format,
        ..Default::default()
    };

    match opcode {
        // --- constantes ---------------------------------------------- //
        0x12 => {
            // const/4 vA, #+B
            ins.dst = Some(((u0 >> 8) & 0x0f) as u8);
            let raw = ((u0 >> 12) & 0x0f) as i8;
            ins.int_const = Some(raw as i64);
        }
        0x13 => {
            // const/16 vAA, #+BBBB
            ins.dst = Some((u0 >> 8) as u8);
            ins.int_const = Some(u1 as i16 as i64);
        }
        0x14 => {
            // const vAA, #+BBBBBBBB
            ins.dst = Some((u0 >> 8) as u8);
            ins.int_const = Some((((u2 as u32) << 16) | u1 as u32) as i32 as i64);
        }
        0x15 => {
            // const/high16 vAA, #+BBBB0000
            ins.dst = Some((u0 >> 8) as u8);
            ins.int_const = Some((u1 as i16 as i64) << 16);
        }
        0x16 => {
            ins.dst = Some((u0 >> 8) as u8);
            ins.int_const = Some(u1 as i16 as i64);
        }
        0x1a => {
            // const-string vAA, string@BBBB
            ins.dst = Some((u0 >> 8) as u8);
            ins.string_idx = Some(u1 as u32);
            ins.str_const = dex.strings.get(u1 as usize).cloned();
        }
        0x1b => {
            // const-string/jumbo vAA, string@BBBBBBBB
            ins.dst = Some((u0 >> 8) as u8);
            let idx = ((u2 as u32) << 16) | u1 as u32;
            ins.string_idx = Some(idx);
            ins.str_const = dex.strings.get(idx as usize).cloned();
        }
        0x1c => {
            // const-class vAA, type@BBBB
            ins.dst = Some((u0 >> 8) as u8);
            ins.type_idx = Some(u1 as u32);
        }
        0x1f => {
            // check-cast vAA, type@BBBB
            ins.dst = Some((u0 >> 8) as u8);
            ins.type_idx = Some(u1 as u32);
        }
        0x22 => {
            // new-instance vAA, type@BBBB
            ins.dst = Some((u0 >> 8) as u8);
            ins.type_idx = Some(u1 as u32);
        }
        0x20 | 0x23 => {
            // instance-of / new-array: type@CCCC
            ins.type_idx = Some(u2 as u32);
            ins.dst = Some(((u0 >> 8) & 0x0f) as u8);
        }
        // --- invocaciones -------------------------------------------- //
        0x6e..=0x72 | 0x74..=0x78 | 0xfa..=0xfd => {
            ins.method_idx = Some(u1 as u32);
            if format == "35c" || format == "45cc" {
                // A|G|op BBBB F|E|D|C
                let c = (u2 & 0x0f) as u8;
                let d = ((u2 >> 4) & 0x0f) as u8;
                let e = ((u2 >> 8) & 0x0f) as u8;
                let f = ((u2 >> 12) & 0x0f) as u8;
                let g = ((u0 >> 12) & 0x0f) as u8;
                let argc = ((u0 >> 8) & 0x0f) as usize;
                let mut regs = vec![c, d, e, f, g];
                regs.truncate(argc.min(5));
                ins.regs = regs;
            } else {
                // AA|op BBBB CCCC
                let c = (u2 & 0x0f) as u8;
                let d = ((u2 >> 4) & 0x0f) as u8;
                let e = ((u2 >> 8) & 0x0f) as u8;
                let f = ((u2 >> 12) & 0x0f) as u8;
                let argc = (u0 >> 8) as usize;
                if argc <= 4 {
                    ins.regs = vec![c, d, e, f];
                    ins.regs.truncate(argc);
                } else {
                    // Rango: el primer registro es el inicio.
                    ins.regs = vec![argc as u8];
                }
            }
        }
        // --- campos -------------------------------------------------- //
        0x52..=0x5f => {
            // iget/iput: field@CCCC
            ins.field_idx = Some(u2 as u32);
            ins.dst = Some(((u0 >> 8) & 0x0f) as u8);
        }
        0x60..=0x6d => {
            // sget/sput: field@BBBB
            ins.field_idx = Some(u1 as u32);
            ins.dst = Some(((u0 >> 8) & 0x0f) as u8);
        }
        // --- movimientos --------------------------------------------- //
        0x01 | 0x04 | 0x07 => {
            // move vA, vB
            ins.dst = Some(((u0 >> 8) & 0x0f) as u8);
            ins.src = Some(((u0 >> 12) & 0x0f) as u8);
        }
        0x02 | 0x05 | 0x08 => {
            // move/from16 vAA, vBBBB
            ins.dst = Some((u0 >> 8) as u8);
            ins.src = Some(u1 as u8);
        }
        0x03 | 0x06 | 0x09 => {
            // move/16 vAAAA, vBBBB
            ins.dst = Some((u0 >> 8) as u8);
            ins.src = Some(u1 as u8);
        }
        0x0a..=0x0d => {
            ins.dst = Some((u0 >> 8) as u8);
        }
        _ => {}
    }
    let _ = u3;
    Some(ins)
}

/// ¿Es un *payload* (pseudo-instrucción de datos) y cuántas unidades ocupa?
///
/// Los payloads (`packed-switch-payload`, `sparse-switch-payload`,
/// `fill-array-data-payload`) se identifican por el **byte alto** del primer
/// code unit (0x01/0x02/0x03) con opcode 0x00.  Si no se saltan con su
/// tamaño real, el desensamblado se **desalinea** y todo lo que viene
/// después se atribuye a la instrucción equivocada.
fn payload_width(insns: &[u16], i: usize) -> Option<usize> {
    let u0 = *insns.get(i)?;
    if (u0 & 0xff) != 0 {
        return None;
    }
    let ident = u0 & 0xff00;
    let size = *insns.get(i + 1)? as usize;
    Some(match ident {
        0x0100 => 4 + 2 * size,           // packed-switch-payload
        0x0200 => 2 + 4 * size,           // sparse-switch-payload
        0x0300 => {
            // fill-array-data-payload: element_width en unit+2, size en unit+3
            let width = *insns.get(i + 2)? as usize;
            let count = (*insns.get(i + 3)? as usize)
                | ((*insns.get(i + 4)? as usize) << 16);
            4 + (count.saturating_mul(width) + 1) / 2
        }
        _ => return None,
    })
}

/// Recorre las instrucciones de un `code_item` **sin allocating** un vector.
///
/// Es la via que usa el pipeline: en un APK grande son decenas de miles de
/// metodos y un `Vec<Insn>` por metodo (con su `Vec` de registros) es la
/// diferencia entre 8 s y 20 s de analisis.
pub fn walk_insns(dex: &DexFile, insns: &[u16], mut f: impl FnMut(&Insn)) {
    let mut i = 0usize;
    while i < insns.len() {
        if let Some(w) = payload_width(insns, i) {
            let w = w.max(1);
            f(&Insn {
                offset: i as u32,
                opcode: 0,
                mnemonic: "payload",
                format: "payload",
                ..Default::default()
            });
            i += w;
            continue;
        }
        let Some(ins) = decode_insn(dex, insns, i) else {
            break;
        };
        let w = instruction_width(ins.opcode).max(1);
        f(&ins);
        i += w;
    }
}

/// Desensambla un `code_item` completo.
pub fn disassemble(dex: &DexFile, insns: &[u16]) -> Vec<Insn> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < insns.len() {
        if let Some(w) = payload_width(insns, i) {
            let w = w.max(1);
            out.push(Insn {
                offset: i as u32,
                opcode: 0,
                mnemonic: "payload",
                format: "payload",
                ..Default::default()
            });
            i += w;
            continue;
        }
        let Some(ins) = decode_insn(dex, insns, i) else {
            break;
        };
        let w = instruction_width(ins.opcode).max(1);
        out.push(ins);
        i += w;
    }
    out
}

// ========================================================================= //
// Ingesta del APK
// ========================================================================= //

/// Una entrada del ZIP del APK.
#[derive(Debug, Clone, Default)]
pub struct ApkEntry {
    /// Ruta dentro del APK.
    pub name: String,
    /// Tamaño descomprimido.
    pub size: u64,
    /// Tamaño comprimido.
    pub compressed_size: u64,
    /// ¿Es un directorio?
    pub is_dir: bool,
}

/// El APK abierto y sus piezas.
#[derive(Debug, Clone, Default)]
pub struct ApkBundle {
    /// Ruta del APK.
    pub path: String,
    /// SHA-256 del fichero completo.
    pub sha256: String,
    /// Tamaño del fichero.
    pub file_size: u64,
    /// Entradas del ZIP.
    pub entries: Vec<ApkEntry>,
    /// Contenido de `AndroidManifest.xml`.
    pub manifest_bytes: Vec<u8>,
    /// DEX del APK: (nombre, bytes).
    pub dex_files: Vec<(String, Vec<u8>)>,
    /// Bibliotecas nativas: (ruta, bytes).
    pub native_libs: Vec<(String, Vec<u8>)>,
    /// Assets: (ruta, bytes) — se limita el tamaño de cada uno.
    pub assets: Vec<(String, Vec<u8>)>,
    /// Rutas de `META-INF/`.
    pub meta_inf: Vec<String>,
}

/// Abre un APK y extrae sus piezas principales.
pub fn open_bundle(path: &str) -> Result<ApkBundle, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("no se pudo leer {}: {}", path, e))?;
    let mut file = File::open(path).map_err(|e| format!("no se pudo abrir {}: {}", path, e))?;
    let mut raw = Vec::new();
    file.read_to_end(&mut raw)
        .map_err(|e| format!("no se pudo leer {}: {}", path, e))?;
    let sha256 = sha256_hex(&raw);

    let mut bundle = ApkBundle {
        path: path.to_string(),
        sha256,
        file_size: meta.len(),
        ..Default::default()
    };

    let cursor = Cursor::new(raw);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("no es un ZIP válido: {}", e))?;

    for i in 0..archive.len() {
        let mut entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let name = entry.name().to_string();
        let is_dir = entry.is_dir();
        let size = entry.size();
        let compressed_size = entry.compressed_size();
        if bundle.entries.len() < MAX_ENTRIES_REPORTED * 4 {
            bundle.entries.push(ApkEntry {
                name: name.clone(),
                size,
                compressed_size,
                is_dir,
            });
        }
        if is_dir {
            continue;
        }
        if name.starts_with("META-INF/") {
            bundle.meta_inf.push(name);
            continue;
        }
        if name == "AndroidManifest.xml" {
            let mut buf = Vec::new();
            let _ = entry.read_to_end(&mut buf);
            bundle.manifest_bytes = buf;
            continue;
        }
        if name.starts_with("classes") && name.ends_with(".dex") {
            let mut buf = Vec::new();
            if entry.read_to_end(&mut buf).is_ok() {
                bundle.dex_files.push((name, buf));
            }
            continue;
        }
        if name.starts_with("lib/") && name.ends_with(".so") {
            if size <= MAX_NATIVE_SIZE {
                let mut buf = Vec::new();
                if entry.read_to_end(&mut buf).is_ok() {
                    bundle.native_libs.push((name, buf));
                }
            }
            continue;
        }
        if name.starts_with("assets/") && size <= 2 * 1024 * 1024 {
            let mut buf = Vec::new();
            if entry.read_to_end(&mut buf).is_ok() {
                bundle.assets.push((name, buf));
            }
        }
    }
    bundle.dex_files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(bundle)
}

// ========================================================================= //
// Perfilado de clases y métodos
// ========================================================================= //

/// Una llamada saliente detectada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallEdge {
    /// Clase que llama.
    pub caller_class: String,
    /// Método que llama.
    pub caller_method: String,
    /// Clase llamada.
    pub callee_class: String,
    /// Método llamado.
    pub callee_method: String,
}

/// Perfil de un método concreto.
#[derive(Debug, Clone, Default)]
pub struct MethodProfile {
    /// Clase propietaria.
    pub class_name: String,
    /// Nombre del método.
    pub method_name: String,
    /// Prototipo.
    pub proto: String,
    /// APIs chamadas: `"Lclase;->metodo"`.
    pub apis: BTreeSet<String>,
    /// Literales de texto usados en el método.
    pub const_strings: Vec<String>,
    /// ¿Usa alguna operación XOR? (pista fuerte de descifrado).
    pub uses_xor: bool,
    /// ¿Construye un `String`? (pista de descifrado).
    pub creates_string: bool,
    /// Modos de `Cipher.init` observados (1 = cifrar, 2 = descifrar).
    pub cipher_modes: BTreeSet<i64>,
    /// Constantes enteras que se pasan a un método (índice → valor).
    pub int_args: BTreeMap<u8, i64>,
}

/// Perfil de una clase.
#[derive(Debug, Clone, Default)]
pub struct ClassProfile {
    /// Descriptor de la clase.
    pub class_name: String,
    /// Nombre simple (`a.b.C` → `C`).
    pub simple_name: String,
    /// Superclase.
    pub superclass: String,
    /// Interfaces.
    pub interfaces: Vec<String>,
    /// Fichero fuente.
    pub source_file: String,
    /// Número de métodos.
    pub method_count: usize,
    /// Número de campos.
    pub field_count: usize,
    /// Tipos de los campos estáticos.
    pub static_field_types: Vec<String>,
    /// APIs usadas por la clase (con contador).
    pub apis: BTreeMap<String, usize>,
    /// Perfiles de los métodos.
    pub methods: Vec<MethodProfile>,
    /// Literales de texto de la clase.
    pub string_literals: Vec<String>,
    /// ¿Es un descifrador de strings? (array estático + XOR + `String`).
    pub is_string_decryptor: bool,
    /// ¿Carga código dinámicamente?
    pub loads_dynamic_code: bool,
}

/// Nombres cortos que **no** son ofuscación (los genera el tooling o el
/// idioma) pero que encajan en el patrón "1-3 caracteres alfanuméricos".
const NOT_OBFUSCATED_SHORT: [&str; 24] = [
    "R", "OK", "ID", "T", "K", "V", "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "L", "M", "N",
    "O", "P", "Q", "S", "U",
];

/// ¿El nombre parece ofuscado (1-3 caracteres alfanuméricos con letras)?
///
/// Cubre los tres patrones habituales de los ofuscadores: `a`, `ab`, `a05`,
/// `y87`, `z4`…  (R8 produce `a`, `b`, `c`; ProGuard produce `a`, `b`, `c`;
/// los empaquetadores chinos producen `a05`, `y87`…).
pub fn looks_obfuscated(name: &str) -> bool {
    if name.is_empty() || name.len() > 3 {
        return false;
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    if !name.chars().any(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    !NOT_OBFUSCATED_SHORT.contains(&name)
}

/// ¿El **descriptor de clase** parece ofuscado?
///
/// `La/b;`, `Laa;`, `La/b/c;` → sí.  `Lcom/google/gms/ads/AdView;` → no.
pub fn class_looks_obfuscated(descriptor: &str) -> bool {
    let Some(inner) = descriptor.strip_prefix('L') else {
        return false;
    };
    let inner = inner.trim_end_matches(';');
    let parts: Vec<&str> = inner.split('/').collect();
    if parts.is_empty() {
        return false;
    }
    // Todas las partes menos la última (el nombre de la clase) deben ser
    // cortas **y** no ser paquetes conocidos.
    let pkg_ok = parts[..parts.len().saturating_sub(1)]
        .iter()
        .all(|p| p.len() <= 2 && p.chars().all(|c| c.is_ascii_alphanumeric()));
    let simple = parts[parts.len() - 1];
    // La última parte puede tener `$` (clase interna).
    let simple_base = simple.split('$').next().unwrap_or(simple);
    // Un segmento `a$b` también es ofuscado si AMBAS partes lo son.
    let inner_ok = simple
        .split('$')
        .skip(1)
        .all(|part| part.is_empty() || looks_obfuscated(part));
    pkg_ok && inner_ok && looks_obfuscated(simple_base)
}

/// Construye el perfil completo de una clase a partir del DEX.
fn profile_class(dex: &DexFile, class: &DexClassDef) -> ClassProfile {
    let mut prof = ClassProfile {
        class_name: class.class_name.clone(),
        simple_name: class
            .class_name
            .trim_start_matches('L')
            .trim_end_matches(';')
            .rsplit('/')
            .next()
            .unwrap_or("")
            .to_string(),
        superclass: class.superclass.clone(),
        interfaces: class.interfaces.clone(),
        source_file: class.source_file.clone(),
        method_count: class.methods.len(),
        field_count: class.static_fields.len() + class.instance_fields.len(),
        ..Default::default()
    };

    for &(fidx, _) in class.static_fields.iter().chain(class.instance_fields.iter()) {
        if let Some(f) = dex.fields.get(fidx as usize) {
            prof.static_field_types.push(f.field_type.clone());
        }
    }

    let has_static_array = prof.static_field_types.iter().any(|t| {
        t == "[I" || t == "[B" || t == "[C" || t == "[Ljava/lang/String;"
    });

    for body in &class.methods {
        let mdef = dex.methods.get(body.method_idx as usize);
        let mname = mdef.map(|m| m.name.clone()).unwrap_or_default();
        let mut mp = MethodProfile {
            class_name: class.class_name.clone(),
            method_name: mname.clone(),
            proto: mdef.map(|m| m.proto.clone()).unwrap_or_default(),
            ..Default::default()
        };

        // Seguimiento de constantes (registro → valor).
        let mut int_consts: HashMap<u8, i64> = HashMap::new();
        let mut str_consts: HashMap<u8, String> = HashMap::new();

        walk_insns(dex, &body.insns, |ins| {
            match ins.opcode {
                0x12 | 0x13 | 0x14 | 0x15 | 0x16 => {
                    if let (Some(d), Some(v)) = (ins.dst, ins.int_const) {
                        int_consts.insert(d, v);
                    }
                }
                0x1a | 0x1b => {
                    if let (Some(d), Some(s)) = (ins.dst, ins.str_const.clone()) {
                        str_consts.insert(d, s.clone());
                        if !mp.const_strings.contains(&s) && mp.const_strings.len() < 64 {
                            mp.const_strings.push(s);
                        }
                    }
                }
                // move/move-wide/move-object: propaga la constante.
                0x01 | 0x04 | 0x07 | 0x02 | 0x05 | 0x08 => {
                    let a = ins.src.unwrap_or(0xff);
                    let d = ins.dst.unwrap_or(0);
                    if a == 0xff {
                        return;
                    }
                    if let Some(v) = int_consts.get(&a).copied() {
                        int_consts.insert(d, v);
                    }
                    if let Some(v) = str_consts.get(&a).cloned() {
                        str_consts.insert(d, v);
                    }
                }
                0x97 | 0xb7 | 0xa2 | 0xc2 => {
                    // xor-int / xor-int/2addr / xor-long / xor-long/2addr
                    mp.uses_xor = true;
                }
                _ => {}
            }

            if let Some(midx) = ins.method_idx {
                if let Some(m) = dex.methods.get(midx as usize) {
                    let key = format!("{}->{}", m.class_name, m.name);
                    mp.apis.insert(key.clone());
                    *prof.apis.entry(key).or_insert(0) += 1;
                    if !prof.string_literals.is_empty() {
                        // (relleno abajo)
                    }
                    // Modo de Cipher.init: el parámetro 0 es el modo.
                    let callee_static = dex
                        .method_flags
                        .get(&midx)
                        .map(|f| f & 0x0008 != 0)
                        .unwrap_or(false);
                    if m.class_name == "Ljavax/crypto/Cipher;" && m.name == "init" {
                        if let Some(reg) = param_register_static(m, callee_static, &ins.regs, 0) {
                            if let Some(v) = int_consts.get(&reg).copied() {
                                mp.cipher_modes.insert(v);
                            }
                        }
                    }
                    // Constante de texto pasada como argumento.
                    if let Some(reg) = param_register_static(m, callee_static, &ins.regs, 0) {
                        if let Some(s) = str_consts.get(&reg) {
                            if !mp.const_strings.contains(s) && mp.const_strings.len() < 64 {
                                mp.const_strings.push(s.clone());
                            }
                        }
                    }
                }
            }
            if let Some(tidx) = ins.type_idx {
                if dex.types.get(tidx as usize).map(|s| s.as_str()) == Some("Ljava/lang/String;") {
                    mp.creates_string = true;
                }
            }
        });

        if mp.apis.iter().any(|a| {
            a.starts_with("Ldalvik/system/DexClassLoader;")
                || a.starts_with("Ldalvik/system/PathClassLoader;")
                || a.starts_with("Ldalvik/system/InMemoryDexClassLoader;")
        }) {
            prof.loads_dynamic_code = true;
        }

        for s in &mp.const_strings {
            if !prof.string_literals.contains(s) && prof.string_literals.len() < 96 {
                prof.string_literals.push(s.clone());
            }
        }

        prof.methods.push(mp);
    }

    // Descifrador de strings: array estático + XOR + construcción de String.
    prof.is_string_decryptor = has_static_array
        && prof
            .methods
            .iter()
            .any(|m| m.uses_xor && m.creates_string);

    prof
}

impl Insn {
    /// (helper interno) El registro origen de una instrucción de movimiento.
    fn _unused(&self) {}
}

/// Registro del parámetro `idx` teniendo en cuenta si el método es estático.
///
/// En Dalvik, para un método de instancia `regs[0]` es `this` y los
/// parámetros empiezan en `regs[1]`; para uno estático `regs[0]` es ya el
/// primer parámetro.
fn param_register_static(m: &DexMethodDef, is_static: bool, regs: &[u8], idx: usize) -> Option<u8> {
    let _ = m;
    let base = if is_static { 0 } else { 1 };
    regs.get(base + idx).copied()
}

// ========================================================================= //
// Reglas de renombrado heurístico
// ========================================================================= //

/// Una regla que asigna un **rol** a una clase por su patrón de uso.
pub struct RoleRule {
    /// Descriptor (o prefijo) de la clase llamada.
    pub class: &'static str,
    /// Nombre del método llamado (vacío = cualquiera).
    pub method: &'static str,
    /// Rol sugerido para la clase llamadora.
    pub role: &'static str,
    /// Confianza base (0-100).
    pub confidence: u8,
}

/// Tabla de reglas de rol (por **patrón de uso**, no por nombre).
///
/// El nombre de la clase ofuscada no dice nada; lo que dice es **a qué APIs
/// llama**.  Ésa es la evidencia que se usa aquí.
pub const ROLE_RULES: &[RoleRule] = &[
    RoleRule { class: "Ljavax/crypto/Cipher;", method: "doFinal", role: "cipher", confidence: 70 },
    RoleRule { class: "Ljavax/crypto/Cipher;", method: "init", role: "cipher", confidence: 65 },
    RoleRule { class: "Ljavax/crypto/Mac;", method: "doFinal", role: "hmac", confidence: 76 },
    RoleRule { class: "Ljavax/crypto/spec/SecretKeySpec;", method: "<init>", role: "keyMaterial", confidence: 62 },
    RoleRule { class: "Ljava/security/MessageDigest;", method: "digest", role: "digest", confidence: 76 },
    RoleRule { class: "Ljava/security/KeyStore;", method: "load", role: "keyStore", confidence: 72 },
    RoleRule { class: "Landroid/content/pm/PackageManager;", method: "getPackageInfo", role: "pmQuery", confidence: 76 },
    RoleRule { class: "Landroid/content/pm/PackageManager;", method: "getApplicationInfo", role: "pmQuery", confidence: 72 },
    RoleRule { class: "Landroid/content/pm/PackageManager;", method: "queryIntentActivities", role: "pmQuery", confidence: 72 },
    RoleRule { class: "Landroid/content/pm/PackageManager;", method: "getInstalledPackages", role: "pmInventory", confidence: 78 },
    RoleRule { class: "Landroid/content/pm/PackageManager;", method: "installPackage", role: "pmInstall", confidence: 86 },
    RoleRule { class: "Landroid/content/pm/PackageInstaller;", method: "", role: "pmInstall", confidence: 80 },
    RoleRule { class: "Landroid/telephony/TelephonyManager;", method: "getDeviceId", role: "telephonyInfo", confidence: 82 },
    RoleRule { class: "Landroid/telephony/TelephonyManager;", method: "getLine1Number", role: "telephonyInfo", confidence: 82 },
    RoleRule { class: "Landroid/telephony/TelephonyManager;", method: "getSubscriberId", role: "telephonyInfo", confidence: 82 },
    RoleRule { class: "Landroid/telephony/TelephonyManager;", method: "getSimSerialNumber", role: "simInfo", confidence: 78 },
    RoleRule { class: "Landroid/telephony/TelephonyManager;", method: "getNetworkOperator", role: "simInfo", confidence: 72 },
    RoleRule { class: "Landroid/net/ConnectivityManager;", method: "getActiveNetworkInfo", role: "netState", confidence: 72 },
    RoleRule { class: "Landroid/net/ConnectivityManager;", method: "getNetworkCapabilities", role: "netState", confidence: 70 },
    RoleRule { class: "Landroid/location/LocationManager;", method: "getLastKnownLocation", role: "locationProvider", confidence: 78 },
    RoleRule { class: "Landroid/location/LocationManager;", method: "requestLocationUpdates", role: "locationProvider", confidence: 78 },
    RoleRule { class: "Landroid/location/LocationManager;", method: "", role: "locationProvider", confidence: 60 },
    RoleRule { class: "Landroid/hardware/Camera;", method: "open", role: "camera", confidence: 78 },
    RoleRule { class: "Landroid/media/MediaRecorder;", method: "setAudioSource", role: "audioRecorder", confidence: 78 },
    RoleRule { class: "Landroid/media/AudioRecord;", method: "startRecording", role: "audioCapture", confidence: 84 },
    RoleRule { class: "Landroid/webkit/WebView;", method: "addJavascriptInterface", role: "webViewBridge", confidence: 88 },
    RoleRule { class: "Landroid/webkit/WebView;", method: "setJavaScriptEnabled", role: "webView", confidence: 70 },
    RoleRule { class: "Landroid/webkit/WebView;", method: "loadUrl", role: "webView", confidence: 66 },
    RoleRule { class: "Ljava/net/HttpURLConnection;", method: "", role: "httpClient", confidence: 72 },
    RoleRule { class: "Ljava/net/URL;", method: "openConnection", role: "httpClient", confidence: 68 },
    RoleRule { class: "Lokhttp3/OkHttpClient;", method: "", role: "httpClient", confidence: 78 },
    RoleRule { class: "Lcom/google/gson/Gson;", method: "fromJson", role: "jsonParser", confidence: 80 },
    RoleRule { class: "Lcom/google/gson/Gson;", method: "toJson", role: "jsonParser", confidence: 78 },
    RoleRule { class: "Lorg/json/JSONObject;", method: "", role: "jsonHelper", confidence: 72 },
    RoleRule { class: "Lorg/json/JSONArray;", method: "", role: "jsonHelper", confidence: 66 },
    RoleRule { class: "Lcom/android/internal/org/json/JSONObject;", method: "", role: "jsonHelper", confidence: 72 },
    RoleRule { class: "Landroid/database/sqlite/SQLiteDatabase;", method: "", role: "dbHelper", confidence: 78 },
    RoleRule { class: "Landroid/content/SharedPreferences;", method: "getSharedPreferences", role: "prefsStore", confidence: 72 },
    RoleRule { class: "Landroid/util/Base64;", method: "", role: "base64Helper", confidence: 76 },
    RoleRule { class: "Ljava/util/zip/ZipInputStream;", method: "", role: "zipHelper", confidence: 72 },
    RoleRule { class: "Ljava/util/zip/ZipEntry;", method: "", role: "zipHelper", confidence: 62 },
    RoleRule { class: "Ljava/io/FileOutputStream;", method: "", role: "fileWriter", confidence: 66 },
    RoleRule { class: "Ljava/io/FileInputStream;", method: "", role: "fileReader", confidence: 66 },
    RoleRule { class: "Landroid/accounts/AccountManager;", method: "", role: "accountManager", confidence: 78 },
    RoleRule { class: "Landroid/os/Build;", method: "", role: "deviceInfo", confidence: 64 },
    RoleRule { class: "Landroid/os/Environment;", method: "", role: "envInfo", confidence: 58 },
    RoleRule { class: "Landroid/provider/Settings$Secure;", method: "getString", role: "settingsReader", confidence: 72 },
    RoleRule { class: "Landroid/provider/Settings$Global;", method: "getString", role: "settingsReader", confidence: 68 },
    RoleRule { class: "Lcom/android/vending/billing/IInAppBillingService;", method: "", role: "billingClient", confidence: 90 },
    RoleRule { class: "Lcom/android/billingclient/api/BillingClient;", method: "", role: "billingClient", confidence: 90 },
    RoleRule { class: "Lcom/google/android/gms/ads/", method: "", role: "adsSdk", confidence: 88 },
    RoleRule { class: "Lcom/facebook/", method: "", role: "socialSdk", confidence: 84 },
    RoleRule { class: "Lcom/google/firebase/", method: "", role: "firebaseSdk", confidence: 86 },
    RoleRule { class: "Lcom/android/notification/GcmListenerService;", method: "", role: "pushReceiver", confidence: 82 },
    RoleRule { class: "Lcom/google/android/gms/gcm/", method: "", role: "pushReceiver", confidence: 82 },
    RoleRule { class: "Landroid/app/NotificationManager;", method: "notify", role: "notifier", confidence: 64 },
    RoleRule { class: "Ljava/lang/reflect/Method;", method: "invoke", role: "reflection", confidence: 72 },
    RoleRule { class: "Ldalvik/system/DexClassLoader;", method: "", role: "dynamicLoader", confidence: 92 },
    RoleRule { class: "Ldalvik/system/PathClassLoader;", method: "", role: "dynamicLoader", confidence: 88 },
    RoleRule { class: "Ldalvik/system/InMemoryDexClassLoader;", method: "", role: "dynamicLoader", confidence: 92 },
    RoleRule { class: "Ldalvik/system/ClassLoader;", method: "loadClass", role: "classLoader", confidence: 74 },
    RoleRule { class: "Ljava/lang/Class;", method: "forName", role: "classLoader", confidence: 72 },
    RoleRule { class: "Landroid/net/Uri;", method: "parse", role: "uriHelper", confidence: 60 },
    RoleRule { class: "Ljavax/net/ssl/SSLContext;", method: "getInstance", role: "tlsHelper", confidence: 72 },
    RoleRule { class: "Lorg/apache/http/", method: "", role: "httpClient", confidence: 72 },
    RoleRule { class: "Landroid/app/admin/DevicePolicyManager;", method: "", role: "deviceAdmin", confidence: 84 },
    RoleRule { class: "Landroid/app/backup/BackupManager;", method: "", role: "backupHelper", confidence: 82 },
    RoleRule { class: "Landroid/telephony/SmsManager;", method: "", role: "smsSender", confidence: 86 },
    RoleRule { class: "Landroid/provider/Telephony;", method: "", role: "smsReader", confidence: 84 },
    RoleRule { class: "Landroid/content/ClipData;", method: "", role: "clipboard", confidence: 70 },
    RoleRule { class: "Landroid/util/DisplayMetrics;", method: "", role: "screenMetrics", confidence: 60 },
    RoleRule { class: "Ljava/util/UUID;", method: "", role: "uuidHelper", confidence: 55 },
];

/// Una regla que sugiere un **nombre de método** por su contexto.
pub struct MethodRule {
    /// Clase del método *llamado* (la evidencia).
    pub callee_class: &'static str,
    /// Método llamado.
    pub callee_method: &'static str,
    /// Nombre sugerido para el método *llamador*.
    pub suggested: &'static str,
    /// Confianza base.
    pub confidence: u8,
}

/// Tabla de reglas de renombrado de **métodos**.
pub const METHOD_RULES: &[MethodRule] = &[
    MethodRule { callee_class: "Ljavax/crypto/Cipher;", callee_method: "doFinal", suggested: "encryptOrDecrypt", confidence: 78 },
    MethodRule { callee_class: "Ljavax/crypto/Cipher;", callee_method: "init", suggested: "initCipher", confidence: 68 },
    MethodRule { callee_class: "Ljavax/crypto/Mac;", callee_method: "doFinal", suggested: "computeHmac", confidence: 80 },
    MethodRule { callee_class: "Ljava/security/MessageDigest;", callee_method: "digest", suggested: "computeDigest", confidence: 76 },
    MethodRule { callee_class: "Ldalvik/system/DexClassLoader;", callee_method: "<init>", suggested: "loadDynamicDex", confidence: 86 },
    MethodRule { callee_class: "Ldalvik/system/PathClassLoader;", callee_method: "<init>", suggested: "loadDynamicDex", confidence: 84 },
    MethodRule { callee_class: "Ljava/lang/Class;", callee_method: "forName", suggested: "resolveClass", confidence: 76 },
    MethodRule { callee_class: "Landroid/content/pm/PackageManager;", callee_method: "getPackageInfo", suggested: "queryPackageInfo", confidence: 78 },
    MethodRule { callee_class: "Landroid/telephony/TelephonyManager;", callee_method: "getDeviceId", suggested: "readDeviceId", confidence: 82 },
    MethodRule { callee_class: "Landroid/telephony/TelephonyManager;", callee_method: "getLine1Number", suggested: "readPhoneNumber", confidence: 82 },
    MethodRule { callee_class: "Landroid/location/LocationManager;", callee_method: "getLastKnownLocation", suggested: "readLastLocation", confidence: 78 },
    MethodRule { callee_class: "Landroid/util/Base64;", callee_method: "encodeToString", suggested: "encodeBase64", confidence: 78 },
    MethodRule { callee_class: "Landroid/util/Base64;", callee_method: "decode", suggested: "decodeBase64", confidence: 78 },
    MethodRule { callee_class: "Landroid/content/SharedPreferences;", callee_method: "getSharedPreferences", suggested: "openPreferences", confidence: 74 },
    MethodRule { callee_class: "Landroid/webkit/WebView;", callee_method: "addJavascriptInterface", suggested: "exposeJsBridge", confidence: 88 },
    MethodRule { callee_class: "Landroid/webkit/WebView;", callee_method: "setJavaScriptEnabled", suggested: "enableJavaScript", confidence: 76 },
    MethodRule { callee_class: "Lcom/google/gson/Gson;", callee_method: "fromJson", suggested: "parseJson", confidence: 80 },
    MethodRule { callee_class: "Lcom/google/gson/Gson;", callee_method: "toJson", suggested: "serializeJson", confidence: 80 },
    MethodRule { callee_class: "Landroid/telephony/SmsManager;", callee_method: "sendTextMessage", suggested: "sendSms", confidence: 88 },
    MethodRule { callee_class: "Ljava/util/zip/ZipInputStream;", callee_method: "getNextEntry", suggested: "nextZipEntry", confidence: 70 },
    MethodRule { callee_class: "Landroid/app/Activity;", callee_method: "startActivity", suggested: "launchActivity", confidence: 66 },
    MethodRule { callee_class: "Ljava/lang/String;", callee_method: "<init>", suggested: "buildString", confidence: 55 },
];

/// Confianza mínima para **aplicar** un renombrado.
///
/// Por debajo de este valor se conserva el nombre original y se dice
/// explícitamente por qué: es preferible no renombrar a renombrar mal.
pub const RENAME_CONFIDENCE_THRESHOLD: u8 = 60;

/// Calcula el rol y la confianza de una clase a partir de sus APIs.
///
/// Devuelve `None` si no hay ninguna regla que case (⇒ sin propuesta).
pub fn classify_class(profile: &ClassProfile) -> Option<(String, u8, Vec<String>)> {
    let mut best: Option<(&RoleRule, usize)> = None;
    let mut evidence: Vec<String> = Vec::new();
    let mut distinct_roles: BTreeSet<&str> = BTreeSet::new();

    for api in profile.apis.keys() {
        let (cls, mth) = match api.split_once("->") {
            Some(v) => v,
            None => continue,
        };
        for rule in ROLE_RULES {
            let class_match = if rule.class.ends_with('/') {
                cls.starts_with(rule.class)
            } else {
                cls == rule.class
            };
            if !class_match {
                continue;
            }
            if !rule.method.is_empty() && rule.method != mth {
                continue;
            }
            let count = profile.apis.get(api).copied().unwrap_or(1);
            distinct_roles.insert(rule.role);
            evidence.push(format!("{} (x{})", api, count));
            if best.map(|(_, c)| count > c).unwrap_or(true) {
                best = Some((rule, count));
            }
        }
    }

    let (rule, count) = best?;
    let mut confidence = rule.confidence;
    if distinct_roles.len() > 1 {
        confidence = confidence.saturating_add(6);
    }
    if count > 2 {
        confidence = confidence.saturating_add(4);
    }
    if class_looks_obfuscated(&profile.class_name) {
        confidence = confidence.saturating_add(5);
    } else {
        // La clase ya tiene un nombre legible: no se toca.
        confidence = confidence.saturating_sub(40);
    }
    if confidence > 95 {
        confidence = 95;
    }
    Some((rule.role.to_string(), confidence, evidence))
}

/// Calcula el nombre sugerido para un **método**, con su confianza.
pub fn classify_method(
    profile: &ClassProfile,
    method: &MethodProfile,
) -> Option<(String, u8, String)> {
    // Caso especial: `Cipher.doFinal` distingue cifrar de descifrar por el
    // modo que se le pasó a `Cipher.init` (1 = ENCRYPT, 2 = DECRYPT).
    let mut best: Option<(&MethodRule, String, u8)> = None;
    for api in &method.apis {
        let (cls, mth) = match api.split_once("->") {
            Some(v) => v,
            None => continue,
        };
        for rule in METHOD_RULES {
            if rule.callee_class != cls || rule.callee_method != mth {
                continue;
            }
            let mut suggested = rule.suggested.to_string();
            let mut confidence = rule.confidence;
            if cls == "Ljavax/crypto/Cipher;" && mth == "doFinal" {
                // El modo de `Cipher.init` dice si la clase CIFRA o DESCIFRA:
                // 1 = ENCRYPT_MODE, 2 = DECRYPT_MODE.
                if method.cipher_modes.contains(&1) && !method.cipher_modes.contains(&2) {
                    suggested = "encrypt".to_string();
                } else if method.cipher_modes.contains(&2) && !method.cipher_modes.contains(&1) {
                    suggested = "decrypt".to_string();
                } else {
                    suggested = "transform".to_string();
                    confidence = confidence.saturating_sub(12);
                }
            }
            if best.is_none() {
                best = Some((rule, suggested, confidence));
            }
        }
    }
    let (rule, suggested, mut confidence) = best?;
    if looks_obfuscated(&method.method_name) {
        confidence = confidence.saturating_add(6);
    } else {
        confidence = confidence.saturating_sub(40);
    }
    if class_looks_obfuscated(&profile.class_name) {
        confidence = confidence.saturating_add(4);
    }
    if confidence > 95 {
        confidence = 95;
    }
    let evidence = method
        .apis
        .iter()
        .filter(|a| {
            METHOD_RULES
                .iter()
                .any(|r| a.ends_with(&format!("->{}", r.callee_method)))
        })
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    Some((suggested, confidence, evidence))
}

// ========================================================================= //
// Firmas de ofuscadores / empaquetadores / frameworks de parcheo
// ========================================================================= //

/// Una firma de herramienta de ofuscación, empaquetado o parcheo.
pub struct ObfSignature {
    /// Nombre de la herramienta.
    pub name: &'static str,
    /// Fabricante.
    pub vendor: &'static str,
    /// `obfuscator`, `packer`, `protector` o `patch-framework`.
    pub category: &'static str,
    /// Rutas del APK que delatan la herramienta.
    pub paths: &'static [&'static str],
    /// Cadenas en el DEX o en las `.so`.
    pub strings: &'static [&'static str],
    /// Prefijos de clase.
    pub classes: &'static [&'static str],
    /// Nombres de `.so`.
    pub natives: &'static [&'static str],
    /// Qué se sabe de ella.
    pub note: &'static str,
}

/// Tabla de firmas conocidas.
pub const OBF_SIGNATURES: &[ObfSignature] = &[
    ObfSignature {
        name: "DexGuard",
        vendor: "GuardSquare",
        category: "protector",
        paths: &["assets/guardian/"],
        strings: &["DexGuard", "guardsquare", "guardianprotection"],
        classes: &["Lcom/guardianprotection/", "Lcom/guardsquare/"],
        natives: &["libdexguard", "libguardian"],
        note: "Protector comercial de GuardSquare (ProGuard + cifrado de strings + anti-debug nativo).",
    },
    ObfSignature {
        name: "Allatori",
        vendor: "Allatori Software",
        category: "obfuscator",
        paths: &[],
        strings: &["Allatori Obfuscator", "allatori"],
        classes: &["Lcom/allatori/"],
        natives: &[],
        note: "Ofuscador comercial para Java/Android; renombra y cifra strings.",
    },
    ObfSignature {
        name: "ZKM (Zelix KlassMaster)",
        vendor: "Zelix",
        category: "obfuscator",
        paths: &[],
        strings: &["Zelix KlassMaster", "zelix.classloader", "StringProtector"],
        classes: &["Lcom/zelix/"],
        natives: &[],
        note: "Ofuscador Java con cifrado de strings (StringProtector) y ofuscación de flujo.",
    },
    ObfSignature {
        name: "Stringer",
        vendor: "open source",
        category: "obfuscator",
        paths: &[],
        strings: &["Stringer obfuscator", "com.stringer"],
        classes: &["Lcom/stringer/"],
        natives: &[],
        note: "Ofuscador ligero centrado en el cifrado de strings.",
    },
    ObfSignature {
        name: "Bangcle / SecShell",
        vendor: "Bangcle / SecNeo",
        category: "packer",
        paths: &[
            "assets/libsecmain.so",
            "assets/libsecexe.so",
            "assets/bangcle_classes.jar",
            "assets/classes.jar",
        ],
        strings: &["bangcle", "SecShell", "secneo", "secexe"],
        classes: &["Lcom/secneo/"],
        natives: &["libsecmain.so", "libsecexe.so", "libSecShell.so"],
        note: "Empaquetador que sustituye el DEX original por uno cifrado y lo descifra en `attachBaseContext`.",
    },
    ObfSignature {
        name: "Tencent Legu",
        vendor: "Tencent",
        category: "packer",
        paths: &["assets/o0oooOO0ooOO.dat", "assets/0OO00l111l1l", "assets/af.bin"],
        strings: &["Legu", "libshella", "o0oooOO0ooOO"],
        classes: &["Lcom/tencent/"],
        natives: &["libshell.so", "libshella", "libtup.so", "libtosso"],
        note: "Empaquetador de Tencent: `libshell.so` + el marcador `o0oooOO0ooOO.dat`.",
    },
    ObfSignature {
        name: "360 Jiagu",
        vendor: "Qihoo 360",
        category: "packer",
        paths: &["assets/.app_protect", "assets/.appkey"],
        strings: &["jiagu", "qihoo360", "libjiagu"],
        classes: &["Lcom/qihoo/", "Lcom/stub/"],
        natives: &["libjiagu.so", "libjiagu_art.so", "libjiagu64.so", "libjiagu_x86.so"],
        note: "Empaquetador de 360: `libjiagu.so` + carga en tiempo de ejecución.",
    },
    ObfSignature {
        name: "Ijiami",
        vendor: "Ijiami",
        category: "packer",
        paths: &["assets/ijiami.dat", "assets/ijiamiCa.keystore", "assets/ijiami_"],
        strings: &["ijiami", "ijiamiCa"],
        classes: &["Lcom/shell/"],
        natives: &["libexec.so", "libexecmain.so", "libijiami.so"],
        note: "Empaquetador de Ijiami: `libexecmain.so` + `assets/ijiami.dat`.",
    },
    ObfSignature {
        name: "LSPatch",
        vendor: "LSPosed",
        category: "patch-framework",
        paths: &["assets/lspatch/"],
        strings: &["LSPatch", "lspatch"],
        classes: &["Lcom/lspatch/hook/", "Lcom/lspatch/"],
        natives: &["liblspatch.so"],
        note: "Parcheador que reempaqueta el APK original con un DEX parcheado dentro.",
    },
    ObfSignature {
        name: "KnoxPatch",
        vendor: "TSNG",
        category: "patch-framework",
        paths: &["assets/knoxpatch/"],
        strings: &["KnoxPatch", "knoxpatch"],
        classes: &["Lcom/tsng/knoxpatch/"],
        natives: &[],
        note: "Variante de LSPatch centrada en el bypass de Integrity API de Samsung Knox.",
    },
    ObfSignature {
        name: "EdXposed",
        vendor: "EdXposed",
        category: "patch-framework",
        paths: &["assets/edxposed/"],
        strings: &["EdXposed", "edxposed"],
        classes: &["Lorg/edxposed/hook/", "Lorg/edxposed/"],
        natives: &["libedxposed.so"],
        note: "Framework de hooks moderno; se apoya en LSPosed/Zygisk.",
    },
    ObfSignature {
        name: "MonoPatch",
        vendor: "MonoPatch",
        category: "patch-framework",
        paths: &[],
        strings: &["MonoPatch", "monopatch"],
        classes: &["Lcom/monopatch/"],
        natives: &[],
        note: "Parcheador para apps Unity/Mono.",
    },
    ObfSignature {
        name: "TrustMe",
        vendor: "topjohnwu",
        category: "patch-framework",
        paths: &[],
        strings: &["TrustMe", "trustme"],
        classes: &["Lcom/topjohnwu/trustme/"],
        natives: &[],
        note: "Reemplaza la verificación de firma de la app sin hookear el proceso.",
    },
    ObfSignature {
        name: "Xposed / LSPosed",
        vendor: "Xposed / LSPosed",
        category: "patch-framework",
        paths: &[
            "assets/xposed_init",
            "assets/xposed_scope",
            "META-INF/xposed/",
            "META-INF/xposed/module.prop",
        ],
        strings: &["XposedBridge", "xposedmodule", "de.robv.android.xposed"],
        classes: &["Lde/robv/android/xposed/", "Landroid/content/pm/XposedHelpers;"],
        natives: &[],
        note: "Módulo Xposed/LSPosed: se ancla en `assets/xposed_init`.",
    },
    ObfSignature {
        name: "Frida gadget",
        vendor: "Frida",
        category: "patch-framework",
        paths: &[],
        strings: &["frida-gadget", "frida-agent", "gadget.config", "frida_agent"],
        classes: &[],
        natives: &["libfrida-gadget.so", "libgadget.config.so", "libfrida-agent.so"],
        note: "Binario de instrumentación incrustado en la app (Frida Gadget).",
    },
];

/// El resultado de buscar una firma.
#[derive(Debug, Clone)]
pub struct SignatureHit {
    /// Nombre de la herramienta.
    pub name: String,
    /// Fabricante.
    pub vendor: String,
    /// Categoría.
    pub category: String,
    /// Evidencia concreta (ruta, cadena o clase encontrada).
    pub evidence: Vec<String>,
    /// Confianza 0-100.
    pub confidence: u8,
    /// Nota informative.
    pub note: String,
}

/// Busca las firmas conocidas en un APK.
pub fn detect_signatures(
    entry_names: &[String],
    dex_strings: &HashSet<String>,
    class_names: &[String],
    native_names: &[String],
    native_strings: &[String],
) -> Vec<SignatureHit> {
    let mut hits = Vec::new();
    for sig in OBF_SIGNATURES {
        let mut evidence: Vec<String> = Vec::new();
        let mut confidence: u8 = 0;

        for p in sig.paths {
            if entry_names.iter().any(|n| n.contains(p)) {
                evidence.push(format!("ruta: {}", p));
                confidence = confidence.max(90);
            }
        }
        for n in sig.natives {
            if native_names.iter().any(|x| x.contains(n)) {
                evidence.push(format!("biblioteca nativa: {}", n));
                confidence = confidence.max(88);
            }
        }
        for c in sig.classes {
            if class_names.iter().any(|x| x.starts_with(c)) {
                evidence.push(format!("clase: {}*", c));
                confidence = confidence.max(85);
            }
        }
        for s in sig.strings {
            let mut found = false;
            for ds in dex_strings {
                if ds.to_lowercase().contains(&s.to_lowercase()) {
                    found = true;
                    break;
                }
            }
            if !found {
                for ns in native_strings {
                    if ns.to_lowercase().contains(&s.to_lowercase()) {
                        found = true;
                        break;
                    }
                }
            }
            if found {
                evidence.push(format!("cadena: \"{}\"", s));
                confidence = confidence.max(78);
            }
        }

        if !evidence.is_empty() {
            hits.push(SignatureHit {
                name: sig.name.to_string(),
                vendor: sig.vendor.to_string(),
                category: sig.category.to_string(),
                evidence,
                confidence,
                note: sig.note.to_string(),
            });
        }
    }
    hits.sort_by(|a, b| b.confidence.cmp(&a.confidence));
    hits
}

/// Detecta ofuscación ProGuard/R8 **por estructura** (no hay marcador fijo).
///
/// Un APK ofuscado con R8/ProGuard se caracteriza por: muchísimas clases con
/// nombre de 1-2 caracteres, métodos de 1-2 caracteres y pérdida del
/// `SourceFile`.
pub fn detect_proguard_like(classes: &[DexClassDef]) -> Option<SignatureHit> {
    let total = classes.len();
    let short = classes
        .iter()
        .filter(|c| class_looks_obfuscated(&c.class_name))
        .count();
    let with_source = classes.iter().filter(|c| !c.source_file.is_empty()).count();
    detect_proguard_like_stats(total, short, with_source)
}

/// Variante que recibe ya las estadísticas (no necesita las clases enteras).
///
/// Se usa en el pipeline, que procesa DEX a DEX para no retener todas las
/// clases en memoria.
pub fn detect_proguard_like_stats(
    total: usize,
    short: usize,
    with_source: usize,
) -> Option<SignatureHit> {
    detect_proguard_like_stats_full(total, short, with_source, 0)
}

/// Variante que ademas recibe el numero de clases con **map-id de R8**.
///
/// R8 (y ProGuard con `-adaptclassstrings`) escribe el identificador del
/// mapa de ofuscacion en el atributo `SourceFile`:
/// `r8-map-id-<sha256>`.  Es una **prueba definitiva** de R8.
pub fn detect_proguard_like_stats_full(
    total: usize,
    short: usize,
    with_source: usize,
    r8_map_ids: usize,
) -> Option<SignatureHit> {
    if r8_map_ids > 0 {
        return Some(SignatureHit {
            name: "R8".to_string(),
            vendor: "Google".to_string(),
            category: "obfuscator".to_string(),
            evidence: vec![format!(
                "{} clases con `SourceFile` = `r8-map-id-<sha256>` (identificador del mapa de ofuscacion)",
                r8_map_ids
            )],
            confidence: 99,
            note: "R8 escribe el hash de su mapa de renombrado en `SourceFile`: prueba inequivoca de ofuscacion con R8.".to_string(),
        });
    }
    if total < 20 {
        return None;
    }
    let ratio = short as f64 / total as f64;
    let source_ratio = with_source as f64 / total as f64;

    // Umbral: >40 % de clases cortas => casi seguro ofuscado.
    if ratio < 0.40 {
        return None;
    }
    let mut confidence = (40.0 + ratio * 55.0).min(95.0) as u8;
    let mut evidence = vec![format!(
        "{}/{} clases con nombre de 1-2 caracteres ({:.0} %)",
        short,
        total,
        ratio * 100.0
    )];
    if source_ratio < 0.20 {
        evidence.push(format!(
            "solo {:.0} % de las clases conservan `SourceFile` (atributo eliminado)",
            source_ratio * 100.0
        ));
        confidence = confidence.saturating_add(6);
    }
    Some(SignatureHit {
        name: "ProGuard/R8".to_string(),
        vendor: "Google / ProGuard".to_string(),
        category: "obfuscator".to_string(),
        evidence,
        confidence: confidence.min(97),
        note: "Renombrado de clases/metodos a nombres cortos. Se detecta por estructura, no por un marcador fijo.".to_string(),
    })
}

// ========================================================================= //
// Análisis de bibliotecas nativas
// ========================================================================= //

/// Funciones que permiten **ejecutar procesos**: se busca en la tabla de
/// imports del ELF (preciso) y **no** en cadenas sueltas (falsos positivos
/// con "java.lang.System", "System.currentTimeMillis"…).
pub const DANGEROUS_IMPORTS: &[&str] = &[
    "system", "execve", "execl", "execlp", "execle", "execvp", "popen", "posix_spawn",
];

/// Cadenas **exactas** que delatan ejecucion de comandos o rootkits.
/// Se comparan como subcadena pero son lo bastante especificas.
pub const DANGEROUS_STRINGS: &[&str] = &[
    "/system/bin/sh",
    "/system/bin/su",
    "/system/xbin/su",
    "/sbin/su",
    "/data/local/tmp",
    "magisk",
    "frida-server",
    "com.saurik.substrate",
];

/// Funciones de libc que un framework de hooking intercepta con frecuencia.
pub const HOOK_SURFACE: &[&str] = &[
    "open", "openat", "fopen", "readlink", "stat", "access", "getenv",
    "ptrace", "dlopen", "dlsym", "mprotect", "mmap", "mprotect", "fork",
    "system", "execve", "popen", "kill", "signal", "sigaction", "connect",
    "send", "recv", "strcmp", "strncmp", "memcmp", "__system_property_get",
    "prctl", "syscall",
];

/// Indicadores de frameworks de instrumentación en el interior de una `.so`.
pub const FRAMEWORK_MARKERS: &[&str] = &[
    "frida", "xposed", "substrate", "sandhook", "whale", "epic", "yahfa",
    "riru", "zygisk", "lsposed", "liberty", "dobby", "shadowhook",
];

/// Lo que se sabe de una biblioteca nativa embebida.
#[derive(Debug, Clone, Default)]
pub struct NativeLibInfo {
    /// Ruta dentro del APK.
    pub path: String,
    /// ABI (`arm64-v8a`, `armeabi-v7a`, `x86_64`…).
    pub abi: String,
    /// Nombre del fichero.
    pub name: String,
    /// Tamaño.
    pub size: u64,
    /// SHA-256.
    pub sha256: String,
    /// ¿ELF válido?
    pub is_elf: bool,
    /// ¿Está *stripped* (sin `.symtab`)?
    pub stripped: bool,
    /// Arquitectura.
    pub machine: String,
    /// Tipo (`ET_DYN`…).
    pub elf_type: String,
    /// Símbolos exportados.
    pub exports: Vec<String>,
    /// Símbolos importados (de libc y del resto).
    pub imports: Vec<String>,
    /// Bibliotecas compartidas necesarias.
    pub needed: Vec<String>,
    /// ¿Tiene `JNI_OnLoad`?
    pub jni_onload: bool,
    /// Funciones de libc con superficie de hook.
    pub hook_surface: Vec<String>,
    /// Marcadores de framework de instrumentación.
    pub framework_hits: Vec<String>,
    /// Cadenascave suspicious: `system`, `execve`, `su`, `magisk`…
    pub suspicious: Vec<String>,
}

/// Analiza una `.so` embebida.
pub fn analyze_native(path: &str, bytes: &[u8]) -> NativeLibInfo {
    let mut info = NativeLibInfo {
        path: path.to_string(),
        abi: path
            .split('/')
            .nth(1)
            .unwrap_or("")
            .to_string(),
        name: path.rsplit('/').next().unwrap_or(path).to_string(),
        size: bytes.len() as u64,
        sha256: sha256_hex(bytes),
        ..Default::default()
    };

    match format_oracle::parse_elf(bytes) {
        Ok(elf) => {
            info.is_elf = true;
            info.stripped = elf.stripped;
            info.machine = elf.machine_name.clone();
            info.elf_type = elf.type_name.clone();
            info.exports = elf.exported.iter().take(MAX_NATIVE_SYMBOLS).cloned().collect();
            info.imports = elf.imported.iter().take(MAX_NATIVE_SYMBOLS).cloned().collect();
            info.needed = elf.needed.clone();
            info.jni_onload = elf.exported.iter().any(|s| s == "JNI_OnLoad");
        }
        Err(_) => {
            // No es ELF (puede ser un script o un blob ofuscado).
            info.machine = "no-elf".to_string();
        }
    }

    let strings = format_oracle::extract_strings(bytes, 5);
    for s in &strings {
        let low = s.to_lowercase();
        if HOOK_SURFACE.iter().any(|h| low.contains(&format!("_{}", h)) || low == *h) {
            let name = s.trim_start_matches('_').to_string();
            if !info.hook_surface.contains(&name) && info.hook_surface.len() < 24 {
                info.hook_surface.push(name);
            }
        }
        for marker in FRAMEWORK_MARKERS {
            if low.contains(marker) {
                let hit = format!("{}: {}", marker, truncate(s, 90));
                if !info.framework_hits.contains(&hit) && info.framework_hits.len() < 12 {
                    info.framework_hits.push(hit);
                }
            }
        }
        for bad in DANGEROUS_STRINGS {
            if low.contains(bad) && !info.suspicious.contains(&bad.to_string()) {
                info.suspicious.push(bad.to_string());
            }
        }
    }

    // Imports peligrosos (preciso): viene de la tabla de simbolos del ELF.
    for imp in &info.imports {
        let bare = imp.trim_start_matches('_');
        if DANGEROUS_IMPORTS.contains(&bare) && !info.suspicious.contains(&bare.to_string()) {
            info.suspicious.push(bare.to_string());
        }
    }
    info
}

/// Recorta una cadena a `max` caracteres.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect::<String>() + "…"
    }
}

// ========================================================================= //
// Hallazgos de seguridad
// ========================================================================= //

/// Un hallazgo de auditoría.
#[derive(Debug, Clone)]
pub struct Finding {
    /// Identificador estable.
    pub id: String,
    /// `critical`, `high`, `medium`, `low` o `info`.
    pub severity: String,
    /// Título corto.
    pub title: String,
    /// Detalle.
    pub detail: String,
    /// Evidencia (clase → método → efecto).
    pub evidence: String,
    /// CWE asociado, si aplica.
    pub cwe: Option<String>,
}

impl Finding {
    /// Peso en el cálculo de riesgo.
    fn weight(&self) -> u32 {
        match self.severity.as_str() {
            "critical" => 25,
            "high" => 10,
            "medium" => 4,
            "low" => 1,
            _ => 0,
        }
    }

    /// Versión serializable.
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "severity": self.severity,
            "title": self.title,
            "detail": self.detail,
            "evidence": self.evidence,
            "cwe": self.cwe,
        })
    }
}

/// Calcula el riesgo global 0-100 a partir de los hallazgos.
pub fn compute_risk(findings: &[Finding]) -> u32 {
    let total: u32 = findings.iter().map(|f| f.weight()).sum();
    total.min(100)
}

// ========================================================================= //
// Extracción de endpoints
// ========================================================================= //

/// Extrae URLs de un texto.
pub fn extract_endpoints(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let is_start = bytes[i] == b'h'
            && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
            && text[i..].len() >= 7
            && (text[i..].starts_with("http://") || text[i..].starts_with("https://"));
        if is_start {
            let mut j = i;
            while j < bytes.len() {
                let c = bytes[j];
                if c.is_ascii_whitespace() || c == b'"' || c == b'\'' || c == b'<' || c == b'>' || c == b'\\' {
                    break;
                }
                j += 1;
            }
            let url = text[i..j].trim_end_matches(['.', ',', ';', ')', ']', '}']);
            if url.len() > 10 && !out.iter().any(|u: &String| u == url) {
                out.push(url.to_string());
            }
            i = j;
            continue;
        }
        i += 1;
    }
    out
}


// ========================================================================= //
// Puntos de descifrado y strings cifrados
// ========================================================================= //

/// Entropía de Shannon de un texto (bits por carácter).
pub fn shannon_entropy(text: &str) -> f64 {
    if text.is_empty() {
        return 0.0;
    }
    let mut freq = [0usize; 256];
    for &b in text.as_bytes() {
        freq[b as usize] += 1;
    }
    let len = text.as_bytes().len() as f64;
    let mut ent = 0.0;
    for &c in &freq {
        if c > 0 {
            let p = c as f64 / len;
            ent -= p * p.log2();
        }
    }
    ent
}

/// ¿El texto tiene pinta de estar cifrado o codificado?
pub fn looks_encrypted(text: &str) -> bool {
    let b = text.as_bytes();
    if b.len() < 12 {
        return false;
    }
    // Todo hexadecimal de longitud par.
    if b.len() % 2 == 0 && b.iter().all(|c| c.is_ascii_hexdigit()) {
        return true;
    }
    // Base64 con longitud múltiplo de 4.
    if b.len() % 4 == 0
        && b.iter()
            .all(|c| c.is_ascii_alphanumeric() || *c == b'+' || *c == b'/' || *c == b'=')
    {
        return true;
    }
    // Entropía alta con poca alfabetización.
    let alpha = b.iter().filter(|c| c.is_ascii_alphanumeric()).count() as f64 / b.len() as f64;
    shannon_entropy(text) > 4.2 && alpha > 0.8
}

/// Un punto donde se descifra o se carga algo.
#[derive(Debug, Clone)]
pub struct DecryptPoint {
    /// Clase donde ocurre.
    pub class_name: String,
    /// Método donde ocurre.
    pub method_name: String,
    /// Tipo de punto (`string-decryptor`, `dynamic-loader`).
    pub kind: String,
    /// La constante sospechosa.
    pub constant: String,
    /// Confianza 0-100.
    pub confidence: u8,
    /// Fragmento desensamblado (evidencia).
    pub fragment: String,
}

/// Renderiza un fragmento de desensamblado alrededor de un offset.
pub fn render_fragment(
    dex: &DexFile,
    insns: &[u16],
    center: u32,
    before: usize,
    after: usize,
) -> String {
    let all = disassemble(dex, insns);
    let center_idx = all.iter().position(|i| i.offset == center).unwrap_or(0);
    let start = center_idx.saturating_sub(before);
    let end = (center_idx + after + 1).min(all.len());
    let mut out = String::new();
    for ins in &all[start..end] {
        if !out.is_empty() {
            out.push_str("\n            ");
        }
        out.push_str(&format!("0x{:04x}: {}", ins.offset, ins.mnemonic));
        if let Some(midx) = ins.method_idx {
            if let Some(m) = dex.methods.get(midx as usize) {
                out.push_str(&format!(" {}.{}", m.class_name, m.name));
                continue;
            }
        }
        if let Some(sidx) = ins.string_idx {
            if let Some(s) = dex.strings.get(sidx as usize) {
                out.push_str(&format!(" \"{}\"", truncate(s, 60)));
                continue;
            }
        }
        if let Some(tidx) = ins.type_idx {
            if let Some(t) = dex.types.get(tidx as usize) {
                out.push_str(&format!(" {}", t));
                continue;
            }
        }
        if let Some(v) = ins.int_const {
            out.push_str(&format!(" #{}", v));
        }
    }
    out
}

/// Busca puntos de descifrado y carga dinámica en un DEX.
pub fn find_decrypt_points(_dex: &DexFile, profiles: &[ClassProfile]) -> Vec<DecryptPoint> {
    let mut out = Vec::new();

    for prof in profiles {
        for mp in &prof.methods {
            // (a) Descifrador de strings de la clase.
            if prof.is_string_decryptor {
                for s in &mp.const_strings {
                    if looks_encrypted(s) {
                        out.push(DecryptPoint {
                            class_name: prof.class_name.clone(),
                            method_name: mp.method_name.clone(),
                            kind: "string-decryptor".to_string(),
                            constant: truncate(s, 160),
                            confidence: 86,
                            fragment: format!(
                                "clase {} -> metodo {} -> descifra un literal de {} bytes (entropia {:.2})",
                                prof.class_name,
                                mp.method_name,
                                s.len(),
                                shannon_entropy(s)
                            ),
                        });
                    }
                }
            }

            // (b) Carga dinamica de codigo con constante.
            for api in &mp.apis {
                let is_loader = api.starts_with("Ldalvik/system/DexClassLoader;")
                    || api.starts_with("Ldalvik/system/PathClassLoader;")
                    || api.starts_with("Ldalvik/system/InMemoryDexClassLoader;");
                if !is_loader {
                    continue;
                }
                for s in &mp.const_strings {
                    if s.len() >= 4 {
                        out.push(DecryptPoint {
                            class_name: prof.class_name.clone(),
                            method_name: mp.method_name.clone(),
                            kind: "dynamic-loader".to_string(),
                            constant: truncate(s, 160),
                            confidence: 82,
                            fragment: format!(
                                "clase {} -> metodo {} -> invoca {} con la constante \"{}\"",
                                prof.class_name,
                                mp.method_name,
                                api.split_once("->").map(|x| x.0).unwrap_or(api),
                                truncate(s, 80)
                            ),
                        });
                    }
                }
            }
        }
    }

    // De-duplicar por (clase, metodo, tipo, constante).
    let mut seen = HashSet::new();
    out.retain(|d| {
        seen.insert(format!(
            "{}|{}|{}|{}",
            d.class_name, d.method_name, d.kind, d.constant
        ))
    });
    out.truncate(200);
    out
}

// ========================================================================= //
// Grafo de llamadas (xrefs)
// ========================================================================= //

/// El grafo de llamadas del APK.
#[derive(Debug, Clone, Default)]
pub struct CallGraph {
    /// Numero total de invocaciones.
    pub total_calls: usize,
    /// Clases que mas llaman (descripcion -> numero).
    pub top_callers: Vec<(String, usize)>,
    /// Clases mas llamado (descripcion -> numero).
    pub top_callees: Vec<(String, usize)>,
    /// Aristas: (clase llamadora, clase llamada).
    pub edges: Vec<(String, String)>,
}

/// Construye el grafo de llamadas desde los perfiles.
pub fn build_call_graph(profiles: &[ClassProfile]) -> CallGraph {
    let mut graph = CallGraph::default();
    let mut out_deg: BTreeMap<String, usize> = BTreeMap::new();
    let mut in_deg: BTreeMap<String, usize> = BTreeMap::new();
    let mut edges: BTreeSet<(String, String)> = BTreeSet::new();

    for prof in profiles {
        for mp in &prof.methods {
            for api in &mp.apis {
                let Some((cls, _)) = api.split_once("->") else {
                    continue;
                };
                if cls.is_empty() {
                    continue;
                }
                graph.total_calls += 1;
                *out_deg.entry(prof.class_name.clone()).or_insert(0) += 1;
                *in_deg.entry(cls.to_string()).or_insert(0) += 1;
                edges.insert((prof.class_name.clone(), cls.to_string()));
            }
        }
    }
    graph.edges = edges.into_iter().collect();
    let take_top = |m: BTreeMap<String, usize>| -> Vec<(String, usize)> {
        let mut v: Vec<(String, usize)> = m.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v.truncate(20);
        v
    };
    graph.top_callers = take_top(out_deg);
    graph.top_callees = take_top(in_deg);
    graph
}

// ========================================================================= //
// Pipeline principal
// ========================================================================= //

/// Analiza un APK completo y devuelve el informe.
///
/// Nunca hace panic: cualquier fallo se devuelve como `{ok:false, error}`.
pub fn scan(apk_path: &str) -> Value {
    if apk_path.is_empty() {
        return json!({
            "ok": false,
            "error": "empty_path",
            "hint": "la ruta del APK no puede estar vacia",
        });
    }
    let bundle = match open_bundle(apk_path) {
        Ok(b) => b,
        Err(e) => {
            return json!({
                "ok": false,
                "error": "cannot_open_apk",
                "path": apk_path,
                "detail": e,
            })
        }
    };

    let mut findings: Vec<Finding> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // ------------------------------------------------------------------ //
    // 1. Manifiesto
    // ------------------------------------------------------------------ //
    let manifest = format_oracle::parse_axml(&bundle.manifest_bytes);
    for e in &manifest.errors {
        warnings.push(format!("manifiesto: {}", e));
    }
    let package = manifest.package.clone().unwrap_or_default();
    let min_sdk = manifest.min_sdk;
    let target_sdk = manifest.target_sdk;

    // Huella canonica del APK (estable ante reordenamientos).
    let mut fp_input = String::new();
    fp_input.push_str(&format!(
        "pkg={}\nvc={}\nvsdk={}\ntsdk={}\n",
        package,
        manifest.version_code.unwrap_or(-1),
        min_sdk.unwrap_or(-1),
        target_sdk.unwrap_or(-1)
    ));
    let mut comp_names: Vec<String> = manifest
        .components
        .iter()
        .map(|c| format!("{}:{}", c.kind, c.name))
        .collect();
    comp_names.sort();
    fp_input.push_str(&comp_names.join("\n"));
    let fingerprint = sha256_hex(fp_input.as_bytes());

    // ------------------------------------------------------------------ //
    // 2. DEX
    // ------------------------------------------------------------------ //
    // Los perfiles NO se acumulan: se procesan DEX a DEX y se descartan.
    // (en un APK de 15 MB con 2 DEX, retenerlos todos revienta la RAM).
    let mut all_class_names: Vec<String> = Vec::new();
    let mut all_strings: HashSet<String> = HashSet::new();
    let mut dex_report: Vec<Value> = Vec::new();
    let mut decrypt_points: Vec<DecryptPoint> = Vec::new();
    let mut renames: Vec<Value> = Vec::new();
    let mut applied = 0usize;
    let mut kept = 0usize;
    let mut total_classes = 0usize;
    let mut total_methods = 0usize;
    let mut total_strings = 0usize;
    // Estadisticas para la deteccion estructural de ProGuard/R8.
    let mut pg_total = 0usize;
    let mut pg_short = 0usize;
    let mut pg_with_source = 0usize;
    let mut pg_r8_ids = 0usize;
    // Grafo de llamadas acumulado entre DEX.
    let mut graph_total_calls = 0usize;
    let mut out_deg: BTreeMap<String, usize> = BTreeMap::new();
    let mut in_deg: BTreeMap<String, usize> = BTreeMap::new();
    let mut graph_edges: BTreeSet<(String, String)> = BTreeSet::new();

    for (name, bytes) in &bundle.dex_files {
        let dex = match DexFile::parse(bytes) {
            Ok(d) => d,
            Err(e) => {
                warnings.push(format!("{}: {}", name, e));
                continue;
            }
        };
        let profiles: Vec<ClassProfile> =
            dex.classes.iter().map(|c| profile_class(&dex, c)).collect();
        total_classes += dex.classes.len();
        total_methods += dex.classes.iter().map(|c| c.methods.len()).sum::<usize>();
        total_strings += dex.strings.len();

        for p in &profiles {
            all_class_names.push(p.class_name.clone());
        }
        for s in &dex.strings {
            if all_strings.len() < 40_000 {
                all_strings.insert(s.clone());
            }
        }
        decrypt_points.extend(find_decrypt_points(&dex, &profiles));

        for c in &dex.classes {
            pg_total += 1;
            if class_looks_obfuscated(&c.class_name) {
                pg_short += 1;
            }
            if !c.source_file.is_empty() {
                pg_with_source += 1;
            }
            if c.source_file.starts_with("r8-map-id-") {
                pg_r8_ids += 1;
            }
        }

        // Renombrado heuristico + hallazgos por clase + grafo: se hacen aqui
        // y los perfiles se sueltan al terminar el DEX.
        for prof in &profiles {
            if let Some((role, confidence, evidence)) = classify_class(prof) {
                let keep_original = confidence < RENAME_CONFIDENCE_THRESHOLD;
                if keep_original {
                    kept += 1;
                } else {
                    applied += 1;
                }
                renames.push(json!({
                    "kind": "class",
                    "obfuscated": prof.class_name,
                    "suggested": if keep_original { prof.simple_name.clone() } else { role.clone() },
                    "role": role,
                    "confidence": confidence,
                    "applied": !keep_original,
                    "reason": if keep_original {
                        format!(
                            "confianza {} < {}: se conserva el nombre original",
                            confidence, RENAME_CONFIDENCE_THRESHOLD
                        )
                    } else {
                        "patron de uso inequivoco".to_string()
                    },
                    "evidence": evidence.iter().take(8).collect::<Vec<_>>(),
                    "class_already_readable": !class_looks_obfuscated(&prof.class_name),
                }));
            }

            for mp in &prof.methods {
                if !looks_obfuscated(&mp.method_name) {
                    continue;
                }
                if let Some((suggested, confidence, evidence)) = classify_method(prof, mp) {
                    let keep_original = confidence < RENAME_CONFIDENCE_THRESHOLD;
                    renames.push(json!({
                        "kind": "method",
                        "obfuscated": format!("{}->{}", prof.class_name, mp.method_name),
                        "suggested": if keep_original { mp.method_name.clone() } else { suggested.clone() },
                        "role": suggested,
                        "confidence": confidence,
                        "applied": !keep_original,
                        "reason": if keep_original {
                            format!(
                                "confianza {} < {}: se conserva el nombre original",
                                confidence, RENAME_CONFIDENCE_THRESHOLD
                            )
                        } else {
                            "API llamada inequivoca".to_string()
                        },
                        "evidence": evidence,
                    }));
                }
            }

            // Hallazgos por clase.
            let bridge = prof.methods.iter().find(|m| {
                m.apis
                    .iter()
                    .any(|a| a == "Landroid/webkit/WebView;->addJavascriptInterface")
            });
            if let Some(m) = bridge {
                findings.push(Finding {
                    id: "WEBVIEW_JS_BRIDGE".to_string(),
                    severity: "high".to_string(),
                    title: "WebView con puente JavaScript".to_string(),
                    detail: "La app registra un objeto Java en el WebView: si carga contenido remoto, el JS puede invocar los metodos `@JavascriptInterface`.".to_string(),
                    evidence: format!(
                        "clase {} -> metodo {} -> WebView.addJavascriptInterface(...)",
                        prof.class_name, m.method_name
                    ),
                    cwe: Some("CWE-749".to_string()),
                });
            }
            if prof.loads_dynamic_code {
                let loader = prof.methods.iter().find(|m| {
                    m.apis.iter().any(|a| {
                        a.starts_with("Ldalvik/system/DexClassLoader;")
                            || a.starts_with("Ldalvik/system/PathClassLoader;")
                            || a.starts_with("Ldalvik/system/InMemoryDexClassLoader;")
                    })
                });
                findings.push(Finding {
                    id: "DYNAMIC_CODE_LOAD".to_string(),
                    severity: "high".to_string(),
                    title: "Carga de codigo en tiempo de ejecucion".to_string(),
                    detail: "La app carga DEX u otro codigo dinamicamente: el codigo ejecutado puede no estar en el APK.".to_string(),
                    evidence: format!(
                        "clase {} -> metodo {} -> DexClassLoader/PathClassLoader",
                        prof.class_name,
                        loader.map(|m| m.method_name.clone()).unwrap_or_default()
                    ),
                    cwe: Some("CWE-494".to_string()),
                });
            }
            for m in &prof.methods {
                for s in &m.const_strings {
                    if s == "MODE_WORLD_READABLE"
                        || s == "MODE_WORLD_WRITEABLE"
                        || s == "MODE_WORLD_WRITABLE"
                    {
                        findings.push(Finding {
                            id: "WORLD_READABLE_PREFS".to_string(),
                            severity: "high".to_string(),
                            title: "SharedPreferences en modo global".to_string(),
                            detail: "Las preferencias se crean en un modo legible/escribible por cualquier app.".to_string(),
                            evidence: format!(
                                "clase {} -> metodo {} -> SharedPreferences.getSharedPreferences(..., \"{}\")",
                                prof.class_name, m.method_name, s
                            ),
                            cwe: Some("CWE-732".to_string()),
                        });
                    }
                }
            }

            // Grafo de llamadas (acumulado entre DEX).
            for mp in &prof.methods {
                for api in &mp.apis {
                    let Some((cls, _)) = api.split_once("->") else {
                        continue;
                    };
                    if cls.is_empty() {
                        continue;
                    }
                    graph_total_calls += 1;
                    *out_deg.entry(prof.class_name.clone()).or_insert(0) += 1;
                    *in_deg.entry(cls.to_string()).or_insert(0) += 1;
                    graph_edges.insert((prof.class_name.clone(), cls.to_string()));
                }
            }
        }


        dex_report.push(json!({
            "name": name,
            "sha256": dex.sha256,
            "version": dex.version,
            "classes": dex.classes.len(),
            "methods": dex.classes.iter().map(|c| c.methods.len()).sum::<usize>(),
            "strings": dex.strings.len(),
            "warnings": dex.warnings,
        }));

    }

    // ------------------------------------------------------------------ //
    // 3. Firmas (ofuscador / empaquetador / framework de parcheo)
    // ------------------------------------------------------------------ //
    let entry_names: Vec<String> = bundle.entries.iter().map(|e| e.name.clone()).collect();
    let native_names: Vec<String> = bundle.native_libs.iter().map(|(n, _)| n.clone()).collect();
    let mut native_strings: Vec<String> = Vec::new();
    let mut native_infos: Vec<NativeLibInfo> = Vec::new();
    for (path, bytes) in &bundle.native_libs {
        let info = analyze_native(path, bytes);
        native_strings.extend(format_oracle::extract_strings(bytes, 6));
        native_infos.push(info);
    }
    native_strings.truncate(4000);

    let mut signatures = detect_signatures(
        &entry_names,
        &all_strings,
        &all_class_names,
        &native_names,
        &native_strings,
    );
    // ProGuard/R8 se detecta por estructura, no por marcador fijo.
    if let Some(hit) =
        detect_proguard_like_stats_full(pg_total, pg_short, pg_with_source, pg_r8_ids)
    {
        signatures.insert(0, hit);
    }

    let pick = |cat: &str| -> Option<Value> {
        signatures
            .iter()
            .find(|s| s.category == cat)
            .map(|s| {
                json!({
                    "name": s.name,
                    "vendor": s.vendor,
                    "confidence": s.confidence,
                    "evidence": s.evidence,
                    "note": s.note,
                })
            })
    };
    let obfuscator = pick("obfuscator");
    let packer = pick("packer").or_else(|| pick("protector"));
    let patch_framework = pick("patch-framework");

    // ------------------------------------------------------------------ //
    // 4. Componentes exportados
    // ------------------------------------------------------------------ //
    let mut exported: Vec<Value> = Vec::new();
    for c in &manifest.components {
        if !c.effective_exported(target_sdk) {
            continue;
        }
        let actions: Vec<String> = c
            .intent_filters
            .iter()
            .flat_map(|f| f.actions.clone())
            .collect();
        exported.push(json!({
            "kind": c.kind,
            "name": c.name,
            "exported": true,
            "declared_exported": c.exported,
            "permission": c.permission,
            "permission_group": c.permission_group,
            "read_permission": c.read_permission,
            "write_permission": c.write_permission,
            "protected": c.is_permission_protected(),
            "has_intent_filter": !c.intent_filters.is_empty(),
            "actions": actions,
            "authorities": c.authorities,
        }));

        if !c.is_permission_protected() {
            findings.push(Finding {
                id: "EXPORTED_NO_PERMISSION".to_string(),
                severity: if c.kind == "provider" {
                    "high"
                } else if !c.intent_filters.is_empty() {
                    "medium"
                } else {
                    "low"
                }
                .to_string(),
                title: format!("{} exportado sin permiso", c.kind),
                detail: format!(
                    "`{}` se exporta y no exige ningun permiso: cualquier app instalada puede abrirlo.",
                    c.name
                ),
                evidence: format!(
                    "AndroidManifest.xml -> <{} android:name=\"{}\"> (exportado={}, permiso=none)",
                    c.kind,
                    c.name,
                    c.exported
                        .map(|b| b.to_string())
                        .unwrap_or_else(|| "implicito".into())
                ),
                cwe: Some("CWE-926".to_string()),
            });
        }
    }

    // ------------------------------------------------------------------ //
    // 5. Endpoints
    // ------------------------------------------------------------------ //
    let mut endpoints: Vec<String> = Vec::new();
    let mut cleartext = 0usize;
    for s in all_strings.iter().chain(native_strings.iter()) {
        for u in extract_endpoints(s) {
            if endpoints.len() >= MAX_ENDPOINTS || endpoints.contains(&u) {
                continue;
            }
            if u.starts_with("http://") {
                cleartext += 1;
            }
            endpoints.push(u);
        }
    }

    // ------------------------------------------------------------------ //
    // 7. Hallazgos
    // ------------------------------------------------------------------ //
    if manifest.application.debuggable == Some(true) {
        findings.push(Finding {
            id: "DEBUGGABLE".to_string(),
            severity: "high".to_string(),
            title: "android:debuggable=true".to_string(),
            detail: "La app se depura: cualquier usuario puede adjuntar un depurador al proceso.".to_string(),
            evidence: "AndroidManifest.xml -> <application android:debuggable=\"true\">".to_string(),
            cwe: Some("CWE-489".to_string()),
        });
    }
    if manifest.application.allow_backup == Some(true) {
        findings.push(Finding {
            id: "ALLOW_BACKUP".to_string(),
            severity: "medium".to_string(),
            title: "android:allowBackup=true".to_string(),
            detail: "El `adb backup` puede extraer los datos privados de la app.".to_string(),
            evidence: "AndroidManifest.xml -> <application android:allowBackup=\"true\">".to_string(),
            cwe: Some("CWE-530".to_string()),
        });
    }
    if manifest.application.uses_cleartext_traffic == Some(true) {
        findings.push(Finding {
            id: "CLEARTEXT_TRAFFIC".to_string(),
            severity: "medium".to_string(),
            title: "android:usesCleartextTraffic=true".to_string(),
            detail: "Se permite trafico HTTP en claro.".to_string(),
            evidence: "AndroidManifest.xml -> <application android:usesCleartextTraffic=\"true\">".to_string(),
            cwe: Some("CWE-319".to_string()),
        });
    }
    if cleartext > 0 {
        findings.push(Finding {
            id: "CLEARTEXT_ENDPOINTS".to_string(),
            severity: "low".to_string(),
            title: format!("{} endpoints HTTP en claro", cleartext),
            detail: "Hay URLs `http://` hardcodeadas: el trafico puede interceptarse.".to_string(),
            evidence: endpoints
                .iter()
                .filter(|u| u.starts_with("http://"))
                .take(6)
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
            cwe: Some("CWE-319".to_string()),
        });
    }
    if manifest.permissions.iter().any(|p| p.contains("QUERY_ALL_PACKAGES")) {
        findings.push(Finding {
            id: "QUERY_ALL_PACKAGES".to_string(),
            severity: "medium".to_string(),
            title: "QUERY_ALL_PACKAGES".to_string(),
            detail: "La app puede enumerar todas las apps instaladas (reconocimiento del dispositivo).".to_string(),
            evidence: format!(
                "AndroidManifest.xml -> <uses-permission android:name=\"{}\">",
                manifest
                    .permissions
                    .iter()
                    .find(|p| p.contains("QUERY_ALL_PACKAGES"))
                    .cloned()
                    .unwrap_or_default()
            ),
            cwe: Some("CWE-200".to_string()),
        });
    }

    // Nativas sospechosas: se agrupa por nombre de biblioteca (no por ABI),
    // para no contar 4 veces la misma .so (arm64, armv7, x86, x86_64).
    let mut natives_reported: HashSet<String> = HashSet::new();
    for n in &native_infos {
        if natives_reported.contains(&n.name) {
            continue;
        }
        if n.suspicious.iter().any(|s| s == "system" || s == "execve" || s == "popen") {
            natives_reported.insert(n.name.clone());
            findings.push(Finding {
                id: "NATIVE_COMMAND_EXEC".to_string(),
                severity: "high".to_string(),
                title: "La biblioteca nativa puede ejecutar comandos".to_string(),
                detail: format!(
                    "La `.so` contiene referencias a `{}`: puede lanzar procesos del sistema.",
                    n.suspicious.join(", ")
                ),
                evidence: format!("{} -> {}", n.path, n.suspicious.join(", ")),
                cwe: Some("CWE-78".to_string()),
            });
        }
        if !n.framework_hits.is_empty() && !natives_reported.contains(&n.name) {
            natives_reported.insert(n.name.clone());
            findings.push(Finding {
                id: "NATIVE_HOOK_FRAMEWORK".to_string(),
                severity: "medium".to_string(),
                title: "Indicadores de framework de instrumentacion".to_string(),
                detail: "La `.so` contiene cadenas de un framework de hooking: la app puede estar instrumentada.".to_string(),
                evidence: format!("{} -> {}", n.path, n.framework_hits.join(" | ")),
                cwe: None,
            });
        }
    }

    // Framework de parcheo detectado.
    if let Some(pf) = &patch_framework {
        findings.push(Finding {
            id: "PATCH_FRAMEWORK_PRESENT".to_string(),
            severity: "high".to_string(),
            title: format!("La app esta parcheada ({})", pf["name"].as_str().unwrap_or("?")),
            detail: "Se detecta un framework de parcheo/hooking dentro del APK: el binario instalado difiere del original y puede inyectar codigo en otras apps.".to_string(),
            evidence: pf["evidence"].to_string(),
            cwe: None,
        });
    }

    // Limitar el ruido: maximo 4 hallazgos por tipo (el resto se cuenta).
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut kept_findings: Vec<Finding> = Vec::new();
    for f in findings.iter() {
        let n = counts.entry(f.id.clone()).or_insert(0);
        *n += 1;
        if *n <= 4 {
            kept_findings.push(f.clone());
        }
    }
    let suppressed: usize = findings.len() - kept_findings.len();
    let findings = kept_findings;

    let risk = compute_risk(&findings);
    renames.truncate(400);
    let take_top = |m: BTreeMap<String, usize>| -> Vec<(String, usize)> {
        let mut v: Vec<(String, usize)> = m.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v.truncate(20);
        v
    };
    let graph = CallGraph {
        total_calls: graph_total_calls,
        top_callers: take_top(out_deg),
        top_callees: take_top(in_deg),
        edges: graph_edges.into_iter().collect(),
    };

    // ------------------------------------------------------------------ //
    // 8. Informe
    // ------------------------------------------------------------------ //
    json!({
        "ok": true,
        "path": apk_path,
        "size": bundle.file_size,
        "sha256": bundle.sha256,
        "package": package,
        "sdk": {
            "min": min_sdk,
            "target": target_sdk,
            "version_code": manifest.version_code,
            "version_name": manifest.version_name,
        },
        "fingerprint": fingerprint,
        "obfuscator": obfuscator,
        "packer": packer,
        "patch_framework": patch_framework,
        "permissions": manifest.permissions,
        "endpoints": endpoints,
        "native_libs": native_infos.iter().map(|n| json!({
            "path": n.path,
            "name": n.name,
            "abi": n.abi,
            "size": n.size,
            "sha256": n.sha256,
            "is_elf": n.is_elf,
            "stripped": n.stripped,
            "machine": n.machine,
            "type": n.elf_type,
            "jni_onload": n.jni_onload,
            "exported": n.exports,
            "imported": n.imports,
            "needed": n.needed,
            "hook_surface": n.hook_surface,
            "framework_hits": n.framework_hits,
            "suspicious": n.suspicious,
        })).collect::<Vec<_>>(),
        "exported_components": exported,
        "findings": findings.iter().map(|f| f.to_json()).collect::<Vec<_>>(),
        "risk": risk,
        "signatures": signatures.iter().map(|s| json!({
            "name": s.name,
            "vendor": s.vendor,
            "category": s.category,
            "confidence": s.confidence,
            "evidence": s.evidence,
            "note": s.note,
        })).collect::<Vec<_>>(),
        "renames": renames,
        "rename_summary": {
            "applied": applied,
            "kept_original": kept,
            "threshold": RENAME_CONFIDENCE_THRESHOLD,
        },
        "decrypt_points": decrypt_points.iter().map(|d| json!({
            "class": d.class_name,
            "method": d.method_name,
            "kind": d.kind,
            "constant": d.constant,
            "confidence": d.confidence,
            "evidence": d.fragment,
        })).collect::<Vec<_>>(),
        "call_graph": {
            "total_calls": graph.total_calls,
            "top_callers": graph.top_callers.iter().map(|(k, v)| json!({"class": k, "calls": v})).collect::<Vec<_>>(),
            "top_callees": graph.top_callees.iter().map(|(k, v)| json!({"class": k, "calls": v})).collect::<Vec<_>>(),
        },
        "dex": dex_report,
        "stats": {
            "dex_count": bundle.dex_files.len(),
            "classes": total_classes,
            "methods": total_methods,
            "strings": total_strings,
            "native_libs": bundle.native_libs.len(),
            "assets": bundle.assets.len(),
            "entries": bundle.entries.len(),
            "endpoints": endpoints.len(),
            "findings": findings.len(),
        },
        "manifest": {
            "package": manifest.package,
            "application": manifest.application.name,
            "features": manifest.features,
            "meta_data": manifest.meta_data.iter().map(|(k, v)| json!({"name": k, "value": v})).collect::<Vec<_>>(),
            "debuggable": manifest.application.debuggable,
            "allow_backup": manifest.application.allow_backup,
            "uses_cleartext_traffic": manifest.application.uses_cleartext_traffic,
        },
        "warnings": warnings,
        "findings_suppressed": suppressed,
        "engine": "megadaemon-re/apk-deobfuscator-v1",
    })
}

/// Analiza un APK y devuelve solo el resumen (para listados rapidos).
pub fn scan_summary(apk_path: &str) -> Value {
    let full = scan(apk_path);
    if full["ok"] != true {
        return full;
    }
    json!({
        "ok": true,
        "package": full["package"],
        "sdk": full["sdk"],
        "obfuscator": full["obfuscator"],
        "packer": full["packer"],
        "patch_framework": full["patch_framework"],
        "risk": full["risk"],
        "findings": full["findings"].as_array().map(|a| a.len()).unwrap_or(0),
    })
}

// ========================================================================= //
// Tests
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // ------------------------------------------------------------------ //
    // Constructor de un DEX minimo pero VALIDO
    // ------------------------------------------------------------------ //

    fn uleb(mut v: u32) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut byte = (v & 0x7f) as u8;
            v >>= 7;
            if v != 0 {
                byte |= 0x80;
            }
            out.push(byte);
            if v == 0 {
                break;
            }
        }
        out
    }

    /// DEX minimo: 1 clase (`Lcom/demo/Foo;`) con 1 metodo (`bar`).
    ///
    /// Cabecera de 112 B + tablas + `class_data` + `code_item`.
    fn build_min_dex() -> Vec<u8> {
        const HEADER: usize = 112;
        let string_ids_off = HEADER;
        let type_ids_off = string_ids_off + 4 * 4;
        let proto_ids_off = type_ids_off + 4 * 2;
        let method_ids_off = proto_ids_off + 12;
        let class_defs_off = method_ids_off + 8;
        let string_data_off = class_defs_off + 32;

        // 4 strings: clase, "V", "bar", "Foo.java"
        let mut string_data = Vec::new();
        let mut offsets = Vec::new();
        for s in ["Lcom/demo/Foo;", "V", "bar", "Foo.java"] {
            offsets.push(string_data_off + string_data.len());
            string_data.push(s.len() as u8);
            string_data.extend_from_slice(s.as_bytes());
            string_data.push(0);
        }
        let class_data_off = string_data_off + string_data.len();

        // class_data_item: 0 static, 0 instance, 1 direct, 0 virtual
        let mut class_data = Vec::new();
        class_data.extend(uleb(0));
        class_data.extend(uleb(0));
        class_data.extend(uleb(1));
        class_data.extend(uleb(0));
        // encoded_method: method_idx_diff=0, access_flags=1 (public), code_off
        // El code_item empieza tras los 4 uleb de tamaño, el method_idx_diff
        // (1 B), el access_flags (1 B) y el code_off.  El code_off se
        // escribe SIEMPRE en 5 bytes (uleb128 con relleno) para que su
        // longitud sea conocida de antemano y el offset sea calculable.
        let code_off = class_data_off + class_data.len() + 7;
        class_data.extend(uleb(0));
        class_data.extend(uleb(1));
        // code_off en 5 bytes fijos (uleb128 con relleno: 7 bits por byte).
        // OJO: en uleb128 el PRIMER byte lleva los bits BAJOS.
        let v = code_off as u32;
        class_data.extend_from_slice(&[
            (0x80 | (v & 0x7f)) as u8,
            (0x80 | ((v >> 7) & 0x7f)) as u8,
            (0x80 | ((v >> 14) & 0x7f)) as u8,
            (0x80 | ((v >> 21) & 0x7f)) as u8,
            ((v >> 28) & 0x7f) as u8,
        ]);

        // code_item: 2 unidades -> const/4 v0,#0 ; return-void
        let mut code = Vec::new();
        code.extend_from_slice(&2u16.to_le_bytes()); // registers_size
        code.extend_from_slice(&1u16.to_le_bytes()); // ins_size
        code.extend_from_slice(&0u16.to_le_bytes()); // outs_size
        code.extend_from_slice(&0u16.to_le_bytes()); // tries_size
        code.extend_from_slice(&0u32.to_le_bytes()); // debug_info_off
        code.extend_from_slice(&2u32.to_le_bytes()); // insns_size
        code.extend_from_slice(&0x0012u16.to_le_bytes()); // const/4 v0, #0
        code.extend_from_slice(&0x000eu16.to_le_bytes()); // return-void

        let mut d = vec![0u8; HEADER];
        d[0..8].copy_from_slice(b"dex\n035\x00");
        d[36..40].copy_from_slice(&(HEADER as u32).to_le_bytes()); // header_size
        d[40..44].copy_from_slice(&0x1234_5678u32.to_le_bytes()); // endian_tag
        d[56..60].copy_from_slice(&4u32.to_le_bytes()); // string_ids_size (4 strings)
        d[60..64].copy_from_slice(&(string_ids_off as u32).to_le_bytes());
        d[64..68].copy_from_slice(&2u32.to_le_bytes()); // type_ids_size
        d[68..72].copy_from_slice(&(type_ids_off as u32).to_le_bytes());
        d[72..76].copy_from_slice(&1u32.to_le_bytes()); // proto_ids_size
        d[76..80].copy_from_slice(&(proto_ids_off as u32).to_le_bytes());
        d[80..84].copy_from_slice(&0u32.to_le_bytes()); // field_ids_size
        d[88..92].copy_from_slice(&1u32.to_le_bytes()); // method_ids_size
        d[92..96].copy_from_slice(&(method_ids_off as u32).to_le_bytes());
        d[96..100].copy_from_slice(&1u32.to_le_bytes()); // class_defs_size
        d[100..104].copy_from_slice(&(class_defs_off as u32).to_le_bytes());

        // string_ids -> offsets
        for o in &offsets {
            d.extend_from_slice(&(*o as u32).to_le_bytes());
        }
        // type_ids -> string indices
        d.extend_from_slice(&0u32.to_le_bytes());
        d.extend_from_slice(&1u32.to_le_bytes());
        // proto_ids
        d.extend_from_slice(&1u32.to_le_bytes()); // shorty_idx
        d.extend_from_slice(&1u32.to_le_bytes()); // return_type_idx (V)
        d.extend_from_slice(&0u32.to_le_bytes()); // parameters_off
        // method_ids
        d.extend_from_slice(&0u16.to_le_bytes()); // class_idx
        d.extend_from_slice(&0u16.to_le_bytes()); // proto_idx
        d.extend_from_slice(&2u32.to_le_bytes()); // name_idx ("bar")
        // class_def
        d.extend_from_slice(&0u32.to_le_bytes()); // class_idx
        d.extend_from_slice(&1u32.to_le_bytes()); // access_flags
        d.extend_from_slice(&0u32.to_le_bytes()); // superclass_idx
        d.extend_from_slice(&0u32.to_le_bytes()); // interfaces_off
        d.extend_from_slice(&3u32.to_le_bytes()); // source_file_idx
        d.extend_from_slice(&0u32.to_le_bytes()); // annotations_off
        d.extend_from_slice(&(class_data_off as u32).to_le_bytes());
        d.extend_from_slice(&0u32.to_le_bytes()); // static_values_off

        d.extend_from_slice(&string_data);
        d.extend_from_slice(&class_data);
        d.extend_from_slice(&code);
        let total = d.len() as u32;
        d[32..36].copy_from_slice(&total.to_le_bytes()); // file_size
        d
    }

    /// APK sintetico: manifiesto + classes.dex + una `.so` falsa.
    fn build_test_apk(path: &str) {
        let manifest = crate::format_oracle::testutil::build_axml(&[
            (
                "manifest",
                vec![("package", false, "com.demo.app"), ("versionCode", false, "7")],
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
            ("application", vec![("debuggable", true, "true")]),
        ]);
        let dex = build_min_dex();

        let file = File::create(path).expect("no se pudo crear el APK de test");
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("AndroidManifest.xml", opts).unwrap();
        zip.write_all(&manifest).unwrap();
        zip.start_file("classes.dex", opts).unwrap();
        zip.write_all(&dex).unwrap();
        zip.start_file("lib/arm64-v8a/libdemo.so", opts).unwrap();
        zip.write_all(b"\x7fELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x02\x00\x3e\x00")
            .unwrap();
        zip.finish().unwrap();
    }

    fn tmp_path(name: &str) -> String {
        let mut p = std::env::temp_dir();
        p.push(format!("megadaemon-re-test-{}-{}", std::process::id(), name));
        p.to_string_lossy().to_string()
    }

    // ------------------------------------------------------------------ //
    // Tests
    // ------------------------------------------------------------------ //

    #[test]
    fn una_ruta_inexistente_no_entra_en_panic() {
        let r = scan("/no/existe/este/apk.apk");
        assert_eq!(r["ok"], false);
        assert_eq!(r["error"], "cannot_open_apk");
    }

    #[test]
    fn una_ruta_vacia_no_entra_en_panic() {
        let r = scan("");
        assert_eq!(r["ok"], false);
        assert_eq!(r["error"], "empty_path");
    }

    #[test]
    fn un_fichero_que_no_es_zip_no_entra_en_panic() {
        let path = tmp_path("nozip");
        std::fs::write(&path, b"esto no es un zip").unwrap();
        let r = scan(&path);
        assert_eq!(r["ok"], false);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn el_dex_minimo_se_parsea_bien() {
        let dex = DexFile::parse(&build_min_dex()).expect("debería parsear");
        assert_eq!(dex.version, "035");
        assert_eq!(dex.classes.len(), 1);
        assert_eq!(dex.classes[0].class_name, "Lcom/demo/Foo;");
        assert_eq!(dex.classes[0].methods.len(), 1);
        let m = &dex.methods[dex.classes[0].methods[0].method_idx as usize];
        assert_eq!(m.name, "bar");
        assert_eq!(m.class_name, "Lcom/demo/Foo;");
        assert!(dex.strings.contains(&"Lcom/demo/Foo;".to_string()));
    }

    #[test]
    fn el_dex_minimo_se_desensambla() {
        let dex = DexFile::parse(&build_min_dex()).unwrap();
        let insns = &dex.classes[0].methods[0].insns;
        let d = disassemble(&dex, insns);
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].mnemonic, "const/4");
        assert_eq!(d[0].int_const, Some(0));
        assert_eq!(d[1].mnemonic, "return-void");
    }

    #[test]
    fn goto_ocupa_dos_unidades() {
        // `goto` es 10t: opcode + offset. Con 1 el desensamblado se desalinea.
        assert_eq!(instruction_width(0x28), 2);
        assert_eq!(instruction_width(0x0e), 1);
        assert_eq!(instruction_width(0x1a), 2);
        assert_eq!(instruction_width(0x71), 3);
    }

    #[test]
    fn los_payloads_no_desalinean_el_desensamblado() {
        // packed-switch-payload: ident 0x0100, size 1 -> 6 unidades.
        let insns = vec![0x0100u16, 0x0001, 0x0000, 0x0000, 0x0000, 0x0000, 0x000e];
        let dex = DexFile::parse(&build_min_dex()).unwrap();
        let d = disassemble(&dex, &insns);
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].mnemonic, "payload");
        assert_eq!(d[1].mnemonic, "return-void");
    }

    #[test]
    fn decodifica_mutf8_con_nul_modificado() {
        // MUTF-8 codifica el NUL como C0 80.
        let s = decode_mutf8(&[b'a', 0xC0, 0x80, b'b']);
        assert_eq!(s, "a\u{0}b");
    }

    #[test]
    fn el_escaneo_de_un_apk_sintetico_trae_lo_pedido() {
        let path = tmp_path("apk");
        build_test_apk(&path);
        let r = scan(&path);
        assert_eq!(r["ok"], true, "el escaneo falló: {}", r);
        assert_eq!(r["package"], "com.demo.app");
        assert_eq!(r["sdk"]["min"], 24);
        assert_eq!(r["sdk"]["target"], 33);
        assert_eq!(r["sdk"]["version_code"], 7);
        assert_eq!(r["fingerprint"].as_str().unwrap().len(), 64);
        assert_eq!(r["permissions"][0], "android.permission.INTERNET");
        assert_eq!(r["stats"]["dex_count"], 1);
        assert_eq!(r["stats"]["classes"], 1);
        assert_eq!(r["native_libs"].as_array().unwrap().len(), 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn detecta_el_componente_exportado_sin_permiso() {
        let path = tmp_path("apk2");
        build_test_apk(&path);
        let r = scan(&path);
        let exported = r["exported_components"].as_array().unwrap();
        assert_eq!(exported.len(), 1);
        assert_eq!(exported[0]["name"], "com.demo.app.MainActivity");
        assert_eq!(exported[0]["protected"], false);
        let findings = r["findings"].as_array().unwrap();
        assert!(findings.iter().any(|f| f["id"] == "EXPORTED_NO_PERMISSION"));
        assert!(findings.iter().any(|f| f["id"] == "DEBUGGABLE"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn detecta_nombres_ofuscados() {
        assert!(looks_obfuscated("a"));
        assert!(looks_obfuscated("ab"));
        assert!(looks_obfuscated("a05"));
        assert!(!looks_obfuscated("onCreate"));
        assert!(!looks_obfuscated(""));
        assert!(class_looks_obfuscated("La05;"));
        assert!(class_looks_obfuscated("Lza4;"));
        assert!(!class_looks_obfuscated("Lcom/google/gms/ads/AdView;"));
        assert!(!class_looks_obfuscated("Lcom/demo/Foo;"));
    }

    #[test]
    fn extrae_endpoints() {
        let eps = extract_endpoints("ver https://example.com/a?b=1 y http://otro.io/x fin");
        assert!(eps.contains(&"https://example.com/a?b=1".to_string()));
        assert!(eps.contains(&"http://otro.io/x".to_string()));
        assert_eq!(extract_endpoints("nada aqui").len(), 0);
    }

    #[test]
    fn clasifica_una_clase_por_su_patron_de_uso() {
        let mut prof = ClassProfile {
            class_name: "La05;".to_string(),
            simple_name: "a05".to_string(),
            ..Default::default()
        };
        prof.apis
            .insert("Landroid/database/sqlite/SQLiteDatabase;->execSQL".to_string(), 2);
        let (role, conf, ev) = classify_class(&prof).expect("debería clasificar");
        assert_eq!(role, "dbHelper");
        assert!(conf >= RENAME_CONFIDENCE_THRESHOLD);
        assert!(!ev.is_empty());
    }

    #[test]
    fn una_clase_legible_no_se_renombra() {
        // La clase ya tiene un nombre legible: la confianza baja y se conserva.
        let mut prof = ClassProfile {
            class_name: "Lcom/demo/db/MyDatabaseHelper;".to_string(),
            simple_name: "MyDatabaseHelper".to_string(),
            ..Default::default()
        };
        prof.apis
            .insert("Landroid/database/sqlite/SQLiteDatabase;->execSQL".to_string(), 9);
        let (_role, conf, _) = classify_class(&prof).expect("debería clasificar");
        assert!(
            conf < RENAME_CONFIDENCE_THRESHOLD,
            "una clase con nombre legible no debe renombrarse (confianza {})",
            conf
        );
    }

    #[test]
    fn sin_patron_de_uso_no_hay_propuesta() {
        let prof = ClassProfile {
            class_name: "La05;".to_string(),
            simple_name: "a05".to_string(),
            ..Default::default()
        };
        assert!(classify_class(&prof).is_none());
    }

    #[test]
    fn detecta_proguard_por_estructura() {
        let hit = detect_proguard_like_stats(100, 80, 0).expect("debería detectar");
        assert_eq!(hit.name, "ProGuard/R8");
        assert!(hit.confidence >= 60);
        // Con pocas clases cortas, no se debe detectar.
        assert!(detect_proguard_like_stats(100, 10, 90).is_none());
        // El map-id de R8 es prueba definitiva.
        let r8 = detect_proguard_like_stats_full(100, 10, 90, 42).expect("debería detectar");
        assert_eq!(r8.name, "R8");
        assert_eq!(r8.confidence, 99);
    }

    #[test]
    fn el_riesgo_crece_con_los_hallazgos() {
        let bajo = vec![Finding {
            id: "X".to_string(),
            severity: "low".to_string(),
            title: "t".to_string(),
            detail: "d".to_string(),
            evidence: "e".to_string(),
            cwe: None,
        }];
        let alto = vec![Finding {
            id: "Y".to_string(),
            severity: "critical".to_string(),
            title: "t".to_string(),
            detail: "d".to_string(),
            evidence: "e".to_string(),
            cwe: None,
        }];
        assert!(compute_risk(&alto) > compute_risk(&bajo));
        assert_eq!(compute_risk(&[]), 0);
    }

    #[test]
    fn detecta_textos_cifrados() {
        assert!(looks_encrypted("0123456789abcdef0123"));
        assert!(looks_encrypted("aGVsbG8gd29ybGQgdGhlcmU="));
        assert!(!looks_encrypted("Hola mundo normal"));
        assert!(!looks_encrypted("corto"));
    }

    #[test]
    fn el_sha256_es_el_conocido() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
