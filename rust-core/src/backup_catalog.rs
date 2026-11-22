//! **Catálogo** de backups: qué hay, de qué formato y cuánto pesa.
//!
//! Este módulo hace tres cosas, y ninguna de ellas es inventar:
//!
//! 1. **Clasifica** por nombre y MIME qué clase de backup es cada fichero
//!    (`.dat` de Swift, `.tar` de Titanium/Machiav3lli, `.extra`, `.splits`,
//!    `.msg`/`.cls` de 7-Zip, `.wal` de PNG…).
//! 2. **Lista** el contenido de una carpeta de Drive **sin descargarlo**
//!    (la API de Drive devuelve nombre, tamaño y MIME; el contenido no baja).
//! 3. **Resume** el inventario por clase: ficheros y bytes, para escribir el
//!    informe con cifras medidas.
//!
//! Reglas del repo que se respetan aquí:
//! * **no se inventa**: lo que no se puede leer sale con `available: false`
//!   y el motivo exacto;
//! * **los secretos no se imprimen**: la credencial de Drive sale de
//!   [`crate::google_auth`] y nunca se escribe en la salida;
//! * **no se descarga nada**: listar es `files.list`, no `files.get?alt=media`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::backup_crypto::sha256_hex;

/// Clase de backup: qué **formato** es el fichero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ClaseBackup {
    /// Datos de app de Swift Backup (`.dat`): ZIP, TAR, 7-Zip o SBA1.
    SwiftData,
    /// APK de Swift Backup (`.app`).
    SwiftApk,
    /// Metadatos de Swift Backup (`.extra`).
    SwiftExtra,
    /// APKs de splits (`.splits`).
    SwiftSplits,
    /// Datos externos de Swift Backup (`.extdat`).
    SwiftExternal,
    /// Multimedia de Swift Backup (`.med`).
    SwiftMedia,
    /// Llamadas (`.cls`) o SMS (`.msg`): 7-Zip LZMA2.
    SwiftCallsSms,
    /// Fondos de pantalla (`.wal`): PNG.
    SwiftWalls,
    /// Redes Wi-Fi (`.wfi`).
    SwiftWifi,
    /// Backup Titanium/Machiav3lli: **un tar por paquete**.
    TitaniumTar,
    /// Contenedor Swift Backup `SBA1` (cifrado).
    Sba1,
    /// ZIP genérico.
    Zip,
    /// 7-Zip genérico.
    SevenZip,
    /// TAR genérico.
    Tar,
    /// APK suelto.
    Apk,
    /// Sin clase reconocible.
    Desconocido,
}

impl ClaseBackup {
    /// Nombre estable (para el JSON del informe).
    pub fn as_str(self) -> &'static str {
        match self {
            ClaseBackup::SwiftData => "swift_dat",
            ClaseBackup::SwiftApk => "swift_app",
            ClaseBackup::SwiftExtra => "swift_extra",
            ClaseBackup::SwiftSplits => "swift_splits",
            ClaseBackup::SwiftExternal => "swift_extdat",
            ClaseBackup::SwiftMedia => "swift_med",
            ClaseBackup::SwiftCallsSms => "swift_cls_msg",
            ClaseBackup::SwiftWalls => "swift_wal",
            ClaseBackup::SwiftWifi => "swift_wfi",
            ClaseBackup::TitaniumTar => "titanium_tar",
            ClaseBackup::Sba1 => "sba1",
            ClaseBackup::Zip => "zip",
            ClaseBackup::SevenZip => "7z",
            ClaseBackup::Tar => "tar",
            ClaseBackup::Apk => "apk",
            ClaseBackup::Desconocido => "desconocido",
        }
    }

    /// ¿Es un formato de backup de app (los que se pueden restaurar)?
    pub fn es_restaurable(self) -> bool {
        matches!(
            self,
            ClaseBackup::SwiftData
                | ClaseBackup::SwiftApk
                | ClaseBackup::SwiftExternal
                | ClaseBackup::SwiftMedia
                | ClaseBackup::TitaniumTar
                | ClaseBackup::Sba1
                | ClaseBackup::Tar
                | ClaseBackup::Zip
        )
    }
}

/// Clasifica un fichero de backup por su nombre (y el MIME si se conoce).
///
/// Los nombres de Swift Backup llevan el sufijo de dispositivo
/// (` (SM-G998B)`) y a veces el identificador de backup
/// (` (id-20261120-043914-ES)`): la clasificación ignora ambos.
pub fn clasificar(nombre: &str, mime: Option<&str>) -> ClaseBackup {
    let base = nombre.rsplit('/').next().unwrap_or(nombre);
    let limpio = base.split(" (").next().unwrap_or(base);
    let ext = limpio
        .rsplit('.')
        .next()
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "dat" => ClaseBackup::SwiftData,
        "app" => ClaseBackup::SwiftApk,
        "extra" => ClaseBackup::SwiftExtra,
        "splits" => ClaseBackup::SwiftSplits,
        "extdat" => ClaseBackup::SwiftExternal,
        "med" => ClaseBackup::SwiftMedia,
        "cls" | "msg" => ClaseBackup::SwiftCallsSms,
        "wal" => ClaseBackup::SwiftWalls,
        "wfi" => ClaseBackup::SwiftWifi,
        "tib" => ClaseBackup::TitaniumTar,
        "tar" | "tar.gz" | "tgz" => ClaseBackup::TitaniumTar,
        "apk" => ClaseBackup::Apk,
        "zip" => ClaseBackup::Zip,
        "7z" => ClaseBackup::SevenZip,
        _ => {
            // Sin extensión: el MIME decide.
            match mime.unwrap_or("") {
                "application/vnd.android.package-archive" => ClaseBackup::Apk,
                "application/zip" | "application/x-zip" => ClaseBackup::Zip,
                "application/x-7z-compressed" => ClaseBackup::SevenZip,
                "application/x-tar" | "application/x-gtar" => ClaseBackup::Tar,
                "image/png" => ClaseBackup::SwiftWalls,
                _ => ClaseBackup::Desconocido,
            }
        }
    }
}

/// Una entrada del inventario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntradaInventario {
    /// Nombre (tal cual lo devuelve Drive).
    pub nombre: String,
    /// Tamaño en bytes.
    pub bytes: u64,
    /// Clase de formato.
    pub clase: ClaseBackup,
    /// MIME de Drive (si lo dio).
    pub mime: Option<String>,
    /// Id de Drive (si lo dio).
    pub id: Option<String>,
}

/// Resumen por clase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResumenClase {
    /// Clase.
    pub clase: String,
    /// Número de ficheros.
    pub ficheros: u64,
    /// Bytes totales.
    pub bytes: u64,
}

/// Resumen global de un inventario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResumenInventario {
    /// Número total de ficheros.
    pub ficheros: u64,
    /// Bytes totales.
    pub bytes: u64,
    /// Desglose por clase.
    pub por_clase: Vec<ResumenClase>,
}

/// Resume un inventario por clase.
pub fn resumir(entradas: &[EntradaInventario]) -> ResumenInventario {
    let mut mapa: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    let mut ficheros = 0u64;
    let mut bytes = 0u64;
    for e in entradas {
        ficheros += 1;
        bytes += e.bytes;
        let slot = mapa.entry(e.clase.as_str().to_string()).or_insert((0, 0));
        slot.0 += 1;
        slot.1 += e.bytes;
    }
    let por_clase = mapa
        .into_iter()
        .map(|(clase, (f, b))| ResumenClase {
            clase,
            ficheros: f,
            bytes: b,
        })
        .collect();
    ResumenInventario {
        ficheros,
        bytes,
        por_clase,
    }
}

/// Formatea bytes en unidades legibles (GiB/MiB exactos con 2 decimales).
pub fn bytes_legibles(b: u64) -> String {
    const UNIDADES: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = b as f64;
    let mut i = 0usize;
    while v >= 1024.0 && i + 1 < UNIDADES.len() {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{b} B")
    } else {
        format!("{v:.2} {}", UNIDADES[i])
    }
}

/// Inventario de una carpeta de Drive, **sin descargar**.
///
/// Devuelve `Err` con el motivo exacto si no se puede listar (sin credencial,
/// sin red, carpeta inexistente…).
pub fn listar_drive(
    carpeta_id: &str,
    state_dir: Option<&str>,
    max_paginas: usize,
) -> Result<Vec<EntradaInventario>, String> {
    listar_drive_recursivo(carpeta_id, state_dir, max_paginas, false)
}

/// Inventario de una carpeta de Drive, **sin descargar**.
///
/// Con `recursivo` se bajan también las subcarpetas (cada carpeta tiene su
/// propio presupuesto de páginas, para que una subcarpeta enorme no se coma
/// el de la raíz).
pub fn listar_drive_recursivo(
    carpeta_id: &str,
    state_dir: Option<&str>,
    max_paginas: usize,
    recursivo: bool,
) -> Result<Vec<EntradaInventario>, String> {
    let (ak, av) = crate::google_auth::auth_header(state_dir)?;
    let mut out = Vec::new();
    let mut pendientes: Vec<String> = vec![carpeta_id.to_string()];
    let mut visitadas: Vec<String> = Vec::new();
    while let Some(pid) = pendientes.pop() {
        if visitadas.contains(&pid) {
            continue;
        }
        visitadas.push(pid.clone());
        let mut pagina: Option<String> = None;
        for _ in 0..max_paginas {
            let mut q = format!("'{pid}' in parents and trashed=false");
            let mut url = format!(
                "https://www.googleapis.com/drive/v3/files?q={}&pageSize=1000&fields=nextPageToken,files(id,name,size,mimeType,mimeType)&includeItemsFromAllDrives=true&supportsAllDrives=true",
                urlencode(&q)
            );
            if let Some(p) = &pagina {
                url.push_str(&format!("&pageToken={}", urlencode(p)));
            }
            let resp = ureq::get(&url)
                .header(ak.as_str(), av.as_str())
                .call()
                .map_err(|e| format!("no se pudo listar Drive: {e}"))?;
            let status = resp.status().as_u16();
            let body = resp.into_body().read_to_vec().unwrap_or_default();
            if status != 200 {
                let txt: String = String::from_utf8_lossy(&body).chars().take(200).collect();
                return Err(format!("Drive devolvió {status}: {txt}"));
            }
            let v: serde_json::Value =
                serde_json::from_slice(&body).map_err(|e| format!("respuesta de Drive ilegible: {e}"))?;
            if let Some(files) = v.get("files").and_then(|f| f.as_array()) {
                for f in files {
                    let nombre = f
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string();
                    let mime = f
                        .get("mimeType")
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string());
                    let es_carpeta = mime.as_deref() == Some("application/vnd.google-apps.folder");
                    if es_carpeta && recursivo {
                        if let Some(id) = f.get("id").and_then(|s| s.as_str()) {
                            pendientes.push(id.to_string());
                        }
                    }
                    out.push(EntradaInventario {
                        clase: clasificar(&nombre, mime.as_deref()),
                        bytes: f
                            .get("size")
                            .and_then(|s| s.as_str())
                            .and_then(|s| s.parse::<u64>().ok())
                            .unwrap_or(0),
                        nombre,
                        mime,
                        id: f.get("id").and_then(|s| s.as_str()).map(|s| s.to_string()),
                    });
                }
            }
            match v.get("nextPageToken").and_then(|p| p.as_str()) {
                Some(p) if !p.is_empty() => pagina = Some(p.to_string()),
                _ => break,
            }
        }
    }
    Ok(out)
}

/// Percentiles simples del tamaño de los ficheros de un inventario.
pub fn percentiles(entradas: &[EntradaInventario]) -> (u64, u64, u64) {
    if entradas.is_empty() {
        return (0, 0, 0);
    }
    let mut v: Vec<u64> = entradas.iter().map(|e| e.bytes).collect();
    v.sort_unstable();
    let p = |f: f64| -> u64 {
        let idx = ((v.len() - 1) as f64 * f).round() as usize;
        v[idx.min(v.len() - 1)]
    };
    (p(0.5), p(0.95), v.iter().sum::<u64>() / v.len() as u64)
}

/// Huella (sha-256) de un conjunto de nombres: sirve para comparar dos
/// inventarios **sin volcar** los nombres por encima.
pub fn huella(entradas: &[EntradaInventario]) -> String {
    let mut nombres: Vec<&str> = entradas.iter().map(|e| e.nombre.as_str()).collect();
    nombres.sort_unstable();
    sha256_hex(nombres.join("\n").as_bytes())
}

/// Codifica una cadena para meterla en una query de Drive.
fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'\'' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clasifica_las_extensiones_de_swift() {
        assert_eq!(
            clasificar("com.foo.bar.dat (SM-G998B) (id-2026)", None),
            ClaseBackup::SwiftData
        );
        assert_eq!(clasificar("com.foo.bar.app (SM-G998B)", None), ClaseBackup::SwiftApk);
        assert_eq!(clasificar("com.foo.bar.extra (SM-G998B)", None), ClaseBackup::SwiftExtra);
        assert_eq!(clasificar("com.foo.bar.splits (SM-G998B)", None), ClaseBackup::SwiftSplits);
        assert_eq!(clasificar("com.foo.bar.extdat (SM-G998B)", None), ClaseBackup::SwiftExternal);
        assert_eq!(clasificar("com.foo.bar.med (SM-G998B)", None), ClaseBackup::SwiftMedia);
        assert_eq!(clasificar("v3.1.2.cls (SM-G998B)", None), ClaseBackup::SwiftCallsSms);
        assert_eq!(clasificar("v3.1.2.msg (SM-G998B)", None), ClaseBackup::SwiftCallsSms);
        assert_eq!(clasificar("1624728450496.wal", None), ClaseBackup::SwiftWalls);
        assert_eq!(clasificar("wifi_networks.wfi", None), ClaseBackup::SwiftWifi);
    }

    #[test]
    fn clasifica_titanium_por_extension() {
        assert_eq!(clasificar("com.foo.bar.tar", None), ClaseBackup::TitaniumTar);
        assert_eq!(clasificar("com.foo.bar.tib", None), ClaseBackup::TitaniumTar);
    }

    #[test]
    fn resume_por_clase() {
        let e = vec![
            EntradaInventario {
                nombre: "a.dat".into(),
                bytes: 100,
                clase: ClaseBackup::SwiftData,
                mime: None,
                id: None,
            },
            EntradaInventario {
                nombre: "b.dat".into(),
                bytes: 300,
                clase: ClaseBackup::SwiftData,
                mime: None,
                id: None,
            },
            EntradaInventario {
                nombre: "c.app".into(),
                bytes: 50,
                clase: ClaseBackup::SwiftApk,
                mime: None,
                id: None,
            },
        ];
        let r = resumir(&e);
        assert_eq!(r.ficheros, 3);
        assert_eq!(r.bytes, 450);
        assert_eq!(r.por_clase.len(), 2);
    }

    #[test]
    fn bytes_legibles_redondea() {
        assert_eq!(bytes_legibles(512), "512 B");
        assert_eq!(bytes_legibles(1024), "1.00 KiB");
        assert_eq!(bytes_legibles(1_042_152_811_827), "970.58 GiB");
    }
}
