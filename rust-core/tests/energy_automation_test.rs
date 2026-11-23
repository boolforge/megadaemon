//! Arnés de pruebas de la automatización de energía (`src/energy_automation.rs`).
//!
//! Usa la biblioteca (`use megadaemon_core::…;`): los módulos ya están
//! cableados en `src/lib.rs`.

use megadaemon_core::energy_automation::{
    builtin_profiles, decide, parse_snapshot, profile_from_json, profile_to_json, EnergyAction,
    Profile, ProfileSource, Snapshot, ThermalZone,
};

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
fn la_lectura_del_snapshot_trae_el_xtremebs() {
    let s = parse_snapshot(
        "capacity=51\ncharging=0\nusb=0\nbattery_temp_c=33.8\nthermal=BIG:72.0\n\
         xbs_module=1\nxbs_version=v2.1.4\nxbs_daemon=1\nxbs_whitelist=com.a,com.b\n",
    );
    assert_eq!(s.capacity_pct, Some(51));
    assert_eq!(s.thermal_zones.len(), 1);
    assert_eq!(s.max_zone_temp(), Some(72.0));
    assert!(s.xbs.module_present);
    assert_eq!(s.xbs.version.as_deref(), Some("v2.1.4"));
    assert!(s.xbs.daemon_running);
    assert_eq!(s.xbs.whitelist.len(), 2);
}

#[test]
fn el_calor_veta_el_trabajo_pesado_y_lo_explica() {
    let ps = builtin_profiles();
    let d = decide(&snap(60, false, 60.0), &ps, "normal");
    assert!(d
        .actions
        .iter()
        .any(|a| matches!(a, EnergyAction::RestrictHeavy(true))));
    assert!(d.reasons.iter().any(|r| r.contains("temperatura")));
}

#[test]
fn el_limite_de_carga_se_ajusta_al_perfil_y_se_corta_la_carga() {
    let ps = builtin_profiles();
    let d = decide(&snap(85, true, 30.0), &ps, "viaje");
    assert!(d
        .actions
        .iter()
        .any(|a| matches!(a, EnergyAction::SetChargeLimit(85))));
    assert!(d.actions.iter().any(|a| matches!(a, EnergyAction::StopCharging)));
}

#[test]
fn por_debajo_del_minimo_del_perfil_se_deja_de_cargar() {
    let ps = builtin_profiles();
    let d = decide(&snap(20, true, 30.0), &ps, "conservacion");
    assert!(d.actions.iter().any(|a| matches!(a, EnergyAction::StopCharging)));
    assert!(d.reasons.iter().any(|r| r.contains("mínimo")));
}

#[test]
fn con_bateria_baja_se_elige_el_perfil_conservador() {
    let ps = builtin_profiles();
    let d = decide(&snap(15, false, 30.0), &ps, "");
    assert_eq!(d.profile, "conservacion");
}

#[test]
fn los_perfiles_van_y_vuelven_de_json() {
    let p = Profile {
        name: "mio".into(),
        enabled: true,
        charge_limit_pct: Some(70),
        charge_start_pct: Some(30),
        thermal_limit_c: Some(40.0),
        heavy_allowed: false,
        max_cpu_percent: Some(50),
        source: ProfileSource::User,
    };
    let back = profile_from_json(&profile_to_json(&p)).unwrap();
    assert_eq!(back, p);
}

#[test]
fn una_situacion_normal_no_pide_nada() {
    let ps = builtin_profiles();
    let mut s = snap(50, false, 30.0);
    s.charge_limit_pct = None;
    let d = decide(&s, &ps, "normal");
    assert!(d.actions.iter().any(|a| matches!(a, EnergyAction::Noop)));
}
