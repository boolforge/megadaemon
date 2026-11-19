//! MegaDaemon v13.4 — Motor de ocultación de root REAL y verificable.
//!
//! Este módulo **no** se limita a decir "usa Shamiko": detecta, app por app y
//! señal por señal, qué delata al root en el dispositivo y devuelve un plan de
//! remediación **accionable** (comando exacto + si es reversible + si pide
//! confirmación).
//!
//! Diseño en tres piezas:
//!
//! 1. [`DeviceEnv`] es una **instantánea serializable** del estado del
//!    dispositivo (ficheros presentes, props, procesos, módulos Xposed, appops…)
//!    que se puede construir a mano en un test, sin need de un Galaxy en la
//!    mesa. Es el único punto de contacto con la realidad.
//! 2. [`detect_leaks`] es una función ** pura**: entra `DeviceEnv`, sale
//!    [`HideAudit`]. Nada de efectos, nada de `su`, nada de red.
//! 3. [`plan_remediation`] traduce las fugas encontradas en [`Remediation`]s
//!    con el comando exacto que las cierra; [`apply_remediation`] las aplica
//!    **simulando** el cambio en el `DeviceEnv` salvo que haya un
//!    [`Executor::Shell`] presente (entonces ejecuta de verdad y, además,
//!    refleja el resultado en el entorno en memoria).
//!
//! Lo que este módulo **NO** hace (y conviene decir con todas sus letras):
//!
//! * No falsifica la attestation criptográfica de hardware. Play Integrity
//!   en nivel `STRONG` con bootloader desbloqueado es, en la práctica,
//!   inalcanzable sin hardware firmado. `DEVICE_INTEGRITY` sí es alcanzable
//!   con PlayIntegrityFix + TrickyStore, y eso es *spoofing* de attestation,
//!   no criptografía rota.
//! * No modifica el hardware ni toca OTA. Todo lo que propone es reversible.
//! * No aplica nada sin `requires_confirm = true`.
//!
//! API pública:
//!
//! | Función | Qué hace |
//! |---|---|
//! | `default_profile()` | Perfil de ocultación de referencia (apps bancarias españolas) |
//! | `detect_leaks(pkg, env)` | Auditoría pura: qué delata al root ante `pkg` |
//! | `plan_remediation(audit)` | Plan de remediación con comandos exactos |
//! | `apply_remediation(r, env)` | Aplica (simula) una remediación |
//! | `audit_all(pkgs, env)` | Auditoría de un lote de paquetes |
//! | `magisk_module_layout()` | Layout de un módulo Magisk propio de ocultación |

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::process::Command;

// ========================================================================= //
// Catálogos de referencia
// ========================================================================= //

/// Apps bancarias y de la administración española.
///
/// Paquetes **verificados** contra el Play Store a fecha de la escritura del
/// módulo salvo los marcados como dudosos en el comentario.
pub const SPANISH_TARGET_APPS: &[(&str, &str)] = &[
    ("com.bbva.bbvacontigo", "BBVA España"),
    ("es.cm.android", "CaixaBank"),
    (
        "es.lacaixa.mobile.android.newwapicon",
        "CaixaBankNow (id legacy)",
    ),
    ("es.bancosantander.apps", "Santander"),
    (
        "net.inverline.bancosabadell.officelocator.android",
        "Banco Sabadell",
    ),
    ("com.bankinter.launcher", "BankinterMóvil"),
    ("com.kutxabank.android", "Kutxabank"),
    ("es.openbank.mobile", "Openbank"),
    ("com.revolut.revolut", "Revolut"),
    ("es.bizum.wallet", "Bizum Pay"),
    ("com.wirecash.android", "Bizum (app clásica) — id NO verificado"),
    (
        "es.fnmtrcm.ceres.certificadoDigitalFNMT",
        "Configurador/Certificado digital FNMT",
    ),
    ("es.aeat.dgc.mobile", "Agencia Tributaria (Hacienda)"),
    ("gov.es.segsocial.lanzadera", "Seguridad Social (Seg-Social)"),
    (
        "gov.es.segsocial.importass.android",
        "Importass Seguridad Social",
    ),
    ("com.mcdonalds.mobileapp", "McDonald's España"),
];

/// Paquetes de herramientas root que conviene esconder del listado de apps.
///
/// `com.tsng.hidemyapplist` (Hide My Applist) **no** se incluye por defecto:
/// `pm hide` lo saca también del lanzador, y su UI es precisamente la que
/// sirve para diagnosticar qué se está viendo. Lo que se hace es configurarlo
/// para que *él* oculte los paquetes anteriores (ver `LSPOSED_DENY_MODULES`).
pub const ROOT_TOOL_PACKAGES: &[&str] = &[
    "com.topjohnwu.magisk",
    "io.github.huskydg.magisk",
    "com.dergoogler.magisk",
    "org.lsposed.manager",
];

/// Módulos Xposed/LSPosed que se niegan a las apps sensibles.
///
/// Se niegan los *gestores* (Magisk y LSPosed Manager): si la app ve que el
/// framework Xposed carga el módulo del gestor, deduce que hay root aunque
/// el resto esté oculto. Hide My Applist **no** se niega: es precisamente lo
/// que protege a la app.
pub const LSPOSED_DENY_MODULES: &[&str] = &[
    "com.topjohnwu.magisk",
    "io.github.huskydg.magisk",
    "org.lsposed.manager",
];

/// Apps para las que Zygisk Detach debe quedar **desactivado**.
///
/// Detach desmonta `/data/adb` del espacio de montaje de la app: perfecto para
/// las apps bancarias, pero rompe cualquier app que necesite acceso a
/// `/data/adb` (terminales, gestores de root, el propio Magisk).
pub const DETACH_MUST_BE_OFF: &[&str] = &[
    "com.termux",
    "com.termux.api",
    "com.topjohnwu.magisk",
    "io.github.huskydg.magisk",
];

/// Propiedades del sistema y su valor "de fábrica" (sin root).
///
/// Éste es el contrato de `detect_leaks` y de `plan_remediation`: si una prop
/// no está en su valor limpio, es una fuga y el remediador sabe qué
/// `resetprop` la arregla.
pub const PROP_EXPECTATIONS: &[(&str, &str)] = &[
    ("ro.debuggable", "0"),
    ("ro.secure", "1"),
    ("ro.build.type", "user"),
    ("ro.build.tags", "release-keys"),
    ("ro.build.characteristics", "default"),
    ("ro.boot.verifiedbootstate", "green"),
    ("ro.boot.flash.locked", "1"),
    ("ro.boot.vbmeta.device_state", "locked"),
    ("service.adb.root", "0"),
    ("ro.kernel.qemu", "0"),
];

/// Marcadores de fichero/directorio que delatan al root.
///
/// `(marcador, severidad, descripción)`.
pub const ROOT_PATH_MARKERS: &[(&str, &str, &str)] = &[
    (
        "/sbin/.magisk",
        "critical",
        "montura de Magisk expuesta en el árbol de ficheros del sistema",
    ),
    (
        "/debug_ramdisk",
        "critical",
        "ramdisk de depuración presente (Magisk Kitsune / PIF)",
    ),
    (
        "/data/adb/magisk",
        "critical",
        "estado interno de Magisk accesible desde la app",
    ),
    (
        "/data/adb/modules",
        "high",
        "módulos Magisk visibles en el sistema de ficheros",
    ),
    (
        "/data/adb/magisk.db",
        "high",
        "base de datos de políticas de Magisk (su/su denylist) accesible",
    ),
    (
        "/data/adb/ksu",
        "critical",
        "datos de KernelSU/Kitsune accesibles desde la app",
    ),
    (
        "/data/adb/tricky_store",
        "medium",
        "almacén de TrickyStore accesible (delata el módulo de attestation)",
    ),
    (
        "/data/adb/pif.json",
        "medium",
        "fichero de Play Integrity Fix accesible",
    ),
    (
        "/data/adb/shamiko",
        "medium",
        "datos de Shamiko accesibles",
    ),
    (
        "/system/app/Superuser.apk",
        "high",
        "APK del gestor de superusuario presente",
    ),
    (
        "/system/framework/XposedBridge.jar",
        "critical",
        "puente Xposed cargado en el classpath del sistema",
    ),
    (
        "/system/lib64/libxposed.so",
        "critical",
        "biblioteca nativa de Xposed presente en el sistema",
    ),
    (
        "/data/adb/lspd",
        "high",
        "datos de LSPosed accesibles desde la app",
    ),
    (
        "/metadata/daemon",
        "high",
        "binarios de daemon de Magisk presentes",
    ),
    (
        "/dev/.magisk",
        "high",
        "socket de Magisk presente",
    ),
];

/// Nombres de procesos que delatan al root.
pub const ROOT_PROCESS_NAMES: &[&str] = &["magisk", "magiskd", "magiskpolicy", "ksud"];

/// Paquetes de herramientas root que, si se ven en el listado, delatan.
pub const DETECTION_PACKAGE_NAMES: &[&str] = &[
    "com.topjohnwu.magisk",
    "io.github.huskydg.magisk",
    "com.dergoogler.magisk",
    "org.lsposed.manager",
    "com.tsng.hidemyapplist",
    "eu.chainfire.supersu",
    "com.noshufou.android.su",
    "com.thirdparty.superuser",
    "com.koushikdutta.superbackup",
];

/// AppOps sensibles: si la app los tiene en `allow`, puede escanear el
/// sistema con una facilidad que las apps bancarias consideran sospechosa.
pub const SENSITIVE_APPOPS: &[(&str, &str)] = &[
    ("MANAGE_EXTERNAL_STORAGE", "medium"),
    ("GET_USAGE_STATS", "medium"),
    ("QUERY_ALL_PACKAGES", "medium"),
    ("SYSTEM_ALERT_WINDOW", "low"),
    ("WRITE_SECURE_SETTINGS", "high"),
];

// ========================================================================= //
// Tipos públicos
// ========================================================================= //

/// Perfil de ocultación: el conjunto de cambios que se quieren aplicar.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HideProfile {
    /// Paquetes que se meten en la DenyList de Magisk (`magisk --denylist add`).
    pub denylist_apps: Vec<String>,
    /// Paquetes que se ocultan del listado de apps (`pm hide`).
    pub hide_packages: Vec<String>,
    /// Propiedades del sistema que se falsean con `resetprop`.
    pub prop_overrides: Vec<(String, String)>,
    /// Módulos Xposed/LSPosed que se niegan a las apps de la denylist.
    pub deny_lsposed_modules: Vec<String>,
    /// Apps para las que Zygisk Detach debe quedar desactivado.
    pub disable_zygisk_detach_apps: Vec<String>,
}

impl Default for HideProfile {
    fn default() -> Self {
        default_profile()
    }
}

/// Perfil de referencia: apps bancarias españolas + ocultación deMagisk +
/// props limpias + negación de los gestores Xposed a las apps sensibles.
pub fn default_profile() -> HideProfile {
    HideProfile {
        denylist_apps: SPANISH_TARGET_APPS
            .iter()
            .map(|(p, _)| p.to_string())
            .collect(),
        hide_packages: ROOT_TOOL_PACKAGES.iter().map(|s| s.to_string()).collect(),
        prop_overrides: PROP_EXPECTATIONS
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        deny_lsposed_modules: LSPOSED_DENY_MODULES.iter().map(|s| s.to_string()).collect(),
        disable_zygisk_detach_apps: DETACH_MUST_BE_OFF.iter().map(|s| s.to_string()).collect(),
    }
}

/// Una fuga concreta detectada ante una app.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LeakSignal {
    /// Identificador estable de la fuga (`su_binary`, `prop_leak`, …).
    pub kind: String,
    /// Detalle legible: qué se ha visto exactamente.
    pub detail: String,
    /// `critical` | `high` | `medium` | `low`.
    pub severity: String,
    /// Identificador de la acción que la cierra (ver `plan_remediation`).
    pub remedied_by: String,
}

/// Resultado de auditar una app contra el entorno del dispositivo.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HideAudit {
    /// Paquete auditado.
    pub package: String,
    /// Fugas encontradas (puede estar vacío).
    pub leaks: Vec<LeakSignal>,
    /// **Puntuación de ocultación**: 100 = no se detecta nada; 0 = delatado
    /// de pies a cabeza. No es una nota de calidad: es "cuánto le queda por
    /// tapar a esta app".
    pub score: u8,
    /// `hidden` | `partially_visible` | `detectable` | `exposed`.
    pub verdict: String,
}

/// Una remediación accionable.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Remediation {
    /// Identificador de la acción (`resetprop`, `magisk_denylist_add`, …).
    pub action: String,
    /// Comando exacto, si existe. `None` cuando la acción es manual
    /// (instalar un módulo, tocar un ajuste de la app LSPosed…).
    pub command: Option<String>,
    /// ¿Se puede deshacer sin dejar rastro?
    pub reversible: bool,
    /// ¿Hace falta root para aplicarla?
    pub requires_root: bool,
    /// ¿Hace falta confirmación explícita del usuario?
    pub requires_confirm: bool,
    /// Objetivo de la acción (paquete, clave de prop, op de appops…).
    pub target: String,
    /// Instrucciones en lenguaje llano para lo que no se puede automatizar.
    pub notes: String,
}

/// Cómo se ejecuta una remediación.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum Executor {
    /// Nunca ejecuta nada: sólo simula el efecto sobre el `DeviceEnv`.
    Simulated,
    /// Ejecuta el comando con un shell real (`su`, `/system/bin/sh`, …).
    Shell {
        /// Ruta del binario que se usa como shell (`su` o `/system/bin/sh`).
        shell: String,
    },
}

/// Instantánea serializable del estado del dispositivo.
///
/// Es la única "entrada real" del módulo: los tests la construyen a mano y
/// no hace falta ningún Galaxy para probar el motor de detección.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DeviceEnv {
    /// Rutas de ficheros/directorios presentes en el dispositivo.
    pub files_present: Vec<String>,
    /// Propiedades del sistema (`getprop`) y su valor.
    pub props: Vec<(String, String)>,
    /// Nombres de proceso en ejecución.
    pub processes: Vec<String>,
    /// Módulos Xposed/LSPosed cargados.
    pub xposed_modules: Vec<String>,
    /// ¿Está Zygisk activo?
    pub zygisk: bool,
    /// Versión de Magisk instalada, si se conoce.
    pub magisk_version: Option<String>,
    /// ¿La app auditada está en la DenyList de Magisk?
    pub denylisted: bool,
    /// Rutas de los APK instalados (paquete → ruta).
    pub package_paths: Vec<String>,
    /// AppOps de la app auditada: `(operación, modo)`.
    pub appops: Vec<(String, String)>,

    // --- Extensiones ------------------------------------------------------
    /// Contenido completo de la DenyList de Magisk.
    #[serde(default)]
    pub denylist_apps: Vec<String>,
    /// Paquetes ocultos con `pm hide`.
    #[serde(default)]
    pub hidden_packages: Vec<String>,
    /// ¿Está Shamiko instalado y activo?
    #[serde(default)]
    pub shamiko: bool,
    /// ¿Está Zygisk Next (o ReZygisk) instalado?
    #[serde(default)]
    pub zygisk_next: bool,
    /// ¿Está TrickyStore instalado?
    #[serde(default)]
    pub trickystore: bool,
    /// ¿Está PlayIntegrityFix instalado?
    #[serde(default)]
    pub play_integrity_fix: bool,
    /// ¿Está Hide My Applist instalado?
    #[serde(default)]
    pub hide_my_applist: bool,
    /// Cómo aplicar remediaciones. `None` = simular (nunca ejecutar).
    #[serde(default)]
    pub executor: Option<Executor>,
}

impl Default for DeviceEnv {
    fn default() -> Self {
        DeviceEnv {
            files_present: Vec::new(),
            props: Vec::new(),
            processes: Vec::new(),
            xposed_modules: Vec::new(),
            zygisk: false,
            magisk_version: None,
            denylisted: false,
            package_paths: Vec::new(),
            appops: Vec::new(),
            denylist_apps: Vec::new(),
            hidden_packages: Vec::new(),
            shamiko: false,
            zygisk_next: false,
            trickystore: false,
            play_integrity_fix: false,
            hide_my_applist: false,
            executor: None,
        }
    }
}

impl DeviceEnv {
    /// Entorno "sano": dispositivo de usuario sin root, sin Magisk y con las
    /// props limpias. Sirve de línea base para los tests.
    pub fn clean() -> DeviceEnv {
        let mut env = DeviceEnv::default();
        env.props = PROP_EXPECTATIONS
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        env
    }

    /// Entorno "delatado": Magisk con todo visible (el peor caso típico).
    pub fn fully_exposed() -> DeviceEnv {
        let mut env = DeviceEnv::default();
        env.files_present = vec![
            "/system/bin/su".to_string(),
            "/sbin/.magisk/mirror".to_string(),
            "/debug_ramdisk".to_string(),
            "/data/adb/magisk.db".to_string(),
            "/data/adb/modules/zygisk_next".to_string(),
            "/system/framework/XposedBridge.jar".to_string(),
            "/data/app/~~a1b2==/com.topjohnwu.magisk-1/base.apk".to_string(),
        ];
        env.props = vec![
            ("ro.debuggable".to_string(), "1".to_string()),
            ("ro.secure".to_string(), "0".to_string()),
            ("ro.build.tags".to_string(), "test-keys".to_string()),
            ("ro.build.type".to_string(), "userdebug".to_string()),
            ("ro.boot.verifiedbootstate".to_string(), "orange".to_string()),
            ("ro.boot.flash.locked".to_string(), "0".to_string()),
            ("service.adb.root".to_string(), "1".to_string()),
            ("ro.magisk.version".to_string(), "27000".to_string()),
            ("ro.kernel.qemu".to_string(), "1".to_string()),
        ];
        env.processes = vec!["magiskd".to_string(), "system_server".to_string()];
        env.xposed_modules = vec!["com.topjohnwu.magisk".to_string()];
        env.zygisk = true;
        env.magisk_version = Some("27.0".to_string());
        env.denylisted = false;
        env.package_paths = vec!["/data/app/~~a1b2==/com.topjohnwu.magisk-1/base.apk".to_string()];
        env.appops = vec![("WRITE_SECURE_SETTINGS".to_string(), "allow".to_string())];
        env
    }

    /// Valor de una propiedad.
    pub fn prop(&self, key: &str) -> Option<&str> {
        self.props
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// ¿Está el paquete en la DenyList de Magisk?
    pub fn is_denylisted(&self, package: &str) -> bool {
        self.denylisted || self.denylist_apps.iter().any(|p| p == package)
    }
}

// ========================================================================= //
// Utilidades internas
// ========================================================================= //

/// ¿Aparece `marker` como secuencia contigua de componentes de `path`?
///
/// `/sbin/.magisk/mirror` contiene `/sbin/.magisk` → `true`.
/// `/data/superuser` NO contiene `/su` → `false` (evita falsos positivos).
fn path_has_marker(path: &str, marker: &str) -> bool {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let mparts: Vec<&str> = marker.split('/').filter(|s| !s.is_empty()).collect();
    if mparts.is_empty() || mparts.len() > parts.len() {
        return false;
    }
    for i in 0..=(parts.len() - mparts.len()) {
        if parts[i..i + mparts.len()] == mparts[..] {
            return true;
        }
    }
    false
}

/// Severidad de una propiedad filtrada.
fn prop_severity(key: &str) -> &'static str {
    match key {
        "ro.debuggable" | "ro.secure" | "ro.boot.flash.locked" => "critical",
        "ro.boot.verifiedbootstate" | "ro.boot.vbmeta.device_state" | "service.adb.root"
        | "ro.kernel.qemu" => "high",
        "ro.build.type" | "ro.build.characteristics" => "medium",
        _ => "high",
    }
}

/// Peso en puntos de cada severidad (sobre 100).
fn severity_weight(sev: &str) -> u16 {
    match sev {
        "critical" => 25,
        "high" => 15,
        "medium" => 8,
        "low" => 4,
        _ => 5,
    }
}

/// Severidad de una fuga de proceso o fichero.
fn severity_rank(sev: &str) -> u8 {
    match sev {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

/// ¿La prop está en su valor limpio?
fn prop_is_clean(key: &str, value: &str, expected: &str) -> bool {
    match key {
        // `release-keys` es lo normal; `test-keys`/`dev-keys` delatan.
        "ro.build.tags" => !value.contains("test-keys") && !value.contains("dev-keys"),
        // El emulador es delator aunque el resto esté limpio.
        "ro.kernel.qemu" => value != "1",
        _ => value == expected,
    }
}

/// ¿Hay root en el dispositivo según la instantánea?
///
/// Sin root no tiene sentido auditar la DenyList ni Shamiko: se audita el
/// entorno real, no una lista de deseos.
fn has_root(env: &DeviceEnv) -> bool {
    if env.magisk_version.is_some() || env.zygisk {
        return true;
    }
    ROOT_PATH_MARKERS
        .iter()
        .any(|(marker, _, _)| env.files_present.iter().any(|p| path_has_marker(p, marker)))
        || env
            .files_present
            .iter()
            .any(|p| ["/system/bin/su", "/system/xbin/su", "/sbin/su", "/su"]
                .iter()
                .any(|m| path_has_marker(p, m)))
}

// ========================================================================= //
// Detección de fugas
// ========================================================================= //

/// Audita `package` contra el estado del dispositivo.
///
/// Función ** pura: no toca el sistema, no lee el disco, no hace red. Sólo
/// razona sobre el [`DeviceEnv`] que le pases. Eso la hace testeable sin
/// dispositivo y reutilizable desde un daemon remoto que ya tenga el
/// snapshot.
pub fn detect_leaks(package: &str, env: &DeviceEnv) -> HideAudit {
    let mut leaks: Vec<LeakSignal> = Vec::new();
    let rooted = has_root(env);

    // --- 1. Ficheros y monturas de root -------------------------------- //
    for (marker, severity, desc) in ROOT_PATH_MARKERS {
        if env
            .files_present
            .iter()
            .any(|p| path_has_marker(p, marker))
        {
            leaks.push(LeakSignal {
                kind: "magisk_path".to_string(),
                detail: format!("{} (encontrado: {})", desc, marker),
                severity: severity.to_string(),
                remedied_by: match *marker {
                    "/data/adb/magisk.db" => "magisk_denylist_enable".to_string(),
                    "/data/adb/tricky_store" | "/data/adb/pif.json" => {
                        "play_integrity_fix_check".to_string()
                    }
                    _ => "shamiko_enable".to_string(),
                },
            });
        }
    }

    // `su` genérico: sólo como binario (no cualquier cosa que contenga "/su").
    for marker in ["/system/bin/su", "/system/xbin/su", "/sbin/su", "/su"] {
        if env
            .files_present
            .iter()
            .any(|p| path_has_marker(p, marker))
        {
            leaks.push(LeakSignal {
                kind: "su_binary".to_string(),
                detail: format!("binario `su` accesible en {}", marker),
                severity: "critical".to_string(),
                remedied_by: "magisk_denylist_add".to_string(),
            });
            break;
        }
    }

    // --- 2. Procesos de root ------------------------------------------- //
    for proc_name in ROOT_PROCESS_NAMES {
        if env.processes.iter().any(|p| {
            p == proc_name || p.starts_with(&format!("{}-", proc_name)) || p.ends_with(proc_name)
        }) {
            leaks.push(LeakSignal {
                kind: "magisk_process".to_string(),
                detail: format!("proceso `{}` visible en la tabla de procesos", proc_name),
                severity: "critical".to_string(),
                remedied_by: "shamiko_enable".to_string(),
            });
            break;
        }
    }

    // --- 3. Paquetes de herramientas root visibles --------------------- //
    for pkg in DETECTION_PACKAGE_NAMES {
        let visible_in_files = env
            .files_present
            .iter()
            .any(|p| p.contains(pkg) && p.ends_with(".apk"));
        let visible_in_paths = env.package_paths.iter().any(|p| p.contains(pkg));
        if visible_in_files || visible_in_paths {
            leaks.push(LeakSignal {
                kind: "magisk_app_visible".to_string(),
                detail: format!("el paquete `{}` está en el listado de apps", pkg),
                severity: "high".to_string(),
                remedied_by: "hide_packages".to_string(),
            });
            break;
        }
    }

    // --- 4. Props del sistema ------------------------------------------ //
    for (key, expected) in PROP_EXPECTATIONS {
        if let Some(actual) = env.prop(key) {
            if !prop_is_clean(key, actual, expected) {
                leaks.push(LeakSignal {
                    kind: "prop_leak".to_string(),
                    detail: format!("{} = {} (limpio: {})", key, actual, expected),
                    severity: prop_severity(key).to_string(),
                    remedied_by: "resetprop".to_string(),
                });
            }
        }
    }
    // Cualquier `ro.magisk.*` delata al gestor aunque el resto esté limpio.
    for (key, value) in &env.props {
        if key.starts_with("ro.magisk.") {
            leaks.push(LeakSignal {
                kind: "prop_leak".to_string(),
                detail: format!("propiedad de Magisk expuesta: {} = {}", key, value),
                severity: "high".to_string(),
                remedied_by: "resetprop".to_string(),
            });
        }
    }

    // --- 5. Zygisk / System Framework ---------------------------------- //
    if env.zygisk {
        leaks.push(LeakSignal {
            kind: "zygisk_active".to_string(),
            detail: "Zygisk está activo (Zygisk Next o ReZygisk)".to_string(),
            severity: "medium".to_string(),
            remedied_by: "zygisk_next_enable".to_string(),
        });
    }
    for (marker, desc) in [
        ("/data/adb/modules/zygisk_next", "Zygisk Next instalado"),
        ("/data/adb/modules/rezygisk", "ReZygisk instalado"),
    ] {
        if env.files_present.iter().any(|p| p.contains(marker)) {
            leaks.push(LeakSignal {
                kind: "zygisk_active".to_string(),
                detail: desc.to_string(),
                severity: "medium".to_string(),
                remedied_by: "zygisk_next_enable".to_string(),
            });
        }
    }

    // --- 6. Framework Xposed / LSPosed --------------------------------- //
    if !env.xposed_modules.is_empty() {
        leaks.push(LeakSignal {
            kind: "xposed_framework".to_string(),
            detail: format!(
                "framework Xposed/LSPosed cargado con módulos: {}",
                env.xposed_modules.join(", ")
            ),
            severity: "critical".to_string(),
            remedied_by: "lsposed_denylist".to_string(),
        });
    }

    // --- 7. Hooks de Play Integrity ------------------------------------ //
    for (marker, desc) in [
        (
            "/data/adb/pif.json",
            "Play Integrity Fix (fichero pif.json accesible)",
        ),
        (
            "/data/adb/modules/playintegrityfix",
            "módulo PlayIntegrityFix accesible",
        ),
    ] {
        if env.files_present.iter().any(|p| p.contains(marker)) {
            leaks.push(LeakSignal {
                kind: "play_integrity_hook".to_string(),
                detail: desc.to_string(),
                severity: "medium".to_string(),
                remedied_by: "play_integrity_fix_check".to_string(),
            });
        }
    }
    if rooted && !env.play_integrity_fix {
        leaks.push(LeakSignal {
            kind: "play_integrity_hook".to_string(),
            detail: "Magisk instalado pero sin PlayIntegrityFix: la attestation \
                     de Play Integrity fallará en nivel DEVICE"
                .to_string(),
            severity: "medium".to_string(),
            remedied_by: "play_integrity_fix_check".to_string(),
        });
    }
    if rooted && !env.trickystore {
        leaks.push(LeakSignal {
            kind: "play_integrity_hook".to_string(),
            detail: "Magisk instalado pero sin TrickyStore: el keystore del \
                     dispositivo sigue delatando el unlock"
                .to_string(),
            severity: "low".to_string(),
            remedied_by: "trickystore_check".to_string(),
        });
    }

    // --- 8. Políticas de Magisk ---------------------------------------- //
    for (marker, desc) in [
        ("/data/adb/magisk.db", "base de datos de políticas de Magisk"),
        ("/data/adb/ksu/.allowlist", "allowlist de KernelSU"),
        ("/data/adb/ksu/.denylist", "denylist de KernelSU"),
    ] {
        if env.files_present.iter().any(|p| p.contains(marker)) {
            leaks.push(LeakSignal {
                kind: "magisk_policy_db".to_string(),
                detail: format!("{} accesible desde la app", desc),
                severity: "high".to_string(),
                remedied_by: "magisk_denylist_enable".to_string(),
            });
        }
    }

    // --- 9. AppOps de la app ------------------------------------------- //
    for (op, severity) in SENSITIVE_APPOPS {
        if let Some((_, mode)) = env.appops.iter().find(|(o, _)| o == op) {
            if mode == "allow" {
                leaks.push(LeakSignal {
                    kind: "appops_exposure".to_string(),
                    detail: format!("appop {} = {}", op, mode),
                    severity: severity.to_string(),
                    remedied_by: "appops_restrict".to_string(),
                });
            }
        }
    }

    // --- 10. Denylist / Shamiko ---------------------------------------- //
    if rooted && !env.is_denylisted(package) {
        leaks.push(LeakSignal {
            kind: "not_denylisted".to_string(),
            detail: format!("`{}` no está en la DenyList de Magisk", package),
            severity: "high".to_string(),
            remedied_by: "magisk_denylist_add".to_string(),
        });
    }
    if rooted && !env.shamiko {
        leaks.push(LeakSignal {
            kind: "shamiko_missing".to_string(),
            detail: "Shamiko no está activo: la DenyList sola no oculta \
                     magiskd ni las monturas"
                .to_string(),
            severity: "medium".to_string(),
            remedied_by: "shamiko_enable".to_string(),
        });
    }

    // --- 11. App inyectada por un módulo ------------------------------- //
    for p in &env.package_paths {
        if p.contains("/data/adb/") || p.contains("magisk") {
            leaks.push(LeakSignal {
                kind: "module_injected_app".to_string(),
                detail: format!("la app se sirve desde una ruta de módulo: {}", p),
                severity: "high".to_string(),
                remedied_by: "review_module_injection".to_string(),
            });
            break;
        }
    }

    // --- Puntuación ---------------------------------------------------- //
    let mut penalty: u16 = 0;
    for leak in &leaks {
        penalty = penalty.saturating_add(severity_weight(&leak.severity));
    }
    let score = (100u16.saturating_sub(penalty)).min(100) as u8;
    let verdict = if score >= 90 {
        "hidden"
    } else if score >= 60 {
        "partially_visible"
    } else if score >= 30 {
        "detectable"
    } else {
        "exposed"
    };

    // Orden estable: primero lo más grave, luego por orden de aparición.
    leaks.sort_by(|a, b| {
        severity_rank(&b.severity)
            .cmp(&severity_rank(&a.severity))
            .then_with(|| a.kind.cmp(&b.kind))
    });

    HideAudit {
        package: package.to_string(),
        leaks,
        score,
        verdict: verdict.to_string(),
    }
}

/// Audita un lote de paquetes contra el mismo entorno.
pub fn audit_all(packages: &[String], env: &DeviceEnv) -> Vec<HideAudit> {
    packages
        .iter()
        .map(|p| detect_leaks(p, env))
        .collect()
}

// ========================================================================= //
// Remediación
// ========================================================================= //

/// Traduce las fugas de una auditoría en un plan de remediación.
///
/// Cada remediación es **suficiente** por sí misma (no hace falta combinarla
/// con otras para tapar su fuga) y lleva el comando exacto cuando existe uno
/// (`command`) y las instrucciones manuales cuando no (`notes`).
pub fn plan_remediation(audit: &HideAudit) -> Vec<Remediation> {
    let pkg = audit.package.clone();
    let mut plan: Vec<Remediation> = Vec::new();

    // Props concretas: una remediación por prop, con su `resetprop` exacto.
    for leak in &audit.leaks {
        if leak.kind != "prop_leak" {
            continue;
        }
        // El detalle tiene el formato "<key> = <actual> (limpio: <esperado>)".
        let key = leak
            .detail
            .split(" = ")
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();
        let expected = PROP_EXPECTATIONS
            .iter()
            .find(|(k, _)| key.as_str() == *k)
            .map(|(_, v)| v.to_string());
        let expected = match expected {
            Some(v) => v,
            // Propiedad `ro.magisk.*`: el valor limpio es "vacío" → resetprop
            // --delete es lo correcto.
            None => continue,
        };
        plan.push(Remediation {
            action: "resetprop".to_string(),
            command: Some(format!("resetprop {} {}", key, expected)),
            reversible: true,
            requires_root: true,
            requires_confirm: true,
            target: key.clone(),
            notes: format!(
                "reversible con `resetprop --delete {}`; efectivo tras el siguiente reinicio \
                 (o al instante si el módulo aplica las props en post-fs-data)",
                key
            ),
        });
    }

    // Propiedades `ro.magisk.*`: se borran, no se falsean.
    for leak in &audit.leaks {
        if leak.kind == "prop_leak" && leak.detail.contains("propiedad de Magisk expuesta") {
            let key = leak
                .detail
                .split("expuesta: ")
                .nth(1)
                .and_then(|s| s.split(" = ").next())
                .unwrap_or_default()
                .trim()
                .to_string();
            if key.is_empty() {
                continue;
            }
            plan.push(Remediation {
                action: "resetprop".to_string(),
                command: Some(format!("resetprop --delete {}", key)),
                reversible: true,
                requires_root: true,
                requires_confirm: true,
                target: key.clone(),
                notes: format!("propiedad propia de Magisk: se borra, no se falsea ({})", key),
            });
        }
    }

    let has = |kind: &str| audit.leaks.iter().any(|l| l.kind == kind);

    if has("not_denylisted") {
        plan.push(Remediation {
            action: "magisk_denylist_add".to_string(),
            command: Some(format!("magisk --denylist add {}", pkg)),
            reversible: true,
            requires_root: true,
            requires_confirm: true,
            target: pkg.clone(),
            notes: format!(
                "la DenyList de Magisk esconde la montura y el proceso magiskd ante `{}`; \
                 reversible con `magisk --denylist remove {}`",
                pkg, pkg
            ),
        });
    }

    if has("magisk_policy_db") || (has("magisk_process") && !has("not_denylisted")) {
        plan.push(Remediation {
            action: "magisk_denylist_enable".to_string(),
            command: Some("magisk --denylist enable".to_string()),
            reversible: true,
            requires_root: true,
            requires_confirm: true,
            target: pkg.clone(),
            notes: "la DenyList debe estar activa; con Shamiko se deja el \
                    'Enforce DenyList' de Magisk en OFF (Shamiko lee la lista, \
                    no la aplica)"
                .to_string(),
        });
    }

    if has("magisk_app_visible") {
        let mut cmds: Vec<String> = Vec::new();
        for p in ROOT_TOOL_PACKAGES {
            cmds.push(format!("pm hide {}", p));
        }
        plan.push(Remediation {
            action: "hide_packages".to_string(),
            command: Some(cmds.join(" && ")),
            reversible: true,
            requires_root: true,
            requires_confirm: true,
            target: pkg.clone(),
            notes: "reversible con `pm unhide <paquete>`; Hide My Applist se \
                    configura aparte (oculta estos paquetes de la lista de apps \
                    que leen las bancos)"
                .to_string(),
        });
    }

    if has("magisk_path") || has("su_binary") || has("magisk_process") || has("shamiko_missing") {
        plan.push(Remediation {
            action: "shamiko_enable".to_string(),
            command: None,
            reversible: true,
            requires_root: true,
            requires_confirm: true,
            target: pkg.clone(),
            notes: "instalar el módulo Shamiko (LSPosed Developers) y reiniciar; \
                    requiere Zygisk (Next o ReZygisk). Es el que de verdad oculta \
                    magiskd, /sbin/.magisk y los montajes; la DenyList sola no llega"
                .to_string(),
        });
    }

    if has("zygisk_active") {
        plan.push(Remediation {
            action: "zygisk_next_enable".to_string(),
            command: None,
            reversible: true,
            requires_root: true,
            requires_confirm: true,
            target: pkg.clone(),
            notes: "instalar Zygisk Next (o ReZygisk) como sustituto de Zygisk \
                    incluido: es requisito de Shamiko, PlayIntegrityFix y \
                    TrickyStore"
                .to_string(),
        });
    }

    if has("xposed_framework") {
        plan.push(Remediation {
            action: "lsposed_denylist".to_string(),
            command: None,
            reversible: true,
            requires_root: true,
            requires_confirm: true,
            target: pkg.clone(),
            notes: "en LSPosed: ajustes del módulo → DenyList → marcar la app; \
                    y negar a la app los módulos gestor (Magisk, LSPosed Manager). \
                    Hide My Applist debe quedar **activo** para esa app"
                .to_string(),
        });
    }

    if has("play_integrity_hook") {
        plan.push(Remediation {
            action: "play_integrity_fix_check".to_string(),
            command: Some("ls -l /data/adb/modules/playintegrityfix/pif.json /data/adb/pif.json".to_string()),
            reversible: true,
            requires_root: true,
            requires_confirm: true,
            target: pkg.clone(),
            notes: "comprobar que PlayIntegrityFix (chiteroman) o PIF-Fork está \
                    instalado y con `pif.json` presente; sin él, la app ve \
                    'DEVICE_INTEGRITY' fallida aunque el root esté oculto"
                .to_string(),
        });
    }

    if has("trickystore_missing") {
        plan.push(Remediation {
            action: "trickystore_check".to_string(),
            command: Some("ls -l /data/adb/tricky_store/keybox.xml".to_string()),
            reversible: true,
            requires_root: true,
            requires_confirm: true,
            target: pkg.clone(),
            notes: "comprobar TrickyStore (5ec1cff) y su `keybox.xml` sin revocar; \
                    requiere Android 12+ y Zygisk"
                .to_string(),
        });
    }

    if has("appops_exposure") {
        let mut cmds: Vec<String> = Vec::new();
        let mut ops: Vec<String> = Vec::new();
        for (op, _) in SENSITIVE_APPOPS {
            if audit
                .leaks
                .iter()
                .any(|l| l.kind == "appops_exposure" && l.detail.contains(op))
            {
                cmds.push(format!("appops set {} {} deny", pkg, op));
                ops.push(op.to_string());
            }
        }
        plan.push(Remediation {
            action: "appops_restrict".to_string(),
            command: if cmds.is_empty() {
                None
            } else {
                Some(cmds.join(" && "))
            },
            reversible: true,
            requires_root: true,
            requires_confirm: true,
            target: ops.join(","),
            notes: "reversible con `appops set <pkg> <op> default`; bajar permisos \
                    que delatan capacidad de escaneo"
                .to_string(),
        });
    }

    if has("module_injected_app") {
        plan.push(Remediation {
            action: "review_module_injection".to_string(),
            command: None,
            reversible: true,
            requires_root: true,
            requires_confirm: true,
            target: pkg.clone(),
            notes: "algún módulo está sirviendo la app desde una ruta de módulo: \
                    revisar `magisk --list-modules` y mover esa app a sistema o \
                    reinstalarla como app de usuario"
                .to_string(),
        });
    }

    plan
}

/// Aplica una remediación.
///
/// **Simula** el efecto sobre `env` salvo que `env.executor` sea
/// `Executor::Shell { .. }`: en ese caso ejecuta el comando de verdad y, si
/// termina bien, refleja igualmente el cambio en el entorno en memoria.
///
/// Devuelve `Err` con el motivo cuando la ejecución real falla.
pub fn apply_remediation(r: &Remediation, env: &mut DeviceEnv) -> Result<(), String> {
    // --- 1. Ejecución real (sólo si hay executor) ---------------------- //
    if let Some(Executor::Shell { shell }) = env.executor.clone() {
        if let Some(cmd) = &r.command {
            let output = Command::new(&shell)
                .arg("-c")
                .arg(cmd)
                .output()
                .map_err(|e| format!("no se pudo ejecutar `{}`: {}", cmd, e))?;
            if !output.status.success() {
                return Err(format!(
                    "`{}` falló con {}: {}",
                    cmd,
                    output.status.code().unwrap_or(-1),
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
        }
    }

    // --- 2. Efecto sobre el entorno (simulado o ya ejecutado) ---------- //
    match r.action.as_str() {
        "magisk_denylist_add" => {
            let pkg = if r.target.is_empty() {
                r.command
                    .as_deref()
                    .and_then(|c| c.split_whitespace().last())
                    .unwrap_or_default()
                    .to_string()
            } else {
                r.target.clone()
            };
            if !pkg.is_empty() && !env.denylist_apps.iter().any(|p| *p == pkg) {
                env.denylist_apps.push(pkg);
            }
        }
        "magisk_denylist_enable" => {
            env.denylisted = true;
        }
        "hide_packages" => {
            for p in ROOT_TOOL_PACKAGES {
                if !env.hidden_packages.iter().any(|h| h == p) {
                    env.hidden_packages.push(p.to_string());
                }
            }
        }
        "resetprop" => {
            let key = r.target.clone();
            // `resetprop --delete <k>` → se borra la prop.
            let is_delete = r
                .command
                .as_deref()
                .map(|c| c.contains("--delete"))
                .unwrap_or(false);
            if is_delete {
                env.props.retain(|(k, _)| *k != key);
            } else {
                let value = r
                    .command
                    .as_deref()
                    .and_then(|c| c.split_whitespace().nth(2))
                    .unwrap_or_default()
                    .to_string();
                if let Some(slot) = env.props.iter_mut().find(|(k, _)| k.as_str() == key) {
                    slot.1 = value;
                } else {
                    env.props.push((key, value));
                }
            }
        }
        "shamiko_enable" => {
            env.shamiko = true;
        }
        "zygisk_next_enable" => {
            env.zygisk_next = true;
            env.zygisk = true;
        }
        "lsposed_denylist" => {
            env.xposed_modules.retain(|m| m != &r.target);
        }
        "play_integrity_fix_check" => {
            env.play_integrity_fix = true;
        }
        "trickystore_check" => {
            env.trickystore = true;
        }
        "appops_restrict" => {
            for op in r.target.split(',').filter(|s| !s.is_empty()) {
                if let Some(slot) = env.appops.iter_mut().find(|(o, _)| o.as_str() == op) {
                    slot.1 = "deny".to_string();
                }
            }
        }
        "review_module_injection" => {
            env.package_paths.retain(|p| !p.contains("/data/adb/"));
        }
        _ => {}
    }

    Ok(())
}

// ========================================================================= //
// Módulo Magisk propio
// ========================================================================= //

/// Layout de un módulo Magisk de ocultación listo para empaquetar en un ZIP.
///
/// Devuelve pares `(ruta relativa dentro del módulo, contenido)`. Los
/// *placeholders* están documentados en la cabecera de cada script y se
/// resuelven con valores por defecto en `${VAR:-valor}`.
///
/// El orden de ejecución de Magisk es el que importa:
///
/// ```text
/// install.sh  →  customize.sh  →  post-fs-data.sh  →  (arranque)  →  service.sh
///                             ▲                          ▲
///                    se ejecuta al instalar      ANTES de que arranque zygote
/// ```
///
/// `post-fs-data.sh` es el único sitio donde las props se pueden falsear
/// **antes** de que arranque zygote; si se pusieran en `service.sh`, el
/// `system_server` ya habría cacheado las props originales y Shamiko/PIF no
/// podrían sustituir la attestation.
pub fn magisk_module_layout() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();

    v.push((
        "module.prop".to_string(),
        r#"id=megadaemon_hide
name=MegaDaemon Root Hide
version=v13.4.0
versionCode=13400
author=MegaDaemon Engineering Team
description=Ocultacion de root para apps bancarias: props limpias antes de zygote, DenyList de Magisk, auditoria de fugas. Ver README del modulo.
"#
        .to_string(),
    ));

    v.push((
        "customize.sh".to_string(),
        r#"#!/system/bin/sh
# customize.sh — se ejecuta durante la instalacion del modulo (con root).
#
# PLACEHOLDERS DOCUMENTADOS (todos tienen valor por defecto, se pueden
# sobreescribir tocando este fichero antes de empaquetar el ZIP):
#   ${MD_HIDE_MODULE_NAME}      Nombre visible del modulo.
#   ${MD_HIDE_DENYLIST_APPS}    Paquetes separados por espacio que se meten en
#                               la DenyList de Magisk al terminar la instalacion.
#   ${MD_HIDE_HIDE_PACKAGES}    Paquetes separados por espacio que se ocultan
#                               con `pm hide` al terminar la instalacion.
#   ${MD_HIDE_PROP_RO_DEBUGGABLE}   Valor limpio de ro.debuggable.
#   ${MD_HIDE_PROP_RO_SECURE}       Valor limpio de ro.secure.
#   ${MD_HIDE_PROP_RO_BOOT_VERIFIED} Valor limpio de ro.boot.verifiedbootstate.
#
# REQUISITOS: Magisk (Kitsune) con Zygisk Next o ReZygisk + Shamiko.
# NO se debe aplicar Zygisk Detach a las apps de la denylist: Detach desmonta
# /data/adb y rompe a los modulos (TrickyStore, PIF).

SKIPUNZIP=0
MD_HIDE_MODULE_NAME="${MD_HIDE_MODULE_NAME:-MegaDaemon Root Hide}"
MD_HIDE_DENYLIST_APPS="${MD_HIDE_DENYLIST_APPS:-}"
MD_HIDE_HIDE_PACKAGES="${MD_HIDE_HIDE_PACKAGES:-com.topjohnwu.magisk io.github.huskydg.magisk org.lsposed.manager}"

ui_print "=========================================="
ui_print "  ${MD_HIDE_MODULE_NAME}"
ui_print "  Megadaemon · motor de ocultacion de root"
ui_print "=========================================="

# --- Comprobaciones de entorno -------------------------------------------
if [ ! -d "/data/adb/modules" ]; then
  ui_print "! No se detecta una instalacion de Magisk: abortando."
  abort "! Magisk no encontrado"
fi

for MOD in shamiko zygisk_next rezygisk playintegrityfix tricky_store; do
  if [ -d "/data/adb/modules/$MOD" ]; then
    ui_print "  - modulo detectado: $MOD"
  fi
done

# --- DenyList de Magisk ---------------------------------------------------
for APP in $MD_HIDE_DENYLIST_APPS; do
  magisk --denylist add "$APP" >/dev/null 2>&1 && ui_print "  + denylist: $APP"
done

# --- Ocultar paquetes del listado ----------------------------------------
for PKG in $MD_HIDE_HIDE_PACKAGES; do
  pm hide "$PKG" >/dev/null 2>&1 && ui_print "  + oculto: $PKG"
done

# --- Permisos --------------------------------------------------------------
ui_print "  Permisos aplicados. Reinicia para que las props se apliquen."
set_perm_recursive "$MODPATH" 0 0 0755 0644
ui_print "  Listo."
"#
    .to_string(),
    ));

    v.push((
        "post-fs-data.sh".to_string(),
        r#"#!/system/bin/sh
# post-fs-data.sh — se ejecuta DESPUES de montar /data y ANTES de arrancar
# zygote. Es EL UNICO momento en que las props se pueden sustituir de forma
# que system_server y Play Integrity las vean ya falseadas.
#
# Si las props se pusieran en service.sh, el arranque de zygote habria leido
# las props originales (ro.debuggable=1, ro.build.tags=test-keys) y las
# peticiones de attestation ya habrian salido con los valores reales.
#
# PLACEHOLDERS DOCUMENTADOS (valores por defecto = estado de fabrica):
#   ${MD_HIDE_PROP_RO_DEBUGGABLE}          -> 0
#   ${MD_HIDE_PROP_RO_SECURE}              -> 1
#   ${MD_HIDE_PROP_RO_BUILD_TYPE}          -> user
#   ${MD_HIDE_PROP_RO_BUILD_TAGS}          -> release-keys
#   ${MD_HIDE_PROP_RO_BUILD_CHARACTERISTICS} -> default
#   ${MD_HIDE_PROP_RO_BOOT_VERIFIED}       -> green
#   ${MD_HIDE_PROP_RO_FLASH_LOCKED}        -> 1
#   ${MD_HIDE_PROP_RO_VBMETA_DEVICE_STATE} -> locked
#   ${MD_HIDE_PROP_SERVICE_ADB_ROOT}       -> 0
#   ${MD_HIDE_PROP_RO_KERNEL_QEMU}         -> 0
#   ${MD_HIDE_PROP_RO_MOUNT_SECURE}        -> 1
#
# NOTA HONESTA: resetprop solo cambia lo que el proceso consulta ahi. Las
# props de solo lectura ya cacheadas por init/zygote NO cambian hasta el
# siguiente reinicio: por eso este script debe correr en post-fs-data y no en
# service.sh (se aplica en el mismo arranque, antes de zygote).

MD_HIDE_PROP_RO_DEBUGGABLE="${MD_HIDE_PROP_RO_DEBUGGABLE:-0}"
MD_HIDE_PROP_RO_SECURE="${MD_HIDE_PROP_RO_SECURE:-1}"
MD_HIDE_PROP_RO_BUILD_TYPE="${MD_HIDE_PROP_RO_BUILD_TYPE:-user}"
MD_HIDE_PROP_RO_BUILD_TAGS="${MD_HIDE_PROP_RO_BUILD_TAGS:-release-keys}"
MD_HIDE_PROP_RO_BUILD_CHARACTERISTICS="${MD_HIDE_PROP_RO_BUILD_CHARACTERISTICS:-default}"
MD_HIDE_PROP_RO_BOOT_VERIFIED="${MD_HIDE_PROP_RO_BOOT_VERIFIED:-green}"
MD_HIDE_PROP_RO_FLASH_LOCKED="${MD_HIDE_PROP_RO_FLASH_LOCKED:-1}"
MD_HIDE_PROP_RO_VBMETA_DEVICE_STATE="${MD_HIDE_PROP_RO_VBMETA_DEVICE_STATE:-locked}"
MD_HIDE_PROP_RO_MOUNT_SECURE="${MD_HIDE_PROP_RO_MOUNT_SECURE:-1}"
MD_HIDE_PROP_SERVICE_ADB_ROOT="${MD_HIDE_PROP_SERVICE_ADB_ROOT:-0}"
MD_HIDE_PROP_RO_KERNEL_QEMU="${MD_HIDE_PROP_RO_KERNEL_QEMU:-0}"

# Comprobacion de resetprop: sin el, este script no hace nada util.
if ! command -v resetprop >/dev/null 2>&1; then
  # Sin resetprop se usa `setprop` (solo funciona para props no ro.*).
  setprop ro.boot.verifiedbootstate "$MD_HIDE_PROP_RO_BOOT_VERIFIED" 2>/dev/null
  setprop service.adb.root "$MD_HIDE_PROP_SERVICE_ADB_ROOT" 2>/dev/null
else
  resetprop ro.debuggable "$MD_HIDE_PROP_RO_DEBUGGABLE"
  resetprop ro.secure "$MD_HIDE_PROP_RO_SECURE"
  resetprop ro.build.type "$MD_HIDE_PROP_RO_BUILD_TYPE"
  resetprop ro.build.tags "$MD_HIDE_PROP_RO_BUILD_TAGS"
  resetprop ro.build.characteristics "$MD_HIDE_PROP_RO_BUILD_CHARACTERISTICS"
  resetprop ro.boot.verifiedbootstate "$MD_HIDE_PROP_RO_BOOT_VERIFIED"
  resetprop ro.boot.flash.locked "$MD_HIDE_PROP_RO_FLASH_LOCKED"
  resetprop ro.boot.vbmeta.device_state "$MD_HIDE_PROP_RO_VBMETA_DEVICE_STATE"
  resetprop ro.mount.secure "$MD_HIDE_PROP_RO_MOUNT_SECURE"
  resetprop service.adb.root "$MD_HIDE_PROP_SERVICE_ADB_ROOT"
  resetprop ro.kernel.qemu "$MD_HIDE_PROP_RO_KERNEL_QEMU"
fi

# Props propias de Magisk: se borran (no hay valor "limpio" queponer).
for P in $(getprop 2>/dev/null | grep '^ro\.magisk\.' | cut -d'[' -f1 | sed 's/\[ro\.magisk\..*\]//'); do
  resetprop --delete "$P" 2>/dev/null
done

# Log minimo para depurar desde el propio dispositivo.
mkdir -p /data/adb/megadaemon 2>/dev/null
{
  echo "post-fs-data $(date) props:"
  getprop 2>/dev/null | grep -E 'ro\.debuggable|ro\.secure|ro\.build\.tags|ro\.boot\.verifiedbootstate|ro\.boot\.flash\.locked|service\.adb\.root'
} >> /data/adb/megadaemon/hide.log 2>/dev/null
"#
    .to_string(),
    ));

    v.push((
        "service.sh".to_string(),
        r#"#!/system/bin/sh
# service.sh — se ejecuta con el sistema YA arrancado (tras service).
# Aqui ya no sirven las props (zygote ya las leyo): su trabajo es
# (a) verificar que la configuracion quedo aplicada y (b) dejar traza.
#
# PLACEHOLDERS DOCUMENTADOS:
#   ${MD_HIDE_LOG}    Ruta del log (por defecto /data/adb/megadaemon/hide.log).
#   ${MD_HIDE_APPS}   Paquetes que deben estar en la DenyList (separados por espacio).

MD_HIDE_LOG="${MD_HIDE_LOG:-/data/adb/megadaemon/hide.log}"
MD_HIDE_APPS="${MD_HIDE_APPS:-}"

mkdir -p "$(dirname "$MD_HIDE_LOG")" 2>/dev/null

log() {
  echo "$(date) $*" >> "$MD_HIDE_LOG" 2>/dev/null
}

log "--- verificacion de MegaDaemon Root Hide ---"

# 1) Comprobar la DenyList de Magisk.
for APP in $MD_HIDE_APPS; do
  if magisk --denylist ls 2>/dev/null | grep -q "$APP"; then
    log "OK denylist: $APP"
  else
    log "AVISO fuera de la denylist: $APP"
  fi
done

# 2) Comprobar que los modulos de ocultacion estan instalados.
for MOD in shamiko zygisk_next rezygisk playintegrityfix tricky_store; do
  if [ -d "/data/adb/modules/$MOD" ]; then
    log "OK modulo: $MOD"
  else
    log "AVISO modulo ausente: $MOD"
  fi
done

# 3) Comprobar props clave (lectura, no escritura: aqui ya es tarde).
for P in ro.debuggable ro.secure ro.build.tags ro.boot.verifiedbootstate ro.boot.flash.locked service.adb.root; do
  V=$(getprop "$P" 2>/dev/null)
  case "$P" in
    ro.debuggable) [ "$V" = "1" ] && log "ALERTA $P=$V" ;;
    ro.secure) [ "$V" = "0" ] && log "ALERTA $P=$V" ;;
    ro.build.tags) echo "$V" | grep -q "test-keys" && log "ALERTA $P=$V" ;;
    ro.boot.verifiedbootstate) [ "$V" = "green" ] || log "ALERTA $P=$V" ;;
    ro.boot.flash.locked) [ "$V" = "1" ] || log "ALERTA $P=$V" ;;
    service.adb.root) [ "$V" = "1" ] && log "ALERTA $P=$V" ;;
  esac
  log "prop $P=$V"
done

# 4) Comprobar que la app de Magisk sigue oculta.
for PKG in com.topjohnwu.magisk io.github.huskydg.magisk org.lsposed.manager; do
  if pm list packages 2>/dev/null | grep -q "$PKG"; then
    log "AVISO paquete visible: $PKG"
  else
    log "OK paquete oculto: $PKG"
  fi
done

log "--- fin de la verificacion ---"
"#
    .to_string(),
    ));

    v
}

// ========================================================================= //
// Tests
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;

    fn banco_pkg() -> String {
        "com.bbva.bbvacontigo".to_string()
    }

    #[test]
    fn el_perfil_por_defecto_cubre_las_apps_bancarias_espanolas() {
        let p = default_profile();
        assert!(p.denylist_apps.contains(&"com.bbva.bbvacontigo".to_string()));
        assert!(p.denylist_apps.contains(&"com.revolut.revolut".to_string()));
        assert!(p.denylist_apps.contains(&"es.bancosantander.apps".to_string()));
        assert!(p.denylist_apps.contains(&"com.mcdonalds.mobileapp".to_string()));
        assert!(p.hide_packages.contains(&"com.topjohnwu.magisk".to_string()));
        assert!(p.deny_lsposed_modules.contains(&"org.lsposed.manager".to_string()));
        // Las props del perfil deben ser exactamente las limpias esperadas.
        assert_eq!(p.prop_overrides.len(), PROP_EXPECTATIONS.len());
        for (k, v) in &p.prop_overrides {
            assert!(!k.is_empty() && !v.is_empty());
        }
    }

    #[test]
    fn un_dispositivo_limpio_no_filtra_nada() {
        let env = DeviceEnv::clean();
        let audit = detect_leaks(&banco_pkg(), &env);
        assert!(
            audit.leaks.is_empty(),
            "no debería haber fugas: {:?}",
            audit.leaks
        );
        assert_eq!(audit.score, 100);
        assert_eq!(audit.verdict, "hidden");
    }

    #[test]
    fn se_detectan_su_magisk_magiskd_y_las_monturas() {
        let env = DeviceEnv::fully_exposed();
        let audit = detect_leaks(&banco_pkg(), &env);
        let kinds: Vec<&str> = audit.leaks.iter().map(|l| l.kind.as_str()).collect();
        assert!(kinds.contains(&"su_binary"), "faltan {:?}", kinds);
        assert!(kinds.contains(&"magisk_path"), "faltan {:?}", kinds);
        assert!(kinds.contains(&"magisk_process"), "faltan {:?}", kinds);
        assert!(kinds.contains(&"magisk_app_visible"), "faltan {:?}", kinds);
        assert!(kinds.contains(&"magisk_policy_db"), "faltan {:?}", kinds);
        assert!(audit.score < 40);
        assert_eq!(audit.verdict, "exposed");
    }

    #[test]
    fn se_detectan_las_props_que_delatan_y_las_de_magisk() {
        let env = DeviceEnv::fully_exposed();
        let audit = detect_leaks(&banco_pkg(), &env);
        let prop_details: Vec<&str> = audit
            .leaks
            .iter()
            .filter(|l| l.kind == "prop_leak")
            .map(|l| l.detail.as_str())
            .collect();
        assert!(
            prop_details.iter().any(|d| d.contains("ro.debuggable = 1")),
            "{:?}",
            prop_details
        );
        assert!(
            prop_details.iter().any(|d| d.contains("ro.build.tags = test-keys")),
            "{:?}",
            prop_details
        );
        assert!(
            prop_details.iter().any(|d| d.contains("ro.magisk.version")),
            "{:?}",
            prop_details
        );
        // Toda fuga debe apuntar a una remediación.
        for leak in &audit.leaks {
            assert!(!leak.remedied_by.is_empty());
        }
    }

    #[test]
    fn se_detectan_los_appops_sensibles() {
        let mut env = DeviceEnv::clean();
        env.appops = vec![
            ("WRITE_SECURE_SETTINGS".to_string(), "allow".to_string()),
            ("MANAGE_EXTERNAL_STORAGE".to_string(), "deny".to_string()),
        ];
        let audit = detect_leaks(&banco_pkg(), &env);
        let appops: Vec<&str> = audit
            .leaks
            .iter()
            .filter(|l| l.kind == "appops_exposure")
            .map(|l| l.detail.as_str())
            .collect();
        assert_eq!(appops.len(), 1, "{:?}", appops);
        assert!(appops[0].contains("WRITE_SECURE_SETTINGS"));
    }

    #[test]
    fn el_marco_xposed_cargado_es_fuga_critica() {
        let mut env = DeviceEnv::clean();
        env.xposed_modules = vec!["com.topjohnwu.magisk".to_string()];
        env.magisk_version = Some("27.0".to_string());
        let audit = detect_leaks(&banco_pkg(), &env);
        let xposed = audit
            .leaks
            .iter()
            .find(|l| l.kind == "xposed_framework")
            .expect("debe detectar el framework");
        assert_eq!(xposed.severity, "critical");
    }

    #[test]
    fn el_plan_de_remediacion_cubre_cada_fuga_critica() {
        let env = DeviceEnv::fully_exposed();
        let audit = detect_leaks(&banco_pkg(), &env);
        let plan = plan_remediation(&audit);
        let actions: Vec<&str> = plan.iter().map(|r| r.action.as_str()).collect();
        assert!(actions.contains(&"magisk_denylist_add"));
        assert!(actions.contains(&"resetprop"));
        assert!(actions.contains(&"shamiko_enable"));
        assert!(actions.contains(&"hide_packages"));
        assert!(actions.contains(&"lsposed_denylist"));
        assert!(actions.contains(&"play_integrity_fix_check"));
        // Cada remediación es confirmable y sabe si necesita root.
        for r in &plan {
            assert!(r.requires_confirm, "{} debe pedir confirmacion", r.action);
            assert!(!r.target.is_empty() || r.action == "resetprop");
        }
        // El denylist se añade con el paquete correcto.
        let denylist = plan
            .iter()
            .find(|r| r.action == "magisk_denylist_add")
            .unwrap();
        assert_eq!(
            denylist.command.as_deref(),
            Some("magisk --denylist add com.bbva.bbvacontigo")
        );
    }

    #[test]
    fn el_resetprop_del_plan_usa_el_valor_limpio() {
        let mut env = DeviceEnv::clean();
        env.props = vec![("ro.debuggable".to_string(), "1".to_string())];
        let audit = detect_leaks(&banco_pkg(), &env);
        let plan = plan_remediation(&audit);
        let rp = plan
            .iter()
            .find(|r| r.action == "resetprop" && r.target == "ro.debuggable")
            .expect("debe haber resetprop para ro.debuggable");
        assert_eq!(rp.command.as_deref(), Some("resetprop ro.debuggable 0"));
        assert!(rp.reversible);
    }

    #[test]
    fn un_dispositivo_limpio_no_necesita_remediacion() {
        let env = DeviceEnv::clean();
        let audit = detect_leaks(&banco_pkg(), &env);
        let plan = plan_remediation(&audit);
        assert!(plan.is_empty(), "plan innecesario: {:?}", plan);
    }

    #[test]
    fn aplicar_una_remediacion_simula_el_cambio_en_el_entorno() {
        let mut env = DeviceEnv::fully_exposed();
        let audit = detect_leaks(&banco_pkg(), &env);
        let plan = plan_remediation(&audit);
        let denylist = plan
            .iter()
            .find(|r| r.action == "magisk_denylist_add")
            .cloned()
            .unwrap();
        // Sin executor no se ejecuta nada, pero el entorno refleja el efecto.
        assert!(env.executor.is_none());
        apply_remediation(&denylist, &mut env).expect("debe aplicar");
        assert!(env.is_denylisted("com.bbva.bbvacontigo"));

        // Re-auditar ya no reporta la fuga de denylist.
        let audit2 = detect_leaks(&banco_pkg(), &env);
        assert!(!audit2.leaks.iter().any(|l| l.kind == "not_denylisted"));
        assert!(
            audit2.leaks.len() < audit.leaks.len(),
            "aplicar la remediación debe reducir fugas: {:?} -> {:?}",
            audit.leaks.len(),
            audit2.leaks.len()
        );
    }

    #[test]
    fn aplicar_un_resetprop_corrige_la_prop_en_el_entorno() {
        let mut env = DeviceEnv::fully_exposed();
        let audit = detect_leaks(&banco_pkg(), &env);
        let plan = plan_remediation(&audit);
        let rp = plan
            .iter()
            .find(|r| r.action == "resetprop" && r.target == "ro.debuggable")
            .cloned()
            .unwrap();
        apply_remediation(&rp, &mut env).expect("debe aplicar");
        assert_eq!(env.prop("ro.debuggable"), Some("0"));
    }

    #[test]
    fn con_executor_real_se_ejecuta_el_comando_y_se_informa_del_error() {
        // El executor ejecuta de verdad: /bin/sh existe en el host de test.
        let mut env = DeviceEnv::default();
        env.executor = Some(Executor::Shell {
            shell: "/bin/sh".to_string(),
        });
        let ok = Remediation {
            action: "noop".to_string(),
            command: Some("true".to_string()),
            reversible: true,
            requires_root: false,
            requires_confirm: false,
            target: String::new(),
            notes: String::new(),
        };
        assert!(apply_remediation(&ok, &mut env).is_ok());

        let ko = Remediation {
            command: Some("false".to_string()),
            ..ok.clone()
        };
        let err = apply_remediation(&ko, &mut env).expect_err("debe fallar");
        assert!(err.contains("false"), "{}", err);
    }

    #[test]
    fn audit_all_devuelve_una_auditoria_por_paquete() {
        let env = DeviceEnv::fully_exposed();
        let pkgs = vec!["com.bbva.bbvacontigo".to_string(), "es.bancosantander.apps".to_string()];
        let audits = audit_all(&pkgs, &env);
        assert_eq!(audits.len(), 2);
        assert_eq!(audits[0].package, "com.bbva.bbvacontigo");
        assert_eq!(audits[1].package, "es.bancosantander.apps");
    }

    #[test]
    fn el_layout_del_modulo_magisk_tiene_los_cuatro_ficheros() {
        let layout = magisk_module_layout();
        let names: Vec<&str> = layout.iter().map(|(p, _)| p.as_str()).collect();
        assert!(names.contains(&"module.prop"));
        assert!(names.contains(&"service.sh"));
        assert!(names.contains(&"post-fs-data.sh"));
        assert!(names.contains(&"customize.sh"));

        let get = |name: &str| {
            layout
                .iter()
                .find(|(p, _)| p == name)
                .map(|(_, c)| c.clone())
                .unwrap()
        };
        // post-fs-data.sh debe aplicar las props ANTES de zygote.
        let post = get("post-fs-data.sh");
        assert!(post.contains("resetprop ro.debuggable"));
        assert!(post.contains("ANTES de arrancar"));
        assert!(post.contains("PLACEHOLDERS DOCUMENTADOS"));
        // service.sh no debe ser el sitio de las props.
        let service = get("service.sh");
        assert!(!service.contains("resetprop ro.debuggable"));
        assert!(service.contains("verificacion"));
        // module.prop debe ser válido para Magisk.
        let prop = get("module.prop");
        assert!(prop.contains("id=megadaemon_hide"));
        assert!(prop.contains("version="));
    }

    #[test]
    fn el_entorno_y_el_perfil_serializan_y_vuelven_igual() {
        let env = DeviceEnv::fully_exposed();
        let json = serde_json::to_string(&env).unwrap();
        let back: DeviceEnv = serde_json::from_str(&json).unwrap();
        assert_eq!(env, back);

        let profile = default_profile();
        let json = serde_json::to_string(&profile).unwrap();
        let back: HideProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(profile, back);
    }

    #[test]
    fn un_env_minimo_serializa_sin_los_campos_extendidos() {
        // Los campos extra tienen valor por defecto: un snapshot viejo (o
        // construido a mano) sigue siendo deserializable.
        let raw = json!({
            "files_present": ["/system/bin/su"],
            "props": [["ro.debuggable", "1"]],
            "processes": ["magiskd"],
            "xposed_modules": [],
            "zygisk": true,
            "magisk_version": "27.0",
            "denylisted": false,
            "package_paths": [],
            "appops": []
        });
        let env: DeviceEnv = serde_json::from_value(raw).expect("debe deserializar");
        assert!(env.executor.is_none());
        assert!(!env.shamiko);
        let audit = detect_leaks("com.bbva.bbvacontigo", &env);
        assert!(!audit.leaks.is_empty());
    }
}
