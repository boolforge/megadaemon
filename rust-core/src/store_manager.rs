//! Gestor de tiendas / repos universal — «mega store manager», super ligero.
//!
//! Objetivo: que MegaDaemon sirva como punto único de descarga y verificación de
//! *stores* de cualquier tipo (repos deRecovery, módulos Magisk, APKs, payloads
//! de firmware, herramientas) **sin SDK de terceros, sin binarios externos y sin
//! shell scripts**: todo en Rust, con `sha2` para integridad y RSA-PKCS#1-v1.5
//! (PKCS#8/PEM vía la crate `rsa`) para firma de índices.
//!
//! Principios:
//! * El descriptor de una tienda es un JSON plano y versionable.
//! * El índice se **cachea** en el estado local y se verifica antes de usarlo.
//! * Una entrada se instala sólo si su **sha256** coincide; si la tienda declara
//!   clave pública, el índice además debe ir **firmado**.
//! * Nada se ejecuta: este módulo *descarga y verifica*; el instalador de cada
//!   tipo (Magisk, APK…) es del consumidor.
//!
//! API (JSON in / JSON out) estable:
//!   `list` · `add` · `remove` · `refresh` · `search` · `plan` · `install` · `stats`

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use base64ct::{Base64, Encoding};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

/// Tipo de contenido que sirve una tienda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StoreKind {
    /// Repositorio genérico de ficheros (recovery, GApps, firmware…).
    #[default]
    Repo,
    /// Módulos Magisk (ZIP con `module.prop`).
    Magisk,
    /// APKs sueltos.
    Apk,
    /// Payloads de firmware (`payload.bin`).
    Payload,
    /// Binarios de herramienta.
    Tool,
}

impl StoreKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            StoreKind::Repo => "repo",
            StoreKind::Magisk => "magisk",
            StoreKind::Apk => "apk",
            StoreKind::Payload => "payload",
            StoreKind::Tool => "tool",
        }
    }

    /// Subcarpeta de destino dentro del árbol de contenido.
    pub fn dest_subdir(&self) -> &'static str {
        match self {
            StoreKind::Repo => "repo",
            StoreKind::Magisk => "modules",
            StoreKind::Apk => "apk",
            StoreKind::Payload => "payload",
            StoreKind::Tool => "tools",
        }
    }
}

/// Nivel de confianza declarado por una tienda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrustLevel {
    /// Sólo se comprueba el sha256 de cada entrada.
    Sha256,
    /// El índice debe ir firmado con la clave pública de la tienda.
    Signed,
}

impl TrustLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            TrustLevel::Sha256 => "sha256",
            TrustLevel::Signed => "signed",
        }
    }
}

/// Descriptor de una tienda.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreDescriptor {
    /// Identificador estable (ASCII, sin espacios). Se usa como nombre de carpeta.
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: StoreKind,
    /// URL base del índice (se le añade `index_path`).
    pub base_url: String,
    /// Ruta relativa del índice dentro de `base_url`.
    #[serde(default = "default_index_path")]
    pub index_path: String,
    /// Clave pública RSA en base64 de `n` y `e` separados por `:`.
    #[serde(default)]
    pub public_key: Option<String>,
    #[serde(default = "default_trust")]
    pub trust: TrustLevel,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub updated_at: String,
}

fn default_index_path() -> String {
    "index.json".to_string()
}

fn default_trust() -> TrustLevel {
    TrustLevel::Sha256
}

impl StoreDescriptor {
    pub fn new(id: &str, name: &str, base_url: &str, kind: StoreKind) -> Self {
        StoreDescriptor {
            id: id.to_string(),
            name: name.to_string(),
            kind,
            base_url: base_url.trim_end_matches('/').to_string(),
            index_path: default_index_path(),
            public_key: None,
            trust: default_trust(),
            enabled: true,
            updated_at: String::new(),
        }
    }

    /// URL completa del índice.
    pub fn index_url(&self) -> String {
        if self.index_path.starts_with("http://")
            || self.index_path.starts_with("https://")
            || self.base_url.is_empty()
        {
            self.index_path.clone()
        } else {
            format!(
                "{}/{}",
                self.base_url.trim_end_matches('/'),
                self.index_path.trim_start_matches('/')
            )
        }
    }
}

/// Una entrada del índice (un fichero instalable).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    /// URL absoluta o relativa a `base_url`.
    pub url: String,
    /// sha256 en minúsculas (64 hex). Obligatorio: sin él no se instala.
    pub sha256: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub arch: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Índice completo de una tienda (cuerpo + firma opcional).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreIndex {
    pub store_id: String,
    #[serde(default)]
    pub generated_at: String,
    pub entries: Vec<StoreEntry>,
    /// sha256 del cuerpo tal cual se descargó (lo que se firma).
    pub body_sha256: String,
    /// Firma RSA base64 sobre el cuerpo; `None` si la tienda no firma.
    #[serde(default)]
    pub signature: Option<String>,
}

/// Resultado de verificar la confianza de un índice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", tag = "status")]
pub enum TrustOutcome {
    /// Índice sin firma: sólo se valida el sha256 de cada entrada.
    Unsigned { entries: usize },
    /// Firma válida con la clave declarada.
    SignatureValid { entries: usize },
    /// Firma presente pero no verificable con la clave declarada.
    SignatureInvalid { reason: String },
}

/// sha256 en minúsculas de un chunk de datos.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    let d = h.finalize();
    let mut out = String::with_capacity(64);
    for b in d.iter() {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

/// ¿Es una cadena hexadecimal de 64 caracteres (sha256)?
pub fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

// ---------------------------------------------------------------------------
// Registro de tiendas
// ---------------------------------------------------------------------------

/// Registro persistente de tiendas (`stores.json` en el estado local).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoreRegistry {
    #[serde(default)]
    pub stores: BTreeMap<String, StoreDescriptor>,
    #[serde(default)]
    pub schema: u32,
}

impl StoreRegistry {
    pub fn load(state_dir: &Path) -> Result<Self, String> {
        let path = stores_file(state_dir);
        if !path.exists() {
            return Ok(StoreRegistry {
                stores: BTreeMap::new(),
                schema: 1,
            });
        }
        let raw = fs::read_to_string(&path).map_err(|e| format!("leer registro: {e}"))?;
        let mut reg: StoreRegistry =
            serde_json::from_str(&raw).map_err(|e| format!("registro corrupto: {e}"))?;
        if reg.schema == 0 {
            reg.schema = 1;
        }
        Ok(reg)
    }

    pub fn save(&self, state_dir: &Path) -> Result<(), String> {
        let dir = state_dir.join("stores");
        fs::create_dir_all(&dir).map_err(|e| format!("crear {dir:?}: {e}"))?;
        let path = stores_file(state_dir);
        let tmp = path.with_extension("json.tmp");
        let body = serde_json::to_string_pretty(self).map_err(|e| format!("serializar: {e}"))?;
        fs::write(&tmp, body.as_bytes()).map_err(|e| format!("escribir tmp: {e}"))?;
        fs::rename(&tmp, &path).map_err(|e| format!("renombrar: {e}"))
    }

    pub fn upsert(&mut self, desc: StoreDescriptor) -> Result<(), String> {
        validate_id(&desc.id)?;
        if desc.base_url.trim().is_empty() {
            return Err("base_url vacia".to_string());
        }
        self.stores.insert(desc.id.clone(), desc);
        Ok(())
    }

    pub fn remove(&mut self, id: &str) -> bool {
        self.stores.remove(id).is_some()
    }

    pub fn get(&self, id: &str) -> Option<&StoreDescriptor> {
        self.stores.get(id)
    }

    pub fn list(&self) -> Vec<&StoreDescriptor> {
        self.stores.values().collect()
    }
}

fn stores_file(state_dir: &Path) -> PathBuf {
    state_dir.join("stores").join("stores.json")
}

fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("id vacio".to_string());
    }
    if !id
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
    {
        return Err(format!("id con caracteres no permitidos: {id}"));
    }
    if id.contains("..") {
        return Err(format!("id con recorrido de ruta: {id}"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Índice: parseo y verificación
// ---------------------------------------------------------------------------

/// Parsea el cuerpo JSON de un índice y valida cada entrada.
pub fn parse_index(store_id: &str, body: &str) -> Result<StoreIndex, String> {
    let raw: Value = serde_json::from_str(body).map_err(|e| format!("json invalido: {e}"))?;
    let entries_val = raw
        .get("entries")
        .cloned()
        .unwrap_or_else(|| Value::Array(vec![]));
    let arr = entries_val
        .as_array()
        .ok_or_else(|| "entries no es una lista".to_string())?;
    let mut entries = Vec::with_capacity(arr.len());
    for (i, e) in arr.iter().enumerate() {
        let id = e
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("entry-{i}"));
        let url = e
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("entrada {id}: falta url"))?
            .to_string();
        let sha = e
            .get("sha256")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("entrada {id}: falta sha256"))?
            .to_lowercase();
        if !is_sha256_hex(&sha) {
            return Err(format!("entrada {id}: sha256 no es 64 hex"));
        }
        let entry = StoreEntry {
            id,
            name: e
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            version: e
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            url,
            sha256: sha,
            size: e.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
            arch: e
                .get("arch")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            tags: e
                .get("tags")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            extra: e
                .as_object()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|(k, _)| {
                    !matches!(
                        k.as_str(),
                        "id" | "name" | "version" | "url" | "sha256" | "size" | "arch" | "tags"
                    )
                })
                .collect(),
        };
        entries.push(entry);
    }
    Ok(StoreIndex {
        store_id: store_id.to_string(),
        generated_at: raw
            .get("generated_at")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        entries,
        body_sha256: sha256_hex(body.as_bytes()),
        signature: None,
    })
}

/// Verifica la confianza de un índice frente a la clave pública de la tienda.
///
/// La firma es RSA PKCS#1 v1.5 sobre SHA-256 del **cuerpo exacto** del índice.
pub fn verify_index(
    index: &StoreIndex,
    body: &[u8],
    public_key_b64: Option<&str>,
    trust: TrustLevel,
) -> Result<TrustOutcome, String> {
    let n_entries = index.entries.len();
    if trust == TrustLevel::Sha256 {
        return Ok(TrustOutcome::Unsigned {
            entries: n_entries,
        });
    }
    let sig_b64 = index
        .signature
        .as_deref()
        .ok_or_else(|| "tienda declara firma pero el indice no la trae".to_string())?;
    let pk_b64 = public_key_b64
        .ok_or_else(|| "tienda declara firma pero no tiene clave publica".to_string())?;
    let (n, e) = parse_public_key(pk_b64)?;
    let sig = Base64::decode_vec(sig_b64)
        .map_err(|err| format!("firma no es base64: {err}"))?;
    let digest = Sha256::digest(body);
    match rsa_pkcs1v15_sha256_verify(&n, &e, digest.as_slice(), &sig) {
        Ok(true) => Ok(TrustOutcome::SignatureValid {
            entries: n_entries,
        }),
        Ok(false) => Ok(TrustOutcome::SignatureInvalid {
            reason: "firma no valida con la clave declarada".to_string(),
        }),
        Err(err) => Ok(TrustOutcome::SignatureInvalid { reason: err }),
    }
}

/// DigestInfo DER de SHA-256 (RFC 8017, A.2.4) sin la parte ASN.1 de longitudes
/// variables: prefijo fijo +摘要 de 32 bytes.
const SHA256_DIGEST_INFO_PREFIX: [u8; 19] = [
    0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01, 0x05,
    0x00, 0x04, 0x20,
];

/// Verifica una firma RSA PKCS#1 v1.5 sobre SHA-256 **sin dependencias extra**.
///
/// Se implementa a mano (exponenciación modular + comparación del bloque EM)
/// para no depender de la feature `oid` de `sha2` ni tocar `Cargo.toml`.
/// `Ok(true)` = firma válida, `Ok(false)` = firma incorrecta.
pub fn rsa_pkcs1v15_sha256_verify(
    n_bytes: &[u8],
    e_bytes: &[u8],
    digest: &[u8],
    sig: &[u8],
) -> Result<bool, String> {
    use rsa::BigUint;
    if digest.len() != 32 {
        return Err("digest sha256 no son 32 bytes".to_string());
    }
    let n = BigUint::from_bytes_be(n_bytes);
    let e = BigUint::from_bytes_be(e_bytes);
    if n.bits() < 1024 {
        return Err("clave publica demasiado pequena".to_string());
    }
    let k = n.to_bytes_be().len(); // longitud del módulo en bytes
    if sig.len() != k {
        return Ok(false);
    }
    let s = BigUint::from_bytes_be(sig);
    if s >= n {
        return Ok(false);
    }
    let m = s.modpow(&e, &n);
    let mut em = m.to_bytes_be();
    if em.len() < k {
        //_pkcs1v15: el bloque EM se rellena por la izquierda con ceros.
        let mut padded = vec![0u8; k - em.len()];
        padded.extend_from_slice(&em);
        em = padded;
    }
    if em.len() != k {
        return Ok(false);
    }
    // EM = 0x00 || 0x01 || PS (0xFF...) || 0x00 || DigestInfo
    if em[0] != 0x00 || em[1] != 0x01 {
        return Ok(false);
    }
    let mut i = 2;
    while i < em.len() && em[i] == 0xff {
        i += 1;
    }
    if i >= em.len() || em[i] != 0x00 {
        return Ok(false);
    }
    let tail = &em[i + 1..];
    if tail.len() != SHA256_DIGEST_INFO_PREFIX.len() + 32 {
        return Ok(false);
    }
    if tail[..SHA256_DIGEST_INFO_PREFIX.len()] != SHA256_DIGEST_INFO_PREFIX {
        return Ok(false);
    }
    Ok(&tail[SHA256_DIGEST_INFO_PREFIX.len()..] == digest)
}

/// Clave pública RSA en base64 `n:e` → bytes de `n` y `e`.
pub fn parse_public_key(pk_b64: &str) -> Result<(Vec<u8>, Vec<u8>), String> {
    let raw = Base64::decode_vec(pk_b64.trim())
        .map_err(|e| format!("clave publica no es base64: {e}"))?;
    let sep = raw
        .iter()
        .position(|b| *b == b':')
        .ok_or_else(|| "clave publica sin separador ':' (formato base64(n:e))".to_string())?;
    let n = raw[..sep].to_vec();
    let e = raw[sep + 1..].to_vec();
    if n.is_empty() || e.is_empty() {
        return Err("clave publica con componente vacio".to_string());
    }
    Ok((n, e))
}

/// Serializa `n:e` a base64 (inverso de `parse_public_key`).
pub fn encode_public_key(n: &[u8], e: &[u8]) -> String {
    let mut raw = n.to_vec();
    raw.push(b':');
    raw.extend_from_slice(e);
    Base64::encode_string(&raw)
}

// ---------------------------------------------------------------------------
// Búsqueda
// ---------------------------------------------------------------------------

/// Busca entradas por id, nombre, versión o etiqueta (sin distinguir mayúsculas).
pub fn search<'a>(index: &'a StoreIndex, query: &str) -> Vec<&'a StoreEntry> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return index.entries.iter().collect();
    }
    index
        .entries
        .iter()
        .filter(|e| {
            e.id.to_lowercase().contains(&q)
                || e.name.to_lowercase().contains(&q)
                || e.version.to_lowercase().contains(&q)
                || e.tags.iter().any(|t| t.to_lowercase().contains(&q))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Plan de instalación y descarga verificada
// ---------------------------------------------------------------------------

/// Plan (sin ejecutar) de instalar una entrada.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallPlan {
    pub store_id: String,
    pub entry_id: String,
    pub url: String,
    pub dest: String,
    pub sha256: String,
    pub size: u64,
    pub kind: String,
    /// Espacio libre estimado que hace falta (bytes).
    pub needs_bytes: u64,
    /// true si la entrada es un módulo Magisk (ZIP con module.prop).
    pub magisk_module: bool,
}

/// Calcula el plan de instalación. Rechaza recorridos de ruta.
pub fn install_plan(
    desc: &StoreDescriptor,
    entry: &StoreEntry,
    content_root: &Path,
) -> Result<InstallPlan, String> {
    if !is_sha256_hex(&entry.sha256) {
        return Err(format!("entrada {}: sha256 invalido", entry.id));
    }
    let url = if entry.url.starts_with("http://") || entry.url.starts_with("https://") {
        entry.url.clone()
    } else {
        format!(
            "{}/{}",
            desc.base_url.trim_end_matches('/'),
            entry.url.trim_start_matches('/')
        )
    };
    let fname = sanitize_filename(&entry.id);
    let fname = if fname.contains('.') {
        fname
    } else {
        match desc.kind {
            StoreKind::Apk => format!("{fname}.apk"),
            StoreKind::Magisk | StoreKind::Repo => format!("{fname}.zip"),
            StoreKind::Payload => format!("{fname}.payload"),
            StoreKind::Tool => fname,
        }
    };
    let dest = content_root
        .join(desc.kind.dest_subdir())
        .join(&fname)
        .to_string_lossy()
        .to_string();
    // Defensa extra: el destino no puede salir del content_root.
    let dest_path = Path::new(&dest);
    if !dest_path.starts_with(content_root) {
        return Err("destino fuera del arbol de contenido".to_string());
    }
    Ok(InstallPlan {
        store_id: desc.id.clone(),
        entry_id: entry.id.clone(),
        url,
        dest,
        sha256: entry.sha256.clone(),
        size: entry.size,
        kind: desc.kind.as_str().to_string(),
        needs_bytes: entry.size.saturating_add(1024 * 1024),
        magisk_module: desc.kind == StoreKind::Magisk,
    })
}

/// Deja sólo `[A-Za-z0-9._-]`, sin `..`.
pub fn sanitize_filename(raw: &str) -> String {
    let mut out: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    while out.contains("..") {
        out = out.replace("..", ".");
    }
    if out.is_empty() {
        out = "entrada".to_string();
    }
    out
}

/// Informe de una descarga verificada.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadReport {
    pub dest: String,
    pub bytes: u64,
    pub sha256: String,
    pub verified: bool,
}

/// Descarga una URL a disco calculando el sha256 **mientras** escribe y
/// **borra el fichero** si el hash no coincide.
pub fn download_verified(
    url: &str,
    expected_sha256: &str,
    dest: &Path,
    timeout_ms: u64,
) -> Result<DownloadReport, String> {
    let expected = expected_sha256.to_lowercase();
    if !is_sha256_hex(&expected) {
        return Err("sha256 esperado invalido".to_string());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("crear {:?}: {e}", parent))?;
    }
    let tmp = dest.with_extension("part");
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_millis(timeout_ms)))
        .build()
        .new_agent();
    let resp = agent
        .get(url)
        .call()
        .map_err(|e| format!("descargar {url}: {e}"))?;
    let mut reader = resp.into_body().into_reader();
    let mut file = fs::File::create(&tmp).map_err(|e| format!("crear {:?}: {e}", tmp))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("leer {url}: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        total += n as u64;
        use std::io::Write;
        file.write_all(&buf[..n])
            .map_err(|e| format!("escribir {:?}: {e}", tmp))?;
    }
    let got = hex_digest(hasher.finalize());
    if got != expected {
        let _ = fs::remove_file(&tmp);
        return Err(format!("sha256 incorrecto: esperaba {expected}, obtuve {got}"));
    }
    drop(file);
    fs::rename(&tmp, dest).map_err(|e| format!("renombrar a {dest:?}: {e}"))?;
    Ok(DownloadReport {
        dest: dest.to_string_lossy().to_string(),
        bytes: total,
        sha256: got,
        verified: true,
    })
}

fn hex_digest(d: impl AsRef<[u8]>) -> String {
    let mut out = String::with_capacity(64);
    for b in d.as_ref() {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

/// Lee el `module.prop` de un ZIP de módulo Magisk (verifica que lo sea).
pub fn read_magisk_module_prop(zip_path: &Path) -> Result<String, String> {
    let file = fs::File::open(zip_path).map_err(|e| format!("abrir zip: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("zip invalido: {e}"))?;
    for name in ["module.prop", "META-INF/com/google/android/update-binary"] {
        if let Ok(mut f) = archive.by_name(name) {
            let mut s = String::new();
            if name == "module.prop" {
                use std::io::Read;
                f.read_to_string(&mut s).ok();
                return Ok(s);
            }
        }
    }
    Err("no parece un modulo Magisk (falta module.prop)".to_string())
}

// ---------------------------------------------------------------------------
// Caché de índices en disco
// ---------------------------------------------------------------------------

/// Guarda el índice cacheado de una tienda.
pub fn cache_index(state_dir: &Path, index: &StoreIndex) -> Result<String, String> {
    let dir = state_dir.join("stores").join("index");
    fs::create_dir_all(&dir).map_err(|e| format!("crear cache: {e}"))?;
    let path = dir.join(format!("{}.json", sanitize_filename(&index.store_id)));
    let body = serde_json::to_string_pretty(index).map_err(|e| format!("serializar: {e}"))?;
    fs::write(&path, body.as_bytes()).map_err(|e| format!("escribir cache: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

/// Lee el índice cacheado de una tienda.
pub fn load_cached_index(state_dir: &Path, store_id: &str) -> Result<StoreIndex, String> {
    let path = state_dir
        .join("stores")
        .join("index")
        .join(format!("{}.json", sanitize_filename(store_id)));
    let raw = fs::read_to_string(&path).map_err(|e| format!("leer cache: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("cache corrupto: {e}"))
}

// ---------------------------------------------------------------------------
// API
// ---------------------------------------------------------------------------

/// Descarga el índice de una tienda, verifica su firma y lo cachea.
pub fn refresh_store(
    desc: &StoreDescriptor,
    state_dir: &Path,
    timeout_ms: u64,
) -> Result<(StoreIndex, TrustOutcome), String> {
    let url = desc.index_url();
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_millis(timeout_ms)))
        .build()
        .new_agent();
    let resp = agent
        .get(&url)
        .call()
        .map_err(|e| format!("indice {url}: {e}"))?;
    let body = resp
        .into_body()
        .read_to_string()
        .map_err(|e| format!("leer indice {url}: {e}"))?;
    let mut index = parse_index(&desc.id, &body)?;
    // La firma puede venir en una cabecera o dentro del propio JSON.
    let sig_from_json = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|v| v.get("signature").and_then(|s| s.as_str()).map(String::from));
    index.signature = sig_from_json;
    let outcome = verify_index(
        &index,
        body.as_bytes(),
        desc.public_key.as_deref(),
        desc.trust,
    )?;
    if let TrustOutcome::SignatureInvalid { reason } = &outcome {
        return Err(format!("indice {url}: {reason}"));
    }
    cache_index(state_dir, &index)?;
    Ok((index, outcome))
}

/// Punto de entrada JSON de la API (`action` + `args`).
pub fn handle_action(action: &str, args: &Value, state_dir: &Path) -> Value {
    let reg_path = state_dir.join("stores").join("stores.json");
    let mut reg = match StoreRegistry::load(state_dir) {
        Ok(r) => r,
        Err(e) => return json!({ "ok": false, "error": e }),
    };
    match action {
        "list" => {
            let stores: Vec<Value> = reg
                .list()
                .into_iter()
                .map(|d| {
                    let cached = load_cached_index(state_dir, &d.id).ok();
                    json!({
                        "id": d.id,
                        "name": d.name,
                        "kind": d.kind.as_str(),
                        "base_url": d.base_url,
                        "trust": d.trust.as_str(),
                        "enabled": d.enabled,
                        "public_key": d.public_key.is_some(),
                        "updated_at": d.updated_at,
                        "entries": cached.as_ref().map(|i| i.entries.len()).unwrap_or(0),
                    })
                })
                .collect();
            json!({ "ok": true, "count": stores.len(), "stores": stores })
        }
        "add" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let name = args.get("name").and_then(|v| v.as_str()).unwrap_or(id);
            let base_url = args.get("base_url").and_then(|v| v.as_str()).unwrap_or("");
            let kind = match args.get("kind").and_then(|v| v.as_str()).unwrap_or("repo") {
                "magisk" => StoreKind::Magisk,
                "apk" => StoreKind::Apk,
                "payload" => StoreKind::Payload,
                "tool" => StoreKind::Tool,
                _ => StoreKind::Repo,
            };
            let mut d = StoreDescriptor::new(id, name, base_url, kind);
            if let Some(p) = args.get("index_path").and_then(|v| v.as_str()) {
                d.index_path = p.to_string();
            }
            if let Some(p) = args.get("public_key").and_then(|v| v.as_str()) {
                d.public_key = Some(p.to_string());
            }
            if let Some(t) = args.get("trust").and_then(|v| v.as_str()) {
                d.trust = if t == "signed" {
                    TrustLevel::Signed
                } else {
                    TrustLevel::Sha256
                };
            }
            match reg.upsert(d.clone()) {
                Ok(()) => match reg.save(state_dir) {
                    Ok(()) => json!({ "ok": true, "id": id, "path": reg_path.to_string_lossy() }),
                    Err(e) => json!({ "ok": false, "error": e }),
                },
                Err(e) => json!({ "ok": false, "error": e }),
            }
        }
        "remove" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let removed = reg.remove(id);
            let _ = reg.save(state_dir);
            json!({ "ok": removed, "removed": removed, "id": id })
        }
        "refresh" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let timeout = args
                .get("timeout_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(30_000);
            let targets: Vec<StoreDescriptor> = if id.is_empty() {
                reg.list().into_iter().cloned().collect()
            } else {
                match reg.get(id) {
                    Some(d) => vec![d.clone()],
                    None => return json!({ "ok": false, "error": format!("tienda desconocida: {id}") }),
                }
            };
            let mut results = Vec::new();
            let mut any_err = false;
            for d in targets {
                match refresh_store(&d, state_dir, timeout) {
                    Ok((idx, outcome)) => {
                        let mut dd = d.clone();
                        dd.updated_at = idx.generated_at.clone();
                        let _ = reg.upsert(dd);
                        results.push(json!({
                            "id": d.id,
                            "entries": idx.entries.len(),
                            "trust": outcome,
                            "body_sha256": idx.body_sha256,
                        }));
                    }
                    Err(e) => {
                        any_err = true;
                        results.push(json!({ "id": d.id, "error": e }));
                    }
                }
            }
            let _ = reg.save(state_dir);
            json!({ "ok": !any_err, "results": results })
        }
        "search" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let q = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            match load_cached_index(state_dir, id) {
                Ok(idx) => {
                    let hits = search(&idx, q);
                    json!({
                        "ok": true,
                        "store": id,
                        "query": q,
                        "count": hits.len(),
                        "entries": hits,
                    })
                }
                Err(e) => json!({ "ok": false, "error": e }),
            }
        }
        "plan" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let entry_id = args.get("entry").and_then(|v| v.as_str()).unwrap_or("");
            let root = content_root(state_dir);
            match (reg.get(id), load_cached_index(state_dir, id)) {
                (Some(d), Ok(idx)) => match idx.entries.iter().find(|e| e.id == entry_id) {
                    Some(e) => match install_plan(d, e, &root) {
                        Ok(p) => json!({ "ok": true, "plan": p }),
                        Err(err) => json!({ "ok": false, "error": err }),
                    },
                    None => json!({ "ok": false, "error": format!("entrada no encontrada: {entry_id}") }),
                },
                (None, _) => json!({ "ok": false, "error": format!("tienda desconocida: {id}") }),
                (_, Err(e)) => json!({ "ok": false, "error": e }),
            }
        }
        "install" => {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let entry_id = args.get("entry").and_then(|v| v.as_str()).unwrap_or("");
            if args.get("confirm").and_then(|v| v.as_bool()) != Some(true) {
                return json!({ "ok": false, "error": "instalar exige confirm:true" });
            }
            let timeout = args
                .get("timeout_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(300_000);
            let root = content_root(state_dir);
            let (d, idx) = match (reg.get(id), load_cached_index(state_dir, id)) {
                (Some(d), Ok(i)) => (d.clone(), i),
                (None, _) => return json!({ "ok": false, "error": format!("tienda desconocida: {id}") }),
                (_, Err(e)) => return json!({ "ok": false, "error": e }),
            };
            let entry = match idx.entries.iter().find(|e| e.id == entry_id) {
                Some(e) => e.clone(),
                None => {
                    return json!({ "ok": false, "error": format!("entrada no encontrada: {entry_id}") })
                }
            };
            let plan = match install_plan(&d, &entry, &root) {
                Ok(p) => p,
                Err(err) => return json!({ "ok": false, "error": err }),
            };
            let dest = Path::new(&plan.dest);
            match download_verified(&plan.url, &plan.sha256, dest, timeout) {
                Ok(rep) => {
                    let mut out = json!({ "ok": true, "report": rep, "plan": plan });
                    if plan.magisk_module {
                        let mp = read_magisk_module_prop(dest).ok();
                        out["module_prop"] = mp.map(Value::String).unwrap_or(Value::Null);
                    }
                    out
                }
                Err(e) => json!({ "ok": false, "error": e }),
            }
        }
        "stats" => {
            let total_entries: usize = reg
                .list()
                .into_iter()
                .filter_map(|d| load_cached_index(state_dir, &d.id).ok())
                .map(|i| i.entries.len())
                .sum();
            let content = content_root(state_dir);
            let bytes = dir_size(&content);
            json!({
                "ok": true,
                "stores": reg.list().len(),
                "cached_entries": total_entries,
                "content_root": content.to_string_lossy(),
                "content_bytes": bytes,
                "registry": reg_path.to_string_lossy(),
            })
        }
        other => json!({ "ok": false, "error": format!("accion desconocida: {other}") }),
    }
}

/// Raíz del árbol de contenido descargado.
pub fn content_root(state_dir: &Path) -> PathBuf {
    state_dir.join("content")
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(p) = stack.pop() {
        let rd = match fs::read_dir(&p) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for e in rd.flatten() {
            match e.file_type() {
                Ok(ft) if ft.is_dir() => stack.push(e.path()),
                Ok(ft) if ft.is_file() => {
                    if let Ok(md) = e.metadata() {
                        total += md.len();
                    }
                }
                _ => {}
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("md_store_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    const INDEX_JSON: &str = r#"{
      "generated_at": "2026-11-20T00:00:00Z",
      "entries": [
        {"id":"magisk-play-integrity-fix","name":"Play Integrity Fix","version":"v16","url":"files/pif.zip","sha256":"1111111111111111111111111111111111111111111111111111111111111111","size":10240,"tags":["magisk"]},
        {"id":"lsposed","name":"LSPosed","version":"1.9.2","url":"files/lsposed.zip","sha256":"2222222222222222222222222222222222222222222222222222222222222222","size":20480,"arch":"arm64"}
      ]
    }"#;

    #[test]
    fn parse_index_valida_campos_obligatorios() {
        let idx = parse_index("demo", INDEX_JSON).unwrap();
        assert_eq!(idx.entries.len(), 2);
        assert_eq!(idx.store_id, "demo");
        assert_eq!(idx.body_sha256.len(), 64);
        assert_eq!(idx.entries[1].arch.as_deref(), Some("arm64"));
        // sha256 no hexadecimal -> error
        let bad = r#"{"entries":[{"id":"x","url":"u","sha256":"zz"}]}"#;
        assert!(parse_index("demo", bad).is_err());
        // falta url -> error
        let bad2 = r#"{"entries":[{"id":"x","sha256":"1111111111111111111111111111111111111111111111111111111111111111"}]}"#;
        assert!(parse_index("demo", bad2).is_err());
    }

    #[test]
    fn busqueda_cubre_id_nombre_version_y_tags() {
        let idx = parse_index("demo", INDEX_JSON).unwrap();
        assert_eq!(search(&idx, "lsposed").len(), 1);
        assert_eq!(search(&idx, "PLAY").len(), 1); // sin distinguir mayúsculas
        assert_eq!(search(&idx, "1.9.2").len(), 1);
        assert_eq!(search(&idx, "magisk").len(), 1);
        assert_eq!(search(&idx, "").len(), 2);
        assert_eq!(search(&idx, "no-existe").len(), 0);
    }

    #[test]
    fn registro_persiste_y_lista() {
        let d = tmpdir("reg");
        let mut reg = StoreRegistry::load(&d).unwrap();
        assert_eq!(reg.list().len(), 0);
        reg.upsert(StoreDescriptor::new(
            "demo",
            "Demo",
            "https://example.invalid/",
            StoreKind::Magisk,
        ))
        .unwrap();
        reg.save(&d).unwrap();
        let mut reg2 = StoreRegistry::load(&d).unwrap();
        assert_eq!(reg2.list().len(), 1);
        assert_eq!(reg2.get("demo").unwrap().index_url(), "https://example.invalid/index.json");
        assert!(reg2.remove("demo"));
        assert!(!reg2.remove("demo"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn ids_invalidos_se_rechazan() {
        let mut reg = StoreRegistry::default();
        let mut d = StoreDescriptor::new("../evil", "x", "https://e.invalid", StoreKind::Repo);
        assert!(reg.upsert(d.clone()).is_err());
        d.id = "con espacio".to_string();
        assert!(reg.upsert(d).is_err());
    }

    #[test]
    fn plan_de_instalacion_calcula_destino_y_evita_recorridos() {
        let d = tmpdir("plan");
        let desc = StoreDescriptor::new("demo", "Demo", "https://example.invalid", StoreKind::Magisk);
        let idx = parse_index("demo", INDEX_JSON).unwrap();
        let p = install_plan(&desc, &idx.entries[0], &d).unwrap();
        assert!(p.dest.ends_with("modules/magisk-play-integrity-fix.zip"));
        assert_eq!(p.url, "https://example.invalid/files/pif.zip");
        assert!(p.magisk_module);
        assert!(p.needs_bytes >= 10240);
        // el destino nunca sale del árbol
        assert!(Path::new(&p.dest).starts_with(&d));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn sanitize_filename_no_deja_recorridos() {
        // Contrato: NUNCA puede quedar ".." ni separadores ni espacios.
        for raw in ["../../etc/passwd", "a b/c", "..", "x/../../y", r"con\barra"] {
            let s = sanitize_filename(raw);
            assert!(!s.contains(".."), "recorrido sobrevivido en {raw}: {s}");
            assert!(!s.contains('/'), "separador sobrevivido en {raw}: {s}");
                        assert!(!s.contains("\\"), "barra invertida sobrevivida en {raw}: {s}");
            assert!(!s.contains(' '), "espacio sobrevivido en {raw}: {s}");
        }
        assert_eq!(sanitize_filename(""), "entrada");
        assert_eq!(sanitize_filename("ok-name_1.zip"), "ok-name_1.zip");
    }

    /// Fixture real: RSA-2048 generada con OpenSSL, indice firmado con
    /// `openssl dgst -sha256 -sign` (PKCS#1 v1.5).  Reproducible con
    /// `tools/store_fixtures/` (script `store_manager_fixture.sh`).
    const FIXTURE_BODY: &str = r#"{"generated_at":"2026-11-20T00:00:00Z","entries":[{"id":"lsposed","name":"LSPosed","version":"1.9.2","url":"files/lsposed.zip","sha256":"2222222222222222222222222222222222222222222222222222222222222222","size":20480}]}"#;
    const FIXTURE_PUBKEY: &str = "wPvS4fmDY/A8v7jegBNZfa2x/uWSmaFjvhwGfkk2HrOrddy350ID/qVRk77TkHSEdd635OQotweO6G8MKnh3MsoLn6zgQuLKTIgzeuDyjSADgi/cxSc2dTv4WKeDgML1Fc4XRG2TXzGKn/4noMJVsQWg0OSioT43Dm+YnlZTqH5BT8CIMAOXHSLPT+8AGOOt7nh/c+xCLkDp5VnKz69ZXhVf51hJfhAK5sgeZA6GYa6zaDmD/YeOkl6hIhbYSJqrhq2nbKf0vzTP6zs0JcXLEKt+36LlnLWKgLQs1iCM31uLj8U/CTq3/YW8yHcvkfFbF6+C+KWG0gBGY9rjtcEJNToBAAE=";
    const FIXTURE_SIG: &str = "DDwYuUPscB7G7+jsESEHwONeaFTxv/5yyHQP7IR+4qiUuRFuDULrWsGkI8Ut5Pp0dDWvx4Ui4bgHsiaTxyLyvbmRaGAQyC2W75zWL93C4ZDaSA0Zj2dSOhvPbZIQTBnj7103dAlQCU5i8NUMjjjNwZ+8y328YZaqokDM1Txcuf07itMdqxwLyq8yeKTwbRptP4fuk6sfbIOUGjSmIO6MSkhPkc8f4ihKb6Q7XppsYLf3hViAtwjyGkJimN6jrwDmCuokP+mZVsuvBJ6NK34swfVBTCVRa5bVU7mTwFST1smbwoKhcDoXvZsIR5egAePBrf2oJfQFGkseNHI23lp17g==";

    #[test]
    fn indice_firmado_por_rsa_se_verifica() {
        let mut idx = parse_index("fixture", FIXTURE_BODY).unwrap();
        idx.signature = Some(FIXTURE_SIG.to_string());
        let out = verify_index(
            &idx,
            FIXTURE_BODY.as_bytes(),
            Some(FIXTURE_PUBKEY),
            TrustLevel::Signed,
        )
        .unwrap();
        assert_eq!(out, TrustOutcome::SignatureValid { entries: 1 });
    }

    #[test]
    fn firma_manipulada_o_clave_ajena_se_rechazan() {
        let mut idx = parse_index("fixture", FIXTURE_BODY).unwrap();
        idx.signature = Some(FIXTURE_SIG.to_string());
        // Cuerpo manipulado (misma firma) -> inválida.
        let tampered = FIXTURE_BODY.replace("1.9.2", "1.9.3");
        let out = verify_index(
            &idx,
            tampered.as_bytes(),
            Some(FIXTURE_PUBKEY),
            TrustLevel::Signed,
        )
        .unwrap();
        assert!(matches!(out, TrustOutcome::SignatureInvalid { .. }));
        // Firma alterada -> inválida.
        let mut idx2 = idx.clone();
        idx2.signature = Some("A".repeat(344));
        let out2 = verify_index(
            &idx2,
            FIXTURE_BODY.as_bytes(),
            Some(FIXTURE_PUBKEY),
            TrustLevel::Signed,
        )
        .unwrap();
        assert!(matches!(out2, TrustOutcome::SignatureInvalid { .. }));
        // Clave pública ajena -> inválida.
        let (n2, e2) = parse_public_key(FIXTURE_PUBKEY).unwrap();
        let mut n3 = n2.clone();
        n3[0] ^= 0xff;
        let otra = encode_public_key(&n3, &e2);
        let out3 = verify_index(
            &idx,
            FIXTURE_BODY.as_bytes(),
            Some(&otra),
            TrustLevel::Signed,
        )
        .unwrap();
        assert!(matches!(out3, TrustOutcome::SignatureInvalid { .. }));
    }

    #[test]
    fn tienda_signed_sin_firma_es_error() {
        let idx = parse_index("fixture", FIXTURE_BODY).unwrap();
        let err = verify_index(&idx, FIXTURE_BODY.as_bytes(), Some(FIXTURE_PUBKEY), TrustLevel::Signed)
            .unwrap_err();
        assert!(err.contains("firma"));
    }

    #[test]
    fn descarga_verificada_rechaza_hash_incorrecto_y_limpia() {
        let d = tmpdir("dl");
        let dest = d.join("sub").join("f.bin");
        let err = download_verified(
            "http://127.0.0.1:1/nada",
            &"a".repeat(64),
            &dest,
            1500,
        );
        assert!(err.is_err());
        assert!(!dest.exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn api_list_add_search_stats() {
        let d = tmpdir("api");
        let out = handle_action("add", &json!({"id":"demo","name":"Demo","base_url":"https://example.invalid","kind":"magisk"}), &d);
        assert_eq!(out["ok"], json!(true));
        let list = handle_action("list", &json!({}), &d);
        assert_eq!(list["count"], json!(1));
        // search sin índice cacheado -> error honesto
        let s = handle_action("search", &json!({"id":"demo","query":"x"}), &d);
        assert_eq!(s["ok"], json!(false));
        // install exige confirm
        let i = handle_action("install", &json!({"id":"demo","entry":"x"}), &d);
        assert_eq!(i["ok"], json!(false));
        let st = handle_action("stats", &json!({}), &d);
        assert_eq!(st["stores"], json!(1));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn cache_de_indice_ida_y_vuelta() {
        let d = tmpdir("cache");
        let idx = parse_index("demo", INDEX_JSON).unwrap();
        cache_index(&d, &idx).unwrap();
        let back = load_cached_index(&d, "demo").unwrap();
        assert_eq!(back.entries.len(), 2);
        assert_eq!(back.body_sha256, idx.body_sha256);
        let _ = fs::remove_dir_all(&d);
    }
}
