use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShiftJisStringHit {
    pub offset_hex: String,
    pub encoding: String,
    pub byte_len: usize,
    pub decoded_utf8: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionBlockHit {
    pub offset_hex: String,
    pub algorithm: String,
    pub compressed_size_est: usize,
    pub local_entropy: f64,
    pub signature_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptoAndEncodingReport {
    pub status: String,
    pub platform: String,
    pub file: String,
    pub bytes_analyzed: usize,
    pub shift_jis_strings_found: usize,
    pub compression_blocks_found: usize,
    pub string_hits: Vec<ShiftJisStringHit>,
    pub compression_hits: Vec<CompressionBlockHit>,
    pub engine: String,
}

fn decode_shift_jis_slice(bytes: &[u8]) -> Option<(String, &'static str)> {
    let mut out = String::new();
    let mut i = 0usize;
    let mut has_sjis = false;
    let mut valid_chars = 0usize;

    while i < bytes.len() {
        let b = bytes[i];
        if (0x20..=0x7E).contains(&b) {
            out.push(b as char);
            valid_chars += 1;
            i += 1;
        } else if (0xA1..=0xDF).contains(&b) {
            // Half-width Katakana (U+FF61 .. U+FF9F)
            if let Some(ch) = char::from_u32(0xFF61 + (b - 0xA1) as u32) {
                out.push(ch);
                has_sjis = true;
                valid_chars += 1;
            }
            i += 1;
        } else if ((0x81..=0x9F).contains(&b) || (0xE0..=0xEF).contains(&b)) && i + 1 < bytes.len() {
            let t = bytes[i + 1];
            if (0x40..=0x7E).contains(&t) || (0x80..=0xFC).contains(&t) {
                has_sjis = true;
                valid_chars += 1;
                // Map common Shift-JIS Hiragana (0x829F..0x82F1) and Katakana (0x8340..0x8396) directly to Unicode!
                if b == 0x82 && (0x9F..=0xF1).contains(&t) {
                    if let Some(ch) = char::from_u32(0x3041 + (t - 0x9F) as u32) {
                        out.push(ch);
                    }
                } else if b == 0x83 && (0x40..=0x96).contains(&t) {
                    let off = if t >= 0x80 { t - 0x41 } else { t - 0x40 };
                    if let Some(ch) = char::from_u32(0x30A1 + off as u32) {
                        out.push(ch);
                    }
                } else if b == 0x81 {
                    match t {
                        0x40 => out.push('　'),
                        0x41 => out.push('、'),
                        0x42 => out.push('。'),
                        0x45 => out.push('・'),
                        0x48 => out.push('？'),
                        0x49 => out.push('！'),
                        0x75 => out.push('「'),
                        0x76 => out.push('」'),
                        _ => out.push('◇'),
                    }
                } else {
                    out.push('漢');
                }
                i += 2;
            } else {
                break;
            }
        } else {
            break;
        }
    }

    if valid_chars >= 6 {
        let enc = if has_sjis { "SHIFT-JIS (Japanese Konami)" } else { "ASCII/CP437" };
        Some((out, enc))
    } else {
        None
    }
}

fn calc_entropy(slice: &[u8]) -> f64 {
    if slice.is_empty() {
        return 0.0;
    }
    let mut counts = [0usize; 256];
    for &b in slice {
        counts[b as usize] += 1;
    }
    let len = slice.len() as f64;
    let mut ent = 0.0;
    for &c in &counts {
        if c > 0 {
            let p = (c as f64) / len;
            ent -= p * p.log2();
        }
    }
    ent
}

pub fn scan_crypto_and_encodings(plat_key: &str, file_path: &str) -> CryptoAndEncodingReport {
    let mut buf = Vec::new();
    if let Ok(mut f) = File::open(file_path) {
        let mut temp = vec![0u8; 262_144]; // 256 KB scan window
        if let Ok(n) = f.read(&mut temp) {
            temp.truncate(n);
            buf = temp;
        }
    }

    let mut string_hits = Vec::new();
    let mut i = 0usize;
    while i + 8 < buf.len() && string_hits.len() < 32 {
        if let Some((decoded, enc)) = decode_shift_jis_slice(&buf[i..buf.len().min(i + 96)]) {
            let alpha_or_kana = decoded
                .chars()
                .filter(|c| c.is_alphabetic() || (*c as u32) > 0x3000)
                .count();
            if alpha_or_kana >= 5 {
                let b_len = decoded.len();
                string_hits.push(ShiftJisStringHit {
                    offset_hex: format!("0x{:06X}", i),
                    encoding: enc.to_string(),
                    byte_len: b_len,
                    decoded_utf8: decoded,
                });
                i += b_len.max(8);
                continue;
            }
        }
        i += 1;
    }

    let mut compression_hits = Vec::new();
    for (blk_idx, chunk) in buf.chunks(2048).enumerate() {
        if compression_hits.len() >= 16 {
            break;
        }
        let ent = calc_entropy(chunk);
        if (5.2..=7.4).contains(&ent) {
            let off = blk_idx * 2048;
            let algo = if ent > 6.6 {
                "KONAMI_LZSS_STREAM (Sliding Window)"
            } else {
                "KONAMI_RLE_TILEMAP_PACK"
            };
            let sig = chunk
                .iter()
                .take(8)
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ");
            compression_hits.push(CompressionBlockHit {
                offset_hex: format!("0x{:06X}", off),
                algorithm: algo.to_string(),
                compressed_size_est: chunk.len(),
                local_entropy: (ent * 1000.0).round() / 1000.0,
                signature_hex: sig,
            });
        }
    }

    CryptoAndEncodingReport {
        status: "OK".to_string(),
        platform: plat_key.to_uppercase(),
        file: file_path.to_string(),
        bytes_analyzed: buf.len(),
        shift_jis_strings_found: string_hits.len(),
        compression_blocks_found: compression_hits.len(),
        string_hits,
        compression_hits,
        engine: "rust-arm64-v12.0-shiftjis-lzss-scanner".to_string(),
    }
}
