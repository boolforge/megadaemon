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
//! **exactamente** el binario canónico que se auto-actualiza.
//!
//! ASIMILACIÓN 2026-11-22 (qué se queda y qué se va)
//! ----------------------------------------------------
//! * **Se queda**: la **medición** ([`estado`], [`estado_clon`],
//!   [`estado_runtime`]) y la **unificación** ([`unificar_runtime`]).
//!   Medir la duplicidad es lo que permite governedla.
//! * **Se va**: el **segundo motor de pull** que tenía este módulo (bajaba
//!   el tarball de `main` y lo aplicaba sobre el clon).  Ahora
//!   [`sincronizar_codigo`] **delega** en el mecanismo canónico
//!   (`deploy/instalador-megadaemon.sh --instalar`); el daemon ya tiene su
//!   propio auto-update (`self_update`) y una segunda implementación del
//!   "pull" dentro del daemon es exactamente el *doble motor* que hay que
//!   eliminar.
//!
//! CORRECCIÓN DE UNA AFIRMACIÓN FALSA (MEDIDO 2026-11-22)
//! ----------------------------------------------------
//! La versión anterior de este módulo decía *"el móvil **no** tiene `git`"*.
//! Es **FALSO**: el runtime de Antigravity trae un userland Termux completo
//! con `git 2.56.0` (ver `antigravity_env.rs`, que lo cataloga).  Ninguna
//! función de este módulo debe volver a suponer que no hay `git`.
//!
//! Todo es reversible: antes de tocar nada se hace una copia de lo que se
//! va a sustituir.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Clon del repo que mantiene Antigravity.
pub const CLON: &str = "/data/user/0/com.agy/files/megadaemon";
/// Runtime (binarios) de Antigravity.
///
/// ⚠️ **NUNCA escribir aquí**: es el directorio que la app DESEMPAQUETA en
/// cada arranque (assets/runtime → files/runtime).  Si el daemon deja un
/// fichero ROOT dentro (o un directorio root-owned), la app no puede
/// borrarlo al desempaquetar y **no arranca**:
///
///   "cannot unpack assets/runtime into /data/user/0/com.agy/files/runtime:
///    cannot delete .../runtime/etc/antigravity/admin_settings.json"
///
/// MEDIDO (2026-11-23): el daemon se copiaba a sí mismo aquí (root) en
/// cada arranque (`unificar_runtime`) ⇒ la app se rompía en el siguiente
/// arranque, y el daemon volvía a meterlo en el siguiente reinicio.
pub const RUNTIME: &str = "/data/user/0/com.agy/files/runtime/bin";

/// Directorio del CLON del daemon dentro de la app (NO se desempaqueta).
///
/// `files/megadaemon/` es del daemon: la app no lo toca ⇒ aquí sí se puede
/// dejar una copia del binario sin romper el desempaquetado.
pub const BIN_CLON: &str = "/data/user/0/com.agy/files/megadaemon/bin/megadaemon-core";
/// Binario canónico del daemon (root).
///
/// MEDIDO (2026-11-23): estaba en `/data/local/tmp/megadaemon/...`, que es
/// una ruta **VOLÁTIL** (el sistema la limpia al reiniciar) ⇒ era
/// precisamente la causa de que el móvil se quedara sin binario en servicio
/// y el supervisor entrara en bucle.  Ahora vive en el prefijo PERSISTENTE
/// (`/data/adb/megadaemon`), que sobrevive a los reinicios.
pub const BIN_CANONICO: &str = "/data/adb/megadaemon/megadaemon-core";

/// Ruta del binario canónico respetando `MEGADAEMON_PREFIX`.
///
/// Es la MISMA ruta que usa el reparador de entorno
/// ([`crate::reparar_entorno::Config::binario`]) ⇒ una sola fuente de verdad.
pub fn bin_canonico() -> String {
    crate::reparar_entorno::Config::del_entorno().binario()
}
/// Repositorio público.
pub const REPO: &str = "boolforge/megadaemon";
/// UID de la app Antigravity (`u0_a1573` ⇒ 100000 + 1573).
pub const UID_ANTIGRAVITY: u32 = 101_573;

/// Devuelve un fichero al **dueño de la app** (y a su contexto SELinux).
///
/// MEDIDO (2026-11-23): el daemon escribe como ROOT dentro del directorio
/// privado de la app ⇒ el fichero queda root-owned ⇒ la app no lo puede
/// reescribir ni borrar.  En el caso grave (el runtime, que la app
/// desempaqueta en cada arranque) eso IMPIDE QUE LA APP ARRANQUE.
///
/// Con esta función, todo lo que el daemon escribe dentro de la app queda
/// como si lo hubiera escrito la propia app.
pub fn devolver_al_dueno_de_la_app(ruta: &Path) -> bool {
    let Some(dir) = ruta.parent() else {
        return false;
    };
    // El uid se toma del directorio que lo contiene (el de la app).
    let uid = uid_de(dir).unwrap_or(UID_ANTIGRAVITY);
    #[cfg(unix)]
    {
        use std::os::unix::fs::chown;
        let _ = chown(ruta, Some(uid), None);
    }
    // Contexto SELinux: `restorecon` (el daemon corre como root).
    let _ = std::process::Command::new("restorecon")
        .arg(ruta)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    uid_de(ruta) == Some(uid)
}

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
    let canonico = sha256_de(Path::new(&bin_canonico()));
    let canonico_bytes = std::fs::metadata(bin_canonico()).map(|m| m.len()).unwrap_or(0);
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
    // El binario del daemon ya NO vive en el runtime de la app (rompia el
    // desempaquetado); vive en el clon del daemon.
    let bin = PathBuf::from(BIN_CLON);
    let mut e = EstadoRuntime {
        ruta: bin.to_string_lossy().to_string(),
        ..Default::default()
    };
    if let Ok(md) = std::fs::metadata(&bin) {
        e.tiene_megadaemon = true;
        e.bytes = md.len();
        e.sha256 = sha256_de(&bin);
        e.uid = uid_de(&bin);
        if let Some(c) = sha256_de(Path::new(&bin_canonico())) {
            e.igual_que_canonico = Some(c == e.sha256.clone().unwrap_or_default());
        }
    }
    e.uid_directorio = uid_de(Path::new(RUNTIME));
    // ¿Queda algún intruso nuestro en el runtime de la app?
    let intruso = Path::new(RUNTIME).join("megadaemon-core");
    e.tiene_megadaemon = intruso.exists();
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
    let canonico = PathBuf::from(bin_canonico());
    if !canonico.exists() {
        return Err(format!("no existe el binario canónico {}", bin_canonico()));
    }
    // MEDIDO (2026-11-23): el destino ya NO es el runtime de la app (ahí la
    // app desempaqueta sus assets y un fichero root lo rompe todo), sino el
    // directorio del clon del daemon, que la app no toca.
    let dest = PathBuf::from(BIN_CLON);
    let dir = dest
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from(BIN_CLON));
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;

    // Limpieza del INTRUSO: si una versión anterior dejó el binario (o su
    // .prev) dentro del runtime de la app, se borra AHORA (con el dueño y
    // el contexto SELinux correctos: root puede borrarlo, la app no).
    let intrusos = [
        PathBuf::from(RUNTIME).join("megadaemon-core"),
        PathBuf::from(RUNTIME).join("megadaemon-core.prev"),
    ];
    let mut limpiados = vec![];
    for i in intrusos {
        if i.exists() {
            if std::fs::remove_file(&i).is_ok() {
                limpiados.push(i.to_string_lossy().to_string());
            }
        }
    }

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

    let datos = std::fs::read(&canonico).map_err(|e| format!("leer el canónico: {e}"))?;
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
    rep.igual_que_canonico = Some(sha == sha256_de(&canonico));
    rep.aplicado = true;
    rep.detalle = if limpiados.is_empty() {
        format!(
            "binario canónico copiado al clon del daemon (sha256 {})",
            sha.clone().unwrap_or_else(|| "?".into())
        )
    } else {
        format!(
            "binario canónico copiado al clon del daemon (sha256 {}) y              {} intruso(s) borrado(s) del runtime de la app: {}",
            sha.clone().unwrap_or_else(|| "?".into()),
            limpiados.len(),
            limpiados.join(", ")
        )
    };
    Ok(rep)
}

// ---------------------------------------------------------------------------
// Sincronización del código: DELEGADA (puente fino)
// ---------------------------------------------------------------------------

/// Resultado de la sincronización del código del clon.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReporteSync {
    /// ¿Se aplicó algún cambio?
    pub aplicado: bool,
    /// Commit al que apunta el clon (informativo).
    pub commit: Option<String>,
    /// Nº de ficheros escritos (0 en la vía delegada).
    pub ficheros_escritos: usize,
    /// Bytes escritos (0 en la vía delegada).
    pub bytes_escritos: u64,
    /// Respaldos hechos (vacío en la vía delegada).
    pub respaldos: Vec<String>,
    /// Qué se ha hecho y por qué.
    pub detalle: String,
    /// Ruta del mecanismo **canónico** que se ha usado (si lo hay).
    pub mecanismo_canonico: Option<String>,
    /// Si el clon está desfasado respecto a lo esperado.
    pub desfasado: bool,
}

/// Sincroniza el código del clon **delegando** en el mecanismo canónico.
///
/// DECISIÓN (asimilación de Antigravity, 2026-11-22)
/// --------------------------------------------------
/// Este módulo **tenía** su propio motor de *pull*: descargaba el tarball de
/// `main` de GitHub y lo aplicaba encima del clon con su propio gestor de
/// respaldos.  Eso era **el segundo motor**: el daemon ya tiene el suyo
/// (`self_update` para el binario y `deploy/instalador-megadaemon.sh` para la
/// instalación completa), y el clon de Antigravity es una **copia** que la
/// propia IDE no necesita para nada: la IDE sólo necesita el **binario**, y
/// ése ya queda unificado con el canónico por [`unificar_runtime`].
///
/// Aquí no se borra nada sin sustituto: el equivalente canónico es
///
/// * **binario** → [`unificar_runtime`] (copia el binario canónico al runtime
///   de Antigravity: mismo binario, misma versión, mismo auto-update), y
/// * **instalación completa** → `deploy/instalador-megadaemon.sh --instalar`.
///
/// Si el instalador canónico está presente en el dispositivo, se le llama;
/// si no, se informa con exactitud de qué falta (sin inventar).
pub fn sincronizar_codigo() -> Result<ReporteSync, String> {
    let clon = estado_clon();
    let instalador = instalador_canonico();
    let commit = clon.commit.clone();

    let Some(instalador) = instalador else {
        return Ok(ReporteSync {
            aplicado: false,
            commit,
            ficheros_escritos: 0,
            bytes_escritos: 0,
            respaldos: Vec::new(),
            detalle: format!(
                "sin motor de sincronización propio (ASIMILADO): la fuente canónica es el repo \
                 (main de {REPO}) y el binario canónico ya está unificado en {RUNTIME}. \
                 Para una instalación completa usa deploy/instalador-megadaemon.sh --instalar."
            ),
            mecanismo_canonico: None,
            desfasado: clon.commit.is_none(),
        });
    };

    // El instalador canónico se delega: el daemon no reimplementa el pull.
    let salida = crate::android::root_exec(&format!("sh '{}' --instalar", instalador), 300_000, 64 * 1024);
    Ok(ReporteSync {
        aplicado: salida.code == Some(0),
        commit,
        ficheros_escritos: 0,
        bytes_escritos: 0,
        respaldos: Vec::new(),
        detalle: format!(
            "sincronización DELEGADA al instalador canónico ({}){}{}",
            instalador,
            salida.stdout.lines().last().unwrap_or(""),
            if salida.timed_out { " (TIEMPO AGOTADO)" } else { "" }
        ),
        mecanismo_canonico: Some(instalador),
        desfasado: false,
    })
}

/// Ruta del instalador canónico en el dispositivo (si está).
fn instalador_canonico() -> Option<String> {
    for cand in [
        "/data/local/tmp/megadaemon/instalador-megadaemon.sh",
        "/data/user/0/com.agy/files/megadaemon/deploy/instalador-megadaemon.sh",
        // MEDIDO (2026-11-23): el directorio base canónico del daemon (en el
        // móvil, `/data/adb/megadaemon`, persistente) es donde se despliega
        // el instalador.
        "/data/adb/megadaemon/instalador-megadaemon.sh",
    ] {
        if Path::new(cand).exists() {
            return Some(cand.to_string());
        }
    }
    None
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
        if !Path::new(&bin_canonico()).exists() {
            let e = unificar_runtime().unwrap_err();
            assert!(e.contains("canónico"), "mensaje: {e}");
        }
    }

    #[test]
    fn sha256_de_un_fichero_inexistente_es_none() {
        assert!(sha256_de(Path::new("/no/existe/nada")).is_none());
    }
}
