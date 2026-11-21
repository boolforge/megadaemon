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
    trace: Option<&mut Vec<(u64, u64)>>,
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
    let q = m_prime / p; // bloques por carril
    let segment = q / 4;

    let mut mem: Vec<[u64; WORDS]> = vec![[0u64; WORDS]; m_prime as usize];
    for lane in 0..p {
        for j in 0..2u32 {
            let mut m = Vec::with_capacity(64 + 8);
            m.extend_from_slice(&h0);
            m.extend_from_slice(&j.to_le_bytes());
            m.extend_from_slice(&lane.to_le_bytes());
            let block = h_prime(BLOCK, &m);
            let lane_base = (lane * q) as usize;
            for (k, w) in block.chunks(8).enumerate() {
                mem[lane_base + j as usize][k] = u64::from_le_bytes(w.try_into().unwrap());
            }
        }
    }

    let zero = [0u64; WORDS];

    for pass in 0..t {
        for slice in 0..4u32 {
            // Estado de la cadena de direcciones (Argon2i / Argon2id).
            let mut addr_input = [0u64; WORDS];
            let mut addr_block = [0u64; WORDS];
            let mut addr_counter: u64 = 0;

            for lane in 0..p {
                let start_index = if pass == 0 && slice == 0 { 2 } else { 0 };
                for idx in start_index..q {
                    let cur = (lane * q + idx) as usize;
                    let pos_in_seg = idx % segment; // índice DENTRO del segmento

                    // ---- X (bloque previo) y valores pseudoaleatorios ----
                    // pase 0: el bloque recién calculado en idx-1.
                    // pase >0: el valor ANTIGUO de este mismo bloque.
                    // El bloque previo: en la primera posicion de una rebanada
                    // (idx == 0) es el ULTIMO bloque de la rebanada anterior
                    // del mismo carril (referencia de Argon2: prev_offset =
                    // curr_offset - 1 sobre la memoria contigua del carril).
                    let prev_idx = if idx == 0 {
                        (lane * q + q - 1) as usize
                    } else {
                        cur - 1
                    };
                    let (x_block, prand): ([u64; WORDS], [u64; WORDS]) = if pass == 0 {
                        let prev = mem[prev_idx];
                        (prev, prev)
                    } else {
                        let old = mem[cur];
                        (old, old)
                    };
                    let prand_u64 = prand[0];
                    let (j1, j2) = match ty {
                        Argon2Type::D => ((prand_u64 as u32), (prand_u64 >> 32) as u32),
                        Argon2Type::I => {
                            let (a, b) = address_at(
                                &mut addr_input,
                                &mut addr_block,
                                &mut addr_counter,
                                pass,
                                lane,
                                slice,
                                m_prime,
                                t,
                                ty,
                                pos_in_seg,
                                segment,
                                &zero,
                            );
                            (a, b)
                        }
                        Argon2Type::Id => {
                            if pass == 0 && slice <= 1 {
                                let (a, b) = address_at(
                                    &mut addr_input,
                                    &mut addr_block,
                                    &mut addr_counter,
                                    pass,
                                    lane,
                                    slice,
                                    m_prime,
                                    t,
                                    ty,
                                    pos_in_seg,
                                    segment,
                                    &zero,
                                );
                                (a, b)
                            } else {
                                ((prand_u64 as u32), (prand_u64 >> 32) as u32)
                            }
                        }
                    };

                    // ---- bloque de referencia ----
                    // El carril de referencia sale del u64 pseudoaleatorio
                    // completo (implementacion de referencia de Argon2).
                    let ref_lane = ((prand_u64.wrapping_mul(p as u64) >> 32) % p as u64) as usize;
                    let x = (j1 as u64) * (j1 as u64) >> 32;
                    let area = ref_area_size(pass, slice, pos_in_seg, segment, ref_lane, lane);
                    if area <= 0 {
                        // caso degenerado (indice 0 sin area previa)
                        let gx = compress_g(&x_block, &x_block);
                        for k in 0..WORDS {
                            mem[cur][k] = if pass == 0 { gx[k] } else { gx[k] ^ mem[cur][k] };
                        }
                        continue;
                    }
                    let rel = area as u64 - 1 - ((area as u64 * x) >> 32);
                    let start_position = if pass == 0 {
                        0u64
                    } else if slice == 3 {
                        0u64
                    } else {
                        ((slice + 1) * segment) as u64
                    };
                    let abs_position = (start_position + rel) % q as u64;
                    let ref_index = (ref_lane * q as usize + abs_position as usize) as usize;

                    // ---- G(X, Y) ----
                    let gx = compress_g(&x_block, &mem[ref_index]);
                    for k in 0..WORDS {
                        mem[cur][k] = if pass == 0 { gx[k] } else { gx[k] ^ mem[cur][k] };
                    }
                }
            }
        }
    }

    if let Some(tr) = trace { tr.push((mem[0][0], mem[(m_prime as usize) - 1][127])); }

    // ---- bloque final: XOR de la última columna ----
    let mut c = [0u64; WORDS];
    for lane in 0..p as usize {
        let last = lane * q as usize + q as usize - 1;
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

/// Tamaño del área de referencia (formulación de la implementación de
/// referencia de Argon2; `idx` es el índice **dentro del segmento**).
fn ref_area_size(
    pass: u32,
    slice: u32,
    idx: u32,
    segment: u32,
    ref_lane: usize,
    lane: u32,
) -> i64 {
    let same = ref_lane == lane as usize;
    if pass == 0 {
        if slice == 0 {
            return idx as i64 - 1;
        }
        if same {
            return (slice * segment + idx) as i64 - 1;
        }
        return (slice * segment) as i64 + if idx == 0 { -1 } else { 0 };
    }
    if same {
        return (idx + segment * if slice == 3 { 1 } else { 0 }) as i64 - 1;
    }
    (idx + segment * if slice == 3 { 1 } else { 0 }) as i64 + if idx == 0 { -1 } else { 0 }
}

/// Direcciones de Argon2i (RFC 9106 §3.4.1.2): por segmento se genera una
/// cadena de bloques; cada bloque da 128 direcciones (64 pares J1/J2).
#[allow(clippy::too_many_arguments)]
fn address_at(
    addr_input: &mut [u64; WORDS],
    addr_block: &mut [u64; WORDS],
    counter: &mut u64,
    pass: u32,
    lane: u32,
    slice: u32,
    m_prime: u32,
    t: u32,
    ty: Argon2Type,
    pos_in_seg: u32,
    segment: u32,
    zero: &[u64; WORDS],
) -> (u32, u32) {
    // Z = LE64(r)||LE64(l)||LE64(sl)||LE64(m')||LE64(t)||LE64(y)   (48 bytes)
    let z: [u64; 6] = [
        pass as u64,
        lane as u64,
        slice as u64,
        m_prime as u64,
        t as u64,
        ty as u64 as u64,
    ];
    // Cada bloque de direcciones cubre 128 posiciones del segmento.
    let block_in_seg = (pos_in_seg / ADDRESSES_IN_BLOCK) as u64;
    let slot = pos_in_seg % ADDRESSES_IN_BLOCK;
    // (re)genera el bloque de direcciones que toca
    if *counter != block_in_seg + 1 {
        *addr_input = [0u64; WORDS];
        for (i, v) in z.iter().enumerate() {
            addr_input[i] = *v;
        }
        *counter = block_in_seg + 1;
        addr_input[6] = *counter;
        let inner = compress_g(zero, addr_input);
        *addr_block = compress_g(zero, &inner);
    }
    let _ = segment;
    let w = addr_block[slot as usize];
    ((w as u32), (w >> 32) as u32)
}

/// `G(X, Y)` de la RFC 9106 §3.5: `R = X⊕Y`, `P` por filas, `P` por columnas,
/// salida `Z⊕R`.
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
