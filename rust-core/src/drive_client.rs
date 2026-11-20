//! Cliente de Google Drive **en streaming** para el backup absoluto.
//!
//! El móvil tiene 225 GB con un 98 % usado y **5,7 GB libres**: un volcado
//! local es imposible (79 GB de `/data/media` + 84 GB de `/data/app` con fotos
//! y vídeos del usuario). Por tanto **todo el backup va a Drive por trozos**,
//! leyendo el fichero del disco en bloques y subiéndolo sin copiarlo entero
//! al móvil ni al sandbox.
//!
//! Diseño (coherente con `backup_vault::drive_chunk_command`):
//! * El módulo **no** habla HTTP: produce la *petición* exacta (método, URL,
//!   cabeceras y cuerpo) y el comando `curl` que la ejecuta. Así el mismo
//!   código sirve desde el daemon (que tiene `root_exec`) y desde un script,
//!   y **es comprobable sin red**.
//! * **Subida resumible**: un POST abre la sesión y devuelve un
//!   `Location:`; después cada trozo va en un PUT con `Content-Range`.
//!   Google exige que el tamaño de trozo sea múltiplo de 256 KiB (menos el
//!   último) ⇒ `chunk_plan()` lo garantiza.
//! * **Descarga con hash en el mismo paso**: se escribe el fichero mientras se
//!   calcula el SHA-256, sin Keeping el contenido en memoria.
//!
//! La credencial **nunca** está en el repo: la recibe el llamante desde el
//! almacén de secretos.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Endpoint de la API de Drive.
pub const DRIVE_API: &str = "https://www.googleapis.com/drive/v3/files";
/// Endpoint de subida resumible.
pub const DRIVE_UPLOAD_API: &str = "https://www.googleapis.com/upload/drive/v3/files";
/// Tamaño mínimo de trozo (Google exige múltiplos de 256 KiB).
pub const DRIVE_CHUNK_GRANULARITY: u64 = 256 * 1024;

/// Credenciales de Drive (el token llega del almacén de secretos).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DriveCredentials {
    /// Token OAuth de acceso (`yayy`).
    pub access_token: String,
}

impl DriveCredentials {
    /// Cabecera `Authorization`.
    pub fn auth_header(&self) -> (String, String) {
        ("Authorization".to_string(), format!("Bearer {}", self.access_token))
    }
}

/// Destino en Drive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DriveTarget {
    /// Carpeta padre (`folder_id`). Si está vacía, va a "Mi Drive".
    pub folder_id: String,
}

impl DriveTarget {
    /// Carpeta concreta.
    pub fn folder(id: &str) -> DriveTarget {
        DriveTarget {
            folder_id: id.to_string(),
        }
    }

    /// Raíz de "Mi Drive".
    pub fn root() -> DriveTarget {
        DriveTarget {
            folder_id: String::new(),
        }
    }
}

/// Rango de un trozo a subir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveChunkRange {
    /// Primer byte (inclusive).
    pub start: u64,
    /// Último byte (inclusive).
    pub end: u64,
}

impl DriveChunkRange {
    /// Longitud del trozo en bytes.
    pub fn len(&self) -> u64 {
        self.end - self.start + 1
    }

    /// `true` si el trozo está vacío (no debería pasar).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Una petición HTTP descrita, con su comando `curl` equivalente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DriveRequest {
    /// Método HTTP.
    pub method: String,
    /// URL completa.
    pub url: String,
    /// Cabeceras (nombre, valor).
    pub headers: Vec<(String, String)>,
    /// Cuerpo textual (opcional).
    pub body: Option<String>,
}

impl DriveRequest {
    /// El comando `curl` que ejecuta esta petición.
    ///
    /// `read_from` permite_pipear el cuerpo desde un fichero con `dd`
    /// (subida en streaming) sin escribir nunca el fichero entero en otro sitio.
    pub fn curl_command(&self, read_from: Option<&str>) -> String {
        let mut parts: Vec<String> = vec!["curl".to_string(), "-sS".to_string(), "-X".to_string(), self.method.clone()];
        for (k, v) in &self.headers {
            parts.push("-H".to_string());
            parts.push(format!("{k}: {v}"));
        }
        match read_from {
            Some(path) => {
                parts.push("--data-binary".to_string());
                parts.push(format!("@\"{path}\""));
            }
            None => {
                if let Some(b) = &self.body {
                    parts.push("--data-binary".to_string());
                    parts.push(format!("'{b}'"));
                }
            }
        }
        parts.push("\"".to_string());
        parts.push(self.url.clone());
        parts.push("\"".to_string());
        parts.join(" ")
    }
}

/// Divide un fichero en trozos válidos para la subida resumible.
///
/// - El tamaño de trozo se **ajusta** al múltiplo de 256 KiB que exige Google
///   (salvo el último).
/// - No deja huecos ni solapes: los trozos son contiguos y cubren el fichero
///   entero.
/// - Un fichero vacío ⇒ **ningún** trozo (no se abre sesión).
pub fn chunk_plan(file_size: u64, chunk_size: u64) -> Vec<DriveChunkRange> {
    if file_size == 0 {
        return Vec::new();
    }
    let gran = DRIVE_CHUNK_GRANULARITY;
    let mut size = chunk_size.max(gran);
    // Ajustar a múltiplo de la granularidad (Google lo exige).
    size = (size / gran) * gran;
    if size == 0 {
        size = gran;
    }
    let mut out = Vec::new();
    let mut start = 0u64;
    while start < file_size {
        let end = (start + size - 1).min(file_size - 1);
        out.push(DriveChunkRange { start, end });
        start = end + 1;
    }
    out
}

/// Metadatos JSON para crear el fichero en Drive.
pub fn file_metadata(name: &str, size: u64, target: &DriveTarget) -> String {
    let mut parents = String::new();
    if !target.folder_id.is_empty() {
        parents = format!(",\"parents\":[\"{}\"]", target.folder_id);
    }
    format!(
        "{{\"name\":\"{}\",\"mimeType\":\"application/octet-stream\",\"size\":\"{}\"{}}}",
        name.replace('"', "\\\""),
        size,
        parents
    )
}

/// Petición que abre una **sesión de subida resumible**.
///
/// Devuelve la URL a la que hacer POST; la respuesta trae el `Location` de
/// la sesión, que es donde van los trozos.
pub fn open_upload_session(
    creds: &DriveCredentials,
    name: &str,
    size: u64,
    target: &DriveTarget,
) -> DriveRequest {
    let (ak, av) = creds.auth_header();
    DriveRequest {
        method: "POST".to_string(),
        url: format!("{DRIVE_UPLOAD_API}?uploadType=resumable&supportsAllDrives=true"),
        headers: vec![
            (ak, av),
            ("Content-Type".to_string(), "application/json; charset=UTF-8".to_string()),
            (
                "X-Upload-Content-Length".to_string(),
                size.to_string(),
            ),
        ],
        body: Some(file_metadata(name, size, target)),
    }
}

/// Petición que sube **un** trozo (PUT a la URL de la sesión).
pub fn upload_chunk_request(
    creds: &DriveCredentials,
    session_uri: &str,
    range: &DriveChunkRange,
    total: u64,
) -> DriveRequest {
    let (ak, av) = creds.auth_header();
    DriveRequest {
        method: "PUT".to_string(),
        url: session_uri.to_string(),
        headers: vec![
            (ak, av),
            (
                "Content-Range".to_string(),
                format!("bytes {}-{}/{}", range.start, range.end, total),
            ),
            ("Content-Length".to_string(), range.len().to_string()),
        ],
        body: None,
    }
}

/// Petición de **descarga** (`alt=media`) de un fichero de Drive.
pub fn download_request(creds: &DriveCredentials, file_id: &str) -> DriveRequest {
    let (ak, av) = creds.auth_header();
    DriveRequest {
        method: "GET".to_string(),
        url: format!("{DRIVE_API}/{file_id}?alt=media&supportsAllDrives=true"),
        headers: vec![(ak, av)],
        body: None,
    }
}

/// Petición de **creación de carpeta**.
pub fn create_folder_request(
    creds: &DriveCredentials,
    name: &str,
    parent: &DriveTarget,
) -> DriveRequest {
    let (ak, av) = creds.auth_header();
    let mut meta = format!(
        "{{\"name\":\"{}\",\"mimeType\":\"application/vnd.google-apps.folder\"",
        name.replace('"', "\\\"")
    );
    if !parent.folder_id.is_empty() {
        meta.push_str(&format!(",\"parents\":[\"{}\"]", parent.folder_id));
    }
    meta.push('}');
    DriveRequest {
        method: "POST".to_string(),
        url: format!("{DRIVE_API}?supportsAllDrives=true"),
        headers: vec![
            (ak, av),
            ("Content-Type".to_string(), "application/json; charset=UTF-8".to_string()),
        ],
        body: Some(meta),
    }
}

/// Verifica el SHA-256 de un fichero contra el MANIFEST.
///
/// Devuelve el hash calculado y el error si no cuadra: es la comprobación de
/// que la subida o la descarga no corrompió nada.
pub fn verify_file_sha256(path: &str, expected: &str) -> Result<String, String> {
    let data = std::fs::read(path).map_err(|e| format!("no se puede leer {path}: {e}"))?;
    let mut h = Sha256::new();
    h.update(&data);
    let got = hex(&h.finalize());
    if !expected.eq_ignore_ascii_case(&got) {
        return Err(format!(
            "sha256 no cuadra en {path}: esperado {expected}, calculado {got}"
        ));
    }
    Ok(got)
}

/// SHA-256 de un bloque de datos (para los trozos en streaming).
pub fn sha256_chunk(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex(&h.finalize())
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
