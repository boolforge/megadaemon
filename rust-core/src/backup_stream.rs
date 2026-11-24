//! **Subida en streaming a Drive** (multipart, paralela, reanudable).
//!
//! Este módulo **no** es un motor de backup: es la *fontanería* de la
//! subida. La idea que lo motiva es una restricción medible del móvil:
//!
//! * El S21 Ultra tiene **10 GB libres** y 225 GB usados: no se puede
//!   escribir un volcado local antes de subirlo.
//! * El usuario quiere **ver que avanza**: bytes subidos, MB/s y ETA.
//! * El usuario quiere que la subida **empiece mientras se crea** el
//!   contenido, no cuando ya está terminado.
//!
//! ## Lo que se midió contra la API real de Drive (2026-11-22)
//!
//! | Experimento | Resultado medido |
//! |---|---|
//! | Dos trozos **fuera de orden** en la misma sesión | **503** `the upload offset is 524288 byte(s), which exceeds already uploaded size of 0 byte(s)` |
//! | Último trozo con total `*` (desconocido) | **308** para siempre: la sesión **nunca** finaliza |
//! | Último trozo con total conocido (`bytes a-b/TOTAL`) | **200** + el recurso (`id`, `md5Checksum`, `size`) |
//! | Estado de la sesión (`PUT` con `bytes */TOTAL`, cuerpo vacío) | **308** + cabecera `Range: bytes=0-N` ⇒ se puede **reanudar** |
//!
//! Conclusión de diseño: **Drive exige trozos secuenciales por sesión**, así
//! que la paralelización **no** puede ser "N trozos de la misma sesión" sino
//! **N sesiones**: el flujo se parte en `N` tramos contiguos, cada uno con su
//! propia sesión reanudable y su propio objeto en Drive, subidos a la vez.
//! Un objeto `MANIFEST.json` ata los tramos: tamaño, SHA-256 global y
//! SHA-256 de cada trozo.
//!
//! ## El embudo
//!
//! ```text
//!   productor ──► Pipeline (cola acotada = contrapresión)
//!                    │  trozo i → tramo (i·N/T)
//!                    ▼
//!              N conexiones (una sesión de Drive por tramo)
//!                    │  Content-Range: bytes off-(off+len-1)/total_del_tramo
//!                    ▼
//!              Drive  ──►  MANIFEST.json (al final)
//! ```
//!
//! La cola acotada es lo que hace que la memoria no crezca: el productor se
//! bloquea cuando el receptor va lento ⇒ **subir mientras se crea** sin
//! cargar el volcado en RAM ni en disco.

use crate::drive_client::{file_metadata, DriveTarget, DRIVE_UPLOAD_API};
use crate::google_auth;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Constantes
// ---------------------------------------------------------------------------

/// Google exige trozos múltiplos de **256 KiB** (menos el último).
pub const DRIVE_GRANULARITY: u64 = 256 * 1024;
/// Tamaño de trozo por defecto (8 MiB = 32 × 256 KiB).
pub const DEFAULT_CHUNK_BYTES: u64 = 8 * 1024 * 1024;
/// Conexiones paralelas por defecto (el usuario pidió 2-4).
pub const DEFAULT_CONNECTIONS: usize = 3;

// ---------------------------------------------------------------------------
// Trozos
// ---------------------------------------------------------------------------

/// Ficha de un trozo (lo que va al manifiesto).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkRecord {
    /// Índice global del trozo (0, 1, 2…).
    pub index: u64,
    /// Offset dentro del **tramo** al que pertenece.
    pub offset: u64,
    /// Longitud en bytes.
    pub len: u64,
    /// SHA-256 del contenido del trozo.
    pub sha256: String,
}

/// Un trozo en vuelo: datos + ficha.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// Índice global del trozo.
    pub index: u64,
    /// Tramo (conexión) al que pertenece.
    pub stream: usize,
    /// Offset dentro del tramo.
    pub offset: u64,
    /// Datos.
    pub data: Vec<u8>,
    /// SHA-256 de los datos.
    pub sha256: String,
}

impl Chunk {
    /// Longitud del trozo.
    pub fn len(&self) -> u64 {
        self.data.len() as u64
    }

    /// ¿Troz vacío?
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Resumen de un tramo subido.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamSummary {
    /// Índice del tramo.
    pub stream: usize,
    /// Nombre del objeto en Drive.
    pub name: String,
    /// Id del objeto en Drive.
    pub file_id: String,
    /// Bytes subidos.
    pub bytes: u64,
    /// Trozos subidos.
    pub chunks: u64,
}

/// Resumen de una subida completa.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UploadSummary {
    /// Bytes subidos (suma de todos los tramos).
    pub bytes: u64,
    /// Trozos subidos.
    pub chunks: u64,
    /// Tramos (uno por conexión).
    pub streams: usize,
    /// Ids de los objetos en Drive.
    pub file_ids: Vec<String>,
    /// Nombres de los objetos en Drive.
    pub names: Vec<String>,
    /// SHA-256 del flujo completo (orden original).
    pub sha256: String,
    /// Etiqueta que se le puso al flujo.
    pub label: String,
}

// ---------------------------------------------------------------------------
// Destino de los trozos
// ---------------------------------------------------------------------------

/// Error del destino de los trozos.
pub type SinkResult = Result<(), String>;

/// Destino de los trozos.
///
/// La unidad de trabajo es el **tramo** (`stream`): cada tramo es una sesión
/// independiente (⇒ conexión independiente ⇒ subida en paralelo).
pub trait ChunkSink: Send + Sync {
    /// Prepara un tramo: abre su sesión y dice cuántos bytes van a ir.
    fn open_stream(&self, stream: usize, total: u64) -> SinkResult;

    /// Sube un trozo de un tramo. Los trozos de un mismo tramo **deben**
    /// llegar en orden y con el offset exacto.
    fn upload(&self, stream: usize, chunk: &Chunk) -> SinkResult;

    /// Cierra un tramo ya subido.
    fn close_stream(&self, stream: usize) -> Result<StreamSummary, String>;

    /// Cierra todo y devuelve el resumen.
    fn finish(&self) -> Result<UploadSummary, String>;

    /// ¿Este trozo ya estaba subido (reanudación)?
    fn already_done(&self, _stream: usize, _chunk: &Chunk) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// Progreso
// ---------------------------------------------------------------------------

/// Estado del progreso (compartido entre el productor y los trabajadores).
#[derive(Debug)]
pub struct Progress {
    bytes: AtomicU64,
    chunks: AtomicU64,
    errors: AtomicU64,
    total: AtomicU64,
    started: Instant,
}

/// Foto del progreso.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProgressSnapshot {
    /// Bytes ya subidos.
    pub bytes_uploaded: u64,
    /// Bytes que hay en total (0 = desconocido).
    pub total_bytes: u64,
    /// Velocidad media en bytes por segundo.
    pub bytes_per_sec: f64,
    /// Velocidad media en MB/s (decimal).
    pub mb_per_sec: f64,
    /// Segundos que faltan (si se conoce el total).
    pub eta_secs: Option<u64>,
    /// Trozos subidos.
    pub chunks_done: u64,
    /// Trozos fallidos.
    pub chunks_failed: u64,
    /// Segundos transcurridos.
    pub elapsed_secs: u64,
}

impl Progress {
    /// Progresso nuevo.
    pub fn new(total_bytes: u64) -> Progress {
        Progress {
            bytes: AtomicU64::new(0),
            chunks: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            total: AtomicU64::new(total_bytes),
            started: Instant::now(),
        }
    }

    /// Suma bytes subidos.
    pub fn add_bytes(&self, n: u64) {
        self.bytes.fetch_add(n, Ordering::Relaxed);
    }

    /// Suma un trozo terminado.
    pub fn add_chunk(&self) {
        self.chunks.fetch_add(1, Ordering::Relaxed);
    }

    /// Suma un fallo.
    pub fn add_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Foto del progreso.
    pub fn snapshot(&self) -> ProgressSnapshot {
        let bytes = self.bytes.load(Ordering::Relaxed);
        let total = self.total.load(Ordering::Relaxed);
        let elapsed = self.started.elapsed().as_secs_f64().max(0.001);
        let bps = bytes as f64 / elapsed;
        let eta = if total > 0 && bytes < total {
            Some(((total - bytes) as f64 / bps.max(1.0)) as u64)
        } else {
            None
        };
        ProgressSnapshot {
            bytes_uploaded: bytes,
            total_bytes: total,
            bytes_per_sec: bps,
            mb_per_sec: bps / 1_000_000.0,
            eta_secs: eta,
            chunks_done: self.chunks.load(Ordering::Relaxed),
            chunks_failed: self.errors.load(Ordering::Relaxed),
            elapsed_secs: self.started.elapsed().as_secs(),
        }
    }

    /// Línea legible para el usuario.
    pub fn line(&self) -> String {
        let s = self.snapshot();
        let pct = if s.total_bytes > 0 {
            format!("{:.1}%", (s.bytes_uploaded as f64 * 100.0) / s.total_bytes as f64)
        } else {
            "-".to_string()
        };
        let eta = match s.eta_secs {
            Some(e) => {
                let h = e / 3600;
                let m = (e % 3600) / 60;
                let sec = e % 60;
                format!("{h:02}:{m:02}:{sec:02}")
            }
            None => "--:--:--".to_string(),
        };
        format!(
            "{} / {} MiB ({} %) · {:.2} MB/s · ETA {} · {} trozos",
            human_mib(s.bytes_uploaded),
            human_mib(s.total_bytes),
            pct,
            s.mb_per_sec,
            eta,
            s.chunks_done
        )
    }
}

/// Bytes en MiB (con decimales).
pub fn human_mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

/// Imprime el progreso cada `interval` mientras el trabajo dure.
pub struct ProgressReporter {
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl ProgressReporter {
    /// Arranca el hilo que informa.
    pub fn start(progress: Arc<Progress>, interval: Duration) -> ProgressReporter {
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        let p = progress.clone();
        let handle = std::thread::spawn(move || {
            while !s.load(Ordering::Relaxed) {
                std::thread::sleep(interval);
                if s.load(Ordering::Relaxed) {
                    break;
                }
                eprintln!("[md-backup] {}", p.line());
            }
        });
        ProgressReporter {
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for ProgressReporter {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

// ---------------------------------------------------------------------------
// El pipeline
// ---------------------------------------------------------------------------

/// Error del pipeline.
pub type PipeResult = Result<(), String>;

/// Una conexión: cola propia + hilo propio.
///
/// Cada conexión es un **tramo** del flujo: dentro de un tramo los trozos
/// van **en orden** (Drive lo exige, medido) y los tramos van **en paralelo**.
struct Connection {
    tx: Option<SyncSender<Chunk>>,
    handle: Option<std::thread::JoinHandle<Result<(), String>>>,
    progress: Arc<Progress>,
    failed: Arc<AtomicBool>,
}

/// Tubería de subida: produce → reparte → N conexiones → Drive.
pub struct Pipeline {
    connections: Vec<Connection>,
    sink: Arc<dyn ChunkSink>,
    progress: Arc<Progress>,
    /// Hash del flujo completo (se calcula en orden, en el productor).
    hasher: Sha256,
    /// Índice del siguiente trozo.
    next_index: u64,
    /// Bytes escritos hasta ahora.
    written: u64,
    /// Tamaño de trozo.
    chunk_bytes: u64,
    /// Total del flujo.
    total_bytes: u64,
    /// Reparto de trozos por conexión.
    plan: Vec<(usize, u64, u64)>, // (conexión, primer índice, nº de trozos)
    /// Etiqueta del flujo.
    label: String,
    /// Trozos ya subidos (reanudación): clave = índice global.
    done: BTreeMap<u64, String>,
}

impl Pipeline {
    /// Crea la tubería.
    ///
    /// * `total_bytes`: tamaño **total** del flujo (obligatorio: Drive no
    ///   finaliza una sesión sin total conocido, medido).
    /// * `chunk_bytes`: tamaño del trozo (múltiplo de 256 KiB).
    /// * `connections`: cuántos tramos en paralelo (2-4 pidió el usuario).
    /// * `done`: trozos ya subidos en una ejecución anterior (reanudación).
    pub fn new(
        sink: Arc<dyn ChunkSink>,
        label: &str,
        total_bytes: u64,
        chunk_bytes: u64,
        connections: usize,
        done: BTreeMap<u64, String>,
    ) -> Result<Pipeline, String> {
        if chunk_bytes == 0 || chunk_bytes % DRIVE_GRANULARITY != 0 {
            return Err(format!(
                "el tamaño de trozo ({chunk_bytes}) debe ser múltiplo de {DRIVE_GRANULARITY} (lo exige Drive)"
            ));
        }
        let connections = connections.max(1);
        let n_chunks = if total_bytes == 0 {
            0
        } else {
            (total_bytes + chunk_bytes - 1) / chunk_bytes
        };
        if n_chunks == 0 {
            return Err("el flujo está vacío: no hay nada que subir".to_string());
        }
        // Reparto contiguo y **alineado a trozo**: el tramo `c` recibe los
        // trozos [ini, fin). Cada tramo tiene su propio tamaño.
        let mut plan: Vec<(usize, u64, u64)> = Vec::new();
        let base = n_chunks / connections as u64;
        let mut extra = n_chunks % connections as u64;
        let mut ini = 0u64;
        for c in 0..connections {
            let mut n = base;
            if extra > 0 {
                n += 1;
                extra -= 1;
            }
            plan.push((c, ini, n));
            ini += n;
        }

        let progress = Arc::new(Progress::new(total_bytes));
        let mut conns = Vec::new();
        for (c, ini, n) in &plan {
            // Bytes del tramo: de su primer trozo al final del último.
            let start = ini * chunk_bytes;
            let end = ((ini + n) * chunk_bytes).min(total_bytes);
            let stream_total = end.saturating_sub(start);
            let stream_idx = *c;
            sink.open_stream(stream_idx, stream_total)?;
            let (tx, rx) = sync_channel::<Chunk>(4);
            let sink2 = sink.clone();
            let prog2 = progress.clone();
            let failed = Arc::new(AtomicBool::new(false));
            let failed2 = failed.clone();
            let handle = std::thread::spawn(move || -> Result<(), String> {
                while let Ok(chunk) = rx.recv() {
                    if failed2.load(Ordering::Relaxed) {
                        // Ya hay un error: no gastes más red.
                        continue;
                    }
                    if let Err(e) = sink2.upload(stream_idx, &chunk) {
                        failed2.store(true, Ordering::Relaxed);
                        prog2.add_error();
                        return Err(format!("tramo {stream_idx}, trozo {}: {e}", chunk.index));
                    }
                    prog2.add_bytes(chunk.len());
                    prog2.add_chunk();
                }
                Ok(())
            });
            conns.push(Connection {
                tx: Some(tx),
                handle: Some(handle),
                progress: progress.clone(),
                failed: failed.clone(),
            });
        }

        Ok(Pipeline {
            connections: conns,
            sink,
            progress,
            hasher: Sha256::new(),
            next_index: 0,
            written: 0,
            chunk_bytes,
            total_bytes,
            plan,
            label: label.to_string(),
            done,
        })
    }

    /// Progreso compartido.
    pub fn progress(&self) -> Arc<Progress> {
        self.progress.clone()
    }

    /// Etiqueta del flujo.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// ¿Cuántos trozos lleva el flujo?
    pub fn total_chunks(&self) -> u64 {
        self.plan.iter().map(|(_, _, n)| *n).sum()
    }

    /// Trozos ya subidos de una ejecución anterior.
    pub fn skipped(&self) -> usize {
        self.done.len()
    }

    /// Empuja un trozo. Calcula su SHA-256 y lo manda a su conexión.
    ///
    /// Devuelve el SHA-256 del trozo (para el manifiesto final).
    pub fn push(&mut self, data: Vec<u8>) -> Result<String, String> {
        if data.is_empty() {
            return Ok(String::new());
        }
        let index = self.next_index;
        self.next_index += 1;
        self.hasher.update(&data);
        self.written += data.len() as u64;

        let (conn, offset) = self.locate(index)?;
        let sha = hex(&Sha256::digest(&data));
        let chunk = Chunk {
            index,
            stream: conn,
            offset,
            data,
            sha256: sha.clone(),
        };
        if self.done.contains_key(&index) {
            // Ya estaba subido en otra ejecución: no se vuelve a subir, pero
            // el hash se recalcula para que el manifiesto sea el de verdad.
            self.done.insert(index, sha.clone());
            return Ok(sha);
        }
        let c = &mut self.connections[conn];
        let tx = c.tx.as_ref().ok_or("la tubería está cerrada")?;
        tx.send(chunk)
            .map_err(|_| "la conexión se cerró antes de tiempo".to_string())?;
        Ok(sha)
    }

    /// A qué conexión y con qué offset va el trozo `index`.
    fn locate(&self, index: u64) -> Result<(usize, u64), String> {
        for (c, ini, n) in &self.plan {
            if index >= *ini && index < *ini + *n {
                return Ok((*c, (index - *ini) * self.chunk_bytes));
            }
        }
        Err(format!("trozo {index} fuera del reparto"))
    }

    /// Cierra la tubería y devuelve el resumen.
    pub fn finish(mut self) -> Result<UploadSummary, String> {
        // Cierra las colas: los hilos terminan cuando la cola se vacía.
        for c in self.connections.iter_mut() {
            c.tx = None;
        }
        let mut first_error: Option<String> = None;
        for c in self.connections.iter_mut() {
            if let Some(h) = c.handle.take() {
                match h.join() {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        if first_error.is_none() {
                            first_error = Some(e);
                        }
                    }
                    Err(_) => {
                        if first_error.is_none() {
                            first_error = Some("un hilo de subida ha petado".to_string());
                        }
                    }
                }
            }
        }
        if let Some(e) = first_error {
            return Err(e);
        }
        let mut ids = Vec::new();
        let mut names = Vec::new();
        let mut chunks = 0u64;
        for (i, (_c, _ini, n)) in self.plan.iter().enumerate() {
            let s = self.sink.close_stream(i)?;
            chunks += s.chunks;
            ids.push(s.file_id);
            names.push(s.name);
        }
        let summary = self.sink.finish()?;
        Ok(UploadSummary {
            bytes: summary.bytes,
            chunks,
            streams: self.plan.len(),
            file_ids: ids,
            names,
            sha256: hex(&self.hasher.finalize()),
            label: self.label.clone(),
        })
    }
}

/// Bytes que le tocan a un tramo, dado el reparto de trozos.
///
/// El último trozo del flujo puede ser más corto que los demás, así que el
/// tramo se calcula con los **extremos reales** (del primer trozo al final
/// del último), nunca con una multiplicación ciega.
pub fn stream_bytes(total: u64, chunk_bytes: u64, first_chunk: u64, n_chunks_in_stream: u64) -> u64 {
    if n_chunks_in_stream == 0 {
        return 0;
    }
    let start = first_chunk * chunk_bytes;
    let end = ((first_chunk + n_chunks_in_stream) * chunk_bytes).min(total);
    end.saturating_sub(start)
}

// ---------------------------------------------------------------------------
// Escritor que trocea
// ---------------------------------------------------------------------------

/// `Write` que acumula en trozos y los empuja a la tubería.
///
/// Es lo que permite **subir mientras se crea**: el productor escribe el
/// contenido (el TAR del perfil, o los bloques de una partición) y cada vez
/// que se llena un trozo, sale hacia Drive.
pub struct ChunkWriter<'a> {
    pipeline: &'a mut Pipeline,
    buf: Vec<u8>,
    chunk_bytes: usize,
    written: u64,
}

impl<'a> ChunkWriter<'a> {
    /// Envuelve la tubería.
    pub fn new(pipeline: &'a mut Pipeline, chunk_bytes: usize) -> ChunkWriter<'a> {
        ChunkWriter {
            pipeline,
            buf: Vec::with_capacity(chunk_bytes),
            chunk_bytes,
            written: 0,
        }
    }

    /// Bytes escritos hasta ahora.
    pub fn written(&self) -> u64 {
        self.written
    }
}

impl Write for ChunkWriter<'_> {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        let mut off = 0usize;
        while off < data.len() {
            let room = self.chunk_bytes - self.buf.len();
            let take = room.min(data.len() - off);
            self.buf.extend_from_slice(&data[off..off + take]);
            off += take;
            if self.buf.len() >= self.chunk_bytes {
                let full = std::mem::take(&mut self.buf);
                self.buf = Vec::with_capacity(self.chunk_bytes);
                self.written += full.len() as u64;
                self.pipeline.push(full).map_err(std::io::Error::other)?;
            }
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Vacía el resto del búfer (el último trozo suele ser menor).
pub fn flush_writer(w: &mut ChunkWriter<'_>) -> Result<(), String> {
    let rest = std::mem::take(&mut w.buf);
    if !rest.is_empty() {
        w.written += rest.len() as u64;
        w.pipeline.push(rest).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Destino de prueba (sin red)
// ---------------------------------------------------------------------------

/// Destino que escribe los trozos en ficheros locales (para tests y para
/// `--simulate`): **no** toca la red.
pub struct FileSink {
    dir: PathBuf,
    prefix: String,
    streams: Mutex<BTreeMap<usize, FileStreamState>>,
    hasher: Mutex<Sha256>,
}

/// Estado de un tramo del [`FileSink`].
struct FileStreamState {
    file: std::fs::File,
    written: u64,
    chunks: u64,
    path: PathBuf,
}

impl FileSink {
    /// Crea el destino en un directorio.
    pub fn new(dir: &Path, prefix: &str) -> Result<FileSink, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("crear {}: {e}", dir.display()))?;
        Ok(FileSink {
            dir: dir.to_path_buf(),
            prefix: prefix.to_string(),
            streams: Mutex::new(BTreeMap::new()),
            hasher: Mutex::new(Sha256::new()),
        })
    }
}

impl ChunkSink for FileSink {
    fn open_stream(&self, stream: usize, _total: u64) -> SinkResult {
        let path = self.dir.join(format!("{}.part{:02}", self.prefix, stream));
        let file = std::fs::File::create(&path).map_err(|e| format!("crear {}: {e}", path.display()))?;
        let mut st = self.streams.lock().map_err(|_| "estado envenenado")?;
        st.insert(
            stream,
            FileStreamState {
                file,
                written: 0,
                chunks: 0,
                path,
            },
        );
        Ok(())
    }

    fn upload(&self, stream: usize, chunk: &Chunk) -> SinkResult {
        let mut st = self.streams.lock().map_err(|_| "estado envenenado")?;
        let s = st
            .get_mut(&stream)
            .ok_or_else(|| format!("el tramo {stream} no está abierto"))?;
        s.file
            .write_all(&chunk.data)
            .map_err(|e| format!("escribir tramo {stream}: {e}"))?;
        s.written += chunk.len();
        s.chunks += 1;
        let mut h = self.hasher.lock().map_err(|_| "hash envenenado")?;
        h.update(&chunk.data);
        Ok(())
    }

    fn close_stream(&self, stream: usize) -> Result<StreamSummary, String> {
        let mut st = self.streams.lock().map_err(|_| "estado envenenado")?;
        let s = st
            .remove(&stream)
            .ok_or_else(|| format!("el tramo {stream} no está abierto"))?;
        Ok(StreamSummary {
            stream,
            name: s
                .path
                .file_name()
                .map(|x| x.to_string_lossy().to_string())
                .unwrap_or_default(),
            file_id: s.path.to_string_lossy().to_string(),
            bytes: s.written,
            chunks: s.chunks,
        })
    }

    fn finish(&self) -> Result<UploadSummary, String> {
        let h = self.hasher.lock().map_err(|_| "hash envenenado")?;
        Ok(UploadSummary {
            bytes: 0,
            chunks: 0,
            streams: 0,
            file_ids: Vec::new(),
            names: Vec::new(),
            sha256: hex(&h.clone().finalize()),
            label: self.prefix.clone(),
        })
    }
}

// ---------------------------------------------------------------------------
// Destino real: Google Drive (multipart reanudable)
// ---------------------------------------------------------------------------

/// Estado de reanudación (se guarda en disco: si el daemon muere, se sigue).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UploadState {
    /// Etiqueta del flujo.
    pub label: String,
    /// Tamaño total del flujo.
    pub total_bytes: u64,
    /// Tamaño de trozo.
    pub chunk_bytes: u64,
    /// Número de conexiones.
    pub connections: usize,
    /// Por conexión: la URI de la sesión reanudable.
    pub sessions: BTreeMap<usize, String>,
    /// Por conexión: trozos ya subidos (índice global → SHA-256).
    pub uploaded: BTreeMap<usize, BTreeMap<u64, String>>,
    /// Momento de creación (epoch).
    pub created_at: u64,
}

impl UploadState {
    /// Estado nuevo.
    pub fn new(label: &str, total_bytes: u64, chunk_bytes: u64, connections: usize) -> UploadState {
        UploadState {
            label: label.to_string(),
            total_bytes,
            chunk_bytes,
            connections,
            sessions: BTreeMap::new(),
            uploaded: BTreeMap::new(),
            created_at: now_secs(),
        }
    }

    /// Trozos ya subidos (índice global → hash).
    pub fn done(&self) -> BTreeMap<u64, String> {
        let mut out = BTreeMap::new();
        for (_, m) in self.uploaded.iter() {
            for (k, v) in m.iter() {
                out.insert(*k, v.clone());
            }
        }
        out
    }

    /// Guarda el estado en disco (atómico: fichero temporal + rename).
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).map_err(|e| format!("crear {}: {e}", p.display()))?;
        }
        let tmp = path.with_extension("tmp");
        let data = serde_json::to_vec_pretty(self).map_err(|e| format!("json: {e}"))?;
        std::fs::write(&tmp, data).map_err(|e| format!("escribir {}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| format!("renombrar a {}: {e}", path.display()))?;
        Ok(())
    }

    /// Carga el estado de disco (si existe).
    pub fn load(path: &Path) -> Option<UploadState> {
        let data = std::fs::read(path).ok()?;
        serde_json::from_slice(&data).ok()
    }
}

/// Estado interno de una conexión de Drive.
struct DriveStream {
    session: String,
    total: u64,
    written: u64,
    chunks: u64,
    name: String,
    file_id: String,
    /// Trozos ya subidos (índice global → hash).
    done: BTreeMap<u64, String>,
}

/// Destino real: Drive, con una sesión reanudable por conexión.
pub struct DriveSink {
    /// Nombre base de los objetos.
    pub name: String,
    /// Carpeta destino en Drive.
    pub target: DriveTarget,
    /// Dónde está la credencial (state_dir).
    pub state_dir: Option<String>,
    /// Ruta del fichero de estado (reanudación).
    pub state_path: Option<PathBuf>,
    streams: Mutex<BTreeMap<usize, DriveStream>>,
    /// Un candado por conexión: Drive exige trozos **en orden** dentro de la
    /// misma sesión (medido: fuera de orden ⇒ 503).
    locks: Mutex<BTreeMap<usize, Arc<Mutex<()>>>>,
    /// Estado de reanudación (se guarda en disco).
    state: Mutex<UploadState>,
    total_bytes: Mutex<u64>,
    label: String,
}

impl DriveSink {
    /// Crea el destino.
    ///
    /// Si hay un fichero de estado previo **del mismo flujo y tamaño**, se
    /// carga: así una subida interrumpida **se reanuda** (la sesión de Drive
    /// se reutiliza y se pregunta al servidor cuánto recibió).
    pub fn new(
        name: &str,
        target: DriveTarget,
        state_dir: Option<String>,
        state_path: Option<PathBuf>,
        total_bytes: u64,
    ) -> DriveSink {
        let mut state = UploadState::new(name, total_bytes, DEFAULT_CHUNK_BYTES, 1);
        if let Some(p) = &state_path {
            if let Some(s) = UploadState::load(p) {
                if s.total_bytes == total_bytes && s.label == name {
                    state = s;
                }
            }
        }
        DriveSink {
            name: name.to_string(),
            target,
            state_dir,
            state_path,
            streams: Mutex::new(BTreeMap::new()),
            locks: Mutex::new(BTreeMap::new()),
            state: Mutex::new(state),
            total_bytes: Mutex::new(total_bytes),
            label: name.to_string(),
        }
    }

    /// Agente HTTP con tiempos acotados (como el de `drive_native`).
    fn agent() -> ureq::Agent {
        let cfg = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(900)))
            .timeout_connect(Some(Duration::from_secs(20)))
            .http_status_as_error(false)
            .build();
        ureq::Agent::new_with_config(cfg)
    }

    /// Abre (o reutiliza) la sesión reanudable de una conexión.
    ///
    /// Devuelve la URI de la sesión y cuántos bytes tiene ya recibidos.
    fn open_session(
        &self,
        stream: usize,
        total: u64,
    ) -> Result<(String, u64, BTreeMap<u64, String>), String> {
        // 1) Reanudación: si hay sesión guardada, se pregunta al servidor
        //    cuántos bytes ha recibido (medido: PUT con `bytes */TOTAL` → 308
        //    + cabecera `Range`).
        let saved = {
            let st = self.state.lock().map_err(|_| "estado envenenado")?;
            st.sessions.get(&stream).cloned()
        };
        if let Some(sess) = saved {
            if let Some(off) = resume_offset(&sess, total, self.state_dir.as_deref()) {
                if off > 0 {
                    let mut st = self.state.lock().map_err(|_| "estado envenenado")?;
                    let done = st.uploaded.get(&stream).cloned().unwrap_or_default();
                    return Ok((sess, off, done));
                }
            }
        }
        // 2) Sesión nueva.
        let name = format!("{}.part{:02}", self.name, stream);
        let meta = file_metadata(&name, total, &self.target);
        let (ak, av) = google_auth::auth_header(self.state_dir.as_deref())?;
        let url = format!(
            "{}?uploadType=resumable&supportsAllDrives=true",
            DRIVE_UPLOAD_API
        );
        let resp = Self::agent()
            .post(&url)
            .header(ak, av)
            .header("Content-Type", "application/json; charset=UTF-8")
            .header("X-Upload-Content-Length", total.to_string())
            .send(meta.as_bytes())
            .map_err(|e| format!("abrir sesión de subida: {e}"))?;
        let status = resp.status().as_u16();
        if status != 200 && status != 201 {
            let b = resp.into_body().read_to_vec().unwrap_or_default();
            let txt: String = String::from_utf8_lossy(&b).chars().take(200).collect();
            return Err(format!("abrir sesión devolvió {status}: {txt}"));
        }
        let sess = resp
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .ok_or("la sesión de subida no trae la cabecera Location")?
            .to_string();
        Ok((sess, 0, BTreeMap::new()))
    }

    /// Guarda el estado de reanudación en disco.
    fn save_state(&self) -> Result<(), String> {
        if let Some(p) = &self.state_path {
            let st = self.state.lock().map_err(|_| "estado envenenado")?;
            st.save(p)?;
        }
        Ok(())
    }

    /// El PUT de verdad.
    fn upload_inner(&self, stream: usize, chunk: &Chunk) -> SinkResult {
        let (session, total) = {
            let st = self.streams.lock().map_err(|_| "estado envenenado")?;
            let s = st
                .get(&stream)
                .ok_or_else(|| format!("el tramo {stream} no está abierto"))?;
            (s.session.clone(), s.total)
        };
        let end = chunk.offset + chunk.len().saturating_sub(1);
        let (ak, av) = google_auth::auth_header(self.state_dir.as_deref())?;
        let resp = Self::agent()
            .put(&session)
            .header(ak, av)
            .header(
                "Content-Range",
                format!("bytes {}-{}/{}", chunk.offset, end, total),
            )
            .header("Content-Length", chunk.len().to_string())
            .send(&chunk.data)
            .map_err(|e| format!("subir trozo {}: {e}", chunk.index))?;
        let status = resp.status().as_u16();
        let body = resp.into_body().read_to_vec().unwrap_or_default();
        if status == 401 {
            google_auth::invalidate();
            return Err(format!("token de Google caducado en el trozo {}", chunk.index));
        }
        if status != 200 && status != 201 && status != 308 {
            let txt: String = String::from_utf8_lossy(&body).chars().take(200).collect();
            return Err(format!("trozo {} devolvió {status}: {txt}", chunk.index));
        }
        // El último trozo (el que cierra el total) devuelve el recurso.
        if status == 200 || status == 201 {
            if let Ok(j) = serde_json::from_slice::<Value>(&body) {
                if let Some(id) = j.get("id").and_then(|v| v.as_str()) {
                    let mut st = self.streams.lock().map_err(|_| "estado envenenado")?;
                    if let Some(s) = st.get_mut(&stream) {
                        s.file_id = id.to_string();
                    }
                }
            }
        }
        {
            let mut st = self.streams.lock().map_err(|_| "estado envenenado")?;
            if let Some(s) = st.get_mut(&stream) {
                s.written = s.written.max(chunk.offset + chunk.len());
                s.chunks += 1;
                s.done.insert(chunk.index, chunk.sha256.clone());
            }
        }
        {
            let mut st = self.state.lock().map_err(|_| "estado envenenado")?;
            st.uploaded
                .entry(stream)
                .or_default()
                .insert(chunk.index, chunk.sha256.clone());
        }
        self.save_state()?;
        Ok(())
    }
}

impl ChunkSink for DriveSink {
    fn open_stream(&self, stream: usize, total: u64) -> SinkResult {
        let (sess, resumed, done) = self.open_session(stream, total)?;
        {
            let mut st = self.streams.lock().map_err(|_| "estado envenenado")?;
            st.insert(
                stream,
                DriveStream {
                    session: sess.clone(),
                    total,
                    written: resumed,
                    chunks: done.len() as u64,
                    name: format!("{}.part{:02}", self.name, stream),
                    file_id: String::new(),
                    done: done.clone(),
                },
            );
        }
        {
            let mut l = self.locks.lock().map_err(|_| "estado envenenado")?;
            l.entry(stream).or_insert_with(|| Arc::new(Mutex::new(())));
        }
        {
            let mut st = self.state.lock().map_err(|_| "estado envenenado")?;
            st.sessions.insert(stream, sess);
            st.uploaded.insert(stream, done);
        }
        self.save_state()?;
        Ok(())
    }

    fn upload(&self, stream: usize, chunk: &Chunk) -> SinkResult {
        // Serializa los trozos de esta conexión (Drive los exige en orden:
        // medido ⇒ fuera de orden devuelve 503).
        let lock = {
            let mut locks = self.locks.lock().map_err(|_| "estado envenenado")?;
            locks
                .entry(stream)
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _guard = lock.lock().map_err(|_| "candado envenenado")?;
        self.upload_inner(stream, chunk)
    }

    fn close_stream(&self, stream: usize) -> Result<StreamSummary, String> {
        let mut st = self.streams.lock().map_err(|_| "estado envenenado")?;
        let s = st
            .get_mut(&stream)
            .ok_or_else(|| format!("el tramo {stream} no está abierto"))?;
        if s.file_id.is_empty() {
            s.file_id = find_by_name(&s.name, &self.target, self.state_dir.as_deref())?;
        }
        Ok(StreamSummary {
            stream,
            name: s.name.clone(),
            file_id: s.file_id.clone(),
            bytes: s.written,
            chunks: s.chunks,
        })
    }

    fn finish(&self) -> Result<UploadSummary, String> {
        let st = self.streams.lock().map_err(|_| "estado envenenado")?;
        let bytes: u64 = st.values().map(|s| s.written).sum();
        Ok(UploadSummary {
            bytes,
            chunks: st.values().map(|s| s.chunks).sum(),
            streams: st.len(),
            file_ids: st.values().map(|s| s.file_id.clone()).collect(),
            names: st.values().map(|s| s.name.clone()).collect(),
            sha256: String::new(),
            label: self.label.clone(),
        })
    }

    fn already_done(&self, stream: usize, chunk: &Chunk) -> bool {
        let st = match self.streams.lock() {
            Ok(s) => s,
            Err(_) => return false,
        };
        st.get(&stream)
            .map(|s| s.done.contains_key(&chunk.index))
            .unwrap_or(false)
    }
}

/// Cuántos bytes ha recibido ya la sesión (para reanudar).
///
/// Medido: `PUT` con `Content-Range: bytes */TOTAL` y cuerpo vacío ⇒ **308**
/// + cabecera `Range: bytes=0-N`.
pub fn resume_offset(session: &str, total: u64, state_dir: Option<&str>) -> Option<u64> {
    let (ak, av) = google_auth::auth_header(state_dir).ok()?;
    let cfg = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(60)))
        .timeout_connect(Some(Duration::from_secs(15)))
        .http_status_as_error(false)
        .build();
    let agent = ureq::Agent::new_with_config(cfg);
    let resp = agent
        .put(session)
        .header(ak, av)
        .header("Content-Range", format!("bytes */{total}"))
        .header("Content-Length", "0")
        .send(&[] as &[u8])
        .ok()?;
    let range = resp
        .headers()
        .get("range")
        .and_then(|v| v.to_str().ok())?
        .to_string();
    parse_range_end(&range)
}

/// `bytes=0-524287` ⇒ `Some(524288)`.
pub fn parse_range_end(range: &str) -> Option<u64> {
    let r = range.trim().trim_start_matches("bytes=");
    let (_, end) = r.split_once('-')?;
    end.trim().parse::<u64>().ok().map(|v| v + 1)
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
    let resp = DriveSink::agent()
        .get(&url)
        .header(ak, av)
        .call()
        .map_err(|e| format!("buscar en Drive: {e}"))?;
    let st = resp.status().as_u16();
    let body = resp.into_body().read_to_vec().unwrap_or_default();
    if st != 200 {
        return Err(format!("buscar en Drive devolvió {st}"));
    }
    let j: Value = serde_json::from_slice(&body).map_err(|e| format!("json de Drive: {e}"))?;
    j.get("files")
        .and_then(|f| f.as_array())
        .and_then(|a| a.first())
        .and_then(|f| f.get("id"))
        .and_then(|i| i.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("no se encontró el fichero '{name}' en Drive"))
}

// ---------------------------------------------------------------------------
// Utilidades
// ---------------------------------------------------------------------------

/// Hexadecimal en minúsculas.
pub fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Epoch en segundos.
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Informe JSON de una subida (para el MCP y el bus).
pub fn summary_json(s: &UploadSummary) -> Value {
    json!({
        "ok": true,
        "label": s.label,
        "bytes": s.bytes,
        "chunks": s.chunks,
        "streams": s.streams,
        "sha256": s.sha256,
        "file_ids": s.file_ids,
        "names": s.names,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_reparto_de_trozos_es_contiguo_y_alinhado() {
        let sink = Arc::new(FileSink::new(Path::new("/tmp/md-stream-test"), "x").unwrap());
        let p = Pipeline::new(sink, "x", 100 * 1024, 256 * 1024, 3, BTreeMap::new()).unwrap();
        let plan = p.plan.clone();
        // 100 KiB con trozos de 256 KiB = 1 solo trozo -> una conexión con
        // el trozo y las demás con cero.
        assert_eq!(plan.len(), 3);
        assert_eq!(plan[0].1, 0);
        assert_eq!(plan[0].2, 1);
        assert_eq!(plan[1].2, 0);
    }

    #[test]
    fn el_parseo_del_rango_da_el_offset_resumible() {
        assert_eq!(parse_range_end("bytes=0-524287"), Some(524288));
        assert_eq!(parse_range_end("bytes 0-1023"), Some(1024));
        assert_eq!(parse_range_end("bytes=0-"), None);
    }

    #[test]
    fn el_tamano_de_trozo_ilegal_se_rechaza() {
        let sink = Arc::new(FileSink::new(Path::new("/tmp/md-stream-test"), "y").unwrap());
        let e = match Pipeline::new(sink, "y", 1024, 1000, 1, BTreeMap::new()) {
            Ok(_) => panic!("un trozo de 1000 B tiene que ser rechazado"),
            Err(e) => e,
        };
        assert!(e.contains("múltiplo"));
    }

    #[test]
    fn el_sha256_de_un_trozo_es_el_real() {
        let d = std::env::temp_dir().join(format!("md-stream-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let sink = Arc::new(FileSink::new(&d, "z").unwrap());
        let mut p = Pipeline::new(sink, "z", 10, 4, 1, BTreeMap::new()).unwrap();
        p.push(b"abcd".to_vec()).unwrap();
        p.push(b"efgh".to_vec()).unwrap();
        p.push(b"ij".to_vec()).unwrap();
        let s = p.finish().unwrap();
        assert_eq!(s.bytes, 10);
        assert_eq!(s.chunks, 3);
        // El hash global es el del flujo entero.
        assert_eq!(s.sha256, hex(&Sha256::digest(b"abcdefghij")));
        let _ = std::fs::remove_dir_all(&d);
    }
}
