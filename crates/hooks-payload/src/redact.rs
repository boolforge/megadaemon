//! REDACCIÓN de credenciales antes de escribir en el NDJSON.
//!
//! CRITERIO REUTILIZADO: el de `rust-core/src/agyshim.rs` (`redactar`).
//! Aquí **no** se puede llamar a `agyshim::redactar` (el payload es un crate
//! aparte: no puede depender del daemon, que arrastra 200 módulos), así que
//! se replica el **algoritmo y los patrones** de aquél, con dos añadidos que
//! el shim no necesitaba:
//!
//! * el shim redactaba *cuerpos HTTP*; el payload además registra rutas,
//!   sentencias SQL y pares host:puerto, donde aparecen `password`,
//!   `cookie` o tokens de sesión;
//! * una URL con credenciales embebidas (`https://user:pass@host/…`) pierde
//!   la parte sensible.
//!
//! NUNCA se escribe en claro: una API key, un bearer, una cookie de sesión o
//! una contraseña de URL.

/// Patrones EXACTOS de `agyshim::redactar` (mismo criterio, mismo formato de
/// salida: `patron=<REDACTADO>`).
const PATRONES_AGYSHIM: &[&str] = &[
    "api_key",
    "apiKey",
    "Authorization",
    "bearer ",
    "x-goog-api-key",
];

/// Patrones EXTRA del payload.  A diferencia de los de arriba, aquí sólo se
/// redacta si el nombre va seguido de `=` o `:` (si no, una tabla SQLite
/// llamada `tokens` saldría mutilada y el registro de tablas perdería valor).
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

/// Longitud máxima de un texto redactado (el shim usaba 400).
const MAX_TEXTO: usize = 400;

/// Redacta lo que parece una credencial (algoritmo de `agyshim::redactar`).
///
/// Réplica del criterio del shim: se tapa el NOMBRE de la clave **y** su
/// VALOR (`{"api_key":"sk-secreto"}` → `{"api_key=<REDACTADO>"}`); si sólo
/// se tapara el nombre, el secreto seguiría en la línea.
pub fn redactar(texto: &str) -> String {
    let mut out = texto.to_string();
    for pat in PATRONES_AGYSHIM {
        out = redacta_patron(&out, pat, true);
    }
    for pat in PATRONES_EXTRA {
        out = redacta_patron(&out, pat, false);
    }
    out
}

/// Redacta un patrón concreto.
///
/// `valor_siempre`:
/// * `true`  → comportamiento de `agyshim::redactar`: si detrás del nombre no
///   hay un valor reconocible, se tapa igualmente el nombre.
/// * `false` → sólo se redacta si detrás del nombre hay `=` o `:` (o, para
///   los patrones que acaban en espacio, el token siguiente).
fn redacta_patron(texto: &str, pat: &str, valor_siempre: bool) -> String {
    let mut out = texto.to_string();
    let mut desde = 0usize;
    while let Some(rel) = out[desde..].find(pat) {
        let i = desde + rel;
        // El cursor avanza por delante de cada redacción: si no, el texto ya
        // redactado ("api_key=<REDACTADO>") volvería a casar con el patrón.
        let fin_nombre = out[i..]
            .find(|c: char| c == '"' || c == ',' || c == '\n' || c == ':')
            .map(|k| i + k)
            .unwrap_or(out.len());
        let mut fin = fin_nombre;
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
                    .find(|c: char| c == ',' || c == '\n' || c == '}' || c == ';')
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
            // Patrón extra: sólo si va seguido de '=' o ':'.
            let sep = out[pos..].find(|c: char| c == '=' || c == ':');
            match sep {
                Some(k) => {
                    let inicio = pos + k + 1;
                    fin = out[inicio..]
                        .find(|c: char| {
                            c.is_whitespace() || c == '"' || c == ',' || c == '}' || c == ';'
                        })
                        .map(|k| inicio + k)
                        .unwrap_or(out.len());
                }
                None => {
                    // No hay valor detrás: se deja el texto intacto y se
                    // avanza el cursor para no volver a casar.
                    desde = i + pat.len();
                    if desde >= out.len() {
                        break;
                    }
                    continue;
                }
            }
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

/// Quita las credenciales embebidas de una URL (`user:pass@host`).
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
    let s = redactar(&quita_credenciales_url(sql));
    if s.chars().count() > MAX_TEXTO {
        let mut out: String = s.chars().take(MAX_TEXTO).collect();
        out.push_str("...");
        return out;
    }
    s
}

/// Redacta un texto corto (ruta, host, servicio).
pub fn redacta_corto(texto: &str) -> String {
    let s = redactar(&quita_credenciales_url(texto));
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
    fn redacta_credenciales() {
        // Mismo test que el del shim (`agyshim.rs`): la clave NO puede
        // filtrarse al log.
        let t = redactar(r#"{"api_key":"sk-cp-MUY-SECRETO"}"#);
        assert!(!t.contains("MUY-SECRETO"), "no debe filtrar la clave: {t}");
        assert!(t.contains("REDACTADO"), "{t}");
    }

    #[test]
    fn redacta_el_bearer() {
        let t = redactar("Authorization: Bearer sk-ant-SECRETO123");
        assert!(!t.contains("SECRETO123"), "{t}");
        assert!(t.contains("REDACTADO"), "{t}");
    }

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
