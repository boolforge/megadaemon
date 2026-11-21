//! MegaDaemon v13.4 — Pure Rust Recursive Retro-Media & Archive Unpacker + Multi-Hasher.
//!
//! Bounded, read-only recursive container unpacker for DAAD preservation media:
//! - ZIP archives (`zip` crate)
//! - GZIP / ADZ streams (`flate2` crate)
//! - ZX Spectrum `+3DOS` 128-byte wrapper peeling
//! - Commodore `P00` 26-byte wrapper peeling
//! - ZX Spectrum `.tap` & `.tzx`/`.cdt` standard/turbo/pure data block extraction
//! - Commodore 64 `.d64` / `.d71` (1541/1571 directory track 18/1 + T/S sector chains)
//! - Amstrad CPC `.dsk` (standard & extended DSK CP/M directory + extent extraction)
//! - Amiga `.adf` (OFS/FFS root block hash table + file header/extension/data blocks)
//! - Amiga `.dms` (stored & SIMPLE RLE validated track decoder -> recurses into ADF)
//! - MSX / Atari ST / DOS FAT12/FAT16 (BPB root/subdir + cluster chain extraction)
//!
//! Safety guarantees: bounded recursion depth, per-member 16 MiB cap, 256 max total
//! members, cycle detection on all disk/filesystem chains, and zero panics on malformed input.

use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::io::{Cursor, Read};
use std::path::Path;

/// Maximum uncompressed size of any single extracted member (16 MiB).
pub const MAX_MEMBER_SIZE: usize = 16 * 1024 * 1024;
/// Maximum total extracted members returned by a single recursive unpack call.
pub const MAX_TOTAL_MEMBERS: usize = 256;
/// Default maximum recursion depth when unpacking nested containers.
pub const DEFAULT_MAX_DEPTH: usize = 6;

// ============================================================================
// Multi-Hasher
// ============================================================================

/// Deterministic multi-hash digest suite for an artifact or extracted member.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactHashes {
    pub sha256: String,
    pub sha1: String,
    pub crc32: String,
    pub adler32: String,
    pub size: usize,
}

/// Compute SHA-256, SHA-1, CRC-32 (IEEE 802.3), Adler-32 (RFC 1950), and byte size.
pub fn compute_hashes(data: &[u8]) -> ArtifactHashes {
    let mut s256 = Sha256::new();
    s256.update(data);
    let sha256 = hex_lower(&s256.finalize());

    let mut s1 = Sha1::new();
    s1.update(data);
    let sha1 = hex_lower(&s1.finalize());

    let crc = compute_crc32(data);
    let adler = compute_adler32(data);

    ArtifactHashes {
        sha256,
        sha1,
        crc32: format!("{:08x}", crc),
        adler32: format!("{:08x}", adler),
        size: data.len(),
    }
}

/// Standard IEEE 802.3 CRC-32 (polynomial `0xEDB88320`, matching `zlib.crc32`).
pub fn compute_crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Standard RFC 1950 Adler-32 (mod 65521, matching `zlib.adler32`).
pub fn compute_adler32(data: &[u8]) -> u32 {
    const MOD_ADLER: u32 = 65521;
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + byte as u32) % MOD_ADLER;
        b = (b + a) % MOD_ADLER;
    }
    (b << 16) | a
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        use std::fmt::Write;
        let _ = write!(&mut s, "{:02x}", b);
    }
    s
}

// ============================================================================
// Extracted Member Model & Recursive Unpacker
// ============================================================================

/// One extracted file or payload member produced by [`unpack_recursive`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtractedMember {
    pub filename: String,
    pub container_path: String,
    pub container_format: String,
    pub depth: usize,
    pub hashes: ArtifactHashes,
    #[serde(skip_serializing, default)]
    pub data: Vec<u8>,
}

/// Recursively unpack archives, wrappers, tape dumps, and disk images up to `max_depth`.
///
/// Enforces bounded memory (`MAX_MEMBER_SIZE` = 16 MiB per member) and member count
/// (`MAX_TOTAL_MEMBERS` = 256) to prevent zip-bombs or recursive amplification.
pub fn unpack_recursive(data: &[u8], filename: &str, max_depth: usize) -> Vec<ExtractedMember> {
    let mut results = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    unpack_inner(
        data,
        filename,
        filename,
        0,
        max_depth,
        &mut results,
        &mut visited,
    );
    results
}

fn unpack_inner(
    data: &[u8],
    filename: &str,
    container_path: &str,
    depth: usize,
    max_depth: usize,
    out: &mut Vec<ExtractedMember>,
    visited: &mut HashSet<String>,
) {
    if depth >= max_depth || out.len() >= MAX_TOTAL_MEMBERS || data.is_empty() {
        return;
    }

    let Some((format, children)) = extract_single_layer(data, filename) else {
        return;
    };

    for (child_name, child_bytes) in children {
        if out.len() >= MAX_TOTAL_MEMBERS {
            break;
        }
        if child_bytes.len() > MAX_MEMBER_SIZE {
            continue;
        }
        let clean = sanitize_member_name(&child_name);
        let hashes = compute_hashes(&child_bytes);
        let child_path = format!("{}!{}", container_path, clean);
        let visit_key = format!("{}:{}:{}", depth + 1, clean, hashes.sha256);
        if !visited.insert(visit_key) {
            continue;
        }

        let member = ExtractedMember {
            filename: clean.clone(),
            container_path: child_path.clone(),
            container_format: format.to_string(),
            depth: depth + 1,
            hashes,
            data: child_bytes,
        };
        let next_idx = out.len();
        out.push(member);

        if depth + 1 < max_depth && out.len() < MAX_TOTAL_MEMBERS {
            let nested_data = out[next_idx].data.clone();
            unpack_inner(
                &nested_data,
                &clean,
                &child_path,
                depth + 1,
                max_depth,
                out,
                visited,
            );
        }
    }
}

/// Dispatch a single container layer to the appropriate pure-Rust extractor.
pub fn extract_single_layer(data: &[u8], filename: &str) -> Option<(&'static str, Vec<(String, Vec<u8>)>)> {
    let ext = Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    // 1. ZIP
    if data.starts_with(b"PK\x03\x04")
        || data.starts_with(b"PK\x05\x06")
        || data.starts_with(b"PK\x07\x08")
        || ext == "zip"
    {
        let members = extract_zip(data);
        if !members.is_empty() {
            return Some(("zip", members));
        }
    }

    // 2. GZIP / ADZ
    if data.starts_with(b"\x1f\x8b") || ext == "gz" || ext == "adz" {
        let members = extract_gzip(data, filename);
        if !members.is_empty() {
            return Some((if ext == "adz" { "amiga-adz" } else { "gzip" }, members));
        }
    }

    // 3. ZX Spectrum +3DOS 128-byte wrapper
    if data.len() >= 128 && data.starts_with(b"PLUS3DOS\x1a") {
        if let Some(member) = peel_plus3dos(data, filename) {
            return Some(("plus3dos", vec![member]));
        }
    }

    // 4. Commodore P00 26-byte wrapper
    if data.len() > 26 && data.starts_with(b"C64File\0") {
        if let Some(member) = peel_p00(data) {
            return Some(("commodore-p00", vec![member]));
        }
    }

    // 5. ZX Spectrum / CPC TZX / CDT
    if data.starts_with(b"ZXTape!\x1a") || ext == "tzx" || ext == "cdt" {
        let members = extract_tzx(data);
        if !members.is_empty() {
            return Some((if ext == "cdt" { "cdt" } else { "tzx" }, members));
        }
    }

    // 6. Amstrad CPC DSK (standard & extended)
    if data.len() >= 0x200
        && (data.starts_with(b"EXTENDED CPC DSK") || data.starts_with(b"MV - CPCEMU"))
    {
        let members = extract_cpc_dsk(data);
        if !members.is_empty() {
            return Some(("cpc-dsk", members));
        }
    }

    // 7. Commodore 64 D64 / D71
    if matches!(data.len(), 174_848 | 175_531) || ext == "d64" {
        let members = extract_d64(data);
        if !members.is_empty() {
            return Some(("c64-d64", members));
        }
    }
    if matches!(data.len(), 349_696 | 351_062) || ext == "d71" {
        let members = extract_d71(data);
        if !members.is_empty() {
            return Some(("c64-d71", members));
        }
    }

    // 8. Amiga DMS (decodes to ADF -> recursion unpacks the ADF)
    if data.starts_with(b"DMS!") || ext == "dms" {
        if let Some(adf) = decompress_dms_basic(data) {
            let base = strip_ext(filename, ".dms");
            return Some(("amiga-dms", vec![(format!("{}.adf", base), adf)]));
        }
    }

    // 9. Amiga ADF (OFS / FFS)
    if (data.starts_with(b"DOS") && data.len() >= 2048 && data.len() % 512 == 0) || ext == "adf" {
        let members = extract_adf(data);
        if !members.is_empty() {
            return Some(("amiga-adf", members));
        }
    }

    // 10. ZX Spectrum TAP (avoid confusing with C64-TAPE-RAW)
    if ext == "tap" && !data.starts_with(b"C64-TAPE-RAW") && !data.starts_with(b"C16-TAPE-RAW") {
        let members = extract_zx_tap(data);
        if !members.is_empty() {
            return Some(("zx-tap", members));
        }
    }

    // 11. MSX / Atari ST / DOS FAT12/FAT16 disk image
    if ext == "dsk"
        || ext == "st"
        || ext == "img"
        || is_msx_dos_fat12_boot_sector(data)
        || (data.len() >= 512 && data[510] == 0x55 && data[511] == 0xAA)
    {
        let members = extract_fat12_or_16(data);
        if !members.is_empty() {
            return Some(("msx-fat12", members));
        }
    }

    None
}

// ============================================================================
// 1. ZIP & 2. GZIP
// ============================================================================

/// Extract files from a ZIP archive with per-member size and total count bounds.
pub fn extract_zip(data: &[u8]) -> Vec<(String, Vec<u8>)> {
    let cursor = Cursor::new(data);
    let Ok(mut archive) = zip::ZipArchive::new(cursor) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let count = archive.len().min(MAX_TOTAL_MEMBERS);
    for i in 0..count {
        let Ok(file) = archive.by_index(i) else {
            continue;
        };
        if file.is_dir() {
            continue;
        }
        if file.size() as usize > MAX_MEMBER_SIZE {
            continue;
        }
        let name = file.name().to_string();
        let mut buf = Vec::new();
        let mut limited = file.take((MAX_MEMBER_SIZE + 1) as u64);
        if limited.read_to_end(&mut buf).is_ok() && buf.len() <= MAX_MEMBER_SIZE {
            out.push((name, buf));
        }
    }
    out
}

/// Decompress a GZIP stream (including `.adz`) with a strict 16 MiB cap.
pub fn extract_gzip(data: &[u8], filename: &str) -> Vec<(String, Vec<u8>)> {
    if !data.starts_with(b"\x1f\x8b") {
        return Vec::new();
    }
    let mut decoder = GzDecoder::new(Cursor::new(data));
    let mut buf = Vec::new();
    let mut limited = (&mut decoder).take((MAX_MEMBER_SIZE + 1) as u64);
    if limited.read_to_end(&mut buf).is_err() || buf.is_empty() || buf.len() > MAX_MEMBER_SIZE {
        return Vec::new();
    }
    let lower = filename.to_ascii_lowercase();
    let out_name = if lower.ends_with(".adz") {
        format!("{}.adf", strip_ext(filename, ".adz"))
    } else if lower.ends_with(".gz") {
        strip_ext(filename, ".gz")
    } else {
        format!("{}.gunzip", filename)
    };
    vec![(out_name, buf)]
}

// ============================================================================
// 3. ZX Spectrum +3DOS Wrapper & 4. Commodore P00 Wrapper
// ============================================================================

/// Peel a validated 128-byte ZX Spectrum `+3DOS` header.
pub fn peel_plus3dos(data: &[u8], filename: &str) -> Option<(String, Vec<u8>)> {
    if data.len() <= 128 || !data.starts_with(b"PLUS3DOS\x1a") {
        return None;
    }
    let checksum: u8 = data[..127]
        .iter()
        .fold(0u8, |acc, &b| acc.wrapping_add(b));
    if checksum != data[127] {
        return None;
    }
    let declared_total = u32::from_le_bytes([data[11], data[12], data[13], data[14]]) as usize;
    let declared_payload = u16::from_le_bytes([data[16], data[17]]) as usize;
    let payload_len = data.len() - 128;
    if declared_total != data.len() || (declared_payload != 0 && declared_payload > payload_len) {
        return None;
    }
    let actual_len = if declared_payload > 0 {
        declared_payload
    } else {
        payload_len
    };
    let stem = Path::new(filename)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("plus3dos");
    let out_name = format!("{}.raw", stem);
    Some((out_name, data[128..128 + actual_len].to_vec()))
}

/// Peel a validated 26-byte Commodore `P00` wrapper (`C64File\0`) into `.prg` bytes.
pub fn peel_p00(data: &[u8]) -> Option<(String, Vec<u8>)> {
    if data.len() < 28 || !data.starts_with(b"C64File\0") {
        return None;
    }
    let name = clean_ascii_name(&data[8..25], "p00_program");
    let payload = data[26..].to_vec();
    Some((format!("{}.prg", name), payload))
}

// ============================================================================
// 5. ZX Spectrum .TAP & .TZX / .CDT
// ============================================================================

fn extract_spectrum_blocks(blocks: &[&[u8]], prefix: &str) -> Vec<(String, Vec<u8>)> {
    let mut extracted = Vec::new();
    let mut pending_header: Option<(String, &'static str)> = None;

    for (idx, block) in blocks.iter().enumerate() {
        if block.len() == 19 && block[0] == 0x00 {
            let htype = block[1];
            let ext = match htype {
                0 => "bas",
                1 => "num",
                2 => "chr",
                3 => "bin",
                _ => "dat",
            };
            let name = clean_ascii_name(&block[2..12], &format!("{}_{}", prefix, idx));
            pending_header = Some((name, ext));
        } else if let Some((name, ext)) = pending_header.take() {
            if block.len() >= 2 && block[0] == 0xFF {
                extracted.push((format!("{}.{}", name, ext), block[1..block.len() - 1].to_vec()));
            } else if !block.is_empty() {
                extracted.push((format!("{}_{}.bin", prefix, idx), block.to_vec()));
            }
        } else if !block.is_empty() {
            extracted.push((format!("{}_{}.bin", prefix, idx), block.to_vec()));
        }
    }
    extracted
}

/// Extract named payload blocks from a ZX Spectrum `.tap` tape image.
pub fn extract_zx_tap(data: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut blocks = Vec::new();
    let mut pos = 0usize;
    while pos + 2 <= data.len() && blocks.len() < MAX_TOTAL_MEMBERS * 2 {
        let block_len = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2;
        if block_len == 0 || pos + block_len > data.len() {
            break;
        }
        blocks.push(&data[pos..pos + block_len]);
        pos += block_len;
    }
    extract_spectrum_blocks(&blocks, "tap")
}

/// Extract standard (`0x10`), turbo (`0x11`), and pure (`0x14`) data blocks from TZX/CDT.
pub fn extract_tzx(data: &[u8]) -> Vec<(String, Vec<u8>)> {
    if data.len() < 10 || !data.starts_with(b"ZXTape!\x1a") {
        return Vec::new();
    }
    let mut pos = 10usize;
    let mut data_blocks: Vec<&[u8]> = Vec::new();

    while pos < data.len() && data_blocks.len() < MAX_TOTAL_MEMBERS * 2 {
        let block_id = data[pos];
        pos += 1;
        match block_id {
            0x10 => {
                // Standard speed data
                if pos + 4 > data.len() {
                    return Vec::new();
                }
                let size = u16::from_le_bytes([data[pos + 2], data[pos + 3]]) as usize;
                if pos + 4 + size > data.len() {
                    return Vec::new();
                }
                data_blocks.push(&data[pos + 4..pos + 4 + size]);
                pos += 4 + size;
            }
            0x11 => {
                // Turbo speed data
                if pos + 18 > data.len() {
                    return Vec::new();
                }
                let size = (data[pos + 15] as usize)
                    | ((data[pos + 16] as usize) << 8)
                    | ((data[pos + 17] as usize) << 16);
                if pos + 18 + size > data.len() {
                    return Vec::new();
                }
                data_blocks.push(&data[pos + 18..pos + 18 + size]);
                pos += 18 + size;
            }
            0x12 => {
                if pos + 4 > data.len() {
                    return Vec::new();
                }
                pos += 4;
            }
            0x13 => {
                if pos >= data.len() {
                    return Vec::new();
                }
                let count = data[pos] as usize;
                let skip = 1 + count * 2;
                if pos + skip > data.len() {
                    return Vec::new();
                }
                pos += skip;
            }
            0x14 => {
                // Pure data
                if pos + 10 > data.len() {
                    return Vec::new();
                }
                let size = (data[pos + 7] as usize)
                    | ((data[pos + 8] as usize) << 8)
                    | ((data[pos + 9] as usize) << 16);
                if pos + 10 + size > data.len() {
                    return Vec::new();
                }
                data_blocks.push(&data[pos + 10..pos + 10 + size]);
                pos += 10 + size;
            }
            0x15 => {
                if pos + 8 > data.len() {
                    return Vec::new();
                }
                let size = (data[pos + 5] as usize)
                    | ((data[pos + 6] as usize) << 8)
                    | ((data[pos + 7] as usize) << 16);
                if pos + 8 + size > data.len() {
                    return Vec::new();
                }
                pos += 8 + size;
            }
            0x18 | 0x19 | 0x2A | 0x2B => {
                if pos + 4 > data.len() {
                    return Vec::new();
                }
                let size = u32::from_le_bytes([
                    data[pos],
                    data[pos + 1],
                    data[pos + 2],
                    data[pos + 3],
                ]) as usize;
                if pos + 4 + size > data.len() {
                    return Vec::new();
                }
                pos += 4 + size;
            }
            0x20 | 0x23 | 0x24 => {
                if pos + 2 > data.len() {
                    return Vec::new();
                }
                pos += 2;
            }
            0x21 | 0x30 => {
                if pos >= data.len() {
                    return Vec::new();
                }
                let len = data[pos] as usize;
                if pos + 1 + len > data.len() {
                    return Vec::new();
                }
                pos += 1 + len;
            }
            0x22 | 0x25 | 0x27 => {}
            0x26 => {
                if pos + 2 > data.len() {
                    return Vec::new();
                }
                let count = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
                let skip = 2 + count * 2;
                if pos + skip > data.len() {
                    return Vec::new();
                }
                pos += skip;
            }
            0x28 | 0x32 => {
                if pos + 2 > data.len() {
                    return Vec::new();
                }
                let size = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
                if pos + 2 + size > data.len() {
                    return Vec::new();
                }
                pos += 2 + size;
            }
            0x31 => {
                if pos + 2 > data.len() {
                    return Vec::new();
                }
                let len = data[pos + 1] as usize;
                if pos + 2 + len > data.len() {
                    return Vec::new();
                }
                pos += 2 + len;
            }
            0x33 => {
                if pos >= data.len() {
                    return Vec::new();
                }
                let count = data[pos] as usize;
                let skip = 1 + count * 3;
                if pos + skip > data.len() {
                    return Vec::new();
                }
                pos += skip;
            }
            0x35 => {
                if pos + 20 > data.len() {
                    return Vec::new();
                }
                let size = u32::from_le_bytes([
                    data[pos + 16],
                    data[pos + 17],
                    data[pos + 18],
                    data[pos + 19],
                ]) as usize;
                if pos + 20 + size > data.len() {
                    return Vec::new();
                }
                pos += 20 + size;
            }
            0x5A => {
                if pos + 9 > data.len() || &data[pos..pos + 7] != b"XTape!\x1a" {
                    return Vec::new();
                }
                pos += 9;
            }
            _ => return Vec::new(),
        }
    }

    extract_spectrum_blocks(&data_blocks, "tzx")
}

// ============================================================================
// 6. Commodore 64 .D64 / .D71 (1541 / 1571 CBM DOS)
// ============================================================================

/// Extract files from a single-sided 35-track Commodore 1541 `.d64` image.
pub fn extract_d64(data: &[u8]) -> Vec<(String, Vec<u8>)> {
    extract_cbm_dos(data, 1)
}

/// Extract files from a double-sided 70-track Commodore 1571 `.d71` image.
pub fn extract_d71(data: &[u8]) -> Vec<(String, Vec<u8>)> {
    extract_cbm_dos(data, 2)
}

fn cbm_track_sector_offset(track: u8, sector: u8, sides: u8) -> Option<usize> {
    if track == 0 || track > 35 * sides {
        return None;
    }
    if track > 35 {
        return Some(174_848 + cbm_track_sector_offset(track - 35, sector, 1)?);
    }
    let sectors_in_track = |t: u8| -> usize {
        if t <= 17 {
            21
        } else if t <= 24 {
            19
        } else if t <= 30 {
            18
        } else {
            17
        }
    };
    if (sector as usize) >= sectors_in_track(track) {
        return None;
    }
    let sectors_before: usize = (1..track).map(sectors_in_track).sum();
    Some((sectors_before + sector as usize) * 256)
}

fn extract_cbm_dos(data: &[u8], sides: u8) -> Vec<(String, Vec<u8>)> {
    let base_size = 174_848 * sides as usize;
    let error_size = base_size + 683 * sides as usize;
    if data.len() != base_size && data.len() != error_size {
        return Vec::new();
    }

    let mut extracted = Vec::new();
    let mut dir_track: u8 = 18;
    let mut dir_sector: u8 = 1;
    let mut dir_seen: HashSet<(u8, u8)> = HashSet::new();

    while dir_track != 0 && dir_seen.insert((dir_track, dir_sector)) {
        let Some(offset) = cbm_track_sector_offset(dir_track, dir_sector, sides) else {
            break;
        };
        if offset + 256 > data.len() {
            break;
        }
        let dir = &data[offset..offset + 256];
        dir_track = dir[0];
        dir_sector = dir[1];

        for entry_idx in 0..8 {
            let start = 2 + entry_idx * 32;
            if start + 32 > dir.len() {
                break;
            }
            let entry = &dir[start..start + 32];
            if (entry[0] & 0x07) == 0 {
                continue;
            }
            let filename = clean_ascii_name(&entry[3..19], "");
            if filename.is_empty() {
                continue;
            }
            let mut ftrack = entry[1];
            let mut fsector = entry[2];
            let mut chain_seen: HashSet<(u8, u8)> = HashSet::new();
            let mut payload = Vec::new();
            let mut valid = true;

            while ftrack != 0 && chain_seen.insert((ftrack, fsector)) {
                let Some(sec_off) = cbm_track_sector_offset(ftrack, fsector, sides) else {
                    valid = false;
                    break;
                };
                if sec_off + 256 > data.len() {
                    valid = false;
                    break;
                }
                let sec = &data[sec_off..sec_off + 256];
                ftrack = sec[0];
                fsector = sec[1];
                if ftrack == 0 {
                    if fsector < 1 {
                        valid = false;
                        break;
                    }
                    let end = (fsector as usize + 1).min(256);
                    payload.extend_from_slice(&sec[2..end]);
                } else {
                    payload.extend_from_slice(&sec[2..256]);
                }
                if payload.len() > MAX_MEMBER_SIZE {
                    valid = false;
                    break;
                }
            }

            if valid && ftrack == 0 && !payload.is_empty() {
                extracted.push((filename, payload));
                if extracted.len() >= MAX_TOTAL_MEMBERS {
                    return extracted;
                }
            }
        }
    }

    extracted
}

// ============================================================================
// 7. Amstrad CPC .DSK (Standard & Extended CP/M)
// ============================================================================

/// Extract CP/M files from standard (`MV - CPCEMU`) and extended (`EXTENDED CPC DSK`) images.
pub fn extract_cpc_dsk(data: &[u8]) -> Vec<(String, Vec<u8>)> {
    let extended = data.starts_with(b"EXTENDED CPC DSK");
    let standard = data.starts_with(b"MV - CPCEMU");
    if data.len() < 0x200 || (!extended && !standard) {
        return Vec::new();
    }

    let tracks = data[0x30] as usize;
    let sides = data[0x31] as usize;
    if tracks == 0 || sides == 0 || tracks > 84 || sides > 2 {
        return Vec::new();
    }
    let total_tracks = tracks * sides;
    let fixed_track_size = u16::from_le_bytes([data[0x32], data[0x33]]) as usize;
    if extended && 0x34 + total_tracks > data.len() {
        return Vec::new();
    }

    let mut pos = 0x100usize;
    let mut logical_sectors: Vec<u8> = Vec::new();
    let mut track_payload_sizes: Vec<usize> = Vec::new();

    for t_idx in 0..total_tracks {
        let track_size = if extended {
            (data[0x34 + t_idx] as usize) * 256
        } else {
            fixed_track_size
        };
        if track_size == 0 {
            continue;
        }
        if pos + track_size.min(0x100) > data.len() {
            break;
        }
        let thdr = &data[pos..pos + 0x100];
        if !thdr.starts_with(b"Track-Info") {
            pos += track_size;
            continue;
        }
        let sector_count = thdr[0x15] as usize;
        let mut sec_pos = pos + 0x100;
        let track_end = (pos + track_size).min(data.len());
        let mut track_secs: Vec<(u8, &[u8])> = Vec::new();

        for s_idx in 0..sector_count {
            let desc_off = 0x18 + s_idx * 8;
            if desc_off + 8 > thdr.len() {
                break;
            }
            let desc = &thdr[desc_off..desc_off + 8];
            let mut sec_size = u16::from_le_bytes([desc[6], desc[7]]) as usize;
            if sec_size == 0 {
                let code = desc[3];
                if code > 7 {
                    break;
                }
                sec_size = 128usize << code;
            }
            if sec_pos + sec_size > track_end {
                break;
            }
            track_secs.push((desc[2], &data[sec_pos..sec_pos + sec_size]));
            sec_pos += sec_size;
        }

        track_secs.sort_by_key(|&(id, _)| id);
        let mut t_bytes = 0usize;
        for &(_, sdata) in &track_secs {
            logical_sectors.extend_from_slice(sdata);
            t_bytes += sdata.len();
        }
        track_payload_sizes.push(t_bytes);
        pos += track_size;
    }

    if logical_sectors.len() < 1024 {
        return Vec::new();
    }

    // Probe Data format origin (0) and System format origin (after 2 boot tracks)
    let mut origins = vec![0usize];
    if track_payload_sizes.len() >= 2 {
        let sys_origin = track_payload_sizes[0] + track_payload_sizes[1];
        if sys_origin > 0 && sys_origin < logical_sectors.len() {
            origins.push(sys_origin);
        }
    }

    for origin in origins {
        let mut file_extents: BTreeMap<String, Vec<(usize, usize, Vec<u8>)>> = BTreeMap::new();
        let dir_end = logical_sectors.len().min(origin + 2048);
        let mut off = origin;
        while off + 32 <= dir_end {
            let entry = &logical_sectors[off..off + 32];
            off += 32;
            let user = entry[0];
            if user > 15 || user == 0xE5 {
                continue;
            }
            let base = decode_cpm_name(&entry[1..9]);
            let ext = decode_cpm_name(&entry[9..12]);
            let rec_count = entry[15] as usize;
            if base.is_empty() || rec_count == 0 {
                continue;
            }
            let fname = if ext.is_empty() {
                base
            } else {
                format!("{}.{}", base, ext)
            };
            let extent_num = (entry[12] as usize) + 32 * ((entry[14] & 0x3F) as usize);
            let blocks: Vec<u8> = entry[16..32].iter().copied().filter(|&b| b != 0).collect();
            if blocks.is_empty() {
                continue;
            }
            file_extents
                .entry(fname)
                .or_default()
                .push((extent_num, rec_count, blocks));
        }

        let mut extracted = Vec::new();
        for (fname, mut extents) in file_extents {
            extents.sort_by_key(|&(ext_num, _, _)| ext_num);
            let mut file_data = Vec::new();
            for (_, rec_count, blocks) in extents {
                let mut ext_buf = Vec::new();
                for blk in blocks {
                    let start = origin + (blk as usize) * 1024;
                    if start >= logical_sectors.len() {
                        continue;
                    }
                    let end = (start + 1024).min(logical_sectors.len());
                    ext_buf.extend_from_slice(&logical_sectors[start..end]);
                }
                let take_len = (rec_count * 128).min(ext_buf.len());
                file_data.extend_from_slice(&ext_buf[..take_len]);
                if file_data.len() > MAX_MEMBER_SIZE {
                    break;
                }
            }
            if !file_data.is_empty() && file_data.len() <= MAX_MEMBER_SIZE {
                extracted.push((fname, file_data));
            }
        }
        if !extracted.is_empty() {
            return extracted;
        }
    }

    Vec::new()
}

fn decode_cpm_name(raw: &[u8]) -> String {
    let mut s = String::new();
    for &b in raw {
        let ch = b & 0x7F;
        if ch == 0 || ch == b' ' {
            continue;
        }
        if ch.is_ascii_alphanumeric() || b"!#$%&'()@^_`{}~-".contains(&ch) {
            s.push(ch as char);
        } else {
            return String::new();
        }
    }
    s
}

// ============================================================================
// 8. Amiga .ADF (OFS / FFS) & Basic .DMS
// ============================================================================

fn adf_be32(block: &[u8], word_idx: usize) -> u32 {
    let s = word_idx * 4;
    u32::from_be_bytes([block[s], block[s + 1], block[s + 2], block[s + 3]])
}

fn adf_be32_signed(block: &[u8], word_idx: usize) -> i32 {
    adf_be32(block, word_idx) as i32
}

fn adf_block(data: &[u8], number: u32) -> Option<&[u8]> {
    if number == 0 {
        return None;
    }
    let start = (number as usize).checked_mul(512)?;
    if start + 512 > data.len() {
        return None;
    }
    Some(&data[start..start + 512])
}

fn adf_name(block: &[u8]) -> String {
    let pos = (128 - 20) * 4;
    let len = block[pos] as usize;
    if len == 0 || len > 30 || pos + 1 + len > block.len() {
        return String::new();
    }
    clean_ascii_name(&block[pos + 1..pos + 1 + len], "")
}

/// Extract files from an AmigaDOS OFS or FFS `.adf` floppy disk image.
pub fn extract_adf(data: &[u8]) -> Vec<(String, Vec<u8>)> {
    if data.len() < 2048 || data.len() % 512 != 0 || !data.starts_with(b"DOS") {
        return Vec::new();
    }
    let block_count = (data.len() / 512) as u32;
    let stored_root = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
    let root_num = if stored_root != 0 {
        stored_root
    } else {
        block_count / 2
    };
    let Some(root) = adf_block(data, root_num) else {
        return Vec::new();
    };
    if adf_be32(root, 0) != 2 || adf_be32_signed(root, 127) != 1 {
        return Vec::new();
    }

    let is_ffs = (data[3] & 0x01) != 0;
    let mut members = Vec::new();
    let mut visited_headers: HashSet<u32> = HashSet::new();

    adf_walk_dir(
        data,
        root_num,
        "",
        1,
        is_ffs,
        &mut visited_headers,
        &mut members,
    );
    members
}

fn adf_walk_dir(
    data: &[u8],
    dir_num: u32,
    prefix: &str,
    expected_secondary: i32,
    is_ffs: bool,
    visited_headers: &mut HashSet<u32>,
    members: &mut Vec<(String, Vec<u8>)>,
) {
    if members.len() >= MAX_TOTAL_MEMBERS {
        return;
    }
    let Some(dir) = adf_block(data, dir_num) else {
        return;
    };
    if adf_be32(dir, 0) != 2 || adf_be32_signed(dir, 127) != expected_secondary {
        return;
    }

    for hash_idx in 6..78 {
        let mut entry_num = adf_be32(dir, hash_idx);
        let mut chain_seen: HashSet<u32> = HashSet::new();
        while entry_num != 0 && chain_seen.insert(entry_num) && visited_headers.insert(entry_num) {
            if members.len() >= MAX_TOTAL_MEMBERS {
                return;
            }
            let Some(entry) = adf_block(data, entry_num) else {
                break;
            };
            if adf_be32(entry, 0) != 2 || adf_be32(entry, 1) != entry_num {
                break;
            }
            let name = adf_name(entry);
            let secondary = adf_be32_signed(entry, 127);
            let path = if prefix.is_empty() {
                name.clone()
            } else if name.is_empty() {
                String::new()
            } else {
                format!("{}/{}", prefix, name)
            };
            if secondary == -3 && !name.is_empty() {
                if let Some(payload) = adf_read_file(data, entry_num, entry, is_ffs) {
                    members.push((path, payload));
                }
            } else if secondary == 2 && !name.is_empty() {
                adf_walk_dir(
                    data,
                    entry_num,
                    &path,
                    2,
                    is_ffs,
                    visited_headers,
                    members,
                );
            }
            entry_num = adf_be32(entry, 124);
        }
    }
}

fn adf_read_file(data: &[u8], number: u32, block: &[u8], is_ffs: bool) -> Option<Vec<u8>> {
    let byte_size = adf_be32(block, 81) as usize;
    if byte_size > data.len() || byte_size > MAX_MEMBER_SIZE {
        return None;
    }

    let mut header_chain = vec![block];
    if is_ffs {
        let mut ext = adf_be32(block, 126);
        let mut ext_seen: HashSet<u32> = HashSet::new();
        ext_seen.insert(number);
        while ext != 0 {
            if !ext_seen.insert(ext) {
                return None;
            }
            let ext_blk = adf_block(data, ext)?;
            if adf_be32(ext_blk, 0) != 2
                || adf_be32(ext_blk, 1) != ext
                || adf_be32_signed(ext_blk, 127) != -3
            {
                return None;
            }
            header_chain.push(ext_blk);
            ext = adf_be32(ext_blk, 126);
        }

        let mut payload = Vec::with_capacity(byte_size);
        let mut data_seen: HashSet<u32> = HashSet::new();
        'outer: for hdr in header_chain {
            for idx in (6..=77).rev() {
                let ptr = adf_be32(hdr, idx);
                if ptr == 0 {
                    continue;
                }
                if !data_seen.insert(ptr) {
                    return None;
                }
                let dblk = adf_block(data, ptr)?;
                payload.extend_from_slice(dblk);
                if payload.len() >= byte_size {
                    break 'outer;
                }
            }
        }
        if payload.len() < byte_size {
            return None;
        }
        payload.truncate(byte_size);
        Some(payload)
    } else {
        // OFS linked data blocks
        let mut payload = Vec::with_capacity(byte_size);
        let mut chain_seen: HashSet<u32> = HashSet::new();
        let mut ptr = adf_be32(block, 4);
        while ptr != 0 && chain_seen.insert(ptr) && payload.len() < byte_size {
            let dblk = adf_block(data, ptr)?;
            if adf_be32(dblk, 0) != 8 {
                return None;
            }
            let chunk_size = adf_be32(dblk, 3) as usize;
            if chunk_size > 488 {
                return None;
            }
            payload.extend_from_slice(&dblk[24..24 + chunk_size]);
            ptr = adf_be32(dblk, 4);
        }
        if payload.len() < byte_size {
            return None;
        }
        payload.truncate(byte_size);
        Some(payload)
    }
}

/// Bounded Amiga DMS decoder (CRC-16/ARC validated stored & SIMPLE RLE tracks).
fn crc16_arc(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for &b in data {
        crc ^= b as u16;
        for _ in 0..8 {
            crc = if (crc & 1) != 0 {
                (crc >> 1) ^ 0xA001
            } else {
                crc >> 1
            };
        }
    }
    crc
}

fn decompress_dms_basic(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 56 || !data.starts_with(b"DMS!") {
        return None;
    }
    let hdr_crc = u16::from_be_bytes([data[54], data[55]]);
    if crc16_arc(&data[4..54]) != hdr_crc {
        return None;
    }
    let general_info = u16::from_be_bytes([data[10], data[11]]);
    if (general_info & 0x0002) != 0 {
        return None; // encrypted
    }

    let mut pos = 56usize;
    let mut out = Vec::new();
    while pos < data.len() {
        if pos + 20 > data.len() {
            return None;
        }
        let th = &data[pos..pos + 20];
        if &th[0..2] != b"TR" || crc16_arc(&th[0..18]) != u16::from_be_bytes([th[18], th[19]]) {
            return None;
        }
        let track_num = u16::from_be_bytes([th[2], th[3]]);
        let packed_len = u16::from_be_bytes([th[6], th[7]]) as usize;
        let unpacked_len = u16::from_be_bytes([th[10], th[11]]) as usize;
        let mode = th[13];
        let checksum = u16::from_be_bytes([th[14], th[15]]);
        let data_crc = u16::from_be_bytes([th[16], th[17]]);
        pos += 20;
        if packed_len > 32_000 || unpacked_len > 32_000 || pos + packed_len > data.len() {
            return None;
        }
        let packed = &data[pos..pos + packed_len];
        pos += packed_len;
        if crc16_arc(packed) != data_crc {
            return None;
        }
        if track_num == 80 || track_num == 0xFFFF || unpacked_len <= 2048 {
            continue;
        }
        let decoded = match mode {
            0 => {
                if packed_len < unpacked_len {
                    return None;
                }
                packed[..unpacked_len].to_vec()
            }
            1 => dms_rle_simple(packed, unpacked_len)?,
            _ => return None,
        };
        let sum: u16 = decoded
            .iter()
            .fold(0u16, |acc, &b| acc.wrapping_add(b as u16));
        if sum != checksum {
            return None;
        }
        out.extend_from_slice(&decoded);
        if out.len() > MAX_MEMBER_SIZE {
            return None;
        }
    }
    if out.is_empty() || out.len() % 512 != 0 {
        return None;
    }
    Some(out)
}

fn dms_rle_simple(data: &[u8], expected_len: usize) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(expected_len);
    let mut pos = 0usize;
    while out.len() < expected_len {
        let val = *data.get(pos)?;
        pos += 1;
        if val != 0x90 {
            out.push(val);
            continue;
        }
        let mut run = *data.get(pos)? as usize;
        pos += 1;
        if run == 0 {
            out.push(0x90);
            continue;
        }
        let repeated = *data.get(pos)?;
        pos += 1;
        if run == 0xFF {
            let hi = *data.get(pos)? as usize;
            let lo = *data.get(pos + 1)? as usize;
            pos += 2;
            run = (hi << 8) | lo;
        }
        if run == 0 || out.len() + run > expected_len {
            return None;
        }
        out.resize(out.len() + run, repeated);
    }
    Some(out)
}

// ============================================================================
// 9. MSX / Atari ST / DOS FAT12 & FAT16
// ============================================================================

/// Detect MSX-DOS FAT12 boot sector (`EB FE 90` + verified MSX 1DD/2DD geometry).
pub fn is_msx_dos_fat12_boot_sector(data: &[u8]) -> bool {
    if data.len() < 512 || &data[0..3] != b"\xeb\xfe\x90" {
        return false;
    }
    let geom = match data[21] {
        0xF8 => (720usize, 9u16, 1u16, 2u16),
        0xF9 => (1440, 9, 2, 3),
        0xFA => (640, 8, 1, 1),
        0xFB => (1280, 8, 2, 2),
        _ => return false,
    };
    let (total_sectors, spt, heads, spf) = geom;
    u16::from_le_bytes([data[11], data[12]]) == 512
        && data[13] == 2
        && u16::from_le_bytes([data[14], data[15]]) == 1
        && data[16] == 2
        && u16::from_le_bytes([data[17], data[18]]) == 112
        && u16::from_le_bytes([data[19], data[20]]) as usize == total_sectors
        && u16::from_le_bytes([data[22], data[23]]) == spf
        && u16::from_le_bytes([data[24], data[25]]) == spt
        && u16::from_le_bytes([data[26], data[27]]) == heads
        && data.len() >= total_sectors * 512
}

/// Extract files from an MSX / Atari ST / DOS FAT12 or FAT16 disk image.
pub fn extract_fat12_or_16(data: &[u8]) -> Vec<(String, Vec<u8>)> {
    if data.len() < 512
        || (!(data[510] == 0x55 && data[511] == 0xAA) && !is_msx_dos_fat12_boot_sector(data))
    {
        return Vec::new();
    }

    let bytes_per_sector = u16::from_le_bytes([data[11], data[12]]) as usize;
    let sectors_per_cluster = data[13] as usize;
    let reserved = u16::from_le_bytes([data[14], data[15]]) as usize;
    let fat_count = data[16] as usize;
    let root_entries = u16::from_le_bytes([data[17], data[18]]) as usize;
    let total_16 = u16::from_le_bytes([data[19], data[20]]) as usize;
    let total_32 = u32::from_le_bytes([data[32], data[33], data[34], data[35]]) as usize;
    let total_sectors = if total_16 != 0 { total_16 } else { total_32 };
    let sectors_per_fat = u16::from_le_bytes([data[22], data[23]]) as usize;

    if !matches!(bytes_per_sector, 128 | 256 | 512 | 1024)
        || sectors_per_cluster == 0
        || reserved == 0
        || fat_count == 0
        || root_entries == 0
        || sectors_per_fat == 0
        || total_sectors == 0
    {
        return Vec::new();
    }

    let image_size = match total_sectors.checked_mul(bytes_per_sector) {
        Some(sz) if sz <= data.len() => sz,
        _ => return Vec::new(),
    };

    let root_sector_count = (root_entries * 32 + bytes_per_sector - 1) / bytes_per_sector;
    let fat_start = reserved * bytes_per_sector;
    let fat_size = sectors_per_fat * bytes_per_sector;
    let root_start = (reserved + fat_count * sectors_per_fat) * bytes_per_sector;
    let data_start_sector = reserved + fat_count * sectors_per_fat + root_sector_count;

    if data_start_sector >= total_sectors
        || fat_start + fat_size > data.len()
        || root_start + root_entries * 32 > data.len()
    {
        return Vec::new();
    }

    let data_sector_count = total_sectors - data_start_sector;
    let cluster_count = data_sector_count / sectors_per_cluster;
    let is_fat12 = if cluster_count < 4085 {
        true
    } else if cluster_count < 65525 {
        false
    } else {
        return Vec::new(); // reject FAT32
    };

    let fat = &data[fat_start..fat_start + fat_size];
    let max_cluster = cluster_count + 1;
    let mut members = Vec::new();
    let mut dir_clusters: HashSet<usize> = HashSet::new();

    let ctx = FatCtx {
        data,
        fat,
        image_size,
        bytes_per_sector,
        sectors_per_cluster,
        data_start_sector,
        max_cluster,
        is_fat12,
    };

    let root_bytes = &data[root_start..root_start + root_entries * 32];
    ctx.walk_dir(root_bytes, "", &mut dir_clusters, &mut members);
    members
}

struct FatCtx<'a> {
    data: &'a [u8],
    fat: &'a [u8],
    image_size: usize,
    bytes_per_sector: usize,
    sectors_per_cluster: usize,
    data_start_sector: usize,
    max_cluster: usize,
    is_fat12: bool,
}

impl<'a> FatCtx<'a> {
    fn next_cluster(&self, cluster: usize) -> Option<usize> {
        if self.is_fat12 {
            let idx = cluster + (cluster / 2);
            if idx + 1 >= self.fat.len() {
                return None;
            }
            let pair = (self.fat[idx] as usize) | ((self.fat[idx + 1] as usize) << 8);
            Some(if (cluster & 1) != 0 {
                (pair >> 4) & 0x0FFF
            } else {
                pair & 0x0FFF
            })
        } else {
            let off = cluster * 2;
            if off + 2 > self.fat.len() {
                return None;
            }
            Some(u16::from_le_bytes([self.fat[off], self.fat[off + 1]]) as usize)
        }
    }

    fn chain_payload(&self, first_cluster: usize) -> Option<Vec<u8>> {
        if first_cluster < 2 || first_cluster > self.max_cluster {
            return None;
        }
        let mut seen: HashSet<usize> = HashSet::new();
        let mut payload = Vec::new();
        let mut cluster = first_cluster;
        let eof = if self.is_fat12 { 0xFF8 } else { 0xFFF8 };
        let bad = if self.is_fat12 { 0xFF7 } else { 0xFFF7 };
        let cluster_bytes = self.sectors_per_cluster * self.bytes_per_sector;

        loop {
            if !seen.insert(cluster) || cluster < 2 || cluster > self.max_cluster {
                return None;
            }
            let sector = self.data_start_sector + (cluster - 2) * self.sectors_per_cluster;
            let start = sector * self.bytes_per_sector;
            if start + cluster_bytes > self.image_size {
                return None;
            }
            payload.extend_from_slice(&self.data[start..start + cluster_bytes]);
            if payload.len() > MAX_MEMBER_SIZE {
                return None;
            }
            let next = self.next_cluster(cluster)?;
            if next == bad {
                return None;
            }
            if next >= eof {
                return Some(payload);
            }
            cluster = next;
        }
    }

    fn walk_dir(
        &self,
        entries: &[u8],
        prefix: &str,
        dir_clusters: &mut HashSet<usize>,
        members: &mut Vec<(String, Vec<u8>)>,
    ) {
        let mut off = 0usize;
        while off + 32 <= entries.len() && members.len() < MAX_TOTAL_MEMBERS {
            let entry = &entries[off..off + 32];
            off += 32;
            if entry[0] == 0x00 {
                return;
            }
            if entry[0] == 0xE5 {
                continue;
            }
            let attr = entry[11];
            if attr == 0x0F || (attr & 0x08) != 0 {
                // LFN entry or volume label
                continue;
            }
            let name = clean_ascii_name(&entry[0..8], "file");
            let ext = clean_ascii_name(&entry[8..11], "");
            let short_name = if ext.is_empty() {
                name
            } else {
                format!("{}.{}", name, ext)
            };
            let cluster = u16::from_le_bytes([entry[26], entry[27]]) as usize;
            let path = if prefix.is_empty() {
                short_name.clone()
            } else {
                format!("{}/{}", prefix, short_name)
            };

            if (attr & 0x10) != 0 {
                if short_name == "." || short_name == ".." || !dir_clusters.insert(cluster) {
                    continue;
                }
                if let Some(sub) = self.chain_payload(cluster) {
                    self.walk_dir(&sub, &path, dir_clusters, members);
                }
                continue;
            }

            let file_size =
                u32::from_le_bytes([entry[28], entry[29], entry[30], entry[31]]) as usize;
            if file_size > MAX_MEMBER_SIZE {
                continue;
            }
            if file_size == 0 {
                members.push((path, Vec::new()));
                continue;
            }
            if let Some(mut payload) = self.chain_payload(cluster) {
                if payload.len() >= file_size {
                    payload.truncate(file_size);
                    members.push((path, payload));
                }
            }
        }
    }
}

// ============================================================================
// Filename Sanitization Helpers
// ============================================================================

fn clean_ascii_name(raw: &[u8], fallback: &str) -> String {
    let mut s = String::with_capacity(raw.len());
    for &b in raw {
        let ch = if b == 0xA0 { b' ' } else { b };
        if ch == 0 {
            continue;
        }
        if ch.is_ascii_alphanumeric() || matches!(ch, b'.' | b'_' | b'-' | b' ') {
            s.push(ch as char);
        } else if ch >= 0x20 && ch < 0x7F {
            s.push('_');
        }
    }
    let trimmed = s.trim_matches(|c| c == ' ' || c == '.');
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn sanitize_member_name(name: &str) -> String {
    let normalized = name.replace('\\', "/");
    let leaf = normalized.rsplit('/').next().unwrap_or("unnamed");
    let mut out = String::with_capacity(leaf.len());
    for ch in leaf.chars() {
        if ch.is_ascii_control() || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        {
            out.push('_');
        } else {
            out.push(ch);
        }
    }
    let trimmed = out.trim_matches(|c| c == ' ' || c == '.');
    if trimmed.is_empty() {
        "unnamed".to_string()
    } else {
        trimmed.to_string()
    }
}

fn strip_ext(filename: &str, suffix: &str) -> String {
    let lower = filename.to_ascii_lowercase();
    if lower.ends_with(suffix) && filename.len() > suffix.len() {
        filename[..filename.len() - suffix.len()].to_string()
    } else {
        filename.to_string()
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[test]
    fn test_multi_hash_vectors() {
        // Known standard test vector: "hello"
        let h = compute_hashes(b"hello");
        assert_eq!(h.size, 5);
        assert_eq!(
            h.sha256,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert_eq!(h.sha1, "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d");
        assert_eq!(h.crc32, "3610a686");
        assert_eq!(h.adler32, "062c0215");
    }

    #[test]
    fn test_plus3dos_peeling() {
        let payload = b"DAAD_PAYLOAD_TEST_BYTES";
        let total = 128 + payload.len();
        let mut buf = vec![0u8; total];
        buf[..9].copy_from_slice(b"PLUS3DOS\x1a");
        buf[9] = 1; // issue
        buf[10] = 0; // version
        buf[11..15].copy_from_slice(&(total as u32).to_le_bytes());
        buf[15] = 3; // code file
        buf[16..18].copy_from_slice(&(payload.len() as u16).to_le_bytes());
        buf[127] = buf[..127]
            .iter()
            .fold(0u8, |acc, &b| acc.wrapping_add(b));
        buf[128..].copy_from_slice(payload);

        let members = unpack_recursive(&buf, "GAME.DDB", 4);
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].container_format, "plus3dos");
        assert_eq!(members[0].data, payload);
    }

    #[test]
    fn test_p00_peeling() {
        let mut p00 = Vec::new();
        p00.extend_from_slice(b"C64File\0");
        let mut name = [0u8; 17];
        name[..6].copy_from_slice(b"ADVENT");
        p00.extend_from_slice(&name);
        p00.push(0x00); // record size
        p00.extend_from_slice(&[0x01, 0x08, 0xAA, 0xBB, 0xCC]);

        let members = unpack_recursive(&p00, "advent.p00", 4);
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].container_format, "commodore-p00");
        assert_eq!(members[0].filename, "ADVENT.prg");
        assert_eq!(members[0].data, vec![0x01, 0x08, 0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn test_zx_tap_extraction() {
        // Build a standard ZX TAP: 19-byte header + data block
        let mut tap = Vec::new();
        let mut hdr = [0u8; 19];
        hdr[0] = 0x00; // header flag
        hdr[1] = 3; // CODE (.bin)
        hdr[2..12].copy_from_slice(b"PART1     ");
        hdr[12..14].copy_from_slice(&4u16.to_le_bytes());
        hdr[18] = hdr[..18].iter().fold(0u8, |a, &b| a ^ b);

        tap.extend_from_slice(&(hdr.len() as u16).to_le_bytes());
        tap.extend_from_slice(&hdr);

        let payload = [0xDE, 0xAD, 0xBE, 0xEF];
        let mut dblk = Vec::new();
        dblk.push(0xFF); // data flag
        dblk.extend_from_slice(&payload);
        let csum = dblk.iter().fold(0u8, |a, &b| a ^ b);
        dblk.push(csum);

        tap.extend_from_slice(&(dblk.len() as u16).to_le_bytes());
        tap.extend_from_slice(&dblk);

        let members = unpack_recursive(&tap, "game.tap", 4);
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].container_format, "zx-tap");
        assert_eq!(members[0].filename, "PART1.bin");
        assert_eq!(members[0].data, payload);
    }

    #[test]
    fn test_nested_zip_and_gzip_recursive_unpack() {
        // Create gzipped inner payload
        let inner_payload = b"Nested DAAD binary inside GZ inside ZIP!";
        let mut gz_enc = GzEncoder::new(Vec::new(), Compression::default());
        gz_enc.write_all(inner_payload).unwrap();
        let gz_bytes = gz_enc.finish().unwrap();

        // Wrap gzip inside a ZIP archive
        let mut zip_buf = Cursor::new(Vec::new());
        {
            let mut zw = zip::ZipWriter::new(&mut zip_buf);
            let opts = SimpleFileOptions::default();
            zw.start_file("game.ddb.gz", opts).unwrap();
            zw.write_all(&gz_bytes).unwrap();
            zw.finish().unwrap();
        }
        let zip_bytes = zip_buf.into_inner();

        let members = unpack_recursive(&zip_bytes, "bundle.zip", 4);
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].depth, 1);
        assert_eq!(members[0].container_format, "zip");
        assert_eq!(members[0].filename, "game.ddb.gz");

        assert_eq!(members[1].depth, 2);
        assert_eq!(members[1].container_format, "gzip");
        assert_eq!(members[1].filename, "game.ddb");
        assert_eq!(members[1].data, inner_payload);
        assert_eq!(members[1].hashes, compute_hashes(inner_payload));
    }
}
