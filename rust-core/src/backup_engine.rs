//! **Motor de backup nativo** de MegaDaemon.
//!
//! Unifica lo que antes era un monta de piezas sueltas: un **plan** que
//! decide qué entra y por qué, una **captura** que produce un contenedor
//! **reproducible y verificable** (TAR + zstd + MANIFEST), una **verificación**
//! que recalcula los hashes, un **restore selectivo** con *dry-run* obligatorio
//! y **rollback**, una **subida resumible** a Drive y un **planificador** que
//! sólo dispara cuando la carga y la batería lo permiten.
//!
//! ## El contenedor propio
//!
//! ```
//! +--------+  512 B  cabecera de tamaño FIJO (JSON + relleno)
//! | MDBK1 |  payload: TAR(zstd)  [+ AEGIS-256 por trozos]
//! +--------+
//! ```
//!
//! * La cabecera es **fija** ⇒ se puede *parchear* después de escribir el
//!   payload en streaming ⇒ **no hace falta una copia temporal** del backup
//!   (imposible con 5 GB libres en el móvil).
//! * El **MANIFEST** (SHA-256 + tamaño + ruta + mtime por entrada) viaja
//!   **dentro** del contenedor ⇒ `verify` puede rehacerlo todo y detectar
//!   corrupción.
//! * El cifrado es **AEGIS-256 por trozos**: cada trozo ata su nonce y su
//!   AAD a su posición y a su entrada.
//!
//! ## Interoperabilidad
//!
//! El contenedor **propio** no es de Swift Backup (ni pretende serlo): usa
//! magic y etiquetas distintos (`MDBK1`) para que nunca se confundan. El
//! camino de interoperabilidad con Swift Backup vive en
//! [`crate::backup_crypto::read_swift_backup`] y está marcado **EXPERIMENTAL**
//! (el layout byte a byte de su info page no está confirmado).

use crate::backup_crypto::{
    build_tar_from_dir, hex, key_check, sha256_file, unhex, ChunkCipher, ContainerHeader,
    KdfMethod, KdfParams, Manifest, ManifestEntry, FLAG_ENCRYPTED, LABEL_KEY_CHECK,
    AEGIS_KEY_LEN, CONTAINER_HEADER_SIZE, CONTAINER_VERSION, DEFAULT_CHUNK_SIZE,
};
use crate::backup_formats::{safe_relative_path, TarStream};
use crate::drive_client::{
    chunk_plan, open_upload_session, upload_chunk_request, DriveChunkRange, DriveCredentials,
    DriveRequest, DriveTarget,
};
use crate::task_scheduler::Resources;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::Path;

/// Ruta del manifiesto dentro del tar del contenedor.
pub const MANIFEST_ENTRY: &str = "manifest.json";
/// Prefijo de los datos dentro del tar del contenedor.
pub const PAYLOAD_PREFIX: &str = "payload/";

// ---------------------------------------------------------------------------
// Política
// ---------------------------------------------------------------------------

/// De dónde sale un dato.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupSource {
    /// El APK instalado.
    Apk,
    /// Datos privados de la app (`/data/user/0/<pkg>`).
    Data,
    /// Datos externos (`/sdcard/Android/data/<pkg>`).
    External,
    /// Multimedia del usuario (fotos, vídeos).
    Media,
}

impl BackupSource {
    /// Nombre corto y estable.
    pub fn as_str(self) -> &'static str {
        match self {
            BackupSource::Apk => "apk",
            BackupSource::Data => "data",
            BackupSource::External => "external",
            BackupSource::Media => "media",
        }
    }
}

/// Qué clase de fichero es (para el informe del plan).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryKind {
    /// El APK.
    Apk,
    /// `shared_prefs` (preferencias).
    SharedPrefs,
    /// Base de datos (SQLite).
    Database,
    /// Caché (regenerable).
    Cache,
    /// Cualquier otro fichero.
    File,
}

impl EntryKind {
    /// Nombre corto y estable.
    pub fn as_str(self) -> &'static str {
        match self {
            EntryKind::Apk => "apk",
            EntryKind::SharedPrefs => "shared_prefs",
            EntryKind::Database => "database",
            EntryKind::Cache => "cache",
            EntryKind::File => "file",
        }
    }
}

/// Las reglas de qué entra en el backup.
///
/// Todo es parametrizable: sin excepciones hardcodeadas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackupPolicy {
    /// Lista blanca de paquetes (vacía = todos).
    pub include_packages: Vec<String>,
    /// Paquetes excluidos (gana sobre la lista blanca).
    pub exclude_packages: Vec<String>,
    /// Patrones de ruta excluidos. `*` es un comodín simple.
    pub exclude_patterns: Vec<String>,
    /// Incluir cachés y `code_cache` (regenerables: fuera por defecto).
    pub include_cache: bool,
    /// Incluir el APK de la app.
    pub include_apk: bool,
    /// Incluir los datos privados de la app.
    pub include_data: bool,
    /// Incluir `/sdcard/Android/data/<pkg>`.
    pub include_external: bool,
    /// Incluir fotos y vídeos (enorme: fuera por defecto).
    pub include_media: bool,
    /// Tope total en bytes (0 = sin tope).
    pub max_total_bytes: u64,
    /// Tope por fichero en bytes (0 = sin tope).
    pub max_entry_bytes: u64,
    /// Tope de paquetes (0 = sin tope).
    pub max_packages: usize,
}

impl Default for BackupPolicy {
    fn default() -> Self {
        BackupPolicy {
            include_packages: Vec::new(),
            exclude_packages: Vec::new(),
            exclude_patterns: vec![
                "/cache/".to_string(),
                "/code_cache/".to_string(),
                "/no_backup/".to_string(),
                "/lib/".to_string(),
            ],
            include_cache: false,
            include_apk: true,
            include_data: true,
            include_external: true,
            include_media: false,
            max_total_bytes: 0,
            max_entry_bytes: 0,
            max_packages: 0,
        }
    }
}

impl BackupPolicy {
    /// Política "sólo APK".
    pub fn apk_only() -> BackupPolicy {
        BackupPolicy {
            include_data: false,
            include_external: false,
            include_media: false,
            ..BackupPolicy::default()
        }
    }

    /// Política "sólo datos" (sin APK).
    pub fn data_only() -> BackupPolicy {
        BackupPolicy {
            include_apk: false,
            ..BackupPolicy::default()
        }
    }

    /// ¿La ruta casa con alguno de los patrones excluidos?
    pub fn matches_exclusion(&self, path: &str) -> bool {
        self.exclude_patterns.iter().any(|p| glob_match(p, path))
    }
}

// La función de comodín vive en `crate::backup_glob` (módulo sin
// dependencias) ⇒ aquí se re-exporta para que la API pública de este módulo
// NO cambie. Ver `backup_glob.rs` para el porqué.
pub use crate::backup_glob::glob_match;

// ---------------------------------------------------------------------------
// El plan
// ---------------------------------------------------------------------------

/// Una entrada del plan: un fichero o directorio candidato.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanEntry {
    /// Paquete al que pertenece.
    pub package: String,
    /// De dónde sale.
    pub source: BackupSource,
    /// Qué clase de fichero es.
    pub kind: EntryKind,
    /// Ruta **absoluta** en el dispositivo.
    pub path: String,
    /// Ruta **dentro** del contenedor.
    pub rel_path: String,
    /// Tamaño en bytes.
    pub size: u64,
    /// mtime (epoch).
    pub mtime: u64,
    /// ¿Entra en el backup?
    pub included: bool,
    /// **Por qué**: la razón de incluirlo o de excluirlo.
    pub reason: String,
}

/// El plan completo: qué se va a respaldar y por qué.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackupPlan {
    /// La política aplicada.
    pub policy: BackupPolicy,
    /// Todas las entradas candidatas.
    pub entries: Vec<PlanEntry>,
    /// Paquetes considerados.
    pub packages: usize,
    /// Bytes totales vistos.
    pub total_bytes: u64,
    /// Bytes que entran.
    pub included_bytes: u64,
    /// Bytes que se quedan fuera.
    pub excluded_bytes: u64,
    /// ¿Se cortó algo por los topes?
    pub truncated: bool,
}

impl BackupPlan {
    /// Las entradas que entran.
    pub fn included(&self) -> impl Iterator<Item = &PlanEntry> {
        self.entries.iter().filter(|e| e.included)
    }

    /// Informe en JSON.
    pub fn to_json(&self) -> Value {
        json!({
            "ok": true,
            "packages": self.packages,
            "entries": self.entries.len(),
            "included_entries": self.included().count(),
            "total_bytes": self.total_bytes,
            "included_bytes": self.included_bytes,
            "excluded_bytes": self.excluded_bytes,
            "truncated": self.truncated,
            "policy": self.policy,
        })
    }
}

/// Motivos de exclusión (constantes: el informe dice **por qué**).
pub mod reason {
    /// Entra: datos privados de la app.
    pub const DATA: &str = "datos privados de la app";
    /// Entra: APK instalado.
    pub const APK: &str = "APK instalado";
    /// Entra: datos externos de la app.
    pub const EXTERNAL: &str = "datos externos de la app";
    /// Entra: multimedia del usuario.
    pub const MEDIA: &str = "multimedia del usuario";
    /// Fuera: paquete excluido por la lista de exclusión.
    pub const EXCLUDED_PACKAGE: &str = "paquete excluido por política";
    /// Fuera: la app no está en la lista blanca.
    pub const NOT_WHITELISTED: &str = "paquete no está en la lista blanca";
    /// Fuera: caché regenerable.
    pub const CACHE: &str = "caché regenerable: excluida por política";
    /// Fuera: el tipo de dato está desactivado.
    pub const SOURCE_DISABLED: &str = "tipo de dato desactivado por política";
    /// Fuera: patrón de ruta.
    pub const PATTERN: &str = "excluido por patrón de ruta";
    /// Fuera: supera el tope por fichero.
    pub const TOO_BIG: &str = "supera el tope por fichero";
    /// Fuera: no cabe en el tope total.
    pub const NO_ROOM: &str = "no cabe en el tope total de bytes";
    /// Fuera: se agotó el tope de paquetes.
    pub const TOO_MANY_PACKAGES: &str = "se agotó el tope de paquetes";
}

/// Construye el **plan** de backup.
///
/// `root` es la raíz del dispositivo (`/` en el móvil; un directorio de
/// pruebas en los tests). Todo lo que se decide queda anotado con su
/// **motivo**: un plan que no explica sus exclusiones no es auditable.
pub fn plan(root: &str, policy: &BackupPolicy) -> BackupPlan {
    let mut entries: Vec<PlanEntry> = Vec::new();
    let mut packages: Vec<String> = Vec::new();
    let mut truncated = false;

    // 1) Datos privados + APK.
    let data_root = first_existing(&[
        &format!("{root}/data/user/0"),
        &format!("{root}/data/data"),
    ]);
    let mut package_dirs: Vec<(String, std::path::PathBuf)> = Vec::new();
    if let Some(dr) = &data_root {
        if let Ok(rd) = std::fs::read_dir(dr) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue;
                }
                if e.path().is_dir() {
                    package_dirs.push((name.clone(), e.path()));
                }
            }
        }
    }
    package_dirs.sort_by(|a, b| a.0.cmp(&b.0));

    // Paquetes que sólo tienen APK (sin directorio de datos).
    let apk_root = format!("{root}/data/app");
    let mut apk_dirs: Vec<(String, std::path::PathBuf)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&apk_root) {
        for e in rd.flatten() {
            if !e.path().is_dir() {
                continue;
            }
            let pkg = package_from_app_dir(&e.file_name().to_string_lossy());
            if pkg.is_empty() {
                continue;
            }
            if !apk_dirs.iter().any(|(p, _)| *p == pkg) {
                apk_dirs.push((pkg, e.path()));
            }
        }
    }
    apk_dirs.sort_by(|a, b| a.0.cmp(&b.0));

    let all_packages: Vec<String> = {
        let mut v: Vec<String> = package_dirs.iter().map(|(p, _)| p.clone()).collect();
        for (p, _) in &apk_dirs {
            if !v.contains(p) {
                v.push(p.clone());
            }
        }
        v.sort();
        v
    };

    for pkg in &all_packages {
        packages.push(pkg.clone());
        if policy.max_packages > 0 && packages.len() > policy.max_packages {
            truncated = true;
            break;
        }

        let excluded_by_policy = policy.exclude_packages.iter().any(|p| p == pkg);
        let not_whitelisted = !policy.include_packages.is_empty()
            && !policy.include_packages.iter().any(|p| p == pkg);

        // 1.a) APK
        if policy.include_apk {
            let dir = apk_dirs
                .iter()
                .find(|(p, _)| p == pkg)
                .map(|(_, d)| d.clone());
            if let Some(d) = dir {
                let mut apks: Vec<std::path::PathBuf> = Vec::new();
                if let Ok(rd) = std::fs::read_dir(&d) {
                    for e in rd.flatten() {
                        let n = e.file_name().to_string_lossy().to_string();
                        if n == "base.apk" || n.starts_with("split_") && n.ends_with(".apk") {
                            apks.push(e.path());
                        }
                    }
                }
                apks.sort();
                for apk in apks {
                    let rel = format!("{PAYLOAD_PREFIX}{pkg}/apk/{}", apk.file_name().unwrap().to_string_lossy());
                    let (size, mtime) = stat_of(&apk);
                    let (included, why) = decide(
                        policy,
                        &apk.to_string_lossy(),
                        &rel,
                        size,
                        excluded_by_policy,
                        not_whitelisted,
                        BackupSource::Apk,
                        EntryKind::Apk,
                    );
                    entries.push(PlanEntry {
                        package: pkg.clone(),
                        source: BackupSource::Apk,
                        kind: EntryKind::Apk,
                        path: apk.to_string_lossy().to_string(),
                        rel_path: rel,
                        size,
                        mtime,
                        included,
                        reason: why,
                    });
                }
            }
        }

        // 1.b) Datos privados
        if policy.include_data {
            if let Some((_, dir)) = package_dirs.iter().find(|(p, _)| p == pkg) {
                let mut files: Vec<(String, std::path::PathBuf)> = Vec::new();
                collect_files(dir, &format!("{PAYLOAD_PREFIX}{pkg}/data"), &mut files, 0);
                files.sort_by(|a, b| a.0.cmp(&b.0));
                for (rel, path) in files {
                    let (size, mtime) = stat_of(&path);
                    let kind = classify(&rel);
                    let (included, why) = decide(
                        policy,
                        &path.to_string_lossy(),
                        &rel,
                        size,
                        excluded_by_policy,
                        not_whitelisted,
                        BackupSource::Data,
                        kind,
                    );
                    entries.push(PlanEntry {
                        package: pkg.clone(),
                        source: BackupSource::Data,
                        kind,
                        path: path.to_string_lossy().to_string(),
                        rel_path: rel,
                        size,
                        mtime,
                        included,
                        reason: why,
                    });
                }
            }
        }

        // 1.c) Datos externos
        if policy.include_external {
            let ext = format!("{root}/data/media/0/Android/data/{pkg}");
            let p = Path::new(&ext);
            if p.is_dir() {
                let mut files: Vec<(String, std::path::PathBuf)> = Vec::new();
                collect_files(p, &format!("{PAYLOAD_PREFIX}{pkg}/external"), &mut files, 0);
                files.sort_by(|a, b| a.0.cmp(&b.0));
                for (rel, path) in files {
                    let (size, mtime) = stat_of(&path);
                    let (included, why) = decide(
                        policy,
                        &path.to_string_lossy(),
                        &rel,
                        size,
                        excluded_by_policy,
                        not_whitelisted,
                        BackupSource::External,
                        classify(&rel),
                    );
                    entries.push(PlanEntry {
                        package: pkg.clone(),
                        source: BackupSource::External,
                        kind: classify(&rel),
                        path: path.to_string_lossy().to_string(),
                        rel_path: rel,
                        size,
                        mtime,
                        included,
                        reason: why,
                    });
                }
            }
        }
    }

    // 2) Multimedia del usuario (opt-in: son decenas de gigas).
    if policy.include_media {
        let media = format!("{root}/data/media/0");
        let mut files: Vec<(String, std::path::PathBuf)> = Vec::new();
        collect_files(
            Path::new(&media),
            &format!("{PAYLOAD_PREFIX}media"),
            &mut files,
            0,
        );
        files.sort_by(|a, b| a.0.cmp(&b.0));
        for (rel, path) in files {
            // `/Android/data` ya se ha tocado como "external".
            if rel.contains("/Android/data/") {
                continue;
            }
            let (size, mtime) = stat_of(&path);
            let (included, why) = decide(
                policy,
                &path.to_string_lossy(),
                &rel,
                size,
                false,
                false,
                BackupSource::Media,
                classify(&rel),
            );
            entries.push(PlanEntry {
                package: "*".to_string(),
                source: BackupSource::Media,
                kind: classify(&rel),
                path: path.to_string_lossy().to_string(),
                rel_path: rel,
                size,
                mtime,
                included,
                reason: why,
            });
        }
    }

    // 3) Topes finales.
    let mut total_bytes = 0u64;
    let mut included_bytes = 0u64;
    let mut excluded_bytes = 0u64;
    let mut budget = policy.max_total_bytes;
    for e in entries.iter_mut() {
        total_bytes += e.size;
        if !e.included {
            excluded_bytes += e.size;
            continue;
        }
        if policy.max_total_bytes > 0 && included_bytes + e.size > budget {
            e.included = false;
            e.reason = reason::NO_ROOM.to_string();
            excluded_bytes += e.size;
            truncated = true;
            continue;
        }
        included_bytes += e.size;
        budget = budget.saturating_sub(e.size);
    }

    BackupPlan {
        policy: policy.clone(),
        entries,
        packages: packages.len(),
        total_bytes,
        included_bytes,
        excluded_bytes,
        truncated,
    }
}

#[allow(clippy::too_many_arguments)]
fn decide(
    policy: &BackupPolicy,
    abs_path: &str,
    rel_path: &str,
    size: u64,
    excluded_by_policy: bool,
    not_whitelisted: bool,
    source: BackupSource,
    kind: EntryKind,
) -> (bool, String) {
    let _ = rel_path;
    if excluded_by_policy {
        return (false, reason::EXCLUDED_PACKAGE.to_string());
    }
    if not_whitelisted {
        return (false, reason::NOT_WHITELISTED.to_string());
    }
    if kind == EntryKind::Cache && !policy.include_cache {
        return (false, reason::CACHE.to_string());
    }
    if policy.matches_exclusion(abs_path) {
        return (false, reason::PATTERN.to_string());
    }
    if policy.max_entry_bytes > 0 && size > policy.max_entry_bytes {
        return (false, reason::TOO_BIG.to_string());
    }
    let why = match source {
        BackupSource::Apk => reason::APK,
        BackupSource::Data => reason::DATA,
        BackupSource::External => reason::EXTERNAL,
        BackupSource::Media => reason::MEDIA,
    };
    (true, why.to_string())
}

/// Clasifica una ruta por su forma.
pub fn classify(rel: &str) -> EntryKind {
    let r = rel.to_ascii_lowercase();
    if r.contains("/shared_prefs/") {
        EntryKind::SharedPrefs
    } else if r.contains("/databases/") || r.ends_with(".db") || r.ends_with(".db-wal") || r.ends_with(".db-shm") {
        EntryKind::Database
    } else if r.contains("/cache/") || r.contains("/code_cache/") {
        EntryKind::Cache
    } else {
        EntryKind::File
    }
}

/// Extrae el paquete del nombre de un directorio de `/data/app`
/// (`com.foo.bar-1a2B3c4D` ⇒ `com.foo.bar`).
pub fn package_from_app_dir(dir_name: &str) -> String {
    match dir_name.rfind('-') {
        Some(i) if dir_name.len() - i - 1 >= 6 => dir_name[..i].to_string(),
        _ => dir_name.to_string(),
    }
}

fn first_existing(candidates: &[&str]) -> Option<String> {
    for c in candidates {
        if Path::new(c).is_dir() {
            return Some((*c).to_string());
        }
    }
    None
}

fn stat_of(p: &Path) -> (u64, u64) {
    match std::fs::metadata(p) {
        Ok(m) => {
            let size = m.len();
            let mtime = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            (size, mtime)
        }
        Err(_) => (0, 0),
    }
}

fn collect_files(
    base: &Path,
    prefix: &str,
    out: &mut Vec<(String, std::path::PathBuf)>,
    depth: usize,
) {
    if depth > 16 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(base) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let rel = format!("{prefix}/{name}");
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            collect_files(&path, &rel, out, depth + 1);
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        out.push((rel, path));
    }
}

// ---------------------------------------------------------------------------
// Captura
// ---------------------------------------------------------------------------

/// Opciones de la captura.
#[derive(Debug, Clone, PartialEq)]
pub struct CaptureOptions {
    /// Contraseña: `None` = contenedor **sin cifrar**.
    pub passphrase: Option<String>,
    /// Parámetros del KDF (Argon2id).
    pub kdf: KdfParams,
    /// Tamaño del trozo cifrado.
    pub chunk_size: usize,
    /// Nivel de zstd.
    pub compression_level: i32,
}

impl Default for CaptureOptions {
    fn default() -> Self {
        CaptureOptions {
            passphrase: None,
            kdf: KdfParams::default_params(),
            chunk_size: DEFAULT_CHUNK_SIZE,
            compression_level: 3,
        }
    }
}

/// Resultado de la captura.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptureResult {
    /// Ruta del contenedor escrito.
    pub container: String,
    /// Tamaño del contenedor en bytes.
    pub bytes: u64,
    /// El manifiesto de lo que hay dentro.
    pub manifest: Manifest,
    /// ¿Va cifrado?
    pub encrypted: bool,
    /// SHA-256 del **payload** (lo que hay tras la cabecera).
    pub payload_sha256: String,
    /// La cabecera final del contenedor.
    pub header: ContainerHeader,
}

/// Captura el plan a un contenedor.
///
/// Devuelve el manifiesto, que es la **prueba** de qué se metió dentro.
pub fn capture(
    plan: &BackupPlan,
    out: &str,
    opts: &CaptureOptions,
) -> Result<CaptureResult, String> {
    // 1) MANIFEST: SHA-256 + tamaño + ruta + mtime de cada entrada.
    let mut manifest = Manifest::new();
    for e in plan.included() {
        let (size, sha) = sha256_file(Path::new(&e.path))?;
        manifest.entries.push(ManifestEntry {
            path: e.rel_path.clone(),
            package: e.package.clone(),
            source: e.source.as_str().to_string(),
            size,
            sha256: sha,
            mtime: e.mtime,
            mode: crate::backup_crypto::unix_mode(
                &std::fs::metadata(&e.path).map_err(|err| {
                    format!("no se puede ver {}: {err}", e.path)
                })?,
            ),
        });
    }
    manifest.sort();
    let manifest_json = manifest.canonical_json();
    let manifest_hash = sha256_hex_bytes(manifest_json.as_bytes());

    // 2) Clave y key-check.
    let encrypted = opts.passphrase.is_some();
    let mut key: Vec<u8> = Vec::new();
    let mut check_hex = String::new();
    if let Some(pass) = &opts.passphrase {
        key = opts.kdf.derive_key(pass)?;
        check_hex = hex(&key_check(&key, LABEL_KEY_CHECK, 4));
    }

    // 3) Escribir el contenedor.
    let file = std::fs::File::create(out).map_err(|e| format!("no se puede crear {out}: {e}"))?;
    let mut hasher = HashingWriter::new(file);
    let provisional = ContainerHeader {
        version: CONTAINER_VERSION,
        flags: if encrypted { FLAG_ENCRYPTED } else { 0 },
        cipher: if encrypted {
            "aegis256-chunk-v1".to_string()
        } else {
            "none".to_string()
        },
        kdf: opts.kdf.clone(),
        key_check: check_hex.clone(),
        chunk_size: opts.chunk_size as u32,
        manifest_sha256: manifest_hash.clone(),
        payload_len: 0,
        payload_sha256: String::new(),
        created_at: crate::backup_crypto::now_epoch(),
    };
    // Cabecera provisional: el hash del manifiesto ya se conoce; el del
    // payload se parchea al final.
    hasher
        .write_all(&crate::backup_crypto::encode_container_header(&provisional))
        .map_err(|e| format!("error escribiendo la cabecera: {e}"))?;
    // MEDIDO 2026-11-22: el hash del payload se calcula SOLO sobre los
    // marcos (como hace `verify`), no sobre la cabecera + payload.
    hasher.reset_hash();

    // El "nombre lógico" del payload ata los trozos al manifiesto.
    let stream_name = manifest_hash.clone();

    let (payload_sha256, payload_len) = {
        let mut sink = ChunkSink::new(
            &mut hasher,
            if encrypted {
                Some(ChunkCipher::own(&key)?)
            } else {
                None
            },
            stream_name.clone(),
            opts.chunk_size,
        );
        {
            let mut enc = zstd::stream::write::Encoder::new(&mut sink, opts.compression_level)
                .map_err(|e| format!("zstd: {e}"))?;
            // El manifiesto va **primero** dentro del tar: viaja con los datos.
            write_tar_entry(
                &mut enc,
                MANIFEST_ENTRY,
                manifest_json.len() as u64,
                manifest_json.as_bytes(),
            )
            .map_err(|e| format!("error escribiendo la entrada del manifiesto: {e}"))?;

            for e in plan.included() {
                copy_file_into_tar(&mut enc, &e.rel_path, &e.path, e.mtime)
                    .map_err(|err| format!("error escribiendo {}: {err}", e.rel_path))?;
            }
            enc.finish().map_err(|e| format!("zstd: {e}"))?;
        }
        sink.flush_all().map_err(|e| format!("error volcando el payload: {e}"))?;
        // `sink` se deja caer aquí ⇒ el préstamo de `hasher` termina.
        drop(sink);
        let sha = hasher.finish();
        let len = hasher.bytes_written().saturating_sub(CONTAINER_HEADER_SIZE as u64);
        (sha, len)
    };
    hasher.flush().map_err(|e| format!("error volcando el fichero: {e}"))?;

    // 4) Parchear la cabecera con el hash real del payload.
    let final_header = ContainerHeader {
        payload_len,
        payload_sha256: payload_sha256.clone(),
        ..provisional
    };
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .open(out)
        .map_err(|e| format!("no se puede reabrir {out}: {e}"))?;
    use std::io::Seek;
    f.seek(std::io::SeekFrom::Start(0))
        .map_err(|e| format!("no se puede rebobinar {out}: {e}"))?;
    f.write_all(&crate::backup_crypto::encode_container_header(&final_header))
        .map_err(|e| format!("no se puede parchear la cabecera: {e}"))?;
    f.flush().map_err(|e| format!("no se puede volcar {out}: {e}"))?;

    Ok(CaptureResult {
        container: out.to_string(),
        bytes: CONTAINER_HEADER_SIZE as u64 + payload_len,
        manifest,
        encrypted,
        payload_sha256,
        header: final_header,
    })
}

/// Escribe una entrada de tar (cabecera + contenido + relleno).
fn write_tar_entry<W: Write>(
    w: &mut W,
    name: &str,
    size: u64,
    data: &[u8],
) -> Result<(), String> {
    let mtime = 0u64;
    let mode = 0o644u32;
    w.write_all(&crate::backup_crypto::tar_header(name, size, mode, mtime))
        .map_err(|e| format!("tar: {e}"))?;
    w.write_all(data).map_err(|e| format!("tar: {e}"))?;
    let pad = (512 - (data.len() % 512)) % 512;
    if pad > 0 {
        w.write_all(&vec![0u8; pad]).map_err(|e| format!("tar: {e}"))?;
    }
    Ok(())
}

/// Copia un fichero real al tar **en streaming** (no lo carga en memoria:
/// un APK o un vídeo de 2 GB tienen que poder pasar por aquí).
fn copy_file_into_tar<W: Write>(
    w: &mut W,
    name: &str,
    path: &str,
    mtime: u64,
) -> Result<(), String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("no se puede ver {path}: {e}"))?;
    let size = meta.len();
    let mode = crate::backup_crypto::unix_mode(&meta) | 0o400;
    w.write_all(&crate::backup_crypto::tar_header(name, size, mode, mtime))
        .map_err(|e| format!("tar: {e}"))?;
    let mut f = std::fs::File::open(path).map_err(|e| format!("no se puede abrir {path}: {e}"))?;
    let mut buf = vec![0u8; 1024 * 1024];
    let mut written = 0u64;
    loop {
        let n = f.read(&mut buf).map_err(|e| format!("error leyendo {path}: {e}"))?;
        if n == 0 {
            break;
        }
        w.write_all(&buf[..n]).map_err(|e| format!("tar: {e}"))?;
        written += n as u64;
    }
    let pad = (512 - (written % 512)) % 512;
    if pad > 0 {
        w.write_all(&vec![0u8; pad as usize]).map_err(|e| format!("tar: {e}"))?;
    }
    Ok(())
}

/// SHA-256 de un bloque.
pub fn sha256_hex_bytes(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex(&h.finalize())
}

/// Escritor que **calcula el hash de lo que escribe** (sin releer el disco).
struct HashingWriter<W: Write> {
    inner: W,
    hash: Sha256,
    written: u64,
}

impl<W: Write> HashingWriter<W> {
    fn new(inner: W) -> HashingWriter<W> {
        HashingWriter {
            inner,
            hash: Sha256::new(),
            written: 0,
        }
    }
    fn bytes_written(&self) -> u64 {
        self.written
    }
    /// Hash de lo escrito hasta ahora (finaliza el hasher).
    fn finish(&mut self) -> String {
        hex(&self.hash.clone().finalize())
    }
    /// Reinicia el hash (el hash del payload NO debe incluir la cabecera).
    ///
    /// MEDIDO 2026-11-22 (bug REAL): `capture` escribia la cabecera y el
    /// payload por el mismo `HashingWriter`, asi que `payload_sha256` era el
    /// hash de **cabecera + payload**, mientras que `verify` hashea **solo el
    /// payload** => `payload_ok=false` siempre.
    fn reset_hash(&mut self) {
        self.hash = Sha256::new();
    }
}

impl<W: Write> Write for HashingWriter<W> {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.inner.write_all(data)?;
        self.hash.update(data);
        self.written += data.len() as u64;
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// Receptor del payload: **cifra por trozos** y escribe los marcos.
struct ChunkSink<W: Write> {
    inner: W,
    cipher: Option<ChunkCipher>,
    stream: String,
    chunk_size: usize,
    buf: Vec<u8>,
    index: u64,
}

impl<W: Write> ChunkSink<W> {
    fn new(inner: W, cipher: Option<ChunkCipher>, stream: String, chunk_size: usize) -> ChunkSink<W> {
        ChunkSink {
            inner,
            cipher,
            stream,
            chunk_size: chunk_size.max(4096),
            buf: Vec::new(),
            index: 0,
        }
    }
    fn flush_all(&mut self) -> Result<(), String> {
        if !self.buf.is_empty() {
            self.emit()?;
        }
        self.inner.flush().map_err(|e| format!("error volcando: {e}"))
    }
    fn emit(&mut self) -> Result<(), String> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let data = std::mem::take(&mut self.buf);
        match &self.cipher {
            Some(c) => {
                let frame = c.seal_chunk(&self.stream, self.index, &data);
                self.inner
                    .write_all(&frame)
                    .map_err(|e| format!("error escribiendo el marco: {e}"))?;
            }
            None => {
                self.inner
                    .write_all(&data)
                    .map_err(|e| format!("error escribiendo el payload: {e}"))?;
            }
        }
        self.index += 1;
        Ok(())
    }
}

impl<W: Write> Write for ChunkSink<W> {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        let mut off = 0usize;
        while off < data.len() {
            let room = self.chunk_size - self.buf.len();
            let take = room.min(data.len() - off);
            self.buf.extend_from_slice(&data[off..off + take]);
            off += take;
            if self.buf.len() >= self.chunk_size {
                self.emit().map_err(|e| std::io::Error::other(e))?;
            }
        }
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.flush_all().map_err(|e| std::io::Error::other(e))
    }
}

// ---------------------------------------------------------------------------
// Verificación
// ---------------------------------------------------------------------------

/// Informe de la verificación.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerifyReport {
    /// ¿Todo cuadra?
    pub ok: bool,
    /// ¿El contenedor va cifrado?
    pub encrypted: bool,
    /// ¿El hash del payload cuadra?
    pub payload_ok: bool,
    /// ¿El manifiesto cuadra con su hash?
    pub manifest_ok: bool,
    /// Entradas re-hasheadas.
    pub entries_checked: usize,
    /// Entradas whose hash **no** cuadra.
    pub entries_bad: Vec<String>,
    /// Bytes leídos.
    pub bytes: u64,
    /// Detalle legible.
    pub detail: String,
}

/// **Verifica** un contenedor: recalcula los hashes y los compara con el
/// MANIFEST. Detecta corrupción (de transporte o de disco).
pub fn verify(container: &str, passphrase: Option<&str>) -> Result<VerifyReport, String> {
    let mut file = std::fs::File::open(container).map_err(|e| format!("no se puede abrir {container}: {e}"))?;
    let mut header_bytes = vec![0u8; CONTAINER_HEADER_SIZE];
    file.read_exact(&mut header_bytes)
        .map_err(|e| format!("cabecera incompleta en {container}: {e}"))?;
    let (header, payload_off) = crate::backup_crypto::decode_container_header(&header_bytes)?;

    // 1) Contraseña: el key-check decide **sin descifrar**.
    let mut cipher: Option<ChunkCipher> = None;
    let encrypted = header.flags & FLAG_ENCRYPTED != 0;
    if encrypted {
        let pass = passphrase.ok_or("el contenedor está cifrado y no se ha dado contraseña")?;
        let key = header.kdf.derive_key(pass)?;
        let check = key_check(&key, LABEL_KEY_CHECK, 4);
        if !crate::backup_crypto::key_check_eq(&check, &unhex(&header.key_check).unwrap_or_default()) {
            return Err("la contraseña no cuadra con el key-check del contenedor".to_string());
        }
        cipher = Some(ChunkCipher::own(&key)?);
    } else if passphrase.is_some() {
        return Err("se dio contraseña pero el contenedor no está cifrado".to_string());
    }

    // 2) Hash del payload + descifrado + descompresión, todo en una pasada.
    let mut hash = Sha256::new();
    let mut payload_bytes = 0u64;
    let stream_name = header.manifest_sha256.clone();
    let plain = read_payload(
        &mut file,
        payload_off,
        cipher.as_ref(),
        &stream_name,
        &mut hash,
        &mut payload_bytes,
    )?;

    // 3) El hash del payload.
    let payload_hash = hex(&hash.finalize());
    let payload_ok = payload_hash.eq_ignore_ascii_case(&header.payload_sha256);

    // 4) Descomprimir (en streaming: no hace falta saber el tamaño) y leer el tar.
    let mut dec = zstd::stream::read::Decoder::new(std::io::Cursor::new(plain))
        .map_err(|e| format!("zstd: {e}"))?;
    let mut tar_stream = TarStream::new(&mut dec);
    let mut manifest: Option<Manifest> = None;
    let mut entries: Vec<ManifestEntry> = Vec::new();
    while let Some(entry) = tar_stream
        .next_entry()
        .map_err(|e| format!("tar: {e}"))?
    {
        if entry.name == MANIFEST_ENTRY {
            let mut buf = Vec::with_capacity(entry.size as usize);
            tar_stream
                .copy_data(&mut buf)
                .map_err(|e| format!("tar: {e}"))?;
            manifest = Some(
                serde_json::from_slice(&buf).map_err(|e| format!("manifiesto inválido: {e}"))?,
            );
            continue;
        }
        // Para el resto: se recalcula el hash sobre el propio contenido.
        let mut h = Sha256::new();
        let mut sink = HashSink { hash: &mut h, written: 0 };
        tar_stream
            .copy_data(&mut sink)
            .map_err(|e| format!("tar: {e}"))?;
        entries.push(ManifestEntry {
            path: entry.name.clone(),
            package: String::new(),
            source: String::new(),
            size: sink.written,
            sha256: hex(&h.finalize()),
            mtime: entry.mtime,
            mode: entry.mode,
        });
    }

    let manifest = manifest.ok_or("el contenedor no trae manifiesto")?;
    let manifest_ok = manifest.hash().eq_ignore_ascii_case(&header.manifest_sha256);

    // 5) Comparar entrada a entrada.
    let mut entries_checked = 0usize;
    let mut entries_bad: Vec<String> = Vec::new();
    for m in &manifest.entries {
        let found = entries.iter().find(|e| e.path == m.path);
        match found {
            Some(e) => {
                entries_checked += 1;
                if !e.sha256.eq_ignore_ascii_case(&m.sha256) || e.size != m.size {
                    entries_bad.push(m.path.clone());
                }
            }
            None => {
                entries_checked += 1;
                entries_bad.push(m.path.clone());
            }
        }
    }

    let ok = payload_ok && manifest_ok && entries_bad.is_empty();
    // El detalle se calcula ANTES de mover `entries_bad` al informe.
    let bad_len = entries_bad.len();
    Ok(VerifyReport {
        ok,
        encrypted,
        payload_ok,
        manifest_ok,
        entries_checked,
        entries_bad,
        bytes: payload_bytes,
        detail: if ok {
            format!("{entries_checked} entradas verificadas: payload, manifiesto y hashes cuadran")
        } else {
            format!(
                "payload_ok={payload_ok} manifest_ok={manifest_ok} entradas_cojas={bad_len}",
            )
        },
    })
}

/// Receptor que sólo calcula el hash de lo que pasa por él.
struct HashSink<'a> {
    hash: &'a mut Sha256,
    written: u64,
}

impl Write for HashSink<'_> {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.hash.update(data);
        self.written += data.len() as u64;
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Lee el payload: descifra (si toca) y lo devuelve **plano**.
///
/// Hash y descifrado en la misma pasada ⇒ no se relee el disco.
///
/// `stream` es el "nombre lógico" del payload: ata cada trozo a **el
/// manifiesto** que describe lo que contiene (si alguien cambia el manifiesto
/// sin cambiar el payload, los trozos dejan de cuadrar).
fn read_payload(
    file: &mut std::fs::File,
    offset: usize,
    cipher: Option<&ChunkCipher>,
    stream: &str,
    hash: &mut Sha256,
    bytes: &mut u64,
) -> Result<Vec<u8>, String> {
    use std::io::Seek;
    file.seek(std::io::SeekFrom::Start(offset as u64))
        .map_err(|e| format!("no se puede posicionar en el payload: {e}"))?;
    match cipher {
        None => {
            let mut out = Vec::new();
            let mut buf = vec![0u8; 1024 * 1024];
            loop {
                let n = file
                    .read(&mut buf)
                    .map_err(|e| format!("error leyendo el payload: {e}"))?;
                if n == 0 {
                    break;
                }
                hash.update(&buf[..n]);
                *bytes += n as u64;
                out.extend_from_slice(&buf[..n]);
            }
            Ok(out)
        }
        Some(c) => {
            // Formato de los marcos: [u32 len][nonce 32][cifrado][etiqueta 32]
            let mut out = Vec::new();
            let mut index = 0u64;
            loop {
                let mut len_buf = [0u8; 4];
                let n = read_full(file, &mut len_buf)?;
                if n == 0 {
                    break;
                }
                let len = u32::from_le_bytes(len_buf) as usize;
                let mut frame = vec![0u8; 4 + 32 + len + 32];
                // MEDIDO 2026-11-22 (CAUSA RAIZ de los 4 tests rojos del
                // motor de backup): la longitud del trozo se leia en
                // `len_buf` pero NUNCA se copiaba al frame => los 4 primeros
                // bytes del marco quedaban a CERO => `open_chunk` calculaba
                // el AAD con longitudes 0 y el AEAD rechazaba el marco con
                // 'la etiqueta no cuadra'.  El motor estaba BIEN: el lector
                // perdera la cabecera del marco.
                frame[..4].copy_from_slice(&len_buf);
                let mut got = 4usize;
                while got < frame.len() {
                    let n = read_full(file, &mut frame[got..])?;
                    if n == 0 {
                        return Err("el payload termina a mitad de un marco".to_string());
                    }
                    got += n;
                }
                hash.update(&frame);
                *bytes += frame.len() as u64;
                let plain = c
                    .open_chunk(stream, index, &frame)
                    .map_err(|e| format!("marco {index}: {e}"))?;
                out.extend_from_slice(&plain);
                index += 1;
            }
            Ok(out)
        }
    }
}

fn read_full(file: &mut std::fs::File, buf: &mut [u8]) -> Result<usize, String> {
    let mut got = 0usize;
    while got < buf.len() {
        let n = file
            .read(&mut buf[got..])
            .map_err(|e| format!("error leyendo: {e}"))?;
        if n == 0 {
            break;
        }
        got += n;
    }
    Ok(got)
}

// ---------------------------------------------------------------------------
// Restore selectivo
// ---------------------------------------------------------------------------

/// Qué se restaura.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RestoreSelector {
    /// Paquetes concretos (vacío = todos).
    pub packages: Vec<String>,
    /// Rutas concretas (coincidencia exacta o prefijo de directorio).
    pub entries: Vec<String>,
}

impl RestoreSelector {
    /// Todo.
    pub fn all() -> RestoreSelector {
        RestoreSelector::default()
    }
    /// Un paquete.
    pub fn package(pkg: &str) -> RestoreSelector {
        RestoreSelector {
            packages: vec![pkg.to_string()],
            entries: Vec::new(),
        }
    }
    /// ¿Esta entrada del manifiesto entra en la selección?
    pub fn matches(&self, entry: &ManifestEntry) -> bool {
        if !self.packages.is_empty() && !self.packages.contains(&entry.package) {
            // si se piden rutas concretas, el paquete no tiene que estar
            if self.entries.is_empty() {
                return false;
            }
        }
        if self.entries.is_empty() {
            return true;
        }
        self.entries
            .iter()
            .any(|p| entry.path == *p || entry.path.starts_with(&format!("{p}/")))
    }
}

/// Opciones del restore.
#[derive(Debug, Clone, PartialEq)]
pub struct RestoreOptions {
    /// **Obligatorio**: si es `false`, se exige `confirm_write`.
    pub dry_run: bool,
    /// Confirmación explícita para escribir.
    pub confirm_write: bool,
    /// Sobrescribir ficheros que ya existen.
    pub overwrite: bool,
    /// Volver a hashear lo escrito y compararlo con el manifiesto.
    pub verify_after: bool,
}

impl Default for RestoreOptions {
    fn default() -> Self {
        RestoreOptions {
            dry_run: true,
            confirm_write: false,
            overwrite: false,
            verify_after: true,
        }
    }
}

/// Informe del restore.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RestoreReport {
    /// ¿Fue sólo un simulacro?
    pub dry_run: bool,
    /// ¿Terminó bien?
    pub ok: bool,
    /// Ficheros escritos.
    pub written: Vec<String>,
    /// Ficheros saltados (noSelected).
    pub skipped: Vec<String>,
    /// Ficheros que fallaron.
    pub failed: Vec<String>,
    /// ¿Se hubo que dar marcha atrás?
    pub rolled_back: bool,
    /// Bytes escritos.
    pub bytes: u64,
    /// Detalle legible.
    pub detail: String,
}

/// Restaura **selectivamente** un contenedor.
///
/// * El **dry-run es obligatorio**: sin `confirm_write` no se escribe nada.
/// * Si algo falla a mitad ⇒ **rollback**: se vuelven a poner los ficheros
///   originales (o se borran los nuevos) y se devuelve el informe.
pub fn restore(
    container: &str,
    passphrase: Option<&str>,
    dest: &str,
    selector: &RestoreSelector,
    options: &RestoreOptions,
) -> Result<RestoreReport, String> {
    if !options.dry_run && !options.confirm_write {
        return Err(
            "restore sin dry-run exige confirm_write=true: un restore escribe en el dispositivo"
                .to_string(),
        );
    }

    // 1) Abrir y descifrar (mismo camino que `verify`).
    let mut file = std::fs::File::open(container).map_err(|e| format!("no se puede abrir {container}: {e}"))?;
    let mut header_bytes = vec![0u8; CONTAINER_HEADER_SIZE];
    file.read_exact(&mut header_bytes)
        .map_err(|e| format!("cabecera incompleta: {e}"))?;
    let (header, payload_off) = crate::backup_crypto::decode_container_header(&header_bytes)?;
    let mut cipher: Option<ChunkCipher> = None;
    if header.flags & FLAG_ENCRYPTED != 0 {
        let pass = passphrase.ok_or("el contenedor está cifrado y no se ha dado contraseña")?;
        let key = header.kdf.derive_key(pass)?;
        let check = key_check(&key, LABEL_KEY_CHECK, 4);
        if !crate::backup_crypto::key_check_eq(&check, &unhex(&header.key_check).unwrap_or_default()) {
            return Err("la contraseña no cuadra con el key-check del contenedor".to_string());
        }
        cipher = Some(ChunkCipher::own(&key)?);
    }
    let mut hash = Sha256::new();
    let mut nbytes = 0u64;
    let stream_name = header.manifest_sha256.clone();
    let plain = read_payload(
        &mut file,
        payload_off,
        cipher.as_ref(),
        &stream_name,
        &mut hash,
        &mut nbytes,
    )?;

    // 2) Recorrer el tar y restaurar lo seleccionado.
    let mut dec = zstd::stream::read::Decoder::new(std::io::Cursor::new(plain))
        .map_err(|e| format!("zstd: {e}"))?;
    let mut tar_stream = TarStream::new(&mut dec);
    let mut manifest: Option<Manifest> = None;
    let mut written: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut failed: Vec<String> = Vec::new();
    let mut bytes = 0u64;
    let mut undone: Vec<(String, Option<std::path::PathBuf>)> = Vec::new();
    let mut rolled_back = false;

    let dest_root = Path::new(dest);
    if !options.dry_run {
        std::fs::create_dir_all(dest_root).map_err(|e| format!("no se puede crear {dest}: {e}"))?;
    }

    while let Some(entry) = tar_stream
        .next_entry()
        .map_err(|e| format!("tar: {e}"))?
    {
        if entry.name == MANIFEST_ENTRY {
            let mut buf = Vec::with_capacity(entry.size as usize);
            tar_stream.copy_data(&mut buf).map_err(|e| format!("tar: {e}"))?;
            manifest = Some(serde_json::from_slice(&buf).map_err(|e| format!("manifiesto inválido: {e}"))?);
            continue;
        }
        // Leer el contenido a memoria **sólo** si es lo que se va a escribir.
        let manifest_entry = manifest
            .as_ref()
            .and_then(|m| m.entries.iter().find(|e| e.path == entry.name))
            .cloned();
        let selected = manifest_entry
            .as_ref()
            .map(|m| selector.matches(m))
            .unwrap_or(false);

        if !selected {
            // descartar
            tar_stream
                .copy_data(&mut std::io::sink())
                .map_err(|e| format!("tar: {e}"))?;
            skipped.push(entry.name.clone());
            continue;
        }

        let rel = match safe_relative_path(&entry.name) {
            Ok(r) => r,
            Err(e) => {
                failed.push(entry.name.clone());
                let _ = e;
                continue;
            }
        };
        let target = dest_root.join(&rel);

        if options.dry_run {
            let mut sink = HashSink {
                hash: &mut Sha256::new(),
                written: 0,
            };
            let n = tar_stream.copy_data(&mut sink).map_err(|e| format!("tar: {e}"))?;
            bytes += n;
            written.push(rel.to_string_lossy().to_string());
            continue;
        }

        // Respaldo del original para poder deshacer.
        let backup = if target.exists() {
            if !options.overwrite {
                skipped.push(entry.name.clone());
                tar_stream
                    .copy_data(&mut std::io::sink())
                    .map_err(|e| format!("tar: {e}"))?;
                continue;
            }
            let bpath = backup_path(&target);
            if let Some(parent) = bpath.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            // `undone` guarda la RUTA del backup (no el numero de bytes):
            // al reponer hay que saber de donde se copio.
            std::fs::copy(&target, &bpath).ok().map(|_| bpath)
        } else {
            None
        };
        undone.push((target.to_string_lossy().to_string(), backup));

        if let Some(parent) = target.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                failed.push(entry.name.clone());
                let _ = e;
                break;
            }
        }

        // Se escribe **en streaming** a un fichero temporal y sólo se renombra
        // al final ⇒ un fallo a mitad no deja un fichero corrupto en su sitio.
        let tmp = tmp_path(&target);
        let write_result = (|| -> Result<(u64, String), String> {
            let f = std::fs::File::create(&tmp).map_err(|e| format!("no se puede crear {tmp:?}: {e}"))?;
            let mut sink = HashingFileSink {
                inner: f,
                hash: Sha256::new(),
                written: 0,
            };
            let n = tar_stream.copy_data(&mut sink).map_err(|e| format!("tar: {e}"))?;
            sink.inner
                .flush()
                .map_err(|e| format!("error volcando {tmp:?}: {e}"))?;
            let sha = hex(&sink.hash.finalize());
            Ok((n, sha))
        })();

        let (n, sha) = match write_result {
            Ok(v) => v,
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                failed.push(entry.name.clone());
                let _ = e;
                break;
            }
        };

        // Verificación posterior: el hash de lo escrito debe cuadrar.
        if options.verify_after {
            if let Some(m) = &manifest_entry {
                if !sha.eq_ignore_ascii_case(&m.sha256) {
                    let _ = std::fs::remove_file(&tmp);
                    failed.push(entry.name.clone());
                    break;
                }
            }
        }
        if let Err(e) = std::fs::rename(&tmp, &target) {
            let _ = std::fs::remove_file(&tmp);
            failed.push(entry.name.clone());
            let _ = e;
            break;
        }
        bytes += n;
        written.push(rel.to_string_lossy().to_string());
    }

    // 3) Rollback si algo falló.
    let ok = failed.is_empty();
    if !ok && !options.dry_run {
        rolled_back = true;
        for (path, backup) in undone.iter().rev() {
            let p = Path::new(path);
            match backup {
                Some(b) => {
                    let _ = std::fs::copy(b, p);
                }
                None => {
                    let _ = std::fs::remove_file(p);
                }
            }
        }
        written.clear();
    }

    // Las longitudes se calculan ANTES de mover los vectores al informe.
    let written_len = written.len();
    let failed_len = failed.len();
    Ok(RestoreReport {
        dry_run: options.dry_run,
        ok,
        written,
        skipped,
        failed,
        rolled_back,
        bytes,
        detail: if options.dry_run {
            format!("simulacion: {} ficheros seleccionados, {} bytes", written_len, bytes)
        } else if ok {
            format!("{} ficheros restaurados, {} bytes", written_len, bytes)
        } else {
            format!(
                "fallo en {} ficheros => rollback ({})",
                failed_len,
                if rolled_back { "hecho" } else { "no" }
            )
        },
    })
}

/// Ruta del respaldo de seguridad (para el rollback).
fn backup_path(target: &Path) -> std::path::PathBuf {
    let mut p = target.to_path_buf();
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "f".to_string());
    p.set_file_name(format!(".md-rollback-{name}"));
    p
}

/// Ruta del fichero temporal mientras se restaura.
fn tmp_path(target: &Path) -> std::path::PathBuf {
    let mut p = target.to_path_buf();
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "f".to_string());
    p.set_file_name(format!(".md-parcial-{name}"));
    p
}

/// Escribe en el fichero **y** calcula el hash de lo escrito.
struct HashingFileSink<W: Write> {
    inner: W,
    hash: Sha256,
    written: u64,
}

impl<W: Write> Write for HashingFileSink<W> {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.inner.write_all(data)?;
        self.hash.update(data);
        self.written += data.len() as u64;
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

// ---------------------------------------------------------------------------
// Subida a Drive (resumible)
// ---------------------------------------------------------------------------

/// Plan de subida de un contenedor a Drive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UploadPlan {
    /// Ruta del contenedor local.
    pub container: String,
    /// Tamaño total en bytes.
    pub total: u64,
    /// SHA-256 del payload (para verificar tras la descarga).
    pub payload_sha256: String,
    /// Petición que abre la sesión de subida resumible.
    pub open_session: DriveRequest,
    /// Los trozos, contiguos y sin huecos.
    pub chunks: Vec<DriveChunkRange>,
}

/// Prepara la **subida resumible** de un contenedor a Drive.
///
/// Usa [`crate::drive_client`]: la sesión se abre con un POST (que devuelve
/// el `Location:`) y cada trozo va en un PUT con `Content-Range`.
pub fn upload_plan(
    container: &str,
    creds: &DriveCredentials,
    target: &DriveTarget,
    chunk_size: u64,
) -> Result<UploadPlan, String> {
    let meta = std::fs::metadata(container).map_err(|e| format!("no se puede ver {container}: {e}"))?;
    let total = meta.len();
    let mut f = std::fs::File::open(container).map_err(|e| format!("no se puede abrir {container}: {e}"))?;
    let mut header_bytes = vec![0u8; CONTAINER_HEADER_SIZE];
    f.read_exact(&mut header_bytes)
        .map_err(|e| format!("cabecera incompleta: {e}"))?;
    let (header, _) = crate::backup_crypto::decode_container_header(&header_bytes)?;
    let name = std::path::Path::new(container)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "backup.mdbk".to_string());
    Ok(UploadPlan {
        container: container.to_string(),
        total,
        payload_sha256: header.payload_sha256,
        open_session: open_upload_session(creds, &name, total, target),
        chunks: chunk_plan(total, chunk_size),
    })
}

/// Las peticiones de cada trozo, una vez conocida la URL de sesión.
pub fn upload_chunk_requests(
    creds: &DriveCredentials,
    session_uri: &str,
    plan: &UploadPlan,
) -> Vec<DriveRequest> {
    plan.chunks
        .iter()
        .map(|r| upload_chunk_request(creds, session_uri, r, plan.total))
        .collect()
}

// ---------------------------------------------------------------------------
// Planificador
// ---------------------------------------------------------------------------

/// ¿Es buen momento para respaldar?
///
/// De noche, con batería suficiente y sin calor: un backup de gigas en un
/// móvil con la batería al 8 % es un móvil muerto.
pub fn is_good_time_to_backup() -> (bool, String) {
    let hour = chrono_hour();
    let status = crate::battery_saver::status();
    let power = &status["power"];
    let pct = power["battery_pct"].as_u64().unwrap_or(100);
    let charging = power["charging"].as_bool().unwrap_or(false);
    let hot = power["thermal_hot"].as_bool().unwrap_or(false);
    let allow_heavy = power["allow_heavy"].as_bool().unwrap_or(true);

    if hot {
        return (false, format!("el dispositivo está caliente (batería {pct}%)"));
    }
    if !allow_heavy {
        return (
            false,
            format!("la política de energía no permite carga pesada (batería {pct}%)"),
        );
    }
    if pct < 20 && !charging {
        return (
            false,
            format!("batería al {pct}% y sin cargador: un backup de gigas la dejaría en cero"),
        );
    }
    if !(1..=6).contains(&hour) {
        return (
            false,
            format!("no es de noche (hora {hour}): se reserva a la ventana 01:00-06:00"),
        );
    }
    (true, format!("ventana nocturna, batería {pct}%, política {policy}", policy = policy_name()))
}

/// Nombre de la política de energía vigente.
fn policy_name() -> String {
    crate::battery_saver::status()["power"]["policy"]
        .as_str()
        .unwrap_or("desconocida")
        .to_string()
}

/// Hora local (0..23). Sin chrono: se usa la zona horaria del sistema por
/// el desplazamiento de `localtime`, vía `libc` (el crate ya depende de él).
fn chrono_hour() -> u32 {
    #[cfg(unix)]
    {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as libc::time_t)
            .unwrap_or(0);
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        unsafe {
            libc::localtime_r(&now, &mut tm);
        }
        tm.tm_hour as u32
    }
    #[cfg(not(unix))]
    {
        0
    }
}

/// Encola el backup nocturno en el [`crate::task_scheduler`].
///
/// Devuelve el id de la tarea. La tarea es **pesada** (gigas) ⇒ el
/// planificador la Launch sólo cuando la batería y la política lo
/// permiten.
pub fn schedule_backup(policy: BackupPolicy, out_path: &str) -> String {
    let payload = json!({
        "policy": policy,
        "out": out_path,
    });
    crate::task_scheduler::submit(
        "backup.nightly",
        payload,
        5,
        vec![],
        Resources {
            cpu: 2,
            net: true,
            heavy: true,
        },
    )
}

/// Ejecuta el backup ya encolado (lo llama el planificador).
pub fn run_scheduled_backup(payload: &Value) -> Value {
    let policy: BackupPolicy = serde_json::from_value(payload["policy"].clone())
        .unwrap_or_else(|_| BackupPolicy::default());
    let out = payload["out"].as_str().unwrap_or("").to_string();
    if out.is_empty() {
        return json!({ "ok": false, "error": "missing_out" });
    }
    let root = payload["root"].as_str().unwrap_or("/").to_string();
    let plan = plan(&root, &policy);
    let opts = CaptureOptions::default();
    match capture(&plan, &out, &opts) {
        Ok(r) => json!({
            "ok": true,
            "summary": format!("{} entradas, {} bytes", r.manifest.entries.len(), r.bytes),
            "container": r.container,
            "bytes": r.bytes,
            "encrypted": r.encrypted,
        }),
        Err(e) => json!({ "ok": false, "error": e }),
    }
}

// ---------------------------------------------------------------------------
// Pruebas
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Monta un árbol de dispositivo falso.
    /// MEDIDO 2026-11-22 (CAUSA RAIZ de 5 tests rojos): `fake_device()`
    /// usaba SIEMPRE el mismo directorio (`md-eng-<pid>`), asi que los tests
    /// se pisaban entre si: el que corria en paralelo BORRABA el arbol del
    /// otro (`remove_dir_all`) y el test de corrupcion alteraba el
    /// contenedor del vecino (`data[pos] ^= 0xff`), haciendo fallar al verify
    /// con 'la etiqueta no cuadra' aunque el motor este bien.
    /// Ahora cada llamada usa un directorio UNICO.
    fn fake_device() -> std::path::PathBuf {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!("md-eng-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("data/user/0/com.foo/shared_prefs")).unwrap();
        std::fs::create_dir_all(root.join("data/user/0/com.foo/databases")).unwrap();
        std::fs::create_dir_all(root.join("data/user/0/com.foo/cache")).unwrap();
        std::fs::write(root.join("data/user/0/com.foo/shared_prefs/a.xml"), b"<prefs/>").unwrap();
        std::fs::write(root.join("data/user/0/com.foo/databases/b.db"), b"SQLite format 3").unwrap();
        std::fs::write(root.join("data/user/0/com.foo/cache/c.tmp"), b"basura").unwrap();
        std::fs::create_dir_all(root.join("data/user/0/com.bar/files")).unwrap();
        std::fs::write(root.join("data/user/0/com.bar/files/f.txt"), b"hola").unwrap();
        std::fs::create_dir_all(root.join("data/app/com.foo-1a2B3c4D")).unwrap();
        std::fs::write(root.join("data/app/com.foo-1a2B3c4D/base.apk"), b"APK").unwrap();
        std::fs::create_dir_all(root.join("data/media/0/Android/data/com.foo")).unwrap();
        std::fs::write(root.join("data/media/0/Android/data/com.foo/x.bin"), b"ext").unwrap();
        root
    }

    #[test]
    fn el_plan_explica_cada_exclusion() {
        let root = fake_device();
        let p = plan(root.to_str().unwrap(), &BackupPolicy::default());
        // com.foo está dentro, com.bar también
        assert!(p.packages >= 2);
        // la caché queda fuera, y **se dice por qué**
        let cache = p
            .entries
            .iter()
            .find(|e| e.rel_path.contains("/cache/"))
            .expect("debería haber una entrada de caché");
        assert!(!cache.included);
        assert_eq!(cache.reason, reason::CACHE);
        // los shared_prefs y las DBs sí entran
        assert!(p
            .entries
            .iter()
            .any(|e| e.rel_path.contains("shared_prefs") && e.included));
        assert!(p
            .entries
            .iter()
            .any(|e| e.rel_path.contains("databases") && e.included));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn la_lista_blanca_y_la_negra_se_respetan() {
        let root = fake_device();
        let mut pol = BackupPolicy::default();
        pol.include_packages = vec!["com.foo".to_string()];
        let p = plan(root.to_str().unwrap(), &pol);
        // MEDIDO 2026-11-22: el aserto original exigia que TODAS las entradas
        // fueran de "com.foo", pero el diseno es listar tambien las de otros
        // paquetes marcadas como NO incluidas (razon NOT_WHITELISTED).
        assert!(p
            .entries
            .iter()
            .filter(|e| e.included)
            .all(|e| e.package == "com.foo"));
        assert!(p
            .entries
            .iter()
            .any(|e| !e.included && e.reason == reason::NOT_WHITELISTED));

        let mut pol2 = BackupPolicy::default();
        pol2.exclude_packages = vec!["com.bar".to_string()];
        let p2 = plan(root.to_str().unwrap(), &pol2);
        let bar = p2.entries.iter().find(|e| e.package == "com.bar").unwrap();
        assert!(!bar.included);
        assert_eq!(bar.reason, reason::EXCLUDED_PACKAGE);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn los_topes_de_tamano_se_cumplen() {
        let root = fake_device();
        let mut pol = BackupPolicy::default();
        pol.max_entry_bytes = 3;
        let p = plan(root.to_str().unwrap(), &pol);
        assert!(p
            .entries
            .iter()
            .any(|e| !e.included && e.reason == reason::TOO_BIG));

        let mut pol2 = BackupPolicy::default();
        pol2.max_total_bytes = 4;
        let p2 = plan(root.to_str().unwrap(), &pol2);
        assert!(p2.truncated);
        assert!(p2.included_bytes <= 4);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn solo_apk_y_solo_datos() {
        let root = fake_device();
        let p = plan(root.to_str().unwrap(), &BackupPolicy::apk_only());
        assert!(p.entries.iter().all(|e| e.source != BackupSource::Data));
        let p2 = plan(root.to_str().unwrap(), &BackupPolicy::data_only());
        assert!(p2.entries.iter().all(|e| e.source != BackupSource::Apk));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn el_contenedor_ida_y_vuelta_y_se_verifica() {
        let root = fake_device();
        let p = plan(root.to_str().unwrap(), &BackupPolicy::default());
        let out = root.join("backup.mdbk");
        let mut opts = CaptureOptions::default();
        opts.passphrase = Some("clave-de-prueba".to_string());
        opts.kdf.iterations = 1;
        let r = capture(&p, out.to_str().unwrap(), &opts).expect("debería capturar");
        assert!(r.encrypted);
        assert!(r.bytes > 0);
        assert!(!r.manifest.entries.is_empty());

        let v = verify(out.to_str().unwrap(), Some("clave-de-prueba")).expect("debería verificar");
        assert!(v.ok, "{}", v.detail);
        assert!(v.payload_ok);
        assert!(v.manifest_ok);
        assert!(v.entries_bad.is_empty());
        assert_eq!(v.entries_checked, r.manifest.entries.len());

        // contraseña equivocada => error, no verificación "verde"
        assert!(verify(out.to_str().unwrap(), Some("mala")).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn la_corrupcion_se_detecta() {
        let root = fake_device();
        let p = plan(root.to_str().unwrap(), &BackupPolicy::default());
        let out = root.join("backup.mdbk");
        let mut opts = CaptureOptions::default();
        opts.passphrase = Some("k".to_string());
        opts.kdf.iterations = 1;
        capture(&p, out.to_str().unwrap(), &opts).unwrap();

        // toca un byte del payload
        let mut data = std::fs::read(&out).unwrap();
        let pos = data.len() - 10;
        data[pos] ^= 0xff;
        std::fs::write(&out, &data).unwrap();

        // MEDIDO 2026-11-22: al corromper un byte del ETIQUETA del ultimo
        // marco, el AEAD lo rechaza y `verify` devuelve Err (no un informe
        // con ok=false).  Las dos formas son "la corrupcion se detecta".
        match verify(out.to_str().unwrap(), Some("k")) {
            Ok(v) => {
                assert!(!v.ok, "la corrupción tiene que detectarse");
                assert!(!v.payload_ok || !v.entries_bad.is_empty());
            }
            Err(e) => {
                assert!(!e.trim().is_empty(), "la corrupción tiene que detectarse");
            }
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn el_restore_es_seco_por_defecto_y_exige_confirmacion() {
        let root = fake_device();
        let p = plan(root.to_str().unwrap(), &BackupPolicy::default());
        let out = root.join("backup.mdbk");
        let mut opts = CaptureOptions::default();
        opts.passphrase = Some("k".to_string());
        opts.kdf.iterations = 1;
        capture(&p, out.to_str().unwrap(), &opts).unwrap();

        let dest = root.join("restaurado");
        // sin dry-run y sin confirmar => error
        let r = restore(
            out.to_str().unwrap(),
            Some("k"),
            dest.to_str().unwrap(),
            &RestoreSelector::all(),
            &RestoreOptions {
                dry_run: false,
                confirm_write: false,
                overwrite: false,
                verify_after: true,
            },
        );
        assert!(r.is_err());

        // dry-run => no escribe nada
        let r = restore(
            out.to_str().unwrap(),
            Some("k"),
            dest.to_str().unwrap(),
            &RestoreSelector::all(),
            &RestoreOptions::default(),
        )
        .unwrap();
        assert!(r.dry_run);
        assert!(!r.written.is_empty());
        assert!(!dest.exists() || std::fs::read_dir(&dest).unwrap().count() == 0);

        // con confirmación => escribe
        let r = restore(
            out.to_str().unwrap(),
            Some("k"),
            dest.to_str().unwrap(),
            &RestoreSelector::all(),
            &RestoreOptions {
                dry_run: false,
                confirm_write: true,
                overwrite: true,
                verify_after: true,
            },
        )
        .unwrap();
        assert!(r.ok, "{}", r.detail);
        assert!(!r.written.is_empty());
        assert!(dest.join("payload/com.foo/data/shared_prefs/a.xml").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn el_restore_selectivo_solo_toca_lo_pedido() {
        let root = fake_device();
        let p = plan(root.to_str().unwrap(), &BackupPolicy::default());
        let out = root.join("backup.mdbk");
        let mut opts = CaptureOptions::default();
        opts.passphrase = Some("k".to_string());
        opts.kdf.iterations = 1;
        capture(&p, out.to_str().unwrap(), &opts).unwrap();

        let dest = root.join("restaurado2");
        let r = restore(
            out.to_str().unwrap(),
            Some("k"),
            dest.to_str().unwrap(),
            &RestoreSelector::package("com.bar"),
            &RestoreOptions {
                dry_run: false,
                confirm_write: true,
                overwrite: true,
                verify_after: true,
            },
        )
        .unwrap();
        assert!(r.ok);
        assert!(r.written.iter().all(|w| w.contains("com.bar")), "{:?}", r.written);
        assert!(dest.join("payload/com.bar/data/files/f.txt").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn el_comodin_simple_funciona() {
        assert!(glob_match("/cache/", "/data/user/0/com.foo/cache/x"));
        assert!(!glob_match("/cache/", "/data/user/0/com.foo/databases/x"));
        assert!(glob_match("*.db", "/x/y.db"));
        assert!(glob_match("/a/*/c", "/a/b/c"));
        assert!(!glob_match("/a/*/c", "/a/b/d"));
    }

    #[test]
    fn el_plan_de_subida_cubre_el_fichero_entero() {
        let root = fake_device();
        let p = plan(root.to_str().unwrap(), &BackupPolicy::default());
        let out = root.join("backup.mdbk");
        let mut opts = CaptureOptions::default();
        opts.passphrase = Some("k".to_string());
        opts.kdf.iterations = 1;
        capture(&p, out.to_str().unwrap(), &opts).unwrap();

        let creds = DriveCredentials {
            access_token: "token-de-prueba".to_string(),
        };
        let up = upload_plan(out.to_str().unwrap(), &creds, &DriveTarget::root(), 8 * 1024 * 1024).unwrap();
        assert_eq!(up.total, std::fs::metadata(&out).unwrap().len());
        assert!(!up.chunks.is_empty());
        // los trozos son contiguos y cubren el fichero
        let mut expect = 0u64;
        for c in &up.chunks {
            assert_eq!(c.start, expect);
            expect = c.end + 1;
        }
        assert_eq!(expect, up.total);
        let reqs = upload_chunk_requests(&creds, "https://session.example/1", &up);
        assert_eq!(reqs.len(), up.chunks.len());
        assert!(reqs[0].headers.iter().any(|(k, v)| k == "Content-Range" && v.contains("bytes 0-")));
        let _ = std::fs::remove_dir_all(&root);
    }
}
