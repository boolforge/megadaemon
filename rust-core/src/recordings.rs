//! MegaDaemon v13.4 — Índice unificado de grabaciones + backup de seguridad.
//!
//! ## Qué hace este módulo (y qué NO)
//!
//! Este módulo **no graba nada**. La grabación la hacen las apps del móvil
//! (la grabadora **nativa de Samsung**, Telegram, BCR, …) y sus avisos legales
//! ya están resueltos: con `RecordingAllowedByMenu,disclaimer_latin` la
//! grabación nativa **avisa** a la otra parte. Aquí sólo se:
//!
//! 1. **Indexa**: recorre la raíz de audio del móvil y lista lo que hay.
//! 2. **Unifica**: reconcilia lo que *declara* cada grabador (su lista) con lo
//!    que *existe* de verdad en disco, y clasifica cada grabación.
//! 3. **Respalda**: copia de seguridad **incremental** con MANIFEST y
//!    verificación posterior.
//!
//! ## Por qué el SHA-256 es la clave fiable del emparejamiento
//!
//! Las listas de los grabadores **no usan el mismo nombre de fichero que el
//! disco**: el grabador nativo de Samsung escribe en
//! `SoundRecordings/<subcarpeta>/<numero>_<timestamp>.amr`, Telegram en la
//! caché de la app con nombres propios, BCR en su propio árbol, y además cada
//! app mete el número de teléfono y/o una marca de tiempo en el nombre. El
//! nombre es, por tanto, **volátil**: cambia con el firmware, con la versión
//! de la app y con la carpeta donde el sistema mueve los ficheros.
//!
//! El **SHA-256 es la identidad del contenido**: no depende del nombre, ni de
//! la carpeta, ni del grabador que lo escribió. Por eso el emparejamiento va
//! **primero por hash**, y sólo si el grabador no publica hash se degrada a
//! *nombre normalizado* (sin marca de tiempo, en minúsculas, sin extensión) y,
//! en último caso, a *tamaño + primeros N bytes* (que es una identidad
//! **débil**: dos grabaciones del mismo tamaño y del mismo prefijo se
//! confunden; por eso es el último recurso y queda marcado en el informe).
//!
//! ## De dónde sale el código
//!
//! Las 4 funciones de hash/fecha de este módulo son **copia comentada** de
//! `call_recorder.rs` (`sha256_hex`, `sha256_file`, `now_iso8601`,
//! `civil_from_epoch`) y la lista de extensiones de audio es la misma que
//! `call_recorder::RECORDING_EXTENSIONS`. Se copian y no se importan porque
//! `call_recorder` **todavía no está registrado en `lib.rs`**: importarlo
//! (`crate::call_recorder`) haría que este módulo no compilase hasta que
//! alguien lo cablee. En cuanto `pub mod call_recorder;` esté en `lib.rs`,
//! estas cuatro funciones se pueden borrar y usar las del módulo original
//! (mismo nombre, misma firma) sin tocar nada más.
//!
//! El **MANIFEST** y el plan de subida a Drive reutilizan el estilo ya
//! establecido en el repo: NDJSON *append-only* (`device_inventory`,
//! `agent_loop`) y trozos de 256 KiB con `Content-Range`
//! (`drive_client::chunk_plan`).

use crate::drive_client::{chunk_plan, DriveChunkRange};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Constantes
// ---------------------------------------------------------------------------

/// Extensiones de audio que se consideran una grabación.
/// COPIA de `call_recorder::RECORDING_EXTENSIONS` (misma lista, mismo orden).
pub const RECORDING_EXTENSIONS: [&str; 13] = [
    "wav", "opus", "ogg", "pcm", "raw", "m4a", "aac", "mp3", "flac", "amr", "3gp", "mkv", "webm",
];

/// Bytes del prefijo usados para la identidad "tamaño + primeros N bytes".
pub const PREFIX_BYTES: usize = 4096;

/// Profundidad máxima al recorrer la raíz de audio (evita buclesalbien).
pub const MAX_DEPTH: usize = 8;

/// Nombre del MANIFEST (NDJSON append-only) dentro del destino.
pub const MANIFEST_FILE: &str = "MANIFEST.ndjson";

/// Nombre del resumen del MANIFEST (JSON) dentro del destino.
pub const MANIFEST_SUMMARY_FILE: &str = "manifest.json";

// ---------------------------------------------------------------------------
// Utilidades copiadas de `call_recorder.rs` (ver la nota de cabecera)
// ---------------------------------------------------------------------------

/// SHA-256 en hexadecimal de un buffer.
/// COPIA de `call_recorder::sha256_hex`.
pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    hex(&digest)
}

/// SHA-256 en hexadecimal de un fichero (streaming, sin cargarlo en memoria).
/// COPIA de `call_recorder::sha256_file`.
pub fn sha256_file(path: &Path) -> Option<String> {
    let mut f = File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = match f.read(&mut buf) {
            Ok(n) => n,
            Err(_) => return None,
        };
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(hex(&hasher.finalize()))
}

/// Convierte bytes a hexadecimal en minúsculas.
fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Devuelve la hora actual en ISO-8601 UTC (`2024-05-06T07:08:09Z`).
/// COPIA de `call_recorder::now_iso8601`.
pub fn now_iso8601() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, m, d, hh, mm, ss) = civil_from_epoch(secs as i64);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, hh, mm, ss)
}

/// Convierte segundos desde epoch a fecha civil (algoritmo de Hinnant).
/// COPIA de `call_recorder::civil_from_epoch`.
fn civil_from_epoch(secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (
        y,
        m,
        d,
        (rem / 3600) as u32,
        ((rem % 3600) / 60) as u32,
        (rem % 60) as u32,
    )
}

/// Sello de día en UTC (`AAAA-MM-DD`) a partir de un epoch en segundos.
pub fn day_stamp(epoch: i64) -> String {
    let (y, m, d, _, _, _) = civil_from_epoch(epoch);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Sello del día de hoy en UTC (`AAAA-MM-DD`).
pub fn today_stamp() -> String {
    day_stamp(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
    )
}

// ---------------------------------------------------------------------------
// Modelo de datos
// ---------------------------------------------------------------------------

/// Estado de una grabación tras reconciliar la lista del grabador con el disco.
///
/// | Token JSON   | Español    | Significado                                        |
/// |--------------|------------|----------------------------------------------------|
/// | `ok`         | Ok         | Emparejada y verificada (hash en disco = declarado)  |
/// | `missing`    | Ausente    | El fichero existe pero **no tiene audio** (vacío o ilegible) |
/// | `corrupt`    | Corrupto   | El hash en disco **no cuadra** con el declarado      |
/// | `orphan`     | Huérfano   | Fichero en disco que **ninguna** app declara         |
/// | `no_audio`   | SinAudio   | La app la declara pero **no hay fichero** que corresponda |
/// | `duplicate`  | Duplicado  | El mismo SHA-256 está en **dos sitios** (copia de más) |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingState {
    /// Emparejada y verificada.
    Ok,
    /// El fichero existe pero no tiene audio utilizable (vacío o ilegible).
    Missing,
    /// El hash en disco no cuadra con el declarado.
    Corrupt,
    /// Fichero en disco que ninguna app declara.
    Orphan,
    /// Declarada por una app pero no hay fichero que corresponda.
    NoAudio,
    /// Mismo SHA-256 en dos sitios: hay una copia redundante.
    Duplicate,
}

impl RecordingState {
    /// Token estable para JSON y para la API de la GUI.
    pub fn as_str(&self) -> &'static str {
        match self {
            RecordingState::Ok => "ok",
            RecordingState::Missing => "missing",
            RecordingState::Corrupt => "corrupt",
            RecordingState::Orphan => "orphan",
            RecordingState::NoAudio => "no_audio",
            RecordingState::Duplicate => "duplicate",
        }
    }

    /// Etiqueta en español (consola y GUI).
    pub fn label_es(&self) -> &'static str {
        match self {
            RecordingState::Ok => "ok",
            RecordingState::Missing => "ausente",
            RecordingState::Corrupt => "corrupto",
            RecordingState::Orphan => "huérfano",
            RecordingState::NoAudio => "sin audio",
            RecordingState::Duplicate => "duplicado",
        }
    }
}

/// Una grabación del índice unificado.
///
/// Los nombres JSON son los del contrato de la API (`GET /api/recordings`);
/// los identificadores en Rust están en inglés.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recording {
    /// Identificador estable: `<app>:<sha256[..16]>` (o el hash de la ruta si no hay audio).
    #[serde(rename = "id")]
    pub id: String,
    /// Ruta del fichero en el móvil (o la ruta declarada si no existe).
    #[serde(rename = "ruta")]
    pub path: String,
    /// Tamaño en bytes.
    #[serde(rename = "bytes")]
    pub bytes: u64,
    /// SHA-256 de referencia (el declarado por la app; si no hay, el de disco).
    #[serde(rename = "sha256")]
    pub sha256: String,
    /// Duración en segundos (0 si no se sabe).
    #[serde(rename = "duracion_s")]
    pub duration_s: u32,
    /// Fecha de modificación (epoch, segundos).
    #[serde(rename = "mtime")]
    pub mtime: i64,
    /// Origen de la llamada: `system`, `whatsapp`, `telegram`, `signal`, `discord`…
    #[serde(rename = "fuente")]
    pub source: String,
    /// App que declara la grabación (paquete Android o nombre corto).
    #[serde(rename = "app")]
    pub app: String,
    /// Formato del fichero (`wav`, `m4a`, `amr`…).
    #[serde(rename = "tipo")]
    pub kind: String,
    /// Estado de la grabación.
    #[serde(rename = "estado")]
    pub state: RecordingState,
    /// SHA-256 **calculado** en disco (si se pudo leer).
    #[serde(rename = "sha256_disco", default, skip_serializing_if = "Option::is_none")]
    pub disk_sha256: Option<String>,
    /// SHA-256 **declarado** por la app (si lo publica).
    #[serde(rename = "sha256_declarado", default, skip_serializing_if = "Option::is_none")]
    pub declared_sha256: Option<String>,
    /// Cómo se emparejó con el disco: `ruta`, `hash`, `nombre`, `tamano_prefijo`, `tamano`.
    #[serde(rename = "emparejado_por", default, skip_serializing_if = "Option::is_none")]
    pub matched_by: Option<String>,
    /// Clase de la llamada si la app la publica (`voice`, `video`).
    #[serde(rename = "clase", default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
}

/// Informe de la reconciliación: los contadores que consume la GUI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Informe {
    /// Número de entradas del índice unificado (declaradas + huérfanas).
    pub total: usize,
    /// Grabaciones verificadas y sin duplicar.
    pub ok: usize,
    /// Ficheros sin audio utilizable (vacíos o ilegibles).
    #[serde(rename = "ausentes")]
    pub missing: usize,
    /// Grabaciones cuyo hash en disco no cuadra con el declarado.
    #[serde(rename = "corruptos")]
    pub corrupt: usize,
    /// Ficheros en disco que ninguna app declara.
    #[serde(rename = "huerfanos")]
    pub orphans: usize,
    /// Grabaciones con el mismo SHA-256 en más de un sitio.
    #[serde(rename = "duplicados")]
    pub duplicates: usize,
    /// Bytes ocupados por las grabaciones indexadas.
    #[serde(rename = "bytes_totales")]
    pub total_bytes: u64,
    /// Bytes ocupados por las **copias sobrantes** (duplicados).
    #[serde(rename = "wasted_bytes")]
    pub wasted_bytes: u64,
    /// Grabaciones declaradas sin fichero en disco.
    #[serde(rename = "sin_audio")]
    pub no_audio: usize,
}

impl Informe {
    /// Informe vacío.
    pub fn vacio() -> Informe {
        Informe {
            total: 0,
            ok: 0,
            missing: 0,
            corrupt: 0,
            orphans: 0,
            duplicates: 0,
            total_bytes: 0,
            wasted_bytes: 0,
            no_audio: 0,
        }
    }

    /// ¿Hay problemas de integridad o de ausencia de audio?
    ///
    /// Es lo que decide el código de salida del CLI (1 = hay problemas).
    /// Los huérfanos y los duplicados **no** son errores: son datos.
    pub fn hay_problemas(&self) -> bool {
        self.missing > 0 || self.corrupt > 0 || self.no_audio > 0
    }
}

/// Grupo de ficheros con el **mismo SHA-256** (clave de agrupación: el hash).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrupoDuplicado {
    /// SHA-256 común a todo el grupo.
    pub sha256: String,
    /// Rutas de los ficheros del grupo.
    pub ficheros: Vec<String>,
    /// Bytes de una copia (todas son iguales: mismo contenido).
    pub bytes: u64,
    /// Bytes que ocupan las copias **sobrantes** (se conserva una).
    pub wasted_bytes: u64,
}

// ---------------------------------------------------------------------------
// Parseo tolerante de las listas de los grabadores
// ---------------------------------------------------------------------------

/// Entrada declarada por un grabador, ya normalizada.
#[derive(Debug, Clone, Default)]
struct Declarada {
    app: String,
    path: Option<String>,
    name: Option<String>,
    bytes: Option<u64>,
    sha256: Option<String>,
    head_hex: Option<String>,
    duration_s: Option<u32>,
    mtime: Option<i64>,
    kind: Option<String>,
    class: Option<String>,
}

/// Lee el primer campo presente de un objeto JSON.
fn first_str(obj: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(v) = obj.get(*k) {
            if let Some(s) = v.as_str() {
                let s = s.trim();
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
    }
    None
}

/// Lee el primer campo numérico presente de un objeto JSON.
fn first_num(obj: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<f64> {
    for k in keys {
        if let Some(v) = obj.get(*k) {
            match v {
                Value::Number(n) => {
                    if let Some(f) = n.as_f64() {
                        return Some(f);
                    }
                }
                Value::String(s) => {
                    if let Ok(f) = s.trim().parse::<f64>() {
                        return Some(f);
                    }
                }
                _ => {}
            }
        }
    }
    None
}

/// Normaliza una lista JSON (de una app o de un grabador) a `Vec<Value>`.
///
/// Acepta todas las formas que se han visto en el móvil:
/// * `[{ "app": "x", "recordings": [ … ] }]` (formato canónico),
/// * `[{ "path": …, "sha256": … }]` (entradas sueltas),
/// * `{"apps": [ … ]}`, `{"recordings": [ … ]}`, `[ … ]`.
pub fn listas_desde_json(valor: &Value) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    match valor {
        Value::Array(arr) => {
            // Puede ser una lista de apps o una lista de entradas sueltas.
            let es_lista_de_apps = arr.iter().all(|v| {
                v.as_object()
                    .map(|o| {
                        o.contains_key("recordings")
                            || o.contains_key("items")
                            || o.contains_key("files")
                            || o.contains_key("entries")
                            || o.contains_key("lista")
                            || o.contains_key("grabaciones")
                    })
                    .unwrap_or(false)
            });
            if es_lista_de_apps && !arr.is_empty() {
                out.extend(arr.clone());
            } else {
                out.push(Value::Array(arr.clone()));
            }
        }
        Value::Object(obj) => {
            let arr_key = [
                "recordings", "items", "files", "entries", "lista", "grabaciones", "audios",
            ]
            .iter()
            .find(|k| obj.get(**k).map(|v| v.is_array()).unwrap_or(false));
            if let Some(k) = arr_key {
                let app = first_str(obj, &["app", "package", "source", "recorder", "name", "id"])
                    .unwrap_or_default();
                let arr = obj.get(*k).and_then(|v| v.as_array()).cloned().unwrap_or_default();
                out.push(json!({"app": app, "recordings": arr}));
            } else if let Some(k) = ["apps", "sources", "recorders", "list"]
                .iter()
                .find(|k| obj.get(**k).map(|v| v.is_array()).unwrap_or(false))
            {
                let arr = obj.get(*k).and_then(|v| v.as_array()).cloned().unwrap_or_default();
                out.extend(listas_desde_json(&Value::Array(arr)));
            } else {
                out.push(valor.clone());
            }
        }
        _ => {}
    }
    out
}

/// Convierte una entrada JSON suelta en una entrada normalizada.
fn parse_entrada(v: &Value, app_por_defecto: &str) -> Option<Declarada> {
    let obj = v.as_object()?;
    let mut d = Declarada {
        app: first_str(obj, &["app", "package", "source", "recorder", "app_id"])
            .unwrap_or_else(|| app_por_defecto.to_string()),
        ..Default::default()
    };
    d.path = first_str(
        obj,
        &[
            "path",
            "file",
            "file_path",
            "filepath",
            "audio_path",
            "local_path",
            "location",
            "uri",
        ],
    );
    d.name = first_str(obj, &["name", "file_name", "filename", "display_name", "title"])
        .or_else(|| d.path.as_deref().map(|p| basename(p)));
    d.bytes = first_num(obj, &["bytes", "size", "size_bytes", "length", "filesize"])
        .map(|n| n as u64);
    d.sha256 = first_str(obj, &["sha256", "sha_256", "hash", "sha", "checksum", "digest"])
        .map(|h| h.trim().to_ascii_lowercase())
        .filter(|h| h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit()));
    d.head_hex = first_str(obj, &["head_hex", "prefix_hex", "first_bytes_hex", "head"])
        .map(|h| h.trim().to_ascii_lowercase());
    d.duration_s = first_num(
        obj,
        &[
            "duration_s",
            "duration",
            "duration_sec",
            "duration_seconds",
            "length_s",
            "seconds",
        ],
    )
    .map(|n| n as u32);
    d.mtime = first_num(
        obj,
        &["mtime", "modified_unix", "modified", "timestamp", "time", "date"],
    )
    .map(|n| n as i64);
    d.kind = first_str(obj, &["type", "kind", "format", "ext", "extension", "mime"]);
    d.class = first_str(obj, &["class", "call_kind", "media", "media_kind", "call_type"]);
    Some(d)
}

/// Aplana `lista_por_app` en entradas declaradas.
fn parse_listas(lista_por_app: &[Value]) -> Vec<Declarada> {
    let mut out: Vec<Declarada> = Vec::new();
    for item in lista_por_app {
        match item {
            Value::Array(arr) => {
                for e in arr {
                    if let Some(d) = parse_entrada(e, "") {
                        out.push(d);
                    }
                }
            }
            Value::Object(obj) => {
                let arr_key = [
                    "recordings", "items", "files", "entries", "lista", "grabaciones", "audios",
                ]
                .iter()
                .find(|k| obj.get(**k).map(|v| v.is_array()).unwrap_or(false));
                match arr_key {
                    Some(k) => {
                        let app = first_str(obj, &["app", "package", "source", "recorder", "name", "id"])
                            .unwrap_or_default();
                        if let Some(arr) = obj.get(*k).and_then(|v| v.as_array()) {
                            for e in arr {
                                if let Some(d) = parse_entrada(e, &app) {
                                    out.push(d);
                                }
                            }
                        }
                    }
                    None => {
                        if let Some(d) = parse_entrada(item, "") {
                            out.push(d);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Nombre de fichero de una ruta (sin directorios).
fn basename(p: &str) -> String {
    p.rsplit(['/', '\\']).next().unwrap_or(p).to_string()
}

// ---------------------------------------------------------------------------
// Normalización de nombres (emparejamiento por nombre)
// ---------------------------------------------------------------------------

/// Quita de un texto los patrones de fecha (`2024-01-01`, `20240101`).
fn quitar_fechas(s: &str) -> String {
    let b: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        // AAAA[-_.]MM[-_.]DD — el año tiene que *parecer* un año (19xx/20xx):
        // si no, un número de teléfono ("34600123456") se tomaría por una fecha.
        if i + 4 <= b.len()
            && b[i..i + 4].iter().all(|c| c.is_ascii_digit())
            && ((b[i] == '1' && b[i + 1] == '9') || (b[i] == '2' && b[i + 1] == '0'))
        {
            let mut j = i + 4;
            let mut sep1 = false;
            if j < b.len() && (b[j] == '-' || b[j] == '_' || b[j] == '.') {
                sep1 = true;
                j += 1;
            }
            if j + 2 <= b.len() && b[j..j + 2].iter().all(|c| c.is_ascii_digit()) {
                let mut k = j + 2;
                let mut sep2 = false;
                if k < b.len() && (b[k] == '-' || b[k] == '_' || b[k] == '.') {
                    sep2 = true;
                    k += 1;
                }
                if sep1 == sep2
                    && k + 2 <= b.len()
                    && b[k..k + 2].iter().all(|c| c.is_ascii_digit())
                {
                    i = k + 2;
                    continue;
                }
            }
        }
        // HH[-_.]MM[-_.]SS — sólo con separadores: sin ellos, seis dígitos
        // seguidos son más probablemente un número de teléfono que una hora
        // (y de ésos se ocupa `quitar_rachas_digitos`).
        if i + 2 <= b.len() && b[i..i + 2].iter().all(|c| c.is_ascii_digit()) {
            let mut j = i + 2;
            let mut sep1 = false;
            if j < b.len() && (b[j] == '-' || b[j] == '_' || b[j] == ':') {
                sep1 = true;
                j += 1;
            }
            if sep1 && j + 2 <= b.len() && b[j..j + 2].iter().all(|c| c.is_ascii_digit()) {
                let mut k = j + 2;
                let mut sep2 = false;
                if k < b.len() && (b[k] == '-' || b[k] == '_' || b[k] == ':') {
                    sep2 = true;
                    k += 1;
                }
                if sep2 && k + 2 <= b.len() && b[k..k + 2].iter().all(|c| c.is_ascii_digit()) {
                    i = k + 2;
                    continue;
                }
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// Quita las rachas largas de dígitos (números de teléfono, epoch en ms).
fn quitar_rachas_digitos(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut run = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            run.push(c);
        } else {
            if run.len() >= 5 {
                // es ruido: no se escribe
            } else {
                out.push_str(&run);
            }
            run.clear();
            out.push(c);
        }
    }
    if run.len() < 5 {
        out.push_str(&run);
    }
    out
}

/// Normaliza un nombre de fichero para comparar grabaciones de apps distintas.
///
/// Quita la extensión, las fechas, las horas, el número de teléfono y todos los
/// separadores; deja sólo minúsculas y letras/dígitos cortos.
pub fn normalizar_nombre(nombre: &str) -> String {
    let base = basename(nombre);
    let stem = match base.rsplit_once('.') {
        Some((b, e)) if !e.is_empty() && e.len() <= 5 && !e.chars().all(|c| c.is_ascii_digit()) => b,
        _ => base.as_str(),
    };
    let low = stem.to_ascii_lowercase();
    let sin_fechas = quitar_fechas(&low);
    let sin_rachas = quitar_rachas_digitos(&sin_fechas);
    sin_rachas
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

// ---------------------------------------------------------------------------
// Índice del disco
// ---------------------------------------------------------------------------

/// Fichero de audio encontrado en disco.
#[derive(Debug, Clone)]
struct FicheroDisco {
    path: PathBuf,
    bytes: u64,
    sha256: String,
    prefix: String,
    mtime: i64,
    norm: String,
    legible: bool,
}

/// SHA-256 de los primeros `n` bytes de un fichero (identidad débil).
fn prefijo_sha(path: &Path, n: usize) -> Option<String> {
    let mut f = File::open(path).ok()?;
    let mut buf = vec![0u8; n];
    let mut leidos = 0usize;
    while leidos < n {
        match f.read(&mut buf[leidos..]) {
            Ok(0) => break,
            Ok(k) => leidos += k,
            Err(_) => return None,
        }
    }
    Some(sha256_hex(&buf[..leidos]))
}

/// Recorre la raíz de audio y devuelve los ficheros de grabación.
fn escanear_disco(raiz: &str) -> Vec<FicheroDisco> {
    let mut out: Vec<FicheroDisco> = Vec::new();
    recorrer(Path::new(raiz), &mut out, 0);
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

fn recorrer(dir: &Path, out: &mut Vec<FicheroDisco>, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    let rd = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    let mut entries: Vec<fs::DirEntry> = rd.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let md = match fs::symlink_metadata(&p) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if md.file_type().is_symlink() {
            continue; // no se siguen symlinks (evita salir del árbol)
        }
        if md.is_dir() {
            recorrer(&p, out, depth + 1);
            continue;
        }
        if !md.is_file() {
            continue;
        }
        let ext = p
            .extension()
            .map(|x| x.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if !RECORDING_EXTENSIONS.contains(&ext.as_str()) {
            continue; // sólo audio
        }
        let bytes = md.len();
        let mtime = md
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let (sha, prefix, legible) = match (sha256_file(&p), prefijo_sha(&p, PREFIX_BYTES)) {
            (Some(s), Some(pfx)) => (s, pfx, true),
            _ => (String::new(), String::new(), false),
        };
        let norm = normalizar_nombre(&p.to_string_lossy());
        out.push(FicheroDisco {
            path: p,
            bytes,
            sha256: sha,
            prefix,
            mtime,
            norm,
            legible,
        });
    }
}

/// Índices de búsqueda sobre los ficheros del disco.
struct Indice {
    ficheros: Vec<FicheroDisco>,
    por_ruta: BTreeMap<String, usize>,
    por_hash: BTreeMap<String, Vec<usize>>,
    por_norm: BTreeMap<String, Vec<usize>>,
    por_tamano: BTreeMap<u64, Vec<usize>>,
}

impl Indice {
    fn construir(raiz: &str) -> Indice {
        let ficheros = escanear_disco(raiz);
        let mut por_ruta = BTreeMap::new();
        let mut por_hash: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut por_norm: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut por_tamano: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
        for (i, f) in ficheros.iter().enumerate() {
            por_ruta.insert(f.path.to_string_lossy().to_string(), i);
            if f.legible && f.bytes > 0 {
                por_hash.entry(f.sha256.clone()).or_default().push(i);
            }
            if !f.norm.is_empty() {
                por_norm.entry(f.norm.clone()).or_default().push(i);
            }
            por_tamano.entry(f.bytes).or_default().push(i);
        }
        Indice {
            ficheros,
            por_ruta,
            por_hash,
            por_norm,
            por_tamano,
        }
    }
}

/// Resuelve una ruta declarada (absoluta o relativa a la raíz de audio).
fn resolver_ruta(p: &str, raiz: &str) -> String {
    if p.starts_with('/') {
        p.to_string()
    } else {
        format!("{}/{}", raiz.trim_end_matches('/'), p.trim_start_matches("./"))
    }
}

/// Elige el mejor candidato de una lista (determinista).
fn elegir(candidatos: &[usize], d: &Declarada, idx: &Indice) -> Option<usize> {
    if candidatos.is_empty() {
        return None;
    }
    if candidatos.len() == 1 {
        return Some(candidatos[0]);
    }
    // 1) el que tiene el mismo nombre normalizado
    if let Some(n) = &d.name {
        let norm = normalizar_nombre(n);
        if !norm.is_empty() {
            for &i in candidatos {
                if idx.ficheros[i].norm == norm {
                    return Some(i);
                }
            }
        }
    }
    // 2) el cuya fecha está más cerca de la declarada
    if let Some(mt) = d.mtime {
        let mut best: Option<(i64, usize)> = None;
        for &i in candidatos {
            let diff = (idx.ficheros[i].mtime - mt).abs();
            if best.map(|(b, _)| diff < b).unwrap_or(true) {
                best = Some((diff, i));
            }
        }
        if let Some((_, i)) = best {
            return Some(i);
        }
    }
    // 3) el primero (la lista viene ordenada por ruta)
    Some(candidatos[0])
}

/// Empareja una entrada declarada con un fichero del disco.
///
/// Orden de estrategias (de la más fiable a la menos):
/// 1. **ruta** exacta,
/// 2. **hash** (SHA-256) — la fiable,
/// 3. **nombre normalizado** (sin fecha, minúsculas, sin extensión),
/// 4. **tamaño + primeros N bytes** (si la app publica la cabeza del fichero),
/// 5. **tamaño único** (heurística débil; queda marcado).
fn emparejar(d: &Declarada, raiz: &str, idx: &Indice) -> Option<(usize, &'static str)> {
    if let Some(p) = &d.path {
        let cand = resolver_ruta(p, raiz);
        if let Some(&i) = idx.por_ruta.get(&cand) {
            return Some((i, "ruta"));
        }
    }
    if let Some(h) = &d.sha256 {
        if let Some(cands) = idx.por_hash.get(h) {
            if let Some(i) = elegir(cands, d, idx) {
                return Some((i, "hash"));
            }
        }
    }
    if let Some(n) = &d.name {
        let norm = normalizar_nombre(n);
        if !norm.is_empty() {
            if let Some(cands) = idx.por_norm.get(&norm) {
                if let Some(i) = elegir(cands, d, idx) {
                    return Some((i, "nombre"));
                }
            }
        }
    }
    if let (Some(sz), Some(head)) = (d.bytes, d.head_hex.as_ref()) {
        let want = head.trim().to_ascii_lowercase();
        if !want.is_empty() {
            if let Some(cands) = idx.por_tamano.get(&sz) {
                for &i in cands {
                    let pfx = &idx.ficheros[i].prefix;
                    if !pfx.is_empty() && (pfx.starts_with(&want) || want.starts_with(pfx.as_str())) {
                        return Some((i, "tamano_prefijo"));
                    }
                }
            }
        }
    }
    if let Some(sz) = d.bytes {
        if let Some(cands) = idx.por_tamano.get(&sz) {
            if cands.len() == 1 {
                return Some((cands[0], "tamano"));
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Clasificación de la fuente (copia del mapa de `call_recorder::detect_source`)
// ---------------------------------------------------------------------------

/// Clasifica el origen de una grabación a partir de la app que la declara.
///
/// Copia del mapa de paquetes de `call_recorder::detect_source`, ampliada con
/// la grabadora **nativa de Samsung** (que graba llamadas del sistema).
pub fn detectar_fuente(app: &str) -> String {
    let p = app.trim().to_ascii_lowercase();
    if p.is_empty() {
        return "unknown".to_string();
    }
    if p.contains("whatsapp") {
        return "whatsapp".to_string();
    }
    if p.contains("telegram") || p.contains("ngram") {
        return "telegram".to_string();
    }
    if p.contains("signal") || p.contains("securesms") || p.contains("molly") {
        return "signal".to_string();
    }
    if p.contains("discord") {
        return "discord".to_string();
    }
    // La grabadora nativa de Samsung y la del sistema: llamadas del sistema.
    if p.contains("samsung")
        || p.contains("voicerecorder")
        || p.contains("voice_recorder")
        || p.contains("soundrecord")
        || p.contains("recorder")
        || p.contains("system")
        || p.contains("telecom")
        || p.contains("android")
    {
        return "system".to_string();
    }
    format!("unknown:{p}")
}

/// Deduce la app a partir de la ruta (la primera carpeta bajo la raíz).
fn deducir_app(raiz: &str, ruta: &Path) -> String {
    let r = raiz.trim_end_matches('/');
    let s = ruta.to_string_lossy().to_string();
    let rel = s.strip_prefix(r).map(|x| x.trim_start_matches('/')).unwrap_or("");
    let mut parts = rel.split('/').filter(|p| !p.is_empty());
    let first = parts.next().unwrap_or("");
    if first.is_empty() || parts.next().is_none() {
        // el fichero está directamente en la raíz
        return "raiz".to_string();
    }
    first.to_string()
}

// ---------------------------------------------------------------------------
// Reconciliación
// ---------------------------------------------------------------------------

/// Índice unificado: reconcilia las listas de los grabadores con el disco.
///
/// Devuelve las grabaciones clasificadas y el informe con los contadores.
pub fn reconciliar(lista_por_app: &[Value], raiz_audio: &str) -> (Vec<Recording>, Informe) {
    let declaradas = parse_listas(lista_por_app);
    let idx = Indice::construir(raiz_audio);
    let mut usado: Vec<bool> = vec![false; idx.ficheros.len()];
    let mut registros: Vec<Recording> = Vec::new();

    // 1) Las entradas declaradas por los grabadores.
    for d in &declaradas {
        match emparejar(d, raiz_audio, &idx) {
            Some((i, metodo)) => {
                usado[i] = true;
                let f = &idx.ficheros[i];
                let declarado = d.sha256.clone();
                let estado = if !f.legible || f.bytes == 0 {
                    // el fichero está pero no hay audio dentro (o no se puede leer)
                    RecordingState::Missing
                } else if let Some(h) = &declarado {
                    if h != &f.sha256 {
                        RecordingState::Corrupt
                    } else {
                        RecordingState::Ok
                    }
                } else {
                    RecordingState::Ok
                };
                let app = if d.app.is_empty() {
                    deducir_app(raiz_audio, &f.path)
                } else {
                    d.app.clone()
                };
                registros.push(Recording {
                    id: id_de(&app, declarado.as_deref().unwrap_or(&f.sha256)),
                    path: f.path.to_string_lossy().to_string(),
                    bytes: f.bytes,
                    sha256: declarado.clone().unwrap_or_else(|| f.sha256.clone()),
                    duration_s: d.duration_s.unwrap_or(0),
                    mtime: if f.mtime > 0 { f.mtime } else { d.mtime.unwrap_or(0) },
                    source: detectar_fuente(&app),
                    app,
                    kind: d
                        .kind
                        .clone()
                        .or_else(|| {
                            f.path
                                .extension()
                                .map(|x| x.to_string_lossy().to_string())
                        })
                        .unwrap_or_default(),
                    state: estado,
                    disk_sha256: if f.legible {
                        Some(f.sha256.clone())
                    } else {
                        None
                    },
                    declared_sha256: declarado,
                    matched_by: Some(metodo.to_string()),
                    class: d.class.clone(),
                });
            }
            None => {
                // La app declara una grabación pero no hay fichero que corresponda.
                let app = d.app.clone();
                let sha = d.sha256.clone().unwrap_or_default();
                let ruta = d
                    .path
                    .clone()
                    .map(|p| resolver_ruta(&p, raiz_audio))
                    .unwrap_or_default();
                let mtime = d.mtime.unwrap_or(0);
                registros.push(Recording {
                    id: id_de(&app, if sha.is_empty() { &ruta } else { &sha }),
                    path: ruta,
                    bytes: d.bytes.unwrap_or(0),
                    sha256: sha.clone(),
                    duration_s: d.duration_s.unwrap_or(0),
                    mtime,
                    source: detectar_fuente(&app),
                    app,
                    kind: d.kind.clone().unwrap_or_default(),
                    state: RecordingState::NoAudio,
                    disk_sha256: None,
                    declared_sha256: d.sha256.clone(),
                    matched_by: None,
                    class: d.class.clone(),
                });
            }
        }
    }

    // 2) Los ficheros del disco que nadie declara (huérfanos).
    for (i, f) in idx.ficheros.iter().enumerate() {
        if usado[i] {
            continue;
        }
        let app = deducir_app(raiz_audio, &f.path);
        registros.push(Recording {
            id: id_de(&app, &f.sha256),
            path: f.path.to_string_lossy().to_string(),
            bytes: f.bytes,
            sha256: f.sha256.clone(),
            duration_s: 0,
            mtime: f.mtime,
            source: detectar_fuente(&app),
            app,
            kind: f
                .path
                .extension()
                .map(|x| x.to_string_lossy().to_string())
                .unwrap_or_default(),
            state: RecordingState::Orphan,
            disk_sha256: if f.legible { Some(f.sha256.clone()) } else { None },
            declared_sha256: None,
            matched_by: None,
            class: None,
        });
    }

    // 3) Duplicados: el mismo SHA-256 en dos sitios.
    let grupos = grupos_duplicados(&registros);
    let paths_dup: Vec<String> = grupos
        .iter()
        .flat_map(|g| g.ficheros.iter().cloned())
        .collect();
    for r in registros.iter_mut() {
        if matches!(
            r.state,
            RecordingState::Corrupt | RecordingState::Missing | RecordingState::NoAudio
        ) {
            continue;
        }
        if !r.sha256.is_empty() && paths_dup.contains(&r.path) {
            r.state = RecordingState::Duplicate;
        }
    }

    // 4) Informe.
    let informe = informe_de(&registros);

    (registros, informe)
}

/// Calcula el informe a partir de un índice ya construido.
///
/// Los duplicados se recalculan aquí (por hash efectivo), así que el informe
/// es coherente tanto con `reconciliar()` como con `escanear()`.
pub fn informe_de(registros: &[Recording]) -> Informe {
    let grupos = grupos_duplicados(registros);
    let paths_dup: Vec<String> = grupos
        .iter()
        .flat_map(|g| g.ficheros.iter().cloned())
        .collect();
    Informe {
        total: registros.len(),
        ok: registros
            .iter()
            .filter(|r| r.state == RecordingState::Ok)
            .count(),
        missing: registros
            .iter()
            .filter(|r| r.state == RecordingState::Missing)
            .count(),
        corrupt: registros
            .iter()
            .filter(|r| r.state == RecordingState::Corrupt)
            .count(),
        orphans: registros
            .iter()
            .filter(|r| r.state == RecordingState::Orphan)
            .count(),
        duplicates: paths_dup.len(),
        total_bytes: registros.iter().map(|r| r.bytes).sum(),
        wasted_bytes: grupos.iter().map(|g| g.wasted_bytes).sum(),
        no_audio: registros
            .iter()
            .filter(|r| r.state == RecordingState::NoAudio)
            .count(),
    }
}

/// Índice unificado (sólo las grabaciones, sin el informe).
pub fn unificar(lista_por_app: &[Value], raiz_audio: &str) -> Vec<Recording> {
    reconciliar(lista_por_app, raiz_audio).0
}

/// Hash **efectivo** de una grabación: el de disco si se pudo leer (el
/// contenido real), si no el declarado.
///
/// Los duplicados se agrupan por este hash: dos ficheros con el mismo
/// **contenido** son duplicados aunque la app declarara un hash distinto (o
/// ninguno).
fn hash_efectivo(r: &Recording) -> &str {
    match &r.disk_sha256 {
        Some(s) if !s.is_empty() => s.as_str(),
        _ => r.sha256.as_str(),
    }
}

/// Agrupa las grabaciones que comparten SHA-256 (clave de agrupación: el hash).
///
/// Sólo cuenta como duplicado lo que está en **ficheros distintos**: si dos
/// apps declaran el mismo fichero, no hay copia de más (es el mismo fichero),
/// y no se marca como desperdicio.
pub fn grupos_duplicados(registros: &[Recording]) -> Vec<GrupoDuplicado> {
    let mut por_hash: BTreeMap<String, Vec<&Recording>> = BTreeMap::new();
    for r in registros {
        let h = hash_efectivo(r);
        if h.is_empty() || r.bytes == 0 {
            continue; // sin audio no se puede duplicar
        }
        por_hash.entry(h.to_string()).or_default().push(r);
    }
    let mut out = Vec::new();
    for (sha, items) in por_hash {
        let mut ficheros: Vec<String> = Vec::new();
        for r in items.iter() {
            if !ficheros.contains(&r.path) {
                ficheros.push(r.path.clone());
            }
        }
        if ficheros.len() < 2 {
            continue;
        }
        let bytes = items[0].bytes;
        let n_copias = ficheros.len() as u64;
        out.push(GrupoDuplicado {
            sha256: sha,
            ficheros,
            bytes,
            // Se conserva una copia; el resto es espacio desperdiciado.
            wasted_bytes: bytes * (n_copias - 1),
        });
    }
    out
}

/// Identificador estable de una grabación.
fn id_de(app: &str, semilla: &str) -> String {
    let s = if semilla.is_empty() { "vacio" } else { semilla };
    let corto = if s.len() > 16 { &s[..16] } else { s };
    format!("{}:{}", if app.is_empty() { "raiz" } else { app }, corto)
}

/// Inventario de **todo** lo que hay en la raíz de audio (sin listas de apps).
///
/// Es lo que usa el backup de seguridad: una copia de todo, da igual quién lo
/// declare. Los duplicados se marcan igual que en la reconciliación.
pub fn escanear(raiz_audio: &str) -> Vec<Recording> {
    let idx = Indice::construir(raiz_audio);
    let mut registros: Vec<Recording> = Vec::new();
    for f in &idx.ficheros {
        let app = deducir_app(raiz_audio, &f.path);
        registros.push(Recording {
            id: id_de(&app, &f.sha256),
            path: f.path.to_string_lossy().to_string(),
            bytes: f.bytes,
            sha256: f.sha256.clone(),
            duration_s: 0,
            mtime: f.mtime,
            source: detectar_fuente(&app),
            app,
            kind: f
                .path
                .extension()
                .map(|x| x.to_string_lossy().to_string())
                .unwrap_or_default(),
            state: if f.legible && f.bytes > 0 {
                RecordingState::Ok
            } else {
                RecordingState::Missing
            },
            disk_sha256: if f.legible { Some(f.sha256.clone()) } else { None },
            declared_sha256: None,
            matched_by: None,
            class: None,
        });
    }
    let grupos = grupos_duplicados(&registros);
    let paths_dup: Vec<String> = grupos
        .iter()
        .flat_map(|g| g.ficheros.iter().cloned())
        .collect();
    for r in registros.iter_mut() {
        if r.state == RecordingState::Ok && paths_dup.contains(&r.path) {
            r.state = RecordingState::Duplicate;
        }
    }
    registros
}

/// Reporte completo en JSON: es lo que devolverá `GET /api/recordings`.
pub fn reporte_json(lista_por_app: &[Value], raiz_audio: &str) -> Value {
    let (registros, informe) = reconciliar(lista_por_app, raiz_audio);
    let grupos = grupos_duplicados(&registros);
    json!({
        "raiz": raiz_audio,
        "informe": informe,
        "grabaciones": registros,
        "duplicados": grupos,
    })
}

// ---------------------------------------------------------------------------
// Backup de seguridad
// ---------------------------------------------------------------------------

/// Una línea del MANIFEST.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntradaManifest {
    /// Ruta **relativa** dentro del destino.
    pub path: String,
    /// SHA-256 del contenido respaldado.
    pub sha256: String,
    /// Tamaño en bytes.
    pub bytes: u64,
    /// App de origen.
    pub app: String,
    /// Ruta original en el móvil.
    pub source: String,
    /// Fecha de modificación del original (epoch, segundos).
    pub mtime: i64,
    /// Hash declarado por la app, si no cuadra con el del disco.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256_declarado: Option<String>,
    /// Estado de la grabación en el momento del backup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estado: Option<String>,
}

/// Opciones del backup.
#[derive(Debug, Clone)]
pub struct OpcionesBackup {
    /// Día (`AAAA-MM-DD`) donde se coloca la carga dentro del destino.
    pub dia: String,
    /// Si es `true`, sólo se respaldan las grabaciones en estado `Ok`/`Duplicate`.
    /// Por defecto se respaldan **todas** (incluidas huérfanas): es una copia
    /// de seguridad, no una limpieza.
    pub solo_ok: bool,
}

impl Default for OpcionesBackup {
    fn default() -> Self {
        OpcionesBackup {
            dia: today_stamp(),
            solo_ok: false,
        }
    }
}

/// Resultado de un backup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultadoBackup {
    /// Destino usado.
    pub destino: String,
    /// Ficheros copiados en esta ejecución.
    pub copiados: usize,
    /// Ficheros que ya estaban respaldados (saltados).
    pub saltados: usize,
    /// Bytes copiados en esta ejecución.
    pub bytes_copiados: u64,
    /// Ficheros verificados tras copiar (releídos y comparados).
    pub verificados: usize,
    /// Fallos (fichero que no se pudo copiar o que no cuadra al releerlo).
    pub fallos: Vec<String>,
    /// SHA-256 del MANIFEST (hash de los hashes).
    pub manifest_sha256: String,
    /// Número de entradas del MANIFEST tras el backup.
    pub total_manifest: usize,
    /// Bytes totales del MANIFEST tras el backup.
    pub bytes_manifest: u64,
    /// Número de copias redundantes que **no** se hicieron (mismo SHA-256).
    pub duplicados_omitidos: usize,
}

/// Resultado de una verificación del destino.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verificacion {
    /// Entradas del MANIFEST.
    pub total: usize,
    /// Ficheros presentes y con el hash correcto.
    pub ok: usize,
    /// Ficheros que no están en el destino.
    pub faltan: usize,
    /// Ficheros cuyo hash no cuadra con el MANIFEST.
    pub corruptos: usize,
    /// Bytes comprobados.
    pub bytes: u64,
    /// SHA-256 del MANIFEST.
    pub manifest_sha256: String,
    /// Detalle de los ficheros con problemas.
    pub problemas: Vec<String>,
}

/// Ruta por defecto del destino de backups: `<state_dir>/backups/recordings`.
pub fn destino_por_defecto(state_dir: &Path) -> PathBuf {
    state_dir.join("backups").join("recordings")
}

/// Localiza el MANIFEST: en el directorio dado o en su padre (el destino
/// puede ser la raíz del backup o la carpeta de un día concreto).
fn base_manifest(destino: &Path) -> PathBuf {
    if destino.join(MANIFEST_FILE).is_file() {
        return destino.to_path_buf();
    }
    if let Some(padre) = destino.parent() {
        if padre.join(MANIFEST_FILE).is_file() {
            return padre.to_path_buf();
        }
    }
    destino.to_path_buf()
}

/// Ruta del MANIFEST dentro del destino.
fn ruta_manifest(destino: &Path) -> PathBuf {
    base_manifest(destino).join(MANIFEST_FILE)
}

/// Lee el MANIFEST (NDJSON). Si no existe, devuelve una lista vacía.
pub fn leer_manifest(destino: &Path) -> Result<Vec<EntradaManifest>, String> {
    let ruta = ruta_manifest(destino);
    let txt = match fs::read_to_string(&ruta) {
        Ok(t) => t,
        Err(_) => return Ok(Vec::new()),
    };
    let mut out = Vec::new();
    for (n, linea) in txt.lines().enumerate() {
        let l = linea.trim();
        if l.is_empty() {
            continue;
        }
        match serde_json::from_str::<EntradaManifest>(l) {
            Ok(e) => out.push(e),
            Err(e) => {
                return Err(format!(
                    "MANIFEST {} línea {} inválida: {e}",
                    ruta.display(),
                    n + 1
                ))
            }
        }
    }
    Ok(out)
}

/// Escribe el resumen `manifest.json` (total + hash del MANIFEST).
fn escribir_resumen(base: &Path, entradas: &[EntradaManifest]) -> Result<String, String> {
    let ruta_ndjson = base.join(MANIFEST_FILE);
    let bytes = fs::read(&ruta_ndjson).unwrap_or_default();
    let hash = sha256_hex(&bytes);
    let total_bytes: u64 = entradas.iter().map(|e| e.bytes).sum();
    let resumen = json!({
        "version": 1,
        "manifest_file": MANIFEST_FILE,
        "total_files": entradas.len(),
        "total_bytes": total_bytes,
        "manifest_sha256": hash,
    });
    let ruta = base.join(MANIFEST_SUMMARY_FILE);
    let txt = serde_json::to_string_pretty(&resumen)
        .map_err(|e| format!("no se puede serializar el resumen: {e}"))?;
    fs::write(&ruta, format!("{txt}\n"))
        .map_err(|e| format!("no se puede escribir {}: {e}", ruta.display()))?;
    Ok(hash)
}

/// Ruta relativa dentro del destino para una grabación.
fn ruta_relativa(r: &Recording, dia: &str, ocupados: &BTreeMap<String, String>) -> String {
    let app = sanea(&r.app, "sin-app");
    let nombre = sanea(
        &basename(&r.path),
        &format!("{}.bin", &r.sha256[..8.min(r.sha256.len())]),
    );
    let mut rel = format!("{dia}/{app}/{nombre}");
    // Si el sitio ya está ocupado por OTRO contenido, se desambigua con el hash.
    if let Some(prev) = ocupados.get(&rel) {
        if prev != &r.sha256 {
            let (stem, ext) = match nombre.rsplit_once('.') {
                Some((s, e)) => (s.to_string(), format!(".{e}")),
                None => (nombre.clone(), String::new()),
            };
            let corto = if r.sha256.len() > 8 { &r.sha256[..8] } else { "" };
            rel = format!("{dia}/{app}/{stem}-{corto}{ext}");
        }
    }
    rel
}

/// Limpia un nombre para usarlo como parte de una ruta.
fn sanea(s: &str, por_defecto: &str) -> String {
    let limpio: String = s
        .chars()
        .map(|c| {
            if c == '/' || c == '\\' || c == ':' || c == '\0' {
                '_'
            } else {
                c
            }
        })
        .collect();
    let t = limpio.trim().trim_matches('.').to_string();
    if t.is_empty() {
        por_defecto.to_string()
    } else {
        t
    }
}

/// Copia de seguridad **incremental** de las grabaciones.
///
/// * Es **incremental**: un fichero cuyo SHA-256 ya está en el MANIFEST no se
///   vuelve a copiar (ni siquiera si cambia de nombre o de carpeta).
/// * Es **idempotente**: ejecutar dos veces seguidas no copia nada la segunda
///   vez y el MANIFEST queda **byte a byte idéntico** (por eso las líneas no
///   llevan marca de tiempo: la fecha del original sí).
/// * **Verifica**: tras copiar, relee el fichero y compara el hash. Si no
///   cuadra, el fichero **no** entra en el MANIFEST y se reporta como fallo.
/// * El MANIFEST es **NDJSON append-only** (`MANIFEST.ndjson`) y su resumen
///   (`manifest.json`) lleva el total y el hash del propio MANIFEST.
pub fn respaldar(
    grabaciones: &[Recording],
    destino: &Path,
    opts: &OpcionesBackup,
) -> Result<ResultadoBackup, String> {
    // La base es donde vive el MANIFEST: el destino dado o su padre (si el
    // destino es la carpeta de un día y el MANIFEST está en la raíz del backup).
    let base = base_manifest(destino);
    fs::create_dir_all(&base)
        .map_err(|e| format!("no se puede crear {}: {e}", base.display()))?;

    let existentes = leer_manifest(&base)?;
    let mut ocupados: BTreeMap<String, String> = BTreeMap::new();
    let mut ya: BTreeMap<String, String> = BTreeMap::new(); // sha256 -> ruta relativa
    let mut total_bytes = 0u64;
    for e in &existentes {
        ocupados.insert(e.path.clone(), e.sha256.clone());
        ya.insert(e.sha256.clone(), e.path.clone());
        total_bytes += e.bytes;
    }

    let ruta_ndjson = base.join(MANIFEST_FILE);
    let mut copiados = 0usize;
    let mut saltados = 0usize;
    let mut duplicados_omitidos = 0usize;
    let mut bytes_copiados = 0u64;
    let mut verificados = 0usize;
    let mut fallos: Vec<String> = Vec::new();
    let mut nuevas: Vec<EntradaManifest> = Vec::new();

    for r in grabaciones {
        if opts.solo_ok && !matches!(r.state, RecordingState::Ok | RecordingState::Duplicate) {
            continue;
        }
        if r.path.is_empty() {
            continue; // no hay fichero que respaldar (estado `no_audio`)
        }
        let origen = Path::new(&r.path);
        if !origen.is_file() {
            continue;
        }
        let sha = match &r.disk_sha256 {
            Some(s) if !s.is_empty() => s.clone(),
            _ => match sha256_file(origen) {
                Some(s) => s,
                None => {
                    fallos.push(format!("no se puede leer {}", origen.display()));
                    continue;
                }
            },
        };
        if ya.contains_key(&sha) {
            // Ya respaldado (con otro nombre o en otra carpeta): no se copia.
            saltados += 1;
            duplicados_omitidos += 1;
            continue;
        }
        let rel = ruta_relativa(r, &opts.dia, &ocupados);
        let destino_fichero = base.join(&rel);
        if let Some(p) = destino_fichero.parent() {
            fs::create_dir_all(p).map_err(|e| format!("no se puede crear {}: {e}", p.display()))?;
        }
        match fs::copy(origen, &destino_fichero) {
            Ok(n) => bytes_copiados += n,
            Err(e) => {
                fallos.push(format!(
                    "no se puede copiar {} -> {}: {e}",
                    origen.display(),
                    destino_fichero.display()
                ));
                continue;
            }
        }
        // Verificación: releer lo copiado y comparar.
        match sha256_file(&destino_fichero) {
            Some(s) if s == sha => {
                verificados += 1;
            }
            Some(s) => {
                fallos.push(format!(
                    "hash no cuadra tras copiar {}: esperado {sha}, calculado {s}",
                    destino_fichero.display()
                ));
                let _ = fs::remove_file(&destino_fichero);
                continue;
            }
            None => {
                fallos.push(format!(
                    "no se puede releer {}",
                    destino_fichero.display()
                ));
                let _ = fs::remove_file(&destino_fichero);
                continue;
            }
        }
        let entrada = EntradaManifest {
            path: rel.clone(),
            sha256: sha.clone(),
            bytes: r.bytes,
            app: r.app.clone(),
            source: r.path.clone(),
            mtime: r.mtime,
            sha256_declarado: r.declared_sha256.clone(),
            estado: Some(r.state.as_str().to_string()),
        };
        // NDJSON append-only.
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&ruta_ndjson)
            .map_err(|e| format!("no se puede abrir {}: {e}", ruta_ndjson.display()))?;
        let linea = serde_json::to_string(&entrada)
            .map_err(|e| format!("no se puede serializar la entrada: {e}"))?;
        writeln!(f, "{linea}").map_err(|e| format!("no se puede escribir el MANIFEST: {e}"))?;
        f.flush().ok();

        ocupados.insert(rel.clone(), sha.clone());
        ya.insert(sha, rel);
        total_bytes += r.bytes;
        copiados += 1;
        nuevas.push(entrada);
    }

    let todas = {
        let mut v = existentes;
        v.extend(nuevas);
        v
    };
    let hash = escribir_resumen(&base, &todas)?;

    Ok(ResultadoBackup {
        destino: base.to_string_lossy().to_string(),
        copiados,
        saltados,
        bytes_copiados,
        verificados,
        fallos,
        manifest_sha256: hash,
        total_manifest: todas.len(),
        bytes_manifest: total_bytes,
        duplicados_omitidos,
    })
}

/// Verifica el destino: cada entrada del MANIFEST debe estar y cuadrar.
pub fn verificar(destino: &Path) -> Result<Verificacion, String> {
    let base = base_manifest(destino);
    let entradas = leer_manifest(&base)?;
    let ruta_ndjson = base.join(MANIFEST_FILE);
    let bytes = fs::read(&ruta_ndjson).unwrap_or_default();
    let hash = sha256_hex(&bytes);
    let mut ok = 0usize;
    let mut faltan = 0usize;
    let mut corruptos = 0usize;
    let mut total_bytes = 0u64;
    let mut problemas: Vec<String> = Vec::new();
    for e in &entradas {
        let p = base.join(&e.path);
        total_bytes += e.bytes;
        if !p.is_file() {
            faltan += 1;
            problemas.push(format!("falta {}", e.path));
            continue;
        }
        match sha256_file(&p) {
            Some(s) if s == e.sha256 => ok += 1,
            Some(s) => {
                corruptos += 1;
                problemas.push(format!(
                    "{}: hash esperado {}, calculado {}",
                    e.path, e.sha256, s
                ));
            }
            None => {
                corruptos += 1;
                problemas.push(format!("no se puede leer {}", e.path));
            }
        }
    }
    Ok(Verificacion {
        total: entradas.len(),
        ok,
        faltan,
        corruptos,
        bytes: total_bytes,
        manifest_sha256: hash,
        problemas,
    })
}

// ---------------------------------------------------------------------------
// Gancho de Drive (subida por streaming, `Content-Range`)
// ---------------------------------------------------------------------------

/// Un trozo a subir a Drive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrozoDrive {
    /// Primer byte (inclusive).
    pub start: u64,
    /// Último byte (inclusive).
    pub end: u64,
    /// Cabecera `Content-Range` equivalente.
    pub content_range: String,
}

/// Item preparado para subir a Drive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemDrive {
    /// Fichero local de origen.
    pub source: String,
    /// Nombre sugerido en Drive.
    pub name: String,
    /// Tamaño total.
    pub size: u64,
    /// SHA-256 (para verificar tras subir).
    pub sha256: String,
    /// Carpeta destino en Drive.
    pub folder_id: String,
    /// Trozos (multiplos de 256 KiB, como exige Google).
    pub chunks: Vec<TrozoDrive>,
}

/// Prepara el plan de subida a Drive de las grabaciones.
///
/// Reusa `drive_client::chunk_plan` (trozos de 256 KiB) y el formato de
/// `Content-Range` de `drive_client::upload_chunk_request`. **Este módulo no
/// habla HTTP** (igual que `drive_client` y `recording_vault`): deja el plan
/// listo y quien tenga el token ejecuta
/// `open_upload_session` + un `upload_chunk_request` por trozo.
pub fn plan_drive(
    grabaciones: &[Recording],
    folder_id: &str,
    chunk_size: u64,
) -> Vec<ItemDrive> {
    grabaciones
        .iter()
        .filter(|r| !r.path.is_empty() && r.bytes > 0)
        .map(|r| {
            let chunks: Vec<TrozoDrive> = chunk_plan(r.bytes, chunk_size)
                .iter()
                .map(|c: &DriveChunkRange| TrozoDrive {
                    start: c.start,
                    end: c.end,
                    content_range: format!("bytes {}-{}/{}", c.start, c.end, r.bytes),
                })
                .collect();
            ItemDrive {
                source: r.path.clone(),
                name: format!("{}_{}", r.id.replace(':', "_"), basename(&r.path)),
                size: r.bytes,
                sha256: r.sha256.clone(),
                folder_id: folder_id.to_string(),
                chunks,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Crea un directorio temporal único para el test.
    fn tmpdir(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "md_rec_{}_{}_{}",
            tag,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn escribir(dir: &Path, rel: &str, contenido: &[u8]) -> PathBuf {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&p, contenido).unwrap();
        p
    }

    // (a) Una entrada en la lista sin fichero => SinAudio.
    #[test]
    fn entrada_declarada_sin_fichero_es_sin_audio() {
        let raiz = tmpdir("a");
        let listas = vec![json!({
            "app": "com.whatsapp",
            "recordings": [{
                "path": "/sdcard/WhatsApp/Media/llamada-2024.wav",
                "name": "llamada-2024.wav",
                "bytes": 1234,
                "sha256": sha256_hex(b"fantasma"),
                "duration_s": 42
            }]
        })];
        let (registros, informe) = reconciliar(&listas, raiz.to_str().unwrap());
        assert_eq!(registros.len(), 1);
        assert_eq!(registros[0].state, RecordingState::NoAudio);
        assert_eq!(registros[0].app, "com.whatsapp");
        assert_eq!(registros[0].duration_s, 42);
        assert_eq!(informe.no_audio, 1);
        assert_eq!(informe.total, 1);
        assert!(informe.hay_problemas());
        let _ = fs::remove_dir_all(&raiz);
    }

    // (b) Un fichero en disco que nadie declara => Huérfano.
    #[test]
    fn fichero_sin_entrada_es_huerfano() {
        let raiz = tmpdir("b");
        escribir(&raiz, "samsung/SoundRecordings/solto.amr", b"audio huerfano");
        let (registros, informe) = reconciliar(&[], raiz.to_str().unwrap());
        assert_eq!(registros.len(), 1);
        assert_eq!(registros[0].state, RecordingState::Orphan);
        assert_eq!(registros[0].bytes, b"audio huerfano".len() as u64);
        assert_eq!(informe.orphans, 1);
        assert_eq!(informe.ok, 0);
        // Un huérfano no es un "problema" de integridad.
        assert!(!informe.hay_problemas());
        let _ = fs::remove_dir_all(&raiz);
    }

    // (c) Contenido distinto del SHA declarado => Corrupto.
    #[test]
    fn contenido_distinto_del_hash_declarado_es_corrupto() {
        let raiz = tmpdir("c");
        escribir(&raiz, "grab/call.wav", b"contenido real en disco");
        let listas = vec![json!({
            "app": "telegram",
            "recordings": [{
                "path": raiz.join("grab/call.wav").to_string_lossy(),
                "name": "call.wav",
                "bytes": b"contenido real en disco".len(),
                "sha256": sha256_hex(b"otro contenido distinto")
            }]
        })];
        let (registros, informe) = reconciliar(&listas, raiz.to_str().unwrap());
        assert_eq!(registros.len(), 1);
        assert_eq!(registros[0].state, RecordingState::Corrupt);
        assert_eq!(informe.corrupt, 1);
        assert!(informe.hay_problemas());
        let _ = fs::remove_dir_all(&raiz);
    }

    // (d) Dos ficheros con el mismo contenido => duplicados + wasted_bytes.
    #[test]
    fn dos_ficheros_iguales_son_duplicados() {
        let raiz = tmpdir("d");
        escribir(&raiz, "app1/llamada.wav", b"mismo audio");
        escribir(&raiz, "app2/copia-llamada.wav", b"mismo audio");
        let (registros, informe) = reconciliar(&[], raiz.to_str().unwrap());
        assert_eq!(registros.len(), 2);
        assert!(registros
            .iter()
            .all(|r| r.state == RecordingState::Duplicate));
        assert_eq!(informe.duplicates, 2);
        assert_eq!(
            informe.wasted_bytes,
            b"mismo audio".len() as u64,
            "una copia sobrante ocupa el tamaño de un audio"
        );
        let grupos = grupos_duplicados(&registros);
        assert_eq!(grupos.len(), 1);
        assert_eq!(grupos[0].ficheros.len(), 2);
        assert_eq!(grupos[0].sha256, sha256_hex(b"mismo audio"));
        let _ = fs::remove_dir_all(&raiz);
    }

    // (e) Backup idempotente: la segunda vez no copia nada y el MANIFEST es idéntico.
    #[test]
    fn backup_es_idempotente() {
        let raiz = tmpdir("e");
        let destino = tmpdir("e_dest");
        escribir(&raiz, "a/uno.wav", b"uno");
        escribir(&raiz, "a/dos.wav", b"dos");
        let grabaciones = escanear(raiz.to_str().unwrap());
        assert_eq!(grabaciones.len(), 2);

        let opts = OpcionesBackup {
            dia: "2026-01-02".to_string(),
            solo_ok: false,
        };
        let r1 = respaldar(&grabaciones, &destino, &opts).unwrap();
        assert_eq!(r1.copiados, 2);
        assert_eq!(r1.saltados, 0);
        assert_eq!(r1.verificados, 2);
        assert!(r1.fallos.is_empty());

        let ndjson = fs::read_to_string(destino.join(MANIFEST_FILE)).unwrap();
        let resumen = fs::read_to_string(destino.join(MANIFEST_SUMMARY_FILE)).unwrap();

        // Segunda ejecución: no se copia nada y el MANIFEST es idéntico.
        let r2 = respaldar(&grabaciones, &destino, &opts).unwrap();
        assert_eq!(r2.copiados, 0, "la segunda vez no debe copiar nada");
        assert_eq!(r2.saltados, 2);
        assert_eq!(
            fs::read_to_string(destino.join(MANIFEST_FILE)).unwrap(),
            ndjson,
            "el MANIFEST debe ser idéntico"
        );
        assert_eq!(
            fs::read_to_string(destino.join(MANIFEST_SUMMARY_FILE)).unwrap(),
            resumen,
            "el resumen debe ser idéntico"
        );
        let _ = fs::remove_dir_all(&raiz);
        let _ = fs::remove_dir_all(&destino);
    }

    // (f) El MANIFEST cuadra con lo que hay en el destino.
    #[test]
    fn backup_verificado_cuadra_con_el_destino() {
        let raiz = tmpdir("f");
        let destino = tmpdir("f_dest");
        escribir(&raiz, "a/uno.wav", b"uno");
        escribir(&raiz, "b/dos.opus", b"dos");
        let grabaciones = escanear(raiz.to_str().unwrap());
        let opts = OpcionesBackup {
            dia: "2026-01-02".to_string(),
            solo_ok: false,
        };
        respaldar(&grabaciones, &destino, &opts).unwrap();

        let v = verificar(&destino).unwrap();
        assert_eq!(v.total, 2);
        assert_eq!(v.ok, 2);
        assert_eq!(v.faltan, 0);
        assert_eq!(v.corruptos, 0);
        assert!(v.problemas.is_empty());

        // El hash del MANIFEST del resumen coincide con el del fichero.
        let resumen: Value =
            serde_json::from_str(&fs::read_to_string(destino.join(MANIFEST_SUMMARY_FILE)).unwrap())
                .unwrap();
        assert_eq!(resumen["manifest_sha256"], v.manifest_sha256);
        assert_eq!(resumen["total_files"], 2);

        // Si se estropea un fichero, la verificación lo detecta.
        let entrada = leer_manifest(&destino).unwrap();
        let destino_fichero = destino.join(&entrada[0].path);
        fs::write(&destino_fichero, b"corrupto").unwrap();
        let v2 = verificar(&destino).unwrap();
        assert_eq!(v2.corruptos, 1);
        assert_eq!(v2.ok, 1);
        let _ = fs::remove_dir_all(&raiz);
        let _ = fs::remove_dir_all(&destino);
    }

    // (g) El emparejamiento por hash funciona aunque el nombre sea distinto.
    #[test]
    fn emparejamiento_por_hash_ignora_el_nombre() {
        let raiz = tmpdir("g");
        escribir(&raiz, "carpeta/2026/loquesea-0001.wav", b"contenido unico");
        let sha = sha256_hex(b"contenido unico");
        let listas = vec![json!({
            "app": "com.samsung.android.app.soundrecorder",
            "recordings": [{
                "name": "llamada-grabada-2026-01-01_10-00-00_+34600111222.wav",
                "sha256": sha
            }]
        })];
        let (registros, informe) = reconciliar(&listas, raiz.to_str().unwrap());
        assert_eq!(registros.len(), 1);
        assert_eq!(registros[0].state, RecordingState::Ok);
        assert_eq!(registros[0].matched_by.as_deref(), Some("hash"));
        assert_eq!(registros[0].sha256, sha);
        assert_eq!(registros[0].source, "system");
        assert_eq!(informe.ok, 1);
        assert_eq!(informe.no_audio, 0);
        let _ = fs::remove_dir_all(&raiz);
    }

    // Extra: la normalización de nombres quita fechas, horas y teléfono.
    #[test]
    fn normalizar_nombre_quita_fecha_hora_y_telefono() {
        assert_eq!(
            normalizar_nombre("Call_2024-01-01_10-30-00_+34600123456.wav"),
            "call"
        );
        assert_eq!(normalizar_nombre("grabacion_20240101.amr"), "grabacion");
        assert_eq!(
            normalizar_nombre("telegram/call_1.opus"),
            normalizar_nombre("call_1.opus")
        );
    }

    // Extra: el hash manda sobre el nombre (mismo nombre, distinto contenido).
    #[test]
    fn el_hash_manda_sobre_el_nombre() {
        let raiz = tmpdir("h");
        escribir(&raiz, "x/llamada.wav", b"contenido real");
        let listas = vec![json!({
            "app": "app1",
            "recordings": [{
                "name": "llamada.wav",
                "sha256": sha256_hex(b"contenido real")
            }]
        })];
        let (registros, _) = reconciliar(&listas, raiz.to_str().unwrap());
        assert_eq!(registros[0].state, RecordingState::Ok);
        assert_eq!(registros[0].matched_by.as_deref(), Some("hash"));
        let _ = fs::remove_dir_all(&raiz);
    }

    // Extra: el plan de Drive sale en trozos de 256 KiB con Content-Range.
    #[test]
    fn plan_drive_usa_trozos_con_content_range() {
        let raiz = tmpdir("i");
        escribir(&raiz, "a/uno.wav", &vec![7u8; 1024]);
        let grabaciones = escanear(raiz.to_str().unwrap());
        let plan = plan_drive(&grabaciones, "folder123", 256 * 1024);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].chunks.len(), 1);
        assert_eq!(plan[0].chunks[0].content_range, "bytes 0-1023/1024");
        assert_eq!(plan[0].folder_id, "folder123");
        let _ = fs::remove_dir_all(&raiz);
    }
}
