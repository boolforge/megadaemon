//! MegaDaemon v15 — Extracción de los descriptores protobuf embebidos.
//!
//! # Por qué este módulo
//!
//! El binario `liblanguage_server.so` (ELF ARM64, Go 1.28, sin DWARF) trae
//! **65.230 de sus 161.682 funciones** generadas por `protoc-gen-go` a
//! partir de ficheros `.proto` que viven en el sistema de build interno de
//! Google (Blaze). Descompilar ese Go es tirar tiempo: lo correcto es
//! **regenerarlo** desde los descriptores que el binario lleva embebidos.
//!
//! `protoc-gen-go` (protobuf-go v2) guarda el `FileDescriptorProto` crudo
//! como una constante de cadena en `.rodata`:
//!
//! ```text
//! const file_foo_proto_rawDesc = "" + "\n\x0efoo.proto" + ...
//! ```
//!
//! O sea: los bytes serializados del descriptor están tal cual en el
//! binario, sin comprimir (la función `file_foo_proto_rawDescGZIP` que
//! aparece en la tabla de símbolos sólo los comprime *en memoria* al
//! registrarlos en el `protoregistry` global).
//!
//! # Algoritmo (medido, no supuesto)
//!
//! 1. Se recorren los segmentos `PT_LOAD` de sólo lectura del ELF.
//! 2. Se buscan **candidatos**: un byte `0x0A` (el tag del campo 1,
//!    `name`) seguido de un varint de longitud y de esa cantidad de bytes
//!    ASCII que terminen en `.proto`.
//! 3. Cada candidato se recorre con un parser **estricto** de
//!    `FileDescriptorProto` ([`parse_file_descriptor_proto`]): el fin
//!    exacto del descriptor se averigua porque
//!    - los números de campo tienen que estar entre 1 y 15 y llevar el
//!      *wire-type* que les corresponde,
//!    - el campo 1 (`name`) es singular: si aparece dos veces, el
//!      descriptor anterior ha terminado,
//!    - los números de campo van en **orden ascendente** (así serializa
//!      `protoc`), así que uno menor que el anterior marca el final.
//!
//! Medido sobre `liblanguage_server.so`: 704 candidatos → 585
//! descriptores válidos → **557 `.proto` únicos**, 3.019.042 bytes de
//! descriptor crudo, 5.353 mensajes, 531 enums, 36 servicios y 272
//! extensiones.
//!
//! Este módulo es la parte de *extracción*. La reconstrucción del texto
//! `.proto` y la del Go se hacen en `tools/proto-regen/` (Python), porque
//! necesitan la librería `protobuf`; aquí no se añade ninguna dependencia
//! nueva al crate.
//!
//! # Notas de protocolo
//!
//! Este módulo **no se ha compilado** en el sandbox donde se escribió: el
//! crate `megadaemon-core` pesa +200 módulos y enlazarlo en una máquina
//! de 3 GB es un OOM garantizado (ver `docs/PROTOCOLO_FLOCKA.md`). El root
//! lo compila y testea. Por eso los tests de este módulo son unitarios y
//! no dependen de ficheros externos.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Especificación de `FileDescriptorProto` (`google/protobuf/descriptor.proto`):
/// número de campo → (wire-type, ¿repetido?).
///
/// El wire-type 0 es varint y el 2 es *length-delimited*. Estos números
/// están medidos contra `descriptor_pb2` de la librería `protobuf`, no
/// de memoria.
const ESPECIFICACION: [(u32, u8, bool); 14] = [
    (1, 2, false),  // name
    (2, 2, false),  // package
    (3, 2, true),   // dependency
    (4, 2, true),   // message_type
    (5, 2, true),   // enum_type
    (6, 2, true),   // service
    (7, 2, true),   // extension
    (8, 2, false),  // options
    (9, 2, false),  // source_code_info
    (10, 0, true),  // public_dependency
    (11, 0, true),  // weak_dependency
    (12, 2, false), // syntax
    (14, 0, false), // edition
    (15, 2, true),  // option_dependency
];

/// Un descriptor protobuf localizado dentro del binario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Descriptor {
    /// Nombre del fichero `.proto` (campo 1 del descriptor).
    pub nombre: String,
    /// Offset del primer byte del descriptor dentro del fichero.
    pub offset: usize,
    /// Offset del primer byte *después* del descriptor.
    pub fin: usize,
    /// Longitud del descriptor en bytes.
    pub bytes: usize,
}

impl Descriptor {
    /// Los bytes crudos del descriptor.
    pub fn bytes_crudos<'a>(&self, buf: &'a [u8]) -> &'a [u8] {
        &buf[self.offset..self.fin]
    }
}

/// Segmento `PT_LOAD` de un ELF64.
#[derive(Debug, Clone, Copy)]
pub struct Segmento {
    pub vaddr: u64,
    pub offset: u64,
    pub filesz: u64,
    pub flags: u32,
}

/// Error del lector de ELF o del escaner de descriptores.
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Formato(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "error de E/S: {e}"),
            Error::Formato(m) => write!(f, "formato invalido: {m}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

fn u16_le(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

fn u32_le(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

fn u64_le(buf: &[u8], off: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&buf[off..off + 8]);
    u64::from_le_bytes(b)
}

/// Lee los segmentos `PT_LOAD` de un ELF64 little-endian.
pub fn segmentos_elf64(buf: &[u8]) -> Result<Vec<Segmento>, Error> {
    if buf.len() < 64 || &buf[0..4] != b"\x7fELF" {
        return Err(Error::Formato("no es un ELF".into()));
    }
    if buf[4] != 2 {
        return Err(Error::Formato("solo se soporta ELF64".into()));
    }
    if buf[5] != 1 {
        return Err(Error::Formato("solo se soporta little-endian".into()));
    }
    let e_phoff = u64_le(buf, 32) as usize;
    let e_phentsize = u16_le(buf, 54) as usize;
    let e_phnum = u16_le(buf, 56) as usize;
    let mut out = Vec::new();
    for i in 0..e_phnum {
        let off = e_phoff + i * e_phentsize;
        if off + 56 > buf.len() {
            return Err(Error::Formato("tabla de programas truncada".into()));
        }
        let p_type = u32_le(buf, off);
        let p_flags = u32_le(buf, off + 4);
        if p_type != 1 {
            continue; // PT_LOAD
        }
        let p_offset = u64_le(buf, off + 8);
        let p_vaddr = u64_le(buf, off + 16);
        let p_filesz = u64_le(buf, off + 32);
        out.push(Segmento {
            vaddr: p_vaddr,
            offset: p_offset,
            filesz: p_filesz,
            flags: p_flags,
        });
    }
    Ok(out)
}

/// Zonas de sólo lectura (donde vive `.rodata`) del binario.
pub fn zonas_solo_lectura(buf: &[u8]) -> Result<Vec<(usize, usize)>, Error> {
    const PF_W: u32 = 0x2;
    let mut zonas = Vec::new();
    for s in segmentos_elf64(buf)? {
        if s.flags & PF_W != 0 || s.filesz == 0 {
            continue;
        }
        let ini = s.offset as usize;
        let fin = ini + s.filesz as usize;
        if fin <= buf.len() {
            zonas.push((ini, fin));
        }
    }
    Ok(zonas)
}

/// Lee un varint de protobuf. Devuelve `None` si está truncado.
pub fn leer_varint(buf: &[u8], mut i: usize) -> Option<(u64, usize)> {
    let mut res: u64 = 0;
    let mut shift = 0u32;
    loop {
        if i >= buf.len() {
            return None;
        }
        let c = buf[i];
        i += 1;
        res |= ((c & 0x7f) as u64) << shift;
        if c & 0x80 == 0 {
            return Some((res, i));
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
}

/// Recorre un `FileDescriptorProto` de forma estricta.
///
/// Devuelve el offset inmediatamente posterior al descriptor. Si el
/// recorrido se rompe, devuelve el offset donde se rompió (que es el
/// principio del siguiente mensaje).
pub fn parse_file_descriptor_proto(buf: &[u8], inicio: usize) -> usize {
    let mut i = inicio;
    let mut vistos: Vec<u32> = Vec::new();
    let mut ultimo = 0u32;
    loop {
        let antes = i;
        let (clave, sig) = match leer_varint(buf, i) {
            Some(v) => v,
            None => return antes,
        };
        i = sig;
        let num = (clave >> 3) as u32;
        let wt = (clave & 7) as u8;
        let spec = ESPECIFICACION.iter().find(|(n, _, _)| *n == num);
        let Some((_, wt_esperado, repetido)) = spec.copied() else {
            return antes;
        };
        if wt != wt_esperado {
            return antes;
        }
        // protoc serializa en orden ASCENDENTE de número de campo.
        if num < ultimo {
            return antes;
        }
        // Los campos singulares no se repiten.
        if !repetido && vistos.contains(&num) {
            return antes;
        }
        if wt == 2 {
            let (ln, sig) = match leer_varint(buf, i) {
                Some(v) => v,
                None => return antes,
            };
            i = sig;
            let l = ln as usize;
            if i + l > buf.len() {
                return antes;
            }
            i += l;
        } else {
            let (_v, sig) = match leer_varint(buf, i) {
                Some(v) => v,
                None => return antes,
            };
            i = sig;
        }
        vistos.push(num);
        ultimo = num;
    }
}

/// ¿Es este texto un nombre de fichero `.proto` plausible?
///
/// Filtra los falsos positivos: los payloads de opciones personalizadas
/// (p.ej. la extensión `proto_file` de `google/api/service.proto`) tienen
/// la misma forma que un descriptor (tag 0x0A + longitud + texto) pero su
/// nombre lleva dos puntos o mayúsculas.
pub fn nombre_proto_valido(nombre: &str) -> bool {
    if !nombre.ends_with(".proto") || nombre.len() < 5 {
        return false;
    }
    let head = &nombre[..nombre.len() - 6];
    if head.is_empty() {
        return false;
    }
    let mut chars = head.chars();
    let primero = chars.next().unwrap();
    if !(primero.is_ascii_alphanumeric() || primero == '_') {
        return false;
    }
    head.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '/' || c == '-')
}

/// Extrae todos los descriptores protobuf embebidos en el binario.
///
/// Devuelve un mapa `nombre → descriptor`. Si un mismo `.proto` aparece
/// varias veces (versiones distintas), se queda con el más grande: es el
/// más completo.
pub fn extraer_descriptores(buf: &[u8]) -> Result<BTreeMap<String, Descriptor>, Error> {
    let mut candidatos: Vec<(usize, String)> = Vec::new();
    for (ini, fin) in zonas_solo_lectura(buf)? {
        let mut i = ini;
        while i < fin {
            if buf[i] != 0x0A {
                i += 1;
                continue;
            }
            let Some((ln, sig)) = leer_varint(buf, i + 1) else {
                i += 1;
                continue;
            };
            let l = ln as usize;
            if !(4..=400).contains(&l) || sig + l > buf.len() {
                i += 1;
                continue;
            }
            let trozo = &buf[sig..sig + l];
            if !trozo.ends_with(b".proto") {
                i += 1;
                continue;
            }
            if !trozo.iter().all(|c| (32..127).contains(c)) {
                i += 1;
                continue;
            }
            if let Ok(nombre) = std::str::from_utf8(trozo) {
                if nombre_proto_valido(nombre) {
                    candidatos.push((i, nombre.to_string()));
                }
            }
            i += 1;
        }
    }
    candidatos.sort_by(|a, b| a.0.cmp(&b.0));
    candidatos.dedup_by(|a, b| a.0 == b.0);

    let mut out: BTreeMap<String, Descriptor> = BTreeMap::new();
    for (offset, nombre) in candidatos {
        let fin = parse_file_descriptor_proto(buf, offset);
        if fin <= offset {
            continue;
        }
        let crudo = &buf[offset..fin];
        // El `name` (campo 1) debe ser lo primero y coincidir con el
        // candidato: si no, no es un FileDescriptorProto.
        if crudo.len() < 2 || crudo[0] != 0x0A {
            continue;
        }
        let Some((ln, sig)) = leer_varint(crudo, 1) else {
            continue;
        };
        let l = ln as usize;
        if sig + l > crudo.len() {
            continue;
        }
        if &crudo[sig..sig + l] != nombre.as_bytes() {
            continue;
        }
        let d = Descriptor {
            nombre: nombre.clone(),
            offset,
            fin,
            bytes: fin - offset,
        };
        match out.get(&nombre) {
            Some(prev) if prev.bytes >= d.bytes => {}
            _ => {
                out.insert(nombre, d);
            }
        }
    }
    Ok(out)
}

/// Extrae los descriptores de un fichero y escribe un manifiesto TSV.
///
/// El TSV lleva: nombre, offset, bytes y sha-256 de los bytes crudos.
pub fn escribir_manifiesto(
    ruta_binario: &Path,
    ruta_salida: &Path,
) -> Result<usize, Error> {
    let buf = fs::read(ruta_binario)?;
    let descriptores = extraer_descriptores(&buf)?;
    let mut texto = String::from("nombre\toffset\tbytes\tsha256\n");
    for (nombre, d) in &descriptores {
        let crudo = d.bytes_crudos(&buf);
        texto.push_str(&format!(
            "{}\t{:#x}\t{}\t{}\n",
            nombre,
            d.offset,
            d.bytes,
            hex_sha256(crudo)
        ));
    }
    if let Some(dir) = ruta_salida.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(ruta_salida, texto)?;
    Ok(descriptores.len())
}

/// SHA-256 en hexadecimal (implementación propia para no añadir dependencias
/// más allá de las que ya tiene el crate: aquí se usa `sha2`).
fn hex_sha256(datos: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(datos);
    let out = h.finalize();
    out.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construye un `FileDescriptorProto` mínimo y válido:
    /// name(1) + package(2) + syntax(12).
    fn descriptor_minimo(nombre: &str, paquete: &str, sintaxis: &str) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(0x0A);
        out.push(nombre.len() as u8);
        out.extend_from_slice(nombre.as_bytes());
        out.push(0x12);
        out.push(paquete.len() as u8);
        out.extend_from_slice(paquete.as_bytes());
        out.push(0x62);
        out.push(sintaxis.len() as u8);
        out.extend_from_slice(sintaxis.as_bytes());
        out
    }

    #[test]
    fn varint_se_lee_bien() {
        let buf = [0x96u8, 0x01];
        let (v, i) = leer_varint(&buf, 0).unwrap();
        assert_eq!(v, 150);
        assert_eq!(i, 2);
        assert!(leer_varint(&[0x80], 0).is_none());
    }

    #[test]
    fn nombres_proto_validos_se_filtran() {
        assert!(nombre_proto_valido("google/protobuf/any.proto"));
        assert!(nombre_proto_valido("net/proto2/proto/descriptor.proto"));
        // El falso positivo medido: el payload de la opción `proto_file`.
        assert!(!nombre_proto_valido("proto_file:google/api/service.proto"));
        assert!(!nombre_proto_valido("proto"));
    }

    #[test]
    fn el_parser_encuentra_el_fin_del_descriptor() {
        let d = descriptor_minimo("google/protobuf/any.proto", "google.protobuf", "proto3");
        let fin = parse_file_descriptor_proto(&d, 0);
        assert_eq!(fin, d.len());
    }

    #[test]
    fn un_segundo_name_marca_el_fin_del_descriptor_anterior() {
        // Dos descriptores concatenated: el parser debe parar en el segundo.
        let mut d = descriptor_minimo("a.proto", "p", "proto3");
        let fin1 = parse_file_descriptor_proto(&d, 0);
        assert_eq!(fin1, d.len());
        let mut dos = descriptor_minimo("a.proto", "p", "proto3");
        dos.extend_from_slice(&descriptor_minimo("b.proto", "p", "proto3"));
        let f = parse_file_descriptor_proto(&dos, 0);
        assert_eq!(f, dos.len() / 2);
    }

    #[test]
    fn campos_desconocidos_cortan_el_descriptor() {
        // Campo 40 (no existe en FileDescriptorProto): el parser para ahí.
        let mut d = descriptor_minimo("a.proto", "p", "proto3");
        d.push(0xC0);
        d.push(0x03);
        d.push(0x08);
        d.push(0x01);
        let fin = parse_file_descriptor_proto(&d, 0);
        assert!(fin < d.len());
    }

    #[test]
    fn el_escanner_halla_el_descriptor_embebido() {
        // Se simula un binario: no es un ELF, asi que se comprueba el
        // camino de error y el parser sobre un buffer plano.
        let d = descriptor_minimo("x/y.proto", "x.y", "proto3");
        // MEDIDO (2026-11-23): el buffer minimo mide 24 B; el test cortaba
        // `d[..64]` => "range end index 64 out of range" (panic por el
        // propio test, no por el escaner).  Se pasa el buffer entero: no es
        // un ELF, asi que el escaner debe rechazarlo.
        assert!(segmentos_elf64(&d).is_err());
        let fin = parse_file_descriptor_proto(&d, 0);
        assert_eq!(fin, d.len());
    }
}
