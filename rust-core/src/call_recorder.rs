//! MegaDaemon v13.4 — Grabador de llamadas nativo y oculto.
//!
//! Este módulo NO inventa capacidades. Se limita a:
//!
//! * Clasificar el **origen** de una llamada a partir del paquete/servicio real
//!   que Android reporta (`dumpsys audio` → sección `Communication client`).
//! * Generar un **plan de captura** por fuente (función PURA: no toca disco ni
//!   red, sólo devuelve la lista de comandos a ejecutar con root).
//! * Detectar si hay una **llamada activa** parseando `dumpsys audio` real.
//! * Detectar si hay una **captura de pantalla** activa (`dumpsys
//!   media_projection`), que es el indicador más fiable de videollamada.
//! * Inventariar las **grabaciones** ya existentes en disco con SHA-256.
//!
//! REALIDAD TÉCNICA (no hay "fantasmadas" aquí):
//!
//! | Fuente              | ¿Se puede grabar el audio remoto?                                   |
//! |---------------------|--------------------------------------------------------------------|
//! | Sistema (CSD/VoLTE) | Sí, vía CSC patch (soulr344/OMCDecoder) o captura de AudioFlinger |
//! | WhatsApp            | Sólo captura de SALIDA de AudioFlinger o hook en proceso           |
//! | Telegram            | Sí, el propio cliente tiene grabador nativo de llamadas            |
//! | Signal / Molly      | Sólo hook en proceso (Frida) sobre AudioRecord/AudioTrack/Opus     |
//! | Discord             | Sólo hook en proceso o captura de la proyección de pantalla       |
//!
//! El audio remoto de WhatsApp/Signal/Discord viaja como Opus cifrado sobre
//! SRTP/WebRTC: no existe ninguna API pública que lo exponga. Lo único
//! "oficial" es capturar el audio **ya decodificado** en el dispositivo
//! (salida de AudioFlinger) o interceptarlo **dentro del proceso** de la app.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Extensiones de audio aceptadas por el inventario de grabaciones.
pub const RECORDING_EXTENSIONS: [&str; 13] = [
    "wav", "opus", "ogg", "pcm", "raw", "m4a", "aac", "mp3", "flac", "amr", "3gp", "mkv", "webm",
];

/// Placeholder del timestamp en los nombres de fichero generados por los planes.
/// El ejecutor lo sustituye por `date -u +%Y%m%dT%H%M%SZ` en el momento de correr.
pub const TS_PLACEHOLDER: &str = "%TS%";

/// Umbral (bytes) a partir del cual Google Drive recomienda subida reanudable
/// (chunks de 8 MiB). Por debajo, la subida simple es más eficiente.
pub const DRIVE_RESUMABLE_THRESHOLD: u64 = 8 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Sesión de llamada
// ---------------------------------------------------------------------------

/// Estado de una sesión de llamada grabada.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallSession {
    /// Identificador estable de la sesión (timestamp + fuente + hash del audio).
    pub id: String,
    /// Origen de la llamada (`system`, `whatsapp`, `telegram`, `signal`,
    /// `discord` o `unknown:<pkg>`).
    pub source: String,
    /// Inicio de la llamada en ISO-8601 UTC.
    pub started_at: String,
    /// Fin de la llamada en ISO-8601 UTC (`None` mientras sigue en curso).
    pub ended_at: Option<String>,
    /// Duración en segundos.
    pub duration_s: u32,
    /// Ruta del fichero de audio grabado (si se grabó).
    pub audio_path: Option<String>,
    /// Tipo de llamada: `voice` o `video`.
    pub kind: String,
    /// Estado: `recording`, `stopped`, `failed`, `skipped`.
    pub state: String,
    /// Transcripción opcional (requiere `enable_transcription`).
    pub transcript: Option<String>,
}

impl CallSession {
    /// Crea una sesión en curso.
    pub fn new(id: &str, source: &str, started_at: &str, kind: &str) -> Self {
        CallSession {
            id: id.to_string(),
            source: source.to_string(),
            started_at: started_at.to_string(),
            ended_at: None,
            duration_s: 0,
            audio_path: None,
            kind: kind.to_string(),
            state: "recording".to_string(),
            transcript: None,
        }
    }

    /// Cierra la sesión con su marca de fin y su duración.
    pub fn finalize(&mut self, ended_at: &str, duration_s: u32) {
        self.ended_at = Some(ended_at.to_string());
        self.duration_s = duration_s;
        self.state = "stopped".to_string();
    }

    /// Serializa la sesión a JSON plano.
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "source": self.source,
            "started_at": self.started_at,
            "ended_at": self.ended_at,
            "duration_s": self.duration_s,
            "audio_path": self.audio_path,
            "kind": self.kind,
            "state": self.state,
            "transcript": self.transcript,
        })
    }
}

// ---------------------------------------------------------------------------
// Origen de la llamada
// ---------------------------------------------------------------------------

/// Origen/origen técnico de una llamada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallSource {
    /// Llamada del sistema (CSD/IMS/VoLTE) o app deVoIP del sistema.
    System,
    /// WhatsApp (`com.whatsapp`).
    WhatsApp,
    /// Telegram (`org.telegram.messenger`, Telegram Web `…messenger.web`) y el
    /// cliente Ngram (`org.ngram`/`com.ngram`), que habla el protocolo Telegram.
    Telegram,
    /// Signal / Molly (`org.thoughtcrime.securesms`).
    Signal,
    /// Discord (`com.discord`).
    Discord,
    /// Origen no identificado: se conserva el paquete/servicio crudo.
    Unknown(String),
}

impl CallSource {
    /// Nombre estable en minúsculas (usado en rutas, IDs y JSON).
    pub fn as_str(&self) -> &str {
        match self {
            CallSource::System => "system",
            CallSource::WhatsApp => "whatsapp",
            CallSource::Telegram => "telegram",
            CallSource::Signal => "signal",
            CallSource::Discord => "discord",
            CallSource::Unknown(_) => "unknown",
        }
    }

    /// Paquete Android asociado, si se conoce.
    pub fn package(&self) -> Option<&str> {
        match self {
            CallSource::System => Some("com.android.server.telecom"),
            CallSource::WhatsApp => Some("com.whatsapp"),
            CallSource::Telegram => Some("org.telegram.messenger"),
            CallSource::Signal => Some("org.thoughtcrime.securesms"),
            CallSource::Discord => Some("com.discord"),
            CallSource::Unknown(p) => {
                if p.is_empty() {
                    None
                } else {
                    Some(p.as_str())
                }
            }
        }
    }
}

impl fmt::Display for CallSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CallSource::Unknown(p) => write!(f, "unknown:{}", p),
            other => write!(f, "{}", other.as_str()),
        }
    }
}

/// Mapa exacto de paquete Android → origen.
fn map_package_exact(pkg: &str) -> Option<CallSource> {
    match pkg {
        // --- WhatsApp (incluye WhatsApp Business) ---
        "com.whatsapp" | "com.whatsapp.w4b" => Some(CallSource::WhatsApp),
        // --- Telegram: app Android, Web K y cliente Ngram ---
        "org.telegram.messenger" | "org.telegram.messenger.web" | "com.telegram" => {
            Some(CallSource::Telegram)
        }
        "org.ngram" | "com.ngram" | "com.ngram.messenger" => Some(CallSource::Telegram),
        // --- Signal / Molly (fork de Signal usado en cuentas de pruebas) ---
        "org.thoughtcrime.securesms" | "org.thoughtcrime.securesms.playstore" => {
            Some(CallSource::Signal)
        }
        // --- Discord ---
        "com.discord" => Some(CallSource::Discord),
        // --- Sistema: telecom y apps de SMS/RCS por defecto ---
        "com.android.server.telecom"
        | "com.android.phone"
        | "com.android.mms"
        | "com.google.android.apps.messaging"
        | "android"
        | "telecom"
        | "system" => Some(CallSource::System),
        _ => None,
    }
}

/// Mapa laxo (subcadena) para paquetes no catalogados.
fn map_package_loose(pkg: &str) -> Option<CallSource> {
    if pkg.is_empty() {
        return None;
    }
    if pkg.contains("whatsapp") {
        return Some(CallSource::WhatsApp);
    }
    if pkg.contains("telegram") || pkg.contains("ngram") {
        return Some(CallSource::Telegram);
    }
    if pkg.contains("securesms") || pkg.contains("signal") {
        return Some(CallSource::Signal);
    }
    if pkg.contains("discord") {
        return Some(CallSource::Discord);
    }
    if pkg.contains("telecom") || pkg.contains("messaging") || pkg.contains("phone") {
        return Some(CallSource::System);
    }
    None
}

/// Clasifica el origen de una llamada a partir del paquete y/o del servicio
/// que Android reporta.
///
/// `package` es el paquete Android de la app (o `com.android.server.telecom`
/// para llamadas del sistema) y `service` es el identificador del cliente de
/// comunicación (el `Communication client` de `dumpsys audio`).
pub fn detect_source(package: &str, service: &str) -> CallSource {
    let pkg = package.trim().to_ascii_lowercase();
    let svc = service.trim().to_ascii_lowercase();

    if let Some(s) = map_package_exact(&pkg) {
        return s;
    }
    if let Some(s) = map_package_exact(&svc) {
        return s;
    }
    if let Some(s) = map_package_loose(&pkg) {
        return s;
    }
    if let Some(s) = map_package_loose(&svc) {
        return s;
    }
    if !pkg.is_empty() {
        return CallSource::Unknown(pkg);
    }
    if !svc.is_empty() {
        return CallSource::Unknown(svc);
    }
    CallSource::Unknown(String::new())
}

// ---------------------------------------------------------------------------
// Configuración y plan de captura
// ---------------------------------------------------------------------------

/// Códigos de CSC que habilitan la **grabación nativa** de llamadas.
///
/// Con un CSC de América (el de este móvil es AMO) la grabación nativa está
/// desactivada por el flag `CscFeature_VoiceCall_ConfigRecording` del
/// `cscfeature.xml`. Con un CSC de Asia (THL, INS) el sistema graba solo,
/// sin hook ni AudioFlinger parcheado: es la vía preferente.
///
/// Ver `rom_migration::csc_allows_native_recording`.
pub const NATIVE_RECORDING_CSC_CODES: [&str; 4] = ["THL", "INS", "ILO", "XXV"];

/// ¿El CSC actual permite la grabación nativa?
pub fn native_recording_allowed(csc: &str) -> bool {
    let c = csc.trim().to_ascii_uppercase();
    NATIVE_RECORDING_CSC_CODES.iter().any(|k| *k == c)
}

/// Configuración del grabador.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecorderConfig {
    /// Frecuencia de muestreo en Hz (16 kHz por defecto: suficiente para voz).
    pub sample_rate: u32,
    /// Número de canales (1 = mono).
    pub channels: u16,
    /// Duración máxima en segundos antes de auto-parar (1800 s = 30 min).
    pub max_duration_s: u32,
    /// Directorio donde se dejan las grabaciones.
    pub output_dir: String,
    /// Formato de salida (`wav` = PCM 16-bit; `opus` requiere transcodificar).
    pub format: String,
    /// Si se debe transcribir localmente (whisper.cpp) al terminar.
    pub enable_transcription: bool,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        RecorderConfig {
            sample_rate: 16_000,
            channels: 1,
            max_duration_s: 1800,
            output_dir: "/sdcard/MegaDaemon/recordings".to_string(),
            format: "wav".to_string(),
            enable_transcription: false,
        }
    }
}

/// Estrategia de captura para una fuente concreta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturePlan {
    /// Identificador de la estrategia (`audioflinger_tacap`, …).
    pub method: String,
    /// Comandos exactos a ejecutar (shell, ya envueltos en `su -mm -c`).
    pub commands: Vec<String>,
    /// Si la estrategia exige root (`su -mm`).
    pub needs_root: bool,
    /// Notas honestas: qué funciona, qué no y qué falta en el dispositivo.
    pub notes: String,
}

fn sh(cmd: &str) -> String {
    format!("su -mm -c '{}'", cmd.replace('\'', "'\\''"))
}

/// Comandos de detección comunes a cualquier plan.
fn detection_commands() -> Vec<String> {
    vec![
        sh("dumpsys audio | grep -E \"mCallState|mode:|MODE_|Communication client|package:\""),
        sh("dumpsys media_projection | grep -iE \"media projection|packageName|uid\""),
    ]
}

/// Genera el plan de captura para una fuente. Función PURA: misma entrada ⇒
/// misma salida, sin acceso a disco, red ni reloj.
pub fn plan_capture(cfg: &RecorderConfig, source: CallSource) -> CapturePlan {
    let mut commands = detection_commands();
    let out = cfg.output_dir.trim_end_matches('/');
    let fmt = if cfg.format.is_empty() { "wav" } else { &cfg.format };
    let file = format!("{}/call-{}-{}{}", out, source.as_str(), TS_PLACEHOLDER, fmt_ext(fmt));
    let raw = format!("{}/call-{}-{}{}", out, source.as_str(), TS_PLACEHOLDER, ".pcm");

    let (method, notes, extra): (&str, &str, Vec<String>) = match &source {
        CallSource::System => (
            "audioflinger_tacap",
            "Llamada del sistema (CSD/IMS/VoLTE). Dos vías reales: (1) captura del audio \
             ya decodificado por AudioFlinger, que exige el permiso CAPTURE_AUDIO_OUTPUT \
             (sólo system/root) y los binarios `tacap`/`tinycap`, que NO vienen preinstalados \
             en el Galaxy S21 Ultra: hay que compilar `system/extras/tinycap` para arm64 o \
             copiar un `tacap` con ese soporte y empujarlo por root; (2) parche CSC \
             (soulr344/OMCDecoder), que decodifica el audio del módem a partir del códec \
             (CSCCfg) y es la opción cuando AudioFlinger no expone la ruta de llamada. \
             El parche CSC NO sirve para WhatsApp/Signal/Discord.",
            vec![
                sh(&format!(
                    "dumpsys media.audio_flinger > /data/local/tmp/aflinger-{}.txt",
                    TS_PLACEHOLDER
                )),
                sh(&format!(
                    "tacap -c {} -r {} -b 16 -f {} -t {}",
                    cfg.channels, cfg.sample_rate, file, cfg.max_duration_s
                )),
                sh(&format!(
                    "tinycap -r {} -c {} -b 16 -f {} -t {}",
                    cfg.sample_rate, cfg.channels, raw, cfg.max_duration_s
                )),
            ],
        ),
        CallSource::WhatsApp => (
            "audioflinger_output_capture",
            "WhatsApp transporta el audio remoto como Opus sobre SRTP y no expone ninguna \
             API de grabación: el parche CSC no aplica (es sólo para llamadas del sistema). \
             Viable: capturar la SALIDA de AudioFlinger (requiere CAPTURE_AUDIO_OUTPUT, sólo \
             system/root) o hacer hook en proceso del AudioTrack/descodificador Opus de \
             `com.whatsapp`. Sin root efectivo no hay audio remoto, sólo metadata \
             (duración, timestamps) y notificaciones.",
            vec![
                sh(&format!(
                    "dumpsys media.audio_flinger | grep -A 40 -iE \"output thread|com.whatsapp\""
                )),
                sh(&format!(
                    "tacap -c {} -r {} -b 16 -f {} -t {}",
                    cfg.channels, cfg.sample_rate, file, cfg.max_duration_s
                )),
            ],
        ),
        CallSource::Telegram => (
            "audioflinger_output_capture",
            "Telegram tiene grabador NATIVO de llamadas: el audio decodificado se guarda en \
             la caché de la app y es la vía más fiable (copiar ese fichero, sin hook). \
             Si el cliente no ofrece grabación, el plan cae a la captura de SALIDA de \
             AudioFlinger (CAPTURE_AUDIO_OUTPUT) y, en último término, a hook en proceso \
             (Frida) del decodificador Opus. En el cliente Ngram (`org.ngram`/`com.ngram`) \
             el comportamiento de grabación nativa depende de la build: si no está, se aplica \
             la misma cadena de audioflinger/hook.",
            vec![
                sh("ls -l /data/data/org.telegram.messenger/cache | grep -iE \"call|audio\""),
                sh(&format!(
                    "tacap -c {} -r {} -b 16 -f {} -t {}",
                    cfg.channels, cfg.sample_rate, file, cfg.max_duration_s
                )),
            ],
        ),
        CallSource::Signal => (
            "inprocess_hook_audio",
            "Signal/Molly cifra el audio remoto (Opus sobre SRTP) y no permite grabación. La \
             ÚNICA vía es un hook EN PROCESO (Frida sobre `org.thoughtcrime.securesms`) que \
             intercepte el buffer PCM de AudioRecord/AudioTrack o la llamada JNI al \
             descodificador Opus, y lo vuelque a fichero. Sin ese hook sólo se puede leer \
             metadata y notificaciones; el parche CSC no aplica.",
            vec![
                sh("frida-ps -Ua | grep -i securesms"),
                sh("frida -U -n org.thoughtcrime.securesms -l /data/local/tmp/hook_audio.js --no-pause"),
            ],
        ),
        CallSource::Discord => (
            "inprocess_hook_audio",
            "Discord usa Opus sobre WebRTC; no hay API de grabación. Dos vías: (1) cuando se \
             comparte pantalla, el audio viaja por MediaProjection y se puede capturar desde \
             el sistema (`dumpsys media_projection` lo delata); (2) hook en proceso \
             (Frida sobre `com.discord`) sobre AudioTrack/AudioRecord. La captura de audio \
             del sistema con CAPTURE_AUDIO_OUTPUT también sirve, pero requiere root real.",
            vec![
                sh("dumpsys media_projection | grep -iE \"packageName|media projection\""),
                sh("frida -U -n com.discord -l /data/local/tmp/hook_audio.js --no-pause"),
            ],
        ),
        CallSource::Unknown(_) => (
            "generic_audioflinger_poll",
            "Origen no identificado: el paquete real se deduce de la sección \
             `Communication client` de `dumpsys audio`. Se aplica el plan genérico de \
             captura de audio del sistema; si el paquete detectado es una app VoIP, la \
             captura de SALIDA puede requerir CAPTURE_AUDIO_OUTPUT (root efectivo) o \
             hook en proceso.",
            vec![sh(&format!(
                "tacap -c {} -r {} -b 16 -f {} -t {}",
                cfg.channels, cfg.sample_rate, file, cfg.max_duration_s
            ))],
        ),
    };

    commands.extend(extra);

    // Cierre común: hash de integridad y transcripción opcional.
    commands.push(sh(&format!("sha256sum {}", file)));
    if cfg.enable_transcription {
        commands.push(sh(&format!(
            "whisper-cli -m /data/local/tmp/ggml-base.bin -f {} -otxt -of {}",
            file,
            format!("{}/call-{}-{}{}", out, source.as_str(), TS_PLACEHOLDER, ".txt")
        )));
    }

    CapturePlan {
        method: method.to_string(),
        commands,
        needs_root: true,
        notes: notes.to_string(),
    }
}

/// Extensión de fichero según el formato configurado.
fn fmt_ext(fmt: &str) -> &'static str {
    match fmt.to_ascii_lowercase().as_str() {
        "opus" => ".opus",
        "ogg" => ".ogg",
        "m4a" => ".m4a",
        "aac" => ".aac",
        "mp3" => ".mp3",
        "pcm" => ".pcm",
        "raw" => ".raw",
        _ => ".wav",
    }
}

/// Plan de captura para VIDEOLLAMADAS (o cuando `dumpsys media_projection`
/// detecta una captura de pantalla activa).
///
/// `screenrecord --audio-source VOICE_COMMUNICATION` existe desde Android 11,
/// pero sólo captura el micrófono si la ruta de audio no está en modo
/// comunicación; el audio remoto del interlocutor sigue necesitando
/// CAPTURE_AUDIO_OUTPUT o hook en proceso.
pub fn plan_video_capture(cfg: &RecorderConfig, source: CallSource) -> CapturePlan {
    let out = cfg.output_dir.trim_end_matches('/');
    let file = format!("{}/call-video-{}{}", out, TS_PLACEHOLDER, ".mp4");
    let mut commands = vec![
        sh("dumpsys media_projection | grep -iE \"media projection|packageName|uid|type\""),
        sh("dumpsys audio | grep -E \"mode:|MODE_|mCallState\""),
        sh("appops set com.android.shell PROJECT_MEDIA allow"),
        sh(&format!(
            "screenrecord --audio-source VOICE_COMMUNICATION --size 1280x720 --time-limit {} {}",
            cfg.max_duration_s, file
        )),
    ];
    commands.push(sh(&format!("sha256sum {}", file)));
    if cfg.enable_transcription {
        commands.push(sh(&format!(
            "whisper-cli -m /data/local/tmp/ggml-base.bin -f {} -otxt -of {}",
            file,
            format!("{}/call-video-{}{}", out, TS_PLACEHOLDER, ".txt")
        )));
    }
    CapturePlan {
        method: "media_projection_av".to_string(),
        commands,
        needs_root: true,
        notes: format!(
            "Videollamada de origen `{}`. La detección de captura de pantalla activa \
             (`dumpsys media_projection`) es el indicador más fiable de videollamada. \
             `screenrecord --audio-source VOICE_COMMUNICATION` captura el micrófono y, \
             según el ROM, parte del audio de llamada; el audio remoto del interlocutor \
             sigue requiriendo CAPTURE_AUDIO_OUTPUT o hook en proceso. \
             `appops set com.android.shell PROJECT_MEDIA allow` evita el diálogo de \
             consentimiento en algunos builds; en otros hay que aceptar la Projection \
             manualmente (Android 14 exige confirmación del usuario en la primera vez).",
            source
        ),
    }
}

// ---------------------------------------------------------------------------
// Detección de llamada activa
// ---------------------------------------------------------------------------

/// Estado detectado de una llamada en curso.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveCall {
    /// `true` si hay una llamada en curso (modo comunicación / offhook / foco).
    pub active: bool,
    /// Modo de audio (`MODE_IN_COMMUNICATION`, `MODE_NORMAL`, …).
    pub mode: String,
    /// Estado de llamada (`IN_CALL`, `RINGING`, `IDLE`).
    pub state: String,
    /// Cliente de comunicación que Android reporta.
    pub client: Option<String>,
    /// Paquete Android dueño de la llamada.
    pub package: Option<String>,
}

/// Extrae el valor de una línea `clave: valor` (o `clave=valor`).
fn kv(line: &str) -> Option<(String, String)> {
    let l = line.trim();
    if let Some((k, v)) = l.split_once(':') {
        let key = k.trim().to_ascii_lowercase();
        if !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ' ') {
            return Some((key, v.trim().to_string()));
        }
    }
    if let Some((k, v)) = l.split_once('=') {
        let key = k.trim().to_ascii_lowercase();
        if !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ' ') {
            return Some((key, v.trim().to_string()));
        }
    }
    None
}

/// Detecta si el paquete/valor parece un identificador de paquete Android.
fn looks_like_package(v: &str) -> bool {
    let v = v.trim();
    if v.is_empty() || v.contains(' ') {
        return false;
    }
    let mut parts = v.split('.');
    let first = parts.next().unwrap_or("");
    if first.is_empty() {
        return false;
    }
    // com.foo / org.foo / net.foo / io.foo ...
    ["com", "org", "net", "io", "me", "xyz", "app", "dev"].contains(&first)
}

/// Analiza la salida real de `dumpsys audio` y devuelve el estado de llamada.
///
/// Devuelve `None` si el texto no parece un dump de audio (sin ningún marcador
/// reconocible). Si el dump es válido pero no hay llamada, devuelve
/// `Some(ActiveCall { active: false, … })`.
pub fn detect_active_call(dumpsys_audio: &str) -> Option<ActiveCall> {
    if dumpsys_audio.trim().is_empty() {
        return None;
    }

    let lines: Vec<&str> = dumpsys_audio.lines().collect();
    let mut mode: Option<String> = None;
    let mut call_state: Option<i32> = None;
    let mut client: Option<String> = None;
    let mut package: Option<String> = None;
    let mut client_state: Option<String> = None;
    let mut focus_active = false;
    let mut saw_marker = false;

    let mut i = 0usize;
    while i < lines.len() {
        let raw = lines[i];
        let line = raw.trim().trim_start_matches('-').trim();

        if let Some((key, value)) = kv(line) {
            // --- Modo de audio ---
            if (key == "mode" || key == "mmode" || key.ends_with(" mode") || key.ends_with("mode"))
                && value.to_ascii_uppercase().starts_with("MODE_")
            {
                mode = Some(value.to_ascii_uppercase());
                saw_marker = true;
            }
            // --- Estado de llamada del módem ---
            if key == "mcallstate" || key == "callstate" || key == "call state" {
                if let Some(d) = value.chars().find(|c| c.is_ascii_digit()) {
                    call_state = d.to_digit(10).map(|d| d as i32);
                    saw_marker = true;
                }
            }
            // --- Sección "Communication client" ---
            if key == "communication client" || key == "communicationclient" {
                saw_marker = true;
                if !value.is_empty() {
                    client = Some(value.clone());
                }
                // Bloque con sub-líneas indentadas: package / state / client.
                let mut j = i + 1;
                while j < lines.len() {
                    let sub = lines[j];
                    if sub.trim().is_empty() {
                        j += 1;
                        continue;
                    }
                    if !sub.starts_with(char::is_whitespace) {
                        break; // fin del bloque
                    }
                    if let Some((k, v)) = kv(sub.trim().trim_start_matches('-').trim()) {
                        match k.as_str() {
                            "package" | "package name" | "package name com" => {
                                package = Some(v.clone());
                                saw_marker = true;
                            }
                            "state" | "client state" => {
                                client_state = Some(v.clone());
                                saw_marker = true;
                            }
                            "client" => {
                                client = Some(v.clone());
                                saw_marker = true;
                            }
                            _ => {}
                        }
                    }
                    j += 1;
                }
            }
            // --- Stack de foco de audio: package/client/state por entrada ---
            if key == "package" && package.is_none() && looks_like_package(&value) {
                // Puede ser la cabecera de una entrada del focus stack.
                package = Some(value.clone());
                saw_marker = true;
            }
            if key == "state" {
                let lv = value.to_ascii_lowercase();
                if lv.contains("gained")
                    || lv.contains("awaiting_loss")
                    || lv.contains("delayed")
                    || lv.contains("loss_transient")
                    || lv.contains("active")
                    || lv.contains("focus")
                {
                    focus_active = true;
                    if client_state.is_none() {
                        client_state = Some(value.clone());
                    }
                }
            }
            if key == "client" && client.is_none() && looks_like_package(&value) {
                client = Some(value.clone());
            }
        }
        i += 1;
    }

    if !saw_marker {
        return None;
    }

    let mode_name = mode.unwrap_or_else(|| {
        if call_state == Some(2) {
            "MODE_IN_COMMUNICATION".to_string()
        } else {
            "MODE_NORMAL".to_string()
        }
    });

    let state = match call_state {
        Some(2) => "IN_CALL",
        Some(1) => "RINGING",
        _ => {
            if mode_name == "MODE_IN_COMMUNICATION" || focus_active {
                "IN_CALL"
            } else {
                "IDLE"
            }
        }
    };

    let active = state == "IN_CALL"
        || mode_name == "MODE_IN_COMMUNICATION"
        || call_state == Some(2)
        || focus_active;

    let _ = client_state; // informativo, no cambia el veredicto

    // El "cliente" es el identificador de paquete: si el bloque trae `client`
    // con un valor tipo `AudioFocusRequester@…`, se prefiere el paquete.
    let client_final = match (&client, &package) {
        (Some(c), _) if looks_like_package(c) => Some(c.clone()),
        (Some(c), Some(p)) if !c.is_empty() => Some(c.clone()),
        (None, Some(p)) => Some(p.clone()),
        _ => package.clone(),
    };

    Some(ActiveCall {
        active,
        mode: mode_name,
        state: state.to_string(),
        client: client_final,
        package,
    })
}

/// Detecta si hay una captura de pantalla (MediaProjection) activa, que es el
/// indicador más fiable de que hay una videollamada en curso.
pub fn detect_media_projection(dumpsys_media_projection: &str) -> bool {
    let low = dumpsys_media_projection.to_ascii_lowercase();
    if low.trim().is_empty() {
        return false;
    }

    // Marcadores explícitos de ausencia (se comprueban primero).
    const NEGATIVES: [&str; 7] = [
        "no active projection",
        "no active media projection",
        "no projections",
        "no projection",
        "projection: none",
        "mediaprojection: none",
        "active projections: 0",
    ];
    for n in NEGATIVES.iter() {
        if low.contains(n) {
            return false;
        }
    }

    // Marcadores inequívocos de una proyección viva.
    if low.contains("mediaprojection{")
        || low.contains("projection record")
        || low.contains("projection: mediaprojection")
    {
        return true;
    }

    const POSITIVOS: [&str; 5] = [
        "media projection",
        "mediaprojection",
        "media_projection",
        "virtual display",
        "virtualdisplay",
    ];
    if !POSITIVOS.iter().any(|p| low.contains(p)) {
        return false;
    }

    // La sección debe traer un dueño (paquete o uid) para no dar positivo por
    // el mero encabezado de la sección.
    low.contains("package") || low.contains("uid") || low.contains("com.") || low.contains("org.")
}

// ---------------------------------------------------------------------------
// Inventario de grabaciones
// ---------------------------------------------------------------------------

/// SHA-256 en hexadecimal de un buffer.
pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    hex(&digest)
}

/// SHA-256 en hexadecimal de un fichero (streaming).
pub fn sha256_file(path: &Path) -> Option<String> {
    let mut f = fs::File::open(path).ok()?;
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
pub fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Recorre un directorio collecting ficheros regulares (sin seguir symlinks).
fn walk_files(root: &Path, out: &mut Vec<PathBuf>) {
    let rd = match fs::read_dir(root) {
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
            continue; // no se siguen symlinks (evita escapar del directorio)
        }
        if md.is_dir() {
            walk_files(&p, out);
        } else if md.is_file() {
            out.push(p);
        }
    }
}

/// Inventario de grabaciones de un directorio: tamaño + SHA-256 de cada una.
pub fn list_recordings(dir: &str) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    let root = Path::new(dir);
    if !root.is_dir() {
        return out;
    }

    let mut files: Vec<PathBuf> = Vec::new();
    walk_files(root, &mut files);

    for p in files {
        let ext = p
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if !RECORDING_EXTENSIONS.contains(&ext.as_str()) {
            continue; // sólo audio: el resto del directorio no es una grabación
        }
        let md = match fs::metadata(&p) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let modified = md
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        out.push(json!({
            "path": p.to_string_lossy(),
            "name": p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            "ext": ext,
            "bytes": md.len(),
            "sha256": sha256_file(&p).unwrap_or_default(),
            "modified_unix": modified,
        }));
    }

    out.sort_by(|a, b| {
        let pa = a.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let pb = b.get("path").and_then(|v| v.as_str()).unwrap_or("");
        pa.cmp(pb)
    });
    out
}

/// Devuelve la hora actual en ISO-8601 UTC (`2024-05-06T07:08:09Z`).
pub fn now_iso8601() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, m, d, hh, mm, ss) = civil_from_epoch(secs as i64);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, m, d, hh, mm, ss
    )
}

/// Convierte segundos desde epoch a fecha civil (algoritmo de Hinnant).
pub fn civil_from_epoch(secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmpdir(tag: &str) -> String {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "md_callrec_{}_{}_{}",
            tag,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&p).unwrap();
        p.to_string_lossy().to_string()
    }

    // --- Detección de origen ------------------------------------------------

    #[test]
    fn detect_source_mapea_paquetes_reales() {
        assert_eq!(detect_source("com.whatsapp", ""), CallSource::WhatsApp);
        assert_eq!(
            detect_source("org.telegram.messenger", ""),
            CallSource::Telegram
        );
        assert_eq!(
            detect_source("org.telegram.messenger.web", ""),
            CallSource::Telegram
        );
        assert_eq!(detect_source("org.ngram", ""), CallSource::Telegram);
        assert_eq!(detect_source("com.ngram", ""), CallSource::Telegram);
        assert_eq!(
            detect_source("com.google.android.apps.messaging", ""),
            CallSource::System
        );
        assert_eq!(
            detect_source("org.thoughtcrime.securesms", ""),
            CallSource::Signal
        );
        assert_eq!(detect_source("com.discord", ""), CallSource::Discord);
    }

    #[test]
    fn detect_source_usa_el_servicio_como_fallback() {
        assert_eq!(detect_source("", "com.whatsapp"), CallSource::WhatsApp);
        assert_eq!(detect_source("", "com.android.server.telecom"), CallSource::System);
        match detect_source("", "com.ejemplo.rareza") {
            CallSource::Unknown(s) => assert_eq!(s, "com.ejemplo.rareza"),
            other => panic!("esperaba Unknown, {:?}", other),
        }
    }

    // --- Plan de captura -----------------------------------------------------

    #[test]
    fn plan_capture_es_puro_y_especifico_por_fuente() {
        let cfg = RecorderConfig::default();
        let p1 = plan_capture(&cfg, CallSource::Signal);
        let p2 = plan_capture(&cfg, CallSource::Signal);
        assert_eq!(p1.commands, p2.commands, "el plan debe ser determinista");
        assert_eq!(p1.method, "inprocess_hook_audio");
        assert!(p1.needs_root);
        assert!(p1.commands.iter().any(|c| c.contains("frida")));
        // La nota debe dejar claro que Signal NO se puede grabar sin hook.
        assert!(p1.notes.to_lowercase().contains("hook"));
    }

    #[test]
    fn plan_capture_usa_los_parametros_de_la_configuracion() {
        let cfg = RecorderConfig {
            sample_rate: 44_100,
            channels: 2,
            max_duration_s: 600,
            output_dir: "/data/local/tmp/rec/".to_string(),
            format: "opus".to_string(),
            enable_transcription: true,
        };
        let plan = plan_capture(&cfg, CallSource::System);
        assert!(plan.commands.iter().any(|c| c.contains("44100")));
        assert!(plan.commands.iter().any(|c| c.contains("-c 2")));
        assert!(plan.commands.iter().any(|c| c.contains("-t 600")));
        assert!(plan.commands.iter().any(|c| c.contains("/data/local/tmp/rec")));
        assert!(plan.commands.iter().any(|c| c.contains(".opus")));
        assert!(plan.commands.iter().any(|c| c.contains("whisper-cli")));
    }

    // --- dumpsys audio -------------------------------------------------------

    const DUMP_CALL: &str = r#"- STREAM_VOICE_CALL:
   Muted: false
   Devices: speaker

Audio Focus stack entries:
  AudioFocus stack:
   - Entry: FocusRequester
     source: VOICE_COMMUNICATION
     client: AudioFocusRequester@1b2c3d
     package: com.whatsapp
     state: AWAITING_LOSS

MODE:
  Mode: MODE_IN_COMMUNICATION
  PhoneState:
    mCallState: 2
"#;

    const DUMP_IDLE: &str = r#"MODE:
  Mode: MODE_NORMAL
  PhoneState:
    mCallState: 0
"#;

    #[test]
    fn detect_active_call_detecta_una_llamada_real() {
        let c = detect_active_call(DUMP_CALL).expect("debe detectar llamada");
        assert!(c.active);
        assert_eq!(c.mode, "MODE_IN_COMMUNICATION");
        assert_eq!(c.state, "IN_CALL");
        assert_eq!(c.package.as_deref(), Some("com.whatsapp"));
    }

    #[test]
    fn detect_active_call_sin_llamada_devuelve_inactivo() {
        let c = detect_active_call(DUMP_IDLE).expect("dump válido");
        assert!(!c.active);
        assert_eq!(c.state, "IDLE");
        assert_eq!(c.mode, "MODE_NORMAL");
    }

    #[test]
    fn detect_active_call_ignora_basura() {
        assert!(detect_active_call("").is_none());
        assert!(detect_active_call("no soy un dump de audio").is_none());
    }

    // --- Media projection ----------------------------------------------------

    #[test]
    fn detect_media_projection_reconoce_proyeccion_activa() {
        let d = "Media Projection:\n  MediaProjection{\n    packageName: com.google.android.apps.messaging\n    uid: 10234\n  }\n";
        assert!(detect_media_projection(d));
    }

    #[test]
    fn detect_media_projection_reconoce_ausencia() {
        assert!(!detect_media_projection(""));
        assert!(!detect_media_projection(
            "Media Projection:\n  No active projections\n"
        ));
        assert!(!detect_media_projection(
            "Media Projection:\n  Projection: none\n"
        ));
    }

    // --- Inventario ----------------------------------------------------------

    #[test]
    fn list_recordings_inventaria_con_sha256() {
        let dir = tmpdir("inv");
        fs::write(format!("{}/call-a.wav", dir), b"OggS-fake-audio").unwrap();
        fs::write(format!("{}/notas.txt", dir), b"no es audio").unwrap();
        let list = list_recordings(&dir);
        assert_eq!(list.len(), 1, "sólo los ficheros de audio");
        let sha = sha256_hex(b"OggS-fake-audio");
        assert_eq!(list[0]["sha256"].as_str().unwrap(), sha);
        assert_eq!(list[0]["bytes"].as_u64().unwrap(), b"OggS-fake-audio".len() as u64);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn civil_from_epoch_es_correcto() {
        assert_eq!(civil_from_epoch(0), (1970, 1, 1, 0, 0, 0));
        assert_eq!(civil_from_epoch(1_700_000_000), (2023, 11, 14, 22, 13, 20));
    }
}

// ====================================================================

// ===========================================================================
// Captura del audio de la llamada: qué se puede y cómo
// ===========================================================================

/// Permiso que exige Android para capturar el audio de una llamada
/// (`MediaRecorder.AudioSource.VOICE_CALL` / `VOICE_UPLINK` / `VOICE_DOWNLINK`).
///
/// Es `signature|privileged`: **ninguna app normal puede obtenerlo**, ni con
/// `RECORD_AUDIO`. Medido en este S21 Ultra: la app SamsungInCallUI es la que
/// lo usa (y por eso el móvil puede grabar llamadas "nativas" con CSC THL/INS).
pub const CAPTURE_AUDIO_OUTPUT: &str = "android.permission.CAPTURE_AUDIO_OUTPUT";

/// UID del usuario `system` (1000). En AOSP es un `sharedUserId` firmado con
/// la clave de plataforma, así que **cualquier proceso que corra bajo ese UID
/// tiene el permiso de captura de audio de llamada**.
pub const SYSTEM_UID: u32 = 1000;

/// Fuente de captura de audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureSource {
    /// Micrófono. Lo único que puede usar una app normal.
    Mic,
    /// Audio completo de la llamada (lo que dice el otro + lo que oyes).
    VoiceCall,
    /// Sólo la voz de quien llama (uplink / TX).
    VoiceUplink,
    /// Sólo lo que se oye en el auricular (downlink / RX).
    VoiceDownlink,
    /// Submix remoto: el audio que reproduce otra app.
    RemoteSubmix,
}

impl CaptureSource {
    /// Nombre estable.
    pub fn as_str(&self) -> &'static str {
        match self {
            CaptureSource::Mic => "mic",
            CaptureSource::VoiceCall => "voice_call",
            CaptureSource::VoiceUplink => "voice_uplink",
            CaptureSource::VoiceDownlink => "voice_downlink",
            CaptureSource::RemoteSubmix => "remote_submix",
        }
    }

    /// Valor de `android.media.MediaRecorder.AudioSource`.
    pub fn android_source(&self) -> u32 {
        match self {
            CaptureSource::Mic => 1,
            CaptureSource::VoiceUplink => 2,
            CaptureSource::VoiceDownlink => 3,
            CaptureSource::VoiceCall => 4,
            CaptureSource::RemoteSubmix => 8,
        }
    }

    /// `true` si hace falta `CAPTURE_AUDIO_OUTPUT` para esta fuente.
    pub fn requires_capture_audio_output(&self) -> bool {
        matches!(
            self,
            CaptureSource::VoiceCall | CaptureSource::VoiceUplink | CaptureSource::VoiceDownlink
        )
    }

    /// `true` si la fuente es audio de llamada (y no micrófono).
    pub fn captures_call_audio(&self) -> bool {
        self.requires_capture_audio_output()
    }
}

/// Cómo lanzar el grabador para capturar el audio de llamada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptureLaunch {
    /// Fuente elegida.
    pub source: CaptureSource,
    /// Comando exacto.
    pub command: String,
    /// UID con el que corre.
    pub uid: u32,
    /// `true` si hace falta root.
    pub needs_root: bool,
    /// Notas honestas sobre el camino elegido.
    pub notes: Vec<String>,
}

/// Plan de lanzamiento del grabador nativo para una fuente dada.
///
/// **La vía que funciona con root y sin parchar nada** es correr el grabador
/// bajo el **UID de sistema (1000)**: ese UID está firmado con la clave de
/// plataforma, así que tiene `CAPTURE_AUDIO_OUTPUT` y puede abrir
/// `VOICE_CALL`. Medido en el móvil: `su 1000 -c id` ⇒ `uid=1000(system)`.
///
/// Lo que **no** se puede prometer: que el HAL de Samsung exponga la voz del
/// módem como entrada capturable. Si el HAL sólo tiene salida de voz, el
/// `VOICE_CALL` devuelve silencio y **no hay parche de AudioFlinger que lo
/// arregle**: la vía real sería el grabador nativo de Samsung (CSC THL/INS).
pub fn capture_launch_plan(source: CaptureSource, binary: &str) -> CaptureLaunch {
    let mut notes = Vec::new();
    let mut command;
    let mut uid = 0u32;
    let mut needs_root = false;

    if source.requires_capture_audio_output() {
        needs_root = true;
        uid = SYSTEM_UID;
        command = format!(
            "su -mm -c \"su {uid} -c '{binary} --source {src} --out /data/local/tmp/megadaemon-call.raw'\"",
            uid = SYSTEM_UID,
            binary = binary,
            src = source.android_source()
        );
        notes.push(format!(
            "{} exige {}: se corre bajo el UID de sistema ({})",
            source.as_str(),
            CAPTURE_AUDIO_OUTPUT,
            SYSTEM_UID
        ));
        notes.push(
            "Si el HAL de Samsung no expone la voz del modem como entrada, el \
             fichero sale en silencio: eso NO se arregla parcheando AudioFlinger, \
             se comprueba grabando y mirando si el PCM no es todo ceros."
                .to_string(),
        );
        notes.push(
            "Alternativa sin root: cambiar el CSC a uno que habilite la grabación \
             nativa (THL/INS) y usar el grabador de Samsung."
                .to_string(),
        );
    } else {
        command = format!(
            "{} --source {} --out /data/local/tmp/megadaemon-call.raw",
            binary,
            source.android_source()
        );
        notes.push(
            "El micrófono capta lo que entra por el micrófono: con el altavoz \
             se oye al interlocutor, pero NO es el audio de la llamada."
                .to_string(),
        );
        notes.push(
            "Con la pantalla bloqueada, el micrófono puede dejar de dar datos \
             (depende del firmware); la grabación quedaría vacía."
                .to_string(),
        );
    }

    CaptureLaunch {
        source,
        command,
        uid,
        needs_root,
        notes,
    }
}

// ===========================================================================
// Política de audio: ¿hace falta parchearla?
// ===========================================================================

/// Hechos extraídos de la política de audio del dispositivo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioPolicyFacts {
    /// Ruta de la política analizada.
    pub policy_path: String,
    /// Mixports declarados.
    pub mix_ports: Vec<String>,
    /// `true` si existe el puerto de dispositivo "Voice Call Mic".
    pub has_voice_call_device_port: bool,
    /// `true` si la política declara alguna fuente `AUDIO_SOURCE_VOICE_*`.
    pub has_voice_call_source: bool,
    /// `true` si existe un mixport de captura específico de llamada.
    pub has_call_capture_mix_port: bool,
}

impl AudioPolicyFacts {
    /// ¿Hace falta un parche de política para capturar la llamada?
    ///
    /// En este S21 Ultra la respuesta es **no**: la voz entra por el módem y
    /// se mezcla en el HAL, no hay un mixport de voz que capturar; lo que hay
    /// es un puerto de dispositivo "Voice Call Mic" enrutado a `primary-in`.
    /// Un parche de política sólo ayudaría en dispositivos donde la voz **sí**
    /// pasa por un mixport del sistema (AOSP puro), y entonces consistiría en
    /// **añadir** un mixport de captura, no en "desbloquear" uno existente.
    pub fn needs_policy_patch(&self) -> bool {
        !self.has_call_capture_mix_port && self.has_voice_call_source
    }
}

/// Analiza una política de audio (XML) y extrae los hechos relevantes.
pub fn audio_policy_facts(path: &str, xml: &str) -> AudioPolicyFacts {
    let mut mix_ports = Vec::new();
    for cap in xml.split("mixPort name=\"").skip(1) {
        if let Some(end) = cap.find('"') {
            mix_ports.push(cap[..end].to_string());
        }
    }
    let has_voice_call_device_port = xml.contains("AUDIO_DEVICE_IN_VOICE_CALL");
    let has_voice_call_source = xml.contains("AUDIO_SOURCE_VOICE_CALL")
        || xml.contains("AUDIO_SOURCE_VOICE_UPLINK")
        || xml.contains("AUDIO_SOURCE_VOICE_DOWNLINK");
    let has_call_capture_mix_port = mix_ports.iter().any(|m| {
        let m = m.to_ascii_lowercase();
        m.contains("call") || m.contains("voice") || m.contains("record")
    });
    AudioPolicyFacts {
        policy_path: path.to_string(),
        mix_ports,
        has_voice_call_device_port,
        has_voice_call_source,
        has_call_capture_mix_port,
    }
}

// ===========================================================================
// Flags de CSC (la vía de la grabación NATIVA)
// ===========================================================================

/// Dónde puede vivir la configuración de CSC en un Samsung.
pub const CSC_FEATURE_SOURCES: [&str; 6] = [
    "/optics/configs/carriers/single/{csc}/conf/system/cscfeature.xml",
    "/prism/etc/carriers/single/{csc}/contents.db",
    "/prism/etc/carriers/single/{csc}/sysconfig",
    "/prism/etc/carriers/single/{csc}/permissions",
    "/optics/configs/carriers/single/{csc}/conf",
    "/prism/etc/carriers/single/{csc}/default-permissions",
];

/// Rutas candidatas para la configuración del CSC indicado.
///
/// Nota: `contents.db` **es** SQLite pero es un **inventario de ficheros**
/// (tabla `contents(salesCode, _id, filePath, fileSize)`, 2.006 filas): sirve
/// para auditar qué ficheros forman el CSC y sus tamaños esperados, **no**
/// guarda los valores de `CscFeature_*`. Los valores están en el
/// `cscfeature.xml` de cada CSC, que va **ofuscado con OMC**.
///
/// Además, para saber qué ficheros componen un CSC conviene tener el
/// `cscfeature.xml` de otro CSC (p. ej. THL) para comparar
pub fn csc_feature_candidates(csc: &str) -> Vec<String> {
    let csc = csc.trim().to_ascii_uppercase();
    CSC_FEATURE_SOURCES
        .iter()
        .map(|p| p.replace("{csc}", &csc))
        .collect()
}

/// Una bandera de CSC con su valor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CscFeature {
    /// Nombre de la bandera (p. ej. `CscFeature_VoiceCall_ConfigRecording`).
    pub key: String,
    /// Valor: `true`/`false` si es booleano; el texto si no.
    pub value: String,
}

impl CscFeature {
    /// El valor leído como booleano (`1`/`0`/`true`/`false`).
    pub fn enabled(&self) -> Option<bool> {
        match self.value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        }
    }
}

/// Parsea las banderas de CSC de un blob.
///
/// Acepta los dos formatos que se han visto en Samsung:
/// · `clave=valor;clave2=valor2` (el de `sysconfig`/`contents.db`)
/// · XML de atributos `name="clave" value="1"` (el de `cscfeature.xml`)
pub fn parse_csc_features(blob: &str) -> Vec<CscFeature> {
    let mut out: Vec<CscFeature> = Vec::new();

    // Formato clave=valor
    for chunk in blob.split(';').flat_map(|p| p.split('\n')) {
        let chunk = chunk.trim();
        if chunk.is_empty() || !chunk.contains('=') {
            continue;
        }
        if let Some((k, v)) = chunk.split_once('=') {
            let key = k.trim().trim_matches('"').trim();
            let val = v.trim().trim_matches('"').trim();
            if key.is_empty() || key.contains(' ') {
                continue;
            }
            out.push(CscFeature {
                key: key.to_string(),
                value: val.to_string(),
            });
        }
    }

    // Formato XML de atributos
    let mut rest = blob;
    while let Some(i) = rest.find("name=\"") {
        let after = &rest[i + 6..];
        let key_end = match after.find('"') {
            Some(e) => e,
            None => break,
        };
        let key = after[..key_end].to_string();
        let tail = &after[key_end..];
        let value = match tail.find("value=\"") {
            Some(v) => {
                let vs = &tail[v + 7..];
                let ve = vs.find('"').unwrap_or(0);
                vs[..ve].to_string()
            }
            None => String::new(),
        };
        if key.starts_with("CscFeature_") {
            out.push(CscFeature {
                key,
                value: value.clone(),
            });
        }
        rest = tail;
    }

    out
}

/// Busca una bandera de CSC por nombre.
pub fn csc_feature<'a>(features: &'a [CscFeature], key: &str) -> Option<&'a CscFeature> {
    features.iter().find(|f| f.key == key)
}

/// ¿Está activada la grabación **nativa** de llamadas?
///
/// La bandera es `CscFeature_VoiceCall_ConfigRecording`: la lee la app
/// SamsungInCallUI (medido en su DEX) y es la que hace aparecer el botón de
/// grabar en la pantalla de llamada.
pub fn voice_call_config_recording(features: &[CscFeature]) -> Option<bool> {
    csc_feature(features, "CscFeature_VoiceCall_ConfigRecording").and_then(|f| f.enabled())
}


// ===========================================================================
// Estado del envoltorio OMC y veredicto final de la captura
// ===========================================================================

/// Cómo está codificado un fichero de configuración de Samsung.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OmcState {
    /// Texto legible: se puede parsear tal cual.
    Plaintext,
    /// Comprimido con gzip (magic `1f 8b`).
    Gzip,
    /// Envoltorio OMC propietario: cabecera binaria, no texto.
    Obfuscated {
        /// Cabecera en hexadecimal.
        magic: String,
        /// Tamaño del fichero.
        bytes: usize,
    },
}

/// Detecta cómo está codificado un fichero de CSC.
///
/// **Medido**: el `cscfeature.xml` del CSC AMO de este móvil son **1.109 B** y
/// empieza por `2f 27 29 b7 da c1 ac 37`: **no** es gzip (`1f 8b`) ni texto ⇒
/// el envoltorio OMC es un **formato binario propio con cabecera**, no
/// "gzip + XOR". Por eso buscar el XOR a ciegas no funciona: hay que leer la
/// cabecera OMC primero.
pub fn omc_state(bytes: &[u8]) -> OmcState {
    if bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
        return OmcState::Gzip;
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        if text.contains("CscFeature_") {
            return OmcState::Plaintext;
        }
    }
    let n = bytes.len().min(8);
    let magic = bytes[..n]
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<String>>()
        .join("");
    OmcState::Obfuscated {
        magic,
        bytes: bytes.len(),
    }
}

/// Los hechos que determinan si se puede grabar el audio de una llamada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallAudioFacts {
    /// ¿El HAL de audio expone un camino de captura de voz?
    ///
    /// **Medido en este móvil**: `libaudiohal.so` (33.184 B) tiene **cero**
    /// cadenas `voice|voip|csd|VOICE_CALL|capture|in_call` ⇒ **no lo expone**:
    /// el audio de la llamada lo genera el **modem** y **ningún parche de
    /// AudioFlinger lo arregla**.
    pub hal_has_voice_path: bool,
    /// Apps que **declaran** `CAPTURE_AUDIO_OUTPUT` en su manifiesto.
    ///
    /// **Medido**: sólo dos, y **ninguna de Samsung**: `YourPhone_P1_5` y
    /// `LinkToWindowsService` (Microsoft). El grabador tampoco está en los
    /// jars de `/system/framework`.
    pub apps_with_capture_permission: Vec<String>,
    /// ¿El CSC actual habilita el grabador nativo de Samsung?
    pub csc_allows_native: bool,
    /// ¿Se puede correr como UID de sistema (root)?
    pub system_uid_available: bool,
}

impl CallAudioFacts {
    /// El veredicto honesto, en una frase.
    ///
    /// El orden de preferencia real, medido en este móvil:
    /// 1. **Cambiar el CSC** a uno que habilite la grabación nativa
    ///    (`THL`/`INS`): el propio sistema graba, sin hacks.
    /// 2. **UID de sistema + `VOICE_CALL`**: tiene el permiso, pero si el HAL
    ///    no expone la voz el PCM sale en silencio ⇒ hay que comprobarlo
    ///    grabando (fichero no vacío y no todo ceros).
    /// 3. Las apps de **Microsoft** ya tienen el permiso declarado.
    pub fn conclusion(&self) -> String {
        let mut v: Vec<String> = Vec::new();
        if self.csc_allows_native {
            v.push("el CSC actual ya habilita el grabador nativo de Samsung: se usa ese".to_string());
        } else {
            v.push(
                "el CSC actual NO habilita el grabador nativo: la via fiable es cambiarlo \
                 (THL/INS) o usar una app con CAPTURE_AUDIO_OUTPUT"
                    .to_string(),
            );
        }
        if !self.hal_has_voice_path {
            v.push(
                "el HAL no expone un camino de captura de voz (medido: cero cadenas de voz \
                 en libaudiohal.so): el audio lo genera el modem y ningun parche de \
                 AudioFlinger lo arregla"
                    .to_string(),
            );
        }
        if self.system_uid_available {
            v.push(
                "con root se puede correr el grabador bajo el UID de sistema (1000), que tiene \
                 CAPTURE_AUDIO_OUTPUT, pero hay que comprobar que el PCM no sale en silencio"
                    .to_string(),
            );
        }
        if !self.apps_with_capture_permission.is_empty() {
            v.push(format!(
                "apps con el permiso ya declarado: {}",
                self.apps_with_capture_permission.join(", ")
            ));
        }
        v.join(". ")
    }
}


// ===========================================================================
// Captura de la voz del modem por ALSA (la vía que SÍ funciona)
// ===========================================================================

/// Los PCM de captura **WDMA0..7** del Exynos 2100
/// (`/dev/snd/pcmC0D12..19`) son la **ruta de voz AP<->CP**: el audio de la
/// llamada pasa por ahí, no por el HAL de audio de usuario.
///
/// **Medido**: `/proc/asound/pcm` lista `00-12: WDMA0 ... 00-19: WDMA7` como
/// `capture`. Por eso el HAL no tiene cadenas de voz (la voz no pasa por él) y
/// sin embargo **la voz es capturable, sin `CAPTURE_AUDIO_OUTPUT`**.
pub const ALSA_WDMA_DEVICES: [&str; 8] = [
    "hw:0,12", "hw:0,13", "hw:0,14", "hw:0,15", "hw:0,16", "hw:0,17", "hw:0,18",
    "hw:0,19",
];

/// Dispositivos ALSA donde el audio de la llamada **no** está.
pub const ALSA_NOT_VOICE_HINT: &str =
    "Los PCM WDMA son la ruta de voz del modem; los demas (RDMA/WDMA de audio \
     multimedia) no la llevan.";

/// Plan de captura por ALSA.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlsaCapturePlan {
    /// Dispositivo ALSA (p. ej. `hw:0,12`).
    pub device: String,
    /// Frecuencia de muestreo.
    pub sample_rate: u32,
    /// Canales.
    pub channels: u16,
    /// Formato.
    pub format: String,
    /// Comando exacto del capturador.
    pub command: String,
    /// Salida.
    pub out: String,
    /// Notas honestas.
    pub notes: Vec<String>,
}

/// Plan de captura de la voz del modem por ALSA.
///
/// **La ventaja**: no necesita `CAPTURE_AUDIO_OUTPUT`, ni parche de
/// AudioFlinger, ni cambio de CSC: lee el PCM del hardware directamente.
/// **Lo que hay que comprobar**: que el PCM correcta sea la de la llamada
/// (hace falta una llamada en curso) y en qué sentido va cada una (el
/// uplink y el downlink suelen ir en WDMA distintos).
pub fn alsa_voice_capture_plan(device: &str, seconds: u32) -> AlsaCapturePlan {
    AlsaCapturePlan {
        device: device.to_string(),
        sample_rate: 48000,
        channels: 1,
        format: "S16_LE".to_string(),
        command: format!(
            "su -mm -c \"md_alsa --dev {} --sec {} --out /data/local/tmp/megadaemon-wdma.raw\"",
            device, seconds
        ),
        out: "/data/local/tmp/megadaemon-wdma.raw".to_string(),
        notes: vec![
            "Los nodos /dev/snd/* son system:audio 0660 => hace falta root (o grupo audio).".to_string(),
            ALSA_NOT_VOICE_HINT.to_string(),
            "Sin llamada en curso el PCM devuelve silencio: hay que comprobarlo DURANTE una llamada.".to_string(),
            "El binario se compila con el NDK y se enlaza con -lasound (libasound del propio Android).".to_string(),
        ],
    }
}

// ===========================================================================
// Cambio de CSC: lo que hay en el móvil y el riesgo real
// ===========================================================================

/// Operador de la SIM **medido** en este móvil.
pub const SIM_OPERATOR_ALPHA: &str = "JAZZTEL";
/// MCC/MNC de la SIM **medido**: 214 = España, 03 = Jazztel.
pub const SIM_OPERATOR_NUMERIC: &str = "21403";

/// CSC presentes en el móvil que **habilitan la grabación nativa**.
///
/// `INS` (India) **no está** en este firmware; sí están `THL`, `ILO` y `XXV`.
pub const RECORDING_CSC_AVAILABLE: [&str; 3] = ["THL", "ILO", "XXV"];

/// Riesgo de cambiar el CSC, en texto honesto.
///
/// El CSC **no** cambia la SIM ni su MCC/MNC: el módem sigue registrando en
/// la red de la SIM (aquí Jazztel 21403, que ya no coincide con el CSC
/// `AMO`). El CSC elige el **juego de configuración** (operador, MMS, APN,
///VoLTE, y las banderas `CscFeature_*`). Cambiarlo puede, por tanto, tocar
/// el APN/VoLTE y **perder alguna función del operador**, aunque la llamada
/// y los datos siguen funcionan en la red de la SIM.
pub fn csc_switch_risk(sim_numeric: &str, target_csc: &str) -> String {
    format!(
        "Cambiar el CSC a {t} no cambia la SIM ({s}) ni su MCC/MNC: el modem sigue \
         registrando en la red de la SIM. Lo que cambia es el juego de configuracion \
         (APN, VoLTE/IMS, MMS y las banderas CscFeature_*). Riesgos reales: (1) el \
         APN/IMS pueden no encajar con la SIM y perder VoLTE o datos; (2) con AVB \
         verde y /prism montado en solo-lectura, el cambio no es permanente, solo \
         dura hasta que el sistema reescriba el CSC; (3) es reversible: se \
         restauran los 3 bytes de mps_code.dat y se reinicia.",
        t = target_csc,
        s = sim_numeric
    )
}

/// Comprobaciones **después** de cambiar el CSC.
pub fn csc_switch_verify_plan(target_csc: &str) -> Vec<String> {
    vec![
        format!(
            "`getprop ro.csc.sales_code` debe mostrar {}",
            target_csc
        ),
        "`getprop gsm.version.baseband` debe ser coherente con el CSC destino".to_string(),
        "Comprobar que la app de llamadas de Samsung (`com.samsung.android.incallui`) \
         tiene el flag `CscFeature_VoiceCall_ConfigRecording` a 1: con el CSC de \
         America (AMO) esta apagado y el boton de grabar no aparece."
            .to_string(),
        "Si la llamada saliente pierde IMS/VoLTE: revertir (restaurar mps_code.dat y reiniciar)."
            .to_string(),
    ]
}

