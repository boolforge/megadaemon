//! Arnés de pruebas de la bóveda de certificados (`cert_vault`).
//!
//! Los tests son **deterministas**: el certificado está congelado en DER
//! (generado con `openssl` una vez y embebido aquí), las fechas se comprueban
//! con un instante **inyectado** y el KDF usa el coste mínimo (`KdfParams::fast`)
//! para que la suite no tarde minutos.
//!
//! El módulo se compila **autocontenido** (mismo patrón que
//! `tests/argon2_smoke.rs`): `cert_vault` sólo depende de `crate::argon2` y
//! `crate::aegis` de la biblioteca.

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

use cert_vault::*;
use std::fs;
use std::path::Path;

/// Certificado de prueba **congelado** (DER, 845 bytes).
///
/// Autofirmado, CN="MD Test Persona Fisica", C=ES, O=MEGADAEMON TEST ONLY.
/// `notBefore` = 2020-01-01T00:00:00Z, `notAfter` = 2040-01-01T00:00:00Z.
/// Generado con `openssl` y congelado aquí: el test **no** depende de
/// `openssl` en tiempo de ejecución.
const CERT_DER_HEX: &str = concat!(
    "30820349308202310214025cc1b5163119d1558733789deb48b3c47f454c300d06092a864886f70d01010b0500306131",
    "1f301d06035504030c164d44205465737420506572736f6e6120466973696361310b3009060355040613024553311d30",
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

/// Material de clave **sintético** para las pruebas.
///
/// No es una clave real (ni de prueba real): es un patrón de bytes para
/// comprobar que el cifrado funciona y que **no aparece en claro** en disco.
/// No hay ningún secreto en el repo.
const CLAVE_SINTETICA: &[u8] = b"MDAEMON-PRUEBA-MATERIAL-CLAVE-NO-ES-UNA-CLAVE-REAL";

/// Frase de paso de las pruebas (no es un secreto: es una constante de test).
const PASS: &str = "clave-de-prueba-local";

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

/// Directorio temporal propio de cada test.
fn tmp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "md_certvault_test_{}_{}_{}",
        tag,
        std::process::id(),
        hex(&now().to_le_bytes())
    ));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// Bytes aleatorios deterministas (para nombres de directorio sin colisiones).
fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

/// Vault con un certificado exportable y uno no exportable.
fn vault_con_dos(dir: &Path) -> CertVault {
    let v = CertVault::new(dir).unwrap();
    v.add_certificate(
        "fnmt",
        "FNMT persona fisica",
        &cert_der(),
        KeyMaterial::Exportable {
            backend: "archivo".to_string(),
            key_bytes: CLAVE_SINTETICA.to_vec(),
        },
        Some(PASS),
        KdfParams::fast(),
    )
    .unwrap();
    v.add_certificate(
        "dnie",
        "DNI electronico (chip)",
        &cert_der(),
        KeyMaterial::NonExportable {
            backend: "android_keystore".to_string(),
            handle: "alias-dnie-1".to_string(),
        },
        None,
        KdfParams::fast(),
    )
    .unwrap();
    v
}

// ---------------------------------------------------------------------------
// Parseo del DER
// ---------------------------------------------------------------------------

#[test]
fn el_der_se_parsea_sin_depender_de_ningun_crate() {
    let p = parse_certificate(&cert_der()).unwrap();
    assert_eq!(p.not_before, 1_577_836_800, "2020-01-01T00:00:00Z");
    assert_eq!(p.not_after, 2_208_988_800, "2040-01-01T00:00:00Z");
    assert!(
        p.subject.contains("CN=MD Test Persona Fisica"),
        "{}",
        p.subject
    );
    assert!(p.subject.contains("C=ES"), "{}", p.subject);
    assert!(
        p.issuer.contains("CN=MD Test Persona Fisica"),
        "{}",
        p.issuer
    );
    assert_eq!(p.signature_algorithm, "sha256WithRSAEncryption");
    assert_eq!(p.public_key_algorithm, "rsaEncryption");
    assert!(!p.serial_number.is_empty());
}

#[test]
fn un_der_corrupto_se_rechaza_sin_panicar() {
    assert!(parse_certificate(b"no soy un der").is_err());
    assert!(parse_certificate(&cert_der()[..40]).is_err());
    let mut roto = cert_der();
    let n = roto.len();
    roto[n - 1] ^= 0xff;
    // El último byte es de la firma: el resto del certificado debe seguir
    // parseando (no se valida la firma, sólo la estructura).
    assert!(parse_certificate(&roto).is_ok());
}

#[test]
fn el_estado_de_caducidad_se_calcula_desde_el_der() {
    let p = parse_certificate(&cert_der()).unwrap();
    // 10 días antes de caducar => "expiring"
    let diez_dias_antes = p.not_after - 10 * 86_400;
    assert_eq!(expiry_status(p.not_after, diez_dias_antes, 30), "expiring");
    // Un día después de caducar => "expired"
    assert_eq!(
        expiry_status(p.not_after, p.not_after + 86_400, 30),
        "expired"
    );
    // Con margen de aviso de 30 días, dentro del mes => "expiring"
    assert_eq!(
        expiry_status(p.not_after, p.not_after - 20 * 86_400, 30),
        "expiring"
    );
    // Lejos de la caducidad => "valid"
    assert_eq!(expiry_status(p.not_after, p.not_before, 30), "valid");
}

// ---------------------------------------------------------------------------
// Alta y clave cifrada
// ---------------------------------------------------------------------------

#[test]
fn el_fichero_en_disco_no_contiene_la_clave_en_claro() {
    let dir = tmp_dir("claro");
    let v = vault_con_dos(&dir);

    // El material de la clave NO puede aparecer en ningún fichero de la bóveda.
    let mut offenders: Vec<String> = Vec::new();
    for entry in walk(&v.root()) {
        let data = fs::read(&entry).unwrap();
        if data
            .windows(CLAVE_SINTETICA.len())
            .any(|w| w == CLAVE_SINTETICA)
        {
            offenders.push(entry.display().to_string());
        }
        // Tampoco la frase de paso.
        if data.windows(PASS.len()).any(|w| w == PASS.as_bytes()) {
            offenders.push(entry.display().to_string());
        }
    }
    assert!(
        offenders.is_empty(),
        "material en claro en: {:?}",
        offenders
    );

    // Y el blob cifrado está donde toca, con 0600.
    let key_file = v.root().join("keys").join("fnmt.key");
    assert!(key_file.exists(), "el blob cifrado debe existir");
    assert_eq!(
        mode_of(&key_file) & 0o777,
        0o600,
        "la clave cifrada va con 0600"
    );
    assert_eq!(mode_of(&v.root()) & 0o777, 0o700, "la bóveda va con 0700");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn round_trip_de_exportar_e_importar_con_clave_cifrada() {
    let dir = tmp_dir("roundtrip");
    let v = vault_con_dos(&dir);

    // Exportar exige `confirm: true` para la clave privada.
    let bundle = v
        .export_certificate("fnmt", PASS, true, KdfParams::fast())
        .expect("la exportación con confirm debe funcionar");

    // El paquete NO lleva la clave en claro (lleva el material cifrado).
    assert!(
        !bundle
            .windows(CLAVE_SINTETICA.len())
            .any(|w| w == CLAVE_SINTETICA),
        "el paquete no puede llevar la clave en claro"
    );

    // Importar en otra bóveda (otro "nodo").
    let dir2 = tmp_dir("roundtrip2");
    let v2 = CertVault::new(&dir2).unwrap();
    let entry = v2
        .import_certificate(&bundle, Some(PASS), true)
        .expect("la importación debe funcionar");
    assert_eq!(entry.id, "fnmt");
    assert_eq!(entry.subject, v.get("fnmt").unwrap().subject);
    assert_eq!(
        entry.fingerprint_sha256,
        v.get("fnmt").unwrap().fingerprint_sha256
    );

    // La clave se abre con la misma frase de paso y da el material original.
    let plain = v2
        .unlock_private_key("fnmt", PASS)
        .expect("la clave debe abrirse");
    assert_eq!(
        plain,
        CLAVE_SINTETICA.to_vec(),
        "el material debe volver idéntico"
    );

    // Con otra frase de paso NO se abre (fail-closed).
    assert!(v2.unlock_private_key("fnmt", "otra-frase").is_err());

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&dir2);
}

#[test]
fn la_exportacion_de_clave_privada_exige_confirm() {
    let dir = tmp_dir("confirm");
    let v = vault_con_dos(&dir);

    let err = v
        .export_certificate("fnmt", PASS, false, KdfParams::fast())
        .expect_err("sin confirm no se exporta la clave privada");
    assert!(err.contains("confirm"), "{}", err);

    // Y queda traza en la auditoría.
    let audit = v.read_audit().unwrap();
    assert!(
        audit
            .iter()
            .any(|l| l.contains("export_denied") && l.contains("fnmt")),
        "auditoría: {:?}",
        audit
    );

    // Un certificado NO exportable sí se puede exportar sin confirm
    // (sólo viaja el certificado público y la referencia).
    let bundle = v
        .export_certificate("dnie", PASS, false, KdfParams::fast())
        .expect("el certificado público se exporta sin confirm");
    assert!(!bundle.is_empty());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn la_importacion_de_clave_privada_exige_confirm() {
    let dir = tmp_dir("confirm2");
    let v = vault_con_dos(&dir);
    let bundle = v
        .export_certificate("fnmt", PASS, true, KdfParams::fast())
        .unwrap();

    let dir2 = tmp_dir("confirm3");
    let v2 = CertVault::new(&dir2).unwrap();
    let err = v2
        .import_certificate(&bundle, Some(PASS), false)
        .expect_err("sin confirm no entra la clave privada");
    assert!(err.contains("confirm"), "{}", err);
    assert!(v2
        .read_audit()
        .unwrap()
        .iter()
        .any(|l| l.contains("import_denied")));

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&dir2);
}

#[test]
fn la_auditoria_no_guarda_la_frase_de_paso() {
    let dir = tmp_dir("audit");
    let v = vault_con_dos(&dir);
    v.export_certificate("fnmt", PASS, true, KdfParams::fast())
        .unwrap();
    let audit = v.read_audit().unwrap();
    assert!(!audit.is_empty());
    for line in &audit {
        assert!(!line.contains(PASS), "la frase de paso no puede ir al log");
        assert!(
            !line
                .as_bytes()
                .windows(CLAVE_SINTETICA.len())
                .any(|w| w == CLAVE_SINTETICA),
            "la clave no puede ir al log"
        );
    }
    // Pero sí la huella del certificado (para poder correlacionar).
    let fp = v.get("fnmt").unwrap().fingerprint_sha256;
    assert!(audit
        .iter()
        .any(|l| l.contains("export") && l.contains(&fp[..16])));

    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// Manifiesto y sincronización
// ---------------------------------------------------------------------------

#[test]
fn el_manifiesto_es_estable_mismo_contenido_mismo_sha256() {
    let dir = tmp_dir("manifiesto");
    let v = vault_con_dos(&dir);

    let m1 = v.build_manifest().unwrap();
    let m2 = v.build_manifest().unwrap();
    assert_eq!(
        m1.digest, m2.digest,
        "el mismo contenido da el mismo digest"
    );
    assert_eq!(m1.digest.len(), 64, "digest SHA-256 en hexadecimal");
    assert_eq!(m1.entries.len(), 2);

    // El manifiesto persistido también es estable (mismo fichero byte a byte).
    v.refresh_manifest().unwrap();
    let a = fs::read(v.manifest_path()).unwrap();
    v.refresh_manifest().unwrap();
    let b = fs::read(v.manifest_path()).unwrap();
    assert_eq!(a, b, "el manifiesto en disco debe ser byte a byte idéntico");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn un_certificado_no_exportable_no_materializa_clave_privada() {
    let dir = tmp_dir("noexp");
    let v = vault_con_dos(&dir);

    // En disco: el certificado público sí; la clave privada, **no**.
    assert!(v.root().join("certs").join("dnie.der").exists());
    assert!(
        !v.root().join("keys").join("dnie.key").exists(),
        "una clave no exportable no puede tener fichero de clave"
    );

    // En el manifiesto: sólo la referencia (backend + handle), nunca material.
    let m = v.build_manifest().unwrap();
    let e = m.entries.iter().find(|e| e.id == "dnie").unwrap();
    match &e.key {
        ManifestKey::NonExportable { backend, handle } => {
            assert_eq!(backend, "android_keystore");
            assert_eq!(handle, "alias-dnie-1");
        }
        other => panic!("se esperaba una referencia no exportable: {:?}", other),
    }
    // El manifiesto no contiene ningún blob de clave.
    let texto = serde_json::to_string(&m).unwrap();
    assert!(!texto.contains("exportable\":{\"backend\":\"android_keystore\",\"sha256"));

    // Y el paquete de sincronización del certificado no exportable **no**
    // lleva ningún material de clave.
    let mut buf: Vec<u8> = Vec::new();
    v.write_sync_bundle(&mut buf, "nodo-destino", SyncOptions::default())
        .unwrap();
    let texto = String::from_utf8_lossy(&buf).to_string();
    assert!(!texto
        .as_bytes()
        .windows(CLAVE_SINTETICA.len())
        .any(|w| w == CLAVE_SINTETICA));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn la_sincronizacion_por_defecto_no_replica_claves_exportables() {
    let dir = tmp_dir("sync1");
    let v = vault_con_dos(&dir);

    let mut buf: Vec<u8> = Vec::new();
    let manifest = v
        .write_sync_bundle(&mut buf, "nodo-destino", SyncOptions::default())
        .unwrap();

    // El paquete declara que NO lleva material privado.
    let texto = String::from_utf8_lossy(&buf).to_string();
    assert!(texto.contains("\"include_exportable_keys\":false"));

    // En destino se importan los dos certificados; el exportable llega **sin**
    // material de clave (sólo el certificado público).
    let dir2 = tmp_dir("sync2");
    let v2 = CertVault::new(&dir2).unwrap();
    let mut cursor = std::io::Cursor::new(buf);
    let report = v2.read_sync_bundle(&mut cursor).unwrap();
    assert_eq!(report.imported, 2, "importa los dos certificados");
    assert_eq!(report.rejected, 0);

    let entry = v2.get("fnmt").unwrap();
    assert!(
        !entry.key.is_exportable(),
        "sin material traveling la clave no puede ser local"
    );
    assert!(
        matches!(entry.key, PrivateKeyRef::Remote { .. }),
        "la clave queda como referencia remota: {:?}",
        entry.key
    );
    // Y **no** se puede abrir con ninguna frase de paso.
    let err = v2.unlock_private_key("fnmt", PASS).unwrap_err();
    assert!(err.contains("este nodo"), "{}", err);

    // Los certificados públicos sí han llegado: mismo DER, misma huella.
    assert_eq!(
        v2.get("fnmt").unwrap().fingerprint_sha256,
        v.get("fnmt").unwrap().fingerprint_sha256
    );
    // El manifiesto del destino **difiere** del origen en `present` (aquí la
    // clave no está) ⇒ digests distintos: es la señal correcta de "aquí sólo
    // está el certificado".
    assert_ne!(
        report.manifest_digest, manifest.digest,
        "sin el material de clave los manifiestos no pueden ser iguales"
    );

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&dir2);
}

#[test]
fn la_sincronizacion_replica_claves_exportables_solo_con_confirm() {
    let dir = tmp_dir("sync3");
    let v = vault_con_dos(&dir);

    // Pedir el material sin confirmar => error (no se replica a medias).
    let mut buf: Vec<u8> = Vec::new();
    let err = v
        .write_sync_bundle(
            &mut buf,
            "nodo-destino",
            SyncOptions {
                include_exportable_keys: true,
                confirm: false,
            },
        )
        .expect_err("sin confirm no se replica la clave privada");
    assert!(err.contains("confirm"), "{}", err);

    // Con los dos sí.
    let mut buf: Vec<u8> = Vec::new();
    let manifest_origen = v
        .write_sync_bundle(
            &mut buf,
            "nodo-destino",
            SyncOptions {
                include_exportable_keys: true,
                confirm: true,
            },
        )
        .unwrap();
    let texto = String::from_utf8_lossy(&buf).to_string();
    assert!(texto.contains("\"include_exportable_keys\":true"));

    let dir2 = tmp_dir("sync4");
    let v2 = CertVault::new(&dir2).unwrap();
    let mut cursor = std::io::Cursor::new(buf);
    let report = v2.read_sync_bundle(&mut cursor).unwrap();

    // En destino la clave está cifrada y se abre con la misma frase de paso.
    let plain = v2
        .unlock_private_key("fnmt", PASS)
        .expect("la clave debe abrirse");
    assert_eq!(plain, CLAVE_SINTETICA.to_vec());

    // Nodo plenamente sincronizado => **mismo manifiesto** (mismo digest).
    assert_eq!(report.manifest_digest, manifest_origen.digest);

    // Y en el disco del destino tampoco hay material en claro.
    for entry in walk(&v2.root()) {
        let data = fs::read(&entry).unwrap();
        assert!(
            !data
                .windows(CLAVE_SINTETICA.len())
                .any(|w| w == CLAVE_SINTETICA),
            "material en claro en {:?}",
            entry
        );
    }

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&dir2);
}

#[test]
fn la_sincronizacion_es_idempotente() {
    let dir = tmp_dir("idem");
    let v = vault_con_dos(&dir);
    let mut buf: Vec<u8> = Vec::new();
    v.write_sync_bundle(&mut buf, "nodo-destino", SyncOptions::default())
        .unwrap();

    let dir2 = tmp_dir("idem2");
    let v2 = CertVault::new(&dir2).unwrap();

    let mut c1 = std::io::Cursor::new(buf.clone());
    let r1 = v2.read_sync_bundle(&mut c1).unwrap();
    assert_eq!(r1.imported, 2);

    // Segunda vez: nada cambia (ni entradas ni ficheros de material).
    // El log de auditoría **sí** crece: es append-only y deja traza de la
    // segunda sincronización (eso es lo que se quiere).
    let before: Vec<(std::path::PathBuf, Vec<u8>)> = walk(&v2.root())
        .into_iter()
        .filter(|p| p.file_name().map(|n| n != "audit.log").unwrap_or(false))
        .map(|p| {
            let d = fs::read(&p).unwrap();
            (p, d)
        })
        .collect();
    let mut c2 = std::io::Cursor::new(buf);
    let r2 = v2.read_sync_bundle(&mut c2).unwrap();
    assert_eq!(
        r2.imported, 0,
        "la segunda importación no debe importar nada"
    );
    assert_eq!(r2.unchanged, 2, "las dos entradas ya estaban");

    let after: Vec<(std::path::PathBuf, Vec<u8>)> = walk(&v2.root())
        .into_iter()
        .filter(|p| p.file_name().map(|n| n != "audit.log").unwrap_or(false))
        .map(|p| {
            let d = fs::read(&p).unwrap();
            (p, d)
        })
        .collect();
    assert_eq!(before.len(), after.len());
    for (p, d) in &before {
        let found = after
            .iter()
            .find(|(q, _)| q == p)
            .expect("el fichero debe seguir");
        assert_eq!(found.1, *d, "el fichero {:?} no debe cambiar", p);
    }

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&dir2);
}

#[test]
fn un_paquete_con_sha256_rotado_se_rechaza_fail_closed() {
    let dir = tmp_dir("roto");
    let v = vault_con_dos(&dir);
    let mut buf: Vec<u8> = Vec::new();
    v.write_sync_bundle(&mut buf, "nodo-destino", SyncOptions::default())
        .unwrap();

    // Se toca un byte del DER que va dentro del paquete (justo detrás de la
    // cabecera: se localiza el DER por su sha256 declarado).
    let mut roto = buf.clone();
    let pos = roto
        .windows(4)
        .position(|w| w == b"\x30\x82\x03\x49")
        .expect("el DER debe estar en el paquete");
    roto[pos + 40] ^= 0xff;

    let dir2 = tmp_dir("roto2");
    let v2 = CertVault::new(&dir2).unwrap();
    let mut cursor = std::io::Cursor::new(roto);
    let err = v2
        .read_sync_bundle(&mut cursor)
        .expect_err("el paquete roto no entra");
    assert!(err.contains("sha256"), "{}", err);
    assert_eq!(v2.list().unwrap().len(), 0, "no se escribe nada");

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&dir2);
}

// ---------------------------------------------------------------------------
// Utilidades del test
// ---------------------------------------------------------------------------

fn walk(root: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(root) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(walk(&p));
            } else {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

#[cfg(unix)]
fn mode_of(p: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(p).unwrap().permissions().mode()
}

#[cfg(not(unix))]
fn mode_of(_p: &Path) -> u32 {
    0
}
