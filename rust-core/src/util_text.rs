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
