//! Smoke de la tubería de subida: reparto en N conexiones, SHA-256 por
//! trozo, progreso, estado de reanudación y destino local.
//!
//! **Determinista**: sin red. El destino es un [`bs::FileSink`] (ficheros
//! locales), que es exactamente lo que usa `--simulate`.

use megadaemon_core::backup_stream as bs;
use sha2::Digest;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("md_stream_test_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn el_flujo_se_reparte_en_n_conexiones_y_sale_entero() {
    let d = tmp("split");
    let sink = Arc::new(bs::FileSink::new(&d, "flujo").unwrap());
    let chunk = 256 * 1024u64;
    let total = 10 * chunk;
    let mut pipeline = bs::Pipeline::new(sink, "flujo", total, chunk, 3, BTreeMap::new()).unwrap();
    assert_eq!(pipeline.total_chunks(), 10);

    // 10 trozos de 256 KiB con contenido distinto cada uno.
    let mut esperado: Vec<u8> = Vec::new();
    for i in 0..10u64 {
        let mut b = vec![i as u8; chunk as usize];
        b[0] = (i % 251) as u8;
        esperado.extend_from_slice(&b);
        pipeline.push(b).unwrap();
    }
    let summary = pipeline.finish().unwrap();

    assert_eq!(summary.bytes, total);
    assert_eq!(summary.chunks, 10);
    assert_eq!(summary.streams, 3);
    // El hash global es el del flujo entero, en orden.
    assert_eq!(
        summary.sha256,
        bs::hex(&sha2::Sha256::digest(&esperado))
    );

    // Cada tramo está en su fichero y cada uno lleva su parte.
    let mut recomponido: Vec<u8> = Vec::new();
    for s in 0..3 {
        let p = d.join(format!("flujo.part{:02}", s));
        let data = std::fs::read(&p).unwrap();
        assert!(!data.is_empty(), "el tramo {s} está vacío");
        recomponido.extend_from_slice(&data);
    }
    assert_eq!(recomponido, esperado);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn el_progreso_informa_de_bytes_velocidad_y_eta() {
    let d = tmp("progress");
    let sink = Arc::new(bs::FileSink::new(&d, "p").unwrap());
    let chunk = 256 * 1024u64;
    let total = 4 * chunk;
    let mut pipeline = bs::Pipeline::new(sink, "p", total, chunk, 1, BTreeMap::new()).unwrap();
    let prog = pipeline.progress();
    for _ in 0..4 {
        pipeline.push(vec![7u8; chunk as usize]).unwrap();
    }
    let _ = pipeline.finish().unwrap();
    let s = prog.snapshot();
    assert_eq!(s.bytes_uploaded, total);
    assert_eq!(s.chunks_done, 4);
    assert_eq!(s.total_bytes, total);
    assert!(s.mb_per_sec > 0.0);
    let linea = prog.line();
    assert!(linea.contains("MB/s"));
    assert!(linea.contains("ETA"));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn el_estado_de_reanudacion_se_guarda_y_se_recupera() {
    let d = tmp("resume");
    let state_path = d.join("estado.json");
    let mut st = bs::UploadState::new("flujo", 1024, 256 * 1024, 2);
    st.sessions.insert(0, "https://example/session0".to_string());
    st.uploaded
        .entry(0)
        .or_default()
        .insert(0, "aa".to_string());
    st.save(&state_path).unwrap();

    let leido = bs::UploadState::load(&state_path).unwrap();
    assert_eq!(leido.label, "flujo");
    assert_eq!(leido.total_bytes, 1024);
    assert_eq!(leido.sessions.get(&0).unwrap(), "https://example/session0");
    assert_eq!(leido.done().len(), 1);
    assert_eq!(leido.done().get(&0).unwrap(), "aa");

    // Con el estado cargado, la tubería se salta los trozos ya subidos.
    let sink = Arc::new(bs::FileSink::new(&d, "r").unwrap());
    let pipeline =
        bs::Pipeline::new(sink, "r", 1024, 256 * 1024, 1, leido.done()).unwrap();
    assert_eq!(pipeline.skipped(), 1);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn el_tamano_de_trozo_debe_ser_multiplo_de_256_kib() {
    let d = tmp("size");
    let sink = Arc::new(bs::FileSink::new(&d, "x").unwrap());
    let e = match bs::Pipeline::new(sink, "x", 1024, 1000, 1, BTreeMap::new()) {
        Ok(_) => panic!("un trozo de 1000 B tiene que ser rechazado"),
        Err(e) => e,
    };
    assert!(e.contains("múltiplo"), "{e}");
    // 256 KiB exactos: vale.
    let sink = Arc::new(bs::FileSink::new(&d, "y").unwrap());
    assert!(bs::Pipeline::new(sink, "y", 1024, 256 * 1024, 1, BTreeMap::new()).is_ok());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn el_rango_de_reanudacion_se_interpreta() {
    assert_eq!(bs::parse_range_end("bytes=0-524287"), Some(524288));
    assert_eq!(bs::parse_range_end("bytes 0-0"), Some(1));
    assert_eq!(bs::parse_range_end("nonsense"), None);
}
