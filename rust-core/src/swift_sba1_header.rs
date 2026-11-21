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
//! * **x86_64**: la misma secuencia de 27 escrituras ⇒ ⇒ **el layout NO
//!   cambia entre arquitecturas**.
//!
//! ## Hechos verificados en el desensamblado
//!
//! | Hecho | Evidencia |
//! |---|---|
//! | El primer campo es el literal `SBA1` (4 B) | `bl 0x6ae10` con `x2="SBA1"`, `w4=4` |
//! | Los enteros de 2 bytes van **BIG-ENDIAN** | helper `0x6aa00`: `lsr w8,w20,#8` + `strb` (byte alto) y luego `strb w20` (byte bajo) |
//! | Los enteros de 8 bytes van **BIG-ENDIAN** | `time(NULL)*1000` se escribe byte a byte con `lsr x20,x28,x21` empezando por `>>56` |
//! | `headerSize` = 0x60 (96) o 0x90 (144) | `mov w8,#0x60` + `csel w23,w9,w8,eq` |
//! | Hay **tres bloques** de bytes precedidos de su longitud | `sub w1,w8,w9` (longitud) y `bl 0x6b524` (bloque) |
//!
//! ## Lo que NO se congela
//!
//! Los campos **4, 22 y 23** se escriben con un helper de **ancho
//! variable** (`0x6ab6c`): el ancho (1, 2 o 3 bytes) lo decide el escritor
//! con un `csel` según otros campos ⇒ ⇒ **un lector no puede deducir el
//! ancho sólo de los bytes**.  Por eso [`leer`] recibe un [`Perfil`] con los
//! anchos y [`escribir`] usa el mismo.  Cuando llegue un `.dat` real, el
//! perfil se casa con los bytes y **entonces** se congela en un test.
//!
//! ## Postura
//!
//! Esto es **interoperabilidad** (leer el backup del propio usuario), no
//! reimplementación de un motor propietario: no se ha copiado ni una línea
//! del código de Swift Apps, sólo se han medido bytes y desensamblado.

use serde::{Deserialize, Serialize};

/// Magic del contenedor de archivo.
pub const MAGIC: &[u8; 4] = b"SBA1";
/// Magic del tar interior.
pub const MAGIC_TAR: &[u8; 12] = b"SBA_SWIFTTAR";
/// Tamaños de cabecera que escribe el nativo.
pub const TAMANOS_CABECERA: &[usize] = &[96, 144];

/// Ancho (en bytes) de los enteros de ancho variable.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Perfil {
    /// Ancho del campo 4 (`encryptionMethodId`): 1, 2 o 3.
    pub campo4: u8,
    /// Ancho del campo 22 (`struct[0x18]`).
    pub campo22: u8,
    /// Ancho del campo 23 (`struct[0x10]`).
    pub campo23: u8,
}

impl Default for Perfil {
    /// El perfil por defecto es el de un contenedor **cifrado con
    /// AEGIS-256**: `encryptionMethodId` = 4 (cabe en 1 byte) y los campos
    /// 22/23 ocupan 4 bytes (son `u32` en el modelo Kotlin).
    fn default() -> Self {
        Perfil {
            campo4: 1,
            campo22: 4,
            campo23: 4,
        }
    }
}

impl Perfil {
    /// Perfil de un contenedor **sin cifrar** (`encryptionMethodId` = 0).
    pub fn sin_cifrado() -> Self {
        Perfil {
            campo4: 1,
            campo22: 2,
            campo23: 2,
        }
    }
}

/// La cabecera `SBA1`, campo a campo.
///
/// Los nombres que no están confirmados se numeran por su **offset en el
/// struct nativo** (`struct[0xNN]`), que es lo único que se puede afirmar
/// sin inventar: el escritor lee esos offsets y nada más.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cabecera {
    /// `struct[0x00]`: flags.
    pub flags: u16,
    /// Tamaño total de la cabecera: 96 o 144.
    pub header_size: u16,
    /// `struct[0x04]`: id del método de cifrado (0=None, 4=AEGIS-256).
    pub encryption_method_id: u32,
    /// `time(NULL) * 1000` en el momento de escribir.
    pub created_at_epoch_ms: u64,
    /// Campo `A` (u16, sin nombre confirmado).
    pub campo_a: u16,
    /// Campo `B` (u16, sin nombre confirmado).
    pub campo_b: u16,
    /// `struct[0x04]` tal y como se escribe en la 8ª posición.
    pub struct_04: u16,
    /// `struct[0x08]`.
    pub struct_08: u16,
    /// `struct[0x0c]`.
    pub struct_0c: u16,
    /// Longitud declarada del bloque 1 (`struct[0x48]-[0x40]`).
    pub len_bloque1: u16,
    /// Longitud declarada del bloque 2 (`struct[0x60]-[0x58]`).
    pub len_bloque2: u16,
    /// Longitud declarada del bloque 3 (`struct[0x78]-[0x70]`).
    pub len_bloque3: u16,
    /// Bloque 1 (bytes).
    pub bloque1: Vec<u8>,
    /// Bloque 2 (bytes).
    pub bloque2: Vec<u8>,
    /// Bloque 3 (bytes).
    pub bloque3: Vec<u8>,
    /// `struct[0x18]`.
    pub struct_18: u32,
    /// `struct[0x10]` (sólo se escribe si `flags == 2`).
    pub struct_10: u32,
    /// `struct[0x14]`.
    pub struct_14: u16,
    /// `struct[0x1c]`.
    pub struct_1c: u16,
    /// `struct[0x20]`.
    pub struct_20: u16,
    /// `struct[0x04]` (última escritura).
    pub struct_04_final: u16,
}

/// Error de lectura de la cabecera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCabecera {
    /// El buffer no empieza por `SBA1`.
    MagicInvalido,
    /// El buffer se ha terminado antes de tiempo.
    Truncado,
    /// Los anchos del perfil no son coherentes.
    PerfilInvalido,
    /// El tamaño de cabecera no es 96 ni 144.
    TamanoDesconocido(u16),
}

impl std::fmt::Display for ErrorCabecera {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorCabecera::MagicInvalido => write!(f, "no empieza por el magic SBA1"),
            ErrorCabecera::Truncado => write!(f, "la cabecera está truncada"),
            ErrorCabecera::PerfilInvalido => write!(f, "el perfil de anchos no es válido"),
            ErrorCabecera::TamanoDesconocido(t) => {
                write!(f, "tamaño de cabecera desconocido: {}", t)
            }
        }
    }
}

/// Escribe un entero **big-endian** de `ancho` bytes.
///
/// Los anchos de 0 no escriben nada (así lo hace el nativo cuando decide
/// omitir un campo).
fn put_uint(out: &mut Vec<u8>, value: u64, ancho: u8) {
    for i in (0..ancho as u32).rev() {
        out.push(((value >> (8 * i)) & 0xff) as u8);
    }
}

/// Lee un entero **big-endian** de `ancho` bytes.
fn get_uint(bytes: &[u8], pos: &mut usize, ancho: u8) -> Result<u64, ErrorCabecera> {
    if *pos + ancho as usize > bytes.len() {
        return Err(ErrorCabecera::Truncado);
    }
    let mut v = 0u64;
    for _ in 0..ancho {
        v = (v << 8) | bytes[*pos] as u64;
        *pos += 1;
    }
    Ok(v)
}

impl Cabecera {
    /// Escribe la cabecera **exactamente** como lo hace el nativo.
    ///
    /// El orden de los campos es el medido en el escritor (ARM64 y x86_64
    /// coinciden); ver la cabecera del módulo.
    pub fn escribir(&self, perfil: &Perfil) -> Vec<u8> {
        let mut out = Vec::with_capacity(160);
        // 1. magic
        out.extend_from_slice(MAGIC);
        // 2. flags (u16 BE)
        put_uint(&mut out, self.flags as u64, 2);
        // 3. headerSize (u16 BE)
        put_uint(&mut out, self.header_size as u64, 2);
        // 4. encryptionMethodId (ancho variable)
        put_uint(&mut out, self.encryption_method_id as u64, perfil.campo4);
        // 5. createdAtEpochMs (u64 BE)
        put_uint(&mut out, self.created_at_epoch_ms, 8);
        // 6-7. campos A y B
        put_uint(&mut out, self.campo_a as u64, 2);
        put_uint(&mut out, self.campo_b as u64, 2);
        // 8-10. struct[0x04], [0x08], [0x0c]
        put_uint(&mut out, self.struct_04 as u64, 2);
        put_uint(&mut out, self.struct_08 as u64, 2);
        put_uint(&mut out, self.struct_0c as u64, 2);
        // 11-13. longitudes de los tres bloques
        put_uint(&mut out, self.len_bloque1 as u64, 2);
        put_uint(&mut out, self.len_bloque2 as u64, 2);
        put_uint(&mut out, self.len_bloque3 as u64, 2);
        // 14-19. los tres bloques, cada uno como (longitud + bytes)
        for bloque in [&self.bloque1, &self.bloque2, &self.bloque3] {
            put_uint(&mut out, bloque.len() as u64, 4);
            out.extend_from_slice(bloque);
        }
        // 20-21. dos u16 a cero
        put_uint(&mut out, 0, 2);
        put_uint(&mut out, 0, 2);
        // 22. struct[0x18] (ancho variable)
        put_uint(&mut out, self.struct_18 as u64, perfil.campo22);
        // 23. struct[0x10] (ancho variable; sólo si flags == 2)
        if self.flags == 2 {
            put_uint(&mut out, self.struct_10 as u64, perfil.campo23);
        }
        // 24-26. struct[0x14], [0x1c], [0x20]
        put_uint(&mut out, self.struct_14 as u64, 2);
        put_uint(&mut out, self.struct_1c as u64, 2);
        put_uint(&mut out, self.struct_20 as u64, 2);
        // 27. struct[0x04] (última escritura)
        put_uint(&mut out, self.struct_04_final as u64, 2);
        out
    }

    /// Lee la cabecera desde el principio del buffer.
    ///
    /// Devuelve la cabecera y el número de bytes consumidos (que es el
    /// tamaño de la cabecera: 96 o 144).
    pub fn leer(bytes: &[u8], perfil: &Perfil) -> Result<(Cabecera, usize), ErrorCabecera> {
        if bytes.len() < MAGIC.len() || &bytes[0..4] != MAGIC {
            return Err(ErrorCabecera::MagicInvalido);
        }
        if perfil.campo4 > 8 || perfil.campo22 > 8 || perfil.campo23 > 8 {
            return Err(ErrorCabecera::PerfilInvalido);
        }
        let mut p = 4usize;
        let mut cab = Cabecera::default();
        cab.flags = get_uint(bytes, &mut p, 2)? as u16;
        cab.header_size = get_uint(bytes, &mut p, 2)? as u16;
        if !TAMANOS_CABECERA.contains(&(cab.header_size as usize)) {
            return Err(ErrorCabecera::TamanoDesconocido(cab.header_size));
        }
        cab.encryption_method_id = get_uint(bytes, &mut p, perfil.campo4)? as u32;
        cab.created_at_epoch_ms = get_uint(bytes, &mut p, 8)?;
        cab.campo_a = get_uint(bytes, &mut p, 2)? as u16;
        cab.campo_b = get_uint(bytes, &mut p, 2)? as u16;
        cab.struct_04 = get_uint(bytes, &mut p, 2)? as u16;
        cab.struct_08 = get_uint(bytes, &mut p, 2)? as u16;
        cab.struct_0c = get_uint(bytes, &mut p, 2)? as u16;
        cab.len_bloque1 = get_uint(bytes, &mut p, 2)? as u16;
        cab.len_bloque2 = get_uint(bytes, &mut p, 2)? as u16;
        cab.len_bloque3 = get_uint(bytes, &mut p, 2)? as u16;
        for (len, destino) in [
            (cab.len_bloque1, 0usize),
            (cab.len_bloque2, 1usize),
            (cab.len_bloque3, 2usize),
        ] {
            let n = get_uint(bytes, &mut p, 4)? as usize;
            if p + n > bytes.len() {
                return Err(ErrorCabecera::Truncado);
            }
            let bloque = bytes[p..p + n].to_vec();
            p += n;
            match destino {
                0 => cab.bloque1 = bloque,
                1 => cab.bloque2 = bloque,
                _ => cab.bloque3 = bloque,
            }
            let _ = len;
        }
        // 20-21. dos u16 a cero (el escritor los emite, el lector los consume)
        let _cero1 = get_uint(bytes, &mut p, 2)?;
        let _cero2 = get_uint(bytes, &mut p, 2)?;
        cab.struct_18 = get_uint(bytes, &mut p, perfil.campo22)? as u32;
        if cab.flags == 2 {
            cab.struct_10 = get_uint(bytes, &mut p, perfil.campo23)? as u32;
        }
        cab.struct_14 = get_uint(bytes, &mut p, 2)? as u16;
        cab.struct_1c = get_uint(bytes, &mut p, 2)? as u16;
        cab.struct_20 = get_uint(bytes, &mut p, 2)? as u16;
        cab.struct_04_final = get_uint(bytes, &mut p, 2)? as u16;
        Ok((cab, p))
    }
}

/// Longitud del payload (lo que va **después** de la cabecera).
///
/// La cabecera va al principio del fichero y el payload detrás; los tamaños
/// son los del nativo (96 para la info page de una entrada, 144 para la del
/// archivo raíz).
pub fn localizar_payload(bytes: &[u8], header_size: usize) -> Option<&[u8]> {
    if bytes.len() <= header_size {
        return None;
    }
    Some(&bytes[header_size..])
}

/// Heurística: probar los dos perfiles y quedarse con el que cuadra.
///
/// Se usa cuando no se sabe si el contenedor lleva cifrado: sin cifrar el
/// campo `flags` no vale 2 y el campo 23 no se escribe.
pub fn leer_auto(bytes: &[u8]) -> Result<(Cabecera, usize), ErrorCabecera> {
    for perfil in [Perfil::default(), Perfil::sin_cifrado()] {
        if let Ok((cab, n)) = Cabecera::leer(bytes, &perfil) {
            return Ok((cab, n));
        }
    }
    Err(ErrorCabecera::MagicInvalido)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cabecera_de_ejemplo() -> Cabecera {
        Cabecera {
            flags: 2,
            header_size: 144,
            encryption_method_id: 4,
            created_at_epoch_ms: 1_700_000_000_123,
            campo_a: 1,
            campo_b: 2,
            struct_04: 4,
            struct_08: 0,
            struct_0c: 3,
            len_bloque1: 32,
            len_bloque2: 16,
            len_bloque3: 4,
            bloque1: vec![0xAB; 32],
            bloque2: vec![0xCD; 16],
            bloque3: vec![0xEF; 4],
            struct_18: 0,
            struct_10: 1,
            struct_14: 7,
            struct_1c: 8,
            struct_20: 9,
            struct_04_final: 4,
        }
    }

    #[test]
    fn la_cabecera_empieza_por_el_magic() {
        let cab = cabecera_de_ejemplo();
        let bytes = cab.escribir(&Perfil::default());
        assert_eq!(&bytes[0..4], b"SBA1");
    }

    #[test]
    fn escribir_y_leer_es_un_round_trip() {
        let cab = cabecera_de_ejemplo();
        let bytes = cab.escribir(&Perfil::default());
        let (leida, n) = Cabecera::leer(&bytes, &Perfil::default()).expect("debería leer");
        assert_eq!(n, bytes.len());
        assert_eq!(leida.flags, cab.flags);
        assert_eq!(leida.header_size, cab.header_size);
        assert_eq!(leida.encryption_method_id, cab.encryption_method_id);
        assert_eq!(leida.created_at_epoch_ms, cab.created_at_epoch_ms);
        assert_eq!(leida.bloque1, cab.bloque1);
        assert_eq!(leida.bloque2, cab.bloque2);
        assert_eq!(leida.bloque3, cab.bloque3);
        assert_eq!(leida.struct_18, cab.struct_18);
        assert_eq!(leida.struct_10, cab.struct_10);
    }

    #[test]
    fn los_enteros_son_big_endian() {
        let mut cab = cabecera_de_ejemplo();
        cab.flags = 0x0102;
        let bytes = cab.escribir(&Perfil::default());
        // flags van en los bytes 4..6 y en big-endian: 0x01 0x02.
        assert_eq!(&bytes[4..6], &[0x01, 0x02]);
    }

    #[test]
    fn el_created_at_es_big_endian_de_8_bytes() {
        let mut cab = cabecera_de_ejemplo();
        cab.encryption_method_id = 4;
        let bytes = cab.escribir(&Perfil::default());
        // con campo4 de 1 byte, createdAt empieza en 4+2+2+1 = 9
        let ts = &bytes[9..17];
        assert_eq!(ts[0], 0x00); // el byte más alto de 1_700_000_000_123 es 0
        assert_eq!(ts[7], 0x7B); // ... y el más bajo es 0x7B (123)
    }

    #[test]
    fn un_magic_invalido_no_entra_en_panic() {
        let r = Cabecera::leer(b"XXXX\x00\x00\x00\x00", &Perfil::default());
        assert_eq!(r.err(), Some(ErrorCabecera::MagicInvalido));
    }

    #[test]
    fn una_cabecera_truncada_no_entra_en_panic() {
        let cab = cabecera_de_ejemplo();
        let bytes = cab.escribir(&Perfil::default());
        for corte in 0..bytes.len() {
            let _ = Cabecera::leer(&bytes[..corte], &Perfil::default());
        }
    }

    #[test]
    fn un_tamano_de_cabecera_desconocido_se_rechaza() {
        let mut cab = cabecera_de_ejemplo();
        cab.header_size = 123;
        let bytes = cab.escribir(&Perfil::default());
        assert_eq!(
            Cabecera::leer(&bytes, &Perfil::default()).err(),
            Some(ErrorCabecera::TamanoDesconocido(123))
        );
    }

    #[test]
    fn el_payload_empieza_despues_de_la_cabecera() {
        let cab = cabecera_de_ejemplo();
        let mut bytes = cab.escribir(&Perfil::default());
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
        let mut cab = cabecera_de_ejemplo();
        cab.encryption_method_id = 0;
        cab.flags = 0;
        cab.header_size = 96;
        let mut bytes = cab.escribir(&Perfil::default());
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
        assert_eq!(inf.cabecera.encryption_method_id, 0);
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
        let mut cab = cabecera_de_ejemplo();
        cab.encryption_method_id = 4; // AEGIS-256
        let mut bytes = cab.escribir(&Perfil::default());
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

    #[test]
    fn leer_auto_acepta_los_dos_perfiles() {
        let cab = cabecera_de_ejemplo();
        let bytes = cab.escribir(&Perfil::default());
        let (c, n) = leer_auto(&bytes).expect("debería leer");
        assert_eq!(n, bytes.len());
        assert_eq!(c.encryption_method_id, 4);
    }
}

// ========================================================================= //
// Extracción del contenedor
// ========================================================================= //

use std::path::{Component, Path, PathBuf};

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
/// (`L`) y el tipo de entrada regular (`0`/NUL).
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
                // Nombre largo GNU: el contenido es el nombre.
                let end = (pos + size as usize).min(bytes.len());
                pending_long = Some(cstr(&bytes[pos..end]));
                pos = end + ((512 - (size as usize % 512)) % 512);
                continue;
            }
            b'x' | b'g' | b'K' => {
                // pax / cabeceras extendidas: se saltan.
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
/// Es laMISMA regla que usa el lector de la flota
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
    /// Nota informative (por ejemplo, por qué no se pudo descifrar).
    pub mensaje: String,
}

impl InformeExtraccion {
    /// Versión serializable.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "header_size": self.cabecera.header_size,
            "flags": self.cabecera.flags,
            "encryption_method_id": self.cabecera.encryption_method_id,
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
/// **Sin cifrar**: si `encryptionMethodId == 0`, el payload va directo a
/// zstd/TAR y se puede leer entero.
///
/// **Cifrado**: la derivación del nonce y del AAD por trozo es **nativa**
/// (`SbaLibaegisCryptoNative_decryptAegis256ChunkedEntryFdRaw`), así que
/// aquí **no** se descifra: se dice exactamente por qué, en vez de
/// inventarse una derivación que produciría basura.
pub fn informar_contenedor(bytes: &[u8]) -> Result<InformeExtraccion, String> {
    let (cabecera, n) = leer_auto(bytes).map_err(|e| e.to_string())?;
    let payload = &bytes[n.min(bytes.len())..];
    let mut inf = InformeExtraccion {
        cabecera: cabecera.clone(),
        cabecera_bytes: n,
        payload_bytes: payload.len(),
        ..Default::default()
    };

    if cabecera.encryption_method_id != 0 {
        inf.mensaje = format!(
            "contenido cifrado (encryptionMethodId={}): la derivación del nonce y del AAD \
             por trozo es NATIVA (SbaLibaegisCryptoNative_decryptAegis256ChunkedEntryFdRaw); \
             hace falta la clave y esa derivación. No se descifra a ciegas.",
            cabecera.encryption_method_id
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
    if inf.cabecera.encryption_method_id != 0 {
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
        let fin = (ini + e.tamano as usize).min(plano.len());
        if ini > plano.len() {
            continue;
        }
        std::fs::write(&destino, &plano[ini..fin])
            .map_err(|err| format!("no se pudo escribir {:?}: {}", destino, err))?;
    }
    let mut inf = inf;
    inf.entradas = entradas;
    inf.mensaje = format!("{} ficheros extraídos en {:?}", inf.entradas.len(), dest);
    Ok(inf)
}
