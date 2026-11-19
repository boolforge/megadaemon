//! MegaDaemon v13.4 — Inventario, análisis y detección de deriva de APKs.
//!
//! Todo en Rust, sin Python y **sin reescribir el soporte ZIP**: los APK se
//! leen con el crate `zip` (ya en `Cargo.toml`, features `deflate`) que se
//! alimenta de un lector `Read + Seek` propio que trae los bytes del
//! dispositivo con `crate::android::root_exec` + base64.  Así se reutiliza el
//! ZIP trillado (que ya sabe de ZIP64, deflate, data descriptors…) en vez de
//! duplicarlo.
//!
//! El dispositivo se consulta siempre con `crate::android::root_exec`, que ya
//! envuelve el `su -mm -c` (no hay que añadirlo).
//!
//! API pública:
//!
//! | Función | Qué hace |
//! |---|---|
//! | `inventory(limit)` | Apps instaladas: ruta, tamaño, SHA-256, permisos, componentes y huella |
//! | `analyze_apk(path)` | Análisis de UNA APK: entradas, DEX, `.so` nativas, assets y manifiesto |
//! | `manifest_axml(buf)` | Parser propio del XML binario de Android (AXML) |
//! | `manifest_fingerprint(json)` | SHA-256 de la forma canónica del manifiesto (detecta cambios) |
//! | `drift(known)` | Inventario actual contra el manifiesto previo → cambios/additions/bajas |
//! | `dump_all(out_dir)` | Vuelca todas las APK instaladas a un directorio (rutas + hashes) |

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{Read, Seek, SeekFrom};
use std::time::Instant;

/// Bytes que se piden al dispositivo en cada bloque (288 KiB).
///
/// 288 KiB → 393216 caracteres en base64, por debajo del tope de 512 KiB que
/// impone `android::root_exec`: ningún bloque se trunca nunca.
const CHUNK_BYTES: usize = 288 * 1024;

/// Timeout de una lectura del dispositivo.
const READ_TIMEOUT_MS: u64 = 90_000;

/// Timeout de las consultas al gestor de paquetes.
const PM_TIMEOUT_MS: u64 = 60_000;

/// Timeout de `dumpsys package`.
const DUMPSYS_TIMEOUT_MS: u64 = 45_000;

/// Entradas del ZIP que se listan como máximo en la respuesta.
const MAX_ENTRIES_REPORTED: usize = 200;

/// Assets que se listan como máximo en la respuesta.
const MAX_ASSETS_REPORTED: usize = 100;

/// Paquetes que se procesan como máximo en un barrido completo.
const MAX_PACKAGES: usize = 2000;

/// Hilos que hashean APK en paralelo (el SHA-256 es lo caro: son muchas
/// lecturas del dispositivo).
const HASH_WORKERS: usize = 4;

/// Paquetes que mira `drift` en cada barrido.
const DRIFT_PACKAGE_LIMIT: usize = 200;

// ========================================================================= //
// Utilidades de bajo nivel: hash y base64
// ========================================================================= //

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

/// SHA-256 en hexadecimal. Es la única forma de calcular hashes en este módulo.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex(&hasher.finalize())
}

/// Decodifica base64 estándar (ignora espacios y saltos de línea).
pub fn decode_b64(s: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut rev = [255u8; 256];
    for (i, c) in TABLE.iter().enumerate() {
        rev[*c as usize] = i as u8;
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for ch in s.bytes() {
        if ch == b'=' {
            break;
        }
        let v = rev[ch as usize];
        if v == 255 {
            continue;
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

// ========================================================================= //
// Acceso a los ficheros del dispositivo
// ========================================================================= //

/// Lector de un fichero del dispositivo.
///
/// Si el binario corre EN el dispositivo y el fichero se puede abrir
/// directamente, se usa la E/S local (rápido).  Si no, se pide por bloques con
/// `root_exec` + base64.  En ambos casos implementa `Read + Seek`, que es
/// justo lo que necesita `zip::ZipArchive` para no reimplementar el ZIP.
struct DeviceFile {
    path: String,
    size: u64,
    pos: u64,
    local: Option<std::fs::File>,
    cache: Option<(u64, Vec<u8>)>,
}

impl DeviceFile {
    /// Abre un fichero del dispositivo (local si se puede, remoto si no).
    fn open(path: &str) -> Option<DeviceFile> {
        if path.is_empty() {
            return None;
        }
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.is_file() {
                if let Ok(f) = std::fs::File::open(path) {
                    return Some(DeviceFile {
                        path: path.to_string(),
                        size: meta.len(),
                        pos: 0,
                        local: Some(f),
                        cache: None,
                    });
                }
            }
        }
        // Remoto: el tamaño lo da `stat` a través de root.
        let r = crate::android::root_exec(
            &format!("stat -c %s {} 2>/dev/null", crate::android::sh_quote(path)),
            20_000,
            1024,
        );
        let size = r.stdout.trim().parse::<u64>().ok()?;
        if size == 0 {
            return None;
        }
        Some(DeviceFile {
            path: path.to_string(),
            size,
            pos: 0,
            local: None,
            cache: None,
        })
    }

    /// Tamaño del fichero en bytes.
    fn size(&self) -> u64 {
        self.size
    }

    /// Pide un bloque de bytes al dispositivo (base64) y lo decodifica.
    fn fetch(&mut self, offset: u64, len: usize) -> Option<Vec<u8>> {
        if len == 0 {
            return None;
        }
        let quoted = crate::android::sh_quote(&self.path);
        let cmd = format!(
            "tail -c +{} {} 2>/dev/null | head -c {} | base64 -w 0",
            offset + 1,
            quoted,
            len
        );
        let r = crate::android::root_exec(&cmd, READ_TIMEOUT_MS, 512 * 1024);
        let mut bytes = decode_b64(&r.stdout).unwrap_or_default();
        if bytes.is_empty() {
            // toybox ancient: `base64` sin `-w`. Se reintenta sin la opción.
            let cmd2 = format!(
                "tail -c +{} {} 2>/dev/null | head -c {} | base64 | tr -d '\\n'",
                offset + 1,
                quoted,
                len
            );
            let r2 = crate::android::root_exec(&cmd2, READ_TIMEOUT_MS, 512 * 1024);
            bytes = decode_b64(&r2.stdout).unwrap_or_default();
        }
        if bytes.is_empty() {
            None
        } else {
            Some(bytes)
        }
    }

    /// Asegura que la caché cubre la posición actual.
    fn ensure_cache(&mut self) -> Option<()> {
        if let Some((start, data)) = self.cache.as_ref() {
            if *start <= self.pos && (self.pos - start) < data.len() as u64 {
                return Some(());
            }
        }
        let want = CHUNK_BYTES.min((self.size.saturating_sub(self.pos)) as usize);
        if want == 0 {
            return None;
        }
        let data = self.fetch(self.pos, want)?;
        self.cache = Some((self.pos, data));
        Some(())
    }
}

impl Read for DeviceFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.pos >= self.size {
            return Ok(0);
        }
        if let Some(f) = self.local.as_mut() {
            f.seek(SeekFrom::Start(self.pos))?;
            let n = f.read(buf)?;
            self.pos += n as u64;
            return Ok(n);
        }
        self.ensure_cache()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "dispositivo sin datos"))?;
        let (start, data) = match self.cache.as_ref() {
            Some(c) => c,
            None => return Ok(0),
        };
        let off = (self.pos - start) as usize;
        if off >= data.len() {
            return Ok(0);
        }
        let n = (data.len() - off).min(buf.len());
        buf[..n].copy_from_slice(&data[off..off + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for DeviceFile {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let target = match pos {
            SeekFrom::Start(p) => p as i64,
            SeekFrom::End(d) => self.size as i64 + d,
            SeekFrom::Current(d) => self.pos as i64 + d,
        };
        if target < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "posición negativa",
            ));
        }
        self.pos = target as u64;
        if let Some(f) = self.local.as_mut() {
            f.seek(SeekFrom::Start(self.pos))?;
        }
        Ok(self.pos)
    }
}

/// Tamaño de un fichero del dispositivo en bytes (0 si no se puede leer).
pub fn file_size(path: &str) -> u64 {
    DeviceFile::open(path).map(|f| f.size()).unwrap_or(0)
}

/// SHA-256 de un fichero del dispositivo, leído a trozos.
///
/// El hash se calcula **en Rust** con `sha2`; los bytes viajan en base64 por
/// bloques desde el dispositivo.
pub fn sha256_file(path: &str) -> Option<String> {
    let mut file = DeviceFile::open(path)?;
    if file.size() == 0 {
        return None;
    }
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK_BYTES];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(hex(&hasher.finalize()))
}

// ========================================================================= //
// ZIP: se reutiliza el crate `zip`, no se reimplementa
// ========================================================================= //

/// Datos de una entrada del ZIP del APK.
#[derive(Clone, Debug)]
pub struct ZipEntry {
    pub name: String,
    pub method: u16,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
    pub local_header_offset: u64,
}

/// Abre el APK con el crate `zip` y devuelve (entradas, AndroidManifest.xml).
fn open_apk(path: &str) -> Option<(Vec<ZipEntry>, Option<Vec<u8>>)> {
    let file = DeviceFile::open(path)?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let mut entries = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        // Una entrada corrupta no puede tumbar el análisis entero.
        let Ok(entry) = archive.by_index(i) else {
            continue;
        };
        entries.push(ZipEntry {
            name: entry.name().to_string(),
            method: if matches!(entry.compression(), zip::CompressionMethod::Stored) {
                0
            } else {
                8
            },
            compressed_size: entry.compressed_size(),
            uncompressed_size: entry.size(),
            local_header_offset: 0,
        });
    }
    let manifest = match archive.by_name("AndroidManifest.xml") {
        Ok(mut entry) => {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).ok().map(|_| buf)
        }
        Err(_) => None,
    };
    Some((entries, manifest))
}

/// Entradas del APK sin descargar el cuerpo de los ficheros.
pub fn apk_entries(apk_path: &str) -> Vec<ZipEntry> {
    open_apk(apk_path).map(|(e, _)| e).unwrap_or_default()
}

// ========================================================================= //
// AXML: parser propio del XML binario de Android
// ========================================================================= //

const AXML_MAGIC: u32 = 0x0008_0003;
const CHUNK_STRING_POOL: u16 = 0x0001;
const CHUNK_START_ELEMENT: u16 = 0x0102;
const CHUNK_END_ELEMENT: u16 = 0x0103;

const TYPE_REFERENCE: u8 = 0x01;
const TYPE_STRING: u8 = 0x03;
const TYPE_FLOAT: u8 = 0x04;
const TYPE_INT_DEC: u8 = 0x10;
const TYPE_INT_HEX: u8 = 0x11;
const TYPE_INT_BOOLEAN: u8 = 0x12;

/// Valor de un atributo del manifiesto.
#[derive(Clone, Debug)]
enum AttrVal {
    Str(String),
    Int(i64),
    Bool(bool),
    Ref(u32),
    Float(f64),
}

impl AttrVal {
    fn as_string(&self) -> Option<String> {
        match self {
            AttrVal::Str(s) => Some(s.clone()),
            AttrVal::Int(i) => Some(i.to_string()),
            AttrVal::Bool(b) => Some(b.to_string()),
            AttrVal::Ref(r) => Some(format!("@0x{:08x}", r)),
            AttrVal::Float(f) => Some(f.to_string()),
        }
    }

    fn as_int(&self) -> Option<i64> {
        match self {
            AttrVal::Int(i) => Some(*i),
            AttrVal::Bool(b) => Some(if *b { 1 } else { 0 }),
            AttrVal::Str(s) => s.parse().ok(),
            _ => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        match self {
            AttrVal::Bool(b) => Some(*b),
            AttrVal::Int(i) => Some(*i != 0),
            _ => None,
        }
    }
}

/// Un atributo del manifiesto (con su espacio de nombres).
#[derive(Clone, Debug)]
struct AxmlAttr {
    ns: String,
    name: String,
    value: AttrVal,
}

/// Filtro de intención de un componente.
#[derive(Clone, Debug, Default)]
struct Filter {
    actions: Vec<String>,
    categories: Vec<String>,
}

/// Componente declarado en el manifiesto.
#[derive(Clone, Debug, Default)]
struct Component {
    name: String,
    exported: Option<bool>,
    filters: Vec<Filter>,
}

/// Todo lo que se extrae del manifiesto.
#[derive(Clone, Debug, Default)]
struct ManifestData {
    package: Option<String>,
    version_code: Option<i64>,
    version_name: Option<String>,
    min_sdk: Option<i64>,
    target_sdk: Option<i64>,
    permissions: Vec<String>,
    components: Vec<(String, Component)>,
    strings_found: usize,
    errors: Vec<String>,
}

fn rd_u16(buf: &[u8], off: usize) -> u16 {
    if off + 2 > buf.len() {
        return 0;
    }
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

fn rd_u32(buf: &[u8], off: usize) -> u32 {
    if off + 4 > buf.len() {
        return 0;
    }
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

/// Extrae el *string pool* de un AXML (UTF-16 o UTF-8 según el bit 8).
fn parse_string_pool(buf: &[u8], start: usize) -> Vec<String> {
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
    let mut out = Vec::with_capacity(string_count);
    for i in 0..string_count {
        let off = rd_u32(buf, offsets_base + i * 4) as usize;
        out.push(parse_pool_string(buf, data_base + off, utf8));
    }
    out
}

/// Lee una cadena del pool (precedida de su longitud).
fn parse_pool_string(buf: &[u8], p: usize, utf8: bool) -> String {
    if p >= buf.len() {
        return String::new();
    }
    if utf8 {
        // Longitud en 1 byte, o en 2 si el bit alto está activo.
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
        // Longitud en UTF-16: u16, o u32 si el bit 15 está activo.
        let first = rd_u16(buf, p);
        let (units, start) = if first & 0x8000 != 0 {
            let second = rd_u16(buf, p + 2);
            (
                (((first & 0x7fff) as usize) << 16) | second as usize,
                p + 4,
            )
        } else {
            (first as usize, p + 2)
        };
        let end = (start + units * 2).min(buf.len());
        let slice = &buf[start..end];
        let mut codes: Vec<u16> = slice
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        while codes.last() == Some(&0) {
            codes.pop();
        }
        String::from_utf16_lossy(&codes)
    }
}

/// Convierte el `Res_value` de un atributo en un valor usable.
fn attr_value(pool: &[String], v_type: u8, data: u32, raw: u32) -> AttrVal {
    match v_type {
        TYPE_STRING => {
            // `data` es el índice en el pool; `raw` el del valor original.
            let idx = if raw != u32::MAX && (raw as usize) < pool.len() {
                raw as usize
            } else {
                data as usize
            };
            match pool.get(idx) {
                Some(s) => AttrVal::Str(s.clone()),
                None => AttrVal::Str(String::new()),
            }
        }
        TYPE_INT_DEC => AttrVal::Int(data as i32 as i64),
        TYPE_INT_HEX => AttrVal::Int(data as i32 as i64),
        TYPE_INT_BOOLEAN => AttrVal::Bool(data != 0),
        TYPE_REFERENCE => AttrVal::Ref(data),
        TYPE_FLOAT => AttrVal::Float(f32::from_bits(data) as f64),
        _ => AttrVal::Int(data as i64),
    }
}

/// Parsea un elemento inicial (start element) con sus atributos.
fn parse_start_element(buf: &[u8], pos: usize, pool: &[String]) -> (String, Vec<AxmlAttr>) {
    let header_size = rd_u16(buf, pos + 2) as usize;
    let ext = pos + header_size; // inicio de ResXMLTree_attrExt
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
        let ns_idx = rd_u32(buf, a);
        let name_idx = rd_u32(buf, a + 4);
        let raw = rd_u32(buf, a + 8);
        let v_type = buf[a + 15];
        let data = rd_u32(buf, a + 16);
        attrs.push(AxmlAttr {
            ns: pool.get(ns_idx as usize).cloned().unwrap_or_default(),
            name: pool.get(name_idx as usize).cloned().unwrap_or_default(),
            value: attr_value(pool, v_type, data, raw),
        });
    }
    (
        pool.get(name_idx as usize).cloned().unwrap_or_default(),
        attrs,
    )
}

/// Busca un atributo por nombre (prefirando el espacio de nombres de Android).
fn find_attr(attrs: &[AxmlAttr], name: &str) -> Option<AttrVal> {
    attrs
        .iter()
        .find(|a| a.name == name && a.ns.contains("schemas.android.com"))
        .or_else(|| attrs.iter().find(|a| a.name == name))
        .map(|a| a.value.clone())
}

/// Parsea un AndroidManifest.xml binario (AXML).
fn parse_axml(buf: &[u8]) -> ManifestData {
    let mut m = ManifestData::default();
    if buf.len() < 8 || rd_u32(buf, 0) != AXML_MAGIC {
        m.errors
            .push("no es un AXML válido (magic incorrecto)".to_string());
        return m;
    }
    let total = rd_u32(buf, 4) as usize;
    let end = total.min(buf.len());
    let mut pool: Vec<String> = Vec::new();
    let mut pos = 8usize;
    let mut stack: Vec<String> = Vec::new();
    let mut current_component: Option<usize> = None;
    let mut current_filter: Option<Filter> = None;

    while pos + 8 <= end {
        let chunk_type = rd_u16(buf, pos);
        let chunk_size = rd_u32(buf, pos + 4) as usize;
        if chunk_size < 8 {
            break;
        }
        match chunk_type {
            CHUNK_STRING_POOL => {
                pool = parse_string_pool(buf, pos);
                m.strings_found = pool.len();
            }
            CHUNK_START_ELEMENT => {
                let (name, attrs) = parse_start_element(buf, pos, &pool);
                match name.as_str() {
                    "manifest" => {
                        m.package = find_attr(&attrs, "package").and_then(|v| v.as_string());
                        m.version_code = find_attr(&attrs, "versionCode").and_then(|v| v.as_int());
                        m.version_name = find_attr(&attrs, "versionName").and_then(|v| v.as_string());
                    }
                    "uses-sdk" => {
                        m.min_sdk = find_attr(&attrs, "minSdkVersion").and_then(|v| v.as_int());
                        m.target_sdk = find_attr(&attrs, "targetSdkVersion").and_then(|v| v.as_int());
                    }
                    "uses-permission" | "uses-permission-sdk-23" | "uses-permission-sdk-m" => {
                        if let Some(p) = find_attr(&attrs, "name").and_then(|v| v.as_string()) {
                            if !p.is_empty() {
                                m.permissions.push(p);
                            }
                        }
                    }
                    "activity" | "activity-alias" | "service" | "receiver" | "provider" => {
                        let kind = match name.as_str() {
                            "activity" | "activity-alias" => "activity",
                            "service" => "service",
                            "receiver" => "receiver",
                            _ => "provider",
                        };
                        let comp = Component {
                            name: find_attr(&attrs, "name")
                                .and_then(|v| v.as_string())
                                .unwrap_or_default(),
                            exported: find_attr(&attrs, "exported").and_then(|v| v.as_bool()),
                            filters: Vec::new(),
                        };
                        m.components.push((kind.to_string(), comp));
                        current_component = Some(m.components.len() - 1);
                    }
                    "intent-filter" => {
                        current_filter = Some(Filter::default());
                    }
                    "action" => {
                        if let Some(a) = find_attr(&attrs, "name").and_then(|v| v.as_string()) {
                            if let Some(f) = current_filter.as_mut() {
                                f.actions.push(a);
                            }
                        }
                    }
                    "category" => {
                        if let Some(c) = find_attr(&attrs, "name").and_then(|v| v.as_string()) {
                            if let Some(f) = current_filter.as_mut() {
                                f.categories.push(c);
                            }
                        }
                    }
                    _ => {}
                }
                stack.push(name);
            }
            CHUNK_END_ELEMENT => {
                let name = stack.pop().unwrap_or_default();
                if name == "intent-filter" {
                    if let (Some(filter), Some(idx)) = (current_filter.take(), current_component) {
                        if let Some((_, comp)) = m.components.get_mut(idx) {
                            comp.filters.push(filter);
                        }
                    }
                } else if matches!(
                    name.as_str(),
                    "activity" | "activity-alias" | "service" | "receiver" | "provider"
                ) {
                    current_component = None;
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

/// Convierte un componente a JSON.
fn component_json(c: &Component) -> Value {
    json!({
        "name": c.name,
        "exported": c.exported,
        "intent_filters": c.filters.iter().map(|f| json!({
            "actions": f.actions,
            "categories": f.categories,
        })).collect::<Vec<Value>>(),
    })
}

/// Parsea un AndroidManifest.xml binario (AXML) y devuelve su contenido.
///
/// Formato: `{ package, version_code, version_name, min_sdk, target_sdk,
/// permissions, activities, services, receivers, providers, strings_found }`.
pub fn manifest_axml(buf: &[u8]) -> Value {
    let m = parse_axml(buf);
    let of_kind = |kind: &str| -> Vec<Value> {
        m.components
            .iter()
            .filter(|(k, _)| k == kind)
            .map(|(_, c)| component_json(c))
            .collect()
    };
    json!({
        "package": m.package,
        "version_code": m.version_code,
        "version_name": m.version_name,
        "min_sdk": m.min_sdk,
        "target_sdk": m.target_sdk,
        "permissions": m.permissions,
        "activities": of_kind("activity"),
        "services": of_kind("service"),
        "receivers": of_kind("receiver"),
        "providers": of_kind("provider"),
        "strings_found": m.strings_found,
        "errors": m.errors,
    })
}

// ========================================================================= //
// Huella del manifiesto (para detectar cambios entre versiones)
// ========================================================================= //

/// Forma canónica de un componente (estable frente a reordenamientos).
fn component_canonical(c: &Value) -> String {
    let name = c["name"].as_str().unwrap_or("");
    let exported = match c["exported"] {
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        ref other => other.to_string(),
    };
    let mut filters: Vec<String> = c["intent_filters"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|f| {
                    let actions: Vec<&str> = f["actions"]
                        .as_array()
                        .map(|x| x.iter().filter_map(|v| v.as_str()).collect())
                        .unwrap_or_default();
                    let cats: Vec<&str> = f["categories"]
                        .as_array()
                        .map(|x| x.iter().filter_map(|v| v.as_str()).collect())
                        .unwrap_or_default();
                    format!("{}=>[{}]", actions.join("|"), cats.join("|"))
                })
                .collect()
        })
        .unwrap_or_default();
    filters.sort();
    format!("{}|{}|{}", name, exported, filters.join(";"))
}

/// Serialización canónica del manifiesto: lo que se hashea para compararlo.
pub fn manifest_canonical(manifest: &Value) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "package={}\n",
        manifest["package"].as_str().unwrap_or("")
    ));
    out.push_str(&format!(
        "version_code={}\n",
        manifest["version_code"].as_i64().unwrap_or(-1)
    ));
    out.push_str(&format!(
        "version_name={}\n",
        manifest["version_name"].as_str().unwrap_or("")
    ));
    out.push_str(&format!(
        "min_sdk={}\ntarget_sdk={}\n",
        manifest["min_sdk"].as_i64().unwrap_or(-1),
        manifest["target_sdk"].as_i64().unwrap_or(-1)
    ));
    let mut perms: Vec<String> = manifest["permissions"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    perms.sort();
    perms.dedup();
    out.push_str(&format!("permissions={}\n", perms.join(",")));
    for group in ["activities", "services", "receivers", "providers"] {
        let mut comps: Vec<String> = manifest[group]
            .as_array()
            .map(|a| a.iter().map(component_canonical).collect())
            .unwrap_or_default();
        comps.sort();
        out.push_str(&format!("{}={}\n", group, comps.join(";")));
    }
    out
}

/// SHA-256 de la forma canónica del manifiesto.
///
/// Es la **huella** de la app: si dos versionesInstalled tienen la misma
/// huella, el manifiesto no ha cambiado (aunque el binario sí).
pub fn manifest_fingerprint(manifest: &Value) -> String {
    sha256_hex(manifest_canonical(manifest).as_bytes())
}

// ========================================================================= //
// Inventario de apps instaladas
// ========================================================================= //

/// Extrae las rutas de APK de la salida de `pm list packages -f`.
///
/// El formato real es `package:/ruta/app.apk` (NO hay `=`).
fn parse_apk_paths(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|l| l.trim().strip_prefix("package:"))
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Extrae los nombres de paquete de la salida de `pm list packages`.
fn parse_package_names(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|l| l.trim().strip_prefix("package:"))
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Deduce el nombre de paquete de una ruta de APK (`.../com.foo-1/base.apk`).
fn package_from_path(path: &str) -> Option<String> {
    let p = path.trim_end_matches('/');
    let dir = p.rsplit('/').nth(1)?;
    let candidate = match dir.rsplit_once('-') {
        Some((head, tail)) if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) => head,
        _ => dir,
    };
    if candidate.contains('.') && !candidate.is_empty() {
        Some(candidate.to_string())
    } else {
        None
    }
}

/// Empareja nombres de paquete con rutas de APK.
///
/// `pm list packages` y `pm list packages -f` devuelven el mismo orden, así
/// que se emparejan por índice; cuando la ruta permite deducir el nombre
/// (layout moderno de Android) se usa esa deducción, que es más fiable.
fn pair_packages(names: &[String], paths: &[String]) -> Vec<(String, String)> {
    let mut out = Vec::with_capacity(paths.len());
    for (i, path) in paths.iter().enumerate() {
        let deduced = package_from_path(path);
        let name = match deduced {
            Some(d) if names.contains(&d) => d,
            _ => names.get(i).cloned().unwrap_or_default(),
        };
        out.push((name, path.clone()));
    }
    out
}

/// Lista los paquetes instalados y la ruta de su APK.
fn list_packages() -> Vec<(String, String)> {
    let with_paths = crate::android::root_exec(
        "pm list packages -f 2>/dev/null",
        PM_TIMEOUT_MS,
        512 * 1024,
    );
    let paths = parse_apk_paths(&with_paths.stdout);
    let names = if paths.len() > 1 {
        let plain = crate::android::root_exec("pm list packages 2>/dev/null", PM_TIMEOUT_MS, 512 * 1024);
        parse_package_names(&plain.stdout)
    } else {
        Vec::new()
    };
    pair_packages(&names, &paths)
}

/// Versión de un paquete según `dumpsys package`.
fn dumpsys_version(pkg: &str) -> (Option<String>, Option<i64>) {
    let cmd = format!(
        "dumpsys package {} 2>/dev/null | grep -E 'versionCode=|versionName=' | head -6",
        crate::android::sh_quote(pkg)
    );
    let r = crate::android::root_exec(&cmd, DUMPSYS_TIMEOUT_MS, 16 * 1024);
    let mut name = None;
    let mut code = None;
    for line in r.stdout.lines() {
        let t = line.trim();
        if let Some(v) = t.strip_prefix("versionCode=") {
            if code.is_none() {
                code = v.split_whitespace().next().and_then(|x| x.parse::<i64>().ok());
            }
        }
        if let Some(v) = t.strip_prefix("versionName=") {
            if name.is_none() {
                let v = v.trim();
                if !v.is_empty() && v != "null" {
                    name = Some(v.to_string());
                }
            }
        }
    }
    (name, code)
}

/// Análisis interno de un APK (sin el hash, que se calcula aparte).
struct ApkAnalysis {
    entries: Vec<ZipEntry>,
    dex_files: Vec<String>,
    native_libs: Vec<String>,
    abis: Vec<String>,
    assets: Vec<String>,
    has_resources_arsc: bool,
    manifest: Value,
    fingerprint: String,
}

fn analyze_apk_internal(path: &str) -> Option<ApkAnalysis> {
    let (entries, manifest_bytes) = open_apk(path)?;
    let dex_files: Vec<String> = entries
        .iter()
        .filter(|e| e.name.starts_with("classes") && e.name.ends_with(".dex"))
        .map(|e| e.name.clone())
        .collect();
    let native_libs: Vec<String> = entries
        .iter()
        .filter(|e| e.name.starts_with("lib/") && e.name.ends_with(".so"))
        .map(|e| e.name.clone())
        .collect();
    let mut abis: Vec<String> = native_libs
        .iter()
        .filter_map(|l| l.strip_prefix("lib/"))
        .filter_map(|r| r.split('/').next())
        .map(String::from)
        .collect();
    abis.sort();
    abis.dedup();
    let assets: Vec<String> = entries
        .iter()
        .filter(|e| e.name.starts_with("assets/"))
        .map(|e| e.name.clone())
        .take(MAX_ASSETS_REPORTED)
        .collect();
    let has_resources_arsc = entries.iter().any(|e| e.name == "resources.arsc");
    let manifest = match manifest_bytes {
        Some(buf) => manifest_axml(&buf),
        None => json!({
            "package": Value::Null,
            "errors": json!(["AndroidManifest.xml no encontrado"]),
        }),
    };
    let fingerprint = manifest_fingerprint(&manifest);
    // `entries` se mueve en el literal; el tamaño se calcula antes del movimiento.
    Some(ApkAnalysis {
        entries,
        dex_files,
        native_libs,
        abis,
        assets,
        has_resources_arsc,
        manifest,
        fingerprint,
    })
}

/// Ficha de un paquete instalado.
fn package_record(pkg: &str, path: &str) -> Value {
    let mut rec = json!({
        "package": pkg,
        "apk_path": path,
        "size": 0,
        "version_name": Value::Null,
        "version_code": Value::Null,
        "sha256": Value::Null,
        "dex_count": 0,
        "native_libs": json!([]),
        "permissions": json!([]),
        "activities": json!([]),
        "services": json!([]),
        "receivers": json!([]),
        "fingerprint": Value::Null,
    });

    // Versión: `dumpsys package` es la fuente primaria.
    let (version_name, version_code) = dumpsys_version(pkg);
    if let Some(v) = version_name {
        rec["version_name"] = json!(v);
    }
    if let Some(v) = version_code {
        rec["version_code"] = json!(v);
    }

    // Tamaño y hash del APK.
    rec["size"] = json!(file_size(path));
    rec["sha256"] = match sha256_file(path) {
        Some(h) => json!(h),
        None => Value::Null,
    };

    // Manifiesto: permisos, componentes y huella.
    if let Some(a) = analyze_apk_internal(path) {
        rec["dex_count"] = json!(a.dex_files.len());
        rec["native_libs"] = json!(a.native_libs);
        rec["permissions"] = a.manifest["permissions"].clone();
        for group in ["activities", "services", "receivers"] {
            let names: Vec<String> = a.manifest[group]
                .as_array()
                .map(|list| {
                    list.iter()
                        .filter_map(|c| c["name"].as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            rec[group] = json!(names);
        }
        rec["fingerprint"] = json!(a.fingerprint);
        // El manifiesto sirve de respaldo si `dumpsys` no dio la versión.
        if rec["version_code"].is_null() {
            rec["version_code"] = a.manifest["version_code"].clone();
        }
        if rec["version_name"].is_null() {
            rec["version_name"] = a.manifest["version_name"].clone();
        }
    }
    rec
}

/// Calcula las fichas de varios paquetes en paralelo (el hash es lo caro).
fn package_records(items: &[(String, String)]) -> Vec<Value> {
    if items.is_empty() {
        return Vec::new();
    }
    let workers = HASH_WORKERS.min(items.len());
    if workers <= 1 {
        return items
            .iter()
            .map(|(p, a)| package_record(p, a))
            .collect();
    }
    let chunk = (items.len() + workers - 1) / workers;
    let mut out: Vec<Value> = Vec::with_capacity(items.len());
    std::thread::scope(|scope| {
        let handles: Vec<_> = items
            .chunks(chunk)
            .map(|group| {
                scope.spawn(move || {
                    group
                        .iter()
                        .map(|(p, a)| package_record(p, a))
                        .collect::<Vec<Value>>()
                })
            })
            .collect();
        for handle in handles {
            if let Ok(mut part) = handle.join() {
                out.append(&mut part);
            }
        }
    });
    out
}

/// Apps instaladas y sus APK.
///
/// `limit` es el número máximo de paquetes a procesar (0 = todos).
pub fn inventory(limit: usize) -> Value {
    let started = Instant::now();
    let all = list_packages();
    let max = if limit == 0 {
        MAX_PACKAGES
    } else {
        limit.min(MAX_PACKAGES)
    };
    let selected: Vec<(String, String)> = all.into_iter().take(max).collect();
    let mut packages = package_records(&selected);
    packages.sort_by(|a, b| {
        a["package"]
            .as_str()
            .unwrap_or("")
            .cmp(b["package"].as_str().unwrap_or(""))
    });
    json!({
        "ok": true,
        "count": packages.len(),
        "packages": packages,
        "duration_ms": started.elapsed().as_millis() as u64,
    })
}

/// Analiza UNA APK: entradas, DEX, librerías nativas, assets y manifiesto.
pub fn analyze_apk(path: &str) -> Value {
    if path.is_empty() || !path.starts_with('/') {
        return json!({
            "ok": false,
            "error": "bad_path",
            "path": path,
            "hint": "la ruta del APK debe ser absoluta",
        });
    }
    let Some(analysis) = analyze_apk_internal(path) else {
        return json!({
            "ok": false,
            "error": "cannot_read_apk",
            "path": path,
            "hint": "no se pudo abrir el ZIP del APK (¿ruta o permisos?)",
        });
    };
    let entries_json: Vec<Value> = analysis
        .entries
        .iter()
        .take(MAX_ENTRIES_REPORTED)
        .map(|e| {
            json!({
                "name": e.name,
                "size": e.uncompressed_size,
                "compressed_size": e.compressed_size,
                "method": e.method,
            })
        })
        .collect();
    let entry_count = analysis.entries.len();
    let entries_truncated = entry_count > MAX_ENTRIES_REPORTED;
    let manifest = analysis.manifest.clone();
    let fingerprint = analysis.fingerprint.clone();
    let dex_count = analysis.dex_files.len();
    let dex_files = analysis.dex_files;
    let native_libs = analysis.native_libs;
    let abis = analysis.abis;
    let assets = analysis.assets;
    let has_resources_arsc = analysis.has_resources_arsc;
    json!({
        "ok": true,
        "path": path,
        "size": file_size(path),
        "entry_count": entry_count,
        "entries": entries_json,
        "entries_truncated": entries_truncated,
        "dex_count": dex_count,
        "dex_files": dex_files,
        "native_libs": native_libs,
        "abis": abis,
        "assets": assets,
        "has_resources_arsc": has_resources_arsc,
        "manifest": manifest,
        "fingerprint": fingerprint,
        "sha256": sha256_file(path),
    })
}

// ========================================================================= //
// Deriva: qué ha cambiado desde el inventario previo
// ========================================================================= //

/// Compara un campo entre el manifiesto previo y el actual.
fn changed_field(prev: &Value, cur: &Value, field: &str) -> Option<(Value, Value)> {
    let a = prev.get(field).cloned().unwrap_or(Value::Null);
    let b = cur.get(field).cloned().unwrap_or(Value::Null);
    if a == b {
        None
    } else {
        Some((a, b))
    }
}

/// Compara el inventario actual con el previo (función ** pura**, testeable).
///
/// `known` = `{ "<paquete>": {"sha256":..,"fingerprint":..,"version_code":..} }`.
/// `current` = salida de `inventory` (`{ packages: [...] }`).
pub fn drift_compare(known: &Value, current: &Value) -> Value {
    let mut changed: Vec<Value> = Vec::new();
    let mut added: Vec<String> = Vec::new();
    let mut removed: Vec<String> = Vec::new();
    let known_map = known.as_object().cloned().unwrap_or_default();
    let items = current["packages"].as_array().cloned().unwrap_or_default();
    let mut current_names: Vec<String> = Vec::new();

    for item in &items {
        let pkg = item["package"].as_str().unwrap_or("").to_string();
        if pkg.is_empty() {
            continue;
        }
        current_names.push(pkg.clone());
        match known_map.get(&pkg) {
            Some(prev) => {
                if let Some((from, to)) = changed_field(prev, item, "sha256") {
                    changed.push(json!({
                        "package": pkg, "from": from, "to": to, "kind": "binary",
                    }));
                }
                if let Some((from, to)) = changed_field(prev, item, "fingerprint") {
                    changed.push(json!({
                        "package": pkg, "from": from, "to": to, "kind": "manifest",
                    }));
                }
                if let Some((from, to)) = changed_field(prev, item, "version_code") {
                    changed.push(json!({
                        "package": pkg, "from": from, "to": to, "kind": "version",
                    }));
                }
            }
            None => added.push(pkg),
        }
    }
    for pkg in known_map.keys() {
        if !current_names.contains(pkg) {
            removed.push(pkg.clone());
        }
    }
    added.sort();
    removed.sort();
    json!({
        "ok": true,
        "changed": changed,
        "added": added,
        "removed": removed,
        "changed_count": changed.len(),
        "added_count": added.len(),
        "removed_count": removed.len(),
    })
}

/// Compara el inventario actual contra el manifiesto previo.
///
/// `known` = `{ "<paquete>": {"sha256":..,"fingerprint":..,"version_code":..} }`.
pub fn drift(known: &Value) -> Value {
    let current = inventory(DRIFT_PACKAGE_LIMIT);
    let mut out = drift_compare(known, &current);
    if let Some(map) = out.as_object_mut() {
        map.insert("scanned".to_string(), json!(current["count"]));
    }
    out
}

// ========================================================================= //
// Volcado de todas las APK
// ========================================================================= //

/// Copia un fichero (local si se puede, `cp` de root si no).
fn copy_file(src: &str, dest: &str) -> bool {
    if std::fs::copy(src, dest).is_ok() {
        return true;
    }
    let cmd = format!(
        "cp {} {} 2>/dev/null",
        crate::android::sh_quote(src),
        crate::android::sh_quote(dest)
    );
    crate::android::root_exec(&cmd, 300_000, 1024).code == Some(0)
}

/// Vuelca todas las APK instaladas a un directorio.
///
/// Devuelve rutas y hashes; **nunca** devuelve los binarios.
pub fn dump_all(out_dir: &str) -> Value {
    if out_dir.is_empty() || !out_dir.starts_with('/') || out_dir.contains("..") {
        return json!({
            "ok": false,
            "error": "bad_path",
            "hint": "out_dir debe ser una ruta absoluta sin '..'",
        });
    }
    let mk = crate::android::root_exec(
        &format!("mkdir -p {}", crate::android::sh_quote(out_dir)),
        30_000,
        1024,
    );
    if mk.code != Some(0) {
        return json!({
            "ok": false,
            "error": "mkdir_failed",
            "out_dir": out_dir,
            "detail": mk.stderr.trim().chars().take(200).collect::<String>(),
        });
    }
    let packages = list_packages();
    let mut files: Vec<Value> = Vec::new();
    let mut errors: Vec<Value> = Vec::new();
    let mut total_bytes = 0u64;
    for (pkg, src) in packages {
        if src.is_empty() {
            continue;
        }
        let dest = format!("{}/{}.apk", out_dir, pkg);
        if !copy_file(&src, &dest) {
            errors.push(json!({ "package": pkg, "error": "copy_failed" }));
            continue;
        }
        let size = file_size(&dest);
        total_bytes += size;
        files.push(json!({
            "package": pkg,
            "source": src,
            "path": dest,
            "size": size,
            "sha256": match sha256_file(&dest) {
                Some(h) => json!(h),
                None => Value::Null,
            },
        }));
    }
    json!({
        "ok": true,
        "out_dir": out_dir,
        "count": files.len(),
        "total_bytes": total_bytes,
        "files": files,
        "errors": errors,
    })
}

// ========================================================================= //
// Tests
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;

    // --- Utilidades para los tests (todo sintético, sin red ni dispositivo) ---

    /// Ruta de un fichero temporal de test (dentro del workspace).
    fn test_path(name: &str) -> std::path::PathBuf {
        let base = std::env::var("CARGO_TARGET_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("target"));
        let dir = base.join("apk_inventory_tests");
        let _ = std::fs::create_dir_all(&dir);
        dir.join(name)
    }

    /// Valor de atributo para el constructor de AXML.
    #[derive(Clone)]
    enum TestVal {
        S(&'static str),
        I(i64),
        B(bool),
    }

    /// Atributo preparado para el constructor de AXML.
    struct TestAttr {
        ns: u32,
        name: u32,
        vtype: u8,
        data: u32,
        raw: u32,
    }

    const ANDROID_NS: &str = "http://schemas.android.com/apk/res/android";

    /// Indexa una cadena en el pool y devuelve su índice.
    fn intern(pool: &mut Vec<String>, s: &str) -> u32 {
        if let Some(i) = pool.iter().position(|x| x == s) {
            return i as u32;
        }
        pool.push(s.to_string());
        (pool.len() - 1) as u32
    }

    /// Construye el chunk del string pool en UTF-16.
    fn build_string_pool(strings: &[String]) -> Vec<u8> {
        // Cabecera real del chunk "string pool": 28 bytes (tipo, headerSize,
        // size, stringCount, styleCount, flags, stringsStart, stylesStart).
        // El array de offsets va justo detrás (28..28+4n) y los datos de las
        // cadenas en `stringsStart`. Es el formato que emite aapt.
        let header_size = 28usize;
        let strings_start = header_size + strings.len() * 4;
        let mut body = Vec::new();
        let mut offsets = Vec::new();
        for s in strings {
            offsets.push(body.len() as u32);
            let units: Vec<u16> = s.encode_utf16().collect();
            body.extend_from_slice(&(units.len() as u16).to_le_bytes());
            for u in units {
                body.extend_from_slice(&u.to_le_bytes());
            }
            body.extend_from_slice(&0u16.to_le_bytes());
        }
        let mut out = Vec::new();
        out.extend_from_slice(&1u16.to_le_bytes()); // tipo: string pool
        out.extend_from_slice(&(header_size as u16).to_le_bytes());
        out.extend_from_slice(&((strings_start + body.len()) as u32).to_le_bytes());
        out.extend_from_slice(&(strings.len() as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // style count
        out.extend_from_slice(&0u32.to_le_bytes()); // flags (UTF-16)
        out.extend_from_slice(&(strings_start as u32).to_le_bytes()); // strings start
        out.extend_from_slice(&0u32.to_le_bytes()); // styles start
        for o in offsets {
            out.extend_from_slice(&o.to_le_bytes());
        }
        out.extend_from_slice(&body);
        out
    }

    /// Construye un chunk de "start element".
    fn build_start_element(ns: u32, name: u32, attrs: &[TestAttr]) -> Vec<u8> {
        let attr_size = 20usize;
        let chunk_size = 16 + 20 + attrs.len() * attr_size;
        let mut out = Vec::new();
        out.extend_from_slice(&0x0102u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(&(chunk_size as u32).to_le_bytes());
        out.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes()); // line number
        out.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes()); // comment
        out.extend_from_slice(&ns.to_le_bytes());
        out.extend_from_slice(&name.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // attributeStart
        out.extend_from_slice(&(attr_size as u16).to_le_bytes());
        out.extend_from_slice(&(attrs.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // idIndex
        out.extend_from_slice(&0u16.to_le_bytes()); // classIndex
        out.extend_from_slice(&0u16.to_le_bytes()); // styleIndex
        for a in attrs {
            out.extend_from_slice(&a.ns.to_le_bytes());
            out.extend_from_slice(&a.name.to_le_bytes());
            out.extend_from_slice(&a.raw.to_le_bytes());
            out.extend_from_slice(&8u16.to_le_bytes()); // tamaño del Res_value
            out.push(0); // res0
            out.push(a.vtype);
            out.extend_from_slice(&a.data.to_le_bytes());
        }
        out
    }

    /// Construye un chunk de "end element".
    fn build_end_element(ns: u32, name: u32) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&0x0103u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(&24u32.to_le_bytes());
        out.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());
        out.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());
        out.extend_from_slice(&ns.to_le_bytes());
        out.extend_from_slice(&name.to_le_bytes());
        out
    }

    /// Construye un AXML completo con los elementos indicados.
    ///
    /// La lista es plana, pero el AXML resultante sale **bien anidado** como el
    /// que emite aapt: `<application>` contiene a los componentes, cada
    /// componente contiene a sus `<intent-filter>` y cada filtro a sus
    /// `<action>`/`<category>`.
    fn build_axml(elements: &[(&str, &[(&str, TestVal)])]) -> Vec<u8> {
        let mut pool: Vec<String> = Vec::new();
        let ns_idx = intern(&mut pool, ANDROID_NS);
        let mut prepared: Vec<(u32, Vec<TestAttr>)> = Vec::new();
        for (name, attrs) in elements {
            let name_idx = intern(&mut pool, name);
            let mut list = Vec::new();
            for (attr_name, value) in attrs.iter() {
                let attr_idx = intern(&mut pool, attr_name);
                let (vtype, data, raw) = match value {
                    TestVal::S(s) => {
                        let i = intern(&mut pool, s);
                        (0x03u8, i, i)
                    }
                    TestVal::I(i) => (0x10u8, *i as u32, 0xFFFF_FFFF),
                    TestVal::B(b) => (0x12u8, if *b { 1 } else { 0 }, 0xFFFF_FFFF),
                };
                list.push(TestAttr {
                    ns: ns_idx,
                    name: attr_idx,
                    vtype,
                    data,
                    raw,
                });
            }
            prepared.push((name_idx, list));
        }

        /// ¿Puede `padre` contener a `hijo` en el esquema de AndroidManifest?
        fn puede_contener(padre: &str, hijo: &str) -> bool {
            let es_componente = |n: &str| {
                matches!(
                    n,
                    "activity" | "activity-alias" | "service" | "receiver" | "provider"
                )
            };
            match hijo {
                "intent-filter" => es_componente(padre),
                "action" | "category" => padre == "intent-filter",
                "activity" | "activity-alias" | "service" | "receiver" | "provider" => {
                    padre == "application" || es_componente(padre)
                }
                _ => padre == "manifest",
            }
        }

        let mut out = Vec::new();
        out.extend_from_slice(&AXML_MAGIC.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // tamaño (se parchea al final)
        out.extend_from_slice(&build_string_pool(&pool));
        let mut abiertos: Vec<u32> = Vec::new();
        for (name_idx, attrs) in prepared {
            let name = pool[name_idx as usize].clone();
            // Cierra los elementos abiertos que ya no pueden contener a este.
            while let Some(&top) = abiertos.last() {
                let top_name = pool[top as usize].clone();
                if puede_contener(&top_name, &name) {
                    break;
                }
                let ni = abiertos.pop().unwrap();
                out.extend_from_slice(&build_end_element(ns_idx, ni));
            }
            out.extend_from_slice(&build_start_element(ns_idx, name_idx, &attrs));
            abiertos.push(name_idx);
        }
        while let Some(ni) = abiertos.pop() {
            out.extend_from_slice(&build_end_element(ns_idx, ni));
        }
        let total = out.len() as u32;
        out[4..8].copy_from_slice(&total.to_le_bytes());
        out
    }

    /// APK sintética con DEX, `.so`, assets, resources.arsc y manifiesto.
    fn build_synthetic_apk(path: &std::path::Path) {
        use std::io::Write;
        let manifest = build_axml(&[
            ("manifest", &[("package", TestVal::S("com.example.sintetica"))]),
            (
                "uses-permission",
                &[("name", TestVal::S("android.permission.INTERNET"))],
            ),
        ]);
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("classes.dex", opts).unwrap();
        zip.write_all(b"dex-falso").unwrap();
        zip.start_file("classes2.dex", opts).unwrap();
        zip.write_all(b"dex-falso-2").unwrap();
        zip.start_file("lib/arm64-v8a/libdemo.so", opts).unwrap();
        zip.write_all(b"so-falsa").unwrap();
        zip.start_file("assets/demo.txt", opts).unwrap();
        zip.write_all(b"asset").unwrap();
        zip.start_file("resources.arsc", opts).unwrap();
        zip.write_all(b"arsc").unwrap();
        zip.start_file("AndroidManifest.xml", opts).unwrap();
        zip.write_all(&manifest).unwrap();
        zip.finish().unwrap();
    }

    // --- Tests ---

    #[test]
    fn el_sha256_coincide_con_el_vector_conocido() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn el_sha256_de_un_fichero_local_es_el_del_vector() {
        let path = test_path("abc.bin");
        std::fs::write(&path, b"abc").unwrap();
        let hash = sha256_file(path.to_str().unwrap()).expect("debe hashear");
        assert_eq!(
            hash,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(file_size(path.to_str().unwrap()), 3);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn el_parser_axml_extrae_manifiesto_permisos_y_componentes() {
        let buf = build_axml(&[
            (
                "manifest",
                &[
                    ("package", TestVal::S("com.example.app")),
                    ("versionCode", TestVal::I(42)),
                    ("versionName", TestVal::S("1.2.3")),
                ],
            ),
            (
                "uses-sdk",
                &[
                    ("minSdkVersion", TestVal::I(23)),
                    ("targetSdkVersion", TestVal::I(33)),
                ],
            ),
            (
                "uses-permission",
                &[("name", TestVal::S("android.permission.CAMERA"))],
            ),
            (
                "uses-permission",
                &[("name", TestVal::S("android.permission.INTERNET"))],
            ),
            ("application", &[]),
            (
                "activity",
                &[
                    ("name", TestVal::S("com.example.app.Main")),
                    ("exported", TestVal::B(true)),
                ],
            ),
            ("intent-filter", &[]),
            (
                "action",
                &[("name", TestVal::S("android.intent.action.MAIN"))],
            ),
            (
                "category",
                &[("name", TestVal::S("android.intent.category.LAUNCHER"))],
            ),
            (
                "service",
                &[
                    ("name", TestVal::S("com.example.app.Svc")),
                    ("exported", TestVal::B(false)),
                ],
            ),
            // El filtro del service va DESPUÉS del service: el constructor
            // anida cada intent-filter dentro del componente que le precede.
            ("intent-filter", &[]),
            ("receiver", &[("name", TestVal::S("com.example.app.Rcv"))]),
        ]);
        let m = manifest_axml(&buf);
        assert_eq!(m["package"], json!("com.example.app"));
        assert_eq!(m["version_code"], json!(42));
        assert_eq!(m["version_name"], json!("1.2.3"));
        assert_eq!(m["min_sdk"], json!(23));
        assert_eq!(m["target_sdk"], json!(33));
        let perms = m["permissions"].as_array().unwrap();
        assert_eq!(perms.len(), 2);
        assert_eq!(perms[0], json!("android.permission.CAMERA"));
        assert_eq!(perms[1], json!("android.permission.INTERNET"));
        let acts = m["activities"].as_array().unwrap();
        assert_eq!(acts.len(), 1);
        assert_eq!(acts[0]["name"], json!("com.example.app.Main"));
        assert_eq!(acts[0]["exported"], json!(true));
        let filters = acts[0]["intent_filters"].as_array().unwrap();
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0]["actions"][0], json!("android.intent.action.MAIN"));
        assert_eq!(filters[0]["categories"][0], json!("android.intent.category.LAUNCHER"));
        let svcs = m["services"].as_array().unwrap();
        assert_eq!(svcs[0]["name"], json!("com.example.app.Svc"));
        assert_eq!(svcs[0]["exported"], json!(false));
        assert_eq!(m["receivers"][0]["name"], json!("com.example.app.Rcv"));
        assert!(m["errors"].as_array().unwrap().is_empty());
    }

    #[test]
    fn un_axml_invalido_no_entra_en_panic() {
        let m = manifest_axml(&[0, 1, 2, 3]);
        assert!(!m["errors"].as_array().unwrap().is_empty());
        assert!(m["package"].is_null());
    }

    #[test]
    fn la_huella_cambia_si_cambia_el_manifiesto() {
        let a = json!({
            "package": "com.example.app",
            "version_code": 1,
            "version_name": "1.0",
            "permissions": ["android.permission.CAMERA", "android.permission.INTERNET"],
            "activities": [], "services": [], "receivers": [], "providers": [],
        });
        let mut b = a.clone();
        b["permissions"] = json!(["android.permission.INTERNET"]);
        assert_ne!(manifest_fingerprint(&a), manifest_fingerprint(&b));
        // El orden de los permisos NO debe alterar la huella.
        let mut c = a.clone();
        c["permissions"] = json!(["android.permission.INTERNET", "android.permission.CAMERA"]);
        assert_eq!(manifest_fingerprint(&a), manifest_fingerprint(&c));
    }

    #[test]
    fn el_drift_detecta_cambios_entre_dos_manifiestos() {
        let known = json!({
            "com.example.app": { "sha256": "aaa", "fingerprint": "fff", "version_code": 1 },
            "com.old.app": { "sha256": "bbb", "fingerprint": "ggg", "version_code": 3 },
        });
        let current = json!({
            "packages": [
                { "package": "com.example.app", "sha256": "aaa", "fingerprint": "hhh", "version_code": 2 },
                { "package": "com.new.app", "sha256": "ccc", "fingerprint": "iii", "version_code": 1 },
            ],
        });
        let d = drift_compare(&known, &current);
        let changed = d["changed"].as_array().unwrap();
        assert_eq!(changed.len(), 2);
        assert!(changed.iter().any(|c| c["kind"] == json!("manifest")));
        assert!(changed.iter().any(|c| c["kind"] == json!("version")));
        assert_eq!(d["added"], json!(["com.new.app"]));
        assert_eq!(d["removed"], json!(["com.old.app"]));
        assert_eq!(d["changed_count"], json!(2));
    }

    #[test]
    fn el_drift_detecta_cambio_de_binario() {
        let known = json!({
            "com.example.app": { "sha256": "aaa", "fingerprint": "fff", "version_code": 1 },
        });
        let current = json!({
            "packages": [
                { "package": "com.example.app", "sha256": "zzz", "fingerprint": "fff", "version_code": 1 },
            ],
        });
        let d = drift_compare(&known, &current);
        let changed = d["changed"].as_array().unwrap();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0]["kind"], json!("binary"));
        assert_eq!(changed[0]["from"], json!("aaa"));
        assert_eq!(changed[0]["to"], json!("zzz"));
        assert!(d["added"].as_array().unwrap().is_empty());
        assert!(d["removed"].as_array().unwrap().is_empty());
    }

    #[test]
    fn el_analisis_de_una_apk_sintetica_ve_dex_manifesto_y_so() {
        let path = test_path("sintetica.apk");
        build_synthetic_apk(&path);
        let a = analyze_apk(path.to_str().unwrap());
        assert_eq!(a["ok"], json!(true));
        assert_eq!(a["dex_count"], json!(2));
        assert_eq!(a["native_libs"], json!(["lib/arm64-v8a/libdemo.so"]));
        assert_eq!(a["abis"], json!(["arm64-v8a"]));
        assert_eq!(a["has_resources_arsc"], json!(true));
        assert_eq!(a["manifest"]["package"], json!("com.example.sintetica"));
        assert_eq!(
            a["manifest"]["permissions"],
            json!(["android.permission.INTERNET"])
        );
        assert!(!a["fingerprint"].as_str().unwrap_or("").is_empty());
        assert_eq!(a["sha256"], json!(sha256_file(path.to_str().unwrap())));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn la_salida_de_pm_list_packages_f_se_interpreta_bien() {
        // El formato real es "package:/ruta/app.apk" (sin '=').
        let salida = "package:/data/app/~~a==/com.foo-1/base.apk\npackage:/system/app/Bar/Bar.apk\n";
        let rutas = parse_apk_paths(salida);
        assert_eq!(rutas.len(), 2);
        assert_eq!(rutas[0], "/data/app/~~a==/com.foo-1/base.apk");
        assert_eq!(
            package_from_path(&rutas[0]).as_deref(),
            Some("com.foo")
        );
        // El layout antiguo no permite deducirlo: se empareja por índice.
        assert_eq!(package_from_path(&rutas[1]), None);
        let nombres = vec!["com.foo".to_string(), "com.bar".to_string()];
        let emparejado = pair_packages(&nombres, &rutas);
        assert_eq!(emparejado[0].0, "com.foo");
        assert_eq!(emparejado[1].0, "com.bar");
    }

    #[test]
    fn una_ruta_relativa_se_rechaza() {
        let r = analyze_apk("app.apk");
        assert_eq!(r["ok"], json!(false));
        assert_eq!(r["error"], json!("bad_path"));
        let d = dump_all("relative/dir");
        assert_eq!(d["ok"], json!(false));
        assert_eq!(d["error"], json!("bad_path"));
    }

    #[test]
    fn el_base64_propio_descodifica() {
        assert_eq!(decode_b64("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(decode_b64("").unwrap().len(), 0);
        assert_eq!(decode_b64("aGVsbG8g\naGk=").unwrap(), b"hello hi");
    }
}
