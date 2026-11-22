//! Pure-Rust Retro Graphics & Palette Inspectors:
//! - MSX2 / PC-88 / Game Boy 1bpp 8x8 tile & font glyph extractor
//! - Sega Saturn VDP2 CRAM 15-bit RGB555 palette decoder

use serde_json::{json, Value};

/// Extracts 1bpp 8x8 monochrome font/tile glyphs (MSX2 / PC-88 / Game Boy).
pub fn extract_1bpp_glyphs(data: &[u8], max_chars: usize) -> Value {
    let total = (data.len() / 8).min(max_chars);
    let mut glyphs = Vec::with_capacity(total);
    for idx in 0..total {
        let tile = &data[idx * 8..(idx + 1) * 8];
        let mut ascii_lines = Vec::with_capacity(8);
        for &row_byte in tile {
            let mut line = String::with_capacity(8);
            for bit in (0..8).rev() {
                line.push(if (row_byte >> bit) & 1 == 1 { '#' } else { '.' });
            }
            ascii_lines.push(line);
        }
        glyphs.push(json!({
            "index": idx,
            "offset": idx * 8,
            "ascii": ascii_lines,
        }));
    }
    json!({
        "ok": true,
        "format": "1bpp-8x8",
        "glyph_count": glyphs.len(),
        "glyphs": glyphs,
    })
}

/// Decodes Sega Saturn VDP2 CRAM 15-bit RGB555 color palettes (`0b0BBBBBGGGGGRRRRR`).
pub fn inspect_saturn_palette(data: &[u8], big_endian: bool) -> Value {
    let count = data.len() / 2;
    let mut colors = Vec::with_capacity(count);
    for i in 0..count {
        let s = &data[i * 2..i * 2 + 2];
        let word = if big_endian {
            u16::from_be_bytes([s[0], s[1]])
        } else {
            u16::from_le_bytes([s[0], s[1]])
        };
        let r5 = (word & 0x1F) as u32;
        let g5 = ((word >> 5) & 0x1F) as u32;
        let b5 = ((word >> 10) & 0x1F) as u32;
        let msb = (word >> 15) & 1;
        let r8 = (r5 * 255) / 31;
        let g8 = (g5 * 255) / 31;
        let b8 = (b5 * 255) / 31;
        colors.push(json!({
            "index": i,
            "raw_word": format!("0x{:04X}", word),
            "msb": msb,
            "hex": format!("#{:02X}{:02X}{:02X}", r8, g8, b8),
        }));
    }
    json!({
        "ok": true,
        "format": "saturn-cram-rgb555",
        "endian": if big_endian { "be" } else { "le" },
        "color_count": colors.len(),
        "colors": colors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_1bpp_glyph_and_saturn_palette() {
        let g = extract_1bpp_glyphs(&[0xFF, 0x81, 0x81, 0x81, 0x81, 0x81, 0x81, 0xFF], 4);
        assert_eq!(g["glyph_count"], 1);
        assert_eq!(g["glyphs"][0]["ascii"][0], "########");
        assert_eq!(g["glyphs"][0]["ascii"][1], "#......#");

        let pal = inspect_saturn_palette(&[0x00, 0x1F, 0x7F, 0xFF], true);
        assert_eq!(pal["color_count"], 2);
        assert_eq!(pal["colors"][0]["hex"], "#FF0000");
        assert_eq!(pal["colors"][1]["hex"], "#FFFFFF");
    }
}
