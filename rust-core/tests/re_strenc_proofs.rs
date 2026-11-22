//! Pruebas del descifrado de strings (`re_strenc`).
//!
//! Lo que se prueba: que el motor recupera el texto plano **desde el texto
//! cifrado**, y que la prueba de ida y vuelta **falla** cuando tiene que fallar
//! (es decir, que no se publica como plaintext un descifrado sin probar).

use megadaemon_core::re_strenc::{
    decrypt, detect, encrypt, printable_ratio, prove, DetectOptions, Transform,
};

fn opts() -> DetectOptions {
    DetectOptions {
        min_len: 6,
        max_keys: 8,
        min_printable: 0.9,
        max_bytes: 1 << 20,
    }
}

#[test]
fn la_ida_y_vuelta_es_exacta() {
    let texto = b"password=admin123";
    for t in [
        Transform::XorByte(0x5a),
        Transform::AddByte(0x11),
        Transform::XorRolling(0x07),
        Transform::RolXor { rot: 3, key: 0x22 },
    ] {
        let cifrado = encrypt(texto, &t);
        assert_ne!(cifrado, texto.to_vec());
        let (proof, plano) = prove(&cifrado, &t);
        assert!(proof.round_trip, "{}: {proof:?}", t.label());
        assert!(proof.passed, "{}: {proof:?}", t.label());
        assert_eq!(plano, texto.to_vec());
    }
}

#[test]
fn detecta_un_string_cifrado_con_xor() {
    let texto = b"com.agy.app/MODEL_PLACEHOLDER_M319";
    let cifrado = encrypt(texto, &Transform::XorByte(0x5a));
    // Se mezcla con ruido para que el detector tenga que encontrar la tirada.
    let mut datos = vec![0x00u8; 64];
    datos.extend_from_slice(&cifrado);
    datos.extend_from_slice(&[0x00u8; 32]);

    let r = detect(&datos, &opts());
    let hit = r
        .found
        .iter()
        .find(|s| s.plaintext.contains("MODEL_PLACEHOLDER"))
        .expect("deberia encontrar el string cifrado");
    assert!(hit.plaintext.contains("com.agy.app"));
    assert!(hit.printable_ratio > 0.9);
    assert_eq!(hit.transform, Transform::XorByte(0x5a));
}

#[test]
fn detecta_un_string_cifrado_con_suma() {
    let texto = b"custom_models.json";
    let cifrado = encrypt(texto, &Transform::AddByte(0x33));
    let mut datos = vec![0xffu8; 32];
    datos.extend_from_slice(&cifrado);

    let r = detect(&datos, &opts());
    assert!(
        r.found.iter().any(|s| s.plaintext.contains("custom_models")),
        "no lo encontro: {:?}",
        r.found
    );
}

#[test]
fn una_clave_equivocada_no_recupera_el_texto_original() {
    // LIMITE HONESTO: la prueba certifica que la transformacion es involutiva y
    // que el texto sale valido, pero NO que la clave sea la unica posible
    // (`'a'^1 == 'b'` ⇒ una clave cercana tambien da texto imprimible).  Lo que
    // si se puede exigir es que NO recupere el texto original.
    let texto = b"secreto-de-prueba";
    let cifrado = encrypt(texto, &Transform::XorByte(0x5a));
    let (proof, plano) = prove(&cifrado, &Transform::XorByte(0x5b));
    assert_ne!(plano, texto.to_vec(), "la clave equivocada no puede dar el original");
    // La ida y vuelta si se cumple (XOR es involutiva) ⇒ por eso la prueba
    // declara tambien que no es la unica clave posible.
    assert!(proof.round_trip);
}

#[test]
fn una_clave_lejana_no_pasa_la_prueba() {
    let texto = b"secreto-de-prueba";
    let cifrado = encrypt(texto, &Transform::XorByte(0x5a));
    let (proof, _) = prove(&cifrado, &Transform::XorByte(0x00));
    assert!(!proof.passed, "{proof:?}");
}

#[test]
fn la_proporcion_imprimible_se_mide() {
    assert_eq!(printable_ratio(b"hola mundo"), 1.0);
    assert!(printable_ratio(&[0x00, 0x01, 0x02, 0x03]) < 0.5);
}

#[test]
fn el_bloque_se_corta_y_se_declara() {
    let mut datos = vec![0u8; 5000];
    let o = DetectOptions {
        max_bytes: 1024,
        ..opts()
    };
    let r = detect(&datos, &o);
    assert!(r.truncated);
    assert_eq!(r.scanned_bytes, 1024);
}

#[test]
fn el_descifrado_es_la_inversa_del_cifrado() {
    let texto = b"rot-xor";
    let t = Transform::RolXor { rot: 5, key: 0x99 };
    assert_eq!(decrypt(&encrypt(texto, &t), &t), texto.to_vec());
}
