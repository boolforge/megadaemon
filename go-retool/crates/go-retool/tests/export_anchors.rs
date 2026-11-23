//! Export de simbolos Y DE ANCLAJES: prueba de integracion contra el
//! binario real de Antigravity.
//!
//! QUE COMPRUEBA
//! 1. El export TSV nuevo (7 columnas: va, nombre, fichero:linea, va_fin,
//!    paquete, tamano, entryoff) cubre las 161.682 funciones MEDIDAS del
//!    binario real y se relee identico (ida y vuelta).
//! 2. Todas las funciones tienen tamano (extent de la pclntab) y el tamano
//!    es coherente con `end_va - va`.
//! 3. Los anclajes (`validation/antigravity-go1.28.anchors.tsv`) se leen, se
//!    cruzan con los simbolos y TODOS caen sobre una funcion real del
//!    binario: ni una direccion inventada.
//! 4. El JSON lleva la seccion `anchors` con el numero de anclajes.
//! 5. Las familias de anclajes son las esperadas: `entry`, `mcp`, `provider`.
//!
//! CONTEXTO MEDIDO (2026-11-22): binario real
//! `/workspace/agy-re/liblanguage_server.so` (181.358.800 B, Go 1.28 RC02,
//! pclntab en `.lrodata` @ 0x1449ff0, base de texto 0x4d06cb8).
//! Si el binario no esta, los tests que lo necesitan se omiten.

use go_retool::anchors;
use go_retool::pclntab::ParseOptions;
use go_retool::symbols::{
    build_export, parse_json, parse_tsv, to_tsv, SymbolExport,
};
use go_retool::parse_go118_plus;

/// Offset de la pclntab del binario real (MEDIDO).
const PCLNTAB_OFFSET: u64 = 0x1449ff0;
/// Base de texto real (MEDIDO: `.text.hot`, seccion de tamano 0).
const TEXT_BASE: u64 = 0x4d06cb8;
/// Numero de funciones del binario real (MEDIDO).
const NFUNC: u64 = 161_682;
/// Numero de anclajes del binario real (MEDIDO).
const NANCHORS: u64 = 32;

/// Anclajes de referencia quesfn van en el repo.
fn ruta_anchors_repo() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../validation/antigravity-go1.28.anchors.tsv")
}

fn ruta_binario_real() -> Option<std::path::PathBuf> {
    let p = std::path::Path::new("/workspace/agy-re/liblanguage_server.so");
    if p.exists() {
        Some(p.to_path_buf())
    } else {
        None
    }
}

fn exportar_binario_real() -> Option<SymbolExport> {
    let ruta = ruta_binario_real()?;
    let bytes = std::fs::read(&ruta).expect("leer el binario real");
    let opt = ParseOptions {
        module_index: 0,
        file_offset: PCLNTAB_OFFSET,
        text_base: Some(TEXT_BASE),
        text_base_source: Some("explicitly-supplied".to_string()),
        ..Default::default()
    };
    let analysis = parse_go118_plus(&bytes, opt).expect("parsear la pclntab");
    Some(build_export(&analysis, ruta.to_string_lossy().as_ref()))
}

#[test]
fn el_tsv_nuevo_tiene_siete_columnas_y_tamano() {
    let Some(exp) = exportar_binario_real() else {
        eprintln!("binario real no presente: test omitido");
        return;
    };
    assert_eq!(exp.count, NFUNC);
    let tsv = to_tsv(&exp);
    let primera = tsv.lines().next().expect("una linea");
    assert_eq!(primera.split('\t').count(), 7, "7 columnas: va, nombre, loc, fin, pkg, tamano, entryoff");
    assert_eq!(tsv.lines().count() as u64, NFUNC);

    // Todas las funciones tienen tamano y es end_va - va.
    for s in &exp.functions {
        let va = go_retool::symbols::parse_hex(&s.va).expect("va");
        let fin = go_retool::symbols::parse_hex(&s.end_va).expect("end_va");
        assert!(fin > va, "extent vacio en {}", s.name);
        assert_eq!(s.size, Some(fin - va), "tamano incoherente en {}", s.name);
    }
}

#[test]
fn el_tsv_nuevo_se_relee_identico() {
    let Some(exp) = exportar_binario_real() else {
        eprintln!("binario real no presente: test omitido");
        return;
    };
    let tsv = to_tsv(&exp);
    let vuelta = parse_tsv(&tsv).expect("releer el TSV");
    assert_eq!(vuelta.functions.len(), exp.functions.len());
    assert_eq!(vuelta.rows_fnv1a64, exp.rows_fnv1a64);
    for (a, b) in exp.functions.iter().zip(vuelta.functions.iter()) {
        assert_eq!(a.va, b.va);
        assert_eq!(a.name, b.name);
        assert_eq!(a.size, b.size, "el tamano debe sobrevivir a la ida y vuelta");
        assert_eq!(a.entryoff, b.entryoff);
        assert_eq!(a.file, b.file);
        assert_eq!(a.line, b.line);
    }
}

#[test]
fn los_anclajes_caen_sobre_funciones_reales() {
    let Some(exp) = exportar_binario_real() else {
        eprintln!("binario real no presente: test omitido");
        return;
    };
    let ruta = ruta_anchors_repo();
    let texto = std::fs::read_to_string(&ruta).expect("leer los anclajes del repo");
    let mut lista = anchors::parse_tsv(&texto)
        .expect("parsear los anclajes")
        .anchors;
    assert_eq!(lista.len() as u64, NANCHORS, "numero de anclajes MEDIDO");

    anchors::enrich(&mut lista, &exp);

    let por_va: std::collections::HashMap<&str, &go_retool::symbols::Symbol> =
        exp.functions.iter().map(|s| (s.va.as_str(), s)).collect();
    for a in &lista {
        let s = por_va
            .get(a.addr.as_str())
            .unwrap_or_else(|| panic!("el anclaje {} no cae sobre ninguna funcion exportada", a.addr));
        assert_eq!(s.name, a.name, "el nombre real del anclaje debe coincidir con el simbolo");
        assert!(a.size.is_some(), "el anclaje {} debe tener tamano", a.name);
        assert!(a.line.is_some(), "el anclaje {} debe tener linea", a.name);
        assert!(a.file.is_some(), "el anclaje {} debe tener fichero", a.name);
    }
}

#[test]
fn las_familias_de_anclajes_son_las_esperadas() {
    let texto = std::fs::read_to_string(ruta_anchors_repo()).expect("leer los anclajes del repo");
    let lista = anchors::parse_tsv(&texto).expect("parsear los anclajes").anchors;
    let mut familias: Vec<&str> = lista.iter().map(|a| a.tag.as_str()).collect();
    familias.sort();
    familias.dedup();
    assert_eq!(familias, vec!["entry", "mcp", "provider"]);
    // Los dos puntos de entrada estan.
    assert!(lista.iter().any(|a| a.name == "runtime.main"));
    assert!(lista.iter().any(|a| a.name == "main.main"));
}

#[test]
fn el_json_lleva_la_seccion_de_anclajes() {
    let Some(exp) = exportar_binario_real() else {
        eprintln!("binario real no presente: test omitido");
        return;
    };
    let texto = std::fs::read_to_string(ruta_anchors_repo()).expect("leer los anclajes del repo");
    let mut lista = anchors::parse_tsv(&texto).expect("parsear los anclajes").anchors;
    anchors::enrich(&mut lista, &exp);
    let mut con_anchors = exp.clone();
    con_anchors.anchors = lista.clone();

    let json = serde_json::to_string(&con_anchors).expect("serializar");
    let vuelta = parse_json(&json).expect("releer el JSON");
    assert_eq!(vuelta.anchors.len(), NANCHORS as usize);
    assert_eq!(vuelta.functions.len(), NFUNC as usize);
    // El JSON antiguo (sin 'anchors') tambien debe releerse: campo opcional.
    let mut sin_anchors: serde_json::Value = serde_json::from_str(&json).unwrap();
    sin_anchors.as_object_mut().unwrap().remove("anchors");
    assert!(parse_json(&sin_anchors.to_string()).is_ok());
}
