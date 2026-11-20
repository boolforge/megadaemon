//! Arnés de pruebas de los lectores de formatos externos (tar / zip / Swift).

#[path = "../src/backup_formats.rs"]
mod backup_formats;

use backup_formats::*;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use std::hash::Hasher;
use std::io::Write;

// --- constructores de prueba ------------------------------------------------

fn octal_field(v: u64, len: usize) -> Vec<u8> {
    let s = format!("{:0width$o}", v, width = len - 1);
    let mut b = s.into_bytes();
    b.push(0);
    b
}

fn tar_header(name: &str, size: u64, typeflag: u8) -> Vec<u8> {
    let mut h = vec![0u8; 512];
    let nb = name.as_bytes();
    h[..nb.len().min(100)].copy_from_slice(&nb[..nb.len().min(100)]);
    h[100..108].copy_from_slice(&octal_field(0o644, 8));
    h[108..116].copy_from_slice(&octal_field(0, 8));
    h[116..124].copy_from_slice(&octal_field(0, 8));
    h[124..136].copy_from_slice(&octal_field(size, 12));
    h[136..148].copy_from_slice(&octal_field(1_700_000_000, 12));
    h[148..156].copy_from_slice(b"        ");
    h[156] = typeflag;
    h[257..263].copy_from_slice(b"ustar\0");
    h[263..265].copy_from_slice(b"00");
    let sum: u32 = h.iter().map(|&b| b as u32).sum();
    h[148..156].copy_from_slice(&octal_field(sum as u64, 8));
    h
}

fn tar_append(out: &mut Vec<u8>, name: &str, data: &[u8], typeflag: u8) {
    out.extend_from_slice(&tar_header(name, data.len() as u64, typeflag));
    out.extend_from_slice(data);
    let pad = (512 - (data.len() % 512)) % 512;
    out.extend(std::iter::repeat(0u8).take(pad));
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = flate2::Crc::new();
    c.update(data);
    c.sum()
}

fn zip_build(entries: &[(&str, &[u8], bool)]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut cds: Vec<u8> = Vec::new();
    for (name, data, deflate) in entries {
        let local_off = out.len() as u32;
        let (method, payload) = if *deflate {
            let mut e = DeflateEncoder::new(Vec::new(), Compression::default());
            e.write_all(data).unwrap();
            (8u16, e.finish().unwrap())
        } else {
            (0u16, data.to_vec())
        };
        let crc = crc32(data);
        let nb = name.as_bytes();
        out.extend_from_slice(b"PK\x03\x04");
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&method.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(nb.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(nb);
        out.extend_from_slice(&payload);
        cds.extend_from_slice(b"PK\x01\x02");
        cds.extend_from_slice(&20u16.to_le_bytes());
        cds.extend_from_slice(&20u16.to_le_bytes());
        cds.extend_from_slice(&0u16.to_le_bytes());
        cds.extend_from_slice(&method.to_le_bytes());
        cds.extend_from_slice(&0u16.to_le_bytes());
        cds.extend_from_slice(&0u16.to_le_bytes());
        cds.extend_from_slice(&crc.to_le_bytes());
        cds.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        cds.extend_from_slice(&(data.len() as u32).to_le_bytes());
        cds.extend_from_slice(&(nb.len() as u16).to_le_bytes());
        cds.extend_from_slice(&0u16.to_le_bytes());
        cds.extend_from_slice(&0u16.to_le_bytes());
        cds.extend_from_slice(&0u16.to_le_bytes());
        cds.extend_from_slice(&0u16.to_le_bytes());
        cds.extend_from_slice(&0u32.to_le_bytes());
        cds.extend_from_slice(&local_off.to_le_bytes());
        cds.extend_from_slice(nb);
    }
    let cd_off = out.len() as u32;
    let cd_size = cds.len() as u32;
    out.extend_from_slice(&cds);
    let n = entries.len() as u16;
    out.extend_from_slice(b"PK\x05\x06");
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

// --- pruebas ----------------------------------------------------------------

#[test]
fn el_tar_se_lee_con_nombre_tamano_y_tipo() {
    let mut t = Vec::new();
    tar_append(&mut t, "app/com.agy/files/datos.db", b"contenido sqlite", b'0');
    tar_append(&mut t, "app/com.agy/", b"", b'5');
    let e = read_tar(&t).unwrap();
    assert_eq!(e.len(), 2);
    assert_eq!(e[0].name, "app/com.agy/files/datos.db");
    assert_eq!(e[0].size, 16);
    assert_eq!(e[0].typeflag, b'0');
    assert!(e[1].is_dir());
    let data = &t[e[0].data_offset..e[0].data_offset + e[0].size as usize];
    assert_eq!(data, b"contenido sqlite");
}

#[test]
fn el_tar_admite_nombres_largos_gnu_y_pax() {
    let largo = format!("app/{}/datos.bin", "x".repeat(120));
    let mut t = Vec::new();
    tar_append(&mut t, "././@LongLink", largo.as_bytes(), b'L');
    tar_append(&mut t, "app/x", b"dato", b'0');
    let e = read_tar(&t).unwrap();
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].name, largo);
}

#[test]
fn la_extraccion_de_tar_es_segura() {
    let mut t = Vec::new();
    tar_append(&mut t, "datos/fichero.txt", b"hola", b'0');
    let dir = std::env::temp_dir().join(format!("md-tar-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let w = tar_extract(&t, &dir).unwrap();
    assert_eq!(w, vec!["datos/fichero.txt".to_string()]);
    assert_eq!(std::fs::read_to_string(dir.join("datos/fichero.txt")).unwrap(), "hola");

    let mut evil = Vec::new();
    tar_append(&mut evil, "../escapado.txt", b"x", b'0');
    let err = tar_extract(&evil, &dir).unwrap_err();
    assert!(err.contains("salto de directorio"), "{}", err);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn el_zip_se_lee_con_los_dos_metodos() {
    let grande = "a".repeat(5000);
    let z = zip_build(&[
        ("app/datos.db", b"sqlite payload".as_slice(), false),
        ("app/perfil.txt", grande.as_bytes(), true),
    ]);
    let e = read_zip(&z).unwrap();
    assert_eq!(e.len(), 2);
    assert_eq!(e[0].method, 0);
    assert_eq!(e[0].size, 14);
    assert_eq!(e[1].method, 8);
    assert_eq!(e[1].size, 5000);
    assert_eq!(e[1].compressed_size < 5000, true);
    assert_eq!(zip_read_data(&z, &e[0]).unwrap(), b"sqlite payload");
    assert_eq!(zip_read_data(&z, &e[1]).unwrap().len(), 5000);
}

#[test]
fn la_extraccion_de_zip_es_segura_y_escribe_los_ficheros() {
    let grande = "z".repeat(4096);
    let z = zip_build(&[
        ("respaldo/db.sqlite", b"contenido db".as_slice(), false),
        ("respaldo/grande.bin", grande.as_bytes(), true),
    ]);
    let dir = std::env::temp_dir().join(format!("md-zip-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let w = zip_extract(&z, &dir).unwrap();
    assert_eq!(w.len(), 2);
    assert_eq!(std::fs::read(dir.join("respaldo/grande.bin")).unwrap().len(), 4096);

    let evil = zip_build(&[("/absoluto.txt", b"x".as_slice(), false)]);
    assert!(zip_extract(&evil, &dir).unwrap_err().contains("absoluta"));
    let evil2 = zip_build(&[("../fuera.txt", b"x".as_slice(), false)]);
    assert!(zip_extract(&evil2, &dir).unwrap_err().contains("salto de directorio"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn el_metadata_de_swift_se_lee() {
    let dir = std::env::temp_dir().join(format!("md-swift-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("metadata.json");
    std::fs::write(&p, r#"{"package":"com.agy","files":["a.db","b.apk"],"parts":["APP","DATA"],"timestamp":1700000000}"#).unwrap();
    let m = read_swift_metadata(&p.to_string_lossy()).unwrap();
    assert_eq!(m.package.as_deref(), Some("com.agy"));
    assert_eq!(m.files.len(), 2);
    assert!(is_swift_part("DATA"));
    assert!(!is_swift_part("MEDIA2"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn las_rutas_relativas_se_validan() {
    assert!(safe_relative_path("a/b.txt").is_ok());
    assert!(safe_relative_path("./a//b.txt").is_ok());
    assert!(safe_relative_path("../x").is_err());
    assert!(safe_relative_path("/x").is_err());
    assert!(safe_relative_path("C:\\x").is_err());
    assert!(safe_relative_path("").is_err());
}
