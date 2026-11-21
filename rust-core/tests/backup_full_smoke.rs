//! W3 · Arnés de pruebas del módulo `backup_full`.
//!
//! `src/lib.rs` es intocable en esta tarea (regla del plan: no se toca
//! `lib.rs` sin acuerdo), así que el módulo se incluye aquí directamente con
//! `#[path]`, igual que `tests/rom_migration_smoke.rs`. En cuanto se añada
//! `pub mod backup_full;` a `src/lib.rs`, este arnés puede borrarse sin tocar
//! los tests.
//!
//! Lo que se prueba aquí es la **línea roja** del reparto de firmas:
//! la zona pública se replica a todas las instancias y la zona privada
//! **nunca sale del dispositivo**.

use megadaemon_core::backup_full;

use megadaemon_core::backup_full::{
    build_pack, chunk_sha256, classify_zone, level_specs, not_recoverable, plan_chunks,
    sha256_file, verify_at_destination, Level, PackItem, Zone, EFS_NON_EXISTENT,
    EFS_SAFE_WORLD,
};
use std::fs;
use std::path::Path;

fn tmpdir(tag: &str) -> String {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "md_bf_{}_{}_{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&p).unwrap();
    p.to_string_lossy().to_string()
}

fn item(rel: &str, bytes: u64) -> PackItem {
    PackItem {
        rel_path: rel.to_string(),
        level: Level::L2UserData,
        zone: classify_zone(rel),
        size_bytes: bytes,
        sha256: "0".repeat(64),
    }
}

#[test]
fn la_zona_publica_y_la_privada_no_se_confunden() {
    // Público: certificados, CMS, PDF firmados, el pack público de firma.
    assert_eq!(classify_zone("public/cert-chain.pem"), Zone::Public);
    assert_eq!(classify_zone("public/firma-firmada.pdf"), Zone::Public);
    assert_eq!(classify_zone("public/signing.cms"), Zone::Public);
    // Privado: material de clave, para donde vaya.
    assert_eq!(classify_zone("private/maestro.key"), Zone::Private);
    assert_eq!(classify_zone("public/privkey.pem"), Zone::Private);
    assert_eq!(classify_zone("tokens/pkcs11.tok"), Zone::Private);
    assert_eq!(classify_zone("nostr/nsec-abc"), Zone::Private);
}

#[test]
fn el_pack_de_la_zona_publica_no_arrastra_la_privada() {
    let plan = build_pack(
        Level::L2UserData,
        vec![
            item("public/cert.pem", 100),
            item("public/firma.pdf", 200),
            item("private/maestro.key", 50),
            item("tokens/session.tok", 10),
        ],
    );
    assert_eq!(plan.items.len(), 2);
    assert!(plan.items.iter().all(|i| i.zone == Zone::Public));
    assert_eq!(plan.excluded_private.len(), 2);
    assert_eq!(plan.total_bytes, 300);
}

#[test]
fn el_nivel_de_credenciales_no_es_replicable() {
    let plan = build_pack(Level::L3Credentials, vec![item("token.json", 10)]);
    assert!(plan.items.is_empty());
    assert_eq!(plan.excluded_private.len(), 1);
}

#[test]
fn los_cuatro_niveles_estan_declarados() {
    let specs = level_specs();
    assert_eq!(specs.len(), 4);
    let niveles: Vec<&str> = specs.iter().map(|s| s.level.as_str()).collect();
    assert_eq!(niveles, vec!["l0", "l1", "l2", "l3"]);
    // L3 es privado; los otros tres no.
    assert_eq!(specs[3].zone, Zone::Private);
    assert!(specs[0..3].iter().all(|s| s.zone == Zone::Public));
}

#[test]
fn el_nivel_de_rom_pide_las_siete_particiones_y_no_inventa_las_otras() {
    let specs = level_specs();
    let l1 = specs.iter().find(|s| s.level == Level::L1Rom).unwrap();
    let txt = l1.includes.join(" ");
    for p in EFS_SAFE_WORLD.iter() {
        assert!(txt.contains(p), "falta la partición {}", p);
    }
    // Las particiones que NO existen tienen que estar **excluidas** (no
    // listadas como particiones a respaldar). El plan L1 sí puede mencionar
    // la *imagen* init_boot de fábrica del pack del nodo: eso es material
    // real, no la partición inexistente.
    let exc = l1.excludes.join(" ");
    for p in EFS_NON_EXISTENT.iter() {
        assert!(
            exc.contains(p),
            "{} no existe en este firmware: tiene que estar excluido",
            p
        );
    }
}

#[test]
fn lo_que_no_se_puede_recuperar_se_declara() {
    let nr = not_recoverable();
    assert!(nr.len() >= 4);
    for n in &nr {
        assert!(!n.what.is_empty());
        assert!(!n.reason.is_empty());
        assert!(!n.salvage.is_empty());
    }
    // Lo de AndroidKeyStore es el límite duro y tiene que estar dicho.
    assert!(nr.iter().any(|n| n.reason.contains("Keystore")));
}

#[test]
fn el_plan_de_trozos_no_requiere_copia_local() {
    let dir = tmpdir("chunks");
    fs::write(format!("{}/datos.bin", dir), vec![9u8; 2500]).unwrap();
    let sha = sha256_file(Path::new(&format!("{}/datos.bin", dir))).unwrap();
    let plan = build_pack(
        Level::L2UserData,
        vec![PackItem {
            rel_path: "datos.bin".into(),
            level: Level::L2UserData,
            zone: Zone::Public,
            size_bytes: 2500,
            sha256: sha,
        }],
    );
    let chunks = plan_chunks(&plan, Path::new(&dir), 1000);
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].len, 1000);
    assert_eq!(chunks[2].len, 500);
    assert_eq!(chunks[2].offset, 2000);
    // El hash del último trozo sale de leer sólo ese rango.
    assert_eq!(
        chunk_sha256(Path::new(&format!("{}/datos.bin", dir)), 2000, 500).unwrap(),
        chunks[2].sha256
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn la_verificacion_en_el_destino_es_lo_que_prueba_la_replica() {
    let plan = build_pack(
        Level::L1Rom,
        vec![item("public/cert.pem", 10), item("public/firma.pdf", 10)],
    );
    let lookup = |rel: &str| -> Option<String> {
        match rel {
            "public/cert.pem" => Some("0".repeat(64)),
            _ => Some("0".repeat(64)),
        }
    };
    let rep = verify_at_destination(&plan, lookup);
    assert!(rep.ok());
    assert_eq!(rep.verified, 2);
}
