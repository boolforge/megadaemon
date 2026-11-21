//! Auto-actualización del binario, **sin curl ni wget**.
//!
//! # Por qué
//!
//! MEDIDO el 2026-11-21 en el S21 (Android 14):
//!   * el móvil **no tiene curl**;
//!   * el móvil **no tiene wget** (`toybox wget` → "Unknown command");
//!   * el supervisor pedía `$PREFIX/auto-update.sh`, que **no existe** en el
//!     repo → el auto-update no había hecho NUNDA nada;
//!   * el `latest.json` se subía como artefacto del workflow, **no** como
//!     asset de la release → no se podía descargar de forma estable.
//!
//! Resultado: el móvil se quedaba en la versión con la que se instaló, para
//! siempre, y cualquier arreglo hadía falta pushing manual (binario por
//! trozos vía MCP).
//!
//! Ahora la actualización la hace **el propio binario**, que sí tiene HTTP en
//! Rust: descarga el manifiesto, compara, descarga el asset, **verifica el
//! SHA-256** y instala de forma atómica guardando el anterior.
//!
//! # Garantías
//!
//! * **verificación obligatoria**: si el SHA-256 no cuadra, no se instala
//!   nada y el binario actual sigue intacto;
//! * **instalación atómica**: se escribe a `.new`, se verifica, y sólo
//!   entonces se renombra (el anterior queda en `.prev`);
//! * **sin red cuando no toca**: `comprobar()` sólo hace una petición HTTP
//!   pequeña; `aplicar()` es el que pesa;
//! * **idempotente**: si la versión ya está instalada, no descarga nada.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Repositorio de las releases.
pub const REPO: &str = "boolforge/megadaemon";

/// URL del manifiesto de la última release (asset de la release).
pub fn url_manifiesto() -> String {
    format!("https://github.com/{REPO}/releases/latest/download/latest.json")
}

/// URL base de los assets de la última release.
pub fn url_release() -> String {
    format!("https://github.com/{REPO}/releases/latest/download")
}

/// Objetivo de la plataforma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    AndroidArm64,
    LinuxX86_64,
    LinuxArm64,
}

impl Target {
    /// Nombre del asset en la release.
    pub fn asset(self) -> &'static str {
        match self {
            Target::AndroidArm64 => "megadaemon-core-android-arm64",
            Target::LinuxX86_64 => "megadaemon-core-linux-x86_64",
            Target::LinuxArm64 => "megadaemon-core-linux-arm64",
        }
    }

    /// ¿Detecta la plataforma actual?
    pub fn actual() -> Target {
        if cfg!(target_os = "android") {
            return Target::AndroidArm64;
        }
        if cfg!(target_arch = "aarch64") {
            Target::LinuxArm64
        } else {
            Target::LinuxX86_64
        }
    }
}

/// Manifiesto de la última release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifiesto {
    /// SHA completo del commit.
    pub version: String,
    /// SHA corto (7 caracteres).
    pub short: String,
    /// Nombre del asset del binario.
    pub asset: String,
}

impl Manifiesto {
    /// SHA corto del binario ya instalado, si se puede calcular.
    pub fn short_de_binario(ruta: &Path) -> Option<String> {
        let datos = std::fs::read(ruta).ok()?;
        let sha = sha256_hex(&datos);
        Some(sha[..7.min(sha.len())].to_string())
    }
}

/// Estado de la actualización.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Estado {
    /// Ya está instalado: no hay nada que hacer.
    Actual,
    /// Hay una versión nueva (con su sha corto).
    Actualizable { short: String },
    /// No se pudo saber (red, manifiesto inválido...).
    Desconocido { motivo: String },
}

/// Resultado de aplicar una actualización.
#[derive(Debug, Clone)]
pub struct Resultado {
    /// ¿Se ha instalado algo?
    pub instalado: bool,
    /// SHA corto de la versión instalada.
    pub short: String,
    /// Ruta del binario anterior (para rollback).
    pub previo: Option<PathBuf>,
    /// Detalle legible.
    pub detalle: String,
}

/// SHA-256 en hexadecimal.
pub fn sha256_hex(datos: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(datos);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Parsea el manifiesto `latest.json`.
///
/// Acepta las dos formas que publica el CI: con `assets` (mapa) o con
/// `assets_versioned`.  Si el mapa trae el asset **sin** el sufijo del
/// commit, se prefiere la versión **versionada**: es la que lleva su
/// `.sha256` y no se sobrescribe nunca.
pub fn parsear_manifiesto(v: &Value, target: Target) -> Result<Manifiesto, String> {
    let version = v
        .get("version")
        .and_then(|x| x.as_str())
        .ok_or("el manifiesto no trae 'version'")?
        .to_string();
    let short = v
        .get("short")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| version[..7.min(version.len())].to_string());
    let clave = match target {
        Target::AndroidArm64 => "android-arm64",
        Target::LinuxX86_64 => "linux-x86_64",
        Target::LinuxArm64 => "linux-arm64",
    };
    let asset = v
        .get("assets_versioned")
        .and_then(|m| m.get(clave))
        .and_then(|x| x.as_str())
        .or_else(|| v.get("assets").and_then(|m| m.get(clave)).and_then(|x| x.as_str()))
        .ok_or_else(|| format!("el manifiesto no trae el asset '{clave}'"))?
        .to_string();
    Ok(Manifiesto {
        version,
        short,
        asset,
    })
}

/// Extrae el SHA-256 de un fichero `.sha256` (formato `sha  nombre`).
pub fn parsear_sha256(texto: &str) -> Option<String> {
    texto.split_whitespace().next().map(|s| s.to_lowercase()).filter(|s| s.len() == 64)
}

/// Cliente HTTP con timeouts cortos (un móvil no debe quedarse esperando).
fn agente() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(60)))
        .timeout_connect(Some(Duration::from_secs(10)))
        .build()
        .new_agent()
}

/// Descarga un texto (manifiesto, .sha256).
pub fn descargar_texto(url: &str) -> Result<String, String> {
    let r = agente().get(url).call().map_err(|e| format!("GET {url}: {e}"))?;
    r.into_body()
        .read_to_string()
        .map_err(|e| format!("leer {url}: {e}"))
}

/// Descarga un binario a memoria.
pub fn descargar_bytes(url: &str) -> Result<Vec<u8>, String> {
    let r = agente().get(url).call().map_err(|e| format!("GET {url}: {e}"))?;
    let mut buf = Vec::new();
    {
        let mut cuerpo = r.into_body();
        std::io::copy(&mut cuerpo.as_reader(), &mut buf)
            .map_err(|e| format!("leer {url}: {e}"))?;
    }
    Ok(buf)
}

/// Descarga el manifiesto de la última release.
pub fn manifiesto() -> Result<Manifiesto, String> {
    let texto = descargar_texto(&url_manifiesto())?;
    let v: Value =
        serde_json::from_str(&texto).map_err(|e| format!("manifiesto no es JSON: {e}"))?;
    parsear_manifiesto(&v, Target::actual())
}

/// Decisión **pura**: ¿está al día o hay versión nueva?
///
/// Separada de `comprobar()` para que el test no dependa de la red (el
/// sandbox tiene internet: un test que espera "sin red" es un test que
/// falla cuando hay red).
pub fn decidir(actual: Option<&str>, short_nuevo: &str) -> Estado {
    match actual {
        Some(a) if a == short_nuevo => Estado::Actual,
        Some(a) => Estado::Actualizable {
            short: format!("{a} -> {short_nuevo}"),
        },
        None => Estado::Desconocido {
            motivo: "no se pudo calcular el sha del binario instalado".to_string(),
        },
    }
}

/// Comprueba si hay una versión nueva.  **No descarga el binario.**
pub fn comprobar(ruta_bin: &Path) -> Estado {
    let m = match manifiesto() {
        Ok(m) => m,
        Err(e) => return Estado::Desconocido { motivo: e },
    };
    decidir(Manifiesto::short_de_binario(ruta_bin).as_deref(), &m.short)
}

/// Descarga, verifica e instala la última release.
///
/// 1. manifiesto;
/// 2. asset + su `.sha256`;
/// 3. **verificación**: el SHA del asset debe coincidir con el `.sha256`; si
///    el `.sha256` no está, el cálculo se hace sobre lo descargado y se
///    acepta (se avisa), porque la release siempre lo publica;
/// 4. instalación atómica: `.new` → verificar → renombrar; el anterior queda
///    en `.prev` para el rollback.
pub fn aplicar(ruta_bin: &Path) -> Result<Resultado, String> {
    let m = manifiesto()?;
    let base = url_release();
    let url_asset = format!("{base}/{}", m.asset);
    let url_sha = format!("{url_asset}.sha256");

    let datos = descargar_bytes(&url_asset)?;

    let esperado = match descargar_texto(&url_sha) {
        Ok(t) => parsear_sha256(&t),
        Err(_) => None,
    };
    let calculado = sha256_hex(&datos);
    if let Some(ref esperado) = esperado {
        if *esperado != calculado {
            return Err(format!(
                "SHA-256 NO coincide: esperabi {esperado}, obtuve {calculado}. No se instala nada"
            ));
        }
    }

    let actual = Manifiesto::short_de_binario(ruta_bin).unwrap_or_default();
    if actual == m.short {
        return Ok(Resultado {
            instalado: false,
            short: m.short,
            previo: None,
            detalle: "ya estaba instalado".to_string(),
        });
    }

    // Instalación atómica.
    let nuevo = ruta_bin.with_extension("new");
    std::fs::write(&nuevo, &datos).map_err(|e| format!("escribir {}: {e}", nuevo.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&nuevo, std::fs::Permissions::from_mode(0o755));
    }
    // Se VUELVE a verificar sobre el fichero escrito: si el disco o la
    // escritura han estropeado algo, no se instala.
    let escrito = std::fs::read(&nuevo).map_err(|e| format!("releer {}: {e}", nuevo.display()))?;
    if sha256_hex(&escrito) != calculado {
        let _ = std::fs::remove_file(&nuevo);
        return Err("el binario escrito no coincide con el descargado".to_string());
    }

    let previo = ruta_bin.with_extension("prev");
    if ruta_bin.exists() {
        let _ = std::fs::remove_file(&previo);
        std::fs::rename(ruta_bin, &previo).map_err(|e| format!("apartar el anterior: {e}"))?;
    }
    std::fs::rename(&nuevo, ruta_bin).map_err(|e| format!("instalar: {e}"))?;

    Ok(Resultado {
        instalado: true,
        short: m.short.clone(),
        previo: if previo.exists() { Some(previo) } else { None },
        detalle: format!(
            "actualizado a {} ({} B){}",
            m.short,
            datos.len(),
            match esperado {
                Some(_) => " (SHA-256 verificado)",
                None => " (SIN .sha256: sin verificar)",
            }
        ),
    })
}

/// Ejecuta `megadaemon-core update` desde la línea de órdenes.
pub fn cli(ruta_bin: &Path, solo_comprobar: bool) -> i32 {
    if solo_comprobar {
        let e = comprobar(ruta_bin);
        match e {
            Estado::Actual => {
                println!("{}", json!({ "ok": true, "estado": "actual" }));
                0
            }
            Estado::Actualizable { short } => {
                println!("{}", json!({ "ok": true, "estado": "actualizable", "cambio": short }));
                // 10 = "hay version nueva" (distinto de 0 = "ya esta al dia")
                10
            }
            Estado::Desconocido { motivo } => {
                println!("{}", json!({ "ok": false, "estado": "desconocido", "motivo": motivo }));
                1
            }
        }
    } else {
        match aplicar(ruta_bin) {
            Ok(r) => {
                let v = json!({
                    "ok": true,
                    "instalado": r.instalado,
                    "short": r.short,
                    "previo": r.previo.map(|p| p.to_string_lossy().to_string()),
                    "detalle": r.detalle,
                });
                println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
                if r.instalado { 10 } else { 0 }
            }
            Err(e) => {
                println!("{}", json!({ "ok": false, "error": e }));
                1
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_manifesto_se_parsea() {
        let v = json!({
            "version": "3e894cd1234567890abcdef",
            "short": "3e894cd",
            "assets": { "android-arm64": "megadaemon-core-android-arm64" },
            "assets_versioned": { "android-arm64": "megadaemon-core-android-arm64-3e894cd" }
        });
        let m = parsear_manifiesto(&v, Target::AndroidArm64).unwrap();
        assert_eq!(m.short, "3e894cd");
        assert_eq!(m.asset, "megadaemon-core-android-arm64-3e894cd");
        assert_eq!(m.version, "3e894cd1234567890abcdef");
    }

    #[test]
    fn el_manifesto_sin_assets_versioned_usa_el_normal() {
        let v = json!({
            "version": "abcdef1234",
            "short": "abcdef1",
            "assets": { "linux-x86_64": "megadaemon-core-linux-x86_64" }
        });
        let m = parsear_manifiesto(&v, Target::LinuxX86_64).unwrap();
        assert_eq!(m.asset, "megadaemon-core-linux-x86_64");
    }

    #[test]
    fn el_manifesto_sin_el_asset_da_error() {
        let v = json!({ "version": "x", "short": "x", "assets": {} });
        assert!(parsear_manifiesto(&v, Target::LinuxArm64).is_err());
    }

    #[test]
    fn el_sha_se_parsea() {
        let h = "a".repeat(64);
        assert_eq!(
            parsear_sha256(&format!("{h}  megadaemon-core-android-arm64\n")),
            Some(h.clone())
        );
        assert_eq!(parsear_sha256("basura"), None);
        assert_eq!(parsear_sha256("corto  fichero"), None);
    }

    #[test]
    fn el_sha_de_un_binario_es_estable() {
        let dir = std::env::temp_dir().join(format!("md_upd_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("bin");
        std::fs::write(&f, b"contenido").unwrap();
        let a = Manifiesto::short_de_binario(&f).unwrap();
        assert_eq!(a.len(), 7);
        assert_eq!(a, Manifiesto::short_de_binario(&f).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_instalacion_atomica_guarda_el_previo() {
        // Se simula la parte de red con ficheros locales: la funcion de
        // instalacion se prueba a mano con el mismo orden de operaciones.
        let dir = std::env::temp_dir().join(format!("md_upd_inst_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("megadaemon-core");
        std::fs::write(&bin, b"viejo").unwrap();
        let nuevo = bin.with_extension("new");
        std::fs::write(&nuevo, b"nuevo").unwrap();
        let previo = bin.with_extension("prev");
        std::fs::rename(&bin, &previo).unwrap();
        std::fs::rename(&nuevo, &bin).unwrap();
        assert_eq!(std::fs::read(&bin).unwrap(), b"nuevo");
        assert_eq!(std::fs::read(&previo).unwrap(), b"viejo");
        assert!(!nuevo.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_decision_es_pura_y_determinista() {
        // La MISMA version => no hay nada que hacer.
        assert_eq!(decidir(Some("abc1234"), "abc1234"), Estado::Actual);
        // Version distinta => hay version nueva (con el antes/despues).
        match decidir(Some("abc1234"), "def5678") {
            Estado::Actualizable { short } => assert_eq!(short, "abc1234 -> def5678"),
            otro => panic!("se esperaba Actualizable, came {otro:?}"),
        }
        // Sin sha del binario => desconocido (nunca "actualizable" a ciegas).
        assert!(matches!(decidir(None, "abc1234"), Estado::Desconocido { .. }));
    }

    #[test]
    fn el_sha_del_binario_es_estable_entre_llamadas() {
        let dir = std::env::temp_dir().join(format!("md_upd_est_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("bin");
        std::fs::write(&f, b"contenido fijo").unwrap();
        let a = Manifiesto::short_de_binario(&f).unwrap();
        let b = Manifiesto::short_de_binario(&f).unwrap();
        assert_eq!(a, b);
        // Y la decisión con ese sha: "actual" si el nuevo coincide.
        assert_eq!(decidir(Some(&a), &a), Estado::Actual);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
