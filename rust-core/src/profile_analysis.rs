//! MegaDaemon v13.4 — Analizador de perfil del usuario (historial de IA).
//!
//! MegaDaemon ya sincroniza **todo** el historial de conversaciones del
//! usuario (ChatGPT, Claude, Gemini, DeepSeek, Kimi, Perplexity…) en un
//! almacén NDJSON dentro del *state dir*.  Este módulo convierte ese historial
//! en un **perfil** legible por un profesional (psicólogo, coach, terapeuta):
//! intereses, fortalezas, bloqueos, pendientes, patrones de uso y deriva
//! temática.
//!
//! Principios de diseño:
//!
//! * **100 % local**: sólo `std` + `serde_json`.  Sin red, sin servicios
//!   externos, sin crates añadidos.
//! * **Reproducible**: cada evidencia cita `conversation_id` y `ts`, de forma
//!   que cualquier afirmación del informe se puede volver a localizar en el
//!   texto original.
//! * **Explícito**: se declara qué es heurística y qué no.  Esto es
//!   autoobservación, **no** un diagnóstico clínico.
//! * **Testeable**: el núcleo (`analyze_conversations`) opera sobre un
//!   `&[Value]` de conversaciones, así que la suite no depende del *state dir*
//!   real del dispositivo.
//!
//! API pública:
//!
//! | función                    | para qué sirve                                    |
//! |----------------------------|---------------------------------------------------|
//! | `analyze()`                | perfil completo del usuario                       |
//! | `briefing(markdown)`       | informe listo para un profesional                  |
//! | `topic_drift(window_days)` | en qué está evolucionando el usuario              |
//! | `daily_digest(days)`       | resumen por día de la actividad                   |
//!
//! Las fechas se calculan en **UTC** (no hay base de datos de zonas horarias
//! disponible sin dependencias); el anclaje temporal es siempre el último
//! mensaje del corpus, no el reloj del sistema, para que el análisis de un
//! histórico congelado siga siendo coherente.

use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::util_text::truncate_chars;

// --------------------------------------------------------------------------- //
// Palabras vacías (stopwords)
// --------------------------------------------------------------------------- //

/// Palabras vacías en español: artículos, preposiciones, conjunciones, verbos
/// auxiliares, pronombres y sustantivos vacíos de contenido.
const STOPWORDS_ES: &[&str] = &[
    "a", "al", "algo", "algun", "alguna", "algunas", "alguno", "algunos", "ante", "antes", "aquel",
    "aquella", "aquello", "aqui", "asi", "aun", "aunque", "cada", "casi", "como", "con", "contra",
    "cual", "cuales", "cuando", "cuanto", "de", "del", "desde", "donde", "dos", "durante", "e",
    "el", "ella", "ellas", "ello", "ellos", "en", "entre", "era", "eran", "eras", "eres", "es",
    "esa", "esas", "ese", "eso", "esos", "esta", "estaba", "estaban", "estado", "estan", "estar",
    "estas", "este", "esto", "estos", "estoy", "fue", "fueron", "fui", "ha", "habia", "han",
    "hasta", "hay", "hace", "hacen", "hacia", "la", "las", "le", "les", "lo", "los", "mas",
    "me", "mi", "mis", "mucho", "muy", "nada", "ni", "no", "nos", "nosotros", "nuestra",
    "nuestro", "nunca", "o", "os", "otra", "otro", "para", "pero", "poco", "por", "porque",
    "puede", "pueden", "pues", "que", "quien", "quienes", "se", "segun", "ser", "si", "sido",
    "siempre", "sin", "sobre", "sois", "solo", "son", "soy", "su", "sus", "tal", "tambien",
    "tampoco", "tan", "tanto", "te", "tenemos", "tener", "tengo", "ti", "tiene", "tienen",
    "toda", "todas", "todo", "todos", "tras", "tu", "tus", "un", "una", "uno", "unos", "usted",
    "va", "vamos", "van", "ver", "vez", "y", "ya", "yo", "cosa", "cosas", "gente", "sirve",
    "usar", "usando", "puedes", "podemos", "decir", "dije", "tampoco", "vez", "vez",
];

/// Palabras vacías en inglés: artículos, preposiciones, pronombres, auxiliares
/// y verbos muy frecuentes.
///
/// La lista es deliberadamente amplia: en un corpus técnico real hay mucho
/// texto en inglés (código, logs, documentación pegada) y, si se colaran
/// "to", "is", "of", "in"… el ranking de intereses sería basura.
const STOPWORDS_EN: &[&str] = &[
    "a", "about", "above", "after", "again", "against", "all", "also", "am", "an", "and", "any",
    "are", "as", "at", "be", "because", "been", "before", "being", "below", "between", "both",
    "but", "by", "can", "cannot", "could", "did", "do", "does", "doing", "done", "down", "during",
    "each", "either", "else", "enough", "even", "ever", "every", "few", "for", "from", "further",
    "get", "gets", "getting", "give", "given", "go", "goes", "going", "gonna", "got", "had", "has",
    "have", "having", "he", "her", "here", "hers", "herself", "him", "himself", "his", "how",
    "however", "i", "if", "in", "into", "is", "it", "its", "itself", "just", "know", "like", "made",
    "make", "makes", "making", "many", "may", "me", "might", "mine", "more", "most", "much", "must",
    "my", "myself", "need", "needs", "neither", "never", "new", "no", "nor", "not", "now", "of",
    "off", "on", "once", "one", "only", "or", "other", "others", "ought", "our", "ours", "out",
    "over", "own", "perhaps", "quite", "really", "same", "say", "says", "see", "seen", "several",
    "shall", "she", "should", "so", "some", "such", "than", "that", "the", "their", "theirs",
    "them", "themselves", "then", "there", "these", "they", "thing", "things", "this", "those",
    "though", "through", "thus", "to", "too", "under", "until", "up", "upon", "us", "use", "used",
    "using", "very", "want", "was", "way", "we", "well", "were", "what", "when", "where", "whether",
    "which", "while", "who", "whom", "whose", "why", "will", "with", "within", "without", "would",
    "yes", "yet", "you", "your", "yours", "yourself",
];

/// Términos de interfaz y cortesía: ruido habitual del chat (saludos, roles,
/// confirmaciones) que no debe contar como interés del usuario.
const UI_TERMS: &[&str] = &[
    "hola", "buenas", "buenos", "buen", "gracias", "vale", "claro", "excelente", "genial",
    "assistant", "user", "usuario", "sistema", "system", "prompt", "comando", "hey", "hi",
    "hello", "thanks", "thank", "please", "yes", "sure", "right", "correct", "nada",
];

/// Conjunto completo de palabras vacías (ES + EN + UI).
fn stopwords() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        let mut s: HashSet<&'static str> = HashSet::with_capacity(220);
        s.extend(STOPWORDS_ES.iter().copied());
        s.extend(STOPWORDS_EN.iter().copied());
        s.extend(UI_TERMS.iter().copied());
        s
    })
}

// --------------------------------------------------------------------------- //
// Utilidades de texto
// --------------------------------------------------------------------------- //

/// Sustituye un carácter acentuado por su equivalente ASCII.
fn fold_accent(c: char) -> char {
    match c {
        'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' => 'a',
        'é' | 'è' | 'ë' | 'ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
        'ú' | 'ù' | 'ü' | 'û' => 'u',
        'ñ' => 'n',
        'ç' => 'c',
        'ý' | 'ÿ' => 'y',
        _ => c,
    }
}

/// Normaliza texto: minúsculas, sin acentos y con separadores no alfanuméricos
/// convertidos en espacios.  Es la base de la extracción de temas.
fn normalize_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let lower = c.to_lowercase().next().unwrap_or(c);
        if lower.is_alphanumeric() {
            out.push(fold_accent(lower));
        } else {
            out.push(' ');
        }
    }
    out
}

/// Divide el texto en palabras en minúsculas **conservando los acentos**.
/// Se usa para detectar marcadores ("mañana", "olvidé") sin perder información.
fn raw_words(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect()
}

/// ¿El token es sólo un número (o un número con separadores)?
fn is_numberish(t: &str) -> bool {
    !t.is_empty() && t.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',' || c == '.')
}

/// Léxico genérico que NO describe un interés: verbos sin contenido ("hacer",
/// "revisar"), nouns de proceso ("error", "problema") y adverbios temporales.
///
/// Se aplica **sólo** a la extracción de temas: los marcadores de pendiente,
/// logro y bloqueo sí necesitan esas palabras para ser detectados.
const GENERIC_TERMS: &[&str] = &[
    // verbos y sustantivos genéricos en español (forma normalizada, sin acentos)
    "arreglar", "arreglado", "revisar", "revision", "mejorar", "hacer", "hecho",
    "dejar", "dejarlo", "dejo", "poder", "puede", "puedo", "tengo", "tener",
    "tengo que", "doy", "dar", "poner", "puesto", "sacar", "cambiar", "anadir",
    "anadir", "borrar", "mover", "abrir", "cerrar", "leer", "escribir", "buscar",
    "encontrar", "empezar", "empece", "terminar", "termine", "continuar", "parar",
    "ayudar", "ayuda", "necesitar", "necesito", "querer", "quiero", "saber",
    "entender", "explicar", "decir", "digo", "trabajar", "trabajo", "probar",
    "intentar", "configurar", "instalar", "descargar", "compilar", "ejecutar",
    "solucionar", "solucionado", "funciona", "funcionando", "funciono", "error",
    "errores", "falla", "fallo", "fallos", "problema", "problemas", "imposible",
    "cosa", "cosas", "manera", "modo", "forma", "parte", "partes", "vez", "veces",
    "ahora", "hoy", "ayer", "manana", "luego", "entonces", "tambien", "ademas",
    "despues", "todavia", "queda", "quedo", "debe", "debo", "sigo", "vuelvo",
    "vale", "guiate", "ayudame", "favor", "cosa", "punto", "puntos", "nivel",
    "gente", "mundo", "vida", "tiempo", "veces", "momento", "sitio",
    // vocabulario de los marcadores: son señales, no intereses
    "conseguido", "logrado", "listo", "perfecto", "resuelto", "terminado",
    "completado", "pendiente", "pendientes", "olvide", "olvidar",
    "recordar", "recordatorio", "implementar", "fix", "aplicar", "aplicado",
    "da", "dado", "ya", "aqui", "alli", "sigue", "siguiente", "mismo", "misma",
    // equivalentes en inglés
    "thing", "things", "stuff", "work", "working", "works", "worked", "need",
    "needs", "needed", "want", "wants", "help", "issue", "issues", "problem",
    "problems", "error", "errors", "fail", "failed", "failing", "fix", "fixed",
    "try", "tried", "trying", "add", "remove", "update", "updated", "check",
    "checked", "set", "getting", "getting", "also", "maybe", "actually",
    "probably", "take", "look", "see", "give", "find", "tell", "ask", "feel",
    "day", "days", "time", "times", "way", "ways", "thing", "lot", "kind",
];

/// Palabras vacías para la extracción de temas (vacías + léxico genérico).
fn topic_stopwords() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        let mut s: HashSet<&'static str> = stopwords().clone();
        s.extend(GENERIC_TERMS.iter().copied());
        s
    })
}

/// Fragmentos de URL que no son temas: en un corpus técnico hay muchísimos
/// enlaces pegados y, sin filtrarlos, "https" o "com" dominan el ranking.
const URL_FRAGMENTS: &[&str] = &[
    "http", "https", "www", "com", "org", "net", "html", "htm", "php", "aspx", "ftp", "cdn",
    "href", "src", "index",
];

/// Longitud máxima (en caracteres) de un mensaje que se considera para la
/// extracción de temas.
///
/// Motivo: el usuario pega código, logs y documentos enteros.  Esos bloques
/// (a menudo decenas de KB) no describen sus intereses y, contados tal cual,
/// dominan el ranking.  Se corta por el principio del mensaje, que es donde
/// está la petición propia del usuario.
const TOPIC_UNIT_MAX_CHARS: usize = 600;

/// Tokens significativos de un texto (sin palabras vacías ni números).
fn content_tokens(text: &str) -> Vec<String> {
    let sw = topic_stopwords();
    normalize_text(text)
        .split_whitespace()
        .filter(|t| t.len() >= 2 && t.len() <= 20)
        .filter(|t| !sw.contains(*t) && !is_numberish(t) && !URL_FRAGMENTS.contains(t))
        .map(|t| t.to_string())
        .collect()
}

/// Inserta `s` en el vector sólo si no estaba (orden de primera aparición).
///
/// Caso rápido: si `s` ya es el último elemento (mensajes seguidos de la
/// misma conversación) no hay nada que buscar.
fn push_unique(v: &mut Vec<String>, s: &str) {
    if v.last().map(|x| x.as_str()) == Some(s) {
        return;
    }
    if !v.iter().any(|x| x == s) {
        v.push(s.to_string());
    }
}

/// Recorta a `n` decimales.
fn round4(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}

/// Recorta a `n` decimales.
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Fragmento de `text` alrededor de la primera aparición de `marker`.
fn snippet_around(text: &str, marker: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let lower: Vec<char> = text.to_lowercase().chars().collect();
    let needle: Vec<char> = marker.to_lowercase().chars().collect();
    let mut pos = 0usize;
    if !needle.is_empty() && lower.len() >= needle.len() {
        pos = lower
            .windows(needle.len())
            .position(|w| w == needle.as_slice())
            .unwrap_or(0);
    }
    let start = pos.saturating_sub(140);
    let end = (pos + needle.len() + 140).min(chars.len());
    let mut out: String = chars[start..end].iter().collect();
    out = out.trim().to_string();
    if start > 0 {
        out.insert(0, '…');
    }
    if end < chars.len() {
        out.push('…');
    }
    out
}

// --------------------------------------------------------------------------- //
// Utilidades de tiempo (sin chrono: aritmética civil propia)
// --------------------------------------------------------------------------- //

/// Fecha civil (año, mes 1-12, día) a partir de los días desde el epoch.
/// Algoritmo de Hinnant (`civil_from_days`), exacto para todo el rango i64.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Días desde el epoch para una fecha civil (inversa de `civil_from_days`).
#[cfg(test)]
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Índice de día de la semana con **lunes = 0**.
fn weekday_index(days: i64) -> usize {
    // 1970-01-01 fue jueves → con lunes=0, el desplazamiento es 3.
    (((days + 3) % 7 + 7) % 7) as usize
}

/// Etiquetas de los días de la semana (lunes = 0).
const WEEKDAY_LABELS: [&str; 7] = [
    "lunes",
    "martes",
    "miércoles",
    "jueves",
    "viernes",
    "sábado",
    "domingo",
];

/// Clave de mes `"YYYY-MM"` de un instante epoch.
fn month_key(ts: f64) -> String {
    let days = (ts / 86_400.0).floor() as i64;
    let (y, m, _) = civil_from_days(days);
    format!("{:04}-{:02}", y, m)
}

/// Fecha `"YYYY-MM-DD"` de un instante epoch.
fn iso_date(ts: f64) -> String {
    if ts <= 0.0 {
        return String::new();
    }
    let days = (ts / 86_400.0).floor() as i64;
    let (y, m, d) = civil_from_days(days);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// Marca de tiempo ISO-8601 en UTC.
fn iso_datetime(ts: f64) -> String {
    if ts <= 0.0 {
        return String::new();
    }
    let days = (ts / 86_400.0).floor() as i64;
    let secs = (ts - (days as f64) * 86_400.0).floor() as i64;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

/// Epoch actual en segundos.
fn now_epoch() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Normaliza una marca de tiempo a epoch en segundos.
///
/// Algunas fuentes guardan milisegundos y otras un *string* numérico: aquí se
/// unifica todo a segundos para que los histogramas por hora sean correctos.
fn ts_of(v: &Value) -> Option<f64> {
    let raw = match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }?;
    if !raw.is_finite() {
        return None;
    }
    if raw.abs() > 1e11 {
        Some(raw / 1000.0) // venían en milisegundos
    } else {
        Some(raw)
    }
}

// --------------------------------------------------------------------------- //
// Marcadores (pendientes, logros, bloqueos)
// --------------------------------------------------------------------------- //

/// Marcadores de intención pendiente (frases).
const PENDING_PHRASES: &[&str] = &[
    "tengo que",
    "tenemos que",
    "habia que",
    "habra que",
    "hay que",
    "no he podido",
    "no he hecho",
    "no he",
    "me falta",
    "queda pendiente",
    "pendiente de",
    "sin hacer",
    "debo",
    "necesito",
];

/// Marcadores de intención pendiente (palabras sueltas).
const PENDING_WORDS: &[&str] = &[
    "pendiente",
    "pendientes",
    "mañana",
    "olvidé",
    "olvide",
    "olvidar",
    "recordar",
    "recordatorio",
    "implementar",
    "fix",
    "faltan",
    "falta",
    "aplazar",
    "aplazado",
];

/// Señales de logro (frases de varias palabras).
const STRENGTH_PHRASES: &[&str] = &[
    "he conseguido",
    "he logrado",
    "ya funciona",
    "funciona correctamente",
    "funciona bien",
    "ya está",
    "ya esta",
    "lo he hecho",
    "va bien",
    "por fin",
    "todo listo",
];

/// Señales de logro (palabras sueltas).
///
/// Ojo: "funciona" **no** está aquí a propósito — sola es demasiado ambigua
/// ("no funciona" es un bloqueo).  El logro se detecta por la frase
/// "ya funciona" / "funciona bien" / "funciona correctamente".
const STRENGTH_WORDS: &[&str] = &[
    "solucionado",
    "resuelto",
    "terminado",
    "completado",
    "funcionando",
    "listo",
    "logrado",
    "conseguido",
    "perfecto",
];

/// Señales de bloqueo (frases).
const DIFFICULTY_PHRASES: &[&str] = &[
    "no funciona",
    "no me funciona",
    "no he podido",
    "me da error",
    "da error",
    "error de",
    "no se puede",
    "es imposible",
    "no entiendo",
    "atascado",
    "bloqueado",
    "no logro",
    "no consigo",
    "sigo con",
    "me cuesta",
];

/// Señales de bloqueo (palabras sueltas).
const DIFFICULTY_WORDS: &[&str] = &[
    "error",
    "errores",
    "falla",
    "fallo",
    "fallos",
    "problema",
    "problemas",
    "imposible",
    "frustrado",
    "frustrante",
    "roto",
    "bloqueo",
];

/// ¿Una palabra coincide con el marcador (con plural regular)?
fn word_matches(word: &str, marker: &str) -> bool {
    if word == marker {
        return true;
    }
    if word.len() > marker.len() && word.starts_with(marker) {
        let rest = &word[marker.len()..];
        return rest == "s" || rest == "es";
    }
    false
}

/// Palabras de negación: si anteceden al marcador, la señal se descarta
/// ("no funciona" no es un logro).
const NEGATIONS: &[&str] = &[
    "no", "nunca", "jamas", "sin", "todavia", "aun", "tampoco", "nada", "quedo",
];

/// ¿La palabra está precedida (hasta dos posiciones antes) por una negación?
fn negated_before(words: &[String], idx: usize) -> bool {
    let start = idx.saturating_sub(2);
    words[start..idx].iter().any(|w| NEGATIONS.contains(&w.as_str()))
}

/// Localiza la secuencia de palabras de `phrase` dentro de `words`.
fn find_phrase(words: &[String], phrase: &str) -> Option<usize> {
    let pw = raw_words(phrase);
    if pw.is_empty() || pw.len() > words.len() {
        return None;
    }
    (0..=(words.len() - pw.len())).find(|&i| words[i..i + pw.len()] == pw[..])
}

/// Busca marcadores en un texto.
///
/// * Las frases se buscan como **secuencia de palabras** (no como subcadena):
///   así "no funciona" se detecta aunque el usuario escriba "no funciona," o
///   "no  funciona".
/// * Las palabras sueltas se casan por palabra completa (para no confundir
///   "fix" con "prefijo") y admiten plural regular.
/// * Todo marker va con **comprobación de negación**: "no está solucionado"
///   no es un logro, "no he conseguido" tampoco.
///
/// Las frases se devuelven primero porque son más específicas que la palabra
/// suelta equivalente.
fn match_markers(text: &str, phrases: &[&str], words: &[&str]) -> Vec<String> {
    let raw = raw_words(text);
    let mut hits: Vec<String> = Vec::new();
    for p in phrases {
        if let Some(i) = find_phrase(&raw, p) {
            if !negated_before(&raw, i) {
                hits.push((*p).to_string());
            }
        }
    }
    for w in words {
        if hits.iter().any(|h| h == w) {
            continue;
        }
        let hit = raw
            .iter()
            .enumerate()
            .any(|(i, x)| word_matches(x, w) && !negated_before(&raw, i));
        if hit {
            hits.push((*w).to_string());
        }
    }
    let mut seen: HashSet<String> = HashSet::new();
    hits.retain(|h| seen.insert(h.clone()));
    hits
}

/// Marcadores de pendiente (incluye el `TODO:` clásico).
fn pending_markers(text: &str) -> Vec<String> {
    let mut hits = match_markers(text, PENDING_PHRASES, PENDING_WORDS);
    let lower = text.to_lowercase();
    let is_todo = lower.contains("todo:") || lower.lines().any(|l| {
        let t = l.trim_start();
        t == "todo" || t.starts_with("todo ")
    });
    if is_todo {
        hits.push("todo".to_string());
    }
    hits
}

/// Marcadores de logro.
fn strength_markers(text: &str) -> Vec<String> {
    match_markers(text, STRENGTH_PHRASES, STRENGTH_WORDS)
}

/// Marcadores de bloqueo.
fn difficulty_markers(text: &str) -> Vec<String> {
    match_markers(text, DIFFICULTY_PHRASES, DIFFICULTY_WORDS)
}

/// ¿El texto es una pregunta abierta del usuario?
fn is_question(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return false;
    }
    if t.contains('?') || t.contains('¿') {
        return true;
    }
    let lower = t.to_lowercase();
    const OPENERS: &[&str] = &[
        "cómo ",
        "como ",
        "por qué",
        "qué hago",
        "que hago",
        "qué es",
        "qué se",
        "cuál es",
        "cuál podría",
        "cuándo ",
        "donde ",
        "dónde ",
        "quién ",
        "no sé ",
        "no sabría",
        "para qué",
        "ayuda",
    ];
    OPENERS.iter().any(|o| lower.starts_with(o))
}

// --------------------------------------------------------------------------- //
// Almacén de conversaciones
// --------------------------------------------------------------------------- //

/// Directorio donde vive el almacén NDJSON de conversaciones.
fn sync_dir() -> PathBuf {
    PathBuf::from(crate::cockpit_server::state_dir()).join("sync")
}

/// Lee todas las conversaciones del almacén NDJSON (`<state_dir>/sync/*.ndjson`).
///
/// `crate::sync::load_store()` es privada, así que el analizador lee el mismo
/// formato directamente.  Se deduplica por `service::id` (gana la última línea)
/// igual que hace el motor de sincronización.
pub fn load_conversations() -> Vec<Value> {
    load_conversations_from(&sync_dir())
}

/// Lee las conversaciones de un directorio concreto (usado por los tests).
pub fn load_conversations_from(dir: &Path) -> Vec<Value> {
    let mut map: HashMap<String, Value> = HashMap::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("ndjson") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let svc = v.get("service").and_then(|x| x.as_str()).unwrap_or("");
            let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("");
            if svc.is_empty() || id.is_empty() {
                continue;
            }
            map.insert(format!("{}::{}", svc, id), v);
        }
    }
    let mut out: Vec<Value> = map.into_values().collect();
    out.sort_by(|a, b| {
        let ta = a.get("updated_at").and_then(ts_of).unwrap_or(0.0);
        let tb = b.get("updated_at").and_then(ts_of).unwrap_or(0.0);
        tb.partial_cmp(&ta).unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

// --------------------------------------------------------------------------- //
// Aplanado de conversaciones
// --------------------------------------------------------------------------- //

/// Un mensaje aplanado, con la procedencia necesaria para citarlo.
#[derive(Clone, Debug)]
struct FlatMsg {
    service: String,
    conv_id: String,
    title: String,
    role: String,
    content: String,
    ts: f64,
}

impl FlatMsg {
    /// ¿Es un mensaje del usuario (no del asistente)?
    fn is_user(&self) -> bool {
        matches!(
            self.role.to_lowercase().as_str(),
            "user" | "human" | "usuario" | "yo" | "prompt"
        )
    }
}

/// Aplana las conversaciones a una lista de mensajes.
fn flatten(convs: &[Value]) -> Vec<FlatMsg> {
    let mut out: Vec<FlatMsg> = Vec::new();
    for c in convs {
        let service = c
            .get("service")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown")
            .to_string();
        let conv_id = c.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let title = c
            .get("title")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let updated = c.get("updated_at").and_then(ts_of).unwrap_or(0.0);
        let msgs = c
            .get("messages")
            .and_then(|m| m.as_array())
            .cloned()
            .unwrap_or_default();
        for m in msgs {
            let content = m
                .get("content")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let role = m.get("role").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let mut ts = m.get("ts").and_then(ts_of).unwrap_or(0.0);
            if ts <= 0.0 {
                ts = updated;
            }
            out.push(FlatMsg {
                service: service.clone(),
                conv_id: conv_id.clone(),
                title: title.clone(),
                role,
                content,
                ts,
            });
        }
    }
    out
}

/// Mensajes que cuentan como "voz del usuario".
///
/// Si el corpus no tiene ningún mensaje de usuario (raro, pero posible en
/// exportaciones antiguas), se usan todos los mensajes.
fn analysis_messages(flat: &[FlatMsg]) -> Vec<&FlatMsg> {
    let users: Vec<&FlatMsg> = flat.iter().filter(|m| m.is_user()).collect();
    if users.is_empty() {
        flat.iter().collect()
    } else {
        users
    }
}

/// Unidades de texto para la extracción de temas: título de cada conversación
/// + contenido de los mensajes del usuario.
///
/// Las unidades van **agrupadas por conversación**, lo que permite deduplicar
/// las conversaciones de cada término de forma barata (el caso normal es que
/// el término ya sea el último de la lista).
fn topic_units(flat: &[FlatMsg]) -> Vec<(String, String)> {
    let use_users = flat.iter().any(|m| m.is_user());
    let mut order: Vec<String> = Vec::new();
    let mut by_conv: HashMap<String, Vec<&FlatMsg>> = HashMap::new();
    for m in flat {
        if !by_conv.contains_key(&m.conv_id) {
            order.push(m.conv_id.clone());
        }
        by_conv.entry(m.conv_id.clone()).or_default().push(m);
    }
    let mut units: Vec<(String, String)> = Vec::new();
    for cid in order {
        let msgs = &by_conv[&cid];
        let title = msgs.first().map(|m| m.title.clone()).unwrap_or_default();
        if !title.trim().is_empty() {
            units.push((cid.clone(), title));
        }
        for m in msgs.iter() {
            if use_users && !m.is_user() {
                continue;
            }
            // Se corta por el principio: lo que el usuario pega (código, logs)
            // no es su interés, y sus mensajes llegan a medir decenas de KB.
            let head: String = m.content.chars().take(TOPIC_UNIT_MAX_CHARS).collect();
            if !head.trim().is_empty() {
                units.push((cid.clone(), head));
            }
        }
    }
    units
}

// --------------------------------------------------------------------------- //
// Temas (términos y bigramas)
// --------------------------------------------------------------------------- //

/// Un tema detectado en el corpus.
#[derive(Clone, Debug)]
struct TopicEntry {
    term: String,
    kind: &'static str, // "term" | "phrase"
    mentions: usize,
    convs: Vec<String>,
}

/// Resultado de la extracción de temas.
struct Topics {
    terms: Vec<TopicEntry>,
    total_mentions: usize,
}

/// Extrae temas: unigramas (siempre) y bigramas (sólo si se repiten ≥ 2).
///
/// Los bigramas sin repetición son ruido, así que se descartan; así el ranking
/// mixes lo que de verdad se repite en el discurso del usuario.
fn extract_topics(units: &[(String, String)]) -> Topics {
    let mut uni: HashMap<String, (usize, Vec<String>)> = HashMap::new();
    let mut bi: HashMap<String, (usize, Vec<String>)> = HashMap::new();
    for (cid, text) in units {
        let toks = content_tokens(text);
        for (i, t) in toks.iter().enumerate() {
            let e = uni.entry(t.clone()).or_insert((0, Vec::new()));
            e.0 += 1;
            push_unique(&mut e.1, cid);
            if i + 1 < toks.len() {
                let phrase = format!("{} {}", t, toks[i + 1]);
                let e = bi.entry(phrase).or_insert((0, Vec::new()));
                e.0 += 1;
                push_unique(&mut e.1, cid);
            }
        }
    }
    let mut terms: Vec<TopicEntry> = Vec::new();
    for (term, (n, convs)) in uni {
        terms.push(TopicEntry {
            term,
            kind: "term",
            mentions: n,
            convs,
        });
    }
    for (term, (n, convs)) in bi {
        if n >= 2 {
            terms.push(TopicEntry {
                term,
                kind: "phrase",
                mentions: n,
                convs,
            });
        }
    }
    // Orden determinista: menciones ↓, luego término más corto, luego alfabético.
    terms.sort_by(|a, b| {
        b.mentions
            .cmp(&a.mentions)
            .then(a.term.len().cmp(&b.term.len()))
            .then(a.term.cmp(&b.term))
    });
    let total: usize = terms.iter().map(|t| t.mentions).sum();
    Topics { terms, total_mentions: total }
}

/// Cuenta sólo unigramas (para ventanas temporales, donde los bigramas añaden
/// demasiado ruido a la comparación de participaciones).
fn extract_unigrams(units: &[(String, String)]) -> Topics {
    let mut uni: HashMap<String, (usize, Vec<String>)> = HashMap::new();
    for (cid, text) in units {
        for t in content_tokens(text) {
            let e = uni.entry(t).or_insert((0, Vec::new()));
            e.0 += 1;
            push_unique(&mut e.1, cid);
        }
    }
    let mut terms: Vec<TopicEntry> = uni
        .into_iter()
        .map(|(term, (n, convs))| TopicEntry {
            term,
            kind: "term",
            mentions: n,
            convs,
        })
        .collect();
    terms.sort_by(|a, b| {
        b.mentions
            .cmp(&a.mentions)
            .then(a.term.len().cmp(&b.term.len()))
            .then(a.term.cmp(&b.term))
    });
    let total: usize = terms.iter().map(|t| t.mentions).sum();
    Topics { terms, total_mentions: total }
}

// --------------------------------------------------------------------------- //
// Constructores de las secciones del perfil
// --------------------------------------------------------------------------- //

/// Metadatos del corpus analizado.
fn build_corpus(convs: &[Value], flat: &[FlatMsg]) -> Value {
    let mut services: Vec<String> = Vec::new();
    for c in convs {
        let s = c
            .get("service")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown")
            .to_string();
        if !services.contains(&s) {
            services.push(s);
        }
    }
    services.sort();
    let chars: usize = flat.iter().map(|m| m.content.chars().count()).sum();
    let mut first = f64::MAX;
    let mut last = 0.0f64;
    for m in flat {
        if m.ts > 0.0 {
            first = first.min(m.ts);
            last = last.max(m.ts);
        }
    }
    if first == f64::MAX {
        first = 0.0;
    }
    let span_days = if last > first {
        (last - first) / 86_400.0
    } else {
        0.0
    };
    json!({
        "conversations": convs.len(),
        "messages": flat.len(),
        "chars": chars,
        "services": services,
        "first_at": first,
        "last_at": last,
        "first_at_iso": iso_datetime(first),
        "last_at_iso": iso_datetime(last),
        "span_days": round2(span_days),
    })
}

/// Histograma de actividad: por hora, por día de la semana y por mes.
fn build_activity(flat: &[FlatMsg]) -> Value {
    let mut hours = vec![0usize; 24];
    let mut weekdays = vec![0usize; 7];
    let mut months: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
    let mut month_convs: HashMap<String, HashSet<String>> = HashMap::new();
    let mut days_seen: HashSet<i64> = HashSet::new();
    let mut total = 0usize;

    for m in flat {
        if m.ts <= 0.0 {
            continue;
        }
        total += 1;
        let day = (m.ts / 86_400.0).floor() as i64;
        days_seen.insert(day);
        let h = (((m.ts % 86_400.0) / 3600.0).floor() as usize).min(23);
        hours[h] += 1;
        weekdays[weekday_index(day)] += 1;
        let key = month_key(m.ts);
        let e = months.entry(key.clone()).or_insert((0, 0, 0));
        e.0 += 1;
        e.2 += m.content.chars().count();
        month_convs.entry(key).or_default().insert(m.conv_id.clone());
    }

    let share = |n: usize| -> f64 {
        if total == 0 {
            0.0
        } else {
            round4(n as f64 / total as f64)
        }
    };

    let by_hour: Vec<Value> = (0..24)
        .map(|h| {
            json!({
                "hour": h,
                "label": format!("{:02}:00", h),
                "count": hours[h],
                "share": share(hours[h]),
            })
        })
        .collect();

    let by_weekday: Vec<Value> = (0..7)
        .map(|w| {
            json!({
                "weekday": w,
                "label": WEEKDAY_LABELS[w],
                "count": weekdays[w],
                "share": share(weekdays[w]),
            })
        })
        .collect();

    let by_month: Vec<Value> = months
        .iter()
        .map(|(k, (msgs, _, chars))| {
            let convs = month_convs.get(k).map(|s| s.len()).unwrap_or(0);
            json!({
                "month": k,
                "messages": msgs,
                "conversations": convs,
                "chars": chars,
                "share": share(*msgs),
            })
        })
        .collect();

    let peak_hour = hours
        .iter()
        .enumerate()
        .max_by_key(|(_, c)| **c)
        .map(|(h, c)| (h, *c));
    let peak_weekday = weekdays
        .iter()
        .enumerate()
        .max_by_key(|(_, c)| **c)
        .map(|(w, c)| (w, *c));
    let night: usize = hours[23] + hours[0] + hours[1] + hours[2] + hours[3] + hours[4] + hours[5];

    json!({
        "by_hour": by_hour,
        "by_weekday": by_weekday,
        "by_month": by_month,
        "total_messages": total,
        "active_days": days_seen.len(),
        "peak_hour": peak_hour.map(|(h, c)| json!({"hour": h, "label": format!("{:02}:00", h), "count": c})),
        "peak_weekday": peak_weekday.map(|(w, c)| json!({"weekday": w, "label": WEEKDAY_LABELS[w], "count": c})),
        "late_night_share": share(night),
        "late_night_messages": night,
        "timezone": "UTC",
    })
}

/// Agrupación temática por co-ocurrencia de términos.
fn build_themes(flat: &[FlatMsg], units: &[(String, String)], topics: &Topics) -> Vec<Value> {
    let top: Vec<&TopicEntry> = topics.terms.iter().filter(|t| t.kind == "term").take(24).collect();
    let topset: HashSet<&str> = top.iter().map(|t| t.term.as_str()).collect();

    let mut titles: HashMap<String, String> = HashMap::new();
    for m in flat {
        titles
            .entry(m.conv_id.clone())
            .or_insert_with(|| m.title.clone());
    }

    // Términos presentes en cada conversación.
    let mut per_conv: HashMap<String, HashSet<String>> = HashMap::new();
    for (cid, text) in units {
        let set = per_conv.entry(cid.clone()).or_default();
        for t in content_tokens(text) {
            if topset.contains(t.as_str()) {
                set.insert(t);
            }
        }
    }

    // Pares co-ocurrentes.
    let mut pairs: HashMap<(String, String), (usize, Vec<String>)> = HashMap::new();
    let mut conv_ids: Vec<String> = per_conv.keys().cloned().collect();
    conv_ids.sort();
    for cid in &conv_ids {
        let set = &per_conv[cid];
        let mut items: Vec<&String> = set.iter().collect();
        items.sort();
        for i in 0..items.len() {
            for j in (i + 1)..items.len() {
                let key = (items[i].clone(), items[j].clone());
                let e = pairs.entry(key).or_insert((0, Vec::new()));
                e.0 += 1;
                e.1.push(cid.clone());
            }
        }
    }

    let mut cand: Vec<((String, String), (usize, Vec<String>))> = pairs.into_iter().collect();
    cand.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then(a.0 .0.cmp(&b.0 .0)));

    let mut used: HashSet<String> = HashSet::new();
    let mut themes: Vec<Value> = Vec::new();
    for ((a, b), (n, convs)) in cand {
        if n < 2 {
            break; // el resto tiene menos co-ocurrencias
        }
        if used.contains(&a) || used.contains(&b) {
            continue; // no repetimos términos ya cubiertos por otro tema
        }
        used.insert(a.clone());
        used.insert(b.clone());
        let samples: Vec<String> = convs
            .iter()
            .filter_map(|c| titles.get(c).cloned())
            .filter(|t| !t.is_empty())
            .take(5)
            .collect();
        themes.push(json!({
            "label": format!("{} + {}", a, b),
            "count": n,
            "terms": [a, b],
            "sample_titles": samples,
        }));
        if themes.len() >= 12 {
            break;
        }
    }

    // Sin co-ocurrencias claras: se usan los términos individuales más fuertes.
    if themes.is_empty() {
        for t in topics.terms.iter().filter(|t| t.kind == "term").take(6) {
            let samples: Vec<String> = t
                .convs
                .iter()
                .filter_map(|c| titles.get(c).cloned())
                .filter(|s| !s.is_empty())
                .take(5)
                .collect();
            themes.push(json!({
                "label": t.term.clone(),
                "count": t.mentions,
                "terms": [t.term.clone()],
                "sample_titles": samples,
            }));
        }
    }
    themes
}

/// Temas que se repiten en varias conversaciones (interés sostenido).
fn build_recurring(topics: &Topics) -> Vec<Value> {
    let mut out: Vec<Value> = topics
        .terms
        .iter()
        .filter(|t| t.convs.len() >= 2)
        .map(|t| {
            json!({
                "term": t.term,
                "kind": t.kind,
                "count": t.convs.len(),
                "mentions": t.mentions,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        b["count"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["count"].as_u64().unwrap_or(0))
            .then(
                b["mentions"]
                    .as_u64()
                    .unwrap_or(0)
                    .cmp(&a["mentions"].as_u64().unwrap_or(0)),
            )
    });
    out.truncate(20);
    out
}

/// Estadísticas cuantitativas del corpus.
fn build_stats(convs: &[Value], flat: &[FlatMsg]) -> Value {
    let n = flat.len();
    let chars: usize = flat.iter().map(|m| m.content.chars().count()).sum();
    let avg = if n > 0 { chars as f64 / n as f64 } else { 0.0 };
    let longest = flat.iter().max_by_key(|m| m.content.chars().count());
    let longest_json = match longest {
        Some(m) if !m.content.is_empty() => json!({
            "chars": m.content.chars().count(),
            "conversation_id": m.conv_id.clone(),
            "source_service": m.service.clone(),
            "ts": m.ts,
            "snippet": truncate_chars(&m.content, 280),
        }),
        _ => Value::Null,
    };
    let mut models: HashMap<String, usize> = HashMap::new();
    for c in convs {
        let model = c
            .get("model")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if model.is_empty() {
            continue;
        }
        *models.entry(model).or_insert(0) += 1;
    }
    let mut top_models: Vec<(String, usize)> = models.into_iter().collect();
    top_models.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    top_models.truncate(10);
    json!({
        "avg_msg_len": round2(avg),
        "longest_msg": longest_json,
        "msgs_per_conversation": if convs.is_empty() {
            0.0
        } else {
            round2(n as f64 / convs.len() as f64)
        },
        "top_models": top_models
            .iter()
            .map(|(m, c)| json!({"model": m, "count": c}))
            .collect::<Vec<Value>>(),
    })
}

/// Detecta pendientes, logros, bloqueos y preguntas abiertas.
fn scan_markers(flat: &[FlatMsg]) -> (Vec<Value>, Vec<Value>, Vec<Value>, Vec<Value>) {
    let mut pending: Vec<Value> = Vec::new();
    let mut strengths: Vec<Value> = Vec::new();
    let mut difficulties: Vec<Value> = Vec::new();
    let mut questions: Vec<Value> = Vec::new();

    for m in analysis_messages(flat) {
        let text = m.content.trim();
        if text.is_empty() {
            continue;
        }

        let p_hits = pending_markers(text);
        if !p_hits.is_empty() {
            let body = if text.chars().count() <= 300 {
                text.to_string()
            } else {
                snippet_around(text, &p_hits[0])
            };
            pending.push(json!({
                "text": body,
                "marker": p_hits[0],
                "markers": p_hits,
                "source_service": m.service.clone(),
                "conversation_id": m.conv_id.clone(),
                "ts": m.ts,
                "when": iso_datetime(m.ts),
            }));
        }

        let s_hits = strength_markers(text);
        if !s_hits.is_empty() {
            strengths.push(json!({
                "evidence": format!("señal de logro: \"{}\"", s_hits[0]),
                "markers": s_hits,
                "conversation_id": m.conv_id.clone(),
                "source_service": m.service.clone(),
                "ts": m.ts,
                "when": iso_datetime(m.ts),
                "quote": snippet_around(text, &s_hits[0]),
            }));
        }

        let d_hits = difficulty_markers(text);
        if !d_hits.is_empty() {
            difficulties.push(json!({
                "evidence": format!("señal de bloqueo: \"{}\"", d_hits[0]),
                "markers": d_hits,
                "conversation_id": m.conv_id.clone(),
                "source_service": m.service.clone(),
                "ts": m.ts,
                "when": iso_datetime(m.ts),
                "quote": snippet_around(text, &d_hits[0]),
            }));
        }

        if is_question(text) {
            questions.push(json!({
                "text": truncate_chars(text, 300),
                "source_service": m.service.clone(),
                "conversation_id": m.conv_id.clone(),
                "ts": m.ts,
                "when": iso_datetime(m.ts),
            }));
        }
    }

    pending.truncate(200);
    strengths.truncate(100);
    difficulties.truncate(100);
    questions.truncate(100);
    (pending, strengths, difficulties, questions)
}

// --------------------------------------------------------------------------- //
// API pública: análisis
// --------------------------------------------------------------------------- //

/// Analiza **todo** el almacén y devuelve el perfil del usuario.
pub fn analyze() -> Value {
    let convs = load_conversations();
    analyze_conversations(&convs)
}

/// Analiza un conjunto de conversaciones y devuelve el perfil del usuario.
///
/// Es el núcleo del módulo (todo lo demás lo envuelve), y está separado para
/// que los tests puedan ejercitarlo sin tocar el *state dir* real.
pub fn analyze_conversations(convs: &[Value]) -> Value {
    let flat = flatten(convs);
    let corpus = build_corpus(convs, &flat);
    let units = topic_units(&flat);
    let topics = extract_topics(&units);

    let interests: Vec<Value> = topics
        .terms
        .iter()
        .take(20)
        .map(|t| {
            let share = if topics.total_mentions > 0 {
                t.mentions as f64 / topics.total_mentions as f64
            } else {
                0.0
            };
            json!({
                "topic": t.term,
                "kind": t.kind,
                "mentions": t.mentions,
                "share": round4(share),
                "evidence": t.convs.iter().take(5).collect::<Vec<&String>>(),
            })
        })
        .collect();

    let activity = build_activity(&flat);
    let themes = build_themes(&flat, &units, &topics);
    let (pending, strengths, difficulties, open_questions) = scan_markers(&flat);
    let recurring_topics = build_recurring(&topics);
    let stats = build_stats(convs, &flat);
    let now = now_epoch();

    json!({
        "ok": true,
        "generated_at": now,
        "generated_at_iso": iso_datetime(now),
        "corpus": corpus,
        "interests": interests,
        "activity": activity,
        "themes": themes,
        "pending": pending,
        "strengths": strengths,
        "difficulties": difficulties,
        "recurring_topics": recurring_topics,
        "open_questions": open_questions,
        "stats": stats,
        "method": {
            "units": "títulos de conversación + mensajes del usuario",
            "stopwords": "ES + EN + términos de interfaz + léxico genérico",
            "markers": "pendiente / logro / bloqueo por léxico (ES/EN), con negación",
            "timezone": "UTC",
            "note": "heurística estadística sobre texto escrito; no es un diagnóstico clínico",
        },
    })
}

// --------------------------------------------------------------------------- //
// API pública: deriva temática
// --------------------------------------------------------------------------- //

/// Compara la ventana reciente con la anterior: ¿en qué está evolucionando el
/// usuario?
pub fn topic_drift(window_days: u32) -> Value {
    let convs = load_conversations();
    topic_drift_from(&convs, window_days)
}

/// Núcleo de `topic_drift` sobre un conjunto de conversaciones.
pub fn topic_drift_from(convs: &[Value], window_days: u32) -> Value {
    let flat = flatten(convs);
    let win_days = if window_days == 0 { 7 } else { window_days.min(365) };
    let w = win_days as f64;
    let anchor = flat
        .iter()
        .map(|m| m.ts)
        .filter(|t| *t > 0.0)
        .fold(0.0f64, f64::max);

    if anchor <= 0.0 {
        return json!({
            "ok": true,
            "window_days": win_days,
            "earlier": [],
            "later": [],
            "emerging": [],
            "fading": [],
            "note": "no hay mensajes con marca de tiempo en el corpus",
        });
    }

    let later_start = anchor - w * 86_400.0;
    let earlier_start = anchor - 2.0 * w * 86_400.0;
    let mut later_units: Vec<(String, String)> = Vec::new();
    let mut earlier_units: Vec<(String, String)> = Vec::new();
    for m in analysis_messages(&flat) {
        if m.content.trim().is_empty() {
            continue;
        }
        if m.ts > later_start {
            later_units.push((m.conv_id.clone(), m.content.clone()));
        } else if m.ts > earlier_start {
            earlier_units.push((m.conv_id.clone(), m.content.clone()));
        }
    }

    let later = extract_unigrams(&later_units);
    let earlier = extract_unigrams(&earlier_units);

    let share_of = |topics: &Topics, term: &str| -> f64 {
        if topics.total_mentions == 0 {
            return 0.0;
        }
        topics
            .terms
            .iter()
            .find(|t| t.term == term)
            .map(|t| t.mentions as f64 / topics.total_mentions as f64)
            .unwrap_or(0.0)
    };

    let earlier_list: Vec<Value> = earlier
        .terms
        .iter()
        .take(20)
        .map(|t| {
            let share = if earlier.total_mentions > 0 {
                t.mentions as f64 / earlier.total_mentions as f64
            } else {
                0.0
            };
            json!({"topic": t.term, "share": round4(share), "mentions": t.mentions})
        })
        .collect();

    let later_list: Vec<Value> = later
        .terms
        .iter()
        .take(20)
        .map(|t| {
            let share = if later.total_mentions > 0 {
                t.mentions as f64 / later.total_mentions as f64
            } else {
                0.0
            };
            json!({"topic": t.term, "share": round4(share), "mentions": t.mentions})
        })
        .collect();

    // Emergentes: participación que crece en la ventana reciente.
    let mut emerging: Vec<Value> = Vec::new();
    for t in later.terms.iter().take(40) {
        let l_share = if later.total_mentions > 0 {
            t.mentions as f64 / later.total_mentions as f64
        } else {
            0.0
        };
        let e_share = share_of(&earlier, &t.term);
        let delta = l_share - e_share;
        if delta >= 0.01 && t.mentions >= 2 {
            emerging.push(json!({
                "topic": t.term,
                "earlier_share": round4(e_share),
                "later_share": round4(l_share),
                "delta": round4(delta),
                "mentions": t.mentions,
            }));
        }
    }
    emerging.sort_by(|a, b| {
        b["delta"]
            .as_f64()
            .unwrap_or(0.0)
            .partial_cmp(&a["delta"].as_f64().unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    emerging.truncate(15);

    // En desvanecimiento: participación que cae.
    let mut fading: Vec<Value> = Vec::new();
    for t in earlier.terms.iter().take(40) {
        let e_share = if earlier.total_mentions > 0 {
            t.mentions as f64 / earlier.total_mentions as f64
        } else {
            0.0
        };
        let l_share = share_of(&later, &t.term);
        let delta = l_share - e_share;
        if delta <= -0.01 && t.mentions >= 2 {
            fading.push(json!({
                "topic": t.term,
                "earlier_share": round4(e_share),
                "later_share": round4(l_share),
                "delta": round4(delta),
                "mentions": t.mentions,
            }));
        }
    }
    fading.sort_by(|a, b| {
        a["delta"]
            .as_f64()
            .unwrap_or(0.0)
            .partial_cmp(&b["delta"].as_f64().unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    fading.truncate(15);

    json!({
        "ok": true,
        "window_days": win_days,
        "anchor": anchor,
        "anchor_iso": iso_datetime(anchor),
        "earlier_range": [iso_date(earlier_start), iso_date(later_start)],
        "later_range": [iso_date(later_start), iso_date(anchor)],
        "earlier": earlier_list,
        "later": later_list,
        "emerging": emerging,
        "fading": fading,
    })
}

// --------------------------------------------------------------------------- //
// API pública: resumen diario
// --------------------------------------------------------------------------- //

/// Resumen por día de la actividad (conversaciones, mensajes, temas y
/// pendientes de cada día).
pub fn daily_digest(days: u32) -> Value {
    let convs = load_conversations();
    daily_digest_from(&convs, days)
}

/// Núcleo de `daily_digest` sobre un conjunto de conversaciones.
pub fn daily_digest_from(convs: &[Value], days: u32) -> Value {
    let flat = flatten(convs);
    let d = if days == 0 { 7 } else { days.min(365) } as usize;
    let anchor = flat
        .iter()
        .map(|m| m.ts)
        .filter(|t| *t > 0.0)
        .fold(0.0f64, f64::max);

    if anchor <= 0.0 {
        return json!({
            "ok": true,
            "days": d,
            "by_day": [],
            "note": "no hay mensajes con marca de tiempo en el corpus",
        });
    }

    let anchor_day = (anchor / 86_400.0).floor() as i64;
    let first_day = anchor_day - (d as i64) + 1;

    // Agrupar mensajes por día.
    let mut by_day: HashMap<i64, Vec<&FlatMsg>> = HashMap::new();
    for m in flat.iter() {
        if m.ts <= 0.0 {
            continue;
        }
        let day = (m.ts / 86_400.0).floor() as i64;
        if day < first_day || day > anchor_day {
            continue;
        }
        by_day.entry(day).or_default().push(m);
    }

    let mut days_vec: Vec<Value> = Vec::new();
    let mut total_msgs = 0usize;
    let mut total_convs: HashSet<String> = HashSet::new();
    let mut day_list: Vec<i64> = by_day.keys().cloned().collect();
    day_list.sort();
    for day in day_list {
        let msgs = &by_day[&day];
        let convs_set: HashSet<&String> = msgs.iter().map(|m| &m.conv_id).collect();
        let chars: usize = msgs.iter().map(|m| m.content.chars().count()).sum();
        total_msgs += msgs.len();
        for c in &convs_set {
            total_convs.insert(c.to_string());
        }

        // Temas dominantes del día (unigramas).
        let units: Vec<(String, String)> = msgs
            .iter()
            .filter(|m| m.is_user())
            .map(|m| (m.conv_id.clone(), m.content.clone()))
            .collect();
        let day_topics = extract_unigrams(&units);
        let topics: Vec<String> = day_topics
            .terms
            .iter()
            .filter(|t| t.mentions >= 2)
            .take(3)
            .map(|t| t.term.clone())
            .collect();

        // Pendientes de ese día.
        let mut pending: Vec<Value> = Vec::new();
        for m in msgs.iter().filter(|m| m.is_user()) {
            let text = m.content.trim();
            if text.is_empty() {
                continue;
            }
            let hits = pending_markers(text);
            if hits.is_empty() {
                continue;
            }
            let body = if text.chars().count() <= 300 {
                text.to_string()
            } else {
                snippet_around(text, &hits[0])
            };
            pending.push(json!({
                "text": body,
                "marker": hits[0],
                "conversation_id": m.conv_id.clone(),
                "source_service": m.service.clone(),
                "ts": m.ts,
            }));
        }

        days_vec.push(json!({
            "date": iso_date(day as f64 * 86_400.0),
            "conversations": convs_set.len(),
            "messages": msgs.len(),
            "chars": chars,
            "topics": topics,
            "pending": pending,
        }));
    }

    json!({
        "ok": true,
        "generated_at": now_epoch(),
        "days": d,
        "from": iso_date(first_day as f64 * 86_400.0),
        "to": iso_date(anchor_day as f64 * 86_400.0),
        "anchor_iso": iso_datetime(anchor),
        "days_with_activity": days_vec.len(),
        "total_messages": total_msgs,
        "total_conversations": total_convs.len(),
        "by_day": days_vec,
    })
}

// --------------------------------------------------------------------------- //
// API pública: informe para profesionales
// --------------------------------------------------------------------------- //

/// Elemento de una sección del informe.
struct Item {
    text: String,
    citation: Option<String>,
}

/// Sección del informe.
struct Section {
    id: &'static str,
    title: &'static str,
    summary: String,
    items: Vec<Item>,
}

/// Informe listo para un profesional (psicólogo, coach, terapeuta).
///
/// Con `markdown = true` devuelve `{ ok, markdown }` con un documento Markdown;
/// con `markdown = false` devuelve el mismo contenido como secciones JSON.
pub fn briefing(markdown: bool) -> Value {
    let convs = load_conversations();
    briefing_from(&convs, markdown)
}

/// Núcleo de `briefing` sobre un conjunto de conversaciones.
pub fn briefing_from(convs: &[Value], markdown: bool) -> Value {
    let profile = analyze_conversations(convs);
    let drift = topic_drift_from(convs, 14);
    let sections = build_sections(&profile, &drift);

    if markdown {
        let md = render_markdown(&profile, &sections);
        json!({
            "ok": true,
            "generated_at": profile["generated_at"].clone(),
            "generated_at_iso": profile["generated_at_iso"].clone(),
            "corpus": profile["corpus"].clone(),
            "markdown": md,
        })
    } else {
        let sections_json: Vec<Value> = sections
            .iter()
            .enumerate()
            .map(|(i, s)| {
                json!({
                    "index": i + 1,
                    "id": s.id,
                    "title": s.title,
                    "summary": s.summary,
                    "items": s.items
                        .iter()
                        .map(|it| json!({"text": it.text, "citation": it.citation}))
                        .collect::<Vec<Value>>(),
                })
            })
            .collect();
        json!({
            "ok": true,
            "generated_at": profile["generated_at"].clone(),
            "generated_at_iso": profile["generated_at_iso"].clone(),
            "corpus": profile["corpus"].clone(),
            "sections": sections_json,
        })
    }
}

/// Construye las secciones del informe a partir del perfil.
fn build_sections(profile: &Value, drift: &Value) -> Vec<Section> {
    let corpus = &profile["corpus"];
    let convs = corpus["conversations"].as_u64().unwrap_or(0);
    let msgs = corpus["messages"].as_u64().unwrap_or(0);
    let chars = corpus["chars"].as_u64().unwrap_or(0);
    let services = corpus["services"].as_array().cloned().unwrap_or_default();
    let span = corpus["span_days"].as_f64().unwrap_or(0.0);
    let first_iso = corpus["first_at_iso"].as_str().unwrap_or("").to_string();
    let last_iso = corpus["last_at_iso"].as_str().unwrap_or("").to_string();

    let interests = profile["interests"].as_array().cloned().unwrap_or_default();
    let pending = profile["pending"].as_array().cloned().unwrap_or_default();
    let strengths = profile["strengths"].as_array().cloned().unwrap_or_default();
    let difficulties = profile["difficulties"].as_array().cloned().unwrap_or_default();
    let questions = profile["open_questions"].as_array().cloned().unwrap_or_default();
    let themes = profile["themes"].as_array().cloned().unwrap_or_default();
    let activity = &profile["activity"];

    // --- 1. Resumen ejecutivo -------------------------------------------- //
    let top_names: Vec<String> = interests
        .iter()
        .take(3)
        .map(|i| i["topic"].as_str().unwrap_or("").to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let peak_hour = activity["peak_hour"]["label"].as_str().unwrap_or("—").to_string();
    let late_night = activity["late_night_share"].as_f64().unwrap_or(0.0);
    let summary = format!(
        "El historial analizado contiene {} conversaciones y {} mensajes ({} caracteres) de {} servicios, \
         repartidos a lo largo de {:.1} días. Los temas que más se repiten son {}. \
         Se han detectado {} mensajes con intención de pendiente, {} señales de logro y {} señales de bloqueo. \
         La actividad se concentra en las horas {} y el {:.1}% de los mensajes cae entre las 23:00 y las 05:00 (UTC).",
        convs,
        msgs,
        chars,
        services.len(),
        span,
        if top_names.is_empty() {
            "(sin temas dominantes)".to_string()
        } else {
            top_names.join(", ")
        },
        pending.len(),
        strengths.len(),
        difficulties.len(),
        peak_hour,
        late_night * 100.0
    );

    let mut items = vec![
        Item {
            text: format!(
                "Corpus: {} conversaciones · {} mensajes · {} caracteres · {} días de historial",
                convs, msgs, chars, span
            ),
            citation: None,
        },
        Item {
            text: format!(
                "Servicios analizados: {}",
                if services.is_empty() {
                    "—".to_string()
                } else {
                    services
                        .iter()
                        .filter_map(|s| s.as_str())
                        .collect::<Vec<&str>>()
                        .join(", ")
                }
            ),
            citation: None,
        },
        Item {
            text: format!(
                "Ventana temporal: {} → {}",
                if first_iso.is_empty() { "—" } else { &first_iso },
                if last_iso.is_empty() { "—" } else { &last_iso }
            ),
            citation: None,
        },
        Item {
            text: format!(
                "Pendientes detectados: {} · logros: {} · bloqueos: {} · preguntas abiertas: {}",
                pending.len(),
                strengths.len(),
                difficulties.len(),
                questions.len()
            ),
            citation: None,
        },
    ];
    if let Some(i0) = interests.first() {
        items.push(Item {
            text: format!(
                "Tema dominante: \"{}\" ({} menciones, {}% del total)",
                i0["topic"].as_str().unwrap_or(""),
                i0["mentions"].as_u64().unwrap_or(0),
                (i0["share"].as_f64().unwrap_or(0.0) * 100.0 * 10.0).round() / 10.0
            ),
            citation: None,
        });
    }

    let mut sections = vec![Section {
        id: "resumen",
        title: "Resumen ejecutivo",
        summary,
        items,
    }];

    // --- 2. Corpus -------------------------------------------------------- //
    sections.push(Section {
        id: "corpus",
        title: "Corpus analizado",
        summary: "Qué material sostiene este informe. Cuanto mayor, más fiables son las conclusiones.".to_string(),
        items: vec![
            Item {
                text: format!("{} conversaciones, {} mensajes, {} caracteres leídos.", convs, msgs, chars),
                citation: None,
            },
            Item {
                text: format!(
                    "Mensajes por conversación: {} · longitud media de mensaje: {} caracteres.",
                    profile["stats"]["msgs_per_conversation"].as_f64().unwrap_or(0.0),
                    profile["stats"]["avg_msg_len"].as_f64().unwrap_or(0.0)
                ),
                citation: None,
            },
            Item {
                text: format!(
                    "Modelos más usados: {}.",
                    profile["stats"]["top_models"]
                        .as_array()
                        .map(|m| {
                            m.iter()
                                .map(|x| format!(
                                    "{} ({})",
                                    x["model"].as_str().unwrap_or("?"),
                                    x["count"].as_u64().unwrap_or(0)
                                ))
                                .collect::<Vec<String>>()
                                .join(", ")
                        })
                        .unwrap_or_default()
                ),
                citation: None,
            },
            Item {
                text: "Los temas se extraen de los títulos de conversación y de los mensajes escritos por el usuario; \
                       las respuestas del asistente se ignoran para no inflar el perfil."
                    .to_string(),
                citation: None,
            },
        ],
    });

    // --- 3. Intereses ----------------------------------------------------- //
    let mut items = Vec::new();
    for i in interests.iter().take(12) {
        let topic = i["topic"].as_str().unwrap_or("");
        let ev: Vec<String> = i["evidence"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str()).map(|s| s.to_string()).collect())
            .unwrap_or_default();
        items.push(Item {
            text: format!(
                "\"{}\" — {} menciones ({}% del corpus temático{})",
                topic,
                i["mentions"].as_u64().unwrap_or(0),
                (i["share"].as_f64().unwrap_or(0.0) * 100.0 * 10.0).round() / 10.0,
                if ev.is_empty() {
                    String::new()
                } else {
                    format!("; presente en {}", ev.join(", "))
                }
            ),
            citation: None,
        });
    }
    if !themes.is_empty() {
        for t in themes.iter().take(6) {
            let samples: Vec<String> = t["sample_titles"]
                .as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str()).map(|s| s.to_string()).collect())
                .unwrap_or_default();
            items.push(Item {
                text: format!(
                    "Agrupación temática \"{}\" ({} conversaciones) — ejemplos: {}",
                    t["label"].as_str().unwrap_or(""),
                    t["count"].as_u64().unwrap_or(0),
                    if samples.is_empty() { "—".to_string() } else { samples.join(" / ") }
                ),
                citation: None,
            });
        }
    }
    sections.push(Section {
        id: "intereses",
        title: "Intereses y áreas de trabajo",
        summary: "De qué habla el usuario, con qué insistencia y en qué conversaciones.".to_string(),
        items,
    });

    // --- 4. Fortalezas ---------------------------------------------------- //
    let mut items = Vec::new();
    for s in strengths.iter().take(12) {
        items.push(Item {
            text: format!(
                "{} — «{}»",
                s["evidence"].as_str().unwrap_or(""),
                s["quote"].as_str().unwrap_or("")
            ),
            citation: Some(format!(
                "{}::{} @ {}",
                s["source_service"].as_str().unwrap_or("?"),
                s["conversation_id"].as_str().unwrap_or("?"),
                s["when"].as_str().unwrap_or("")
            )),
        });
    }
    if items.is_empty() {
        items.push(Item {
            text: "No se han encontrado señales explícitas de logro en el texto del usuario.".to_string(),
            citation: None,
        });
    }
    sections.push(Section {
        id: "fortalezas",
        title: "Fortalezas (evidencias del propio usuario)",
        summary: "Logros que el usuario ha contado en primera persona. Son evidencia, no valoración.".to_string(),
        items,
    });

    // --- 5. Dificultades -------------------------------------------------- //
    let mut items = Vec::new();
    for s in difficulties.iter().take(12) {
        items.push(Item {
            text: format!(
                "{} — «{}»",
                s["evidence"].as_str().unwrap_or(""),
                s["quote"].as_str().unwrap_or("")
            ),
            citation: Some(format!(
                "{}::{} @ {}",
                s["source_service"].as_str().unwrap_or("?"),
                s["conversation_id"].as_str().unwrap_or("?"),
                s["when"].as_str().unwrap_or("")
            )),
        });
    }
    if items.is_empty() {
        items.push(Item {
            text: "No se han encontrado señales explícitas de bloqueo en el texto del usuario.".to_string(),
            citation: None,
        });
    }
    sections.push(Section {
        id: "dificultades",
        title: "Dificultades y bloqueos",
        summary: "Dónde se atasca el usuario. Cada punto cita la conversación exacta.".to_string(),
        items,
    });

    // --- 6. Pendientes ---------------------------------------------------- //
    let mut items = Vec::new();
    for p in pending.iter().take(15) {
        items.push(Item {
            text: format!("«{}»", p["text"].as_str().unwrap_or("")),
            citation: Some(format!(
                "{}::{} @ {}",
                p["source_service"].as_str().unwrap_or("?"),
                p["conversation_id"].as_str().unwrap_or("?"),
                p["when"].as_str().unwrap_or("")
            )),
        });
    }
    if items.is_empty() {
        items.push(Item {
            text: "No hay mensajes con intención de pendiente detectada.".to_string(),
            citation: None,
        });
    }
    sections.push(Section {
        id: "pendientes",
        title: "Pendientes y cosas que hay que hacer",
        summary: "Compromisos que el usuario ha expresado en su propio texto: la lista de trabajo real.".to_string(),
        items,
    });

    // --- 7. Patrones de uso ----------------------------------------------- //
    let hours = activity["by_hour"].as_array().cloned().unwrap_or_default();
    let mut busiest: Vec<String> = hours
        .iter()
        .filter(|h| h["count"].as_u64().unwrap_or(0) > 0)
        .map(|h| {
            format!(
                "{}: {} mensajes",
                h["label"].as_str().unwrap_or("?"),
                h["count"].as_u64().unwrap_or(0)
            )
        })
        .collect();
    busiest.sort();
    let weekdays = activity["by_weekday"].as_array().cloned().unwrap_or_default();
    let busiest_day = weekdays
        .iter()
        .max_by_key(|w| w["count"].as_u64().unwrap_or(0))
        .map(|w| {
            format!(
                "{} ({} mensajes)",
                w["label"].as_str().unwrap_or("?"),
                w["count"].as_u64().unwrap_or(0)
            )
        })
        .unwrap_or_else(|| "—".to_string());
    sections.push(Section {
        id: "uso",
        title: "Patrones de uso y ritmo",
        summary: "Cuándo y cómo usa el usuario las herramientas de IA. Es el bloque más relevante \
                  para hablar de hábitos, uso compulsivo y descanso."
            .to_string(),
        items: vec![
            Item {
                text: format!(
                    "Distribución por hora (UTC): {}",
                    if busiest.is_empty() {
                        "sin datos".to_string()
                    } else {
                        busiest.join(", ")
                    }
                ),
                citation: None,
            },
            Item {
                text: format!("Día más activo: {}.", busiest_day),
                citation: None,
            },
            Item {
                text: format!(
                    "Uso nocturno (23:00–05:00): {} mensajes ({}% del total). Días activos: {}.",
                    activity["late_night_messages"].as_u64().unwrap_or(0),
                    (late_night * 100.0 * 10.0).round() / 10.0,
                    activity["active_days"].as_u64().unwrap_or(0)
                ),
                citation: None,
            },
            Item {
                text: format!(
                    "Mensajes por día de actividad: {}.",
                    if activity["active_days"].as_u64().unwrap_or(0) > 0 {
                        round2(msgs as f64 / activity["active_days"].as_u64().unwrap_or(1) as f64)
                    } else {
                        0.0
                    }
                ),
                citation: None,
            },
        ],
    });

    // --- 8. Preguntas abiertas -------------------------------------------- //
    let mut items = Vec::new();
    for q in questions.iter().take(12) {
        items.push(Item {
            text: format!("«{}»", q["text"].as_str().unwrap_or("")),
            citation: Some(format!(
                "{}::{} @ {}",
                q["source_service"].as_str().unwrap_or("?"),
                q["conversation_id"].as_str().unwrap_or("?"),
                q["when"].as_str().unwrap_or("")
            )),
        });
    }
    if items.is_empty() {
        items.push(Item {
            text: "No se han detectado preguntas abiertas en el texto del usuario.".to_string(),
            citation: None,
        });
    }
    sections.push(Section {
        id: "preguntas",
        title: "Preguntas abiertas y estado de incertidumbre",
        summary: "Lo que el usuario pregunta y deja sin respuesta: material habitual de una sesión de coaching.".to_string(),
        items,
    });

    // --- 9. Evolución reciente -------------------------------------------- //
    let emerging = drift["emerging"].as_array().cloned().unwrap_or_default();
    let fading = drift["fading"].as_array().cloned().unwrap_or_default();
    let mut items = Vec::new();
    for e in emerging.iter().take(8) {
        items.push(Item {
            text: format!(
                "Emergente: \"{}\" ({}% → {}%)",
                e["topic"].as_str().unwrap_or(""),
                (e["earlier_share"].as_f64().unwrap_or(0.0) * 100.0 * 10.0).round() / 10.0,
                (e["later_share"].as_f64().unwrap_or(0.0) * 100.0 * 10.0).round() / 10.0
            ),
            citation: None,
        });
    }
    for f in fading.iter().take(8) {
        items.push(Item {
            text: format!(
                "En desvanecimiento: \"{}\" ({}% → {}%)",
                f["topic"].as_str().unwrap_or(""),
                (f["earlier_share"].as_f64().unwrap_or(0.0) * 100.0 * 10.0).round() / 10.0,
                (f["later_share"].as_f64().unwrap_or(0.0) * 100.0 * 10.0).round() / 10.0
            ),
            citation: None,
        });
    }
    if items.is_empty() {
        items.push(Item {
            text: "Sin cambios temáticos significativos entre las dos últimas ventanas.".to_string(),
            citation: None,
        });
    }
    sections.push(Section {
        id: "deriva",
        title: "Evolución reciente (deriva temática)",
        summary: "Comparación entre los últimos 14 días y los 14 anteriores.".to_string(),
        items,
    });

    // --- 10. Recomendaciones ---------------------------------------------- //
    sections.push(Section {
        id: "recomendaciones",
        title: "Recomendaciones prácticas",
        summary: "Acciones concretas que se pueden proponer en sesión.".to_string(),
        items: build_recommendations(profile, drift),
    });

    // --- 11. Metodología --------------------------------------------------- //
    sections.push(Section {
        id: "metodologia",
        title: "Notas metodológicas y límites",
        summary: "Qué puede y qué no puede concluirse de este informe.".to_string(),
        items: vec![
            Item {
                text: "Este informe describe patrones observables en texto escrito por el usuario; \
                       no es un diagnóstico clínico ni una valoración de personalidad."
                    .to_string(),
                citation: None,
            },
            Item {
                text: "Las marcas de tiempo están en UTC y el anclaje es el último mensaje del corpus, \
                       no el reloj del sistema."
                    .to_string(),
                citation: None,
            },
            Item {
                text: "Pendientes, logros y bloqueos se detectan por léxico: es una aproximación con falsos \
                       positivos y falsos negativos que un profesional debe matizar."
                    .to_string(),
                citation: None,
            },
            Item {
                text: "Cada afirmación relevante cita conversación y fecha, de modo que cualquier punto \
                       puede revisarse en el original."
                    .to_string(),
                citation: None,
            },
            Item {
                text: format!(
                    "Cobertura: {} tema(s) dominante(s), {} señal(es) de logro, {} señal(es) de bloqueo, \
                     {} pendiente(s), {} pregunta(s) abierta(s).",
                    interests.len(),
                    strengths.len(),
                    difficulties.len(),
                    pending.len(),
                    questions.len()
                ),
                citation: None,
            },
        ],
    });

    sections
}

/// Genera las recomendaciones prácticas a partir de los datos.
fn build_recommendations(profile: &Value, drift: &Value) -> Vec<Item> {
    let mut out: Vec<Item> = Vec::new();
    let pending = profile["pending"].as_array().cloned().unwrap_or_default();
    let strengths = profile["strengths"].as_array().cloned().unwrap_or_default();
    let difficulties = profile["difficulties"].as_array().cloned().unwrap_or_default();
    let questions = profile["open_questions"].as_array().cloned().unwrap_or_default();
    let recurring = profile["recurring_topics"].as_array().cloned().unwrap_or_default();
    let interests = profile["interests"].as_array().cloned().unwrap_or_default();
    let activity = &profile["activity"];
    let corpus = &profile["corpus"];

    if pending.len() >= 3 {
        let oldest = pending
            .iter()
            .map(|p| p["ts"].as_f64().unwrap_or(0.0))
            .filter(|t| *t > 0.0)
            .fold(f64::MAX, f64::min);
        let oldest_txt = if oldest == f64::MAX {
            String::new()
        } else {
            format!(" El más antiguo es de {} ({})", iso_date(oldest), iso_datetime(oldest))
        };
        out.push(Item {
            text: format!(
                "Hay {} mensajes con intención de pendiente: conviértelos en una lista de tareas con \
                 fecha y responsable.{}",
                pending.len(),
                oldest_txt
            ),
            citation: None,
        });
    }

    if let Some(top) = difficulties.first() {
        let marker = top["markers"]
            .as_array()
            .and_then(|m| m.first())
            .and_then(|m| m.as_str())
            .unwrap_or("bloqueo");
        out.push(Item {
            text: format!(
                "El bloqueo más repetido gira en torno a \"{}\". Suele señalar una frontera técnica \
                 concreta: propón una sesión de trabajo con un objetivo pequeño y medible.",
                marker
            ),
            citation: Some(format!(
                "{}::{} @ {}",
                top["source_service"].as_str().unwrap_or("?"),
                top["conversation_id"].as_str().unwrap_or("?"),
                top["when"].as_str().unwrap_or("")
            )),
        });
    }

    if !strengths.is_empty() {
        let marker = strengths[0]["markers"]
            .as_array()
            .and_then(|m| m.first())
            .and_then(|m| m.as_str())
            .unwrap_or("logro");
        out.push(Item {
            text: format!(
                "Hay {} señales de logro en el historial (la primera: \"{}\"). Son evidencia objetiva de \
                 competencia: úsalas como referencia en los momentos de baja confianza.",
                strengths.len(),
                marker
            ),
            citation: None,
        });
    }

    let late = activity["late_night_share"].as_f64().unwrap_or(0.0);
    if late >= 0.15 {
        out.push(Item {
            text: format!(
                "El {:.1}% de los mensajes cae entre las 23:00 y las 05:00 (UTC). Si el patrón es diario, \
                 es material de primera para hablar de higiene del sueño y de uso compulsivo de la pantalla.",
                late * 100.0
            ),
            citation: None,
        });
    }

    let active_days = activity["active_days"].as_u64().unwrap_or(0);
    let msgs = corpus["messages"].as_u64().unwrap_or(0);
    if active_days > 0 && msgs as f64 / active_days as f64 >= 20.0 {
        out.push(Item {
            text: format!(
                "Volumen alto: {:.0} mensajes por día activo. Un ritmo así necesita un plan explícito \
                 (horario de corte, avisos y alternativa) para que no se vuelva automático.",
                msgs as f64 / active_days as f64
            ),
            citation: None,
        });
    }

    if let Some(i0) = interests.first() {
        out.push(Item {
            text: format!(
                "\"{}\" es el tema con más presencia ({} menciones). Si es algo que quieres sostener, \
                 merece tiempo deliberado; si no lo es, dice en qué se te va la atención mental.",
                i0["topic"].as_str().unwrap_or(""),
                i0["mentions"].as_u64().unwrap_or(0)
            ),
            citation: None,
        });
    }

    if let Some(r0) = recurring.first() {
        out.push(Item {
            text: format!(
                "\"{}\" reaparece en {} conversaciones distintas: interés sostenido, no un pico puntual.",
                r0["term"].as_str().unwrap_or(""),
                r0["count"].as_u64().unwrap_or(0)
            ),
            citation: None,
        });
    }

    if questions.len() >= 5 {
        out.push(Item {
            text: format!(
                "Hay {} preguntas abiertas en el propio historial. Convertirlas en preguntas concretas \
                 (con fecha y respuesta esperada) suele bajar la ansiedad asociada.",
                questions.len()
            ),
            citation: None,
        });
    }

    let emerging = drift["emerging"].as_array().cloned().unwrap_or_default();
    if let Some(e0) = emerging.first() {
        out.push(Item {
            text: format!(
                "En la ventana reciente está ganando peso \"{}\": puede ser un cambio de interés en \
                 curso y merecería una conversación específica.",
                e0["topic"].as_str().unwrap_or("")
            ),
            citation: None,
        });
    }

    let convs = corpus["conversations"].as_u64().unwrap_or(0);
    if convs < 10 {
        out.push(Item {
            text: format!(
                "El corpus es pequeño ({} conversaciones): las conclusiones son preliminares. \
                 Sincronizar con regularidad es lo que da base a un análisis de largo recorrido.",
                convs
            ),
            citation: None,
        });
    }

    out.push(Item {
        text: "Este documento sirve como material de autoobservación y de preparación: úsalo para \
               conversar con un profesional, no como veredicto."
            .to_string(),
        citation: None,
    });
    out
}

/// Renderiza el informe como documento Markdown.
fn render_markdown(profile: &Value, sections: &[Section]) -> String {
    let corpus = &profile["corpus"];
    let mut md = String::new();
    md.push_str("# Informe de perfil personal — MegaDaemon\n\n");
    md.push_str("> Autoobservación a partir del historial de conversaciones del usuario.\n");
    md.push_str("> Herramienta de apoyo para conversar con un profesional: **no es un diagnóstico**.\n\n");
    md.push_str("| Dato | Valor |\n|---|---|\n");
    md.push_str(&format!(
        "| Generado | {} |\n",
        profile["generated_at_iso"].as_str().unwrap_or("")
    ));
    md.push_str(&format!(
        "| Conversaciones | {} |\n",
        corpus["conversations"].as_u64().unwrap_or(0)
    ));
    md.push_str(&format!(
        "| Mensajes | {} |\n",
        corpus["messages"].as_u64().unwrap_or(0)
    ));
    md.push_str(&format!(
        "| Caracteres | {} |\n",
        corpus["chars"].as_u64().unwrap_or(0)
    ));
    md.push_str(&format!(
        "| Servicios | {} |\n",
        corpus["services"]
            .as_array()
            .map(|a| a
                .iter()
                .filter_map(|s| s.as_str())
                .collect::<Vec<&str>>()
                .join(", "))
            .unwrap_or_default()
    ));
    md.push_str(&format!(
        "| Periodo | {} → {} |\n",
        corpus["first_at_iso"].as_str().unwrap_or(""),
        corpus["last_at_iso"].as_str().unwrap_or("")
    ));
    md.push('\n');

    for (i, s) in sections.iter().enumerate() {
        md.push_str(&format!("\n## {}. {}\n\n", i + 1, s.title));
        if !s.summary.is_empty() {
            md.push_str(&s.summary);
            md.push_str("\n\n");
        }
        if !s.items.is_empty() {
            for it in &s.items {
                md.push_str("- ");
                md.push_str(&it.text);
                if let Some(c) = &it.citation {
                    md.push_str(&format!(" `{}`", c));
                }
                md.push('\n');
            }
            md.push('\n');
        }
    }
    md
}

// --------------------------------------------------------------------------- //
// Tests
// --------------------------------------------------------------------------- //

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Construye un almacén sintético (3-4 conversaciones falsas) en un
    /// directorio temporal único.
    fn synthetic_conversations() -> Vec<Value> {
        let base = days_from_civil(2024, 5, 1) * 86_400; // 2024-05-01T00:00:00Z
        let t = |h: i64| (base + h * 3600) as f64;

        vec![
            json!({
                "service": "chatgpt",
                "id": "c1",
                "title": "Desplegar kubernetes en el servidor",
                "updated_at": t(10),
                "model": "gpt-4o",
                "messages": [
                    {"role": "user", "content": "Tengo que arreglar el servidor de kubernetes mañana", "ts": t(10)},
                    {"role": "assistant", "content": "Vale, revisamos la configuración del despliegue.", "ts": t(10)},
                    {"role": "user", "content": "Ya funciona, he conseguido desplegar el cluster", "ts": t(11)}
                ]
            }),
            json!({
                "service": "claude",
                "id": "c2",
                "title": "Configurar el pipeline de CI",
                "updated_at": t(34),
                "model": "claude-3",
                "messages": [
                    {"role": "user", "content": "El pipeline da error, no funciona el paso de build", "ts": t(34)},
                    {"role": "user", "content": "Tengo que revisar los logs del servidor", "ts": t(35)},
                    {"role": "assistant", "content": "Empecemos por los logs del runner.", "ts": t(36)}
                ]
            }),
            json!({
                "service": "deepseek",
                "id": "c3",
                "title": "Kubernetes y despliegue",
                "updated_at": t(58),
                "model": "deepseek-chat",
                "messages": [
                    {"role": "user", "content": "kubernetes es genial, ya funciona el despliegue", "ts": t(58)},
                    {"role": "user", "content": "¿Cómo puedo monitorizar el cluster?", "ts": t(59)}
                ]
            }),
            json!({
                "service": "kimi",
                "id": "c4",
                "title": "Notas del proyecto",
                "updated_at": t(82),
                "model": "kimi",
                "messages": [
                    {"role": "user", "content": "Mañana tengo que escribir la documentación, está pendiente", "ts": t(82)},
                    {"role": "user", "content": "El error persiste, es imposible", "ts": t(83)}
                ]
            }),
        ]
    }

    /// Escribe el almacén sintético en un directorio temporal y lo devuelve.
    fn write_synthetic_store(convs: &[Value]) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "megadaemon-profile-analysis-{}-{}",
            std::process::id(),
            nanos
        ));
        let sync = root.join("sync");
        fs::create_dir_all(&sync).expect("no se pudo crear el directorio de pruebas");
        for c in convs {
            let svc = c["service"].as_str().unwrap_or("unknown");
            let path = sync.join(format!("{}.ndjson", svc));
            let mut buf = String::new();
            buf.push_str(&serde_json::to_string(c).unwrap());
            buf.push('\n');
            let mut f = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .expect("no se pudo abrir el NDJSON de pruebas");
            f.write_all(buf.as_bytes()).expect("no se pudo escribir el NDJSON");
        }
        root
    }

    #[test]
    fn el_cuadro_del_corpus_es_correcto() {
        let convs = synthetic_conversations();
        let p = analyze_conversations(&convs);
        assert_eq!(p["ok"], json!(true));
        let corpus = &p["corpus"];
        assert_eq!(corpus["conversations"], json!(4));
        assert_eq!(corpus["messages"], json!(10));
        assert!(corpus["chars"].as_u64().unwrap_or(0) > 0);
        let services = corpus["services"].as_array().unwrap();
        assert_eq!(services.len(), 4);
        assert!(services.iter().any(|s| s == "chatgpt"));
        assert!(services.iter().any(|s| s == "kimi"));
        assert!(corpus["span_days"].as_f64().unwrap_or(0.0) > 0.0);
        assert!(corpus["first_at"].as_f64().unwrap_or(0.0) > 0.0);
        assert!(corpus["last_at"].as_f64().unwrap_or(0.0) >= corpus["first_at"].as_f64().unwrap_or(0.0));
    }

    #[test]
    fn detecta_los_pendientes_con_su_texto() {
        let convs = synthetic_conversations();
        let p = analyze_conversations(&convs);
        let pending = p["pending"].as_array().unwrap();
        assert!(pending.len() >= 3, "esperaba varios pendientes: {:?}", pending);
        let texts: Vec<String> = pending
            .iter()
            .map(|x| x["text"].as_str().unwrap_or("").to_string())
            .collect();
        assert!(
            texts.iter().any(|t| t.contains("tengo que escribir la documentación")),
            "no se encontró el pendiente esperado: {:?}",
            texts
        );
        assert!(texts.iter().any(|t| t.contains("arreglar el servidor de kubernetes")));
        // Toda evidencia es reproducible: cita conversación y marca de tiempo.
        for p0 in pending {
            assert!(!p0["conversation_id"].as_str().unwrap_or("").is_empty());
            assert!(p0["ts"].as_f64().unwrap_or(0.0) > 0.0);
            assert!(!p0["source_service"].as_str().unwrap_or("").is_empty());
        }
    }

    #[test]
    fn detecta_las_fortalezas_con_la_cita_correcta() {
        let convs = synthetic_conversations();
        let p = analyze_conversations(&convs);
        let strengths = p["strengths"].as_array().unwrap();
        assert!(!strengths.is_empty());
        let hit = strengths
            .iter()
            .find(|s| s["conversation_id"] == "c1")
            .expect("no se detectó el logro de c1");
        assert!(hit["quote"].as_str().unwrap_or("").contains("he conseguido"));
        assert!(hit["evidence"].as_str().unwrap_or("").contains("logro"));
        assert!(hit["ts"].as_f64().unwrap_or(0.0) > 0.0);
    }

    #[test]
    fn detecta_las_dificultades_con_la_cita_correcta() {
        let convs = synthetic_conversations();
        let p = analyze_conversations(&convs);
        let diffs = p["difficulties"].as_array().unwrap();
        assert!(!diffs.is_empty());
        let hit = diffs
            .iter()
            .find(|s| s["conversation_id"] == "c2")
            .expect("no se detectó el bloqueo de c2");
        assert!(hit["quote"].as_str().unwrap_or("").contains("no funciona"));
        assert!(hit["evidence"].as_str().unwrap_or("").contains("bloqueo"));
        assert!(hit["ts"].as_f64().unwrap_or(0.0) > 0.0);
        // El segundo bloqueo (kimi) también debe aparecer.
        assert!(diffs.iter().any(|s| s["conversation_id"] == "c4"));
    }

    #[test]
    fn los_intereses_destacan_el_tema_dominante() {
        let convs = synthetic_conversations();
        let p = analyze_conversations(&convs);
        let interests = p["interests"].as_array().unwrap();
        assert!(!interests.is_empty());
        assert_eq!(interests[0]["topic"], json!("kubernetes"));
        assert!(interests[0]["mentions"].as_u64().unwrap_or(0) >= 4);
        let ev = interests[0]["evidence"].as_array().unwrap();
        assert!(ev.iter().any(|c| c == "c1"));
        assert!(ev.iter().any(|c| c == "c3"));
        // La participación es una fracción coherente.
        let share = interests[0]["share"].as_f64().unwrap_or(0.0);
        assert!(share > 0.0 && share < 1.0);
    }

    #[test]
    fn el_almacen_sintetico_se_lee_desde_disco() {
        let convs = synthetic_conversations();
        let root = write_synthetic_store(&convs);
        let loaded = load_conversations_from(&root.join("sync"));
        assert_eq!(loaded.len(), 4);
        let p = analyze_conversations(&loaded);
        assert_eq!(p["corpus"]["conversations"], json!(4));
        assert_eq!(p["corpus"]["messages"], json!(10));
        assert!(!p["pending"].as_array().unwrap().is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn el_digest_diario_agrupa_por_dia() {
        let convs = synthetic_conversations();
        let d = daily_digest_from(&convs, 7);
        assert_eq!(d["ok"], json!(true));
        let days = d["by_day"].as_array().unwrap();
        assert!(!days.is_empty());
        // Cada día con actividad tiene fecha, mensajes y temas.
        for day in days {
            assert!(!day["date"].as_str().unwrap_or("").is_empty());
            assert!(day["messages"].as_u64().unwrap_or(0) > 0);
        }
        // Los pendientes del día 1 deben aparecer en su día.
        let first = &days[0];
        assert!(!first["pending"].as_array().unwrap().is_empty());
    }

    #[test]
    fn la_deriva_de_temas_compara_las_dos_ventanas() {
        let convs = synthetic_conversations();
        let d = topic_drift_from(&convs, 2);
        assert_eq!(d["ok"], json!(true));
        assert_eq!(d["window_days"], json!(2));
        assert!(d["earlier"].is_array());
        assert!(d["later"].is_array());
        assert!(d["emerging"].is_array());
        assert!(d["fading"].is_array());
    }

    #[test]
    fn el_informe_markdown_tiene_estructura_completa() {
        let convs = synthetic_conversations();
        let b = briefing_from(&convs, true);
        assert_eq!(b["ok"], json!(true));
        let md = b["markdown"].as_str().unwrap_or("");
        assert!(md.starts_with("# Informe de perfil personal"));
        assert!(md.contains("## 1. Resumen ejecutivo"));
        assert!(md.contains("## 3. Intereses y áreas de trabajo"));
        assert!(md.contains("## 10. Recomendaciones prácticas"));
        assert!(md.contains("kubernetes"));
    }

    #[test]
    fn el_informe_json_devuelve_las_mismas_secciones() {
        let convs = synthetic_conversations();
        let b = briefing_from(&convs, false);
        let sections = b["sections"].as_array().unwrap();
        assert!(sections.len() >= 10);
        assert_eq!(sections[0]["id"], json!("resumen"));
        assert_eq!(sections[0]["index"], json!(1));
        for s in sections {
            assert!(!s["title"].as_str().unwrap_or("").is_empty());
            assert!(s["items"].is_array());
        }
    }

    #[test]
    fn la_actividad_por_hora_cubre_las_24_franjas() {
        let convs = synthetic_conversations();
        let p = analyze_conversations(&convs);
        let hours = p["activity"]["by_hour"].as_array().unwrap();
        assert_eq!(hours.len(), 24);
        assert_eq!(p["activity"]["by_weekday"].as_array().unwrap().len(), 7);
        assert!(p["activity"]["by_month"].as_array().unwrap().len() >= 1);
        // Las 10:00 UTC deben ser la hora punta del corpus sintético.
        let peak = p["activity"]["peak_hour"]["hour"].as_u64().unwrap_or(99);
        assert_eq!(peak, 10);
    }

    #[test]
    fn las_marcas_de_tiempo_en_milisegundos_se_normalizan() {
        let convs = synthetic_conversations();
        let mut c = convs[0].clone();
        c["messages"][0]["ts"] = json!(1714521600000i64); // milisegundos
        let p = analyze_conversations(&[c]);
        let ts = p["pending"][0]["ts"].as_f64().unwrap_or(0.0);
        assert!(
            (ts - 1714521600.0).abs() < 2.0,
            "la marca de tiempo no se normalizó: {}",
            ts
        );
    }

    #[test]
    fn un_corpus_vacío_no_rompe_nada() {
        let p = analyze_conversations(&[]);
        assert_eq!(p["ok"], json!(true));
        assert_eq!(p["corpus"]["conversations"], json!(0));
        assert_eq!(p["interests"].as_array().unwrap().len(), 0);
        let b = briefing_from(&[], true);
        assert!(b["markdown"].as_str().unwrap_or("").contains("Informe de perfil"));
        let d = daily_digest_from(&[], 7);
        assert_eq!(d["ok"], json!(true));
        let t = topic_drift_from(&[], 7);
        assert_eq!(t["ok"], json!(true));
    }

    #[test]
    fn el_contrato_json_de_analyze_se_cumple() {
        let convs = synthetic_conversations();
        let p = analyze_conversations(&convs);
        for key in [
            "ok",
            "generated_at",
            "corpus",
            "interests",
            "activity",
            "themes",
            "pending",
            "strengths",
            "difficulties",
            "recurring_topics",
            "open_questions",
            "stats",
        ] {
            assert!(p.get(key).is_some(), "falta la clave raíz '{}'", key);
        }
        for key in [
            "conversations",
            "messages",
            "chars",
            "services",
            "first_at",
            "last_at",
            "span_days",
        ] {
            assert!(
                p["corpus"].get(key).is_some(),
                "falta corpus.{}",
                key
            );
        }
        for key in ["avg_msg_len", "longest_msg", "msgs_per_conversation", "top_models"] {
            assert!(p["stats"].get(key).is_some(), "falta stats.{}", key);
        }
        for key in ["by_hour", "by_weekday", "by_month"] {
            assert!(p["activity"].get(key).is_some(), "falta activity.{}", key);
        }
        // Cada bloque con items debe tener sus campos.
        for i in p["interests"].as_array().unwrap() {
            for key in ["topic", "mentions", "share", "evidence"] {
                assert!(i.get(key).is_some(), "falta interests.{}", key);
            }
        }
        for t in p["themes"].as_array().unwrap() {
            for key in ["label", "count", "sample_titles"] {
                assert!(t.get(key).is_some(), "falta themes.{}", key);
            }
        }
        for x in p["pending"].as_array().unwrap() {
            for key in ["text", "source_service", "conversation_id", "ts"] {
                assert!(x.get(key).is_some(), "falta pending.{}", key);
            }
        }
        for x in p["strengths"].as_array().unwrap() {
            for key in ["evidence", "conversation_id", "ts", "quote"] {
                assert!(x.get(key).is_some(), "falta strengths.{}", key);
            }
        }
        for x in p["difficulties"].as_array().unwrap() {
            for key in ["evidence", "conversation_id", "ts", "quote"] {
                assert!(x.get(key).is_some(), "falta difficulties.{}", key);
            }
        }
        for x in p["recurring_topics"].as_array().unwrap() {
            for key in ["term", "count"] {
                assert!(x.get(key).is_some(), "falta recurring_topics.{}", key);
            }
        }
        assert!(!p["open_questions"].as_array().unwrap().is_empty());
    }

    #[test]
    fn la_negacion_no_convierte_un_bloqueo_en_logro() {
        // "no funciona" es un bloqueo, nunca un logro.
        let texto = "El build no funciona y no está solucionado";
        assert!(strength_markers(texto).is_empty(), "falso positivo de logro");
        assert!(!difficulty_markers(texto).is_empty());
        // Esa frase tampoco expresa ninguna tarea pendiente.
        assert!(pending_markers(texto).is_empty());
        // Pero aquí sí hay intención de tarea, y hay bloqueo.
        let tarea = "no he podido arreglarlo, tengo que revisarlo";
        assert!(pending_markers(tarea).iter().any(|m| m == "tengo que"));
        assert!(!difficulty_markers(tarea).is_empty());
    }
}
