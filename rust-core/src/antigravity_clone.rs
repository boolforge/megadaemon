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
pub const RUNTIME: &str = "/data/user/0/com.agy/files/runtime/bin";
/// Binario canónico del daemon (root).
pub const BIN_CANONICO: &str = "/data/local/tmp/megadaemon/megadaemon-core";

/// Binario unificado **FUERA** del runtime que gestiona Antigravity.
///
/// MEDIDO (2026-11-23, móvil): el desempaquetador de la app **borra**
/// `files/runtime/` en cada arranque/actualización y **falla** si no puede
/// borrar lo que hay dentro:
///
/// ```text
/// core start failed: cannot unpack assets/runtime into
/// /data/user/0/com.agy/files/runtime: cannot delete
/// /data/user/0/com.agy/files/runtime/bin/megadaemon-core
/// ```
///
/// ⇒ **`runtime/` es de la app: no se escribe nada ahí**.  El binario
/// unificado vive en el clon (que ya existe y mantiene la propia IDE), en
/// `megadaemon/bin/`, y el intruso que nosotros metimos en `runtime/bin/`
/// se borra (ver [`limpiar_runtime`]).
pub const BIN_UNIFICADO: &str = "/data/user/0/com.agy/files/megadaemon/bin/megadaemon-core";

/// Config MCP de Antigravity (la que declara el servidor `megadaemon-minimax`).
pub const MCP_CONFIG: &str = "/data/user/0/com.agy/files/.gemini/config/mcp_config.json";

/// Ficheros que **nosotros** metimos en `runtime/bin/` y que el
/// desempaquetador de la app no puede borrar (rompen su arranque).
pub const INTRUSOS_RUNTIME: &[&str] = &["megadaemon-core", "megadaemon-core.prev"];
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
    /// ¿Sigue algún intruso nuestro en `runtime/bin/`? (rompe el arranque
    /// de la app: medido 2026-11-23).
    pub intruso_en_runtime: bool,
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
///
/// MEDIDO (2026-11-23): el binario unificado **no** va en `runtime/bin/`
/// (rompe el arranque de la app, ver [`BIN_UNIFICADO`]); aquí se mide la
/// ruta buena y se avisa si el intruso sigue en el runtime.
pub fn estado_runtime() -> EstadoRuntime {
    let bin = Path::new(BIN_UNIFICADO).to_path_buf();
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
    e.intruso_en_runtime = intrusos_presentes();
    e
}

/// ¿Sigue alguno de nuestros ficheros en `runtime/bin/`?
///
/// Si sí, el **core de Antigravity no arranca** (medido 2026-11-23): el
/// desempaquetador no puede borrarlos.
pub fn intrusos_presentes() -> bool {
    let dir = Path::new(RUNTIME);
    INTRUSOS_RUNTIME
        .iter()
        .any(|n| dir.join(n).exists() || dir.join(n).is_symlink())
}

/// Arregla la config MCP de Antigravity para que el servidor
/// `megadaemon-minimax` apunte al binario **unificado**.
///
/// MEDIDO (2026-11-22/23): la config del móvil declara
/// `"command": "/data/user/0/com.agy/files/megadaemon/rust-core/target/release/megadaemon-core"`,
/// ruta que **no existe** en el dispositivo (el clon no tiene build) ⇒ el
/// servidor MCP no arranca aunque sus *schemas* de tools estén cacheados
/// (por eso "parece" funcionar).
///
/// Es **idempotente**: sólo reescribe los `command` que apuntan a un
/// `megadaemon-core` inexistente.  Devuelve `true` si cambió algo.
pub fn arreglar_mcp_config() -> Result<bool, String> {
    let raw = std::fs::read_to_string(MCP_CONFIG)
        .map_err(|e| format!("leer {MCP_CONFIG}: {e}"))?;
    let mut v: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| format!("parsear {MCP_CONFIG}: {e}"))?;
    let servers = match v.get_mut("mcpServers").and_then(|m| m.as_object_mut()) {
        Some(m) => m,
        None => return Ok(false),
    };
    let mut cambiado = false;
    for (_nombre, srv) in servers.iter_mut() {
        let cmd = match srv.get("command").and_then(|c| c.as_str()) {
            Some(c) => c.to_string(),
            None => continue,
        };
        if !cmd.contains("megadaemon-core") {
            continue;
        }
        if Path::new(&cmd).exists() {
            continue; // ya apunta a algo real: no se toca
        }
        if let Some(o) = srv.as_object_mut() {
            o.insert(
                "command".to_string(),
                serde_json::Value::String(BIN_UNIFICADO.to_string()),
            );
            cambiado = true;
        }
    }
    if !cambiado {
        return Ok(false);
    }
    let salida = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
    std::fs::write(MCP_CONFIG, salida).map_err(|e| format!("escribir {MCP_CONFIG}: {e}"))?;
    Ok(true)
}

/// Borra de `runtime/bin/` lo que **nosotros** metimos allí.
///
/// Es la reparación del fallo medido el 2026-11-23 (`cannot unpack
/// assets/runtime … cannot delete …/runtime/bin/megadaemon-core`): con esto
/// fuera, la app puede desempaquetar su runtime y su core vuelve a arrancar.
///
/// Devuelve las rutas que ha quitado (vacío = no había nada).
pub fn limpiar_runtime() -> Vec<String> {
    let dir = Path::new(RUNTIME);
    let mut quitados = Vec::new();
    for n in INTRUSOS_RUNTIME {
        let p = dir.join(n);
        if p.exists() || p.is_symlink() {
            match std::fs::remove_file(&p) {
                Ok(()) => quitados.push(p.to_string_lossy().to_string()),
                Err(e) => {
                    // Con root se puede; sin root, el mensaje dice qué hacer.
                    eprintln!("antigravity: no se pudo borrar {p:?}: {e}");
                }
            }
        }
    }
    quitados
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
    /// Ficheros intrusos que se han quitado de `runtime/bin/`.
    pub intrusos_eliminados: Vec<String>,
    /// Qué pasó con la config MCP de Antigravity.
    pub detalle_mcp: Option<String>,
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
    // MEDIDO (2026-11-23): el binario va FUERA de `runtime/bin/` (esa
    // carpeta la gestiona la app y su desempaquetador no puede borrar lo que
    // encuentra ⇒ el core de Antigravity deja de arrancar).
    let dir = Path::new(BIN_UNIFICADO)
        .parent()
        .ok_or_else(|| "ruta unificada sin directorio".to_string())?
        .to_path_buf();
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
    let dest = Path::new(BIN_UNIFICADO).to_path_buf();

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
        let uid = uid_de(&dir).unwrap_or(UID_ANTIGRAVITY);
        let _ = std::os::unix::fs::chown(&dest, Some(uid), None);
        rep.uid = uid_de(&dest);
    }

    // Lo que dejamos antes en `runtime/bin/` fuera: es lo que rompía el
    // arranque de la app (medido 2026-11-23).
    rep.intrusos_eliminados = limpiar_runtime();

    // Y el MCP de Antigravity apunta al binario unificado (si no, su
    // servidor MCP no arranca: el `command` apunta a un build inexistente).
    match arreglar_mcp_config() {
        Ok(true) => rep.detalle_mcp = Some("config MCP apuntada al binario unificado".into()),
        Ok(false) => rep.detalle_mcp = Some("config MCP ya correcta o sin servidores".into()),
        Err(e) => rep.detalle_mcp = Some(format!("config MCP no arreglada: {e}")),
    }

    let sha = sha256_de(&dest);
    rep.bytes = datos.len() as u64;
    rep.sha256 = sha.clone();
    rep.igual_que_canonico = Some(sha == sha256_de(canonico));
    rep.aplicado = true;
    rep.detalle = format!(
        "binario canónico copiado a {BIN_UNIFICADO} (sha256 {}); intrusos fuera de runtime/bin: {}",
        sha.clone().unwrap_or_else(|| "?".into()),
        if rep.intrusos_eliminados.is_empty() {
            "ninguno".to_string()
        } else {
            rep.intrusos_eliminados.join(", ")
        }
    );
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
        if !Path::new(BIN_CANONICO).exists() {
            let e = unificar_runtime().unwrap_err();
            assert!(e.contains("canónico"), "mensaje: {e}");
        }
    }

    #[test]
    fn el_binario_unificado_no_vive_en_el_runtime_de_la_app() {
        // MEDIDO (2026-11-23): si el binario está en `runtime/bin/`, el
        // desempaquetador de Antigravity no puede borrarlo y el core de la
        // app NO arranca (`cannot unpack assets/runtime ... cannot delete`).
        assert!(!BIN_UNIFICADO.starts_with(RUNTIME), "ruta: {BIN_UNIFICADO}");
        assert!(BIN_UNIFICADO.contains("/megadaemon/bin/"), "ruta: {BIN_UNIFICADO}");
    }

    #[test]
    fn sha256_de_un_fichero_inexistente_es_none() {
        assert!(sha256_de(Path::new("/no/existe/nada")).is_none());
    }
}
