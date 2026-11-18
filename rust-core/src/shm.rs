use std::fs::File;
use std::io::Read;
use serde::{Deserialize, Serialize};

pub const SHM_PATH: &str = "/data/user/0/com.agy/files/snatcher_vla_shm.bin";
pub const SHM_SIZE: usize = 1048576; // 1 MB

#[derive(Debug, Serialize, Deserialize)]
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

    if buf.len() < 28 {
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

    let payload = &buf[32824..];
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
pub fn encode_bmp_24(width: u32, height: u32, rgb_data: &[u8]) -> Vec<u8> {
    let row_size = ((width * 3 + 3) / 4) * 4; // 4-byte aligned rows
    let pixel_array_size = row_size * height;
    let file_size = 54 + pixel_array_size;

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
    let padding = (row_size - width * 3) as usize;
    for y in (0..height).rev() {
        let row_start = (y * width * 3) as usize;
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
