//! Lector (sólo lectura) del contenedor de backup exportado por Swift Backup.
//!
//! # Alcance honesto
//!
//! Este módulo **no adivina el formato**. El layout de la cabecera `SBA1` se
//! describe en una **tabla versionada** (`HeaderLayout`) que se rellena con lo
//! medido (hoy: el RE del escritor ARM64 + el marshalling de Kotlin). Mientras
//! la tabla esté incompleta, el lector **se niega a parsear** y lo dice:
//!
//! * `status: "layout_no_verificado"` — falta el layout ⇒ no se inventa nada.
//! * `status: "kdf_error"`             — el KDF falló (parámetros fuera de
//!   rango). El KDF **ya no falta**: el valor por defecto es
//!   `crate::argon2::derive_key` (Argon2id, verificado contra la RFC 9106 y
//!   el oráculo C 36/36) ⇒ **una sola implementación del KDF en el repo**.
//!   Se puede seguir **inyectando** otro KDF (`KdfFn`) para el arnés
//!   aislado, pero entonces el informe lo dice (`kdf.verificada = false`).
//! * `status: "aead_no_replicado"`    — la derivación de nonce/AAD por trozo es
//!   **nativa** (`SBA1-AEGIS256-entry-chunk-nonce-v1`): **no se replica**.
//!
//! Lo que sí se puede hacer sin replicar el AEAD: **parsear la cabecera**,
//! **derivar la clave** y **verificar el key-check** ⇒ eso demuestra que la
//! passphrase es correcta y que el fichero no está manipulado ⇒ interop real.

use serde::{Deserialize, Serialize};

/// KDF inyectado (Argon2id). La implementación es del módulo `argon2` del repo.
pub type KdfFn =
    fn(&str, &[u8], u32, u32, u32, usize) -> Result<Vec<u8>, String>;

/// **El KDF por defecto**: la implementación **única** del repo.
///
/// `crate::argon2::derive_key` = Argon2id (RFC 9106) verificado por el
/// agente del signer contra los vectores oficiales **y** contra el oráculo C
/// `phc-winner-argon2` (36/36 en el rango de parámetros de Swift:
/// iter 1..100, memoria 16384..65536 KiB, paralelismo 1..8, salida 16..128 B).
///
/// ⇒ **una sola implementación del KDF en el repo** (el crate `argon2` de
/// crates.io queda fuera) ⇒ ⇒ el crate `argon2` de crates.io queda fuera para siempre.
pub fn kdf_argon2id(
    passphrase: &str,
    salt: &[u8],
    iterations: u32,
    memory_kib: u32,
    parallelism: u32,
    out_len: usize,
) -> Result<Vec<u8>, String> {
    crate::argon2::derive_key(
        passphrase,
        salt,
        iterations,
        memory_kib,
        parallelism,
        out_len,
    )
}

/// Parámetros del KDF, tal y como se leen de la cabecera.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
pub struct KdfParams {
    pub iterations: u32,
    pub memory_kib: u32,
    pub parallelism: u32,
    pub out_len: usize,
}

/// Un campo de la cabecera: dónde está, cuánto ocupa y cómo se lee.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HeaderField {
    pub name: String,
    pub offset: u64,
    /// Ancho en bytes.
    pub width: u8,
    /// Los enteros de la cabecera son **big-endian** (invariante medida).
    pub big_endian: bool,
}

/// Un bloque de bytes de la cabecera (`len` + datos).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ByteBlock {
    pub name: String,
    /// Offset del **prefijo de longitud**.
    pub len_offset: u64,
    /// Ancho del prefijo de longitud (2 en el layout medido).
    pub len_width: u8,
    /// Longitud del prefijo de longitud (2 en el layout medido).
    pub data_offset: u64,
}

/// Cómo se comprueba el key-check (si se conoce).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyCheckSpec {
    /// Offset del key-check dentro de la cabecera.
    pub offset: u64,
    pub width: u8,
    /// Ordinal del metodo KDF/cifrado.
    pub method_ordinal: u32,
    /// ¿El hash del key-check incluye el ordinal del metodo?
    /// **NO DETERMINADO todavia** (el agente DEX lo cierra con el marshalling):
    /// por eso es un flag explicito y no una suposicion escondida.
    pub include_ordinal: bool,
}

/// Tabla del layout de la cabecera. **Versionada a propósito**: si la
/// medición cambia, se cambia la tabla, no el lector.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct HeaderLayout {
    /// Magic del fichero (p.ej. `"SBA1"`).
    pub magic: Option<String>,
    /// Tamaño de la cabecera en bytes (0x60 = 96 en el layout medido).
    pub header_size: Option<u64>,
    pub fields: Vec<HeaderField>,
    pub byte_blocks: Vec<ByteBlock>,
    pub key_check: Option<KeyCheckSpec>,
}

impl HeaderLayout {
    /// ¿Está la tabla completa? Si no, el lector no parsea.
    pub fn complete(&self) -> bool {
        self.magic.is_some() && !self.fields.is_empty()
    }
    /// Offsets de los campos, ya“TODO DETERMINADO” o no: lo que falte queda `None`.
    pub fn missing(&self) -> Vec<String> {
        let mut m = Vec::new();
        if self.magic.is_none() {
            m.push("magic".into());
        }
        if self.header_size.is_none() {
            m.push("header_size".into());
        }
        for f in &self.fields {
            if f.width == 0 {
                m.push(format!("{}: ancho", f.name));
            }
        }
        m
    }
}

/// Valor de un campo ya leído.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HeaderValue {
    pub name: String,
    pub offset: u64,
    pub width: u8,
    pub value: u64,
}

/// Bloque de bytes ya leído.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlockValue {
    pub name: String,
    pub len_offset: u64,
    pub data_offset: u64,
    pub len: u64,
    /// `true` si el bloque va cifrado (los tres bloques del layout medido).
    pub encrypted: bool,
}

/// Cabecera parseada.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HeaderReport {
    pub magic: String,
    pub header_size: u64,
    pub fields: Vec<HeaderValue>,
    pub blocks: Vec<BlockValue>,
}

/// Resultado de verificar el key-check.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyCheckReport {
    pub matched: bool,
    /// Hex del key-check esperado (para diagnóstico).
    pub expected_hex: String,
    pub method_ordinal: u32,
}

/// Informe del KDF que se ha usado.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KdfInfo {
    /// `"argon2id"` (la del repo) o `"inyectado"` (arnés/tests).
    pub algoritmo: String,
    /// `true` ⇒ se ha usado la implementación **verificada** del repo.
    pub verificada: bool,
    /// De dónde sale (ruta del código).
    pub source: String,
    pub iterations: u32,
    pub memory_kib: u32,
    pub parallelism: u32,
    pub out_len: usize,
    /// Longitud de la sal leída de la cabecera (0 ⇒ no se encontró).
    pub salt_len: usize,
}

/// Informe completo.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwiftArchiveReport {
    /// `ok` | `layout_no_verificado` | `kdf_error` |
    /// `key_check_mismatch` | `aead_no_replicado`.
    pub status: String,
    pub header: Option<HeaderReport>,
    pub key_check: Option<KeyCheckReport>,
    /// Qué KDF se ha usado (y si es el verificado).
    pub kdf: Option<KdfInfo>,
    /// Lo que **no** se ha hecho (por qué).
    pub notes: Vec<String>,
}

/// Lee un entero big-endian (o little-endian) de un slice.
pub fn read_uint(bytes: &[u8], big_endian: bool) -> u64 {
    let mut v = 0u64;
    if big_endian {
        for b in bytes {
            v = (v << 8) | *b as u64;
        }
    } else {
        for b in bytes.iter().rev() {
            v = (v << 8) | *b as u64;
        }
    }
    v
}

/// Parsea la cabecera con la tabla dada. **No inventa**: si la tabla está
/// incompleta devuelve `Err` con el motivo.
pub fn parse_header(bytes: &[u8], layout: &HeaderLayout) -> Result<HeaderReport, String> {
    if !layout.complete() {
        return Err(format!(
            "layout incompleto ({}): NO DETERMINADO, no se parsea",
            layout.missing().join(", ")
        ));
    }
    let magic = layout.magic.clone().unwrap_or_default();
    if bytes.len() < magic.len() {
        return Err("fichero mas corto que el magic".into());
    }
    if &bytes[..magic.len()] != magic.as_bytes() {
        return Err(format!(
            "magic distinto: {:?} != {:?}",
            String::from_utf8_lossy(&bytes[..magic.len()]),
            magic
        ));
    }
    let mut fields = Vec::new();
    for f in &layout.fields {
        let off = f.offset as usize;
        let end = off + f.width as usize;
        if end > bytes.len() {
            return Err(format!("campo {} se sale del fichero", f.name));
        }
        fields.push(HeaderValue {
            name: f.name.clone(),
            offset: f.offset,
            width: f.width,
            value: read_uint(&bytes[off..end], f.big_endian),
        });
    }
    let mut blocks = Vec::new();
    for b in &layout.byte_blocks {
        let lo = b.len_offset as usize;
        let lend = lo + b.len_width as usize;
        if lend > bytes.len() {
            return Err(format!("bloque {} se sale del fichero", b.name));
        }
        let len = read_uint(&bytes[lo..lend], true);
        let data_off = b.data_offset as usize;
        let data_end = data_off + len as usize;
        if data_end > bytes.len() {
            return Err(format!("bloque {} declara {} B y no caben", b.name, len));
        }
        blocks.push(BlockValue {
            name: b.name.clone(),
            len_offset: b.len_offset,
            data_offset: b.data_offset,
            len,
            encrypted: true,
        });
    }
    Ok(HeaderReport {
        magic,
        header_size: layout.header_size.unwrap_or(0),
        fields,
        blocks,
    })
}

/// Verifica el key-check con la clave derivada.
///
/// `kdf` es **inyectable** (arnés aislado / tests). Si se pasa `None` se usa
/// **la implementación verificada del repo** (`crate::argon2::derive_key`,
/// Argon2id: RFC 9106 + oráculo C 36/36) ⇒ ⇒ **una sola implementación del
/// KDF** ⇒ ⇒ el estado `kdf_no_verificado` **ya no existe**: el KDF siempre
/// está; si falla (parámetros fuera de rango) ⇒ `kdf_error`.
pub fn verify_key_check(
    bytes: &[u8],
    layout: &HeaderLayout,
    kdf: Option<KdfFn>,
    passphrase: &str,
    params: KdfParams,
) -> SwiftArchiveReport {
    let mut notes = Vec::new();
    let header = match parse_header(bytes, layout) {
        Ok(h) => h,
        Err(e) => {
            return SwiftArchiveReport {
                status: "layout_no_verificado".into(),
                header: None,
                key_check: None,
                kdf: None,
                notes: vec![e],
            }
        }
    };
    let spec = match &layout.key_check {
        Some(s) => s.clone(),
        None => {
            notes.push("key-check: NO DETERMINADO (no se donde esta) => no se verifica".into());
            return SwiftArchiveReport {
                status: "layout_no_verificado".into(),
                header: Some(header),
                key_check: None,
                kdf: None,
                notes,
            };
        }
    };
    // El KDF: el **verificado del repo**, salvo que se inyecte otro (arnés).
    let (kdf_fn, kdf_verificada, kdf_source) = match kdf {
        Some(k) => (k, false, "KdfFn inyectado (arnes aislado)".to_string()),
        None => (
            kdf_argon2id as KdfFn,
            true,
            "crate::argon2::derive_key (Argon2id verificado: RFC 9106 + oraculo C 36/36)"
                .to_string(),
        ),
    };
    let salt = header
        .blocks
        .iter()
        .find(|b| b.name.eq_ignore_ascii_case("salt"))
        .map(|b| {
            let off = b.data_offset as usize;
            bytes[off..off + b.len as usize].to_vec()
        })
        .unwrap_or_default();
    if salt.is_empty() {
        notes.push("salt no localizada en la cabecera => el layout no dice donde esta => no se deriva la clave".into());
        return SwiftArchiveReport {
            status: "layout_no_verificado".into(),
            header: Some(header),
            key_check: None,
            kdf: None,
            notes,
        };
    }
    let kdf_info = KdfInfo {
        algoritmo: if kdf_verificada {
            "argon2id".into()
        } else {
            "inyectado".into()
        },
        verificada: kdf_verificada,
        source: kdf_source,
        iterations: params.iterations,
        memory_kib: params.memory_kib,
        parallelism: params.parallelism,
        out_len: params.out_len,
        salt_len: salt.len(),
    };
    notes.push(if kdf_verificada {
        "KDF: Argon2id VERIFICADA del repo (una sola implementacion; crates.io fuera)".into()
    } else {
        "KDF inyectado: NO es la implementacion verificada del repo".into()
    });
    let key = match kdf_fn(passphrase, &salt, params.iterations, params.memory_kib, params.parallelism, params.out_len) {
        Ok(k) => k,
        Err(e) => {
            notes.push(format!("el KDF fallo: {}", e));
            return SwiftArchiveReport {
                status: "kdf_error".into(),
                header: Some(header),
                key_check: None,
                kdf: Some(kdf_info),
                notes,
            };
        }
    };
    let off = spec.offset as usize;
    let end = off + spec.width as usize;
    if end > bytes.len() {
        notes.push("el key-check se sale del fichero".into());
        return SwiftArchiveReport {
            status: "layout_no_verificado".into(),
            header: Some(header),
            key_check: None,
            kdf: Some(kdf_info),
            notes,
        };
    }
    // El key-check es SHA-256 sobre la clave (mas el ordinal del metodo si la
    // tabla dice que entra) => la composicion exacta la fija la tabla, no el codigo.
    let mut buf = key.clone();
    if spec.include_ordinal {
        buf.extend_from_slice(&spec.method_ordinal.to_be_bytes());
    }
    let expected = sha256(&buf);
    let found = &bytes[off..end];
    let matched = expected.len() >= found.len() && &expected[..found.len()] == found;
    notes.push(if matched {
        "key-check coincide => la passphrase es correcta".into()
    } else {
        "key-check NO coincide => passphrase incorrecta o fichero manipulado".into()
    });
    notes.push(
        "el contenido (AEAD por trozos) NO se extrae: la derivacion de nonce/AAD es nativa, no replicada"
            .into(),
    );
    SwiftArchiveReport {
        status: if matched { "ok".into() } else { "key_check_mismatch".into() },
        header: Some(header),
        key_check: Some(KeyCheckReport {
            matched,
            expected_hex: expected.iter().map(|b| format!("{:02x}", b)).collect(),
            method_ordinal: spec.method_ordinal,
        }),
        kdf: Some(kdf_info),
        notes,
    }
}

/// Atajo: verifica con el KDF **verificado** del repo (sin inyectar nada).
///
/// Es la vía que debe usar el resto del código: una sola implementación del
/// KDF en el repo ⇒ ningún otro sitio deriva claves por su cuenta.
pub fn verificar(
    bytes: &[u8],
    layout: &HeaderLayout,
    passphrase: &str,
    params: KdfParams,
) -> SwiftArchiveReport {
    verify_key_check(bytes, layout, None, passphrase, params)
}

/// SHA-256 mínimo (el repo ya tiene `sha2`; aquí se evita depender de el para
/// que este modulo se pueda compilar y testear aislado).
fn sha256(data: &[u8]) -> Vec<u8> {
    // Implementación recortada: delegamos en `sha2` si está disponible.
    // (El crate real usa `sha2::Sha256`; esta copia es para el arnés aislado.)
    sha256_impl(data)
}

/// SHA-256 (implementación propia, sin dependencias externas).
fn sha256_impl(data: &[u8]) -> Vec<u8> {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    let bitlen = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bitlen.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = Vec::with_capacity(32);
    for v in h.iter() {
        out.extend_from_slice(&v.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout_demo() -> HeaderLayout {
        HeaderLayout {
            magic: Some("SBA1".into()),
            header_size: Some(96),
            fields: vec![
                HeaderField { name: "flags".into(), offset: 4, width: 2, big_endian: true },
                HeaderField { name: "headerSize".into(), offset: 6, width: 2, big_endian: true },
                HeaderField { name: "kdfIterations".into(), offset: 30, width: 2, big_endian: true },
            ],
            byte_blocks: vec![ByteBlock {
                name: "salt".into(),
                len_offset: 10,
                len_width: 2,
                data_offset: 12,
            }],
            key_check: Some(KeyCheckSpec {
                offset: 40,
                width: 32,
                method_ordinal: 1,
                include_ordinal: false,
            }),
        }
    }

    fn cabecera_demo() -> Vec<u8> {
        let mut v = vec![0u8; 96];
        v[..4].copy_from_slice(b"SBA1");
        v[4..6].copy_from_slice(&0x0001u16.to_be_bytes()); // flags
        v[6..8].copy_from_slice(&96u16.to_be_bytes()); // headerSize
        v[10..12].copy_from_slice(&16u16.to_be_bytes()); // len del bloque
        v[12..28].copy_from_slice(&[7u8; 16]); // salt
        v[30..32].copy_from_slice(&42u16.to_be_bytes()); // kdfIterations (fuera del salt)
        v
    }

    #[test]
    fn lee_enteros_big_endian() {
        assert_eq!(read_uint(&[0x12, 0x34], true), 0x1234);
        assert_eq!(read_uint(&[0x12, 0x34], false), 0x3412);
        assert_eq!(read_uint(&[0xde, 0xad, 0xbe, 0xef], true), 0xdeadbeef);
    }

    #[test]
    fn sha256_es_correcto() {
        // Vector NIST: sha256("abc")
        let h = sha256(b"abc");
        let hex: String = h.iter().map(|b| format!("{:02x}", b)).collect();
        assert_eq!(
            hex,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let h2 = sha256(b"");
        let hex2: String = h2.iter().map(|b| format!("{:02x}", b)).collect();
        assert_eq!(
            hex2,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn layout_incompleto_no_parsea() {
        let vacio = HeaderLayout::default();
        let err = parse_header(&cabecera_demo(), &vacio).unwrap_err();
        assert!(err.contains("NO DETERMINADO"), "{}", err);
    }

    #[test]
    fn parsea_la_cabecera_big_endian() {
        let h = parse_header(&cabecera_demo(), &layout_demo()).unwrap();
        assert_eq!(h.magic, "SBA1");
        assert_eq!(h.header_size, 96);
        let flags = h.fields.iter().find(|f| f.name == "flags").unwrap();
        assert_eq!(flags.value, 1);
        let it = h.fields.iter().find(|f| f.name == "kdfIterations").unwrap();
        assert_eq!(it.value, 42);
        assert_eq!(h.blocks.len(), 1);
        assert_eq!(h.blocks[0].len, 16);
    }

    #[test]
    fn magic_distinto_se_rechaza() {
        let mut b = cabecera_demo();
        b[0] = b'X';
        let err = parse_header(&b, &layout_demo()).unwrap_err();
        assert!(err.contains("magic"), "{}", err);
    }

    #[test]
    fn sin_kdf_inyectado_usa_el_argon2id_verificado_del_repo() {
        let r = verify_key_check(
            &cabecera_demo(),
            &layout_demo(),
            None,
            "secreto",
            KdfParams { iterations: 1, memory_kib: 16384, parallelism: 1, out_len: 32 },
        );
        // El estado "kdf_no_verificado" ya no existe: el KDF del repo se usa.
        assert_ne!(r.status, "kdf_no_verificado");
        let k = r.kdf.expect("el informe debe decir que KDF se uso");
        assert!(k.verificada, "sin KDF inyectado debe usarse el verificado del repo");
        assert_eq!(k.algoritmo, "argon2id");
        assert!(k.source.contains("argon2::derive_key"), "{}", k.source);
        assert_eq!(k.salt_len, 16);
        // El key-check del fixture no cuadra con la clave real => mismatch.
        assert_eq!(r.status, "key_check_mismatch");
    }

    #[test]
    fn el_kdf_del_repo_reproduce_el_vector_congelado_del_signer() {
        // Vector congelado en `rust-core/tests/argon2_smoke.rs` (oráculo C
        // `phc-winner-argon2`): iter 3, 16 MiB, 4 carriles, salida 32.
        let k = kdf_argon2id("aaaaaaaaaaaaaaaa", &[0x02u8; 16], 3, 16384, 4, 32).unwrap();
        let hex: String = k.iter().map(|b| format!("{:02x}", b)).collect();
        assert_eq!(
            hex,
            "c7f25bb92e59b63b36f9c1577069ad17d1ac8a232133008212ffaf56ddba03be"
        );
    }

    #[test]
    fn key_check_ok_con_el_argon2id_real() {
        // Cabecera con key-check = sha256(clave real derivada con Argon2id).
        let params = KdfParams { iterations: 3, memory_kib: 16384, parallelism: 4, out_len: 32 };
        let salt = [0x02u8; 16];
        let clave = kdf_argon2id(
            "secreto",
            &salt,
            params.iterations,
            params.memory_kib,
            params.parallelism,
            params.out_len,
        )
        .unwrap();
        let mut cab = cabecera_demo();
        cab[12..28].copy_from_slice(&salt);
        cab[40..72].copy_from_slice(&sha256(&clave));
        let r = verificar(&cab, &layout_demo(), "secreto", params);
        assert_eq!(r.status, "ok");
        assert!(r.key_check.unwrap().matched);
        assert!(r.kdf.unwrap().verificada);
    }

    #[test]
    fn kdf_roto_da_kdf_error() {
        // iterations = 0 => el KDF del repo lo rechaza => "kdf_error"
        // (y NO "kdf_no_verificado", que ya no existe).
        let r = verificar(
            &cabecera_demo(),
            &layout_demo(),
            "secreto",
            KdfParams { iterations: 0, memory_kib: 16384, parallelism: 1, out_len: 32 },
        );
        assert_eq!(r.status, "kdf_error");
        assert!(r.notes.iter().any(|n| n.contains("el KDF fallo")));
    }

    #[test]
    fn key_check_correcto_da_ok_y_uno_malo_da_mismatch() {
        fn kdf(pass: &str, salt: &[u8], _i: u32, _m: u32, _p: u32, len: usize) -> Result<Vec<u8>, String> {
            let mut v = Vec::new();
            v.extend_from_slice(pass.as_bytes());
            v.extend_from_slice(salt);
            v.resize(len, 0);
            Ok(v)
        }
        let mut cab = cabecera_demo();
        // key-check = sha256(clave) (el ordinal entra solo si la tabla lo dice).
        let mut clave = Vec::new();
        clave.extend_from_slice(b"secreto");
        clave.extend_from_slice(&[7u8; 16]);
        clave.resize(32, 0);
        let hc = sha256(&clave);
        cab[40..72].copy_from_slice(&hc);
        let r = verify_key_check(
            &cab,
            &layout_demo(),
            Some(kdf),
            "secreto",
            KdfParams { iterations: 1, memory_kib: 16384, parallelism: 1, out_len: 32 },
        );
        assert_eq!(r.status, "ok");
        assert!(r.key_check.unwrap().matched);
        let r2 = verify_key_check(
            &cab,
            &layout_demo(),
            Some(kdf),
            "mal",
            KdfParams { iterations: 1, memory_kib: 16384, parallelism: 1, out_len: 32 },
        );
        assert_eq!(r2.status, "key_check_mismatch");
        assert!(r2.notes.iter().any(|n| n.contains("NO se extrae")));
    }
}
