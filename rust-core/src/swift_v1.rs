//! Formato contenedor **`v1:::`** de Swift Backup (medido el 2026-11-21).
//!
//! # Qué es
//!
//! Swift Backup (`org.swiftapps.swiftbackup`) guarda cada pieza de una copia
//! de seguridad como un fichero de texto con esta forma:
//!
//! ```text
//! v1:::<base64 bloque 1>:::<base64 bloque 2>
//! ```
//!
//! Medido en el móvil (cuenta `fcf724b2ae9147c8`), en
//! `apps/cloud/com.samsung.android.mtp/20261119-063712-BY/`:
//!
//! | Fichero | Bytes | Contenido |
//! |---|---|---|
//! | `com.samsung.android.mtp.xml`  | 374 | metadatos de la copia MTP |
//! | `com.samsung.android.mtp.extra`| 788 | datos de la copia MTP |
//!
//! # Lo que se sabe (medido, no supuesto)
//!
//! * El prefijo textual es exactamente `v1:::` (dos bloques separados por
//!   `:::`).
//! * Los bloques van en **base64 estándar** con saltos de línea (76
//!   caracteres), como `MimeEncoder`/`Base64.encodeToString` de Java.
//! * **Los dos bloques empiezan por los mismos dos bytes: `01 02`.**
//!   ⇒ hay una cabecera binaria común de 2 bytes antes del contenido.
//! * A partir de ahí, los bytes son de **entropía alta** (cifrados o
//!   comprimidos): no son texto ni XML.
//! * El `base64` **no** empieza con un `SBA1`: este contenedor es
//!   **distinto** del formato de exportación `.dat`/`SBA1` que ya
//!   documenta `swift_archive.rs`.
//!
//! # Lo que NO se sabe (y no se inventa)
//!
//! * Qué significan exactamente `01 02` (versión + tipo, o flags).
//! * Qué cifrado se usa (ni siquiera si lo hay: podría ser sólo compresión).
//! * Cómo se relacionan `01 02` con el `SBA1` del export.
//!
//! Por eso este módulo **no descifra nada**: **parsea, separa, mide y
//! reporta**. Cuando se traiga el fichero completo (o el APK) se podrá
//! cerrar el formato con evidencia.
//!
//! # Uso
//!
//! ```no_run
//! let v = swift_v1::parse_bytes(b"v1:::AAAA\n:::\nBBBB").unwrap();
//! assert_eq!(v.bloques.len(), 2);
//! assert!(v.bloques[0].empieza_con_0102());
//! ```

use serde_json::{json, Value};

/// Prefijo textual del contenedor.
pub const PREFIJO: &str = "v1:::";

/// Separador entre bloques.
pub const SEPARADOR: &str = ":::";

/// Bytes que preceden al contenido en cada bloque (medido: `01 02`).
pub const CABECERA_COMUN: [u8; 2] = [0x01, 0x02];

/// Un bloque binario dentro del contenedor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bloque {
    /// Posición (0 = primero).
    pub indice: usize,
    /// Bytes decodificados.
    pub datos: Vec<u8>,
}

impl Bloque {
    /// Longitud en bytes.
    pub fn len(&self) -> usize {
        self.datos.len()
    }

    /// ¿Está vacío?
    pub fn is_empty(&self) -> bool {
        self.datos.is_empty()
    }

    /// ¿Empieza por la cabecera común `01 02`?
    pub fn empieza_con_0102(&self) -> bool {
        self.datos.len() >= 2 && self.datos[0] == CABECERA_COMUN[0] && self.datos[1] == CABECERA_COMUN[1]
    }

    /// Bytes de cabecera tal cual (los 2 primeros, o lo que haya).
    pub fn cabecera(&self) -> Vec<u8> {
        self.datos.iter().take(2).copied().collect()
    }

    /// Contenido sin la cabecera.
    pub fn cuerpo(&self) -> &[u8] {
        if self.datos.len() > 2 {
            &self.datos[2..]
        } else {
            &[]
        }
    }

    /// Entropía de Shannon (bits por byte, máximo 8).
    ///
    /// Sirve para distinguir "cifrado/comprimido" (≈8) de "texto/estructura"
    /// (mucho menos) **sin adivinar el algoritmo**.
    pub fn entropia(&self) -> f64 {
        entropia(&self.datos)
    }

    /// ¿El cuerpo parece texto plano imprimible?
    pub fn parece_texto(&self) -> bool {
        let c = self.cuerpo();
        if c.is_empty() {
            return false;
        }
        let imprimibles = c
            .iter()
            .filter(|b| b.is_ascii_graphic() || **b == b'\n' || **b == b'\r' || **b == b'\t')
            .count();
        imprimibles * 100 / c.len() >= 90
    }
}

/// Entropía de Shannon en bits por byte.
pub fn entropia(datos: &[u8]) -> f64 {
    if datos.is_empty() {
        return 0.0;
    }
    let mut counts = [0usize; 256];
    for b in datos {
        counts[*b as usize] += 1;
    }
    let n = datos.len() as f64;
    let mut h = 0.0f64;
    for c in counts.iter() {
        if *c > 0 {
            let p = *c as f64 / n;
            h -= p * p.log2();
        }
    }
    h
}

/// Contenedor `v1:::` parseado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contenedor {
    /// Bloques decodificados.
    pub bloques: Vec<Bloque>,
    /// Longitud del texto original.
    pub longitud_texto: usize,
}

impl Contenedor {
    /// ¿Todos los bloques empiezan por `01 02`?
    pub fn cabecera_consistente(&self) -> bool {
        !self.bloques.is_empty() && self.bloques.iter().all(|b| b.empieza_con_0102())
    }

    /// Informe en JSON (para documentar y comparar versiones).
    pub fn informe(&self) -> Value {
        json!({
            "formato": "v1:::",
            "longitud_texto": self.longitud_texto,
            "bloques": self.bloques.len(),
            "cabecera_consistente": self.cabecera_consistente(),
            "detalle": self.bloques.iter().map(|b| json!({
                "indice": b.indice,
                "bytes": b.len(),
                "cabecera": b.cabecera().iter().map(|x| format!("{x:02x}")).collect::<Vec<String>>(),
                "entropia_bits_por_byte": (b.entropia() * 100.0).round() / 100.0,
                "parece_texto": b.parece_texto(),
            })).collect::<Vec<Value>>(),
        })
    }
}

/// Error al parsear el contenedor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// No empieza por `v1:::`.
    PrefijoInvalido,
    /// base64 inválido.
    Base64Invalido(String),
    /// Menos de dos bloques.
    BloquesInsuficientes,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::PrefijoInvalido => write!(f, "el fichero no empieza por 'v1:::'"),
            Error::Base64Invalido(e) => write!(f, "base64 invalido: {e}"),
            Error::BloquesInsuficientes => write!(f, "hacen falta al menos dos bloques"),
        }
    }
}

/// Parsea el texto del contenedor.
///
/// Acepta saltos de línea dentro del base64 (los mete `Base64.encodeToString`
/// de Java con `BASE64` normal, que corta a 76 caracteres).
pub fn parse_texto(texto: &str) -> Result<Contenedor, Error> {
    let t = texto.trim();
    if !t.starts_with(PREFIJO) {
        return Err(Error::PrefijoInvalido);
    }
    let resto = &t[PREFIJO.len()..];
    let partes: Vec<&str> = resto.split(SEPARADOR).collect();
    if partes.len() < 2 {
        return Err(Error::BloquesInsuficientes);
    }
    let mut bloques = Vec::new();
    for (i, p) in partes.iter().enumerate() {
        let limpio: String = p.chars().filter(|c| !c.is_whitespace()).collect();
        if limpio.is_empty() {
            continue;
        }
        let datos = base64_decode(&limpio).map_err(Error::Base64Invalido)?;
        bloques.push(Bloque { indice: i, datos });
    }
    if bloques.len() < 2 {
        return Err(Error::BloquesInsuficientes);
    }
    Ok(Contenedor {
        bloques,
        longitud_texto: t.len(),
    })
}

/// Parsea desde bytes.
pub fn parse_bytes(datos: &[u8]) -> Result<Contenedor, Error> {
    let texto = String::from_utf8_lossy(datos);
    parse_texto(&texto)
}

/// Base64 estándar (con `+` y `/`), sin relleno `=` opcional.
fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    const TABLA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut rev = [255u8; 256];
    for (i, c) in TABLA.iter().enumerate() {
        rev[*c as usize] = i as u8;
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for c in s.bytes() {
        if c == b'=' {
            break;
        }
        // El base64 de Java mete saltos de linea: se ignoran.
        if c.is_ascii_whitespace() {
            continue;
        }
        let v = rev[c as usize];
        if v == 255 {
            return Err(format!("caracter invalido {:?}", c as char));
        }
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

/// Base64 estándar con relleno (para escribir).
pub fn base64_encode(datos: &[u8]) -> String {
    const TABLA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for (i, trio) in datos.chunks(3).enumerate() {
        let b0 = trio[0] as u32;
        let b1 = *trio.get(1).unwrap_or(&0) as u32;
        let b2 = *trio.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLA[(n >> 18) as usize & 63] as char);
        out.push(TABLA[(n >> 12) as usize & 63] as char);
        out.push(if trio.len() > 1 {
            TABLA[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if trio.len() > 2 {
            TABLA[n as usize & 63] as char
        } else {
            '='
        });
        // Saltos de línea cada 76 caracteres (como `Base64.encodeToString`).
        if (i + 1) % 19 == 0 && i + 1 < datos.len().div_ceil(3) {
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Muestra REAL medida en el móvil (20261119-063712-BY, .xml).
    const MUESTRA_XML: &str = "v1:::AQL7810KHg6TWid2NqFGI88/CP6v5Wn//iaXz4cFqqotFMciY+5hedNMz+WHjDjmNdh1PC2Q0fUJ\niw==\n:::AQKE+kIheaARMujL8+C/pPbwvnJqlqfObqorVjW1oB5SGu/4g7+AA8IJqD62zWxI8gfgb3M/jQ0a\nFjljmMzWFo0PZJIVPZRMu6hHq9tmpIKENST1N+BONuGs6uMQPOrgtHv/qy+Pvci7vvDwLBckDivi\nIGrc8AYAVoO9IcCt44VjZMXU67UqR4BmF3ajNqvIzJic1f9qZkP9G6nc\n";

    #[test]
    fn la_muestra_real_se_parsea() {
        let c = parse_texto(MUESTRA_XML).expect("debe parsear");
        assert_eq!(c.bloques.len(), 2, "dos bloques");
        assert_eq!(c.bloques[0].len(), 58);
        assert_eq!(c.bloques[1].len(), 156);
    }

    #[test]
    fn los_dos_bloques_empiezan_por_0102() {
        let c = parse_texto(MUESTRA_XML).unwrap();
        assert!(c.bloques[0].empieza_con_0102(), "bloque 0");
        assert!(c.bloques[1].empieza_con_0102(), "bloque 1");
        assert!(c.cabecera_consistente());
    }

    #[test]
    fn el_cuerpo_es_de_alta_entropia() {
        // OJO: la entropia maxima de una muestra de n bytes es log2(n).  Con
        // 58 bytes el techo es 5,86 bits/byte: pedir >7 seria imposible.
        // Lo que se comprueba es que el cuerpo este cerca del techo, es
        // decir: distribution uniforme, no texto ni estructura.
        let c = parse_texto(MUESTRA_XML).unwrap();
        for b in &c.bloques {
            let techo = (b.len() as f64).log2();
            assert!(
                b.entropia() > techo * 0.9,
                "bloque {}: entropia {} de un techo de {techo}",
                b.indice,
                b.entropia()
            );
            assert!(!b.parece_texto(), "bloque {} no es texto", b.indice);
        }
    }

    #[test]
    fn el_prefijo_invalido_falla() {
        assert_eq!(parse_texto("hola"), Err(Error::PrefijoInvalido));
        assert_eq!(parse_texto("v2:::AAA:::BBB"), Err(Error::PrefijoInvalido));
    }

    #[test]
    fn un_solo_bloque_no_suficiente() {
        assert_eq!(parse_texto("v1:::AAAA"), Err(Error::BloquesInsuficientes));
    }

    #[test]
    fn el_base64_invalido_falla() {
        assert!(matches!(
            parse_texto("v1:::!!!!:::AAAA"),
            Err(Error::Base64Invalido(_))
        ));
    }

    #[test]
    fn base64_ida_y_vuelta() {
        for datos in [
            vec![],
            vec![0u8],
            vec![0u8, 1],
            vec![0u8, 1, 2],
            (0..=255u8).collect::<Vec<u8>>(),
            b"hola mundo".to_vec(),
        ] {
            let s = base64_encode(&datos);
            assert_eq!(base64_decode(&s).unwrap(), datos, "ida y vuelta");
        }
    }

    #[test]
    fn el_base64_de_java_usa_saltos_de_76() {
        // 57 bytes -> 76 caracteres -> un salto de linea.
        let datos = vec![0xABu8; 57];
        let s = base64_encode(&datos);
        let primera = s.lines().next().unwrap();
        assert_eq!(primera.len(), 76);
    }

    #[test]
    fn el_informe_es_json_valido() {
        let c = parse_texto(MUESTRA_XML).unwrap();
        let j = c.informe();
        assert_eq!(j["formato"], json!("v1:::"));
        assert_eq!(j["bloques"], json!(2));
        assert_eq!(j["cabecera_consistente"], json!(true));
        assert!(j["detalle"].is_array());
    }

    #[test]
    fn la_entropia_de_un_texto_es_baja() {
        let e = entropia(b"aaaaaaaa");
        assert!(e < 1.0, "entropia de texto repetido: {e}");
        let e2 = entropia(&(0..=255u8).collect::<Vec<u8>>());
        assert!(e2 > 7.9, "entropia de bytes aleatorios: {e2}");
    }
}
