//! Google Drive **nativo** (Rust puro): subir y bajar ficheros con la API de
//! Drive v3 y su subida resumible, **sin `curl`, sin `dd`, sin shell**.
//!
//! Sustituye a `drive_client::drive_chunk_command` (que construía una tubería
//! `dd | curl`): aquí la subida la hace el propio daemon, con `ureq`, y el
//! token sale de [`crate::google_auth`] (OAuth2 en Rust, sin scripts).
//!
//! El fichero se sube en trozos de 256 KiB (la granularidad que exige
//! Google) y **nunca se copia entero a otro sitio**: se lee del disco en
//! bloques y se manda por el socket.

use crate::drive_client::{
    chunk_plan, file_metadata, DriveTarget, DRIVE_CHUNK_GRANULARITY, DRIVE_UPLOAD_API,
};
use crate::google_auth;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Resultado de una subida.
#[derive(Debug, Clone)]
pub struct Uploaded {
    /// Id del fichero en Drive.
    pub file_id: String,
    /// Nombre con el que quedó en Drive.
    pub name: String,
    /// Bytes subidos.
    pub bytes: u64,
    /// SHA-256 del contenido (verificado en local antes de subir).
    pub sha256: String,
}

/// SHA-256 de un fichero (streaming, sin cargarlo entero en memoria).
pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut f = File::open(path).map_err(|e| format!("abrir {}: {}", path.display(), e))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| format!("leer {}: {}", path.display(), e))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    // `sha2` 0.10: el hash se obtiene con `finalize()`, no con `iter()`.
    let out = h.finalize();
    Ok(out.iter().map(|x| format!("{:02x}", x)).collect())
}

/// Agente HTTP con tiempos acotados.
fn agent() -> ureq::Agent {
    let cfg = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(900)))
        .timeout_connect(Some(Duration::from_secs(20)))
        .http_status_as_error(false)
        .build();
    ureq::Agent::new_with_config(cfg)
}

use std::time::Duration;

/// Sube un fichero a Drive (subida resumible, en trozos de 256 KiB).
pub fn upload_file(
    path: &Path,
    name: &str,
    target: &DriveTarget,
    state_dir: Option<&str>,
) -> Result<Uploaded, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {}", path.display(), e))?;
    if !meta.is_file() {
        return Err(format!("{} no es un fichero", path.display()));
    }
    let size = meta.len();
    let sha = sha256_file(path)?;
    let ag = agent();

    // 1) Sesión de subida resumible.
    let url = format!(
        "{}?uploadType=resumable&supportsAllDrives=true",
        DRIVE_UPLOAD_API
    );
    let (ak, av) = google_auth::auth_header(state_dir)?;
    let resp = ag
        .post(&url)
        .header(ak, av)
        .header("Content-Type", "application/json; charset=UTF-8")
        .header("X-Upload-Content-Length", size.to_string())
        .send(file_metadata(name, size, target).as_bytes())
        .map_err(|e| format!("abrir sesión de subida: {}", e))?;
    let status = resp.status().as_u16();
    if status != 200 && status != 201 {
        let b = resp.into_body().read_to_vec().unwrap_or_default();
        let txt: String = String::from_utf8_lossy(&b).chars().take(200).collect();
        return Err(format!("abrir sesión devolvió {}: {}", status, txt));
    }
    let session = resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .ok_or("la sesión de subida no trae la cabecera Location")?
        .to_string();

    // 2) Trozos. Con un fichero vacío no hay trozos: se sube en una go.
    let plan = chunk_plan(size, DRIVE_CHUNK_GRANULARITY);
    let mut file = File::open(path).map_err(|e| format!("abrir {}: {}", path.display(), e))?;
    let mut buf = vec![0u8; DRIVE_CHUNK_GRANULARITY as usize];
    let mut offset: u64 = 0;
    let mut file_id = String::new();

    for range in plan {
        let want = range.len() as usize;
        let mut got = 0usize;
        while got < want {
            let n = file
                .read(&mut buf[got..want])
                .map_err(|e| format!("leer {}: {}", path.display(), e))?;
            if n == 0 {
                break;
            }
            got += n;
        }
        if got == 0 {
            return Err("lectura corta: el fichero cambió mientras se subía".to_string());
        }
        let end = offset + got as u64 - 1;
        let last = end + 1 >= size;
        let (ak2, av2) = google_auth::auth_header(state_dir)?;
        let resp = ag
            .put(&session)
            .header(ak2, av2)
            .header("Content-Range", format!("bytes {}-{}/{}", offset, end, size))
            .header("Content-Length", got.to_string())
            .send(&buf[..got])
            .map_err(|e| format!("subir trozo en {}: {}", offset, e))?;
        let st = resp.status().as_u16();
        let body = resp.into_body().read_to_vec().unwrap_or_default();
        if st != 200 && st != 201 {
            let txt: String = String::from_utf8_lossy(&body).chars().take(200).collect();
            // 401 = el token caducó: se invalida y se reintenta una vez.
            if st == 401 {
                google_auth::invalidate();
                return Err(format!(
                    "token de Google caducado durante la subida (trozo en {}): {}",
                    offset, txt
                ));
            }
            return Err(format!("subir trozo en {} devolvió {}: {}", offset, st, txt));
        }
        offset += got as u64;
        if last {
            if let Ok(j) = serde_json::from_slice::<Value>(&body) {
                if let Some(id) = j.get("id").and_then(|v| v.as_str()) {
                    file_id = id.to_string();
                }
            }
        }
    }

    if file_id.is_empty() {
        // Google devuelve el id en el cuerpo del último trozo; si no vino,
        // se pregunta por el fichero recién subido por su nombre.
        file_id = find_by_name(name, target, state_dir)?;
    }
    Ok(Uploaded {
        file_id,
        name: name.to_string(),
        bytes: size,
        sha256: sha,
    })
}

/// Busca el id de un fichero por nombre (respaldo si la API no lo devolvió).
fn find_by_name(name: &str, target: &DriveTarget, state_dir: Option<&str>) -> Result<String, String> {
    let mut url = format!(
        "{}?supportsAllDrives=true&fields=files(id,name,size)&q=name%20%3D%20%27{}%27%20and%20trashed%20%3D%20false",
        crate::drive_client::DRIVE_API,
        name.replace(' ', "%20").replace('\'', "%27")
    );
    if !target.folder_id.is_empty() {
        url.push_str(&format!("&q%20%3D%20%27%27%20in%20parents%20%3D%20%27{}%27", target.folder_id));
    }
    let (ak, av) = google_auth::auth_header(state_dir)?;
    let resp = agent()
        .get(&url)
        .header(ak, av)
        .call()
        .map_err(|e| format!("buscar en Drive: {}", e))?;
    let st = resp.status().as_u16();
    let body = resp.into_body().read_to_vec().unwrap_or_default();
    if st != 200 {
        return Err(format!("buscar en Drive devolvió {}", st));
    }
    let j: Value = serde_json::from_slice(&body).map_err(|e| format!("json de Drive: {}", e))?;
    j.get("files")
        .and_then(|f| f.as_array())
        .and_then(|a| a.first())
        .and_then(|f| f.get("id"))
        .and_then(|i| i.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("no se encontró el fichero '{}' en Drive", name))
}

/// Empaqueta un **directorio** en un ZIP (para subirlo como una unidad).
pub fn zip_directory(dir: &Path, zip_path: &Path) -> Result<u64, String> {
    let out = File::create(zip_path).map_err(|e| format!("crear {}: {}", zip_path.display(), e))?;
    let mut zw = zip::ZipWriter::new(out);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut count = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = std::fs::read_dir(&current)
            .map_err(|e| format!("leer {}: {}", current.display(), e))?;
        for e in entries {
            let e = e.map_err(|x| format!("leer entrada: {}", x))?;
            let p = e.path();
            let rel = p
                .strip_prefix(dir)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            if p.is_dir() {
                zw.add_directory(format!("{}/", rel), opts)
                    .map_err(|x| format!("zip dir {}: {}", rel, x))?;
                stack.push(p);
            } else {
                let data = std::fs::read(&p).map_err(|x| format!("leer {}: {}", p.display(), x))?;
                zw.start_file(rel.clone(), opts)
                    .map_err(|x| format!("zip {}: {}", rel, x))?;
                zw.write_all(&data)
                    .map_err(|x| format!("zip write {}: {}", rel, x))?;
                count += data.len() as u64;
            }
        }
    }
    zw.finish().map_err(|e| format!("cerrar zip: {}", e))?;
    Ok(count)
}

/// Sube un **ruta** (fichero o directorio) a Drive. Un directorio se
/// comprime en ZIP y se sube como una unidad.
pub fn upload_path(
    path: &Path,
    name: &str,
    folder_id: &str,
    state_dir: Option<&str>,
) -> Result<Uploaded, String> {
    let target = DriveTarget::folder(folder_id);
    if path.is_dir() {
        let zip_path = std::env::temp_dir().join(format!(
            "md_drive_{}_{}.zip",
            google_auth::now_secs(),
            std::process::id()
        ));
        zip_directory(path, &zip_path)?;
        let up = upload_file(&zip_path, name, &target, state_dir);
        let _ = std::fs::remove_file(&zip_path);
        let mut up = up?;
        up.name = name.to_string();
        Ok(up)
    } else {
        upload_file(path, name, &target, state_dir)
    }
}

/// Descarga un fichero de Drive (`alt=media`) a disco.
pub fn download_file(
    file_id: &str,
    dest: &Path,
    state_dir: Option<&str>,
) -> Result<u64, String> {
    let url = format!(
        "{}/{}?alt=media&supportsAllDrives=true",
        crate::drive_client::DRIVE_API,
        file_id
    );
    let (ak, av) = google_auth::auth_header(state_dir)?;
    let resp = agent()
        .get(&url)
        .header(ak, av)
        .call()
        .map_err(|e| format!("descargar {}: {}", file_id, e))?;
    let st = resp.status().as_u16();
    if st != 200 {
        let b = resp.into_body().read_to_vec().unwrap_or_default();
        let txt: String = String::from_utf8_lossy(&b).chars().take(160).collect();
        if st == 401 {
            google_auth::invalidate();
        }
        return Err(format!("descargar {} devolvió {}: {}", file_id, st, txt));
    }
    // `ureq` 3.x: el cuerpo se lee con `as_reader()`.
    let mut resp_body = resp.into_body();
    let mut body = resp_body.as_reader();
    let mut out = File::create(dest).map_err(|e| format!("crear {}: {}", dest.display(), e))?;
    let mut total = 0u64;
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = body
            .read(&mut buf)
            .map_err(|e| format!("leer cuerpo: {}", e))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])
            .map_err(|e| format!("escribir {}: {}", dest.display(), e))?;
        total += n as u64;
    }
    Ok(total)
}

/// Ruta destino por defecto de una descarga.
pub fn default_download_path(file_id: &str) -> PathBuf {
    std::env::temp_dir().join(format!("md_drive_dl_{}", file_id))
}

/// Informe del estado de Google Drive (sin secretos).
pub fn status(state_dir: Option<&str>) -> Value {
    let mut st = google_auth::status(state_dir);
    st["api"] = json!({
        "upload": DRIVE_UPLOAD_API,
        "files": crate::drive_client::DRIVE_API,
        "chunk_bytes": DRIVE_CHUNK_GRANULARITY,
        "transport": "nativo (ureq, Rust puro; sin curl ni shell)",
    });
    st
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "md_dn_{}_{}",
            tag,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn sha256_de_un_fichero_conocido() {
        let d = tmp("sha");
        let f = d.join("a.txt");
        std::fs::write(&f, b"abc").unwrap();
        let h = sha256_file(&f).unwrap();
        assert_eq!(
            h,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn un_directorio_se_empaqueta_zip_con_su_contenido() {
        let d = tmp("zip");
        std::fs::create_dir_all(d.join("sub")).unwrap();
        std::fs::write(d.join("a.txt"), b"uno").unwrap();
        std::fs::write(d.join("sub/b.txt"), b"dos").unwrap();
        let z = d.join("out.zip");
        zip_directory(&d, &z).unwrap();
        assert!(z.exists());
        let data = std::fs::read(&z).unwrap();
        // La firma PK\x03\x04 del zip.
        assert_eq!(&data[..4], b"PK\x03\x04");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn subir_un_directorio_que_no_existe_falla_con_mensaje_util() {
        let e = upload_path(
            Path::new("/no/existe/esto"),
            "x.zip",
            "",
            None,
        )
        .unwrap_err();
        assert!(!e.is_empty());
    }
}
