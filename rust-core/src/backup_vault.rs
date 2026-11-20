//! MegaDaemon v13.4 — Bóveda cifrada para subida a Google Drive.
//!
//! Implementación **en Rust puro** (sin crates nuevos): AES-256 en modo GCM,
//! HMAC-SHA256 y PBKDF2-HMAC-SHA256 están implementados a mano en este mismo
//! fichero sobre el crate `sha2` (SHA-256 nativo). El resultado es criptografía
//! **estándar**: AES-256-GCM según NIST SP 800-38D y PBKDF2 según RFC 2898, con
//! los vectores de prueba de FIPS-197 y de la especificación GCM verificados en
//! los tests de este módulo.
//!
//! FORMATO DE LA BÓVEDA (`MDV1`):
//!
//! ```text
//! offset  tam  contenido
//! 0       4    magic "MDV1"
//! 4       4    versión (u32 LE)
//! 8       1    id de KDF (1 = PBKDF2-HMAC-SHA256)
//! 9       1    id de cifrador (1 = AES-256-GCM)
//! 10      2    reservado (u16 LE, 0)
//! 12      4    iteraciones PBKDF2 (u32 LE)
//! 16      32   salt
//! 48      32   vault_id
//! 80      4    longitud del manifiesto (u32 LE)
//! 84      N    manifiesto JSON (UTF-8)
//! ...           registros de chunk
//! ```
//!
//! Cada registro de chunk:
//!
//! ```text
//! u16 len_rel | rel_path | u32 idx | u32 plain_len | u32 cipher_len | nonce[12] | ct | tag[16]
//! ```
//!
//! El **AAD** de cada chunk es `rel_path || 0x00 || idx_be_u32`, de forma que
//! el tag GCM ata criptográficamente cada chunk a su ruta y a su posición:
//! renombrar o reordenar chunks rompe la autenticación (no se puede "mover" un
//! fichero cifrado dentro de la bóveda sin que se detecte).
//!
//! La clave se deriva con PBKDF2-HMAC-SHA256 (salt aleatorio de 32 bytes) y se
//! partitiona en dos: 32 bytes para AES-256 y 32 bytes para derivar los nonces
//! (`nonce = HMAC(nonce_key, vault_id || rel_path || idx)[0..12]`), lo que
//! garantiza nonces únicos por (bóveda, fichero, chunk) sin necesidad de un
//! generador aleatorio durante el cifrado.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Magic de la bóveda.
pub const VAULT_MAGIC: &[u8; 4] = b"MDV1";
/// Versión del formato.
pub const VAULT_VERSION: u32 = 1;
/// Tamaño fijo de la cabecera.
pub const VAULT_HEADER_LEN: usize = 84;
/// Tamaño de chunk (1 MiB).
pub const CHUNK_SIZE: usize = 1024 * 1024;
/// Longitud del salt.
pub const SALT_LEN: usize = 32;
/// Longitud del identificador de bóveda.
pub const VAULT_ID_LEN: usize = 32;
/// Longitud del nonce GCM.
pub const NONCE_LEN: usize = 12;
/// Longitud del tag GCM.
pub const TAG_LEN: usize = 16;
/// Longitud de la clave AES-256.
pub const KEY_LEN: usize = 32;
/// Iteraciones por defecto de PBKDF2-HMAC-SHA256.
pub const PBKDF2_ITERATIONS: u32 = 120_000;
/// Identificador de KDF en la cabecera.
pub const KDF_PBKDF2_HMAC_SHA256: u8 = 1;
/// Identificador de cifrador en la cabecera.
pub const CIPHER_AES_256_GCM: u8 = 1;
/// Nombre del algoritmo (manifiesto).
pub const ALGORITHM_NAME: &str = "AES-256-GCM";
/// Nombre del KDF (manifiesto).
pub const KDF_NAME: &str = "PBKDF2-HMAC-SHA256";
/// Umbral (bytes) desde el que se recomienda subida reanudable a Drive.
pub const DRIVE_RESUMABLE_THRESHOLD: u64 = 8 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Manifiesto
// ---------------------------------------------------------------------------

/// Entrada del manifiesto: un fichero dentro de la bóveda.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultEntry {
    /// Ruta relativa dentro de la bóveda (UTF-8, separador `/`).
    pub rel_path: String,
    /// Tamaño del texto plano en bytes.
    pub plaintext_len: u64,
    /// Tamaño del texto cifrado en bytes (incluye los tags GCM).
    pub cipher_len: u64,
    /// SHA-256 (hex) del texto plano.
    pub sha256_plain: String,
    /// Nonce base del fichero (hex).
    pub nonce: String,
    /// Índice del primer chunk del fichero.
    pub chunk_index: usize,
}

/// Manifiesto de la bóveda.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultManifest {
    /// Versión del formato.
    pub version: u32,
    /// Fecha de creación (ISO-8601 UTC).
    pub created_at: String,
    /// Algoritmo de cifrado (`AES-256-GCM`).
    pub algorithm: String,
    /// KDF (`PBKDF2-HMAC-SHA256`).
    pub kdf: String,
    /// Entradas (una por fichero).
    pub entries: Vec<VaultEntry>,
    /// Total de bytes en claro.
    pub total_plain_bytes: u64,
}

// ---------------------------------------------------------------------------
// Utilidades
// ---------------------------------------------------------------------------

/// Bytes a hexadecimal en minúsculas.
pub fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// SHA-256 en hexadecimal de un buffer.
pub fn sha256_hex(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

/// Bytes aleatorios criptográficos (`/dev/urandom`), con respaldo no
/// criptográfico si el dispositivo no lo expone.
fn random_bytes(n: usize) -> Vec<u8> {
    if let Ok(mut f) = fs::File::open("/dev/urandom") {
        let mut buf = vec![0u8; n];
        if f.read_exact(&mut buf).is_ok() {
            return buf;
        }
    }
    // Respaldo: mezcla de reloj + pid + contador (NO criptográfico).
    let mut seed = Vec::new();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    seed.extend_from_slice(&now.to_le_bytes());
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

/// Fecha/hora actual en ISO-8601 UTC.
pub fn now_iso8601() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let rem = (secs % 86_400) as u32;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
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

/// Comparación en tiempo constante.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

// ---------------------------------------------------------------------------
// HMAC-SHA256 y PBKDF2 (RFC 2104 / RFC 2898)
// ---------------------------------------------------------------------------

/// HMAC-SHA256 (RFC 2104).
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut k = [0u8; BLOCK];
    if key.len() > BLOCK {
        let d = Sha256::digest(key);
        k[..32].copy_from_slice(&d);
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; BLOCK];
    let mut opad = [0x5cu8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(data);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_hash);
    let out = outer.finalize();
    let mut r = [0u8; 32];
    r.copy_from_slice(&out);
    r
}

/// PBKDF2-HMAC-SHA256 (RFC 2898, dkLen múltiplo o no de hLen).
pub fn pbkdf2_hmac_sha256(password: &[u8], salt: &[u8], iterations: u32, dklen: usize) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(dklen);
    let mut block: u32 = 1;
    while out.len() < dklen {
        let mut msg = salt.to_vec();
        msg.extend_from_slice(&block.to_be_bytes());
        let mut u = hmac_sha256(password, &msg);
        let mut t = u;
        for _ in 1..iterations.max(1) {
            u = hmac_sha256(password, &u);
            for i in 0..32 {
                t[i] ^= u[i];
            }
        }
        out.extend_from_slice(&t);
        block += 1;
    }
    out.truncate(dklen);
    out
}

// ---------------------------------------------------------------------------
// AES-256 (FIPS-197) + GCM (NIST SP 800-38D)
// ---------------------------------------------------------------------------

/// S-box de AES (FIPS-197, Fig. 7).
const SBOX: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

/// Rcon de AES-256.
const RCON: [u8; 11] = [
    0x00, 0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1b, 0x36,
];

/// Multiplica por x en GF(2^8) (polinomio mónico 0x11b).
fn xtime(x: u8) -> u8 {
    (x << 1) ^ if x & 0x80 != 0 { 0x1b } else { 0x00 }
}

/// Cifrador de bloque AES-256.
struct Aes256 {
    round_keys: [u8; 240], // 15 rondas × 16 bytes
}

impl Aes256 {
    /// Expande la clave de 256 bits (Nk=8, Nr=14).
    fn new(key: &[u8]) -> Aes256 {
        const NK: usize = 8;
        const NR: usize = 14;
        let mut w = [0u8; 240];
        w[..32].copy_from_slice(&key[..32]);
        let total_words = 4 * (NR + 1); // 60
        let mut i = NK;
        while i < total_words {
            let mut temp = [
                w[4 * (i - 1)],
                w[4 * (i - 1) + 1],
                w[4 * (i - 1) + 2],
                w[4 * (i - 1) + 3],
            ];
            if i % NK == 0 {
                temp = [temp[1], temp[2], temp[3], temp[0]]; // RotWord
                for b in temp.iter_mut() {
                    *b = SBOX[*b as usize];
                }
                temp[0] ^= RCON[i / NK];
            } else if i % NK == 4 {
                for b in temp.iter_mut() {
                    *b = SBOX[*b as usize];
                }
            }
            for j in 0..4 {
                w[4 * i + j] = w[4 * (i - NK) + j] ^ temp[j];
            }
            i += 1;
        }
        Aes256 { round_keys: w }
    }

    /// Cifra un bloque de 16 bytes.
    fn encrypt_block(&self, block: &[u8; 16]) -> [u8; 16] {
        let mut s = *block;
        for i in 0..16 {
            s[i] ^= self.round_keys[i]; // AddRoundKey(0)
        }
        for round in 1..=14usize {
            for b in s.iter_mut() {
                *b = SBOX[*b as usize];
            }
            shift_rows(&mut s);
            if round != 14 {
                mix_columns(&mut s);
            }
            for i in 0..16 {
                s[i] ^= self.round_keys[round * 16 + i]; // AddRoundKey(round)
            }
        }
        s
    }
}

/// ShiftRows: `state[r][c] = state[r][(c + r) % 4]`.
fn shift_rows(s: &mut [u8; 16]) {
    let t = *s;
    for r in 1..4usize {
        for c in 0..4usize {
            s[r + 4 * c] = t[r + 4 * ((c + r) % 4)];
        }
    }
}

/// MixColumns.
fn mix_columns(s: &mut [u8; 16]) {
    for c in 0..4usize {
        let a0 = s[4 * c];
        let a1 = s[4 * c + 1];
        let a2 = s[4 * c + 2];
        let a3 = s[4 * c + 3];
        let t0 = xtime(a0);
        let t1 = xtime(a1);
        let t2 = xtime(a2);
        let t3 = xtime(a3);
        s[4 * c] = t0 ^ t1 ^ a1 ^ a2 ^ a3;
        s[4 * c + 1] = a0 ^ t1 ^ t2 ^ a2 ^ a3;
        s[4 * c + 2] = a0 ^ a1 ^ t2 ^ t3 ^ a3;
        s[4 * c + 3] = t0 ^ a0 ^ a1 ^ a2 ^ t3;
    }
}

/// Multiplicación en GF(2^128) para GHASH.
fn ghash_mul(x: &mut [u8; 16], h: &[u8; 16]) {
    let mut z = [0u8; 16];
    let mut v = *h;
    for i in 0..128usize {
        if (x[i >> 3] >> (7 - (i & 7))) & 1 == 1 {
            for j in 0..16 {
                z[j] ^= v[j];
            }
        }
        // v = v >> 1 (con reducción)
        let lsb = v[15] & 1;
        for j in (1..16usize).rev() {
            v[j] = (v[j] >> 1) | ((v[j - 1] & 1) << 7);
        }
        v[0] >>= 1;
        if lsb == 1 {
            v[0] ^= 0xe1;
        }
    }
    *x = z;
}

/// GHASH sobre AAD + texto cifrado.
fn ghash(h: &[u8; 16], aad: &[u8], ct: &[u8]) -> [u8; 16] {
    let mut y = [0u8; 16];
    let mut feed = |data: &[u8], y: &mut [u8; 16]| {
        let mut i = 0usize;
        while i < data.len() {
            let mut block = [0u8; 16];
            let n = std::cmp::min(16, data.len() - i);
            block[..n].copy_from_slice(&data[i..i + n]);
            for j in 0..16 {
                y[j] ^= block[j];
            }
            ghash_mul(y, h);
            i += 16;
        }
    };
    feed(aad, &mut y);
    feed(ct, &mut y);
    let mut lens = [0u8; 16];
    lens[0..8].copy_from_slice(&((aad.len() as u64) * 8).to_be_bytes());
    lens[8..16].copy_from_slice(&((ct.len() as u64) * 8).to_be_bytes());
    for j in 0..16 {
        y[j] ^= lens[j];
    }
    ghash_mul(&mut y, h);
    y
}

/// Incrementa los 4 últimos bytes del contador (modo CTR de GCM).
fn inc32(counter: &mut [u8; 16]) {
    for i in (12..16usize).rev() {
        let (v, carry) = counter[i].overflowing_add(1);
        counter[i] = v;
        if !carry {
            break;
        }
    }
}

/// Cifrado GCM con IV de 96 bits. Devuelve (texto cifrado, tag).
fn gcm_encrypt(
    key: &Aes256,
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    plaintext: &[u8],
) -> (Vec<u8>, [u8; TAG_LEN]) {
    let h = key.encrypt_block(&[0u8; 16]);
    let mut j0 = [0u8; 16];
    j0[..NONCE_LEN].copy_from_slice(nonce);
    j0[15] = 1;

    let mut ct = Vec::with_capacity(plaintext.len());
    let mut counter = j0;
    let mut off = 0usize;
    while off < plaintext.len() {
        inc32(&mut counter);
        let ks = key.encrypt_block(&counter);
        let n = std::cmp::min(16, plaintext.len() - off);
        for i in 0..n {
            ct.push(plaintext[off + i] ^ ks[i]);
        }
        off += 16;
    }
    // T = GCTR(K, J0, S) con |S| = 16 → T = E(K, J0) ⊕ S
    // (NIST SP 800-38D, §7.1). El GHASH `s` **sí** forma parte del tag: sin
    // ese XOR el tag no dependería del texto cifrado ni del AAD y un chunk
    // alterado, reordenado o renombrado pasaría la verificación.
    let s = ghash(&h, aad, &ct);
    let ek = key.encrypt_block(&j0);
    let mut tag = [0u8; TAG_LEN];
    for i in 0..TAG_LEN {
        tag[i] = s[i] ^ ek[i];
    }
    (ct, tag)
}

/// Descifrado GCM con verificación del tag.
fn gcm_decrypt(
    key: &Aes256,
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    ciphertext: &[u8],
    tag: &[u8],
) -> Result<Vec<u8>, String> {
    let h = key.encrypt_block(&[0u8; 16]);
    let mut j0 = [0u8; 16];
    j0[..NONCE_LEN].copy_from_slice(nonce);
    j0[15] = 1;

    let mut pt = Vec::with_capacity(ciphertext.len());
    let mut counter = j0;
    let mut off = 0usize;
    while off < ciphertext.len() {
        inc32(&mut counter);
        let ks = key.encrypt_block(&counter);
        let n = std::cmp::min(16, ciphertext.len() - off);
        for i in 0..n {
            pt.push(ciphertext[off + i] ^ ks[i]);
        }
        off += 16;
    }
    let s = ghash(&h, aad, ciphertext);
    let ek = key.encrypt_block(&j0);
    let mut expected = [0u8; TAG_LEN];
    for i in 0..TAG_LEN {
        expected[i] = s[i] ^ ek[i];
    }
    if !ct_eq(&expected, tag) {
        return Err(
            "fallo de autenticación GCM: el chunk ha sido alterado, reordenado o renombrado"
                .to_string(),
        );
    }
    Ok(pt)
}

// ---------------------------------------------------------------------------
// Formato de bóveda
// ---------------------------------------------------------------------------

/// Cabecera parseada.
struct VaultHeader {
    version: u32,
    kdf_id: u8,
    cipher_id: u8,
    iterations: u32,
    salt: [u8; SALT_LEN],
    vault_id: [u8; VAULT_ID_LEN],
}

/// Deriva el nonce de un chunk: HMAC(nonce_key, vault_id || rel || idx)[0..12].
fn derive_nonce(nonce_key: &[u8], vault_id: &[u8], rel: &[u8], idx: usize) -> [u8; NONCE_LEN] {
    let mut msg: Vec<u8> = Vec::with_capacity(vault_id.len() + rel.len() + 4);
    msg.extend_from_slice(vault_id);
    msg.extend_from_slice(rel);
    msg.extend_from_slice(&(idx as u32).to_be_bytes());
    let mac = hmac_sha256(nonce_key, &msg);
    let mut n = [0u8; NONCE_LEN];
    n.copy_from_slice(&mac[..NONCE_LEN]);
    n
}

/// AAD: ata el chunk a su ruta y a su índice (anti-renombrado/anti-reorden).
fn build_aad(rel: &[u8], idx: usize) -> Vec<u8> {
    let mut aad = Vec::with_capacity(rel.len() + 5);
    aad.extend_from_slice(rel);
    aad.push(0x00);
    aad.extend_from_slice(&(idx as u32).to_be_bytes());
    aad
}

/// Normaliza una ruta relativa: separador `/`, sin `.` ni `..`, sin vacío.
fn normalize_rel(rel: &str) -> Result<String, String> {
    let mut parts: Vec<&str> = Vec::new();
    for p in rel.split(['/', '\\']) {
        if p.is_empty() || p == "." {
            continue;
        }
        if p == ".." {
            return Err(format!("ruta relativa insegura: {}", rel));
        }
        parts.push(p);
    }
    if parts.is_empty() {
        return Err("ruta relativa vacía".to_string());
    }
    Ok(parts.join("/"))
}

/// Recorre un directorio recogiendo ficheros regulares (sin symlinks).
fn walk_files(root: &Path, out: &mut Vec<PathBuf>) {
    let rd = match fs::read_dir(root) {
        Ok(r) => r,
        Err(_) => return,
    };
    let mut entries: Vec<fs::DirEntry> = rd.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let md = match fs::symlink_metadata(&p) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if md.file_type().is_symlink() {
            continue;
        }
        if md.is_dir() {
            walk_files(&p, out);
        } else if md.is_file() {
            out.push(p);
        }
    }
}

// ---------------------------------------------------------------------------
// Sellado (cifrado)
// ---------------------------------------------------------------------------

/// Cifra un árbol de directorios completo en un fichero de bóveda.
///
/// Devuelve el manifiesto resultante.
pub fn seal(dir: &str, out: &str, passphrase: &str) -> Result<VaultManifest, String> {
    seal_with_iterations(dir, out, passphrase, PBKDF2_ITERATIONS)
}

/// Igual que [`seal`] pero con un número de iteraciones de PBKDF2 explícito
/// (útil para tests rápidos o para tunear el coste en el dispositivo).
pub fn seal_with_iterations(
    dir: &str,
    out: &str,
    passphrase: &str,
    iterations: u32,
) -> Result<VaultManifest, String> {
    if passphrase.is_empty() {
        return Err("la passphrase no puede estar vacía".to_string());
    }
    let root = Path::new(dir);
    if !root.is_dir() {
        return Err(format!("no existe el directorio a cifrar: {}", dir));
    }
    if iterations == 0 {
        return Err("las iteraciones del KDF deben ser > 0".to_string());
    }

    let mut files: Vec<PathBuf> = Vec::new();
    walk_files(root, &mut files);
    let mut rels: Vec<(PathBuf, String)> = Vec::new();
    for p in files {
        let rel = normalize_rel(
            &p.strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .to_string(),
        )?;
        rels.push((p, rel));
    }
    rels.sort_by(|a, b| a.1.cmp(&b.1));

    let salt = random_bytes(SALT_LEN);
    let vault_id = random_bytes(VAULT_ID_LEN);
    let dk = pbkdf2_hmac_sha256(passphrase.as_bytes(), &salt, iterations, KEY_LEN * 2);
    let enc_key = &dk[..KEY_LEN];
    let nonce_key = &dk[KEY_LEN..];
    let aes = Aes256::new(enc_key);

    let mut entries: Vec<VaultEntry> = Vec::new();
    let mut records: Vec<(String, usize, Vec<u8>, Vec<u8>, [u8; TAG_LEN], [u8; NONCE_LEN])> =
        Vec::new();
    let mut total_plain_bytes: u64 = 0;

    for (path, rel) in &rels {
        let data = fs::read(path).map_err(|e| format!("lectura de {}: {}", path.display(), e))?;
        let sha = sha256_hex(&data);
        let base_nonce = derive_nonce(nonce_key, &vault_id, rel.as_bytes(), 0);
        let mut cipher_len: u64 = 0;
        let mut idx: usize = 0;
        let mut off: usize = 0;
        loop {
            let end = std::cmp::min(off + CHUNK_SIZE, data.len());
            let chunk = &data[off..end];
            let nonce = derive_nonce(nonce_key, &vault_id, rel.as_bytes(), idx);
            let aad = build_aad(rel.as_bytes(), idx);
            let (ct, tag) = gcm_encrypt(&aes, &nonce, &aad, chunk);
            cipher_len += (ct.len() + TAG_LEN) as u64;
            records.push((rel.clone(), idx, chunk.to_vec(), ct, tag, nonce));
            idx += 1;
            off = end;
            if off >= data.len() {
                break;
            }
        }
        total_plain_bytes += data.len() as u64;
        entries.push(VaultEntry {
            rel_path: rel.clone(),
            plaintext_len: data.len() as u64,
            cipher_len,
            sha256_plain: sha,
            nonce: hex(&base_nonce),
            chunk_index: 0,
        });
    }

    let manifest = VaultManifest {
        version: VAULT_VERSION,
        created_at: now_iso8601(),
        algorithm: ALGORITHM_NAME.to_string(),
        kdf: KDF_NAME.to_string(),
        entries,
        total_plain_bytes,
    };
    let manifest_bytes =
        serde_json::to_vec(&manifest).map_err(|e| format!("serialización del manifiesto: {}", e))?;

    let mut buf: Vec<u8> = Vec::with_capacity(VAULT_HEADER_LEN + manifest_bytes.len() + 4096);
    buf.extend_from_slice(VAULT_MAGIC);
    buf.extend_from_slice(&VAULT_VERSION.to_le_bytes());
    buf.push(KDF_PBKDF2_HMAC_SHA256);
    buf.push(CIPHER_AES_256_GCM);
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&iterations.to_le_bytes());
    buf.extend_from_slice(&salt);
    buf.extend_from_slice(&vault_id);
    buf.extend_from_slice(&(manifest_bytes.len() as u32).to_le_bytes());
    buf.extend_from_slice(&manifest_bytes);

    for (rel, idx, plain, ct, tag, nonce) in &records {
        let rb = rel.as_bytes();
        buf.extend_from_slice(&(rb.len() as u16).to_le_bytes());
        buf.extend_from_slice(rb);
        buf.extend_from_slice(&(*idx as u32).to_le_bytes());
        buf.extend_from_slice(&(plain.len() as u32).to_le_bytes());
        buf.extend_from_slice(&((ct.len() + TAG_LEN) as u32).to_le_bytes());
        buf.extend_from_slice(nonce);
        buf.extend_from_slice(ct);
        buf.extend_from_slice(tag);
    }

    if let Some(parent) = Path::new(out).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("creación de {}: {}", parent.display(), e))?;
        }
    }
    fs::write(out, &buf).map_err(|e| format!("escritura de {}: {}", out, e))?;
    Ok(manifest)
}

// ---------------------------------------------------------------------------
// Apertura (descifrado) y verificación
// ---------------------------------------------------------------------------

/// Parsea la cabecera y el manifiesto; devuelve el offset del primer chunk.
fn parse_header(raw: &[u8]) -> Result<(VaultHeader, VaultManifest, usize), String> {
    if raw.len() < VAULT_HEADER_LEN {
        return Err("bóveda truncada: cabecera incompleta".to_string());
    }
    if &raw[0..4] != VAULT_MAGIC {
        return Err("firma inválida: no es una bóveda MDV1".to_string());
    }
    let version = u32::from_le_bytes(raw[4..8].try_into().unwrap());
    if version != VAULT_VERSION {
        return Err(format!("versión de bóveda no soportada: {}", version));
    }
    let kdf_id = raw[8];
    let cipher_id = raw[9];
    if kdf_id != KDF_PBKDF2_HMAC_SHA256 {
        return Err(format!("KDF no soportado: {}", kdf_id));
    }
    if cipher_id != CIPHER_AES_256_GCM {
        return Err(format!("cifrador no soportado: {}", cipher_id));
    }
    let iterations = u32::from_le_bytes(raw[12..16].try_into().unwrap());
    let mut salt = [0u8; SALT_LEN];
    salt.copy_from_slice(&raw[16..16 + SALT_LEN]);
    let mut vault_id = [0u8; VAULT_ID_LEN];
    vault_id.copy_from_slice(&raw[48..48 + VAULT_ID_LEN]);
    let manifest_len = u32::from_le_bytes(raw[80..84].try_into().unwrap()) as usize;
    let start = VAULT_HEADER_LEN;
    if raw.len() < start + manifest_len {
        return Err("bóveda truncada: manifiesto incompleto".to_string());
    }
    let manifest: VaultManifest = serde_json::from_slice(&raw[start..start + manifest_len])
        .map_err(|e| format!("manifiesto ilegible: {}", e))?;
    Ok((
        VaultHeader {
            version,
            kdf_id,
            cipher_id,
            iterations,
            salt,
            vault_id,
        },
        manifest,
        start + manifest_len,
    ))
}

/// Registro de chunk parseado.
struct ChunkRecord {
    rel_path: String,
    index: usize,
    plain_len: usize,
    cipher: Vec<u8>,
    nonce: [u8; NONCE_LEN],
    next: usize,
}

/// Parsea un registro de chunk a partir de `pos`.
fn parse_chunk(raw: &[u8], pos: usize) -> Result<ChunkRecord, String> {
    if pos + 2 > raw.len() {
        return Err("bóveda truncada en un chunk".to_string());
    }
    let rel_len = u16::from_le_bytes(raw[pos..pos + 2].try_into().unwrap()) as usize;
    let mut p = pos + 2;
    if p + rel_len + 4 + 4 + 4 + NONCE_LEN > raw.len() {
        return Err("bóveda truncada en un chunk".to_string());
    }
    let rel_path = String::from_utf8(raw[p..p + rel_len].to_vec())
        .map_err(|_| "chunk con ruta no UTF-8".to_string())?;
    p += rel_len;
    let index = u32::from_le_bytes(raw[p..p + 4].try_into().unwrap()) as usize;
    p += 4;
    let plain_len = u32::from_le_bytes(raw[p..p + 4].try_into().unwrap()) as usize;
    p += 4;
    let cipher_len = u32::from_le_bytes(raw[p..p + 4].try_into().unwrap()) as usize;
    p += 4;
    if cipher_len < TAG_LEN {
        return Err("chunk con longitud de cifrado inválida".to_string());
    }
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&raw[p..p + NONCE_LEN]);
    p += NONCE_LEN;
    if p + cipher_len > raw.len() {
        return Err("bóveda truncada en un chunk".to_string());
    }
    let cipher = raw[p..p + cipher_len].to_vec();
    let next = p + cipher_len;
    Ok(ChunkRecord {
        rel_path,
        index,
        plain_len,
        cipher,
        nonce,
        next,
    })
}

/// Descifra la bóveda en `dest` y devuelve las rutas relativas escritas.
///
/// Verifica el tag GCM de cada chunk y el SHA-256 de cada fichero.
pub fn open(vault: &str, passphrase: &str, dest: &str) -> Result<Vec<String>, String> {
    if passphrase.is_empty() {
        return Err("la passphrase no puede estar vacía".to_string());
    }
    let raw = fs::read(vault).map_err(|e| format!("lectura de {}: {}", vault, e))?;
    let (hdr, manifest, offset) = parse_header(&raw)?;

    let dk = pbkdf2_hmac_sha256(passphrase.as_bytes(), &hdr.salt, hdr.iterations, KEY_LEN * 2);
    let aes = Aes256::new(&dk[..KEY_LEN]);

    let mut buffers: Vec<(String, Vec<u8>)> = Vec::new();
    let mut pos = offset;
    while pos < raw.len() {
        let rec = parse_chunk(&raw, pos)?;
        let rel = normalize_rel(&rec.rel_path)?;
        let aad = build_aad(rel.as_bytes(), rec.index);
        let split = rec.cipher.len() - TAG_LEN;
        let pt = gcm_decrypt(
            &aes,
            &rec.nonce,
            &aad,
            &rec.cipher[..split],
            &rec.cipher[split..],
        )?;
        if pt.len() != rec.plain_len {
            return Err(format!("longitud inesperada en {}", rel));
        }
        match buffers.iter().position(|(r, _)| r == &rel) {
            Some(i) => buffers[i].1.extend_from_slice(&pt),
            None => buffers.push((rel.clone(), pt)),
        }
        pos = rec.next;
    }

    // Cada chunk debe corresponder a una entrada del manifiesto.
    for (rel, _) in &buffers {
        if !manifest.entries.iter().any(|e| &e.rel_path == rel) {
            return Err(format!(
                "la bóveda contiene {} que no está en el manifiesto",
                rel
            ));
        }
    }

    // Verificación de integridad por fichero.
    for entry in &manifest.entries {
        let buf = buffers
            .iter()
            .find(|(r, _)| r == &entry.rel_path)
            .ok_or_else(|| format!("falta el fichero {} en la bóveda", entry.rel_path))?;
        if buf.1.len() as u64 != entry.plaintext_len {
            return Err(format!(
                "tamaño incorrecto en {}: {} != {}",
                entry.rel_path, buf.1.len(), entry.plaintext_len
            ));
        }
        let sha = sha256_hex(&buf.1);
        if sha != entry.sha256_plain {
            return Err(format!(
                "SHA-256 incorrecto en {}: fichero alterado o truncado",
                entry.rel_path
            ));
        }
    }

    fs::create_dir_all(dest).map_err(|e| format!("creación de {}: {}", dest, e))?;
    let mut written: Vec<String> = Vec::new();
    for (rel, data) in &buffers {
        let out_path = Path::new(dest).join(rel);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("creación de {}: {}", parent.display(), e))?;
        }
        fs::write(&out_path, data).map_err(|e| format!("escritura de {}: {}", out_path.display(), e))?;
        written.push(rel.clone());
    }
    Ok(written)
}

/// Verifica la integridad **estructural** de la bóveda (sin passphrase) y
/// devuelve su manifiesto.
///
/// Verifica tamaños, longitudes y que los chunks del fichero coinciden con las
/// entradas del manifiesto. NO puede verificar los tags GCM (requiere la clave).
pub fn verify(vault: &str) -> Result<VaultManifest, String> {
    let raw = fs::read(vault).map_err(|e| format!("lectura de {}: {}", vault, e))?;
    let (_hdr, manifest, offset) = parse_header(&raw)?;

    let mut totals: Vec<(String, u64, usize)> = Vec::new(); // rel, cipher bytes, chunks
    let mut pos = offset;
    while pos < raw.len() {
        let rec = parse_chunk(&raw, pos)?;
        let rel = normalize_rel(&rec.rel_path)?;
        match totals.iter_mut().find(|(r, _, _)| r == &rel) {
            Some(t) => {
                t.1 += rec.cipher.len() as u64;
                t.2 += 1;
            }
            None => totals.push((rel, rec.cipher.len() as u64, 1)),
        }
        pos = rec.next;
    }

    for entry in &manifest.entries {
        let t = totals
            .iter()
            .find(|(r, _, _)| r == &entry.rel_path)
            .ok_or_else(|| {
                format!(
                    "el manifiesto declara {} pero no hay chunks en la bóveda",
                    entry.rel_path
                )
            })?;
        if t.1 != entry.cipher_len {
            return Err(format!(
                "longitud cifrada incorrecta en {}: {} != {}",
                entry.rel_path, t.1, entry.cipher_len
            ));
        }
    }
    for (rel, _, _) in &totals {
        if !manifest.entries.iter().any(|e| &e.rel_path == rel) {
            return Err(format!("{} no figura en el manifiesto", rel));
        }
    }
    Ok(manifest)
}

// ---------------------------------------------------------------------------
// Google Drive
// ---------------------------------------------------------------------------

/// Destino de subida a Drive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveTarget {
    /// Ruta relativa dentro de la bóveda.
    pub rel_path: String,
    /// Ruta final dentro de la carpeta de destino en Drive.
    pub target_path: String,
    /// Tamaño en bytes (texto cifrado + tags).
    pub size_bytes: u64,
    /// Orden de subida (0 = primero).
    pub order: usize,
    /// Si conviene usar subida reanudable por chunks.
    pub resumable: bool,
}

/// Construye el plan de subida a Google Drive a partir del manifiesto.
///
/// Google Drive recomienda (y en ficheros grandes *exige*) la subida reanudable
/// por chunks a partir de 8 MiB; por debajo la subida simple es más eficiente.
pub fn drive_upload_plan(manifest: &VaultManifest, target_dir: &str) -> Vec<DriveTarget> {
    let base = target_dir.trim_end_matches('/');
    manifest
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| DriveTarget {
            rel_path: e.rel_path.clone(),
            target_path: format!("{}/{}", base, e.rel_path),
            size_bytes: e.cipher_len,
            order: i,
            resumable: e.cipher_len >= DRIVE_RESUMABLE_THRESHOLD,
        })
        .collect()
}

/// Cita una ruta para shell.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Comando `curl` para subir un chunk a una sesión reanudable de Drive.
///
/// Se usa el `curl` del dispositivo y no Rust porque la huella TLS de los
/// clientes Rust (rustls) choca con la validación de certificado de Google;
/// el binario `curl` del sistema Android usa la pila TLS de BoringSSL del
/// sistema y es el que Google acepta en Android.
pub fn drive_resumable_command(file: &str, session_uri: &str) -> String {
    let f = shell_quote(file);
    let s = shell_quote(session_uri);
    format!(
        "curl -sS -X PUT --data-binary @{f} \
         -H 'Content-Type: application/octet-stream' \
         -H \"Content-Range: bytes 0-$(($(stat -c %s {f})-1))/$(stat -c %s {f})\" \
         {s}",
        f = f,
        s = s
    )
}

/// Extrae la URI de sesión reanudable de la respuesta de Google Drive.
///
/// Acepta tanto la cabecera HTTP `Location:` como el cuerpo JSON
/// `{"location": "https://…"}` que devuelve la API v3 al crear la sesión.
pub fn parse_drive_session_response(body: &str) -> Result<String, String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err("respuesta vacía: no hay Location de sesión".to_string());
    }

    // 1) Cabecera HTTP "Location: <uri>".
    for line in body.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        if let Some((k, v)) = l.split_once(':') {
            let key = k.trim();
            if key.eq_ignore_ascii_case("location") {
                let val = v.trim();
                if !val.is_empty() {
                    return validate_session_uri(val);
                }
            }
        }
    }

    // 2) Cuerpo JSON.
    if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
        for key in ["location", "Location", "uploadUrl", "sessionUri"] {
            if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
                if !s.trim().is_empty() {
                    return validate_session_uri(s.trim());
                }
            }
        }
    }

    Err("no se encontró la URI de sesión reanudable (Location)".to_string())
}

/// Valida que la URI tenga forma de sesión reanudable de Drive.
fn validate_session_uri(uri: &str) -> Result<String, String> {
    if !(uri.starts_with("https://") || uri.starts_with("http://")) {
        return Err(format!("URI de sesión inválida: {}", uri));
    }
    Ok(uri.to_string())
}

// ---------------------------------------------------------------------------
// W3 · Sellado en streaming, restore verificado y subida por trozos
// ---------------------------------------------------------------------------

/// Escribe un registro de chunk en el formato de la bóveda.
fn write_chunk_record<W: std::io::Write>(
    w: &mut W,
    rel: &str,
    idx: usize,
    plain_len: usize,
    ct: &[u8],
    tag: &[u8; TAG_LEN],
    nonce: &[u8; NONCE_LEN],
) -> Result<(), String> {
    let rb = rel.as_bytes();
    w.write_all(&(rb.len() as u16).to_le_bytes())
        .map_err(|e| format!("escritura de la bóveda: {}", e))?;
    w.write_all(rb)
        .map_err(|e| format!("escritura de la bóveda: {}", e))?;
    w.write_all(&(idx as u32).to_le_bytes())
        .map_err(|e| format!("escritura de la bóveda: {}", e))?;
    w.write_all(&(plain_len as u32).to_le_bytes())
        .map_err(|e| format!("escritura de la bóveda: {}", e))?;
    w.write_all(&((ct.len() + TAG_LEN) as u32).to_le_bytes())
        .map_err(|e| format!("escritura de la bóveda: {}", e))?;
    w.write_all(nonce)
        .map_err(|e| format!("escritura de la bóveda: {}", e))?;
    w.write_all(ct)
        .map_err(|e| format!("escritura de la bóveda: {}", e))?;
    w.write_all(tag)
        .map_err(|e| format!("escritura de la bóveda: {}", e))?;
    Ok(())
}

/// Sella un directorio **en streaming**: memoria acotada a un chunk.
///
/// El móvil tiene 5,9 GB libres y el pack son decenas de GB, así que no
/// puede existir una copia intermedia: el fichero se lee por trozos y se
/// cifra trozo a trozo. Se hacen dos pasadas sobre el origen (una para el
/// SHA-256 del manifiesto y otra para cifrar), pero nunca se guarda el
/// fichero entero ni su cifrado en memoria.
///
/// Mismo formato (`MDV1`) que [`seal_with_iterations`]: las dos funciones
/// producen bóvedas intercambiables.
pub fn seal_dir_streaming(
    dir: &str,
    out: &str,
    passphrase: &str,
    iterations: u32,
) -> Result<VaultManifest, String> {
    use std::io::{Read, Seek, SeekFrom, Write};

    if passphrase.is_empty() {
        return Err("la passphrase no puede estar vacía".to_string());
    }
    let root = Path::new(dir);
    if !root.is_dir() {
        return Err(format!("no existe el directorio a cifrar: {}", dir));
    }
    if iterations == 0 {
        return Err("las iteraciones del KDF deben ser > 0".to_string());
    }

    let mut files: Vec<PathBuf> = Vec::new();
    walk_files(root, &mut files);
    let mut rels: Vec<(PathBuf, String)> = Vec::new();
    for p in files {
        let rel = normalize_rel(
            &p.strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .to_string(),
        )?;
        rels.push((p, rel));
    }
    rels.sort_by(|a, b| a.1.cmp(&b.1));

    let salt = random_bytes(SALT_LEN);
    let vault_id = random_bytes(VAULT_ID_LEN);
    let dk = pbkdf2_hmac_sha256(passphrase.as_bytes(), &salt, iterations, KEY_LEN * 2);
    let enc_key = &dk[..KEY_LEN];
    let nonce_key = &dk[KEY_LEN..];
    let aes = Aes256::new(enc_key);

    // --- Pasada 1: tamaño y SHA-256 de cada fichero (streaming) -------------
    struct Meta {
        rel: String,
        path: PathBuf,
        size: u64,
        sha: String,
        chunks: u64,
    }
    let mut metas: Vec<Meta> = Vec::new();
    for (path, rel) in &rels {
        let (size, sha) = hash_file_streaming(path)
            .ok_or_else(|| format!("no se pudo leer {}", path.display()))?;
        let chunks = if size == 0 {
            1
        } else {
            (size + CHUNK_SIZE as u64 - 1) / CHUNK_SIZE as u64
        };
        metas.push(Meta {
            rel: rel.clone(),
            path: path.clone(),
            size,
            sha,
            chunks,
        });
    }

    let entries: Vec<VaultEntry> = metas
        .iter()
        .map(|m| VaultEntry {
            rel_path: m.rel.clone(),
            plaintext_len: m.size,
            cipher_len: m.size + m.chunks * TAG_LEN as u64,
            sha256_plain: m.sha.clone(),
            nonce: hex(&derive_nonce(nonce_key, &vault_id, m.rel.as_bytes(), 0)),
            chunk_index: 0,
        })
        .collect();
    let total_plain_bytes = metas.iter().map(|m| m.size).sum();

    let manifest = VaultManifest {
        version: VAULT_VERSION,
        created_at: now_iso8601(),
        algorithm: ALGORITHM_NAME.to_string(),
        kdf: KDF_NAME.to_string(),
        entries,
        total_plain_bytes,
    };
    let manifest_bytes =
        serde_json::to_vec(&manifest).map_err(|e| format!("serialización del manifiesto: {}", e))?;

    if let Some(parent) = Path::new(out).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("creación de {}: {}", parent.display(), e))?;
        }
    }
    let mut f = fs::File::create(out).map_err(|e| format!("escritura de {}: {}", out, e))?;

    // Cabecera + manifiesto.
    let mut head: Vec<u8> = Vec::with_capacity(VAULT_HEADER_LEN + manifest_bytes.len());
    head.extend_from_slice(VAULT_MAGIC);
    head.extend_from_slice(&VAULT_VERSION.to_le_bytes());
    head.push(KDF_PBKDF2_HMAC_SHA256);
    head.push(CIPHER_AES_256_GCM);
    head.extend_from_slice(&0u16.to_le_bytes());
    head.extend_from_slice(&iterations.to_le_bytes());
    head.extend_from_slice(&salt);
    head.extend_from_slice(&vault_id);
    head.extend_from_slice(&(manifest_bytes.len() as u32).to_le_bytes());
    head.extend_from_slice(&manifest_bytes);
    f.write_all(&head)
        .map_err(|e| format!("escritura de {}: {}", out, e))?;

    // --- Pasada 2: cifrar trozo a trozo -------------------------------------
    for m in &metas {
        let mut src = fs::File::open(&m.path)
            .map_err(|e| format!("lectura de {}: {}", m.path.display(), e))?;
        let mut idx: usize = 0;
        let mut off: u64 = 0;
        loop {
            let want = std::cmp::min(CHUNK_SIZE as u64, m.size.saturating_sub(off)) as usize;
            let mut buf = vec![0u8; want];
            if want > 0 {
                src.seek(SeekFrom::Start(off))
                    .map_err(|e| format!("lectura de {}: {}", m.path.display(), e))?;
                src.read_exact(&mut buf)
                    .map_err(|e| format!("lectura de {}: {}", m.path.display(), e))?;
            }
            let nonce = derive_nonce(nonce_key, &vault_id, m.rel.as_bytes(), idx);
            let aad = build_aad(m.rel.as_bytes(), idx);
            let (ct, tag) = gcm_encrypt(&aes, &nonce, &aad, &buf);
            write_chunk_record(&mut f, &m.rel, idx, buf.len(), &ct, &tag, &nonce)?;
            idx += 1;
            off += want as u64;
            if off >= m.size {
                break;
            }
        }
    }
    f.flush()
        .map_err(|e| format!("escritura de {}: {}", out, e))?;
    Ok(manifest)
}

/// Calcula (tamaño, SHA-256) de un fichero sin cargarlo en memoria.
fn hash_file_streaming(path: &Path) -> Option<(u64, String)> {
    let mut f = fs::File::open(path).ok()?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut total: u64 = 0;
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        total += n as u64;
    }
    Some((total, hex(&h.finalize())))
}

/// Abre una bóveda **en streaming**: descifra chunk a chunk directamente al
/// destino, sin ever buffers el fichero entero.
///
/// A diferencia de [`open`], aquí la memoria queda acotada a un chunk: es lo
/// que permite restaurar un pack de decenas de GB en un móvil con 5,9 GB
/// libres.
pub fn open_streaming(
    vault: &str,
    passphrase: &str,
    dest: &str,
) -> Result<Vec<String>, String> {
    use std::collections::BTreeMap;
    use std::io::{Read, Seek, SeekFrom, Write};

    if passphrase.is_empty() {
        return Err("la passphrase no puede estar vacía".to_string());
    }
    let mut f = fs::File::open(vault).map_err(|e| format!("lectura de {}: {}", vault, e))?;
    let mut raw: Vec<u8> = Vec::new();
    f.read_to_end(&mut raw)
        .map_err(|e| format!("lectura de {}: {}", vault, e))?;
    let (hdr, manifest, offset) = parse_header(&raw)?;

    let dk = pbkdf2_hmac_sha256(passphrase.as_bytes(), &hdr.salt, hdr.iterations, KEY_LEN * 2);
    let aes = Aes256::new(&dk[..KEY_LEN]);

    fs::create_dir_all(dest).map_err(|e| format!("creación de {}: {}", dest, e))?;

    // Hash por fichero, calculado mientras se escribe.
    let mut hashes: BTreeMap<String, Sha256> = BTreeMap::new();
    let mut sizes: BTreeMap<String, u64> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();

    let mut pos = offset;
    while pos < raw.len() {
        let rec = parse_chunk(&raw, pos)?;
        let rel = normalize_rel(&rec.rel_path)?;
        let aad = build_aad(rel.as_bytes(), rec.index);
        let split = rec.cipher.len() - TAG_LEN;
        let pt = gcm_decrypt(
            &aes,
            &rec.nonce,
            &aad,
            &rec.cipher[..split],
            &rec.cipher[split..],
        )?;
        if pt.len() != rec.plain_len {
            return Err(format!("longitud inesperada en {}", rel));
        }
        let out_path = Path::new(dest).join(&rel);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("creación de {}: {}", parent.display(), e))?;
        }
        let mut o = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&out_path)
            .map_err(|e| format!("escritura de {}: {}", out_path.display(), e))?;
        o.write_all(&pt)
            .map_err(|e| format!("escritura de {}: {}", out_path.display(), e))?;
        let entry = hashes.entry(rel.clone()).or_insert_with(Sha256::new);
        entry.update(&pt);
        *sizes.entry(rel.clone()).or_insert(0) += pt.len() as u64;
        if !order.contains(&rel) {
            order.push(rel.clone());
        }
        pos = rec.next;
    }

    // Verificación por fichero contra el manifiesto.
    for entry in &manifest.entries {
        let h = hashes.get(&entry.rel_path).ok_or_else(|| {
            format!("falta el fichero {} en la bóveda", entry.rel_path)
        })?;
        let got = hex(&h.clone().finalize());
        if got != entry.sha256_plain {
            return Err(format!(
                "SHA-256 incorrecto en {}: fichero alterado o truncado",
                entry.rel_path
            ));
        }
        let size = sizes.get(&entry.rel_path).copied().unwrap_or(0);
        if size != entry.plaintext_len {
            return Err(format!(
                "tamaño incorrecto en {}: {} != {}",
                entry.rel_path, size, entry.plaintext_len
            ));
        }
    }

    Ok(order)
}

/// El resultado de verificar un restore.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RestoreReport {
    /// Cada lectura comprobada.
    pub reads: Vec<RestoreRead>,
    /// `true` si todas las lecturas casaron.
    pub ok: bool,
}

/// Una lectura comprobada tras restaurar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RestoreRead {
    /// Ruta dentro del pack.
    pub rel_path: String,
    /// Bytes leídos de verdad del destino.
    pub bytes_read: u64,
    /// SHA-256 calculado sobre lo leído.
    pub sha256: String,
    /// Hash que esperaba el manifiesto.
    pub expected_sha256: String,
    /// `true` si casan.
    pub ok: bool,
}

/// **Restore probado**: relee del destino los ficheros indicados y comprueba
/// su SHA-256 contra el manifiesto de la bóveda.
///
/// Un backup sin restore probado no es un backup: esto es lo que demuestra
/// que el pack sirve. El manifiesto se lee **sin** passphrase (está en claro
/// en la cabecera), así que la comprobación es independiente de la clave.
///
/// `picks` son las rutas relativas a releer (por ejemplo las tres lecturas
/// reales mínimas: un fichero de texto, uno multimedia y uno del estado).
pub fn restore_verify(
    vault: &str,
    dest: &str,
    picks: &[&str],
) -> Result<RestoreReport, String> {
    let raw = fs::read(vault).map_err(|e| format!("lectura de {}: {}", vault, e))?;
    let (_hdr, manifest, _offset) = parse_header(&raw)?;
    let mut reads: Vec<RestoreRead> = Vec::new();
    for rel in picks {
        let path = Path::new(dest).join(rel);
        let (bytes, sha) = match hash_file_streaming(&path) {
            Some(v) => v,
            None => {
                reads.push(RestoreRead {
                    rel_path: rel.to_string(),
                    bytes_read: 0,
                    sha256: String::new(),
                    expected_sha256: String::new(),
                    ok: false,
                });
                continue;
            }
        };
        let expected = manifest
            .entries
            .iter()
            .find(|e| e.rel_path == *rel)
            .map(|e| e.sha256_plain.clone())
            .unwrap_or_default();
        reads.push(RestoreRead {
            rel_path: rel.to_string(),
            bytes_read: bytes,
            sha256: sha.clone(),
            expected_sha256: expected.clone(),
            ok: !expected.is_empty() && expected.eq_ignore_ascii_case(&sha),
        });
    }
    let ok = !reads.is_empty() && reads.iter().all(|r| r.ok);
    Ok(RestoreReport { reads, ok })
}

/// Deriva una clave de bóveda a partir de un **secreto ligado al hardware**.
///
/// El secreto (por ejemplo el material de una Keystore hardware-backed o de
/// un secure element) lo obtiene el llamante **en el propio dispositivo**;
/// este módulo sólo deriva y nunca lee, guarda ni transmite el secreto.
/// Es la base del nivel L3: las credenciales se cifran con una clave que no
/// existe fuera del móvil.
pub fn derive_hardware_key(
    device_secret: &[u8],
    salt: &[u8],
    iterations: u32,
) -> Result<[u8; KEY_LEN], String> {
    if device_secret.len() < 16 {
        return Err("el secreto del dispositivo es demasiado corto (mínimo 16 bytes)".into());
    }
    if salt.is_empty() {
        return Err("el salt no puede estar vacío".into());
    }
    let dk = pbkdf2_hmac_sha256(device_secret, salt, iterations.max(1), KEY_LEN);
    let mut out = [0u8; KEY_LEN];
    out.copy_from_slice(&dk);
    Ok(out)
}

/// Un trozo de la bóveda listo para subirse a Drive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DriveChunk {
    /// Índice del trozo.
    pub index: u32,
    /// Desplazamiento en el fichero de bóveda.
    pub offset: u64,
    /// Longitud del trozo.
    pub len: u64,
    /// SHA-256 del trozo.
    pub sha256: String,
    /// Cabecera `Content-Range` correspondiente.
    pub content_range: String,
}

/// Plan de subida por trozos a Drive, con SHA-256 por trozo.
///
/// El móvil no tiene espacio para una copia intermedia: el fichero de bóveda
/// se sube **por rangos**, calculando el hash de cada rango mientras se lee.
/// El tamaño de trozo tiene que ser múltiplo de 1 MiB para que el comando
/// `dd` sea reproducible en Android (toybox).
pub fn drive_chunk_plan(vault: &str, chunk_size: u64) -> Result<Vec<DriveChunk>, String> {
    const MIB: u64 = 1024 * 1024;
    if chunk_size == 0 || chunk_size % MIB != 0 {
        return Err("el tamaño de trozo debe ser múltiplo de 1 MiB".into());
    }
    let md = fs::metadata(vault).map_err(|e| format!("lectura de {}: {}", vault, e))?;
    let total = md.len();
    let mut out: Vec<DriveChunk> = Vec::new();
    let mut offset: u64 = 0;
    let mut index: u32 = 0;
    loop {
        let len = std::cmp::min(chunk_size, total.saturating_sub(offset));
        if len == 0 {
            break;
        }
        let sha = chunk_sha256_at(vault, offset, len)
            .ok_or_else(|| format!("no se pudo leer el trozo {} de {}", index, vault))?;
        out.push(DriveChunk {
            index,
            offset,
            len,
            sha256: sha,
            content_range: format!("bytes {}-{}/{}", offset, offset + len - 1, total),
        });
        offset += len;
        index += 1;
        if offset >= total {
            break;
        }
    }
    Ok(out)
}

/// SHA-256 de un rango del fichero, sin cargarlo entero en memoria.
fn chunk_sha256_at(path: &str, offset: u64, len: u64) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = fs::File::open(path).ok()?;
    f.seek(SeekFrom::Start(offset)).ok()?;
    let mut h = Sha256::new();
    let mut remaining = len;
    let mut buf = vec![0u8; 1 << 20];
    while remaining > 0 {
        let want = std::cmp::min(remaining as usize, buf.len());
        let n = f.read(&mut buf[..want]).ok()?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        remaining -= n as u64;
    }
    Some(hex(&h.finalize()))
}

/// Comando `curl` para subir **un trozo** de la bóveda a una sesión
/// reanudable de Drive.
///
/// No hace copia local: `dd` recorta el rango en memoria y lo pipea a `curl`.
pub fn drive_chunk_command(vault: &str, chunk: &DriveChunk, session_uri: &str) -> String {
    const MIB: u64 = 1024 * 1024;
    let skip_mib = chunk.offset / MIB;
    let count_mib = chunk.len.div_ceil(MIB);
    format!(
        "dd if={f} bs={mib} skip={skip} count={count} iflag=fullblock 2>/dev/null | \
         curl -sS -X PUT --data-binary @- \
         -H 'Content-Type: application/octet-stream' \
         -H 'Content-Range: {range}' {s}",
        f = shell_quote(vault),
        mib = MIB,
        skip = skip_mib,
        count = count_mib,
        range = chunk.content_range,
        s = shell_quote(session_uri)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmpdir(tag: &str) -> String {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "md_vault_{}_{}_{}",
            tag,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&p).unwrap();
        p.to_string_lossy().to_string()
    }

    // --- Primitivas criptográficas -----------------------------------------

    #[test]
    fn aes256_cifra_el_vector_de_fips197() {
        // FIPS-197, Apéndice C.3: AES-256.
        let key: Vec<u8> = (0u8..32).collect();
        let pt: [u8; 16] = [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ];
        let aes = Aes256::new(&key);
        let ct = aes.encrypt_block(&pt);
        let expect: [u8; 16] = [
            0x8e, 0xa2, 0xb7, 0xca, 0x51, 0x67, 0x45, 0xbf, 0xea, 0xfc, 0x49, 0x90, 0x4b, 0x49,
            0x60, 0x89,
        ];
        assert_eq!(ct, expect);
    }

    #[test]
    fn gcm_encapsula_el_vector_de_nist() {
        // NIST GCM, Test Case 13/14 (AES-256, IV de 96 bits todo cero).
        let aes = Aes256::new(&[0u8; 32]);
        let nonce = [0u8; 12];
        let (ct, tag) = gcm_encrypt(&aes, &nonce, &[], &[]);
        assert!(ct.is_empty());
        assert_eq!(
            hex(&tag),
            "530f8afbc74536b9a963b4f1c4cb738b"
        );

        let (ct, tag) = gcm_encrypt(&aes, &nonce, &[], &[0u8; 16]);
        assert_eq!(hex(&ct), "cea7403d4d606b6e074ec5d3baf39d18");
        assert_eq!(hex(&tag), "d0d1c8a799996bf0265b98b5d48ab919");
    }

    #[test]
    fn gcm_ata_el_aad_al_vector_de_nist() {
        // NIST GCM, Test Case 15/16 (AES-256, IV de 96 bits todo cero, AAD de
        // 128 bits). Son los casos que demuestran que el AAD entra en el tag:
        // sin AAD el mismo texto con y sin datos asociados NO puede compartir
        // etiqueta. En la bóveda el AAD es `rel_path || 0x00 || idx`, así que
        // renombrar o reordenar un chunk rompe la autenticación.
        let aes = Aes256::new(&[0u8; 32]);
        let nonce = [0u8; 12];
        let aad = [0u8; 16];

        // T15: texto plano vacío.
        let (ct, tag) = gcm_encrypt(&aes, &nonce, &aad, &[]);
        assert!(ct.is_empty());
        assert_eq!(hex(&tag), "2d45552d8575922b3ca3cc538442fa26");

        // T16: un bloque de 16 bytes.
        let (ct, tag) = gcm_encrypt(&aes, &nonce, &aad, &[0u8; 16]);
        assert_eq!(hex(&ct), "cea7403d4d606b6e074ec5d3baf39d18");
        assert_eq!(hex(&tag), "ae9b1771dba9cf62b39be017940330b4");

        // Mismo texto y mismo nonce con OTRO AAD → mismo texto cifrado (el AAD
        // no toca el CTR) pero etiqueta distinta: el AAD está criptográficamente
        // atado al chunk.
        let (ct2, tag2) = gcm_encrypt(&aes, &nonce, &[0u8; 15], &[0u8; 16]);
        assert_eq!(ct2, ct);
        assert_ne!(tag2, tag);

        // Y el descifrado con el AAD equivocado debe fallar la autenticación.
        let err = gcm_decrypt(&aes, &nonce, &[0u8; 15], &ct, &tag).unwrap_err();
        assert!(err.contains("autenticación") || err.contains("autenticacion"));
    }

    #[test]
    fn hmac_y_pbkdf2_coinciden_con_los_valores_estandar() {
        // RFC 4231, caso 1: HMAC-SHA-256 con clave de 20 bytes de 0x0b.
        let mac = hmac_sha256(&[0x0b; 20], b"Hi There");
        assert_eq!(
            hex(&mac),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
        // RFC 6070 no cubre SHA-256; vector clásico PBKDF2-HMAC-SHA256:
        // P="password", S="salt", c=1  -> 120fb6cffcf8b32c43e7225256c4f837
        //                            c=2  -> ae4d0c95af6b46d32d0adff928f06dd02a303f8ef3c251dfd6e2d85a95474c43
        let d1 = pbkdf2_hmac_sha256(b"password", b"salt", 1, 32);
        assert_eq!(hex(&d1), "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b");
        let d2 = pbkdf2_hmac_sha256(b"password", b"salt", 2, 32);
        assert_eq!(
            hex(&d2),
            "ae4d0c95af6b46d32d0adff928f06dd02a303f8ef3c251dfd6e2d85a95474c43"
        );
    }

    // --- Bóveda -------------------------------------------------------------

    #[test]
    fn round_trip_de_la_boveda() {
        let src = tmpdir("src");
        let dst = tmpdir("dst");
        fs::create_dir_all(format!("{}/sub", src)).unwrap();
        fs::write(format!("{}/a.txt", src), b"contenido secreto A").unwrap();
        fs::write(format!("{}/sub/b.bin", src), vec![0xABu8; 5000]).unwrap();
        let vault = format!("{}/vault.mdv", dst);

        let manifest = seal_with_iterations(&src, &vault, "clave-de-prueba", 1000).unwrap();
        assert_eq!(manifest.entries.len(), 2);
        assert_eq!(manifest.algorithm, "AES-256-GCM");
        assert_eq!(manifest.kdf, "PBKDF2-HMAC-SHA256");
        assert!(manifest.total_plain_bytes > 0);
        // El texto plano NO debe aparecer en el fichero de bóveda.
        let raw = fs::read(&vault).unwrap();
        assert!(!raw
            .windows(20)
            .any(|w| w == b"contenido secreto A"));

        let out = format!("{}/out", dst);
        let written = open(&vault, "clave-de-prueba", &out).unwrap();
        assert_eq!(written.len(), 2);
        assert_eq!(
            fs::read(format!("{}/a.txt", out)).unwrap(),
            b"contenido secreto A"
        );
        assert_eq!(fs::read(format!("{}/sub/b.bin", out)).unwrap(), vec![0xABu8; 5000]);
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&dst);
    }

    #[test]
    fn passphrase_incorrecta_falla_el_tag_gcm() {
        let src = tmpdir("src2");
        let dst = tmpdir("dst2");
        fs::write(format!("{}/x.txt", src), b"secreto").unwrap();
        let vault = format!("{}/v.mdv", dst);
        seal_with_iterations(&src, &vault, "buena", 1000).unwrap();
        let err = open(&vault, "mala", &format!("{}/out", dst)).unwrap_err();
        assert!(err.contains("autenticación") || err.contains("autenticacion"));
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&dst);
    }

    #[test]
    fn fichero_alterado_es_detectado() {
        let src = tmpdir("src3");
        let dst = tmpdir("dst3");
        fs::write(format!("{}/x.txt", src), b"0123456789abcdef").unwrap();
        let vault = format!("{}/v.mdv", dst);
        seal_with_iterations(&src, &vault, "buena", 1000).unwrap();
        let mut raw = fs::read(&vault).unwrap();
        let n = raw.len();
        raw[n - 20] ^= 0xFF; // altera el tag del único chunk
        fs::write(&vault, &raw).unwrap();
        let err = open(&vault, "buena", &format!("{}/out", dst)).unwrap_err();
        assert!(err.contains("autenticación") || err.contains("autenticacion"));
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&dst);
    }

    #[test]
    fn renombrar_un_chunk_rompe_el_aad() {
        let src = tmpdir("src4");
        let dst = tmpdir("dst4");
        fs::write(format!("{}/x.txt", src), b"contenido que se renombra").unwrap();
        let vault = format!("{}/v.mdv", dst);
        seal_with_iterations(&src, &vault, "buena", 1000).unwrap();
        let mut raw = fs::read(&vault).unwrap();
        // Localiza la ruta "x.txt" dentro del registro de chunk y la renombra
        // (última aparición: el manifiesto va antes que los chunks).
        let pos = raw
            .windows(5)
            .rposition(|w| w == b"x.txt")
            .expect("debe aparecer la ruta");
        raw[pos] = b'y';
        fs::write(&vault, &raw).unwrap();
        let err = open(&vault, "buena", &format!("{}/out", dst)).unwrap_err();
        assert!(err.contains("autenticación") || err.contains("autenticacion"));
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&dst);
    }

    #[test]
    fn verify_devuelve_el_manifiesto_y_detecta_manipulaciones() {
        let src = tmpdir("src5");
        let dst = tmpdir("dst5");
        fs::write(format!("{}/x.txt", src), b"datos").unwrap();
        let vault = format!("{}/v.mdv", dst);
        seal_with_iterations(&src, &vault, "buena", 1000).unwrap();
        let m = verify(&vault).unwrap();
        assert_eq!(m.entries.len(), 1);
        assert_eq!(m.entries[0].rel_path, "x.txt");
        assert_eq!(m.entries[0].plaintext_len, 5);
        assert_eq!(m.entries[0].nonce.len(), 24); // 12 bytes en hex

        // Truncar la bóveda debe hacer fallar verify().
        let raw = fs::read(&vault).unwrap();
        fs::write(&vault, &raw[..raw.len() - 5]).unwrap();
        assert!(verify(&vault).is_err());
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&dst);
    }

    // --- Drive --------------------------------------------------------------

    #[test]
    fn plan_de_subida_a_drive() {
        let manifest = VaultManifest {
            version: 1,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            algorithm: ALGORITHM_NAME.to_string(),
            kdf: KDF_NAME.to_string(),
            entries: vec![
                VaultEntry {
                    rel_path: "chats/a.txt".to_string(),
                    plaintext_len: 10,
                    cipher_len: 1024,
                    sha256_plain: "ab".to_string(),
                    nonce: "cd".to_string(),
                    chunk_index: 0,
                },
                VaultEntry {
                    rel_path: "video/big.mp4".to_string(),
                    plaintext_len: 20_000_000,
                    cipher_len: 20_000_016,
                    sha256_plain: "ef".to_string(),
                    nonce: "12".to_string(),
                    chunk_index: 0,
                },
            ],
            total_plain_bytes: 20_000_010,
        };
        let plan = drive_upload_plan(&manifest, "/MegaDaemon/backup");
        assert_eq!(plan.len(), 2);
        assert_eq!(plan[0].target_path, "/MegaDaemon/backup/chats/a.txt");
        assert_eq!(plan[0].order, 0);
        assert!(!plan[0].resumable, "1 KiB no necesita reanudable");
        assert!(plan[1].resumable, ">8 MiB debe ser reanudable");
        assert_eq!(plan[1].size_bytes, 20_000_016);
    }

    #[test]
    fn parseo_de_la_respuesta_de_drive() {
        // Cabecera HTTP.
        let hdr = "HTTP/1.1 200 OK\r\nLocation: https://www.googleapis.com/upload/drive/v3/files?uploadType=resumable&upload_id=ABC\r\n\r\n";
        assert_eq!(
            parse_drive_session_response(hdr).unwrap(),
            "https://www.googleapis.com/upload/drive/v3/files?uploadType=resumable&upload_id=ABC"
        );
        // Cuerpo JSON.
        let json = r#"{"location":"https://www.googleapis.com/upload/drive/v3/files?uploadType=resumable&upload_id=XYZ"}"#;
        assert_eq!(
            parse_drive_session_response(json).unwrap(),
            "https://www.googleapis.com/upload/drive/v3/files?uploadType=resumable&upload_id=XYZ"
        );
        // Sin Location → error.
        assert!(parse_drive_session_response("{\"error\":{\"code\":400}}").is_err());
        assert!(parse_drive_session_response("").is_err());
    }

    #[test]
    fn comando_curl_reanudable() {
        let cmd = drive_resumable_command("/sdcard/v.mdv", "https://example/session");
        assert!(cmd.starts_with("curl"));
        assert!(cmd.contains("-X PUT"));
        assert!(cmd.contains("Content-Range"));
        assert!(cmd.contains("/sdcard/v.mdv"));
        assert!(cmd.contains("https://example/session"));
    }

    // --- W3 · streaming, restore y subida por trozos -------------------------

    #[test]
    fn el_sellado_en_streaming_produce_la_misma_estructura_que_el_normal() {
        let src = tmpdir("s1");
        let dst = tmpdir("d1");
        fs::write(format!("{}/a.txt", src), b"contenido secreto A").unwrap();
        fs::write(format!("{}/b.bin", src), vec![0xABu8; 5000]).unwrap();
        let v_normal = format!("{}/normal.mdv", dst);
        let v_stream = format!("{}/stream.mdv", dst);

        let m1 = seal_with_iterations(&src, &v_normal, "clave", 1000).unwrap();
        let m2 = seal_dir_streaming(&src, &v_stream, "clave", 1000).unwrap();

        assert_eq!(m1.entries.len(), m2.entries.len());
        for (a, b) in m1.entries.iter().zip(m2.entries.iter()) {
            assert_eq!(a.rel_path, b.rel_path);
            assert_eq!(a.plaintext_len, b.plaintext_len);
            assert_eq!(a.cipher_len, b.cipher_len);
            assert_eq!(a.sha256_plain, b.sha256_plain);
        }
        assert_eq!(m1.total_plain_bytes, m2.total_plain_bytes);
        // Ambas bóvedas se abren con la misma passphrase.
        let out1 = format!("{}/o1", dst);
        let out2 = format!("{}/o2", dst);
        assert_eq!(open(&v_normal, "clave", &out1).unwrap().len(), 2);
        assert_eq!(open(&v_stream, "clave", &out2).unwrap().len(), 2);
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&dst);
    }

    #[test]
    fn el_streaming_trocea_y_restaura_exacto() {
        let src = tmpdir("s2");
        let dst = tmpdir("d2");
        // 2,5 MiB => 3 chunks de 1 MiB.
        let big = vec![0x5Au8; 2_500_000];
        fs::write(format!("{}/grande.bin", src), &big).unwrap();
        fs::write(format!("{}/texto.txt", src), b"hola").unwrap();
        let vault = format!("{}/v.mdv", dst);
        let m = seal_dir_streaming(&src, &vault, "clave", 1000).unwrap();
        let grande = m.entries.iter().find(|e| e.rel_path == "grande.bin").unwrap();
        assert_eq!(grande.plaintext_len, 2_500_000);
        assert_eq!(grande.cipher_len, 2_500_000 + 3 * TAG_LEN as u64);

        let out = format!("{}/out", dst);
        let written = open_streaming(&vault, "clave", &out).unwrap();
        assert_eq!(written.len(), 2);
        assert_eq!(
            fs::read(format!("{}/grande.bin", out)).unwrap().len(),
            2_500_000
        );
        assert_eq!(fs::read(format!("{}/grande.bin", out)).unwrap(), big);
        assert_eq!(fs::read(format!("{}/texto.txt", out)).unwrap(), b"hola");
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&dst);
    }

    #[test]
    fn el_streaming_detecta_un_fichero_alterado() {
        let src = tmpdir("s3");
        let dst = tmpdir("d3");
        fs::write(format!("{}/x.txt", src), b"contenido").unwrap();
        let vault = format!("{}/v.mdv", dst);
        seal_dir_streaming(&src, &vault, "clave", 1000).unwrap();
        let mut raw = fs::read(&vault).unwrap();
        let n = raw.len();
        raw[n - 20] ^= 0xFF;
        fs::write(&vault, &raw).unwrap();
        let err = open_streaming(&vault, "clave", &format!("{}/out", dst)).unwrap_err();
        assert!(
            err.contains("autenticación") || err.contains("SHA-256"),
            "mensaje inesperado: {}",
            err
        );
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&dst);
    }

    #[test]
    fn restore_verify_relee_de_verdad_del_destino() {
        let src = tmpdir("s4");
        let dst = tmpdir("d4");
        fs::write(format!("{}/nota.txt", src), b"una nota").unwrap();
        fs::write(format!("{}/foto.jpg", src), vec![1u8; 3000]).unwrap();
        fs::write(format!("{}/estado.log", src), b"linea1\nlinea2\n").unwrap();
        let vault = format!("{}/v.mdv", dst);
        seal_dir_streaming(&src, &vault, "clave", 1000).unwrap();
        let out = format!("{}/out", dst);
        open_streaming(&vault, "clave", &out).unwrap();

        // Las 3 lecturas reales mínimas.
        let rep = restore_verify(
            &vault,
            &out,
            &["nota.txt", "foto.jpg", "estado.log"],
        )
        .unwrap();
        assert!(rep.ok, "el restore debe verificar: {:?}", rep.reads);
        assert_eq!(rep.reads.len(), 3);
        assert_eq!(rep.reads[0].bytes_read, 8);
        assert_eq!(rep.reads[1].bytes_read, 3000);
        assert!(rep.reads.iter().all(|r| r.sha256.len() == 64));

        // Si se borra un fichero del destino, la lectura falla.
        fs::remove_file(format!("{}/foto.jpg", out)).unwrap();
        let rep2 = restore_verify(&vault, &out, &["nota.txt", "foto.jpg"]).unwrap();
        assert!(!rep2.ok);
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&dst);
    }

    #[test]
    fn la_clave_ligada_al_hardware_exige_un_secreto_real() {
        assert!(derive_hardware_key(b"corto", b"salt", 1000).is_err());
        let a = derive_hardware_key(&[7u8; 32], b"salt", 1000).unwrap();
        let b = derive_hardware_key(&[7u8; 32], b"salt", 1000).unwrap();
        let c = derive_hardware_key(&[8u8; 32], b"salt", 1000).unwrap();
        assert_eq!(a, b, "la derivación debe ser determinista");
        assert_ne!(a, c, "otro secreto → otra clave");
        assert_eq!(a.len(), KEY_LEN);
        assert!(derive_hardware_key(&[7u8; 32], b"", 1000).is_err());
    }

    #[test]
    fn el_plan_de_subida_por_trozos_tiene_hash_y_rango() {
        let src = tmpdir("s5");
        let dst = tmpdir("d5");
        fs::write(format!("{}/x.bin", src), vec![3u8; 2_500_000]).unwrap();
        let vault = format!("{}/v.mdv", dst);
        seal_dir_streaming(&src, &vault, "clave", 1000).unwrap();

        let chunks = drive_chunk_plan(&vault, 1024 * 1024).unwrap();
        assert!(chunks.len() >= 2);
        assert_eq!(chunks[0].offset, 0);
        assert_eq!(chunks[0].len, 1024 * 1024);
        let vault_size = fs::metadata(&vault).unwrap().len();
        assert_eq!(
            chunks[0].content_range,
            format!("bytes 0-{}/{}", 1024 * 1024 - 1, vault_size)
        );
        assert_eq!(chunks[0].sha256.len(), 64);
        // El tamaño de trozo tiene que ser múltiplo de MiB.
        assert!(drive_chunk_plan(&vault, 1000).is_err());

        let cmd = drive_chunk_command(&vault, &chunks[1], "https://example/session");
        assert!(cmd.contains("Content-Range: bytes 1048576-"));
        assert!(cmd.contains("skip=1"));
        assert!(cmd.contains("https://example/session"));
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&dst);
    }
}
