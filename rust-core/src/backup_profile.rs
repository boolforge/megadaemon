//! **Respaldo de PERFIL** (segunda pieza del respaldo masivo).
//!
//! La primera pieza (los APK) la hace otro módulo; esta se ocupa de **todo
//! lo que no es un APK** y que, sin embargo, es lo que hace que un móvil
//! "vuelva a ser el mismo móvil":
//!
//! | Cubo | Qué entra | Dónde vive (medido en el S21 Ultra) |
//! |---|---|---|
//! | `AppSharedPrefs` | preferencias de cada app | `/data/user/0/<pkg>/shared_prefs` (**928** apps) |
//! | `AppFiles` | ficheros de datos de cada app | `/data/user/0/<pkg>/files` |
//! | `SystemConfig` | configuración del sistema | `/data/system` (**583 MiB** medidos: `blobstore` 400 MiB + `package_cache` 81 MiB + …) |
//! | `Certificates` | keychain, keystore y los `.p12`/`.cer` del usuario | `/data/misc/keychain`, `/data/misc/keystore`, `*.p12` bajo `/data/user/0` |
//! | `Browser` | marcadores, contraseñas, cookies, historial, preferencias, extensiones | `/data/user/0/<pkg>/app_chrome/…`, `files/mozilla/…` |
//! | `Misc` | KDE Connect, notificaciones, ajustes del sistema | `/data/user/0/org.kde.kdeconnect_tp`, `/data/system/users/0/settings_*` |
//! | `Sdcard` | lo que hay en `/sdcard` | `/data/media/0` (225 GB: **P2**, aparte) |
//!
//! ## Requisito previo: root
//!
//! Sin root, `/data/user/0` es inaccesible (el sandbox de cada app). Con
//! root (Magisk) el daemon lee `/data/user/0` directamente: es la **premisa**
//! de todo este módulo, y el plan lo dice si no se puede leer.
//!
//! ## Dos caminos de salida, un solo motor
//!
//! * **Contenedor local** ([`capture_to_container`]): reutiliza
//!   [`crate::backup_engine::capture`] (MDBK1: TAR + zstd + manifiesto +
//!   cifrado opcional). Es el camino "bonito" y verificado.
//! * **Subida en streaming** ([`stream_to_drive`]): el mismo plan se
//!   convierte en un TAR que se **empuja a Drive mientras se escribe**, sin
//!   tocar el disco del móvil (que sólo tiene 10 GB libres). Al final se
//!   sube el manifiesto con el SHA-256 de cada fichero y de cada trozo.
//!
//! No hay dos motores: el **plan** (qué entra y por qué) es el mismo, y el
//! motor de subida es [`crate::backup_stream`].

use crate::backup_engine::{BackupPlan, BackupPolicy, BackupSource, CaptureOptions, EntryKind};
use crate::backup_stream::{ChunkRecord, Pipeline};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Perfiles y cubos
// ---------------------------------------------------------------------------

/// Un cubo de datos del perfil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Bucket {
    /// Preferencias de las apps (`shared_prefs`).
    AppSharedPrefs,
    /// Ficheros de datos de las apps (`files`).
    AppFiles,
    /// Configuración del sistema (`/data/system`).
    SystemConfig,
    /// Certificados y material criptográfico del usuario.
    Certificates,
    /// Perfiles de los navegadores.
    Browser,
    /// Misceláneos (KDE Connect, notificaciones, ajustes).
    Misc,
    /// `/sdcard` (multimedia del usuario).
    Sdcard,
}

impl Bucket {
    /// Nombre estable (el que va en el manifiesto).
    pub fn as_str(self) -> &'static str {
        match self {
            Bucket::AppSharedPrefs => "shared_prefs",
            Bucket::AppFiles => "app_files",
            Bucket::SystemConfig => "system",
            Bucket::Certificates => "certificates",
            Bucket::Browser => "browser",
            Bucket::Misc => "misc",
            Bucket::Sdcard => "sdcard",
        }
    }

    /// Todos los cubos.
    pub fn all() -> [Bucket; 7] {
        [
            Bucket::AppSharedPrefs,
            Bucket::AppFiles,
            Bucket::SystemConfig,
            Bucket::Certificates,
            Bucket::Browser,
            Bucket::Misc,
            Bucket::Sdcard,
        ]
    }
}

/// Los cuatro perfiles de prioridad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupProfile {
    /// **P0**: apps + datos (lo que ya no se puede re-instalar).
    P0AppsData,
    /// **P1**: perfil (configuración, certificados, navegadores, varios).
    P1Profile,
    /// **P2**: `/sdcard` (multimedia).
    P2Sdcard,
    /// **P3**: raw de particiones (forense). Lo hace [`crate::backup_raw`].
    P3Raw,
}

impl BackupProfile {
    /// Nombre estable.
    pub fn as_str(self) -> &'static str {
        match self {
            BackupProfile::P0AppsData => "P0",
            BackupProfile::P1Profile => "P1",
            BackupProfile::P2Sdcard => "P2",
            BackupProfile::P3Raw => "P3",
        }
    }

    /// Descripción para el usuario.
    pub fn title(self) -> &'static str {
        match self {
            BackupProfile::P0AppsData => "P0 · apps y datos",
            BackupProfile::P1Profile => "P1 · perfil (config, certificados, navegadores)",
            BackupProfile::P2Sdcard => "P2 · sdcard",
            BackupProfile::P3Raw => "P3 · raw de particiones (forense)",
        }
    }

    /// Cubos que incluye el perfil.
    pub fn buckets(self) -> Vec<Bucket> {
        match self {
            BackupProfile::P0AppsData => vec![Bucket::AppSharedPrefs, Bucket::AppFiles],
            BackupProfile::P1Profile => vec![
                Bucket::AppSharedPrefs,
                Bucket::SystemConfig,
                Bucket::Certificates,
                Bucket::Browser,
                Bucket::Misc,
            ],
            BackupProfile::P2Sdcard => vec![Bucket::Sdcard],
            BackupProfile::P3Raw => Vec::new(),
        }
    }

    /// Los cuatro perfiles.
    pub fn all() -> [BackupProfile; 4] {
        [
            BackupProfile::P0AppsData,
            BackupProfile::P1Profile,
            BackupProfile::P2Sdcard,
            BackupProfile::P3Raw,
        ]
    }
}

/// Motivos de exclusión (constantes: el informe dice **por qué**).
pub mod reason {
    /// Fuera: caché regenerable.
    pub const CACHE: &str = "caché regenerable: excluida por política";
    /// Fuera: patrón de ruta.
    pub const PATTERN: &str = "excluido por patrón de ruta";
    /// Fuera: paquete excluido.
    pub const PACKAGE: &str = "paquete excluido por política";
    /// Fuera: no está en la lista blanca.
    pub const NOT_WHITELISTED: &str = "paquete no está en la lista blanca";
    /// Fuera: supera el tope por fichero.
    pub const TOO_BIG: &str = "supera el tope por fichero";
    /// Fuera: no cabe en el tope total.
    pub const NO_ROOM: &str = "no cabe en el tope total de bytes";
    /// Fuera: se agotó el tope de ficheros.
    pub const TOO_MANY: &str = "se agotó el tope de ficheros";
}

/// Patrones de exclusión por defecto (cachés regenerables).
///
/// Son los que hacen que un backup de 583 MiB de `/data/system` no se
/// convierta en un backup de 583 MiB de *basura*.
pub fn default_exclude_patterns() -> Vec<String> {
    vec![
        "/cache/".to_string(),
        "/caches/".to_string(),
        "/code_cache/".to_string(),
        "/no_backup/".to_string(),
        "/noBackup/".to_string(),
        "/nocache/".to_string(),
        "/GPUCache/".to_string(),
        "/DawnCache/".to_string(),
        "/DawnGraphiteCache/".to_string(),
        "/DawnWebGPUCache/".to_string(),
        "/ShaderCache/".to_string(),
        "/Service Worker/CacheStorage/".to_string(),
        "/Service Worker/ScriptCache/".to_string(),
        "/blob_storage/".to_string(),
        "/thumbs/".to_string(),
        "/thumbnails/".to_string(),
        "/.thumbnails/".to_string(),
        ".crdownload".to_string(),
        ".part".to_string(),
        ".tmp".to_string(),
    ]
}

/// Exclusiones propias de `/data/system` (regenerables o gigantes).
///
/// Medido en este móvil: `blobstore` 400 MiB (copia de los blobs de las apps
/// para la nube), `package_cache` 81 MiB (APKs ya instalados), `thanos_*`
/// 41 MiB, `procstats` 3 MiB. Todos se regeneran solos.
pub fn system_exclusions() -> Vec<(String, String)> {
    vec![
        ("blobstore".to_string(), "blobstore: copia de los blobs de las apps (400 MiB medidos); los datos vivos están en /data/user/0".to_string()),
        ("package_cache".to_string(), "package_cache: APK ya instalados (81 MiB medidos): se regeneran al reinstalar".to_string()),
        ("procstats".to_string(), "procstats: estadísticas de procesos: regenerables".to_string()),
        ("uiderrors.txt".to_string(), "uiderrors.txt: registro de errores de uid: regenerable".to_string()),
    ]
}

/// Un navegador conocido y sus ficheros interestinges.
#[derive(Debug, Clone, PartialEq)]
pub struct BrowserSpec {
    /// Paquete Android.
    pub package: String,
    /// Nombre legible.
    pub label: String,
    /// Motor del navegador.
    pub engine: &'static str,
    /// Rutas (relativas a `/data/user/0/<pkg>`) que se copian.
    /// Las que acaban en `/` se copian **entero** (directorios).
    pub paths: Vec<String>,
}

/// Los navegadores que se buscan en el dispositivo.
///
/// Los paquetes se detectan solos (si el directorio de datos existe); esta
/// tabla sólo dice **qué** copiar de cada motor.
pub fn browser_specs() -> Vec<BrowserSpec> {
    let chromium = vec![
        "app_chrome/Default/Bookmarks".to_string(),
        "app_chrome/Default/Bookmarks.bak".to_string(),
        "app_chrome/Default/Login Data".to_string(),
        "app_chrome/Default/Cookies".to_string(),
        "app_chrome/Default/Network/Cookies".to_string(),
        "app_chrome/Default/History".to_string(),
        "app_chrome/Default/Preferences".to_string(),
        "app_chrome/Default/Web Data".to_string(),
        // MEDIDO en este móvil (2026-11-22): Chrome **no** tiene "Login Data"
        // (sí "Web Data" y "Account Web Data"): las contraseñas van en otro
        // sitio según la versión, así que se copian las dos variantes.
        "app_chrome/Default/Account Web Data".to_string(),
        "app_chrome/Default/Local State".to_string(),
        "app_chrome/Default/Extensions/".to_string(),
    ];
    let samsung = vec![
        "app_sbrowser/Default/Bookmarks".to_string(),
        "app_sbrowser/Default/Login Data".to_string(),
        "app_sbrowser/Default/Cookies".to_string(),
        "app_sbrowser/Default/History".to_string(),
        "app_sbrowser/Default/Preferences".to_string(),
        "app_sbrowser/Default/Web Data".to_string(),
        "app_sbrowser/Local State".to_string(),
        "app_sbrowser/Default/Extensions/".to_string(),
    ];
    vec![
        BrowserSpec {
            package: "com.android.chrome".into(),
            label: "Chrome".into(),
            engine: "chromium",
            paths: chromium.clone(),
        },
        BrowserSpec {
            package: "com.chrome.dev".into(),
            label: "Chrome Dev".into(),
            engine: "chromium",
            paths: chromium.clone(),
        },
        BrowserSpec {
            package: "org.chromium.thorium".into(),
            label: "Thorium".into(),
            engine: "chromium",
            paths: chromium.clone(),
        },
        BrowserSpec {
            package: "com.microsoft.emmx".into(),
            label: "Edge".into(),
            engine: "chromium",
            paths: chromium.clone(),
        },
        BrowserSpec {
            package: "com.brave.browser".into(),
            label: "Brave".into(),
            engine: "chromium",
            paths: chromium.clone(),
        },
        BrowserSpec {
            package: "com.sec.android.app.sbrowser".into(),
            label: "Samsung Internet".into(),
            engine: "samsung",
            paths: samsung.clone(),
        },
        BrowserSpec {
            package: "com.sec.android.app.sbrowser.beta".into(),
            label: "Samsung Internet Beta".into(),
            engine: "samsung",
            paths: samsung,
        },
        BrowserSpec {
            package: "org.mozilla.fenix".into(),
            label: "Firefox".into(),
            engine: "gecko",
            paths: gecko_paths(),
        },
        BrowserSpec {
            package: "io.github.forkmaintainers.iceraven".into(),
            label: "Iceraven".into(),
            engine: "gecko",
            paths: gecko_paths(),
        },
        BrowserSpec {
            package: "us.spotco.fennec_dos".into(),
            label: "Fennec".into(),
            engine: "gecko",
            paths: gecko_paths(),
        },
        BrowserSpec {
            package: "org.torproject.torbrowser".into(),
            label: "Tor Browser".into(),
            engine: "gecko",
            paths: gecko_paths(),
        },
    ]
}

/// Rutas de un navegador Gecko (los perfiles tienen nombre aleatorio, así que
/// se descubren en runtime).
fn gecko_paths() -> Vec<String> {
    vec![
        "files/mozilla/profiles.ini".to_string(),
        "files/mozilla/<perfil>/places.sqlite".to_string(),
        "files/mozilla/<perfil>/cookies.sqlite".to_string(),
        "files/mozilla/<perfil>/formhistory.sqlite".to_string(),
        "files/mozilla/<perfil>/prefs.js".to_string(),
        "files/mozilla/<perfil>/cert9.db".to_string(),
        "files/mozilla/<perfil>/key4.db".to_string(),
        "files/mozilla/<perfil>/addons.json".to_string(),
        "files/mozilla/<perfil>/extensions.json".to_string(),
        "files/mozilla/<perfil>/logins.json".to_string(),
        "files/mozilla/<perfil>/storage/default/".to_string(),
    ]
}

// ---------------------------------------------------------------------------
// Política
// ---------------------------------------------------------------------------

/// Qué entra en el backup del perfil.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfilePolicy {
    /// Perfil de prioridad.
    pub profile: BackupProfile,
    /// Cubos concretos (si está vacío, los del perfil).
    pub buckets: Vec<Bucket>,
    /// Lista blanca de paquetes (vacía = todos).
    pub include_packages: Vec<String>,
    /// Paquetes excluidos (gana sobre la blanca).
    pub exclude_packages: Vec<String>,
    /// Patrones de ruta excluidos (`*` y `?`).
    pub exclude_patterns: Vec<String>,
    /// Incluir cachés (regenerables: fuera por defecto).
    pub include_cache: bool,
    /// Incluir los ficheros gigantes de `/data/system` (`blobstore`…).
    pub include_system_blobs: bool,
    /// Tope total en bytes (0 = sin tope).
    pub max_total_bytes: u64,
    /// Tope por fichero en bytes (0 = sin tope).
    pub max_entry_bytes: u64,
    /// Tope de ficheros (0 = sin tope).
    pub max_files: usize,
}

impl Default for ProfilePolicy {
    fn default() -> Self {
        ProfilePolicy {
            profile: BackupProfile::P1Profile,
            buckets: Vec::new(),
            include_packages: Vec::new(),
            exclude_packages: Vec::new(),
            exclude_patterns: default_exclude_patterns(),
            include_cache: false,
            include_system_blobs: false,
            max_total_bytes: 0,
            max_entry_bytes: 0,
            max_files: 0,
        }
    }
}

impl ProfilePolicy {
    /// Política de un perfil.
    pub fn for_profile(p: BackupProfile) -> ProfilePolicy {
        ProfilePolicy {
            profile: p,
            buckets: p.buckets(),
            ..ProfilePolicy::default()
        }
    }

    /// Cubos efectivos.
    pub fn effective_buckets(&self) -> Vec<Bucket> {
        if self.buckets.is_empty() {
            self.profile.buckets()
        } else {
            self.buckets.clone()
        }
    }

    /// ¿Esta ruta está excluida?
    pub fn matches_exclusion(&self, path: &str) -> bool {
        self.exclude_patterns
            .iter()
            .any(|p| crate::backup_engine::glob_match(p, path))
    }
}

// ---------------------------------------------------------------------------
// El plan
// ---------------------------------------------------------------------------

/// Una entrada del plan (un fichero candidato).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileEntry {
    /// Cubo al que pertenece.
    pub bucket: Bucket,
    /// Paquete (o `*` si no es de ninguna app).
    pub package: String,
    /// Ruta **absoluta** en el dispositivo.
    pub path: String,
    /// Ruta dentro del contenedor/TAR.
    pub rel_path: String,
    /// Tamaño en bytes.
    pub size: u64,
    /// mtime (epoch).
    pub mtime: u64,
    /// ¿Entra?
    pub included: bool,
    /// **Por qué** entra o no entra.
    pub reason: String,
}

/// El plan del perfil: qué entra, qué no y por qué.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfilePlan {
    /// La política aplicada.
    pub policy: ProfilePolicy,
    /// Todas las entradas candidatas.
    pub entries: Vec<ProfileEntry>,
    /// Paquetes vistos.
    pub packages: usize,
    /// Bytes vistos.
    pub total_bytes: u64,
    /// Bytes que entran.
    pub included_bytes: u64,
    /// Bytes que se quedan fuera.
    pub excluded_bytes: u64,
    /// Resumen por cubo: nombre → (ficheros, bytes).
    pub by_bucket: BTreeMap<String, (usize, u64)>,
    /// Avisos (root ausente, cachés excluidas, etc.).
    pub warnings: Vec<String>,
}

impl ProfilePlan {
    /// Las entradas que entran.
    pub fn included(&self) -> impl Iterator<Item = &ProfileEntry> {
        self.entries.iter().filter(|e| e.included)
    }

    /// Informe en JSON.
    pub fn to_json(&self) -> Value {
        let buckets: Vec<Value> = self
            .by_bucket
            .iter()
            .map(|(k, (n, b))| json!({"bucket": k, "files": n, "bytes": b}))
            .collect();
        json!({
            "ok": true,
            "profile": self.policy.profile.as_str(),
            "title": self.policy.profile.title(),
            "packages": self.packages,
            "entries": self.entries.len(),
            "included_entries": self.included().count(),
            "total_bytes": self.total_bytes,
            "included_bytes": self.included_bytes,
            "excluded_bytes": self.excluded_bytes,
            "buckets": buckets,
            "warnings": self.warnings,
        })
    }
}

// ---------------------------------------------------------------------------
// Construcción del plan
// ---------------------------------------------------------------------------

/// Construye el plan del perfil sobre una raíz (`/` en el móvil).
///
/// `root` permite trabajar sobre un directorio de pruebas: los tests del
/// repo **no** tocan el móvil ni la red.
pub fn plan(root: &str, policy: &ProfilePolicy) -> ProfilePlan {
    let buckets = policy.effective_buckets();
    let mut entries: Vec<ProfileEntry> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut packages: Vec<String> = Vec::new();

    let data_user = first_existing(&[
        &format!("{root}/data/user/0"),
        &format!("{root}/data/data"),
    ]);
    if data_user.is_none() {
        warnings.push(
            "no se encuentra /data/user/0: sin root el sandbox de las apps es \
             ilegible y el perfil sale vacío"
                .to_string(),
        );
    }

    // Paquetes vistos.
    let package_dirs: Vec<(String, PathBuf)> = match &data_user {
        Some(dr) => list_packages(Path::new(dr)),
        None => Vec::new(),
    };
    for (p, _) in &package_dirs {
        packages.push(p.clone());
    }
    packages.sort();

    // 1) shared_prefs y files de cada app.
    if buckets.contains(&Bucket::AppSharedPrefs) {
        for (pkg, dir) in &package_dirs {
            let base = dir.join("shared_prefs");
            if !base.is_dir() {
                continue;
            }
            let mut files = Vec::new();
            collect_files(&base, &format!("profile/apps/{pkg}/shared_prefs"), &mut files, 0);
            for (rel, path) in files {
                push_entry(
                    &mut entries,
                    Bucket::AppSharedPrefs,
                    pkg,
                    &path,
                    &rel,
                    policy,
                    &packages,
                );
            }
        }
    }
    if buckets.contains(&Bucket::AppFiles) {
        for (pkg, dir) in &package_dirs {
            let base = dir.join("files");
            if !base.is_dir() {
                continue;
            }
            let mut files = Vec::new();
            collect_files(&base, &format!("profile/apps/{pkg}/files"), &mut files, 0);
            for (rel, path) in files {
                push_entry(
                    &mut entries,
                    Bucket::AppFiles,
                    pkg,
                    &path,
                    &rel,
                    policy,
                    &packages,
                );
            }
        }
    }

    // 2) Configuración del sistema.
    if buckets.contains(&Bucket::SystemConfig) {
        let sys = format!("{root}/data/system");
        let mut files = Vec::new();
        collect_files(Path::new(&sys), "profile/system", &mut files, 0);
        let excl = system_exclusions();
        for (rel, path) in files {
            let mut included = true;
            let mut why = "configuración del sistema".to_string();
            if !policy.include_system_blobs {
                if let Some((_name, why2)) = excl.iter().find(|(n, _)| {
                    let p = path.to_string_lossy();
                    p.contains(&format!("/data/system/{n}"))
                }) {
                    included = false;
                    why = why2.clone();
                }
            }
            push_entry_with(
                &mut entries,
                Bucket::SystemConfig,
                "*",
                &path,
                &rel,
                policy,
                &packages,
                included,
                why,
            );
        }
    }

    // 3) Certificados.
    if buckets.contains(&Bucket::Certificates) {
        for (dir, rel) in [
            (format!("{root}/data/misc/keychain"), "profile/certs/keychain"),
            (format!("{root}/data/misc/keystore"), "profile/certs/keystore"),
        ] {
            let mut files = Vec::new();
            collect_files(Path::new(&dir), rel, &mut files, 0);
            for (r, path) in files {
                push_entry(
                    &mut entries,
                    Bucket::Certificates,
                    "*",
                    &path,
                    &r,
                    policy,
                    &packages,
                );
            }
        }
        // Los `.p12`/`.cer` del usuario (bancos, FNMT, etc.).
        if let Some(dr) = &data_user {
            for (pkg, dir) in &package_dirs {
                let mut hits = Vec::new();
                collect_by_extension(dir, &["p12", "pfx", "cer", "crt", "pem"], 5, &mut hits);
                for path in hits {
                    let name = path
                        .file_name()
                        .map(|x| x.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let rel = format!("profile/certs/user/{pkg}/{name}");
                    push_entry(
                        &mut entries,
                        Bucket::Certificates,
                        pkg,
                        &path,
                        &rel,
                        policy,
                        &packages,
                    );
                }
            }
        }
    }

    // 4) Navegadores.
    if buckets.contains(&Bucket::Browser) {
        for spec in browser_specs() {
            let dir = match &data_user {
                Some(dr) => Path::new(dr).join(&spec.package),
                None => continue,
            };
            if !dir.is_dir() {
                continue;
            }
            let mut files = Vec::new();
            for p in &spec.paths {
                if let Some(rel) = p.strip_prefix("files/mozilla/<perfil>/") {
                    // Gecko: el nombre del perfil es aleatorio.
                    let moz = dir.join("files/mozilla");
                    if let Ok(rd) = std::fs::read_dir(&moz) {
                        let mut profiles: Vec<PathBuf> = rd
                            .flatten()
                            .map(|e| e.path())
                            .filter(|p| {
                                p.is_dir()
                                    && p.file_name()
                                        .map(|n| {
                                            let n = n.to_string_lossy().to_string();
                                            n.ends_with(".default") || n.ends_with(".default-release")
                                        })
                                        .unwrap_or(false)
                            })
                            .collect();
                        profiles.sort();
                        for prof in profiles {
                            let fname =
                                prof.file_name().unwrap().to_string_lossy().to_string();
                            let full = prof.join(rel);
                            let r = format!("profile/browser/{}/{}/{}", spec.package, fname, rel);
                            if full.is_file() {
                                files.push((r, full));
                            } else if full.is_dir() {
                                let mut sub = Vec::new();
                                collect_files(&full, &r, &mut sub, 0);
                                files.extend(sub);
                            }
                        }
                    }
                    continue;
                }
                let full = dir.join(p);
                let r = format!("profile/browser/{}/{}", spec.package, p);
                if full.is_file() {
                    files.push((r, full));
                } else if full.is_dir() {
                    let mut sub = Vec::new();
                    collect_files(&full, &r, &mut sub, 0);
                    files.extend(sub);
                }
            }
            for (r, path) in files {
                push_entry(
                    &mut entries,
                    Bucket::Browser,
                    &spec.package,
                    &path,
                    &r,
                    policy,
                    &packages,
                );
            }
        }
    }

    // 5) Misceláneos.
    if buckets.contains(&Bucket::Misc) {
        // KDE Connect: los dispositivos emparejados y su configuración.
        for pkg in ["org.kde.kdeconnect_tp", "org.kde.kdeconnect_tp.debug"] {
            let dir = match &data_user {
                Some(dr) => Path::new(dr).join(pkg),
                None => continue,
            };
            if !dir.is_dir() {
                continue;
            }
            for sub in ["shared_prefs", "files"] {
                let base = dir.join(sub);
                if !base.is_dir() {
                    continue;
                }
                let mut files = Vec::new();
                collect_files(&base, &format!("profile/misc/{pkg}/{sub}"), &mut files, 0);
                for (rel, path) in files {
                    push_entry(
                        &mut entries,
                        Bucket::Misc,
                        pkg,
                        &path,
                        &rel,
                        policy,
                        &packages,
                    );
                }
            }
        }
        // Ajustes del sistema del usuario 0 y notificaciones.
        for extra in [
            "data/system/users/0/settings_ssaid.xml",
            "data/system/users/0/settings_secure.xml",
            "data/system/users/0/settings_global.xml",
            "data/system/notification_policy.xml",
            "data/system/users/0/notification_policy.xml",
            "data/system/packages.xml",
        ] {
            let p = format!("{root}/{extra}");
            let path = PathBuf::from(&p);
            if path.is_file() {
                let rel = format!("profile/misc/{}", extra);
                push_entry(
                    &mut entries,
                    Bucket::Misc,
                    "*",
                    &path,
                    &rel,
                    policy,
                    &packages,
                );
            }
        }
    }

    // 6) sdcard (P2).
    if buckets.contains(&Bucket::Sdcard) {
        let media = first_existing(&[
            &format!("{root}/data/media/0"),
            &format!("{root}/storage/emulated/0"),
        ]);
        if let Some(m) = media {
            let mut files = Vec::new();
            collect_files(Path::new(&m), "profile/sdcard", &mut files, 0);
            for (rel, path) in files {
                push_entry(
                    &mut entries,
                    Bucket::Sdcard,
                    "*",
                    &path,
                    &rel,
                    policy,
                    &packages,
                );
            }
        } else {
            warnings.push("no se encuentra /data/media/0 (sdcard)".to_string());
        }
    }

    // 7) Topes finales.
    let mut total_bytes = 0u64;
    let mut included_bytes = 0u64;
    let mut excluded_bytes = 0u64;
    let mut budget = policy.max_total_bytes;
    let mut included_files = 0usize;
    for e in entries.iter_mut() {
        total_bytes += e.size;
        if !e.included {
            excluded_bytes += e.size;
            continue;
        }
        if policy.max_files > 0 && included_files >= policy.max_files {
            e.included = false;
            e.reason = reason::TOO_MANY.to_string();
            excluded_bytes += e.size;
            continue;
        }
        if policy.max_total_bytes > 0 && included_bytes + e.size > budget {
            e.included = false;
            e.reason = reason::NO_ROOM.to_string();
            excluded_bytes += e.size;
            continue;
        }
        included_bytes += e.size;
        included_files += 1;
        budget = budget.saturating_sub(e.size);
    }

    // Resumen por cubo.
    let mut by_bucket: BTreeMap<String, (usize, u64)> = BTreeMap::new();
    for e in entries.iter().filter(|e| e.included) {
        let k = e.bucket.as_str().to_string();
        let v = by_bucket.entry(k).or_insert((0, 0));
        v.0 += 1;
        v.1 += e.size;
    }

    ProfilePlan {
        policy: policy.clone(),
        entries,
        packages: packages.len(),
        total_bytes,
        included_bytes,
        excluded_bytes,
        by_bucket,
        warnings,
    }
}

#[allow(clippy::too_many_arguments)]
fn push_entry(
    entries: &mut Vec<ProfileEntry>,
    bucket: Bucket,
    pkg: &str,
    path: &Path,
    rel: &str,
    policy: &ProfilePolicy,
    packages: &[String],
) {
    let (size, mtime) = stat_of(path);
    let abs = path.to_string_lossy().to_string();
    let (included, why) = decide(policy, pkg, &abs, size, packages);
    push_entry_with(
        entries, bucket, pkg, path, rel, policy, packages, included, why,
    );
}

#[allow(clippy::too_many_arguments)]
fn push_entry_with(
    entries: &mut Vec<ProfileEntry>,
    bucket: Bucket,
    pkg: &str,
    path: &Path,
    rel: &str,
    policy: &ProfilePolicy,
    packages: &[String],
    included: bool,
    why: String,
) {
    let (size, mtime) = stat_of(path);
    let _ = (policy, packages);
    entries.push(ProfileEntry {
        bucket,
        package: pkg.to_string(),
        path: path.to_string_lossy().to_string(),
        rel_path: rel.to_string(),
        size,
        mtime,
        included,
        reason: why,
    });
}

fn decide(
    policy: &ProfilePolicy,
    pkg: &str,
    abs: &str,
    size: u64,
    packages: &[String],
) -> (bool, String) {
    if policy.exclude_packages.iter().any(|p| p == pkg) {
        return (false, reason::PACKAGE.to_string());
    }
    if !policy.include_packages.is_empty()
        && !policy.include_packages.iter().any(|p| p == pkg)
        && packages.iter().any(|p| p == pkg)
    {
        return (false, reason::NOT_WHITELISTED.to_string());
    }
    if !policy.include_cache && is_cache_path(abs) {
        return (false, reason::CACHE.to_string());
    }
    if policy.matches_exclusion(abs) {
        return (false, reason::PATTERN.to_string());
    }
    if policy.max_entry_bytes > 0 && size > policy.max_entry_bytes {
        return (false, reason::TOO_BIG.to_string());
    }
    (true, "entra en el perfil".to_string())
}

/// ¿La ruta es caché regenerable?
pub fn is_cache_path(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    p.contains("/cache/")
        || p.contains("/caches/")
        || p.contains("/code_cache/")
        || p.contains("/no_backup/")
        || p.contains("/gpu_cache/")
        || p.contains("/thumbnails/")
        || p.ends_with(".crdownload")
}

fn first_existing(candidates: &[&str]) -> Option<String> {
    for c in candidates {
        if Path::new(c).is_dir() {
            return Some((*c).to_string());
        }
    }
    None
}

fn list_packages(root: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(root) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "ce" {
                continue;
            }
            let p = e.path();
            if p.is_dir() {
                out.push((name, p));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn collect_files(base: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>, depth: usize) {
    if depth > 12 {
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

/// Busca ficheros por extensión (con límite de profundidad).
fn collect_by_extension(dir: &Path, exts: &[&str], max_depth: usize, out: &mut Vec<PathBuf>) {
    if max_depth == 0 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            collect_by_extension(&path, exts, max_depth - 1, out);
            continue;
        }
        let name = path
            .file_name()
            .map(|x| x.to_string_lossy().to_string())
            .unwrap_or_default();
        let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        if exts.iter().any(|e| *e == ext) {
            out.push(path);
        }
    }
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

// ---------------------------------------------------------------------------
// Captura local (contenedor MDBK1: reutiliza el motor existente)
// ---------------------------------------------------------------------------

/// Convierte el plan del perfil en el plan del motor de backup.
///
/// **No** hay motor nuevo: se reutiliza [`crate::backup_engine`], que ya
/// sabe hacer el contenedor MDBK1 (TAR + zstd + manifiesto + cifrado).
pub fn to_engine_plan(plan: &ProfilePlan) -> BackupPlan {
    let mut entries = Vec::new();
    for e in plan.included() {
        entries.push(crate::backup_engine::PlanEntry {
            package: e.package.clone(),
            source: BackupSource::Data,
            kind: EntryKind::File,
            path: e.path.clone(),
            rel_path: e.rel_path.clone(),
            size: e.size,
            mtime: e.mtime,
            included: true,
            reason: e.reason.clone(),
        });
    }
    BackupPlan {
        policy: BackupPolicy::default(),
        entries,
        packages: plan.packages,
        total_bytes: plan.total_bytes,
        included_bytes: plan.included_bytes,
        excluded_bytes: plan.excluded_bytes,
        truncated: false,
    }
}

/// Escribe el perfil en un **contenedor local** (MDBK1).
///
/// Camino "bonito": cifrado opcional, manifiesto dentro y verificación con
/// `backup_engine::verify`.
pub fn capture_to_container(
    plan: &ProfilePlan,
    out: &str,
    opts: &CaptureOptions,
) -> Result<crate::backup_engine::CaptureResult, String> {
    let engine_plan = to_engine_plan(plan);
    crate::backup_engine::capture(&engine_plan, out, opts)
}

// ---------------------------------------------------------------------------
// Subida en streaming (sin disco local)
// ---------------------------------------------------------------------------

/// Ficha de un fichero dentro del manifiesto del stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileRecord {
    /// Ruta dentro del TAR.
    pub path: String,
    /// Tamaño.
    pub size: u64,
    /// SHA-256 del contenido.
    pub sha256: String,
}

/// Ficha de un tramo (una conexión = un objeto en Drive).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartRecord {
    /// Índice del tramo.
    pub stream: usize,
    /// Nombre del objeto en Drive.
    pub name: String,
    /// Id del objeto en Drive.
    pub file_id: String,
    /// Bytes del tramo.
    pub bytes: u64,
}

/// El manifiesto final del stream (se sube a Drive **al final**).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamManifest {
    /// Etiqueta del flujo.
    pub label: String,
    /// Perfil de origen.
    pub profile: String,
    /// Epoch de creación.
    pub created_at: u64,
    /// Bytes totales.
    pub total_bytes: u64,
    /// Tamaño de trozo.
    pub chunk_bytes: u64,
    /// SHA-256 del flujo completo.
    pub sha256: String,
    /// Trozos (con su SHA-256).
    pub chunks: Vec<ChunkRecord>,
    /// Ficheros (con su SHA-256).
    pub files: Vec<FileRecord>,
    /// Tramos (objetos de Drive).
    pub parts: Vec<PartRecord>,
    /// Avisos del plan.
    pub warnings: Vec<String>,
}

impl StreamManifest {
    /// Informe JSON.
    pub fn to_json(&self) -> Value {
        json!({
            "ok": true,
            "label": self.label,
            "profile": self.profile,
            "total_bytes": self.total_bytes,
            "chunk_bytes": self.chunk_bytes,
            "sha256": self.sha256,
            "chunks": self.chunks.len(),
            "files": self.files.len(),
            "parts": self.parts,
            "warnings": self.warnings,
        })
    }
}

/// Calcula el tamaño **exacto** del TAR que se va a subir.
///
/// Hace falta porque Drive **no finaliza** una sesión si el total es
/// desconocido (medido: 308 para siempre), y el total tiene que estar
/// **antes** de empezar a subir ⇒ hay que saber el tamaño del TAR de
/// antemano: 512 por cabecera de entrada + contenido alineado + 1024 de fin.
pub fn tar_total_bytes(manifest_len: u64, files: &[u64]) -> u64 {
    let mut total = 512 + pad512(manifest_len);
    for size in files {
        total += 512 + pad512(*size);
    }
    total + 1024
}

fn pad512(n: u64) -> u64 {
    (512 - (n % 512)) % 512
}

/// Calcula el manifiesto de ficheros (SHA-256 de cada uno).
pub fn hash_files(plan: &ProfilePlan) -> Result<Vec<FileRecord>, String> {
    let mut out = Vec::new();
    for e in plan.included() {
        let (size, sha) = crate::backup_crypto::sha256_file(Path::new(&e.path))?;
        out.push(FileRecord {
            path: e.rel_path.clone(),
            size,
            sha256: sha,
        });
    }
    Ok(out)
}

/// Escribe el perfil como un **TAR en streaming** directamente a la tubería.
///
/// El contenido se empuja a Drive **mientras se genera**: no se escribe ni un
/// byte en el móvil (que sólo tiene 10 GB libres) y la subida empieza antes
/// de que el perfil esté completo.
///
/// Devuelve el manifiesto del stream (el llamante lo sube al final).
pub fn stream_to_drive(
    plan: &ProfilePlan,
    pipeline: &mut Pipeline,
    chunk_bytes: u64,
) -> Result<StreamManifest, String> {
    // 1) Hash de los ficheros (paso 1) ⇒ manifiesto y tamaño exacto.
    let files = hash_files(plan)?;
    let manifest_json = serde_json::to_string(&files).map_err(|e| format!("json: {e}"))?;
    let sizes: Vec<u64> = files.iter().map(|f| f.size).collect();
    let total = tar_total_bytes(manifest_json.len() as u64, &sizes);

    // 2) Escritura del TAR en streaming.
    let label = pipeline.label().to_string();
    let mut pusher = TarPusher {
        pipeline,
        chunks: Vec::new(),
        index: 0,
        offset: 0,
    };

    // 2.a) La entrada del manifiesto va primero (viaja con los datos).
    let header = crate::backup_crypto::tar_header(
        "profile-manifest.json",
        manifest_json.len() as u64,
        0o644,
        0,
    );
    pusher.push(&header)?;
    pusher.push(manifest_json.as_bytes())?;
    let pad = pad512(manifest_json.len() as u64);
    if pad > 0 {
        pusher.push(&vec![0u8; pad as usize])?;
    }

    // 2.b) Los ficheros, uno a uno.
    for (f, e) in files.iter().zip(plan.included()) {
        let header = crate::backup_crypto::tar_header(&f.path, f.size, 0o600, e.mtime);
        pusher.push(&header)?;
        let data = std::fs::read(&e.path).map_err(|err| format!("leer {}: {err}", e.path))?;
        if data.len() as u64 != f.size {
            return Err(format!(
                "{} cambió mientras se respaldaba ({} ≠ {})",
                e.path,
                data.len(),
                f.size
            ));
        }
        pusher.push(&data)?;
        let pad = pad512(f.size);
        if pad > 0 {
            pusher.push(&vec![0u8; pad as usize])?;
        }
    }

    // 2.c) Fin del TAR (1024 bytes de ceros).
    pusher.push(&vec![0u8; 1024])?;

    Ok(StreamManifest {
        label,
        profile: plan.policy.profile.as_str().to_string(),
        created_at: crate::backup_stream::now_secs(),
        total_bytes: total,
        chunk_bytes,
        sha256: String::new(),
        chunks: pusher.chunks,
        files,
        parts: Vec::new(),
        warnings: plan.warnings.clone(),
    })
}

/// Escribe el TAR byte a byte en la tubería, anotando cada trozo.
struct TarPusher<'a> {
    pipeline: &'a mut Pipeline,
    chunks: Vec<ChunkRecord>,
    index: u64,
    offset: u64,
}

impl TarPusher<'_> {
    fn push(&mut self, data: &[u8]) -> Result<(), String> {
        let sha = self.pipeline.push(data.to_vec())?;
        self.chunks.push(ChunkRecord {
            index: self.index,
            offset: self.offset,
            len: data.len() as u64,
            sha256: sha,
        });
        self.index += 1;
        self.offset += data.len() as u64;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Verificación del stream
// ---------------------------------------------------------------------------

/// Informe de la verificación de un stream (tramos en disco).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamVerify {
    /// ¿Todo cuadra?
    pub ok: bool,
    /// Trozos comprobados.
    pub chunks_checked: usize,
    /// Trozos que no cuadran.
    pub chunks_bad: Vec<String>,
    /// Ficheros comprobados.
    pub files_checked: usize,
    /// Ficheros que no cuadran.
    pub files_bad: Vec<String>,
    /// Bytes leídos.
    pub bytes: u64,
    /// Detalle legible.
    pub detail: String,
}

/// Verifica los tramos de un stream contra su manifiesto.
///
/// `dir` es donde están los ficheros `partNN`; se comprueba **cada trozo**
/// con su SHA-256 y luego **cada fichero** del TAR.
pub fn verify_stream(dir: &Path, manifest: &StreamManifest) -> Result<StreamVerify, String> {
    let mut chunks_checked = 0usize;
    let mut chunks_bad = Vec::new();
    let mut bytes = 0u64;
    for c in &manifest.chunks {
        // Los trozos se verifican leyendo el tramo que los contiene.
        let stream = chunk_stream(manifest, c.index);
        let path = dir.join(format!("part{:02}", stream));
        let data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(e) => {
                chunks_bad.push(format!("tramo {stream} ilegible: {e}"));
                continue;
            }
        };
        let off = (c.offset - stream_offset(manifest, stream)) as usize;
        let end = off + c.len as usize;
        if end > data.len() {
            chunks_bad.push(format!("trozo {} fuera del tramo", c.index));
            continue;
        }
        let sha = crate::backup_stream::hex(&Sha256::digest(&data[off..end]));
        chunks_checked += 1;
        bytes += c.len;
        if sha != c.sha256 {
            chunks_bad.push(format!("trozo {}", c.index));
        }
    }
    // 2) Ficheros: se reextraen del TAR y se rehashean.
    let mut files_checked = 0usize;
    let mut files_bad = Vec::new();
    let tar = reassemble(dir, manifest)?;
    if let Ok(decoded) = untar(&tar) {
        for (path, sha) in decoded {
            files_checked += 1;
            match manifest.files.iter().find(|f| f.path == path) {
                Some(f) if f.sha256 == sha => {}
                _ => files_bad.push(path),
            }
        }
    }
    let ok = chunks_bad.is_empty() && files_bad.is_empty();
    Ok(StreamVerify {
        ok,
        chunks_checked,
        chunks_bad,
        files_checked,
        files_bad,
        bytes,
        detail: format!(
            "{} trozos y {} ficheros comprobados",
            chunks_checked, files_checked
        ),
    })
}

/// Índice de tramo de un trozo (reparto contiguo y alineado).
fn chunk_stream(manifest: &StreamManifest, index: u64) -> usize {
    let n_chunks = manifest.chunks.len() as u64;
    let parts = manifest.parts.len().max(1) as u64;
    let base = n_chunks / parts;
    let mut extra = n_chunks % parts;
    let mut ini = 0u64;
    for c in 0..parts {
        let mut n = base;
        if extra > 0 {
            n += 1;
            extra -= 1;
        }
        if index >= ini && index < ini + n {
            return c as usize;
        }
        ini += n;
    }
    0
}

/// Offset (bytes) del primer trozo del tramo.
fn stream_offset(manifest: &StreamManifest, stream: usize) -> u64 {
    let n_chunks = manifest.chunks.len() as u64;
    let parts = manifest.parts.len().max(1) as u64;
    let base = n_chunks / parts;
    let mut extra = n_chunks % parts;
    let mut ini = 0u64;
    for c in 0..parts {
        let mut n = base;
        if extra > 0 {
            n += 1;
            extra -= 1;
        }
        if c as usize == stream {
            return ini * manifest.chunk_bytes;
        }
        ini += n;
    }
    0
}

/// Reensambla los tramos en el TAR completo.
pub fn reassemble(dir: &Path, manifest: &StreamManifest) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(manifest.total_bytes as usize);
    for s in 0..manifest.parts.len() {
        let path = dir.join(format!("part{:02}", s));
        let data = std::fs::read(&path).map_err(|e| format!("leer {}: {e}", path.display()))?;
        out.extend_from_slice(&data);
    }
    Ok(out)
}

/// Extrae los ficheros del TAR y devuelve (ruta, sha256).
pub fn untar(tar: &[u8]) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    let mut off = 0usize;
    while off + 512 <= tar.len() {
        let hdr = &tar[off..off + 512];
        if hdr.iter().all(|b| *b == 0) {
            break;
        }
        let name = String::from_utf8_lossy(&hdr[0..100])
            .trim_end_matches('\0')
            .to_string();
        let size_str: String = String::from_utf8_lossy(&hdr[124..136])
            .trim_end_matches('\0')
            .trim_end_matches(' ')
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect();
        let size = u64::from_str_radix(&size_str, 8).unwrap_or(0);
        let start = off + 512;
        let end = (start + size as usize).min(tar.len());
        if !name.is_empty() && name != "profile-manifest.json" {
            let sha = crate::backup_stream::hex(&Sha256::digest(&tar[start..end]));
            out.push((name, sha));
        }
        off = start + ((size as usize + 511) / 512) * 512;
    }
    Ok(out)
}

/// Escribe un fichero de texto (manifiesto) en disco.
pub fn write_manifest(path: &Path, manifest: &StreamManifest) -> Result<(), String> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("crear {}: {e}", p.display()))?;
    }
    let data = serde_json::to_vec_pretty(manifest).map_err(|e| format!("json: {e}"))?;
    std::fs::write(path, data).map_err(|e| format!("escribir {}: {e}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_tamano_del_tar_es_exacto() {
        // 1 fichero de 100 bytes + manifiesto de 10 bytes.
        let t = tar_total_bytes(10, &[100]);
        // manifiesto: 512 + 512 (10 bytes + relleno) = 1024
        // fichero:  512 + 512 = 1024
        // fin:      1024
        assert_eq!(t, 1024 + 1024 + 1024);
    }

    #[test]
    fn las_caches_se_excluyen() {
        let p = ProfilePolicy::default();
        assert!(is_cache_path("/data/user/0/x/cache/y"));
        assert!(is_cache_path("/data/user/0/x/code_cache/y"));
        assert!(!is_cache_path("/data/user/0/x/shared_prefs/y.xml"));
        assert!(p.matches_exclusion("/data/user/0/x/cache/y"));
    }

    #[test]
    fn los_perfiles_declaran_sus_cubos() {
        assert!(BackupProfile::P1Profile
            .buckets()
            .contains(&Bucket::Certificates));
        assert!(BackupProfile::P3Raw.buckets().is_empty());
    }
}
