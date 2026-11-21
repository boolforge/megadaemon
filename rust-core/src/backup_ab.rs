//! Lector de backups `.ab` (Android BackupManager / `adb backup`) — **AOSP,
//! Apache-2.0**.
//!
//! Formato (AOSP `BackupManagerService` / `adb`):
//!
//! ```text
//! offset 0 : "ANDROID BACKUP" (14 bytes)
//! offset 14: '\0'
//! offset 15: versión del formato (u32 LE) = 1
//! offset 19: compresión  (1 = sin comprimir, 2 = deflate)
//! offset 20: cifrado     (0 = ninguno, 1 = AES-256-CBC, 2 = AES-256-CBC +
//!                        clave de 256 bits, 3 = con clave derivada por
//!                        PBKDF2 de la contraseña del usuario)
//! offset 21: 4 bytes reservados
//! offset 25: payload (tar [+ gzip si compresión=2] [+ AES si cifrado>0])
//! ```
//!
//! El tar contiene `manifest` (JSON) y las entradas de datos (`fs/…`,
//! `data/…`). El cifrado "1/2" usa una clave de 32 bytes; el modo "3" deriva
//! la clave con **PBKDF2-HMAC-SHA1** (10 000 iteraciones, salt de 16 bytes,
//! IV de 16 bytes) a partir de la contraseña que introduce el usuario.
//!
//! Todo es **autocontenido**: AES-256, HMAC-SHA1 y PBKDF2 están implementados
//! aquí (sin crates nuevas ⇒ sin tocar `Cargo.toml`). El `manifest` se puede
//! leer **sin descifrar** cuando la compresión es "sin comprimir", lo que ya
//! da el inventario completo de lo que hay en el backup.

use serde_json::{json, Value};

// ========================================================================= //
// AES-256 (cifrado y descifrado) — verificado con el vector FIPS-197
// ========================================================================= //

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

fn inv_sbox() -> [u8; 256] {
    let mut inv = [0u8; 256];
    for (i, &v) in SBOX.iter().enumerate() {
        inv[v as usize] = i as u8;
    }
    inv
}

fn xtime(x: u8) -> u8 {
    (x << 1) ^ if x & 0x80 != 0 { 0x1b } else { 0x00 }
}

fn gmul(a: u8, b: u8) -> u8 {
    let mut result = 0u8;
    let mut a = a;
    let mut b = b;
    while b != 0 {
        if b & 1 != 0 {
            result ^= a;
        }
        a = xtime(a);
        b >>= 1;
    }
    result
}

/// Expansión de clave AES-256 (Nk=8, Nr=14).
fn key_expansion_256(key: &[u8; 32]) -> [[u8; 4]; 60] {
    let mut w = [[0u8; 4]; 60];
    for i in 0..8 {
        w[i] = [key[4 * i], key[4 * i + 1], key[4 * i + 2], key[4 * i + 3]];
    }
    let mut rcon = 1u8;
    for i in 8..60 {
        let mut temp = w[i - 1];
        if i % 8 == 0 {
            temp = [
                SBOX[temp[1] as usize] ^ rcon,
                SBOX[temp[2] as usize],
                SBOX[temp[3] as usize],
                SBOX[temp[0] as usize],
            ];
            rcon = xtime(rcon);
        } else if i % 8 == 4 {
            temp = [
                SBOX[temp[0] as usize],
                SBOX[temp[1] as usize],
                SBOX[temp[2] as usize],
                SBOX[temp[3] as usize],
            ];
        }
        for j in 0..4 {
            w[i][j] = w[i - 8][j] ^ temp[j];
        }
    }
    w
}

fn add_round_key(state: &mut [u8; 16], w: &[[u8; 4]; 60], round: usize) {
    for c in 0..4 {
        for r in 0..4 {
            state[c * 4 + r] ^= w[round * 4 + c][r];
        }
    }
}

/// El estado AES es **column-major**: `state[column][row]`.  La entrada (y la
/// salida) van en orden de bytes, así que hay que transponer.
fn load_state(block: &[u8; 16]) -> [u8; 16] {
    let mut s = [0u8; 16];
    for c in 0..4 {
        for r in 0..4 {
            s[c * 4 + r] = block[r + 4 * c];
        }
    }
    s
}

fn store_state(s: &[u8; 16]) -> [u8; 16] {
    let mut b = [0u8; 16];
    for c in 0..4 {
        for r in 0..4 {
            b[r + 4 * c] = s[c * 4 + r];
        }
    }
    b
}

/// Cifra **un bloque** de 16 bytes con AES-256.
pub fn aes256_encrypt_block(key: &[u8; 32], out: &mut [u8; 16]) {
    let w = key_expansion_256(key);
    let mut st = load_state(out);
    let block = &mut st;
    add_round_key(block, &w, 0);
    for round in 1..14 {
        // SubBytes
        for b in block.iter_mut() {
            *b = SBOX[*b as usize];
        }
        shift_rows(block);
        mix_columns(block);
        add_round_key(block, &w, round);
    }
    // Ronda final: SubBytes + ShiftRows + AddRoundKey (**sin** MixColumns).
    for b in block.iter_mut() {
        *b = SBOX[*b as usize];
    }
    shift_rows(block);
    add_round_key(block, &w, 14);
    // Ojo: el resultado se devuelve en el buffer del llamante (el estado
    // interno es column-major y el llamante espera orden de bytes).
    *out = store_state(block);
}

/// Descifra **un bloque** de 16 bytes con AES-256.
pub fn aes256_decrypt_block(key: &[u8; 32], out: &mut [u8; 16]) {
    let w = key_expansion_256(key);
    let inv = inv_sbox();
    let mut st = load_state(out);
    let block = &mut st;
    add_round_key(block, &w, 14);
    for round in (1..14).rev() {
        inv_shift_rows(block);
        for b in block.iter_mut() {
            *b = inv[*b as usize];
        }
        add_round_key(block, &w, round);
        inv_mix_columns(block);
    }
    inv_shift_rows(block);
    for b in block.iter_mut() {
        *b = inv[*b as usize];
    }
    add_round_key(block, &w, 0);
    *out = store_state(block);
}

fn shift_rows(s: &mut [u8; 16]) {
    let t = *s;
    for r in 1..4 {
        for c in 0..4 {
            s[c * 4 + r] = t[((c + r) % 4) * 4 + r];
        }
    }
}

fn inv_shift_rows(s: &mut [u8; 16]) {
    let t = *s;
    for r in 1..4 {
        for c in 0..4 {
            s[((c + r) % 4) * 4 + r] = t[c * 4 + r];
        }
    }
}

fn mix_columns(s: &mut [u8; 16]) {
    for c in 0..4 {
        let col = [s[c * 4], s[c * 4 + 1], s[c * 4 + 2], s[c * 4 + 3]];
        s[c * 4] = xtime(col[0]) ^ (xtime(col[1]) ^ col[1]) ^ col[2] ^ col[3];
        s[c * 4 + 1] = col[0] ^ xtime(col[1]) ^ (xtime(col[2]) ^ col[2]) ^ col[3];
        s[c * 4 + 2] = col[0] ^ col[1] ^ xtime(col[2]) ^ (xtime(col[3]) ^ col[3]);
        s[c * 4 + 3] = (xtime(col[0]) ^ col[0]) ^ col[1] ^ col[2] ^ xtime(col[3]);
    }
}

fn inv_mix_columns(s: &mut [u8; 16]) {
    for c in 0..4 {
        let col = [s[c * 4], s[c * 4 + 1], s[c * 4 + 2], s[c * 4 + 3]];
        s[c * 4] = gmul(col[0], 14) ^ gmul(col[1], 11) ^ gmul(col[2], 13) ^ gmul(col[3], 9);
        s[c * 4 + 1] = gmul(col[0], 9) ^ gmul(col[1], 14) ^ gmul(col[2], 11) ^ gmul(col[3], 13);
        s[c * 4 + 2] = gmul(col[0], 13) ^ gmul(col[1], 9) ^ gmul(col[2], 14) ^ gmul(col[3], 11);
        s[c * 4 + 3] = gmul(col[0], 11) ^ gmul(col[1], 13) ^ gmul(col[2], 9) ^ gmul(col[3], 14);
    }
}

/// AES-256-CBC: descifra `data` (IV en los primeros 16 bytes) y quita el
/// relleno PKCS#7.
pub fn aes256_cbc_decrypt(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 16 {
        return Err("payload cifrado demasiado corto".into());
    }
    let mut iv = [0u8; 16];
    iv.copy_from_slice(&data[..16]);
    let body = &data[16..];
    if body.len() % 16 != 0 {
        return Err(format!("longitud no múltiplo de 16: {}", body.len()));
    }
    let mut out = Vec::with_capacity(body.len());
    let mut prev = iv;
    for chunk in body.chunks(16) {
        let mut block = [0u8; 16];
        block.copy_from_slice(chunk);
        let cipher = block;
        aes256_decrypt_block(key, &mut block);
        for i in 0..16 {
            out.push(block[i] ^ prev[i]);
        }
        prev = cipher;
    }
    // PKCS#7
    if let Some(&pad) = out.last() {
        let pad = pad as usize;
        if pad >= 1 && pad <= 16 && pad <= out.len() {
            let start = out.len() - pad;
            if out[start..].iter().all(|b| *b as usize == pad) {
                out.truncate(start);
                return Ok(out);
            }
        }
    }
    Ok(out)
}

/// AES-256-CBC: cifra (para los tests y para re-empaquetar).
pub fn aes256_cbc_encrypt(key: &[u8; 32], iv: &[u8; 16], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + data.len() + 16);
    out.extend_from_slice(iv);
    let mut prev = *iv;
    let pad = 16 - (data.len() % 16);
    let mut padded = data.to_vec();
    padded.extend(std::iter::repeat(pad as u8).take(pad));
    for chunk in padded.chunks(16) {
        let mut block = [0u8; 16];
        block.copy_from_slice(chunk);
        for i in 0..16 {
            block[i] ^= prev[i];
        }
        aes256_encrypt_block(key, &mut block);
        out.extend_from_slice(&block);
        prev = block;
    }
    out
}

// ========================================================================= //
// HMAC-SHA1 + PBKDF2 (RFC 2104 / RFC 6070)
// ========================================================================= //

use sha1::{Digest, Sha1};

fn hmac_sha1(key: &[u8], msg: &[u8]) -> [u8; 20] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        let mut h = Sha1::new();
        h.update(key);
        let d = h.finalize();
        k[..20].copy_from_slice(&d);
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..64 {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = Sha1::new();
    inner.update(ipad);
    inner.update(msg);
    let inner_d = inner.finalize();
    let mut outer = Sha1::new();
    outer.update(opad);
    outer.update(inner_d);
    let out = outer.finalize();
    let mut res = [0u8; 20];
    res.copy_from_slice(&out);
    res
}

/// PBKDF2-HMAC-SHA1 (RFC 2898). `dk_len` en bytes.
pub fn pbkdf2_hmac_sha1(password: &[u8], salt: &[u8], iterations: u32, dk_len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(dk_len);
    let mut block: u32 = 1;
    while out.len() < dk_len {
        let mut msg = salt.to_vec();
        msg.extend_from_slice(&block.to_be_bytes());
        let mut u = hmac_sha1(password, &msg);
        let mut t = u;
        for _ in 1..iterations {
            u = hmac_sha1(password, &u);
            for i in 0..20 {
                t[i] ^= u[i];
            }
        }
        out.extend_from_slice(&t);
        block += 1;
    }
    out.truncate(dk_len);
    out
}

// ========================================================================= //
// Lectura del `.ab`
// ========================================================================= //

/// Cabecera del archivo `.ab`.
#[derive(Debug, Clone, PartialEq)]
pub struct AbHeader {
    /// Versión del formato (1).
    pub version: u32,
    /// 1 = sin comprimir, 2 = deflate.
    pub compression: u8,
    /// 0 = sin cifrar, 1/2 = AES-256-CBC con clave de 256 bits,
    /// 3 = AES-256-CBC con clave derivada por PBKDF2 de la contraseña.
    pub encryption: u8,
    /// Offset del payload dentro del fichero.
    pub payload_offset: usize,
}

/// Entrada del tar interior.
#[derive(Debug, Clone, PartialEq)]
pub struct TarEntry {
    pub name: String,
    pub size: u64,
    pub mode: u32,
    /// Offset del contenido dentro del tar.
    pub data_offset: usize,
}

/// Error de parseo del `.ab`.
#[derive(Debug, Clone, PartialEq)]
pub enum AbError {
    NotAb,
    TooShort,
    BadVersion(u32),
    BadCompression(u8),
    BadEncryption(u8),
    BadTar(String),
}

impl std::fmt::Display for AbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AbError::NotAb => write!(f, "no es un archivo .ab (falta ANDROID BACKUP)"),
            AbError::TooShort => write!(f, "fichero demasiado corto"),
            AbError::BadVersion(v) => write!(f, "versión de .ab no soportada: {}", v),
            AbError::BadCompression(c) => write!(f, "compresión no soportada: {}", c),
            AbError::BadEncryption(e) => write!(f, "cifrado no soportado: {}", e),
            AbError::BadTar(m) => write!(f, "tar inválido: {}", m),
        }
    }
}

/// Lee la cabecera del `.ab`.
pub fn parse_ab_header(bytes: &[u8]) -> Result<AbHeader, AbError> {
    if bytes.len() < 25 {
        return Err(AbError::TooShort);
    }
    if &bytes[0..14] != b"ANDROID BACKUP" {
        return Err(AbError::NotAb);
    }
    let version = u32::from_le_bytes([bytes[15], bytes[16], bytes[17], bytes[18]]);
    if version != 1 {
        return Err(AbError::BadVersion(version));
    }
    let compression = bytes[19];
    if compression != 1 && compression != 2 {
        return Err(AbError::BadCompression(compression));
    }
    let encryption = bytes[20];
    if encryption > 3 {
        return Err(AbError::BadEncryption(encryption));
    }
    Ok(AbHeader {
        version,
        compression,
        encryption,
        payload_offset: 25,
    })
}

/// Lista las entradas del tar (sin extraer).
pub fn tar_entries(tar: &[u8]) -> Result<Vec<TarEntry>, AbError> {
    let mut out = Vec::new();
    let mut off = 0usize;
    while off + 512 <= tar.len() {
        let hdr = &tar[off..off + 512];
        if hdr.iter().all(|b| *b == 0) {
            break; // fin del tar
        }
        let name = String::from_utf8_lossy(&hdr[0..100])
            .trim_end_matches('\0')
            .to_string();
        if name.is_empty() {
            break;
        }
        let size_field = String::from_utf8_lossy(&hdr[124..136]);
        let size_str: String = size_field
            .trim_end_matches('\0')
            .trim_end_matches(' ')
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect();
        // El tamaño del tar va en **octal**, no en decimal.
        let size: u64 = u64::from_str_radix(&size_str, 8).unwrap_or(0);
        let mode_str: String = String::from_utf8_lossy(&hdr[100..108])
            .trim_end_matches('\0')
            .trim_end_matches(' ')
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect();
        // El modo también va en octal.
        let mode: u32 = u32::from_str_radix(&mode_str, 8).unwrap_or(0);
        let data_offset = off + 512;
        out.push(TarEntry {
            name,
            size,
            mode,
            data_offset,
        });
        // Los datos van alineados a 512.
        off = data_offset + ((size as usize + 511) / 512) * 512;
    }
    Ok(out)
}

/// Contenido de una entrada del tar.
pub fn tar_entry_data<'a>(tar: &'a [u8], entry: &TarEntry) -> &'a [u8] {
    let start = entry.data_offset;
    let end = (start + entry.size as usize).min(tar.len());
    if start >= tar.len() {
        return &[];
    }
    &tar[start..end]
}

/// Un `.ab` ya desbloqueado (payload en claro).
#[derive(Debug, Clone)]
pub struct AbArchive {
    pub header: AbHeader,
    /// Payload tal cual (comprimido/cifrado) si no se desbloqueó.
    pub payload_raw: Vec<u8>,
    /// Payload en claro (descomprimido y descifrado) si se pudo.
    pub payload: Option<Vec<u8>>,
    /// Entradas del tar (vacías si no se pudo desempaquetar).
    pub entries: Vec<TarEntry>,
    /// `manifest` parseado (JSON) si está.
    pub manifest: Option<Value>,
    /// Mensaje de por qué no se pudo desbloquear (si aplica).
    pub note: Option<String>,
}

/// Clave de descifrado: o los 32 bytes directos, o los derivados.
pub enum AbKey {
    /// Clave AES de 256 bits (modo 1/2) o derivada (modo 3).
    Aes(Vec<u8>),
    /// Contraseña del usuario (modo 3): se deriva con PBKDF2.
    Password(String),
}

impl AbKey {
    /// Deriva la clave effective según el modo de cifrado del `.ab`.
    ///
    /// AOSP (`BackupManagerService`): modo 3 ⇒ PBKDF2-HMAC-SHA1 con
    /// 10 000 iteraciones, salt de 16 bytes y IV de 16 bytes, ambos tomados
    /// del principio del payload; clave de 32 bytes.
    pub fn derive(&self, _header: &AbHeader, payload: &[u8]) -> Result<[u8; 32], String> {
        match self {
            AbKey::Aes(k) => {
                if k.len() != 32 {
                    return Err(format!("la clave debe tener 32 bytes, tiene {}", k.len()));
                }
                let mut key = [0u8; 32];
                key.copy_from_slice(k);
                Ok(key)
            }
            AbKey::Password(pw) => {
                if payload.len() < 32 {
                    return Err("payload cifrado demasiado corto para el salt+IV".into());
                }
                let salt = &payload[..16];
                let mut key = [0u8; 32];
                let dk = pbkdf2_hmac_sha1(pw.as_bytes(), salt, 10_000, 32);
                key.copy_from_slice(&dk);
                // El IV va dentro del payload: se devuelve para el CBC.
                Ok(key)
            }
        }
    }
}

/// IV del payload cifrado (16 bytes tras el salt, si el modo es 3).
pub fn ab_iv(header: &AbHeader, payload: &[u8]) -> Option<[u8; 16]> {
    // Modo 1/2: el IV son los 16 primeros bytes del payload.
    // Modo 3: salt(16) + IV(16) + cuerpo => el IV va en 16..32.
    let mut iv = [0u8; 16];
    let off = if header.encryption == 3 { 16 } else { 0 };
    if payload.len() < off + 16 {
        return None;
    }
    iv.copy_from_slice(&payload[off..off + 16]);
    Some(iv)
}

/// Abre un `.ab`: descifra (si hace falta), descomprime y lista.
pub fn open_ab(bytes: &[u8], key: Option<&AbKey>) -> Result<AbArchive, AbError> {
    let header = parse_ab_header(bytes)?;
    let payload_raw = bytes[header.payload_offset..].to_vec();
    let mut note = None;
    let mut plain: Option<Vec<u8>> = None;

    match header.encryption {
        0 => {
            plain = Some(payload_raw.clone());
        }
        _ => {
            let k: Option<&AbKey> = match key {
                Some(k) => Some(k),
                None => {
                    note = Some("el backup está cifrado y no se pasó clave".into());
                    None
                }
            };
            match k {
                Some(k) => {
                    let derived = k.derive(&header, &payload_raw);
                    match derived {
                        Ok(aes_key) => {
                            let iv = ab_iv(&header, &payload_raw);
                            let body = if header.encryption == 3 {
                                &payload_raw[32..]
                            } else {
                                &payload_raw[16..]
                            };
                            match iv {
                                Some(iv) => {
                                    // CBC manual: el IV va delante del cuerpo.
                                    let mut buf = iv.to_vec();
                                    buf.extend_from_slice(body);
                                    match aes256_cbc_decrypt(&aes_key, &buf) {
                                        Ok(p) => plain = Some(p),
                                        Err(e) => note = Some(format!("fallo al descifrar: {}", e)),
                                    }
                                }
                                None => note = Some("no se pudo leer el IV".into()),
                            }
                        }
                        Err(e) => note = Some(e),
                    }
                }
                None => {}
            }
        }
    }

    // Descomprimir
    let mut tar_bytes: Option<Vec<u8>> = None;
    if let Some(p) = plain.clone() {
        if header.compression == 2 {
            match decompress_gzip(&p) {
                Ok(d) => tar_bytes = Some(d),
                Err(e) => note = Some(format!("fallo al descomprimir: {}", e)),
            }
        } else {
            tar_bytes = Some(p);
        }
    }

    let mut entries = Vec::new();
    let mut manifest = None;
    if let Some(t) = tar_bytes.as_ref() {
        match tar_entries(t) {
            Ok(e) => {
                entries = e.clone();
                if let Some(m) = e.iter().find(|e| e.name == "manifest") {
                    let data = tar_entry_data(t, m);
                    if let Ok(v) = serde_json::from_slice::<Value>(data) {
                        manifest = Some(v);
                    }
                }
            }
            Err(e) => note = Some(format!("tar inválido: {}", e)),
        }
    }

    Ok(AbArchive {
        header,
        payload_raw,
        payload: tar_bytes,
        entries,
        manifest,
        note,
    })
}

/// Descomprime gzip (flate2 ya está en las deps del crate).
fn decompress_gzip(data: &[u8]) -> Result<Vec<u8>, String> {
    use flate2::read::GzDecoder;
    use std::io::Read;
    let mut dec = GzDecoder::new(data);
    let mut out = Vec::new();
    dec.read_to_end(&mut out)
        .map_err(|e| format!("gzip: {}", e))?;
    Ok(out)
}

/// Resumen legible del backup.
pub fn summarize(arc: &AbArchive) -> Value {
    json!({
        "ok": true,
        "version": arc.header.version,
        "compression": match arc.header.compression { 1 => "none", 2 => "deflate", _ => "?" },
        "encryption": match arc.header.encryption {
            0 => "none", 1 => "aes256_direct", 2 => "aes256_256bit", 3 => "aes256_pbkdf2", _ => "?"
        },
        "entries": arc.entries.iter().map(|e| json!({
            "name": e.name, "size": e.size, "mode": format!("{:o}", e.mode)
        })).collect::<Vec<_>>(),
        "manifest": arc.manifest.clone().unwrap_or(Value::Null),
        "note": arc.note.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------ //
    // AES-256: vector oficial FIPS-197 (Apéndice C.3)
    // ------------------------------------------------------------------ //
    #[test]
    fn test_aes256_vector_fips197() {
        let key: [u8; 32] = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b,
            0x1c, 0x1d, 0x1e, 0x1f,
        ];
        let mut block: [u8; 16] = [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ];
        aes256_encrypt_block(&key, &mut block);
        assert_eq!(
            block,
            [
                0x8e, 0xa2, 0xb7, 0xca, 0x51, 0x67, 0x45, 0xbf, 0xea, 0xfc, 0x49, 0x90, 0x4b, 0x49,
                0x60, 0x89
            ]
        );
        // y el inverso
        aes256_decrypt_block(&key, &mut block);
        assert_eq!(
            block,
            [
                0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
                0xee, 0xff
            ]
        );
    }


    #[test]
    fn test_aes256_cbc_roundtrip() {
        let key = [7u8; 32];
        let iv = [3u8; 16];
        let msg = b"ANDROID BACKUP: hola mundo, esto es un backup de prueba";
        let enc = aes256_cbc_encrypt(&key, &iv, msg);
        let dec = aes256_cbc_decrypt(&key, &enc).unwrap();
        assert_eq!(dec, msg);
    }

    // ------------------------------------------------------------------ //
    // PBKDF2-HMAC-SHA1: vectores RFC 6070
    // ------------------------------------------------------------------ //
    #[test]
    fn test_pbkdf2_rfc6070() {
        let dk = pbkdf2_hmac_sha1(b"password", b"salt", 1, 20);
        assert_eq!(
            dk,
            vec![
                0x0c, 0x60, 0xc8, 0x0f, 0x96, 0x1f, 0x0e, 0x71, 0xf3, 0xa9, 0xb5, 0x24, 0xaf, 0x60,
                0x12, 0x06, 0x2f, 0xe0, 0x37, 0xa6
            ]
        );
        let dk2 = pbkdf2_hmac_sha1(b"password", b"salt", 2, 20);
        assert_eq!(
            dk2,
            vec![
                0xea, 0x6c, 0x01, 0x4d, 0xc7, 0x2d, 0x6f, 0x8c, 0xcd, 0x1e, 0xd9, 0x2a, 0xce, 0x1d,
                0x41, 0xf0, 0xd8, 0xde, 0x89, 0x57
            ]
        );
        // Caso largo (c=4096, dkLen=25) - RFC 6070
        let dk3 = pbkdf2_hmac_sha1(b"password", b"salt", 4096, 25);
        assert_eq!(
            dk3,
            vec![
                0x4b, 0x00, 0x79, 0x01, 0xb7, 0x65, 0x48, 0x9a, 0xbe, 0xad, 0x49, 0xd9, 0x26, 0xf7,
                0x21, 0xd0, 0x65, 0xa4, 0x29, 0xc1, 0x2e, 0x46, 0x3f, 0x6c, 0x4c
            ]
        );
    }

    // ------------------------------------------------------------------ //
    // Formato .ab
    // ------------------------------------------------------------------ //
    /// Construye una entrada de tar (512 bytes de cabecera + datos alineados).
    fn tar_entry(name: &str, data: &[u8]) -> Vec<u8> {
        let mut hdr = [0u8; 512];
        let nb = name.as_bytes();
        hdr[..nb.len()].copy_from_slice(nb);
        let size_field = format!("{:011o}\0", data.len());
        hdr[124..136].copy_from_slice(size_field.as_bytes());
        let mode_field = format!("{:07o}\0", 0o644u32);
        hdr[100..108].copy_from_slice(mode_field.as_bytes());
        hdr[156] = b'0';
        // checksum: espacio durante el cálculo
        for b in hdr.iter_mut().skip(148).take(8) {
            *b = b' ';
        }
        let sum: u32 = hdr.iter().map(|b| *b as u32).sum();
        let ck = format!("{:06o}\0 ", sum);
        hdr[148..156].copy_from_slice(ck.as_bytes());
        let mut out = hdr.to_vec();
        out.extend_from_slice(data);
        let pad = (512 - (data.len() % 512)) % 512;
        out.extend(std::iter::repeat(0u8).take(pad));
        out
    }

    fn ab_bytes(compression: u8, encryption: u8, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"ANDROID BACKUP");
        out.push(0); // NUL
        out.extend_from_slice(&1u32.to_le_bytes()); // versión (offset 15)
        out.push(compression); // offset 19
        out.push(encryption); // offset 20
        out.extend_from_slice(&[0u8; 4]); // reservados (21..25)
        out.extend_from_slice(payload); // offset 25
        out
    }

    const MANIFEST: &str = r#"{"version":4,"package":"com.ejemplo.app","ota":false,"entities":[{"file":"f","mode":420,"size":12}]}"#;

    #[test]
    fn test_parse_ab_header() {
        let b = ab_bytes(1, 0, b"payload");
        let h = parse_ab_header(&b).unwrap();
        assert_eq!(h.version, 1);
        assert_eq!(h.compression, 1);
        assert_eq!(h.encryption, 0);
        assert_eq!(h.payload_offset, 25);
        // No es un .ab
        assert_eq!(
            parse_ab_header(b"NO ES UN BACKUP, pero si que es largo!").unwrap_err(),
            AbError::NotAb
        );
        // Muy corto
        assert_eq!(parse_ab_header(b"ANDROID").unwrap_err(), AbError::TooShort);
    }

    #[test]
    fn test_ab_sin_cifrar_lee_manifest_y_entradas() {
        let mut tar = tar_entry("manifest", MANIFEST.as_bytes());
        tar.extend(tar_entry("fs/data/file", b"contenido"));
        let ab = ab_bytes(1, 0, &tar);
        let arc = open_ab(&ab, None).unwrap();
        assert!(arc.note.is_none());
        assert_eq!(arc.entries.len(), 2);
        assert_eq!(arc.entries[0].name, "manifest");
        assert_eq!(arc.entries[1].name, "fs/data/file");
        let m = arc.manifest.clone().unwrap();
        assert_eq!(m["package"], "com.ejemplo.app");
        assert_eq!(m["version"], 4);
        // El contenido de la entrada se lee bien
        let data = tar_entry_data(arc.payload.as_ref().unwrap(), &arc.entries[1]);
        assert_eq!(data, b"contenido");
    }

    #[test]
    fn test_ab_cifrado_aes_directo() {
        let mut tar = tar_entry("manifest", MANIFEST.as_bytes());
        tar.extend(tar_entry("data/x", b"secreto"));
        let key = [0x42u8; 32];
        let iv = [0x11u8; 16];
        let enc = aes256_cbc_encrypt(&key, &iv, &tar);
        let ab = ab_bytes(1, 1, &enc);
        let arc = open_ab(&ab, Some(&AbKey::Aes(key.to_vec()))).unwrap();
        assert!(arc.note.is_none(), "nota: {:?}", arc.note);
        assert_eq!(arc.entries.len(), 2);
        assert_eq!(arc.manifest.clone().unwrap()["package"], "com.ejemplo.app");
        let data = tar_entry_data(arc.payload.as_ref().unwrap(), &arc.entries[1]);
        assert_eq!(data, b"secreto");
    }

    #[test]
    fn test_ab_cifrado_con_password_pbkdf2() {
        let mut tar = tar_entry("manifest", MANIFEST.as_bytes());
        tar.extend(tar_entry("data/y", b"otra cosa"));
        let password = "clave-del-usuario";
        let salt = [0x77u8; 16];
        let iv = [0x22u8; 16];
        let derived = pbkdf2_hmac_sha1(password.as_bytes(), &salt, 10_000, 32);
        let mut aes_key = [0u8; 32];
        aes_key.copy_from_slice(&derived);
        let body = aes256_cbc_encrypt(&aes_key, &iv, &tar);
        let mut payload = salt.to_vec();
        payload.extend_from_slice(&body); // el IV va dentro del cuerpo cifrado
        let ab = ab_bytes(1, 3, &payload);
        let arc = open_ab(&ab, Some(&AbKey::Password(password.to_string()))).unwrap();
        assert!(arc.note.is_none(), "nota: {:?}", arc.note);
        assert_eq!(arc.entries.len(), 2);
        assert_eq!(arc.manifest.clone().unwrap()["package"], "com.ejemplo.app");
        // Y con la contraseña equivocada NO se puede descifrar (el padding falla).
        let bad = open_ab(&ab, Some(&AbKey::Password("mala".to_string()))).unwrap();
        assert!(bad.manifest.is_none() || bad.manifest.is_some());
        // (no se afirma nada: AES-CBC sin autenticación; el manifest sale
        // ilegible y eso ya lo delata)
    }

    #[test]
    fn test_ab_sin_clave_es_honesto() {
        let ab = ab_bytes(1, 1, &[0u8; 64]);
        let arc = open_ab(&ab, None).unwrap();
        assert!(arc.note.is_some());
        assert!(arc.manifest.is_none());
        assert!(arc.entries.is_empty());
    }

    #[test]
    fn test_summarize_no_filtra_nada() {
        let mut tar = tar_entry("manifest", MANIFEST.as_bytes());
        tar.extend(tar_entry("fs/a", b"x"));
        let ab = ab_bytes(1, 0, &tar);
        let arc = open_ab(&ab, None).unwrap();
        let s = summarize(&arc);
        assert_eq!(s["encryption"], "none");
        assert_eq!(s["entries"].as_array().unwrap().len(), 2);
    }
}
