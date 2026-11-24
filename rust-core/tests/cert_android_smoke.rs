//! Arnés de pruebas del backup de certificados desde Android (`cert_android`).
//!
//! Determinista: el certificado está **congelado** en DER (el mismo que usa
//! `cert_vault_smoke.rs`) y el "dispositivo" es un árbol de ficheros sintético
//! que se crea en un directorio temporal.

#[path = "../src/aegis.rs"]
#[allow(dead_code, unused_imports, unused_mut)]
mod aegis;
#[path = "../src/argon2.rs"]
#[allow(dead_code, unused_imports, unused_mut)]
mod argon2;
#[path = "../src/backup_vault.rs"]
#[allow(dead_code, unused_imports, unused_mut)]
mod backup_vault;
#[path = "../src/cert_vault.rs"]
#[allow(dead_code, unused_imports, unused_mut)]
mod cert_vault;
#[path = "../src/cert_android.rs"]
#[allow(dead_code, unused_imports, unused_mut)]
mod cert_android;

use cert_android::*;
use cert_vault::*;
use std::fs;
use std::path::{Path, PathBuf};

/// Certificado de prueba congelado (DER, 845 bytes): CN="MD Test Persona
/// Fisica", C=ES, O=MEGADAEMON TEST ONLY, notAfter 2040-01-01.
const CERT_DER_HEX: &str = concat!(
    "30820349308202310214025cc1b5163119d1558733789deb48b3c47f454c300d06092a864886f70d01010b0500306131",
    "1f301d06035504030c164d44205465737320506572736f6e6120466973696361310b3009060355040613024553311d30",
    "1b060355040a0c144d4547414441454d4f4e2054455354204f4e4c59311230100603550405130931323334353637385a",
    "301e170d3230303130313030303030305a170d3430303130313030303030305a3061311f301d06035504030c164d4420",
    "5465737420506572736f6e6120466973696361310b3009060355040613024553311d301b060355040a0c144d45474144",
    "41454d4f4e2054455354204f4e4c59311230100603550405130931323334353637385a30820122300d06092a864886f7",
    "0d01010105000382010f003082010a0282010100aba36d6c802b754d8095a45c8d8ddfe10a332be0e44ed9c4457bef96",
    "cfce56718322f2d62080c0125d107d81bc919ed488e782629bc6492b6538882c15b1e2498f66b114a602d8d776a6bcf4",
    "3b1364bb0ca323a52678c3fcf1fe74465ff14484196c67d4c54e9ae89f9ec532974fdfe08425e8d5ed620a3c8253395c",
    "cc8d5b06b2afa2b977cbf13f9ece8bd99f4f3456df0b87462ef27323d48a761cbe911f1c60229e500a2b0a9730e27dff",
    "73e0de2a9f31277903a94aa26a7a311e61cb103a669bd42a800517da4e37e12d5bc887c6ddf75e2122502ec75c39cd23",
    "35e29635c77f60ac807fc0537dbf0ec04b71e9fe732e435f411828eaa4e32c573a8dfa150203010001300d06092a8648",
    "86f70d01010b050003820101001966c4f5ac75be337e09fe417662f44734ac661beff1bc035731309e6803306224fac2",
    "2fa7b9c9361958dd4fa159ea8e6053db4b144d528232d3cfda5f6c29a7a054e7a0b2f9e881db67dbf3ad6843856dec05",
    "8faa4c90471d14f1c5bb0bdb9d200ef097e621a26def1d94c52123c1309df69bc06cbcfc23b8f330ea1e5cca8cb39b3a",
    "fa9eaffef0bc52ed398f658bf9225fb21e5f89784301eb011c81e8a16865ec328848f749f3e06e0e95ce1b0540edc0d8",
    "48fdfb5b4910bf5d37ab26d168c6491ea13a8a6e226ca78c1f18433f19a1691bcb64639a2d91f767e559761e6d8d0ace",
    "a30c7cb3ecd9047c2815c9393812d32d8eb7caa86ac5e2dc0c71d8b896",
);

/// Clave privada **sintética** (NO es una clave real ni un secreto).
const CLAVE_SINTETICA: &[u8] = b"MDAEMON-PRUEBA-CLAVE-PRIVADA-NO-ES-UNA-CLAVE-REAL";

fn hex_to_bytes(h: &str) -> Vec<u8> {
    let h: String = h.chars().filter(|c| !c.is_whitespace()).collect();
    (0..h.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap())
        .collect()
}

fn cert_der() -> Vec<u8> {
    hex_to_bytes(CERT_DER_HEX)
}

/// PEM del certificado de prueba.
fn cert_pem() -> String {
    let b64 = base64ct::Base64::encode_string(&cert_der());
    let mut out = String::from("-----BEGIN CERTIFICATE-----\n");
    let mut i = 0;
    while i < b64.len() {
        let end = std::cmp::min(i + 64, b64.len());
        out.push_str(&b64[i..end]);
        out.push('\n');
        i = end;
    }
    out.push_str("-----END CERTIFICATE-----\n");
    out
}

/// PEM de "clave privada" sintética (nunca se debe leer ni copiar).
fn key_pem() -> String {
    let b64 = base64ct::Base64::encode_string(CLAVE_SINTETICA);
    format!(
        "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
        b64
    )
}

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "md_certandroid_test_{}_{}",
        tag,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// Monta un "dispositivo" sintético con las cuatro fuentes.
fn fake_device(root: &Path) -> Vec<AndroidCertSource> {
    let sys = root.join("system_ca");
    let user = root.join("user_ca");
    let kc = root.join("keychain");
    let ks = root.join("keystore");
    for d in [&sys, &user, &kc, &ks] {
        fs::create_dir_all(d).unwrap();
    }
    // CA del sistema: dos ficheros PEM (mismo cert ⇒ un solo inventario).
    fs::write(sys.join("system-ca-1.pem"), cert_pem()).unwrap();
    fs::write(sys.join("system-ca-2.pem"), cert_pem()).unwrap();
    // CA del usuario: PEM.
    fs::write(user.join("user-added.pem"), cert_pem()).unwrap();
    // Keychain: un DER público, una clave privada PEM y un .p12.
    fs::write(kc.join("user.der"), cert_der()).unwrap();
    fs::write(kc.join("user.key.pem"), key_pem()).unwrap();
    fs::write(kc.join("user.p12"), b"\x30\x82PKCS12-SINTETICO").unwrap();
    // Keystore: alias (sólo nombres).
    fs::write(ks.join("USRSKEY_alias1"), b"blob cifrado").unwrap();
    fs::write(ks.join("USRPKEY_alias1"), b"publico").unwrap();

    vec![
        AndroidCertSource {
            label: "system_ca".into(),
            dir: sys.display().to_string(),
            backend: BACKEND_KEYSTORE.into(),
            private: false,
        },
        AndroidCertSource {
            label: "user_ca".into(),
            dir: user.display().to_string(),
            backend: BACKEND_KEYSTORE.into(),
            private: false,
        },
        AndroidCertSource {
            label: "keychain".into(),
            dir: kc.display().to_string(),
            backend: BACKEND_KEYCHAIN.into(),
            private: true,
        },
        AndroidCertSource {
            label: "keystore".into(),
            dir: ks.display().to_string(),
            backend: BACKEND_KEYSTORE.into(),
            private: true,
        },
    ]
}

#[test]
fn el_escaneo_encuentra_los_certificados_publicos() {
    let dir = tmp_dir("scan");
    let sources = fake_device(&dir);
    let scan = scan_certificates(&sources).unwrap();

    // Los tres ficheros con el mismo certificado ⇒ **una** entrada (dedup).
    assert_eq!(scan.cert_count(), 1, "certs: {:?}", scan.certs);
    let c = &scan.certs[0];
    assert_eq!(c.der, cert_der());
    assert_eq!(c.sha256, sha256_hex(&cert_der()));
    assert!(c.subject.contains("CN=MD Test Persona Fisica"), "{}", c.subject);
    assert!(c.self_signed, "el cert de prueba es autofirmado");
    assert_eq!(c.not_after, 2_208_988_800);
    assert!(c.id.starts_with("android-"), "{}", c.id);

    // La fuente del keychain (fichero DER) también entra.
    assert!(scan.certs.iter().all(|c| c.der == cert_der()));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn el_material_privado_se_lista_pero_no_se_lee() {
    let dir = tmp_dir("priv");
    let sources = fake_device(&dir);
    let scan = scan_certificates(&sources).unwrap();

    // El `.p12` y el PEM de clave privada quedan como REFERENCIA.
    let paths: Vec<&str> = scan.private_refs.iter().map(|p| p.path.as_str()).collect();
    assert!(
        paths.iter().any(|p| p.ends_with("user.p12")),
        "el .p12 debe listarse: {:?}",
        paths
    );
    assert!(
        paths.iter().any(|p| p.ends_with("user.key.pem")),
        "el PEM de clave debe listarse: {:?}",
        paths
    );
    assert!(scan.has_private_refs());

    // Y **no** hay ningún certificado con material de clave.
    assert!(scan.certs.iter().all(|c| c.der == cert_der()));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn la_importacion_deja_solo_referencias_y_sincroniza_el_parte_publico() {
    let dir = tmp_dir("import");
    let sources = fake_device(&dir);
    let scan = scan_certificates(&sources).unwrap();

    let vdir = tmp_dir("import-vault");
    let vault = CertVault::new(&vdir).unwrap();
    let rep = import_into_vault(&vault, &scan, KdfParams::fast()).unwrap();
    assert_eq!(rep.imported, 1, "errores: {:?}", rep.errors);
    assert_eq!(rep.rejected, 0);
    assert_eq!(rep.private_refs, 2, "las dos referencias de material privado");

    // La bóveda tiene el certificado y **ningún** fichero de clave.
    let entry = vault.get(&scan.certs[0].id).unwrap();
    assert!(!entry.key.is_exportable(), "la clave del móvil no es exportable");
    match &entry.key {
        PrivateKeyRef::NonExportable { backend, handle } => {
            assert!(backend == BACKEND_KEYSTORE || backend == BACKEND_KEYCHAIN);
            assert!(!handle.is_empty());
        }
        other => panic!("se esperaba una referencia no exportable: {:?}", other),
    }
    let keys = vault.root().join("keys");
    let n_keys = fs::read_dir(&keys).unwrap().count();
    assert_eq!(n_keys, 0, "no puede haber ningún blob de clave: {:?}", n_keys);

    // El manifiesto lo dice: la clave es **no exportable**.
    let m = vault.build_manifest().unwrap();
    assert!(m
        .entries
        .iter()
        .all(|e| matches!(e.key, ManifestKey::NonExportable { .. })));

    // El paquete de sincronización lleva el certificado **público** y **nada**
    // de material de clave.
    let mut buf: Vec<u8> = Vec::new();
    vault
        .write_sync_bundle(&mut buf, "pc-casa", SyncOptions::default())
        .unwrap();
    assert!(!buf
        .windows(CLAVE_SINTETICA.len())
        .any(|w| w == CLAVE_SINTETICA));
    let mut cursor = std::io::Cursor::new(buf);
    let vdir2 = tmp_dir("import-vault2");
    let v2 = CertVault::new(&vdir2).unwrap();
    let r = v2.read_sync_bundle(&mut cursor).unwrap();
    assert_eq!(r.imported, 1);
    assert!(v2.get(&scan.certs[0].id).unwrap().key.is_exportable() == false);

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&vdir);
    let _ = fs::remove_dir_all(&vdir2);
}

#[test]
fn la_importacion_es_idempotente() {
    let dir = tmp_dir("idem");
    let sources = fake_device(&dir);
    let scan = scan_certificates(&sources).unwrap();

    let vdir = tmp_dir("idem-vault");
    let vault = CertVault::new(&vdir).unwrap();
    let r1 = import_into_vault(&vault, &scan, KdfParams::fast()).unwrap();
    assert_eq!(r1.imported, 1);
    let r2 = import_into_vault(&vault, &scan, KdfParams::fast()).unwrap();
    assert_eq!(r2.imported, 0, "la segunda vez no debe importar nada");
    assert_eq!(r2.unchanged, 1);
    assert_eq!(vault.list().unwrap().len(), 1);

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&vdir);
}

#[test]
fn una_fuente_inexistente_no_es_error() {
    let dir = tmp_dir("missing");
    let sources = vec![AndroidCertSource {
        label: "system_ca".into(),
        dir: dir.join("no-existe").display().to_string(),
        backend: BACKEND_KEYSTORE.into(),
        private: false,
    }];
    let scan = scan_certificates(&sources).unwrap();
    assert_eq!(scan.cert_count(), 0);
    assert_eq!(scan.missing_sources.len(), 1);
    assert!(scan.missing_sources[0].ends_with("no-existe"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn un_fichero_que_no_es_certificado_se_descarta() {
    let dir = tmp_dir("basura");
    let d = dir.join("system_ca");
    fs::create_dir_all(&d).unwrap();
    fs::write(d.join("basura.pem"), "esto no es un certificado\n").unwrap();
    fs::write(d.join("basura.der"), b"\x00\x01\x02\x03").unwrap();
    let sources = vec![AndroidCertSource {
        label: "system_ca".into(),
        dir: d.display().to_string(),
        backend: BACKEND_KEYSTORE.into(),
        private: false,
    }];
    let scan = scan_certificates(&sources).unwrap();
    assert_eq!(scan.cert_count(), 0);
    assert_eq!(scan.skipped.len(), 2, "skipped: {:?}", scan.skipped);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn los_aliases_del_keystore_son_solo_nombres() {
    let dir = tmp_dir("alias");
    let sources = fake_device(&dir);
    let ks = dir.join("keystore");
    let aliases = keystore_aliases(&ks.display().to_string()).unwrap();
    assert_eq!(aliases, vec!["alias1".to_string()]);

    let _ = fs::remove_dir_all(&dir);
}
