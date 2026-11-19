//! Motor de firma electrónica en Rust puro para MegaDaemon (sin JVM).
//!
//! Este módulo implementa, sin Java y sin binarios externos en tiempo de
//! ejecución, las tres piezas que hacen falta para firmar electrónicamente
//! desde MegaDaemon:
//!
//! 1. **PKCS#11 (OpenSC)** — carga dinámica de `libpcsclite.so` /
//!    `opensc-pkcs11.so` mediante `dlopen`/`dlsym` (`libc`), con soporte de
//!    `C_Initialize`, `C_OpenSession`, `C_Login`, `C_FindObjects`,
//!    `C_Sign` (CKM_RSA_PKCS y CKM_ECDSA) y `C_CloseSession`.  La capa de
//!    hardware se abstrae tras el trait [`Pkcs11Token`]; para tests (y para
//!   获悉 keystores software) existe una implementación software que usa las
//!    primitivas nativas de este mismo fichero.
//!
//! 2. **CMS / CAdES** — construcción y verificación de `SignedData`
//!    (PKCS#7) en DER, con RSA-PKCS1 y ECDSA, SHA-256, atributos firmados
//!    (`contentType`, `messageDigest`, `signingTime`, `signingCertificateV2`,
//!    `commitmentTypeIndication`) y atributos no firmados.  La salida es
//!    verificable con `openssl cms -verify`.
//!
//! 3. **PAdES-B-B sobre PDF** — reserva de `/Contents`, escritura de
//!    `/ByteRange` y del diccionario de firma (`/SubFilter
//!    /ETSI.CAdES.detached`).  Si el PDF no trae campo de firma se genera uno
//!    (SAV: widget de apariencia de firma) con un escritor PDF propio.
//!
//! 4. **Protocolo `afirma://`** — servidor local que habla el esquema de
//!    rFirma / AutoFirme (`assertion`, `signature`, `cert`, `parameters`,
//!    `signatureValue`) en JSON, con puerto fijo `63117` (protocolo 3) y
//!    puertos dinámicos (protocolo 4).
//!
//! Limitaciones deliberadas (documentadas en el informe de entrega):
//! * No se implementa XAdES-T ni XAdES-LT: requieren TSA y OCSP/CRL.
//! * No se implementa el renderizado de la apariencia de firma (SAV visual):
//!   se genera el campo y el widget, sin flujo de apariencia.
//! * La capa TLS del servidor `afirma://` se inyecta; sin ella el socket es
//!   HTTP en claro sobre 127.0.0.1 (ver [`AfirmaServerConfig::tls`]).
//!
//! Nota de dependencias: este fichero **no** añade crates nuevos.  Las
//! primitivas criptográficas (DER, RSA, ECDSA, X.509) están implementadas
//! aquí porque `Cargo.toml` es intocable en esta tarea y ni `cms`, ni `der`,
//! ni `x509-cert`, ni `rsa`/`ecdsa` son dependencias directas del crate.
//! El hash usa `sha2`, que ya era dependencia del crate.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use std::collections::HashMap;
use std::io::{BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

// ===========================================================================
// 1. Tipos públicos (API serializable con serde_json)
// ===========================================================================

/// Objeto (certificado o clave) visible en un token PKCS#11.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenObject {
    /// Etiqueta legible del objeto (`CKA_LABEL`).
    pub label: String,
    /// Identificador del objeto (`CKA_ID`) en hexadecimal.
    pub id: String,
    /// Tipo de clave: `"RSA"`, `"EC"` o `"UNKNOWN"`.
    pub key_kind: String,
    /// Certificado DER asociado, si el token lo expone.
    pub cert_der: Option<Vec<u8>>,
}

/// Petición de firma recibida desde una sede electrónica (o desde el
/// protocolo `afirma://`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignatureRequest {
    /// Referencia del firmante: alias, `CKA_ID` o nombre de certificado.
    pub signer_ref: String,
    /// Certificado DER a utilizar, si la sede lo impone.
    pub certificate: Option<Vec<u8>>,
    /// Algoritmo de digest (`"SHA-256"`).
    pub digest_algorithm: String,
    /// Algoritmo de firma (`"RSA-PKCS1"` o `"ECDSA"`).
    pub signature_algorithm: String,
    /// Operación: `"sign"`, `"cosign"`, `"countersign"`, `"selectcert"`, ...
    pub operation: String,
    /// Parámetros libres del protocolo (formato, fichero, descripción...).
    pub properties: Value,
}

/// Respuesta serializada que consumen las sedes electrónicas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignedAssertion {
    /// Cuerpo de la aserción (metadatos del firmante y de la sesión).
    pub assertion: Value,
    /// Firma CMS/CAdES en Base64.
    pub signature: String,
    /// Certificado del firmante en Base64 (clave `cert` en el cable).
    #[serde(rename = "cert")]
    pub certificate: String,
    /// Parámetros de la petición original (clave `parameters`).
    #[serde(default)]
    pub parameters: Value,
    /// Firma cruda sobre el digest (clave `signatureValue`).
    #[serde(rename = "signatureValue", default)]
    pub signature_value: String,
}

/// Datos de identidad del firmante que viajan dentro de la aserción.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SignerIdentity {
    pub name: String,
    pub surname: String,
    pub email: String,
    pub nif: String,
    pub organization: String,
    pub serial_number: String,
}

/// Algoritmo de clave soportado por el motor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyAlgorithm {
    /// RSA con relleno PKCS#1 v1.5 (`CKM_RSA_PKCS`).
    RsaPkcs1,
    /// ECDSA sobre curva P-256 (`CKM_ECDSA`).
    Ecdsa,
}

impl KeyAlgorithm {
    /// Nombre canónico usado en el JSON del protocolo.
    pub fn as_str(&self) -> &'static str {
        match self {
            KeyAlgorithm::RsaPkcs1 => "RSA-PKCS1",
            KeyAlgorithm::Ecdsa => "ECDSA",
        }
    }

    /// Interpreta el nombre del algoritmo; `"RSA"` y `"SHA256withRSA"` también
    /// se aceptan por compatibilidad con Cliente @firma.
    pub fn parse(name: &str) -> Option<KeyAlgorithm> {
        let n = name.trim().to_ascii_uppercase();
        if n.contains("ECDSA") {
            Some(KeyAlgorithm::Ecdsa)
        } else if n.contains("RSA") {
            Some(KeyAlgorithm::RsaPkcs1)
        } else {
            None
        }
    }
}

/// Opciones de construcción del `SignedData` CMS/CAdES.
#[derive(Debug, Clone)]
pub struct CmsOptions {
    /// Firma desprendida: el contenido no viaja dentro del CMS (PAdES).
    pub detached: bool,
    /// Incluir el atributo ESS `signingCertificateV2`.
    pub include_signing_certificate_v2: bool,
    /// Incluir `commitmentTypeIndication` (exigido por PAdES).
    pub include_commitment_type: bool,
    /// Marca de tiempo fija en formato UTCTime (`YYMMDDHHMMSSZ`).  Si es
    /// `None` se usa la hora actual; los tests la fijan para ser reproducibles.
    pub signing_time: Option<String>,
}

impl Default for CmsOptions {
    fn default() -> Self {
        CmsOptions {
            detached: false,
            include_signing_certificate_v2: true,
            include_commitment_type: true,
            signing_time: None,
        }
    }
}

/// Proveedor de firma: abstrae "quién tiene la clave privada".
///
/// `sign_digest` recibe el digest SHA-256 y devuelve la firma cruda:
/// * RSA: firma PKCS#1 v1.5 tal cual (256 bytes para RSA-2048).
/// * ECDSA: `r || s` concatenados (64 bytes para P-256), tal y como los
///   devuelve `C_Sign` con `CKM_ECDSA`.
pub trait SignatureProvider: Send {
    /// Certificado DER del firmante.
    fn certificate_der(&self) -> Result<Vec<u8>, String>;
    /// Algoritmo de la clave.
    fn signature_algorithm(&self) -> KeyAlgorithm;
    /// Firma el digest (SHA-256) con la clave privada.
    fn sign_digest(&self, digest: &[u8]) -> Result<Vec<u8>, String>;
}

// ===========================================================================
// 2. Utilidades: Base64, hex, tiempo y aleatoriedad
// ===========================================================================

const B64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Codifica en Base64 estándar (con relleno `=`).
pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64_ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(B64_ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            B64_ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64_ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Decodifica Base64 estándar o URL-safe, tolerando espacios y saltos de línea.
pub fn base64_decode(text: &str) -> Result<Vec<u8>, String> {
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    for ch in text.chars() {
        let v = match ch {
            'A'..='Z' => ch as u32 - 'A' as u32,
            'a'..='z' => ch as u32 - 'a' as u32 + 26,
            '0'..='9' => ch as u32 - '0' as u32 + 52,
            '+' | '-' => 62,
            '/' | '_' => 63,
            '=' => break,
            '\r' | '\n' | ' ' | '\t' => continue,
            _ => return Err(format!("carácter Base64 inválido: {:?}", ch)),
        };
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xff) as u8);
        }
    }
    Ok(out)
}

/// Decodifica Base64 que puede venir con prefijo PEM; devuelve los bytes del
/// cuerpo DER si el texto es un PEM, o el Base64 plano tal cual.
pub fn base64_decode_pem_or_raw(text: &str) -> Result<Vec<u8>, String> {
    let trimmed = text.trim();
    if trimmed.starts_with("-----BEGIN") {
        let mut body = String::new();
        let mut inside = false;
        for line in trimmed.lines() {
            if line.starts_with("-----BEGIN") {
                inside = true;
                continue;
            }
            if line.starts_with("-----END") {
                break;
            }
            if inside {
                body.push_str(line.trim());
            }
        }
        base64_decode(&body)
    } else {
        base64_decode(trimmed)
    }
}

/// Codifica en hexadecimal en minúsculas.
pub fn hex_encode(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len() * 2);
    for b in data {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Decodifica hexadecimal admitiendo separadores (`:` y espacios).
pub fn hex_decode(text: &str) -> Result<Vec<u8>, String> {
    let clean: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':')
        .collect();
    if clean.len() % 2 != 0 {
        return Err("longitud hexadecimal impar".to_string());
    }
    let mut out = Vec::with_capacity(clean.len() / 2);
    let bytes = clean.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = (bytes[i] as char)
            .to_digit(16)
            .ok_or_else(|| format!("dígito hexadecimal inválido: {}", bytes[i] as char))?;
        let lo = (bytes[i + 1] as char)
            .to_digit(16)
            .ok_or_else(|| format!("dígito hexadecimal inválido: {}", bytes[i + 1] as char))?;
        out.push(((hi << 4) | lo) as u8);
    }
    Ok(out)
}

/// Segundos transcurridos desde la época Unix.
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Convierte días julianos a fecha civil (algoritmo de Hinnant).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as i64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Marca de tiempo DER `UTCTime` (`YYMMDDHHMMSSZ`).
pub fn format_utctime(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:02}{:02}{:02}{:02}{:02}{:02}Z",
        y % 100,
        m,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
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

/// Bytes aleatorios criptográficos (`/dev/urandom`).
pub fn random_bytes(n: usize) -> Result<Vec<u8>, String> {
    let mut f = std::fs::File::open("/dev/urandom")
        .map_err(|e| format!("no se puede abrir /dev/urandom: {e}"))?;
    let mut buf = vec![0u8; n];
    f.read_exact(&mut buf)
        .map_err(|e| format!("no se puede leer /dev/urandom: {e}"))?;
    Ok(buf)
}

/// Digest SHA-256.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    let out = h.finalize();
    let mut buf = [0u8; 32];
    buf.copy_from_slice(&out);
    buf
}

// ===========================================================================
// 3. ASN.1/DER: escritura y lectura
// ===========================================================================

/// Longitud DER en forma compacta.
fn der_len(len: usize) -> Vec<u8> {
    if len < 0x80 {
        vec![len as u8]
    } else {
        let mut tmp = Vec::new();
        let mut v = len;
        while v > 0 {
            tmp.push((v & 0xff) as u8);
            v >>= 8;
        }
        tmp.reverse();
        let mut out = vec![0x80 | tmp.len() as u8];
        out.extend_from_slice(&tmp);
        out
    }
}

/// TLV DER genérico.
fn der_tlv(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(content.len() + 6);
    out.push(tag);
    out.extend_from_slice(&der_len(content.len()));
    out.extend_from_slice(content);
    out
}

/// `SEQUENCE` con los elementos indicados.
fn der_seq(items: &[Vec<u8>]) -> Vec<u8> {
    let mut content = Vec::new();
    for i in items {
        content.extend_from_slice(i);
    }
    der_tlv(0x30, &content)
}

/// `SET` con los elementos indicados.
fn der_set(items: &[Vec<u8>]) -> Vec<u8> {
    let mut content = Vec::new();
    for i in items {
        content.extend_from_slice(i);
    }
    der_tlv(0x31, &content)
}

/// `INTEGER` positivo a partir de sus bytes big-endian (con signo).
fn der_int(bytes: &[u8]) -> Vec<u8> {
    let mut v: Vec<u8> = bytes.to_vec();
    while v.len() > 1 && v[0] == 0 && v[1] < 0x80 {
        v.remove(0);
    }
    if v.is_empty() {
        v.push(0);
    }
    if v[0] >= 0x80 {
        v.insert(0, 0);
    }
    der_tlv(0x02, &v)
}

/// `INTEGER` desde `u64`.
fn der_int_u64(value: u64) -> Vec<u8> {
    let mut v = value.to_be_bytes().to_vec();
    while v.len() > 1 && v[0] == 0 {
        v.remove(0);
    }
    der_int(&v)
}

/// `OID` a partir de sus arcos.
fn der_oid(arcs: &[u64]) -> Vec<u8> {
    let mut out = Vec::new();
    if arcs.len() < 2 {
        return der_tlv(0x06, &out);
    }
    out.push((arcs[0] * 40 + arcs[1]) as u8);
    for arc in &arcs[2..] {
        let mut stack = Vec::new();
        let mut v = *arc;
        loop {
            stack.push((v & 0x7f) as u8);
            v >>= 7;
            if v == 0 {
                break;
            }
        }
        stack.reverse();
        for (i, b) in stack.iter().enumerate() {
            if i + 1 < stack.len() {
                out.push(b | 0x80);
            } else {
                out.push(*b);
            }
        }
    }
    der_tlv(0x06, &out)
}

/// `NULL`.
fn der_null() -> Vec<u8> {
    der_tlv(0x05, &[])
}

/// `OCTET STRING`.
fn der_octet(bytes: &[u8]) -> Vec<u8> {
    der_tlv(0x04, bytes)
}

/// `BIT STRING` sin bits sobrantes.
#[allow(dead_code)]
fn der_bitstring(bytes: &[u8]) -> Vec<u8> {
    let mut content = Vec::with_capacity(bytes.len() + 1);
    content.push(0);
    content.extend_from_slice(bytes);
    der_tlv(0x03, &content)
}

/// `UTF8String`.
#[allow(dead_code)]
fn der_utf8(text: &str) -> Vec<u8> {
    der_tlv(0x0c, text.as_bytes())
}

/// `PrintableString`.
#[allow(dead_code)]
fn der_printable(text: &str) -> Vec<u8> {
    der_tlv(0x13, text.as_bytes())
}

/// `IA5String`.
#[allow(dead_code)]
fn der_ia5(text: &str) -> Vec<u8> {
    der_tlv(0x16, text.as_bytes())
}

/// `UTCTime`.
fn der_utctime(text: &str) -> Vec<u8> {
    der_tlv(0x17, text.as_bytes())
}

/// `GeneralizedTime`.
#[allow(dead_code)]
fn der_gentime(text: &str) -> Vec<u8> {
    der_tlv(0x18, text.as_bytes())
}

/// `BOOLEAN` (verdadero).
#[allow(dead_code)]
fn der_bool_true() -> Vec<u8> {
    der_tlv(0x01, &[0xff])
}

/// Etiqueta de contexto explícita `[n]` construida.
fn der_context(n: u8, content: &[u8]) -> Vec<u8> {
    der_tlv(0xa0 | (n & 0x1f), content)
}

/// Etiqueta de contexto explícita `[n]` primitiva.
#[allow(dead_code)]
fn der_context_primitive(n: u8, content: &[u8]) -> Vec<u8> {
    der_tlv(0x80 | (n & 0x1f), content)
}

/// `AlgorithmIdentifier` con parámetros `NULL` (RSA) o sin parámetros.
fn der_algid(oid: &[u64], null_params: bool) -> Vec<u8> {
    let mut items = vec![der_oid(oid)];
    if null_params {
        items.push(der_null());
    }
    der_seq(&items)
}

/// TLV DER leído.
#[derive(Debug, Clone, Copy)]
pub struct DerTlv<'a> {
    /// Etiqueta (byte inicial).
    pub tag: u8,
    /// Contenido (sin cabecera).
    pub content: &'a [u8],
    /// TLV completo (con cabecera).
    pub full: &'a [u8],
}

impl<'a> DerTlv<'a> {
    /// ¿Es una etiqueta de contexto con el número dado?
    pub fn is_context(&self, n: u8) -> bool {
        self.tag & 0xe0 == 0xa0 && (self.tag & 0x1f) == (n & 0x1f)
    }

    /// Decodifica el OID codificado.
    pub fn oid(&self) -> Result<Vec<u64>, String> {
        if self.tag != 0x06 {
            return Err(format!("se esperaba OID, se encontró etiqueta {:#04x}", self.tag));
        }
        if self.content.is_empty() {
            return Err("OID vacío".to_string());
        }
        let mut arcs = Vec::new();
        let first = self.content[0] as u64;
        arcs.push(first / 40);
        arcs.push(first % 40);
        let mut acc: u64 = 0;
        let mut started = false;
        for &b in &self.content[1..] {
            acc = (acc << 7) | ((b & 0x7f) as u64);
            started = true;
            if b & 0x80 == 0 {
                arcs.push(acc);
                acc = 0;
                started = false;
            }
        }
        if started {
            return Err("OID truncado".to_string());
        }
        Ok(arcs)
    }

    /// Bytes del entero (sin cabecera), sin el byte de signo.
    pub fn int_bytes(&self) -> Result<Vec<u8>, String> {
        if self.tag != 0x02 {
            return Err(format!("se esperaba INTEGER, se encontró {:#04x}", self.tag));
        }
        let mut v = self.content.to_vec();
        while v.len() > 1 && v[0] == 0 {
            v.remove(0);
        }
        Ok(v)
    }
}

/// Lector DER con cursor.
pub struct DerReader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> DerReader<'a> {
    /// Crea un lector sobre el buffer.
    pub fn new(buf: &'a [u8]) -> Self {
        DerReader { buf, pos: 0 }
    }

    /// ¿Se ha consumido todo el buffer?
    pub fn is_empty(&self) -> bool {
        self.pos >= self.buf.len()
    }

    /// Lee el siguiente TLV.
    pub fn read(&mut self) -> Result<DerTlv<'a>, String> {
        let start = self.pos;
        let tag = *self
            .buf
            .get(self.pos)
            .ok_or_else(|| "DER truncado: falta la etiqueta".to_string())?;
        self.pos += 1;
        let first = *self
            .buf
            .get(self.pos)
            .ok_or_else(|| "DER truncado: falta la longitud".to_string())?;
        self.pos += 1;
        let len = if first & 0x80 == 0 {
            first as usize
        } else {
            let n = (first & 0x7f) as usize;
            if n == 0 {
                return Err("longitud DER indefinida no soportada".to_string());
            }
            if n > 8 {
                return Err("longitud DER demasiado grande".to_string());
            }
            let mut l = 0usize;
            for _ in 0..n {
                let b = *self
                    .buf
                    .get(self.pos)
                    .ok_or_else(|| "DER truncado: longitud incompleta".to_string())?;
                self.pos += 1;
                l = (l << 8) | b as usize;
            }
            l
        };
        let end = self
            .pos
            .checked_add(len)
            .ok_or_else(|| "desbordamiento de longitud DER".to_string())?;
        if end > self.buf.len() {
            return Err("DER truncado: contenido incompleto".to_string());
        }
        let content = &self.buf[self.pos..end];
        self.pos = end;
        Ok(DerTlv {
            tag,
            content,
            full: &self.buf[start..end],
        })
    }

    /// Lee el siguiente TLV si su etiqueta coincide.
    pub fn read_if(&mut self, tag: u8) -> Result<Option<DerTlv<'a>>, String> {
        if self.buf.get(self.pos).copied() == Some(tag) {
            Ok(Some(self.read()?))
        } else {
            Ok(None)
        }
    }

    /// Lee todos los TLV hijos de una etiqueta construida.
    pub fn read_children(&mut self, parent: &DerTlv<'a>) -> Result<Vec<DerTlv<'a>>, String> {
        let mut r = DerReader::new(parent.content);
        let mut out = Vec::new();
        while !r.is_empty() {
            out.push(r.read()?);
        }
        Ok(out)
    }
}

// ===========================================================================
// 4. Aritmética de enteros grandes (BigUint)
// ===========================================================================

use std::cmp::Ordering;

/// Entero grande sin signo, limbs `u64` en orden little-endian.
///
/// Tipo opaco: sólo se exponen los tipos públicos que lo contienen
/// (`PublicKey`, `EcdsaSignature`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BigUint {
    limbs: Vec<u64>,
}

impl BigUint {
    pub fn zero() -> Self {
        BigUint { limbs: Vec::new() }
    }

    pub fn from_u64(v: u64) -> Self {
        let mut n = BigUint { limbs: vec![v] };
        n.trim();
        n
    }

    pub fn from_bytes_be(bytes: &[u8]) -> Self {
        let mut limbs = Vec::with_capacity(bytes.len() / 8 + 1);
        let mut acc: u64 = 0;
        let mut shift = 0;
        for &b in bytes.iter().rev() {
            acc |= (b as u64) << shift;
            shift += 8;
            if shift == 64 {
                limbs.push(acc);
                acc = 0;
                shift = 0;
            }
        }
        if shift > 0 {
            limbs.push(acc);
        }
        let mut n = BigUint { limbs };
        n.trim();
        n
    }

    /// Bytes big-endian sin ceros a la izquierda (vacío si es cero).
    pub fn to_bytes_be(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.limbs.len() * 8);
        for limb in self.limbs.iter().rev() {
            out.extend_from_slice(&limb.to_be_bytes());
        }
        let first_nonzero = out.iter().position(|&b| b != 0).unwrap_or(out.len());
        out.drain(..first_nonzero);
        out
    }

    /// Bytes big-endian rellenados con ceros a la izquierda hasta `len`.
    pub fn to_bytes_be_padded(&self, len: usize) -> Vec<u8> {
        let raw = self.to_bytes_be();
        if raw.len() >= len {
            raw[raw.len() - len..].to_vec()
        } else {
            let mut out = vec![0u8; len - raw.len()];
            out.extend_from_slice(&raw);
            out
        }
    }

    fn trim(&mut self) {
        while self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
    }

    pub fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    pub fn is_one(&self) -> bool {
        self.limbs.len() == 1 && self.limbs[0] == 1
    }

    pub fn bit_len(&self) -> usize {
        match self.limbs.last() {
            None => 0,
            Some(top) => (self.limbs.len() - 1) * 64 + (64 - top.leading_zeros() as usize),
        }
    }

    pub fn bit(&self, i: usize) -> bool {
        let limb = i / 64;
        if limb >= self.limbs.len() {
            return false;
        }
        (self.limbs[limb] >> (i % 64)) & 1 == 1
    }

    pub fn cmp_big(&self, other: &BigUint) -> Ordering {
        if self.limbs.len() != other.limbs.len() {
            return self.limbs.len().cmp(&other.limbs.len());
        }
        for i in (0..self.limbs.len()).rev() {
            if self.limbs[i] != other.limbs[i] {
                return self.limbs[i].cmp(&other.limbs[i]);
            }
        }
        Ordering::Equal
    }

    pub fn add(&self, other: &BigUint) -> BigUint {
        let n = self.limbs.len().max(other.limbs.len());
        let mut out = Vec::with_capacity(n + 1);
        let mut carry: u128 = 0;
        for i in 0..n {
            let a = *self.limbs.get(i).unwrap_or(&0) as u128;
            let b = *other.limbs.get(i).unwrap_or(&0) as u128;
            let s = a + b + carry;
            out.push(s as u64);
            carry = s >> 64;
        }
        if carry > 0 {
            out.push(carry as u64);
        }
        let mut r = BigUint { limbs: out };
        r.trim();
        r
    }

    /// Resta `self - other`; exige `self >= other`.
    pub fn sub(&self, other: &BigUint) -> BigUint {
        debug_assert!(self.cmp_big(other) != Ordering::Less);
        let mut out = Vec::with_capacity(self.limbs.len());
        let mut borrow: i128 = 0;
        for i in 0..self.limbs.len() {
            let a = self.limbs[i] as i128;
            let b = *other.limbs.get(i).unwrap_or(&0) as i128;
            let mut d = a - b - borrow;
            if d < 0 {
                d += 1i128 << 64;
                borrow = 1;
            } else {
                borrow = 0;
            }
            out.push(d as u64);
        }
        let mut r = BigUint { limbs: out };
        r.trim();
        r
    }

    pub fn mul(&self, other: &BigUint) -> BigUint {
        if self.is_zero() || other.is_zero() {
            return BigUint::zero();
        }
        let mut out = vec![0u64; self.limbs.len() + other.limbs.len()];
        for (i, &a) in self.limbs.iter().enumerate() {
            let mut carry: u128 = 0;
            for (j, &b) in other.limbs.iter().enumerate() {
                let cur = out[i + j] as u128 + (a as u128) * (b as u128) + carry;
                out[i + j] = cur as u64;
                carry = cur >> 64;
            }
            let mut k = i + other.limbs.len();
            while carry > 0 {
                let cur = out[k] as u128 + carry;
                out[k] = cur as u64;
                carry = cur >> 64;
                k += 1;
            }
        }
        let mut r = BigUint { limbs: out };
        r.trim();
        r
    }

    /// División entera: devuelve (cociente, resto).  El divisor debe ser > 0.
    pub fn divrem(&self, divisor: &BigUint) -> (BigUint, BigUint) {
        assert!(!divisor.is_zero(), "división por cero");
        if self.cmp_big(divisor) == Ordering::Less {
            return (BigUint::zero(), self.clone());
        }
        let bits = self.bit_len();
        let mut q = BigUint { limbs: vec![0u64; (bits + 63) / 64] };
        let mut r = BigUint::zero();
        for i in (0..bits).rev() {
            r.shl1_assign();
            if self.bit(i) {
                r.limbs[0] |= 1;
            }
            if r.cmp_big(divisor) != Ordering::Less {
                r = r.sub(divisor);
                q.limbs[i / 64] |= 1u64 << (i % 64);
            }
        }
        q.trim();
        r.trim();
        (q, r)
    }

    pub fn rem(&self, m: &BigUint) -> BigUint {
        self.divrem(m).1
    }

    /// Exponiación modular (`self^exp mod m`).
    pub fn modpow(&self, exp: &BigUint, m: &BigUint) -> BigUint {
        if m.is_one() {
            return BigUint::zero();
        }
        let mut result = BigUint::from_u64(1);
        let base = self.rem(m);
        for i in (0..exp.bit_len()).rev() {
            result = result.mul(&result).rem(m);
            if exp.bit(i) {
                result = result.mul(&base).rem(m);
            }
        }
        result
    }

    /// Inverso modular (algoritmo de Euclides binario).  `m` debe ser impar.
    pub fn modinv(&self, m: &BigUint) -> Option<BigUint> {
        if self.is_zero() {
            return None;
        }
        let mut u = self.rem(m);
        let mut v = m.clone();
        let mut x1 = BigUint::from_u64(1);
        let mut x2 = BigUint::zero();
        while !u.is_one() && !v.is_one() {
            while !u.is_zero() && !u.bit(0) {
                u.shr1_assign();
                if x1.bit(0) {
                    x1 = x1.add(m);
                }
                x1.shr1_assign();
            }
            while !v.is_zero() && !v.bit(0) {
                v.shr1_assign();
                if x2.bit(0) {
                    x2 = x2.add(m);
                }
                x2.shr1_assign();
            }
            if u.cmp_big(&v) != Ordering::Less {
                u = u.sub(&v);
                x1 = sub_mod(&x1, &x2, m);
            } else {
                v = v.sub(&u);
                x2 = sub_mod(&x2, &x1, m);
            }
        }
        if u.is_one() {
            Some(x1.rem(m))
        } else {
            Some(x2.rem(m))
        }
    }

    pub fn shl1_assign(&mut self) {
        if self.limbs.is_empty() {
            self.limbs.push(0);
        }
        let mut carry = 0u64;
        for limb in self.limbs.iter_mut() {
            let new_carry = *limb >> 63;
            *limb = (*limb << 1) | carry;
            carry = new_carry;
        }
        if carry > 0 {
            self.limbs.push(carry);
        }
    }

    /// Desplazamiento a la derecha de un bit (entero, trunca).
    pub fn shr1_assign(&mut self) {
        let mut carry = 0u64;
        for i in (0..self.limbs.len()).rev() {
            let new_carry = self.limbs[i] & 1;
            self.limbs[i] = (self.limbs[i] >> 1) | (carry << 63);
            carry = new_carry;
        }
        self.trim();
    }
}

/// Resta modular (`a - b mod m`), exige `0 <= a,b < m`.
fn sub_mod(a: &BigUint, b: &BigUint, m: &BigUint) -> BigUint {
    if a.cmp_big(b) == Ordering::Less {
        a.add(m).sub(b)
    } else {
        a.sub(b)
    }
}

/// Suma modular (`a + b mod m`).
fn add_mod(a: &BigUint, b: &BigUint, m: &BigUint) -> BigUint {
    let s = a.add(b);
    if s.cmp_big(m) != Ordering::Less {
        s.sub(m)
    } else {
        s
    }
}

/// Multiplicación modular.
fn mul_mod(a: &BigUint, b: &BigUint, m: &BigUint) -> BigUint {
    a.mul(b).rem(m)
}

// ===========================================================================
// 5. Claves privadas: RSA y ECDSA (P-256)
// ===========================================================================

/// DigestInfo DER de SHA-256 (RFC 8017, DigestInfo).
const SHA256_DIGEST_INFO: &[u8] = &[
    0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01, 0x05,
    0x00, 0x04, 0x20,
];

/// Clave privada RSA con parámetros CRT.
#[derive(Clone, Debug)]
pub struct RsaPrivateKey {
    n: BigUint,
    /// Exponente público (informativo).
    #[allow(dead_code)]
    e: BigUint,
    /// Exponente privado (informativo; la firma usa CRT).
    #[allow(dead_code)]
    d: BigUint,
    p: BigUint,
    q: BigUint,
    dp: BigUint,
    dq: BigUint,
    qinv: BigUint,
    size: usize,
}

impl RsaPrivateKey {
    /// Construye la clave desde sus parámetros PKCS#1.
    pub fn from_pkcs1_der(der: &[u8]) -> Result<RsaPrivateKey, String> {
        let mut outer = DerReader::new(der);
        let seq = outer
            .read()
            .map_err(|e| format!("RSAPrivateKey: {}", e))?;
        if seq.tag != 0x30 {
            return Err("RSAPrivateKey: se esperaba SEQUENCE".to_string());
        }
        let items = outer
            .read_children(&seq)
            .map_err(|e| format!("RSAPrivateKey (campos): {}", e))?;
        if items.len() < 9 {
            return Err(format!(
                "RSAPrivateKey: se esperaban 9 elementos, hay {}",
                items.len()
            ));
        }
        let num = |t: &DerTlv| -> Result<BigUint, String> {
            Ok(BigUint::from_bytes_be(&t.int_bytes()?))
        };
        let n = num(&items[1])?;
        let e = num(&items[2])?;
        let d = num(&items[3])?;
        let p = num(&items[4])?;
        let q = num(&items[5])?;
        let dp = num(&items[6])?;
        let dq = num(&items[7])?;
        let qinv = num(&items[8])?;
        let size = (n.bit_len() + 7) / 8;
        Ok(RsaPrivateKey {
            n,
            e,
            d,
            p,
            q,
            dp,
            dq,
            qinv,
            size,
        })
    }

    /// Tamaño del módulo en bytes.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Módulo público.
    pub fn modulus(&self) -> Vec<u8> {
        self.n.to_bytes_be_padded(self.size)
    }

    /// Firma PKCS#1 v1.5 sobre un digest SHA-256.
    pub fn sign_digest(&self, digest: &[u8]) -> Result<Vec<u8>, String> {
        let em = pkcs1_v15_encode(digest, self.size)?;
        let m = BigUint::from_bytes_be(&em);
        // CRT: m1 = m^p-1 mod p ; h1 = m1^dp mod p ; idem con q.
        let m1 = m.rem(&self.p);
        let m2 = m.rem(&self.q);
        let h1 = m1.modpow(&self.dp, &self.p);
        let h2 = m2.modpow(&self.dq, &self.q);
        let diff = sub_mod(&h1, &h2, &self.p);
        let h = mul_mod(&diff, &self.qinv, &self.p);
        let s = h2.add(&self.q.mul(&h));
        Ok(s.to_bytes_be_padded(self.size))
    }
}

/// Codifica EMSA-PKCS1-v1_5 para SHA-256 sobre un digest de `k` bytes.
fn pkcs1_v15_encode(digest: &[u8], k: usize) -> Result<Vec<u8>, String> {
    let t_len = SHA256_DIGEST_INFO.len() + digest.len();
    if k < t_len + 11 {
        return Err(format!(
            "clave RSA demasiado corta: {k} bytes para {t_len} + 11",
        ));
    }
    let mut em = Vec::with_capacity(k);
    em.push(0x00);
    em.push(0x01);
    em.extend(std::iter::repeat(0xffu8).take(k - t_len - 3));
    em.push(0x00);
    em.extend_from_slice(SHA256_DIGEST_INFO);
    em.extend_from_slice(digest);
    Ok(em)
}

/// Verifica una firma PKCS#1 v1.5 con una clave pública RSA (`n`, `e`).
fn rsa_pkcs1_v15_verify(n: &BigUint, e: &BigUint, digest: &[u8], sig: &[u8]) -> bool {
    let size = (n.bit_len() + 7) / 8;
    if sig.len() != size {
        return false;
    }
    let s = BigUint::from_bytes_be(sig);
    if s.cmp_big(n) != Ordering::Less {
        return false;
    }
    let m = s.modpow(e, n);
    let em = m.to_bytes_be_padded(size);
    match pkcs1_v15_encode(digest, size) {
        Ok(expected) => constant_time_eq(&em, &expected),
        Err(_) => false,
    }
}

/// Comparación en tiempo constante.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

// ===========================================================================
// 6. ECDSA sobre P-256 (NIST P-256 / prime256v1)
// ===========================================================================

use std::sync::OnceLock;

/// Parámetros de la curva P-256.
struct P256Params {
    p: BigUint,
    b: BigUint,
    gx: BigUint,
    gy: BigUint,
    n: BigUint,
}

/// Parámetros P-256 (calculados una sola vez).
fn p256() -> &'static P256Params {
    static PARAMS: OnceLock<P256Params> = OnceLock::new();
    PARAMS.get_or_init(|| {
        let p = BigUint::from_bytes_be(
            &hex_decode("ffffffff00000001000000000000000000000000ffffffffffffffffffffffff")
                .expect("constante P-256"),
        );
        let b = BigUint::from_bytes_be(
            &hex_decode(
                "5ac635d8aa3a93e7b3ebbd55769886bc651d06b0cc53b0f63bce3c3e27d2604b",
            )
            .expect("constante P-256"),
        );
        let gx = BigUint::from_bytes_be(
            &hex_decode(
                "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296",
            )
            .expect("constante P-256"),
        );
        let gy = BigUint::from_bytes_be(
            &hex_decode(
                "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5",
            )
            .expect("constante P-256"),
        );
        let n = BigUint::from_bytes_be(
            &hex_decode("ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551")
                .expect("constante P-256"),
        );
        P256Params { p, b, gx, gy, n }
    })
}

/// Punto en coordenadas jacobianas (`z == 0` es el punto infinito).
#[derive(Clone, Debug)]
struct JPoint {
    x: BigUint,
    y: BigUint,
    z: BigUint,
}

impl JPoint {
    fn infinity() -> Self {
        JPoint {
            x: BigUint::from_u64(1),
            y: BigUint::from_u64(1),
            z: BigUint::zero(),
        }
    }

    fn is_infinity(&self) -> bool {
        self.z.is_zero()
    }

    /// Punto afín en bytes no comprimidos (`0x04 || X || Y`).
    fn to_uncompressed(&self, p: &BigUint) -> Option<Vec<u8>> {
        if self.is_infinity() {
            return None;
        }
        let zinv = self.z.modinv(p)?;
        let z2 = mul_mod(&zinv, &zinv, p);
        let z3 = mul_mod(&z2, &zinv, p);
        let x = mul_mod(&self.x, &z2, p);
        let y = mul_mod(&self.y, &z3, p);
        let mut out = Vec::with_capacity(65);
        out.push(0x04);
        out.extend_from_slice(&x.to_bytes_be_padded(32));
        out.extend_from_slice(&y.to_bytes_be_padded(32));
        Some(out)
    }
}

/// Doble de un punto jacobiano (fórmula `dbl-2001-b`, válida para a = -3).
fn p256_double(pt: &JPoint, params: &P256Params) -> JPoint {
    let p = &params.p;
    if pt.is_infinity() || pt.y.is_zero() {
        return JPoint::infinity();
    }
    let delta = mul_mod(&pt.z, &pt.z, p);
    let gamma = mul_mod(&pt.y, &pt.y, p);
    let beta = mul_mod(&pt.x, &gamma, p);
    let t1 = sub_mod(&pt.x, &delta, p);
    let t2 = add_mod(&pt.x, &delta, p);
    let alpha = mul_mod(&BigUint::from_u64(3), &mul_mod(&t1, &t2, p), p);
    let x3 = sub_mod(
        &mul_mod(&alpha, &alpha, p),
        &mul_mod(&BigUint::from_u64(8), &beta, p),
        p,
    );
    let z3 = sub_mod(
        &sub_mod(
            &mul_mod(&add_mod(&pt.y, &pt.z, p), &add_mod(&pt.y, &pt.z, p), p),
            &gamma,
            p,
        ),
        &delta,
        p,
    );
    let y3 = sub_mod(
        &mul_mod(
            &alpha,
            &sub_mod(
                &mul_mod(&BigUint::from_u64(4), &beta, p),
                &x3,
                p,
            ),
            p,
        ),
        &mul_mod(
            &BigUint::from_u64(8),
            &mul_mod(&gamma, &gamma, p),
            p,
        ),
        p,
    );
    JPoint { x: x3, y: y3, z: z3 }
}

/// Suma de dos puntos jacobianos (fórmula `add-2007-bl`).
fn p256_add(a: &JPoint, b: &JPoint, params: &P256Params) -> JPoint {
    let p = &params.p;
    if a.is_infinity() {
        return b.clone();
    }
    if b.is_infinity() {
        return a.clone();
    }
    let z1z1 = mul_mod(&a.z, &a.z, p);
    let z2z2 = mul_mod(&b.z, &b.z, p);
    let u1 = mul_mod(&a.x, &z2z2, p);
    let u2 = mul_mod(&b.x, &z1z1, p);
    let s1 = mul_mod(&a.y, &mul_mod(&b.z, &z2z2, p), p);
    let s2 = mul_mod(&b.y, &mul_mod(&a.z, &z1z1, p), p);
    if u1 == u2 {
        if s1 == s2 {
            return p256_double(a, params);
        }
        return JPoint::infinity();
    }
    let h = sub_mod(&u2, &u1, p);
    let i = mul_mod(&BigUint::from_u64(2), &h, p);
    let i = mul_mod(&i, &i, p);
    let j = mul_mod(&h, &i, p);
    let r = mul_mod(&BigUint::from_u64(2), &sub_mod(&s2, &s1, p), p);
    let v = mul_mod(&u1, &i, p);
    let x3 = sub_mod(
        &sub_mod(&mul_mod(&r, &r, p), &j, p),
        &mul_mod(&BigUint::from_u64(2), &v, p),
        p,
    );
    let y3 = sub_mod(
        &mul_mod(&r, &sub_mod(&v, &x3, p), p),
        &mul_mod(&BigUint::from_u64(2), &mul_mod(&s1, &j, p), p),
        p,
    );
    let zsum = add_mod(&a.z, &b.z, p);
    let z3 = mul_mod(
        &sub_mod(
            &sub_mod(&mul_mod(&zsum, &zsum, p), &z1z1, p),
            &z2z2,
            p,
        ),
        &h,
        p,
    );
    JPoint { x: x3, y: y3, z: z3 }
}

/// Multiplicación escalar (double-and-add, MSB primero).
fn p256_mul(k: &BigUint, pt: &JPoint, params: &P256Params) -> JPoint {
    let mut acc = JPoint::infinity();
    let bits = k.bit_len();
    for i in (0..bits).rev() {
        acc = p256_double(&acc, params);
        if k.bit(i) {
            acc = p256_add(&acc, pt, params);
        }
    }
    acc
}

/// Generador de la curva.
fn p256_generator(params: &P256Params) -> JPoint {
    JPoint {
        x: params.gx.clone(),
        y: params.gy.clone(),
        z: BigUint::from_u64(1),
    }
}

/// Firma ECDSA (devuelve `r` y `s`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EcdsaSignature {
    pub r: BigUint,
    pub s: BigUint,
}

impl EcdsaSignature {
    /// Codificación DER `SEQUENCE { INTEGER r, INTEGER s }`.
    pub fn to_der(&self) -> Vec<u8> {
        der_seq(&[der_int(&self.r.to_bytes_be()), der_int(&self.s.to_bytes_be())])
    }

    /// Decodifica una firma DER.
    pub fn from_der(der: &[u8]) -> Result<EcdsaSignature, String> {
        let mut outer = DerReader::new(der);
        let seq = outer.read()?;
        if seq.tag != 0x30 {
            return Err("firma ECDSA: se esperaba SEQUENCE".to_string());
        }
        let items = outer.read_children(&seq)?;
        if items.len() != 2 {
            return Err(format!(
                "firma ECDSA: se esperaban 2 elementos, hay {}",
                items.len()
            ));
        }
        Ok(EcdsaSignature {
            r: BigUint::from_bytes_be(&items[0].int_bytes()?),
            s: BigUint::from_bytes_be(&items[1].int_bytes()?),
        })
    }
}

/// Clave privada ECDSA P-256.
#[derive(Clone, Debug)]
pub struct EcdsaPrivateKey {
    d: BigUint,
    public: JPoint,
}

impl EcdsaPrivateKey {
    /// Crea la clave a partir del escalar privado (32 bytes).
    pub fn from_scalar(bytes: &[u8]) -> Result<EcdsaPrivateKey, String> {
        if bytes.len() > 32 {
            return Err("escalar ECDSA demasiado largo".to_string());
        }
        let params = p256();
        let d = BigUint::from_bytes_be(bytes);
        if d.is_zero() || d.cmp_big(&params.n) != Ordering::Less {
            return Err("escalar ECDSA fuera de rango".to_string());
        }
        let public = p256_mul(&d, &p256_generator(params), params);
        Ok(EcdsaPrivateKey { d, public })
    }

    /// Punto público no comprimido.
    pub fn public_point(&self) -> Vec<u8> {
        self.public
            .to_uncompressed(&p256().p)
            .unwrap_or_else(|| vec![0x04; 65])
    }

    /// Genera el par (r, s) de una firma sobre un digest SHA-256.
    pub fn sign_pair(&self, digest: &[u8]) -> Result<EcdsaSignature, String> {
        let params = p256();
        let z = BigUint::from_bytes_be(digest);
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            if attempt > 64 {
                return Err("no se pudo generar un nonce ECDSA válido".to_string());
            }
            let mut kbytes = random_bytes(32)?;
            // Recorta por encima de n paraSesgar la distribución.
            if BigUint::from_bytes_be(&kbytes).cmp_big(&params.n) != Ordering::Less {
                kbytes[0] &= 0x7f;
            }
            let k = BigUint::from_bytes_be(&kbytes);
            if k.is_zero() || k.cmp_big(&params.n) != Ordering::Less {
                continue;
            }
            let point = p256_mul(&k, &p256_generator(params), params);
            let affine = match point.to_uncompressed(&params.p) {
                Some(a) => a,
                None => continue,
            };
            let r = BigUint::from_bytes_be(&affine[1..33]).rem(&params.n);
            if r.is_zero() {
                continue;
            }
            let kinv = match k.modinv(&params.n) {
                Some(v) => v,
                None => continue,
            };
            let s = mul_mod(
                &kinv,
                &add_mod(&z.rem(&params.n), &mul_mod(&r, &self.d, &params.n), &params.n),
                &params.n,
            );
            if s.is_zero() {
                continue;
            }
            return Ok(EcdsaSignature { r, s });
        }
    }
}

impl EcdsaPrivateKey {
    /// Firma un digest SHA-256 y devuelve el DER `SEQUENCE {r, s}`.
    pub fn sign_digest(&self, digest: &[u8]) -> Result<Vec<u8>, String> {
        Ok(self.sign_pair(digest)?.to_der())
    }

    /// Firma un digest SHA-256 y devuelve `r || s` (formato crudo de
    /// `CKM_ECDSA` en PKCS#11).
    pub fn sign_raw(&self, digest: &[u8]) -> Result<Vec<u8>, String> {
        let sig = self.sign_pair(digest)?;
        let mut out = sig.r.to_bytes_be_padded(32);
        out.extend_from_slice(&sig.s.to_bytes_be_padded(32));
        Ok(out)
    }
}

/// Verifica una firma ECDSA DER con un punto público no comprimido.
fn ecdsa_verify_digest(
    public_point: &[u8],
    digest: &[u8],
    signature_der: &[u8],
) -> Result<bool, String> {
    let params = p256();
    let sig = EcdsaSignature::from_der(signature_der)?;
    if sig.r.is_zero()
        || sig.r.cmp_big(&params.n) != Ordering::Less
        || sig.s.is_zero()
        || sig.s.cmp_big(&params.n) != Ordering::Less
    {
        return Ok(false);
    }
    if public_point.len() != 65 || public_point[0] != 0x04 {
        return Err("punto público ECDSA inválido".to_string());
    }
    let qx = BigUint::from_bytes_be(&public_point[1..33]);
    let qy = BigUint::from_bytes_be(&public_point[33..65]);
    // Comprueba que el punto está en la curva.
    let lhs = mul_mod(&qy, &qy, &params.p);
    let x3 = mul_mod(&mul_mod(&qx, &qx, &params.p), &qx, &params.p);
    let rhs = add_mod(&sub_mod(&x3, &mul_mod(&BigUint::from_u64(3), &qx, &params.p), &params.p), &params.b, &params.p);
    if lhs != rhs {
        return Err("el punto público ECDSA no pertenece a la curva P-256".to_string());
    }
    let z = BigUint::from_bytes_be(digest).rem(&params.n);
    let w = match sig.s.modinv(&params.n) {
        Some(v) => v,
        None => return Ok(false),
    };
    let u1 = mul_mod(&z, &w, &params.n);
    let u2 = mul_mod(&sig.r, &w, &params.n);
    let point = p256_add(
        &p256_mul(&u1, &p256_generator(params), params),
        &p256_mul(&u2, &JPoint { x: qx, y: qy, z: BigUint::from_u64(1) }, params),
        params,
    );
    if point.is_infinity() {
        return Ok(false);
    }
    let affine = match point.to_uncompressed(&params.p) {
        Some(a) => a,
        None => return Ok(false),
    };
    let v = BigUint::from_bytes_be(&affine[1..33]).rem(&params.n);
    Ok(v == sig.r)
}

// ===========================================================================
// 7. PEM y claves privadas software
// ===========================================================================

/// Envuelve un DER en PEM con la etiqueta indicada.
pub fn pem_encode(label: &str, der: &[u8]) -> String {
    let b64 = base64_encode(der);
    let mut out = format!("-----BEGIN {label}-----\n");
    for chunk in b64.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(chunk).unwrap_or_default());
        out.push('\n');
    }
    out.push_str(&format!("-----END {label}-----\n"));
    out
}

/// Decodifica un PEM: devuelve (etiqueta, DER).
pub fn pem_decode(text: &str) -> Result<(String, Vec<u8>), String> {
    let mut label = String::new();
    let mut body = String::new();
    let mut inside = false;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("-----BEGIN ") {
            label = rest.trim_end_matches('-').trim().to_string();
            inside = true;
            continue;
        }
        if line.starts_with("-----END") {
            break;
        }
        if inside {
            body.push_str(line);
        }
    }
    if label.is_empty() {
        return Err("no se encontró ninguna cabecera PEM".to_string());
    }
    Ok((label, base64_decode(&body)?))
}

/// Clave privada software (RSA o ECDSA) con su certificado.
#[derive(Clone, Debug)]
pub enum PrivateKey {
    Rsa(RsaPrivateKey),
    Ecdsa(EcdsaPrivateKey),
}

impl PrivateKey {
    /// Interpreta un DER: PKCS#8, PKCS#1 (RSA) o SEC1 (EC).
    pub fn from_der(der: &[u8]) -> Result<PrivateKey, String> {
        let mut outer = DerReader::new(der);
        let seq = outer
            .read()
            .map_err(|e| format!("clave privada (PrivateKeyInfo): {}", e))?;
        if seq.tag != 0x30 {
            return Err("clave privada: se esperaba SEQUENCE".to_string());
        }
        let items = outer
            .read_children(&seq)
            .map_err(|e| format!("clave privada (campos): {}", e))?;
        if items.len() >= 3 && items[0].tag == 0x02 && items[1].tag == 0x30 && items[2].tag == 0x04 {
            // PKCS#8 PrivateKeyInfo: el OID está dentro del
            // AlgorithmIdentifier.
            // `items[1]` es el AlgorithmIdentifier: sus hijos son el OID y
            // los parámetros (la curva en el caso EC).
            let alg_items = outer
                .read_children(&items[1])
                .map_err(|e| format!("clave privada (OID del algoritmo): {}", e))?;
            let alg_oid = alg_items
                .first()
                .ok_or_else(|| "PKCS#8: AlgorithmIdentifier vacío".to_string())?
                .oid()?;
            if alg_oid == OID_EC_PUBLIC_KEY {
                let curve = if alg_items.len() > 1 {
                    alg_items[1].oid().unwrap_or_default()
                } else {
                    Vec::new()
                };
                if curve != OID_PRIME256V1 {
                    return Err(format!(
                        "curva ECDSA no soportada (se espera P-256): {}",
                        curve
                            .iter()
                            .map(|a| a.to_string())
                            .collect::<Vec<_>>()
                            .join(".")
                    ));
                }
                // ECPrivateKey (SEC1) dentro del OCTET STRING.
                let mut inner = DerReader::new(items[2].content);
                let ec_seq = inner
                    .read()
                    .map_err(|e| format!("clave privada (ECPrivateKey): {}", e))?;
                let ec_items = inner
                    .read_children(&ec_seq)
                    .map_err(|e| format!("clave privada (ECPrivateKey): {}", e))?;
                if ec_items.len() < 2 || ec_items[1].tag != 0x04 {
                    return Err("ECPrivateKey mal formada".to_string());
                }
                return Ok(PrivateKey::Ecdsa(EcdsaPrivateKey::from_scalar(
                    ec_items[1].content,
                )?));
            }
            // RSA: el OCTET STRING contiene el RSAPrivateKey PKCS#1.
            return Ok(PrivateKey::Rsa(RsaPrivateKey::from_pkcs1_der(
                items[2].content,
            )?));
        }
        if items.len() >= 9 && items.iter().all(|i| i.tag == 0x02) {
            // RSAPrivateKey (PKCS#1) directo.
            return Ok(PrivateKey::Rsa(RsaPrivateKey::from_pkcs1_der(der)?));
        }
        Err("formato de clave privada no reconocido".to_string())
    }

    /// Interpreta un PEM (`PRIVATE KEY`, `RSA PRIVATE KEY`, `EC PRIVATE KEY`).
    pub fn from_pem(text: &str) -> Result<PrivateKey, String> {
        let (_label, der) = pem_decode(text)?;
        PrivateKey::from_der(&der)
    }

    /// Algoritmo de la clave.
    pub fn algorithm(&self) -> KeyAlgorithm {
        match self {
            PrivateKey::Rsa(_) => KeyAlgorithm::RsaPkcs1,
            PrivateKey::Ecdsa(_) => KeyAlgorithm::Ecdsa,
        }
    }

    /// Firma el digest en formato DER canónico (el que espera el CMS).
    pub fn sign_digest(&self, digest: &[u8]) -> Result<Vec<u8>, String> {
        match self {
            PrivateKey::Rsa(k) => k.sign_digest(digest),
            PrivateKey::Ecdsa(k) => k.sign_digest(digest),
        }
    }

    /// Firma el digest en el formato crudo que devuelve `C_Sign`
    /// (RSA: PKCS#1 v1.5; ECDSA: `r || s`).
    pub fn sign_digest_raw(&self, digest: &[u8]) -> Result<Vec<u8>, String> {
        match self {
            PrivateKey::Rsa(k) => k.sign_digest(digest),
            PrivateKey::Ecdsa(k) => k.sign_raw(digest),
        }
    }
}

// ===========================================================================
// 8. X.509: análisis mínimo de certificados
// ===========================================================================

/// OID de `rsaEncryption`.
const OID_RSA_ENCRYPTION: &[u64] = &[1, 2, 840, 113549, 1, 1, 1];
/// OID de `id-ecPublicKey`.
const OID_EC_PUBLIC_KEY: &[u64] = &[1, 2, 840, 10045, 2, 1];
/// OID de la curva `prime256v1` (P-256).
const OID_PRIME256V1: &[u64] = &[1, 2, 840, 10045, 3, 1, 7];

/// Clave pública extraída de un certificado.
#[derive(Clone, Debug)]
pub enum PublicKey {
    /// RSA con módulo y exponente.
    Rsa { n: BigUint, e: BigUint },
    /// ECDSA con el punto en formato no comprimido (`0x04 || X || Y`).
    Ec { point: Vec<u8> },
}

/// Certificado X.509 analizado mínimamente.
#[derive(Clone, Debug)]
pub struct X509Certificate {
    /// DER completo del certificado.
    pub der: Vec<u8>,
    /// `issuer` codificado como DER (Name completa).
    pub issuer: Vec<u8>,
    /// `serialNumber` en bytes big-endian.
    pub serial: Vec<u8>,
    /// `subject` codificado como DER.
    pub subject: Vec<u8>,
    /// Clave pública del sujeto.
    pub public_key: PublicKey,
}

/// Analiza un certificado X.509 en DER.
pub fn parse_x509_certificate(der: &[u8]) -> Result<X509Certificate, String> {
    let mut outer = DerReader::new(der);
    let cert = outer
        .read()
        .map_err(|e| format!("certificado (Certificate): {}", e))?;
    if cert.tag != 0x30 {
        return Err("certificado: se esperaba SEQUENCE".to_string());
    }
    let cert_items = outer
        .read_children(&cert)
        .map_err(|e| format!("certificado (campos): {}", e))?;
    if cert_items.len() < 3 {
        return Err("certificado: estructura incompleta".to_string());
    }
    let tbs = &cert_items[0];
    let tbs_items = outer
        .read_children(tbs)
        .map_err(|e| format!("certificado (tbsCertificate): {}", e))?;
    let mut idx = 0usize;
    if tbs_items.first().map(|t| t.tag) == Some(0xa0) {
        idx = 1; // version explícita
    }
    if tbs_items.len() < idx + 6 {
        return Err("certificado: tbsCertificate incompleto".to_string());
    }
    let serial = tbs_items[idx].int_bytes()?;
    let issuer = tbs_items[idx + 2].full.to_vec();
    let subject = tbs_items[idx + 4].full.to_vec();
    let spki = &tbs_items[idx + 5];
    if spki.tag != 0x30 {
        return Err("certificado: subjectPublicKeyInfo inválido".to_string());
    }
    let spki_items = outer
        .read_children(spki)
        .map_err(|e| format!("certificado (subjectPublicKeyInfo): {}", e))?;
    if spki_items.len() < 2 {
        return Err("certificado: subjectPublicKeyInfo incompleto".to_string());
    }
    let alg_children = outer
        .read_children(&spki_items[0])
        .map_err(|e| format!("certificado (AlgorithmIdentifier): {}", e))?;
    let alg_oid = alg_children
        .first()
        .ok_or_else(|| "certificado: AlgorithmIdentifier vacío".to_string())?
        .oid()?;
    let key_bits = spki_items[1].content;
    if key_bits.is_empty() {
        return Err("certificado: BIT STRING de clave vacía".to_string());
    }
    let key_bytes = &key_bits[1..]; // primer byte = bits no usados
    let public_key = if alg_oid == OID_RSA_ENCRYPTION {
        let mut r = DerReader::new(key_bytes);
        let seq = r.read()?;
        let items = r.read_children(&seq)?;
        if items.len() < 2 {
            return Err("certificado: RSAPublicKey incompleta".to_string());
        }
        PublicKey::Rsa {
            n: BigUint::from_bytes_be(&items[0].int_bytes()?),
            e: BigUint::from_bytes_be(&items[1].int_bytes()?),
        }
    } else if alg_oid == OID_EC_PUBLIC_KEY {
        PublicKey::Ec {
            point: key_bytes.to_vec(),
        }
    } else {
        return Err(format!(
            "algoritmo de clave no soportado: {}",
            alg_oid
                .iter()
                .map(|a| a.to_string())
                .collect::<Vec<_>>()
                .join(".")
        ));
    };
    Ok(X509Certificate {
        der: der.to_vec(),
        issuer,
        serial,
        subject,
        public_key,
    })
}

// ===========================================================================
// 9. CMS / CAdES (SignedData PKCS#7)
// ===========================================================================

/// `id-signedData`.
const OID_SIGNED_DATA: &[u64] = &[1, 2, 840, 113549, 1, 7, 2];
/// `id-data`.
const OID_DATA: &[u64] = &[1, 2, 840, 113549, 1, 7, 1];
/// `id-sha256`.
const OID_SHA256: &[u64] = &[2, 16, 840, 1, 101, 3, 4, 2, 1];
/// `ecdsa-with-SHA256`.
const OID_ECDSA_SHA256: &[u64] = &[1, 2, 840, 10045, 4, 3, 2];
/// `contentType` (atributo firmado).
const OID_CONTENT_TYPE: &[u64] = &[1, 2, 840, 113549, 1, 9, 3];
/// `messageDigest` (atributo firmado).
const OID_MESSAGE_DIGEST: &[u64] = &[1, 2, 840, 113549, 1, 9, 4];
/// `signingTime` (atributo firmado).
const OID_SIGNING_TIME: &[u64] = &[1, 2, 840, 113549, 1, 9, 5];
/// `signingCertificateV2` (ESS, atributo firmado).
const OID_SIGNING_CERT_V2: &[u64] = &[1, 2, 840, 113549, 1, 9, 16, 2, 47];
/// `commitment-type-indication` (ETSI, atributo firmado).
const OID_ETS_COMMITMENT_TYPE: &[u64] = &[1, 2, 840, 113549, 1, 9, 16, 2, 16];
/// `id-cti-ets-proofOfApproval`.
const OID_CTI_PROOF_OF_APPROVAL: &[u64] = &[1, 2, 840, 113549, 1, 9, 16, 6, 5];

/// Un atributo CMS: `SEQUENCE { attrType OID, attrValues SET OF ANY }`.
fn cms_attribute(oid: &[u64], values: &[Vec<u8>]) -> Vec<u8> {
    der_seq(&[der_oid(oid), der_set(values)])
}

/// `signingCertificateV2` con `ESSCertIDv2` (hash del DER completo del
/// certificado, que es lo que calcula OpenSSL).
fn cms_signing_certificate_v2(cert: &X509Certificate) -> Vec<u8> {
    let cert_hash = sha256(&cert.der);
    let issuer_serial = der_seq(&[
        der_seq(&[der_context(4, &cert.issuer)]),
        der_int(&cert.serial),
    ]);
    let ess_cert_id = der_seq(&[
        der_algid(OID_SHA256, false),
        der_octet(&cert_hash),
        issuer_serial,
    ]);
    let signing_certificate = der_seq(&[der_seq(&[ess_cert_id])]);
    cms_attribute(OID_SIGNING_CERT_V2, &[signing_certificate])
}

/// Construye el `SignedData` a partir del digest del contenido.
///
/// `econtent` es `Some` para firmas adjuntas y `None` para firmas
/// desprendidas (PAdES), donde el digest se calcula fuera sobre el
/// `ByteRange`.
fn build_signed_data(
    provider: &dyn SignatureProvider,
    content_digest: &[u8],
    econtent: Option<&[u8]>,
    options: &CmsOptions,
) -> Result<Vec<u8>, String> {
    if content_digest.len() != 32 {
        return Err(format!(
            "el digest debe medir 32 bytes (SHA-256), received {}",
            content_digest.len()
        ));
    }
    let cert_der = provider.certificate_der()?;
    let cert = parse_x509_certificate(&cert_der)?;
    let algorithm = provider.signature_algorithm();

    // --- Atributos firmados ---
    let signing_time = options
        .signing_time
        .clone()
        .unwrap_or_else(|| format_utctime(unix_now()));
    let mut attributes: Vec<Vec<u8>> = vec![
        cms_attribute(OID_CONTENT_TYPE, &[der_oid(OID_DATA)]),
        cms_attribute(OID_SIGNING_TIME, &[der_utctime(&signing_time)]),
        cms_attribute(OID_MESSAGE_DIGEST, &[der_octet(content_digest)]),
    ];
    if options.include_signing_certificate_v2 {
        attributes.push(cms_signing_certificate_v2(&cert));
    }
    if options.include_commitment_type {
        let indication = der_seq(&[der_oid(OID_CTI_PROOF_OF_APPROVAL)]);
        attributes.push(cms_attribute(OID_ETS_COMMITMENT_TYPE, &[indication]));
    }
    // DER exige que los elementos de un SET ordenen por su codificación.
    attributes.sort();

    let mut attrs_content: Vec<u8> = Vec::new();
    for a in &attributes {
        attrs_content.extend_from_slice(a);
    }
    // La firma se calcula sobre los atributos con etiqueta SET (0x31),
    // aunque en el SignerInfo van con etiqueta [0] IMPLICIT (0xA0).
    let signed_attrs_for_signing = der_tlv(0x31, &attrs_content);
    let digest = sha256(&signed_attrs_for_signing);
    let signature = provider.sign_digest(&digest)?;

    let signature_alg = match algorithm {
        KeyAlgorithm::RsaPkcs1 => der_algid(OID_RSA_ENCRYPTION, true),
        KeyAlgorithm::Ecdsa => der_algid(OID_ECDSA_SHA256, false),
    };

    let sid = der_seq(&[cert.issuer.clone(), der_int(&cert.serial)]);
    let signer_info = der_seq(&[
        der_int_u64(1),
        sid,
        der_algid(OID_SHA256, false),
        der_tlv(0xa0, &attrs_content),
        signature_alg,
        der_octet(&signature),
    ]);

    // --- SignedData ---
    let mut encap_items = vec![der_oid(OID_DATA)];
    if let Some(content) = econtent {
        encap_items.push(der_context(0, &der_octet(content)));
    }
    let encap = der_seq(&encap_items);

    let signed_data = der_seq(&[
        der_int_u64(1),
        der_set(&[der_algid(OID_SHA256, false)]),
        encap,
        der_context(0, &cert.der),
        der_set(&[signer_info]),
    ]);

    let content_info = der_seq(&[der_oid(OID_SIGNED_DATA), der_context(0, &signed_data)]);
    Ok(content_info)
}

/// Genera un `SignedData` CMS/CAdES con el contenido adjunto.
///
/// El resultado es verificable con:
/// `openssl cms -verify -inform DER -in sig.der -content fichero -binary -noverify`.
pub fn build_cms_signed_data(
    provider: &dyn SignatureProvider,
    content: &[u8],
    options: &CmsOptions,
) -> Result<Vec<u8>, String> {
    let digest = sha256(content);
    build_signed_data(provider, &digest, Some(content), options)
}

/// Genera un `SignedData` CMS/CAdES desprendido (PAdES).
///
/// `content_digest` es el SHA-256 de los bytes que se firman (por ejemplo,
/// los bloques del `ByteRange` de un PDF).
pub fn build_cms_detached_signed_data(
    provider: &dyn SignatureProvider,
    content_digest: &[u8],
    options: &CmsOptions,
) -> Result<Vec<u8>, String> {
    let mut opts = CmsOptions {
        detached: true,
        ..CmsOptions::default()
    };
    opts.signing_time = options.signing_time.clone();
    opts.include_signing_certificate_v2 = options.include_signing_certificate_v2;
    opts.include_commitment_type = options.include_commitment_type;
    build_signed_data(provider, content_digest, None, &opts)
}

/// Información extraída de un `SignedData` (para verificación).
pub struct CmsSignatureInfo {
    /// Digest del contenido declarado en `messageDigest`.
    pub content_digest: Vec<u8>,
    /// Bytes de los atributos firmados tal y como se firman (etiqueta SET).
    pub signed_attrs: Vec<u8>,
    /// Algoritmo de firma declarado.
    pub signature_algorithm: KeyAlgorithm,
    /// Firma cruda (DER para ECDSA).
    pub signature: Vec<u8>,
    /// Certificado del firmante embebido en el CMS (si lo hay).
    pub certificate: Option<Vec<u8>>,
}

/// Analiza un `SignedData` y extrae lo necesario para verificarlo.
pub fn parse_cms_signed_data(der: &[u8]) -> Result<CmsSignatureInfo, String> {
    let mut outer = DerReader::new(der);
    let content_info = outer
        .read()
        .map_err(|e| format!("CMS (ContentInfo): {}", e))?;
    let ci_items = outer
        .read_children(&content_info)
        .map_err(|e| format!("CMS (campos del ContentInfo): {}", e))?;
    if ci_items.len() < 2 {
        return Err("CMS: ContentInfo incompleto".to_string());
    }
    let ct = ci_items[0].oid()?;
    if ct != OID_SIGNED_DATA {
        return Err("CMS: el contentType no es signedData".to_string());
    }
    let mut sd_reader = DerReader::new(ci_items[1].content);
    let signed_data = sd_reader
        .read()
        .map_err(|e| format!("CMS (SignedData): {}", e))?;
    let sd_items = outer
        .read_children(&signed_data)
        .map_err(|e| format!("CMS (campos del SignedData): {}", e))?;
    // version, digestAlgorithms, encapContentInfo, [certificates], signerInfos
    let mut idx = 0usize;
    if sd_items.len() < 4 {
        return Err("CMS: SignedData incompleto".to_string());
    }
    idx += 3; // saltar version, digestAlgorithms, encapContentInfo
    let mut certificate: Option<Vec<u8>> = None;
    if sd_items.get(idx).map(|t| t.is_context(0)).unwrap_or(false) {
        certificate = Some(sd_items[idx].content.to_vec());
        idx += 1;
    }
    let signer_infos = sd_items
        .get(idx)
        .ok_or_else(|| "CMS: no hay signerInfos".to_string())?;
    let si_items = outer
        .read_children(signer_infos)
        .map_err(|e| format!("CMS (signerInfos): {}", e))?;
    let si = si_items
        .first()
        .ok_or_else(|| "CMS: signerInfos vacío".to_string())?;
    let fields = outer
        .read_children(si)
        .map_err(|e| format!("CMS (campos del SignerInfo): {}", e))?;
    // version, sid, digestAlgorithm, [signedAttrs], signatureAlgorithm, signature
    if fields.len() < 5 {
        return Err("CMS: SignerInfo incompleto".to_string());
    }
    // version, sid, digestAlgorithm, [0] signedAttrs, signatureAlgorithm,
    // signature, [1] unsignedAttrs
    let attrs_index = fields
        .iter()
        .position(|t| t.is_context(0))
        .ok_or_else(|| "CMS: faltan los atributos firmados".to_string())?;
    let signed_attrs_tlv = &fields[attrs_index];
    let sig_alg = fields
        .get(attrs_index + 1)
        .ok_or_else(|| "CMS: falta el algoritmo de firma".to_string())?;
    let signature = fields
        .get(attrs_index + 2)
        .ok_or_else(|| "CMS: falta la firma".to_string())?;
    if signature.tag != 0x04 {
        return Err("CMS: la firma no es un OCTET STRING".to_string());
    }

    // Recorre los atributos firmados buscando contentType y messageDigest.
    let mut content_digest = Vec::new();
    let mut attrs_reader = DerReader::new(signed_attrs_tlv.content);
    while !attrs_reader.is_empty() {
        let attr = attrs_reader
            .read()
            .map_err(|e| format!("CMS (atributo firmado): {}", e))?;
        let attr_items = outer
            .read_children(&attr)
            .map_err(|e| format!("CMS (campos del atributo): {}", e))?;
        if attr_items.len() < 2 {
            continue;
        }
        let oid = attr_items[0].oid()?;
        if oid == OID_MESSAGE_DIGEST {
            let mut set = DerReader::new(attr_items[1].content);
            let value = set
                .read()
                .map_err(|e| format!("CMS (valor de messageDigest): {}", e))?;
            if value.tag == 0x04 {
                content_digest = value.content.to_vec();
            }
        }
    }
    if content_digest.is_empty() {
        return Err("CMS: no se encontró el atributo messageDigest".to_string());
    }

    // `sig_alg` es el AlgorithmIdentifier: sus hijos son el OID y los
    // parámetros (NULL en RSA, ausentes en ECDSA).
    let alg_items = outer
        .read_children(sig_alg)
        .map_err(|e| format!("CMS (AlgorithmIdentifier de firma): {}", e))?;
    let alg_oid = alg_items
        .first()
        .ok_or_else(|| "CMS: algoritmo de firma vacío".to_string())?
        .oid()?;
    let signature_algorithm = if alg_oid == OID_RSA_ENCRYPTION {
        KeyAlgorithm::RsaPkcs1
    } else if alg_oid == OID_ECDSA_SHA256 {
        KeyAlgorithm::Ecdsa
    } else {
        return Err(format!(
            "CMS: algoritmo de firma no soportado: {}",
            alg_oid
                .iter()
                .map(|a| a.to_string())
                .collect::<Vec<_>>()
                .join(".")
        ));
    };

    Ok(CmsSignatureInfo {
        content_digest,
        signed_attrs: signed_attrs_tlv.full.to_vec(),
        signature_algorithm,
        signature: signature.content.to_vec(),
        certificate,
    })
}

/// Verifica un `SignedData` CMS/CAdES.
///
/// * `der`: CMS en DER.
/// * `data`: contenido firmado (el mismo que se resume en `messageDigest`).
/// * `cert_der`: certificado del firmante en DER.
///
/// Devuelve `Ok(true)` si la firma y el digest son correctos.
pub fn verify_cms(der: &[u8], data: &[u8], cert_der: &[u8]) -> Result<bool, String> {
    let info = parse_cms_signed_data(der)?;
    let cert = parse_x509_certificate(cert_der)?;

    // 1) El digest declarado debe coincidir con el del contenido.
    let expected = sha256(data);
    if !constant_time_eq(&info.content_digest, &expected) {
        return Ok(false);
    }

    // 2) La firma se calcula sobre los atributos firmados reetiquetados como SET.
    let mut signed_content = info.signed_attrs.clone();
    if signed_content.is_empty() {
        return Ok(false);
    }
    signed_content[0] = 0x31; // [0] IMPLICIT -> SET
    let digest = sha256(&signed_content);

    // 3) Verifica la firma con la clave pública del certificado.
    match (&cert.public_key, info.signature_algorithm) {
        (PublicKey::Rsa { n, e }, KeyAlgorithm::RsaPkcs1) => {
            Ok(rsa_pkcs1_v15_verify(n, e, &digest, &info.signature))
        }
        (PublicKey::Ec { point }, KeyAlgorithm::Ecdsa) => {
            ecdsa_verify_digest(point, &digest, &info.signature)
        }
        _ => Ok(false),
    }
}

// ===========================================================================
// 10. PDF: modelo de objetos, parser y escritor incremental
// ===========================================================================

/// Objeto PDF analizado.
#[derive(Clone, Debug)]
pub enum PdfObject {
    Null,
    Bool(bool),
    Number(f64),
    /// Literal string `(...)`.
    Literal(Vec<u8>),
    /// Hex string `<...>` con la posición de los dígitos en el fichero.
    Hex(Vec<u8>, usize, usize),
    /// Nombre `/Name`.
    Name(Vec<u8>),
    Array(Vec<PdfObject>),
    Dict(HashMap<Vec<u8>, PdfObject>),
    Stream {
        dict: HashMap<Vec<u8>, PdfObject>,
        /// Offset del primer byte de datos.
        data_offset: usize,
        /// Longitud de los datos sin descomprimir.
        data_len: usize,
    },
    /// Referencia indirecta `(n, g)`.
    Ref(u32, u16),
}

impl PdfObject {
    /// Nombre del objeto, si lo es.
    pub fn name(&self) -> Option<&[u8]> {
        match self {
            PdfObject::Name(n) => Some(n),
            _ => None,
        }
    }

    /// Diccionario (o stream), si lo es.
    pub fn dict(&self) -> Option<&HashMap<Vec<u8>, PdfObject>> {
        match self {
            PdfObject::Dict(d) => Some(d),
            PdfObject::Stream { dict, .. } => Some(dict),
            _ => None,
        }
    }

    /// Entero, si lo es.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            PdfObject::Number(n) => Some(*n as i64),
            _ => None,
        }
    }

    /// Número de objeto, tanto si es un número como una referencia indirecta.
    pub fn object_number(&self) -> Option<u32> {
        match self {
            PdfObject::Number(n) => Some(*n as u32),
            PdfObject::Ref(n, _) => Some(*n),
            _ => None,
        }
    }
}

/// Entrada de la tabla de referencias cruzadas.
#[derive(Clone, Copy, Debug)]
enum XrefEntry {
    /// Objeto directo en el fichero, en el offset dado.
    Offset(usize, u16),
    /// Objeto dentro de un object stream.
    InStream(u32, u32),
}

/// Analizador sintáctico de PDF.
struct PdfParser<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> PdfParser<'a> {
    fn new(buf: &'a [u8], pos: usize) -> Self {
        PdfParser { buf, pos }
    }

    fn skip_ws(&mut self) {
        loop {
            while self.pos < self.buf.len() && is_pdf_space(self.buf[self.pos]) {
                self.pos += 1;
            }
            if self.pos < self.buf.len() && self.buf[self.pos] == b'%' {
                while self.pos < self.buf.len()
                    && self.buf[self.pos] != b'\n'
                    && self.buf[self.pos] != b'\r'
                {
                    self.pos += 1;
                }
                continue;
            }
            break;
        }
    }

    fn peek_keyword(&mut self, kw: &[u8]) -> bool {
        self.skip_ws();
        if self.buf.len() >= self.pos + kw.len()
            && &self.buf[self.pos..self.pos + kw.len()] == kw
        {
            self.pos += kw.len();
            true
        } else {
            false
        }
    }

    fn parse_object(&mut self) -> Result<PdfObject, String> {
        self.skip_ws();
        let b = *self
            .buf
            .get(self.pos)
            .ok_or_else(|| "PDF: final de fichero inesperado".to_string())?;
        match b {
            b'<' => {
                if self.buf.get(self.pos + 1) == Some(&b'<') {
                    self.pos += 2;
                    let mut map = HashMap::new();
                    loop {
                        self.skip_ws();
                        if self.peek_keyword(b">>") {
                            break;
                        }
                        let key = match self.parse_object()? {
                            PdfObject::Name(n) => n,
                            other => {
                                return Err(format!(
                                    "PDF: clave de diccionario inesperada {:?} (offset {})",
                                    other, self.pos
                                ))
                            }
                        };
                        let value = self.parse_object()?;
                        map.insert(key, value);
                    }
                    Ok(PdfObject::Dict(map))
                } else {
                    let start = self.pos + 1;
                    self.pos += 1;
                    let mut digits = Vec::new();
                    while self.pos < self.buf.len() && self.buf[self.pos] != b'>' {
                        let c = self.buf[self.pos];
                        if is_pdf_space(c) {
                            self.pos += 1;
                            continue;
                        }
                        let v = (c as char)
                            .to_digit(16)
                            .ok_or_else(|| "PDF: dígito hexadecimal inválido".to_string())?;
                        digits.push(v as u8);
                        self.pos += 1;
                    }
                    if self.pos < self.buf.len() {
                        self.pos += 1; // '>'
                    }
                    let mut out = Vec::with_capacity(digits.len() / 2);
                    let mut i = 0;
                    while i + 1 < digits.len() {
                        out.push((digits[i] << 4) | digits[i + 1]);
                        i += 2;
                    }
                    if i < digits.len() {
                        out.push(digits[i] << 4);
                    }
                    Ok(PdfObject::Hex(out, start, digits.len()))
                }
            }
            b'(' => {
                self.pos += 1;
                let mut depth = 1usize;
                let mut out = Vec::new();
                while self.pos < self.buf.len() {
                    let c = self.buf[self.pos];
                    self.pos += 1;
                    if c == b'\\' {
                        if self.pos < self.buf.len() {
                            let e = self.buf[self.pos];
                            self.pos += 1;
                            match e {
                                b'n' => out.push(b'\n'),
                                b'r' => out.push(b'\r'),
                                b't' => out.push(b'\t'),
                                b'b' => out.push(8),
                                b'f' => out.push(12),
                                b'(' => out.push(b'('),
                                b')' => out.push(b')'),
                                b'\\' => out.push(b'\\'),
                                b'\r' => {
                                    if self.buf.get(self.pos) == Some(&b'\n') {
                                        self.pos += 1;
                                    }
                                }
                                b'\n' => {}
                                d if d.is_ascii_digit() => {
                                    let mut val = (d - b'0') as u32;
                                    for _ in 0..2 {
                                        if let Some(&nx) = self.buf.get(self.pos) {
                                            if nx.is_ascii_digit() {
                                                val = val * 8 + (nx - b'0') as u32;
                                                self.pos += 1;
                                            }
                                        }
                                    }
                                    out.push((val & 0xff) as u8);
                                }
                                other => out.push(other),
                            }
                        }
                        continue;
                    }
                    if c == b'(' {
                        depth += 1;
                    } else if c == b')' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    out.push(c);
                }
                Ok(PdfObject::Literal(out))
            }
            b'[' => {
                self.pos += 1;
                let mut items = Vec::new();
                loop {
                    self.skip_ws();
                    if self.pos < self.buf.len() && self.buf[self.pos] == b']' {
                        self.pos += 1;
                        break;
                    }
                    items.push(self.parse_object()?);
                }
                Ok(PdfObject::Array(items))
            }
            b'/' => {
                self.pos += 1;
                let mut name = Vec::new();
                while self.pos < self.buf.len() {
                    let c = self.buf[self.pos];
                    if is_pdf_space(c) || matches!(c, b'/' | b'(' | b'<' | b'[' | b']' | b'>') {
                        break;
                    }
                    name.push(c);
                    self.pos += 1;
                }
                Ok(PdfObject::Name(name))
            }
            b'+' | b'-' | b'.' | b'0'..=b'9' => {
                let start = self.pos;
                if self.buf[self.pos] == b'+' || self.buf[self.pos] == b'-' {
                    self.pos += 1;
                }
                while self.pos < self.buf.len()
                    && (self.buf[self.pos].is_ascii_digit() || self.buf[self.pos] == b'.')
                {
                    self.pos += 1;
                }
                let text = std::str::from_utf8(&self.buf[start..self.pos])
                    .map_err(|_| "PDF: número inválido".to_string())?;
                let value: f64 = text
                    .parse()
                    .map_err(|_| format!("PDF: número inválido '{}'", text))?;
                // ¿Es una referencia indirecta "n g R"?
                if value.fract() == 0.0 {
                    let save = self.pos;
                    self.skip_ws();
                    let gen_start = self.pos;
                    let mut q = self.pos;
                    while q < self.buf.len() && self.buf[q].is_ascii_digit() {
                        q += 1;
                    }
                    if q > gen_start {
                        let gen_text =
                            std::str::from_utf8(&self.buf[gen_start..q]).unwrap_or_default();
                        if let Ok(gen) = gen_text.parse::<u16>() {
                            self.pos = q;
                            self.skip_ws();
                            if self.buf.get(self.pos) == Some(&b'R')
                                && self
                                    .buf
                                    .get(self.pos + 1)
                                    .map(|&c| is_pdf_delimiter(c))
                                    .unwrap_or(true)
                            {
                                self.pos += 1;
                                return Ok(PdfObject::Ref(value as u32, gen));
                            }
                        }
                    }
                    self.pos = save;
                }
                Ok(PdfObject::Number(value))
            }
            _ => {
                // Palabra clave: true / false / null / R / endobj / ...
                let start = self.pos;
                while self.pos < self.buf.len()
                    && !is_pdf_space(self.buf[self.pos])
                    && !matches!(
                        self.buf[self.pos],
                        b'/' | b'(' | b'<' | b'[' | b']' | b'>' | b'{' | b'}'
                    )
                {
                    self.pos += 1;
                }
                let word = std::str::from_utf8(&self.buf[start..self.pos]).unwrap_or("");
                match word {
                    "true" => Ok(PdfObject::Bool(true)),
                    "false" => Ok(PdfObject::Bool(false)),
                    "null" => Ok(PdfObject::Null),
                    "" => Err("PDF: objeto vacío".to_string()),
                    other => Err(format!(
                        "PDF: palabra clave inesperada '{}' (offset {})",
                        other, start
                    )),
                }
            }
        }
    }
}

/// ¿Es un carácter de espacio en PDF?
fn is_pdf_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\r' | b'\n' | 0x0c)
}

/// ¿Es un carácter que puede seguir a un objeto PDF?
fn is_pdf_delimiter(c: u8) -> bool {
    is_pdf_space(c) || matches!(c, b'/' | b'[' | b']' | b'<' | b'>' | b')' | b'{' | b'}')
}

/// Lee un entero del analizador.
fn pdf_read_int(p: &mut PdfParser) -> Result<i64, String> {
    p.skip_ws();
    let start = p.pos;
    if p.pos < p.buf.len() && (p.buf[p.pos] == b'+' || p.buf[p.pos] == b'-') {
        p.pos += 1;
    }
    while p.pos < p.buf.len() && p.buf[p.pos].is_ascii_digit() {
        p.pos += 1;
    }
    let text = std::str::from_utf8(&p.buf[start..p.pos]).unwrap_or("");
    text.parse::<i64>()
        .map_err(|_| format!("PDF: entero inválido '{}'", text))
}

/// Lee una palabra clave corta.
fn pdf_read_word(p: &mut PdfParser) -> Result<String, String> {
    p.skip_ws();
    let start = p.pos;
    while p.pos < p.buf.len()
        && !is_pdf_space(p.buf[p.pos])
        && !matches!(p.buf[p.pos], b'/' | b'(' | b'<' | b'[' | b']' | b'>')
    {
        p.pos += 1;
    }
    Ok(String::from_utf8_lossy(&p.buf[start..p.pos]).to_string())
}

/// Busca la última aparición de un marcador en el buffer.
fn pdf_find_last(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    let mut pos = 0usize;
    let mut found = None;
    while pos + needle.len() <= haystack.len() {
        if &haystack[pos..pos + needle.len()] == needle {
            found = Some(pos);
            pos += 1;
        } else {
            pos += 1;
        }
    }
    found
}

/// Documento PDF con su tabla de referencias cruzadas resuelta.
pub struct PdfDocument<'a> {
    bytes: &'a [u8],
    xref: HashMap<u32, XrefEntry>,
    trailer: HashMap<Vec<u8>, PdfObject>,
    root: u32,
    startxref: usize,
    classic: bool,
    size: u32,
}

impl<'a> PdfDocument<'a> {
    /// Analiza un PDF.
    pub fn parse(bytes: &'a [u8]) -> Result<PdfDocument<'a>, String> {
        let sx_pos = pdf_find_last(bytes, b"startxref")
            .ok_or_else(|| "PDF: no se encontró 'startxref'".to_string())?;
        let mut p = PdfParser::new(bytes, sx_pos + 9);
        let start = pdf_read_int(&mut p)? as usize;
        let (mut xref, mut trailer, classic) = parse_xref_at(bytes, start)?;
        let mut prev = trailer
            .get(&b"Prev".to_vec())
            .and_then(|o| o.as_int())
            .map(|v| v as usize);
        let mut guard = 0;
        while let Some(po) = prev {
            guard += 1;
            if guard > 64 {
                break;
            }
            match parse_xref_at(bytes, po) {
                Ok((x, t, _)) => {
                    for (k, v) in x {
                        xref.entry(k).or_insert(v);
                    }
                    for (k, v) in &t {
                        trailer.entry(k.clone()).or_insert(v.clone());
                    }
                    prev = t.get(&b"Prev".to_vec()).and_then(|o| o.as_int()).map(|v| v as usize);
                }
                Err(_) => break,
            }
        }
        // Respaldo: si la xref no ofrece la raíz, se busca por el patrón "N G obj".
        let root = match trailer
            .get(&b"Root".to_vec())
            .and_then(|o| o.object_number())
        {
            Some(r) => r,
            None => scan_for_root(bytes),
        };
        let size = trailer
            .get(&b"Size".to_vec())
            .and_then(|o| o.as_int())
            .map(|v| v as u32)
            .unwrap_or_else(|| {
                xref.keys().copied().max().unwrap_or(0) + 1
            });
        Ok(PdfDocument {
            bytes,
            xref,
            trailer,
            root,
            startxref: start,
            classic,
            size,
        })
    }

    /// Número de objeto del catálogo.
    pub fn root_object(&self) -> u32 {
        self.root
    }

    /// Tamaño declarado del fichero (número de objetos).
    pub fn size(&self) -> u32 {
        self.size
    }

    /// Offset del `startxref` original.
    pub fn startxref(&self) -> usize {
        self.startxref
    }

    /// ¿La xref original es una tabla clásica?
    pub fn has_classic_xref(&self) -> bool {
        self.classic
    }

    /// Generación declarada en la xref para un objeto.
    pub fn xref_entry_gen(&self, num: u32) -> Option<u16> {
        match self.xref.get(&num) {
            Some(XrefEntry::Offset(_, gen)) => Some(*gen),
            Some(XrefEntry::InStream(_, _)) => Some(0),
            None => None,
        }
    }

    /// Offset declarado en la xref para un objeto.
    pub fn xref_entry_offset(&self, num: u32) -> Option<usize> {
        match self.xref.get(&num) {
            Some(XrefEntry::Offset(off, _)) => Some(*off),
            Some(XrefEntry::InStream(_, _)) => None,
            None => scan_object_offset(self.bytes, num),
        }
    }

    /// Número de objeto del `/Info` del trailer.
    pub fn trailer_info(&self) -> Option<u32> {
        self.trailer
            .get(&b"Info".to_vec())
            .and_then(|o| o.object_number())
    }

    /// Número de objeto más alto conocido (para asignar objetos nuevos).
    pub fn max_object_number(&self) -> u32 {
        let scanned = scan_object_numbers(self.bytes);
        let mut max = self.xref.keys().copied().max().unwrap_or(0);
        for (num, _) in scanned {
            if num > max {
                max = num;
            }
        }
        max
    }

    /// Objeto del catálogo.
    pub fn catalog(&self) -> Result<PdfObject, String> {
        self.load_object(self.root)
    }

    /// Carga un objeto por número, resolviendo referencias indirectas.
    pub fn load_object(&self, num: u32) -> Result<PdfObject, String> {
        let entry = match self.xref.get(&num) {
            Some(e) => *e,
            None => {
                // Respaldo: búsqueda por patrón "N G obj".
                match scan_object_offset(self.bytes, num) {
                    Some(off) => return self.parse_indirect_at(off),
                    None => return Err(format!("PDF: objeto {} no encontrado", num)),
                }
            }
        };
        match entry {
            XrefEntry::Offset(off, _) => self.parse_indirect_at(off),
            XrefEntry::InStream(stm, idx) => self.load_from_object_stream(stm, idx),
        }
    }

    /// Resuelve una referencia indirecta si lo es.
    pub fn resolve(&self, obj: &PdfObject) -> Result<PdfObject, String> {
        match obj {
            PdfObject::Ref(num, _) => self.load_object(*num),
            other => Ok(other.clone()),
        }
    }

    fn parse_indirect_at(&self, offset: usize) -> Result<PdfObject, String> {
        let mut p = PdfParser::new(self.bytes, offset);
        p.skip_ws();
        let _num = pdf_read_int(&mut p)?;
        let _gen = pdf_read_int(&mut p)?;
        let word = pdf_read_word(&mut p)?;
        if word != "obj" {
            return Err(format!("PDF: se esperaba 'obj' en el offset {}", offset));
        }
        let obj = p.parse_object()?;
        p.skip_ws();
        if self.bytes.len() >= p.pos + 6 && &self.bytes[p.pos..p.pos + 6] == b"stream" {
            p.pos += 6;
            if self.bytes.get(p.pos) == Some(&b'\r') {
                p.pos += 1;
            }
            if self.bytes.get(p.pos) == Some(&b'\n') {
                p.pos += 1;
            }
            let data_offset = p.pos;
            let dict = obj.dict().cloned().unwrap_or_default();
            let len = match dict.get(&b"Length".to_vec()) {
                Some(PdfObject::Number(n)) => *n as usize,
                Some(PdfObject::Ref(num, _)) => match self.load_object(*num)? {
                    PdfObject::Number(n) => n as usize,
                    _ => 0,
                },
                _ => 0,
            };
            let len = len.min(self.bytes.len().saturating_sub(data_offset));
            return Ok(PdfObject::Stream {
                dict,
                data_offset,
                data_len: len,
            });
        }
        Ok(obj)
    }

    fn load_from_object_stream(&self, stm_num: u32, index: u32) -> Result<PdfObject, String> {
        let obj = self.load_object(stm_num)?;
        let (dict, data) = match obj {
            PdfObject::Stream { dict, data_offset, data_len } => {
                let raw = &self.bytes[data_offset..data_offset + data_len];
                let data = decode_stream(&dict, raw)?;
                (dict, data)
            }
            _ => return Err("PDF: el object stream no es un stream".to_string()),
        };
        let n = dict.get(&b"N".to_vec()).and_then(|o| o.as_int()).unwrap_or(0) as usize;
        let first = dict
            .get(&b"First".to_vec())
            .and_then(|o| o.as_int())
            .unwrap_or(0) as usize;
        let header = &data[..first.min(data.len())];
        let mut nums: Vec<u32> = Vec::with_capacity(n * 2);
        let mut cur = String::new();
        for &c in header {
            if c.is_ascii_digit() {
                cur.push(c as char);
            } else if !cur.is_empty() {
                nums.push(cur.parse::<u32>().unwrap_or(0));
                cur.clear();
            }
        }
        if !cur.is_empty() {
            nums.push(cur.parse::<u32>().unwrap_or(0));
        }
        if index as usize * 2 + 1 >= nums.len() {
            return Err("PDF: índice fuera del object stream".to_string());
        }
        let obj_num = nums[index as usize * 2];
        let rel = nums[index as usize * 2 + 1] as usize;
        let start = first + rel;
        if start >= data.len() {
            return Err("PDF: offset inválido dentro del object stream".to_string());
        }
        let mut p = PdfParser::new(&data, start);
        let parsed = p.parse_object()?;
        let _ = obj_num;
        Ok(parsed)
    }
}

/// Decodifica los datos de un stream (soporta `FlateDecode`).
fn decode_stream(
    dict: &HashMap<Vec<u8>, PdfObject>,
    raw: &[u8],
) -> Result<Vec<u8>, String> {
    let filters = dict.get(&b"Filter".to_vec());
    let is_flate = match filters {
        Some(PdfObject::Name(n)) => n.as_slice() == b"FlateDecode",
        Some(PdfObject::Array(items)) => items
            .iter()
            .any(|i| matches!(i, PdfObject::Name(n) if n.as_slice() == b"FlateDecode")),
        _ => false,
    };
    if !is_flate {
        return Ok(raw.to_vec());
    }
    use std::io::Read as _;
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(raw)
        .read_to_end(&mut out)
        .map_err(|e| format!("PDF: error al descomprimir el stream: {e}"))?;
    Ok(out)
}

/// Analiza una sección xref (tabla clásica o stream).
fn parse_xref_at(
    bytes: &[u8],
    offset: usize,
) -> Result<(HashMap<u32, XrefEntry>, HashMap<Vec<u8>, PdfObject>, bool), String> {
    let mut p = PdfParser::new(bytes, offset);
    p.skip_ws();
    if bytes.len() >= p.pos + 4 && &bytes[p.pos..p.pos + 4] == b"xref" {
        p.pos += 4;
        let mut entries = HashMap::new();
        loop {
            p.skip_ws();
            if p.pos < bytes.len() && bytes[p.pos] == b't' {
                break; // "trailer"
            }
            if p.pos >= bytes.len() {
                break;
            }
            let start = pdf_read_int(&mut p)?;
            let count = pdf_read_int(&mut p)?;
            for i in 0..count {
                p.skip_ws();
                let off = pdf_read_int(&mut p)?;
                let gen = pdf_read_int(&mut p)?;
                let kind = pdf_read_word(&mut p)?;
                if kind == "n" {
                    entries.insert(
                        (start + i) as u32,
                        XrefEntry::Offset(off.max(0) as usize, gen.max(0) as u16),
                    );
                }
            }
        }
        p.skip_ws();
        let keyword = pdf_read_word(&mut p)?;
        if keyword != "trailer" {
            return Err(format!(
                "PDF: se esperaba 'trailer' tras la tabla xref, se encontró '{}'",
                keyword
            ));
        }
        let mut q = PdfParser::new(bytes, p.pos);
        let trailer_obj = q.parse_object()?;
        let trailer = trailer_obj.dict().cloned().unwrap_or_default();
        return Ok((entries, trailer, true));
    }

    // xref stream: "N G obj << ... >> stream ... endstream"
    let _num = pdf_read_int(&mut p)?;
    let _gen = pdf_read_int(&mut p)?;
    let word = pdf_read_word(&mut p)?;
    if word != "obj" {
        return Err("PDF: xref inválida".to_string());
    }
    let dict_obj = p.parse_object()?;
    let dict = dict_obj.dict().cloned().unwrap_or_default();
    p.skip_ws();
    if bytes.len() < p.pos + 6 || &bytes[p.pos..p.pos + 6] != b"stream" {
        return Err("PDF: falta el stream de la xref".to_string());
    }
    p.pos += 6;
    if bytes.get(p.pos) == Some(&b'\r') {
        p.pos += 1;
    }
    if bytes.get(p.pos) == Some(&b'\n') {
        p.pos += 1;
    }
    let data_offset = p.pos;
    let len = dict
        .get(&b"Length".to_vec())
        .and_then(|o| o.as_int())
        .unwrap_or(0)
        .max(0) as usize;
    let len = len.min(bytes.len().saturating_sub(data_offset));
    let data = decode_stream(&dict, &bytes[data_offset..data_offset + len])?;
    let w: Vec<usize> = dict
        .get(&b"W".to_vec())
        .and_then(|o| match o {
            PdfObject::Array(items) => Some(
                items
                    .iter()
                    .map(|i| i.as_int().unwrap_or(0).max(0) as usize)
                    .collect(),
            ),
            _ => None,
        })
        .ok_or_else(|| "PDF: xref stream sin /W".to_string())?;
    let size = dict.get(&b"Size".to_vec()).and_then(|o| o.as_int()).unwrap_or(0);
    let index: Vec<usize> = match dict.get(&b"Index".to_vec()) {
        Some(PdfObject::Array(items)) => items
            .iter()
            .map(|i| i.as_int().unwrap_or(0).max(0) as usize)
            .collect(),
        _ => vec![0, size.max(0) as usize],
    };
    let mut entries = HashMap::new();
    let mut cursor = 0usize;
    let mut pair = index.chunks(2);
    while let Some(chunk) = pair.next() {
        if chunk.len() < 2 {
            break;
        }
        let start = chunk[0];
        let count = chunk[1];
        for i in 0..count {
            if cursor + w.iter().sum::<usize>() > data.len() {
                break;
            }
            let mut fields = [0u64; 3];
            for (fi, width) in w.iter().enumerate() {
                let mut value: u64 = 0;
                for _ in 0..*width {
                    value = (value << 8) | data[cursor] as u64;
                    cursor += 1;
                }
                fields[fi] = value;
            }
            let obj_num = (start + i) as u32;
            if w[0] == 0 {
                continue;
            }
            match fields[0] {
                1 => {
                    entries.insert(
                        obj_num,
                        XrefEntry::Offset(fields[1] as usize, fields[2] as u16),
                    );
                }
                2 => {
                    entries.insert(obj_num, XrefEntry::InStream(fields[1] as u32, fields[2] as u32));
                }
                _ => {}
            }
        }
    }
    Ok((entries, dict, false))
}

/// Escanea el fichero buscando "N G obj" (respaldo si la xref está rota).
fn scan_object_offsets(bytes: &[u8]) -> Vec<(u32, usize)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 3 < bytes.len() {
        if bytes[i] == b'o' && &bytes[i + 1..i + 4] == b"obj" {
            let before_ok = i == 0 || is_pdf_space(bytes[i - 1]);
            if before_ok {
                // Retrocede para leer "N G obj".
                let mut j = i;
                while j > 0 && (bytes[j - 1].is_ascii_digit() || bytes[j - 1] == b' ') {
                    j -= 1;
                }
                let text = String::from_utf8_lossy(&bytes[j..i]).trim().to_string();
                let mut parts = text.split_whitespace();
                if let (Some(num), Some(_gen)) = (parts.next(), parts.next()) {
                    if let Ok(n) = num.parse::<u32>() {
                        out.push((n, j));
                    }
                }
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    out
}

/// Números de objeto presentes en el fichero.
fn scan_object_numbers(bytes: &[u8]) -> Vec<(u32, ())> {
    scan_object_offsets(bytes).into_iter().map(|(n, _)| (n, ())).collect()
}

/// Busca el offset de un objeto concreto.
fn scan_object_offset(bytes: &[u8], num: u32) -> Option<usize> {
    scan_object_offsets(bytes)
        .into_iter()
        .find(|(n, _)| *n == num)
        .map(|(_, off)| off)
}

/// Busca el objeto raíz (`/Type /Catalog`) escaneando el fichero.
fn scan_for_root(bytes: &[u8]) -> u32 {
    for (num, off) in scan_object_offsets(bytes) {
        let window_end = (off + 2048).min(bytes.len());
        if let Some(pos) = find_bytes(&bytes[off..window_end], b"/Catalog") {
            let _ = pos;
            return num;
        }
    }
    1
}

/// Busca una subcadena.
fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Opciones de firma de un PDF.
#[derive(Debug, Clone)]
pub struct PdfSignOptions {
    /// Nombre del campo de firma (`/T`).
    pub field_name: String,
    /// Motivo (`/Reason`).
    pub reason: String,
    /// Localidad (`/Location`).
    pub location: String,
    /// Nombre del firmante (`/Name`).
    pub signer_name: String,
    /// Bytes reservados para la firma dentro de `/Contents`.
    pub reserved_signature_bytes: usize,
    /// Opciones del CMS/CAdES embebido.
    pub cms_options: CmsOptions,
}

impl Default for PdfSignOptions {
    fn default() -> Self {
        PdfSignOptions {
            field_name: "Signature1".to_string(),
            reason: "Firmado electrónicamente".to_string(),
            location: "España".to_string(),
            signer_name: "MegaDaemon".to_string(),
            reserved_signature_bytes: 16384,
            cms_options: CmsOptions::default(),
        }
    }
}

/// Resultado de firmar un PDF.
#[derive(Debug, Clone)]
pub struct PdfSignResult {
    /// PDF firmado.
    pub bytes: Vec<u8>,
    /// `/ByteRange` aplicado.
    pub byte_range: [usize; 4],
    /// CMS/CAdES generado.
    pub cms_der: Vec<u8>,
    /// Digest SHA-256 de los bytes firmados.
    pub digest: [u8; 32],
    /// Bytes efectivamente firmados (los dos tramos del `ByteRange`).
    pub signed_content: Vec<u8>,
    /// Nombre del campo de firma utilizado.
    pub field_name: String,
}

/// Ancho reservado para cada número del `/ByteRange` (separación por espacios).
const BR_FIELD_WIDTH: usize = 12;

/// Fecha PDF (`D:YYYYMMDDHHmmSSZ`).
fn pdf_date(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "D:{:04}{:02}{:02}{:02}{:02}{:02}Z",
        y,
        m,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Serializa un objeto PDF analizado.
fn pdf_write_object(obj: &PdfObject) -> Vec<u8> {
    match obj {
        PdfObject::Null => b"null".to_vec(),
        PdfObject::Bool(b) => {
            if *b { b"true".to_vec() } else { b"false".to_vec() }
        }
        PdfObject::Number(n) => {
            if n.fract() == 0.0 {
                format!("{}", *n as i64).into_bytes()
            } else {
                format!("{}", n).into_bytes()
            }
        }
        PdfObject::Literal(bytes) => {
            let mut out = vec![b'('];
            for &b in bytes {
                match b {
                    b'(' | b')' | b'\\' => {
                        out.push(b'\\');
                        out.push(b);
                    }
                    b'\r' => out.extend_from_slice(b"\\r"),
                    b'\n' => out.extend_from_slice(b"\\n"),
                    _ => out.push(b),
                }
            }
            out.push(b')');
            out
        }
        PdfObject::Hex(bytes, _, _) => {
            let mut out = vec![b'<'];
            out.extend_from_slice(&hex_encode(bytes).into_bytes());
            out.push(b'>');
            out
        }
        PdfObject::Name(name) => {
            let mut out = vec![b'/'];
            for &b in name {
                if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'+') {
                    out.push(b);
                } else {
                    out.extend_from_slice(format!("#{:02X}", b).as_bytes());
                }
            }
            out
        }
        PdfObject::Array(items) => {
            let mut out = vec![b'['];
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(b' ');
                }
                out.extend_from_slice(&pdf_write_object(item));
            }
            out.push(b']');
            out
        }
        PdfObject::Dict(map) => {
            let mut out = b"<<".to_vec();
            for (k, v) in map {
                out.push(b' ');
                out.extend_from_slice(&pdf_write_object(&PdfObject::Name(k.clone())));
                out.push(b' ');
                out.extend_from_slice(&pdf_write_object(v));
            }
            out.extend_from_slice(b" >>");
            out
        }
        PdfObject::Stream { dict, .. } => pdf_write_object(&PdfObject::Dict(dict.clone())),
        PdfObject::Ref(num, gen) => format!("{} {} R", num, gen).into_bytes(),
    }
}

/// Cuerpo del diccionario de firma con los offsets de sus campos clave.
///
/// Devuelve (cuerpo, offset de los 4 números del ByteRange, offset de los
/// dígitos hexadecimales de `/Contents`).
fn signature_dict_body(
    options: &PdfSignOptions,
    date: &str,
) -> (Vec<u8>, [usize; 4], usize) {
    let mut body = String::new();
    body.push_str("<< /Type /Sig /Filter /Adobe.PPKLite /SubFilter /ETSI.CAdES.detached");
    body.push_str(&format!(" /M ({})", date));
    body.push_str(&format!(" /Name ({})", pdf_escape(&options.signer_name)));
    body.push_str(&format!(" /Reason ({})", pdf_escape(&options.reason)));
    body.push_str(&format!(" /Location ({})", pdf_escape(&options.location)));
    body.push_str(" /ByteRange [");
    let br_start = body.len();
    let mut br_offsets = [0usize; 4];
    for i in 0..4 {
        if i > 0 {
            body.push(' ');
        }
        br_offsets[i] = body.len();
        body.push_str(&format!("{:>width$}", 0, width = BR_FIELD_WIDTH));
    }
    body.push(']');
    let _ = br_start;
    body.push_str(" /Contents <");
    let contents_offset = body.len();
    body.push_str(&"0".repeat(options.reserved_signature_bytes * 2));
    body.push('>');
    body.push_str(" >>");
    (
        body.into_bytes(),
        br_offsets,
        contents_offset,
    )
}

/// Escapa una cadena para un literal PDF.
fn pdf_escape(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out
}

/// Escribe una tabla xref clásica al final del buffer.
fn write_classic_xref(
    out: &mut Vec<u8>,
    entries: &[(u32, u16, usize)],
    size: u32,
    root: u32,
    prev: usize,
    info: Option<u32>,
) {
    out.extend_from_slice(b"xref\n");
    let mut sorted: Vec<(u32, u16, usize)> = entries.to_vec();
    sorted.sort_by_key(|e| e.0);
    // Agrupa números de objeto consecutivos.
    let mut idx = 0usize;
    while idx < sorted.len() {
        let start = sorted[idx].0;
        let mut count = 1usize;
        while idx + count < sorted.len() && sorted[idx + count].0 == start + count as u32 {
            count += 1;
        }
        out.extend_from_slice(format!("{} {}\n", start, count).as_bytes());
        for e in &sorted[idx..idx + count] {
            out.extend_from_slice(format!("{:010} {:05} n \n", e.2, e.1).as_bytes());
        }
        idx += count;
    }
    out.extend_from_slice(b"trailer\n<< ");
    out.extend_from_slice(format!("/Size {} ", size).as_bytes());
    out.extend_from_slice(format!("/Root {} 0 R ", root).as_bytes());
    if let Some(i) = info {
        out.extend_from_slice(format!("/Info {} 0 R ", i).as_bytes());
    }
    out.extend_from_slice(format!("/Prev {} >>\n", prev).as_bytes());
}

/// Construye un PDF mínimo con un campo de firma (SAV) vacío.
pub fn build_pdf_with_signature_field(title: &str) -> Result<Vec<u8>, String> {
    let options = PdfSignOptions::default();
    let date = pdf_date(unix_now());
    let (sig_body, _br, _co) = signature_dict_body(&options, &date);

    let mut objects: Vec<(u32, Vec<u8>)> = Vec::new();
    objects.push((
        1,
        b"<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [5 0 R] /SigFlags 3 /DA (/Helv 0 Tf 0 g) >> >>".to_vec(),
    ));
    objects.push((2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec()));
    objects.push((
        3,
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 6 0 R >> >> /Contents 4 0 R /Annots [5 0 R] >>".to_vec(),
    ));
    let content = format!("BT /F1 18 Tf 72 760 Td ({}) Tj ET", pdf_escape(title));
    objects.push((
        4,
        format!(
            "<< /Length {} >>\nstream\n{}\nendstream",
            content.len(),
            content
        )
        .into_bytes(),
    ));
    objects.push((
        5,
        b"<< /Type /Annot /Subtype /Widget /FT /Sig /T (Signature1) /Rect [72 690 320 750] /F 4 /P 3 0 R /V 7 0 R /DA (/Helv 0 Tf 0 g) >>".to_vec(),
    ));
    objects.push((
        6,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec(),
    ));
    objects.push((7, sig_body));

    let mut out: Vec<u8> = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets: Vec<(u32, u16, usize)> = Vec::new();
    for (num, body) in &objects {
        let offset = out.len();
        out.extend_from_slice(format!("{} 0 obj\n", num).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
        offsets.push((*num, 0, offset));
    }
    let xref_offset = out.len();
    write_classic_xref(&mut out, &offsets, 8, 1, 0, None);
    out.extend_from_slice(format!("startxref\n{}\n%%EOF\n", xref_offset).as_bytes());
    Ok(out)
}

/// Información sobre el campo de firma encontrado en un PDF.
#[derive(Debug, Clone)]
pub struct PdfSignatureField {
    /// Número de objeto del campo (widget).
    pub field_number: u32,
    /// Número de objeto del diccionario de firma (`/V`), si es indirecto.
    pub signature_number: Option<u32>,
    /// Nombre del campo (`/T`).
    pub name: String,
}

/// Localiza el primer campo de firma de un documento.
pub fn find_signature_field(doc: &PdfDocument) -> Result<Option<PdfSignatureField>, String> {
    let catalog = doc.catalog()?;
    let cat = catalog.dict().ok_or("PDF: catálogo inválido")?;
    let acro = match cat.get(&b"AcroForm".to_vec()) {
        Some(a) => doc.resolve(a)?,
        None => return Ok(None),
    };
    let acro_dict = acro.dict().ok_or("PDF: /AcroForm inválido")?;
    let fields = match acro_dict.get(&b"Fields".to_vec()) {
        Some(PdfObject::Array(items)) => items.clone(),
        _ => return Ok(None),
    };
    for f in fields {
        if let Some(info) = inspect_field(doc, &f, 0)? {
            return Ok(Some(info));
        }
    }
    Ok(None)
}

/// Inspecciona un campo (recursivamente por `/Kids`).
fn inspect_field(
    doc: &PdfDocument,
    field: &PdfObject,
    depth: usize,
) -> Result<Option<PdfSignatureField>, String> {
    if depth > 4 {
        return Ok(None);
    }
    let number = match field {
        PdfObject::Ref(n, _) => *n,
        _ => return Ok(None),
    };
    let obj = doc.resolve(field)?;
    let dict = match obj.dict() {
        Some(d) => d.clone(),
        None => return Ok(None),
    };
    let ft = dict.get(&b"FT".to_vec()).and_then(|v| v.name());
    if ft == Some(b"Sig".as_slice()) {
        let name = dict
            .get(&b"T".to_vec())
            .map(|v| match v {
                PdfObject::Literal(b) => String::from_utf8_lossy(b).to_string(),
                PdfObject::Name(n) => String::from_utf8_lossy(n).to_string(),
                _ => String::new(),
            })
            .unwrap_or_default();
        let signature_number = match dict.get(&b"V".to_vec()) {
            Some(PdfObject::Ref(n, _)) => Some(*n),
            _ => None,
        };
        return Ok(Some(PdfSignatureField {
            field_number: number,
            signature_number,
            name,
        }));
    }
    if let Some(PdfObject::Array(kids)) = dict.get(&b"Kids".to_vec()) {
        for kid in kids.clone() {
            if let Some(info) = inspect_field(doc, &kid, depth + 1)? {
                return Ok(Some(info));
            }
        }
    }
    Ok(None)
}

/// Número de objeto de la primera página del documento.
fn find_first_page(doc: &PdfDocument) -> Result<Option<u32>, String> {
    let catalog = doc.catalog()?;
    let cat = catalog.dict().ok_or("PDF: catálogo inválido")?;
    let pages = doc.resolve(cat.get(&b"Pages".to_vec()).ok_or("PDF: no /Pages")?)?;
    let pages_dict = pages.dict().ok_or("PDF: /Pages inválido")?;
    let mut kids = match pages_dict.get(&b"Kids".to_vec()) {
        Some(PdfObject::Array(items)) => items.clone(),
        _ => return Ok(None),
    };
    for _ in 0..16 {
        let first = match kids.first() {
            Some(f) => f.clone(),
            None => return Ok(None),
        };
        let obj = doc.resolve(&first)?;
        let dict = match obj.dict() {
            Some(d) => d.clone(),
            None => return Ok(None),
        };
        let is_page = dict.get(&b"Type".to_vec()).and_then(|t| t.name()) == Some(b"Page".as_slice());
        if is_page {
            return Ok(match first {
                PdfObject::Ref(n, _) => Some(n),
                _ => None,
            });
        }
        match dict.get(&b"Kids".to_vec()) {
            Some(PdfObject::Array(items)) => kids = items.clone(),
            _ => return Ok(None),
        }
    }
    Ok(None)
}

/// Busca un byte patrono a partir de una posición.
fn pdf_find_from(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    let start = from.min(haystack.len());
    haystack[start..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + start)
}

/// Localiza las regiones (inicio, fin) de los 4 números del `/ByteRange`.
///
/// La región de cada número incluye el espacio que lo precede, de forma que
/// se puede sustituir por un valor más largo alineado a la derecha sin
/// cambiar la longitud del fichero.
fn find_byte_range_regions(pdf: &[u8], from: usize) -> Result<[(usize, usize); 4], String> {
    let key_pos = pdf_find_from(pdf, b"/ByteRange", from)
        .ok_or_else(|| "PDF: no se encontró la clave /ByteRange".to_string())?;
    let open = pdf[key_pos..]
        .iter()
        .position(|&c| c == b'[')
        .map(|i| key_pos + i)
        .ok_or_else(|| "PDF: /ByteRange sin '['".to_string())?;
    let close = pdf[open..]
        .iter()
        .position(|&c| c == b']')
        .map(|i| open + i)
        .ok_or_else(|| "PDF: /ByteRange sin ']'".to_string())?;
    let mut regions = [(0usize, 0usize); 4];
    let mut idx = 0usize;
    let mut i = open + 1;
    let mut prev_end = open + 1;
    while i < close && idx < 4 {
        if pdf[i].is_ascii_digit() {
            while i < close && pdf[i].is_ascii_digit() {
                i += 1;
            }
            regions[idx] = (prev_end, i);
            prev_end = i;
            idx += 1;
        } else {
            i += 1;
        }
    }
    if idx != 4 {
        return Err(format!(
            "PDF: /ByteRange tiene {} números (se esperaban 4)",
            idx
        ));
    }
    Ok(regions)
}

/// Sustituye los números del `/ByteRange` conservando la longitud del fichero.
fn patch_byte_range(pdf: &mut [u8], regions: &[(usize, usize); 4], values: [usize; 4]) -> Result<(), String> {
    for (i, (start, end)) in regions.iter().enumerate() {
        let width = end - start;
        let text = format!("{:>width$}", values[i], width = width);
        if text.trim().is_empty() || text.trim().len() > width {
            return Err("PDF: no cabe el /ByteRange en el hueco disponible".to_string());
        }
        pdf[*start..*end].copy_from_slice(text.as_bytes());
    }
    Ok(())
}

/// Escribe los dígitos hexadecimales de la firma en el hueco de `/Contents`.
fn patch_contents(pdf: &mut [u8], hex_offset: usize, reserved: usize, cms: &[u8]) -> Result<(), String> {
    if cms.len() > reserved {
        return Err(format!(
            "la firma ({}) no cabe en el hueco reservado ({})",
            cms.len(),
            reserved
        ));
    }
    let hex = hex_encode(cms);
    let zeros = reserved * 2 - hex.len();
    // La firma ocupa el principio del valor y el resto se rellena con ceros.
    let mut padded = String::with_capacity(reserved * 2);
    padded.push_str(&hex);
    padded.push_str(&"0".repeat(zeros));
    pdf[hex_offset..hex_offset + reserved * 2].copy_from_slice(padded.as_bytes());
    Ok(())
}

/// Cuerpo del widget/campo de firma.
fn pdf_field_body(
    field_name: &str,
    page_number: u32,
    signature_number: u32,
    rect: [i64; 4],
) -> Vec<u8> {
    format!(
        "<< /Type /Annot /Subtype /Widget /FT /Sig /T ({}) /Rect [{} {} {} {}] /F 4 /P {} 0 R /V {} 0 R /DA (/Helv 0 Tf 0 g) >>",
        pdf_escape(field_name),
        rect[0],
        rect[1],
        rect[2],
        rect[3],
        page_number,
        signature_number
    )
    .into_bytes()
}

/// Genera el texto de un PDF con los mismos objetos que el original pero
/// con el `ByteRange` y la firma ya incrustados.
pub fn sign_pdf(
    pdf: &[u8],
    provider: &dyn SignatureProvider,
    options: &PdfSignOptions,
) -> Result<PdfSignResult, String> {
    let doc = PdfDocument::parse(pdf)?;
    let field = find_signature_field(&doc)?;
    let date = pdf_date(unix_now());

    // --- Camino 1: el documento ya tiene un campo con /Contents ---
    if let Some(f) = &field {
        if let Some(sig_num) = f.signature_number {
            let gen = match doc.xref_entry_gen(sig_num) {
                Some(g) => g,
                None => 0,
            };
            if let Ok(sig_obj) = doc.load_object(sig_num) {
                if let Some(dict) = sig_obj.dict() {
                    if let Some(PdfObject::Hex(_, hex_off, hex_len)) = dict.get(&b"Contents".to_vec())
                    {
                        let sig_offset = doc.xref_entry_offset(sig_num).unwrap_or(0);
                        if let Ok(regions) = find_byte_range_regions(pdf, sig_offset) {
                            let reserved = hex_len / 2;
                            if reserved >= 1024 {
                                if let Ok(result) = sign_in_place(
                                    pdf,
                                    provider,
                                    options,
                                    *hex_off,
                                    *hex_len,
                                    reserved,
                                    regions,
                                    &f.name,
                                ) {
                                    return Ok(result);
                                }
                            }
                        }
                    }
                }
            }
            let _ = gen;
        }
    }

    // --- Camino 2: actualización incremental ---
    sign_incremental(pdf, &doc, provider, options, field.as_ref(), &date)
}

/// Firma "in situ": rellena el `/Contents` existente y actualiza el
/// `/ByteRange` sin tocar la longitud del fichero.
fn sign_in_place(
    pdf: &[u8],
    provider: &dyn SignatureProvider,
    options: &PdfSignOptions,
    hex_offset: usize,
    hex_len: usize,
    reserved: usize,
    regions: [(usize, usize); 4],
    field_name: &str,
) -> Result<PdfSignResult, String> {
    let total = pdf.len();
    let gap_start = hex_offset.saturating_sub(1); // incluye '<'
    let gap_end = (hex_offset + hex_len + 1).min(total); // incluye '>'
    let values = [0usize, gap_start, gap_end, total - gap_end];
    let mut buffer = pdf.to_vec();
    patch_byte_range(&mut buffer, &regions, values)?;
    let signed_content = extract_signed_content(&buffer, values)?;
    let digest = sha256(&signed_content);
    let cms = build_cms_detached_signed_data(provider, &digest, &options.cms_options)?;
    patch_contents(&mut buffer, hex_offset, reserved, &cms)?;
    Ok(PdfSignResult {
        bytes: buffer,
        byte_range: values,
        cms_der: cms,
        digest,
        signed_content,
        field_name: field_name.to_string(),
    })
}

/// Extrae los bytes firmados a partir del `/ByteRange`.
pub fn extract_signed_content(pdf: &[u8], byte_range: [usize; 4]) -> Result<Vec<u8>, String> {
    let (a, b, c, d) = (byte_range[0], byte_range[1], byte_range[2], byte_range[3]);
    if b > pdf.len() || c > pdf.len() || c + d > pdf.len() {
        return Err("PDF: /ByteRange fuera de los límites del fichero".to_string());
    }
    let mut out = Vec::with_capacity(b - a + d);
    out.extend_from_slice(&pdf[a..b]);
    out.extend_from_slice(&pdf[c..c + d]);
    Ok(out)
}

/// Firma mediante actualización incremental: reserva el hueco de `/Contents`,
/// escribe el `/ByteRange` y añade el diccionario PAdES.
fn sign_incremental(
    pdf: &[u8],
    doc: &PdfDocument,
    provider: &dyn SignatureProvider,
    options: &PdfSignOptions,
    field: Option<&PdfSignatureField>,
    date: &str,
) -> Result<PdfSignResult, String> {
    let (sig_body, br_offsets, contents_rel) = signature_dict_body(options, date);
    let max_obj = doc.max_object_number();
    let mut objects: Vec<(u32, u16, Vec<u8>)> = Vec::new();

    let (sig_number, field_override) = match field {
        Some(f) if f.signature_number.is_some() => (f.signature_number.unwrap(), None),
        Some(f) => {
            let sig_num = max_obj + 1;
            let body = pdf_field_body(
                &options.field_name,
                find_first_page(doc)?.unwrap_or(1),
                sig_num,
                [72, 690, 320, 750],
            );
            (sig_num, Some((f.field_number, body)))
        }
        None => {
            let sig_num = max_obj + 1;
            let field_num = max_obj + 2;
            let body = pdf_field_body(
                &options.field_name,
                find_first_page(doc)?.unwrap_or(1),
                sig_num,
                [72, 690, 320, 750],
            );
            (sig_num, Some((field_num, body)))
        }
    };
    let sig_gen = doc.xref_entry_gen(sig_number).unwrap_or(0);
    objects.push((sig_number, sig_gen, sig_body));

    // Si hay que crear o reutilizar el campo, actualizamos catálogo y página.
    let mut extra: Vec<(u32, u16, Vec<u8>)> = Vec::new();
    if let Some((field_num, field_body)) = field_override {
        extra.push((field_num, doc.xref_entry_gen(field_num).unwrap_or(0), field_body));
        // Catálogo: se añade /AcroForm con el campo nuevo.
        if let Ok(catalog) = doc.catalog() {
            if let Some(cat) = catalog.dict() {
                let mut new_cat = cat.clone();
                let mut fields: Vec<PdfObject> = Vec::new();
                if let Some(acro) = cat.get(&b"AcroForm".to_vec()) {
                    if let Ok(resolved) = doc.resolve(acro) {
                        if let Some(ad) = resolved.dict() {
                            if let Some(PdfObject::Array(items)) = ad.get(&b"Fields".to_vec()) {
                                fields.extend(items.iter().cloned());
                            }
                        }
                    }
                }
                fields.push(PdfObject::Ref(field_num, 0));
                let acro_form = PdfObject::Dict(
                    [
                        (b"Fields".to_vec(), PdfObject::Array(fields)),
                        (b"SigFlags".to_vec(), PdfObject::Number(3.0)),
                        (b"DA".to_vec(), PdfObject::Literal(b"(/Helv 0 Tf 0 g)".to_vec())),
                    ]
                    .into_iter()
                    .collect(),
                );
                new_cat.insert(b"AcroForm".to_vec(), acro_form);
                extra.push((
                    doc.root_object(),
                    doc.xref_entry_gen(doc.root_object()).unwrap_or(0),
                    pdf_write_object(&PdfObject::Dict(new_cat)),
                ));
                // Página: se añade el widget a /Annots.
                if let Some(page_num) = find_first_page(doc)? {
                    if let Ok(page) = doc.load_object(page_num) {
                        if let Some(pd) = page.dict() {
                            let mut new_page = pd.clone();
                            let mut annots: Vec<PdfObject> = Vec::new();
                            if let Some(PdfObject::Array(items)) = pd.get(&b"Annots".to_vec()) {
                                annots.extend(items.iter().cloned());
                            }
                            annots.push(PdfObject::Ref(field_num, 0));
                            new_page.insert(b"Annots".to_vec(), PdfObject::Array(annots));
                            extra.push((
                                page_num,
                                doc.xref_entry_gen(page_num).unwrap_or(0),
                                pdf_write_object(&PdfObject::Dict(new_page)),
                            ));
                        }
                    }
                }
            }
        }
    }

    // --- Ensamblado del fichero ---
    let mut out = pdf.to_vec();
    let mut written: Vec<(u32, u16, usize)> = Vec::new();
    let mut sig_base = 0usize;
    for (num, gen, body) in objects.iter().chain(extra.iter()) {
        let offset = out.len();
        out.extend_from_slice(format!("{} {} obj\n", num, gen).as_bytes());
        // El cuerpo (y no la cabecera) es lo que contiene los offsets.
        if *num == sig_number {
            sig_base = out.len();
        }
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
        written.push((*num, *gen, offset));
    }

    // La xref debe incluir la nueva entrada del propio objeto xref si es stream.
    let xref_offset = out.len();
    let max_written = written.iter().map(|w| w.0).max().unwrap_or(0);
    let size = doc.size().max(max_written + 1);
    let info = doc.trailer_info();
    if doc.has_classic_xref() {
        write_classic_xref(&mut out, &written, size, doc.root_object(), doc.startxref(), info);
    } else {
        write_xref_stream(
            &mut out,
            &written,
            size,
            doc.root_object(),
            doc.startxref(),
            info,
        );
    }
    out.extend_from_slice(format!("startxref\n{}\n%%EOF\n", xref_offset).as_bytes());

    // --- Cálculo del ByteRange y firma ---
    let contents_offset = sig_base + contents_rel;
    let regions = [
        (sig_base + br_offsets[0], sig_base + br_offsets[0] + BR_FIELD_WIDTH),
        (sig_base + br_offsets[1], sig_base + br_offsets[1] + BR_FIELD_WIDTH),
        (sig_base + br_offsets[2], sig_base + br_offsets[2] + BR_FIELD_WIDTH),
        (sig_base + br_offsets[3], sig_base + br_offsets[3] + BR_FIELD_WIDTH),
    ];
    let total = out.len();
    let gap_start = contents_offset.saturating_sub(1);
    let gap_end = (contents_offset + options.reserved_signature_bytes * 2 + 1).min(total);
    let values = [0usize, gap_start, gap_end, total - gap_end];
    patch_byte_range(&mut out, &regions, values)?;
    let signed_content = extract_signed_content(&out, values)?;
    let digest = sha256(&signed_content);
    let cms = build_cms_detached_signed_data(provider, &digest, &options.cms_options)?;
    patch_contents(
        &mut out,
        contents_offset,
        options.reserved_signature_bytes,
        &cms,
    )?;
    Ok(PdfSignResult {
        bytes: out,
        byte_range: values,
        cms_der: cms,
        digest,
        signed_content,
        field_name: options.field_name.clone(),
    })
}

/// Serializa las entradas de un xref stream (1 byte de tipo, 4 de offset,
/// 2 de generación) y devuelve también los rangos del índice.
fn build_xref_stream_data(
    sorted: &[(u32, u16, usize)],
    offsets: &HashMap<u32, usize>,
) -> (Vec<u32>, Vec<u8>) {
    let mut index: Vec<u32> = Vec::new();
    let mut data: Vec<u8> = Vec::new();
    let mut i = 0usize;
    while i < sorted.len() {
        let start = sorted[i].0;
        let mut count = 1u32;
        while (i + count as usize) < sorted.len() && sorted[i + count as usize].0 == start + count {
            count += 1;
        }
        index.push(start);
        index.push(count);
        for e in &sorted[i..i + count as usize] {
            let off = offsets.get(&e.0).copied().unwrap_or(e.2);
            data.push(1);
            data.extend_from_slice(&(off as u32).to_be_bytes());
            data.extend_from_slice(&e.1.to_be_bytes());
        }
        i += count as usize;
    }
    (index, data)
}

/// Escribe una sección xref en forma de stream (PDF 1.5+).
fn write_xref_stream(
    out: &mut Vec<u8>,
    entries: &[(u32, u16, usize)],
    size: u32,
    root: u32,
    prev: usize,
    info: Option<u32>,
) {
    let xref_number = size;
    let mut sorted: Vec<(u32, u16, usize)> = entries.to_vec();
    sorted.push((xref_number, 0, 0));
    sorted.sort_by_key(|e| e.0);

    // Los datos se construyen primero con offsets provisionales: la longitud
    // del stream no depende del valor del offset, así que el tamaño final es
    // estable y se puede calcular la posición real del objeto xref.
    let (index, provisional) = build_xref_stream_data(&sorted, &HashMap::new());
    let mut dict = format!(
        "<< /Type /XRef /Size {} /W [1 4 2] /Root {} 0 R /Prev {} ",
        size + 1,
        root,
        prev
    );
    if let Some(i) = info {
        dict.push_str(&format!("/Info {} 0 R ", i));
    }
    dict.push_str("/Index [");
    for (n, v) in index.iter().enumerate() {
        if n > 0 {
            dict.push(' ');
        }
        dict.push_str(&v.to_string());
    }
    dict.push_str("] /Length ");
    dict.push_str(&provisional.len().to_string());
    dict.push_str(" >>\nstream\n");

    let header = format!("{} 0 obj\n", xref_number);
    let offset = out.len() + header.len() + dict.len();
    let mut offsets = HashMap::new();
    offsets.insert(xref_number, offset);
    let (_index2, data) = build_xref_stream_data(&sorted, &offsets);
    debug_assert_eq!(data.len(), provisional.len());

    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(dict.as_bytes());
    out.extend_from_slice(&data);
    out.extend_from_slice(b"\nendstream\nendobj\n");
}

/// Verifica la firma de un PDF: extrae el CMS del `/Contents`, reconstruye los
/// bytes firmados según el `/ByteRange` y comprueba la firma.
pub fn verify_pdf_signature(pdf: &[u8], cert_der: &[u8]) -> Result<bool, String> {
    let doc = PdfDocument::parse(pdf)?;
    let field = find_signature_field(&doc)?
        .ok_or_else(|| "PDF: el documento no tiene campo de firma".to_string())?;
    let sig_num = field
        .signature_number
        .ok_or_else(|| "PDF: el campo de firma no tiene diccionario /V".to_string())?;
    let sig = doc.load_object(sig_num)?;
    let dict = sig.dict().ok_or("PDF: /V no es un diccionario".to_string())?;
    let contents = match dict.get(&b"Contents".to_vec()) {
        Some(PdfObject::Hex(bytes, _, _)) => bytes.clone(),
        _ => return Err("PDF: /Contents no es una cadena hexadecimal".to_string()),
    };
    let range = match dict.get(&b"ByteRange".to_vec()) {
        Some(PdfObject::Array(items)) => {
            let mut values = [0usize; 4];
            for (i, item) in items.iter().take(4).enumerate() {
                values[i] = item.as_int().unwrap_or(0).max(0) as usize;
            }
            values
        }
        _ => return Err("PDF: falta el /ByteRange".to_string()),
    };
    let signed = extract_signed_content(pdf, range)?;
    verify_cms(&contents, &signed, cert_der)
}

// ===========================================================================
// 12. PKCS#11 (OpenSC) con FFI dinámico
// ===========================================================================

use std::ffi::{c_void, CString};

/// `CK_RV` (unsigned long en la especificación).
type CkRv = libc::c_ulong;
/// `CK_ULONG`.
type CkUlong = libc::c_ulong;
/// `CK_SLOT_ID`.
type CkSlotId = libc::c_ulong;
/// `CK_SESSION_HANDLE`.
type CkSessionHandle = libc::c_ulong;
/// `CK_OBJECT_HANDLE`.
type CkObjectHandle = libc::c_ulong;
/// `CK_USER_TYPE`.
type CkUserType = libc::c_ulong;
/// `CK_FLAGS`.
type CkFlags = libc::c_ulong;
/// `CK_ATTRIBUTE_TYPE`.
type CkAttributeType = libc::c_ulong;
/// `CK_MECHANISM_TYPE`.
type CkMechanismType = libc::c_ulong;
/// `CK_BYTE`.
type CkByte = u8;
/// `CK_BBOOL`.
type CkBool = u8;
/// `CK_VOID_PTR`.
type CkVoidPtr = *mut c_void;

/// `CK_ATTRIBUTE`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CkAttribute {
    pub attr_type: CkAttributeType,
    pub p_value: CkVoidPtr,
    pub ul_value_len: CkUlong,
}

/// `CK_MECHANISM`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CkMechanism {
    pub mechanism: CkMechanismType,
    pub p_parameter: CkVoidPtr,
    pub ul_parameter_len: CkUlong,
}

type CInitializeFn = unsafe extern "C" fn(CkVoidPtr) -> CkRv;
type CFinalizeFn = unsafe extern "C" fn(CkVoidPtr) -> CkRv;
type CGetSlotListFn = unsafe extern "C" fn(CkBool, *mut CkSlotId, *mut CkUlong) -> CkRv;
type COpenSessionFn =
    unsafe extern "C" fn(CkSlotId, CkFlags, CkVoidPtr, *mut c_void, *mut CkSessionHandle) -> CkRv;
type CCloseSessionFn = unsafe extern "C" fn(CkSessionHandle) -> CkRv;
type CLoginFn = unsafe extern "C" fn(CkSessionHandle, CkUserType, *const CkByte, CkUlong) -> CkRv;
type CFindObjectsInitFn = unsafe extern "C" fn(CkSessionHandle, *mut CkAttribute, CkUlong) -> CkRv;
type CFindObjectsFn =
    unsafe extern "C" fn(CkSessionHandle, *mut CkObjectHandle, CkUlong, *mut CkUlong) -> CkRv;
type CFindObjectsFinalFn = unsafe extern "C" fn(CkSessionHandle) -> CkRv;
type CGetAttributeValueFn =
    unsafe extern "C" fn(CkSessionHandle, CkObjectHandle, *mut CkAttribute, CkUlong) -> CkRv;
type CSignInitFn = unsafe extern "C" fn(CkSessionHandle, *mut CkMechanism) -> CkRv;
type CSignFn =
    unsafe extern "C" fn(CkSessionHandle, *const CkByte, CkUlong, *mut CkByte, *mut CkUlong) -> CkRv;

/// `CKA_CLASS`.
const CKA_CLASS: CkAttributeType = 0x0000_0000;
/// `CKA_LABEL`.
const CKA_LABEL: CkAttributeType = 0x0000_0003;
/// `CKA_ID`.
const CKA_ID: CkAttributeType = 0x0000_0002;
/// `CKA_VALUE`.
const CKA_VALUE: CkAttributeType = 0x0000_0011;
/// `CKO_CERTIFICATE`.
const CKO_CERTIFICATE: CkUlong = 1;
/// `CKM_RSA_PKCS`.
const CKM_RSA_PKCS: CkMechanismType = 0x0000_0001;
/// `CKM_ECDSA`.
const CKM_ECDSA: CkMechanismType = 0x0000_1041;
/// `CKF_SERIAL_SESSION`.
const CKF_SERIAL_SESSION: CkFlags = 0x0000_0004;
/// `CKF_RW_SESSION`.
const CKF_RW_SESSION: CkFlags = 0x0000_0002;
/// `CKU_USER`.
const CKU_USER: CkUserType = 1;
/// `CKR_USER_ALREADY_LOGGED_IN`.
const CKR_USER_ALREADY_LOGGED_IN: CkRv = 0x0000_000D;

/// Traduce un código de retorno de PKCS#11 a texto.
pub fn pkcs11_error_text(rv: CkRv) -> String {
    match rv {
        0x0000_0000 => "CKR_OK".to_string(),
        0x0000_0001 => "CKR_CANCEL".to_string(),
        0x0000_0002 => "CKR_HOST_MEMORY".to_string(),
        0x0000_0003 => "CKR_SLOT_ID_INVALID".to_string(),
        0x0000_0005 => "CKR_GENERAL_ERROR".to_string(),
        0x0000_0006 => "CKR_FUNCTION_FAILED".to_string(),
        0x0000_0007 => "CKR_ARGUMENTS_BAD".to_string(),
        0x0000_000D => "CKR_USER_ALREADY_LOGGED_IN".to_string(),
        0x0000_000E => "CKR_PIN_INCORRECT".to_string(),
        0x0000_0010 => "CKR_PIN_LOCKED".to_string(),
        0x0000_0014 => "CKR_TOKEN_NOT_PRESENT".to_string(),
        0x0000_0019 => "CKR_TOKEN_NOT_RECOGNIZED".to_string(),
        0x0000_0020 => "CKR_PIN_INCORRECT (o PIN bloqueado)".to_string(),
        0x0000_0030 => "CKR_DEVICE_ERROR".to_string(),
        0x0000_0054 => "CKR_OBJECT_HANDLE_INVALID".to_string(),
        0x0000_0060 => "CKR_OPERATION_NOT_INITIALIZED".to_string(),
        0x0000_0061 => "CKR_OPERATION_CANCEL_FAILED".to_string(),
        0x0000_0070 => "CKR_ATTRIBUTE_TYPE_INVALID".to_string(),
        0x0000_0072 => "CKR_ATTRIBUTE_VALUE_INVALID".to_string(),
        0x0000_0080 => "CKR_MECHANISM_INVALID".to_string(),
        0x0000_0081 => "CKR_MECHANISM_PARAM_INVALID".to_string(),
        0x0000_0090 => "CKR_KEY_HANDLE_INVALID".to_string(),
        0x0000_00A0 => "CKR_KEY_TYPE_INCONSISTENT".to_string(),
        0x0000_00B0 => "CKR_PIN_MIN_LENGTH".to_string(),
        0x0000_00C0 => "CKR_CRYPTOKI_ALREADY_INITIALIZED".to_string(),
        0x0000_00D0 => "CKR_CRYPTOKI_NOT_INITIALIZED".to_string(),
        0x0000_00E0 => "CKR_FUNCTION_REJECTED".to_string(),
        0x0000_0100 => "CKR_BUFFER_TOO_SMALL".to_string(),
        0x0000_0120 => "CKR_SIGNATURE_INVALID".to_string(),
        0x0000_0190 => "CKR_USER_TYPE_INVALID".to_string(),
        other => format!("CKR 0x{:08X}", other),
    }
}

/// Token PKCS#11: lista de objetos y firma.
pub trait Pkcs11Token: Send + Sync {
    /// Lista los certificados del token.
    fn list_objects(&self) -> Result<Vec<TokenObject>, String>;
    /// Firma `data` (un digest) con la clave privada del objeto indicado.
    ///
    /// RSA devuelve la firma PKCS#1 v1.5; ECDSA devuelve `r || s` (64 bytes
    /// en P-256), tal como exige `CKM_ECDSA`.
    fn sign(&self, object: &TokenObject, data: &[u8]) -> Result<Vec<u8>, String>;
}

/// Handle de la librería PKCS#11 cargada dinámicamente.
pub struct Pkcs11Library {
    handle: *mut c_void,
    c_initialize: Option<CInitializeFn>,
    c_finalize: Option<CFinalizeFn>,
    c_get_slot_list: Option<CGetSlotListFn>,
    c_open_session: Option<COpenSessionFn>,
    /// `C_CloseSession` (reservado para el cierre explícito de sesión).
    #[allow(dead_code)]
    c_close_session: Option<CCloseSessionFn>,
    c_login: Option<CLoginFn>,
    c_find_objects_init: Option<CFindObjectsInitFn>,
    c_find_objects: Option<CFindObjectsFn>,
    c_find_objects_final: Option<CFindObjectsFinalFn>,
    c_get_attribute_value: Option<CGetAttributeValueFn>,
    c_sign_init: Option<CSignInitFn>,
    c_sign: Option<CSignFn>,
}

// El handle de la librería es opaco; se sincroniza el acceso a la sesión.
unsafe impl Send for Pkcs11Library {}
unsafe impl Sync for Pkcs11Library {}

impl Pkcs11Library {
    /// Carga la librería PKCS#11 con `dlopen`.
    pub fn open(path: &str) -> Result<Pkcs11Library, String> {
        let cpath = CString::new(path).map_err(|_| "ruta PKCS#11 inválida".to_string())?;
        let handle = unsafe {
            libc::dlopen(cpath.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL)
        };
        if handle.is_null() {
            let err = unsafe { libc::dlerror() };
            let msg = if err.is_null() {
                "error desconocido".to_string()
            } else {
                unsafe { std::ffi::CStr::from_ptr(err) }
                    .to_string_lossy()
                    .to_string()
            };
            return Err(format!("no se puede cargar {path}: {msg}"));
        }
        Ok(Pkcs11Library {
            handle,
            c_initialize: unsafe { load_symbol(handle, "C_Initialize") },
            c_finalize: unsafe { load_symbol(handle, "C_Finalize") },
            c_get_slot_list: unsafe { load_symbol(handle, "C_GetSlotList") },
            c_open_session: unsafe { load_symbol(handle, "C_OpenSession") },
            c_close_session: unsafe { load_symbol(handle, "C_CloseSession") },
            c_login: unsafe { load_symbol(handle, "C_Login") },
            c_find_objects_init: unsafe { load_symbol(handle, "C_FindObjectsInit") },
            c_find_objects: unsafe { load_symbol(handle, "C_FindObjects") },
            c_find_objects_final: unsafe { load_symbol(handle, "C_FindObjectsFinal") },
            c_get_attribute_value: unsafe { load_symbol(handle, "C_GetAttributeValue") },
            c_sign_init: unsafe { load_symbol(handle, "C_SignInit") },
            c_sign: unsafe { load_symbol(handle, "C_Sign") },
        })
    }

    /// Ruta de una librería PKCS#11 conocida.
    pub fn path(&self) -> String {
        format!("{:p}", self.handle)
    }
}

impl Drop for Pkcs11Library {
    fn drop(&mut self) {
        if let Some(f) = self.c_finalize {
            unsafe {
                f(std::ptr::null_mut());
            }
        }
        if !self.handle.is_null() {
            unsafe {
                libc::dlclose(self.handle);
            }
        }
    }
}

/// Carga un símbolo de la librería.
unsafe fn load_symbol<T: Copy>(handle: *mut c_void, name: &str) -> Option<T> {
    let cname = CString::new(name).ok()?;
    let sym = libc::dlsym(handle, cname.as_ptr());
    if sym.is_null() {
        None
    } else {
        Some(std::mem::transmute_copy::<*mut c_void, T>(&sym))
    }
}

/// Rutas habituales de los módulos PKCS#11 de OpenSC.
pub const PKCS11_CANDIDATE_PATHS: &[&str] = &[
    "/usr/lib/x86_64-linux-gnu/opensc-pkcs11.so",
    "/usr/lib/aarch64-linux-gnu/opensc-pkcs11.so",
    "/usr/lib64/pkcs11/opensc-pkcs11.so",
    "/usr/lib64/opensc-pkcs11.so",
    "/usr/lib/opensc-pkcs11.so",
    "/usr/lib/softhsm/libsofthsm2.so",
    "/usr/local/lib/opensc-pkcs11.so",
    "opensc-pkcs11.so",
    "libpcsclite.so",
];

/// Busca un módulo PKCS#11 instalado en el sistema.
pub fn find_pkcs11_library() -> Option<String> {
    for candidate in PKCS11_CANDIDATE_PATHS {
        if candidate.contains('/') {
            if std::path::Path::new(candidate).exists() {
                return Some(candidate.to_string());
            }
        } else {
            // Nombre sin ruta: dlopen lo buscará en el camino del sistema.
            return Some(candidate.to_string());
        }
    }
    None
}

/// Estado de la sesión PKCS#11.
struct Pkcs11Session {
    session: CkSessionHandle,
    logged_in: bool,
}

/// Token PKCS#11 real (hardware: DNIe, tarjeta criptográfica, etc.).
pub struct Pkcs11HardwareToken {
    lib: Pkcs11Library,
    state: Mutex<Pkcs11Session>,
}

impl Pkcs11HardwareToken {
    /// Abre el token del primer slot (o del slot indicado).
    ///
    /// `pin` es el PIN del usuario; si es `None` se intenta firmar con
    /// claves que no lo requieran (por ejemplo, con un token ya iniciado
    /// de sesión por otro proceso).
    pub fn open(
        library_path: &str,
        pin: Option<&str>,
        slot_index: usize,
    ) -> Result<Pkcs11HardwareToken, String> {
        let lib = Pkcs11Library::open(library_path)?;
        lib.initialize()?;
        let slots = lib.slot_list()?;
        let slot = *slots
            .get(slot_index)
            .ok_or_else(|| format!("el token no tiene el slot {}", slot_index))?;
        let session = lib.open_session(slot)?;
        let token = Pkcs11HardwareToken {
            lib,
            state: Mutex::new(Pkcs11Session {
                session,
                logged_in: false,
            }),
        };
        if let Some(pin) = pin {
            token.login(pin)?;
        }
        Ok(token)
    }

    /// Hace login en la sesión (idempotente).
    pub fn login(&self, pin: &str) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "estado de sesión PKCS#11 envenenado".to_string())?;
        if state.logged_in {
            return Ok(());
        }
        let rv = self.lib.login(state.session, pin)?;
        if rv != 0 && rv != CKR_USER_ALREADY_LOGGED_IN {
            return Err(format!(
                "C_Login falló: {}",
                pkcs11_error_text(rv)
            ));
        }
        state.logged_in = true;
        Ok(())
    }

    /// Lista los slots disponibles.
    pub fn list_slots(library_path: &str) -> Result<Vec<CkSlotId>, String> {
        let lib = Pkcs11Library::open(library_path)?;
        lib.initialize()?;
        lib.slot_list()
    }
}

impl Pkcs11Token for Pkcs11HardwareToken {
    fn list_objects(&self) -> Result<Vec<TokenObject>, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "estado de sesión PKCS#11 envenenado".to_string())?;
        let mut template = CkAttribute {
            attr_type: CKA_CLASS,
            p_value: CKO_CERTIFICATE as *mut c_void,
            ul_value_len: std::mem::size_of::<CkUlong>() as CkUlong,
        };
        let rv = self.lib.find_objects_init(state.session, &mut template, 1);
        if rv != 0 {
            return Err(format!(
                "C_FindObjectsInit falló: {}",
                pkcs11_error_text(rv)
            ));
        }
        let mut objects = Vec::new();
        let mut handles = [0 as CkObjectHandle; 16];
        let mut count: CkUlong = 0;
        loop {
            let rv = self.lib.find_objects(state.session, handles.as_mut_ptr(), 16, &mut count);
            if rv != 0 {
                let _ = self.lib.find_objects_final(state.session);
                return Err(format!(
                    "C_FindObjects falló: {}",
                    pkcs11_error_text(rv)
                ));
            }
            if count == 0 {
                break;
            }
            for handle in handles.iter().take(count as usize) {
                if let Some(obj) = self.object_info(state.session, *handle) {
                    objects.push(obj);
                }
            }
            if (count as usize) < 16 {
                break;
            }
        }
        let _ = self.lib.find_objects_final(state.session);
        Ok(objects)
    }

    fn sign(&self, object: &TokenObject, data: &[u8]) -> Result<Vec<u8>, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "estado de sesión PKCS#11 envenenado".to_string())?;
        let algorithm = match object.key_kind.as_str() {
            "EC" => KeyAlgorithm::Ecdsa,
            "RSA" => KeyAlgorithm::RsaPkcs1,
            other => return Err(format!("tipo de clave no soportado: {}", other)),
        };
        let mut mechanism = match algorithm {
            KeyAlgorithm::RsaPkcs1 => CkMechanism {
                mechanism: CKM_RSA_PKCS,
                p_parameter: std::ptr::null_mut(),
                ul_parameter_len: 0,
            },
            KeyAlgorithm::Ecdsa => {
                // Parámetro: OID DER de la curva prime256v1 (P-256).
                let oid: [u8; 10] = [0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07];
                CkMechanism {
                    mechanism: CKM_ECDSA,
                    p_parameter: oid.as_ptr() as CkVoidPtr,
                    ul_parameter_len: oid.len() as CkUlong,
                }
            }
        };
        let rv = self.lib.sign_init(state.session, &mut mechanism);
        if rv != 0 {
            return Err(format!("C_SignInit falló: {}", pkcs11_error_text(rv)));
        }
        self.lib.sign(state.session, data)
    }
}

impl Pkcs11HardwareToken {
    /// Lee los atributos de un objeto del token.
    fn object_info(&self, session: CkSessionHandle, handle: CkObjectHandle) -> Option<TokenObject> {
        let label = self.attribute_bytes(session, handle, CKA_LABEL).unwrap_or_default();
        let id = self
            .attribute_bytes(session, handle, CKA_ID)
            .unwrap_or_default();
        let cert = self.attribute_bytes(session, handle, CKA_VALUE);
        let key_kind = match &cert {
            Some(der) => match parse_x509_certificate(der) {
                Ok(c) => match c.public_key {
                    PublicKey::Rsa { .. } => "RSA".to_string(),
                    PublicKey::Ec { .. } => "EC".to_string(),
                },
                Err(_) => "UNKNOWN".to_string(),
            },
            None => "UNKNOWN".to_string(),
        };
        Some(TokenObject {
            label: String::from_utf8_lossy(&label).trim_end_matches('\0').to_string(),
            id: hex_encode(&id),
            key_kind,
            cert_der: cert,
        })
    }

    /// Lee un atributo que puede ser de longitud variable.
    fn attribute_bytes(
        &self,
        session: CkSessionHandle,
        handle: CkObjectHandle,
        attr_type: CkAttributeType,
    ) -> Option<Vec<u8>> {
        let mut attr = CkAttribute {
            attr_type,
            p_value: std::ptr::null_mut(),
            ul_value_len: 0,
        };
        let rv = self
            .lib
            .get_attribute_value(session, handle, &mut attr, 1);
        if rv != 0 || attr.ul_value_len == 0 || attr.ul_value_len == CkUlong::MAX {
            return None;
        }
        let mut buffer = vec![0u8; attr.ul_value_len as usize];
        attr.p_value = buffer.as_mut_ptr() as CkVoidPtr;
        let rv = self
            .lib
            .get_attribute_value(session, handle, &mut attr, 1);
        if rv != 0 {
            return None;
        }
        let len = (attr.ul_value_len as usize).min(buffer.len());
        buffer.truncate(len);
        Some(buffer)
    }
}

impl Pkcs11Library {
    fn initialize(&self) -> Result<(), String> {
        let f = self
            .c_initialize
            .ok_or_else(|| "la librería PKCS#11 no exporta C_Initialize".to_string())?;
        let rv = unsafe { f(std::ptr::null_mut()) };
        if rv != 0 && rv != 0x0000_00C1 {
            return Err(format!(
                "C_Initialize falló: {}",
                pkcs11_error_text(rv)
            ));
        }
        Ok(())
    }

    fn slot_list(&self) -> Result<Vec<CkSlotId>, String> {
        let f = self
            .c_get_slot_list
            .ok_or_else(|| "la librería PKCS#11 no exporta C_GetSlotList".to_string())?;
        let mut count: CkUlong = 0;
        let rv = unsafe { f(0, std::ptr::null_mut(), &mut count) };
        if rv != 0 {
            return Err(format!("C_GetSlotList falló: {}", pkcs11_error_text(rv)));
        }
        let mut slots = vec![0 as CkSlotId; count as usize];
        let mut count2 = count;
        let rv = unsafe { f(0, slots.as_mut_ptr(), &mut count2) };
        if rv != 0 {
            return Err(format!("C_GetSlotList falló: {}", pkcs11_error_text(rv)));
        }
        slots.truncate(count2 as usize);
        Ok(slots)
    }

    fn open_session(&self, slot: CkSlotId) -> Result<CkSessionHandle, String> {
        let f = self
            .c_open_session
            .ok_or_else(|| "la librería PKCS#11 no exporta C_OpenSession".to_string())?;
        let mut session: CkSessionHandle = 0;
        let rv = unsafe {
            f(
                slot,
                CKF_SERIAL_SESSION | CKF_RW_SESSION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut session,
            )
        };
        if rv != 0 {
            return Err(format!("C_OpenSession falló: {}", pkcs11_error_text(rv)));
        }
        Ok(session)
    }

    fn login(&self, session: CkSessionHandle, pin: &str) -> Result<CkRv, String> {
        let f = self
            .c_login
            .ok_or_else(|| "la librería PKCS#11 no exporta C_Login".to_string())?;
        let bytes = pin.as_bytes();
        let rv = unsafe {
            f(
                session,
                CKU_USER,
                bytes.as_ptr() as *const CkByte,
                bytes.len() as CkUlong,
            )
        };
        Ok(rv)
    }

    fn find_objects_init(
        &self,
        session: CkSessionHandle,
        template: &mut CkAttribute,
        count: CkUlong,
    ) -> CkRv {
        match self.c_find_objects_init {
            Some(f) => unsafe { f(session, template, count) },
            None => 0x0000_0006,
        }
    }

    fn find_objects(
        &self,
        session: CkSessionHandle,
        handles: *mut CkObjectHandle,
        max: CkUlong,
        count: *mut CkUlong,
    ) -> CkRv {
        match self.c_find_objects {
            Some(f) => unsafe { f(session, handles, max, count) },
            None => 0x0000_0006,
        }
    }

    fn find_objects_final(&self, session: CkSessionHandle) -> Result<(), String> {
        if let Some(f) = self.c_find_objects_final {
            unsafe {
                f(session);
            }
        }
        Ok(())
    }

    fn get_attribute_value(
        &self,
        session: CkSessionHandle,
        handle: CkObjectHandle,
        attr: &mut CkAttribute,
        count: CkUlong,
    ) -> CkRv {
        match self.c_get_attribute_value {
            Some(f) => unsafe { f(session, handle, attr, count) },
            None => 0x0000_0006,
        }
    }

    fn sign_init(&self, session: CkSessionHandle, mechanism: &mut CkMechanism) -> CkRv {
        match self.c_sign_init {
            Some(f) => unsafe { f(session, mechanism) },
            None => 0x0000_0054,
        }
    }

    fn sign(
        &self,
        session: CkSessionHandle,
        data: &[u8],
    ) -> Result<Vec<u8>, String> {
        let f = self
            .c_sign
            .ok_or_else(|| "la librería PKCS#11 no exporta C_Sign".to_string())?;
        let mut buffer = vec![0u8; 1024];
        let mut len: CkUlong = buffer.len() as CkUlong;
        let rv = unsafe {
            f(
                session,
                data.as_ptr(),
                data.len() as CkUlong,
                buffer.as_mut_ptr(),
                &mut len,
            )
        };
        if rv != 0 {
            // Algunos módulos devuelven CKR_BUFFER_TOO_SMALL indicando el
            // tamaño necesario; se reintenta con ese tamaño.
            if rv == 0x0000_0100 {
                buffer = vec![0u8; len as usize];
                let mut len2: CkUlong = buffer.len() as CkUlong;
                let rv2 = unsafe {
                    f(
                        session,
                        data.as_ptr(),
                        data.len() as CkUlong,
                        buffer.as_mut_ptr(),
                        &mut len2,
                    )
                };
                if rv2 != 0 {
                    return Err(format!("C_Sign falló: {}", pkcs11_error_text(rv2)));
                }
                buffer.truncate(len2 as usize);
                return Ok(buffer);
            }
            return Err(format!("C_Sign falló: {}", pkcs11_error_text(rv)));
        }
        buffer.truncate(len as usize);
        Ok(buffer)
    }
}

/// Token software: usa las primitivas nativas de este módulo.
#[derive(Clone, Debug)]
pub struct SoftwareToken {
    key: PrivateKey,
    cert_der: Vec<u8>,
    label: String,
    id: String,
}

impl SoftwareToken {
    /// Construye el token a partir de una clave PEM y un certificado PEM.
    pub fn from_pem(private_pem: &str, cert_pem: &str) -> Result<SoftwareToken, String> {
        let key = PrivateKey::from_pem(private_pem)?;
        let (_label, der) = pem_decode(cert_pem)?;
        Ok(SoftwareToken {
            key,
            cert_der: der,
            label: "software".to_string(),
            id: String::new(),
        })
    }

    /// Construye el token a partir de la clave y el certificado en DER.
    pub fn from_parts(key: PrivateKey, cert_der: Vec<u8>) -> SoftwareToken {
        SoftwareToken {
            key,
            cert_der,
            label: "software".to_string(),
            id: String::new(),
        }
    }

    /// Certificado DER del token.
    pub fn certificate_der(&self) -> &[u8] {
        &self.cert_der
    }

    /// Firma el digest en el formato crudo de PKCS#11 (RSA: PKCS#1 v1.5;
    /// ECDSA: `r || s`).
    pub fn sign_digest_raw(&self, digest: &[u8]) -> Result<Vec<u8>, String> {
        self.key.sign_digest_raw(digest)
    }

    /// Firma el digest en DER canónico (el que espera el CMS/CAdES).
    pub fn sign_digest(&self, digest: &[u8]) -> Result<Vec<u8>, String> {
        self.key.sign_digest(digest)
    }
}

impl Pkcs11Token for SoftwareToken {
    fn list_objects(&self) -> Result<Vec<TokenObject>, String> {
        let key_kind = match self.key.algorithm() {
            KeyAlgorithm::RsaPkcs1 => "RSA",
            KeyAlgorithm::Ecdsa => "EC",
        };
        Ok(vec![TokenObject {
            label: self.label.clone(),
            id: self.id.clone(),
            key_kind: key_kind.to_string(),
            cert_der: Some(self.cert_der.clone()),
        }])
    }

    fn sign(&self, _object: &TokenObject, data: &[u8]) -> Result<Vec<u8>, String> {
        self.key.sign_digest_raw(data)
    }
}

/// Firma usando un token PKCS#11 (hardware o software).
pub struct TokenSigner<'a> {
    token: &'a dyn Pkcs11Token,
    object: TokenObject,
    cert_der: Vec<u8>,
    algorithm: KeyAlgorithm,
}

impl<'a> TokenSigner<'a> {
    /// Prepara un firmante con el primer objeto del token que encaje.
    pub fn select_first(token: &'a dyn Pkcs11Token) -> Result<TokenSigner<'a>, String> {
        let objects = token.list_objects()?;
        let object = objects
            .into_iter()
            .find(|o| o.cert_der.is_some())
            .ok_or_else(|| "el token no contiene certificados".to_string())?;
        TokenSigner::new(token, object)
    }

    /// Prepara un firmante con un objeto concreto.
    pub fn new(token: &'a dyn Pkcs11Token, object: TokenObject) -> Result<TokenSigner<'a>, String> {
        let cert_der = object
            .cert_der
            .clone()
            .ok_or_else(|| "el objeto del token no tiene certificado".to_string())?;
        let cert = parse_x509_certificate(&cert_der)?;
        let algorithm = match cert.public_key {
            PublicKey::Rsa { .. } => KeyAlgorithm::RsaPkcs1,
            PublicKey::Ec { .. } => KeyAlgorithm::Ecdsa,
        };
        Ok(TokenSigner {
            token,
            object,
            cert_der,
            algorithm,
        })
    }
}

impl SignatureProvider for TokenSigner<'_> {
    fn certificate_der(&self) -> Result<Vec<u8>, String> {
        Ok(self.cert_der.clone())
    }

    fn signature_algorithm(&self) -> KeyAlgorithm {
        self.algorithm
    }

    fn sign_digest(&self, digest: &[u8]) -> Result<Vec<u8>, String> {
        let raw = self.token.sign(&self.object, digest)?;
        match self.algorithm {
            KeyAlgorithm::RsaPkcs1 => Ok(raw),
            KeyAlgorithm::Ecdsa => {
                // PKCS#11 devuelve r || s; el CMS exige DER.
                if raw.len() == 64 {
                    let r = BigUint::from_bytes_be(&raw[..32]);
                    let s = BigUint::from_bytes_be(&raw[32..]);
                    Ok(EcdsaSignature { r, s }.to_der())
                } else if raw.len() > 64 {
                    // Algunos módulos devuelven DER directamente.
                    Ok(raw)
                } else {
                    Err(format!(
                        "firma ECDSA con longitud inesperada: {} bytes",
                        raw.len()
                    ))
                }
            }
        }
    }
}

// ===========================================================================
// 13. Protocolo `afirma://`
// ===========================================================================

/// Descodifica percent-encoding (y `+` como espacio).
pub fn url_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hi = (bytes[i + 1] as char).to_digit(16);
                let lo = (bytes[i + 2] as char).to_digit(16);
                match (hi, lo) {
                    (Some(h), Some(l)) => {
                        out.push(((h << 4) | l) as u8);
                        i += 3;
                    }
                    _ => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Parámetros de una operación `afirma://`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AfirmaParams {
    /// Operación (`sign`, `selectcert`, `service`, ...).
    pub operation: String,
    /// Parámetros de la query, ya decodificados.
    pub params: HashMap<String, String>,
}

impl AfirmaParams {
    /// Valor de un parámetro.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.params.get(key).map(|s| s.as_str())
    }
}

/// Analiza una URL `afirma://operacion?param=valor&...`.
pub fn parse_afirma_url(url: &str) -> Result<AfirmaParams, String> {
    let rest = url
        .strip_prefix("afirma://")
        .or_else(|| url.strip_prefix("afirma:"))
        .ok_or_else(|| format!("URL no válida (se esperaba afirma://): {}", url))?;
    let (op_raw, query) = match rest.split_once('?') {
        Some((o, q)) => (o, q),
        None => (rest, ""),
    };
    let operation = op_raw.trim_end_matches('/').to_string();
    let mut params = HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = match pair.split_once('=') {
            Some((k, v)) => (k, v),
            None => (pair, ""),
        };
        params.insert(url_decode(k), url_decode(v));
    }
    Ok(AfirmaParams {
        operation,
        params,
    })
}

/// Extrae el primer valor textual de una lista de alias.
fn pick_str(obj: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(v) = obj.get(*key) {
            match v {
                Value::String(s) => return Some(s.clone()),
                Value::Number(n) => return Some(n.to_string()),
                _ => {}
            }
        }
    }
    None
}

/// Extrae bytes de una clave (Base64, PEM o array de números).
fn pick_bytes(obj: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<Vec<u8>> {
    for key in keys {
        if let Some(v) = obj.get(*key) {
            match v {
                Value::String(s) => {
                    if s.trim().is_empty() {
                        continue;
                    }
                    if let Ok(bytes) = base64_decode_pem_or_raw(s) {
                        if !bytes.is_empty() {
                            return Some(bytes);
                        }
                    }
                }
                Value::Array(items) => {
                    let mut bytes = Vec::with_capacity(items.len());
                    for item in items {
                        if let Some(n) = item.as_u64() {
                            bytes.push(n as u8);
                        }
                    }
                    if !bytes.is_empty() {
                        return Some(bytes);
                    }
                }
                _ => {}
            }
        }
    }
    None
}

/// Analiza la petición de firma que envía la sede electrónica.
pub fn parse_signature_request(json: &str) -> Result<SignatureRequest, String> {
    let value: Value =
        serde_json::from_str(json).map_err(|e| format!("JSON inválido: {}", e))?;
    let obj = value
        .as_object()
        .ok_or_else(|| "la petición de firma debe ser un objeto JSON".to_string())?;
    let signer_ref = pick_str(
        obj,
        &["signerRef", "signer_ref", "signer", "alias", "certId", "certificateId", "id"],
    )
    .unwrap_or_default();
    let certificate = pick_bytes(
        obj,
        &[
            "certificate",
            "cert",
            "certificateDer",
            "certDer",
            "certificateBase64",
            "certBase64",
        ],
    );
    let digest_algorithm = pick_str(
        obj,
        &[
            "digestAlgorithm",
            "digest_algorithm",
            "digest",
            "hashAlgorithm",
            "hash_algorithm",
        ],
    )
    .unwrap_or_else(|| "SHA-256".to_string());
    let signature_algorithm = pick_str(
        obj,
        &[
            "signatureAlgorithm",
            "signature_algorithm",
            "algorithm",
            "signAlgorithm",
            "sign_algorithm",
        ],
    )
    .unwrap_or_default();
    let operation = pick_str(obj, &["operation", "op", "cop", "action"])
        .unwrap_or_else(|| "sign".to_string());
    let properties = obj
        .get("properties")
        .or_else(|| obj.get("params"))
        .or_else(|| obj.get("parameters"))
        .or_else(|| obj.get("extra"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    Ok(SignatureRequest {
        signer_ref,
        certificate,
        digest_algorithm,
        signature_algorithm,
        operation,
        properties,
    })
}

/// Construye el JSON de aserción que consumen las sedes electrónicas.
///
/// El resultado cumple el esquema usado por rFirma/AutoFirme: claves
/// `assertion`, `signature`, `cert`, `parameters` y `signatureValue`.
pub fn build_assertion_json(
    session_id: &str,
    signer: &SignerIdentity,
    cert_der: &[u8],
    cms_der: &[u8],
    signature_value: &[u8],
    properties: &Value,
) -> SignedAssertion {
    let now = unix_now();
    let fingerprint = hex_encode(&sha256(cert_der));
    let algorithm_name = match parse_x509_certificate(cert_der) {
        Ok(cert) => match cert.public_key {
            PublicKey::Rsa { .. } => "SHA256withRSA",
            PublicKey::Ec { .. } => "SHA256withECDSA",
        },
        Err(_) => "SHA256withRSA",
    };
    let subject = json!({
        "name": signer.name,
        "surname": signer.surname,
        "email": signer.email,
        "nif": signer.nif,
        "organization": signer.organization,
        "certificateSerialNumber": signer.serial_number,
        "certificateFingerprint": format!("sha256:{}", fingerprint),
        "publicKeyAlgorithm": algorithm_name,
    });
    let assertion = json!({
        "id": format!("{}-{}", session_id, now),
        "type": "urn:oasis:names:tc:SAML:2.0:assertion",
        "issuer": "urn:megadaemon:signing:afirma",
        "issueInstant": format_iso8601(now),
        "notOnOrAfter": format_iso8601(now + 300),
        "inResponseTo": session_id,
        "audience": properties
            .get("appname")
            .or_else(|| properties.get("appName"))
            .and_then(|v| v.as_str())
            .unwrap_or("afirma"),
        "subject": subject,
        "authnStatement": {
            "authnInstant": format_iso8601(now),
            "authnContext": "urn:megadaemon:afirma:local-signature",
        },
        "signatureAlgorithm": algorithm_name,
        "digestAlgorithm": "SHA-256",
        "signatureDescription": {
            "description": properties
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("Firma electrónica"),
            "reason": properties.get("reason").and_then(|v| v.as_str()),
            "location": properties.get("location").and_then(|v| v.as_str()),
        },
    });
    SignedAssertion {
        assertion,
        signature: base64_encode(cms_der),
        certificate: base64_encode(cert_der),
        parameters: properties.clone(),
        signature_value: base64_encode(signature_value),
    }
}

/// Resultado de ejecutar una operación del protocolo.
#[derive(Debug, Clone)]
pub struct AfirmaOperationResult {
    /// Aserción firmada (el JSON que espera la sede).
    pub assertion: SignedAssertion,
    /// Documento firmado (Base64) cuando la operación produce un fichero.
    pub result: Option<Vec<u8>>,
}

/// Extrae los datos a firmar de los parámetros de la petición.
fn extract_data(properties: &Value) -> Result<Vec<u8>, String> {
    for key in ["dat", "data", "content", "document", "file"] {
        if let Some(v) = properties.get(key) {
            match v {
                Value::String(s) => {
                    if let Ok(bytes) = base64_decode(s) {
                        if !bytes.is_empty() {
                            return Ok(bytes);
                        }
                    }
                    return Ok(s.as_bytes().to_vec());
                }
                Value::Array(items) => {
                    let mut bytes = Vec::with_capacity(items.len());
                    for item in items {
                        if let Some(n) = item.as_u64() {
                            bytes.push(n as u8);
                        }
                    }
                    return Ok(bytes);
                }
                _ => {}
            }
        }
    }
    Err("la petición no contiene los datos a firmar ('dat')".to_string())
}

/// Ejecuta una petición de firma y produce la aserción.
pub fn process_signature_request(
    request: &SignatureRequest,
    provider: &dyn SignatureProvider,
    session_id: &str,
    signer: &SignerIdentity,
) -> Result<AfirmaOperationResult, String> {
    let operation = request.operation.to_ascii_lowercase();
    match operation.as_str() {
        "selectcert" | "listcertificates" | "listcert" => {
            let cert = provider.certificate_der()?;
            let assertion = build_assertion_json(
                session_id,
                signer,
                &cert,
                &[],
                &[],
                &request.properties,
            );
            Ok(AfirmaOperationResult {
                assertion,
                result: None,
            })
        }
        "sign" | "cosign" | "countersign" | "signandsave" => {
            let data = extract_data(&request.properties)?;
            let format = request
                .properties
                .get("format")
                .and_then(|v| v.as_str())
                .unwrap_or("PAdES")
                .to_ascii_uppercase();
            let is_pdf = format.contains("PADES") || (data.starts_with(b"%PDF") && !format.contains("CADES"));
            let cms_der = if is_pdf {
                let pdf_options = PdfSignOptions {
                    field_name: request
                        .properties
                        .get("field")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Signature1")
                        .to_string(),
                    reason: request
                        .properties
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Firmado electrónicamente")
                        .to_string(),
                    location: request
                        .properties
                        .get("location")
                        .and_then(|v| v.as_str())
                        .unwrap_or("España")
                        .to_string(),
                    signer_name: format!(
                        "{} {}",
                        signer.name, signer.surname
                    )
                    .trim()
                    .to_string(),
                    ..PdfSignOptions::default()
                };
                let signed = sign_pdf(&data, provider, &pdf_options)?;
                signed.cms_der
            } else {
                build_cms_signed_data(provider, &data, &request.cms_options())?
            };
            let cert = provider.certificate_der()?;
            let signature_value = extract_signature_value(&cms_der);
            let assertion = build_assertion_json(
                session_id,
                signer,
                &cert,
                &cms_der,
                &signature_value,
                &request.properties,
            );
            Ok(AfirmaOperationResult {
                assertion,
                result: None,
            })
        }
        other => Err(format!("operación no soportada: {}", other)),
    }
}

impl SignatureRequest {
    /// Opciones de CMS derivadas de la petición (detached si es PAdES).
    fn cms_options(&self) -> CmsOptions {
        let format = self
            .properties
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_uppercase();
        CmsOptions {
            detached: format.contains("PADES") || format.contains("DETACHED"),
            ..CmsOptions::default()
        }
    }
}

/// Extrae el valor de la firma (último OCTET STRING del SignerInfo).
fn extract_signature_value(cms: &[u8]) -> Vec<u8> {
    match parse_cms_signed_data(cms) {
        Ok(info) => info.signature,
        Err(_) => Vec::new(),
    }
}

/// Flujo de red usado por el servidor `afirma://`.
pub trait AfirmaStream: Read + Write + Send {}
impl<T: Read + Write + Send> AfirmaStream for T {}

/// Proveedor de TLS para el servidor local.
pub trait TlsProvider: Send + Sync {
    /// Envuelve el socket en TLS.
    fn accept(&self, stream: TcpStream) -> Result<Box<dyn AfirmaStream>, String>;
}

/// Implementación sin TLS (sólo para pruebas y para el modo loopback).
pub struct PlaintextTls;

impl TlsProvider for PlaintextTls {
    fn accept(&self, stream: TcpStream) -> Result<Box<dyn AfirmaStream>, String> {
        Ok(Box::new(stream))
    }
}

/// Configuración del servidor local `afirma://`.
#[derive(Clone)]
pub struct AfirmaServerConfig {
    /// Versión del protocolo: 3 (puerto fijo 63117) o 4 (puerto dinámico).
    pub protocol_version: u8,
    /// Dirección de escucha (siempre 127.0.0.1 en producción).
    pub bind_address: String,
    /// Puerto de escucha; 0 elige uno libre (protocolo 4).
    pub port: u16,
    /// Identificador de sesión.
    pub session_id: String,
    /// Nombre de la aplicación que aparece en la aserción.
    pub app_name: String,
    /// Identidad del firmante.
    pub signer: SignerIdentity,
    /// Proveedor de firma (token PKCS#11 o software).
    pub provider: Option<Arc<Mutex<dyn SignatureProvider>>>,
    /// Capa TLS (si se omite, el socket sirve HTTP en claro).
    pub tls: Option<Arc<dyn TlsProvider>>,
}

impl Default for AfirmaServerConfig {
    fn default() -> Self {
        AfirmaServerConfig {
            protocol_version: 3,
            bind_address: "127.0.0.1".to_string(),
            port: 63117,
            session_id: format!("mgd{}", unix_now()),
            app_name: "MegaDaemon".to_string(),
            signer: SignerIdentity::default(),
            provider: None,
            tls: None,
        }
    }
}

/// Estado compartido del servidor.
struct AfirmaServerState {
    config: AfirmaServerConfig,
    port: u16,
    stop: Arc<AtomicBool>,
}

/// Servidor local del protocolo `afirma://`.
pub struct AfirmaServer {
    state: Arc<AfirmaServerState>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl AfirmaServer {
    /// Arranca el servidor en segundo plano.
    pub fn start(config: AfirmaServerConfig) -> Result<AfirmaServer, String> {
        let address = format!("{}:{}", config.bind_address, config.port);
        let listener = TcpListener::bind(&address)
            .map_err(|e| format!("no se puede escuchar en {}: {}", address, e))?;
        let port = listener
            .local_addr()
            .map(|a| a.port())
            .unwrap_or(config.port);
        listener
            .set_nonblocking(true)
            .map_err(|e| format!("no se puede configurar el socket: {}", e))?;
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(AfirmaServerState {
            config,
            port,
            stop: stop.clone(),
        });
        let worker_state = state.clone();
        let worker = std::thread::spawn(move || {
            while !worker_state.stop.load(AtomicOrdering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _addr)) => {
                        let st = worker_state.clone();
                        let _ = std::thread::Builder::new()
                            .name("afirma-conn".to_string())
                            .spawn(move || {
                                let _ = handle_connection(stream, &st);
                            });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(AfirmaServer {
            state,
            worker: Some(worker),
        })
    }

    /// Puerto efectivo del servidor.
    pub fn port(&self) -> u16 {
        self.state.port
    }

    /// URL base del servidor.
    pub fn url(&self) -> String {
        format!(
            "http://{}:{}",
            self.state.config.bind_address, self.state.port
        )
    }

    /// Versión del protocolo negociada.
    pub fn protocol_version(&self) -> u8 {
        self.state.config.protocol_version
    }

    /// Detiene el servidor.
    pub fn stop(&mut self) {
        self.state.stop.store(true, AtomicOrdering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for AfirmaServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Atiende una conexión HTTP del servidor `afirma://`.
fn handle_connection(stream: TcpStream, state: &Arc<AfirmaServerState>) -> Result<(), String> {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));
    let mut boxed: Box<dyn AfirmaStream> = match &state.config.tls {
        Some(tls) => tls.accept(stream)?,
        None => Box::new(stream),
    };
    let mut reader = std::io::BufReader::new(ReadAdapter(boxed.as_mut()));
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(|e| format!("no se puede leer la petición: {}", e))?;
    if request_line.trim().is_empty() {
        return Ok(());
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("GET").to_string();
    let target = parts.next().unwrap_or("/").to_string();

    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        let n = reader
            .read_line(&mut header)
            .map_err(|e| format!("cabecera ilegible: {}", e))?;
        if n == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
    }
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader
            .read_exact(&mut body)
            .map_err(|e| format!("cuerpo incompleto: {}", e))?;
    }

    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target.clone(), String::new()),
    };
    let (status, content_type, payload) = dispatch(state, &method, &path, &query, &body);
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        reason_phrase(status),
        content_type,
        payload.len(),
        payload
    );
    boxed
        .write_all(response.as_bytes())
        .map_err(|e| format!("no se puede escribir la respuesta: {}", e))?;
    let _ = boxed.flush();
    Ok(())
}

/// Adaptador que permite leer de `&mut dyn AfirmaStream`.
struct ReadAdapter<'a>(&'a mut dyn AfirmaStream);

impl Read for ReadAdapter<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

/// Frase HTTP según el código.
fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "Unknown",
    }
}

/// Enruta una petición HTTP del servidor `afirma://`.
fn dispatch(
    state: &Arc<AfirmaServerState>,
    method: &str,
    path: &str,
    query: &str,
    body: &[u8],
) -> (u16, String, String) {
    let json_type = "application/json; charset=utf-8".to_string();
    let clean_path = path.split('?').next().unwrap_or(path);
    match (method, clean_path) {
        ("GET", "/health") => (200, "text/plain; charset=utf-8".to_string(), "OK".to_string()),
        ("GET", "/") => {
            let info = json!({
                "protocol": state.config.protocol_version,
                "port": state.port,
                "appName": state.config.app_name,
                "session": state.config.session_id,
                "operations": ["sign", "selectcert", "service", "websocket"],
                "tls": state.config.tls.is_some(),
            });
            (200, json_type, info.to_string())
        }
        ("POST", "/afirma") | ("POST", "/sign") | ("POST", "/") => {
            match handle_sign_request(state, query, body) {
                Ok(value) => (200, json_type, value.to_string()),
                Err(e) => (
                    400,
                    json_type,
                    json!({ "error": e }).to_string(),
                ),
            }
        }
        ("GET", "/afirma") => {
            let params = parse_afirma_url(&format!("afirma://?{}", query)).unwrap_or_default();
            match params.get("echo") {
                Some(echo) if echo.contains("@EOF") => {
                    (200, "text/plain; charset=utf-8".to_string(), "OK".to_string())
                }
                _ => {
                    let info = json!({
                        "protocol": state.config.protocol_version,
                        "port": state.port,
                        "appName": state.config.app_name,
                        "session": state.config.session_id,
                        "operations": ["sign", "selectcert", "service", "websocket"],
                        "tls": state.config.tls.is_some(),
                    });
                    (200, json_type, info.to_string())
                }
            }
        }
        _ => (
            404,
            json_type,
            json!({ "error": format!("ruta desconocida: {}", clean_path) }).to_string(),
        ),
    }
}

/// Procesa una petición de firma del protocolo `afirma://`.
fn handle_sign_request(
    state: &Arc<AfirmaServerState>,
    query: &str,
    body: &[u8],
) -> Result<Value, String> {
    let text = std::str::from_utf8(body).unwrap_or_default();
    let trimmed = text.trim_start();
    let request = if trimmed.starts_with('{') {
        parse_signature_request(trimmed)?
    } else {
        // Formato clásico del protocolo de socket: pares clave=valor.
        let mut params = HashMap::new();
        for pair in query.split('&').chain(text.split('&')) {
            if pair.trim().is_empty() {
                continue;
            }
            if let Some((k, v)) = pair.split_once('=') {
                params.insert(url_decode(k), url_decode(v));
            }
        }
        let operation = params.get("cop").cloned().unwrap_or_else(|| "sign".to_string());
        let properties = Value::Object(
            params
                .iter()
                .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                .collect(),
        );
        SignatureRequest {
            signer_ref: params.get("id").cloned().unwrap_or_default(),
            certificate: None,
            digest_algorithm: "SHA-256".to_string(),
            signature_algorithm: params
                .get("algorithm")
                .cloned()
                .unwrap_or_else(|| "SHA256withRSA".to_string()),
            operation,
            properties,
        }
    };
    let provider = state
        .config
        .provider
        .as_ref()
        .ok_or_else(|| "el servidor no tiene configurado ningún firmante".to_string())?;
    let guard = provider
        .lock()
        .map_err(|_| "no se puede bloquear el proveedor de firma".to_string())?;
    let outcome = process_signature_request(
        &request,
        &*guard,
        &state.config.session_id,
        &state.config.signer,
    )?;
    let mut value = serde_json::to_value(&outcome.assertion)
        .map_err(|e| format!("no se puede serializar la aserción: {}", e))?;
    if let Value::Object(map) = &mut value {
        map.insert(
            "session".to_string(),
            json!({
                "id": state.config.session_id,
                "protocol": state.config.protocol_version,
                "started": format_iso8601(unix_now()),
            }),
        );
        map.insert(
            "operation".to_string(),
            Value::String(request.operation.clone()),
        );
    }
    Ok(value)
}
/// Clave RSA de prueba (PKCS#8 PEM, 2048 bits). Generada para los tests.
pub const TEST_RSA_KEY_PEM: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQC5y963EZ9v9HWO
VDbTC5GrZN1mml5Umx9vk1IhBSVMoPL1Nivl0Fa6h2g78zh0CBrEE+4h4Dyw+dIu
Tdt6JmL0mSuXmbQuvJuw7+whilTVmTeXQcLaZHAEoDqrSq/4L6+PYEQnPwHIKzbQ
a3v19m0PzoE02oCOtcicPUZzzku98bBbGNC7tyAtDIQYbOD4OHkf3OKA80vZrf8u
qP7W8iNl4huvGD9wvqqf+VXHuNOujz2B+9CN/pK6UDkYHr3T2aFx683PGzScw89J
e2SIlb/cgrt6f3BxFGy3BI/Puxpi3MxNBa7v/UhIB8K5Tu4vG1xn7SmFU1mXVTEx
KjOJF5KPAgMBAAECggEAAbhYgVvmtGwxaPFUIhdR/r44+0GHf6vWzBLnrlcGE/im
O1Qf83jM5ogAw9rzfGc44Rqn6OwVIWPyQt20oAdTGUgstP6GL2l5O/i2vO+R9TKo
iCIud2n0tJKz7H7wvdNCAj264W2xA6SWVWaZ8CikjrszqiOaP0+w3S3hYZ2uNftw
DBNffSphDlTRcd8nRSPUr8Le6NqTxVJgU3WWbfiRbPD6eYO75gje6JFcBOyzZNFP
zAiuTU4LeDhCm8NgVOGi7BSPpLJsFzhb+ocNaH/8VGU+LUpspNKTnmaqRP37GdzB
8YcdyoH5P6E3wqUHxoAovwhrxEG9KXinb6GjO+268QKBgQDs+D/I9RakojML7r3J
BsuAH9OQWSGqm9obo6th8YAwcqfXlEtNb3MT/5RmuaRok3CNEJ8wZuSJNWO/sG5e
yuV6x8TBz+CzEGv9yWykOF00gxZfKjJvgHKjf1PLySa0DR4F3CiiR0KtaGLl7k3R
M2FHKjLoQyFP95vJ3Yrx794C2wKBgQDIt5JMlp07hB7agkFBMwgglVXMGWCaQIQX
FltL0pWdPYj1oxqr0Bb3IAv839U2vX+RFw4xbS7GhkGynvO+6B6VNDMqOE01EJNZ
5IqLHuh3P6G4KYrZ8L36b6PbNFLKOc0r3TX5ALbFE8wf4/R/OXMUXPUswFthr8XF
t75tGW1rXQKBgGS/Wqb+hGT0TM2a7/dCu0w6u8DYFDcr0bwEhr26Ziyy4mcGNkmy
w3NSKu7NqHeJj8V5tqRmam1nVIRP52zHyG699goMblTS+4bHlzfuernzrZRCJXVA
X/zhCmhOmfTcytHk+yWbNF7zTXDCMzVpKceOylvzBnuwIvzdAtC/kV0XAoGAVmmp
461IPhJQkjRX0bEdw6FnUbpvO5mhBKuMXKVp4ktWxqIU3nl1GanDX3giPDHzVMy/
QIpIBBJTAIqsV7lpSUUy7e9VVg4v0hixiNmirYxInJfN/qxFricYPUxWhFfEPCk9
jlk/VrOCQwyJ+6gJInAu4bhi3zikhc3ofuHemMUCgYEAouKsgLgAJumlHqVisvmJ
XxzmdwR3otZwJpRZILoOmKRm/uX7dEqRfpLokYjv0oJiEoSTpa4N1TxaYazsRN8R
5/7bJowJzs9p4Lg1rTvjyqk+vKfE04Yg4sKPQAXafCv7IzlJ/LuIVevtxqmXPMBX
Ia/kucv3AOpxEuklv/65gmE=
-----END PRIVATE KEY-----"#;

/// Certificado RSA de prueba (autofirmado, PEM).
pub const TEST_RSA_CERT_PEM: &str = r#"-----BEGIN CERTIFICATE-----
MIIDezCCAmOgAwIBAgIUT924S0neGhM8bGc4u7mPNA3oYdYwDQYJKoZIhvcNAQEL
BQAwTTELMAkGA1UEBhMCRVMxGDAWBgNVBAoMD01lZ2FEYWVtb24gVGVzdDEkMCIG
A1UEAwwbTWVnYURhZW1vbiBSU0EgU2lnbmluZyBUZXN0MB4XDTI2MTExOTE5MDQx
NFoXDTQ2MTExNDE5MDQxNFowTTELMAkGA1UEBhMCRVMxGDAWBgNVBAoMD01lZ2FE
YWVtb24gVGVzdDEkMCIGA1UEAwwbTWVnYURhZW1vbiBSU0EgU2lnbmluZyBUZXN0
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAucvetxGfb/R1jlQ20wuR
q2TdZppeVJsfb5NSIQUlTKDy9TYr5dBWuodoO/M4dAgaxBPuIeA8sPnSLk3beiZi
9Jkrl5m0LrybsO/sIYpU1Zk3l0HC2mRwBKA6q0qv+C+vj2BEJz8ByCs20Gt79fZt
D86BNNqAjrXInD1Gc85LvfGwWxjQu7cgLQyEGGzg+Dh5H9zigPNL2a3/Lqj+1vIj
ZeIbrxg/cL6qn/lVx7jTro89gfvQjf6SulA5GB6909mhcevNzxs0nMPPSXtkiJW/
3IK7en9wcRRstwSPz7saYtzMTQWu7/1ISAfCuU7uLxtcZ+0phVNZl1UxMSoziReS
jwIDAQABo1MwUTAdBgNVHQ4EFgQUoLSMj1j2Wj4bpiqS6KgTskSp4L4wHwYDVR0j
BBgwFoAUoLSMj1j2Wj4bpiqS6KgTskSp4L4wDwYDVR0TAQH/BAUwAwEB/zANBgkq
hkiG9w0BAQsFAAOCAQEARdnNT2HPPd5jPWBkKv6+mr9YyPLdqGkr8CJI++5pimUa
lLaXmKisfiWe9LDBi0QvJpeTkv0XmPQz+2sMMBOLQacphBVXJjioGh3MBO0WLihu
xYdl/SaRf6GnPKMVqlmlbt+E5Xn3vbZQXilqpDAvwBFJdGbv8fQAeT38YpdUFwif
M+e3xdIqD1MMLi+aYZQyvwWKv7ZGtcHN7oE+UQCpQsgdGptElhWeaniBKQkUfSnT
dfAqV07FJzmQwrzRK6+fLievQ4Blbtpo7pF3cK4Yx9ZwQHMZ9nAm5QV49ivjuaE2
AWhP3jtJwOX2uuldF1wMB0kOwJ7dPcdAqxOGo/F9NQ==
-----END CERTIFICATE-----"#;

/// Clave ECDSA de prueba (PKCS#8 PEM, curva P-256).
pub const TEST_EC_KEY_PEM: &str = r#"-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgBZI3roBxvqtKFZDP
sWX9vPYZiFgIVGEStWCYXxPvFGahRANCAATMK8rEA4MNsK10XU2jGwZtwv0vYu3y
HbjvRxgtpI0LJ5aOwvbSuJwDOurPIDVI32XZA6FEGkGt/FxV3upc20R8
-----END PRIVATE KEY-----"#;

/// Certificado ECDSA de prueba (autofirmado, PEM).
pub const TEST_EC_CERT_PEM: &str = r#"-----BEGIN CERTIFICATE-----
MIIB7jCCAZOgAwIBAgIUFmkBM5Xqlb+/cX3/oapjWFBfAD0wCgYIKoZIzj0EAwIw
TDELMAkGA1UEBhMCRVMxGDAWBgNVBAoMD01lZ2FEYWVtb24gVGVzdDEjMCEGA1UE
AwwaTWVnYURhZW1vbiBFQyBTaWduaW5nIFRlc3QwHhcNMjYxMTE5MTkwNDE0WhcN
NDYxMTE0MTkwNDE0WjBMMQswCQYDVQQGEwJFUzEYMBYGA1UECgwPTWVnYURhZW1v
biBUZXN0MSMwIQYDVQQDDBpNZWdhRGFlbW9uIEVDIFNpZ25pbmcgVGVzdDBZMBMG
ByqGSM49AgEGCCqGSM49AwEHA0IABMwrysQDgw2wrXRdTaMbBm3C/S9i7fIduO9H
GC2kjQsnlo7C9tK4nAM66s8gNUjfZdkDoUQaQa38XFXe6lzbRHyjUzBRMB0GA1Ud
DgQWBBS+wrITSYEgulKAsCaKhONvKYH/WTAfBgNVHSMEGDAWgBS+wrITSYEgulKA
sCaKhONvKYH/WTAPBgNVHRMBAf8EBTADAQH/MAoGCCqGSM49BAMCA0kAMEYCIQCi
Og7hxsrVPIgIauXO9cFeZboPa8GLtVpsomCAfjD8HAIhAObiABGmx71l4e0PSdes
dIaSVrZJaTg2u9ztqbZVQ3Rg
-----END CERTIFICATE-----"#;
// ===========================================================================
// 14. Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Firmante software estático (para el servidor, que exige `'static`).
    struct StaticSigner {
        token: SoftwareToken,
    }

    impl SignatureProvider for StaticSigner {
        fn certificate_der(&self) -> Result<Vec<u8>, String> {
            Ok(self.token.certificate_der().to_vec())
        }

        fn signature_algorithm(&self) -> KeyAlgorithm {
            KeyAlgorithm::RsaPkcs1
        }

        fn sign_digest(&self, digest: &[u8]) -> Result<Vec<u8>, String> {
            self.token.sign_digest_raw(digest)
        }
    }

    /// Devuelve el certificado DER del token RSA de prueba.
    fn rsa_cert_der() -> Vec<u8> {
        let (_label, der) = pem_decode(TEST_RSA_CERT_PEM).expect("certificado de prueba");
        der
    }

    /// Devuelve el certificado DER del token EC de prueba.
    fn ec_cert_der() -> Vec<u8> {
        let (_label, der) = pem_decode(TEST_EC_CERT_PEM).expect("certificado de prueba");
        der
    }

    /// PDF mínimo sin campo de firma (para la actualización incremental).
    fn build_pdf_without_signature_field() -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        let mut offsets: Vec<usize> = Vec::new();
        out.extend_from_slice(b"%PDF-1.4\n");
        let bodies = [
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] >>",
        ];
        for (i, body) in bodies.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", i + 1, body).as_bytes());
        }
        let xref = out.len();
        out.extend_from_slice(
            format!(
                "xref\n0 4\n0000000000 65535 f \n{:010} 00000 n \n{:010} 00000 n \n{:010} 00000 n \ntrailer\n<< /Size 4 /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
                offsets[0], offsets[1], offsets[2], xref
            )
            .as_bytes(),
        );
        out
    }

    /// POST HTTP mínimo contra el servidor local.
    fn http_post(port: u16, path: &str, body: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("no se puede conectar");
        let request = format!(
            "POST {} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            path,
            body.len(),
            body
        );
        stream
            .write_all(request.as_bytes())
            .expect("no se puede enviar la petición");
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .expect("no se puede leer la respuesta");
        response
    }

    #[test]
    fn test_base64_hex_y_pem_roundtrip() {
        let data = b"MegaDaemon: firma electronica \x00\xff\x10";
        let b64 = base64_encode(data);
        assert_eq!(base64_decode(&b64).unwrap(), data.to_vec());
        let hex = hex_encode(data);
        assert_eq!(hex_decode(&hex).unwrap(), data.to_vec());
        let pem = pem_encode("CERTIFICATE", data);
        let (label, der) = pem_decode(&pem).unwrap();
        assert_eq!(label, "CERTIFICATE");
        assert_eq!(der, data.to_vec());
        assert!(base64_decode("%%%").is_err());
    }

    #[test]
    fn test_der_codificacion_primitivas() {
        // OID de SHA-256: 2.16.840.1.101.3.4.2.1
        let oid = der_oid(OID_SHA256);
        assert_eq!(
            oid,
            vec![0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01]
        );
        // INTEGER positivo con byte de signo
        assert_eq!(der_int(&[0xff]), vec![0x02, 0x02, 0x00, 0xff]);
        // Longitud larga
        assert_eq!(der_len(300), vec![0x82, 0x01, 0x2c]);
        // El lector devuelve los mismos datos
        let mut reader = DerReader::new(&oid);
        let tlv = reader.read().unwrap();
        assert_eq!(tlv.oid().unwrap(), OID_SHA256.to_vec());
        assert!(reader.is_empty());
        // Truncado -> error
        let mut bad = DerReader::new(&oid[..3]);
        assert!(bad.read().is_err());
    }

    #[test]
    fn test_software_token_lista_objetos() {
        let token = SoftwareToken::from_pem(TEST_RSA_KEY_PEM, TEST_RSA_CERT_PEM).unwrap();
        let objects = token.list_objects().unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].key_kind, "RSA");
        assert!(objects[0].cert_der.is_some());
        let ec_token = SoftwareToken::from_pem(TEST_EC_KEY_PEM, TEST_EC_CERT_PEM).unwrap();
        assert_eq!(ec_token.list_objects().unwrap()[0].key_kind, "EC");
    }

    #[test]
    fn test_firma_software_rsa_verifica_con_verify_cms() {
        let token = SoftwareToken::from_pem(TEST_RSA_KEY_PEM, TEST_RSA_CERT_PEM).unwrap();
        let object = token.list_objects().unwrap().remove(0);
        let signer = TokenSigner::new(&token, object).unwrap();
        let data = b"contenido que firma MegaDaemon";
        let cms = build_cms_signed_data(&signer, data, &CmsOptions::default()).unwrap();
        let cert = rsa_cert_der();
        assert!(verify_cms(&cms, data, &cert).unwrap(), "la firma debe verificar");
        assert!(
            !verify_cms(&cms, b"otro contenido", &cert).unwrap(),
            "con datos alterados no debe verificar"
        );
        assert!(
            !verify_cms(&cms, data, &ec_cert_der()).unwrap(),
            "con otro certificado no debe verificar"
        );
        // La firma DESPRENDIDA sobre el mismo contenido también verifica.
        let detached = build_cms_detached_signed_data(&signer, &sha256(data), &CmsOptions::default())
            .unwrap();
        assert!(verify_cms(&detached, data, &cert).unwrap());
    }

    #[test]
    fn test_firma_software_ecdsa_verifica_con_verify_cms() {
        let token = SoftwareToken::from_pem(TEST_EC_KEY_PEM, TEST_EC_CERT_PEM).unwrap();
        let object = token.list_objects().unwrap().remove(0);
        let signer = TokenSigner::new(&token, object).unwrap();
        let data = b"contenido ECDSA";
        let cms = build_cms_signed_data(&signer, data, &CmsOptions::default()).unwrap();
        let cert = ec_cert_der();
        assert!(verify_cms(&cms, data, &cert).unwrap());
        assert!(!verify_cms(&cms, b"otro", &cert).unwrap());
        // La firma cruda del token es r||s (64 bytes) y el CMS lleva DER.
        let raw = token.sign_digest_raw(&sha256(data)).unwrap();
        assert_eq!(raw.len(), 64);
        let info = parse_cms_signed_data(&cms).unwrap();
        assert_eq!(info.signature_algorithm, KeyAlgorithm::Ecdsa);
        // El CMS lleva la firma ECDSA en DER, con (r, s) dentro de rango.
        let signature = EcdsaSignature::from_der(&info.signature).unwrap();
        let params = p256();
        assert!(!signature.r.is_zero() && signature.r.cmp_big(&params.n) != Ordering::Less);
        assert!(!signature.s.is_zero() && signature.s.cmp_big(&params.n) != Ordering::Less);
    }

    #[test]
    fn test_cms_contiene_atributos_firmados_cades() {
        let token = SoftwareToken::from_pem(TEST_RSA_KEY_PEM, TEST_RSA_CERT_PEM).unwrap();
        let object = token.list_objects().unwrap().remove(0);
        let signer = TokenSigner::new(&token, object).unwrap();
        let data = b"atributos";
        let mut options = CmsOptions::default();
        options.signing_time = Some("261119190213Z".to_string());
        let cms = build_cms_signed_data(&signer, data, &options).unwrap();
        let info = parse_cms_signed_data(&cms).unwrap();
        assert_eq!(info.content_digest, sha256(data).to_vec());
        assert!(info.certificate.is_some());
        // Se recorren los atributos firmados.
        let mut reader = DerReader::new(&info.signed_attrs);
        let mut oids: Vec<Vec<u64>> = Vec::new();
        while !reader.is_empty() {
            let attr = reader.read().unwrap();
            // Cada atributo es SEQUENCE { OID, SET OF valor }: sus hijos
            // están directamente en el contenido de la SEQUENCE.
            let mut fields = DerReader::new(attr.content);
            oids.push(fields.read().unwrap().oid().unwrap());
        }
        assert!(oids.contains(&OID_CONTENT_TYPE.to_vec()));
        assert!(oids.contains(&OID_MESSAGE_DIGEST.to_vec()));
        assert!(oids.contains(&OID_SIGNING_TIME.to_vec()));
        assert!(oids.contains(&OID_SIGNING_CERT_V2.to_vec()));
        assert!(oids.contains(&OID_ETS_COMMITMENT_TYPE.to_vec()));
    }

    #[test]
    fn test_parse_signature_request_roundtrip() {
        let cert = rsa_cert_der();
        let json = format!(
            r#"{{"signerRef":"mi-certificado","certificate":"{}","digestAlgorithm":"SHA-256","signatureAlgorithm":"RSA","operation":"sign","properties":{{"format":"PAdES","reason":"prueba"}}}}"#,
            base64_encode(&cert)
        );
        let request = parse_signature_request(&json).unwrap();
        assert_eq!(request.signer_ref, "mi-certificado");
        assert_eq!(request.digest_algorithm, "SHA-256");
        assert_eq!(request.signature_algorithm, "RSA");
        assert_eq!(request.operation, "sign");
        assert_eq!(request.properties["format"], "PAdES");
        assert_eq!(request.certificate.unwrap(), cert);
        // Variantes de nombres de clave.
        let alt = parse_signature_request(
            r#"{"signer":"alias","digest":"SHA-256","algorithm":"SHA256withRSA","op":"sign","params":{"format":"CAdES"}}"#,
        )
        .unwrap();
        assert_eq!(alt.signer_ref, "alias");
        assert_eq!(alt.operation, "sign");
        assert_eq!(alt.properties["format"], "CAdES");
        assert!(alt.certificate.is_none());
        // Errores.
        assert!(parse_signature_request("{no es json").is_err());
        assert!(parse_signature_request("[]").is_err());
    }

    #[test]
    fn test_build_assertion_json_tiene_las_claves_del_protocolo() {
        let cert = rsa_cert_der();
        let cms = vec![0x30, 0x03, 0x02, 0x01, 0x00];
        let raw = vec![1u8, 2, 3, 4];
        let identity = SignerIdentity {
            name: " testers".to_string(),
            surname: "Prueba".to_string(),
            email: "tester@example.es".to_string(),
            nif: "12345678Z".to_string(),
            organization: "MegaDaemon".to_string(),
            serial_number: "01AB".to_string(),
        };
        let assertion = build_assertion_json(
            "sesion-1",
            &identity,
            &cert,
            &cms,
            &raw,
            &json!({ "format": "PAdES", "appname": "sede" }),
        );
        let value = serde_json::to_value(&assertion).unwrap();
        for key in ["assertion", "signature", "cert", "parameters", "signatureValue"] {
            assert!(value.get(key).is_some(), "falta la clave {}", key);
        }
        assert_eq!(base64_decode(&assertion.certificate).unwrap(), cert);
        assert_eq!(base64_decode(&assertion.signature).unwrap(), cms);
        assert_eq!(base64_decode(&assertion.signature_value).unwrap(), raw);
        assert_eq!(value["assertion"]["inResponseTo"], "sesion-1");
        assert_eq!(value["assertion"]["audience"], "sede");
        assert_eq!(
            value["assertion"]["subject"]["certificateFingerprint"]
                .as_str()
                .unwrap()
                .to_string(),
            format!("sha256:{}", hex_encode(&sha256(&cert)))
        );
    }

    #[test]
    fn test_pdf_genera_campo_de_firma_y_placeholder() {
        let pdf = build_pdf_with_signature_field("Prueba MegaDaemon").unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        let doc = PdfDocument::parse(&pdf).unwrap();
        let field = find_signature_field(&doc)
            .unwrap()
            .expect("el PDF debe traer campo de firma");
        assert_eq!(field.name, "Signature1");
        let sig = doc.load_object(field.signature_number.unwrap()).unwrap();
        let dict = sig.dict().unwrap();
        assert_eq!(
            dict.get(&b"SubFilter".to_vec()).unwrap().name().unwrap(),
            b"ETSI.CAdES.detached"
        );
        assert_eq!(dict.get(&b"Filter".to_vec()).unwrap().name().unwrap(), b"Adobe.PPKLite");
        // Placeholder de /Contents: todo ceros y con el tamaño reservado.
        match dict.get(&b"Contents".to_vec()).unwrap() {
            PdfObject::Hex(bytes, offset, len) => {
                assert_eq!(bytes.len(), 16384);
                assert!(bytes.iter().all(|&b| b == 0));
                assert_eq!(pdf[*offset - 1], b'<');
                assert_eq!(pdf[*offset + *len], b'>');
            }
            other => panic!("/Contents no es hexadecimal: {:?}", other),
        }
        // El /ByteRange se localiza y tiene 4 huecos.
        let regions = find_byte_range_regions(&pdf, 0).unwrap();
        assert_eq!(regions.len(), 4);
        assert!(regions[0].0 < regions[0].1);
    }

    #[test]
    fn test_sign_pdf_produce_pdf_firmado_verificable() {
        let pdf = build_pdf_with_signature_field("Prueba").unwrap();
        let token = SoftwareToken::from_pem(TEST_RSA_KEY_PEM, TEST_RSA_CERT_PEM).unwrap();
        let object = token.list_objects().unwrap().remove(0);
        let signer = TokenSigner::new(&token, object).unwrap();
        let result = sign_pdf(&pdf, &signer, &PdfSignOptions::default()).unwrap();
        // El PDF ya traía el hueco reservado: se rellena sin cambiar el tamaño.
        assert!(result.bytes.len() >= pdf.len());
        assert_eq!(result.byte_range[0], 0);
        // El CMS verifica sobre los bytes firmados.
        let cert = rsa_cert_der();
        assert!(verify_cms(&result.cms_der, &result.signed_content, &cert).unwrap());
        // El PDF firmado se puede volver a leer y verificar.
        assert!(verify_pdf_signature(&result.bytes, &cert).unwrap());
        // Los bytes firmados se reconstruyen a partir del ByteRange.
        let extracted = extract_signed_content(&result.bytes, result.byte_range).unwrap();
        assert_eq!(extracted, result.signed_content);
        assert_eq!(result.digest, sha256(&result.signed_content));
    }

    #[test]
    fn test_sign_pdf_sin_campo_usa_actualizacion_incremental() {
        let pdf = build_pdf_without_signature_field();
        let token = SoftwareToken::from_pem(TEST_RSA_KEY_PEM, TEST_RSA_CERT_PEM).unwrap();
        let object = token.list_objects().unwrap().remove(0);
        let signer = TokenSigner::new(&token, object).unwrap();
        let result = sign_pdf(&pdf, &signer, &PdfSignOptions::default()).unwrap();
        let cert = rsa_cert_der();
        assert!(verify_pdf_signature(&result.bytes, &cert).unwrap());
        // El PDF resultante ya tiene campo de firma.
        let doc = PdfDocument::parse(&result.bytes).unwrap();
        assert!(find_signature_field(&doc).unwrap().is_some());
    }

    #[test]
    fn test_servidor_afirma_responde_con_assertion() {
        let token = SoftwareToken::from_pem(TEST_RSA_KEY_PEM, TEST_RSA_CERT_PEM).unwrap();
        let provider: Arc<Mutex<dyn SignatureProvider>> =
            Arc::new(Mutex::new(StaticSigner { token }));
        let config = AfirmaServerConfig {
            protocol_version: 4,
            port: 0, // puerto dinámico (protocolo 4)
            provider: Some(provider),
            ..AfirmaServerConfig::default()
        };
        let mut server = AfirmaServer::start(config).unwrap();
        let port = server.port();
        assert!(port > 0);
        assert_eq!(server.protocol_version(), 4);

        let data = b"datos para la sede";
        let body = format!(
            r#"{{"signerRef":"alias","operation":"sign","properties":{{"format":"CAdES","dat":"{}"}}}}"#,
            base64_encode(data)
        );
        let response = http_post(port, "/afirma", &body);
        assert!(response.starts_with("HTTP/1.1 200"), "respuesta: {}", response);
        let payload = response.split_once("\r\n\r\n").unwrap().1;
        let json: Value = serde_json::from_str(payload).expect("la respuesta debe ser JSON");
        for key in ["assertion", "signature", "cert", "parameters", "signatureValue"] {
            assert!(json.get(key).is_some(), "falta {}", key);
        }
        // La firma devuelta verifica sobre los datos originales.
        let cms = base64_decode(json["signature"].as_str().unwrap()).unwrap();
        let cert = base64_decode(json["cert"].as_str().unwrap()).unwrap();
        assert!(verify_cms(&cms, data, &cert).unwrap());
        server.stop();
    }

    #[test]
    fn test_parse_afirma_url() {
        let params = parse_afirma_url("afirma://sign?dat=SGVsbG8%3D&id=abc&appname=sede").unwrap();
        assert_eq!(params.operation, "sign");
        assert_eq!(params.get("dat"), Some("SGVsbG8="));
        assert_eq!(params.get("id"), Some("abc"));
        assert_eq!(params.get("appname"), Some("sede"));
        assert!(parse_afirma_url("https://ejemplo.es").is_err());
    }

    #[test]
    fn test_pkcs11_sondeo_opcional() {
        // Sólo se puede probar si hay un módulo PKCS#11 instalado (OpenSC).
        if let Some(path) = find_pkcs11_library() {
            match Pkcs11HardwareToken::open(&path, None, 0) {
                Ok(token) => {
                    // Sin tarjeta puede no haber objetos: no debe entrar en pánico.
                    let objects = token.list_objects().unwrap_or_default();
                    for object in objects {
                        assert!(!object.key_kind.is_empty());
                    }
                }
                Err(_) => {} // sin lector: es el caso normal en CI
            }
        }
    }
}
