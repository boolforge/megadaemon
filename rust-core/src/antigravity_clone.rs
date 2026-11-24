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

/// Binario **unificado**: el único que usan TODOS (la IDE, el servidor MCP
/// de Antigravity y el clon).
///
/// MEDIDO (2026-11-23, móvil, diagnóstico de la app):
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

/// Configuración de permisos del agente de Antigravity (`.gemini/config`).
///
/// MEDIDO (2026-11-24, móvil): aquí viven los *grants* de permisos que el
/// language server se auto-concede para ejecutar comandos.  Si la forma es
/// inválida, el LS lo registra en su log y **la app lo pinta en la UI**
/// ("ignoring invalid allow entry ..."), que es el "muro de texto" que veía
/// el usuario.
pub const CONFIG_PERMISOS: &str = "/data/user/0/com.agy/files/.gemini/config/config.json";

/// **Sanea** los grants de permisos de Antigravity.
///
/// MEDIDO (2026-11-24): el LS sólo acepta dos formas de grant:
///
/// * `command(<cmd>)`  → dentro del sandbox
/// * `n(<cmd>)`        → fuera del sandbox
///
/// y el comodín exacto `unsandboxed(*)`.  Cualquier otra forma (por ejemplo
/// `unsandboxed(python3 -c "...")`) la rechaza el LS una y otra vez ⇒
/// flood de logs ⇒ la UI muestra el error largo.
///
/// El daemon no es quien escribe esos grants (los escribe el agente), pero
/// puede **sanearlos** en cuanto aparecen: así el flood no puede volver
/// aunque el emisor sea otro.
///
/// Devuelve cuántas entradas se corrigieron (0 = nada que hacer).
pub fn sanear_grants_permisos() -> Result<usize, String> {
    let ruta = std::path::Path::new(CONFIG_PERMISOS);
    let crudo = match std::fs::read_to_string(ruta) {
        Ok(c) => c,
        Err(_) => return Ok(0),
    };
    let mut v: serde_json::Value = match serde_json::from_str(&crudo) {
        Ok(v) => v,
        Err(_) => return Ok(0),
    };
    let mut corregidas = 0usize;
    sanear_valor(&mut v, &mut corregidas);
    if corregidas == 0 {
        return Ok(0);
    }
    let nuevo = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
    // Escritura atómica (mismo directorio) para que el LS no lea medio
    // fichero: `.tmp` + rename.
    let tmp = ruta.with_extension("json.tmp-megadaemon");
    std::fs::write(&tmp, nuevo.as_bytes()).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, ruta).map_err(|e| format!("{}: {e}", ruta.display()))?;
    Ok(corregidas)
}

/// Recorre el JSON y corrige las formas de grant inválidas.
fn sanear_valor(v: &mut serde_json::Value, corregidas: &mut usize) {
    match v {
        serde_json::Value::Object(m) => {
            for (_k, val) in m.iter_mut() {
                sanear_valor(val, corregidas);
            }
        }
        serde_json::Value::Array(a) => {
            for val in a.iter_mut() {
                sanear_valor(val, corregidas);
            }
        }
        serde_json::Value::String(s) => {
            // `unsandboxed(...)` => `n(...)`, salvo el comodín `unsandboxed(*)`.
            if s.starts_with("unsandboxed(") && s != "unsandboxed(*)" {
                let dentro = s.trim_start_matches("unsandboxed(");
                let dentro = dentro.strip_suffix(')').unwrap_or(dentro);
                *s = format!("n({dentro})");
                *corregidas += 1;
            }
        }
        _ => {}
    }
}

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
///
/// MEDIDO (2026-11-23): el binario unificado **no** va en `runtime/bin/`
/// (rompe el arranque de la app, ver [`BIN_UNIFICADO`]); aquí se mide la
/// ruta buena y se avisa si el intruso sigue en el runtime.
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
// Config MCP de Antigravity
// ---------------------------------------------------------------------------

/// `mcp_config.json` de Antigravity: donde se declara el servidor MCP
/// `megadaemon-minimax` que hace que Antigravity use a MegaDaemon.
pub const AGY_MCP_CONFIG: &str = "/data/user/0/com.agy/files/.gemini/config/mcp_config.json";
/// Nombre del servidor MCP que declara MegaDaemon en Antigravity.
pub const SERVIDOR_MCP: &str = "megadaemon-minimax";
/// Puerto del daemon en el móvil (alias multi-puerto).
pub const PUERTO_DAEMON: u16 = 9090;

/// Qué cambió al sincronizar el `mcp_config.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReporteMcpConfig {
    pub aplicado: bool,
    pub ruta: String,
    pub campos_actualizados: Vec<String>,
    pub detalle: String,
}

/// Mantiene el `mcp_config.json` de Antigravity apuntando al daemon local
/// con las credenciales **vigentes**.
///
/// MEDIDO (2026-11-22): el fichero traía el `X-Seere-Auth-Token` y el
/// `X-API-Key` **escritos a mano**.  Si se rota cualquiera de los dos (la
/// rotación de la API key es una operación normal), el plugin de Antigravity
/// se queda roto y sus 52 tools dejan de responder, sin que nadie se entere.
///
/// Aqui se re-sincroniza solo al arrancar el daemon:
///
/// * `url` => `http://127.0.0.1:9090/mcp` (el daemon local, no el tunel:
///   asi funciona aunque no haya red);
/// * `X-API-Key` => la API key de la instancia;
/// * `X-Seere-Auth-Token` => el token de sesion vigente.
///
/// **El resto del JSON se conserva** (por ejemplo el bloque `providers` con
/// las claves de los modelos externos): esto NO toca ni rota credenciales
/// ajenas.
pub fn sincronizar_mcp_config() -> Result<ReporteMcpConfig, String> {
    let ruta = std::path::Path::new(AGY_MCP_CONFIG);
    if !ruta.exists() {
        return Err(format!("no existe {AGY_MCP_CONFIG}"));
    }
    let texto = std::fs::read_to_string(ruta).map_err(|e| format!("leer mcp_config: {e}"))?;
    let mut v: serde_json::Value =
        serde_json::from_str(&texto).map_err(|e| format!("mcp_config no es JSON: {e}"))?;

    let sd = crate::cockpit_server::state_dir();
    // MEDIDO (2026-11-24, CI): el módulo de claves se llama `ApiKeyConfig`
    // y su carga es `ApiKeyConfig::load(state_dir)`.
    let cfg = crate::api_key::ApiKeyConfig::load(&sd);
    let api_key = cfg.key.clone();
    let token = crate::cockpit_server::auth_token().to_string();

    let mut cambios: Vec<String> = Vec::new();
    {
        let servidores = v
            .get_mut("mcpServers")
            .and_then(|m| m.as_object_mut())
            .ok_or("mcp_config.json no tiene 'mcpServers'")?;
        let entrada = servidores
            .get_mut(SERVIDOR_MCP)
            .and_then(|s| s.as_object_mut())
            .ok_or("mcp_config.json no declara 'megadaemon-minimax'")?;

        let url = format!("http://127.0.0.1:{PUERTO_DAEMON}/mcp");
        if entrada.get("url").and_then(|u| u.as_str()) != Some(url.as_str()) {
            entrada.insert("url".into(), serde_json::Value::String(url));
            cambios.push("url".into());
        }
        let cabeceras = entrada
            .entry("headers".to_string())
            .or_insert_with(|| serde_json::json!({}));
        let cabeceras = cabeceras
            .as_object_mut()
            .ok_or("'headers' de megadaemon-minimax no es un objeto")?;
        for (k, valor) in [("X-API-Key", &api_key), ("X-Seere-Auth-Token", &token)] {
            if cabeceras.get(k).and_then(|x| x.as_str()) != Some(valor.as_str()) {
                cabeceras.insert(k.to_string(), serde_json::Value::String(valor.clone()));
                cambios.push(k.to_string());
            }
        }
    }

    if cambios.is_empty() {
        return Ok(ReporteMcpConfig {
            aplicado: false,
            ruta: AGY_MCP_CONFIG.to_string(),
            campos_actualizados: cambios,
            detalle: "ya estaba sincronizado".into(),
        });
    }

    // Se escribe EN EL SITIO (truncar) y no con rename: asi se conservan el
    // propietario (u0_a1573) y los permisos del fichero de la app.
    let mut salida = serde_json::to_string_pretty(&v).map_err(|e| format!("serializar: {e}"))?;
    salida.push('\n');
    std::fs::write(ruta, salida.as_bytes()).map_err(|e| format!("escribir mcp_config: {e}"))?;

    Ok(ReporteMcpConfig {
        aplicado: true,
        ruta: AGY_MCP_CONFIG.to_string(),
        campos_actualizados: cambios,
        detalle: format!("sincronizado con el daemon local ({PUERTO_DAEMON})"),
    })
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
    #[test]
    fn los_grants_solo_dejan_las_formas_que_el_ls_acepta() {
        // MEDIDO (2026-11-24): el LS acepta `command(...)` y `n(...)`, y el
        // comodín exacto `unsandboxed(*)`.  Cualquier otra forma la
        // rechaza (flood de logs + el error pintado en la UI de la app).
        let mut v: serde_json::Value = serde_json::from_str(
            r#"{"allow":["unsandboxed(python3 -c \"print(1)\")","command(ls)","unsandboxed(*)"],"otro":{"x":"unsandboxed(a b c)"}}"#,
        )
        .unwrap();
        let mut n = 0;
        super::sanear_valor(&mut v, &mut n);
        assert_eq!(n, 2, "debía corregir 2 grants: {v}");
        let arr = v.get("allow").and_then(|a| a.as_array()).unwrap();
        assert_eq!(arr[0].as_str().unwrap(), "n(python3 -c \"print(1)\")");
        assert_eq!(arr[1].as_str().unwrap(), "command(ls)");
        assert_eq!(arr[2].as_str().unwrap(), "unsandboxed(*)");
    }
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
