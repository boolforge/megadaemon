//! MegaDaemon v13.0 Zenith — Tile Molester / YY-CHR / CrystalTile2 Native Rust Engine
//! Visualizes ANY binary file as raw VRAM (1bpp, 2bpp planar, 4bpp linear, 4bpp planar,
//! 8bpp linear, 15bpp RGB555) with real-time offset, tile grid width, stride, and hardware palettes.

use crate::shm;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

fn build_palette(mode: &str, file_data: &[u8], pal_offset: usize) -> Vec<[u8; 3]> {
    let mut pal = vec![[0u8; 3]; 256];
    match mode {
        "obsidian_mint" => {
            for i in 0..256 {
                let t = i as f32 / 255.0;
                pal[i] = [
                    (8.0 + t * 20.0) as u8,
                    (12.0 + t * 233.0) as u8,
                    (18.0 + t * 140.0) as u8,
                ];
            }
        }
        "sega_cram_9bit" => {
            // Sega Genesis / Mega-CD CRAM format: 0000 BBB0 GGG0 RRR0 (16-bit BE)
            if pal_offset + 32 <= file_data.len() {
                for i in 0..16 {
                    let w = u16::from_be_bytes([
                        file_data[pal_offset + i * 2],
                        file_data[pal_offset + i * 2 + 1],
                    ]);
                    let r = (((w >> 1) & 0x7) * 36) as u8;
                    let g = (((w >> 5) & 0x7) * 36) as u8;
                    let b = (((w >> 9) & 0x7) * 36) as u8;
                    pal[i] = [r, g, b];
                }
                for i in 16..256 {
                    pal[i] = pal[i % 16];
                }
            } else {
                for i in 0..16 {
                    let v = (i * 17) as u8;
                    pal[i] = [v / 2, v, (v as u16 * 3 / 4) as u8];
                }
            }
        }
        "ps1_clut_15bit" => {
            // PS1 / Saturn RGB555 CLUT: 0BBBBBGGGGGRRRRR (16-bit LE)
            if pal_offset + 32 <= file_data.len() {
                for i in 0..16 {
                    let w = u16::from_le_bytes([
                        file_data[pal_offset + i * 2],
                        file_data[pal_offset + i * 2 + 1],
                    ]);
                    let r = ((w & 0x1F) << 3) as u8;
                    let g = (((w >> 5) & 0x1F) << 3) as u8;
                    let b = (((w >> 10) & 0x1F) << 3) as u8;
                    pal[i] = [r, g, b];
                }
                for i in 16..256 {
                    pal[i] = pal[i % 16];
                }
            } else {
                for i in 0..16 {
                    let v = (i * 17) as u8;
                    pal[i] = [v, v / 2, (255 - v) / 2];
                }
            }
        }
        "vga_heat" => {
            for i in 0..256 {
                let r = if i < 128 { (i * 2) as u8 } else { 255 };
                let g = if i < 64 {
                    0
                } else if i < 192 {
                    ((i - 64) * 2) as u8
                } else {
                    255
                };
                let b = if i < 192 { (i / 2) as u8 } else { ((i - 192) * 4) as u8 };
                pal[i] = [r, g, b];
            }
        }
        "cga_cyber" => {
            let base: [[u8; 3]; 16] = [
                [8, 12, 18],
                [0, 120, 160],
                [0, 210, 255],
                [0, 245, 155],
                [40, 60, 90],
                [130, 70, 220],
                [255, 184, 0],
                [230, 240, 255],
                [25, 36, 52],
                [0, 175, 220],
                [255, 45, 85],
                [110, 255, 210],
                [255, 100, 130],
                [190, 120, 255],
                [255, 225, 100],
                [255, 255, 255],
            ];
            for i in 0..256 {
                pal[i] = base[i % 16];
            }
        }
        _ => {
            // High-contrast grayscale
            for i in 0..256 {
                let v = i as u8;
                pal[i] = [v, v, v];
            }
        }
    }
    pal
}

/// Renders raw binary bytes into a crisp 24-bit BMP tile sheet (Tile Molester / CrystalTile2 style)
pub fn render_vram_bmp(
    file_path: &str,
    offset: u64,
    codec: &str,
    tiles_x: usize,
    tiles_y: usize,
    palette_mode: &str,
    pal_offset: usize,
    scale: usize,
) -> Result<Vec<u8>, String> {
    let tx = tiles_x.clamp(4, 32);
    let ty = tiles_y.clamp(4, 32);
    let sc = scale.clamp(1, 4);

    let bytes_per_tile = match codec {
        "1bpp" => 8,
        "2bpp_planar" => 16,
        "4bpp_linear" | "4bpp_planar" => 32,
        "8bpp_linear" => 64,
        "15bpp_rgb555" => 128,
        _ => 32,
    };
    let needed = tx * ty * bytes_per_tile;

    let mut f = File::open(file_path).map_err(|e| format!("Cannot open {}: {}", file_path, e))?;
    let fsize = f.metadata().map(|m| m.len()).unwrap_or(0);
    let clamped_off = offset.min(fsize);
    let _ = f.seek(SeekFrom::Start(clamped_off));
    let mut data = vec![0u8; needed];
    let nread = f.read(&mut data).unwrap_or(0);
    data.truncate(nread.max(needed));
    if data.len() < needed {
        data.resize(needed, 0);
    }

    // Read small slice for optional hardware palette extraction
    let mut pal_src = Vec::new();
    if palette_mode == "sega_cram_9bit" || palette_mode == "ps1_clut_15bit" {
        if f.seek(SeekFrom::Start(pal_offset as u64)).is_ok() {
            let mut pb = vec![0u8; 64];
            let _ = f.read(&mut pb);
            pal_src = pb;
        }
    }
    let palette = build_palette(palette_mode, &pal_src, 0);

    let raw_w = tx * 8;
    let raw_h = ty * 8;
    let out_w = raw_w * sc;
    let out_h = raw_h * sc;
    let mut rgb = vec![0u8; out_w * out_h * 3];

    let mut put_px = |x: usize, y: usize, color: [u8; 3]| {
        for sy in 0..sc {
            for sx in 0..sc {
                let ox = x * sc + sx;
                let oy = y * sc + sy;
                if ox < out_w && oy < out_h {
                    let idx = (oy * out_w + ox) * 3;
                    rgb[idx] = color[0];
                    rgb[idx + 1] = color[1];
                    rgb[idx + 2] = color[2];
                }
            }
        }
    };

    for tile_idx in 0..(tx * ty) {
        let base = tile_idx * bytes_per_tile;
        let gx = (tile_idx % tx) * 8;
        let gy = (tile_idx / tx) * 8;

        for py in 0..8 {
            for px in 0..8 {
                let color: [u8; 3] = match codec {
                    "1bpp" => {
                        let b = data[base + py];
                        let bit = (b >> (7 - px)) & 1;
                        let idx = if bit == 1 { 240 } else { 12 };
                        palette[idx]
                    }
                    "2bpp_planar" => {
                        // NES / GB 2bpp planar (2 bytes per row)
                        let p0 = data[base + py * 2];
                        let p1 = data[base + py * 2 + 1];
                        let b0 = (p0 >> (7 - px)) & 1;
                        let b1 = (p1 >> (7 - px)) & 1;
                        let ci = ((b1 << 1) | b0) as usize;
                        palette[(ci * 85).min(255)]
                    }
                    "4bpp_planar" => {
                        // SNES / PC Engine 4bpp planar (planes 0,1 at +0..16, planes 2,3 at +16..32)
                        let p0 = data[base + py * 2];
                        let p1 = data[base + py * 2 + 1];
                        let p2 = data[base + 16 + py * 2];
                        let p3 = data[base + 16 + py * 2 + 1];
                        let shift = 7 - px;
                        let ci = (((p0 >> shift) & 1)
                            | (((p1 >> shift) & 1) << 1)
                            | (((p2 >> shift) & 1) << 2)
                            | (((p3 >> shift) & 1) << 3)) as usize;
                        let mapped = if palette_mode == "sega_cram_9bit"
                            || palette_mode == "ps1_clut_15bit"
                            || palette_mode == "cga_cyber"
                        {
                            ci
                        } else {
                            ci * 17
                        };
                        palette[mapped.min(255)]
                    }
                    "8bpp_linear" => {
                        let ci = data[base + py * 8 + px] as usize;
                        palette[ci]
                    }
                    "15bpp_rgb555" => {
                        let off2 = base + (py * 8 + px) * 2;
                        let w = u16::from_le_bytes([data[off2], data[off2 + 1]]);
                        let r = ((w & 0x1F) << 3) as u8;
                        let g = (((w >> 5) & 0x1F) << 3) as u8;
                        let b = (((w >> 10) & 0x1F) << 3) as u8;
                        [r, g, b]
                    }
                    _ => {
                        // Default 4bpp_linear (Sega MD / Sega CD / GBA / PS1 TIM)
                        let b = data[base + py * 4 + (px / 2)];
                        let ci = if px % 2 == 0 { (b >> 4) & 0x0F } else { b & 0x0F } as usize;
                        let mapped = if palette_mode == "sega_cram_9bit"
                            || palette_mode == "ps1_clut_15bit"
                            || palette_mode == "cga_cyber"
                        {
                            ci
                        } else {
                            ci * 17
                        };
                        palette[mapped.min(255)]
                    }
                };
                put_px(gx + px, gy + py, color);
            }
        }
    }

    Ok(shm::encode_bmp_24(out_w as u32, out_h as u32, &rgb))
}
