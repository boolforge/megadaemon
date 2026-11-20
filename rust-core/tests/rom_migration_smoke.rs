//! W4 · Arnés de pruebas del módulo `rom_migration`.
//!
//! `src/lib.rs` es intocable en esta tarea (regla del plan: no se toca
//! `lib.rs` sin acuerdo), así que el módulo se incluye aquí directamente con
//! `#[path]`, igual que hacen `tests/signing_vault.rs` y
//! `tests/device_inventory_smoke.rs`. En cuanto se añada
//! `pub mod rom_migration;` a `src/lib.rs`, este arnés puede borrarse sin
//! tocar los tests.
//!
//! Aquí se prueba el **comportamiento completo** (decisiones y abortes), que es
//! lo que importa en una migración de ROM: un plan que no aborta cuando debe
//! acaba en un ladrillo.

#[path = "../src/rom_migration.rs"]
mod rom_migration;

use rom_migration::{
    compare_inventories, dry_run_report, efs_backup_plan, flash_plan, mark_efs_verified,
    plan_json, preflight, select_rom, AvbState, DeviceFacts, EfsBackupPlan, EFS_SAFE_WORLD,
    FlashStage, RomPackage, SelectionCriteria, UnlockState,
};
use std::collections::BTreeMap;

fn device_ready() -> DeviceFacts {
    DeviceFacts {
        model: "SM-G998B".into(),
        csc: "AMO".into(),
        patch_date: "2024-06-01".into(),
        android_version: "14".into(),
        bootloader_version: "S21U_2A_14_0".into(),
        modem_region: "SM".into(),
        avb_state: AvbState::Yellow,
        flash_locked: false,
        fmm_lock: false,
        frp_locked: false,
        omi_locked: false,
        battery_pct: 91,
        battery_temp_c: 27.5,
        free_space_mb: 38_000,
        required_space_mb: 8_192,
        odin_version: Some("3.14.4".into()),
        odin_min_version: Some("3.14.4".into()),
        stock_bootimg: true,
        efs_partitions: EFS_SAFE_WORLD.iter().map(|s| s.to_string()).collect(),
        anti_rollback_path: Some("/root/FLL.bin".into()),
        anti_rollback_sha256: "0".repeat(64),
    }
}

fn rom_ok() -> RomPackage {
    RomPackage {
        name: "ROM-AMO-2024-07".into(),
        csc: "AMO".into(),
        patch_date: "2024-07-01".into(),
        android_version: "14".into(),
        modem_region: "SM".into(),
        bootloader_version: "S21U_2A_14_1".into(),
        stock_bootimg: true,
        bootimg_note: None,
        pit_match: true,
        artifacts: vec![
            rom_migration::Artifact {
                name: "AP_MDPO".into(),
                path: "/rom/AP_MDPO_MD1.img".into(),
                size_bytes: 5_000_000_000,
                sha256: "a".repeat(64),
            },
            rom_migration::Artifact {
                name: "CP_MDPO".into(),
                path: "/rom/CP_MDPO_MD1.img".into(),
                size_bytes: 300_000_000,
                sha256: "b".repeat(64),
            },
            rom_migration::Artifact {
                name: "CSC_MD1".into(),
                path: "/rom/CSC_MD1.img".into(),
                size_bytes: 200_000_000,
                sha256: "c".repeat(64),
            },
            rom_migration::Artifact {
                name: "HOME_CSC".into(),
                path: "/rom/HOME_CSC.img".into(),
                size_bytes: 20_000_000,
                sha256: "d".repeat(64),
            },
            rom_migration::Artifact {
                name: "BOOTLOADER".into(),
                path: "/rom/BOOTLOADER.img".into(),
                size_bytes: 15_000_000,
                sha256: "e".repeat(64),
            },
        ],
    }
}

fn efs_verified(device: &DeviceFacts) -> EfsBackupPlan {
    let mut plan = efs_backup_plan(device, "/tmp/efs-backup");
    let mut hashes = BTreeMap::new();
    for p in EFS_SAFE_WORLD.iter() {
        hashes.insert(p.to_string(), "1".repeat(64));
    }
    mark_efs_verified(&mut plan, &hashes);
    plan
}

#[test]
fn el_movil_medido_no_tiene_fichero_anti_rollback_y_eso_aborta() {
    // Medido: /root/ no existe en este dispositivo y /efs/ no lo contiene.
    // Sin copia verificada del anti-rollback, la migración no puede empezar.
    let mut dev = device_ready();
    dev.anti_rollback_path = None;
    dev.anti_rollback_sha256 = String::new();
    let rep = preflight(&dev, &rom_ok(), &BTreeMap::new());
    assert!(rep.abort);
    let c = rep
        .failed_blockers()
        .into_iter()
        .find(|c| c.id == "anti_rollback_backup")
        .expect("debe aparecer el check");
    assert!(c.detail.contains("/root/"));
}

#[test]
fn el_movil_medido_no_puede_migrar_y_se_explica_por_que() {
    // Hechos reales del S21 Ultra: bootloader cerrado con FMM Lock, FRP
    // activa, batería al 62 % y boot.img no stock.
    let dev = DeviceFacts::default();
    assert_eq!(dev.unlock_state(), UnlockState::LockedFmm);
    let rep = preflight(&dev, &rom_ok(), &BTreeMap::new());
    assert!(rep.abort);
    let texto = rep.to_string();
    assert!(texto.contains("ABORTA"));
    assert!(texto.contains("try jtag unlock"));
}

#[test]
fn con_todo_en_verde_el_plan_de_flash_tiene_los_cuatro_pasos() {
    let dev = device_ready();
    let rom = rom_ok();
    let efs = efs_verified(&dev);
    let pre = preflight(&dev, &rom, &BTreeMap::new());
    assert!(pre.can_flash(), "{:?}", pre.blockers);

    let plan = flash_plan(&rom, &dev, &efs, true);
    assert!(!plan.abort);
    assert_eq!(plan.steps.len(), 4);
    assert_eq!(plan.steps[0].stage, FlashStage::WipeData);
    assert_eq!(plan.steps[3].stage, FlashStage::HomeCsc);
    // El paso 3 escribe AP, CP y CSC; el 2 sólo el bootloader.
    assert!(plan.steps[2].writes.iter().any(|w| w == "AP_MDPO"));
    assert!(plan.steps[2].writes.iter().any(|w| w == "CSC_MD1"));
    assert_eq!(plan.steps[1].writes, vec!["BOOTLOADER".to_string()]);
}

#[test]
fn sin_efs_verificada_el_wipe_data_queda_bloqueado() {
    let dev = device_ready();
    let rom = rom_ok();
    let efs = efs_backup_plan(&dev, "/tmp/efs"); // sin hashes
    let plan = flash_plan(&rom, &dev, &efs, true);
    assert!(plan.abort);
    assert!(plan.steps[0].blocked_reason.is_some());
    assert!(plan.steps[0].blocked_reason.as_ref().unwrap().contains("EFS"));
    // Los otros tres pasos no dependen de EFS, pero el plan entero aborta.
    assert!(!plan.blockers.is_empty());
}

#[test]
fn la_seleccion_escoge_la_rom_del_csc_real_y_explica_el_resto() {
    let dev = device_ready();
    let mut newest = rom_ok();
    newest.name = "ROM-AMO-2024-08".into();
    newest.patch_date = "2024-08-01".into();
    let mut other_csc = rom_ok();
    other_csc.name = "ROM-EUU".into();
    other_csc.csc = "EUU".into();
    let mut old = rom_ok();
    old.name = "ROM-AMO-2024-01".into();
    old.patch_date = "2024-01-01".into();

    let sel = select_rom(
        &[rom_ok(), newest, other_csc, old],
        &dev,
        &SelectionCriteria::default(),
    );
    assert_eq!(sel.chosen.unwrap().name, "ROM-AMO-2024-08");
    assert_eq!(sel.rejected.len(), 2);
    assert!(sel.rejected.iter().any(|r| r.name == "ROM-EUU"));
    assert!(sel.rejected.iter().any(|r| r.name == "ROM-AMO-2024-01"));
}

#[test]
fn el_plan_json_marca_el_dry_run_y_las_7_particiones() {
    let dev = device_ready();
    let rom = rom_ok();
    let json = plan_json(&dev, &rom, &BTreeMap::new());
    assert!(json.contains("\"dry_run\": true"));
    for p in EFS_SAFE_WORLD.iter() {
        assert!(json.contains(p), "falta {} en el plan", p);
    }
    // Las particiones que no existen en este firmware no deben aparecer.
    assert!(!json.contains("vbmeta_vendor"));
    assert!(!json.contains("init_boot"));
}

#[test]
fn el_dry_run_es_legible_y_no_afirma_haber_escrito_nada() {
    let out = dry_run_report(&device_ready(), &[rom_ok()], &BTreeMap::new());
    assert!(out.contains("DRY-RUN"));
    assert!(out.contains("worldsafe") || out.contains("mundo seguro"));
    assert!(out.contains("no se ha escrito nada"));
    assert!(out.contains("--confirm"));
}

#[test]
fn el_reinventario_compara_antes_y_despues() {
    let mut before = BTreeMap::new();
    before.insert("csc".to_string(), "AMO".to_string());
    before.insert("patch".to_string(), "2024-06-01".to_string());
    let mut after = before.clone();
    after.insert("patch".to_string(), "2024-07-01".to_string());
    after.insert("root".to_string(), "magisk-27.0".to_string());
    let diffs = compare_inventories(&before, &after);
    assert_eq!(diffs.len(), 2);
}
