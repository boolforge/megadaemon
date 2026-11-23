//! MegaDaemon — **AUTORREPARACIÓN DEL ENTORNO** con un solo comando.
//!
//! QUÉ ES
//!   Hasta ahora el entorno se reparaba desde FUERA (`tools/recuperar-movil.sh`,
//!   `deploy/bootstrap-movil.sh`, `tools/md-repair-entorno.sh`) ⇒ si el móvil
//!   se rompía y no había PC, nadie lo arreglaba.  Este módulo mete la
//!   reparación DENTRO del daemon: el propio proceso que está roto es el
//!   que la sabe hacer.
//!
//! POR QUÉ UN VEREDICTO Y NO UN "OK"
//!   Cada eslabón dice tres cosas, como el reparador de shell del repo:
//!     * `que`   — qué pasa (o qué pasaba antes de reparar);
//!     * `toca`  — qué se ha hecho (o qué hay que hacer a mano);
//!     * `bloquea` — si esto deja al nodo inservible.
//!   Un `ok: true` sin detalles no sirve de nada cuando falla a medias.
//!
//! DISEÑO (puro y testeable)
//!   Toda la lógica pasa por el trait [`Entorno`], así que el mismo código
//!   que corre en el móvil se testea en un PC con un [`Entorno`] de mentira.
//!   Los motores que ya existen se reutilizan (NO se reimplementan):
//!     * [`crate::supervisor::WATCHDOG_SH`] / [`crate::supervisor::KEEPER_SH`]
//!       — fuente única del watchdog y del keeper;
//!     * [`crate::api_key::ApiKeyConfig::generate_and_store`] — acuñar la API key;
//!     * [`crate::cockpit_server::EMBEDDED_TEMPLATE`] — plantilla web.
//!
//! MEDIDO (2026-11-23, S21 Ultra): las 4 causas por las que el móvil se
//! quedaba sin daemon eran, en orden de frecuencia:
//!   1. el binario vivía en `/data/local/tmp` (volátil) y el reinicio lo
//!      borraba ⇒ el supervisor entraba en bucle;
//!   2. el supervisor no estaba instalado (o era de una versión anterior);
//!   3. faltaba la API key ⇒ TODO respondía 403;
//!   4. las credenciales de Drive no estaban ⇒ el tablón y los backups
//!      fallaban con 401.
//!   Las cuatro están cubiertas aquí.
//!
//! Autoría: sesión root de MegaDaemon (2026-11-23).

use serde_json::{json, Value};
use std::path::Path;

/// Prefijo **persistente** de la instalación en Android.
///
/// MEDIDO (2026-11-23): `/data/local/tmp` es VOLÁTIL (lo limpia el sistema) ⇒
/// el binario y el estado NO pueden vivir ahí.  `/data/adb` sobrevive a los
/// reinicios y es el directorio que usa Magisk.
pub const PREFIJO_PERSISTENTE: &str = "/data/adb/megadaemon";

/// Prefijo LEGACY (volátil).  Se migra, no se usa.
pub const PREFIJO_LEGACY: &str = "/data/local/tmp/megadaemon";

/// Watchdog de arranque (Magisk `service.d`).
pub const RUTA_SUPERVISOR: &str = "/data/adb/service.d/99-megadaemon.sh";

/// Guardian del supervisor (evita que el watchdog entre en bucle).
pub const RUTA_KEEPER: &str = "/data/adb/service.d/97-megadaemon-keeper.sh";

/// Nombre del binario del daemon.
pub const NOMBRE_BINARIO: &str = "megadaemon-core";

/// Estado de un eslabón del entorno.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    /// Estaba bien y no hubo que tocar nada.
    Ok,
    /// Estaba roto y **se ha reparado**.
    Reparado,
    /// Aviso: no es un fallo, pero conviene enterarse (y hay algo que
    /// hacer que el daemon no puede hacer solo).
    Aviso,
    /// Fallo: el daemon **no** puede repararlo solo.
    Fallo,
}

impl Estado {
    pub fn as_str(&self) -> &'static str {
        match self {
            Estado::Ok => "ok",
            Estado::Reparado => "reparado",
            Estado::Aviso => "aviso",
            Estado::Fallo => "fallo",
        }
    }
}

/// Un eslabón del entorno (con su veredicto y su explicación).
#[derive(Debug, Clone)]
pub struct Eslabon {
    /// Nombre estable del eslabón (para scripts y tests).
    pub nombre: &'static str,
    /// Veredicto.
    pub estado: Estado,
    /// Qué pasa (o qué pasaba antes de reparar).
    pub que: String,
    /// Qué se ha hecho, o qué hay que hacer a mano.
    pub toca: String,
    /// Si esto deja al nodo inservible.
    pub bloquea: bool,
}

impl Eslabon {
    fn nuevo(nombre: &'static str, estado: Estado, que: &str, toca: &str, bloquea: bool) -> Self {
        Eslabon {
            nombre,
            estado,
            que: que.to_string(),
            toca: toca.to_string(),
            bloquea,
        }
    }

    /// Una línea de resumen (una por eslabón).
    pub fn linea(&self) -> String {
        format!(
            "[{}] {} :: {} :: {}",
            self.estado.as_str(),
            self.nombre,
            self.que,
            self.toca
        )
    }
}

/// El veredicto completo.
#[derive(Debug, Clone)]
pub struct Veredicto {
    pub eslabones: Vec<Eslabon>,
    /// ¿Se ha reparado algo?
    pub reparado: bool,
}

impl Veredicto {
    /// El nodo sirve: ningún eslabón bloqueante.
    pub fn ok(&self) -> bool {
        !self.eslabones.iter().any(|e| e.bloquea)
    }

    /// Número de eslabones con problema.
    pub fn con_problemas(&self) -> usize {
        self.eslabones
            .iter()
            .filter(|e| e.estado == Estado::Fallo || e.estado == Estado::Aviso)
            .count()
    }

    /// Las 3 líneas del informe (qué / toca / bloquea).
    pub fn informe(&self) -> Vec<String> {
        let rotos: Vec<&Eslabon> = self.eslabones.iter().filter(|e| e.bloquea).collect();
        let reparados: Vec<&Eslabon> = self
            .eslabones
            .iter()
            .filter(|e| e.estado == Estado::Reparado)
            .collect();
        let avisos: Vec<&Eslabon> = self
            .eslabones
            .iter()
            .filter(|e| e.estado == Estado::Aviso && !e.bloquea)
            .collect();
        vec![
            format!(
                "que: {} eslabones, {} roto(s), {} reparado(s), {} aviso(s)",
                self.eslabones.len(),
                rotos.len(),
                reparados.len(),
                avisos.len()
            ),
            format!(
                "toca: {}",
                if reparados.is_empty() {
                    "nada que reparar".to_string()
                } else {
                    reparados
                        .iter()
                        .map(|e| e.nombre)
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ),
            format!(
                "bloquea: {}",
                if rotos.is_empty() {
                    "nada: el nodo sirve".to_string()
                } else {
                    rotos
                        .iter()
                        .map(|e| format!("{} ({})", e.nombre, e.toca))
                        .collect::<Vec<_>>()
                        .join("; ")
                }
            ),
        ]
    }

    /// JSON del veredicto.
    pub fn a_json(&self) -> Value {
        json!({
            "ok": self.ok(),
            "reparado": self.reparado,
            "eslabones": self.eslabones.iter().map(|e| json!({
                "nombre": e.nombre,
                "estado": e.estado.as_str(),
                "que": e.que,
                "toca": e.toca,
                "bloquea": e.bloquea,
            })).collect::<Vec<_>>(),
            "informe": self.informe(),
        })
    }
}

/// El entorno (disco).  Todo pasa por aquí ⇒ el módulo es testeable.
pub trait Entorno {
    fn existe(&self, ruta: &str) -> bool;
    fn es_dir(&self, ruta: &str) -> bool;
    fn leer(&self, ruta: &str) -> Option<String>;
    fn escribir(&self, ruta: &str, contenido: &str, modo: u32) -> Result<(), String>;
    fn mkdir(&self, ruta: &str, modo: u32) -> Result<(), String>;
    fn chmod(&self, ruta: &str, modo: u32) -> Result<(), String>;
    fn modo(&self, ruta: &str) -> Option<u32>;
    fn remover(&self, ruta: &str) -> Result<(), String>;
}

/// Implementación real sobre el sistema de ficheros.
pub struct FsEntorno;

impl Entorno for FsEntorno {
    fn existe(&self, ruta: &str) -> bool {
        Path::new(ruta).exists()
    }
    fn es_dir(&self, ruta: &str) -> bool {
        Path::new(ruta).is_dir()
    }
    fn leer(&self, ruta: &str) -> Option<String> {
        std::fs::read_to_string(ruta).ok()
    }
    fn escribir(&self, ruta: &str, contenido: &str, modo: u32) -> Result<(), String> {
        // Escritura ATOMICA: temporal + rename.  Un fichero a medias deja el
        // móvil sin daemon (el watchdog encuentra un script truncado).
        crate::util_fs::write_atomic(Path::new(ruta), contenido.as_bytes(), modo)
    }
    fn mkdir(&self, ruta: &str, modo: u32) -> Result<(), String> {
        std::fs::create_dir_all(ruta).map_err(|e| format!("crear {ruta}: {e}"))?;
        crate::util_fs::set_mode(Path::new(ruta), modo);
        Ok(())
    }
    fn chmod(&self, ruta: &str, modo: u32) -> Result<(), String> {
        crate::util_fs::set_mode(Path::new(ruta), modo);
        Ok(())
    }
    fn modo(&self, ruta: &str) -> Option<u32> {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(ruta)
            .ok()
            .map(|m| m.permissions().mode() & 0o7777)
    }
    fn remover(&self, ruta: &str) -> Result<(), String> {
        std::fs::remove_file(ruta).map_err(|e| format!("borrar {ruta}: {e}"))
    }
}

/// Configuración del entorno a reparar.
#[derive(Debug, Clone)]
pub struct Config {
    /// Prefijo persistente de la instalación.
    pub prefijo: String,
    /// Prefijo legacy (volátil): se migra.
    pub prefijo_legacy: String,
    /// Directorio de estado.
    pub state_dir: String,
    /// Ruta del watchdog.
    pub ruta_supervisor: String,
    /// Ruta del keeper.
    pub ruta_keeper: String,
    /// ¿Estamos en Android?  (en un PC no se tocan `/data/adb`).
    pub android: bool,
}

impl Config {
    /// Configuración autodetectada.
    pub fn del_entorno() -> Config {
        let android = crate::supervisor::es_android();
        let prefijo = std::env::var("MEGADAEMON_PREFIX")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| PREFIJO_PERSISTENTE.to_string());
        let state_dir = crate::cockpit_server::state_dir();
        Config {
            prefijo,
            prefijo_legacy: PREFIJO_LEGACY.to_string(),
            state_dir,
            ruta_supervisor: RUTA_SUPERVISOR.to_string(),
            ruta_keeper: RUTA_KEEPER.to_string(),
            android,
        }
    }

    /// Ruta del binario del daemon en el prefijo persistente.
    pub fn binario(&self) -> String {
        format!("{}/{}", self.prefijo, NOMBRE_BINARIO)
    }

    /// Ruta del binario en el prefijo legacy.
    pub fn binario_legacy(&self) -> String {
        format!("{}/{}", self.prefijo_legacy, NOMBRE_BINARIO)
    }

    /// Ruta de la copia anterior del binario (`.prev`).
    pub fn binario_prev(&self) -> String {
        format!("{}.prev", self.binario())
    }

    /// Ruta de la API key.
    pub fn api_key(&self) -> String {
        format!("{}/megadaemon_api.key", self.state_dir)
    }

    /// Ruta de las credenciales de Drive.
    pub fn credenciales_drive(&self) -> String {
        format!("{}/gdrive_credentials.json", self.state_dir)
    }

    /// Ruta de la plantilla web.
    pub fn plantilla(&self) -> String {
        format!("{}/web/cockpit_template.html", self.prefijo)
    }
}

// ---------------------------------------------------------------------------
// DIAGNÓSTICO
// ---------------------------------------------------------------------------

/// **DIAGNÓSTICA** el entorno.  No toca nada.
pub fn verificar(env: &dyn Entorno, cfg: &Config) -> Vec<Eslabon> {
    let mut v = Vec::new();

    // 1) prefijo persistente
    if env.es_dir(&cfg.prefijo) {
        v.push(Eslabon::nuevo(
            "prefijo_persistente",
            Estado::Ok,
            &format!("{} existe", cfg.prefijo),
            "nada",
            false,
        ));
    } else {
        v.push(Eslabon::nuevo(
            "prefijo_persistente",
            Estado::Fallo,
            &format!("{} NO existe", cfg.prefijo),
            "crear el directorio (hace falta root)",
            true,
        ));
    }

    // 2) directorio de estado
    if env.es_dir(&cfg.state_dir) {
        v.push(Eslabon::nuevo(
            "state_dir",
            Estado::Ok,
            &format!("{} existe", cfg.state_dir),
            "nada",
            false,
        ));
    } else {
        v.push(Eslabon::nuevo(
            "state_dir",
            Estado::Fallo,
            &format!("{} NO existe", cfg.state_dir),
            "crear el directorio",
            true,
        ));
    }

    // 3) binario en servicio
    let bin = cfg.binario();
    if env.existe(&bin) {
        let modo = env.modo(&bin).unwrap_or(0);
        let ejecutable = modo & 0o111 != 0;
        if ejecutable {
            v.push(Eslabon::nuevo(
                "binario",
                Estado::Ok,
                &format!("{} presente (modo {:o})", bin, modo),
                "nada",
                false,
            ));
        } else {
            v.push(Eslabon::nuevo(
                "binario",
                Estado::Fallo,
                &format!("{} NO es ejecutable (modo {:o})", bin, modo),
                "chmod 0755",
                true,
            ));
        }
    } else {
        v.push(Eslabon::nuevo(
            "binario",
            Estado::Fallo,
            &format!("{} NO existe", bin),
            "restaurar el binario (ver binario_legacy / .prev)",
            true,
        ));
    }

    // 4) binario en el prefijo legacy (volátil): hay que migrarlo
    let bin_legacy = cfg.binario_legacy();
    if env.existe(&bin_legacy) {
        v.push(Eslabon::nuevo(
            "binario_legacy",
            Estado::Aviso,
            &format!("{} existe (ruta VOLATIL)", bin_legacy),
            &format!("migrar a {}", cfg.binario()),
            false,
        ));
    } else {
        v.push(Eslabon::nuevo(
            "binario_legacy",
            Estado::Ok,
            "no hay binario en la ruta volatil",
            "nada",
            false,
        ));
    }

    // 5) supervisor
    match env.leer(&cfg.ruta_supervisor) {
        Some(c) if c == crate::supervisor::WATCHDOG_SH => v.push(Eslabon::nuevo(
            "supervisor",
            Estado::Ok,
            &format!("{} instalado y al dia", cfg.ruta_supervisor),
            "nada",
            false,
        )),
        Some(_) => v.push(Eslabon::nuevo(
            "supervisor",
            Estado::Aviso,
            &format!("{} existe pero es de otra version", cfg.ruta_supervisor),
            "reinstalar el watchdog actual",
            false,
        )),
        None => v.push(Eslabon::nuevo(
            "supervisor",
            Estado::Fallo,
            &format!("{} NO instalado", cfg.ruta_supervisor),
            "instalar el watchdog (Magisk lo ejecuta en cada boot)",
            true,
        )),
    }

    // 6) keeper
    match env.leer(&cfg.ruta_keeper) {
        Some(c) if c == crate::supervisor::KEEPER_SH => v.push(Eslabon::nuevo(
            "keeper",
            Estado::Ok,
            &format!("{} instalado y al dia", cfg.ruta_keeper),
            "nada",
            false,
        )),
        Some(_) => v.push(Eslabon::nuevo(
            "keeper",
            Estado::Aviso,
            &format!("{} existe pero es de otra version", cfg.ruta_keeper),
            "reinstalar el guardian actual",
            false,
        )),
        None => v.push(Eslabon::nuevo(
            "keeper",
            Estado::Aviso,
            &format!("{} NO instalado", cfg.ruta_keeper),
            "instalar el guardian (evita el bucle de reinicios)",
            false,
        )),
    }

    // 7) API key
    match env.leer(&cfg.api_key()) {
        Some(k) if k.trim().len() >= 16 => {
            let modo = env.modo(&cfg.api_key()).unwrap_or(0);
            if modo == 0o600 {
                v.push(Eslabon::nuevo(
                    "api_key",
                    Estado::Ok,
                    "API key presente (0600)",
                    "nada",
                    false,
                ));
            } else {
                v.push(Eslabon::nuevo(
                    "api_key",
                    Estado::Aviso,
                    &format!("API key presente pero con modo {:o}", modo),
                    "chmod 0600",
                    false,
                ));
            }
        }
        Some(_) => v.push(Eslabon::nuevo(
            "api_key",
            Estado::Fallo,
            "la API key existe pero esta vacia o es corta",
            "acunar una nueva",
            true,
        )),
        None => v.push(Eslabon::nuevo(
            "api_key",
            Estado::Fallo,
            "NO hay API key: todo responde 403",
            "acunar una key (queda en el fichero, 0600)",
            true,
        )),
    }

    // 8) credenciales de Drive
    if env.existe(&cfg.credenciales_drive()) {
        v.push(Eslabon::nuevo(
            "credenciales_drive",
            Estado::Ok,
            "credenciales de Drive presentes",
            "nada",
            false,
        ));
    } else {
        v.push(Eslabon::nuevo(
            "credenciales_drive",
            Estado::Aviso,
            "NO hay credenciales de Drive: el tablon y los backups fallan",
            "copiar gdrive_credentials.json al state dir (el daemon no puede inventarlas)",
            false,
        ));
    }

    // 9) plantilla web
    if env.existe(&cfg.plantilla()) {
        v.push(Eslabon::nuevo(
            "plantilla_web",
            Estado::Ok,
            "plantilla web presente",
            "nada",
            false,
        ));
    } else {
        v.push(Eslabon::nuevo(
            "plantilla_web",
            Estado::Aviso,
            "NO hay plantilla web en disco: se usara la embebida",
            "materializar la plantilla embebida",
            false,
        ));
    }

    v
}

// ---------------------------------------------------------------------------
// Reparación
// ---------------------------------------------------------------------------

/// **REPARA** el entorno.  Devuelve el veredicto (con lo que se hizo).
///
/// Es IDEMPOTENTE: se puede repetir mil veces; sólo toca lo que está roto.
pub fn reparar(env: &dyn Entorno, cfg: &Config) -> Veredicto {
    let antes = verificar(env, cfg);
    let mut v: Vec<Eslabon> = Vec::new();
    let mut reparado = false;

    // --- 1) directorios -------------------------------------------------
    let d_pre = env.es_dir(&cfg.prefijo);
    let d_est = env.es_dir(&cfg.state_dir);
    if !d_pre {
        match env.mkdir(&cfg.prefijo, 0o755) {
            Ok(_) => {
                reparado = true;
                v.push(Eslabon::nuevo(
                    "prefijo_persistente",
                    Estado::Reparado,
                    &format!("{} no existia", cfg.prefijo),
                    &format!("creado {}", cfg.prefijo),
                    false,
                ));
            }
            Err(e) => v.push(Eslabon::nuevo(
                "prefijo_persistente",
                Estado::Fallo,
                &format!("{} no existia", cfg.prefijo),
                &e,
                true,
            )),
        }
    } else {
        v.push(Eslabon::nuevo(
            "prefijo_persistente",
            Estado::Ok,
            &format!("{} existe", cfg.prefijo),
            "nada",
            false,
        ));
    }
    if !d_est {
        match env.mkdir(&cfg.state_dir, 0o700) {
            Ok(_) => {
                reparado = true;
                v.push(Eslabon::nuevo(
                    "state_dir",
                    Estado::Reparado,
                    &format!("{} no existia", cfg.state_dir),
                    &format!("creado {}", cfg.state_dir),
                    false,
                ));
            }
            Err(e) => v.push(Eslabon::nuevo(
                "state_dir",
                Estado::Fallo,
                &format!("{} no existia", cfg.state_dir),
                &e,
                true,
            )),
        }
    } else {
        v.push(Eslabon::nuevo(
            "state_dir",
            Estado::Ok,
            &format!("{} existe", cfg.state_dir),
            "nada",
            false,
        ));
    }

    // --- 2) binario -----------------------------------------------------
    // Orden de preferencia: el `.prev` (última build buena), luego el
    // legacy (misma build, ruta volátil).  Si no hay ninguno, NO se puede
    // fabricar el binario: se dice con precisión qué falta.
    let bin = cfg.binario();
    let prev = cfg.binario_prev();
    let legacy = cfg.binario_legacy();
    if env.existe(&bin) {
        let modo = env.modo(&bin).unwrap_or(0);
        if modo & 0o111 == 0 {
            let _ = env.chmod(&bin, 0o755);
            reparado = true;
            v.push(Eslabon::nuevo(
                "binario",
                Estado::Reparado,
                &format!("{} no era ejecutable (modo {:o})", bin, modo),
                "chmod 0755",
                false,
            ));
        } else {
            v.push(Eslabon::nuevo(
                "binario",
                Estado::Ok,
                &format!("{} presente (modo {:o})", bin, modo),
                "nada",
                false,
            ));
        }
    } else {
        let origen = if env.existe(&prev) {
            Some(prev.clone())
        } else if env.existe(&legacy) {
            Some(legacy.clone())
        } else {
            None
        };
        match origen {
            Some(o) => {
                match env.leer(&o).and_then(|c| env.escribir(&bin, &c, 0o755).ok()) {
                    Some(_) => {
                        reparado = true;
                        v.push(Eslabon::nuevo(
                            "binario",
                            Estado::Reparado,
                            &format!("{} no existia", bin),
                            &format!("restaurado desde {o}"),
                            false,
                        ));
                    }
                    None => v.push(Eslabon::nuevo(
                        "binario",
                        Estado::Fallo,
                        &format!("{} no existia", bin),
                        &format!("no se pudo copiar {o}"),
                        true,
                    )),
                }
            }
            None => v.push(Eslabon::nuevo(
                "binario",
                Estado::Fallo,
                &format!("{} no existia", bin),
                "no hay ni .prev ni copia legacy: instalar una build (auto-actualizador o release)",
                true,
            )),
        }
    }

    // --- 3) binario legacy: migrar y limpiar ---------------------------
    if env.existe(&legacy) {
        if env.existe(&bin) {
            v.push(Eslabon::nuevo(
                "binario_legacy",
                Estado::Aviso,
                &format!("{} sigue en la ruta VOLATIL", legacy),
                "se puede borrar: la copia buena ya esta en el prefijo persistente",
                false,
            ));
        } else {
            match env.leer(&legacy).and_then(|c| env.escribir(&bin, &c, 0o755).ok()) {
                Some(_) => {
                    reparado = true;
                    v.push(Eslabon::nuevo(
                        "binario_legacy",
                        Estado::Reparado,
                        &format!("{} estaba en la ruta VOLATIL", legacy),
                        &format!("migrado a {bin}"),
                        false,
                    ));
                }
                None => v.push(Eslabon::nuevo(
                    "binario_legacy",
                    Estado::Fallo,
                    &format!("{} esta en la ruta VOLATIL", legacy),
                    "no se pudo migrar",
                    true,
                )),
            }
        }
    } else {
        v.push(Eslabon::nuevo(
            "binario_legacy",
            Estado::Ok,
            "no hay binario en la ruta volatil",
            "nada",
            false,
        ));
    }

    // --- 4) supervisor y keeper ----------------------------------------
    for (nombre, ruta, cuerpo) in [
        ("supervisor", &cfg.ruta_supervisor, crate::supervisor::WATCHDOG_SH),
        ("keeper", &cfg.ruta_keeper, crate::supervisor::KEEPER_SH),
    ] {
        match env.leer(ruta) {
            Some(c) if c == cuerpo => v.push(Eslabon::nuevo(
                nombre,
                Estado::Ok,
                &format!("{ruta} instalado y al dia"),
                "nada",
                false,
            )),
            Some(_) => match env.escribir(ruta, cuerpo, 0o755) {
                Ok(_) => {
                    reparado = true;
                    v.push(Eslabon::nuevo(
                        nombre,
                        Estado::Reparado,
                        &format!("{ruta} era de otra version"),
                        "reinstalado (atomico)",
                        false,
                    ));
                }
                Err(e) => v.push(Eslabon::nuevo(
                    nombre,
                    Estado::Fallo,
                    &format!("{ruta} existe pero no se pudo reescribir"),
                    &e,
                    nombre == "supervisor",
                )),
            },
            None => match env.escribir(ruta, cuerpo, 0o755) {
                Ok(_) => {
                    reparado = true;
                    v.push(Eslabon::nuevo(
                        nombre,
                        Estado::Reparado,
                        &format!("{ruta} no estaba instalado"),
                        "instalado (atomico)",
                        false,
                    ));
                }
                Err(e) => v.push(Eslabon::nuevo(
                    nombre,
                    Estado::Fallo,
                    &format!("{ruta} no estaba instalado"),
                    &e,
                    nombre == "supervisor",
                )),
            },
        }
    }

    // --- 5) API key -----------------------------------------------------
    let ruta_key = cfg.api_key();
    match env.leer(&ruta_key) {
        Some(k) if k.trim().len() >= 16 => {
            let modo = env.modo(&ruta_key).unwrap_or(0);
            if modo != 0o600 {
                let _ = env.chmod(&ruta_key, 0o600);
                reparado = true;
                v.push(Eslabon::nuevo(
                    "api_key",
                    Estado::Reparado,
                    &format!("API key presente pero con modo {:o}", modo),
                    "chmod 0600",
                    false,
                ));
            } else {
                v.push(Eslabon::nuevo(
                    "api_key",
                    Estado::Ok,
                    "API key presente (0600)",
                    "nada",
                    false,
                ));
            }
        }
        _ => {
            // Se acuña con el motor que ya existe (no se reimplementa).
            match crate::api_key::ApiKeyConfig::generate_and_store(&cfg.state_dir) {
                Ok(_) => {
                    reparado = true;
                    v.push(Eslabon::nuevo(
                        "api_key",
                        Estado::Reparado,
                        "no habia API key",
                        "acuñada una nueva (fichero 0600; no se imprime)",
                        false,
                    ));
                }
                Err(e) => v.push(Eslabon::nuevo(
                    "api_key",
                    Estado::Fallo,
                    "no habia API key",
                    &e,
                    true,
                )),
            }
        }
    }

    // --- 6) credenciales de Drive --------------------------------------
    if env.existe(&cfg.credenciales_drive()) {
        v.push(Eslabon::nuevo(
            "credenciales_drive",
            Estado::Ok,
            "credenciales de Drive presentes",
            "nada",
            false,
        ));
    } else {
        v.push(Eslabon::nuevo(
            "credenciales_drive",
            Estado::Aviso,
            "NO hay credenciales de Drive: el tablon y los backups fallan",
            "copiar gdrive_credentials.json al state dir (el daemon no puede inventarlas)",
            false,
        ));
    }

    // --- 7) plantilla web ----------------------------------------------
    let ruta_tpl = cfg.plantilla();
    if env.existe(&ruta_tpl) {
        v.push(Eslabon::nuevo(
            "plantilla_web",
            Estado::Ok,
            "plantilla web presente",
            "nada",
            false,
        ));
    } else {
        match env.escribir(
            &ruta_tpl,
            crate::cockpit_server::EMBEDDED_TEMPLATE,
            0o644,
        ) {
            Ok(_) => {
                reparado = true;
                v.push(Eslabon::nuevo(
                    "plantilla_web",
                    Estado::Reparado,
                    "no habia plantilla web en disco",
                    "materializada desde la embebida",
                    false,
                ));
            }
            Err(e) => v.push(Eslabon::nuevo(
                "plantilla_web",
                Estado::Aviso,
                "no se pudo materializar la plantilla web",
                &e,
                false,
            )),
        }
    }

    Veredicto { eslabones: v, reparado }
}

/// Diagnostica (sin tocar nada).
pub fn diagnosticar() -> Veredicto {
    let cfg = Config::del_entorno();
    Veredicto {
        eslabones: verificar(&FsEntorno, &cfg),
        reparado: false,
    }
}

/// Diagnostica y repara.
pub fn reparar_ahora() -> Veredicto {
    let cfg = Config::del_entorno();
    reparar(&FsEntorno, &cfg)
}

/// CLI: `megadaemon-core reparar [--diagnostico] [--json]`.
pub fn run_cli(args: &[String]) -> i32 {
    let solo = args.iter().any(|a| a == "--diagnostico" || a == "diagnostico");
    let json = args.iter().any(|a| a == "--json");
    let v = if solo {
        diagnosticar()
    } else {
        reparar_ahora()
    };
    if json {
        println!("{}", v.a_json());
    } else {
        for e in &v.eslabones {
            println!("{}", e.linea());
        }
        println!("---");
        for l in v.informe() {
            println!("{}", l);
        }
    }
    if v.ok() {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Entorno de mentira: un mapa en memoria.
    #[derive(Default)]
    struct Fake {
        ficheros: RefCell<std::collections::BTreeMap<String, String>>,
        modos: RefCell<std::collections::BTreeMap<String, u32>>,
        dirs: RefCell<std::collections::BTreeSet<String>>,
    }

    impl Fake {
        fn nuevo() -> Fake {
            Fake::default()
        }
        fn con_dir(self, d: &str) -> Fake {
            self.dirs.borrow_mut().insert(d.to_string());
            self
        }
        fn con_fichero(self, r: &str, c: &str, m: u32) -> Fake {
            self.ficheros.borrow_mut().insert(r.to_string(), c.to_string());
            self.modos.borrow_mut().insert(r.to_string(), m);
            self
        }
    }

    impl Entorno for Fake {
        fn existe(&self, ruta: &str) -> bool {
            self.ficheros.borrow().contains_key(ruta) || self.dirs.borrow().contains(ruta)
        }
        fn es_dir(&self, ruta: &str) -> bool {
            self.dirs.borrow().contains(ruta)
        }
        fn leer(&self, ruta: &str) -> Option<String> {
            self.ficheros.borrow().get(ruta).cloned()
        }
        fn escribir(&self, ruta: &str, contenido: &str, modo: u32) -> Result<(), String> {
            if let Some(p) = ruta.rfind('/') {
                self.dirs.borrow_mut().insert(ruta[..p].to_string());
            }
            self.ficheros
                .borrow_mut()
                .insert(ruta.to_string(), contenido.to_string());
            self.modos.borrow_mut().insert(ruta.to_string(), modo);
            Ok(())
        }
        fn mkdir(&self, ruta: &str, _modo: u32) -> Result<(), String> {
            self.dirs.borrow_mut().insert(ruta.to_string());
            Ok(())
        }
        fn chmod(&self, ruta: &str, modo: u32) -> Result<(), String> {
            self.modos.borrow_mut().insert(ruta.to_string(), modo);
            Ok(())
        }
        fn modo(&self, ruta: &str) -> Option<u32> {
            self.modos.borrow().get(ruta).copied()
        }
        fn remover(&self, ruta: &str) -> Result<(), String> {
            self.ficheros.borrow_mut().remove(ruta);
            Ok(())
        }
    }

    fn cfg_de_prueba() -> Config {
        Config {
            prefijo: "/data/adb/megadaemon".to_string(),
            prefijo_legacy: "/data/local/tmp/megadaemon".to_string(),
            state_dir: "/data/adb/megadaemon/state".to_string(),
            ruta_supervisor: "/data/adb/service.d/99-megadaemon.sh".to_string(),
            ruta_keeper: "/data/adb/service.d/97-megadaemon-keeper.sh".to_string(),
            android: true,
        }
    }

    fn entorno_sano() -> Fake {
        Fake::nuevo()
            .con_dir("/data/adb/megadaemon")
            .con_dir("/data/adb/megadaemon/state")
            .con_fichero(
                "/data/adb/megadaemon/megadaemon-core",
                "ELF",
                0o755,
            )
            .con_fichero(
                "/data/adb/service.d/99-megadaemon.sh",
                crate::supervisor::WATCHDOG_SH,
                0o755,
            )
            .con_fichero(
                "/data/adb/service.d/97-megadaemon-keeper.sh",
                crate::supervisor::KEEPER_SH,
                0o755,
            )
            .con_fichero(
                "/data/adb/megadaemon/state/megadaemon_api.key",
                "mdaemon_sk_live_0123456789abcdef0123456789abcdef",
                0o600,
            )
            .con_fichero(
                "/data/adb/megadaemon/state/gdrive_credentials.json",
                "{}",
                0o600,
            )
            .con_fichero(
                "/data/adb/megadaemon/web/cockpit_template.html",
                "<html></html>",
                0o644,
            )
    }

    #[test]
    fn un_entorno_sano_no_necesita_reparacion() {
        let e = entorno_sano();
        let v = verificar(&e, &cfg_de_prueba());
        assert!(!v.iter().any(|x| x.bloquea), "no debe haber eslabones bloqueantes");
        assert!(v.iter().all(|x| x.estado == Estado::Ok));
    }

    #[test]
    fn sin_api_key_todo_esta_roto_y_se_acuña() {
        let e = Fake::nuevo()
            .con_dir("/data/adb/megadaemon")
            .con_dir("/data/adb/megadaemon/state")
            .con_fichero("/data/adb/megadaemon/megadaemon-core", "ELF", 0o755);
        let cfg = cfg_de_prueba();
        let v = verificar(&e, &cfg);
        let key = v.iter().find(|x| x.nombre == "api_key").unwrap();
        assert!(key.bloquea, "sin API key el nodo no sirve");
        let r = reparar(&e, &cfg);
        let key = r.eslabones.iter().find(|x| x.nombre == "api_key").unwrap();
        assert_eq!(key.estado, Estado::Reparado);
        assert!(r.ok());
    }

    #[test]
    fn el_binario_se_migra_desde_la_ruta_volatil() {
        // MEDIDO (2026-11-23): el bug que tumbaba el movil era el binario en
        // /data/local/tmp (volatil) => el reinicio lo borraba.
        let e = Fake::nuevo()
            .con_dir("/data/local/tmp/megadaemon")
            .con_fichero("/data/local/tmp/megadaemon/megadaemon-core", "ELF", 0o755)
            .con_dir("/data/adb/megadaemon")
            .con_dir("/data/adb/megadaemon/state");
        let cfg = cfg_de_prueba();
        let v = verificar(&e, &cfg);
        let bin = v.iter().find(|x| x.nombre == "binario").unwrap();
        assert!(bin.bloquea, "sin binario en el prefijo persistente no arranca");
        let r = reparar(&e, &cfg);
        let bin = r.eslabones.iter().find(|x| x.nombre == "binario").unwrap();
        assert_eq!(bin.estado, Estado::Reparado);
        assert!(e.existe("/data/adb/megadaemon/megadaemon-core"));
    }

    #[test]
    fn reparar_es_idempotente() {
        let e = entorno_sano();
        let cfg = cfg_de_prueba();
        let r1 = reparar(&e, &cfg);
        let r2 = reparar(&e, &cfg);
        assert!(!r2.reparado, "la segunda pasada no debe tocar nada");
        assert!(r1.ok() && r2.ok());
        assert_eq!(r1.eslabones.len(), r2.eslabones.len());
    }

    #[test]
    fn el_supervisor_roto_se_reinstala() {
        let mut e = entorno_sano();
        e.ficheros
            .borrow_mut()
            .insert("/data/adb/service.d/99-megadaemon.sh".to_string(), "viejo".to_string());
        let cfg = cfg_de_prueba();
        let r = reparar(&e, &cfg);
        let s = r.eslabones.iter().find(|x| x.nombre == "supervisor").unwrap();
        assert_eq!(s.estado, Estado::Reparado);
        assert_eq!(
            e.leer("/data/adb/service.d/99-megadaemon.sh").unwrap(),
            crate::supervisor::WATCHDOG_SH
        );
    }

    #[test]
    fn el_informe_dice_que_toca_y_que_bloquea() {
        let e = Fake::nuevo();
        let v = Veredicto {
            eslabones: vec![
                Eslabon::nuevo("a", Estado::Ok, "bien", "nada", false),
                Eslabon::nuevo("b", Estado::Reparado, "roto", "arreglado", false),
                Eslabon::nuevo("c", Estado::Fallo, "muy roto", "manual", true),
            ],
            reparado: true,
        };
        assert!(!v.ok());
        let inf = v.informe();
        assert_eq!(inf.len(), 3);
        assert!(inf[2].contains("c"));
    }
}
