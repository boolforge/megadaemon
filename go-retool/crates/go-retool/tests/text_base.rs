//! Tests de la resolucion de la base de texto (text_base).
//!
//! CONTEXTO MEDIDO (2026-11-23, S21 Ultra -> sandbox):
//! Binario real: `liblanguage_server.so` de la IDE Antigravity (Google),
//! 181.358.800 B, ELF64 AArch64 PIE **stripped**, Go **1.28-20260921-RC02**
//! (layout de pclntab 1.20, minLC=4), con la pclntab **dentro de
//! `.lrodata`** (offset 0x1449ff0) y **sin** seccion `.gopclntab`.
//!
//! HECHOS MEDIDOS QUE ESTE TEST FIJA:
//! 1. `pcHeader.textStart` vale **0** en una biblioteca compartida: el
//!    linker no puede conocer la VA final del texto.
//! 2. La base de texto REAL es **0x4d0f0b0** = `moduledata.text`, que se
//!    lee de la seccion `.go.module` del propio binario.
//! 3. La estimacion por secciones (menor direccion ".text*") da
//!    **0x4d06cb8** = `.text.hot`, una seccion de **TAMANO 0** => **error de
//!    0x83F8 bytes** en TODAS las direcciones exportadas.
//! 4. ERROR REAL QUE SE PRODUJO: con 0x4d06cb8, el pseudocodigo de
//!    `defaultAdminSettingsPathForOS` salia con los nombres de las
//!    herramientas ("send_message", "manage_inbox", ...) porque el
//!    desensamblado apuntaba 0x83F8 bytes antes de la funcion correcta.
//!    Con la base real, esa funcion referencia "/etc/antigravity/
//!    admin_settings.json" y `defaultUserSettingsPath` referencia
//!    ".antigravity" + "settings.json" (MEDIDO).
//! 5. TRAMPA: comparar "go-retool" con "unstrip" NO valida nada, porque las
//!    dos implementaciones compartian la misma heuristica erronea y por eso
//!    coincidian en las 161.682 direcciones.  La validacion real son los
//!    invariantes del runtime: `base+entryoff[0] == moduledata.minpc` y
//!    `base+sentinel == moduledata.maxpc`.
//!
//! LO QUE NO SE AFIRMA (NO VERIFICADO):
//! * `unstrip` (Rust) parsea el binario pero con otro layout: por eso hace
//!   falta un parser propio.
//! * La herramienta de referencia `redress` **NO** soporta este binario
//!   ("no goversion found").

use go_retool::container::{scan_container, text_base_hint, SectionInfo};
use go_retool::moduledata::read_moduledata;

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
fn la_heuristica_por_secciones_sigue_existiendo_pero_se_sabe_equivocada() {
    // Reproduce el caso MEDIDO del binario real de Antigravity:
    // .text.hot (tamano 0) va ANTES que .text, y la estimacion se queda con
    // .text.hot -> 0x4d06cb8, que es 0x83F8 bytes MENOS que la real
    // (0x4d0f0b0).  Esta heuristica es SOLO el respaldo cuando el binario no
    // trae `.go.module`.
    let secciones = vec![
        sec(".text.hot", "0x4d06cb8", 0),
        sec(".text", "0x4d0f000", 44_578_320),
        sec(".text.unlikely", "0x7792610", 13_872),
    ];
    assert_eq!(text_base_hint(&secciones), Some(0x4d06cb8));
    assert_ne!(
        text_base_hint(&secciones),
        Some(0x4d0f0b0),
        "la heuristica NO es la base real: por eso el moduledata la pisa"
    );
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
    // si esta presente en el entorno, y si no, se salta.
    let ruta = std::path::Path::new("/workspace/agy-re/liblanguage_server.so");
    if !ruta.exists() {
        eprintln!("binario real no presente: test omitido");
        return;
    }
    let bytes = std::fs::read(ruta).expect("leer el binario");
    let rep = scan_container(&bytes).expect("escanear el contenedor");
    // MEDIDO: moduledata.text = 0x4d0f0b0 (no la estimacion 0x4d06cb8).
    assert_eq!(
        rep.text_base_hint.as_deref(),
        Some("0x4d0f0b0"),
        "la base de texto medida del binario real"
    );
    assert!(
        rep.text_base_source
            .as_deref()
            .unwrap_or("")
            .contains("moduledata"),
        "la base debe venir del moduledata, no de la heuristica: {:?}",
        rep.text_base_source
    );
    // Y el pclntab se encuentra dentro de .lrodata, no en .gopclntab.
    let cand = rep.pclntab_candidates.first().expect("candidato pclntab");
    assert_eq!(cand.section.as_deref(), Some(".lrodata"));
    assert_eq!(cand.nfunc, 161_682, "numero de funciones MEDIDO");
    assert_eq!(cand.names_resolved, 161_682, "los 161.682 nombres se resuelven");
}

#[test]
fn el_moduledata_del_binario_real_cuadra_con_la_base() {
    // Invariantes del runtime de Go: la base tiene que hacer que la
    // primera y la ultima funcion caigan donde dice el moduledata.
    let ruta = std::path::Path::new("/workspace/agy-re/liblanguage_server.so");
    if !ruta.exists() {
        eprintln!("binario real no presente: test omitido");
        return;
    }
    let bytes = std::fs::read(ruta).expect("leer el binario");
    let md = read_moduledata(&bytes).expect("moduledata del binario real");
    assert_eq!(md.text, 0x4d0f0b0);
    assert_eq!(md.minpc, 0x4d0f0b0, "minpc == text");
    assert_eq!(md.maxpc, 0x71d0dd1);
    assert!(md.etext > md.text);
    // La base heuristica NO cumple los invariantes (por eso se cambio).
    assert!(go_retool::moduledata::verify_text_base(&md, 0x4d06cb8, 0, 0x24c1d21).is_err());
    // La base real SI.
    assert!(
        go_retool::moduledata::verify_text_base(&md, 0x4d0f0b0, 0, 0x24c1d21).is_ok(),
        "con la base real los dos invariantes se cumplen"
    );
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
