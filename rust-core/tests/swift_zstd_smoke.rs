//! Arnés de pruebas de ZSTD (FFI por dlopen) y del contenedor SBA1 de Swift.
//!
//! El frame ZSTD del test lo generó `libzstd` del sistema (nivel 3) ⇒ el
//! productor del fixture es **independiente** del código que lo descomprime.

use megadaemon_core::zstd;
use megadaemon_core::swift_sba1;

use std::path::Path;

/// Frame ZSTD real (producido por libzstd 1.5.4, nivel 3) del texto plano
/// "contenido de prueba para el frame zstd del contenedor SBA1\n" × 40.
const FRAME_ZSTD_HEX: &str = "28b52ffd6038080d02005403636f6e74656e69646f20646520707275656261207061726120656c206672616d65207a7374642064656c2065646f7220534241310a0200faf0caa3394c9501";

fn h(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn texto_plano() -> Vec<u8> {
    "contenido de prueba para el frame zstd del contenedor SBA1\n"
        .repeat(40)
        .into_bytes()
}

// ---------------------------------------------------------------------------
// zstd
// ---------------------------------------------------------------------------

#[test]
fn el_frame_zstd_congelado_se_descomprime_exacto() {
    let frame = h(FRAME_ZSTD_HEX);
    let plano = zstd::descomprimir(&frame, 1 << 20).expect("debe descomprimirse");
    assert_eq!(plano, texto_plano());
    assert_eq!(plano.len(), 2360);
    assert_eq!(frame.len(), 75, "el fixture debe seguir siendo el mismo");
}

#[test]
fn el_tamano_de_contenido_se_conoce_sin_descomprimir() {
    let frame = h(FRAME_ZSTD_HEX);
    if zstd::disponible() {
        assert_eq!(zstd::tamano_contenido(&frame), Some(2360));
    } else {
        // Sin libzstd no se puede consultar; el test no debe fallar por ello.
        assert_eq!(zstd::tamano_contenido(&frame), None);
    }
}

#[test]
fn comprimir_y_descomprimir_es_id_y_tope() {
    if !zstd::disponible() {
        eprintln!("libzstd no está en esta máquina: prueba omitida");
        return;
    }
    let original = texto_plano();
    let comprimido = zstd::comprimir(&original, 3).unwrap();
    assert!(comprimido.len() < original.len());
    assert_eq!(zstd::descomprimir(&comprimido, 1 << 20).unwrap(), original);
}

#[test]
fn los_datos_invalidos_dan_error_y_no_panic() {
    assert!(zstd::descomprimir(&[], 1024).is_err(), "entrada vacía");
    assert!(zstd::descomprimir(&[0xff, 0xff, 0xff, 0xff], 1024).is_err());
    assert!(zstd::descomprimir(b"no soy zstd", 1024).is_err());
}

#[test]
fn el_maximo_de_salida_se_respeta() {
    let frame = h(FRAME_ZSTD_HEX);
    // 100 bytes no bastan para 2360: el error debe ser explícito, no un
    // desbordamiento silencioso.
    let r = zstd::descomprimir(&frame, 100);
    match r {
        Ok(v) => assert!(v.len() <= 100, "no debe devolver más de lo permitido"),
        Err(e) => assert!(!e.is_empty()),
    }
}

// ---------------------------------------------------------------------------
// SBA1
// ---------------------------------------------------------------------------

/// Construye un contenedor SBA1 sintético: cabecera de 96 bytes con el magic,
/// seguida de un TAR comprimido con ZSTD.
fn contenedor_sba1(entrada_tar: &[u8]) -> Vec<u8> {
    let mut cab = vec![0u8; 96];
    cab[0..4].copy_from_slice(b"SBA1");
    // Tamaño de cabecera declarado en big-endian (offset 4, u16).
    cab[4..6].copy_from_slice(&96u16.to_be_bytes());
    // Bytes "mágicos" que no importan para el lector, pero que deben
    // sobrevivir intactos en el informe.
    cab[16..20].copy_from_slice(&4u32.to_be_bytes()); // kdfMethodId = Aegis256
    cab[20..24].copy_from_slice(&65536u32.to_be_bytes()); // payloadChunkSize
    let comprimido = zstd::comprimir(entrada_tar, 3).unwrap();
    cab.extend_from_slice(&comprimido);
    cab
}

/// TAR mínimo con un fichero.
fn tar_minimo() -> Vec<u8> {
    let mut t = Vec::new();
    let nombre = b"hola.txt";
    let contenido = b"contenido de hola";
    // cabecera ustar
    let mut h = [0u8; 512];
    h[..nombre.len()].copy_from_slice(nombre);
    h[100..108].copy_from_slice(b"0000644\0");
    h[108..116].copy_from_slice(b"0000000\0");
    h[116..124].copy_from_slice(b"0000000\0");
    h[124..136].copy_from_slice(format!("{:011o}\0", contenido.len()).as_bytes());
    h[136..148].copy_from_slice(format!("{:011o}\0", 0).as_bytes());
    h[148..156].copy_from_slice(b"        ");
    h[156..157].copy_from_slice(b"0");
    for (i, b) in h.iter_mut().enumerate() {
        if *b == 0 && i >= 148 {
            *b = b' ';
        }
    }
    let suma: u32 = h.iter().map(|b| *b as u32).sum();
    h[148..156].copy_from_slice(format!("{suma:06o}\0 ").as_bytes());
    t.extend_from_slice(&h);
    t.extend_from_slice(contenido);
    let relleno = [0u8; 512];
    t.extend_from_slice(&relleno[..(512 - contenido.len() % 512) % 512]);
    // dos bloques de ceros = fin
    t.extend_from_slice(&[0u8; 1024]);
    t
}

#[test]
fn se_reconoce_el_magic_sba1() {
    let c = contenedor_sba1(&tar_minimo());
    assert!(swift_sba1::es_sba1(&c));
    let cab = swift_sba1::leer_cabecera(&c).unwrap();
    assert_eq!(cab.magic, "SBA1");
    assert_eq!(cab.tamano, 96);
    assert_eq!(cab.crudo.len(), 96);
}

#[test]
fn un_fichero_que_no_es_sba1_da_error() {
    let err = swift_sba1::leer_cabecera(b"PK\x03\x04 lo que sea").unwrap_err();
    assert!(err.contains("magic"), "el error debe nombrar el problema: {err}");
    let err = swift_sba1::leer_cabecera(b"SBA").unwrap_err();
    assert!(!err.is_empty());
}

#[test]
fn el_payload_tar_zstd_se_lee_como_inventario() {
    let c = contenedor_sba1(&tar_minimo());
    let entradas = swift_sba1::leer_tar_zstd(&c, 1 << 20).expect("debe leer el TAR");
    assert_eq!(entradas.len(), 1);
    assert_eq!(entradas[0].nombre, "hola.txt");
    assert_eq!(entradas[0].datos, b"contenido de hola");
}

#[test]
fn el_contenedor_se_extrae_a_disco() {
    let c = contenedor_sba1(&tar_minimo());
    let dir = std::env::temp_dir().join(format!("sba1-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let escritos = swift_sba1::extraer_tar_zstd(&c, &dir, 1 << 20).unwrap();
    assert_eq!(escritos, vec!["hola.txt".to_string()]);
    let leido = std::fs::read(dir.join("hola.txt")).unwrap();
    assert_eq!(leido, b"contenido de hola");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn el_informe_muestra_el_magic_y_las_candidatas() {
    let c = contenedor_sba1(&tar_minimo());
    let inf = swift_sba1::informe_contenedor(&c).unwrap();
    assert!(inf.contains("SBA1"));
    assert!(inf.contains("96"));
    // Las candidatas deben incluir los valores que plantamos (kdfMethodId=4,
    // payloadChunkSize=65536) para que sirvan de punto de partida.
    assert!(inf.contains("65536"), "informe:\n{inf}");
}

#[test]
fn las_etiquetas_de_dominio_son_las_medidas() {
    assert_eq!(
        swift_sba1::ETIQUETA_AAD,
        "SBA1-AEGIS256-entry-chunk-aad-v1"
    );
    assert_eq!(
        swift_sba1::ETIQUETA_NONCE,
        "SBA1-AEGIS256-entry-chunk-nonce-v1"
    );
    assert_eq!(swift_sba1::KDF_ARGON2ID, 1);
    assert_eq!(swift_sba1::KDF_AEGIS256, 4);
    assert_eq!(swift_sba1::TAMANOS_CABECERA, &[96, 144]);
}

#[test]
fn una_cabecera_truncada_da_error_claro() {
    let c = contenedor_sba1(&tar_minimo());
    // Se recorta el payload: la cabecera se lee, el payload no.
    let solo_cabecera = &c[..96];
    let cab = swift_sba1::leer_cabecera(solo_cabecera).unwrap();
    assert_eq!(cab.tamano, 96);
    assert!(swift_sba1::descomprimir_payload(solo_cabecera, 1 << 20).is_err());
    assert!(swift_sba1::localizar_payload(solo_cabecera, 96).is_none());
}

#[test]
fn el_lector_de_tar_rechaza_rutas_maliciosas() {
    // El extractor usa `safe_relative_path`: una ruta con ".." no debe
    // escapar del directorio destino.
    let mut t = Vec::new();
    let nombre = b"../escapado.txt";
    let contenido = b"x";
    let mut h = [0u8; 512];
    h[..nombre.len()].copy_from_slice(nombre);
    h[100..108].copy_from_slice(b"0000644\0");
    h[108..116].copy_from_slice(b"0000000\0");
    h[116..124].copy_from_slice(b"0000000\0");
    h[124..136].copy_from_slice(format!("{:011o}\0", contenido.len()).as_bytes());
    h[136..148].copy_from_slice(format!("{:011o}\0", 0).as_bytes());
    for b in h.iter_mut().skip(148) {
        *b = b' ';
    }
    let suma: u32 = h.iter().map(|b| *b as u32).sum();
    h[148..156].copy_from_slice(format!("{suma:06o}\0 ").as_bytes());
    t.extend_from_slice(&h);
    t.extend_from_slice(contenido);
    t.extend_from_slice(&[0u8; 1024]);
    let c = contenedor_sba1(&t);
    let dir = std::env::temp_dir().join(format!("sba1-evil-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let r = swift_sba1::extraer_tar_zstd(&c, &dir, 1 << 20);
    assert!(r.is_err(), "una ruta con .. debe rechazarse");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn el_lector_de_tar_propio_sigue_disponible() {
    // El módulo se apoya en `backup_formats`: si ese módulo desaparece, el
    // guard-fuentes del repo lo detecta; aquí comprobamos que la API sigue.
    assert!(swift_sba1::sanear_ruta("a/b.txt").is_ok());
    assert!(Path::new("/tmp").exists());
}
