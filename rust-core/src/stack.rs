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
    /// ¿El supervisor cambió?  (entonces hay que reiniciarlo)
    pub supervisor_cambiado: bool,
}

impl Informe {
    /// ¿Hubo cambios?
    pub fn cambio(&self) -> bool {
        !self.escritos.is_empty() || !self.fallidos.is_empty()
    }

    /// ¿Todo correcto?
    pub fn ok(&self) -> bool {
        self.fallidos.is_empty()
    }

    /// Resumen de una línea.
    pub fn linea(&self) -> String {
        format!(
            "stack: {} escrito(s), {} ya ok, {} fallo(s)",
            self.escritos.len(),
            self.correctos.len(),
            self.fallidos.len()
        )
    }

    /// A JSON (para la Web UI y el tablón).
    pub fn a_json(&self) -> Value {
        json!({
            "escritos": self.escritos,
            "correctos": self.correctos.len(),
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
    inf
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

/// **Reinicia el supervisor** si su fichero en disco cambió.
///
/// MEDIDO (2026-11-25): el supervisor en ejecución es el que relanza el
/// daemon, el túnel y la energía.  Si se reescribe su fichero pero el
/// proceso sigue siendo el viejo, el cambio no surte efecto hasta el
/// próximo reinicio del móvil ⇒ hay que matarlo.  Lo revive el *keeper*
/// (`97-megadaemon-keeper.sh`), que comprueba cada 60 s.
///
/// Devuelve `true` si se pidió el reinicio.
pub fn reiniciar_supervisor() -> bool {
    // Patrón con corchetes: el propio comando no se mata a sí mismo.
    // MEDIDO: `pkill -f` con un patrón que aparece en la propia línea de
    // comandos se suicida ⇒ por eso se usa el truco de los corchetes.
    let salida = std::process::Command::new("pkill")
        .args(["-9", "-f", "service.d/99-megadaemo[n].sh"])
        .output();
    match salida {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

/// ¿Existe el proceso del supervisor?
pub fn supervisor_vivo() -> bool {
    for e in std::fs::read_dir("/proc").ok().into_iter().flatten() {
        let e = match e {
            Ok(e) => e,
            Err(_) => continue,
        };
        let nombre = e.file_name();
        let nombre = nombre.to_string_lossy();
        if !nombre.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let cmdline = std::fs::read(e.path().join("cmdline")).unwrap_or_default();
        let cmd = String::from_utf8_lossy(&cmdline);
        if cmd.contains("99-megadaemon.sh") {
            return true;
        }
    }
    false
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
