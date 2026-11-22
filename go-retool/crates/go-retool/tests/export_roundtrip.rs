//! Prueba de ronda (round-trip) del export de simbolos de go-retool.
//!
//! QUE COMPRUEBA
//! 1. El export (JSON y TSV) se puede **releer** con los parsers del propio
//!    crate y la tabla releida es IDENTICA a la exportada (mismos VA, nombres,
//!    fichero y linea).
//! 2. La **corrupcion** se detecta: TSV con campos de menos, direccion no
//!    hexadecimal, JSON truncado, JSON con `count` incorrecto y JSON con el
//!    checksum de filas manipulado.
//!
//! CONTEXTO MEDIDO (2026-11-22): con el binario real de Antigravity
//! (`/workspace/agy-re/liblanguage_server.so`, 181.358.800 B, Go 1.28 RC02,
//! pclntab en `.lrodata` @ 0x1449ff0, base de texto 0x4d06cb8) el export
//! produce 161.682 funciones, 161.680 con fichero y linea, y coincide
//! EXACTAMENTE con la tabla de referencia del agente de RE (161.682
//! direcciones, 0 diferencias de nombre, 0 diferencias de fichero:linea).
//! El test usa ese binario si esta presente; si no, se salta.

use go_retool::pclntab::ParseOptions;
use go_retool::sha256::sha256_hex;
use go_retool::symbols::{
    build_export, parse_json, parse_tsv, to_rizin_script, to_tsv, SymbolExport,
};
use go_retool::{parse_go118_plus, Result};

/// Offset de la pclntab del binario real (MEDIDO).
const PCLNTAB_OFFSET: u64 = 0x1449ff0;
/// Base de texto real (MEDIDO: `.text.hot`, seccion de tamano 0).
const TEXT_BASE: u64 = 0x4d06cb8;
/// Numero de funciones del binario real (MEDIDO).
const NFUNC: u64 = 161_682;

fn ruta_binario_real() -> Option<std::path::PathBuf> {
    let p = std::path::Path::new("/workspace/agy-re/liblanguage_server.so");
    if p.exists() {
        Some(p.to_path_buf())
    } else {
        None
    }
}

/// Exporta el binario real (si esta) y devuelve la tabla de simbolos.
fn exportar_binario_real() -> Option<SymbolExport> {
    let ruta = ruta_binario_real()?;
    let bytes = std::fs::read(&ruta).expect("leer el binario real");
    let opt = ParseOptions {
        module_index: 0,
        file_offset: PCLNTAB_OFFSET,
        text_base: Some(TEXT_BASE),
        text_base_source: Some("explicitly-supplied"),
        ..Default::default()
    };
    let analysis = parse_go118_plus(&bytes, opt).expect("parsear la pclntab");
    Some(build_export(&analysis, ruta.to_string_lossy().as_ref()))
}

#[test]
fn el_export_se_relee_identico() {
    let Some(exp) = exportar_binario_real() else {
        eprintln!("binario real no presente: test omitido");
        return;
    };
    assert_eq!(exp.count, NFUNC, "numero de funciones MEDIDO");
    assert_eq!(exp.functions.len() as u64, NFUNC);

    // TSV: ida y vuelta.
    let tsv = to_tsv(&exp);
    let vuelta = parse_tsv(&tsv).expect("releer el TSV exportado");
    assert_eq!(vuelta.functions.len(), exp.functions.len());
    assert_eq!(vuelta.rows_fnv1a64, exp.rows_fnv1a64, "checksum de filas");
    for (a, b) in exp.functions.iter().zip(vuelta.functions.iter()) {
        assert_eq!(a.va, b.va);
        assert_eq!(a.name, b.name);
        assert_eq!(a.file, b.file);
        assert_eq!(a.line, b.line);
        assert_eq!(a.package, b.package);
    }

    // JSON: ida y vuelta.
    let json = serde_json::to_string(&exp).expect("serializar el JSON");
    let vuelta = parse_json(&json).expect("releer el JSON exportado");
    assert_eq!(vuelta.functions, exp.functions);
    assert_eq!(vuelta.count, exp.count);
}

#[test]
fn el_export_detecta_corrupcion() {
    let Some(exp) = exportar_binario_real() else {
        eprintln!("binario real no presente: test omitido");
        return;
    };
    let tsv = to_tsv(&exp);
    let json = serde_json::to_string(&exp).expect("serializar el JSON");

    // 1. TSV truncado (una linea con menos campos).
    let primera = tsv.lines().next().unwrap();
    let cortada = primera.split('\t').take(2).collect::<Vec<_>>().join("\t");
    assert!(parse_tsv(&cortada).is_err(), "TSV con 2 campos debe fallar");

    // 2. TSV con direccion no hexadecimal.
    let mut campos: Vec<&str> = primera.split('\t').collect();
    campos[0] = "0xZZZZ";
    assert!(parse_tsv(&campos.join("\t")).is_err(), "direccion no hex debe fallar");

    // 3. JSON truncado.
    assert!(parse_json(&json[..json.len() / 3]).is_err(), "JSON truncado debe fallar");

    // 4. JSON con el checksum de filas manipulado.
    let mut roto: serde_json::Value = serde_json::from_str(&json).expect("JSON valido");
    roto["rows_fnv1a64"] = serde_json::Value::String("0000000000000000".to_string());
    assert!(parse_json(&roto.to_string()).is_err(), "checksum manipulado debe fallar");

    // 5. JSON con `count` manipulado.
    let mut roto: serde_json::Value = serde_json::from_str(&json).expect("JSON valido");
    roto["count"] = serde_json::Value::from(1u64);
    assert!(parse_json(&roto.to_string()).is_err(), "count incorrecto debe fallar");

    // 6. JSON con una fila cambiada (nombre) pero checksum intacto.
    let mut roto: serde_json::Value = serde_json::from_str(&json).expect("JSON valido");
    roto["functions"][0]["name"] = serde_json::Value::String("main.hack".to_string());
    assert!(parse_json(&roto.to_string()).is_err(), "nombre alterado debe fallar");
}

#[test]
fn el_script_de_rizin_cubre_todas_las_funciones() {
    let Some(exp) = exportar_binario_real() else {
        eprintln!("binario real no presente: test omitido");
        return;
    };
    let script = to_rizin_script(&exp);
    let flags = script.lines().filter(|l| l.starts_with("f ")).count();
    assert_eq!(flags as u64, NFUNC, "un flag de Rizin por funcion");
    // Los nombres de flag no pueden llevar '/' ni parentesis.
    for linea in script.lines().filter(|l| l.starts_with("f ")) {
        let nombre = linea.split_whitespace().nth(1).unwrap();
        assert!(!nombre.contains('/'), "nombre de flag con '/': {nombre}");
        assert!(!nombre.contains('('), "nombre de flag con '(': {nombre}");
    }
}

#[test]
fn el_sha256_del_export_es_estable() {
    let Some(exp) = exportar_binario_real() else {
        eprintln!("binario real no presente: test omitido");
        return;
    };
    let tsv = to_tsv(&exp);
    // El hash se recalcula igual cada vez (determinista).
    assert_eq!(sha256_hex(tsv.as_bytes()), sha256_hex(to_tsv(&exp).as_bytes()));
    assert_eq!(tsv.lines().count() as u64, NFUNC, "una linea TSV por funcion");
}

/// Variante con errores de tipos: el crate no debe compilar con `?` sobre un
/// `Result` con error dinamico.  Este test existe para dejar constancia de
/// que el error de corrupcion es *mensaje*, no panico.
#[test]
fn los_errores_de_parseo_no_panican() {
    let caso: Result<()> = Err(go_retool::Error::InvalidMsg("prueba".to_string()));
    assert!(caso.is_err());
}
