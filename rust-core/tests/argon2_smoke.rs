//! Arnés de pruebas de Argon2 (RFC 9106) contra los vectores oficiales.

#[path = "../src/argon2.rs"]
mod argon2;

use argon2::*;

fn h(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn params() -> Argon2Params {
    Argon2Params {
        t_cost: 3,
        m_cost: 32,
        parallelism: 4,
        output_len: 32,
    }
}

#[test]
fn blake2b_coincide_con_el_vector_de_rfc7693() {
    // BLAKE2b-512("abc")
    let got = hex(&blake2b(b"abc", 64));
    assert_eq!(
        got,
        "ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d17d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923"
    );
    // el hash vacío también es conocido
    assert_eq!(
        hex(&blake2b(b"", 64)),
        "786a02f742015903c6c6fd852552d272912f4740e15847618a86e217f71f5419d25e1031afee585313896444934eb04b903a685b1448b755d56f701afe9be2ce"
    );
}

#[test]
fn el_h0_coincide_con_el_pre_hash_de_la_rfc() {
    let pwd = vec![0x01u8; 32];
    let salt = vec![0x02u8; 16];
    let secret = vec![0x03u8; 8];
    let ad = vec![0x04u8; 12];
    let h0 = h0(Argon2Type::Id, &pwd, &salt, &secret, &ad, 3, 32, 4, 32);
    assert_eq!(
        hex(&h0),
        "2889de487eb42ae500c0007ed9252f1069eadec40d5765b485de6dc2437a67b8546a2f0acc1a0882db8fcf74714b472e94df421a5da1112ffa11434370a1e997"
    );
}

#[test]
fn argon2d_coincide_con_el_vector_de_la_rfc() {
    let pwd = vec![0x01u8; 32];
    let salt = vec![0x02u8; 16];
    let secret = vec![0x03u8; 8];
    let ad = vec![0x04u8; 12];
    let tag = argon2_hash(Argon2Type::D, &pwd, &salt, &secret, &ad, &params()).unwrap();
    assert_eq!(
        hex(&tag),
        "512b391b6f1162975371d30919734294f868e3be3984f3c1a13a4db9fabe4acb"
    );
}

#[test]
fn argon2i_coincide_con_el_vector_de_la_rfc() {
    let pwd = vec![0x01u8; 32];
    let salt = vec![0x02u8; 16];
    let secret = vec![0x03u8; 8];
    let ad = vec![0x04u8; 12];
    let tag = argon2_hash(Argon2Type::I, &pwd, &salt, &secret, &ad, &params()).unwrap();
    assert_eq!(
        hex(&tag),
        "c814d9d1dc7f37aa13f0d77f2494bda1c8de6b016dd388d29952a4c4672b6ce8"
    );
}

#[test]
fn argon2id_coincide_con_el_vector_de_la_rfc() {
    let pwd = vec![0x01u8; 32];
    let salt = vec![0x02u8; 16];
    let secret = vec![0x03u8; 8];
    let ad = vec![0x04u8; 12];
    let tag = argon2_hash(Argon2Type::Id, &pwd, &salt, &secret, &ad, &params()).unwrap();
    assert_eq!(
        hex(&tag),
        "0d640df58d78766c08c037a34a8b53c9d01ef0452d75b65eb52520e96b01e659"
    );
}

#[test]
fn h_prime_es_coherente_con_blake2b() {
    // H'(64)(A) = BLAKE2b-512(LE32(64) || A)
    let a = b"contenido";
    let mut m = Vec::new();
    m.extend_from_slice(&64u32.to_le_bytes());
    m.extend_from_slice(a);
    assert_eq!(hex(&h_prime(64, a)), hex(&blake2b(&m, 64)));
}

#[test]
fn los_parametros_invalidos_se_rechazan() {
    let pwd = b"clave";
    let salt = vec![0x02u8; 16];
    let mala = Argon2Params {
        t_cost: 1,
        m_cost: 4, // menos de 8*p con p=4
        parallelism: 4,
        output_len: 32,
    };
    assert!(argon2_hash(Argon2Type::Id, pwd, &salt, b"", b"", &mala).is_err());
    let corta = vec![0x02u8; 4];
    assert!(argon2_hash(Argon2Type::Id, pwd, &corta, b"", b"", &params()).is_err());
}
