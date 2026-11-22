//! Tests del contenedor **Swift Backup `SBA1`** sobre el **`.dat` real**.
//!
//! # El fichero
//!
//! `tests/data/swift/com.letsappbuilder.dat` — el **backup real de 944 B**
//! (`com.letsappbuilder.dat (SM-G998B) (id-20261120-043914-ES)`) descargado
//! de la carpeta **pública** de Swift Backup en Google Drive.
//!
//! | | |
//! |---|---|
//! | Bytes | **944** |
//! | md5 | `d9519867c8874f7a8b87086049266982` (el mismo que dice Drive) |
//! | sha256 | `1f85eb55cf45b78c7b1ea3e43cb79e2dbbb0ecc762c786a7b16a7ea266184c90` |
//! | Magic | `SBA1` (cabecera de 144 B) + `SAE1` + `SAI1` + `SAF1` |
//!
//! Los **valores esperados** de la cabecera están **congelados** aquí: se
//! midieron comparando **tres** ficheros reales del mismo backup (este, más
//! `com.android.chrome.dat` e `io.github.forkmaintainers.iceraven.dat`), y
//! los que **cambian** entre backups (sal, nonce, key-check, HMAC) **no** se
//! congelan: son aleatorios por diseño.
//!
//! Lo que **sí** se congela son los parámetros del KDF y del troceo, que son
//! **constantes** en los tres ficheros reales:
//!
//! * `payloadChunkSize` = **1048576** (1 MiB)
//! * `argon2MemoryKib` = **16384**
//! * `kdfMethodId` = **1** (Argon2id)
//! * `keyLength` = **32**
//! * `encryptionMethodId` = **4** (AEGIS-256)

use megadaemon_core::swift_container as sc;
use megadaemon_core::swift_archive;
use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/swift/com.letsappbuilder.dat")
}

fn bytes() -> Vec<u8> {
    let b = std::fs::read(fixture()).expect("debe existir el .dat real");
    assert_eq!(b.len(), 944, "el .dat real mide 944 B");
    assert_eq!(
        sc::hex(&sha256(&b)),
        "1f85eb55cf45b78c7b1ea3e43cb79e2dbbb0ecc762c786a7b16a7ea266184c90"
    );
    b
}

fn sha256(data: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().to_vec()
}

#[test]
fn la_cabecera_real_cumple_el_layout_medido() {
    let b = bytes();
    let h = sc::leer_cabecera(&b).expect("la cabecera real debe leerse");
    assert_eq!(h.formato, 2);
    assert_eq!(h.header_size, 144);
    assert_eq!(h.compression, 3, "zstd");
    assert_eq!(h.encryption_method_id, 4, "AEGIS-256");
    assert_eq!(h.encryption, 1, "el backup está cifrado");
    assert_eq!(h.payload_chunk_size, 1_048_576, "trozos de 1 MiB");
    assert_eq!(h.argon2_memory_kib, 16384);
    assert_eq!(h.kdf_method_id, 1, "Argon2id");
    assert_eq!(h.key_length, 32);
    assert_eq!(&h.reservado8, &[0u8; 8], "los 8 B reservados son 0");
    // createdAtEpochMs: 2026-11-20 (el backup `id-20261120-043914-ES`).
    assert_eq!(h.created_at_epoch_ms, 1_795_145_956_000);
}

#[test]
fn el_lector_del_repo_ve_la_misma_cabecera() {
    // `swift_archive` es la fuente de verdad del formato: tiene que coincidir
    // con lo que medimos aquí (o el repo miente).
    let b = bytes();
    let layout = swift_archive::sba1_detectar(&b).expect("debe detectar SBA1");
    assert_eq!(layout.header_size, Some(144));
    let informe = swift_archive::parse_header(&b, &layout).expect("debe parsear");
    // Los nombres de campo son los **medidos** en el layout actual del repo
    // (`chunkSize`, `argon2MemoryKib`, `keyLength`); antes se llamaban
    // `campo12`, `campo13` y `campo16`.
    let get = |n: &str| informe.fields.iter().find(|h| h.name == n).map(|h| h.value);
    assert_eq!(get("formato"), Some(2));
    assert_eq!(get("headerSize"), Some(144));
    assert_eq!(get("compression"), Some(3));
    assert_eq!(get("chunkSize"), Some(1_048_576));
    assert_eq!(get("argon2MemoryKib"), Some(16384));
    assert_eq!(get("campo14"), Some(1));
    assert_eq!(get("keyLength"), Some(32));
}

#[test]
fn el_contenedor_real_tiene_sae1_sai1_y_saf1() {
    let b = bytes();
    let cont = sc::abrir(&b).expect("debe abrir");
    assert_eq!(cont.entradas.len(), 1, "una entrada: {:?}", cont.entradas);
    assert_eq!(cont.entradas[0].nombre, "data");
    assert_eq!(cont.entradas[0].header_size, 24);
    // El payload cifrado de la entrada empieza justo detrás del nombre.
    assert_eq!(cont.entradas[0].payload_offset, 172);

    let met = cont.metadatos.expect("debe haber SAI1");
    assert_eq!(met.json["packageName"], "com.letsappbuilder");
    assert_eq!(met.json["kind"], "swiftbackup.app-data");
    assert_eq!(met.json["entries"][0], "data");
    assert_eq!(met.json["encrypted"], true, "el payload va cifrado");
    assert_eq!(met.json["dataSize"], 11264);
    assert_eq!(met.json["versionName"], "5.0.4");
    assert_eq!(met.json["versionCode"], 2305);
    assert_eq!(met.json["compressionLevel"], "FASTEST");

    let pie = cont.pie.expect("debe haber SAF1");
    assert_eq!(pie.header_size, 32);
    // Invariante medida: el pie apunta a la posición real de SAI1.
    assert_eq!(
        pie.offset_sai as usize, met.offset,
        "el pie SAF1 debe apuntar al SAI1 real"
    );
    assert!(cont.avisos.is_empty(), "avisos: {:?}", cont.avisos);
}

#[test]
fn el_contenedor_sba1_no_se_descifra_a_ciegas() {
    // Sin la passphrase correcta el key-check **no** puede cuadrar: el lector
    // tiene que decirlo, no devolver basura.
    let b = bytes();
    let r = sc::round_trip(&b, "una-passphrase-que-no-es-la-buena")
        .expect("el round-trip debe devolver veredicto, no panic");
    assert!(!r.ok);
    assert!(!r.pasos.is_empty());
    let kc = r.pasos.iter().find(|p| p.paso == "key_check").unwrap();
    assert!(!kc.ok, "el key-check NO puede pasar con una passphrase falsa");
    assert!(
        kc.detalle.contains("passphrase") || kc.detalle.contains("candidatos"),
        "el motivo debe ser explícito: {}",
        kc.detalle
    );
    // Los metadatos en claro se leen **igual** (no dependen de la clave).
    let m = r.metadatos.expect("los metadatos en claro no dependen de la clave");
    assert_eq!(m["encrypted"], true);
}

#[test]
fn la_busqueda_de_clave_es_acotada_y_determinista() {
    // Con la passphrase equivocada la búsqueda agota el espacio medido y
    // falla con un motivo concreto (no se cuelga).
    let b = bytes();
    let e = sc::buscar_clave(&b, "mala").expect_err("no puede achar con una mala");
    assert!(e.contains("candidatos"), "{e}");
    // La búsqueda es determinista: dos veces, el mismo resultado.
    let e2 = sc::buscar_clave(&b, "mala").expect_err("otra vez");
    assert_eq!(e, e2);
}
