//! **Motor de INVENTARIO EXHAUSTIVO** del móvil (S21 Ultra con root).
//!
//! Este módulo implementa, en Rust puro, el plan de
//! `docs/INVENTARIO_EXHAUSTIVO.md`: una matriz **P0..P5** de qué se recoge
//! y de dónde, un escaneo **por lotes** que no deja el móvil tonto, un
//! contenedor cifrado con **AEGIS-256** (clave separada para lo sensible),
//! una verificación y un *restore* con `--confirm` obligatorio.
//!
//! ## La regla de oro
//!
//! **De los secretos sólo sale su `sha16` y su tamaño.** Ni al repo, ni al
//! chat, ni al tablón. El valor viaja **dentro del contenedor cifrado** y la
//! clave se entrega por un canal `0600` (ver [`KeyMaterial`]).
//!
//! ## Anti-congelación (requisito del usuario)
//!
//! El escaneo del móvil entero (`/data/user/0` con 1.721 paquetes) **no cabe**
//! en una sola pasada razonable: medido en el S21 Ultra, un `du -sk` completo
//! de `/data/user/0` tarda **~19 s** y un `find` de ficheros a profundidad 4
//! tarda **>12 s**. Por eso el motor:
//!
//! * trabaja en **lotes** (`batch_size`) con **pausa** entre lotes
//!   (`pause_ms`);
//! * baja su propia prioridad de I/O (`nice` + `ionice`, ver [`apply_io_limits`]);
//! * **muestra el progreso** mientras va;
//! * tiene `--dry-run` (enumera sin leer contenido).
//!
//! ## Estructura del contenedor `MDBINV1`
//!
//! ```text
//! +------------------+ 8 B    magic
//! | "MDBINV1\n"     |
//! +------------------+ 4096 B cabecera JSON (sin secretos)
//! | streams[]        |   payload  : tar(zstd) con el manifiesto + lo normal
//! |                  |   sensitive: tar(zstd) con lo sensible (CLAVE APARTE)
//! +------------------+
//! | marcos AEGIS-256 |   [u32 len][nonce 32][cifrado][etiqueta 32] ...
//! +------------------+
//! ```
//!
//! El manifiesto (`manifest.json`) lleva **sha256 por fichero**, ruta,
//! tamaño y mtime: **ningún valor**.

use crate::backup_crypto::{
    hex, key_check, sha256_hex, tar_header, unhex, ChunkCipher, KdfParams, AEGIS_KEY_LEN,
    LABEL_KEY_CHECK,
};
use crate::backup_formats::{safe_relative_path, TarStream};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ========================================================================= //
// Constantes del formato
// ========================================================================= //

/// Magic del contenedor de inventario.
pub const INVENTORY_MAGIC: &[u8; 8] = b"MDBINV1\n";
/// Versión del formato.
pub const INVENTORY_VERSION: u32 = 1;
/// Tamaño de la cabecera JSON (fijo ⇒ se puede parchear).
pub const INVENTORY_HEADER_SIZE: usize = 4096;
/// Etiqueta del *key-check* de la clave principal.
pub const LABEL_MAIN_KEY_CHECK: &str = "MDBINV1-aegis256-key-check-v1";
/// Etiqueta del *key-check* de la clave sensible.
pub const LABEL_SENSITIVE_KEY_CHECK: &str = "MDBINV1-aegis256-sensitive-key-check-v1";
/// Tamaño por defecto del trozo cifrado.
pub const INVENTORY_CHUNK_SIZE: usize = 256 * 1024;
/// Nombre del fichero de manifiesto dentro del tar.
pub const INVENTORY_MANIFEST_ENTRY: &str = "manifest.json";
/// Ruta dentro del tar donde va la clave sensible.
pub const INVENTORY_KEY_ENTRY: &str = "KEYS/sensitive.key";
/// Prefijo de los datos dentro del tar.
pub const INVENTORY_DATA_PREFIX: &str = "data/";

// ========================================================================= //
// La matriz P0..P5
// ========================================================================= //

/// Nivel de la matriz del plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Level {
    /// P0: identidad y acceso.
    P0,
    /// P1: navegadores.
    P1,
    /// P2: apps (APK + datos).
    P2,
    /// P3: comunicaciones y multimedia.
    P3,
    /// P4: estado del sistema.
    P4,
    /// P5: exclusiones (lo que NO se copia).
    P5,
}

impl Level {
    /// Identificador estable (`P0`, `P1`, ...).
    pub fn id(self) -> &'static str {
        match self {
            Level::P0 => "P0",
            Level::P1 => "P1",
            Level::P2 => "P2",
            Level::P3 => "P3",
            Level::P4 => "P4",
            Level::P5 => "P5",
        }
    }

    /// Nombre del perfil de exportación.
    pub fn profile(self) -> &'static str {
        match self {
            Level::P0 => "P0-identidad",
            Level::P1 => "P1-navegadores",
            Level::P2 => "P2-apps",
            Level::P3 => "P3-comunicaciones",
            Level::P4 => "P4-sistema",
            Level::P5 => "P5-exclusiones",
        }
    }

    /// Todos los niveles, en orden.
    pub fn all() -> [Level; 6] {
        [
            Level::P0,
            Level::P1,
            Level::P2,
            Level::P3,
            Level::P4,
            Level::P5,
        ]
    }

    /// Parsea un identificador (`P0`, `p0`, `P0-identidad`, ...).
    pub fn parse(text: &str) -> Option<Level> {
        let t = text.trim().to_ascii_lowercase();
        for l in Level::all() {
            if t == l.id().to_ascii_lowercase() || t == l.profile().to_ascii_lowercase() {
                return Some(l);
            }
        }
        None
    }
}

/// Cómo se hashea un fichero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HashPolicy {
    /// No se hashea (sólo ruta/tamaño/mtime).
    None,
    /// Se hashea siempre (sha16 del contenido).
    Full,
    /// Se hashea sólo si el fichero cabe en el tope.
    Small(u64),
}

/// Una regla de la matriz: qué se recoge, de dónde y con qué cuidado.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Rule {
    /// Identificador estable de la regla.
    pub id: &'static str,
    /// Nivel al que pertenece.
    pub level: Level,
    /// Descripción legible.
    pub label: &'static str,
    /// Rutas absolutas en el dispositivo.
    pub paths: &'static [&'static str],
    /// ¿Se recorre el contenido del directorio?
    pub walk: bool,
    /// Profundidad máxima del recorrido.
    pub max_depth: usize,
    /// ¿Es **sensible** (clave de cifrado aparte)?
    pub sensitive: bool,
    /// Política de hash.
    pub hash: HashPolicy,
}

/// Una regla que depende del **paquete** (se resuelve en tiempo de escaneo).
#[derive(Debug, Clone, PartialEq)]
pub struct PackageRule {
    /// Identificador estable.
    pub id: &'static str,
    /// Nivel.
    pub level: Level,
    /// Descripción legible.
    pub label: &'static str,
    /// Prefijo bajo `/data/user/0`.
    pub data_subdir: &'static str,
    /// ¿Se excluyen cachés?
    pub exclude_cache: bool,
    /// ¿Es sensible?
    pub sensitive: bool,
}

/// La matriz completa del plan.
pub fn matrix() -> Vec<Rule> {
    vec![
        // ---------------- P0 · identidad y acceso ----------------
        Rule {
            id: "p0.wifi",
            level: Level::P0,
            label: "Claves WiFi guardadas (SSID + clave)",
            paths: &[
                "/data/misc/wifi/WifiConfigStore.xml",
                "/data/misc/apexdata/com.android.wifi/WifiConfigStore.xml",
                "/data/misc/wifi/softap.conf",
                "/data/misc/apexdata/com.android.wifi/WifiConfigStoreSoftAp.xml",
            ],
            walk: false,
            max_depth: 0,
            sensitive: true,
            hash: HashPolicy::Full,
        },
        Rule {
            id: "p0.wifi_certs",
            level: Level::P0,
            label: "Certificados de cliente WiFi (EAP)",
            paths: &["/data/misc/apexdata/com.android.wifi"],
            walk: true,
            max_depth: 3,
            sensitive: true,
            hash: HashPolicy::Full,
        },
        Rule {
            id: "p0.vpn",
            level: Level::P0,
            label: "Configuración VPN",
            paths: &["/data/misc/vpn", "/data/user/0/com.android.vpnmanager"],
            walk: true,
            max_depth: 4,
            sensitive: true,
            hash: HashPolicy::Full,
        },
        Rule {
            id: "p0.unlock",
            level: Level::P0,
            label: "Credenciales de desbloqueo (gatekeeper)",
            paths: &[
                "/data/system/gatekeeper.password.key",
                "/data/system/gatekeeper.pattern.key",
                "/data/system/gatekeeper.password_wallpaper.key",
                "/data/system/users/0/ENCRYPTED_KEY_SBrowser_00_0",
                "/data/system/users/0/ENCRYPTED_KEY_SdpSecureDataKey_0",
            ],
            walk: false,
            max_depth: 0,
            sensitive: true,
            hash: HashPolicy::Full,
        },
        Rule {
            id: "p0.accounts",
            level: Level::P0,
            label: "AccountsManager (tokens de autenticación)",
            paths: &[
                "/data/system/users/0/accounts.db",
                "/data/system_ce/0/accounts_ce.db",
                "/data/system_de/0/accounts_de.db",
            ],
            walk: false,
            max_depth: 0,
            sensitive: true,
            hash: HashPolicy::Full,
        },
        Rule {
            id: "p0.2fa",
            level: Level::P0,
            label: "Autenticadores 2FA (TOTP)",
            paths: &[
                "/data/user/0/com.google.android.apps.authenticator2/databases",
                "/data/user/0/com.azure.authenticator/databases",
                "/data/user/0/com.beem/databases",
                "/data/user/0/com.beem.libre/databases",
                "/data/user/0/com.mufri.jp2aegis/databases",
                "/data/user/0/org.fedorahosted.freeyourgadget.gadgetbridge/databases",
            ],
            walk: true,
            max_depth: 3,
            sensitive: true,
            hash: HashPolicy::Full,
        },
        Rule {
            id: "p0.passman",
            level: Level::P0,
            label: "Gestores de contraseñas",
            paths: &[
                "/data/user/0/com.androidkeepass/files",
                "/data/user/0/com.x8bit.bitwarden/databases",
                "/data/user/0/com.x8bit.bitwarden/files",
                "/data/user/0/com.lastpass.lpandroid/databases",
                "/data/user/0/com.keepass.android/files",
                "/data/user/0/com.ritsu.SecureNotes/files",
            ],
            walk: true,
            max_depth: 4,
            sensitive: true,
            hash: HashPolicy::Full,
        },
        Rule {
            id: "p0.usercerts",
            level: Level::P0,
            label: "Certificados del usuario (keychain/keystore)",
            paths: &[
                "/data/misc/keychain",
                "/data/misc/keystore",
                "/data/misc/keystore/user_0",
            ],
            walk: true,
            max_depth: 3,
            sensitive: true,
            hash: HashPolicy::Full,
        },
        Rule {
            id: "p0.megadaemon",
            level: Level::P0,
            label: "Token de sesión propio de MegaDaemon",
            paths: &[
                "/data/local/tmp/megadaemon/state/megadaemon_api.key",
                "/data/local/tmp/megadaemon/state/megadaemon_api.keys",
                "/data/local/tmp/megadaemon/tunnel.url",
                "/data/local/tmp/megadaemon/channel.json",
            ],
            walk: false,
            max_depth: 0,
            sensitive: true,
            hash: HashPolicy::Full,
        },
        // ---------------- P1 · navegadores ----------------
        Rule {
            id: "p1.chrome",
            level: Level::P1,
            label: "Chrome (perfil Default)",
            paths: &["/data/user/0/com.android.chrome/app_chrome/Default"],
            walk: true,
            max_depth: 4,
            sensitive: true,
            hash: HashPolicy::Small(8 * 1024 * 1024),
        },
        Rule {
            id: "p1.edge",
            level: Level::P1,
            label: "Microsoft Edge",
            paths: &[
                "/data/user/0/com.microsoft.emmx/app_chrome/Default",
                "/data/user/0/com.microsoft.emmx/files",
            ],
            walk: true,
            max_depth: 4,
            sensitive: true,
            hash: HashPolicy::Small(8 * 1024 * 1024),
        },
        Rule {
            id: "p1.sbrowser",
            level: Level::P1,
            label: "Samsung Internet (+ beta)",
            paths: &[
                "/data/user/0/com.sec.android.app.sbrowser/files",
                "/data/user/0/com.sec.android.app.sbrowser.beta/files",
            ],
            walk: true,
            max_depth: 4,
            sensitive: true,
            hash: HashPolicy::Small(8 * 1024 * 1024),
        },
        Rule {
            id: "p1.tor",
            level: Level::P1,
            label: "Tor Browser",
            paths: &["/data/user/0/org.torproject.torbrowser/files"],
            walk: true,
            max_depth: 4,
            sensitive: true,
            hash: HashPolicy::Small(8 * 1024 * 1024),
        },
        Rule {
            id: "p1.fennec",
            level: Level::P1,
            label: "Fennec / Firefox",
            paths: &[
                "/data/user/0/org.mozilla.firefox/files",
                "/data/user/0/us.spotco.fennec_dos/files",
            ],
            walk: true,
            max_depth: 4,
            sensitive: true,
            hash: HashPolicy::Small(8 * 1024 * 1024),
        },
        Rule {
            id: "p1.others",
            level: Level::P1,
            label: "Otros navegadores (Brave, Opera, Vivaldi, DuckDuckGo...)",
            paths: &[
                "/data/user/0/com.brave.browser/app_webview/Default",
                "/data/user/0/com.opera.browser/files",
                "/data/user/0/com.vivaldi.browser/files",
                "/data/user/0/com.duckduckgo.mobile.android/files",
                "/data/user/0/com.kiwibrowser.browser/files",
                "/data/user/0/com.yandex.browser/files",
            ],
            walk: true,
            max_depth: 4,
            sensitive: true,
            hash: HashPolicy::Small(8 * 1024 * 1024),
        },
        // ---------------- P3 · comunicaciones ----------------
        Rule {
            id: "p3.telephony",
            level: Level::P3,
            label: "SMS/MMS y registro de llamadas",
            paths: &[
                "/data/user_de/0/com.android.providers.telephony/databases",
                "/data/user_de/0/com.android.providers.contacts/databases",
                "/data/user/0/com.android.providers.telephony/databases",
            ],
            walk: true,
            max_depth: 3,
            sensitive: true,
            hash: HashPolicy::Full,
        },
        Rule {
            id: "p3.rcs",
            level: Level::P3,
            label: "RCS / Google Messages",
            paths: &[
                "/data/user/0/com.google.android.apps.messaging/databases",
                "/data/user/0/com.google.android.rcs/databases",
            ],
            walk: true,
            max_depth: 3,
            sensitive: true,
            hash: HashPolicy::Full,
        },
        Rule {
            id: "p3.messengers",
            level: Level::P3,
            label: "Mensajería (WhatsApp, Telegram, Signal, Discord)",
            paths: &[
                "/data/user/0/com.whatsapp/databases",
                "/data/user/0/com.whatsapp/media",
                "/data/user/0/org.telegram.messenger/databases",
                "/data/user/0/org.telegram.messenger/files",
                "/data/user/0/org.thoughtcrime.securesms/databases",
                "/data/user/0/com.discord/databases",
                "/data/user/0/com.discord/files",
            ],
            walk: true,
            max_depth: 4,
            sensitive: true,
            hash: HashPolicy::Full,
        },
        Rule {
            id: "p3.keyboard",
            level: Level::P3,
            label: "Diccionario aprendido del teclado",
            paths: &[
                "/data/user/0/com.google.android.inputmethod.latin/databases",
                "/data/user/0/com.google.android.inputmethod.latin/shared_prefs",
            ],
            walk: true,
            max_depth: 3,
            sensitive: false,
            hash: HashPolicy::Full,
        },
        Rule {
            id: "p3.media",
            level: Level::P3,
            label: "Fotos, vídeos y multimedia",
            paths: &[
                "/sdcard/DCIM",
                "/sdcard/Pictures",
                "/sdcard/Movies",
                "/sdcard/Documents",
                "/sdcard/Download",
                "/sdcard/Recordings",
                "/sdcard/Music",
            ],
            walk: false,
            max_depth: 0,
            sensitive: false,
            hash: HashPolicy::None,
        },
        Rule {
            id: "p3.android_data",
            level: Level::P3,
            label: "Android/data y Android/obb",
            paths: &["/sdcard/Android/data", "/sdcard/Android/obb"],
            walk: false,
            max_depth: 0,
            sensitive: false,
            hash: HashPolicy::None,
        },
        // ---------------- P4 · sistema ----------------
        Rule {
            id: "p4.settings",
            level: Level::P4,
            label: "Ajustes del sistema",
            paths: &[
                "/data/system/users/0/settings_system.xml",
                "/data/system/users/0/settings_secure.xml",
                "/data/system/users/0/settings_global.xml",
                "/data/system/users/0/package-restrictions.xml",
            ],
            walk: false,
            max_depth: 0,
            sensitive: false,
            hash: HashPolicy::Full,
        },
        Rule {
            id: "p4.props",
            level: Level::P4,
            label: "Props de arranque",
            paths: &[
                "/system/build.prop",
                "/vendor/build.prop",
                "/product/build.prop",
                "/data/prop/default.prop",
            ],
            walk: false,
            max_depth: 0,
            sensitive: false,
            hash: HashPolicy::Full,
        },
        Rule {
            id: "p4.magisk",
            level: Level::P4,
            label: "Módulos de Magisk",
            paths: &["/data/adb/modules", "/data/adb/magisk.db"],
            walk: true,
            max_depth: 3,
            sensitive: false,
            hash: HashPolicy::Small(1024 * 1024),
        },
        Rule {
            id: "p4.csc",
            level: Level::P4,
            label: "CSC / configuración del operador",
            paths: &["/data/vendor/csc", "/data/vendor/secradio/cscver"],
            walk: true,
            max_depth: 3,
            sensitive: false,
            hash: HashPolicy::Small(1024 * 1024),
        },
        Rule {
            id: "p4.megadaemon",
            level: Level::P4,
            label: "Estado de MegaDaemon",
            paths: &["/data/local/tmp/megadaemon"],
            walk: true,
            max_depth: 3,
            sensitive: false,
            hash: HashPolicy::Small(1024 * 1024),
        },
        Rule {
            id: "p4.antigravity",
            level: Level::P4,
            label: "Antigravity (backlog, brain, conversaciones, mcp)",
            paths: &["/data/user/0/com.agy/files/.gemini/antigravity-app"],
            walk: true,
            max_depth: 4,
            sensitive: false,
            hash: HashPolicy::Small(1024 * 1024),
        },
        // ---------------- P2 (dinámica) · apps ----------------
        Rule {
            id: "p2.bluetooth",
            level: Level::P2,
            label: "Bluetooth (claves de enlace de los emparejados)",
            paths: &[
                "/data/misc/bluedroid/bt_config.conf",
                "/data/misc/bluedroid/downloadable_bt.db",
                "/data/user/0/com.android.bluetooth",
            ],
            walk: true,
            max_depth: 3,
            sensitive: true,
            hash: HashPolicy::Full,
        },
        Rule {
            id: "p2.system_pkgs",
            level: Level::P2,
            label: "Datos de sistema de paquetes",
            paths: &[
                "/data/system/packages.xml",
                "/data/system/users/0/instant",
                "/data/vendor",
            ],
            walk: true,
            max_depth: 3,
            sensitive: false,
            hash: HashPolicy::Small(1024 * 1024),
        },
    ]
}

/// Reglas que dependen del **paquete instalado**.
pub fn package_rules() -> Vec<PackageRule> {
    vec![
        PackageRule {
            id: "p2.appdata",
            level: Level::P2,
            label: "Datos privados de la app (/data/user/0/<pkg>)",
            data_subdir: "",
            exclude_cache: true,
            sensitive: true,
        },
        PackageRule {
            id: "p2.apk",
            level: Level::P2,
            label: "APK instalado (pm path)",
            data_subdir: "__apk__",
            exclude_cache: false,
            sensitive: false,
        },
    ]
}

/// Patrones de exclusión (P5): lo regenerable que **no** se copia.
pub const EXCLUSION_PATTERNS: &[&str] = &[
    "/cache/",
    "/code_cache/",
    "/no_backup/",
    "/thumbnails/",
    // MEDIDO 2026-11-22: Android nombra la carpeta de miniaturas del usuario
    // con punto delante (".thumbnails"), y el patron "/thumbnails/" NO la
    // cubria (test exclusiones_p5).
    ".thumbnails",
    ".tmp",
    ".crdownload",
];

// ========================================================================= //
// Dispositivo: la frontera con el móvil real
// ========================================================================= //

/// Metadatos de un fichero del dispositivo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileMeta {
    /// Ruta absoluta.
    pub path: String,
    /// Tamaño en bytes.
    pub size: u64,
    /// mtime (epoch).
    pub mtime: u64,
    /// ¿Es directorio?
    pub is_dir: bool,
}

/// Una entrada de directorio.
#[derive(Debug, Clone, PartialEq)]
pub struct DirItem {
    /// Nombre de la entrada.
    pub name: String,
    /// ¿Es directorio?
    pub is_dir: bool,
}

/// Información de un paquete instalado.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackageInfo {
    /// Nombre del paquete.
    pub package: String,
    /// ¿Es del sistema?
    pub system: bool,
    /// ¿Está deshabilitado?
    pub disabled: bool,
    /// Ruta del APK (si se conoce).
    pub apk: Option<String>,
    /// Bytes de datos privados (medido en el escaneo).
    pub data_bytes: u64,
}

/// La frontera con el dispositivo: el motor **no** sabe si habla con el móvil
/// real o con un doble de pruebas.
pub trait Device {
    /// ¿Existe la ruta?
    fn exists(&self, path: &str) -> bool;
    /// Metadatos (o `None` si no existe).
    fn meta(&self, path: &str) -> Option<FileMeta>;
    /// Contenido de un directorio.
    fn list_dir(&self, path: &str) -> Vec<DirItem>;
    /// Lee un fichero entero (sólo para hashear; el motor limita el tamaño).
    fn read_file(&self, path: &str) -> Option<Vec<u8>>;
    /// Paquetes instalados.
    fn packages(&self) -> Vec<PackageInfo>;
    /// Rutas de los APK de un paquete.
    fn apk_paths(&self, package: &str) -> Vec<String>;
    /// `loadavg` del dispositivo.
    fn loadavg(&self) -> String;
}

/// Implementación real: el sistema de ficheros del móvil (el daemon es root).
pub struct RealDevice {
    /// Raíz del dispositivo (`/` en el móvil; útil en tests).
    pub root: String,
}

impl RealDevice {
    /// Crea el dispositivo real con raíz `/`.
    pub fn new() -> RealDevice {
        RealDevice {
            root: "/".to_string(),
        }
    }

    fn full(&self, path: &str) -> PathBuf {
        if self.root == "/" {
            PathBuf::from(path)
        } else {
            Path::new(&self.root).join(path.trim_start_matches('/'))
        }
    }
}

impl Default for RealDevice {
    fn default() -> Self {
        RealDevice::new()
    }
}

impl Device for RealDevice {
    fn exists(&self, path: &str) -> bool {
        self.full(path).symlink_metadata().is_ok()
    }

    fn meta(&self, path: &str) -> Option<FileMeta> {
        let p = self.full(path);
        let md = std::fs::symlink_metadata(&p).ok()?;
        let mtime = md
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Some(FileMeta {
            path: path.to_string(),
            size: if md.is_dir() { 0 } else { md.len() },
            mtime,
            is_dir: md.is_dir(),
        })
    }

    fn list_dir(&self, path: &str) -> Vec<DirItem> {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(self.full(path)) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                out.push(DirItem { name, is_dir });
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    fn read_file(&self, path: &str) -> Option<Vec<u8>> {
        std::fs::read(self.full(path)).ok()
    }

    fn packages(&self) -> Vec<PackageInfo> {
        let mut out = Vec::new();
        let mut sys = std::collections::BTreeSet::new();
        let mut dis = std::collections::BTreeSet::new();
        if let Some(s) = run_capture(&self.root, &["pm", "list", "packages", "-s"]) {
            for l in s.lines() {
                if let Some(p) = l.trim().strip_prefix("package:") {
                    sys.insert(p.to_string());
                }
            }
        }
        if let Some(s) = run_capture(&self.root, &["pm", "list", "packages", "-d"]) {
            for l in s.lines() {
                if let Some(p) = l.trim().strip_prefix("package:") {
                    dis.insert(p.to_string());
                }
            }
        }
        let all = run_capture(&self.root, &["pm", "list", "packages"]).unwrap_or_default();
        for l in all.lines() {
            if let Some(p) = l.trim().strip_prefix("package:") {
                out.push(PackageInfo {
                    package: p.to_string(),
                    system: sys.contains(p),
                    disabled: dis.contains(p),
                    apk: None,
                    data_bytes: 0,
                });
            }
        }
        out
    }

    fn apk_paths(&self, package: &str) -> Vec<String> {
        run_capture(&self.root, &["pm", "path", package])
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.trim().strip_prefix("package:").map(|s| s.to_string()))
            .collect()
    }

    fn loadavg(&self) -> String {
        let p = self.full("/proc/loadavg");
        std::fs::read_to_string(p)
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    }
}

/// Ejecuta un comando y devuelve su stdout (vacío si falla).
fn run_capture(root: &str, cmd: &[&str]) -> Option<String> {
    let mut c = std::process::Command::new(cmd[0]);
    for a in &cmd[1..] {
        c.arg(a);
    }
    if root != "/" {
        c.current_dir(root);
    }
    let out = c.output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

// ========================================================================= //
// Anti-congelación: límites de I/O
// ========================================================================= //

/// Baja la prioridad de I/O del proceso actual.
///
/// * `nice`: prioridad de CPU (0 = normal, 19 = la más baja).
/// * `ionice_class`: 1 = realtime, 2 = best-effort, 3 = idle (la más baja).
///
/// Se aplica al **propio proceso**: el escaneo nunca compite con el usuario.
pub fn apply_io_limits(nice: i32, ionice_class: u8) -> String {
    let mut notes = Vec::new();
    // nice: `setpriority(PRIO_PROCESS, 0, nice)`.
    let rc = unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, nice) };
    notes.push(format!(
        "nice={} ({})",
        nice,
        if rc == 0 { "ok" } else { "no aplicado" }
    ));
    // ionice: `ioprio_set(IOPRIO_WHO_PROCESS, 0, prio)` vía syscall.
    #[cfg(target_arch = "aarch64")]
    const SYS_IOPRIO_SET: libc::c_long = 252;
    #[cfg(target_arch = "x86_64")]
    const SYS_IOPRIO_SET: libc::c_long = 251;
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    const SYS_IOPRIO_SET: libc::c_long = -1;
    if SYS_IOPRIO_SET > 0 {
        let class = (ionice_class as i32).clamp(0, 3);
        let prio = (class << 13) | 4; // data=4 (prioridad baja dentro de la clase)
        let rc = unsafe { libc::syscall(SYS_IOPRIO_SET, 1i32, 0i32, prio) };
        notes.push(format!(
            "ionice_class={} ({})",
            class,
            if rc == 0 { "ok" } else { "no aplicado" }
        ));
    }
    notes.join(" ")
}

/// Carga del sistema (1 min, 5 min, 15 min).
pub fn loadavg_tuple(loadavg: &str) -> (f64, f64, f64) {
    let mut it = loadavg.split_whitespace();
    let a = it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let b = it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let c = it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    (a, b, c)
}

// ========================================================================= //
// El escaneo
// ========================================================================= //

/// Opciones del escaneo.
#[derive(Debug, Clone, PartialEq)]
pub struct ScanOptions {
    /// Perfiles a escanear (vacío = todos).
    pub levels: Vec<Level>,
    /// Entradas por lote.
    pub batch_size: usize,
    /// Pausa entre lotes (ms).
    pub pause_ms: u64,
    /// Profundidad máxima de recorrido.
    pub max_depth: usize,
    /// Tope de tamaño para hashear el contenido de un fichero.
    pub hash_max_bytes: u64,
    /// ¿Sólo enumerar (sin leer contenido)?
    pub dry_run: bool,
    /// `nice` a aplicar al proceso.
    pub nice: i32,
    /// Clase de `ionice`.
    pub ionice_class: u8,
    /// ¿Mostrar progreso?
    pub progress: bool,
    /// Pausa entre Paquetes (ms) en la fase de apps.
    pub package_pause_ms: u64,
}

impl Default for ScanOptions {
    fn default() -> Self {
        ScanOptions {
            levels: Vec::new(),
            batch_size: 64,
            pause_ms: 120,
            max_depth: 4,
            hash_max_bytes: 8 * 1024 * 1024,
            dry_run: false,
            nice: 10,
            ionice_class: 3,
            progress: true,
            package_pause_ms: 25,
        }
    }
}

/// Una entrada del inventario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InventoryEntry {
    /// Nivel (`P0`..`P5`).
    pub level: String,
    /// Regla de la matriz.
    pub rule: String,
    /// Ruta absoluta en el dispositivo.
    pub path: String,
    /// Ruta dentro del contenedor.
    pub rel_path: String,
    /// Tamaño en bytes.
    pub size: u64,
    /// mtime (epoch).
    pub mtime: u64,
    /// ¿Es directorio?
    pub is_dir: bool,
    /// ¿Es sensible (clave aparte)?
    pub sensitive: bool,
    /// `sha16` del contenido (sólo si se hasheó).
    pub sha16: Option<String>,
    /// Paquete al que pertenece (si aplica).
    pub package: Option<String>,
}

/// Informe de una categoría.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategoryReport {
    /// Nivel.
    pub level: String,
    /// Perfil.
    pub profile: String,
    /// Reglas que overtlyieron algo.
    pub rules: Vec<String>,
    /// ¿Se encontró algo?
    pub exists: bool,
    /// Número de ficheros.
    pub files: usize,
    /// Número de directorios.
    pub dirs: usize,
    /// Bytes totales.
    pub total_bytes: u64,
    /// `sha16` del manifiesto de la categoría.
    pub manifest_sha16: String,
    /// Detalle legible.
    pub detail: String,
}

/// Informe completo del escaneo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InventoryReport {
    /// Versión del formato.
    pub version: u32,
    /// Cuándo se hizo (epoch).
    pub created_at: u64,
    /// Cuánto tardó (ms).
    pub duration_ms: u64,
    /// `loadavg` antes.
    pub loadavg_before: String,
    /// `loadavg` después.
    pub loadavg_after: String,
    /// Lotes ejecutados.
    pub batches: usize,
    /// Pausas aplicadas (ms en total).
    pub pause_total_ms: u64,
    /// Notas de los límites de I/O.
    pub io_limits: String,
    /// ¿Fue sólo un simulacro?
    pub dry_run: bool,
    /// Informe por categoría.
    pub categories: Vec<CategoryReport>,
    /// Todas las entradas.
    pub entries: Vec<InventoryEntry>,
    /// Paquetes detectados.
    pub packages_total: usize,
    /// Paquetes con datos privados.
    pub packages_with_data: usize,
}

impl InventoryReport {
    /// Informe en JSON.
    pub fn to_json(&self) -> Value {
        json!({
            "version": self.version,
            "created_at": self.created_at,
            "duration_ms": self.duration_ms,
            "loadavg_before": self.loadavg_before,
            "loadavg_after": self.loadavg_after,
            "batches": self.batches,
            "pause_total_ms": self.pause_total_ms,
            "io_limits": self.io_limits,
            "dry_run": self.dry_run,
            "packages_total": self.packages_total,
            "packages_with_data": self.packages_with_data,
            "categories": self.categories,
        })
    }

    /// Resumen legible (una línea por categoría).
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "INVENTARIO EXHAUSTIVO  v{}  ({} ms, {} lotes, {} ms de pausa)\n",
            self.version,
            self.duration_ms,
            self.batches,
            self.pause_total_ms
        ));
        s.push_str(&format!(
            "  loadavg antes={}  después={}\n",
            self.loadavg_before, self.loadavg_after
        ));
        s.push_str(&format!("  límites de I/O: {}\n", self.io_limits));
        s.push_str(&format!(
            "  paquetes detectados={}  con datos={}\n",
            self.packages_total, self.packages_with_data
        ));
        s.push_str("  ------------------------------------------------------------------\n");
        for c in &self.categories {
            s.push_str(&format!(
                "  {} {:<16} {:>5} ficheros {:>6} dirs {:>13} bytes  sha16={}  {}\n",
                c.level,
                c.profile,
                c.files,
                c.dirs,
                c.total_bytes,
                c.manifest_sha16,
                if c.exists { "" } else { "(no existe)" }
            ));
        }
        s.push_str("  ------------------------------------------------------------------\n");
        s.push_str("  (de los secretos sólo sale su sha16 y su tamaño: nunca el valor)\n");
        s
    }
}

/// `sha16` de un texto (primeros 16 hex del SHA-256).
pub fn sha16(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    let out = h.finalize();
    let mut s = String::with_capacity(16);
    for b in out.iter().take(8) {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Manifiesto canónico de una categoría: `ruta\ttamaño\tmtime\n`, ordenado.
///
/// **No contiene valores**: sólo metadatos.
pub fn canonical_manifest(entries: &[InventoryEntry]) -> String {
    let mut sorted: Vec<&InventoryEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let mut s = String::new();
    for e in sorted {
        s.push_str(&format!("{}\t{}\t{}\n", e.path, e.size, e.mtime));
    }
    s
}

/// `sha16` del manifiesto canónico.
pub fn manifest_sha16(entries: &[InventoryEntry]) -> String {
    sha16(canonical_manifest(entries).as_bytes())
}

/// Contexto del escaneo (permite pausar y mostrar progreso).
pub trait ScanObserver {
    /// Se Called al empezar un lote.
    fn on_batch(&mut self, batch: usize, total: usize, done: usize, level: &str);
    /// Se llama al terminar.
    fn on_done(&mut self);
}

/// Observador mudo (no imprime nada).
pub struct SilentObserver;

impl ScanObserver for SilentObserver {
    fn on_batch(&mut self, _b: usize, _t: usize, _d: usize, _l: &str) {}
    fn on_done(&mut self) {}
}

/// Observador que escribe el progreso por `stderr`.
pub struct ProgressObserver;

impl ScanObserver for ProgressObserver {
    fn on_batch(&mut self, batch: usize, total: usize, done: usize, level: &str) {
        eprintln!(
            "  [lote {}/{}] {}/{} entradas  ({})",
            batch, total, done, total * 0, level
        );
    }
    fn on_done(&mut self) {}
}

/// Escanea el dispositivo según la matriz.
///
/// Devuelve el informe completo. **Nunca** escribe valores de secretos: sólo
/// `sha16` y tamaños.
pub fn scan(dev: &dyn Device, opts: &ScanOptions, obs: &mut dyn ScanObserver) -> InventoryReport {
    let t0 = Instant::now();
    let load_before = dev.loadavg();
    let io_limits = apply_io_limits(opts.nice, opts.ionice_class);

    let levels: Vec<Level> = if opts.levels.is_empty() {
        Level::all().to_vec()
    } else {
        opts.levels.clone()
    };

    let mut entries: Vec<InventoryEntry> = Vec::new();
    let mut batches = 0usize;
    let mut pause_total = 0u64;
    // Paquetes vistos (para el resumen): se rellena en la fase P2.
    let mut packages_seen: Vec<PackageInfo> = Vec::new();

    for level in &levels {
        // 1) Reglas fijas de la matriz.
        let rules: Vec<Rule> = matrix()
            .into_iter()
            .filter(|r| r.level == *level)
            .collect();
        for rule in rules {
            for path in rule.paths {
                collect_path(dev, &rule, path, opts, &mut entries);
                batches += 1;
                if batches % opts.batch_size.max(1) == 0 {
                    obs.on_batch(batches, batches, entries.len(), level.id());
                    pause(opts.pause_ms);
                    pause_total += opts.pause_ms;
                }
            }
        }

        // 2) Reglas por paquete (P2).
        if *level == Level::P2 {
            let prules = package_rules();
            let mut pkgs = dev.packages();
            let total = pkgs.len();
            for (i, pkg) in pkgs.iter_mut().enumerate() {
                for prule in &prules {
                    if prule.data_subdir == "__apk__" {
                        // El APK no está en la matriz fija: se añade a mano.
                        for apk in dev.apk_paths(&pkg.package) {
                            add_single_file(
                                dev,
                                level,
                                prule.id,
                                &apk,
                                prule.sensitive,
                                opts,
                                &mut entries,
                            );
                        }
                    } else {
                        let base = format!("/data/user/0/{}", pkg.package);
                        collect_dir(
                            dev,
                            level,
                            prule.id,
                            &base,
                            prule.sensitive,
                            opts.max_depth,
                            opts,
                            &mut entries,
                            prule.exclude_cache,
                            &Some(pkg.package.clone()),
                        );
                    }
                }
                pkg.data_bytes = 0;
                batches += 1;
                if i % opts.batch_size.max(1) == 0 {
                    obs.on_batch(batches, total, i, level.id());
                    pause(opts.package_pause_ms);
                    pause_total += opts.package_pause_ms;
                }
            }
            packages_seen = pkgs;
        }
    }

    obs.on_done();

    // Informe por categoría.
    let mut categories = Vec::new();
    for level in &levels {
        let sub: Vec<InventoryEntry> = entries
            .iter()
            .filter(|e| e.level == level.id())
            .cloned()
            .collect();
        let files = sub.iter().filter(|e| !e.is_dir).count();
        let dirs = sub.iter().filter(|e| e.is_dir).count();
        let total_bytes: u64 = sub.iter().map(|e| e.size).sum();
        let mut rules: Vec<String> = sub.iter().map(|e| e.rule.clone()).collect();
        rules.sort();
        rules.dedup();
        categories.push(CategoryReport {
            level: level.id().to_string(),
            profile: level.profile().to_string(),
            rules,
            exists: !sub.is_empty(),
            files,
            dirs,
            total_bytes,
            manifest_sha16: manifest_sha16(&sub),
            detail: String::new(),
        });
    }

    let packages_total = packages_seen.len();
    let packages_with_data = {
        let mut set = std::collections::BTreeSet::new();
        for e in &entries {
            if let Some(p) = &e.package {
                if !p.is_empty() {
                    set.insert(p.clone());
                }
            }
        }
        set.len()
    };

    InventoryReport {
        version: INVENTORY_VERSION,
        created_at: now_epoch(),
        duration_ms: t0.elapsed().as_millis() as u64,
        loadavg_before: load_before,
        loadavg_after: dev.loadavg(),
        batches,
        pause_total_ms: pause_total,
        io_limits,
        dry_run: opts.dry_run,
        categories,
        entries,
        packages_total,
        packages_with_data,
    }
}

/// Pausa (con `sleep` del sistema).
fn pause(ms: u64) {
    if ms > 0 {
        std::thread::sleep(Duration::from_millis(ms));
    }
}

/// Añade un fichero suelto al inventario.
#[allow(clippy::too_many_arguments)]
fn add_single_file(
    dev: &dyn Device,
    level: &Level,
    rule: &str,
    path: &str,
    sensitive: bool,
    opts: &ScanOptions,
    out: &mut Vec<InventoryEntry>,
) {
    let meta = match dev.meta(path) {
        Some(m) => m,
        None => return,
    };
    let sha16 = hash_if_needed(dev, path, meta.size, opts, opts.hash_max_bytes);
    out.push(InventoryEntry {
        level: level.id().to_string(),
        rule: rule.to_string(),
        path: path.to_string(),
        rel_path: rel_path_for(path),
        size: meta.size,
        mtime: meta.mtime,
        is_dir: meta.is_dir,
        sensitive,
        sha16,
        package: package_of(path),
    });
}

/// Recoge una ruta (fichero o directorio) de la matriz.
fn collect_path(
    dev: &dyn Device,
    rule: &Rule,
    path: &str,
    opts: &ScanOptions,
    out: &mut Vec<InventoryEntry>,
) -> bool {
    let meta = match dev.meta(path) {
        Some(m) => m,
        None => return false,
    };
    if meta.is_dir && rule.walk {
        let (files, _bytes) = collect_dir(
            dev,
            &rule.level,
            rule.id,
            path,
            rule.sensitive,
            rule.max_depth,
            opts,
            out,
            true,
            &None,
        );
        return files > 0;
    }
    let max = match rule.hash {
        HashPolicy::None => 0,
        HashPolicy::Full => opts.hash_max_bytes.max(u64::MAX / 2),
        HashPolicy::Small(m) => m,
    };
    let sha16 = hash_if_needed(dev, path, meta.size, opts, max);
    out.push(InventoryEntry {
        level: rule.level.id().to_string(),
        rule: rule.id.to_string(),
        path: path.to_string(),
        rel_path: rel_path_for(path),
        size: meta.size,
        mtime: meta.mtime,
        is_dir: meta.is_dir,
        sensitive: rule.sensitive,
        sha16,
        package: package_of(path),
    });
    true
}

/// Recorre un directorio y añade sus ficheros.
#[allow(clippy::too_many_arguments)]
fn collect_dir(
    dev: &dyn Device,
    level: &Level,
    rule: &str,
    base: &str,
    sensitive: bool,
    max_depth: usize,
    opts: &ScanOptions,
    out: &mut Vec<InventoryEntry>,
    exclude_cache: bool,
    package: &Option<String>,
) -> (usize, u64) {
    let mut files = 0usize;
    let mut bytes = 0u64;
    let mut stack: Vec<(String, usize)> = vec![(base.to_string(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        if depth > max_depth {
            continue;
        }
        let items = dev.list_dir(&dir);
        for it in items {
            let full = format!("{}/{}", dir.trim_end_matches('/'), it.name);
            if exclude_cache && is_excluded(&full) {
                continue;
            }
            if it.is_dir {
                out.push(InventoryEntry {
                    level: level.id().to_string(),
                    rule: rule.to_string(),
                    path: full.clone(),
                    rel_path: rel_path_for(&full),
                    size: 0,
                    mtime: 0,
                    is_dir: true,
                    sensitive,
                    sha16: None,
                    package: package.clone(),
                });
                stack.push((full, depth + 1));
            } else {
                let meta = match dev.meta(&full) {
                    Some(m) => m,
                    None => continue,
                };
                let sha16 = hash_if_needed(dev, &full, meta.size, opts, opts.hash_max_bytes);
                bytes += meta.size;
                files += 1;
                out.push(InventoryEntry {
                    level: level.id().to_string(),
                    rule: rule.to_string(),
                    path: full.clone(),
                    rel_path: rel_path_for(&full),
                    size: meta.size,
                    mtime: meta.mtime,
                    is_dir: false,
                    sensitive,
                    sha16,
                    package: package.clone(),
                });
            }
        }
    }
    (files, bytes)
}

/// ¿La ruta cae en un patrón de exclusión (P5)?
pub fn is_excluded(path: &str) -> bool {
    EXCLUSION_PATTERNS.iter().any(|p| path.contains(p))
}

/// Hashea el contenido si toca (y si el fichero no es demasiado grande).
fn hash_if_needed(
    dev: &dyn Device,
    path: &str,
    size: u64,
    opts: &ScanOptions,
    max_bytes: u64,
) -> Option<String> {
    if opts.dry_run {
        return None;
    }
    if size > max_bytes {
        return None;
    }
    let data = dev.read_file(path)?;
    Some(sha16(&data))
}

/// Ruta dentro del contenedor (`data/...`).
pub fn rel_path_for(path: &str) -> String {
    format!("{}{}", INVENTORY_DATA_PREFIX, path.trim_start_matches('/'))
}

/// Paquete al que pertenece una ruta (`/data/user/0/<pkg>/...`).
pub fn package_of(path: &str) -> Option<String> {
    let p = path.trim_start_matches('/');
    if let Some(rest) = p.strip_prefix("data/user/0/") {
        let pkg = rest.split('/').next()?;
        if !pkg.is_empty() {
            return Some(pkg.to_string());
        }
    }
    if let Some(rest) = p.strip_prefix("data/user_de/0/") {
        let pkg = rest.split('/').next()?;
        if !pkg.is_empty() {
            return Some(pkg.to_string());
        }
    }
    None
}

/// Epoch actual.
pub fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ========================================================================= //
// Claves
// ========================================================================= //

/// El material de claves del contenedor.
///
/// **El valor de la clave NUNCA va al repo ni al chat**: se entrega por un
/// canal `0600` (fichero con permisos `0600`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyMaterial {
    /// Clave principal (AEGIS-256, 32 B) en hexadecimal.
    pub main_key: String,
    /// Clave sensible (AEGIS-256, 32 B) en hexadecimal.
    pub sensitive_key: String,
    /// KDF usado para la clave principal (`argon2id` o `raw`).
    pub main_kdf: String,
    /// `true` si la clave principal se derivó de una contraseña.
    pub main_from_passphrase: bool,
}

impl KeyMaterial {
    /// Bytes de la clave principal.
    pub fn main_bytes(&self) -> Result<Vec<u8>, String> {
        unhex(&self.main_key)
    }

    /// Bytes de la clave sensible.
    pub fn sensitive_bytes(&self) -> Result<Vec<u8>, String> {
        unhex(&self.sensitive_key)
    }

    /// Escribe el fichero de claves con permisos `0600`.
    pub fn write_secure(&self, path: &Path) -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt;
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("no se puede serializar el material de claves: {e}"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("no se puede crear {}: {e}", parent.display()))?;
        }
        std::fs::write(path, json.as_bytes())
            .map_err(|e| format!("no se puede escribir {}: {e}", path.display()))?;
        let mut perms = std::fs::metadata(path)
            .map_err(|e| format!("no se puede ver {}: {e}", path.display()))?
            .permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(path, perms)
            .map_err(|e| format!("no se puede poner 0600 a {}: {e}", path.display()))?;
        Ok(())
    }

    /// Lee el fichero de claves (y comprueba los permisos).
    pub fn read_secure(path: &Path) -> Result<KeyMaterial, String> {
        use std::os::unix::fs::PermissionsExt;
        let md = std::fs::metadata(path)
            .map_err(|e| format!("no se puede ver {}: {e}", path.display()))?;
        let mode = md.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            return Err(format!(
                "{} tiene permisos {mode:o}: el fichero de claves debe ser 0600",
                path.display()
            ));
        }
        let data = std::fs::read_to_string(path)
            .map_err(|e| format!("no se puede leer {}: {e}", path.display()))?;
        serde_json::from_str(&data).map_err(|e| format!("material de claves inválido: {e}"))
    }
}

/// Bytes aleatorios (CSPRNG del sistema).
pub fn random_bytes(n: usize) -> Result<Vec<u8>, String> {
    let mut out = vec![0u8; n];
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut out)
        .map_err(|_| "no se puede leer del CSPRNG del sistema".to_string())?;
    Ok(out)
}

// ========================================================================= //
// Exportación: contenedor MDBINV1
// ========================================================================= //

/// Opciones de exportación.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportOptions {
    /// Perfil a exportar (`P0`..`P5` o `all`).
    pub level: Level,
    /// Directorio de salida.
    pub out_dir: PathBuf,
    /// Contraseña (opcional): de ella se deriva la clave principal.
    pub passphrase: Option<String>,
    /// Dónde escribir el fichero de claves `0600`.
    pub key_out: Option<PathBuf>,
    /// Tamaño del trozo cifrado.
    pub chunk_size: usize,
    /// ¿Comprimir el tar con zstd?
    pub compress: bool,
    /// ¿Subir a Drive al terminar?
    pub upload: bool,
    /// Carpeta de destino en Drive.
    pub folder_id: String,
    /// Directorio de estado (para el token de Google).
    pub state_dir: Option<String>,
}

impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions {
            level: Level::P0,
            out_dir: PathBuf::from("/data/local/tmp/megadaemon/inventory"),
            passphrase: None,
            key_out: None,
            chunk_size: INVENTORY_CHUNK_SIZE,
            compress: true,
            upload: false,
            folder_id: String::new(),
            state_dir: None,
        }
    }
}

/// Resultado de la exportación.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportResult {
    /// Ruta del contenedor.
    pub container: String,
    /// Ruta del manifiesto suelto (JSON, sin valores).
    pub manifest_path: String,
    /// Ruta del fichero de claves `0600`.
    pub key_path: String,
    /// Bytes del contenedor.
    pub bytes: u64,
    /// `sha16` del manifiesto.
    pub manifest_sha16: String,
    /// Ficheros empaquetados.
    pub files: usize,
    /// Bytes de datos empaquetados.
    pub data_bytes: u64,
    /// Ficheros sensibles (clave aparte).
    pub sensitive_files: usize,
    /// Bytes sensibles.
    pub sensitive_bytes: u64,
    /// Identificador del fichero en Drive (si se subió).
    pub drive_file_id: Option<String>,
}

/// El manifiesto de un fichero dentro del contenedor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestFile {
    /// Ruta dentro del contenedor.
    pub path: String,
    /// Ruta original en el dispositivo.
    pub device_path: String,
    /// Tamaño.
    pub size: u64,
    /// mtime.
    pub mtime: u64,
    /// SHA-256 del contenido.
    pub sha256: String,
    /// Permisos.
    pub mode: u32,
    /// ¿Sensible?
    pub sensitive: bool,
    /// Paquete.
    pub package: Option<String>,
    /// Nivel (`P0`..`P5`).
    pub level: String,
}

/// El manifiesto completo (sin valores).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportManifest {
    /// Versión.
    pub version: u32,
    /// Perfil.
    pub profile: String,
    /// Creado (epoch).
    pub created_at: u64,
    /// Ficheros.
    pub files: Vec<ManifestFile>,
    /// `sha16` del propio manifiesto.
    pub sha16: String,
}

/// Descripción de un flujo cifrado dentro del contenedor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamDesc {
    /// Nombre (`payload` o `sensitive`).
    pub name: String,
    /// Algoritmo.
    pub cipher: String,
    /// Etiqueta del key-check.
    pub key_check: String,
    /// KDF.
    pub kdf: String,
    /// Número de trozos.
    pub chunks: u64,
    /// Bytes del flujo.
    pub bytes: u64,
    /// SHA-256 del flujo cifrado.
    pub sha256: String,
}

/// La cabecera del contenedor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InventoryHeader {
    /// Versión.
    pub version: u32,
    /// Perfil.
    pub profile: String,
    /// Creado (epoch).
    pub created_at: u64,
    /// `sha16` del manifiesto.
    pub manifest_sha16: String,
    /// Flujos.
    pub streams: Vec<StreamDesc>,
    /// Entrada donde va la clave sensible.
    pub key_entry: String,
}

/// Exporta un perfil a un contenedor `MDBINV1`.
pub fn export(
    dev: &dyn Device,
    opts: &ExportOptions,
    manifest_entries: &[InventoryEntry],
) -> Result<ExportResult, String> {
    std::fs::create_dir_all(&opts.out_dir)
        .map_err(|e| format!("no se puede crear {}: {e}", opts.out_dir.display()))?;

    // 1) Claves.
    let (main_key, main_kdf, main_from_pass) = match &opts.passphrase {
        Some(p) => {
            let k = KdfParams::default_params()
                .derive_key(p)
                .map_err(|e| format!("no se puede derivar la clave: {e}"))?;
            (k, "argon2id".to_string(), true)
        }
        None => (
            random_bytes(AEGIS_KEY_LEN)?,
            "raw".to_string(),
            false,
        ),
    };
    let sensitive_key = random_bytes(AEGIS_KEY_LEN)?;

    let keys = KeyMaterial {
        main_key: hex(&main_key),
        sensitive_key: hex(&sensitive_key),
        main_kdf: main_kdf.clone(),
        main_from_passphrase: main_from_pass,
    };
    let key_path = opts
        .key_out
        .clone()
        .unwrap_or_else(|| opts.out_dir.join(format!("{}.keys", opts.level.profile())));
    keys.write_secure(&key_path)?;

    // 2) Manifiesto (sha256 por fichero): hay que hashear el contenido *entero*
    //    de cada fichero que entra (aquí sí: el manifiesto del contenedor es la
    //    prueba de integridad del respaldo).
    let mut files: Vec<ManifestFile> = Vec::new();
    let mut normal: Vec<&InventoryEntry> = Vec::new();
    let mut sensitive: Vec<&InventoryEntry> = Vec::new();
    for e in manifest_entries {
        if e.is_dir {
            continue;
        }
        let data = dev
            .read_file(&e.path)
            .ok_or_else(|| format!("no se puede leer {}", e.path))?;
        let sha = sha256_hex(&data);
        files.push(ManifestFile {
            path: e.rel_path.clone(),
            device_path: e.path.clone(),
            size: e.size,
            mtime: e.mtime,
            sha256: sha,
            mode: 0o600,
            sensitive: e.sensitive,
            package: e.package.clone(),
            level: e.level.clone(),
        });
        if e.sensitive {
            sensitive.push(e);
        } else {
            normal.push(e);
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let manifest_sha16 = export_manifest_sha16(&files);
    // MEDIDO 2026-11-22: `ExportManifest { files, }` MUEVE el vector, asi que
    // los recuentos se hacen ANTES de construirlo.
    let sensitive_files = files.iter().filter(|f| f.sensitive).count();
    let sensitive_bytes: u64 = files.iter().filter(|f| f.sensitive).map(|f| f.size).sum();
    let data_bytes: u64 = files.iter().map(|f| f.size).sum();
    let files_count = files.len();
    let manifest = ExportManifest {
        version: INVENTORY_VERSION,
        profile: opts.level.profile().to_string(),
        created_at: now_epoch(),
        sha16: manifest_sha16.clone(),
        files,
    };
    let manifest_json = serde_json::to_string(&manifest)
        .map_err(|e| format!("no se puede serializar el manifiesto: {e}"))?;

    // 3) Flujo normal: manifiesto + clave sensible + ficheros normales.
    let normal_tar = build_tar(dev, opts, &normal, Some((&manifest_json, &keys)))?;
    let sensitive_tar = build_tar(dev, opts, &sensitive, None)?;

    // 4) Escribir el contenedor.
    let container_path = opts
        .out_dir
        .join(format!("{}.mdbinv", opts.level.profile()));
    let main_cipher = ChunkCipher::own(&main_key)
        .map_err(|e| format!("no se puede crear el cifrador principal: {e}"))?;
    let sens_cipher = ChunkCipher::own(&sensitive_key)
        .map_err(|e| format!("no se puede crear el cifrador sensible: {e}"))?;

    let mut file = File::create(&container_path)
        .map_err(|e| format!("no se puede crear {}: {e}", container_path.display()))?;
    file.write_all(INVENTORY_MAGIC)
        .map_err(|e| format!("error escribiendo el magic: {e}"))?;

    // Los flujos se escriben en un buffer temporal para poder poner sus
    // longitudes y hashes en la cabecera (que va **antes**).
    let mut streams_data: Vec<u8> = Vec::new();
    let mut descs: Vec<StreamDesc> = Vec::new();
    for (name, tar_bytes, cipher, key, kdf, label) in [
        (
            "payload",
            &normal_tar,
            &main_cipher,
            &main_key,
            // MEDIDO 2026-11-22: el array de flujos mezclaba `&String` (main_kdf)
            // y `&str` ("raw") en la misma posicion => E0308.  Ahora los dos
            // son &str.
            main_kdf.as_str(),
            LABEL_MAIN_KEY_CHECK,
        ),
        (
            "sensitive",
            &sensitive_tar,
            &sens_cipher,
            &sensitive_key,
            "raw",
            LABEL_SENSITIVE_KEY_CHECK,
        ),
    ] {
        let frames = seal_stream(cipher, name, tar_bytes, opts.chunk_size)?;
        let desc = StreamDesc {
            name: name.to_string(),
            cipher: "aegis256-chunk-v1".to_string(),
            key_check: hex(&key_check(key, label, 4)),
            kdf: kdf.to_string(),
            chunks: (frames.len() / (4 + 32 + opts.chunk_size + 32)) as u64,
            bytes: frames.len() as u64,
            sha256: sha256_hex(&frames),
        };
        descs.push(desc);
        streams_data.extend_from_slice(&frames);
    }

    let header = InventoryHeader {
        version: INVENTORY_VERSION,
        profile: opts.level.profile().to_string(),
        created_at: now_epoch(),
        // MEDIDO 2026-11-22: se clonaba porque el valor se usa DESPUES en el
        // struct de salida (use of moved value).
        manifest_sha16: manifest_sha16.clone(),
        streams: descs,
        key_entry: INVENTORY_KEY_ENTRY.to_string(),
    };
    let header_json = serde_json::to_string(&header)
        .map_err(|e| format!("no se puede serializar la cabecera: {e}"))?;
    if header_json.len() > INVENTORY_HEADER_SIZE - 1 {
        return Err("la cabecera no cabe en el tamaño fijo".to_string());
    }
    let mut header_buf = vec![b' '; INVENTORY_HEADER_SIZE];
    header_buf[..header_json.len()].copy_from_slice(header_json.as_bytes());
    file.write_all(&header_buf)
        .map_err(|e| format!("error escribiendo la cabecera: {e}"))?;
    file.write_all(&streams_data)
        .map_err(|e| format!("error escribiendo los flujos: {e}"))?;
    file.flush()
        .map_err(|e| format!("error volcando el contenedor: {e}"))?;
    let bytes = file
        .metadata()
        .map(|m| m.len())
        .map_err(|e| format!("no se puede ver el contenedor: {e}"))?;

    // 5) Manifiesto suelto (JSON, sin valores): es lo que se sube a Drive
    //    junto al contenedor y lo que `verify` usa como referencia.
    let manifest_path = opts
        .out_dir
        .join(format!("{}.manifest.json", opts.level.profile()));
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest)
            .map_err(|e| format!("no se puede serializar el manifiesto: {e}"))?,
    )
    .map_err(|e| format!("no se puede escribir {}: {e}", manifest_path.display()))?;



    let mut drive_file_id = None;
    if opts.upload {
        let target = crate::drive_client::DriveTarget::folder(&opts.folder_id);
        let up = crate::drive_native::upload_file(
            &container_path,
            &format!("{}.mdbinv", opts.level.profile()),
            &target,
            opts.state_dir.as_deref(),
        )
        .map_err(|e| format!("subida a Drive fallida: {e}"))?;
        drive_file_id = Some(up.file_id);
    }

    Ok(ExportResult {
        container: container_path.to_string_lossy().to_string(),
        manifest_path: manifest_path.to_string_lossy().to_string(),
        key_path: key_path.to_string_lossy().to_string(),
        bytes,
        manifest_sha16,
        files: files_count,
        data_bytes,
        sensitive_files,
        sensitive_bytes,
        drive_file_id,
    })
}

/// `sha16` del manifiesto de exportación: `ruta\ttamaño\tsha256\n`, ordenado.
///
/// No contiene valores: sólo metadatos y el hash (que ya es un resumen).
pub fn export_manifest_sha16(files: &[ManifestFile]) -> String {
    let mut sorted: Vec<&ManifestFile> = files.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let mut s = String::new();
    for f in sorted {
        s.push_str(&format!("{}\t{}\t{}\n", f.path, f.size, f.sha256));
    }
    sha16(s.as_bytes())
}

/// Construye el tar de un conjunto de entradas.
fn build_tar(
    dev: &dyn Device,
    opts: &ExportOptions,
    entries: &[&InventoryEntry],
    extra: Option<(&str, &KeyMaterial)>,
) -> Result<Vec<u8>, String> {
    let mut tar: Vec<u8> = Vec::new();
    if let Some((manifest_json, keys)) = extra {
        write_tar_file(&mut tar, INVENTORY_MANIFEST_ENTRY, manifest_json.as_bytes())?;
        let key_json = serde_json::to_string(keys)
            .map_err(|e| format!("no se puede serializar el material de claves: {e}"))?;
        write_tar_file(&mut tar, INVENTORY_KEY_ENTRY, key_json.as_bytes())?;
    }
    for e in entries {
        let data = dev
            .read_file(&e.path)
            .ok_or_else(|| format!("no se puede leer {}", e.path))?;
        write_tar_file(&mut tar, &e.rel_path, &data)?;
    }
    if opts.compress {
        zstd::stream::encode_all(&tar[..], 3).map_err(|e| format!("zstd: {e}"))
    } else {
        Ok(tar)
    }
}

/// Escribe una entrada de tar (cabecera + contenido + relleno).
fn write_tar_file(w: &mut Vec<u8>, name: &str, data: &[u8]) -> Result<(), String> {
    w.extend_from_slice(&tar_header(name, data.len() as u64, 0o600, 0));
    w.extend_from_slice(data);
    let pad = (512 - (data.len() % 512)) % 512;
    if pad > 0 {
        w.extend_from_slice(&vec![0u8; pad]);
    }
    Ok(())
}

/// Cifra un flujo en trozos AEGIS-256.
fn seal_stream(
    cipher: &ChunkCipher,
    name: &str,
    data: &[u8],
    chunk_size: usize,
) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let cs = chunk_size.max(1024);
    let mut index = 0u64;
    for piece in data.chunks(cs) {
        let frame = cipher.seal_chunk(name, index, piece);
        let len = (frame.len() - 32 - 32) as u32;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&frame);
        index += 1;
    }
    Ok(out)
}

// ========================================================================= //
// Verificación
// ========================================================================= //

/// Informe de verificación.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerifyReport {
    /// ¿Todo cuadra?
    pub ok: bool,
    /// Perfil.
    pub profile: String,
    /// Flujos verificados.
    pub streams: usize,
    /// Ficheros re-hasheados.
    pub files_checked: usize,
    /// Ficheros que no cuadran.
    pub files_bad: Vec<String>,
    /// Bytes leídos.
    pub bytes: u64,
    /// `sha16` del manifiesto.
    pub manifest_sha16: String,
    /// Detalle legible.
    pub detail: String,
}

/// Verifica un contenedor: descifra, re-hashea y compara con el manifiesto.
pub fn verify_container(path: &Path, keys: &KeyMaterial) -> Result<VerifyReport, String> {
    let data = std::fs::read(path).map_err(|e| format!("no se puede leer {}: {e}", path.display()))?;
    if data.len() < INVENTORY_MAGIC.len() + INVENTORY_HEADER_SIZE {
        return Err("el contenedor está truncado".to_string());
    }
    if &data[..INVENTORY_MAGIC.len()] != INVENTORY_MAGIC {
        return Err("no es un contenedor MDBINV1".to_string());
    }
    let hstart = INVENTORY_MAGIC.len();
    let hend = hstart + INVENTORY_HEADER_SIZE;
    let header_json = String::from_utf8_lossy(&data[hstart..hend])
        .trim_end()
        .to_string();
    let header: InventoryHeader = serde_json::from_str(&header_json)
        .map_err(|e| format!("cabecera inválida: {e}"))?;

    let mut offset = hend;
    let mut files_checked = 0usize;
    let mut files_bad: Vec<String> = Vec::new();
    let mut bytes = 0u64;
    // Hash recalculado por cada entrada del tar: `ruta -> sha256`.
    let mut rehashed: BTreeMap<String, String> = BTreeMap::new();

    for desc in &header.streams {
        let len = desc.bytes as usize;
        let end = offset + len;
        if end > data.len() {
            return Err(format!("el flujo '{}' está truncado", desc.name));
        }
        let frames = &data[offset..end];
        offset = end;
        bytes += len as u64;
        let key = match desc.name.as_str() {
            "payload" => keys.main_bytes()?,
            "sensitive" => keys.sensitive_bytes()?,
            other => return Err(format!("flujo desconocido: {other}")),
        };
        // El key-check decide **sin descifrar**.
        let expected = unhex(&desc.key_check).unwrap_or_default();
        let label = if desc.name == "payload" {
            LABEL_MAIN_KEY_CHECK
        } else {
            LABEL_SENSITIVE_KEY_CHECK
        };
        if !crate::backup_crypto::key_check_eq(&key_check(&key, label, 4), &expected) {
            return Err(format!(
                "la clave no cuadra con el key-check del flujo '{}'",
                desc.name
            ));
        }
        if sha256_hex(frames) != desc.sha256 {
            files_bad.push(format!("flujo:{}", desc.name));
            continue;
        }
        let cipher = ChunkCipher::own(&key)?;
        let plain = open_stream(&cipher, &desc.name, frames)?;
        let tar = maybe_decompress(&plain)?;
        // Recorrer el tar y re-hashear cada entrada.
        let mut cursor = std::io::Cursor::new(&tar[..]);
        let mut stream = TarStream::new(&mut cursor);
        while let Some(entry) = stream.next_entry().map_err(|e| format!("tar: {e}"))? {
            if entry.name == INVENTORY_MANIFEST_ENTRY || entry.name == INVENTORY_KEY_ENTRY {
                let mut sink = std::io::sink();
                stream.copy_data(&mut sink).map_err(|e| format!("tar: {e}"))?;
                continue;
            }
            let mut h = Sha256::new();
            let mut written = 0u64;
            let mut sink = HashSink {
                hash: &mut h,
                written: &mut written,
            };
            stream.copy_data(&mut sink).map_err(|e| format!("tar: {e}"))?;
            files_checked += 1;
            if written != entry.size {
                files_bad.push(format!("{}: tamaño del marco no cuadra", entry.name));
            }
            let mut hexs = String::with_capacity(64);
            for b in h.finalize().iter() {
                hexs.push_str(&format!("{:02x}", b));
            }
            rehashed.insert(entry.name.clone(), hexs);
        }
    }

    // 2) El manifiesto (del flujo `payload`) manda: se compara fichero a fichero.
    let manifest = read_manifest_from_container(path, keys)?;
    for f in &manifest.files {
        match rehashed.get(&f.path) {
            Some(h) if h.eq_ignore_ascii_case(&f.sha256) => {}
            Some(_) => files_bad.push(f.path.clone()),
            None => files_bad.push(format!("{}: no está en el contenedor", f.path)),
        }
    }
    if !manifest.sha16.is_empty()
        && !manifest
            .sha16
            .eq_ignore_ascii_case(&header.manifest_sha16)
    {
        files_bad.push("manifiesto: sha16 no cuadra con la cabecera".to_string());
    }

    let ok = files_bad.is_empty();
    // MEDIDO 2026-11-22: `files_bad` se mueve al struct de salida y DESPUES se
    // usaba `.len()` => borrow of moved value.  Se cuenta antes.
    let files_bad_count = files_bad.len();
    // MEDIDO 2026-11-22: el detalle del informe necesita los NOMBRES de los
    // fallos, pero `files_bad` se mueve al struct => se copia el resumen
    // ANTES del move.
    let files_bad_preview = files_bad
        .iter()
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    Ok(VerifyReport {
        ok,
        profile: header.profile.clone(),
        streams: header.streams.len(),
        files_checked,
        files_bad,
        bytes,
        manifest_sha16: header.manifest_sha16.clone(),
        detail: if ok {
            format!(
                "{} ficheros verificados en {} flujos: hashes y AEAD cuadran",
                files_checked,
                header.streams.len()
            )
        } else {
            format!("{files_bad_count} entradas no cuadran: {files_bad_preview}")
        },
    })
}

/// Descomprime el tar si lleva la cabecera de zstd.
fn maybe_decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() > 2 && data[0] == 0x28 && data[1] == 0xb5 {
        zstd::stream::decode_all(data).map_err(|e| format!("zstd: {e}"))
    } else {
        Ok(data.to_vec())
    }
}

/// Receptor que sólo calcula el hash.
struct HashSink<'a> {
    hash: &'a mut Sha256,
    written: &'a mut u64,
}

impl Write for HashSink<'_> {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.hash.update(data);
        *self.written += data.len() as u64;
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Descifra un flujo (marcos AEGIS-256).
fn open_stream(cipher: &ChunkCipher, name: &str, frames: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    let mut index = 0u64;
    while pos < frames.len() {
        if pos + 4 > frames.len() {
            return Err("marco truncado".to_string());
        }
        let len = u32::from_le_bytes([
            frames[pos],
            frames[pos + 1],
            frames[pos + 2],
            frames[pos + 3],
        ]) as usize;
        let frame_len = 4 + 32 + len + 32;
        if pos + frame_len > frames.len() {
            return Err("marco truncado".to_string());
        }
        let frame = &frames[pos + 4..pos + frame_len];
        let plain = cipher
            .open_chunk(name, index, frame)
            .map_err(|e| format!("marco {index}: {e}"))?;
        out.extend_from_slice(&plain);
        pos += frame_len;
        index += 1;
    }
    Ok(out)
}

/// Extrae el manifiesto del contenedor (del flujo `payload`).
pub fn read_manifest_from_container(path: &Path, keys: &KeyMaterial) -> Result<ExportManifest, String> {
    let data = std::fs::read(path).map_err(|e| format!("no se puede leer {}: {e}", path.display()))?;
    let hstart = INVENTORY_MAGIC.len();
    let hend = hstart + INVENTORY_HEADER_SIZE;
    let header_json = String::from_utf8_lossy(&data[hstart..hend])
        .trim_end()
        .to_string();
    let header: InventoryHeader = serde_json::from_str(&header_json)
        .map_err(|e| format!("cabecera inválida: {e}"))?;
    let mut offset = hend;
    for desc in &header.streams {
        let end = offset + desc.bytes as usize;
        let frames = &data[offset..end];
        offset = end;
        if desc.name != "payload" {
            continue;
        }
        let key = keys.main_bytes()?;
        let cipher = ChunkCipher::own(&key)?;
        let plain = open_stream(&cipher, &desc.name, frames)?;
        let tar = maybe_decompress(&plain)?;
        let mut cursor = std::io::Cursor::new(&tar[..]);
        let mut stream = TarStream::new(&mut cursor);
        while let Some(entry) = stream.next_entry().map_err(|e| format!("tar: {e}"))? {
            if entry.name == INVENTORY_MANIFEST_ENTRY {
                let mut buf = Vec::with_capacity(entry.size as usize);
                stream
                    .copy_data(&mut buf)
                    .map_err(|e| format!("tar: {e}"))?;
                let m: ExportManifest = serde_json::from_slice(&buf)
                    .map_err(|e| format!("manifiesto inválido: {e}"))?;
                return Ok(m);
            }
            let mut sink = std::io::sink();
            stream.copy_data(&mut sink).map_err(|e| format!("tar: {e}"))?;
        }
    }
    Err("el contenedor no trae manifiesto".to_string())
}

// ========================================================================= //
// Restore
// ========================================================================= //

/// Qué se restaura.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RestoreSelection {
    /// Paquetes concretos.
    pub packages: Vec<String>,
    /// Niveles concretos (`P0`..`P5`).
    pub levels: Vec<String>,
}

impl RestoreSelection {
    /// ¿Esta entrada entra en la selección?
    pub fn matches(&self, f: &ManifestFile) -> bool {
        let level_ok = self.levels.is_empty()
            || self.levels.iter().any(|l| {
                let l = l.trim().to_ascii_uppercase();
                f.level == l || f.level.starts_with(&format!("{l}-"))
            });
        let pkg_ok = self.packages.is_empty()
            || f.package
                .as_ref()
                .map(|p| self.packages.iter().any(|q| q == p))
                .unwrap_or(false);
        level_ok && pkg_ok
    }
}

/// Opciones del restore.
#[derive(Debug, Clone, PartialEq)]
pub struct RestoreOptions {
    /// Destino.
    pub dest: PathBuf,
    /// Selección.
    pub selection: RestoreSelection,
    /// `--confirm` (obligatorio para escribir).
    pub confirm: bool,
    /// Sobrescribir.
    pub overwrite: bool,
    /// Re-hashear lo escrito.
    pub verify_after: bool,
}

impl Default for RestoreOptions {
    fn default() -> Self {
        RestoreOptions {
            dest: PathBuf::from("/data/local/tmp/megadaemon/restore"),
            selection: RestoreSelection::default(),
            confirm: false,
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
    /// Número de ficheros escritos (MEDIDO 2026-11-22: se cuenta antes de
    /// mover el vector, porque se usaba despues del move).
    pub written_count: usize,
    /// Ficheros saltados.
    pub skipped: Vec<String>,
    /// Ficheros que fallaron.
    pub failed: Vec<String>,
    /// Bytes escritos.
    pub bytes: u64,
    /// Detalle legible.
    pub detail: String,
}

/// Restaura desde un contenedor.
///
/// **Sin `--confirm` no se escribe nada**: el informe dice qué se escribiría.
pub fn restore(
    container: &Path,
    keys: &KeyMaterial,
    opts: &RestoreOptions,
) -> Result<RestoreReport, String> {
    let data = std::fs::read(container)
        .map_err(|e| format!("no se puede leer {}: {e}", container.display()))?;
    if &data[..INVENTORY_MAGIC.len().min(data.len())] != INVENTORY_MAGIC {
        return Err("no es un contenedor MDBINV1".to_string());
    }
    let hstart = INVENTORY_MAGIC.len();
    let hend = hstart + INVENTORY_HEADER_SIZE;
    let header_json = String::from_utf8_lossy(&data[hstart..hend])
        .trim_end()
        .to_string();
    let header: InventoryHeader = serde_json::from_str(&header_json)
        .map_err(|e| format!("cabecera inválida: {e}"))?;
    let manifest = read_manifest_from_container(container, keys)?;

    let dry_run = !opts.confirm;
    let mut written = Vec::new();
    let mut skipped = Vec::new();
    let mut failed = Vec::new();
    let mut bytes = 0u64;

    if !dry_run {
        std::fs::create_dir_all(&opts.dest)
            .map_err(|e| format!("no se puede crear {}: {e}", opts.dest.display()))?;
    }

    let mut offset = hend;
    for desc in &header.streams {
        let end = offset + desc.bytes as usize;
        if end > data.len() {
            return Err(format!("el flujo '{}' está truncado", desc.name));
        }
        let frames = &data[offset..end];
        offset = end;
        let key = match desc.name.as_str() {
            "payload" => keys.main_bytes()?,
            "sensitive" => keys.sensitive_bytes()?,
            other => return Err(format!("flujo desconocido: {other}")),
        };
        let cipher = ChunkCipher::own(&key)?;
        let plain = open_stream(&cipher, &desc.name, frames)?;
        let tar = maybe_decompress(&plain)?;
        let mut cursor = std::io::Cursor::new(&tar[..]);
        let mut stream = TarStream::new(&mut cursor);
        while let Some(entry) = stream.next_entry().map_err(|e| format!("tar: {e}"))? {
            if entry.name == INVENTORY_MANIFEST_ENTRY || entry.name == INVENTORY_KEY_ENTRY {
                let mut sink = std::io::sink();
                stream.copy_data(&mut sink).map_err(|e| format!("tar: {e}"))?;
                continue;
            }
            let mf = manifest.files.iter().find(|f| f.path == entry.name);
            let selected = mf.map(|m| opts.selection.matches(m)).unwrap_or(false);
            if !selected {
                let mut sink = std::io::sink();
                stream.copy_data(&mut sink).map_err(|e| format!("tar: {e}"))?;
                skipped.push(entry.name.clone());
                continue;
            }
            let rel = match safe_relative_path(&entry.name) {
                Ok(r) => r,
                Err(e) => {
                    failed.push(format!("{}: {e}", entry.name));
                    let mut sink = std::io::sink();
                    stream.copy_data(&mut sink).map_err(|e| format!("tar: {e}"))?;
                    continue;
                }
            };
            let target = opts.dest.join(&rel);
            if dry_run {
                let mut sink = std::io::sink();
                let n = stream.copy_data(&mut sink).map_err(|e| format!("tar: {e}"))?;
                bytes += n;
                written.push(rel.to_string_lossy().to_string());
                continue;
            }
            if target.exists() && !opts.overwrite {
                failed.push(format!("{}: ya existe (usa --overwrite)", entry.name));
                let mut sink = std::io::sink();
                stream.copy_data(&mut sink).map_err(|e| format!("tar: {e}"))?;
                continue;
            }
            if let Some(parent) = target.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let mut buf = Vec::with_capacity(entry.size as usize);
            stream
                .copy_data(&mut buf)
                .map_err(|e| format!("tar: {e}"))?;
            // Verificación sha256 **antes** de escribir.
            if let Some(m) = mf {
                if sha256_hex(&buf) != m.sha256 {
                    failed.push(format!("{}: sha256 no cuadra", entry.name));
                    continue;
                }
            }
            std::fs::write(&target, &buf)
                .map_err(|e| format!("no se puede escribir {}: {e}", target.display()))?;
            bytes += buf.len() as u64;
            written.push(rel.to_string_lossy().to_string());
        }
    }

    // MEDIDO 2026-11-22: `written` se mueve al struct; el numero se cuenta
    // ANTES (el campo `written_count` solo vive dentro del literal).
    let written_count = written.len();
    Ok(RestoreReport {
        dry_run,
        ok: failed.is_empty(),
        written_count,
        written,
        skipped,
        failed,
        bytes,
        detail: if dry_run {
            format!(
                "SIMULACRO: se escribirían {} ficheros ({} bytes); usa --confirm para escribir",
                written_count,
                bytes
            )
        } else {
            format!("{written_count} ficheros escritos ({bytes} bytes)")
        },
    })
}

// ========================================================================= //
// Consolidación
// ========================================================================= //

/// Qué se puede hacer con una familia de apps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Consolidation {
    /// Ya hay un equivalente **nativo Rust** en el repo.
    NativeRust,
    /// Se puede consolidar con trabajo de Rust (planificable).
    RustCandidate,
    /// Apps comerciales con licencia: **análisis y auditoría**, nunca
    /// cracking de licencias ni bypass de DRM.
    AnalysisOnly,
    /// Se queda.
    Keep,
}

/// Una familia de apps detectada.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Family {
    /// Clave de la familia.
    pub key: &'static str,
    /// Nombre legible.
    pub title: &'static str,
    /// Apps de la familia.
    pub packages: Vec<String>,
    /// Qué se puede hacer.
    pub consolidation: Consolidation,
    /// Equivalente nativo (si lo hay).
    pub native: Option<&'static str>,
    /// Nota.
    pub note: &'static str,
}

/// Tabla de familias y su equivalente nativo en Rust.
pub fn families() -> Vec<Family> {
    vec![
        Family {
            key: "inventario-apk",
            title: "Inventario / análisis de APKs",
            packages: Vec::new(),
            consolidation: Consolidation::NativeRust,
            native: Some("crate::apk_inventory + crate::apk_deobfuscator"),
            note: "El motor Rust ya lista, hashea y analiza APKs installed.",
        },
        Family {
            key: "dex-inspector",
            title: "Inspección de DEX / descompilador",
            packages: Vec::new(),
            consolidation: Consolidation::NativeRust,
            native: Some("crate::dex_decompile"),
            note: "Descompilación de DEX nativa, sin herramientas de terceros.",
        },
        Family {
            key: "binwalk-diE",
            title: "Carving / identificación de ficheros (binwalk, DiE)",
            packages: Vec::new(),
            consolidation: Consolidation::NativeRust,
            native: Some("crate::binwalk_die"),
            note: "DiE + Binwalk + Scalpel + Comtype Scanner, ya nativos.",
        },
        Family {
            key: "hex-editor",
            title: "Editor hexadecimal / visores de binarios",
            packages: Vec::new(),
            consolidation: Consolidation::NativeRust,
            native: Some("crate::tile_vram + crate::kaitai_imhex"),
            note: "Visor de tiles, Kaitai/ImHex y el formateador nativo.",
        },
        Family {
            key: "backup-phone",
            title: "Copias de seguridad del móvil",
            packages: Vec::new(),
            consolidation: Consolidation::NativeRust,
            native: Some("crate::backup_engine + crate::backup_inventory"),
            note: "Motor de respaldo nativo con contenedor verificado.",
        },
        Family {
            key: "device-inventory",
            title: "Inventario del dispositivo / info del sistema",
            packages: Vec::new(),
            consolidation: Consolidation::NativeRust,
            native: Some("crate::device_inventory"),
            note: "Props, particiones, RAM, SELinux, paquetes: todo nativo.",
        },
        Family {
            key: "pkg-manager",
            title: "Gestor de apps (listar, desinstalar, permisos)",
            packages: Vec::new(),
            consolidation: Consolidation::NativeRust,
            native: Some("crate::pkgmgr + MCP app_control"),
            note: "Gestión de paquetes nativa, sin root-hide ni tricks.",
        },
        Family {
            key: "cert-vault",
            title: "Certificados / keystore / firma",
            packages: Vec::new(),
            consolidation: Consolidation::NativeRust,
            native: Some("crate::cert_vault + crate::signing"),
            note: "Inventario de certificados y firma de APK nativos.",
        },
        Family {
            key: "messaging-sync",
            title: "Lectura/sincronización de mensajería",
            packages: Vec::new(),
            consolidation: Consolidation::RustCandidate,
            native: Some("crate::sync_chats + crate::sync_web"),
            note: "Ya hay sincronización nativa; las bases cifradas de la app siguen sin abrir.",
        },
        Family {
            key: "screen-recorder",
            title: "Grabación de pantalla / audio",
            packages: Vec::new(),
            consolidation: Consolidation::NativeRust,
            native: Some("crate::recordings + crate::call_recorder"),
            note: "Grabación nativa con bóveda de grabaciones.",
        },
        Family {
            key: "clipboard-dict",
            title: "Portapapeles / diccionario / notas",
            packages: Vec::new(),
            consolidation: Consolidation::RustCandidate,
            native: Some("crate::clipboard + crate::dict_sync"),
            note: "Portapapeles y diccionario ya son nativos; el resto, candidato.",
        },
        Family {
            key: "bookmarks",
            title: "Marcadores / notas",
            packages: Vec::new(),
            consolidation: Consolidation::RustCandidate,
            native: Some("crate::bookmarks"),
            note: "Marcadores nativos ya disponibles.",
        },
        Family {
            key: "adb-remote",
            title: "Control remoto ADB / escritorio",
            packages: Vec::new(),
            consolidation: Consolidation::NativeRust,
            native: Some("crate::adb_client + crate::kdeconnect"),
            note: "ADB y KDE Connect ya son nativos.",
        },
        Family {
            key: "browser-multiple",
            title: "Navegadores (Chrome + Edge + Samsung + Tor + ...)",
            packages: Vec::new(),
            consolidation: Consolidation::Keep,
            native: None,
            note: "Cada navegador tiene su perfil y sus cookies: consolidar aquí SIEMPRE pierde datos.",
        },
        Family {
            key: "licence-drm",
            title: "Apps con licencia/DRM (Lucky Patcher y familia)",
            packages: Vec::new(),
            consolidation: Consolidation::AnalysisOnly,
            native: Some("crate::apk_inventory (auditoría)"),
            note: "LÍMITE: aquí la línea es análisis y auditoría. NO cracking de licencias ni bypass de DRM.",
        },
        Family {
            key: "commercial",
            title: "Apps comerciales con licencia",
            packages: Vec::new(),
            consolidation: Consolidation::AnalysisOnly,
            native: None,
            note: "Se auditan (qué datos tienen, cuánto ocupan); no se sustituyen ni se parchean.",
        },
    ]
}

/// Palabras clave que identifican cada familia.
fn family_keywords(key: &str) -> &'static [&'static str] {
    match key {
        "inventario-apk" => &["apkextractor", "apkinfo", "apktool", "apkeditor", "apkparser"],
        "dex-inspector" => &["dex", "decompiler", "jadx", "baksmali", "smali"],
        "binwalk-diE" => &["binwalk", "carver", "comtype", "foremost", "scalpel"],
        "hex-editor" => &["hexeditor", "hexeditor", "bined", "tilevram", "imhex", "kaitai"],
        "backup-phone" => &["backup", "shabefbackup", "swiftbackup"],
        "device-inventory" => &["deviceinfo", "cpumonitor", "devinfo", "sysinfo"],
        "pkg-manager" => &["appmanager", "appbrain", "apptips", "packageinstaller", "debloat"],
        "cert-vault" => &["keymaster", "cert", "signer", "keystore"],
        "messaging-sync" => &["smsbackup", "smsbackup", "smsbackupplus"],
        "screen-recorder" => &["recorder", "screenrecorder", "soundrecorder"],
        "clipboard-dict" => &["clipboard", "dictionary", "aegis"],
        "bookmarks" => &["bookmark", "markor", "notes", "notepad"],
        "adb-remote" => &["chromeremotedesktop", "scrcpy", "splitscreen", "wifikeyboard"],
        "licence-drm" => &["luckypatcher", "lucky", "crack", "patcher", "drm"],
        _ => &[],
    }
}

/// Palabras clave que identifican una app **comercial con licencia**.
const COMMERCIAL_HINTS: &[&str] = &[
    "netflix", "spotify", "youtube.premium", "primevideo", "disney", "hbo", "twitch",
    "prime", "microsoft.office", "word", "excel", "powerpoint", "acrobat", "adobe",
    "duolingo", "strava", "fitbit", "garmin", "waze", "linkedin", "uber", "bolt",
    "canva", "picsart", "adobe", "mcafee", "kaspersky", "avast", "avg", "bitdefender",
    "malwarebytes", "nordvpn", "expressvpn", "surfshark", "protonvpn", "1password",
    "lastpass", "dashlane", "squarespace", "wix", "godaddy", "hootsuite", "buffer",
];

/// ¿La app es comercial con licencia?
pub fn is_commercial(package: &str) -> bool {
    let p = package.to_ascii_lowercase();
    COMMERCIAL_HINTS.iter().any(|h| p.contains(h))
}

/// Detecta la familia de un paquete.
/// MEDIDO 2026-11-22: la version anterior devolvia `Option<&'static Family>`
/// PERO `families()` construye el Vec en el HEAP en cada llamada => era
/// imposible devolver una referencia a ese temporal (E0515).  Ahora se
/// devuelve una COPIA (`Option<Family>`).
pub fn family_of(package: &str) -> Option<Family> {
    let p = package.to_ascii_lowercase();
    let all = families();
    for f in &all {
        for kw in family_keywords(f.key) {
            if p.contains(kw) {
                return Some(f.clone());
            }
        }
    }
    if is_commercial(package) {
        return all.iter().find(|f| f.key == "commercial").cloned();
    }
    None
}

/// Informe de consolidación.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConsolidationReport {
    /// Paquetes analizados.
    pub packages_analyzed: usize,
    /// Paquetes con familia detectada.
    pub packages_classified: usize,
    /// Familias detectadas.
    pub families: Vec<Family>,
    /// Apps de sistema sin datos (candidatas a "?unused?").
    pub system_without_data: Vec<String>,
    /// Duplicados dentro de una familia.
    pub duplicates: Vec<DuplicateGroup>,
    /// Primer corte propuesto (nombres de paquete; **no se ejecuta nada**).
    pub first_cut: Vec<FirstCutItem>,
}

/// Un grupo de apps del mismo propósito.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DuplicateGroup {
    /// Familia.
    pub family: String,
    /// Apps del grupo.
    pub packages: Vec<String>,
    /// Recomendación.
    pub recommendation: Consolidation,
    /// Motivo.
    pub reason: String,
}

/// Una acción del primer corte.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FirstCutItem {
    /// Paquete afectado.
    pub package: String,
    /// Qué se propone.
    pub action: String,
    /// Por qué.
    pub reason: String,
    /// Bytes de datos que ocupa.
    pub data_bytes: u64,
}

/// Construye el informe de consolidación.
pub fn consolidate(
    packages: &[PackageInfo],
    entries: &[InventoryEntry],
) -> ConsolidationReport {
    // Bytes por paquete (a partir del inventario).
    let mut bytes_by_pkg: BTreeMap<String, u64> = BTreeMap::new();
    for e in entries {
        if let Some(p) = &e.package {
            if !p.is_empty() && !e.is_dir {
                *bytes_by_pkg.entry(p.clone()).or_insert(0) += e.size;
            }
        }
    }

    let mut fams = families();
    let mut classified = 0usize;
    for p in packages {
        let lname = p.package.to_ascii_lowercase();
        // Las apps comerciales con licencia se auditan, no se tocan.
        if is_commercial(&p.package) {
            classified += 1;
            for f in fams.iter_mut() {
                if f.key == "commercial" {
                    f.packages.push(p.package.clone());
                }
            }
            continue;
        }
        let mut hit = false;
        for f in fams.iter_mut() {
            for kw in family_keywords(f.key) {
                if lname.contains(kw) {
                    f.packages.push(p.package.clone());
                    hit = true;
                    break;
                }
            }
        }
        if hit {
            classified += 1;
        }
    }
    fams.retain(|f| !f.packages.is_empty());

    // Duplicados: más de una app en la misma familia.
    let mut duplicates = Vec::new();
    for f in &fams {
        if f.packages.len() > 1 {
            duplicates.push(DuplicateGroup {
                family: f.title.to_string(),
                packages: f.packages.clone(),
                recommendation: f.consolidation,
                reason: f.note.to_string(),
            });
        }
    }

    // Apps del sistema sin datos privados: candidatas a limpiar.
    let mut system_without_data = Vec::new();
    for p in packages {
        if p.system && !p.disabled {
            let b = bytes_by_pkg.get(&p.package).copied().unwrap_or(0);
            if b == 0 {
                system_without_data.push(p.package.clone());
            }
        }
    }
    system_without_data.sort();

    // Primer corte: lo que se puede sustituir por el equivalente nativo Rust.
    let mut first_cut = Vec::new();
    for f in &fams {
        if !matches!(
            f.consolidation,
            Consolidation::NativeRust | Consolidation::RustCandidate
        ) {
            continue;
        }
        for pkg in &f.packages {
            first_cut.push(FirstCutItem {
                package: pkg.clone(),
                action: format!(
                    "sustituir por {} (o dejar de instalar)",
                    f.native.unwrap_or("equivalente nativo Rust")
                ),
                reason: f.note.to_string(),
                data_bytes: bytes_by_pkg.get(pkg).copied().unwrap_or(0),
            });
        }
    }

    ConsolidationReport {
        packages_analyzed: packages.len(),
        packages_classified: classified,
        families: fams,
        system_without_data,
        duplicates,
        first_cut,
    }
}

// ========================================================================= //
// Tests internos
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;

    /// Dispositivo de mentira sobre un directorio real (para tests).
    struct FakeDev {
        root: PathBuf,
    }

    impl FakeDev {
        fn new(root: &Path) -> FakeDev {
            FakeDev {
                root: root.to_path_buf(),
            }
        }
        fn full(&self, p: &str) -> PathBuf {
            self.root.join(p.trim_start_matches('/'))
        }
    }

    impl Device for FakeDev {
        fn exists(&self, path: &str) -> bool {
            self.full(path).symlink_metadata().is_ok()
        }
        fn meta(&self, path: &str) -> Option<FileMeta> {
            let md = std::fs::symlink_metadata(self.full(path)).ok()?;
            Some(FileMeta {
                path: path.to_string(),
                size: if md.is_dir() { 0 } else { md.len() },
                mtime: md
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
                is_dir: md.is_dir(),
            })
        }
        fn list_dir(&self, path: &str) -> Vec<DirItem> {
            let mut v = Vec::new();
            if let Ok(rd) = std::fs::read_dir(self.full(path)) {
                for e in rd.flatten() {
                    v.push(DirItem {
                        name: e.file_name().to_string_lossy().to_string(),
                        is_dir: e.file_type().map(|t| t.is_dir()).unwrap_or(false),
                    });
                }
            }
            v.sort_by(|a, b| a.name.cmp(&b.name));
            v
        }
        fn read_file(&self, path: &str) -> Option<Vec<u8>> {
            std::fs::read(self.full(path)).ok()
        }
        fn packages(&self) -> Vec<PackageInfo> {
            Vec::new()
        }
        fn apk_paths(&self, _p: &str) -> Vec<String> {
            Vec::new()
        }
        fn loadavg(&self) -> String {
            "0.00 0.00 0.00".to_string()
        }
    }

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mdinv_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn sha16_estable_y_sin_valores() {
        assert_eq!(sha16(b"hola"), sha16(b"hola"));
        assert_ne!(sha16(b"hola"), sha16(b"holb"));
        assert_eq!(sha16(b"hola").len(), 16);
    }

    #[test]
    fn manifiesto_no_contiene_valores() {
        let e = InventoryEntry {
            level: "P0".into(),
            rule: "p0.wifi".into(),
            path: "/data/misc/wifi/WifiConfigStore.xml".into(),
            rel_path: "data/data/misc/wifi/WifiConfigStore.xml".into(),
            size: 1234,
            mtime: 99,
            is_dir: false,
            sensitive: true,
            sha16: Some("0123456789abcdef".into()),
            package: None,
        };
        // MEDIDO 2026-11-22: `e` se movia al array temporal y luego se
        // usaba otra vez (use of moved value).  Se clona.
        let m = canonical_manifest(std::slice::from_ref(&e));
        assert!(!m.contains("clave"));
        assert!(m.contains("WifiConfigStore.xml"));
        assert_eq!(manifest_sha16(std::slice::from_ref(&e)).len(), 16);
    }

    #[test]
    fn exclusiones_p5() {
        assert!(is_excluded("/data/user/0/x/cache/y"));
        assert!(is_excluded("/data/user/0/x/code_cache/y"));
        assert!(is_excluded("/data/user/0/x/no_backup/y"));
        assert!(is_excluded("/sdcard/a.thumbnails"));
        assert!(!is_excluded("/data/user/0/x/files/y"));
    }

    #[test]
    fn paquete_de_ruta() {
        assert_eq!(
            package_of("/data/user/0/com.whatsapp/databases/x.db"),
            Some("com.whatsapp".to_string())
        );
        assert_eq!(package_of("/system/build.prop"), None);
    }

    #[test]
    fn escaneo_falso_no_toca_el_sistema() {
        let root = tmp("scan");
        std::fs::create_dir_all(root.join("data/misc/wifi")).unwrap();
        std::fs::write(root.join("data/misc/wifi/WifiConfigStore.xml"), b"ssid\n").unwrap();
        let dev = FakeDev::new(&root);
        let opts = ScanOptions {
            levels: vec![Level::P0],
            dry_run: true,
            ..ScanOptions::default()
        };
        let mut obs = SilentObserver;
        let rep = scan(&dev, &opts, &mut obs);
        assert!(rep.categories.len() == 1);
        assert!(rep.categories[0].files >= 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn clave_se_guarda_con_0600() {
        let root = tmp("keys");
        let k = KeyMaterial {
            main_key: "00".repeat(32),
            sensitive_key: "11".repeat(32),
            main_kdf: "raw".into(),
            main_from_passphrase: false,
        };
        let p = root.join("k.keys");
        k.write_secure(&p).unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let back = KeyMaterial::read_secure(&p).unwrap();
        assert_eq!(back, k);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn clave_ilegible_por_otros() {
        let root = tmp("keys2");
        let k = KeyMaterial {
            main_key: "00".repeat(32),
            sensitive_key: "11".repeat(32),
            main_kdf: "raw".into(),
            main_from_passphrase: false,
        };
        let p = root.join("k.keys");
        k.write_secure(&p).unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&p).unwrap().permissions();
            perms.set_mode(0o644);
            std::fs::set_permissions(&p, perms).unwrap();
        }
        assert!(KeyMaterial::read_secure(&p).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn export_verifica_y_restaura() {
        let root = tmp("exp");
        let dev = FakeDev::new(&root);
        // Un fichero "sensible" y uno normal.
        std::fs::create_dir_all(root.join("data/user/0/app/databases")).unwrap();
        std::fs::write(root.join("data/user/0/app/databases/x.db"), b"secreto").unwrap();
        std::fs::write(root.join("data/user/0/app/files.txt"), b"normal").unwrap();
        let entries = vec![
            InventoryEntry {
                level: "P0".into(),
                rule: "p0.test".into(),
                path: "/data/user/0/app/databases/x.db".into(),
                rel_path: "data/data/user/0/app/databases/x.db".into(),
                size: 7,
                mtime: 1,
                is_dir: false,
                sensitive: true,
                sha16: None,
                package: Some("app".into()),
            },
            InventoryEntry {
                level: "P0".into(),
                rule: "p0.test".into(),
                path: "/data/user/0/app/files.txt".into(),
                rel_path: "data/data/user/0/app/files.txt".into(),
                size: 6,
                mtime: 1,
                is_dir: false,
                sensitive: false,
                sha16: None,
                package: Some("app".into()),
            },
        ];
        let out = root.join("out");
        let opts = ExportOptions {
            level: Level::P0,
            out_dir: out.clone(),
            passphrase: Some("clave-de-prueba".into()),
            compress: true,
            ..ExportOptions::default()
        };
        let res = export(&dev, &opts, &entries).unwrap();
        assert_eq!(res.files, 2);
        assert_eq!(res.sensitive_files, 1);

        let keys = KeyMaterial::read_secure(Path::new(&res.key_path)).unwrap();
        let rep = verify_container(Path::new(&res.container), &keys).unwrap();
        assert!(rep.ok, "{}", rep.detail);

        // Restore SIN confirmar: no escribe.
        let dest = root.join("restore");
        let ropts = RestoreOptions {
            dest: dest.clone(),
            ..RestoreOptions::default()
        };
        let rr = restore(Path::new(&res.container), &keys, &ropts).unwrap();
        assert!(rr.dry_run);
        assert_eq!(rr.written.len(), 2);
        assert!(!dest.exists());

        // Restore CON confirmar: escribe y verifica.
        let ropts = RestoreOptions {
            dest: dest.clone(),
            confirm: true,
            ..RestoreOptions::default()
        };
        let rr = restore(Path::new(&res.container), &keys, &ropts).unwrap();
        assert!(!rr.dry_run);
        assert!(rr.ok, "{:?}", rr.failed);
        assert_eq!(rr.written.len(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn consolidacion_detecta_familias() {
        let pkgs = vec![
            PackageInfo {
                package: "com.example.apkextractor".into(),
                system: false,
                disabled: false,
                apk: None,
                data_bytes: 0,
            },
            PackageInfo {
                package: "com.example.hexeditor".into(),
                system: false,
                disabled: false,
                apk: None,
                data_bytes: 0,
            },
            PackageInfo {
                package: "com.netflix.mediaclient".into(),
                system: false,
                disabled: false,
                apk: None,
                data_bytes: 0,
            },
        ];
        let rep = consolidate(&pkgs, &[]);
        assert!(rep.packages_classified >= 3);
        let keys: Vec<&str> = rep.families.iter().map(|f| f.key).collect();
        assert!(keys.contains(&"inventario-apk"));
        assert!(keys.contains(&"commercial"));
        let commercial = rep.families.iter().find(|f| f.key == "commercial").unwrap();
        assert_eq!(commercial.consolidation, Consolidation::AnalysisOnly);
    }
}
