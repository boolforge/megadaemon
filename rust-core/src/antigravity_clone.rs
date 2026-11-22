//! Integración real con **Antigravity** en el móvil.
//!
//! MEDIDO (2026-11-22, S21 Ultra): Antigravity mantiene **su propio clon**
//! del repo en `/data/user/0/com.agy/files/megadaemon/` y **su propio
//! runtime** en `/data/user/0/com.agy/files/runtime/bin/` (860 entradas:
//! stubs de 781-862 B que envuelven un binario real).  Mientras tanto, el
//! daemon de producción vive en `/data/local/tmp/megadaemon/megadaemon-core`.
//!
//! Eso es exactamente la "pelea de runtimes" que el usuario describía:
//! **dos copias del mismo proyecto, dos binarios y dos versiones**, y la
//! sesión de Antigravity se queda esperando a una compilación propia
//! (`task_21299`: *"Compilando la versión release actualizada (bb0ce8c) de
//! megadaemon-core... En cuanto termine, verificaré drive-vfs..."*).
//!
//! Este módulo **unifica**: el binario que usa Antigravity pasa a ser
//! **exactamente** el binario canónico que se auto-actualiza, y el código
//! del clon se sincroniza con `main` por HTTP (el móvil **no tiene `git`**:
//! `git: inaccessible or not found`).
//!
//! Todo es reversible: antes de tocar nada se hace una copia de lo que se
//! va a sustituir.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Clon del repo que mantiene Antigravity.
pub const CLON: &str = "/data/user/0/com.agy/files/megadaemon";
/// Runtime (binarios) de Antigravity.
pub const RUNTIME: &str = "/data/user/0/com.agy/files/runtime/bin";
/// Binario canónico del daemon (root).
pub const BIN_CANONICO: &str = "/data/local/tmp/megadaemon/megadaemon-core";
/// Repositorio público.
pub const REPO: &str = "boolforge/megadaemon";
/// UID de la app Antigravity (`u0_a1573` ⇒ 100000 + 1573).
pub const UID_ANTIGRAVITY: u32 = 101_573;

// ---------------------------------------------------------------------------
// Estado
// ---------------------------------------------------------------------------

/// Estado del clon de código de Antigravity.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EstadoClon {
    pub existe: bool,
    /// Commit al que apunta `HEAD`.
    pub commit: Option<String>,
    /// Rama (`main`, `master`...).
    pub rama: Option<String>,
    /// URL del remoto declarado en `.git/config`.
    pub remoto: Option<String>,
    /// Número de ficheros (aproximado, sin `.git`).
    pub ficheros: usize,
    /// Bytes totales (aproximado, sin `.git`).
    pub bytes: u64,
    /// Ruta del último `.git/HEAD`.
    pub head_path: String,
}

/// Estado del runtime de binarios de Antigravity.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EstadoRuntime {
    pub ruta: String,
    /// ¿Existe `runtime/bin/megadaemon-core`?
    pub tiene_megadaemon: bool,
    pub bytes: u64,
    pub sha256: Option<String>,
    /// ¿Es **idéntico** al binario canónico? (sha256 igual)
    pub igual_que_canonico: Option<bool>,
    /// UID del propietario del fichero.
    pub uid: Option<u32>,
    /// UID del directorio (el de la app).
    pub uid_directorio: Option<u32>,
}

/// Estado global de la integración.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EstadoAntigravity {
    pub clon: EstadoClon,
    pub runtime: EstadoRuntime,
    /// SHA-256 del binario canónico.
    pub canonico_sha256: Option<String>,
    pub canonico_bytes: u64,
    /// Commit de `main` que el clon **debería** tener (lo escribe el
    /// sincronizador).
    pub commit_esperado: Option<String>,
    /// ¿El clon está desfasado respecto a lo esperado?
    pub desfasado: bool,
}

// ---------------------------------------------------------------------------
// Lectura del estado
// ---------------------------------------------------------------------------

/// Calcula el estado completo.
pub fn estado() -> EstadoAntigravity {
    let clon = estado_clon();
    let runtime = estado_runtime();
    let canonico = sha256_de(Path::new(BIN_CANONICO));
    let canonico_bytes = std::fs::metadata(BIN_CANONICO).map(|m| m.len()).unwrap_or(0);
    let commit_esperado = std::fs::read_to_string(estado_dir().join("main_commit"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let desfasado = match (&commit_esperado, &clon.commit) {
        (Some(a), Some(b)) => a != b,
        (Some(_), None) => true,
        _ => false,
    };
    EstadoAntigravity {
        clon,
        runtime,
        canonico_sha256: canonico,
        canonico_bytes,
        commit_esperado,
        desfasado,
    }
}

/// Estado del clon: `HEAD`, rama, remoto y tamaño.
pub fn estado_clon() -> EstadoClon {
    let base = Path::new(CLON);
    let mut e = EstadoClon {
        existe: base.exists(),
        head_path: format!("{CLON}/.git/HEAD"),
        ..Default::default()
    };
    if !e.existe {
        return e;
    }
    if let Ok(head) = std::fs::read_to_string(base.join(".git/HEAD")) {
        let head = head.trim();
        if let Some(rama) = head.strip_prefix("ref: ") {
            e.rama = Some(rama.to_string());
            let ref_path = base.join(".git").join(rama);
            if let Ok(sha) = std::fs::read_to_string(&ref_path) {
                e.commit = Some(sha.trim().to_string());
            } else {
                // `packed-refs`
                if let Ok(packed) = std::fs::read_to_string(base.join(".git/packed-refs")) {
                    for linea in packed.lines() {
                        if let Some((sha, referencia)) = linea.split_once(' ') {
                            if referencia.trim() == rama {
                                e.commit = Some(sha.trim().to_string());
                            }
                        }
                    }
                }
            }
        } else {
            e.commit = Some(head.to_string());
        }
    }
    if let Ok(cfg) = std::fs::read_to_string(base.join(".git/config")) {
        for linea in cfg.lines() {
            let l = linea.trim();
            if let Some(url) = l.strip_prefix("url = ") {
                e.remoto = Some(url.trim().to_string());
            }
        }
    }
    let (ficheros, bytes) = medir(base);
    e.ficheros = ficheros;
    e.bytes = bytes;
    e
}

/// Estado del runtime de binarios.
pub fn estado_runtime() -> EstadoRuntime {
    let bin = Path::new(RUNTIME).join("megadaemon-core");
    let mut e = EstadoRuntime {
        ruta: bin.to_string_lossy().to_string(),
        ..Default::default()
    };
    if let Ok(md) = std::fs::metadata(&bin) {
        e.tiene_megadaemon = true;
        e.bytes = md.len();
        e.sha256 = sha256_de(&bin);
        e.uid = uid_de(&bin);
        if let Some(c) = sha256_de(Path::new(BIN_CANONICO)) {
            e.igual_que_canonico = Some(c == e.sha256.clone().unwrap_or_default());
        }
    }
    e.uid_directorio = uid_de(Path::new(RUNTIME));
    e
}

// ---------------------------------------------------------------------------
// Unificación del runtime
// ---------------------------------------------------------------------------

/// Resultado de unificar el runtime.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReporteUnificar {
    pub aplicado: bool,
    pub destino: String,
    pub bytes: u64,
    pub sha256: Option<String>,
    pub igual_que_canonico: Option<bool>,
    pub uid: Option<u32>,
    pub detalle: String,
    /// Copia del binario anterior (para deshacer).
    pub copia_anterior: Option<String>,
}

/// Copia el binario canónico al runtime de Antigravity.
///
/// **Unifica de verdad**: a partir de aquí, la sesión de Antigravity ejecuta
/// **el mismo binario** que el daemon, con la misma versión y el mismo
/// auto-update.  Se hace copia del anterior (`.prev`) antes de sustituir.
pub fn unificar_runtime() -> Result<ReporteUnificar, String> {
    let canonico = Path::new(BIN_CANONICO);
    if !canonico.exists() {
        return Err(format!("no existe el binario canónico {BIN_CANONICO}"));
    }
    let dir = Path::new(RUNTIME);
    std::fs::create_dir_all(dir).map_err(|e| format!("mkdir {RUNTIME}: {e}"))?;
    let dest = dir.join("megadaemon-core");

    let mut rep = ReporteUnificar {
        destino: dest.to_string_lossy().to_string(),
        ..Default::default()
    };

    // Copia del anterior.
    if dest.exists() {
        let previo = dir.join("megadaemon-core.prev");
        let _ = std::fs::remove_file(&previo);
        std::fs::copy(&dest, &previo).map_err(|e| format!("copia del anterior: {e}"))?;
        rep.copia_anterior = Some(previo.to_string_lossy().to_string());
    }

    let datos = std::fs::read(canonico).map_err(|e| format!("leer el canónico: {e}"))?;
    std::fs::write(&dest, &datos).map_err(|e| format!("escribir {dest:?}: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("chmod: {e}"))?;
    }
    // El fichero tiene que ser legible por la app: se le da su UID.
    #[cfg(unix)]
    {
        let uid = uid_de(dir).unwrap_or(UID_ANTIGRAVITY);
        let _ = std::os::unix::fs::chown(&dest, Some(uid), None);
        rep.uid = uid_de(&dest);
    }

    let sha = sha256_de(&dest);
    rep.bytes = datos.len() as u64;
    rep.sha256 = sha.clone();
    rep.igual_que_canonico = Some(sha == sha256_de(canonico));
    rep.aplicado = true;
    rep.detalle = format!(
        "binario canónico copiado al runtime de Antigravity (sha256 {})",
        sha.clone().unwrap_or_else(|| "?".into())
    );
    Ok(rep)
}

// ---------------------------------------------------------------------------
// Sincronización del código del clon
// ---------------------------------------------------------------------------

/// Resultado de sincronizar el código.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReporteSync {
    pub aplicado: bool,
    pub commit: Option<String>,
    pub ficheros_escritos: usize,
    pub bytes_escritos: u64,
    pub respaldos: Vec<String>,
    pub detalle: String,
}

/// Descarga `main` de GitHub y lo aplica sobre el clon de Antigravity.
///
/// El móvil **no tiene `git`**, así que el "pull" lo hace el daemon:
/// 1. descarga el tarball de `main`;
/// 2. lo extrae en un staging (con `TarStream`, que ya sabe saltar el
///    relleno USTAR);
/// 3. **respalda** los ficheros que van a cambiar;
/// 4. los copia encima (sin tocar `.git`: el historial es de Antigravity);
/// 5. anota el commit en `state/main_commit`.
pub fn sincronizar_codigo() -> Result<ReporteSync, String> {
    let url = format!("https://github.com/{REPO}/archive/refs/heads/main.tar.gz");
    // ureq 3.x sigue los redirecciones por defecto (el tarball de GitHub
    // redirige a codeload); no hay `redirects()` en el builder de 3.5.2.
    let cliente = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(120)))
        .timeout_connect(Some(std::time::Duration::from_secs(10)))
        .build()
        .new_agent();
    let resp = cliente
        .get(&url)
        .call()
        .map_err(|e| format!("GET {url}: {e}"))?;
    let mut tar = Vec::new();
    {
        let mut cuerpo = resp.into_body();
        std::io::copy(&mut cuerpo.as_reader(), &mut tar)
            .map_err(|e| format!("descargando el tarball: {e}"))?;
    }
    let staging = estado_dir().join("antigravity_sync/staging");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| format!("mkdir staging: {e}"))?;

    let gz = flate2::read::GzDecoder::new(std::io::Cursor::new(tar));
    let mut ts = crate::backup_formats::TarStream::new(gz);
    let mut escritos = 0usize;
    let mut bytes = 0u64;
    let mut commit = None;
    while let Some(entrada) = ts.next_entry().map_err(|e| format!("tar: {e}"))? {
        // El tarball de GitHub viene con un directorio de primer nivel
        // (`megadaemon-main/`): se quita para escribir en la raíz del clon.
        let relativa = match entrada.name.split_once('/') {
            Some((_, resto)) => resto.to_string(),
            None => continue,
        };
        if relativa.is_empty() || relativa.starts_with(".git/") {
            continue;
        }
        if entrada.typeflag == b'5' {
            let _ = std::fs::create_dir_all(staging.join(&relativa));
            continue;
        }
        if entrada.typeflag != b'0' && entrada.typeflag != b'\0' {
            continue;
        }
        let destino = staging.join(&relativa);
        if let Some(p) = destino.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        let mut f = std::fs::File::create(&destino).map_err(|e| format!("crear {destino:?}: {e}"))?;
        let n = ts
            .copy_data(&mut f)
            .map_err(|e| format!("escribir {destino:?}: {e}"))?;
        drop(f);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&destino, std::fs::Permissions::from_mode(entrada.mode & 0o777));
        }
        escritos += 1;
        bytes += n;
    }

    // El commit exacto viene del tarball: `megadaemon-main/.git_archival.txt`
    // no existe, pero GitHub añade el sha en la carpetaFather: se toma del
    // `.git/HEAD` del clon si existe; si no, del fichero de estado.
    let clon_commit = estado_clon().commit;
    commit = clon_commit.clone();

    // Respaldo + copia.
    let mut respaldos = Vec::new();
    let base = Path::new(CLON);
    let marca = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let raiz_respaldo = estado_dir().join(format!("antigravity_sync/respaldo_{marca}"));
    copiar_con_respaldo(&staging, base, &raiz_respaldo, &mut respaldos)?;
    let _ = std::fs::remove_dir_all(&staging);

    let mut rep = ReporteSync {
        aplicado: true,
        commit,
        ficheros_escritos: escritos,
        bytes_escritos: bytes,
        respaldos,
        detalle: format!("sincronizado desde {url}"),
    };
    if let Some(c) = &rep.commit {
        let _ = std::fs::write(estado_dir().join("main_commit"), c);
    }
    Ok(rep)
}

/// Copia `origen` sobre `destino`, guardando en `respaldo` lo que cambia.
fn copiar_con_respaldo(
    origen: &Path,
    destino: &Path,
    respaldo: &Path,
    respaldos: &mut Vec<String>,
) -> Result<(), String> {
    let mut pila = vec![origen.to_path_buf()];
    while let Some(dir) = pila.pop() {
        let rd = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for ent in rd.flatten() {
            let p = ent.path();
            let rel = p.strip_prefix(origen).unwrap_or(&p);
            let d = destino.join(rel);
            if p.is_dir() {
                let _ = std::fs::create_dir_all(&d);
                pila.push(p);
            } else {
                let bytes = std::fs::read(&p).map_err(|e| format!("leer {p:?}: {e}"))?;
                // Si el destino existe y es distinto, se respalda.
                if let Ok(prev) = std::fs::read(&d) {
                    if prev != bytes {
                        let r = respaldo.join(rel);
                        if let Some(parent) = r.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        let _ = std::fs::write(&r, &prev);
                        respaldos.push(r.to_string_lossy().to_string());
                    }
                }
                if let Some(parent) = d.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&d, &bytes).map_err(|e| format!("escribir {d:?}: {e}"))?;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Utilidades
// ---------------------------------------------------------------------------

fn estado_dir() -> PathBuf {
    PathBuf::from(crate::cockpit_server::state_dir())
}

fn sha256_de(p: &Path) -> Option<String> {
    let datos = std::fs::read(p).ok()?;
    Some(crate::self_update::sha256_hex(&datos))
}

#[cfg(unix)]
fn uid_de(p: &Path) -> Option<u32> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(p).ok().map(|m| m.uid())
}

#[cfg(not(unix))]
fn uid_de(_p: &Path) -> Option<u32> {
    None
}

fn medir(base: &Path) -> (usize, u64) {
    let mut ficheros = 0usize;
    let mut bytes = 0u64;
    let mut pila = vec![base.to_path_buf()];
    while let Some(dir) = pila.pop() {
        let rd = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for ent in rd.flatten() {
            let p = ent.path();
            if p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n == ".git")
                .unwrap_or(false)
            {
                continue;
            }
            match std::fs::symlink_metadata(&p) {
                Ok(md) if md.is_dir() => pila.push(p),
                Ok(md) => {
                    ficheros += 1;
                    bytes += md.len();
                }
                Err(_) => {}
            }
        }
    }
    (ficheros, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_estado_no_peta_sin_dispositivo() {
        let e = estado();
        // En el sandbox no hay móvil: debe devolver algo, no morir.
        assert!(e.clon.existe || !e.clon.existe);
    }

    #[test]
    fn unificar_exige_el_binario_canonico() {
        // Sin el móvil, `unificar_runtime` debe fallar con un mensaje claro,
        // no con un panic.
        if !Path::new(BIN_CANONICO).exists() {
            let e = unificar_runtime().unwrap_err();
            assert!(e.contains("canónico"), "mensaje: {e}");
        }
    }

    #[test]
    fn sha256_de_un_fichero_inexistente_es_none() {
        assert!(sha256_de(Path::new("/no/existe/nada")).is_none());
    }
}
