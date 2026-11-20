//! **AEGIS-256** (RFC 10032) en Rust puro, sin crates nuevos.
//!
//! Swift Backup cifra sus backups con **AEGIS-256** (constantes medidas en la
//! app instalada: `AEGIS-256`, `Aegis256`, `SBA1-AEGIS256-key-check-v1`), así
//! que sin esto no se puede leer un `.dat` de Swift Backup.
//!
//! Implementación **exacta** de la RFC 10032 (AEGIS-256):
//! * Estado de 768 bits: seis bloques de 128 bits `{S0..S5}`.
//! * `Update(M)`: `S'0 = AESRound(S5, S0^M)`, `S'i = AESRound(S(i-1), Si)`.
//! * `Enc(xi)`: `z = S1 ^ S4 ^ S5 ^ (S2 & S3)`; `Update(xi)`; `ci = xi ^ z`.
//! * `Finalize`: `t = S3 ^ (LE64(ad_len_bits) || LE64(msg_len_bits))`,
//!   7 `Update(t)`, y tag de 128 bits = `S0^S1^S2^S3^S4^S5`.
//!
//! Validado con los vectores oficiales de la RFC (A.3): Update, TV1, TV2, TV3.

use crate::backup_vault::aes_round;

/// Constante `C0` de la RFC 10032 (sec. 3.1): números de Fibonacci.
pub const C0: [u8; 16] = [
    0x00, 0x01, 0x01, 0x02, 0x03, 0x05, 0x08, 0x0d, 0x15, 0x22, 0x37, 0x59, 0x90, 0xe9, 0x79, 0x62,
];

/// Constante `C1` de la RFC 10032 (sec. 3.1).
pub const C1: [u8; 16] = [
    0xdb, 0x3d, 0x18, 0x55, 0x6d, 0xc2, 0x2f, 0xf1, 0x20, 0x11, 0x31, 0x42, 0x73, 0xb5, 0x28, 0xdd,
];

/// Longitud de la clave de AEGIS-256 (256 bits).
pub const KEY_LEN: usize = 32;
/// Longitud del nonce de AEGIS-256 (256 bits).
pub const NONCE_LEN: usize = 32;
/// Longitud de la etiqueta por defecto (128 bits).
pub const TAG_LEN: usize = 16;

/// Estado AEGIS-256 (seis bloques de 128 bits).
#[derive(Debug, Clone, PartialEq)]
pub struct Aegis256 {
    s: [[u8; 16]; 6],
}

impl Aegis256 {
    /// Inicializa el estado con clave y nonce (RFC 10032, sec. 4.4).
    pub fn new(key: &[u8], nonce: &[u8]) -> Result<Aegis256, String> {
        if key.len() != KEY_LEN {
            return Err(format!("la clave de AEGIS-256 son {KEY_LEN} bytes, no {}", key.len()));
        }
        if nonce.len() != NONCE_LEN {
            return Err(format!("el nonce de AEGIS-256 son {NONCE_LEN} bytes, no {}", nonce.len()));
        }
        let mut k0 = [0u8; 16];
        let mut k1 = [0u8; 16];
        k0.copy_from_slice(&key[..16]);
        k1.copy_from_slice(&key[16..]);
        let mut n0 = [0u8; 16];
        let mut n1 = [0u8; 16];
        n0.copy_from_slice(&nonce[..16]);
        n1.copy_from_slice(&nonce[16..]);

        let mut st = Aegis256 {
            s: [[0u8; 16]; 6],
        };
        st.s[0] = xor16(&k0, &n0);
        st.s[1] = xor16(&k1, &n1);
        st.s[2] = C1;
        st.s[3] = C0;
        st.s[4] = xor16(&k0, &C0);
        st.s[5] = xor16(&k1, &C1);
        for _ in 0..4 {
            st.update(&k0);
            st.update(&k1);
            st.update(&xor16(&k0, &n0));
            st.update(&xor16(&k1, &n1));
        }
        Ok(st)
    }

    /// Construye un estado a partir de bloques dados (para validar el
    /// `Update` con el vector de la RFC).
    pub fn with_state(state: [[u8; 16]; 6]) -> Aegis256 {
        Aegis256 { s: state }
    }

    /// Estado actual (para pruebas).
    pub fn state(&self) -> &[[u8; 16]; 6] {
        &self.s
    }

    /// `Update(M)` (RFC 10032, sec. 4.3).
    pub fn update(&mut self, m: &[u8; 16]) {
        let s = self.s;
        let mut n = [[0u8; 16]; 6];
        n[0] = aes_round(&s[5], &xor16(&s[0], m));
        n[1] = aes_round(&s[0], &s[1]);
        n[2] = aes_round(&s[1], &s[2]);
        n[3] = aes_round(&s[2], &s[3]);
        n[4] = aes_round(&s[3], &s[4]);
        n[5] = aes_round(&s[4], &s[5]);
        self.s = n;
    }

    /// `Enc(xi)`: cifra un bloque (RFC 10032, sec. 4.6).
    pub fn enc(&mut self, x: &[u8; 16]) -> [u8; 16] {
        let z = self.z();
        self.update(x);
        xor16(x, &z)
    }

    /// `Dec(ci)`: descifra un bloque (RFC 10032, sec. 4.7).
    pub fn dec(&mut self, c: &[u8; 16]) -> [u8; 16] {
        let z = self.z();
        let x = xor16(c, &z);
        self.update(&x);
        x
    }

    /// `z = S1 ^ S4 ^ S5 ^ (S2 & S3)`.
    fn z(&self) -> [u8; 16] {
        let mut z = [0u8; 16];
        for i in 0..16 {
            z[i] = self.s[1][i] ^ self.s[4][i] ^ self.s[5][i] ^ (self.s[2][i] & self.s[3][i]);
        }
        z
    }

    /// `Finalize` (RFC 10032, sec. 4.9).
    pub fn finalize(&mut self, ad_len_bits: u64, msg_len_bits: u64, tag_len: usize) -> Vec<u8> {
        let mut t = [0u8; 16];
        t[..8].copy_from_slice(&ad_len_bits.to_le_bytes());
        t[8..].copy_from_slice(&msg_len_bits.to_le_bytes());
        let mut base = self.s[3];
        for i in 0..16 {
            base[i] ^= t[i];
        }
        for _ in 0..7 {
            self.update(&base);
        }
        let mut tag = Vec::with_capacity(tag_len);
        if tag_len <= 16 {
            let mut x = [0u8; 16];
            for i in 0..16 {
                x[i] = self.s[0][i] ^ self.s[1][i] ^ self.s[2][i] ^ self.s[3][i] ^ self.s[4][i] ^ self.s[5][i];
            }
            tag.extend_from_slice(&x[..tag_len]);
        } else {
            let mut x = [0u8; 16];
            let mut y = [0u8; 16];
            for i in 0..16 {
                x[i] = self.s[0][i] ^ self.s[1][i] ^ self.s[2][i];
                y[i] = self.s[3][i] ^ self.s[4][i] ^ self.s[5][i];
            }
            tag.extend_from_slice(&x);
            tag.extend_from_slice(&y);
            tag.truncate(tag_len);
        }
        tag
    }
}

fn xor16(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut o = [0u8; 16];
    for i in 0..16 {
        o[i] = a[i] ^ b[i];
    }
    o
}

/// Cifra y autentica (RFC 10032, sec. 4.1).
///
/// Devuelve `(cifrado, etiqueta)`.
pub fn encrypt(
    key: &[u8],
    nonce: &[u8],
    msg: &[u8],
    ad: &[u8],
    tag_len: usize,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    let mut st = Aegis256::new(key, nonce)?;
    // datos asociados (rellenados a 128 bits)
    for block in zero_pad(ad).chunks(16) {
        let mut b = [0u8; 16];
        b.copy_from_slice(block);
        st.update(&b);
    }
    // mensaje
    let mut ct = Vec::with_capacity(msg.len());
    for block in zero_pad(msg).chunks(16) {
        let mut b = [0u8; 16];
        b.copy_from_slice(block);
        let c = st.enc(&b);
        ct.extend_from_slice(&c);
    }
    ct.truncate(msg.len());
    let tag = st.finalize((ad.len() as u64) * 8, (msg.len() as u64) * 8, tag_len);
    Ok((ct, tag))
}

/// Descifra y verifica la etiqueta (RFC 10032, sec. 4.2).
///
/// La comparación de la etiqueta es **en tiempo constante**.
pub fn decrypt(
    key: &[u8],
    nonce: &[u8],
    ct: &[u8],
    ad: &[u8],
    tag: &[u8],
) -> Result<Vec<u8>, String> {
    let mut st = Aegis256::new(key, nonce)?;
    for block in zero_pad(ad).chunks(16) {
        let mut b = [0u8; 16];
        b.copy_from_slice(block);
        st.update(&b);
    }
    let mut msg = Vec::with_capacity(ct.len());
    let full = ct.len() - (ct.len() % 16);
    for block in ct[..full].chunks(16) {
        let mut c = [0u8; 16];
        c.copy_from_slice(block);
        let m = st.dec(&c);
        msg.extend_from_slice(&m);
    }
    let rest = &ct[full..];
    if !rest.is_empty() {
        // DecPartial
        let z = st.z();
        let mut t = [0u8; 16];
        t[..rest.len()].copy_from_slice(rest);
        let mut x = [0u8; 16];
        for i in 0..16 {
            x[i] = t[i] ^ z[i];
        }
        let mut padded = [0u8; 16];
        padded[..rest.len()].copy_from_slice(&x[..rest.len()]);
        st.update(&padded);
        msg.extend_from_slice(&x[..rest.len()]);
    }
    let expected = st.finalize((ad.len() as u64) * 8, (msg.len() as u64) * 8, tag.len());
    if !ct_eq(&expected, tag) {
        return Err("la etiqueta no cuadra: datos alterados o clave/nonce distintos".to_string());
    }
    Ok(msg)
}

/// Comparación en tiempo constante.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut d = 0u8;
    for i in 0..a.len() {
        d |= a[i] ^ b[i];
    }
    d == 0
}

fn zero_pad(data: &[u8]) -> Vec<u8> {
    let rem = data.len() % 16;
    if rem == 0 {
        return data.to_vec();
    }
    let mut v = data.to_vec();
    v.resize(v.len() + (16 - rem), 0);
    v
}

/// Hexadecimal en minúsculas.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
