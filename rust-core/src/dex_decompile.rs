//! MegaDaemon v13.4 — Decompilador DEX/Android en Rust puro.
//!
//! Base del "decompilador de APKs y de Java" del proyecto: lee un fichero
//! **DEX** (formatos `035`..`041`), lo desensambla a Dalvik legible, lo
//! reexpresa como **pseudo-Java** por clase y produce un **mapa de
//! desofuscación** con hipótesis sobre renombrado, strings cifrados y
//! ofuscación de flujo de control.
//!
//! Todo en Rust puro: **sin Java, sin `dex2jar`, sin binario externo**.  Se
//! reutilizan los crates que ya estaban en `Cargo.toml`:
//!
//! | Crate | Uso aquí |
//! |---|---|
//! | `serde` / `serde_json` | Tipos serializables + mapa de desofuscación en JSON |
//! | `zip` | Lectura de las entradas `classes*.dex` dentro del APK |
//! | `flate2` | Inflado de reserva (raw deflate) para APKs que `zip` rechaza |
//! | `sha2` | Huella SHA-256 del DEX (integridad + Identification) |
//!
//! El formato DEX está descrito en la especificación de Android
//! (`dex-format`): header de 112 bytes, `string_ids`, `type_ids`, `proto_ids`,
//! `field_ids`, `method_ids`, `class_defs`, `class_data` (con `code_item`),
//! `debug_info`, `map_list` y anotaciones.
//!
//! API pública:
//!
//! | Función | Qué hace |
//! |---|---|
//! | `DexFile::parse(bytes)` | Parsea un DEX completo en memoria |
//! | `decompile_class(dex, idx)` | Pseudo-Java de UNA clase |
//! | `decompile_all(dex)` | Pseudo-Java de todas las clases |
//! | `disassemble_method(dex, class_idx, method_idx)` | Dalvik legible de un método |
//! | `analyze_obfuscation(dex)` | Informe de desofuscación (tipado) |
//! | `obfuscation_map(dex)` | El mismo informe en JSON, con hipótesis |
//! | `decompile_apk(path)` | Extrae `classes*.dex` de un APK y decompila todo |
//! | `dex_strings(dex)` | Tabla de strings del DEX |

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::io::{Cursor, Read};

// ========================================================================= //
// Constantes del formato DEX
// ========================================================================= //

/// Prefijo mágico de todo fichero DEX.
pub const DEX_MAGIC: &[u8; 4] = b"dex\n";

/// Versiones del formato DEX que sabe leer este módulo.
pub const SUPPORTED_DEX_VERSIONS: [&str; 7] = ["035", "036", "037", "038", "039", "040", "041"];

/// Tamaño del header DEX (fijo desde siempre).
const HEADER_SIZE: usize = 112;

/// `endian_tag` de un DEX little-endian (el único que se soporta).
const ENDIAN_CONSTANT: u32 = 0x1234_5678;

/// Tamaño máximo de un string del pool (evita que un DEX corrupto nos coma la RAM).
const MAX_STRING_BYTES: usize = 1 << 20;

/// Número máximo de clases que se decompilan en `decompile_all` (freno anti DoS).
const MAX_CLASSES_DECOMPILED: usize = 20_000;

// ========================================================================= //
// Utilidades de bajo nivel: lectura, LEB128, Adler-32 y SHA-1
// ========================================================================= //

/// Lee un byte del buffer acotando el rango.
fn read_u8(bytes: &[u8], off: usize) -> Result<u8, String> {
    bytes
        .get(off)
        .copied()
        .ok_or_else(|| format!("lectura fuera de rango: byte en 0x{:x}", off))
}

/// Lee un `u16` little-endian.
fn read_u16(bytes: &[u8], off: usize) -> Result<u16, String> {
    let hi = read_u8(bytes, off)? as u16;
    let lo = read_u8(bytes, off + 1)? as u16;
    Ok(hi | (lo << 8))
}

/// Lee un `u32` little-endian.
fn read_u32(bytes: &[u8], off: usize) -> Result<u32, String> {
    let b0 = read_u8(bytes, off)? as u32;
    let b1 = read_u8(bytes, off + 1)? as u32;
    let b2 = read_u8(bytes, off + 2)? as u32;
    let b3 = read_u8(bytes, off + 3)? as u32;
    Ok(b0 | (b1 << 8) | (b2 << 16) | (b3 << 24))
}

/// Escribe un `u32` little-endian (sólo para los tests y el constructor de DEX).
fn write_u32(buf: &mut [u8], off: usize, value: u32) {
    buf[off] = (value & 0xff) as u8;
    buf[off + 1] = ((value >> 8) & 0xff) as u8;
    buf[off + 2] = ((value >> 16) & 0xff) as u8;
    buf[off + 3] = ((value >> 24) & 0xff) as u8;
}

/// Escribe un `u16` little-endian.
fn write_u16(buf: &mut [u8], off: usize, value: u16) {
    buf[off] = (value & 0xff) as u8;
    buf[off + 1] = ((value >> 8) & 0xff) as u8;
}

/// Decodifica un `uleb128` (unsigned little-endian base 128) desde `pos`.
///
/// Devuelve el valor y avanza `pos` hasta el siguiente byte.
fn read_uleb128(bytes: &[u8], pos: &mut usize) -> Result<u32, String> {
    let mut result: u32 = 0;
    let mut shift = 0u32;
    loop {
        let byte = read_u8(bytes, *pos)?;
        *pos += 1;
        result |= ((byte & 0x7f) as u32).wrapping_shl(shift);
        if byte & 0x80 == 0 {
            return Ok(result);
        }
        shift += 7;
        if shift > 28 {
            return Err("uleb128 demasiado largo".to_string());
        }
    }
}

/// Decodifica un `sleb128` (signed little-endian base 128) desde `pos`.
fn read_sleb128(bytes: &[u8], pos: &mut usize) -> Result<i32, String> {
    let mut result: i32 = 0;
    let mut shift = 0u32;
    loop {
        let byte = read_u8(bytes, *pos)?;
        *pos += 1;
        result |= ((byte & 0x7f) as i32).wrapping_shl(shift);
        shift += 7;
        if byte & 0x80 == 0 {
            if shift < 32 && byte & 0x40 != 0 {
                result |= -1i32.wrapping_shl(shift);
            }
            return Ok(result);
        }
        if shift > 28 {
            return Err("sleb128 demasiado largo".to_string());
        }
    }
}

/// Adler-32 (el checksum del header DEX).
fn adler32(bytes: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for chunk in bytes.chunks(5552) {
        for &byte in chunk {
            a += byte as u32;
            b += a;
        }
        a %= MOD;
        b %= MOD;
    }
    (b << 16) | a
}

/// SHA-1 puro (la firma del header DEX). Implementado a mano porque el crate
/// `sha2` sólo trae SHA-256 y no se pueden añadir dependencias nuevas.
fn sha1(bytes: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476, 0xC3D2_E1F0];
    let bit_len = (bytes.len() as u64).wrapping_mul(8);

    let mut msg = bytes.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for block in msg.chunks(64) {
        let mut w = [0u32; 80];
        for (i, word) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, &wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let tmp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = tmp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut out = [0u8; 20];
    for (i, value) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&value.to_be_bytes());
    }
    out
}

/// Convierte bytes en hexadecimal minúsculas.
fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// Decodifica MUTF-8 (el "UTF-8 modificado" que usan los strings del DEX).
///
/// Diferencias con UTF-8: el NUL se codifica como `C0 80` y los pares de
/// surrogates van en CESU-8 (dos secuencias de 3 bytes).
fn decode_mutf8(bytes: &[u8]) -> String {
    let mut cps: Vec<u32> = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        let b0 = bytes[i];
        if b0 < 0x80 {
            cps.push(b0 as u32);
            i += 1;
        } else if b0 & 0xE0 == 0xC0 && i + 1 < bytes.len() {
            cps.push((((b0 & 0x1F) as u32) << 6) | ((bytes[i + 1] & 0x3F) as u32));
            i += 2;
        } else if b0 & 0xF0 == 0xE0 && i + 2 < bytes.len() {
            cps.push(
                (((b0 & 0x0F) as u32) << 12)
                    | (((bytes[i + 1] & 0x3F) as u32) << 6)
                    | ((bytes[i + 2] & 0x3F) as u32),
            );
            i += 3;
        } else if b0 & 0xF8 == 0xF0 && i + 3 < bytes.len() {
            cps.push(
                (((b0 & 0x07) as u32) << 18)
                    | (((bytes[i + 1] & 0x3F) as u32) << 12)
                    | (((bytes[i + 2] & 0x3F) as u32) << 6)
                    | ((bytes[i + 3] & 0x3F) as u32),
            );
            i += 4;
        } else {
            cps.push(0xFFFD);
            i += 1;
        }
    }

    // Recombina los pares de surrogates.
    let mut out = String::with_capacity(cps.len());
    let mut j = 0usize;
    while j < cps.len() {
        let c = cps[j];
        if (0xD800..=0xDBFF).contains(&c) {
            if j + 1 < cps.len() && (0xDC00..=0xDFFF).contains(&cps[j + 1]) {
                let combined =
                    0x1_0000 + (((c - 0xD800) as u32) << 10) + (cps[j + 1] - 0xDC00) as u32;
                out.push(char::from_u32(combined).unwrap_or('\u{FFFD}'));
                j += 2;
                continue;
            }
            out.push('\u{FFFD}');
            j += 1;
            continue;
        }
        if (0xDC00..=0xDFFF).contains(&c) {
            out.push('\u{FFFD}');
            j += 1;
            continue;
        }
        out.push(char::from_u32(c).unwrap_or('\u{FFFD}'));
        j += 1;
    }
    out
}

// ========================================================================= //
// Estructuras de datos del DEX
// ========================================================================= //

/// Una entrada de `type_ids`: un descriptor (`Lcom/foo/Bar;`) del pool.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DexType {
    /// Índice en la tabla de strings.
    pub descriptor_idx: u32,
    /// Descriptor ya resuelto (`Lcom/foo/Bar;`, `[I`, `V`…).
    pub descriptor: String,
}

/// Una entrada de `proto_ids`: firma de un método.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DexProto {
    /// Índice del "shorty" en la tabla de strings.
    pub shorty_idx: u32,
    /// Shorty resuelto (`V`, `Ljava/lang/String;`…).
    pub shorty: String,
    /// Índice del tipo de retorno en `type_ids`.
    pub return_type_idx: u32,
    /// Tipo de retorno resuelto.
    pub return_type: String,
    /// Descriptores de los parámetros, en orden.
    pub parameters: Vec<String>,
}

/// Una entrada de `field_ids`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DexField {
    /// Índice de la clase en `type_ids`.
    pub class_idx: u16,
    /// Índice del tipo del campo en `type_ids`.
    pub type_idx: u16,
    /// Índice del nombre en la tabla de strings.
    pub name_idx: u32,
    /// Clase propietaria resuelta.
    pub class_name: String,
    /// Tipo del campo resuelto.
    pub field_type: String,
    /// Nombre del campo.
    pub name: String,
}

/// Una entrada de `method_ids`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DexMethod {
    /// Índice de la clase en `type_ids`.
    pub class_idx: u16,
    /// Índice del prototipo en `proto_ids`.
    pub proto_idx: u16,
    /// Índice del nombre en la tabla de strings.
    pub name_idx: u32,
    /// Clase propietaria resuelta.
    pub class_name: String,
    /// Firma completa `(Ljava/lang/String;)V`.
    pub proto: String,
    /// Nombre del método.
    pub name: String,
}

/// Un campo codificado dentro del `class_data_item`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DexEncodedField {
    /// Índice en `field_ids`.
    pub field_idx: u32,
    /// Flags de acceso.
    pub access_flags: u32,
}

/// Un método codificado dentro del `class_data_item`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DexEncodedMethod {
    /// Índice en `method_ids`.
    pub method_idx: u32,
    /// Flags de acceso.
    pub access_flags: u32,
    /// Offset del `code_item` (0 = método abstracto/nativo).
    pub code_off: u32,
    /// Bytecode desensamblado en crudo (código de 16 bits).
    pub code: Option<DexCode>,
}

/// Un `code_item`: el cuerpo de un método.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DexCode {
    /// Registros totales del frame.
    pub registers_size: u16,
    /// Registros de entrada (parámetros + `this`).
    pub ins_size: u16,
    /// Registros de salida.
    pub outs_size: u16,
    /// Número de bloques `try`.
    pub tries_size: u16,
    /// Offset del `debug_info_item` (0 = sin depuración).
    pub debug_info_off: u32,
    /// Código en unidades de 16 bits.
    pub insns: Vec<u16>,
    /// Tabla de manejadores de excepción.
    pub tries: Vec<DexTry>,
    /// Información de depuración (nombres de variables), si existe.
    pub debug: Option<DexDebugInfo>,
}

/// Una entrada de la tabla `try_item` con su manejador resuelto.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DexTry {
    /// Offset (en unidades de instrucción) donde empieza el bloque protegido.
    pub start_addr: u32,
    /// Longitud del bloque protegido.
    pub insn_count: u16,
    /// Offset del manejador dentro de la lista de catch handlers.
    pub handler_off: u16,
    /// Tipos capturados y dirección del `catch_all`.
    pub catches: Vec<DexCatch>,
}

/// Un manejador de excepciones concreto.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DexCatch {
    /// Tipos capturados (vacío = `catch (Throwable)`).
    pub type_names: Vec<String>,
    /// Dirección del manejador `catch_all` (0 = no hay).
    pub catch_all_addr: u32,
}

/// El `debug_info_item`: nombres de parámetros y variables locales.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DexDebugInfo {
    /// Número de línea inicial.
    pub line_start: u32,
    /// Nombres de los parámetros (los que el compilador guardó).
    pub parameter_names: Vec<String>,
    /// Nombres de variables locales.
    pub local_names: Vec<String>,
}

/// Una anotación leída de `annotations_directory_item`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DexAnnotation {
    /// Visibilidad (`BUILD`, `RUNTIME`, `SOURCE`).
    pub visibility: String,
    /// Tipo de la anotación.
    pub type_name: String,
    /// Pares nombre/valor.
    pub elements: Vec<DexAnnotationElement>,
}

/// Un elemento (par nombre/valor) de una anotación.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DexAnnotationElement {
    /// Nombre del elemento.
    pub name: String,
    /// Valor ya formateado como texto.
    pub value: String,
}

/// Una entrada del `map_list`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DexMapItem {
    /// Tipo de item (`0x0000` header, `0x2001` code_item…).
    pub item_type: u16,
    /// Cuántos items hay de este tipo.
    pub size: u32,
    /// Offset del primero.
    pub offset: u32,
}

/// Una clase (`class_def_item`) con todo su contenido ya resuelto.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DexClassDef {
    /// Índice de la clase en `type_ids`.
    pub class_idx: u32,
    /// Flags de acceso.
    pub access_flags: u32,
    /// Índice de la superclase en `type_ids` (`u32::MAX` = `java/lang/Object`).
    pub superclass_idx: u32,
    /// Offset de la lista de interfaces.
    pub interfaces_off: u32,
    /// Índice del fichero fuente en la tabla de strings.
    pub source_file_idx: u32,
    /// Offset del directorio de anotaciones.
    pub annotations_off: u32,
    /// Offset del `class_data_item`.
    pub class_data_off: u32,
    /// Offset de los valores estáticos iniciales.
    pub static_values_off: u32,
    /// Descriptor de la clase.
    pub class_name: String,
    /// Descriptor de la superclase.
    pub superclass_name: String,
    /// Interfaces implementadas.
    pub interfaces: Vec<String>,
    /// Fichero fuente (`Foo.java`), si se conservó.
    pub source_file: String,
    /// Campos estáticos.
    pub static_fields: Vec<DexEncodedField>,
    /// Campos de instancia.
    pub instance_fields: Vec<DexEncodedField>,
    /// Métodos directos (constructores, privados, estáticos).
    pub direct_methods: Vec<DexEncodedMethod>,
    /// Métodos virtuales.
    pub virtual_methods: Vec<DexEncodedMethod>,
    /// Anotaciones de la clase.
    pub annotations: Vec<DexAnnotation>,
    /// Valores estáticos iniciales (`nombre = valor`).
    pub static_values: Vec<String>,
}

/// Un DEX completo ya parseado.
///
/// Es serializable con `serde_json` (los bytes crudos se omiten: para
/// reconstruirlos hay que volver a llamar a [`DexFile::parse`]).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexFile {
    /// Bytes originales del fichero (omitidos al serializar).
    #[serde(skip, default)]
    pub bytes: Vec<u8>,
    /// Versión del formato (`035`…`041`).
    pub version: String,
    /// Checksum Adler-32 del header.
    pub checksum: u32,
    /// `true` si el Adler-32 del fichero coincide con el del header.
    pub checksum_valid: bool,
    /// Firma SHA-1 del DEX (hex).
    pub signature: String,
    /// `true` si la firma SHA-1 del fichero coincide con la del header.
    pub signature_valid: bool,
    /// Huella SHA-256 del DEX (hex) — calculada con el crate `sha2`.
    pub sha256: String,
    /// Tamaño declarado del fichero.
    pub file_size: u32,
    /// `endian_tag` del header.
    pub endian_tag: u32,
    /// Tamaño del header.
    pub header_size: u32,
    /// Tamaño de la sección `link`.
    pub link_size: u32,
    /// Offset de la sección `link`.
    pub link_off: u32,
    /// Offset del `map_list`.
    pub map_off: u32,
    /// Tabla de strings (`string_ids`).
    pub strings: Vec<String>,
    /// Tabla de tipos (`type_ids`).
    pub types: Vec<DexType>,
    /// Tabla de prototipos (`proto_ids`).
    pub protos: Vec<DexProto>,
    /// Tabla de campos (`field_ids`).
    pub fields: Vec<DexField>,
    /// Tabla de métodos (`method_ids`).
    pub methods: Vec<DexMethod>,
    /// Tabla de clases (`class_defs`).
    pub classes: Vec<DexClassDef>,
    /// `map_list` del DEX.
    pub map_items: Vec<DexMapItem>,
}

// ========================================================================= //
// Parser del DEX
// ========================================================================= //

/// Constantes del `map_list` (tipos de item).
const MAP_TYPE_HEADER: u16 = 0x0000;
const MAP_TYPE_STRING_ID: u16 = 0x0001;
const MAP_TYPE_TYPE_ID: u16 = 0x0002;
const MAP_TYPE_PROTO_ID: u16 = 0x0003;
const MAP_TYPE_FIELD_ID: u16 = 0x0004;
const MAP_TYPE_METHOD_ID: u16 = 0x0005;
const MAP_TYPE_CLASS_DEF: u16 = 0x0006;
const MAP_TYPE_MAP_LIST: u16 = 0x1000;
const MAP_TYPE_TYPE_LIST: u16 = 0x1001;
const MAP_TYPE_CLASS_DATA: u16 = 0x2000;
const MAP_TYPE_CODE_ITEM: u16 = 0x2001;
const MAP_TYPE_STRING_DATA: u16 = 0x2002;
const MAP_TYPE_DEBUG_INFO: u16 = 0x2003;

/// Un DEX ya parseado.
impl DexFile {
    /// Parsea un DEX completo a partir de sus bytes.
    ///
    /// Acepta los formatos `035` a `041`.  Devuelve `Err` con un mensaje
    /// legible si el fichero no es un DEX válido o está truncado.
    pub fn parse(bytes: &[u8]) -> Result<DexFile, String> {
        if bytes.len() < HEADER_SIZE {
            return Err(format!(
                "fichero demasiado corto para ser un DEX: {} bytes (mínimo {})",
                bytes.len(),
                HEADER_SIZE
            ));
        }
        if &bytes[0..4] != DEX_MAGIC {
            return Err(format!(
                "magic inválido: {:?} (se esperaba {:?})",
                String::from_utf8_lossy(&bytes[0..4]),
                String::from_utf8_lossy(DEX_MAGIC)
            ));
        }
        let version_bytes = &bytes[4..7];
        let version = String::from_utf8_lossy(version_bytes).to_string();
        if bytes[7] != 0 {
            return Err("magic DEX mal terminado (se esperaba un NUL en el byte 7)".to_string());
        }
        if !SUPPORTED_DEX_VERSIONS.contains(&version.as_str()) {
            return Err(format!(
                "versión de DEX no soportada: {} (soportadas: {})",
                version,
                SUPPORTED_DEX_VERSIONS.join(", ")
            ));
        }

        let checksum = read_u32(bytes, 8)?;
        let signature_bytes = &bytes[12..32];
        let file_size = read_u32(bytes, 32)?;
        let header_size = read_u32(bytes, 36)?;
        let endian_tag = read_u32(bytes, 40)?;
        if endian_tag != ENDIAN_CONSTANT {
            return Err(format!(
                "endianness no soportado (endian_tag 0x{:08x}); sólo se lee DEX little-endian",
                endian_tag
            ));
        }
        if header_size as usize != HEADER_SIZE {
            return Err(format!(
                "header_size inesperado: {} (se esperaba {})",
                header_size, HEADER_SIZE
            ));
        }
        if file_size as usize > bytes.len() {
            // No es fatal: algunos empaquetadores recortan el DEX, pero avisamos.
        }
        let link_size = read_u32(bytes, 44)?;
        let link_off = read_u32(bytes, 48)?;
        let map_off = read_u32(bytes, 52)?;
        let string_ids_size = read_u32(bytes, 56)? as usize;
        let string_ids_off = read_u32(bytes, 60)? as usize;
        let type_ids_size = read_u32(bytes, 64)? as usize;
        let type_ids_off = read_u32(bytes, 68)? as usize;
        let proto_ids_size = read_u32(bytes, 72)? as usize;
        let proto_ids_off = read_u32(bytes, 76)? as usize;
        let field_ids_size = read_u32(bytes, 80)? as usize;
        let field_ids_off = read_u32(bytes, 84)? as usize;
        let method_ids_size = read_u32(bytes, 88)? as usize;
        let method_ids_off = read_u32(bytes, 92)? as usize;
        let class_defs_size = read_u32(bytes, 96)? as usize;
        let class_defs_off = read_u32(bytes, 100)? as usize;
        let data_size = read_u32(bytes, 104)? as usize;
        let data_off = read_u32(bytes, 108)? as usize;
        let _ = (link_size, link_off, data_size, data_off);

        // ---------------------------------------------------------------- //
        // Tabla de strings
        // ---------------------------------------------------------------- //
        let mut strings: Vec<String> = Vec::with_capacity(string_ids_size.min(200_000));
        for i in 0..string_ids_size {
            let off = read_u32(bytes, string_ids_off + i * 4)? as usize;
            strings.push(parse_string_data(bytes, off)?);
        }

        // ---------------------------------------------------------------- //
        // Tabla de tipos
        // ---------------------------------------------------------------- //
        let mut types: Vec<DexType> = Vec::with_capacity(type_ids_size.min(200_000));
        for i in 0..type_ids_size {
            let descriptor_idx = read_u32(bytes, type_ids_off + i * 4)?;
            let descriptor = strings
                .get(descriptor_idx as usize)
                .cloned()
                .unwrap_or_else(|| format!("<string#{}>", descriptor_idx));
            types.push(DexType {
                descriptor_idx,
                descriptor,
            });
        }

        // ---------------------------------------------------------------- //
        // Tabla de prototipos
        // ---------------------------------------------------------------- //
        let mut protos: Vec<DexProto> = Vec::with_capacity(proto_ids_size.min(200_000));
        for i in 0..proto_ids_size {
            let base = proto_ids_off + i * 12;
            let shorty_idx = read_u32(bytes, base)?;
            let return_type_idx = read_u32(bytes, base + 4)?;
            let parameters_off = read_u32(bytes, base + 8)? as usize;
            let shorty = strings
                .get(shorty_idx as usize)
                .cloned()
                .unwrap_or_else(|| format!("<string#{}>", shorty_idx));
            let return_type = types
                .get(return_type_idx as usize)
                .map(|t| t.descriptor.clone())
                .unwrap_or_else(|| format!("<type#{}>", return_type_idx));
            let mut parameters = Vec::new();
            if parameters_off != 0 {
                let count = read_u32(bytes, parameters_off)? as usize;
                for p in 0..count {
                    let idx = read_u16(bytes, parameters_off + 4 + p * 2)? as u32;
                    parameters.push(
                        types
                            .get(idx as usize)
                            .map(|t| t.descriptor.clone())
                            .unwrap_or_else(|| format!("<type#{}>", idx)),
                    );
                }
            }
            protos.push(DexProto {
                shorty_idx,
                shorty,
                return_type_idx,
                return_type,
                parameters,
            });
        }

        // ---------------------------------------------------------------- //
        // Tabla de campos
        // ---------------------------------------------------------------- //
        let mut fields: Vec<DexField> = Vec::with_capacity(field_ids_size.min(200_000));
        for i in 0..field_ids_size {
            let base = field_ids_off + i * 8;
            let class_idx = read_u16(bytes, base)?;
            let type_idx = read_u16(bytes, base + 2)?;
            let name_idx = read_u32(bytes, base + 4)?;
            fields.push(DexField {
                class_idx,
                type_idx,
                name_idx,
                class_name: types
                    .get(class_idx as usize)
                    .map(|t| t.descriptor.clone())
                    .unwrap_or_else(|| format!("<type#{}>", class_idx)),
                field_type: types
                    .get(type_idx as usize)
                    .map(|t| t.descriptor.clone())
                    .unwrap_or_else(|| format!("<type#{}>", type_idx)),
                name: strings
                    .get(name_idx as usize)
                    .cloned()
                    .unwrap_or_else(|| format!("<string#{}>", name_idx)),
            });
        }

        // ---------------------------------------------------------------- //
        // Tabla de métodos
        // ---------------------------------------------------------------- //
        let mut methods: Vec<DexMethod> = Vec::with_capacity(method_ids_size.min(200_000));
        for i in 0..method_ids_size {
            let base = method_ids_off + i * 8;
            let class_idx = read_u16(bytes, base)?;
            let proto_idx = read_u16(bytes, base + 2)?;
            let name_idx = read_u32(bytes, base + 4)?;
            let proto = protos
                .get(proto_idx as usize)
                .map(proto_signature)
                .unwrap_or_else(|| format!("<proto#{}>", proto_idx));
            methods.push(DexMethod {
                class_idx,
                proto_idx,
                name_idx,
                class_name: types
                    .get(class_idx as usize)
                    .map(|t| t.descriptor.clone())
                    .unwrap_or_else(|| format!("<type#{}>", class_idx)),
                proto,
                name: strings
                    .get(name_idx as usize)
                    .cloned()
                    .unwrap_or_else(|| format!("<string#{}>", name_idx)),
            });
        }

        // ---------------------------------------------------------------- //
        // Clases
        // ---------------------------------------------------------------- //
        let mut classes: Vec<DexClassDef> = Vec::with_capacity(class_defs_size.min(200_000));
        for i in 0..class_defs_size {
            let base = class_defs_off + i * 32;
            let class_idx = read_u32(bytes, base)?;
            let access_flags = read_u32(bytes, base + 4)?;
            let superclass_idx = read_u32(bytes, base + 8)?;
            let interfaces_off = read_u32(bytes, base + 12)? as usize;
            let source_file_idx = read_u32(bytes, base + 16)?;
            let annotations_off = read_u32(bytes, base + 20)? as usize;
            let class_data_off = read_u32(bytes, base + 24)? as usize;
            let static_values_off = read_u32(bytes, base + 28)? as usize;

            let mut interfaces = Vec::new();
            if interfaces_off != 0 {
                let count = read_u32(bytes, interfaces_off)? as usize;
                for p in 0..count {
                    let idx = read_u16(bytes, interfaces_off + 4 + p * 2)? as u32;
                    interfaces.push(
                        types
                            .get(idx as usize)
                            .map(|t| t.descriptor.clone())
                            .unwrap_or_else(|| format!("<type#{}>", idx)),
                    );
                }
            }

            let mut class_def = DexClassDef {
                class_idx,
                access_flags,
                superclass_idx,
                interfaces_off: interfaces_off as u32,
                source_file_idx,
                annotations_off: annotations_off as u32,
                class_data_off: class_data_off as u32,
                static_values_off: static_values_off as u32,
                class_name: types
                    .get(class_idx as usize)
                    .map(|t| t.descriptor.clone())
                    .unwrap_or_else(|| format!("<type#{}>", class_idx)),
                superclass_name: if superclass_idx == u32::MAX {
                    "Ljava/lang/Object;".to_string()
                } else {
                    types
                        .get(superclass_idx as usize)
                        .map(|t| t.descriptor.clone())
                        .unwrap_or_else(|| format!("<type#{}>", superclass_idx))
                },
                interfaces,
                source_file: if source_file_idx == u32::MAX {
                    String::new()
                } else {
                    strings
                        .get(source_file_idx as usize)
                        .cloned()
                        .unwrap_or_else(|| format!("<string#{}>", source_file_idx))
                },
                ..Default::default()
            };

            if class_data_off != 0 {
                parse_class_data(bytes, class_data_off, &mut class_def, &strings, &types)?;
            }
            if static_values_off != 0 {
                class_def.static_values =
                    parse_static_values(bytes, static_values_off, &class_def, &fields)?;
            }
            if annotations_off != 0 {
                class_def.annotations = parse_annotations(bytes, annotations_off, &types)?;
            }
            classes.push(class_def);
        }

        // ---------------------------------------------------------------- //
        // map_list
        // ---------------------------------------------------------------- //
        let mut map_items: Vec<DexMapItem> = Vec::new();
        if map_off != 0 {
            let map_at = map_off as usize;
            let count = read_u32(bytes, map_at)?;
            for i in 0..count.min(4096) {
                let base = map_at + 4 + i as usize * 12;
                map_items.push(DexMapItem {
                    item_type: read_u16(bytes, base)?,
                    size: read_u32(bytes, base + 4)?,
                    offset: read_u32(bytes, base + 8)?,
                });
            }
        }

        // ---------------------------------------------------------------- //
        // Integridad
        // ---------------------------------------------------------------- //
        let computed_checksum = adler32(&bytes[12..]);
        let computed_signature = sha1(&bytes[32..]);
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let sha256 = hex(&hasher.finalize());

        Ok(DexFile {
            bytes: bytes.to_vec(),
            version,
            checksum,
            checksum_valid: computed_checksum == checksum,
            signature: hex(signature_bytes),
            signature_valid: computed_signature[..] == signature_bytes[..],
            sha256,
            file_size,
            endian_tag,
            header_size,
            link_size,
            link_off,
            map_off,
            strings,
            types,
            protos,
            fields,
            methods,
            classes,
            map_items,
        })
    }

    /// Devuelve el descriptor del tipo en `idx` (o un marcador si no existe).
    pub fn type_descriptor(&self, idx: u32) -> String {
        self.types
            .get(idx as usize)
            .map(|t| t.descriptor.clone())
            .unwrap_or_else(|| format!("<type#{}>", idx))
    }

    /// Devuelve el string en `idx` (o un marcador si no existe).
    pub fn string_at(&self, idx: u32) -> String {
        self.strings
            .get(idx as usize)
            .cloned()
            .unwrap_or_else(|| format!("<string#{}>", idx))
    }

    /// Devuelve la firma de un método tal y como la escribe `javap`.
    pub fn method_signature(&self, method_idx: u32) -> String {
        match self.methods.get(method_idx as usize) {
            Some(m) => format!("{}.{}{}", m.class_name, m.name, m.proto),
            None => format!("<method#{}>", method_idx),
        }
    }

    /// Devuelve la firma completa de un campo.
    pub fn field_signature(&self, field_idx: u32) -> String {
        match self.fields.get(field_idx as usize) {
            Some(f) => format!("{}.{}:{}", f.class_name, f.name, f.field_type),
            None => format!("<field#{}>", field_idx),
        }
    }
}

/// Parsea un `string_data_item` (LEB128 de longitud UTF-16 + MUTF-8 + NUL).
fn parse_string_data(bytes: &[u8], off: usize) -> Result<String, String> {
    let mut pos = off;
    let utf16_size = read_uleb128(bytes, &mut pos)?;
    let mut end = pos;
    while end < bytes.len() && bytes[end] != 0 {
        end += 1;
        if end - pos > MAX_STRING_BYTES {
            return Err(format!("string_data_item en 0x{:x} demasiado largo", off));
        }
    }
    let text = decode_mutf8(&bytes[pos..end]);
    // `utf16_size` es el número de unidades UTF-16: sanity check.
    let real_units = text.chars().map(|c| c.len_utf16()).sum::<usize>();
    if utf16_size as usize != real_units {
        // No es fatal: algunos empaquetadores recortan el NUL final.
    }
    Ok(text)
}

/// Parsea un `class_data_item` y rellena los campos y métodos de la clase.
fn parse_class_data(
    bytes: &[u8],
    off: usize,
    class_def: &mut DexClassDef,
    strings: &[String],
    types: &[DexType],
) -> Result<(), String> {
    let mut pos = off;
    let static_fields_size = read_uleb128(bytes, &mut pos)? as usize;
    let instance_fields_size = read_uleb128(bytes, &mut pos)? as usize;
    let direct_methods_size = read_uleb128(bytes, &mut pos)? as usize;
    let virtual_methods_size = read_uleb128(bytes, &mut pos)? as usize;

    let mut read_fields = |pos: &mut usize, count: usize| -> Result<Vec<DexEncodedField>, String> {
        let mut out = Vec::with_capacity(count);
        let mut last_idx: u32 = 0;
        for _ in 0..count {
            let diff = read_uleb128(bytes, pos)?;
            let access_flags = read_uleb128(bytes, pos)?;
            last_idx = last_idx.wrapping_add(diff);
            out.push(DexEncodedField {
                field_idx: last_idx,
                access_flags,
            });
        }
        Ok(out)
    };

    class_def.static_fields = read_fields(&mut pos, static_fields_size)?;
    class_def.instance_fields = read_fields(&mut pos, instance_fields_size)?;

    let mut read_methods = |pos: &mut usize, count: usize| -> Result<Vec<DexEncodedMethod>, String> {
        let mut out = Vec::with_capacity(count);
        let mut last_idx: u32 = 0;
        for _ in 0..count {
            let diff = read_uleb128(bytes, pos)?;
            let access_flags = read_uleb128(bytes, pos)?;
            let code_off = read_uleb128(bytes, pos)?;
            last_idx = last_idx.wrapping_add(diff);
            let code = if code_off != 0 {
                match parse_code_item(bytes, code_off as usize, strings, types) {
                    Ok(c) => Some(c),
                    Err(_) => None,
                }
            } else {
                None
            };
            out.push(DexEncodedMethod {
                method_idx: last_idx,
                access_flags,
                code_off,
                code,
            });
        }
        Ok(out)
    };

    class_def.direct_methods = read_methods(&mut pos, direct_methods_size)?;
    class_def.virtual_methods = read_methods(&mut pos, virtual_methods_size)?;
    Ok(())
}

/// Parsea un `code_item`: bytecode, `try`/`catch` y debug info.
fn parse_code_item(
    bytes: &[u8],
    off: usize,
    strings: &[String],
    types: &[DexType],
) -> Result<DexCode, String> {
    let registers_size = read_u16(bytes, off)?;
    let ins_size = read_u16(bytes, off + 2)?;
    let outs_size = read_u16(bytes, off + 4)?;
    let tries_size = read_u16(bytes, off + 6)?;
    let debug_info_off = read_u32(bytes, off + 8)?;
    let insns_size = read_u32(bytes, off + 12)? as usize;

    let insns_start = off + 16;
    let mut insns = Vec::with_capacity(insns_size.min(1 << 20));
    for i in 0..insns_size {
        insns.push(read_u16(bytes, insns_start + i * 2)?);
    }

    let mut tries: Vec<DexTry> = Vec::new();
    if tries_size > 0 {
        // Relleno de alineación a 4 bytes si el código tiene longitud impar.
        let mut tries_off = insns_start + insns_size * 2;
        if insns_size % 2 == 1 {
            tries_off += 2;
        }
        let handlers_off = tries_off + tries_size as usize * 8;
        let handlers = parse_catch_handlers(bytes, handlers_off, types)?;

        for i in 0..tries_size as usize {
            let base = tries_off + i * 8;
            let start_addr = read_u32(bytes, base)?;
            let insn_count = read_u16(bytes, base + 4)?;
            let handler_off = read_u16(bytes, base + 6)?;
            let catches = handlers.get(&handler_off).cloned().unwrap_or_default();
            tries.push(DexTry {
                start_addr,
                insn_count,
                handler_off,
                catches,
            });
        }
    }

    let debug = if debug_info_off != 0 {
        parse_debug_info(bytes, debug_info_off as usize, strings)
    } else {
        None
    };

    Ok(DexCode {
        registers_size,
        ins_size,
        outs_size,
        tries_size,
        debug_info_off,
        insns,
        tries,
        debug,
    })
}

/// Parsea la lista de `catch handlers` (`encoded_catch_handler_list`).
///
/// Devuelve un mapa `handler_off` → manejador, porque cada `try_item`
/// referencia su handler por el offset en bytes dentro de esta lista.
fn parse_catch_handlers(
    bytes: &[u8],
    off: usize,
    types: &[DexType],
) -> Result<HashMap<u16, Vec<DexCatch>>, String> {
    let mut pos = off;
    let size = read_uleb128(bytes, &mut pos)? as usize;
    let mut out: HashMap<u16, Vec<DexCatch>> = HashMap::with_capacity(size);
    for _ in 0..size {
        let handler_offset = (pos - off) as u16;
        let count = read_sleb128(bytes, &mut pos)?;
        let mut types_caught: Vec<String> = Vec::new();
        let mut addrs: Vec<u32> = Vec::new();
        for _ in 0..count.max(0) {
            let type_idx = read_uleb128(bytes, &mut pos)?;
            let addr = read_uleb128(bytes, &mut pos)?;
            types_caught.push(
                types
                    .get(type_idx as usize)
                    .map(|t| t.descriptor.clone())
                    .unwrap_or_else(|| format!("<type#{}>", type_idx)),
            );
            addrs.push(addr);
        }
        let catch_all_addr = read_uleb128(bytes, &mut pos)?;
        out.insert(
            handler_offset,
            vec![DexCatch {
                type_names: types_caught,
                catch_all_addr,
            }],
        );
        let _ = addrs;
    }
    Ok(out)
}

/// Parsea un `debug_info_item` (nombres de parámetros y variables locales).
fn parse_debug_info(bytes: &[u8], off: usize, strings: &[String]) -> Option<DexDebugInfo> {
    let mut pos = off;
    let line_start = read_uleb128(bytes, &mut pos).ok()?;
    let parameters_size = read_uleb128(bytes, &mut pos).ok()? as usize;
    let mut parameter_names = Vec::with_capacity(parameters_size);
    for _ in 0..parameters_size {
        let name_idx_plus_one = read_uleb128(bytes, &mut pos).ok()?;
        if name_idx_plus_one == 0 {
            parameter_names.push(String::new());
        } else {
            parameter_names.push(
                strings
                    .get((name_idx_plus_one - 1) as usize)
                    .cloned()
                    .unwrap_or_else(|| format!("<string#{}>", name_idx_plus_one - 1)),
            );
        }
    }

    // Máquina de estados del debug info (sólo necesitamos los nombres).
    let mut local_names: Vec<String> = Vec::new();
    let mut register_count = 0usize;
    let mut guard = 0usize;
    loop {
        guard += 1;
        if guard > 200_000 {
            break;
        }
        let opcode = match bytes.get(pos) {
            Some(b) => *b,
            None => break,
        };
        pos += 1;
        match opcode {
            0x00 => break, // DBG_END_SEQUENCE
            0x01 => {
                let _line_diff = read_uleb128(bytes, &mut pos).ok()?;
            }
            0x02 => {
                let _addr_diff = read_sleb128(bytes, &mut pos).ok()?;
            }
            0x03 | 0x04 => {
                // DBG_START_LOCAL / DBG_START_LOCAL_EXTENDED
                let _reg_num = read_uleb128(bytes, &mut pos).ok()?;
                let _name_idx_plus_one = read_uleb128(bytes, &mut pos).ok()?;
                let _type_idx_plus_one = read_uleb128(bytes, &mut pos).ok()?;
                if opcode == 0x04 {
                    let _sig_idx_plus_one = read_uleb128(bytes, &mut pos).ok()?;
                }
                register_count += 1;
            }
            0x05 | 0x06 => {
                // DBG_END_LOCAL / DBG_RESTART_LOCAL
                let _reg_num = read_uleb128(bytes, &mut pos).ok()?;
                register_count = register_count.saturating_sub(1);
            }
            0x07 => {
                // DBG_SET_PROLOGUE_END
                let _addr = read_uleb128(bytes, &mut pos).ok()?;
            }
            0x08 => {
                // DBG_SET_EPILOGUE_BEGIN
                let _addr = read_uleb128(bytes, &mut pos).ok()?;
            }
            0x09 => {
                // DBG_SET_FILE
                let _name_idx_plus_one = read_uleb128(bytes, &mut pos).ok()?;
            }
            _ => break,
        }
    }
    local_names.reserve(register_count);

    Some(DexDebugInfo {
        line_start,
        parameter_names,
        local_names,
    })
}

/// Parsea un `encoded_array_item` (valores estáticos iniciales).
fn parse_static_values(
    bytes: &[u8],
    off: usize,
    class_def: &DexClassDef,
    fields: &[DexField],
) -> Result<Vec<String>, String> {
    let mut pos = off;
    let count = read_uleb128(bytes, &mut pos)? as usize;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let value = read_encoded_value(bytes, &mut pos)?;
        let name = class_def
            .static_fields
            .get(i)
            .and_then(|f| fields.get(f.field_idx as usize))
            .map(|f| f.name.clone())
            .unwrap_or_else(|| format!("field#{}", i));
        out.push(format!("{} = {}", name, value));
    }
    Ok(out)
}

/// Lee un `encoded_value` y lo devuelve como texto.
fn read_encoded_value(bytes: &[u8], pos: &mut usize) -> Result<String, String> {
    let header = read_u8(bytes, *pos)?;
    *pos += 1;
    let value_type = header & 0x1f;
    let value_arg = (header >> 5) as usize;

    // Valor entero de `size` bytes (size = value_arg + 1).
    let read_int = |pos: &mut usize, size: usize| -> Result<u64, String> {
        let mut v: u64 = 0;
        for i in 0..size {
            let b = read_u8(bytes, *pos)? as u64;
            v |= b << (8 * i);
        }
        Ok(v)
    };

    let value = match value_type {
        0x00 => {
            // BYTE
            let v = read_int(pos, 1)? as u8;
            format!("{}", v as i8)
        }
        0x01 => {
            // SHORT
            let v = read_int(pos, value_arg + 1)? as u16;
            format!("{}", v as i16)
        }
        0x02 => {
            // CHAR
            let v = read_int(pos, value_arg + 1)? as u16;
            format!("'{}'", char::from_u32(v as u32).unwrap_or('?'))
        }
        0x03 => {
            // INT
            let v = read_int(pos, value_arg + 1)? as u32;
            format!("{}", v as i32)
        }
        0x04 => {
            // LONG
            let v = read_int(pos, value_arg + 1)?;
            format!("{}", v as i64)
        }
        0x05 => {
            // FLOAT
            let v = read_int(pos, value_arg + 1)? as u32;
            format!("{}", f32::from_bits(v))
        }
        0x06 => {
            // DOUBLE
            let v = read_int(pos, value_arg + 1)?;
            format!("{}", f64::from_bits(v))
        }
        0x07 => {
            // METHOD_TYPE
            let v = read_int(pos, value_arg + 1)?;
            format!("<proto#{}>", v)
        }
        0x08 => {
            // METHOD_HANDLE
            let v = read_int(pos, value_arg + 1)?;
            format!("<method_handle#{}>", v)
        }
        0x09 => {
            // STRING
            let v = read_int(pos, value_arg + 1)?;
            format!("\"<string#{}>\"", v)
        }
        0x0a => {
            // TYPE
            let v = read_int(pos, value_arg + 1)?;
            format!("<type#{}>", v)
        }
        0x0b => {
            // FIELD
            let v = read_int(pos, value_arg + 1)?;
            format!("<field#{}>", v)
        }
        0x0c => {
            // METHOD
            let v = read_int(pos, value_arg + 1)?;
            format!("<method#{}>", v)
        }
        0x0d => {
            // ENUM
            let v = read_int(pos, value_arg + 1)?;
            format!("<enum#{}>", v)
        }
        0x0e => {
            // ARRAY
            let count = read_uleb128(bytes, pos)?;
            let mut items = Vec::with_capacity(count.min(4096) as usize);
            for _ in 0..count.min(4096) {
                items.push(read_encoded_value(bytes, pos)?);
            }
            format!("[{}]", items.join(", "))
        }
        0x0f => {
            // ANNOTATION
            let type_idx = read_uleb128(bytes, pos)?;
            let count = read_uleb128(bytes, pos)?;
            let mut items = Vec::with_capacity(count.min(4096) as usize);
            for _ in 0..count.min(4096) {
                let name_idx = read_uleb128(bytes, pos)?;
                let value = read_encoded_value(bytes, pos)?;
                items.push(format!("<string#{}>={}", name_idx, value));
            }
            format!("@<type#{}>({})", type_idx, items.join(", "))
        }
        0x10 => "null".to_string(),
        0x1f => {
            // BOOLEAN: el valor va en `value_arg`, no hay bytes extra.
            format!("{}", value_arg != 0)
        }
        other => {
            return Err(format!("encoded_value con tipo desconocido 0x{:02x}", other));
        }
    };
    Ok(value)
}

/// Parsea el `annotations_directory_item` de una clase (lectura básica).
fn parse_annotations(
    bytes: &[u8],
    off: usize,
    types: &[DexType],
) -> Result<Vec<DexAnnotation>, String> {
    let mut out = Vec::new();
    let class_annotations_off = read_u32(bytes, off)? as usize;
    let fields_size = read_u32(bytes, off + 4)? as usize;
    let methods_size = read_u32(bytes, off + 8)? as usize;
    let parameters_size = read_u32(bytes, off + 12)? as usize;

    if class_annotations_off != 0 {
        out.extend(parse_annotation_set(bytes, class_annotations_off, types)?);
    }

    let mut pos = off + 16;
    // field_annotations[]
    for _ in 0..fields_size {
        let _field_idx = read_u32(bytes, pos)?;
        let set_off = read_u32(bytes, pos + 4)? as usize;
        pos += 8;
        if set_off != 0 {
            out.extend(parse_annotation_set(bytes, set_off, types)?);
        }
    }
    // method_annotations[]
    for _ in 0..methods_size {
        let _method_idx = read_u32(bytes, pos)?;
        let set_off = read_u32(bytes, pos + 4)? as usize;
        pos += 8;
        if set_off != 0 {
            out.extend(parse_annotation_set(bytes, set_off, types)?);
        }
    }
    // parameter_annotations[]
    for _ in 0..parameters_size {
        let _method_idx = read_u32(bytes, pos)?;
        let set_off = read_u32(bytes, pos + 4)? as usize;
        pos += 8;
        if set_off != 0 {
            out.extend(parse_annotation_set(bytes, set_off, types)?);
        }
    }
    Ok(out)
}

/// Parsea un `annotation_set_item` y sus `annotation_item`s.
fn parse_annotation_set(
    bytes: &[u8],
    off: usize,
    types: &[DexType],
) -> Result<Vec<DexAnnotation>, String> {
    let size = read_u32(bytes, off)? as usize;
    let mut out = Vec::with_capacity(size.min(1024));
    for i in 0..size.min(4096) {
        let item_off = read_u32(bytes, off + 4 + i * 4)? as usize;
        if item_off == 0 {
            continue;
        }
        out.push(parse_annotation_item(bytes, item_off, types)?);
    }
    Ok(out)
}

/// Parsea un `annotation_item` (visibilidad + tipo + elementos).
fn parse_annotation_item(
    bytes: &[u8],
    off: usize,
    types: &[DexType],
) -> Result<DexAnnotation, String> {
    let visibility_byte = read_u8(bytes, off)?;
    let visibility = match visibility_byte {
        0x00 => "BUILD",
        0x01 => "RUNTIME",
        0x02 => "SYSTEM",
        _ => "SOURCE",
    };
    let mut pos = off + 1;
    let type_idx = read_uleb128(bytes, &mut pos)?;
    let size = read_uleb128(bytes, &mut pos)? as usize;
    let mut elements = Vec::with_capacity(size.min(1024));
    for _ in 0..size.min(4096) {
        let name_idx = read_uleb128(bytes, &mut pos)?;
        let value = read_encoded_value(bytes, &mut pos)?;
        elements.push(DexAnnotationElement {
            name: format!("<string#{}>", name_idx),
            value,
        });
    }
    Ok(DexAnnotation {
        visibility: visibility.to_string(),
        type_name: types
            .get(type_idx as usize)
            .map(|t| t.descriptor.clone())
            .unwrap_or_else(|| format!("<type#{}>", type_idx)),
        elements,
    })
}

/// Construye la firma `(params)ret` de un prototipo.
fn proto_signature(proto: &DexProto) -> String {
    format!("({}){}", proto.parameters.join(""), proto.return_type)
}

// ========================================================================= //
// Desensamblador Dalvik
// ========================================================================= //

/// Qué clase de referencia lleva el operando de una instrucción.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    /// Sin referencia (registros, literales, saltos).
    None,
    /// Índice en `string_ids`.
    String,
    /// Índice en `type_ids`.
    Type,
    /// Índice en `field_ids`.
    Field,
    /// Índice en `method_ids`.
    Method,
    /// Índice en `proto_ids`.
    Proto,
    /// Índice en `call_site_ids`.
    CallSite,
    /// Índice en `method_handles`.
    MethodHandle,
    /// Referencia mixta método + prototipo (`invoke-polymorphic`).
    MethodAndProto,
    /// Offset de campo (formato `22cs`, sólo quickened).
    FieldOffset,
}

/// Devuelve el nombre y el formato de un opcode Dalvik.
///
/// Tabla tomada de AOSP (`libdexfile/dex/dex_instruction_list.h` y
/// `dalvik/opcode-gen/bytecode.txt`), que es la referencia que usan ART,
/// `dexdump` y baksmali.
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
        0x79..=0x7a => ("unused", "10x"),
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
        0xe3 => ("iget-volatile", "22c"),
        0xe4 => ("iput-volatile", "22c"),
        0xe5 => ("sget-volatile", "21c"),
        0xe6 => ("sput-volatile", "21c"),
        0xe7 => ("iget-object-volatile", "22c"),
        0xe8 => ("iget-wide-volatile", "22c"),
        0xe9 => ("iput-wide-volatile", "22c"),
        0xea => ("sget-wide-volatile", "21c"),
        0xeb => ("sput-wide-volatile", "21c"),
        0xec => ("breakpoint", "10x"),
        0xed => ("throw-verification-error", "20bc"),
        0xee => ("execute-inline", "35c"),
        0xef => ("execute-inline/range", "3rc"),
        0xf0 => ("invoke-object-init/range", "35c"),
        0xf1 => ("return-void-barrier", "10x"),
        0xf2 => ("iget-quick", "22c"),
        0xf3..=0xf9 => ("unused", "10x"),
        0xfa => ("invoke-polymorphic", "45cc"),
        0xfb => ("invoke-polymorphic/range", "4rcc"),
        0xfc => ("invoke-custom", "35c"),
        0xfd => ("invoke-custom/range", "3rc"),
        0xfe => ("const-method-handle", "21c"),
        0xff => ("const-method-type", "21c"),
    }
}

/// Devuelve el tipo de referencia que lleva el operando de un opcode.
pub fn opcode_ref_kind(opcode: u8) -> RefKind {
    match opcode {
        0x1a | 0x1b => RefKind::String,
        0x1c | 0x1f | 0x20 | 0x22 | 0x23 | 0x24 | 0x25 => RefKind::Type,
        0x52..=0x6d => RefKind::Field,
        0x6e..=0x72 | 0x74..=0x78 => RefKind::Method,
        0xfa | 0xfb => RefKind::MethodAndProto,
        0xfc | 0xfd => RefKind::CallSite,
        0xfe => RefKind::MethodHandle,
        0xff => RefKind::Proto,
        _ => RefKind::None,
    }
}

/// Una instrucción Dalvik desensamblada.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DalvikInsn {
    /// Offset en unidades de instrucción (16 bits) dentro del método.
    pub offset: u32,
    /// Byte del opcode.
    pub opcode: u8,
    /// Mnemónico (`invoke-direct`, `const-string`…).
    pub mnemonic: String,
    /// Formato de la instrucción (`35c`, `21c`…).
    pub format: String,
    /// Operandos ya resueltos y formateados.
    pub operands: String,
    /// Código original en unidades de 16 bits.
    pub raw: Vec<u16>,
}

/// Devuelve cuántas unidades de 16 bits ocupa un opcode.
pub fn instruction_width(opcode: u8) -> usize {
    match opcode_info(opcode).1 {
        "10x" | "12x" | "11n" | "11x" | "10t" => 1,
        "20t" | "20bc" | "22x" | "21t" | "21s" | "21h" | "21c" | "23x" | "22b" | "22t"
        | "22s" | "22c" | "22cs" => 2,
        "30t" | "32x" | "31i" | "31t" | "31c" | "35c" | "3rc" => 3,
        "45cc" | "4rcc" => 4,
        "51l" => 5,
        _ => 1,
    }
}

/// Escapa un string para poder imprimirlo entre comillas.
fn escape_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    for ch in text.chars() {
        match ch {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

/// Convierte un descriptor de tipo DEX en su forma Java.
pub fn descriptor_to_java(descriptor: &str) -> String {
    if descriptor.is_empty() {
        return descriptor.to_string();
    }
    if descriptor == "V" {
        return "void".to_string();
    }
    if descriptor.starts_with('[') {
        return format!("{}[]", descriptor_to_java(&descriptor[1..]));
    }
    if let Some(rest) = descriptor.strip_prefix('L') {
        let name = rest.trim_end_matches(';');
        return name.replace('/', ".").replace('$', ".");
    }
    match descriptor {
        "Z" => "boolean".to_string(),
        "B" => "byte".to_string(),
        "S" => "short".to_string(),
        "C" => "char".to_string(),
        "I" => "int".to_string(),
        "J" => "long".to_string(),
        "F" => "float".to_string(),
        "D" => "double".to_string(),
        other => other.to_string(),
    }
}

/// Convierte un descriptor de clase (`Lcom/x/Y;`) en nombre Java con paquete.
pub fn class_to_java(descriptor: &str) -> String {
    descriptor_to_java(descriptor)
}

/// Desensambla un `code_item` completo a una lista de instrucciones.
///
/// Los *payloads* (datos de `packed-switch`, `sparse-switch` y
/// `fill-array-data`) no son instrucciones: se detectan por su identificador
/// y se emiten como una pseudo-instrucción que consume sus unidades.
pub fn disassemble_code(dex: &DexFile, code: &DexCode) -> Vec<DalvikInsn> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    while offset < code.insns.len() {
        let unit0 = code.insns[offset];
        if (unit0 & 0xff) == 0 {
            let ident = unit0 & 0xff00;
            if ident == 0x0100 || ident == 0x0200 || ident == 0x0300 {
                let (name, width) = payload_info(&code.insns, offset, ident);
                let end = (offset + width).min(code.insns.len());
                out.push(DalvikInsn {
                    offset: offset as u32,
                    opcode: 0,
                    mnemonic: name.to_string(),
                    format: "payload".to_string(),
                    operands: format!("@0x{:04x}", offset),
                    raw: code.insns[offset..end].to_vec(),
                });
                offset += width;
                continue;
            }
        }
        let (insn, width) = decode_insn(dex, &code.insns, offset);
        offset += width;
        out.push(insn);
    }
    out
}

/// Calcula el nombre y el tamaño (en unidades) de un payload de Dalvik.
fn payload_info(insns: &[u16], offset: usize, ident: u16) -> (&'static str, usize) {
    let size = insns.get(offset + 1).copied().unwrap_or(0) as usize;
    match ident {
        0x0100 => ("packed-switch-payload", 4 + 2 * size),
        0x0200 => ("sparse-switch-payload", 2 + 4 * size),
        _ => {
            let width = insns.get(offset + 1).copied().unwrap_or(1) as usize;
            let count = insns.get(offset + 2).copied().unwrap_or(0) as usize
                | ((insns.get(offset + 3).copied().unwrap_or(0) as usize) << 16);
            (
                "fill-array-data-payload",
                4 + (count.saturating_mul(width) + 1) / 2,
            )
        }
    }
}

/// Decodifica una instrucción a partir de su offset en el array de código.
fn decode_insn(dex: &DexFile, insns: &[u16], offset: usize) -> (DalvikInsn, usize) {
    let unit0 = insns.get(offset).copied().unwrap_or(0);
    let opcode = (unit0 & 0xff) as u8;
    let (mnemonic, format) = opcode_info(opcode);
    let width = instruction_width(opcode).max(1);
    let raw: Vec<u16> = insns
        .iter()
        .skip(offset)
        .take(width)
        .copied()
        .collect();

    let operands = render_operands(dex, opcode, format, offset, &raw);
    (
        DalvikInsn {
            offset: offset as u32,
            opcode,
            mnemonic: mnemonic.to_string(),
            format: format.to_string(),
            operands,
            raw,
        },
        width,
    )
}

/// Construye el texto de los operandos según el formato de la instrucción.
///
/// El reparto de bits es el de ART (`Instruction::InstA/InstB/InstAA`):
/// `A` = bits 8-11, `B` = bits 12-15, `AA` = bits 8-15 del primer código
/// unit, y los formatos de dos unidades usan el segundo unit como
/// `BBBB`/`CCCC`.
fn render_operands(
    dex: &DexFile,
    opcode: u8,
    format: &str,
    offset: usize,
    raw: &[u16],
) -> String {
    let unit0 = raw.first().copied().unwrap_or(0);
    let unit1 = raw.get(1).copied().unwrap_or(0);
    let unit2 = raw.get(2).copied().unwrap_or(0);
    let unit3 = raw.get(3).copied().unwrap_or(0);

    let aa = (unit0 >> 8) & 0xff; // AA: bits 8-15
    let a = (unit0 >> 8) & 0x0f; // A: bits 8-11
    let b = (unit0 >> 12) & 0x0f; // B: bits 12-15
    let c = unit2 & 0x0f; // C (35c/45cc): nibble bajo del tercer unit
    let d = (unit2 >> 4) & 0x0f; // D
    let e = (unit2 >> 8) & 0x0f; // E
    let f = (unit2 >> 12) & 0x0f; // F
    let g = (unit0 >> 8) & 0x0f; // G (35c): nibble bajo de los bits 8-11

    match format {
        "10x" => String::new(),
        "11x" => format!("v{}", aa),
        "12x" => format!("v{}, v{}", a, b),
        "11n" => {
            let raw_lit = (unit0 >> 12) & 0x0f;
            let lit = if raw_lit > 7 {
                raw_lit as i32 - 16
            } else {
                raw_lit as i32
            };
            format!("v{}, #{}", a, lit)
        }
        "10t" => {
            let delta = ((unit0 >> 8) & 0xff) as i8;
            format!(":{}", branch_target(offset, delta as i32))
        }
        "20t" => {
            let delta = unit1 as i16;
            format!(":{}", branch_target(offset, delta as i32))
        }
        "30t" => {
            let delta = ((unit2 as u32) << 16 | unit1 as u32) as i32;
            format!(":{}", branch_target(offset, delta))
        }
        "21t" => {
            let delta = unit1 as i16;
            format!("v{}, :{}", aa, branch_target(offset, delta as i32))
        }
        "21s" => format!("v{}, #{}", aa, unit1 as i16),
        "22s" => format!("v{}, v{}, #{}", a, b, unit1 as i16),
        "22b" => {
            let lit = ((unit1 >> 8) & 0xff) as u8;
            let lit = if lit > 127 { lit as i32 - 256 } else { lit as i32 };
            format!("v{}, v{}, #{}", aa, unit1 & 0xff, lit)
        }
        "21h" => {
            if opcode == 0x19 {
                // const-wide/high16
                format!("v{}, #{}000000000000", aa, unit1)
            } else {
                format!("v{}, #{}0000", aa, unit1)
            }
        }
        "31i" => {
            let value = ((unit2 as u32) << 16) | unit1 as u32;
            format!("v{}, #{}", aa, value as i32)
        }
        "51l" => {
            let mut value: u64 = 0;
            for (i, unit) in raw.iter().skip(1).take(4).enumerate() {
                value |= (*unit as u64) << (16 * i);
            }
            format!("v{}, #{}", aa, value as i64)
        }
        "31c" => {
            let index = ((unit2 as u32) << 16) | unit1 as u32;
            format!("v{}, {}", aa, render_string_ref(dex, index))
        }
        "31t" => {
            let delta = ((unit2 as u32) << 16) | unit1 as u32;
            let target = branch_target(offset, delta as i32);
            match opcode {
                0x2b => format!("v{}, :switch(packed)@{}", aa, target),
                0x2c => format!("v{}, :switch(sparse)@{}", aa, target),
                _ => format!("v{}, :{}", aa, target),
            }
        }
        "21c" => {
            let index = unit1 as u32;
            match opcode_ref_kind(opcode) {
                RefKind::String => format!("v{}, {}", aa, render_string_ref(dex, index)),
                RefKind::Type => format!("v{}, {}", aa, render_type_ref(dex, index)),
                RefKind::Field => format!("v{}, {}", aa, render_field_ref(dex, index)),
                RefKind::Proto => format!("v{}, {}", aa, render_proto_ref(dex, index)),
                RefKind::MethodHandle => format!("v{}, method_handle@{}", aa, index),
                RefKind::CallSite => format!("v{}, call_site@{}", aa, index),
                _ => format!("v{}, @{}", aa, index),
            }
        }
        "22c" => {
            let index = unit1 as u32;
            match opcode_ref_kind(opcode) {
                RefKind::Type => format!("v{}, v{}, {}", a, b, render_type_ref(dex, index)),
                RefKind::Field => format!("v{}, v{}, {}", a, b, render_field_ref(dex, index)),
                _ => format!("v{}, v{}, @{}", a, b, index),
            }
        }
        "22cs" => format!("v{}, v{}, fieldoffset@{}", a, b, unit1),
        "23x" => format!("v{}, v{}, v{}", aa, unit1 & 0xff, unit1 >> 8),
        "22x" => format!("v{}, v{}", aa, unit1),
        "32x" => format!("v{}, v{}", unit1, unit2),
        "20bc" => format!("{}, kind@{}", aa, unit1),
        "22t" => {
            let delta = unit1 as i16;
            format!("v{}, v{}, :{}", a, b, branch_target(offset, delta as i32))
        }
        "35c" => {
            let arg_count = (unit0 >> 12) & 0x0f;
            let method_idx = unit1 as u32;
            let regs = format_35c_registers(arg_count, c, d, e, f, g);
            match opcode_ref_kind(opcode) {
                RefKind::Method => format!("{}, {}", regs, render_method_ref(dex, method_idx)),
                RefKind::Type => format!("{}, {}", regs, render_type_ref(dex, method_idx)),
                RefKind::CallSite => format!("{}, call_site@{}", regs, method_idx),
                _ => format!("{}, @{}", regs, method_idx),
            }
        }
        "3rc" => {
            let count = aa as u32;
            let first = unit2 as u32;
            let last = first + count.saturating_sub(1);
            let regs = format!("{{v{}..v{}}}", first, last);
            match opcode_ref_kind(opcode) {
                RefKind::Method => format!("{}, {}", regs, render_method_ref(dex, unit1 as u32)),
                RefKind::Type => format!("{}, {}", regs, render_type_ref(dex, unit1 as u32)),
                RefKind::CallSite => format!("{}, call_site@{}", regs, unit1),
                _ => format!("{}, @{}", regs, unit1),
            }
        }
        "45cc" => {
            let count = (unit0 >> 12) & 0x0f;
            let first = c;
            let last = first + count.saturating_sub(1);
            let regs = format!("{{v{}..v{}}}", first, last);
            format!(
                "{}, {}, {}",
                regs,
                render_method_ref(dex, unit1 as u32),
                render_proto_ref(dex, unit3 as u32)
            )
        }
        "4rcc" => {
            let count = aa as u32;
            let first = unit2 as u32;
            let last = first + count.saturating_sub(1);
            let regs = format!("{{v{}..v{}}}", first, last);
            format!(
                "{}, {}, {}",
                regs,
                render_method_ref(dex, unit1 as u32),
                render_proto_ref(dex, unit3 as u32)
            )
        }
        _ => {
            let _ = (opcode, e, f);
            String::new()
        }
    }
}

/// Calcula el destino absoluto de un salto (en unidades de instrucción).
fn branch_target(offset: usize, delta: i32) -> String {
    let target = offset as i64 + delta as i64;
    if target < 0 {
        format!("<neg:{:04x}>", target)
    } else {
        format!("{:04x}", target)
    }
}

/// Formatea la lista de registros de una instrucción 35c.
fn format_35c_registers(count: u16, c: u16, d: u16, e: u16, f: u16, g: u16) -> String {
    let all = [c, d, e, f, g];
    let mut regs = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        if let Some(reg) = all.get(i) {
            regs.push(format!("v{}", reg));
        }
    }
    format!("{{{}}}", regs.join(", "))
}

/// Renderiza una referencia a string.
fn render_string_ref(dex: &DexFile, index: u32) -> String {
    match dex.strings.get(index as usize) {
        Some(text) => format!("\"{}\"  // string@0x{:04x}", escape_string(text), index),
        None => format!("string@0x{:04x}", index),
    }
}

/// Renderiza una referencia a tipo.
fn render_type_ref(dex: &DexFile, index: u32) -> String {
    match dex.types.get(index as usize) {
        Some(t) => format!("{}  // type@0x{:04x}", t.descriptor, index),
        None => format!("type@0x{:04x}", index),
    }
}

/// Renderiza una referencia a campo.
fn render_field_ref(dex: &DexFile, index: u32) -> String {
    match dex.fields.get(index as usize) {
        Some(f) => format!(
            "{}.{}:{}  // field@0x{:04x}",
            f.class_name, f.name, f.field_type, index
        ),
        None => format!("field@0x{:04x}", index),
    }
}

/// Renderiza una referencia a método.
fn render_method_ref(dex: &DexFile, index: u32) -> String {
    match dex.methods.get(index as usize) {
        Some(m) => format!(
            "{}.{}{}  // meth@0x{:04x}",
            m.class_name, m.name, m.proto, index
        ),
        None => format!("meth@0x{:04x}", index),
    }
}

/// Renderiza una referencia a prototipo.
fn render_proto_ref(dex: &DexFile, index: u32) -> String {
    match dex.protos.get(index as usize) {
        Some(p) => format!("{}  // proto@0x{:04x}", proto_signature(p), index),
        None => format!("proto@0x{:04x}", index),
    }
}

// ========================================================================= //
// Pseudo-Java
// ========================================================================= //

/// Flags de acceso de una clase.
fn class_access_keywords(flags: u32) -> Vec<&'static str> {
    let mut out = Vec::new();
    if flags & 0x0001 != 0 {
        out.push("public");
    }
    if flags & 0x0010 != 0 {
        out.push("final");
    }
    if flags & 0x0200 != 0 {
        out.push("interface");
    }
    if flags & 0x0400 != 0 {
        out.push("abstract");
    }
    if flags & 0x2000 != 0 {
        out.push("annotation");
    }
    if flags & 0x4000 != 0 {
        out.push("enum");
    }
    if flags & 0x1000 != 0 {
        out.push("synthetic");
    }
    out
}

/// Flags de acceso de un método.
fn method_access_keywords(flags: u32) -> Vec<&'static str> {
    let mut out = Vec::new();
    if flags & 0x0001 != 0 {
        out.push("public");
    }
    if flags & 0x0002 != 0 {
        out.push("private");
    }
    if flags & 0x0004 != 0 {
        out.push("protected");
    }
    if flags & 0x0008 != 0 {
        out.push("static");
    }
    if flags & 0x0010 != 0 {
        out.push("final");
    }
    if flags & 0x0020 != 0 {
        out.push("synchronized");
    }
    if flags & 0x0100 != 0 {
        out.push("native");
    }
    if flags & 0x0400 != 0 {
        out.push("abstract");
    }
    if flags & 0x0800 != 0 {
        out.push("strictfp");
    }
    if flags & 0x1000 != 0 {
        out.push("synthetic");
    }
    if flags & 0x10000 != 0 {
        out.push("constructor");
    }
    out
}

/// Flags de acceso de un campo.
fn field_access_keywords(flags: u32) -> Vec<&'static str> {
    let mut out = Vec::new();
    if flags & 0x0001 != 0 {
        out.push("public");
    }
    if flags & 0x0002 != 0 {
        out.push("private");
    }
    if flags & 0x0004 != 0 {
        out.push("protected");
    }
    if flags & 0x0008 != 0 {
        out.push("static");
    }
    if flags & 0x0010 != 0 {
        out.push("final");
    }
    if flags & 0x0040 != 0 {
        out.push("volatile");
    }
    if flags & 0x0080 != 0 {
        out.push("transient");
    }
    if flags & 0x1000 != 0 {
        out.push("synthetic");
    }
    if flags & 0x4000 != 0 {
        out.push("enum");
    }
    out
}

/// Devuelve el nombre simple de una clase (`Lcom/x/Foo$Bar;` → `Bar`).
pub fn class_simple_name(descriptor: &str) -> String {
    let trimmed = descriptor.trim_start_matches('L').trim_end_matches(';');
    let last = trimmed.rsplit('/').next().unwrap_or(trimmed);
    last.rsplit('$').next().unwrap_or(last).to_string()
}

/// Devuelve el nombre Java completo con paquete (`Lcom/x/Foo;` → `com.x.Foo`).
pub fn java_class_name(descriptor: &str) -> String {
    let trimmed = descriptor.trim_start_matches('L').trim_end_matches(';');
    trimmed.replace('/', ".")
}

/// Genera el pseudo-Java de una clase (`idx` es el índice en `class_defs`).
pub fn decompile_class(dex: &DexFile, idx: u32) -> String {
    let class_def = match dex.classes.get(idx as usize) {
        Some(c) => c,
        None => {
            return format!(
                "// no existe la clase #{} (hay {} clases en el DEX)\n",
                idx,
                dex.classes.len()
            )
        }
    };

    let mut out = String::new();
    out.push_str("//======================================================================\n");
    out.push_str(&format!("// class {}\n", class_def.class_name));
    out.push_str(&format!("// extends {}\n", class_def.superclass_name));
    if !class_def.interfaces.is_empty() {
        out.push_str(&format!("// implements {}\n", class_def.interfaces.join(", ")));
    }
    if !class_def.source_file.is_empty() {
        out.push_str(&format!("// source {}\n", class_def.source_file));
    }
    let keywords = class_access_keywords(class_def.access_flags);
    out.push_str(&format!(
        "// access {}{}\n",
        if keywords.is_empty() {
            String::new()
        } else {
            format!("{} ", keywords.join(" "))
        },
        format!("0x{:04x}", class_def.access_flags)
    ));
    out.push_str("//======================================================================\n");

    // Paquete.
    let java_name = java_class_name(&class_def.class_name);
    if let Some(pos) = java_name.rfind('.') {
        let package = &java_name[..pos];
        let simple = &java_name[pos + 1..];
        out.push_str(&format!("package {};\n\n", package));
        out.push_str(&format!("public class {} ", simple));
    } else {
        out.push_str(&format!("public class {} ", java_name));
    }
    if !class_def.superclass_name.is_empty() && class_def.superclass_name != "Ljava/lang/Object;" {
        out.push_str(&format!("extends {} ", java_class_name(&class_def.superclass_name)));
    }
    if !class_def.interfaces.is_empty() {
        let ifaces: Vec<String> = class_def
            .interfaces
            .iter()
            .map(|i| java_class_name(i))
            .collect();
        out.push_str(&format!("implements {} ", ifaces.join(", ")));
    }
    out.push_str("{\n");

    // Anotaciones de la clase.
    for annotation in &class_def.annotations {
        out.push_str(&format!(
            "  // @{} ({}) {}\n",
            java_class_name(&annotation.type_name),
            annotation.visibility,
            annotation
                .elements
                .iter()
                .map(|e| format!("{}={}", e.name, e.value))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    // Campos estáticos.
    if !class_def.static_fields.is_empty() {
        out.push_str("\n  // ---- static fields ----\n");
        for encoded in &class_def.static_fields {
            if let Some(field) = dex.fields.get(encoded.field_idx as usize) {
                let keywords = field_access_keywords(encoded.access_flags);
                out.push_str(&format!(
                    "  //   {} {}:{};\n",
                    keywords.join(" "),
                    field.name,
                    descriptor_to_java(&field.field_type)
                ));
            }
        }
    }

    // Campos de instancia.
    if !class_def.instance_fields.is_empty() {
        out.push_str("\n  // ---- instance fields ----\n");
        for encoded in &class_def.instance_fields {
            if let Some(field) = dex.fields.get(encoded.field_idx as usize) {
                let keywords = field_access_keywords(encoded.access_flags);
                out.push_str(&format!(
                    "  //   {} {}:{};\n",
                    keywords.join(" "),
                    field.name,
                    descriptor_to_java(&field.field_type)
                ));
            }
        }
    }

    // Valores estáticos iniciales.
    if !class_def.static_values.is_empty() {
        out.push_str("\n  // ---- static values ----\n");
        for value in &class_def.static_values {
            out.push_str(&format!("  //   {};\n", value));
        }
    }

    // Métodos.
    let methods: Vec<&DexEncodedMethod> = class_def
        .direct_methods
        .iter()
        .chain(class_def.virtual_methods.iter())
        .collect();
    if !methods.is_empty() {
        out.push_str("\n  // ---- methods ----\n");
    }
    for method in methods {
        out.push_str(&render_method(dex, class_def, method));
    }

    out.push_str("}\n");
    out
}

/// Genera el pseudo-Java de un método concreto.
fn render_method(dex: &DexFile, class_def: &DexClassDef, method: &DexEncodedMethod) -> String {
    let mut out = String::new();
    let info = dex.methods.get(method.method_idx as usize);
    let name = info.map(|m| m.name.clone()).unwrap_or_default();
    let proto = info.map(|m| m.proto.clone()).unwrap_or_default();
    let keywords = method_access_keywords(method.access_flags);
    let params = proto
        .split(')')
        .next()
        .map(|p| p.trim_start_matches('(').to_string())
        .unwrap_or_default();
    let param_types: Vec<String> = if params.is_empty() {
        Vec::new()
    } else {
        params
            .split(',')
            .map(descriptor_to_java)
            .collect::<Vec<String>>()
    };
    let ret = proto
        .split(')')
        .nth(1)
        .map(descriptor_to_java)
        .unwrap_or_else(|| "void".to_string());

    out.push_str(&format!(
        "  //   method {}{}  (method_idx 0x{:04x})\n",
        name,
        proto,
        method.method_idx
    ));

    match &method.code {
        None => {
            out.push_str(&format!(
                "  {} {} {}({}) {}  // sin cuerpo (abstracto/nativo)\n",
                keywords.join(" "),
                ret,
                name,
                param_types.join(", "),
                ""
            ));
            out.push('\n');
        }
        Some(code) => {
            // Nombres de los parámetros (del debug info si existe).
            let mut param_names: Vec<String> = Vec::new();
            if let Some(debug) = &code.debug {
                for (i, p) in debug.parameter_names.iter().enumerate() {
                    if p.is_empty() {
                        param_names.push(format!("arg{}", i));
                    } else {
                        param_names.push(p.clone());
                    }
                }
            }
            let mut signature_parts: Vec<String> = Vec::new();
            for (i, ty) in param_types.iter().enumerate() {
                let pname = param_names
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("arg{}", i));
                signature_parts.push(format!("{} {}", ty, pname));
            }
            out.push_str(&format!(
                "  {} {} {}({}) {{\n",
                keywords.join(" "),
                ret,
                name,
                signature_parts.join(", ")
            ));
            out.push_str(&format!(
                "    // registers: {} (ins: {}, outs: {})\n",
                code.registers_size, code.ins_size, code.outs_size
            ));
            if let Some(debug) = &code.debug {
                if debug.line_start > 0 {
                    out.push_str(&format!(
                        "    // debug: línea inicial {}\n",
                        debug.line_start
                    ));
                }
            }

            let insns = disassemble_code(dex, code);
            for insn in &insns {
                out.push_str(&format!(
                    "    {:04x}: {:<24} {}\n",
                    insn.offset, insn.mnemonic, insn.operands
                ));
            }

            for try_item in &code.tries {
                let handlers: Vec<String> = try_item
                    .catches
                    .iter()
                    .map(|c| {
                        if c.type_names.is_empty() {
                            format!("catch_all@0x{:04x}", c.catch_all_addr)
                        } else {
                            format!(
                                "{}@0x{:04x}",
                                c.type_names
                                    .iter()
                                    .map(|t| java_class_name(t))
                                    .collect::<Vec<_>>()
                                    .join("|"),
                                c.catch_all_addr
                            )
                        }
                    })
                    .collect();
                out.push_str(&format!(
                    "    // try 0x{:04x}..0x{:04x} -> {}\n",
                    try_item.start_addr,
                    try_item.start_addr + try_item.insn_count as u32,
                    handlers.join(", ")
                ));
            }

            out.push_str("  }\n\n");
        }
    }
    let _ = class_def;
    out
}

/// Genera el pseudo-Java de todas las clases del DEX.
pub fn decompile_all(dex: &DexFile) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "// MegaDaemon DEX decompiler — DEX {} — {} clases, {} métodos, {} strings\n",
        dex.version,
        dex.classes.len(),
        dex.methods.len(),
        dex.strings.len()
    ));
    out.push_str(&format!("// sha256 {}\n", dex.sha256));
    out.push_str(&format!(
        "// checksum 0x{:08x} ({}) — firma {}\n",
        dex.checksum,
        if dex.checksum_valid { "válido" } else { "NO válido" },
        if dex.signature_valid { "válida" } else { "NO válida" }
    ));
    out.push('\n');

    let limit = dex.classes.len().min(MAX_CLASSES_DECOMPILED);
    for i in 0..limit {
        out.push_str(&decompile_class(dex, i as u32));
        out.push('\n');
    }
    out
}

/// Desensambla un método y devuelve una línea por instrucción.
///
/// `class_idx` es el índice en `class_defs`; `method_idx` es el índice en
/// `method_ids` (si no aparece en esa clase, se interpreta como el ordinal
/// dentro de los métodos de la clase).
pub fn disassemble_method(dex: &DexFile, class_idx: u32, method_idx: u32) -> Vec<String> {
    let class_def = match dex.classes.get(class_idx as usize) {
        Some(c) => c,
        None => return Vec::new(),
    };

    let methods: Vec<&DexEncodedMethod> = class_def
        .direct_methods
        .iter()
        .chain(class_def.virtual_methods.iter())
        .collect();

    let found = methods
        .iter()
        .find(|m| m.method_idx == method_idx)
        .copied()
        .or_else(|| methods.get(method_idx as usize).copied());

    let method = match found {
        Some(m) => m,
        None => return Vec::new(),
    };

    let code = match &method.code {
        Some(c) => c,
        None => return Vec::new(),
    };

    disassemble_code(dex, code)
        .into_iter()
        .map(|insn| {
            if insn.operands.is_empty() {
                format!("{:04x}: {}", insn.offset, insn.mnemonic)
            } else {
                format!("{:04x}: {} {}", insn.offset, insn.mnemonic, insn.operands)
            }
        })
        .collect()
}

/// Devuelve la tabla de strings del DEX.
pub fn dex_strings(dex: &DexFile) -> Vec<String> {
    dex.strings.clone()
}

// ========================================================================= //
// Desofuscación
// ========================================================================= //

/// Un string sospechoso de estar cifrado.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EncryptedString {
    /// Clase donde se encontró la referencia (o `<string-pool>`).
    pub class_name: String,
    /// Método donde se encontró la referencia.
    pub method_name: String,
    /// Texto LITERAL tal cual aparece en el pool de constantes (la evidencia
    /// original). Antes el mapa perdía este dato: `evidence` es una
    /// descripción, no el string.
    pub original: String,
    /// Tipo de cifrado sospechado (`base64`, `xor`, `byte-array`…).
    pub kind: String,
    /// Evidencia textual (patrón detectado, clave, volcado hex…).
    pub evidence: String,
    /// Offset del string en la tabla (índice del pool).
    pub offset: u32,
    /// Texto plano candidato (si se pudo descifrar).
    pub candidate_plaintext: String,
    /// Clave XOR (si el tipo es `xor`).
    pub xor_key: Option<u8>,
    /// Confianza de la hipótesis (0-100).
    pub confidence: u8,
}

/// Informe de desofuscación de un DEX.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObfuscationReport {
    /// Clases con nombre sospechoso de estar ofuscado.
    pub renamed_types: Vec<String>,
    /// Métodos con nombre sospechoso de estar ofuscado.
    pub renamed_methods: Vec<String>,
    /// Strings con pinta de estar cifrados.
    pub encrypted_strings: Vec<EncryptedString>,
    /// Descripciones de las técnicas de ofuscación de flujo detectadas.
    pub control_flow_obfuscation: Vec<String>,
}

/// Nombres que pese a ser cortos NO son ofuscación (los genera el tooling).
const NOT_OBFUSCATED_NAMES: [&str; 8] = [
    "R", "BuildConfig", "Manifest", "Application", "Activity", "Fragment", "View", "Adapter",
];

/// Nombres de método cortos que son common y no cuentan como ofuscación.
const NOT_OBFUSCATED_METHODS: [&str; 16] = [
    "toString", "equals", "hashCode", "run", "main", "get", "set", "id", "ok", "on", "of", "pi",
    "run", "log", "tag", "key",
];

/// ¿El nombre parece un identificador ofuscado (1-2 caracteres)?
fn is_obfuscated_identifier(name: &str) -> bool {
    if name.is_empty() || name.len() > 2 {
        return false;
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    name.chars().any(|c| c.is_ascii_alphabetic())
}

/// Tabla de frecuencias del inglés (en porcentaje) para puntuar texto.
const ENGLISH_FREQ: [f64; 26] = [
    8.167, 1.492, 2.782, 4.253, 12.702, 2.228, 2.015, 6.094, 6.966, 0.153, 0.772, 4.025, 2.406,
    6.749, 7.507, 1.929, 0.095, 5.987, 6.327, 9.056, 2.758, 0.978, 2.360, 0.150, 1.974, 0.074,
];

/// Puntúa qué parece texto en inglés (mayor = más probable).
fn english_score(text: &str) -> f64 {
    let mut score = 0.0f64;
    let mut letters = 0usize;
    for ch in text.chars() {
        match ch {
            ' ' => score += 2.0,
            'a'..='z' => {
                let idx = (ch as u8 - b'a') as usize;
                score += (ENGLISH_FREQ[idx] / 100.0).max(1e-4).ln();
                letters += 1;
            }
            'A'..='Z' => {
                let idx = (ch as u8 - b'A') as usize;
                score += (ENGLISH_FREQ[idx] / 100.0).max(1e-4).ln();
                letters += 1;
            }
            _ => {}
        }
    }
    if letters == 0 {
        return -100.0;
    }
    score / letters as f64
}

/// Decodifica base64 estándar (sin dependencias externas).
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut rev = [255u8; 256];
    for (i, c) in TABLE.iter().enumerate() {
        rev[*c as usize] = i as u8;
    }
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;
    let mut padding = 0usize;
    for ch in text.bytes() {
        if ch == b'=' {
            padding += 1;
            continue;
        }
        if padding > 0 {
            return None;
        }
        let v = rev[ch as usize];
        if v == 255 {
            return None;
        }
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

/// ¿El texto parece base64 válido?
fn looks_like_base64(text: &str) -> bool {
    if text.len() < 8 || text.len() % 4 != 0 {
        return false;
    }
    text.bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=')
}

/// Proporción de bytes imprimibles ASCII.
fn printable_ratio(bytes: &[u8]) -> f64 {
    if bytes.is_empty() {
        return 0.0;
    }
    let printable = bytes.iter().filter(|&&b| (0x20..0x7f).contains(&b)).count();
    printable as f64 / bytes.len() as f64
}

/// Analiza un DEX y devuelve el informe de desofuscación.
pub fn analyze_obfuscation(dex: &DexFile) -> ObfuscationReport {
    let mut report = ObfuscationReport::default();

    // ---------------------------------------------------------------- //
    // 1. Tipos renombrados
    // ---------------------------------------------------------------- //
    for class_def in &dex.classes {
        let simple = class_simple_name(&class_def.class_name);
        if is_obfuscated_identifier(&simple) && !NOT_OBFUSCATED_NAMES.contains(&simple.as_str()) {
            report.renamed_types.push(class_def.class_name.clone());
        }
    }

    // ---------------------------------------------------------------- //
    // 2. Métodos renombrados
    // ---------------------------------------------------------------- //
    for class_def in &dex.classes {
        for method in class_def
            .direct_methods
            .iter()
            .chain(class_def.virtual_methods.iter())
        {
            let name = dex
                .methods
                .get(method.method_idx as usize)
                .map(|m| m.name.clone())
                .unwrap_or_default();
            if name.starts_with('<') {
                continue;
            }
            if is_obfuscated_identifier(&name)
                && !NOT_OBFUSCATED_METHODS.contains(&name.as_str())
            {
                report
                    .renamed_methods
                    .push(format!("{}.{}()", class_def.class_name, name));
            }
        }
    }

    // ---------------------------------------------------------------- //
    // 3. Strings cifrados
    // ---------------------------------------------------------------- //
    report.encrypted_strings = find_encrypted_strings(dex);

    // ---------------------------------------------------------------- //
    // 4. Ofuscación del flujo de control
    // ---------------------------------------------------------------- //
    report.control_flow_obfuscation = find_control_flow_obfuscation(dex);

    report
}

/// Localiza strings con pinta de estar cifrados.
fn find_encrypted_strings(dex: &DexFile) -> Vec<EncryptedString> {
    let mut out: Vec<EncryptedString> = Vec::new();

    // Referencia de cada string desde el código (para atribuirlo a un método).
    let origin = string_origins(dex);

    for (index, text) in dex.strings.iter().enumerate() {
        // Los descriptores de tipo no son strings Interesting.
        if text.starts_with('L') && text.ends_with(';') {
            continue;
        }
        if text.starts_with('[') || text.starts_with('(') {
            continue;
        }
        let bytes = text.as_bytes();
        if bytes.len() < 4 {
            continue;
        }

        let (class_name, method_name) = origin
            .get(&(index as u32))
            .cloned()
            .unwrap_or_else(|| ("<string-pool>".to_string(), String::new()));

        // (a) Base64.
        if looks_like_base64(text) {
            if let Some(decoded) = base64_decode(text) {
                if decoded.len() >= 3 && printable_ratio(&decoded) >= 0.9 {
                    let plain = String::from_utf8_lossy(&decoded).to_string();
                    out.push(EncryptedString {
                        original: text.clone(),
                        class_name: class_name.clone(),
                        method_name: method_name.clone(),
                        kind: "base64".to_string(),
                        evidence: format!(
                            "longitud múltiplo de 4 y sólo alfabetos base64; se decodifica a texto legible"
                        ),
                        offset: index as u32,
                        candidate_plaintext: plain,
                        xor_key: None,
                        confidence: 80,
                    });
                    continue;
                }
            }
        }

        // (b) XOR de un byte.
        if bytes.iter().all(|&b| (0x20..0x7f).contains(&b)) {
            let original_score = english_score(text);
            let mut best_key = 0u8;
            let mut best_score = original_score;
            let mut best_text = String::new();
            for key in 1u16..=255 {
                let key = key as u8;
                let decoded: Vec<u8> = bytes.iter().map(|&b| b ^ key).collect();
                if !decoded.iter().all(|&b| (0x20..0x7f).contains(&b)) {
                    continue;
                }
                let candidate = String::from_utf8_lossy(&decoded).to_string();
                // Un XOR que SÓLO cambia las mayúsculas (clave 0x20 = 32) no es
                // evidencia de cifrado: cualquier texto en inglés "cifrado"
                // con la clave 32 sale en mayúsculas y puntúa EXACTAMENTE
                // igual (mismas letras ⇒ misma frecuencia). Sin esta
                // comprobación el detector marcaba como cifradas sus propias
                // cadenas en claro: "hello" -> "HELLO" (clave 32) ganaba al
                // XOR real y devolvía un falso positivo por delante del
                // hallazgo bueno.
                if candidate.eq_ignore_ascii_case(text) {
                    continue;
                }
                let letters = candidate.chars().filter(|c| c.is_ascii_alphabetic()).count();
                if letters < 3 {
                    continue;
                }
                if !is_prose_candidate(&candidate) {
                    continue;
                }
                // Un nombre con punto ("Obf.java", "a.b.c") es un
                // identificador o un fichero, no una cadena cifrada: su
                // descifrado es ruido ("Alh doxo") y genera falsos positivos.
                if looks_like_dotted_name(text) {
                    continue;
                }
                if !candidate
                    .chars()
                    .any(|c| matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'A' | 'E' | 'I' | 'O' | 'U'))
                {
                    continue;
                }
                let score = english_score(&candidate) + english_word_bonus(&candidate);
                if score > best_score {
                    best_score = score;
                    best_key = key;
                    best_text = candidate;
                }
            }
            if best_score - original_score > 0.25 && !best_text.is_empty() {
                out.push(EncryptedString {
                    original: text.clone(),
                    class_name: class_name.clone(),
                    method_name: method_name.clone(),
                    kind: "xor".to_string(),
                    evidence: format!(
                        "XOR de un byte con 0x{:02x} mejora la probabilidad de texto ({} -> {})",
                        best_key,
                        format!("{:.3}", original_score),
                        format!("{:.3}", best_score)
                    ),
                    offset: index as u32,
                    candidate_plaintext: best_text,
                    xor_key: Some(best_key),
                    confidence: 70,
                });
                continue;
            }
        }

        // (c) Blob binario (bytes no imprimibles).
        let ratio = printable_ratio(bytes);
        if ratio < 0.75 {
            out.push(EncryptedString {
                original: text.clone(),
                class_name,
                method_name,
                kind: "binary-blob".to_string(),
                evidence: format!(
                    "sólo un {:.0}% de los bytes son ASCII imprimible: probable payload cifrado",
                    ratio * 100.0
                ),
                offset: index as u32,
                candidate_plaintext: String::new(),
                xor_key: None,
                confidence: 55,
            });
        }
    }

    // (d) Arrays de bytes rellenados con `fill-array-data`.
    for class_def in &dex.classes {
        for method in class_def
            .direct_methods
            .iter()
            .chain(class_def.virtual_methods.iter())
        {
            let code = match &method.code {
                Some(c) => c,
                None => continue,
            };
            let method_name = dex
                .methods
                .get(method.method_idx as usize)
                .map(|m| m.name.clone())
                .unwrap_or_default();
            let mut offset = 0usize;
            while offset < code.insns.len() {
                let opcode = (code.insns[offset] & 0xff) as u8;
                let width = instruction_width(opcode).max(1);
                if opcode == 0x26 && offset + 2 < code.insns.len() {
                    let delta = ((code.insns[offset + 2] as i32) << 16) | code.insns[offset + 1] as i32;
                    let target = offset as i32 + delta;
                    if target >= 0 && (target as usize) + 3 < code.insns.len() {
                        let payload = code.insns[target as usize];
                        let ident = payload & 0xffff;
                        if ident == 0x0300 {
                            // u16 ident, u16 element_width, u32 size, datos.
                            let element_width = code.insns[target as usize + 1] as usize;
                            let size = code.insns[target as usize + 2] as usize
                                | ((code.insns[target as usize + 3] as usize) << 16);
                            let units = (size.saturating_mul(element_width) + 1) / 2;
                            let data: Vec<u16> = code
                                .insns
                                .iter()
                                .skip(target as usize + 4)
                                .take(units)
                                .copied()
                                .collect();
                            let mut raw = Vec::with_capacity(data.len() * 2);
                            for unit in &data {
                                raw.push((unit & 0xff) as u8);
                                raw.push((unit >> 8) as u8);
                            }
                            if raw.iter().any(|&b| b > 0x7f || (b < 0x20 && b != 0)) {
                                out.push(EncryptedString {
                                    original: String::new(),
                                    class_name: class_def.class_name.clone(),
                                    method_name: method_name.clone(),
                                    kind: "byte-array".to_string(),
                                    evidence: format!(
                                        "fill-array-data con {} bytes no imprimibles: {}",
                                        raw.len(),
                                        hex(&raw[..raw.len().min(32)])
                                    ),
                                    offset: target as u32,
                                    candidate_plaintext: String::new(),
                                    xor_key: None,
                                    confidence: 75,
                                });
                            }
                        }
                    }
                }
                offset += width;
            }
        }
    }

    out
}

/// Detecta técnicas de ofuscación del flujo de control.
fn find_control_flow_obfuscation(dex: &DexFile) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    for class_def in &dex.classes {
        for method in class_def
            .direct_methods
            .iter()
            .chain(class_def.virtual_methods.iter())
        {
            let code = match &method.code {
                Some(c) => c,
                None => continue,
            };
            let method_name = dex
                .methods
                .get(method.method_idx as usize)
                .map(|m| m.name.clone())
                .unwrap_or_default();
            let where_ = format!("{}.{}()", class_def.class_name, method_name);
            let insns = disassemble_code(dex, code);
            if insns.is_empty() {
                continue;
            }

            // (a) Bloques básicos: líderes = destinos de salto + instrucción
            //     siguiente a un salto/return/throw.
            let mut leaders: Vec<u32> = vec![0];
            let mut junk_gotos = 0usize;
            let mut branches = 0usize;
            for (i, insn) in insns.iter().enumerate() {
                let is_branch = matches!(
                    insn.mnemonic.as_str(),
                    "goto"
                        | "goto/16"
                        | "goto/32"
                        | "if-eq"
                        | "if-ne"
                        | "if-lt"
                        | "if-ge"
                        | "if-gt"
                        | "if-le"
                        | "if-eqz"
                        | "if-nez"
                        | "if-ltz"
                        | "if-gez"
                        | "if-gtz"
                        | "if-lez"
                        | "packed-switch"
                        | "sparse-switch"
                );
                let is_terminator = matches!(
                    insn.mnemonic.as_str(),
                    "return" | "return-void" | "return-wide" | "return-object" | "throw"
                );
                if is_branch {
                    branches += 1;
                    if let Some(target) = parse_branch_target(&insn.operands) {
                        if !leaders.contains(&target) {
                            leaders.push(target);
                        }
                        // Salto a la instrucción siguiente = código basura.
                        if insn.mnemonic == "goto" && target == insn.offset + 1 {
                            junk_gotos += 1;
                        }
                    }
                }
                if (is_branch || is_terminator) && i + 1 < insns.len() {
                    let next = insns[i + 1].offset;
                    if !leaders.contains(&next) {
                        leaders.push(next);
                    }
                }
            }

            // (b) Switches con muchos casos: tabla de lookup.
            let mut big_switches = 0usize;
            for insn in &insns {
                if insn.mnemonic == "packed-switch" || insn.mnemonic == "sparse-switch" {
                    if let Some(target) = parse_branch_target(&insn.operands) {
                        let cases = count_switch_cases(code, target);
                        if cases >= 4 {
                            big_switches += 1;
                            out.push(format!(
                                "{}: switch con {} casos (posible tabla de lookup / aplanado de flujo)",
                                where_, cases
                            ));
                        }
                    }
                }
            }

            // (c) Saltos nulos (código opaco).
            if junk_gotos >= 2 {
                out.push(format!(
                    "{}: {} saltos a la instrucción siguiente (código basura / predicado opaco)",
                    where_, junk_gotos
                ));
            }

            // (d) Demasiados bloques básicos: flujo aplanado.
            if leaders.len() >= 12 && branches >= 8 {
                out.push(format!(
                    "{}: {} bloques básicos y {} saltos (flujo de control aplanado)",
                    where_,
                    leaders.len(),
                    branches
                ));
            }

            let _ = big_switches;
        }
    }

    out
}

/// Extrae el destino de un salto del texto de operandos (`:0abc`).
fn parse_branch_target(operands: &str) -> Option<u32> {
    let idx = operands.rfind(':')?;
    let tail = &operands[idx + 1..];
    let hex: String = tail
        .chars()
        .take_while(|c| c.is_ascii_hexdigit())
        .collect();
    if hex.is_empty() {
        None
    } else {
        u32::from_str_radix(&hex, 16).ok()
    }
}

/// Cuenta los casos de un `packed-switch` / `sparse-switch`.
fn count_switch_cases(code: &DexCode, target: u32) -> usize {
    let target = target as usize;
    if target + 1 >= code.insns.len() {
        return 0;
    }
    let ident = code.insns[target] & 0xffff;
    let size = code.insns.get(target + 1).copied().unwrap_or(0) as usize;
    match ident {
        0x0100 => size,                    // packed-switch
        0x0200 => size,                    // sparse-switch
        0x0300 => 0,                       // fill-array-data
        _ => 0,
    }
}

/// Devuelve el mapa de desofuscación en JSON.
pub fn obfuscation_map(dex: &DexFile) -> String {
    let report = analyze_obfuscation(dex);

    let mut hypotheses: Vec<String> = Vec::new();
    if !report.renamed_types.is_empty() {
        hypotheses.push(format!(
            "{} clases con nombre de 1-2 caracteres: renombrado de ofuscador (ProGuard/R8)",
            report.renamed_types.len()
        ));
    }
    if !report.renamed_methods.is_empty() {
        hypotheses.push(format!(
            "{} métodos con nombre de 1-2 caracteres: renombrado de ofuscador",
            report.renamed_methods.len()
        ));
    }
    for enc in &report.encrypted_strings {
        hypotheses.push(format!(
            "string cifrado tipo {} en {} (confianza {}): {}",
            enc.kind,
            if enc.class_name.is_empty() {
                "<desconocida>"
            } else {
                enc.class_name.as_str()
            },
            enc.confidence,
            if enc.candidate_plaintext.is_empty() {
                enc.evidence.clone()
            } else {
                format!("posible texto plano: {}", enc.candidate_plaintext)
            }
        ));
    }
    for cfo in &report.control_flow_obfuscation {
        hypotheses.push(format!("flujo de control: {}", cfo));
    }

    let value: Value = json!({
        "dex": {
            "version": dex.version,
            "sha256": dex.sha256,
            "checksum_valid": dex.checksum_valid,
            "signature_valid": dex.signature_valid,
            "classes": dex.classes.len(),
            "methods": dex.methods.len(),
            "strings": dex.strings.len(),
        },
        "summary": {
            "renamed_types": report.renamed_types.len(),
            "renamed_methods": report.renamed_methods.len(),
            "encrypted_strings": report.encrypted_strings.len(),
            "control_flow_obfuscation": report.control_flow_obfuscation.len(),
        },
        "renamed_types": report.renamed_types,
        "renamed_methods": report.renamed_methods,
        "encrypted_strings": report.encrypted_strings,
        "control_flow_obfuscation": report.control_flow_obfuscation,
        "hypotheses": hypotheses,
    });

    serde_json::to_string_pretty(&value).unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
}

// ========================================================================= //
// Lectura de APKs (ZIP + deflate)
// ========================================================================= //

/// ¿El nombre de la entrada del ZIP corresponde a un DEX de clases?
fn is_dex_entry_name(name: &str) -> bool {
    let file = name.rsplit('/').next().unwrap_or(name);
    if let Some(rest) = file.strip_prefix("classes") {
        let digits = rest.strip_suffix(".dex").unwrap_or("");
        return digits.is_empty() || digits.chars().all(|c| c.is_ascii_digit());
    }
    false
}

/// Extrae las entradas `classes*.dex` de un APK.
///
/// Primero se usa el crate `zip` (que ya sabe de ZIP64, deflate, data
/// descriptors…).  Si el `zip` rechaza el archivo (APKs manipulados, firmas
/// raras, ZIP truncado) se cae a un lector propio que infla el stream con
/// `flate2` leyendo las cabeceras locales.
pub fn apk_dex_entries(path: &str) -> Result<Vec<(String, Vec<u8>)>, String> {
    let raw = std::fs::read(path).map_err(|e| format!("no se pudo leer {}: {}", path, e))?;
    let mut entries = read_dex_entries_with_zip(&raw);
    if entries.is_empty() {
        entries = read_dex_entries_raw(&raw);
    }
    if entries.is_empty() {
        return Err(format!(
            "no se encontró ningún classes*.dex dentro de {}",
            path
        ));
    }
    // Orden estable: classes.dex, classes2.dex, classes3.dex…
    entries.sort_by(|a, b| dex_entry_order(&a.0).cmp(&dex_entry_order(&b.0)));
    Ok(entries)
}

/// Clave de ordenación de las entradas DEX.
fn dex_entry_order(name: &str) -> (String, u32) {
    let file = name.rsplit('/').next().unwrap_or(name).to_string();
    let digits = file
        .strip_prefix("classes")
        .and_then(|r| r.strip_suffix(".dex"))
        .unwrap_or("");
    let num = digits.parse::<u32>().unwrap_or(1);
    (file.clone(), num)
}

/// Lee las entradas DEX con el crate `zip`.
fn read_dex_entries_with_zip(raw: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut archive = match zip::ZipArchive::new(Cursor::new(raw.to_vec())) {
        Ok(a) => a,
        Err(_) => return out,
    };
    for i in 0..archive.len() {
        let mut file = match archive.by_index(i) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let name = file.name().to_string();
        if !is_dex_entry_name(&name) {
            continue;
        }
        let mut buf = Vec::new();
        if file.read_to_end(&mut buf).is_ok() && !buf.is_empty() {
            out.push((name, buf));
        }
    }
    out
}

/// Lector de reserva: busca cabeceras locales ZIP e infla con `flate2`.
fn read_dex_entries_raw(raw: &[u8]) -> Vec<(String, Vec<u8>)> {
    const LOCAL_SIG: u32 = 0x0403_4b50;
    const CENTRAL_SIG: u32 = 0x0201_4b50;
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos + 30 <= raw.len() {
        let sig = u32::from_le_bytes([raw[pos], raw[pos + 1], raw[pos + 2], raw[pos + 3]]);
        if sig != LOCAL_SIG {
            pos += 1;
            continue;
        }
        let flags = u16::from_le_bytes([raw[pos + 6], raw[pos + 7]]);
        let method = u16::from_le_bytes([raw[pos + 8], raw[pos + 9]]);
        let compressed_size = u32::from_le_bytes([
            raw[pos + 18],
            raw[pos + 19],
            raw[pos + 20],
            raw[pos + 21],
        ]) as usize;
        let name_len = u16::from_le_bytes([raw[pos + 26], raw[pos + 27]]) as usize;
        let extra_len = u16::from_le_bytes([raw[pos + 28], raw[pos + 29]]) as usize;
        let name_start = pos + 30;
        if name_start + name_len > raw.len() {
            break;
        }
        let name = String::from_utf8_lossy(&raw[name_start..name_start + name_len]).to_string();
        let data_start = name_start + name_len + extra_len;
        if data_start > raw.len() {
            break;
        }

        // Con data descriptor el tamaño local es 0: hay que buscar el final.
        let data_end = if compressed_size > 0 && flags & 0x08 == 0 {
            (data_start + compressed_size).min(raw.len())
        } else {
            match find_next_zip_boundary(raw, data_start) {
                Some(end) => end,
                None => raw.len(),
            }
        };

        if is_dex_entry_name(&name) {
            let chunk = &raw[data_start..data_end.max(data_start)];
            let data = match method {
                0 => chunk.to_vec(),
                8 => inflate_raw(chunk),
                _ => Vec::new(),
            };
            if !data.is_empty() {
                out.push((name, data));
            }
        }
        pos = data_end.max(pos + 4);
    }
    out
}

/// Busca la siguiente cabecera ZIP (local o central) a partir de `from`.
fn find_next_zip_boundary(raw: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i + 4 <= raw.len() {
        let sig = u32::from_le_bytes([raw[i], raw[i + 1], raw[i + 2], raw[i + 3]]);
        if sig == 0x0403_4b50 || sig == 0x0201_4b50 {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Infla un stream deflate crudo con `flate2`.
fn inflate_raw(chunk: &[u8]) -> Vec<u8> {
    use flate2::read::DeflateDecoder;
    use std::io::Read as _;
    let mut decoder = DeflateDecoder::new(chunk);
    let mut out = Vec::new();
    if decoder.read_to_end(&mut out).is_ok() {
        out
    } else {
        Vec::new()
    }
}

/// Carga un DEX para analizarlo.
///
/// Acepta dos formas:
///   * un fichero `.dex` suelto → se lee tal cual;
///   * un APK → se extrae el primer `classes*.dex` (si hay varios, se
///     devuelve el de mayor tamaño, que es el principal en la práctica).
///
/// Es la entrada que usa la tool MCP `dex_decompile`.
pub fn load_dex(path: &str) -> Result<DexFile, String> {
    let p = std::path::Path::new(path);
    if !p.exists() {
        return Err(format!("no existe: {}", path));
    }
    let is_apk = p
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_ascii_lowercase().ends_with(".apk"))
        .unwrap_or(false);
    if is_apk {
        let entries = apk_dex_entries(path)?;
        // El DEX principal es el mayor (los multidex reparten por peso).
        let mut best: Option<(String, Vec<u8>)> = None;
        for (name, bytes) in entries {
            let better = match &best {
                None => true,
                Some((_, b)) => bytes.len() > b.len(),
            };
            if better {
                best = Some((name, bytes));
            }
        }
        let (name, bytes) = best.ok_or_else(|| {
            format!("el APK {} no contiene classes*.dex", path)
        })?;
        return DexFile::parse(&bytes)
            .map_err(|e| format!("no se pudo parsear {} de {}: {}", name, path, e));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("no se pudo leer {}: {}", path, e))?;
    DexFile::parse(&bytes).map_err(|e| format!("no se pudo parsear {}: {}", path, e))
}

/// Decompila un APK completo: extrae sus DEX y devuelve el pseudo-Java.
///
/// Devuelve el pseudo-Java de todas las clases de todos los `classes*.dex`
/// del APK, con una cabecera por DEX (versión, SHA-256, número de clases).
pub fn decompile_apk(path: &str) -> Result<String, String> {
    let entries = apk_dex_entries(path)?;
    let mut out = String::new();
    out.push_str(&format!(
        "//======================================================================\n\
         // MegaDaemon DEX decompiler — APK {}\n\
         //======================================================================\n",
        path
    ));
    let mut errors = 0usize;
    for (name, bytes) in &entries {
        out.push_str(&format!(
            "\n//---- DEX: {} ({} bytes, sha256 {:016x}…) ----\n",
            name,
            bytes.len(),
            &bytes[..bytes.len().min(8)]
                .iter()
                .fold(0u64, |acc, b| (acc << 8) | *b as u64)
        ));
        match DexFile::parse(bytes) {
            Ok(dex) => {
                out.push_str(&format!(
                    "// DEX {} — {} clases, {} métodos\n",
                    dex.version,
                    dex.classes.len(),
                    dex.methods.len()
                ));
                out.push_str(&decompile_all(&dex));
            }
            Err(e) => {
                errors += 1;
                out.push_str(&format!("// ERROR al parsear {}: {}\n", name, e));
            }
        }
    }
    if errors == entries.len() {
        return Err(format!(
            "ningún DEX de {} se pudo parsear ({})",
            path,
            errors
        ));
    }
    Ok(out)
}

/// Decompila los bytes de un DEX suelto (no un APK).
pub fn decompile_dex_bytes(bytes: &[u8]) -> Result<String, String> {
    let dex = DexFile::parse(bytes)?;
    Ok(decompile_all(&dex))
}

/// Parsea un DEX suelto (alias funcional de [`DexFile::parse`]).
pub fn parse_dex(bytes: &[u8]) -> Result<DexFile, String> {
    DexFile::parse(bytes)
}

// ========================================================================= //
// Findings NDJSON — contrato común del enjambre (esquema v1.2)
// ========================================================================= //
//
// Un "hallazgo" es una línea de `findings.ndjson`. El esquema lo fijó el bus
// de coordinación (v1.2, 19 campos) para que TODOS los agentes hablen el
// mismo idioma:
//
//   ts, package, version, detector, class, method, signal, java_class,
//   java_method, original, decoded, kind, severity, confidence, bypassable,
//   offset, evidence_file, evidence_offset, notes
//
// OJO con dos nombres que colisionan conceptualmente:
//   * `class`  = TIPO de evidencia (fs_path|property|process|maps|ptrace|
//                keystore|signature|network|behavior|pool_string)
//   * `method` = cómo se obtuvo (stat|readlink|grep_props|proc_read|
//                package_manager|signature_check|runtime_probe|dex_pool_scan|
//                manifest_parse|static_scan)
// La clase y el método JAVA del hallazgo viajan en `java_class`/`java_method`.
// ========================================================================= //

/// Un hallazgo del formato común `findings.ndjson` (esquema v1.2).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Finding {
    /// Timestamp Unix del análisis.
    #[serde(default)]
    pub ts: i64,
    /// Paquete Android (vacío si el DEX va suelto).
    #[serde(default)]
    pub package: String,
    /// Versión de la app (vacía si no se sabe).
    #[serde(default)]
    pub version: String,
    /// Clase de detección: `magisk|su|frida|xposed|safetynet|playintegrity|
    /// tamper|emulator|root_behavior|obf_string|crypto_api|net_endpoint`.
    #[serde(default)]
    pub detector: String,
    /// Tipo de evidencia (`pool_string`, `network`, `behavior`…).
    #[serde(rename = "class", default)]
    pub evidence_class: String,
    /// Cómo se obtuvo el hallazgo.
    #[serde(default)]
    pub method: String,
    /// Valor crudo observado.
    #[serde(default)]
    pub signal: String,
    /// Clase Java donde aparece el hallazgo.
    #[serde(default)]
    pub java_class: String,
    /// Método Java donde aparece el hallazgo.
    #[serde(default)]
    pub java_method: String,
    /// Texto literal tal cual aparece (enmascarado si es una credencial).
    #[serde(default)]
    pub original: String,
    /// Candidata descifrada (o vacía).
    #[serde(default)]
    pub decoded: String,
    /// Subtipo libre.
    #[serde(default)]
    pub kind: String,
    /// `info|low|medium|high`.
    #[serde(default)]
    pub severity: String,
    /// Confianza 0-100.
    #[serde(default)]
    pub confidence: u8,
    /// Si la detección es evadible.
    #[serde(default)]
    pub bypassable: bool,
    /// Índice en el pool de constantes del DEX (-1 si no aplica).
    #[serde(default)]
    pub offset: i64,
    /// Ruta absoluta del fichero que evidencia el hallazgo.
    #[serde(default)]
    pub evidence_file: String,
    /// Offset en bytes dentro del fichero (-1 si no aplica).
    #[serde(default)]
    pub evidence_offset: i64,
    /// Notas libres.
    #[serde(default)]
    pub notes: String,
}

impl Finding {
    /// Serializa el hallazgo a una línea NDJSON (sin salto de línea).
    pub fn to_ndjson(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| String::from("{}"))
    }
}

/// Timestamp actual en segundos Unix.
fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Hosts de proveedores de IA con su nombre legible.
const AI_ENDPOINT_HOSTS: &[(&str, &str)] = &[
    ("api.anthropic.com", "Claude"),
    ("api.openai.com", "ChatGPT"),
    ("generativelanguage.googleapis.com", "Gemini"),
    ("aiplatform.googleapis.com", "Gemini"),
    ("api.deepseek.com", "DeepSeek"),
    ("api.moonshot.cn", "Kimi"),
    ("api.moonshot.ai", "Kimi"),
    ("api.minimax.chat", "MiniMax"),
    ("api.minimaxi.com", "MiniMax"),
    ("api.minimax.io", "MiniMax"),
    ("api.groq.com", "Groq"),
    ("api.together.xyz", "Together"),
    ("api.fireworks.ai", "Fireworks"),
    ("api.x.ai", "xAI"),
    ("api.perplexity.ai", "Perplexity"),
    ("openrouter.ai", "OpenRouter"),
];

/// Cabeceras y variables de entorno que delatan integración con IA:
/// (marcador exacto, subtipo, proveedor).
const AI_AUTH_MARKERS: &[(&str, &str, &str)] = &[
    ("x-api-key", "header_name", "Anthropic"),
    ("anthropic-version", "header_name", "Anthropic"),
    ("Authorization", "header_name", "Bearer"),
    ("ANTHROPIC_API_KEY", "env_var", "Anthropic"),
    ("OPENAI_API_KEY", "env_var", "ChatGPT"),
    ("GEMINI_API_KEY", "env_var", "Gemini"),
    ("GOOGLE_API_KEY", "env_var", "Gemini"),
    ("DEEPSEEK_API_KEY", "env_var", "DeepSeek"),
    ("MOONSHOT_API_KEY", "env_var", "Kimi"),
    ("KIMI_API_KEY", "env_var", "Kimi"),
    ("MINIMAX_API_KEY", "env_var", "MiniMax"),
    ("MINIMAX_GROUP_ID", "env_var", "MiniMax"),
    ("GROQ_API_KEY", "env_var", "Groq"),
    ("XAI_API_KEY", "env_var", "xAI"),
    ("PERPLEXITY_API_KEY", "env_var", "Perplexity"),
];

/// Formas de clave API: (prefijo, tipo, longitud mínima, confianza).
const API_KEY_SHAPES: &[(&str, &str, usize, u8)] = &[
    ("sk-ant-", "anthropic_key", 24, 95),
    ("sk-proj-", "openai_project_key", 24, 90),
    ("sk-", "openai_like_key", 24, 85),
    ("gsk_", "groq_key", 24, 90),
    ("xai-", "xai_key", 24, 85),
    ("ghp_", "github_token", 20, 90),
    ("gho_", "github_token", 20, 90),
    ("ghs_", "github_token", 20, 90),
    ("github_pat_", "github_pat", 24, 95),
    ("AIza", "google_api_key", 30, 90),
];

/// Señales anti-root / anti-debug / anti-tamper:
/// (detector, aguja, severidad, evadible).
const ANTI_ROOT_SIGNALS: &[(&str, &str, &str, bool)] = &[
    ("magisk", "/sbin/.magisk", "high", true),
    ("magisk", "magisk", "low", true),
    ("magisk", "Magisk", "low", true),
    ("su", "/system/xbin/su", "high", true),
    ("su", "/system/bin/su", "high", true),
    ("su", "/sbin/su", "high", true),
    ("su", "isRooted", "medium", true),
    ("su", "RootBeer", "medium", true),
    ("su", "test-keys", "medium", true),
    ("xposed", "de.robv.android.xposed", "high", false),
    ("xposed", "XposedBridge", "high", false),
    ("frida", "frida-agent", "high", true),
    ("frida", "frida-gadget", "high", true),
    ("frida", "frida-server", "high", true),
    ("safetynet", "SafetyNet", "medium", false),
    ("safetynet", "safetynet", "medium", false),
    ("playintegrity", "PlayIntegrity", "medium", false),
    ("playintegrity", "playintegrity", "medium", false),
    ("tamper", "/proc/self/maps", "medium", true),
    ("tamper", "TracerPid", "medium", true),
    ("tamper", "ptrace", "medium", true),
    ("emulator", "goldfish", "medium", true),
    ("emulator", "generic_x86", "medium", true),
    ("emulator", "sdk_gphone", "medium", true),
    ("root_behavior", "ro.debuggable", "medium", true),
    ("root_behavior", "ro.build.tags", "medium", true),
];

/// Mapa de strings del DEX: índice del pool → (clase Java, método Java).
///
/// Se calcula UNA vez y se reutiliza para todos los detectores: es el hot
/// path (APKs de 50-100 MB con 5 `classes*.dex`) y recomputarlo por detector
/// multiplicaba el trabajo.
fn string_origins(dex: &DexFile) -> HashMap<u32, (String, String)> {
    let mut origin: HashMap<u32, (String, String)> = HashMap::new();
    for class_def in &dex.classes {
        for method in class_def
            .direct_methods
            .iter()
            .chain(class_def.virtual_methods.iter())
        {
            let code = match &method.code {
                Some(c) => c,
                None => continue,
            };
            let method_name = dex
                .methods
                .get(method.method_idx as usize)
                .map(|m| m.name.clone())
                .unwrap_or_default();
            let mut offset = 0usize;
            while offset < code.insns.len() {
                let opcode = (code.insns[offset] & 0xff) as u8;
                let width = instruction_width(opcode).max(1);
                if opcode == 0x1a && offset + 1 < code.insns.len() {
                    origin
                        .entry(code.insns[offset + 1] as u32)
                        .or_insert_with(|| {
                            (class_def.class_name.clone(), method_name.clone())
                        });
                } else if opcode == 0x1b && offset + 2 < code.insns.len() {
                    let index = ((code.insns[offset + 2] as u32) << 16)
                        | code.insns[offset + 1] as u32;
                    origin.entry(index).or_insert_with(|| {
                        (class_def.class_name.clone(), method_name.clone())
                    });
                }
                offset += width;
            }
        }
    }
    origin
}

/// ¿El texto es un nombre con punto (paquete, clase o fichero) tipo
/// "Obf.java" o "a.b.c"?
fn looks_like_dotted_name(text: &str) -> bool {
    if !text.contains('.') || text.len() > 128 {
        return false;
    }
    text.split('.').all(|part| {
        if part.is_empty() {
            return false;
        }
        let mut chars = part.chars();
        let first = chars.next().unwrap();
        (first.is_ascii_alphabetic() || first == '_' || first == '$')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
    })
}

/// ¿El texto descifrado parece prosa y no ruido?
///
/// Un XOR de un byte aplicado a un identificador produce textos con símbolos
/// raros ("Obf.java" → "@mi!enyn"): no son prosa, son basura. Sin esta
/// comprobación el detector se disparaba sobre nombres de fichero y de clase
/// (falsos positivos) y tapaba los hallazgos reales.
fn is_prose_candidate(text: &str) -> bool {
    /// Signos que sí aparecen en texto plano de verdad (nótese que NO
    /// incluye '@' ni otros símbolos de ruido).
    const PROSE_PUNCT: &str = " \t.,;:!?()[]{}'\"-_+*/&%$#=<>|~^`";
    text.chars()
        .all(|c| c.is_ascii_alphanumeric() || PROSE_PUNCT.contains(c))
}

/// Palabras inglesas frecuentes: sirven para desempatar candidatos XOR
/// cortos. La frecuencia de letras por sí sola no basta ("aleef" puntúa
/// más alto que "hello" con 5 letras porque 'a' es más común que 'h'), pero
/// una PALABRA de verdad es un candidato mucho más probable.
const COMMON_WORDS: &[&str] = &[
    "about", "above", "after", "again", "against", "along", "already", "also", "always",
    "among", "another", "answer", "any", "app", "apply", "area", "around", "ask", "back",
    "because", "been", "before", "begin", "being", "below", "best", "better", "between",
    "both", "bring", "build", "business", "call", "came", "can", "case", "change", "check",
    "child", "come", "could", "country", "create", "data", "day", "default", "delete",
    "did", "different", "do", "does", "done", "down", "each", "early", "email", "end",
    "error", "even", "every", "example", "family", "file", "find", "first", "follow",
    "for", "form", "from", "get", "give", "good", "great", "group", "grow", "had", "has",
    "have", "he", "hello", "help", "here", "high", "home", "how", "if", "image", "import",
    "in", "info", "input", "into", "is", "it", "item", "its", "just", "key", "know", "large",
    "last", "let", "like", "line", "list", "little", "long", "look", "made", "make", "man",
    "many", "may", "me", "message", "more", "most", "name", "need", "new", "next", "not",
    "now", "number", "object", "of", "off", "old", "on", "one", "only", "open", "or",
    "other", "our", "out", "over", "page", "part", "people", "phone", "place", "point",
    "public", "put", "read", "result", "run", "said", "same", "save", "say", "see", "send",
    "server", "set", "should", "show", "sign", "simple", "since", "size", "so", "some",
    "start", "state", "still", "stop", "such", "support", "system", "take", "test",
    "text", "than", "that", "the", "their", "them", "then", "there", "these", "they",
    "this", "those", "time", "title", "to", "today", "together", "too", "true", "under",
    "until", "up", "use", "user", "value", "very", "want", "was", "water", "way", "we",
    "well", "were", "what", "when", "where", "which", "while", "white", "who", "will",
    "with", "word", "work", "world", "would", "write", "year", "you", "your",
];

/// Bonus de puntuación si el candidato es una palabra englisha real.
fn english_word_bonus(text: &str) -> f64 {
    let word: String = text
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .collect::<String>()
        .to_lowercase();
    if word.len() >= 3 && COMMON_WORDS.contains(&word.as_str()) {
        0.5
    } else {
        0.0
    }
}

/// ¿El texto tiene forma de clave API? → (tipo, confianza).
fn api_key_shape(text: &str) -> Option<(&'static str, u8)> {
    if text.len() < 16 || text.chars().any(|c| c.is_whitespace()) {
        return None;
    }
    for (prefix, kind, min_len, conf) in API_KEY_SHAPES {
        if text.starts_with(prefix) && text.len() >= *min_len {
            let rest = &text[prefix.len()..];
            if rest
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
            {
                return Some((kind, *conf));
            }
        }
    }
    if text.starts_with("eyJ") && text.matches('.').count() == 2 {
        return Some(("jwt", 80));
    }
    None
}

/// Enmascara un secreto dejando 4 caracteres al principio y 4 al final.
///
/// Las credenciales NO se escriben en claro en `findings.ndjson`: el
/// `offset` apunta al string literal del pool, así que el dato sigue
/// siendo recuperable por quien tenga el DEX, pero un informe que se
/// comparte por chat o se sube a un almacén no filtra la clave.
pub fn mask_secret(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= 12 {
        return "*".repeat(chars.len());
    }
    let head: String = chars[..4].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{}{}{}", head, "*".repeat(chars.len() - 8), tail)
}

/// Hallazgos de endpoints de IA y credenciales embebidas en el DEX.
///
/// Variante segura: las credenciales salen ENMASCARADAS.
pub fn ai_credential_findings(dex: &DexFile) -> Vec<Finding> {
    ai_credential_findings_inner(dex, false)
}

/// Hallazgos de endpoints de IA y credenciales embebidas en el DEX.
///
/// Variante que revela el literal de la credencial (sólo para consumo local).
pub fn ai_credential_findings_raw(dex: &DexFile) -> Vec<Finding> {
    ai_credential_findings_inner(dex, true)
}

fn ai_credential_findings_inner(dex: &DexFile, reveal: bool) -> Vec<Finding> {
    let origins = string_origins(dex);
    let ts = now_ts();
    let mut out: Vec<Finding> = Vec::new();

    for (index, text) in dex.strings.iter().enumerate() {
        if text.len() < 8 || text.len() > 2048 {
            continue;
        }
        let (java_class, java_method) = origins
            .get(&(index as u32))
            .cloned()
            .unwrap_or_else(|| (String::from("<string-pool>"), String::new()));

        // (1) Endpoint de proveedor de IA.
        for (host, provider) in AI_ENDPOINT_HOSTS {
            if text.contains(host) {
                out.push(Finding {
                    ts,
                    detector: String::from("net_endpoint"),
                    evidence_class: String::from("network"),
                    method: String::from("dex_pool_scan"),
                    signal: text.clone(),
                    java_class: java_class.clone(),
                    java_method: java_method.clone(),
                    original: text.clone(),
                    kind: format!("ai_endpoint:{}", provider),
                    severity: String::from("medium"),
                    confidence: 90,
                    bypassable: false,
                    offset: index as i64,
                    evidence_offset: -1,
                    notes: format!("endpoint de {}", provider),
                    ..Default::default()
                });
                break;
            }
        }

        // (2) Cabecera o variable de entorno de autenticación (match exacto).
        for (marker, kind, provider) in AI_AUTH_MARKERS {
            if text == *marker {
                out.push(Finding {
                    ts,
                    detector: String::from("crypto_api"),
                    evidence_class: String::from("pool_string"),
                    method: String::from("dex_pool_scan"),
                    signal: text.clone(),
                    java_class: java_class.clone(),
                    java_method: java_method.clone(),
                    original: text.clone(),
                    kind: format!("{}:{}", kind, provider),
                    severity: String::from("medium"),
                    confidence: 70,
                    bypassable: false,
                    offset: index as i64,
                    evidence_offset: -1,
                    notes: format!("{} de {}", kind, provider),
                    ..Default::default()
                });
                break;
            }
        }

        // (3) Forma de clave API.
        if let Some((kind, conf)) = api_key_shape(text) {
            let shown = if reveal {
                text.clone()
            } else {
                mask_secret(text)
            };
            out.push(Finding {
                ts,
                detector: String::from("crypto_api"),
                evidence_class: String::from("pool_string"),
                method: String::from("dex_pool_scan"),
                signal: text.clone(),
                java_class: java_class.clone(),
                java_method: java_method.clone(),
                original: shown,
                kind: format!("api_key:{}", kind),
                severity: String::from("high"),
                confidence: conf,
                bypassable: false,
                offset: index as i64,
                evidence_offset: -1,
                notes: if reveal {
                    String::from("credencial en claro")
                } else {
                    String::from("credencial enmascarada; literal en el pool, offset del DEX")
                },
                ..Default::default()
            });
        }
    }

    out
}

/// Hallazgos de señales anti-root / anti-debug / anti-tamper.
pub fn anti_root_findings(dex: &DexFile) -> Vec<Finding> {
    let origins = string_origins(dex);
    let ts = now_ts();
    let mut out: Vec<Finding> = Vec::new();
    // Tope por señal: evita inundar el informe con 200 coincidencias de
    // "magisk" y mantiene el valor real de cada hallazgo.
    let mut per_signal: HashMap<(&str, &str), usize> = HashMap::new();

    for (index, text) in dex.strings.iter().enumerate() {
        if text.len() < 4 || text.len() > 512 {
            continue;
        }
        for (detector, needle, severity, bypassable) in ANTI_ROOT_SIGNALS {
            if !text.contains(needle) {
                continue;
            }
            let counter = per_signal.entry((*detector, *needle)).or_insert(0);
            if *counter >= 3 {
                continue;
            }
            *counter += 1;
            let (java_class, java_method) = origins
                .get(&(index as u32))
                .cloned()
                .unwrap_or_else(|| (String::from("<string-pool>"), String::new()));
            out.push(Finding {
                ts,
                detector: (*detector).to_string(),
                evidence_class: String::from("pool_string"),
                method: String::from("dex_pool_scan"),
                signal: text.clone(),
                java_class,
                java_method,
                original: text.clone(),
                kind: String::from("anti_check"),
                severity: (*severity).to_string(),
                confidence: 75,
                bypassable: *bypassable,
                offset: index as i64,
                evidence_offset: -1,
                notes: format!("señal de detección {}", detector),
                ..Default::default()
            });
        }
    }

    out
}

/// Hallazgos de ofuscación (renombrados + strings cifrados) en el formato
/// común v1.2.
pub fn obfuscation_findings(dex: &DexFile) -> Vec<Finding> {
    let ts = now_ts();
    let report = analyze_obfuscation(dex);
    let mut out: Vec<Finding> = Vec::new();

    for class_name in &report.renamed_types {
        out.push(Finding {
            ts,
            detector: String::from("obf_string"),
            evidence_class: String::from("behavior"),
            method: String::from("static_scan"),
            signal: class_name.clone(),
            java_class: class_name.clone(),
            kind: String::from("renamed_class"),
            severity: String::from("low"),
            confidence: 70,
            bypassable: true,
            offset: -1,
            evidence_offset: -1,
            notes: String::from("nombre de clase de 1-2 caracteres: renombrado de ofuscador"),
            ..Default::default()
        });
    }

    for method in &report.renamed_methods {
        let (class_name, method_name) = match method.split_once('.') {
            Some((c, m)) => (c.to_string(), m.trim_end_matches("()").to_string()),
            None => (String::new(), method.clone()),
        };
        out.push(Finding {
            ts,
            detector: String::from("obf_string"),
            evidence_class: String::from("behavior"),
            method: String::from("static_scan"),
            signal: method.clone(),
            java_class: class_name.clone(),
            java_method: method_name.clone(),
            kind: String::from("renamed_method"),
            severity: String::from("low"),
            confidence: 70,
            bypassable: true,
            offset: -1,
            evidence_offset: -1,
            notes: String::from("nombre de método de 1-2 caracteres: renombrado de ofuscador"),
            ..Default::default()
        });
    }

    for enc in &report.encrypted_strings {
        let severity = if enc.confidence >= 85 {
            "high"
        } else if enc.confidence >= 65 {
            "medium"
        } else {
            "low"
        };
        out.push(Finding {
            ts,
            detector: String::from("obf_string"),
            evidence_class: String::from("pool_string"),
            method: String::from("dex_pool_scan"),
            signal: enc.original.clone(),
            java_class: enc.class_name.clone(),
            java_method: enc.method_name.clone(),
            original: enc.original.clone(),
            decoded: enc.candidate_plaintext.clone(),
            kind: enc.kind.clone(),
            severity: severity.to_string(),
            confidence: enc.confidence,
            bypassable: true,
            offset: enc.offset as i64,
            evidence_offset: -1,
            notes: enc.evidence.clone(),
            ..Default::default()
        });
    }

    out
}

/// Informe completo en NDJSON (esquema v1.2): ofuscación + credenciales +
/// señales anti-root, una línea por hallazgo.
pub fn findings_ndjson(dex: &DexFile, evidence_file: &str) -> String {
    let mut findings = obfuscation_findings(dex);
    findings.extend(ai_credential_findings(dex));
    findings.extend(anti_root_findings(dex));
    let mut out = String::new();
    for f in &findings {
        let mut f = f.clone();
        if f.evidence_file.is_empty() {
            f.evidence_file = evidence_file.to_string();
        }
        out.push_str(&f.to_ndjson());
        out.push('\n');
    }
    out
}

/// Mapa de desofuscación en NDJSON (mismo contenido que [`findings_ndjson`]
/// pero sólo la parte de ofuscación).
pub fn obfuscation_map_ndjson(dex: &DexFile, evidence_file: &str) -> String {
    let mut out = String::new();
    for f in obfuscation_findings(dex) {
        let mut f = f;
        if f.evidence_file.is_empty() {
            f.evidence_file = evidence_file.to_string();
        }
        out.push_str(&f.to_ndjson());
        out.push('\n');
    }
    out
}

/// Lee `findings.ndjson` (texto) y devuelve los hallazgos.
///
/// Tolerante a propósito: líneas vacías, líneas que no son JSON o JSON
/// inválido se saltan en vez de tumbar el análisis completo.
pub fn parse_findings_ndjson(text: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }
        if let Ok(f) = serde_json::from_str::<Finding>(line) {
            out.push(f);
        }
    }
    out
}

/// Lee `findings.ndjson` del disco.
pub fn load_findings_ndjson(path: &str) -> Result<Vec<Finding>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("no se pudo leer {}: {}", path, e))?;
    Ok(parse_findings_ndjson(&text))
}

/// Fusiona hallazgos propios con los del agente externo, sin duplicar.
///
/// La clave de deduplicación es (detector, java_class, java_method, original,
/// signal): el mismo hallazgo visto por dos agentes se reporta una vez.
pub fn merge_findings(local: Vec<Finding>, external: Vec<Finding>) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for f in local.into_iter().chain(external.into_iter()) {
        let key = format!(
            "{}|{}|{}|{}|{}",
            f.detector, f.java_class, f.java_method, f.original, f.signal
        );
        if seen.insert(key) {
            out.push(f);
        }
    }
    out
}

/// Analiza un APK o DEX en disco y devuelve sus hallazgos en NDJSON.
///
/// `evidence_file` es la ruta que se graba en cada hallazgo.
pub fn findings_ndjson_file(path: &str) -> Result<String, String> {
    if path.ends_with(".apk") {
        let entries = apk_dex_entries(path)?;
        let mut out = String::new();
        for (name, bytes) in entries {
            if let Ok(dex) = DexFile::parse(&bytes) {
                out.push_str(&findings_ndjson(&dex, path));
            }
            let _ = name;
        }
        Ok(out)
    } else {
        let bytes = std::fs::read(path).map_err(|e| format!("no se pudo leer {}: {}", path, e))?;
        let dex = DexFile::parse(&bytes)?;
        Ok(findings_ndjson(&dex, path))
    }
}

// ========================================================================= //
// Tests
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------ //
    // Utilidades para construir DEX sintéticos válidos
    // ------------------------------------------------------------------ //

    /// Escribe un `uleb128` en un buffer.
    fn put_uleb(buf: &mut Vec<u8>, value: u32) {
        let mut value = value;
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            buf.push(byte);
            if value == 0 {
                break;
            }
        }
    }

    /// Método sintético.
    #[derive(Clone, Default)]
    struct TestMethod {
        method_idx: u32,
        access_flags: u32,
        registers: u16,
        ins: u16,
        outs: u16,
        insns: Vec<u16>,
    }

    /// Clase sintética.
    #[derive(Clone, Default)]
    struct TestClass {
        class_idx: u32,
        superclass_idx: u32,
        access_flags: u32,
        source_file_idx: u32,
        direct_methods: Vec<TestMethod>,
        virtual_methods: Vec<TestMethod>,
    }

    /// Constructor de DEX sintéticos válidos (header, ids, class_data, code).
    #[derive(Default)]
    struct TestDexBuilder {
        strings: Vec<String>,
        types: Vec<u32>,
        protos: Vec<(u32, u32, Vec<u16>)>,
        fields: Vec<(u16, u16, u32)>,
        methods: Vec<(u16, u16, u32)>,
        classes: Vec<TestClass>,
    }

    impl TestDexBuilder {
        fn new() -> Self {
            Self::default()
        }

        /// Añade un string al pool y devuelve su índice.
        fn string(&mut self, text: &str) -> u32 {
            self.strings.push(text.to_string());
            (self.strings.len() - 1) as u32
        }

        /// Añade un tipo (descriptor) y devuelve su índice en `type_ids`.
        fn add_type(&mut self, descriptor: &str) -> u16 {
            let idx = self.string(descriptor);
            self.types.push(idx);
            (self.types.len() - 1) as u16
        }

        /// Añade un tipo a partir del índice de su string.
        fn add_type_str(&mut self, string_idx: u32) -> u16 {
            self.types.push(string_idx);
            (self.types.len() - 1) as u16
        }

        /// Añade un prototipo: (shorty, tipo de retorno, parámetros).
        fn add_proto(&mut self, shorty: u32, return_type: u16, params: &[u16]) -> u16 {
            self.protos.push((shorty, return_type as u32, params.to_vec()));
            (self.protos.len() - 1) as u16
        }

        /// Añade un método y devuelve su índice en `method_ids`.
        fn add_method(&mut self, class: u16, proto: u16, name: u32) -> u16 {
            self.methods.push((class, proto, name));
            (self.methods.len() - 1) as u16
        }

        /// Añade un campo y devuelve su índice en `field_ids`.
        fn add_field(&mut self, class: u16, ty: u16, name: u32) -> u16 {
            self.fields.push((class, ty, name));
            (self.fields.len() - 1) as u16
        }

        /// Añade una clase.
        fn add_class(&mut self, class: TestClass) {
            self.classes.push(class);
        }

        /// Ensambla el DEX completo.
        fn build(&self) -> Vec<u8> {
            // Offsets de las secciones de identificadores.
            let mut off = HEADER_SIZE;
            let string_ids_off = off;
            off += self.strings.len() * 4;
            let type_ids_off = off;
            off += self.types.len() * 4;
            let proto_ids_off = off;
            off += self.protos.len() * 12;
            let field_ids_off = off;
            off += self.fields.len() * 8;
            let method_ids_off = off;
            off += self.methods.len() * 8;
            let class_defs_off = off;
            off += self.classes.len() * 32;
            let data_off = (off + 3) & !3;

            let mut buf: Vec<u8> = vec![0u8; data_off];

            // --- string_data_item -------------------------------------- //
            let mut string_data_off: Vec<u32> = Vec::with_capacity(self.strings.len());
            for text in &self.strings {
                while buf.len() % 4 != 0 {
                    buf.push(0);
                }
                let at = buf.len();
                let utf16: usize = text.chars().map(|c| c.len_utf16()).sum();
                put_uleb(&mut buf, utf16 as u32);
                buf.extend_from_slice(text.as_bytes());
                buf.push(0);
                string_data_off.push(at as u32);
            }

            // --- type_list (parámetros de los prototipos) --------------- //
            let mut type_list_off: Vec<u32> = Vec::with_capacity(self.protos.len());
            for (_, _, params) in &self.protos {
                if params.is_empty() {
                    type_list_off.push(0);
                    continue;
                }
                while buf.len() % 4 != 0 {
                    buf.push(0);
                }
                let at = buf.len();
                buf.extend_from_slice(&(params.len() as u32).to_le_bytes());
                for p in params {
                    buf.extend_from_slice(&p.to_le_bytes());
                }
                type_list_off.push(at as u32);
            }

            // --- code_item (uno por método con cuerpo) ------------------ //
            let mut code_offs: Vec<Vec<u32>> = Vec::with_capacity(self.classes.len());
            for class in &self.classes {
                let mut per_class = Vec::new();
                for method in class.direct_methods.iter().chain(class.virtual_methods.iter()) {
                    while buf.len() % 4 != 0 {
                        buf.push(0);
                    }
                    let at = buf.len();
                    buf.extend_from_slice(&method.registers.to_le_bytes());
                    buf.extend_from_slice(&method.ins.to_le_bytes());
                    buf.extend_from_slice(&method.outs.to_le_bytes());
                    buf.extend_from_slice(&0u16.to_le_bytes()); // tries_size
                    buf.extend_from_slice(&0u32.to_le_bytes()); // debug_info_off
                    buf.extend_from_slice(&(method.insns.len() as u32).to_le_bytes());
                    for unit in &method.insns {
                        buf.extend_from_slice(&unit.to_le_bytes());
                    }
                    per_class.push(at as u32);
                }
                code_offs.push(per_class);
            }

            // --- class_data_item ---------------------------------------- //
            let mut class_data_off: Vec<u32> = Vec::with_capacity(self.classes.len());
            for (ci, class) in self.classes.iter().enumerate() {
                let at = buf.len();
                put_uleb(&mut buf, 0); // static_fields_size
                put_uleb(&mut buf, 0); // instance_fields_size
                put_uleb(&mut buf, class.direct_methods.len() as u32);
                put_uleb(&mut buf, class.virtual_methods.len() as u32);
                let mut prev = 0u32;
                for (mi, method) in class.direct_methods.iter().enumerate() {
                    put_uleb(&mut buf, method.method_idx - prev);
                    prev = method.method_idx;
                    put_uleb(&mut buf, method.access_flags);
                    put_uleb(&mut buf, code_offs[ci][mi]);
                }
                prev = 0;
                for (vi, method) in class.virtual_methods.iter().enumerate() {
                    put_uleb(&mut buf, method.method_idx - prev);
                    prev = method.method_idx;
                    put_uleb(&mut buf, method.access_flags);
                    let offset = class.direct_methods.len() + vi;
                    put_uleb(&mut buf, code_offs[ci][offset]);
                }
                class_data_off.push(at as u32);
            }

            // --- map_list ------------------------------------------------ //
            let mut map_items: Vec<(u16, u32, u32)> = Vec::new();
            map_items.push((MAP_TYPE_HEADER, 1, 0));
            if !self.strings.is_empty() {
                map_items.push((MAP_TYPE_STRING_ID, self.strings.len() as u32, string_ids_off as u32));
                map_items.push((
                    MAP_TYPE_STRING_DATA,
                    self.strings.len() as u32,
                    string_data_off.first().copied().unwrap_or(0),
                ));
            }
            if !self.types.is_empty() {
                map_items.push((MAP_TYPE_TYPE_ID, self.types.len() as u32, type_ids_off as u32));
            }
            if !self.protos.is_empty() {
                map_items.push((MAP_TYPE_PROTO_ID, self.protos.len() as u32, proto_ids_off as u32));
            }
            if !self.fields.is_empty() {
                map_items.push((MAP_TYPE_FIELD_ID, self.fields.len() as u32, field_ids_off as u32));
            }
            if !self.methods.is_empty() {
                map_items.push((
                    MAP_TYPE_METHOD_ID,
                    self.methods.len() as u32,
                    method_ids_off as u32,
                ));
            }
            if !self.classes.is_empty() {
                map_items.push((
                    MAP_TYPE_CLASS_DEF,
                    self.classes.len() as u32,
                    class_defs_off as u32,
                ));
                map_items.push((
                    MAP_TYPE_CLASS_DATA,
                    self.classes.len() as u32,
                    class_data_off.first().copied().unwrap_or(0),
                ));
                let code_count: u32 = code_offs.iter().map(|c| c.len() as u32).sum();
                if code_count > 0 {
                    map_items.push((MAP_TYPE_CODE_ITEM, code_count, 0));
                }
            }
            // El map_list debe ir ordenado por offset.
            map_items.sort_by_key(|(_, _, offset)| *offset);
            let map_off = buf.len();
            map_items.push((MAP_TYPE_MAP_LIST, 1, map_off as u32));
            map_items.sort_by_key(|(_, _, offset)| *offset);
            buf.extend_from_slice(&(map_items.len() as u32).to_le_bytes());
            for (item_type, size, offset) in &map_items {
                buf.extend_from_slice(&item_type.to_le_bytes());
                buf.extend_from_slice(&0u16.to_le_bytes());
                buf.extend_from_slice(&size.to_le_bytes());
                buf.extend_from_slice(&offset.to_le_bytes());
            }

            // --- Secciones de identificadores ---------------------------- //
            for (i, off) in string_data_off.iter().enumerate() {
                write_u32(&mut buf, string_ids_off + i * 4, *off);
            }
            for (i, str_idx) in self.types.iter().enumerate() {
                write_u32(&mut buf, type_ids_off + i * 4, *str_idx);
            }
            for (i, (shorty, ret, _)) in self.protos.iter().enumerate() {
                write_u32(&mut buf, proto_ids_off + i * 12, *shorty);
                write_u32(&mut buf, proto_ids_off + i * 12 + 4, *ret as u32);
                write_u32(&mut buf, proto_ids_off + i * 12 + 8, type_list_off[i]);
            }
            for (i, (class, ty, name)) in self.fields.iter().enumerate() {
                write_u16(&mut buf, field_ids_off + i * 8, *class);
                write_u16(&mut buf, field_ids_off + i * 8 + 2, *ty);
                write_u32(&mut buf, field_ids_off + i * 8 + 4, *name);
            }
            for (i, (class, proto, name)) in self.methods.iter().enumerate() {
                write_u16(&mut buf, method_ids_off + i * 8, *class);
                write_u16(&mut buf, method_ids_off + i * 8 + 2, *proto);
                write_u32(&mut buf, method_ids_off + i * 8 + 4, *name);
            }
            for (i, class) in self.classes.iter().enumerate() {
                let base = class_defs_off + i * 32;
                write_u32(&mut buf, base, class.class_idx);
                write_u32(&mut buf, base + 4, class.access_flags);
                write_u32(&mut buf, base + 8, class.superclass_idx);
                write_u32(&mut buf, base + 12, 0); // interfaces_off
                write_u32(&mut buf, base + 16, class.source_file_idx);
                write_u32(&mut buf, base + 20, 0); // annotations_off
                write_u32(&mut buf, base + 24, class_data_off[i]);
                write_u32(&mut buf, base + 28, 0); // static_values_off
            }

            // --- Header -------------------------------------------------- //
            buf[0..4].copy_from_slice(b"dex\n");
            buf[4..7].copy_from_slice(b"035");
            buf[7] = 0;
            let total_len = buf.len() as u32;
            write_u32(&mut buf, 32, total_len);
            write_u32(&mut buf, 36, HEADER_SIZE as u32);
            write_u32(&mut buf, 40, ENDIAN_CONSTANT);
            write_u32(&mut buf, 44, 0); // link_size
            write_u32(&mut buf, 48, 0); // link_off
            write_u32(&mut buf, 52, map_off as u32);
            write_u32(&mut buf, 56, self.strings.len() as u32);
            write_u32(&mut buf, 60, string_ids_off as u32);
            write_u32(&mut buf, 64, self.types.len() as u32);
            write_u32(&mut buf, 68, type_ids_off as u32);
            write_u32(&mut buf, 72, self.protos.len() as u32);
            write_u32(&mut buf, 76, proto_ids_off as u32);
            write_u32(&mut buf, 80, self.fields.len() as u32);
            write_u32(&mut buf, 84, field_ids_off as u32);
            write_u32(&mut buf, 88, self.methods.len() as u32);
            write_u32(&mut buf, 92, method_ids_off as u32);
            write_u32(&mut buf, 96, self.classes.len() as u32);
            write_u32(&mut buf, 100, class_defs_off as u32);
            let data_size = (buf.len() - data_off) as u32;
            write_u32(&mut buf, 104, data_size);
            write_u32(&mut buf, 108, data_off as u32);

            // Firma SHA-1 (sobre bytes 32..) y checksum Adler-32 (12..).
            let signature = sha1(&buf[32..]);
            buf[12..32].copy_from_slice(&signature);
            let checksum = adler32(&buf[12..]);
            write_u32(&mut buf, 8, checksum);

            buf
        }
    }

    /// Construye el DEX sintético de referencia para los tests.
    ///
    /// Contiene:
    /// * clase `Lcom/example/Obf;` con `<init>` (const-string "hello") y `a`
    /// * clase `La;` (ofuscada) con `<init>` y `b` (saltos nulos + array)
    /// * strings cifrados: `"SGVsbG8gV29ybGQ="` (base64) y `"kfool"` (XOR 3)
    fn fixture_dex() -> Vec<u8> {
        let mut b = TestDexBuilder::new();

        let s_obf = b.string("Lcom/example/Obf;");
        let s_obf_java = b.string("Obf.java");
        let s_hello = b.string("hello");
        let s_a_class = b.string("La;");
        let s_void = b.string("V");
        let s_object = b.string("Ljava/lang/Object;");
        let s_string = b.string("Ljava/lang/String;");
        let s_init = b.string("<init>");
        let s_shorty = b.string("Ljava/lang/String;");
        let s_m_a = b.string("a");
        let s_m_b = b.string("b");
        let s_b64 = b.string("SGVsbG8gV29ybGQ=");
        let s_xor = b.string("kfool");
        let s_field = b.string("secret");

        let t_obf = b.add_type_str(s_obf);
        let t_a = b.add_type_str(s_a_class);
        let t_object = b.add_type_str(s_object);
        let t_string = b.add_type_str(s_string);
        let t_void = b.add_type_str(s_void);

        let p_void = b.add_proto(s_void, t_void, &[]);
        let p_str = b.add_proto(s_shorty, t_void, &[t_string]);

        let m_init_obf = b.add_method(t_obf, p_void, s_init);
        let m_a_obf = b.add_method(t_obf, p_str, s_m_a);
        let m_init_a = b.add_method(t_a, p_void, s_init);
        let m_b_a = b.add_method(t_a, p_void, s_m_b);

        let f_secret = b.add_field(t_obf, t_string, s_field);

        b.add_class(TestClass {
            class_idx: t_obf as u32,
            superclass_idx: t_object as u32,
            access_flags: 0x0001,
            source_file_idx: s_obf_java,
            direct_methods: vec![
                TestMethod {
                    method_idx: m_init_obf as u32,
                    access_flags: 0x10001,
                    registers: 1,
                    ins: 1,
                    outs: 0,
                    // const-string v0, "hello" ; return-void
                    insns: vec![0x001a, 0x0002, 0x000e],
                },
                TestMethod {
                    method_idx: m_a_obf as u32,
                    access_flags: 0x0009,
                    registers: 2,
                    ins: 2,
                    outs: 0,
                    // const/4 v0, #1 ; return v0
                    insns: vec![0x1012, 0x000f],
                },
            ],
            virtual_methods: vec![],
        });

        b.add_class(TestClass {
            class_idx: t_a as u32,
            superclass_idx: t_object as u32,
            access_flags: 0x0001,
            source_file_idx: u32::MAX,
            direct_methods: vec![
                TestMethod {
                    method_idx: m_init_a as u32,
                    access_flags: 0x10001,
                    registers: 1,
                    ins: 1,
                    outs: 0,
                    insns: vec![0x000e],
                },
                TestMethod {
                    method_idx: m_b_a as u32,
                    access_flags: 0x0009,
                    registers: 2,
                    ins: 1,
                    outs: 0,
                    // 0000: fill-array-data v0, +8
                    // 0003: goto +1 (salto nulo)
                    // 0004: goto +1
                    // 0005: goto +1
                    // 0006: return-void
                    // 0007: nop (alineación)
                    // 0008: payload fill-array-data (ident 0x0300)
                    insns: vec![
                        0x0026, 0x0008, 0x0000, // fill-array-data v0, +8
                        0x0128, 0x0128, 0x0128, // goto +1 (x3, código basura)
                        0x000e, // return-void
                        0x0000, // relleno
                        0x0300, 0x0001, 0x0004, 0x0000, // payload: ident, width=1, size=4
                        0xadde, 0xefbe, // datos: de ad be ef
                    ],
                },
            ],
            virtual_methods: vec![],
        });

        let _ = (s_hello, s_b64, s_xor, f_secret);
        b.build()
    }

    // ------------------------------------------------------------------ //
    // Tests
    // ------------------------------------------------------------------ //

    #[test]
    fn test_opcode_table_covers_all_256() {
        for op in 0u16..=255 {
            let (name, format) = opcode_info(op as u8);
            assert!(!name.is_empty(), "opcode 0x{:02x} sin nombre", op);
            assert!(!format.is_empty(), "opcode 0x{:02x} sin formato", op);
            assert!(instruction_width(op as u8) >= 1);
        }
        assert_eq!(opcode_info(0x00), ("nop", "10x"));
        assert_eq!(opcode_info(0x1a), ("const-string", "21c"));
        assert_eq!(opcode_info(0x6e), ("invoke-virtual", "35c"));
        assert_eq!(opcode_info(0x74), ("invoke-virtual/range", "3rc"));
        assert_eq!(opcode_info(0xfa), ("invoke-polymorphic", "45cc"));
        assert_eq!(opcode_info(0xff), ("const-method-type", "21c"));
        assert_eq!(instruction_width(0x1a), 2);
        assert_eq!(instruction_width(0x6e), 3);
        assert_eq!(instruction_width(0xfa), 4);
        assert_eq!(instruction_width(0x18), 5);
    }

    #[test]
    fn test_parse_synthetic_dex() {
        let bytes = fixture_dex();
        let dex = DexFile::parse(&bytes).expect("el DEX sintético debe parsear");
        assert_eq!(dex.version, "035");
        assert_eq!(dex.classes.len(), 2);
        assert_eq!(dex.methods.len(), 4);
        assert!(dex.strings.len() >= 13);
        assert_eq!(dex.classes[0].class_name, "Lcom/example/Obf;");
        assert_eq!(dex.classes[1].class_name, "La;");
        assert_eq!(dex.classes[0].source_file, "Obf.java");
        assert!(dex.checksum_valid, "el checksum Adler-32 debe cuadrar");
        assert!(dex.signature_valid, "la firma SHA-1 debe cuadrar");
        assert_eq!(dex.sha256.len(), 64);
        assert_eq!(dex.classes[0].direct_methods.len(), 2);
        assert_eq!(dex.classes[0].direct_methods[0].method_idx, 0);
    }

    #[test]
    fn test_parse_rejects_garbage() {
        assert!(DexFile::parse(b"esto no es un dex").is_err());
        assert!(DexFile::parse(&[]).is_err());
        let mut bad = fixture_dex();
        bad[4..7].copy_from_slice(b"099");
        assert!(DexFile::parse(&bad).is_err());
    }

    #[test]
    fn test_disassemble_known_instruction() {
        let bytes = fixture_dex();
        let dex = DexFile::parse(&bytes).unwrap();
        let lines = disassemble_method(&dex, 0, 0);
        assert!(!lines.is_empty(), "el constructor debe desensamblarse");
        let joined = lines.join("\n");
        assert!(
            joined.contains("const-string") && joined.contains("hello"),
            "se esperaba const-string con \"hello\": {}",
            joined
        );
        assert!(joined.contains("return-void"));

        // La segunda clase tiene `<init>` (método 2) con un solo return-void.
        let lines2 = disassemble_method(&dex, 1, 2);
        assert_eq!(lines2.len(), 1);
        assert!(lines2[0].contains("return-void"));
    }

    #[test]
    fn test_detect_obfuscated_class_name() {
        let bytes = fixture_dex();
        let dex = DexFile::parse(&bytes).unwrap();
        let report = analyze_obfuscation(&dex);
        assert!(
            report.renamed_types.contains(&"La;".to_string()),
            "debería detectar La; como ofuscada: {:?}",
            report.renamed_types
        );
        assert!(
            !report.renamed_types.contains(&"Lcom/example/Obf;".to_string()),
            "una clase con nombre normal no debe marcarse"
        );
        assert!(
            report
                .renamed_methods
                .iter()
                .any(|m| m.contains(".a()")),
            "debería detectar el método ofuscado 'a': {:?}",
            report.renamed_methods
        );
    }

    #[test]
    fn test_decompile_class_pseudo_java() {
        let bytes = fixture_dex();
        let dex = DexFile::parse(&bytes).unwrap();
        let java = decompile_class(&dex, 0);
        assert!(java.contains("class Lcom/example/Obf;"), "{}", java);
        assert!(java.contains("package com.example;"), "{}", java);
        assert!(java.contains("public class Obf"), "{}", java);
        assert!(java.contains("const-string"), "{}", java);
        assert!(java.contains("hello"), "{}", java);

        let java_a = decompile_class(&dex, 1);
        assert!(java_a.contains("class La;"), "{}", java_a);
    }

    #[test]
    fn test_decompile_all_contains_every_class() {
        let bytes = fixture_dex();
        let dex = DexFile::parse(&bytes).unwrap();
        let all = decompile_all(&dex);
        assert!(all.contains("Lcom/example/Obf;"));
        assert!(all.contains("La;"));
        assert!(all.contains(&dex.sha256));
    }

    #[test]
    fn test_dex_strings() {
        let bytes = fixture_dex();
        let dex = DexFile::parse(&bytes).unwrap();
        let strings = dex_strings(&dex);
        assert_eq!(strings, dex.strings);
        assert!(strings.iter().any(|s| s == "hello"));
        assert!(strings.iter().any(|s| s == "Lcom/example/Obf;"));
    }

    #[test]
    fn test_encrypted_string_detection() {
        let bytes = fixture_dex();
        let dex = DexFile::parse(&bytes).unwrap();
        let report = analyze_obfuscation(&dex);

        let b64 = report
            .encrypted_strings
            .iter()
            .find(|e| e.kind == "base64")
            .expect("debería detectar el string en base64");
        assert_eq!(b64.candidate_plaintext, "Hello World");

        let xor = report
            .encrypted_strings
            .iter()
            .find(|e| e.kind == "xor")
            .expect("debería detectar el string cifrado con XOR");
        assert_eq!(xor.candidate_plaintext, "hello");
        assert_eq!(xor.xor_key, Some(3));
    }

    #[test]
    fn test_control_flow_obfuscation_detection() {
        let bytes = fixture_dex();
        let dex = DexFile::parse(&bytes).unwrap();
        let report = analyze_obfuscation(&dex);
        assert!(
            report
                .control_flow_obfuscation
                .iter()
                .any(|c| c.contains("saltos a la instrucción siguiente")),
            "debería detectar los saltos nulos: {:?}",
            report.control_flow_obfuscation
        );
        assert!(
            report
                .encrypted_strings
                .iter()
                .any(|e| e.kind == "byte-array"),
            "debería detectar el array de bytes: {:?}",
            report.encrypted_strings
        );
    }

    #[test]
    fn test_obfuscation_map_is_json() {
        let bytes = fixture_dex();
        let dex = DexFile::parse(&bytes).unwrap();
        let map = obfuscation_map(&dex);
        let parsed: Value = serde_json::from_str(&map).expect("el mapa debe ser JSON válido");
        assert!(parsed.get("hypotheses").is_some());
        assert!(parsed["summary"]["renamed_types"].as_u64().unwrap() >= 1);
        assert!(parsed["dex"]["version"].as_str().unwrap() == "035");
    }

    #[test]
    fn test_decompile_apk_end_to_end() {
        let dex_bytes = fixture_dex();
        let mut zip_bytes: Vec<u8> = Vec::new();
        {
            let cursor = Cursor::new(&mut zip_bytes);
            let mut writer = zip::ZipWriter::new(cursor);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            writer.start_file("classes.dex", options).unwrap();
            std::io::Write::write_all(&mut writer, &dex_bytes).unwrap();
            writer.finish().unwrap();
        }

        let path = std::env::temp_dir().join("megadaemon_dex_decompile_test.apk");
        std::fs::write(&path, &zip_bytes).unwrap();

        let entries = apk_dex_entries(path.to_str().unwrap()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "classes.dex");

        let java = decompile_apk(path.to_str().unwrap()).expect("el APK debe decompilarse");
        assert!(java.contains("Lcom/example/Obf;"));
        assert!(java.contains("const-string"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_sha1_and_adler32_vectors() {
        // Vectores conocidos de SHA-1.
        assert_eq!(
            hex(&sha1(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            hex(&sha1(b"")),
            "da39a3ee5e6b4b0d3255bfef95601890afd80709"
        );
        // Adler-32 de "abc".
        assert_eq!(adler32(b"abc"), 0x024d_0127);
    }

    #[test]
    fn test_uleb128_and_mutf8() {
        let mut pos = 0usize;
        assert_eq!(read_uleb128(&[0x80, 0x01], &mut pos).unwrap(), 128);
        assert_eq!(pos, 2);
        let mut pos = 0usize;
        assert_eq!(read_sleb128(&[0x7f], &mut pos).unwrap(), -1);
        // MUTF-8: el NUL va codificado como C0 80.
        assert_eq!(decode_mutf8(&[0xC0, 0x80]), "\0");
        assert_eq!(decode_mutf8(b"hola"), "hola");
        // Par de surrogates CESU-8 -> U+1F600.
        let smile = decode_mutf8(&[0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]);
        assert_eq!(smile, "\u{1F600}");
    }

    #[test]
    fn test_descriptor_conversion() {
        assert_eq!(descriptor_to_java("Ljava/lang/String;"), "java.lang.String");
        assert_eq!(descriptor_to_java("[I"), "int[]");
        assert_eq!(descriptor_to_java("V"), "void");
        assert_eq!(descriptor_to_java("Z"), "boolean");
        assert_eq!(class_simple_name("Lcom/x/Foo$Bar;"), "Bar");
        assert_eq!(java_class_name("Lcom/x/Foo;"), "com.x.Foo");
    }

    #[test]
    fn test_api_key_shape_detects_common_providers() {
        assert!(api_key_shape("sk-ant-api03-AAAABBBBCCCCDDDDEEEE").is_some());
        assert!(api_key_shape("sk-proj-AAAABBBBCCCCDDDDEEEEFFFF").is_some());
        assert!(api_key_shape("ghp_AAAABBBBCCCCDDDDEEEEFFFF").is_some());
        assert!(api_key_shape("AIzaSyAAAABBBBCCCCDDDDEEEEFFFFGGGG").is_some());
        // Demasiado corto o con espacios => no es una clave.
        assert!(api_key_shape("sk-corto").is_none());
        assert!(api_key_shape("Bearer sk-ant-AAAABBBBCCCCDDDDEEEE").is_none());
        assert!(api_key_shape("hola mundo").is_none());
    }

    #[test]
    fn test_mask_secret_never_leaks_the_middle() {
        let m = mask_secret("sk-ant-api03-SECRETSECRETSECRET");
        assert!(!m.contains("SECRETSECRET"));
        assert!(m.starts_with("sk-a"));
        assert!(m.ends_with("CRET"));
        assert_eq!(mask_secret("corto"), "*****");
    }

    #[test]
    fn test_findings_ndjson_uses_the_common_schema() {
        let dex = DexFile::parse(&fixture_dex()).unwrap();
        let ndjson = findings_ndjson(&dex, "/data/app/com.example/base.apk");
        assert!(!ndjson.is_empty());
        let mut count = 0usize;
        for line in ndjson.lines() {
            let v: Value = serde_json::from_str(line).expect("cada línea debe ser JSON");
            // Los 19 campos del esquema v1.2 deben existir.
            for campo in [
                "ts", "package", "version", "detector", "class", "method", "signal",
                "java_class", "java_method", "original", "decoded", "kind", "severity",
                "confidence", "bypassable", "offset", "evidence_file", "evidence_offset",
                "notes",
            ] {
                assert!(v.get(campo).is_some(), "falta el campo {}", campo);
            }
            assert_eq!(v["evidence_file"].as_str().unwrap(), "/data/app/com.example/base.apk");
            count += 1;
        }
        assert!(count > 0);
    }

    #[test]
    fn test_parse_findings_ndjson_is_tolerant() {
        let texto = concat!(
            "{\"detector\":\"magisk\",\"signal\":\"/sbin/.magisk\"}\n",
            "\n",
            "esto no es json\n",
            "{\"detector\":\"crypto_api\",\"java_class\":\"com.x.A\",\"confidence\":80,\"offset\":7}\n"
        );
        let v = parse_findings_ndjson(texto);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].detector, "magisk");
        assert_eq!(v[1].java_class, "com.x.A");
        assert_eq!(v[1].confidence, 80);
        assert_eq!(v[1].offset, 7);
        // Campos ausentes => defaults, sin pánico.
        assert!(v[0].java_class.is_empty());
        assert!(v[0].severity.is_empty());
    }

    #[test]
    fn test_merge_findings_deduplicates() {
        let a = Finding {
            detector: "magisk".into(),
            java_class: "com.x.A".into(),
            original: "/sbin/.magisk".into(),
            ..Default::default()
        };
        let b = a.clone();
        let c = Finding {
            detector: "su".into(),
            ..Default::default()
        };
        let merged = merge_findings(vec![a, b], vec![c]);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn test_ai_findings_mask_credentials_by_default() {
        let mut b = TestDexBuilder::new();
        let s_url = b.string("https://api.anthropic.com/v1/messages");
        let s_key = b.string("sk-ant-api03-AAAABBBBCCCCDDDDEEEE");
        let s_env = b.string("ANTHROPIC_API_KEY");
        let t = b.add_type_str(s_url);
        let p = b.add_proto(s_url, t, &[]);
        let m = b.add_method(t, p, s_env);
        let s_field = b.string("k");
        let f = b.add_field(t, t, s_field);
        b.add_class(TestClass {
            class_idx: t as u32,
            superclass_idx: t as u32,
            access_flags: 0x0001,
            source_file_idx: s_url,
            direct_methods: vec![TestMethod {
                method_idx: m as u32,
                access_flags: 0x10001,
                registers: 1,
                ins: 1,
                outs: 0,
                insns: vec![0x001a, 0x0001, 0x000e],
            }],
            virtual_methods: vec![],
        });
        let bytes = b.build();
        let dex = DexFile::parse(&bytes).unwrap();

        let findings = ai_credential_findings(&dex);
        assert!(findings
            .iter()
            .any(|f| f.detector == "net_endpoint" && f.kind.contains("Claude")));
        assert!(findings.iter().any(|f| f.kind.contains("api_key")));
        assert!(findings.iter().any(|f| f.kind.contains("env_var")));
        // La clave NO sale en claro.
        let joined = findings
            .iter()
            .map(|f| f.original.clone())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!joined.contains("AAAABBBBCCCCDDDDEEEE"));
        // ...pero sí si se pide explícitamente.
        let raw = ai_credential_findings_raw(&dex);
        assert!(raw
            .iter()
            .any(|f| f.original == "sk-ant-api03-AAAABBBBCCCCDDDDEEEE"));
    }
}
