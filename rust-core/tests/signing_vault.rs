//! Arnés de pruebas del módulo `signing_vault`.
//!
//! `src/lib.rs` es intocable en esta tarea (regla del plan v14: no se toca
//! `lib.rs` sin acuerdo), así que el módulo se incluye aquí directamente con
//! `#[path]`, igual que hace `tests/signing.rs`.  En cuanto se añada
//! `pub mod signing_vault;` a `src/lib.rs`, este arnés puede borrarse sin
//! tocar los tests.
//!
//! Lo que se prueba aquí es el REPLICA DE LA ZONA PÚBLICA entre dos
//! instancias, que es la parte que el bus necesita para poder replicar el
//! vault a toda la flota.

#[path = "../src/signing_vault.rs"]
mod signing_vault;

use base64ct::Encoding;
use signing_vault::{sha256_hex, SigningVault, Zone};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn temp_dir(tag: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "md_signing_vault_it_{}_{}_{}",
        tag,
        std::process::id(),
        n
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Replicación completa: una instancia "principal" publica material firmado
/// (certificados, CMS/CAdES, PDF PAdES-B-B) y una "secundaria" lo recibe,
/// verifica el sha-256 en destino y lo indexa.
#[test]
fn replica_public_zone_between_instances() {
    let primary_dir = temp_dir("primary");
    let primary = SigningVault::new(&primary_dir).unwrap();

    let cert = b"-----BEGIN CERTIFICATE-----\nMIIFakeCert\n-----END CERTIFICATE-----\n";
    let cms = b"\x30\x82\x00a0 fake-cms-cades";
    let pdf = b"%PDF-1.7 fake pades-bb";
    primary.put("cert.pem", Zone::Public, cert, "application/x-pem-file", "primary").unwrap();
    primary.put("doc.cms", Zone::Public, cms, "application/pkcs7-mime", "primary").unwrap();
    primary.put("doc.pdf", Zone::Public, pdf, "application/pdf", "primary").unwrap();

    // La zona privada NO se replica.
    primary
        .put("device.key", Zone::Private, b"CLAVE PRIVADA DEL DISPOSITIVO", "", "primary")
        .unwrap();

    let bundle = primary.export_public().unwrap();

    let replica_dir = temp_dir("replica");
    let replica = SigningVault::new(&replica_dir).unwrap();
    let report = replica.import_public(&bundle).unwrap();
    assert_eq!(report.imported, 3, "importadas: {:?}", report.errors);
    assert_eq!(report.rejected, 0);

    // El material llega integro y verificado en destino.
    let items: [(&str, &[u8]); 3] = [
        ("cert.pem", cert),
        ("doc.cms", cms),
        ("doc.pdf", pdf),
    ];
    for (name, expected) in items.iter() {
        let (_entry, data) = replica.get(name).unwrap();
        assert_eq!(&data, expected, "contenido alterado: {}", name);
        let v = replica.verify(name).unwrap();
        assert!(v.ok, "sha-256 verificado en destino: {}", name);
        assert_eq!(v.expected_sha256, sha256_hex(expected));
    }

    // La zona privada no ha llegado, y no ha salido de la principal.
    assert!(replica.get("device.key").is_err());
    let replica_list = replica.list(None).unwrap();
    assert!(!replica_list.iter().any(|e| e.name == "device.key"));
    let decoded = base64ct::Base64::decode_vec(&bundle).unwrap();
    let text = String::from_utf8(decoded).unwrap();
    assert!(!text.contains("device.key"));
    assert!(!text.contains("CLAVE PRIVADA"));

    let _ = fs::remove_dir_all(&primary_dir);
    let _ = fs::remove_dir_all(&replica_dir);
}

/// Un material corrupto en el paquete se rechaza en destino y NO se escribe.
#[test]
fn corrupt_bundle_is_rejected_at_destination() {
    let primary_dir = temp_dir("corrupt_src");
    let primary = SigningVault::new(&primary_dir).unwrap();
    primary.put("firma.p7s", Zone::Public, b"firma original", "", "primary").unwrap();
    let bundle = primary.export_public().unwrap();

    let replica_dir = temp_dir("corrupt_dst");
    let replica = SigningVault::new(&replica_dir).unwrap();
    // Se falsea el sha-256 declarado del paquete (el paquete es base64:
    // hay que decodificar, tocar el NDJSON de dentro y recodificar).
    let decoded = base64ct::Base64::decode_vec(&bundle).unwrap();
    let text = String::from_utf8(decoded).unwrap();
    let tampered_text = text.replace("\"sha256\":\"", "\"sha256\":\"0");
    assert_ne!(tampered_text, text, "el paquete debe quedar manipulado");
    let tampered = base64ct::Base64::encode_string(tampered_text.as_bytes());
    let report = replica.import_public(&tampered).unwrap();
    assert_eq!(report.imported, 0);
    assert_eq!(report.rejected, 1);
    assert!(!replica.entry_path(Zone::Public, "firma.p7s").exists());
    let _ = fs::remove_dir_all(&primary_dir);
    let _ = fs::remove_dir_all(&replica_dir);
}
