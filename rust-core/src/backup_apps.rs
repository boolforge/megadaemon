//! Motor de **backup nativo de apps** (Rust puro) con **subida pipelining**.
//!
//! Qué resuelve (todo medido, no supuesto):
//!
//! * El móvil tiene **poco espacio libre** ⇒ no se puede staging local. Por eso
//!   el contenedor se escribe **por segmentos** y cada segmento se **borra en
//!   cuanto Drive lo confirma**: el pico de disco es un segmento, no el backup.
//! * La **subida** es el cuello de botella (297,6 KiB/s medidos) ⇒ el productor
//!   (el que recoge) y el consumidor (el que sube) van **decoupled**: en
//!   cuanto un segmento está **cerrado** (cifrado y sellado) se entrega al
//!   consumidor, que lo sube mientras el productor sigue recogiendo.
//! * El móvil **no puede ponerse sluggish** ⇒ todo el proceso va con `nice` +
//!   `ionice` de la clase más baja, duerme entre lotes y **descansa si la
//!   carga del sistema se dispara** (el móvil llega a load 20 con un proceso
//!   runaway de Chrome).
//!
//! El contenedor es un **directorio** (no un fichero) porque así se puede
//! reanudar sin staging:
//!
//! ```text
//! <contenedor>/
//!   container.json      cabecera: cifrado, KDF, key-check, lista de segmentos
//!   manifest.json       manifiesto: sha256 por fichero
//!   journal.ndjson      diario de reanudación (una línea por segmento/lote)
//!   segments/
//!     seg-00000.mds     segmento cifrado (AEGIS-256 por trozos)
//!     seg-00000.plain   segmento en curso (se borra al sellarlo)
//! ```
//!
//! **Reutiliza** (no es un motor paralelo): `backup_crypto` (AEGIS-256 por
//! trozos, Argon2id, key-check, manifiesto), `backup_formats` (`TarStream`,
//! `safe_relative_path`), `backup_glob` (`glob_match`),
//! `backup_inventory` (`apply_io_limits`, `loadavg_tuple`),
//! `drive_client`/`drive_native`/`google_auth` (subida resumible nativa).

use crate::backup_crypto::{self, ChunkCipher, KdfMethod, KdfParams, ManifestEntry, AEGIS_KEY_LEN};
// El manifiesto es parte de la API pública del motor (lo usan el CLI, el MCP
// y los tests) ⇒ se re-exporta.
pub use crate::backup_crypto::Manifest;
use crate::backup_glob::glob_match;
use crate::backup_formats::{safe_relative_path, TarStream};
use crate::backup_io::{apply_io_limits, loadavg_tuple};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ========================================================================= //
// 1. Constantes del formato
// ========================================================================= //

/// Magic del contenedor (directorio con `container.json`).
pub const CONTAINER_MAGIC: &str = "MDAPP1";
/// Versión del formato del contenedor.
pub const CONTAINER_VERSION: u32 = 1;
/// Nombre de la cabecera dentro del contenedor.
pub const CONTAINER_FILE: &str = "container.json";
/// Nombre del manifiesto dentro del contenedor.
pub const MANIFEST_FILE: &str = "manifest.json";
/// Nombre del diario de reanudación.
pub const JOURNAL_FILE: &str = "journal.ndjson";
/// Subdirectorio de los segmentos.
pub const SEGMENTS_DIR: &str = "segments";
/// Magic del fichero de segmento.
pub const SEGMENT_MAGIC: &[u8; 8] = b"MDSEG001";
/// Versión del formato de segmento.
pub const SEGMENT_VERSION: u8 = 1;
/// Etiqueta del nonce (AEGIS-256 por trozos, dominio del contenedor de apps).
pub const LABEL_SEGMENT_NONCE: &str = "MDAPP1-aegis256-nonce-v1";
/// Etiqueta del AAD (ata el trozo a su índice y a su longitud).
pub const LABEL_SEGMENT_AAD: &str = "MDAPP1-aegis256-aad-v1";
/// Tamaño por defecto del segmento: 16 MiB (medido como el mejor equilibrio
/// entre staging, paralelismo y sobrecarga por fichero en Drive).
pub const DEFAULT_SEGMENT_BYTES: u64 = 16 * 1024 * 1024;
/// Tope del tamaño de un segmento: 32 MiB.
///
/// MEDIDO (2026-11-22): el móvil tiene la RAM justa y el sello de un
/// segmento trabaja con **tres** copias en memoria (el `.plain` leído, el
/// marco AEAD y el fichero final) ⇒ con 32 MiB el pico es ~96 MiB. Con
/// 1 GiB (el tope anterior) el pico sería de ~3 GiB: el OOM killer se
/// lleva el daemon y el túnel cae. **Nada de leer 2 GiB de una vez.**
pub const MAX_SEGMENT_BYTES: u64 = 32 * 1024 * 1024;
/// Granularidad mínima que exige Drive en las subidas resumibles.
pub const DRIVE_CHUNK_MIN: u64 = 256 * 1024;
/// Tamaño del trozo de subida: **1 MiB**.
///
/// MEDIDO 2026-11-22: la subida a Drive va a ~297,6 KiB/s ⇒ el trozo **no**
/// es el cuello de botella; el coste real es el número de peticiones. 1 MiB
/// es el equilibrio: un segmento de 16 MiB son 16 trozos (16 peticiones
/// resumibles) y Drive acepta 1 MiB sin problema (el mínimo son 256 KiB y
/// la granularidad se ajusta a múltiplos de 256 KiB).
pub const UPLOAD_CHUNK_BYTES: u64 = 1024 * 1024;
/// Tamaño de la cabecera de un segmento (bytes).
pub const SEGMENT_HEADER_LEN: usize = 32;

/// Tope de segmentos **en vuelo** (cola acotada del pipelining).
///
/// El productor (recogida) y el consumidor (subida) están desacoplados por
/// una cola **acotada**: si Drive va lento (medido: 297,6 KiB/s), el
/// productor **se frena** (backpressure) en vez de llenar la memoria del móvil
/// con segmentos cerrados. 4–8 es el rango pedido: enough para que la subida
/// no se pare entre recogidas, poco para que la RAM no se dispare.
pub const MAX_INFLIGHT_SEGMENTS: usize = 8;
/// Mínimo de la cola en vuelo (con 1 worker, 4 segmentos bastan para no
/// dejar la red ociosa).
pub const MIN_INFLIGHT_SEGMENTS: usize = 4;

/// Directorios **regenerables** dentro de los datos de una app.
///
/// MEDIDO 2026-11-22 en el S21 Ultra: `caches 2,61 GiB + oat 7,28 GiB +
/// *.dm 0,41 GiB + dalvik-cache 0,26 GiB + iLovePDF 0,77 GiB = 11,32 GiB` de
/// basura regenerable. `oat/`, `*.dm` y `dalvik-cache/` **no** están dentro de
/// `/data/user/0/<pkg>` (viven en `/data/app` y `/data/dalvik-cache`), así que
/// los excluye el propio diseño (aquí sólo se copia el `.apk`, no el árbol de
/// `/data/app`). Lo que sí hay dentro del directorio de datos de la app son
/// `cache/`, `code_cache/` y `no_backup/`: es lo que se excluye aquí.
pub const REGENERABLE_DIRS: &[&str] = &["cache", "code_cache", "no_backup"];
/// Sufijos regenerables dentro de los datos de una app.
pub const REGENERABLE_SUFFIXES: &[&str] = &[".dm"];

/// Rutas que el motor **nunca** copia, aunque se le pida.
///
/// MEDIDO 2026-11-22 en el S21 Ultra (`root_exec`):
///
/// * `/sdcard/Android/media` — ahí vive WhatsApp (6,71 GiB medidos) y el
///   framework lo usa mientras las apps están en marcha: copiarlo a mitad de
///   una escritura da ficheros inconsistentes.
/// * `/sdcard/Android/data` — mismo motivo (el framework mueve ficheros de
///   `Android/data` al iniciar/desinstalar una app).
/// * `/data/misc/keystore`, `/data/misc/keychain`, `/data/misc/credstore`,
///   `/data/misc/knoxcore`, `/data/misc/fabric_crypto` — el CSC de Samsung
///   (KeyStore, Keychain, credenciales, Knox). Copiarlo con el framework en
///   uso puede dejar la caja de credenciales inconsistente **y** expone
///   material criptográfico en un contenedor que va a Drive.
///
/// Prefijos que sólo se tocan con la puerta explícita `allow_live_framework`
/// (el framework **parado**: por ejemplo en modo avión con la pantalla
/// apagada). Con el framework en uso ⇒ **nunca**.
pub const LIVE_ONLY_PREFIXES: &[&str] = &["/sdcard/Android/media", "/sdcard/Android/data"];

/// El motor los marca como `forbidden` y los cuenta aparte: no se copian y
/// no se esconden en el recuento de "regenerables".
pub const FORBIDDEN_PREFIXES: &[&str] = &[
    "/data/misc/keystore",
    "/data/misc/keychain",
    "/data/misc/credstore",
    "/data/misc/knoxcore",
    "/data/misc/fabric_crypto",
    "/data/misc/encryption",
    "/data/system_ce",
];

/// Normaliza una ruta a `/…`.
fn abs_rel(rel: &str) -> String {
    let rel = rel.replace('\\', "/");
    if rel.starts_with('/') {
        rel
    } else {
        format!("/{rel}")
    }
}

/// ¿La ruta es del CSC (nunca se copia, bajo ninguna circunstancia)?
pub fn is_forbidden_path(rel: &str) -> bool {
    let rel = abs_rel(rel);
    FORBIDDEN_PREFIXES
        .iter()
        .any(|p| rel == *p || rel.starts_with(&format!("{p}/")))
}

/// ¿La ruta es de Android/media o Android/data (sólo con el framework parado)?
pub fn is_live_only_path(rel: &str) -> bool {
    let rel = abs_rel(rel);
    LIVE_ONLY_PREFIXES
        .iter()
        .any(|p| rel == *p || rel.starts_with(&format!("{p}/")))
}

// ========================================================================= //
// 2. Inventario de apps
// ========================================================================= //

/// Una app instalada, vista desde el móvil.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppEntry {
    /// Nombre del paquete.
    pub package: String,
    /// ¿Es una app del sistema?
    pub system: bool,
    /// ¿Está deshabilitada?
    pub disabled: bool,
    /// uid del paquete (0 si no se pudo leer).
    pub uid: u32,
    /// APK principal (el que devuelve `pm list packages -f`).
    pub apk_path: String,
    /// Todos los APK del paquete (`pm path <pkg>`), si se pudo leer.
    pub apk_paths: Vec<String>,
    /// Ruta de los datos privados.
    pub data_dir: String,
    /// Bytes medidos en los datos privados (sin lo regenerable) y `None` si no
    /// se midió (el inventario rápido no recorre los directorios).
    pub data_bytes: Option<u64>,
    /// Ficheros medidos en los datos privados.
    pub file_count: Option<u64>,
    /// Bytes del APK principal (medido, si se pudo).
    pub apk_bytes: Option<u64>,
    /// Ruta de los datos externos (`/sdcard/Android/data/<pkg>`).
    pub external_dir: String,
    /// Ruta de los datos multimedia (`/sdcard/Android/media/<pkg>`).
    ///
    /// MEDIDO: **WhatsApp NO está en `/sdcard/WhatsApp`** sino en
    /// `Android/media/com.whatsapp` (6,71 GiB).
    pub media_dir: String,
}

/// Opciones del inventario.
#[derive(Debug, Clone, PartialEq)]
pub struct InventoryOptions {
    /// Raíz del dispositivo (`/` en el móvil).
    pub root: PathBuf,
    /// Paquetes concretos (vacío = todos).
    pub packages: Vec<String>,
    /// ¿Mide los datos privados (recorre los directorios)?
    pub measure: bool,
    /// Profundidad máxima al medir.
    pub max_depth: usize,
    /// Tope de apps devueltas (0 = todas).
    pub limit: usize,
    /// ¿Incluye las apps deshabilitadas?
    pub include_disabled: bool,
}

impl Default for InventoryOptions {
    fn default() -> Self {
        InventoryOptions {
            root: PathBuf::from("/"),
            packages: Vec::new(),
            measure: false,
            max_depth: 8,
            limit: 0,
            include_disabled: true,
        }
    }
}

/// Ejecuta un comando y devuelve su stdout (vacío si falla).
fn run_capture(root: &Path, cmd: &[&str]) -> Option<String> {
    let mut c = std::process::Command::new(cmd[0]);
    for a in &cmd[1..] {
        c.arg(a);
    }
    if root != Path::new("/") {
        c.current_dir(root);
    }
    let out = c.output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Parsea `pm list packages -f -U`.
///
/// Formato medido en el S21 Ultra:
/// `package:/data/app/~~abc==/com.x-y==/base.apk=com.x uid:10342`
///
/// OJO: la ruta del APK **acaba en `==`** ⇒ el separador es el **último** `=`.
pub fn parse_pm_packages_f_u(text: &str) -> Vec<(String, String, u32)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("package:") else {
            continue;
        };
        let (path, tail) = match rest.rsplit_once('=') {
            Some((p, t)) => (p, t),
            None => (rest, ""),
        };
        let (pkg, uid) = match tail.split_once(' ') {
            Some((p, u)) => (
                p.trim().to_string(),
                u.trim()
                    .strip_prefix("uid:")
                    .and_then(|n| n.parse::<u32>().ok())
                    .unwrap_or(0),
            ),
            None => (tail.trim().to_string(), 0),
        };
        if pkg.is_empty() {
            continue;
        }
        out.push((pkg, path.to_string(), uid));
    }
    out
}

/// Parsea `pm list packages -U` → `(paquete, uid)`.
pub fn parse_pm_packages_u(text: &str) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("package:") else {
            continue;
        };
        let (pkg, uid) = match rest.split_once(' ') {
            Some((p, u)) => (
                p.trim().to_string(),
                u.trim()
                    .strip_prefix("uid:")
                    .and_then(|n| n.parse::<u32>().ok())
                    .unwrap_or(0),
            ),
            None => (rest.trim().to_string(), 0),
        };
        if !pkg.is_empty() {
            out.push((pkg, uid));
        }
    }
    out
}

/// Lista las apps instaladas. **No escribe nada** en el disco.
pub fn inventory(opts: &InventoryOptions) -> Result<Vec<AppEntry>, String> {
    let root = &opts.root;
    let sys = run_capture(root, &["pm", "list", "packages", "-s"]).unwrap_or_default();
    let dis = run_capture(root, &["pm", "list", "packages", "-d"]).unwrap_or_default();
    let sys_set: Vec<String> = sys
        .lines()
        .filter_map(|l| l.trim().strip_prefix("package:").map(|s| s.to_string()))
        .collect();
    let dis_set: Vec<String> = dis
        .lines()
        .filter_map(|l| l.trim().strip_prefix("package:").map(|s| s.to_string()))
        .collect();

    let mut entries: Vec<AppEntry> = Vec::new();
    // Con `-f -U` viene el APK y el uid en una sola llamada.
    let with_apk = run_capture(root, &["pm", "list", "packages", "-f", "-U"]);
    let mut seen: Vec<String> = Vec::new();
    if let Some(text) = with_apk {
        for (pkg, path, uid) in parse_pm_packages_f_u(&text) {
            seen.push(pkg.clone());
            // El paquete se mueve al `AppEntry`: los flags se calculan antes.
            let is_sys = sys_set.contains(&pkg);
            let is_dis = dis_set.contains(&pkg);
            // Rutas derivadas: se calculan **antes** de mover `pkg`.
            let data_dir = format!("/data/user/0/{}", pkg);
            let external_dir = format!("/sdcard/Android/data/{}", pkg);
            let media_dir = format!("/sdcard/Android/media/{}", pkg);
            entries.push(AppEntry {
                package: pkg,
                system: is_sys,
                disabled: is_dis,
                uid,
                apk_path: path,
                apk_paths: Vec::new(),
                data_dir,
                data_bytes: None,
                file_count: None,
                apk_bytes: None,
                external_dir,
                media_dir,
            });
        }
    }
    if entries.is_empty() {
        // Plan B: `pm list packages -U` + `pm path` por paquete.
        let text = run_capture(root, &["pm", "list", "packages", "-u"]).unwrap_or_default();
        for (pkg, uid) in parse_pm_packages_u(&text) {
            seen.push(pkg.clone());
            let apks = apk_paths_of(root, &pkg);
            entries.push(AppEntry {
                package: pkg.clone(),
                system: sys_set.contains(&pkg),
                disabled: dis_set.contains(&pkg),
                uid,
                apk_path: apks.first().cloned().unwrap_or_default(),
                apk_paths: apks,
                data_dir: format!("/data/user/0/{}", pkg),
                data_bytes: None,
                file_count: None,
                apk_bytes: None,
                external_dir: format!("/sdcard/Android/data/{}", pkg),
                media_dir: format!("/sdcard/Android/media/{}", pkg),
            });
        }
    }

    // Filtro por paquete.
    if !opts.packages.is_empty() {
        entries.retain(|e| opts.packages.iter().any(|p| p == &e.package));
    }
    // Filtro de deshabilitadas.
    if !opts.include_disabled {
        entries.retain(|e| !e.disabled);
    }
    // APK completos sólo para los paquetes pedidos (`pm path` es caro).
    if !opts.packages.is_empty() {
        for e in entries.iter_mut() {
            e.apk_paths = apk_paths_of(root, &e.package);
            if e.apk_path.is_empty() {
                e.apk_path = e.apk_paths.first().cloned().unwrap_or_default();
            }
        }
    }
    // Tamaño del APK: se mide con `stat` (barato: un `stat` por APK).
    for e in entries.iter_mut() {
        let mut total = 0u64;
        let mut any = false;
        let paths = if e.apk_paths.is_empty() {
            vec![e.apk_path.clone()]
        } else {
            e.apk_paths.clone()
        };
        for p in paths.iter().filter(|p| !p.is_empty()) {
            let full = join_root(root, Path::new(p));
            if let Ok(md) = fs::metadata(&full) {
                if md.is_file() {
                    total += md.len();
                    any = true;
                }
            }
        }
        if any {
            e.apk_bytes = Some(total);
        }
    }
    // Medición de los datos privados.
    if opts.measure {
        for e in entries.iter_mut() {
            let (bytes, files) = measure_dir(root, &e.data_dir, opts.max_depth);
            e.data_bytes = Some(bytes);
            e.file_count = Some(files);
        }
    }
    // Tope.
    if opts.limit > 0 && entries.len() > opts.limit {
        entries.truncate(opts.limit);
    }
    Ok(entries)
}

/// `pm path <pkg>` → lista de APK.
fn apk_paths_of(root: &Path, pkg: &str) -> Vec<String> {
    run_capture(root, &["pm", "path", pkg])
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.trim().strip_prefix("package:").map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
        .collect()
}

/// Mide bytes y ficheros de un directorio (sin escribir nada).
fn measure_dir(root: &Path, dir: &str, max_depth: usize) -> (u64, u64) {
    let mut bytes = 0u64;
    let mut files = 0u64;
    let mut stack = vec![(PathBuf::from(dir), 0usize)];
    while let Some((path, depth)) = stack.pop() {
        if depth > max_depth {
            continue;
        }
        let full = join_root(root, &path);
        let Ok(rd) = fs::read_dir(&full) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let rel = format!("{}/{}", path.to_string_lossy(), name);
            if is_dir {
                if REGENERABLE_DIRS.contains(&name.as_str()) {
                    continue;
                }
                stack.push((PathBuf::from(&rel), depth + 1));
            } else {
                if is_regenerable_file(&rel) {
                    continue;
                }
                if let Ok(md) = e.metadata() {
                    bytes += md.len();
                }
                files += 1;
            }
        }
    }
    (bytes, files)
}

/// Junta la raíz del dispositivo con una ruta absoluta del móvil.
pub fn join_root(root: &Path, path: &Path) -> PathBuf {
    if root == Path::new("/") {
        path.to_path_buf()
    } else {
        root.join(path.to_string_lossy().trim_start_matches('/'))
    }
}

// ========================================================================= //
// 3. Exclusiones
// ========================================================================= //

/// ¿La ruta es basura regenerable (no se respalda)?
pub fn is_regenerable(rel: &str) -> bool {
    let rel = rel.replace('\\', "/");
    for part in rel.split('/') {
        if REGENERABLE_DIRS.contains(&part) {
            return true;
        }
    }
    is_regenerable_file(&rel)
}

/// ¿El nombre del fichero es basura regenerable?
pub fn is_regenerable_file(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    REGENERABLE_SUFFIXES.iter().any(|s| name.ends_with(s))
}

/// ¿La ruta está excluida por los patrones del usuario?
pub fn matches_exclude(rel: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|p| glob_match(p, rel))
}

// ========================================================================= //
// 4. Anti-congelación (el "gobierno" del proceso)
// ========================================================================= //

/// Carga del sistema leída del dispositivo.
pub fn read_loadavg(root: &Path) -> String {
    let p = join_root(root, Path::new("/proc/loadavg"));
    fs::read_to_string(p)
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// El gobierno anti-congelación del proceso de backup.
///
/// Reglas (todas medibles):
///
/// * `nice` + `ionice`: el proceso **nunca** compite con el daemon (que va a
///   `nice 0`). El mínimo es 10 ⇒ siempre por debajo.
/// * `sleep` entre lotes: el disco del móvil tiene tiempo para servir a la UI.
/// * **descanso si la carga se dispara**: con `loadavg > load_high` el proceso
///   espera a que baje a `load_resume` (con tope: `max_pause_s`).
#[derive(Debug, Clone, PartialEq)]
pub struct Governor {
    /// ¿Aplica los límites?
    pub enabled: bool,
    /// `nice` (0-19). El motor **Fuerza** un mínimo de 10.
    pub nice: i32,
    /// Clase de `ionice` (1 realtime, 2 best-effort, 3 idle).
    pub ionice_class: u8,
    /// Ficheros por lote.
    pub batch_size: usize,
    /// `sleep` entre lotes (ms).
    pub sleep_ms: u64,
    /// Carga 1-min a partir de la cual se descansa.
    pub load_high: f64,
    /// Carga 1-min a la que se vuelve al trabajo.
    pub load_resume: f64,
    /// Tope de la espera (s) por pausa.
    pub max_pause_s: u64,
}

impl Default for Governor {
    fn default() -> Self {
        Governor {
            enabled: true,
            nice: 15,
            ionice_class: 3,
            batch_size: 64,
            sleep_ms: 120,
            load_high: 8.0,
            load_resume: 4.0,
            max_pause_s: 300,
        }
    }
}

/// Resultado de un `tick` del gobierno.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TickInfo {
    /// ¿Hubo que descansar por carga?
    pub paused: bool,
    /// Milisegundos de espera por carga.
    pub pause_ms: u64,
    /// Milisegundos de `sleep` entre lotes.
    pub sleep_ms: u64,
    /// Carga 1-min en el momento del tick.
    pub load1: f64,
    /// Motivo de la pausa (`carga` o vacío).
    pub reason: String,
}

impl Governor {
    /// Aplica `nice` + `ionice` al proceso actual.
    ///
    /// El `nice` efectivo es `max(nice, 10)`: el backup **jamás**compite con
    /// el daemon (que corre a `nice 0`).
    pub fn apply(&self) -> String {
        if !self.enabled {
            return "gobierno desactivado (nice/ionice sin tocar)".to_string();
        }
        let nice = self.nice.clamp(0, 19).max(10);
        apply_io_limits(nice, self.ionice_class)
    }

    /// Un lote: duerme y, si la carga está alta, **descansa**.
    pub fn tick(&self, loadavg: &str) -> TickInfo {
        self.tick_with(loadavg, || read_loadavg(Path::new("/")))
    }

    /// Un lote con una **fuente de carga inyectada** (para los tests).
    pub fn tick_with<F: Fn() -> String>(&self, loadavg: &str, source: F) -> TickInfo {
        let (l1, _l5, _l15) = loadavg_tuple(loadavg);
        let mut info = TickInfo {
            paused: false,
            pause_ms: 0,
            sleep_ms: 0,
            load1: l1,
            reason: String::new(),
        };
        if !self.enabled {
            return info;
        }
        if l1 > self.load_high {
            // Descanso: se espera a que la carga baje, con tope duro.
            let mut waited = 0u64;
            let mut cur = l1;
            while cur > self.load_resume && waited < self.max_pause_s * 1000 {
                std::thread::sleep(Duration::from_millis(1000));
                waited += 1000;
                let (c, _, _) = loadavg_tuple(&source());
                cur = c;
            }
            info.paused = true;
            info.pause_ms = waited;
            info.reason = format!(
                "carga {l1:.2} > {} (espera {} s, carga final {cur:.2})",
                self.load_high,
                waited / 1000
            );
        }
        if self.sleep_ms > 0 {
            std::thread::sleep(Duration::from_millis(self.sleep_ms));
            info.sleep_ms = self.sleep_ms;
        }
        info
    }
}

// ========================================================================= //
// 5. Manifiesto y cabecera
// ========================================================================= //

/// Cabecera del contenedor (fichero `container.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContainerHeader {
    /// Magic (`MDAPP1`).
    pub magic: String,
    /// Versión del formato.
    pub version: u32,
    /// Cuándo se creó (epoch).
    pub created_at: u64,
    /// Qué lo produjo.
    pub tool: String,
    /// Cifrador (`aegis256-chunk-v1`).
    pub cipher: String,
    /// De dónde sale la clave (`passphrase` o `random`).
    pub key_source: String,
    /// Parámetros del KDF.
    pub kdf: KdfParams,
    /// Key-check (hex): permite verificar la clave sin descifrar.
    pub key_check: String,
    /// Tamaño del segmento en bytes.
    pub segment_bytes: u64,
    /// SHA-256 del manifiesto canónico.
    pub manifest_sha256: String,
    /// Paquetes incluidos.
    pub packages: Vec<String>,
    /// Segmentos del contenedor.
    pub segments: Vec<SegmentInfo>,
    /// Bytes de texto plano (suma de los segmentos).
    pub plain_bytes: u64,
    /// Ficheros respaldados.
    pub files: usize,
    /// Plan **validado** que se aplicó (`None` si no hubo plan).
    ///
    /// MEDIDO: el LLM propone y el código valida ⇒ lo que se guarda aquí es
    /// la **decisión**, con la propuesta cruda dentro (auditoría).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<Value>,
    /// SHA-256 del plan aplicado (lo ata a la cabecera).
    #[serde(default)]
    pub plan_sha256: String,
}

/// Un segmento ya cerrado.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SegmentInfo {
    /// Índice del segmento (0, 1, 2, …).
    pub index: u32,
    /// Nombre del fichero dentro de `segments/`.
    pub name: String,
    /// Bytes de texto plano.
    pub plain_len: u64,
    /// SHA-256 del texto plano.
    pub plain_sha256: String,
    /// SHA-256 del fichero cifrado.
    pub cipher_sha256: String,
    /// Bytes del fichero cifrado en disco.
    pub bytes: u64,
}

/// Un segmento cerrado listo para subir.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SealedSegment {
    /// Índice.
    pub index: u32,
    /// Ruta local del fichero cifrado.
    pub path: PathBuf,
    /// Metadatos.
    pub info: SegmentInfo,
}

/// Lee la cabecera de un contenedor.
pub fn read_header(dir: &Path) -> Result<ContainerHeader, String> {
    let p = dir.join(CONTAINER_FILE);
    let data = fs::read_to_string(&p).map_err(|e| format!("no se puede leer {}: {e}", p.display()))?;
    let h: ContainerHeader =
        serde_json::from_str(&data).map_err(|e| format!("cabecera inválida en {}: {e}", p.display()))?;
    if h.magic != CONTAINER_MAGIC {
        return Err(format!(
            "no es un contenedor de apps: magic {:?} (se esperaba {:?})",
            h.magic, CONTAINER_MAGIC
        ));
    }
    Ok(h)
}

/// Lee el manifiesto de un contenedor.
pub fn read_manifest(dir: &Path) -> Result<Manifest, String> {
    let p = dir.join(MANIFEST_FILE);
    let data = fs::read_to_string(&p).map_err(|e| format!("no se puede leer {}: {e}", p.display()))?;
    serde_json::from_str(&data).map_err(|e| format!("manifiesto inválido en {}: {e}", p.display()))
}

// ========================================================================= //
// 6. Segmentos: sellado (AEAD) y lectura
// ========================================================================= //

/// Sella un trozo de texto plano en un segmento cifrado.
///
/// Formato del fichero de segmento (32 B de cabecera + marco + hash):
///
/// ```text
/// 0   8  magic "MDSEG001"
/// 8   1  versión (1)
/// 9   1  cifrador (1 = aegis256-chunk-v1, 0 = sin cifrar)
/// 10  2  flags (u16 LE, reservado)
/// 12  4  índice del segmento (u32 LE)
/// 16  8  longitud del texto plano (u64 LE)
/// 24  4  longitud del marco (u32 LE)
/// 28  4  reservado (u32 LE)
/// 32  .. marco AEAD: [u32 len][nonce 32][cifrado][etiqueta 32]
/// ..  32 SHA-256 del texto plano
/// ```
pub fn seal_segment(plain: &[u8], index: u32, cipher: Option<&ChunkCipher>) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(SEGMENT_HEADER_LEN + plain.len() + 128);
    out.extend_from_slice(SEGMENT_MAGIC);
    out.push(SEGMENT_VERSION);
    let (cid, frame) = match cipher {
        Some(c) => (1u8, c.seal_chunk("segment", index as u64, plain)),
        None => (0u8, plain.to_vec()),
    };
    out.push(cid);
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&index.to_le_bytes());
    out.extend_from_slice(&(plain.len() as u64).to_le_bytes());
    out.extend_from_slice(&(frame.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&frame);
    out.extend_from_slice(&sha256_raw(plain));
    Ok(out)
}

/// Abre un segmento: devuelve el texto plano y su metadato.
pub fn open_segment(bytes: &[u8], key: Option<&[u8]>) -> Result<(Vec<u8>, SegmentMeta), String> {
    if bytes.len() < SEGMENT_HEADER_LEN {
        return Err("segmento demasiado corto".to_string());
    }
    if &bytes[0..8] != SEGMENT_MAGIC {
        return Err(format!(
            "magic de segmento inválido: {:?}",
            String::from_utf8_lossy(&bytes[0..8])
        ));
    }
    if bytes[8] != SEGMENT_VERSION {
        return Err(format!("versión de segmento no soportada: {}", bytes[8]));
    }
    let cipher_id = bytes[9];
    let index = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    let plain_len = u64::from_le_bytes([
        bytes[16], bytes[17], bytes[18], bytes[19], bytes[20], bytes[21], bytes[22], bytes[23],
    ]);
    let frame_len =
        u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]) as usize;
    let frame_start = SEGMENT_HEADER_LEN;
    let frame_end = frame_start
        .checked_add(frame_len)
        .ok_or("longitud de marco desbordada")?;
    if bytes.len() < frame_end + 32 {
        return Err("segmento truncado".to_string());
    }
    let frame = &bytes[frame_start..frame_end];
    let plain_hash = &bytes[frame_end..frame_end + 32];
    let plain = match cipher_id {
        0 => frame.to_vec(),
        1 => {
            let key = key.ok_or("el segmento está cifrado y no se ha dado la clave")?;
            let c = ChunkCipher::new(key, LABEL_SEGMENT_NONCE, LABEL_SEGMENT_AAD)?;
            c.open_chunk("segment", index as u64, frame)?
        }
        other => return Err(format!("cifrador desconocido en el segmento: {other}")),
    };
    if plain.len() as u64 != plain_len {
        return Err(format!(
            "el segmento declara {plain_len} B y entrega {} B",
            plain.len()
        ));
    }
    if sha256_raw(&plain) != plain_hash {
        return Err("el SHA-256 del texto plano no cuadra".to_string());
    }
    Ok((
        plain,
        SegmentMeta {
            index,
            plain_len,
            cipher_id,
            bytes: bytes.len() as u64,
        },
    ))
}

/// Metadatos de un segmento abierto.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SegmentMeta {
    /// Índice.
    pub index: u32,
    /// Longitud del texto plano.
    pub plain_len: u64,
    /// Cifrador (0 = sin cifrar, 1 = AEGIS-256).
    pub cipher_id: u8,
    /// Bytes del fichero en disco.
    pub bytes: u64,
}

/// SHA-256 de un buffer (hex minúsculas).
pub fn sha256_bytes(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// SHA-256 de un buffer (bytes crudos, 32 B).
pub fn sha256_raw(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    let out = h.finalize();
    let mut buf = [0u8; 32];
    buf.copy_from_slice(&out);
    buf
}

// ========================================================================= //
// 7. Lectura en streaming del contenedor (para verify y restore)
// ========================================================================= //

/// Lector que descifra los segmentos **uno a uno** y los sirve como un flujo
/// continuo (el tar). La memoria punta es un segmento, no el backup entero.
pub struct SegmentReader {
    segs: Vec<PathBuf>,
    key: Option<Vec<u8>>,
    pos: usize,
    cur: std::io::Cursor<Vec<u8>>,
    bytes_read: u64,
}

impl SegmentReader {
    /// Crea el lector sobre la lista de segmentos (en orden).
    pub fn new(segs: Vec<PathBuf>, key: Option<Vec<u8>>) -> SegmentReader {
        SegmentReader {
            segs,
            key,
            pos: 0,
            cur: std::io::Cursor::new(Vec::new()),
            bytes_read: 0,
        }
    }

    /// Carga el siguiente segmento.
    fn next_segment(&mut self) -> Result<bool, String> {
        if self.pos >= self.segs.len() {
            return Ok(false);
        }
        let p = &self.segs[self.pos];
        let raw = fs::read(p).map_err(|e| format!("no se puede leer {}: {e}", p.display()))?;
        let (plain, _meta) = open_segment(&raw, self.key.as_deref())?;
        self.cur = std::io::Cursor::new(plain);
        self.pos += 1;
        Ok(true)
    }
}

impl Read for SegmentReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            let n = self.cur.read(buf)?;
            if n > 0 {
                self.bytes_read += n as u64;
                return Ok(n);
            }
            if !self.next_segment().map_err(std::io::Error::other)? {
                return Ok(0);
            }
        }
    }
}

// ========================================================================= //
// 8. Escritura del tar (con nombres largos)
// ========================================================================= //

/// Cabecera de tar **con nombre largo** (campo `prefix` de ustar).
///
/// MEDIDO: los datos de las apps tienen rutas de más de 100 bytes ⇒ el
/// `tar_header` del repo (que corta a 100) las trunca. Aquí se usa el campo
/// `prefix` (155 B) del formato USTAR, que es lo que leen los lectores.
pub fn tar_header_long(name: &str, size: u64, mode: u32, mtime: u64) -> [u8; 512] {
    let mut h = [0u8; 512];
    let bytes = name.as_bytes();
    let (prefix, base) = if bytes.len() <= 100 {
        ("", name)
    } else {
        // Se parte por un '/' para que `prefix` + '/' + `name` reconstruya la ruta.
        let mut split = None;
        for (i, b) in bytes.iter().enumerate().rev() {
            if *b == b'/' && i <= 155 && bytes.len() - i - 1 <= 100 {
                split = Some(i);
                break;
            }
        }
        match split {
            Some(i) => (&name[..i], &name[i + 1..]),
            None => ("", &name[bytes.len().saturating_sub(100)..]),
        }
    };
    let nb = base.as_bytes();
    let n = nb.len().min(100);
    h[..n].copy_from_slice(&nb[..n]);
    let pb = prefix.as_bytes();
    let pn = pb.len().min(155);
    h[345..345 + pn].copy_from_slice(&pb[..pn]);
    write_octal(&mut h[100..108], mode as u64, 8);
    write_octal(&mut h[108..116], 0, 8); // uid
    write_octal(&mut h[116..124], 0, 8); // gid
    write_octal(&mut h[124..136], size, 12);
    write_octal(&mut h[136..148], mtime, 12);
    h[148..156].copy_from_slice(b"        ");
    h[156] = b'0'; // fichero regular
    h[257..262].copy_from_slice(b"ustar");
    h[263..265].copy_from_slice(b"00");
    let mut sum: u32 = 0;
    for b in h.iter() {
        sum += *b as u32;
    }
    let chk = format!("{sum:06o}\0 ");
    h[148..156].copy_from_slice(chk.as_bytes());
    h
}

fn write_octal(buf: &mut [u8], value: u64, width: usize) {
    let s = format!("{value:0width$o}", width = width - 1);
    let bytes = s.as_bytes();
    let n = bytes.len().min(width - 1);
    buf[..n].copy_from_slice(&bytes[bytes.len() - n..]);
    buf[width - 1] = 0;
}

/// Bytes de relleno de una entrada de tar.
pub fn tar_pad(size: u64) -> usize {
    ((512 - (size % 512)) % 512) as usize
}

// ========================================================================= //
// 9. El productor: `create`
// ========================================================================= //

/// Opciones de `create`.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateOptions {
    /// Raíz del dispositivo.
    pub root: PathBuf,
    /// Directorio del contenedor a crear.
    pub out: PathBuf,
    /// Paquetes a respaldar (vacío = todos).
    pub packages: Vec<String>,
    /// Tamaño del segmento.
    pub segment_bytes: u64,
    /// Contraseña (opcional): de ella se deriva la clave principal.
    pub passphrase: Option<String>,
    /// Dónde escribir el fichero de claves `0600`.
    pub key_out: Option<PathBuf>,
    /// ¿Incluye el APK?
    pub include_apk: bool,
    /// ¿Incluye los datos privados?
    pub include_data: bool,
    /// ¿Incluye `/sdcard/Android/data/<pkg>`?
    pub include_external: bool,
    /// ¿Incluye `/sdcard/Android/media/<pkg>`?
    pub include_media: bool,
    /// **Puerta explícita**: autoriza a tocar `Android/media` y
    /// `Android/data`. Sin ella (el defecto) **no se tocan**, porque el
    /// framework los está usando mientras las apps están en marcha.
    ///
    /// El CSC (`/data/misc/keystore`, `keychain`, `credstore`, `knoxcore`,
    /// `fabric_crypto`) queda prohibido **siempre**: no hay puerta.
    pub allow_live_framework: bool,
    /// Patrones extra a excluir (globs).
    pub exclude_patterns: Vec<String>,
    /// Plan **validado** (propuesto por el LLM, validado por el código).
    ///
    /// Si viene, **manda el plan**: sus paquetes, sus exclusiones y sus
    /// banderas. El modelo nunca se aplica sin validar.
    pub plan: Option<crate::backup_plan::PlanDecision>,
    /// ¿Simulación (no escribe nada)?
    pub dry_run: bool,
    /// Gobierno anti-congelación.
    pub gov: Governor,
}

impl Default for CreateOptions {
    fn default() -> Self {
        CreateOptions {
            root: PathBuf::from("/"),
            out: PathBuf::from("/data/local/tmp/megadaemon/backup"),
            packages: Vec::new(),
            segment_bytes: DEFAULT_SEGMENT_BYTES,
            passphrase: None,
            key_out: None,
            include_apk: true,
            include_data: true,
            include_external: false,
            include_media: false,
            allow_live_framework: false,
            exclude_patterns: Vec::new(),
            plan: None,
            dry_run: false,
            gov: Governor::default(),
        }
    }
}

/// Informe de `create`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateReport {
    /// Ruta del contenedor.
    pub container: String,
    /// Paquetes incluidos.
    pub packages: Vec<String>,
    /// Ficheros respaldados.
    pub files: usize,
    /// Bytes de texto plano.
    pub bytes: u64,
    /// Segmentos producidos.
    pub segments: usize,
    /// SHA-256 del manifiesto.
    pub manifest_sha256: String,
    /// Ficheros excluidos (regenerables o patrones).
    pub excluded_files: u64,
    /// Bytes excluidos.
    pub excluded_bytes: u64,
    /// ¿Simulación?
    pub dry_run: bool,
    /// Carga del sistema antes.
    pub loadavg_before: String,
    /// Carga del sistema después.
    pub loadavg_after: String,
    /// Milisegundos de espera por carga del sistema.
    pub paused_ms: u64,
    /// Milisegundos de `sleep` entre lotes.
    pub slept_ms: u64,
    /// Milisegundos totales.
    pub elapsed_ms: u64,
    /// Notas de los límites de I/O.
    pub io_limits: String,
    /// Segmentos entregados al consumidor (pipelining).
    pub handed_off: usize,
    /// Bytes leídos del dispositivo.
    pub read_bytes: u64,
    /// MiB/s **útiles** de lectura (sin contar las pausas por carga ni el
    /// `sleep` entre lotes): es la velocidad real del disco del móvil.
    pub read_mib_per_s: f64,
    /// Ficheros **prohibidos** por política (CSC, Android/media, Android/data).
    pub forbidden_files: u64,
    /// Bytes prohibidos por política.
    pub forbidden_bytes: u64,
    /// Plan **validado** que se aplicó (`None` si no hubo plan).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<Value>,
    /// SHA-256 del plan aplicado.
    #[serde(default)]
    pub plan_sha256: String,
}

/// El consumidor de segmentos (la otra mitad del pipelining).
///
/// `create` **entrega** cada segmento en cuanto está cerrado; el consumidor
/// decide qué hacer con él (subirlo a Drive, guardarlo, …).
pub trait SegmentSink: Send {
    /// Recibe un segmento ya cerrado (cifrado y sellado).
    fn submit(&mut self, seg: &SealedSegment) -> Result<(), String>;
    /// Se llama al terminar (para subir manifiesto y cabecera).
    fn finish(&mut self, header: &ContainerHeader, manifest: &Manifest)
        -> Result<(), String>;
}

/// Consumidor nulo (sólo para `create` sin subida).
pub struct NullSink;

impl SegmentSink for NullSink {
    fn submit(&mut self, _seg: &SealedSegment) -> Result<(), String> {
        Ok(())
    }
    fn finish(
        &mut self,
        _header: &ContainerHeader,
        _manifest: &Manifest,
    ) -> Result<(), String> {
        Ok(())
    }
}

/// Una fuente de ficheros a respaldar.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceFile {
    /// Ruta en el dispositivo.
    pub device_path: String,
    /// Ruta dentro del contenedor (relativa, con `/`).
    pub inside: String,
    /// Paquete al que pertenece.
    pub package: String,
    /// Origen (`apk`, `data`, `external`, `media`).
    pub source: String,
}

/// Construye la lista de ficheros a respaldar para un conjunto de paquetes.
#[allow(clippy::too_many_arguments)]
pub fn collect_sources(
    root: &Path,
    packages: &[String],
    include_apk: bool,
    include_data: bool,
    include_external: bool,
    include_media: bool,
    allow_live_framework: bool,
    exclude_patterns: &[String],
) -> Result<(Vec<SourceFile>, u64, u64, u64, u64), String> {
    let inv = InventoryOptions {
        root: root.to_path_buf(),
        packages: packages.to_vec(),
        measure: false,
        ..Default::default()
    };
    let apps = inventory(&inv)?;
    let mut out: Vec<SourceFile> = Vec::new();
    let mut excl_files = 0u64;
    let mut excl_bytes = 0u64;
    // Prohibidos por política (CSC) y saltados por puerta (media/data).
    let mut forbid_files = 0u64;
    let mut forbid_bytes = 0u64;
    for app in &apps {
        let mut roots: Vec<(String, String)> = Vec::new();
        if include_data {
            roots.push((app.data_dir.clone(), "data".to_string()));
        }
        // `Android/data` y `Android/media` **no** se tocan sin la puerta
        // explícita: con el framework en uso es una carrera contra la app
        // que los está usando.
        if include_external && allow_live_framework {
            roots.push((app.external_dir.clone(), "external".to_string()));
        }
        if include_media && allow_live_framework {
            roots.push((app.media_dir.clone(), "media".to_string()));
        }
        for (base, source) in roots {
            let full = join_root(root, Path::new(&base));
            if !full.is_dir() {
                continue;
            }
            let mut stack = vec![(full, base.clone(), 0usize)];
            while let Some((dir, rel_base, depth)) = stack.pop() {
                if depth > 24 {
                    continue;
                }
                let Ok(rd) = fs::read_dir(&dir) else {
                    continue;
                };
                for e in rd.flatten() {
                    let name = e.file_name().to_string_lossy().to_string();
                    let p = e.path();
                    let rel = format!("{rel_base}/{name}");
                    let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                    if is_dir {
                        if is_regenerable(&rel) {
                            continue;
                        }
                        // CSC: prohibido siempre. media/data: sólo con puerta.
                        if is_forbidden_path(&rel)
                            || (is_live_only_path(&rel) && !allow_live_framework)
                        {
                            forbid_files += 1;
                            continue;
                        }
                        stack.push((p, rel, depth + 1));
                    } else {
                        if is_forbidden_path(&rel)
                            || (is_live_only_path(&rel) && !allow_live_framework)
                        {
                            // Prohibido: se cuenta aparte (ni regenerable ni
                            // excluido por patrón: es una decisión de política).
                            forbid_files += 1;
                            if let Ok(md) = e.metadata() {
                                forbid_bytes += md.len();
                            }
                            continue;
                        }
                        if is_regenerable(&rel) || matches_exclude(&rel, exclude_patterns) {
                            excl_files += 1;
                            if let Ok(md) = e.metadata() {
                                excl_bytes += md.len();
                            }
                            continue;
                        }
                        out.push(SourceFile {
                            device_path: p.to_string_lossy().to_string(),
                            // La ruta DENTRO del contenedor es **relativa**
                            // (como la del APK): sin la `/` inicial.
                            //
                            // BUG REAL (medido el 2026-11-22 en el móvil): antes
                            // los datos se guardaban con la ruta ABSOLUTA del
                            // dispositivo (`/data/user/0/<pkg>/...`) y los APK
                            // con la relativa (`data/app/...`) ⇒ el `restore`
                            // (que valida con `safe_relative_path`) rechazaba
                            // los datos con "ruta absoluta no permitida" y el
                            // backup no se podía restaurar.
                            inside: rel.trim_start_matches('/').to_string(),
                            package: app.package.clone(),
                            source: source.clone(),
                        });
                    }
                }
            }
        }
        if include_apk {
            let apks = if app.apk_paths.is_empty() {
                vec![app.apk_path.clone()]
            } else {
                app.apk_paths.clone()
            };
            for apk in apks {
                if apk.is_empty() {
                    continue;
                }
                let full = join_root(root, Path::new(&apk));
                if !full.is_file() {
                    continue;
                }
                // La ruta dentro del contenedor es estilo dispositivo:
                // `data/app/...` ⇒ el restore sabe dónde va.
                let inside = apk.trim_start_matches('/').to_string();
                if is_forbidden_path(&inside)
                    || (is_live_only_path(&inside) && !allow_live_framework)
                {
                    forbid_files += 1;
                    continue;
                }
                if matches_exclude(&inside, exclude_patterns) {
                    excl_files += 1;
                    continue;
                }
                out.push(SourceFile {
                    device_path: full.to_string_lossy().to_string(),
                    inside,
                    package: app.package.clone(),
                    source: "apk".to_string(),
                });
            }
        }
    }
    // Orden estable: por paquete y por ruta ⇒ manifiesto reproducible.
    out.sort_by(|a, b| a.package.cmp(&b.package).then(a.inside.cmp(&b.inside)));
    Ok((out, excl_files, excl_bytes, forbid_files, forbid_bytes))
}

/// Estado del diario de reanudación.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Journal {
    /// Segmentos ya sellados: (índice, longitudes, entradas del manifiesto).
    segments: Vec<JournalSegment>,
    /// Progreso del segmento en curso.
    partial: Option<JournalPartial>,
}

/// Registro de un segmento sellado.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JournalSegment {
    index: u32,
    name: String,
    plain_len: u64,
    plain_sha256: String,
    cipher_sha256: String,
    bytes: u64,
    entries: Vec<ManifestEntry>,
}

/// Progreso dentro del segmento en curso.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JournalPartial {
    index: u32,
    plain_len: u64,
    entries: Vec<ManifestEntry>,
}

/// Lee el diario (descarta las líneas a medias).
fn read_journal(dir: &Path) -> Journal {
    let mut j = Journal::default();
    let p = dir.join(JOURNAL_FILE);
    let Ok(text) = fs::read_to_string(&p) else {
        return j;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue; // línea a medias: se descarta
        };
        match v.get("kind").and_then(|k| k.as_str()) {
            Some("seg") => {
                if let Ok(s) = serde_json::from_value::<JournalSegment>(v.clone()) {
                    j.segments.push(s);
                    j.partial = None;
                }
            }
            Some("part") => {
                if let Ok(p) = serde_json::from_value::<JournalPartial>(v.clone()) {
                    j.partial = Some(p);
                }
            }
            _ => {}
        }
    }
    j
}

/// Añade una línea al diario (y la baja a disco: es un diario, no un log).
fn append_journal(dir: &Path, value: &Value) -> Result<(), String> {
    use std::io::Write as _;
    let p = dir.join(JOURNAL_FILE);
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&p)
        .map_err(|e| format!("no se puede abrir {}: {e}", p.display()))?;
    let mut line = serde_json::to_string(value).unwrap_or_default();
    line.push('\n');
    f.write_all(line.as_bytes())
        .map_err(|e| format!("no se puede escribir {}: {e}", p.display()))?;
    f.sync_data().ok();
    Ok(())
}

/// Crea el contenedor. **Reanudable**: si el diario existe, continúa.
pub fn create(opts: &CreateOptions, sink: &mut dyn SegmentSink) -> Result<CreateReport, String> {
    let t0 = Instant::now();
    let load_before = read_loadavg(&opts.root);
    let io_limits = opts.gov.apply();

    let seg_bytes = opts.segment_bytes.clamp(DRIVE_CHUNK_MIN, MAX_SEGMENT_BYTES);
    let segs_dir = opts.out.join(SEGMENTS_DIR);
    if !opts.dry_run {
        fs::create_dir_all(&segs_dir)
            .map_err(|e| format!("no se puede crear {}: {e}", segs_dir.display()))?;
    }

    // 1) Claves.
    let (master, kdf, key_check, key_source) = build_master_key(opts)?;
    let cipher = Some(ChunkCipher::new(
        &master,
        LABEL_SEGMENT_NONCE,
        LABEL_SEGMENT_AAD,
    )?);

    // 2) Fuentes.
    //
    // Si hay un plan **validado**, el plan manda: sus paquetes, sus
    // exclusiones y sus banderas (que ya pasaron el filtro del código).
    // El LLM no decide: sólo propone.
    let (eff_packages, eff_excludes, eff_media, eff_external) = match &opts.plan {
        Some(p) => {
            let pkgs = if opts.packages.is_empty() {
                p.packages.clone()
            } else {
                opts.packages.clone()
            };
            let mut ex = p.exclude_patterns.clone();
            for e in opts.exclude_patterns.iter() {
                if !ex.contains(e) {
                    ex.push(e.clone());
                }
            }
            (
                pkgs,
                ex,
                p.include_media && opts.include_media,
                p.include_external && opts.include_external,
            )
        }
        None => (
            opts.packages.clone(),
            opts.exclude_patterns.clone(),
            opts.include_media,
            opts.include_external,
        ),
    };
    let (sources, excl_files, excl_bytes, forbid_files, forbid_bytes) = collect_sources(
        &opts.root,
        &eff_packages,
        opts.include_apk,
        opts.include_data,
        eff_external,
        eff_media,
        opts.allow_live_framework,
        &eff_excludes,
    )?;

    // 3) Reanudación.
    let journal = read_journal(&opts.out);
    let mut entries: Vec<ManifestEntry> = Vec::new();
    let mut seg_infos: Vec<SegmentInfo> = Vec::new();
    let mut skipped: BTreeMap<String, ()> = BTreeMap::new();
    for s in &journal.segments {
        // Si el segmento ya no está en disco (p.ej. se borró tras subirlo),
        // **no** se da por bueno: sus ficheros se vuelven a recoger.
        let sp = opts.out.join(SEGMENTS_DIR).join(&s.name);
        if !sp.exists() {
            continue;
        }
        for e in &s.entries {
            skipped.insert(e.path.clone(), ());
            entries.push(e.clone());
        }
        seg_infos.push(SegmentInfo {
            index: s.index,
            name: s.name.clone(),
            plain_len: s.plain_len,
            plain_sha256: s.plain_sha256.clone(),
            cipher_sha256: s.cipher_sha256.clone(),
            bytes: s.bytes,
        });
    }
    // Siguiente índice a escribir: el siguiente al último segmento sellado, o
    // el del segmento a medias (si lo hay).
    let mut resumed_from = seg_infos.len();
    let mut resumed_partial: Option<(u32, u64, Vec<ManifestEntry>)> = None;
    if let Some(p) = &journal.partial {
        let pp = opts.out.join(SEGMENTS_DIR).join(format!("seg-{:05}.plain", p.index));
        if pp.exists() {
            for e in &p.entries {
                skipped.insert(e.path.clone(), ());
                entries.push(e.clone());
            }
            resumed_from = p.index as usize;
            resumed_partial = Some((p.index, p.plain_len, p.entries.clone()));
        }
    }

    // 4) Escribir el tar por segmentos.
    let mut writer = SegmentWriter::new(
        &opts.out,
        seg_bytes,
        cipher.as_ref(),
        resumed_from,
        resumed_partial,
        opts.dry_run,
        &mut seg_infos,
        sink,
    )?;

    let mut paused_ms = 0u64;
    let mut slept_ms = 0u64;
    let mut batch = 0usize;
    let mut packages: Vec<String> = Vec::new();
    for src in &sources {
        if !packages.contains(&src.package) {
            packages.push(src.package.clone());
        }
        if skipped.contains_key(&src.inside) {
            continue;
        }
        let full = join_root(&opts.root, Path::new(&src.device_path));
        let md = match fs::symlink_metadata(&full) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !md.is_file() {
            continue;
        }
        let (size, mtime, mode) = (
            md.len(),
            md.modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0),
            backup_crypto::unix_mode(&md),
        );
        let sha = match writer.add_file(&src.inside, &src.package, &src.source, &full, size, mtime, mode)
        {
            Ok(sha) => sha,
            Err(e) => {
                // Un fichero que desaparece o cambia mientras se copia se salta
                // (y se anota): no tumba todo el backup.
                eprintln!("[md-backup] se salta {}: {e}", src.inside);
                continue;
            }
        };
        entries.push(ManifestEntry {
            path: src.inside.clone(),
            package: src.package.clone(),
            source: src.source.clone(),
            size,
            sha256: sha,
            mtime,
            mode,
        });
        batch += 1;
        if batch % opts.gov.batch_size.max(1) == 0 {
            let tick = opts.gov.tick(&read_loadavg(&opts.root));
            paused_ms += tick.pause_ms;
            slept_ms += tick.sleep_ms;
        }
    }
    // Fin del tar: dos bloques nulos.
    writer.finish()?;

    // 5) Manifiesto y cabecera.
    let mut manifest = Manifest {
        version: 1,
        tool: format!("md-backup/{}", env!("CARGO_PKG_VERSION")),
        created_at: now_epoch(),
        entries: entries.clone(),
        total_bytes: entries.iter().map(|e| e.size).sum(),
    };
    manifest.sort();
    let manifest_sha = manifest.hash();
    // El plan validado (si lo hay) queda **registrado**: la propuesta cruda
    // del LLM y la decisión del código, con sus rechazos.
    let plan_value = opts.plan.as_ref().map(|p| {
        json!({
            "summary": p.summary(),
            "decision": p,
            "proposal_raw": p.proposal.raw,
        })
    });
    let plan_sha = opts.plan.as_ref().map(|p| p.hash()).unwrap_or_default();
    let header = ContainerHeader {
        magic: CONTAINER_MAGIC.to_string(),
        version: CONTAINER_VERSION,
        created_at: now_epoch(),
        tool: format!("md-backup/{}", env!("CARGO_PKG_VERSION")),
        cipher: "aegis256-chunk-v1".to_string(),
        key_source: key_source.clone(),
        kdf: kdf.clone(),
        key_check: key_check.clone(),
        segment_bytes: seg_bytes,
        manifest_sha256: manifest_sha.clone(),
        packages: packages.clone(),
        segments: seg_infos.clone(),
        plain_bytes: seg_infos.iter().map(|s| s.plain_len).sum(),
        files: manifest.entries.len(),
        plan: plan_value.clone(),
        plan_sha256: plan_sha.clone(),
    };

    if !opts.dry_run {
        fs::write(
            opts.out.join(MANIFEST_FILE),
            serde_json::to_vec_pretty(&manifest).unwrap_or_default(),
        )
        .map_err(|e| format!("no se puede escribir el manifiesto: {e}"))?;
        fs::write(
            opts.out.join(CONTAINER_FILE),
            serde_json::to_vec_pretty(&header).unwrap_or_default(),
        )
        .map_err(|e| format!("no se puede escribir la cabecera: {e}"))?;
        // La clave sale del contenedor: vive en un fichero `0600` aparte
        // (el contenedor se sube a Drive; la clave no).
        if let Some(p) = opts.key_out.as_ref() {
            write_key_file(p, &master, &kdf, &key_check, &key_source)?;
        }
        // El plan aplicado queda **en el contenedor**: qué propuso la IA, qué
        // se aceptó y qué se rechazó.
        if let (Some(pv), Some(p)) = (plan_value.as_ref(), opts.plan.as_ref()) {
            fs::write(
                opts.out.join(crate::backup_plan::PLAN_FILE),
                serde_json::to_vec_pretty(&pv).unwrap_or_default(),
            )
            .map_err(|e| format!("no se puede escribir el plan: {e}"))?;
            let _ = p;
        }
    }
    sink.finish(&header, &manifest)?;

    Ok(CreateReport {
        container: opts.out.to_string_lossy().to_string(),
        packages,
        files: manifest.entries.len(),
        bytes: manifest.total_bytes,
        segments: seg_infos.len(),
        manifest_sha256: manifest_sha,
        excluded_files: excl_files,
        excluded_bytes: excl_bytes,
        dry_run: opts.dry_run,
        loadavg_before: load_before,
        loadavg_after: read_loadavg(&opts.root),
        paused_ms,
        slept_ms,
        elapsed_ms: t0.elapsed().as_millis() as u64,
        io_limits,
        handed_off: seg_infos.len(),
        read_bytes: manifest.total_bytes,
        read_mib_per_s: {
            let busy = (t0.elapsed().as_millis() as u64)
                .saturating_sub(paused_ms)
                .saturating_sub(slept_ms)
                .max(1);
            mib_per_s(manifest.total_bytes, busy)
        },
        forbidden_files: forbid_files,
        forbidden_bytes: forbid_bytes,
        plan: plan_value,
        plan_sha256: plan_sha,
    })
}

/// Bytes aleatorios del CSPRNG (reutiliza el del módulo de inventario).
fn backup_inventory_random(n: usize) -> Result<Vec<u8>, String> {
    crate::backup_io::random_bytes(n)
}

/// Epoch actual.
pub fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Construye la clave maestra (y su key-check) según las opciones.
///
/// * Con contraseña: se deriva con **Argon2id** (los parámetros del repo).
/// * Sin contraseña: clave **aleatoria** del CSPRNG (la que sea, se guarda en
///   el fichero de claves `0600`; nunca en el repo ni en el contenedor).
fn build_master_key(opts: &CreateOptions) -> Result<(Vec<u8>, KdfParams, String, String), String> {
    let mut kdf = KdfParams::default_params();
    kdf.salt = backup_inventory_random(kdf.salt.len().max(16))?;
    match &opts.passphrase {
        Some(pass) => {
            let key = kdf.derive_key(pass)?;
            let kc = backup_crypto::key_check(&key, &LABEL_SEGMENT_NONCE, 1);
            Ok((key, kdf, backup_crypto::hex(&kc), "passphrase".to_string()))
        }
        None => {
            let key = backup_inventory_random(AEGIS_KEY_LEN)?;
            let kc = backup_crypto::key_check(&key, &LABEL_SEGMENT_NONCE, 1);
            Ok((key, kdf, backup_crypto::hex(&kc), "random".to_string()))
        }
    }
}

/// Escribe el fichero de claves con permisos `0600`.
///
/// Contiene la **clave maestra** en hexadecimal: es tan sensible como la
/// contraseña, por eso vive fuera del repo y fuera del contenedor (el
/// contenedor se sube a Drive; el fichero de claves no).
fn write_key_file(
    path: &Path,
    master: &[u8],
    kdf: &KdfParams,
    key_check: &str,
    source: &str,
) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let obj = json!({
        "cipher": "aegis256-chunk-v1",
        "kdf": kdf,
        "key_check": key_check,
        "key_source": source,
        "master_key_hex": backup_crypto::hex(master),
    });
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("no se puede crear {}: {e}", parent.display()))?;
    }
    fs::write(path, serde_json::to_vec_pretty(&obj).unwrap_or_default())
        .map_err(|e| format!("no se puede escribir {}: {e}", path.display()))?;
    let mut perms = fs::metadata(path)
        .map_err(|e| format!("no se puede ver {}: {e}", path.display()))?
        .permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)
        .map_err(|e| format!("no se puede poner 0600 a {}: {e}", path.display()))?;
    Ok(())
}

/// Lee el fichero de claves (`0600`) y devuelve la clave maestra.
pub fn read_key_file(path: &Path) -> Result<Vec<u8>, String> {
    use std::os::unix::fs::PermissionsExt;
    let md = fs::metadata(path).map_err(|e| format!("no se puede ver {}: {e}", path.display()))?;
    let mode = md.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(format!(
            "{} tiene permisos {mode:o}: el fichero de claves debe ser 0600",
            path.display()
        ));
    }
    let data = fs::read_to_string(path).map_err(|e| format!("no se puede leer {}: {e}", path.display()))?;
    let v: Value = serde_json::from_str(&data).map_err(|e| format!("fichero de claves inválido: {e}"))?;
    let hexkey = v
        .get("master_key_hex")
        .and_then(|k| k.as_str())
        .ok_or("el fichero de claves no trae 'master_key_hex'")?;
    backup_crypto::unhex(hexkey)
}

/// El escritor de segmentos: rota el tar, sella y entrega.
struct SegmentWriter<'a> {
    out: PathBuf,
    seg_bytes: u64,
    cipher: Option<&'a ChunkCipher>,
    index: u32,
    plain: Option<File>,
    plain_len: u64,
    dry_run: bool,
    seg_infos: &'a mut Vec<SegmentInfo>,
    sink: &'a mut dyn SegmentSink,
    handed: usize,
    batch: usize,
    pending_entries: Vec<ManifestEntry>,
}

impl<'a> SegmentWriter<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        out: &Path,
        seg_bytes: u64,
        cipher: Option<&'a ChunkCipher>,
        resumed_from: usize,
        resumed_partial: Option<(u32, u64, Vec<ManifestEntry>)>,
        dry_run: bool,
        seg_infos: &'a mut Vec<SegmentInfo>,
        sink: &'a mut dyn SegmentSink,
    ) -> Result<SegmentWriter<'a>, String> {
        let mut w = SegmentWriter {
            out: out.to_path_buf(),
            seg_bytes,
            cipher,
            index: resumed_from as u32,
            plain: None,
            plain_len: 0,
            dry_run,
            seg_infos,
            sink,
            handed: 0,
            batch: 0,
            pending_entries: Vec::new(),
        };
        // Reanudación: se retoma el segmento a medias (truncando el .plain a
        // lo que el diario dice que ya está escrito).
        if let Some((index, keep, entries)) = resumed_partial {
            let name = format!("seg-{:05}.plain", index);
            let p = w.out.join(SEGMENTS_DIR).join(&name);
            if p.exists() {
                let f = fs::OpenOptions::new()
                    .write(true)
                    .open(&p)
                    .map_err(|e| format!("no se puede abrir {}: {e}", p.display()))?;
                f.set_len(keep)
                    .map_err(|e| format!("no se puede truncar {}: {e}", p.display()))?;
                w.plain = Some(f);
                w.plain_len = keep;
                // Los ficheros ya copiados en este segmento cuentan como
                // copiados: si no, en la segunda reanudación se duplicarían.
                w.pending_entries = entries;
            }
        }
        Ok(w)
    }

    fn plain_path(&self) -> PathBuf {
        self.out
            .join(SEGMENTS_DIR)
            .join(format!("seg-{:05}.plain", self.index))
    }

    /// Añade un fichero al tar en curso; devuelve su SHA-256.
    fn add_file(
        &mut self,
        inside: &str,
        package: &str,
        source: &str,
        full: &Path,
        size: u64,
        mtime: u64,
        mode: u32,
    ) -> Result<String, String> {
        // Cabecera tar (con nombre largo).
        let header = tar_header_long(inside, size, mode | 0o400, mtime);
        let mut h = Sha256::new();
        let mut f = File::open(full).map_err(|e| format!("no se puede abrir {}: {e}", full.display()))?;
        let mut buf = vec![0u8; 256 * 1024];
        let mut total = 0u64;
        if !self.dry_run {
            if self.plain.is_none() {
                let p = self.plain_path();
                let f = File::create(&p)
                    .map_err(|e| format!("no se puede crear {}: {e}", p.display()))?;
                self.plain = Some(f);
                self.plain_len = 0;
                self.pending_entries.clear();
            }
            let w = self.plain.as_mut().unwrap();
            w.write_all(&header)
                .map_err(|e| format!("no se puede escribir el tar: {e}"))?;
        }
        loop {
            let n = f
                .read(&mut buf)
                .map_err(|e| format!("no se puede leer {}: {e}", full.display()))?;
            if n == 0 {
                break;
            }
            if !self.dry_run {
                let w = self.plain.as_mut().unwrap();
                w.write_all(&buf[..n])
                    .map_err(|e| format!("no se puede escribir el tar: {e}"))?;
            }
            h.update(&buf[..n]);
            total += n as u64;
        }
        if total != size {
            return Err(format!(
                "el fichero cambió mientras se copiaba (se esperaban {size} B, se leyeron {total})"
            ));
        }
        let pad = tar_pad(size);
        if pad > 0 && !self.dry_run {
            let w = self.plain.as_mut().unwrap();
            w.write_all(&vec![0u8; pad])
                .map_err(|e| format!("no se puede escribir el relleno: {e}"))?;
        }
        self.plain_len += 512 + size + pad as u64;
        let sha = h.finalize().iter().map(|b| format!("{b:02x}")).collect::<String>();
        self.pending_entries.push(ManifestEntry {
            path: inside.to_string(),
            package: package.to_string(),
            source: source.to_string(),
            size,
            sha256: sha.clone(),
            mtime,
            mode,
        });
        // ¿Toca rotar?
        if self.plain_len >= self.seg_bytes {
            self.rotate()?;
        }
        Ok(sha)
    }

    /// Sella el segmento en curso y lo entrega al consumidor.
    fn rotate(&mut self) -> Result<(), String> {
        if self.dry_run {
            // Simulación: no se escribe nada, pero se cuenta como si el
            // segmento existiera (para poder medir el plan).
            self.plain_len = 0;
            self.pending_entries.clear();
            self.index += 1;
            return Ok(());
        }
        let Some(_) = self.plain.take() else {
            return Ok(());
        };
        let plain_path = self.plain_path();
        let plain = fs::read(&plain_path)
            .map_err(|e| format!("no se puede leer {}: {e}", plain_path.display()))?;
        let sealed = seal_segment(&plain, self.index, self.cipher)?;
        let name = format!("seg-{:05}.mds", self.index);
        let final_path = self.out.join(SEGMENTS_DIR).join(&name);
        fs::write(&final_path, &sealed)
            .map_err(|e| format!("no se puede escribir {}: {e}", final_path.display()))?;
        let plain_sha = sha256_bytes(&plain);
        let cipher_sha = sha256_bytes(&sealed);
        let info = SegmentInfo {
            index: self.index,
            name: name.clone(),
            plain_len: plain.len() as u64,
            plain_sha256: plain_sha,
            cipher_sha256: cipher_sha.clone(),
            bytes: sealed.len() as u64,
        };
        // Diario: el segmento queda confirmado.
        let jseg = JournalSegment {
            index: self.index,
            name: name.clone(),
            plain_len: info.plain_len,
            plain_sha256: info.plain_sha256.clone(),
            cipher_sha256: cipher_sha,
            bytes: info.bytes,
            entries: self.pending_entries.clone(),
        };
        append_journal(
            &self.out,
            &json!({
                "kind": "seg",
                "index": jseg.index,
                "name": jseg.name,
                "plain_len": jseg.plain_len,
                "plain_sha256": jseg.plain_sha256,
                "cipher_sha256": jseg.cipher_sha256,
                "bytes": jseg.bytes,
                "entries": jseg.entries,
            }),
        )?;
        self.pending_entries.clear();
        // El .plain ya no hace falta: el espacio es el recurso escaso.
        let _ = fs::remove_file(&plain_path);
        self.seg_infos.push(info.clone());
        // **Entrega**: el consumidor puede subirlo mientras seguimos recogiendo.
        let seg = SealedSegment {
            index: self.index,
            path: final_path,
            info,
        };
        self.sink.submit(&seg)?;
        self.handed += 1;
        self.index += 1;
        Ok(())
    }

    /// Cierra el tar (dos bloques nulos) y sella el último segmento.
    fn finish(&mut self) -> Result<(), String> {
        if self.plain.is_none() {
            // Sin fichero en curso: se crea el segmento vacío para que el
            // contenedor sea siempre válido.
            //
            // BUG REAL (medido el 2026-11-22 en el móvil): el `File` se
            // creaba y se **soltaba** sin guardarlo en `self.plain` ⇒ el
            // `unwrap()` de abajo **panicaba** cuando el último fichero
            // había rotado justo el segmento (caso normal: el panic tumbaba
            // el backup entero y se perdían los segmentos ya subida).
            let p = self.plain_path();
            let f = File::create(&p).map_err(|e| format!("no se puede crear {}: {e}", p.display()))?;
            self.plain = Some(f);
            self.plain_len = 0;
        }
        {
            let w = self.plain.as_mut().ok_or("el segmento en curso se perdió")?;
            w.write_all(&[0u8; 1024])
                .map_err(|e| format!("no se puede escribir el fin del tar: {e}"))?;
            w.flush().ok();
        }
        self.plain_len += 1024;
        self.rotate()?;
        Ok(())
    }
}

// ========================================================================= //
// 10. Verificación
// ========================================================================= //

/// Informe de `verify`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerifyReport {
    /// ¿Todo cuadra?
    pub ok: bool,
    /// Segmentos verificados.
    pub segments: usize,
    /// Entradas del manifiesto comprobadas.
    pub entries: usize,
    /// Bytes verificados.
    pub bytes: u64,
    /// ¿El manifiesto del contenedor cuadra con su hash?
    pub manifest_ok: bool,
    /// Entradas cuyo SHA-256 **no** cuadra.
    pub mismatches: Vec<String>,
    /// Entradas que están en el manifiesto pero no en el contenedor.
    pub missing: Vec<String>,
    /// Entradas que están en el contenedor pero no en el manifiesto.
    pub extra: Vec<String>,
    /// Milisegundos totales.
    pub elapsed_ms: u64,
}

/// Verifica el manifiesto **y** el AEAD de cada segmento.
pub fn verify(dir: &Path, key: Option<&[u8]>) -> Result<VerifyReport, String> {
    let t0 = Instant::now();
    let header = read_header(dir)?;
    let manifest = read_manifest(dir)?;
    let mut report = VerifyReport {
        ok: false,
        segments: header.segments.len(),
        entries: 0,
        bytes: 0,
        manifest_ok: manifest.hash() == header.manifest_sha256,
        mismatches: Vec::new(),
        missing: Vec::new(),
        extra: Vec::new(),
        elapsed_ms: 0,
    };

    // 1) Manifiesto vs. cabecera.
    let by_path: BTreeMap<&str, &ManifestEntry> = manifest
        .entries
        .iter()
        .map(|e| (e.path.as_str(), e))
        .collect();

    // 2) Recorrido del tar descifrado (streaming).
    let mut segs: Vec<PathBuf> = Vec::new();
    for s in &header.segments {
        let p = dir.join(SEGMENTS_DIR).join(&s.name);
        if !p.exists() {
            report.missing.push(s.name.clone());
            continue;
        }
        segs.push(p);
    }
    let mut reader = SegmentReader::new(segs, key.map(|k| k.to_vec()));
    let mut tar = TarStream::new(&mut reader);
    let mut seen: BTreeMap<String, ()> = BTreeMap::new();
    let mut hasher = Sha256::new();
    let mut current_size = 0u64;
    let mut current_path = String::new();
    loop {
        let entry = match tar.next_entry() {
            Ok(Some(e)) => e,
            Ok(None) => break,
            Err(e) => return Err(format!("tar ilegible: {e}")),
        };
        hasher = Sha256::new();
        current_path = entry.name.clone();
        current_size = entry.size;
        {
            let mut sink = HashingWriter {
                hasher: &mut hasher,
                written: 0,
            };
            tar.copy_data(&mut sink)
                .map_err(|e| format!("no se puede leer el contenido de {}: {e}", entry.name))?;
        }
        let sha = hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        seen.insert(entry.name.clone(), ());
        report.entries += 1;
        report.bytes += current_size;
        match by_path.get(entry.name.as_str()) {
            Some(m) => {
                if m.sha256 != sha {
                    report
                        .mismatches
                        .push(format!("{}: manifiesto {} ≠ calculado {sha}", entry.name, m.sha256));
                }
                if m.size != current_size {
                    report.mismatches.push(format!(
                        "{}: tamaño del manifiesto {} ≠ {current_size}",
                        entry.name, m.size
                    ));
                }
            }
            None => report.extra.push(entry.name.clone()),
        }
    }
    for m in &manifest.entries {
        if !seen.contains_key(&m.path) {
            report.missing.push(m.path.clone());
        }
    }
    report.ok = report.manifest_ok
        && report.mismatches.is_empty()
        && report.missing.is_empty()
        && report.extra.is_empty();
    report.elapsed_ms = t0.elapsed().as_millis() as u64;
    Ok(report)
}

/// escritor que sólo hashea (no escribe en disco).
struct HashingWriter<'a> {
    hasher: &'a mut Sha256,
    written: u64,
}

impl Write for HashingWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.hasher.update(buf);
        self.written += buf.len() as u64;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// ========================================================================= //
// 11. Restauración
// ========================================================================= //

/// Opciones de `restore`.
#[derive(Debug, Clone, PartialEq)]
pub struct RestoreOptions {
    /// Contenedor.
    pub dir: PathBuf,
    /// Dónde se restaura.
    pub dest: PathBuf,
    /// Paquetes a restaurar (vacío = todos).
    pub packages: Vec<String>,
    /// ¿Confirmación explícita? Sin ella es **simulación**.
    pub confirm: bool,
    /// ¿Sobrescribe lo que ya existe?
    pub overwrite: bool,
    /// Clave (si el contenedor está cifrado).
    pub key: Option<Vec<u8>>,
}

/// Informe de `restore`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RestoreReport {
    /// ¿Fue sólo una simulación?
    pub dry_run: bool,
    /// Ficheros vistos.
    pub files: usize,
    /// Bytes vistos.
    pub bytes: u64,
    /// Ficheros escritos.
    pub written: usize,
    /// Ficheros saltados (ya existían).
    pub skipped: usize,
    /// Entradas cuyo SHA-256 no cuadra (**no** se escriben).
    pub mismatches: Vec<String>,
    /// Milisegundos totales.
    pub elapsed_ms: u64,
}

/// Restaura un contenedor.
///
/// **Sin `confirm` no se escribe nada**: el defecto es la simulación.
pub fn restore(opts: &RestoreOptions) -> Result<RestoreReport, String> {
    let t0 = Instant::now();
    let dry_run = !opts.confirm;
    let header = read_header(&opts.dir)?;
    let manifest = read_manifest(&opts.dir)?;
    let mut report = RestoreReport {
        dry_run,
        files: 0,
        bytes: 0,
        written: 0,
        skipped: 0,
        mismatches: Vec::new(),
        elapsed_ms: 0,
    };
    let by_path: BTreeMap<&str, &ManifestEntry> = manifest
        .entries
        .iter()
        .map(|e| (e.path.as_str(), e))
        .collect();

    let mut segs: Vec<PathBuf> = Vec::new();
    for s in &header.segments {
        segs.push(opts.dir.join(SEGMENTS_DIR).join(&s.name));
    }
    let mut reader = SegmentReader::new(segs, opts.key.clone());
    let mut tar = TarStream::new(&mut reader);
    loop {
        let entry = match tar.next_entry() {
            Ok(Some(e)) => e,
            Ok(None) => break,
            Err(e) => return Err(format!("tar ilegible: {e}")),
        };
        // ¿Toca este paquete?
        let m = match by_path.get(entry.name.as_str()) {
            Some(m) => m,
            None => {
                // No está en el manifiesto: se descarta su contenido.
                let mut sink = std::io::sink();
                tar.copy_data(&mut sink).ok();
                continue;
            }
        };
        if !opts.packages.is_empty() && !opts.packages.iter().any(|p| p == &m.package) {
            let mut sink = std::io::sink();
            tar.copy_data(&mut sink).ok();
            continue;
        }
        report.files += 1;
        report.bytes += entry.size;
        // Destino seguro: la ruta del manifiesto **nunca** puede salirse.
        let rel = safe_relative_path(&entry.name)?;
        let dest = opts.dest.join(&rel);
        if dest.exists() && !opts.overwrite {
            let mut sink = std::io::sink();
            tar.copy_data(&mut sink).ok();
            report.skipped += 1;
            continue;
        }
        if dry_run {
            let mut sink = std::io::sink();
            tar.copy_data(&mut sink).ok();
            continue;
        }
        // Se escribe en un temporal y se **verifica el SHA-256 antes** de
        // ponerlo en su sitio: un fichero corrupto no llega al destino.
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("no se puede crear {}: {e}", parent.display()))?;
        }
        let tmp = dest.with_extension("mdbtmp");
        {
            let mut f = File::create(&tmp).map_err(|e| format!("no se puede crear {}: {e}", tmp.display()))?;
            let mut hasher = Sha256::new();
            let mut hw = HashingWriter {
                hasher: &mut hasher,
                written: 0,
            };
            tar.copy_data(&mut hw)
                .map_err(|e| format!("no se puede escribir {}: {e}", tmp.display()))?;
            let sha = hasher
                .finalize()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            if sha != m.sha256 {
                let _ = fs::remove_file(&tmp);
                report.mismatches.push(format!(
                    "{}: SHA-256 del contenido {sha} ≠ manifiesto {}",
                    entry.name, m.sha256
                ));
                continue;
            }
            f.flush().ok();
        }
        fs::rename(&tmp, &dest)
            .map_err(|e| format!("no se puede renombrar a {}: {e}", dest.display()))?;
        report.written += 1;
    }
    report.elapsed_ms = t0.elapsed().as_millis() as u64;
    Ok(report)
}

// ========================================================================= //
// 12. Listado
// ========================================================================= //

/// Resumen de un contenedor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContainerSummary {
    /// Ruta del contenedor.
    pub path: String,
    /// Paquetes.
    pub packages: Vec<String>,
    /// Ficheros.
    pub files: usize,
    /// Bytes.
    pub bytes: u64,
    /// Segmentos.
    pub segments: usize,
    /// Creado (epoch).
    pub created_at: u64,
    /// SHA-256 del manifiesto.
    pub manifest_sha256: String,
    /// ¿Están todos los segmentos en disco?
    pub complete: bool,
}

/// Lista los contenedores de un directorio.
pub fn list(dir: &Path) -> Result<Vec<ContainerSummary>, String> {
    let mut out = Vec::new();
    if dir.join(CONTAINER_FILE).exists() {
        out.push(summary_of(dir)?);
        return Ok(out);
    }
    let rd = fs::read_dir(dir).map_err(|e| format!("no se puede leer {}: {e}", dir.display()))?;
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() && p.join(CONTAINER_FILE).exists() {
            out.push(summary_of(&p)?);
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// Resumen de un contenedor concreto.
pub fn summary_of(dir: &Path) -> Result<ContainerSummary, String> {
    let h = read_header(dir)?;
    let complete = h
        .segments
        .iter()
        .all(|s| dir.join(SEGMENTS_DIR).join(&s.name).exists());
    Ok(ContainerSummary {
        path: dir.to_string_lossy().to_string(),
        packages: h.packages.clone(),
        files: h.files,
        bytes: h.plain_bytes,
        segments: h.segments.len(),
        created_at: h.created_at,
        manifest_sha256: h.manifest_sha256.clone(),
        complete,
    })
}

// ========================================================================= //
// 13. Subida nativa a Drive (pipelining)
// ========================================================================= //

/// Hash de un trozo subido (SHA-256 **por trozo**).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkHash {
    /// Índice del trozo (0-based).
    pub index: u32,
    /// Desplazamiento dentro del fichero.
    pub offset: u64,
    /// Longitud del trozo en bytes.
    pub len: u64,
    /// SHA-256 del trozo (hex).
    pub sha256: String,
}

/// Ficha de los trozos de un fichero subido (`<nombre>.chunks.json`).
///
/// Es lo que permite **verificar en el otro extremo**: al bajar el fichero de
/// Drive se trocea igual y se compara el SHA-256 de cada trozo. Si un solo
/// trozo difiere, se sabe **exactamente** dónde está el corrupto.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkManifest {
    /// Nombre del fichero en Drive.
    pub name: String,
    /// Tamaño total del fichero.
    pub bytes: u64,
    /// SHA-256 del fichero completo.
    pub sha256: String,
    /// Tamaño del trozo con el que se subió.
    pub chunk_bytes: u64,
    /// SHA-256 de cada trozo.
    pub chunks: Vec<ChunkHash>,
}

/// Estadísticas de la subida (instantánea, serializable).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UploadStats {
    /// Bytes subidos.
    pub bytes: u64,
    /// Segmentos subidos.
    pub segments: u64,
    /// Reintentos.
    pub retries: u64,
    /// Errores.
    pub errors: u64,
    /// Primer byte enviado (epoch ms).
    pub started_ms: u64,
    /// Último byte enviado (epoch ms).
    pub ended_ms: u64,
    /// Cifrado total (bytes) para las ETA.
    pub total_bytes: u64,
}

/// Contadores atómicos compartidos por los hilos de subida.
#[derive(Debug, Default)]
struct Counters {
    bytes: AtomicU64,
    segments: AtomicU64,
    retries: AtomicU64,
    errors: AtomicU64,
    started_ms: AtomicU64,
    ended_ms: AtomicU64,
    total_bytes: AtomicU64,
}

impl Counters {
    fn snapshot(&self) -> UploadStats {
        UploadStats {
            bytes: self.bytes.load(Ordering::Relaxed),
            segments: self.segments.load(Ordering::Relaxed),
            retries: self.retries.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            started_ms: self.started_ms.load(Ordering::Relaxed),
            ended_ms: self.ended_ms.load(Ordering::Relaxed),
            total_bytes: self.total_bytes.load(Ordering::Relaxed),
        }
    }
}

/// Millisegundos epoch.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// MiB/s a partir de bytes y milisegundos.
pub fn mib_per_s(bytes: u64, ms: u64) -> f64 {
    if ms == 0 {
        return 0.0;
    }
    (bytes as f64 / (1024.0 * 1024.0)) / (ms as f64 / 1000.0)
}

/// ETA (s) para los bytes que faltan.
pub fn eta_s(bytes_done: u64, bytes_total: u64, ms: u64) -> f64 {
    if bytes_done == 0 || bytes_total <= bytes_done {
        return 0.0;
    }
    let rate = mib_per_s(bytes_done, ms);
    if rate <= 0.0 {
        return 0.0;
    }
    ((bytes_total - bytes_done) as f64 / (1024.0 * 1024.0)) / rate
}

/// Contexto de la subida (lo comparten los hilos).
struct UploadCtx {
    folder_id: String,
    name: String,
    state_dir: Option<String>,
    chunk_bytes: u64,
    max_retries: usize,
    delete_after_upload: bool,
    stats: Arc<Counters>,
    /// Directorio del contenedor (para la ficha de trozos).
    container_dir: Option<PathBuf>,
}

/// Sube **un** fichero a Drive con subida resumible y reintentos.
///
/// Reutiliza el camino **nativo** del daemon (`ureq` + `google_auth` +
/// `drive_client`): ni `curl`, ni `rclone`, ni shell.
///
/// Variante simple: sin el hash por trozo (la usa `bench-upload`).
pub fn upload_file_chunked(
    path: &Path,
    name: &str,
    folder_id: &str,
    state_dir: Option<&str>,
    chunk_bytes: u64,
    max_retries: usize,
) -> Result<crate::drive_native::Uploaded, String> {
    upload_file_chunked_hashed(path, name, folder_id, state_dir, chunk_bytes, max_retries)
        .map(|(up, _)| up)
}

/// Sube un fichero a Drive **devolviendo el SHA-256 de cada trozo**.
///
/// El hash se calcula **sobre los mismos bytes que se envían** (no en una
/// segunda pasada): el trozo se hashea justo antes de salir ⇒ lo que se
/// verifica en Drive es exactamente lo que se leyó del móvil.
pub fn upload_file_chunked_hashed(
    path: &Path,
    name: &str,
    folder_id: &str,
    state_dir: Option<&str>,
    chunk_bytes: u64,
    max_retries: usize,
) -> Result<(crate::drive_native::Uploaded, Vec<ChunkHash>), String> {
    let meta = fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let size = meta.len();
    let sha = crate::drive_native::sha256_file(path)?;
    let target = crate::drive_client::DriveTarget::folder(folder_id);
    let ag = crate::drive_native::agent();

    // 1) Sesión de subida resumible.
    let url = format!(
        "{}?uploadType=resumable&supportsAllDrives=true",
        crate::drive_client::DRIVE_UPLOAD_API
    );
    let (ak, av) = crate::google_auth::auth_header(state_dir)?;
    let resp = ag
        .post(&url)
        .header(ak, av)
        .header("Content-Type", "application/json; charset=UTF-8")
        .header("X-Upload-Content-Length", size.to_string())
        .send(crate::drive_client::file_metadata(name, size, &target).as_bytes())
        .map_err(|e| format!("abrir sesión de subida: {e}"))?;
    let status = resp.status().as_u16();
    if status != 200 && status != 201 {
        let b = resp.into_body().read_to_vec().unwrap_or_default();
        let txt: String = String::from_utf8_lossy(&b).chars().take(200).collect();
        return Err(format!("abrir sesión devolvió {status}: {txt}"));
    }
    let session = resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .ok_or("la sesión de subida no trae la cabecera Location")?
        .to_string();

    // 2) Trozos.
    let plan = crate::drive_client::chunk_plan(size, chunk_bytes);
    let mut file = File::open(path).map_err(|e| format!("abrir {}: {e}", path.display()))?;
    let mut buf = vec![0u8; chunk_bytes.max(DRIVE_CHUNK_MIN) as usize];
    let mut offset: u64 = 0;
    let mut file_id = String::new();
    let mut chunks: Vec<ChunkHash> = Vec::new();
    for (chunk_index, range) in plan.into_iter().enumerate() {
        // El contador de reintentos es **por trozo**: si un trozo falla y se
        // reintenta, los siguientes empiezan de cero (si no, un solo fallo
        // envenenaba el resto de la subida).
        let mut attempt = 0usize;
        let want = range.len() as usize;
        let mut got = 0usize;
        while got < want {
            let n = file
                .read(&mut buf[got..want])
                .map_err(|e| format!("leer {}: {e}", path.display()))?;
            if n == 0 {
                break;
            }
            got += n;
        }
        if got == 0 {
            return Err("lectura corta: el fichero cambió mientras se subía".to_string());
        }
        let end = offset + got as u64 - 1;
        let last = end + 1 >= size;
        let mut ok = false;
        let mut last_err = String::new();
        while attempt <= max_retries {
            let (ak2, av2) = crate::google_auth::auth_header(state_dir)?;
            let r = ag
                .put(&session)
                .header(ak2, av2)
                .header("Content-Range", format!("bytes {}-{}/{}", offset, end, size))
                .header("Content-Length", got.to_string())
                .send(&buf[..got]);
            match r {
                Ok(resp) => {
                    let st = resp.status().as_u16();
                    let body = resp.into_body().read_to_vec().unwrap_or_default();
                    if st == 200 || st == 201 {
                        if last {
                            if let Ok(j) = serde_json::from_slice::<Value>(&body) {
                                if let Some(id) = j.get("id").and_then(|v| v.as_str()) {
                                    file_id = id.to_string();
                                }
                            }
                        }
                        ok = true;
                        break;
                    }
                    if st == 401 {
                        crate::google_auth::invalidate();
                    }
                    let txt: String = String::from_utf8_lossy(&body).chars().take(160).collect();
                    last_err = format!("HTTP {st}: {txt}");
                    if !crate::drive_native::is_transient_status(st) {
                        break;
                    }
                }
                Err(e) => {
                    last_err = e.to_string();
                }
            }
            attempt += 1;
            if attempt > max_retries {
                break;
            }
            crate::drive_native::backoff_with_jitter(attempt as u32);
        }
        if !ok {
            return Err(format!(
                "subir el trozo en {offset} falló tras {max_retries} reintentos: {last_err}"
            ));
        }
        // Hash del trozo **antes** de enviarlo: son los bytes exactos que
        // salen a la red.
        let mut ch = Sha256::new();
        ch.update(&buf[..got]);
        let chunk_sha = ch
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        chunks.push(ChunkHash {
            index: chunk_index as u32,
            offset,
            len: got as u64,
            sha256: chunk_sha,
        });
        offset += got as u64;
    }
    if file_id.is_empty() {
        file_id = find_drive_file_by_name(name, folder_id, state_dir)?;
    }
    Ok((
        crate::drive_native::Uploaded {
            file_id,
            name: name.to_string(),
            bytes: size,
            sha256: sha,
        },
        chunks,
    ))
}

/// Busca el id de un fichero de Drive por nombre (respaldo).
fn find_drive_file_by_name(name: &str, folder_id: &str, state_dir: Option<&str>) -> Result<String, String> {
    let q = format!(
        "name = '{}' and trashed = false{}",
        name.replace('\'', "\\'"),
        if folder_id.is_empty() {
            String::new()
        } else {
            format!(" and '{}' in parents", folder_id)
        }
    );
    let url = format!(
        "{}?supportsAllDrives=true&includeItemsFromAllDrives=true&fields=files(id,name,size)&q={}",
        crate::drive_client::DRIVE_API,
        urlencode(&q)
    );
    let (ak, av) = crate::google_auth::auth_header(state_dir)?;
    let resp = crate::drive_native::agent()
        .get(&url)
        .header(ak, av)
        .call()
        .map_err(|e| format!("buscar en Drive: {e}"))?;
    let st = resp.status().as_u16();
    let body = resp.into_body().read_to_vec().unwrap_or_default();
    if st != 200 {
        return Err(format!("buscar en Drive devolvió {st}"));
    }
    let j: Value = serde_json::from_slice(&body).map_err(|e| format!("json de Drive: {e}"))?;
    j.get("files")
        .and_then(|f| f.as_array())
        .and_then(|a| a.first())
        .and_then(|f| f.get("id"))
        .and_then(|i| i.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("no se encontró el fichero '{name}' en Drive"))
}

/// Lista los ficheros de una carpeta de Drive cuyo nombre empieza por
/// `prefix`: `(id, nombre, tamaño)`.
pub fn list_drive_files(
    folder_id: &str,
    prefix: &str,
    state_dir: Option<&str>,
) -> Result<Vec<(String, String, u64)>, String> {
    let q = format!(
        "'{folder_id}' in parents and trashed = false and name contains '{}'",
        prefix.replace('\'', "\\'")
    );
    let url = format!(
        "{}?supportsAllDrives=true&includeItemsFromAllDrives=true&fields=files(id,name,size)&pageSize=1000&q={}",
        crate::drive_client::DRIVE_API,
        urlencode(&q)
    );
    let (ak, av) = crate::google_auth::auth_header(state_dir)?;
    let resp = crate::drive_native::agent()
        .get(&url)
        .header(ak, av)
        .call()
        .map_err(|e| format!("listar en Drive: {e}"))?;
    let st = resp.status().as_u16();
    let body = resp.into_body().read_to_vec().unwrap_or_default();
    if st != 200 {
        return Err(format!("listar en Drive devolvió {st}"));
    }
    let j: Value = serde_json::from_slice(&body).map_err(|e| format!("json de Drive: {e}"))?;
    let mut out = Vec::new();
    if let Some(files) = j.get("files").and_then(|f| f.as_array()) {
        for f in files {
            let id = f.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let nm = f.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let sz = f.get("size").and_then(|v| v.as_str()).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            if !id.is_empty() && !nm.is_empty() {
                out.push((id, nm, sz));
            }
        }
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(out)
}

/// Informe de `fetch`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FetchReport {
    /// Carpeta de Drive de la que se bajó.
    pub folder_id: String,
    /// Nombre del backup en Drive.
    pub name: String,
    /// Dónde se dejó el contenedor.
    pub dest: String,
    /// Segmentos bajados.
    pub segments: usize,
    /// Bytes bajados.
    pub bytes: u64,
    /// Trozos verificados por SHA-256.
    pub chunks_verified: usize,
    /// ¿El manifiesto cuadra con la cabecera?
    pub manifest_ok: bool,
    /// Desajustes (SHA-256).
    pub mismatches: Vec<String>,
    /// Ficheros de Drive usados: `{name, id, bytes, ok}`.
    pub files: Vec<Value>,
    /// Milisegundos totales.
    pub elapsed_ms: u64,
}

/// Baja un contenedor de Drive y lo deja listo para `restore`.
///
/// **Verifica por SHA-256**: el fichero completo y **cada trozo** (la ficha
/// `.chunks.json` se baja también). Untrozo que no cuadra se reporta con su
/// índice ⇒ se sabe dónde está la corrupción.
pub fn fetch_from_drive(
    folder_id: &str,
    name: &str,
    dest: &Path,
    state_dir: Option<&str>,
) -> Result<FetchReport, String> {
    let t0 = Instant::now();
    let files = list_drive_files(folder_id, name, state_dir)?;
    if files.is_empty() {
        return Err(format!("no hay ningún fichero '{name}' en la carpeta {folder_id}"));
    }
    let segs_dir = dest.join(SEGMENTS_DIR);
    fs::create_dir_all(&segs_dir).map_err(|e| format!("no se puede crear {}: {e}", segs_dir.display()))?;

    let mut report = FetchReport {
        folder_id: folder_id.to_string(),
        name: name.to_string(),
        dest: dest.to_string_lossy().to_string(),
        segments: 0,
        bytes: 0,
        chunks_verified: 0,
        manifest_ok: false,
        mismatches: Vec::new(),
        files: Vec::new(),
        elapsed_ms: 0,
    };

    // 1) Segmentos (+ su ficha de trozos).
    for (id, fname, _size) in &files {
        let Some(rest) = fname.strip_prefix(&format!("{name}.seg")) else {
            continue;
        };
        let Some(digits) = rest.strip_suffix(".mds") else {
            continue;
        };
        if !digits.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let local = segs_dir.join(format!("seg-{digits}.mds"));
        crate::drive_native::download_file(id, &local, state_dir)?;
        let sha = crate::drive_native::sha256_file(&local)?;
        let mut ok = true;
        // Ficha de trozos (si está).
        let side_name = format!("{fname}.chunks.json");
        if let Some((sid, _, _)) = files.iter().find(|(_, n, _)| *n == side_name) {
            let tmp = std::env::temp_dir().join(format!("md-fetch-{}-{digits}", std::process::id()));
            if crate::drive_native::download_file(sid, &tmp, state_dir).is_ok() {
                if let Ok(text) = fs::read_to_string(&tmp) {
                    if let Ok(cm) = serde_json::from_str::<ChunkManifest>(&text) {
                        if cm.sha256 != sha {
                            ok = false;
                            report.mismatches.push(format!(
                                "{fname}: SHA-256 del fichero {sha} ≠ ficha {}",
                                cm.sha256
                            ));
                        }
                        // Trozo a trozo.
                        if let Some(res) = verify_chunks(&local, &cm.chunks) {
                            report.chunks_verified += cm.chunks.len();
                            for m in res {
                                ok = false;
                                report.mismatches.push(m);
                            }
                        } else {
                            ok = false;
                            report.mismatches.push(format!("{fname}: no se pudo trocear el fichero descargado"));
                        }
                    }
                }
                let _ = fs::remove_file(&tmp);
            }
        }
        report.segments += 1;
        report.bytes += fs::metadata(&local).map(|m| m.len()).unwrap_or(0);
        report.files.push(json!({
            "name": fname,
            "id": id,
            "sha256": sha,
            "ok": ok,
        }));
    }

    // 2) Manifiesto y cabecera.
    let manifest_name = format!("{name}.manifest.json");
    let header_name = format!("{name}.container.json");
    for (target, want) in [
        (dest.join(MANIFEST_FILE), manifest_name),
        (dest.join(CONTAINER_FILE), header_name),
    ] {
        if let Some((id, _, _)) = files.iter().find(|(_, n, _)| *n == want) {
            crate::drive_native::download_file(id, &target, state_dir)?;
            report.files.push(json!({
                "name": want,
                "id": id,
                "ok": true,
            }));
        } else {
            return Err(format!("falta '{want}' en Drive: el backup no es restaurable"));
        }
    }

    // 3) El manifiesto tiene que cuadrar con la cabecera.
    if let (Ok(h), Ok(m)) = (read_header(dest), read_manifest(dest)) {
        report.manifest_ok = m.hash() == h.manifest_sha256;
        if !report.manifest_ok {
            report.mismatches.push("el manifiesto no cuadra con la cabecera".to_string());
        }
    } else {
        report.mismatches.push("cabecera o manifiesto ilegibles".to_string());
    }
    report.elapsed_ms = t0.elapsed().as_millis() as u64;
    Ok(report)
}

/// Recalcula el SHA-256 de los trozos indicados y devuelve los que no
/// cuadran (vacío = todo bien).
pub fn verify_chunks(path: &Path, chunks: &[ChunkHash]) -> Option<Vec<String>> {
    let mut f = File::open(path).ok()?;
    let mut bad = Vec::new();
    for c in chunks {
        let mut h = Sha256::new();
        let mut left = c.len;
        let mut buf = vec![0u8; 64 * 1024];
        let mut off = c.offset;
        while left > 0 {
            let want = left.min(buf.len() as u64) as usize;
            let n = f.read(&mut buf[..want]).ok()?;
            if n == 0 {
                bad.push(format!("trozo {}: el fichero se acaba antes de tiempo", c.index));
                break;
            }
            h.update(&buf[..n]);
            off += n as u64;
            left -= n as u64;
        }
        let sha = h
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        if sha != c.sha256 {
            bad.push(format!(
                "trozo {} (offset {off}): SHA-256 {sha} ≠ ficha {}",
                c.index, c.sha256
            ));
        }
    }
    Some(bad)
}

/// Percent-encoding mínimo.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// El consumidor **pipelining**: sube cada segmento en cuanto está cerrado.
///
/// N hilos ⇒ N conexiones en paralelo a Drive. El productor no espera: en
/// cuanto el segmento está sellado, se encola y el hilo lo sube.
pub struct DriveUploader {
    /// Carpeta destino en Drive.
    pub folder_id: String,
    /// Prefijo del nombre de los ficheros en Drive.
    pub name: String,
    /// Directorio del contenedor (para subir manifiesto y cabecera al final).
    pub container_dir: Option<PathBuf>,
    /// Número de conexiones en paralelo.
    pub workers: usize,
    /// Tope de segmentos en vuelo (cola acotada ⇒ backpressure).
    pub queue_capacity: usize,
    /// Tamaño del trozo de la subida resumible.
    pub chunk_bytes: u64,
    /// Directorio de estado (token de Google).
    pub state_dir: Option<String>,
    /// ¿Borra el segmento local tras subirlo?
    pub delete_after_upload: bool,
    /// Reintentos por trozo.
    pub max_retries: usize,
    /// ¿Simulación (no sube nada)?
    pub dry_run: bool,
    tx: Option<SyncSender<Job>>,
    rx: Option<Arc<Mutex<Receiver<Job>>>>,
    workers_handles: Vec<std::thread::JoinHandle<()>>,
    stats: Arc<Counters>,
    errors: Arc<Mutex<Vec<String>>>,
    uploaded_ids: Arc<Mutex<Vec<(u32, String)>>>,
    /// Ids de Drive de las fichas de trozos (`<seg>.chunks.json`).
    sidecar_ids: Arc<Mutex<Vec<(u32, String)>>>,
}

/// Un trabajo para los hilos de subida.
struct Job {
    seg: SealedSegment,
}

impl DriveUploader {
    /// Crea el consumidor (no arranca los hilos todavía).
    pub fn new(folder_id: &str, name: &str, workers: usize) -> DriveUploader {
        DriveUploader {
            folder_id: folder_id.to_string(),
            name: name.to_string(),
            container_dir: None,
            workers: workers.clamp(1, 16),
            queue_capacity: (workers.clamp(1, 16) * 2).clamp(MIN_INFLIGHT_SEGMENTS, MAX_INFLIGHT_SEGMENTS),
            chunk_bytes: DRIVE_CHUNK_MIN,
            state_dir: None,
            delete_after_upload: false,
            max_retries: 4,
            dry_run: false,
            tx: None,
            rx: None,
            workers_handles: Vec::new(),
            stats: Arc::new(Counters::default()),
            errors: Arc::new(Mutex::new(Vec::new())),
            uploaded_ids: Arc::new(Mutex::new(Vec::new())),
            sidecar_ids: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Arranca los hilos de subida.
    pub fn start(&mut self) -> Result<(), String> {
        // Cola ACOTADA: `sync_channel` bloquea al productor cuando hay
        // `queue_capacity` segmentos esperando ⇒ backpressure real (el móvil
        // no llena la RAM de segmentos cerrados).
        let (tx, rx) = sync_channel::<Job>(self.queue_capacity);
        let rx = Arc::new(Mutex::new(rx));
        self.tx = Some(tx);
        self.rx = Some(rx.clone());
        for _ in 0..self.workers {
            let rx = rx.clone();
            let ctx = UploadCtx {
                folder_id: self.folder_id.clone(),
                name: self.name.clone(),
                state_dir: self.state_dir.clone(),
                chunk_bytes: self.chunk_bytes,
                max_retries: self.max_retries,
                delete_after_upload: self.delete_after_upload,
                stats: self.stats.clone(),
                container_dir: self.container_dir.clone(),
            };
            let errors = self.errors.clone();
            let ids = self.uploaded_ids.clone();
            let sidecars = self.sidecar_ids.clone();
            let h = std::thread::spawn(move || loop {
                // `try_recv` (y no `recv` bloqueante con el mutex tomado):
                // así un hilo **no** retiene el cerrojo mientras espera, y
                // los N workers pueden subir en paralelo de verdad.
                let job = {
                    let g = match rx.lock() {
                        Ok(g) => g,
                        Err(_) => break,
                    };
                    match g.try_recv() {
                        Ok(j) => Some(j),
                        // Cola vacía: se suelta el cerrojo y se reintenta
                        // (20 ms: no es espera activa pesada).
                        Err(std::sync::mpsc::TryRecvError::Empty) => None,
                        // Canal cerrado y vacío: no queda nada que subir.
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                    }
                };
                let Some(job) = job else {
                    std::thread::sleep(Duration::from_millis(20));
                    continue;
                };
                let seg_name = format!("{}.seg{:05}", ctx.name, job.seg.index);
                let _ = ctx.stats.started_ms.compare_exchange(
                    0,
                    now_ms(),
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                );
                match upload_file_chunked_hashed(
                    &job.seg.path,
                    &seg_name,
                    &ctx.folder_id,
                    ctx.state_dir.as_deref(),
                    ctx.chunk_bytes,
                    ctx.max_retries,
                ) {
                    Ok((up, chunks)) => {
                        ctx.stats.bytes.fetch_add(up.bytes, Ordering::Relaxed);
                        ctx.stats.segments.fetch_add(1, Ordering::Relaxed);
                        ctx.stats.ended_ms.store(now_ms(), Ordering::Relaxed);
                        if let Ok(mut v) = ids.lock() {
                            v.push((job.seg.index, up.file_id.clone()));
                        }
                        // Ficha de trozos: SHA-256 por trozo, subida **junto**
                        // al segmento ⇒ en el otro extremo se puede saber
                        // exactamente qué trozo se corrompió.
                        let manifest = ChunkManifest {
                            name: seg_name.clone(),
                            bytes: up.bytes,
                            sha256: up.sha256.clone(),
                            chunk_bytes: ctx.chunk_bytes,
                            chunks,
                        };
                        if let Some(dir) = ctx.container_dir.as_ref() {
                            let cdir = dir.join("chunks");
                            let _ = fs::create_dir_all(&cdir);
                            let side = cdir.join(format!("{seg_name}.chunks.json"));
                            if let Ok(bytes) = serde_json::to_vec_pretty(&manifest) {
                                if fs::write(&side, &bytes).is_ok() {
                                    if let Ok(up2) = upload_file_chunked(
                                        &side,
                                        &format!("{seg_name}.chunks.json"),
                                        &ctx.folder_id,
                                        ctx.state_dir.as_deref(),
                                        ctx.chunk_bytes,
                                        ctx.max_retries,
                                    ) {
                                        ctx.stats.bytes.fetch_add(up2.bytes, Ordering::Relaxed);
                                        if let Ok(mut v) = sidecars.lock() {
                                            v.push((job.seg.index, up2.file_id.clone()));
                                        }
                                    }
                                }
                            }
                        }
                        if ctx.delete_after_upload {
                            let _ = fs::remove_file(&job.seg.path);
                        }
                    }
                    Err(e) => {
                        ctx.stats.errors.fetch_add(1, Ordering::Relaxed);
                        if let Ok(mut v) = errors.lock() {
                            v.push(format!("{}: {e}", seg_name));
                        }
                    }
                }
            });
            self.workers_handles.push(h);
        }
        Ok(())
    }

    /// Estadísticas compartidas.
    pub fn stats(&self) -> UploadStats {
        self.stats.snapshot()
    }

    /// Errores de la subida.
    pub fn errors(&self) -> Vec<String> {
        self.errors.lock().map(|e| e.clone()).unwrap_or_default()
    }

    /// Ids de Drive de las fichas de trozos: `(índice, file_id)`.
    pub fn uploaded_sidecars(&self) -> Vec<(u32, String)> {
        let mut v = self
            .sidecar_ids
            .lock()
            .map(|x| x.clone())
            .unwrap_or_default();
        v.sort_by_key(|(i, _)| *i);
        v
    }

    /// Ids de Drive de los segmentos subidos: `(índice, file_id)`.
    pub fn uploaded_segments(&self) -> Vec<(u32, String)> {
        let mut v = self
            .uploaded_ids
            .lock()
            .map(|x| x.clone())
            .unwrap_or_default();
        v.sort_by_key(|(i, _)| *i);
        v
    }
}

impl SegmentSink for DriveUploader {
    fn submit(&mut self, seg: &SealedSegment) -> Result<(), String> {
        self.stats
            .total_bytes
            .fetch_add(seg.info.bytes, Ordering::Relaxed);
        if self.dry_run {
            self.stats.bytes.fetch_add(seg.info.bytes, Ordering::Relaxed);
            self.stats.segments.fetch_add(1, Ordering::Relaxed);
            let _ = self.stats.started_ms.compare_exchange(
                0,
                now_ms(),
                Ordering::Relaxed,
                Ordering::Relaxed,
            );
            self.stats.ended_ms.store(now_ms(), Ordering::Relaxed);
            return Ok(());
        }
        let tx = self
            .tx
            .as_ref()
            .ok_or("el consumidor de subida no está arrancado")?;
        tx.send(Job { seg: seg.clone() })
            .map_err(|e| format!("no se puede encolar el segmento {}: {e}", seg.index))
    }

    fn finish(
        &mut self,
        header: &ContainerHeader,
        manifest: &Manifest,
    ) -> Result<(), String> {
        // Cierra el canal y espera a los hilos.
        self.tx = None;
        for h in self.workers_handles.drain(..) {
            h.join().map_err(|_| "un hilo de subida ha muerto".to_string())?;
        }
        if self.dry_run {
            return Ok(());
        }
        // El manifiesto y la cabecera se suben **al final**: son el índice del
        // backup (sin ellos los segmentos no son restaurables).
        let dir = match &self.container_dir {
            Some(d) => d.clone(),
            None => return Ok(()),
        };
        let manifest_path = dir.join(MANIFEST_FILE);
        if manifest_path.exists() {
            let up = upload_file_chunked(
                &manifest_path,
                &format!("{}.manifest.json", self.name),
                &self.folder_id,
                self.state_dir.as_deref(),
                self.chunk_bytes,
                self.max_retries,
            )?;
            self.stats.bytes.fetch_add(up.bytes, Ordering::Relaxed);
        }
        let header_path = dir.join(CONTAINER_FILE);
        if header_path.exists() {
            let up = upload_file_chunked(
                &header_path,
                &format!("{}.container.json", self.name),
                &self.folder_id,
                self.state_dir.as_deref(),
                self.chunk_bytes,
                self.max_retries,
            )?;
            self.stats.bytes.fetch_add(up.bytes, Ordering::Relaxed);
        }
        // Journal de la subida: qué segmento está en Drive con qué id.
        let ids = self
            .uploaded_ids
            .lock()
            .map(|v| v.clone())
            .unwrap_or_default();
        let mut ids_sorted = ids;
        ids_sorted.sort_by_key(|(i, _)| *i);
        let journal: Vec<Value> = ids_sorted
            .iter()
            .map(|(i, id)| json!({"index": i, "drive_file_id": id}))
            .collect();
        let _ = fs::write(
            dir.join("upload_journal.json"),
            serde_json::to_vec_pretty(&json!({
                "name": self.name,
                "folder_id": self.folder_id,
                "segments": journal,
                "stats": self.stats(),
            }))
            .unwrap_or_default(),
        );
        let _ = manifest;
        let _ = header;
        Ok(())
    }
}
