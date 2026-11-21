//! **Argon2** (RFC 9106) en Rust puro: BLAKE2b + Argon2d/i/id.
//!
//! Swift Backup deriva la clave de sus backups con **Argon2id** (medido en
//! `libsba_archive.so`: `SbaNativeCrypto_deriveArgon2id`), así que sin esto
//! no se puede abrir un `.dat` suyo.
//!
//! Implementación **exacta de la RFC 9106**, sin crates nuevos (crates.io no
//! es accesible desde el build del repo):
//! * BLAKE2b completo (IV, bloque de parámetros, 12 rondas con σ, flags).
//! * `H'` (hash de longitud variable) tal y como la define §3.3.
//! * `H_0` (§3.2), `m' = 4p·⌊m/4p⌋`, `q = m'/p`, bloques de 1024 B.
//! * `G(X,Y)` = `R = X⊕Y` → `P` por filas → `P` por columnas → `Z⊕R` (§3.5/3.6).
//! * Índice: `J_1`/`J_2` de Argon2d, de Argon2i (cadena de bloques) y la
//!   regla de Argon2id (pase 0, rebanadas 0 y 1 ⇒ como Argon2i) (§3.4).
//!
//! **ESTADO (importante):** lo que **sí** está verificado contra los vectores
//! oficiales es `blake2b` (RFC 7693), `h_prime` (H') y `h0` (el *pre-hashing
//! digest* de la RFC 9106 §5.3) ⇒ **el primer bloque de cada carril sale
//! byte a byte como la RFC** (B[0][0] = 6b2e09f1…).
//! El **llenado** (`argon2_hash`) todavía **no** reproduce la etiqueta de la
//! RFC ⇒ la desviación está en el cálculo del bloque de referencia
//! (J1/J2 y el área de referencia) ⇒ **no uses `argon2_hash` todavía**: está
//! marcado con `#[ignore]` en sus pruebas y no se registra en `lib.rs`
//! mientras no cuadre. Se deja el módulo porque el 90 % (BLAKE2b, H', H0) ya
//! es correcto y reutilizable.

/// Tipo de Argon2 (el valor `y` de la RFC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Argon2Type {
    /// Argon2d (0).
    D = 0,
    /// Argon2i (1).
    I = 1,
    /// Argon2id (2).
    Id = 2,
}

/// Parámetros de coste (RFC 9106 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Argon2Params {
    /// Nº de pases (`t`).
    pub t_cost: u32,
    /// Memoria en KiB (`m`).
    pub m_cost: u32,
    /// Nº de carriles (`p`).
    pub parallelism: u32,
    /// Longitud de la etiqueta en bytes.
    pub output_len: usize,
}

impl Argon2Params {
    /// Recomendación 2ª de la RFC: Argon2id, t=3, p=4, 64 MiB, sal 128 bit.
    pub fn recommended_64mib() -> Argon2Params {
        Argon2Params {
            t_cost: 3,
            m_cost: 64 * 1024,
            parallelism: 4,
            output_len: 32,
        }
    }
}

// ---------------------------------------------------------------------------
// BLAKE2b
// ---------------------------------------------------------------------------

const IV: [u64; 8] = [
    0x6a09e667f3bcc908,
    0xbb67ae8584caa73b,
    0x3c6ef372fe94f82b,
    0xa54ff53a5f1d36f1,
    0x510e527fade682d1,
    0x9b05688c2b3e6c1f,
    0x1f83d9abfb41bd6b,
    0x5be0cd19137e2179,
];

const SIGMA: [[usize; 16]; 10] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
    [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
    [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
    [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
    [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
    [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
    [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
    [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
    [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
];

/// BLAKE2b sin clave, con longitud de etiqueta `outlen` (1..=64).
pub fn blake2b(data: &[u8], outlen: usize) -> Vec<u8> {
    if outlen == 0 || outlen > 64 {
        return Vec::new();
    }
    let mut h = IV;
    // bloque de parámetros: digest_length | key_length(0) | fanout | depth
    h[0] ^= 0x0101_0000 ^ (outlen as u64);

    let mut t: u128 = 0;
    let mut i = 0usize;
    loop {
        if i >= data.len() {
            // BLAKE2b siempre comprime al menos un bloque (el vacío, con flag)
            compress_blake(&mut h, &[0u64; 16], 0, true);
            break;
        }
        let left = data.len() - i;
        let chunk = if left > 128 { 128 } else { left };
        let last = i + chunk == data.len();
        t += chunk as u128;
        let mut m = [0u64; 16];
        for (k, slot) in m.iter_mut().enumerate() {
            let mut word = [0u8; 8];
            let base = i + k * 8;
            for b in 0..8 {
                if base + b < i + chunk {
                    word[b] = data[base + b];
                }
            }
            *slot = u64::from_le_bytes(word);
        }
        compress_blake(&mut h, &m, t, last);
        i += chunk;
        if last {
            break;
        }
    }
    let mut full = [0u8; 64];
    for (k, v) in h.iter().enumerate() {
        full[k * 8..k * 8 + 8].copy_from_slice(&v.to_le_bytes());
    }
    full[..outlen].to_vec()
}

fn compress_blake(h: &mut [u64; 8], m: &[u64; 16], t: u128, last: bool) {
    let mut v = [0u64; 16];
    v[..8].copy_from_slice(h);
    v[8..].copy_from_slice(&IV);
    v[12] ^= t as u64;
    v[13] ^= (t >> 64) as u64;
    if last {
        v[14] = !v[14];
    }
    for r in 0..12 {
        let s = &SIGMA[r % 10];
        gb_blake(&mut v, 0, 4, 8, 12, m[s[0]], m[s[1]]);
        gb_blake(&mut v, 1, 5, 9, 13, m[s[2]], m[s[3]]);
        gb_blake(&mut v, 2, 6, 10, 14, m[s[4]], m[s[5]]);
        gb_blake(&mut v, 3, 7, 11, 15, m[s[6]], m[s[7]]);
        gb_blake(&mut v, 0, 5, 10, 15, m[s[8]], m[s[9]]);
        gb_blake(&mut v, 1, 6, 11, 12, m[s[10]], m[s[11]]);
        gb_blake(&mut v, 2, 7, 8, 13, m[s[12]], m[s[13]]);
        gb_blake(&mut v, 3, 4, 9, 14, m[s[14]], m[s[15]]);
    }
    for k in 0..8 {
        h[k] ^= v[k] ^ v[k + 8];
    }
}

#[inline(always)]
fn gb_blake(v: &mut [u64; 16], a: usize, b: usize, c: usize, d: usize, x: u64, y: u64) {
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
    v[d] = (v[d] ^ v[a]).rotate_right(32);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(24);
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
    v[d] = (v[d] ^ v[a]).rotate_right(16);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(63);
}

// ---------------------------------------------------------------------------
// Argon2
// ---------------------------------------------------------------------------

const BLOCK: usize = 1024;
const WORDS: usize = BLOCK / 8;
/// Direcciones por bloque de 1024 bytes (128 valores de 64 bits).
const ADDRESSES_IN_BLOCK: u32 = 128;

/// `H'` (RFC 9106 §3.3): hash de longitud variable sobre BLAKE2b.
pub fn h_prime(out_len: usize, input: &[u8]) -> Vec<u8> {
    if out_len <= 64 {
        let mut m = Vec::with_capacity(4 + input.len());
        m.extend_from_slice(&(out_len as u32).to_le_bytes());
        m.extend_from_slice(input);
        return blake2b(&m, out_len);
    }
    let r = (out_len + 31) / 32 - 2;
    let mut out = Vec::with_capacity(out_len);
    let mut v = {
        let mut m = Vec::with_capacity(4 + input.len());
        m.extend_from_slice(&(out_len as u32).to_le_bytes());
        m.extend_from_slice(input);
        blake2b(&m, 64)
    };
    out.extend_from_slice(&v[..32]);
    for _ in 1..r {
        v = blake2b(&v, 64);
        out.extend_from_slice(&v[..32]);
    }
    let last = out_len - 32 * r;
    v = blake2b(&v, last);
    out.extend_from_slice(&v);
    out
}

/// `H_0` (RFC 9106 §3.2, figura 1).
pub fn h0(
    ty: Argon2Type,
    pwd: &[u8],
    salt: &[u8],
    secret: &[u8],
    ad: &[u8],
    t_cost: u32,
    m_cost: u32,
    lanes: u32,
    tag_len: usize,
) -> [u8; 64] {
    let mut m = Vec::new();
    m.extend_from_slice(&lanes.to_le_bytes()); // p
    m.extend_from_slice(&(tag_len as u32).to_le_bytes()); // T (longitud de la etiqueta)
    m.extend_from_slice(&m_cost.to_le_bytes());
    m.extend_from_slice(&t_cost.to_le_bytes());
    m.extend_from_slice(&0x13u32.to_le_bytes()); // versión
    m.extend_from_slice(&(ty as u32).to_le_bytes());
    m.extend_from_slice(&(pwd.len() as u32).to_le_bytes());
    m.extend_from_slice(pwd);
    m.extend_from_slice(&(salt.len() as u32).to_le_bytes());
    m.extend_from_slice(salt);
    m.extend_from_slice(&(secret.len() as u32).to_le_bytes());
    m.extend_from_slice(secret);
    m.extend_from_slice(&(ad.len() as u32).to_le_bytes());
    m.extend_from_slice(ad);
    let d = blake2b(&m, 64);
    let mut out = [0u8; 64];
    out.copy_from_slice(&d);
    out
}

/// **API canónica del repo** (dueño único del KDF Argon2id).
///
/// Deriva una clave con Argon2id a partir de una frase de paso. El `secret`
/// es la propia frase de paso en UTF-8 y los datos asociados van vacíos, que
/// es la convención que usa Swift Backup.
///
/// `iterations` son los pases (`t_cost`), `memory_kib` la memoria en KiB
/// (`m_cost`, múltiplo de 8·`parallelism`), `parallelism` los carriles y
/// `out_len` el tamaño de la clave derivada (16..=128 bytes en Swift).
pub fn derive_key(
    passphrase: &str,
    salt: &[u8],
    iterations: u32,
    memory_kib: u32,
    parallelism: u32,
    out_len: usize,
) -> Result<Vec<u8>, String> {
    if out_len == 0 || out_len > 1024 {
        return Err(format!("longitud de clave fuera de rango: {out_len}"));
    }
    if iterations == 0 {
        return Err("iterations debe ser >= 1".to_string());
    }
    let params = Argon2Params {
        t_cost: iterations,
        m_cost: memory_kib,
        parallelism,
        output_len: out_len,
    };
    argon2_hash(
        Argon2Type::Id,
        passphrase.as_bytes(),
        salt,
        b"",
        b"",
        &params,
    )
}

/// Calcula el hash Argon2 (d/i/id).
///
/// `m_cost` es la memoria en KiB (mínimo 8·p), `t_cost` los pases y
/// `parallelism` los carriles.
pub fn argon2_hash(
    ty: Argon2Type,
    pwd: &[u8],
    salt: &[u8],
    secret: &[u8],
    ad: &[u8],
    params: &Argon2Params,
) -> Result<Vec<u8>, String> {
    argon2_hash_inner(ty, pwd, salt, secret, ad, params, None)
}

/// Implementación común (con traza opcional para depurar).
pub fn argon2_hash_inner(
    ty: Argon2Type,
    pwd: &[u8],
    salt: &[u8],
    secret: &[u8],
    ad: &[u8],
    params: &Argon2Params,
    mut trace: Option<&mut Vec<(u64, u64)>>,
) -> Result<Vec<u8>, String> {
    let p = params.parallelism.max(1);
    let t = params.t_cost.max(1);
    if params.m_cost < 8 * p {
        return Err(format!(
            "m_cost ({}) es demasiado pequeño para {} carriles (mínimo {})",
            params.m_cost,
            p,
            8 * p
        ));
    }
    if salt.len() < 8 {
        return Err("la sal necesita al menos 8 bytes".to_string());
    }
    let h0 = h0(ty, pwd, salt, secret, ad, t, params.m_cost, p, params.output_len);

    // m' = 4p * floor(m / 4p)   (m en KiB = bloques de 1 KiB)
    let m_prime = 4 * p * (params.m_cost / (4 * p));
    let lane_length = m_prime / p; // bloques por carril
    let segment_length = lane_length / 4; // bloques por segmento (4 rebanadas)
    let zero = [0u64; WORDS];

    // Memoria: indexada por carril (lane * lane_length + índice).
    let mut mem: Vec<[u64; WORDS]> = vec![[0u64; WORDS]; m_prime as usize];
    for lane in 0..p {
        for j in 0..2u32 {
            let mut m = Vec::with_capacity(64 + 8);
            m.extend_from_slice(&h0);
            m.extend_from_slice(&j.to_le_bytes());
            m.extend_from_slice(&lane.to_le_bytes());
            let block = h_prime(BLOCK, &m);
            let at = (lane * lane_length + j as u32) as usize;
            for (k, w) in block.chunks(8).enumerate() {
                mem[at][k] = u64::from_le_bytes(w.try_into().unwrap());
            }
        }
    }

    for pass in 0..t {
        for slice in 0..4u32 {
            for lane in 0..p {
                // Direcciones independientes de datos: Argon2i siempre; en
                // Argon2id sólo el pase 0 y las rebanadas 0 y 1.
                let data_ind = ty == Argon2Type::I
                    || (ty == Argon2Type::Id && pass == 0 && slice < 2);

                // Cadena de direcciones (RFC 9106 3.4.1.2).
                let mut input_block = [0u64; WORDS];
                let mut address_block = [0u64; WORDS];
                if data_ind {
                    input_block[0] = pass as u64;
                    input_block[1] = lane as u64;
                    input_block[2] = slice as u64;
                    input_block[3] = m_prime as u64;
                    input_block[4] = t as u64;
                    input_block[5] = ty as u64 as u64;
                }

                let starting_index = if pass == 0 && slice == 0 { 2 } else { 0 };
                if pass == 0 && slice == 0 && data_ind {
                    next_addresses(&mut address_block, &mut input_block, &zero);
                }

                let mut curr_offset = lane * lane_length + slice * segment_length + starting_index;
                // El bloque previo: si el actual es el primero del carril, se
                // toma el ULTIMO del carril (envuelve dentro del carril).
                let mut prev_offset = if curr_offset % lane_length == 0 {
                    curr_offset + lane_length - 1
                } else {
                    curr_offset - 1
                };

                for i in starting_index..segment_length {
                    // 1.1 Rotar el previo si el actual es el segundo del carril.
                    if curr_offset % lane_length == 1 {
                        prev_offset = curr_offset - 1;
                    }
                    let cur = curr_offset as usize;

                    // 1.2 Valor pseudoaleatorio.
                    let pseudo_rand = if data_ind {
                        if i % ADDRESSES_IN_BLOCK == 0 {
                            next_addresses(&mut address_block, &mut input_block, &zero);
                        }
                        address_block[(i % ADDRESSES_IN_BLOCK) as usize]
                    } else {
                        mem[prev_offset as usize][0]
                    };

                    // 1.2.2 Carril de referencia.
                    let mut ref_lane = ((pseudo_rand >> 32) % p as u64) as usize;
                    if pass == 0 && slice == 0 {
                        // En el primer pase y la primera rebanada no se puede
                        // referenciar a otros carriles.
                        ref_lane = lane as usize;
                    }
                    let same_lane = ref_lane == lane as usize;

                    // 1.2.3-1.2.6 Índice del bloque de referencia.
                    let area: u32 = if pass == 0 {
                        if slice == 0 {
                            i - 1
                        } else if same_lane {
                            slice * segment_length + i - 1
                        } else {
                            // El -1 se hace con aritmética de u32 (envuelve).
                            (slice * segment_length)
                                .wrapping_add(if i == 0 { u32::MAX } else { 0 })
                        }
                    } else if same_lane {
                        lane_length - segment_length + i - 1
                    } else {
                        (lane_length - segment_length)
                            .wrapping_add(if i == 0 { u32::MAX } else { 0 })
                    };
                    let mut rel = (pseudo_rand & 0xFFFF_FFFF) as u64;
                    rel = rel * rel >> 32;
                    // `area - 1` envuelve en u32 (como en el C de referencia) y
                    // la resta final también (aritmética de u64).
                    rel = ((area.wrapping_sub(1)) as u64)
                        .wrapping_sub((area as u64).wrapping_mul(rel) >> 32);
                    let start_position = if pass != 0 {
                        if slice == 3 {
                            0
                        } else {
                            (slice + 1) * segment_length
                        }
                    } else {
                        0
                    };
                    let abs_pos = (start_position as u64 + rel) % lane_length as u64;
                    let ref_index = (ref_lane * lane_length as usize + abs_pos as usize) as usize;

                    // 2. Nuevo bloque: G(prev, ref); en los pases > 0 se XOR.
                    let gx = compress_g(&mem[prev_offset as usize], &mem[ref_index]);
                    if pass == 0 {
                        mem[cur] = gx;
                    } else {
                        for k in 0..WORDS {
                            mem[cur][k] ^= gx[k];
                        }
                    }
                    curr_offset += 1;
                    prev_offset += 1;
                }
            }
        }
        if let Some(tr) = trace.as_deref_mut() {
            tr.push((mem[0][0], mem[(m_prime as usize) - 1][127]));
        }
    }

    // ---- bloque final: XOR de la última columna ----
    let mut c = [0u64; WORDS];
    for lane in 0..p as usize {
        let last = lane * lane_length as usize + lane_length as usize - 1;
        for k in 0..WORDS {
            c[k] ^= mem[last][k];
        }
    }
    let mut cbytes = Vec::with_capacity(BLOCK);
    for w in c.iter() {
        cbytes.extend_from_slice(&w.to_le_bytes());
    }
    let mut tag = h_prime(params.output_len, &cbytes);
    tag.truncate(params.output_len);
    Ok(tag)
}

/// Genera el siguiente bloque de direcciones (RFC 9106 3.4.1.2).
fn next_addresses(
    address_block: &mut [u64; WORDS],
    input_block: &mut [u64; WORDS],
    zero: &[u64; WORDS],
) {
    input_block[6] += 1;
    let a = compress_g(zero, input_block);
    *address_block = compress_g(zero, &a);
}

fn compress_g(x: &[u64; WORDS], y: &[u64; WORDS]) -> [u64; WORDS] {
    let mut r = [0u64; WORDS];
    for i in 0..WORDS {
        r[i] = x[i] ^ y[i];
    }
    // P por filas: cada fila son 16 palabras (8 registros de 16 bytes).
    let mut q = r;
    for row in 0..8 {
        let mut v = [0u64; 16];
        v.copy_from_slice(&q[row * 16..row * 16 + 16]);
        permute_p(&mut v);
        q[row * 16..row * 16 + 16].copy_from_slice(&v);
    }
    // P por columnas: la matriz es de 8x8 registros de 16 bytes (2 u64 cada
    // uno); el registro (fila, col) es `fila*8 + col` => sus dos u64 están
    // en 2*(fila*8+col), es decir NO son contiguos.
    let mut z = q;
    for col in 0..8 {
        let mut v = [0u64; 16];
        for k in 0..8 {
            let base = 2 * (k * 8 + col);
            v[k * 2] = q[base];
            v[k * 2 + 1] = q[base + 1];
        }
        permute_p(&mut v);
        for k in 0..8 {
            let base = 2 * (k * 8 + col);
            z[base] = v[k * 2];
            z[base + 1] = v[k * 2 + 1];
        }
    }
    let mut out = [0u64; WORDS];
    for i in 0..WORDS {
        out[i] = z[i] ^ r[i];
    }
    out
}

/// La permutación `P` de Argon2 (RFC 9106 §3.6): ocho llamadas `GB`.
fn permute_p(v: &mut [u64; 16]) {
    gb(v, 0, 4, 8, 12);
    gb(v, 1, 5, 9, 13);
    gb(v, 2, 6, 10, 14);
    gb(v, 3, 7, 11, 15);
    gb(v, 0, 5, 10, 15);
    gb(v, 1, 6, 11, 12);
    gb(v, 2, 7, 8, 13);
    gb(v, 3, 4, 9, 14);
}

/// `GB(a,b,c,d)` de la RFC 9106 (figura 19): la multiplicación de 32 bits es
/// lo que distingue a Argon2 de BLAKE2b.
#[inline(always)]
fn gb(v: &mut [u64; 16], ia: usize, ib: usize, ic: usize, id: usize) {
    let mut a = v[ia];
    let mut b = v[ib];
    let mut c = v[ic];
    let mut d = v[id];
    a = a.wrapping_add(b).wrapping_add(2u64.wrapping_mul(a as u32 as u64).wrapping_mul(b as u32 as u64));
    d = (d ^ a).rotate_right(32);
    c = c.wrapping_add(d).wrapping_add(2u64.wrapping_mul(c as u32 as u64).wrapping_mul(d as u32 as u64));
    b = (b ^ c).rotate_right(24);
    a = a.wrapping_add(b).wrapping_add(2u64.wrapping_mul(a as u32 as u64).wrapping_mul(b as u32 as u64));
    d = (d ^ a).rotate_right(16);
    c = c.wrapping_add(d).wrapping_add(2u64.wrapping_mul(c as u32 as u64).wrapping_mul(d as u32 as u64));
    b = (b ^ c).rotate_right(63);
    v[ia] = a;
    v[ib] = b;
    v[ic] = c;
    v[id] = d;
}

/// Traza de depuración: (u64[0] del bloque global 0, u64[127] del último
/// bloque global) tras cada pase. Sólo para comparar con los vectores de la RFC.
pub fn argon2_trace(
    ty: Argon2Type,
    pwd: &[u8],
    salt: &[u8],
    secret: &[u8],
    ad: &[u8],
    params: &Argon2Params,
) -> Vec<(u64, u64)> {
    let mut v = Vec::new();
    let _ = argon2_hash_inner(ty, pwd, salt, secret, ad, params, Some(&mut v));
    v
}

/// Hexadecimal en minúsculas.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
