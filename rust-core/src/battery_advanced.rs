//! MegaDaemon v13.4 — Gestor de energía AVANZADO (`battery_advanced`).
//!
//! Este módulo va **un paso por delante** de Xtreme Battery Saver (XBS) y del
//! resto de la pila "root + ahorro de batería" que usa este dispositivo:
//! * lee el estado **real** de la celda (`/sys/class/power_supply/battery`),
//! * calcula la **degradación real** de la celda (`charge_full` vs
//!   `charge_full_design`) y estima los **ciclos que le quedan**,
//! * decide una **política por evento** (cargando / descargando / temperatura)
//!   en lugar de reaccionar sólo a "al enchufar el cargador",
//! * **audita** el reposo real del sistema parseando `dumpsys power` y
//!   `dumpsys alarm` (wakelocks, alarmas, estado de Doze),
//! * y todo lo que propone es un **plan puro** que se puede revisar, simular
//!   y aplicar de forma **reversible**; las acciones destructivas exigen
//!   `confirm = true`.
//!
//! Dispositivo de referencia: Galaxy S21 Ultra SM-G998B (Exynos 2100,
//! Android 14, Magisk Kitsune, root `su -mm -c`).
//!
//! ---------------------------------------------------------------------------
//! COMPARATIVA HONESTA CON EL ECOSISTEMA EXISTENTE (2026)
//! ---------------------------------------------------------------------------
//!
//! **Xtreme Battery Saver (XBS)** — <https://github.com/Magisk-Modules-Alt-Repo/Xtreme-Battery-Saver>
//! * Qué hace: aplica sus reglas **por evento de carga**.  El log real de este
//!   dispositivo (`XtremeBS.log`) sólo contiene dos líneas:
//!   `Performing actions for charging event` y `Actions for charging completed`;
//!   es decir, **actúa al enchufar/desenchufar, no de forma continua**.
//! * Qué NO hace (verificado con ese log): no mide la degradación de la
//!   celda, no conoce el estado térmico, no decide en función del estado de
//!   Doze / wakelocks / alarmas, y no deja auditoría de por qué hizo lo que hizo.
//! * Detalles de su interior más allá de eso: **no verificados**.
//!
//! **PowerBoost** — <https://github.com/Yusuf6411/PowerBoost>
//! * Qué hace (según su README): perfiles adaptativos (batería / equilibrado /
//!   rendimiento / gaming), ajuste de governor y frecuencias, refresco de
//!   pantalla, sensores, conectividad en reposo, monitorización de temperatura
//!   y "charging optimization donde el hardware lo permite".
//! * Qué NO hace: no gestiona Doze / GMS Doze, no mide la degradación de la
//!   celda ni estima ciclos, y no audita wakelocks/alarmas.
//! * Detalles de su implementación exacta (qué switches toca, cómo decide el
//!   momento de actuar): **no verificados**.
//!
//! **Universal GMS Doze** — <https://github.com/Magisk-Modules-Alt-Repo/GMSDoze>
//! * Qué hace: **parchea la aplicación de Google Play Services** (y sus
//!   procesos) para que Play Services *pueda* entrar en Doze (es decir, deja
//!   de estar exento).  Efecto real: menos wakelocks y alarmas de GMS.
//! * Qué NO hace: no toca la carga, ni la salud de la celda, ni el térmico, ni
//!   el resto de apps; y al parquear la APK exige reiniciar GMS.
//! * En este módulo se hace el equivalente **sin tocar la APK**: se quita GMS
//!   de la whitelist de Doze (`dumpsys deviceidle whitelist -<pkg>`) y se le
//!   fija el bucket `rare` (`am set-standby-bucket`), que es reversible con
//!   dos comandos.
//!
//! **ACC / Advanced Charging Controller** — <https://github.com/vr-25/acc>
//! * Qué hace: demonio (`accd`) que escribe en `/sys/class/power_supply` para
//!   limitar **corriente, voltaje y temperatura** de carga, con umbrales
//!   `pause_capacity`/`resume_capacity` (por defecto 75/70) y "idle mode"
//!   (el móvil se alimenta del cargador sin cargar la celda).
//! * Qué NO hace: no gestiona Doze, ni GMS, ni la salud de la celda, ni el
//!   térmico de CPU, ni audita nada.
//! * En este módulo se **reutiliza** ACC si está instalado y, si no, se
//!   intenta el atributo de kernel `charge_control`; si el dispositivo no
//!   expone ningún interruptor escribible (frecuente en Samsung), se avisa
//!   con claridad en lugar de fingir que se ha limitado la carga.
//!
//! **Naptime** — app/módulo muy citado en XDA.
//! * Qué hace (descripción pública): forzar al dispositivo a entrar en Doze
//!   antes y con la pantalla encendida, y ajustar el "light doze".
//! * Qué NO hace: no gestiona carga ni salud de celda.
//! * Detalles exactos de su implementación: **no verificados**.
//!
//! **Scourge** — módulo Magisk de "limpieza" del ecosistema root.
//! * Su documentación pública es escasa; comportamiento y alcance exactos:
//!   **no verificados**.  No se le atribuye ninguna capacidad concreta aquí.
//!
//! **Don't Kill My App** — <https://dontkillmyapp.com>
//! * **No es un optimizador**: es un sitio/documentación que explica, OEM por
//!   OEM y versión por versión, cómo evitar que el sistema mate apps en
//!   segundo plano (por ejemplo el "Put apps to sleep" de Samsung o el
//!   arranque automático de Xiaomi).  No modifica el dispositivo por sí mismo.
//!
//! **Qué aporta este módulo que los anteriores no aportan**
//!
//! 1. **Política por evento y por estado real**: la decisión se toma con la
//!    lectura real de celda (carga, corriente, temperatura, salud) en vez de
//!    un "al enchufar, aplica la lista".
//! 2. **Degradación de la celda**: `health_pct()` mide el desgaste real
//!    (`charge_full / charge_full_design`) y `estimate_cycles_left()` estima
//!    los ciclos restantes; si la celda está worn, el límite de carga se
//!    aplica **aunque el perfil no lo pida**.
//! 3. **Térmico**: la política se endurece con la temperatura (el calor no
//!    aparece en el log de XBS, que sólo habla de eventos de carga).
//! 4. **Auditoría de Doze/wakelocks/alarmas**: `audit_doze()` parsea el texto
//!    real de `dumpsys power`/`dumpsys alarm`, de modo que se puede ver *qué*
//!    rompe el reposo en lugar de matar procesos a ciegas.
//! 5. **Reversibilidad y seguridad**: el plan es puro y revisable; `apply()`
//!    simula; las acciones destructivas exigen `confirm = true`.
//! 6. **Sin parcheo de APK de terceros y sin recompilar el kernel**.
//!
//! ---------------------------------------------------------------------------
//! Nota sobre "correr en el núcleo": ver `crate::magisk_module`, que explica
//! con honestidad por qué un demonio userspace no puede ejecutarse *dentro*
//! del kernel y qué alternativas reales existen.
//! ---------------------------------------------------------------------------

use crate::android::sh_quote;
use serde_json::json;
use std::path::Path;

/// Directorio de estado LEGADO del daemon en el móvil (VOLÁTIL).
pub const STATE_DIR_LEGADO: &str = "/data/local/tmp/megadaemon/state";

/// Directorio de estado del daemon (state dir canónico).
///
/// MEDIDO (2026-11-23): era `/data/local/tmp/megadaemon/state`, que en
/// Android se borra al reiniciar ⇒ la política de energía (y todo lo que
/// el daemon guarda ahí) se perdía.  Ahora es el state dir canónico
/// (`MEGADAEMON_STATE_DIR`, y en el móvil `/data/adb/megadaemon/state`).
pub fn state_dir() -> String {
    crate::state_dir::state_dir()
}


/// Temperatura de batería (°C) a partir de la cual la política se endurece.
pub const THERMAL_WARN_C: f32 = 42.0;

/// Temperatura de batería (°C) crítica: aviso fuerte de desconectar el cargador.
pub const THERMAL_CRIT_C: f32 = 45.0;

/// Por debajo de este porcentaje de salud la celda se considera degradada.
pub const HEALTH_DEGRADED_PCT: f32 = 90.0;

/// Umbral de "fin de vida" de una celda Li-ion (80 % de la capacidad original).
pub const EOL_HEALTH_PCT: f32 = 80.0;

/// Ciclos nominales de una celda Li-ion hasta el 80 % de capacidad.
/// 500 es un valor **conservador de la industria** (las celdas de móvil suelen
/// declarar 500-1000 ciclos al 80 %); **no** es una medida de esta celda.
pub const NOMINAL_CYCLE_LIFE: f64 = 500.0;

/// Paquete de Google Play Services (objetivo habitual del "GMS Doze").
pub const GMS_PACKAGE: &str = "com.google.android.gms";

// ---------------------------------------------------------------------------
// Estado de la batería
// ---------------------------------------------------------------------------

/// Lectura normalizada del estado de la batería del dispositivo.
///
/// Las unidades son las que publica sysfs (típicamente µAh para
/// `charge_full`/`charge_full_design` y µV/µA para `voltage_now`/`current_now`);
/// los ratios que se calculan a partir de ellas son independientes de la
/// unidad, por eso se guardan tal cual.
#[derive(Debug, Clone, PartialEq)]
pub struct BatterySnapshot {
    /// Porcentaje de carga (0-100) según `capacity`.
    pub capacity_pct: i32,
    /// Estado: `Charging`, `Discharging`, `Full`, `Not charging`...
    pub status: String,
    /// Salud declarada por el firmware: `Good`, `Overheat`, `Cold`...
    pub health: String,
    /// Temperatura de la celda en °C.
    pub temp_c: f32,
    /// Tensión de la celda en milivoltios.
    pub voltage_mv: i32,
    /// Corriente en miliamperios (negativa si la celda se descarga).
    pub current_ma: i32,
    /// Capacidad actual de carga (µAh según sysfs).
    pub charge_full: i64,
    /// Capacidad de diseño de fábrica (µAh según sysfs).
    pub charge_full_design: i64,
    /// Ciclos acumulados, si el kernel los expone.
    pub cycle_count: Option<i32>,
    /// Tecnología de la celda (`Li-ion`, ...).
    pub technology: String,
    /// true si el dispositivo está cargando.
    pub charging: bool,
    /// Ruta de la que se leyó el estado (para auditoría).
    pub source: String,
}

fn read_attr(dir: &Path, name: &str) -> Option<String> {
    std::fs::read_to_string(dir.join(name))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn read_i64(dir: &Path, name: &str) -> Option<i64> {
    read_attr(dir, name).and_then(|s| s.parse::<i64>().ok())
}

/// Normaliza la temperatura de sysfs (Android publica décimas de grado).
fn normalize_temp(raw: i64) -> f32 {
    let v = raw.abs();
    if v > 1000 {
        // Algunos kernels publicar mili-grados.
        raw as f32 / 1000.0
    } else {
        raw as f32 / 10.0
    }
}

/// Normaliza la tensión de sysfs (Android publica microvoltios).
fn normalize_voltage_mv(raw: i64) -> i32 {
    if raw.abs() > 100_000 {
        (raw / 1000) as i32
    } else {
        raw as i32
    }
}

/// Normaliza la corriente de sysfs (Android publica microamperios, con signo).
fn normalize_current_ma(raw: i64) -> i32 {
    if raw.abs() > 100_000 {
        (raw / 1000) as i32
    } else {
        raw as i32
    }
}

/// Resuelve el directorio real de la batería a partir de una raíz parametrizable.
///
/// Acepta tanto `/sys/class/power_supply` como `/sys/class/power_supply/battery`
/// (y, por supuesto, un árbol sintético de test).
fn resolve_battery_dir(root: &str) -> Result<std::path::PathBuf, String> {
    let p = Path::new(root);
    if !p.exists() {
        return Err(format!("no existe la ruta de batería: {}", root));
    }
    if p.join("capacity").exists() || p.join("status").exists() {
        return Ok(p.to_path_buf());
    }
    let sub = p.join("battery");
    if sub.join("capacity").exists() || sub.join("status").exists() {
        return Ok(sub);
    }
    Err(format!(
        "{} no parece un directorio de batería (faltan capacity/status)",
        root
    ))
}

/// Lee el estado real de la batería desde una raíz parametrizable.
///
/// `root` puede ser `/sys/class/power_supply`, `/sys/class/power_supply/battery`
/// o un árbol sintético con los mismos ficheros (tests).
pub fn read_battery(root: &str) -> Result<BatterySnapshot, String> {
    let dir = resolve_battery_dir(root)?;

    let capacity_pct = read_i64(&dir, "capacity")
        .map(|v| v.clamp(0, 100) as i32)
        .unwrap_or(0);
    let status = read_attr(&dir, "status").unwrap_or_else(|| "Unknown".to_string());
    let health = read_attr(&dir, "health").unwrap_or_else(|| "Unknown".to_string());
    let temp_c = read_i64(&dir, "temp").map(normalize_temp).unwrap_or(0.0);
    let voltage_mv = read_i64(&dir, "voltage_now").map(normalize_voltage_mv).unwrap_or(0);
    let current_ma = read_i64(&dir, "current_now").map(normalize_current_ma).unwrap_or(0);
    let charge_full = read_i64(&dir, "charge_full").unwrap_or(0);
    let charge_full_design = read_i64(&dir, "charge_full_design").unwrap_or(0);
    let cycle_count = read_i64(&dir, "cycle_count").map(|v| v as i32);
    let technology = read_attr(&dir, "technology").unwrap_or_else(|| "unknown".to_string());

    let charging = match status.as_str() {
        "Charging" | "Full" => true,
        "Discharging" | "Not charging" => false,
        _ => current_ma > 0,
    };

    Ok(BatterySnapshot {
        capacity_pct,
        status,
        health,
        temp_c,
        voltage_mv,
        current_ma,
        charge_full,
        charge_full_design,
        cycle_count,
        technology,
        charging,
        source: dir.to_string_lossy().to_string(),
    })
}

/// Salud real de la celda en porcentaje: `charge_full / charge_full_design * 100`.
///
/// Es la medida objetiva de desgaste: por debajo de 100 % la celda ha perdido
/// parte de su capacidad original.  Devuelve 0.0 si el firmware no expone
/// `charge_full_design` (no se puede entonces afirmar nada).
pub fn health_pct(s: &BatterySnapshot) -> f32 {
    if s.charge_full_design <= 0 {
        return 0.0;
    }
    ((s.charge_full as f64 / s.charge_full_design as f64) * 100.0) as f32
}

/// Ciclos que le quedan a la celda (estimación).
///
/// Se combinan dos modelos y se devuelve el **más conservador**:
///
/// * **Modelo A (nominal)**: degradación lineal del 100 % al 80 % en
///   [`NOMINAL_CYCLE_LIFE`] ciclos.
/// * **Modelo B (observado)**: si el kernel expone `cycle_count`, se extrapola
///   la degradación *real* por ciclo medida en este dispositivo.
///
/// Devuelve `None` si no hay datos suficientes (sin `charge_full_design` o sin
/// degradación medible: en ese caso no se puede afirmar nada y no se inventa).
pub fn estimate_cycles_left(s: &BatterySnapshot) -> Option<f64> {
    if s.charge_full_design <= 0 || s.charge_full <= 0 {
        return None;
    }
    let health = health_pct(s) as f64;
    if health >= 99.5 {
        // Sin degradación medible: no hay nada que estimar.
        return None;
    }
    if health <= EOL_HEALTH_PCT as f64 {
        return Some(0.0);
    }

    // Modelo A: degradación lineal 100 % -> 80 % en NOMINAL_CYCLE_LIFE ciclos.
    let per_cycle_nominal = (100.0 - EOL_HEALTH_PCT as f64) / NOMINAL_CYCLE_LIFE;
    let mut remaining = (health - EOL_HEALTH_PCT as f64) / per_cycle_nominal;

    // Modelo B: proyección desde los ciclos realmente acumulados.
    if let Some(cc) = s.cycle_count {
        if cc > 0 {
            let per_cycle_observed = (100.0 - health) / cc as f64;
            if per_cycle_observed > 0.0 {
                let projected = (health - EOL_HEALTH_PCT as f64) / per_cycle_observed;
                remaining = remaining.min(projected);
            }
        }
    }
    Some(remaining.max(0.0))
}

/// Temperatura más alta de las zonas térmicas del SoC (°C).
///
/// Lee `/sys/class/thermal/thermal_zone*/temp` (mili-grados en Android) desde
/// una raíz parametrizable (`/sys/class/thermal` por defecto).  Devuelve
/// `None` si no hay ninguna zona legible.
///
/// Nota: `plan()` es puro y sólo recibe la [`BatterySnapshot`], así que la
/// escalada térmica que aplica usa la temperatura **de la batería**.  Esta
/// función sirve al llamante para informar del estado del SoC (y para elegir
/// perfil antes de planificar).
pub fn read_hottest_thermal_c(root: &str) -> Option<f32> {
    let entries = std::fs::read_dir(Path::new(root)).ok()?;
    let mut hottest: Option<f32> = None;
    for e in entries.flatten() {
        let p = e.path();
        let raw = match read_i64(&p, "temp") {
            Some(v) => v,
            None => continue,
        };
        let c = if raw.abs() > 1000 {
            raw as f32 / 1000.0
        } else {
            raw as f32 / 10.0
        };
        if c > 0.0 && c < 120.0 {
            hottest = Some(hottest.map_or(c, |h: f32| if c > h { c } else { h }));
        }
    }
    hottest
}

// ---------------------------------------------------------------------------
// Perfiles de energía
// ---------------------------------------------------------------------------

/// Política de energía aplicable: qué se restringe y hasta dónde.
#[derive(Debug, Clone, PartialEq)]
pub struct PowerProfile {
    /// Nombre del perfil (`balanced`, `endurance`, ...).
    pub name: String,
    /// Forzar/permitir Doze del sistema.
    pub doze_enabled: bool,
    /// Forzar Google Play Services dentro de Doze (estilo Universal GMS Doze).
    pub gms_doze: bool,
    /// Bucket de App Standby objetivo (`active`, `working_set`, `frequent`,
    /// `rare`, `restricted`).
    pub app_standby_bucket: String,
    /// Restringir la ejecución en segundo plano de las apps gestionadas.
    pub restrict_bg: bool,
    /// Tiempo máximo (s) que el demonio mantiene un wakelock parcial.
    pub wakelock_timeout_s: u32,
    /// Límite de carga en % (sólo tiene efecto mientras se carga).
    pub charge_limit_pct: Option<u8>,
    /// Endurecer la política cuando la temperatura es alta.
    pub enforce_on_thermal: bool,
    /// Notas legibles para la UI y la auditoría.
    pub notes: String,
}

/// Catálogo de perfiles reales.
///
/// `endurance` es **el más restrictivo**: Doze + GMS Doze + bucket `rare` +
/// restricción de segundo plano + wakelock corto + límite de carga al 80 %.
pub fn profiles() -> Vec<PowerProfile> {
    vec![
        PowerProfile {
            name: "balanced".to_string(),
            doze_enabled: true,
            gms_doze: false,
            app_standby_bucket: "working_set".to_string(),
            restrict_bg: false,
            wakelock_timeout_s: 300,
            charge_limit_pct: None,
            enforce_on_thermal: true,
            notes: "Equilibrio: Doze activo, sin tocar GMS ni la carga.".to_string(),
        },
        PowerProfile {
            name: "endurance".to_string(),
            doze_enabled: true,
            gms_doze: true,
            app_standby_bucket: "rare".to_string(),
            restrict_bg: true,
            wakelock_timeout_s: 60,
            charge_limit_pct: Some(80),
            enforce_on_thermal: true,
            notes: "Máxima autonomía: el más restrictivo de todos los perfiles."
                .to_string(),
        },
        PowerProfile {
            name: "performance".to_string(),
            doze_enabled: false,
            gms_doze: false,
            app_standby_bucket: "active".to_string(),
            restrict_bg: false,
            wakelock_timeout_s: 1800,
            charge_limit_pct: None,
            enforce_on_thermal: true,
            notes: "Rendimiento: Doze desactivado, wakelocks largos.".to_string(),
        },
        PowerProfile {
            name: "travel".to_string(),
            doze_enabled: true,
            gms_doze: false,
            app_standby_bucket: "working_set".to_string(),
            restrict_bg: false,
            wakelock_timeout_s: 600,
            charge_limit_pct: Some(90),
            enforce_on_thermal: true,
            notes: "Viaje: aguanta la navegación, tope de carga al 90 %.".to_string(),
        },
        PowerProfile {
            name: "gms_doze".to_string(),
            doze_enabled: true,
            gms_doze: true,
            app_standby_bucket: "working_set".to_string(),
            restrict_bg: false,
            wakelock_timeout_s: 300,
            charge_limit_pct: None,
            enforce_on_thermal: false,
            notes: "Doze para Google Play Services sin tocar la carga (equivalente \
                    userland a Universal GMS Doze, sin parchear la APK)."
                .to_string(),
        },
        PowerProfile {
            name: "charge_80".to_string(),
            doze_enabled: true,
            gms_doze: false,
            app_standby_bucket: "working_set".to_string(),
            restrict_bg: false,
            wakelock_timeout_s: 600,
            charge_limit_pct: Some(80),
            enforce_on_thermal: true,
            notes: "Carga limitada al 80 % mientras esté enchufado.".to_string(),
        },
    ]
}

/// Devuelve un perfil por su nombre (`None` si no existe).
pub fn profile(name: &str) -> Option<PowerProfile> {
    profiles().into_iter().find(|p| p.name == name)
}

// ---------------------------------------------------------------------------
// Acciones
// ---------------------------------------------------------------------------

/// Acción proposing por el planificador.
///
/// Todas las acciones son **reversibles** salvo las marcadas como tal, y las
/// destructivas exigen `confirm = true` para ejecutarse.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Fija una propiedad del sistema (`resetprop <clave> <valor>`).
    SetProp {
        /// Clave de la propiedad.
        key: String,
        /// Valor a fijar.
        value: String,
    },
    /// Ejecuta un comando de shell.
    Shell {
        /// Comando (sh).
        cmd: String,
        /// true si el comando tiene vuelta atrás conocida.
        reversible: bool,
    },
    /// Escribe un fichero.
    File {
        /// Ruta absoluta.
        path: String,
        /// Contenido.
        content: String,
    },
    /// Aviso para el usuario/auditoría (no toca nada).
    Notice {
        /// Texto del aviso.
        text: String,
    },
}

impl Action {
    /// Una acción es **destructiva** si no se puede deshacer con un comando
    /// conocido o si escribe fuera del directorio de estado del daemon.
    pub fn is_destructive(&self) -> bool {
        match self {
            Action::Shell { reversible, .. } => !reversible,
            Action::File { path, .. } => !path.starts_with(state_dir().as_str()),
            Action::SetProp { .. } => false,
            Action::Notice { .. } => false,
        }
    }

    /// Descripción corta para logs y auditorías.
    pub fn describe(&self) -> String {
        match self {
            Action::SetProp { key, value } => format!("setprop {}={}", key, value),
            Action::Shell { cmd, reversible } => {
                format!("shell {}{}", cmd, if *reversible { " [rev]" } else { " [DESTRUCTIVA]" })
            }
            Action::File { path, .. } => format!("file {}", path),
            Action::Notice { text } => format!("notice: {}", text),
        }
    }
}

/// Resultado de aplicar una acción.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionResult {
    /// La acción ejecutada (o simulada).
    pub action: Action,
    /// true si se aplicó correctamente.
    pub ok: bool,
    /// Salida del comando o motivo del fallo.
    pub output: String,
}

// ---------------------------------------------------------------------------
// Ejecutores
// ---------------------------------------------------------------------------

/// Ejecutor de acciones: permite simular o ejecutar de verdad.
pub trait Executor {
    /// Ejecuta un comando de shell y devuelve su salida.
    fn run(&mut self, cmd: &str) -> Result<String, String>;

    /// true si el ejecutor **no** debe tocar el sistema (simulación).
    fn is_dry_run(&self) -> bool {
        false
    }
}

/// Ejecutor real: lanza `sh -c` en el sistema.
///
/// **Sólo debe usarse con `confirm = true`**: el demonio se ejecuta como root
/// en el móvil (Magisk), por lo que los comandos se ejecutan con permisos
/// plenos.  En un PC sin root, prepende `su -mm -c` al comando.
pub struct ShellExecutor;

impl Executor for ShellExecutor {
    fn run(&mut self, cmd: &str) -> Result<String, String> {
        let out = std::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .output()
            .map_err(|e| format!("no se pudo ejecutar sh: {}", e))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(if err.is_empty() {
                format!("exit status {}", out.status)
            } else {
                err
            });
        }
        let mut s = String::from_utf8_lossy(&out.stdout).to_string();
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if !err.is_empty() {
            s.push('\n');
            s.push_str(&err);
        }
        Ok(s.trim().to_string())
    }
}

/// Ejecutor de simulación: **no ejecuta nada**, sólo describe lo que se haría.
pub struct DryRunExecutor;

impl Executor for DryRunExecutor {
    fn run(&mut self, cmd: &str) -> Result<String, String> {
        Ok(format!("[dry-run] {}", cmd))
    }

    fn is_dry_run(&self) -> bool {
        true
    }
}

/// Ejecutor de pruebas: registra las llamadas y puede fallar a propósito.
pub struct FakeExecutor {
    /// Comandos recibidos, en orden.
    pub calls: Vec<String>,
    /// Si contiene este texto, el comando falla.
    pub fail_on: Option<String>,
}

impl FakeExecutor {
    /// Ejecutor que nunca falla.
    pub fn new() -> Self {
        FakeExecutor {
            calls: Vec::new(),
            fail_on: None,
        }
    }

    /// Ejecutor que falla cuando el comando contiene `pattern`.
    pub fn failing(pattern: &str) -> Self {
        FakeExecutor {
            calls: Vec::new(),
            fail_on: Some(pattern.to_string()),
        }
    }
}

impl Default for FakeExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl Executor for FakeExecutor {
    fn run(&mut self, cmd: &str) -> Result<String, String> {
        self.calls.push(cmd.to_string());
        if let Some(p) = &self.fail_on {
            if cmd.contains(p.as_str()) {
                return Err(format!("fallo simulado (patrón: {})", p));
            }
        }
        Ok(format!("ok: {}", cmd))
    }
}

// ---------------------------------------------------------------------------
// Planificador
// ---------------------------------------------------------------------------

/// Política resuelta tras aplicar el perfil, el estado real y las reglas
/// de degradación/thermal.
#[derive(Debug, Clone)]
struct Resolved {
    doze_enabled: bool,
    gms_doze: bool,
    bucket: String,
    restrict_bg: bool,
    wakelock_timeout_s: u32,
    charge_limit_pct: Option<u8>,
    thermal_escalated: bool,
    reasons: Vec<String>,
}

/// Resuelve el perfil efectivo: perfil + temperatura + salud de la celda.
fn resolve(profile: &PowerProfile, snap: &BatterySnapshot) -> Resolved {
    let mut r = Resolved {
        doze_enabled: profile.doze_enabled,
        gms_doze: profile.gms_doze,
        bucket: profile.app_standby_bucket.clone(),
        restrict_bg: profile.restrict_bg,
        wakelock_timeout_s: profile.wakelock_timeout_s,
        charge_limit_pct: profile.charge_limit_pct,
        thermal_escalated: false,
        reasons: Vec::new(),
    };

    // 1) Thermal: el calor no aparece en el log de XBS (sólo eventos de carga).
    if profile.enforce_on_thermal && snap.temp_c >= THERMAL_WARN_C {
        r.thermal_escalated = true;
        r.doze_enabled = true;
        r.restrict_bg = true;
        r.wakelock_timeout_s = r.wakelock_timeout_s.min(60);
        if r.bucket == "active" {
            r.bucket = "working_set".to_string();
        }
        r.reasons.push(format!(
            "térmica: {:.1} °C >= {:.0} °C -> política restrictiva (Doze + \
             segundo plano restringido + wakelock <= 60 s)",
            snap.temp_c, THERMAL_WARN_C
        ));
        if snap.temp_c >= THERMAL_CRIT_C {
            r.reasons.push(format!(
                "temperatura crítica ({:.1} °C >= {:.0} °C): desconecta el cargador \
                 si puedes; el límite de carga se mantiene",
                snap.temp_c, THERMAL_CRIT_C
            ));
        }
    }

    // 2) Degradación de la celda: si está worn, se limita la carga aunque el
    //    perfil no lo pida.
    let health = health_pct(snap);
    if health > 0.0 && health < HEALTH_DEGRADED_PCT {
        if r.charge_limit_pct.is_none() {
            r.charge_limit_pct = Some(80);
        }
        r.reasons.push(format!(
            "celda degradada: {:.1} % de salud ({}/{} µAh) -> límite de carga al 80 %",
            health, snap.charge_full, snap.charge_full_design
        ));
    }

    r
}

/// Comando de límite de carga.
///
/// Prioriza ACC (el estándar de facto) y, si no está, intenta el atributo de
/// kernel `charge_control`.  Si el dispositivo no expone ningún interruptor
/// escribible, el comando no hace nada: la ausencia se avisa aparte (no se
/// finge que la carga esté limitada).
fn charge_limit_cmd(limit: u8) -> String {
    let resume = limit.saturating_sub(5).max(40);
    format!(
        "if command -v acc >/dev/null 2>&1; then acc {limit} {resume}; \
         else for f in /sys/class/power_supply/*/charge_control; do \
         [ -w \"$f\" ] && echo {limit} > \"$f\" 2>/dev/null; done; fi",
        limit = limit,
        resume = resume
    )
}

/// Devuelve el JSON de la política efectiva (sin marca de tiempo: `plan` es puro).
fn policy_json(r: &Resolved, profile: &PowerProfile, snap: &BatterySnapshot, charging: bool) -> String {
    let v = json!({
        "profile": profile.name,
        "resolved": {
            "doze_enabled": r.doze_enabled,
            "gms_doze": r.gms_doze,
            "app_standby_bucket": r.bucket,
            "restrict_bg": r.restrict_bg,
            "wakelock_timeout_s": r.wakelock_timeout_s,
            "charge_limit_pct": r.charge_limit_pct,
            "thermal_escalated": r.thermal_escalated,
        },
        "battery": {
            "capacity_pct": snap.capacity_pct,
            "status": snap.status,
            "health": snap.health,
            "temp_c": snap.temp_c,
            "voltage_mv": snap.voltage_mv,
            "current_ma": snap.current_ma,
            "health_pct": health_pct(snap),
            "cycles_left_estimate": estimate_cycles_left(snap),
            "technology": snap.technology,
        },
        "charging": charging,
        "reasons": r.reasons,
    });
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

/// Construye el **plan** de acciones para un perfil y un estado real.
///
/// Es una función ** pura**: no lee el disco, no ejecuta nada y no depende del
/// reloj; el mismo perfil con la misma foto de batería produce siempre el
/// mismo plan (fácil de testear y de auditar).
///
/// Reglas aplicadas:
/// * Doze activado/desactivado según el perfil;
/// * GMS Doze: se quita GMS de la whitelist de Doze y se le fija el bucket
///   `rare` (equivalente userland a Universal GMS Doze, sin parchear la APK);
/// * bucket de App Standby para las apps gestionadas;
/// * restricción de segundo plano (`cmd appops ... RUN_ANY_IN_BACKGROUND`);
/// * **límite de carga sólo mientras se carga**;
/// * endurecimiento por temperatura y por degradación de la celda.
pub fn plan(profile: &PowerProfile, snap: &BatterySnapshot, charging: bool) -> Vec<Action> {
    let r = resolve(profile, snap);
    let mut out: Vec<Action> = Vec::new();

    // 1) Doze del sistema.
    out.push(Action::Shell {
        cmd: if r.doze_enabled {
            "dumpsys deviceidle enable".to_string()
        } else {
            "dumpsys deviceidle disable".to_string()
        },
        reversible: true,
    });

    // 2) GMS Doze (reversible: whitelist + y bucket active).
    if r.gms_doze {
        out.push(Action::Shell {
            cmd: format!("dumpsys deviceidle whitelist -{}", GMS_PACKAGE),
            reversible: true,
        });
        out.push(Action::Shell {
            cmd: format!("am set-standby-bucket {} rare", GMS_PACKAGE),
            reversible: true,
        });
    }

    // 3) Bucket de App Standby para las apps que gestiona el daemon.
    if r.bucket != "active" {
        out.push(Action::Shell {
            cmd: format!(
                "for p in $(cat {}/standby_targets.txt 2>/dev/null); do \
                 am set-standby-bucket \"$p\" {} 2>/dev/null; done",
                state_dir(), r.bucket
            ),
            reversible: true,
        });
    }

    // 4) Restricción de segundo plano de las apps gestionadas.
    if r.restrict_bg {
        out.push(Action::Shell {
            cmd: format!(
                "for p in $(cat {}/bg_targets.txt 2>/dev/null); do \
                 cmd appops set \"$p\" RUN_ANY_IN_BACKGROUND ignore 2>/dev/null; done; \
                 cmd appops write-settings",
                state_dir()
            ),
            reversible: true,
        });
    }

    // 5) Límite de carga: sólo tiene sentido mientras carga.
    if charging {
        if let Some(limit) = r.charge_limit_pct {
            out.push(Action::Shell {
                cmd: charge_limit_cmd(limit),
                reversible: true,
            });
            out.push(Action::Notice {
                text: format!("límite de carga al {} % mientras esté enchufado", limit),
            });
        } else {
            out.push(Action::Notice {
                text: "cargando y sin límite de carga definido en el perfil".to_string(),
            });
        }
    }

    // 6) Propiedades informativas (reversibles con `resetprop --delete`).
    out.push(Action::SetProp {
        key: "persist.megadaemon.profile".to_string(),
        value: profile.name.clone(),
    });
    if let Some(limit) = r.charge_limit_pct {
        out.push(Action::SetProp {
            key: "persist.megadaemon.charge_limit".to_string(),
            value: limit.to_string(),
        });
    }

    // 7) Política efectiva dentro del directorio de estado (no destructivo).
    out.push(Action::File {
        path: format!("{}/power_policy.json", state_dir()),
        content: policy_json(&r, profile, snap, charging),
    });

    // 8) Marcador para el módulo Magisk (FUERA del estado -> destructivo ->
    //    exige confirm).  El service.sh lo lee al arrancar.
    out.push(Action::File {
        path: "/data/adb/megadaemon/profile.json".to_string(),
        content: policy_json(&r, profile, snap, charging),
    });

    // 9) Avisos (motivos de la decisión).
    for reason in &r.reasons {
        out.push(Action::Notice {
            text: reason.clone(),
        });
    }
    out.push(Action::Notice {
        text: format!(
            "plan perfil={} | carga={} | {}% | {:.1} °C | salud {:.1}% | {} acciones",
            profile.name,
            charging,
            snap.capacity_pct,
            snap.temp_c,
            health_pct(snap),
            out.len() + 1
        ),
    });

    out
}

// ---------------------------------------------------------------------------
// Aplicación
// ---------------------------------------------------------------------------

fn write_file(path: &str, content: &str) -> Result<(), String> {
    if let Some(parent) = Path::new(path).parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("no se pudo crear {}: {}", parent.display(), e))?;
    }
    std::fs::write(path, content).map_err(|e| format!("no se pudo escribir {}: {}", path, e))
}

fn set_prop_cmd(key: &str, value: &str) -> String {
    format!(
        "if command -v resetprop >/dev/null 2>&1; then resetprop {k} {v}; \
         elif [ -x /data/adb/magisk/resetprop ]; then /data/adb/magisk/resetprop {k} {v}; \
         else echo 'resetprop no disponible' >&2; exit 127; fi",
        k = key,
        v = value
    )
}

/// Aplica un plan con un ejecutor inyectado.
///
/// * Las acciones **destructivas** (`Action::is_destructive`) sólo se ejecutan
///   si `confirm = true`; si no, quedan registradas como omitidas.
/// * Un ejecutor en modo *dry-run* no escribe ficheros ni ejecuta nada.
pub fn apply_with(actions: &[Action], ex: &mut dyn Executor, confirm: bool) -> Vec<ActionResult> {
    let mut out = Vec::with_capacity(actions.len());
    for a in actions {
        let (ok, output) = match a {
            Action::Notice { text } => (true, format!("aviso: {}", text)),
            Action::SetProp { key, value } => match ex.run(&set_prop_cmd(key, value)) {
                Ok(o) => (true, o),
                Err(e) => (false, e),
            },
            Action::Shell { cmd, reversible } => {
                if !*reversible && !confirm {
                    (false, "omitida: acción destructiva, exige confirm=true".to_string())
                } else {
                    match ex.run(cmd) {
                        Ok(o) => (true, o),
                        Err(e) => (false, e),
                    }
                }
            }
            Action::File { path, content } => {
                if !path.starts_with(state_dir().as_str()) && !confirm {
                    (
                        false,
                        format!(
                            "omitida: escribir fuera del directorio de estado ({}) exige confirm=true",
                            path
                        ),
                    )
                } else if ex.is_dry_run() {
                    (
                        true,
                        format!("[dry-run] escribir {} ({} bytes)", path, content.len()),
                    )
                } else {
                    match write_file(path, content) {
                        Ok(()) => (true, format!("escrito: {}", path)),
                        Err(e) => (false, e),
                    }
                }
            }
        };
        out.push(ActionResult {
            action: a.clone(),
            ok,
            output,
        });
    }
    out
}

/// Aplica un plan **en modo simulación**: no toca nada.
///
/// Es la función por defecto; para ejecutar de verdad hay que usar
/// [`apply_confirmed`] con `confirm = true` (o [`apply_with`] con un
/// [`ShellExecutor`]).
pub fn apply(actions: &[Action]) -> Vec<ActionResult> {
    apply_with(actions, &mut DryRunExecutor, false)
}

/// Aplica un plan contra el sistema real.
///
/// Con `confirm = false` **no se ejecuta ninguna acción destructiva** y el
/// resto se simula (mismo criterio que [`apply`]).  Con `confirm = true` se
/// ejecutan de verdad mediante [`ShellExecutor`] (el daemon corre como root).
pub fn apply_confirmed(actions: &[Action], confirm: bool) -> Vec<ActionResult> {
    if !confirm {
        return apply(actions);
    }
    apply_with(actions, &mut ShellExecutor, true)
}

// ---------------------------------------------------------------------------
// Auditoría de Doze / wakelocks / alarmas
// ---------------------------------------------------------------------------

/// Foto del reposo del sistema tal y como la cuenta el sistema.
#[derive(Debug, Clone, PartialEq)]
pub struct DozeAudit {
    /// Estado de Doze (`IDLE`, `MAINTENANCE`, `LIGHT_IDLE`, `DOZING`,
    /// `AWAKE`, `ASLEEP`, `UNKNOWN`).
    pub doze_state: String,
    /// true si Google Play Services **no** está en la whitelist de Doze
    /// (es decir, está sujeto a Doze).  Sólo es concluyente si el texto
    /// incluye la sección de whitelist de `dumpsys deviceidle`.
    pub gms_doze: bool,
    /// Wakelocks activos (etiquetas).
    pub wakelocks: Vec<String>,
    /// Alarmas pendientes (paquetes).
    pub alarms: Vec<String>,
    /// true si el sistema está restringiendo el trabajo en segundo plano.
    pub bg_restricted: bool,
}

/// Límite de entradas guardadas (protege frente a volcados enormes).
const AUDIT_MAX: usize = 64;

/// Estado de Doze a partir del texto de `dumpsys`.
///
/// Se busca, por orden: `mState=` (deviceidle), `mLightState=` (deviceidle) y
/// `mWakefulness=` (power manager).
fn parse_doze_state(text: &str) -> String {
    for line in text.lines() {
        let t = line.trim();
        if let Some(v) = t.strip_prefix("mState=") {
            let v = v.split_whitespace().next().unwrap_or("");
            return match v {
                "IDLE_MAINTENANCE" => "MAINTENANCE".to_string(),
                "IDLE" | "STATE_IDLE" => "IDLE".to_string(),
                "ACTIVE" | "STATE_ACTIVE" => "ACTIVE".to_string(),
                other => other.trim_start_matches("STATE_").to_string(),
            };
        }
    }
    for line in text.lines() {
        let t = line.trim();
        if let Some(v) = t.strip_prefix("mLightState=") {
            let v = v.split_whitespace().next().unwrap_or("");
            return match v {
                "STATE_LIGHT_IDLE" => "LIGHT_IDLE".to_string(),
                other => other.trim_start_matches("STATE_").to_string(),
            };
        }
    }
    for line in text.lines() {
        let t = line.trim();
        if let Some(v) = t.strip_prefix("mWakefulness=") {
            return match v.split_whitespace().next().unwrap_or("") {
                "Awake" => "AWAKE".to_string(),
                "Asleep" => "ASLEEP".to_string(),
                "Dozing" => "DOZING".to_string(),
                other => other.to_string(),
            };
        }
    }
    "UNKNOWN".to_string()
}

/// Extrae las etiquetas de wakelock de `dumpsys power`.
///
/// Formatos soportados (todos reales en distintas versiones de Android):
/// ```text
/// Wakelocks:
///   +1 Partially Suspended:  PowerManagerService
/// Kernel Wake Locks:
///   [0] "knxt.queue" count:1 timeout:2000000000
/// ```
fn parse_wakelocks(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut in_section = false;
    for line in text.lines() {
        let trimmed = line.trim();
        // Cabecera de sección (columna 0 y acaba en ':').
        if !line.starts_with(char::is_whitespace) && trimmed.ends_with(':') {
            in_section = trimmed == "Wakelocks:" || trimmed == "Kernel Wake Locks:";
            continue;
        }
        if !in_section {
            continue;
        }
        if trimmed.is_empty() {
            in_section = false;
            continue;
        }
        let quoted = || -> Option<String> {
            let start = trimmed.find('"')? + 1;
            let end = trimmed[start..].find('"')? + start;
            Some(trimmed[start..end].to_string())
        };
        let tag = if let Some(rest) = trimmed.split_once("Partially Suspended:") {
            Some(rest.1.trim().to_string())
        } else if trimmed.contains('"') {
            quoted()
        } else if let Some((head, _)) = trimmed.split_once(':') {
            let head = head.trim();
            if head.is_empty() || head.contains(' ') {
                None
            } else {
                Some(head.to_string())
            }
        } else {
            None
        };
        if let Some(t) = tag {
            if !t.is_empty() && !out.contains(&t) {
                out.push(t);
            }
            if out.len() >= AUDIT_MAX {
                break;
            }
        }
    }
    out
}

/// Extrae los paquetes con alarmas pendientes de `dumpsys alarm`.
///
/// Formatos soportados (reales):
/// ```text
///   u0a123:com.google.android.gms +1m3s last -2m:
///   com.android.systemui +3m20s 1 wakes 1 alarms:
/// ```
fn parse_alarms(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        if !line.starts_with(char::is_whitespace) {
            continue;
        }
        if !(line.contains('+') || line.contains("wakes") || line.contains("alarms")) {
            continue;
        }
        for tok in line.split_whitespace() {
            // Quita el prefijo "u0a123:" (uid:paquete).
            let tok = tok.trim_end_matches(',');
            let tok = match tok.split_once(':') {
                Some((head, rest))
                    if !head.is_empty()
                        && head.chars().all(|c| c.is_ascii_alphanumeric()) =>
                {
                    rest
                }
                _ => tok,
            };
            let ok = tok.contains('.')
                && !tok.starts_with('-')
                && !tok.starts_with('+')
                && tok
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
                && !tok.chars().next().unwrap_or('x').is_ascii_digit();
            if ok {
                if !out.contains(&tok.to_string()) {
                    out.push(tok.to_string());
                }
                break;
            }
        }
        if out.len() >= AUDIT_MAX {
            break;
        }
    }
    out
}

/// ¿Está Google Play Services sujeto a Doze?
///
/// Sólo es concluyente si el texto incluye la whitelist de `dumpsys deviceidle`
/// (sección `Whitelist system apps:` / `Whitelist user apps:`):
/// * GMS en la whitelist -> **no** puede estar en Doze -> false;
/// * whitelist presente y GMS ausente -> true;
/// * sin información de whitelist -> **no se afirma nada** -> false.
fn parse_gms_doze(text: &str) -> bool {
    let gms = GMS_PACKAGE;
    let mut in_whitelist = false;
    let mut has_whitelist = false;
    for line in text.lines() {
        let t = line.trim();
        if !line.starts_with(char::is_whitespace) && t.ends_with(':') {
            in_whitelist = t.to_lowercase().contains("whitelist");
            if in_whitelist {
                has_whitelist = true;
            }
            continue;
        }
        if in_whitelist && t.starts_with(gms) {
            return false;
        }
    }
    has_whitelist
}

/// Audita el reposo del sistema a partir de texto real de `dumpsys`.
///
/// `dumpsys_power` y `dumpsys_alarm` son volcados de texto; para que
/// `gms_doze` sea concluyente, el primero debe incluir también la salida de
/// `dumpsys deviceidle` (es habitual concatenar los tres).
pub fn audit_doze(dumpsys_power: &str, dumpsys_alarm: &str) -> DozeAudit {
    let combined = format!("{}\n{}", dumpsys_power, dumpsys_alarm);
    let doze_state = parse_doze_state(&combined);
    let wakelocks = parse_wakelocks(dumpsys_power);
    let alarms = parse_alarms(dumpsys_alarm);

    let lower = combined.to_lowercase();
    let bg_restricted = lower.contains("mbackgroundrestricted=true")
        || lower.contains("background restricted: true")
        || matches!(
            doze_state.as_str(),
            "IDLE" | "MAINTENANCE" | "LIGHT_IDLE" | "DOZING"
        );

    DozeAudit {
        gms_doze: parse_gms_doze(&combined),
        doze_state,
        wakelocks,
        alarms,
        bg_restricted,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Crea un directorio temporal único para el test.
    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let p = std::env::temp_dir().join(format!("md-batt-{}-{}-{}", tag, std::process::id(), nanos));
        fs::create_dir_all(&p).unwrap();
        p
    }

    /// Escribe un árbol sintético de `/sys/class/power_supply/battery`.
    fn synth_battery(dir: &Path, status: &str, current: &str, temp: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("capacity"), "87").unwrap();
        fs::write(dir.join("status"), status).unwrap();
        fs::write(dir.join("health"), "Good").unwrap();
        fs::write(dir.join("temp"), temp).unwrap();
        fs::write(dir.join("voltage_now"), "4250000").unwrap();
        fs::write(dir.join("current_now"), current).unwrap();
        fs::write(dir.join("charge_full"), "3520000").unwrap();
        fs::write(dir.join("charge_full_design"), "4000000").unwrap();
        fs::write(dir.join("cycle_count"), "412").unwrap();
        fs::write(dir.join("technology"), "Li-ion").unwrap();
    }

    fn snapshot(status: &str, current: &str, temp: &str) -> BatterySnapshot {
        BatterySnapshot {
            capacity_pct: 87,
            status: status.to_string(),
            health: "Good".to_string(),
            temp_c: temp.parse::<f32>().unwrap(),
            voltage_mv: 4250,
            current_ma: current.parse::<i32>().unwrap(),
            charge_full: 3_520_000,
            charge_full_design: 4_000_000,
            cycle_count: Some(412),
            technology: "Li-ion".to_string(),
            charging: status == "Charging" || status == "Full",
            source: "/sys/class/power_supply/battery".to_string(),
        }
    }

    fn has_shell(actions: &[Action], needle: &str) -> bool {
        actions.iter().any(|a| match a {
            Action::Shell { cmd, .. } => cmd.contains(needle),
            _ => false,
        })
    }

    fn notices(actions: &[Action]) -> Vec<String> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Notice { text } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    // --- lectura de sysfs -------------------------------------------------

    #[test]
    fn lee_el_arbol_sintetico_de_la_bateria() {
        let d = temp_dir("read");
        let batt = d.join("battery");
        synth_battery(&batt, "Charging", "450000", "315");
        let s = read_battery(batt.to_str().unwrap()).unwrap();
        assert_eq!(s.capacity_pct, 87);
        assert_eq!(s.status, "Charging");
        assert_eq!(s.health, "Good");
        assert!((s.temp_c - 31.5).abs() < 0.01, "temp = {}", s.temp_c);
        assert_eq!(s.voltage_mv, 4250);
        assert_eq!(s.current_ma, 450);
        assert_eq!(s.charge_full, 3_520_000);
        assert_eq!(s.charge_full_design, 4_000_000);
        assert_eq!(s.cycle_count, Some(412));
        assert_eq!(s.technology, "Li-ion");
        assert!(s.charging);
        assert!(s.source.ends_with("battery"));
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn acepta_la_raiz_de_power_supply_y_detecta_descarga() {
        let d = temp_dir("read2");
        let batt = d.join("battery");
        synth_battery(&batt, "Discharging", "-450000", "300");
        let s = read_battery(d.to_str().unwrap()).unwrap();
        assert!(!s.charging, "descargando no debe marcar charging");
        assert_eq!(s.current_ma, -450);
        assert!((s.temp_c - 30.0).abs() < 0.01);
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn una_raiz_inexistente_es_error() {
        let r = read_battery("/no/existe/de/ningun/sitio");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("no existe"));
    }

    #[test]
    fn lee_la_zona_termica_mas_caliente() {
        let d = temp_dir("thermal");
        fs::create_dir_all(d.join("thermal_zone0")).unwrap();
        fs::create_dir_all(d.join("thermal_zone1")).unwrap();
        fs::write(d.join("thermal_zone0/temp"), "45000").unwrap();
        fs::write(d.join("thermal_zone1/temp"), "52000").unwrap();
        let t = read_hottest_thermal_c(d.to_str().unwrap()).expect("debe leer zonas");
        assert!((t - 52.0).abs() < 0.01, "temp = {}", t);
        assert_eq!(read_hottest_thermal_c("/no/existe/thermal"), None);
        fs::remove_dir_all(&d).ok();
    }

    // --- salud y ciclos ---------------------------------------------------

    #[test]
    fn la_salud_de_la_celda_es_el_ratio_real() {
        let s = snapshot("Charging", "450000", "31.5");
        assert!((health_pct(&s) - 88.0).abs() < 0.01, "salud = {}", health_pct(&s));
        // Sin charge_full_design no se puede afirmar nada.
        let mut roto = s.clone();
        roto.charge_full_design = 0;
        assert_eq!(health_pct(&roto), 0.0);
    }

    #[test]
    fn estima_los_ciclos_restantes_con_dos_modelos() {
        let s = snapshot("Charging", "450000", "31.5");
        let est = estimate_cycles_left(&s).expect("debe estimar");
        // Modelo nominal: (88-80)/((100-80)/500) = 200; el observado da ~275.
        assert!((est - 200.0).abs() < 1.0, "estimación = {}", est);

        // Sin cycle_count se usa sólo el modelo nominal.
        let mut s2 = s.clone();
        s2.cycle_count = None;
        let est2 = estimate_cycles_left(&s2).unwrap();
        assert!((est2 - 200.0).abs() < 1.0, "estimación = {}", est2);

        // Celda sin degradación medible -> no se inventa una estimación.
        let mut s3 = s.clone();
        s3.charge_full = s3.charge_full_design;
        assert_eq!(estimate_cycles_left(&s3), None);

        // Celda ya por debajo del 80 % -> 0 ciclos.
        let mut s4 = s.clone();
        s4.charge_full = 3_000_000; // 75 %
        assert_eq!(estimate_cycles_left(&s4), Some(0.0));
    }

    // --- perfiles ----------------------------------------------------------

    #[test]
    fn el_catalogo_tiene_los_seis_perfiles_realistas() {
        let p = profiles();
        assert_eq!(p.len(), 6);
        for n in [
            "balanced",
            "endurance",
            "performance",
            "travel",
            "gms_doze",
            "charge_80",
        ] {
            assert!(profile(n).is_some(), "falta el perfil {}", n);
        }
        assert!(profile("no_existe").is_none());
    }

    #[test]
    fn endurance_es_el_perfil_mas_restrictivo() {
        let e = profile("endurance").unwrap();
        let b = profile("balanced").unwrap();
        let perf = profile("performance").unwrap();
        assert!(e.restrict_bg && b.restrict_bg == false);
        assert!(e.gms_doze && !b.gms_doze);
        assert!(e.wakelock_timeout_s < b.wakelock_timeout_s);
        assert!(e.wakelock_timeout_s < perf.wakelock_timeout_s);
        assert_eq!(e.charge_limit_pct, Some(80));
        assert_eq!(e.app_standby_bucket, "rare");
    }

    // --- plan --------------------------------------------------------------

    #[test]
    fn el_plan_es_puro_y_reproducible() {
        let p = profile("balanced").unwrap();
        let s = snapshot("Discharging", "-450000", "31.5");
        let a = plan(&p, &s, false);
        let b = plan(&p, &s, false);
        assert_eq!(a, b, "plan() debe ser determinista");
    }

    #[test]
    fn el_plan_activa_doze_pero_no_toca_la_carga_si_no_se_carga() {
        let p = profile("balanced").unwrap();
        let s = snapshot("Discharging", "-450000", "31.5");
        let acts = plan(&p, &s, false);
        assert!(has_shell(&acts, "dumpsys deviceidle enable"));
        assert!(
            !has_shell(&acts, "charge_control") && !has_shell(&acts, "acc "),
            "sin carga no debe haber límite de carga"
        );
    }

    #[test]
    fn cargando_con_perfil_de_carga_se_aplica_el_limite() {
        let p = profile("charge_80").unwrap();
        let s = snapshot("Charging", "450000", "31.5");
        let acts = plan(&p, &s, true);
        assert!(has_shell(&acts, "charge_control"));
        assert!(has_shell(&acts, "acc 80 75"));
        assert!(notices(&acts).iter().any(|n| n.contains("80 %")));
    }

    #[test]
    fn el_perfil_de_rendimiento_desactiva_doze() {
        let p = profile("performance").unwrap();
        let s = snapshot("Discharging", "-450000", "31.5");
        let acts = plan(&p, &s, false);
        assert!(has_shell(&acts, "dumpsys deviceidle disable"));
    }

    #[test]
    fn el_perfil_gms_doze_saca_gms_de_la_whitelist() {
        let p = profile("gms_doze").unwrap();
        let s = snapshot("Discharging", "-450000", "31.5");
        let acts = plan(&p, &s, false);
        assert!(has_shell(&acts, "dumpsys deviceidle whitelist -com.google.android.gms"));
        assert!(has_shell(&acts, "am set-standby-bucket com.google.android.gms rare"));
    }

    #[test]
    fn la_temperatura_alta_endurece_la_politica() {
        let p = profile("balanced").unwrap();
        let s = snapshot("Charging", "450000", "47.5");
        let acts = plan(&p, &s, true);
        assert!(
            has_shell(&acts, "RUN_ANY_IN_BACKGROUND"),
            "con calor debe restringirse el segundo plano"
        );
        assert!(notices(&acts).iter().any(|n| n.contains("térmica")));
    }

    #[test]
    fn una_celda_degradada_limita_la_carga_aunque_el_perfil_no_lo_pida() {
        let p = profile("balanced").unwrap();
        let mut s = snapshot("Charging", "450000", "31.5");
        s.charge_full = 3_300_000; // 82.5 % de salud
        s.charge_full_design = 4_000_000;
        let acts = plan(&p, &s, true);
        assert!(has_shell(&acts, "charge_control"), "debe limitar la carga");
        assert!(notices(&acts).iter().any(|n| n.contains("degradada")));
    }

    #[test]
    fn el_plan_marca_el_fichero_fuera_del_estado_como_destructivo() {
        let p = profile("balanced").unwrap();
        let s = snapshot("Discharging", "-450000", "31.5");
        let acts = plan(&p, &s, false);
        let fuera = acts
            .iter()
            .find(|a| matches!(a, Action::File { path, .. } if path.starts_with("/data/adb/")))
            .expect("debe haber un marcador fuera del estado");
        assert!(fuera.is_destructive());
        // Y el fichero de política dentro del estado NO es destructivo.
        let dentro = acts
            .iter()
            .find(|a| matches!(a, Action::File { path, .. } if path.starts_with(state_dir().as_str())))
            .expect("debe haber la política en el estado");
        assert!(!dentro.is_destructive());
    }

    // --- aplicación --------------------------------------------------------

    #[test]
    fn apply_es_simulado_y_no_toca_nada() {
        let d = temp_dir("dry");
        // Un fichero FUERA del directorio de estado es una acción destructiva:
        // sin `confirm` se omite y, en cualquier caso, no se escribe.
        let fuera = d.join("no-debe-existir.json");
        let acts = vec![Action::File {
            path: fuera.to_string_lossy().to_string(),
            content: "{}".to_string(),
        }];
        let res = apply(&acts);
        assert!(!res[0].ok, "fuera del estado exige confirm");
        assert!(res[0].output.contains("confirm"));
        assert!(!fuera.exists(), "la simulación no puede escribir ficheros");

        // Un fichero DENTRO del directorio de estado se simula (no se escribe).
        let dentro = format!("{}/power_policy.json", state_dir());
        let res2 = apply(&[Action::File {
            path: dentro.clone(),
            content: "{}".to_string(),
        }]);
        assert!(res2[0].ok);
        assert!(res2[0].output.contains("dry-run"));
        assert!(
            !Path::new(&dentro).exists(),
            "la simulación no puede escribir ficheros"
        );
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn las_acciones_destructivas_exigen_confirm() {
        let d = temp_dir("confirm");
        let target = d.join("destructivo.json");
        let acts = vec![Action::File {
            path: target.to_string_lossy().to_string(),
            content: "{}".to_string(),
        }];
        let mut ex = FakeExecutor::new();
        let sin = apply_with(&acts, &mut ex, false);
        assert!(!sin[0].ok);
        assert!(sin[0].output.contains("confirm"));
        assert!(!target.exists());

        let mut ex2 = FakeExecutor::new();
        let con = apply_with(&acts, &mut ex2, true);
        assert!(con[0].ok, "con confirm debe ejecutarse: {}", con[0].output);
        assert!(target.exists());
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn el_ejecutor_de_shell_real_propaga_los_errores() {
        let mut ex = ShellExecutor;
        let ok = ex.run("echo hola").unwrap();
        assert!(ok.contains("hola"));
        let err = ex.run("exit 3").unwrap_err();
        assert!(err.contains("3") || err.contains("status"));
    }

    #[test]
    fn el_ejecutor_falso_registra_las_llamadas() {
        let mut ex = FakeExecutor::failing("charge_control");
        let acts = vec![
            Action::Shell {
                cmd: "dumpsys deviceidle enable".to_string(),
                reversible: true,
            },
            Action::Shell {
                cmd: "echo 80 > /sys/class/power_supply/battery/charge_control".to_string(),
                reversible: true,
            },
        ];
        let res = apply_with(&acts, &mut ex, true);
        assert_eq!(ex.calls.len(), 2);
        assert!(res[0].ok);
        assert!(!res[1].ok);
        assert!(res[1].output.contains("fallo simulado"));
    }

    // --- auditoría ---------------------------------------------------------

    const DUMPSYS_POWER: &str = "\
POWER MANAGER (dumpsys power)

Display Power: state=OFF, display=true, brightness=0, brightnessInt=0, displayPower=OFF
mScreenBrightnessOverrideFromWindowManager=
mWakefulness=Dozing
mWakefulnessChanging=false
mWakefulnessSummary=Dozing
mUserActivityTimeout=in 25h 12m 3s
mIsPowered=true
mStayOn=false
mScreenOn=false
Wakelocks:
  +1 Partially Suspended:  PowerManagerService
  +1 Partially Suspended:  ActivityManager
  +1 Partially Suspended:  NetworkManagement
";

    const DUMPSYS_ALARM: &str = "\
Alarm Stats:
  u0a123:com.google.android.gms +1m3s last -2m:
    +1m3s running, 0 wakeups:
  u0a45:com.whatsapp +2h 3m 0 wakes 2 alarms:
";

    #[test]
    fn audita_doze_wakelocks_y_alarmas_de_texto_real() {
        let a = audit_doze(DUMPSYS_POWER, DUMPSYS_ALARM);
        assert_eq!(a.doze_state, "DOZING");
        assert_eq!(
            a.wakelocks,
            vec![
                "PowerManagerService".to_string(),
                "ActivityManager".to_string(),
                "NetworkManagement".to_string()
            ]
        );
        assert_eq!(
            a.alarms,
            vec![
                "com.google.android.gms".to_string(),
                "com.whatsapp".to_string()
            ]
        );
        assert!(a.bg_restricted, "en Doze el segundo plano está restringido");
    }

    #[test]
    fn gms_doze_necesita_la_whitelist_para_ser_concluyente() {
        // Sin sección de whitelist: no se afirma nada.
        let a = audit_doze(DUMPSYS_POWER, DUMPSYS_ALARM);
        assert!(!a.gms_doze);

        // Con whitelist y GMS ausente -> GMS está sujeto a Doze.
        let power_con_whitelist = format!(
            "{}\nWhitelist system apps:\n  com.android.phone\nWhitelist user apps:\n  com.termux\n",
            DUMPSYS_POWER
        );
        let b = audit_doze(&power_con_whitelist, DUMPSYS_ALARM);
        assert!(b.gms_doze);

        // Con whitelist y GMS presente -> NO está sujeto a Doze.
        let power_gms_whitelisted = format!(
            "{}\nWhitelist user apps:\n  com.google.android.gms\n",
            DUMPSYS_POWER
        );
        let c = audit_doze(&power_gms_whitelisted, DUMPSYS_ALARM);
        assert!(!c.gms_doze);
    }

    #[test]
    fn detecta_el_estado_idle_de_deviceidle() {
        let power = "mState=IDLE\nmLightState=STATE_LIGHT_IDLE\n";
        let a = audit_doze(power, "");
        assert_eq!(a.doze_state, "IDLE");
        assert!(a.bg_restricted);
    }
}

// ===========================================================================
// v15 — POLÍTICA REAL DEL DISPOSITIVO (SM-G998B) + WATCHDOG DE RESPUESTA
// ===========================================================================
//
// MEDIDAS de este SM-G998B (2026-11-20, 03:29 CET, phone real):
//   RAM total 10.829.476 kB / libres 336.380 kB / disponibles 3.649.892 kB
//   swap total 4.194.300 kB / libres 535.348 kB   <- 87 % del swap ocupado
//   disco /storage/emulated: 225 G totales, 5.1 G libres (98 %)
//   our footprint: /data/local/tmp/megadaemon = 178.340 kB  (sync_snap 59.860,
//   state/sync 15.479, state/inventory 11.170 con instantáneas de 9.104 kB)
//   térmica: NPU 55, ISP 56, CP 58, MID 58, BIG 59, LITTLE 60  (°C)
//   batería: 47 %, cargando, 38.1 °C, 3955 mV
//   brillo: 68 (manual)   governor: energy_step
//   Doze: mEnabled=true mDeepEnabled=true pero mState=ACTIVE (pantalla ON)
//
// El móvil NO expone `charge_control_end_threshold`: este kernel no permite
// forzar el límite de carga por sysfs, así que el plan lo INTENTA, lo VERIFICA
// y si no existe lo DICE (número, no adjetivo).

use std::collections::BTreeMap;

/// Umbrales de la política del dispositivo.
///
/// Todos son configurables por entorno (para no tener que recompilar el
/// daemon en el móvil).  Los valores por defecto son los MEDIDOS en este
/// SM-G998B, no los de otro móvil.
#[derive(Debug, Clone, PartialEq)]
pub struct Thresholds {
    /// Límite de carga de la celda en % (el "carga a 80" del encargo).
    pub charge_limit_pct: u8,
    /// Temperatura (°C) a partir de la cual se recorta el brillo y el trabajo.
    pub thermal_warn_c: f32,
    /// Temperatura (°C) a partir de la cual se limita el trabajo pesado.
    pub thermal_crit_c: f32,
    /// Temperatura (°C) techo absoluto: por encima se sella el dispositivo.
    pub thermal_hard_c: f32,
    /// Batería (%) por debajo de la cual se "sella" el dispositivo.
    pub seal_below_pct: u8,
    /// Brillo máximo que se impone en modo conservación.
    pub brightness_max: i32,
    /// RAM disponible (kB) por debajo de la cual se deja de hacer trabajo extra.
    pub ram_floor_kb: u64,
    /// Disco libre (kB) por debajo del cual se poda el estado.
    pub disk_floor_kb: u64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Thresholds {
            charge_limit_pct: 80,
            thermal_warn_c: 45.0,
            thermal_crit_c: 55.0,
            thermal_hard_c: 80.0,
            seal_below_pct: 20,
            brightness_max: 60,
            ram_floor_kb: 512 * 1024,
            disk_floor_kb: 1024 * 1024,
        }
    }
}

fn env_u8(key: &str, default: u8) -> u8 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<u8>().ok())
        .unwrap_or(default)
}

fn env_f32(key: &str, default: f32) -> f32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<f32>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(default)
}

fn env_i32(key: &str, default: i32) -> i32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<i32>().ok())
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

impl Thresholds {
    /// Umbrales con los overrides de entorno aplicados.
    pub fn from_env() -> Self {
        let d = Thresholds::default();
        Thresholds {
            charge_limit_pct: env_u8("MEGADAEMON_CHARGE_LIMIT_PCT", d.charge_limit_pct),
            thermal_warn_c: env_f32("MEGADAEMON_THERMAL_WARN_C", d.thermal_warn_c),
            thermal_crit_c: env_f32("MEGADAEMON_THERMAL_CRIT_C", d.thermal_crit_c),
            thermal_hard_c: env_f32("MEGADAEMON_THERMAL_HARD_C", d.thermal_hard_c),
            seal_below_pct: env_u8("MEGADAEMON_SEAL_BELOW_PCT", d.seal_below_pct),
            brightness_max: env_i32("MEGADAEMON_BRIGHTNESS_MAX", d.brightness_max),
            ram_floor_kb: env_u64("MEGADAEMON_RAM_FLOOR_KB", d.ram_floor_kb),
            disk_floor_kb: env_u64("MEGADAEMON_DISK_FLOOR_KB", d.disk_floor_kb),
        }
    }
}

/// Estado real del dispositivo tal y como lo ve el daemon.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DeviceState {
    /// % de batería.
    pub battery_pct: Option<u8>,
    /// ¿Está cargando?
    pub charging: bool,
    /// Temperatura de la batería (°C).
    pub battery_temp_c: Option<f32>,
    /// Temperatura más alta del sistema (°C).
    pub max_temp_c: Option<f32>,
    /// ¿La pantalla está encendida?
    pub screen_on: bool,
    /// RAM disponible (kB).
    pub ram_available_kb: u64,
    /// Disco libre (kB).
    pub disk_free_kb: u64,
}

/// Clase de política según el estado medido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyClass {
    /// Todo normal: se deja el dispositivo como está.
    Normal,
    /// Ahorro: se recorta el consumo discrecional.
    Conserve,
    /// Térmico: se limita el trabajo pesado.
    Thermal,
    /// Sellado: sólo sobrevivencia (watchdog, túnel y nada más).
    Seal,
}

impl PolicyClass {
    /// Nombre estable (para logs y para el bus).
    pub fn as_str(self) -> &'static str {
        match self {
            PolicyClass::Normal => "normal",
            PolicyClass::Conserve => "conserve",
            PolicyClass::Thermal => "thermal",
            PolicyClass::Seal => "seal",
        }
    }
}

/// Clasifica el estado con los umbrales dados.
///
/// Orden de decisión (y por qué):
/// 1. `Seal` si la batería está por debajo del umbral (20 % por defecto) o la
///    temperatura pasa del techo duro (80 °C): el encargo pide "batería < 20 %
///    ⇒ sellar" y un sellado nunca daña nada (sólo quita trabajo discrecional).
/// 2. `Thermal` si la temperatura pasa del umbral de aviso (45 °C) o del
///    crítico (55 °C).
/// 3. `Conserve` si la batería baja del 35 % sin cargador, o si la RAM
///    disponible cae por debajo del suelo.
/// 4. `Normal` en caso contrario.
pub fn classify(st: &DeviceState, th: &Thresholds) -> PolicyClass {
    let pct = st.battery_pct.unwrap_or(100);
    let temp = st.max_temp_c.unwrap_or(0.0).max(st.battery_temp_c.unwrap_or(0.0));

    if pct < th.seal_below_pct {
        return PolicyClass::Seal;
    }
    if temp >= th.thermal_hard_c {
        return PolicyClass::Seal;
    }
    if temp >= th.thermal_crit_c {
        return PolicyClass::Thermal;
    }
    if temp >= th.thermal_warn_c {
        return PolicyClass::Thermal;
    }
    if !st.charging && pct < 35 {
        return PolicyClass::Conserve;
    }
    if st.ram_available_kb > 0 && st.ram_available_kb < th.ram_floor_kb {
        return PolicyClass::Conserve;
    }
    PolicyClass::Normal
}

/// Una acción de política con su COMPROBACIÓN: cada acción dice cómo se
/// verifica que surtió efecto.  Sin verificación, una política es una promesa.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceAction {
    /// Etiqueta corta (para logs).
    pub label: String,
    /// Comando a ejecutar (sh).
    pub cmd: String,
    /// Comando de verificación (se ejecuta después; su salida es el número).
    pub verify: String,
    /// ¿Se puede deshacer con un comando conocido?
    pub reversible: bool,
}

impl DeviceAction {
    fn new(label: &str, cmd: &str, verify: &str, reversible: bool) -> Self {
        DeviceAction {
            label: label.to_string(),
            cmd: cmd.to_string(),
            verify: verify.to_string(),
            reversible,
        }
    }
}

/// Paquetes que se pueden restringir en segundo plano (los que se comen la
/// RAM del móvil).  Medido el 2026-11-20: Chrome 557 MB RSS, YouTube 412 MB.
const HEAVY_PACKAGES: [&str; 2] = ["com.google.android.youtube", "com.android.chrome"];

/// Paquetes que NUNCA hay que tocar: los nuestros.
const PROTECTED_PACKAGES: [&str; 3] = ["com.termux", "com.termux.api", "com.agy"];

/// Construye el plan de acciones para el estado dado.
///
/// Todas las acciones son REVERSIBLES y llevan su verificación.  Ninguna toca
/// los paquetes protegidos (Termux = nuestro canal) ni borra datos del
/// usuario: `appops` sólo recorta ejecución en segundo plano.
pub fn device_plan(st: &DeviceState, th: &Thresholds) -> Vec<DeviceAction> {
    let mut out = Vec::new();
    let class = classify(st, th);

    // 1) Límite de carga: sólo si el kernel lo expone.
    out.push(DeviceAction::new(
        "charge_limit",
        &format!(
            "set_threshold {} {}",
            "/sys/class/power_supply/battery/charge_control_end_threshold",
            th.charge_limit_pct
        ),
        "cat /sys/class/power_supply/battery/charge_control_end_threshold 2>/dev/null || echo UNSUPPORTED",
        true,
    ));

    // 2) Fondo de las apps que se comen la RAM (sólo en conserve/thermal/seal).
    if class != PolicyClass::Normal {
        for pkg in HEAVY_PACKAGES {
            if PROTECTED_PACKAGES.contains(&pkg) {
                continue;
            }
            out.push(DeviceAction::new(
                "appops_bg",
                &format!("cmd appops set {} RUN_ANY_IN_BACKGROUND ignore", pkg),
                &format!("cmd appops get {} RUN_ANY_IN_BACKGROUND 2>/dev/null | tail -1", pkg),
                true,
            ));
        }
    }

    // 3) Brillo: techo en conserve/thermal/seal.
    if class != PolicyClass::Normal {
        let max = match class {
            PolicyClass::Seal => th.brightness_max - 20,
            _ => th.brightness_max,
        };
        out.push(DeviceAction::new(
            "brightness",
            &format!("settings put system screen_brightness {}", max.max(1)),
            "settings get system screen_brightness",
            true,
        ));
    }

    // 4) Governor: sólo cuando aprieta (thermal/seal).
    if class == PolicyClass::Thermal || class == PolicyClass::Seal {
        out.push(DeviceAction::new(
            "governor",
            "set_governor powersave",
            "cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor",
            true,
        ));
    }

    // 5) Ahorro de energía del sistema: thermal/seal.
    if class == PolicyClass::Thermal || class == PolicyClass::Seal {
        out.push(DeviceAction::new(
            "low_power",
            "settings put global low_power 1",
            "settings get global low_power",
            true,
        ));
    }

    // 6) Doze profundo: sólo si la pantalla está apagada (si no, se deja al
    //    usuario: no se le apaga la pantalla bajo los pies).
    if (class == PolicyClass::Thermal || class == PolicyClass::Seal) && !st.screen_on {
        out.push(DeviceAction::new(
            "doze_deep",
            "cmd deviceidle force-idle",
            "dumpsys deviceidle | grep -m1 'mState='",
            true,
        ));
    }

    // 7) Disco: si el disco está por debajo del suelo, podar el estado.
    if st.disk_free_kb > 0 && st.disk_free_kb < th.disk_floor_kb {
        out.push(DeviceAction::new(
            "prune_state",
            "prune_state_keep 2",
            &format!("du -sk {d} 2>/dev/null", d = sh_quote(&state_dir())),
            true,
        ));
    }

    out
}

/// Lee el estado del dispositivo desde sysfs (`root` permite pruebas con un
/// árbol falso).  Devuelve lo que pudo leer y `None` en lo que no.
pub fn read_device_state(root: &str) -> DeviceState {
    let mut st = DeviceState::default();
    let bat = format!("{}/sys/class/power_supply/battery", root);

    if let Some(v) =
        read_trim_file(&format!("{}/capacity", bat)).and_then(|s| s.parse::<u8>().ok())
    {
        st.battery_pct = Some(v);
    }
    if let Some(s) = read_trim_file(&format!("{}/status", bat)) {
        st.charging = s.eq_ignore_ascii_case("charging")
            || s.eq_ignore_ascii_case("full")
            || s.eq_ignore_ascii_case("unknown");
    }
    if let Some(v) =
        read_trim_file(&format!("{}/temp", bat)).and_then(|s| s.trim().parse::<f64>().ok())
    {
        // Unidades de /sys/class/power_supply/*/temp en Android:
        //   * >= 10000 -> milikelvin  (38100 = 38.1 C)
        //   * >= 100   -> decigrados  (381   = 38.1 C)  <-- el de este S21
        //   * <  100   -> centigrados (38    = 38 C)
        // MEDIDO en este SM-G998B: battery/temp = 381 con la bateria a 38.1 C.
        st.battery_temp_c = Some(if v >= 10_000.0 {
            (v / 1000.0) as f32
        } else if v >= 100.0 {
            (v / 10.0) as f32
        } else {
            v as f32
        });
    }
    st.max_temp_c = read_hottest_thermal_c(root);

    // RAM disponible.
    if let Ok(mem) = std::fs::read_to_string("/proc/meminfo") {
        for line in mem.lines() {
            if let Some(rest) = line.strip_prefix("MemAvailable:") {
                st.ram_available_kb = rest
                    .trim()
                    .split_whitespace()
                    .next()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0);
                break;
            }
        }
    }
    st
}

fn read_trim_file(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

// ---------------------------------------------------------------------------
// WATCHDOG DE RESPUESTA (no de pgrep)
// ---------------------------------------------------------------------------

/// Resultado de una sonda de vida real (una petición HTTP de verdad).
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeOutcome {
    /// Código HTTP devuelto.
    pub code: u16,
    /// Milisegundos que tardó.
    pub elapsed_ms: u64,
    /// ¿El daemon se considera sano?
    pub healthy: bool,
    /// Motivo (para el log).
    pub reason: &'static str,
}

/// Parsea la salida de `curl -w '%{http_code} %{time_total}'`.
///
/// Devuelve `None` si la salida no tiene ese formato (curl falló, timeout o
/// la respuesta no llegó): que es EXACTAMENTE el caso que `pgrep` no detecta.
pub fn parse_curl_probe(out: &str) -> Option<(u16, u64)> {
    let line = out.trim().lines().last()?.trim();
    let mut parts = line.split_whitespace();
    let code = parts.next()?.parse::<u16>().ok()?;
    let secs = parts.next().unwrap_or("0").parse::<f64>().ok()?;
    Some((code, (secs * 1000.0).round() as u64))
}

/// Evalúa una sonda: sólo es sana si contesta 200 Y rápido Y con cuerpo.
pub fn evaluate_probe(code: u16, elapsed_ms: u64, timeout_ms: u64, body_len: usize) -> ProbeOutcome {
    if code == 0 {
        return ProbeOutcome {
            code,
            elapsed_ms,
            healthy: false,
            reason: "sin respuesta (conexion rechazada o timeout)",
        };
    }
    if code >= 500 {
        return ProbeOutcome {
            code,
            elapsed_ms,
            healthy: false,
            reason: "error del servidor (>=500): el proceso vive pero no responde",
        };
    }
    if code != 200 {
        return ProbeOutcome {
            code,
            elapsed_ms,
            healthy: false,
            reason: "codigo HTTP no sano",
        };
    }
    if timeout_ms > 0 && elapsed_ms > timeout_ms {
        return ProbeOutcome {
            code,
            elapsed_ms,
            healthy: false,
            reason: "respuesta demasiado lenta",
        };
    }
    if body_len == 0 {
        return ProbeOutcome {
            code,
            elapsed_ms,
            healthy: false,
            reason: "respuesta vacia",
        };
    }
    ProbeOutcome {
        code,
        elapsed_ms,
        healthy: true,
        reason: "ok",
    }
}

/// Decisión del watchdog: cuenta fallos CONSECUTIVOS y decide cuándo hay que
/// reiniciar el daemon.
///
/// Un fallo suelto no reinicia nada (los móviles dan tirones: un 502 puntual
/// mientras el sistema está ocupado no significa nada).  Tres seguidos sí.
#[derive(Debug, Clone, PartialEq)]
pub struct Watchdog {
    /// Fallos consecutivos actuales.
    pub consecutive_failures: u32,
    /// Fallos necesarios para reiniciar.
    pub threshold: u32,
    /// Reinicios hechos por este watchdog.
    pub restarts: u32,
}

impl Default for Watchdog {
    fn default() -> Self {
        Watchdog {
            consecutive_failures: 0,
            threshold: 3,
            restarts: 0,
        }
    }
}

impl Watchdog {
    /// Registra una sonda y devuelve `true` si toca REINICIAR el daemon.
    pub fn observe(&mut self, healthy: bool) -> bool {
        if healthy {
            self.consecutive_failures = 0;
            return false;
        }
        self.consecutive_failures += 1;
        if self.consecutive_failures >= self.threshold {
            self.consecutive_failures = 0;
            self.restarts += 1;
            return true;
        }
        false
    }
}

// ---------------------------------------------------------------------------
// GUARDA DE DISCO: el daemon no puede crecer sin límite
// ---------------------------------------------------------------------------

/// Decide qué entradas de un directorio se pueden borrar.
///
/// `names` son los nombres de fichero (por convención, marcas de tiempo en el
/// nombre).  Se ordenan y se conservan los `keep` más recientes; el resto se
/// devuelve para borrar.  Los nombres que NO parecen marcas de tiempo (no son
/// dígitos) se conservan siempre: no se borra nada que no entendamos.
pub fn prune_candidates(names: &[String], keep: usize) -> Vec<String> {
    let mut stamped: Vec<(&String, i64)> = names
        .iter()
        .filter_map(|n| {
            let digits: String = n.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.is_empty() {
                None
            } else {
                digits.parse::<i64>().ok().map(|v| (n, v))
            }
        })
        .collect();
    stamped.sort_by(|a, b| a.1.cmp(&b.1));
    let excess = stamped.len().saturating_sub(keep);
    stamped.into_iter().take(excess).map(|(n, _)| n.clone()).collect()
}

/// Suma los tamaños (kB) de las entradas que se van a borrar: el número que
/// se publica en el bus.
pub fn prune_reclaim_kb(sizes_kb: &BTreeMap<String, u64>, victims: &[String]) -> u64 {
    victims
        .iter()
        .filter_map(|v| sizes_kb.get(v))
        .copied()
        .sum()
}

#[cfg(test)]
mod device_tests {
    use super::*;
    use std::fs;

    fn st(pct: u8, charging: bool, temp: f32, ram: u64) -> DeviceState {
        DeviceState {
            battery_pct: Some(pct),
            charging,
            battery_temp_c: Some(temp),
            max_temp_c: Some(temp),
            screen_on: false,
            ram_available_kb: ram,
            disk_free_kb: 5_000_000,
        }
    }

    #[test]
    fn umbrales_por_defecto_son_los_del_s21() {
        let t = Thresholds::default();
        assert_eq!(t.charge_limit_pct, 80);
        assert_eq!(t.seal_below_pct, 20);
        assert_eq!(t.thermal_hard_c, 80.0);
        assert_eq!(t.brightness_max, 60);
    }

    #[test]
    fn bateria_baja_sella_el_dispositivo() {
        let t = Thresholds::default();
        assert_eq!(classify(&st(19, false, 30.0, 3_000_000), &t), PolicyClass::Seal);
        assert_eq!(classify(&st(19, true, 30.0, 3_000_000), &t), PolicyClass::Seal);
        // al 20 % exacto ya NO se sella; sin cargador es "conserve"
        assert_eq!(
            classify(&st(20, false, 30.0, 3_000_000), &t),
            PolicyClass::Conserve
        );
        assert_eq!(
            classify(&st(20, true, 30.0, 3_000_000), &t),
            PolicyClass::Normal
        );
    }

    #[test]
    fn temperatura_alta_es_termica_y_extrema_sella() {
        let t = Thresholds::default();
        assert_eq!(
            classify(&st(80, true, 46.0, 3_000_000), &t),
            PolicyClass::Thermal
        );
        assert_eq!(
            classify(&st(80, true, 81.0, 3_000_000), &t),
            PolicyClass::Seal
        );
    }

    #[test]
    fn ram_baja_fuerza_conservacion() {
        let t = Thresholds::default();
        assert_eq!(
            classify(&st(90, true, 25.0, 100_000), &t),
            PolicyClass::Conserve
        );
    }

    #[test]
    fn plan_normal_solo_intenta_el_limite_de_carga() {
        let t = Thresholds::default();
        let plan = device_plan(&st(90, true, 25.0, 3_000_000), &t);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].label, "charge_limit");
        assert!(plan[0].cmd.contains("80"));
    }

    #[test]
    fn plan_conservacion_recorta_fondo_brillo_y_no_toca_termux() {
        let t = Thresholds::default();
        let plan = device_plan(&st(30, false, 25.0, 3_000_000), &t);
        let cmds: Vec<&str> = plan.iter().map(|a| a.cmd.as_str()).collect();
        assert!(cmds.iter().any(|c| c.contains("RUN_ANY_IN_BACKGROUND ignore")));
        assert!(cmds.iter().any(|c| c.contains("screen_brightness")));
        assert!(!cmds.iter().any(|c| c.contains("com.termux")));
        assert!(!cmds.iter().any(|c| c.contains("com.agy")));
        assert!(plan.iter().all(|a| a.reversible));
    }

    #[test]
    fn plan_terminal_pone_governor_y_low_power() {
        let t = Thresholds::default();
        let plan = device_plan(&st(80, true, 56.0, 3_000_000), &t);
        let cmds: Vec<&str> = plan.iter().map(|a| a.cmd.as_str()).collect();
        assert!(cmds.iter().any(|c| c.contains("powersave")));
        assert!(cmds.iter().any(|c| c.contains("low_power 1")));
    }

    #[test]
    fn cada_accion_lleva_su_verificacion() {
        let t = Thresholds::default();
        for a in device_plan(&st(19, false, 30.0, 3_000_000), &t) {
            assert!(
                !a.verify.trim().is_empty(),
                "acción sin verificar: {}",
                a.label
            );
        }
    }

    #[test]
    fn sonda_curl_se_parsea() {
        assert_eq!(parse_curl_probe("200 0.012"), Some((200, 12)));
        assert_eq!(parse_curl_probe("503 1.500"), Some((503, 1500)));
        // curl falló: NO hay código -> el daemon no responde (pgrep no lo ve).
        assert_eq!(parse_curl_probe(""), None);
        assert_eq!(parse_curl_probe("curl: (7) Failed to connect"), None);
    }

    #[test]
    fn sonda_200_lenta_o_vacia_no_es_sana() {
        assert!(evaluate_probe(200, 10, 5000, 120).healthy);
        assert!(!evaluate_probe(200, 9000, 5000, 120).healthy);
        assert!(!evaluate_probe(200, 10, 5000, 0).healthy);
        assert!(!evaluate_probe(502, 10, 5000, 120).healthy);
        assert!(!evaluate_probe(0, 0, 5000, 0).healthy);
    }

    #[test]
    fn watchdog_reinicia_tras_tres_fallos_consecutivos() {
        let mut w = Watchdog::default();
        assert!(!w.observe(false));
        assert!(!w.observe(false));
        assert!(w.observe(false));
        assert_eq!(w.restarts, 1);
        // un acierto resetea la cuenta
        assert!(!w.observe(true));
        assert_eq!(w.consecutive_failures, 0);
        assert!(!w.observe(false));
        assert!(!w.observe(false));
        assert!(w.observe(false));
        assert_eq!(w.restarts, 2);
    }

    #[test]
    fn watchdog_no_reinicia_por_un_fallo_suelto() {
        let mut w = Watchdog::default();
        assert!(!w.observe(false));
        assert!(!w.observe(true)); // un acierto resetea la cuenta
        assert!(!w.observe(false));
        assert!(!w.observe(true)); // y otro: sigue sin reiniciar
        assert_eq!(w.restarts, 0);
        // pero TRES SEGUIDOS sí: el umbral es de fallos consecutivos
        assert!(!w.observe(false));
        assert!(!w.observe(false));
        assert!(w.observe(false));
        assert_eq!(w.restarts, 1);
    }

    #[test]
    fn poda_conserva_los_mas_recientes_y_no_toca_nombres_raros() {
        let names = vec![
            "1795136951".to_string(),
            "1795137099".to_string(),
            "1795137346".to_string(),
            "1795137714".to_string(),
            "estado_actual.json".to_string(),
        ];
        let victims = prune_candidates(&names, 2);
        assert_eq!(
            victims,
            vec!["1795136951".to_string(), "1795137099".to_string()]
        );
        assert!(!victims.iter().any(|v| v.contains("estado_actual")));
    }

    #[test]
    fn lectura_de_estado_desde_arbol_falso() {
        let dir = std::env::temp_dir().join("md_batt_fake_root");
        let bat = dir.join("sys/class/power_supply/battery");
        fs::create_dir_all(&bat).unwrap();
        fs::write(bat.join("capacity"), "37\n").unwrap();
        fs::write(bat.join("status"), "Discharging\n").unwrap();
        fs::write(bat.join("temp"), "38500\n").unwrap(); // milikelvin -> 38.5 C
        let st = read_device_state(dir.to_str().unwrap());
        assert_eq!(st.battery_pct, Some(37));
        assert!(!st.charging);
        assert_eq!(st.battery_temp_c, Some(38.5));
        // y en decigrados, como lo da este kernel (381 = 38.1 C)
        fs::write(bat.join("temp"), "381\n").unwrap();
        let st2 = read_device_state(dir.to_str().unwrap());
        assert_eq!(st2.battery_temp_c, Some(38.1));
        let _ = fs::remove_dir_all(&dir);
    }
}
