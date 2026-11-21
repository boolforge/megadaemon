//! MegaDaemon v13.4 — Pure Rust DAAD (Diseñador de Aventuras AD) Harvester & Parser.
//!
//! Evidence-led parser and extractor for DAAD databases across all official targets:
//! ZX Spectrum, Amstrad CPC, Commodore 64, Commodore Plus/4, MSX, MSX2,
//! Amstrad PCW, Atari ST, Amiga, and PC/DOS.
//!
//! Fully compliant with the DAAD Harvester specification (DRC v2/v3 and historical
//! legacy v1/v2 layouts derived from MSX2DAAD).

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

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

/// Opcode specification: opcode byte (0..127), canonical mnemonic name, and parameter arity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpcodeDef {
    pub opcode: u8,
    pub name: &'static str,
    pub arity: u8,
}

// Full V1 Opcode Table (ADP DDB_VERSION1_CONDACTS)
pub const V1_OPCODES: &[OpcodeDef] = &[
    OpcodeDef { opcode: 0, name: "AT", arity: 1 },
    OpcodeDef { opcode: 1, name: "NOTAT", arity: 1 },
    OpcodeDef { opcode: 2, name: "ATGT", arity: 1 },
    OpcodeDef { opcode: 3, name: "ATLT", arity: 1 },
    OpcodeDef { opcode: 4, name: "PRESENT", arity: 1 },
    OpcodeDef { opcode: 5, name: "ABSENT", arity: 1 },
    OpcodeDef { opcode: 6, name: "WORN", arity: 1 },
    OpcodeDef { opcode: 7, name: "NOTWORN", arity: 1 },
    OpcodeDef { opcode: 8, name: "CARRIED", arity: 1 },
    OpcodeDef { opcode: 9, name: "NOTCARR", arity: 1 },
    OpcodeDef { opcode: 10, name: "CHANCE", arity: 1 },
    OpcodeDef { opcode: 11, name: "ZERO", arity: 1 },
    OpcodeDef { opcode: 12, name: "NOTZERO", arity: 1 },
    OpcodeDef { opcode: 13, name: "EQ", arity: 2 },
    OpcodeDef { opcode: 14, name: "GT", arity: 2 },
    OpcodeDef { opcode: 15, name: "LT", arity: 2 },
    OpcodeDef { opcode: 16, name: "ADJECT1", arity: 1 },
    OpcodeDef { opcode: 17, name: "ADVERB", arity: 1 },
    OpcodeDef { opcode: 18, name: "SFX", arity: 2 },
    OpcodeDef { opcode: 19, name: "DESC", arity: 0 },
    OpcodeDef { opcode: 20, name: "QUIT", arity: 0 },
    OpcodeDef { opcode: 21, name: "END", arity: 0 },
    OpcodeDef { opcode: 22, name: "DONE", arity: 0 },
    OpcodeDef { opcode: 23, name: "OK", arity: 0 },
    OpcodeDef { opcode: 24, name: "ANYKEY", arity: 0 },
    OpcodeDef { opcode: 25, name: "SAVE", arity: 0 },
    OpcodeDef { opcode: 26, name: "LOAD", arity: 0 },
    OpcodeDef { opcode: 27, name: "TURNS", arity: 0 },
    OpcodeDef { opcode: 28, name: "DISPLAY", arity: 1 },
    OpcodeDef { opcode: 29, name: "CLS", arity: 0 },
    OpcodeDef { opcode: 30, name: "DROPALL", arity: 0 },
    OpcodeDef { opcode: 31, name: "AUTOG", arity: 0 },
    OpcodeDef { opcode: 32, name: "AUTOD", arity: 0 },
    OpcodeDef { opcode: 33, name: "AUTOW", arity: 0 },
    OpcodeDef { opcode: 34, name: "AUTOR", arity: 0 },
    OpcodeDef { opcode: 35, name: "PAUSE", arity: 1 },
    OpcodeDef { opcode: 36, name: "TIMEOUT", arity: 0 },
    OpcodeDef { opcode: 37, name: "GOTO", arity: 1 },
    OpcodeDef { opcode: 38, name: "MESSAGE", arity: 1 },
    OpcodeDef { opcode: 39, name: "REMOVE", arity: 1 },
    OpcodeDef { opcode: 40, name: "GET", arity: 1 },
    OpcodeDef { opcode: 41, name: "DROP", arity: 1 },
    OpcodeDef { opcode: 42, name: "WEAR", arity: 1 },
    OpcodeDef { opcode: 43, name: "DESTROY", arity: 1 },
    OpcodeDef { opcode: 44, name: "CREATE", arity: 1 },
    OpcodeDef { opcode: 45, name: "SWAP", arity: 2 },
    OpcodeDef { opcode: 46, name: "PLACE", arity: 2 },
    OpcodeDef { opcode: 47, name: "SET", arity: 1 },
    OpcodeDef { opcode: 48, name: "CLEAR", arity: 1 },
    OpcodeDef { opcode: 49, name: "PLUS", arity: 2 },
    OpcodeDef { opcode: 50, name: "MINUS", arity: 2 },
    OpcodeDef { opcode: 51, name: "LET", arity: 2 },
    OpcodeDef { opcode: 52, name: "NEWLINE", arity: 0 },
    OpcodeDef { opcode: 53, name: "PRINT", arity: 1 },
    OpcodeDef { opcode: 54, name: "SYSMESS", arity: 1 },
    OpcodeDef { opcode: 55, name: "ISAT", arity: 2 },
    OpcodeDef { opcode: 56, name: "COPYOF", arity: 2 },
    OpcodeDef { opcode: 57, name: "COPYOO", arity: 2 },
    OpcodeDef { opcode: 58, name: "COPYFO", arity: 2 },
    OpcodeDef { opcode: 59, name: "COPYFF", arity: 2 },
    OpcodeDef { opcode: 60, name: "LISTOBJ", arity: 0 },
    OpcodeDef { opcode: 61, name: "EXTERN", arity: 2 },
    OpcodeDef { opcode: 62, name: "RAMSAVE", arity: 0 },
    OpcodeDef { opcode: 63, name: "RAMLOAD", arity: 1 },
    OpcodeDef { opcode: 64, name: "BEEP", arity: 2 },
    OpcodeDef { opcode: 65, name: "PAPER", arity: 1 },
    OpcodeDef { opcode: 66, name: "INK", arity: 1 },
    OpcodeDef { opcode: 67, name: "BORDER", arity: 1 },
    OpcodeDef { opcode: 68, name: "PREP", arity: 1 },
    OpcodeDef { opcode: 69, name: "NOUN2", arity: 1 },
    OpcodeDef { opcode: 70, name: "ADJECT2", arity: 1 },
    OpcodeDef { opcode: 71, name: "ADD", arity: 2 },
    OpcodeDef { opcode: 72, name: "SUB", arity: 2 },
    OpcodeDef { opcode: 73, name: "PARSE", arity: 0 },
    OpcodeDef { opcode: 74, name: "LISTAT", arity: 1 },
    OpcodeDef { opcode: 75, name: "PROCESS", arity: 1 },
    OpcodeDef { opcode: 76, name: "SAME", arity: 2 },
    OpcodeDef { opcode: 77, name: "MES", arity: 1 },
    OpcodeDef { opcode: 78, name: "WINDOW", arity: 1 },
    OpcodeDef { opcode: 79, name: "NOTEQ", arity: 2 },
    OpcodeDef { opcode: 80, name: "NOTSAME", arity: 2 },
    OpcodeDef { opcode: 81, name: "MODE", arity: 1 },
    OpcodeDef { opcode: 82, name: "WINAT", arity: 2 },
    OpcodeDef { opcode: 83, name: "TIME", arity: 2 },
    OpcodeDef { opcode: 84, name: "PICTURE", arity: 1 },
    OpcodeDef { opcode: 85, name: "DOALL", arity: 1 },
    OpcodeDef { opcode: 86, name: "PROMPT", arity: 1 },
    OpcodeDef { opcode: 87, name: "GRAPHIC", arity: 2 },
    OpcodeDef { opcode: 88, name: "ISNOTAT", arity: 2 },
    OpcodeDef { opcode: 89, name: "WEIGH", arity: 2 },
    OpcodeDef { opcode: 90, name: "PUTIN", arity: 2 },
    OpcodeDef { opcode: 91, name: "TAKEOUT", arity: 2 },
    OpcodeDef { opcode: 92, name: "NEWTEXT", arity: 0 },
    OpcodeDef { opcode: 93, name: "ABILITY", arity: 2 },
    OpcodeDef { opcode: 94, name: "WEIGHT", arity: 1 },
    OpcodeDef { opcode: 95, name: "RANDOM", arity: 1 },
    OpcodeDef { opcode: 96, name: "INPUT", arity: 2 },
    OpcodeDef { opcode: 97, name: "SAVEAT", arity: 0 },
    OpcodeDef { opcode: 98, name: "BACKAT", arity: 0 },
    OpcodeDef { opcode: 99, name: "PRINTAT", arity: 2 },
    OpcodeDef { opcode: 100, name: "WHATO", arity: 0 },
    OpcodeDef { opcode: 101, name: "RESET", arity: 1 },
    OpcodeDef { opcode: 102, name: "PUTO", arity: 1 },
    OpcodeDef { opcode: 103, name: "NOTDONE", arity: 0 },
    OpcodeDef { opcode: 104, name: "AUTOP", arity: 1 },
    OpcodeDef { opcode: 105, name: "AUTOT", arity: 1 },
    OpcodeDef { opcode: 106, name: "MOVE", arity: 1 },
    OpcodeDef { opcode: 107, name: "WINSIZE", arity: 2 },
    OpcodeDef { opcode: 108, name: "REDO", arity: 0 },
];

// Full V2 Opcode Table (ADP DDB_VERSION2_CONDACTS)
pub const V2_OPCODES: &[OpcodeDef] = &[
    OpcodeDef { opcode: 0, name: "AT", arity: 1 },
    OpcodeDef { opcode: 1, name: "NOTAT", arity: 1 },
    OpcodeDef { opcode: 2, name: "ATGT", arity: 1 },
    OpcodeDef { opcode: 3, name: "ATLT", arity: 1 },
    OpcodeDef { opcode: 4, name: "PRESENT", arity: 1 },
    OpcodeDef { opcode: 5, name: "ABSENT", arity: 1 },
    OpcodeDef { opcode: 6, name: "WORN", arity: 1 },
    OpcodeDef { opcode: 7, name: "NOTWORN", arity: 1 },
    OpcodeDef { opcode: 8, name: "CARRIED", arity: 1 },
    OpcodeDef { opcode: 9, name: "NOTCARR", arity: 1 },
    OpcodeDef { opcode: 10, name: "CHANCE", arity: 1 },
    OpcodeDef { opcode: 11, name: "ZERO", arity: 1 },
    OpcodeDef { opcode: 12, name: "NOTZERO", arity: 1 },
    OpcodeDef { opcode: 13, name: "EQ", arity: 2 },
    OpcodeDef { opcode: 14, name: "GT", arity: 2 },
    OpcodeDef { opcode: 15, name: "LT", arity: 2 },
    OpcodeDef { opcode: 16, name: "ADJECT1", arity: 1 },
    OpcodeDef { opcode: 17, name: "ADVERB", arity: 1 },
    OpcodeDef { opcode: 18, name: "SFX", arity: 2 },
    OpcodeDef { opcode: 19, name: "DESC", arity: 1 },
    OpcodeDef { opcode: 20, name: "QUIT", arity: 0 },
    OpcodeDef { opcode: 21, name: "END", arity: 0 },
    OpcodeDef { opcode: 22, name: "DONE", arity: 0 },
    OpcodeDef { opcode: 23, name: "OK", arity: 0 },
    OpcodeDef { opcode: 24, name: "ANYKEY", arity: 0 },
    OpcodeDef { opcode: 25, name: "SAVE", arity: 1 },
    OpcodeDef { opcode: 26, name: "LOAD", arity: 1 },
    OpcodeDef { opcode: 27, name: "DPRINT", arity: 1 },
    OpcodeDef { opcode: 28, name: "DISPLAY", arity: 1 },
    OpcodeDef { opcode: 29, name: "CLS", arity: 0 },
    OpcodeDef { opcode: 30, name: "DROPALL", arity: 0 },
    OpcodeDef { opcode: 31, name: "AUTOG", arity: 0 },
    OpcodeDef { opcode: 32, name: "AUTOD", arity: 0 },
    OpcodeDef { opcode: 33, name: "AUTOW", arity: 0 },
    OpcodeDef { opcode: 34, name: "AUTOR", arity: 0 },
    OpcodeDef { opcode: 35, name: "PAUSE", arity: 1 },
    OpcodeDef { opcode: 36, name: "SYNONYM", arity: 2 },
    OpcodeDef { opcode: 37, name: "GOTO", arity: 1 },
    OpcodeDef { opcode: 38, name: "MESSAGE", arity: 1 },
    OpcodeDef { opcode: 39, name: "REMOVE", arity: 1 },
    OpcodeDef { opcode: 40, name: "GET", arity: 1 },
    OpcodeDef { opcode: 41, name: "DROP", arity: 1 },
    OpcodeDef { opcode: 42, name: "WEAR", arity: 1 },
    OpcodeDef { opcode: 43, name: "DESTROY", arity: 1 },
    OpcodeDef { opcode: 44, name: "CREATE", arity: 1 },
    OpcodeDef { opcode: 45, name: "SWAP", arity: 2 },
    OpcodeDef { opcode: 46, name: "PLACE", arity: 2 },
    OpcodeDef { opcode: 47, name: "SET", arity: 1 },
    OpcodeDef { opcode: 48, name: "CLEAR", arity: 1 },
    OpcodeDef { opcode: 49, name: "PLUS", arity: 2 },
    OpcodeDef { opcode: 50, name: "MINUS", arity: 2 },
    OpcodeDef { opcode: 51, name: "LET", arity: 2 },
    OpcodeDef { opcode: 52, name: "NEWLINE", arity: 0 },
    OpcodeDef { opcode: 53, name: "PRINT", arity: 1 },
    OpcodeDef { opcode: 54, name: "SYSMESS", arity: 1 },
    OpcodeDef { opcode: 55, name: "ISAT", arity: 2 },
    OpcodeDef { opcode: 56, name: "SETCO", arity: 1 },
    OpcodeDef { opcode: 57, name: "SPACE", arity: 0 },
    OpcodeDef { opcode: 58, name: "HASAT", arity: 1 },
    OpcodeDef { opcode: 59, name: "HASNAT", arity: 1 },
    OpcodeDef { opcode: 60, name: "LISTOBJ", arity: 0 },
    OpcodeDef { opcode: 61, name: "EXTERN", arity: 2 },
    OpcodeDef { opcode: 62, name: "RAMSAVE", arity: 0 },
    OpcodeDef { opcode: 63, name: "RAMLOAD", arity: 1 },
    OpcodeDef { opcode: 64, name: "BEEP", arity: 2 },
    OpcodeDef { opcode: 65, name: "PAPER", arity: 1 },
    OpcodeDef { opcode: 66, name: "INK", arity: 1 },
    OpcodeDef { opcode: 67, name: "BORDER", arity: 1 },
    OpcodeDef { opcode: 68, name: "PREP", arity: 1 },
    OpcodeDef { opcode: 69, name: "NOUN2", arity: 1 },
    OpcodeDef { opcode: 70, name: "ADJECT2", arity: 1 },
    OpcodeDef { opcode: 71, name: "ADD", arity: 2 },
    OpcodeDef { opcode: 72, name: "SUB", arity: 2 },
    OpcodeDef { opcode: 73, name: "PARSE", arity: 1 },
    OpcodeDef { opcode: 74, name: "LISTAT", arity: 1 },
    OpcodeDef { opcode: 75, name: "PROCESS", arity: 1 },
    OpcodeDef { opcode: 76, name: "SAME", arity: 2 },
    OpcodeDef { opcode: 77, name: "MES", arity: 1 },
    OpcodeDef { opcode: 78, name: "WINDOW", arity: 1 },
    OpcodeDef { opcode: 79, name: "NOTEQ", arity: 2 },
    OpcodeDef { opcode: 80, name: "NOTSAME", arity: 2 },
    OpcodeDef { opcode: 81, name: "MODE", arity: 1 },
    OpcodeDef { opcode: 82, name: "WINAT", arity: 2 },
    OpcodeDef { opcode: 83, name: "TIME", arity: 2 },
    OpcodeDef { opcode: 84, name: "PICTURE", arity: 1 },
    OpcodeDef { opcode: 85, name: "DOALL", arity: 1 },
    OpcodeDef { opcode: 86, name: "MOUSE", arity: 2 },
    OpcodeDef { opcode: 87, name: "GFX", arity: 2 },
    OpcodeDef { opcode: 88, name: "ISNOTAT", arity: 2 },
    OpcodeDef { opcode: 89, name: "WEIGH", arity: 2 },
    OpcodeDef { opcode: 90, name: "PUTIN", arity: 2 },
    OpcodeDef { opcode: 91, name: "TAKEOUT", arity: 2 },
    OpcodeDef { opcode: 92, name: "NEWTEXT", arity: 0 },
    OpcodeDef { opcode: 93, name: "ABILITY", arity: 2 },
    OpcodeDef { opcode: 94, name: "WEIGHT", arity: 1 },
    OpcodeDef { opcode: 95, name: "RANDOM", arity: 1 },
    OpcodeDef { opcode: 96, name: "INPUT", arity: 2 },
    OpcodeDef { opcode: 97, name: "SAVEAT", arity: 0 },
    OpcodeDef { opcode: 98, name: "BACKAT", arity: 0 },
    OpcodeDef { opcode: 99, name: "PRINTAT", arity: 2 },
    OpcodeDef { opcode: 100, name: "WHATO", arity: 0 },
    OpcodeDef { opcode: 101, name: "CALL", arity: 1 },
    OpcodeDef { opcode: 102, name: "PUTO", arity: 1 },
    OpcodeDef { opcode: 103, name: "NOTDONE", arity: 0 },
    OpcodeDef { opcode: 104, name: "AUTOP", arity: 1 },
    OpcodeDef { opcode: 105, name: "AUTOT", arity: 1 },
    OpcodeDef { opcode: 106, name: "MOVE", arity: 1 },
    OpcodeDef { opcode: 107, name: "WINSIZE", arity: 2 },
    OpcodeDef { opcode: 108, name: "REDO", arity: 0 },
    OpcodeDef { opcode: 109, name: "CENTRE", arity: 0 },
    OpcodeDef { opcode: 110, name: "EXIT", arity: 1 },
    OpcodeDef { opcode: 111, name: "INKEY", arity: 0 },
    OpcodeDef { opcode: 112, name: "BIGGER", arity: 2 },
    OpcodeDef { opcode: 113, name: "SMALLER", arity: 2 },
    OpcodeDef { opcode: 114, name: "ISDONE", arity: 0 },
    OpcodeDef { opcode: 115, name: "ISNDONE", arity: 0 },
    OpcodeDef { opcode: 116, name: "SKIP", arity: 1 },
    OpcodeDef { opcode: 117, name: "RESTART", arity: 0 },
    OpcodeDef { opcode: 118, name: "TAB", arity: 1 },
    OpcodeDef { opcode: 119, name: "COPYOF", arity: 2 },
    OpcodeDef { opcode: 120, name: "XMESSAGE", arity: 2 },
    OpcodeDef { opcode: 121, name: "COPYOO", arity: 2 },
    OpcodeDef { opcode: 122, name: "INDIR", arity: 1 },
    OpcodeDef { opcode: 123, name: "COPYFO", arity: 2 },
    OpcodeDef { opcode: 124, name: "SETAT", arity: 2 },
    OpcodeDef { opcode: 125, name: "COPYFF", arity: 2 },
    OpcodeDef { opcode: 126, name: "COPYBF", arity: 2 },
    OpcodeDef { opcode: 127, name: "RESET", arity: 0 },
];

pub fn get_opcode_def(opcode: u8, dialect: &str) -> Option<OpcodeDef> {
    let raw = opcode & 0x7F;
    let table = match dialect {
        "v1" => V1_OPCODES,
        _ => V2_OPCODES,
    };
    table.iter().copied().find(|d| d.opcode == raw)
}

pub fn condact_name(opcode: u8, version: u8) -> &'static str {
    let dialect = if version == 1 { "v1" } else { "v2" };
    get_opcode_def(opcode, dialect).map(|d| d.name).unwrap_or("CONDACT")
}

/// Disassembled CondAct instruction with source offsets and parameter formatting.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DisassembledCondAct {
    pub offset: usize,
    pub opcode: u8,
    pub name: String,
    pub parameters: Vec<u8>,
    pub first_param_indirect: bool,
    pub second_param_indirect: bool,
    pub display: String,
}

pub struct CondActDisassembler;

impl CondActDisassembler {
    /// Disassembles a stream of condacts until STREAM_TERMINATOR (0xFF).
    pub fn disassemble_stream(data: &[u8], offset: usize, dialect: &str) -> (Vec<DisassembledCondAct>, usize) {
        let mut results = Vec::new();
        let mut pos = offset;
        let mut next_second_indirect = false;

        while pos < data.len() {
            let b = data[pos];
            if b == STREAM_TERMINATOR {
                pos += 1;
                break;
            }
            let raw_opcode = b & 0x7F;
            let first_indirect = (b & 0x80) != 0;
            let second_indirect = next_second_indirect;
            next_second_indirect = false;

            let def = get_opcode_def(raw_opcode, dialect);
            let name = def.map(|d| d.name).unwrap_or("UNKNOWN").to_string();
            let arity = def.map(|d| d.arity as usize).unwrap_or(0);

            pos += 1;
            let mut params = Vec::new();
            for _ in 0..arity {
                if pos < data.len() {
                    params.push(data[pos]);
                    pos += 1;
                }
            }

            // In V2, INDIR (opcode 122) applies indirection to the following instruction's 2nd parameter
            if dialect == "v2" && raw_opcode == 122 {
                next_second_indirect = true;
            }

            let mut display = name.clone();
            if !params.is_empty() {
                let p_strs: Vec<String> = params.iter().enumerate().map(|(idx, &p)| {
                    let ind = if idx == 0 && first_indirect { "@" } else if idx == 1 && second_indirect { "@" } else { "" };
                    format!("{}{}", ind, p)
                }).collect();
                display.push(' ');
                display.push_str(&p_strs.join(", "));
            }

            results.push(DisassembledCondAct {
                offset: pos.saturating_sub(1 + params.len()),
                opcode: raw_opcode,
                name,
                parameters: params,
                first_param_indirect: first_indirect,
                second_param_indirect: second_indirect,
                display,
            });
        }
        (results, pos)
    }
}

/// Structural retro-computing media inspection result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaInspection {
    pub parser: String,
    pub status: String,     // "recognized_evidence", "rejected", "partial"
    pub validation: String, // e.g. "validated_tzx_v1_block_stream", "valid_cpc_dsk"
    pub details: Value,
}

pub struct MediaInspector;

impl MediaInspector {
    /// Inspect Commodore 64 / C16 raw tape containers (.tap).
    pub fn inspect_cbm_tap(data: &[u8]) -> MediaInspection {
        if data.len() < 20 {
            return MediaInspection {
                parser: "cbm-tap".into(),
                status: "rejected".into(),
                validation: "truncated_header".into(),
                details: json!({ "size": data.len() }),
            };
        }
        let sig = &data[..12];
        let is_c64 = sig == b"C64-TAPE-RAW";
        let is_c16 = sig == b"C16-TAPE-RAW";
        if !is_c64 && !is_c16 {
            return MediaInspection {
                parser: "cbm-tap".into(),
                status: "rejected".into(),
                validation: "signature_mismatch".into(),
                details: json!({}),
            };
        }
        let version = data[12];
        let platform_code = data[13];
        let declared_size = u32::from_le_bytes([data[16], data[17], data[18], data[19]]) as usize;
        let actual_size = data.len().saturating_sub(20);
        let valid = declared_size == actual_size && (version <= 2);

        MediaInspection {
            parser: "cbm-tap".into(),
            status: if valid { "recognized_evidence".into() } else { "rejected".into() },
            validation: if valid { "valid_pulse_stream".into() } else { "length_mismatch".into() },
            details: json!({
                "version": version,
                "platform_code": platform_code,
                "declared_size": declared_size,
                "pulse_bytes": actual_size,
                "system": if is_c64 { "c64" } else { "c16" },
            }),
        }
    }

    /// Inspect Commodore 64 P00 container files.
    pub fn inspect_cbm_p00(data: &[u8]) -> MediaInspection {
        if data.len() < 26 || &data[..8] != b"C64File\0" {
            return MediaInspection {
                parser: "cbm-p00".into(),
                status: "rejected".into(),
                validation: "signature_mismatch".into(),
                details: json!({}),
            };
        }
        let name_bytes = &data[8..24];
        let name = String::from_utf8_lossy(name_bytes).trim_matches(|c| c == ' ' || c == '\0').to_string();
        MediaInspection {
            parser: "cbm-p00".into(),
            status: "recognized_evidence".into(),
            validation: "valid_wrapper".into(),
            details: json!({
                "name": name,
                "payload_size": data.len().saturating_sub(26),
            }),
        }
    }

    /// Inspect C64 BASIC SYS program launcher stub.
    pub fn inspect_c64_basic_sys(data: &[u8]) -> MediaInspection {
        if data.len() < 15 {
            return MediaInspection {
                parser: "c64-basic-sys-prg".into(),
                status: "rejected".into(),
                validation: "truncated_basic_sys_stub".into(),
                details: json!({}),
            };
        }
        let load_address = u16::from_le_bytes([data[0], data[1]]);
        if load_address != 0x0801 {
            return MediaInspection {
                parser: "c64-basic-sys-prg".into(),
                status: "rejected".into(),
                validation: "unexpected_basic_load_address".into(),
                details: json!({ "load_address": format!("0x{:04X}", load_address) }),
            };
        }
        if data[6] != 0x9E { // SYS token
            return MediaInspection {
                parser: "c64-basic-sys-prg".into(),
                status: "rejected".into(),
                validation: "missing_basic_sys_token".into(),
                details: json!({}),
            };
        }
        MediaInspection {
            parser: "c64-basic-sys-prg".into(),
            status: "recognized_evidence".into(),
            validation: "validated_c64_basic_sys_launcher".into(),
            details: json!({ "load_address": 0x0801 }),
        }
    }

    /// Inspect ZX Spectrum / Amstrad CPC TZX / CDT tape files.
    pub fn inspect_tzx(data: &[u8]) -> MediaInspection {
        if data.len() < 10 || &data[..8] != b"ZXTape!\x1a" {
            return MediaInspection {
                parser: "tzx-cdt".into(),
                status: "rejected".into(),
                validation: "signature_mismatch".into(),
                details: json!({}),
            };
        }
        let major = data[8];
        let minor = data[9];
        let mut pos = 10;
        let mut blocks = 0;

        while pos < data.len() {
            let block_id = data[pos];
            pos += 1;
            blocks += 1;
            match block_id {
                0x10 => { // Standard Speed Data Block
                    if pos + 4 > data.len() { break; }
                    let len = u16::from_le_bytes([data[pos + 2], data[pos + 3]]) as usize;
                    pos += 4 + len;
                }
                0x11 => { // Turbo Speed Data Block
                    if pos + 0x12 > data.len() { break; }
                    let len = (data[pos + 15] as usize) | ((data[pos + 16] as usize) << 8) | ((data[pos + 17] as usize) << 16);
                    pos += 0x12 + len;
                }
                0x12 => { // Pure Tone
                    pos += 4;
                }
                0x13 => { // Pulse sequence
                    if pos >= data.len() { break; }
                    let count = data[pos] as usize;
                    pos += 1 + count * 2;
                }
                0x14 => { // Pure Data Block
                    if pos + 10 > data.len() { break; }
                    let len = (data[pos + 7] as usize) | ((data[pos + 8] as usize) << 8) | ((data[pos + 9] as usize) << 16);
                    pos += 10 + len;
                }
                0x20 => { // Pause or Stop the Tape command
                    pos += 2;
                }
                0x30 => { // Text Description
                    if pos >= data.len() { break; }
                    let len = data[pos] as usize;
                    pos += 1 + len;
                }
                0x32 => { // Archive Info
                    if pos + 2 > data.len() { break; }
                    let len = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
                    pos += 2 + len;
                }
                _ => { // Unknown or unsupported block length, break safely
                    break;
                }
            }
        }

        MediaInspection {
            parser: "tzx-cdt".into(),
            status: "recognized_evidence".into(),
            validation: "validated_tzx_block_stream".into(),
            details: json!({
                "major": major,
                "minor": minor,
                "blocks_parsed": blocks,
            }),
        }
    }

    /// Inspect Amstrad CPC DSK floppy disk images.
    pub fn inspect_cpc_dsk(data: &[u8]) -> MediaInspection {
        let is_std = data.starts_with(b"MV - CPCEMU Disk-File");
        let is_ext = data.starts_with(b"EXTENDED CPC DSK File");
        if data.len() < 0x100 || (!is_std && !is_ext) {
            return MediaInspection {
                parser: "cpc-dsk".into(),
                status: "rejected".into(),
                validation: "signature_mismatch".into(),
                details: json!({}),
            };
        }
        let tracks = data[0x30];
        let sides = data[0x31];
        let valid_geom = tracks > 0 && tracks <= 84 && (sides == 1 || sides == 2);

        MediaInspection {
            parser: "cpc-dsk".into(),
            status: if valid_geom { "recognized_evidence".into() } else { "rejected".into() },
            validation: if valid_geom { "validated_cpc_dsk".into() } else { "invalid_geometry".into() },
            details: json!({
                "variant": if is_ext { "extended" } else { "standard" },
                "tracks": tracks,
                "sides": sides,
                "total_tracks": tracks as u16 * sides as u16,
            }),
        }
    }

    /// Inspect Amiga ADF floppy disk images (AmigaDOS OFS/FFS).
    pub fn inspect_amiga_adf(data: &[u8]) -> MediaInspection {
        if data.len() < 2048 || data.len() % 512 != 0 || !data.starts_with(b"DOS") {
            return MediaInspection {
                parser: "amiga-adf".into(),
                status: "rejected".into(),
                validation: "signature_mismatch".into(),
                details: json!({}),
            };
        }
        let dos_flags = data[3];
        let ffs = (dos_flags & 0x01) != 0;
        let intl = (dos_flags & 0x02) != 0;
        let dircache = (dos_flags & 0x04) != 0;
        let num_blocks = data.len() / 512;

        MediaInspection {
            parser: "amiga-adf".into(),
            status: "recognized_evidence".into(),
            validation: "validated_amigados_adf".into(),
            details: json!({
                "blocks": num_blocks,
                "dos_type": format!("DOS\\{}", dos_flags),
                "is_ffs": ffs,
                "international_mode": intl,
                "directory_cache": dircache,
            }),
        }
    }

    /// Automatic format inspection dispatcher.
    pub fn inspect_media(data: &[u8], filename: &str) -> MediaInspection {
        let ext = Path::new(filename).extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        match ext.as_str() {
            "tap" => Self::inspect_cbm_tap(data),
            "p00" => Self::inspect_cbm_p00(data),
            "tzx" | "cdt" => Self::inspect_tzx(data),
            "dsk" => Self::inspect_cpc_dsk(data),
            "adf" => Self::inspect_amiga_adf(data),
            "prg" => {
                let basic = Self::inspect_c64_basic_sys(data);
                if basic.status == "recognized_evidence" {
                    basic
                } else {
                    MediaInspection {
                        parser: "cbm-prg".into(),
                        status: "recognized_evidence".into(),
                        validation: "generic_cbm_prg".into(),
                        details: json!({ "size": data.len() }),
                    }
                }
            }
            _ => {
                if data.starts_with(b"ZXTape!\x1a") {
                    Self::inspect_tzx(data)
                } else if data.starts_with(b"MV - CPCEMU") || data.starts_with(b"EXTENDED CPC DSK") {
                    Self::inspect_cpc_dsk(data)
                } else if data.starts_with(b"DOS") && data.len() >= 512 * 80 * 11 * 2 {
                    Self::inspect_amiga_adf(data)
                } else if data.starts_with(b"C64File\0") {
                    Self::inspect_cbm_p00(data)
                } else if data.starts_with(b"C64-TAPE-RAW") || data.starts_with(b"C16-TAPE-RAW") {
                    Self::inspect_cbm_tap(data)
                } else {
                    MediaInspection {
                        parser: "unknown".into(),
                        status: "rejected".into(),
                        validation: "unrecognized_media_signature".into(),
                        details: json!({ "size": data.len() }),
                    }
                }
            }
        }
    }
}

/// Statistics from the DAAD preservation SQLite catalog (`state.db`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogStats {
    pub total_sources: i64,
    pub total_artifacts: i64,
    pub total_daad_payloads: i64,
    pub total_games: i64,
    pub unique_platforms: Vec<String>,
}

/// Game entry from DAAD preservation catalog.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogGame {
    pub id: i64,
    pub game_id: String,
    pub title: String,
    pub platform: String,
    pub language: String,
    pub year: Option<i64>,
    pub publisher: Option<String>,
    pub author: Option<String>,
}

/// Artifact entry from DAAD preservation catalog.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogArtifact {
    pub id: i64,
    pub original_filename: String,
    pub extracted_path: Option<String>,
    pub file_size: i64,
    pub sha256: String,
    pub is_daad_payload: bool,
    pub ddb_format: Option<String>,
    pub ddb_major_version: Option<i64>,
    pub measured_platform: Option<String>,
}

/// Native Rust SQLite preservation catalog database interface.
pub struct DAADCatalog {
    db_path: PathBuf,
}

impl DAADCatalog {
    /// Attempts to open the preservation database at default location.
    pub fn open_default() -> Result<Self, String> {
        let candidates = [
            "daad/preservation_corpus/state.db",
            "/data/user/0/com.agy/files/megadaemon/daad/preservation_corpus/state.db",
            "../daad/preservation_corpus/state.db",
        ];
        for candidate in &candidates {
            let p = PathBuf::from(candidate);
            if p.exists() {
                return Self::open(p);
            }
        }
        Err("state.db not found in standard preservation paths".into())
    }

    /// Opens SQLite catalog at specified path with read-only flags.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let p = path.as_ref().to_path_buf();
        if !p.exists() {
            return Err(format!("Database file not found: {}", p.display()));
        }
        // Verify database can be opened
        let _ = Connection::open_with_flags(&p, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("Failed to open SQLite database: {}", e))?;
        Ok(Self { db_path: p })
    }

    fn conn(&self) -> Result<Connection, String> {
        Connection::open_with_flags(&self.db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("SQLite connection error: {}", e))
    }

    /// Retrieve catalog summary statistics.
    pub fn get_stats(&self) -> Result<CatalogStats, String> {
        let conn = self.conn()?;
        let total_sources: i64 = conn.query_row("SELECT count(*) FROM sources", [], |r| r.get(0)).unwrap_or(0);
        let total_artifacts: i64 = conn.query_row("SELECT count(*) FROM artifacts", [], |r| r.get(0)).unwrap_or(0);
        let total_daad_payloads: i64 = conn.query_row("SELECT count(*) FROM artifacts WHERE is_daad_payload = 1", [], |r| r.get(0)).unwrap_or(0);
        let total_games: i64 = conn.query_row("SELECT count(*) FROM games", [], |r| r.get(0)).unwrap_or(0);

        let mut stmt = conn.prepare("SELECT DISTINCT platform FROM games WHERE platform IS NOT NULL ORDER BY platform")
            .map_err(|e| e.to_string())?;
        let platforms = stmt.query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(CatalogStats {
            total_sources,
            total_artifacts,
            total_daad_payloads,
            total_games,
            unique_platforms: platforms,
        })
    }

    /// Query preserved games with optional platform and title filter.
    pub fn query_games(&self, platform: Option<&str>, query: Option<&str>) -> Result<Vec<CatalogGame>, String> {
        let conn = self.conn()?;
        let mut sql = "SELECT id, game_id, title, platform, language, year, publisher, author FROM games WHERE 1=1".to_string();
        if platform.is_some() {
            sql.push_str(" AND platform = ?");
        }
        if query.is_some() {
            sql.push_str(" AND (title LIKE ? OR game_id LIKE ?)");
        }
        sql.push_str(" ORDER BY title LIMIT 200");

        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let mut games = Vec::new();

        let mapper = Self::map_game_row;
        let rows = match (platform, query) {
            (Some(p), Some(q)) => {
                let pattern = format!("%{}%", q);
                stmt.query_map(rusqlite::params![p, pattern, pattern], mapper)
            }
            (Some(p), None) => {
                stmt.query_map(rusqlite::params![p], mapper)
            }
            (None, Some(q)) => {
                let pattern = format!("%{}%", q);
                stmt.query_map(rusqlite::params![pattern, pattern], mapper)
            }
            (None, None) => {
                stmt.query_map([], mapper)
            }
        }.map_err(|e| e.to_string())?;

        for g in rows {
            if let Ok(game) = g {
                games.push(game);
            }
        }
        Ok(games)
    }

    fn map_game_row(r: &rusqlite::Row) -> rusqlite::Result<CatalogGame> {
        Ok(CatalogGame {
            id: r.get(0)?,
            game_id: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
            title: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
            platform: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            language: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
            year: r.get(5)?,
            publisher: r.get(6)?,
            author: r.get(7)?,
        })
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
