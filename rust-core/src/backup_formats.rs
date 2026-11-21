//! Lectores de **formatos de backup externos**.
//!
//! Objetivo del bloque F2: que MegaDaemon pueda **leer** backups hechos por
//! otras herramientas (interop), no copiar su código. Licencias de cada
//! proyecto en `docs/BACKUP_FORMATS_CATALOG.md`:
//!
//! | Formato | Proyecto | Licencia | Qué hacemos aquí |
//! |---|---|---|---|
//! | `tar` (ustar/GNU/pax) | Neo Backup (AGPL-3), OAndBackupX (GPL), tarb | AGPL/GPL ⇒ **sólo formato** | Lector + extracción segura |
//! | `zip` (stored/deflate) | Titanium/Swift Backup (contenedores), ADB legacy | Propietario ⇒ **sólo interop** | Lector + extracción segura |
//! | `metadata.json` | Swift Backup (propietario) | Propietario ⇒ **sólo interop** | Catálogo de ficheros por app |
//!
//! **Seguridad**: la extracción rechaza rutas absolutas, `..` y enlaces
//! simbólicos (un backup ajeno no puede escribir fuera del destino).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// TAR (ustar / GNU longname / pax)
// ---------------------------------------------------------------------------

/// Una entrada de un archivo tar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TarEntry {
    /// Ruta dentro del archivo.
    pub name: String,
    /// Tamaño en bytes (0 para directorios).
    pub size: u64,
    /// Byte de tipo (`'0'` fichero, `'5'` directorio, `'2'` symlink…).
    pub typeflag: u8,
    /// Permisos.
    pub mode: u32,
    /// Fecha de modificación (epoch).
    pub mtime: u64,
    /// Offset del contenido dentro del tar.
    pub data_offset: usize,
}

impl TarEntry {
    /// `true` si es un directorio.
    pub fn is_dir(&self) -> bool {
        self.typeflag == b'5' || self.name.ends_with('/')
    }

    /// `true` si es un enlace simbólico (no se extrae).
    pub fn is_symlink(&self) -> bool {
        self.typeflag == b'2'
    }
}

fn octal(buf: &[u8]) -> u64 {
    let mut v: u64 = 0;
    for b in buf {
        match b {
            b'0'..=b'7' => v = v * 8 + (b - b'0') as u64,
            b' ' | 0 => break,
            _ => break,
        }
    }
    v
}

fn cstr(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).trim_end().to_string()
}

/// Lee las entradas de un tar (soporta nombres largos GNU y cabeceras pax).
pub fn read_tar(bytes: &[u8]) -> Result<Vec<TarEntry>, String> {
    let mut out = Vec::new();
    let mut off = 0usize;
    let mut long_name: Option<String> = None;
    let mut pax_name: Option<String> = None;
    while off + 512 <= bytes.len() {
        let hdr = &bytes[off..off + 512];
        if hdr.iter().all(|&b| b == 0) {
            break; // dos bloques nulos = fin
        }
        let size = octal(&hdr[124..136]);
        let typeflag = hdr[156];
        let mode = octal(&hdr[100..108]) as u32;
        let mtime = octal(&hdr[136..148]);
        let prefix = cstr(&hdr[345..500]);
        let base = cstr(&hdr[0..100]);
        let data_off = off + 512;
        let data_len = size as usize;
        let next = data_off + data_len.div_ceil(512) * 512;

        match typeflag {
            b'L' => {
                // GNU long name: el contenido es el nombre largo
                long_name = Some(cstr(&bytes[data_off..(data_off + data_len).min(bytes.len())]));
                off = next;
                continue;
            }
            b'x' | b'g' => {
                // pax extended header: "longpath=..." o "path=..."
                let content = &bytes[data_off..(data_off + data_len).min(bytes.len())];
                pax_name = pax_path(content);
                off = next;
                continue;
            }
            _ => {}
        }
        let name = if let Some(p) = pax_name.take() {
            p
        } else if let Some(l) = long_name.take() {
            l
        } else if !prefix.is_empty() {
            format!("{prefix}/{base}")
        } else {
            base
        };
        out.push(TarEntry {
            name,
            size,
            typeflag,
            mode,
            mtime,
            data_offset: data_off,
        });
        off = next;
    }
    Ok(out)
}

fn pax_path(content: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(content);
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("path=") {
            return Some(v.trim_matches('"').to_string());
        }
        if let Some(v) = line.strip_prefix("GNU.sparse.name=") {
            return Some(v.trim_matches('"').to_string());
        }
    }
    None
}

/// Extrae un tar a un directorio, **de forma segura**.
///
/// Devuelve la lista de rutas escritas. Rechaza rutas absolutas, `..` y
/// enlaces simbólicos.
pub fn tar_extract(bytes: &[u8], dest: &Path) -> Result<Vec<String>, String> {
    let mut written = Vec::new();
    for e in read_tar(bytes)? {
        if e.is_symlink() {
            continue; // no se extraen enlaces
        }
        let rel = safe_relative_path(&e.name)?;
        let target = dest.join(&rel);
        if e.is_dir() {
            std::fs::create_dir_all(&target).map_err(|err| format!("mkdir {}: {err}", target.display()))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|err| format!("mkdir {}: {err}", parent.display()))?;
        }
        let data = bytes
            .get(e.data_offset..e.data_offset + e.size as usize)
            .ok_or_else(|| format!("datos truncados en {}", e.name))?;
        std::fs::write(&target, data).map_err(|err| format!("write {}: {err}", target.display()))?;
        written.push(rel.to_string_lossy().to_string());
    }
    Ok(written)
}

// ---------------------------------------------------------------------------
// ZIP (stored + deflate)
// ---------------------------------------------------------------------------

/// Una entrada de un archivo zip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZipEntry {
    /// Ruta dentro del zip.
    pub name: String,
    /// Tamaño descomprimido.
    pub size: u64,
    /// Tamaño comprimido.
    pub compressed_size: u64,
    /// Método (0 = stored, 8 = deflate).
    pub method: u16,
    /// CRC-32 announced.
    pub crc32: u32,
    /// Offset de la cabecera local.
    pub local_offset: u64,
}

impl ZipEntry {
    /// `true` si el nombre acaba en `/` (directorio).
    pub fn is_dir(&self) -> bool {
        self.name.ends_with('/')
    }
}

fn u16le(b: &[u8]) -> u16 {
    u16::from_le_bytes([b[0], b[1]])
}
fn u32le(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// Lee el directorio central de un zip.
pub fn read_zip(bytes: &[u8]) -> Result<Vec<ZipEntry>, String> {
    // localizar el End Of Central Directory (EOCD)
    let mut eocd = None;
    let start = bytes.len().saturating_sub(66_000);
    let mut i = bytes.len().saturating_sub(22);
    while i >= start {
        if &bytes[i..i + 4] == b"PK\x05\x06" {
            eocd = Some(i);
            break;
        }
        if i == 0 {
            break;
        }
        i -= 1;
    }
    let eocd = eocd.ok_or("no se encuentra el EOCD del zip")?;
    let count = u16le(&bytes[eocd + 10..eocd + 12]) as usize;
    let cd_off = u32le(&bytes[eocd + 16..eocd + 20]) as usize;
    let mut out = Vec::with_capacity(count);
    let mut p = cd_off;
    for _ in 0..count {
        if p + 46 > bytes.len() || &bytes[p..p + 4] != b"PK\x01\x02" {
            return Err("directorio central corrupto".to_string());
        }
        let method = u16le(&bytes[p + 10..p + 12]);
        let crc = u32le(&bytes[p + 16..p + 20]);
        let csize = u32le(&bytes[p + 20..p + 24]) as u64;
        let usize_ = u32le(&bytes[p + 24..p + 28]) as u64;
        let name_len = u16le(&bytes[p + 28..p + 30]) as usize;
        let extra_len = u16le(&bytes[p + 30..p + 32]) as usize;
        let comment_len = u16le(&bytes[p + 32..p + 34]) as usize;
        let local_off = u32le(&bytes[p + 42..p + 46]) as u64;
        let name = String::from_utf8_lossy(&bytes[p + 46..p + 46 + name_len]).to_string();
        out.push(ZipEntry {
            name,
            size: usize_,
            compressed_size: csize,
            method,
            crc32: crc,
            local_offset: local_off,
        });
        p += 46 + name_len + extra_len + comment_len;
    }
    Ok(out)
}

/// Extrae el contenido de una entrada zip (stored o deflate).
pub fn zip_read_data(bytes: &[u8], e: &ZipEntry) -> Result<Vec<u8>, String> {
    let lo = e.local_offset as usize;
    if lo + 30 > bytes.len() || &bytes[lo..lo + 4] != b"PK\x03\x04" {
        return Err(format!("cabecera local inválida en {}", e.name));
    }
    let name_len = u16le(&bytes[lo + 26..lo + 28]) as usize;
    let extra_len = u16le(&bytes[lo + 28..lo + 30]) as usize;
    let start = lo + 30 + name_len + extra_len;
    let end = start + e.compressed_size as usize;
    let raw = bytes
        .get(start..end)
        .ok_or_else(|| format!("datos truncados en {}", e.name))?;
    match e.method {
        0 => Ok(raw.to_vec()),
        8 => {
            use flate2::read::DeflateDecoder;
            use std::io::Read;
            let mut d = DeflateDecoder::new(raw);
            let mut out = Vec::with_capacity(e.size as usize);
            d.read_to_end(&mut out)
                .map_err(|err| format!("inflate {}: {err}", e.name))?;
            Ok(out)
        }
        m => Err(format!("método zip no soportado: {m}")),
    }
}

/// Extrae un zip a un directorio, **de forma segura**.
pub fn zip_extract(bytes: &[u8], dest: &Path) -> Result<Vec<String>, String> {
    let mut written = Vec::new();
    for e in read_zip(bytes)? {
        if e.is_dir() {
            let rel = safe_relative_path(&e.name)?;
            let target = dest.join(&rel);
            std::fs::create_dir_all(&target).map_err(|err| format!("mkdir {}: {err}", target.display()))?;
            continue;
        }
        let rel = safe_relative_path(&e.name)?;
        let target = dest.join(&rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|err| format!("mkdir {}: {err}", parent.display()))?;
        }
        let data = zip_read_data(bytes, &e)?;
        std::fs::write(&target, &data).map_err(|err| format!("write {}: {err}", target.display()))?;
        written.push(rel.to_string_lossy().to_string());
    }
    Ok(written)
}

// ---------------------------------------------------------------------------
// Swift Backup: metadata.json (catálogo por app)
// ---------------------------------------------------------------------------

/// Partes de un backup de app (Swift Backup).
pub const SWIFT_PARTS: [&str; 5] = ["APP", "DATA", "EXTDATA", "EXPANSION", "MEDIA"];

/// `metadata.json` de Swift Backup (lectura de interop).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SwiftMetadata {
    /// Paquete de la app (si está presente).
    #[serde(default)]
    pub package: Option<String>,
    /// Lista de ficheros de la app.
    #[serde(default)]
    pub files: Vec<String>,
    /// Partes presentes en el backup.
    #[serde(default)]
    pub parts: Vec<String>,
    /// Marca de tiempo (epoch, si está presente).
    #[serde(default)]
    pub timestamp: Option<i64>,
}

/// Lee el `metadata.json` de Swift Backup.
pub fn read_swift_metadata(path: &str) -> Result<SwiftMetadata, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("no se puede leer {path}: {e}"))?;
    let meta: SwiftMetadata = serde_json::from_str(&raw).map_err(|e| format!("metadata.json inválido: {e}"))?;
    Ok(meta)
}

/// ¿Es esta parte una de las cinco de Swift Backup?
pub fn is_swift_part(part: &str) -> bool {
    SWIFT_PARTS.iter().any(|p| p.eq_ignore_ascii_case(part))
}

// ---------------------------------------------------------------------------
// zstd + TAR en streaming
// ---------------------------------------------------------------------------

/// Comprime con zstd (nivel 1..=22).
///
/// El crate `zstd` enlaza el **C de zstd embebido** (no la libzstd del
/// sistema): Android no trae ninguna ⇒ el binario ARM64 es autónomo.
pub fn zstd_compress(data: &[u8], level: i32) -> Result<Vec<u8>, String> {
    zstd::bulk::compress(data, level).map_err(|e| format!("zstd: {e}"))
}

/// Descomprime con zstd (con la longitud de salida conocida).
pub fn zstd_decompress(data: &[u8], capacity: usize) -> Result<Vec<u8>, String> {
    zstd::bulk::decompress(data, capacity).map_err(|e| format!("zstd: {e}"))
}

/// Lee las entradas de un **TAR comprimido con zstd**.
///
/// Es el interior de los contenedores de Swift Backup: `tar` + zstd
/// (`SbaSwiftTarNative::extractTarZstdFromFdRaw`).
pub fn read_tar_zstd(bytes: &[u8]) -> Result<Vec<TarEntry>, String> {
    let plain = zstd::bulk::decompress(bytes, bytes.len() * 8 + 4096)
        .map_err(|e| format!("zstd: {e}"))?;
    read_tar(&plain)
}

/// Extrae un **TAR comprimido con zstd** a un directorio, de forma segura.
pub fn tar_extract_zstd(bytes: &[u8], dest: &Path) -> Result<Vec<String>, String> {
    let plain = zstd::bulk::decompress(bytes, bytes.len() * 8 + 4096)
        .map_err(|e| format!("zstd: {e}"))?;
    tar_extract(&plain, dest)
}

/// Una entrada de un tar leída **en streaming**.
#[derive(Debug, Clone, PartialEq)]
pub struct TarStreamEntry {
    /// Ruta dentro del tar.
    pub name: String,
    /// Tamaño del contenido.
    pub size: u64,
    /// mtime (epoch).
    pub mtime: u64,
    /// Permisos.
    pub mode: u32,
    /// Byte de tipo.
    pub typeflag: u8,
}

/// Lector de tar **en streaming**.
///
/// Necesario para backups de gigas: el tar completo no cabe en la RAM del
/// móvil, así que se vaentry a entry, y el contenido se copia directamente al
/// destino.
pub struct TarStream<R: std::io::Read> {
    reader: R,
    buf: Vec<u8>,
    remaining: u64,
    done: bool,
    current: Option<TarStreamEntry>,
}

impl<R: std::io::Read> TarStream<R> {
    /// Crea el lector.
    pub fn new(reader: R) -> TarStream<R> {
        TarStream {
            reader,
            buf: vec![0u8; 512],
            remaining: 0,
            done: false,
            current: None,
        }
    }

    /// Lee la **siguiente** entrada (descarta el contenido de la anterior).
    ///
    /// Devuelve `Ok(None)` al final del tar.
    pub fn next_entry(&mut self) -> Result<Option<TarStreamEntry>, String> {
        if self.done {
            return Ok(None);
        }
        // Descarta lo que quede de la entrada anterior.
        if self.remaining > 0 {
            let mut sink = std::io::sink();
            let mut left = self.remaining;
            let mut chunk = vec![0u8; 64 * 1024];
            while left > 0 {
                let n = left.min(chunk.len() as u64) as usize;
                let read = self
                    .reader
                    .read(&mut chunk[..n])
                    .map_err(|e| format!("error leyendo el tar: {e}"))?;
                if read == 0 {
                    self.done = true;
                    return Ok(None);
                }
                left -= read as u64;
                std::io::Write::write_all(&mut sink, &chunk[..read])
                    .map_err(|e| format!("error descartando: {e}"))?;
            }
            self.remaining = 0;
        }
        loop {
            let n = self
                .reader
                .read(&mut self.buf)
                .map_err(|e| format!("error leyendo la cabecera del tar: {e}"))?;
            if n == 0 {
                self.done = true;
                return Ok(None);
            }
            if n < 512 {
                // rellena con ceros: cabecera parcial
                for b in self.buf[n..].iter_mut() {
                    *b = 0;
                }
            }
            if self.buf.iter().all(|&b| b == 0) {
                self.done = true;
                return Ok(None);
            }
            let size = octal(&self.buf[124..136]);
            let typeflag = self.buf[156];
            let mode = octal(&self.buf[100..108]) as u32;
            let mtime = octal(&self.buf[136..148]);
            let prefix = cstr(&self.buf[345..500]);
            let base = cstr(&self.buf[0..100]);
            let name = if prefix.is_empty() {
                base
            } else {
                format!("{prefix}/{base}")
            };
            self.remaining = size;
            let entry = TarStreamEntry {
                name,
                size,
                mtime,
                mode,
                typeflag,
            };
            self.current = Some(entry.clone());
            return Ok(Some(entry));
        }
    }

    /// Copia el contenido de la entrada actual a un escritor.
    ///
    /// Devuelve los bytes copiados. Si se pide menos de lo que queda, la
    /// entrada queda a medias (el siguiente `next_entry` descarta el resto).
    pub fn copy_data(&mut self, out: &mut impl std::io::Write) -> Result<u64, String> {
        let mut chunk = vec![0u8; 64 * 1024];
        let mut total = 0u64;
        while self.remaining > 0 {
            let want = self.remaining.min(chunk.len() as u64) as usize;
            let n = self
                .reader
                .read(&mut chunk[..want])
                .map_err(|e| format!("error leyendo el contenido: {e}"))?;
            if n == 0 {
                return Err("el tar termina antes de lo que dice la cabecera".to_string());
            }
            std::io::Write::write_all(out, &chunk[..n]).map_err(|e| format!("error escribiendo: {e}"))?;
            self.remaining -= n as u64;
            total += n as u64;
        }
        Ok(total)
    }
}

// ---------------------------------------------------------------------------
// Seguridad de rutas
// ---------------------------------------------------------------------------

/// Convierte una ruta de un backup en una ruta **segura** relativa.
///
/// Rechaza: rutas absolutas (`/x`), `..`, y rutas de Windows (`C:\`).
pub fn safe_relative_path(name: &str) -> Result<PathBuf, String> {
    let n = name.replace('\\', "/");
    if n.starts_with('/') {
        return Err(format!("ruta absoluta no permitida: {name}"));
    }
    if n.contains(':') {
        return Err(format!("ruta con unidad no permitida: {name}"));
    }
    let mut out = PathBuf::new();
    for part in n.split('/') {
        match part {
            "" | "." => continue,
            ".." => return Err(format!("salto de directorio no permitido: {name}")),
            p => out.push(p),
        }
    }
    if out.as_os_str().is_empty() {
        return Err(format!("ruta vacía: {name}"));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Swift Backup: recorrido del arbol de un backup
// ---------------------------------------------------------------------------

/// Un fichero dentro de un backup de Swift Backup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwiftBackupItem {
    /// Ruta relativa dentro del backup.
    pub path: String,
    /// Parte a la que pertenece (`APP`, `DATA`, `EXTDATA`, `EXPANSION`,
    /// `MEDIA`), si se deduce de la ruta.
    pub part: Option<String>,
    /// Extensión del fichero (sin punto, en minúsculas).
    pub extension: String,
    /// Tamaño en bytes.
    pub size: u64,
}

/// Extensión ⇒ parte probable de Swift Backup (medido en la app instalada).
pub fn swift_extension_part(ext: &str) -> Option<&'static str> {
    match ext {
        "app" | "apk" | "splits" => Some("APP"),
        "dat" => Some("DATA"),
        "extdat" => Some("EXTDATA"),
        "med" | "media" => Some("MEDIA"),
        "extra" => Some("EXTRA"),
        _ => None,
    }
}

/// Recorre un directorio de backup de Swift Backup y lista sus ficheros.
///
/// **Sólo-lectura**: no abre ni modifica ningún fichero de datos (los `.dat`
/// van cifrados con AEGIS-256 y comprimidos con zstd).
pub fn scan_swift_backup(root: &str) -> Result<Vec<SwiftBackupItem>, String> {
    let mut out = Vec::new();
    walk_swift(root, root, &mut out, 0)?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn walk_swift(root: &str, dir: &str, out: &mut Vec<SwiftBackupItem>, depth: usize) -> Result<(), String> {
    if depth > 12 {
        return Err("árbol demasiado profundo".to_string());
    }
    let rd = std::fs::read_dir(dir).map_err(|e| format!("no se puede leer {dir}: {e}"))?;
    for entry in rd {
        let entry = entry.map_err(|e| format!("no se puede leer {dir}: {e}"))?;
        let path = entry.path();
        let p = path.to_string_lossy().to_string();
        if path.is_dir() {
            walk_swift(root, &p, out, depth + 1)?;
            continue;
        }
        let meta = entry
            .metadata()
            .map_err(|e| format!("no se puede ver {p}: {e}"))?;
        let rel = p
            .strip_prefix(root)
            .unwrap_or(&p)
            .trim_start_matches('/')
            .to_string();
        let extension = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        // la parte deducida del directorio contenedor (si es una de las 5)
        let part = path
            .parent()
            .and_then(|d| d.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .filter(|n| is_swift_part(n))
            .or_else(|| swift_extension_part(&extension).map(|s| s.to_string()));
        out.push(SwiftBackupItem {
            path: rel,
            part,
            extension,
            size: meta.len(),
        });
    }
    Ok(())
}
