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

/// Vectores congelados generados con la implementación C de referencia
/// (`phc-winner-argon2`, tag 20190702) para parámetros **fuera** de los de la
/// RFC 9106: son los que usa Swift Backup (salt 16 B, iter 1..100,
/// memKiB 16384..65536, par 1..8, salida 16..128 B).
///
/// Sin esta tabla, un fallo en `index_alpha` o en el área de referencia sólo se
/// vería con los tres vectores de la RFC, que comparten `m_cost = 32`.
#[test]
fn argon2id_coincide_con_el_oraculo_c_en_el_rango_de_swift() {
    // iter 3, 16 MiB, 4 carriles, salida 32
    let k = derive_key(
        "aaaaaaaaaaaaaaaa",
        &h("02020202020202020202020202020202"),
        3,
        16384,
        4,
        32,
    )
    .unwrap();
    assert_eq!(
        hex(&k),
        "c7f25bb92e59b63b36f9c1577069ad17d1ac8a232133008212ffaf56ddba03be"
    );

    // mínimo de Swift: iter 1, 1 carril, salida 16
    let k = derive_key("abcd", &h("02020202020202020202020202020202"), 1, 16384, 1, 16).unwrap();
    assert_eq!(hex(&k), "1f045638d19142ba796b5066c968d919");

    // iter 100, 8 carriles, salt de 16 B no uniforme
    let k = derive_key(
        "passward!",
        &h("0a0b0c0d0e0f1011121314151617"),
        100,
        32768,
        8,
        32,
    )
    .unwrap();
    assert_eq!(
        hex(&k),
        "0a63b6d2c7e48cc6689dc2e544bb142f17cba1b0efc97c946c999e475acb664b"
    );

    // salida de 128 bytes (máximo de Swift)
    let k = derive_key(
        "clave-de prueba",
        &h("11111111111111111111111111111111"),
        2,
        65536,
        2,
        128,
    )
    .unwrap();
    assert_eq!(
        hex(&k),
        "0dd848e5906933fba2c61881d3274163861cf174710eee5c49158e405d1ba1d486352e256af02c13f1476e56001aef00f11973763ee654396b01416d37123791949f2b9e08b0ab2f0119dfdd77054d68ce02cb95e6807a958ad2cd22a2a6797ae9c52a01e4293e654d3705d97ab79fa34803729330ee7e57f94d1b0161b810d9"
    );

    // clave larga (>64 bytes de frase de paso)
    let k = derive_key(
        "metadata-metadata-metadata-metadata",
        &h("aabbccddeeff00112233445566778899"),
        4,
        16384,
        4,
        32,
    )
    .unwrap();
    assert_eq!(
        hex(&k),
        "1164de0f89573ed05d90c4bd09a9d4e5b01d555c71ce385d98e6afc55ed33b11"
    );
}

#[test]
fn derive_key_es_determinista_y_sensible_a_cada_entrada() {
    let salt = h("02020202020202020202020202020202");
    let a = derive_key("clave", &salt, 3, 16384, 2, 32).unwrap();
    let b = derive_key("clave", &salt, 3, 16384, 2, 32).unwrap();
    assert_eq!(a, b, "la derivación debe ser determinista");
    assert_eq!(a.len(), 32);

    let otra_clave = derive_key("clave!", &salt, 3, 16384, 2, 32).unwrap();
    assert_ne!(a, otra_clave, "la frase de paso debe cambiar la clave");
    let otra_sal = derive_key("clave", &h("03030303030303030303030303030303"), 3, 16384, 2, 32).unwrap();
    assert_ne!(a, otra_sal, "la sal debe cambiar la clave");
    let otras_iter = derive_key("clave", &salt, 4, 16384, 2, 32).unwrap();
    assert_ne!(a, otras_iter, "las iteraciones deben cambiar la clave");
    let otra_mem = derive_key("clave", &salt, 3, 32768, 2, 32).unwrap();
    assert_ne!(a, otra_mem, "la memoria debe cambiar la clave");
    let otro_par = derive_key("clave", &salt, 3, 16384, 4, 32).unwrap();
    assert_ne!(a, otro_par, "el paralelismo debe cambiar la clave");
}

#[test]
fn derive_key_rechaza_parametros_invalidos() {
    let salt = h("02020202020202020202020202020202");
    assert!(derive_key("clave", &salt, 0, 16384, 1, 32).is_err(), "iter 0");
    assert!(derive_key("clave", &salt, 3, 16384, 1, 0).is_err(), "salida vacía");
    assert!(derive_key("clave", &salt, 3, 16384, 1, 2048).is_err(), "salida enorme");
    assert!(derive_key("clave", &salt, 3, 4, 1, 32).is_err(), "memoria < 8*p");
    assert!(derive_key("clave", &[0x02u8; 4], 3, 16384, 1, 32).is_err(), "sal corta");
}

#[test]
fn el_rango_de_memoria_de_swift_se_respeta() {
    // Swift usa memKiB 16384..65536: ambos extremos deben derivar sin error.
    let salt = h("02020202020202020202020202020202");
    let bajo = derive_key("clave", &salt, 1, 16384, 1, 32).unwrap();
    let alto = derive_key("clave", &salt, 1, 65536, 1, 32).unwrap();
    assert_eq!(bajo.len(), 32);
    assert_eq!(alto.len(), 32);
    assert_ne!(bajo, alto);
}
