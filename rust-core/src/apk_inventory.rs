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

use crate::util_text::hex;
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
        self.ensure_cache().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "dispositivo sin datos")
        })?;
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
    /// Permiso que hay que tener para poder invocarlo (`android:permission`).
    ///
    /// Si el componente está `exported` y esto es `None`, cualquiera de otra
    /// app puede llamarlo: ése es el hallazgo de seguridad más típico.
    permission: Option<String>,
    /// Permiso de **lectura** de un `ContentProvider`.
    read_permission: Option<String>,
    /// Permiso de **escritura** de un `ContentProvider`.
    write_permission: Option<String>,
    /// `android:authorities` del `ContentProvider`.
    authorities: Vec<String>,
    /// `android:grantUriPermissions` del `ContentProvider`.
    grant_uri_permissions: Option<bool>,
}

/// Atributos relevantes de `<application>`.
#[derive(Clone, Debug, Default)]
struct ApplicationInfo {
    /// `android:debuggable`: la app acepta depuración desde el PC.
    debuggable: Option<bool>,
    /// `android:allowBackup`: se puede copiar su `/data` con `adb backup`.
    allow_backup: Option<bool>,
    /// `android:usesCleartextTraffic`: permite HTTP sin cifrar.
    uses_cleartext_traffic: Option<bool>,
    /// `android:testOnly`: la app se instaló como paquete de test.
    test_only: Option<bool>,
    /// `android:networkSecurityConfig`: política de red propia.
    network_security_config: Option<String>,
    /// `android:hasCode`: la app tiene DEX propio.
    has_code: Option<bool>,
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
    /// Permisos **declarados** por la app (`<permission>`): nombre + nivel de
    /// protección declarado (`normal`, `dangerous`, `signature`…).
    declared_permissions: Vec<(String, Option<String>)>,
    /// `<meta-data>` del manifiesto (clave, valor).
    meta_data: Vec<(String, String)>,
    /// Atributos de `<application>`.
    application: ApplicationInfo,
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
            (
                (((first & 0x7f) as usize) << 8) | buf[p + 1] as usize,
                p + 2,
            )
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
            ((((first & 0x7fff) as usize) << 16) | second as usize, p + 4)
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
                        m.version_name =
                            find_attr(&attrs, "versionName").and_then(|v| v.as_string());
                    }
                    "uses-sdk" => {
                        m.min_sdk = find_attr(&attrs, "minSdkVersion").and_then(|v| v.as_int());
                        m.target_sdk =
                            find_attr(&attrs, "targetSdkVersion").and_then(|v| v.as_int());
                    }
                    "uses-permission" | "uses-permission-sdk-23" | "uses-permission-sdk-m" => {
                        if let Some(p) = find_attr(&attrs, "name").and_then(|v| v.as_string()) {
                            if !p.is_empty() {
                                m.permissions.push(p);
                            }
                        }
                    }
                    "permission" | "permission-tree" | "permission-group" => {
                        // Permiso **declarado** por la app (no el que pide).
                        if let Some(p) = find_attr(&attrs, "name").and_then(|v| v.as_string()) {
                            if !p.is_empty() {
                                let level = find_attr(&attrs, "protectionLevel")
                                    .and_then(|v| v.as_string());
                                m.declared_permissions.push((p, level));
                            }
                        }
                    }
                    "meta-data" => {
                        let n = find_attr(&attrs, "name").and_then(|v| v.as_string());
                        if let Some(n) = n {
                            if !n.is_empty() {
                                let v = find_attr(&attrs, "value")
                                    .and_then(|v| v.as_string())
                                    .unwrap_or_default();
                                m.meta_data.push((n, v));
                            }
                        }
                    }
                    "application" => {
                        m.application.debuggable =
                            find_attr(&attrs, "debuggable").and_then(|v| v.as_bool());
                        m.application.allow_backup =
                            find_attr(&attrs, "allowBackup").and_then(|v| v.as_bool());
                        m.application.uses_cleartext_traffic =
                            find_attr(&attrs, "usesCleartextTraffic").and_then(|v| v.as_bool());
                        m.application.test_only =
                            find_attr(&attrs, "testOnly").and_then(|v| v.as_bool());
                        m.application.network_security_config =
                            find_attr(&attrs, "networkSecurityConfig").and_then(|v| v.as_string());
                        m.application.has_code =
                            find_attr(&attrs, "hasCode").and_then(|v| v.as_bool());
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
                            permission: find_attr(&attrs, "permission").and_then(|v| v.as_string()),
                            read_permission: find_attr(&attrs, "readPermission")
                                .and_then(|v| v.as_string()),
                            write_permission: find_attr(&attrs, "writePermission")
                                .and_then(|v| v.as_string()),
                            authorities: find_attr(&attrs, "authorities")
                                .and_then(|v| v.as_string())
                                .map(|a| {
                                    a.split(';')
                                        .map(|x| x.trim().to_string())
                                        .filter(|x| !x.is_empty())
                                        .collect()
                                })
                                .unwrap_or_default(),
                            grant_uri_permissions: find_attr(&attrs, "grantUriPermissions")
                                .and_then(|v| v.as_bool()),
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
        "permission": c.permission,
        "read_permission": c.read_permission,
        "write_permission": c.write_permission,
        "authorities": c.authorities,
        "grant_uri_permissions": c.grant_uri_permissions,
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
    let declared: Vec<Value> = m
        .declared_permissions
        .iter()
        .map(|(name, level)| json!({ "name": name, "protection_level": level }))
        .collect();
    let meta: Vec<Value> = m
        .meta_data
        .iter()
        .map(|(name, value)| json!({ "name": name, "value": value }))
        .collect();
    json!({
        "package": m.package,
        "version_code": m.version_code,
        "version_name": m.version_name,
        "min_sdk": m.min_sdk,
        "target_sdk": m.target_sdk,
        "permissions": m.permissions,
        "declared_permissions": declared,
        "meta_data": meta,
        "application": {
            "debuggable": m.application.debuggable,
            "allow_backup": m.application.allow_backup,
            "uses_cleartext_traffic": m.application.uses_cleartext_traffic,
            "test_only": m.application.test_only,
            "network_security_config": m.application.network_security_config,
            "has_code": m.application.has_code,
        },
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
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
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
    let with_paths =
        crate::android::root_exec("pm list packages -f 2>/dev/null", PM_TIMEOUT_MS, 512 * 1024);
    let paths = parse_apk_paths(&with_paths.stdout);
    let names = if paths.len() > 1 {
        let plain =
            crate::android::root_exec("pm list packages 2>/dev/null", PM_TIMEOUT_MS, 512 * 1024);
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
                code = v
                    .split_whitespace()
                    .next()
                    .and_then(|x| x.parse::<i64>().ok());
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
        return items.iter().map(|(p, a)| package_record(p, a)).collect();
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
///
/// Desde v13.5 es un **escáner**: además del listado, devuelve el fingerprint
/// de ofuscación, los empaquetadores y frameworks de parche detectados, los
/// permisos agrupados por riesgo, los endpoints (URL/dominio/IP), el informe
/// de las `.so` nativas, la firma (v1/v2/v3 + firmante + SHA-256 del
/// certificado) y los componentes exportados sin permiso.
///
/// La **firma de la función no cambia** (`analyze_apk(path) -> Value`): todo
/// lo nuevo son claves **adicionales** del mismo objeto.
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
    // Barrido profundo (DEX, assets, nativas, firma, endpoints).
    let deep = deep_scan(path, &analysis.entries, &analysis.manifest);
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
    let mut out = json!({
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
    });
    // Las claves del barrido profundo se **añaden** (nunca sustituyen).
    if let (Some(dst), Some(src)) = (out.as_object_mut(), deep.as_object()) {
        for (k, v) in src {
            dst.insert(k.clone(), v.clone());
        }
    }
    out
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
// ESCÁNER DE APK: ofuscación, empaquetadores, permisos, endpoints, nativas
// ========================================================================= //
//
// Este bloque convierte `analyze_apk` de un "listado de entradas" en un
// escáner de verdad.  Todo se calcula **en Rust** con lo que ya hay en el
// crate: `zip` (lectura del APK), `flate2` (descompresión), `sha2` (hashes) y
// `serde_json` (salida).  El análisis del DEX y el framework de parcheo se
// **consumen** desde las APIs públicas de `crate::dex_decompile` y
// `crate::app_intel` (no se reimplementan aquí).
//
// Lo que se mide:
//
// | Bloque | Qué mide |
// |---|---|
// | `obfuscation` | nombres cortos, strings cifrados, `SourceFile`/`LineNumberTable`, packs |
// | `packers` | DexGuard, R8, Allatori, ZKM, Stringer, Bangcle, Jiagu, Legu, 360, Ijiami |
// | `patch_frameworks` | LSPatch, KnoxPatch, EdXposed, MonoPatch, TrustMe, LSPosed, Frida |
// | `permissions_risk` | permisos agrupados por clase de riesgo |
// | `endpoints` | URLs, dominios e IPs del DEX y de los assets |
// | `native_libraries` | ABI, tamaño, exports y si la `.so` está stripped |
// | `signature` | v1/v2/v3, firmante y SHA-256 del certificado |
// | `exported_without_permission` | componentes exportados sin permiso |

/// Bytes máximos que se leen de un DEX.
const MAX_DEX_BYTES: usize = 24 * 1024 * 1024;

/// DEX analizados por APK (el resto se cuenta pero no se lee).
const MAX_DEX_ANALYZED: usize = 3;

/// Bytes máximos leídos de un asset (para buscar URLs y packs).
const MAX_ASSET_BYTES: usize = 256 * 1024;

/// Bytes máximos leídos de una `.so` (cabecera + secciones).
const MAX_SO_BYTES: usize = 12 * 1024 * 1024;

/// `.so` analizadas por APK (el resto se lista sin abrir).
const MAX_SO_ANALYZED: usize = 8;

/// Assets leídos por APK (para buscar endpoints y packs).
const MAX_ASSETS_READ: usize = 24;

/// Endpoints reportados por APK.
const MAX_ENDPOINTS: usize = 150;

/// Apps que `scan_installed` analiza cuando `limit` es 0.
///
/// Tope de seguridad: el móvil va cargado (load 17,6) y cada APK lee su
/// manifiesto del dispositivo.  `limit` explícito manda sobre este tope.
const MAX_SCAN_APPS: usize = 60;

/// Bytes del final del APK que se leen para buscar el bloque de firma.
const SIG_TAIL_BYTES: usize = 128 * 1024;

/// Permisos **peligrosos**: conceden acceso a datos personales.
const DANGEROUS_PERMISSIONS: &[&str] = &[
    "ACCESS_BACKGROUND_LOCATION",
    "ACCESS_COARSE_LOCATION",
    "ACCESS_FINE_LOCATION",
    "ACCESS_MEDIA_LOCATION",
    "ACTIVITY_RECOGNITION",
    "ADD_VOICEMAIL",
    "ANSWER_PHONE_CALLS",
    "BLUETOOTH_ADVERTISE",
    "BLUETOOTH_CONNECT",
    "BLUETOOTH_SCAN",
    "BODY_SENSORS",
    "BODY_SENSORS_BACKGROUND",
    "CALL_PHONE",
    "CAMERA",
    "GET_ACCOUNTS",
    "NEARBY_WIFI_DEVICES",
    "POST_NOTIFICATIONS",
    "PROCESS_OUTGOING_CALLS",
    "READ_CALENDAR",
    "READ_CALL_LOG",
    "READ_CONTACTS",
    "READ_EXTERNAL_STORAGE",
    "READ_MEDIA_AUDIO",
    "READ_MEDIA_IMAGES",
    "READ_MEDIA_VIDEO",
    "READ_PHONE_NUMBERS",
    "READ_PHONE_STATE",
    "READ_SMS",
    "RECEIVE_MMS",
    "RECEIVE_SMS",
    "RECEIVE_WAP_PUSH",
    "RECORD_AUDIO",
    "SEND_SMS",
    "UWB_RANGING",
    "USE_SIP",
    "WRITE_CALENDAR",
    "WRITE_CALL_LOG",
    "WRITE_CONTACTS",
    "WRITE_EXTERNAL_STORAGE",
];

/// Permisos de riesgo **alto**: dan poder sobre el sistema.
const HIGH_PERMISSIONS: &[&str] = &[
    "ACCESSIBILITY",
    "BIND_ACCESSIBILITY_SERVICE",
    "BIND_DEVICE_ADMIN",
    "BIND_INPUT_METHOD",
    "BIND_NOTIFICATION_LISTENER_SERVICE",
    "BIND_VPN_SERVICE",
    "CAPTURE_AUDIO_OUTPUT",
    "CAPTURE_SECURE_VIDEO_OUTPUT",
    "DELETE_PACKAGES",
    "INSTALL_PACKAGES",
    "MANAGE_EXTERNAL_STORAGE",
    "MODIFY_PHONE_STATE",
    "PACKAGE_USAGE_STATS",
    "QUERY_ALL_PACKAGES",
    "REQUEST_DELETE_PACKAGES",
    "REQUEST_INSTALL_PACKAGES",
    "SYSTEM_ALERT_WINDOW",
    "WRITE_SECURE_SETTINGS",
    "WRITE_SETTINGS",
];

/// Permisos de riesgo **medio**: red, arranque o primer plano.
const MEDIUM_PERMISSIONS: &[&str] = &[
    "ACCESS_NETWORK_STATE",
    "ACCESS_WIFI_STATE",
    "BLUETOOTH",
    "CHANGE_WIFI_STATE",
    "DISABLE_KEYGUARD",
    "FOREGROUND_SERVICE",
    "FOREGROUND_SERVICE_DATA_SYNC",
    "FOREGROUND_SERVICE_MEDIA_PLAYBACK",
    "GET_PACKAGE_SIZE",
    "INTERNET",
    "RECEIVE_BOOT_COMPLETED",
    "SCHEDULE_EXACT_ALARM",
    "USE_BIOMETRIC",
    "USE_EXACT_ALARM",
    "USE_FINGERPRINT",
    "VIBRATE",
    "WAKE_LOCK",
];

/// Empaquetadores y protectores: `(id, nombre, marcador)`.
///
/// Heurística por **marcador literal** (nombre de clase, de `.so` o de
/// asset).  Un marcador no prueba el packer: prueba que el packer o su
/// cargador está dentro del APK.
const PACKER_MARKERS: &[(&str, &str, &str)] = &[
    ("dexguard", "DexGuard (GuardSquare)", "dexguard"),
    ("dexguard", "DexGuard (GuardSquare)", "com.guardsquare"),
    ("allatori", "Allatori", "allatori"),
    ("zkm", "ZKM / Zelix KlassMaster", "zelix"),
    ("zkm", "ZKM / Zelix KlassMaster", "stringencryption"),
    ("stringer", "Stringer (DexProtector)", "stringer"),
    ("bangcle", "Bangcle / SecNeo", "bangcle"),
    ("bangcle", "Bangcle / SecNeo", "secshell"),
    ("secneo", "SecNeo (helper)", "libdexhelper"),
    ("jiagu", "Qihoo 360 Jiagu", "libjiagu"),
    ("jiagu", "Qihoo 360 Jiagu", "360sec"),
    ("legu", "Tencent Legu", "libshella"),
    ("legu", "Tencent Legu", "libshellx"),
    ("baidu", "Baidu protect", "libbaiduprotect"),
    ("ijiami", "Ijiami", "ijiami"),
    ("ijiami", "Ijiami", "libexecmain"),
    ("tencent", "Tencent terSafe", "libtersafe"),
    ("alibaba", "Ali Ju'an Bao", "libmobisec"),
    ("alibaba", "Ali Ju'an Bao", "libpreverify1"),
    ("nagain", "Nagain", "libnagain"),
    ("r8", "R8 (Android)", "com.android.tools.r8"),
];

/// Frameworks de parche y de hook: `(id, nombre, marcador)`.
const PATCH_FRAMEWORK_MARKERS: &[(&str, &str, &str)] = &[
    ("lspatch", "LSPatch", "lspatch"),
    ("knoxpatch", "KnoxPatch", "knoxpatch"),
    ("edxposed", "EdXposed", "edxposed"),
    ("monopatch", "MonoPatch", "monopatch"),
    ("trustme", "TrustMe", "trustme"),
    ("lsposed", "LSPosed", "lsposed"),
    ("xposed", "Xposed", "com.xposed"),
    ("xposed", "Xposed", "xposed_init"),
    ("frida-gadget", "Frida Gadget", "frida-gadget"),
    ("frida-gadget", "Frida Gadget", "libfrida-gadget.so"),
    ("frida-server", "Frida Server", "frida-server"),
    ("sandhook", "SandHook", "sandhook"),
    ("whale", "Whale", "whale"),
    ("yahfa", "YAHFA", "yahfa"),
    ("epic", "Epic", "com.epic"),
    ("pine", "Pine", "pine"),
    ("substrate", "Cydia Substrate", "com.saurik.substrate"),
    (
        "lucky-patcher",
        "Lucky Patcher (repaquetado)",
        "luckypatcher",
    ),
    (
        "lucky-patcher",
        "Lucky Patcher (repaquetado)",
        "com.chelpus",
    ),
];

/// TLD aceptados al reconocer un dominio suelto (heurística).
const KNOWN_TLDS: &[&str] = &[
    "com", "net", "org", "io", "co", "es", "app", "dev", "me", "eu", "uk", "de", "fr", "it", "cn",
    "ru", "info", "biz", "xyz", "top", "online", "site", "tech", "cloud", "ai", "gov", "edu",
    "mil", "int", "cat", "mobi", "tv", "cc", "ly", "sh", "to", "so", "gg", "us", "ca", "br", "mx",
    "ar", "cl", "pt", "pl", "ua", "tr", "jp", "kr", "tw", "hk", "sg", "my", "id", "th", "vn", "ph",
    "za", "nz", "au", "ie", "at", "be", "ch", "cz", "dk", "fi", "no", "se", "il", "in", "gr", "hu",
    "ro", "sk", "si", "hr", "bg", "ee", "lv", "lt", "is", "lu", "mt", "cy", "ee", "nu", "be", "st",
    "fm", "am", "im", "je", "gg", "tc", "ws", "bz", "cr", "cu", "do", "ec", "gt", "py", "uy", "ve",
    "bo", "pe", "eg", "sa", "ae", "qa", "kz", "uz", "by",
];

// ========================================================================= //
// Permisos por riesgo
// ========================================================================= //

/// Clase de riesgo de un permiso Android.
///
/// * `peligroso` — permiso de runtime que da acceso a datos personales.
/// * `alto` — da poder sobre el sistema (overlays, instalación, accesibilidad).
/// * `medio` — red, arranque o primer plano.
/// * `bajo` — el resto.
pub fn permission_risk_class(perm: &str) -> &'static str {
    let short = perm.rsplit('.').next().unwrap_or(perm);
    if DANGEROUS_PERMISSIONS.contains(&short) {
        "peligroso"
    } else if HIGH_PERMISSIONS.contains(&short) {
        "alto"
    } else if MEDIUM_PERMISSIONS.contains(&short) {
        "medio"
    } else {
        "bajo"
    }
}

/// Agrupa los permisos de un manifiesto por clase de riesgo.
pub fn permission_risk_groups(manifest: &Value) -> Value {
    let perms: Vec<String> = manifest["permissions"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let mut peligroso: Vec<String> = Vec::new();
    let mut alto: Vec<String> = Vec::new();
    let mut medio: Vec<String> = Vec::new();
    let mut bajo: Vec<String> = Vec::new();
    for p in &perms {
        match permission_risk_class(p) {
            "peligroso" => peligroso.push(p.clone()),
            "alto" => alto.push(p.clone()),
            "medio" => medio.push(p.clone()),
            _ => bajo.push(p.clone()),
        }
    }
    json!({
        "total": perms.len(),
        "counts": {
            "peligroso": peligroso.len(),
            "alto": alto.len(),
            "medio": medio.len(),
            "bajo": bajo.len(),
        },
        "by_risk": {
            "peligroso": peligroso,
            "alto": alto,
            "medio": medio,
            "bajo": bajo,
        },
        "score": perms.len() as i64 * 0, // se rellena fuera: placeholder de forma estable
    })
}

// ========================================================================= //
// Lectura de entradas concretas del APK
// ========================================================================= //

/// Lee **una** entrada del APK sin listar el archivo entero.
///
/// Se usa el crate `zip` sobre el `DeviceFile` (local o remoto), así que no
/// se reimplementa nada del formato ZIP.
fn read_apk_entry(path: &str, name: &str, max_bytes: usize) -> Option<Vec<u8>> {
    let file = DeviceFile::open(path)?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let mut entry = archive.by_name(name).ok()?;
    let mut buf = Vec::new();
    let mut limited = entry.by_ref().take(max_bytes as u64);
    limited.read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// Lee un rango de bytes de un fichero del dispositivo.
fn read_file_range(path: &str, offset: u64, len: usize) -> Option<Vec<u8>> {
    let mut f = DeviceFile::open(path)?;
    if offset >= f.size() || len == 0 {
        return None;
    }
    f.seek(SeekFrom::Start(offset)).ok()?;
    let want = len.min((f.size() - offset) as usize);
    let mut buf = vec![0u8; want];
    let n = f.read(&mut buf).ok()?;
    buf.truncate(n);
    Some(buf)
}

/// Nombres de los DEX de un APK (en el orden habitual).
fn dex_entry_names() -> Vec<String> {
    let mut v = vec!["classes.dex".to_string()];
    for i in 2..=MAX_DEX_ANALYZED + 2 {
        v.push(format!("classes{}.dex", i));
    }
    v
}

// ========================================================================= //
// Endpoints: URLs, dominios e IPs (heurística, sin regex)
// ========================================================================= //

/// Host de una URL (`https://user@host:443/x` → `host`).
fn url_host(url: &str) -> Option<String> {
    let rest = match url.find("://") {
        Some(i) => &url[i + 3..],
        None => url,
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.rsplit('@').next().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    if host.is_empty() {
        None
    } else {
        Some(host.to_ascii_lowercase())
    }
}

/// Dominios sueltos que aparecen en el texto (heurística por TLD).
fn scan_domains(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let bytes = text.as_bytes();
    let mut start = 0usize;
    let mut i = 0usize;
    while i <= bytes.len() {
        let c = if i < bytes.len() { bytes[i] } else { b' ' };
        let is_domain_char = c.is_ascii_alphanumeric() || c == b'.' || c == b'-';
        if !is_domain_char {
            if i > start {
                let token = &text[start..i];
                let token = token.trim_matches(|c: char| c == '.' || c == '-');
                if let Some(d) = normalize_domain(token) {
                    if !out.contains(&d) {
                        out.push(d);
                    }
                }
            }
            start = i + 1;
        }
        i += 1;
    }
    out
}

/// Valida y normaliza un token como nombre de dominio.
fn normalize_domain(token: &str) -> Option<String> {
    if token.len() < 5 || token.len() > 253 {
        return None;
    }
    if !token.contains('.') {
        return None;
    }
    let labels: Vec<&str> = token.split('.').collect();
    if labels.len() < 2 {
        return None;
    }
    for l in &labels {
        if l.is_empty() || l.len() > 63 {
            return None;
        }
        if !l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return None;
        }
    }
    // La última etiqueta tiene que parecer un TLD: letras, 2..=24 caracteres.
    let tld = labels[labels.len() - 1];
    if tld.len() < 2 || tld.len() > 24 || !tld.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    if !KNOWN_TLDS.contains(&tld.to_ascii_lowercase().as_str()) {
        return None;
    }
    Some(token.to_ascii_lowercase())
}

/// Direcciones IPv4 literales que aparecen en el texto.
fn scan_ipv4(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            let mut j = i;
            let mut groups: Vec<u8> = Vec::new();
            let mut ok = true;
            while j < bytes.len() && groups.len() < 4 {
                let mut val: u32 = 0;
                let mut digits = 0usize;
                while j < bytes.len() && bytes[j].is_ascii_digit() && digits < 3 {
                    val = val * 10 + (bytes[j] - b'0') as u32;
                    digits += 1;
                    j += 1;
                }
                if digits == 0 || val > 255 {
                    ok = false;
                    break;
                }
                groups.push(val as u8);
                if j < bytes.len() && bytes[j] == b'.' {
                    j += 1;
                } else {
                    break;
                }
            }
            if ok && groups.len() == 4 {
                // No puede ser parte de un número más largo (12.34.56.789).
                let prev_ok = start == 0 || !bytes[start - 1].is_ascii_digit();
                let next_ok = j >= bytes.len() || !bytes[j].is_ascii_digit();
                if prev_ok && next_ok {
                    let ip = groups
                        .iter()
                        .map(|g| g.to_string())
                        .collect::<Vec<String>>()
                        .join(".");
                    if !out.contains(&ip) {
                        out.push(ip);
                    }
                }
            }
            i = j.max(start + 1);
        } else {
            i += 1;
        }
    }
    out
}

/// Extrae URLs, dominios e IPs de un texto.
///
/// Las URLs salen de `crate::app_intel::extract_endpoints` (API pública ya
/// existente); los dominios y las IPs los saca esta heurística.
fn endpoints_from_text(text: &str) -> Value {
    let mut urls = crate::app_intel::extract_endpoints(text);
    if urls.len() > MAX_ENDPOINTS {
        urls.truncate(MAX_ENDPOINTS);
    }
    let mut domains: Vec<String> = Vec::new();
    for u in &urls {
        if let Some(h) = url_host(u) {
            if !domains.contains(&h) {
                domains.push(h);
            }
        }
    }
    for d in scan_domains(text) {
        if !domains.contains(&d) {
            domains.push(d);
        }
    }
    let ips = scan_ipv4(text);
    json!({
        "urls": urls,
        "domains": domains,
        "ips": ips,
        "counts": {
            "urls": urls.len(),
            "domains": domains.len(),
            "ips": ips.len(),
        },
    })
}

// ========================================================================= //
// ELF: ¿está stripped? ¿cuántos exports?
// ========================================================================= //

/// Datos mínimos de un ELF (lo justo para el informe de una `.so`).
struct ElfInfo {
    class: &'static str,
    machine: &'static str,
    stripped: bool,
    exports: usize,
    has_debug: bool,
    sections: usize,
}

fn rd_u16_le(b: &[u8], o: usize) -> u16 {
    if o + 2 > b.len() {
        return 0;
    }
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn rd_u32_le(b: &[u8], o: usize) -> u32 {
    if o + 4 > b.len() {
        return 0;
    }
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn rd_u64_le(b: &[u8], o: usize) -> u64 {
    if o + 8 > b.len() {
        return 0;
    }
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[o..o + 8]);
    u64::from_le_bytes(v)
}

/// Lee una cadena terminada en NUL.
fn cstr(b: &[u8], off: usize) -> String {
    if off >= b.len() {
        return String::new();
    }
    let end = b[off..]
        .iter()
        .position(|c| *c == 0)
        .unwrap_or(b.len() - off);
    String::from_utf8_lossy(&b[off..off + end]).into_owned()
}

/// Analiza una `.so`: secciones, símbolos y exports.
fn parse_elf(bytes: &[u8]) -> Option<ElfInfo> {
    if bytes.len() < 64 || &bytes[0..4] != b"\x7fELF" {
        return None;
    }
    let is64 = bytes[4] == 2;
    let class = if is64 { "ELF64" } else { "ELF32" };
    if bytes[5] != 1 {
        // Big-endian: no se analiza (los `.so` de Android son little-endian).
        return None;
    }
    let machine = match rd_u16_le(bytes, 18) {
        3 => "i386",
        40 => "ARM",
        62 => "x86_64",
        183 => "AArch64",
        _ => "otro",
    };
    let (shoff, shentsize, shnum, shstrndx) = if is64 {
        (
            rd_u64_le(bytes, 0x28) as usize,
            rd_u16_le(bytes, 0x3A) as usize,
            rd_u16_le(bytes, 0x3C) as usize,
            rd_u16_le(bytes, 0x3E) as usize,
        )
    } else {
        (
            rd_u32_le(bytes, 0x20) as usize,
            rd_u16_le(bytes, 0x2E) as usize,
            rd_u16_le(bytes, 0x30) as usize,
            rd_u16_le(bytes, 0x32) as usize,
        )
    };
    if shoff == 0 || shnum == 0 || shentsize == 0 {
        return Some(ElfInfo {
            class,
            machine,
            stripped: true,
            exports: 0,
            has_debug: false,
            sections: 0,
        });
    }
    // Tabla de nombres de sección.
    let strtab_off = if is64 {
        rd_u64_le(bytes, shoff + shstrndx * shentsize + 0x18) as usize
    } else {
        rd_u32_le(bytes, shoff + shstrndx * shentsize + 0x10) as usize
    };
    let strtab_size = if is64 {
        rd_u64_le(bytes, shoff + shstrndx * shentsize + 0x20) as usize
    } else {
        rd_u32_le(bytes, shoff + shstrndx * shentsize + 0x14) as usize
    };
    let strtab_end = (strtab_off + strtab_size).min(bytes.len());

    let mut stripped = true;
    let mut exports = 0usize;
    let mut has_debug = false;
    for i in 0..shnum {
        let base = shoff + i * shentsize;
        if base + shentsize > bytes.len() {
            break;
        }
        let name_off = rd_u32_le(bytes, base) as usize;
        let name = if strtab_off <= strtab_end && name_off < strtab_size {
            cstr(bytes, strtab_off + name_off)
        } else {
            String::new()
        };
        let sh_type = rd_u32_le(bytes, base + 4);
        let sh_size = if is64 {
            rd_u64_le(bytes, base + 0x20) as usize
        } else {
            rd_u32_le(bytes, base + 0x14) as usize
        };
        let sh_entsize = if is64 {
            rd_u64_le(bytes, base + 0x38) as usize
        } else {
            rd_u32_le(bytes, base + 0x24) as usize
        };
        match name.as_str() {
            ".symtab" => stripped = false,
            ".dynsym" => {
                if sh_entsize > 0 {
                    exports = sh_size / sh_entsize;
                }
            }
            ".debug_info" | ".debug_line" | ".zdebug_info" => has_debug = true,
            _ => {}
        }
        // SHT_DYNSYM = 11.
        if name.is_empty() && sh_type == 11 && sh_entsize > 0 {
            exports = sh_size / sh_entsize;
        }
    }
    Some(ElfInfo {
        class,
        machine,
        stripped,
        exports,
        has_debug,
        sections: shnum,
    })
}

// ========================================================================= //
// DER / X.509: firmante y SHA-256 del certificado
// ========================================================================= //

/// Lee un TLV DER: `(tag, inicio del contenido, longitud, offset siguiente)`.
fn der_tlv(b: &[u8], off: usize) -> Option<(u8, usize, usize, usize)> {
    if off + 2 > b.len() {
        return None;
    }
    let tag = b[off];
    let first = b[off + 1] as usize;
    let (len, hdr) = if first & 0x80 == 0 {
        (first, 2usize)
    } else {
        let n = first & 0x7f;
        if n == 0 || n > 4 || off + 2 + n > b.len() {
            return None;
        }
        let mut v = 0usize;
        for i in 0..n {
            v = (v << 8) | b[off + 2 + i] as usize;
        }
        (v, 2 + n)
    };
    let end = off.checked_add(hdr)?.checked_add(len)?;
    if end > b.len() {
        return None;
    }
    Some((tag, off + hdr, len, end))
}

/// OID DER → texto con puntos.
fn der_oid(b: &[u8], off: usize, len: usize) -> String {
    if len == 0 || off + len > b.len() {
        return String::new();
    }
    let mut parts = vec![(b[off] / 40).to_string(), (b[off] % 40).to_string()];
    let mut v = 0u64;
    for i in 1..len {
        let byte = b[off + i];
        v = (v << 7) | (byte & 0x7f) as u64;
        if byte & 0x80 == 0 {
            parts.push(v.to_string());
            v = 0;
        }
    }
    parts.join(".")
}

/// Nombre corto de un OID conocido de X.509.
fn oid_name(oid: &str) -> &'static str {
    match oid {
        "2.5.4.3" => "CN",
        "2.5.4.4" => "SN",
        "2.5.4.5" => "serialNumber",
        "2.5.4.6" => "C",
        "2.5.4.7" => "L",
        "2.5.4.8" => "ST",
        "2.5.4.9" => "STREET",
        "2.5.4.10" => "O",
        "2.5.4.11" => "OU",
        "1.2.840.113549.1.9.1" => "emailAddress",
        _ => "OID",
    }
}

/// Valor de un atributo X.509 → texto.
fn der_value(b: &[u8], off: usize, len: usize, tag: u8) -> String {
    let end = (off + len).min(b.len());
    match tag {
        0x1e => {
            // BMPString (UTF-16 BE).
            let mut units: Vec<u16> = Vec::new();
            let mut i = off;
            while i + 1 < end {
                units.push(u16::from_be_bytes([b[i], b[i + 1]]));
                i += 2;
            }
            String::from_utf16_lossy(&units)
        }
        0x0c | 0x13 | 0x16 | 0x14 | 0x1a => {
            String::from_utf8_lossy(&b[off..end]).trim().to_string()
        }
        _ => hex(&b[off..end]),
    }
}

/// Extrae el DN (issuer o subject) de un `Name` DER.
fn der_name(b: &[u8], off: usize) -> Option<String> {
    let (tag, content, len, _) = der_tlv(b, off)?;
    if tag != 0x30 {
        return None;
    }
    let end = content + len;
    let mut p = content;
    let mut parts: Vec<String> = Vec::new();
    while p < end {
        let (t2, c2, l2, n2) = der_tlv(b, p)?;
        p = n2;
        if t2 != 0x31 {
            continue;
        }
        let set_end = c2 + l2;
        let mut q = c2;
        while q < set_end {
            let (t3, c3, _l3, n3) = der_tlv(b, q)?;
            q = n3;
            if t3 != 0x30 {
                continue;
            }
            let (t4, c4, l4, n4) = match der_tlv(b, c3) {
                Some(v) => v,
                None => break,
            };
            if t4 != 0x06 {
                continue;
            }
            let oid = der_oid(b, c4, l4);
            let (t5, c5, l5, _) = match der_tlv(b, n4) {
                Some(v) => v,
                None => break,
            };
            let val = der_value(b, c5, l5, t5);
            parts.push(format!("{}={}", oid_name(&oid), val));
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

/// Interpreta un bloque DER como certificado X.509: `(issuer, subject)`.
fn der_parse_cert(b: &[u8]) -> Option<(String, String)> {
    let (tag, content, len, _) = der_tlv(b, 0)?;
    if tag != 0x30 {
        return None;
    }
    let _ = len;
    let (t1, c1, l1, _) = der_tlv(b, content)?;
    if t1 != 0x30 {
        return None;
    }
    let _ = l1;
    let mut p = c1;
    let (tv, _cv, _lv, nv) = der_tlv(b, p)?;
    if tv == 0xa0 {
        p = nv;
    }
    let (ts, _cs, _ls, ns) = der_tlv(b, p)?;
    if ts != 0x02 {
        return None;
    }
    p = ns;
    let (ta, _ca, _la, na) = der_tlv(b, p)?;
    if ta != 0x30 {
        return None;
    }
    p = na;
    let (ti, _ci, _li, ni) = der_tlv(b, p)?;
    if ti != 0x30 {
        return None;
    }
    let issuer = der_name(b, p)?;
    p = ni;
    let (tt, _ct, _lt, nt) = der_tlv(b, p)?;
    if tt != 0x30 {
        return None;
    }
    p = nt;
    let (tu, _cu, _lu, _nu) = der_tlv(b, p)?;
    if tu != 0x30 {
        return None;
    }
    let subject = der_name(b, p)?;
    Some((issuer, subject))
}

/// Busca un X.509 dentro de una estructura DER (PKCS#7 de la firma v1).
fn der_find_cert(b: &[u8]) -> Option<Vec<u8>> {
    fn walk(b: &[u8], off: usize, end: usize, depth: usize) -> Option<Vec<u8>> {
        if depth > 8 {
            return None;
        }
        let (tag, content, len, _) = der_tlv(b, off)?;
        let stop = content + len;
        if tag == 0x30 && der_parse_cert(&b[off..stop.min(b.len())]).is_some() {
            return Some(b[off..stop.min(b.len())].to_vec());
        }
        let mut p = content;
        while p < stop {
            let (_, _, _, next) = der_tlv(b, p)?;
            if next <= p {
                break;
            }
            if let Some(c) = walk(b, p, stop, depth + 1) {
                return Some(c);
            }
            p = next;
        }
        let _ = end;
        None
    }
    walk(b, 0, b.len(), 0)
}

/// Busca un X.509 dentro de un bloque con longitudes `u32` (v2/v3).
fn lp_find_cert(b: &[u8]) -> Option<Vec<u8>> {
    fn walk(b: &[u8], start: usize, end: usize, depth: usize) -> Option<Vec<u8>> {
        if depth > 8 {
            return None;
        }
        let mut p = start;
        while p + 4 <= end {
            let len = rd_u32_le(b, p) as usize;
            let content = p + 4;
            if len == 0 || content + len > end {
                break;
            }
            let chunk = &b[content..content + len];
            if der_parse_cert(chunk).is_some() {
                return Some(chunk.to_vec());
            }
            if let Some(c) = walk(chunk, 0, chunk.len(), depth + 1) {
                return Some(c);
            }
            p = content + len;
        }
        None
    }
    walk(b, 0, b.len(), 0)
}

/// Ficha del certificado del firmante.
fn cert_info(cert: &[u8]) -> Value {
    let sha = sha256_hex(cert);
    let (issuer, subject) = der_parse_cert(cert).unwrap_or_else(|| (String::new(), String::new()));
    let signer = if subject.is_empty() {
        issuer.clone()
    } else {
        subject.clone()
    };
    json!({
        "cert_sha256": sha,
        "cert_sha256_short": sha.chars().take(8).collect::<String>(),
        "subject": subject,
        "issuer": issuer,
        "signer": signer,
        "cert_bytes": cert.len(),
    })
}

/// Localiza el APK Signing Block y devuelve sus pares `(id, valor)`.
fn read_signing_block(path: &str, cd_offset: u64) -> Option<Vec<(u32, Vec<u8>)>> {
    let head = read_file_range(path, cd_offset.saturating_sub(24), 24)?;
    if head.len() < 32 || &head[16..32] != b"APK Sig Block 42" {
        return None;
    }
    let size = rd_u64_le(&head, 0);
    if size < 24 {
        return None;
    }
    let total = (size + 8) as usize;
    let start = cd_offset.checked_sub(total as u64)?;
    let raw = read_file_range(path, start, total)?;
    if raw.len() < total {
        return None;
    }
    let pairs_len = (size as usize).saturating_sub(24);
    let end = 8usize.saturating_add(pairs_len);
    let mut out: Vec<(u32, Vec<u8>)> = Vec::new();
    let mut p = 8usize;
    while p + 12 <= end && p + 12 <= raw.len() {
        let len = rd_u64_le(&raw, p) as usize;
        let id = rd_u32_le(&raw, p + 8);
        let vstart = p + 12;
        if len == 0 || vstart + len > raw.len() {
            break;
        }
        out.push((id, raw[vstart..vstart + len].to_vec()));
        p = vstart + len;
    }
    Some(out)
}

/// Informe de firma de la APK: v1/v2/v3, firmante y SHA-256 del certificado.
fn signature_report(path: &str, entries: &[ZipEntry]) -> Value {
    let mut v1 = false;
    let mut v1_file: Option<String> = None;
    for e in entries {
        let upper = e.name.to_ascii_uppercase();
        if upper.starts_with("META-INF/")
            && (upper.ends_with(".RSA") || upper.ends_with(".DSA") || upper.ends_with(".EC"))
        {
            v1 = true;
            if v1_file.is_none() {
                v1_file = Some(e.name.clone());
            }
        }
    }
    let mut schemes: Vec<String> = Vec::new();
    let mut cert: Option<Vec<u8>> = None;
    let mut source = "ninguna";

    // v2 (0x7109871a) y v3 (0xf05368c0) viven en el APK Signing Block.
    let size = file_size(path);
    if size > SIG_TAIL_BYTES as u64 {
        let tail =
            read_file_range(path, size - SIG_TAIL_BYTES as u64, SIG_TAIL_BYTES).unwrap_or_default();
        // El EOCD está al final: se busca hacia atrás.
        let mut eocd: Option<usize> = None;
        let mut i = tail.len().saturating_sub(22);
        while i > 0 {
            if tail[i..].starts_with(&[0x50, 0x4b, 0x05, 0x06]) {
                eocd = Some(i);
                break;
            }
            i -= 1;
        }
        if let Some(pos) = eocd {
            let cd_offset = rd_u32_le(&tail, pos + 16) as u64;
            if let Some(pairs) = read_signing_block(path, cd_offset) {
                for (id, value) in pairs {
                    match id {
                        0x7109_871a => {
                            schemes.push("v2".to_string());
                            if cert.is_none() {
                                if let Some(c) = lp_find_cert(&value) {
                                    cert = Some(c);
                                    source = "v2";
                                }
                            }
                        }
                        0xf053_68c0 => {
                            schemes.push("v3".to_string());
                            if cert.is_none() {
                                if let Some(c) = lp_find_cert(&value) {
                                    cert = Some(c);
                                    source = "v3";
                                }
                            }
                        }
                        0x1b93_ad61 => {
                            schemes.push("v3.1".to_string());
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    // v1: el PKCS#7 está en META-INF/*.RSA.
    if cert.is_none() {
        if let Some(name) = v1_file.clone() {
            if let Some(bytes) = read_apk_entry(path, &name, 4 * 1024 * 1024) {
                if let Some(c) = der_find_cert(&bytes) {
                    cert = Some(c);
                    source = "v1";
                }
            }
        }
    }
    let mut out = json!({
        "v1": v1,
        "v1_file": v1_file,
        "v2": schemes.iter().any(|s| s == "v2"),
        "v3": schemes.iter().any(|s| s == "v3"),
        "schemes": schemes,
        "source": source,
    });
    if let Some(c) = cert {
        if let Some(map) = out.as_object_mut() {
            map.insert("certificate".to_string(), cert_info(&c));
        }
    }
    out
}

// ========================================================================= //
// Ofuscación: fingerprint medible
// ========================================================================= //

/// ¿Es un identificador "corto" (1-2 letras), el signo clásico de ofuscación?
fn is_short_identifier(name: &str) -> bool {
    if name.is_empty() || name.len() > 2 {
        return false;
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    name.chars().any(|c| c.is_ascii_alphabetic())
}

/// Nombre simple de un descriptor de clase (`La/b/C;` → `C`).
fn simple_name(descriptor: &str) -> String {
    let d = descriptor.trim_start_matches('L');
    let d = d.trim_end_matches(';');
    d.rsplit('/').next().unwrap_or(d).to_string()
}

/// Calcula el fingerprint de ofuscación de los DEX de un APK.
///
/// Se apoya en `crate::dex_decompile` (API pública del carril de
/// MegaDaemon-RE): no se reimplementa el análisis de DEX aquí.
fn obfuscation_fingerprint(dex_files: &[(String, crate::dex_decompile::DexFile)]) -> Value {
    let mut classes_total = 0usize;
    let mut classes_short = 0usize;
    let mut fields_total = 0usize;
    let mut fields_short = 0usize;
    let mut methods_total = 0usize;
    let mut methods_short = 0usize;
    let mut strings_total = 0usize;
    let mut encrypted_strings = 0usize;
    let mut cfo = 0usize;
    let mut renamed_types = 0usize;
    let mut renamed_methods = 0usize;
    let mut with_source_file = 0usize;
    let mut with_line_numbers = 0usize;
    let mut methods_with_code = 0usize;
    let mut parsed = 0usize;

    for (_name, dex) in dex_files {
        parsed += 1;
        classes_total += dex.classes.len();
        for c in &dex.classes {
            if is_short_identifier(&simple_name(&c.class_name)) {
                classes_short += 1;
            }
            if !c.source_file.is_empty() {
                with_source_file += 1;
            }
            for m in c.direct_methods.iter().chain(c.virtual_methods.iter()) {
                methods_with_code += 1;
                if let Some(code) = m.code.as_ref() {
                    if code.debug_info_off != 0 {
                        with_line_numbers += 1;
                    }
                }
            }
        }
        fields_total += dex.fields.len();
        for f in &dex.fields {
            if is_short_identifier(&f.name) {
                fields_short += 1;
            }
        }
        methods_total += dex.methods.len();
        for m in &dex.methods {
            if is_short_identifier(&m.name) {
                methods_short += 1;
            }
        }
        strings_total += dex.strings.len();
        let report = crate::dex_decompile::analyze_obfuscation(dex);
        encrypted_strings += report.encrypted_strings.len();
        renamed_types += report.renamed_types.len();
        renamed_methods += report.renamed_methods.len();
        cfo += report.control_flow_obfuscation.len();
    }

    let ratio = |num: usize, den: usize| -> f64 {
        if den == 0 {
            0.0
        } else {
            (num as f64 * 10000.0 / den as f64).round() / 10000.0
        }
    };
    let classes_ratio = ratio(classes_short, classes_total);
    let fields_ratio = ratio(fields_short, fields_total);
    let methods_ratio = ratio(methods_short, methods_total);
    let encrypted_ratio = ratio(encrypted_strings, strings_total);
    let source_ratio = ratio(with_source_file, classes_total);
    let line_ratio = ratio(with_line_numbers, methods_with_code);

    // Puntuación 0..100: cuanto más ofuscado, más alto.
    let mut score = 0i64;
    score += (classes_ratio * 100.0).round() as i64 / 2;
    score += (fields_ratio * 100.0).round() as i64 / 3;
    score += (methods_ratio * 100.0).round() as i64 / 4;
    score += (encrypted_ratio * 1000.0).round() as i64;
    score += (cfo as i64).min(10);
    if source_ratio < 0.05 && classes_total > 50 {
        // Sin `SourceFile` en un APK grande: release minificado (típico R8).
        score += 10;
    }
    if score > 100 {
        score = 100;
    }
    let level = if score >= 70 {
        "alta"
    } else if score >= 35 {
        "media"
    } else if score >= 10 {
        "baja"
    } else {
        "nula"
    };

    json!({
        "dex_parsed": parsed,
        "dex_total": dex_files.len(),
        "classes_total": classes_total,
        "classes_short": classes_short,
        "classes_short_ratio": classes_ratio,
        "fields_total": fields_total,
        "fields_short": fields_short,
        "fields_short_ratio": fields_ratio,
        "methods_total": methods_total,
        "methods_short": methods_short,
        "methods_short_ratio": methods_ratio,
        "strings_total": strings_total,
        "encrypted_strings": encrypted_strings,
        "encrypted_strings_ratio": encrypted_ratio,
        "renamed_types": renamed_types,
        "renamed_methods": renamed_methods,
        "control_flow_obfuscation": cfo,
        "source_file_present": with_source_file,
        "source_file_ratio": source_ratio,
        "line_number_table_present": with_line_numbers,
        "line_number_table_ratio": line_ratio,
        "score": score,
        "level": level,
    })
}

// ========================================================================= //
// Marcadores: empaquetadores y frameworks de parche
// ========================================================================= //

/// Busca marcadores literales en un texto en minúsculas.
fn detect_markers(haystack: &str, table: &[(&str, &str, &str)]) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for (id, name, marker) in table {
        if !haystack.contains(&marker.to_ascii_lowercase()) {
            continue;
        }
        if out.iter().any(|v| v["id"] == *id) {
            continue;
        }
        out.push(json!({
            "id": id,
            "name": name,
            "marker": marker,
            "evidence": "marker",
        }));
    }
    out
}

// ========================================================================= //
// Componentes exportados sin permiso (hallazgo de seguridad)
// ========================================================================= //

/// Componentes `exported` que no exigen ningún permiso.
///
/// Es el hallazgo de seguridad real más típico de una app: cualquier otra
/// app puede lanzar su actividad, invocar su servicio o leer su proveedor.
pub fn exported_without_permission(manifest: &Value) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for group in ["activities", "services", "receivers", "providers"] {
        let list = match manifest[group].as_array() {
            Some(l) => l,
            None => continue,
        };
        for c in list {
            if c["exported"].as_bool() != Some(true) {
                continue;
            }
            let has_perm = c["permission"]
                .as_str()
                .map(|s| !s.is_empty())
                .unwrap_or(false);
            if group == "providers" {
                let rp = c["read_permission"].as_str().unwrap_or("");
                let wp = c["write_permission"].as_str().unwrap_or("");
                if !rp.is_empty() || !wp.is_empty() {
                    continue;
                }
            }
            if has_perm {
                continue;
            }
            let actions: Vec<String> = c["intent_filters"]
                .as_array()
                .map(|fs| {
                    fs.iter()
                        .flat_map(|f| {
                            f["actions"]
                                .as_array()
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|v| v.as_str().map(String::from))
                                        .collect::<Vec<String>>()
                                })
                                .unwrap_or_default()
                        })
                        .collect()
                })
                .unwrap_or_default();
            out.push(json!({
                "kind": group.trim_end_matches('s'),
                "name": c["name"].as_str().unwrap_or(""),
                "actions": actions,
                "authorities": c["authorities"].as_array().cloned().unwrap_or_default(),
            }));
        }
    }
    out
}

/// Hallazgos de seguridad del manifiesto (medidos, sin opinión).
fn security_findings(manifest: &Value) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    let app = &manifest["application"];
    if app["debuggable"].as_bool() == Some(true) {
        out.push(json!({
            "id": "debuggable",
            "severity": "media",
            "detail": "android:debuggable=true: se puede depurar y leer la memoria de la app",
        }));
    }
    if app["allow_backup"].as_bool() == Some(true) {
        out.push(json!({
            "id": "allow_backup",
            "severity": "media",
            "detail": "android:allowBackup=true: adb backup puede copiar sus datos",
        }));
    }
    if app["uses_cleartext_traffic"].as_bool() == Some(true) {
        out.push(json!({
            "id": "cleartext_traffic",
            "severity": "media",
            "detail": "android:usesCleartextTraffic=true: se permite HTTP sin cifrar",
        }));
    }
    if app["test_only"].as_bool() == Some(true) {
        out.push(json!({
            "id": "test_only",
            "severity": "baja",
            "detail": "android:testOnly=true: APK de pruebas",
        }));
    }
    let exported = exported_without_permission(manifest);
    if !exported.is_empty() {
        out.push(json!({
            "id": "exported_without_permission",
            "severity": "alto",
            "detail": format!("{} componente(s) exportados sin permiso", exported.len()),
            "components": exported,
        }));
    }
    out
}

// ========================================================================= //
// Ensamblado del análisis profundo de una APK
// ========================================================================= //

/// ¿Es un asset del que merece la pena leer texto?
fn is_text_asset(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".json")
        || lower.ends_with(".xml")
        || lower.ends_with(".txt")
        || lower.ends_with(".properties")
        || lower.ends_with(".js")
        || lower.ends_with(".html")
        || lower.ends_with(".cfg")
        || lower.ends_with(".conf")
        || lower.ends_with(".ini")
        || lower.ends_with(".yml")
        || lower.ends_with(".yaml")
        || lower.ends_with(".csv")
}

/// Lectura profunda de una APK: DEX, assets, nativas, firma y endpoints.
///
/// Devuelve un JSON con los bloques nuevos del escáner.  Nunca hace panic: si
/// una parte falla, se devuelve vacía y se anota el motivo.
fn deep_scan(path: &str, entries: &[ZipEntry], manifest: &Value) -> Value {
    // ------------------------------------------------------------------ //
    // 1. DEX
    // ------------------------------------------------------------------ //
    let mut parsed_dex: Vec<(String, crate::dex_decompile::DexFile)> = Vec::new();
    let mut dex_errors: Vec<String> = Vec::new();
    for name in dex_entry_names() {
        if parsed_dex.len() >= MAX_DEX_ANALYZED {
            break;
        }
        match read_apk_entry(path, &name, MAX_DEX_BYTES) {
            Some(bytes) if !bytes.is_empty() => match crate::dex_decompile::parse_dex(&bytes) {
                Ok(dex) => parsed_dex.push((name, dex)),
                Err(e) => dex_errors.push(format!("{}: {}", name, e)),
            },
            _ => {}
        }
    }

    // ------------------------------------------------------------------ //
    // 2. Assets (sólo los de texto: son los que traen endpoints y packs)
    // ------------------------------------------------------------------ //
    let mut asset_names: Vec<String> = Vec::new();
    let mut asset_text: String = String::new();
    let mut assets_read = 0usize;
    for e in entries {
        if !e.name.starts_with("assets/") {
            continue;
        }
        asset_names.push(e.name.clone());
        if assets_read >= MAX_ASSETS_READ || !is_text_asset(&e.name) {
            continue;
        }
        if let Some(bytes) = read_apk_entry(path, &e.name, MAX_ASSET_BYTES) {
            assets_read += 1;
            if asset_text.len() < 512 * 1024 {
                asset_text.push_str(&String::from_utf8_lossy(&bytes));
                asset_text.push('\n');
            }
        }
    }

    // ------------------------------------------------------------------ //
    // 3. Textos para endpoints y marcadores
    // ------------------------------------------------------------------ //
    let mut dex_text = String::new();
    for (_, dex) in &parsed_dex {
        if dex_text.len() > 2 * 1024 * 1024 {
            break;
        }
        for s in &dex.strings {
            if s.len() < 512 {
                dex_text.push_str(s);
                dex_text.push('\n');
            }
        }
    }
    let mut meta_text = String::new();
    for m in manifest["meta_data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        let n = m["name"].as_str().unwrap_or("");
        let v = m["value"].as_str().unwrap_or("");
        meta_text.push_str(n);
        meta_text.push(' ');
        meta_text.push_str(v);
        meta_text.push('\n');
    }
    let combined = format!("{}\n{}\n{}", dex_text, asset_text, meta_text);
    let endpoints = endpoints_from_text(&combined);

    // ------------------------------------------------------------------ //
    // 4. Marcadores (empaquetadores y frameworks de parche)
    // ------------------------------------------------------------------ //
    let mut haystack = String::new();
    for e in entries {
        haystack.push_str(&e.name);
        haystack.push('\n');
    }
    for line in dex_text.lines() {
        haystack.push_str(line);
        haystack.push('\n');
    }
    haystack.push_str(&meta_text);
    let haystack = haystack.to_ascii_lowercase();
    let packers = detect_markers(&haystack, PACKER_MARKERS);
    let patch_frameworks = detect_markers(&haystack, PATCH_FRAMEWORK_MARKERS);
    // El marcador de la app_intel (Lucky Patcher repaquetado) se consume tal
    // cual: la API pública ya está hecha.
    let patcher_toolkit = crate::app_intel::detect_patcher_toolkit_detail(&haystack);

    // ------------------------------------------------------------------ //
    // 5. Librerías nativas
    // ------------------------------------------------------------------ //
    let mut natives: Vec<Value> = Vec::new();
    let mut so_read = 0usize;
    for e in entries {
        if !e.name.starts_with("lib/") || !e.name.ends_with(".so") {
            continue;
        }
        let parts: Vec<&str> = e.name.split('/').collect();
        let abi = parts.get(1).copied().unwrap_or("").to_string();
        let file = parts.get(2).copied().unwrap_or("").to_string();
        let mut item = json!({
            "name": file,
            "path": e.name,
            "abi": abi,
            "size": e.uncompressed_size,
            "compressed_size": e.compressed_size,
            "stripped": Value::Null,
            "exports": Value::Null,
            "has_debug": Value::Null,
            "machine": Value::Null,
            "elf_class": Value::Null,
            "sections": Value::Null,
        });
        if so_read < MAX_SO_ANALYZED {
            if let Some(bytes) = read_apk_entry(path, &e.name, MAX_SO_BYTES) {
                so_read += 1;
                if let Some(info) = parse_elf(&bytes) {
                    item["stripped"] = json!(info.stripped);
                    item["exports"] = json!(info.exports);
                    item["has_debug"] = json!(info.has_debug);
                    item["machine"] = json!(info.machine);
                    item["elf_class"] = json!(info.class);
                    item["sections"] = json!(info.sections);
                }
            }
        }
        natives.push(item);
    }

    // ------------------------------------------------------------------ //
    // 6. Firma
    // ------------------------------------------------------------------ //
    let signature = signature_report(path, entries);

    // ------------------------------------------------------------------ //
    // 7. Permisos por riesgo, componentes exportados y hallazgos
    // ------------------------------------------------------------------ //
    let permissions_risk = permission_risk_groups(manifest);
    let exported = exported_without_permission(manifest);
    let findings = security_findings(manifest);

    json!({
        "obfuscation": obfuscation_fingerprint(&parsed_dex),
        "packers": packers,
        "patch_frameworks": patch_frameworks,
        "patcher_toolkit": patcher_toolkit,
        "permissions_risk": permissions_risk,
        "endpoints": endpoints,
        "native_libraries": natives,
        "native_libraries_analyzed": so_read,
        "signature": signature,
        "exported_without_permission": exported,
        "security_findings": findings,
        "dex_errors": dex_errors,
        "assets_read": assets_read,
    })
}

// ========================================================================= //
// Scanner del móvil: `pm list packages` + manifiesto + puntuación
// ========================================================================= //

/// Extrae pares `(paquete, ruta de APK)` de `pm list packages -f`.
///
/// El formato real es `package:/ruta/base.apk=com.paquete`.
pub fn parse_package_paths(stdout: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        let Some(body) = line.strip_prefix("package:") else {
            continue;
        };
        let Some((path, pkg)) = body.rsplit_once('=') else {
            continue;
        };
        let path = path.trim();
        let pkg = pkg.trim();
        if pkg.is_empty() || path.is_empty() {
            continue;
        }
        out.push((pkg.to_string(), path.to_string()));
    }
    out
}

/// Extrae el instalador de cada paquete de `pm list packages -i`.
///
/// El formato real es `package:com.paquete=com.android.vending`.
pub fn parse_installers(stdout: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        let Some(body) = line.strip_prefix("package:") else {
            continue;
        };
        let Some((pkg, installer)) = body.rsplit_once('=') else {
            continue;
        };
        let pkg = pkg.trim();
        if pkg.is_empty() {
            continue;
        }
        out.push((pkg.to_string(), installer.trim().to_string()));
    }
    out
}

/// Parsea `dumpsys package` (todos los paquetes) en fichas por app.
///
/// Función ** pura**: no toca el dispositivo, así que se puede testear con un
/// volcado real.
pub fn parse_dumpsys_packages(stdout: &str) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    let mut cur: Option<Value> = None;
    let mut mode = 0u8; // 0 = normal, 1 = permisos, 2 = usuario
    for raw in stdout.lines() {
        let trimmed = raw.trim();
        if let Some(rest) = trimmed.strip_prefix("Package [") {
            if let Some(pkg) = rest.split(']').next() {
                cur = Some(json!({
                    "package": pkg,
                    "version_code": Value::Null,
                    "version_name": Value::Null,
                    "min_sdk": Value::Null,
                    "target_sdk": Value::Null,
                    "code_path": Value::Null,
                    "data_dir": Value::Null,
                    "installer": Value::Null,
                    "first_install": Value::Null,
                    "last_update": Value::Null,
                    "signing_version": Value::Null,
                    "enabled": Value::Null,
                    "flags": json!([]),
                    "requested_permissions": json!([]),
                    "granted_permissions": json!([]),
                }));
                mode = 0;
            }
            continue;
        }
        let Some(pkg_rec) = cur.as_mut() else {
            continue;
        };
        if trimmed == "install permissions:" || trimmed == "runtime permissions:" {
            mode = 1;
            continue;
        }
        if trimmed.starts_with("User ") && trimmed.contains("installed=") {
            // La línea "User 0: ..." cierra el bloque de permisos: se cambia de
            // modo **sin** `continue` para poder leer el estado de la misma línea.
            mode = 2;
        }
        if mode == 2 {
            if let Some(v) = trimmed
                .split("enabled=")
                .nth(1)
                .and_then(|x| x.split_whitespace().next())
            {
                pkg_rec["enabled"] = json!(v);
            }
            if let Some(v) = trimmed.strip_prefix("firstInstallTime=") {
                pkg_rec["first_install"] = json!(v.trim());
            }
            if let Some(v) = trimmed.strip_prefix("lastUpdateTime=") {
                pkg_rec["last_update"] = json!(v.trim());
            }
            continue;
        }
        if mode == 1 {
            if let Some((perm, granted)) = parse_permission_line(trimmed) {
                let mut req = pkg_rec["requested_permissions"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                if !req.iter().any(|v| v == &json!(perm)) {
                    req.push(json!(perm));
                }
                pkg_rec["requested_permissions"] = json!(req);
                if granted {
                    let mut gr = pkg_rec["granted_permissions"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default();
                    if !gr.iter().any(|v| v == &json!(perm)) {
                        gr.push(json!(perm));
                    }
                    pkg_rec["granted_permissions"] = json!(gr);
                }
                continue;
            }
            mode = 0;
        }
        // Atributos del paquete (4 espacios de sangría).
        if !raw.starts_with("    ") {
            continue;
        }
        if let Some(v) = trimmed.strip_prefix("versionCode=") {
            let mut it = v.split_whitespace();
            if let Some(code) = it.next().and_then(|x| x.parse::<i64>().ok()) {
                pkg_rec["version_code"] = json!(code);
            }
            for tok in it {
                if let Some(x) = tok.strip_prefix("minSdk=") {
                    pkg_rec["min_sdk"] = json!(x.parse::<i64>().unwrap_or(-1));
                } else if let Some(x) = tok.strip_prefix("targetSdk=") {
                    pkg_rec["target_sdk"] = json!(x.parse::<i64>().unwrap_or(-1));
                }
            }
        } else if let Some(v) = trimmed.strip_prefix("versionName=") {
            pkg_rec["version_name"] = json!(v.trim());
        } else if let Some(v) = trimmed.strip_prefix("codePath=") {
            pkg_rec["code_path"] = json!(v.trim());
        } else if let Some(v) = trimmed.strip_prefix("dataDir=") {
            pkg_rec["data_dir"] = json!(v.trim());
        } else if let Some(v) = trimmed.strip_prefix("installerPackageName=") {
            pkg_rec["installer"] = json!(v.trim());
        } else if let Some(v) = trimmed.strip_prefix("firstInstallTime=") {
            pkg_rec["first_install"] = json!(v.trim());
        } else if let Some(v) = trimmed.strip_prefix("lastUpdateTime=") {
            pkg_rec["last_update"] = json!(v.trim());
        } else if let Some(v) = trimmed.strip_prefix("apkSigningVersion=") {
            pkg_rec["signing_version"] = json!(v.trim());
        } else if let Some(v) = trimmed.strip_prefix("flags=[") {
            let flags: Vec<String> = v
                .trim_end_matches(']')
                .split_whitespace()
                .map(|x| x.to_string())
                .filter(|x| !x.is_empty())
                .collect();
            pkg_rec["flags"] = json!(flags);
        }
    }
    if let Some(p) = cur {
        out.push(p);
    }
    out
}

/// Parsea una línea `permiso: granted=true`.
fn parse_permission_line(line: &str) -> Option<(String, bool)> {
    let (perm, rest) = line.split_once(": ")?;
    if !perm.contains('.') {
        return None;
    }
    let granted = rest.trim() == "granted=true";
    Some((perm.trim().to_string(), granted))
}

/// Suma un motivo a la puntuación de riesgo de una app.
fn push_reason(reasons: &mut Vec<Value>, score: &mut i64, code: &str, weight: i64, detail: String) {
    *score += weight;
    reasons.push(json!({
        "code": code,
        "weight": weight,
        "detail": detail,
    }));
}

/// Puntúa el riesgo de una app instalada.  Función ** pura** (testeable).
///
/// La puntuación es la suma de los pesos de cada motivo; la lista de motivos
/// explica **por qué** puntúa así (nada de "parece sospechoso").
pub fn score_installed_app(rec: &Value) -> Value {
    let mut score = 0i64;
    let mut reasons: Vec<Value> = Vec::new();

    let perms: Vec<String> = rec["requested_permissions"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let dangerous: Vec<String> = perms
        .iter()
        .filter(|p| permission_risk_class(p) == "peligroso")
        .cloned()
        .collect();
    let high: Vec<String> = perms
        .iter()
        .filter(|p| permission_risk_class(p) == "alto")
        .cloned()
        .collect();
    if !dangerous.is_empty() {
        let w = (dangerous.len() as i64 * 3).min(30);
        push_reason(
            &mut reasons,
            &mut score,
            "dangerous_permissions",
            w,
            format!(
                "{} permiso(s) peligroso(s): {}",
                dangerous.len(),
                dangerous.join(", ")
            ),
        );
    }
    if !high.is_empty() {
        let w = (high.len() as i64 * 4).min(20);
        push_reason(
            &mut reasons,
            &mut score,
            "high_permissions",
            w,
            format!(
                "{} permiso(s) de alto riesgo: {}",
                high.len(),
                high.join(", ")
            ),
        );
    }
    let exported = rec["exported_without_permission"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if !exported.is_empty() {
        let w = (exported.len() as i64 * 6).min(18);
        push_reason(
            &mut reasons,
            &mut score,
            "exported_without_permission",
            w,
            format!("{} componente(s) exportados sin permiso", exported.len()),
        );
    }
    let installer = rec["installer"].as_str().unwrap_or("");
    if installer.is_empty() || installer == "null" {
        push_reason(
            &mut reasons,
            &mut score,
            "sideloaded",
            4,
            "sin instalador registrado: APK cargado a mano (sideload)".to_string(),
        );
    } else if installer != "com.android.vending" {
        push_reason(
            &mut reasons,
            &mut score,
            "third_party_installer",
            2,
            format!("instalada desde {}", installer),
        );
    }
    let target = rec["target_sdk"].as_i64().unwrap_or(0);
    if target > 0 && target < 26 {
        push_reason(
            &mut reasons,
            &mut score,
            "legacy_target_sdk",
            3,
            format!(
                "targetSdk {}: permisos se conceden en la instalación",
                target
            ),
        );
    }
    if perms.len() > 40 {
        push_reason(
            &mut reasons,
            &mut score,
            "too_many_permissions",
            2,
            format!("{} permisos pedidos", perms.len()),
        );
    }
    if rec["enabled"].as_str() == Some("0") {
        push_reason(
            &mut reasons,
            &mut score,
            "disabled",
            1,
            "app deshabilitada pero con permisos declarados".to_string(),
        );
    }
    if rec["debuggable"].as_bool() == Some(true) {
        push_reason(
            &mut reasons,
            &mut score,
            "debuggable",
            3,
            "android:debuggable=true".to_string(),
        );
    }
    let frameworks = rec["patch_frameworks"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if !frameworks.is_empty() {
        let names: Vec<String> = frameworks
            .iter()
            .filter_map(|v| v["name"].as_str().map(String::from))
            .collect();
        push_reason(
            &mut reasons,
            &mut score,
            "patch_framework",
            8,
            format!(
                "framework de parche/hook dentro del APK: {}",
                names.join(", ")
            ),
        );
    }
    let packs = rec["packers"].as_array().cloned().unwrap_or_default();
    if !packs.is_empty() {
        let names: Vec<String> = packs
            .iter()
            .filter_map(|v| v["name"].as_str().map(String::from))
            .collect();
        push_reason(
            &mut reasons,
            &mut score,
            "packer",
            5,
            format!("empaquetador/protector detectado: {}", names.join(", ")),
        );
    }
    let level = if score >= 25 {
        "critico"
    } else if score >= 15 {
        "alto"
    } else if score >= 8 {
        "medio"
    } else if score > 0 {
        "bajo"
    } else {
        "sin_riesgo"
    };
    json!({
        "risk_score": score,
        "risk_level": level,
        "reasons": reasons,
    })
}

/// Escanea las apps instaladas en el móvil y devuelve un **ranking de riesgo**.
///
/// * `limit` = número máximo de apps analizadas (0 = `MAX_SCAN_APPS`).
/// * ElDevice se consulta con `crate::android::root_exec`: `pm list packages
///   -3 -f` (rutas) y `dumpsys package` (versión, instalador, estado).
/// * Por cada app se lee **sólo el manifiesto** del APK: es lo que da los
///   permisos y los componentes exportados, que son los que puntúan.
pub fn scan_installed(limit: usize) -> Value {
    let started = Instant::now();
    let max = if limit == 0 {
        MAX_SCAN_APPS
    } else {
        limit.min(MAX_SCAN_APPS)
    };

    // 1) Rutas de las APK instaladas por el usuario (una llamada).
    let listed = crate::android::root_exec(
        "pm list packages -3 -f 2>/dev/null",
        PM_TIMEOUT_MS,
        512 * 1024,
    );
    let pairs = parse_package_paths(&listed.stdout);

    // 2) Ficha de cada paquete (versión, instalador, estado, permisos).
    let dumped =
        crate::android::root_exec("dumpsys package 2>/dev/null", 120_000, 16 * 1024 * 1024);
    let dumps = parse_dumpsys_packages(&dumped.stdout);
    let dump_idx: Vec<(String, Value)> = dumps
        .into_iter()
        .map(|d| {
            let pkg = d["package"].as_str().unwrap_or("").to_string();
            (pkg, d)
        })
        .collect();

    let mut packages: Vec<Value> = Vec::new();
    let mut errors: Vec<Value> = Vec::new();
    for (pkg, apk_path) in pairs.iter().take(max) {
        let mut rec = json!({
            "package": pkg,
            "apk_path": apk_path,
            "version_name": Value::Null,
            "version_code": Value::Null,
            "target_sdk": Value::Null,
            "installer": Value::Null,
            "enabled": Value::Null,
            "requested_permissions": json!([]),
            "exported_without_permission": json!([]),
            "packers": json!([]),
            "patch_frameworks": json!([]),
            "debuggable": Value::Null,
        });
        if let Some((_, d)) = dump_idx.iter().find(|(p, _)| p == pkg) {
            rec["version_name"] = d["version_name"].clone();
            rec["version_code"] = d["version_code"].clone();
            rec["target_sdk"] = d["target_sdk"].clone();
            rec["installer"] = d["installer"].clone();
            rec["enabled"] = d["enabled"].clone();
            if d["requested_permissions"]
                .as_array()
                .map(|a| !a.is_empty())
                .unwrap_or(false)
            {
                rec["requested_permissions"] = d["requested_permissions"].clone();
            }
        }
        // Manifiesto: permisos, componentes y packs.
        match read_apk_entry(apk_path, "AndroidManifest.xml", 4 * 1024 * 1024) {
            Some(bytes) => {
                let manifest = manifest_axml(&bytes);
                rec["permissions"] = manifest["permissions"].clone();
                rec["target_sdk"] = if rec["target_sdk"].is_null() {
                    manifest["target_sdk"].clone()
                } else {
                    rec["target_sdk"].clone()
                };
                rec["version_code"] = if rec["version_code"].is_null() {
                    manifest["version_code"].clone()
                } else {
                    rec["version_code"].clone()
                };
                rec["version_name"] = if rec["version_name"].is_null() {
                    manifest["version_name"].clone()
                } else {
                    rec["version_name"].clone()
                };
                rec["exported_without_permission"] = json!(exported_without_permission(&manifest));
                rec["debuggable"] = manifest["application"]["debuggable"].clone();
                // Marcadores de parche: el manifiesto y el nombre del paquete.
                let mut hay = String::new();
                hay.push_str(pkg);
                hay.push(' ');
                hay.push_str(&String::from_utf8_lossy(&bytes));
                hay = hay.to_ascii_lowercase();
                rec["patch_frameworks"] = json!(detect_markers(&hay, PATCH_FRAMEWORK_MARKERS));
                rec["packers"] = json!(detect_markers(&hay, PACKER_MARKERS));
            }
            None => {
                errors.push(json!({ "package": pkg, "error": "manifest_unreadable" }));
            }
        }
        let score = score_installed_app(&rec);
        if let Some(map) = rec.as_object_mut() {
            map.insert("risk_score".to_string(), score["risk_score"].clone());
            map.insert("risk_level".to_string(), score["risk_level"].clone());
            map.insert("reasons".to_string(), score["reasons"].clone());
        }
        packages.push(rec);
    }

    // Ranking: de más riesgo a menos.
    let mut ranking: Vec<Value> = packages.clone();
    ranking.sort_by(|a, b| {
        b["risk_score"]
            .as_i64()
            .unwrap_or(0)
            .cmp(&a["risk_score"].as_i64().unwrap_or(0))
    });
    json!({
        "ok": true,
        "count": packages.len(),
        "scanned": packages.len(),
        "limit": max,
        "duration_ms": started.elapsed().as_millis() as u64,
        "packages": packages,
        "ranking": ranking,
        "errors": errors,
    })
}

/// Sólo el **ranking de riesgo** de lo instalado (mismo barrido, otra vista).
///
/// Es la acción `rank` de la tool: la lista de apps ordenada de más a menos
/// riesgo, con el motivo numérico de cada una.
pub fn rank_risk(limit: usize) -> Value {
    let mut out = scan_installed(limit);
    if let Some(map) = out.as_object_mut() {
        map.insert("action".to_string(), json!("rank"));
    }
    out
}

/// Alias de [`rank_risk`].
pub fn rank_installed(limit: usize) -> Value {
    rank_risk(limit)
}

/// Barrido + **veredicto** por app (`KEEP` / `REIMPLEMENTAR EN RUST` /
/// `DESINSTALAR`), con el módulo Rust destino cuando procede.
///
/// El veredicto lo decide `crate::app_intel::verdict_for` (motor de
/// veredictos): aquí sólo se le pasa el registro medido.
pub fn verdict_installed(limit: usize) -> Value {
    let mut out = scan_installed(limit);
    if let Some(map) = out.as_object_mut() {
        map.insert("action".to_string(), json!("verdict"));
        let packages = map
            .get("packages")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut with_verdict: Vec<Value> = Vec::with_capacity(packages.len());
        for p in packages {
            let v = crate::app_intel::verdict_for(&p);
            let mut p2 = p.clone();
            if let Some(m) = p2.as_object_mut() {
                m.insert("verdict".to_string(), v["verdict"].clone());
                m.insert("verdict_reason".to_string(), v["reason"].clone());
                m.insert("rust_module".to_string(), v["rust_module"].clone());
                m.insert("verdict_source".to_string(), v["source"].clone());
            }
            with_verdict.push(p2);
        }
        map.insert("packages".to_string(), json!(with_verdict));
    }
    out
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
            (
                "manifest",
                &[("package", TestVal::S("com.example.sintetica"))],
            ),
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
        assert_eq!(
            filters[0]["actions"][0],
            json!("android.intent.action.MAIN")
        );
        assert_eq!(
            filters[0]["categories"][0],
            json!("android.intent.category.LAUNCHER")
        );
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
        let salida =
            "package:/data/app/~~a==/com.foo-1/base.apk\npackage:/system/app/Bar/Bar.apk\n";
        let rutas = parse_apk_paths(salida);
        assert_eq!(rutas.len(), 2);
        assert_eq!(rutas[0], "/data/app/~~a==/com.foo-1/base.apk");
        assert_eq!(package_from_path(&rutas[0]).as_deref(), Some("com.foo"));
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

    // ================================================================= //
    // Tests del escáner (sintéticos, sin red y sin dispositivo)
    // ================================================================= //

    /// Manifiesto sintético con un componente exportado SIN permiso, otro con
    /// permiso y un `provider` sin permisos de lectura/escritura.
    fn build_manifest_con_exportados() -> Vec<u8> {
        build_axml(&[
            (
                "manifest",
                &[
                    ("package", TestVal::S("com.example.expuesta")),
                    ("versionCode", TestVal::I(7)),
                ],
            ),
            (
                "uses-permission",
                &[("name", TestVal::S("android.permission.INTERNET"))],
            ),
            (
                "uses-permission",
                &[("name", TestVal::S("android.permission.CAMERA"))],
            ),
            (
                "uses-permission",
                &[(
                    "name",
                    TestVal::S("android.permission.ACCESS_FINE_LOCATION"),
                )],
            ),
            (
                "uses-permission",
                &[(
                    "name",
                    TestVal::S("android.permission.REQUEST_INSTALL_PACKAGES"),
                )],
            ),
            (
                "application",
                &[
                    ("debuggable", TestVal::B(true)),
                    ("allowBackup", TestVal::B(true)),
                    ("usesCleartextTraffic", TestVal::B(true)),
                ],
            ),
            (
                "activity",
                &[
                    ("name", TestVal::S("com.example.expuesta.SinPermiso")),
                    ("exported", TestVal::B(true)),
                ],
            ),
            (
                "service",
                &[
                    ("name", TestVal::S("com.example.expuesta.ConPermiso")),
                    ("exported", TestVal::B(true)),
                    ("permission", TestVal::S("com.example.expuesta.PRIVADO")),
                ],
            ),
            (
                "provider",
                &[
                    ("name", TestVal::S("com.example.expuesta.Provider")),
                    ("exported", TestVal::B(true)),
                    ("authorities", TestVal::S("com.example.expuesta.datos")),
                ],
            ),
        ])
    }

    #[test]
    fn los_permisos_se_agrupan_por_clase_de_riesgo() {
        assert_eq!(
            permission_risk_class("android.permission.CAMERA"),
            "peligroso"
        );
        assert_eq!(
            permission_risk_class("android.permission.REQUEST_INSTALL_PACKAGES"),
            "alto"
        );
        assert_eq!(
            permission_risk_class("android.permission.INTERNET"),
            "medio"
        );
        assert_eq!(permission_risk_class("com.mi.app.PERMISO_PROPIO"), "bajo");
        let m = manifest_axml(&build_manifest_con_exportados());
        let g = permission_risk_groups(&m);
        assert_eq!(g["total"], json!(4));
        assert_eq!(g["counts"]["peligroso"], json!(2));
        assert_eq!(g["counts"]["alto"], json!(1));
        assert_eq!(g["counts"]["medio"], json!(1));
        assert_eq!(g["counts"]["bajo"], json!(0));
    }

    #[test]
    fn los_componentes_exportados_sin_permiso_se_detectan() {
        let m = manifest_axml(&build_manifest_con_exportados());
        let v = exported_without_permission(&m);
        // La actividad sin permiso y el provider sin permisos de lectura.
        assert_eq!(v.len(), 2);
        let nombres: Vec<String> = v
            .iter()
            .filter_map(|c| c["name"].as_str().map(String::from))
            .collect();
        assert!(nombres.contains(&"com.example.expuesta.SinPermiso".to_string()));
        assert!(nombres.contains(&"com.example.expuesta.Provider".to_string()));
        // El service con permiso NO aparece.
        assert!(!nombres.contains(&"com.example.expuesta.ConPermiso".to_string()));
    }

    #[test]
    fn el_manifiesto_expone_permiso_meta_data_y_atributos_de_application() {
        let m = manifest_axml(&build_manifest_con_exportados());
        assert_eq!(
            m["services"][0]["permission"],
            json!("com.example.expuesta.PRIVADO")
        );
        assert_eq!(
            m["providers"][0]["authorities"],
            json!(["com.example.expuesta.datos"])
        );
        assert_eq!(m["application"]["debuggable"], json!(true));
        assert_eq!(m["application"]["allow_backup"], json!(true));
        assert_eq!(m["application"]["uses_cleartext_traffic"], json!(true));
    }

    #[test]
    fn el_escaner_de_una_apk_sintetica_devuelve_todos_los_bloques() {
        let path = test_path("escaner.apk");
        build_synthetic_apk(&path);
        let a = analyze_apk(path.to_str().unwrap());
        assert_eq!(a["ok"], json!(true));
        // Bloques nuevos del escáner.
        assert!(a["obfuscation"].is_object());
        assert!(a["obfuscation"]["score"].is_i64());
        assert!(a["permissions_risk"].is_object());
        assert!(a["endpoints"].is_object());
        assert!(a["native_libraries"].is_array());
        assert!(a["signature"].is_object());
        assert!(a["exported_without_permission"].is_array());
        assert!(a["security_findings"].is_array());
        // La `.so` falsa no es un ELF: no rompe nada.
        let libs = a["native_libraries"].as_array().unwrap();
        assert_eq!(libs.len(), 1);
        assert_eq!(libs[0]["abi"], json!("arm64-v8a"));
        assert_eq!(libs[0]["name"], json!("libdemo.so"));
        assert!(libs[0]["stripped"].is_null());
        // Los DEX falsos no parsean: el escáner lo anota y sigue.
        assert!(a["obfuscation"]["dex_parsed"].as_i64().unwrap_or(0) == 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn el_escaner_detecta_el_componente_exportado_en_una_apk_sintetica() {
        let path = test_path("exportada.apk");
        let manifest = build_manifest_con_exportados();
        {
            use std::io::Write;
            let file = std::fs::File::create(&path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file("AndroidManifest.xml", opts).unwrap();
            zip.write_all(&manifest).unwrap();
            zip.finish().unwrap();
        }
        let a = analyze_apk(path.to_str().unwrap());
        let v = a["exported_without_permission"].as_array().unwrap();
        assert_eq!(v.len(), 2);
        let findings = a["security_findings"].as_array().unwrap();
        assert!(findings
            .iter()
            .any(|f| f["id"] == json!("exported_without_permission")));
        assert!(findings.iter().any(|f| f["id"] == json!("debuggable")));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn el_parser_de_pm_list_packages_f_extrae_paquete_y_ruta() {
        let salida = "package:/data/app/~~a==/com.foo-1/base.apk=com.foo\n\
                      package:/system/app/Bar/Bar.apk=com.bar\n\
                      paquete:sin_prefijo\n";
        let pares = parse_package_paths(salida);
        assert_eq!(pares.len(), 2);
        assert_eq!(pares[0].0, "com.foo");
        assert_eq!(pares[0].1, "/data/app/~~a==/com.foo-1/base.apk");
        assert_eq!(pares[1].0, "com.bar");
    }

    #[test]
    fn el_parser_de_instaladores_extrae_el_instalador() {
        let salida = "package:com.foo=com.android.vending\npackage:com.bar=null\n";
        let inst = parse_installers(salida);
        assert_eq!(inst.len(), 2);
        assert_eq!(
            inst[0],
            ("com.foo".to_string(), "com.android.vending".to_string())
        );
        assert_eq!(inst[1], ("com.bar".to_string(), "null".to_string()));
    }

    #[test]
    fn el_parser_de_dumpsys_extrae_la_measured_de_cada_paquete() {
        // Volcado recortado del formato real de `dumpsys package`.
        // (Cadena cruda: la sangría de 4 espacios es parte del formato.)
        let salida = r#"Packages:
  Package [com.ejemplo] (a1b2c3):
    appId=10123
    codePath=/data/app/~~x==/com.ejemplo-y==/base.apk
    versionCode=4242 minSdk=23 targetSdk=33
    versionName=9.9.9
    apkSigningVersion=3
    flags=[ HAS_CODE ALLOW_BACKUP ]
    installerPackageName=com.android.vending
    install permissions:
      android.permission.INTERNET: granted=true
      android.permission.CAMERA: granted=false
    User 0: ceDataInode=1 installed=true hidden=false suspended=false distractionFlags=0 stopped=true notLaunched=false enabled=0 instant=false virtual=false
      installReason=4
      firstInstallTime=2026-01-25 13:10:05
      uninstallReason=0
"#;
        let pkgs = parse_dumpsys_packages(salida);
        assert_eq!(pkgs.len(), 1);
        let p = &pkgs[0];
        assert_eq!(p["package"], json!("com.ejemplo"));
        assert_eq!(p["version_code"], json!(4242));
        assert_eq!(p["version_name"], json!("9.9.9"));
        assert_eq!(p["min_sdk"], json!(23));
        assert_eq!(p["target_sdk"], json!(33));
        assert_eq!(p["installer"], json!("com.android.vending"));
        assert_eq!(p["enabled"], json!("0"));
        assert_eq!(p["first_install"], json!("2026-01-25 13:10:05"));
        assert_eq!(p["flags"].as_array().unwrap().len(), 2);
        let req = p["requested_permissions"].as_array().unwrap();
        assert_eq!(req.len(), 2);
        let gr = p["granted_permissions"].as_array().unwrap();
        assert_eq!(gr.len(), 1);
        assert_eq!(gr[0], json!("android.permission.INTERNET"));
    }

    #[test]
    fn la_puntuacion_de_riesgo_suma_los_motivos() {
        let rec = json!({
            "package": "com.ejemplo",
            "apk_path": "/data/app/~~x==/com.ejemplo/base.apk",
            "requested_permissions": [
                "android.permission.CAMERA",
                "android.permission.RECORD_AUDIO",
                "android.permission.REQUEST_INSTALL_PACKAGES",
            ],
            "exported_without_permission": [
                { "kind": "activity", "name": "com.ejemplo.A" },
            ],
            "installer": "null",
            "target_sdk": 24,
            "enabled": "0",
        });
        let s = score_installed_app(&rec);
        // 2 peligrosos (3+3) + 1 alto (4) + 1 exportado (6) + sideload (4)
        // + targetSdk legado (3) + deshabilitada (1) = 24.
        assert_eq!(s["risk_score"], json!(24));
        assert_eq!(s["risk_level"], json!("alto"));
        let motivos = s["reasons"].as_array().unwrap();
        let codigos: Vec<String> = motivos
            .iter()
            .filter_map(|m| m["code"].as_str().map(String::from))
            .collect();
        assert!(codigos.contains(&"dangerous_permissions".to_string()));
        assert!(codigos.contains(&"high_permissions".to_string()));
        assert!(codigos.contains(&"exported_without_permission".to_string()));
        assert!(codigos.contains(&"sideloaded".to_string()));
        assert!(codigos.contains(&"legacy_target_sdk".to_string()));
        // Cada motivo lleva su peso: la suma cuadra.
        let suma: i64 = motivos.iter().filter_map(|m| m["weight"].as_i64()).sum();
        assert_eq!(suma, s["risk_score"].as_i64().unwrap());
    }

    #[test]
    fn una_app_limpia_no_puntua() {
        let rec = json!({
            "package": "com.ejemplo",
            "apk_path": "/data/app/~~x==/com.ejemplo/base.apk",
            "requested_permissions": ["android.permission.INTERNET"],
            "exported_without_permission": [],
            "installer": "com.android.vending",
            "target_sdk": 34,
        });
        let s = score_installed_app(&rec);
        assert_eq!(s["risk_score"], json!(0));
        assert_eq!(s["risk_level"], json!("sin_riesgo"));
    }

    #[test]
    fn los_endpoints_salen_de_urls_dominios_e_ips() {
        let texto = "conecta a https://api.ejemplo.com/v1/datos y a http://109.9.8.7/x \
                     y a https://cdn.otro.net/recursos";
        let e = endpoints_from_text(texto);
        let urls = e["urls"].as_array().unwrap();
        assert!(urls
            .iter()
            .any(|u| u == &json!("https://api.ejemplo.com/v1/datos")));
        let dominios = e["domains"].as_array().unwrap();
        assert!(dominios.iter().any(|d| d == &json!("api.ejemplo.com")));
        assert!(dominios.iter().any(|d| d == &json!("cdn.otro.net")));
        let ips = e["ips"].as_array().unwrap();
        assert!(ips.iter().any(|i| i == &json!("109.9.8.7")));
    }

    #[test]
    fn los_dominios_falsos_no_se_aceptan() {
        assert!(normalize_domain("com.example.app").is_some());
        assert!(normalize_domain("1.2.3.4").is_none());
        assert!(normalize_domain("version").is_none());
        assert!(normalize_domain("example.invalidtld").is_none());
        assert!(scan_ipv4("10.0.0.1 y 999.1.1.1").contains(&"10.0.0.1".to_string()));
        assert_eq!(scan_ipv4("999.1.1.1").len(), 0);
    }

    #[test]
    fn los_marcadores_de_pack_y_de_parche_se_detectan() {
        let hay = "com.lsposed.lspatch y libjiagu.so y com.chelpus.lucky";
        let packs = detect_markers(hay, PACKER_MARKERS);
        assert!(packs.iter().any(|p| p["id"] == json!("jiagu")));
        let frameworks = detect_markers(hay, PATCH_FRAMEWORK_MARKERS);
        assert!(frameworks.iter().any(|p| p["id"] == json!("lspatch")));
        assert!(frameworks.iter().any(|p| p["id"] == json!("lucky-patcher")));
    }

    /// ELF64 mínimo con tres secciones: null, `.shstrtab` y `.dynsym`.
    fn build_min_elf64() -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        out.extend_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1, 0]);
        out.extend_from_slice(&[0u8; 8]); // e_ident[8..16]
        out.extend_from_slice(&3u16.to_le_bytes()); // e_type = ET_DYN
        out.extend_from_slice(&183u16.to_le_bytes()); // e_machine = AArch64
        out.extend_from_slice(&1u32.to_le_bytes()); // e_version
        out.extend_from_slice(&0u64.to_le_bytes()); // e_entry
        out.extend_from_slice(&0u64.to_le_bytes()); // e_phoff
        out.extend_from_slice(&64u64.to_le_bytes()); // e_shoff
        out.extend_from_slice(&0u32.to_le_bytes()); // e_flags
        out.extend_from_slice(&64u16.to_le_bytes()); // e_ehsize
        out.extend_from_slice(&0u16.to_le_bytes()); // e_phentsize
        out.extend_from_slice(&0u16.to_le_bytes()); // e_phnum
        out.extend_from_slice(&64u16.to_le_bytes()); // e_shentsize
        out.extend_from_slice(&3u16.to_le_bytes()); // e_shnum
        out.extend_from_slice(&1u16.to_le_bytes()); // e_shstrndx
        assert_eq!(out.len(), 64);
        // Tabla de nombres: "\0.shstrtab\0.dynsym\0"
        let strtab = b"\0.shstrtab\0.dynsym\0";
        let shstrtab_off = 64 + 3 * 64;
        let dynsym_off = shstrtab_off + strtab.len();
        // sh[0]: null
        out.extend_from_slice(&[0u8; 64]);
        // sh[1]: .shstrtab
        out.extend_from_slice(&1u32.to_le_bytes()); // sh_name
        out.extend_from_slice(&3u32.to_le_bytes()); // sh_type = STRTAB
        out.extend_from_slice(&0u64.to_le_bytes()); // sh_flags
        out.extend_from_slice(&0u64.to_le_bytes()); // sh_addr
        out.extend_from_slice(&(shstrtab_off as u64).to_le_bytes()); // sh_offset
        out.extend_from_slice(&(strtab.len() as u64).to_le_bytes()); // sh_size
        out.extend_from_slice(&0u32.to_le_bytes()); // sh_link
        out.extend_from_slice(&0u32.to_le_bytes()); // sh_info
        out.extend_from_slice(&1u64.to_le_bytes()); // sh_addralign
        out.extend_from_slice(&0u64.to_le_bytes()); // sh_entsize
                                                    // sh[2]: .dynsym (2 símbolos de 24 bytes)
        out.extend_from_slice(&11u32.to_le_bytes()); // sh_name
        out.extend_from_slice(&11u32.to_le_bytes()); // sh_type = DYNSYM
        out.extend_from_slice(&0u64.to_le_bytes()); // sh_flags
        out.extend_from_slice(&0u64.to_le_bytes()); // sh_addr
        out.extend_from_slice(&(dynsym_off as u64).to_le_bytes()); // sh_offset
        out.extend_from_slice(&48u64.to_le_bytes()); // sh_size (2*24)
        out.extend_from_slice(&0u32.to_le_bytes()); // sh_link
        out.extend_from_slice(&1u32.to_le_bytes()); // sh_info
        out.extend_from_slice(&8u64.to_le_bytes()); // sh_addralign
        out.extend_from_slice(&24u64.to_le_bytes()); // sh_entsize
                                                     // Contenido
        out.extend_from_slice(strtab);
        out.extend_from_slice(&[0u8; 48]); // dos símbolos vacíos
        out
    }

    #[test]
    fn el_elf_se_lee_y_dice_si_esta_stripped() {
        let info = parse_elf(&build_min_elf64()).expect("debe leer el ELF");
        assert_eq!(info.class, "ELF64");
        assert_eq!(info.machine, "AArch64");
        assert!(info.stripped);
        assert_eq!(info.exports, 2);
        assert_eq!(info.sections, 3);
        // Basura: no entra en panic.
        assert!(parse_elf(b"no soy un elf").is_none());
        assert!(parse_elf(&[]).is_none());
    }

    #[test]
    fn la_lectura_de_una_apk_que_no_existe_no_entra_en_panic() {
        let a = analyze_apk("/no/existe/esta/apk.apk");
        assert_eq!(a["ok"], json!(false));
        assert_eq!(a["error"], json!("cannot_read_apk"));
    }
}
