//! MegaDaemon Spanish eID Engine: FNMT Certificado Electrónico (PKCS#12) & DNIe (ISO-7816-4).
//!
//! Pure-Rust implementation built on `p12-keystore`, `x509-cert`, `rustls`, and `ureq`
//! (zero OpenSSL / libclang / PC-SC C dependencies):
//! - Load & inspect FNMT / Ceres / Camerfirma `.p12`/`.pfx` containers
//! - Extract subject, issuer, serial, validity, and Spanish NIF/FNMT metadata via `x509-cert`
//! - Build `rustls::ClientConfig` + `ureq::Agent` configured for mutual TLS (mTLS)
//!   against Spanish e-Government portals (AEAT, Sede Electrónica, Cl@ve, FNMT)
//! - Generate standard ISO-7816-4 APDU sequences for DNI Electrónico (DNIe 3.0/4.0)
//!   smartcard / Android NFC (`IsoDep`) authentication & PKCS#1/PSS signing.

use p12_keystore::{KeyStore, KeyStoreEntry, Pkcs12ImportPolicy};
pub use rdnie::{
    Atr as RdnieAtr, DigestAlgorithm as RdnieDigestAlgorithm, Error as RdnieError,
    KeyGeneration as RdnieKeyGeneration, KeyReference as RdnieKeyReference, Pin as RdniePin,
    Port as RdniePort, Session as RdnieSession, Transport as RdnieTransport,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use x509_cert::der::Decode;
use x509_cert::Certificate;

/// Canonical DNIe 3.0/4.0 DF Application Identifier (`A0 00 00 00 63 50 4B 43 53 2D 31 35`, PKCS#15).
pub const DNIE_PKCS15_AID: &[u8] = &[
    0xA0, 0x00, 0x00, 0x00, 0x63, 0x50, 0x4B, 0x43, 0x53, 0x2D, 0x31, 0x35,
];

/// FNMT-RCM root/CA indicator substrings.
const FNMT_MARKERS: &[&str] = &["FNMT", "RCM", "AC FNMT Usuarios", "Ceres"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pkcs12Inspection {
    pub ok: bool,
    pub alias: String,
    pub subject: String,
    pub issuer: String,
    pub serial_hex: String,
    pub not_before: String,
    pub not_after: String,
    pub is_fnmt: bool,
    pub chain_len: usize,
    pub has_private_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnieApduFrame {
    pub step: String,
    pub cla: u8,
    pub ins: u8,
    pub p1: u8,
    pub p2: u8,
    pub hex: String,
}

/// Extracted mTLS identity (`rustls` cert chain + PKCS#8 private key).
pub struct LoadedIdentity {
    pub inspection: Pkcs12Inspection,
    pub cert_chain: Vec<CertificateDer<'static>>,
    pub private_key: PrivateKeyDer<'static>,
}

fn hex_upper(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        use std::fmt::Write;
        let _ = write!(&mut s, "{:02X}", b);
    }
    s
}

/// Loads and inspects an FNMT (or any standard) `.p12` / `.pfx` certificate bundle.
pub fn load_pkcs12(path: &Path, password: &str) -> Result<LoadedIdentity, String> {
    let bytes = fs::read(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
    load_pkcs12_bytes(&bytes, password)
}

/// Parses raw `.p12` / `.pfx` bytes into a verified `LoadedIdentity`.
pub fn load_pkcs12_bytes(p12_bytes: &[u8], password: &str) -> Result<LoadedIdentity, String> {
    let ks = KeyStore::from_pkcs12(p12_bytes, password, Pkcs12ImportPolicy::Relaxed)
        .map_err(|e| format!("PKCS#12 decrypt/parse failed: {:?}", e))?;

    for (alias, entry) in ks.entries() {
        if let KeyStoreEntry::PrivateKeyChain(chain) = entry {
            let certs: Vec<CertificateDer<'static>> = chain
                .certs()
                .iter()
                .map(|c| CertificateDer::from(c.as_der().to_vec()))
                .collect();
            if certs.is_empty() {
                continue;
            }
            let key_der = PrivatePkcs8KeyDer::from(chain.key().as_der().to_vec());
            let private_key = PrivateKeyDer::Pkcs8(key_der);

            let leaf = Certificate::from_der(certs[0].as_ref())
                .map_err(|e| format!("X.509 DER parse failed: {:?}", e))?;
            let tbs = leaf.tbs_certificate();
            let subject = tbs.subject().to_string();
            let issuer = tbs.issuer().to_string();
            let serial_hex = hex_upper(tbs.serial_number().as_bytes());
            let not_before = tbs.validity().not_before.to_string();
            let not_after = tbs.validity().not_after.to_string();
            let is_fnmt = FNMT_MARKERS
                .iter()
                .any(|m| issuer.contains(m) || subject.contains(m));

            let inspection = Pkcs12Inspection {
                ok: true,
                alias: alias.clone(),
                subject,
                issuer,
                serial_hex,
                not_before,
                not_after,
                is_fnmt,
                chain_len: certs.len(),
                has_private_key: true,
            };

            return Ok(LoadedIdentity {
                inspection,
                cert_chain: certs,
                private_key,
            });
        }
    }

    Err("no PrivateKeyChain entry found in PKCS#12 container".to_string())
}

/// Builds a `rustls::ClientConfig` equipped with client certificate mTLS for FNMT / AEAT / Cl@ve.
pub fn build_mtls_rustls_config(id: LoadedIdentity) -> Result<Arc<rustls::ClientConfig>, String> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut roots = rustls::RootCertStore::empty();
    // Embed standard WebPKI roots when available, or rely on system verifier fallback via webpki-roots.
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let cfg = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| format!("rustls protocol versions: {}", e))?
        .with_root_certificates(roots)
        .with_client_auth_cert(id.cert_chain, id.private_key)
        .map_err(|e| format!("rustls client auth cert: {}", e))?;

    Ok(Arc::new(cfg))
}

/// Encodes an ISO-7816-4 short APDU command (`CLA INS P1 P2 [Lc Data] [Le]`).
pub fn encode_apdu(cla: u8, ins: u8, p1: u8, p2: u8, data: &[u8], le: Option<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 1 + data.len() + 1);
    out.extend_from_slice(&[cla, ins, p1, p2]);
    if !data.is_empty() {
        out.push(data.len() as u8);
        out.extend_from_slice(data);
    }
    if let Some(l) = le {
        out.push(l);
    }
    out
}

/// Builds the canonical DNIe (DNI Electrónico) ISO-7816-4 APDU sequence for
/// selecting the PKCS#15 app, verifying the user PIN, and computing a SHA-256 DigestInfo signature.
pub fn dnie_sign_apdu_sequence(pin: &str, key_ref: u8, sha256_digest: &[u8; 32]) -> Vec<DnieApduFrame> {
    let mut frames = Vec::new();

    // 1. SELECT MF / PKCS#15 AID (`00 A4 04 00 Lc AID`)
    let sel = encode_apdu(0x00, 0xA4, 0x04, 0x00, DNIE_PKCS15_AID, None);
    frames.push(DnieApduFrame {
        step: "SELECT_DNIE_AID".into(),
        cla: 0x00,
        ins: 0xA4,
        p1: 0x04,
        p2: 0x00,
        hex: hex_upper(&sel),
    });

    // 2. VERIFY PIN (`00 20 00 00 Lc PIN_ASCII`)
    let pin_apdu = encode_apdu(0x00, 0x20, 0x00, 0x00, pin.as_bytes(), None);
    frames.push(DnieApduFrame {
        step: "VERIFY_PIN".into(),
        cla: 0x00,
        ins: 0x20,
        p1: 0x00,
        p2: 0x00,
        hex: hex_upper(&pin_apdu),
    });

    // 3. MSE:SET DST (`00 22 41 B6 ...` algorithm SHA-256 + RSA PKCS#1 v1.5)
    let mse_data = [0x80, 0x01, 0x02, 0x84, 0x01, key_ref];
    let mse = encode_apdu(0x00, 0x22, 0x41, 0xB6, &mse_data, None);
    frames.push(DnieApduFrame {
        step: "MSE_SET_DST".into(),
        cla: 0x00,
        ins: 0x22,
        p1: 0x41,
        p2: 0xB6,
        hex: hex_upper(&mse),
    });

    // 4. PSO:COMPUTE DIGITAL SIGNATURE (`00 2A 9E 9A Lc DigestInfo Le=00`)
    // ASN.1 DigestInfo prefix for SHA-256 (19 bytes) + 32-byte digest.
    let mut digest_info = Vec::with_capacity(19 + 32);
    digest_info.extend_from_slice(&[
        0x30, 0x31, 0x30, 0x0D, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01,
        0x05, 0x00, 0x04, 0x20,
    ]);
    digest_info.extend_from_slice(sha256_digest);
    let pso = encode_apdu(0x00, 0x2A, 0x9E, 0x9A, &digest_info, Some(0x00));
    frames.push(DnieApduFrame {
        step: "PSO_COMPUTE_SIGNATURE".into(),
        cla: 0x00,
        ins: 0x2A,
        p1: 0x9E,
        p2: 0x9A,
        hex: hex_upper(&pso),
    });

    frames
}

/// Classifies a DNIe card ATR using `rdnie::Atr` (detects DNIe 3.0/4.0 vs legacy/burned/other).
pub fn inspect_dnie_atr(atr_bytes: &[u8]) -> serde_json::Value {
    let atr = RdnieAtr::new(atr_bytes.to_vec());
    match atr.check() {
        Ok(()) => serde_json::json!({
            "ok": true,
            "is_dnie_3_4": true,
            "atr_hex": hex_upper(atr_bytes),
        }),
        Err(e) => serde_json::json!({
            "ok": false,
            "is_dnie_3_4": false,
            "atr_hex": hex_upper(atr_bytes),
            "error": e.to_string(),
        }),
    }
}

/// Opens an `rdnie::Session` over any pure-Rust `rdnie::Transport` (e.g. Android `IsoDep` NFC
/// bridge or CCID), reads PIN retries, signs `digest`, and returns the PKCS#1 v1.5 signature.
pub fn sign_dnie_digest_with_transport<T: RdnieTransport>(
    transport: T,
    pin: &str,
    digest: &[u8],
    algorithm: RdnieDigestAlgorithm,
) -> Result<Vec<u8>, String> {
    let parsed_pin = RdniePin::new(pin.to_string());
    let mut session = RdnieSession::open(transport).map_err(|e| format!("DNIe open: {}", e))?;
    let _retries = session
        .pin_retries_left()
        .map_err(|e| format!("DNIe pin_retries_left: {}", e))?;
    let key_ref = session
        .private_key(rdnie::SIGNATURE_LABEL)
        .map_err(|e| format!("DNIe private_key: {}", e))?
        .ok_or_else(|| "DNIe signature key not found on card".to_string())?;
    session
        .sign(digest, algorithm, &key_ref, parsed_pin)
        .map_err(|e| format!("DNIe sign: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_valid_dnie_apdu_sequence() {
        let digest = [0xABu8; 32];
        let seq = dnie_sign_apdu_sequence("1234", 0x01, &digest);
        assert_eq!(seq.len(), 4);
        assert_eq!(seq[0].step, "SELECT_DNIE_AID");
        assert!(seq[0].hex.starts_with("00A404000C"));
        assert_eq!(seq[1].step, "VERIFY_PIN");
        assert!(seq[1].hex.starts_with("002000000431323334"));
        assert_eq!(seq[3].step, "PSO_COMPUTE_SIGNATURE");
        assert!(seq[3].hex.starts_with("002A9E9A33")); // 19 + 32 = 51 = 0x33
    }

    #[test]
    fn rejects_invalid_pkcs12_cleanly() {
        let res = load_pkcs12_bytes(b"not a valid p12", "secret");
        assert!(res.is_err());
    }

    #[test]
    fn rdnie_atr_classifier_rejects_non_dnie_atr() {
        let info = inspect_dnie_atr(&[0x3B, 0x00]);
        assert_eq!(info["is_dnie_3_4"], false);
        assert!(info["error"].as_str().is_some());
    }
}
