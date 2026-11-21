//! MegaDaemon v13.4 — Pure Rust DAAD (Diseñador de Aventuras AD) Harvester & Parser.
//!
//! Evidence-led parser and extractor for DAAD databases across all official targets:
//! ZX Spectrum, Amstrad CPC, Commodore 64, Commodore Plus/4, MSX, MSX2,
//! Amstrad PCW, Atari ST, Amiga, and PC/DOS.
//!
//! Fully compliant with the DAAD Harvester specification (DRC v2/v3 and historical
//! legacy v1/v2 layouts derived from MSX2DAAD).

use serde::{Deserialize, Serialize};

/// Machine target definition: platform, memory base address, pointer endianness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetSpec {
    pub machine_id: u8,
    pub platform: &'static str,
    pub base_address: u16,
    pub is_little_endian: bool,
}

pub const MACHINE_TARGETS: &[TargetSpec] = &[
    TargetSpec { machine_id: 0x0, platform: "dos", base_address: 0x0000, is_little_endian: false },
    TargetSpec { machine_id: 0x1, platform: "zx", base_address: 0x8400, is_little_endian: false },
    TargetSpec { machine_id: 0x2, platform: "c64", base_address: 0x3880, is_little_endian: false },
    TargetSpec { machine_id: 0x3, platform: "cpc", base_address: 0x2880, is_little_endian: false },
    TargetSpec { machine_id: 0x4, platform: "msx", base_address: 0x0100, is_little_endian: false },
    TargetSpec { machine_id: 0x5, platform: "atarist", base_address: 0x0000, is_little_endian: true },
    TargetSpec { machine_id: 0x6, platform: "amiga", base_address: 0x0000, is_little_endian: true },
    TargetSpec { machine_id: 0x7, platform: "pcw", base_address: 0x0100, is_little_endian: false },
    TargetSpec { machine_id: 0xE, platform: "plus4", base_address: 0x7080, is_little_endian: false },
    TargetSpec { machine_id: 0xF, platform: "msx2", base_address: 0x0000, is_little_endian: false },
];

pub fn get_target_spec(machine_id: u8) -> Option<TargetSpec> {
    MACHINE_TARGETS.iter().copied().find(|t| t.machine_id == machine_id)
}

pub const HEADER_SIZE: usize = 60;
pub const HEADER_POINTER_COUNT: usize = 13;
pub const LEGACY_HEADER_SIZE: usize = 34;
pub const LEGACY_POINTER_COUNT: usize = 13;
pub const LEGACY_V1_HEADER_SIZE: usize = 32;
pub const LEGACY_V1_SECTION_POINTER_COUNT: usize = 11;
pub const MAX_EMBEDDED_SCAN: usize = 524_288;
pub const MAX_PROCESS_ENTRIES: usize = 256;
pub const MAX_BYTECODE_LENGTH: usize = 1024;
pub const STREAM_TERMINATOR: u8 = 0xFF;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DDBHeader {
    pub major_version: u8,
    pub machine_id: u8,
    pub platform: String,
    pub base_address: u16,
    pub endianness: String,
    pub language: String,
    pub submachine: u8,
    pub object_count: u8,
    pub location_count: u8,
    pub message_count: u8,
    pub system_message_count: u8,
    pub process_count: u8,
    pub pointers: Vec<u16>,
    pub file_length: u16,
    pub header_size: usize,
    pub layout: String,
}

impl DDBHeader {
    pub fn expected_size(&self) -> usize {
        (self.file_length as usize).saturating_sub(self.base_address as usize)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessValidation {
    pub valid: bool,
    pub reason: String,
    pub process_table_offset: usize,
    pub process_offsets: Vec<usize>,
    pub parsed_entries: usize,
    pub terminated_streams: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DDBAnalysis {
    pub is_daad: bool,
    pub confidence: f64,
    pub confidence_label: String,
    pub version: Option<String>,
    pub ddb_format: Option<String>,
    pub ddb_major_version: Option<u8>,
    pub platform: String,
    pub language: String,
    pub reason: String,
    pub payload_offset: usize,
    pub payload_size: usize,
    pub header: Option<DDBHeader>,
    pub process_validation: Option<ProcessValidation>,
    pub container_wrapper: String,
}

impl Default for DDBAnalysis {
    fn default() -> Self {
        Self {
            is_daad: false,
            confidence: 0.0,
            confidence_label: "unverified".into(),
            version: None,
            ddb_format: None,
            ddb_major_version: None,
            platform: "unknown".into(),
            language: "unknown".into(),
            reason: "no_valid_target_aware_ddb_structure".into(),
            payload_offset: 0,
            payload_size: 0,
            header: None,
            process_validation: None,
            container_wrapper: "raw".into(),
        }
    }
}

pub struct DAADBytecodeParser;

impl DAADBytecodeParser {
    pub fn is_explicit_rejection(data: &[u8], filename: &str) -> (bool, &'static str) {
        let lower_name = filename.to_lowercase();
        let ext = lower_name.rsplit('.').next().unwrap_or("");
        match ext {
            "php" | "html" | "htm" | "xml" | "json" | "css" | "js" | "py" | "cpp" | "h"
            | "c" | "txt" | "md" | "rpy" | "rpyc" | "nes" | "sfc" | "smc" | "z64" | "v64"
            | "n64" | "gba" | "gbc" | "gb" | "nds" | "3ds" | "iso" | "cue" | "mp3" | "mp4"
            | "wav" | "flac" | "ogg" | "png" | "jpg" | "jpeg" | "gif" | "bmp" | "avi"
            | "mkv" => return (true, "explicit_non_daad_extension"),
            _ => {}
        }
        let check_len = data.len().min(4096);
        let initial = &data[..check_len];
        if ext == "rpyc" || initial.starts_with(b"RPYC") {
            return (true, "renpy_engine_payload");
        }
        if initial.windows(5).any(|w| w == b"<?php")
            || initial.windows(6).any(|w| w == b"<html" || w == b"<head")
        {
            return (true, "html_php_content");
        }
        if data.starts_with(b"PK\x03\x04")
            || data.starts_with(b"7z\xbc\xaf\x27\x1c")
            || data.starts_with(b"Rar!")
            || data.starts_with(b"\x1f\x8b")
        {
            return (true, "raw_archive_header");
        }
        let initial1k = &data[..data.len().min(1024)];
        if initial1k.windows(9).any(|w| w == b"The Quill" || w == b"SWAN Syst")
            || initial1k.windows(4).any(|w| w == b"PAWS")
        {
            return (true, "other_authoring_engine");
        }
        (false, "")
    }

    fn read_word(data: &[u8], offset: usize, is_little_endian: bool) -> Option<u16> {
        if offset + 2 > data.len() {
            return None;
        }
        let b0 = data[offset];
        let b1 = data[offset + 1];
        Some(if is_little_endian {
            u16::from_le_bytes([b0, b1])
        } else {
            u16::from_be_bytes([b0, b1])
        })
    }

    fn plus3dos_wrapper(data: &[u8]) -> Option<(usize, String)> {
        if data.len() < 128 || !data.starts_with(b"PLUS3DOS\x1a") {
            return None;
        }
        let checksum: u8 = data[..127].iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
        if checksum != data[127] {
            return None;
        }
        let declared_total = u32::from_le_bytes([data[11], data[12], data[13], data[14]]) as usize;
        let declared_payload = u16::from_le_bytes([data[16], data[17]]) as usize;
        if declared_total != data.len() || declared_payload != data.len().saturating_sub(128) {
            return None;
        }
        Some((128, "plus3dos".into()))
    }

    fn candidate_offsets(data: &[u8], filename: &str) -> Vec<(usize, String)> {
        let mut candidates = vec![(0, "raw".into())];
        if let Some(plus3) = Self::plus3dos_wrapper(data) {
            candidates.insert(0, plus3);
        }
        let lower = filename.to_lowercase();
        if (lower.ends_with(".prg") || lower.ends_with(".p00")) && data.len() >= 2 {
            candidates.insert(0, (2, "commodore-prg".into()));
        }
        candidates
    }

    fn pointer_index(address: u16, base_address: u16) -> Option<usize> {
        if address == 0 || address < base_address {
            None
        } else {
            Some((address - base_address) as usize)
        }
    }

    fn parse_header(data: &[u8], offset: usize) -> Option<DDBHeader> {
        if offset + HEADER_SIZE > data.len() {
            return None;
        }
        let major = data[offset];
        let machine_language = data[offset + 1];
        let machine_id = machine_language >> 4;
        let target = get_target_spec(machine_id)?;
        if major != 2 && major != 3 {
            return None;
        }
        let mut pointers = Vec::with_capacity(HEADER_POINTER_COUNT);
        for index in 0..HEADER_POINTER_COUNT {
            let p = Self::read_word(data, offset + 8 + (index * 2), target.is_little_endian)?;
            pointers.push(p);
        }
        let file_length = pointers[12];
        Some(DDBHeader {
            major_version: major,
            machine_id,
            platform: target.platform.to_string(),
            base_address: target.base_address,
            endianness: if target.is_little_endian { "little" } else { "big" }.into(),
            language: if (machine_language & 0x01) != 0 { "es" } else { "en" }.into(),
            submachine: data[offset + 2],
            object_count: data[offset + 3],
            location_count: data[offset + 4],
            message_count: data[offset + 5],
            system_message_count: data[offset + 6],
            process_count: data[offset + 7],
            pointers,
            file_length,
            header_size: HEADER_SIZE,
            layout: "drc".into(),
        })
    }

    fn parse_legacy_header(data: &[u8], offset: usize) -> Option<DDBHeader> {
        if offset + LEGACY_V1_HEADER_SIZE > data.len() {
            return None;
        }
        let major = data[offset];
        let machine_language = data[offset + 1];
        let machine_id = machine_language >> 4;
        let target = get_target_spec(machine_id)?;
        if (major != 1 && major != 2) || data[offset + 2] != 0x5F {
            return None;
        }
        let is_little_endian = match target.platform {
            "atarist" | "amiga" => false,
            _ => true,
        };
        let (header_size, pointer_count) = if major == 1 {
            (LEGACY_V1_HEADER_SIZE, LEGACY_V1_SECTION_POINTER_COUNT)
        } else {
            (LEGACY_HEADER_SIZE, LEGACY_POINTER_COUNT - 1)
        };
        if offset + header_size > data.len() {
            return None;
        }
        let mut pointers = Vec::with_capacity(pointer_count);
        for index in 0..pointer_count {
            let p = Self::read_word(data, offset + 8 + (index * 2), is_little_endian)?;
            pointers.push(p);
        }
        let file_length = Self::read_word(data, offset + 8 + (pointer_count * 2), is_little_endian)?;
        let available_size = data.len() - offset;
        let absolute_size = file_length.saturating_sub(target.base_address) as usize;
        let base_address = if target.base_address > 0
            && absolute_size >= header_size
            && absolute_size <= available_size
        {
            target.base_address
        } else {
            0
        };

        Some(DDBHeader {
            major_version: major,
            machine_id,
            platform: target.platform.to_string(),
            base_address,
            endianness: if is_little_endian { "little" } else { "big" }.into(),
            language: if (machine_language & 0x01) != 0 { "es" } else { "en" }.into(),
            submachine: data[offset + 2],
            object_count: data[offset + 3],
            location_count: data[offset + 4],
            message_count: data[offset + 5],
            system_message_count: data[offset + 6],
            process_count: data[offset + 7],
            pointers,
            file_length,
            header_size,
            layout: "legacy".into(),
        })
    }

    fn validate_process_entries(
        data: &[u8],
        offset: usize,
        header: &DDBHeader,
        payload_size: usize,
    ) -> ProcessValidation {
        if header.pointers.len() < 2 {
            return ProcessValidation {
                valid: false,
                reason: "missing_process_table".into(),
                process_table_offset: 0,
                process_offsets: vec![],
                parsed_entries: 0,
                terminated_streams: 0,
            };
        }
        let process_table_address = header.pointers[1];
        let process_table = match Self::pointer_index(process_table_address, header.base_address) {
            Some(idx) => idx,
            None => {
                return ProcessValidation {
                    valid: false,
                    reason: "missing_process_table".into(),
                    process_table_offset: 0,
                    process_offsets: vec![],
                    parsed_entries: 0,
                    terminated_streams: 0,
                }
            }
        };

        if header.process_count == 0 || header.process_count > 64 {
            return ProcessValidation {
                valid: false,
                reason: "invalid_process_count".into(),
                process_table_offset: process_table,
                process_offsets: vec![],
                parsed_entries: 0,
                terminated_streams: 0,
            };
        }
        if process_table < header.header_size
            || process_table + (header.process_count as usize * 2) > payload_size
        {
            return ProcessValidation {
                valid: false,
                reason: "process_table_out_of_bounds".into(),
                process_table_offset: process_table,
                process_offsets: vec![],
                parsed_entries: 0,
                terminated_streams: 0,
            };
        }

        let is_le = header.endianness == "little";
        let mut process_offsets = Vec::with_capacity(header.process_count as usize);
        for index in 0..(header.process_count as usize) {
            let addr = match Self::read_word(data, offset + process_table + (index * 2), is_le) {
                Some(a) => a,
                None => {
                    return ProcessValidation {
                        valid: false,
                        reason: "process_entry_pointer_out_of_bounds".into(),
                        process_table_offset: process_table,
                        process_offsets,
                        parsed_entries: 0,
                        terminated_streams: 0,
                    }
                }
            };
            match Self::pointer_index(addr, header.base_address) {
                Some(idx) if idx >= header.header_size && idx < payload_size => {
                    process_offsets.push(idx);
                }
                _ => {
                    return ProcessValidation {
                        valid: false,
                        reason: "process_entry_pointer_out_of_bounds".into(),
                        process_table_offset: process_table,
                        process_offsets,
                        parsed_entries: 0,
                        terminated_streams: 0,
                    }
                }
            }
        }

        let mut referenced_streams = 0;
        let mut terminated_streams = 0;
        let mut parsed_entries = 0;

        let check_limit = (header.process_count as usize).min(8);
        for &process_offset in &process_offsets[..check_limit] {
            let mut entry_position = process_offset;
            let mut entries_count = 0;
            while entries_count < MAX_PROCESS_ENTRIES {
                if entry_position >= payload_size {
                    return ProcessValidation {
                        valid: false,
                        reason: "process_entry_out_of_bounds".into(),
                        process_table_offset: process_table,
                        process_offsets,
                        parsed_entries,
                        terminated_streams,
                    };
                }
                if data[offset + entry_position] == 0 {
                    break;
                }
                if entry_position + 4 > payload_size {
                    return ProcessValidation {
                        valid: false,
                        reason: "truncated_process_entry".into(),
                        process_table_offset: process_table,
                        process_offsets,
                        parsed_entries,
                        terminated_streams,
                    };
                }
                let condact_addr = match Self::read_word(data, offset + entry_position + 2, is_le) {
                    Some(a) => a,
                    None => {
                        return ProcessValidation {
                            valid: false,
                            reason: "condact_pointer_out_of_bounds".into(),
                            process_table_offset: process_table,
                            process_offsets,
                            parsed_entries,
                            terminated_streams,
                        }
                    }
                };
                let condact_offset = match Self::pointer_index(condact_addr, header.base_address) {
                    Some(idx) if idx >= header.header_size && idx < payload_size => idx,
                    _ => {
                        return ProcessValidation {
                            valid: false,
                            reason: "condact_pointer_out_of_bounds".into(),
                            process_table_offset: process_table,
                            process_offsets,
                            parsed_entries,
                            terminated_streams,
                        }
                    }
                };

                let end = payload_size.min(condact_offset + MAX_BYTECODE_LENGTH);
                let stream = &data[offset + condact_offset..offset + end];
                referenced_streams += 1;
                if let Some(terminator) = stream.iter().position(|&b| b == STREAM_TERMINATOR) {
                    if terminator == 0 {
                        return ProcessValidation {
                            valid: false,
                            reason: "empty_condact_stream".into(),
                            process_table_offset: process_table,
                            process_offsets,
                            parsed_entries,
                            terminated_streams,
                        };
                    }
                    terminated_streams += 1;
                } else {
                    return ProcessValidation {
                        valid: false,
                        reason: "unterminated_condact_stream".into(),
                        process_table_offset: process_table,
                        process_offsets,
                        parsed_entries,
                        terminated_streams,
                    };
                }
                parsed_entries += 1;
                entry_position += 4;
                entries_count += 1;
            }
            if entries_count >= MAX_PROCESS_ENTRIES {
                return ProcessValidation {
                    valid: false,
                    reason: "process_entry_limit_exceeded".into(),
                    process_table_offset: process_table,
                    process_offsets,
                    parsed_entries,
                    terminated_streams,
                };
            }
        }

        if referenced_streams == 0 || terminated_streams != referenced_streams {
            return ProcessValidation {
                valid: false,
                reason: "no_valid_condact_streams".into(),
                process_table_offset: process_table,
                process_offsets,
                parsed_entries,
                terminated_streams,
            };
        }

        ProcessValidation {
            valid: true,
            reason: "ok".into(),
            process_table_offset: process_table,
            process_offsets,
            parsed_entries,
            terminated_streams,
        }
    }

    fn validate_at(
        data: &[u8],
        offset: usize,
        wrapper: &str,
        allow_trailing: bool,
        legacy: bool,
    ) -> Option<(DDBHeader, ProcessValidation, usize)> {
        let header = if legacy {
            Self::parse_legacy_header(data, offset)?
        } else {
            Self::parse_header(data, offset)?
        };

        let available_size = data.len().saturating_sub(offset);
        let expected_size = header.expected_size();
        if expected_size < header.header_size || expected_size > available_size {
            return None;
        }
        if !allow_trailing && expected_size != available_size {
            let padding = available_size - expected_size;
            let padded_end = ((expected_size + 127) / 128) * 128;
            let padding_bytes = &data[offset + expected_size..offset + available_size];
            if !(legacy && padding > 0 && padding < 128 && available_size == padded_end && padding_bytes.iter().all(|&b| b == 0)) {
                return None;
            }
        }
        let payload_size = expected_size;
        if !legacy && header.submachine != 95 && header.machine_id != 0xF {
            return None;
        }

        let max_check_ptr = header.pointers.len().min(12);
        let mut pointer_offsets = Vec::with_capacity(max_check_ptr);
        for &ptr in &header.pointers[..max_check_ptr] {
            pointer_offsets.push(Self::pointer_index(ptr, header.base_address));
        }

        if pointer_offsets.len() <= 7 || pointer_offsets[1].is_none() || pointer_offsets[7].is_none() {
            return None;
        }
        for opt in &pointer_offsets {
            if let Some(idx) = *opt {
                if idx < header.header_size || idx >= payload_size {
                    return None;
                }
            }
        }

        if !legacy {
            let concrete: Vec<usize> = pointer_offsets.iter().filter_map(|&x| x).collect();
            if let Some(&max_idx) = concrete.iter().max() {
                if pointer_offsets[1] != Some(max_idx) {
                    return None;
                }
            }
        }

        let proc = Self::validate_process_entries(data, offset, &header, payload_size);
        if !proc.valid {
            return None;
        }

        Some((header, proc, payload_size))
    }

    pub fn parse_ddb(data: &[u8], filename: &str) -> DDBAnalysis {
        let (rejected, reason) = Self::is_explicit_rejection(data, filename);
        if rejected {
            let mut a = DDBAnalysis::default();
            a.reason = reason.into();
            return a;
        }

        for (offset, wrapper) in Self::candidate_offsets(data, filename) {
            let mut validated = Self::validate_at(data, offset, &wrapper, false, false);
            if validated.is_none() {
                validated = Self::validate_at(data, offset, &wrapper, false, true);
            }
            if let Some((header, proc, payload_size)) = validated {
                let format_name = if header.layout == "drc" {
                    format!("daad-v{}", header.major_version)
                } else {
                    format!("daad-v{}-legacy", header.major_version)
                };
                return DDBAnalysis {
                    is_daad: true,
                    confidence: 1.0,
                    confidence_label: "verified".into(),
                    version: Some(format!("DAAD DDB v{}", header.major_version)),
                    ddb_format: Some(format_name),
                    ddb_major_version: Some(header.major_version),
                    platform: header.platform.clone(),
                    language: header.language.clone(),
                    reason: format!("verified_structural_{}_ddb", header.layout),
                    payload_offset: offset,
                    payload_size,
                    header: Some(header),
                    process_validation: Some(proc),
                    container_wrapper: wrapper,
                };
            }
        }

        DDBAnalysis::default()
    }

    pub fn find_embedded_ddb(data: &[u8]) -> Option<(usize, usize, DDBAnalysis)> {
        if data.len() < HEADER_SIZE {
            return None;
        }
        let limit = (data.len() - HEADER_SIZE).min(MAX_EMBEDDED_SCAN);
        for offset in 0..=limit {
            let b0 = data[offset];
            if b0 != 1 && b0 != 2 && b0 != 3 {
                continue;
            }
            let b1 = data[offset + 1];
            if get_target_spec(b1 >> 4).is_none() {
                continue;
            }
            let mut validated = Self::validate_at(data, offset, "embedded", true, false);
            if validated.is_none() {
                validated = Self::validate_at(data, offset, "embedded", true, true);
            }
            if let Some((header, proc, payload_size)) = validated {
                let format_name = if header.layout == "drc" {
                    format!("daad-v{}", header.major_version)
                } else {
                    format!("daad-v{}-legacy", header.major_version)
                };
                let analysis = DDBAnalysis {
                    is_daad: true,
                    confidence: 1.0,
                    confidence_label: "verified".into(),
                    version: Some(format!("DAAD DDB v{}", header.major_version)),
                    ddb_format: Some(format_name),
                    ddb_major_version: Some(header.major_version),
                    platform: header.platform.clone(),
                    language: header.language.clone(),
                    reason: format!("verified_structural_{}_ddb", header.layout),
                    payload_offset: offset,
                    payload_size,
                    header: Some(header),
                    process_validation: Some(proc),
                    container_wrapper: "embedded".into(),
                };
                return Some((offset, payload_size, analysis));
            }
        }
        None
    }
}

/// Disassembled CondAct instruction.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CondActInst {
    pub opcode: u8,
    pub name: String,
    pub params: Vec<u8>,
}

pub fn condact_name(opcode: u8, version: u8) -> &'static str {
    match opcode {
        0 => "AT",
        1 => "NOTAT",
        2 => "ATGT",
        3 => "ATLT",
        4 => "PRESENT",
        5 => "ABSENT",
        6 => "WORN",
        7 => "NOTWORN",
        8 => "CARRIED",
        9 => "NOTCARR",
        10 => "CHANCE",
        11 => "ZERO",
        12 => "NOTZERO",
        13 => "EQ",
        14 => "GT",
        15 => "LT",
        16 => "ADJECT1",
        17 => "ADJECT2",
        18 => "NOUN2",
        19 => "ADVERB",
        20 => "SFX",
        21 => "DESC",
        22 => "QUIT",
        23 => "END",
        24 => "DONE",
        25 => "OK",
        26 => "ANYKEY",
        27 => "SAVE",
        28 => "LOAD",
        29 => "TURNS",
        30 => "SCORE",
        31 => "CLS",
        32 => "DROP",
        33 => "WEAR",
        34 => "REMOVE",
        35 => "CREATE",
        36 => "DESTROY",
        37 => "SWAP",
        38 => "PLACE",
        39 => "SET",
        40 => "CLEAR",
        41 => "PLUS",
        42 => "MINUS",
        43 => "LET",
        44 => "NEWLINE",
        45 => "MES",
        46 => "SYSMESS",
        47 => "GOTO",
        48 => "RESET",
        49 => "AUTOG",
        50 => "AUTOD",
        51 => "AUTOW",
        52 => "AUTOR",
        53 => "PAUSE",
        54 => "SYNONYM",
        55 => "GOSUB",
        56 => "RETURN",
        57 => "PRINT",
        58 => "PROCESS",
        59 => "PARSE",
        60 => "EXTERN",
        _ => "CONDACT",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rejection_rules() {
        let (rej, reason) = DAADBytecodeParser::is_explicit_rejection(b"<html></html>", "index.html");
        assert!(rej);
        assert_eq!(reason, "explicit_non_daad_extension");

        let (rej, _) = DAADBytecodeParser::is_explicit_rejection(b"PK\x03\x04test", "game.bin");
        assert!(rej);
    }

    #[test]
    fn test_synthetic_drc_ddb_validation() {
        // Build a minimal, structurally valid DRC v2 Spectrum DDB
        let mut data = vec![0u8; 1024];
        let base = 0x8400u16;

        // Byte 0: version 2
        data[0] = 2;
        // Byte 1: ZX (0x10) + Spanish (0x01)
        data[1] = 0x11;
        // Byte 2: submachine 95 (0x5F)
        data[2] = 95;
        // Byte 3..7: counts
        data[3] = 10; // objects
        data[4] = 10; // locations
        data[5] = 10; // messages
        data[6] = 10; // sys messages
        data[7] = 1;  // 1 process

        // Pointers table (13 pointers, big-endian)
        // Offset 8: Pointers
        // Ptr 0: Vocab = base + 70
        let p_vocab = base + 70;
        data[8] = (p_vocab >> 8) as u8;
        data[9] = (p_vocab & 0xFF) as u8;

        // Ptr 1: Process Table = base + 200 (highest concrete offset)
        let p_proc = base + 200;
        data[10] = (p_proc >> 8) as u8;
        data[11] = (p_proc & 0xFF) as u8;

        // Ptr 7: Required section pointer = base + 100
        let p_sec7 = base + 100;
        data[8 + 7 * 2] = (p_sec7 >> 8) as u8;
        data[9 + 7 * 2] = (p_sec7 & 0xFF) as u8;

        // Ptr 12: File Length = base + 500
        let p_len = base + 500;
        data[8 + 12 * 2] = (p_len >> 8) as u8;
        data[9 + 12 * 2] = (p_len & 0xFF) as u8;

        // In process table (offset 200):
        // Process 0 pointer = base + 210
        let p_proc0 = base + 210;
        data[200] = (p_proc0 >> 8) as u8;
        data[201] = (p_proc0 & 0xFF) as u8;

        // At Process 0 (offset 210):
        // Entry: Verb=1, Noun=2, CondactPtr=base+220
        let p_cond = base + 220;
        data[210] = 1; // verb
        data[211] = 2; // noun
        data[212] = (p_cond >> 8) as u8;
        data[213] = (p_cond & 0xFF) as u8;
        // End of process table entry (verb=0)
        data[214] = 0;

        // At Condact stream (offset 220):
        data[220] = 45; // MES opcode
        data[221] = 1;  // mes param
        data[222] = 0xFF; // Stream terminator

        // Test with exact file length (500 bytes)
        let payload = &data[..500];
        let analysis = DAADBytecodeParser::parse_ddb(payload, "game.ddb");
        assert!(analysis.is_daad);
        assert_eq!(analysis.platform, "zx");
        assert_eq!(analysis.language, "es");
        assert_eq!(analysis.ddb_major_version, Some(2));
        assert_eq!(analysis.confidence, 1.0);
    }
}
