//! MegaDaemon v13.4 — Extractores de mensajería y de citas médicas.
//!
//! Este módulo saca la conversación real del usuario (Telegram, WhatsApp,
//! SMS/MMS) y sus citas médicas (Salud Responde de Andalucía + los SMS del
//! propio hospital) del dispositivo Android con root, y lo normaliza al
//! **mismo esquema que usa el motor de sync de IA** (`src/sync.rs`):
//!
//! ```json
//! {"service":"telegram","id":"chat_42","title":"Ana","updated_at":1712345678.0,
//!  "messages":[{"role":"user","content":"hola","ts":1712345678.0}]}
//! ```
//!
//! Reglas de diseño (importantes):
//!
//!  * **Lógica pura separada del dispositivo.** Todo el parseo (filas de
//!    `content query`, esquema real de Telegram, texto de WhatsApp, palabras
//!    clave médicas, fechas en español) vive en funciones puras, testeables
//!    sin móvil.  Lo único que habla con el dispositivo son
//!    `crate::android::root_exec` y `crate::android::fs_read`.
//!  * **Nada inventado.** Si un almacén está cifrado (WhatsApp) se dice
//!    explícitamente por qué; no se escribe ni una sola palabra de texto que
//!    no venga literalmente del dispositivo.
//!  * **Copia antes de leer.** Las apps escriben en caliente: cada BD se
//!    copia (con su `-wal` y su `-shm`) a un snapshot antes de abrirse.
//!
//! Roles (los mismos que usa el motor de sync de IA):
//!
//!  * `user`      → mensaje **saliente** (lo escribió el usuario)
//!  * `peer`      → mensaje **entrante** (lo escribió el otro)
//!  * `assistant` → reservado para el motor de IA

use rusqlite::Connection;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// ========================================================================= //
// Constantes
// ========================================================================= //

/// Paquete de Telegram (oficial).
pub const TELEGRAM_PACKAGE: &str = "org.telegram.messenger";
/// Paquete de WhatsApp principal.
pub const WHATSAPP_PACKAGE: &str = "com.whatsapp";
/// WhatsApp Business.
pub const WHATSAPP_W4B_PACKAGE: &str = "com.whatsapp.w4b";
/// Proveedor de SMS/MMS de AOSP.
pub const SMS_PACKAGE: &str = "com.google.android.apps.messaging";
/// Paquete real de "Salud Responde" (Servicio Andaluz de Salud).
pub const SALUD_RESPONDE_PACKAGE: &str = "citamovil.saludresponde";

/// Directorio de snapshots en el dispositivo (mismo criterio que el
/// extractor Python de `sync.rs`: global y legible por el proceso).
const SNAPSHOT_DIR: &str = "/data/local/tmp/megadaemon/messaging_snap";

/// Palabras clave que delatan una cita o un asunto médico en un mensaje.
///
/// Se comparan **sin acentos y por token**, de forma que "análisis" y
/// "analitica" son la misma palabra y "citación" NO es una cita.
pub const MEDICAL_KEYWORDS: &[&str] = &[
    "cita",
    "citas",
    "cita previa",
    "citas previas",
    "consulta",
    "consultas",
    "medico",
    "medica",
    "medicos",
    "medicas",
    "hospital",
    "hospitales",
    "analitica",
    "analiticas",
    "analisis",
    "vacunacion",
    "vacunaciones",
    "revision",
    "revisiones",
    "urgencia",
    "urgencias",
    "centro de salud",
    "salud responde",
    "laboratorio",
    "radiografia",
    "ecografia",
    "resonancia",
    "receta medica",
    "dentista",
    "pediatra",
    "cardio",
    "dermatologo",
    "ginecologo",
    "oftalmologo",
    "psicologo",
    "traumatologo",
];

/// Motivos (en el mismo esquema) por los que el almacén de mensajes de
/// WhatsApp no se puede leer sin inyectarse en el proceso.
pub const WHATSAPP_ENCRYPTION_REASON: &str = "El almacen de mensajes de WhatsApp (msgstore.db y su variante cifrada .crypt15) esta cifrado con una clave AES que vive en el AndroidKeyStore y no es exportable: sin root en el keystore, o sin inyectarse en el proceso de WhatsApp (Frida) no se puede descifrar. Si solo se lee wa.db se obtiene la metadata (contactos, chats, no leidos) y el texto visible de las notificaciones push, que no van cifrados.";

/// Ficheros de datos candidatos por app (rutas exactas en el dispositivo).
struct AppSpec {
    service: &'static str,
    package: &'static str,
    candidates: &'static [&'static str],
}

const APP_SPECS: &[AppSpec] = &[
    AppSpec {
        service: "whatsapp",
        package: WHATSAPP_PACKAGE,
        candidates: &[
            "/data/data/com.whatsapp/databases/msgstore.db",
            "/data/data/com.whatsapp/databases/msgstore.db.crypt15",
            "/data/data/com.whatsapp/databases/wa.db",
        ],
    },
    AppSpec {
        service: "whatsapp_business",
        package: WHATSAPP_W4B_PACKAGE,
        candidates: &[
            "/data/data/com.whatsapp.w4b/databases/msgstore.db",
            "/data/data/com.whatsapp.w4b/databases/msgstore.db.crypt15",
            "/data/data/com.whatsapp.w4b/databases/wa.db",
        ],
    },
    AppSpec {
        service: "telegram",
        package: TELEGRAM_PACKAGE,
        candidates: &[
            "/data/data/org.telegram.messenger/databases/msgstore.db",
            "/data/data/org.telegram.messenger/databases/messages.db",
            "/data/data/org.telegram.messenger/databases/messages2.db",
        ],
    },
    AppSpec {
        service: "sms",
        package: SMS_PACKAGE,
        candidates: &[
            "/data/data/com.google.android.apps.messaging/databases/mmssms.db",
            "/data/data/com.android.providers.telephony/databases/mmssms.db",
        ],
    },
    AppSpec {
        service: "salud_responde",
        package: SALUD_RESPONDE_PACKAGE,
        candidates: &[
            "/data/data/citamovil.saludresponde/databases/saludresponde.db",
            "/data/data/citamovil.saludresponde/databases/citas.db",
            "/data/data/citamovil.saludresponde/databases/appointments.db",
        ],
    },
];

// ========================================================================= //
// 1. Utilidades de texto (puro)
// ========================================================================= //

/// Normaliza para comparar: minúsculas y sin acentos.
///
/// Así "análisis" y "analitica" son la misma palabra y la comparación no
/// depende de la codificación del mensaje.
pub fn normalize_for_match(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'Á' => 'a',
            'é' | 'É' => 'e',
            'í' | 'Í' => 'i',
            'ó' | 'Ó' => 'o',
            'ú' | 'Ú' => 'u',
            'ü' | 'Ü' => 'u',
            'ñ' | 'Ñ' => 'n',
            _ => c,
        })
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Trocea el texto normalizado en palabras (secuencias alfanuméricas).
pub fn tokenize_match(s: &str) -> Vec<String> {
    normalize_for_match(s)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect()
}

/// ¿Aparece `needle` como secuencia contigua de tokens dentro de `tokens`?
fn contains_subsequence(tokens: &[String], needle: &[String]) -> bool {
    if needle.is_empty() || needle.len() > tokens.len() {
        return false;
    }
    (0..=tokens.len() - needle.len())
        .any(|i| tokens[i..i + needle.len()].iter().eq(needle.iter()))
}

/// Devuelve las palabras clave médicas que aparecen en el texto.
///
/// La comparación es por token y sin acentos: "Tienes cita con el médico"
/// devuelve `["cita", "medico"]`.
pub fn detect_medical_keywords(text: &str) -> Vec<String> {
    let tokens = tokenize_match(text);
    let mut found: Vec<String> = Vec::new();
    for kw in MEDICAL_KEYWORDS {
        let ktoks = tokenize_match(kw);
        if ktoks.is_empty() {
            continue;
        }
        if contains_subsequence(&tokens, &ktoks) && !found.iter().any(|f| f == kw) {
            found.push(kw.to_string());
        }
    }
    found
}

/// ¿El texto habla de una cita o de un asunto médico?
pub fn is_medical_text(text: &str) -> bool {
    !detect_medical_keywords(text).is_empty()
}

/// Limpia un texto para usarlo como título (una línea, sinTrim).
pub fn title_from_text(text: &str, max: usize) -> String {
    let t: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() > max {
        let cut: String = t.chars().take(max).collect();
        format!("{}…", cut)
    } else {
        t
    }
}

// ========================================================================= //
// 2. Fechas (puro)
// ========================================================================= //

/// Fecha parseada dentro de un texto.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedDate {
    /// Segundos desde epoch (UTC).
    pub epoch: i64,
    /// Fecha y hora en ISO corto: "2025-03-12 09:00:00".
    pub iso: String,
    /// Texto original que se usó para la fecha.
    pub raw: String,
    /// Precisión: "day" (día exacto), "month" (sólo mes) o "relative".
    pub precision: String,
}

/// Días → fecha civil (algoritmo de Hinnant, sin dependencias).
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let zz = z + 719_468;
    let era = if zz >= 0 { zz } else { zz - 146_096 } / 146_097;
    let doe = zz - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if m <= 2 { y + 1 } else { y };
    (year, m as u32, d as u32)
}

/// Fecha civil → días desde epoch.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m as i64 - 3 } else { m as i64 + 9 };
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Epoch (segundos) → "YYYY-MM-DD HH:MM:SS" (UTC).
pub fn format_epoch(epoch: i64) -> String {
    let days = epoch.div_euclid(86_400);
    let secs = epoch.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        y,
        m,
        d,
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

/// Epoch (segundos) → "YYYY-MM-DD".
pub fn format_date(epoch: i64) -> String {
    let (y, m, d) = civil_from_days(epoch.div_euclid(86_400));
    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// Epoch actual en segundos.
pub fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Inicio del día (00:00 UTC) del epoch dado.
pub fn start_of_day(epoch: i64) -> i64 {
    epoch.div_euclid(86_400) * 86_400
}

/// Meses en español (normalizados, sin acentos).
const MONTHS_ES: &[&str] = &[
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "setiembre",
    "octubre",
    "noviembre",
    "diciembre",
];

/// Días de la semana (normalizados).
const WEEKDAYS_ES: &[&str] = &[
    "lunes",
    "martes",
    "miercoles",
    "jueves",
    "viernes",
    "sabado",
    "domingo",
];

/// Token simple del texto: número o palabra.
#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(i64, usize),
    Word(String, usize),
}

/// Trocea el texto en números y palabras normalizadas.
fn scan_tokens(text: &str) -> Vec<Tok> {
    let norm = normalize_for_match(text);
    let mut out = Vec::new();
    let mut chars = norm.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c.is_ascii_digit() {
            let mut s = String::from(c);
            while let Some((_, d)) = chars.peek() {
                if d.is_ascii_digit() {
                    s.push(*d);
                    chars.next();
                } else {
                    break;
                }
            }
            if let Ok(v) = s.parse::<i64>() {
                out.push(Tok::Num(v, i));
            }
        } else if c.is_alphanumeric() {
            let mut s = String::from(c);
            while let Some((_, d)) = chars.peek() {
                if d.is_alphanumeric() {
                    s.push(*d);
                    chars.next();
                } else {
                    break;
                }
            }
            out.push(Tok::Word(s, i));
        }
    }
    out
}

/// Devuelve (inicio, fin, valor, nº de dígitos) de cada racha de dígitos.
fn digit_runs(text: &str) -> Vec<(usize, usize, i64, usize)> {
    let mut out = Vec::new();
    let bytes: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            let mut val: i64 = 0;
            let mut n = 0usize;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                val = val.saturating_mul(10).saturating_add(bytes[i] as i64 - '0' as i64);
                n += 1;
                i += 1;
            }
            out.push((start, i, val, n));
        } else {
            i += 1;
        }
    }
    out
}

/// Texto que hay entre dos posiciones del original.
fn between(text: &str, from: usize, to: usize) -> String {
    text.chars()
        .skip(from)
        .take(to.saturating_sub(from))
        .collect()
}

/// ¿El texto entre dos posiciones es un separador de fecha (`/`, `-`, `.`)?
fn is_date_separator(text: &str, from: usize, to: usize) -> bool {
    let mid = between(text, from, to);
    !mid.is_empty() && mid.chars().all(|c| c == '/' || c == '-' || c == '.')
}

/// ¿El texto entre dos posiciones es un separador de hora (`:` o `.`)?
fn is_time_separator(text: &str, from: usize, to: usize) -> bool {
    let mid = between(text, from, to);
    !mid.is_empty() && mid.chars().all(|c| c == ':' || c == '.')
}

/// Construye una fecha-hora a partir de día civil + hora.
fn make_epoch(y: i64, m: u32, d: u32, hour: u32, min: u32) -> i64 {
    let mut days = days_from_civil(y, m, d);
    // Si el día no existe (31 de febrero) se recurre al día 1 del mes.
    let (_, cm, cd) = civil_from_days(days);
    if cm != m || cd != d {
        days = days_from_civil(y, m, 1);
    }
    days * 86_400 + (hour as i64) * 3600 + (min as i64) * 60
}

/// Aplica el "año implícito": si la fecha sin año cae lejos en el pasado,
/// se asume el año siguiente (p. ej. "el 12 de marzo" dicho en diciembre).
fn resolve_year(month: u32, day: u32, now: i64) -> i64 {
    let (cy, _, _) = civil_from_days(now.div_euclid(86_400));
    let mut year = cy;
    let candidate = make_epoch(year, month, day, 9, 0);
    if candidate < now - 60 * 86_400 {
        year += 1;
    }
    year
}

/// Busca una fecha numérica (`dd/mm/aaaa`, `aaaa-mm-dd`, con hora opcional).
fn parse_numeric_date(text: &str, _now: i64) -> Option<ParsedDate> {
    let runs = digit_runs(text);
    if runs.len() < 3 {
        return None;
    }
    for i in 0..runs.len() - 2 {
        let a = runs[i];
        let b = runs[i + 1];
        let c = runs[i + 2];
        if !is_date_separator(text, a.1, b.0) || !is_date_separator(text, b.1, c.0) {
            continue;
        }
        let (y, m, d) = if a.3 == 4 {
            (a.2, b.2 as u32, c.2 as u32)
        } else if c.3 == 4 {
            (c.2, b.2 as u32, a.2 as u32)
        } else if c.3 == 2 {
            (2000 + c.2, b.2 as u32, a.2 as u32)
        } else {
            continue;
        };
        if !(1..=12).contains(&m) || !(1..=31).contains(&d) || !(1900..=2200).contains(&y) {
            continue;
        }
        // Hora opcional "hh:mm" o "hh.mm" justo después de la fecha.
        let (mut hour, mut min) = (0u32, 0u32);
        let mut found_time = false;
        if let Some(hr) = runs.get(i + 3) {
            if hr.3 <= 2 {
                if let Some(next) = runs.get(i + 4) {
                    if next.3 == 2 && is_time_separator(text, hr.1, next.0) {
                        hour = hr.2 as u32;
                        min = next.2 as u32;
                        found_time = true;
                    }
                }
            }
        }
        let epoch = make_epoch(y, m, d, hour, min);
        let raw: String = text
            .chars()
            .skip(a.0)
            .take(c.1.saturating_sub(a.0))
            .collect();
        return Some(ParsedDate {
            epoch,
            iso: format_epoch(epoch),
            raw: raw.trim().to_string(),
            precision: if found_time { "minute".into() } else { "day".into() },
        });
    }
    None
}

/// Busca una fecha con el mes escrito ("12 de marzo", "12 de marzo de 2025").
fn parse_long_date(text: &str, now: i64) -> Option<ParsedDate> {
    let toks = scan_tokens(text);
    for i in 0..toks.len() {
        let Tok::Word(w, _) = &toks[i] else { continue };
        if w != "de" {
            continue;
        }
        // "12 de marzo [de 2025]"
        if i >= 1 && i + 1 < toks.len() {
            if let (Tok::Num(day, _), Tok::Word(mw, _)) = (&toks[i - 1], &toks[i + 1]) {
                if let Some(mi) = MONTHS_ES.iter().position(|m| *m == mw.as_str()) {
                    let month = mi as u32 + 1;
                    let mut year: Option<i64> = None;
                    if i + 3 < toks.len() {
                        if let (Tok::Word(de2, _), Tok::Num(yv, _)) = (&toks[i + 2], &toks[i + 3]) {
                            if de2 == "de" && *yv > 1900 && *yv < 2200 {
                                year = Some(*yv);
                            }
                        }
                    }
                    let y = year.unwrap_or_else(|| resolve_year(month, *day as u32, now));
                    let epoch = make_epoch(y, month, *day as u32, 9, 0);
                    return Some(ParsedDate {
                        epoch,
                        iso: format_epoch(epoch),
                        raw: format!("{} de {}", day, mw),
                        precision: if year.is_some() { "day".into() } else { "month".into() },
                    });
                }
            }
        }
        // "marzo de 12" (raro, pero se soporta)
        if i >= 1 && i + 1 < toks.len() {
            if let (Tok::Word(mw, _), Tok::Num(day, _)) = (&toks[i - 1], &toks[i + 1]) {
                if let Some(mi) = MONTHS_ES.iter().position(|m| *m == mw.as_str()) {
                    let month = mi as u32 + 1;
                    let y = resolve_year(month, *day as u32, now);
                    let epoch = make_epoch(y, month, *day as u32, 9, 0);
                    return Some(ParsedDate {
                        epoch,
                        iso: format_epoch(epoch),
                        raw: format!("{} de {}", mw, day),
                        precision: "month".into(),
                    });
                }
            }
        }
    }
    None
}

/// Fechas relativas: hoy, mañana, pasado mañana, el lunes, la semana que viene.
fn parse_relative_date(text: &str, now: i64) -> Option<ParsedDate> {
    let norm = normalize_for_match(text);
    let tokens = tokenize_match(text);
    let today = start_of_day(now);
    let day = 86_400i64;

    if tokens.iter().any(|t| t == "hoy") {
        let epoch = today + 9 * 3600;
        return Some(ParsedDate {
            epoch,
            iso: format_epoch(epoch),
            raw: "hoy".into(),
            precision: "day".into(),
        });
    }
    let has_manana = tokens.iter().any(|t| t == "manana");
    if has_manana {
        let pasado = tokens.iter().any(|t| t == "pasado");
        let offset = if pasado { 2 } else { 1 };
        let epoch = today + offset * day + 9 * 3600;
        return Some(ParsedDate {
            epoch,
            iso: format_epoch(epoch),
            raw: if pasado { "pasado mañana".into() } else { "mañana".into() },
            precision: "day".into(),
        });
    }
    // "el lunes", "la próxima semana", "la semana que viene"
    for (i, wd) in WEEKDAYS_ES.iter().enumerate() {
        if tokens.iter().any(|t| t == wd) {
            let current = ((now.div_euclid(86_400) + 3) % 7) as i64; // 0 = lunes
            let target = i as i64;
            let mut delta = (target - current + 7) % 7;
            if delta == 0 {
                delta = 7;
            }
            let epoch = today + delta * day + 9 * 3600;
            return Some(ParsedDate {
                epoch,
                iso: format_epoch(epoch),
                raw: format!("el {}", wd),
                precision: "day".into(),
            });
        }
    }
    if norm.contains("semana que viene") || norm.contains("proxima semana") {
        let epoch = today + 7 * day + 9 * 3600;
        return Some(ParsedDate {
            epoch,
            iso: format_epoch(epoch),
            raw: "la semana que viene".into(),
            precision: "day".into(),
        });
    }
    None
}

/// ¿La cita es próxima? (hoy o más adelante, según la fecha actual).
pub fn is_upcoming(date: Option<i64>, now: i64) -> bool {
    date.map(|e| e >= start_of_day(now)).unwrap_or(false)
}

/// Extrae la fecha de un mensaje en español.
///
/// Acepta `dd/mm/aaaa`, `aaaa-mm-dd`, `12 de marzo de 2025`, `mañana`,
/// `el lunes`…  `now` es el epoch de referencia (para el año implícito).
pub fn parse_appointment_date(text: &str, now: i64) -> Option<ParsedDate> {
    parse_numeric_date(text, now)
        .or_else(|| parse_long_date(text, now))
        .or_else(|| parse_relative_date(text, now))
}

// ========================================================================= //
// 3. Base64 (puro)
// ========================================================================= //

/// Decodifica base64 (sin dependencias externas).
pub fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let mut out: Vec<u8> = Vec::with_capacity(input.len() / 4 * 3 + 3);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for c in input.bytes() {
        let v: u32 = match c {
            b'A'..=b'Z' => (c - b'A') as u32,
            b'a'..=b'z' => (c - b'a') as u32 + 26,
            b'0'..=b'9' => (c - b'0') as u32 + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b'\n' | b'\r' | b' ' | b'\t' => continue,
            _ => return None,
        };
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xFF) as u8);
        }
    }
    Some(out)
}

// ========================================================================= //
// 4. SMS / MMS (puro)
// ========================================================================= //

/// Un SMS (o parte de MMS) ya parseado.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SmsRecord {
    pub id: i64,
    pub address: String,
    pub body: String,
    /// Fecha en milisegundos desde epoch.
    pub date_ms: i64,
    /// Tipo del proveedor: 1 = bandeja de entrada, 2 = enviado, 3 = borrador…
    pub sms_type: i64,
    pub read: bool,
    pub thread_id: Option<i64>,
}

impl SmsRecord {
    /// Timestamp en segundos (el que usa el esquema de `sync.rs`).
    pub fn ts(&self) -> f64 {
        self.date_ms as f64 / 1000.0
    }

    /// `peer` si el mensaje es entrante, `user` si es saliente.
    pub fn role(&self) -> &'static str {
        if self.sms_type == 1 {
            "peer"
        } else {
            "user"
        }
    }
}

/// Extrae los campos `clave=valor` de una fila de `content query`.
///
/// Robusto frente a cuerpos con comas y saltos de línea: sólo se corta en
/// las claves conocidas, nunca en una coma del texto.
fn extract_fields(row: &str, keys: &[&str]) -> HashMap<String, String> {
    let mut positions: Vec<(usize, &str)> = Vec::new();
    for key in keys {
        let pat = format!("{}=", key);
        let mut from = 0usize;
        while let Some(p) = row[from..].find(pat.as_str()) {
            let abs = from + p;
            let prev_ok = abs == 0
                || !matches!(
                    row.as_bytes()[abs - 1],
                    b'_' | b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9'
                );
            if prev_ok {
                positions.push((abs, key));
            }
            from = abs + pat.len();
        }
    }
    positions.sort_by_key(|(p, _)| *p);
    let mut out: HashMap<String, String> = HashMap::new();
    for (i, (pos, key)) in positions.iter().enumerate() {
        let vstart = pos + key.len() + 1;
        let vend = positions.get(i + 1).map(|(p, _)| *p).unwrap_or(row.len());
        if vstart > vend {
            continue;
        }
        let mut value = row[vstart..vend].trim().to_string();
        if value.ends_with(',') {
            value.pop();
            value = value.trim_end().to_string();
        }
        out.insert((*key).to_string(), value);
    }
    out
}

/// Convierte la salida de `content query` en registros de SMS.
///
/// Formato esperado del proveedor:
///
/// ```text
/// Row: 0 _id=3, address=+34600111222, body=hola, date=1712345678000, type=1, read=1
/// ```
pub fn parse_sms_rows(raw: &str) -> Vec<SmsRecord> {
    const KEYS: &[&str] = &[
        "_id",
        "address",
        "body",
        "date",
        "type",
        "read",
        "thread_id",
        "person",
        "subject",
        "service_center",
    ];
    let mut starts: Vec<usize> = Vec::new();
    let mut from = 0usize;
    while let Some(p) = raw[from..].find("Row: ") {
        starts.push(from + p);
        from = from + p + 5;
    }
    let mut out = Vec::new();
    for (i, s) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(raw.len());
        if end <= *s {
            continue;
        }
        let row = &raw[*s..end];
        let f = extract_fields(row, KEYS);
        if f.is_empty() {
            continue;
        }
        let body = f.get("body").cloned().unwrap_or_default();
        if body.trim().is_empty() && !f.contains_key("address") {
            continue;
        }
        let mut date_ms = f
            .get("date")
            .and_then(|d| d.trim().parse::<i64>().ok())
            .unwrap_or(0);
        // Algunos proveedores dan segundos en vez de milisegundos.
        if date_ms > 0 && date_ms < 100_000_000_000 {
            date_ms *= 1000;
        }
        out.push(SmsRecord {
            id: f.get("_id").and_then(|v| v.trim().parse::<i64>().ok()).unwrap_or(0),
            address: f.get("address").cloned().unwrap_or_default().trim().to_string(),
            body,
            date_ms,
            sms_type: f.get("type").and_then(|v| v.trim().parse::<i64>().ok()).unwrap_or(1),
            read: f
                .get("read")
                .map(|v| v.trim() == "1" || v.trim().eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            thread_id: f.get("thread_id").and_then(|v| v.trim().parse::<i64>().ok()),
        });
    }
    out
}

/// Agrupa los SMS por hilo (`address` + `thread_id`) y normaliza.
///
/// Devuelve una conversación por hilo con el esquema de `sync.rs`.
pub fn sms_conversations(records: Vec<SmsRecord>, limit: usize, names: &HashMap<String, String>) -> Vec<Value> {
    let limit = limit.clamp(1, 5000);
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<SmsRecord>> = HashMap::new();
    for r in records {
        let key = match r.thread_id {
            Some(t) => format!("thread_{}", t),
            None => format!("addr_{}", r.address),
        };
        if !groups.contains_key(&key) {
            order.push(key.clone());
        }
        groups.entry(key).or_default().push(r);
    }
    let mut out = Vec::new();
    for key in order {
        let mut recs = groups.remove(&key).unwrap_or_default();
        recs.sort_by(|a, b| a.date_ms.cmp(&b.date_ms));
        if recs.is_empty() {
            continue;
        }
        let title = recs
            .iter()
            .rev()
            .find_map(|r| names.get(&r.address).cloned())
            .unwrap_or_else(|| {
                recs
                    .iter()
                    .rev()
                    .map(|r| r.address.clone())
                    .find(|a| !a.is_empty())
                    .unwrap_or_else(|| key.clone())
            });
        let mut msgs: Vec<Value> = Vec::new();
        let mut medical = false;
        for r in recs.iter().rev().take(limit).collect::<Vec<_>>().into_iter().rev() {
            let kws = detect_medical_keywords(&r.body);
            if !kws.is_empty() {
                medical = true;
            }
            let mut m = json!({
                "role": r.role(),
                "content": r.body,
                "ts": r.ts(),
                "id": r.id.to_string(),
                "read": r.read,
            });
            if !kws.is_empty() {
                m["medical"] = json!(true);
                m["keywords"] = json!(kws);
            }
            msgs.push(m);
        }
        let updated_at = recs.iter().map(|r| r.ts()).fold(0.0f64, f64::max);
        let thread_id = recs.iter().rev().find_map(|r| r.thread_id);
        let address = recs
            .iter()
            .rev()
            .map(|r| r.address.clone())
            .find(|a| !a.is_empty())
            .unwrap_or_default();
        let mut conv = json!({
            "service": "sms",
            "id": format!("sms:{}", key),
            "title": title,
            "updated_at": updated_at,
            "messages": msgs,
            "medical": medical,
            "source": "content://sms",
            "address": address,
        });
        if let Some(t) = thread_id {
            conv["thread_id"] = json!(t);
        }
        out.push(conv);
    }
    out
}

/// Extrae las partes de texto de los MMS (`content://mms/part`).
///
/// `mid` es el identificador del mensaje al que pertenece la parte.
pub fn parse_mms_parts(raw: &str) -> HashMap<i64, String> {
    const KEYS: &[&str] = &["mid", "seq", "ct", "text", "_id"];
    let mut starts: Vec<usize> = Vec::new();
    let mut from = 0usize;
    while let Some(p) = raw[from..].find("Row: ") {
        starts.push(from + p);
        from = from + p + 5;
    }
    let mut out: HashMap<i64, String> = HashMap::new();
    for (i, s) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(raw.len());
        if end <= *s {
            continue;
        }
        let f = extract_fields(&raw[*s..end], KEYS);
        let mid = match f.get("mid").and_then(|v| v.trim().parse::<i64>().ok()) {
            Some(m) => m,
            None => continue,
        };
        let ct = f.get("ct").cloned().unwrap_or_default();
        if !ct.starts_with("text/plain") {
            continue;
        }
        let text = f.get("text").cloned().unwrap_or_default();
        if text.trim().is_empty() {
            continue;
        }
        out.entry(mid)
            .and_modify(|acc| {
                acc.push(' ');
                acc.push_str(&text);
            })
            .or_insert(text);
    }
    out
}

// ========================================================================= //
// 5. SQLite: utilidades compartidas
// ========================================================================= //

/// Cita un identificador (tabla o columna) de forma segura.
fn ident(name: &str) -> String {
    let clean: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    format!("\"{}\"", clean)
}

/// Abre una BD en modo sólo lectura (con respaldo a lectura-escritura sobre
/// la **copia**, nunca sobre el original).
fn open_snapshot(path: &Path) -> Result<(Connection, String), String> {
    let ro = rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY;
    match Connection::open_with_flags(path, ro) {
        Ok(c) => Ok((c, "read_only".to_string())),
        Err(e) => {
            // Una BD en WAL necesita poder crear el -shm: se reintenta en
            // lectura-escritura, pero siempre sobre la copia local.
            match Connection::open(path) {
                Ok(c) => Ok((c, "read_write_on_copy".to_string())),
                Err(e2) => Err(format!("{} / {}", e, e2)),
            }
        }
    }
}

/// Tablas (y vistas) de una BD.
pub fn table_names(conn: &Connection) -> Vec<String> {
    let sql = "SELECT name FROM sqlite_master WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%'";
    let Ok(mut st) = conn.prepare(sql) else {
        return Vec::new();
    };
    let Ok(rows) = st.query_map([], |row| row.get::<_, String>(0)) else {
        return Vec::new();
    };
    rows.flatten().collect()
}

/// Columnas de una tabla.
pub fn table_columns(conn: &Connection, table: &str) -> Vec<String> {
    let sql = format!("PRAGMA table_info({})", ident(table));
    let Ok(mut st) = conn.prepare(&sql) else {
        return Vec::new();
    };
    let Ok(rows) = st.query_map([], |row| row.get::<_, String>(1)) else {
        return Vec::new();
    };
    rows.flatten().collect()
}

/// Elige la primera columna que coincide (exacta, sin distinguir mayúsculas).
fn pick_col(cols: &[String], candidates: &[&str]) -> Option<String> {
    for c in candidates {
        if let Some(found) = cols.iter().find(|x| x.eq_ignore_ascii_case(c)) {
            return Some(found.clone());
        }
    }
    None
}

/// Elige la primera columna que *contiene* alguno de los candidatos.
///
/// Sólo se usa para columnas cuyo nombre es lo bastante específico (texto,
/// fecha, título): para identificadores sería demasiado ambiguo.
fn pick_col_loose(cols: &[String], candidates: &[&str]) -> Option<String> {
    for c in candidates {
        if let Some(found) = cols.iter().find(|x| x.to_lowercase().contains(&c.to_lowercase())) {
            return Some(found.clone());
        }
    }
    None
}

/// Valor de una celda como texto (los blobs se pasan por el descodificador).
fn ref_text(v: rusqlite::types::ValueRef<'_>) -> Option<String> {
    use rusqlite::types::ValueRef;
    match v {
        ValueRef::Null => None,
        ValueRef::Integer(i) => Some(i.to_string()),
        ValueRef::Real(f) => Some(f.to_string()),
        ValueRef::Text(t) => Some(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => decode_message_blob(b),
    }
}

/// Valor de una celda como entero.
fn ref_i64(v: rusqlite::types::ValueRef<'_>) -> Option<i64> {
    use rusqlite::types::ValueRef;
    match v {
        ValueRef::Integer(i) => Some(i),
        ValueRef::Real(f) => Some(f as i64),
        ValueRef::Text(t) => String::from_utf8_lossy(t).trim().parse::<i64>().ok(),
        _ => None,
    }
}

/// ¿El texto parece texto legible (y no bytes arbitrarios)?
pub fn is_probably_text(s: &str) -> bool {
    if s.trim().is_empty() {
        return false;
    }
    let total = s.chars().count();
    let printable = s
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t' || *c == '\r')
        .count();
    let letters = s.chars().filter(|c| c.is_alphabetic()).count();
    printable * 100 >= total * 80 && letters >= 2
}

/// Decodifica el texto de un mensaje almacenado como blob.
///
/// Los clientes oficiales guardan el mensaje serializado en TL (binario).
/// Se intenta primero UTF-8 plano; si no, se recorre el blob buscando
/// cadenas con prefijo de longitud (formato TL) y se escoge la más
/// plausible.  Es una **heurística**: puede fallar, y por eso el extractor
/// informa del `text_source` usado.
pub fn decode_message_blob(blob: &[u8]) -> Option<String> {
    if blob.is_empty() {
        return None;
    }
    if let Ok(s) = std::str::from_utf8(blob) {
        let clean = s.trim_matches(|c: char| c.is_control() && c != '\n' && c != '\t' && c != '\r');
        if is_probably_text(clean) {
            return Some(clean.trim().to_string());
        }
    }
    // Barrido TL: longitud en 1 byte (1..253) o 254 + 3 bytes.
    let mut candidates: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < blob.len() {
        let b = blob[i];
        let (len, vstart) = if b == 254 && i + 4 <= blob.len() {
            (
                blob[i + 1] as usize | (blob[i + 2] as usize) << 8 | (blob[i + 3] as usize) << 16,
                i + 4,
            )
        } else if (1..=253).contains(&b) {
            (b as usize, i + 1)
        } else {
            i += 1;
            continue;
        };
        if len > 0 && vstart + len <= blob.len() {
            if let Ok(s) = std::str::from_utf8(&blob[vstart..vstart + len]) {
                let t = s.trim_matches(|c: char| c.is_control() && c != '\n' && c != '\t');
                if is_probably_text(t) {
                    candidates.push(t.trim().to_string());
                }
            }
        }
        i = vstart.max(i + 1);
    }
    candidates
        .iter()
        .filter(|c| c.chars().filter(|x| x.is_whitespace()).count() >= 1)
        .max_by_key(|c| c.chars().count())
        .cloned()
        .or_else(|| candidates.into_iter().max_by_key(|c| c.chars().count()))
}

// ========================================================================= //
// 6. Telegram (puro: lee una BD local)
// ========================================================================= //

/// Esquema detectado de la BD de Telegram.
#[derive(Debug, Clone, Default)]
pub struct TelegramSchema {
    pub tables: Vec<String>,
    pub messages_table: Option<String>,
    pub chats_table: Option<String>,
    pub users_table: Option<String>,
    pub info_table: Option<String>,
    pub message_columns: Vec<String>,
    pub chat_columns: Vec<String>,
    pub id_col: Option<String>,
    pub chat_id_col: Option<String>,
    pub from_id_col: Option<String>,
    pub date_col: Option<String>,
    pub text_col: Option<String>,
    pub title_col: Option<String>,
}

/// Detecta el esquema real de la BD de Telegram (`sqlite_master` + PRAGMA).
pub fn detect_telegram_schema(conn: &Connection) -> TelegramSchema {
    let tables = table_names(conn);
    let find = |cands: &[&str]| -> Option<String> {
        for c in cands {
            if let Some(t) = tables.iter().find(|x| x.eq_ignore_ascii_case(c)) {
                return Some(t.clone());
            }
        }
        None
    };
    let messages_table = find(&[
        "messages",
        "message",
        "msg_store",
        "messages_v2",
        "chat_messages",
    ]);
    let chats_table = find(&["chats", "chat", "dialogs", "conversations", "dialog"]);
    let users_table = find(&["users", "user"]);
    let info_table = find(&["info", "settings", "preferences", "app_info"]);

    let message_columns = messages_table
        .as_ref()
        .map(|t| table_columns(conn, t))
        .unwrap_or_default();
    let chat_columns = chats_table
        .as_ref()
        .map(|t| table_columns(conn, t))
        .unwrap_or_default();

    let id_col = pick_col(
        &message_columns,
        &["uid", "id", "_id", "msg_id", "message_id", "rowid"],
    );
    let chat_id_col = pick_col(
        &message_columns,
        &["chat_id", "dialog_id", "peer_id", "conversation_id", "chatid", "chat"],
    )
    .or_else(|| {
        message_columns
            .iter()
            .find(|c| {
                let l = c.to_lowercase();
                l.ends_with("_id") && (l.contains("chat") || l.contains("dialog") || l.contains("peer"))
            })
            .cloned()
    });
    let from_id_col = pick_col(
        &message_columns,
        &["from_id", "sender_id", "from_uid", "user_id", "from", "sender", "out"],
    );
    let date_col = pick_col(
        &message_columns,
        &["date", "msg_date", "timestamp", "time", "date_created", "created_at", "message_date"],
    )
    .or_else(|| pick_col_loose(&message_columns, &["date", "time"]));
    let text_col = pick_col(
        &message_columns,
        &["message", "text", "body", "message_text", "message_body", "data", "content"],
    )
    .or_else(|| pick_col_loose(&message_columns, &["text", "message", "body"]));
    let title_col = pick_col(
        &chat_columns,
        &["title", "name", "display_name", "chat_name", "subject", "description"],
    )
    .or_else(|| pick_col_loose(&chat_columns, &["title", "name"]));

    TelegramSchema {
        tables,
        messages_table,
        chats_table,
        users_table,
        info_table,
        message_columns,
        chat_columns,
        id_col,
        chat_id_col,
        from_id_col,
        date_col,
        text_col,
        title_col,
    }
}

/// Busca el identificador del propio usuario en la tabla `info`.
///
/// La tabla `info` guarda pares clave/valor (`user_id`, `self_id`…).
pub fn self_id_from_info(conn: &Connection, schema: &TelegramSchema) -> Option<i64> {
    let table = schema.info_table.as_ref()?;
    let cols = table_columns(conn, table);
    if cols.is_empty() {
        return None;
    }
    // Formato clave/valor.
    if let (Some(k), Some(v)) = (
        pick_col(&cols, &["key", "name", "k"]),
        pick_col(&cols, &["value", "val", "v"]),
    ) {
        let sql = format!(
            "SELECT {} FROM {} WHERE {} LIKE '%user_id%' OR {} LIKE '%self_id%' LIMIT 1",
            ident(&v),
            ident(table),
            ident(&k),
            ident(&k)
        );
        if let Ok(mut st) = conn.prepare(&sql) {
            if let Ok(mut rows) = st.query_map([], |row| row.get::<_, String>(0)) {
                if let Some(Ok(val)) = rows.next() {
                    if let Ok(n) = val.trim().parse::<i64>() {
                        return Some(n);
                    }
                }
            }
        }
    }
    // Formato tabla con una columna tipo "user_id".
    if let Some(c) = pick_col_loose(&cols, &["user_id", "self_id", "uid"]) {
        let sql = format!("SELECT {} FROM {} LIMIT 1", ident(&c), ident(table));
        if let Ok(mut st) = conn.prepare(&sql) {
            if let Ok(mut rows) =
                st.query_map([], |row| Ok(row.get_ref(0).ok().and_then(ref_i64)))
            {
                if let Some(Ok(n)) = rows.next() {
                    return n;
                }
            }
        }
    }
    None
}

/// Busca el identificador del propio usuario en la tabla `users`.
pub fn self_id_from_users(conn: &Connection, schema: &TelegramSchema) -> Option<i64> {
    let table = schema.users_table.as_ref()?;
    let cols = table_columns(conn, table);
    let id_col = pick_col(&cols, &["user_id", "uid", "id", "_id"])?;
    let flag_col = pick_col(&cols, &["is_self", "self", "is_me", "me", "own"]);
    let sql = match &flag_col {
        Some(f) => format!(
            "SELECT {} FROM {} WHERE {} = 1 LIMIT 1",
            ident(&id_col),
            ident(table),
            ident(f)
        ),
        None => format!("SELECT {} FROM {} LIMIT 1", ident(&id_col), ident(table)),
    };
    if let Ok(mut st) = conn.prepare(&sql) {
        if let Ok(mut rows) = st.query_map([], |row| Ok(row.get_ref(0).ok().and_then(ref_i64))) {
            if let Some(Ok(n)) = rows.next() {
                return n;
            }
        }
    }
    None
}

/// Fila de mensaje ya normalizada desde la BD.
struct RawMessage {
    id: Option<i64>,
    chat_id: Option<i64>,
    from_id: Option<i64>,
    date: Option<i64>,
    text: Option<String>,
}

/// Extrae los mensajes de una BD de Telegram (copia local).
///
/// Devuelve el mismo esquema de conversación que usa `sync.rs`.
pub fn extract_telegram_db(path: &Path, limit: usize) -> Value {
    let limit = limit.clamp(1, 5000);
    let (conn, mode) = match open_snapshot(path) {
        Ok(v) => v,
        Err(e) => {
            return json!({
                "ok": false,
                "service": "telegram",
                "error": "open_failed",
                "detail": e,
            })
        }
    };
    let schema = detect_telegram_schema(&conn);
    let Some(messages_table) = schema.messages_table.clone() else {
        return json!({
            "ok": false,
            "service": "telegram",
            "error": "no_messages_table",
            "schema_detected": { "tables": schema.tables },
        });
    };

    // Nombres de chat desde la tabla de chats/dialogs.
    let mut titles: HashMap<i64, String> = HashMap::new();
    if let Some(chats_table) = schema.chats_table.as_ref() {
        let key_expr = pick_col(
            &schema.chat_columns,
            &["chat_id", "dialog_id", "peer_id", "_id", "id", "rowid"],
        )
        .map(|c| ident(&c))
        .unwrap_or_else(|| "\"rowid\"".to_string());
        let title_expr = schema
            .title_col
            .as_ref()
            .map(|c| ident(c))
            .unwrap_or_else(|| "NULL".to_string());
        let sql = format!(
            "SELECT {}, {} FROM {}",
            key_expr, title_expr, ident(chats_table)
        );
        if let Ok(mut st) = conn.prepare(&sql) {
            if let Ok(mut rows) = st.query_map([], |row| {
                // Los valores se convierten aquí: `ValueRef` no puede salir
                // del closure (préstamo de la fila).
                let k = row.get_ref(0).ok().and_then(ref_i64);
                let t = row.get_ref(1).ok().and_then(ref_text);
                Ok::<(Option<i64>, Option<String>), rusqlite::Error>((k, t))
            }) {
                while let Some(Ok((id, name))) = rows.next() {
                    if let Some(name) = name {
                        if !name.trim().is_empty() {
                            titles.insert(id.unwrap_or(0), name);
                        }
                    }
                }
            }
        }
    }

    // Filas de mensajes.
    let id_expr = schema
        .id_col
        .as_ref()
        .map(|c| ident(c))
        .unwrap_or_else(|| "NULL".into());
    let chat_expr = schema
        .chat_id_col
        .as_ref()
        .map(|c| ident(c))
        .unwrap_or_else(|| "NULL".into());
    let from_expr = schema
        .from_id_col
        .as_ref()
        .map(|c| ident(c))
        .unwrap_or_else(|| "NULL".into());
    let date_expr = schema
        .date_col
        .as_ref()
        .map(|c| ident(c))
        .unwrap_or_else(|| "NULL".into());
    let text_expr = schema
        .text_col
        .as_ref()
        .map(|c| ident(c))
        .unwrap_or_else(|| "NULL".into());
    let order_expr = schema
        .date_col
        .as_ref()
        .map(|c| ident(c))
        .unwrap_or_else(|| "\"rowid\"".into());
    let sql = format!(
        "SELECT {}, {}, {}, {}, {} FROM {} ORDER BY {} DESC LIMIT ?",
        id_expr,
        chat_expr,
        from_expr,
        date_expr,
        text_expr,
        ident(&messages_table),
        order_expr
    );
    let fetch = (limit * 20).clamp(50, 20_000);
    let mut rows_raw: Vec<RawMessage> = Vec::new();
    if let Ok(mut st) = conn.prepare(&sql) {
        if let Ok(mut rows) = st.query_map([fetch as i64], |row| {
            Ok(RawMessage {
                id: row.get_ref(0).ok().and_then(ref_i64),
                chat_id: row.get_ref(1).ok().and_then(ref_i64),
                from_id: row.get_ref(2).ok().and_then(ref_i64),
                date: row.get_ref(3).ok().and_then(ref_i64),
                text: row.get_ref(4).ok().and_then(ref_text),
            })
        }) {
            while let Some(Ok(r)) = rows.next() {
                rows_raw.push(r);
            }
        }
    }

    // Identificador propio: primero la tabla `info`, luego `users`, y si no
    // se infiere por patrón (el remitente más frecuente es el usuario:
    // es el que más mensajes envía).
    let (self_id, self_source) = match self_id_from_info(&conn, &schema) {
        Some(v) => (Some(v), "info".to_string()),
        None => match self_id_from_users(&conn, &schema) {
            Some(v) => (Some(v), "users".to_string()),
            None => {
                let mut counts: HashMap<i64, usize> = HashMap::new();
                for r in &rows_raw {
                    if let Some(f) = r.from_id {
                        *counts.entry(f).or_insert(0) += 1;
                    }
                }
                let best = counts
                    .iter()
                    .max_by_key(|(k, v)| (**v, -(**k)))
                    .map(|(k, _)| *k);
                (best, "pattern".to_string())
            }
        },
    };

    // Agrupar por chat.
    let mut order: Vec<i64> = Vec::new();
    let mut groups: HashMap<i64, Vec<&RawMessage>> = HashMap::new();
    for r in rows_raw.iter() {
        let chat = r.chat_id.unwrap_or(0);
        if !groups.contains_key(&chat) {
            order.push(chat);
        }
        groups.entry(chat).or_default().push(r);
    }

    let mut conversations: Vec<Value> = Vec::new();
    let mut total_messages = 0usize;
    for chat_id in order {
        let mut msgs_in = groups.remove(&chat_id).unwrap_or_default();
        msgs_in.sort_by_key(|r| r.date.unwrap_or(0));
        let start = msgs_in.len().saturating_sub(limit);
        let msgs: Vec<&RawMessage> = msgs_in[start..].to_vec();
        let mut out_msgs: Vec<Value> = Vec::new();
        let mut updated_at = 0.0f64;
        for r in msgs.iter() {
            let ts = r.date.unwrap_or(0) as f64;
            updated_at = updated_at.max(ts);
            let role = match (r.from_id, self_id) {
                (Some(f), Some(s)) if f == s => "user",
                _ => "peer",
            };
            let content = r.text.clone().unwrap_or_default();
            let mut m = json!({
                "role": role,
                "content": content,
                "ts": ts,
            });
            if let Some(id) = r.id {
                m["id"] = json!(id.to_string());
            }
            let kws = detect_medical_keywords(r.text.as_deref().unwrap_or(""));
            if !kws.is_empty() {
                m["medical"] = json!(true);
                m["keywords"] = json!(kws);
            }
            out_msgs.push(m);
        }
        total_messages += out_msgs.len();
        let title = titles
            .get(&chat_id)
            .cloned()
            .unwrap_or_else(|| format!("chat {}", chat_id));
        conversations.push(json!({
            "service": "telegram",
            "id": format!("telegram:chat_{}", chat_id),
            "title": title,
            "updated_at": updated_at,
            "messages": out_msgs,
            "source": path.to_string_lossy(),
        }));
    }

    json!({
        "ok": true,
        "service": "telegram",
        "open_mode": mode,
        "schema_detected": {
            "tables": schema.tables,
            "messages_table": schema.messages_table,
            "messages_columns": schema.message_columns,
            "chats_table": schema.chats_table,
            "chats_columns": schema.chat_columns,
            "users_table": schema.users_table,
            "info_table": schema.info_table,
            "text_column": schema.text_col,
            "date_column": schema.date_col,
            "from_column": schema.from_id_col,
        },
        "self_id": self_id,
        "self_id_source": self_source,
        "conversations": conversations,
        "messages": total_messages,
    })
}

// ========================================================================= //
// 7. WhatsApp (puro)
// ========================================================================= //

/// Extrae contactos y chats de `wa.db` (que **no** está cifrado).
///
/// El esquema de `wa.db` cambia entre versiones de WhatsApp, así que se
/// detectan las tablas por nombre de columna y se informa del esquema real.
pub fn extract_whatsapp_db(path: &Path, limit: usize) -> Value {
    let limit = limit.clamp(1, 5000);
    let (conn, mode) = match open_snapshot(path) {
        Ok(v) => v,
        Err(e) => {
            return json!({
                "ok": false,
                "service": "whatsapp",
                "error": "open_failed",
                "detail": e,
            })
        }
    };
    let tables = table_names(&conn);
    let mut schema: Vec<Value> = Vec::new();
    let mut contacts_table: Option<String> = None;
    let mut chats_table: Option<String> = None;

    for t in &tables {
        let cols = table_columns(&conn, t);
        schema.push(json!({ "name": t, "columns": cols }));
        let has_jid = cols.iter().any(|c| {
            let l = c.to_lowercase();
            l == "jid" || l.contains("jid") || l.contains("number") || l == "msisdn" || l.contains("recipient")
        });
        let has_name = cols.iter().any(|c| {
            let l = c.to_lowercase();
            l == "name" || l.contains("display_name") || l.contains("push_name") || l.contains("first_name")
        });
        let has_chat = cols.iter().any(|c| {
            let l = c.to_lowercase();
            l.contains("unread") || l.contains("message_count") || l.contains("chat_row")
        });
        if has_jid && has_name && contacts_table.is_none() {
            contacts_table = Some(t.clone());
        }
        if has_jid && has_chat && chats_table.is_none() {
            chats_table = Some(t.clone());
        }
    }

    let contacts = contacts_table
        .as_ref()
        .map(|t| extract_rows_as_json(&conn, t, limit))
        .unwrap_or_default();
    let chats = chats_table
        .as_ref()
        .map(|t| extract_rows_as_json(&conn, t, limit))
        .unwrap_or_default();

    json!({
        "ok": true,
        "service": "whatsapp",
        "open_mode": mode,
        "schema_detected": { "tables": schema },
        "contacts_table": contacts_table,
        "chats_table": chats_table,
        "contacts": contacts,
        "chats": chats,
    })
}

/// Vuelca las filas de una tabla como array de objetos JSON.
fn extract_rows_as_json(conn: &Connection, table: &str, limit: usize) -> Vec<Value> {
    let sql = format!("SELECT * FROM {} LIMIT {}", ident(table), limit);
    let Ok(mut st) = conn.prepare(&sql) else {
        return Vec::new();
    };
    let cols: Vec<String> = st
        .column_names()
        .iter()
        .map(|s| s.to_string())
        .collect();
    let Ok(mut rows) = st.query_map([], |row| {
        let mut obj = serde_json::Map::new();
        for (i, c) in cols.iter().enumerate() {
            let v = row.get_ref(i)?;
            let val = match v {
                rusqlite::types::ValueRef::Null => Value::Null,
                rusqlite::types::ValueRef::Integer(i) => json!(i),
                rusqlite::types::ValueRef::Real(f) => json!(f),
                rusqlite::types::ValueRef::Text(t) => {
                    json!(String::from_utf8_lossy(t).chars().take(2000).collect::<String>())
                }
                rusqlite::types::ValueRef::Blob(b) => {
                    json!(String::from_utf8_lossy(b).chars().take(2000).collect::<String>())
                }
            };
            obj.insert(c.clone(), val);
        }
        Ok::<Value, rusqlite::Error>(Value::Object(obj))
    }) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    while let Some(Ok(r)) = rows.next() {
        out.push(r);
    }
    out
}

/// Notificación del sistema (visible, **no** cifrada).
#[derive(Debug, Clone, Default)]
pub struct NotificationRecord {
    pub package: String,
    pub title: String,
    pub text: String,
    pub ts: Option<i64>,
}

/// Extrae las notificaciones push de `dumpsys notification --noredact`.
///
/// El texto de una notificación push **no** va cifrado (viaja desde el
/// servidor de WhatsApp al sistema), así que es legible sin inyectarse.
pub fn parse_notification_dump(text: &str, packages: &[&str]) -> Vec<NotificationRecord> {
    let mut out: Vec<NotificationRecord> = Vec::new();
    let mut current: Option<NotificationRecord> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("NotificationRecord(") || trimmed.starts_with("Notification(0x") {
            if let Some(rec) = current.take() {
                out.push(rec);
            }
            let pkg = extract_between(trimmed, "pkg=", ' ').unwrap_or_default();
            current = Some(NotificationRecord {
                package: pkg,
                ..Default::default()
            });
            continue;
        }
        if trimmed.starts_with("extras=") {
            continue;
        }
        if trimmed.starts_with("android.") {
            let (key, value) = match trimmed.find('=') {
                Some(p) => (trimmed[..p].to_string(), trimmed[p + 1..].trim().to_string()),
                None => continue,
            };
            let value = clean_dumpsys_value(&value);
            let Some(rec) = current.as_mut() else { continue };
            match key.as_str() {
                "android.title" => rec.title = value,
                "android.text" | "android.bigText" => {
                    if rec.text.is_empty() {
                        rec.text = value;
                    }
                }
                _ => {}
            }
            continue;
        }
        if trimmed.starts_with("postTime=") {
            let raw = trimmed["postTime=".len()..].trim().to_string();
            if let Some(rec) = current.as_mut() {
                rec.ts = parse_dumpsys_posttime(&raw);
            }
            continue;
        }
    }
    if let Some(rec) = current.take() {
        out.push(rec);
    }
    out.into_iter()
        .filter(|r| packages.iter().any(|p| r.package == *p))
        .collect()
}

/// Limpia el valor de un extra de `dumpsys` (`String (texto)` → `texto`).
fn clean_dumpsys_value(v: &str) -> String {
    let v = v.trim();
    if let Some(rest) = v.strip_prefix("String (") {
        return rest.trim_end_matches(')').to_string();
    }
    if let Some(rest) = v.strip_prefix("CharSequence (") {
        return rest.trim_end_matches(')').to_string();
    }
    v.trim_end_matches(')').trim().to_string()
}

/// Extrae el texto entre `pre` y el carácter `stop`.
fn extract_between(s: &str, pre: &str, stop: char) -> Option<String> {
    let p = s.find(pre)? + pre.len();
    let rest = &s[p..];
    let end = rest.find(stop).unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}

/// Interpreta `postTime=09-15 12:34:56.789` (año actual, UTC).
fn parse_dumpsys_posttime(raw: &str) -> Option<i64> {
    let raw = raw.trim();
    let (date_part, time_part) = match raw.find(' ') {
        Some(p) => (&raw[..p], &raw[p + 1..]),
        None => (raw, ""),
    };
    let mut dit = date_part.split('-');
    let month: u32 = dit.next()?.parse().ok()?;
    let day: u32 = dit.next()?.parse().ok()?;
    let time_part = time_part.split('.').next().unwrap_or("");
    let mut tit = time_part.split(':');
    let hour: u32 = tit.next()?.parse().ok()?;
    let minute: u32 = tit.next().unwrap_or("0").parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let now = now_epoch();
    let (year, _, _) = civil_from_days(now.div_euclid(86_400));
    Some(make_epoch(year, month, day, hour, minute))
}

// ========================================================================= //
// 8. Capa de dispositivo (root_exec / fs_read)
// ========================================================================= //

/// Ejecuta una orden como root en el dispositivo.
fn exec(command: &str, timeout_ms: u64) -> crate::android::ExecResult {
    crate::android::root_exec(command, timeout_ms, 256 * 1024)
}

/// Si la llamada al dispositivo falló (sin root, sin `su`, sin permiso),
/// devuelve el error estructurado en lugar de un resultado vacío.
fn device_error(r: &crate::android::ExecResult) -> Option<Value> {
    let err = r.stderr.trim();
    if err.contains("no se pudo ejecutar")
        || err.contains("Permission denied")
        || err.contains("not found")
        || err.contains("Permission Denial")
    {
        return Some(json!({
            "ok": false,
            "error": "device_unavailable",
            "detail": err,
            "hint": "hace falta un dispositivo con root accesible por 'su -mm -c'",
        }));
    }
    None
}

/// Lee un fichero del dispositivo en binario (vía `fs_read` + base64).
fn read_remote_bytes(path: &str, max_bytes: usize) -> Option<Vec<u8>> {
    let r = crate::android::fs_read(path, true, max_bytes);
    if r.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        return None;
    }
    let content = r.get("content").and_then(|v| v.as_str())?;
    decode_base64(content)
}

/// Directorios donde se dejan los snapshots (el primero es global).
fn snapshot_dirs() -> Vec<String> {
    let mut v = vec![SNAPSHOT_DIR.to_string()];
    let local = format!("{}/messaging_snap", crate::cockpit_server::state_dir());
    if local != SNAPSHOT_DIR {
        v.push(local);
    }
    v
}

/// Copia una BD del dispositivo (con su `-wal` y su `-shm`) a un snapshot.
///
/// Se copia porque las apps escriben en caliente: leer el original en vivo
/// puede dar un snapshot incoherente (o un "database is locked").
fn snapshot_remote(remote: &str, tag: &str) -> Option<(PathBuf, String)> {
    let base = remote.rsplit('/').next().unwrap_or("db");
    let safe_base: String = base
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '_' { c } else { '_' })
        .collect();
    for dir in snapshot_dirs() {
        let dest = format!("{}/{}_{}", dir, tag, safe_base);
        let mut script = format!(
            "mkdir -p {} 2>/dev/null; cp -f {} {} 2>/dev/null; chmod 644 {} 2>/dev/null; ",
            crate::android::sh_quote(&dir),
            crate::android::sh_quote(remote),
            crate::android::sh_quote(&dest),
            crate::android::sh_quote(&dest)
        );
        for suffix in ["-wal", "-shm"] {
            script.push_str(&format!(
                "cp -f {} {} 2>/dev/null; ",
                crate::android::sh_quote(&format!("{}{}", remote, suffix)),
                crate::android::sh_quote(&format!("{}{}", dest, suffix))
            ));
        }
        script.push_str(&format!(
            "stat -c %s {} 2>/dev/null || echo 0",
            crate::android::sh_quote(&dest)
        ));
        let r = exec(&script, 90_000);
        let size: i64 = r.stdout.trim().parse().unwrap_or(0);
        if size > 0 {
            return Some((PathBuf::from(dest), dir));
        }
    }
    None
}

/// Busca el primer fichero existente entre los candidatos.
fn device_find_first(paths: &[&str]) -> Option<String> {
    if paths.is_empty() {
        return None;
    }
    let list: Vec<String> = paths.iter().map(|p| crate::android::sh_quote(p)).collect();
    let script = format!(
        "for f in {}; do if [ -e \"$f\" ]; then echo \"$f\"; break; fi; done",
        list.join(" ")
    );
    let r = exec(&script, 30_000);
    let found = r.stdout.trim().to_string();
    if found.is_empty() {
        None
    } else {
        Some(found)
    }
}

/// Lista los ficheros de un directorio del dispositivo (nombre + tamaño).
fn device_list_dir(dir: &str) -> Vec<(String, i64)> {
    let script = format!(
        "for f in {}/*; do [ -e \"$f\" ] || continue; echo \"$(stat -c %s \"$f\" 2>/dev/null)|$f\"; done",
        crate::android::sh_quote(dir)
    );
    let r = exec(&script, 30_000);
    let mut out = Vec::new();
    for line in r.stdout.lines() {
        let mut it = line.splitn(2, '|');
        let size: i64 = it.next().unwrap_or("0").trim().parse().unwrap_or(0);
        let path = it.next().unwrap_or("").trim().to_string();
        if !path.is_empty() {
            out.push((path, size));
        }
    }
    out
}

/// Paquetes instalados que parecen de salud (Salud Responde,IMARY, …).
fn find_health_packages(installed: &[String]) -> Vec<String> {
    installed
        .iter()
        .filter(|p| {
            let l = p.to_lowercase();
            l.contains("salud") || l.contains("responde") || l.contains("sanidad") || l.contains("andsalud")
        })
        .cloned()
        .collect()
}

// ========================================================================= //
// 9. API pública
// ========================================================================= //

/// Inventario: qué apps de mensajería hay y qué almacenes existen.
pub fn inventory() -> Value {
    let r = exec("pm list packages 2>/dev/null", 60_000);
    let installed: Vec<String> = r
        .stdout
        .lines()
        .filter_map(|l| l.strip_prefix("package:").map(|s| s.trim().to_string()))
        .collect();
    if installed.is_empty() && !r.stderr.is_empty() {
        return json!({
            "ok": false,
            "error": "device_unavailable",
            "detail": r.stderr.trim(),
            "hint": "hace falta un dispositivo con root accesible por 'su -mm -c'",
        });
    }

    // Paquetes candidatos: los conocidos + los de salud detectados por patrón.
    let mut candidates: Vec<(String, String)> = Vec::new();
    for spec in APP_SPECS {
        if installed.iter().any(|p| p == spec.package) {
            candidates.push((spec.service.to_string(), spec.package.to_string()));
        }
    }
    for pkg in find_health_packages(&installed) {
        if !candidates.iter().any(|(_, p)| *p == pkg) {
            candidates.push(("salud_responde".to_string(), pkg));
        }
    }

    // APK + ficheros de datos, todo en una sola pasada al dispositivo.
    let mut script = String::new();
    for (_, pkg) in &candidates {
        script.push_str(&format!(
            "apk=$(pm path {p} 2>/dev/null | head -1 | cut -d: -f2); sz=0; [ -n \"$apk\" ] && sz=$(stat -c %s \"$apk\" 2>/dev/null); echo \"PKG|{p}|$apk|$sz\"; ",
            p = crate::android::sh_quote(pkg)
        ));
    }
    let mut all_candidates: Vec<String> = Vec::new();
    for spec in APP_SPECS {
        for c in spec.candidates {
            all_candidates.push((*c).to_string());
        }
    }
    for c in &all_candidates {
        script.push_str(&format!(
            "if [ -e {c} ]; then echo \"CAND|{c}|$(stat -c %s {c} 2>/dev/null)\"; else echo \"CAND|{c}|\"; fi; ",
            c = crate::android::sh_quote(c)
        ));
    }
    let out = exec(&script, 120_000);

    let mut apk_by_pkg: HashMap<String, (String, i64)> = HashMap::new();
    let mut cand_sizes: HashMap<String, i64> = HashMap::new();
    for line in out.stdout.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        match parts.first().copied() {
            Some("PKG") if parts.len() >= 4 => {
                apk_by_pkg.insert(
                    parts[1].to_string(),
                    (parts[2].to_string(), parts[3].trim().parse().unwrap_or(0)),
                );
            }
            Some("CAND") if parts.len() >= 3 => {
                cand_sizes.insert(
                    parts[1].to_string(),
                    parts[2].trim().parse().unwrap_or(0),
                );
            }
            _ => {}
        }
    }

    let mut apps: Vec<Value> = Vec::new();
    for (service, pkg) in &candidates {
        let (apk_path, apk_size) = apk_by_pkg
            .get(pkg)
            .cloned()
            .unwrap_or_else(|| (String::new(), 0));
        let data_dir = format!("/data/data/{}", pkg);
        let db_dir = format!("{}/databases", data_dir);
        let files: Vec<Value> = device_list_dir(&db_dir)
            .into_iter()
            .map(|(p, s)| json!({ "path": p, "size": s }))
            .collect();
        let spec_candidates: Vec<Value> = APP_SPECS
            .iter()
            .find(|s| s.package == pkg)
            .map(|s| {
                s.candidates
                    .iter()
                    .map(|c| {
                        json!({
                            "path": c,
                            "exists": cand_sizes.get(*c).copied().unwrap_or(0) > 0,
                            "size": cand_sizes.get(*c).copied().unwrap_or(0),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        apps.push(json!({
            "service": service,
            "package": pkg,
            "installed": true,
            "apk_path": apk_path,
            "apk_size": apk_size,
            "data_dir": data_dir,
            "databases_dir": db_dir,
            "data_files": files,
            "candidate_paths": spec_candidates,
        }));
    }

    json!({
        "ok": true,
        "packages_installed": installed.len(),
        "apps": apps,
        "health_packages": find_health_packages(&installed),
        "notes": [
            "Los almacenes de WhatsApp (msgstore.db) estan cifrados: wa.db si es legible.",
            "Telegram guarda msgstore.db en /data/data/org.telegram.messenger/databases.",
            "Los SMS se leen por content provider (content://sms), no por fichero.",
        ],
    })
}

/// Extrae los mensajes de Telegram.
pub fn telegram(limit: usize) -> Value {
    let limit = limit.clamp(1, 5000);
    let pkg_list = exec("pm list packages 2>/dev/null", 60_000);
    if let Some(e) = device_error(&pkg_list) {
        return e;
    }
    let mut candidates: Vec<String> = Vec::new();
    for c in APP_SPECS
        .iter()
        .find(|s| s.package == TELEGRAM_PACKAGE)
        .map(|s| s.candidates.to_vec())
        .unwrap_or_default()
    {
        candidates.push(c.to_string());
    }
    // Cualquier otra .db del directorio de bases de datos de Telegram.
    for (path, _) in device_list_dir("/data/data/org.telegram.messenger/databases") {
        if path.ends_with(".db") && !candidates.contains(&path) {
            candidates.push(path);
        }
    }
    let refs: Vec<&str> = candidates.iter().map(|s| s.as_str()).collect();
    let Some(remote) = device_find_first(&refs) else {
        return json!({
            "ok": false,
            "service": "telegram",
            "error": "db_not_found",
            "candidates": candidates,
            "hint": "instala Telegram o abre su chat una vez para que cree la base de datos",
        });
    };
    let Some((snap, dir)) = snapshot_remote(&remote, "telegram") else {
        return json!({
            "ok": false,
            "service": "telegram",
            "error": "snapshot_failed",
            "source": remote,
            "hint": "no se pudo copiar la base de datos al snapshot",
        });
    };
    let mut out = extract_telegram_db(&snap, limit);
    out["source"] = json!(remote);
    out["snapshot"] = json!(snap.to_string_lossy());
    out["snapshot_dir"] = json!(dir);
    out
}

/// Recoge los SMS del dispositivo (content provider) y los normaliza.
fn collect_sms_records() -> (Vec<SmsRecord>, Value) {
    let cmd = "content query --uri content://sms --projection _id:address:body:date:type:read:thread_id 2>&1";
    let r = exec(cmd, 90_000);
    let mut records = parse_sms_rows(&r.stdout);
    let mut meta = json!({
        "uri": "content://sms",
        "rows": records.len(),
        "stderr": r.stderr.trim(),
    });

    // MMS: las partes de texto van en content://mms/part.
    let mms = exec(
        "content query --uri content://mms --projection _id:thread_id:date:read 2>&1",
        60_000,
    );
    let parts = exec(
        "content query --uri content://mms/part --projection mid:seq:ct:text 2>&1",
        60_000,
    );
    let part_text = parse_mms_parts(&parts.stdout);
    if !part_text.is_empty() {
        let mut mms_records: Vec<SmsRecord> = Vec::new();
        for line in mms.stdout.lines() {
            if !line.contains("Row: ") {
                continue;
            }
            let f = extract_fields(line, &["_id", "thread_id", "date", "read"]);
            let id = f.get("_id").and_then(|v| v.trim().parse::<i64>().ok());
            let Some(id) = id else { continue };
            let Some(text) = part_text.get(&id) else { continue };
            let mut date_ms = f
                .get("date")
                .and_then(|d| d.trim().parse::<i64>().ok())
                .unwrap_or(0);
            if date_ms > 0 && date_ms < 100_000_000_000 {
                date_ms *= 1000;
            }
            mms_records.push(SmsRecord {
                id,
                address: String::new(),
                body: text.clone(),
                date_ms,
                sms_type: 1,
                read: f
                    .get("read")
                    .map(|v| v.trim() == "1")
                    .unwrap_or(false),
                thread_id: f.get("thread_id").and_then(|v| v.trim().parse::<i64>().ok()),
            });
        }
        meta["mms_rows"] = json!(mms_records.len());
        records.extend(mms_records);
    }
    (records, meta)
}

/// Nombres de contacto (opcional; mejora los títulos de los hilos).
///
/// Se indexa por **número**, que es como aparece el `address` en los SMS.
fn contact_names() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let r = exec(
        "content query --uri content://contacts --projection _id:display_name:number 2>&1",
        45_000,
    );
    if r.stdout.contains("No result found") {
        return map;
    }
    for line in r.stdout.lines() {
        if !line.contains("Row: ") {
            continue;
        }
        let f = extract_fields(line, &["_id", "display_name", "number"]);
        let name = f.get("display_name").cloned().unwrap_or_default();
        if name.trim().is_empty() {
            continue;
        }
        if let Some(number) = f.get("number") {
            if !number.trim().is_empty() {
                map.insert(number.trim().to_string(), name.clone());
            }
        }
        if let Some(id) = f.get("_id") {
            if !id.trim().is_empty() {
                map.insert(id.trim().to_string(), name);
            }
        }
    }
    map
}

/// Extrae los SMS/MMS y los normaliza al esquema de `sync.rs`.
pub fn sms(limit: usize) -> Value {
    let limit = limit.clamp(1, 5000);
    let (records, meta) = collect_sms_records();
    let stderr = meta["stderr"].as_str().unwrap_or("").to_string();
    if stderr.contains("no se pudo ejecutar") {
        return json!({
            "ok": false,
            "service": "sms",
            "error": "device_unavailable",
            "detail": stderr,
            "hint": "hace falta un dispositivo con root accesible por 'su -mm -c'",
        });
    }
    let names = contact_names();
    let conversations = sms_conversations(records.clone(), limit, &names);
    let medical_messages = records
        .iter()
        .filter(|r| is_medical_text(&r.body))
        .count();
    json!({
        "ok": true,
        "service": "sms",
        "messages": records.len(),
        "medical_messages": medical_messages,
        "conversations": conversations,
        "provider": meta,
    })
}

/// Extrae lo legible de WhatsApp: `wa.db`, notificaciones y el estado del
/// almacén cifrado.
pub fn whatsapp(limit: usize) -> Value {
    let limit = limit.clamp(1, 5000);
    let mut packages: Vec<&str> = Vec::new();
    let pm = exec("pm list packages 2>/dev/null", 60_000);
    if let Some(e) = device_error(&pm) {
        return e;
    }
    let installed_list: Vec<String> = pm
        .stdout
        .lines()
        .filter_map(|l| l.strip_prefix("package:").map(|s| s.trim().to_string()))
        .collect();
    for p in [WHATSAPP_PACKAGE, WHATSAPP_W4B_PACKAGE] {
        if installed_list.iter().any(|x| x == p) {
            packages.push(p);
        }
    }

    let mut db_info = json!({
        "wa_db": Value::Null,
        "msgstore": Value::Null,
    });
    let mut contacts: Vec<Value> = Vec::new();
    let mut chats: Vec<Value> = Vec::new();
    let mut schema_detected = json!({});

    for pkg in &packages {
        let data = format!("/data/data/{}", pkg);
        // wa.db: metadata de contactos y chats (NO está cifrado).
        let wa = format!("{}/databases/wa.db", data);
        if let Some((snap, _)) = snapshot_remote(&wa, "wa") {
            let ext = extract_whatsapp_db(&snap, limit);
            if ext.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                if let Some(arr) = ext["contacts"].as_array() {
                    contacts.extend(arr.iter().cloned());
                }
                if let Some(arr) = ext["chats"].as_array() {
                    chats.extend(arr.iter().cloned());
                }
                schema_detected = ext["schema_detected"].clone();
                db_info["wa_db"] = json!({
                    "package": pkg,
                    "path": wa,
                    "snapshot": snap.to_string_lossy(),
                    "open_mode": ext["open_mode"].clone(),
                    "readable": true,
                });
            }
        } else {
            db_info["wa_db"] = json!({
                "package": pkg,
                "path": wa,
                "readable": false,
                "note": "wa.db no encontrado o no copiable",
            });
        }

        // msgstore.db: el almacén de mensajes está CIFRADO.
        let msg = format!("{}/databases/msgstore.db", data);
        let crypt15 = format!("{}/databases/msgstore.db.crypt15", data);
        let exists = device_find_first(&[msg.as_str(), crypt15.as_str()]);
        let mut entry = json!({
            "package": pkg,
            "path": exists.clone().unwrap_or_default(),
            "exists": exists.is_some(),
        });
        if let Some(path) = exists.as_ref() {
            let size = device_list_dir(&format!("{}/databases", data))
                .into_iter()
                .find(|(p, _)| p == path)
                .map(|(_, s)| s)
                .unwrap_or(0);
            let magic = read_remote_bytes(path, 16)
                .map(|b| b.starts_with(b"SQLite format 3\0"))
                .unwrap_or(false);
            entry["size"] = json!(size);
            entry["sqlite_magic"] = json!(magic);
            entry["encrypted"] = json!(!magic);
        }
        db_info["msgstore"] = entry;
    }

    // Notificaciones: el texto push NO va cifrado.
    let notifications_raw = exec("dumpsys notification --noredact 2>/dev/null", 60_000);
    let notifs = parse_notification_dump(&notifications_raw.stdout, &packages);
    let notifications: Vec<Value> = notifs
        .iter()
        .take(limit)
        .map(|n| {
            let mut v = json!({
                "package": n.package,
                "title": n.title,
                "text": n.text,
            });
            if let Some(ts) = n.ts {
                v["ts"] = json!(ts as f64);
                v["post_time"] = json!(format_epoch(ts));
            }
            let kws = detect_medical_keywords(&format!("{} {}", n.title, n.text));
            if !kws.is_empty() {
                v["medical"] = json!(true);
                v["keywords"] = json!(kws);
            }
            v
        })
        .collect();

    json!({
        "ok": true,
        "service": "whatsapp",
        "installed": !packages.is_empty(),
        "packages": packages,
        "contacts": contacts,
        "chats": chats,
        "notifications": notifications,
        "stores": db_info,
        "schema_detected": schema_detected,
        "encrypted_messages_unreadable": true,
        "reason": WHATSAPP_ENCRYPTION_REASON,
        "how_to_read_messages": [
            "Inyectar en el proceso de WhatsApp con Frida y hooked la clave del keystore.",
            "Volcar la memoria del proceso (frida / /proc/<pid>/mem) y buscar la clave AES.",
            "Reenviar el trafico de notificaciones con un proxy MITM ya hooks el texto push.",
        ],
    })
}

/// Extrae las citas médicas de la BD de una app de salud (Salud Responde).
///
/// Si el esquema no encaja, se informa del esquema real detectado en lugar
/// de inventar datos.
pub fn extract_appointments_db(path: &Path, limit: usize) -> Value {
    let limit = limit.clamp(1, 5000);
    let (conn, mode) = match open_snapshot(path) {
        Ok(v) => v,
        Err(e) => {
            return json!({
                "ok": false,
                "error": "open_failed",
                "detail": e,
            })
        }
    };
    let tables = table_names(&conn);
    let mut schema: Vec<Value> = Vec::new();
    let mut appointments: Vec<Value> = Vec::new();
    let mut readable = false;
    let now = now_epoch();

    for t in &tables {
        let cols = table_columns(&conn, t);
        schema.push(json!({ "name": t, "columns": cols.clone() }));
        let lname = t.to_lowercase();
        let looks_like_appointments = ["cita", "appointment", "evento", "event", "agenda", "consulta"]
            .iter()
            .any(|k| lname.contains(k));
        if !looks_like_appointments {
            continue;
        }
        let date_col = pick_col(
            &cols,
            &[
                "fecha",
                "fecha_cita",
                "date",
                "dia",
                "day",
                "start_date",
                "fecha_inicio",
                "hora",
                "hour",
                "start_time",
                "timestamp",
            ],
        )
        .or_else(|| pick_col_loose(&cols, &["fecha", "date", "dia"]));
        let place_col = pick_col(
            &cols,
            &[
                "lugar",
                "place",
                "centro",
                "servicio",
                "service",
                "especialidad",
                "consulta",
                "direccion",
                "address",
                "centro_salud",
            ],
        )
        .or_else(|| pick_col_loose(&cols, &["lugar", "centro", "especialidad", "servicio"]));
        let notes_col = pick_col(
            &cols,
            &[
                "notas",
                "nota",
                "notes",
                "note",
                "observaciones",
                "observacion",
                "descripcion",
                "comentario",
                "detalle",
                "motivo",
            ],
        )
        .or_else(|| pick_col_loose(&cols, &["nota", "observ", "descri", "comentario"]));
        let Some(date_col) = date_col else { continue };
        readable = true;

        let date_expr = ident(&date_col);
        let place_expr = place_col
            .as_ref()
            .map(|c| ident(c))
            .unwrap_or_else(|| "NULL".into());
        let notes_expr = notes_col
            .as_ref()
            .map(|c| ident(c))
            .unwrap_or_else(|| "NULL".into());
        let sql = format!(
            "SELECT {}, {}, {} FROM {} LIMIT {}",
            date_expr,
            place_expr,
            notes_expr,
            ident(t),
            limit
        );
        let Ok(mut st) = conn.prepare(&sql) else { continue };
        let Ok(mut rows) = st.query_map([], |row| {
            let raw_date = row.get_ref(0).ok().and_then(ref_text);
            let numeric = row.get_ref(0).ok().and_then(ref_i64);
            let place = row.get_ref(1).ok().and_then(ref_text);
            let notes = row.get_ref(2).ok().and_then(ref_text);
            Ok::<(Option<String>, Option<i64>, Option<String>, Option<String>), rusqlite::Error>((
                raw_date, numeric, place, notes,
            ))
        }) else {
            continue;
        };
        while let Some(Ok((raw_date, numeric, place, notes))) = rows.next() {
            let raw_date = raw_date.unwrap_or_default();
            let parsed = match numeric {
                Some(n) if n > 1_000_000_000_000 => Some(ParsedDate {
                    epoch: n / 1000,
                    iso: format_epoch(n / 1000),
                    raw: raw_date.clone(),
                    precision: "minute".into(),
                }),
                Some(n) if n > 1_000_000_000 => Some(ParsedDate {
                    epoch: n,
                    iso: format_epoch(n),
                    raw: raw_date.clone(),
                    precision: "minute".into(),
                }),
                _ => parse_appointment_date(&raw_date, now),
            };
            let epoch = parsed.as_ref().map(|p| p.epoch);
            let upcoming = is_upcoming(epoch, now);
            appointments.push(json!({
                "date": parsed.as_ref().map(|p| p.iso.clone()),
                "date_raw": raw_date,
                "service": place.clone().unwrap_or_default(),
                "place": place.clone().unwrap_or_default(),
                "notes": notes.clone().unwrap_or_default(),
                "source": "salud_responde",
                "table": t,
                "upcoming": upcoming,
            }));
        }
    }

    json!({
        "ok": true,
        "open_mode": mode,
        "readable": readable,
        "schema_detected": { "tables": schema },
        "appointments": appointments,
    })
}

/// Extrae las citas médicas: del SMS y de la app de Salud Responde.
pub fn medical(limit: usize) -> Value {
    let limit = limit.clamp(1, 5000);
    let now = now_epoch();

    // (a) Citas detectadas en los SMS.
    let (records, meta) = collect_sms_records();
    let mut appointments: Vec<Value> = Vec::new();
    for r in &records {
        let kws = detect_medical_keywords(&r.body);
        if kws.is_empty() {
            continue;
        }
        let parsed = parse_appointment_date(&r.body, now);
        let upcoming = is_upcoming(parsed.as_ref().map(|p| p.epoch), now);
        appointments.push(json!({
            "date": parsed.as_ref().map(|p| p.iso.clone()),
            "date_raw": parsed.as_ref().map(|p| p.raw.clone()).unwrap_or_default(),
            "service": "sms",
            "place": r.address,
            "notes": title_from_text(&r.body, 400),
            "source": "sms",
            "keywords": kws,
            "upcoming": upcoming,
            "ts": r.ts(),
        }));
    }
    let sms_count = appointments.len();

    // (b) Salud Responde (u otra app de salud instalada).
    let pm = exec("pm list packages 2>/dev/null", 60_000);
    if let Some(e) = device_error(&pm) {
        return e;
    }
    let installed_list: Vec<String> = pm
        .stdout
        .lines()
        .filter_map(|l| l.strip_prefix("package:").map(|s| s.trim().to_string()))
        .collect();
    let mut health_packages: Vec<String> = find_health_packages(&installed_list);
    if !health_packages.iter().any(|p| p == SALUD_RESPONDE_PACKAGE)
        && installed_list.iter().any(|p| p == SALUD_RESPONDE_PACKAGE)
    {
        health_packages.push(SALUD_RESPONDE_PACKAGE.to_string());
    }
    let mut app_report = json!({
        "installed": !health_packages.is_empty(),
        "packages": health_packages.clone(),
        "databases": Vec::<Value>::new(),
        "readable": false,
    });
    for pkg in &health_packages {
        let db_dir = format!("/data/data/{}/databases", pkg);
        for (path, size) in device_list_dir(&db_dir) {
            if !path.ends_with(".db") {
                continue;
            }
            let mut entry = json!({
                "package": pkg,
                "path": path,
                "size": size,
                "readable": false,
            });
            if let Some((snap, _)) = snapshot_remote(&path, "salud") {
                let ext = extract_appointments_db(&snap, limit);
                if ext.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                    entry["readable"] = json!(ext["readable"].clone());
                    entry["schema_detected"] = ext["schema_detected"].clone();
                    if let Some(arr) = ext["appointments"].as_array() {
                        for a in arr {
                            let mut av = a.clone();
                            av["package"] = json!(pkg);
                            appointments.push(av);
                        }
                    }
                }
            }
            if let Some(arr) = app_report["databases"].as_array() {
                let mut v = arr.clone();
                v.push(entry);
                app_report["databases"] = json!(v);
            }
        }
    }
    let readable = app_report["databases"]
        .as_array()
        .map(|a| a.iter().any(|d| d["readable"].as_bool() == Some(true)))
        .unwrap_or(false);
    app_report["readable"] = json!(readable);

    appointments.sort_by(|a, b| {
        let ta = a["date"].as_str().unwrap_or("").to_string();
        let tb = b["date"].as_str().unwrap_or("").to_string();
        ta.cmp(&tb)
    });
    let upcoming = appointments
        .iter()
        .filter(|a| a["upcoming"].as_bool() == Some(true))
        .count();

    json!({
        "ok": true,
        "now": now,
        "appointments": appointments,
        "upcoming": upcoming,
        "sources": {
            "sms": { "count": sms_count, "provider": meta },
            "health_app": app_report,
        },
    })
}

/// Ejecuta todos los extractores e inyecta el resultado en el almacén de
/// conversaciones (`crate::sync::ingest_conversations`).
///
/// Es **idempotente**: el almacén fusiona por `service` + `id`, así que
/// repetir la ingesta actualiza en lugar de duplicar.
pub fn ingest_all() -> Value {
    let mut conversations: Vec<Value> = Vec::new();
    let mut per_source = serde_json::Map::new();

    let tg = telegram(500);
    if let Some(arr) = tg["conversations"].as_array() {
        per_source.insert("telegram".into(), json!(arr.len()));
        conversations.extend(arr.iter().cloned());
    } else {
        per_source.insert("telegram".into(), json!(0));
    }

    let sm = sms(200);
    if let Some(arr) = sm["conversations"].as_array() {
        per_source.insert("sms".into(), json!(arr.len()));
        conversations.extend(arr.iter().cloned());
    } else {
        per_source.insert("sms".into(), json!(0));
    }

    // WhatsApp: sólo el texto que el sistema ya tenía visible (notificaciones).
    let wa = whatsapp(100);
    if let Some(arr) = wa["notifications"].as_array() {
        if !arr.is_empty() {
            let mut msgs: Vec<Value> = Vec::new();
            for n in arr {
                let text = format!("{} {}", n["title"].as_str().unwrap_or(""), n["text"].as_str().unwrap_or(""));
                if text.trim().is_empty() {
                    continue;
                }
                msgs.push(json!({
                    "role": "peer",
                    "content": title_from_text(&text, 1000),
                    "ts": n.get("ts").cloned().unwrap_or(json!(0.0)),
                    "source": "notification",
                }));
            }
            if !msgs.is_empty() {
                let updated_at = msgs
                    .iter()
                    .filter_map(|m| m["ts"].as_f64())
                    .fold(0.0f64, f64::max);
                conversations.push(json!({
                    "service": "whatsapp",
                    "id": "whatsapp:notifications",
                    "title": "WhatsApp (notificaciones visibles)",
                    "updated_at": updated_at,
                    "messages": msgs,
                    "source": "dumpsys notification --noredact",
                    "note": "solo texto que el sistema ya mostraba: el historico cifrado no es legible",
                }));
            }
        }
        per_source.insert("whatsapp_notifications".into(), json!(arr.len()));
    }

    let total = crate::sync::ingest_conversations(conversations.clone());
    json!({
        "ok": true,
        "ingested": conversations.len(),
        "total_conversations": total,
        "per_source": Value::Object(per_source),
        "idempotent": true,
    })
}

/// Lista los mensajes pendientes relacionados con citas médicas o con una
/// fecha próxima (semilla del sistema de alertas).
pub fn alert_candidates() -> Value {
    let now = now_epoch();
    let mut candidates: Vec<Value> = Vec::new();

    let (records, _) = collect_sms_records();
    for r in &records {
        let kws = detect_medical_keywords(&r.body);
        if kws.is_empty() {
            continue;
        }
        let parsed = parse_appointment_date(&r.body, now);
        let upcoming = is_upcoming(parsed.as_ref().map(|p| p.epoch), now);
        candidates.push(json!({
            "service": "sms",
            "id": format!("sms:{}", r.id),
            "title": r.address,
            "content": title_from_text(&r.body, 500),
            "keywords": kws,
            "date": parsed.as_ref().map(|p| p.iso.clone()),
            "date_raw": parsed.as_ref().map(|p| p.raw.clone()).unwrap_or_default(),
            "days_until": parsed.as_ref().map(|p| (p.epoch - now) / 86_400),
            "upcoming": upcoming,
            "ts": r.ts(),
        }));
    }

    // Notificaciones de WhatsApp (texto visible, no cifrado).
    let pm = exec("pm list packages 2>/dev/null", 60_000);
    if let Some(e) = device_error(&pm) {
        return e;
    }
    let mut packages: Vec<&str> = Vec::new();
    for p in [WHATSAPP_PACKAGE, WHATSAPP_W4B_PACKAGE] {
        if pm.stdout.contains(p) {
            packages.push(p);
        }
    }
    if !packages.is_empty() {
        let raw = exec("dumpsys notification --noredact 2>/dev/null", 60_000);
        for n in parse_notification_dump(&raw.stdout, &packages) {
            let kws = detect_medical_keywords(&format!("{} {}", n.title, n.text));
            if kws.is_empty() {
                continue;
            }
            let body = format!("{} {}", n.title, n.text);
            let parsed = parse_appointment_date(&body, now);
            candidates.push(json!({
                "service": "whatsapp",
                "id": format!("whatsapp:notification:{}", n.ts.unwrap_or(0)),
                "title": n.title,
                "content": title_from_text(&n.text, 500),
                "keywords": kws,
                "date": parsed.as_ref().map(|p| p.iso.clone()),
                "date_raw": parsed.as_ref().map(|p| p.raw.clone()).unwrap_or_default(),
                "days_until": parsed.as_ref().map(|p| (p.epoch - now) / 86_400),
                "upcoming": is_upcoming(parsed.as_ref().map(|p| p.epoch), now),
                "ts": n.ts.map(|t| t as f64).unwrap_or(0.0),
            }));
        }
    }

    let mut upcoming = 0usize;
    for c in &candidates {
        if c["upcoming"].as_bool() == Some(true) {
            upcoming += 1;
        }
    }
    json!({
        "ok": true,
        "now": now,
        "count": candidates.len(),
        "upcoming": upcoming,
        "candidates": candidates,
    })
}

// ========================================================================= //
// 10. Definiciones MCP (para cablear el módulo al servidor)
// ========================================================================= //

/// Definiciones de las herramientas de mensajería.
pub fn tool_definitions() -> Value {
    json!([
        {
            "name": "messaging_inventory",
            "description": "Inventario de apps de mensajeria instaladas (Telegram, WhatsApp, SMS, Salud Responde): APK, tamaño y almacenes de datos disponibles en el dispositivo.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "messaging_telegram",
            "description": "Extrae los mensajes de Telegram de msgstore.db (copia previa a un snapshot) y los normaliza al esquema de conversaciones de MegaDaemon.",
            "inputSchema": {
                "type": "object",
                "properties": { "limit": { "type": "integer", "default": 200, "maximum": 5000 } }
            }
        },
        {
            "name": "messaging_sms",
            "description": "Extrae SMS y MMS por el content provider de Android y los normaliza al esquema de conversaciones, marcando los que hablan de citas medicas.",
            "inputSchema": {
                "type": "object",
                "properties": { "limit": { "type": "integer", "default": 200, "maximum": 5000 } }
            }
        },
        {
            "name": "messaging_whatsapp",
            "description": "Extrae lo legible de WhatsApp: contactos y chats de wa.db, y el texto visible de las notificaciones push. El historico de mensajes esta cifrado y se informa de por que.",
            "inputSchema": {
                "type": "object",
                "properties": { "limit": { "type": "integer", "default": 100, "maximum": 5000 } }
            }
        },
        {
            "name": "messaging_medical",
            "description": "Extrae las citas medicas: las que se deducen de los SMS y las que estan en la app de Salud Responde (con su esquema detectado).",
            "inputSchema": {
                "type": "object",
                "properties": { "limit": { "type": "integer", "default": 200, "maximum": 5000 } }
            }
        },
        {
            "name": "messaging_ingest_all",
            "description": "Ejecuta todos los extractores e inyecta el resultado en el almacen de conversaciones de MegaDaemon. Idempotente (mismo id actualiza, no duplica).",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "messaging_alert_candidates",
            "description": "Mensajes pendientes relacionados con citas medicas o recordatorios con fecha proxima, con la fecha ya parseada.",
            "inputSchema": { "type": "object", "properties": {} }
        }
    ])
}

/// Despacha las herramientas de mensajería.
pub fn dispatch(name: &str, args: &Value) -> Option<Value> {
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(200) as usize;
    Some(match name {
        "messaging_inventory" => inventory(),
        "messaging_telegram" => telegram(limit),
        "messaging_sms" => sms(limit),
        "messaging_whatsapp" => whatsapp(limit),
        "messaging_medical" => medical(limit),
        "messaging_ingest_all" => ingest_all(),
        "messaging_alert_candidates" => alert_candidates(),
        _ => return None,
    })
}

// ========================================================================= //
// 11. Tests (sin dispositivo)
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;

    /// Crea una BD SQLite temporal con el esquema simplificado de Telegram.
    fn temp_telegram_db(with_info: bool, self_id: i64) -> (PathBuf, Connection) {
        let dir = std::env::temp_dir().join(format!("megadaemon-msg-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(format!(
            "telegram_{}_{}.db",
            if with_info { "info" } else { "ninfo" },
            self_id
        ));
        let _ = std::fs::remove_file(&path);
        let conn = Connection::open(&path).expect("se abre la BD de prueba");
        conn.execute_batch(
            "CREATE TABLE messages (
                uid INTEGER PRIMARY KEY,
                chat_id INTEGER,
                from_id INTEGER,
                date INTEGER,
                message TEXT
             );
             CREATE TABLE chats (
                chat_id INTEGER PRIMARY KEY,
                title TEXT
             );",
        )
        .expect("se crea el esquema");
        if with_info {
            conn.execute_batch(
                "CREATE TABLE info (key TEXT, value TEXT);
                 INSERT INTO info VALUES ('user_id', '42');",
            )
            .expect("se crea la tabla info");
        }
        (path, conn)
    }

    fn insert_tg(conn: &Connection, uid: i64, chat: i64, from: i64, date: i64, text: &str) {
        conn.execute(
            "INSERT INTO messages (uid, chat_id, from_id, date, message) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![uid, chat, from, date, text],
        )
        .expect("se inserta el mensaje");
    }

    // ---------------------------------------------------------------- texto --

    #[test]
    fn detecta_las_palabras_clave_medicas() {
        let kws = detect_medical_keywords("Recuerdame la cita con el médico del hospital");
        assert!(kws.contains(&"cita".to_string()));
        assert!(kws.contains(&"medico".to_string()));
        assert!(kws.contains(&"hospital".to_string()));
        assert!(is_medical_text("Tienes una cita previa mañana"));
    }

    #[test]
    fn no_confunde_palabras_similares() {
        // "citacion" no es una cita; "anfitrion" no es un hospital.
        assert!(!is_medical_text("Una citacion del libro"));
        assert!(!is_medical_text("El anfitrion recibio a los guests"));
        assert!(!is_medical_text("Buenos dias, como estas?"));
    }

    #[test]
    fn reconoce_palabras_clave_compuestas() {
        let kws = detect_medical_keywords("Te han dado cita previa en el centro de salud");
        assert!(kws.contains(&"cita previa".to_string()));
        assert!(kws.contains(&"centro de salud".to_string()));
    }

    #[test]
    fn la_comparacion_ignora_acentos_y_mayusculas() {
        assert!(is_medical_text("CITAS: ANÁLISIS SANGUÍNEO"));
        assert!(is_medical_text("Revision medica en la clinica"));
    }

    // -------------------------------------------------------------- fechas --

    #[test]
    fn formatea_el_epoch_a_fecha_legible() {
        // 2025-03-12 09:00:00 UTC
        assert_eq!(format_epoch(1_741_770_000), "2025-03-12 09:00:00");
        assert_eq!(format_date(1_741_770_000), "2025-03-12");
    }

    #[test]
    fn parsea_fecha_numerica_espanola() {
        let now = 1_741_770_000; // 12/03/2025 09:00
        let d = parse_appointment_date("Tienes cita el 25/04/2025 a las 10:30", now).unwrap();
        assert_eq!(format_date(d.epoch), "2025-04-25");
        assert_eq!(d.precision, "minute");
    }

    #[test]
    fn parsea_fecha_iso() {
        let now = 1_741_770_000;
        let d = parse_appointment_date("cita el 2025-05-02", now).unwrap();
        assert_eq!(format_date(d.epoch), "2025-05-02");
    }

    #[test]
    fn parsea_fecha_con_mes_escrito() {
        let now = 1_741_770_000;
        let d = parse_appointment_date("cita el 12 de marzo de 2025", now).unwrap();
        assert_eq!(format_date(d.epoch), "2025-03-12");
    }

    #[test]
    fn parsea_fechas_relativas() {
        let now = 1_741_770_000; // 12/03/2025 09:00 09:00 UTC
        let manana = parse_appointment_date("nos vemos mañana", now).unwrap();
        assert_eq!(format_date(manana.epoch), "2025-03-13");
        let hoy = parse_appointment_date("hoy a las 10", now).unwrap();
        assert_eq!(format_date(hoy.epoch), "2025-03-12");
    }

    // ----------------------------------------------------------------- sms --

    #[test]
    fn parsea_las_filas_del_content_provider() {
        let raw = "Row: 0 _id=1, address=+34600111222, body=Hola, date=1712345678000, type=1, read=1\n\
                   Row: 1 _id=2, address=+34600111222, body=Buenos dias, cita a las 10, date=1712345760000, type=2, read=0";
        let recs = parse_sms_rows(raw);
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].address, "+34600111222");
        assert_eq!(recs[0].body, "Hola");
        assert_eq!(recs[0].date_ms, 1_712_345_678_000);
        assert_eq!(recs[0].role(), "peer");
        assert_eq!(recs[1].role(), "user");
        assert!(recs[1].read == false);
    }

    #[test]
    fn el_cuerpo_con_comas_y_saltos_no_rompe_el_parseo() {
        let raw = "Row: 0 _id=7, address=+34600, body=Hola, mundo\nSegunda linea, date=1712345678000, type=1, read=1";
        let recs = parse_sms_rows(raw);
        assert_eq!(recs.len(), 1);
        assert!(recs[0].body.contains("Hola, mundo"));
        assert!(recs[0].body.contains("Segunda linea"));
        // La coma final es el separador de campos, no parte del cuerpo.
        assert!(!recs[0].body.trim_end().ends_with(','));
        assert_eq!(recs[0].date_ms, 1_712_345_678_000);
    }

    #[test]
    fn agrupa_los_sms_por_hilo() {
        let recs = vec![
            SmsRecord {
                id: 1,
                address: "+34600".into(),
                body: "hola".into(),
                date_ms: 1_712_345_678_000,
                sms_type: 1,
                read: true,
                thread_id: Some(9),
            },
            SmsRecord {
                id: 2,
                address: "+34600".into(),
                body: "que tal".into(),
                date_ms: 1_712_345_688_000,
                sms_type: 2,
                read: true,
                thread_id: Some(9),
            },
            SmsRecord {
                id: 3,
                address: "+34611".into(),
                body: "otro".into(),
                date_ms: 1_712_345_698_000,
                sms_type: 1,
                read: true,
                thread_id: Some(10),
            },
        ];
        let convs = sms_conversations(recs, 50, &HashMap::new());
        assert_eq!(convs.len(), 2);
        let first = &convs[0];
        assert_eq!(first["service"], json!("sms"));
        assert_eq!(first["id"], json!("sms:thread_9"));
        assert_eq!(first["messages"].as_array().unwrap().len(), 2);
        // Orden ascendente por fecha.
        assert_eq!(
            first["messages"][0]["content"],
            json!("hola"),
            "los mensajes deben ir de mas antiguo a mas reciente"
        );
        assert_eq!(first["messages"][0]["role"], json!("peer"));
        assert_eq!(first["messages"][1]["role"], json!("user"));
    }

    #[test]
    fn marca_los_mensajes_medicos_en_el_hilo() {
        let recs = vec![SmsRecord {
            id: 1,
            address: "+34600".into(),
            body: "Confirmada tu cita con el médico el 25/04/2025".into(),
            date_ms: 1_712_345_678_000,
            sms_type: 2,
            read: true,
            thread_id: Some(9),
        }];
        let convs = sms_conversations(recs, 50, &HashMap::new());
        assert_eq!(convs[0]["medical"], json!(true));
        let m = &convs[0]["messages"][0];
        assert_eq!(m["medical"], json!(true));
        let kws = m["keywords"].as_array().unwrap();
        assert!(kws.iter().any(|k| k == "cita"));
        assert!(kws.iter().any(|k| k == "medico"));
    }

    #[test]
    fn agrupa_por_direccion_cuando_no_hay_thread_id() {
        let recs = vec![
            SmsRecord {
                id: 1,
                address: "+34600".into(),
                body: "a".into(),
                date_ms: 1,
                sms_type: 1,
                read: true,
                thread_id: None,
            },
            SmsRecord {
                id: 2,
                address: "+34600".into(),
                body: "b".into(),
                date_ms: 2,
                sms_type: 1,
                read: true,
                thread_id: None,
            },
        ];
        let convs = sms_conversations(recs, 50, &HashMap::new());
        assert_eq!(convs.len(), 1);
        assert_eq!(convs[0]["id"], json!("sms:addr_+34600"));
    }

    #[test]
    fn el_id_del_hilo_es_estable_para_la_ingesta_idempotente() {
        let recs = vec![SmsRecord {
            id: 1,
            address: "+34600".into(),
            body: "a".into(),
            date_ms: 1,
            sms_type: 1,
            read: true,
            thread_id: Some(9),
        }];
        let a = sms_conversations(recs.clone(), 50, &HashMap::new());
        let b = sms_conversations(recs, 50, &HashMap::new());
        assert_eq!(a[0]["id"], b[0]["id"]);
    }

    // ------------------------------------------------------------ telegram --

    #[test]
    fn extrae_los_mensajes_de_telegram_con_el_rol_correcto() {
        let (path, conn) = temp_telegram_db(true, 42);
        conn.execute(
            "INSERT INTO chats (chat_id, title) VALUES (100, 'Ana')",
            [],
        )
        .unwrap();
        insert_tg(&conn, 1, 100, 42, 1_712_345_678, "Hola Ana");
        insert_tg(&conn, 2, 100, 7, 1_712_345_688, "Hola a ti");
        insert_tg(&conn, 3, 100, 42, 1_712_345_698, "Nos vemos manana");
        drop(conn);

        let out = extract_telegram_db(&path, 100);
        assert_eq!(out["ok"], json!(true));
        assert_eq!(out["self_id"], json!(42));
        assert_eq!(out["self_id_source"], json!("info"));
        let convs = out["conversations"].as_array().unwrap();
        assert_eq!(convs.len(), 1);
        assert_eq!(convs[0]["title"], json!("Ana"));
        assert_eq!(convs[0]["service"], json!("telegram"));
        assert_eq!(convs[0]["id"], json!("telegram:chat_100"));
        let msgs = convs[0]["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0]["role"], json!("user"), "el primero lo escribio el usuario");
        assert_eq!(msgs[0]["content"], json!("Hola Ana"));
        assert_eq!(msgs[1]["role"], json!("peer"), "el segundo lo escribio el otro");
        assert_eq!(msgs[2]["content"], json!("Nos vemos manana"));
        assert_eq!(msgs[2]["ts"], json!(1_712_345_698.0));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn infiere_el_usuario_por_patron_si_no_hay_tabla_info() {
        let (path, conn) = temp_telegram_db(false, 0);
        // Tres mensajes salientes (from_id 0) y uno entrante.
        insert_tg(&conn, 1, 100, 0, 10, "uno");
        insert_tg(&conn, 2, 100, 0, 20, "dos");
        insert_tg(&conn, 3, 100, 0, 30, "tres");
        insert_tg(&conn, 4, 100, 9, 40, "respuesta");
        drop(conn);

        let out = extract_telegram_db(&path, 100);
        assert_eq!(out["self_id_source"], json!("pattern"));
        assert_eq!(out["self_id"], json!(0));
        let msgs = out["conversations"][0]["messages"].as_array().unwrap();
        assert_eq!(msgs[3]["role"], json!("peer"));
        assert_eq!(msgs[0]["role"], json!("user"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn detecta_el_esquema_real_de_telegram() {
        let (path, conn) = temp_telegram_db(true, 42);
        let schema = detect_telegram_schema(&conn);
        assert_eq!(schema.messages_table.as_deref(), Some("messages"));
        assert_eq!(schema.chats_table.as_deref(), Some("chats"));
        assert_eq!(schema.text_col.as_deref(), Some("message"));
        assert_eq!(schema.from_id_col.as_deref(), Some("from_id"));
        assert_eq!(schema.date_col.as_deref(), Some("date"));
        assert_eq!(schema.chat_id_col.as_deref(), Some("chat_id"));
        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn informa_cuando_no_hay_tabla_de_mensajes() {
        let dir = std::env::temp_dir().join(format!("megadaemon-msg-test-vacio-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("vacio.db");
        let _ = std::fs::remove_file(&path);
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE otra (a INTEGER);")
            .unwrap();
        drop(conn);
        let out = extract_telegram_db(&path, 10);
        assert_eq!(out["ok"], json!(false));
        assert_eq!(out["error"], json!("no_messages_table"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn decodifica_el_texto_de_un_blob_en_utf8() {
        let blob = b"Hola, como estas?";
        assert_eq!(
            decode_message_blob(blob),
            Some("Hola, como estas?".to_string())
        );
    }

    #[test]
    fn decodifica_el_texto_serializado_al_estilo_tl() {
        // TL: longitud en 1 byte + texto.
        let mut blob = vec![0x2b, 0x00, 0x00, 0x00];
        blob.push(15u8);
        blob.extend_from_slice("Cita en el hospital".as_bytes());
        let out = decode_message_blob(&blob);
        assert!(out.is_some());
        assert!(out.unwrap().contains("Cita en el hospital"));
    }

    // ----------------------------------------------------------- whatsapp --

    #[test]
    fn extrae_contactos_y_chats_de_wa_db() {
        let dir = std::env::temp_dir().join(format!("megadaemon-msg-wa-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("wa.db");
        let _ = std::fs::remove_file(&path);
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE wa_contacts (jid TEXT, display_name TEXT);
             CREATE TABLE wa_chats (jid TEXT, unread_count INTEGER, timestamp INTEGER);
             INSERT INTO wa_contacts VALUES ('34600111222@s.whatsapp.net', 'Ana');
             INSERT INTO wa_chats VALUES ('34600111222@s.whatsapp.net', 3, 1712345678);",
        )
        .unwrap();
        drop(conn);

        let out = extract_whatsapp_db(&path, 50);
        assert_eq!(out["ok"], json!(true));
        assert_eq!(out["contacts_table"], json!("wa_contacts"));
        assert_eq!(out["chats_table"], json!("wa_chats"));
        assert_eq!(out["contacts"].as_array().unwrap().len(), 1);
        assert_eq!(out["chats"][0]["unread_count"], json!(3));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn parsea_las_notificaciones_push() {
        let dump = "NotificationRecord(0x1: pkg=com.whatsapp user=0 id=1 tag=null key=0|com.whatsapp|1|null|10)\n  \
            extras={\n    \
            android.title=String (Ana)\n    \
            android.text=String (Tienes cita manana a las 10)\n  \
            }\n  \
            postTime=03-12 09:30:00.000\n\
             NotificationRecord(0x2: pkg=com.android.systemui user=0 id=2 tag=null key=1)\n  \
            extras={\n    android.title=String (Bateria)\n    android.text=String (50%)\n  \
            }\n";
        let notifs = parse_notification_dump(dump, &["com.whatsapp"]);
        assert_eq!(notifs.len(), 1);
        assert_eq!(notifs[0].package, "com.whatsapp");
        assert_eq!(notifs[0].title, "Ana");
        assert_eq!(notifs[0].text, "Tienes cita manana a las 10");
        assert!(notifs[0].ts.is_some());
        assert!(is_medical_text(&notifs[0].text));
    }

    #[test]
    fn el_motivo_del_cifrado_de_whatsapp_esta_explicado() {
        assert!(WHATSAPP_ENCRYPTION_REASON.contains("AndroidKeyStore"));
        assert!(WHATSAPP_ENCRYPTION_REASON.contains("Frida"));
    }

    // -------------------------------------------------------------- base64 --

    #[test]
    fn decodifica_base64() {
        assert_eq!(
            decode_base64("SG9sYSBtdW5kbw==").unwrap(),
            b"Hola mundo".to_vec()
        );
        assert!(decode_base64("no es base64 !!!").is_none());
    }

    // ------------------------------------------------------------- medical --

    #[test]
    fn extrae_citas_de_una_bd_de_salud() {
        let dir = std::env::temp_dir().join(format!("megadaemon-msg-salud-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("salud.db");
        let _ = std::fs::remove_file(&path);
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE citas (id INTEGER, fecha TEXT, lugar TEXT, notas TEXT);
             INSERT INTO citas VALUES (1, '25/04/2025 10:30', 'Hospital Virgen del Carmen', 'Revision');",
        )
        .unwrap();
        drop(conn);

        let out = extract_appointments_db(&path, 50);
        assert_eq!(out["ok"], json!(true));
        assert_eq!(out["readable"], json!(true));
        let ap = out["appointments"].as_array().unwrap();
        assert_eq!(ap.len(), 1);
        assert_eq!(ap[0]["place"], json!("Hospital Virgen del Carmen"));
        assert_eq!(ap[0]["notes"], json!("Revision"));
        assert_eq!(ap[0]["date"].as_str().unwrap().chars().take(10).collect::<String>(), "2025-04-25");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn informa_del_esquema_cuando_la_bd_de_salud_no_encaja() {
        let dir = std::env::temp_dir().join(format!("megadaemon-msg-salud2-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("salud2.db");
        let _ = std::fs::remove_file(&path);
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE otra_cosa (x INTEGER, y TEXT);")
            .unwrap();
        drop(conn);
        let out = extract_appointments_db(&path, 50);
        assert_eq!(out["ok"], json!(true));
        assert_eq!(out["readable"], json!(false));
        assert!(out["schema_detected"]["tables"].as_array().unwrap().len() == 1);
        let _ = std::fs::remove_file(&path);
    }

    // ---------------------------------------------------------- esquema API --

    #[test]
    fn las_conversaciones_cumplen_el_esquema_de_sync() {
        let (path, conn) = temp_telegram_db(true, 42);
        insert_tg(&conn, 1, 100, 42, 1_712_345_678, "Hola");
        drop(conn);
        let out = extract_telegram_db(&path, 10);
        let conv = &out["conversations"][0];
        assert!(conv["service"].is_string());
        assert!(conv["id"].is_string());
        assert!(conv["title"].is_string());
        assert!(conv["updated_at"].is_number());
        let m = &conv["messages"][0];
        assert!(m["role"].is_string());
        assert!(m["content"].is_string());
        assert!(m["ts"].is_number());
        // El rol sólo puede ser user, peer o assistant.
        let role = m["role"].as_str().unwrap();
        assert!(["user", "peer", "assistant"].contains(&role));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn el_limite_deja_solo_los_mensajes_mas_recientes() {
        let recs: Vec<SmsRecord> = (0..5)
            .map(|i| SmsRecord {
                id: i,
                address: "+34600".into(),
                body: format!("mensaje {}", i),
                date_ms: 1_712_345_678_000 + i * 1000,
                sms_type: 1,
                read: true,
                thread_id: Some(1),
            })
            .collect();
        let convs = sms_conversations(recs, 2, &HashMap::new());
        let msgs = convs[0]["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["content"], json!("mensaje 3"));
        assert_eq!(msgs[1]["content"], json!("mensaje 4"));
    }

    #[test]
    fn se_adapta_a_un_esquema_de_telegram_distinto() {
        let dir = std::env::temp_dir().join(format!("megadaemon-msg-var-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("variante.db");
        let _ = std::fs::remove_file(&path);
        let conn = Connection::open(&path).unwrap();
        // Nombres de columna distintos: los habituales en forks y en
        // clientes viejos.
        conn.execute_batch(
            "CREATE TABLE message (id INTEGER PRIMARY KEY, dialog_id INTEGER,
                                   sender_id INTEGER, timestamp INTEGER, text TEXT);
             CREATE TABLE dialogs (dialog_id INTEGER PRIMARY KEY, name TEXT);
             INSERT INTO dialogs VALUES (5, 'Grupo familiar');
             INSERT INTO message VALUES (1, 5, 77, 1712345678, 'primer mensaje');
             INSERT INTO message VALUES (2, 5, 88, 1712345688, 'segundo mensaje');",
        )
        .unwrap();
        drop(conn);

        let out = extract_telegram_db(&path, 50);
        assert_eq!(out["ok"], json!(true));
        assert_eq!(out["schema_detected"]["messages_table"], json!("message"));
        assert_eq!(out["schema_detected"]["chats_table"], json!("dialogs"));
        let conv = &out["conversations"][0];
        assert_eq!(conv["title"], json!("Grupo familiar"));
        assert_eq!(conv["id"], json!("telegram:chat_5"));
        let msgs = conv["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["content"], json!("primer mensaje"));
        assert_eq!(msgs[0]["role"], json!("user"), "el mas frecuente es el propio usuario");
        assert_eq!(msgs[1]["role"], json!("peer"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn extrae_las_partes_de_texto_de_un_mms() {
        let raw = "Row: 0 mid=42, seq=0, ct=text/plain, text=Hola, esto es un MMS\nRow: 1 mid=42, seq=1, ct=image/jpeg, text=\nRow: 2 mid=43, seq=0, ct=text/plain, text=Otro mensaje";
        let parts = parse_mms_parts(raw);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts.get(&42).unwrap(), "Hola, esto es un MMS");
        assert_eq!(parts.get(&43).unwrap(), "Otro mensaje");
    }

    #[test]
    fn calcula_si_una_cita_esta_pasada() {
        let now = 1_741_770_000; // 12/03/2025 09:00
        assert!(is_upcoming(Some(1_741_770_000), now), "hoy es proxima");
        assert!(is_upcoming(Some(1_750_000_000), now), "mañana es proxima");
        assert!(!is_upcoming(Some(1_700_000_000), now), "ayer no es proxima");
        assert!(!is_upcoming(None, now), "sin fecha no se puede afirmar");
    }

    #[test]
    fn las_herramientas_declaran_su_esquema() {
        let defs = tool_definitions();
        let arr = defs.as_array().unwrap();
        assert_eq!(arr.len(), 7);
        for d in arr {
            assert!(d["name"].is_string());
            assert!(d["description"].is_string());
            assert!(d["inputSchema"].is_object());
        }
    }
}
