//! Pure-Rust DAAD Harvester Load-Model Verifiers & Z80 Image Observers.
//!
//! Fail-closed static structural verification for 10 retro-computing targets:
//! 1. Amiga Hunk (`0x000003F3` HUNK_HEADER + HUNK_CODE + HUNK_RELOC32 + HUNK_END)
//! 2. Atari ST TOS PRG (`0x601A` header + delta relocation stream)
//! 3. Commodore 64 PRG (`$0801` BASIC `SYS 2063` launcher + `$4C` JMP entry)
//! 4. Commodore 64 Entry-State Admission (hash-bound capture gate & contract verifier)
//! 5. Amstrad CPC AMSDOS (128-byte binary type-2 header + 16-bit checksum)
//! 6. MS-DOS MZ (`MZ`/`ZM` header + relocation bounds + relative `CS:IP` / `SS:SP`)
//! 7. Commodore Plus/4 PRG (`$4001` BASIC `SYS 2063` wrapper without C64 assumptions)
//! 8. ZX Spectrum +3DOS (`PLUS3DOS\x1A` 128-byte header + 8-bit checksum + tail)
//! 9. MSX Z80 Image Observation (leading `0xC3` JP absolute prefix)
//! 10. Amstrad PCW Z80 Image Observation (leading CP/M BDOS `CD 05 00` call at offset 16)
//!
//! None of these verifiers enable execution (`execution_eligible` is always `false`).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

// ============================================================================
// Constants & Required Future-Capture Field Schemas
// ============================================================================

pub const AMIGA_HUNK_HEADER: u32 = 0x0000_03F3;
pub const AMIGA_HUNK_CODE: u32 = 0x0000_03E9;
pub const AMIGA_HUNK_RELOC32: u32 = 0x0000_03EC;
pub const AMIGA_HUNK_END: u32 = 0x0000_03F2;

pub const ATARI_ST_PRG_HEADER_SIZE: usize = 28;
pub const ATARI_ST_PRG_MAGIC: u16 = 0x601A;

pub const C64_EXPECTED_LOAD_ADDRESS: u16 = 0x0801;
pub const C64_EXPECTED_SYS_TARGET: u16 = 2063;
pub const C64_EXPECTED_MACHINE_CODE_OFFSET: usize = 16;

pub const CPC_AMSDOS_HEADER_SIZE: usize = 128;
pub const PLUS4_EXPECTED_LOAD_ADDRESS: u16 = 0x4001;
pub const PLUS4_EXPECTED_SYS_TARGET: u16 = 2063;
pub const PLUS4_EXPECTED_BASIC_LINE_END: usize = 16;

pub const ZX_PLUS3DOS_HEADER_SIZE: usize = 128;
pub const PCW_BDOS_CALL_OFFSET: usize = 16;
pub const PCW_BDOS_CALL_BYTES: &[u8; 3] = b"\xcd\x05\x00";

pub const AMIGA_FUTURE_CAPTURE_FIELDS: &[&str] = &[
    "official_hunk_sha256",
    "loader_context_sha256",
    "kickstart_rom_sha256",
    "amigados_identity",
    "loadseg_transition_sha256",
    "bootstrap_medium_sha256",
    "snapshot_sha256",
    "machine_configuration",
    "segment_list_mapping",
    "segment_allocation_addresses",
    "process_context",
    "cli_context",
    "m68000_registers",
    "stack_state",
    "library_device_state",
];

pub const ATARI_ST_FUTURE_CAPTURE_FIELDS: &[&str] = &[
    "official_prg_sha256",
    "loader_context_sha256",
    "tos_rom_sha256",
    "gemdos_identity",
    "pexec_transition_sha256",
    "bootstrap_medium_sha256",
    "snapshot_sha256",
    "machine_configuration",
    "basepage_mapping",
    "tpa_mapping",
    "segment_allocation_addresses",
    "process_context",
    "environment_context",
    "handle_state",
    "m68000_registers",
    "stack_state",
    "gem_aes_context",
];

pub const C64_ENTRY_STATE_REQUIRED_CAPTURE_FIELDS: &[&str] = &[
    "official_prg_sha256",
    "capture_sha256",
    "emulator_name",
    "emulator_version",
    "machine_model",
    "processor_port_ddr",
    "processor_port_data",
    "basic_rom_visible",
    "kernal_rom_visible",
    "io_visible",
    "ram_under_rom_visible",
    "entry_pc",
];

pub const CPC_FUTURE_CAPTURE_FIELDS: &[&str] = &[
    "official_program_sha256",
    "snapshot_sha256",
    "snapshot_format",
    "emulator_name",
    "emulator_version",
    "machine_model",
    "loader_transition_sha256",
    "entry_pc",
    "z80_registers_sha256",
    "gate_array_configuration",
    "gate_array_ram_configuration",
    "upper_rom_number",
    "firmware_rom_sha256",
    "pio_state_sha256",
    "psg_state_sha256",
    "memory_dump_sha256",
    "memory_dump_size",
];

pub const DOS_MZ_FUTURE_CAPTURE_FIELDS: &[&str] = &[
    "official_mz_sha256",
    "loader_context_sha256",
    "dos_kernel_sha256",
    "command_com_sha256",
    "exec_transition_sha256",
    "bootstrap_medium_sha256",
    "snapshot_sha256",
    "machine_configuration",
    "actual_load_segment",
    "psp_mapping",
    "memory_allocation_mapping",
    "environment_block",
    "command_tail",
    "fcb_state",
    "handle_state",
    "dta_state",
    "i8086_registers",
    "stack_state",
];

pub const PLUS4_FUTURE_CAPTURE_FIELDS: &[&str] = &[
    "official_prg_sha256",
    "loader_context_sha256",
    "basic_rom_sha256",
    "kernal_rom_sha256",
    "loader_transition_sha256",
    "bootstrap_medium_sha256",
    "snapshot_sha256",
    "machine_model",
    "mos7501_registers",
    "processor_port_ddr",
    "processor_port_data",
    "ted_paging_state",
    "ted_configuration_state",
    "ted_interrupt_state",
    "upper_memory_selection",
    "ram_mapping",
    "stack_state",
];

pub const ZX_PLUS3DOS_FUTURE_CAPTURE_FIELDS: &[&str] = &[
    "official_p3f_sha256",
    "disk_or_loader_context_sha256",
    "plus3dos_rom_sha256",
    "loader_transition_sha256",
    "bootstrap_sector_sha256",
    "snapshot_sha256",
    "machine_model",
    "z80_registers",
    "port_7ffd",
    "port_1ffd",
    "bankm",
    "bank678",
    "ram_page_mapping",
    "rom_selection",
    "stack_state",
    "plus3dos_vector_bytes",
];

pub const MSX_Z80_FUTURE_CAPTURE_FIELDS: &[&str] = &[
    "official_image_sha256",
    "loader_or_disk_context_sha256",
    "msxdos_system_sha256",
    "command_com_sha256",
    "disk_interface_rom_sha256",
    "launch_transition_sha256",
    "snapshot_sha256",
    "machine_model",
    "z80_registers",
    "primary_slot_register_a8",
    "expanded_slot_state",
    "memory_mapper_state",
    "page_mapping",
    "tpa_range",
    "bios_bdos_vector_bytes",
];

pub const PCW_Z80_FUTURE_CAPTURE_FIELDS: &[&str] = &[
    "official_image_sha256",
    "loader_or_disk_context_sha256",
    "cpm_system_image_sha256",
    "ccp_to_tpa_transition_sha256",
    "snapshot_sha256",
    "machine_model",
    "cpm_version",
    "z80_registers",
    "pcw_memory_paging",
    "zero_page_bank_state",
    "tpa_range",
    "bdos_vector_bytes",
];

// ============================================================================
// Core Serializable Output Structs
// ============================================================================

/// Structured fail-closed admission record returned by every load-model verifier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadModelAdmission {
    pub platform: String,
    pub verified: bool,
    pub admission_state: String,
    pub execution_eligible: bool,
    pub input_sha256: String,
    pub input_size: usize,
    pub evidence: Value,
    pub required_future_capture_fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Structured observation record for raw Z80 images (MSX / PCW) where load address
/// and banking remain unresolved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageObservation {
    pub platform: String,
    pub verified: bool,
    pub admission_state: String,
    pub execution_eligible: bool,
    pub input_sha256: String,
    pub image_size: usize,
    pub marker_kind: String,
    pub marker_offset: usize,
    pub marker_target: u16,
    pub required_future_capture_fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ImageObservation {
    pub fn into_admission(self) -> LoadModelAdmission {
        let evidence = if self.verified {
            match self.platform.as_str() {
                "msx_z80" => json!({
                    "leading_jump_offset": self.marker_offset,
                    "leading_jump_target": self.marker_target,
                    "image_size": self.image_size,
                }),
                "pcw_z80" => json!({
                    "bdos_call_offset": self.marker_offset,
                    "bdos_call_target": self.marker_target,
                    "image_size": self.image_size,
                }),
                _ => json!({
                    "marker_kind": self.marker_kind,
                    "marker_offset": self.marker_offset,
                    "marker_target": self.marker_target,
                    "image_size": self.image_size,
                }),
            }
        } else {
            json!({})
        };

        LoadModelAdmission {
            platform: self.platform,
            verified: self.verified,
            admission_state: self.admission_state,
            execution_eligible: false,
            input_sha256: self.input_sha256,
            input_size: self.image_size,
            evidence,
            required_future_capture_fields: self.required_future_capture_fields,
            error: self.error,
        }
    }
}

// ============================================================================
// Typed Per-Platform Parsed Structures
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmigaHunkFields {
    pub code_longwords: u32,
    pub code_size: usize,
    pub relocation_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtariStPrgFields {
    pub text_size: u32,
    pub data_size: u32,
    pub bss_size: u32,
    pub symbol_size: u32,
    pub load_size: usize,
    pub first_relocation_offset: u32,
    pub relocation_stream_size: usize,
    pub relocation_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct C64PrgFields {
    pub load_address: u16,
    pub basic_line_number: u16,
    pub sys_target: u16,
    pub basic_program_end_offset: usize,
    pub machine_code_offset: usize,
    pub trailing_rem_marker: bool,
    pub entry_opcode: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpcAmsdosFields {
    pub binary_type: u8,
    pub load_address: u16,
    pub entry_address: u16,
    pub logical_payload_size: usize,
    pub real_payload_size: usize,
    pub stored_payload_size: usize,
    pub stored_header_checksum: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DosMzFields {
    pub declared_size: usize,
    pub header_size: usize,
    pub load_module_size: usize,
    pub load_module_start: usize,
    pub load_module_end_exclusive: usize,
    pub load_module_sha256: String,
    pub relocation_count: u16,
    pub relocation_offset: u16,
    pub initial_cs: u16,
    pub initial_ip: u16,
    pub relative_entry_offset: usize,
    pub initial_ss: u16,
    pub initial_sp: u16,
    pub relative_stack_offset: usize,
    pub overlay_number: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plus4PrgFields {
    pub load_address: u16,
    pub basic_line_number: u16,
    pub sys_target: u16,
    pub basic_program_end_offset: usize,
    pub image_start: usize,
    pub image_end_exclusive: usize,
    pub sys_target_within_loaded_image: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZxPlus3DosFields {
    pub declared_total: usize,
    pub declared_payload: usize,
    pub header_load_address: u16,
    pub physical_tail_size: usize,
    pub stored_checksum: u8,
}

// ============================================================================
// Byte & Hash Helpers
// ============================================================================

fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut out = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(out, "{:02x}", b);
    }
    out
}

fn read_be_u16(data: &[u8], offset: usize) -> Result<u16, String> {
    let slice = data
        .get(offset..offset + 2)
        .ok_or_else(|| format!("truncated BE u16 at offset {}", offset))?;
    Ok(u16::from_be_bytes([slice[0], slice[1]]))
}

fn read_be_u32(data: &[u8], offset: usize) -> Result<(u32, usize), String> {
    let slice = data
        .get(offset..offset + 4)
        .ok_or_else(|| format!("truncated BE u32 at offset {}", offset))?;
    Ok((
        u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]),
        offset + 4,
    ))
}

fn read_le_u16(data: &[u8], offset: usize) -> Result<u16, String> {
    let slice = data
        .get(offset..offset + 2)
        .ok_or_else(|| format!("truncated LE u16 at offset {}", offset))?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_le_u24(data: &[u8], offset: usize) -> Result<usize, String> {
    let slice = data
        .get(offset..offset + 3)
        .ok_or_else(|| format!("truncated LE u24 at offset {}", offset))?;
    Ok((slice[0] as usize) | ((slice[1] as usize) << 8) | ((slice[2] as usize) << 16))
}

fn read_le_u32(data: &[u8], offset: usize) -> Result<u32, String> {
    let slice = data
        .get(offset..offset + 4)
        .ok_or_else(|| format!("truncated LE u32 at offset {}", offset))?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn owned_fields(fields: &[&str]) -> Vec<String> {
    fields.iter().map(|s| (*s).to_string()).collect()
}

fn rejected_admission(
    platform: &str,
    data: &[u8],
    required_fields: &[&str],
    err: String,
) -> LoadModelAdmission {
    LoadModelAdmission {
        platform: platform.to_string(),
        verified: false,
        admission_state: "rejected".to_string(),
        execution_eligible: false,
        input_sha256: sha256_hex(data),
        input_size: data.len(),
        evidence: json!({}),
        required_future_capture_fields: owned_fields(required_fields),
        error: Some(err),
    }
}

// ============================================================================
// 1. Amiga Hunk Load Model
// ============================================================================

/// Validates the retained one-code-hunk + `HUNK_RELOC32` + `HUNK_END` subset.
pub fn parse_amiga_hunk_executable(data: &[u8]) -> Result<AmigaHunkFields, String> {
    let mut offset = 0usize;
    let (marker, next) = read_be_u32(data, offset).map_err(|_| "truncated Hunk word".to_string())?;
    offset = next;
    if marker != AMIGA_HUNK_HEADER {
        return Err("missing HUNK_HEADER (0x000003F3)".to_string());
    }

    let (name_words, next) =
        read_be_u32(data, offset).map_err(|_| "truncated Hunk word".to_string())?;
    offset = next;
    if name_words != 0 {
        return Err("unexpected resident-name hunk in retained executable".to_string());
    }

    let (table_size, next) =
        read_be_u32(data, offset).map_err(|_| "truncated Hunk word".to_string())?;
    offset = next;
    let (first_hunk, next) =
        read_be_u32(data, offset).map_err(|_| "truncated Hunk word".to_string())?;
    offset = next;
    let (last_hunk, next) =
        read_be_u32(data, offset).map_err(|_| "truncated Hunk word".to_string())?;
    offset = next;
    if table_size != 1 || first_hunk != 0 || last_hunk != 0 {
        return Err("expected exactly one allocated Hunk".to_string());
    }

    let (declared_size_word, next) =
        read_be_u32(data, offset).map_err(|_| "truncated Hunk word".to_string())?;
    offset = next;
    let declared_code_longwords = declared_size_word & 0x3FFF_FFFF;

    let (marker, next) = read_be_u32(data, offset).map_err(|_| "truncated Hunk word".to_string())?;
    offset = next;
    if marker != AMIGA_HUNK_CODE {
        return Err("expected HUNK_CODE after header".to_string());
    }

    let (code_longwords, next) =
        read_be_u32(data, offset).map_err(|_| "truncated Hunk word".to_string())?;
    offset = next;
    if code_longwords != declared_code_longwords {
        return Err("HUNK_CODE size differs from header allocation".to_string());
    }

    let code_size = (code_longwords as usize)
        .checked_mul(4)
        .ok_or_else(|| "HUNK_CODE size overflow".to_string())?;
    if offset
        .checked_add(code_size)
        .map_or(true, |end| end > data.len())
    {
        return Err("truncated HUNK_CODE payload".to_string());
    }
    offset += code_size;

    let (marker, next) = read_be_u32(data, offset).map_err(|_| "truncated Hunk word".to_string())?;
    offset = next;
    if marker != AMIGA_HUNK_RELOC32 {
        return Err("expected HUNK_RELOC32 after code payload".to_string());
    }

    let mut relocation_count: u32 = 0;
    loop {
        let (count, next) =
            read_be_u32(data, offset).map_err(|_| "truncated Hunk word".to_string())?;
        offset = next;
        if count == 0 {
            break;
        }
        let (target_hunk, next) =
            read_be_u32(data, offset).map_err(|_| "truncated Hunk word".to_string())?;
        offset = next;
        if target_hunk != 0 {
            return Err("relocation target exceeds retained one-Hunk table".to_string());
        }
        let group_bytes = (count as usize)
            .checked_mul(4)
            .ok_or_else(|| "unbounded HUNK_RELOC32 offset group".to_string())?;
        if count > code_longwords
            || offset
                .checked_add(group_bytes)
                .map_or(true, |end| end > data.len())
        {
            return Err("unbounded HUNK_RELOC32 offset group".to_string());
        }
        for _ in 0..count {
            let (rel_off, next) =
                read_be_u32(data, offset).map_err(|_| "truncated Hunk word".to_string())?;
            offset = next;
            let rel_off_usize = rel_off as usize;
            if rel_off_usize % 2 != 0
                || rel_off_usize
                    .checked_add(4)
                    .map_or(true, |end| end > code_size)
            {
                return Err("HUNK_RELOC32 offset lies outside code payload".to_string());
            }
        }
        relocation_count = relocation_count
            .checked_add(count)
            .ok_or_else(|| "relocation count overflow".to_string())?;
    }

    let (marker, next) = read_be_u32(data, offset).map_err(|_| "truncated Hunk word".to_string())?;
    offset = next;
    if marker != AMIGA_HUNK_END || offset != data.len() {
        return Err("expected terminal HUNK_END with no trailing bytes".to_string());
    }

    Ok(AmigaHunkFields {
        code_longwords,
        code_size,
        relocation_count,
    })
}

pub fn verify_amiga_hunk(data: &[u8]) -> LoadModelAdmission {
    match parse_amiga_hunk_executable(data) {
        Ok(fields) => LoadModelAdmission {
            platform: "amiga_hunk".to_string(),
            verified: true,
            admission_state: "hunk_container_and_relocations_verified_runtime_unresolved"
                .to_string(),
            execution_eligible: false,
            input_sha256: sha256_hex(data),
            input_size: data.len(),
            evidence: serde_json::to_value(fields).unwrap_or_else(|_| json!({})),
            required_future_capture_fields: owned_fields(AMIGA_FUTURE_CAPTURE_FIELDS),
            error: None,
        },
        Err(e) => rejected_admission("amiga_hunk", data, AMIGA_FUTURE_CAPTURE_FIELDS, e),
    }
}

// ============================================================================
// 2. Atari ST TOS PRG Load Model
// ============================================================================

/// Validates Atari ST PRG (`0x601A`) segment arithmetic and delta relocation stream.
pub fn parse_atari_st_prg(data: &[u8]) -> Result<AtariStPrgFields, String> {
    if data.len() < ATARI_ST_PRG_HEADER_SIZE || read_be_u16(data, 0)? != ATARI_ST_PRG_MAGIC {
        return Err("missing Atari ST PRG magic (0x601A)".to_string());
    }
    let (text_size, _) = read_be_u32(data, 2)?;
    let (data_size, _) = read_be_u32(data, 6)?;
    let (bss_size, _) = read_be_u32(data, 10)?;
    let (symbol_size, _) = read_be_u32(data, 14)?;
    let absflag = read_be_u16(data, 26)?;

    let text_data_size = (text_size as usize)
        .checked_add(data_size as usize)
        .ok_or_else(|| "PRG text+data overflow".to_string())?;
    let load_size = text_data_size
        .checked_add(symbol_size as usize)
        .ok_or_else(|| "PRG load_size overflow".to_string())?;
    let relocation_offset = ATARI_ST_PRG_HEADER_SIZE
        .checked_add(load_size)
        .ok_or_else(|| "PRG relocation_offset overflow".to_string())?;

    if relocation_offset > data.len() {
        return Err("PRG declared segments exceed retained file".to_string());
    }
    if absflag != 0 {
        return Err(
            "retained PRG absolute flag is not the expected relocatable form".to_string(),
        );
    }
    if relocation_offset
        .checked_add(5)
        .map_or(true, |end| end > data.len())
    {
        return Err("truncated PRG relocation stream".to_string());
    }

    let (first_offset, mut cursor) = read_be_u32(data, relocation_offset)?;
    if (first_offset as usize) >= text_data_size {
        return Err("first PRG relocation lies outside text/data image".to_string());
    }

    let mut current = first_offset as usize;
    let mut relocation_count = 1usize;
    loop {
        let delta = *data
            .get(cursor)
            .ok_or_else(|| "truncated PRG relocation stream".to_string())?;
        cursor += 1;
        if delta == 0 {
            break;
        }
        let step = if delta == 1 { 254usize } else { delta as usize };
        current = current
            .checked_add(step)
            .ok_or_else(|| "PRG relocation delta overflow".to_string())?;
        if current >= text_data_size {
            return Err("PRG relocation delta exceeds text/data image".to_string());
        }
        relocation_count += 1;
    }

    if cursor != data.len() {
        return Err("PRG relocation stream has trailing bytes".to_string());
    }

    Ok(AtariStPrgFields {
        text_size,
        data_size,
        bss_size,
        symbol_size,
        load_size,
        first_relocation_offset: first_offset,
        relocation_stream_size: cursor - relocation_offset,
        relocation_count,
    })
}

pub fn verify_atari_st_prg(data: &[u8]) -> LoadModelAdmission {
    match parse_atari_st_prg(data) {
        Ok(fields) => LoadModelAdmission {
            platform: "atari_st_prg".to_string(),
            verified: true,
            admission_state: "prg_segments_and_relocations_verified_runtime_unresolved".to_string(),
            execution_eligible: false,
            input_sha256: sha256_hex(data),
            input_size: data.len(),
            evidence: serde_json::to_value(fields).unwrap_or_else(|_| json!({})),
            required_future_capture_fields: owned_fields(ATARI_ST_FUTURE_CAPTURE_FIELDS),
            error: None,
        },
        Err(e) => rejected_admission("atari_st_prg", data, ATARI_ST_FUTURE_CAPTURE_FIELDS, e),
    }
}

// ============================================================================
// 3. Commodore 64 PRG Load Model
// ============================================================================

/// Validates the C64 `$0801` BASIC `SYS 2063` launcher and leading `$4C` JMP opcode.
pub fn parse_c64_prg(data: &[u8]) -> Result<C64PrgFields, String> {
    if data.len() < 15 {
        return Err("truncated C64 BASIC SYS stub".to_string());
    }
    let load_address = read_le_u16(data, 0)?;
    if load_address != C64_EXPECTED_LOAD_ADDRESS {
        return Err(format!(
            "unexpected C64 BASIC load address 0x{:04X} (expected 0x{:04X})",
            load_address, C64_EXPECTED_LOAD_ADDRESS
        ));
    }
    let next_line = read_le_u16(data, 2)?;
    let line_number = read_le_u16(data, 4)?;
    if data[6] != 0x9E {
        return Err(format!(
            "missing BASIC SYS token 0x9E (found 0x{:02X})",
            data[6]
        ));
    }

    let search_end = data.len().min(14);
    let line_terminator = data[7..search_end]
        .iter()
        .position(|&b| b == 0)
        .map(|rel| 7 + rel)
        .ok_or_else(|| "missing or empty SYS target".to_string())?;
    if line_terminator < 8 {
        return Err("missing or empty SYS target".to_string());
    }

    let (sys_terminator, trailing_rem_marker) =
        if line_terminator >= 9 && &data[line_terminator - 2..line_terminator] == b":\x8f" {
            (line_terminator - 2, true)
        } else {
            (line_terminator, false)
        };

    let target_bytes = &data[7..sys_terminator];
    if target_bytes.is_empty() || !target_bytes.iter().all(|b| b.is_ascii_digit()) {
        return Err("non-decimal SYS target".to_string());
    }
    let target_str =
        std::str::from_utf8(target_bytes).map_err(|_| "invalid UTF-8 SYS target".to_string())?;
    let sys_target_u32: u32 = target_str
        .parse()
        .map_err(|_| "SYS target integer parse failed".to_string())?;
    if sys_target_u32 > 0xFFFF {
        return Err("SYS target out of 16-bit range".to_string());
    }
    let sys_target = sys_target_u32 as u16;

    let terminal_link = line_terminator + 3;
    if terminal_link > data.len() || &data[line_terminator + 1..terminal_link] != b"\x00\x00" {
        return Err("missing terminal BASIC link".to_string());
    }

    let expected_next_line = (load_address as usize) + (line_terminator - 2 + 1);
    if (next_line as usize) != expected_next_line {
        return Err(format!(
            "inconsistent BASIC next-line pointer 0x{:04X} (expected 0x{:04X})",
            next_line, expected_next_line
        ));
    }

    if sys_target != C64_EXPECTED_SYS_TARGET {
        return Err(format!(
            "unexpected C64 SYS target {} (expected {})",
            sys_target, C64_EXPECTED_SYS_TARGET
        ));
    }
    if terminal_link != C64_EXPECTED_MACHINE_CODE_OFFSET {
        return Err(format!(
            "unexpected machine-code offset {} (expected {})",
            terminal_link, C64_EXPECTED_MACHINE_CODE_OFFSET
        ));
    }
    let entry_opcode = *data
        .get(C64_EXPECTED_MACHINE_CODE_OFFSET)
        .ok_or_else(|| "missing machine-code entry byte at offset 16".to_string())?;
    if entry_opcode != 0x4C {
        return Err(format!(
            "declared entry does not begin with measured JMP opcode 0x4C (found 0x{:02X})",
            entry_opcode
        ));
    }

    Ok(C64PrgFields {
        load_address,
        basic_line_number: line_number,
        sys_target,
        basic_program_end_offset: terminal_link,
        machine_code_offset: terminal_link,
        trailing_rem_marker,
        entry_opcode,
    })
}

pub fn verify_c64_prg(data: &[u8]) -> LoadModelAdmission {
    match parse_c64_prg(data) {
        Ok(fields) => LoadModelAdmission {
            platform: "c64_prg".to_string(),
            verified: true,
            admission_state: "load_and_basic_entry_verified_banking_unresolved".to_string(),
            execution_eligible: false,
            input_sha256: sha256_hex(data),
            input_size: data.len(),
            evidence: serde_json::to_value(fields).unwrap_or_else(|_| json!({})),
            required_future_capture_fields: owned_fields(C64_ENTRY_STATE_REQUIRED_CAPTURE_FIELDS),
            error: None,
        },
        Err(e) => rejected_admission(
            "c64_prg",
            data,
            C64_ENTRY_STATE_REQUIRED_CAPTURE_FIELDS,
            e,
        ),
    }
}

// ============================================================================
// 4. Commodore 64 Entry-State Admission
// ============================================================================

/// Validates a JSON C64 entry-state admission contract fail-closed.
pub fn validate_c64_entry_state_contract(contract: &Value) -> Result<(), String> {
    if contract.get("schema_version").and_then(Value::as_u64) != Some(1) {
        return Err("schema_version must be 1".to_string());
    }
    if contract.get("execution_eligible").and_then(Value::as_bool) != Some(false) {
        return Err("C64 entry-state admission must not enable execution".to_string());
    }
    let expected_fields: Vec<Value> = C64_ENTRY_STATE_REQUIRED_CAPTURE_FIELDS
        .iter()
        .map(|s| Value::String((*s).to_string()))
        .collect();
    if contract.get("required_capture_fields").and_then(Value::as_array) != Some(&expected_fields) {
        return Err("contract must preserve the C64 future-capture field schema".to_string());
    }
    let profiles = contract
        .get("profiles")
        .and_then(Value::as_array)
        .ok_or_else(|| "contract must bind both official C64 PRG profiles".to_string())?;
    let mut ids: Vec<&str> = profiles
        .iter()
        .filter_map(|p| p.get("artifact_id").and_then(Value::as_str))
        .collect();
    ids.sort_unstable();
    if ids != ["daad-c64-edi64-official", "daad-c64-sdi64-official"] {
        return Err("contract must bind both official C64 PRG profiles".to_string());
    }
    for profile in profiles {
        let sha = profile
            .get("prg_sha256")
            .and_then(Value::as_str)
            .unwrap_or("");
        if sha.len() != 64 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("official C64 PRG SHA-256 is required".to_string());
        }
        if profile.get("basic_sys_entry").and_then(Value::as_u64)
            != Some(C64_EXPECTED_SYS_TARGET as u64)
        {
            return Err(
                "contract must retain the measured BASIC SYS 2063 declaration".to_string(),
            );
        }
        if !matches!(profile.get("entry_state_observation"), None | Some(Value::Null)) {
            let aid = profile
                .get("artifact_id")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            return Err(format!(
                "{}: no official C64 entry-state capture is currently admitted",
                aid
            ));
        }
    }
    if contract.get("admission_state").and_then(Value::as_str)
        != Some("official_entry_state_not_observed")
    {
        return Err(
            "official C64 entry state remains not observed until a matching capture is retained"
                .to_string(),
        );
    }
    Ok(())
}

/// Verifies either a JSON C64 entry-state contract or a raw C64 PRG under the
/// entry-state admission gate (always retaining `official_entry_state_not_observed`).
pub fn verify_c64_entry_state(data: &[u8]) -> LoadModelAdmission {
    let trimmed_start = data.iter().copied().find(|b| !b.is_ascii_whitespace());
    if trimmed_start == Some(b'{') {
        match serde_json::from_slice::<Value>(data) {
            Ok(contract) => match validate_c64_entry_state_contract(&contract) {
                Ok(()) => LoadModelAdmission {
                    platform: "c64_entry_state".to_string(),
                    verified: true,
                    admission_state: "official_entry_state_not_observed".to_string(),
                    execution_eligible: false,
                    input_sha256: sha256_hex(data),
                    input_size: data.len(),
                    evidence: json!({
                        "contract_validated": true,
                        "basic_sys_entry": C64_EXPECTED_SYS_TARGET,
                        "entry_state_observed": false,
                    }),
                    required_future_capture_fields: owned_fields(
                        C64_ENTRY_STATE_REQUIRED_CAPTURE_FIELDS,
                    ),
                    error: None,
                },
                Err(e) => rejected_admission(
                    "c64_entry_state",
                    data,
                    C64_ENTRY_STATE_REQUIRED_CAPTURE_FIELDS,
                    e,
                ),
            },
            Err(e) => rejected_admission(
                "c64_entry_state",
                data,
                C64_ENTRY_STATE_REQUIRED_CAPTURE_FIELDS,
                format!("invalid C64 entry-state contract JSON: {}", e),
            ),
        }
    } else {
        match parse_c64_prg(data) {
            Ok(fields) => LoadModelAdmission {
                platform: "c64_entry_state".to_string(),
                verified: true,
                admission_state: "official_entry_state_not_observed".to_string(),
                execution_eligible: false,
                input_sha256: sha256_hex(data),
                input_size: data.len(),
                evidence: json!({
                    "basic_sys_entry": fields.sys_target,
                    "load_address": fields.load_address,
                    "machine_code_offset": fields.machine_code_offset,
                    "entry_state_observed": false,
                }),
                required_future_capture_fields: owned_fields(
                    C64_ENTRY_STATE_REQUIRED_CAPTURE_FIELDS,
                ),
                error: None,
            },
            Err(e) => rejected_admission(
                "c64_entry_state",
                data,
                C64_ENTRY_STATE_REQUIRED_CAPTURE_FIELDS,
                e,
            ),
        }
    }
}

// ============================================================================
// 5. Amstrad CPC AMSDOS Load Model
// ============================================================================

/// Validates the 128-byte AMSDOS binary type-2 header, 16-bit checksum, sizes, and entry range.
pub fn parse_amsdos_binary_header(data: &[u8]) -> Result<CpcAmsdosFields, String> {
    if data.len() < CPC_AMSDOS_HEADER_SIZE {
        return Err("truncated AMSDOS header".to_string());
    }
    let binary_type = data[0x12];
    if binary_type != 0x02 {
        return Err("retained CPC header is not AMSDOS binary type 2".to_string());
    }
    let load_address = read_le_u16(data, 0x15)?;
    let logical_size = read_le_u16(data, 0x18)? as usize;
    let entry_address = read_le_u16(data, 0x1A)?;
    let real_size = read_le_u24(data, 0x40)?;
    let stored_checksum = read_le_u16(data, 0x43)?;
    let computed_checksum: u16 = data[..0x43]
        .iter()
        .fold(0u32, |acc, &b| acc + (b as u32)) as u16;
    let stored_payload_size = data.len() - CPC_AMSDOS_HEADER_SIZE;

    if stored_checksum != computed_checksum {
        return Err(format!(
            "AMSDOS header checksum differs (stored 0x{:04X}, computed 0x{:04X})",
            stored_checksum, computed_checksum
        ));
    }
    if logical_size != stored_payload_size || real_size != stored_payload_size {
        return Err("AMSDOS declared payload size differs".to_string());
    }
    let entry = entry_address as usize;
    let load = load_address as usize;
    if !(load <= entry && entry < load + stored_payload_size) {
        return Err("AMSDOS entry is outside the loaded payload".to_string());
    }

    Ok(CpcAmsdosFields {
        binary_type,
        load_address,
        entry_address,
        logical_payload_size: logical_size,
        real_payload_size: real_size,
        stored_payload_size,
        stored_header_checksum: stored_checksum,
    })
}

pub fn verify_cpc_amsdos(data: &[u8]) -> LoadModelAdmission {
    match parse_amsdos_binary_header(data) {
        Ok(fields) => LoadModelAdmission {
            platform: "cpc_amsdos".to_string(),
            verified: true,
            admission_state: "amsdos_header_load_and_entry_verified_memory_unresolved".to_string(),
            execution_eligible: false,
            input_sha256: sha256_hex(data),
            input_size: data.len(),
            evidence: serde_json::to_value(fields).unwrap_or_else(|_| json!({})),
            required_future_capture_fields: owned_fields(CPC_FUTURE_CAPTURE_FIELDS),
            error: None,
        },
        Err(e) => rejected_admission("cpc_amsdos", data, CPC_FUTURE_CAPTURE_FIELDS, e),
    }
}

// ============================================================================
// 6. MS-DOS MZ Load Model
// ============================================================================

/// Validates the DOS `MZ`/`ZM` header, relocation table bounds, and relative `CS:IP` / `SS:SP`.
pub fn parse_mz_header(data: &[u8]) -> Result<DosMzFields, String> {
    if data.len() < 28 || (&data[..2] != b"MZ" && &data[..2] != b"ZM") {
        return Err("missing or truncated MZ signature".to_string());
    }
    let bytes_last_page = read_le_u16(data, 2)? as usize;
    let page_count = read_le_u16(data, 4)? as usize;
    let relocation_count = read_le_u16(data, 6)?;
    let header_paragraphs = read_le_u16(data, 8)? as usize;
    let initial_ss = read_le_u16(data, 14)?;
    let initial_sp = read_le_u16(data, 16)?;
    let initial_ip = read_le_u16(data, 20)?;
    let initial_cs = read_le_u16(data, 22)?;
    let relocation_offset = read_le_u16(data, 24)?;
    let overlay_number = read_le_u16(data, 26)?;

    if page_count == 0 || bytes_last_page > 512 {
        return Err("invalid MZ page fields".to_string());
    }
    let total_pages_bytes = page_count
        .checked_mul(512)
        .ok_or_else(|| "MZ page size overflow".to_string())?;
    let declared_size = if bytes_last_page > 0 {
        total_pages_bytes - (512 - bytes_last_page)
    } else {
        total_pages_bytes
    };
    let header_size = header_paragraphs * 16;
    if declared_size != data.len() || declared_size < header_size {
        return Err("declared MZ size does not match retained file".to_string());
    }

    let reloc_off_usize = relocation_offset as usize;
    let reloc_end = reloc_off_usize + (relocation_count as usize) * 4;
    if reloc_off_usize < 28 || reloc_end > header_size {
        return Err("MZ relocation table is outside the header".to_string());
    }

    let load_module_size = declared_size - header_size;
    let load_module = &data[header_size..declared_size];
    let load_module_sha256 = sha256_hex(load_module);
    let entry_offset = (initial_cs as usize) * 16 + (initial_ip as usize);
    let stack_offset = (initial_ss as usize) * 16 + (initial_sp as usize);

    if entry_offset >= load_module_size {
        return Err("MZ relative CS:IP is outside the load module".to_string());
    }

    for idx in 0..(relocation_count as usize) {
        let pos = reloc_off_usize + idx * 4;
        let rel_off_in_seg = read_le_u16(data, pos)? as usize;
        let rel_seg = read_le_u16(data, pos + 2)? as usize;
        if rel_seg * 16 + rel_off_in_seg + 2 > load_module_size {
            return Err("MZ relocation target is outside the load module".to_string());
        }
    }

    Ok(DosMzFields {
        declared_size,
        header_size,
        load_module_size,
        load_module_start: header_size,
        load_module_end_exclusive: declared_size,
        load_module_sha256,
        relocation_count,
        relocation_offset,
        initial_cs,
        initial_ip,
        relative_entry_offset: entry_offset,
        initial_ss,
        initial_sp,
        relative_stack_offset: stack_offset,
        overlay_number,
    })
}

pub fn verify_dos_mz(data: &[u8]) -> LoadModelAdmission {
    match parse_mz_header(data) {
        Ok(fields) => LoadModelAdmission {
            platform: "dos_mz".to_string(),
            verified: true,
            admission_state: "mz_header_and_relative_entry_verified_psp_and_runtime_unresolved"
                .to_string(),
            execution_eligible: false,
            input_sha256: sha256_hex(data),
            input_size: data.len(),
            evidence: serde_json::to_value(fields).unwrap_or_else(|_| json!({})),
            required_future_capture_fields: owned_fields(DOS_MZ_FUTURE_CAPTURE_FIELDS),
            error: None,
        },
        Err(e) => rejected_admission("dos_mz", data, DOS_MZ_FUTURE_CAPTURE_FIELDS, e),
    }
}

// ============================================================================
// 7. Commodore Plus/4 PRG Load Model
// ============================================================================

/// Validates the Plus/4 `$4001` PRG wrapper and single BASIC `SYS 2063` line.
pub fn parse_plus4_prg_wrapper(data: &[u8]) -> Result<Plus4PrgFields, String> {
    if data.len() < PLUS4_EXPECTED_BASIC_LINE_END + 1 {
        return Err("truncated Plus/4 PRG wrapper".to_string());
    }
    let load_address = read_le_u16(data, 0)?;
    let next_line = read_le_u16(data, 2)?;
    let line_number = read_le_u16(data, 4)?;
    if load_address != PLUS4_EXPECTED_LOAD_ADDRESS {
        return Err("unexpected Plus/4 PRG load address".to_string());
    }
    if next_line != 0x400D || line_number != 1 || data[6] != 0x9E {
        return Err("unrecognized Plus/4 BASIC launcher structure".to_string());
    }
    if &data[7..11] != b"2063" || &data[11..14] != b":\x8f\x00" || &data[14..16] != b"\x00\x00" {
        return Err("unexpected Plus/4 BASIC SYS line bytes".to_string());
    }
    let sys_target = PLUS4_EXPECTED_SYS_TARGET;
    let image_start = load_address as usize;
    let image_end = image_start + data.len() - 2;
    let sys_within = image_start <= (sys_target as usize) && (sys_target as usize) < image_end;

    Ok(Plus4PrgFields {
        load_address,
        basic_line_number: line_number,
        sys_target,
        basic_program_end_offset: PLUS4_EXPECTED_BASIC_LINE_END,
        image_start,
        image_end_exclusive: image_end,
        sys_target_within_loaded_image: sys_within,
    })
}

pub fn verify_plus4_prg(data: &[u8]) -> LoadModelAdmission {
    match parse_plus4_prg_wrapper(data) {
        Ok(fields) => {
            if fields.sys_target_within_loaded_image {
                return rejected_admission(
                    "plus4_prg",
                    data,
                    PLUS4_FUTURE_CAPTURE_FIELDS,
                    "expected unresolved SYS target must remain outside the PRG image".to_string(),
                );
            }
            LoadModelAdmission {
                platform: "plus4_prg".to_string(),
                verified: true,
                admission_state: "prg_wrapper_verified_launcher_target_unresolved".to_string(),
                execution_eligible: false,
                input_sha256: sha256_hex(data),
                input_size: data.len(),
                evidence: serde_json::to_value(fields).unwrap_or_else(|_| json!({})),
                required_future_capture_fields: owned_fields(PLUS4_FUTURE_CAPTURE_FIELDS),
                error: None,
            }
        }
        Err(e) => rejected_admission("plus4_prg", data, PLUS4_FUTURE_CAPTURE_FIELDS, e),
    }
}

// ============================================================================
// 8. ZX Spectrum +3DOS Load Model
// ============================================================================

/// Validates the 128-byte `PLUS3DOS\x1A` header, 8-bit checksum, and declared sizes.
pub fn parse_plus3dos_header(data: &[u8]) -> Result<ZxPlus3DosFields, String> {
    if data.len() < ZX_PLUS3DOS_HEADER_SIZE || !data.starts_with(b"PLUS3DOS\x1a") {
        return Err("missing or truncated PLUS3DOS header".to_string());
    }
    let stored_checksum = data[127];
    let computed_checksum: u8 = data[..127]
        .iter()
        .fold(0u32, |acc, &b| acc + (b as u32)) as u8;
    if stored_checksum != computed_checksum {
        return Err(format!(
            "PLUS3DOS header checksum differs (stored 0x{:02X}, computed 0x{:02X})",
            stored_checksum, computed_checksum
        ));
    }
    let declared_total = read_le_u32(data, 11)? as usize;
    let declared_payload = read_le_u16(data, 16)? as usize;
    let header_load_address = read_le_u16(data, 18)?;

    if declared_total < ZX_PLUS3DOS_HEADER_SIZE || declared_total > data.len() {
        return Err("PLUS3DOS declared total is outside retained file".to_string());
    }
    if declared_payload != declared_total - ZX_PLUS3DOS_HEADER_SIZE {
        return Err("PLUS3DOS declared payload does not match declared total".to_string());
    }

    Ok(ZxPlus3DosFields {
        declared_total,
        declared_payload,
        header_load_address,
        physical_tail_size: data.len() - declared_total,
        stored_checksum,
    })
}

pub fn verify_zx_plus3dos(data: &[u8]) -> LoadModelAdmission {
    match parse_plus3dos_header(data) {
        Ok(fields) => LoadModelAdmission {
            platform: "zx_plus3dos".to_string(),
            verified: true,
            admission_state: "plus3dos_header_verified_payload_execution_unresolved".to_string(),
            execution_eligible: false,
            input_sha256: sha256_hex(data),
            input_size: data.len(),
            evidence: serde_json::to_value(fields).unwrap_or_else(|_| json!({})),
            required_future_capture_fields: owned_fields(ZX_PLUS3DOS_FUTURE_CAPTURE_FIELDS),
            error: None,
        },
        Err(e) => rejected_admission("zx_plus3dos", data, ZX_PLUS3DOS_FUTURE_CAPTURE_FIELDS, e),
    }
}

// ============================================================================
// 9. MSX Z80 Image Observation
// ============================================================================

/// Observes the leading Z80 `JP nn` (`0xC3`) instruction without inferring a load address.
pub fn observe_msx_z80_image(data: &[u8]) -> ImageObservation {
    let sha = sha256_hex(data);
    if data.len() < 3 {
        return ImageObservation {
            platform: "msx_z80".to_string(),
            verified: false,
            admission_state: "rejected".to_string(),
            execution_eligible: false,
            input_sha256: sha,
            image_size: data.len(),
            marker_kind: "z80_jp_absolute".to_string(),
            marker_offset: 0,
            marker_target: 0,
            required_future_capture_fields: owned_fields(MSX_Z80_FUTURE_CAPTURE_FIELDS),
            error: Some("truncated MSX Z80 image".to_string()),
        };
    }
    if data[0] != 0xC3 {
        return ImageObservation {
            platform: "msx_z80".to_string(),
            verified: false,
            admission_state: "rejected".to_string(),
            execution_eligible: false,
            input_sha256: sha,
            image_size: data.len(),
            marker_kind: "z80_jp_absolute".to_string(),
            marker_offset: 0,
            marker_target: 0,
            required_future_capture_fields: owned_fields(MSX_Z80_FUTURE_CAPTURE_FIELDS),
            error: Some("MSX image does not begin with absolute JP opcode (0xC3)".to_string()),
        };
    }
    let target = u16::from_le_bytes([data[1], data[2]]);
    ImageObservation {
        platform: "msx_z80".to_string(),
        verified: true,
        admission_state: "image_identity_and_leading_jump_observed_load_unresolved".to_string(),
        execution_eligible: false,
        input_sha256: sha,
        image_size: data.len(),
        marker_kind: "z80_jp_absolute".to_string(),
        marker_offset: 0,
        marker_target: target,
        required_future_capture_fields: owned_fields(MSX_Z80_FUTURE_CAPTURE_FIELDS),
        error: None,
    }
}

pub fn verify_msx_z80(data: &[u8]) -> LoadModelAdmission {
    observe_msx_z80_image(data).into_admission()
}

// ============================================================================
// 10. Amstrad PCW Z80 Image Observation
// ============================================================================

/// Observes the leading CP/M BDOS `CALL 0x0005` (`CD 05 00`) sequence at offset 16.
pub fn observe_pcw_z80_image(data: &[u8]) -> ImageObservation {
    let sha = sha256_hex(data);
    let required_len = PCW_BDOS_CALL_OFFSET + PCW_BDOS_CALL_BYTES.len();
    if data.len() < required_len {
        return ImageObservation {
            platform: "pcw_z80".to_string(),
            verified: false,
            admission_state: "rejected".to_string(),
            execution_eligible: false,
            input_sha256: sha,
            image_size: data.len(),
            marker_kind: "cpm_bdos_call".to_string(),
            marker_offset: PCW_BDOS_CALL_OFFSET,
            marker_target: 0,
            required_future_capture_fields: owned_fields(PCW_Z80_FUTURE_CAPTURE_FIELDS),
            error: Some("truncated PCW Z80 image".to_string()),
        };
    }
    if &data[PCW_BDOS_CALL_OFFSET..required_len] != PCW_BDOS_CALL_BYTES {
        return ImageObservation {
            platform: "pcw_z80".to_string(),
            verified: false,
            admission_state: "rejected".to_string(),
            execution_eligible: false,
            input_sha256: sha,
            image_size: data.len(),
            marker_kind: "cpm_bdos_call".to_string(),
            marker_offset: PCW_BDOS_CALL_OFFSET,
            marker_target: 0,
            required_future_capture_fields: owned_fields(PCW_Z80_FUTURE_CAPTURE_FIELDS),
            error: Some("expected leading CP/M BDOS call bytes (CD 05 00) differ".to_string()),
        };
    }
    ImageObservation {
        platform: "pcw_z80".to_string(),
        verified: true,
        admission_state: "image_identity_and_bdos_call_observed_load_unresolved".to_string(),
        execution_eligible: false,
        input_sha256: sha,
        image_size: data.len(),
        marker_kind: "cpm_bdos_call".to_string(),
        marker_offset: PCW_BDOS_CALL_OFFSET,
        marker_target: 5,
        required_future_capture_fields: owned_fields(PCW_Z80_FUTURE_CAPTURE_FIELDS),
        error: None,
    }
}

pub fn verify_pcw_z80(data: &[u8]) -> LoadModelAdmission {
    observe_pcw_z80_image(data).into_admission()
}

// ============================================================================
// Unified Dispatcher
// ============================================================================

/// Dispatches load-model verification or Z80 image observation for all 10 targets.
pub fn verify_load_model(platform: &str, data: &[u8]) -> LoadModelAdmission {
    let norm = platform.trim().to_ascii_lowercase().replace('-', "_");
    match norm.as_str() {
        "amiga" | "amiga_hunk" | "hunk" => verify_amiga_hunk(data),
        "atari" | "atari_st" | "atari_st_prg" | "tos" => verify_atari_st_prg(data),
        "c64" | "c64_prg" => verify_c64_prg(data),
        "c64_entry" | "c64_entry_state" | "c64_entry_state_admission" => {
            verify_c64_entry_state(data)
        }
        "cpc" | "cpc_amsdos" | "amsdos" => verify_cpc_amsdos(data),
        "dos" | "dos_mz" | "mz" | "i8086" => verify_dos_mz(data),
        "plus4" | "plus4_prg" | "c16" => verify_plus4_prg(data),
        "zx" | "zx_plus3dos" | "plus3dos" | "p3f" => verify_zx_plus3dos(data),
        "msx" | "msx_z80" => verify_msx_z80(data),
        "pcw" | "pcw_z80" => verify_pcw_z80(data),
        other => LoadModelAdmission {
            platform: other.to_string(),
            verified: false,
            admission_state: "unsupported_platform".to_string(),
            execution_eligible: false,
            input_sha256: sha256_hex(data),
            input_size: data.len(),
            evidence: json!({}),
            required_future_capture_fields: Vec::new(),
            error: Some(format!("unsupported DAAD load-model platform '{}'", platform)),
        },
    }
}

/// Lists all 10 canonical platform identifiers supported by [`verify_load_model`].
pub const SUPPORTED_PLATFORMS: &[&str] = &[
    "amiga_hunk",
    "atari_st_prg",
    "c64_prg",
    "c64_entry_state",
    "cpc_amsdos",
    "dos_mz",
    "plus4_prg",
    "zx_plus3dos",
    "msx_z80",
    "pcw_z80",
];

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn build_valid_amiga_hunk() -> Vec<u8> {
        let mut buf = Vec::new();
        for w in [
            AMIGA_HUNK_HEADER,
            0, // name_words
            1, // table_size
            0, // first_hunk
            0, // last_hunk
            4, // declared_code_longwords = 4 (16 bytes)
            AMIGA_HUNK_CODE,
            4,          // code_longwords = 4
            0x4E714E71, // NOP NOP
            0x00000000,
            0x00000000,
            0x4E754E75, // RTS RTS
            AMIGA_HUNK_RELOC32,
            1, // 1 relocation
            0, // target_hunk = 0
            4, // offset = 4 (even, 4+4 <= 16)
            0, // end of reloc groups
            AMIGA_HUNK_END,
        ] {
            buf.extend_from_slice(&w.to_be_bytes());
        }
        buf
    }

    fn build_valid_atari_st_prg() -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&ATARI_ST_PRG_MAGIC.to_be_bytes());
        buf.extend_from_slice(&16u32.to_be_bytes()); // text_size
        buf.extend_from_slice(&8u32.to_be_bytes()); // data_size
        buf.extend_from_slice(&32u32.to_be_bytes()); // bss_size
        buf.extend_from_slice(&0u32.to_be_bytes()); // symbol_size
        buf.extend_from_slice(&[0u8; 8]); // reserved + flags
        buf.extend_from_slice(&0u16.to_be_bytes()); // absflag = 0 (relocatable)
        buf.extend_from_slice(&[0x4E; 24]); // text + data (24 bytes)
        buf.extend_from_slice(&4u32.to_be_bytes()); // first relocation offset = 4
        buf.push(8); // delta +8 -> offset 12
        buf.push(0); // terminator
        buf
    }

    fn build_valid_c64_prg() -> Vec<u8> {
        // load_addr=0x0801, next_line=0x080D (0x0801 + (13 - 2 + 1)), line=1,
        // 0x9E "2063" ":\x8f\x00" "\x00\x00", then 0x4C JMP
        let mut buf = Vec::new();
        buf.extend_from_slice(&0x0801u16.to_le_bytes());
        buf.extend_from_slice(&0x080Du16.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes());
        buf.push(0x9E);
        buf.extend_from_slice(b"2063");
        buf.extend_from_slice(b":\x8f\x00");
        buf.extend_from_slice(b"\x00\x00");
        buf.push(0x4C); // JMP opcode at offset 16
        buf.extend_from_slice(&[0x00, 0x09]);
        buf
    }

    fn build_valid_cpc_amsdos() -> Vec<u8> {
        let payload = vec![0xC9u8; 64]; // RET * 64
        let mut hdr = vec![0u8; CPC_AMSDOS_HEADER_SIZE];
        hdr[0x12] = 0x02; // binary type 2
        hdr[0x15..0x17].copy_from_slice(&0x4000u16.to_le_bytes()); // load_address
        hdr[0x18..0x1A].copy_from_slice(&(payload.len() as u16).to_le_bytes()); // logical_size
        hdr[0x1A..0x1C].copy_from_slice(&0x4010u16.to_le_bytes()); // entry_address
        hdr[0x40] = payload.len() as u8; // real_size (24-bit LE)
        let checksum: u16 = hdr[..0x43].iter().fold(0u32, |a, &b| a + b as u32) as u16;
        hdr[0x43..0x45].copy_from_slice(&checksum.to_le_bytes());
        hdr.extend_from_slice(&payload);
        hdr
    }

    fn build_valid_dos_mz() -> Vec<u8> {
        // 1 page (512 bytes total), header_paragraphs = 4 (64 bytes), 1 relocation at offset 28
        let mut buf = vec![0u8; 512];
        buf[0..2].copy_from_slice(b"MZ");
        buf[2..4].copy_from_slice(&0u16.to_le_bytes()); // bytes_last_page = 0 -> full 512
        buf[4..6].copy_from_slice(&1u16.to_le_bytes()); // page_count = 1
        buf[6..8].copy_from_slice(&1u16.to_le_bytes()); // relocation_count = 1
        buf[8..10].copy_from_slice(&4u16.to_le_bytes()); // header_paragraphs = 4 (64 B)
        buf[14..16].copy_from_slice(&0x0010u16.to_le_bytes()); // initial_ss
        buf[16..18].copy_from_slice(&0x0080u16.to_le_bytes()); // initial_sp
        buf[20..22].copy_from_slice(&0x0000u16.to_le_bytes()); // initial_ip
        buf[22..24].copy_from_slice(&0x0000u16.to_le_bytes()); // initial_cs
        buf[24..26].copy_from_slice(&28u16.to_le_bytes()); // relocation_offset
        buf[26..28].copy_from_slice(&0u16.to_le_bytes()); // overlay_number
        // 1 relocation entry at offset 28: seg=0, off=0x0010 (within 448 B load module)
        buf[28..30].copy_from_slice(&0x0010u16.to_le_bytes());
        buf[30..32].copy_from_slice(&0x0000u16.to_le_bytes());
        buf[64] = 0x90; // NOP at entry
        buf
    }

    fn build_valid_plus4_prg() -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&0x4001u16.to_le_bytes());
        buf.extend_from_slice(&0x400Du16.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes());
        buf.push(0x9E);
        buf.extend_from_slice(b"2063");
        buf.extend_from_slice(b":\x8f\x00");
        buf.extend_from_slice(b"\x00\x00");
        buf.push(0xEA); // trailing byte so len >= 17
        buf
    }

    fn build_valid_zx_plus3dos() -> Vec<u8> {
        let payload = vec![0x00u8; 32];
        let total = (ZX_PLUS3DOS_HEADER_SIZE + payload.len()) as u32;
        let mut buf = vec![0u8; ZX_PLUS3DOS_HEADER_SIZE];
        buf[..9].copy_from_slice(b"PLUS3DOS\x1a");
        buf[11..15].copy_from_slice(&total.to_le_bytes());
        buf[16..18].copy_from_slice(&(payload.len() as u16).to_le_bytes());
        buf[18..20].copy_from_slice(&0x8000u16.to_le_bytes());
        let csum: u8 = buf[..127].iter().fold(0u32, |a, &b| a + b as u32) as u8;
        buf[127] = csum;
        buf.extend_from_slice(&payload);
        buf.extend_from_slice(&[0xFF; 4]); // 4-byte physical tail
        buf
    }

    #[test]
    fn test_amiga_hunk_valid_and_rejected() {
        let valid = build_valid_amiga_hunk();
        let adm = verify_amiga_hunk(&valid);
        assert!(adm.verified, "err: {:?}", adm.error);
        assert!(!adm.execution_eligible);
        assert_eq!(adm.evidence["code_longwords"], 4);
        assert_eq!(adm.evidence["relocation_count"], 1);

        let mut bad = valid.clone();
        bad[3] = 0x00; // corrupt HUNK_HEADER
        assert!(!verify_amiga_hunk(&bad).verified);

        let mut trailing = valid;
        trailing.extend_from_slice(&[0, 0, 0, 0]);
        assert!(!verify_amiga_hunk(&trailing).verified);
    }

    #[test]
    fn test_atari_st_prg_valid_and_rejected() {
        let valid = build_valid_atari_st_prg();
        let adm = verify_atari_st_prg(&valid);
        assert!(adm.verified, "err: {:?}", adm.error);
        assert_eq!(adm.evidence["relocation_count"], 2);
        assert_eq!(adm.evidence["text_size"], 16);

        let mut bad_magic = valid.clone();
        bad_magic[0] = 0x00;
        assert!(!verify_atari_st_prg(&bad_magic).verified);

        let mut bad_abs = valid;
        bad_abs[27] = 1; // non-zero absflag
        assert!(!verify_atari_st_prg(&bad_abs).verified);
    }

    #[test]
    fn test_c64_prg_and_entry_state_valid_and_rejected() {
        let valid = build_valid_c64_prg();
        let adm = verify_c64_prg(&valid);
        assert!(adm.verified, "err: {:?}", adm.error);
        assert_eq!(adm.evidence["load_address"], 0x0801);
        assert_eq!(adm.evidence["sys_target"], 2063);

        let mut bad_jmp = valid.clone();
        bad_jmp[16] = 0xEA; // NOP instead of JMP
        assert!(!verify_c64_prg(&bad_jmp).verified);

        // C64 entry-state via raw PRG
        let es_prg = verify_c64_entry_state(&valid);
        assert!(es_prg.verified);
        assert_eq!(es_prg.admission_state, "official_entry_state_not_observed");

        // C64 entry-state via JSON contract
        let contract = json!({
            "schema_version": 1,
            "execution_eligible": false,
            "admission_state": "official_entry_state_not_observed",
            "required_capture_fields": C64_ENTRY_STATE_REQUIRED_CAPTURE_FIELDS,
            "profiles": [
                {
                    "artifact_id": "daad-c64-edi64-official",
                    "prg_sha256": "a".repeat(64),
                    "basic_sys_entry": 2063,
                    "entry_state_observation": null
                },
                {
                    "artifact_id": "daad-c64-sdi64-official",
                    "prg_sha256": "b".repeat(64),
                    "basic_sys_entry": 2063,
                    "entry_state_observation": null
                }
            ]
        });
        let es_json = verify_c64_entry_state(contract.to_string().as_bytes());
        assert!(es_json.verified, "err: {:?}", es_json.error);

        // Reject if execution_eligible is true
        let mut bad_contract = contract;
        bad_contract["execution_eligible"] = json!(true);
        assert!(!verify_c64_entry_state(bad_contract.to_string().as_bytes()).verified);
    }

    #[test]
    fn test_cpc_amsdos_valid_and_rejected() {
        let valid = build_valid_cpc_amsdos();
        let adm = verify_cpc_amsdos(&valid);
        assert!(adm.verified, "err: {:?}", adm.error);
        assert_eq!(adm.evidence["load_address"], 0x4000);
        assert_eq!(adm.evidence["entry_address"], 0x4010);

        let mut bad_csum = valid.clone();
        bad_csum[0x43] ^= 0xFF;
        assert!(!verify_cpc_amsdos(&bad_csum).verified);

        let mut bad_type = valid;
        bad_type[0x12] = 0x00;
        assert!(!verify_cpc_amsdos(&bad_type).verified);
    }

    #[test]
    fn test_dos_mz_valid_and_rejected() {
        let valid = build_valid_dos_mz();
        let adm = verify_dos_mz(&valid);
        assert!(adm.verified, "err: {:?}", adm.error);
        assert_eq!(adm.evidence["header_size"], 64);
        assert_eq!(adm.evidence["load_module_size"], 448);
        assert_eq!(adm.evidence["relocation_count"], 1);

        let mut bad_sig = valid.clone();
        bad_sig[0] = b'X';
        assert!(!verify_dos_mz(&bad_sig).verified);

        // Relocation target out of bounds
        let mut bad_reloc = valid;
        bad_reloc[30..32].copy_from_slice(&0x1000u16.to_le_bytes());
        assert!(!verify_dos_mz(&bad_reloc).verified);
    }

    #[test]
    fn test_plus4_prg_valid_and_rejected() {
        let valid = build_valid_plus4_prg();
        let adm = verify_plus4_prg(&valid);
        assert!(adm.verified, "err: {:?}", adm.error);
        assert_eq!(adm.evidence["load_address"], 0x4001);
        assert_eq!(adm.evidence["sys_target"], 2063);
        assert_eq!(adm.evidence["sys_target_within_loaded_image"], false);

        let mut bad_addr = valid;
        bad_addr[0..2].copy_from_slice(&0x0801u16.to_le_bytes());
        assert!(!verify_plus4_prg(&bad_addr).verified);
    }

    #[test]
    fn test_zx_plus3dos_valid_and_rejected() {
        let valid = build_valid_zx_plus3dos();
        let adm = verify_zx_plus3dos(&valid);
        assert!(adm.verified, "err: {:?}", adm.error);
        assert_eq!(adm.evidence["declared_payload"], 32);
        assert_eq!(adm.evidence["physical_tail_size"], 4);

        let mut bad_csum = valid.clone();
        bad_csum[127] ^= 0x01;
        assert!(!verify_zx_plus3dos(&bad_csum).verified);

        let mut bad_magic = valid;
        bad_magic[0] = b'X';
        assert!(!verify_zx_plus3dos(&bad_magic).verified);
    }

    #[test]
    fn test_msx_and_pcw_z80_observations() {
        let msx_ok = vec![0xC3, 0x34, 0x12, 0x00, 0x00];
        let obs = observe_msx_z80_image(&msx_ok);
        assert!(obs.verified);
        assert_eq!(obs.marker_target, 0x1234);
        assert!(!observe_msx_z80_image(&[0x00, 0x34, 0x12]).verified);

        let mut pcw_ok = vec![0u8; 32];
        pcw_ok[16..19].copy_from_slice(PCW_BDOS_CALL_BYTES);
        let p_obs = observe_pcw_z80_image(&pcw_ok);
        assert!(p_obs.verified);
        assert_eq!(p_obs.marker_offset, 16);
        assert_eq!(p_obs.marker_target, 5);
        assert!(!observe_pcw_z80_image(&vec![0u8; 32]).verified);
    }

    #[test]
    fn test_unified_dispatcher_all_targets() {
        assert!(verify_load_model("amiga_hunk", &build_valid_amiga_hunk()).verified);
        assert!(verify_load_model("atari_st_prg", &build_valid_atari_st_prg()).verified);
        assert!(verify_load_model("c64_prg", &build_valid_c64_prg()).verified);
        assert!(verify_load_model("c64_entry_state", &build_valid_c64_prg()).verified);
        assert!(verify_load_model("cpc_amsdos", &build_valid_cpc_amsdos()).verified);
        assert!(verify_load_model("dos_mz", &build_valid_dos_mz()).verified);
        assert!(verify_load_model("plus4_prg", &build_valid_plus4_prg()).verified);
        assert!(verify_load_model("zx_plus3dos", &build_valid_zx_plus3dos()).verified);
        assert!(verify_load_model("msx_z80", &[0xC3, 0x00, 0x40]).verified);
        let mut pcw = vec![0u8; 24];
        pcw[16..19].copy_from_slice(PCW_BDOS_CALL_BYTES);
        assert!(verify_load_model("pcw_z80", &pcw).verified);

        let unknown = verify_load_model("dreamcast", b"abc");
        assert!(!unknown.verified);
        assert_eq!(unknown.admission_state, "unsupported_platform");
    }
}
