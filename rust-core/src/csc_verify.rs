//! **Verificador post-parche del CSC**: ¿el framework del móvil se ha quedado
//! con la bandera de grabación nativa de llamadas?
//!
//! # Por qué existe
//!
//! El parche del CSC (añadir `<CscFeature_VoiceCall_ConfigRecording>` al
//! `cscfeature.xml` del CSC activo) se puede aplicar y **aún así no servir**
//! de nada: si el framework no lo lee (cacheado, otra ruta, verificación de
//! `contents.db`), el móvil sigue sin la feature. Esta tool **mide** el estado
//! real en el dispositivo en vez de asumir que el parche funcionó.
//!
//! # Qué comprueba
//!
//! 1. **CSC activo**: `mps_code.dat` (3 bytes) ⇒ p. ej. `AMO`.
//! 2. **Fichero OMC del CSC activo**: lo descifra (OMC = rotación+XOR posicional
//!    + gzip, ver `rom_migration::omc_decode_gzip`) y lee el valor de la
//!    bandera.
//! 3. **Inventario `contents.db`**: guarda el `fileSize` de cada fichero del
//!    CSC ⇒ si el XML parcheado tiene otro tamaño, **el sistema puede
//!    detectarlo** ⇒ se reporta si coincide.
//! 4. **Copias en claro**: si el framework dejó una copia *descifrada* del
//!    XML (caché de arranque), se lee también ⇒ esa es la **prueba de runtime**.
//! 5. **Matriz de CSC**: para todos los CSC presentes en el dispositivo, si
//!    traen la bandera y con qué valor ⇒ el mapa vivo (19 de 126 en el
//!    firmware medido).
//!
//! Uso: `megadaemon-core csc-verify`

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::rom_migration::{
    csc_feature_get, omc_decode_gzip, CSC_ROOT, CSC_SYSTEM_ROOT, MPS_CODE_PATH,
};

/// La bandera que decide si el móvil graba llamadas de forma nativa.
pub const FEATURE: &str = "CscFeature_VoiceCall_ConfigRecording";

/// Valor de la feature **sin** disclaimer (variante "sin aviso").
/// OJO: el verificador NO presupone cual de las dos variantes es la legal:
/// las compara y las exige por parametro.
pub const VARIANT_PLAIN: &str = "RecordingAllowed";
/// Prefijo de las variantes **con** disclaimer (con aviso al usuario).
pub const VARIANT_DISCLAIMER: &str = "RecordingAllowedByMenu";

/// Clasifica el valor de la feature: `sin-disclaimer` | `con-disclaimer` |
/// `apagado` | `otro-valor`.
pub fn classify_variant(value: &str) -> &'static str {
    let v = value.trim();
    if v.is_empty() || v.eq_ignore_ascii_case("false") || v == "0" {
        return "apagado";
    }
    if v.starts_with(VARIANT_DISCLAIMER) {
        return "con-disclaimer";
    }
    if v.starts_with(VARIANT_PLAIN) {
        return "sin-disclaimer";
    }
    "otro-valor"
}

/// Estado de una copia en claro del XML de CSC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeCopy {
    pub path: String,
    pub bytes: u64,
    /// ¿Es XML legible (o sea, ya descifrado por el framework)?
    pub plaintext: bool,
    /// Valor de la bandera en esa copia.
    pub feature: Option<String>,
}

/// Fila CSC ↔ bandera.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CscRow {
    pub csc: String,
    pub bytes: u64,
    pub has_feature: bool,
    pub value: Option<String>,
    /// Valor que se **espera** encontrar (lo exige quien llama, no el modulo).
    /// ¿Coincide el valor presente con el esperado?
    /// `sin-disclaimer` | `con-disclaimer` | `apagado` | `otro-valor`.
    pub decode_error: Option<String>,
}

/// Comprobación del inventario `contents.db`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentsDbCheck {
    pub path: String,
    pub db_path: String,
    pub expected_size: Option<i64>,
    pub real_size: Option<u64>,
    pub matches: Option<bool>,
    pub error: Option<String>,
}

/// Informe completo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CscVerifyReport {
    /// CSC activo leído de `mps_code.dat`.
    pub csc_code: Option<String>,
    /// La bandera vigilada.
    pub feature: String,
    /// `ON` | `OFF` | `UNKNOWN`.
    pub verdict: String,
    /// Valor de la bandera en el CSC activo.
    pub value: Option<String>,
    /// Valor que se **espera** encontrar (lo exige quien llama, no el modulo).
    pub expected_value: Option<String>,
    /// ¿Coincide el valor presente con el esperado?
    pub value_matches: Option<bool>,
    /// `sin-disclaimer` | `con-disclaimer` | `apagado` | `otro-valor`.
    pub variant: Option<String>,
    /// Fichero del CSC activo del que se leyó.
    pub source: Option<String>,
    /// `omc` | `plain`.
    pub source_encoding: Option<String>,
    pub decode_error: Option<String>,
    pub file_bytes: Option<u64>,
    /// ¿Coincide el tamaño con el inventario?
    pub contents_db: Option<ContentsDbCheck>,
    /// Copias en claro halladas (prueba de runtime).
    pub runtime_copies: Vec<RuntimeCopy>,
    /// Todos los CSC del dispositivo y si traen la bandera.
    pub csc_matrix: Vec<CscRow>,
    /// Notas legibles.
    pub notes: Vec<String>,
}

/// Lee el CSC activo de `mps_code.dat` (3 bytes, sin NUL).
pub fn read_csc_code() -> Option<String> {
    let raw = std::fs::read(MPS_CODE_PATH).ok()?;
    let s: String = raw
        .iter()
        .map(|&b| if b == 0 { ' ' } else { b as char })
        .collect();
    let t = s.trim().to_string();
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

/// Rutas candidatas del XML de CSC.
pub fn csc_xml_candidates(csc: &str) -> Vec<PathBuf> {
    vec![
        PathBuf::from(format!(
            "{}/{}/conf/system/cscfeature.xml",
            CSC_SYSTEM_ROOT, csc
        )),
        PathBuf::from(format!("{}/{}/cscfeature.xml", CSC_SYSTEM_ROOT, csc)),
        PathBuf::from(format!("{}/{}/conf/system/cscfeature.xml", CSC_ROOT, csc)),
    ]
}

/// Descifra el fichero OMC (o lo lee como XML plano si ya está descifrado).
///
/// Devuelve `(xml, encoding, error)`.
pub fn decode_csc_file(bytes: &[u8]) -> (Option<String>, &'static str, Option<String>) {
    // Si ya es XML plano (el framework lo descifró), se usa tal cual.
    if bytes.starts_with(b"<?xml") || bytes.starts_with(b"<SamsungMobileFeature") {
        return (
            Some(String::from_utf8_lossy(bytes).to_string()),
            "plain",
            None,
        );
    }
    match omc_decode_gzip(bytes) {
        Ok(xml) => (Some(xml), "omc", None),
        Err(e) => (None, "omc", Some(e)),
    }
}

/// Lee la bandera de un fichero OMC de CSC.
pub fn feature_of_file(path: &Path) -> (Option<String>, Option<u64>, &'static str, Option<String>) {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return (None, None, "none", Some(e.to_string())),
    };
    let len = bytes.len() as u64;
    let (xml, enc, err) = decode_csc_file(&bytes);
    match xml {
        Some(x) => (csc_feature_get(&x, FEATURE), Some(len), enc, err),
        None => (None, Some(len), enc, err),
    }
}

/// Comprueba el `fileSize` que el inventario `contents.db` espera para el XML.
pub fn check_contents_db(csc: &str, real_size: Option<u64>) -> Option<ContentsDbCheck> {
    let db_path = format!("{}/{}/contents.db", CSC_ROOT, csc);
    if !Path::new(&db_path).exists() {
        return None;
    }
    let mut chk = ContentsDbCheck {
        path: db_path.clone(),
        db_path: db_path.clone(),
        expected_size: None,
        real_size,
        matches: None,
        error: None,
    };
    match rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    ) {
        Ok(con) => {
            let sql = "SELECT filePath, fileSize FROM contents WHERE filePath LIKE '%cscfeature.xml'";
            let mut st = match con.prepare(sql) {
                Ok(s) => s,
                Err(e) => {
                    chk.error = Some(format!("prepare: {e}"));
                    return Some(chk);
                }
            };
            let rows = st.query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            });
            match rows {
                Ok(it) => {
                    for row in it.flatten() {
                        if row.0.contains(csc) || row.0.contains("cscfeature.xml") {
                            chk.expected_size = Some(row.1);
                            break;
                        }
                    }
                }
                Err(e) => chk.error = Some(format!("query: {e}")),
            }
        }
        Err(e) => chk.error = Some(format!("open: {e}")),
    }
    chk.matches = match (chk.expected_size, real_size) {
        (Some(exp), Some(real)) => Some(exp == real as i64),
        _ => None,
    };
    Some(chk)
}

/// Busca copias **en claro** del XML de CSC (la prueba de que el framework lo
/// leyó y lo descifró). Búsqueda acotada: profundidad ≤ 3 y sin `/data/user`.
fn find_runtime_copies() -> Vec<RuntimeCopy> {
    let roots = [
        "/data/system",
        "/data/vendor_deploy",
        "/data/misc",
        "/system/etc",
        "/vendor/etc",
        "/odm/etc",
        "/data/local/tmp",
    ];
    let mut out = Vec::new();
    fn walk(dir: &Path, depth: usize, out: &mut Vec<RuntimeCopy>) {
        if depth == 0 || out.len() >= 12 {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("user") || name.starts_with("user_") {
                continue;
            }
            if p.is_dir() {
                walk(&p, depth - 1, out);
            } else if name.eq_ignore_ascii_case("cscfeature.xml") {
                let Ok(md) = e.metadata() else { continue };
                let bytes = md.len();
                let plaintext = std::fs::read(&p)
                    .map(|b| b.starts_with(b"<?xml") || b.starts_with(b"<SamsungMobileFeature"))
                    .unwrap_or(false);
                let feature = if plaintext {
                    std::fs::read_to_string(&p)
                        .ok()
                        .and_then(|x| csc_feature_get(&x, FEATURE))
                } else {
                    None
                };
                out.push(RuntimeCopy {
                    path: p.to_string_lossy().to_string(),
                    bytes,
                    plaintext,
                    feature,
                });
            }
        }
    }
    for r in roots {
        walk(Path::new(r), 3, &mut out);
    }
    out
}

/// Mapa vivo: todos los CSC presentes y si traen la bandera.
pub fn csc_matrix() -> Vec<CscRow> {
    let mut rows = Vec::new();
    let Ok(rd) = std::fs::read_dir(CSC_SYSTEM_ROOT) else {
        return rows;
    };
    for e in rd.flatten() {
        let csc = e.file_name().to_string_lossy().to_string();
        let p = e.path().join("conf/system/cscfeature.xml");
        if !p.exists() {
            continue;
        }
        let bytes = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        let (val, _len, _enc, err) = feature_of_file(&p);
        rows.push(CscRow {
            has_feature: val.is_some(),
            value: val.clone(),
            csc,
            bytes,
            decode_error: err,
        });
    }
    rows.sort_by(|a, b| a.csc.cmp(&b.csc));
    rows
}

/// Verificación completa.
pub fn verify() -> CscVerifyReport {
    verify_expecting(None)
}

/// Igual que [`verify`], pero **exige** un valor concreto de la feature.
///
/// No presupone si el valor correcto es el de disclaimer o el que no lo lleva:
/// el llamante dice que espera y el verificador comprueba, compara y anota
/// la discrepancia (veredicto `MISMATCH`).
pub fn verify_expecting(expect: Option<&str>) -> CscVerifyReport {
    let mut notes: Vec<String> = Vec::new();
    let mut report = CscVerifyReport {
        csc_code: read_csc_code(),
        feature: FEATURE.to_string(),
        verdict: "UNKNOWN".to_string(),
        value: None,
        expected_value: expect.map(|s| s.to_string()),
        value_matches: None,
        variant: None,
        source: None,
        source_encoding: None,
        decode_error: None,
        file_bytes: None,
        contents_db: None,
        runtime_copies: Vec::new(),
        csc_matrix: Vec::new(),
        notes: Vec::new(),
    };

    // 1) CSC activo + fichero
    let mut value: Option<String> = None;
    if let Some(csc) = report.csc_code.clone() {
        for cand in csc_xml_candidates(&csc) {
            if !cand.exists() {
                continue;
            }
            let (val, len, enc, err) = feature_of_file(&cand);
            report.source = Some(cand.to_string_lossy().to_string());
            report.source_encoding = Some(enc.to_string());
            report.file_bytes = len;
            report.decode_error = err;
            value = val.clone();
            break;
        }
        if report.source.is_none() {
            notes.push(format!(
                "no se encontró el cscfeature.xml del CSC {} (probados: {})",
                csc,
                csc_xml_candidates(&csc)
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        // 2) inventario
        report.contents_db = check_contents_db(&csc, report.file_bytes);
        if let Some(c) = &report.contents_db {
            match (c.expected_size, c.matches) {
                (Some(exp), Some(false)) => notes.push(format!(
                    "ATENCION: contents.db espera {} B para el cscfeature.xml pero el fichero pesa {} B => \
                     un parche de otro tamaño se detecta en el inventario",
                    exp,
                    report.file_bytes.unwrap_or(0)
                )),
                (Some(exp), Some(true)) => notes.push(format!(
                    "contents.db coincide ({} B) => el inventario no delata el cambio",
                    exp
                )),
                _ => {}
            }
        }
    } else {
        notes.push(format!(
            "no se pudo leer el CSC activo en {}",
            MPS_CODE_PATH
        ));
    }

    // 3) copias en claro (prueba de runtime)
    report.runtime_copies = find_runtime_copies();
    for c in &report.runtime_copies {
        if let Some(v) = &c.feature {
            notes.push(format!(
                "copia EN CLARO del framework en {} => valor {}",
                c.path, v
            ));
            if value.is_none() {
                value = Some(v.clone());
            }
        }
    }

    // 4) matriz de CSC
    report.csc_matrix = csc_matrix();
    let con_grab = report
        .csc_matrix
        .iter()
        .filter(|r| r.has_feature)
        .count();
    notes.push(format!(
        "{} de {} CSC presentes traen {}",
        con_grab,
        report.csc_matrix.len(),
        FEATURE
    ));

    // 5) veredicto
    report.value = value.clone();
    report.verdict = match value.as_deref() {
        Some(v) if !v.is_empty() && !v.eq_ignore_ascii_case("false") && v != "0" => "ON".to_string(),
        Some(_) => "OFF".to_string(),
        None => {
            if report.source.is_some() {
                "OFF".to_string()
            } else {
                "UNKNOWN".to_string()
            }
        }
    };
    if report.verdict == "ON" {
        notes.push("la feature está presente en el fichero del CSC activo (o en su copia en claro)".to_string());
    } else if report.verdict == "OFF" {
        notes.push("la feature NO está presente en el CSC activo => la grabación nativa está apagada".to_string());
    }
    // 6) variante y comprobacion del valor exigido
    report.variant = report.value.as_deref().map(classify_variant).map(|v| v.to_string());
    if let (Some(exp), Some(actual)) = (report.expected_value.clone(), report.value.clone()) {
        let ok = actual.trim() == exp.trim();
        report.value_matches = Some(ok);
        if ok {
            notes.push(format!("el valor presente ({:?}) es el esperado", actual));
        } else {
            notes.push(format!(
                "ATENCION: valor presente {:?} != valor esperado {:?}",
                actual, exp
            ));
            if report.verdict == "ON" {
                report.verdict = "MISMATCH".to_string();
            }
        }
    }
    report.notes = notes;
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_feature_vigilada_es_la_de_grabacion() {
        assert_eq!(FEATURE, "CscFeature_VoiceCall_ConfigRecording");
    }

    #[test]
    fn un_xml_plano_se_lee_tal_cual() {
        let xml = b"<?xml version='1.0'?><SamsungMobileFeature><CscFeature_VoiceCall_ConfigRecording>RecordingAllowed</CscFeature_VoiceCall_ConfigRecording></SamsungMobileFeature>";
        let (x, enc, err) = decode_csc_file(xml);
        assert_eq!(enc, "plain");
        assert!(err.is_none());
        assert_eq!(
            csc_feature_get(&x.unwrap(), FEATURE).as_deref(),
            Some("RecordingAllowed")
        );
    }

    #[test]
    fn un_omc_corrupto_no_revienta() {
        let (x, enc, err) = decode_csc_file(&[0x2f, 0x27, 0x29, 0xb7, 0x00, 0x01]);
        assert_eq!(enc, "omc");
        assert!(x.is_none());
        assert!(err.is_some());
    }

    #[test]
    fn las_candidatas_cubren_el_csc_activo() {
        let c = csc_xml_candidates("AMO");
        assert!(c.iter().any(|p| p.to_string_lossy().contains("/AMO/")));
        assert!(c
            .iter()
            .any(|p| p.to_string_lossy().ends_with("conf/system/cscfeature.xml")));
    }
}
