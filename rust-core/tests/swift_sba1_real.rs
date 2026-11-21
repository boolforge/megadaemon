//! Vector REAL de la cabecera `SBA1` de Swift Backup.
//!
//! ORIGEN (medido el 2026-11-21): cabecera de 144 B descargada de Google
//! Drive con una petición por rango (HTTP 206), del fichero
//!
//!   io.github.forkmaintainers.iceraven.dat (SM-G998B) (id-20261120-062807-RY)
//!
//! de la carpeta de Swift Backup del usuario.  **No** es un fichero de test
//! inventado: son los bytes reales del primer bloque de un backup de 2,35 GB.
//!
//! Lo que este test congela:
//!   * el magic `SBA1`;
//!   * `formato = 2` ⇒ cabecera de **144** bytes (96 en el formato corto);
//!   * big-endian en todos los campos;
//!   * `compression = 3`;
//!   * `createdAtEpochMs` = 1795152489000 = 2026-11-20T05:28:09Z (el `id`
//!     del nombre del fichero dice 20261120-062807: la misma fecha en hora
//!     de España, UTC+1);
//!   * tres bloques de 16 B (salt, nonce2, key-check) en 36, 52 y 68;
//!   * `chunkSize = 1048576` (1 MiB);
//!   * `argon2MemoryKib = 16384`;
//!   * `keyLength = 32`;
//!   * `payloadHmac` de 32 B no nulos en 104;
//!   * los últimos 8 B a cero.

use megadaemon_core::swift_archive::{sba1_detectar, sba1_layout};

/// Los 144 bytes reales de la cabecera (hex, en líneas de 32 bytes).
const CABECERA_REAL_HEX: &str = concat!(
    "534241310002009000000003000001a1f76d2a28000100010004000100030010",
    "0010001070b5bde9b5b3705b3b2a1f1a0001fea1784c7cd73cd583169670f6db",
    "9735a1e97da85d053170a461002bda90b781225d000000000010000000004000",
    "0001000100200001846c1c36f1fe4702fca7ff6a95666d3949dde7481d16a266",
    "583d6e70cf9b67530000000000000000"
);

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn cabecera_real() -> Vec<u8> {
    hex(CABECERA_REAL_HEX)
}

/// Epoch (ms) -> ISO-8601 UTC, sin dependencias externas.
///
/// Algoritmo de Howard Hinnant (`civil_from_days`): los dias desde la
/// epoca se convierten a fecha civil con la regla de los años de 400 años.
fn iso_utc(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    let dias = secs.div_euclid(86_400);
    let sod = secs.rem_euclid(86_400);
    let z = dias + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        sod / 3600,
        (sod % 3600) / 60,
        sod % 60
    )
}

#[test]
fn la_cabecera_real_mide_144_bytes_y_formato_2() {
    let b = cabecera_real();
    assert_eq!(b.len(), 144, "la cabecera real ocupa 144 bytes");
    assert_eq!(&b[..4], b"SBA1");
    assert_eq!(u16::from_be_bytes([b[4], b[5]]), 2, "formato");
    assert_eq!(u16::from_be_bytes([b[6], b[7]]), 144, "headerSize");
    assert_eq!(
        u32::from_be_bytes([b[8], b[9], b[10], b[11]]),
        3,
        "compression"
    );
}

#[test]
fn la_fecha_de_creacion_coincide_con_el_id_del_fichero() {
    let b = cabecera_real();
    let ts = u64::from_be_bytes([b[12], b[13], b[14], b[15], b[16], b[17], b[18], b[19]]);
    assert_eq!(ts, 1_795_152_489_000);
    // 2026-11-20T05:28:09Z  ==  2026-11-20 06:28:07 en España (UTC+1),
    // que es exactamente el "id-20261120-062807" del nombre del fichero.
    assert_eq!(iso_utc(ts), "2026-11-20T05:28:09Z");
}

#[test]
fn los_tres_bloques_de_16_bytes_estan_esperados() {
    let b = cabecera_real();
    // longitudes (u16 BE) en 30, 32 y 34
    for off in [30usize, 32, 34] {
        assert_eq!(u16::from_be_bytes([b[off], b[off + 1]]), 16);
    }
    // bloques en 36, 52 y 68
    let salt = &b[36..52];
    let nonce2 = &b[52..68];
    let key_check = &b[68..84];
    assert_eq!(salt.len(), 16);
    assert_eq!(nonce2.len(), 16);
    assert_eq!(key_check.len(), 16);
    // no son todo ceros (o no habria cifrado que verificar)
    assert!(salt.iter().any(|x| *x != 0), "el salt no puede ser todo ceros");
    assert!(nonce2.iter().any(|x| *x != 0), "el nonce no puede ser todo ceros");
    assert!(
        key_check.iter().any(|x| *x != 0),
        "el key-check no puede ser todo ceros"
    );
    // y los tres bloques son DISTINTOS entre si
    assert_ne!(salt, nonce2);
    assert_ne!(salt, key_check);
    assert_ne!(nonce2, key_check);
}

#[test]
fn los_parametros_del_kdf_y_del_trozo_son_los_medidos() {
    let b = cabecera_real();
    // chunkSize @88 (u32 BE) = 1 MiB
    assert_eq!(
        u32::from_be_bytes([b[88], b[89], b[90], b[91]]),
        1_048_576,
        "el tamaño del trozo es 1 MiB"
    );
    // argon2MemoryKib @92 (u32 BE) = 16384 KiB
    assert_eq!(
        u32::from_be_bytes([b[92], b[93], b[94], b[95]]),
        16_384,
        "la memoria de Argon2id son 16384 KiB"
    );
    // keyLength @100 (u16 BE) = 32
    assert_eq!(u16::from_be_bytes([b[100], b[101]]), 32);
    // payloadHmac @104 (32 B): no nulo
    assert!(
        b[104..136].iter().any(|x| *x != 0),
        "el HMAC del payload no puede ser todo ceros"
    );
    // reservado @136 (8 B) = 0
    assert_eq!(&b[136..144], &[0u8; 8], "los ultimos 8 bytes son cero");
}

#[test]
fn el_parser_reconoce_la_cabecera_real() {
    let b = cabecera_real();
    let layout = sba1_detectar(&b).expect("debe detectar el formato");
    assert!(layout.complete());
    assert_eq!(layout.header_size, Some(144));

    // los nombres con significado medido
    let nombres: Vec<&str> = layout.fields.iter().map(|f| f.name.as_str()).collect();
    assert!(nombres.contains(&"chunkSize"), "falta chunkSize: {nombres:?}");
    assert!(nombres.contains(&"argon2MemoryKib"), "falta argon2MemoryKib");
    assert!(nombres.contains(&"keyLength"), "falta keyLength");
    assert!(nombres.contains(&"payloadHmac"), "falta payloadHmac");

    // el layout del formato 2 es el mismo que el medido en el DEX
    let l2 = sba1_layout(2);
    assert_eq!(l2.header_size, Some(144));
    let l1 = sba1_layout(1);
    assert_eq!(l1.header_size, Some(96));
}
