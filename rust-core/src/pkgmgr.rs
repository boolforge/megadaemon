//! MegaDaemon v13.4 — **Gestión integrada de paquetes** (`pkgmgr`).
//!
//! # El problema que resuelve
//!
//! El usuario se quejaba de que "el sistema integrado de package management /
//! repo / store etc con todos los estándares que hay que es un caos" y de que
//! las auto-actualizaciones no son eficientes. Y el razón es visible en el
//! código: hoy hay **cuatro formatos de manifiesto** que no se hablan entre sí
//! y **tres caminos de actualización** distintos:
//!
//! | Dónde vive | Formato | Qué es | ¿Trae SHA-256? |
//! |---|---|---|---|
//! | `.github/workflows/build.yml` → `dist/state/latest.json` | `{"version","short","assets":{…},"assets_versioned":{…}}` | estado de la release | **NO** (va en ficheros `.sha256` sueltos) |
//! | `rust-core/src/channel.rs` → `state/channel.json` | `{"quick","duckdns","canonical","ts"}` | estado del canal | NO |
//! | `rust-core/src/repo_updates.rs` | índice F-Droid/Obtainium | apps con `versionCode` | SÍ |
//! | `deploy/99-megadaemon.sh` + `deploy/install-phone.sh` | shell | actualización del binario | sí, pero **fuera del manifiesto** |
//!
//! Y el camino de instalación es **peligroso**: `install-phone.sh` y
//! `repo_updates::deploy_binary()` hacen `cp -f` **sobre el binario en
//! marcha** ⇒ `ETXTBSY` ("Text file busy") y, si el `cp` se corta a medias,
//! el móvil se queda **sin daemon** (por eso el supervisor tiene que
//! resurrectar el binario a mano).
//!
//! # La solución: UN esquema, UN flujo
//!
//! Este módulo define:
//!
//! * [`Manifest`] — el **único** formato de manifiesto. Vale igual para una
//!   release de GitHub, para el canal, para un repo git y para una URL
//!   directa; y para APKs, binarios, repos, fuentes y diccionarios.
//! * [`Fuente`] — las **cuatro** fuentes, con una API uniforme
//!   (`listar_disponibles`) y una única forma de convertirlas a `Manifest`.
//! * [`comprobar_actualizacion`] — decide si hay que actualizar **comparando
//!   el SHA-256**, no la versión: si el contenido es idéntico, no se descarga
//!   **ni un byte**.
//! * [`descargar_resumible`] — descarga **reanudable por rango HTTP**
//!   (`Range: bytes=N-`), verifica el SHA-256 **antes** de instalar y hace el
//!   cambio **atómico** (`rename`, nunca `cp`).
//! * [`instalar_con_rollback`] — guarda el anterior como `.prev` y devuelve un
//!   [`Rollback`] si algo falla.
//! * [`IndicePaquetes`] — la **vista unificada**: qué hay instalado (APKs,
//!   binarios, repos) × qué hay disponible × **qué está desfasado y por qué**.
//!
//! # Eficiencia (lo que el usuario pedía)
//!
//! * **Cero descargas** si el destino ya tiene ese SHA-256.
//! * **Reanudable**: una descarga cortada continúa con `Range: bytes=N-`.
//! * **Cursor por proveedor** (ETag / Last-Modified) ⇒ no se relee el
//!   manifiesto entero cuando el proveedor dice "sin cambios".
//! * **Cuarentena de fallos** ⇒ no se reintenta lo que ya se sabe que falla.
//! * **Atómico** ⇒ nunca se rompe el binario en marcha.
//!
//! Reglas: **Rust puro**, identificadores en inglés, comentarios en español,
//! sin dependencias nuevas (`serde`, `serde_json`, `sha2`, `ureq` ya están en
//! el `Cargo.toml`) y **sin secretos**: las credenciales, si las hubiera,
//! entrarían por el almacén de secretos, nunca por el manifiesto.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ========================================================================= //
// Constantes
// ========================================================================= //

/// Versión del **esquema** de manifiesto. Cambiar el formato ⇒ subir esto.
pub const SCHEMA_VERSION: u32 = 1;

/// Nombre canónico del fichero de manifiesto dentro de un repo git.
pub const MANIFEST_FILE: &str = "pkg-manifest.json";

/// Nombre alternativo aceptado (por compatibilidad con repos antiguos).
pub const MANIFEST_FILE_ALT: &str = "manifest.json";

/// Subdirectorio del `state_dir` donde vive el estado de `pkgmgr`.
pub const ESTADO_DIR: &str = "pkgmgr";

/// Fichero con los cursores (ETag / Last-Modified) por proveedor.
pub const CURSORES_FILE: &str = "cursors.json";

/// Fichero con el registro de fallos (cuarentena) por proveedor.
pub const FALLOS_FILE: &str = "fallos.json";

/// Fichero con la configuración de fuentes.
pub const FUENTES_FILE: &str = "fuentes.json";

/// Tope de bytes de un manifiesto descargado (32 MiB).
pub const MAX_MANIFEST_BYTES: u64 = 32 * 1024 * 1024;

/// Tope de bytes de un artefacto descargado (2 GiB).
pub const MAX_ARTIFACT_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// User-Agent del cliente.
pub const USER_AGENT: &str = "MegaDaemon-pkgmgr/13.4 (+https://github.com/boolforge/megadaemon)";

/// Tiempoouts por defecto (segundos).
pub const TIMEOUT_MANIFESTO_SECS: u64 = 20;
/// Timeout de la descarga de un artefacto (segundos): son ficheros grandes.
pub const TIMEOUT_DESCARGA_SECS: u64 = 600;

/// Backoff base de la cuarentena de fallos (segundos).
pub const BACKOFF_BASE_SECS: u64 = 300;
/// Backoff máximo de la cuarentena de fallos (segundos).
pub const BACKOFF_MAX_SECS: u64 = 6 * 3600;

// ========================================================================= //
// Tipos del esquema
// ========================================================================= //

/// Tipo de artefacto que puede viajar en un [`Manifest`].
///
/// **Un solo formato para todos los proveedores**: el `kind` describe *qué*
/// es el fichero, no *de dónde* viene (eso lo dice la [`Fuente`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoAsset {
    /// APK de Android.
    Apk,
    /// Binario ejecutable (el daemon, los auxiliares, etc.).
    Binary,
    /// Snapshot de un repositorio git.
    Repo,
    /// Fuente (tipografía).
    Font,
    /// Diccionario (huspell, diccionarios de palabras…).
    Dict,
}

impl TipoAsset {
    /// Nombre estable en minúsculas (el que va en el JSON y en la tabla).
    pub fn as_str(&self) -> &'static str {
        match self {
            TipoAsset::Apk => "apk",
            TipoAsset::Binary => "binary",
            TipoAsset::Repo => "repo",
            TipoAsset::Font => "font",
            TipoAsset::Dict => "dict",
        }
    }

    /// Interpreta un texto (`"apk"`, `"binary"`, …). Acepta también el plural.
    pub fn from_str(s: &str) -> Option<TipoAsset> {
        match s.trim().to_ascii_lowercase().as_str() {
            "apk" | "apks" | "android" => Some(TipoAsset::Apk),
            "binary" | "bin" | "binarios" | "exe" => Some(TipoAsset::Binary),
            "repo" | "repos" | "git" => Some(TipoAsset::Repo),
            "font" | "fonts" | "fuente" | "fuentes" => Some(TipoAsset::Font),
            "dict" | "dicts" | "diccionario" | "diccionarios" => Some(TipoAsset::Dict),
            _ => None,
        }
    }
}

impl std::fmt::Display for TipoAsset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Un artefacto dentro de un [`Manifest`].
///
/// Es la **misma** estructura para un APK de F-Droid, para el binario del
/// daemon publicado por el CI, para un repo git y para un diccionario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    /// Nombre estable del artefacto. Para un APK es el *package id*.
    ///
    /// Regla estricta: **sin separadores de ruta** (ni `/` ni `\`), sin `..`
    /// y sin caracteres de control.
    pub name: String,
    /// Qué es el artefacto.
    pub kind: TipoAsset,
    /// Sistema operativo destino (`android`, `linux`, `any`).
    pub os: String,
    /// Arquitectura destino (`arm64`, `x86_64`, `any`).
    pub arch: String,
    /// URL **https** de descarga.
    pub url: String,
    /// SHA-256 en hexadecimal (64 caracteres).
    pub sha256: String,
    /// Tamaño en bytes (> 0).
    pub size: u64,
    /// Firma o commit de referencia (opcional).
    ///
    /// * APK: huella del certificado que firma el paquete.
    /// * Repo: **commit** del repo (la identidad de un repo no es un
    ///   SHA-256 del fichero, es el commit).
    /// * Binario: firma de la release que lo produjo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

impl Asset {
    /// Constructor con los campos obligatorios.
    pub fn nuevo(
        name: &str,
        kind: TipoAsset,
        os: &str,
        arch: &str,
        url: &str,
        sha256: &str,
        size: u64,
    ) -> Asset {
        Asset {
            name: name.to_string(),
            kind,
            os: os.to_string(),
            arch: arch.to_string(),
            url: url.to_string(),
            sha256: sha256.to_string(),
            size,
            signature: None,
        }
    }

    /// Validación **estricta** de un artefacto.
    pub fn validar(&self) -> Result<(), String> {
        // --- nombre -------------------------------------------------------
        if self.name.trim().is_empty() {
            return Err("el asset no tiene nombre".to_string());
        }
        if self.name.len() > 128 {
            return Err(format!(
                "el nombre del asset es demasiado largo ({} > 128): {}",
                self.name.len(),
                self.name
            ));
        }
        if self.name.contains('/') || self.name.contains('\\') {
            return Err(format!(
                "el nombre del asset lleva separadores de ruta: {}",
                self.name
            ));
        }
        if self.name.contains("..") {
            return Err(format!(
                "el nombre del asset lleva '..' (podría escapar del directorio): {}",
                self.name
            ));
        }
        if self.name == "." {
            return Err("el nombre del asset no puede ser '.'".to_string());
        }
        if let Some(c) = self.name.chars().find(|c| c.is_control()) {
            return Err(format!(
                "el nombre del asset tiene un carácter de control (0x{:02x})",
                c as u32
            ));
        }

        // --- os / arch ----------------------------------------------------
        for (campo, valor) in [("os", &self.os), ("arch", &self.arch)] {
            if valor.trim().is_empty() {
                return Err(format!("el asset {} no declara '{campo}'", self.name));
            }
            if valor.len() > 16 {
                return Err(format!(
                    "el {campo} del asset {} es demasiado largo: {valor}",
                    self.name
                ));
            }
            if !valor
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
            {
                return Err(format!(
                    "el {campo} del asset {} sólo admite [a-z0-9_-]: {valor}",
                    self.name
                ));
            }
        }

        // --- url ----------------------------------------------------------
        if !self.url.starts_with("https://") {
            return Err(format!(
                "la url del asset {} no es https: {}",
                self.name, self.url
            ));
        }
        if self.url.len() <= "https://".len() {
            return Err(format!("la url del asset {} no tiene host", self.name));
        }
        if self
            .url
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(format!(
                "la url del asset {} tiene espacios o caracteres de control",
                self.name
            ));
        }

        // --- sha256 --------------------------------------------------------
        if self.sha256.len() != 64 || !self.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!(
                "el sha256 del asset {} no son 64 hexadecimales: {}",
                self.name, self.sha256
            ));
        }

        // --- tamaño --------------------------------------------------------
        if self.size == 0 {
            return Err(format!("el asset {} declara tamaño 0", self.name));
        }

        Ok(())
    }
}

/// El **manifiesto único**: el mismo formato para todos los proveedores.
///
/// ```json
/// {
///   "schema_version": 1,
///   "version": "13.4.0",
///   "released_at": "2026-11-21T00:00:00Z",
///   "assets": [
///     {
///       "name": "megadaemon-core",
///       "kind": "binary",
///       "os": "android",
///       "arch": "arm64",
///       "url": "https://example.org/megadaemon-core-android-arm64",
///       "sha256": "…64 hex…",
///       "size": 12345678
///     }
///   ]
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// Versión del **esquema** (no del contenido): hoy `1`.
    pub schema_version: u32,
    /// Versión del contenido, estilo semver (`13.4.0`, `1.2.3-rc1`).
    pub version: String,
    /// Fecha de publicación (ISO-8601, `2026-11-21T00:00:00Z`).
    pub released_at: String,
    /// Todos los artefactos publicados.
    pub assets: Vec<Asset>,
}

impl Manifest {
    /// Manifiesto vacío (para tests y para empezar a construir uno).
    pub fn vacio() -> Manifest {
        Manifest {
            schema_version: SCHEMA_VERSION,
            version: "0.0.0".to_string(),
            released_at: "1970-01-01T00:00:00Z".to_string(),
            assets: Vec::new(),
        }
    }

    /// Validación **estricta** del manifiesto completo.
    ///
    /// Reglas:
    /// * `schema_version` tiene que ser exactamente [`SCHEMA_VERSION`].
    /// * `version` tiene que ser semver-ish.
    /// * `released_at` tiene que parecer una fecha ISO-8601.
    /// * Tiene que haber **al menos un** asset (un manifiesto sin assets no
    ///   sirve para nada y suele ser un fichero truncado).
    /// * Cada asset cumple las reglas de [`Asset::validar`].
    pub fn validar(&self) -> Result<(), String> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(format!(
                "schema_version {} no soportada (esta versión entiende la {})",
                self.schema_version, SCHEMA_VERSION
            ));
        }
        if !version_valida(&self.version) {
            return Err(format!(
                "la versión '{}' no es semver (se espera MAJOR.MINOR[.PATCH][-pre][+build])",
                self.version
            ));
        }
        if !fecha_iso_valida(&self.released_at) {
            return Err(format!(
                "released_at '{}' no es una fecha ISO-8601 (AAAA-MM-DDThh:mm:ssZ)",
                self.released_at
            ));
        }
        if self.assets.is_empty() {
            return Err("el manifiesto no declara ningún asset".to_string());
        }
        for a in &self.assets {
            a.validar()?;
        }
        Ok(())
    }

    /// Parsea un manifiesto desde JSON **validándolo**.
    pub fn de_json(texto: &str) -> Result<Manifest, String> {
        let m: Manifest = serde_json::from_str(texto)
            .map_err(|e| format!("el manifiesto no es un JSON válido de este esquema: {e}"))?;
        m.validar()?;
        Ok(m)
    }

    /// Serializa el manifiesto a JSON (bonito, con salto de línea final).
    pub fn a_json_texto(&self) -> String {
        let mut t = serde_json::to_string_pretty(self).unwrap_or_default();
        t.push('\n');
        t
    }

    /// Ficha JSON del manifiesto.
    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    /// Busca el asset que corresponde a un artefacto instalado.
    ///
    /// `os`/`arch` pueden ser `"any"` para no filtrar. Entre los candidatos
    /// gana el que coincide exactamente con la plataforma; después, el
    /// "any"; después, el primero.
    pub fn asset_para(&self, kind: TipoAsset, name: &str, os: &str, arch: &str) -> Option<&Asset> {
        let candidatos: Vec<&Asset> = self
            .assets
            .iter()
            .filter(|a| a.kind == kind && a.name == name)
            .collect();
        if candidatos.is_empty() {
            return None;
        }
        let mut mejor: Option<(&Asset, u8)> = None;
        for c in candidatos {
            let p = prioridad_plataforma(c, os, arch);
            match mejor {
                Some((_, p0)) if p0 <= p => {}
                _ => mejor = Some((c, p)),
            }
        }
        mejor.map(|(a, _)| a)
    }

    /// Todos los assets de un tipo.
    pub fn assets_de(&self, kind: TipoAsset) -> Vec<&Asset> {
        self.assets.iter().filter(|a| a.kind == kind).collect()
    }
}

/// Prioridad de un asset respecto a la plataforma pedida (menor = mejor).
fn prioridad_plataforma(a: &Asset, os: &str, arch: &str) -> u8 {
    let coincide_os = a.os == os;
    let coincide_arch = a.arch == arch;
    if coincide_os && coincide_arch {
        0
    } else if a.os == "any" && a.arch == "any" {
        1
    } else if coincide_os || coincide_arch {
        2
    } else {
        3
    }
}

/// ¿Es una versión estilo semver?
///
/// Acepta un `v` inicial, de 1 a 3 componentes numéricos, un sufijo
/// `-pre` opcional y unos metadatos `+build` opcionales.
pub fn version_valida(v: &str) -> bool {
    let s = v.trim();
    if s.is_empty() {
        return false;
    }
    let s = s.strip_prefix('v').unwrap_or(s);
    // +build
    let s = match s.split_once('+') {
        Some((base, build)) => {
            if build.is_empty()
                || !build
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
            {
                return false;
            }
            base
        }
        None => s,
    };
    // -pre
    let s = match s.split_once('-') {
        Some((base, pre)) => {
            if pre.is_empty()
                || !pre
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
            {
                return false;
            }
            base
        }
        None => s,
    };
    let partes: Vec<&str> = s.split('.').collect();
    if partes.is_empty() || partes.len() > 3 {
        return false;
    }
    partes
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// ¿Parece una fecha ISO-8601 (`AAAA-MM-DDThh:mm:ssZ`)?
pub fn fecha_iso_valida(s: &str) -> bool {
    let s = s.trim();
    if s.len() < 20 {
        return false;
    }
    let b = s.as_bytes();
    let digitos = |r: std::ops::Range<usize>| r.into_iter().all(|i| b[i].is_ascii_digit());
    digitos(0..4)
        && b[4] == b'-'
        && digitos(5..7)
        && b[7] == b'-'
        && digitos(8..10)
        && (b[10] == b'T' || b[10] == b' ')
        && digitos(11..13)
        && b[13] == b':'
        && digitos(14..16)
        && b[16] == b':'
        && digitos(17..19)
}

// ========================================================================= //
// Estado local: qué hay instalado
// ========================================================================= //

/// Un artefacto **instalado** en esta instancia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Instalado {
    /// Qué es.
    pub kind: TipoAsset,
    /// Nombre estable (package id, nombre del binario, nombre del repo…).
    pub name: String,
    /// Versión instalada (o la rama, para un repo).
    pub version: String,
    /// SHA-256 del contenido instalado (vacío si no se pudo calcular).
    pub sha256: String,
    /// Tamaño en bytes (0 si no se sabe).
    pub size: u64,
    /// Ruta donde vive.
    pub path: String,
    /// Firma o commit (para un repo: el commit).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// De dónde salió el dato (`inventario`, `directorio`, `git`…).
    #[serde(default)]
    pub source: String,
}

impl Instalado {
    /// Constructor con los campos obligatorios.
    pub fn nuevo(kind: TipoAsset, name: &str, path: &str) -> Instalado {
        Instalado {
            kind,
            name: name.to_string(),
            version: String::new(),
            sha256: String::new(),
            size: 0,
            path: path.to_string(),
            signature: None,
            source: String::new(),
        }
    }
}

/// Qué hay que hacer con un artefacto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Accion {
    /// Hay una versión nueva (el contenido **no** es el mismo).
    Actualizar,
    /// Ya está al día: el SHA-256 coincide.
    YaAlDia,
    /// Está disponible pero no está instalado.
    NoInstalado,
    /// No se puede saber (sin hash local o sin fuente).
    Desconocido,
}

impl Accion {
    /// Nombre estable (el que va en la tabla y en el JSON).
    pub fn as_str(&self) -> &'static str {
        match self {
            Accion::Actualizar => "Actualizar",
            Accion::YaAlDia => "YaAlDia",
            Accion::NoInstalado => "NoInstalado",
            Accion::Desconocido => "Desconocido",
        }
    }
}

/// Por qué se ha decidido esa acción.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Motivo {
    /// El SHA-256 instalado es el mismo que el disponible.
    MismoSha,
    /// El SHA-256 instalado es distinto ⇒ hay que descargar.
    ShaDistinto,
    /// Sin hash local: la versión dice que hay algo nuevo.
    VersionDistinta,
    /// Sin hash local y la versión coincide: no se puede **probar** que sea
    /// lo mismo ⇒ no se descarga (pero tampoco se da por al día).
    SinHash,
    /// No hay fuente disponible para ese artefacto.
    SinFuente,
    /// Está disponible pero no instalado.
    NoInstalado,
}

impl Motivo {
    /// Nombre estable.
    pub fn as_str(&self) -> &'static str {
        match self {
            Motivo::MismoSha => "mismo_sha",
            Motivo::ShaDistinto => "sha_distinto",
            Motivo::VersionDistinta => "version_distinta",
            Motivo::SinHash => "sin_hash",
            Motivo::SinFuente => "sin_fuente",
            Motivo::NoInstalado => "no_instalado",
        }
    }
}

/// Una actualización pendiente (ya decidida: hay que actuar).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Actualizacion {
    /// El asset disponible que hay que instalar.
    pub asset: Asset,
    /// Por qué.
    pub motivo: Motivo,
    /// La acción (siempre [`Accion::Actualizar`] aquí).
    pub accion: Accion,
    /// Explicación legible.
    pub detalle: String,
    /// Cuántos bytes hay que descargar (0 si ya está en caché local).
    pub bytes_a_descargar: u64,
}

/// ¿Son el mismo commit?
///
/// El repo local suele tener el **sha corto** (`git log --oneline`) y el
/// manifiesto el **completo**: si uno es prefijo del otro, es el mismo
/// commit.
pub fn mismo_commit(a: &str, b: &str) -> bool {
    let a = a.trim().to_ascii_lowercase();
    let b = b.trim().to_ascii_lowercase();
    if a.is_empty() || b.is_empty() {
        return false;
    }
    a == b || a.starts_with(&b) || b.starts_with(&a)
}

/// Evalúa un par (instalado, disponible) ⇒ (acción, motivo).
///
/// **El SHA-256 manda**: si el contenido instalado es idéntico al disponible,
/// no hay nada que hacer aunque el número de versión sea distinto (un
/// re-build con el mismo contenido no es una actualización).
pub fn evaluar(
    instalado: Option<&Instalado>,
    asset: Option<&Asset>,
    version_manifiesto: &str,
) -> (Accion, Motivo) {
    match (instalado, asset) {
        (None, None) => (Accion::Desconocido, Motivo::SinFuente),
        (None, Some(_)) => (Accion::NoInstalado, Motivo::NoInstalado),
        (Some(_), None) => (Accion::Desconocido, Motivo::SinFuente),
        (Some(i), Some(a)) => {
            // Un repo se identifica por su **commit**, no por el hash de un
            // fichero: el commit va en el campo `signature`.
            if a.kind == TipoAsset::Repo {
                let ai = a.signature.clone().unwrap_or_default();
                let ii = i.signature.clone().unwrap_or_default();
                if !ai.is_empty() && !ii.is_empty() {
                    return if mismo_commit(&ai, &ii) {
                        (Accion::YaAlDia, Motivo::MismoSha)
                    } else {
                        (Accion::Actualizar, Motivo::ShaDistinto)
                    };
                }
                // Sin commit no se puede comparar (la versión de un repo es
                // una rama: no es comparable con la versión del manifiesto).
                return (Accion::Desconocido, Motivo::SinHash);
            }
            // Caso general: comparación por SHA-256.
            if !i.sha256.is_empty() && !a.sha256.is_empty() {
                return if i.sha256.eq_ignore_ascii_case(&a.sha256) {
                    (Accion::YaAlDia, Motivo::MismoSha)
                } else {
                    (Accion::Actualizar, Motivo::ShaDistinto)
                };
            }
            // Sin hash: sólo podemos mirar la versión.
            if !i.version.is_empty()
                && !version_manifiesto.is_empty()
                && i.version != version_manifiesto
            {
                return (Accion::Actualizar, Motivo::VersionDistinta);
            }
            (Accion::Desconocido, Motivo::SinHash)
        }
    }
}

/// Decide si hay que actualizar comparando el **SHA-256**.
///
/// Devuelve `None` ⇒ **no hay nada que hacer** (o no hay nada que descargar):
/// * el asset no existe en el manifiesto;
/// * el SHA-256 instalado **es** el del asset (contenido idéntico ⇒ cero
///   descarga, por muy distinto que sea el número de versión);
/// * no se puede verificar nada y la versión coincide.
pub fn comprobar_actualizacion(
    instalado: &Instalado,
    disponible: &Manifest,
) -> Option<Actualizacion> {
    let asset = disponible.asset_para(instalado.kind, &instalado.name, "any", "any")?;
    let (accion, motivo) = evaluar(Some(instalado), Some(asset), &disponible.version);
    if accion != Accion::Actualizar {
        return None;
    }
    let detalle = match motivo {
        Motivo::ShaDistinto => format!(
            "sha-256 distinto: instalado {}…, disponible {}…",
            corto(&instalado.sha256),
            corto(&asset.sha256)
        ),
        Motivo::VersionDistinta => format!(
            "versión distinta: instalada {}, disponible {}",
            instalado.version, disponible.version
        ),
        _ => "hay que actualizar".to_string(),
    };
    Some(Actualizacion {
        asset: asset.clone(),
        motivo,
        accion,
        detalle,
        bytes_a_descargar: asset.size,
    })
}

/// Primeros `n` caracteres de un hash (para las tablas).
fn corto(hash: &str) -> String {
    if hash.len() <= 12 {
        hash.to_string()
    } else {
        format!("{}…", &hash[..12])
    }
}

// ========================================================================= //
// Transporte HTTP (inyectable para poder testear sin red)
// ========================================================================= //

/// Respuesta HTTP de una petición de **texto** (manifiestos).
#[derive(Debug, Clone, Default)]
pub struct RespuestaHttp {
    /// Código de estado.
    pub status: u16,
    /// Cuerpo.
    pub cuerpo: Vec<u8>,
    /// `ETag` (para el cursor del proveedor).
    pub etag: Option<String>,
    /// `Last-Modified` (para el cursor del proveedor).
    pub last_modified: Option<String>,
    /// `Content-Length` declarado.
    pub content_length: Option<u64>,
}

impl RespuestaHttp {
    /// El cuerpo como texto (UTF-8 tolerante).
    pub fn texto(&self) -> String {
        String::from_utf8_lossy(&self.cuerpo).to_string()
    }

    /// `true` si el servidor dice "no ha cambiado".
    pub fn no_modificado(&self) -> bool {
        self.status == 304
    }
}

/// Descarga **en streaming**: el cuerpo llega como `Read`.
///
/// Es lo que permite bajar un binario de 200 MiB sin meterlo en memoria
/// (el móvil tiene la RAM justa) y calcular el SHA-256 en la misma pasada.
pub struct DescargaBinaria {
    /// Código de estado (`206` = resumed, `200` = desde cero).
    pub status: u16,
    /// `ETag` del recurso.
    pub etag: Option<String>,
    /// `Last-Modified` del recurso.
    pub last_modified: Option<String>,
    /// Longitud total del recurso (si el servidor la dice).
    pub content_length: Option<u64>,
    /// El cuerpo.
    pub reader: Box<dyn Read>,
}

/// Abstacción del transporte HTTP: permite **testear sin red**.
pub trait Transportador {
    /// Petición de texto (con tope de bytes).
    fn get_texto(&self, url: &str, cabeceras: &[(&str, &str)]) -> Result<RespuestaHttp, String>;

    /// Descarga en streaming (con `Range` opcional).
    fn get_binario(
        &self,
        url: &str,
        offset: u64,
        cabeceras: &[(&str, &str)],
    ) -> Result<DescargaBinaria, String>;
}

/// Transporte real sobre `ureq`.
pub struct TransporteUreq {
    /// Agente para textos (manifiestos): timeout corto.
    agente_texto: ureq::Agent,
    /// Agente para binarios: timeout largo (los APK son grandes).
    agente_binario: ureq::Agent,
}

impl TransporteUreq {
    /// Crea el transporte con los timeouts por defecto.
    pub fn nuevo() -> TransporteUreq {
        TransporteUreq::con_timeouts(
            Duration::from_secs(TIMEOUT_MANIFESTO_SECS),
            Duration::from_secs(TIMEOUT_DESCARGA_SECS),
        )
    }

    /// Crea el transporte con timeouts propios.
    pub fn con_timeouts(timeout_manifesto: Duration, timeout_descarga: Duration) -> TransporteUreq {
        // Dos agentes: los manifiestos son pequeños y no pueden esperar
        // minutos; los binarios son grandes y sí.
        let agente_texto = ureq::Agent::new_with_config(
            ureq::Agent::config_builder()
                .timeout_global(Some(timeout_manifesto))
                .timeout_connect(Some(Duration::from_secs(6)))
                // Los 4xx/5xx **no** son un error de ureq: los queremos ver
                // para poder distinguir un 404 de un 500.
                .http_status_as_error(false)
                .build(),
        );
        let agente_binario = ureq::Agent::new_with_config(
            ureq::Agent::config_builder()
                .timeout_global(Some(timeout_descarga))
                .timeout_connect(Some(Duration::from_secs(6)))
                .http_status_as_error(false)
                .build(),
        );
        TransporteUreq {
            agente_texto,
            agente_binario,
        }
    }

    fn cabeceras_por_defecto(extra: &[(&str, &str)]) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = vec![("User-Agent".to_string(), USER_AGENT.to_string())];
        for (k, val) in extra {
            v.push((k.to_string(), val.to_string()));
        }
        v
    }
}

impl Default for TransporteUreq {
    fn default() -> Self {
        TransporteUreq::nuevo()
    }
}

fn cabeceras_respuesta(
    headers: &ureq::http::HeaderMap,
) -> (Option<String>, Option<String>, Option<u64>) {
    let etag = headers
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let last_modified = headers
        .get("last-modified")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let content_length = headers
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok());
    (etag, last_modified, content_length)
}

impl Transportador for TransporteUreq {
    fn get_texto(&self, url: &str, cabeceras: &[(&str, &str)]) -> Result<RespuestaHttp, String> {
        let mut req = self.agente_texto.get(url);
        for (k, v) in TransporteUreq::cabeceras_por_defecto(cabeceras) {
            req = req.header(k.as_str(), v.as_str());
        }
        let resp = req
            .call()
            .map_err(|e| format!("fallo HTTP al pedir {url}: {e}"))?;
        let status = resp.status().as_u16();
        let (etag, last_modified, content_length) = cabeceras_respuesta(resp.headers());
        let mut body = resp.into_body();
        let cuerpo = body
            .with_config()
            .limit(MAX_MANIFEST_BYTES)
            .read_to_vec()
            .map_err(|e| format!("no se pudo leer el cuerpo de {url}: {e}"))?;
        Ok(RespuestaHttp {
            status,
            cuerpo,
            etag,
            last_modified,
            content_length,
        })
    }

    fn get_binario(
        &self,
        url: &str,
        _offset: u64,
        cabeceras: &[(&str, &str)],
    ) -> Result<DescargaBinaria, String> {
        let mut req = self.agente_binario.get(url);
        for (k, v) in TransporteUreq::cabeceras_por_defecto(cabeceras) {
            req = req.header(k.as_str(), v.as_str());
        }
        let resp = req
            .call()
            .map_err(|e| format!("fallo HTTP al descargar {url}: {e}"))?;
        let status = resp.status().as_u16();
        let (etag, last_modified, content_length) = cabeceras_respuesta(resp.headers());
        let reader = resp.into_body().into_reader();
        Ok(DescargaBinaria {
            status,
            etag,
            last_modified,
            content_length,
            reader: Box::new(reader),
        })
    }
}

// ========================================================================= //
// Fuentes: los cuatro proveedores, una sola API
// ========================================================================= //

/// Release de GitHub (el CI publica una por commit, con tag `v<sha>`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GitHubRelease {
    /// Dueño de la repo (`boolforge`).
    pub owner: String,
    /// Nombre de la repo (`megadaemon`).
    pub repo: String,
    /// Tag concreto o `"latest"`.
    pub tag: String,
}

impl GitHubRelease {
    /// URL de la API que devuelve la release.
    pub fn url_api(&self) -> String {
        if self.tag.is_empty() || self.tag == "latest" {
            format!(
                "https://api.github.com/repos/{}/{}/releases/latest",
                self.owner, self.repo
            )
        } else {
            format!(
                "https://api.github.com/repos/{}/{}/releases/tags/{}",
                self.owner, self.repo, self.tag
            )
        }
    }

    /// Convierte la respuesta de la API de GitHub en un [`Manifest`].
    ///
    /// La API da `name`, `browser_download_url`, `size` y (en las releases
    /// nuevas) `digest` (`"sha256:abcd…"`). Si no hay `digest`, se puede
    /// rellenar con el mapa `digests` (por ejemplo, los ficheros `.sha256`
    /// que publica el CI).
    pub fn manifest_desde_json(
        doc: &Value,
        digests: &BTreeMap<String, String>,
    ) -> Result<Manifest, String> {
        let tag = doc
            .get("tag_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let published = doc
            .get("published_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let lista = doc
            .get("assets")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "la release de GitHub no trae la lista 'assets'".to_string())?;

        let mut assets = Vec::new();
        for a in lista {
            let name = a
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if name.is_empty() {
                continue;
            }
            let url = a
                .get("browser_download_url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let size = a.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
            let mut sha = a
                .get("digest")
                .and_then(|v| v.as_str())
                .map(|d| d.trim_start_matches("sha256:").to_string())
                .unwrap_or_default();
            if sha.is_empty() {
                if let Some(d) = digests.get(&name) {
                    sha = d.trim().to_string();
                }
            }
            let (os, arch) = adivinar_plataforma(&name);
            assets.push(Asset {
                name: nombre_de_asset(&name),
                kind: adivinar_tipo(&name),
                os,
                arch,
                url,
                sha256: sha.to_ascii_lowercase(),
                size,
                signature: None,
            });
        }
        if assets.is_empty() {
            return Err("la release no tiene assets utilizables".to_string());
        }
        let version = if tag.is_empty() {
            "0.0.0".to_string()
        } else {
            normalizar_version_tag(&tag)
        };
        let m = Manifest {
            schema_version: SCHEMA_VERSION,
            version,
            released_at: if published.is_empty() {
                ahora_iso()
            } else {
                published
            },
            assets,
        };
        m.validar()?;
        Ok(m)
    }
}

/// Convierte un tag (`v1.2.3`, `v13.4.0-rc1`) en una versión semver válida.
pub fn normalizar_version_tag(tag: &str) -> String {
    let t = tag.trim();
    let t = t.strip_prefix('v').unwrap_or(t);
    if version_valida(t) {
        return t.to_string();
    }
    // Un tag que no es semver (p. ej. el sha de un commit) se mete como
    // metadatos de build: `0.0.0+g<sha>` ⇒ sigue siendo semver válido.
    let limpio: String = t
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!("0.0.0+{}", limpio.trim_matches('-'))
}

/// El `channel.json` publicado por el daemon (o el que contenga un manifiesto).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelJson {
    /// URL del `channel.json`.
    pub url: String,
}

impl ChannelJson {
    /// Extrae el manifiesto de un documento de canal.
    ///
    /// Acepta dos formas:
    /// * `{"manifest": {…}}` — el manifiesto completo anidado (lo preferred);
    /// * `{"assets": [...], "version": "...", "released_at": "..."}` — el
    ///   manifiesto "despieceado" en el propio canal.
    pub fn manifest_desde_doc(doc: &Value) -> Result<Manifest, String> {
        if let Some(m) = doc.get("manifest") {
            if m.is_object() {
                let parsed: Manifest = serde_json::from_value(m.clone())
                    .map_err(|e| format!("el 'manifest' del canal no es válido: {e}"))?;
                parsed.validar()?;
                return Ok(parsed);
            }
        }
        if let Some(a) = doc.get("assets").and_then(|v| v.as_array()) {
            let version = doc
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("0.0.0")
                .to_string();
            let released_at = doc
                .get("released_at")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let assets: Vec<Asset> = a
                .iter()
                .filter_map(|v| serde_json::from_value::<Asset>(v.clone()).ok())
                .collect();
            let m = Manifest {
                schema_version: SCHEMA_VERSION,
                version,
                released_at: if released_at.is_empty() {
                    ahora_iso()
                } else {
                    released_at
                },
                assets,
            };
            m.validar()?;
            return Ok(m);
        }
        Err("el channel.json no trae manifiesto (ni 'manifest' ni 'assets')".to_string())
    }
}

/// Una **fuente** de paquetes: los cuatro proveedores, una sola API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Fuente {
    /// Release de GitHub (lo que publica el CI).
    Release(GitHubRelease),
    /// `channel.json` del daemon.
    Canal(ChannelJson),
    /// Repositorio git (el manifiesto se lee del repo, en la rama indicada).
    RepoGit {
        /// URL del repo (`https://github.com/owner/repo`).
        url: String,
        /// Rama o ref.
        rama: String,
    },
    /// Un manifiesto JSON en una URL directa.
    Directo {
        /// URL del manifiesto.
        url: String,
    },
}

/// Listado de manifiestos **con los metadatos HTTP** de la respuesta.
///
/// El `ETag` / `Last-Modified` son lo que permite el **cursor**: sin ellos,
/// cada comprobación releería el manifiesto entero.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ListadoConMeta {
    /// Los manifiestos encontrados.
    pub manifiestos: Vec<Manifest>,
    /// `ETag` de la respuesta.
    pub etag: Option<String>,
    /// `Last-Modified` de la respuesta.
    pub last_modified: Option<String>,
    /// `true` si el servidor respondió `304 Not Modified` (no hay nada nuevo).
    pub not_modified: bool,
}

impl Fuente {
    /// Identificador estable del proveedor (sirve de clave del cursor y de la
    /// caché del manifiesto).
    pub fn id(&self) -> String {
        match self {
            Fuente::Release(r) => format!("release:{}/{}@{}", r.owner, r.repo, r.tag),
            Fuente::Canal(c) => format!("canal:{}", c.url),
            Fuente::RepoGit { url, rama } => format!("git:{}@{}", url, rama),
            Fuente::Directo { url } => format!("directo:{}", url),
        }
    }

    /// Descripción legible (para la UI).
    pub fn descripcion(&self) -> String {
        match self {
            Fuente::Release(r) => format!("release {}@{}", r.repo, r.tag),
            Fuente::Canal(c) => format!("canal {}", c.url),
            Fuente::RepoGit { url, rama } => format!("repo {} ({})", url, rama),
            Fuente::Directo { url } => format!("directo {}", url),
        }
    }

    /// Lista los manifiestos disponibles en esta fuente (API uniforme).
    ///
    /// Es la **misma** función para los cuatro proveedores: quien llama no
    /// necesita saber de dónde sale el manifiesto.
    pub fn listar_disponibles(&self) -> Result<Vec<Manifest>, String> {
        self.listar_disponibles_con(&TransporteUreq::nuevo())
    }

    /// Igual que [`Fuente::listar_disponibles`] pero con un transporte
    /// inyectado (para testear sin red).
    pub fn listar_disponibles_con<T: Transportador>(&self, t: &T) -> Result<Vec<Manifest>, String> {
        Ok(self.listar_con_meta(t, &[])?.manifiestos)
    }

    /// Igual que [`Fuente::listar_disponibles_con`] pero **con los metadatos
    /// HTTP** (ETag / Last-Modified / 304) ⇒ es lo que alimenta el cursor.
    pub fn listar_con_meta<T: Transportador>(
        &self,
        t: &T,
        cabeceras: &[(&str, &str)],
    ) -> Result<ListadoConMeta, String> {
        match self {
            Fuente::Release(r) => {
                let resp = t.get_texto(&r.url_api(), cabeceras)?;
                if resp.no_modificado() {
                    return Ok(ListadoConMeta {
                        not_modified: true,
                        ..Default::default()
                    });
                }
                if resp.status == 404 {
                    return Err(format!("no hay ninguna release en {}", r.url_api()));
                }
                if resp.status >= 400 {
                    return Err(format!(
                        "la API de GitHub devolvió {} al pedir {}",
                        resp.status,
                        r.url_api()
                    ));
                }
                let doc: Value = serde_json::from_str(&resp.texto())
                    .map_err(|e| format!("la release de GitHub no es JSON: {e}"))?;
                let digests = digests_de_release(&doc);
                let m = GitHubRelease::manifest_desde_json(&doc, &digests)?;
                Ok(ListadoConMeta {
                    manifiestos: vec![m],
                    etag: resp.etag.clone(),
                    last_modified: resp.last_modified.clone(),
                    not_modified: false,
                })
            }
            Fuente::Canal(c) => {
                let resp = t.get_texto(&c.url, cabeceras)?;
                if resp.no_modificado() {
                    return Ok(ListadoConMeta {
                        not_modified: true,
                        ..Default::default()
                    });
                }
                if resp.status >= 400 {
                    return Err(format!("el canal devolvió {} en {}", resp.status, c.url));
                }
                let doc: Value = serde_json::from_str(&resp.texto())
                    .map_err(|e| format!("el channel.json no es JSON: {e}"))?;
                let m = ChannelJson::manifest_desde_doc(&doc)?;
                Ok(ListadoConMeta {
                    manifiestos: vec![m],
                    etag: resp.etag.clone(),
                    last_modified: resp.last_modified.clone(),
                    not_modified: false,
                })
            }
            Fuente::RepoGit { url, rama } => {
                let base = url_raw_de_repo(url, rama)?;
                let mut out = Vec::new();
                let mut meta = ListadoConMeta::default();
                let mut ultimo_error = String::new();
                for nombre in [MANIFEST_FILE, MANIFEST_FILE_ALT] {
                    let objetivo = format!("{base}{nombre}");
                    let resp = match t.get_texto(&objetivo, cabeceras) {
                        Ok(r) => r,
                        Err(e) => {
                            ultimo_error = e;
                            continue;
                        }
                    };
                    if resp.no_modificado() {
                        return Ok(ListadoConMeta {
                            not_modified: true,
                            ..Default::default()
                        });
                    }
                    if resp.status == 404 {
                        ultimo_error = format!("{objetivo} no existe");
                        continue;
                    }
                    if resp.status >= 400 {
                        ultimo_error = format!("{objetivo} devolvió {}", resp.status);
                        continue;
                    }
                    let doc: Value = match serde_json::from_str(&resp.texto()) {
                        Ok(v) => v,
                        Err(e) => {
                            ultimo_error = format!("{objetivo} no es JSON: {e}");
                            continue;
                        }
                    };
                    out.extend(manifiestos_de_doc(&doc)?);
                    meta.etag = resp.etag.clone();
                    meta.last_modified = resp.last_modified.clone();
                    if !out.is_empty() {
                        break;
                    }
                }
                if out.is_empty() {
                    return Err(if ultimo_error.is_empty() {
                        format!("el repo {url} no trae manifiesto")
                    } else {
                        ultimo_error
                    });
                }
                meta.manifiestos = out;
                Ok(meta)
            }
            Fuente::Directo { url } => {
                let resp = t.get_texto(url, cabeceras)?;
                if resp.no_modificado() {
                    return Ok(ListadoConMeta {
                        not_modified: true,
                        ..Default::default()
                    });
                }
                if resp.status >= 400 {
                    return Err(format!("la URL devolvió {} en {}", resp.status, url));
                }
                let doc: Value = serde_json::from_str(&resp.texto())
                    .map_err(|e| format!("el manifiesto no es JSON: {e}"))?;
                let ms = manifiestos_de_doc(&doc)?;
                Ok(ListadoConMeta {
                    manifiestos: ms,
                    etag: resp.etag.clone(),
                    last_modified: resp.last_modified.clone(),
                    not_modified: false,
                })
            }
        }
    }
}

/// Extrae los SHA-256 de una release de GitHub.
///
/// La API reciente trae `digest` por asset; las antiguas, no. En ese caso se
/// busca el fichero `.sha256` hermano (que es lo que publica el CI) y se
/// descarga.
fn digests_de_release(doc: &Value) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Some(lista) = doc.get("assets").and_then(|v| v.as_array()) else {
        return out;
    };
    for a in lista {
        let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.is_empty() {
            continue;
        }
        if let Some(d) = a.get("digest").and_then(|v| v.as_str()) {
            let d = d.trim_start_matches("sha256:").trim().to_string();
            if d.len() == 64 {
                out.insert(name.to_string(), d);
            }
        }
    }
    out
}

/// Acepta un manifiesto suelto, un array de manifiestos o `{"manifests":[…]}`.
fn manifiestos_de_doc(doc: &Value) -> Result<Vec<Manifest>, String> {
    if let Some(lista) = doc.get("manifests").and_then(|v| v.as_array()) {
        let mut out = Vec::new();
        for m in lista {
            let parsed: Manifest = serde_json::from_value(m.clone())
                .map_err(|e| format!("manifiesto inválido en la lista: {e}"))?;
            parsed.validar()?;
            out.push(parsed);
        }
        if out.is_empty() {
            return Err("la lista 'manifests' está vacía".to_string());
        }
        return Ok(out);
    }
    if let Some(lista) = doc.as_array() {
        let mut out = Vec::new();
        for m in lista {
            let parsed: Manifest = serde_json::from_value(m.clone())
                .map_err(|e| format!("manifiesto inválido en la lista: {e}"))?;
            parsed.validar()?;
            out.push(parsed);
        }
        if out.is_empty() {
            return Err("la lista de manifiestos está vacía".to_string());
        }
        return Ok(out);
    }
    let m: Manifest = serde_json::from_value(doc.clone())
        .map_err(|e| format!("no es un manifiesto válido: {e}"))?;
    m.validar()?;
    Ok(vec![m])
}

/// Deriva la URL **cruda** de un repo git para una rama concreta.
pub fn url_raw_de_repo(url: &str, rama: &str) -> Result<String, String> {
    let u = url.trim().trim_end_matches('/');
    let r = if rama.trim().is_empty() {
        "main"
    } else {
        rama.trim()
    };
    if let Some(rest) = u.strip_prefix("https://github.com/") {
        let (owner, repo) = rest
            .split_once('/')
            .ok_or_else(|| format!("URL de repo incompleta: {url}"))?;
        let repo = repo.trim_end_matches(".git");
        return Ok(format!(
            "https://raw.githubusercontent.com/{owner}/{repo}/{r}/"
        ));
    }
    if let Some(rest) = u.strip_prefix("https://raw.githubusercontent.com/") {
        let partes: Vec<&str> = rest.split('/').collect();
        if partes.len() >= 2 {
            let owner = partes[0];
            let repo = partes[1].trim_end_matches(".git");
            return Ok(format!(
                "https://raw.githubusercontent.com/{owner}/{repo}/{r}/"
            ));
        }
        return Err(format!("URL cruda incompleta: {url}"));
    }
    Err(format!(
        "no se sabe derivar la URL cruda de '{url}' (sólo se admiten repos de GitHub)"
    ))
}

/// Nombre del asset a partir del nombre del fichero.
///
/// `megadaemon-core-android-arm64` → `megadaemon-core` (+ os/arch deduced).
fn nombre_de_asset(fichero: &str) -> String {
    let base = fichero.rsplit('/').next().unwrap_or(fichero);
    for sufijo in [
        "-android-arm64",
        "-android-x86_64",
        "-linux-x86_64",
        "-linux-arm64",
        "-linux-aarch64",
    ] {
        if let Some(cortado) = base.strip_suffix(sufijo) {
            return cortado.to_string();
        }
    }
    match base.rsplit_once('.') {
        Some((sin_ext, "apk")) => sin_ext.to_string(),
        _ => base.to_string(),
    }
}

/// Deduce el sistema operativo y la arquitectura del nombre del fichero.
fn adivinar_plataforma(nombre: &str) -> (String, String) {
    let n = nombre.to_ascii_lowercase();
    let os = if n.contains("android") {
        "android"
    } else if n.contains("linux") {
        "linux"
    } else if n.contains("darwin") || n.contains("macos") {
        "macos"
    } else if n.contains("windows") {
        "windows"
    } else {
        "any"
    };
    let arch = if n.contains("x86_64") || n.contains("amd64") {
        "x86_64"
    } else if n.contains("arm64") || n.contains("aarch64") {
        "arm64"
    } else if n.contains("armv7") || n.contains("armhf") {
        "arm"
    } else {
        "any"
    };
    (os.to_string(), arch.to_string())
}

/// Deduce el tipo de asset del nombre del fichero.
fn adivinar_tipo(nombre: &str) -> TipoAsset {
    let n = nombre.to_ascii_lowercase();
    if n.ends_with(".apk") {
        TipoAsset::Apk
    } else if n.ends_with(".ttf") || n.ends_with(".otf") {
        TipoAsset::Font
    } else if n.ends_with(".dic") || n.ends_with(".dict") {
        TipoAsset::Dict
    } else if n.contains("repo") || n.ends_with(".tar.gz") {
        TipoAsset::Repo
    } else {
        TipoAsset::Binary
    }
}

// ========================================================================= //
// Adaptador del `latest.json` heredado
// ========================================================================= //

/// Convierte el `latest.json` **heredado** del CI en un [`Manifest`].
///
/// El formato viejo (`{"version","assets":{…},"assets_versioned":{…}}`) NO
/// lleva SHA-256 ni tamaño: viven en ficheros `.sha256` sueltos. Este
/// adaptador los acepta por parámetro (`digests` y `sizes`) para poder
/// **migrar sin tocar el CI de golpe**; cuando el CI publique el formato
/// nuevo, este adaptador dejará de usarse.
///
/// La versión del heredado es un sha de commit ⇒ se normaliza a
/// `0.0.0+<sha>` (semver válido con metadatos de build).
pub fn adaptar_latest_json(
    doc: &Value,
    base_url: &str,
    digests: &BTreeMap<String, String>,
    sizes: &BTreeMap<String, u64>,
) -> Result<Manifest, String> {
    let version_cruda = doc
        .get("version")
        .and_then(|v| v.as_str())
        .or_else(|| doc.get("short").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();
    let version = if version_cruda.is_empty() {
        "0.0.0".to_string()
    } else {
        normalizar_version_tag(&version_cruda)
    };
    let released_at = doc
        .get("released_at")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let released_at = if released_at.is_empty() {
        ahora_iso()
    } else {
        released_at
    };

    let mut assets = Vec::new();
    for clave in ["assets_versioned", "assets"] {
        let Some(mapa) = doc.get(clave).and_then(|v| v.as_object()) else {
            continue;
        };
        for (plataforma, fichero) in mapa {
            let Some(fichero) = fichero.as_str() else {
                continue;
            };
            let sha = digests
                .get(fichero)
                .cloned()
                .or_else(|| digests.get(&nombre_de_asset(fichero)).cloned())
                .unwrap_or_default();
            let size = sizes
                .get(fichero)
                .copied()
                .or_else(|| sizes.get(&nombre_de_asset(fichero)).copied())
                .unwrap_or(0);
            let url = format!("{}/{}", base_url.trim_end_matches('/'), fichero);
            let (os, arch) = if plataforma.contains('-') {
                let mut it = plataforma.splitn(2, '-');
                let o = it.next().unwrap_or("any").to_string();
                let a = it.next().unwrap_or("any").to_string();
                (o, a)
            } else {
                adivinar_plataforma(fichero)
            };
            assets.push(Asset {
                name: nombre_de_asset(fichero),
                kind: adivinar_tipo(fichero),
                os,
                arch,
                url,
                sha256: sha.to_ascii_lowercase(),
                size,
                signature: None,
            });
        }
        if !assets.is_empty() {
            break;
        }
    }
    if assets.is_empty() {
        return Err("el latest.json heredado no declara assets".to_string());
    }
    let m = Manifest {
        schema_version: SCHEMA_VERSION,
        version,
        released_at,
        assets,
    };
    m.validar()?;
    Ok(m)
}

// ========================================================================= //
// Descarga reanudable + instalación atómica + rollback
// ========================================================================= //

/// Sufijo del fichero de descarga parcial (el que se **reanuda**).
pub const SUFIJO_PARCIAL: &str = ".new";
/// Sufijo del fichero con la versión anterior (el que permite el rollback).
pub const SUFIJO_PREVIO: &str = ".prev";

/// Ruta del fichero parcial (descarga reanudable) de un destino.
pub fn ruta_parcial(destino: &Path) -> PathBuf {
    let mut s = destino.as_os_str().to_os_string();
    s.push(SUFIJO_PARCIAL);
    PathBuf::from(s)
}

/// Ruta del fichero con la versión anterior de un destino.
pub fn ruta_previa(destino: &Path) -> PathBuf {
    let mut s = destino.as_os_str().to_os_string();
    s.push(SUFIJO_PREVIO);
    PathBuf::from(s)
}

/// SHA-256 de un fichero (streaming, sin cargarlo en memoria).
pub fn sha256_de_fichero(path: &Path) -> Result<(u64, String), String> {
    let f =
        fs::File::open(path).map_err(|e| format!("no se puede abrir {}: {e}", path.display()))?;
    crate::drive_client::sha256_reader(f)
}

/// Estado del hash **incremental** (para reanudar sin releer el fichero).
struct HasherParcial {
    hasher: Sha256,
    bytes: u64,
}

/// Prepara el hash de lo ya descargado (para continuar la suma al reanudar).
fn hasher_de_fichero(path: &Path) -> Result<HasherParcial, String> {
    let mut f =
        fs::File::open(path).map_err(|e| format!("no se puede abrir {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    let mut bytes = 0u64;
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| format!("error leyendo {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        bytes += n as u64;
    }
    Ok(HasherParcial { hasher, bytes })
}

/// Descarga **reanumable** por rango HTTP, verifica el SHA-256 e instala de
/// forma **atómica**.
///
/// * Si ya hay un parcial (`<destino>.new`), se pide `Range: bytes=N-` ⇒ la
///   descarga continúa donde se quedó (móvil sin red a media descarga: no se
///   tira el trabajo hecho).
/// * El SHA-256 se calcula **en la misma pasada** (y el parcial ya descargado
///   se mete en el hash antes de continuar) ⇒ ni una copia extra en disco.
/// * El SHA-256 se verifica **antes** de instalar: si no cuadra, el parcial se
///   borra y se devuelve el error (no se instala nada roto).
/// * El cambio es **atómico**: `rename` (nunca `cp`) ⇒ no hay "Text file busy"
///   al sustituir un binario en marcha, y no queda un fichero a medias.
///
/// Devuelve la ficha de lo instalado.
pub fn descargar_resumible(
    url: &str,
    sha256_esperado: &str,
    destino: &Path,
) -> Result<Instalado, String> {
    descargar_resumible_con(url, sha256_esperado, destino, &TransporteUreq::nuevo())
}

/// Igual que [`descargar_resumible`] pero con el transporte inyectado.
pub fn descargar_resumible_con<T: Transportador>(
    url: &str,
    sha256_esperado: &str,
    destino: &Path,
    t: &T,
) -> Result<Instalado, String> {
    if sha256_esperado.len() != 64 || !sha256_esperado.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "el sha256 esperado no es válido (64 hex): {sha256_esperado}"
        ));
    }
    let parcial = ruta_parcial(destino);
    if let Some(dir) = destino.parent() {
        if !dir.as_os_str().is_empty() {
            fs::create_dir_all(dir)
                .map_err(|e| format!("no se puede crear {}: {e}", dir.display()))?;
        }
    }

    // 1) ¿Cuánto hay ya descargado?
    let mut hasher = match hasher_de_fichero(&parcial) {
        Ok(h) => h,
        Err(_) => HasherParcial {
            hasher: Sha256::new(),
            bytes: 0,
        },
    };
    let mut offset = hasher.bytes;
    if offset > MAX_ARTIFACT_BYTES {
        let _ = fs::remove_file(&parcial);
        return Err("el fichero parcial es demasiado grande; se descarta".to_string());
    }

    // 2) Pedir (o continuar) la descarga.
    let rango = format!("bytes={offset}-");
    let cabeceras: Vec<(&str, &str)> = if offset > 0 {
        vec![("Range", rango.as_str())]
    } else {
        Vec::new()
    };
    let mut d = t
        .get_binario(url, offset, &cabeceras)
        .map_err(|e| format!("no se pudo descargar {url}: {e}"))?;

    // 3) ¿El servidor respeta el rango?
    let (reanudado, a_continuar) = match d.status {
        206 => (true, offset),
        200 => {
            // Ignoró el `Range` ⇒ hay que empezar de cero.
            (false, 0)
        }
        416 => {
            // Rango no satisfacible: el parcial ya está completo (o es basura).
            // Se comprueba el hash del parcial; si no cuadra, se descarta y se
            // empieza de cero (no se descarga a ciegas).
            let calculado = match hasher_de_fichero(&parcial) {
                Ok(h) => hex(&h.hasher.finalize()),
                Err(_) => String::new(),
            };
            if calculado.eq_ignore_ascii_case(sha256_esperado) {
                // El parcial **es** el fichero: se instala directamente.
                instalar_atomicamente(&parcial, destino)?;
                let size = fs::metadata(destino).map(|m| m.len()).unwrap_or(0);
                return Ok(Instalado {
                    kind: TipoAsset::Binary,
                    name: nombre_de_fichero(destino),
                    version: String::new(),
                    sha256: calculado,
                    size,
                    path: destino.to_string_lossy().to_string(),
                    signature: None,
                    source: "descarga".to_string(),
                });
            }
            let _ = fs::remove_file(&parcial);
            hasher = HasherParcial {
                hasher: Sha256::new(),
                bytes: 0,
            };
            offset = 0;
            (false, 0)
        }
        s => return Err(format!("la descarga de {url} devolvió {s}")),
    };

    if !reanudado && offset > 0 {
        // Se reinicia: el hash anterior ya no vale.
        hasher = HasherParcial {
            hasher: Sha256::new(),
            bytes: 0,
        };
        let _ = fs::remove_file(&parcial);
    }

    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(a_continuar > 0)
        .write(true)
        .truncate(a_continuar == 0)
        .open(&parcial)
        .map_err(|e| format!("no se puede abrir {}: {e}", parcial.display()))?;

    let mut buf = vec![0u8; 1024 * 1024];
    let mut total = hasher.bytes;
    loop {
        let n = d
            .reader
            .read(&mut buf)
            .map_err(|e| format!("error de red leyendo {url}: {e}"))?;
        if n == 0 {
            break;
        }
        f.write_all(&buf[..n])
            .map_err(|e| format!("error escribiendo {}: {e}", parcial.display()))?;
        hasher.hasher.update(&buf[..n]);
        total += n as u64;
        if total > MAX_ARTIFACT_BYTES {
            return Err("el artefacto supera el tamaño máximo permitido".to_string());
        }
    }
    f.flush()
        .map_err(|e| format!("no se pudo volcar {}: {e}", parcial.display()))?;
    drop(f);

    // 4) Verificar **antes** de instalar.
    let calculado = hex(&hasher.hasher.finalize());
    if !calculado.eq_ignore_ascii_case(sha256_esperado) {
        let _ = fs::remove_file(&parcial);
        return Err(format!(
            "sha256 no cuadra: esperado {sha256_esperado}, calculado {calculado} (no se instala nada)"
        ));
    }

    // 5) Instalación atómica: el anterior a `.prev`, el nuevo encima.
    instalar_atomicamente(&parcial, destino)?;

    Ok(Instalado {
        kind: TipoAsset::Binary,
        name: nombre_de_fichero(destino),
        version: String::new(),
        sha256: calculado,
        size: total,
        path: destino.to_string_lossy().to_string(),
        signature: None,
        source: "descarga".to_string(),
    })
}

/// Nombre de fichero de una ruta (sin directorios).
fn nombre_de_fichero(destino: &Path) -> String {
    destino
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// Sustituye `destino` por `origen` de forma **atómica**, guardando el
/// anterior como `<destino>.prev`.
///
/// ⚠️ **Por qué `rename` y no `cp`**: `cp` sobre un binario **en marcha** da
/// `ETXTBSY` ("Text file busy") y, si se corta, deja el binario truncado
/// (el móvil se queda sin daemon). `rename` es atómico dentro del mismo
/// sistema de ficheros: o el binario viejo está o el nuevo, nunca un
/// mélange.
///
/// Se intenta primero un **enlace duro** al anterior (`hard_link`): es gratis
/// y deja el `.prev` apuntando al inode viejo **sin ventana** en la que el
/// destino no existe. Si el enlace duro no es posible, se copia; y sólo si
/// tampoco, se renombra (dejando una ventana mínima).
pub fn instalar_atomicamente(origen: &Path, destino: &Path) -> Result<(), String> {
    let previo = ruta_previa(destino);
    let _ = fs::remove_file(&previo);
    let habia_destino = destino.exists();
    if habia_destino {
        if fs::hard_link(destino, &previo).is_err() && fs::copy(destino, &previo).is_err() {
            // Último recurso: renombrar (deja el `.prev` y una ventana mínima).
            fs::rename(destino, &previo)
                .map_err(|e| format!("no se pudo apartar {}: {e}", destino.display()))?;
        }
    }
    if let Some(dir) = destino.parent() {
        if !dir.as_os_str().is_empty() {
            let _ = fs::create_dir_all(dir);
        }
    }
    // `rename` sustituye el destino de forma atómica (mismo sistema de
    // ficheros ⇒ nunca "Text file busy").
    fs::rename(origen, destino).map_err(|e| {
        // Si el rename falla, se intenta volver a poner el anterior.
        if habia_destino && previo.exists() {
            let _ = fs::rename(&previo, destino);
        }
        format!("no se pudo instalar {}: {e}", destino.display())
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(md) = fs::metadata(destino) {
            if md.is_file() {
                let mut p = md.permissions();
                // Se conserva el modo anterior si había uno; si no, 0755.
                if p.mode() & 0o111 == 0 {
                    p.set_mode(0o755);
                    let _ = fs::set_permissions(destino, p);
                }
            }
        }
    }
    Ok(())
}

/// Lo que hay que hacer para **deshacer** una instalación.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rollback {
    /// Ruta del destino instalado.
    pub destino: String,
    /// Ruta del fichero con la versión anterior (`.prev`).
    pub previo: String,
    /// `true` si ya se restauró el anterior.
    pub restaurado: bool,
    /// Por qué se ha revertido.
    pub motivo: String,
}

/// Error de instalación: mensaje + rollback (si lo hubo).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorInstalacion {
    /// Qué pasó.
    pub mensaje: String,
    /// Qué hay que hacer para volver atrás (si procede).
    pub rollback: Option<Rollback>,
}

impl std::fmt::Display for ErrorInstalacion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.mensaje)?;
        if let Some(r) = &self.rollback {
            write!(
                f,
                " (rollback: {} ← {}, restaurado={})",
                r.destino, r.previo, r.restaurado
            )?;
        }
        Ok(())
    }
}

/// Instala un artefacto del manifiesto **con rollback**.
///
/// * Busca el asset en el manifiesto (por tipo y nombre).
/// * **Eficiencia**: si el destino ya tiene ese SHA-256, no hace **nada**
///   (cero descargas, cero escrituras).
/// * Descarga de forma reanudable y verifica el hash **antes** de instalar.
/// * Guarda el anterior como `<destino>.prev`.
/// * Si algo falla **después** de tocar el destino, restaura el anterior y
///   devuelve el [`Rollback`].
///
/// La firma pedida es `Result<(), String>`; [`instalar_con_rollback_detallado`]
/// da el detalle (incluido el `Rollback`).
pub fn instalar_con_rollback(
    instalado: &Instalado,
    manifest: &Manifest,
    ruta_destino: &Path,
) -> Result<(), String> {
    instalar_con_rollback_detallado(instalado, manifest, ruta_destino).map_err(|e| e.mensaje)
}

/// Igual que [`instalar_con_rollback`] pero devuelve el [`Rollback`].
pub fn instalar_con_rollback_detallado(
    instalado: &Instalado,
    manifest: &Manifest,
    ruta_destino: &Path,
) -> Result<(), ErrorInstalacion> {
    instalar_con_rollback_con(instalado, manifest, ruta_destino, &TransporteUreq::nuevo())
}

/// Igual que [`instalar_con_rollback_detallado`] con el transporte inyectado.
pub fn instalar_con_rollback_con<T: Transportador>(
    instalado: &Instalado,
    manifest: &Manifest,
    ruta_destino: &Path,
    t: &T,
) -> Result<(), ErrorInstalacion> {
    let asset = match manifest.asset_para(instalado.kind, &instalado.name, "any", "any") {
        Some(a) => a.clone(),
        None => {
            return Err(ErrorInstalacion {
                mensaje: format!(
                    "el manifiesto no declara el asset '{}' ({})",
                    instalado.name, instalado.kind
                ),
                rollback: None,
            })
        }
    };

    // Eficiencia: si el destino YA tiene el contenido correcto, no hay nada
    // que hacer (ni una descarga, ni una escritura).
    if ruta_destino.exists() {
        if let Ok((_, sha)) = sha256_de_fichero(ruta_destino) {
            if sha.eq_ignore_ascii_case(&asset.sha256) {
                return Ok(());
            }
        }
    }

    // Descarga (reanudable) + verificación + cambio atómico.
    let parcial = ruta_parcial(ruta_destino);
    if let Err(e) = descargar_resumible_con(&asset.url, &asset.sha256, ruta_destino, t) {
        return Err(ErrorInstalacion {
            mensaje: e,
            rollback: None,
        });
    }

    // Comprobación posterior: el fichero instalado tiene que ser el correcto.
    let verificacion = sha256_de_fichero(ruta_destino);
    match verificacion {
        Ok((_, sha)) if sha.eq_ignore_ascii_case(&asset.sha256) => Ok(()),
        Ok((_, sha)) => {
            // Contenido incorrecto tras instalar ⇒ a rollback.
            let restaurado = restaurar_previo(ruta_destino);
            Err(ErrorInstalacion {
                mensaje: format!(
                    "la comprobación posterior falló (sha instalado {sha}, esperado {})",
                    asset.sha256
                ),
                rollback: Some(Rollback {
                    destino: ruta_destino.to_string_lossy().to_string(),
                    previo: ruta_previa(ruta_destino).to_string_lossy().to_string(),
                    restaurado,
                    motivo: "el contenido instalado no coincide con el manifiesto".to_string(),
                }),
            })
        }
        Err(e) => {
            let restaurado = restaurar_previo(ruta_destino);
            Err(ErrorInstalacion {
                mensaje: format!("no se pudo comprobar lo instalado: {e}"),
                rollback: Some(Rollback {
                    destino: ruta_destino.to_string_lossy().to_string(),
                    previo: ruta_previa(ruta_destino).to_string_lossy().to_string(),
                    restaurado,
                    motivo: "no se pudo leer el fichero instalado".to_string(),
                }),
            })
        }
    }
    .map(|_| {
        // El parcial ya no existe (se renombró); si se quedó, se limpia.
        let _ = fs::remove_file(&parcial);
    })
}

/// Vuelve a poner la versión anterior (`.prev`) en su sitio.
pub fn restaurar_previo(destino: &Path) -> bool {
    let previo = ruta_previa(destino);
    if !previo.exists() {
        return false;
    }
    fs::rename(&previo, destino).is_ok()
}

// ========================================================================= //
// El índice unificado: instalado × disponible × qué está desfasado
// ========================================================================= //

/// Una fila de la tabla unificada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilaPaquete {
    /// Tipo (`apk`, `binary`, `repo`, `font`, `dict`).
    pub kind: String,
    /// Nombre del artefacto.
    pub nombre: String,
    /// Versión instalada.
    pub instalado_version: String,
    /// SHA-256 instalado (o vacío).
    pub instalado_sha: String,
    /// Versión disponible.
    pub disponible_version: String,
    /// SHA-256 disponible (o vacío).
    pub disponible_sha: String,
    /// Qué hay que hacer.
    pub accion: Accion,
    /// Por qué.
    pub motivo: Motivo,
    /// Explicación legible del desfase.
    pub detalle: String,
    /// Tamaño instalado (bytes).
    pub instalado_size: u64,
    /// Tamaño disponible (bytes).
    pub disponible_size: u64,
    /// Ruta local de lo instalado.
    pub ruta: String,
    /// Proveedor del que sale lo disponible (vacío si no se sabe).
    pub fuente: String,
}

impl FilaPaquete {
    /// Ficha JSON de la fila.
    pub fn to_json(&self) -> Value {
        json!({
            "kind": self.kind,
            "nombre": self.nombre,
            "instalado_version": self.instalado_version,
            "instalado_sha": self.instalado_sha,
            "disponible_version": self.disponible_version,
            "disponible_sha": self.disponible_sha,
            "accion": self.accion.as_str(),
            "motivo": self.motivo.as_str(),
            "detalle": self.detalle,
            "instalado_size": self.instalado_size,
            "disponible_size": self.disponible_size,
            "ruta": self.ruta,
            "fuente": self.fuente,
        })
    }
}

/// La **vista unificada** que pide el usuario: qué hay instalado, qué hay
/// disponible y **qué está desfasado y por qué**.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndicePaquetes {
    /// Las filas (una por artefacto).
    pub filas: Vec<FilaPaquete>,
    /// Cuándo se generó (ISO-8601).
    pub generado_en: String,
}

impl IndicePaquetes {
    /// Construye el índice a partir de lo instalado y lo disponible.
    pub fn construir(instalados: &[Instalado], disponibles: &[Manifest]) -> IndicePaquetes {
        let pares: Vec<(String, Manifest)> = disponibles
            .iter()
            .map(|m| (String::new(), m.clone()))
            .collect();
        IndicePaquetes::construir_con_fuentes(instalados, &pares)
    }

    /// Igual que [`IndicePaquetes::construir`] pero sabe de qué
    /// proveedor viene cada manifiesto.
    pub fn construir_con_fuentes(
        instalados: &[Instalado],
        fuentes: &[(String, Manifest)],
    ) -> IndicePaquetes {
        // 1) Aplanar lo disponible: (kind, nombre) → (asset, versión, fuente).
        let mut disponible: BTreeMap<(String, String), (Asset, String, String)> = BTreeMap::new();
        for (fuente, m) in fuentes {
            for a in &m.assets {
                let clave = (a.kind.as_str().to_string(), a.name.clone());
                let tomar = match disponible.get(&clave) {
                    None => true,
                    Some((_, v_actual, _)) => {
                        comparar_version(&m.version, v_actual) == std::cmp::Ordering::Greater
                    }
                };
                if tomar {
                    disponible.insert(clave, (a.clone(), m.version.clone(), fuente.clone()));
                }
            }
        }

        // 2) Una fila por artefacto instalado.
        let mut filas = Vec::new();
        let mut cubiertos: Vec<(String, String)> = Vec::new();
        for inst in instalados {
            let clave = (inst.kind.as_str().to_string(), inst.name.clone());
            let encontrado = disponible.get(&clave);
            let (accion, motivo) = match encontrado {
                Some((asset, version, _)) => evaluar(Some(inst), Some(asset), version),
                None => (Accion::Desconocido, Motivo::SinFuente),
            };
            let (disp_version, disp_sha, disp_size) = match encontrado {
                Some((asset, version, _)) => (version.clone(), asset.sha256.clone(), asset.size),
                None => (String::new(), String::new(), 0),
            };
            let detalle = detalle_de(&accion, &motivo, inst, encontrado);
            filas.push(FilaPaquete {
                kind: inst.kind.as_str().to_string(),
                nombre: inst.name.clone(),
                instalado_version: inst.version.clone(),
                instalado_sha: inst.sha256.clone(),
                disponible_version: disp_version,
                disponible_sha: disp_sha,
                accion,
                motivo,
                detalle,
                instalado_size: inst.size,
                disponible_size: disp_size,
                ruta: inst.path.clone(),
                fuente: encontrado.map(|(_, _, f)| f.clone()).unwrap_or_default(),
            });
            cubiertos.push(clave);
        }

        // 3) Lo que está disponible y **no** está instalado.
        for ((kind, nombre), (asset, version, fuente)) in disponible.iter() {
            if cubiertos.iter().any(|(k, n)| k == kind && n == nombre) {
                continue;
            }
            filas.push(FilaPaquete {
                kind: kind.clone(),
                nombre: nombre.clone(),
                instalado_version: String::new(),
                instalado_sha: String::new(),
                disponible_version: version.clone(),
                disponible_sha: asset.sha256.clone(),
                accion: Accion::NoInstalado,
                motivo: Motivo::NoInstalado,
                detalle: format!("disponible pero no instalado ({} bytes)", asset.size),
                instalado_size: 0,
                disponible_size: asset.size,
                ruta: String::new(),
                fuente: fuente.clone(),
            });
        }

        // Orden estable: primero lo que hay que actualizar.
        filas.sort_by(|a, b| {
            let pa = prioridad_accion(a.accion);
            let pb = prioridad_accion(b.accion);
            pa.cmp(&pb)
                .then_with(|| a.kind.cmp(&b.kind))
                .then_with(|| a.nombre.cmp(&b.nombre))
        });

        IndicePaquetes {
            filas,
            generado_en: ahora_iso(),
        }
    }

    /// Sólo las filas que piden una acción.
    pub fn pendientes(&self) -> Vec<&FilaPaquete> {
        self.filas
            .iter()
            .filter(|f| f.accion == Accion::Actualizar || f.accion == Accion::NoInstalado)
            .collect()
    }

    /// Recuento por acción.
    pub fn resumen(&self) -> Value {
        let mut cuenta = [0usize; 4];
        for f in &self.filas {
            let i = match f.accion {
                Accion::Actualizar => 0,
                Accion::YaAlDia => 1,
                Accion::NoInstalado => 2,
                Accion::Desconocido => 3,
            };
            cuenta[i] += 1;
        }
        json!({
            "total": self.filas.len(),
            "actualizar": cuenta[0],
            "ya_al_dia": cuenta[1],
            "no_instalado": cuenta[2],
            "desconocido": cuenta[3],
        })
    }

    /// Ficha JSON del índice.
    pub fn to_json(&self) -> Value {
        json!({
            "ok": true,
            "count": self.filas.len(),
            "generado_en": self.generado_en,
            "resumen": self.resumen(),
            "filas": self.filas.iter().map(|f| f.to_json()).collect::<Vec<_>>(),
        })
    }

    /// La tabla plana, en texto.
    pub fn tabla(&self) -> String {
        let cabeceras = [
            "KIND",
            "NOMBRE",
            "INSTALADA",
            "DISPONIBLE",
            "ACCION",
            "MOTIVO",
        ];
        let mut filas: Vec<[String; 6]> = Vec::with_capacity(self.filas.len());
        for f in &self.filas {
            filas.push([
                f.kind.clone(),
                f.nombre.clone(),
                if f.instalado_version.is_empty() {
                    "-".to_string()
                } else {
                    f.instalado_version.clone()
                },
                if f.disponible_version.is_empty() {
                    "-".to_string()
                } else {
                    f.disponible_version.clone()
                },
                f.accion.as_str().to_string(),
                f.motivo.as_str().to_string(),
            ]);
        }
        let mut anchos = [0usize; 6];
        for (i, h) in cabeceras.iter().enumerate() {
            anchos[i] = h.chars().count();
        }
        for f in &filas {
            for (i, c) in f.iter().enumerate() {
                let n = c.chars().count();
                if n > anchos[i] {
                    anchos[i] = n;
                }
            }
        }
        let mut out = String::new();
        let linea = |out: &mut String, celdas: &[String; 6], anchos: &[usize; 6]| {
            for (i, c) in celdas.iter().enumerate() {
                if i + 1 == celdas.len() {
                    out.push_str(c);
                } else {
                    out.push_str(c);
                    let n = c.chars().count();
                    for _ in n..(anchos[i] + 2) {
                        out.push(' ');
                    }
                }
            }
            out.push('\n');
        };
        linea(
            &mut out,
            &[
                cabeceras[0].to_string(),
                cabeceras[1].to_string(),
                cabeceras[2].to_string(),
                cabeceras[3].to_string(),
                cabeceras[4].to_string(),
                cabeceras[5].to_string(),
            ],
            &anchos,
        );
        for f in &filas {
            linea(&mut out, &f, &anchos);
        }
        out
    }
}

/// Prioridad de ordenación de las acciones (lo que hay que hacer, primero).
fn prioridad_accion(a: Accion) -> u8 {
    match a {
        Accion::Actualizar => 0,
        Accion::NoInstalado => 1,
        Accion::Desconocido => 2,
        Accion::YaAlDia => 3,
    }
}

/// Explicación legible del desfase.
fn detalle_de(
    accion: &Accion,
    motivo: &Motivo,
    inst: &Instalado,
    encontrado: Option<&(Asset, String, String)>,
) -> String {
    match accion {
        Accion::Actualizar => match motivo {
            Motivo::ShaDistinto => {
                let disp = encontrado.map(|e| e.0.sha256.clone()).unwrap_or_default();
                format!(
                    "sha-256 distinto: instalado {}…, disponible {}…",
                    corto(&inst.sha256),
                    corto(&disp)
                )
            }
            Motivo::VersionDistinta => {
                let disp = encontrado.map(|e| e.1.clone()).unwrap_or_default();
                format!("versión distinta: {} → {}", inst.version, disp)
            }
            _ => "hay que actualizar".to_string(),
        },
        Accion::YaAlDia => "el contenido instalado es idéntico al disponible".to_string(),
        Accion::NoInstalado => "disponible pero no instalado".to_string(),
        Accion::Desconocido => match motivo {
            Motivo::SinFuente => "no hay ninguna fuente disponible para este artefacto".to_string(),
            Motivo::SinHash => format!(
                "no se puede verificar el contenido (sin sha-256 local); versión instalada {}",
                if inst.version.is_empty() {
                    "-"
                } else {
                    &inst.version
                }
            ),
            _ => "no se puede saber".to_string(),
        },
    }
}

/// Compara dos versiones semver-ish (numérico cuando puede).
pub fn comparar_version(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let pa = partes_version(a);
    let pb = partes_version(b);
    for i in 0..pa.len().max(pb.len()) {
        let x = pa.get(i).copied().unwrap_or(0);
        let y = pb.get(i).copied().unwrap_or(0);
        match x.cmp(&y) {
            Ordering::Equal => continue,
            o => return o,
        }
    }
    a.cmp(b)
}

/// Partes numéricas de una versión.
fn partes_version(v: &str) -> Vec<u64> {
    let s = v.trim();
    let s = s.strip_prefix('v').unwrap_or(s);
    let s = s.split(['-', '+']).next().unwrap_or(s);
    s.split('.')
        .map(|p| p.trim().parse::<u64>().unwrap_or(0))
        .collect()
}

// ========================================================================= //
// Cursores por proveedor y cuarentena de fallos
// ========================================================================= //

/// Cursor de un proveedor: evita releer el manifiesto entero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CursorProveedor {
    /// Identificador del proveedor.
    pub id: String,
    /// `ETag` de la última respuesta.
    pub etag: Option<String>,
    /// `Last-Modified` de la última respuesta.
    pub last_modified: Option<String>,
    /// SHA-256 del documento del manifiesto (para saber si cambió).
    pub sha256_manifiesto: String,
    /// Número de assets del último manifiesto.
    pub assets: usize,
    /// Epoch del último refresco.
    pub actualizado_en: u64,
    /// Descripción de la fuente.
    pub fuente: String,
}

/// Registro de cursores (uno por proveedor).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RegistroCursores {
    /// Cursor por identificador de proveedor.
    pub cursores: BTreeMap<String, CursorProveedor>,
}

/// Un fallo de un proveedor (para no reintentar lo que ya falló).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FalloProveedor {
    /// Identificador del proveedor.
    pub id: String,
    /// URL a la que se intentó.
    pub url: String,
    /// Número de intentos.
    pub intentos: u32,
    /// Último error.
    pub ultimo_error: String,
    /// Epoch del último intento.
    pub ultimo_intento: u64,
    /// Epoch hasta el que no se reintenta.
    pub reintentar_tras: u64,
}

/// Registro de fallos.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RegistroFallos {
    /// Fallo por proveedor.
    pub fallos: BTreeMap<String, FalloProveedor>,
}

impl RegistroFallos {
    /// ¿Este proveedor está en cuarentena (no se reintenta)?
    pub fn en_cuarentena(&self, id: &str, ahora: u64) -> bool {
        match self.fallos.get(id) {
            Some(f) => ahora < f.reintentar_tras,
            None => false,
        }
    }

    /// Registra un fallo y calcula el siguiente backoff.
    pub fn registrar(&mut self, id: &str, url: &str, error: &str, ahora: u64) {
        let intentos = self.fallos.get(id).map(|f| f.intentos + 1).unwrap_or(1);
        let backoff = BACKOFF_BASE_SECS
            .saturating_mul(1u64 << (intentos.min(6) - 1))
            .min(BACKOFF_MAX_SECS);
        self.fallos.insert(
            id.to_string(),
            FalloProveedor {
                id: id.to_string(),
                url: url.to_string(),
                intentos,
                ultimo_error: error.to_string(),
                ultimo_intento: ahora,
                reintentar_tras: ahora + backoff,
            },
        );
    }

    /// Borra el fallo (el proveedor volvió a responder).
    pub fn limpiar(&mut self, id: &str) {
        self.fallos.remove(id);
    }
}

// ========================================================================= //
// Estado en disco (cursores, fallos, manifiesto cacheado)
// ========================================================================= //

/// Ruta del directorio de estado de `pkgmgr`.
pub fn dir_estado(state_dir: &str) -> PathBuf {
    PathBuf::from(state_dir).join(ESTADO_DIR)
}

/// Ruta del fichero de cursores.
pub fn ruta_cursores(state_dir: &str) -> PathBuf {
    dir_estado(state_dir).join(CURSORES_FILE)
}

/// Ruta del fichero de fallos.
pub fn ruta_fallos(state_dir: &str) -> PathBuf {
    dir_estado(state_dir).join(FALLOS_FILE)
}

/// Ruta de la caché de manifiestos de un proveedor.
pub fn ruta_manifiesto_cache(state_dir: &str, proveedor_id: &str) -> PathBuf {
    let seguro = sanitize_id(proveedor_id);
    dir_estado(state_dir)
        .join("manifests")
        .join(format!("{seguro}.json"))
}

/// Convierte un identificador en algo que se pueda usar como nombre de fichero.
pub fn sanitize_id(id: &str) -> String {
    let s: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.len() > 120 {
        s[..120].to_string()
    } else {
        s
    }
}

/// Lee el registro de cursores (tolerante: si no está o está roto, vacío).
pub fn leer_cursores(state_dir: &str) -> RegistroCursores {
    leer_json(&ruta_cursores(state_dir)).unwrap_or_default()
}

/// Guarda el registro de cursores.
pub fn guardar_cursores(state_dir: &str, reg: &RegistroCursores) -> Result<(), String> {
    escribir_json(&ruta_cursores(state_dir), reg)
}

/// Lee el registro de fallos (tolerante).
pub fn leer_fallos(state_dir: &str) -> RegistroFallos {
    leer_json(&ruta_fallos(state_dir)).unwrap_or_default()
}

/// Guarda el registro de fallos.
pub fn guardar_fallos(state_dir: &str, reg: &RegistroFallos) -> Result<(), String> {
    escribir_json(&ruta_fallos(state_dir), reg)
}

/// Lee un JSON de disco (tolerante).
fn leer_json<T: for<'de> Deserialize<'de>>(ruta: &Path) -> Option<T> {
    let texto = fs::read_to_string(ruta).ok()?;
    serde_json::from_str(&texto).ok()
}

/// Escribe un JSON de disco (creando el directorio).
fn escribir_json<T: Serialize>(ruta: &Path, valor: &T) -> Result<(), String> {
    if let Some(dir) = ruta.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("no se puede crear {}: {e}", dir.display()))?;
    }
    let mut texto = serde_json::to_string_pretty(valor).unwrap_or_default();
    texto.push('\n');
    fs::write(ruta, texto).map_err(|e| format!("no se puede escribir {}: {e}", ruta.display()))
}

/// Resultado de refrescar un proveedor con cursor.
#[derive(Debug, Clone, PartialEq)]
pub struct ManifiestoCacheado {
    /// Los manifiestos del proveedor.
    pub manifiestos: Vec<Manifest>,
    /// El cursor actualizado.
    pub cursor: CursorProveedor,
    /// `true` si el servidor dijo "sin cambios" (no se releyó nada).
    pub sin_cambios: bool,
    /// Error del proveedor, si lo hubo (se sirve la copia de caché).
    pub error: Option<String>,
}

/// Guarda la copia local de los manifiestos de un proveedor.
pub fn guardar_manifiesto_cache(
    state_dir: &str,
    proveedor_id: &str,
    manifiestos: &[Manifest],
) -> Result<(), String> {
    escribir_json(
        &ruta_manifiesto_cache(state_dir, proveedor_id),
        &manifiestos,
    )
}

/// Lee el manifiesto cacheado de un proveedor (con todos sus manifiestos).
pub fn leer_manifiesto_cache(state_dir: &str, proveedor_id: &str) -> Option<Vec<Manifest>> {
    let texto = fs::read_to_string(ruta_manifiesto_cache(state_dir, proveedor_id)).ok()?;
    serde_json::from_str(&texto).ok()
}

/// Refresca un proveedor **usando el cursor** (ETag / Last-Modified).
///
/// * Si el cursor tiene ETag, se manda `If-None-Match`; si tiene
///   `Last-Modified`, `If-Modified-Since`.
/// * Si el servidor responde `304 Not Modified`, se devuelve el manifiesto
///   **de la caché** ⇒ no se relee (ni se reparsea) el manifiesto entero.
/// * Si el proveedor está en **cuarentena** (falló hace poco), no se intenta.
pub fn refrescar_proveedor<T: Transportador>(
    fuente: &Fuente,
    state_dir: &str,
    t: &T,
) -> Result<ManifiestoCacheado, String> {
    let id = fuente.id();
    let ahora = ahora_epoch();
    let mut fallos = leer_fallos(state_dir);
    if fallos.en_cuarentena(&id, ahora) {
        let (reintentar, ultimo) = match fallos.fallos.get(&id) {
            Some(f) => (f.reintentar_tras, f.ultimo_error.clone()),
            None => (0, String::new()),
        };
        return Err(format!(
            "el proveedor '{}' está en cuarentena hasta {} (último error: {})",
            id, reintentar, ultimo
        ));
    }

    let cursores = leer_cursores(state_dir);
    let cursor_previo = cursores.cursores.get(&id).cloned();

    let mut cabeceras: Vec<(&str, &str)> = Vec::new();
    if let Some(c) = &cursor_previo {
        if let Some(e) = &c.etag {
            cabeceras.push(("If-None-Match", e.as_str()));
        }
        if let Some(lm) = &c.last_modified {
            cabeceras.push(("If-Modified-Since", lm.as_str()));
        }
    }

    match fuente.listar_con_meta(t, &cabeceras) {
        Ok(listado) => {
            // 304: el proveedor dice "no ha cambiado" ⇒ se sirve la caché
            // **sin releer nada**.
            if listado.not_modified {
                let ms = leer_manifiesto_cache(state_dir, &id).unwrap_or_default();
                let cursor = cursor_previo.unwrap_or(CursorProveedor {
                    id: id.clone(),
                    etag: None,
                    last_modified: None,
                    sha256_manifiesto: String::new(),
                    assets: 0,
                    actualizado_en: ahora,
                    fuente: fuente.descripcion(),
                });
                return Ok(ManifiestoCacheado {
                    cursor,
                    manifiestos: ms,
                    sin_cambios: true,
                    error: None,
                });
            }
            let mani = listado.manifiestos;
            fallos.limpiar(&id);
            let _ = guardar_fallos(state_dir, &fallos);
            let cursor = CursorProveedor {
                id: id.clone(),
                etag: listado.etag.clone(),
                last_modified: listado.last_modified.clone(),
                sha256_manifiesto: hash_manifiestos(&mani),
                assets: mani.iter().map(|m| m.assets.len()).sum(),
                actualizado_en: ahora,
                fuente: fuente.descripcion(),
            };
            // Se guarda la copia local para poder servirla sin red.
            let _ = guardar_manifiesto_cache(state_dir, &id, &mani);
            let mut reg = cursores;
            reg.cursores.insert(id.clone(), cursor.clone());
            let _ = guardar_cursores(state_dir, &reg);
            Ok(ManifiestoCacheado {
                manifiestos: mani,
                cursor,
                sin_cambios: false,
                error: None,
            })
        }
        Err(e) => {
            // Puede ser un fallo momentáneo: si el manifiesto está en caché,
            // se sirve sin volver a pedirlo (pero se dice que hubo error).
            if let Some(ms) = leer_manifiesto_cache(state_dir, &id) {
                if !ms.is_empty() {
                    let cursor = cursor_previo.unwrap_or(CursorProveedor {
                        id: id.clone(),
                        etag: None,
                        last_modified: None,
                        sha256_manifiesto: String::new(),
                        assets: 0,
                        actualizado_en: ahora,
                        fuente: fuente.descripcion(),
                    });
                    return Ok(ManifiestoCacheado {
                        cursor,
                        manifiestos: ms,
                        sin_cambios: true,
                        error: Some(e),
                    });
                }
            }
            fallos.registrar(&id, &fuente.descripcion(), &e, ahora);
            let _ = guardar_fallos(state_dir, &fallos);
            Err(e)
        }
    }
}

/// Hash conjunto de unos manifiestos (para el cursor).
fn hash_manifiestos(ms: &[Manifest]) -> String {
    let mut h = Sha256::new();
    for m in ms {
        h.update(m.a_json_texto().as_bytes());
    }
    hex(&h.finalize())
}

// ========================================================================= //
// Instalado: de dónde sale cada cosa
// ========================================================================= //

/// APKs instalados (del inventario de `apk_inventory`).
///
/// En un PC sin Android devuelve una lista vacía **honestamente** (no
/// inventa nada).
pub fn instalados_desde_inventario(limit: usize) -> Vec<Instalado> {
    let inv = crate::apk_inventory::inventory(limit);
    let Some(lista) = inv.get("packages").and_then(|p| p.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for p in lista {
        let Some(nombre) = p.get("package").and_then(|v| v.as_str()) else {
            continue;
        };
        let mut inst = Instalado::nuevo(
            TipoAsset::Apk,
            nombre,
            p.get("apk_path").and_then(|v| v.as_str()).unwrap_or(""),
        );
        inst.version = p
            .get("version_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        inst.sha256 = p
            .get("sha256")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        inst.size = p.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
        inst.source = "inventario".to_string();
        out.push(inst);
    }
    out
}

/// Binarios instalados en un directorio (con su SHA-256 calculado).
pub fn instalados_desde_directorio(dir: &Path) -> Vec<Instalado> {
    let Ok(rd) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        if !p.is_file() {
            continue;
        }
        let nombre = p
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        if nombre.starts_with('.') {
            continue;
        }
        let (size, sha) = match sha256_de_fichero(&p) {
            Ok(v) => v,
            Err(_) => (0, String::new()),
        };
        let mut inst = Instalado::nuevo(TipoAsset::Binary, &nombre, &p.to_string_lossy());
        inst.sha256 = sha;
        inst.size = size;
        inst.source = format!("directorio:{}", dir.display());
        out.push(inst);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Repos git instalados (del estado de `git_ops`).
///
/// Si `git_ops` no puede leer el repo (no hay Android, no hay repo, no hay
/// `git`), se devuelve una lista **vacía**: no se inventa un repo que no
/// está.
pub fn instalados_desde_git() -> Vec<Instalado> {
    let st = crate::git_ops::status();
    let ok = st.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    let ruta = st.get("repo").and_then(|v| v.as_str()).unwrap_or("");
    if !ok || ruta.is_empty() {
        return Vec::new();
    }
    if !Path::new(ruta).exists() {
        return Vec::new();
    }
    let mut inst = Instalado::nuevo(TipoAsset::Repo, "megadaemon", ruta);
    inst.version = st
        .get("branch")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let head = st
        .get("recent_commits")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .map(|s| s.split_whitespace().next().unwrap_or("").to_string())
        .unwrap_or_default();
    inst.signature = if head.is_empty() { None } else { Some(head) };
    inst.source = "git".to_string();
    vec![inst]
}

/// Todo lo instalado, de las tres fuentes.
pub fn instalados_completos(state_dir: &str, limit_inventario: usize) -> Vec<Instalado> {
    let mut out = instalados_desde_inventario(limit_inventario);
    out.extend(instalados_desde_directorio(
        &dir_estado(state_dir).join("bin"),
    ));
    out.extend(instalados_desde_git());
    out
}

// ========================================================================= //
// Fuentes: configuración y catálogo por defecto
// ========================================================================= //

/// Configuración de fuentes (fichero `fuentes.json`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ConfigFuentes {
    /// Las fuentes configuradas.
    pub fuentes: Vec<Fuente>,
}

/// Lee la configuración de fuentes (`<state_dir>/pkgmgr/fuentes.json`).
pub fn leer_fuentes(state_dir: &str) -> ConfigFuentes {
    leer_json(&dir_estado(state_dir).join(FUENTES_FILE)).unwrap_or_default()
}

/// Guarda la configuración de fuentes.
pub fn guardar_fuentes(state_dir: &str, cfg: &ConfigFuentes) -> Result<(), String> {
    escribir_json(&dir_estado(state_dir).join(FUENTES_FILE), cfg)
}

/// Catálogo de fuentes por defecto.
///
/// * La release del CI (`boolforge/megadaemon`), salvo que se diga otra cosa.
/// * El canal del daemon (`state/channel.json` publicado).
/// * El repo git del proyecto (el manifiesto puede vivir en el propio repo).
///
/// Todo es **configurable por entorno** y se puede ampliar con
/// `<state_dir>/pkgmgr/fuentes.json`.
pub fn fuentes_por_defecto(state_dir: &str) -> Vec<Fuente> {
    let mut out = Vec::new();

    let owner_repo = std::env::var("MEGADAEMON_PKG_REPO")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "boolforge/megadaemon".to_string());
    if let Some((owner, repo)) = owner_repo.split_once('/') {
        let tag = std::env::var("MEGADAEMON_PKG_TAG")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "latest".to_string());
        out.push(Fuente::Release(GitHubRelease {
            owner: owner.to_string(),
            repo: repo.to_string(),
            tag,
        }));
    }

    if let Ok(url) = std::env::var("MEGADAEMON_PKG_CHANNEL_URL") {
        let url = url.trim().to_string();
        if !url.is_empty() {
            out.push(Fuente::Canal(ChannelJson { url }));
        }
    }

    if let Ok(url) = std::env::var("MEGADAEMON_PKG_MANIFEST_URL") {
        let url = url.trim().to_string();
        if !url.is_empty() {
            out.push(Fuente::Directo { url });
        }
    }

    if let Ok(url) = std::env::var("MEGADAEMON_PKG_GIT_URL") {
        let url = url.trim().to_string();
        if !url.is_empty() {
            let rama = std::env::var("MEGADAEMON_PKG_GIT_BRANCH")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "main".to_string());
            out.push(Fuente::RepoGit { url, rama });
        }
    }

    // El canal local del propio daemon (si existe el fichero).
    let canal_local = PathBuf::from(state_dir).join("channel.json");
    if canal_local.exists() {
        // El canal local no se descarga: se lee del disco.
    }

    // Fuentes del usuario (tienen prioridad: sustituyen a las de por defecto
    // con el mismo identificador).
    let cfg = leer_fuentes(state_dir);
    for f in cfg.fuentes {
        if let Some(slot) = out.iter_mut().find(|x| x.id() == f.id()) {
            *slot = f;
        } else {
            out.push(f);
        }
    }
    out
}

/// Destination por defecto de un artefacto según su tipo.
pub fn destino_por_defecto(state_dir: &str, kind: TipoAsset, name: &str) -> PathBuf {
    let base = dir_estado(state_dir);
    match kind {
        TipoAsset::Apk => base.join("apks").join(format!("{name}.apk")),
        TipoAsset::Binary => base.join("bin").join(name),
        TipoAsset::Repo => base.join("repos").join(name),
        TipoAsset::Font => base.join("fonts").join(name),
        TipoAsset::Dict => base.join("dicts").join(name),
    }
}

// ========================================================================= //
// Utilidades
// ========================================================================= //

/// Epoch actual (segundos).
pub fn ahora_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Instante actual en ISO-8601 (UTC).
pub fn ahora_iso() -> String {
    let secs = ahora_epoch();
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days as i64);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

/// Días desde epoch ⇒ fecha civil (algoritmo de Howard Hinnant).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Hexadecimal en minúsculas.
fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

// ========================================================================= //
// Tests
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;

    // ── Utilidades de test ───────────────────────────────────────────────

    /// SHA-256 de una cadena de prueba.
    fn sha_de(s: &str) -> String {
        let mut h = Sha256::new();
        h.update(s.as_bytes());
        hex(&h.finalize())
    }

    /// Asset válido de prueba.
    fn asset_ok(nombre: &str) -> Asset {
        Asset::nuevo(
            nombre,
            TipoAsset::Binary,
            "any",
            "any",
            "https://example.org/descarga",
            &sha_de(nombre),
            1024,
        )
    }

    /// Manifiesto válido de prueba.
    fn manifiesto_ok() -> Manifest {
        Manifest {
            schema_version: SCHEMA_VERSION,
            version: "1.2.3".to_string(),
            released_at: "2026-11-21T10:00:00Z".to_string(),
            assets: vec![asset_ok("megadaemon-core")],
        }
    }

    /// Directorio temporal único por test.
    fn dir_tmp(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("md_pkgmgr_{nombre}"));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).expect("no se pudo crear el temporal");
        d
    }

    // ── (a) Manifiesto válido / inválido ─────────────────────────────────

    #[test]
    fn un_manifiesto_valido_pasa_la_validacion() {
        let m = manifiesto_ok();
        assert!(m.validar().is_ok(), "{:?}", m.validar());
    }

    #[test]
    fn un_sha256_malo_invalida_el_manifiesto() {
        let mut m = manifiesto_ok();
        m.assets[0].sha256 = "abc123".to_string();
        let e = m.validar().unwrap_err();
        assert!(e.contains("sha256"), "{e}");
    }

    #[test]
    fn un_sha256_con_caracteres_no_hex_invalida_el_manifiesto() {
        let mut m = manifiesto_ok();
        m.assets[0].sha256 = "z".repeat(64);
        assert!(m.validar().is_err());
    }

    #[test]
    fn una_url_http_invalida_el_manifiesto() {
        let mut m = manifiesto_ok();
        m.assets[0].url = "http://example.org/descarga".to_string();
        let e = m.validar().unwrap_err();
        assert!(e.contains("https"), "{e}");
    }

    #[test]
    fn un_tamano_cero_invalida_el_manifiesto() {
        let mut m = manifiesto_ok();
        m.assets[0].size = 0;
        let e = m.validar().unwrap_err();
        assert!(e.contains("tamaño 0"), "{e}");
    }

    #[test]
    fn un_nombre_con_ruta_invalida_el_manifiesto() {
        let mut m = manifiesto_ok();
        m.assets[0].name = "../etc/passwd".to_string();
        let e = m.validar().unwrap_err();
        assert!(e.contains(".."), "{e}");

        let mut m2 = manifiesto_ok();
        m2.assets[0].name = "carpeta/fichero".to_string();
        assert!(m2.validar().is_err());
    }

    #[test]
    fn una_version_no_semver_invalida_el_manifiesto() {
        let mut m = manifiesto_ok();
        m.version = "no-es-una-version".to_string();
        assert!(m.validar().is_err());
        m.version = "1.2".to_string();
        assert!(m.validar().is_ok());
        m.version = "v13.4.0-rc1".to_string();
        assert!(m.validar().is_ok());
        m.version = "13.4.0+build.7".to_string();
        assert!(m.validar().is_ok());
    }

    #[test]
    fn una_fecha_que_no_es_iso_invalida_el_manifiesto() {
        let mut m = manifiesto_ok();
        m.released_at = "ayer".to_string();
        assert!(m.validar().is_err());
    }

    #[test]
    fn un_esquema_distinto_invalida_el_manifiesto() {
        let mut m = manifiesto_ok();
        m.schema_version = 99;
        assert!(m.validar().is_err());
    }

    // ── (b) `comprobar_actualizacion` con el mismo SHA ⇒ None ────────────

    #[test]
    fn con_el_mismo_sha_no_hay_actualizacion_ninguna() {
        let m = manifiesto_ok();
        let mut inst = Instalado::nuevo(TipoAsset::Binary, "megadaemon-core", "/tmp/x");
        inst.sha256 = m.assets[0].sha256.clone();
        inst.version = "0.0.1".to_string(); // versión DISTINTA a propósito
        let r = comprobar_actualizacion(&inst, &m);
        assert!(
            r.is_none(),
            "con el mismo contenido no debe haber actualización (cero descarga): {r:?}"
        );
    }

    #[test]
    fn con_un_sha_distinto_si_hay_actualizacion() {
        let m = manifiesto_ok();
        let mut inst = Instalado::nuevo(TipoAsset::Binary, "megadaemon-core", "/tmp/x");
        inst.sha256 = sha_de("otro contenido");
        let r = comprobar_actualizacion(&inst, &m).expect("debería haber actualización");
        assert_eq!(r.accion, Accion::Actualizar);
        assert_eq!(r.motivo, Motivo::ShaDistinto);
        assert_eq!(r.bytes_a_descargar, m.assets[0].size);
    }

    #[test]
    fn un_asset_que_no_existe_no_pide_actualizacion() {
        let m = manifiesto_ok();
        let inst = Instalado::nuevo(TipoAsset::Binary, "otro-cosa", "/tmp/x");
        assert!(comprobar_actualizacion(&inst, &m).is_none());
    }

    #[test]
    fn sin_hash_local_la_version_manda() {
        let mut m = manifiesto_ok();
        m.version = "2.0.0".to_string();
        let mut inst = Instalado::nuevo(TipoAsset::Binary, "megadaemon-core", "/tmp/x");
        inst.version = "1.0.0".to_string();
        let r = comprobar_actualizacion(&inst, &m).expect("versión distinta ⇒ actualizar");
        assert_eq!(r.motivo, Motivo::VersionDistinta);
    }

    // ── (c) Round-trip del manifiesto en JSON ─────────────────────────────

    #[test]
    fn el_manifiesto_sobrevive_a_un_round_trip_de_json() {
        let m = manifiesto_ok();
        let texto = m.a_json_texto();
        let vuelta = Manifest::de_json(&texto).expect("no se pudo volver a leer");
        assert_eq!(m, vuelta);
        assert!(vuelta.validar().is_ok());
    }

    #[test]
    fn el_manifesto_heredado_se_adapta_al_esquema_unico() {
        let doc = json!({
            "version": "c7575420000000000000000000000000000000",
            "short": "c757542",
            "released_at": "2026-11-21T00:00:00Z",
            "assets": { "android-arm64": "megadaemon-core-android-arm64" },
            "assets_versioned": {
                "android-arm64": "megadaemon-core-android-arm64-c757542"
            },
        });
        let mut digests = BTreeMap::new();
        digests.insert(
            "megadaemon-core-android-arm64-c757542".to_string(),
            sha_de("binario"),
        );
        let mut sizes = BTreeMap::new();
        sizes.insert("megadaemon-core-android-arm64-c757542".to_string(), 4242u64);
        let m = adaptar_latest_json(&doc, "https://example.org/releases/v1", &digests, &sizes)
            .expect("no se pudo adaptar");
        assert!(m.validar().is_ok());
        assert_eq!(m.assets.len(), 1);
        assert_eq!(m.assets[0].os, "android");
        assert_eq!(m.assets[0].arch, "arm64");
        assert_eq!(m.assets[0].kind, TipoAsset::Binary);
        assert!(m.version.starts_with("0.0.0+"), "{}", m.version);
    }

    // ── (d) La instalación atómica deja el `.prev` ───────────────────────

    /// Transporte de mentira: sirve un cuerpo fijo.
    struct TransporteFalso {
        cuerpo: Vec<u8>,
    }

    impl TransporteFalso {
        fn nuevo(cuerpo: &[u8]) -> TransporteFalso {
            TransporteFalso {
                cuerpo: cuerpo.to_vec(),
            }
        }
    }

    impl Transportador for TransporteFalso {
        fn get_texto(&self, _url: &str, _cab: &[(&str, &str)]) -> Result<RespuestaHttp, String> {
            Ok(RespuestaHttp {
                status: 200,
                cuerpo: self.cuerpo.clone(),
                ..Default::default()
            })
        }

        fn get_binario(
            &self,
            _url: &str,
            offset: u64,
            _cab: &[(&str, &str)],
        ) -> Result<DescargaBinaria, String> {
            let trozo = if offset > 0 {
                let o = offset as usize;
                self.cuerpo[o.min(self.cuerpo.len())..].to_vec()
            } else {
                self.cuerpo.clone()
            };
            Ok(DescargaBinaria {
                status: if offset > 0 { 206 } else { 200 },
                etag: None,
                last_modified: None,
                content_length: Some(self.cuerpo.len() as u64),
                reader: Box::new(std::io::Cursor::new(trozo)),
            })
        }
    }

    #[test]
    fn la_descarga_verifica_el_sha_y_no_instala_nada_malo() {
        let d = dir_tmp("sha_malo");
        let destino = d.join("binario");
        let cuerpo = b"contenido nuevo".to_vec();
        let t = TransporteFalso::nuevo(&cuerpo);
        let r = descargar_resumible_con("https://example.org/x", &sha_de("otro"), &destino, &t);
        assert!(r.is_err(), "un sha que no cuadra no debe instalar");
        assert!(!destino.exists(), "no debe quedar nada instalado");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn la_descarga_reanudable_continua_desde_el_parcial() {
        let d = dir_tmp("reanuda");
        let destino = d.join("binario");
        let cuerpo = b"0123456789abcdef".to_vec();
        // Se deja un parcial con la mitad.
        let parcial = ruta_parcial(&destino);
        fs::write(&parcial, &cuerpo[..8]).unwrap();
        let t = TransporteFalso::nuevo(&cuerpo);
        let inst = descargar_resumible_con(
            "https://example.org/x",
            &sha_de("0123456789abcdef"),
            &destino,
            &t,
        )
        .expect("debía descargar");
        assert_eq!(inst.size, 16);
        let contenido = fs::read(&destino).unwrap();
        assert_eq!(
            contenido, cuerpo,
            "la reanudación tiene que respectar el orden"
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn la_instalacion_atomica_deja_el_previo() {
        let d = dir_tmp("previo");
        let destino = d.join("megadaemon-core");
        fs::write(&destino, b"version vieja").unwrap();
        let cuerpo = b"version nueva".to_vec();
        let m = Manifest {
            schema_version: SCHEMA_VERSION,
            version: "2.0.0".to_string(),
            released_at: "2026-11-21T10:00:00Z".to_string(),
            assets: vec![Asset::nuevo(
                "megadaemon-core",
                TipoAsset::Binary,
                "any",
                "any",
                "https://example.org/megadaemon-core",
                &sha_de("version nueva"),
                12,
            )],
        };
        let inst = Instalado::nuevo(
            TipoAsset::Binary,
            "megadaemon-core",
            &destino.to_string_lossy(),
        );
        let t = TransporteFalso::nuevo(&cuerpo);
        instalar_con_rollback_con(&inst, &m, &destino, &t).expect("debería instalar");

        assert_eq!(
            fs::read(&destino).unwrap(),
            cuerpo,
            "el nuevo debe estar en su sitio"
        );
        let previo = ruta_previa(&destino);
        assert!(previo.exists(), "debe quedar el .prev");
        assert_eq!(fs::read(&previo).unwrap(), b"version vieja");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn si_el_destino_ya_tiene_el_sha_no_se_descarga_nada() {
        let d = dir_tmp("ya_al_dia");
        let destino = d.join("megadaemon-core");
        let cuerpo = b"contenido".to_vec();
        fs::write(&destino, &cuerpo).unwrap();
        let m = Manifest {
            schema_version: SCHEMA_VERSION,
            version: "2.0.0".to_string(),
            released_at: "2026-11-21T10:00:00Z".to_string(),
            assets: vec![Asset::nuevo(
                "megadaemon-core",
                TipoAsset::Binary,
                "any",
                "any",
                "https://example.org/megadaemon-core",
                &sha_de("contenido"),
                8,
            )],
        };
        let inst = Instalado::nuevo(
            TipoAsset::Binary,
            "megadaemon-core",
            &destino.to_string_lossy(),
        );
        // El transporte falso devolvería error si se usara: no debe usarse.
        struct TransporteQueFalla;
        impl Transportador for TransporteQueFalla {
            fn get_texto(&self, _u: &str, _c: &[(&str, &str)]) -> Result<RespuestaHttp, String> {
                Err("no debería llamarse".to_string())
            }
            fn get_binario(
                &self,
                _u: &str,
                _o: u64,
                _c: &[(&str, &str)],
            ) -> Result<DescargaBinaria, String> {
                Err("no debería llamarse".to_string())
            }
        }
        instalar_con_rollback_con(&inst, &m, &destino, &TransporteQueFalla)
            .expect("con el mismo sha no hay que descargar nada");
        assert_eq!(fs::read(&destino).unwrap(), cuerpo);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn el_rollback_devuelve_el_previo() {
        let d = dir_tmp("rollback");
        let destino = d.join("binario");
        fs::write(&destino, b"viejo").unwrap();
        let previo = ruta_previa(&destino);
        fs::write(&previo, b"viejo").unwrap();
        let restaurado = restaurar_previo(&destino);
        assert!(restaurado);
        assert_eq!(fs::read(&destino).unwrap(), b"viejo");
        let _ = fs::remove_dir_all(&d);
    }

    // ── (e) La tabla unificada: APK + binario + repo ─────────────────────

    #[test]
    fn la_tabla_unificada_junta_apk_binario_y_repo() {
        // APK instalada y con versión nueva disponible.
        let mut apk = Instalado::nuevo(
            TipoAsset::Apk,
            "org.ejemplo.app",
            "/data/app/org.ejemplo.app/base.apk",
        );
        apk.version = "1.0.0".to_string();
        apk.sha256 = sha_de("apk viejo");
        apk.size = 100;

        // Binario ya al día (mismo sha).
        let mut bin = Instalado::nuevo(
            TipoAsset::Binary,
            "megadaemon-core",
            "/data/local/tmp/megadaemon/megadaemon-core",
        );
        bin.version = "13.4.0".to_string();
        bin.sha256 = sha_de("binario");
        bin.size = 200;

        // Repo git con commit distinto.
        let mut repo = Instalado::nuevo(TipoAsset::Repo, "megadaemon", "/data/repos/megadaemon");
        repo.version = "main".to_string();
        repo.signature = Some("1111111".to_string());

        let m = Manifest {
            schema_version: SCHEMA_VERSION,
            version: "13.4.0".to_string(),
            released_at: "2026-11-21T10:00:00Z".to_string(),
            assets: vec![
                Asset::nuevo(
                    "org.ejemplo.app",
                    TipoAsset::Apk,
                    "android",
                    "arm64",
                    "https://example.org/app.apk",
                    &sha_de("apk nuevo"),
                    150,
                ),
                Asset::nuevo(
                    "megadaemon-core",
                    TipoAsset::Binary,
                    "any",
                    "any",
                    "https://example.org/core",
                    &sha_de("binario"),
                    200,
                ),
                {
                    let mut a = Asset::nuevo(
                        "megadaemon",
                        TipoAsset::Repo,
                        "any",
                        "any",
                        "https://example.org/repo.tar.gz",
                        &sha_de("repo"),
                        300,
                    );
                    a.signature = Some("2222222".to_string());
                    a
                },
            ],
        };

        let idx = IndicePaquetes::construir(&[apk, bin, repo], &[m]);
        assert_eq!(idx.filas.len(), 3);

        let apk_fila = idx
            .filas
            .iter()
            .find(|f| f.nombre == "org.ejemplo.app")
            .unwrap();
        assert_eq!(apk_fila.accion, Accion::Actualizar);
        assert_eq!(apk_fila.motivo, Motivo::ShaDistinto);
        assert_eq!(apk_fila.instalado_version, "1.0.0");
        assert_eq!(apk_fila.disponible_size, 150);
        assert_eq!(apk_fila.instalado_size, 100);

        let bin_fila = idx
            .filas
            .iter()
            .find(|f| f.nombre == "megadaemon-core")
            .unwrap();
        assert_eq!(bin_fila.accion, Accion::YaAlDia);
        assert_eq!(bin_fila.motivo, Motivo::MismoSha);

        let repo_fila = idx.filas.iter().find(|f| f.nombre == "megadaemon").unwrap();
        assert_eq!(repo_fila.accion, Accion::Actualizar);

        // La tabla tiene que ser legible y con las columnas pedidas.
        let tabla = idx.tabla();
        assert!(tabla.contains("KIND"));
        assert!(tabla.contains("org.ejemplo.app"));
        assert!(tabla.contains("Actualizar"));
        assert!(tabla.contains("YaAlDia"));
    }

    #[test]
    fn lo_que_esta_disponible_y_no_instalado_aparece_como_no_instalado() {
        let m = manifiesto_ok();
        let idx = IndicePaquetes::construir(&[], &[m]);
        assert_eq!(idx.filas.len(), 1);
        assert_eq!(idx.filas[0].accion, Accion::NoInstalado);
    }

    #[test]
    fn el_indice_serializa_a_json_con_el_resumen() {
        let idx = IndicePaquetes::construir(&[], &[manifiesto_ok()]);
        let v = idx.to_json();
        assert_eq!(v["count"], json!(1));
        assert_eq!(v["resumen"]["no_instalado"], json!(1));
        assert_eq!(v["filas"][0]["accion"], json!("NoInstalado"));
    }

    // ── Fuentes ──────────────────────────────────────────────────────────

    #[test]
    fn las_cuatro_fuentes_dan_manifiestos_con_la_misma_forma() {
        let doc = serde_json::to_value(manifiesto_ok()).unwrap();
        let texto = serde_json::to_string(&doc).unwrap();

        // Directo.
        struct T(String);
        impl Transportador for T {
            fn get_texto(&self, _u: &str, _c: &[(&str, &str)]) -> Result<RespuestaHttp, String> {
                Ok(RespuestaHttp {
                    status: 200,
                    cuerpo: self.0.as_bytes().to_vec(),
                    ..Default::default()
                })
            }
            fn get_binario(
                &self,
                _u: &str,
                _o: u64,
                _c: &[(&str, &str)],
            ) -> Result<DescargaBinaria, String> {
                Err("no".to_string())
            }
        }

        let m1 = Fuente::Directo {
            url: "https://example.org/m.json".to_string(),
        }
        .listar_disponibles_con(&T(texto.clone()))
        .expect("directo");
        assert_eq!(m1.len(), 1);
        assert!(m1[0].validar().is_ok());

        let m2 = Fuente::Canal(ChannelJson {
            url: "https://example.org/channel.json".to_string(),
        })
        .listar_disponibles_con(&T(texto.clone()))
        .expect("canal");
        assert_eq!(m2.len(), 1);

        let repo_doc = json!({ "manifests": [manifiesto_ok()] }).to_string();
        let m3 = Fuente::RepoGit {
            url: "https://github.com/owner/repo".to_string(),
            rama: "main".to_string(),
        }
        .listar_disponibles_con(&T(repo_doc.clone()))
        .expect("repo git");
        assert_eq!(m3.len(), 1);

        let release_doc = json!({
            "tag_name": "v1.2.3",
            "published_at": "2026-11-21T10:00:00Z",
            "assets": [{
                "name": "megadaemon-core-linux-x86_64",
                "browser_download_url": "https://example.org/core",
                "size": 1234,
                "digest": format!("sha256:{}", sha_de("core")),
            }],
        })
        .to_string();
        let m4 = Fuente::Release(GitHubRelease {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            tag: "latest".to_string(),
        })
        .listar_disponibles_con(&T(release_doc.clone()))
        .expect("release");
        assert_eq!(m4.len(), 1);
        assert_eq!(m4[0].version, "1.2.3");
        assert_eq!(m4[0].assets[0].os, "linux");
        assert_eq!(m4[0].assets[0].arch, "x86_64");
        assert_eq!(m4[0].assets[0].name, "megadaemon-core");
    }

    #[test]
    fn la_url_cruda_del_repo_se_deriva_bien() {
        assert_eq!(
            url_raw_de_repo("https://github.com/owner/repo", "main").unwrap(),
            "https://raw.githubusercontent.com/owner/repo/main/"
        );
        assert_eq!(
            url_raw_de_repo("https://github.com/owner/repo.git", "v2").unwrap(),
            "https://raw.githubusercontent.com/owner/repo/v2/"
        );
        assert!(url_raw_de_repo("https://ejemplo.local/repo", "main").is_err());
    }

    // ── Cursores y cuarentena ────────────────────────────────────────────

    #[test]
    fn el_cursor_evita_releer_el_manifiesto() {
        let d = dir_tmp("cursor");
        let estado = d.to_string_lossy().to_string();
        let fuente = Fuente::Directo {
            url: "https://example.org/m.json".to_string(),
        };
        let texto = serde_json::to_string(&manifiesto_ok()).unwrap();

        /// Transporte que responde 200 la primera vez y 304 después, y
        /// **apunta** las cabeceras que recibe.
        struct T {
            cuerpo: String,
            llamadas: std::cell::RefCell<Vec<Vec<(String, String)>>>,
        }
        impl Transportador for T {
            fn get_texto(&self, _u: &str, cab: &[(&str, &str)]) -> Result<RespuestaHttp, String> {
                self.llamadas.borrow_mut().push(
                    cab.iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect(),
                );
                let n = self.llamadas.borrow().len();
                if n == 1 {
                    Ok(RespuestaHttp {
                        status: 200,
                        cuerpo: self.cuerpo.as_bytes().to_vec(),
                        etag: Some("\"v1\"".to_string()),
                        last_modified: Some("Fri, 21 Nov 2026 10:00:00 GMT".to_string()),
                        ..Default::default()
                    })
                } else {
                    Ok(RespuestaHttp {
                        status: 304,
                        ..Default::default()
                    })
                }
            }
            fn get_binario(
                &self,
                _u: &str,
                _o: u64,
                _c: &[(&str, &str)],
            ) -> Result<DescargaBinaria, String> {
                Err("no".to_string())
            }
        }

        let t = T {
            cuerpo: texto.clone(),
            llamadas: std::cell::RefCell::new(Vec::new()),
        };

        let r1 = refrescar_proveedor(&fuente, &estado, &t).expect("primer refresco");
        assert!(!r1.sin_cambios);
        assert_eq!(r1.cursor.etag.as_deref(), Some("\"v1\""));
        assert!(leer_cursores(&estado).cursores.contains_key(&fuente.id()));
        assert!(leer_manifiesto_cache(&estado, &fuente.id()).is_some());

        // Segundo refresco: el servidor dice "sin cambios" ⇒ no se relee.
        let r2 = refrescar_proveedor(&fuente, &estado, &t).expect("segundo refresco");
        assert!(
            r2.sin_cambios,
            "con el cursor no hay que releer el manifiesto"
        );
        assert_eq!(r2.manifiestos.len(), 1);

        // Y la segunda petición **lleva** el cursor (If-None-Match).
        let llamadas = t.llamadas.borrow();
        assert_eq!(llamadas.len(), 2);
        assert!(
            llamadas[1]
                .iter()
                .any(|(k, v)| k == "If-None-Match" && v == "\"v1\""),
            "la segunda petición debe llevar el ETag: {:?}",
            llamadas[1]
        );
        drop(llamadas);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn un_proveedor_que_falla_no_se_reintenta_enseguida() {
        let d = dir_tmp("cuarentena");
        let estado = d.to_string_lossy().to_string();
        let fuente = Fuente::Directo {
            url: "https://example.org/m.json".to_string(),
        };
        struct TFallo;
        impl Transportador for TFallo {
            fn get_texto(&self, _u: &str, _c: &[(&str, &str)]) -> Result<RespuestaHttp, String> {
                Err("se cayó la red".to_string())
            }
            fn get_binario(
                &self,
                _u: &str,
                _o: u64,
                _c: &[(&str, &str)],
            ) -> Result<DescargaBinaria, String> {
                Err("no".to_string())
            }
        }
        let e1 = refrescar_proveedor(&fuente, &estado, &TFallo).unwrap_err();
        assert!(e1.contains("se cayó la red"), "{e1}");
        let fallos = leer_fallos(&estado);
        assert_eq!(fallos.fallos.get(&fuente.id()).map(|f| f.intentos), Some(1));
        // El segundo intento ya no sale a la red: está en cuarentena.
        let e2 = refrescar_proveedor(&fuente, &estado, &TFallo).unwrap_err();
        assert!(e2.contains("cuarentena"), "{e2}");
        let _ = fs::remove_dir_all(&d);
    }

    // ── Utilidades ───────────────────────────────────────────────────────

    #[test]
    fn el_sha_de_fichero_coincide_con_el_de_drive_client() {
        let d = dir_tmp("sha_fichero");
        let f = d.join("datos");
        fs::write(&f, b"hola mundo").unwrap();
        let (_, a) = sha256_de_fichero(&f).unwrap();
        let mut h = Sha256::new();
        h.update(b"hola mundo");
        assert_eq!(a, hex(&h.finalize()));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn las_versiones_se_comparan_de_forma_numerica() {
        use std::cmp::Ordering;
        assert_eq!(comparar_version("1.2.0", "1.10.0"), Ordering::Less);
        assert_eq!(comparar_version("2.0.0", "1.99.99"), Ordering::Greater);
        assert_eq!(comparar_version("1.2.3", "1.2.3"), Ordering::Equal);
    }

    #[test]
    fn el_tipo_de_asset_se_interpreta_desde_texto() {
        assert_eq!(TipoAsset::from_str("apk"), Some(TipoAsset::Apk));
        assert_eq!(TipoAsset::from_str("BINARY"), Some(TipoAsset::Binary));
        assert_eq!(TipoAsset::from_str("repo"), Some(TipoAsset::Repo));
        assert_eq!(TipoAsset::from_str("font"), Some(TipoAsset::Font));
        assert_eq!(TipoAsset::from_str("dict"), Some(TipoAsset::Dict));
        assert_eq!(TipoAsset::from_str("nada"), None);
    }
}
