//! JSON mínimo para el NDJSON del payload.
//!
//! El payload **no** arrastra `serde`/`serde_json` a propósito: se carga en
//! el proceso de una app de terceros y cada dependencia es peso y superficie
//! de ataque.  Aquí sólo hace faltaescapar cadenas y montar pares
//! `"clave":"valor"`.

/// Escapa una cadena para JSON y la devuelve **sin** comillas.
///
/// Reglas (RFC 8259): `"` → `\"`, `\` → `\\`, control (< 0x20) → `\u00XX`.
/// Los bytes UTF-8 inválidos se pierden (el texto de los hooks es ASCII o
/// UTF-8 válido; si no lo es, se degrada a texto plano en vez de romper la
/// línea).
pub fn escapa(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// Devuelve el par `"clave":"valor"` (escapado) listo para un objeto JSON.
pub fn campo(clave: &str, valor: &str) -> String {
    format!("\"{}\":\"{}\"", escapa(clave), escapa(valor))
}

/// Devuelve el par `"clave":<numero>` listo para un objeto JSON.
pub fn campo_num(clave: &str, valor: u64) -> String {
    format!("\"{}\":{}", escapa(clave), valor)
}

/// Une los campos de un objeto JSON (sin llaves) con comas.
pub fn une(campos: &[String]) -> String {
    campos.join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapa_las_comillas_y_el_salto_de_linea() {
        assert_eq!(escapa("a\"b"), "a\\\"b");
        assert_eq!(escapa("a\\b"), "a\\\\b");
        assert_eq!(escapa("a\nb"), "a\\nb");
        assert_eq!(escapa("a\u{1}b"), "a\\u0001b");
    }

    #[test]
    fn el_campo_queda_escapado() {
        assert_eq!(campo("sql", "SELECT 'x'"), "\"sql\":\"SELECT 'x'\"");
        assert_eq!(campo_num("puerto", 443), "\"puerto\":443");
    }
}
