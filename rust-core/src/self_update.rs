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

/// Lanza el hilo de **auto-actualización permanente**.
///
/// DECISIÓN DEL USUARIO (2026-11-22): *"que megadaemon se auto actualice
/// siempre"*.  Antes sólo existía `megadaemon-core update` (manual).
///
/// MECÁNICA (todo con el cliente HTTP propio: el móvil NO tiene curl/wget):
/// 1. espera `intervalo` (con jitter, para no hacer todos los nodos a la vez);
/// 2. `comprobar()` (1 petición al manifiesto, sin descargar el binario);
/// 3. si hay versión nueva ⇒ `aplicar()` (descarga, verifica SHA-256,
///    instalación atómica con `.prev` para el rollback);
/// 4. deja rastro en `state/auto_update.log`.
///
/// SEGURIDAD:
/// * `MD_NO_AUTO_UPDATE=1` desactiva el hilo por completo.
/// * `MD_AUTO_UPDATE_INTERVAL_MIN` cambia el intervalo (por defecto 360 min).
/// * Tras instalar, el daemon **NO se reinicia solo**: sale con código 10
///   (que es lo que el supervisor `service.d` entiende como "hay binario
///   nuevo, relánzame").  Si no hay supervisor, el proceso **no** sale:
///   el binario nuevo queda instalado y se aplica en el próximo reinicio.
pub fn lanzar_auto_actualizador(ruta_bin: std::path::PathBuf, minutos: u64) {
    if std::env::var("MD_NO_AUTO_UPDATE").ok().as_deref() == Some("1") {
        return;
    }
    let intervalo = std::env::var("MD_AUTO_UPDATE_INTERVAL_MIN")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(minutos.max(30));
    std::thread::Builder::new()
        .name("md-auto-update".to_string())
        .spawn(move || loop {
            // Jitter de hasta 5 minutos: evita que toda la flota golpee a la
            // vez la API de GitHub en el mismo instante.
            let jitter = (ruta_bin
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.subsec_nanos() as u64)
                .unwrap_or(0))
                % 300;
            std::thread::sleep(std::time::Duration::from_secs(jitter * 60 / 300));
            let estado = comprobar(&ruta_bin);
            let hay = matches!(estado, Estado::Actualizable { .. });
            if hay {
                match aplicar(&ruta_bin) {
                    Ok(r) if r.instalado => {
                        registrar(&format!(
                            "instalado {} (previo {:?}) el {}",
                            r.short,
                            r.previo,
                            ahora()
                        ));
                        // Código 10 = "se instaló una versión nueva" => el
                        // supervisor relanza el daemon con el binario nuevo.
                        // Si no hay supervisor, NO se sale: el binario queda
                        // instalado para el próximo reinicio.
                        if supervisor_presente() {
                            std::process::exit(10);
                        }
                    }
                    Ok(_) => {}
                    Err(e) => registrar(&format!("fallo al actualizar: {e}")),
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(intervalo * 60));
        })
        .ok();
}

/// ¿Hay un supervisor que relance el daemon?  (Magisk `service.d` o el
/// proceso `99-megadaemon.sh`).
fn supervisor_presente() -> bool {
    if std::env::var("MD_NO_AUTO_UPDATE_RESTART").ok().as_deref() == Some("1") {
        return false;
    }
    if std::path::Path::new("/data/adb/service.d/99-megadaemon.sh").exists() {
        return true;
    }
    // En el PC puede ser otro supervisor: se busca el binario en la lista.
    std::process::Command::new("sh")
        .args(["-c", "ps -A -o args 2>/dev/null | grep -c '99-megadaemon' || true"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<u64>().ok())
        .map(|n| n > 0)
        .unwrap_or(false)
}

/// Escribe una línea en `state/auto_update.log` (best effort).
fn registrar(linea: &str) {
    let sd = crate::cockpit_server::state_dir();
    let dir = std::path::Path::new(&sd);
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let ruta = dir.join("auto_update.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&ruta)
    {
        use std::io::Write;
        let _ = writeln!(f, "[{}] {linea}", ahora());
    }
}

fn ahora() -> String {
    crate::util_text::now_iso()
}

/// SHA-256 de un fichero, **en streaming** (no lo carga entero en memoria:
/// el binario del móvil son 41 MB y el sandbox tiene 3 GB).
pub fn sha256_de_fichero(ruta: &Path) -> Option<String> {
    use sha2::{Digest, Sha256};
    let mut f = std::fs::File::open(ruta).ok()?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = std::io::Read::read(&mut f, &mut buf).ok()?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Some(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// SHA-256 que publica la release para su asset (`<asset>.sha256`).
pub fn sha_esperado_del_asset(m: &Manifiesto) -> Option<String> {
    let url = format!("{}/{}.sha256", url_release(), m.asset);
    descargar_texto(&url).ok().and_then(|t| parsear_sha256(&t))
}

/// Decisión **por hash de artefacto**: ¿el binario instalado es exactamente
/// el que publica la release?
///
/// MEDIDO (2026-11-23, móvil): la comparación anterior era "sha256 del
/// FICHERO" contra "commit del manifiesto" (`c8ab17e`).  Son identificadores
/// distintos: NUNCA coinciden, así que el daemon se reinstalaba a sí mismo
/// en bucle (41 MB por vuelta) y el supervisor lo relanzaba sin parar.
/// Ahora se comparan los dos sha256 **del mismo artefacto**.
pub fn decidir_por_sha(instalado: Option<&str>, esperado: Option<&str>) -> Estado {
    match (instalado, esperado) {
        (Some(i), Some(e)) if i.eq_ignore_ascii_case(e) => Estado::Actual,
        (Some(i), Some(e)) => Estado::Actualizable {
            short: format!(
                "{} -> {}",
                &i[..7.min(i.len())],
                &e[..7.min(e.len())]
            ),
        },
        (None, _) => Estado::Desconocido {
            motivo: "no se pudo calcular el sha256 del binario instalado".to_string(),
        },
        (_, None) => Estado::Desconocido {
            motivo: "la release no publica el sha256 del asset".to_string(),
        },
    }
}

/// Comprobación **sin red** (test): el binario instalado es el de la release.

/// Comprueba si hay una versión nueva.  **No descarga el binario.**
pub fn comprobar(ruta_bin: &Path) -> Estado {
    let m = match manifiesto() {
        Ok(m) => m,
        Err(e) => return Estado::Desconocido { motivo: e },
    };
    // MEDIDO (2026-11-23, móvil): antes se comparaba el sha256 del FICHERO
    // con el COMMIT del manifiesto ('c8ab17e') => nunca coincidían => el
    // daemon se reinstalaba a sí mismo en bucle (41 MB por vuelta) y el
    // supervisor lo relanzaba sin parar (el móvil se caía cada 30 min).
    // Ahora se comparan los dos sha256 DEL MISMO artefacto: el del binario
    // instalado y el que publica la release en '<asset>.sha256'.
    // Si el .sha256 no se puede leer => Desconocido => NO se toca el móvil.
    let esperado = match sha_esperado_del_asset(&m) {
        Some(s) => s,
        None => {
            return Estado::Desconocido {
                motivo: format!("no se pudo leer el .sha256 de {} en la release", m.asset),
            }
        }
    };
    let instalado = sha256_de_fichero(ruta_bin);
    decidir_por_sha(instalado.as_deref(), Some(esperado.as_str()))
}

/// Descarga, verifica e instala la última release.
///
/// 1. manifiesto;
/// 2. asset + su `.sha256`;
/// 3. **verificación OBLIGATORIA**: el SHA del asset debe coincidir con el
///    `.sha256`; si el `.sha256` no se puede leer o no cuadra, **no se
///    instala nada** y el binario actual sigue intacto (MEDIDO 2026-11-23:
///    antes se instalaba sin verificar cuando faltaba el `.sha256`);
/// 4. instalación atómica: `.new` → verificar → renombrar encima del
///    destino (el anterior queda copiado en `.prev` para el rollback).
pub fn aplicar(ruta_bin: &Path) -> Result<Resultado, String> {
    let m = manifiesto()?;
    let base = url_release();
    let url_asset = format!("{base}/{}", m.asset);
    let url_sha = format!("{url_asset}.sha256");

    let datos = descargar_bytes(&url_asset)?;

    // La verificación del SHA-256 es **obligatoria**.
    //
    // MEDIDO (2026-11-23, auditoría de calidad): antes, si el `.sha256` no se
    // podía leer, se instalaba **sin verificar** (el propio comentario lo
    // admitía: "el cálculo se hace sobre lo descargado y se acepta (se
    // avisa)") ⇒ un `.sha256` caído por un fallo de red (o una descarga
    // manipulada) instalaba un binario cualquiera en el móvil y el informe
    // decía "SIN .sha256: sin verificar".  Eso contradice la garantía que
    // promete el propio módulo ("verificación obligatoria: si el SHA-256 no
    // cuadra, no se instala nada y el binario actual sigue intacto").
    // `comprobar()` ya devuelve `Desconocido` cuando no hay `.sha256` (o
    // sea: no se toca el móvil); aquí se aplica lo mismo.
    //
    // La release publica el `.sha256` del asset versionado (y del normal):
    // ver `.github/workflows/build.yml` (los dos `sha256sum`).
    let texto_sha = descargar_texto(&url_sha).map_err(|e| {
        format!(
            "no se pudo leer el .sha256 de {} ({e}): no se instala nada",
            m.asset
        )
    })?;
    let esperado = parsear_sha256(&texto_sha).ok_or_else(|| {
        format!(
            "el .sha256 de {} no contiene un sha256 válido: no se instala nada",
            m.asset
        )
    })?;
    let calculado = sha256_hex(&datos);
    if esperado != calculado {
        return Err(format!(
            "SHA-256 NO coincide: esperaba {esperado}, obtuve {calculado}. No se instala nada"
        ));
    }

    // ¿El binario instalado ES ya el asset de la release?  Se comparan los
    // dos sha256 DEL MISMO artefacto.  Antes se comparaba el sha del fichero
    // con el commit => nunca coincidía => se reinstalaba SIEMPRE (el bucle
    // de 41 MB medido el 2026-11-23).
    if let Some(h) = sha256_de_fichero(ruta_bin) {
        if h.eq_ignore_ascii_case(&esperado) {
            return Ok(Resultado {
                instalado: false,
                short: m.short,
                previo: None,
                detalle: "ya estaba instalado (sha256 igual al de la release)".to_string(),
            });
        }
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

    // Se deja el short publicado para que /health pueda decir qué build es
    // (si el binario no lo trae embebido).
    {
        let ruta = crate::cockpit_server::state_dir() + "/build_short";
        let _ = std::fs::write(&ruta, format!("{}\n", m.short));
    }

    Ok(Resultado {
        instalado: true,
        short: m.short.clone(),
        previo: if previo.exists() { Some(previo) } else { None },
        detalle: format!(
            "actualizado a {} ({} B) (SHA-256 verificado)",
            m.short,
            datos.len(),
        ),
    })
}

/// SHA-256 del binario **en ejecución**.
///
/// MEDIDO: el móvil sirve `/health` sin credencial, así que publicar aquí el
/// hash permite comprobar (desde fuera y sin key) si el móvil ya corre el
/// build nuevo.  Es la forma de verificar una auto-actualización.
pub fn build_sha() -> String {
    static CACHE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| std::fs::read(p).ok())
                .map(|d| sha256_hex(&d))
                .unwrap_or_else(|| "desconocido".to_string())
        })
        .clone()
}

/// Commit con el que se compiló este binario.
///
/// Lo inyecta el CI como `MEGADAEMON_BUILD_SHA` en el momento de compilar
/// (`option_env!` lo resuelve en tiempo de compilación).  Si el binario se
/// compiló a mano y sin esa variable, devuelve "desconocido" y el resto del
/// sistema sigue funcionando (sólo se pierde esta comprobación).
pub fn build_commit() -> &'static str {
    option_env!("MEGADAEMON_BUILD_SHA").unwrap_or("desconocido")
}

/// Información del build en servicio: hash del binario + short del commit.
///
/// MEDIDO (2026-11-23): `short` salía SIEMPRE a null porque el fichero
/// `state/build_short` sólo lo escribía el auto-actualizador y nunca se
/// creaba.  Ahora se prioriza el commit EMBEBIDO en el binario (el CI lo
/// inyecta al compilar) y, si no lo hay, el que dejó el auto-actualizador.
pub fn build_info() -> Value {
    let commit = build_commit();
    let short_embebido = if commit == "desconocido" {
        None
    } else {
        Some(commit[..7.min(commit.len())].to_string())
    };
    let ruta = crate::cockpit_server::state_dir() + "/build_short";
    let publicado = std::fs::read_to_string(&ruta).ok().map(|s| s.trim().to_string());
    json!({
        "sha256": build_sha(),
        "short": short_embebido.or(publicado),
        "commit": commit,
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
    fn la_decision_por_sha_no_confunde_el_commit_con_el_hash() {
        // REGRESION (2026-11-23): el movil se reinstalaba en bucle porque se
        // comparaba el sha256 del FICHERO con el COMMIT del manifiesto.
        let sha_fichero = "fa27914a33a29b8bd84cbf74a0aa84616609a401894225f7284e6c0c47b1c67c";
        let commit = "c8ab17e";
        // Comparar commit contra hash: NUNCA es Actual (que era el bug).
        assert!(!matches!(
            decidir_por_sha(Some(sha_fichero), Some(commit)),
            Estado::Actual
        ));
        // Comparar hash contra hash: si son el mismo artefacto, Actual.
        assert!(matches!(
            decidir_por_sha(Some(sha_fichero), Some(sha_fichero)),
            Estado::Actual
        ));
        // Hash distinto => Actualizable, y el mensaje dice los dos.
        let otro = "917f4410403896a34f53f9a844c413cd66ae47180079c047c571b836db777e91";
        match decidir_por_sha(Some(sha_fichero), Some(otro)) {
            Estado::Actualizable { short } => {
                assert!(short.contains("fa27914"), "{short}");
                assert!(short.contains("917f441"), "{short}");
            }
            otro_estado => panic!("se esperaba Actualizable, came {otro_estado:?}"),
        }
    }

    #[test]
    fn sin_sha_esperado_no_se_toca_nada() {
        // Fail-safe: si la release no publica el .sha256, el estado es
        // Desconocido => el auto-actualizador NO descarga ni instala.
        assert!(matches!(
            decidir_por_sha(Some("aa"), None),
            Estado::Desconocido { .. }
        ));
        assert!(matches!(
            decidir_por_sha(None, Some("aa")),
            Estado::Desconocido { .. }
        ));
    }

    #[test]
    fn el_sha_de_un_fichero_es_el_mismo_que_el_del_contenido() {
        let dir = std::env::temp_dir().join(format!("md-sha-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("datos.bin");
        let contenido: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&f, &contenido).unwrap();
        let por_fichero = sha256_de_fichero(&f).unwrap();
        let por_memoria = sha256_hex(&contenido);
        assert_eq!(por_fichero, por_memoria);
        let _ = std::fs::remove_file(&f);
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
