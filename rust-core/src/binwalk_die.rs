//! MegaDaemon v13.0 Zenith — Detect It Easy (DiE) + Binwalk + Unblob + Scalpel + Bulk Extractor + Comtype Scanner
//! Multi-OS binary identification (Windows PE/PE32+, Linux/Android ELF/DEX/APK, macOS Mach-O/Fat,
//! DOS MZ/COM, Retro ROMs, Archives) using zero-copy `memmap2` file mapping.

use memmap2::Mmap;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionInfo {
    pub name: String,
    pub virtual_addr: String,
    pub raw_offset: String,
    pub size: u64,
    pub entropy: f64,
    pub flags: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedSignature {
    pub offset: usize,
    pub offset_hex: String,
    pub signature: String,
    pub category: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComtypeHit {
    pub offset_hex: String,
    pub algorithm: String,
    pub confidence: f64,
    pub magic_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkArtifact {
    pub kind: String,
    pub offset_hex: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DieBinwalkReport {
    pub filename: String,
    pub path: String,
    pub file_size: usize,
    pub format_family: String,
    pub target_os: String,
    pub architecture: String,
    pub endianness: String,
    pub bitness: u8,
    pub packer_or_compiler: String,
    pub security_mitigations: Vec<String>,
    pub global_entropy: f64,
    pub sections: Vec<SectionInfo>,
    pub embedded_signatures: Vec<EmbeddedSignature>,
    pub comtype_candidates: Vec<ComtypeHit>,
    pub bulk_artifacts: Vec<BulkArtifact>,
    pub engine: String,
}

fn shannon_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut counts = [0u64; 256];
    for &b in data {
        counts[b as usize] += 1;
    }
    let len = data.len() as f64;
    let mut h = 0.0f64;
    for &c in &counts {
        if c > 0 {
            let p = c as f64 / len;
            h -= p * p.log2();
        }
    }
    (h * 1000.0).round() / 1000.0
}

fn read_u16_le(d: &[u8], o: usize) -> u16 {
    if o + 2 <= d.len() {
        u16::from_le_bytes([d[o], d[o + 1]])
    } else {
        0
    }
}

fn read_u16_be(d: &[u8], o: usize) -> u16 {
    if o + 2 <= d.len() {
        u16::from_be_bytes([d[o], d[o + 1]])
    } else {
        0
    }
}

fn read_u32_le(d: &[u8], o: usize) -> u32 {
    if o + 4 <= d.len() {
        u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
    } else {
        0
    }
}

fn read_u32_be(d: &[u8], o: usize) -> u32 {
    if o + 4 <= d.len() {
        u32::from_be_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
    } else {
        0
    }
}

fn read_u64_le(d: &[u8], o: usize) -> u64 {
    if o + 8 <= d.len() {
        u64::from_le_bytes([
            d[o], d[o + 1], d[o + 2], d[o + 3], d[o + 4], d[o + 5], d[o + 6], d[o + 7],
        ])
    } else {
        0
    }
}

fn parse_pe_or_dos(data: &[u8], rep: &mut DieBinwalkReport) {
    if data.len() < 64 {
        rep.format_family = "DOS Raw / MZ Stub".into();
        rep.target_os = "MS-DOS".into();
        rep.architecture = "x86-16 (8086)".into();
        rep.bitness = 16;
        return;
    }
    let e_lfanew = read_u32_le(data, 0x3C) as usize;
    if e_lfanew + 24 <= data.len() && &data[e_lfanew..e_lfanew + 4] == b"PE\x00\x00" {
        let machine = read_u16_le(data, e_lfanew + 4);
        let num_sec = read_u16_le(data, e_lfanew + 6) as usize;
        let opt_size = read_u16_le(data, e_lfanew + 20) as usize;
        let opt_off = e_lfanew + 24;
        let opt_magic = read_u16_le(data, opt_off);

        rep.target_os = "Windows".into();
        rep.endianness = "Little-Endian".into();
        rep.bitness = if opt_magic == 0x20B { 64 } else { 32 };
        rep.format_family = if opt_magic == 0x20B {
            "Windows PE32+ (64-bit)".into()
        } else {
            "Windows PE32 (32-bit)".into()
        };
        rep.architecture = match machine {
            0x014C => "x86 (i386)".into(),
            0x8664 => "x86-64 (AMD64)".into(),
            0xAA64 => "ARM64 (AArch64)".into(),
            0x01C0 | 0x01C4 => "ARM (Thumb-2)".into(),
            other => format!("PE-Machine(0x{:04X})", other),
        };

        // Parse DllCharacteristics for ASLR/DEP/CFG
        let dll_char_off = opt_off + 70;
        if dll_char_off + 2 <= data.len() {
            let dc = read_u16_le(data, dll_char_off);
            if dc & 0x0040 != 0 {
                rep.security_mitigations.push("ASLR/DYNAMIC_BASE".into());
            }
            if dc & 0x0100 != 0 {
                rep.security_mitigations.push("NX/DEP".into());
            }
            if dc & 0x4000 != 0 {
                rep.security_mitigations.push("CFG (Control Flow Guard)".into());
            }
        }

        let sec_table = opt_off + opt_size;
        let mut packer_hints: Vec<&str> = Vec::new();
        for i in 0..num_sec.min(32) {
            let so = sec_table + i * 40;
            if so + 40 > data.len() {
                break;
            }
            let raw_name = &data[so..so + 8];
            let name = String::from_utf8_lossy(raw_name)
                .trim_end_matches('\0')
                .to_string();
            let vaddr = read_u32_le(data, so + 12);
            let raw_sz = read_u32_le(data, so + 16) as usize;
            let raw_ptr = read_u32_le(data, so + 20) as usize;
            let chars = read_u32_le(data, so + 36);
            let end = raw_ptr.saturating_add(raw_sz).min(data.len());
            let ent = if raw_ptr < end {
                shannon_entropy(&data[raw_ptr..end])
            } else {
                0.0
            };

            let low = name.to_lowercase();
            if low.contains("upx") {
                packer_hints.push("UPX Packer");
            } else if low.contains("vmp") {
                packer_hints.push("VMProtect");
            } else if low.contains("themida") || low.contains("winlice") {
                packer_hints.push("Themida/WinLicense");
            } else if low.contains("aspack") {
                packer_hints.push("ASPack");
            } else if ent > 7.4 && (chars & 0x2000_0000) != 0 {
                packer_hints.push("High-Entropy Executable Section (Packed/Encrypted)");
            }

            let mut fl = String::new();
            if chars & 0x4000_0000 != 0 {
                fl.push('R');
            }
            if chars & 0x8000_0000 != 0 {
                fl.push('W');
            }
            if chars & 0x2000_0000 != 0 {
                fl.push('X');
            }

            rep.sections.push(SectionInfo {
                name,
                virtual_addr: format!("0x{:08X}", vaddr),
                raw_offset: format!("0x{:08X}", raw_ptr),
                size: raw_sz as u64,
                entropy: ent,
                flags: fl,
            });
        }
        if !packer_hints.is_empty() {
            packer_hints.sort_unstable();
            packer_hints.dedup();
            rep.packer_or_compiler = packer_hints.join(" + ");
        } else {
            rep.packer_or_compiler = detect_compiler_strings(data);
        }
    } else if e_lfanew + 2 <= data.len() && &data[e_lfanew..e_lfanew + 2] == b"NE" {
        rep.format_family = "16-bit NE (Windows 3.x / OS/2)".into();
        rep.target_os = "DOS / Win16".into();
        rep.architecture = "x86-16 (286)".into();
        rep.bitness = 16;
    } else if e_lfanew + 2 <= data.len() && (&data[e_lfanew..e_lfanew + 2] == b"LE" || &data[e_lfanew..e_lfanew + 2] == b"LX") {
        rep.format_family = "32-bit DOS/4GW Linear Executable (LE/LX)".into();
        rep.target_os = "MS-DOS Extender".into();
        rep.architecture = "x86-32 (386+)".into();
        rep.bitness = 32;
    } else {
        rep.format_family = "MS-DOS MZ Executable".into();
        rep.target_os = "MS-DOS".into();
        rep.architecture = "x86-16 (8086/286)".into();
        rep.bitness = 16;
        rep.endianness = "Little-Endian".into();
    }
}

fn parse_elf(data: &[u8], rep: &mut DieBinwalkReport) {
    if data.len() < 52 {
        return;
    }
    let class = data[4]; // 1=32, 2=64
    let endian = data[5]; // 1=LE, 2=BE
    let is_le = endian != 2;
    let etype = if is_le { read_u16_le(data, 16) } else { read_u16_be(data, 16) };
    let emach = if is_le { read_u16_le(data, 18) } else { read_u16_be(data, 18) };

    rep.bitness = if class == 2 { 64 } else { 32 };
    rep.endianness = if is_le { "Little-Endian".into() } else { "Big-Endian".into() };
    let kind_str = match etype {
        1 => "Relocatable (.o)",
        2 => "Executable (ET_EXEC)",
        3 => "Shared Object / PIE (ET_DYN)",
        4 => "Core Dump",
        _ => "ELF",
    };
    rep.format_family = format!("ELF{} {}", rep.bitness, kind_str);
    rep.architecture = match emach {
        0x03 => "x86 (i386)".into(),
        0x08 => "MIPS".into(),
        0x28 => "ARM32".into(),
        0x2A => "SuperH (SH)".into(),
        0x3E => "x86-64 (AMD64)".into(),
        0xB7 => "AArch64 (ARM64)".into(),
        0xF3 => "RISC-V".into(),
        0x04 => "Motorola 68000".into(),
        other => format!("ELF-Machine(0x{:04X})", other),
    };

    // Detect Android vs Linux via dynamic strings
    let probe = &data[..data.len().min(65536)];
    let is_android = contains_bytes(probe, b"/system/bin/linker")
        || contains_bytes(probe, b"libandroid")
        || contains_bytes(probe, b"__android_log")
        || contains_bytes(probe, b"JNI_OnLoad");
    rep.target_os = if is_android {
        "Android (Linux Bionic)".into()
    } else {
        "Linux / UNIX".into()
    };

    if etype == 3 {
        rep.security_mitigations.push("PIE/PIC".into());
    }
    if contains_bytes(probe, b"__stack_chk_fail") {
        rep.security_mitigations.push("Stack Canary".into());
    }
    if contains_bytes(probe, b"GNU_RELRO") || contains_bytes(probe, b".got.plt") {
        rep.security_mitigations.push("RELRO".into());
    }

    // Parse 64-bit LE section headers
    if class == 2 && is_le && data.len() >= 64 {
        let e_shoff = read_u64_le(data, 40) as usize;
        let e_shentsize = read_u16_le(data, 58) as usize;
        let e_shnum = read_u16_le(data, 60) as usize;
        let e_shstrndx = read_u16_le(data, 62) as usize;

        let strtab_off = if e_shstrndx < e_shnum && e_shoff + (e_shstrndx + 1) * e_shentsize <= data.len() {
            read_u64_le(data, e_shoff + e_shstrndx * e_shentsize + 24) as usize
        } else {
            0
        };

        for i in 0..e_shnum.min(32) {
            let off = e_shoff + i * e_shentsize;
            if off + 64 > data.len() {
                break;
            }
            let sh_name = read_u32_le(data, off) as usize;
            let sh_flags = read_u64_le(data, off + 8);
            let sh_addr = read_u64_le(data, off + 16);
            let sh_offset = read_u64_le(data, off + 24) as usize;
            let sh_size = read_u64_le(data, off + 32) as usize;

            let name = if strtab_off > 0 && strtab_off + sh_name < data.len() {
                let s = &data[strtab_off + sh_name..];
                let nul = s.iter().position(|&b| b == 0).unwrap_or(s.len().min(32));
                String::from_utf8_lossy(&s[..nul]).to_string()
            } else {
                format!("sec_{}", i)
            };
            if name.is_empty() && sh_size == 0 {
                continue;
            }
            let end = sh_offset.saturating_add(sh_size).min(data.len());
            let ent = if sh_offset < end && sh_size > 0 {
                shannon_entropy(&data[sh_offset..end.min(sh_offset + 65536)])
            } else {
                0.0
            };
            let mut fl = String::new();
            if sh_flags & 0x2 != 0 {
                fl.push('A');
            }
            if sh_flags & 0x1 != 0 {
                fl.push('W');
            }
            if sh_flags & 0x4 != 0 {
                fl.push('X');
            }
            rep.sections.push(SectionInfo {
                name,
                virtual_addr: format!("0x{:08X}", sh_addr),
                raw_offset: format!("0x{:08X}", sh_offset),
                size: sh_size as u64,
                entropy: ent,
                flags: fl,
            });
        }
    }
    rep.packer_or_compiler = detect_compiler_strings(data);
}

fn parse_macho(data: &[u8], rep: &mut DieBinwalkReport) {
    let magic = read_u32_be(data, 0);
    rep.target_os = "macOS / iOS (Darwin)".into();
    match magic {
        0xCAFEBABE => {
            let nfat = read_u32_be(data, 4);
            rep.format_family = format!("Mach-O Universal Fat Binary ({} slices)", nfat);
            rep.architecture = "Universal (ARM64 + x86-64)".into();
            rep.bitness = 64;
            rep.endianness = "Big-Endian Header".into();
        }
        0xCFFAEDFE | 0xFEEDFACF => {
            let is_le = magic == 0xCFFAEDFE;
            let cputype = if is_le { read_u32_le(data, 4) } else { read_u32_be(data, 4) };
            rep.format_family = "Mach-O 64-bit".into();
            rep.bitness = 64;
            rep.endianness = if is_le { "Little-Endian".into() } else { "Big-Endian".into() };
            rep.architecture = match cputype {
                0x0100000C => "ARM64 (Apple Silicon / AArch64)".into(),
                0x01000007 => "x86-64 (Intel Mac)".into(),
                other => format!("MachO-CPU(0x{:08X})", other),
            };
        }
        _ => {
            rep.format_family = "Mach-O 32-bit".into();
            rep.bitness = 32;
            rep.architecture = "ARM32 / x86".into();
        }
    }
    rep.packer_or_compiler = detect_compiler_strings(data);
}

fn detect_compiler_strings(data: &[u8]) -> String {
    let scan_len = data.len().min(524288);
    let s = &data[..scan_len];
    let mut tags = Vec::new();
    if contains_bytes(s, b"UPX!") || contains_bytes(s, b"UPX0") {
        tags.push("UPX Packed");
    }
    if contains_bytes(s, b"rustc") || contains_bytes(s, b"rust_panic") || contains_bytes(s, b"core::panicking") {
        tags.push("Rust (rustc/LLVM)");
    }
    if contains_bytes(s, b"Go build ID") || contains_bytes(s, b"runtime.goexit") {
        tags.push("Go (gc)");
    }
    if contains_bytes(s, b"Android clang") || contains_bytes(s, b"clang version") {
        tags.push("Clang/LLVM");
    }
    if contains_bytes(s, b"GCC: (") {
        tags.push("GCC");
    }
    if contains_bytes(s, b"Rich") && contains_bytes(s, b"MSVCR") {
        tags.push("MSVC");
    }
    if contains_bytes(s, b"PyInstaller") || contains_bytes(s, b"MEI\x0c\x0b\x0a") {
        tags.push("PyInstaller Bundle");
    }
    if contains_bytes(s, b"ollvm") || contains_bytes(s, b"Hikari") {
        tags.push("OLLVM/Hikari Obfuscator");
    }
    if tags.is_empty() {
        "Generic Native / Stripped".to_string()
    } else {
        tags.join(" + ")
    }
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn scan_embedded_binwalk_and_comtype(data: &[u8]) -> (Vec<EmbeddedSignature>, Vec<ComtypeHit>) {
    let sigs_def: &[(&[u8], &str, &str, &str)] = &[
        (b"\x7fELF", "ELF Executable/Library", "Binary", "Linux/Android ELF header"),
        (b"PK\x03\x04", "ZIP / APK / JAR Archive", "Archive", "PKZIP local file header"),
        (b"7z\xBC\xAF\x27\x1C", "7-Zip Archive", "Archive", "7z LZMA2 container"),
        (b"\x1F\x8B\x08", "GZIP Stream", "Compression", "DEFLATE compressed stream"),
        (b"\xFD7zXZ\x00", "XZ / LZMA2 Stream", "Compression", "XZ compressed container"),
        (b"\x28\xB5\x2F\xFD", "Zstandard (ZSTD) Frame", "Compression", "Zstd compressed data"),
        (b"BZh9", "BZIP2 Stream", "Compression", "Bzip2 block-sorted stream"),
        (b"\x04\x22\x4D\x18", "LZ4 Frame", "Compression", "LZ4 fast frame"),
        (b"hsqs", "SquashFS (LE)", "Filesystem", "Embedded Linux SquashFS"),
        (b"sqsh", "SquashFS (BE)", "Filesystem", "Embedded SquashFS Big-Endian"),
        (b"dex\n03", "Android DEX Bytecode", "Android", "Dalvik Executable"),
        (b"\x89PNG\r\n\x1a\n", "PNG Image", "Graphics", "Portable Network Graphics"),
        (b"\xFF\xD8\xFF\xE0", "JPEG Image (JFIF)", "Graphics", "JPEG media stream"),
        (b"PS-X EXE", "PlayStation 1 Executable", "Retro", "MIPS R3000A PS-X EXE"),
        (b"SEGA SEGASATURN", "Sega Saturn Header", "Retro", "SH-2 IP.BIN header"),
        (b"SEGADISCSYSTEM", "Sega Mega-CD Header", "Retro", "M68K Boot Sector"),
        (b"SEGA MEGA DRIVE", "Sega Mega Drive ROM", "Retro", "M68K Cartridge Header"),
        (b"MComprHD", "MAME CHD Container", "Retro", "Compressed Hunks of Data"),
        (b"OggS", "Ogg Audio Stream", "Audio", "Vorbis/Opus audio"),
        (b"RIFF", "RIFF / WAV / AVI Container", "Media", "RIFF multimedia header"),
        (b"SQLite format 3\0", "SQLite 3 Database", "Database", "SQLite storage engine"),
    ];

    let scan_limit = data.len().min(2 * 1024 * 1024); // Fast 2MB scan cap on mobile
    let slice = &data[..scan_limit];
    let mut found_sigs = Vec::new();
    let mut comtype = Vec::new();

    for &(magic, name, cat, det) in sigs_def {
        let mut pos = 0usize;
        let mut hits_for_sig = 0usize;
        while pos + magic.len() <= slice.len() && hits_for_sig < 8 && found_sigs.len() < 64 {
            if let Some(rel) = slice[pos..].windows(magic.len()).position(|w| w == magic) {
                let off = pos + rel;
                found_sigs.push(EmbeddedSignature {
                    offset: off,
                    offset_hex: format!("0x{:08X}", off),
                    signature: name.to_string(),
                    category: cat.to_string(),
                    detail: det.to_string(),
                });
                if cat == "Compression" || cat == "Archive" {
                    comtype.push(ComtypeHit {
                        offset_hex: format!("0x{:08X}", off),
                        algorithm: name.to_string(),
                        confidence: 0.96,
                        magic_hex: magic.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" "),
                    });
                }
                pos = off + magic.len().max(16);
                hits_for_sig += 1;
            } else {
                break;
            }
        }
    }

    // Comtype Scanner: Zlib RFC-1950 header check ((CMF*256 + FLG) % 31 == 0 && (CMF & 0x0F) == 8)
    let mut zlib_hits = 0usize;
    let mut i = 0usize;
    while i + 2 < slice.len() && zlib_hits < 6 {
        let cmf = slice[i];
        let flg = slice[i + 1];
        if cmf == 0x78 && matches!(flg, 0x01 | 0x5E | 0x9C | 0xDA) && (((cmf as u16) * 256 + flg as u16) % 31 == 0) {
            comtype.push(ComtypeHit {
                offset_hex: format!("0x{:08X}", i),
                algorithm: "ZLIB / DEFLATE (RFC-1950)".to_string(),
                confidence: 0.89,
                magic_hex: format!("{:02X} {:02X}", cmf, flg),
            });
            zlib_hits += 1;
            i += 256;
        } else {
            i += 1;
        }
    }

    found_sigs.sort_by_key(|s| s.offset);
    (found_sigs, comtype)
}

fn scan_bulk_extractor(data: &[u8]) -> Vec<BulkArtifact> {
    let limit = data.len().min(512 * 1024);
    let mut artifacts = Vec::new();
    let mut seen = HashSet::new();

    let mut start = 0usize;
    let mut in_str = false;
    for i in 0..limit {
        let b = data[i];
        let printable = (32..=126).contains(&b);
        if printable && !in_str {
            start = i;
            in_str = true;
        } else if (!printable || i + 1 == limit) && in_str {
            let end = if printable { i + 1 } else { i };
            in_str = false;
            if end - start >= 7 && end - start <= 160 {
                let s = String::from_utf8_lossy(&data[start..end]).to_string();
                let kind = if s.starts_with("http://") || s.starts_with("https://") {
                    Some("URL")
                } else if s.starts_with("/system/") || s.starts_with("/vendor/") || s.starts_with("/data/") || s.starts_with("/dev/") {
                    Some("UNIX_PATH")
                } else if s.contains("HKEY_") || s.ends_with(".dll") || s.ends_with(".DLL") {
                    Some("WIN_ARTIFACT")
                } else if s.starts_with("Java_") || s.contains("Landroid/") || s.contains("com.android") {
                    Some("ANDROID_JNI")
                } else if s.contains('@') && s.contains('.') && !s.contains(' ') {
                    Some("EMAIL_OR_SYM")
                } else {
                    None
                };
                if let Some(k) = kind {
                    if seen.insert(s.clone()) && artifacts.len() < 48 {
                        artifacts.push(BulkArtifact {
                            kind: k.to_string(),
                            offset_hex: format!("0x{:08X}", start),
                            value: s,
                        });
                    }
                }
            }
        }
    }
    artifacts
}

pub fn analyze_binary_full(file_path: &str) -> DieBinwalkReport {
    let fname = Path::new(file_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(file_path)
        .to_string();

    let mut rep = DieBinwalkReport {
        filename: fname.clone(),
        path: file_path.to_string(),
        file_size: 0,
        format_family: "Raw Binary / Unknown".into(),
        target_os: "Agnostic / Raw".into(),
        architecture: "Unknown".into(),
        endianness: "Unknown".into(),
        bitness: 0,
        packer_or_compiler: "Unclassified".into(),
        security_mitigations: Vec::new(),
        global_entropy: 0.0,
        sections: Vec::new(),
        embedded_signatures: Vec::new(),
        comtype_candidates: Vec::new(),
        bulk_artifacts: Vec::new(),
        engine: "megadaemon-die-binwalk-v13-rust-mmap".into(),
    };

    let file = match File::open(file_path) {
        Ok(f) => f,
        Err(e) => {
            rep.format_family = format!("IO Error: {}", e);
            return rep;
        }
    };
    let meta_len = file.metadata().map(|m| m.len() as usize).unwrap_or(0);
    if meta_len == 0 {
        rep.format_family = "Empty File (0 bytes)".into();
        return rep;
    }

    // Zero-copy mmap via memmap2 (with fallback to Vec read if special proc/fs file)
    let owned_fallback: Vec<u8>;
    let mmap_holder: Option<Mmap> = unsafe { Mmap::map(&file).ok() };
    let data: &[u8] = if let Some(ref m) = mmap_holder {
        &m[..]
    } else {
        owned_fallback = fs::read(file_path).unwrap_or_default();
        &owned_fallback[..]
    };

    rep.file_size = data.len();
    rep.global_entropy = shannon_entropy(&data[..data.len().min(1024 * 1024)]);

    // Identify primary container / executable format
    if data.starts_with(b"\x7fELF") {
        parse_elf(data, &mut rep);
    } else if data.starts_with(b"MZ") {
        parse_pe_or_dos(data, &mut rep);
    } else if data.len() >= 4
        && matches!(
            read_u32_be(data, 0),
            0xCAFEBABE | 0xFEEDFACE | 0xCEFAEDFE | 0xFEEDFACF | 0xCFFAEDFE
        )
    {
        parse_macho(data, &mut rep);
    } else if data.starts_with(b"dex\n") {
        rep.format_family = "Android Dalvik Executable (DEX)".into();
        rep.target_os = "Android".into();
        rep.architecture = "Dalvik/ART VM Bytecode".into();
        rep.endianness = "Little-Endian".into();
        rep.bitness = 32;
        rep.packer_or_compiler = "d8 / dx / R8".into();
    } else if data.starts_with(b"PK\x03\x04") {
        let is_apk = contains_bytes(&data[..data.len().min(65536)], b"AndroidManifest.xml")
            || contains_bytes(&data[..data.len().min(65536)], b"classes.dex");
        rep.format_family = if is_apk {
            "Android Package (APK)".into()
        } else {
            "ZIP Archive Container".into()
        };
        rep.target_os = if is_apk { "Android".into() } else { "Cross-Platform".into() };
        rep.architecture = "Archive / Bytecode".into();
    } else if data.starts_with(b"7z\xBC\xAF\x27\x1C") {
        rep.format_family = "7-Zip Archive (.7z)".into();
        rep.target_os = "Cross-Platform".into();
        rep.architecture = "LZMA / LZMA2 Container".into();
    } else if data.starts_with(b"PS-X EXE") {
        rep.format_family = "PlayStation 1 Executable (PS-X EXE)".into();
        rep.target_os = "Sony PlayStation (PS1)".into();
        rep.architecture = "MIPS R3000A".into();
        rep.endianness = "Little-Endian".into();
        rep.bitness = 32;
    } else if contains_bytes(&data[..data.len().min(512)], b"SEGADISCSYSTEM")
        || contains_bytes(&data[..data.len().min(512)], b"SEGA MEGA DRIVE")
    {
        rep.format_family = "Sega Mega Drive / Mega-CD Binary".into();
        rep.target_os = "Sega 68K".into();
        rep.architecture = "Motorola 68000".into();
        rep.endianness = "Big-Endian".into();
        rep.bitness = 32;
    } else if contains_bytes(&data[..data.len().min(512)], b"SEGA SEGASATURN") {
        rep.format_family = "Sega Saturn Binary".into();
        rep.target_os = "Sega Saturn".into();
        rep.architecture = "Hitachi SuperH SH-2".into();
        rep.endianness = "Big-Endian".into();
        rep.bitness = 32;
    } else if fname.to_lowercase().ends_with(".com") {
        rep.format_family = "MS-DOS / MSX .COM Flat Executable".into();
        rep.target_os = "MS-DOS / MSX-DOS".into();
        rep.architecture = "x86-16 / Z80".into();
        rep.bitness = 16;
    } else if fname.to_lowercase().ends_with(".py")
        || fname.to_lowercase().ends_with(".md")
        || fname.to_lowercase().ends_with(".rs")
        || fname.to_lowercase().ends_with(".c")
        || fname.to_lowercase().ends_with(".json")
    {
        rep.format_family = format!("Source / Text Document ({})", fname);
        rep.target_os = "Cross-Platform".into();
        rep.architecture = "UTF-8 Text / Code".into();
    }

    let (sigs, coms) = scan_embedded_binwalk_and_comtype(data);
    rep.embedded_signatures = sigs;
    rep.comtype_candidates = coms;
    rep.bulk_artifacts = scan_bulk_extractor(data);

    rep
}

/// Unblob/Binwalk-style archive extraction using native ARM64 `7zz` / `unzip` / `zstd` / `xz`
pub fn extract_archive_or_carve(file_path: &str, out_dir: &str) -> Result<serde_json::Value, String> {
    fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let low = file_path.to_lowercase();
    let (tool, out) = if low.ends_with(".7z")
        || low.ends_with(".zip")
        || low.ends_with(".apk")
        || low.ends_with(".jar")
        || low.ends_with(".iso")
        || low.ends_with(".tar")
        || low.ends_with(".gz")
        || low.ends_with(".xz")
    {
        let r = Command::new("7zz")
            .args(["x", "-y", &format!("-o{}", out_dir), file_path])
            .output()
            .map_err(|e| format!("7zz failed: {}", e))?;
        ("7zz", r)
    } else {
        // Fallback: carve embedded signatures into separate files
        let data = fs::read(file_path).map_err(|e| e.to_string())?;
        let (sigs, _) = scan_embedded_binwalk_and_comtype(&data);
        let mut carved = Vec::new();
        for (idx, s) in sigs.iter().take(12).enumerate() {
            let end = sigs.get(idx + 1).map(|n| n.offset).unwrap_or(data.len()).min(s.offset + 262144);
            if end > s.offset {
                let p = format!("{}/carved_{:02}_{}.bin", out_dir, idx, s.offset_hex);
                let _ = fs::write(&p, &data[s.offset..end]);
                carved.push(p);
            }
        }
        return Ok(serde_json::json!({
            "status": "CARVED",
            "tool": "megadaemon-scalpel-rust",
            "out_dir": out_dir,
            "carved_count": carved.len(),
            "files": carved
        }));
    };

    let mut extracted_files = Vec::new();
    if let Ok(entries) = fs::read_dir(out_dir) {
        for e in entries.flatten().take(60) {
            extracted_files.push(e.path().to_string_lossy().to_string());
        }
    }
    Ok(serde_json::json!({
        "status": if out.status.success() { "EXTRACTED" } else { "PARTIAL" },
        "tool": tool,
        "out_dir": out_dir,
        "extracted_count": extracted_files.len(),
        "files": extracted_files,
        "log": String::from_utf8_lossy(&out.stdout).chars().take(600).collect::<String>()
    }))
}
