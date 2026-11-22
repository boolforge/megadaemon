//! `kernel_backup` — arnés de humo del módulo de respaldo del kernel.
//!
//! El módulo `rust-core/src/kernel_and_blocks.rs` es **autocontenido**
//! (no usa `crate::`), así que se puede compilar **aislado** y probar sus
//! funciones puras sin que el resto del crate compile. Eso es lo que hace
//! este test: incluye el fichero por ruta y ejercita lo que NO necesita
//! root ni red.
//!
//! Lo que se comprueba aquí:
//!   * el contrato de la tool MCP (`kernel_backup`) y sus acciones;
//!   * el plan de rollback: **nunca** se ejecuta y filtra las particiones
//!     ausentes;
//!   * la validación fail-closed (nombres de partición y rutas de destino);
//!   * el formato de los metadatos y de las cabeceras de la subida a Drive;
//!   * el hash de un fichero real y la lectura de un manifiesto real.
//!
//! Lo que NO se comprueba aquí (necesita el móvil): el volcado real, el
//! `dd`, el hash de una partición viva y la subida de verdad a Drive.
//! Eso se mide en el dispositivo, con la tool `kernel_backup`.

use serde_json::json;

/// Copia aislada del módulo real (sin `crate::`).
#[path = "../src/kernel_and_blocks.rs"]
mod kb;

#[test]
fn la_tool_declara_todas_las_acciones_del_contrato() {
    let def = kb::tool_definition();
    assert_eq!(def["name"], json!("kernel_backup"));
    let acciones = def["inputSchema"]["properties"]["action"]["enum"]
        .as_array()
        .unwrap()
        .clone();
    for esperada in [
        "info",
        "dmesg",
        "partitions",
        "processes",
        "fingerprint",
        "backup",
        "verify",
        "upload_drive",
        "rollback_plan",
    ] {
        assert!(
            acciones.contains(&json!(esperada)),
            "falta la acción {} en el contrato",
            esperada
        );
    }
}

#[test]
fn la_tool_no_acepta_una_accion_inventada() {
    let r = kb::tool_dispatch(&json!({ "action": "flashear_kernel" }));
    assert_eq!(r["ok"], json!(false));
    assert!(r["error"].as_str().unwrap().contains("no soportada"));
}

#[test]
fn el_plan_de_rollback_no_se_ejecuta_y_filtra_lo_ausente() {
    let dir = std::env::temp_dir().join("md_kernel_smoke_rollback");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("manifest.json");
    std::fs::write(
        &manifest,
        json!({
            "schema": kb::MANIFEST_SCHEMA,
            "created_at": "2026-11-22T13:40:39Z",
            "device": { "model": "SM-G998B", "soc": "Exynos 2100" },
            "entries": [
                {
                    "partition": "boot",
                    "file": "/data/local/tmp/kernel-backup/boot.img",
                    "sha256": "f96cd410e6f048739fe36737f047e2ede6d229d428e1d0c74de80017c8beb4fd"
                },
                {
                    "partition": "init_boot",
                    "file": "/data/local/tmp/kernel-backup/init_boot.img",
                    "sha256": "",
                    "ausente": true
                }
            ]
        })
        .to_string(),
    )
    .unwrap();

    let plan = kb::rollback_plan(manifest.to_str().unwrap());
    assert_eq!(plan["ok"], json!(true));
    assert_eq!(plan["executed"], json!(false), "el plan NUNCA se ejecuta solo");
    let pasos = plan["steps"].as_array().unwrap();
    assert_eq!(pasos.len(), 1, "init_boot ausente no genera paso de escritura");
    assert_eq!(pasos[0]["partition"], json!("boot"));
    let script = plan["script"].as_str().unwrap();
    assert!(script.contains("ESCRIBE en la partición"));
    assert!(script.contains("boot.img"));
    assert!(!script.contains("init_boot"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn la_validacion_es_fail_closed() {
    assert!(kb::validate_partition("boot").is_ok());
    assert!(kb::validate_partition("boot; reboot").is_err());
    assert!(kb::validate_partition("../boot").is_err());
    assert!(kb::validate_dst("/data/local/tmp/kernel-backup").is_ok());
    assert!(kb::validate_dst("/dev/block/by-name/boot").is_err());
    assert!(kb::validate_dst("relative/path").is_err());
    assert!(kb::validate_dst("/data/../../etc").is_err());
}

#[test]
fn el_volcado_usa_el_orden_estandar_de_particiones() {
    // `init_boot` se pide aunque no exista en Samsung: el volcado lo marca
    // como ausente en vez de fallar.
    assert!(kb::DEFAULT_BOOT_PARTITIONS.contains(&"boot"));
    assert!(kb::DEFAULT_BOOT_PARTITIONS.contains(&"vendor_boot"));
    assert!(kb::DEFAULT_BOOT_PARTITIONS.contains(&"recovery"));
    assert!(kb::DEFAULT_BOOT_PARTITIONS.contains(&"init_boot"));
}

#[test]
fn la_subida_a_drive_lleva_sha256_y_trozos_validos() {
    let cuerpo = kb::drive_metadata_body("kernel-backup-boot.img", "folder", "abc123");
    let v: serde_json::Value = serde_json::from_str(&cuerpo).unwrap();
    assert_eq!(v["name"], json!("kernel-backup-boot.img"));
    assert_eq!(v["parents"][0], json!("folder"));
    assert_eq!(v["appProperties"]["sha256"], json!("abc123"));

    assert_eq!(kb::drive_content_range(0, 8 * 1024 * 1024, 16 * 1024 * 1024), "bytes 0-8388607/16777216");
    assert_eq!(kb::drive_chunk_len(20 * 1024 * 1024, 16 * 1024 * 1024), 4 * 1024 * 1024);
    assert_eq!(kb::drive_chunk_len(20 * 1024 * 1024, 20 * 1024 * 1024), 0);
}

#[test]
fn el_manifiesto_se_lee_y_se_verifica() {
    let dir = std::env::temp_dir().join("md_kernel_smoke_manifest");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let img = dir.join("boot.img");
    std::fs::write(&img, b"contenido de prueba").unwrap();
    let sha = kb::sha256_file(&img).unwrap();
    let manifest = dir.join("manifest.json");
    std::fs::write(
        &manifest,
        json!({
            "schema": kb::MANIFEST_SCHEMA,
            "created_at": "2026-11-22T13:40:39Z",
            "entries": [
                { "partition": "boot", "file": img.to_str().unwrap(), "sha256": sha }
            ]
        })
        .to_string(),
    )
    .unwrap();

    let v = kb::verify_manifest(manifest.to_str().unwrap(), false);
    assert_eq!(v["ok"], json!(true), "el volcado debe cuadrar: {:?}", v);
    let checks = v["checks"].as_array().unwrap();
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0]["file_ok"], json!(true));

    // Si el fichero se corrompe, la verificación falla (fail-closed).
    std::fs::write(&img, b"otro contenido").unwrap();
    let v2 = kb::verify_manifest(manifest.to_str().unwrap(), false);
    assert_eq!(v2["ok"], json!(false));
    assert!(v2["note"].as_str().unwrap().contains("NO es un punto de retorno"));
    let _ = std::fs::remove_dir_all(&dir);
}
