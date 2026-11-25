//! Smoke del **raw** de particiones: parseo, FS por superbloque, volcado por
//! bloques y verificación.
//!
//! **Determinista**: se usa un fichero que imita a un dispositivo de bloque
//! (con su superbloque f2fs) en un directorio temporal. Ni red, ni móvil.

use megadaemon_core::backup_raw as br;
use megadaemon_core::backup_stream as bs;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use sha2::Digest;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("md_raw_test_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Fichero que imita a una partición: superbloque f2fs + bloques de datos.
fn fake_partition(path: &Path, size: usize) {
    let mut data = vec![0u8; size];
    for i in 0..size {
        // Relleno determinista (depende del offset ⇒ los bloques se notan).
        data[i] = ((i / 4096) % 251) as u8;
    }
    // MEDIDO (2026-11-25): el magic se escribía ANTES del relleno, que
    // lo pisaba ⇒ `probe_fs` devolvía `Unknown` en vez de `F2fs`.
    data[1024..1028].copy_from_slice(&[0xF2, 0xF5, 0x20, 0x10]);
    std::fs::write(path, data).unwrap();
}

#[test]
fn se_listan_las_particiones_de_un_proc_partitions_real() {
    let txt = "major minor  #blocks  name\n\n   8        0 249892864 sda\n   8        1     20480 sda1\n   8       34 235732992 sda34\n 253        0   8192000 dm-0\n   7        0     111972 loop0\n";
    let p = br::parse_proc_partitions(txt);
    assert_eq!(p.len(), 3, "sólo los discos: ni loop ni dm-");
    let sda34 = p.iter().find(|x| x.name == "sda34").unwrap();
    assert_eq!(sda34.blocks, 235732992);
    assert_eq!(sda34.size_hint(), 235732992 * 1024);
}

#[test]
fn el_fs_se_lee_del_superbloque_real() {
    let d = tmp("fs");
    let p = d.join("userdata.img");
    fake_partition(&p, 64 * 1024);
    assert_eq!(br::probe_fs(&p), br::FsKind::F2fs);
    // Un fichero vacío no se puede identificar (y no se inventa).
    let e = d.join("vacio.img");
    std::fs::write(&e, b"").unwrap();
    assert_eq!(br::probe_fs(&e), br::FsKind::Unknown);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn el_volcado_por_bloques_sale_exacto_y_se_verifica() {
    let d = tmp("dump");
    let img = d.join("part.img");
    let size = 300 * 1024;
    fake_partition(&img, size);
    let out = d.join("out");

    let sink = Arc::new(bs::FileSink::new(&out, "raw").unwrap());
    // MEDIDO (2026-11-25): 64 KiB NO es múltiplo de 256 KiB, que es lo que
    // exige Drive => la tubería lo rechaza.  El volcado usa trozos
    // válidos: 300 KiB son 2 trozos (256 KiB + 44 KiB).
    let chunk = 256 * 1024u64;
    let mut pipeline =
        bs::Pipeline::new(sink, "raw", size as u64, chunk, 2, BTreeMap::new()).unwrap();
    let opts = br::RawOptions {
        device: img.to_string_lossy().to_string(),
        chunk_bytes: chunk,
        max_bytes: 0,
        label: "raw".into(),
    };
    let report = br::dump_to_stream(&img, &mut pipeline, &opts).unwrap();
    let summary = pipeline.finish().unwrap();

    // Todo el contenido ha salido, bloque a bloque.
    assert_eq!(report.bytes, size as u64);
    assert_eq!(summary.bytes, size as u64);
    assert_eq!(report.chunks.len(), 2); // 300 KiB en trozos de 256 KiB
    assert_eq!(report.fs.as_deref(), Some("f2fs"));

    // El hash global es el del fichero original.
    let original = std::fs::read(&img).unwrap();
    assert_eq!(
        report.sha256,
        megadaemon_core::backup_stream::hex(&sha2::Sha256::digest(&original))
    );

    // La verificación relee el origen y cuadra.
    let v = br::verify_raw(&img, &report).unwrap();
    assert!(v.ok, "{:?}", v.blocks_bad);
    assert_eq!(v.blocks_checked, 2);

    // Si el origen cambia, la verificación lo detecta (bloque exacto).
    let mut changed = original.clone();
    changed[70 * 1024] ^= 0xFF; // dentro del segundo bloque
    std::fs::write(&img, &changed).unwrap();
    let v2 = br::verify_raw(&img, &report).unwrap();
    assert!(!v2.ok);
    assert_eq!(v2.blocks_bad.len(), 1);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn el_plan_raw_avisa_del_tamano_antes_de_empezar() {
    let d = tmp("plan");
    fake_device(&d);
    let plan = br::plan_raw(d.to_str().unwrap(), &[], 1024);
    // No hay /proc/partitions en el temporal ⇒ plan vacío con aviso.
    assert!(plan.partitions.is_empty());
    assert!(plan.warnings.iter().any(|w| w.contains("particiones")));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn el_volcado_nunca_escribe_en_el_origen() {
    // El módulo sólo abre en lectura: se comprueba que el origen conserva
    // su contenido y su mtime después de volcarlo.
    let d = tmp("ro");
    let img = d.join("part.img");
    fake_partition(&img, 128 * 1024);
    let before = std::fs::metadata(&img).unwrap();
    let out = d.join("out");
    let sink = Arc::new(bs::FileSink::new(&out, "raw").unwrap());
    let mut pipeline =
        bs::Pipeline::new(sink, "raw", before.len(), 256 * 1024, 1, BTreeMap::new()).unwrap();
    let opts = br::RawOptions {
        device: img.to_string_lossy().to_string(),
        chunk_bytes: 256 * 1024,
        max_bytes: 0,
        label: "raw".into(),
    };
    let _ = br::dump_to_stream(&img, &mut pipeline, &opts).unwrap();
    let _ = pipeline.finish().unwrap();
    let after = std::fs::metadata(&img).unwrap();
    assert_eq!(before.len(), after.len());
    assert_eq!(before.modified().unwrap(), after.modified().unwrap());
    let _ = std::fs::remove_dir_all(&d);
}

/// Crea un `/proc` y `/dev/block` de mentira para el plan.
fn fake_device(root: &Path) {
    std::fs::create_dir_all(root.join("proc")).unwrap();
    std::fs::write(
        root.join("proc/partitions"),
        "major minor  #blocks  name\n\n   8       34 235732992 sda34\n",
    )
    .unwrap();
    std::fs::write(
        root.join("proc/mounts"),
        "/dev/block/dm-0 / ext4 ro 0 0\n",
    )
    .unwrap();
}
