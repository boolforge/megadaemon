//! **Contenedor `SBA1` de Swift Backup** (`.dat` / `.med`).
//!
//! Lo que está **medido** (en el escritor nativo ARM64 de
//! `libsba_archive.so`, y en los `.cls`/`.msg` del propio móvil):
//! * magic `SBA1` (y `SBA_SWIFTTAR` para el payload);
//! * **cabecera BIG-ENDIAN** de 96 o 144 bytes ⇒ mismo orden de campos en
//!   x86_64 y ARM64 ⇒ un único lector;
//! * 12 campos, entre ellos `kdfMethodId` (`Argon2id` = 1, `Aegis256` = 4),
//!   `kdfIterations` y `payloadChunkSize`;
//! * payload = **TAR + ZSTD**;
//! * cifrado por trozos **AEGIS-256** (clave 32 B, nonce 32 B) con etiquetas
//!   de dominio `SBA1-AEGIS256-entry-chunk-aad-v1` / `…-nonce-v1`.
//!
//! Lo que **NO** está cerrado (y por eso este módulo es honesto): el ancho
//! exacto de cada campo y su desplazamiento dentro de la cabecera, el mapeo a
//! los nombres del modelo Kotlin y la derivación **nativa** de nonce/AAD por
//! trozo. Sin un `.dat` real, la cabecera se **inspecciona**, no se congela:
//! `leer_cabecera` devuelve los bytes crudos y las candidatas de campo, para
//! que al llegar un fichero real se pueda confirmar o refutar el layout.
//!
//! Lo que **sí** está resuelto y es usable hoy: el payload es TAR dentro de
//! ZSTD, así que `leer_tar_zstd` extrae el inventario de ficheros de un
//! backup de Swift sin depender del cifrado.

use super::zstd;
use std::path::Path;

/// Una entrada del TAR que hay dentro del contenedor de Swift.
#[derive(Debug, Clone)]
pub struct EntradaTar {
    /// Ruta dentro del archivo (ya saneada: sin `..` ni absolutas).
    pub nombre: String,
    /// Tipo: `b'0'` fichero regular, `b'5'` directorio, etc.
    pub tipo: u8,
    /// Tamaño declarado en la cabecera.
    pub tamano: usize,
    /// Contenido (vacío para directorios).
    pub datos: Vec<u8>,
}

/// Lector TAR mínimo (ustar + GNU longname) **autocontenido**.
///
/// No usa `backup_formats` a propósito: este módulo debe poder compilarse y
/// probarse en un arnés suelto (`rustc --test`), sin `serde` ni `flate2`.
/// El lector general de TAR/ZIP sigue en `backup_formats`.
pub fn leer_tar(bytes: &[u8]) -> Result<Vec<EntradaTar>, String> {
    let mut entradas = Vec::new();
    let mut off = 0usize;
    let mut nombre_largo: Option<String> = None;

    while off + 512 <= bytes.len() {
        let h = &bytes[off..off + 512];
        if h.iter().all(|b| *b == 0) {
            break; // bloque de ceros = fin del archivo
        }
        let nombre = match nombre_largo.take() {
            Some(n) => n,
            None => {
                let bruto = String::from_utf8_lossy(&h[..100])
                    .trim_end_matches('\0')
                    .trim_end()
                    .to_string();
                if bruto.is_empty() {
                    break;
                }
                bruto
            }
        };
        let prefijo = String::from_utf8_lossy(&h[345..500])
            .trim_end_matches('\0')
            .trim_end()
            .to_string();
        let completo = if prefijo.is_empty() {
            nombre
        } else {
            format!("{prefijo}/{nombre}")
        };
        let tipo = h[156];
        let tamano = octal(&h[124..136]).unwrap_or(0);

        off += 512;
        let fin = (off + tamano).min(bytes.len());
        let datos = bytes[off..fin].to_vec();
        off += tamano.div_ceil(512) * 512;

        // EntradaGNU "L": el nombre real va en el siguiente bloque.
        if tipo == b'L' {
            nombre_largo = Some(
                String::from_utf8_lossy(&datos)
                    .trim_end_matches('\0')
                    .trim_end()
                    .to_string(),
            );
            continue;
        }
        // "x"/"g": cabeceras extendidas (pax); se ignoran.
        if tipo == b'x' || tipo == b'g' {
            continue;
        }

        let limpio = sanear_ruta(&completo)?;
        entradas.push(EntradaTar {
            nombre: limpio,
            tipo,
            tamano,
            datos,
        });
    }
    Ok(entradas)
}

/// Octal ASCII (soporta el espacio inicial y el byte nulo final).
fn octal(b: &[u8]) -> Option<usize> {
    let s: String = b
        .iter()
        .take_while(|c| **c != 0 && **c != b' ')
        .map(|c| *c as char)
        .collect();
    if s.is_empty() {
        return Some(0);
    }
    usize::from_str_radix(&s, 8).ok()
}

/// Sanea una ruta del archivo: sin absolutas, sin `..`, sin barras dobles.
pub fn sanear_ruta(nombre: &str) -> Result<String, String> {
    if nombre.starts_with('/') || nombre.contains(':') {
        return Err(format!("ruta no permitida: {nombre}"));
    }
    let mut partes = Vec::new();
    for p in nombre.split('/') {
        match p {
            "" | "." => continue,
            ".." => return Err(format!("ruta con ..: {nombre}")),
            otro => partes.push(otro),
        }
    }
    if partes.is_empty() {
        return Err(format!("ruta vacía: {nombre}"));
    }
    Ok(partes.join("/"))
}

/// Magic del contenedor de Swift Backup.
pub const SBA1_MAGIC: &[u8; 4] = b"SBA1";
/// Magic del payload TAR dentro del contenedor.
pub const SBA_SWIFTTAR_MAGIC: &[u8; 12] = b"SBA_SWIFTTAR";
/// Tamaños de cabecera medidos en el escritor nativo.
pub const TAMANOS_CABECERA: &[usize] = &[96, 144];
/// Etiquetas de dominio para el AAD y el nonce de cada trozo (medidas).
pub const ETIQUETA_AAD: &str = "SBA1-AEGIS256-entry-chunk-aad-v1";
pub const ETIQUETA_NONCE: &str = "SBA1-AEGIS256-entry-chunk-nonce-v1";
/// Identificadores del método KDF medidos en el escritor.
pub const KDF_ARGON2ID: u32 = 1;
pub const KDF_AEGIS256: u32 = 4;

/// Cabecera de un contenedor `SBA1`, **sin interpretar los campos**.
///
/// `campos` son las candidatas big-endian que el módulo extrae del bloque de
/// cabecera (offset, valor). Están para poder casarlas con un fichero real,
/// no como formato congelado.
#[derive(Debug, Clone)]
pub struct CabeceraSba1 {
    /// Magic detectado: `SBA1` o `SBA_SWIFTTAR`.
    pub magic: String,
    /// Tamaño de la cabecera deducido (96, 144 o el declarado).
    pub tamano: usize,
    /// Bytes crudos de la cabecera.
    pub crudo: Vec<u8>,
    /// Candidatas big-endian: (desplazamiento, valor).
    pub campos: Vec<(usize, u64)>,
}

/// ¿El fichero empieza por un magic de Swift Backup?
pub fn es_sba1(bytes: &[u8]) -> bool {
    bytes.starts_with(SBA1_MAGIC) || bytes.starts_with(SBA_SWIFTTAR_MAGIC)
}

/// Lee la cabecera de un contenedor `SBA1`.
///
/// No interpreta los campos: los **inspecciona**. El tamaño de cabecera se
/// deduce del propio bloque (el valor 96 o 144 aparece como u16/u32
/// big-endian justo tras el magic) y, si no aparece, se asume 96.
pub fn leer_cabecera(bytes: &[u8]) -> Result<CabeceraSba1, String> {
    // El magic es el discriminante principal: se mira antes que la longitud
    // para que el error diga *qué* no es, no sólo cuánto mide.
    let magic = if bytes.starts_with(SBA1_MAGIC) {
        "SBA1".to_string()
    } else if bytes.starts_with(SBA_SWIFTTAR_MAGIC) {
        "SBA_SWIFTTAR".to_string()
    } else if bytes.len() < 4 {
        return Err(format!("el fichero es demasiado corto ({} bytes)", bytes.len()));
    } else {
        return Err(format!(
            "magic desconocido: {:02x?} (se esperaba SBA1 o SBA_SWIFTTAR)",
            &bytes[..4]
        ));
    };
    if bytes.len() < 16 {
        return Err(format!("cabecera truncada: sólo {} bytes", bytes.len()));
    }

    // Tamaño de cabecera: se busca 96 o 144 como entero big-endian en los
    // primeros 16 bytes (offsets 4, 6 y 8 son los candidatos del escritor).
    let mut tamano = TAMANOS_CABECERA[0];
    for off in [4usize, 6, 8] {
        if off + 2 <= bytes.len() {
            let v = u16::from_be_bytes([bytes[off], bytes[off + 1]]) as usize;
            if TAMANOS_CABECERA.contains(&v) {
                tamano = v;
                break;
            }
        }
    }
    if let Some(v) = off_declara96_o_144(bytes) {
        tamano = v;
    }

    let crudo = bytes[..tamano.min(bytes.len())].to_vec();
    let campos = candidatas_big_endian(&crudo);
    Ok(CabeceraSba1 {
        magic,
        tamano,
        crudo,
        campos,
    })
}

/// Busca un u32/u64 big-endian que valga 96 o 144 cerca del principio.
fn off_declara96_o_144(bytes: &[u8]) -> Option<usize> {
    for off in 0..bytes.len().min(16).saturating_sub(3) {
        let v = u32::from_be_bytes([
            bytes[off],
            bytes[off + 1],
            bytes[off + 2],
            bytes[off + 3],
        ]);
        if v == 96 || v == 144 {
            return Some(v as usize);
        }
    }
    None
}

/// Candidatas de campo big-endian: u32 en cada offset par y u64 en cada
/// offset múltiplo de 8. Sirve para casar con un fichero real.
fn candidatas_big_endian(crudo: &[u8]) -> Vec<(usize, u64)> {
    let mut out = Vec::new();
    for off in (0..crudo.len().saturating_sub(3)).step_by(2) {
        let v = u32::from_be_bytes([crudo[off], crudo[off + 1], crudo[off + 2], crudo[off + 3]]);
        out.push((off, v as u64));
    }
    for off in (0..crudo.len().saturating_sub(7)).step_by(8) {
        let mut b = [0u8; 8];
        b.copy_from_slice(&crudo[off..off + 8]);
        out.push((off, u64::from_be_bytes(b)));
    }
    out
}

/// Localiza el payload (TAR comprimido) a partir del tamaño de cabecera.
pub fn localizar_payload<'a>(bytes: &'a [u8], tamano_cabecera: usize) -> Option<&'a [u8]> {
    if bytes.len() <= tamano_cabecera {
        return None;
    }
    Some(&bytes[tamano_cabecera..])
}

/// Descomprime el payload (ZSTD) y devuelve el TAR plano.
pub fn descomprimir_payload(bytes: &[u8], max_salida: usize) -> Result<Vec<u8>, String> {
    let cab = leer_cabecera(bytes)?;
    let payload = localizar_payload(bytes, cab.tamano)
        .ok_or_else(|| "el fichero no tiene payload tras la cabecera".to_string())?;
    zstd::descomprimir(payload, max_salida)
}

/// Lee el inventario de ficheros de un backup de Swift: ZSTD → TAR → entradas.
pub fn leer_tar_zstd(bytes: &[u8], max_salida: usize) -> Result<Vec<EntradaTar>, String> {
    let tar = descomprimir_payload(bytes, max_salida)?;
    leer_tar(&tar)
}

/// Extrae el contenido de un backup de Swift a disco (ZSTD → TAR → ficheros).
pub fn extraer_tar_zstd(bytes: &[u8], dest: &Path, max_salida: usize) -> Result<Vec<String>, String> {
    let tar = descomprimir_payload(bytes, max_salida)?;
    let mut escritos = Vec::new();
    for e in leer_tar(&tar)? {
        if e.tipo != b'0' {
            continue; // sólo ficheros regulares
        }
        let rel = std::path::PathBuf::from(&e.nombre);
        let destino = dest.join(&rel);
        if let Some(dir) = destino.parent() {
            std::fs::create_dir_all(dir).map_err(|er| format!("mkdir {:?}: {er}", dir))?;
        }
        std::fs::write(&destino, &e.datos).map_err(|er| format!("write {:?}: {er}", destino))?;
        escritos.push(rel.to_string_lossy().to_string());
    }
    Ok(escritos)
}

/// Informe legible de un contenedor: qué magic, qué tamaño de cabecera y qué
/// candidatas de campo. Pensado para pegar en un informe cuando aparezca un
/// `.dat` real.
pub fn informe_contenedor(bytes: &[u8]) -> Result<String, String> {
    let cab = leer_cabecera(bytes)?;
    let mut s = String::new();
    s.push_str(&format!("magic: {}\n", cab.magic));
    s.push_str(&format!("tamaño de cabecera: {}\n", cab.tamano));
    s.push_str(&format!("bytes crudos: {}\n", hex(&cab.crudo)));
    s.push_str("candidatas big-endian (offset: valor):\n");
    for (off, v) in &cab.campos {
        // Sólo las que "parecen" campos: no 0 ni valores absurdos.
        if *v != 0 && *v < 1 << 32 {
            s.push_str(&format!("  {off:3}: {v}\n"));
        }
    }
    Ok(s)
}

/// Hexadecimal en minúsculas.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
