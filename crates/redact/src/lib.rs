//! Redacción de credenciales: **fuente única** de la política de secretos.
//!
//! POR QUÉ EXISTE (y por qué es un crate aparte)
//! --------------------------------------------
//! La misma política de redacción la necesitan TRES programas que **no
//! comparten proceso ni binario**:
//!
//! 1. El **shim** del daemon (`rust-core/src/agyshim.rs`), que redacta los
//!    cuerpos HTTP antes de guardarlos en su NDJSON.
//! 2. El **payload de hooks** (`crates/hooks-payload`), que se compila con
//!    frida-gum y se carga dentro de un proceso de terceros (Antigravity):
//!    escribe un NDJSON con lo que ve (SQL, red, ficheros).
//! 3. El **payload del inyector** (`crates/injector-payload`), la variante
//!    con `ptrace`.
//!
//! Si cada uno tuviera su propia copia, el payload podría filtrar un token
//! que el shim ya sabe tapar ⇒ el NDJSON del móvil acabaría en el bus.
//! Por eso la implementación vive AQUÍ y los tres la usan.
//!
//! REGLAS
//! ------
//! * Se redacta el **nombre** de la clave **y su valor**: si sólo se tapa el
//!   nombre, el secreto sigue ahí.
//! * Se redacta lo que parece una credencial, no lo que lo es con certeza:
//!   el objetivo es no filtrar, no adivinar.
//! * La salida es texto plano: no introduce JSON ni escapes nuevos, para que
//!   siga siendo legible en un log y en un `tail`.
//!
//! MEDIDO (2026-11-23, test rojo del payload `la_contrasena_y_la_cookie_no_salen`):
//! el terminador del NOMBRE de la clave no incluía `=`, así que en texto
//! plano (`password=hunter2 cookie=session-abc`) el nombre nunca terminaba
//! y el valor **no se redactaba**.  Corregido aquí (y por tanto en los tres
//! consumidores) junto con la unificación de los patrones.

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

/// Patrones EXTRA (los del payload de hooks).
///
/// A diferencia de los de arriba, aquí sólo se redacta si el nombre va
/// seguido de `=` o `:` (si no, una tabla SQLite llamada `tokens` saldría
/// mutilada y el registro de tablas perdería valor).
const PATRONES_EXTRA: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "token",
    "cookie",
    "set-cookie",
    "access_token",
    "refresh_token",
    "csrf_token",
    "x-api-key",
    "x-auth-token",
    "session_id",
];

/// Longitud máxima de un texto redactado.
const MAX_TEXTO: usize = 400;

/// Redacta lo que parece una credencial.
///
/// Devuelve el texto con los valores credenciales sustituidos por
/// `<REDACTADO>`.  Si no encuentra nada, devuelve el texto tal cual (copia).
pub fn redactar(texto: &str) -> String {
    let mut out = texto.to_string();
    for pat in PATRONES {
        out = redacta_patron(&out, pat, true);
    }
    for pat in PATRONES_EXTRA {
        out = redacta_patron(&out, pat, false);
    }
    out
}

/// Redacta un patrón concreto.
///
/// `valor_siempre` = `true`  ⇒ el valor se redacta aunque no haya `=`/`:`.
/// `valor_siempre` = `false` ⇒ sólo si el nombre va seguido de `=` o `:`.
fn redacta_patron(texto: &str, pat: &str, valor_siempre: bool) -> String {
    let mut out = texto.to_string();
    let mut desde = 0usize;
    while let Some(rel) = out[desde..].find(pat) {
        let i = desde + rel;
        // El cursor avanza por delante de cada redacción: si no, el texto ya
        // redactado ("api_key=<REDACTADO>") volvería a casar con el patrón.
        //
        // MEDIDO (2026-11-23): el terminador del nombre incluye `=`; sin él,
        // "password=hunter2" no tenía fin de nombre y el valor se quedaba
        // **en claro**.
        let fin_nombre = out[i..]
            .find(|c: char| c == '"' || c == ',' || c == '\n' || c == ':' || c == '=')
            .map(|k| i + k)
            .unwrap_or(out.len());
        let mut fin = fin_nombre;
        // Detrás del nombre puede venir la comilla de cierre del nombre
        // ("api_key":"valor") y después el separador y el valor.
        let mut pos = fin_nombre;
        if out[pos..].starts_with('"') {
            pos += 1;
        }
        let tras = out[pos..].trim_start();
        let inicio_trim = pos + (out[pos..].len() - tras.len());
        let sep = if tras.starts_with(':') {
            Some(':')
        } else if tras.starts_with('=') {
            Some('=')
        } else {
            None
        };
        if let Some(sep) = sep {
            // ':' es JSON ("api_key":"valor"); '=' es texto plano
            // ("password=hunter2").  El valor acaba en la coma/llave (JSON)
            // o en el espacio (texto plano).
            let inicio_valor = inicio_trim + 1;
            let tras2 = out[inicio_valor..].trim_start();
            let offset = out[inicio_valor..].len() - tras2.len();
            let inicio_real = inicio_valor + offset;
            if tras2.starts_with('"') {
                fin = match tras2[1..].find('"') {
                    Some(k) => inicio_real + 1 + k + 1,
                    None => out.len(),
                };
            } else if sep == ':' {
                fin = out[inicio_real..]
                    .find(|c: char| c == ',' || c == '\n' || c == '}')
                    .map(|k| inicio_real + k)
                    .unwrap_or(out.len());
            } else {
                fin = out[inicio_real..]
                    .find(|c: char| {
                        c.is_whitespace() || c == '"' || c == ',' || c == '}' || c == ';'
                    })
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
        } else if !valor_siempre {
            // Patrón extra sin `=` ni `:` detrás: se deja el texto intacto.
            desde = i + pat.len();
            if desde >= out.len() {
                break;
            }
            continue;
        }
        let relleno = format!("{pat}=<REDACTADO>");
        out.replace_range(i..fin, &relleno);
        desde = i + relleno.len();
        if desde >= out.len() {
            break;
        }
    }
    out
}

/// Quita las credenciales embebidas en una URL (`user:pass@host`).
pub fn quita_credenciales_url(texto: &str) -> String {
    let Some(p_esquema) = texto.find("://") else {
        return texto.to_string();
    };
    let ini = p_esquema + 3;
    let resto = &texto[ini..];
    let fin_host = resto.find('/').unwrap_or(resto.len());
    let autoridad = &resto[..fin_host];
    let Some(p_arroba) = autoridad.rfind('@') else {
        return texto.to_string();
    };
    let usuario = &autoridad[..p_arroba];
    if !usuario.contains(':') {
        return texto.to_string();
    }
    let solo_usuario = usuario.split(':').next().unwrap_or("");
    let nuevo = format!("{solo_usuario}:<REDACTADO>");
    format!("{}{}{}", &texto[..ini], nuevo, &resto[p_arroba..])
}

/// Redacta una sentencia SQL y la recorta.
pub fn redacta_sql(sql: &str) -> String {
    recorta(redactar(&quita_credenciales_url(sql)))
}

/// Redacta un texto corto (ruta, host, servicio).
pub fn redacta_corto(texto: &str) -> String {
    recorta(redactar(&quita_credenciales_url(texto)))
}

/// Recorta a `MAX_TEXTO` caracteres (sin partir en un UTF-8 inválido).
fn recorta(s: String) -> String {
    if s.chars().count() > MAX_TEXTO {
        let mut out: String = s.chars().take(MAX_TEXTO).collect();
        out.push_str("...");
        return out;
    }
    s
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

    // ── Los que vivían en el payload (MEDIDO 2026-11-23: el de la contraseña
    //    y la cookie era ROJO porque el terminador del nombre no incluía
    //    `=`; con la corrección, los tres consumidores los pasan).

    #[test]
    fn la_contrasena_y_la_cookie_no_salen() {
        let t = redactar("password=hunter2 cookie=session-abc");
        assert!(!t.contains("hunter2"), "{t}");
        assert!(!t.contains("session-abc"), "{t}");
    }

    #[test]
    fn una_tabla_que_se_llama_token_no_se_mutila() {
        // El patrón extra "token" sólo se redacta si va seguido de '=' o ':'.
        let t = redactar("INSERT INTO tokens (a) VALUES (1)");
        assert!(t.contains("tokens"), "{t}");
        assert!(!t.contains("REDACTADO"), "{t}");
    }

    #[test]
    fn quita_las_credenciales_de_la_url() {
        let t = quita_credenciales_url("https://user:clave-secreta@ejemplo.com/x");
        assert!(!t.contains("clave-secreta"), "{t}");
        assert!(t.contains("<REDACTADO>"), "{t}");
        assert!(t.contains("ejemplo.com"), "{t}");
    }

    #[test]
    fn una_url_sin_credenciales_no_cambia() {
        let t = quita_credenciales_url("https://ejemplo.com/x?a=1");
        assert_eq!(t, "https://ejemplo.com/x?a=1");
    }

    #[test]
    fn la_sql_se_recorta() {
        let s = redacta_sql(&"SELECT ".repeat(200));
        assert!(s.chars().count() <= 403, "{}", s.chars().count());
    }
}
