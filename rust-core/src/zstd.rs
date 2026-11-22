//! MegaDaemon v16 — MÓDULO `zstd` (compresión/descompresión nativa).
//!
//! API **reconstruida** el 2026-11-22: `src/swift_sba1_header.rs` la usaba
//! (`crate::zstd::descomprimir`) y se perdió en un merge, con lo que el
//! módulo de cabecera SBA1 no compilaba y quedaba sin cablear.
//!
//! ## Por qué un módulo propio y no el crate directo
//!
//! El crate `zstd` de crates.io queda **fuera** del binario ARM64 del móvil
//! como dependencia del sistema: `Cargo.toml` lo activa con
//! `default-features = false` + `arrays`, así que se enlaza el **C de zstd
//! embebido** (Android no trae `libzstd`). Este módulo es la **única**
//! puerta de entrada a zstd en la biblioteca:
//!
//! * una sola implementación (nadie llama a `zstd::` suelto);
//! * **tope de salida** en cada descompresión ⇒ una "bomba zstd" no puede
//!   tumbar el móvil (el daemon vive en un móvil con 12 GB y la OOM killer
//!   ya se llevó el túnel una vez);
//! * errores en castellano, como el resto de la biblioteca.
//!
//! ## Compatibilidad con Swift Backup
//!
//! El contenedor `.sba` de Swift Backup es `cabecera SBA1` + payload, y el
//! payload **sin cifrar** es un **TAR comprimido con zstd** (magic
//! `28 B5 2F FD`). Es exactamente lo que este módulo y
//! `crate::swift_sba1_header` recorren: cabecera → zstd → TAR. El resultado es
//! un TAR byte a byte idéntico al que escribió la app ⇒ los ficheros
//! extraídos son los mismos.

use std::io::Read;

/// Magic de un frame zstd (formatoMAGIC-less, nivel estándar).
pub const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];

/// ¿El buffer empieza por un frame zstd?
pub fn es_zstd(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[0..4] == ZSTD_MAGIC
}

/// Comprime con zstd (nivel 1..=22).
///
/// El crate `zstd` enlaza el C embebido: no depende de `libzstd` del sistema.
pub fn comprimir(data: &[u8], nivel: i32) -> Result<Vec<u8>, String> {
    let nivel = nivel.clamp(1, 22);
    ::zstd::bulk::compress(data, nivel).map_err(|e| format!("zstd: fallo al comprimir: {}", e))
}

/// Descomprime un frame zstd con **tope duro** de salida.
///
/// `max_salida` es el máximo de bytes descomprimidos que se aceptan: si el
/// frame declara (o produce) más, se corta con error en vez de reservar
/// gigabytes. Es la protección contra bombas de descompresión.
pub fn descomprimir(data: &[u8], max_salida: usize) -> Result<Vec<u8>, String> {
    let mut dec = ::zstd::stream::read::Decoder::new(std::io::Cursor::new(data))
        .map_err(|e| format!("zstd: no se pudo abrir el frame: {}", e))?;
    let mut out: Vec<u8> = Vec::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = dec
            .read(&mut buf)
            .map_err(|e| format!("zstd: fallo al descomprimir: {}", e))?;
        if n == 0 {
            break;
        }
        if out.len() + n > max_salida {
            return Err(format!(
                "zstd: la salida supera el tope de {} bytes (bomba de descompresion)",
                max_salida
            ));
        }
        out.extend_from_slice(&buf[..n]);
    }
    Ok(out)
}

/// Descomprime y, si lo que sale es un TAR, devuelve sus entradas.
///
/// Réplica exacta del camino que usa Swift Backup: el payload es
/// `TAR(zstd)`; el TAR interior se lee con `crate::swift_sba1_header::leer_tar`.
pub fn descomprimir_tar(
    data: &[u8],
    max_salida: usize,
) -> Result<(Vec<u8>, Vec<crate::swift_sba1_header::Entrada>), String> {
    let plano = descomprimir(data, max_salida)?;
    let entradas = crate::swift_sba1_header::leer_tar(&plano);
    Ok((plano, entradas))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_magic_zstd_se_reconoce() {
        assert!(es_zstd(&[0x28, 0xb5, 0x2f, 0xfd, 0x00]));
        assert!(!es_zstd(b"<?xml"));
        assert!(!es_zstd(&[0x28, 0xb5]));
    }

    #[test]
    fn comprimir_y_descomprimir_es_ida_y_vuelta() {
        let datos = b"contenido de prueba para Swift Backup".repeat(50);
        let comp = comprimir(&datos, 3).unwrap();
        assert!(es_zstd(&comp));
        assert!(comp.len() < datos.len());
        let plano = descomprimir(&comp, 10 * 1024 * 1024).unwrap();
        assert_eq!(plano, datos);
    }

    #[test]
    fn el_tope_de_salida_corta_la_bomba() {
        let datos = vec![0u8; 4 * 1024 * 1024];
        let comp = comprimir(&datos, 3).unwrap();
        // Muy por debajo de lo que el frame produciría.
        let r = descomprimir(&comp, 1024);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("tope"));
    }

    #[test]
    fn datos_que_no_son_zstd_dan_error_legible() {
        let r = descomprimir(b"esto no es zstd", 1024);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("zstd"));
    }
}
