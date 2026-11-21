//! **Bóveda de certificados del usuario y sincronización entre nodos**
//! (`cert_vault`).
//!
//! QUÉ ES:
//!
//!   * Un **inventario** de los certificados del usuario: id, etiqueta,
//!     emisor, sujeto, caducidad, algoritmo, huella SHA-256 del DER y cómo
//!     está guardada la clave privada.
//!   * Dos modos de guardar la clave privada, y sólo dos:
//!       (a) **NO exportable** → se guarda **sólo la referencia** (alias del
//!           Android Keystore / TPM / PKCS#11).  El material **no sale del
//!           dispositivo**: no hay bytes de clave en el disco, ni cifrados ni
//!           sin cifrar.
//!       (b) **exportable** → los bytes de la clave se cifran con
//!           **AEGIS-256** (`crate::aegis`) y la clave de cifrado se deriva con
//!           **Argon2id** (`crate::argon2::derive_key`, el KDF canónico del
//!           repo) con **sal aleatoria por clave**.  El fichero va con
//!           permisos 0600.
//!   * **Exportación / importación** con política: sacar una clave privada
//!     **exige `confirm: true` explícito** y deja **traza en el log de
//!     auditoría** (nunca la frase de paso ni los bytes de la clave).
//!   * **Sincronización entre nodos**: manifiesto con **SHA-256 por
//!     elemento**, transferencia en **streaming**, **idempotente**, y por
//!     defecto sólo viajan **certificados públicos + referencias de clave no
//!     exportable**.  La clave privada exportable sólo se replica si el nodo
//!     destino la pide **de forma explícita** y con la **misma política de
//!     `confirm`**.
//!
//! DÓNDE VIVE:
//!
//! ```text
//! <state_dir>/cert_vault/
//!     inventory.json      metadatos de los certificados (0600)
//!     manifest.json       manifiesto de sincronización (0600)
//!     audit.log           traza de exportación / importación / sync (0600)
//!     certs/<id>.der      certificado público en DER (0600)
//!     keys/<id>.key       clave privada CIFRADA (AEGIS-256) (0600)
//! ```
//!
//! POR QUÉ NO HAY CRATES NUEVOS:
//!
//! `rust-core/Cargo.toml` **no** trae `x509-parser` (comprobado), así que el
//! `notAfter` y el resto del X.509 se parsean **a mano** con un lector DER
//! mínimo de este mismo fichero (`der_tlv`): sin dependencias nuevas y sin
//! `openssl` en tiempo de ejecución.  El `openssl` del sistema sólo se usó
//! para generar el certificado de prueba **congelado** que vive en los tests.
//!
//! LÍMITES HONESTOS (NO VERIFICADO / fuera de alcance):
//!
//!   * El módulo **no comprueba que la clave privada corresponda al
//!     certificado** (no valida el par público/privado).  Habría que derivar
//!     la clave pública del DER y compararla con el `SubjectPublicKeyInfo`.
//!   * El módulo **no habla con** ninguna sede electrónica: no hay mTLS, no
//!     hay PKCS#11, no hay descarga de CRL/OCSP.  Eso vive en
//!     `crate::signing` (OpenSC/PKCS#11) y en el despliegue descrito en
//!     `docs/CERT_SYNC_EN_NODOS.md`.
//!   * El estado de caducidad (`valid` / `expiring` / `expired`) se calcula
//!     **desde el `notAfter` del DER**, con el instante actual **inyectado**
//!     en las funciones puras ⇒ los tests son deterministas.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Constantes
// ---------------------------------------------------------------------------

/// Nombre del directorio de la bóveda dentro del `state_dir`.
pub const VAULT_DIR: &str = "cert_vault";
/// Fichero del inventario (metadatos, ningún secreto).
pub const INVENTORY_NAME: &str = "inventory.json";
/// Fichero del manifiesto de sincronización.
pub const MANIFEST_NAME: &str = "manifest.json";
/// Fichero de la traza de auditoría (NDJSON).
pub const AUDIT_NAME: &str = "audit.log";
/// Subdirectorio de los certificados públicos (DER).
pub const CERTS_DIR: &str = "certs";
/// Subdirectorio de las claves privadas cifradas.
pub const KEYS_DIR: &str = "keys";

/// Versión del formato del inventario.
pub const INVENTORY_VERSION: u32 = 1;
/// Versión del formato del manifiesto.
pub const MANIFEST_VERSION: u32 = 1;

/// Magic del paquete de exportación de UN certificado.
pub const EXPORT_MAGIC: &[u8; 4] = b"MDC1";
/// Magic del paquete de sincronización entre nodos.
pub const SYNC_MAGIC: &[u8; 4] = b"MDCS";
/// Magic del blob de clave privada cifrada.
pub const KEY_MAGIC: &[u8; 4] = b"MDK1";

/// Longitud de la sal del KDF (Argon2id).
pub const SALT_LEN: usize = 16;
/// Longitud del nonce de AEGIS-256.
pub const NONCE_LEN: usize = 32;
/// Longitud de la etiqueta de AEGIS-256.
pub const TAG_LEN: usize = 16;
/// Longitud de la clave derivada.
pub const DERIVED_KEY_LEN: usize = 32;

/// Datos asociados (AAD) del blob de clave: ata el ciphertext al `id`.
pub const KEY_AAD_PREFIX: &[u8] = b"megadaemon-cert-vault/KEY1";

/// Longitud máxima del `id` de un certificado.
pub const MAX_ID_LEN: usize = 64;
/// Longitud máxima de la etiqueta de un certificado.
pub const MAX_LABEL_LEN: usize = 120;

/// Días de margen por defecto para marcar un certificado como `expiring`.
pub const DEFAULT_WARN_DAYS: i64 = 30;

/// Tamaño de la ventana de streaming al copiar blobs.
const CHUNK: usize = 64 * 1024;

// ---------------------------------------------------------------------------
// Parámetros del KDF
// ---------------------------------------------------------------------------

/// Parámetros del KDF (Argon2id) con el que se cifra la clave privada.
///
/// Se guardan **dentro** del blob cifrado (`MDK1`), así que una clave
/// exportada a otro nodo se puede descifrar aunque el destino use otros
/// parámetros por defecto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    /// Pases de Argon2id (`t`).
    pub iterations: u32,
    /// Memoria en KiB (`m`).
    pub memory_kib: u32,
    /// Carriles (`p`).
    pub parallelism: u32,
    /// Longitud de la clave derivada en bytes.
    pub key_len: usize,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self::production()
    }
}

impl KdfParams {
    /// Parámetros de producción: 3 pases, 64 MiB, 4 carriles (RFC 9106 §4).
    pub fn production() -> Self {
        KdfParams {
            iterations: 3,
            memory_kib: 64 * 1024,
            parallelism: 4,
            key_len: DERIVED_KEY_LEN,
        }
    }

    /// Parámetros **de prueba**: coste mínimo (Argon2id necesita `m >= 8·p`).
    ///
    /// Sirven para que los tests sean deterministas **y** rápidos: la
    /// implementación de Argon2id del repo es de referencia y en *debug*
    /// 64 MiB × 3 pases tardan minutos.
    pub fn fast() -> Self {
        KdfParams {
            iterations: 1,
            memory_kib: 8,
            parallelism: 1,
            key_len: DERIVED_KEY_LEN,
        }
    }
}

// ---------------------------------------------------------------------------
// Inventario
// ---------------------------------------------------------------------------

/// Cómo está guardada la clave privada de un certificado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PrivateKeyRef {
    /// **No exportable**: sólo se guarda la referencia al almacén
    /// (Android Keystore / TPM / PKCS#11).  El material **no sale** del
    /// dispositivo y **no existe** en el disco.
    NonExportable {
        /// Almacén donde vive la clave (`android_keystore`, `tpm`, `pkcs11`…).
        backend: String,
        /// Referencia opaca dentro del almacén (alias, URI PKCS#11, slot…).
        handle: String,
    },
    /// **Exportable**: los bytes están cifrados con AEGIS-256 en
    /// `keys/<id>.key`; la clave de cifrado sale de Argon2id + frase de paso.
    Exportable {
        /// Almacén/origen de la clave (informativo).
        backend: String,
        /// Nombre del fichero con la clave cifrada, relativo a la bóveda.
        file: String,
    },
    /// **Exportable pero NO en este nodo**: el certificado público está
    /// aquí; el material de la clave vive en otro nodo (o no se pidió que
    /// viajara).  `sha256`/`len` identifican el blob cifrado donde esté.
    Remote {
        /// Almacén/origen de la clave (informativo).
        backend: String,
        /// SHA-256 del blob cifrado de la clave.
        sha256: String,
        /// Longitud del blob cifrado de la clave.
        len: u64,
    },
}

impl PrivateKeyRef {
    /// ¿Hay material de clave privada **local** y exportable?
    pub fn is_exportable(&self) -> bool {
        matches!(self, PrivateKeyRef::Exportable { .. })
    }

    /// ¿Hay material de clave privada en este nodo (de algún tipo)?
    pub fn has_local_material(&self) -> bool {
        !matches!(self, PrivateKeyRef::Remote { .. })
    }

    /// Nombre estable del tipo (para manifiestos y logs).
    pub fn kind(&self) -> &'static str {
        match self {
            PrivateKeyRef::NonExportable { .. } => "non_exportable",
            PrivateKeyRef::Exportable { .. } => "exportable",
            PrivateKeyRef::Remote { .. } => "remote_exportable",
        }
    }
}

/// Una entrada del inventario: un certificado del usuario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertificateEntry {
    /// Identificador local (único en la bóveda).
    pub id: String,
    /// Etiqueta legible ("FNMT persona física", "DNIe", ...).
    pub label: String,
    /// Emisor (DN del `issuer`), en texto.
    pub issuer: String,
    /// Sujeto (DN del `subject`), en texto.
    pub subject: String,
    /// Número de serie en hexadecimal.
    pub serial_number: String,
    /// `notBefore` (epoch, segundos UTC).
    pub not_before: i64,
    /// `notAfter` (epoch, segundos UTC).
    pub not_after: i64,
    /// Algoritmo de firma del certificado (texto legible).
    pub signature_algorithm: String,
    /// Algoritmo de la clave pública (texto legible).
    pub public_key_algorithm: String,
    /// Huella **SHA-256 del DER** en hexadecimal.
    pub fingerprint_sha256: String,
    /// Fichero con el DER del certificado, relativo a la bóveda.
    pub der_file: String,
    /// Cómo está guardada la clave privada.
    pub key: PrivateKeyRef,
    /// Epoch de alta en la bóveda.
    pub added_at: i64,
}

impl CertificateEntry {
    /// Estado de caducidad respecto a `now`: `valid` | `expiring` | `expired`.
    pub fn status_at(&self, now: i64, warn_days: i64) -> &'static str {
        expiry_status(self.not_after, now, warn_days)
    }
}

/// Estado de caducidad calculado desde el `notAfter` del DER.
///
/// * `expired`  → ya ha pasado `not_after`.
/// * `expiring` → caduca dentro de `warn_days`.
/// * `valid`    → en vigor.
pub fn expiry_status(not_after: i64, now: i64, warn_days: i64) -> &'static str {
    if now > not_after {
        "expired"
    } else if now + warn_days * 86_400 >= not_after {
        "expiring"
    } else {
        "valid"
    }
}

/// El inventario completo de la bóveda.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    /// Versión del formato.
    pub version: u32,
    /// Epoch de la última escritura.
    pub updated_at: i64,
    /// Certificados del usuario.
    pub certificates: Vec<CertificateEntry>,
}

// ---------------------------------------------------------------------------
// Manifiesto de sincronización
// ---------------------------------------------------------------------------

/// Cómo aparece una clave en el manifiesto.
///
/// Importante: el manifiesto describe **lo que hay en la bóveda**, no lo que
/// viaja en un paquete concreto ⇒ el mismo inventario da **siempre** el mismo
/// manifiesto (y el mismo SHA-256), sea cual sea la política de réplica.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ManifestKey {
    /// Referencia a una clave **no exportable**: sólo viaja el puntero al
    /// almacén; el material **no viaja nunca**.
    NonExportable {
        /// Almacén donde vive la clave.
        backend: String,
        /// Referencia opaca dentro del almacén.
        handle: String,
    },
    /// Clave **exportable**: los bytes están cifrados en el nodo origen.
    ///
    /// `sha256` es la huella del blob **cifrado** (nunca de la clave en
    /// claro) e identifica el material donde esté.  `present` dice si ese
    /// blob está **en este nodo**: `false` ⇒ el certificado está aquí pero la
    /// clave vive en otro nodo.
    Exportable {
        /// Almacén/origen (informativo).
        backend: String,
        /// SHA-256 del blob cifrado.
        sha256: String,
        /// Longitud del blob cifrado.
        len: u64,
        /// ¿El blob cifrado está en este nodo?
        present: bool,
    },
}

/// Una entrada del manifiesto de sincronización.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    /// Identificador del certificado.
    pub id: String,
    /// Etiqueta legible.
    pub label: String,
    /// SHA-256 del DER del certificado.
    pub der_sha256: String,
    /// Longitud del DER.
    pub der_len: u64,
    /// `notAfter` (epoch).
    pub not_after: i64,
    /// `notBefore` (epoch).
    pub not_before: i64,
    /// Clave privada asociada (ver [`ManifestKey`]).
    pub key: ManifestKey,
}

/// El manifiesto de sincronización: qué hay en este nodo y con qué huella.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncManifest {
    /// Versión del formato.
    pub version: u32,
    /// Identificador del nodo que genera el manifiesto.
    pub node: String,
    /// Entradas (ordenadas por `id`).
    pub entries: Vec<ManifestEntry>,
    /// SHA-256 del cuerpo canónico (las entradas ordenadas).
    pub digest: String,
}

impl SyncManifest {
    /// Calcula (y fija) el digest canónico del manifiesto.
    ///
    /// El cuerpo canónico es el JSON de las entradas **ordenadas por `id`**:
    /// no incluye marcas de tiempo ni el nombre del nodo ⇒ dos nodos con el
    /// mismo contenido dan **exactamente** el mismo SHA-256.
    pub fn compute_digest(&mut self) {
        self.entries.sort_by(|a, b| a.id.cmp(&b.id));
        let body = serde_json::to_string(&self.entries).unwrap_or_default();
        self.digest = sha256_hex(body.as_bytes());
    }
}

/// Opciones de una exportación para otro nodo.
#[derive(Debug, Clone, Copy, Default)]
pub struct SyncOptions {
    /// El nodo destino **pide explícitamente** las claves privadas
    /// exportables.
    pub include_exportable_keys: bool,
    /// Confirmación explícita del usuario.  **Obligatoria** si se pide
    /// material de clave privada.
    pub confirm: bool,
}

/// Resultado de importar un paquete de sincronización.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SyncReport {
    /// Entradas importadas de verdad.
    pub imported: usize,
    /// Entradas que ya estaban (idempotencia: no se reescriben).
    pub unchanged: usize,
    /// Entradas rechazadas (con motivo en `errors`).
    pub rejected: usize,
    /// Motivos de rechazo.
    pub errors: Vec<String>,
    /// Digest del manifiesto **después** de importar.
    pub manifest_digest: String,
}

// ---------------------------------------------------------------------------
// La bóveda
// ---------------------------------------------------------------------------

/// Material de clave privada que se entrega a la bóveda al dar de alta un
/// certificado.
#[derive(Debug, Clone)]
pub enum KeyMaterial {
    /// Clave **no exportable**: la bóveda guarda **sólo** la referencia.
    NonExportable {
        /// Almacén donde vive la clave.
        backend: String,
        /// Referencia opaca (alias/URI).
        handle: String,
    },
    /// Clave **exportable**: se cifra con AEGIS-256 y se guarda en disco.
    Exportable {
        /// Almacén/origen (informativo).
        backend: String,
        /// Bytes de la clave privada (se cifran; nunca se guardan en claro).
        key_bytes: Vec<u8>,
    },
}

/// La bóveda de certificados.
pub struct CertVault {
    root: PathBuf,
}

impl CertVault {
    /// Abre (o crea) la bóveda bajo `state_dir`.
    ///
    /// Directorios con 0700 y ficheros con 0600: son secretos del usuario.
    pub fn new(state_dir: &Path) -> Result<Self, String> {
        let root = state_dir.join(VAULT_DIR);
        fs::create_dir_all(root.join(CERTS_DIR)).map_err(|e| format!("mkdir certs: {}", e))?;
        fs::create_dir_all(root.join(KEYS_DIR)).map_err(|e| format!("mkdir keys: {}", e))?;
        set_mode(&root, 0o700);
        set_mode(&root.join(CERTS_DIR), 0o700);
        set_mode(&root.join(KEYS_DIR), 0o700);
        Ok(CertVault { root })
    }

    /// Raíz de la bóveda.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Ruta del inventario.
    pub fn inventory_path(&self) -> PathBuf {
        self.root.join(INVENTORY_NAME)
    }

    /// Ruta del manifiesto.
    pub fn manifest_path(&self) -> PathBuf {
        self.root.join(MANIFEST_NAME)
    }

    /// Ruta del log de auditoría.
    pub fn audit_path(&self) -> PathBuf {
        self.root.join(AUDIT_NAME)
    }

    // -- inventario ---------------------------------------------------------

    /// Lee el inventario (vacío si no existe).
    pub fn inventory(&self) -> Result<Inventory, String> {
        let path = self.inventory_path();
        if !path.exists() {
            return Ok(Inventory {
                version: INVENTORY_VERSION,
                updated_at: now(),
                certificates: Vec::new(),
            });
        }
        let raw = fs::read(&path).map_err(|e| format!("lectura del inventario: {}", e))?;
        let inv: Inventory =
            serde_json::from_slice(&raw).map_err(|e| format!("inventario ilegible: {}", e))?;
        Ok(inv)
    }

    /// Escribe el inventario (0600, atómico).
    pub fn write_inventory(&self, inv: &Inventory) -> Result<(), String> {
        let mut inv = inv.clone();
        inv.version = INVENTORY_VERSION;
        inv.updated_at = now();
        inv.certificates.sort_by(|a, b| a.id.cmp(&b.id));
        let bytes = serde_json::to_vec_pretty(&inv).map_err(|e| format!("serialización: {}", e))?;
        write_atomic(&self.inventory_path(), &bytes, 0o600)
    }

    /// Lista los certificados del inventario.
    pub fn list(&self) -> Result<Vec<CertificateEntry>, String> {
        let mut inv = self.inventory()?;
        inv.certificates.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(inv.certificates)
    }

    /// Una entrada del inventario por `id`.
    pub fn get(&self, id: &str) -> Result<CertificateEntry, String> {
        let safe = sanitize_id(id)?;
        self.list()?
            .into_iter()
            .find(|c| c.id == safe)
            .ok_or_else(|| format!("no existe el certificado: {}", safe))
    }

    /// Ruta absoluta del DER de un certificado.
    pub fn der_path(&self, id: &str) -> PathBuf {
        self.root.join(CERTS_DIR).join(format!("{}.der", id))
    }

    /// Lee el DER de un certificado y **verifica** su SHA-256 contra el
    /// inventario (fail-closed: si el fichero cambió, error).
    pub fn read_der(&self, entry: &CertificateEntry) -> Result<Vec<u8>, String> {
        let path = self.root.join(&entry.der_file);
        let data = fs::read(&path).map_err(|e| format!("lectura de {}: {}", entry.der_file, e))?;
        let real = sha256_hex(&data);
        if real != entry.fingerprint_sha256 {
            return Err(format!(
                "sha256 del DER no coincide (inventario {} vs disco {}): {}",
                entry.fingerprint_sha256, real, entry.id
            ));
        }
        Ok(data)
    }

    // -- alta de certificados ------------------------------------------------

    /// Da de alta un certificado.
    ///
    /// * Con [`KeyMaterial::NonExportable`] **no se escribe ningún byte de
    ///   clave**: sólo la referencia.
    /// * Con [`KeyMaterial::Exportable`] los bytes se cifran con AEGIS-256 y
    ///   una clave derivada con Argon2id de `passphrase` (sal aleatoria por
    ///   clave) ⇒ en disco **nunca** hay material en claro.
    pub fn add_certificate(
        &self,
        id: &str,
        label: &str,
        der: &[u8],
        key: KeyMaterial,
        passphrase: Option<&str>,
        kdf: KdfParams,
    ) -> Result<CertificateEntry, String> {
        let safe_id = sanitize_id(id)?;
        let safe_label = sanitize_label(label)?;
        let parsed = parse_certificate(der)?;

        let key_ref = match key {
            KeyMaterial::NonExportable { backend, handle } => PrivateKeyRef::NonExportable {
                backend: backend.trim().to_string(),
                handle: handle.trim().to_string(),
            },
            KeyMaterial::Exportable { backend, key_bytes } => {
                let pass = passphrase
                    .ok_or_else(|| "una clave exportable necesita frase de paso".to_string())?;
                if pass.is_empty() {
                    return Err("la frase de paso no puede estar vacía".to_string());
                }
                let blob = wrap_private_key(&safe_id, &key_bytes, pass, &kdf)?;
                let file = format!("{}/{}", KEYS_DIR, format!("{}.key", safe_id));
                write_atomic(&self.root.join(&file), &blob, 0o600)?;
                PrivateKeyRef::Exportable {
                    backend: backend.trim().to_string(),
                    file,
                }
            }
        };

        let der_file = format!("{}/{}.der", CERTS_DIR, safe_id);
        write_atomic(&self.root.join(&der_file), der, 0o600)?;

        let entry = CertificateEntry {
            id: safe_id.clone(),
            label: safe_label,
            issuer: parsed.issuer,
            subject: parsed.subject,
            serial_number: parsed.serial_number,
            not_before: parsed.not_before,
            not_after: parsed.not_after,
            signature_algorithm: parsed.signature_algorithm,
            public_key_algorithm: parsed.public_key_algorithm,
            fingerprint_sha256: sha256_hex(der),
            der_file,
            key: key_ref,
            added_at: now(),
        };

        let mut inv = self.inventory()?;
        inv.certificates.retain(|c| c.id != safe_id);
        inv.certificates.push(entry.clone());
        self.write_inventory(&inv)?;
        self.audit(
            "add",
            &safe_id,
            &format!(
                "certificado dado de alta (clave {}, huella {})",
                entry.key.kind(),
                entry.fingerprint_sha256
            ),
        )?;
        Ok(entry)
    }

    /// Borra un certificado de la bóveda (certificado y clave cifrada).
    pub fn remove_certificate(&self, id: &str, confirm: bool) -> Result<(), String> {
        let safe_id = sanitize_id(id)?;
        if !confirm {
            return Err(format!(
                "borrar el certificado '{}' exige confirm: true",
                safe_id
            ));
        }
        let entry = self.get(&safe_id)?;
        let _ = fs::remove_file(self.root.join(&entry.der_file));
        if let PrivateKeyRef::Exportable { file, .. } = &entry.key {
            let _ = fs::remove_file(self.root.join(file));
        }
        let mut inv = self.inventory()?;
        inv.certificates.retain(|c| c.id != safe_id);
        self.write_inventory(&inv)?;
        self.audit("remove", &safe_id, "certificado eliminado de la bóveda")?;
        Ok(())
    }

    // -- claves privadas -----------------------------------------------------

    /// Abre (descifra) la clave privada exportable de un certificado.
    ///
    /// El material devuelto **no** se persiste en claro en ningún sitio: es
    /// para usarlo en memoria (p. ej. un handshake mTLS).
    pub fn unlock_private_key(&self, id: &str, passphrase: &str) -> Result<Vec<u8>, String> {
        let entry = self.get(id)?;
        let file = match &entry.key {
            PrivateKeyRef::Exportable { file, .. } => file.clone(),
            PrivateKeyRef::NonExportable { backend, .. } => {
                return Err(format!(
                    "la clave de '{}' no es exportable (backend {}): no hay material que abrir",
                    entry.id, backend
                ))
            }
            PrivateKeyRef::Remote {
                backend, sha256, ..
            } => {
                return Err(format!(
                    "la clave privada de '{}' NO está en este nodo (backend {}; vive en el nodo \
                     origen, sha256 {}): pide al nodo origen que te la replique con confirmación",
                    entry.id, backend, sha256
                ))
            }
        };
        let blob =
            fs::read(self.root.join(&file)).map_err(|e| format!("lectura de clave: {}", e))?;
        unwrap_private_key(&entry.id, &blob, passphrase)
    }

    // -- exportación / importación -------------------------------------------

    /// Exporta **un** certificado a un paquete portable.
    ///
    /// * Si la clave es **no exportable**, el paquete lleva el certificado
    ///   público y la **referencia** al almacén (el material no sale).
    /// * Si la clave es **exportable**, el paquete lleva además el material
    ///   cifrado, y **exige `confirm: true`**: sin confirmación la función
    ///   falla **antes** de tocar nada y deja traza en la auditoría.
    ///
    /// La `passphrase` es la misma con la que se cifró la clave: sirve para
    /// **desencriptar y re-cifrar** el material con sal nueva, y de paso
    /// demuestra que quien llama tiene la frase de paso.
    pub fn export_certificate(
        &self,
        id: &str,
        passphrase: &str,
        confirm: bool,
        kdf: KdfParams,
    ) -> Result<Vec<u8>, String> {
        let entry = self.get(id)?;
        let der = self.read_der(&entry)?;

        // Identificador del material cifrado de la clave (si la hay): sirve
        // para que el destino sepa **qué** material es aunque no viaje.
        let (local_sha, local_len) = match &entry.key {
            PrivateKeyRef::Exportable { file, .. } => {
                let blob = fs::read(self.root.join(file))
                    .map_err(|e| format!("lectura de la clave cifrada: {}", e))?;
                (sha256_hex(&blob), blob.len() as u64)
            }
            _ => (String::new(), 0),
        };

        let mut wrapped: Option<(Vec<u8>, String)> = None;
        if entry.key.is_exportable() {
            if !confirm {
                self.audit(
                    "export_denied",
                    &entry.id,
                    "exportación de clave privada rechazada: falta confirm: true",
                )?;
                return Err(format!(
                    "exportar la clave privada de '{}' exige confirm: true",
                    entry.id
                ));
            }
            if passphrase.is_empty() {
                return Err("la frase de paso no puede estar vacía".to_string());
            }
            let plain = self.unlock_private_key(&entry.id, passphrase)?;
            let blob = wrap_private_key(&entry.id, &plain, passphrase, &kdf)?;
            let sha = sha256_hex(&blob);
            wrapped = Some((blob, sha));
        }

        let header = ExportHeader {
            version: 1,
            node: node_id(),
            created_at: now(),
            cert: ExportCert {
                id: entry.id.clone(),
                label: entry.label.clone(),
                der_sha256: entry.fingerprint_sha256.clone(),
                der_len: der.len() as u64,
                key: export_key_ref(&entry.key, &local_sha),
                wrapped_sha256: local_sha,
                wrapped_len: local_len,
                include_wrapped: wrapped.is_some(),
            },
        };
        let header_bytes = serde_json::to_vec(&header)
            .map_err(|e| format!("serialización de la cabecera: {}", e))?;

        let mut out: Vec<u8> = Vec::with_capacity(header_bytes.len() + der.len() + 64);
        out.extend_from_slice(EXPORT_MAGIC);
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&(header_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&header_bytes);
        out.extend_from_slice(&der);
        if let Some((blob, _)) = &wrapped {
            out.extend_from_slice(blob);
        }

        self.audit(
            "export",
            &entry.id,
            &format!(
                "exportado (clave {}; material privado incluido: {})",
                entry.key.kind(),
                if wrapped.is_some() { "si" } else { "no" }
            ),
        )?;
        Ok(out)
    }

    /// Importa un paquete de exportación.
    ///
    /// * El SHA-256 del DER se **verifica** antes de escribir nada.
    /// * Si el paquete trae material de clave privada, hace falta
    ///   `confirm: true` (misma política que la exportación) y, si se pasa
    ///   `passphrase`, se **descifra** para comprobar que la frase es correcta
    ///   (fail-closed).
    pub fn import_certificate(
        &self,
        bundle: &[u8],
        passphrase: Option<&str>,
        confirm: bool,
    ) -> Result<CertificateEntry, String> {
        let (header, der, wrapped) = parse_export_bundle(bundle)?;
        if header.cert.include_wrapped {
            if !confirm {
                self.audit(
                    "import_denied",
                    &header.cert.id,
                    "importación con clave privada rechazada: falta confirm: true",
                )?;
                return Err(format!(
                    "importar la clave privada de '{}' exige confirm: true",
                    header.cert.id
                ));
            }
            if let Some(p) = passphrase {
                // Comprobación de la frase de paso: si no descifra, no entra.
                let _ = unwrap_private_key(&header.cert.id, &wrapped, p)?;
            }
        }

        let id = sanitize_id(&header.cert.id)?;
        let label = sanitize_label(&header.cert.label)?;
        let parsed = parse_certificate(&der)?;
        if sha256_hex(&der) != header.cert.der_sha256 {
            return Err(format!("sha256 del DER no coincide: {}", id));
        }

        let der_file = format!("{}/{}.der", CERTS_DIR, id);
        write_atomic(&self.root.join(&der_file), &der, 0o600)?;

        let key = match &header.cert.key {
            ExportKeyRef::NonExportable { backend, handle } => PrivateKeyRef::NonExportable {
                backend: backend.clone(),
                handle: handle.clone(),
            },
            ExportKeyRef::Exportable { backend, .. } => {
                if header.cert.include_wrapped {
                    let file = format!("{}/{}.key", KEYS_DIR, id);
                    write_atomic(&self.root.join(&file), &wrapped, 0o600)?;
                    PrivateKeyRef::Exportable {
                        backend: backend.clone(),
                        file,
                    }
                } else {
                    // El material no viaja: aquí sólo está el certificado.
                    PrivateKeyRef::Remote {
                        backend: backend.clone(),
                        sha256: header.cert.wrapped_sha256.clone(),
                        len: header.cert.wrapped_len,
                    }
                }
            }
        };

        let entry = CertificateEntry {
            id: id.clone(),
            label,
            issuer: parsed.issuer,
            subject: parsed.subject,
            serial_number: parsed.serial_number,
            not_before: parsed.not_before,
            not_after: parsed.not_after,
            signature_algorithm: parsed.signature_algorithm,
            public_key_algorithm: parsed.public_key_algorithm,
            fingerprint_sha256: sha256_hex(&der),
            der_file,
            key,
            added_at: now(),
        };

        let mut inv = self.inventory()?;
        inv.certificates.retain(|c| c.id != id);
        inv.certificates.push(entry.clone());
        self.write_inventory(&inv)?;
        self.audit(
            "import",
            &id,
            &format!(
                "importado (clave {}; material privado incluido: {})",
                entry.key.kind(),
                if header.cert.include_wrapped {
                    "si"
                } else {
                    "no"
                }
            ),
        )?;
        Ok(entry)
    }

    // -- sincronización entre nodos ------------------------------------------

    /// Construye el manifiesto de sincronización y lo escribe en disco.
    pub fn refresh_manifest(&self) -> Result<SyncManifest, String> {
        let manifest = self.build_manifest()?;
        let bytes =
            serde_json::to_vec_pretty(&manifest).map_err(|e| format!("serialización: {}", e))?;
        write_atomic(&self.manifest_path(), &bytes, 0o600)?;
        Ok(manifest)
    }

    /// Construye el manifiesto (sin escribirlo).
    pub fn build_manifest(&self) -> Result<SyncManifest, String> {
        let inv = self.inventory()?;
        let mut entries: Vec<ManifestEntry> = Vec::new();
        for c in &inv.certificates {
            let der_path = self.root.join(&c.der_file);
            let der =
                fs::read(&der_path).map_err(|e| format!("lectura de {}: {}", c.der_file, e))?;
            let key = match &c.key {
                PrivateKeyRef::NonExportable { backend, handle } => ManifestKey::NonExportable {
                    backend: backend.clone(),
                    handle: handle.clone(),
                },
                PrivateKeyRef::Exportable { backend, file } => {
                    let blob = fs::read(self.root.join(file))
                        .map_err(|e| format!("lectura de {}: {}", file, e))?;
                    ManifestKey::Exportable {
                        backend: backend.clone(),
                        sha256: sha256_hex(&blob),
                        len: blob.len() as u64,
                        present: true,
                    }
                }
                PrivateKeyRef::Remote {
                    backend,
                    sha256,
                    len,
                } => ManifestKey::Exportable {
                    backend: backend.clone(),
                    sha256: sha256.clone(),
                    len: *len,
                    present: false,
                },
            };
            entries.push(ManifestEntry {
                id: c.id.clone(),
                label: c.label.clone(),
                der_sha256: sha256_hex(&der),
                der_len: der.len() as u64,
                not_after: c.not_after,
                not_before: c.not_before,
                key,
            });
        }
        let mut manifest = SyncManifest {
            version: MANIFEST_VERSION,
            node: node_id(),
            entries,
            digest: String::new(),
        };
        manifest.compute_digest();
        Ok(manifest)
    }

    /// Escribe un paquete de sincronización para otro nodo (streaming).
    ///
    /// Por defecto viajan **certificados públicos + referencias de clave no
    /// exportable**.  La clave privada exportable sólo se replica si
    /// `opts.include_exportable_keys` **y** `opts.confirm` son true; en
    /// cualquier otro caso la función falla (no se replica a medias).
    pub fn write_sync_bundle<W: Write>(
        &self,
        w: &mut W,
        node: &str,
        opts: SyncOptions,
    ) -> Result<SyncManifest, String> {
        if opts.include_exportable_keys && !opts.confirm {
            return Err(
                "replicar claves privadas exportables exige confirm: true (política de la bóveda)"
                    .to_string(),
            );
        }
        let manifest = self.build_manifest()?;
        let inv = self.inventory()?;

        let mut header_entries: Vec<SyncHeaderEntry> = Vec::new();
        for m in &manifest.entries {
            let c = inv
                .certificates
                .iter()
                .find(|c| c.id == m.id)
                .ok_or_else(|| format!("certificador ausente del inventario: {}", m.id))?;
            let include_wrapped = opts.include_exportable_keys && c.key.is_exportable();
            // El hash y la longitud del blob cifrado se declaran **siempre**
            // (identifican el material), pero los bytes sólo viajan si el
            // destino lo pide con `confirm`.
            let (wrapped_sha, wrapped_len) = match &c.key {
                PrivateKeyRef::Exportable { file, .. } => {
                    let blob = fs::read(self.root.join(file))
                        .map_err(|e| format!("lectura de {}: {}", file, e))?;
                    (sha256_hex(&blob), blob.len() as u64)
                }
                PrivateKeyRef::Remote { sha256, len, .. } => (sha256.clone(), *len),
                PrivateKeyRef::NonExportable { .. } => (String::new(), 0),
            };
            header_entries.push(SyncHeaderEntry {
                id: m.id.clone(),
                label: m.label.clone(),
                der_sha256: m.der_sha256.clone(),
                der_len: m.der_len,
                key: sync_key_ref(&m.key),
                wrapped_sha256: wrapped_sha,
                wrapped_len,
                include_wrapped,
            });
        }

        let header = SyncHeader {
            version: 1,
            node: node.to_string(),
            created_at: now(),
            include_exportable_keys: opts.include_exportable_keys && opts.confirm,
            entries: header_entries,
        };
        let header_bytes = serde_json::to_vec(&header)
            .map_err(|e| format!("serialización de la cabecera: {}", e))?;

        w.write_all(SYNC_MAGIC)
            .map_err(|e| format!("escritura del paquete: {}", e))?;
        w.write_all(&1u32.to_le_bytes())
            .map_err(|e| format!("escritura del paquete: {}", e))?;
        w.write_all(&(header_bytes.len() as u32).to_le_bytes())
            .map_err(|e| format!("escritura del paquete: {}", e))?;
        w.write_all(&header_bytes)
            .map_err(|e| format!("escritura del paquete: {}", e))?;

        // Blobs: primero el DER de cada entrada y, si toca, la clave cifrada.
        for he in &header.entries {
            let c = inv
                .certificates
                .iter()
                .find(|c| c.id == he.id)
                .ok_or_else(|| format!("certificador ausente del inventario: {}", he.id))?;
            let der = fs::read(self.root.join(&c.der_file))
                .map_err(|e| format!("lectura de {}: {}", c.der_file, e))?;
            if der.len() as u64 != he.der_len || sha256_hex(&der) != he.der_sha256 {
                return Err(format!("el DER de {} no cuadra con el manifiesto", he.id));
            }
            stream_out(w, &der)?;
            if he.include_wrapped {
                if let PrivateKeyRef::Exportable { file, .. } = &c.key {
                    let blob = fs::read(self.root.join(file))
                        .map_err(|e| format!("lectura de {}: {}", file, e))?;
                    if blob.len() as u64 != he.wrapped_len || sha256_hex(&blob) != he.wrapped_sha256
                    {
                        return Err(format!("la clave cifrada de {} no cuadra", he.id));
                    }
                    stream_out(w, &blob)?;
                }
            }
        }

        self.audit(
            "sync_export",
            "*",
            &format!(
                "paquete hacia '{}' ({} entradas; material privado incluido: {})",
                node,
                header.entries.len(),
                if header.include_exportable_keys {
                    "si"
                } else {
                    "no"
                }
            ),
        )?;
        Ok(manifest)
    }

    /// Lee (importa) un paquete de sincronización (streaming).
    ///
    /// * El SHA-256 de cada elemento se **verifica** antes de escribirlo
    ///   (fail-closed por entrada: un elemento corrupto no toca el disco).
    /// * Es **idempotente**: si el `id` ya está con el mismo SHA-256, no se
    ///   reescribe (se cuenta como `unchanged`).
    /// * Si el `id` existe con **distinto** SHA-256, la entrada se **rechaza**
    ///   (no se pisa un certificado que el usuario ya tenía).
    pub fn read_sync_bundle<R: Read>(&self, r: &mut R) -> Result<SyncReport, String> {
        let mut buf: Vec<u8> = Vec::new();
        let mut magic = [0u8; 4];
        r.read_exact(&mut magic)
            .map_err(|e| format!("cabecera: {}", e))?;
        if &magic != SYNC_MAGIC {
            return Err("firma inválida: no es un paquete de sincronización".to_string());
        }
        let mut ver = [0u8; 4];
        r.read_exact(&mut ver)
            .map_err(|e| format!("cabecera: {}", e))?;
        let version = u32::from_le_bytes(ver);
        if version != 1 {
            return Err(format!("versión de paquete no soportada: {}", version));
        }
        let mut hl = [0u8; 4];
        r.read_exact(&mut hl)
            .map_err(|e| format!("cabecera: {}", e))?;
        let header_len = u32::from_le_bytes(hl) as usize;
        if header_len > 8 * 1024 * 1024 {
            return Err("cabecera desproporcionada".to_string());
        }
        buf.resize(header_len, 0);
        r.read_exact(&mut buf)
            .map_err(|e| format!("cabecera: {}", e))?;
        let header: SyncHeader =
            serde_json::from_slice(&buf).map_err(|e| format!("cabecera ilegible: {}", e))?;

        let mut report = SyncReport::default();
        for he in &header.entries {
            match self.import_sync_entry(r, he) {
                Ok(Outcome::Imported) => report.imported += 1,
                Ok(Outcome::Unchanged) => report.unchanged += 1,
                Err(e) => {
                    report.rejected += 1;
                    report.errors.push(format!("{}: {}", he.id, e));
                    // Si el elemento falla no se puede seguir leyendo el
                    // paquete (los bytes están desalineados) ⇒ se aborta.
                    return Err(format!("paquete de sincronización abortado: {}", e));
                }
            }
        }
        let manifest = self.refresh_manifest()?;
        report.manifest_digest = manifest.digest;
        self.audit(
            "sync_import",
            "*",
            &format!(
                "importado desde '{}' ({} nuevos, {} sin cambios, {} rechazados)",
                header.node, report.imported, report.unchanged, report.rejected
            ),
        )?;
        Ok(report)
    }

    /// Importa una entrada del paquete de sincronización.
    fn import_sync_entry<R: Read>(
        &self,
        r: &mut R,
        he: &SyncHeaderEntry,
    ) -> Result<Outcome, String> {
        let id = sanitize_id(&he.id)?;
        let der = read_exact_streaming(r, he.der_len)?;
        if sha256_hex(&der) != he.der_sha256 {
            return Err("sha256 del DER no coincide (fail-closed)".to_string());
        }
        let mut wrapped: Vec<u8> = Vec::new();
        if he.include_wrapped {
            wrapped = read_exact_streaming(r, he.wrapped_len)?;
            if !he.wrapped_sha256.is_empty() && sha256_hex(&wrapped) != he.wrapped_sha256 {
                return Err("sha256 de la clave cifrada no coincide (fail-closed)".to_string());
            }
        }

        // Idempotencia: si ya está con el mismo contenido, no se toca nada.
        if let Ok(existing) = self.get(&id) {
            if existing.fingerprint_sha256 == he.der_sha256 {
                return Ok(Outcome::Unchanged);
            }
            return Err(format!("ya existe '{}' con otro contenido: no se pisa", id));
        }

        let parsed = parse_certificate(&der)?;
        let label = sanitize_label(&he.label)?;
        let der_file = format!("{}/{}.der", CERTS_DIR, id);
        write_atomic(&self.root.join(&der_file), &der, 0o600)?;

        let key = match &he.key {
            SyncKeyRef::NonExportable { backend, handle } => PrivateKeyRef::NonExportable {
                backend: backend.clone(),
                handle: handle.clone(),
            },
            SyncKeyRef::Exportable { backend, .. } => {
                if he.include_wrapped {
                    let file = format!("{}/{}.key", KEYS_DIR, id);
                    write_atomic(&self.root.join(&file), &wrapped, 0o600)?;
                    PrivateKeyRef::Exportable {
                        backend: backend.clone(),
                        file,
                    }
                } else {
                    // El material no viaja: aquí sólo está el certificado
                    // público y la referencia de dónde está la clave.
                    PrivateKeyRef::Remote {
                        backend: backend.clone(),
                        sha256: he.wrapped_sha256.clone(),
                        len: he.wrapped_len,
                    }
                }
            }
        };

        let entry = CertificateEntry {
            id: id.clone(),
            label,
            issuer: parsed.issuer,
            subject: parsed.subject,
            serial_number: parsed.serial_number,
            not_before: parsed.not_before,
            not_after: parsed.not_after,
            signature_algorithm: parsed.signature_algorithm,
            public_key_algorithm: parsed.public_key_algorithm,
            fingerprint_sha256: sha256_hex(&der),
            der_file,
            key,
            added_at: now(),
        };
        let mut inv = self.inventory()?;
        inv.certificates.retain(|c| c.id != id);
        inv.certificates.push(entry);
        self.write_inventory(&inv)?;
        Ok(Outcome::Imported)
    }

    // -- auditoría -----------------------------------------------------------

    /// Añade una línea al log de auditoría (NDJSON, 0600, *append*).
    ///
    /// **Nunca** se escribe aquí la frase de paso ni los bytes de la clave:
    /// sólo el `id`, la huella y una descripción de la acción.
    pub fn audit(&self, event: &str, cert_id: &str, detail: &str) -> Result<(), String> {
        let rec = serde_json::json!({
            "ts": now(),
            "event": event,
            "cert_id": cert_id,
            "detail": detail.replace('\n', " "),
        });
        let line = format!("{}\n", serde_json::to_string(&rec).unwrap_or_default());
        let path = self.audit_path();
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| format!("apertura del log de auditoría: {}", e))?;
        set_mode(&path, 0o600);
        f.write_all(line.as_bytes())
            .map_err(|e| format!("escritura del log de auditoría: {}", e))?;
        Ok(())
    }

    /// Lee el log de auditoría.
    pub fn read_audit(&self) -> Result<Vec<String>, String> {
        let path = self.audit_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let txt =
            fs::read_to_string(&path).map_err(|e| format!("lectura de la auditoría: {}", e))?;
        Ok(txt
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.to_string())
            .collect())
    }

    /// Certificados caducados o a punto de caducar.
    pub fn expiry_warnings(
        &self,
        now_ts: i64,
        warn_days: i64,
    ) -> Result<Vec<(String, String)>, String> {
        let list = self.list()?;
        let mut out = Vec::new();
        for c in list {
            let st = c.status_at(now_ts, warn_days);
            if st != "valid" {
                out.push((c.id.clone(), st.to_string()));
            }
        }
        Ok(out)
    }
}

/// Resultado interno de importar una entrada del paquete de sincronización.
enum Outcome {
    Imported,
    Unchanged,
}

// ---------------------------------------------------------------------------
// Formatos: paquete de exportación y paquete de sincronización
// ---------------------------------------------------------------------------

/// Referencia a la clave dentro de un paquete de exportación.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ExportKeyRef {
    NonExportable {
        backend: String,
        handle: String,
    },
    Exportable {
        backend: String,
        /// SHA-256 del blob cifrado de la clave (identifica el material).
        sha256: String,
    },
}

/// Certificado descrito en la cabecera de un paquete de exportación.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExportCert {
    id: String,
    label: String,
    der_sha256: String,
    der_len: u64,
    key: ExportKeyRef,
    /// SHA-256 del blob cifrado de la clave (vacío si no hay clave).
    wrapped_sha256: String,
    /// Longitud del blob cifrado de la clave.
    wrapped_len: u64,
    /// ¿Viajan los bytes del material cifrado en este paquete?
    include_wrapped: bool,
}

/// Cabecera del paquete de exportación de un certificado.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExportHeader {
    version: u32,
    node: String,
    created_at: i64,
    cert: ExportCert,
}

/// Referencia a la clave dentro de un paquete de sincronización.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SyncKeyRef {
    NonExportable { backend: String, handle: String },
    Exportable { backend: String, sha256: String },
}

/// Entrada de la cabecera de un paquete de sincronización.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncHeaderEntry {
    id: String,
    label: String,
    der_sha256: String,
    der_len: u64,
    key: SyncKeyRef,
    /// SHA-256 del blob cifrado de la clave (vacío si no hay clave).
    wrapped_sha256: String,
    /// Longitud del blob cifrado de la clave.
    wrapped_len: u64,
    /// ¿Viajan los bytes del material cifrado en este paquete?
    include_wrapped: bool,
}

/// Cabecera del paquete de sincronización.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncHeader {
    version: u32,
    node: String,
    created_at: i64,
    include_exportable_keys: bool,
    entries: Vec<SyncHeaderEntry>,
}

fn export_key_ref(k: &PrivateKeyRef, wrapped_sha: &str) -> ExportKeyRef {
    match k {
        PrivateKeyRef::NonExportable { backend, handle } => ExportKeyRef::NonExportable {
            backend: backend.clone(),
            handle: handle.clone(),
        },
        PrivateKeyRef::Exportable { backend, .. } => ExportKeyRef::Exportable {
            backend: backend.clone(),
            sha256: wrapped_sha.to_string(),
        },
        PrivateKeyRef::Remote {
            backend, sha256, ..
        } => ExportKeyRef::Exportable {
            backend: backend.clone(),
            sha256: sha256.clone(),
        },
    }
}

fn sync_key_ref(k: &ManifestKey) -> SyncKeyRef {
    match k {
        ManifestKey::NonExportable { backend, handle } => SyncKeyRef::NonExportable {
            backend: backend.clone(),
            handle: handle.clone(),
        },
        ManifestKey::Exportable {
            backend, sha256, ..
        } => SyncKeyRef::Exportable {
            backend: backend.clone(),
            sha256: sha256.clone(),
        },
    }
}
/// Parsea un paquete de exportación: `(cabecera, DER, clave cifrada)`.
fn parse_export_bundle(bundle: &[u8]) -> Result<(ExportHeader, Vec<u8>, Vec<u8>), String> {
    if bundle.len() < 12 {
        return Err("paquete truncado".to_string());
    }
    if &bundle[0..4] != EXPORT_MAGIC {
        return Err("firma inválida: no es un paquete de exportación".to_string());
    }
    let version = u32::from_le_bytes(bundle[4..8].try_into().unwrap());
    if version != 1 {
        return Err(format!("versión de paquete no soportada: {}", version));
    }
    let header_len = u32::from_le_bytes(bundle[8..12].try_into().unwrap()) as usize;
    let start = 12;
    if bundle.len() < start + header_len {
        return Err("paquete truncado (cabecera)".to_string());
    }
    let header: ExportHeader = serde_json::from_slice(&bundle[start..start + header_len])
        .map_err(|e| format!("cabecera ilegible: {}", e))?;
    let der_start = start + header_len;
    let der_end = der_start + header.cert.der_len as usize;
    if bundle.len() < der_end {
        return Err("paquete truncado (DER)".to_string());
    }
    let der = bundle[der_start..der_end].to_vec();
    let wrapped_end = der_end + header.cert.wrapped_len as usize;
    if bundle.len() < wrapped_end {
        return Err("paquete truncado (clave)".to_string());
    }
    let wrapped = bundle[der_end..wrapped_end].to_vec();
    Ok((header, der, wrapped))
}

// ---------------------------------------------------------------------------
// Cifrado de la clave privada (AEGIS-256 + Argon2id)
// ---------------------------------------------------------------------------

/// Cifra los bytes de la clave privada con AEGIS-256.
///
/// La clave de cifrado sale de `crate::argon2::derive_key` (Argon2id canónico
/// del repo) con **sal aleatoria de 16 bytes por clave**; el nonce también es
/// aleatorio.  El AAD ata el ciphertext al `id` ⇒ el blob de una clave no se
/// puede pegar en el hueco de otra.
pub fn wrap_private_key(
    id: &str,
    key_bytes: &[u8],
    passphrase: &str,
    kdf: &KdfParams,
) -> Result<Vec<u8>, String> {
    let salt = random_bytes(SALT_LEN);
    let nonce = random_bytes(NONCE_LEN);
    let dk = crate::argon2::derive_key(
        passphrase,
        &salt,
        kdf.iterations,
        kdf.memory_kib,
        kdf.parallelism,
        kdf.key_len,
    )?;
    let ad = key_aad(id);
    let (ct, tag) = crate::aegis::encrypt(&dk, &nonce, key_bytes, &ad, TAG_LEN)?;

    let mut out: Vec<u8> = Vec::with_capacity(96 + ct.len());
    out.extend_from_slice(KEY_MAGIC);
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&kdf.iterations.to_le_bytes());
    out.extend_from_slice(&kdf.memory_kib.to_le_bytes());
    out.extend_from_slice(&kdf.parallelism.to_le_bytes());
    out.extend_from_slice(&(kdf.key_len as u32).to_le_bytes());
    out.extend_from_slice(&(salt.len() as u32).to_le_bytes());
    out.extend_from_slice(&(nonce.len() as u32).to_le_bytes());
    out.extend_from_slice(&(tag.len() as u32).to_le_bytes());
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&tag);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Descifra los bytes de la clave privada (operación inversa de
/// [`wrap_private_key`]).
pub fn unwrap_private_key(id: &str, blob: &[u8], passphrase: &str) -> Result<Vec<u8>, String> {
    if blob.len() < 96 || &blob[0..4] != KEY_MAGIC {
        return Err("el blob de clave no tiene el formato esperado".to_string());
    }
    let rd = |off: usize| -> u32 { u32::from_le_bytes(blob[off..off + 4].try_into().unwrap()) };
    let iterations = rd(8);
    let memory_kib = rd(12);
    let parallelism = rd(16);
    let key_len = rd(20) as usize;
    let salt_len = rd(24) as usize;
    let nonce_len = rd(28) as usize;
    let tag_len = rd(32) as usize;
    let mut pos = 36usize;
    if blob.len() < pos + salt_len + nonce_len + tag_len {
        return Err("el blob de clave está truncado".to_string());
    }
    let salt = &blob[pos..pos + salt_len];
    pos += salt_len;
    let nonce = &blob[pos..pos + nonce_len];
    pos += nonce_len;
    let tag = &blob[pos..pos + tag_len];
    pos += tag_len;
    let ct = &blob[pos..];

    let dk = crate::argon2::derive_key(
        passphrase,
        salt,
        iterations,
        memory_kib,
        parallelism,
        key_len,
    )?;
    let ad = key_aad(id);
    crate::aegis::decrypt(&dk, nonce, ct, &ad, tag)
}

/// AAD del blob de clave: prefijo + `id` del certificado.
fn key_aad(id: &str) -> Vec<u8> {
    let mut ad = KEY_AAD_PREFIX.to_vec();
    ad.extend_from_slice(id.as_bytes());
    ad
}

// ---------------------------------------------------------------------------
// Parseo DER mínimo (X.509)
// ---------------------------------------------------------------------------

/// Lo que se extrae de un certificado X.509.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCertificate {
    /// Número de serie en hexadecimal.
    pub serial_number: String,
    /// Emisor (DN del `issuer`).
    pub issuer: String,
    /// Sujeto (DN del `subject`).
    pub subject: String,
    /// `notBefore` en epoch (UTC).
    pub not_before: i64,
    /// `notAfter` en epoch (UTC).
    pub not_after: i64,
    /// Algoritmo de firma (texto legible).
    pub signature_algorithm: String,
    /// Algoritmo de la clave pública (texto legible).
    pub public_key_algorithm: String,
}

/// Lee un TLV DER: devuelve `(tag, contenido, posición siguiente)`.
fn der_tlv(buf: &[u8], pos: usize) -> Result<(u8, &[u8], usize), String> {
    if pos + 2 > buf.len() {
        return Err("DER truncado".to_string());
    }
    let tag = buf[pos];
    let first = buf[pos + 1];
    let (len, header) = if first & 0x80 == 0 {
        (first as usize, 2usize)
    } else {
        let n = (first & 0x7f) as usize;
        if n == 0 || n > 4 || pos + 2 + n > buf.len() {
            return Err("longitud DER no soportada".to_string());
        }
        let mut l = 0usize;
        for i in 0..n {
            l = (l << 8) | buf[pos + 2 + i] as usize;
        }
        (l, 2 + n)
    };
    let start = pos + header;
    let end = start
        .checked_add(len)
        .ok_or_else(|| "longitud DER desbordada".to_string())?;
    if end > buf.len() {
        return Err("DER truncado".to_string());
    }
    Ok((tag, &buf[start..end], end))
}

/// Parsea un certificado X.509 en DER.
///
/// **A mano** (no hay `x509-parser` en `Cargo.toml`): se recorre
/// `Certificate → TBSCertificate → {version, serial, signature, issuer,
/// validity, subject, subjectPublicKeyInfo}` y se leen `notBefore`/`notAfter`
/// como `UTCTime`/`GeneralizedTime`.
pub fn parse_certificate(der: &[u8]) -> Result<ParsedCertificate, String> {
    let (tag, cert, _) = der_tlv(der, 0)?;
    if tag != 0x30 {
        return Err("el certificado no empieza por una SEQUENCE".to_string());
    }
    let (tag, tbs, after_tbs) = der_tlv(cert, 0)?;
    if tag != 0x30 {
        return Err("TBSCertificate no es una SEQUENCE".to_string());
    }
    let (tag, sig_alg, _) = der_tlv(cert, after_tbs)?;
    if tag != 0x30 {
        return Err("firma ausente en el certificado".to_string());
    }
    let signature_algorithm = algorithm_name(sig_alg);

    let mut pos = 0usize;
    let (tag, _version, next) = der_tlv(tbs, pos)?;
    pos = if tag == 0xA0 { next } else { pos }; // `version` es opcional ([0])
    let (tag, serial, pos2) = der_tlv(tbs, pos)?;
    if tag != 0x02 {
        return Err("número de serie ausente".to_string());
    }
    pos = pos2;
    let (_tag, _sigalg, pos3) = der_tlv(tbs, pos)?;
    pos = pos3;
    let (_tag, issuer, pos4) = der_tlv(tbs, pos)?;
    pos = pos4;
    let (tag, validity, pos5) = der_tlv(tbs, pos)?;
    if tag != 0x30 {
        return Err("validez ausente".to_string());
    }
    pos = pos5;
    let (_tag, subject, pos6) = der_tlv(tbs, pos)?;
    pos = pos6;
    let (_tag, spki, _pos7) = der_tlv(tbs, pos)?;

    let (tag, nb, next) = der_tlv(validity, 0)?;
    let not_before = parse_time(tag, nb)?;
    let (tag, na, _next) = der_tlv(validity, next)?;
    let not_after = parse_time(tag, na)?;

    Ok(ParsedCertificate {
        serial_number: hex(serial),
        issuer: render_name(issuer),
        subject: render_name(subject),
        not_before,
        not_after,
        signature_algorithm,
        public_key_algorithm: spki_algorithm(spki),
    })
}

/// Convierte un `UTCTime` / `GeneralizedTime` DER a epoch (UTC).
fn parse_time(tag: u8, content: &[u8]) -> Result<i64, String> {
    let s = std::str::from_utf8(content).map_err(|_| "fecha no UTF-8".to_string())?;
    let (year, rest) = match tag {
        // UTCTime: YYMMDDHHMMSSZ (dos dígitos de año)
        0x17 => {
            if s.len() < 11 {
                return Err("UTCTime demasiado corto".to_string());
            }
            let yy: i64 = s[0..2].parse().map_err(|_| "año inválido".to_string())?;
            let year = if yy >= 50 { 1900 + yy } else { 2000 + yy };
            (year, &s[2..])
        }
        // GeneralizedTime: YYYYMMDDHHMMSSZ
        0x18 => {
            if s.len() < 13 {
                return Err("GeneralizedTime demasiado corto".to_string());
            }
            let year: i64 = s[0..4].parse().map_err(|_| "año inválido".to_string())?;
            (year, &s[4..])
        }
        _ => return Err(format!("tag de fecha no soportado: 0x{:02x}", tag)),
    };
    let num =
        |a: &str| -> Result<i64, String> { a.parse().map_err(|_| "fecha inválida".to_string()) };
    let month = num(&rest[0..2])?;
    let day = num(&rest[2..4])?;
    let hour = num(&rest[4..6])?;
    let minute = num(&rest[6..8])?;
    let second = if rest.len() >= 10 && rest.as_bytes()[8].is_ascii_digit() {
        num(&rest[8..10])?
    } else {
        0
    };
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err("fecha fuera de rango".to_string());
    }
    // Desplazamiento horario: 'Z' (UTC) o ±hhmm.
    let tail = &rest[10.min(rest.len())..];
    let offset_secs: i64 = if tail.starts_with('Z') || tail.is_empty() {
        0
    } else if tail.starts_with('+') || tail.starts_with('-') {
        if tail.len() < 5 {
            return Err("desplazamiento horario inválido".to_string());
        }
        let oh: i64 = tail[1..3]
            .parse()
            .map_err(|_| "offset inválido".to_string())?;
        let om: i64 = tail[3..5]
            .parse()
            .map_err(|_| "offset inválido".to_string())?;
        let sign = if tail.starts_with('-') { -1 } else { 1 };
        sign * (oh * 3600 + om * 60)
    } else {
        return Err("formato de fecha no soportado".to_string());
    };
    let days = days_from_civil(year, month as u32, day as u32);
    Ok(days * 86_400 + hour * 3600 + minute * 60 + second - offset_secs)
}

/// Días desde 1970-01-01 para una fecha civil (algoritmo de Hinnant).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = ((m as i64) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + (d as i64) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Nombre legible del algoritmo de una `AlgorithmIdentifier`.
fn algorithm_name(alg: &[u8]) -> String {
    let Ok((tag, oid_raw, _)) = der_tlv(alg, 0) else {
        return "desconocido".to_string();
    };
    if tag != 0x06 {
        return "desconocido".to_string();
    }
    let oid = decode_oid(oid_raw);
    let nombre: &str = match oid.as_str() {
        "1.2.840.113549.1.1.4" => "md5WithRSAEncryption",
        "1.2.840.113549.1.1.5" => "sha1WithRSAEncryption",
        "1.2.840.113549.1.1.10" => "RSASSA-PSS",
        "1.2.840.113549.1.1.11" => "sha256WithRSAEncryption",
        "1.2.840.113549.1.1.12" => "sha384WithRSAEncryption",
        "1.2.840.113549.1.1.13" => "sha512WithRSAEncryption",
        "1.2.840.113549.1.1.14" => "sha224WithRSAEncryption",
        "1.2.840.10040.4.3" => "dsaWithSHA1",
        "1.2.840.10045.4.1" => "ecdsa-with-SHA1",
        "1.2.840.10045.4.3.2" => "ecdsa-with-SHA256",
        "1.2.840.10045.4.3.3" => "ecdsa-with-SHA384",
        "1.2.840.10045.4.3.4" => "ecdsa-with-SHA512",
        "1.3.101.112" => "Ed25519",
        "1.3.101.113" => "Ed448",
        _ => return oid,
    };
    nombre.to_string()
}

/// Algoritmo de la clave pública a partir del `SubjectPublicKeyInfo`.
fn spki_algorithm(spki: &[u8]) -> String {
    let Ok((tag, alg, _)) = der_tlv(spki, 0) else {
        return "desconocido".to_string();
    };
    if tag != 0x30 {
        return "desconocido".to_string();
    }
    let Ok((otag, oid_raw, _)) = der_tlv(alg, 0) else {
        return "desconocido".to_string();
    };
    if otag != 0x06 {
        return "desconocido".to_string();
    }
    match decode_oid(oid_raw).as_str() {
        "1.2.840.113549.1.1.1" => "rsaEncryption".to_string(),
        "1.2.840.10045.2.1" => "id-ecPublicKey".to_string(),
        "1.3.101.112" => "Ed25519".to_string(),
        "1.3.101.113" => "Ed448".to_string(),
        other => other.to_string(),
    }
}

/// Decodifica un OID DER a forma "1.2.840.113549".
fn decode_oid(raw: &[u8]) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let mut parts: Vec<String> = Vec::new();
    let first = raw[0] as u64;
    if first < 80 {
        parts.push((first / 40).to_string());
        parts.push((first % 40).to_string());
    } else {
        parts.push("2".to_string());
        parts.push((first - 80).to_string());
    }
    let mut acc: u64 = 0;
    for (i, b) in raw.iter().enumerate().skip(1) {
        acc = (acc << 7) | (b & 0x7f) as u64;
        if b & 0x80 == 0 {
            parts.push(acc.to_string());
            acc = 0;
        } else if i > 9 {
            break;
        }
    }
    parts.join(".")
}

/// Nombre corto de un OID de atributo (`CN`, `O`, `C`, ...).
fn oid_short_name(raw: &[u8]) -> String {
    let oid = decode_oid(raw);
    match oid.as_str() {
        "2.5.4.3" => "CN".to_string(),
        "2.5.4.4" => "SN".to_string(),
        "2.5.4.5" => "serialNumber".to_string(),
        "2.5.4.6" => "C".to_string(),
        "2.5.4.7" => "L".to_string(),
        "2.5.4.8" => "ST".to_string(),
        "2.5.4.9" => "STREET".to_string(),
        "2.5.4.10" => "O".to_string(),
        "2.5.4.11" => "OU".to_string(),
        "2.5.4.42" => "GN".to_string(),
        "0.9.2342.19200300.100.1.1" => "UID".to_string(),
        "0.9.2342.19200300.100.1.25" => "DC".to_string(),
        "1.2.840.113549.1.9.1" => "emailAddress".to_string(),
        _ => oid,
    }
}

/// Decodifica un valor de directorio a texto.
fn decode_string(tag: u8, raw: &[u8]) -> String {
    match tag {
        0x0C | 0x13 | 0x16 | 0x1A | 0x14 => String::from_utf8_lossy(raw).to_string(),
        0x1E => {
            // BMPString: UTF-16BE
            let mut s = String::new();
            let mut i = 0;
            while i + 1 < raw.len() {
                let u = u16::from_be_bytes([raw[i], raw[i + 1]]);
                if let Some(c) = char::from_u32(u as u32) {
                    s.push(c);
                }
                i += 2;
            }
            s
        }
        _ => hex(raw),
    }
}

/// Renderiza un `Name` (RDNSequence) como texto `CN=..., O=...`.
fn render_name(name: &[u8]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut pos = 0usize;
    while pos < name.len() {
        let Ok((_tag, rdn, next)) = der_tlv(name, pos) else {
            break;
        };
        pos = next;
        let mut rpos = 0usize;
        while rpos < rdn.len() {
            let Ok((_t, atv, rnext)) = der_tlv(rdn, rpos) else {
                break;
            };
            rpos = rnext;
            let Ok((otag, oid_raw, after_oid)) = der_tlv(atv, 0) else {
                continue;
            };
            if otag != 0x06 {
                continue;
            }
            let Ok((vtag, value, _)) = der_tlv(atv, after_oid) else {
                continue;
            };
            parts.push(format!(
                "{}={}",
                oid_short_name(oid_raw),
                decode_string(vtag, value)
            ));
        }
    }
    parts.join(", ")
}

// ---------------------------------------------------------------------------
// Utilidades
// ---------------------------------------------------------------------------

/// Bytes a hexadecimal en minúsculas.
pub fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// SHA-256 en hexadecimal.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex(&h.finalize())
}

/// Epoch actual (segundos).
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Identificador del nodo (host + usuario): sólo informativo.
fn node_id() -> String {
    let host = std::fs::read_to_string("/proc/sys/kernel/hostname")
        .unwrap_or_default()
        .trim()
        .to_string();
    if host.is_empty() {
        "desconocido".to_string()
    } else {
        host
    }
}

/// Bytes aleatorios criptográficos (`/dev/urandom`).
fn random_bytes(n: usize) -> Vec<u8> {
    use std::io::Read as _;
    if let Ok(mut f) = fs::File::open("/dev/urandom") {
        let mut buf = vec![0u8; n];
        if f.read_exact(&mut buf).is_ok() {
            return buf;
        }
    }
    // Respaldo NO criptográfico (sólo para que no se caiga el proceso).
    let mut seed = Vec::new();
    seed.extend_from_slice(&now().to_le_bytes());
    seed.extend_from_slice(&std::process::id().to_le_bytes());
    let mut out = Vec::with_capacity(n);
    let mut counter: u64 = 0;
    while out.len() < n {
        let mut h = Sha256::new();
        h.update(&seed);
        h.update(&counter.to_le_bytes());
        out.extend_from_slice(&h.finalize());
        counter += 1;
    }
    out.truncate(n);
    out
}

/// Valida un `id` de certificado (sin rutas ni separadores).
pub fn sanitize_id(id: &str) -> Result<String, String> {
    let n = id.trim();
    if n.is_empty() {
        return Err("id vacio".to_string());
    }
    if n.len() > MAX_ID_LEN {
        return Err(format!("id demasiado largo ({} > {})", n.len(), MAX_ID_LEN));
    }
    if !n
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err(format!("id invalido: {}", n));
    }
    if n.starts_with('.') || n.contains("..") {
        return Err(format!("id invalido: {}", n));
    }
    Ok(n.to_string())
}

/// Valida una etiqueta (texto libre, sin saltos de línea).
pub fn sanitize_label(label: &str) -> Result<String, String> {
    let n = label.trim().replace(['\n', '\r'], " ");
    if n.len() > MAX_LABEL_LEN {
        return Err(format!(
            "etiqueta demasiado larga ({} > {})",
            n.len(),
            MAX_LABEL_LEN
        ));
    }
    Ok(n)
}

/// Escribe un fichero de forma atómica con permisos `mode`.
fn write_atomic(path: &Path, data: &[u8], mode: u32) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {}", parent.display(), e))?;
    }
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or("bin")
    ));
    {
        let mut f =
            fs::File::create(&tmp).map_err(|e| format!("creación de {}: {}", tmp.display(), e))?;
        // Los permisos se fijan **antes** de escribir: el fichero temporal
        // no existe ni un instante con el modo por defecto del umask.
        set_mode(&tmp, mode);
        f.write_all(data)
            .map_err(|e| format!("escritura de {}: {}", tmp.display(), e))?;
        f.sync_all()
            .map_err(|e| format!("fsync de {}: {}", tmp.display(), e))?;
    }
    fs::rename(&tmp, path).map_err(|e| format!("rename {}: {}", tmp.display(), e))?;
    set_mode(path, mode);
    Ok(())
}

/// Escribe un blob en el flujo de salida (streaming, por trozos).
fn stream_out<W: Write>(w: &mut W, data: &[u8]) -> Result<(), String> {
    let mut off = 0usize;
    while off < data.len() {
        let end = std::cmp::min(off + CHUNK, data.len());
        w.write_all(&data[off..end])
            .map_err(|e| format!("escritura del paquete: {}", e))?;
        off = end;
    }
    Ok(())
}

/// Lee exactamente `len` bytes del flujo (streaming, por trozos).
fn read_exact_streaming<R: Read>(r: &mut R, len: u64) -> Result<Vec<u8>, String> {
    let mut out: Vec<u8> = Vec::with_capacity(std::cmp::min(len as usize, 1024 * 1024));
    let mut left = len;
    let mut buf = vec![0u8; CHUNK];
    while left > 0 {
        let want = std::cmp::min(left as usize, CHUNK);
        r.read_exact(&mut buf[..want])
            .map_err(|e| format!("lectura del paquete: {}", e))?;
        out.extend_from_slice(&buf[..want]);
        left -= want as u64;
    }
    Ok(out)
}

/// Fija los permisos de un fichero o directorio (no-op fuera de Unix).
#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = fs::metadata(path) {
        let mut perm = meta.permissions();
        perm.set_mode(mode);
        let _ = fs::set_permissions(path, perm);
    }
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) {}
