//! MegaDaemon v13.4 — Extracción de códigos de un solo uso (OTP) del buzón propio.
//!
//! Lee el **buzón del propio usuario** (vía `telecom::ContentTelecom`) y saca
//! los códigos de un solo uso de sus propias cuentas (2FA, verificación por
//! SMS, códigos de pedido). Nada de esto toca cuentas ajenas: es lectura del
//! buzón propio.
//!
//! ## Lo que se busca
//!
//! * Números de **4 a 8 dígitos**, con o sin separador (`123456`, `123 456`,
//!   `483-921`).
//! * Con **palabra clave** cerca: "código", "code", "verification", "OTP",
//!   "2FA"… (la lista completa está en [`KEYWORDS`]).
//!
//! ## Lo que NO se extrae (falsos positivos)
//!
//! * Un **teléfono** que aparece en el texto (9-15 dígitos, o precedido de
//!   `+`, o con "llama/tel" delante).
//! * Un **número de pedido** largo (12+ dígitos, o con "pedido/factura" delante).
//! * Un número suelto **sin palabra clave**: no se descarta (a veces el
//!   mensaje es sólo el código), pero su **confianza se queda baja** y se
//!   marca como `dubious`.
//!
//! ## Confianza explicable
//!
//! Cada candidato lleva **por qué** se ha extraído (`reason`): qué palabra
//! clave se ha encontrado, a cuántos caracteres y de qué lado. La confianza es
//! un número en `0..=1`, pero **el motivo es la parte importante**: no es una
//! caja negra.
//!
//! ## Normalización sin dependencias
//!
//! `unicode-normalization` **no está** en `Cargo.toml` (verificado) y no se
//! añade: la normalización es manual y sólo a ASCII (dígitos de ancho
//! completo, dígitos de otros alfabetos, espacios raros, acentos, ancho
//! cero). Cada carácter se mapea a **un** carácter, así que las posiciones del
//! texto original se conservan y las distancias siguen siendo exactas.

use serde::{Deserialize, Serialize};

/// Longitud mínima (dígitos) de un código aceptado.
pub const MIN_OTP_DIGITS: usize = 4;
/// Longitud máxima (dígitos) de un código aceptado.
pub const MAX_OTP_DIGITS: usize = 8;

/// Palabras clave que respaldan un código.
///
/// En minúsculas y **sin acentos** (el texto se normaliza antes de buscar).
pub const KEYWORDS: &[&str] = &[
    "codigo",
    "codigo de verificacion",
    "codigo de seguridad",
    "code",
    "verification code",
    "verification",
    "verificacion",
    "verify",
    "otp",
    "2fa",
    "two-factor",
    "two factor",
    "doble factor",
    "one-time password",
    "onetime password",
    "token",
    "clave",
    "pin",
    "contrasena",
    "password",
];

/// Palabras que delatan un **teléfono** delante del número.
pub const PHONE_CONTEXT: &[&str] = &[
    "tel", "telefono", "llama", "llamar", "llamalo", "llamala", "movil", "celular", "whatsapp",
    "fono",
];

/// Palabras que delatan un **número de pedido/factura** delante del número.
pub const ORDER_CONTEXT: &[&str] = &[
    "pedido",
    "orden",
    "factura",
    "referencia",
    "tracking",
    "envio",
    "invoice",
    "order",
];

/// Distancia máxima (caracteres) para que una palabra clave se considere
/// "cercana" al número.
pub const KEYWORD_MAX_DISTANCE: usize = 80;

/// Un mensaje del buzón (mínimo para poder extraer un OTP).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtpMessage {
    /// Identificador del mensaje (`_id` del proveedor de contenido).
    pub id: String,
    /// Remitente.
    pub sender: String,
    /// Cuerpo del mensaje.
    pub body: String,
    /// Marca de tiempo (ms epoch).
    pub timestamp: i64,
}

impl OtpMessage {
    /// Constructor con los campos esenciales.
    pub fn new(id: &str, sender: &str, body: &str, timestamp: i64) -> Self {
        OtpMessage {
            id: id.to_string(),
            sender: sender.to_string(),
            body: body.to_string(),
            timestamp,
        }
    }
}

/// Un código candidato, con su confianza y **su motivo**.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtpCandidate {
    /// El código, sólo dígitos (normalizado).
    pub value: String,
    /// Confianza en `0..=1`.
    pub confidence: f32,
    /// Mensaje del que salió.
    pub message_id: String,
    /// Remitente del mensaje.
    pub sender: String,
    /// Marca de tiempo del mensaje (ms epoch).
    pub timestamp: i64,
    /// **Por qué** se ha extraído (palabra clave, distancia, nº de dígitos).
    pub reason: String,
    /// Sin palabra clave: puede ser un número suelto que no sea un código.
    pub dubious: bool,
}

impl OtpCandidate {
    /// Etiqueta de la confianza ("alta", "media", "baja").
    pub fn confidence_label(&self) -> &'static str {
        if self.confidence >= 0.8 {
            "alta"
        } else if self.confidence >= 0.5 {
            "media"
        } else {
            "baja"
        }
    }
}

/// Un número que **no** se ha extraído, con el motivo del descarte.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RejectedToken {
    /// Texto tal cual se vio (con separadores).
    pub text: String,
    /// Cuántos dígitos tenía.
    pub digits: usize,
    /// Por qué se descartó.
    pub reason: String,
}

/// Resultado completo de una extracción: lo que sale y lo que se descarta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtpExtraction {
    /// Candidatos, ordenados por confianza (de mayor a menor).
    pub candidates: Vec<OtpCandidate>,
    /// Números descartados, con su motivo.
    pub rejected: Vec<RejectedToken>,
}

// ---------------------------------------------------------------------------
// Normalización (manual, sin dependencias)
// ---------------------------------------------------------------------------

/// Normaliza un texto a ASCII "plano" para poder buscar dígitos y palabras.
///
/// * dígitos de ancho completo (`１２３`) y de otros alfabetos → ASCII;
/// * espacios raros (NBSP, espacio fino) → espacio normal;
/// * acentos del latín → letra sin acento (`código` → `codigo`);
/// * caracteres de ancho cero → eliminados;
/// * mayúsculas ASCII → minúsculas.
///
/// Cada carácter se mapea a **exactamente un** carácter (o se elimina), de
/// modo que las posiciones del texto original se conservan.
pub fn normalize_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            // Dígitos de ancho completo.
            '\u{ff10}'..='\u{ff19}' => out.push((b'0' + (c as u32 - 0xff10) as u8) as char),
            // Letras de ancho completo (ｍａｉｌ) → ASCII.
            '\u{ff21}'..='\u{ff3a}' => out.push((b'A' + (c as u32 - 0xff21) as u8) as char),
            '\u{ff41}'..='\u{ff5a}' => out.push((b'a' + (c as u32 - 0xff41) as u8) as char),
            // Dígitos arábigos, persas, devanagari, bengalí y tailandés.
            '\u{0660}'..='\u{0669}' => out.push((b'0' + (c as u32 - 0x0660) as u8) as char),
            '\u{06f0}'..='\u{06f9}' => out.push((b'0' + (c as u32 - 0x06f0) as u8) as char),
            '\u{0966}'..='\u{096f}' => out.push((b'0' + (c as u32 - 0x0966) as u8) as char),
            '\u{09e6}'..='\u{09ef}' => out.push((b'0' + (c as u32 - 0x09e6) as u8) as char),
            '\u{0e50}'..='\u{0e59}' => out.push((b'0' + (c as u32 - 0x0e50) as u8) as char),
            // Espacios raros → espacio normal.
            '\u{00a0}' | '\u{2000}'..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}' => {
                out.push(' ')
            }
            // Ancho cero / BOM → nada.
            '\u{200b}'..='\u{200d}' | '\u{feff}' | '\u{200e}' | '\u{200f}' => {}
            // Acentos latinos.
            'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' | 'Á' | 'À' | 'Ä' | 'Â' | 'Ã' | 'Å' => {
                out.push('a')
            }
            'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => out.push('e'),
            'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => out.push('i'),
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' | 'Ó' | 'Ò' | 'Ö' | 'Ô' | 'Õ' => out.push('o'),
            'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => out.push('u'),
            'ñ' | 'Ñ' => out.push('n'),
            'ç' | 'Ç' => out.push('c'),
            'ý' | 'ÿ' | 'Ý' | 'Ÿ' => out.push('y'),
            // Mayúsculas ASCII → minúsculas.
            'A'..='Z' => out.push(c.to_ascii_lowercase()),
            _ => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Búsqueda de palabras clave
// ---------------------------------------------------------------------------

/// Una palabra clave encontrada en el texto normalizado.
#[derive(Debug, Clone, PartialEq)]
pub struct KeywordHit {
    /// Posición inicial (en caracteres del texto normalizado).
    pub start: usize,
    /// Posición final (exclusiva).
    pub end: usize,
    /// La palabra clave encontrada.
    pub keyword: String,
}

/// Busca las palabras clave en un texto **ya normalizado**.
///
/// Reglas de frontera (para no engancharse dentro de otra palabra):
/// * el carácter anterior no puede ser alfanumérico;
/// * el carácter posterior no puede ser un dígito;
/// * en palabras cortas (`otp`, `2fa`, `pin`, `code`) el carácter posterior
///   tampoco puede ser una letra (así "barcode" no cuenta como "code").
pub fn find_keywords(text: &str) -> Vec<KeywordHit> {
    let chars: Vec<char> = text.chars().collect();
    let mut hits = Vec::new();
    for kw in KEYWORDS {
        let kchars: Vec<char> = kw.chars().collect();
        if kchars.is_empty() {
            continue;
        }
        let mut from = 0usize;
        while from + kchars.len() <= chars.len() {
            if chars[from..from + kchars.len()] == kchars[..] {
                let before_ok = from == 0 || !is_word_char(chars[from - 1]);
                let after = chars.get(from + kchars.len()).copied();
                let after_ok = match after {
                    None => true,
                    Some(c) => {
                        if c.is_ascii_digit() {
                            false
                        } else if kchars.len() <= 4 && c.is_alphabetic() {
                            false
                        } else {
                            true
                        }
                    }
                };
                if before_ok && after_ok {
                    hits.push(KeywordHit {
                        start: from,
                        end: from + kchars.len(),
                        keyword: (*kw).to_string(),
                    });
                }
            }
            from += 1;
        }
    }
    hits.sort_by_key(|h| h.start);
    hits
}

/// ¿Es un carácter de "palabra" (letra o dígito)?
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric()
}

// ---------------------------------------------------------------------------
// Tokenización de números
// ---------------------------------------------------------------------------

/// Un número encontrado en el texto (con sus separadores).
#[derive(Debug, Clone, PartialEq)]
struct Token {
    /// Posición inicial.
    start: usize,
    /// Posición final (exclusiva).
    end: usize,
    /// Dígitos sin separadores.
    digits: String,
}

/// ¿Es un separador válido dentro de un número?
fn is_separator(c: char) -> bool {
    matches!(
        c,
        ' ' | '-' | '_' | '.' | '\u{2010}' | '\u{2011}' | '\u{2013}'
    )
}

/// Trocea el texto normalizado en números (dígitos con separadores).
fn scan_tokens(text: &str) -> Vec<Token> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        let mut digits = String::new();
        let mut last_digit_end = i;
        let mut j = i;
        while j < chars.len() {
            let c = chars[j];
            if c.is_ascii_digit() {
                digits.push(c);
                j += 1;
                last_digit_end = j;
            } else if is_separator(c) {
                // Un separador sólo cuenta si le sigue un dígito.
                if chars
                    .get(j + 1)
                    .map(|n| n.is_ascii_digit())
                    .unwrap_or(false)
                {
                    j += 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        if !digits.is_empty() {
            tokens.push(Token {
                start,
                end: last_digit_end,
                digits,
            });
        }
        i = last_digit_end.max(i + 1);
    }
    tokens
}

// ---------------------------------------------------------------------------
// Extracción
// ---------------------------------------------------------------------------

/// Extrae los códigos de un mensaje, con motivo y confianza.
pub fn extract_otp(message: &OtpMessage) -> Vec<OtpCandidate> {
    extract_otp_detailed(message).candidates
}

/// Extrae los códigos de un texto suelto (sin metadatos del mensaje).
pub fn extract_otp_text(body: &str) -> Vec<OtpCandidate> {
    extract_otp(&OtpMessage::new("", "", body, 0))
}

/// Extrae los códigos de **todos** los mensajes del buzón.
///
/// Devuelve la lista aplanada y ordenada por confianza (después por fecha,
/// del más reciente al más antiguo).
pub fn extract_otp_in_inbox(messages: &[OtpMessage]) -> Vec<OtpCandidate> {
    let mut out: Vec<OtpCandidate> = Vec::new();
    for m in messages {
        for c in extract_otp(m) {
            // Un mismo valor puede aparecer en varios mensajes: se queda el
            // de mayor confianza (y, a igualdad, el más reciente).
            match out.iter_mut().find(|x| x.value == c.value) {
                Some(existente) => {
                    if c.confidence > existente.confidence {
                        *existente = c;
                    }
                }
                None => out.push(c),
            }
        }
    }
    out.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.timestamp.cmp(&a.timestamp))
    });
    out
}

/// Extracción completa: candidatos **y** descartes con su motivo.
pub fn extract_otp_detailed(message: &OtpMessage) -> OtpExtraction {
    let norm = normalize_text(&message.body);
    let keywords = find_keywords(&norm);
    let tokens = scan_tokens(&norm);
    let mut candidates: Vec<OtpCandidate> = Vec::new();
    let mut rejected: Vec<RejectedToken> = Vec::new();

    for token in &tokens {
        let n = token.digits.len();
        let text = norm
            .chars()
            .skip(token.start)
            .take(token.end - token.start)
            .collect::<String>();

        if n < MIN_OTP_DIGITS {
            rejected.push(RejectedToken {
                text,
                digits: n,
                reason: format!("menos de {MIN_OTP_DIGITS} dígitos: no puede ser un código"),
            });
            continue;
        }
        if n > MAX_OTP_DIGITS {
            let reason = if (9..=11).contains(&n) {
                format!("{n} dígitos: parece un teléfono, no un código")
            } else if (12..=15).contains(&n) {
                format!("{n} dígitos: parece un número de pedido o factura")
            } else {
                format!("{n} dígitos: demasiado largo para un código")
            };
            rejected.push(RejectedToken {
                text,
                digits: n,
                reason,
            });
            continue;
        }
        if let Some(reason) = phone_reason(&norm, token) {
            rejected.push(RejectedToken {
                text,
                digits: n,
                reason,
            });
            continue;
        }
        if let Some(reason) = order_reason(&norm, token) {
            rejected.push(RejectedToken {
                text,
                digits: n,
                reason,
            });
            continue;
        }

        // Confianza según la palabra clave más cercana.
        let (confidence, reason, dubious) = score(&keywords, token);
        candidates.push(OtpCandidate {
            value: token.digits.clone(),
            confidence,
            message_id: message.id.clone(),
            sender: message.sender.clone(),
            timestamp: message.timestamp,
            reason,
            dubious,
        });
    }

    // Un mismo valor repetido: se queda el de mayor confianza.
    candidates.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.value.cmp(&b.value))
    });
    let mut unicos: Vec<OtpCandidate> = Vec::new();
    for c in candidates {
        if !unicos.iter().any(|x| x.value == c.value) {
            unicos.push(c);
        }
    }

    OtpExtraction {
        candidates: unicos,
        rejected,
    }
}

/// Motivo de descarte por contexto de teléfono (`+`, "llama", "tel"...).
fn phone_reason(norm: &str, token: &Token) -> Option<String> {
    let antes = context_before(norm, token, 20);
    if antes.trim_end().ends_with('+') {
        return Some("precedido de '+': parece un teléfono".to_string());
    }
    let context = antes.to_lowercase();
    for w in PHONE_CONTEXT {
        if let Some(pos) = context.rfind(w) {
            // La palabra tiene que estar pegada (o casi) al número.
            let gap = context.len() - (pos + w.len());
            if gap <= 2 {
                return Some(format!("precedido de '{w}': parece un teléfono"));
            }
        }
    }
    None
}

/// Motivo de descarte por contexto de pedido/factura.
fn order_reason(norm: &str, token: &Token) -> Option<String> {
    let antes = context_before(norm, token, 20);
    let context = antes.to_lowercase();
    for w in ORDER_CONTEXT {
        if let Some(pos) = context.rfind(w) {
            let gap = context.len() - (pos + w.len());
            if gap <= 4 {
                return Some(format!("precedido de '{w}': parece un número de pedido"));
            }
        }
    }
    None
}

/// Los últimos `n` caracteres que hay **antes** del token (en el texto
/// normalizado). `token.start` es un índice de carácter, no de byte.
fn context_before(norm: &str, token: &Token, n: usize) -> String {
    let chars: Vec<char> = norm.chars().collect();
    let end = token.start.min(chars.len());
    let start = end.saturating_sub(n);
    chars[start..end].iter().collect()
}

/// Calcula la confianza y el motivo de un token aceptado.
fn score(keywords: &[KeywordHit], token: &Token) -> (f32, String, bool) {
    let n = token.digits.len();
    // Palabra clave más cercana (por distancia en caracteres).
    let mut best: Option<(usize, &KeywordHit, bool)> = None; // (distancia, hit, está antes)
    for hit in keywords {
        let (dist, before) = if hit.end <= token.start {
            (token.start - hit.end, true)
        } else if token.end <= hit.start {
            (hit.start - token.end, false)
        } else {
            continue; // la palabra está dentro del número: no cuenta
        };
        if dist > KEYWORD_MAX_DISTANCE {
            continue;
        }
        if best.map(|(d, _, _)| dist < d).unwrap_or(true) {
            best = Some((dist, hit, before));
        }
    }

    let Some((dist, hit, before)) = best else {
        let reason = format!(
            "sin palabra clave: {n} dígitos sueltos; puede ser cualquier número \
             (confianza baja, marcado como dudoso)"
        );
        return (0.20, reason, true);
    };

    let (base, nivel) = match dist {
        0..=12 => (0.95f32, "muy cerca"),
        13..=40 => (0.85, "cerca"),
        41..=80 => (0.65, "lejos"),
        _ => (0.45, "muy lejos"),
    };
    let lado = if before { "antes" } else { "después" };
    let mut confidence = base;
    let mut extra = String::new();
    if n == 4 {
        // Un código de 4 dígitos es más ambiguo (puede ser una extensión).
        confidence -= 0.05;
        extra = "; 4 dígitos: más ambiguo".to_string();
    }
    let reason = format!(
        "palabra clave '{}' {dist} caracteres {lado} del código ({nivel}); {n} dígitos{extra}",
        hit.keyword
    );
    (confidence, reason, false)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(body: &str) -> OtpMessage {
        OtpMessage::new("42", "+34600111222", body, 1_712_345_678_000)
    }

    fn first(body: &str) -> OtpCandidate {
        let cands = extract_otp(&msg(body));
        assert!(
            !cands.is_empty(),
            "esperaba al menos un candidato en: {body}"
        );
        cands[0].clone()
    }

    #[test]
    fn tu_codigo_es_123456() {
        let c = first("Tu código es 123456");
        assert_eq!(c.value, "123456");
        assert!(c.confidence >= 0.9, "confianza: {}", c.confidence);
        assert!(!c.dubious);
        assert_eq!(c.message_id, "42");
        assert_eq!(c.sender, "+34600111222");
        assert_eq!(c.timestamp, 1_712_345_678_000);
        assert!(c.reason.contains("codigo"), "motivo: {}", c.reason);
    }

    #[test]
    fn codigo_con_guion_483_921() {
        let c = first("Código: 483-921");
        assert_eq!(c.value, "483921", "los separadores se quitan");
        assert!(c.confidence >= 0.9);
        assert!(!c.dubious);
    }

    #[test]
    fn codigo_con_separador_123_456() {
        let c = first("Tu codigo de verificacion es 123 456");
        assert_eq!(c.value, "123456");
        assert!(c.confidence >= 0.8);
    }

    #[test]
    fn codigo_en_ingles_verification_code() {
        let c = first("Your verification code is 7788. Do not share it.");
        assert_eq!(c.value, "7788");
        assert!(c.confidence >= 0.8);
        assert!(!c.dubious);
    }

    #[test]
    fn otp_y_2fa_en_ingles() {
        let c = first("Your OTP is 654321");
        assert_eq!(c.value, "654321");
        assert!(c.confidence >= 0.9);
        let c2 = first("2FA code: 4321");
        assert_eq!(c2.value, "4321");
        assert!(c2.confidence >= 0.8);
    }

    #[test]
    fn el_codigo_va_despues_del_numero() {
        let c = first("123456 es tu código de verificación");
        assert_eq!(c.value, "123456");
        assert!(c.confidence >= 0.8, "confianza: {}", c.confidence);
        assert!(c.reason.contains("después") || c.reason.contains("despues"));
    }

    #[test]
    fn un_telefono_en_el_texto_no_se_extrae() {
        let cands = extract_otp(&msg("Llámanos al 900 123 456 para más información"));
        assert!(cands.is_empty(), "no debe extraerse el teléfono: {cands:?}");
        let ext = extract_otp_detailed(&msg("Llámanos al 900 123 456"));
        assert!(!ext.rejected.is_empty());
        assert!(ext.rejected[0].reason.contains("teléfono"));
    }

    #[test]
    fn un_telefono_con_signo_mas_no_se_extrae() {
        let cands = extract_otp(&msg("Escríbenos al +34 600112233"));
        assert!(cands.is_empty(), "no debe extraerse: {cands:?}");
    }

    #[test]
    fn un_numero_de_pedido_largo_no_se_extrae() {
        let cands = extract_otp(&msg("Tu pedido 1234567890123 ha salido del almacén"));
        assert!(cands.is_empty(), "no debe extraerse: {cands:?}");
        let ext = extract_otp_detailed(&msg("Tu pedido 1234567890123 ha salido"));
        assert!(ext.rejected[0].reason.contains("pedido"));
    }

    #[test]
    fn sin_palabra_clave_la_confianza_es_baja_y_marca_dudoso() {
        let c = first("El total es 4321");
        assert_eq!(c.value, "4321");
        assert!(c.confidence < 0.5, "confianza: {}", c.confidence);
        assert!(c.dubious);
        assert_eq!(c.confidence_label(), "baja");
        assert!(c.reason.contains("sin palabra clave"));
    }

    #[test]
    fn dos_codigos_en_el_mismo_mensaje() {
        let cands = extract_otp(&msg(
            "Tu código es 123456 y tu código de recuperación es 654321",
        ));
        assert_eq!(cands.len(), 2, "candidatos: {cands:?}");
        let valores: Vec<&str> = cands.iter().map(|c| c.value.as_str()).collect();
        assert!(valores.contains(&"123456"));
        assert!(valores.contains(&"654321"));
        for c in &cands {
            assert!(!c.dubious);
            assert!(c.confidence >= 0.8);
        }
    }

    #[test]
    fn los_numeros_pequenos_no_son_codigos() {
        let cands = extract_otp(&msg("Tienes 3 avisos nuevos"));
        assert!(cands.is_empty(), "3 dígitos no son un código: {cands:?}");
    }

    #[test]
    fn normaliza_digitos_de_ancho_completo_y_espacios() {
        let c = first("Tu ｃóｄigo es １２３ ４５６");
        assert_eq!(c.value, "123456");
        assert!(c.confidence >= 0.8, "confianza: {}", c.confidence);
    }

    #[test]
    fn normaliza_acentos_y_digitos_extranos() {
        assert_eq!(normalize_text("código"), "codigo");
        assert_eq!(normalize_text("VERIFICACIÓN"), "verificacion");
        assert_eq!(normalize_text("٤٥٦"), "456");
        assert_eq!(normalize_text("123\u{200b}456"), "123456");
        assert_eq!(normalize_text("a\u{00a0}b"), "a b");
    }

    #[test]
    fn la_normalizacion_conserva_la_longitud_de_los_caracteres() {
        // Cada carácter se mapea a uno (o se elimina): las posiciones siguen
        // valiendo para medir distancias.
        assert_eq!(normalize_text("código").chars().count(), 6);
        assert_eq!(normalize_text("CÓDIGO").chars().count(), 6);
    }

    #[test]
    fn la_palabra_clave_no_se_engancha_dentro_de_otra_palabra() {
        // "barcode" no es la palabra clave "code".
        let cands = extract_otp(&msg("Su barcode es 998877"));
        assert!(
            cands.iter().all(|c| c.confidence < 0.5),
            "no debe recognise 'code' dentro de 'barcode': {cands:?}"
        );
    }

    #[test]
    fn un_codigo_repetido_solo_aparece_una_vez() {
        let cands = extract_otp(&msg("Tu código es 123456. Repite 123456, por favor."));
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].value, "123456");
    }

    #[test]
    fn el_buzon_completo_aplana_y_ordena_por_confianza() {
        let msgs = vec![
            OtpMessage::new("1", "A", "El total es 4321", 1_000),
            OtpMessage::new("2", "B", "Tu código es 123456", 2_000),
            OtpMessage::new("3", "C", "verification code 7777", 3_000),
        ];
        let cands = extract_otp_in_inbox(&msgs);
        assert_eq!(cands.len(), 3);
        assert!(cands[0].confidence >= cands[1].confidence);
        assert!(cands[1].confidence >= cands[2].confidence);
        // El más reciente va después en igualdad de confianza.
        assert_eq!(cands[2].message_id, "1");
    }
}
