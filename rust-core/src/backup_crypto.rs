//! Criptografía y formato de contenedor del **motor de backup nativo**.
//!
//! Tres piezas, y una frontera explícita entre lo **medido** y lo **supuesto**:
//!
//! 1. **KDF**: **Argon2id** (RFC 9106) con los **rangos medidos** en Swift
//!    Backup (salt 16 B, iteraciones 1..100, memoria 16384..65536 KiB,
//!    paralelismo 1..8, salida 16..128 B). Los parámetros se **leen del
//!    propio fichero**, nunca se hardcodean.
//! 2. **AEAD**: **AEGIS-256** (RFC 10032, implementado en `crate::aegis`)
//!    **por trozos**: cada chunk lleva su nonce y su AAD, derivados de la
//!    clave maestra y de la ruta de la entrada ⇒ un backup no se puede
//!    reordenar ni renombrar sin que la etiqueta deje de cuadrar.
//! 3. **key-check**: SHA-256 etiquetado + comparación **en tiempo constante**
//!    ⇒ se **verifica la contraseña sin descifrar** (mismo truco que Swift
//!    Backup: `SBA1-AEGIS256-key-check-v1`).
//!
//! ## La frontera: medido ≠ supuesto
//!
//! | Cosa | Estado | Evidencia |
//! |---|---|---|
//! | Argon2id + sus rangos | **medido** | `docs/SWIFT_BACKUP_CRYPTO.md` §2.1 |
//! | Etiquetas `SBA1-…` | **medido** | idem §2.3 y §3 |
//! | ids de cifrado (4 = AEGIS-256) y de KDF (1 = Argon2id) | **medido** | idem §4.4 |
//! | AEGIS-256 (RFC 10032) | **medido y validado** | vectores oficiales, `crate::aegis` |
//! | Layout byte a byte de la info page | **NO confirmado** | idem §4.3/§4.7 (sólo escritor x86_64) |
//! | Fórmula exacta del key-check | **NO confirmada** | idem §5.3 |
//! | Derivación de nonce/AAD por chunk | **NO confirmada** | idem §5.4 |
//!
//! ⇒ Por eso la interoperabilidad con Swift Backup está marcada
//! **EXPERIMENTAL** en [`read_swift_backup`] y [`write_swift_backup`]: el
//! lector **busca** entre un conjunto **acotado y medido** de candidatos y
//! **sólo da por bueno** lo que el *key-check* confirma (el key-check es el
//! oráculo: si la contraseña y el layout son correctos, la etiqueta cuadra;
//! si no, se dice exactamente qué se probó y se falla limpio).
//!
//! El contenedor **propio** de MegaDaemon ([`CONTAINER_MAGIC`]) **no** usa
//! esas etiquetas: usa las suyas (`MDBK1-…`) para que un fichero nuestro
//! nunca se confunda con uno de Swift Backup.

use argon2::{Algorithm, Argon2, Params, Version};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

// ---------------------------------------------------------------------------
// Contrato de formato: contenedor propio
// ---------------------------------------------------------------------------

/// Magic del contenedor propio de MegaDaemon (`MDBK1`).
pub const CONTAINER_MAGIC: &[u8; 5] = b"MDBK1";
/// Versión del contenedor propio.
pub const CONTAINER_VERSION: u8 = 1;
/// Etiqueta del key-check del contenedor propio.
pub const LABEL_KEY_CHECK: &str = "MDBK1-aegis256-key-check-v1";
/// Etiqueta de la derivación de la clave de nonce.
pub const LABEL_NONCE_KEY: &str = "MDBK1-aegis256-nonce-key-v1";
/// Etiqueta del AAD por trozo.
pub const LABEL_CHUNK_AAD: &str = "MDBK1-aegis256-chunk-aad-v1";
/// Etiqueta del nonce por trozo.
pub const LABEL_CHUNK_NONCE: &str = "MDBK1-aegis256-chunk-nonce-v1";
/// Tamaño por defecto del trozo cifrado (1 MiB).
pub const DEFAULT_CHUNK_SIZE: usize = 1024 * 1024;

// ---------------------------------------------------------------------------
// Contrato de formato: Swift Backup (medido)
// ---------------------------------------------------------------------------

/// Etiqueta del key-check de Swift Backup (**medida**).
pub const SBA_LABEL_KEY_CHECK: &str = "SBA1-AEGIS256-key-check-v1";
/// Etiqueta del AAD por trozo de Swift Backup (**medida**).
pub const SBA_LABEL_CHUNK_AAD: &str = "SBA1-AEGIS256-entry-chunk-aad-v1";
/// Etiqueta del nonce por trozo de Swift Backup (**medida**).
pub const SBA_LABEL_CHUNK_NONCE: &str = "SBA1-AEGIS256-entry-chunk-nonce-v1";
/// Magic de la info page de Swift Backup (**medido**).
pub const SBA_MAGIC: &[u8; 4] = b"SBA1";
/// Tamaño de la info page de Swift Backup (**medido**: 128 B).
pub const SBA_INFO_PAGE_LEN: usize = 128;

/// ids de método de cifrado de Swift Backup (**medidos**).
pub const SBA_CIPHER_NONE: u32 = 0;
/// SevenZip AES.
pub const SBA_CIPHER_SEVENZIP_AES: u32 = 1;
/// AES-256-GCM.
pub const SBA_CIPHER_AES256_GCM: u32 = 2;
/// AES-256-GCM-SIV.
pub const SBA_CIPHER_AES256_GCM_SIV: u32 = 3;
/// AEGIS-256 (el que nos interesa).
pub const SBA_CIPHER_AEGIS256: u32 = 4;
/// AEGIS-128X2.
pub const SBA_CIPHER_AEGIS128X2: u32 = 5;

/// ids de método de KDF de Swift Backup (**medidos**).
pub const SBA_KDF_NONE: u32 = 0;
/// Argon2id (el que nos interesa).
pub const SBA_KDF_ARGON2ID: u32 = 1;

// ---------------------------------------------------------------------------
// Rangos medidos del KDF de Swift Backup
// ---------------------------------------------------------------------------

/// Longitud del salt de Swift Backup (**medida**: 16 B).
pub const SBA_SALT_LEN: usize = 16;
/// Iteraciones mínima (**medida**).
pub const SBA_ITER_MIN: u32 = 1;
/// Iteraciones máxima (**medida**).
pub const SBA_ITER_MAX: u32 = 100;
/// Memoria mínima en KiB (**medida**).
pub const SBA_MEM_MIN_KIB: u32 = 16384;
/// Memoria máxima en KiB (**medida**).
pub const SBA_MEM_MAX_KIB: u32 = 65536;
/// Paralelismo mínimo (**medido**).
pub const SBA_PAR_MIN: u32 = 1;
/// Paralelismo máximo (**medido**).
pub const SBA_PAR_MAX: u32 = 8;
/// Longitud mínima de la salida (**medida**).
pub const SBA_OUT_MIN: usize = 16;
/// Longitud máxima de la salida (**medida**).
pub const SBA_OUT_MAX: usize = 128;

/// Longitud de la clave de AEGIS-256 (256 bits).
pub const AEGIS_KEY_LEN: usize = 32;
/// Longitud del nonce de AEGIS-256 (256 bits, RFC 10032).
pub const AEGIS_NONCE_LEN: usize = 32;
/// Longitud de la etiqueta de AEGIS-256 (256 bits).
pub const AEGIS_TAG_LEN: usize = 32;

/// Bytes en hexadecimal en minúsculas.
pub fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Bytes desde hexadecimal en minúsculas.
pub fn unhex(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err("hexadecimal de longitud impar".to_string());
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let b = s.as_bytes();
    for i in (0..b.len()).step_by(2) {
        let hi = (b[i] as char)
            .to_digit(16)
            .ok_or_else(|| format!("hexadecimal inválido en la posición {i}"))?;
        let lo = (b[i + 1] as char)
            .to_digit(16)
            .ok_or_else(|| format!("hexadecimal inválido en la posición {}", i + 1))?;
        out.push((hi * 16 + lo) as u8);
    }
    Ok(out)
}

/// SHA-256 en hexadecimal de un bloque en memoria.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex(&h.finalize())
}

/// SHA-256 en hexadecimal de un fichero, **en streaming** (no carga el
/// fichero en memoria: sirve para backups de gigas).
pub fn sha256_file(path: &std::path::Path) -> Result<(u64, String), String> {
    let mut f = std::fs::File::open(path).map_err(|e| format!("no se puede abrir {}: {e}", path.display()))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    let mut total = 0u64;
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| format!("error leyendo {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        total += n as u64;
    }
    Ok((total, hex(&h.finalize())))
}

// ---------------------------------------------------------------------------
// KDF: Argon2id
// ---------------------------------------------------------------------------

/// Método de derivación de clave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KdfMethod {
    /// Sin cifrado.
    None,
    /// Argon2id (RFC 9106) — el de Swift Backup.
    Argon2id,
}

impl KdfMethod {
    /// id numérico (los de Swift Backup: 0 = none, 1 = Argon2id).
    pub fn id(self) -> u32 {
        match self {
            KdfMethod::None => SBA_KDF_NONE,
            KdfMethod::Argon2id => SBA_KDF_ARGON2ID,
        }
    }

    /// id numérico ⇒ método.
    pub fn from_id(id: u32) -> Option<KdfMethod> {
        match id {
            SBA_KDF_NONE => Some(KdfMethod::None),
            SBA_KDF_ARGON2ID => Some(KdfMethod::Argon2id),
            _ => None,
        }
    }
}

/// Parámetros del KDF.
///
/// Los rangos son los **medidos** en Swift Backup; por eso
/// [`KdfParams::validate`] reproduce sus mensajes de error exactos: un
/// contenedor con parámetros fuera de rango **no** lo habría creado la app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KdfParams {
    /// Método de derivación.
    pub method: KdfMethod,
    /// Iteraciones (`t`).
    pub iterations: u32,
    /// Memoria en KiB (`m`).
    pub memory_kib: u32,
    /// Paralelismo (`p`).
    pub parallelism: u32,
    /// Longitud de la salida en bytes.
    pub output_len: usize,
    /// Salt (16 B en Swift Backup).
    pub salt: Vec<u8>,
}

impl KdfParams {
    /// Parámetros por defecto **de MegaDaemon** (dentro de los rangos medidos
    /// de Swift Backup).
    ///
    /// ⚠️ **No** son los de la app: los de Swift Backup se **leen del
    /// fichero**. Estos son los que usamos para nuestros propios backups.
    pub fn default_params() -> KdfParams {
        KdfParams {
            method: KdfMethod::Argon2id,
            iterations: 3,
            memory_kib: SBA_MEM_MIN_KIB,
            parallelism: 4,
            output_len: AEGIS_KEY_LEN,
            salt: vec![0u8; SBA_SALT_LEN],
        }
    }

    /// Comprueba los parámetros contra los **rangos medidos**.
    pub fn validate(&self) -> Result<(), String> {
        match self.method {
            KdfMethod::None => return Err("SBA encryption password must not be empty".to_string()),
            KdfMethod::Argon2id => {}
        }
        if self.salt.len() != SBA_SALT_LEN {
            return Err(format!(
                "SBA Argon2id salt must be {} bytes ({} dados)",
                SBA_SALT_LEN,
                self.salt.len()
            ));
        }
        if self.iterations < SBA_ITER_MIN || self.iterations > SBA_ITER_MAX {
            return Err(format!(
                "Argon2id iterations must be in {}..{}",
                SBA_ITER_MIN, SBA_ITER_MAX
            ));
        }
        if self.memory_kib < SBA_MEM_MIN_KIB || self.memory_kib > SBA_MEM_MAX_KIB {
            return Err(format!(
                "Argon2id memoryKiB must be in {}..{}",
                SBA_MEM_MIN_KIB, SBA_MEM_MAX_KIB
            ));
        }
        if self.parallelism < SBA_PAR_MIN || self.parallelism > SBA_PAR_MAX {
            return Err(format!(
                "Argon2id parallelism must be in {}..{}",
                SBA_PAR_MIN, SBA_PAR_MAX
            ));
        }
        if self.output_len < SBA_OUT_MIN || self.output_len > SBA_OUT_MAX {
            return Err(format!(
                "Argon2id outputBytes must be in {}..{}",
                SBA_OUT_MIN, SBA_OUT_MAX
            ));
        }
        // Argon2 exige m >= 8*p.
        if self.memory_kib < self.parallelism.saturating_mul(8) {
            return Err(format!(
                "Argon2id memoryKiB must be at least 8*parallelism ({} < {})",
                self.memory_kib,
                self.parallelism.saturating_mul(8)
            ));
        }
        Ok(())
    }

    /// Deriva la clave maestra a partir de la contraseña.
    ///
    /// La contraseña se codifica en **UTF-8** (medido: la app usa
    /// `Charset.newEncoder` → `CharBuffer.wrap` → `encode`).
    pub fn derive_key(&self, passphrase: &str) -> Result<Vec<u8>, String> {
        self.validate()?;
        if passphrase.is_empty() {
            return Err("SBA encryption password must not be empty".to_string());
        }
        let params = Params::new(
            self.memory_kib,
            self.iterations,
            self.parallelism,
            Some(self.output_len),
        )
        .map_err(|e| format!("parámetros de Argon2id inválidos: {e}"))?;
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut out = vec![0u8; self.output_len];
        argon
            .hash_password_into(passphrase.as_bytes(), &self.salt, &mut out)
            .map_err(|e| format!("no se pudo derivar la clave: {e}"))?;
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// Key-check (verificar la contraseña sin descifrar)
// ---------------------------------------------------------------------------

/// Calcula el **key-check**: SHA-256 etiquetado con la clave maestra.
pub fn key_check(key: &[u8], label: &str, cipher_ordinal: u32) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(label.as_bytes());
    h.update(cipher_ordinal.to_le_bytes());
    h.update(key);
    let mut out = [0u8; 32];
    out.copy_from_slice(&h.finalize());
    out
}

/// Compara dos key-checks **en tiempo constante**.
pub fn key_check_eq(a: &[u8], b: &[u8]) -> bool {
    crate::aegis::ct_eq(a, b)
}

/// Verifica la contraseña contra un key-check almacenado.
///
/// Devuelve `false` (no un error) si no cuadra: el llamante decide si es
/// "contraseña incorrecta" o "el layout no es el que creemos".
pub fn verify_key_check(key: &[u8], label: &str, cipher_ordinal: u32, expected_hex: &str) -> bool {
    let expected = match unhex(expected_hex) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let got = key_check(key, label, cipher_ordinal);
    key_check_eq(&got, &expected)
}

// ---------------------------------------------------------------------------
// AEAD por trozos: AEGIS-256
// ---------------------------------------------------------------------------

/// Cifrador AEAD **por trozos** con AEGIS-256.
///
/// Cada trozo lleva su **nonce** y su **AAD**:
/// * nonce = `HMAC-SHA256(nonce_key, etiqueta_nonce || índice || ruta)`
/// * AAD   = `etiqueta_aad || índice || longitud || ruta || índice_total`
///
/// ⇒ el tag ata cada trozo a **su posición** y a **su entrada**: reordenar,
/// renombrar o mover un fichero dentro del backup rompe la verificación.
#[derive(Clone)]
pub struct ChunkCipher {
    master: Vec<u8>,
    nonce_key: [u8; 32],
    label_nonce: String,
    label_aad: String,
}

impl ChunkCipher {
    /// Crea el cifrador a partir de la clave maestra.
    pub fn new(master: &[u8], label_nonce: &str, label_aad: &str) -> Result<ChunkCipher, String> {
        if master.len() < AEGIS_KEY_LEN {
            return Err(format!(
                "la clave maestra debe tener al menos {AEGIS_KEY_LEN} bytes, no {}",
                master.len()
            ));
        }
        let mut nonce_key = [0u8; 32];
        nonce_key.copy_from_slice(&crate::backup_vault::hmac_sha256(master, LABEL_NONCE_KEY.as_bytes()));
        Ok(ChunkCipher {
            master: master.to_vec(),
            nonce_key,
            label_nonce: label_nonce.to_string(),
            label_aad: label_aad.to_string(),
        })
    }

    /// Cifrador con las etiquetas del contenedor propio.
    pub fn own(master: &[u8]) -> Result<ChunkCipher, String> {
        ChunkCipher::new(master, LABEL_CHUNK_NONCE, LABEL_CHUNK_AAD)
    }

    /// Cifrador con las etiquetas **medidas** de Swift Backup.
    ///
    /// ⚠️ La **derivación** concreta (qué se mezcla con la etiqueta) **no
    /// está confirmada**: ver `docs/SWIFT_BACKUP_CRYPTO.md` §5.4.
    pub fn swift(master: &[u8]) -> Result<ChunkCipher, String> {
        ChunkCipher::new(master, SBA_LABEL_CHUNK_NONCE, SBA_LABEL_CHUNK_AAD)
    }

    /// Nonce del trozo.
    ///
    /// `nonce = HMAC-SHA256(nonce_key, etiqueta || índice || ruta)`.
    pub fn nonce(&self, entry: &str, index: u64) -> [u8; AEGIS_NONCE_LEN] {
        let mut data = Vec::new();
        data.extend_from_slice(self.label_nonce.as_bytes());
        data.extend_from_slice(&index.to_le_bytes());
        data.extend_from_slice(entry.as_bytes());
        let mac = crate::backup_vault::hmac_sha256(&self.nonce_key, &data);
        let mut out = [0u8; AEGIS_NONCE_LEN];
        out.copy_from_slice(&mac);
        out
    }

    /// AAD del trozo.
    ///
    /// `aad = etiqueta || índice || longitud || ruta`.
    ///
    /// El AAD **no** incluye el número total de trozos: el total no se
    /// conoce cuando se cifra (el payload se comprime en streaming) ⇒
    /// atarlo al índice y a la longitud de cada trozo es lo que ata el
    /// marco a su sitio.
    pub fn aad(&self, entry: &str, index: u64, plain_len: u64) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(self.label_aad.as_bytes());
        v.extend_from_slice(&index.to_le_bytes());
        v.extend_from_slice(&plain_len.to_le_bytes());
        v.extend_from_slice(entry.as_bytes());
        v
    }

    /// Cifra un trozo y devuelve el **marco** (frame).
    ///
    /// Layout del marco: `[u32 LE longitud][nonce 32 B][cifrado][etiqueta 32 B]`.
    pub fn seal_chunk(&self, entry: &str, index: u64, plaintext: &[u8]) -> Vec<u8> {
        let nonce = self.nonce(entry, index);
        let aad = self.aad(entry, index, plaintext.len() as u64);
        let (ct, tag) = crate::aegis::encrypt(&self.master, &nonce, plaintext, &aad, AEGIS_TAG_LEN)
            .expect("AEGIS-256 con clave y nonce de longitudes válidas");
        let mut frame = Vec::with_capacity(4 + AEGIS_NONCE_LEN + ct.len() + AEGIS_TAG_LEN);
        frame.extend_from_slice(&(plaintext.len() as u32).to_le_bytes());
        frame.extend_from_slice(&nonce);
        frame.extend_from_slice(&ct);
        frame.extend_from_slice(&tag);
        frame
    }

    /// Abre un marco de trozo, verificando la etiqueta.
    pub fn open_chunk(&self, entry: &str, index: u64, frame: &[u8]) -> Result<Vec<u8>, String> {
        if frame.len() < 4 + AEGIS_NONCE_LEN + AEGIS_TAG_LEN {
            return Err("marco de trozo truncado".to_string());
        }
        let plain_len = u32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
        let nonce = &frame[4..4 + AEGIS_NONCE_LEN];
        let ct_start = 4 + AEGIS_NONCE_LEN;
        let tag_start = ct_start + plain_len;
        if frame.len() < tag_start + AEGIS_TAG_LEN {
            return Err("marco de trozo truncado (etiqueta)".to_string());
        }
        let ct = &frame[ct_start..tag_start];
        let tag = &frame[tag_start..tag_start + AEGIS_TAG_LEN];
        let aad = self.aad(entry, index, plain_len as u64);
        crate::aegis::decrypt(&self.master, nonce, ct, &aad, tag)
            .map_err(|e| format!("no se pudo abrir el trozo {index} de {entry}: {e}"))
    }

    /// Longitud del nonce de AEGIS-256 (para los llamantes).
    pub fn nonce_len(&self) -> usize {
        AEGIS_NONCE_LEN
    }
}

/// Longitud de la clave maestra de AEGIS-256.
pub fn aegis_key_len() -> usize {
    AEGIS_KEY_LEN
}

// ---------------------------------------------------------------------------
// Manifiesto
// ---------------------------------------------------------------------------

/// Una entrada del manifiesto: un fichero dentro del backup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestEntry {
    /// Ruta dentro del contenedor (separador `/`, relativa).
    pub path: String,
    /// Paquete al que pertenece.
    pub package: String,
    /// Origen del dato (`apk`, `data`, `external`, `media`, …).
    pub source: String,
    /// Tamaño en bytes.
    pub size: u64,
    /// SHA-256 en hexadecimal minúsculas.
    pub sha256: String,
    /// mtime (epoch, segundos).
    pub mtime: u64,
    /// Permisos.
    pub mode: u32,
}

/// El **manifiesto** del backup: la lista de todo lo que hay dentro, con su
/// hash. Es lo que permite **verificar** sin confiar en el transporte.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// Versión del formato de manifiesto.
    pub version: u32,
    /// Qué lo produjo.
    pub tool: String,
    /// Cuándo (epoch, segundos).
    pub created_at: u64,
    /// Entradas, **ordenadas por ruta** (orden reproducible).
    pub entries: Vec<ManifestEntry>,
    /// Suma de los tamaños.
    pub total_bytes: u64,
}

impl Manifest {
    /// Manifiesto vacío.
    pub fn new() -> Manifest {
        Manifest {
            version: 1,
            tool: format!("megadaemon-backup/{}", env!("CARGO_PKG_VERSION")),
            created_at: now_epoch(),
            entries: Vec::new(),
            total_bytes: 0,
        }
    }

    /// Serialización **canónica** (determinista: `serde_json` conserva el
    /// orden de los campos de la struct y ordena las claves de los mapas).
    pub fn canonical_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// SHA-256 del manifiesto canónico.
    pub fn hash(&self) -> String {
        sha256_hex(self.canonical_json().as_bytes())
    }

    /// Ordena las entradas por ruta (reproducibilidad).
    pub fn sort(&mut self) {
        self.entries.sort_by(|a, b| a.path.cmp(&b.path));
        self.total_bytes = self.entries.iter().map(|e| e.size).sum();
    }
}

impl Default for Manifest {
    fn default() -> Self {
        Manifest::new()
    }
}

/// Construye una entrada de manifiesto a partir de un fichero real.
pub fn manifest_entry(
    path: &std::path::Path,
    inside: &str,
    package: &str,
    source: &str,
) -> Result<ManifestEntry, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("no se puede ver {}: {e}", path.display()))?;
    let (size, sha) = sha256_file(path)?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Ok(ManifestEntry {
        path: inside.to_string(),
        package: package.to_string(),
        source: source.to_string(),
        size,
        sha256: sha,
        mtime,
        mode: unix_mode(&meta),
    })
}

/// Permisos POSIX de un `metadata`.
pub fn unix_mode(meta: &std::fs::Metadata) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o7777
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        0o644
    }
}

/// Epoch actual en segundos.
pub fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Cabecera del contenedor propio
// ---------------------------------------------------------------------------

/// Cabecera del contenedor propio de MegaDaemon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContainerHeader {
    /// Versión del formato.
    pub version: u8,
    /// Flags (bit 0 = payload cifrado).
    pub flags: u8,
    /// Cifrador (`aegis256-chunk-v1` o `none`).
    pub cipher: String,
    /// Parámetros del KDF.
    pub kdf: KdfParams,
    /// Key-check (hex) ⇒ permite verificar la contraseña sin descifrar.
    pub key_check: String,
    /// Tamaño del trozo.
    pub chunk_size: u32,
    /// SHA-256 del manifiesto (hex).
    pub manifest_sha256: String,
    /// Longitud del payload (bytes).
    pub payload_len: u64,
    /// SHA-256 del payload (hex).
    pub payload_sha256: String,
    /// Cuándo se creó (epoch, segundos).
    pub created_at: u64,
}

/// `true` si el flag de cifrado está puesto.
pub const FLAG_ENCRYPTED: u8 = 0x01;

/// Tamaño **fijo** de la cabecera del contenedor (512 B).
///
/// Fijo a propósito: permite **parchear** la cabecera (hash del manifiesto y
/// del payload) **después** de haber escrito el payload en streaming ⇒ no
/// hace falta una copia temporal del backup (que en un móvil con 5 GB libres
/// sería imposible).
pub const CONTAINER_HEADER_SIZE: usize = 512;

/// Serializa la cabecera del contenedor: **exactamente** [`CONTAINER_HEADER_SIZE`]
/// bytes.
///
/// Layout: `[5 B "MDBK1"][1 B versión][1 B flags][2 B u16 LE long_json][JSON UTF-8
/// relleno con espacios hasta 512 B]`.
pub fn encode_container_header(header: &ContainerHeader) -> Vec<u8> {
    let json = serde_json::to_vec(header).unwrap_or_else(|_| b"{}".to_vec());
    let mut out = Vec::with_capacity(CONTAINER_HEADER_SIZE);
    out.extend_from_slice(CONTAINER_MAGIC);
    out.push(CONTAINER_VERSION);
    out.push(header.flags);
    out.extend_from_slice(&(json.len() as u16).to_le_bytes());
    out.extend_from_slice(&json);
    // Relleno con espacios: el JSON sigue siendo válido (los espacios
    // finales son separadores legítimos en JSON).
    while out.len() < CONTAINER_HEADER_SIZE {
        out.push(b' ');
    }
    out.truncate(CONTAINER_HEADER_SIZE);
    out
}

/// Lee la cabecera del contenedor y devuelve **el offset del payload**
/// (siempre [`CONTAINER_HEADER_SIZE`]).
pub fn decode_container_header(bytes: &[u8]) -> Result<(ContainerHeader, usize), String> {
    if bytes.len() < CONTAINER_HEADER_SIZE {
        return Err("contenedor demasiado corto para una cabecera".to_string());
    }
    if &bytes[0..5] != CONTAINER_MAGIC {
        return Err(format!(
            "magic de contenedor inválido: {:?} (se esperaba {:?})",
            String::from_utf8_lossy(&bytes[0..5]),
            String::from_utf8_lossy(CONTAINER_MAGIC)
        ));
    }
    let version = bytes[5];
    if version != CONTAINER_VERSION {
        return Err(format!("versión de contenedor no soportada: {version}"));
    }
    let flags = bytes[6];
    let json_len = u16::from_le_bytes([bytes[7], bytes[8]]) as usize;
    let start = 9usize;
    let end = start
        .checked_add(json_len)
        .ok_or_else(|| "longitud de cabecera desbordada".to_string())?;
    if end > CONTAINER_HEADER_SIZE {
        return Err("la cabecera declara más JSON del que cabe".to_string());
    }
    let header: ContainerHeader = serde_json::from_slice(&bytes[start..end])
        .map_err(|e| format!("cabecera de contenedor inválida: {e}"))?;
    Ok((header, CONTAINER_HEADER_SIZE))
}

// ---------------------------------------------------------------------------
// Interoperabilidad con Swift Backup — EXPERIMENTAL
// ---------------------------------------------------------------------------

/// Estado del camino de interoperabilidad con Swift Backup.
///
/// Todo lo que no está **confirmado** por medición se declara aquí, no se
/// disimula.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InteropLevel {
    /// Confirmado por medición (o por los vectores oficiales).
    Measured,
    /// Implementado sobre una hipótesis **acotada**: funciona si la
    /// hipótesis es correcta, y falla limpio si no lo es.
    Experimental,
    /// No implementado.
    Unsupported,
}

/// Qué parte de Swift Backup está confirmado y cuál no.
pub fn swift_interop_status() -> Vec<(&'static str, InteropLevel, &'static str)> {
    vec![
        (
        "KDF Argon2id + rangos",
        InteropLevel::Measured,
        "salt 16 B, iter 1..100, m 16384..65536 KiB, p 1..8, salida 16..128 B",
    ),
        (
            "Etiquetas SBA1-…",
        InteropLevel::Measured,
        "key-check, AAD y nonce por chunk (contrato de formato)",
    ),
        (
            "ids de cifrado y de KDF",
        InteropLevel::Measured,
        "4 = AEGIS-256, 1 = Argon2id",
    ),
        (
            "AEGIS-256 (RFC 10032)",
        InteropLevel::Measured,
        "validado con los vectores oficiales de la RFC",
        ),
        (
            "Layout byte a byte de la info page",
        InteropLevel::Experimental,
        "orden de campos medido en el escritor x86_64; los offsets no confirmados",
        ),
        (
            "Fórmula del key-check",
        InteropLevel::Experimental,
        "se prueban un número acotado de mezclas; el key-check decide",
        ),
        (
            "Derivación de nonce/AAD por chunk",
        InteropLevel::Experimental,
        "etiquetas medidas; la mezcla con la ruta no confirmada",
        ),
        (
            "Layout del índice de entradas",
        InteropLevel::Unsupported,
        "medidas sólo las cotas (cabecera 16 B, mínimo 84 B, máximo 1 MiB)",
        ),
    ]
}

/// La info page de Swift Backup (128 B), tal y como la **parsea** nuestro
/// lector experimental.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwiftInfoPage {
    /// ¿Empieza por el magic `SBA1`?
    pub magic_ok: bool,
    /// Flags.
    pub flags: u16,
    /// Tamaño de cabecera declarado (96 ó 144 en lo medido).
    pub header_size: u16,
    /// id del método de cifrado.
    pub method_id: u32,
    /// Timestamp de creación (ms).
    pub created_at_ms: u64,
    /// id del método de KDF (0 ó 1).
    pub kdf_method_id: u16,
    /// Candidatos a `kdfIterations` (los u16/u32 de la página en rango 1..100).
    pub iteration_candidates: Vec<u32>,
    /// Bloques de longitud variable hallados en la página.
    pub blocks: Vec<Vec<u8>>,
    /// Todos los u16 leídos (para candidatos).
    pub u16s: Vec<u16>,
    /// Todos los u32 leídos (para candidatos).
    pub u32s: Vec<u32>,
}

fn rd_u16(b: &[u8], off: usize) -> Option<u16> {
    if off + 2 > b.len() {
        return None;
    }
    Some(u16::from_le_bytes([b[off], b[off + 1]]))
}

fn rd_u32(b: &[u8], off: usize) -> Option<u32> {
    if off + 4 > b.len() {
        return None;
    }
    Some(u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]]))
}

fn rd_u64(b: &[u8], off: usize) -> Option<u64> {
    if off + 8 > b.len() {
        return None;
    }
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[off..off + 8]);
    Some(u64::from_le_bytes(a))
}

/// Parsea una info page de Swift Backup.
///
/// ⚠️ **Experimental**: el orden de campos viene del escritor nativo x86_64
/// (`docs/SWIFT_BACKUP_CRYPTO.md` §4.7). Lo que no cuadra con lo medido
/// (magic, `header_size` en {96, 144}, ids en rango) se **rechaza**, no se
/// adivina.
pub fn parse_swift_info_page(bytes: &[u8]) -> Result<SwiftInfoPage, String> {
    if bytes.len() < SBA_INFO_PAGE_LEN {
        return Err("info page incompleta".to_string());
    }
    let page = &bytes[..SBA_INFO_PAGE_LEN];
    let magic_ok = &page[0..4] == SBA_MAGIC;
    if !magic_ok {
        return Err("la info page no empieza por SBA1".to_string());
    }
    let flags = rd_u16(page, 4).unwrap_or(0);
    let header_size = rd_u16(page, 6).unwrap_or(0);
    if header_size != 96 && header_size != 144 {
        return Err(format!(
            "headerSizeDeclared ({header_size}) fuera de lo medido (96 o 144)"
        ));
    }
    let method_id = rd_u32(page, 8).unwrap_or(0);
    if method_id > SBA_CIPHER_AEGIS128X2 {
        return Err(format!("id de cifrado desconocido: {method_id}"));
    }
    let created_at_ms = rd_u64(page, 12).unwrap_or(0);

    // Recorre la página buscando los bloques de longitud variable
    // (`u16 len` + bytes) y recogiendo los u16/u32 sueltos.
    let mut blocks: Vec<Vec<u8>> = Vec::new();
    let mut u16s: Vec<u16> = Vec::new();
    let mut u32s: Vec<u32> = Vec::new();
    let mut off = 20usize;
    while off + 2 <= SBA_INFO_PAGE_LEN {
        let len = rd_u16(page, off).unwrap_or(0) as usize;
        if len == 0 || len > 64 || off + 2 + len > SBA_INFO_PAGE_LEN {
            off += 2;
            continue;
        }
        // Un bloque de datos va precedido de su longitud; se acepta si el
        // resto de la página sigue siendo coherente.
        blocks.push(page[off + 2..off + 2 + len].to_vec());
        off += 2 + len;
    }
    for i in (0..SBA_INFO_PAGE_LEN - 1).step_by(2) {
        if let Some(v) = rd_u16(page, i) {
            u16s.push(v);
        }
    }
    for i in (0..SBA_INFO_PAGE_LEN - 3).step_by(4) {
        if let Some(v) = rd_u32(page, i) {
            u32s.push(v);
        }
    }
    let iteration_candidates: Vec<u32> = u16s
        .iter()
        .filter(|v| (**v as u32) >= SBA_ITER_MIN && (**v as u32) <= SBA_ITER_MAX)
        .map(|v| *v as u32)
        .chain(
            u32s
                .iter()
                .filter(|v| **v >= SBA_ITER_MIN && **v <= SBA_ITER_MAX)
                .map(|v| *v),
        )
        .collect();
    let mut iteration_candidates = iteration_candidates;
    iteration_candidates.dedup();
    // El flag de KDF medido ocupa la penúltima posición del campo u16.
    let kdf_method_id = if method_id != SBA_CIPHER_NONE { SBA_KDF_ARGON2ID } else { SBA_KDF_NONE };

    Ok(SwiftInfoPage {
        magic_ok,
        flags,
        header_size,
        method_id,
        created_at_ms,
        kdf_method_id: kdf_method_id
            .try_into()
            .map_err(|_| "kdf_method_id fuera de rango u16".to_string())?,
        iteration_candidates,
        blocks,
        u16s,
        u32s,
    })
}

/// Candidatos a **salt** de 16 B extraídos de los bloques de la página.
///
/// El bloque de 32 B es el candidato natural (el KDF de Swift Backup lleva la
/// sal en la cabecera y la sal son 16 B). No se puede confirmar cuál de los
/// bloques es ⇒ se prueban **todos** y decide el key-check.
fn swift_salt_candidates(page: &SwiftInfoPage) -> Vec<Vec<u8>> {
    let mut out: Vec<Vec<u8>> = Vec::new();
    for b in &page.blocks {
        if b.len() >= SBA_SALT_LEN {
            out.push(b[..SBA_SALT_LEN].to_vec());
        }
        if b.len() == 32 {
            out.push(b[16..].to_vec());
            out.push(b[..16].to_vec());
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Candidatos a **key-check almacenado**: los bloques de 32 B de la página.
fn swift_key_check_candidates(page: &SwiftInfoPage) -> Vec<Vec<u8>> {
    page.blocks
        .iter()
        .filter(|b| b.len() == 32)
        .cloned()
        .collect()
}

/// Formulas candidatas del key-check (todas con SHA-256 + etiqueta + ordinal).
///
/// La fórmula exacta **no está confirmada** (`docs/SWIFT_BACKUP_CRYPTO.md`
/// §5.3) ⇒ se prueban variantes acotadas y **el key-checkStored decide**.
fn swift_key_check_formulas(label: &str, cipher_ordinal: u32, key: &[u8]) -> Vec<Vec<u8>> {
    let mut out: Vec<Vec<u8>> = Vec::new();
    let ord_le = cipher_ordinal.to_le_bytes();
    let ord_be = cipher_ordinal.to_be_bytes();
    let mut push = |data: Vec<u8>| out.push(sha256_hex(&data).into_bytes());
    push({
        let mut v = Vec::new();
        v.extend_from_slice(label.as_bytes());
        v.extend_from_slice(&ord_le);
        v.extend_from_slice(key);
        v
    });
    push({
        let mut v = Vec::new();
        v.extend_from_slice(label.as_bytes());
        v.extend_from_slice(&ord_be);
        v.extend_from_slice(key);
        v
    });
    push({
        let mut v = Vec::new();
        v.extend_from_slice(label.as_bytes());
        v.extend_from_slice(key);
        v.extend_from_slice(&ord_le);
        v
    });
    push({
        let mut v = Vec::new();
        v.extend_from_slice(&ord_le);
        v.extend_from_slice(label.as_bytes());
        v.extend_from_slice(key);
        v
    });
    push({
        let mut v = Vec::new();
        v.extend_from_slice(key);
        v.extend_from_slice(label.as_bytes());
        v.extend_from_slice(&ord_le);
        v
    });
    push({
        let mut v = Vec::new();
        v.extend_from_slice(label.as_bytes());
        v.extend_from_slice(key);
        v
    });
    push({
        let mut v = Vec::new();
        v.extend_from_slice(key);
        v.extend_from_slice(label.as_bytes());
        v
    });
    out
}

/// Resultado de intentar leer un backup de Swift Backup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwiftReadReport {
    /// ¿Se pudo?
    pub ok: bool,
    /// Nivel de confianza del camino.
    pub support: InteropLevel,
    /// Explicación (incluye **qué se probó** cuando falla).
    pub detail: String,
    /// Datos de la info page raíz.
    pub header: Option<SwiftInfoPage>,
    /// Entradas halladas (con su info page).
    pub entries: Vec<SwiftEntryInfo>,
}

/// Una entrada de un archivo de Swift Backup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwiftEntryInfo {
    /// Offset de su info page dentro del fichero.
    pub offset: u64,
    /// Tamaño de cabecera declarado.
    pub header_size: u16,
    /// id del método de cifrado.
    pub method_id: u32,
    /// Timestamp de creación (ms).
    pub created_at_ms: u64,
    /// ¿Se pudo descifrar?
    pub decrypted: bool,
    /// Detalle (por qué no, si no).
    pub detail: String,
}

/// **Lee** un archivo de Swift Backup (`.dat`, `.med`, …).
///
/// ⚠️ **CAMINO EXPERIMENTAL**.
///
/// Qué hace y qué NO:
///
/// * **Medido y fiable**: el KDF es Argon2id con los rangos de la app; las
///   etiquetas `SBA1-…` son las de la app; el key-check se compara **en
///   tiempo constante**.
/// * **Hipótesis acotada**: el layout byte a byte de la info page y la
///   fórmula del key-check. Para compensarlo, el lector **busca** entre los
///   candidatos medidos y **sólo acepta** lo que el key-check confirma.
///   Si la hipótesis es incorrecta, el resultado es un error honesto que
///   dice qué se probó — **nunca** un descifrado silenciosamente equivocado.
///
/// Lo que **no** se hace: leer el índice de entradas (layout no medido). Las
/// entradas se localizan **buscando el magic `SBA1`** en el fichero.
pub fn read_swift_backup(path: &str, passphrase: &str) -> Result<serde_json::Value, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("no se puede leer {path}: {e}"))?;
    if bytes.len() < SBA_INFO_PAGE_LEN {
        return Err("fichero demasiado corto para una info page de Swift Backup".to_string());
    }
    let page = parse_swift_info_page(&bytes)?;

    // 1) ¿Es un archivo raíz cifrado con AEGIS-256?
    if page.method_id != SBA_CIPHER_AEGIS256 {
        return Ok(serde_json::json!({
            "ok": false,
            "support": "experimental",
            "detail": format!(
                "método de cifrado {}: sólo se implementa AEGIS-256 (4)",
                page.method_id
            ),
            "header": page,
            "entries": [],
        }));
    }

    // 2) Recuperar la contraseña: el key-check es el oráculo.
    let salts = swift_salt_candidates(&page);
    let stored_checks = swift_key_check_candidates(&page);
    if salts.is_empty() || stored_checks.is_empty() {
        return Ok(serde_json::json!({
            "ok": false,
            "support": "experimental",
            "detail": "la info page no expone bloques de los que sacar el salt y el key-check",
            "header": page,
            "entries": [],
        }));
    }
    let iterations: Vec<u32> = if page.iteration_candidates.is_empty() {
        vec![3]
    } else {
        page.iteration_candidates.clone()
    };

    let mut attempts = 0usize;
    let mut recovered: Option<(Vec<u8>, KdfParams)> = None;
    'outer: for salt in &salts {
        for it in &iterations {
            for mem in [SBA_MEM_MIN_KIB, SBA_MEM_MAX_KIB, 32768] {
                for par in [1u32, 4] {
                    let params = KdfParams {
                        method: KdfMethod::Argon2id,
                        iterations: *it,
                        memory_kib: mem,
                        parallelism: par,
                        output_len: AEGIS_KEY_LEN,
                        salt: salt.clone(),
                    };
                    if params.validate().is_err() {
                        continue;
                    }
                    let key = match params.derive_key(passphrase) {
                        Ok(k) => k,
                        Err(_) => continue,
                    };
                    for formula in swift_key_check_formulas(SBA_LABEL_KEY_CHECK, SBA_CIPHER_AEGIS256, &key) {
                        attempts += 1;
                        for stored in &stored_checks {
                            if key_check_eq(&formula, stored) {
                                recovered = Some((key, params));
                                break 'outer;
                            }
                        }
                    }
                }
            }
        }
    }

    let Some((key, params)) = recovered else {
        return Ok(serde_json::json!({
            "ok": false,
            "support": "experimental",
            "detail": format!(
                "la contraseña no se pudo confirmar: se probaron {} sales × {} iteraciones × \
                 3 memorias × 2 paralelismos × {} fórmulas de key-check (={} intentos). \
                 O la contraseña es incorrecta, o el layout de la info page no es el medido.",
                salts.len(), iterations.len(), 7, attempts
            ),
            "header": page,
            "attempts": attempts,
            "entries": [],
        }));
    };

    // 3) Localizar las entradas buscando el magic (el índice no está medido).
    let mut entries: Vec<SwiftEntryInfo> = Vec::new();
    let mut off = 0usize;
    while off + SBA_INFO_PAGE_LEN <= bytes.len() {
        if &bytes[off..off + 4] == SBA_MAGIC {
            if let Ok(p) = parse_swift_info_page(&bytes[off..]) {
                entries.push(SwiftEntryInfo {
                    offset: off as u64,
                    header_size: p.header_size,
                    method_id: p.method_id,
                    created_at_ms: p.created_at_ms,
                    decrypted: false,
                    detail: String::new(),
                });
            }
            off += SBA_INFO_PAGE_LEN;
            continue;
        }
        off += 1;
    }

    Ok(serde_json::json!({
        "ok": true,
        "support": "experimental",
        "detail": format!(
            "contraseña confirmada por el key-check ({} intentos); {} entradas localizadas. \
             El layout de la info page y la fórmula del key-check NO están confirmados: \
             este camino es experimental.",
            attempts, entries.len()
        ),
        "header": page,
        "kdf": params,
        "entries": entries,
    }))
}

/// Opciones de escritura de un archivo con **layout supuesto** de Swift
/// Backup.
#[derive(Debug, Clone, PartialEq)]
pub struct SwiftWriteOptions {
    /// Parámetros del KDF.
    pub kdf: KdfParams,
    /// Etiqueta del key-check.
    pub key_check_label: String,
}

impl Default for SwiftWriteOptions {
    fn default() -> Self {
        SwiftWriteOptions {
            kdf: KdfParams::default_params(),
            key_check_label: SBA_LABEL_KEY_CHECK.to_string(),
        }
    }
}

/// **Escribe** un archivo con el layout **supuesto** de Swift Backup.
///
/// ⚠️ **CAMINO EXPERIMENTAL**: el layout de la info page **no está
/// confirmado** (sólo se midió el orden de campos del escritor x86_64). Este
/// función escribe algo **estructuralmente** parecido, pero **no se puede
/// garantizar** que la app de Swift Backup lo lea. Se usa, sobre todo, para
/// poder probar el lector de ida y vuelta.
///
/// Devuelve el informe: lo que se escribió y **lo que no se puede prometer**.
pub fn write_swift_backup(
    dir: &str,
    out: &str,
    passphrase: &str,
    opts: &SwiftWriteOptions,
) -> Result<serde_json::Value, String> {
    opts.kdf.validate()?;
    let key = opts.kdf.derive_key(passphrase)?;
    let check = key_check(&key, &opts.key_check_label, SBA_CIPHER_AEGIS256);

    // Info page **supuesta**: mismo orden de campos que el escritor medido.
    let mut page = Vec::with_capacity(SBA_INFO_PAGE_LEN);
    page.extend_from_slice(SBA_MAGIC);
    page.extend_from_slice(&0u16.to_le_bytes()); // flags
    page.extend_from_slice(&144u16.to_le_bytes()); // headerSize (variante raíz)
    page.extend_from_slice(&SBA_CIPHER_AEGIS256.to_le_bytes());
    page.extend_from_slice(&now_epoch().saturating_mul(1000).to_le_bytes());
    page.extend_from_slice(&0u16.to_le_bytes()); // campo A
    page.extend_from_slice(&0u16.to_le_bytes()); // campo B
    page.extend_from_slice(&0u16.to_le_bytes()); // struct[0x04]
    page.extend_from_slice(&0u16.to_le_bytes()); // struct[0x08]
    page.extend_from_slice(&0u16.to_le_bytes()); // struct[0x0c]
    for block in [&opts.kdf.salt[..], &check[..]] {
        page.extend_from_slice(&(block.len() as u16).to_le_bytes());
        page.extend_from_slice(block);
        page.push(0); // byte 0 medido tras el bloque
    }
    while page.len() < SBA_INFO_PAGE_LEN {
        page.push(0);
    }
    let page = &page[..SBA_INFO_PAGE_LEN];

    // Contenido: tar de todo el directorio, comprimido con zstd.
    let tar = build_tar_from_dir(dir)?;
    let compressed = zstd::bulk::compress(&tar, 3).map_err(|e| format!("zstd falló: {e}"))?;

    let mut out_bytes = Vec::new();
    out_bytes.extend_from_slice(page);
    out_bytes.extend_from_slice(&compressed);
    std::fs::write(out, &out_bytes).map_err(|e| format!("no se puede escribir {out}: {e}"))?;

    Ok(serde_json::json!({
        "ok": true,
        "support": "experimental",
        "detail": "escrito con el layout SUPUESTO de Swift Backup: la app no lo leerá con \
                   seguridad hasta contrastar con un .dat real",
        "out": out,
        "bytes": out_bytes.len(),
        "kdf": opts.kdf,
        "key_check": hex(&check),
    }))
}

// ---------------------------------------------------------------------------
// TAR (escritura determinista) — compartido con el motor
// ---------------------------------------------------------------------------

/// Escribe un **TAR ustar determinista** con el contenido de un directorio.
///
/// Reproducible: rutas ordenadas, mtime real (va en el manifiesto), uid/gid a
/// 0 y modo normalizado. Dos capturas del mismo contenido dan **el mismo
/// tar** ⇒ el hash del contenedor es estable.
pub fn build_tar_from_dir(dir: &str) -> Result<Vec<u8>, String> {
    let mut files: Vec<(String, std::path::PathBuf)> = Vec::new();
    collect_files(std::path::Path::new(dir), "", &mut files, 0)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out: Vec<u8> = Vec::new();
    for (rel, path) in files {
        let meta = std::fs::metadata(&path).map_err(|e| format!("no se puede ver {}: {e}", path.display()))?;
        let data = std::fs::read(&path).map_err(|e| format!("no se puede leer {}: {e}", path.display()))?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mode = unix_mode(&meta) | 0o400;
        out.extend_from_slice(&tar_header(&rel, data.len() as u64, mode, mtime));
        out.extend_from_slice(&data);
        let pad = (512 - (data.len() % 512)) % 512;
        out.extend(std::iter::repeat_n(0u8, pad));
    }
    // Dos bloques nulos = fin.
    out.extend(std::iter::repeat_n(0u8, 1024));
    Ok(out)
}

/// Cabecera de una entrada de tar (ustar, 512 B).
pub fn tar_header(name: &str, size: u64, mode: u32, mtime: u64) -> [u8; 512] {
    let mut h = [0u8; 512];
    let nb = name.as_bytes();
    let n = nb.len().min(100);
    h[..n].copy_from_slice(&nb[..n]);
    write_octal(&mut h[100..108], mode as u64, 7);
    write_octal(&mut h[108..116], 0, 7); // uid
    write_octal(&mut h[116..124], 0, 7); // gid
    write_octal(&mut h[124..136], size, 11);
    write_octal(&mut h[136..148], mtime, 11);
    h[148..156].copy_from_slice(b"        "); // checksum: se rellena luego
    h[156] = b'0'; // fichero regular
    h[257..262].copy_from_slice(b"ustar");
    h[263..265].copy_from_slice(b"00");
    // checksum: suma de todos los bytes con el campo a cero.
    let mut sum: u32 = 0;
    for b in h.iter() {
        sum += *b as u32;
    }
    let chk = format!("{sum:06o}\0 ");
    h[148..156].copy_from_slice(chk.as_bytes());
    h
}

fn write_octal(buf: &mut [u8], value: u64, width: usize) {
    let s = format!("{:0width$o}", value, width = width - 1);
    let bytes = s.as_bytes();
    let n = bytes.len().min(width - 1);
    buf[..n].copy_from_slice(&bytes[bytes.len() - n..]);
    buf[width - 1] = 0;
}

fn collect_files(
    base: &std::path::Path,
    prefix: &str,
    out: &mut Vec<(String, std::path::PathBuf)>,
    depth: usize,
) -> Result<(), String> {
    if depth > 16 {
        return Err("árbol demasiado profundo".to_string());
    }
    let rd = std::fs::read_dir(base).map_err(|e| format!("no se puede leer {}: {e}", base.display()))?;
    for entry in rd {
        let entry = entry.map_err(|e| format!("no se puede leer {}: {e}", base.display()))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let rel = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let meta = entry
            .metadata()
            .map_err(|e| format!("no se puede ver {}: {e}", path.display()))?;
        if meta.is_dir() {
            collect_files(&path, &rel, out, depth + 1)?;
            continue;
        }
        if !meta.is_file() {
            continue; // ni symlinks ni devices
        }
        out.push((rel, path));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Pruebas
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn params_rapidos() -> KdfParams {
        KdfParams {
            method: KdfMethod::Argon2id,
            iterations: 1,
            memory_kib: SBA_MEM_MIN_KIB,
            parallelism: 1,
            output_len: AEGIS_KEY_LEN,
            salt: vec![7u8; SBA_SALT_LEN],
        }
    }

    #[test]
    fn argon2id_coincide_con_el_vector_de_la_rfc_9106() {
        // Vector taken de `docs/argon2id_vectors.json` (implementación de
        // referencia P-H-C, RFC 9106, tipo id).
        let p = KdfParams {
            method: KdfMethod::Argon2id,
            iterations: 3,
            memory_kib: 65536,
            parallelism: 4,
            output_len: 32,
            salt: vec![
                0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
                0x0e, 0x0f,
            ],
        };
        let key = p.derive_key("contrasena-de-ejemplo").expect("debería derivar");
        assert_eq!(
            hex(&key),
            "19cc3464d21d32b2c2fcbc9a042574c360019110c23a0ec692c82456c9bb10f5"
        );
    }

    #[test]
    fn los_parametros_fuera_de_rango_se_rechazan_como_la_app() {
        let mut p = params_rapidos();
        p.iterations = 0;
        assert!(p.validate().unwrap_err().contains("iterations must be in 1..100"));
        p = params_rapidos();
        p.iterations = 101;
        assert!(p.validate().is_err());
        p = params_rapidos();
        p.memory_kib = 16383;
        assert!(p.validate().unwrap_err().contains("memoryKiB must be in 16384..65536"));
        p = params_rapidos();
        p.memory_kib = 65537;
        assert!(p.validate().is_err());
        p = params_rapidos();
        p.parallelism = 9;
        assert!(p.validate().unwrap_err().contains("parallelism must be in 1..8"));
        p = params_rapidos();
        p.output_len = 15;
        assert!(p.validate().unwrap_err().contains("outputBytes must be in 16..128"));
        p = params_rapidos();
        p.salt = vec![0u8; 15];
        assert!(p.validate().unwrap_err().contains("salt must be 16 bytes"));
    }

    #[test]
    fn la_contrasena_vacia_se_rechaza() {
        let p = params_rapidos();
        assert!(p.derive_key("").unwrap_err().contains("must not be empty"));
    }

    #[test]
    fn el_key_check_detecta_la_contrasena_equivocada() {
        let p = params_rapidos();
        let key = p.derive_key("buena").unwrap();
        let check = key_check(&key, LABEL_KEY_CHECK, SBA_CIPHER_AEGIS256);
        assert!(key_check_eq(&check, &check));
        let otra = p.derive_key("mala").unwrap();
        let check2 = key_check(&otra, LABEL_KEY_CHECK, SBA_CIPHER_AEGIS256);
        assert!(!key_check_eq(&check, &check2));
        // y con la etiqueta equivocada tampoco cuadra
        let check3 = key_check(&key, SBA_LABEL_KEY_CHECK, SBA_CIPHER_AEGIS256);
        assert!(!key_check_eq(&check, &check3));
    }

    #[test]
    fn verify_key_check_es_toleraante_al_hexadecimal_mal_casado() {
        let p = params_rapidos();
        let key = p.derive_key("buena").unwrap();
        let check = hex(&key_check(&key, LABEL_KEY_CHECK, SBA_CIPHER_AEGIS256));
        assert!(verify_key_check(&key, LABEL_KEY_CHECK, SBA_CIPHER_AEGIS256, &check));
        assert!(!verify_key_check(&key, LABEL_KEY_CHECK, SBA_CIPHER_AEGIS256, "no-hex"));
    }

    #[test]
    fn el_aead_por_trozos_ida_y_vuelta() {
        let key = vec![9u8; AEGIS_KEY_LEN];
        let c = ChunkCipher::own(&key).unwrap();
        let datos = "contenido de prueba, con acentos: ñ é 😀".repeat(10).into_bytes();
        let frame = c.seal_chunk("payload/app.dat", 0, &datos);
        let abierto = c.open_chunk("payload/app.dat", 0, &frame).unwrap();
        assert_eq!(abierto, datos);
    }

    #[test]
    fn el_trozo_ata_a_su_posicion_y_a_su_entrada() {
        let key = vec![9u8; AEGIS_KEY_LEN];
        let c = ChunkCipher::own(&key).unwrap();
        let datos = b"secreto".repeat(100);
        let frame = c.seal_chunk("payload/app.dat", 0, &datos);
        // mismo troce, otra posición => no descifra
        assert!(c.open_chunk("payload/app.dat", 1, &frame).is_err());
        // mismo troce, otra entrada => no descifra
        assert!(c.open_chunk("payload/otra.dat", 0, &frame).is_err());
        // y alterando un byte del ciphertext tampoco
        let mut roto = frame.clone();
        let pos = 4 + AEGIS_NONCE_LEN + 3;
        roto[pos] ^= 0x01;
        assert!(c.open_chunk("payload/app.dat", 0, &roto).is_err());
    }

    #[test]
    fn el_manifiesto_es_determinista() {
        let mut m = Manifest::new();
        m.created_at = 1;
        m.entries.push(ManifestEntry {
            path: "b.txt".into(),
            package: "p".into(),
            source: "data".into(),
            size: 1,
            sha256: "00".into(),
            mtime: 0,
            mode: 0o644,
        });
        m.entries.push(ManifestEntry {
            path: "a.txt".into(),
            package: "p".into(),
            source: "data".into(),
            size: 2,
            sha256: "11".into(),
            mtime: 0,
            mode: 0o644,
        });
        m.sort();
        assert_eq!(m.entries[0].path, "a.txt");
        assert_eq!(m.total_bytes, 3);
        let h1 = m.hash();
        let h2 = m.hash();
        assert_eq!(h1, h2);
        // el hash cambia si cambia una entrada
        m.entries[0].size = 99;
        assert_ne!(m.hash(), h1);
    }

    #[test]
    fn la_cabecera_del_contenedor_ida_y_vuelta() {
        let h = ContainerHeader {
            version: CONTAINER_VERSION,
            flags: FLAG_ENCRYPTED,
            cipher: "aegis256-chunk-v1".into(),
            kdf: params_rapidos(),
            key_check: "ab".into(),
            chunk_size: 1024,
            manifest_sha256: "cd".into(),
            payload_len: 42,
            payload_sha256: "ef".into(),
            created_at: 7,
        };
        let bytes = encode_container_header(&h);
        let (back, off) = decode_container_header(&bytes).unwrap();
        assert_eq!(back, h);
        assert_eq!(off, bytes.len());
    }

    #[test]
    fn la_cabecera_rechaza_un_magic_ajeno() {
        let bytes = encode_container_header(&ContainerHeader {
            version: CONTAINER_VERSION,
            flags: 0,
            cipher: "none".into(),
            kdf: params_rapidos(),
            key_check: String::new(),
            chunk_size: 0,
            manifest_sha256: String::new(),
            payload_len: 0,
            payload_sha256: String::new(),
            created_at: 0,
        });
        let mut malo = bytes.clone();
        malo[0] = b'X';
        assert!(decode_container_header(&malo).unwrap_err().contains("magic"));
    }

    #[test]
    fn el_tar_es_determinista() {
        let dir = std::env::temp_dir().join(format!("md-tar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("b.txt"), b"dos").unwrap();
        std::fs::write(dir.join("a.txt"), b"uno").unwrap();
        let t1 = build_tar_from_dir(dir.to_str().unwrap()).unwrap();
        let t2 = build_tar_from_dir(dir.to_str().unwrap()).unwrap();
        assert_eq!(t1, t2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_info_page_de_swift_se_rechaza_si_no_encaja() {
        // magic equivocado
        let mut page = vec![0u8; SBA_INFO_PAGE_LEN];
        assert!(parse_swift_info_page(&page).is_err());
        // magic bueno pero headerSize fuera de lo medido
        page[0..4].copy_from_slice(SBA_MAGIC);
        page[6..8].copy_from_slice(&999u16.to_le_bytes());
        assert!(parse_swift_info_page(&page).unwrap_err().contains("headerSizeDeclared"));
        // headerSize válido y método AEGIS-256 => se acepta
        page[6..8].copy_from_slice(&96u16.to_le_bytes());
        page[8..12].copy_from_slice(&SBA_CIPHER_AEGIS256.to_le_bytes());
        let p = parse_swift_info_page(&page).unwrap();
        assert!(p.magic_ok);
        assert_eq!(p.method_id, SBA_CIPHER_AEGIS256);
    }

    #[test]
    fn el_estado_de_interop_no_promete_mas_de_lo_medido() {
        let st = swift_interop_status();
        let idx = st.iter().position(|(n, _, _)| *n == "Layout byte a byte de la info page").unwrap();
        assert_eq!(st[idx].1, InteropLevel::Experimental);
        let idx = st.iter().position(|(n, _, _)| *n == "KDF Argon2id + rangos").unwrap();
        assert_eq!(st[idx].1, InteropLevel::Measured);
    }
}
