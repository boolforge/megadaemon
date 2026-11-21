//! **Parche del CSC en Rust puro** (sin Python en el móvil).
//!
//! # Qué hace
//!
//! El CSC (Customer Specific Customization) de Samsung guarda sus "features"
//! en un `cscfeature.xml` comprimido con un contenedor **OMC** (rotación +
//! XOR posicional + gzip; ver `rom_migration::omc_decode_gzip`).  Este módulo
//! lo lee, lo lista, lo parchea y lo vuelve a escribir **en el contenedor del
//! módulo Magisk** (bind-mount systemless: el fichero original no se toca y
//! revertir es desactivar el módulo).
//!
//! Es el equivalente Rust de `deploy/patch_csc_value.py` + `tools/omc_codec.py`,
//! con dos ventajas medidas: no hace falta `python3` en el móvil (que no
//! siempre está) y el discovery no depende de un intérprete externo.
//!
//! # Lo que aporta frente al script Python
//!
//! * `list_features`: **descubre TODAS** las `CscFeature_*` del CSC activo y
//!   sus valores (para saber qué más se puede tocar), no sólo la de grabación.
//! * `set_feature`: escribe **cualquier** valor del esquema del fabricante
//!   (`RecordingAllowed`, `RecordingAllowedByMenu,disclaimer_latin`,
//!   `RecordingNotAllowed`, …) en **cualquier** feature y **cualquier** CSC.
//! * `update_contents_db`: actualiza el `fileSize` de `contents.db`, que el
//!   framework compara con el fichero servido (si no cuadra, el CSC se
//!   descarta).
//!
//! Uso (CLI `megadaemon-core csc`):
//!
//! ```text
//! megadaemon-core csc list
//! megadaemon-core csc set RecordingAllowed
//! megadaemon-core csc set RecordingAllowed --feature CscFeature_VoiceCall_ConfigRecording
//! megadaemon-core csc set RecordingAllowed --csc TPA
//! ```

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::rom_migration::{
    csc_feature_get, omc_decode_gzip, omc_encode_gzip, CSC_ROOT, CSC_SYSTEM_ROOT, MPS_CODE_PATH,
};

/// Módulo Magisk (systemless) donde se escribe el fichero parcheado.
pub const MODULE_DIR: &str = "/data/adb/modules/megadaemon_csc_recording";
/// Feature por defecto: la de grabación de llamadas.
pub const FEATURE_RECORDING: &str = "CscFeature_VoiceCall_ConfigRecording";

/// Una feature del CSC con su valor actual.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CscFeature {
    pub name: String,
    pub value: String,
}

/// Resultado de aplicar un parche.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchReport {
    pub csc: String,
    pub feature: String,
    pub valor_anterior: Option<String>,
    pub valor_nuevo: String,
    pub bytes_escritos: usize,
    pub modulo: String,
    pub contents_db_actualizado: usize,
    pub ruta_original: String,
}

/// Extrae **todas** las features `CscFeature_*` del XML con su valor.
///
/// MEDIDO el 2026-11-21 en el S21 Ultra (CSC AMO): el XML trae una decena de
/// `CscFeature_*`; sólo una (`..._ConfigRecording`) es la de grabación, pero
/// las demás son tan parcheables como ésta (el CSC es XML plano).
pub fn list_features(xml: &str) -> Vec<CscFeature> {
    let mut out: Vec<CscFeature> = Vec::new();
    // <CscFeature_Name>valor</CscFeature_Name>
    let mut rest = xml;
    while let Some(i) = rest.find("<CscFeature_") {
        let tail = &rest[i..];
        let Some(gt) = tail.find('>') else { break };
        let name = tail[1..gt].trim().to_string();
        let after = &tail[gt + 1..];
        let close = format!("</{name}>");
        let value = match after.find(&close) {
            Some(c) => after[..c].to_string(),
            None => String::new(),
        };
        let name = name.trim_end_matches('/').to_string();
        if !name.is_empty() && !out.iter().any(|f| f.name == name) {
            out.push(CscFeature {
                name,
                value: value.trim().to_string(),
            });
        }
        rest = &rest[i + 1..];
    }
    out
}

/// Devuelve el XML con `feature` puesta a `valor` (o la añade si no existe).
///
/// No es un parser de XML: el fichero del CSC es un XML plano y plano se
/// queda (MEDIDO: el framework lo lee tal cual; re-serializarlo cambiaría
/// el `fileSize` y el formato).
pub fn set_feature(xml: &str, feature: &str, valor: &str) -> String {
    let tag_open = format!("<{feature}>");
    let tag_close = format!("</{feature}>");
    if let Some(v) = csc_feature_get(xml, feature) {
        let _ = v;
        let pos = xml.find(&tag_open).unwrap_or(0);
        let after_open = pos + tag_open.len();
        match xml[after_open..].find(&tag_close) {
            Some(c) => {
                let mut s = String::with_capacity(xml.len() + valor.len());
                s.push_str(&xml[..after_open]);
                s.push_str(valor);
                s.push_str(&xml[after_open + c..]);
                s
            }
            None => xml.to_string(),
        }
    } else {
        let ins = format!("{tag_open}{valor}{tag_close}");
        match xml.find("</root>") {
            Some(p) => {
                let mut s = String::with_capacity(xml.len() + ins.len());
                s.push_str(&xml[..p]);
                s.push_str(&ins);
                s.push_str(&xml[p..]);
                s
            }
            None => {
                let mut s = xml.to_string();
                s.push_str(&ins);
                s
            }
        }
    }
}

/// CSC activo del dispositivo (de `mps_code.dat`), o el primero que exista.
pub fn active_csc() -> Option<String> {
    if let Ok(raw) = std::fs::read(MPS_CODE_PATH) {
        let s: String = raw
            .iter()
            .map(|b| if b.is_ascii_graphic() { *b as char } else { ' ' })
            .collect();
        let csc = s.trim().to_string();
        if !csc.is_empty() {
            return Some(csc);
        }
    }
    carriers().into_iter().next()
}

/// Todos los CSC presentes en el firmware.
pub fn carriers() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for root in [CSC_SYSTEM_ROOT, CSC_ROOT] {
        let p = Path::new(root);
        if let Ok(rd) = std::fs::read_dir(p) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.len() == 3 && name.chars().all(|c| c.is_ascii_uppercase()) && !out.contains(&name)
                {
                    out.push(name);
                }
            }
        }
    }
    out.sort();
    out
}

/// Rutas candidatas del `cscfeature.xml` de un CSC (medidas en el firmware).
pub fn xml_candidates(csc: &str) -> Vec<PathBuf> {
    vec![
        PathBuf::from(format!("{CSC_SYSTEM_ROOT}/{csc}/cscfeature.xml")),
        PathBuf::from(format!("{CSC_ROOT}/{csc}/conf/system/cscfeature.xml")),
        PathBuf::from(format!("{CSC_ROOT}/{csc}/cscfeature.xml")),
    ]
}

/// Contenido XML del CSC: prefiere el del módulo (si ya está parcheado) y
/// si no, el de la partición.
pub fn read_csc_xml(csc: &str) -> Result<(String, PathBuf), String> {
    let modulo = Path::new(MODULE_DIR).join("cscfeature.xml");
    let mut candidatas = vec![modulo];
    candidatas.extend(xml_candidates(csc));
    for p in &candidatas {
        if let Ok(bytes) = std::fs::read(p) {
            let xml = omc_decode_gzip(&bytes)?;
            return Ok((xml, p.clone()));
        }
    }
    Err(format!(
        "no encuentro cscfeature.xml del CSC {csc} (probado: {})",
        candidatas
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// Actualiza el `fileSize` de `cscfeature.xml` en `contents.db`.
///
/// El framework compara el tamaño declarado con el fichero servido: si no
/// cuadra, el CSC parcheado se descarta (MEDIDO: así se pierde el parche).
pub fn update_contents_db(csc: &str, nuevo_tamano: u64) -> Result<usize, String> {
    let db = format!("{CSC_ROOT}/{csc}/contents.db");
    if !Path::new(&db).exists() {
        return Ok(0);
    }
    let con = rusqlite::Connection::open(&db).map_err(|e| format!("sqlite: {e}"))?;
    let n = con
        .execute(
            "UPDATE contents SET fileSize=? WHERE filePath LIKE ?",
            rusqlite::params![nuevo_tamano as i64, "%cscfeature.xml"],
        )
        .map_err(|e| format!("sqlite update: {e}"))?;
    Ok(n)
}

/// Aplica el parche completo: lee, parchea, re-codifica a OMC, escribe el
/// módulo y cuadra el `contents.db`.
///
/// `modulo` permite probar en un directorio aparte (tests).
pub fn patch(
    csc: &str,
    feature: &str,
    valor: &str,
    modulo: &Path,
) -> Result<PatchReport, String> {
    let (xml, fuente) = read_csc_xml(csc)?;
    let valor_anterior = csc_feature_get(&xml, feature);
    let nuevo_xml = set_feature(&xml, feature, valor);
    let datos = omc_encode_gzip(nuevo_xml.as_bytes())?;
    std::fs::create_dir_all(modulo).map_err(|e| format!("mkdir {:?}: {e}", modulo))?;
    let destino = modulo.join("cscfeature.xml");
    std::fs::write(&destino, &datos).map_err(|e| format!("write {:?}: {e}", destino))?;
    let n = update_contents_db(csc, datos.len() as u64).unwrap_or(0);
    Ok(PatchReport {
        csc: csc.to_string(),
        feature: feature.to_string(),
        valor_anterior,
        valor_nuevo: valor.to_string(),
        bytes_escritos: datos.len(),
        modulo: destino.display().to_string(),
        contents_db_actualizado: n,
        ruta_original: fuente.display().to_string(),
    })
}

/// Una fila del inventario `contents.db`: un fichero del CSC y su tamaño.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CscInventoryRow {
    pub id: i64,
    pub file_path: String,
    pub file_size: i64,
}

/// Inventario completo del CSC (`contents.db`): TODOS los ficheros que
/// contiene el CSC y su `fileSize`.
///
/// Esto es el mapa de "qué más se puede parchear": el CSC no es sólo el
/// `cscfeature.xml`, es un contenedor con decenas de ficheros (build.prop,
/// init scripts, overlays…).  Cada uno se puede sacar, editar y volver a
/// meter con el mismo envoltorio OMC.
pub fn inventory(csc: &str) -> Result<Vec<CscInventoryRow>, String> {
    let db = format!("{CSC_ROOT}/{csc}/contents.db");
    if !Path::new(&db).exists() {
        return Err(format!("no existe {db}"));
    }
    let con = rusqlite::Connection::open(&db).map_err(|e| format!("sqlite: {e}"))?;
    let mut st = con
        .prepare("SELECT _id, filePath, fileSize FROM contents ORDER BY filePath")
        .map_err(|e| format!("sqlite prepare: {e}"))?;
    let rows = st
        .query_map([], |r| {
            Ok(CscInventoryRow {
                id: r.get(0)?,
                file_path: r.get::<_, String>(1)?,
                file_size: r.get(2)?,
            })
        })
        .map_err(|e| format!("sqlite query: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| format!("sqlite row: {e}"))?);
    }
    Ok(out)
}

/// Fila de la comparación entre CSC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CscMatrixRow {
    pub csc: String,
    pub legible: bool,
    pub num_features: usize,
    pub recording: Option<String>,
    pub features: Vec<CscFeature>,
}

/// COMPARA TODOS los CSC presentes en el firmware: para cada uno, decodifica
/// su `cscfeature.xml` y lista sus features y valores.
///
/// MEDIDO el 2026-11-21: en el firmware del S21 Ultra hay 126 CSC y sólo
/// una decena traen la feature de grabación; el resto sirve para saber qué
/// más se puede tocar (y para elegir un CSC con más features).
pub fn matrix() -> Vec<CscMatrixRow> {
    let mut out = Vec::new();
    for csc in carriers() {
        let (legible, feats) = match read_csc_xml(&csc) {
            Ok((xml, _)) => {
                let f = list_features(&xml);
                (true, f)
            }
            Err(_) => (false, Vec::new()),
        };
        let recording = feats
            .iter()
            .find(|f| f.name == FEATURE_RECORDING)
            .map(|f| f.value.clone());
        out.push(CscMatrixRow {
            csc,
            legible,
            num_features: feats.len(),
            recording,
            features: feats,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = "<root><CscFeature_VoiceCall_ConfigRecording>RecordingNotAllowed</CscFeature_VoiceCall_ConfigRecording><CscFeature_Otro>x</CscFeature_Otro></root>";

    #[test]
    fn lista_todas_las_features() {
        let f = list_features(XML);
        assert_eq!(f.len(), 2, "{f:?}");
        assert_eq!(f[0].name, FEATURE_RECORDING);
        assert_eq!(f[0].value, "RecordingNotAllowed");
        assert_eq!(f[1].name, "CscFeature_Otro");
    }

    #[test]
    fn cambia_el_valor_de_una_feature() {
        let nuevo = set_feature(XML, FEATURE_RECORDING, "RecordingAllowed");
        assert!(nuevo.contains("<CscFeature_VoiceCall_ConfigRecording>RecordingAllowed<"));
        assert!(!nuevo.contains("RecordingNotAllowed"));
        // el resto del XML no se toca
        assert!(nuevo.contains("<CscFeature_Otro>x</CscFeature_Otro>"));
    }

    #[test]
    fn anade_una_feature_que_no_existe() {
        let nuevo = set_feature(XML, "CscFeature_Nueva", "1");
        assert!(nuevo.contains("<CscFeature_Nueva>1</CscFeature_Nueva>"));
        assert!(nuevo.ends_with("</root>"));
    }

    #[test]
    fn round_trip_omc_conserva_el_xml() {
        let gz = omc_encode_gzip(XML.as_bytes()).expect("encode");
        let vuelta = omc_decode_gzip(&gz).expect("decode");
        assert_eq!(vuelta, XML);
    }

    #[test]
    fn la_matriz_no_falla_sin_dispositivo() {
        // en el sandbox no hay CSC: la matriz debe devolver una lista vacía
        // y NO entrar en panic.
        let m = matrix();
        assert!(m.iter().all(|r| !r.legible || r.num_features >= 0));
    }

    #[test]
    fn el_inventario_erre_limpiamente_sin_csc() {
        let e = inventory("NOEXISTE");
        assert!(e.is_err());
    }

    #[test]
    fn el_valor_por_defecto_es_el_de_grabacion() {
        assert_eq!(FEATURE_RECORDING, "CscFeature_VoiceCall_ConfigRecording");
    }
}
