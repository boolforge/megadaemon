//! Utilidades de texto seguras (v13.2 — hallazgo S13).
//!
//! `&str[..n]` corta por BYTES: si el corte cae dentro de un carácter
//! multibyte (p. ej. 'é' = 2 bytes) Rust entra en panic
//! ("byte index is not a char boundary"). Con `panic = "abort"` en release
//! eso tumba el proceso entero: un texto UTF-8 truncado a 65536 bytes
//! provoca un DoS local trivial (y remoto si el texto viene de una
//! respuesta de LLM o de un fichero subido).
//!
//! `truncate_chars` corta por PUNTOS DE CARACTER y nunca entra en panic.

/// Trunca `s` a un máximo de `max_chars` CARACTERES (no bytes).
/// Devuelve un `String` (copia). Si el corte cae dentro de un carácter
/// multibyte, retrocede al límite de carácter anterior.
pub fn truncate_chars(s: &str, max_chars: usize) -> String {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => s[..idx].to_string(),
        None => s.to_string(),
    }
}

/// Trunca `s` a un máximo de `max_bytes` BYTES sin partir caracteres
/// (deja el último carácter multibyte entero fuera si no cabe).
pub fn truncate_bytes(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_panic_con_corte_dentro_de_multibyte() {
        // Patrón exacto que entraba en panic antes: 65535 'A' + 'é' (2 bytes).
        let mut s = String::with_capacity(70000);
        for _ in 0..65535 {
            s.push('A');
        }
        s.push('\u{00E9}');
        s.push('B');
        let t = truncate_chars(&s, 65536);
        assert!(t.chars().count() <= 65536);
        let b = truncate_bytes(&s, 65536);
        assert!(b.len() <= 65536);
        assert!(s.is_char_boundary(b.len()));
    }
}

/// Bytes a hexadecimal en minúsculas.
pub fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// Días desde el epoch (1970-01-01) → (año, mes, día) en UTC.
/// Algoritmo de Hinnant: aritmética entera pura, sin dependencias ni `chrono`.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Marca de tiempo ISO-8601 en UTC (`2026-11-19T19:30:00Z`).
pub fn format_iso8601(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Hora actual en ISO-8601 UTC (sin dependencias: días → fecha civil).
pub fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests_iso {
    use super::*;

    #[test]
    fn hex_es_minusculas_y_de_longitud_doble() {
        assert_eq!(hex(&[0x00, 0x0f, 0xff, 0xa5]), "000fffa5");
        assert_eq!(hex(b""), "");
        assert_eq!(hex(&[0xde, 0xad, 0xbe, 0xef]), "deadbeef");
    }

    #[test]
    fn iso8601_de_una_epoch_conocida() {
        // 2026-11-19T19:30:00Z = 1795138200
        assert_eq!(format_iso8601(1_795_138_200), "2026-11-19T19:30:00Z");
        assert_eq!(format_iso8601(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn now_iso_tiene_forma_iso_y_no_esta_vacio() {
        let s = now_iso();
        assert_eq!(s.len(), 20, "iso inesperado: {s}");
        assert!(s.ends_with('Z'));
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[10..11], "T");
    }
}
