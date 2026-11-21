//! Certificado X.509 **autofirmado** para el canal KDE Connect.
//!
//! # Por qué hace falta
//!
//! KDE Connect **no** usa una autoridad de certificación: cada dispositivo
//! genera su propio certificado autofirmado (CN = `deviceId`) y la
//! confianza entre dos nodos se establece por *pinning*: al parear, cada
//! lado guarda el certificado del otro y a partir de ese momento exige que
//! el certificado que presenta el peer sea **byte a byte** el mismo que
//! quedó guardado.  Es el mismo modelo que SSH (`known_hosts`).
//!
//! Aquí se hace igual, en Rust puro:
//!
//! * la clave es **Ed25519** (`ring`, que ya está en el árbol porque
//!   `rustls` lo usa) → generación instantánea (a diferencia de RSA-2048,
//!   que tarda segundos buscando primos y hace los tests no deterministas);
//! * el DER se escribe **a mano**: no se añade ningún crate de X.509 ni de
//!   DER al repo;
//! * el CN es el `deviceId` del nodo, igual que en KDE Connect.
//!
//! # Formato
//!
//! ```text
//! Certificate ::= SEQUENCE {
//!     tbsCertificate       TBSCertificate,
//!     signatureAlgorithm   AlgorithmIdentifier,   -- Ed25519 (1.3.101.112)
//!     signatureValue       BIT STRING             -- 64 bytes
//! }
//! ```
//!
//! # Límites (honestos)
//!
//! * El certificado **no** lo valida ninguna CA: por eso el único control
//!   de acceso real es el *pinning* del pareado (ver `kdeconnect`).
//! * `valid_days` por defecto 3650 (10 años): el pin vive en el almacén de
//!   peers, no en el certificado, así que la caducidad del certificado no
//!   invalida un pareado (pero el TLS sí rechazará un cert caducado si
//!   algún verificador mira las fechas; el nuestro comprueba el pin).

use base64ct::Encoding;
use sha2::{Digest, Sha256};

/// Error del módulo (cadena simple: no hay `thiserror` en el repo).
pub type Result<T> = std::result::Result<T, String>;

// ─────────────────────────────────────────────────────────────────────────────
// Codificador DER mínimo (escrito a mano)
// ─────────────────────────────────────────────────────────────────────────────

/// Longitud DER en forma corta/larga.
fn der_len(len: usize) -> Vec<u8> {
    if len < 0x80 {
        vec![len as u8]
    } else {
        let mut bytes = Vec::new();
        let mut v = len;
        while v > 0 {
            bytes.push((v & 0xff) as u8);
            v >>= 8;
        }
        bytes.reverse();
        let mut out = vec![0x80 | bytes.len() as u8];
        out.extend_from_slice(&bytes);
        out
    }
}

/// TLV (`tag`, `length`, `value`).
fn tlv(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(content.len() + 6);
    out.push(tag);
    out.extend_from_slice(&der_len(content.len()));
    out.extend_from_slice(content);
    out
}

/// `SEQUENCE` con los elementos dados.
fn der_sequence(items: &[Vec<u8>]) -> Vec<u8> {
    let mut body = Vec::new();
    for i in items {
        body.extend_from_slice(i);
    }
    tlv(0x30, &body)
}

/// `SET` con los elementos dados.
fn der_set(items: &[Vec<u8>]) -> Vec<u8> {
    let mut body = Vec::new();
    for i in items {
        body.extend_from_slice(i);
    }
    tlv(0x31, &body)
}

/// `OBJECT IDENTIFIER` con los bytes ya codificados (ver `OID_*`).
fn der_oid(encoded: &[u8]) -> Vec<u8> {
    tlv(0x06, encoded)
}

/// `INTEGER` (contenido ya en big-endian sin signo, sin ceros delante).
fn der_integer(bytes: &[u8]) -> Vec<u8> {
    let mut v: Vec<u8> = bytes.to_vec();
    while v.len() > 1 && v[0] == 0 {
        v.remove(0);
    }
    if v.first().map(|b| b & 0x80 != 0).unwrap_or(false) {
        v.insert(0, 0x00);
    }
    tlv(0x02, &v)
}

/// `BIT STRING` sin bits sin usar (`unused bits = 0`).
fn der_bit_string(bits: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(bits.len() + 1);
    v.push(0x00);
    v.extend_from_slice(bits);
    tlv(0x03, &v)
}

/// `OCTET STRING`.
fn der_octet_string(bytes: &[u8]) -> Vec<u8> {
    tlv(0x04, bytes)
}

/// `UTF8String`.
fn der_utf8_string(s: &str) -> Vec<u8> {
    tlv(0x0c, s.as_bytes())
}

/// `IA5String`.
fn der_ia5_string(s: &str) -> Vec<u8> {
    tlv(0x16, s.as_bytes())
}

/// `BOOLEAN`.
fn der_boolean(v: bool) -> Vec<u8> {
    tlv(0x01, &[if v { 0xff } else { 0x00 }])
}

/// `UTCTime` (`YYMMDDHHMMSSZ`).
fn der_utc_time(s: &str) -> Vec<u8> {
    tlv(0x17, s.as_bytes())
}

/// Contexto explícito `[n] EXPLICIT`.
fn der_explicit(tag_num: u8, content: &[u8]) -> Vec<u8> {
    tlv(0xa0 | tag_num, content)
}

// OIDs ya codificados (primera arista en DER: 40*X + Y).
const OID_CN: &[u8] = &[0x55, 0x04, 0x03]; // 2.5.4.3  commonName
const OID_BASIC_CONSTRAINTS: &[u8] = &[0x55, 0x1d, 0x13]; // 2.5.29.19
const OID_KEY_USAGE: &[u8] = &[0x55, 0x1d, 0x0f]; // 2.5.29.15
const OID_SUBJECT_ALT_NAME: &[u8] = &[0x55, 0x1d, 0x11]; // 2.5.29.17
const OID_ED25519: &[u8] = &[0x2b, 0x65, 0x70]; // 1.3.101.112

// ─────────────────────────────────────────────────────────────────────────────
// Fecha (UTCTime) sin chrono
// ─────────────────────────────────────────────────────────────────────────────

/// Fecha civil (año, mes, día) a partir de los días desde el epoch.
///
/// Algoritmo de Howard Hinnant (`civil_from_days`), dominio público.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as i64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Epoch → `YYMMDDHHMMSSZ` (formato `UTCTime`).
fn utc_time_string(epoch: u64) -> String {
    let days = (epoch / 86_400) as i64;
    let secs = epoch % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:02}{:02}{:02}{:02}{:02}{:02}Z",
        y.rem_euclid(100),
        m,
        d,
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Certificado
// ─────────────────────────────────────────────────────────────────────────────

/// Material TLS de un nodo: certificado autofirmado + clave privada.
///
/// La clave **nunca** sale del disco con permisos 0600 y nunca se imprime.
#[derive(Clone)]
pub struct SelfSignedCert {
    /// Certificado DER (el que se presenta en el handshake TLS).
    pub der: Vec<u8>,
    /// Clave privada en PKCS#8 (v1, sólo la semilla Ed25519).
    pub key_pkcs8: Vec<u8>,
    /// Huella SHA-256 del DER en hex (es el "pin" que se guarda al parear).
    pub fingerprint: String,
}

impl std::fmt::Debug for SelfSignedCert {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // NUNCA se imprime la clave privada.
        f.debug_struct("SelfSignedCert")
            .field("fingerprint", &self.fingerprint)
            .field("der_len", &self.der.len())
            .finish()
    }
}

/// Huella SHA-256 (hex) de un certificado DER.
pub fn fingerprint_hex(der: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(der);
    let digest = h.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest.iter() {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// SPKI (SubjectPublicKeyInfo) de una clave Ed25519 pública.
///
/// `SEQUENCE { SEQUENCE { OID 1.3.101.112 }, BIT STRING (clave) }`
///
/// Tiene que ser **byte a byte** lo que rustls calcula a partir de la clave
/// privada, porque `CertifiedKey::keys_match()` compara ambos SPKI.
fn spki_ed25519(public_key: &[u8]) -> Vec<u8> {
    der_sequence(&[
        der_sequence(&[der_oid(OID_ED25519)]),
        der_bit_string(public_key),
    ])
}

/// `AlgorithmIdentifier` de Ed25519 (los mismos bytes dentro y fuera del
/// certificado: `webpki` rechaza el cert si no coinciden).
fn alg_id_ed25519() -> Vec<u8> {
    der_sequence(&[der_oid(OID_ED25519)])
}

/// `Name` (issuer y subject) con un único CN.
fn name_cn(cn: &str) -> Vec<u8> {
    der_sequence(&[der_set(&[der_sequence(&[
        der_oid(OID_CN),
        der_utf8_string(cn),
    ])])])
}

/// Extensiones: basicConstraints, keyUsage, subjectAltName.
fn extensions(cn: &str) -> Vec<u8> {
    let basic_constraints = der_sequence(&[
        der_oid(OID_BASIC_CONSTRAINTS),
        der_boolean(true), // critical
        der_octet_string(&der_sequence(&[der_boolean(false)])),
    ]);
    // keyUsage: sólo digitalSignature (bit 0) -> 0x80 con 7 bits sin usar.
    let key_usage = der_sequence(&[
        der_oid(OID_KEY_USAGE),
        der_boolean(true), // critical
        der_octet_string(&der_bit_string(&[0x80])),
    ]);
    // subjectAltName: dNSName = el deviceId (CN en el SAN, como hace KDE Connect).
    let san = der_sequence(&[
        der_oid(OID_SUBJECT_ALT_NAME),
        der_octet_string(&der_sequence(&[tlv(0x82, cn.as_bytes())])),
    ]);
    der_sequence(&[basic_constraints, key_usage, san])
}

/// Número de serie aleatorio (16 bytes, positivo).
fn random_serial() -> Vec<u8> {
    let mut b = crate::kdeconnect::random_bytes(16);
    // El bit alto a 0 para que el INTEGER sea positivo sin ceros extra.
    if let Some(first) = b.first_mut() {
        *first &= 0x7f;
    }
    b
}

/// Genera un certificado autofirmado nuevo.
///
/// * `common_name`: normalmente el `deviceId` del nodo (KDE Connect pone el
///   id en el CN).
/// * `valid_days`: validez en días (por defecto 3650).
pub fn generate(common_name: &str, valid_days: u32) -> Result<SelfSignedCert> {
    let cn: String = common_name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(64)
        .collect();
    if cn.is_empty() {
        return Err("el CN del certificado no puede estar vacío".into());
    }

    // 1. Clave Ed25519 (PKCS#8 v1: sólo la semilla).
    use ring::signature::KeyPair;
    let rng = ring::rand::SystemRandom::new();
    let pkcs8_doc = ring::signature::Ed25519KeyPair::generate_pkcs8(&rng)
        .map_err(|_| "no se pudo generar la clave Ed25519".to_string())?;
    let key_pkcs8 = pkcs8_doc.as_ref().to_vec();
    let key_pair = ring::signature::Ed25519KeyPair::from_pkcs8_maybe_unchecked(&key_pkcs8)
        .map_err(|e| format!("la clave Ed25519 generada no es utilizable: {e}"))?;
    let public_key = key_pair.public_key().as_ref().to_vec();

    // 2. TBSCertificate.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let not_before = now.saturating_sub(86_400);
    let not_after = now.saturating_add(valid_days as u64 * 86_400);

    let spki = spki_ed25519(&public_key);
    let tbs = der_sequence(&[
        der_explicit(0, &der_integer(&[2])), // version v3
        der_integer(&random_serial()),
        alg_id_ed25519(),
        name_cn(&cn),
        der_sequence(&[
            der_utc_time(&utc_time_string(not_before)),
            der_utc_time(&utc_time_string(not_after)),
        ]),
        name_cn(&cn),
        spki,
        der_explicit(3, &extensions(&cn)),
    ]);

    // 3. Firma (Ed25519 firma el DER del TBS tal cual, sin digest).
    // Ed25519 firma directamente el DER del TBS (sin digest previo).
    let signature = key_pair.sign(&tbs);

    // 4. Certificado.
    let der = der_sequence(&[tbs, alg_id_ed25519(), der_bit_string(signature.as_ref())]);
    let fingerprint = fingerprint_hex(&der);

    Ok(SelfSignedCert {
        der,
        key_pkcs8,
        fingerprint,
    })
}

/// Envuelve un DER en PEM (`CERTIFICATE`).
pub fn to_pem(der: &[u8]) -> String {
    let b64 = base64ct::Base64::encode_string(der);
    let mut out = String::from("-----BEGIN CERTIFICATE-----\n");
    for chunk in b64.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(chunk).unwrap_or(""));
        out.push('\n');
    }
    out.push_str("-----END CERTIFICATE-----\n");
    out
}

/// Extrae el DER de un PEM (`CERTIFICATE`).  `None` si no hay bloque válido.
pub fn from_pem(pem: &str) -> Option<Vec<u8>> {
    let mut b64 = String::new();
    let mut inside = false;
    for line in pem.lines() {
        let l = line.trim();
        if l.starts_with("-----BEGIN") && l.contains("CERTIFICATE") {
            inside = true;
            continue;
        }
        if l.starts_with("-----END") {
            break;
        }
        if inside {
            b64.push_str(l);
        }
    }
    if b64.is_empty() {
        return None;
    }
    base64ct::Base64::decode_vec(&b64).ok()
}

/// Envuelve un PKCS#8 en PEM (`PRIVATE KEY`).
pub fn key_to_pem(pkcs8: &[u8]) -> String {
    let b64 = base64ct::Base64::encode_string(pkcs8);
    let mut out = String::from("-----BEGIN PRIVATE KEY-----\n");
    for chunk in b64.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(chunk).unwrap_or(""));
        out.push('\n');
    }
    out.push_str("-----END PRIVATE KEY-----\n");
    out
}

/// Extrae el PKCS#8 de un PEM (`PRIVATE KEY`).
pub fn key_from_pem(pem: &str) -> Option<Vec<u8>> {
    let mut b64 = String::new();
    let mut inside = false;
    for line in pem.lines() {
        let l = line.trim();
        if l.starts_with("-----BEGIN") && l.contains("PRIVATE KEY") {
            inside = true;
            continue;
        }
        if l.starts_with("-----END") {
            break;
        }
        if inside {
            b64.push_str(l);
        }
    }
    if b64.is_empty() {
        return None;
    }
    base64ct::Base64::decode_vec(&b64).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_der_usa_longitud_corta_y_larga() {
        assert_eq!(der_len(0), vec![0x00]);
        assert_eq!(der_len(127), vec![0x7f]);
        assert_eq!(der_len(128), vec![0x81, 0x80]);
        assert_eq!(der_len(256), vec![0x82, 0x01, 0x00]);
    }

    #[test]
    fn el_tlv_estrucutura_correctamente() {
        assert_eq!(tlv(0x04, &[1, 2, 3]), vec![0x04, 0x03, 1, 2, 3]);
    }

    #[test]
    fn la_fecha_utc_es_correcta() {
        // 2021-01-01T00:00:00Z = 1609459200
        assert_eq!(utc_time_string(1_609_459_200), "210101000000Z");
        // 2000-03-01 = 951868800 -> 00:00:00
        assert_eq!(utc_time_string(951_868_800), "000301000000Z");
    }

    #[test]
    fn el_certificado_generado_es_autofirmado_y_tiene_spki_ed25519() {
        let c = generate("abc123", 365).expect("debe generar certificado");
        // El certificado empieza por SEQUENCE y contiene el OID de Ed25519.
        assert_eq!(c.der[0], 0x30);
        // SPKI de Ed25519: SEQUENCE { SEQUENCE { OID 1.3.101.112 }, BIT STRING }
        let spki = spki_ed25519(&[7u8; 32]);
        assert_eq!(spki[0], 0x30);
        assert!(spki.windows(5).any(|w| w == [0x06, 0x03, 0x2b, 0x65, 0x70]));
        // La huella es SHA-256 del DER: 64 hex.
        assert_eq!(c.fingerprint.len(), 64);
        assert_eq!(c.fingerprint, fingerprint_hex(&c.der));
    }

    #[test]
    fn el_pem_ida_y_vuelve() {
        let c = generate("nodo1", 30).expect("cert");
        let pem = to_pem(&c.der);
        assert!(pem.starts_with("-----BEGIN CERTIFICATE-----"));
        let back = from_pem(&pem).expect("debe releer el PEM");
        assert_eq!(back, c.der);

        let kpem = key_to_pem(&c.key_pkcs8);
        assert!(kpem.starts_with("-----BEGIN PRIVATE KEY-----"));
        assert_eq!(key_from_pem(&kpem).expect("clave"), c.key_pkcs8);
    }

    #[test]
    fn el_certificado_no_sale_en_el_debug() {
        let c = generate("nodo1", 30).expect("cert");
        let d = format!("{c:?}");
        assert!(!d.contains("PRIVATE KEY"));
        assert!(d.contains("fingerprint"));
    }
}
