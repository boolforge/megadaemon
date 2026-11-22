//! Tests de la resolucion de la base de texto (text_base).
//!
//! CONTEXTO MEDIDO (2026-11-22, S21 Ultra -> sandbox):
//! Binario real: `liblanguage_server.so` de la IDE Antigravity (Google),
//! 181.358.800 B, ELF64 AArch64 PIE **stripped**, Go **1.28-20260921-RC02**
//! (layout de pclntab 1.20, minLC=4), con la pclntab **dentro de
//! `.lrodata`** (offset 0x1449ff0) y **sin** seccion `.gopclntab`.
//!
//! HECHOS MEDIDOS QUE ESTE TEST FIJA:
//! 1. `pcHeader.textStart` vale **0** en una biblioteca compartida: el
//!    linker no puede conocer la VA final del texto.
//! 2. La base de texto real es **0x4d06cb8**, que es la direccion de la
//!    seccion `.text.hot` (de **tamano 0**), NO la de `.text` (0x4d0f000).
//!    Usar `.text` desplaza todas las VAs ~0x8330 bytes y los nombres NO
//!    cuadran con las direcciones.
//! 3. Con la base 0x4d06cb8, las 161.682 funciones salen **exactamente**
//!    igual que en la tabla de referencia del agente de RE
//!    (`go.funcs.tsv`, 161.682 entradas): 100 % de direcciones coincidentes.
//! 4. La herramienta de referencia `redress` **NO** soporta este binario
//!    ("no goversion found") y `unstrip` (Rust) parsea el binario pero con
//!    otro layout: por eso hace falta un parser propio.
//!
//! LO QUE NO SE AFIRMA (NO VERIFICADO):
//! * `moduledata.text` NO se localiza todavia: la base se ESTIMA con la
//!   regla de las secciones ".text*". En un ELF enlazado normal esa
//!   estimacion coincide con `.text`; en un PIE con subsecciones puede
//!   diferir, y el `text_base_status` lo dice explicitamente.

use go_retool::container::{scan_container, text_base_hint, SectionInfo};

fn sec(name: &str, addr: &str, size: u64) -> SectionInfo {
    SectionInfo {
        name: name.to_string(),
        kind: "text".to_string(),
        section_address: addr.to_string(),
        address_domain: "virtual_address".to_string(),
        size,
        file_offset: Some(addr.to_string()),
        file_size: Some(size),
    }
}

#[test]
fn la_base_de_texto_es_la_menor_seccion_text_incluso_si_mide_cero() {
    // Reproduce el caso MEDIDO del binario real de Antigravity:
    // .text.hot (tamano 0) va ANTES que .text, y la base correcta es la
    // de .text.hot.
    let secciones = vec![
        sec(".text.hot", "0x4d06cb8", 0),
        sec(".text", "0x4d0f000", 44_578_320),
        sec(".text.unlikely", "0x7792610", 13_872),
    ];
    assert_eq!(text_base_hint(&secciones), Some(0x4d06cb8));
}

#[test]
fn una_seccion_text_de_tamano_cero_no_se_ignora() {
    // Si se filtrase por size>0, la estimacion seria 0x4d0f000 (equivocada).
    let secciones = vec![sec(".text.hot", "0x4d06cb8", 0), sec(".text", "0x4d0f000", 100)];
    assert_ne!(text_base_hint(&secciones), Some(0x4d0f000));
}

#[test]
fn sin_secciones_de_texto_no_hay_estimacion() {
    assert_eq!(text_base_hint(&[]), None);
    assert_eq!(
        text_base_hint(&[sec(".rodata", "0x1449ff0", 48_129_622)]),
        None
    );
}

#[test]
fn el_binario_real_de_antigravity_da_la_base_medida() {
    // El fixture gigante no cabe en el repo: se comprueba el binario real
    // si esta presente en el entorno, y si no, se salta (NO VERIFICADO
    // aqui, pero medido en la maquina donde se genero).
    let ruta = std::path::Path::new("/workspace/agy-re/liblanguage_server.so");
    if !ruta.exists() {
        eprintln!("binario real no presente: test omitido");
        return;
    }
    let bytes = std::fs::read(ruta).expect("leer el binario");
    let rep = scan_container(&bytes).expect("escanear el contenedor");
    assert_eq!(
        rep.text_base_hint.as_deref(),
        Some("0x4d06cb8"),
        "la base de texto medida del binario real"
    );
    // Y el pclntab se encuentra dentro de .lrodata, no en .gopclntab.
    let cand = rep.pclntab_candidates.first().expect("candidato pclntab");
    assert_eq!(cand.section.as_deref(), Some(".lrodata"));
    assert_eq!(cand.nfunc, 161_682, "numero de funciones MEDIDO");
    assert_eq!(cand.names_resolved, 161_682, "los 161.682 nombres se resuelven");
}

#[test]
fn el_fixture_go1232_da_la_base_de_su_seccion_text() {
    // Fixture real del repo (Go 1.23.2 linux/amd64): aqui si hay una sola
    // seccion de texto, asi que la estimacion debe caer en ella.
    let ruta = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artifacts/fixture-go1.23.2-linux-amd64");
    if !ruta.exists() {
        eprintln!("fixture no presente: test omitido");
        return;
    }
    let bytes = std::fs::read(&ruta).expect("leer el fixture");
    let rep = scan_container(&bytes).expect("escanear el fixture");
    let hint = rep
        .text_base_hint
        .as_deref()
        .and_then(|h| h.strip_prefix("0x"))
        .and_then(|h| u64::from_str_radix(h, 16).ok())
        .expect("base estimada");
    let texto = rep
        .sections
        .iter()
        .find(|s| s.name == ".text")
        .expect("seccion .text");
    let addr_texto = texto
        .section_address
        .strip_prefix("0x")
        .and_then(|h| u64::from_str_radix(h, 16).ok())
        .expect("direccion de .text");
    assert_eq!(hint, addr_texto);
}
