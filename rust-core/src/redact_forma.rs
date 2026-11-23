//! Redacción de credenciales **por FORMA** (no por nombre de campo).
//!
//! MEDIDO (2026-11-23): la política de redacción por **nombre** de campo
//! (`api_key`, `Authorization`, `x-api-key`, `token`, `cookie`…) la tiene
//! el crate compartido `megadaemon-redact` (`crates/redact/`), que es la
//! fuente única para el shim, el payload de hooks y el del inyector.
//!
//! Ese enfoque tapa la credencial **cuando viene con un nombre reconocible**.
//! Se queda un hueco: una credencial que viaja **suelta**, sin nombre:
//!
//! ```text
//! {"session":"sk-cp-ABCDEFGHIJKLMNOP"}   <- "session" no es un patrón
//! https://host/?key=sk-cp-ABCDEFGHIJKLMNOP
//! ```
//!
//! ⇒ esta segunda pasada tapa los **prefijos típicos de token** (con su
//! valor detrás), que es lo que delata una credencial aunque nadie la
//! etiquete.
//!
//! DÓNDE DEBERÍA VIVIR
//! --------------------
//! Lo ideal es que esta función fuera **dentro** de `crates/redact/`
//! (la fuente única), pero esta rama está basada en `d57cae5`, **anterior**
//! a la creación de ese crate ⇒ meterlo aquí crearía un conflicto
//! *add/add* al fusionar.  Por eso vive en su propio módulo (que no
//! colisiona con nada) y el root lo puede mover a `crates/redact/src/lib.rs`
//! copiando esta función (es pura, sin dependencias).
//!
//! REGLAS
//! ------
//! * Un token de **menos de 12 caracteres** NO se redacta: `sk-3` o
//!   `AIza algo` son texto normal, no credenciales, y mutilarlos rompe
//!   logs legibles.
//! * La salida es texto plano (no introduce escapes nuevos).

/// Prefijos típicos de token de credencial.
const PREFIJOS: &[&str] = &[
    "sk-cp-",
    "sk-",
    "AIza",
    "gsk_",
    "xoxb-",
    "xapp-",
    "ghp_",
    "glpat-",
];

/// Longitud mínima para que un fragmento con prefijo se considere token.
const MINIMO: usize = 12;

/// Redacta los tokens que se cuelan **sin nombre de campo**.
pub fn redactar_por_forma(texto: &str) -> String {
    let mut out = texto.to_string();
    for pre in PREFIJOS {
        let mut desde = 0usize;
        while let Some(rel) = out[desde..].find(pre) {
            let i = desde + rel;
            let fin = out[i..]
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_' && c != '.')
                .map(|k| i + k)
                .unwrap_or(out.len());
            if fin - i >= MINIMO {
                out.replace_range(i..fin, "<REDACTADO>");
                desde = i + "<REDACTADO>".len();
            } else {
                desde = i + pre.len();
            }
            if desde >= out.len() {
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tapa_un_token_suelto() {
        let t = redactar_por_forma(r#"{"session":"sk-cp-ABCDEFGHIJKLMNOP"}"#);
        assert!(!t.contains("ABCDEFGHIJKLMNOP"), "no debe filtrar: {t}");
        assert!(t.contains("REDACTADO"));
    }

    #[test]
    fn tapa_los_prefijos_conocidos() {
        for (pre, cuerpo) in [
            ("AIza", "AIzaSyABCDEFGHIJKLMNOPQRST"),
            ("gsk_", "gsk_ABCDEFGHIJKLMNOPQRST"),
            ("xoxb-", "xoxb-ABCDEFGHIJKLMNOPQRST"),
            ("ghp_", "ghp_ABCDEFGHIJKLMNOPQRST"),
        ] {
            let t = redactar_por_forma(&format!("valor={pre}{cuerpo}"));
            assert!(!t.contains(cuerpo), "{pre} debe quedar redactado: {t}");
        }
    }

    #[test]
    fn no_toca_el_texto_normal() {
        let t = redactar_por_forma("esto es sk-3 corto y AIza algo");
        assert!(t.contains("sk-3"), "no debe mutilar texto normal: {t}");
        assert!(t.contains("AIza algo"), "no debe mutilar texto normal: {t}");
    }

    #[test]
    fn un_texto_limpio_no_cambia() {
        let t = "hola mundo, 12345, fin";
        assert_eq!(redactar_por_forma(t), t);
    }
}
