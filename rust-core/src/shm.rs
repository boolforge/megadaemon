use std::fs::File;
use std::io::Read;
use serde::{Deserialize, Serialize};

pub const SHM_PATH: &str = "/data/user/0/com.agy/files/snatcher_vla_shm.bin";
pub const SHM_SIZE: usize = 1048576; // 1 MB
/// Offset del payload del frame dentro del fichero SHM (cabecera + metadatos).
pub const FRAME_OFFSET: usize = 32824;
/// Límite de dimensiones del frame.  La cabecera la escribe la app Android y
/// es un dato NO CONFIABLE: sin este tope, 65535x65535 pedía 12,9 GB de RAM
/// y mataba el proceso (DoS trivial con 28 bytes).
pub const MAX_FRAME_DIM: u32 = 4096;
/// Límite de megapíxeles (8 MP) y de bytes de payload.
pub const MAX_FRAME_PIXELS: u64 = 8 * 1024 * 1024;
pub const MAX_FRAME_BYTES: u64 = 32 * 1024 * 1024;
/// Longitud de la cabecera (magic..format).
pub const HEADER_SIZE: usize = 28;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShmHeader {
    pub magic: u32,
    pub version: u32,
    pub frame_counter: u32,
    pub timestamp_us: u64,
    pub width: u16,
    pub height: u16,
    pub bpp: u16,
    pub format: u16,
    pub nonzero_bytes: usize,
    pub is_active: bool,
}

pub fn read_shm_status() -> Result<ShmHeader, String> {
    let mut file = File::open(SHM_PATH).map_err(|e| format!("Failed to open SHM: {}", e))?;
    let mut buf = vec![0u8; SHM_SIZE];
    file.read_exact(&mut buf)
        .map_err(|e| format!("Failed to read SHM: {}", e))?;

    if buf.len() < HEADER_SIZE {
        return Err("SHM file too small for header".to_string());
    }

    let magic = u32::from_le_bytes(buf[0..4].try_into().unwrap());
    let version = u32::from_le_bytes(buf[4..8].try_into().unwrap());
    let frame_counter = u32::from_le_bytes(buf[8..12].try_into().unwrap());
    let timestamp_us = u64::from_le_bytes(buf[12..20].try_into().unwrap());
    let width = u16::from_le_bytes(buf[20..22].try_into().unwrap());
    let height = u16::from_le_bytes(buf[22..24].try_into().unwrap());
    let bpp = u16::from_le_bytes(buf[24..26].try_into().unwrap());
    let format = u16::from_le_bytes(buf[26..28].try_into().unwrap());

    if buf.len() <= FRAME_OFFSET {
        return Err("SHM file too small for frame payload".to_string());
    }
    let payload = &buf[FRAME_OFFSET..];
    let nonzero = payload.iter().filter(|&&b| b != 0).count();

    Ok(ShmHeader {
        magic,
        version,
        frame_counter,
        timestamp_us,
        width,
        height,
        bpp,
        format,
        nonzero_bytes: nonzero,
        is_active: nonzero > 0 && magic == 0x534E4154,
    })
}

/// Generates a valid 24-bit uncompressed Windows BMP file header + pixel data
///
/// v14 (UI-03 · CRÍTICA): con `width=65535, height=65535` el cálculo
/// `row_size * height` desborda u32 y, en release (`panic = "abort"`), el
/// panic **mataba el proceso entero del cockpit** ⇒ ERR_CONNECTION_REFUSED
/// para el usuario.  Un header SHM hostil no puede tumbar el daemon, así que
/// aquí se satura y se acota la imagen en lugar de desbordar.
pub fn encode_bmp_24(width: u32, height: u32, rgb_data: &[u8]) -> Vec<u8> {
    // Acota las dimensiones para que ningún producto intermedio desborde.
    const MAX_DIM: u32 = 16_384;
    let w = width.clamp(1, MAX_DIM);
    let h = height.clamp(1, MAX_DIM);
    let row_size = ((w as u64 * 3 + 3) / 4) * 4; // filas alineadas a 4 bytes
    let pixel_array_size = row_size.saturating_mul(h as u64);
    // Tope duro del BMP: 64 MiB.  Por encima, la cabecera se recorta.
    let pixel_array_size = pixel_array_size.min(64 * 1024 * 1024) as u32;
    let file_size = 54u32.saturating_add(pixel_array_size);
    let width = w;
    let height = h;

    let mut bmp = Vec::with_capacity(file_size as usize);

    // Bitmap File Header (14 bytes)
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&file_size.to_le_bytes());
    bmp.extend_from_slice(&[0, 0, 0, 0]); // Reserved
    bmp.extend_from_slice(&54u32.to_le_bytes()); // Offset to pixel data

    // DIB Header - BITMAPINFOHEADER (40 bytes)
    bmp.extend_from_slice(&40u32.to_le_bytes());
    bmp.extend_from_slice(&(width as i32).to_le_bytes());
    bmp.extend_from_slice(&(height as i32).to_le_bytes()); // Positive height = bottom-up
    bmp.extend_from_slice(&1u16.to_le_bytes()); // Color planes
    bmp.extend_from_slice(&24u16.to_le_bytes()); // 24-bit BGR
    bmp.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB (uncompressed)
    bmp.extend_from_slice(&pixel_array_size.to_le_bytes());
    bmp.extend_from_slice(&2835u32.to_le_bytes()); // 72 DPI X
    bmp.extend_from_slice(&2835u32.to_le_bytes()); // 72 DPI Y
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());

    // Pixel data (BGR bottom-up)
    let padding = (row_size - (width as u64 * 3)) as usize;
    // Tope de píxeles realmente escritos: una imagen hostil no puede hacer
    // que el daemon reserve/rellene gigabytes.
    const MAX_PIXELS: u64 = 4_194_304; // 4 Mpx (~12 MiB de píxeles)
    let max_rows = (MAX_PIXELS / (width as u64).max(1)).max(1).min(height as u64);
    for y in (0..height).rev() {
        if (height as u64 - y as u64) > max_rows {
            break;
        }
        let row_start = (y as u64 * width as u64 * 3) as usize;
        for x in 0..width as usize {
            let px_idx = row_start + x * 3;
            if px_idx + 2 < rgb_data.len() {
                let r = rgb_data[px_idx];
                let g = rgb_data[px_idx + 1];
                let b = rgb_data[px_idx + 2];
                bmp.push(b);
                bmp.push(g);
                bmp.push(r);
            } else {
                bmp.extend_from_slice(&[0, 0, 0]);
            }
        }
        for _ in 0..padding {
            bmp.push(0);
        }
    }

    bmp
}

/// Procedural Kobe 2042 Radar Tactical HUD in pure Rust
pub fn generate_tactical_hud_bmp(width: u32, height: u32) -> Vec<u8> {
    let mut rgb = vec![0u8; (width * height * 3) as usize];
    let cx = width as i32 / 2;
    let cy = height as i32 / 2;

    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let idx = ((y * width as i32 + x) * 3) as usize;

            // Background dark tactical green
            let mut r = 3u8;
            let mut g = 8u8;
            let mut b = 5u8;

            // Grid lines every 32px
            if x % 32 == 0 || y % 32 == 0 {
                r = 14;
                g = 46;
                b = 24;
            }

            // Radar circles
            let dx = x - cx;
            let dy = y - cy;
            let dist_sq = dx * dx + dy * dy;
            let r1 = 80;
            let r2 = 160;

            if (dist_sq >= (r1 - 1) * (r1 - 1) && dist_sq <= (r1 + 1) * (r1 + 1))
                || (dist_sq >= (r2 - 1) * (r2 - 1) && dist_sq <= (r2 + 1) * (r2 + 1))
            {
                r = 26;
                g = 85;
                b = 40;
            }

            // Crosshairs
            if (dy.abs() <= 1 && dx.abs() <= 40) || (dx.abs() <= 1 && dy.abs() <= 40) {
                r = 0;
                g = 255;
                b = 102;
            }

            rgb[idx] = r;
            rgb[idx + 1] = g;
            rgb[idx + 2] = b;
        }
    }

    encode_bmp_24(width, height, &rgb)
}

/// Instantánea del frame leída de forma **coherente**.
///
/// La v13.2 leía el fichero SHM dos veces por análisis (2 MB de E/S) y sin
/// comprobar que el frame no hubiera cambiado a mitad de la copia: el
/// productor (la app Android) escribe mientras nosotros leemos, así que
/// podíamos analizar un frame "a medio escribir".
///
/// Ahora: una sola lectura del payload + lectura del contador antes y después.
/// Si el contador cambió, el frame estaba cambiando: se marca `torn` y el
/// análisis lo declara en vez de dar un resultado silenciosamente corrupto.
#[derive(Debug, Clone)]
pub struct FrameSnapshot {
    pub header: ShmHeader,
    pub rgb: Vec<u8>,
    /// `true` si el frame cambió mientras se copiaba.
    pub torn: bool,
}

/// Lee cabecera + frame con verificación de coherencia.
pub fn read_frame_snapshot() -> Result<FrameSnapshot, String> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = File::open(SHM_PATH).map_err(|e| format!("Failed to open SHM: {}", e))?;

    // 1) Cabecera "antes".
    let mut head = [0u8; HEADER_SIZE];
    file.seek(SeekFrom::Start(0))
        .and_then(|_| file.read_exact(&mut head))
        .map_err(|e| format!("SHM header read: {}", e))?;

    let width = u16::from_le_bytes(head[20..22].try_into().unwrap()) as usize;
    let height = u16::from_le_bytes(head[22..24].try_into().unwrap()) as usize;
    if width == 0 || height == 0 {
        return Err("SHM header reports an empty frame".to_string());
    }
    // La cabecera no es de fiar: validar ANTES de reservar memoria.
    if width as u32 > MAX_FRAME_DIM || height as u32 > MAX_FRAME_DIM {
        return Err(format!(
            "dimensiones del frame fuera de rango: {}x{} (max {}x{})",
            width, height, MAX_FRAME_DIM, MAX_FRAME_DIM
        ));
    }
    let pixels = (width as u64) * (height as u64);
    if pixels > MAX_FRAME_PIXELS {
        return Err(format!(
            "frame con {} pixeles (max {})",
            pixels, MAX_FRAME_PIXELS
        ));
    }
    let frame_len = (pixels * 3) as usize;
    if frame_len as u64 > MAX_FRAME_BYTES {
        return Err(format!("frame de {} bytes (max {})", frame_len, MAX_FRAME_BYTES));
    }
    // Y no puede exceder lo que realmente hay en el fichero.
    let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);
    if (FRAME_OFFSET as u64) + frame_len as u64 > file_len {
        return Err(format!(
            "el frame declarado ({} bytes) no cabe en el fichero ({} bytes)",
            frame_len, file_len
        ));
    }

    // 2) Payload (una sola copia).
    let mut rgb = vec![0u8; frame_len];
    file.seek(SeekFrom::Start(FRAME_OFFSET as u64))
        .and_then(|_| file.read_exact(&mut rgb))
        .map_err(|e| format!("SHM frame read: {}", e))?;

    // 3) Cabecera "después": si el contador cambió, el frame estaba en
    //    mitad de actualización.
    let mut head2 = [0u8; HEADER_SIZE];
    file.seek(SeekFrom::Start(0))
        .and_then(|_| file.read_exact(&mut head2))
        .map_err(|e| format!("SHM header re-read: {}", e))?;
    let torn = head2[8..12] != head[8..12];

    let header = ShmHeader {
        magic: u32::from_le_bytes(head[0..4].try_into().unwrap()),
        version: u32::from_le_bytes(head[4..8].try_into().unwrap()),
        frame_counter: u32::from_le_bytes(head2[8..12].try_into().unwrap()),
        timestamp_us: u64::from_le_bytes(head2[12..20].try_into().unwrap()),
        width: width as u16,
        height: height as u16,
        bpp: u16::from_le_bytes(head2[24..26].try_into().unwrap()),
        format: u16::from_le_bytes(head2[26..28].try_into().unwrap()),
        nonzero_bytes: 0,
        is_active: true,
    };

    Ok(FrameSnapshot { header, rgb, torn })
}
