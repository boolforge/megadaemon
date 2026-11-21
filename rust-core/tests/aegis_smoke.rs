//! Arnés de pruebas de AEGIS-256 contra los vectores de la RFC 10032.

use megadaemon_core::backup_vault;
use megadaemon_core::aegis;

use megadaemon_core::aegis::*;

fn h(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn bloque(s: &str) -> [u8; 16] {
    let v = h(s);
    let mut o = [0u8; 16];
    o.copy_from_slice(&v);
    o
}

#[test]
fn la_ronda_aes_coincide_con_el_vector_de_la_rfc() {
    // RFC 10032, A.1: AESRound(in, rk)
    let entrada = bloque("000102030405060708090a0b0c0d0e0f");
    let rk = bloque("101112131415161718191a1b1c1d1e1f");
    let salida = backup_vault::aes_round(&entrada, &rk);
    assert_eq!(hex(&salida), "7a7b4e5638782546a8c0477a3b813f43");
}

#[test]
fn el_update_coincide_con_el_vector_de_la_rfc() {
    let st = Aegis256::with_state([
        bloque("1fa1207ed76c86f2c4bb40e8b395b43e"),
        bloque("b44c375e6c1e1978db64bcd12e9e332f"),
        bloque("0dab84bfa9f0226432ff630f233d4e5b"),
        bloque("d7ef65c9b93e8ee60c75161407b066e7"),
        bloque("a760bb3da073fbd92bdc24734b1f56fb"),
        bloque("a828a18d6a964497ac6e7e53c5f55c73"),
    ]);
    let mut st = st;
    st.update(&bloque("b165617ed04ab738afb2612c6d18a1ec"));
    assert_eq!(hex(&st.state()[0]), "e6bc643bae82dfa3d991b1b323839dcd");
    assert_eq!(hex(&st.state()[1]), "648578232ba0f2f0a3677f617dc052c3");
    assert_eq!(hex(&st.state()[2]), "ea788e0e572044a46059212dd007a789");
    assert_eq!(hex(&st.state()[3]), "2f1498ae19b80da13fba698f088a8590");
    assert_eq!(hex(&st.state()[4]), "a54c2ee95e8c2a2c3dae2ec743ae6b86");
    assert_eq!(hex(&st.state()[5]), "a3240fceb68e32d5d114df1b5363ab67");
}

#[test]
fn vector_1_de_la_rfc() {
    let key = h("1001000000000000000000000000000000000000000000000000000000000000");
    let nonce = h("1000020000000000000000000000000000000000000000000000000000000000");
    let msg = h("00000000000000000000000000000000");
    let (ct, tag) = encrypt(&key, &nonce, &msg, &[], 16).unwrap();
    assert_eq!(hex(&ct), "754fc3d8c973246dcc6d741412a4b236");
    assert_eq!(hex(&tag), "3fe91994768b332ed7f570a19ec5896e");
    let (ct256, tag256) = encrypt(&key, &nonce, &msg, &[], 32).unwrap();
    assert_eq!(hex(&ct256), "754fc3d8c973246dcc6d741412a4b236");
    assert_eq!(hex(&tag256), "1181a1d18091082bf0266f66297d167d2e68b845f61a3b0527d31fc7b7b89f13");
}

#[test]
fn vector_2_de_la_rfc_mensaje_vacio() {
    let key = h("1001000000000000000000000000000000000000000000000000000000000000");
    let nonce = h("1000020000000000000000000000000000000000000000000000000000000000");
    let (ct, tag) = encrypt(&key, &nonce, &[], &[], 16).unwrap();
    assert!(ct.is_empty());
    assert_eq!(hex(&tag), "e3def978a0f054afd1e761d7553afba3");
}

#[test]
fn vector_3_de_la_rfc_con_datos_asociados() {
    let key = h("1001000000000000000000000000000000000000000000000000000000000000");
    let nonce = h("1000020000000000000000000000000000000000000000000000000000000000");
    let ad = h("0001020304050607");
    let msg = h("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f");
    let (ct, tag) = encrypt(&key, &nonce, &msg, &ad, 16).unwrap();
    assert_eq!(
        hex(&ct),
        "f373079ed84b2709faee373584585d60accd191db310ef5d8b11833df9dec711"
    );
    assert_eq!(hex(&tag), "8d86f91ee606e9ff26a01b64ccbdd91d");
}

#[test]
fn cifrar_y_descifrar_es_coherente() {
    let key = [7u8; 32];
    let nonce = [9u8; 32];
    let msg: Vec<u8> = (0..100u32).map(|i| (i % 251) as u8).collect();
    let ad = b"metadatos";
    let (ct, tag) = encrypt(&key, &nonce, &msg, ad, 16).unwrap();
    assert_eq!(ct.len(), msg.len());
    let back = decrypt(&key, &nonce, &ct, ad, &tag).unwrap();
    assert_eq!(back, msg);
}

#[test]
fn un_cambio_detalecta_el_tag() {
    let key = [7u8; 32];
    let nonce = [9u8; 32];
    let (mut ct, tag) = encrypt(&key, &nonce, b"secreto", b"", 16).unwrap();
    ct[0] ^= 1;
    assert!(decrypt(&key, &nonce, &ct, b"", &tag).is_err());
    // y con datos asociados distintos tambien
    let (ct, tag) = encrypt(&key, &nonce, b"secreto", b"ad1", 16).unwrap();
    assert!(decrypt(&key, &nonce, &ct, b"ad2", &tag).is_err());
}

#[test]
fn las_longitudes_de_clave_y_nonce_se_validan() {
    assert!(encrypt(&[0u8; 16], &[0u8; 32], b"x", b"", 16).is_err());
    assert!(encrypt(&[0u8; 32], &[0u8; 16], b"x", b"", 16).is_err());
    assert!(encrypt(&[0u8; 32], &[0u8; 32], b"x", b"", 16).is_ok());
}
