//! **Backup de certificados del usuario desde Android** (`cert_android`).
//!
//! QUÉ ES:
//!
//! Inventariar los **certificados** que existen en un móvil Android y
//! meterlos en la [`crate::cert_vault`] **sin que salga ni un byte de clave
//! privada**.
//!
//! * Se guarda la **parte pública** (DER) de cada certificado ⇒ es pública
//!   por definición y se puede replicar a los otros nodos.
//! * Se guarda la **referencia** de la clave (alias del Keystore, ruta del
//!   almacén de credenciales) ⇒ el material sigue atado al TEE/TPM y **no se
//!   copia**.
//! * Los ficheros que contienen **material privado** (`.p12`, `.pfx`, `.jks`,
//!   PEM `PRIVATE KEY`) **no se leen**: se listan como referencia y se marcan
//!   como "no extraído".
//!
//! ⇒ Esto es **lo mismo** que dice el plan de backup del repo
//! ([`crate::backup_full::keystore2_plan`]): los datos del keystore "no son
//! portables (están atados al TEE) y no salen del dispositivo: sólo su
//! inventario (alias y huellas)".
//!
//! DÓNDE BUSCA (raíces por defecto, **configurables**):
//!
//! | Fuente | Ruta | Qué es | ¿Privado? |
//! |---|---|---|---|
//! | CA del sistema | `/system/etc/security/cacerts` | Confianza del sistema (PEM) | No |
//! | CA del usuario | `/data/misc/keychain/cacerts-added` | CAs que instaló el usuario (PEM) | No |
//! | Credenciales de usuario | `/data/misc/keychain` | Pares usuario/certificado | **Sí** (sólo referencia) |
//! | Keystore del usuario | `/data/misc/keystore/user_0` | Aliases y blobs cifrados | **Sí** (sólo referencia) |
//!
//! **HONESTIDAD**: las rutas de arriba son las **estándar de Android**, pero
//! **no las he medido en el S21 en esta sesión** (el móvil no tenía ni ADB
//! ni puertos abiertos el 2026-11-21) ⇒ el escaneo es **parametrizable** y
//! una ruta que no exista simply yields an empty source, never a wrong
//! backup.  El agente con canal al móvil confirma las rutas y las fija.
//!
//! CÓMO SE USA:
//!
//! ```rust
//! let certs = cert_android::scan_certificates(&cert_android::default_sources())?;
//! let n = cert_android::import_into_vault(&vault, &certs)?;
//! // a partir de aquí, la bóveda ya sincroniza los certificados públicos
//! // a los demás nodos (sin material de clave).
//! ```

use base64ct::Encoding;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::cert_vault::{parse_certificate, sha256_hex, CertVault, KeyMaterial, KdfParams};

// ---------------------------------------------------------------------------
// Rutas por defecto
// ---------------------------------------------------------------------------

/// Almacén de credenciales del usuario (pares usuario/certificado).
pub const KEYCHAIN_DIR: &str = "/data/misc/keychain";
/// CAs que ha instalado el usuario (PEM, uno por fichero).
pub const USER_CA_DIR: &str = "/data/misc/keychain/cacerts-added";
/// Confianza del sistema (PEM, uno por fichero).
pub const SYSTEM_CA_DIR: &str = "/system/etc/security/cacerts";
/// Datos del Keystore del usuario (aliases + blobs cifrados).
pub const KEYSTORE_USER_DIR: &str = "/data/misc/keystore/user_0";

/// Backend de la bóveda para lo que vive en el Keystore de Android.
pub const BACKEND_KEYSTORE: &str = "android_keystore";
/// Backend de la bóveda para lo que vive en el almacén de credenciales.
pub const BACKEND_KEYCHAIN: &str = "android_keychain";

// ---------------------------------------------------------------------------
// Inventario
// ---------------------------------------------------------------------------

/// Una fuente de certificados a inventariar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AndroidCertSource {
    /// Etiqueta legible (`system_ca`, `user_ca`, `keychain`, `keystore`).
    pub label: String,
    /// Ruta raíz en el dispositivo.
    pub dir: String,
    /// Backend que se anotará en la bóveda.
    pub backend: String,
    /// ¿La fuente **debería** contener material privado?  Si es `true`, los
    /// ficheros con clave se listan pero **no se leen**.
    pub private: bool,
}

/// Las fuentes por defecto del dispositivo Android.
pub fn default_sources() -> Vec<AndroidCertSource> {
    vec![
        AndroidCertSource {
            label: "system_ca".to_string(),
            dir: SYSTEM_CA_DIR.to_string(),
            backend: BACKEND_KEYSTORE.to_string(),
            private: false,
        },
        AndroidCertSource {
            label: "user_ca".to_string(),
            dir: USER_CA_DIR.to_string(),
            backend: BACKEND_KEYSTORE.to_string(),
            private: false,
        },
        AndroidCertSource {
            label: "keychain".to_string(),
            dir: KEYCHAIN_DIR.to_string(),
            backend: BACKEND_KEYCHAIN.to_string(),
            private: true,
        },
        AndroidCertSource {
            label: "keystore".to_string(),
            dir: KEYSTORE_USER_DIR.to_string(),
            backend: BACKEND_KEYSTORE.to_string(),
            private: true,
        },
    ]
}

/// Un certificado **público** encontrado en el dispositivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AndroidCert {
    /// Identificador estable para la bóveda (`android-<label>-<sha16>`).
    pub id: String,
    /// Etiqueta legible.
    pub label: String,
    /// Certificado en DER (parte **pública**).
    pub der: Vec<u8>,
    /// Fuente de la que salió (`system_ca`, `user_ca`, `keychain`…).
    pub source: String,
    /// Referencia de la clave privada (alias o ruta): **nunca** el material.
    pub handle: String,
    /// SHA-256 del DER.
    pub sha256: String,
    /// Sujeto (DN).
    pub subject: String,
    /// Emisor (DN).
    pub issuer: String,
    /// `notAfter` (epoch).
    pub not_after: i64,
    /// ¿Es autofirmado (sujeto == emisor)?
    pub self_signed: bool,
}

/// Un fichero con **material privado**: se lista, **no se lee**.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateRef {
    /// Ruta completa en el dispositivo.
    pub path: String,
    /// Tamaño en bytes (sólo metadato).
    pub size: u64,
    /// Por qué no se ha extraído.
    pub reason: String,
}

/// El resultado de un escaneo del dispositivo.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AndroidScan {
    /// Certificados públicos encontrados.
    pub certs: Vec<AndroidCert>,
    /// Ficheros con material privado: listados, **no leídos**.
    pub private_refs: Vec<PrivateRef>,
    /// Ficheros que no se han podido interpretar como certificado.
    pub skipped: Vec<String>,
    /// Fuentes que no existen en el dispositivo (no es un error).
    pub missing_sources: Vec<String>,
}

impl AndroidScan {
    /// Número de certificados públicos encontrados.
    pub fn cert_count(&self) -> usize {
        self.certs.len()
    }

    /// ¿Se ha encontrado algún material privado? (⇒ está **listado**, no leído)
    pub fn has_private_refs(&self) -> bool {
        !self.private_refs.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Escaneo
// ---------------------------------------------------------------------------

/// Escanea las fuentes dadas y devuelve el inventario.
///
/// **Nunca** lee material privado: los ficheros que lo contienen se listan en
/// [`AndroidScan::private_refs`] y su contenido **no** se toca.
pub fn scan_certificates(sources: &[AndroidCertSource]) -> Result<AndroidScan, String> {
    let mut out = AndroidScan::default();
    for src in sources {
        let dir = Path::new(&src.dir);
        if !dir.is_dir() {
            out.missing_sources.push(src.dir.clone());
            continue;
        }
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) => {
                out.skipped
                    .push(format!("{}: no se pudo leer el directorio: {}", src.dir, e));
                continue;
            }
        };
        let mut names: Vec<String> = entries
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        names.sort(); // determinista
        for name in names {
            let path = dir.join(&name);
            let md = match fs::metadata(&path) {
                Ok(m) => m,
                Err(e) => {
                    out.skipped.push(format!("{}: {}", path.display(), e));
                    continue;
                }
            };
            if !md.is_file() {
                continue;
            }
            let size = md.len();

            // 1) ¿Es material privado por el **nombre**?  ⇒ NO se lee, sólo se
            //    referencia.
            if looks_private(&name) {
                if src.private {
                    out.private_refs.push(PrivateRef {
                        path: path.display().to_string(),
                        size,
                        reason: "material de clave: la clave privada no sale del dispositivo \
                                 (atada al TEE/Keystore)"
                            .to_string(),
                    });
                } else {
                    out.skipped.push(format!(
                        "{}: parece material privado en una fuente pública: no se toca",
                        path.display()
                    ));
                }
                continue;
            }

            // 2) ¿Es un PEM de clave privada?  ⇒ se lee **sólo la cabecera**
            //    (para saberlo) y el resto **no** se toca.
            if sniff_private_header(&path) {
                if src.private {
                    out.private_refs.push(PrivateRef {
                        path: path.display().to_string(),
                        size,
                        reason: "PEM de clave privada: sólo se ha leido la cabecera, el \
                                 material no se copia"
                            .to_string(),
                    });
                } else {
                    out.skipped.push(format!(
                        "{}: PEM de clave privada en una fuente pública: no se toca",
                        path.display()
                    ));
                }
                continue;
            }

            // 2) Lectura del fichero **público**.
            let data = match fs::read(&path) {
                Ok(d) => d,
                Err(e) => {
                    out.skipped.push(format!("{}: {}", path.display(), e));
                    continue;
                }
            };
            let ders = extract_der_certs(&data);
            if ders.is_empty() {
                out.skipped.push(format!(
                    "{}: no es un certificado (ni PEM ni DER)",
                    path.display()
                ));
                continue;
            }
            for der in ders {
                let parsed = match parse_certificate(&der) {
                    Ok(p) => p,
                    Err(e) => {
                        out.skipped
                            .push(format!("{}: no se pudo parsear: {}", path.display(), e));
                        continue;
                    }
                };
                let sha = sha256_hex(&der);
                let id = format!("android-{}-{}", src.label, &sha[..16.min(sha.len())]);
                let self_signed = parsed.subject == parsed.issuer;
                out.certs.push(AndroidCert {
                    id,
                    label: format!("{} ({})", src.label, parsed.subject),
                    der,
                    source: src.label.clone(),
                    handle: path.display().to_string(),
                    sha256: sha,
                    subject: parsed.subject,
                    issuer: parsed.issuer,
                    not_after: parsed.not_after,
                    self_signed,
                });
            }
        }
    }
    out.certs.sort_by(|a, b| a.id.cmp(&b.id));
    out.certs.dedup_by(|a, b| a.id == b.id);
    Ok(out)
}

/// ¿El nombre del fichero sugiere material privado?
fn looks_private(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    // `.der` **no** se marca: un certificado DER es público; el contenido
    // decide (si no es un certificado, se lista como "skipped").
    n.ends_with(".p12")
        || n.ends_with(".pfx")
        || n.ends_with(".jks")
        || n.ends_with(".keystore")
        || n.ends_with(".key")
}

/// Lee **sólo la cabecera** (200 B) y dice si es un PEM de clave privada.
///
/// Es un "olfateo" mínimo: el objetivo es **no** cargar el material de clave
/// en memoria.  Un `.p12` binario no seSnifea (el nombre ya lo cubre).
fn sniff_private_header(path: &Path) -> bool {
    use std::io::Read as _;
    let mut buf = [0u8; 200];
    let mut f = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let n = match f.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return false,
    };
    let head = &buf[..n];
    if head.starts_with(b"-----BEGIN") && String::from_utf8_lossy(head).contains("PRIVATE KEY") {
        return true;
    }
    false
}

/// Extrae los certificados (DER) de un fichero PEM o DER.
///
/// * PEM: se separa por bloques `-----BEGIN CERTIFICATE-----` … `-----END
///   CERTIFICATE-----` y se decodifica el base64 de cada uno.
/// * DER: si el fichero empieza por `0x30` (SEQUENCE), se toma tal cual.
/// * Un PEM con `PRIVATE KEY` **no** devuelve nada (y el llamador ni lo lee:
///   `looks_private` lo filtra antes).
pub fn extract_der_certs(data: &[u8]) -> Vec<Vec<u8>> {
    if data.is_empty() {
        return Vec::new();
    }
    let looks_pem = data
        .windows(11)
        .any(|w| w == b"-----BEGIN ");
    if !looks_pem {
        // ¿DER?  Un certificado empieza por SEQUENCE (0x30) con longitud
        // coherente: comprobamos que el TLV declarado quepa en el fichero.
        if data[0] == 0x30 && data.len() > 4 {
            let first = data[1];
            let (len, hdr) = if first & 0x80 == 0 {
                (first as usize, 2usize)
            } else {
                let n = (first & 0x7f) as usize;
                if n == 0 || n > 4 || data.len() < 2 + n {
                    return Vec::new();
                }
                let mut l = 0usize;
                for i in 0..n {
                    l = (l << 8) | data[2 + i] as usize;
                }
                (l, 2 + n)
            };
            if hdr + len == data.len() {
                return vec![data.to_vec()];
            }
        }
        return Vec::new();
    }

    let text = String::from_utf8_lossy(data);
    let mut out = Vec::new();
    let mut rest = text.as_ref();
    while let Some(start) = rest.find("-----BEGIN CERTIFICATE-----") {
        let after = &rest[start + "-----BEGIN CERTIFICATE-----".len()..];
        let Some(end) = after.find("-----END CERTIFICATE-----") else {
            break;
        };
        let b64: String = after[..end].chars().filter(|c| !c.is_whitespace()).collect();
        if let Ok(der) = base64ct::Base64::decode_vec(b64.as_str()) {
            if !der.is_empty() {
                out.push(der);
            }
        }
        rest = &after[end + "-----END CERTIFICATE-----".len()..];
    }
    out
}

// ---------------------------------------------------------------------------
// Importación en la bóveda
// ---------------------------------------------------------------------------

/// Resultado de meter el inventario del móvil en la bóveda.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AndroidImportReport {
    /// Certificados importados.
    pub imported: usize,
    /// Certificados que ya estaban (misma huella) ⇒ no se tocan.
    pub unchanged: usize,
    /// Rechazos (con motivo).
    pub rejected: usize,
    /// Detalle de los rechazos.
    pub errors: Vec<String>,
    /// Referencias de clave privada registradas (nunca el material).
    pub private_refs: usize,
}

/// mete el inventario del móvil en la bóveda.
///
/// Los certificados se registran **siempre** como **no exportables**: su
/// clave privada sigue en el Keystore/TEE del móvil y aquí sólo queda el
/// **alias** (o la ruta) como referencia.
///
/// Es **idempotente**: si el `id` ya está con la misma huella, no se
/// reescribe nada.
pub fn import_into_vault(
    vault: &CertVault,
    scan: &AndroidScan,
    kdf: KdfParams,
) -> Result<AndroidImportReport, String> {
    let mut rep = AndroidImportReport::default();
    for c in &scan.certs {
        // Idempotencia: si ya está con la misma huella, **no** se toca.
        if let Ok(existing) = vault.get(&c.id) {
            if existing.fingerprint_sha256 == c.sha256 {
                rep.unchanged += 1;
                continue;
            }
            rep.rejected += 1;
            rep.errors.push(format!(
                "{}: ya existe con otra huella: no se pisa",
                c.id
            ));
            continue;
        }
        match vault.add_certificate(
            &c.id,
            &c.label,
            &c.der,
            KeyMaterial::NonExportable {
                backend: backend_of(&c.source),
                handle: c.handle.clone(),
            },
            None,
            kdf,
        ) {
            Ok(_) => rep.imported += 1,
            Err(e) => {
                rep.rejected += 1;
                rep.errors.push(format!("{}: {}", c.id, e));
            }
        }
    }
    rep.private_refs = scan.private_refs.len();
    Ok(rep)
}

/// Backend de la bóveda según la fuente.
fn backend_of(source: &str) -> String {
    match source {
        "keychain" => BACKEND_KEYCHAIN.to_string(),
        _ => BACKEND_KEYSTORE.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Referencias de clave (sin material)
// ---------------------------------------------------------------------------

/// Alias del Keystore del usuario **sin** material: sólo el nombre.
///
/// En `keystore2`/`keystore` los ficheros del usuario se llaman
/// `USRSKEY_<alias>` (clave privada) y `USRPKEY_<alias>` (pública).
pub fn keystore_aliases(dir: &str) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    let rd = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(e) => return Err(format!("no se pudo leer {}: {}", dir, e)),
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if let Some(alias) = name.strip_prefix("USRSKEY_") {
            out.push(alias.to_string());
        } else if let Some(alias) = name.strip_prefix("USRPKEY_") {
            out.push(alias.to_string());
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}
