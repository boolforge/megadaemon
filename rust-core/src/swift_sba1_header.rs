//! Cabecera `SBA1` de Swift Backup: lector y escritor **byte a byte**.
//!
//! ## De dónde sale el layout
//!
//! Se midió en el **escritor nativo** (`SbaArchiveNative_createArchive` de
//! `libsba_archive.so`), no se dedujo:
//!
//! * **ARM64** (el del móvil): el literal `"SBA1"` está en `.rodata` en
//!   `0x11b35`; el xref es `adrp x2,0x11000` + `add x2,x2,#0xb35` en
//!   `0x6562c`; a partir de ahí la secuencia de llamadas a los helpers de
//!   *append* da el orden exacto de los campos.
//! * **x86_64**: la misma secuencia ⇒ ⇒ **el layout NO cambia entre
//!   arquitecturas**.
//!
//! ## Hechos verificados en el desensamblado
//!
//! | Hecho | Evidencia |
//! |---|---|
//! | El primer campo es el literal `SBA1` (4 B) | `bl 0x6ae10` con `x2="SBA1"`, `w4=4` |
//! | Los enteros de 2 bytes van **BIG-ENDIAN** | helper `0x6aa00`: `lsr w8,w20,#8` + `strb` (byte alto) y luego `strb w20` (byte bajo) |
//! | Los enteros de 4 bytes van **BIG-ENDIAN** | helper `0x6ab6c`: `lsr w20,#24` → `#16` → `#8` → byte bajo, **sin ninguna rama de ancho** ⇒ **u32 fijo** |
//! | Los enteros de 8 bytes van **BIG-ENDIAN** | `time(NULL)*1000` se escribe byte a byte con `lsr x20,x28,x21` empezando por `>>56` |
//! | `headerSize` = 0x60 (96) o 0x90 (144) | `mov w8,#0x60` + `csel w23,w9,w8,eq` |
//! | Hay **tres bloques** de bytes de 16 B fijos | longitudes en 3 `u16` + 3 bloques de 16 B |
//!
//! ⚠️ **Corrección importante** (gracias al principal, que la detectó):
//! los `csel` del escritor **eligen el VALOR** de un campo, **no su
//! ancho**.  Los campos 4, 22 y 23 son **u32 fijos**, no de ancho variable
//! ⇒ ⇒ **no hace falta un `Perfil`**: el layout es una **tabla versionada**
//! (ver más abajo) y el lector y el escritor se derivan de ella.
//!
//! ## Tabla canónica
//!
//! La tabla **medida** vive en `crate::swift_archive::sba1_layout(formato)`
//! (fuente de verdad de la flota) y en
//! `docs/SWIFT_SBA1_HEADER_ARM64.md`.  Este módulo **no la duplica**: la
//! pide a `swift_archive` ⇒ ⇒ el escritor y el lector **no pueden
//! divergir** de la tabla, y el round-trip se prueba contra ella.
//!
//! ## Postura
//!
//! Esto es **interoperabilidad** (leer el backup del propio usuario), no
//! reimplementación de un motor propietario: no se ha copiado ni una línea
//! del código de Swift Apps, sólo se han medido bytes y desensamblado.

use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

use crate::swift_archive::{
    bloque_named, parse_header, sba1_detectar, sba1_layout, HeaderReport,
};

/// Magic del contenedor de archivo.
pub const MAGIC: &[u8; 4] = b"SBA1";
/// Magic del tar interior.
pub const MAGIC_TAR: &[u8; 12] = b"SBA_SWIFTTAR";
/// Tamaños de cabecera que escribe el nativo.
pub const TAMANOS_CABECERA: &[usize] = &[96, 144];

/// Error de lectura de la cabecera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCabecera {
    /// El buffer no empieza por `SBA1`.
    MagicInvalido,
    /// El buffer se ha terminado antes de tiempo.
    Truncado,
    /// El tamaño de cabecera no es 96 ni 144.
    TamanoDesconocido(u16),
}

impl std::fmt::Display for ErrorCabecera {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorCabecera::MagicInvalido => write!(f, "no empieza por el magic SBA1"),
            ErrorCabecera::Truncado => write!(f, "la cabecera está truncada"),
            ErrorCabecera::TamanoDesconocido(t) => {
                write!(f, "tamaño de cabecera desconocido: {}", t)
            }
        }
    }
}

/// La cabecera `SBA1`, campo a campo.
///
/// Los nombres vienen de la **tabla medida** (`swift_archive::sba1_layout`);
/// los que la tabla llama `campoN` son los que aún no tienen nombre
/// confirmado ⇒ **no se inventan**.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cabecera {
    /// `formato` (offset 4, u16 BE): 2 ⇒ variante extendida (144 B).
    pub formato: u16,
    /// `headerSize` (offset 6, u16 BE): 96 o 144.
    pub header_size: u16,
    /// `compression` (offset 8, u32 BE).
    pub compression: u32,
    /// `createdAtEpochMs` (offset 12, u64 BE).
    pub created_at_epoch_ms: u64,
    /// `flags` (offset 20, u16 BE).
    pub flags: u16,
    /// `flags2` (offset 22, u16 BE).
    pub flags2: u16,
    /// `campo3` (offset 24, u16 BE).
    pub campo3: u16,
    /// `campo4` (offset 26, u16 BE).
    pub campo4: u16,
    /// `campo5` (offset 28, u16 BE).
    pub campo5: u16,
    /// `lenBloque1` (offset 30, u16 BE).
    pub len_bloque1: u16,
    /// `lenBloque2` (offset 32, u16 BE).
    pub len_bloque2: u16,
    /// `lenBloque3` (offset 34, u16 BE).
    pub len_bloque3: u16,
    /// `bloque1` (offset 36, 16 B fijos, rellenados con ceros).
    pub bloque1: Vec<u8>,
    /// `bloque2` (offset 52, 16 B fijos).
    pub bloque2: Vec<u8>,
    /// `bloque3` (offset 68, 16 B fijos).
    pub bloque3: Vec<u8>,
    /// `campo10` (offset 84, u16 BE).
    pub campo10: u16,
    /// `campo11` (offset 86, u16 BE).
    pub campo11: u16,
    /// Tamaño del TROZO del AEAD (offset 88, u32 BE).
    ///
    /// MEDIDO el 2026-11-21 en cabeceras reales de Drive: siempre
    /// **1048576** (1 MiB).
    pub chunk_size: u32,
    /// Memoria (KiB) de Argon2id del KDF (offset 92, u32 BE) — sólo en la
    /// variante extendida.  MEDIDO: **16384**.
    pub argon2_memory_kib: u32,
    /// `campo14` (offset 96, u16 BE) — sólo extendida.
    pub campo14: u16,
    /// `campo15` (offset 98, u16 BE) — sólo extendida.
    pub campo15: u16,
    /// Longitud de la clave derivada en bytes (offset 100, u16 BE) — sólo
    /// extendida.  MEDIDO: **32**.
    pub key_length: u16,
    /// `campo17` (offset 102, u16 BE) — sólo extendida.
    pub campo17: u16,
    /// HMAC del payload (offset 104, 32 B) — sólo extendida.
    ///
    /// MEDIDO: 32 bytes NO nulos ⇒ es
    /// `SHA-256("SBA1-payload-hmac-key-v1" || salt || nonce2 || clave)`.
    pub payload_hmac: Vec<u8>,
    /// `reservado8` (offset 136, 8 B) — sólo extendida.
    pub reservado8: Vec<u8>,
    /// `reservado4` (offset 92, 4 B) — sólo en la variante corta.
    pub reservado4: Vec<u8>,
}

/// Escribe un entero **big-endian** de `ancho` bytes.
///
/// El formato medido es **big-endian sin excepción** (invariante), así que
/// no hay variante little-endian que mantener.
fn escribir_campo(out: &mut [u8], offset: usize, ancho: usize, valor: u64) {
    for i in 0..ancho {
        let byte = ((valor >> (8 * (ancho - 1 - i))) & 0xff) as u8;
        if offset + i < out.len() {
            out[offset + i] = byte;
        }
    }
}

impl Cabecera {
    /// La cabecera de la variante corta (96 B).
    pub fn corta() -> Cabecera {
        Cabecera {
            formato: 1,
            header_size: 96,
            ..Default::default()
        }
    }

    /// La cabecera de la variante extendida (144 B).
    pub fn extendida() -> Cabecera {
        Cabecera {
            formato: 2,
            header_size: 144,
            ..Default::default()
        }
    }

    /// Escribe la cabecera **exactamente** como la tabla medida.
    ///
    /// El buffer se dimensiona con `headerSize` y se rellena de ceros; cada
    /// campo se escribe en su offset con su ancho.  Los tres bloques se
    /// escriben en su `data_offset` con 16 B fijos (rellenados por la
    /// derecha con ceros), que es lo que hace el nativo.
    pub fn escribir(&self) -> Vec<u8> {
        let layout = sba1_layout(self.formato);
        let size = self.header_size.max(96) as usize;
        let mut out = vec![0u8; size];
        // El magic va en el offset 0 (la tabla lo declara aparte de los campos).
        out[0..4].copy_from_slice(MAGIC);
        let scalares: Vec<(&str, u64)> = vec![
            ("formato", self.formato as u64),
            ("headerSize", self.header_size as u64),
            ("compression", self.compression as u64),
            ("createdAtEpochMs", self.created_at_epoch_ms),
            ("flags", self.flags as u64),
            ("flags2", self.flags2 as u64),
            ("campo3", self.campo3 as u64),
            ("campo4", self.campo4 as u64),
            ("campo5", self.campo5 as u64),
            ("lenBloque1", self.len_bloque1 as u64),
            ("lenBloque2", self.len_bloque2 as u64),
            ("lenBloque3", self.len_bloque3 as u64),
            ("campo10", self.campo10 as u64),
            ("campo11", self.campo11 as u64),
            ("chunkSize", self.chunk_size as u64),
            ("argon2MemoryKib", self.argon2_memory_kib as u64),
            ("campo14", self.campo14 as u64),
            ("campo15", self.campo15 as u64),
            ("keyLength", self.key_length as u64),
            ("campo17", self.campo17 as u64),
        ];
        for f in &layout.fields {
            if let Some((_, v)) = scalares.iter().find(|(n, _)| *n == f.name) {
                debug_assert!(f.big_endian, "la tabla medida es big-endian");
                escribir_campo(&mut out, f.offset as usize, f.width as usize, *v);
            }
        }
        // Los bloques: longitud en `len_offset` (u16 BE) y datos en
        // `data_offset` (16 B fijos).
        let bloques: [(&str, &Vec<u8>); 3] = [
            ("bloque1", &self.bloque1),
            ("bloque2", &self.bloque2),
            ("bloque3", &self.bloque3),
        ];
        for b in &layout.byte_blocks {
            if let Some((_, datos)) = bloques.iter().find(|(n, _)| *n == b.name) {
                escribir_campo(
                    &mut out,
                    b.len_offset as usize,
                    b.len_width as usize,
                    datos.len() as u64,
                );
                let ini = b.data_offset as usize;
                for (i, byte) in datos.iter().enumerate() {
                    if ini + i < out.len() {
                        out[ini + i] = *byte;
                    }
                }
            }
        }
        out
    }

    /// Lee la cabecera desde el principio del buffer.
    ///
    /// Devuelve la cabecera y el número de bytes consumidos (= `headerSize`).
    pub fn leer(bytes: &[u8]) -> Result<(Cabecera, usize), ErrorCabecera> {
        let layout = sba1_detectar(bytes).ok_or(ErrorCabecera::MagicInvalido)?;
        let report = parse_header(bytes, &layout).map_err(|_| ErrorCabecera::Truncado)?;
        let mut cab = Self::desde_report(&report);
        cab.bloque1 = bloque_named(&report, bytes, "bloque1").unwrap_or_default();
        cab.bloque2 = bloque_named(&report, bytes, "bloque2").unwrap_or_default();
        cab.bloque3 = bloque_named(&report, bytes, "bloque3").unwrap_or_default();
        let n = report.header_size as usize;
        if !TAMANOS_CABECERA.contains(&n) {
            return Err(ErrorCabecera::TamanoDesconocido(cab.header_size));
        }
        Ok((cab, n))
    }

    /// Construye la cabecera desde el informe de la tabla medida.
    fn desde_report(report: &HeaderReport) -> Cabecera {
        let mut cab = Cabecera {
            header_size: report.header_size as u16,
            ..Default::default()
        };
        for f in &report.fields {
            let v = f.value;
            match f.name.as_str() {
                "formato" => cab.formato = v as u16,
                "headerSize" => cab.header_size = v as u16,
                "compression" => cab.compression = v as u32,
                "createdAtEpochMs" => cab.created_at_epoch_ms = v,
                "flags" => cab.flags = v as u16,
                "flags2" => cab.flags2 = v as u16,
                "campo3" => cab.campo3 = v as u16,
                "campo4" => cab.campo4 = v as u16,
                "campo5" => cab.campo5 = v as u16,
                "lenBloque1" => cab.len_bloque1 = v as u16,
                "lenBloque2" => cab.len_bloque2 = v as u16,
                "lenBloque3" => cab.len_bloque3 = v as u16,
                "campo10" => cab.campo10 = v as u16,
                "campo11" => cab.campo11 = v as u16,
                "chunkSize" | "campo12" => cab.chunk_size = v as u32,
                "argon2MemoryKib" | "campo13" => cab.argon2_memory_kib = v as u32,
                "campo14" => cab.campo14 = v as u16,
                "campo15" => cab.campo15 = v as u16,
                "keyLength" | "campo16" => cab.key_length = v as u16,
                "campo17" => cab.campo17 = v as u16,
                _ => {}
            }
        }
        for b in &report.blocks {
            match b.name.as_str() {
                "bloque1" => cab.len_bloque1 = b.len as u16,
                "bloque2" => cab.len_bloque2 = b.len as u16,
                "bloque3" => cab.len_bloque3 = b.len as u16,
                _ => {}
            }
        }
        cab
    }
}

/// Longitud del payload (lo que va **después** de la cabecera).
pub fn localizar_payload(bytes: &[u8], header_size: usize) -> Option<&[u8]> {
    if bytes.len() <= header_size {
        return None;
    }
    Some(&bytes[header_size..])
}

// ========================================================================= //
// Extracción del contenedor
// ========================================================================= //

/// Una entrada del TAR interior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entrada {
    /// Ruta dentro del backup.
    pub nombre: String,
    /// Tamaño del fichero.
    pub tamano: u64,
    /// Offset del contenido dentro del TAR.
    pub offset: usize,
}

/// Lee un TAR (POSIX/GNU) y devuelve sus entradas.
///
/// No extrae nada a disco: sólo el índice.  Soporta nombres largos GNU
/// (`L`) y las cabeceras extendidas pax (`x`/`g`).
pub fn leer_tar(bytes: &[u8]) -> Vec<Entrada> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    let mut pending_long: Option<String> = None;
    while pos + 512 <= bytes.len() {
        let hdr = &bytes[pos..pos + 512];
        if hdr.iter().all(|&b| b == 0) {
            break; // dos bloques de NUL = fin
        }
        let name = cstr(&hdr[0..100]);
        let prefix = cstr(&hdr[345..500]);
        let size_field = cstr(&hdr[124..136]);
        let size = u64::from_str_radix(size_field.trim().trim_end_matches('\0').trim(), 8)
            .unwrap_or(0);
        let typeflag = hdr[156];
        pos += 512;
        match typeflag {
            b'L' => {
                let end = (pos + size as usize).min(bytes.len());
                pending_long = Some(cstr(&bytes[pos..end]));
                pos = end + ((512 - (size as usize % 512)) % 512);
                continue;
            }
            b'x' | b'g' | b'K' => {
                let end = (pos + size as usize).min(bytes.len());
                pos = end + ((512 - (size as usize % 512)) % 512);
                continue;
            }
            _ => {}
        }
        let full = match pending_long.take() {
            Some(l) => l,
            None => {
                if prefix.is_empty() {
                    name
                } else {
                    format!("{}/{}", prefix.trim_end_matches('/'), name)
                }
            }
        };
        if typeflag == b'0' || typeflag == 0 {
            out.push(Entrada {
                nombre: full,
                tamano: size,
                offset: pos,
            });
        }
        pos += ((size as usize) + 511) / 512 * 512;
    }
    out
}

/// Lee una cadena terminada en NUL.
fn cstr(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).to_string()
}

/// Sanea una ruta de entrada: fuera rutas absolutas, `..` y `:`.
///
/// Es la misma regla que usa el lector de la flota
/// (`swift_sba1::sanear_ruta`): un backup **no** puede escribir fuera del
/// destino.
pub fn sanear_ruta(nombre: &str) -> Result<PathBuf, String> {
    let p = Path::new(nombre);
    let mut limpio = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::Normal(s) => {
                let s = s.to_string_lossy();
                if s.contains(':') || s.contains('\0') {
                    return Err(format!("ruta con caracteres no permitidos: {}", nombre));
                }
                limpio.push(s.as_ref());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(format!("ruta con '..': {}", nombre));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!("ruta absoluta: {}", nombre));
            }
        }
    }
    if limpio.as_os_str().is_empty() {
        return Err("ruta vacía".to_string());
    }
    Ok(limpio)
}

/// Qué se ha extraído de un contenedor.
#[derive(Debug, Clone, Default)]
pub struct InformeExtraccion {
    /// La cabecera leída.
    pub cabecera: Cabecera,
    /// Bytes consumidos por la cabecera.
    pub cabecera_bytes: usize,
    /// Bytes del payload (tras la cabecera).
    pub payload_bytes: usize,
    /// `true` si el payload venía comprimido con zstd.
    pub comprimido: bool,
    /// Entradas del TAR interior.
    pub entradas: Vec<Entrada>,
    /// Nota informativa (por ejemplo, por qué no se pudo descifrar).
    pub mensaje: String,
}

impl InformeExtraccion {
    /// Versión serializable.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "header_size": self.cabecera.header_size,
            "formato": self.cabecera.formato,
            "flags": self.cabecera.flags,
            "compression": self.cabecera.compression,
            "created_at_epoch_ms": self.cabecera.created_at_epoch_ms,
            "header_bytes": self.cabecera_bytes,
            "payload_bytes": self.payload_bytes,
            "compressed": self.comprimido,
            "entries": self.entradas.iter().map(|e| serde_json::json!({
                "name": e.nombre,
                "size": e.tamano,
            })).collect::<Vec<_>>(),
            "message": self.mensaje,
        })
    }
}

/// Detecta si el payload está comprimido con zstd (magic `28 B5 2F FD`).
fn es_zstd(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && &bytes[0..4] == &[0x28, 0xb5, 0x2f, 0xfd]
}

/// Analiza un contenedor `SBA1`: cabecera + payload + TAR interior.
///
/// **Sin cifrar**: si `compression`/`encryptionMethodId` es 0, el payload va
/// directo a zstd/TAR y se puede leer entero.
///
/// **Cifrado**: la derivación del nonce y del AAD por trozo es **nativa**
/// (`SbaLibaegisCryptoNative_decryptAegis256ChunkedEntryFdRaw`), así que
/// aquí **no** se descifra: se dice exactamente por qué, en vez de
/// inventarse una derivación que produciría basura.
pub fn informar_contenedor(bytes: &[u8]) -> Result<InformeExtraccion, String> {
    let (cabecera, n) = Cabecera::leer(bytes).map_err(|e| e.to_string())?;
    let payload = &bytes[n.min(bytes.len())..];
    let mut inf = InformeExtraccion {
        cabecera: cabecera.clone(),
        cabecera_bytes: n,
        payload_bytes: payload.len(),
        ..Default::default()
    };

    if cabecera.compression != 0 {
        inf.mensaje = format!(
            "contenido cifrado (encryptionMethodId={}): la derivación del nonce y del AAD \
             por trozo es NATIVA (SbaLibaegisCryptoNative_decryptAegis256ChunkedEntryFdRaw); \
             hace falta la clave y esa derivación. No se descifra a ciegas.",
            cabecera.compression
        );
        return Ok(inf);
    }

    let plano = if es_zstd(payload) {
        inf.comprimido = true;
        crate::zstd::descomprimir(payload, 256 * 1024 * 1024)?
    } else {
        payload.to_vec()
    };
    inf.entradas = leer_tar(&plano);
    if inf.entradas.is_empty() {
        inf.mensaje = "el payload no es un TAR reconocible".to_string();
    } else {
        inf.mensaje = format!("{} entradas en el TAR interior", inf.entradas.len());
    }
    Ok(inf)
}

/// Extrae el contenido de un contenedor `SBA1` a `dest`.
///
/// Devuelve el informe con la lista de ficheros escritos.  Las rutas se
/// sanean: **nada** se escribe fuera de `dest`.
pub fn extraer_contenedor(
    bytes: &[u8],
    dest: &Path,
    max_salida: usize,
) -> Result<InformeExtraccion, String> {
    let inf = informar_contenedor(bytes)?;
    if inf.cabecera.compression != 0 {
        return Ok(inf); // cifrado: nada que extraer (y se explica por qué)
    }
    let payload = &bytes[inf.cabecera_bytes.min(bytes.len())..];
    let plano = if inf.comprimido {
        crate::zstd::descomprimir(payload, max_salida)?
    } else {
        payload.to_vec()
    };
    let entradas = leer_tar(&plano);
    std::fs::create_dir_all(dest).map_err(|e| format!("no se pudo crear {:?}: {}", dest, e))?;
    for e in &entradas {
        let limpio = sanear_ruta(&e.nombre)?;
        let destino = dest.join(&limpio);
        if let Some(padre) = destino.parent() {
            std::fs::create_dir_all(padre)
                .map_err(|err| format!("no se pudo crear {:?}: {}", padre, err))?;
        }
        let ini = e.offset;
        if ini > plano.len() {
            continue;
        }
        let fin = (ini + e.tamano as usize).min(plano.len());
        std::fs::write(&destino, &plano[ini..fin])
            .map_err(|err| format!("no se pudo escribir {:?}: {}", destino, err))?;
    }
    let mut inf = inf;
    inf.entradas = entradas;
    inf.mensaje = format!("{} ficheros extraídos en {:?}", inf.entradas.len(), dest);
    Ok(inf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cabecera_corta() -> Cabecera {
        Cabecera {
            formato: 1,
            header_size: 96,
            compression: 0,
            created_at_epoch_ms: 1_700_000_000_123,
            flags: 3,
            flags2: 0,
            campo3: 1,
            campo4: 2,
            campo5: 3,
            len_bloque1: 16,
            len_bloque2: 16,
            len_bloque3: 16,
            bloque1: vec![0xAB; 16],
            bloque2: vec![0xCD; 16],
            bloque3: vec![0xEF; 16],
            campo10: 7,
            campo11: 8,
            chunk_size: 9,
            ..Default::default()
        }
    }

    #[test]
    fn la_cabecera_empieza_por_el_magic() {
        let bytes = cabecera_corta().escribir();
        assert_eq!(&bytes[0..4], b"SBA1");
        assert_eq!(bytes.len(), 96);
    }

    #[test]
    fn la_variante_extendida_mide_144() {
        let mut cab = cabecera_corta();
        cab.formato = 2;
        cab.header_size = 144;
        let bytes = cab.escribir();
        assert_eq!(bytes.len(), 144);
    }

    #[test]
    fn escribir_y_leer_es_un_round_trip() {
        let cab = cabecera_corta();
        let bytes = cab.escribir();
        let (leida, n) = Cabecera::leer(&bytes).expect("debería leer");
        assert_eq!(n, bytes.len());
        assert_eq!(leida.formato, cab.formato);
        assert_eq!(leida.header_size, cab.header_size);
        assert_eq!(leida.created_at_epoch_ms, cab.created_at_epoch_ms);
        assert_eq!(leida.flags, cab.flags);
        assert_eq!(leida.campo3, cab.campo3);
        assert_eq!(leida.bloque1, cab.bloque1);
        assert_eq!(leida.bloque2, cab.bloque2);
        assert_eq!(leida.bloque3, cab.bloque3);
        assert_eq!(leida.chunk_size, cab.chunk_size);
    }

    #[test]
    fn los_enteros_son_big_endian() {
        let mut cab = cabecera_corta();
        cab.flags = 0x0102;
        let bytes = cab.escribir();
        // flags van en el offset 20 (u16 BE) => 0x01 0x02
        assert_eq!(&bytes[20..22], &[0x01, 0x02]);
    }

    #[test]
    fn el_created_at_es_big_endian_de_8_bytes() {
        let cab = cabecera_corta();
        let bytes = cab.escribir();
        // createdAtEpochMs va en el offset 12 (u64 BE)
        let ts = &bytes[12..20];
        assert_eq!(ts[0], 0x00); // el byte más alto de 1_700_000_000_123 es 0
        assert_eq!(ts[7], 0x7B); // ... y el más bajo es 0x7B (123)
    }

    #[test]
    fn los_bloques_van_en_su_offset_con_16_bytes_fijos() {
        let cab = cabecera_corta();
        let bytes = cab.escribir();
        assert_eq!(&bytes[36..52], &cab.bloque1[..]);
        assert_eq!(&bytes[52..68], &cab.bloque2[..]);
        assert_eq!(&bytes[68..84], &cab.bloque3[..]);
    }

    #[test]
    fn un_magic_invalido_no_entra_en_panic() {
        let r = Cabecera::leer(b"XXXX\x00\x00\x00\x00");
        assert!(r.is_err());
    }

    #[test]
    fn una_cabecera_truncada_no_entra_en_panic() {
        let bytes = cabecera_corta().escribir();
        for corte in 0..bytes.len() {
            let _ = Cabecera::leer(&bytes[..corte]);
        }
    }

    #[test]
    fn el_payload_empieza_despues_de_la_cabecera() {
        let cab = cabecera_corta();
        let mut bytes = cab.escribir();
        let n = bytes.len();
        bytes.extend_from_slice(b"PAYLOAD");
        let p = localizar_payload(&bytes, n).expect("debería localizar");
        assert_eq!(p, b"PAYLOAD");
    }

    // ------------------------------------------------------------------ //
    // Extracción
    // ------------------------------------------------------------------ //

    /// Construye un TAR con los ficheros indicados (cabeceras POSIX de 512 B).
    fn build_tar(ficheros: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        for (nombre, datos) in ficheros {
            let mut hdr = [0u8; 512];
            let nb = nombre.as_bytes();
            let n = nb.len().min(100);
            hdr[..n].copy_from_slice(&nb[..n]);
            let octal = format!("{:011o}\0", datos.len());
            hdr[124..124 + octal.len()].copy_from_slice(octal.as_bytes());
            hdr[156] = b'0';
            for b in hdr.iter_mut().take(156).skip(148) {
                *b = b' ';
            }
            let sum: u32 = hdr.iter().map(|&b| b as u32).sum();
            let chk = format!("{:06o}\0 ", sum);
            hdr[148..148 + chk.len()].copy_from_slice(chk.as_bytes());
            out.extend_from_slice(&hdr);
            out.extend_from_slice(datos);
            let pad = (512 - (datos.len() % 512)) % 512;
            out.extend(std::iter::repeat(0u8).take(pad));
        }
        out.extend(std::iter::repeat(0u8).take(1024)); // dos bloques NUL
        out
    }

    fn contenedor_sintetico(contenido: &[u8]) -> Vec<u8> {
        let mut cab = cabecera_corta();
        cab.compression = 0;
        let mut bytes = cab.escribir();
        bytes.extend_from_slice(contenido);
        bytes
    }

    #[test]
    fn el_tar_se_lee_con_sus_entradas() {
        let tar = build_tar(&[("a.txt", b"hola"), ("dir/b.bin", &[0u8, 1, 2, 3])]);
        let entradas = leer_tar(&tar);
        assert_eq!(entradas.len(), 2);
        assert_eq!(entradas[0].nombre, "a.txt");
        assert_eq!(entradas[0].tamano, 4);
        assert_eq!(entradas[1].nombre, "dir/b.bin");
        assert_eq!(entradas[1].tamano, 4);
    }

    #[test]
    fn el_contenedor_sintetico_se_informa() {
        let tar = build_tar(&[("a.txt", b"hola")]);
        let bytes = contenedor_sintetico(&tar);
        let inf = informar_contenedor(&bytes).expect("debería informar");
        assert_eq!(inf.cabecera.compression, 0);
        assert_eq!(inf.entradas.len(), 1);
        assert_eq!(inf.entradas[0].nombre, "a.txt");
        assert!(!inf.comprimido);
    }

    #[test]
    fn el_contenedor_sintetico_se_extrae() {
        let tar = build_tar(&[("a.txt", b"hola"), ("dir/b.bin", &[7u8; 5])]);
        let bytes = contenedor_sintetico(&tar);
        let dest =
            std::env::temp_dir().join(format!("megadaemon-sba1-test-{}-out", std::process::id()));
        let _ = std::fs::remove_dir_all(&dest);
        let inf = extraer_contenedor(&bytes, &dest, 1 << 20).expect("debería extraer");
        assert_eq!(inf.entradas.len(), 2);
        let a = std::fs::read(dest.join("a.txt")).expect("debería existir a.txt");
        assert_eq!(a, b"hola");
        let b = std::fs::read(dest.join("dir/b.bin")).expect("debería existir b.bin");
        assert_eq!(b, vec![7u8; 5]);
        let _ = std::fs::remove_dir_all(&dest);
    }

    #[test]
    fn un_contenedor_cifrado_no_se_descifra_a_ciegas() {
        let mut cab = cabecera_corta();
        cab.compression = 4; // AEGIS-256
        let mut bytes = cab.escribir();
        bytes.extend_from_slice(b"cifrado");
        let inf = informar_contenedor(&bytes).expect("debería informar");
        assert!(inf.entradas.is_empty());
        assert!(
            inf.mensaje.contains("NATIVA"),
            "el mensaje debe explicar por qué: {}",
            inf.mensaje
        );
    }

    #[test]
    fn las_rutas_se_sanean() {
        assert!(sanear_ruta("a/b/c.txt").is_ok());
        assert!(sanear_ruta("../escape.txt").is_err());
        assert!(sanear_ruta("/absoluta.txt").is_err());
        assert!(sanear_ruta("con:dos:puntos").is_err());
        assert!(sanear_ruta("").is_err());
    }
}
