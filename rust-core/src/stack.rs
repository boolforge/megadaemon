//! **Stack autocontenido**: el daemon lleva DENTRO todo lo que el móvil
//! necesita, y se lo repara solo.
//!
//! POR QUÉ EXISTE (medido el 2026-11-25 en el móvil del usuario)
//! -----------------------------------------------------------
//! El binario sí se actualizaba, pero **el resto del stack no**.  El
//! supervisor instalado en el móvil seguía siendo una versión vieja cuyo
//! prefijo era la ruta VOLÁTIL (`/data/local/tmp/megadaemon`), mientras el
//! binario canónico vive en `/data/adb/megadaemon`:
//!
//! ```text
//! [2026-11-25 12:00:57] ERROR: no existe /data/local/tmp/megadaemon/megadaemon-core
//!                          (y no hay despliegue en curso)
//! ```
//!
//! ⇒ el supervisor no encontraba el binario ⇒ **no podía relanzar el
//! daemon** ⇒ el móvil se quedaba sin daemon (UI y gateway caídos) y
//! entraba en bucle de reinicios.  Además la Web UI servida era la V1
//! (7 vistas, sin Antigravity) porque el HTML en disco era viejo.
//!
//! QUÉ HACE ESTE MÓDULO
//! --------------------
//! 1. Una tabla **embebida** de los ficheros del stack (supervisor,
//!    keeper, cloudflared, ADB-TCP, scripts de Antigravity y la Web UI).
//! 2. [`reconciliar`]: compara el SHA-256 de lo que hay en disco con lo
//!    embebido y reescribe **sólo lo que difiere**.  Es idempotente y la
//!    escritura es atómica (`.tmp` + `rename`).
//! 3. [`estado`]: el veredicto, para la Web UI, el tablón y el log.
//!
//! REGLA DE ORO
//! ------------
//! El stack que se reconcilia es **siempre el del binario en ejecución**
//! ⇒ nunca hay mezcla de versiones: si el binario es viejo, repara con el
//! stack viejo (coherente); cuando el binario se actualiza, el stack se
//! actualiza con él en el siguiente arranque.
//!
//! Autor: sesión root de MegaDaemon (2026-11-25).

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Prefijo persistente de la instalación en Android.
pub const PREFIJO: &str = "/data/adb/megadaemon";

/// Dónde va un fichero del stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destino {
    /// Ruta absoluta (no depende del prefijo).
    Absoluta(&'static str),
    /// Dentro del prefijo persistente: `<prefijo>/<resto>`.
    Prefijo(&'static str),
    /// En el directorio de servicios de Magisk: `<padre del prefijo>/service.d/<resto>`.
    Servicio(&'static str),
}

/// Un fichero del stack, embebido en el binario.
#[derive(Debug, Clone, Copy)]
pub struct Entrada {
    /// De dónde viene (en el repo), para poder auditar.
    pub origen: &'static str,
    /// Dónde se instala.
    pub destino: Destino,
    /// Permisos del fichero instalado.
    pub modo: u32,
    /// Contenido embebido.
    pub contenido: &'static str,
}

impl Entrada {
    /// Resuelve la ruta final de instalación.
    pub fn ruta(&self, prefijo: &str) -> PathBuf {
        match self.destino {
            Destino::Absoluta(p) => PathBuf::from(p),
            Destino::Prefijo(r) => Path::new(prefijo).join(r),
            Destino::Servicio(r) => Path::new(prefijo)
                .parent()
                .unwrap_or_else(|| Path::new("/data/adb"))
                .join("service.d")
                .join(r),
        }
    }
}

/// **El stack del móvil, embebido.**
///
/// MEDIDO (2026-11-25): el supervisor es el fichero CRÍTICO (es el que
/// relanza el daemon, el túnel y la política de energía).  El keeper es su
/// guardián.  Los scripts de Antigravity son los que lanzan las tareas.  Y
/// la Web UI en disco es la que sirve el daemon si existe (si no, la
/// embebida) ⇒ mantenerla al día evita volver a la V1.
pub const STACK: &[Entrada] = &[
    Entrada {
        origen: "deploy/99-megadaemon.sh",
        destino: Destino::Servicio("99-megadaemon.sh"),
        modo: 0o755,
        contenido: include_str!("../../deploy/99-megadaemon.sh"),
    },
    Entrada {
        origen: "deploy/97-megadaemon-keeper.sh",
        destino: Destino::Servicio("97-megadaemon-keeper.sh"),
        modo: 0o755,
        contenido: include_str!("../../deploy/97-megadaemon-keeper.sh"),
    },
    Entrada {
        origen: "deploy/98-cloudflared.sh",
        destino: Destino::Servicio("98-cloudflared.sh"),
        modo: 0o755,
        contenido: include_str!("../../deploy/98-cloudflared.sh"),
    },
    Entrada {
        // MEDIDO (2026-11-26): el watchdog del túnel estaba FUERA del stack
        // embebido ⇒ sólo lo instalaba el instalador manual ⇒ la
        // auto-actualización del daemon NUNCA lo refrescaba (y `98-cloudflared.sh`
        // lo lanza: sin él, "NO existe $WATCHDOG").  Con el watchdog en el
        // stack, el móvil se autorrepara también con este script.
        origen: "deploy/tunnel-watchdog.sh",
        destino: Destino::Prefijo("tunnel-watchdog.sh"),
        modo: 0o755,
        contenido: include_str!("../../deploy/tunnel-watchdog.sh"),
    },
    Entrada {
        origen: "deploy/91-adb-tcp.sh",
        destino: Destino::Servicio("91-adb-tcp.sh"),
        modo: 0o755,
        contenido: include_str!("../../deploy/91-adb-tcp.sh"),
    },
    Entrada {
        origen: "deploy/antigravity-task-runner.sh",
        destino: Destino::Prefijo("antigravity-task-runner.sh"),
        modo: 0o755,
        contenido: include_str!("../../deploy/antigravity-task-runner.sh"),
    },
    Entrada {
        origen: "deploy/antigravity-task-supervisor.sh",
        destino: Destino::Prefijo("antigravity-task-supervisor.sh"),
        modo: 0o755,
        contenido: include_str!("../../deploy/antigravity-task-supervisor.sh"),
    },
    Entrada {
        origen: "web/cockpit_v2.html",
        destino: Destino::Prefijo("web/cockpit_v2.html"),
        modo: 0o644,
        contenido: include_str!("../../web/cockpit_v2.html"),
    },
    Entrada {
        origen: "web/webui_v2.css",
        destino: Destino::Prefijo("web/webui_v2.css"),
        modo: 0o644,
        contenido: include_str!("../../web/webui_v2.css"),
    },
    Entrada {
        origen: "web/cockpit_template.html",
        destino: Destino::Prefijo("web/cockpit_template.html"),
        modo: 0o644,
        contenido: include_str!("../../web/cockpit_template.html"),
    },
];

/// Configuración mínima del reconciliador.
#[derive(Debug, Clone)]
pub struct Config {
    /// Prefijo de la instalación.
    pub prefijo: String,
    /// ¿Estamos en Android?  (en un PC no se toca `/data/adb`).
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
            .unwrap_or_else(|| PREFIJO.to_string());
        Config { prefijo, android }
    }
}

/// SHA-256 de un fichero (o `None` si no existe).
pub fn hash_de(ruta: &Path) -> Option<String> {
    let datos = std::fs::read(ruta).ok()?;
    Some(crate::self_update::sha256_hex(&datos))
}

/// SHA-256 esperado (embebido) de una entrada.
pub fn hash_esperado(e: &Entrada) -> String {
    crate::self_update::sha256_hex(e.contenido.as_bytes())
}

/// Estado de un fichero del stack.
#[derive(Debug, Clone)]
pub struct EstadoFichero {
    /// Ruta final.
    pub ruta: String,
    /// De dónde viene en el repo.
    pub origen: &'static str,
    /// Hash embebido (el que debería estar en disco).
    pub esperado: String,
    /// Hash real en disco (si existe).
    pub actual: Option<String>,
    /// ¿Coincide?
    pub ok: bool,
}

/// El veredicto de una reconciliación.
#[derive(Debug, Clone, Default)]
pub struct Informe {
    /// Ficheros reescritos.
    pub escritos: Vec<String>,
    /// Ficheros que ya estaban bien.
    pub correctos: Vec<String>,
    /// (ruta, motivo) de los que no se pudieron escribir.
    pub fallidos: Vec<(String, String)>,
    /// Ficheros **sobrantes** que se han borrado (copias `.bak` del
    /// supervisor en `service.d`).
    ///
    /// MEDIDO 2026-11-28 (móvil): una copia `.bak-50k` del supervisor se
    /// quedó en `/data/adb/service.d/` ⇒ **Magisk la ejecutaba en cada
    /// arranque** ⇒ dos supervisores peleándose por el daemon y un túnel
    /// `mcp-legacy` relanzado en bucle (12 reinicios/15 min medidos).
    pub borrados: Vec<String>,
    /// ¿El supervisor cambió?  (entonces hay que reiniciarlo)
    pub supervisor_cambiado: bool,
}

impl Informe {
    /// ¿Hubo cambios?
    pub fn cambio(&self) -> bool {
        !self.escritos.is_empty() || !self.fallidos.is_empty() || !self.borrados.is_empty()
    }

    /// ¿Todo correcto?
    pub fn ok(&self) -> bool {
        self.fallidos.is_empty()
    }

    /// Resumen de una línea.
    pub fn linea(&self) -> String {
        format!(
            "stack: {} escrito(s), {} ya ok, {} borrado(s), {} fallo(s)",
            self.escritos.len(),
            self.correctos.len(),
            self.borrados.len(),
            self.fallidos.len()
        )
    }

    /// A JSON (para la Web UI y el tablón).
    pub fn a_json(&self) -> Value {
        json!({
            "escritos": self.escritos,
            "correctos": self.correctos.len(),
            "borrados": self.borrados,
            "fallidos": self.fallidos,
            "supervisor_cambiado": self.supervisor_cambiado,
            "ok": self.ok(),
        })
    }
}

/// Escribe bytes de forma ATÓMICA (`.tmp` + `rename`) y con permisos.
///
/// Es la versión binaria de [`escribir_atomico`] (la del bundle meta).
pub fn escribir_atomico_bytes(destino: &Path, datos: &[u8], modo: u32) -> Result<(), String> {
    if let Some(p) = destino.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("no se pudo crear {}: {e}", p.display()))?;
    }
    let tmp = destino.with_extension("tmp-bundle");
    std::fs::write(&tmp, datos)
        .map_err(|e| format!("no se pudo escribir {}: {e}", tmp.display()))?;
    chmod(&tmp, modo)?;
    std::fs::rename(&tmp, destino)
        .map_err(|e| format!("no se pudo renombrar a {}: {e}", destino.display()))?;
    chmod(destino, modo)?;
    Ok(())
}

/// Escribe un fichero de forma ATÓMICA (`.tmp` + `rename`) y con permisos.
///
/// MEDIDO: un `cp` directo deja el fichero a medias si el sistema se queda
/// sin batería en mitad ⇒ el supervisor puede leer un shell truncated.
pub fn escribir_atomico(destino: &Path, contenido: &str, modo: u32) -> Result<(), String> {
    if let Some(p) = destino.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("no se pudo crear {}: {e}", p.display()))?;
    }
    let tmp = destino.with_extension("tmp-stack");
    std::fs::write(&tmp, contenido.as_bytes())
        .map_err(|e| format!("no se pudo escribir {}: {e}", tmp.display()))?;
    chmod(&tmp, modo)?;
    std::fs::rename(&tmp, destino)
        .map_err(|e| format!("no se pudo renombrar a {}: {e}", destino.display()))?;
    chmod(destino, modo)?;
    Ok(())
}

#[cfg(unix)]
fn chmod(p: &Path, modo: u32) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(modo))
        .map_err(|e| format!("chmod {}: {e}", p.display()))
}

#[cfg(not(unix))]
fn chmod(_p: &Path, _modo: u32) -> Result<(), String> {
    Ok(())
}

/// **RECONCILA** el stack: reescribe sólo lo que difiere del embebido.
///
/// Es **idempotente**: se puede llamar cada minuto y no hace nada si todo
/// está en su sitio.  Devuelve qué tocó.
pub fn reconciliar(cfg: &Config) -> Informe {
    let mut inf = Informe::default();
    for e in STACK {
        let ruta = e.ruta(&cfg.prefijo);
        let esperado = hash_esperado(e);
        let actual = hash_de(&ruta);
        if actual.as_deref() == Some(esperado.as_str()) {
            inf.correctos.push(ruta.display().to_string());
            continue;
        }
        match escribir_atomico(&ruta, e.contenido, e.modo) {
            Ok(()) => {
                inf.escritos.push(ruta.display().to_string());
                if matches!(e.destino, Destino::Servicio(r) if r == "99-megadaemon.sh") {
                    inf.supervisor_cambiado = true;
                }
            }
            Err(e) => inf.fallidos.push((ruta.display().to_string(), e)),
        }
    }
    limpiar_servicios_sobrantes(cfg, &mut inf);
    inf
}

/// Nombres canónicos del stack que viven en `service.d` (Magisk).
fn nombres_de_servicio() -> Vec<String> {
    STACK
        .iter()
        .filter_map(|e| match e.destino {
            Destino::Servicio(r) => Some(r.to_string()),
            _ => None,
        })
        .collect()
}

/// **Borra los sobrantes** del directorio de servicios de Magisk.
///
/// MEDIDO 2026-11-28 (móvil): el `.bak` del supervisor (la copia de
/// seguridad que se hace al actualizar) se quedaba **dentro** de
/// `/data/adb/service.d/` ⇒ Magisk lo ejecutaba igual que al canónico
/// (Magisk ejecuta todo lo ejecutable de ese directorio) ⇒ dos
/// supervisores, y el viejo relanzaba el túnel `mcp-legacy` en bucle
/// (medido: 12 reinicios/15 min) contra un puerto (8765) que nadie
/// escuchaba.
///
/// Sólo se borran los ficheros **nuestros** (los que empiezan por el
/// nombre canónico de un fichero del stack): los de otras apps no se
/// tocan.  Los directorios tampoco.
fn limpiar_servicios_sobrantes(cfg: &Config, inf: &mut Informe) {
    let dir = Path::new(&cfg.prefijo)
        .parent()
        .unwrap_or_else(|| Path::new("/data/adb"))
        .join("service.d");
    let canonicos = nombres_de_servicio();
    // El prefijo de cada nombre canónico: "99-megadaemon.sh" -> "99-megadaemon"
    let prefijos: Vec<&str> = canonicos
        .iter()
        .filter_map(|c| c.split('.').next())
        .collect();
    let Ok(entradas) = std::fs::read_dir(&dir) else {
        return;
    };
    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        let nombre = entrada.file_name().to_string_lossy().to_string();
        if canonicos.iter().any(|c| *c == nombre) {
            continue;
        }
        if !prefijos.iter().any(|p| nombre.starts_with(p)) {
            continue;
        }
        if !ruta.is_file() {
            continue;
        }
        match std::fs::remove_file(&ruta) {
            Ok(()) => inf.borrados.push(ruta.display().to_string()),
            Err(e) => inf.fallidos.push((ruta.display().to_string(), e.to_string())),
        }
    }
}

/// Estado (sin tocar nada) de cada fichero del stack.
pub fn estado(cfg: &Config) -> Vec<EstadoFichero> {
    STACK
        .iter()
        .map(|e| {
            let ruta = e.ruta(&cfg.prefijo);
            let esperado = hash_esperado(e);
            let actual = hash_de(&ruta);
            EstadoFichero {
                ruta: ruta.display().to_string(),
                origen: e.origen,
                ok: actual.as_deref() == Some(esperado.as_str()),
                esperado,
                actual,
            }
        })
        .collect()
}

/// Estado del stack en JSON.
pub fn estado_json(cfg: &Config) -> Value {
    let lista = estado(cfg);
    let fuera = lista.iter().filter(|e| !e.ok).count();
    json!({
        "prefijo": cfg.prefijo,
        "total": lista.len(),
        "correctos": lista.len() - fuera,
        "desfasados": fuera,
        "ficheros": lista.iter().map(|e| json!({
            "ruta": e.ruta,
            "origen": e.origen,
            "ok": e.ok,
            "esperado": e.esperado,
            "actual": e.actual,
        })).collect::<Vec<Value>>(),
    })
}

/// Ruta canónica del supervisor (la de Magisk `service.d`).
pub const RUTA_SUPERVISOR_CANONICO: &str = "/data/adb/service.d/99-megadaemon.sh";

/// **Reinicia el supervisor** si su fichero en disco cambió, y se
/// **queda con UN SOLO supervisor**.
///
/// MEDIDO (2026-11-25, movil): el diagnóstico automático encontró DOS
/// supervisores vivos:
///
/// ```text
/// 4808 sh /data/adb/service.d/99-megadaemon.sh
/// 4823 sh /data/adb/service.d/99-megadaemon.sh.bak-50k
/// ```
///
/// El segundo es una COPIA del supervisor (el `.bak` que se hace al
/// instalar).  Con dos supervisores, uno reinicia el daemon mientras el
/// otro lo mata ⇒ bucle de reinicios (medido: "el daemon no responde
/// /health ni /api/power (fallo 3/3)" una y otra vez).
///
/// El patrón de `pkill -f` es una SUBSTRING, así que también se lleva el
/// `.bak-50k` (que contiene la ruta canónica) ⇒ aquí se matan TODOS y se
/// relanza el canónico, para no depender del keeper (que tarda hasta 60 s).
///
/// Patrón con corchetes: el propio comando no se mata a sí mismo.
pub fn reiniciar_supervisor() -> bool {
    // Patrón con corchetes: el propio comando no se mata a sí mismo.
    // MEDIDO: `pkill -f` con un patrón que aparece en la propia línea de
    // comandos se suicida ⇒ por eso se usa el truco de los corchetes.
    let _ = std::process::Command::new("pkill")
        .args(["-9", "-f", "service.d/99-megadaemo[n].sh"])
        .output();
    // Que sólo quede el canónico.
    let _ = std::process::Command::new("pkill")
        .args(["-9", "-f", "service.d/99-megadaemo[n].sh.bak"])
        .output();
    // MEDIDO 2026-11-28 (móvil): matarlo NO basta.  El fichero `.bak*` se
    // queda en `service.d/` ⇒ Magisk lo vuelve a ejecutar en el siguiente
    // arranque ⇒ el supervisor obsoleto vuelve y con él el túnel
    // `mcp-legacy` relanzado en bucle.  Ahora el fichero también se borra.
    borrar_copias_de_servicio();
    // Relanzar el canónico si no quedó ninguno vivo (no depender del keeper).
    if !supervisor_vivo() {
        let _ = std::process::Command::new("setsid")
            .args(["nohup", "sh", RUTA_SUPERVISOR_CANONICO])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
    true
}

/// Borra las copias `.bak*` del supervisor en `service.d`.
///
/// MEDIDO 2026-11-28 (móvil): Magisk ejecuta **todo** fichero ejecutable de
/// `/data/adb/service.d/`, así que la "copia de seguridad" del supervisor
/// (`99-megadaemon.sh.bak-50k`) era un **segundo supervisor** ⇒ bucle de
/// reinicios del daemon y del túnel `mcp-legacy`.
fn borrar_copias_de_servicio() {
    let dir = Path::new(RUTA_SUPERVISOR_CANONICO)
        .parent()
        .unwrap_or_else(|| Path::new("/data/adb/service.d"));
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return;
    };
    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        let nombre = entrada.file_name().to_string_lossy().to_string();
        if !nombre.starts_with("99-megadaemon.sh.") {
            continue;
        }
        if !ruta.is_file() {
            continue;
        }
        let _ = std::fs::remove_file(&ruta);
    }
}

/// Un supervisor vivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupervisorVivo {
    /// El PID del proceso.
    pub pid: i32,
    /// ¿Es **el** supervisor canónico (el de la ruta de Magisk)?
    pub canonico: bool,
    /// El `cmdline` (los elementos unidos por espacio), para el informe.
    pub cmdline: String,
}

/// Estado COMPLETO de los supervisores: los canónicos, los que no lo son,
/// y lo que **no se ha podido leer**.
///
/// MEDIDO (2026-11-28, móvil real, build 4a51e82): el diagnóstico del daemon
/// decía `"supervisores": 0` mientras el instalador (shell, root, en el mismo
/// móvil y en el mismo minuto) decía "4 supervisores vivos".  **Las dos
/// cifras mentían**, y por motivos distintos:
///
/// * El instalador cuenta con `grep -lsa '99-megadaemon.sh' /proc/[0-9]*/cmdline`
///   ⇒ **el comando se cuenta a SÍ MISMO**: su propio `cmdline` contiene el
///   patrón, y el del envoltorio (`su -mm -c ...`) también.  MEDIDO (2026-11-28,
///   sandbox): con UN solo supervisor real, ese `grep | wc -l` devuelve **3**
///   (1 real + el `grep` + su envoltorio); en el móvil, con el envoltorio del
///   proveedor de Magisk encima, son **4** ⇒ el "4" del móvil eran CUATRO
///   falsos positivos y CERO supervisores.
/// * El daemon contaba por SUBSTRING (`cmd.contains("99-megadaemon.sh")`) ⇒
///   el mismo falso positivo: la vez que se midió (diagnóstico publicado a las
///   11:50:38, build 4a51e82) contaba **1**, y ese "1" era el propio comando
///   `grep -lsa 99-megadaemon.sh /proc/[0-9]*/cmdline` del instalador, NO un
///   supervisor.  Cuando ese comando ya había terminado (11:52:01) el daemon
///   contaba 0.
/// * Y lo peor: `unwrap_or_default()` **se tragaba el error** de lectura ⇒
///   "no se pudo leer" era INDISTINGUIBLE de "no hay supervisores".
///
/// Por eso ahora el estado dice las tres cosas: cuántos canónicos hay,
/// cuántos no canónicos, y si se ha podido leer todo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EstadoSupervisores {
    /// Los que son **el** supervisor canónico (`/data/adb/service.d/99-megadaemon.sh`).
    pub canonicos: Vec<SupervisorVivo>,
    /// Supervisores que **NO** son el canónico (copias `.bak*`, prefijo viejo...).
    pub no_canonicos: Vec<SupervisorVivo>,
    /// PIDs cuyo `cmdline` **no se ha podido leer**: de ellos no se puede
    /// afirmar nada (ni que haya supervisor ni que no lo haya).
    pub ilegibles: Vec<i32>,
    /// Error al listar `/proc`: si está, el conteo NO es concluyente.
    pub error: Option<String>,
}

impl EstadoSupervisores {
    /// Total de supervisores vivos (canónicos + copias).
    pub fn total(&self) -> usize {
        self.canonicos.len() + self.no_canonicos.len()
    }

    /// ¿Hay al menos un supervisor **canónico**?
    pub fn hay_canonico(&self) -> bool {
        !self.canonicos.is_empty()
    }

    /// ¿El conteo es CONFIABLE?  (false si no se pudo leer `/proc` o algún `cmdline`)
    pub fn fiable(&self) -> bool {
        self.error.is_none() && self.ilegibles.is_empty()
    }

    /// Una frase para el informe: qué hay y qué no se ha podido leer.
    pub fn resumen(&self) -> String {
        let mut s = format!("{} supervisor(es) canonico(s)", self.canonicos.len());
        if !self.no_canonicos.is_empty() {
            s.push_str(&format!(", {} NO canonico(s)", self.no_canonicos.len()));
        }
        match (&self.error, self.ilegibles.len()) {
            (Some(e), _) => s.push_str(&format!(", no se pudo leer /proc: {e}")),
            (None, 0) => {}
            (None, n) => s.push_str(&format!(
                ", {n} proceso(s) con cmdline ilegible: el conteo NO es concluyente"
            )),
        }
        s
    }
}

/// Los elementos del `cmdline` de un proceso (los `\0` son separadores).
fn elementos_cmdline(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).to_string())
        .collect()
}

/// ¿Es este elemento del `argv` la RUTA de un supervisor?
///
/// MEDIDO (2026-11-28, móvil): tiene que ser una RUTA (con `/`) y **sin
/// espacios**:
///
/// * el comando que cuenta los supervisores (`grep -lsa 99-megadaemon.sh ...`)
///   tiene el patrón como argumento suelto (`"99-megadaemon.sh"`, sin `/`)
///   ⇒ NO es un supervisor (era lo que el daemon contaba como tal);
/// * el envoltorio (`su -mm -c "sh /data/.../99-megadaemon.sh"`) lo lleva
///   como UNA cadena con espacios ⇒ NO es un supervisor;
/// * el supervisor de verdad: `sh /data/adb/service.d/99-megadaemon.sh` ⇒ sí.
fn es_ruta_de_supervisor(elemento: &str) -> bool {
    if !elemento.contains('/') || elemento.chars().any(char::is_whitespace) {
        return false;
    }
    let nombre = elemento.rsplit('/').next().unwrap_or(elemento);
    nombre.starts_with("99-megadaemon.sh")
}

/// ¿Este `argv` es el del supervisor **CANÓNICO**?  (elemento a elemento)
///
/// MEDIDO (2026-11-28): la copia `.bak-50k` tiene la ruta canónica como
/// PREFIJO ⇒ con una comparación por subcadena la copia PASABA por canónica
/// (y el canónico de verdad no se relanzaba nunca).  Aquí se comparan los
/// elementos del `argv` uno a uno, como hace el instalador (`grep -qx`).
pub fn es_supervisor_canonico(argv: &[String]) -> bool {
    argv.iter().any(|a| a == RUTA_SUPERVISOR_CANONICO)
}

/// ¿Este `argv` es el de ALGÚN supervisor (canónico o copia)?
pub fn es_supervisor(argv: &[String]) -> bool {
    argv.iter().any(|a| es_ruta_de_supervisor(a))
}

/// Estado de los supervisores vivos, leyendo `/proc`.
pub fn estado_supervisores() -> EstadoSupervisores {
    estado_supervisores_en(Path::new("/proc"))
}

/// Igual que [`estado_supervisores`] pero con la raíz de `/proc`
/// parametrizable (para poder probar con directorios de mentira; el mismo
/// patrón que [`crate::inject::listar_procesos_en`]).
pub fn estado_supervisores_en(raiz: &Path) -> EstadoSupervisores {
    let mut st = EstadoSupervisores::default();
    let entradas = match std::fs::read_dir(raiz) {
        Ok(e) => e,
        Err(e) => {
            // NO se puede listar `/proc` ⇒ no se puede afirmar nada.
            st.error = Some(format!("no se pudo listar {}: {}", raiz.display(), e));
            return st;
        }
    };
    for e in entradas.flatten() {
        let nombre = e.file_name().to_string_lossy().to_string();
        if nombre.is_empty() || !nombre.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let Ok(pid) = nombre.parse::<i32>() else {
            continue;
        };
        let bytes = match std::fs::read(e.path().join("cmdline")) {
            Ok(b) => b,
            Err(err) => {
                // El proceso se ha ido mientras lo miramos (ENOENT): es una
                // carrera normal, no un fallo de lectura.
                if err.kind() != std::io::ErrorKind::NotFound {
                    // Un fallo de lectura NO es "no hay supervisor": se apunta.
                    st.ilegibles.push(pid);
                }
                continue;
            }
        };
        if bytes.is_empty() {
            // Hilo del kernel o zombie: no tiene `cmdline`.
            continue;
        }
        let argv = elementos_cmdline(&bytes);
        if !es_supervisor(&argv) {
            continue;
        }
        let v = SupervisorVivo {
            pid,
            canonico: es_supervisor_canonico(&argv),
            cmdline: argv.join(" "),
        };
        if v.canonico {
            st.canonicos.push(v);
        } else {
            st.no_canonicos.push(v);
        }
    }
    st.canonicos.sort_by_key(|s| s.pid);
    st.no_canonicos.sort_by_key(|s| s.pid);
    st.ilegibles.sort_unstable();
    st
}

/// ¿Cuántos supervisores hay vivos?  (canónicos + copias)
///
/// MEDIDO (2026-11-28, móvil): antes contaba por SUBSTRING ⇒ contaba el
/// comando que cuenta (`grep -lsa 99-megadaemon.sh ...`) y las copias
/// `.bak*` indistinguibles del canónico.  Ahora es el total real.
pub fn numero_supervisores() -> usize {
    estado_supervisores().total()
}

/// Igual que [`numero_supervisores`] con la raíz de `/proc` parametrizable.
pub fn numero_supervisores_en(raiz: &Path) -> usize {
    estado_supervisores_en(raiz).total()
}

/// ¿Existe el proceso del supervisor **CANÓNICO**?
///
/// MEDIDO (2026-11-28): antes cualquier copia `.bak*` hacía de canónico ⇒
/// el canónico de verdad NO se relanzaba (el móvil se quedaba sin él).
pub fn supervisor_vivo() -> bool {
    estado_supervisores().hay_canonico()
}

/// Igual que [`supervisor_vivo`] con la raíz de `/proc` parametrizable.
pub fn supervisor_vivo_en(raiz: &Path) -> bool {
    estado_supervisores_en(raiz).hay_canonico()
}

/// El estado de los supervisores en JSON (para el diagnóstico).
pub fn estado_supervisores_json() -> Value {
    estado_supervisores_json_de(&estado_supervisores())
}

/// Igual que [`estado_supervisores_json`] pero sobre un estado ya calculado
/// (para no barrer `/proc` dos veces).
pub fn estado_supervisores_json_de(st: &EstadoSupervisores) -> Value {
    json!({
        "canonicos": st.canonicos.len(),
        "no_canonicos": st.no_canonicos.len(),
        "total": st.total(),
        "ilegibles": st.ilegibles.len(),
        "pids_canonicos": st.canonicos.iter().map(|s| s.pid).collect::<Vec<i32>>(),
        "pids_no_canonicos": st.no_canonicos.iter().map(|s| s.pid).collect::<Vec<i32>>(),
        "cmdlines_no_canonicos": st.no_canonicos.iter().map(|s| s.cmdline.clone()).collect::<Vec<String>>(),
        "error": st.error,
        "fiable": st.fiable(),
        "resumen": st.resumen(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prefijo temporal **unico por hilo**, con su propio padre.
    ///
    /// MEDIDO (2026-11-25, CI): los destinos `servicio/` caen en
    /// `<padre del prefijo>/service.d/`.  Con el prefijo directamente en
    /// /tmp, TODOS los hilos compartian `/tmp/service.d` => los tests se
    /// pisaban (uno escribia, otro veia el hash ya correcto y contaba 5 de
    /// 9) y uno BORRABA el directorio del otro.  Ahora cada hilo tiene su
    /// propio arbol.
    fn prefijo_temporal() -> String {
        let raiz = std::env::temp_dir().join(format!(
            "megadaemon-stack-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let prefijo = raiz.join("prefijo");
        std::fs::create_dir_all(&prefijo).expect("temp");
        prefijo.to_string_lossy().to_string()
    }

    // --- Los supervisores: el argv manda, no la subcadena ---------------
    //
    // MEDIDO (2026-11-28, movil real, build 4a51e82): el diagnostico decia
    // "supervisores: 0" y el instalador "4".  Las dos cifras eran
    // FALSOS POSITIVOS del comando que cuenta (su propio `cmdline` contiene
    // el patron) y ningun supervisor estaba vivo de verdad.

    /// Escribe un `cmdline` de mentira (los elementos van separados por `\0`).
    fn escribir_cmdline(dir: &Path, pid: i32, argv: &[&str]) {
        let d = dir.join(pid.to_string());
        let _ = std::fs::create_dir_all(&d);
        let mut bytes: Vec<u8> = Vec::new();
        for a in argv {
            bytes.extend_from_slice(a.as_bytes());
            bytes.push(0);
        }
        let _ = std::fs::write(d.join("cmdline"), &bytes);
    }

    /// Un `/proc` de mentira, único por prueba.
    fn proc_de_mentira(nombre: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "megadaemon-sup-{nombre}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp");
        dir
    }

    /// MEDIDO (2026-11-28, movil): el argv real del supervisor es
    /// `busybox\0sh\0/data/adb/service.d/99-megadaemon.sh\0` ⇒ el
    /// canónico se reconoce ELEMENTO A ELEMENTO.
    #[test]
    fn el_supervisor_canonico_se_reconoce_elemento_a_elemento() {
        let dir = proc_de_mentira("canonico");
        escribir_cmdline(
            &dir,
            100,
            &[
                "/debug_ramdisk/.magisk/busybox/busybox",
                "sh",
                RUTA_SUPERVISOR_CANONICO,
            ],
        );
        let st = estado_supervisores_en(&dir);
        assert_eq!(st.canonicos.len(), 1, "no vio el canonico: {st:?}");
        assert_eq!(st.no_canonicos.len(), 0);
        assert_eq!(st.total(), 1);
        assert_eq!(st.canonicos[0].pid, 100);
        assert!(st.hay_canonico());
        assert!(supervisor_vivo_en(&dir));
        assert!(st.fiable());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// MEDIDO (2026-11-28, movil): la copia `.bak-50k` tiene la ruta
    /// canónica como PREFIJO ⇒ con subcadena pasaba por canónica y el
    /// canónico de verdad no se relanzaba nunca.
    #[test]
    fn la_copia_bak_no_es_el_canonico() {
        let dir = proc_de_mentira("bak");
        escribir_cmdline(
            &dir,
            100,
            &[
                "busybox",
                "sh",
                "/data/adb/service.d/99-megadaemon.sh.bak-50k",
            ],
        );
        let st = estado_supervisores_en(&dir);
        assert_eq!(st.canonicos.len(), 0, "la copia NO es el canonico: {st:?}");
        assert_eq!(st.no_canonicos.len(), 1);
        assert_eq!(st.total(), 1, "pero sigue siendo un supervisor vivo");
        assert!(
            !supervisor_vivo_en(&dir),
            "una copia NO puede hacer de canonico (el movil se queda sin supervisor)"
        );
        // Con el canónico de verdad al lado, el canónico manda.
        escribir_cmdline(&dir, 200, &["busybox", "sh", RUTA_SUPERVISOR_CANONICO]);
        let st2 = estado_supervisores_en(&dir);
        assert_eq!(st2.canonicos.len(), 1);
        assert_eq!(st2.no_canonicos.len(), 1);
        assert_eq!(st2.total(), 2);
        assert!(supervisor_vivo_en(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// MEDIDO (2026-11-28, movil): el comando que cuenta los supervisores
    /// tiene el patron en SU PROPIO `cmdline` ⇒ se contaba a si mismo.  El
    /// diagnostico de las 11:50:38 (build 4a51e82) decia "1" y ese "1" era
    /// el `grep -lsa 99-megadaemon.sh ...` del instalador.
    #[test]
    fn el_comando_que_cuenta_no_cuenta_a_si_mismo() {
        let dir = proc_de_mentira("autoconteo");
        escribir_cmdline(
            &dir,
            100,
            &["grep", "-lsa", "99-megadaemon.sh", "/proc/[0-9]*/cmdline"],
        );
        escribir_cmdline(
            &dir,
            101,
            &[
                "su",
                "-mm",
                "-c",
                "grep -lsa 99-megadaemon.sh /proc/[0-9]*/cmdline | wc -l",
            ],
        );
        escribir_cmdline(
            &dir,
            102,
            &[
                "sh",
                "-c",
                "grep -lsa 99-megadaemon.sh /proc/[0-9]*/cmdline | wc -l",
            ],
        );
        escribir_cmdline(&dir, 103, &["wc", "-l"]);
        let st = estado_supervisores_en(&dir);
        assert_eq!(
            st.total(),
            0,
            "el comando que cuenta no es un supervisor: {st:?}"
        );
        assert_eq!(numero_supervisores_en(&dir), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// MEDIDO (2026-11-28, movil): `unwrap_or_default()` se tragaba el error
    /// de lectura ⇒ "no se pudo leer" era INDISTINGUIBLE de "no hay".
    #[test]
    fn un_cmdline_ilegible_no_es_cero_supervisores() {
        let dir = proc_de_mentira("ilegible");
        // Un "cmdline" que es un DIRECTORIO: la lectura falla (EISDIR)
        // con cualquier usuario, incluso root.
        let d = dir.join("100");
        std::fs::create_dir_all(&d).expect("temp");
        std::fs::create_dir_all(d.join("cmdline")).expect("cmdline");
        let st = estado_supervisores_en(&dir);
        assert_eq!(
            st.ilegibles.len(),
            1,
            "el fallo de lectura tiene que verse: {st:?}"
        );
        assert_eq!(st.total(), 0);
        assert!(
            !st.fiable(),
            "con algo ilegible el conteo no es concluyente"
        );
        assert!(
            st.resumen().contains("ilegible"),
            "resumen: {}",
            st.resumen()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Un proceso que se va mientras lo miramos (ENOENT) NO es un fallo de
    /// lectura: es una carrera normal y no puede volver "no concluyente".
    #[test]
    fn un_proceso_que_se_va_no_es_ilegible() {
        let dir = proc_de_mentira("enoent");
        // Un "cmdline" que es un symlink roto: la lectura da ENOENT.
        let d = dir.join("100");
        std::fs::create_dir_all(&d).expect("temp");
        std::os::unix::fs::symlink("/no/existe/cmdline", d.join("cmdline")).expect("symlink");
        let st = estado_supervisores_en(&dir);
        assert_eq!(st.ilegibles.len(), 0, "ENOENT no es ilegible: {st:?}");
        assert!(st.fiable(), "el conteo sigue siendo concluyente");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Si `/proc` no se puede listar, el estado lo DICE (no inventa un cero
    /// limpio).
    #[test]
    fn si_no_se_puede_listar_proc_no_se_inventa_un_cero() {
        let dir = proc_de_mentira("nopro");
        let _ = std::fs::remove_dir_all(&dir);
        let st = estado_supervisores_en(&dir);
        assert!(st.error.is_some(), "tiene que decir que no pudo leer");
        assert_eq!(st.total(), 0);
        assert!(!st.fiable());
        assert!(
            st.resumen().contains("no se pudo leer"),
            "resumen: {}",
            st.resumen()
        );
        // Y `supervisor_vivo` NO puede dar un "si" falso: da "no" => se relanza.
        assert!(!supervisor_vivo_en(&dir));
    }

    /// El JSON del diagnóstico dice LAS TRES cosas: canónicos, no canónicos
    /// y si se ha podido leer.
    #[test]
    fn el_json_del_estado_dice_los_tres_cosas() {
        let dir = proc_de_mentira("json");
        escribir_cmdline(&dir, 100, &["busybox", "sh", RUTA_SUPERVISOR_CANONICO]);
        escribir_cmdline(
            &dir,
            101,
            &[
                "busybox",
                "sh",
                "/data/adb/service.d/99-megadaemon.sh.bak-50k",
            ],
        );
        let st = estado_supervisores_en(&dir);
        let j = estado_supervisores_json_de(&st);
        assert_eq!(j["canonicos"].as_u64().unwrap(), 1);
        assert_eq!(j["no_canonicos"].as_u64().unwrap(), 1);
        assert_eq!(j["total"].as_u64().unwrap(), 2);
        assert_eq!(j["fiable"].as_bool().unwrap(), true);
        assert!(j["error"].is_null());
        assert_eq!(j["pids_canonicos"][0].as_i64().unwrap(), 100);
        assert_eq!(j["pids_no_canonicos"][0].as_i64().unwrap(), 101);
    }

    #[test]
    fn el_manifiesto_no_esta_vacio() {
        assert!(!STACK.is_empty(), "el stack no puede estar vacio");
        assert!(STACK.len() >= 5, "faltan entradas: {}", STACK.len());
    }

    #[test]
    fn los_contenidos_no_estan_vacios() {
        for e in STACK {
            assert!(
                e.contenido.len() > 100,
                "{} esta vacio o es un resto",
                e.origen
            );
        }
    }

    /// El watchdog del túnel tiene que estar en el stack: `98-cloudflared.sh`
    /// lo LANZA (sin él, el túnel se queda sin vigilante y el móvil se queda
    /// inalcanzable cuando cloudflared muere).
    ///
    /// MEDIDO (2026-11-26): estaba fuera del stack ⇒ la auto-actualización
    /// del daemon nunca lo refrescaba.
    #[test]
    fn el_watchdog_del_tunel_esta_en_el_stack() {
        let w = STACK
            .iter()
            .find(|e| e.origen == "deploy/tunnel-watchdog.sh")
            .expect("el watchdog del tunel esta en el stack");
        assert!(
            w.contenido.contains("publicar_repo"),
            "el watchdog embebido no publica el canal en el repo"
        );
        assert!(
            matches!(w.destino, Destino::Prefijo(_)),
            "el watchdog va al prefijo (lo lanza 98-cloudflared.sh desde ahi)"
        );
    }

    /// El supervisor embebido DEBE usar el prefijo persistente.
    ///
    /// MEDIDO (2026-11-25): el bug REAL del móvil era un supervisor viejo
    /// con `PREFIX=/data/local/tmp/megadaemon` ⇒ no encontraba el binario
    /// y no podía relanzar el daemon.  Este test es la red de seguridad.
    #[test]
    fn el_supervisor_embebido_usa_el_prefijo_persistente() {
        let sup = STACK
            .iter()
            .find(|e| e.origen == "deploy/99-megadaemon.sh")
            .expect("el supervisor esta en el stack");
        // MEDIDO (2026-11-25, CI): el assert buscar la subcadena
        // "PREFIX=/data/local/tmp/megadaemon" saltaba porque el supervisor
        // tiene `LEGACY_PREFIX=/data/local/tmp/megadaemon` (que es
        // CORRECTO: es la ruta que se migra).  Se comprueba la linea
        // completa, al principio de linea.
        assert!(
            sup.contenido.contains("\nPREFIX=/data/adb/megadaemon"),
            "el supervisor embebido NO usa el prefijo persistente"
        );
        assert!(
            !sup.contenido
                .contains("\nPREFIX=/data/local/tmp/megadaemon"),
            "el supervisor embebido usa la ruta volatil como PREFIX"
        );
    }

    #[test]
    fn los_destinos_se_resuelven_bajo_el_prefijo() {
        let p = prefijo_temporal();
        let padre = Path::new(&p)
            .parent()
            .unwrap()
            .to_string_lossy()
            .to_string();
        for e in STACK {
            let r = e.ruta(&p);
            let s = r.to_string_lossy().to_string();
            match e.destino {
                // Los servicios de Magisk viven en el PADRE del prefijo
                // (/data/adb/service.d), no dentro del prefijo.
                Destino::Servicio(_) => {
                    assert!(
                        s.starts_with(&padre) && s.contains("/service.d/"),
                        "{} deberia ir a service.d: {s}",
                        e.origen
                    );
                }
                _ => {
                    assert!(s.starts_with(&p), "{} se sale del prefijo: {s}", e.origen);
                }
            }
        }
        let _ = std::fs::remove_dir_all(Path::new(&p).parent().unwrap());
    }

    #[test]
    fn reconciliar_escribe_y_es_idempotente() {
        let p = prefijo_temporal();
        let cfg = Config {
            prefijo: p.clone(),
            android: true,
        };
        let inf = reconciliar(&cfg);
        assert!(inf.ok(), "fallos: {:?}", inf.fallidos);
        assert_eq!(inf.escritos.len(), STACK.len(), "deberia escribir todo");
        assert!(inf.supervisor_cambiado, "el supervisor cambio");

        // Segunda vez: no toca nada.
        let inf2 = reconciliar(&cfg);
        assert!(inf2.escritos.is_empty(), "reescribio: {:?}", inf2.escritos);
        assert_eq!(inf2.correctos.len(), STACK.len());
        assert!(!inf2.supervisor_cambiado);

        // El estado coincide.
        for e in estado(&cfg) {
            assert!(e.ok, "{} no coincide", e.ruta);
        }
        let _ = std::fs::remove_dir_all(Path::new(&p).parent().unwrap());
    }

    #[test]
    fn reconciliar_repara_un_fichero_manipulado() {
        let p = prefijo_temporal();
        let cfg = Config {
            prefijo: p.clone(),
            android: true,
        };
        reconciliar(&cfg);
        // Alguien toca el supervisor.
        let sup = Path::new(&p)
            .parent()
            .unwrap()
            .join("service.d")
            .join("99-megadaemon.sh");
        std::fs::write(&sup, "# manipulado\n").expect("escritura");
        let inf = reconciliar(&cfg);
        assert_eq!(
            inf.escritos.len(),
            1,
            "solo el manipulado: {:?}",
            inf.escritos
        );
        assert!(inf.supervisor_cambiado);
        assert_eq!(
            hash_de(&sup).as_deref(),
            Some(hash_esperado(&STACK[0]).as_str()),
            "no se reparo"
        );
        let _ = std::fs::remove_dir_all(Path::new(&p).parent().unwrap());
    }

    #[test]
    fn reconciliar_borra_las_copias_bak_del_service_d() {
        // MEDIDO (2026-11-28, movil): el `.bak` del supervisor dentro de
        // `service.d/` era un SEGUNDO supervisor (Magisk ejecuta todo lo
        // ejecutable de ahi) => bucle de reinicios y tunel mcp-legacy
        // relanzado sin parar.
        let p = prefijo_temporal();
        let cfg = Config {
            prefijo: p.clone(),
            android: true,
        };
        reconciliar(&cfg);
        let dir = Path::new(&p).parent().unwrap().join("service.d");
        let bak = dir.join("99-megadaemon.sh.bak-50k");
        std::fs::write(&bak, "#!/bin/sh\n# copia vieja\n").expect("escritura");
        let otro = dir.join("99-megadaemon.sh.otro");
        std::fs::write(&otro, "#!/bin/sh\n").expect("escritura");
        // Un fichero de OTRA app no se toca.
        let ajeno = dir.join("99-otra-app.sh");
        std::fs::write(&ajeno, "#!/bin/sh\n").expect("escritura");

        let inf = reconciliar(&cfg);
        assert!(
            inf.borrados.iter().any(|b| b.contains(".bak-50k")),
            "no se borro la copia: {:?}",
            inf.borrados
        );
        assert!(
            inf.borrados.iter().any(|b| b.contains(".otro")),
            "no se borro la copia: {:?}",
            inf.borrados
        );
        assert!(!bak.exists(), "la copia sigue en service.d");
        assert!(!otro.exists(), "la copia sigue en service.d");
        assert!(ajeno.exists(), "no se debe tocar lo de otras apps");
        // El canónico sigue ahí.
        assert!(dir.join("99-megadaemon.sh").exists());
        let _ = std::fs::remove_dir_all(Path::new(&p).parent().unwrap());
    }

    #[test]
    fn el_estado_json_es_consistente() {
        let p = prefijo_temporal();
        let cfg = Config {
            prefijo: p.clone(),
            android: true,
        };
        let v = estado_json(&cfg);
        assert_eq!(v["total"].as_u64().unwrap(), STACK.len() as u64);
        assert_eq!(v["desfasados"].as_u64().unwrap(), STACK.len() as u64);
        reconciliar(&cfg);
        let v2 = estado_json(&cfg);
        assert_eq!(v2["desfasados"].as_u64().unwrap(), 0);
        assert_eq!(v2["correctos"].as_u64().unwrap(), STACK.len() as u64);
        let _ = std::fs::remove_dir_all(Path::new(&p).parent().unwrap());
    }
}
