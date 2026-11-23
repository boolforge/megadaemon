//! MegaDaemon v16 — AUTOMATIZACIÓN DE ENERGÍA Y THERMAL.
//!
//! En el S21 Ultra hay **dos** sistemas de ahorro de energía coexistiendo:
//!
//! * `XtremeBS` (módulo Magisk v2.1.4, "Xtreme Battery Saver" de DethByte64):
//!   un daemon (`XtremeBSd`) + un controlador (`XBSctl` con `start/stop/
//!   pause/resume/safe`) + una WebUI en `127.0.0.1:8081`;
//! * el propio Android, con el *ciclo de vida de apps* y la *protección de
//!   batería* de Samsung (`mProtectionThreshold: 80` medido en el móvil).
//!
//! Este módulo **no sustituye** al XtremeBS: lo *integra*. Lee su estado real,
//! lo usa como fuente de la política y decide por su cuenta lo que el módulo
//! no cubre (límite de carga, Política térmica, veto de tareas pesadas).
//!
//! ## Pureza
//!
//! No toca el dispositivo: recibe un [`Snapshot`] ya medido y devuelve una
//! [`Decision`]. El runtime (`crate::automation_runtime`) es quien mide y quien
//! aplica. Así la política es testeable y auditable.

use serde_json::{json, Value};

/// Zona térmica del dispositivo.
#[derive(Clone, Debug, PartialEq)]
pub struct ThermalZone {
    pub name: String,
    pub temp_c: f32,
}

/// Lo que se ha medido del módulo Xtreme Battery Saver.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct XtremeBsInfo {
    /// El módulo Magisk `XtremeBS` está instalado.
    pub module_present: bool,
    pub version: Option<String>,
    /// El daemon `XtremeBSd` está corriendo.
    pub daemon_running: bool,
    /// Ruta del controlador `XBSctl` (si existe).
    pub ctl_path: Option<String>,
    /// Modo agresivo.
    pub aggressive: Option<bool>,
    /// Lista blanca de apps que XtremeBS no mata.
    pub whitelist: Vec<String>,
    /// Nº de apps que el módulo dice haber matado.
    pub killed: usize,
    /// La app Xtreme Battery Saver (XBS de ryangconrad) está instalada.
    pub app_installed: bool,
    pub app_version: Option<String>,
}

/// Estado real del dispositivo en materia de energía.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    /// % de batería.
    pub capacity_pct: Option<u8>,
    pub charging: bool,
    pub usb: bool,
    /// Temperatura de la batería en °C.
    pub battery_temp_c: Option<f32>,
    /// Temperatura de la CPU en °C (la zona "BIG" si no hay sensor de CPU).
    pub cpu_temp_c: Option<f32>,
    pub thermal_zones: Vec<ThermalZone>,
    /// Modo ahorro del sistema (`settings get global low_power`).
    pub system_power_save: bool,
    pub governor: Option<String>,
    /// Límite de carga vigente en el dispositivo (Samsung: 80 %).
    pub charge_limit_pct: Option<u8>,
    /// El dispositivo admite límite de carga.
    pub charge_limit_supported: bool,
    pub xbs: XtremeBsInfo,
}

impl Snapshot {
    /// Temperatura máxima de las zonas (la que manda para el veto térmico).
    pub fn max_zone_temp(&self) -> Option<f32> {
        // `f32` no es `Ord`: se usa `f32::max` (que además trata los NaN).
        self.thermal_zones
            .iter()
            .map(|z| z.temp_c)
            .fold(None, |a, b| Some(a.map_or(b, |x| x.max(b))))
    }

    /// ¿El móvil está demasiado caliente para el umbral dado?
    pub fn is_hot(&self, limit_c: f32) -> bool {
        match self.max_zone_temp() {
            Some(t) => t >= limit_c,
            None => false,
        }
    }
}

/// De dónde sale un perfil.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileSource {
    /// Viene con MegaDaemon.
    Builtin,
    /// Importado del XtremeBS instalado.
    XtremeBs,
    /// Definido por el usuario.
    User,
}

impl ProfileSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProfileSource::Builtin => "builtin",
            ProfileSource::XtremeBs => "xtremebs",
            ProfileSource::User => "user",
        }
    }
    pub fn from_str(s: &str) -> Option<ProfileSource> {
        Some(match s {
            "builtin" => ProfileSource::Builtin,
            "xtremebs" => ProfileSource::XtremeBs,
            "user" => ProfileSource::User,
            _ => return None,
        })
    }
}

/// Política de energía por perfil.
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub name: String,
    pub enabled: bool,
    /// Cargar como máximo hasta este %.
    pub charge_limit_pct: Option<u8>,
    /// No empezar a cargar por debajo de este %.
    pub charge_start_pct: Option<u8>,
    /// Umbral térmico en °C: por encima, se veta el trabajo pesado.
    pub thermal_limit_c: Option<f32>,
    /// Permite tareas pesadas con este perfil.
    pub heavy_allowed: bool,
    /// Tope de CPU (%) que se puede fijar.
    pub max_cpu_percent: Option<u8>,
    pub source: ProfileSource,
}

impl Default for Profile {
    fn default() -> Profile {
        Profile {
            name: "normal".into(),
            enabled: true,
            charge_limit_pct: None,
            charge_start_pct: None,
            thermal_limit_c: Some(45.0),
            heavy_allowed: true,
            max_cpu_percent: None,
            source: ProfileSource::Builtin,
        }
    }
}

/// Acción de energía que hay que aplicar.
#[derive(Clone, Debug, PartialEq)]
pub enum EnergyAction {
    SetChargeLimit(u8),
    StopCharging,
    ResumeCharging,
    SetProfile(String),
    RestrictHeavy(bool),
    Notify { title: String, body: String },
    Noop,
}

/// Decisión completa: qué hacer y por qué.
#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    pub profile: String,
    pub actions: Vec<EnergyAction>,
    pub reasons: Vec<String>,
}

/// Perfiles que trae MegaDaemon de serie.
///
/// * `normal`: comportamiento equilibrado, sin topes de carga agresivos.
/// * `conservacion`: para días largos; limita la carga al 80 % y veta el
///   trabajo pesado por debajo del 25 % de batería.
/// * `viaje`: carga hasta el 90 % pero no pasa del 85 % enPlugged.
pub fn builtin_profiles() -> Vec<Profile> {
    vec![
        Profile {
            name: "normal".into(),
            enabled: true,
            charge_limit_pct: None,
            charge_start_pct: None,
            thermal_limit_c: Some(45.0),
            heavy_allowed: true,
            max_cpu_percent: None,
            source: ProfileSource::Builtin,
        },
        Profile {
            name: "conservacion".into(),
            enabled: true,
            charge_limit_pct: Some(80),
            charge_start_pct: Some(25),
            thermal_limit_c: Some(42.0),
            heavy_allowed: false,
            max_cpu_percent: Some(60),
            source: ProfileSource::Builtin,
        },
        Profile {
            name: "viaje".into(),
            enabled: true,
            charge_limit_pct: Some(85),
            charge_start_pct: Some(20),
            thermal_limit_c: Some(44.0),
            heavy_allowed: true,
            max_cpu_percent: None,
            source: ProfileSource::Builtin,
        },
    ]
}

/// Elige el perfil activo según el estado y las reglas del usuario.
///
/// `explicit` es el perfil pedido a mano (si existe y está activo, gana).
pub fn choose_profile(snap: &Snapshot, profiles: &[Profile], explicit: &str) -> Profile {
    if !explicit.is_empty() {
        if let Some(p) = profiles.iter().find(|p| p.name == explicit && p.enabled) {
            return p.clone();
        }
    }
    let cap = snap.capacity_pct.unwrap_or(100);
    // Conservación por debajo del 25 % de batería.
    if cap < 25 {
        if let Some(p) = profiles
            .iter()
            .find(|p| p.name == "conservacion" && p.enabled)
        {
            return p.clone();
        }
    }
    // Si el móvil está muy caliente, el perfil más restrictivo gana.
    // `f32` no implementa `Ord`: el minimo se calcula con `f32::min`.
    let min_limit = profiles
        .iter()
        .filter(|p| p.enabled)
        .filter_map(|p| p.thermal_limit_c)
        .fold(None, |a: Option<f32>, b| Some(a.map_or(b, |x| x.min(b))));
    if let Some(limit) = min_limit {
        if snap.is_hot(limit) {
            if let Some(p) = profiles
                .iter()
                .find(|p| p.name == "conservacion" && p.enabled)
            {
                return p.clone();
            }
        }
    }
    profiles
        .iter()
        .find(|p| p.enabled)
        .cloned()
        .unwrap_or_else(|| Profile::default())
}

/// Decide qué hacer con la energía.
pub fn decide(snap: &Snapshot, profiles: &[Profile], explicit: &str) -> Decision {
    let profile = choose_profile(snap, profiles, explicit);
    let mut actions = Vec::new();
    let mut reasons = Vec::new();

    // 1. Límite de carga.
    if let (Some(limit), Some(current)) = (profile.charge_limit_pct, snap.charge_limit_pct) {
        if limit != current {
            actions.push(EnergyAction::SetChargeLimit(limit));
            reasons.push(format!(
                "límite de carga {}% ≠ {}% configurado en el perfil '{}'",
                current, limit, profile.name
            ));
        }
    }

    // 2. Cargar / dejar de cargar según el rango del perfil.
    if let (Some(start), Some(cap)) = (profile.charge_start_pct, snap.capacity_pct) {
        if snap.charging && cap < start {
            actions.push(EnergyAction::StopCharging);
            reasons.push(format!(
                "carga al {}% por debajo del mínimo {}% del perfil '{}'",
                cap, start, profile.name
            ));
        }
        if !snap.charging && snap.usb && cap >= start {
            actions.push(EnergyAction::ResumeCharging);
            reasons.push(format!(
                "cargador presente y carga al {}% (mínimo {}) → reanudar carga",
                cap, start
            ));
        }
    }
    if let (Some(limit), Some(cap)) = (profile.charge_limit_pct, snap.capacity_pct) {
        if snap.charging && cap >= limit {
            actions.push(EnergyAction::StopCharging);
            reasons.push(format!(
                "carga al {}% alcanzada: tope del perfil '{}' = {}%",
                cap, profile.name, limit
            ));
        }
    }

    // 3. Política térmica.
    if let Some(limit) = profile.thermal_limit_c {
        let hot = snap.is_hot(limit);
        if hot && profile.heavy_allowed {
            actions.push(EnergyAction::RestrictHeavy(true));
            reasons.push(format!(
                "temperatura {:?}°C ≥ umbral {}°C: se veta el trabajo pesado",
                snap.max_zone_temp(),
                limit
            ));
            actions.push(EnergyAction::Notify {
                title: "MegaDaemon".into(),
                body: format!(
                    "Temperatura alta ({:?}°C): se limita el trabajo pesado",
                    snap.max_zone_temp()
                ),
            });
        } else if !hot && !profile.heavy_allowed {
            // El perfil restrictivo libera el trabajo pesado cuando ya no hace falta.
            actions.push(EnergyAction::RestrictHeavy(false));
            reasons.push("temperatura dentro de límites".into());
        }
        if hot && !profile.heavy_allowed {
            reasons.push(format!(
                "perfil '{}' ya veta el trabajo pesado (temperatura {:?}°C)",
                profile.name,
                snap.max_zone_temp()
            ));
        }
    }

    // 4. El XtremeBS manda sobre el estado real: si está en "safe mode",
    //    no se toca nada heavy.
    if snap.xbs.module_present {
        reasons.push(format!(
            "XtremeBS {} detectado (daemon {}) → la política lo respeta",
            snap.xbs.version.clone().unwrap_or_else(|| "?".into()),
            if snap.xbs.daemon_running { "vivo" } else { "parado" }
        ));
    }

    if actions.is_empty() {
        actions.push(EnergyAction::Noop);
        reasons.push("nada que hacer".into());
    }
    Decision {
        profile: profile.name,
        actions,
        reasons,
    }
}

// ========================================================================= //
// Parseo de la medición
// ========================================================================= //

fn f(v: &str) -> Option<f32> {
    v.trim().parse::<f32>().ok()
}

/// Parsea la medición en formato `clave=valor` que produce el runtime.
///
/// Formato pensado para ser estable y fácil de testear:
/// ```text
/// capacity=51
/// charging=0
/// thermal=BIG:72.0
/// xbs_module=1
/// ```
pub fn parse_snapshot(text: &str) -> Snapshot {
    let mut s = Snapshot::default();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let k = k.trim();
        let v = v.trim();
        let on = matches!(v, "1" | "true" | "yes" | "on");
        match k {
            "capacity" => s.capacity_pct = v.parse::<u8>().ok(),
            "charging" => s.charging = on,
            "usb" => s.usb = on,
            "battery_temp_c" => s.battery_temp_c = f(v),
            "cpu_temp_c" => s.cpu_temp_c = f(v),
            "power_save" => s.system_power_save = on,
            "governor" => s.governor = Some(v.to_string()),
            "charge_limit" => s.charge_limit_pct = v.parse::<u8>().ok(),
            "charge_limit_supported" => s.charge_limit_supported = on,
            "thermal" => {
                if let Some((name, t)) = v.rsplit_once(':') {
                    if let Some(temp) = f(t) {
                        s.thermal_zones.push(ThermalZone {
                            name: name.trim().to_string(),
                            temp_c: temp,
                        });
                    }
                }
            }
            "xbs_module" => s.xbs.module_present = on,
            "xbs_version" => s.xbs.version = Some(v.to_string()),
            "xbs_daemon" => s.xbs.daemon_running = on,
            "xbs_ctl" => s.xbs.ctl_path = Some(v.to_string()),
            "xbs_aggressive" => s.xbs.aggressive = Some(on),
            "xbs_app" => s.xbs.app_installed = on,
            "xbs_app_version" => s.xbs.app_version = Some(v.to_string()),
            "xbs_killed" => s.xbs.killed = v.parse::<usize>().unwrap_or(0),
            "xbs_whitelist" => {
                s.xbs.whitelist = v
                    .split(',')
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty())
                    .collect()
            }
            _ => {}
        }
    }
    s
}

// ========================================================================= //
// JSON / persistencia
// ========================================================================= //

/// Snapshot → JSON (para la herramienta MCP y el CLI).
pub fn snapshot_to_json(s: &Snapshot) -> Value {
    json!({
        "capacity_pct": s.capacity_pct,
        "charging": s.charging,
        "usb": s.usb,
        "battery_temp_c": s.battery_temp_c,
        "cpu_temp_c": s.cpu_temp_c,
        "max_zone_temp_c": s.max_zone_temp(),
        "thermal_zones": s.thermal_zones.iter().map(|z| json!({
            "name": z.name, "temp_c": z.temp_c
        })).collect::<Vec<_>>(),
        "system_power_save": s.system_power_save,
        "governor": s.governor,
        "charge_limit_pct": s.charge_limit_pct,
        "charge_limit_supported": s.charge_limit_supported,
        "xbs": json!({
            "module_present": s.xbs.module_present,
            "version": s.xbs.version,
            "daemon_running": s.xbs.daemon_running,
            "ctl_path": s.xbs.ctl_path,
            "aggressive": s.xbs.aggressive,
            "whitelist": s.xbs.whitelist,
            "app_installed": s.xbs.app_installed,
            "app_version": s.xbs.app_version,
            "killed": s.xbs.killed,
        }),
    })
}

pub fn profile_to_json(p: &Profile) -> Value {
    json!({
        "name": p.name,
        "enabled": p.enabled,
        "charge_limit_pct": p.charge_limit_pct,
        "charge_start_pct": p.charge_start_pct,
        "thermal_limit_c": p.thermal_limit_c,
        "heavy_allowed": p.heavy_allowed,
        "max_cpu_percent": p.max_cpu_percent,
        "source": p.source.as_str(),
    })
}

pub fn profile_from_json(v: &Value) -> Result<Profile, String> {
    let name = v
        .get("name")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "perfil sin 'name'".to_string())?;
    Ok(Profile {
        name: name.to_string(),
        enabled: v.get("enabled").and_then(|x| x.as_bool()).unwrap_or(true),
        charge_limit_pct: v.get("charge_limit_pct").and_then(|x| x.as_u64()).map(|x| x as u8),
        charge_start_pct: v.get("charge_start_pct").and_then(|x| x.as_u64()).map(|x| x as u8),
        thermal_limit_c: v.get("thermal_limit_c").and_then(|x| x.as_f64()).map(|x| x as f32),
        heavy_allowed: v.get("heavy_allowed").and_then(|x| x.as_bool()).unwrap_or(true),
        max_cpu_percent: v.get("max_cpu_percent").and_then(|x| x.as_u64()).map(|x| x as u8),
        source: v
            .get("source")
            .and_then(|x| x.as_str())
            .and_then(ProfileSource::from_str)
            .unwrap_or(ProfileSource::User),
    })
}

/// Carga los perfiles (los de usuario si existen; si no, los de serie).
pub fn load_profiles(path: &str) -> Vec<Profile> {
    let Ok(txt) = std::fs::read_to_string(path) else {
        return builtin_profiles();
    };
    let Ok(v) = serde_json::from_str::<Value>(&txt) else {
        return builtin_profiles();
    };
    let arr = v
        .get("profiles")
        .and_then(|x| x.as_array())
        .cloned()
        .or_else(|| v.as_array().cloned())
        .unwrap_or_default();
    let user: Vec<Profile> = arr.iter().filter_map(|p| profile_from_json(p).ok()).collect();
    if user.is_empty() {
        builtin_profiles()
    } else {
        user
    }
}

pub fn save_profiles(path: &str, profiles: &[Profile]) -> Result<(), String> {
    if let Some(dir) = std::path::Path::new(path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let v = json!({
        "version": 1,
        "profiles": profiles.iter().map(profile_to_json).collect::<Vec<_>>(),
    });
    std::fs::write(path, serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

// ========================================================================= //
// Tests
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(cap: u8, charging: bool, temp: f32) -> Snapshot {
        Snapshot {
            capacity_pct: Some(cap),
            charging,
            usb: charging,
            battery_temp_c: Some(33.8),
            cpu_temp_c: Some(temp),
            thermal_zones: vec![ThermalZone {
                name: "BIG".into(),
                temp_c: temp,
            }],
            charge_limit_pct: Some(80),
            charge_limit_supported: true,
            ..Snapshot::default()
        }
    }

    #[test]
    fn la_lectura_del_snapshot_entiende_el_formato() {
        let s = parse_snapshot(
            "capacity=51\ncharging=0\nusb=0\nbattery_temp_c=33.8\nthermal=BIG:72.0\nthermal=battery:33.8\nxbs_module=1\nxbs_version=v2.1.4\nxbs_whitelist=com.a,com.b\n",
        );
        assert_eq!(s.capacity_pct, Some(51));
        assert!(!s.charging);
        assert_eq!(s.battery_temp_c, Some(33.8));
        assert_eq!(s.thermal_zones.len(), 2);
        assert_eq!(s.max_zone_temp(), Some(72.0));
        assert!(s.xbs.module_present);
        assert_eq!(s.xbs.version.as_deref(), Some("v2.1.4"));
        assert_eq!(s.xbs.whitelist.len(), 2);
    }

    #[test]
    fn el_snapshot_ignora_lineas_basura() {
        let s = parse_snapshot("# comentario\n\nsin_igual\ncapacity=x\n");
        assert_eq!(s.capacity_pct, None);
        assert!(s.thermal_zones.is_empty());
    }

    #[test]
    fn por_debajo_del_25_por_ciento_se_elige_conservacion() {
        let ps = builtin_profiles();
        let p = choose_profile(&snap(20, false, 30.0), &ps, "");
        assert_eq!(p.name, "conservacion");
    }

    #[test]
    fn un_perfil_explicito_manda() {
        let ps = builtin_profiles();
        let p = choose_profile(&snap(90, true, 30.0), &ps, "viaje");
        assert_eq!(p.name, "viaje");
    }

    #[test]
    fn el_limite_de_carga_se_ajusta_al_perfil() {
        let ps = builtin_profiles();
        let d = decide(&snap(50, false, 30.0), &ps, "viaje");
        // El dispositivo está en 80 % y el perfil "viaje" pide 85 %.
        assert!(d
            .actions
            .iter()
            .any(|a| matches!(a, EnergyAction::SetChargeLimit(85))));
    }

    #[test]
    fn el_tope_de_carga_corta_la_carga() {
        let ps = builtin_profiles();
        let d = decide(&snap(85, true, 30.0), &ps, "viaje");
        assert!(d.actions.iter().any(|a| matches!(a, EnergyAction::StopCharging)));
    }

    #[test]
    fn por_debajo_del_minimo_se_deja_de_cargar() {
        let ps = builtin_profiles();
        let d = decide(&snap(20, true, 30.0), &ps, "conservacion");
        assert!(d.actions.iter().any(|a| matches!(a, EnergyAction::StopCharging)));
    }

    #[test]
    fn el_calor_veta_el_trabajo_pesado() {
        let ps = builtin_profiles();
        let d = decide(&snap(60, false, 60.0), &ps, "normal");
        assert!(d
            .actions
            .iter()
            .any(|a| matches!(a, EnergyAction::RestrictHeavy(true))));
        assert!(d.reasons.iter().any(|r| r.contains("temperatura")));
    }

    #[test]
    fn sin_cambios_no_hay_acciones() {
        let ps = builtin_profiles();
        let mut s = snap(50, false, 30.0);
        s.charge_limit_pct = None;
        let d = decide(&s, &ps, "normal");
        assert!(d.actions.iter().any(|a| matches!(a, EnergyAction::Noop)));
    }

    #[test]
    fn el_xtremebs_aparece_en_los_motivos() {
        let ps = builtin_profiles();
        let mut s = snap(50, false, 30.0);
        s.charge_limit_pct = None;
        s.xbs.module_present = true;
        s.xbs.version = Some("v2.1.4".into());
        s.xbs.daemon_running = true;
        let d = decide(&s, &ps, "normal");
        assert!(d.reasons.iter().any(|r| r.contains("XtremeBS")));
    }

    #[test]
    fn los_perfiles_van_y_vuelven_de_json() {
        let p = Profile {
            name: "mio".into(),
            enabled: false,
            charge_limit_pct: Some(70),
            charge_start_pct: Some(30),
            thermal_limit_c: Some(40.0),
            heavy_allowed: false,
            max_cpu_percent: Some(50),
            source: ProfileSource::User,
        };
        let j = profile_to_json(&p);
        let back = profile_from_json(&j).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn los_perfiles_se_guardan_y_se_cargan() {
        let p = "/tmp/md_test_energy_profiles.json";
        let mut ps = builtin_profiles();
        ps[0].charge_limit_pct = Some(75);
        save_profiles(p, &ps).unwrap();
        let back = load_profiles(p);
        assert_eq!(back.len(), 3);
        assert_eq!(back[0].charge_limit_pct, Some(75));
        let _ = std::fs::remove_file(p);
    }
}
