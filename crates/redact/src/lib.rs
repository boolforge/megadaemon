//! Redacción de credenciales: **fuente única** de la política de secretos.
//!
//! POR QUÉ EXISTE (y por qué es un crate aparte)
//! --------------------------------------------
//! La misma política de redacción la necesitan dos programas que **no
//! comparten proceso ni binario**:
//!
//! 1. El **shim** del daemon (`rust-core/src/agyshim.rs`), que redacta los
//!    cuerpos HTTP antes de guardarlos en su NDJSON.
//! 2. El **payload de hooks** (`crates/injector-payload`), que se inyecta con
//!    `ptrace` dentro del proceso de una app de terceros (Antigravity) y
//!    escribe un NDJSON con lo que ve: consultas SQLite, hosts de red y
//!    ficheros abiertos.
//!
//! Si cada uno tuviera su propia copia, el payload podría filtrar un token
//! que el shim ya sabe tapar ⇒ el NDJSON del móvil acabaría en el bus.
//! Por eso la implementación vive AQUÍ y los dos la usan.
//!
//! La función se **copió literalmente** de `agyshim::redactar` (MEDIDO
//! 2026-11-23, test rojo `redacta_credenciales`: la credencial se filtraba
//! al log) y `agyshim::redactar` **delegó** en esta desde ese mismo día.
//!
//! REGLAS
//! ------
//! * Se redacta el **nombre** de la clave **y su valor**: si sólo se tapa el
//!   nombre, el secreto sigue ahí.
//! * Se redacta lo que parece una credencial, no lo que lo es con certeza:
//!   el objetivo es no filtrar, no adivinar.
//! * La salida es texto plano: no introduce JSON ni escapes nuevos, para que
//!   siga siendo legible en un log y en un `tail`.

/// Patrones que delatan una credencial en texto plano.
///
/// `bearer ` acaba en espacio a propósito: el caso `Authorization: Bearer
/// sk-cp-...` se captura por el valor que sigue al token.
const PATRONES: &[&str] = &[
    "api_key",
    "apiKey",
    "Authorization",
    "bearer ",
    "x-goog-api-key",
];

/// Redacta lo que parece una credencial.
///
/// Devuelve el texto con los valores credenciales sustituidos por
/// `<REDACTADO>`.  Si no encuentra nada, devuelve el texto tal cual (copia).
///
/// MEDIDO (2026-11-23, test rojo `redacta_credenciales`): la credencial se
/// filtraba al log.  El arreglo-redacción copia aquí el comportamiento que ya
/// tenía el shim, sin cambiarlo.
pub fn redactar(texto: &str) -> String {
    let mut out = texto.to_string();
    for pat in PATRONES {
        // El cursor avanza por delante de cada redacción: si no, el texto ya
        // redactado ("api_key=<REDACTADO>") volvería a casar con el patrón
        // y el bucle no terminaría nunca.
        let mut desde = 0usize;
        while let Some(rel) = out[desde..].find(pat) {
            let i = desde + rel;
            // Se redacta el NOMBRE de la clave y su VALOR: si sólo se tapa el
            // nombre, el secreto sigue en el texto
            // ('{"api_key":"sk-cp-SECRETO"}' -> '{"api_key=<REDACTADO>":"sk-cp-SECRETO"}').
            let fin_nombre = out[i..]
                .find(|c: char| c == '"' || c == ',' || c == '\n' || c == ':')
                .map(|k| i + k)
                .unwrap_or(out.len());
            let mut fin = fin_nombre;
            // Detrás del nombre puede venir la comilla de cierre del nombre
            // ("api_key":"valor") y después el ':' y el valor.
            let mut pos = fin_nombre;
            if out[pos..].starts_with('"') {
                pos += 1;
            }
            let tras = out[pos..].trim_start();
            if tras.starts_with(':') {
                let inicio_valor = pos + (out[pos..].len() - tras.len()) + 1;
                let tras2 = out[inicio_valor..].trim_start();
                let offset = out[inicio_valor..].len() - tras2.len();
                let inicio_real = inicio_valor + offset;
                if tras2.starts_with('"') {
                    fin = match tras2[1..].find('"') {
                        Some(k) => inicio_real + 1 + k + 1,
                        None => out.len(),
                    };
                } else {
                    fin = out[inicio_real..]
                        .find(|c: char| c == ',' || c == '\n' || c == '}')
                        .map(|k| inicio_real + k)
                        .unwrap_or(out.len());
                }
            } else if pat.ends_with(' ') {
                // 'bearer sk-cp-SECRETO': el valor es el token siguiente.
                let inicio_real = pos + (out[pos..].len() - tras.len());
                fin = out[inicio_real..]
                    .find(|c: char| c.is_whitespace() || c == '"' || c == ',' || c == '}')
                    .map(|k| inicio_real + k)
                    .unwrap_or(out.len());
            }
            let relleno = format!("{pat}=<REDACTADO>");
            out.replace_range(i..fin, &relleno);
            desde = i + relleno.len();
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
    fn redacta_el_valor_de_una_api_key() {
        let t = redactar(r#"{"api_key":"sk-cp-MUY-SECRETO"}"#);
        assert!(!t.contains("MUY-SECRETO"), "la credencial se filtró: {t}");
        assert!(t.contains("REDACTADO"), "no marcó la redacción: {t}");
    }

    #[test]
    fn redacta_el_bearer() {
        let t = redactar("Authorization: Bearer sk-cp-OTRO-SECRETO");
        assert!(!t.contains("OTRO-SECRETO"), "la credencial se filtró: {t}");
    }

    #[test]
    fn redacta_el_valor_sin_comillas() {
        let t = redactar("api_key=sk-cp-1234, otra=cosa");
        assert!(!t.contains("sk-cp-1234"), "la credencial se filtró: {t}");
    }

    #[test]
    fn no_toca_texto_sin_credenciales() {
        let t = redactar("SELECT name FROM sqlite_master");
        assert_eq!(t, "SELECT name FROM sqlite_master");
    }

    #[test]
    fn no_entra_en_bucle_con_texto_ya_redactado() {
        // Si el cursor no avanzase, esto no terminaría nunca.
        let t = redactar("api_key=<REDACTADO>");
        assert_eq!(t, "api_key=<REDACTADO>");
    }
}
