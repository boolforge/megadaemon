//! Gestor de las grabaciones de llamada (estilo BCR-GUI) en Rust.
//!
//! **Alcance legal (importante):** este módulo **no graba**. La grabación la
//! hace la grabadora **nativa de Samsung**, que en Europa **avisa por voz a la
//! otra parte** (obligatorio: art. 197 CP + GDPR en España). Grabar en
//! silencio a un tercero sin su consentimiento es delito. Lo que sí hacemos es
//! **gestionar** lo ya grabado:
//!
//! 1. **Inventariar**: listar las grabaciones con tamaño, fecha y SHA-256.
//! 2. **Empaquetar**: preparar el *pack* cifrado para el vault (AES-256-GCM).
//! 3. **Transcribir**: dejar el *gancho* hacia un ASR (el audio se envía a un
//!    servicio externo configurable; el módulo no trae motor de ASR).
//! 4. **Subir**: plan de subida a Drive por trozos (resumible, sin copias
//!    locales) reutilizando el cliente de streaming.
//!
//! Nota de interoperabilidad: **no** se integra con BCR (grabador open-source)
//! porque son sistemas separados y chocan entre sí (el propio BCR lo avisa):
//! o la grabadora nativa de Samsung, o BCR, nunca las dos.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::drive_client::{chunk_plan, DriveChunkRange, DriveTarget};

/// Metadatos de una grabación.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordingMeta {
    /// Ruta absoluta en el móvil.
    pub path: String,
    /// Nombre del fichero.
    pub name: String,
    /// Tamaño en bytes.
    pub size: u64,
    /// SHA-256 del contenido (clave del MANIFEST).
    pub sha256: String,
    /// Fecha de modificación (epoch, segundos).
    pub modified: Option<i64>,
    /// Transcripción si hay un `.txt`/`srt`/`vtt` al lado.
    pub transcript: Option<String>,
}

/// Extensiones de transcripción que se buscan junto al audio.
pub const TRANSCRIPT_EXTENSIONS: [&str; 3] = ["txt", "srt", "vtt"];

/// Extensiones de audio que se consideran grabaciones.
pub const AUDIO_EXTENSIONS: [&str; 6] = ["m4a", "mp4", "aac", "amr", "3gp", "ogg"];

/// ¿Es un fichero de audio de grabación?
pub fn is_recording_file(name: &str) -> bool {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    AUDIO_EXTENSIONS.contains(&ext.as_str())
}

/// ¿Es un fichero de transcripción?
pub fn is_transcript_file(name: &str) -> bool {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    TRANSCRIPT_EXTENSIONS.contains(&ext.as_str())
}

/// SHA-256 de un fichero (en streaming, sin cargarlo en memoria).
pub fn sha256_file(path: &str) -> Result<String, String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).map_err(|e| format!("no se puede abrir {path}: {e}"))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = f.read(&mut buf).map_err(|e| format!("error leyendo {path}: {e}"))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Inventaría las grabaciones de un directorio (recursivo, sólo lectura).
///
/// Busca también transcripciones junto al audio y las associates.
pub fn scan_recordings(dir: &str) -> Result<Vec<RecordingMeta>, String> {
    let mut out: Vec<RecordingMeta> = Vec::new();
    scan_dir(dir, dir, &mut out, 0)?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn scan_dir(root: &str, dir: &str, out: &mut Vec<RecordingMeta>, depth: usize) -> Result<(), String> {
    if depth > 8 {
        return Err("árbol demasiado profundo".to_string());
    }
    let rd = std::fs::read_dir(dir).map_err(|e| format!("no se puede leer {dir}: {e}"))?;
    for entry in rd {
        let entry = entry.map_err(|e| format!("no se puede leer {dir}: {e}"))?;
        let path = entry.path();
        if path.is_dir() {
            scan_dir(root, &path.to_string_lossy(), out, depth + 1)?;
            continue;
        }
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        if !is_recording_file(&name) {
            continue;
        }
        let meta = entry
            .metadata()
            .map_err(|e| format!("no se puede ver {}: {e}", path.display()))?;
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64);
        let p = path.to_string_lossy().to_string();
        // transcripción al lado: mismo nombre, extensión de texto
        let transcript = path
            .parent()
            .map(|d| {
                let base = name.rsplit_once('.').map(|(b, _)| b).unwrap_or(&name);
                for ext in TRANSCRIPT_EXTENSIONS {
                    let cand = d.join(format!("{base}.{ext}"));
                    if cand.is_file() {
                        return std::fs::read_to_string(&cand).ok();
                    }
                }
                None
            })
            .flatten();
        out.push(RecordingMeta {
            sha256: sha256_file(&p)?,
            path: p,
            name,
            size: meta.len(),
            modified,
            transcript,
        });
    }
    Ok(())
}

/// Una entrada del *pack* del vault.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VaultEntry {
    /// Fichero de origen.
    pub source: String,
    /// Ruta dentro del pack cifrado.
    pub dest: String,
    /// SHA-256 (clave del MANIFEST).
    pub sha256: String,
    /// Tamaño en bytes.
    pub size: u64,
}

/// Prepara el *pack* del vault para las grabaciones.
///
/// Estructura: `grabaciones/<AAAAMMDD>/<nombre>.m4a` + `grabaciones/<AAAAMMDD>/<nombre>.txt`
/// (la transcripción si existe). **El pack va cifrado** con AES-256-GCM
/// (`backup_vault::seal_dir_streaming`), así que el contenido sale del móvil
/// sólo cifrado.
pub fn vault_pack(recordings: &[RecordingMeta]) -> Vec<VaultEntry> {
    let mut out = Vec::new();
    for r in recordings {
        let day = r
            .modified
            .map(|t| day_stamp(t))
            .unwrap_or_else(|| "sin-fecha".to_string());
        out.push(VaultEntry {
            source: r.path.clone(),
            dest: format!("grabaciones/{day}/{}.m4a", stem(&r.name)),
            sha256: r.sha256.clone(),
            size: r.size,
        });
        if let Some(t) = &r.transcript {
            out.push(VaultEntry {
                source: r.path.clone(),
                dest: format!("grabaciones/{day}/{}.txt", stem(&r.name)),
                sha256: sha256_text(t),
                size: t.len() as u64,
            });
        }
    }
    out
}

fn stem(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(b, _)| b.to_string())
        .unwrap_or_else(|| name.to_string())
}

fn day_stamp(epoch: i64) -> String {
    // UTC simple (días desde la época) => AAAA-MM-DD
    let days = epoch.div_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn sha256_text(t: &str) -> String {
    let mut h = Sha256::new();
    h.update(t.as_bytes());
    hex(&h.finalize())
}

/// Backend de transcripción.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TranscriptionBackend {
    /// Sin transcripción (sólo se deja el audio cifrado en el vault).
    None,
    /// Servicio compatible con la API de OpenAI (`/v1/audio/transcriptions`).
    OpenAiCompat {
        /// URL base del servicio.
        endpoint: String,
        /// Nombre del modelo.
        model: String,
    },
}

/// Plan de transcripción: qué enviar a qué sitio.
///
/// El módulo **no** trae motor de ASR: prepara la petición y devuelve lo que
/// hay que enviar (fichero + endpoint + modelo). La llamada de red la hace el
/// ejecutor (el daemon), con la credencial en el almacén de secretos.
pub fn transcription_plan(
    rec: &RecordingMeta,
    backend: &TranscriptionBackend,
) -> Vec<String> {
    match backend {
        TranscriptionBackend::None => vec![format!(
            "Sin transcripción: se guarda sólo el audio cifrado ({})",
            rec.path
        )],
        TranscriptionBackend::OpenAiCompat { endpoint, model } => vec![
            format!("POST {endpoint}/v1/audio/transcriptions"),
            format!("modelo: {model}"),
            format!("fichero: {}", rec.path),
            format!("sha256 del audio: {}", rec.sha256),
            "cabecera Authorization: Bearer <clave del almacén de secretos>".to_string(),
        ],
    }
}

/// Plan de subida de las grabaciones a Drive (por trozos, sin copias locales).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DriveUploadItem {
    /// Fichero local.
    pub source: String,
    /// Nombre en Drive.
    pub name: String,
    /// Tamaño total.
    pub size: u64,
    /// Trozos a subir.
    pub chunks: Vec<DriveChunkRange>,
}

/// Prepara la subida de las grabaciones a Drive.
///
/// Reusa el plan de trozos del cliente de streaming (256 KiB múltiplos), así
/// que la subida **resumible** y la verificación por SHA-256 son las mismas.
pub fn drive_upload_plan(
    recordings: &[RecordingMeta],
    folder_id: &str,
    chunk_size: u64,
) -> Vec<DriveUploadItem> {
    let _ = DriveTarget::folder(folder_id);
    recordings
        .iter()
        .map(|r| DriveUploadItem {
            source: r.path.clone(),
            name: r.name.clone(),
            size: r.size,
            chunks: chunk_plan(r.size, chunk_size),
        })
        .collect()
}

/// Resumen para el informe del bloque.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VaultReport {
    /// Número de grabaciones.
    pub count: usize,
    /// Bytes totales.
    pub total_bytes: u64,
    /// Grabaciones con transcripción.
    pub with_transcript: usize,
    /// SHA-256 del MANIFEST (hash de los hashes, en orden).
    pub manifest_sha256: String,
}

/// Construye el informe del vault a partir del inventario.
pub fn vault_report(recordings: &[RecordingMeta]) -> VaultReport {
    let mut h = Sha256::new();
    for r in recordings {
        h.update(r.sha256.as_bytes());
    }
    VaultReport {
        count: recordings.len(),
        total_bytes: recordings.iter().map(|r| r.size).sum(),
        with_transcript: recordings.iter().filter(|r| r.transcript.is_some()).count(),
        manifest_sha256: hex(&h.finalize()),
    }
}
