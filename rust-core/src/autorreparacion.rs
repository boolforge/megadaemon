//! MegaDaemon — **AUTORREPARACIÓN AUTOMÁTICA**: el daemon se arregla SOLO.
//!
//! QUÉ ES
//!   `reparar_entorno` sabe **reparar el entorno** (ficheros, binario, key,
//!   plantilla) pero hay que **llamarlo**.  Este módulo convierte esa
//!   capacidad en un **ciclo automático**: el daemon **detecta** los fallos,
//!   **se autorrepara** sin que nadie lance un script a mano, y **deja
//!   rastro** de lo que hizo.
//!
//! EL PROCESO QUE SE AUTOMATIZA
//!   El guion es el de `tools/restaurar-movil.sh` (el script de referencia):
//!   **PARAR** todo el stack (orden inverso al de arranque: keeper → watchdog
//!   → supervisor → cloudflared → daemon) → **LIMPIAR** (copias `.bak*` en
//!   `service.d`, URLs de túnel reseteadas) → **ARRANCAR** uno de cada →
//!   **VERIFICAR**.  Aquí se hace lo mismo, pero **desde dentro** y con un
//!   presupuesto para no entrar en bucle.
//!
//! MEDIDO (2026-11-28, móvil): el "pastel" de procesos (4 supervisores, 4
//! watchdogs, 5 cloudflared, 6 daemons, load1=58) **no es un residuo: se
//!   relanza**.  La causa está en el keeper: `[ -r /proc/<pid>/cmdline ] ||
//!   continue` ⇒ si el cmdline no es legible el proceso se cuenta como MUERTO
//!   ⇒ el guardian relanza el supervisor detrás del que ya está vivo.
//!
//! LAS DOS REGLAS QUE LO CAMBIAN TODO
//!   1. **Ficheros de PID, no leer cmdlines.**  La comprobación de vida es
//!      **PID + `/proc/<pid>` + señal 0**.  "No se pudo leer" **NO** puede
//!      contar como "no existe": si no, el motor de reparación sería el
//!      siguiente que multiplica los procesos.
//!   2. **Identificar el canónico comparando el argv ELEMENTO a ELEMENTO.**
//!      La copia `.bak-50k` tiene la ruta canónica como **prefijo**: un
//!      `contains` la tomaría por la canónica (y la dejaría duplicando).
//!
//! IDEMPOTENCIA
//!   Restaurar dos veces seguidas no rompe nada ni mata lo que está sano:
//!   la fase de desinfección sólo mata **duplicados**, nunca el canónico.
//!
//! EL BUCLE (lo que NO debe pasar)
//!   * **Presupuesto**: máximo de restauraciones por hora (3 por defecto).
//!   * **Marca de "ya restaurado"** con marca de tiempo: si vuelve a salir
//!     el **mismo** conjunto de síntomas dentro del enfriamiento, no se
//!     repite (si aparece un síntoma **nuevo**, sí se actúa).
//!   * **Backoff creciente** tras un fallo: 5 → 15 → 45 → 120 min.
//!   * Un fallo **no** reintenta acciones **históricas**: cada intento
//!     recalcula el plan desde el estado actual.
//!
//! Autoría: sesión root de MegaDaemon (2026-11-28).

use serde_json::{json, Value};
use std::path::Path;

/// Versión del esquema (la UI la usa para detectar deriva).
pub const VERSION: &str = "autorreparacion/v1";

/// Fichero de estado (presupuesto + registro) dentro del `state_dir`.
pub const FICHERO_ESTADO: &str = "autorreparacion.json";

/// Log de las restauraciones (en el prefijo, junto al resto del stack).
pub const FICHERO_LOG: &str = "autorreparacion.log";

/// Puerto **legacy** del MCP (0x2235).  Si algo lo escucha, sobra.
pub const PUERTO_LEGACY: u16 = 8765;

/// Script del túnel (levanta cloudflared + watchdog).
pub const NOMBRE_CLOUDFLARED: &str = "98-cloudflared.sh";

/// Script del watchdog del túnel.
pub const NOMBRE_WATCHDOG: &str = "tunnel-watchdog.sh";

/// Script del puente ADB.
pub const NOMBRE_ADB_TCP: &str = "91-adb-tcp.sh";

/// Antigüedad máxima (segundos) de la última publicación de telemetría
/// antes de dar la telemetría por **fallida**.
///
/// MEDIDO (2026-11-28): la telemetría se publica cada 15 min ⇒ 3 ciclos
/// de margen (45 min) antes de sospechar.
pub const TELEMETRIA_MAX_VIEJA_S: i64 = 45 * 60;

/// Ficheros de `service.d` cuya copia `.bak*` es **un segundo servicio**
/// (Magisk ejecuta TODO lo ejecutable de `service.d`).
pub const FICHEROS_SERVICE_D: &[&str] = &[
    "99-megadaemon.sh",
    "97-megadaemon-keeper.sh",
    "98-cloudflared.sh",
    "91-adb-tcp.sh",
];

// ---------------------------------------------------------------------------
// PROCESOS
// ---------------------------------------------------------------------------

/// Un proceso del sistema, con su argv **elemento a elemento**.
///
/// `legible == false` significa que **no se pudo leer** el `cmdline`
/// (SELinux).  Eso **NO** significa que el proceso esté muerto: la vida se
/// comprueba por PID + `/proc/<pid>` + señal 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proceso {
    /// PID.
    pub pid: i32,
    /// argv partido por NUL (vacío si no se pudo leer).
    pub argv: Vec<String>,
    /// ¿Se pudo leer el `cmdline`?
    pub legible: bool,
}

impl Proceso {
    /// Un proceso con argv conocido.
    pub fn nuevo(pid: i32, argv: &[&str]) -> Proceso {
        Proceso {
            pid,
            argv: argv.iter().map(|s| s.to_string()).collect(),
            legible: true,
        }
    }

    /// Un proceso cuyo `cmdline` **no se pudo leer** (pero que puede estar
    /// vivo: no lo confundimos con "muerto").
    pub fn ilegible(pid: i32) -> Proceso {
        Proceso {
            pid,
            argv: Vec::new(),
            legible: false,
        }
    }

    /// ¿El argv **elemento a elemento** contiene este valor exacto?
    pub fn tiene_elemento(&self, valor: &str) -> bool {
        self.argv.iter().any(|a| a == valor)
    }

    /// ¿Algún elemento **empieza** por este valor?  (la copia `.bak-50k`
    /// tiene la ruta canónica como prefijo ⇒ NO es la canónica, pero se
    /// parece).
    pub fn empieza_por(&self, valor: &str) -> bool {
        self.argv.iter().any(|a| a.starts_with(valor))
    }

    /// ¿Algún elemento **termina** en este valor?
    pub fn termina_en(&self, valor: &str) -> bool {
        self.argv.iter().any(|a| a.ends_with(valor))
    }
}

/// Un rol del stack del móvil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rol {
    /// El supervisor de Magisk (`99-megadaemon.sh`).
    Supervisor,
    /// El guardián del supervisor (`97-megadaemon-keeper.sh`).
    Keeper,
    /// El watchdog del túnel (`tunnel-watchdog.sh`).
    Watchdog,
    /// El `cloudflared` del túnel.
    Cloudflared,
    /// El daemon (`megadaemon-core`).
    Daemon,
}

impl Rol {
    /// Nombre estable (para logs y tests).
    pub fn nombre(&self) -> &'static str {
        match self {
            Rol::Supervisor => "supervisor",
            Rol::Keeper => "keeper",
            Rol::Watchdog => "watchdog",
            Rol::Cloudflared => "cloudflared",
            Rol::Daemon => "daemon",
        }
    }

    /// **Orden INVERSO al de arranque** (el de `restaurar-movil.sh`):
    /// keeper → watchdog → supervisor → cloudflared → daemon.
    ///
    /// Se para al revés porque el keeper es el que **relanza** al
    /// supervisor: si se para el supervisor antes, el keeper lo revive.
    pub fn orden_parada() -> [Rol; 5] {
        [
            Rol::Keeper,
            Rol::Watchdog,
            Rol::Supervisor,
            Rol::Cloudflared,
            Rol::Daemon,
        ]
    }

    /// ¿Es el supervisor?  (es el único con "canónico" propio).
    pub fn es_supervisor(&self) -> bool {
        matches!(self, Rol::Supervisor)
    }
}

// ---------------------------------------------------------------------------
// LA COSTURA: EL SISTEMA
// ---------------------------------------------------------------------------

/// Todo lo que el motor necesita del sistema **pasa por aquí** ⇒ el módulo
/// es testeable en un PC, sin disco y sin móvil.
pub trait Sistema {
    /// Instante actual (epoch, segundos).
    fn ahora(&self) -> i64;

    /// Lista los procesos del sistema.
    fn procesos(&self) -> Vec<Proceso>;

    /// ¿Existe `/proc/<pid>`?
    fn proc_existe(&self, pid: i32) -> bool;

    /// Envía una señal (0 = "¿existe?").  `true` = entregada.
    fn senal(&self, pid: i32, sig: i32) -> bool;

    /// Lee un fichero de texto.
    fn leer(&self, ruta: &str) -> Option<String>;

    /// Escribe un fichero (sobrescribe).
    fn escribir(&self, ruta: &str, contenido: &str, modo: u32) -> Result<(), String>;

    /// **Añade** una línea a un fichero (log).
    fn anadir(&self, ruta: &str, linea: &str) -> Result<(), String>;

    /// ¿Existe la ruta?
    fn existe(&self, ruta: &str) -> bool;

    /// ¿Es un directorio?
    fn es_dir(&self, ruta: &str) -> bool;

    /// Borra un fichero.
    fn remover(&self, ruta: &str) -> Result<(), String>;

    /// Nombres de las entradas de un directorio.
    fn listar(&self, dir: &str) -> Vec<String>;

    /// Modo de un fichero (`0o7777`).
    fn modo(&self, ruta: &str) -> Option<u32>;

    /// Puertos con **listener** (leídos de `/proc/net/tcp`).
    fn puertos_listen(&self) -> Vec<u16>;

    /// Sonda HTTP: devuelve el código de respuesta (0 = no hubo respuesta).
    fn sondear(&self, url: &str) -> Result<u16, String>;

    /// Lanza un proceso **desligado** (no muere con el padre).
    fn lanzar(&self, argv: &[&str]) -> Result<(), String>;

    /// SHA-256 de un fichero.
    fn sha256(&self, ruta: &str) -> Option<String>;

    // --- Acciones que son del INTERIOR del daemon ------------------------
    // (van por el trait para que el test pueda simularlas)

    /// Reconcilia el stack embebido (ficheros del supervisor, keeper, ...).
    fn reconciliar_stack(&self) -> Vec<String>;

    /// Repara el entorno (binario, API key, plantilla): el motor base.
    fn reparar_entorno_base(&self) -> Vec<String>;

    /// Publica la telemetría en Drive.
    fn publicar_telemetria(&self) -> Result<(), String>;

    /// Publica el canal (estado local + paste).
    fn publicar_canal(&self) -> Result<(), String>;

    /// Abre (o crea) el tablero de la instancia.
    fn abrir_tablero(&self, ruta: &str) -> Result<(), String>;

    /// SHA-256 del binario **en servicio** (la foto del arranque).
    fn sha_de_arranque(&self) -> String;

    /// Candidatas donde puede vivir `board.sqlite`.
    fn dirs_tablero(&self) -> Vec<String>;

    /// Los **eslabones** del motor base (`reparar_entorno`: prefijo,
    /// binario en servicio, supervisor, keeper, API key, credenciales,
    /// plantilla).  Van al mismo informe que los síntomas.
    fn eslabones_entorno(&self) -> Vec<Value>;

    /// Las 3 líneas del informe del motor base (qué / toca / bloquea).
    fn informe_entorno(&self) -> Vec<String>;

    /// Nº de ficheros del stack desfasados.
    fn desfasados_stack(&self) -> u32;
}

// ---------------------------------------------------------------------------
// CONFIGURACIÓN
// ---------------------------------------------------------------------------

/// Configuración del restaurador.
#[derive(Debug, Clone)]
pub struct Config {
    /// La configuración del motor base (`reparar_entorno`).
    ///
    /// OJO: el **binario en servicio** lo manda `Config::binario()` (que es
    /// `reparar_entorno::Config::binario()`): `/data/local/tmp` es VOLÁTIL
    /// y el móvil usa `/data/adb/megadaemon/`.
    pub entorno: crate::reparar_entorno::Config,
    /// Directorio de los scripts de Magisk (`service.d`).
    pub service_d: String,
    /// Ruta del script del túnel.
    pub ruta_cloudflared: String,
    /// Ruta del watchdog del túnel.
    pub ruta_watchdog: String,
    /// Ruta del puente ADB.
    pub ruta_adb_tcp: String,
    /// Puerto del cockpit (el que sirve `/health`).
    pub puerto: u16,
    /// Máximo de restauraciones **por hora**.
    pub max_por_hora: u32,
    /// Enfriamiento tras una restauración correcta (segundos).
    pub enfriamiento_s: i64,
    /// Backoff creciente tras un fallo (minutos).
    pub backoff_min: Vec<u64>,
    /// Periodo del ciclo automático (minutos).
    pub intervalo_min: u64,
}

impl Config {
    /// Configuración autodetectada.
    pub fn del_entorno() -> Config {
        // Las rutas vienen del motor base (`reparar_entorno::Config`): una
        // sola fuente de verdad (aquí no se repite ninguna).
        let entorno = crate::reparar_entorno::Config::del_entorno();
        let service_d = entorno.service_d.clone();
        let puerto = entorno.puerto;
        let ruta_cloudflared = entorno.ruta_cloudflared.clone();
        let ruta_adb_tcp = entorno.ruta_adb_tcp.clone();
        Config {
            service_d,
            ruta_cloudflared,
            ruta_watchdog: entorno.ruta_watchdog.clone(),
            ruta_adb_tcp,
            puerto,
            max_por_hora: crate::config_efectiva::u64_de("autorreparacion.max_por_hora", 3) as u32,
            enfriamiento_s: (crate::config_efectiva::u64_de("autorreparacion.enfriamiento_min", 30)
                as i64)
                * 60,
            backoff_min: vec![5, 15, 45, 120],
            intervalo_min: crate::config_efectiva::u64_de("autorreparacion.ciclo_min", 10),
            entorno,
        }
    }

    /// Ruta del **binario en servicio** (la manda el motor base).
    pub fn binario(&self) -> String {
        self.entorno.binario()
    }

    /// Ruta del supervisor **canónico**.
    pub fn ruta_supervisor(&self) -> String {
        self.entorno.ruta_supervisor.clone()
    }

    /// Ruta del keeper.
    pub fn ruta_keeper(&self) -> String {
        self.entorno.ruta_keeper.clone()
    }

    /// Ruta del estado.
    pub fn state_dir(&self) -> String {
        self.entorno.state_dir.clone()
    }

    /// Ruta del fichero de estado (presupuesto + registro).
    pub fn ruta_estado(&self) -> String {
        format!("{}/{}", self.state_dir(), FICHERO_ESTADO)
    }

    /// Ruta del log de restauraciones.
    pub fn ruta_log(&self) -> String {
        format!("{}/{}", self.entorno.prefijo, FICHERO_LOG)
    }

    /// Candidatas del fichero de URL del túnel (en este orden).
    pub fn rutas_url_tunel(&self) -> Vec<String> {
        vec![
            format!("{}/tunnel.url", self.entorno.prefijo),
            format!("{}/mcp_tunnel.url", self.entorno.prefijo),
            format!("{}/tunnel.url", self.state_dir()),
        ]
    }
}

// ---------------------------------------------------------------------------
// SÍNTOMAS
// ---------------------------------------------------------------------------

/// Un síntoma medido del móvil (2026-11-28) con su acción.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sintoma {
    /// Más de un supervisor / watchdog / cloudflared / daemon.
    MultiplicacionProcesos,
    /// `tunnel.url` es la URL de la API (o está vacía).
    UrlTunelInvalida,
    /// El túnel no responde (sonda HTTP 0).
    TunelCaido,
    /// El 8765 (legacy) tiene listener ⇒ sobra.
    PuertoLegacyVivo,
    /// El daemon no responde en `/health`.
    DaemonSinHealth,
    /// El binario en servicio ≠ el de disco (build desfasado).
    BinarioDesfasado,
    /// La telemetría no se publica (credenciales ausentes/0600 o falla).
    TelemetriaNoPublicada,
    /// El canal no está publicado (repo/relé/paste/Drive).
    CanalNoPublicado,
    /// La base de la instancia no existe (`board.sqlite`).
    BaseInstanciaAusente,
    /// El stack está desfasado (`stack --json` con `desfasados > 0`).
    StackDesfasado,
    /// Faltan ficheros del stack en `service.d` (o el watchdog/guardián).
    FicherosStackAusentes,
}

impl Sintoma {
    /// Todos los síntomas (en el orden de la lista del jefe).
    pub fn todos() -> [Sintoma; 11] {
        [
            Sintoma::MultiplicacionProcesos,
            Sintoma::UrlTunelInvalida,
            Sintoma::TunelCaido,
            Sintoma::PuertoLegacyVivo,
            Sintoma::DaemonSinHealth,
            Sintoma::BinarioDesfasado,
            Sintoma::TelemetriaNoPublicada,
            Sintoma::CanalNoPublicado,
            Sintoma::BaseInstanciaAusente,
            Sintoma::StackDesfasado,
            Sintoma::FicherosStackAusentes,
        ]
    }

    /// Nombre estable (para logs, informes y tests).
    pub fn nombre(&self) -> &'static str {
        match self {
            Sintoma::MultiplicacionProcesos => "multiplicacion_procesos",
            Sintoma::UrlTunelInvalida => "url_tunel_invalida",
            Sintoma::TunelCaido => "tunel_caido",
            Sintoma::PuertoLegacyVivo => "puerto_8765_vivo",
            Sintoma::DaemonSinHealth => "daemon_sin_health",
            Sintoma::BinarioDesfasado => "binario_desfasado",
            Sintoma::TelemetriaNoPublicada => "telemetria_no_publicada",
            Sintoma::CanalNoPublicado => "canal_no_publicado",
            Sintoma::BaseInstanciaAusente => "base_instancia_ausente",
            Sintoma::StackDesfasado => "stack_desfasado",
            Sintoma::FicherosStackAusentes => "ficheros_stack_ausentes",
        }
    }

    /// Qué pasa (el nombre del síntoma, en palabras).
    pub fn que(&self) -> &'static str {
        match self {
            Sintoma::MultiplicacionProcesos => "hay mas de un proceso por rol (pastel de procesos)",
            Sintoma::UrlTunelInvalida => "la URL del tunel no es un tunel (o esta vacia)",
            Sintoma::TunelCaido => "el tunel no responde",
            Sintoma::PuertoLegacyVivo => "el 8765 (legacy) tiene listener: sobra",
            Sintoma::DaemonSinHealth => "el daemon no responde en /health",
            Sintoma::BinarioDesfasado => {
                "el binario en servicio no es el de disco (build desfasado)"
            }
            Sintoma::TelemetriaNoPublicada => "la telemetria no se publica",
            Sintoma::CanalNoPublicado => "el canal no esta publicado: la flota se queda ciega",
            Sintoma::BaseInstanciaAusente => "la base de la instancia no existe",
            Sintoma::StackDesfasado => "el stack esta desfasado",
            Sintoma::FicherosStackAusentes => "faltan ficheros del stack en service.d",
        }
    }

    /// Qué hace el motor para arreglarlo.
    pub fn toca(&self) -> &'static str {
        match self {
            Sintoma::MultiplicacionProcesos => {
                "matar los duplicados (NO el canonico) y dejar UNO de cada rol"
            }
            Sintoma::UrlTunelInvalida => {
                "resetear las URLs del tunel (las reescribe el supervisor al arrancar)"
            }
            Sintoma::TunelCaido => {
                "relanzar el tunel (98-cloudflared.sh) y volver a publicar la URL"
            }
            Sintoma::PuertoLegacyVivo => "parar el listener del 8765 (el MCP legacy sobra)",
            Sintoma::DaemonSinHealth => "reiniciar el daemon (lo revive el supervisor)",
            Sintoma::BinarioDesfasado => "reiniciar el daemon para que cargue el binario de disco",
            Sintoma::TelemetriaNoPublicada => {
                "arreglar el modo de las credenciales (0600) y republicar la telemetria"
            }
            Sintoma::CanalNoPublicado => "republicar el canal (estado local + paste)",
            Sintoma::BaseInstanciaAusente => "abrir (crear) el tablero de la instancia",
            Sintoma::StackDesfasado => "reconciliar el stack (reescribir lo que difiere)",
            Sintoma::FicherosStackAusentes => "instalar los ficheros que faltan del stack",
        }
    }
}

/// El veredicto de un síntoma.
#[derive(Debug, Clone)]
pub struct Diagnostico {
    /// Qué síntoma.
    pub sintoma: Sintoma,
    /// ¿Está presente?
    pub presente: bool,
    /// ¿El daemon puede **arreglarlo solo**?
    ///
    /// No todos los fallos se pueden arreglar sin una persona: si faltan
    /// las credenciales de Drive, el daemon **no puede inventarlas**
    /// (`reparar_entorno` lo dice: "el daemon no puede inventarlas").
    /// Ese síntoma se INFORMA pero no dispara una restauración ⇒ no
    /// consume presupuesto ni entra en bucle.
    pub accionable: bool,
    /// Detalle (cifras, rutas, PIDs).
    pub detalle: String,
}

impl Diagnostico {
    fn nuevo(sintoma: Sintoma, presente: bool, detalle: impl Into<String>) -> Diagnostico {
        Diagnostico {
            sintoma,
            presente,
            accionable: true,
            detalle: detalle.into(),
        }
    }

    /// Un síntoma que el daemon **no** puede arreglar solo.
    fn no_accionable(sintoma: Sintoma, presente: bool, detalle: impl Into<String>) -> Diagnostico {
        Diagnostico {
            sintoma,
            presente,
            accionable: false,
            detalle: detalle.into(),
        }
    }

    /// JSON del veredicto.
    pub fn a_json(&self) -> Value {
        json!({
            "sintoma": self.sintoma.nombre(),
            "presente": self.presente,
            "accionable": self.accionable,
            "que": self.sintoma.que(),
            "toca": self.sintoma.toca(),
            "detalle": self.detalle,
        })
    }
}

// ---------------------------------------------------------------------------
// LA URL DEL TÚNEL
// ---------------------------------------------------------------------------

/// ¿Es esta URL un **túnel** válido?
///
/// MEDIDO (2026-11-23, móvil): el `grep` cogía `https://api.trycloudflare.com`
/// (la URL de la **API** que el propio cloudflared escribe en su log) ⇒ el
/// fichero `tunnel.url` guardaba una URL que **no** es el túnel ⇒ el móvil se
/// quedaba "sin URL pública".
///
/// Criterio (el de `deploy/99-megadaemon.sh:500-511`): host `*.trycloudflare.com`
/// que **no** sea `api.` ni `developers.`.
pub fn url_tunel_valida(url: &str) -> bool {
    motivo_url_tunel(url).is_empty()
}

/// Por qué la URL **no** es válida (vacío = válida).
pub fn motivo_url_tunel(url: &str) -> String {
    let u = url.trim();
    if u.is_empty() {
        return "la URL esta vacia".to_string();
    }
    let Some(host) = host_de(u) else {
        return format!("{u} no es una URL http(s)");
    };
    if !host.ends_with(".trycloudflare.com") {
        return format!("{host} no es un host de tunel rapido (*.trycloudflare.com)");
    }
    let primero = host.split('.').next().unwrap_or("");
    if primero == "api" {
        return format!("{host} es la URL de la API, no un tunel");
    }
    if primero == "developers" {
        return format!("{host} es la URL de developers, no un tunel");
    }
    String::new()
}

/// Host de una URL (sin esquema, puerto ni ruta).
fn host_de(url: &str) -> Option<String> {
    let resto = match url.split_once("://") {
        Some((_, r)) => r,
        None => return None,
    };
    let host = resto.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.split('@').next_back().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    if host.is_empty() {
        None
    } else {
        Some(host.to_ascii_lowercase())
    }
}

// ---------------------------------------------------------------------------
// IDENTIFICAR LOS PROCESOS
// ---------------------------------------------------------------------------

/// ¿Este proceso es el **supervisor canónico**?
///
/// Se compara el argv **elemento a elemento**: la copia `.bak-50k` tiene la
/// ruta canónica como **prefijo**, así que `contains` la tomaría por la
/// canónica (y dejaría el pastel de supervisores).
pub fn es_supervisor_canonico(p: &Proceso, ruta_canonica: &str) -> bool {
    p.tiene_elemento(ruta_canonica)
}

/// ¿Este proceso es un supervisor **NO canónico** (la copia `.bak*`)?
pub fn es_supervisor_no_canonico(p: &Proceso, ruta_canonica: &str) -> bool {
    if es_supervisor_canonico(p, ruta_canonica) {
        return false;
    }
    p.empieza_por(ruta_canonica) || p.termina_en("99-megadaemon.sh.bak")
}

/// ¿De qué rol es este proceso?  (`None` = no es del stack).
pub fn rol_de(p: &Proceso, cfg: &Config) -> Option<Rol> {
    if es_supervisor_canonico(p, &cfg.ruta_supervisor())
        || es_supervisor_no_canonico(p, &cfg.ruta_supervisor())
    {
        return Some(Rol::Supervisor);
    }
    if p.termina_en("97-megadaemon-keeper.sh") {
        return Some(Rol::Keeper);
    }
    if p.termina_en(NOMBRE_WATCHDOG) {
        return Some(Rol::Watchdog);
    }
    if p.termina_en("cloudflared") {
        return Some(Rol::Cloudflared);
    }
    if p.termina_en("megadaemon-core") {
        return Some(Rol::Daemon);
    }
    None
}

/// ¿El proceso **está vivo**?
///
/// MEDIDO (2026-11-28): la comprobación es **PID + `/proc/<pid>` + señal 0**.
/// "No se pudo leer el cmdline" **NO** cuenta como "no existe": si contara,
/// el motor de reparación sería el siguiente que multiplica los procesos.
pub fn vivo(sis: &dyn Sistema, pid: i32) -> bool {
    sis.proc_existe(pid) && sis.senal(pid, 0)
}

/// Los procesos de un rol **que están vivos**.
pub fn vivos_del_rol(sis: &dyn Sistema, cfg: &Config, rol: Rol) -> Vec<Proceso> {
    sis.procesos()
        .into_iter()
        .filter(|p| rol_de(p, cfg) == Some(rol))
        .filter(|p| vivo(sis, p.pid))
        .collect()
}

// ---------------------------------------------------------------------------
// EL PRESUPUESTO (el "no entrar en bucle")
// ---------------------------------------------------------------------------

/// El estado persistente: presupuesto + registro de restauraciones.
#[derive(Debug, Clone, Default)]
pub struct Estado {
    /// Las restauraciones (historial).  Cada una dice qué la disparó.
    pub restauraciones: Vec<Restauracion>,
    /// Fallos **consecutivos** (⇒ backoff creciente).
    pub fallos_consecutivos: u32,
    /// Instante del último intento (haya sido o no un éxito).
    pub ultimo_intento: i64,
    /// Instante del último **éxito**.
    pub ultimo_exito: i64,
    /// Instante a partir del cual se puede volver a restaurar.
    pub proximo_permitido: i64,
}

/// Una restauración (una entrada del registro).
#[derive(Debug, Clone)]
pub struct Restauracion {
    /// Cuándo.
    pub ts: i64,
    /// Qué síntomas la Propagation.
    pub sintomas: Vec<String>,
    /// Qué se hizo.
    pub acciones: Vec<String>,
    /// Resultado: `ok` | `fallo` | `presupuesto`.
    pub resultado: String,
    /// Cuánto tardó.
    pub duracion_s: i64,
}

impl Restauracion {
    fn a_json(&self) -> Value {
        json!({
            "ts": self.ts,
            "sintomas": self.sintomas,
            "acciones": self.acciones,
            "resultado": self.resultado,
            "duracion_s": self.duracion_s,
        })
    }
}

/// Carga el estado del disco (tolerante: si no hay, estado vacío).
pub fn cargar_estado(sis: &dyn Sistema, ruta: &str) -> Estado {
    let Some(raw) = sis.leer(ruta) else {
        return Estado::default();
    };
    let Ok(v) = serde_json::from_str::<Value>(&raw) else {
        return Estado::default();
    };
    let mut e = Estado::default();
    if let Some(arr) = v.get("restauraciones").and_then(|x| x.as_array()) {
        for r in arr {
            e.restauraciones.push(Restauracion {
                ts: r.get("ts").and_then(|x| x.as_i64()).unwrap_or(0),
                sintomas: r
                    .get("sintomas")
                    .and_then(|x| x.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|s| s.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
                acciones: r
                    .get("acciones")
                    .and_then(|x| x.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|s| s.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
                resultado: r
                    .get("resultado")
                    .and_then(|x| x.as_str())
                    .unwrap_or("ok")
                    .to_string(),
                duracion_s: r.get("duracion_s").and_then(|x| x.as_i64()).unwrap_or(0),
            });
        }
    }
    e.fallos_consecutivos = v
        .get("fallos_consecutivos")
        .and_then(|x| x.as_u64())
        .unwrap_or(0) as u32;
    e.ultimo_intento = v
        .get("ultimo_intento")
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    e.ultimo_exito = v.get("ultimo_exito").and_then(|x| x.as_i64()).unwrap_or(0);
    e.proximo_permitido = v
        .get("proximo_permitido")
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    e
}

/// Guarda el estado en disco.
pub fn guardar_estado(sis: &dyn Sistema, ruta: &str, e: &Estado) -> Result<(), String> {
    let v = json!({
        "version": VERSION,
        "restauraciones": e.restauraciones.iter().map(|r| r.a_json()).collect::<Vec<_>>(),
        "fallos_consecutivos": e.fallos_consecutivos,
        "ultimo_intento": e.ultimo_intento,
        "ultimo_exito": e.ultimo_exito,
        "proximo_permitido": e.proximo_permitido,
    });
    sis.escribir(
        ruta,
        &format!("{}\n", serde_json::to_string_pretty(&v).unwrap_or_default()),
        0o600,
    )
}

/// Por qué **no** se puede restaurar ahora (vacío = se puede).
pub fn motivo_bloqueo(
    sis: &dyn Sistema,
    cfg: &Config,
    e: &Estado,
    sintomas: &[Sintoma],
    ahora: i64,
) -> String {
    let _ = sis;
    // 1) backoff por fallos consecutivos.
    if e.fallos_consecutivos > 0 && ahora < e.proximo_permitido {
        return format!(
            "backoff: {} fallo(s) consecutivo(s); faltan {} s",
            e.fallos_consecutivos,
            e.proximo_permitido - ahora
        );
    }

    // 2) presupuesto por hora.
    let hora = ahora - 3600;
    let n = e.restauraciones.iter().filter(|r| r.ts >= hora).count() as u32;
    if n >= cfg.max_por_hora {
        return format!(
            "presupuesto: {n} restauracion(es) en la ultima hora (tope {})",
            cfg.max_por_hora
        );
    }

    // 3) la marca de "ya restaurado" (enfriamiento).
    if ahora < e.proximo_permitido {
        // Si el conjunto de síntomas es el MISMO que el de la última
        // restauración correcta, no se repite: es el bucle que hay que
        // evitar.  Si son síntomas **nuevos**, se actúa (es un problema
        // distinto).
        let mismos = e
            .restauraciones
            .iter()
            .rev()
            .find(|r| r.resultado == "ok")
            .map(|r| {
                let previos: Vec<&str> = r.sintomas.iter().map(|s| s.as_str()).collect();
                let nuevos: Vec<&str> = sintomas.iter().map(|s| s.nombre()).collect();
                !previos.is_empty()
                    && previos.len() == nuevos.len()
                    && previos.iter().all(|p| nuevos.contains(p))
            })
            .unwrap_or(false);
        if mismos {
            return format!(
                "ya restaurado: los mismos sintomas otra vez; faltan {} s",
                e.proximo_permitido - ahora
            );
        }
    }
    String::new()
}

// ---------------------------------------------------------------------------
// EL DIAGNÓSTICO
// ---------------------------------------------------------------------------

/// Diagnostica **todos** los síntomas medidos.
pub fn diagnosticar(sis: &dyn Sistema, cfg: &Config) -> Vec<Diagnostico> {
    let mut v = Vec::new();

    // --- 1) MULTIPLICACIÓN DE PROCESOS ---------------------------------
    let mut detalle = String::new();
    let mut multiplicado = false;
    for rol in [
        Rol::Supervisor,
        Rol::Keeper,
        Rol::Watchdog,
        Rol::Cloudflared,
        Rol::Daemon,
    ] {
        let vs = vivos_del_rol(sis, cfg, rol);
        if vs.len() > 1 {
            multiplicado = true;
            let pids: Vec<String> = vs.iter().map(|p| p.pid.to_string()).collect();
            detalle.push_str(&format!(
                "{}: {} vivos ({}); ",
                rol.nombre(),
                vs.len(),
                pids.join(", ")
            ));
        }
    }
    v.push(Diagnostico::nuevo(
        Sintoma::MultiplicacionProcesos,
        multiplicado,
        if detalle.is_empty() {
            "un proceso por rol".to_string()
        } else {
            detalle
        },
    ));

    // --- 2) URL DEL TÚNEL INVÁLIDA ---------------------------------------
    let url = leer_url_tunel(sis, cfg);
    let motivo = motivo_url_tunel(&url);
    v.push(Diagnostico::nuevo(
        Sintoma::UrlTunelInvalida,
        !motivo.is_empty(),
        if url.trim().is_empty() {
            "no hay URL de tunel".to_string()
        } else {
            format!("{url} ({motivo})")
        },
    ));

    // --- 3) EL TÚNEL NO RESPONDE ----------------------------------------
    let (caido, detalle) = if motivo.is_empty() {
        match sis.sondear(&url) {
            Ok(code) => {
                let muerto = crate::channel::es_canal_muerto(code);
                (muerto, format!("sonda {url} -> HTTP {code}"))
            }
            Err(e) => (true, format!("sonda {url} fallo: {e}")),
        }
    } else {
        (false, "no se sondea: la URL no es valida".to_string())
    };
    v.push(Diagnostico::nuevo(Sintoma::TunelCaido, caido, detalle));

    // --- 4) EL 8765 (LEGACY) TIENE LISTENER ------------------------------
    let puertos = sis.puertos_listen();
    let legacy = puertos.contains(&PUERTO_LEGACY);
    v.push(Diagnostico::nuevo(
        Sintoma::PuertoLegacyVivo,
        legacy,
        format!(
            "8765 {} en LISTEN (puertos: {})",
            if legacy { "esta" } else { "no esta" },
            puertos
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    ));

    // --- 5a) EL DAEMON NO RESPONDE EN /health -----------------------------
    let url_health = format!("http://127.0.0.1:{}/health", cfg.puerto);
    let (sin_health, detalle) = match sis.sondear(&url_health) {
        Ok(code) => (code != 200, format!("sonda {url_health} -> HTTP {code}")),
        Err(e) => (true, format!("sonda {url_health} fallo: {e}")),
    };
    v.push(Diagnostico::nuevo(
        Sintoma::DaemonSinHealth,
        sin_health,
        detalle,
    ));

    // --- 5b) EL BINARIO EN SERVICIO != EL DE DISCO -----------------------
    let sha_servicio = sis.sha_de_arranque();
    let sha_disco = sis.sha256(&cfg.binario());
    let desfasado = match (sha_servicio.as_str(), sha_disco.as_deref()) {
        ("desconocido", _) => false,
        (_, None) => false,
        (a, Some(b)) => a != b,
    };
    v.push(Diagnostico::nuevo(
        Sintoma::BinarioDesfasado,
        desfasado,
        format!(
            "en servicio: {} | en disco: {} | binario: {}",
            &sha_servicio[..sha_servicio.len().min(16)],
            sha_disco
                .as_deref()
                .map(|s| &s[..s.len().min(16)])
                .unwrap_or("?"),
            cfg.binario()
        ),
    ));

    // --- 6) LA TELEMETRÍA NO SE PUBLICA ----------------------------------
    // MEDIDO (2026-11-28): el diagnóstico NO publica nada (subir a Drive en
    // cada diagnóstico es caro yfulminoso).  Se mira:
    //   * las credenciales (¿están? ¿con el modo 0600?);
    //   * el índice local: cuándo se publicó la última vez (si es viejo,
    //     la publicación está fallando).
    let creds = format!("{}/gdrive_credentials.json", cfg.state_dir());
    let modo = sis.modo(&creds);
    let (sin_creds, creds_mal) = match modo {
        None => (true, false),
        Some(m) => (false, m != 0o600),
    };
    let diag_telemetria = if sin_creds {
        // Sin credenciales el daemon NO puede inventarlas: se informa,
        // pero no se dispara una restauración (no se puede arreglar solo).
        Diagnostico::no_accionable(
            Sintoma::TelemetriaNoPublicada,
            true,
            format!("no hay credenciales de Drive en {creds}"),
        )
    } else if creds_mal {
        Diagnostico::nuevo(
            Sintoma::TelemetriaNoPublicada,
            true,
            format!(
                "las credenciales de Drive estan con modo {:o} (deben ser 0600)",
                modo.unwrap_or(0)
            ),
        )
    } else {
        // ¿Cuándo se publicó la última vez?
        let indice = crate::telemetria::ruta_indice(Some(&cfg.state_dir()));
        let ultima = sis
            .leer(&indice.display().to_string())
            .and_then(|c| serde_json::from_str::<Value>(&c).ok())
            .and_then(|v| v.get("ts").and_then(|x| x.as_i64()));
        match ultima {
            Some(ts) if sis.ahora() - ts <= TELEMETRIA_MAX_VIEJA_S => {
                Diagnostico::nuevo(
                    Sintoma::TelemetriaNoPublicada,
                    false,
                    format!("telemetria publicada hace {} s", sis.ahora() - ts),
                )
            }
            Some(ts) => Diagnostico::nuevo(
                Sintoma::TelemetriaNoPublicada,
                true,
                format!(
                    "la ultima publicacion de telemetria es de hace {} s (deberia ser <= {TELEMETRIA_MAX_VIEJA_S} s)",
                    sis.ahora() - ts
                ),
            ),
            None => Diagnostico::nuevo(
                Sintoma::TelemetriaNoPublicada,
                true,
                format!("no hay indice de telemetria en {}", indice.display()),
            ),
        }
    };
    v.push(diag_telemetria);

    // --- 7) EL CANAL NO ESTÁ PUBLICADO -----------------------------------
    let (canal_roto, detalle) = match crate::channel::read_channel_json(Some(&cfg.state_dir())) {
        Some(c) => {
            let quick = c.get("quick").and_then(|x| x.as_str()).unwrap_or("");
            let paste = c.get("paste").and_then(|x| x.as_str()).unwrap_or("");
            if quick.is_empty() || paste.is_empty() {
                (
                    true,
                    format!("el canal no esta completo (quick={quick:?}, paste={paste:?})"),
                )
            } else {
                (
                    false,
                    format!("canal publicado (quick={quick}, paste={paste})"),
                )
            }
        }
        None => (
            true,
            "no hay channel.json: el canal no esta publicado".to_string(),
        ),
    };
    v.push(Diagnostico::nuevo(
        Sintoma::CanalNoPublicado,
        canal_roto,
        detalle,
    ));

    // --- 8a) LA BASE DE LA INSTANCIA NO EXISTE ----------------------------
    let dirs = sis.dirs_tablero();
    let base = dirs
        .iter()
        .map(|d| format!("{d}/board.sqlite"))
        .find(|r| sis.existe(r));
    v.push(Diagnostico::nuevo(
        Sintoma::BaseInstanciaAusente,
        base.is_none(),
        match base {
            Some(b) => format!("base de la instancia en {b}"),
            None => format!(
                "no hay board.sqlite en {} (candidatas: {})",
                cfg.state_dir(),
                dirs.join(", ")
            ),
        },
    ));

    // --- 8b) EL STACK ESTÁ DESFASADO -------------------------------------
    let desfasados = sis.desfasados_stack();
    v.push(Diagnostico::nuevo(
        Sintoma::StackDesfasado,
        desfasados > 0,
        format!("stack: {desfasados} fichero(s) desfasado(s)"),
    ));

    // --- 9) FALTAN FICHEROS DEL STACK ------------------------------------
    let mut faltan: Vec<String> = Vec::new();
    for (nombre, ruta) in [
        ("99-megadaemon.sh", cfg.ruta_supervisor()),
        ("97-megadaemon-keeper.sh", cfg.ruta_keeper()),
        ("98-cloudflared.sh", cfg.ruta_cloudflared.clone()),
        ("91-adb-tcp.sh", cfg.ruta_adb_tcp.clone()),
        (NOMBRE_WATCHDOG, cfg.ruta_watchdog.clone()),
    ] {
        if !sis.existe(&ruta) {
            faltan.push(format!("{nombre} ({ruta})"));
        }
    }
    v.push(Diagnostico::nuevo(
        Sintoma::FicherosStackAusentes,
        !faltan.is_empty(),
        if faltan.is_empty() {
            "todos los ficheros del stack estan".to_string()
        } else {
            format!("faltan: {}", faltan.join(", "))
        },
    ));

    v
}

/// Lee la URL del túnel de donde esté (la primera que exista).
pub fn leer_url_tunel(sis: &dyn Sistema, cfg: &Config) -> String {
    for ruta in cfg.rutas_url_tunel() {
        if let Some(c) = sis.leer(&ruta) {
            let t = c.trim().to_string();
            if !t.is_empty() {
                return t;
            }
        }
    }
    String::new()
}

// ---------------------------------------------------------------------------
// LA RESTAURACIÓN
// ---------------------------------------------------------------------------

/// Cómo terminó una restauración.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resultado {
    /// Se arregló.
    Ok,
    /// Falló (⇒ backoff creciente).
    Fallo,
    /// No se tocó nada por el presupuesto / la marca de "ya restaurado".
    Presupuesto,
    /// No había ningún síntoma.
    SinSintomas,
}

impl Resultado {
    pub fn as_str(&self) -> &'static str {
        match self {
            Resultado::Ok => "ok",
            Resultado::Fallo => "fallo",
            Resultado::Presupuesto => "presupuesto",
            Resultado::SinSintomas => "sin_sintomas",
        }
    }
}

/// El informe de una restauración.
#[derive(Debug, Clone)]
pub struct Informe {
    /// Cómo terminó.
    pub resultado: Resultado,
    /// Qué síntomas la dispararon.
    pub sintomas: Vec<String>,
    /// Qué se hizo.
    pub acciones: Vec<String>,
    /// Qué quedó (la verificación).
    pub verificacion: Vec<String>,
    /// Por qué no se hizo (si aplica).
    pub motivo: Option<String>,
    /// Instante.
    pub ts: i64,
    /// Duración.
    pub duracion_s: i64,
}

impl Informe {
    /// JSON del informe.
    pub fn a_json(&self) -> Value {
        json!({
            "ok": self.resultado == Resultado::Ok,
            "resultado": self.resultado.as_str(),
            "sintomas": self.sintomas,
            "acciones": self.acciones,
            "verificacion": self.verificacion,
            "motivo": self.motivo,
            "ts": self.ts,
            "duracion_s": self.duracion_s,
        })
    }

    /// Las líneas del informe (para el log y el CLI).
    pub fn lineas(&self) -> Vec<String> {
        let mut v = vec![format!(
            "autorreparacion: {} (resultado: {})",
            if self.sintomas.is_empty() {
                "sin sintomas".to_string()
            } else {
                self.sintomas.join(", ")
            },
            self.resultado.as_str()
        )];
        for a in &self.acciones {
            v.push(format!("  - {a}"));
        }
        for a in &self.verificacion {
            v.push(format!("  = {a}"));
        }
        if let Some(m) = &self.motivo {
            v.push(format!("  ! {m}"));
        }
        v
    }
}

/// **Restaura** el entorno: desinfecta, limpia, reinstala, arranca y verifica.
///
/// Es **IDEMPOTENTE**: llamarla dos veces no rompe nada ni mata lo sano.
pub fn restaurar(sis: &dyn Sistema, cfg: &Config, sintomas: &[Sintoma], forzar: bool) -> Informe {
    let inicio = sis.ahora();
    let mut acciones: Vec<String> = Vec::new();
    let mut verificacion: Vec<String> = Vec::new();
    let nombres: Vec<String> = sintomas.iter().map(|s| s.nombre().to_string()).collect();

    // --- 0) el presupuesto (el bucle) -----------------------------------
    let ruta_estado = cfg.ruta_estado();
    let mut estado = cargar_estado(sis, &ruta_estado);
    if !forzar {
        let bloqueo = motivo_bloqueo(sis, cfg, &estado, sintomas, inicio);
        if !bloqueo.is_empty() {
            let informe = Informe {
                resultado: Resultado::Presupuesto,
                sintomas: nombres,
                acciones,
                verificacion,
                motivo: Some(bloqueo),
                ts: inicio,
                duracion_s: 0,
            };
            let _ = guardar_estado(sis, &ruta_estado, &estado);
            return informe;
        }
    }

    // --- 1) PARAR: el stack, en el ORDEN INVERSO al de arranque ---------
    // keeper → watchdog → supervisor → cloudflared → daemon (el de
    // tools/restaurar-movil.sh).  Se matan los duplicados y NUNCA el
    // canónico; el daemon nunca se suicida.
    if sintomas.contains(&Sintoma::MultiplicacionProcesos)
        || sintomas.contains(&Sintoma::TunelCaido)
    {
        acciones.extend(parar(sis, cfg, sintomas));
    }

    // --- 2) LIMPIAR: copias .bak* y URLs del túnel ----------------------
    // La URL se resetea si es invalida (la de la API) o si el túnel está
    // caído (la URL vieja apuntaría a un túnel muerto).
    if sintomas.contains(&Sintoma::UrlTunelInvalida) || sintomas.contains(&Sintoma::TunelCaido) {
        acciones.extend(limpiar_urls(sis, cfg));
    }
    if sintomas.contains(&Sintoma::FicherosStackAusentes)
        || sintomas.contains(&Sintoma::StackDesfasado)
    {
        acciones.extend(limpiar_copias_service_d(sis, cfg));
    }

    // --- 3) REINSTALAR: el motor base + el stack ------------------------
    if sintomas.contains(&Sintoma::FicherosStackAusentes)
        || sintomas.contains(&Sintoma::StackDesfasado)
    {
        for l in sis.reconciliar_stack() {
            acciones.push(format!("stack: {l}"));
        }
    }
    for l in sis.reparar_entorno_base() {
        acciones.push(format!("entorno: {l}"));
    }

    // --- 4) ARRANCAR: uno de cada ---------------------------------------
    if sintomas.contains(&Sintoma::TunelCaido)
        || sintomas.contains(&Sintoma::UrlTunelInvalida)
        || sintomas.contains(&Sintoma::MultiplicacionProcesos)
    {
        acciones.extend(arrancar_tunel(sis, cfg));
    }
    if sintomas.contains(&Sintoma::MultiplicacionProcesos) {
        acciones.extend(arrancar_supervisor_y_keeper(sis, cfg));
    }

    // --- 5) EL DAEMON: uno de cada, y no puede matarse a sí mismo ------
    if sintomas.contains(&Sintoma::DaemonSinHealth) || sintomas.contains(&Sintoma::BinarioDesfasado)
    {
        acciones.extend(reiniciar_daemon(sis, cfg));
    }

    // --- 6) EL 8765 LEGACY: sobra ---------------------------------------
    if sintomas.contains(&Sintoma::PuertoLegacyVivo) {
        acciones.extend(parar_legacy(sis, cfg));
    }

    // --- 7) TELEMETRÍA Y CANAL ------------------------------------------
    if sintomas.contains(&Sintoma::TelemetriaNoPublicada) {
        acciones.extend(arreglar_telemetria(sis, cfg));
    }
    if sintomas.contains(&Sintoma::CanalNoPublicado) {
        acciones.extend(arreglar_canal(sis, cfg));
    }

    // --- 8) LA BASE DE LA INSTANCIA -------------------------------------
    if sintomas.contains(&Sintoma::BaseInstanciaAusente) {
        acciones.extend(crear_base(sis, cfg));
    }

    // --- 9) VERIFICAR ----------------------------------------------------
    verificacion.extend(verificar(sis, cfg));

    // --- 10) el registro -------------------------------------------------
    let fin = sis.ahora();
    let duracion = fin - inicio;
    let hay_fallo = acciones
        .iter()
        .any(|a| a.starts_with("FALLO") || a.contains("NO se pudo"));
    let resultado = if hay_fallo {
        Resultado::Fallo
    } else {
        Resultado::Ok
    };

    let informe = Informe {
        resultado,
        sintomas: nombres.clone(),
        acciones,
        verificacion,
        motivo: None,
        ts: inicio,
        duracion_s: duracion,
    };

    // presupuesto + marca de "ya restaurado"
    estado.ultimo_intento = fin;
    if informe.resultado == Resultado::Ok {
        estado.fallos_consecutivos = 0;
        estado.ultimo_exito = fin;
        estado.proximo_permitido = fin + cfg.enfriamiento_s;
    } else {
        estado.fallos_consecutivos = estado.fallos_consecutivos.saturating_add(1);
        let idx = ((estado.fallos_consecutivos as usize).saturating_sub(1))
            .min(cfg.backoff_min.len().saturating_sub(1));
        let espera = cfg.backoff_min.get(idx).copied().unwrap_or(15) * 60;
        estado.proximo_permitido = fin + espera as i64;
    }
    estado.restauraciones.push(Restauracion {
        ts: inicio,
        sintomas: nombres,
        acciones: informe.acciones.clone(),
        resultado: informe.resultado.as_str().to_string(),
        duracion_s: duracion,
    });
    // el historial no crece sin límite
    if estado.restauraciones.len() > 50 {
        let exceso = estado.restauraciones.len() - 50;
        estado.restauraciones.drain(0..exceso);
    }
    let _ = guardar_estado(sis, &ruta_estado, &estado);

    // el log
    for l in informe.lineas() {
        let _ = sis.anadir(&cfg.ruta_log(), &format!("[{inicio}] {l}\n"));
    }

    informe
}

/// **PARA** el stack en el **orden inverso al de arranque**
/// (keeper → watchdog → supervisor → cloudflared → daemon).
///
/// MEDIDO (2026-11-28): ese orden es el de `tools/restaurar-movil.sh` y
/// no es casualidad: el keeper va **primero** porque es el que **relanza**
/// al supervisor (si se para el supervisor antes, el keeper lo revive y
/// volvemos a tener dos supervisores: el "pastel" de procesos).
///
/// Las reglas duras (y por qué):
///   * el supervisor **canónico** nunca se mata: es el que sabe relanzar
///     el daemon y el túnel;
///   * el daemon **nunca** se mata a sí mismo (es un suicidio): si el
///     duplicado es **nosotros**, se avisa y se sigue;
///   * de cada rol se deja **UNO**: el canónico, o el más antiguo.
///   * con el túnel **caído** el `cloudflared` se para **entero** (arranque
///     en frío): es el único caso en que se para el que se quedaba.
pub fn parar(sis: &dyn Sistema, cfg: &Config, sintomas: &[Sintoma]) -> Vec<String> {
    let mut acciones = Vec::new();
    let yo = std::process::id() as i32;
    let tunel_caido = sintomas.contains(&Sintoma::TunelCaido);

    for rol in Rol::orden_parada() {
        let vs = vivos_del_rol(sis, cfg, rol);
        if vs.is_empty() {
            continue;
        }
        // El que se queda: el canónico (si lo hay), si no el más antiguo
        // (menor PID) ⇒ el que lleva más tiempo con vida.
        let se_queda = vs
            .iter()
            .find(|p| rol.es_supervisor() && es_supervisor_canonico(p, &cfg.ruta_supervisor()))
            .cloned()
            .unwrap_or_else(|| vs.iter().min_by_key(|p| p.pid).cloned().unwrap());
        // ¿Toca parar el rol ENTERO (arranque en frío)?
        let frio = rol == Rol::Cloudflared && tunel_caido;

        for p in &vs {
            if p.pid == yo {
                acciones.push(format!(
                    "{}: no se mata al propio daemon (pid {yo})",
                    rol.nombre()
                ));
                continue;
            }
            if rol.es_supervisor() && es_supervisor_canonico(p, &cfg.ruta_supervisor()) {
                acciones.push(format!(
                    "{}: pid {} es el CANONICO: no se mata",
                    rol.nombre(),
                    p.pid
                ));
                continue;
            }
            if !frio && p.pid == se_queda.pid {
                continue; // se queda UNO de cada
            }
            let ok = sis.senal(p.pid, 9);
            acciones.push(format!(
                "{}: pid {} {} ({})",
                rol.nombre(),
                p.pid,
                if ok { "parado" } else { "NO se pudo parar" },
                if frio {
                    "arranque en frio: se relanza"
                } else {
                    "duplicado; se queda el canonico"
                }
            ));
        }
    }
    acciones
}

/// Resetea las URLs del túnel (las reescribe el supervisor al arrancar).
pub fn limpiar_urls(sis: &dyn Sistema, cfg: &Config) -> Vec<String> {
    let mut acciones = Vec::new();
    for ruta in cfg.rutas_url_tunel() {
        if sis.existe(&ruta) {
            match sis.remover(&ruta) {
                Ok(_) => acciones.push(format!("URL reseteada: {ruta}")),
                Err(e) => acciones.push(format!("FALLO al resetear {ruta}: {e}")),
            }
        }
    }
    acciones
}

/// Borra las **copias `.bak*`** de `service.d` (Magisk ejecuta TODO lo
/// ejecutable de `service.d`: una copia es un segundo servicio).
pub fn limpiar_copias_service_d(sis: &dyn Sistema, cfg: &Config) -> Vec<String> {
    let mut acciones = Vec::new();
    let entradas = sis.listar(&cfg.service_d);
    for nombre in entradas {
        // La copia de cualquier script de service.d (`.bak*`) es un SEGUNDO
        // servicio: Magisk ejecuta todo lo ejecutable de `service.d`.
        let es_copia = FICHEROS_SERVICE_D
            .iter()
            .any(|f| nombre.starts_with(f) && nombre.len() > f.len());
        if !es_copia {
            continue;
        }
        let ruta = format!("{}/{}", cfg.service_d, nombre);
        match sis.remover(&ruta) {
            Ok(_) => acciones.push(format!("copia borrada: {nombre}")),
            Err(e) => acciones.push(format!("FALLO al borrar {nombre}: {e}")),
        }
    }
    acciones
}

/// Arranca el túnel (y su watchdog) si no hay ninguno vivo.
pub fn arrancar_tunel(sis: &dyn Sistema, cfg: &Config) -> Vec<String> {
    let mut acciones = Vec::new();
    if !vivos_del_rol(sis, cfg, Rol::Cloudflared).is_empty() {
        return acciones;
    }
    if !sis.existe(&cfg.ruta_cloudflared) {
        acciones.push(format!(
            "FALLO: no existe {}: no se puede arrancar el tunel",
            cfg.ruta_cloudflared
        ));
        return acciones;
    }
    match sis.lanzar(&["sh", &cfg.ruta_cloudflared]) {
        Ok(_) => acciones.push("tunel arrancado (98-cloudflared.sh)".to_string()),
        Err(e) => acciones.push(format!("FALLO al arrancar el tunel: {e}")),
    }
    acciones
}

/// Arranca el supervisor canónico y el keeper si no hay ninguno vivo.
pub fn arrancar_supervisor_y_keeper(sis: &dyn Sistema, cfg: &Config) -> Vec<String> {
    let mut acciones = Vec::new();
    // El watchdog del túnel: si no hay ninguno, `98-cloudflared.sh` no lo
    // lanzó (o murió): se levanta solo.
    if vivos_del_rol(sis, cfg, Rol::Watchdog).is_empty() && sis.existe(&cfg.ruta_watchdog) {
        match sis.lanzar(&["sh", &cfg.ruta_watchdog]) {
            Ok(_) => acciones.push("watchdog del tunel arrancado".to_string()),
            Err(e) => acciones.push(format!("FALLO al arrancar el watchdog del tunel: {e}")),
        }
    }
    if vivos_del_rol(sis, cfg, Rol::Supervisor).is_empty() {
        if sis.existe(&cfg.ruta_supervisor()) {
            match sis.lanzar(&["sh", &cfg.ruta_supervisor()]) {
                Ok(_) => acciones.push("supervisor arrancado".to_string()),
                Err(e) => acciones.push(format!("FALLO al arrancar el supervisor: {e}")),
            }
        } else {
            acciones.push(format!(
                "FALLO: no existe {}: no se puede arrancar el supervisor",
                cfg.ruta_supervisor()
            ));
        }
    }
    if vivos_del_rol(sis, cfg, Rol::Keeper).is_empty() {
        if sis.existe(&cfg.ruta_keeper()) {
            match sis.lanzar(&["sh", &cfg.ruta_keeper()]) {
                Ok(_) => acciones.push("keeper arrancado".to_string()),
                Err(e) => acciones.push(format!("FALLO al arrancar el keeper: {e}")),
            }
        } else {
            acciones.push(format!(
                "FALLO: no existe {}: no se puede arrancar el keeper",
                cfg.ruta_keeper()
            ));
        }
    }
    acciones
}

/// Reinicia el daemon (para que cargue el binario de disco / desatraseque).
///
/// El daemon **no se mata a sí mismo**: se lanza un ayudante desligado que
/// lo mata un instante después, y lo revive el supervisor.
pub fn reiniciar_daemon(sis: &dyn Sistema, cfg: &Config) -> Vec<String> {
    let mut acciones = Vec::new();
    let yo = std::process::id() as i32;
    let vs = vivos_del_rol(sis, cfg, Rol::Daemon);
    if vs.is_empty() {
        acciones.push("no hay daemon vivo: lo arranca el supervisor (no se toca nada)".to_string());
        return acciones;
    }
    // Si no hay supervisor vivo, **no** se toca el daemon: sin supervisor
    // nadie lo relanzaría y el móvil se quedaría sin daemon para siempre.
    if vivos_del_rol(sis, cfg, Rol::Supervisor).is_empty() {
        acciones.push(
            "no hay supervisor vivo: NO se reinicia el daemon (quedaria sin quien lo relance)"
                .to_string(),
        );
        return acciones;
    }
    for p in &vs {
        if p.pid == yo {
            let cmd = format!("sleep 3; kill -9 {yo}");
            match sis.lanzar(&["sh", "-c", &cmd]) {
                Ok(_) => acciones.push(format!(
                    "daemon (pid {yo}): reinicio pedido (lo revive el supervisor)"
                )),
                Err(e) => acciones.push(format!("FALLO al pedir el reinicio del daemon: {e}")),
            }
            continue;
        }
        let ok = sis.senal(p.pid, 9);
        acciones.push(format!(
            "daemon: pid {} {} (lo revive el supervisor)",
            p.pid,
            if ok { "parado" } else { "NO se pudo parar" }
        ));
    }
    acciones
}

/// Para el listener del 8765 (el MCP legacy sobra).
pub fn parar_legacy(sis: &dyn Sistema, cfg: &Config) -> Vec<String> {
    let mut acciones = Vec::new();
    if !sis.puertos_listen().contains(&PUERTO_LEGACY) {
        return acciones;
    }
    // Se para el proceso del MCP legacy (el que tiene el 8765).
    let yo = std::process::id() as i32;
    for p in sis.procesos() {
        if !p.termina_en("android-mcp") && !p.termina_en("mcp-legacy") {
            continue;
        }
        if p.pid == yo {
            continue;
        }
        let ok = sis.senal(p.pid, 9);
        acciones.push(format!(
            "8765: pid {} {} (listener legacy)",
            p.pid,
            if ok { "parado" } else { "NO se pudo parar" }
        ));
    }
    if acciones.is_empty() {
        acciones.push(
            "8765: hay listener pero no se ha encontrado el proceso (lo para el supervisor)"
                .to_string(),
        );
    }
    let _ = cfg;
    acciones
}

/// Arregla el modo de las credenciales y republica la telemetría.
pub fn arreglar_telemetria(sis: &dyn Sistema, cfg: &Config) -> Vec<String> {
    let mut acciones = Vec::new();
    let creds = format!("{}/gdrive_credentials.json", cfg.state_dir());
    if let Some(m) = sis.modo(&creds) {
        if m != 0o600 {
            match sis.escribir(&creds, &sis.leer(&creds).unwrap_or_default(), 0o600) {
                Ok(_) => acciones.push(format!("credenciales de Drive: chmod 0600 (eran {:o})", m)),
                Err(e) => acciones.push(format!("FALLO con las credenciales de Drive: {e}")),
            }
        }
    }
    match sis.publicar_telemetria() {
        Ok(_) => acciones.push("telemetria republicada".to_string()),
        Err(e) => acciones.push(format!("FALLO al publicar la telemetria: {e}")),
    }
    acciones
}

/// Republica el canal.
pub fn arreglar_canal(sis: &dyn Sistema, cfg: &Config) -> Vec<String> {
    let mut acciones = Vec::new();
    match sis.publicar_canal() {
        Ok(_) => acciones.push("canal republicado".to_string()),
        Err(e) => acciones.push(format!("FALLO al publicar el canal: {e}")),
    }
    let _ = cfg;
    acciones
}

/// Crea (abre) el tablero de la instancia.
pub fn crear_base(sis: &dyn Sistema, cfg: &Config) -> Vec<String> {
    let mut acciones = Vec::new();
    let dirs = sis.dirs_tablero();
    let destino = dirs.first().cloned().unwrap_or_else(|| cfg.state_dir());
    let ruta = format!("{destino}/board.sqlite");
    match sis.abrir_tablero(&ruta) {
        Ok(_) => acciones.push(format!("base de la instancia creada en {ruta}")),
        Err(e) => acciones.push(format!("FALLO al crear la base de la instancia: {e}")),
    }
    acciones
}

/// Verifica el estado final (lo que queda).
pub fn verificar(sis: &dyn Sistema, cfg: &Config) -> Vec<String> {
    let mut v = Vec::new();
    for rol in [
        Rol::Supervisor,
        Rol::Keeper,
        Rol::Watchdog,
        Rol::Cloudflared,
        Rol::Daemon,
    ] {
        let vs = vivos_del_rol(sis, cfg, rol);
        let pids: Vec<String> = vs.iter().map(|p| p.pid.to_string()).collect();
        v.push(format!(
            "{}: {} vivo(s) [{}]",
            rol.nombre(),
            vs.len(),
            pids.join(", ")
        ));
    }
    let url = leer_url_tunel(sis, cfg);
    let motivo = motivo_url_tunel(&url);
    v.push(if motivo.is_empty() {
        format!("URL del tunel: {url} (valida)")
    } else {
        format!("URL del tunel: {} (INVALIDA: {motivo})", url.trim())
    });
    v
}

// ---------------------------------------------------------------------------
// EL CICLO AUTOMÁTICO
// ---------------------------------------------------------------------------

/// Un ciclo: diagnostica y, si hay síntomas, restaura.
///
/// Devuelve el informe (o `None` si no había nada que hacer).
pub fn ciclo(sis: &dyn Sistema, cfg: &Config) -> Option<Informe> {
    let (presentes, _diagnostico) = sintomas_presentes(sis, cfg);
    if presentes.is_empty() {
        return None;
    }
    Some(restaurar(sis, cfg, &presentes, false))
}

/// El ciclo automático en segundo plano (el hilo del daemon).
pub fn bucle(minutos: u64) -> Result<std::thread::JoinHandle<()>, String> {
    std::thread::Builder::new()
        .name("autorreparacion".to_string())
        .spawn(move || {
            let cfg = Config::del_entorno();
            let sis = SistemaReal;
            loop {
                // MEDIDO (2026-11-28): con la bateria en "seal" el trabajo
                // discrecional se espacia (mismo criterio que el resto).
                let espera = if crate::battery_advanced::modo_ahorro_activo() {
                    minutos * 4
                } else {
                    minutos
                };
                std::thread::sleep(std::time::Duration::from_secs(espera.max(1) * 60));
                if let Some(informe) = ciclo(&sis, &cfg) {
                    for l in informe.lineas() {
                        eprintln!("{l}");
                    }
                }
            }
        })
        .map_err(|e| format!("no se pudo arrancar el hilo de autorreparacion: {e}"))
}

// ---------------------------------------------------------------------------
// LA IMPLEMENTACIÓN REAL
// ---------------------------------------------------------------------------

/// El sistema real (disco + `/proc` + red).
pub struct SistemaReal;

impl Sistema for SistemaReal {
    fn ahora(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    fn procesos(&self) -> Vec<Proceso> {
        let mut v = Vec::new();
        let Ok(entradas) = std::fs::read_dir("/proc") else {
            return v;
        };
        for e in entradas.flatten() {
            let nombre = e.file_name().to_string_lossy().to_string();
            if !nombre.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let Ok(pid) = nombre.parse::<i32>() else {
                continue;
            };
            match std::fs::read(e.path().join("cmdline")) {
                Ok(b) => {
                    let argv: Vec<String> = b
                        .split(|c| *c == 0)
                        .filter(|s| !s.is_empty())
                        .map(|s| String::from_utf8_lossy(s).to_string())
                        .collect();
                    v.push(Proceso {
                        pid,
                        argv,
                        legible: true,
                    });
                }
                // MEDIDO (2026-11-28): SELinux puede impedir leer el cmdline.
                // Eso **NO** significa que el proceso esté muerto: se marca
                // como ilegible y la vida se comprueba por PID + señal 0.
                Err(_) => v.push(Proceso::ilegible(pid)),
            }
        }
        v
    }

    fn proc_existe(&self, pid: i32) -> bool {
        Path::new(&format!("/proc/{pid}")).exists()
    }

    fn senal(&self, pid: i32, sig: i32) -> bool {
        if pid <= 0 {
            return false;
        }
        // SAFETY: `kill` con pid > 0 sólo manda la señal.
        unsafe { libc::kill(pid, sig) == 0 }
    }

    fn leer(&self, ruta: &str) -> Option<String> {
        std::fs::read_to_string(ruta).ok()
    }

    fn escribir(&self, ruta: &str, contenido: &str, modo: u32) -> Result<(), String> {
        crate::util_fs::write_atomic(Path::new(ruta), contenido.as_bytes(), modo)
    }

    fn anadir(&self, ruta: &str, linea: &str) -> Result<(), String> {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(ruta)
            .map_err(|e| format!("abrir {ruta}: {e}"))?;
        f.write_all(linea.as_bytes())
            .map_err(|e| format!("escribir {ruta}: {e}"))
    }

    fn existe(&self, ruta: &str) -> bool {
        Path::new(ruta).exists()
    }

    fn es_dir(&self, ruta: &str) -> bool {
        Path::new(ruta).is_dir()
    }

    fn remover(&self, ruta: &str) -> Result<(), String> {
        std::fs::remove_file(ruta).map_err(|e| format!("borrar {ruta}: {e}"))
    }

    fn listar(&self, dir: &str) -> Vec<String> {
        let mut v = Vec::new();
        if let Ok(entradas) = std::fs::read_dir(dir) {
            for e in entradas.flatten() {
                v.push(e.file_name().to_string_lossy().to_string());
            }
        }
        v.sort();
        v
    }

    fn modo(&self, ruta: &str) -> Option<u32> {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(ruta)
            .ok()
            .map(|m| m.permissions().mode() & 0o7777)
    }

    fn puertos_listen(&self) -> Vec<u16> {
        let mut v = Vec::new();
        for f in ["/proc/net/tcp", "/proc/net/tcp6"] {
            let Ok(cuerpo) = std::fs::read_to_string(f) else {
                continue;
            };
            for linea in cuerpo.lines().skip(1) {
                let Some(local) = linea.split_whitespace().nth(1) else {
                    continue;
                };
                let Some(hex) = local.rsplit(':').next() else {
                    continue;
                };
                let Ok(p) = u16::from_str_radix(hex, 16) else {
                    continue;
                };
                // El estado 0A es LISTEN.
                let estado = linea.split_whitespace().nth(3).unwrap_or("");
                if estado == "0A" && !v.contains(&p) {
                    v.push(p);
                }
            }
        }
        v.sort_unstable();
        v
    }

    fn sondear(&self, url: &str) -> Result<u16, String> {
        Ok(crate::channel::probe_url(url).0)
    }

    fn lanzar(&self, argv: &[&str]) -> Result<(), String> {
        if argv.is_empty() {
            return Err("argv vacio".to_string());
        }
        // `setsid nohup` ⇒ el proceso NO es hijo del daemon (no muere con
        // él).  Es lo que usan los scripts de `deploy/`.
        let mut c = std::process::Command::new("setsid");
        c.arg("nohup");
        for a in argv {
            c.arg(a);
        }
        c.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .stdin(std::process::Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("lanzar {argv:?}: {e}"))
    }

    fn sha256(&self, ruta: &str) -> Option<String> {
        crate::self_update::sha256_de_fichero(Path::new(ruta))
    }

    fn reconciliar_stack(&self) -> Vec<String> {
        let cfg = crate::stack::Config::del_entorno();
        let inf = crate::stack::reconciliar(&cfg);
        let mut v: Vec<String> = inf.escritos.clone();
        for (r, m) in &inf.fallidos {
            v.push(format!("FALLO {r}: {m}"));
        }
        v
    }

    fn reparar_entorno_base(&self) -> Vec<String> {
        let cfg = crate::reparar_entorno::Config::del_entorno();
        let v = crate::reparar_entorno::reparar(&crate::reparar_entorno::FsEntorno, &cfg);
        v.eslabones
            .iter()
            .map(|e| format!("{}: {} ({})", e.nombre, e.estado.as_str(), e.toca))
            .collect()
    }

    fn publicar_telemetria(&self) -> Result<(), String> {
        crate::telemetria::publicar(None).map(|_| ())
    }

    fn publicar_canal(&self) -> Result<(), String> {
        let v = crate::channel::publish(None, None, None, None, None);
        if v.get("published")
            .and_then(|x| x.as_bool())
            .unwrap_or(false)
        {
            Ok(())
        } else {
            Err(v
                .get("detail")
                .and_then(|x| x.as_str())
                .unwrap_or("el canal no se publico")
                .to_string())
        }
    }

    fn abrir_tablero(&self, ruta: &str) -> Result<(), String> {
        crate::coordination::TaskBoard::open(ruta).map(|_| ())
    }

    fn sha_de_arranque(&self) -> String {
        crate::self_update::sha_de_arranque().to_string()
    }

    fn dirs_tablero(&self) -> Vec<String> {
        crate::webui::coord_board_dirs()
    }

    fn eslabones_entorno(&self) -> Vec<Value> {
        let cfg = crate::reparar_entorno::Config::del_entorno();
        crate::reparar_entorno::verificar(&crate::reparar_entorno::FsEntorno, &cfg)
            .iter()
            .map(|e| {
                json!({
                    "nombre": e.nombre,
                    "estado": e.estado.as_str(),
                    "que": e.que,
                    "toca": e.toca,
                    "bloquea": e.bloquea,
                })
            })
            .collect()
    }

    fn informe_entorno(&self) -> Vec<String> {
        let cfg = crate::reparar_entorno::Config::del_entorno();
        let eslabones = crate::reparar_entorno::verificar(&crate::reparar_entorno::FsEntorno, &cfg);
        crate::reparar_entorno::Veredicto {
            eslabones,
            reparado: false,
        }
        .informe()
    }

    fn desfasados_stack(&self) -> u32 {
        let cfg = crate::stack::Config::del_entorno();
        crate::stack::estado_json(&cfg)
            .get("desfasados")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32
    }
}

// ---------------------------------------------------------------------------
// LAS TRES ENTRADAS: MCP, HTTP y CLI
// ---------------------------------------------------------------------------

/// El estado completo (diagnóstico + presupuesto + registro) en JSON.
pub fn estado_json(sis: &dyn Sistema, cfg: &Config) -> Value {
    let diagnostico = diagnosticar(sis, cfg);
    let estado = cargar_estado(sis, &cfg.ruta_estado());
    let ahora = sis.ahora();
    let hora = ahora - 3600;
    let en_la_hora = estado
        .restauraciones
        .iter()
        .filter(|r| r.ts >= hora)
        .count();
    let presentes: Vec<&str> = diagnostico
        .iter()
        .filter(|d| d.presente)
        .map(|d| d.sintoma.nombre())
        .collect();
    let accionables: Vec<&str> = diagnostico
        .iter()
        .filter(|d| d.presente && d.accionable)
        .map(|d| d.sintoma.nombre())
        .collect();
    json!({
        "version": VERSION,
        "ok": presentes.is_empty(),
        // Las MISMAS claves que daba el motor base (ok / reparado /
        // eslabones / informe) MAS las del motor de autorreparacion: los
        // eslabones del motor base (que/toque/bloquea) y los síntomas (la
        // acción de cada uno) van en el MISMO informe.
        "reparado": false,
        "eslabones": sis.eslabones_entorno(),
        "informe": sis.informe_entorno(),
        "sintomas_presentes": presentes,
        "sintomas_accionables": accionables,
        "sintomas": diagnostico.iter().map(|d| d.a_json()).collect::<Vec<_>>(),
        "presupuesto": {
            "max_por_hora": cfg.max_por_hora,
            "restauraciones_ultima_hora": en_la_hora,
            "fallos_consecutivos": estado.fallos_consecutivos,
            "ultimo_intento": estado.ultimo_intento,
            "ultimo_exito": estado.ultimo_exito,
            "proximo_permitido": estado.proximo_permitido,
            "faltan_s": (estado.proximo_permitido - ahora).max(0),
            "enfriamiento_s": cfg.enfriamiento_s,
            "backoff_min": cfg.backoff_min,
        },
        "registro": estado.restauraciones.iter().map(|r| r.a_json()).collect::<Vec<_>>(),
    })
}

/// Los síntomas presentes que **exigen** una restauración.
pub fn sintomas_presentes(sis: &dyn Sistema, cfg: &Config) -> (Vec<Sintoma>, Vec<Diagnostico>) {
    let diagnostico = diagnosticar(sis, cfg);
    // Sólo los ACCIONABLES: un síntoma que el daemon no puede arreglar solo
    // (p.ej. credenciales de Drive ausentes) se informa pero no consume
    // presupuesto ni dispara una restauración.
    let presentes: Vec<Sintoma> = diagnostico
        .iter()
        .filter(|d| d.presente && d.accionable)
        .map(|d| d.sintoma)
        .collect();
    (presentes, diagnostico)
}

/// Diagnostica **y** repara (el motor base + los síntomas).
pub fn reparar_ahora() -> Value {
    let cfg = Config::del_entorno();
    let sis = SistemaReal;
    let (presentes, diagnostico) = sintomas_presentes(&sis, &cfg);
    if presentes.is_empty() {
        return json!({
            "ok": true,
            "resultado": "sin_sintomas",
            "sintomas": diagnostico.iter().map(|d| d.a_json()).collect::<Vec<_>>(),
        });
    }
    let informe = restaurar(&sis, &cfg, &presentes, false);
    let mut v = informe.a_json();
    if let Some(o) = v.as_object_mut() {
        o.insert(
            "sintomas_detalle".to_string(),
            json!(diagnostico.iter().map(|d| d.a_json()).collect::<Vec<_>>()),
        );
    }
    v
}

/// La tool MCP `autorreparar` (`accion`: diagnostico | restaurar | estado).
pub fn tool_dispatch(args: &Value) -> Value {
    let accion = args
        .get("accion")
        .and_then(|v| v.as_str())
        .unwrap_or("diagnostico")
        .to_string();
    let forzar = args
        .get("fuerza")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let cfg = Config::del_entorno();
    let sis = SistemaReal;
    match accion.as_str() {
        "restaurar" | "restore" | "reparar" => {
            let (presentes, diagnostico) = sintomas_presentes(&sis, &cfg);
            if presentes.is_empty() {
                return json!({"ok": true, "resultado": "sin_sintomas"});
            }
            let informe = restaurar(&sis, &cfg, &presentes, forzar);
            let mut v = informe.a_json();
            if let Some(o) = v.as_object_mut() {
                o.insert(
                    "sintomas_detalle".to_string(),
                    json!(diagnostico.iter().map(|d| d.a_json()).collect::<Vec<_>>()),
                );
            }
            v
        }
        _ => estado_json(&sis, &cfg),
    }
}

/// CLI: `megadaemon-core autorreparar [--accion=X] [--json] [--fuerza]`.
pub fn run_cli(args: &[String]) -> i32 {
    let accion = args
        .iter()
        .find_map(|a| a.strip_prefix("--accion=").map(|s| s.to_string()))
        .unwrap_or_else(|| {
            if args.iter().any(|a| a == "--restaurar") {
                "restaurar".to_string()
            } else {
                "diagnostico".to_string()
            }
        });
    let forzar = args.iter().any(|a| a == "--fuerza");
    let json = args.iter().any(|a| a == "--json");
    let cfg = Config::del_entorno();
    let sis = SistemaReal;
    let v = match accion.as_str() {
        "restaurar" | "restore" | "reparar" => {
            let (presentes, _diagnostico) = sintomas_presentes(&sis, &cfg);
            if presentes.is_empty() {
                json!({"ok": true, "resultado": "sin_sintomas"})
            } else {
                restaurar(&sis, &cfg, &presentes, forzar).a_json()
            }
        }
        "ciclar" | "cycle" => match ciclo(&sis, &cfg) {
            Some(i) => i.a_json(),
            None => json!({"ok": true, "resultado": "sin_sintomas"}),
        },
        _ => estado_json(&sis, &cfg),
    };
    if json {
        println!("{}", v);
    } else {
        if let Some(arr) = v.get("sintomas").and_then(|x| x.as_array()) {
            for d in arr {
                // En el informe de una restauracion los sintomas son
                // NOMBRES (strings); en el diagnostico son objetos.
                if let Some(s) = d.as_str() {
                    println!("  - {s}");
                    continue;
                }
                let s = d.get("sintoma").and_then(|x| x.as_str()).unwrap_or("?");
                let p = d.get("presente").and_then(|x| x.as_bool()).unwrap_or(false);
                let q = d.get("que").and_then(|x| x.as_str()).unwrap_or("");
                let det = d.get("detalle").and_then(|x| x.as_str()).unwrap_or("");
                println!(
                    "[{}] {} :: {} :: {}",
                    if p { "ROTO" } else { "ok " },
                    s,
                    q,
                    det
                );
            }
        }
        println!("---");
        if let Some(m) = v.get("motivo").and_then(|x| x.as_str()) {
            println!("motivo: {m}");
        }
        if let Some(r) = v.get("resultado").and_then(|x| x.as_str()) {
            println!("resultado: {r}");
        }
        if let Some(a) = v.get("acciones").and_then(|x| x.as_array()) {
            for x in a {
                println!("  - {}", x.as_str().unwrap_or(""));
            }
        }
    }
    let ok = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(true);
    if ok {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Sistema de mentira: sin disco, sin `/proc`, sin red.
    #[derive(Default)]
    struct Fake {
        procs: RefCell<Vec<Proceso>>,
        vivos: RefCell<std::collections::BTreeSet<i32>>,
        ficheros: RefCell<std::collections::BTreeMap<String, String>>,
        modos: RefCell<std::collections::BTreeMap<String, u32>>,
        dirs: RefCell<std::collections::BTreeSet<String>>,
        puertos: RefCell<Vec<u16>>,
        sondas: RefCell<std::collections::BTreeMap<String, u16>>,
        lanzados: RefCell<Vec<Vec<String>>>,
        ahora: RefCell<i64>,
        sha_disco: RefCell<String>,
        stack_desfasados: RefCell<u32>,
        telemetria_ok: RefCell<bool>,
        canal_ok: RefCell<bool>,
    }

    impl Fake {
        fn nuevo() -> Fake {
            Fake {
                ahora: RefCell::new(1_700_000_000),
                // Por defecto el binario de disco es el MISMO que el en
                // servicio (entorno sano); un test lo aparta para el
                // build desfasado.
                sha_disco: RefCell::new("sha-en-servicio".to_string()),
                ..Default::default()
            }
        }
        fn con_proceso(&mut self, p: Proceso) -> &mut Fake {
            self.vivos.borrow_mut().insert(p.pid);
            self.procs.borrow_mut().push(p);
            self
        }
        fn con_fichero(&mut self, r: &str, c: &str) -> &mut Fake {
            self.ficheros
                .borrow_mut()
                .insert(r.to_string(), c.to_string());
            self
        }
        fn con_modo(&mut self, r: &str, m: u32) -> &mut Fake {
            self.modos.borrow_mut().insert(r.to_string(), m);
            self
        }
        fn con_directorio(&mut self, d: &str) -> &mut Fake {
            self.dirs.borrow_mut().insert(d.to_string());
            self
        }
        fn con_sonda(&mut self, url: &str, code: u16) -> &mut Fake {
            self.sondas.borrow_mut().insert(url.to_string(), code);
            self
        }
    }

    impl Sistema for Fake {
        fn ahora(&self) -> i64 {
            *self.ahora.borrow()
        }
        fn procesos(&self) -> Vec<Proceso> {
            self.procs.borrow().clone()
        }
        fn proc_existe(&self, pid: i32) -> bool {
            self.vivos.borrow().contains(&pid)
        }
        fn senal(&self, pid: i32, _sig: i32) -> bool {
            self.vivos.borrow().contains(&pid)
        }
        fn leer(&self, ruta: &str) -> Option<String> {
            self.ficheros.borrow().get(ruta).cloned()
        }
        fn escribir(&self, ruta: &str, contenido: &str, modo: u32) -> Result<(), String> {
            self.ficheros
                .borrow_mut()
                .insert(ruta.to_string(), contenido.to_string());
            self.modos.borrow_mut().insert(ruta.to_string(), modo);
            Ok(())
        }
        fn anadir(&self, ruta: &str, linea: &str) -> Result<(), String> {
            self.ficheros
                .borrow_mut()
                .entry(ruta.to_string())
                .or_default()
                .push_str(linea);
            Ok(())
        }
        fn existe(&self, ruta: &str) -> bool {
            self.ficheros.borrow().contains_key(ruta) || self.dirs.borrow().contains(ruta)
        }
        fn es_dir(&self, ruta: &str) -> bool {
            self.dirs.borrow().contains(ruta)
        }
        fn remover(&self, ruta: &str) -> Result<(), String> {
            self.ficheros.borrow_mut().remove(ruta);
            Ok(())
        }
        fn listar(&self, _dir: &str) -> Vec<String> {
            self.ficheros
                .borrow()
                .keys()
                .filter_map(|k| k.rsplit('/').next().map(|s| s.to_string()))
                .collect()
        }
        fn modo(&self, ruta: &str) -> Option<u32> {
            if let Some(m) = self.modos.borrow().get(ruta) {
                return Some(*m);
            }
            if self.ficheros.borrow().contains_key(ruta) {
                return Some(0o600);
            }
            None
        }
        fn puertos_listen(&self) -> Vec<u16> {
            self.puertos.borrow().clone()
        }
        fn sondear(&self, url: &str) -> Result<u16, String> {
            match self.sondas.borrow().get(url) {
                Some(c) => Ok(*c),
                None => Err(format!("sin sonda para {url}")),
            }
        }
        fn lanzar(&self, argv: &[&str]) -> Result<(), String> {
            self.lanzados
                .borrow_mut()
                .push(argv.iter().map(|s| s.to_string()).collect());
            Ok(())
        }
        fn sha256(&self, _ruta: &str) -> Option<String> {
            Some(self.sha_disco.borrow().clone())
        }
        fn reconciliar_stack(&self) -> Vec<String> {
            vec!["99-megadaemon.sh reescrito".to_string()]
        }
        fn reparar_entorno_base(&self) -> Vec<String> {
            vec!["binario ok".to_string()]
        }
        fn publicar_telemetria(&self) -> Result<(), String> {
            if *self.telemetria_ok.borrow() {
                Ok(())
            } else {
                Err("no se pudo publicar".to_string())
            }
        }
        fn publicar_canal(&self) -> Result<(), String> {
            if *self.canal_ok.borrow() {
                Ok(())
            } else {
                Err("no se pudo publicar".to_string())
            }
        }
        fn abrir_tablero(&self, ruta: &str) -> Result<(), String> {
            self.ficheros
                .borrow_mut()
                .insert(ruta.to_string(), "sqlite".to_string());
            Ok(())
        }
        fn sha_de_arranque(&self) -> String {
            "sha-en-servicio".to_string()
        }
        fn dirs_tablero(&self) -> Vec<String> {
            vec!["/data/adb/megadaemon/state".to_string()]
        }
        fn eslabones_entorno(&self) -> Vec<Value> {
            vec![json!({
                "nombre": "binario",
                "estado": "ok",
                "que": "ELF",
                "toca": "nada",
                "bloquea": false,
            })]
        }
        fn informe_entorno(&self) -> Vec<String> {
            vec!["que: 1 eslabon, 0 roto(s)".to_string()]
        }
        fn desfasados_stack(&self) -> u32 {
            *self.stack_desfasados.borrow()
        }
    }

    fn cfg_de_prueba() -> Config {
        Config {
            entorno: crate::reparar_entorno::Config {
                prefijo: "/data/adb/megadaemon".to_string(),
                prefijo_legacy: "/data/local/tmp/megadaemon".to_string(),
                state_dir: "/data/adb/megadaemon/state".to_string(),
                ruta_supervisor: "/data/adb/service.d/99-megadaemon.sh".to_string(),
                ruta_keeper: "/data/adb/service.d/97-megadaemon-keeper.sh".to_string(),
                service_d: "/data/adb/service.d".to_string(),
                ruta_cloudflared: "/data/adb/service.d/98-cloudflared.sh".to_string(),
                ruta_watchdog: "/data/adb/megadaemon/tunnel-watchdog.sh".to_string(),
                ruta_adb_tcp: "/data/adb/service.d/91-adb-tcp.sh".to_string(),
                puerto: 9090,
                android: true,
            },
            service_d: "/data/adb/service.d".to_string(),
            ruta_cloudflared: "/data/adb/service.d/98-cloudflared.sh".to_string(),
            ruta_watchdog: "/data/adb/megadaemon/tunnel-watchdog.sh".to_string(),
            ruta_adb_tcp: "/data/adb/service.d/91-adb-tcp.sh".to_string(),
            puerto: 9090,
            max_por_hora: 3,
            enfriamiento_s: 1800,
            backoff_min: vec![5, 15, 45, 120],
            intervalo_min: 10,
        }
    }

    fn entorno_sano() -> Fake {
        let mut f = Fake::nuevo();
        f.con_directorio("/data/adb/megadaemon")
            .con_directorio("/data/adb/megadaemon/state")
            .con_fichero("/data/adb/megadaemon/megadaemon-core", "ELF")
            .con_fichero("/data/adb/service.d/99-megadaemon.sh", "sh")
            .con_fichero("/data/adb/service.d/97-megadaemon-keeper.sh", "sh")
            .con_fichero("/data/adb/service.d/98-cloudflared.sh", "sh")
            .con_fichero("/data/adb/service.d/91-adb-tcp.sh", "sh")
            .con_fichero("/data/adb/megadaemon/tunnel-watchdog.sh", "sh")
            .con_fichero("/data/adb/megadaemon/state/gdrive_credentials.json", "{}")
            .con_fichero("/data/adb/megadaemon/state/board.sqlite", "sqlite")
            // El indice de telemetria: la ultima publicacion (el sintoma
            // "la telemetria no se publica" se mira desde aqui, SIN
            // publicar nada).
            .con_fichero(
                "/data/adb/megadaemon/state/telemetria/indice.json",
                r#"{"ts":1700000000}"#,
            )
            .con_fichero(
                "/data/adb/megadaemon/state/channel.json",
                r#"{"quick":"https://a.trycloudflare.com","paste":"https://paste.rs/x"}"#,
            )
            .con_fichero(
                "/data/adb/megadaemon/tunnel.url",
                "https://abc-def.trycloudflare.com",
            )
            .con_sonda("https://abc-def.trycloudflare.com", 200)
            .con_sonda("http://127.0.0.1:9090/health", 200);
        f.telemetria_ok.replace(true);
        f.canal_ok.replace(true);
        f
    }

    fn presente(d: &[Diagnostico], s: Sintoma) -> bool {
        d.iter().any(|x| x.sintoma == s && x.presente)
    }

    // --- la URL del túnel ------------------------------------------------

    #[test]
    fn la_url_de_la_api_no_es_un_tunel() {
        // MEDIDO (2026-11-23): el grep cogia https://api.trycloudflare.com
        // (la URL de la API, NO un tunel).
        assert!(!url_tunel_valida("https://api.trycloudflare.com"));
        assert!(!url_tunel_valida("https://developers.trycloudflare.com"));
        assert!(!url_tunel_valida(""));
        assert!(!url_tunel_valida("   "));
        assert!(!url_tunel_valida("https://otro.example.com"));
        assert!(url_tunel_valida("https://abc-def.trycloudflare.com"));
        assert!(url_tunel_valida("https://x-1.trycloudflare.com"));
    }

    #[test]
    fn el_motivo_de_una_url_invalida_lo_dice() {
        assert!(motivo_url_tunel("https://api.trycloudflare.com").contains("API"));
        assert!(motivo_url_tunel("").contains("vacia"));
        assert!(motivo_url_tunel("https://abc.trycloudflare.com").is_empty());
    }

    // --- la multiplicación de procesos -----------------------------------

    #[test]
    fn la_multiplicacion_de_procesos_se_detecta() {
        let mut f = entorno_sano();
        f.con_proceso(Proceso::nuevo(
            10,
            &["sh", "/data/adb/service.d/99-megadaemon.sh"],
        ))
        .con_proceso(Proceso::nuevo(
            11,
            &["sh", "/data/adb/service.d/99-megadaemon.sh"],
        ))
        .con_proceso(Proceso::nuevo(
            12,
            &["sh", "/data/adb/service.d/97-megadaemon-keeper.sh"],
        ))
        .con_proceso(Proceso::nuevo(
            13,
            &["sh", "/data/adb/service.d/97-megadaemon-keeper.sh"],
        ));
        let cfg = cfg_de_prueba();
        let d = diagnosticar(&f, &cfg);
        assert!(presente(&d, Sintoma::MultiplicacionProcesos));
    }

    #[test]
    fn la_deduplicacion_mata_solo_a_los_no_canonicos() {
        let mut f = entorno_sano();
        f.con_proceso(Proceso::nuevo(
            10,
            &["sh", "/data/adb/service.d/99-megadaemon.sh"],
        ))
        .con_proceso(Proceso::nuevo(
            11,
            &["sh", "/data/adb/service.d/99-megadaemon.sh.bak-50k"],
        ));
        let cfg = cfg_de_prueba();
        let acciones = parar(&f, &cfg, &[Sintoma::MultiplicacionProcesos]);
        assert!(
            acciones
                .iter()
                .any(|a| a.contains("11") && a.contains("parado")),
            "la copia .bak-50k se mata: {acciones:?}"
        );
        assert!(
            !acciones
                .iter()
                .any(|a| a.contains("pid 10") && a.contains("parado")),
            "el canonico NO se mata: {acciones:?}"
        );
    }

    #[test]
    fn el_canonico_se_reconoce_elemento_a_elemento() {
        // MEDIDO (2026-11-28): la copia .bak-50k tiene la ruta canonica como
        // PREFIJO => un `contains` la tomaria por la canonica.
        let canonica = "/data/adb/service.d/99-megadaemon.sh";
        let p = Proceso::nuevo(11, &["sh", "/data/adb/service.d/99-megadaemon.sh.bak-50k"]);
        assert!(!es_supervisor_canonico(&p, canonica));
        assert!(es_supervisor_no_canonico(&p, canonica));
        let c = Proceso::nuevo(10, &["sh", "/data/adb/service.d/99-megadaemon.sh"]);
        assert!(es_supervisor_canonico(&c, canonica));
        assert!(!es_supervisor_no_canonico(&c, canonica));
    }

    #[test]
    fn no_se_puede_leer_el_cmdline_no_cuenta_como_muerto() {
        // MEDIDO (2026-11-28): `[ -r /proc/<pid>/cmdline ]` falla cuando
        // SELinux no deja leer => "no se puede leer" se confundia con
        // "esta muerto" => el guardian relanzaba el supervisor en bucle.
        let mut f = Fake::nuevo();
        f.con_proceso(Proceso::ilegible(77));
        assert!(
            vivo(&f, 77),
            "un cmdline ilegible con /proc y senal 0 esta VIVO"
        );
        f.vivos.borrow_mut().remove(&77);
        assert!(!vivo(&f, 77), "si no hay /proc ni senal 0, no esta vivo");
    }

    #[test]
    fn el_daemon_nunca_se_mata_a_si_mismo() {
        // Si el propio daemon es el DUPLICADO (o sea, hay otro mas
        // antiguo), el motor NO lo mata: matarse seria un suicidio.
        let mut f = entorno_sano();
        let yo = std::process::id() as i32;
        let antiguo = yo.saturating_sub(1).max(1);
        f.con_proceso(Proceso::nuevo(
            antiguo,
            &["/data/adb/megadaemon/megadaemon-core", "cockpit", "9090"],
        ))
        .con_proceso(Proceso::nuevo(
            yo,
            &["/data/adb/megadaemon/megadaemon-core", "cockpit", "9090"],
        ));
        let cfg = cfg_de_prueba();
        let acciones = parar(&f, &cfg, &[Sintoma::MultiplicacionProcesos]);
        assert!(
            acciones
                .iter()
                .any(|a| a.contains("no se mata al propio daemon")),
            "el daemon no se suicida: {acciones:?}"
        );
        assert!(
            !acciones
                .iter()
                .any(|a| a.contains(&format!("pid {yo} ")) && a.contains("parado")),
            "el propio daemon nunca sale como 'parado': {acciones:?}"
        );
    }

    #[test]
    fn el_duplicado_mas_nuevo_del_daemon_se_mata() {
        // El caso normal: el propio daemon es el mas antiguo (se queda)
        // y el duplicado mas nuevo se mata.
        let mut f = entorno_sano();
        let yo = std::process::id() as i32;
        let nuevo = yo + 1;
        f.con_proceso(Proceso::nuevo(
            yo,
            &["/data/adb/megadaemon/megadaemon-core", "cockpit", "9090"],
        ))
        .con_proceso(Proceso::nuevo(
            nuevo,
            &["/data/adb/megadaemon/megadaemon-core", "cockpit", "9090"],
        ));
        let cfg = cfg_de_prueba();
        let acciones = parar(&f, &cfg, &[Sintoma::MultiplicacionProcesos]);
        assert!(
            acciones
                .iter()
                .any(|a| a.contains(&nuevo.to_string()) && a.contains("parado")),
            "el duplicado se mata: {acciones:?}"
        );
    }

    // --- el presupuesto ---------------------------------------------------

    #[test]
    fn el_presupuesto_limita_las_restauraciones_por_hora() {
        let f = Fake::nuevo();
        let cfg = cfg_de_prueba();
        let ahora = 1_700_000_000i64;
        let mut e = Estado::default();
        for i in 0..3 {
            e.restauraciones.push(Restauracion {
                ts: ahora - 60 * i,
                sintomas: vec!["x".to_string()],
                acciones: vec![],
                resultado: "ok".to_string(),
                duracion_s: 1,
            });
        }
        let bloqueo = motivo_bloqueo(&f, &cfg, &e, &[Sintoma::TunelCaido], ahora);
        assert!(bloqueo.contains("presupuesto"), "{bloqueo}");
        // Y con 2 restauraciones todavia se puede.
        e.restauraciones.pop();
        let bloqueo = motivo_bloqueo(&f, &cfg, &e, &[Sintoma::TunelCaido], ahora);
        assert!(bloqueo.is_empty(), "{bloqueo}");
    }

    #[test]
    fn el_backoff_crece_tras_un_fallo() {
        let f = Fake::nuevo();
        let cfg = cfg_de_prueba();
        let ahora = 1_700_000_000i64;
        let mk = |fallos: u32, proximo: i64| Estado {
            restauraciones: vec![],
            fallos_consecutivos: fallos,
            ultimo_intento: ahora,
            ultimo_exito: 0,
            proximo_permitido: proximo,
        };
        // 1 fallo => 5 min
        let e = mk(1, ahora + 5 * 60);
        assert!(motivo_bloqueo(&f, &cfg, &e, &[Sintoma::TunelCaido], ahora).contains("backoff"));
        // 2 fallos => 15 min
        let e = mk(2, ahora + 15 * 60);
        assert!(motivo_bloqueo(&f, &cfg, &e, &[Sintoma::TunelCaido], ahora).contains("backoff"));
        // 4 fallos => 120 min
        let e = mk(4, ahora + 120 * 60);
        assert!(motivo_bloqueo(&f, &cfg, &e, &[Sintoma::TunelCaido], ahora).contains("backoff"));
        // pasado el backoff, se puede
        let e = mk(1, ahora - 1);
        assert!(motivo_bloqueo(&f, &cfg, &e, &[Sintoma::TunelCaido], ahora).is_empty());
    }

    #[test]
    fn la_marca_de_ya_restaurado_evita_el_bucle() {
        let f = Fake::nuevo();
        let cfg = cfg_de_prueba();
        let ahora = 1_700_000_000i64;
        let e = Estado {
            restauraciones: vec![Restauracion {
                ts: ahora - 60,
                sintomas: vec![Sintoma::TunelCaido.nombre().to_string()],
                acciones: vec![],
                resultado: "ok".to_string(),
                duracion_s: 1,
            }],
            fallos_consecutivos: 0,
            ultimo_intento: ahora - 60,
            ultimo_exito: ahora - 60,
            proximo_permitido: ahora + 1800,
        };
        // Los MISMOS síntomas dentro del enfriamiento: no se repite.
        let bloqueo = motivo_bloqueo(&f, &cfg, &e, &[Sintoma::TunelCaido], ahora);
        assert!(bloqueo.contains("ya restaurado"), "{bloqueo}");
        // Un síntoma NUEVO: sí se actúa (es otro problema).
        let bloqueo = motivo_bloqueo(&f, &cfg, &e, &[Sintoma::CanalNoPublicado], ahora);
        assert!(bloqueo.is_empty(), "{bloqueo}");
    }

    #[test]
    fn un_fallo_no_reintenta_el_historial() {
        // Un fallo NO debe reejecutar las acciones de una restauracion
        // anterior: cada intento recalcula el plan desde el estado actual.
        let f = Fake::nuevo();
        let cfg = cfg_de_prueba();
        let mut e = Estado::default();
        e.restauraciones.push(Restauracion {
            ts: 1_699_999_000,
            sintomas: vec![Sintoma::TunelCaido.nombre().to_string()],
            acciones: vec!["ACCION ANTIGUA".to_string()],
            resultado: "fallo".to_string(),
            duracion_s: 3,
        });
        e.fallos_consecutivos = 1;
        let ahora = 1_700_000_000i64;
        // El backoff esta EN FUTURO (el fallo fue hace nada): bloquea.
        e.proximo_permitido = ahora + 500;
        // El backoff bloquea (fallo reciente)...
        let bloqueo = motivo_bloqueo(&f, &cfg, &e, &[Sintoma::TunelCaido], ahora);
        assert!(bloqueo.contains("backoff"), "{bloqueo}");
        // ...y el estado se PERSISTE (es el que lee `restaurar`).
        guardar_estado(&f, &cfg.ruta_estado(), &e).unwrap();
        // El informe NO arrastra las acciones antiguas.
        let informe = restaurar(&f, &cfg, &[Sintoma::TunelCaido], false);
        assert_eq!(informe.resultado, Resultado::Presupuesto);
        assert!(
            !informe
                .acciones
                .iter()
                .any(|a| a.contains("ACCION ANTIGUA")),
            "no se reintenta el historial: {:?}",
            informe.acciones
        );
        // Y el estado guardado sigue siendo el mismo (el bloqueo no
        // consume presupuesto ni ensucia el registro).
        let leido = cargar_estado(&f, &cfg.ruta_estado());
        assert_eq!(leido.restauraciones.len(), 1);
        assert_eq!(leido.fallos_consecutivos, 1);
    }

    // --- el diagnóstico ---------------------------------------------------

    #[test]
    fn un_entorno_sano_no_tiene_sintomas() {
        let f = entorno_sano();
        let cfg = cfg_de_prueba();
        let d = diagnosticar(&f, &cfg);
        let rotos: Vec<&str> = d
            .iter()
            .filter(|x| x.presente)
            .map(|x| x.sintoma.nombre())
            .collect();
        assert!(
            rotos.is_empty(),
            "un entorno sano no tiene sintomas: {rotos:?}"
        );
    }

    #[test]
    fn cada_sintoma_tiene_una_accion() {
        for s in Sintoma::todos() {
            assert!(!s.que().is_empty(), "{} sin 'que'", s.nombre());
            assert!(!s.toca().is_empty(), "{} sin 'toca'", s.nombre());
            assert!(!s.nombre().is_empty());
        }
        // Los 11 nombres son distintos (son el contrato con el jefe).
        let mut nombres: Vec<&str> = Sintoma::todos().iter().map(|s| s.nombre()).collect();
        nombres.sort_unstable();
        let n = nombres.len();
        nombres.dedup();
        assert_eq!(nombres.len(), n);
    }

    #[test]
    fn el_stack_desfasado_se_detecta() {
        let mut f = entorno_sano();
        f.stack_desfasados.replace(3);
        let cfg = cfg_de_prueba();
        let d = diagnosticar(&f, &cfg);
        assert!(presente(&d, Sintoma::StackDesfasado));
    }

    #[test]
    fn el_8765_con_listener_es_un_sintoma() {
        let mut f = entorno_sano();
        f.puertos.borrow_mut().push(8765);
        let cfg = cfg_de_prueba();
        let d = diagnosticar(&f, &cfg);
        assert!(presente(&d, Sintoma::PuertoLegacyVivo));
    }

    #[test]
    fn la_url_de_la_api_se_detecta_como_sintoma() {
        let mut f = entorno_sano();
        f.ficheros.borrow_mut().insert(
            "/data/adb/megadaemon/tunnel.url".to_string(),
            "https://api.trycloudflare.com".to_string(),
        );
        let cfg = cfg_de_prueba();
        let d = diagnosticar(&f, &cfg);
        assert!(presente(&d, Sintoma::UrlTunelInvalida));
    }

    #[test]
    fn el_build_desfasado_se_detecta() {
        let mut f = entorno_sano();
        // El binario EN SERVICIO es el del arranque; el de DISCO es otro
        // => el proceso lleva un build viejo (MEDIDO 2026-11-25: 7 h de
        // desfase entre /health y el codigo real).
        f.sha_disco.replace("sha-nuevo-en-disco".to_string());
        let cfg = cfg_de_prueba();
        let d = diagnosticar(&f, &cfg);
        assert!(presente(&d, Sintoma::BinarioDesfasado));
    }

    #[test]
    fn el_binario_en_servicio_lo_manda_el_motor_base() {
        // NO es una ruta hardcodeada: `/data/local/tmp` es VOLATIL.
        let cfg = cfg_de_prueba();
        assert_eq!(cfg.binario(), "/data/adb/megadaemon/megadaemon-core");
    }

    // --- la restauración --------------------------------------------------

    #[test]
    fn restaurar_es_idempotente() {
        let f = entorno_sano();
        let cfg = cfg_de_prueba();
        let i1 = restaurar(&f, &cfg, &[Sintoma::UrlTunelInvalida], false);
        assert_eq!(i1.resultado, Resultado::Ok);
        let i2 = restaurar(&f, &cfg, &[Sintoma::UrlTunelInvalida], false);
        // La segunda pasada no hace nada (la marca de "ya restaurado").
        assert_eq!(i2.resultado, Resultado::Presupuesto);
        assert!(i2.motivo.unwrap().contains("ya restaurado"));
    }

    #[test]
    fn la_restauracion_deja_rastro() {
        let f = entorno_sano();
        let cfg = cfg_de_prueba();
        let informe = restaurar(&f, &cfg, &[Sintoma::UrlTunelInvalida], false);
        let estado = cargar_estado(&f, &cfg.ruta_estado());
        assert_eq!(estado.restauraciones.len(), 1);
        let r = &estado.restauraciones[0];
        assert_eq!(r.sintomas, vec!["url_tunel_invalida".to_string()]);
        assert!(!r.acciones.is_empty(), "el registro dice que se hizo");
        assert_eq!(r.resultado, "ok");
        // Y hay log.
        let log = f.leer(&cfg.ruta_log()).unwrap_or_default();
        assert!(log.contains("url_tunel_invalida"), "{log}");
        assert!(!informe.verificacion.is_empty());
    }

    #[test]
    fn el_registro_no_crece_sin_limite() {
        let f = entorno_sano();
        let cfg = cfg_de_prueba();
        let mut e = Estado::default();
        for i in 0..60 {
            e.restauraciones.push(Restauracion {
                ts: 1_700_000_000 + i,
                sintomas: vec![],
                acciones: vec![],
                resultado: "ok".to_string(),
                duracion_s: 0,
            });
        }
        let mut e2 = e.clone();
        e2.restauraciones.truncate(50);
        guardar_estado(&f, &cfg.ruta_estado(), &e2).unwrap();
        let leido = cargar_estado(&f, &cfg.ruta_estado());
        assert!(leido.restauraciones.len() <= 50);
    }

    #[test]
    fn la_url_invalida_se_resetea() {
        let f = entorno_sano();
        let cfg = cfg_de_prueba();
        let acciones = limpiar_urls(&f, &cfg);
        assert!(acciones.iter().any(|a| a.contains("tunnel.url")));
        assert!(f.leer("/data/adb/megadaemon/tunnel.url").is_none());
    }

    #[test]
    fn las_copias_bak_de_service_d_se_borran() {
        let mut f = entorno_sano();
        f.con_fichero("/data/adb/service.d/99-megadaemon.sh.bak-50k", "sh")
            .con_fichero("/data/adb/service.d/98-cloudflared.sh.bak", "sh");
        let cfg = cfg_de_prueba();
        let acciones = limpiar_copias_service_d(&f, &cfg);
        assert!(acciones
            .iter()
            .any(|a| a.contains("99-megadaemon.sh.bak-50k")));
        assert!(acciones.iter().any(|a| a.contains("98-cloudflared.sh.bak")));
        // El canónico NO se toca.
        assert!(f.leer("/data/adb/service.d/99-megadaemon.sh").is_some());
    }

    #[test]
    fn el_tunel_se_arranca_solo_si_no_hay_ninguno() {
        let mut f = entorno_sano();
        let cfg = cfg_de_prueba();
        // No hay cloudflared: se arranca.
        let acciones = arrancar_tunel(&f, &cfg);
        assert!(acciones.iter().any(|a| a.contains("tunel arrancado")));
        assert_eq!(f.lanzados.borrow().len(), 1);
        // Ahora hay uno: no se arranca otro.
        let mut f2 = entorno_sano();
        f2.con_proceso(Proceso::nuevo(
            30,
            &["/data/adb/megadaemon/cloudflared", "tunnel", "--url", "x"],
        ));
        let acciones = arrancar_tunel(&f2, &cfg);
        assert!(acciones.is_empty(), "{acciones:?}");
    }

    #[test]
    fn el_daemon_no_se_reinicia_sin_supervisor() {
        let mut f = entorno_sano();
        f.con_proceso(Proceso::nuevo(
            500,
            &["/data/adb/megadaemon/megadaemon-core", "cockpit", "9090"],
        ));
        let cfg = cfg_de_prueba();
        let acciones = reiniciar_daemon(&f, &cfg);
        assert!(
            acciones
                .iter()
                .any(|a| a.contains("no hay supervisor vivo")),
            "{acciones:?}"
        );
    }

    #[test]
    fn el_ciclo_no_hace_nada_si_no_hay_sintomas() {
        let f = entorno_sano();
        let cfg = cfg_de_prueba();
        assert!(ciclo(&f, &cfg).is_none());
    }

    #[test]
    fn el_ciclo_restaura_si_hay_sintomas() {
        let mut f = entorno_sano();
        f.con_proceso(Proceso::nuevo(
            10,
            &["sh", "/data/adb/service.d/99-megadaemon.sh"],
        ))
        .con_proceso(Proceso::nuevo(
            11,
            &["sh", "/data/adb/service.d/99-megadaemon.sh.bak-50k"],
        ));
        let cfg = cfg_de_prueba();
        let informe = ciclo(&f, &cfg).expect("hay sintomas: hay restauracion");
        assert_eq!(informe.resultado, Resultado::Ok);
        assert!(informe
            .acciones
            .iter()
            .any(|a| a.contains("11") && a.contains("parado")));
    }

    #[test]
    fn la_parada_va_en_el_orden_inverso_al_de_arranque() {
        // keeper → watchdog → supervisor → cloudflared → daemon: el keeper
        // va PRIMERO porque es el que relanza al supervisor.
        let orden: Vec<&str> = Rol::orden_parada().iter().map(|r| r.nombre()).collect();
        assert_eq!(
            orden,
            vec!["keeper", "watchdog", "supervisor", "cloudflared", "daemon"]
        );
    }

    #[test]
    fn el_tunel_caido_arranca_en_frio() {
        // Con el tunel caido el cloudflared se para ENTERO (se relanza).
        let mut f = entorno_sano();
        f.con_proceso(Proceso::nuevo(
            30,
            &["/data/adb/megadaemon/cloudflared", "tunnel", "--url", "x"],
        ))
        .con_proceso(Proceso::nuevo(
            31,
            &["/data/adb/megadaemon/cloudflared", "tunnel", "--url", "y"],
        ));
        let cfg = cfg_de_prueba();
        let acciones = parar(&f, &cfg, &[Sintoma::TunelCaido]);
        assert!(
            acciones
                .iter()
                .any(|a| a.contains("30") && a.contains("parado")),
            "el cloudflared se para en frio: {acciones:?}"
        );
        assert!(
            acciones
                .iter()
                .any(|a| a.contains("31") && a.contains("parado")),
            "y se para el otro tambien: {acciones:?}"
        );
        assert!(acciones.iter().any(|a| a.contains("arranque en frio")));
    }

    #[test]
    fn el_estado_json_lo_expone_todo() {
        let f = entorno_sano();
        let cfg = cfg_de_prueba();
        let v = estado_json(&f, &cfg);
        assert!(v.get("sintomas").is_some());
        assert!(v.get("presupuesto").is_some());
        assert!(v.get("registro").is_some());
        assert_eq!(v["presupuesto"]["max_por_hora"], json!(3));
    }
}
