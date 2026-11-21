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

use megadaemon_core::rom_migration;

use megadaemon_core::rom_migration::{compare_inventories, dry_run_report, efs_backup_plan, flash_plan, mark_efs_verified, plan_json, preflight, select_rom, AvbState, DeviceFacts, EfsBackupPlan, EFS_SAFE_WORLD, FlashStage, RomPackage, SelectionCriteria, UnlockState, omc_decode, omc_encode, omc_decode_gzip, omc_encode_gzip, csc_feature_get, csc_features, csc_feature_set, omc_feature_patch_plan, RECORDING_FEATURE, RECORDING_VALUE_ALLOWED, csc_recording_module_files, recording_value_recommended, recording_value_has_disclaimer, RECORDING_VALUE_ALLOWED_WITH_DISCLAIMER, RECORDING_VALUE_ALLOWED_WITH_DISCLAIMER_INDIA};
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

/// **Fichero real** de `cscfeature.xml` del CSC AMO de este móvil
/// (1.109 B, sha16 `ef110140f7bb95e1`), en base64, para probar el codec
/// contra el fichero de verdad y no contra un fixture inventado.
const AMO_OMC_B64: &str = "Lycpt9rBrDfkIgVFW77n6BY4XukjC85XuCpZ17dgpR0JGNN3zHAo/t00YJ6/kbdSU51r1DvsEj3ZEiGrQZztRMFe1dDEtSkMnBVjnkArCLp+XxAVyxSL4CgrR6EJqU1mcAGYiw4his+/z8p5X7hLrK48rnh/FbpwEUpA0oS7NhQoWarm4QXKM9BxSSJYlLeMMgiYyA2qMqmZwPBXFWZTROt2TgEvUOI8rf8sYxLC3+z4OMhaPLekZ4BSjGJcvVn8C4z4VIVWUr+LOCn+1dYBaB3RkdXvlKqmmj1Ux/fBHpXU/8PKLKxWorvCLfg8Ugbf4OYoUs06awlUBtszgckL0+DZ/7qj5yHQwOSKcL9gnyxY3kIG8jEmfy/AhMdt61sO4V/8wfnB/T+zExmvW7l5tYiQJPzBiQH+MQxD696nrm+xpAs2eBgia0+9Clin5aRLARpNsgTc2zPWGROml50L/JxuCOnI9pPELEad5LXZ64eVMJ0xnVa6xkWj6iDQ7QpjLZs6LCbedy/7EgCkd1C/fW7UbHmjBkhK30nFCfyrWG3mBMmQAhHz6NNAPqy4oVtp6FStkbzk3d4Ljfc9qYjArG1DlXaK66s+J3AZde/z03XyAB7XynHAvMFBD7Fc5hqdQDLZ6ymXPvSmhpHrcBE7xlu+zRSObamvmE9TNkLVFNFDUjTaqMdukemFmYrTh+wZyjj+IvMWVH7ilYj0xfTxPAHZeYUBb6DwHYvPjG9xXtbu990Duj3zsinC/XApsVCd4bfmvxe5kVYg0f6yYLQ+78X4l/h7w7y5NHlH1Dwj+rAlkEqEKKaPyyDHH7ZBhzsD9VTjZXCnokjw+L1YqUXFO3UdB+EuuqBqDK7InNWqvjU8SAw0NRci+LIIqwABoTv+sSZk6+mblbWbv+QyIuE+to6E765Kk/K+xi3Ol5Z2PJgebVrXmM6yxm9/1w6HGBhTTHbMzI3TuYO4jh9R0XSMZcXQE+xuh4fwG7So56y4yun1deyFIZlbF69OMcajxXjDU7yRTV4XSEzm9oi8UkDLVmy08m2fbvA/9PF8Wt3bi+b59c2HPI+W0iE+jkTiMR2+iPw2VZDz0/JHVVSn4IqQQRqN019KQSUm67rwi75xFwncj63dxUkTIrXeUQAMHtF5yT3O2eiCoa/YaFgbdQqTfZrtBblbkeG7EnAiU9LzPyclEpIr8sCqE/hWOkKBIKG1VyPV7gT/rmZJiJQfajM+PTrBnVbpHihG7mDwMUVyEkrlCMtXWtiHD+BBT8zR1ZTPrPg097yNGfACp2Vq8XJo1LXH1cmoI2h4eb1Y4Nyok6xdwPBD37lqve7gEr/9lBIkeslRGs3ro/h+pnCXfYFceM2pwTTKordEsQoet8gqIRcCbRVguO0SxpzY9Q1BkKkjnZ7VtA3rBcp5+rUExb+F7X+8QWHQMs/2BHp7B2XCrreXqxCWtSoCvYm3G9Ygfken+1yLZjA=";

fn omc_amo_bytes() -> Vec<u8> {
    // decodificador de base64 mínimo (sin dependencias)
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut acc: u32 = 0;
    let mut bits = 0;
    let mut out = Vec::new();
    for ch in AMO_OMC_B64.bytes() {
        if ch == b'=' || ch == b'\n' { continue; }
        let v = T.iter().position(|&c| c == ch).expect("base64 valido") as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xFF) as u8);
        }
    }
    out
}

#[test]
fn el_codec_omc_descifra_el_fichero_real_de_amo() {
    let bytes = omc_amo_bytes();
    assert_eq!(bytes.len(), 1109);
    let xml = omc_decode_gzip(&bytes).expect("decodifica");
    assert!(xml.contains("<SalesCode>AMO</SalesCode>"), "{}", xml);
    assert!(xml.contains("<Country>SPAIN</Country>"));
    assert!(xml.contains("<CountryISO>ES</CountryISO>"));
    // AMO NO tiene la bandera de grabacion (medido en el fichero real)
    assert_eq!(csc_feature_get(&xml, RECORDING_FEATURE), None);
}

#[test]
fn el_codec_omc_es_inversible_byte_a_byte() {
    let bytes = omc_amo_bytes();
    let gz = omc_decode(&bytes);
    assert_eq!(omc_encode(&gz), bytes, "encode(decode(x)) == x");
    let xml = omc_decode_gzip(&bytes).unwrap();
    let re = omc_encode_gzip(xml.as_bytes()).unwrap();
    let xml2 = omc_decode_gzip(&re).unwrap();
    assert_eq!(xml, xml2, "el XML sobrevive al viaje de ida y vuelta");
}

#[test]
fn insertar_la_bandera_de_grabacion_en_amo_y_verificarlo() {
    let bytes = omc_amo_bytes();
    let xml = omc_decode_gzip(&bytes).unwrap();
    let nuevo = csc_feature_set(&xml, RECORDING_FEATURE, RECORDING_VALUE_ALLOWED).unwrap();
    assert!(nuevo.contains(&format!(
        "<{f}>{v}</{f}>",
        f = RECORDING_FEATURE,
        v = RECORDING_VALUE_ALLOWED
    )));
    let re = omc_encode_gzip(nuevo.as_bytes()).unwrap();
    let leido = omc_decode_gzip(&re).unwrap();
    assert_eq!(
        csc_feature_get(&leido, RECORDING_FEATURE),
        Some(RECORDING_VALUE_ALLOWED.to_string())
    );
    // y el resto del XML intacto
    assert!(leido.contains("<SalesCode>AMO</SalesCode>"));
    assert!(leido.contains("<Country>SPAIN</Country>"));
}

#[test]
fn el_plan_de_parche_avisa_del_cambio_de_tamano() {
    let bytes = omc_amo_bytes();
    let plan = omc_feature_patch_plan("AMO", &bytes, RECORDING_FEATURE, RECORDING_VALUE_ALLOWED)
        .unwrap();
    assert_eq!(plan.csc, "AMO");
    assert!(plan.path.ends_with("/conf/system/cscfeature.xml"));
    assert_ne!(plan.omc_before, plan.omc_after, "el tamano cambia => hay que actualizar contents.db");
    assert!(plan.steps.iter().any(|s| s.contains("contents.db")));
    assert!(plan.revert.iter().any(|s| s.contains("Restaurar")));
}

#[test]
fn el_modulo_magisk_hace_bind_mount_del_cscfeature_parcheado() {
    let files = csc_recording_module_files("AMO");
    let prop = files.iter().find(|f| f.path == "module.prop").unwrap();
    assert!(prop.content.contains("megadaemon_csc_recording"));
    assert!(prop.content.contains(RECORDING_FEATURE));
    assert!(prop.content.contains(RECORDING_VALUE_ALLOWED));
    assert_eq!(prop.mode, 0o644);
    let script = files.iter().find(|f| f.path == "post-fs-data.sh").unwrap();
    assert!(script.content.contains("/optics/configs/carriers/single/AMO/conf/system/cscfeature.xml"));
    assert!(script.content.contains("/prism/etc/carriers/single/AMO/contents.db"));
    assert!(script.content.contains("mount --bind"));
    assert_eq!(script.mode, 0o755);
}

#[test]
fn el_valor_de_grabacion_recomendado_avisa_a_la_otra_parte() {
    // En Espana grabar en silencio a un tercero es delito (art. 197 CP):
    // el valor por defecto tiene que llevar el disclaimer.
    assert_eq!(
        recording_value_recommended(),
        "RecordingAllowedByMenu,disclaimer_latin"
    );
    assert!(recording_value_has_disclaimer(recording_value_recommended()));
    assert!(recording_value_has_disclaimer(
        RECORDING_VALUE_ALLOWED_WITH_DISCLAIMER_INDIA
    ));
    assert!(!recording_value_has_disclaimer(RECORDING_VALUE_ALLOWED));
    // el modulo Magisk generado usa el valor con aviso
    let files = csc_recording_module_files("AMO");
    let desc = files
        .iter()
        .find(|f| f.path == "module.prop")
        .expect("module.prop");
    assert!(desc.content.contains("disclaimer_latin"), "{}", desc.content);
    assert!(!desc.content.contains("=RecordingAllowed."), "{}", desc.content);
}
